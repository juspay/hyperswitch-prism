"use client"

import React from "react"
import { AdyenWrapper } from "../connectors/adyen/AdyenWrapper"
import { PayPalWrapper } from "../connectors/paypal/PayPalWrapper"
import { GlobalPayWrapper } from "../connectors/globalpay/GlobalPayWrapper"
import { MollieWrapper } from "../connectors/mollie/MollieWrapper"
import {
  isHyperswitchPrismAdyen,
  isHyperswitchPrismPaypal,
  isHyperswitchPrismGlobalpay,
  isHyperswitchPrismMollie,
} from "../utils/predicates"

interface HyperswitchPrismConnectorPanelProps {
  /** The provider ID of the selected payment method (e.g. "pp_hyperswitch-prism_adyen") */
  providerId: string
  /** Payment session data returned from initiatePayment — null/undefined while loading */
  sessionData: Record<string, any> | null | undefined
  /**
   * Adyen client key from your storefront env (NEXT_PUBLIC_ADYEN_CLIENT_KEY).
   * Required when using the Adyen connector.
   */
  adyenClientKey?: string
  /**
   * Called after the connector produces session data that must be persisted on
   * the Medusa payment session before Place Order. The host forwards `data`
   * straight to `initiatePaymentSession(cart, { provider_id, data })`.
   * - GlobalPay passes `{ paymentReference, id }` (tokenized card reference).
   * - Mollie passes `{ ...sessionData, cardToken, returnUrl }` (tokenized card).
   */
  onInitiateSession: (data: Record<string, unknown>) => Promise<void>
  /** Called when Adyen reports an authorised result — use to advance to the next step */
  onPaymentCompleted?: (result?: any) => void
  onError: (error: Error) => void
  environment?: "sandbox" | "production"
  /**
   * PayPal only: forward the shopper's shipping address from the approved
   * order. Must match the provider's `connectorConfig.includeShippingData`.
   */
  includeShippingData?: boolean
  /**
   * PayPal only: forward the payer's details from the approved order.
   * Must match the provider's `connectorConfig.includeCustomerData`.
   */
  includeCustomerData?: boolean
  /**
   * Mollie only: the full URL Mollie redirects to after 3DS. The host builds it
   * (e.g. `${origin}/[cc]/checkout/mollie-return`) and renders a route there
   * that finalises the order (see `MollieReturnHandler`).
   */
  mollieReturnUrl?: string
  /** Mollie only: test mode for Mollie Components (default true). */
  mollieTestmode?: boolean
  /** Mollie only: locale for Mollie Components (default en_US). */
  mollieLocale?: string
}

/**
 * Renders the correct connector UI (Adyen / PayPal / GlobalPay) for a
 * HyperswitchPrism payment session. Returns null for Stripe (handled by the
 * host Stripe Elements wrapper) and for unknown providers.
 *
 * Order placement is the responsibility of the host's Place Order button.
 * This panel handles only the payment instrument (card entry / tokenization).
 */
export function HyperswitchPrismConnectorPanel({
  providerId,
  sessionData,
  adyenClientKey,
  onInitiateSession,
  onPaymentCompleted,
  onError,
  environment = "sandbox",
  includeShippingData = false,
  includeCustomerData = false,
  mollieReturnUrl,
  mollieTestmode = true,
  mollieLocale,
}: HyperswitchPrismConnectorPanelProps) {
  if (!sessionData) return null

  if (isHyperswitchPrismAdyen(providerId)) {
    return (
      <AdyenWrapper
        sessionData={{
          clientKey: adyenClientKey ?? sessionData.publishableKey ?? "",
          session: {
            id:
              sessionData.sessionData?.connectorSpecific?.adyen?.sessionId ??
              sessionData.id,
            sessionData:
              sessionData.sessionData?.connectorSpecific?.adyen?.sessionData
                ?.value ?? "",
          },
          countryCode: "GB",
          minorAmount: sessionData.minorAmount ?? 0,
          currency: sessionData.currency ?? "EUR",
        }}
        onPaymentCompleted={onPaymentCompleted}
        onError={onError}
      />
    )
  }

  if (isHyperswitchPrismPaypal(providerId)) {
    return (
      <PayPalWrapper
        clientId={sessionData.paypalClientId ?? ""}
        currency={sessionData.currency ?? "EUR"}
        amount={(sessionData.minorAmount ?? 0) / 100}
        environment={environment}
        includeShippingData={includeShippingData}
        includeCustomerData={includeCustomerData}
        onCreateOrder={() => Promise.resolve(sessionData.paypalOrderId ?? "")}
        onSubmit={(paymentData: any) => {
          onPaymentCompleted?.(paymentData)
        }}
        onError={onError}
      />
    )
  }

  if (isHyperswitchPrismGlobalpay(providerId)) {
    return (
      <GlobalPayWrapper
        accessToken={sessionData.accessToken ?? ""}
        environment={environment}
        onSubmit={async (paymentData) => {
          if (typeof window !== "undefined") {
            sessionStorage.setItem(
              "globalpay_payment_reference",
              paymentData.paymentReference
            )
          }
          await onInitiateSession({
            paymentReference: paymentData.paymentReference,
            id: sessionData.id,
          })
          onPaymentCompleted?.()
        }}
        onError={onError}
      />
    )
  }

  if (isHyperswitchPrismMollie(providerId)) {
    // Mollie Components need the public profile id; render nothing until it
    // arrives in the session data.
    if (!sessionData.profileId) return null
    return (
      <MollieWrapper
        profileId={sessionData.profileId as string}
        testmode={mollieTestmode}
        locale={mollieLocale}
        onSubmit={async ({ cardToken }) => {
          // Persist the tokenized card + 3DS return URL on the session, then
          // advance — the host's place-order button authorizes, Mollie redirects
          // to `mollieReturnUrl`, and the return handler finalises the order.
          await onInitiateSession({
            ...sessionData,
            cardToken,
            returnUrl: mollieReturnUrl,
          })
          onPaymentCompleted?.()
        }}
        onError={onError}
      />
    )
  }

  return null
}
