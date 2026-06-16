"use client";

import React, { useState } from "react";
import { useStripe, useElements } from "@stripe/react-stripe-js";
import type { StripePaymentButtonProps } from "./types";
import { isNextControlFlowError } from "../../utils/redirect-error";

/**
 * Stripe payment button.
 *
 * Must be rendered inside a Stripe `<Elements>` provider (provided by the
 * host application). Calls `stripe.confirmCardPayment` with the client secret
 * extracted from the pending payment session, then places the order.
 */
export function StripePaymentButton({
  notReady,
  cart,
  onPlaceOrder,
  buttonComponent: Button,
  "data-testid": dataTestId,
}: StripePaymentButtonProps) {
  const [submitting, setSubmitting] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const stripe = useStripe();
  const elements = useElements();
  const card = elements?.getElement("card");

  const session = cart?.payment_collection?.payment_sessions?.find(
    (s) => s.status === "pending"
  );

  const disabled = !stripe || !elements ? true : false;

  const handlePayment = async () => {
    setSubmitting(true);
    setErrorMessage(null);

    if (!stripe || !elements || !card || !cart) {
      setSubmitting(false);
      return;
    }

    const clientSecret = session?.data?.client_secret as string | undefined;
    if (!clientSecret) {
      setErrorMessage("Missing Stripe client secret.");
      setSubmitting(false);
      return;
    }

    try {
      const { error, paymentIntent } = await stripe.confirmCardPayment(
        clientSecret,
        {
          payment_method: { card },
        }
      );

      if (error) {
        const pi = error.payment_intent;
        if (
          (pi && pi.status === "requires_capture") ||
          (pi && pi.status === "succeeded")
        ) {
          await onPlaceOrder();
        } else {
          setErrorMessage(error.message || "Stripe payment failed");
        }
        setSubmitting(false);
        return;
      }

      if (
        paymentIntent &&
        (paymentIntent.status === "requires_capture" ||
          paymentIntent.status === "succeeded")
      ) {
        await onPlaceOrder();
      }
    } catch (err: any) {
      if (isNextControlFlowError(err)) throw err;
      setErrorMessage(err.message || "Payment failed");
    } finally {
      setSubmitting(false);
    }
  };

  const btnProps = {
    disabled: disabled || notReady || submitting,
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
