"use client";

import React from "react";
import type { PaymentButtonProps } from "./types";
import { GlobalPayPaymentButton } from "./GlobalPayPaymentButton";
import { StripePaymentButton } from "./StripePaymentButton";
import { AdyenPaymentButton } from "./AdyenPaymentButton";
import { PayPalPaymentButton } from "./PayPalPaymentButton";
import { MolliePaymentButton, type MolliePaymentButtonProps } from "./MolliePaymentButton";
import { ManualTestPaymentButton } from "./ManualTestPaymentButton";

/** Detect Stripe-like providers */
function isStripeLike(providerId?: string) {
  return (
    providerId?.startsWith("pp_stripe_") ||
    providerId?.startsWith("pp_medusa-") ||
    providerId === "pp_hyperswitch-prism_stripe"
  );
}

/** Detect Adyen-like providers */
function isAdyenLike(providerId?: string) {
  return (
    providerId?.startsWith("pp_adyen_") ||
    providerId === "pp_hyperswitch-prism_adyen"
  );
}

/** Detect GlobalPay provider */
function isGlobalPayLike(providerId?: string) {
  return providerId === "pp_hyperswitch-prism_globalpay";
}

/** Detect PayPal provider */
function isPayPalLike(providerId?: string) {
  return (
    providerId?.startsWith("pp_paypal") ||
    providerId === "pp_hyperswitch-prism_paypal"
  );
}

/** Detect Mollie provider */
function isMollieLike(providerId?: string) {
  return (
    providerId?.startsWith("pp_mollie") ||
    providerId === "pp_hyperswitch-prism_mollie"
  );
}

/** Detect manual / test payment */
function isManual(providerId?: string) {
  return providerId?.startsWith("pp_system_default");
}

/**
 * Auto-dispatching payment button.
 *
 * Chooses the correct connector-specific button based on `providerId`.
 *
 * Usage:
 * ```tsx
 * <PaymentButton
 *   providerId={cart.payment_collection?.payment_sessions?.[0]?.provider_id}
 *   notReady={!cart.shipping_address}
 *   cart={cart}
 *   onPlaceOrder={placeOrder}
 *   onUpdateCart={(meta) => updateCart({ metadata: meta })}
 *   buttonComponent={Button}
 * />
 * ```
 */
export function PaymentButton({
  providerId,
  notReady,
  cart,
  onPlaceOrder,
  onUpdateCart,
  buttonComponent,
  backendUrl,
  publishableKey,
  "data-testid": dataTestId,
}: PaymentButtonProps) {
  const baseProps = {
    notReady,
    onPlaceOrder,
    buttonComponent,
    "data-testid": dataTestId,
  };

  if (isStripeLike(providerId)) {
    if (!cart) {
      return (
        <div style={{ color: "#c00" }}>
          Cart prop is required for Stripe payment button
        </div>
      );
    }
    return <StripePaymentButton {...baseProps} cart={cart} />;
  }

  if (isAdyenLike(providerId)) {
    return <AdyenPaymentButton {...baseProps} />;
  }

  if (isGlobalPayLike(providerId)) {
    return <GlobalPayPaymentButton {...baseProps} onUpdateCart={onUpdateCart} />;
  }

  if (isPayPalLike(providerId)) {
    if (!cart) {
      return (
        <div style={{ color: "#c00" }}>
          Cart prop is required for PayPal payment button
        </div>
      );
    }
    return <PayPalPaymentButton {...baseProps} cart={cart} />;
  }

  if (isMollieLike(providerId)) {
    if (!cart) {
      return (
        <div style={{ color: "#c00" }}>
          Cart prop is required for Mollie payment button
        </div>
      );
    }
    return (
      <MolliePaymentButton
        {...baseProps}
        cart={cart as MolliePaymentButtonProps["cart"]}
        backendUrl={backendUrl ?? ""}
        publishableKey={publishableKey ?? ""}
      />
    );
  }

  if (isManual(providerId)) {
    return <ManualTestPaymentButton {...baseProps} />;
  }

  return (
    <button disabled className="payment-btn">
      Select a payment method
    </button>
  );
}

export { GlobalPayPaymentButton } from "./GlobalPayPaymentButton";
export { StripePaymentButton } from "./StripePaymentButton";
export { AdyenPaymentButton } from "./AdyenPaymentButton";
export { PayPalPaymentButton } from "./PayPalPaymentButton";
export { MolliePaymentButton } from "./MolliePaymentButton";
export { ManualTestPaymentButton } from "./ManualTestPaymentButton";
export type { MolliePaymentButtonProps } from "./MolliePaymentButton";
export type {
  BasePaymentButtonProps,
  StripePaymentButtonProps,
  GlobalPayPaymentButtonProps,
  PayPalPaymentButtonProps,
  PaymentButtonProps,
} from "./types";
