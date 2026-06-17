import {
  PaymentClient,
  MerchantAuthenticationClient,
  types,
} from "hyperswitch-prism"
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
  extractValue,
  mapPrismStatus,
  normalizeAmount,
  toCurrency,
  toMinorAmount,
} from "../../utils"
import { logger } from "../../utils/logger"
import { InitiateConnectorContext } from "../types"

export type GlobalpayReInitiateContext = {
  data: InitiatePaymentInput["data"]
  merchantClientSessionId: string
  currencyCode: string
  minorAmount: number
}

// Re-initiation with card token — skip access-token fetch, just store the token
export async function reInitiatePayment({
  data,
  merchantClientSessionId,
  currencyCode,
  minorAmount,
}: GlobalpayReInitiateContext): Promise<InitiatePaymentOutput> {
  const paymentReference = (data as any).paymentReference as string
  return {
    id: (data as any)?.id ?? merchantClientSessionId,
    data: {
      ...(data as any),
      paymentReference,
      connector: "globalpay",
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
    connectorSpecific?.globalpay ?? connectorSpecific
  const accessToken = extractValue(
    connectorData?.accessToken ?? connectorData?.access_token
  )
  const tokenType =
    connectorData?.tokenType ?? connectorData?.token_type
  const expiresIn =
    connectorData?.expiresIn ?? connectorData?.expires_in

  return {
    id: merchantClientSessionId,
    data: {
      id: merchantClientSessionId,
      accessToken,
      tokenType,
      expiresIn,
      currency: currencyCode,
      minorAmount,
      connector: "globalpay",
      merchantClientSessionId,
      sessionData,
    },
    status: PaymentSessionStatus.PENDING,
  }
}

export type GlobalpayAuthorizeDeps = {
  options: HyperswitchPrismOptions
  authClient: MerchantAuthenticationClient
  paymentClient: PaymentClient
  getPaymentReferenceFromCart: (
    resourceId: string
  ) => Promise<string | undefined>
}

export async function authorizePayment(
  input: AuthorizePaymentInput,
  { options, authClient, paymentClient, getPaymentReferenceFromCart }: GlobalpayAuthorizeDeps
): Promise<AuthorizePaymentOutput> {
  const data = input.data as any
  let paymentReference = data?.paymentReference as string | undefined

  // If not in session data, try reading from cart metadata
  const resourceId = (input.context as any)?.resource_id as string | undefined
  if (!paymentReference && resourceId) {
    paymentReference = await getPaymentReferenceFromCart(resourceId)
  }

  if (!paymentReference) {
    logger.error("[PrismService.authorizePayment] globalpay: missing paymentReference in session data and cart metadata")
    return {
      data: input.data,
      status: PaymentSessionStatus.ERROR,
    }
  }

  const minorAmount = data?.minorAmount as number | undefined
  const currency = data?.currency as string | undefined
  const returnUrl =
    (data?.returnUrl as string | undefined) ??
    ((options.connectorConfig as any)?.returnUrl as string | undefined)

  try {
    // Fetch a server access token with default (authorization) permissions
    const authRes = await authClient.createClientAuthenticationToken({
      merchantClientSessionId: `auth_${Date.now()}`,
      payment: {
        amount: {
          minorAmount: minorAmount ?? 0,
          currency: toCurrency(currency || "USD"),
        },
      },
    })

    const authSessionData =
      (authRes as any).sessionData ?? (authRes as any).session_data ?? {}
    const authConnectorSpecific =
      authSessionData?.connectorSpecific ?? authSessionData?.connector_specific ?? {}
    const gpAuthData = authConnectorSpecific?.globalpay ?? authConnectorSpecific
    const serverToken = extractValue(
      gpAuthData?.accessToken ?? gpAuthData?.access_token
    )

    const countryCodeStr = (data?.country as string) ?? "US"
    const countryAlpha2Code =
      types.CountryAlpha2[countryCodeStr as keyof typeof types.CountryAlpha2] ??
      types.CountryAlpha2.US

    const authorizeReq: any = {
      merchantTransactionId: data?.id || `gp_${Date.now()}`,
      amount: {
        minorAmount: minorAmount ?? 0,
        currency: toCurrency(currency || "USD"),
      },
      connectorToken: { value: paymentReference },
      captureMethod: types.CaptureMethod.AUTOMATIC,
      ...(returnUrl ? { returnUrl } : {}),
      address: {
        billingAddress: {
          countryAlpha2Code,
        },
      },
      testMode: options.environment !== "PRODUCTION",
    }

    if (serverToken) {
      authorizeReq.state = {
        accessToken: {
          token: { value: serverToken },
          tokenType: "Bearer",
        },
      }
    }

    const res = await paymentClient.tokenAuthorize(authorizeReq)
    const rawStatus =
      (res as any).status ?? types.PaymentStatus.PAYMENT_STATUS_UNSPECIFIED
    const mappedStatus = mapPrismStatus(rawStatus)

    return {
      data: {
        ...(data as any),
        ...(res as any),
        connectorTransactionId: (res as any).connectorTransactionId,
        prismStatus: rawStatus,
      },
      status: mappedStatus,
    }
  } catch (error) {
    const msg = (error as Error).message ?? ""
    // GlobalPay returns DECLINED with amount:"" which Prism can't deserialize.
    // Treat deserialization failures as payment errors rather than server errors.
    if (msg.includes("deserialize") || msg.includes("macros")) {
      logger.error("[PrismService.authorizePayment] globalpay deserialization failure (likely DECLINED): %s", msg)
      return {
        data: { ...(data as any), error: msg },
        status: PaymentSessionStatus.ERROR,
      }
    }
    logger.error("[PrismService.authorizePayment] globalpay ERROR: %s", msg)
    throw buildError("An error occurred in authorizePayment", error)
  }
}

// If the session is still at the access-token stage (no real payment
// transaction exists yet), there is nothing to void.
export function shouldSkipVoid(data: Record<string, unknown> | undefined): boolean {
  return (
    !(data as any)?.connectorTransactionId && Boolean((data as any)?.accessToken)
  )
}

export type GlobalpayRefundDeps = {
  options: HyperswitchPrismOptions
  authClient: MerchantAuthenticationClient
  paymentClient: PaymentClient
}

/**
 * GlobalPay authenticates every call with a server access token passed via
 * state.accessToken (see authorizePayment). The generic refund path sends no
 * state, so the connector fails with FAILED_TO_OBTAIN_AUTH_TYPE — fetch a
 * token here and attach it.
 */
export async function refundPayment(
  { data, amount, context }: RefundPaymentInput,
  { options, authClient, paymentClient }: GlobalpayRefundDeps
): Promise<RefundPaymentOutput> {
  const d = data as any
  const connectorTransactionId = (d?.connectorTransactionId ?? d?.id) as
    | string
    | undefined
  const currency = (d?.currency as string) || "USD"
  const minorAmount = toMinorAmount(normalizeAmount(amount), currency)

  if (!connectorTransactionId) {
    logger.error("[PrismService.refundPayment] globalpay: missing transaction id")
    throw buildError(
      "An error occurred in refundPayment",
      new Error("Missing GlobalPay transaction id for refund")
    )
  }

  try {
    const authRes = await authClient.createClientAuthenticationToken({
      merchantClientSessionId: `refund_auth_${Date.now()}`,
      payment: {
        amount: {
          minorAmount,
          currency: toCurrency(currency),
        },
      },
    })

    const authSessionData =
      (authRes as any).sessionData ?? (authRes as any).session_data ?? {}
    const authConnectorSpecific =
      authSessionData?.connectorSpecific ?? authSessionData?.connector_specific ?? {}
    const gpAuthData = authConnectorSpecific?.globalpay ?? authConnectorSpecific
    const serverToken = extractValue(
      gpAuthData?.accessToken ?? gpAuthData?.access_token
    )

    if (!serverToken) {
      throw new Error("Could not obtain GlobalPay access token for refund")
    }

    const res = await paymentClient.refund({
      merchantRefundId:
        (context?.idempotency_key as string) ?? `ref_${Date.now()}`,
      connectorTransactionId,
      refundAmount: {
        minorAmount,
        currency: toCurrency(currency),
      },
      state: {
        accessToken: {
          token: { value: serverToken },
          tokenType: "Bearer",
        },
      },
      testMode: options.environment !== "PRODUCTION",
    } as any)

    const refundStatus = (res as any).status as number | undefined
    if (
      refundStatus === types.RefundStatus.REFUND_FAILURE ||
      refundStatus === types.RefundStatus.REFUND_TRANSACTION_FAILURE
    ) {
      throw new Error(
        (res as any).error?.message ?? "Refund failed at connector"
      )
    }

    return { data: { ...d, refundStatus, raw: res } }
  } catch (error) {
    logger.error("[PrismService.refundPayment] globalpay ERROR: %s", (error as Error).message)
    throw buildError("An error occurred in refundPayment", error)
  }
}
