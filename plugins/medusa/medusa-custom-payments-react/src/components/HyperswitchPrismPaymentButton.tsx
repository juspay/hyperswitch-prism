"use client"

import React from "react"
import {
  StripePaymentButton,
  AdyenPaymentButton,
  PayPalPaymentButton,
  GlobalPayPaymentButton,
  MolliePaymentButton,
} from "./payment-buttons"
import {
  isHyperswitchPrismStripe,
  isHyperswitchPrismAdyen,
  isHyperswitchPrismPaypal,
  isHyperswitchPrismGlobalpay,
  isHyperswitchPrismMollie,
} from "../utils/predicates"

interface HyperswitchPrismPaymentButtonProps {
  providerId: string | undefined
  cart: any
  notReady: boolean
  onPlaceOrder: () => Promise<void>
  /** Host button component (e.g. Medusa UI `Button`) — passed through to each connector button */
  buttonComponent?: React.ComponentType<any>
  /**
   * For Adyen sessions flow: true when the drop-in has reported an
   * authorised resultCode. Disables "Place order" until the customer
   * completes the drop-in, preventing race conditions with the webhook.
   */
  isAuthorized?: boolean
  /**
   * Mollie only: Medusa backend URL + publishable key, used to re-read the
   * active session's 3DS redirect after the place-order call returns
   * `requires_more`. (e.g. NEXT_PUBLIC_MEDUSA_BACKEND_URL / _PUBLISHABLE_KEY)
   */
  backendUrl?: string
  publishableKey?: string
  "data-testid"?: string
}

/**
 * Auto-dispatches to the correct connector payment button based on providerId.
 * Returns null if the providerId is not a HyperswitchPrism provider — the host
 * can render its own fallback (e.g. for Stripe or manual payment).
 */
export function HyperswitchPrismPaymentButton({
  providerId,
  cart,
  notReady,
  onPlaceOrder,
  buttonComponent,
  isAuthorized = true,
  backendUrl,
  publishableKey,
  "data-testid": dataTestId,
}: HyperswitchPrismPaymentButtonProps) {
  const shared = { notReady, onPlaceOrder, buttonComponent, "data-testid": dataTestId }

  if (isHyperswitchPrismStripe(providerId)) {
    return <StripePaymentButton {...shared} cart={cart} />
  }

  if (isHyperswitchPrismAdyen(providerId)) {
    return <AdyenPaymentButton {...shared} isAuthorized={isAuthorized} />
  }

  if (isHyperswitchPrismPaypal(providerId)) {
    return <PayPalPaymentButton {...shared} cart={cart} />
  }

  if (isHyperswitchPrismGlobalpay(providerId)) {
    return <GlobalPayPaymentButton {...shared} />
  }

  if (isHyperswitchPrismMollie(providerId)) {
    return (
      <MolliePaymentButton
        {...shared}
        cart={cart}
        backendUrl={backendUrl ?? ""}
        publishableKey={publishableKey ?? ""}
      />
    )
  }

  return null
}
