import { ConnectorError, IntegrationError } from "hyperswitch-prism"
import {
  AuthorizePaymentInput,
  AuthorizePaymentOutput,
  GetPaymentStatusInput,
  GetPaymentStatusOutput,
  InitiatePaymentOutput,
  ProviderWebhookPayload,
  WebhookActionResult,
} from "@medusajs/framework/types"
import { PaymentActions, PaymentSessionStatus } from "@medusajs/framework/utils"
import { buildError, extractValue } from "../../utils"
import { logger } from "../../utils/logger"
import { InitiateConnectorContext } from "../types"
import {
  WebhookDeps,
  WebhookOutcome,
  callHandleEvent,
  getWebhookOutcome,
  isRefundEvent,
  isSettlementReference,
  isSourceVerified,
  logRefundEvent,
  logSettlementEvent,
  mapPaymentEventToAction,
  parsePaymentReference,
  recordWebhookOutcome,
} from "../webhook-common"

export async function initiatePayment({
  options,
  merchantClientSessionId,
  currencyCode,
  minorAmount,
  sessionData,
  connectorSpecific,
}: InitiateConnectorContext): Promise<InitiatePaymentOutput> {
  const connectorData = connectorSpecific.adyen ?? connectorSpecific
  const clientToken = extractValue(
    connectorData?.clientToken ?? connectorData?.client_token ?? connectorData?.sessionId ?? connectorData?.session_id
  )
  const publishableKey =
    (options as any).adyenClientKey ||
    extractValue((options.connectorConfig as any)?.publishableKey) ||
    extractValue(connectorData?.publishableKey ?? connectorData?.publishable_key)

  const adyenSessionId = clientToken || merchantClientSessionId

  return {
    id: adyenSessionId,
    data: {
      id: adyenSessionId,
      clientToken,
      publishableKey,
      currency: currencyCode,
      minorAmount,
      connector: "adyen",
      merchantClientSessionId,
      sessionData,
    },
    status: PaymentSessionStatus.PENDING,
  }
}

export type AdyenAuthorizeDeps = {
  getPaymentStatus: (
    input: GetPaymentStatusInput
  ) => Promise<GetPaymentStatusOutput>
}

// The retry window must cover Medusa's webhook subscriber delay (5s by
// default) on top of Adyen's own webhook latency.
const AUTHORIZE_MAX_ATTEMPTS = 8
const AUTHORIZE_RETRY_DELAY_MS = 1500

const WEBHOOK_ACTION_TO_SESSION_STATUS: Record<string, PaymentSessionStatus> = {
  [PaymentActions.AUTHORIZED]: PaymentSessionStatus.AUTHORIZED,
  [PaymentActions.SUCCESSFUL]: PaymentSessionStatus.CAPTURED,
  [PaymentActions.FAILED]: PaymentSessionStatus.ERROR,
  [PaymentActions.CANCELED]: PaymentSessionStatus.CANCELED,
}

// Adopt the webhook's pspReference as the transaction id (`data.id`) — it is
// the reference UCS needs for capture/refund, and it only ever reaches the
// server via the webhook.
function buildOutcomeResult(
  input: AuthorizePaymentInput,
  outcome: WebhookOutcome
): AuthorizePaymentOutput {
  const pspReference = outcome.connectorTransactionId
  return {
    data: {
      ...(input.data as Record<string, unknown>),
      ...(pspReference
        ? { id: pspReference, connectorTransactionId: pspReference }
        : {}),
    },
    status:
      WEBHOOK_ACTION_TO_SESSION_STATUS[outcome.action] ??
      PaymentSessionStatus.PENDING,
  }
}

export async function authorizePayment(
  input: AuthorizePaymentInput,
  deps: AdyenAuthorizeDeps
): Promise<AuthorizePaymentOutput> {
  const sessionId = (input.data as any)?.merchantClientSessionId as
    | string
    | undefined

  try {
    // Adyen's sessions flow authorises client-side via the drop-in and the
    // outcome arrives by webhook only — UCS cannot PSync these payments (its
    // Adyen PSync requires redirect encoded_data). The verified webhook
    // outcome store is the source of truth; poll it while the webhook is in
    // flight. getPaymentStatus is still tried for flows where PSync works.
    let result: AuthorizePaymentOutput | undefined
    for (let attempt = 0; attempt < AUTHORIZE_MAX_ATTEMPTS; attempt++) {
      const outcome = sessionId ? getWebhookOutcome(sessionId) : undefined
      if (outcome) {
        return buildOutcomeResult(input, outcome)
      }

      result = (await deps.getPaymentStatus(input)) as AuthorizePaymentOutput
      if (result.status === PaymentSessionStatus.AUTHORIZED) {
        return result
      }
      if (attempt < AUTHORIZE_MAX_ATTEMPTS - 1) {
        await new Promise((r) => setTimeout(r, AUTHORIZE_RETRY_DELAY_MS))
      }
    }

    return {
      data: result!.data,
      status: result!.status,
    }
  } catch (error) {
    logger.error("[adyen.authorizePayment] ERROR: %s", (error as Error).message)
    return {
      data: input.data,
      status: PaymentSessionStatus.PENDING,
    }
  }
}

// Adyen status-fetch errors are treated as transient: callers should return
// PENDING instead of failing the payment session.
export function isTransientStatusError(error: unknown): boolean {
  return error instanceof ConnectorError || error instanceof IntegrationError
}

/**
 * Adyen webhooks are verified by UCS via HMAC-SHA256: the hex HMAC key from
 * the Customer Area is passed as the webhook secret and compared against the
 * base64 hmacSignature embedded in the notification. The key is mandatory —
 * unverifiable events are never processed, in dev or prod.
 */
export async function handleWebhook(
  webhookData: ProviderWebhookPayload["payload"],
  { options, eventClient }: WebhookDeps
): Promise<WebhookActionResult> {
  if (!options.webhookSecret) {
    logger.error(
      "[adyen.handleWebhook] webhookSecret (hex HMAC key from the Adyen Customer Area) is required — event rejected"
    )
    return { action: PaymentActions.NOT_SUPPORTED }
  }

  try {
    const res = await callHandleEvent(
      eventClient,
      webhookData,
      options.webhookSecret
    )
    if (!isSourceVerified(res, "adyen")) {
      return { action: PaymentActions.NOT_SUPPORTED }
    }

    const eventType = (res as any).eventType as number | undefined
    if (isRefundEvent(eventType)) {
      // Adyen refunds are asynchronous — this webhook is the only settlement
      // confirmation, so it must be visible in logs.
      return logRefundEvent(res, "adyen")
    }

    // Adyen's HandleEvent response only carries the pspReference; the
    // merchantReference (Medusa's session id) is exposed via ParseEvent.
    const reference = await parsePaymentReference(
      eventClient,
      webhookData,
      "adyen"
    )

    // Adyen echoes the capture/refund reference back in settlement webhooks —
    // those are confirmations of operations Medusa already recorded, not
    // payment-session events.
    if (isSettlementReference(reference.merchantTransactionId)) {
      return logSettlementEvent(res, reference, "adyen")
    }

    const result = mapPaymentEventToAction(res, reference)
    // The authorize step cannot sync this outcome from UCS — persist it for
    // authorizePayment to pick up when Medusa processes the event.
    recordWebhookOutcome(result)
    return result
  } catch (error) {
    logger.error(
      "[adyen.handleWebhook] ERROR: %s",
      (error as Error).message
    )
    throw buildError("An error occurred in getWebhookActionAndData", error)
  }
}
