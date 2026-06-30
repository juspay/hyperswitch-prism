import {
  PaymentClient,
  MerchantAuthenticationClient,
  types,
} from "hyperswitch-prism"
import {
  AuthorizePaymentInput,
  AuthorizePaymentOutput,
  InitiatePaymentOutput,
} from "@medusajs/framework/types"
import { PaymentSessionStatus } from "@medusajs/framework/utils"
import { HyperswitchPrismOptions } from "../../types"
import { buildError, extractValue, mapPrismStatus, toCurrency } from "../../utils"
import { logger } from "../../utils/logger"
import { InitiateConnectorContext } from "../types"

export type BraintreeDeps = {
  options: HyperswitchPrismOptions
  paymentClient: PaymentClient
  authClient: MerchantAuthenticationClient
}

export type BraintreeWalletType = "paypal" | "googlepay" | "applepay"

// Braintree is wallet-only (PayPal / Google Pay / Apple Pay). The hosted
// client-auth call (PAY_PAL arm, made by PrismService before dispatching here)
// returns one method-agnostic Braintree client_token that the braintree-web SDK
// uses to drive ALL three wallets client-side. So we surface that token once,
// plus the per-wallet config the storefront needs to build each wallet button.
export async function initiatePayment({
  options,
  merchantClientSessionId,
  currencyCode,
  minorAmount,
  sessionData,
  connectorSpecific,
}: InitiateConnectorContext): Promise<InitiatePaymentOutput> {
  const paypalArm = sessionData?.paypal ?? connectorSpecific?.paypal ?? {}

  // The Braintree client_token is identical across wallets: it sits directly on
  // the PayPal arm, and is buried in secrets.display on the gpay/apple arms —
  // check all so initiate still works if the default arm ever changes.
  const clientToken = extractValue(
    paypalArm?.clientToken ??
      paypalArm?.client_token ??
      sessionData?.googlePay?.googlePaySession?.secrets?.display ??
      sessionData?.applePay?.sessionResponse?.thirdPartySdk?.secrets?.display
  )

  if (!clientToken) {
    logger.error(
      "[PrismService.initiatePayment.braintree] no client_token in session data — verify paypal_client_id and merchant_account_id are configured"
    )
  }

  const cfg = (options.connectorConfig ?? {}) as any

  return {
    id: merchantClientSessionId,
    data: {
      id: merchantClientSessionId,
      connector: "braintree",
      merchantClientSessionId,
      currency: currencyCode,
      minorAmount,
      environment: options.environment ?? "SANDBOX",
      clientToken,
      // Surfaced for completeness; braintree-web reads PayPal config from the
      // client token itself, so the button does not depend on this value.
      paypalClientId:
        extractValue(paypalArm?.sessionToken ?? paypalArm?.session_token) ??
        cfg.paypalClientId,
      googlePay: {
        merchantId: cfg.gpayMerchantId,
        gatewayMerchantId: cfg.gpayGatewayMerchantId,
        merchantName: cfg.gpayMerchantName,
        allowedAuthMethods: cfg.gpayAllowedAuthMethods,
        allowedCardNetworks: cfg.gpayAllowedCardNetworks,
      },
      applePay: {
        supportedNetworks: cfg.applePaySupportedNetworks,
        merchantCapabilities: cfg.applePayMerchantCapabilities,
        label: cfg.applePayLabel,
      },
      sessionData,
    },
    status: PaymentSessionStatus.PENDING,
  }
}

// A session that was only initiated (no wallet nonce ever charged) has no real
// connectorTransactionId, so there is nothing to void when it is deleted (e.g.
// the shopper switches payment methods) — skip it rather than letting the
// connector reject the bogus session id.
export function shouldSkipVoid(data: Record<string, unknown> | undefined): boolean {
  return !(data as any)?.connectorTransactionId
}

// The storefront tokenizes the chosen wallet to a single Braintree nonce and
// persists { braintreeWalletType, braintreeNonce } on the session (via
// reinitiate) before cart-complete calls authorize. Google Pay / Apple Pay
// nonces MUST go through the *third-party-sdk* arms (the decrypted apple/google
// arms are rejected by the Braintree connector). With AUTOMATIC capture the
// connector runs a sale (CHARGE_* mutation), so a successful response is
// CAPTURED — no separate capture call is needed.
export async function authorizePayment(
  input: AuthorizePaymentInput,
  { options, paymentClient }: BraintreeDeps
): Promise<AuthorizePaymentOutput> {
  const data = input.data as any
  const walletType = (data?.braintreeWalletType as BraintreeWalletType) ?? "paypal"
  const nonce = (data?.braintreeNonce ?? data?.nonce) as string | undefined

  if (!nonce) {
    logger.error("[PrismService.authorizePayment] braintree: missing wallet nonce")
    return { data: input.data, status: PaymentSessionStatus.ERROR }
  }

  const paymentMethod: types.IPaymentMethod =
    walletType === "googlepay"
      ? { googlePayThirdPartySdk: { token: { value: nonce } } }
      : walletType === "applepay"
      ? { applePayThirdPartySdk: { token: { value: nonce } } }
      : { paypalSdk: { token: { value: nonce } } }

  // The connector requires a billing address on the authorize request. Wallet
  // payments carry no address from the storefront, so default the country (the
  // Braintree wallet sale does not otherwise use it).
  const countryCodeStr = (data?.country as string) ?? "US"
  const countryAlpha2Code =
    types.CountryAlpha2[countryCodeStr as keyof typeof types.CountryAlpha2] ??
    types.CountryAlpha2.US

  try {
    const res = await paymentClient.authorize({
      merchantTransactionId:
        (data?.merchantClientSessionId as string) ?? data?.id ?? `bt_${Date.now()}`,
      amount: {
        minorAmount: (data?.minorAmount as number) ?? 0,
        currency: toCurrency((data?.currency as string) || "USD"),
      },
      paymentMethod,
      captureMethod: types.CaptureMethod.AUTOMATIC,
      address: { billingAddress: { countryAlpha2Code } },
      testMode: options.environment !== "PRODUCTION",
    })

    const rawStatus =
      (res as any).status ?? types.PaymentStatus.PAYMENT_STATUS_UNSPECIFIED

    return {
      data: {
        ...(data as any),
        connectorTransactionId: (res as any).connectorTransactionId,
        braintreeWalletType: walletType,
        prismStatus: rawStatus,
        raw: res,
      },
      status: mapPrismStatus(rawStatus),
    }
  } catch (error) {
    logger.error(
      "[PrismService.authorizePayment] braintree ERROR: %s",
      (error as Error).message
    )
    throw buildError("An error occurred in authorizePayment", error)
  }
}
