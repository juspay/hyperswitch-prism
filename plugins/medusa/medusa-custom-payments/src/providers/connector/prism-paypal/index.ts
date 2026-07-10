import {
  PaymentClient,
  MerchantAuthenticationClient,
  types,
} from "hyperswitch-prism"
import {
  AuthorizePaymentInput,
  AuthorizePaymentOutput,
  InitiatePaymentOutput,
  ProviderWebhookPayload,
  RefundPaymentInput,
  RefundPaymentOutput,
  WebhookActionResult,
} from "@medusajs/framework/types"
import { PaymentActions, PaymentSessionStatus } from "@medusajs/framework/utils"
import { HyperswitchPrismOptions } from "../../types"
import {
  buildError,
  extractValue,
  fromMinorAmount,
  mapPrismStatus,
  normalizeAmount,
  toCurrency,
  toMinorAmount,
} from "../../utils"
import { logger } from "../../utils/logger"
import { InitiateConnectorContext } from "../types"
import {
  WebhookDeps,
  callHandleEvent,
  isDisputeEvent,
  isRefundEvent,
  isSourceVerified,
  logDisputeEvent,
  logRefundEvent,
  mapPaymentEventToAction,
  parsePaymentReference,
} from "../webhook-common"

export type PaypalDeps = {
  options: HyperswitchPrismOptions
  paymentClient: PaymentClient
  authClient: MerchantAuthenticationClient
}

function getPaypalApiBase(options: HyperswitchPrismOptions): string {
  return options.environment === "PRODUCTION"
    ? "https://api-m.paypal.com"
    : "https://api-m.sandbox.paypal.com"
}

// PayPal's orders API takes the amount as a decimal string in major units
// (e.g. "10.00"), with no decimals for zero-decimal currencies like JPY.
function toPaypalAmountValue(minorAmount: number, currencyCode: string): string {
  const majorAmount = fromMinorAmount(minorAmount, currencyCode)
  const isZeroDecimal = toMinorAmount(1, currencyCode) === 1
  return majorAmount.toFixed(isZeroDecimal ? 0 : 2)
}

// Fetches a PayPal OAuth bearer token via the Prism connector service
// (createServerAuthenticationToken runs the client_credentials grant against
// /v1/oauth2/token with the configured connector credentials).
async function fetchPaypalAccessToken(
  authClient: MerchantAuthenticationClient,
  testMode: boolean
): Promise<string> {
  const res = await authClient.createServerAuthenticationToken({
    merchantAccessTokenId: `pp_auth_${Date.now()}`,
    connector: types.Connector.PAYPAL,
    testMode,
  })

  const statusCode = (res as any).statusCode as number | undefined
  const token = extractValue((res as any).accessToken)
  if (
    !token ||
    (statusCode !== undefined && (statusCode < 200 || statusCode >= 300))
  ) {
    throw new Error(
      (res as any).error?.message ??
      "PayPal createServerAuthenticationToken failed"
    )
  }
  return token
}

