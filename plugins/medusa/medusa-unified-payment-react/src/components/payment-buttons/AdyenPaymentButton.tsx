"use client";

import React, { useState } from "react";
import type { BasePaymentButtonProps } from "./types";

/**
 * Adyen payment button.
 *
 * Adyen Drop-in handles authorization client-side; the button only needs to
 * place the order. The button is disabled until `isAuthorized` is true so the
 * customer can't click "Place order" before the drop-in has finished.
 */
export interface AdyenPaymentButtonProps extends BasePaymentButtonProps {
  /** True when the Adyen drop-in has reported an authorised resultCode */
  isAuthorized?: boolean;
}

export function AdyenPaymentButton({
  notReady,
  onPlaceOrder,
  buttonComponent: Button,
  isAuthorized = true,
  "data-testid": dataTestId,
}: AdyenPaymentButtonProps) {
  const [submitting, setSubmitting] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const handlePayment = async () => {
    setSubmitting(true);
    setErrorMessage(null);

    try {
      await onPlaceOrder();
    } catch (err: any) {
      setErrorMessage(err.message || "Payment failed");
    } finally {
      setSubmitting(false);
    }
  };

  const btnProps = {
    disabled: notReady || submitting || !isAuthorized,
    onClick: handlePayment,
    "data-testid": dataTestId,
    type: "button" as const,
  };

  return (
    <>
      {Button ? (
        <Button {...btnProps} size="large" isLoading={submitting}>
          Place order
        </Button>
      ) : (
        <button {...btnProps} className="payment-btn">
          {submitting ? "Processing…" : "Place order"}
        </button>
      )}
      {errorMessage && (
        <div className="payment-error" style={{ color: "#c00", marginTop: 8 }}>
          {errorMessage}
        </div>
      )}
      {!isAuthorized && !errorMessage && (
        <p style={{ color: "#666", fontSize: 13, marginTop: 8 }}>
          Complete the payment form above to enable order placement
        </p>
      )}
    </>
  );
}
