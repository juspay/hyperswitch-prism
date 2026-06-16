"use client";

import React, { useState } from "react";
import type { BasePaymentButtonProps } from "./types";
import { isNextControlFlowError } from "../../utils/redirect-error";

/**
 * Manual / test payment button.
 *
 * Simply places the order without any additional payment authorization.
 */
export function ManualTestPaymentButton({
  notReady,
  onPlaceOrder,
  buttonComponent: Button,
  "data-testid": dataTestId,
}: BasePaymentButtonProps) {
  const [submitting, setSubmitting] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const handlePayment = async () => {
    setSubmitting(true);
    setErrorMessage(null);

    try {
      await onPlaceOrder();
    } catch (err: any) {
      if (isNextControlFlowError(err)) throw err;
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
