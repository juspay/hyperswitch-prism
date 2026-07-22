import { PaymentClient, types } from "hyperswitch-prism"
import {
  AuthorizePaymentInput,
  AuthorizePaymentOutput,
  InitiatePaymentInput,
  InitiatePaymentOutput,
  RefundPaymentInput,
  RefundPaymentOutput,
} from "@medusajs/framework/types"
import { PaymentSessionStatus } from "@medusajs/framework/utils"
import { HyperswitchPrismOptions } from "../../types"
import {
  buildError,
  mapPrismStatus,
  mapRefundStatus,
  normalizeAmount,
  toCurrency,
  toMinorAmount,
} from "../../utils"
import { logger } from "../../utils/logger"
import { InitiateConnectorContext } from "../types"

// Authorize.Net is a raw-card connector in this plugin: the test client collects
// card details and the server authorizes them directly. The UCS connector's
// Authorize flow accepts `PaymentMethodData::Card` (it has no Accept.js opaque-token
// variant), and its session-token flow (createClientAuthenticationToken) is
// not implemented — so there is no SDK/tokenization step here.

// First init: no SDK session is needed. Return a synthetic pending session so the
// card form can render; the card is collected and persisted via reInitiatePayment.
export async function initiatePayment({
  merchantClientSessionId,
  currencyCode,
  minorAmount,
  sessionData,
}: InitiateConnectorContext): Promise<InitiatePaymentOutput> {
  return {
    id: merchantClientSessionId,
    data: {
      id: merchantClientSessionId,
      currency: currencyCode,
      minorAmount,
      connector: "authorizedotnet",
      merchantClientSessionId,
      sessionData,
    },
    status: PaymentSessionStatus.PENDING,
  }
}

export type AuthorizedotnetReInitiateContext = {
  data: InitiatePaymentInput["data"]
  merchantClientSessionId: string
  currencyCode: string
  minorAmount: number
}

// Re-initiation: persist the card the buyer entered in the test client. No network
// call — authorizePayment forwards the card to the connector at cart-complete time.
export async function reInitiatePayment({
  data,
  merchantClientSessionId,
  currencyCode,
  minorAmount,
}: AuthorizedotnetReInitiateContext): Promise<InitiatePaymentOutput> {
  const d = data as any
  return {
    id: d?.id ?? merchantClientSessionId,
    data: {
      ...d,
      connector: "authorizedotnet",
      minorAmount,
      currency: currencyCode,
    },
    status: PaymentSessionStatus.PENDING,
  }
}

export type AuthorizedotnetAuthorizeDeps = {
  options: HyperswitchPrismOptions
  paymentClient: PaymentClient
}

export async function authorizePayment(
  input: AuthorizePaymentInput,
  { options, paymentClient }: AuthorizedotnetAuthorizeDeps
): Promise<AuthorizePaymentOutput> {
  const data = input.data as any
  const cardNumber = data?.cardNumber as string | undefined
  if (!cardNumber) {
    logger.error(
      "[PrismService.authorizePayment] authorizedotnet: missing card details in session data"
    )
    return { data: input.data, status: PaymentSessionStatus.ERROR }
  }

  const minorAmount = data?.minorAmount as number | undefined
  const currency = (data?.currency as string) || "USD"

  try {
    const authReq: any = {
      merchantTransactionId: data?.id || `anet_${Date.now()}`,
      amount: {
        minorAmount: minorAmount ?? 0,
        currency: toCurrency(currency),
      },
      paymentMethod: {
        card: {
          cardNumber: { value: String(cardNumber) },
          cardExpMonth: { value: String(data?.cardExpMonth ?? "") },
          cardExpYear: { value: String(data?.cardExpYear ?? "") },
          cardCvc: { value: String(data?.cardCvc ?? "") },
        },
      },
      captureMethod: types.CaptureMethod.AUTOMATIC,
      testMode: options.environment !== "PRODUCTION",
    }

    // Authorize.Net's connector requires a billing address. Use the session's
    // billing details if present, else test defaults. (Note: the Authorize.Net
    // sandbox can decline with errorCode 27 / "AVS mismatch" if the account's
    // AVS reject settings are strict — disable AVS reject in the sandbox account
    // if approvals fail despite a matching address.)
    const countryAlpha2Code =
      types.CountryAlpha2[
        ((data?.country as string) ?? "US") as keyof typeof types.CountryAlpha2
      ] ?? types.CountryAlpha2.US
    authReq.address = {
      billingAddress: {
        firstName: { value: (data?.firstName as string) ?? "Test" },
        lastName: { value: (data?.lastName as string) ?? "Customer" },
        line1: { value: (data?.line1 as string) ?? "1 Market St" },
        city: { value: (data?.city as string) ?? "San Francisco" },
        state: { value: (data?.state as string) ?? "CA" },
        zipCode: { value: (data?.zipCode as string) ?? "94105" },
        countryAlpha2Code,
        email: { value: (data?.email as string) ?? "test@example.com" },
      },
    }

    const res = await paymentClient.authorize(authReq)

    const rawStatus =
      (res as any).status ?? types.PaymentStatus.PAYMENT_STATUS_UNSPECIFIED
    return {
      data: {
        ...data,
        ...(res as any),
        connectorTransactionId: (res as any).connectorTransactionId,
        prismStatus: rawStatus,
      },
      status: mapPrismStatus(rawStatus),
    }
  } catch (error) {
    logger.error(
      "[PrismService.authorizePayment] authorizedotnet ERROR: %s",
      (error as Error).message
    )
    throw buildError("An error occurred in authorizePayment", error)
  }
}