// Creates the order against PayPal's /v2/checkout/orders API directly
// (instead of going through the Prism createOrder flow) and returns the
// PayPal order id.
async function createPaypalOrderDirect(
  options: HyperswitchPrismOptions,
  authClient: MerchantAuthenticationClient,
  merchantClientSessionId: string,
  currencyCode: string,
  minorAmount: number
): Promise<string> {
  const testMode = options.environment !== "PRODUCTION"
  const accessToken = await fetchPaypalAccessToken(authClient, testMode)

  // Only collect a shipping address in the PayPal popup when the integration
  // opts into it. Without a shipping_preference PayPal defaults to
  // GET_FROM_FILE and shows the address step; NO_SHIPPING removes it. (Mirrors
  // @alphabite/medusa-paypal, which only sends shipping data when its
  // includeShippingData flag is enabled.)
  const includeShippingData =
    (options.connectorConfig as any)?.includeShippingData === true

  // The PayPal-Request-Id must differ from the one Prism sends on the later
  // /capture call (the merchantClientSessionId) — PayPal's idempotency layer
  // replays the create-order response for a duplicate id, which breaks the
  // capture response handling.
  const res = await fetch(`${getPaypalApiBase(options)}/v2/checkout/orders`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${accessToken}`,
      "Content-Type": "application/json",
      "PayPal-Request-Id": `${merchantClientSessionId}_order`,
    },
    body: JSON.stringify({
      intent: "CAPTURE",
      purchase_units: [
        {
          reference_id: merchantClientSessionId,
          invoice_id: merchantClientSessionId,
          amount: {
            currency_code: currencyCode.toUpperCase(),
            value: toPaypalAmountValue(minorAmount, currencyCode),
          },
        },
      ],
      application_context: {
        shipping_preference: includeShippingData ? "GET_FROM_FILE" : "NO_SHIPPING",
        user_action: "PAY_NOW",
      },
    }),
  })

  const body = (await res.json().catch(() => ({}))) as any
  if (!res.ok || !body.id) {
    throw new Error(
      body.message ?? `PayPal create order failed (${res.status})`
    )
  }

  return body.id as string
}

// Refunds run against the capture id, but callers often only have the order
// id. Resolve it via the direct orders API; returns null when not found.
async function fetchCaptureIdForOrder(
  options: HyperswitchPrismOptions,
  authClient: MerchantAuthenticationClient,
  orderId: string
): Promise<string | null> {
  try {
    const testMode = options.environment !== "PRODUCTION"
    const accessToken = await fetchPaypalAccessToken(authClient, testMode)
    const res = await fetch(
      `${getPaypalApiBase(options)}/v2/checkout/orders/${orderId}`,
      { headers: { Authorization: `Bearer ${accessToken}` } }
    )
    if (!res.ok) return null
    const body = (await res.json().catch(() => ({}))) as any
    return body?.purchase_units?.[0]?.payments?.captures?.[0]?.id ?? null
  } catch (error) {
    logger.error(
      "[PrismService.fetchCaptureIdForOrder] paypal: capture id lookup failed: %s",
      (error as Error).message
    )
    return null
  }
}

// The PaypalMeta blob (connectorFeatureData) carries the capture id produced
// by the order capture.
function extractCaptureMeta(res: unknown): { captureId: string | null; authorizeId: string | null } {
  try {
    const raw = (res as any)?.connectorFeatureData?.value
    const meta = raw ? JSON.parse(raw) : {}
    return {
      captureId: meta?.capture_id ?? null,
      authorizeId: meta?.authorize_id ?? null,
    }
  } catch (error) {
    logger.error(
      "[PrismService.extractCaptureMeta] paypal: failed to parse capture meta: %s",
      (error as Error).message
    )
    return { captureId: null, authorizeId: null }
  }
}

// PayPal flows require a connector access token; fetch one and shape it as
// the ConnectorState the payment requests expect.
async function fetchConnectorState(
  authClient: MerchantAuthenticationClient,
  testMode: boolean
): Promise<types.IConnectorState> {
  const token = await fetchPaypalAccessToken(authClient, testMode)
  return {
    accessToken: {
      token: { value: token },
      tokenType: "Bearer",
    },
  }
}

function toSecret(raw: unknown): types.ISecretString | undefined {
  return typeof raw === "string" && raw ? { value: raw } : undefined
}

// Maps a Medusa-style (snake_case) or camelCase address blob to the SDK shape.
function toSdkAddress(addr: any): types.IAddress | undefined {
  if (!addr) return undefined

  const countryStr = (
    addr.countryAlpha2Code ?? addr.country_code ?? addr.country
  )?.toString().toUpperCase()
  const countryAlpha2Code = countryStr
    ? types.CountryAlpha2[countryStr as keyof typeof types.CountryAlpha2]
    : undefined

  const sdkAddress: types.IAddress = {
    firstName: toSecret(addr.firstName ?? addr.first_name),
    lastName: toSecret(addr.lastName ?? addr.last_name),
    line1: toSecret(addr.line1 ?? addr.address_1),
    line2: toSecret(addr.line2 ?? addr.address_2),
    city: toSecret(addr.city),
    state: toSecret(addr.state ?? addr.province),
    zipCode: toSecret(addr.zipCode ?? addr.postal_code),
    ...(countryAlpha2Code !== undefined ? { countryAlpha2Code } : {}),
    email: toSecret(addr.email),
    phoneNumber: toSecret(addr.phoneNumber ?? addr.phone),
  }

  const hasValue = Object.values(sdkAddress).some((v) => v !== undefined)
  return hasValue ? sdkAddress : undefined
}

function toSdkCustomer(customer: any): types.ICustomer | undefined {
  if (!customer) return undefined

  const name =
    customer.name ??
    [customer.first_name, customer.last_name].filter(Boolean).join(" ")
  const email = toSecret(customer.email)
  const phoneNumber = toSecret(customer.phoneNumber ?? customer.phone)

  if (!name && !email && !phoneNumber) return undefined
  return {
    ...(name ? { name } : {}),
    ...(email ? { email } : {}),
    ...(phoneNumber ? { phoneNumber } : {}),
  }
}

export async function initiatePayment({
  options,
  merchantClientSessionId,
  currencyCode,
  minorAmount,
  sessionData,
  connectorSpecific,
  authClient,
}: InitiateConnectorContext): Promise<InitiatePaymentOutput> {
  const connectorData =
    sessionData?.paypal ??
    connectorSpecific?.paypal ??
    connectorSpecific
  const clientToken = extractValue(
    connectorData?.clientToken ?? connectorData?.client_token
  )
  const sessionToken = extractValue(
    connectorData?.sessionToken ?? connectorData?.session_token
  )

  const configClientId = extractValue(
    (options.connectorConfig as any)?.clientId ??
    (options.connectorConfig as any)?.client_id
  )

  // Create the PayPal order via the direct checkout/orders API so it can be
  // captured in authorizePayment
  let paypalOrderId: string | null = null
  try {
    paypalOrderId = await createPaypalOrderDirect(
      options,
      authClient,
      merchantClientSessionId,
      currencyCode,
      minorAmount
    )
  } catch (orderError) {
    logger.error("[PrismService.initiatePayment.paypal] createOrder failed: %s", (orderError as Error).message)
  }

  return {
    id: merchantClientSessionId,
    data: {
      id: merchantClientSessionId,
      paypalOrderId,
      clientToken,
      sessionToken,
      paypalClientId: configClientId,
      currency: currencyCode,
      minorAmount,
      connector: "paypal",
      merchantClientSessionId,
      sessionData,
      environment: options.environment ?? "SANDBOX",
    },
    status: PaymentSessionStatus.PENDING,
  }
}

// A session that was only initiated (a PayPal order was created but never
// authorized) has a `paypalOrderId` but no real `connectorTransactionId`, so
// there is nothing to void when it is deleted (e.g. the shopper switches payment
// methods).
export function shouldSkipVoid(data: Record<string, unknown> | undefined): boolean {
  return !(data as any)?.connectorTransactionId && Boolean((data as any)?.paypalOrderId)
}

export async function authorizePayment(
  input: AuthorizePaymentInput,
  { options, paymentClient, authClient }: PaypalDeps
): Promise<AuthorizePaymentOutput> {
  const data = input.data as any
  const orderId = (data?.paypalOrderId ?? data?.id) as string

  if (!orderId) {
    logger.error("[PrismService.authorizePayment] paypal: missing order id")
    return { data: input.data, status: PaymentSessionStatus.ERROR }
  }

  const minorAmount = data?.minorAmount as number | undefined
  const currency = data?.currency as string | undefined

  try {
    const testMode = options.environment !== "PRODUCTION"
    const state = await fetchConnectorState(authClient, testMode)

    const countryCodeStr = (data?.country as string) ?? "US"
    const countryAlpha2Code =
      types.CountryAlpha2[countryCodeStr as keyof typeof types.CountryAlpha2] ??
      types.CountryAlpha2.US

    const config = options.connectorConfig as any
    // When shipping data is sent PayPal uses SET_PROVIDED_ADDRESS, otherwise
    // it collects the address itself (GET_FROM_FILE).
    const shippingAddress =
      config?.includeShippingData === true
        ? toSdkAddress(
          data?.shippingAddress ??
          data?.shipping_address ??
          (input.context as any)?.shipping_address
        )
        : undefined
    const customer =
      config?.includeCustomerData === true
        ? toSdkCustomer(data?.customer ?? (input.context as any)?.customer)
        : undefined

    // paypalSdk (not paypalRedirect): the buyer has already approved the order
    // via the PayPal JS SDK, so the connector must capture it with a bodiless
    // request. paypalRedirect sends a new payment_source, which makes PayPal
    // restart approval and answer PAYER_ACTION_REQUIRED.
    const res = await paymentClient.authorize({
      merchantTransactionId:
        (data?.merchantClientSessionId as string) ?? data?.id ?? `pp_${Date.now()}`,
      amount: {
        minorAmount: minorAmount ?? 0,
        currency: toCurrency(currency || "USD"),
      },
      paymentMethod: { paypalSdk: { token: { value: orderId } } },
      captureMethod: types.CaptureMethod.AUTOMATIC,
      connectorOrderId: orderId,
      address: {
        billingAddress: {
          countryAlpha2Code,
        },
        ...(shippingAddress ? { shippingAddress } : {}),
      },
      ...(customer ? { customer } : {}),
      state,
      testMode,
    })

    const rawStatus =
      (res as any).status ?? types.PaymentStatus.PAYMENT_STATUS_UNSPECIFIED
    const mappedStatus = mapPrismStatus(rawStatus)
    const { captureId, authorizeId } = extractCaptureMeta(res)

    return {
      data: {
        ...(data as any),
        connectorTransactionId: (res as any).connectorTransactionId,
        orderId,
        captureId,
        authorizeId,
        prismStatus: rawStatus,
        paypalCapture: res,
      },
      status: mappedStatus,
    }
  } catch (error) {
    logger.error("[PrismService.authorizePayment] paypal ERROR: %s", (error as Error).message)
    throw buildError("An error occurred in authorizePayment", error)
  }
}

/**
 * PayPal refunds hit /v2/payments/captures/{capture_id}/refund. The connector
 * reads the capture id from connector_meta_data (the PaypalMeta blob produced
 * by the capture/psync flows), not from connectorTransactionId, and its
 * headers need a connector access token — so both must be supplied here.
 */
export async function refundPayment(
  { data, amount, context }: RefundPaymentInput,
  { options, paymentClient, authClient }: PaypalDeps
): Promise<RefundPaymentOutput> {
  const d = data as any
  const currency = d?.currency as string

  // Prefer an explicit capture id; otherwise treat the supplied id as an
  // order id and resolve its capture id via the direct orders API.
  let captureId = d?.captureId as string | undefined
  if (!captureId) {
    const candidate = (d?.orderId ?? d?.paypalOrderId ?? d?.id) as
      | string
      | undefined
    if (candidate) {
      captureId =
        (await fetchCaptureIdForOrder(options, authClient, candidate)) ??
        candidate
    }
  }

  if (!captureId) {
    logger.error("[PrismService.refundPayment] paypal: missing capture id")
    throw buildError(
      "An error occurred in refundPayment",
      new Error("Missing PayPal capture id for refund")
    )
  }

  try {
    const testMode = options.environment !== "PRODUCTION"
    const state = await fetchConnectorState(authClient, testMode)

    // Shape must match the connector's PaypalMeta struct.
    const paypalMeta = {
      authorize_id: d?.authorizeId ?? null,
      capture_id: captureId,
      incremental_authorization_id: null,
      psync_flow: "CAPTURE",
      next_action: null,
      order_id: d?.orderId ?? d?.paypalOrderId ?? null,
    }

    const res = await paymentClient.refund({
      merchantRefundId:
        (context?.idempotency_key as string) ?? `ref_${Date.now()}`,
      connectorTransactionId: captureId,
      refundAmount: {
        minorAmount: toMinorAmount(normalizeAmount(amount), currency),
        currency: toCurrency(currency),
      },
      connectorFeatureData: { value: JSON.stringify(paypalMeta) },
      paymentMethodType: types.PaymentMethodType.PAY_PAL,
      state,
      testMode,
    })

    const refundStatus = (res as any).status as number | undefined
    if (
      refundStatus === types.RefundStatus.REFUND_FAILURE ||
      refundStatus === types.RefundStatus.REFUND_TRANSACTION_FAILURE
    ) {
      throw new Error(
        (res as any).error?.message ?? "Refund failed at connector"
      )
    }

    return { data: { ...(d as any), refundStatus, raw: res } }
  } catch (error) {
    logger.error("[PrismService.refundPayment] paypal ERROR: %s", (error as Error).message)
    throw buildError("An error occurred in refundPayment", error)
  }
}

/**
 * PayPal webhooks are verified by UCS via an outbound call to PayPal's
 * /v1/notifications/verify-webhook-signature using the connector's Basic Auth
 * credentials — no access token is needed here. The webhook secret must be
 * the PayPal **webhook ID** from the developer dashboard, and it is
 * mandatory: unverifiable events are never processed, in dev or prod.
 */
export async function handleWebhook(
  webhookData: ProviderWebhookPayload["payload"],
  { options, eventClient }: WebhookDeps
): Promise<WebhookActionResult> {
  if (!options.webhookSecret) {
    logger.error(
      "[paypal.handleWebhook] webhookSecret (PayPal webhook ID from the developer dashboard) is required — event rejected"
    )
    return { action: PaymentActions.NOT_SUPPORTED }
  }

  try {
    const res = await callHandleEvent(
      eventClient,
      webhookData,
      options.webhookSecret
    )
    if (!isSourceVerified(res, "paypal")) {
      return { action: PaymentActions.NOT_SUPPORTED }
    }

    const eventType = (res as any).eventType as number | undefined
    if (isRefundEvent(eventType)) {
      return logRefundEvent(res, "paypal")
    }
    if (isDisputeEvent(eventType)) {
      return logDisputeEvent(res, "paypal")
    }

    // Prefer the merchant reference from ParseEvent as the session id —
    // HandleEvent's payment ids may both be connector-side references.
    const reference = await parsePaymentReference(
      eventClient,
      webhookData,
      "paypal"
    )
    return mapPaymentEventToAction(res, reference)
  } catch (error) {
    logger.error(
      "[paypal.handleWebhook] ERROR: %s",
      (error as Error).message
    )
    throw buildError("An error occurred in getWebhookActionAndData", error)
  }
}
