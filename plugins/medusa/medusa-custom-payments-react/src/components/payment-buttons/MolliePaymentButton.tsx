"use client";

import React, { useState } from "react";
import type { BasePaymentButtonProps } from "./types";
import { isNextControlFlowError } from "../../utils/redirect-error";
import {
  fetchMollieRedirect,
  followMollieRedirect,
} from "../../connectors/mollie/redirect";

export interface MolliePaymentButtonProps extends BasePaymentButtonProps {
  /** Cart — its `id` is used to re-read the active session's 3DS redirect. */
  cart: {
    id: string;
    payment_collection?: {
      payment_sessions?: Array<{ status: string; data?: Record<string, unknown> }>;
    } | null;
  };
  /** Medusa backend URL (e.g. NEXT_PUBLIC_MEDUSA_BACKEND_URL). */
  backendUrl: string;
  /** Medusa publishable API key (e.g. NEXT_PUBLIC_MEDUSA_PUBLISHABLE_KEY). */
  publishableKey: string;
}

/**
 * Mollie place-order button. The card was already tokenized in-page by
 * `MollieWrapper` and the cardToken persisted on the session. On click it calls
 * `onPlaceOrder()`; Mollie one-off cards come back `requires_more`, so on the
 * resulting error it reads the persisted 3DS redirect and follows it. After the
 * shopper returns, the host's Mollie return handler finalises the order.
 */
export function MolliePaymentButton({
  cart,
  notReady,
  onPlaceOrder,
  buttonComponent: Button,
  backendUrl,
  publishableKey,
  "data-testid": dataTestId,
}: MolliePaymentButtonProps) {
  const [submitting, setSubmitting] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const session = cart.payment_collection?.payment_sessions?.find(
    (s) => s.status === "pending" || s.status === "requires_more"
  );
  // The MollieWrapper must have tokenized the card and persisted the cardToken
  // on the session before the order can be placed.
  const hasToken = !!(session?.data as Record<string, unknown> | undefined)?.cardToken;

  const handlePayment = async () => {
    setSubmitting(true);
    setErrorMessage(null);
    try {
      // Success (no 3DS): onPlaceOrder() server-redirects to the confirmation
      // page, so this navigates away.
      await onPlaceOrder();
    } catch (err: any) {
      // Don't swallow the success-path Next redirect.
      if (isNextControlFlowError(err)) throw err;
      // requires_more → the plugin persisted the 3DS redirect; follow it.
      const redirect = await fetchMollieRedirect(cart.id, { backendUrl, publishableKey });
      if (followMollieRedirect(redirect)) return;
      setErrorMessage(err?.message || "Payment failed");
      setSubmitting(false);
    }
  };

  const btnProps = {
    disabled: notReady || !hasToken || submitting,
    onClick: handlePayment,
    "data-testid": dataTestId,
    type: "button" as const,
  };
  const label = hasToken
    ? submitting
      ? "Processing…"
      : "Place order"
    : "Enter card details first";

  return (
    <>
      {Button ? (
        <Button {...btnProps} size="large" isLoading={submitting}>
          {hasToken ? "Place order" : "Enter card details first"}
        </Button>
      ) : (
        <button {...btnProps} className="payment-btn">
          {label}
        </button>
      )}
      {errorMessage && (
        <div className="payment-error" style={{ color: "#c00", marginTop: 8 }}>
          {errorMessage}
        </div>
      )}
    </>
  );
}
