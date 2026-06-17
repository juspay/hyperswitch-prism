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
}

// Authorize a Mollie card payment with the client-tokenized cardToken (Mollie
// Components). The connector-service maps paymentMethod.token -> creditcard.cardToken.
// One-off cards come back as status `open` with a 3DS redirect, surfaced here as
// data.redirectUrl (redirectionData.form.endpoint) for the storefront to follow.
export async function authorizePayment(
  input: AuthorizePaymentInput,
  { options, paymentClient }: MollieAuthorizeDeps
): Promise<AuthorizePaymentOutput> {
  const data = input.data as any
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
    const redirectUrl: string | undefined =
      redirect?.form?.endpoint ?? redirect?.uri?.uri ?? undefined

    return {
      data: {
        ...data,
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
