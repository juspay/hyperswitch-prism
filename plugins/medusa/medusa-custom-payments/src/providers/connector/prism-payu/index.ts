import { PaymentClient, types } from "hyperswitch-prism"
import {
  AuthorizePaymentInput,
  AuthorizePaymentOutput,
  InitiatePaymentInput,
  InitiatePaymentOutput,
} from "@medusajs/framework/types"
import { PaymentSessionStatus } from "@medusajs/framework/utils"
import { buildError, extractValue, mapPrismStatus, toCurrency } from "../../utils"
import { logger } from "../../utils/logger"
import { HyperswitchPrismOptions } from "../../types"
import { InitiateConnectorContext } from "../types"

// PayU is an India-first redirect/UPI processor. It exposes the SDK session
// token (an OAuth2 access token) via the ServerSessionAuthenticationToken flow,
// surfaced here in `sessionData.connectorSpecific.payu`. The storefront collects
// a UPI VPA (or chooses a hosted-redirect method) and `authorizePayment` builds
// the actual UPI/redirect payment, returning the next action as data.redirectUrl
// for the storefront to follow (UPI intent deep-link / hosted PayU page).
export async function initiatePayment({
  merchantClientSessionId,
  currencyCode,
  minorAmount,
  sessionData,
  connectorSpecific,
}: InitiateConnectorContext): Promise<InitiatePaymentOutput> {
  const connectorData = connectorSpecific?.payu ?? connectorSpecific
  // OAuth2 access token from PayU's SDKSessionToken flow (not a secret the
  // storefront can misuse on its own — it is a short-lived merchant token).
  const sessionToken = extractValue(
    connectorData?.sessionToken ??
      connectorData?.session_token ??
      connectorData?.accessToken ??
      connectorData?.access_token
  )

  return {
    id: merchantClientSessionId,
    data: {
      id: merchantClientSessionId,
      ...(sessionToken ? { sessionToken } : {}),
      currency: currencyCode,
      minorAmount,
      connector: "payu",
      merchantClientSessionId,
      sessionData,
    },
    status: PaymentSessionStatus.PENDING,
  }
}

// PayU never obtains a real connector transaction id until `authorizePayment`
// runs (it adopts the returned reference as `data.id`). When a session is only
// initiated and then deleted (e.g. the shopper switches payment methods), there
// is nothing to void — skip it instead of letting the connector reject the
// Medusa session id. Mirrors prism-mollie.shouldSkipVoid.
export function shouldSkipVoid(data: Record<string, unknown> | undefined): boolean {
  const d = data as any
  return !d?.id || d.id === d?.merchantClientSessionId
}

export type PayuReInitiateContext = {
  data: InitiatePaymentInput["data"]
  merchantClientSessionId: string
  currencyCode: string
  minorAmount: number
}

// Re-initiation: persist the chosen UPI VPA / payment-method + billing the
// storefront collected onto the session — no network call. Consumed by
// `authorizePayment`. Mirrors prism-mollie.reInitiatePayment.
export async function reInitiatePayment({
  data,
  merchantClientSessionId,
  currencyCode,
  minorAmount,
}: PayuReInitiateContext): Promise<InitiatePaymentOutput> {
  const d = data as any
  return {
    id: d?.id ?? merchantClientSessionId,
    data: {
      ...d,
      connector: "payu",
      minorAmount,
      currency: currencyCode,
    },
    status: PaymentSessionStatus.PENDING,
  }
}

// Billing PayU requires on the payment request (firstname / email / phone are
// mandatory; PayU rejects the order otherwise).
export type PayuBilling = {
  firstName?: string
  lastName?: string
  email?: string
  phone?: string
  line1?: string
  city?: string
  postalCode?: string
  country?: string
}

function toBillingAddress(b: PayuBilling): types.IAddress {
  const sec = (v?: string): types.ISecretString | undefined =>
    v ? { value: v } : undefined
  let countryAlpha2Code: types.CountryAlpha2 | undefined = types.CountryAlpha2.IN
  if (b.country) {
    const code = (
      types.CountryAlpha2 as unknown as Record<string, types.CountryAlpha2 | undefined>
    )[b.country.toUpperCase()]
    if (code !== undefined) countryAlpha2Code = code
  }
  return {
    firstName: sec(b.firstName),
    ...(b.lastName ? { lastName: sec(b.lastName) } : {}),
    ...(b.line1 ? { line1: sec(b.line1) } : {}),
    ...(b.city ? { city: sec(b.city) } : {}),
    ...(b.postalCode ? { zipCode: sec(b.postalCode) } : {}),
    ...(countryAlpha2Code !== undefined ? { countryAlpha2Code } : {}),
    email: sec(b.email),
    ...(b.phone ? { phoneNumber: sec(b.phone), phoneCountryCode: "+91" } : {}),
  }
}

export type PayuAuthorizeDeps = {
  options: HyperswitchPrismOptions
  paymentClient: PaymentClient
  // Status-sync (PSync) used on the post-redirect retry — see authorizePayment.
  getPaymentStatus: (
    input: AuthorizePaymentInput
  ) => Promise<AuthorizePaymentOutput>
}

