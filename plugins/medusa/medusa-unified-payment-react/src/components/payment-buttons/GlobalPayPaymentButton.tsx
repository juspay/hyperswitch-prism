"use client";

import React, { useState } from "react";
import type { GlobalPayPaymentButtonProps } from "./types";

/**
 * GlobalPay payment button.
 *
 * Reads the tokenized `paymentReference` from sessionStorage (written by
 * GlobalPayWrapper when the card form is submitted) and forwards it to the
 * cart metadata before placing the order.
 */
export function GlobalPayPaymentButton({
  notReady,
  onPlaceOrder,
  onUpdateCart,
  buttonComponent: Button,
  "data-testid": dataTestId,
}: GlobalPayPaymentButtonProps) {
  const [submitting, setSubmitting] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const handlePayment = async () => {
    setSubmitting(true);
    setErrorMessage(null);

    const paymentReference =
      typeof window !== "undefined"
        ? sessionStorage.getItem("globalpay_payment_reference")
        : null;

    if (!paymentReference) {
      setErrorMessage(
        "Payment token not found. Please go back to the payment step and re-enter your card details."
      );
      setSubmitting(false);
      return;
    }

    try {
      if (onUpdateCart) {
        await onUpdateCart({ globalpay_payment_reference: paymentReference });
      }
      await onPlaceOrder();
    } catch (err: any) {
      setErrorMessage(err.message || "Payment failed");
    } finally {
      setSubmitting(false);
    }
  };

  const btnProps = {
    disabled: notReady || submitting,
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
    </>
  );
}
