import { EventClient, types } from "hyperswitch-prism"
import {
  ProviderWebhookPayload,
  WebhookActionResult,
} from "@medusajs/framework/types"
import { PaymentActions, isDefined } from "@medusajs/framework/utils"
import { HyperswitchPrismOptions } from "../types"
import { fromMinorAmount } from "../utils"
import { logger } from "../utils/logger"

export type WebhookDeps = {
  options: HyperswitchPrismOptions
  eventClient: EventClient
}

function buildRequestDetails(
  webhookData: ProviderWebhookPayload["payload"]
): types.IRequestDetails {
  const rawBody =
    typeof webhookData.rawData === "string"
      ? Buffer.from(webhookData.rawData)
      : (webhookData.rawData as Buffer)

  const lowercasedHeaders = Object.fromEntries(
    Object.entries(webhookData.headers).map(([k, v]) => [k.toLowerCase(), v])
  ) as { [k: string]: string }

  return {
    method: types.HttpMethod.HTTP_METHOD_POST,
    headers: lowercasedHeaders,
    body: rawBody,
  }
}

export async function callHandleEvent(
  eventClient: EventClient,
  webhookData: ProviderWebhookPayload["payload"],
  webhookSecret: string
): Promise<unknown> {
  return eventClient.handleEvent({
    requestDetails: buildRequestDetails(webhookData),
    webhookSecrets: { secret: webhookSecret },
  })
}

export type PaymentEventReference = {
  merchantTransactionId?: string
  connectorTransactionId?: string
}

// Phase 1 of UCS's two-phase webhook flow: ParseEvent extracts the resource
// reference without credentials. HandleEvent's paymentsResponse does not carry
// the merchant reference for some connectors (e.g. Adyen returns only the
// pspReference), but ParseEvent does — and Medusa needs it as session_id to
// correlate the event to a payment session. Failures are non-fatal: callers
// fall back to the ids in the HandleEvent response.
export async function parsePaymentReference(
  eventClient: EventClient,
  webhookData: ProviderWebhookPayload["payload"],
  connector: string
): Promise<PaymentEventReference> {
  try {
    const res = await eventClient.parseEvent({
      requestDetails: buildRequestDetails(webhookData),
    })
    const payment = (res as any)?.reference?.payment as
      | Record<string, any>
      | undefined
    return {
      merchantTransactionId: payment?.merchantTransactionId ?? undefined,
      connectorTransactionId: payment?.connectorTransactionId ?? undefined,
    }
  } catch (error) {
    logger.error(
      "[webhook-common.parsePaymentReference] ParseEvent failed (connector=%s): %s",
      connector,
      (error as Error).message
    )
    return {}
  }
}

// Mandatory verification gate — applies in dev and prod alike, no bypass.
// A webhook whose source cannot be verified must never drive payment state.
export function isSourceVerified(res: unknown, connector: string): boolean {
  const verified = (res as any).sourceVerified as boolean | undefined
  if (verified === false) {
    return false
  }
  return true
}

function buildPaymentActionData(
  res: unknown,
  reference?: PaymentEventReference
): {
  session_id: string
  amount: number
  connector_transaction_id?: string
} {
  const paymentsResponse = (res as any).eventContent?.paymentsResponse as
    | Record<string, any>
    | undefined

  // Prefer the merchant reference from ParseEvent — it is the id Medusa knows
  // the payment session by; HandleEvent's ids may both be connector-side.
  const sessionId =
    reference?.merchantTransactionId ??
    (paymentsResponse?.merchantTransactionId as string | undefined) ??
    (paymentsResponse?.connectorTransactionId as string | undefined) ??
    ""

  const rawAmount = paymentsResponse?.amount?.minorAmount as number | undefined
  const currency = paymentsResponse?.amount?.currency as string | undefined
  const amount =
    rawAmount !== undefined && currency
      ? fromMinorAmount(rawAmount, currency.toLowerCase())
      : 0

  // The connector transaction reference (e.g. Adyen's pspReference) often
  // only reaches the server via webhooks — surface it so callers can
  // persist it against the session for later capture/refund calls.
  const connectorTransactionId =
    (paymentsResponse?.connectorTransactionId as string | undefined) ??
    reference?.connectorTransactionId

  return {
    session_id: sessionId,
    amount,
    ...(connectorTransactionId
      ? { connector_transaction_id: connectorTransactionId }
      : {}),
  }
}

// UCS cannot PSync some connectors (e.g. Adyen sessions-flow payments require
// redirect encoded_data), so the verified webhook is the only source of truth
// for the payment outcome. Medusa does not persist webhook data onto the
// session before re-invoking authorizePayment, so outcomes are kept here for
// authorizePayment to consult. Single-process only: in a multi-instance
// deployment the webhook and the authorize call may land on different
// processes and this store will miss.
export type WebhookOutcome = {
  action: string
  connectorTransactionId?: string
  recordedAt: number
}

const WEBHOOK_OUTCOME_TTL_MS = 60 * 60 * 1000
const webhookOutcomes = new Map<string, WebhookOutcome>()

