import { PaymentClient, types } from "hyperswitch-prism"
import {
  AuthorizePaymentInput,
  AuthorizePaymentOutput,
  InitiatePaymentInput,
  InitiatePaymentOutput,
} from "@medusajs/framework/types"
import { PaymentSessionStatus } from "@medusajs/framework/utils"
import { HyperswitchPrismOptions } from "../../types"
import { buildError, mapPrismStatus, toCurrency } from "../../utils"
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

// No connector transaction exists until authorize → nothing to void before that.
export function shouldSkipVoid(
  data: Record<string, unknown> | undefined
): boolean {
  return !(data as any)?.connectorTransactionId
}
