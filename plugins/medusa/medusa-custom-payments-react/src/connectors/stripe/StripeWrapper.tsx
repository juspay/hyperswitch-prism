"use client";

import { useMemo, useState } from "react";
import { loadStripe } from "@stripe/stripe-js";
import {
  Elements,
  CardElement,
  useStripe,
  useElements,
} from "@stripe/react-stripe-js";

/** Billing details forwarded to Stripe with the card confirmation. */
export interface StripeBillingDetails {
  name?: string;
  email?: string;
  phone?: string;
  address?: {
    line1?: string;
    line2?: string;
    city?: string;
    state?: string;
    postal_code?: string;
    country?: string;
  };
}

interface StripeWrapperProps {
  /** Publishable key (e.g. pk_test_...) */
  publishableKey: string;
  /** Client secret from initiate (e.g. pi_xx_secret_yy) */
  clientSecret: string;
  /** Called when the payment is authorized/succeeded — receives the PaymentIntent */
  onSubmit: (paymentData: unknown) => void;
  /** Called on Stripe-level errors */
  onError: (error: Error) => void;
  /** Optional billing details attached to the confirmation */
  billingDetails?: StripeBillingDetails;
}

const CARD_ELEMENT_OPTIONS = {
  hidePostalCode: true,
  style: {
    base: {
      fontSize: "16px",
      color: "#1a1a1a",
      fontFamily:
        '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif',
      "::placeholder": { color: "#9ca3af" },
    },
    invalid: { color: "#c0392b" },
  },
} as const;

/**
 * Inner form — must live inside <Elements> so the Stripe context hooks resolve.
 *
 * Uses the all-in-one <CardElement> + `confirmCardPayment` (Medusa's official
 * storefront pattern). Unlike the Payment Element's `confirmPayment`, this path
 * does not invoke Stripe's hCaptcha, so it completes under automation.
 */
function StripeCardForm({
  clientSecret,
  onSubmit,
  onError,
  billingDetails,
}: Omit<StripeWrapperProps, "publishableKey">) {
  const stripe = useStripe();
  const elements = useElements();

  const [submitting, setSubmitting] = useState(false);
  const [complete, setComplete] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const disabled = !stripe || !elements || !complete || submitting;

  const handleSubmit = async () => {
    if (!stripe || !elements) return;

    const card = elements.getElement(CardElement);
    if (!card) return;

    setSubmitting(true);
    setErrorMessage(null);

    try {
      const { error, paymentIntent } = await stripe.confirmCardPayment(
        clientSecret,
        {
          payment_method: {
            card,
            ...(billingDetails ? { billing_details: billingDetails } : {}),
          },
        }
      );

      if (error) {
        // Stripe surfaces an already-authorized intent on the error object when
        // the PI moved forward despite a non-fatal error — treat it as success
        // (matches Medusa's storefront handling).
        const pi = error.payment_intent;
        if (
          pi &&
          (pi.status === "requires_capture" || pi.status === "succeeded")
        ) {
          onSubmit(pi);
          return;
        }
        const message = error.message || "Stripe payment failed";
        setErrorMessage(message);
        onError(new Error(message));
        return;
      }

      if (
        paymentIntent &&
        (paymentIntent.status === "requires_capture" ||
          paymentIntent.status === "succeeded")
      ) {
        onSubmit(paymentIntent);
        return;
      }

      const message = `Unexpected payment status: ${
        paymentIntent?.status ?? "unknown"
      }`;
      setErrorMessage(message);
      onError(new Error(message));
    } catch (err) {
      const message =
        err instanceof Error ? err.message : "Stripe payment failed";
      setErrorMessage(message);
      onError(new Error(message));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="stripe-card-form">
      <div
        className="stripe-card-element"
        style={{
          padding: "14px 16px",
          border: "1px solid #e5e7eb",
          borderRadius: 8,
          background: "#fff",
        }}
      >
        <CardElement
          options={CARD_ELEMENT_OPTIONS}
          onChange={(e) => {
            setComplete(e.complete);
            if (e.error) setErrorMessage(e.error.message);
            else setErrorMessage(null);
          }}
        />
      </div>

      <button
        type="button"
        className="stripe-pay-button"
        disabled={disabled}
        onClick={handleSubmit}
        style={{
          width: "100%",
          marginTop: 16,
          padding: "14px 20px",
          border: "none",
          borderRadius: 24,
          background: disabled ? "#a5a1f5" : "#635bff",
          color: "#fff",
          fontSize: 16,
          fontWeight: 600,
          cursor: disabled ? "not-allowed" : "pointer",
          transition: "background 0.15s ease",
        }}
        onMouseOver={(e) => {
          if (!disabled) e.currentTarget.style.background = "#524dd6";
        }}
        onMouseOut={(e) => {
          if (!disabled) e.currentTarget.style.background = "#635bff";
        }}
      >
        {submitting ? "Processing…" : "Pay now"}
      </button>

      {errorMessage && (
        <div
          className="stripe-payment-error"
          style={{ color: "#c0392b", marginTop: 8, fontSize: 14 }}
        >
          {errorMessage}
        </div>
      )}
    </div>
  );
}

/**
 * React component wrapper around Stripe's Card Element.
 *
 * Mirrors Medusa's official storefront Stripe integration: a single
 * <CardElement> confirmed with `confirmCardPayment`, rather than the Payment
 * Element + `confirmPayment` (which is gated behind hCaptcha).
 */
export function StripeWrapper({
  publishableKey,
  clientSecret,
  onSubmit,
  onError,
  billingDetails,
}: StripeWrapperProps) {
  const stripePromise = useMemo(
    () => loadStripe(publishableKey),
    [publishableKey]
  );

  return (
    <div className="stripe-payment-container">
      <Elements stripe={stripePromise} options={{ clientSecret }}>
        <StripeCardForm
          clientSecret={clientSecret}
          onSubmit={onSubmit}
          onError={onError}
          billingDetails={billingDetails}
        />
      </Elements>
    </div>
  );
}
