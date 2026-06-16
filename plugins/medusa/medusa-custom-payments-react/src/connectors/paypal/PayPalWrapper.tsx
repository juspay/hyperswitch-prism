"use client";

import { useEffect, useRef, useState } from "react";
import { loadPayPalScript } from "./utils";

interface PayPalWrapperProps {
  /** PayPal client id */
  clientId: string;
  /** Currency code */
  currency: string;
  /** Amount in major unit (e.g. 100 for $100) */
  amount: number;
  /** Sandbox or production */
  environment?: "sandbox" | "production";
  /**
   * Read the shopper's shipping address from the approved PayPal order and
   * include it as `shippingAddress` in the submitted payment data. Must match
   * the provider's `connectorConfig.includeShippingData`. Defaults to false.
   */
  includeShippingData?: boolean;
  /**
   * Read the payer's details from the approved PayPal order and include them
   * as `customer` in the submitted payment data. Must match the provider's
   * `connectorConfig.includeCustomerData`. Defaults to false.
   */
  includeCustomerData?: boolean;
  /** Called when PayPal button is clicked — must resolve to a real PayPal order ID */
  onCreateOrder: () => Promise<string>;
  /** Called when PayPal approves the payment */
  onSubmit: (paymentData: unknown) => void;
  /** Called on PayPal-level errors */
  onError: (error: Error) => void;
}

/* Maps the PayPal order's shipping block to the provider's address shape */
function toShippingAddress(shipping: any): Record<string, unknown> | undefined {
  const address = shipping?.address;
  if (!address) return undefined;
  const fullName = (shipping.name?.full_name ?? "").trim();
  const [firstName, ...rest] = fullName.split(/\s+/);
  return {
    ...(firstName ? { firstName } : {}),
    ...(rest.length ? { lastName: rest.join(" ") } : {}),
    line1: address.address_line_1,
    line2: address.address_line_2,
    city: address.admin_area_2,
    state: address.admin_area_1,
    zipCode: address.postal_code,
    country_code: address.country_code,
  };
}

/* Maps the PayPal order's payer block to the provider's customer shape */
function toCustomer(payer: any): Record<string, unknown> | undefined {
  if (!payer) return undefined;
  const name = [payer.name?.given_name, payer.name?.surname]
    .filter(Boolean)
    .join(" ");
  if (!name && !payer.email_address) return undefined;
  return {
    ...(name ? { name } : {}),
    ...(payer.email_address ? { email: payer.email_address } : {}),
    ...(payer.phone?.phone_number?.national_number
      ? { phone: payer.phone.phone_number.national_number }
      : {}),
  };
}

/**
 * React component wrapper around PayPal Buttons.
 *
 * Handles React Strict Mode by tracking mount state with refs and avoiding
 * double SDK initialisation (which triggers zoid destruction errors).
 */