// Map the storefront-selected method to the SDK paymentMethod arm:
//  - UPI Collect: a VPA is pushed to the customer's UPI app for approval.
//  - UPI Intent: returns a deep-link the customer opens in their UPI app.
//  - default → hosted PayU redirect (wallet/netbanking via the PayU page).
function buildPaymentMethod(data: any): any {
  const method = String(data?.paymentMethodType ?? "upi_collect").toLowerCase()
  const vpa = data?.vpa as string | undefined
  if (method === "upi_intent") {
    return { upiIntent: {} }
  }
  if (method === "upi_collect" || method === "upi") {
    return { upiCollect: vpa ? { vpaId: { value: vpa } } : {} }
  }
  // Hosted PayU wallet/netbanking redirect.
  return { payuRedirect: {} }
}

// Authorize a PayU payment. The customer approves out-of-band (UPI app) or on
// the hosted PayU page, so the connector surfaces the next step as
// data.redirectUrl (UPI intent deep-link / hosted-page URL / form). The
// post-redirect retry PSyncs the existing payment instead of re-authorizing.
export async function authorizePayment(
  input: AuthorizePaymentInput,
  { options, paymentClient, getPaymentStatus }: PayuAuthorizeDeps
): Promise<AuthorizePaymentOutput> {
  const data = input.data as any

  // Idempotency: Medusa re-runs authorizePayment on every cart.complete retry
  // (e.g. after the customer returns from the redirect). A repeat call must NOT
  // re-authorize — it must status-sync (PSync) the existing payment. `data.id`
  // was set to the PayU reference on the first authorize so the sync targets it.
  if (data?.connectorTransactionId) {
    logger.error(
      "[PrismService.authorizePayment] payu: existing payment %s — syncing status instead of re-authorizing",
      data.connectorTransactionId
    )
    return await getPaymentStatus(input)
  }

  const minorAmount = (data?.minorAmount as number | undefined) ?? 0
  const currency = (data?.currency as string | undefined) ?? "INR"
  const returnUrl =
    (data?.returnUrl as string | undefined) ??
    ((options.connectorConfig as any)?.returnUrl as string | undefined)

  const billing = (data?.billing as PayuBilling | undefined) ?? {}
  if (!billing.firstName || !billing.email) {
    logger.error(
      "[PrismService.authorizePayment] payu: missing billing (firstName/email)"
    )
    return { data: input.data, status: PaymentSessionStatus.ERROR }
  }

  // The merchant txnid PayU echoes back; PSync (verify_payment) needs it as
  // var1 (NOT the mihpayid), so persist it on the session below.
  const merchantTransactionId = data?.id || `payu_${Date.now()}`
  try {
    const res: any = await paymentClient.authorize({
      merchantTransactionId,
      amount: { minorAmount, currency: toCurrency(currency) },
      description: "Medusa Hyperswitch Prism order",
      paymentMethod: buildPaymentMethod(data),
      captureMethod: types.CaptureMethod.AUTOMATIC,
      ...(returnUrl ? { returnUrl } : {}),
      address: { billingAddress: toBillingAddress(billing) },
      // PayU mandates s2s_client_ip (browser_info.ip_address) on every S2S
      // payment flow. In production the storefront passes the shopper's IP via
      // `data.ipAddress`; fall back to a routable test IP for the local harness.
      browserInfo: {
        ipAddress: extractValue((data as any)?.ipAddress) || "49.36.128.1",
      },
      testMode: options.environment !== "PRODUCTION",
    } as any)

    const rawStatus =
      (res?.status as number | undefined) ??
      types.PaymentStatus.PAYMENT_STATUS_UNSPECIFIED
    const redirect = res?.redirectionData
    // Prefer a ready GET URL (uri.uri); fall back to a form (endpoint +
    // formFields) rebuilt as a query string — PayU's hosted pages are GET URLs.
    const form = redirect?.form
    const formUrl = ((): string | undefined => {
      if (!form?.endpoint) return undefined
      const fields = (form.formFields ?? {}) as Record<string, unknown>
      const params = new URLSearchParams(
        Object.entries(fields).map(([k, v]) => [k, String(v)])
      ).toString()
      return params ? `${form.endpoint}?${params}` : form.endpoint
    })()
    const redirectUrl: string | undefined =
      redirect?.uri?.uri ?? formUrl ?? undefined

    return {
      data: {
        ...data,
        // Preserve the merchant txnid for PSync's verify_payment (var1).
        merchantTransactionId,
        // Adopt the PayU reference as `data.id` so the post-redirect PSync
        // (getTransactionId reads `data.id`) targets the real payment.
        ...(res?.connectorTransactionId ? { id: res.connectorTransactionId } : {}),
        connectorTransactionId: res?.connectorTransactionId,
        prismStatus: rawStatus,
        ...(redirectUrl ? { redirectUrl } : {}),
        raw: res,
      },
      status: mapPrismStatus(rawStatus),
    }
  } catch (error) {
    logger.error(
      "[PrismService.authorizePayment] payu ERROR: %s",
      (error as Error).message
    )
    throw buildError("An error occurred in authorizePayment", error)
  }
}
