"use client";

import { useEffect, useRef } from "react";
import "@adyen/adyen-web/styles/adyen.css";

const AUTHORIZED_CODES = new Set(["Authorised", "Pending", "Received"]);

interface AdyenWrapperProps {
  /** Payment session returned by initiate() or Medusa payment session */
  sessionData: Record<string, any>;
  /** Called when shopper submits the drop-in (advanced flow) */
  onSubmit?: (paymentData: unknown) => void;
  /** Called when session-based payment fails (sessions flow) */
  onPaymentFailed?: (result: any) => void;
  /**
   * Called when Adyen reports an authorised resultCode — use for UI feedback
   * (e.g. show "Payment confirmed — click Place Order"). Order placement is
   * handled by the Place Order button, not this callback.
   */
  onPaymentCompleted?: (result: any) => void;
  /**
   * Called when the Adyen session reaches a definitive authorised state.
   * Use this to enable the Place Order button in the review step.
   */
  onAuthorized?: () => void;
  /** Called on Adyen-level errors */
  onError: (error: Error) => void;
}

/**
 * React component wrapper around Adyen Web v6.
 *
 * Sessions flow: after the drop-in reports resultCode "Authorised" (or
 * "Pending"/"Received"), onPaymentCompleted is called for optional UI feedback.
 * Order placement is the responsibility of the host's Place Order button.
 *
 * Advanced/drop-in flow: onSubmit receives the payment state for server-side
 * processing.
 */
export function AdyenWrapper({
  sessionData,
  onSubmit,
  onPaymentFailed,
  onPaymentCompleted,
  onAuthorized,
  onError,
}: AdyenWrapperProps) {
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let cancelled = false;
    let mountedComponent: any = null;

    const init = async () => {
      try {
        const { AdyenCheckout, Card, Dropin } = await import("@adyen/adyen-web");

        if (cancelled || !containerRef.current) return;

        const isSessionsFlow = !!(
          sessionData.session?.id || sessionData.session?.sessionData
        );

        if (isSessionsFlow) {
          const checkout = await AdyenCheckout({
            session: sessionData.session,
            clientKey: sessionData.clientKey ?? "",
            environment: "test",
            analytics: { enabled: false },
            amount: {
              value: sessionData.minorAmount ?? 0,
              currency: sessionData.currency ?? "EUR",
            },
            countryCode: sessionData.countryCode ?? "GB",
            showPayButton: true,
            onPaymentCompleted: (result: any) => {
              if (AUTHORIZED_CODES.has(result?.resultCode)) {
                onPaymentCompleted?.(result);
                onAuthorized?.();
              } else {
                onPaymentFailed?.(result);
                onError(new Error(`Adyen payment not authorised: ${result?.resultCode}`));
              }
            },
            onPaymentFailed: (result: any) => {
              onPaymentFailed?.(result);
              onError(new Error(`Adyen payment failed: ${result?.resultCode}`));
            },
            onError: (error: any) => {
              onError(error instanceof Error ? error : new Error(String(error)));
            },
          });

          mountedComponent = new Card(checkout, {
            hasHolderName: true,
            holderNameRequired: false,
            brands: ["visa", "mc", "amex", "discover"],
          });

          mountedComponent.mount(containerRef.current);
        } else {
          // Advanced / drop-in flow
          const checkout = await AdyenCheckout({
            environment: "test",
            analytics: { enabled: false },
            clientKey: sessionData.clientKey ?? "",
            countryCode: sessionData.countryCode ?? "GB",
            paymentMethodsResponse: sessionData.paymentMethods ?? {},
            amount: {
              value: sessionData.minorAmount ?? 0,
              currency: sessionData.currency ?? "EUR",
            },
            onSubmit: (state: any) => {
              onSubmit?.(state.data);
            },
            onError: (error: any) => {
              onError(error instanceof Error ? error : new Error(String(error)));
            },
          });

          mountedComponent = new Dropin(checkout);
          mountedComponent.mount(containerRef.current);
        }
      } catch (err) {
        if (!cancelled) {
          onError(err instanceof Error ? err : new Error(String(err)));
        }
      }
    };

    init();

    return () => {
      cancelled = true;
      if (mountedComponent?.unmount) {
        mountedComponent.unmount();
      }
    };
  }, [sessionData, onSubmit, onPaymentFailed, onPaymentCompleted, onError]);

  return <div ref={containerRef} className="adyen-payment-container" />;
}
