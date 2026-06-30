import { PaymentClient, types } from "hyperswitch-prism"
import {
  AuthorizePaymentInput,
  AuthorizePaymentOutput,
  InitiatePaymentInput,
  InitiatePaymentOutput,
} from "@medusajs/framework/types"
import { PaymentSessionStatus } from "@medusajs/framework/utils"
import { HyperswitchPrismOptions } from "../../types"
import { buildError, extractValue, mapPrismStatus, toCurrency } from "../../utils"
import { logger } from "../../utils/logger"
import { InitiateConnectorContext } from "../types"

export type CybersourceReInitiateContext = {
  data: InitiatePaymentInput["data"]
  merchantClientSessionId: string
  currencyCode: string
  minorAmount: number
}

// Re-initiation with the Flex transient token — the Microform SDK has already
// tokenised the card on the client, so we only persist the token in the session.
// No network call: authorizePayment forwards it to UCS as the connectorToken.
export async function reInitiatePayment({
  data,
  merchantClientSessionId,
  currencyCode,
  minorAmount,
}: CybersourceReInitiateContext): Promise<InitiatePaymentOutput> {
  const transientToken = (data as any).transientToken as string
  return {
    id: (data as any)?.id ?? merchantClientSessionId,
    data: {
      ...(data as any),
      transientToken,
      connector: "cybersource",
      minorAmount,
      currency: currencyCode,
    },
    status: PaymentSessionStatus.PENDING,
  }
}

export async function initiatePayment({
  merchantClientSessionId,
  currencyCode,
  minorAmount,
  sessionData,
  connectorSpecific,
}: InitiateConnectorContext): Promise<InitiatePaymentOutput> {
  const connectorData =
    connectorSpecific?.cybersource ?? connectorSpecific
  const captureContext = extractValue(
    connectorData?.captureContext ?? connectorData?.capture_context
  )
  const clientLibrary =
    connectorData?.clientLibrary ?? connectorData?.client_library
  const clientLibraryIntegrity =
    connectorData?.clientLibraryIntegrity ??
    connectorData?.client_library_integrity

  return {
    id: merchantClientSessionId,
    data: {
      id: merchantClientSessionId,
      captureContext,
      clientLibrary,
      clientLibraryIntegrity,
      currency: currencyCode,
      minorAmount,
      connector: "cybersource",
      merchantClientSessionId,
      sessionData,
    },
    status: PaymentSessionStatus.PENDING,
  }
}

export type CybersourceAuthorizeDeps = {
  options: HyperswitchPrismOptions
  paymentClient: PaymentClient
}

export async function authorizePayment(
  input: AuthorizePaymentInput,
  { options, paymentClient }: CybersourceAuthorizeDeps
): Promise<AuthorizePaymentOutput> {
  const data = input.data as any
  const transientToken = data?.transientToken as string | undefined

  if (!transientToken) {
    logger.error(
      "[PrismService.authorizePayment] cybersource: missing transientToken in session data"
    )
    return { data: input.data, status: PaymentSessionStatus.ERROR }
  }

  const minorAmount = data?.minorAmount as number | undefined
  const currency = (data?.currency as string) || "USD"
  const countryCodeStr = (data?.country as string) ?? "US"
  const countryAlpha2Code =
    types.CountryAlpha2[countryCodeStr as keyof typeof types.CountryAlpha2] ??
    types.CountryAlpha2.US

  try {
    const res = await paymentClient.tokenAuthorize({
      merchantTransactionId: data?.id || `cyb_${Date.now()}`,
      amount: {
        minorAmount: minorAmount ?? 0,
        currency: toCurrency(currency),
      },
      // Flex transient token stands in for the payment method (proto: connectorToken).
      connectorToken: { value: transientToken },
      captureMethod: types.CaptureMethod.AUTOMATIC,
      // Cybersource requires a billTo block (email is mandatory; name + address
      // satisfy AVS). The test harness has no customer form, so use the billing
      // details from the session if present, else sensible test defaults.
      address: {
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
      },
      testMode: options.environment !== "PRODUCTION",
    })

    const rawStatus =
      (res as any).status ?? types.PaymentStatus.PAYMENT_STATUS_UNSPECIFIED
    return {
      data: {
        ...(data as any),
        ...(res as any),
        connectorTransactionId: (res as any).connectorTransactionId,
        prismStatus: rawStatus,
      },
      status: mapPrismStatus(rawStatus),
    }
  } catch (error) {
    logger.error(
      "[PrismService.authorizePayment] cybersource ERROR: %s",
      (error as Error).message
    )
    throw buildError("An error occurred in authorizePayment", error)
  }
}

// Cybersource only obtains a real connector transaction id at authorize time, so
// a session deleted before that (e.g. the shopper switches payment methods) has
// nothing to void — skip it instead of letting the connector reject a bogus id.
export function shouldSkipVoid(data: Record<string, unknown> | undefined): boolean {
  return !(data as any)?.connectorTransactionId
}
