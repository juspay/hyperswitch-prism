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

export async function initiatePayment({
  merchantClientSessionId,
  currencyCode,
  minorAmount,
  sessionData,
  connectorSpecific,
}: InitiateConnectorContext): Promise<InitiatePaymentOutput> {
  const connectorData =
    connectorSpecific?.mollie ?? connectorSpecific
  const paymentId =
    connectorData?.paymentId ?? connectorData?.payment_id
  const checkoutUrl = extractValue(
    connectorData?.checkoutUrl ?? connectorData?.checkout_url
  )

  return {
    id: paymentId || merchantClientSessionId,
    data: {
      id: paymentId || merchantClientSessionId,
      paymentId,
      checkoutUrl,
      currency: currencyCode,
      minorAmount,
      connector: "mollie",
      merchantClientSessionId,
      sessionData,
    },
    status: PaymentSessionStatus.PENDING,
  }
}

// Mollie carries its real reference (the payment id, `tr_…`) in `data.id`. When no
// payment id was obtained, `data.id` falls back to the Medusa session id — in that
// case there is no real payment to void, so skip the void.
export function shouldSkipVoid(data: Record<string, unknown> | undefined): boolean {
  const d = data as any
  return !d?.id || d.id === d?.merchantClientSessionId
}

export type MollieReInitiateContext = {
  data: InitiatePaymentInput["data"]
  merchantClientSessionId: string
  currencyCode: string
  minorAmount: number
}

// Mollie Components re-initiation: store the client-tokenized cardToken (and the
// storefront return URL for the 3DS redirect) on the session — no network call.
// Mirrors prism-globalpay reInitiatePayment.
export async function reInitiatePayment({
  data,
  merchantClientSessionId,
  currencyCode,
  minorAmount,
}: MollieReInitiateContext): Promise<InitiatePaymentOutput> {
  const d = data as any
  return {
    id: d?.id ?? merchantClientSessionId,
    data: {
      ...d,
      cardToken: d?.cardToken,
      returnUrl: d?.returnUrl,
      connector: "mollie",
      minorAmount,
      currency: currencyCode,
    },
    status: PaymentSessionStatus.PENDING,
  }
}

export type MollieAuthorizeDeps = {
  options: HyperswitchPrismOptions
  paymentClient: PaymentClient
  // Status-sync (PSync) used on the post-3DS retry — see authorizePayment.
  getPaymentStatus: (
    input: AuthorizePaymentInput
  ) => Promise<AuthorizePaymentOutput>
}

// Authorize a Mollie card payment with the client-tokenized cardToken (Mollie
// Components). The connector-service maps paymentMethod.token -> creditcard.cardToken.
// One-off cards come back as status `open` with a 3DS redirect, surfaced here as
// data.redirectUrl (redirectionData.form.endpoint) for the storefront to follow.
export async function authorizePayment(
  input: AuthorizePaymentInput,
  { options, paymentClient, getPaymentStatus }: MollieAuthorizeDeps
): Promise<AuthorizePaymentOutput> {
  const data = input.data as any

  // Idempotency: the first authorize creates the Mollie payment and consumes the
  // single-use cardToken. Medusa re-runs authorizePayment on every cart.complete
  // retry (e.g. after the customer returns from the 3DS redirect), so a repeat
  // call must NOT re-authorize — it must status-sync (PSync) the existing payment
  // and report its final status (`paid` -> CAPTURED). `data.id` was set to the
  // Mollie reference on the first authorize so the sync targets the right payment.
  if (data?.connectorTransactionId) {
    logger.error(
      "[PrismService.authorizePayment] mollie: existing payment %s — syncing status instead of re-authorizing",
      data.connectorTransactionId
    )
    return await getPaymentStatus(input)
  }

  const cardToken = data?.cardToken as string | undefined
  if (!cardToken) {
    logger.error(
      "[PrismService.authorizePayment] mollie: missing cardToken in session data"
    )
    return { data: input.data, status: PaymentSessionStatus.ERROR }
  }

  const minorAmount = (data?.minorAmount as number | undefined) ?? 0
  const currency = (data?.currency as string | undefined) ?? "USD"
  const returnUrl =
    (data?.returnUrl as string | undefined) ??
    ((options.connectorConfig as any)?.returnUrl as string | undefined)

  try {
    const res: any = await paymentClient.authorize({
      merchantTransactionId: data?.id || `mollie_${Date.now()}`,
      amount: { minorAmount, currency: toCurrency(currency) },
      // Mollie requires a payment description.
      description: "Medusa Hyperswitch Prism order",
      paymentMethod: { token: { token: { value: cardToken } } },
      captureMethod: types.CaptureMethod.AUTOMATIC,
      ...(returnUrl ? { returnUrl } : {}),
      // The SDK requires an address on authorize. The Mollie token branch
      // ignores billing_address connector-side, so a minimal country suffices.
      address: {
        billingAddress: { countryAlpha2Code: types.CountryAlpha2.US },
      },
      testMode: options.environment !== "PRODUCTION",
    } as any)

    const rawStatus =
      (res?.status as number | undefined) ??
      types.PaymentStatus.PAYMENT_STATUS_UNSPECIFIED
    const redirect = res?.redirectionData
    // Mollie's hosted checkout is a GET URL with the token as query params
    // (its `_links.checkout.href`). The connector models it as a `form`
    // (endpoint + formFields); a bare GET to `form.endpoint` 404s and a POST is
    // also rejected — the fields must be appended as query params. Reconstruct
    // that GET URL here. `uri.uri` (used by other connectors) is a ready GET URL.
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
        // Adopt the Mollie reference (tr_…) as both `id` and
        // `connectorTransactionId` so the post-3DS PSync (getTransactionId reads
        // `data.id`) targets the real payment, not the Medusa session id.
        ...(res?.connectorTransactionId
          ? { id: res.connectorTransactionId }
          : {}),
        connectorTransactionId: res?.connectorTransactionId,
        prismStatus: rawStatus,
        ...(redirectUrl ? { redirectUrl } : {}),
        raw: res,
      },
      status: mapPrismStatus(rawStatus),
    }
  } catch (error) {
    logger.error(
      "[PrismService.authorizePayment] mollie ERROR: %s",
      (error as Error).message
    )
    throw buildError("An error occurred in authorizePayment", error)
  }
}