export function PayPalWrapper({
  clientId,
  currency,
  amount,
  environment = "production",
  includeShippingData = false,
  includeCustomerData = false,
  onCreateOrder,
  onSubmit,
  onError,
}: PayPalWrapperProps) {
  const walletRef = useRef<HTMLDivElement>(null);
  const numberRef = useRef<HTMLDivElement>(null);
  const expiryRef = useRef<HTMLDivElement>(null);
  const cvvRef = useRef<HTMLDivElement>(null);
  const fallbackRef = useRef<HTMLDivElement>(null);

  const walletButtonsRef = useRef<any>(null);
  const fallbackButtonsRef = useRef<any>(null);
  const cardFieldsRef = useRef<any>(null);
  const renderedRef = useRef(false);

  /* null = checking, true = CardFields shown, false = fell back to Buttons */
  const [cardEligible, setCardEligible] = useState<boolean | null>(null);
  const [submitting, setSubmitting] = useState(false);

  /* Keep latest callbacks accessible inside effect without re-triggering it */
  const onSubmitRef = useRef(onSubmit);
  const onErrorRef = useRef(onError);
  const onCreateOrderRef = useRef(onCreateOrder);

  useEffect(() => { onSubmitRef.current = onSubmit; }, [onSubmit]);
  useEffect(() => { onErrorRef.current = onError; }, [onError]);
  useEffect(() => { onCreateOrderRef.current = onCreateOrder; }, [onCreateOrder]);

  useEffect(() => {
    /* Prevent double render in React Strict Mode */
    if (renderedRef.current) return;
    renderedRef.current = true;

    let closed = false;

    const createOrder = () => onCreateOrderRef.current();
    const handleError = (err: any) =>
      onErrorRef.current(err instanceof Error ? err : new Error(String(err)));

    /* Wallet (branded) approval forwards shipping/customer only when enabled */
    const walletOnApprove = async (data: any, actions: any) => {
      const paymentData: Record<string, unknown> = {
        orderId: data.orderID,
        payerId: data.payerID,
      };
      if ((includeShippingData || includeCustomerData) && actions?.order?.get) {
        try {
          const order = await actions.order.get();
          if (includeShippingData) {
            const shippingAddress = toShippingAddress(
              order?.purchase_units?.[0]?.shipping
            );
            if (shippingAddress) paymentData.shippingAddress = shippingAddress;
          }
          if (includeCustomerData) {
            const customer = toCustomer(order?.payer);
            if (customer) paymentData.customer = customer;
          }
        } catch (_) {
          /* best effort */
        }
      }
      onSubmitRef.current(paymentData);
    };

    loadPayPalScript(clientId, currency, environment)
      .then((paypal) => {
        if (closed) return;

        /* Branded PayPal wallet button only (no card funding → no guest form) */
        if (walletRef.current) {
          const walletButtons = paypal.Buttons({
            fundingSource: paypal.FUNDING?.PAYPAL,
            style: { layout: "vertical" },
            createOrder,
            onApprove: walletOnApprove,
            onError: handleError,
          });
          if (walletButtons.isEligible()) {
            walletButtonsRef.current = walletButtons;
            walletButtons.render(walletRef.current);
          }
        }

        /* Card path via Advanced Card Fields — renders only number/expiry/cvv,
           no billing-address block. */
        const cardFields = paypal.CardFields({
          createOrder,
          onApprove: (data: any) =>
            onSubmitRef.current({ orderId: data.orderID }),
          onError: handleError,
        });

        if (cardFields.isEligible()) {
          cardFieldsRef.current = cardFields;
          setCardEligible(true);
          /* Containers exist on first render (cardEligible !== false) */
          if (numberRef.current) cardFields.NumberField().render(numberRef.current);
          if (expiryRef.current) cardFields.ExpiryField().render(expiryRef.current);
          if (cvvRef.current) cardFields.CVVField().render(cvvRef.current);
        } else {
          /* ACDC not enabled on this account — fall back to the Buttons guest
             card form (which still shows a billing block). Enable Advanced
             Credit & Debit Card Payments to remove it. */
          // eslint-disable-next-line no-console
          console.warn(
            "[PayPalWrapper] CardFields not eligible (enable Advanced Card Payments); falling back to Buttons guest card form"
          );
          setCardEligible(false);
          if (fallbackRef.current) {
            const fallbackButtons = paypal.Buttons({
              style: { layout: "vertical" },
              createOrder,
              onApprove: walletOnApprove,
              onError: handleError,
            });
            if (fallbackButtons.isEligible()) {
              fallbackButtonsRef.current = fallbackButtons;
              fallbackButtons.render(fallbackRef.current);
            }
          }
        }
      })
      .catch((err) => {
        if (!closed) handleError(err);
      });

    return () => {
      closed = true;
      renderedRef.current = false;
      for (const ref of [walletButtonsRef, fallbackButtonsRef]) {
        if (ref.current && typeof ref.current.close === "function") {
          try { ref.current.close(); } catch (_) { /* noop */ }
        }
        ref.current = null;
      }
      cardFieldsRef.current = null;
    };
  /* eslint-disable-next-line react-hooks/exhaustive-deps */
  }, [clientId, currency, amount, environment]);

  const handleCardPay = async () => {
    if (!cardFieldsRef.current) return;
    setSubmitting(true);
    try {
      /* Resolves after 3DS/SCA; onApprove fires with the orderID */
      await cardFieldsRef.current.submit();
    } catch (err) {
      onErrorRef.current(err instanceof Error ? err : new Error(String(err)));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="paypal-payment-container">
      <div ref={walletRef} className="paypal-wallet-buttons" />

      {cardEligible !== false && (
        <div className="paypal-card-fields" style={{ marginTop: 16 }}>
          <div
            className="paypal-card-divider"
            style={{
              display: "flex",
              alignItems: "center",
              gap: 12,
              color: "#6b7280",
              fontSize: 13,
              margin: "4px 0 12px",
            }}
          >
            <span style={{ flex: 1, height: 1, background: "#e5e7eb" }} />
            Or pay with card
            <span style={{ flex: 1, height: 1, background: "#e5e7eb" }} />
          </div>
          <div ref={numberRef} className="paypal-card-field" />
          <div style={{ display: "flex", gap: 8 }}>
            <div ref={expiryRef} className="paypal-card-field" style={{ flex: 1 }} />
            <div ref={cvvRef} className="paypal-card-field" style={{ flex: 1 }} />
          </div>
          <button
            type="button"
            className="paypal-card-submit"
            disabled={submitting || cardEligible === null}
            onClick={handleCardPay}
            style={{
              width: "100%",
              marginTop: 12,
              padding: "14px 20px",
              border: "none",
              borderRadius: 24,
              background: submitting ? "#7fb3d5" : "#0070ba",
              color: "#fff",
              fontSize: 16,
              fontWeight: 600,
              cursor: submitting ? "not-allowed" : "pointer",
              transition: "background 0.15s ease",
            }}
            onMouseOver={(e) => {
              if (!submitting) e.currentTarget.style.background = "#005ea6";
            }}
            onMouseOut={(e) => {
              if (!submitting) e.currentTarget.style.background = "#0070ba";
            }}
          >
            {submitting ? "Processing…" : "Pay"}
          </button>
        </div>
      )}

      <div ref={fallbackRef} className="paypal-buttons-container" />
    </div>
  );
}
