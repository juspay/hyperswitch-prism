"use client"

import React from "react"
import { AdyenWrapper } from "../connectors/adyen/AdyenWrapper"
import { PayPalWrapper } from "../connectors/paypal/PayPalWrapper"
import { GlobalPayWrapper } from "../connectors/globalpay/GlobalPayWrapper"
import {
  isHyperswitchPrismAdyen,
  isHyperswitchPrismPaypal,
  isHyperswitchPrismGlobalpay,
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
   * Called after GlobalPay tokenization — store the paymentReference in the
   * Medusa payment session before the user clicks Place Order.
   * Receives `{ paymentReference, id }` so the host can call initiatePaymentSession.
   */
  onInitiateSession: (data: { paymentReference: string; id: string }) => Promise<void>
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
          console.log("[PayPal] approved", paymentData)
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

  return null
}