export type AuthorizedotnetRefundDeps = {
  options: HyperswitchPrismOptions
  paymentClient: PaymentClient
}

// Authorize.Net's refund (credit) references the original transaction AND must
// carry the card to credit. The connector reads the card from
// connector_feature_data.creditCard ({ cardNumber, expirationDate }) — the
// generic refund path omits it, so authorizedotnet needs this custom handler.
export async function refundPayment(
  { data, amount, context }: RefundPaymentInput,
  { options, paymentClient }: AuthorizedotnetRefundDeps
): Promise<RefundPaymentOutput> {
  const d = data as any
  const connectorTransactionId = (d?.connectorTransactionId ?? d?.id) as
    | string
    | undefined
  const currency = (d?.currency as string) || "USD"
  const cardNumber = d?.cardNumber as string | undefined

  if (!connectorTransactionId) {
    throw buildError(
      "An error occurred in refundPayment",
      new Error("Missing Authorize.Net transaction id for refund")
    )
  }
  if (!cardNumber) {
    throw buildError(
      "An error occurred in refundPayment",
      new Error("Missing card details for Authorize.Net refund")
    )
  }

  // Refund-by-reference (refTransId) only needs the last four digits + expiry.
  const expirationDate =
    d?.cardExpYear && d?.cardExpMonth
      ? `${String(d.cardExpYear)}-${String(d.cardExpMonth).padStart(2, "0")}`
      : "XXXX"
  const creditCard = {
    cardNumber: String(cardNumber).slice(-4),
    expirationDate,
  }

  try {
    const res = await paymentClient.refund({
      merchantRefundId:
        (context?.idempotency_key as string) ?? `ref_${Date.now()}`,
      connectorTransactionId,
      refundAmount: {
        minorAmount: toMinorAmount(normalizeAmount(amount), currency),
        currency: toCurrency(currency),
      },
      connectorFeatureData: { value: JSON.stringify({ creditCard }) },
      testMode: options.environment !== "PRODUCTION",
    } as any)

    const refundStatus = (res as any).status as number | undefined
    const isFailure =
      refundStatus === types.RefundStatus.REFUND_FAILURE ||
      refundStatus === types.RefundStatus.REFUND_TRANSACTION_FAILURE

    // Authorize.Net cannot refund an UNSETTLED transaction (errorCode 54 — "does
    // not meet the criteria for issuing a credit"): a just-captured charge sits
    // in capturedPendingSettlement until the daily batch settles. The correct
    // reversal for an unsettled capture is a VOID, so fall back to that.
    const connectorCode = (res as any)?.error?.connectorDetails?.code as
      | string
      | undefined
    if (isFailure && connectorCode === "54") {
      logger.error(
        "[authorizedotnet.refundPayment] unsettled (code 54) — voiding instead of refunding"
      )
      await paymentClient.void({
        merchantVoidId: `void_${Date.now()}`,
        connectorTransactionId,
      })
      return {
        data: {
          ...d,
          refundStatus: types.RefundStatus.REFUND_SUCCESS,
          refundStatusText: "refunded",
          refundedViaVoid: true,
        },
      }
    }

    if (isFailure) {
      throw new Error(
        (res as any).error?.message ?? "Refund failed at connector"
      )
    }
    return {
      data: {
        ...d,
        refundStatus,
        refundStatusText: mapRefundStatus(refundStatus ?? 0),
        raw: res,
      },
    }
  } catch (error) {
    logger.error(
      "[authorizedotnet.refundPayment] ERROR: %s",
      (error as Error).message
    )
    throw buildError("An error occurred in refundPayment", error)
  }
}

// No connector transaction exists until authorize → nothing to void before that.
export function shouldSkipVoid(
  data: Record<string, unknown> | undefined
): boolean {
  return !(data as any)?.connectorTransactionId
}
