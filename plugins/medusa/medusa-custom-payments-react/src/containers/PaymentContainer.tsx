"use client";

import { useEffect, useRef, useState, useCallback } from "react";
import type { InitiateParams, InitiateResult, ConfirmResult } from "../types";
import { getConnector } from "../connectors";

interface PaymentContainerProps {
  /** Connector identifier, e.g. "adyen", "stripe", "paypal" */
  connector: string;
  /** Amount in major currency unit */
  amount: number;
  /** ISO 4217 currency code */
  currency: string;
  /** Medusa cart id */
  cartId: string;
  /** Backend URL that exposes /api/payments/initiate */
  apiBaseUrl?: string;
  /** Called when the user successfully completes payment */
  onSuccess?: (result: ConfirmResult) => void;
  /** Called on any error */
  onError?: (error: Error) => void;
  /** Custom loading UI */
  loadingComponent?: React.ReactNode;
  /** Custom error UI */
  errorComponent?: React.ReactNode;
}

/**
 * Generic payment container.
 *
 * Usage:
 * ```tsx
 * <PaymentContainer
 *   connector="adyen"
 *   amount={100}
 *   currency="USD"
 *   cartId="cart_xxx"
 *   onSuccess={(res) => router.push('/order/confirmed')}
 *   onError={(err) => toast.error(err.message)}
 * />
 * ```
 */
export function PaymentContainer({
  connector: connectorName,
  amount,
  currency,
  cartId,
  apiBaseUrl = "",
  onSuccess,
  onError,
  loadingComponent = <div>Loading payment method…</div>,
  errorComponent,
}: PaymentContainerProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [sessionData, setSessionData] = useState<InitiateResult | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const connector = getConnector(connectorName);

  /** 1. Initiate payment session on mount */
  useEffect(() => {
    let cancelled = false;

    const params: InitiateParams = {
      amount,
      currency_code: currency,
      cartId,
    };

    connector
      .initiate(params)
      .then((result) => {
        if (!cancelled) setSessionData(result);
      })
      .catch((err) => {
        if (!cancelled) {
          setError(err instanceof Error ? err : new Error(String(err)));
        }
      });

    return () => {
      cancelled = true;
      connector.destroy();
    };
  }, [connector, amount, currency, cartId]);

  /** 2. Render connector-specific UI once session data is ready */
  useEffect(() => {
    if (!sessionData || !containerRef.current) return;

    const containerId = containerRef.current.id;

    connector.render(containerId, sessionData, {
      onSubmit: async (paymentData) => {
        setSubmitting(true);
        try {
          const result = await connector.confirm({
            connector: connectorName,
            data: paymentData,
          });
          if (result.status === "succeeded" || result.status === "captured") {
            onSuccess?.(result);
          } else {
            onError?.(new Error(result.error || "Payment failed"));
          }
        } catch (err) {
          onError?.(err instanceof Error ? err : new Error(String(err)));
        } finally {
          setSubmitting(false);
        }
      },
      onError: (err) => {
        setError(err);
        onError?.(err);
      },
    });
  }, [sessionData, connector, connectorName, onSuccess, onError]);

  if (error) {
    return (
      <div className="payment-error">
        {errorComponent ?? (
          <p style={{ color: "#c00" }}>
            Payment error: {error.message}
          </p>
        )}
      </div>
    );
  }

  if (!sessionData) {
    return (
      <div className="payment-loading">{loadingComponent}</div>
    );
  }

  return (
    <div className="payment-container">
      <div
        ref={containerRef}
        id={`payment-${connectorName}-${cartId}`}
        style={{ minHeight: 80 }}
      />
      {submitting && (
        <div className="payment-submitting">Processing payment…</div>
      )}
    </div>
  );
}