function pruneWebhookOutcomes(now: number): void {
  for (const [key, outcome] of webhookOutcomes) {
    if (now - outcome.recordedAt > WEBHOOK_OUTCOME_TTL_MS) {
      webhookOutcomes.delete(key)
    }
  }
}

const RECORDABLE_ACTIONS = new Set<string>([
  PaymentActions.AUTHORIZED,
  PaymentActions.SUCCESSFUL,
  PaymentActions.FAILED,
  PaymentActions.CANCELED,
])

export function recordWebhookOutcome(result: WebhookActionResult): void {
  const sessionId = result.data?.session_id
  if (!sessionId || !RECORDABLE_ACTIONS.has(result.action)) {
    return
  }
  const now = Date.now()
  pruneWebhookOutcomes(now)
  webhookOutcomes.set(sessionId, {
    action: result.action,
    connectorTransactionId: (result.data as any)?.connector_transaction_id,
    recordedAt: now,
  })
}

export function getWebhookOutcome(
  sessionId: string
): WebhookOutcome | undefined {
  const outcome = webhookOutcomes.get(sessionId)
  if (!outcome) {
    return undefined
  }
  if (Date.now() - outcome.recordedAt > WEBHOOK_OUTCOME_TTL_MS) {
    webhookOutcomes.delete(sessionId)
    return undefined
  }
  return outcome
}

export function mapPaymentEventToAction(
  res: unknown,
  reference?: PaymentEventReference
): WebhookActionResult {
  const eventType = (res as any).eventType as number | undefined
  if (!isDefined(eventType)) {
    return { action: PaymentActions.NOT_SUPPORTED }
  }

  const actionData = buildPaymentActionData(res, reference)
  const { WebhookEventType } = types

  switch (eventType) {
    case WebhookEventType.PAYMENT_INTENT_AUTHORIZATION_SUCCESS:
      return { action: PaymentActions.AUTHORIZED, data: actionData }
    case WebhookEventType.PAYMENT_INTENT_SUCCESS:
    case WebhookEventType.PAYMENT_INTENT_CAPTURE_SUCCESS:
      return { action: PaymentActions.SUCCESSFUL, data: actionData }
    case WebhookEventType.PAYMENT_ACTION_REQUIRED:
      return { action: PaymentActions.REQUIRES_MORE, data: actionData }
    case WebhookEventType.PAYMENT_INTENT_PROCESSING:
      return { action: PaymentActions.PENDING, data: actionData }
    case WebhookEventType.PAYMENT_INTENT_CANCELLED:
      return { action: PaymentActions.CANCELED, data: actionData }
    case WebhookEventType.PAYMENT_INTENT_FAILURE:
    case WebhookEventType.PAYMENT_INTENT_AUTHORIZATION_FAILURE:
    case WebhookEventType.PAYMENT_INTENT_CAPTURE_FAILURE:
      return { action: PaymentActions.FAILED, data: actionData }
    default:
      return { action: PaymentActions.NOT_SUPPORTED }
  }
}

// Capture/refund calls send their own merchant reference (Medusa's
// idempotency key or the plugin's `capt_`/`ref_` fallback), and the connector
// echoes it back in the settlement webhook. Such a reference is not a payment
// session id — feeding it to Medusa as session_id makes processPaymentWorkflow
// try to authorize a session that doesn't exist.
const SETTLEMENT_REFERENCE_PREFIXES = ["capt_", "ref_"]

export function isSettlementReference(reference?: string): boolean {
  return (
    !!reference &&
    SETTLEMENT_REFERENCE_PREFIXES.some((prefix) => reference.startsWith(prefix))
  )
}

// Settlement confirmations (e.g. Adyen's CAPTURE webhook after an async
// capture) are acknowledged and logged — the log is the deliverable.
export function logSettlementEvent(
  res: unknown,
  reference: PaymentEventReference,
  connector: string
): WebhookActionResult {
  return { action: PaymentActions.NOT_SUPPORTED }
}

export function isRefundEvent(eventType: number | undefined): boolean {
  if (eventType === undefined) return false
  return (
    eventType === types.WebhookEventType.WEBHOOK_REFUND_SUCCESS ||
    eventType === types.WebhookEventType.WEBHOOK_REFUND_FAILURE
  )
}

export function isDisputeEvent(eventType: number | undefined): boolean {
  const { WebhookEventType } = types
  return (
    eventType !== undefined &&
    eventType >= WebhookEventType.WEBHOOK_DISPUTE_OPENED &&
    eventType <= WebhookEventType.WEBHOOK_DISPUTE_LOST
  )
}

// Medusa's PaymentActions has no refund action — the log is the deliverable
// (e.g. for Adyen the REFUND webhook is the only settlement confirmation).
export function logRefundEvent(
  res: unknown,
  connector: string
): WebhookActionResult {
  return { action: PaymentActions.NOT_SUPPORTED }
}

export function logDisputeEvent(
  res: unknown,
  connector: string
): WebhookActionResult {
  return { action: PaymentActions.NOT_SUPPORTED }
}
