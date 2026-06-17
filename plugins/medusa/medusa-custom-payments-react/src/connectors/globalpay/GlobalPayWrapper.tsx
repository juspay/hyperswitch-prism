"use client";

import { useEffect, useRef, useState } from "react";
import { loadGlobalPayScript } from "./utils";

interface GlobalPayWrapperProps {
  /** Access token from the payment session (used to configure GlobalPayments) */
  accessToken: string;
  /** Environment: sandbox (default) or production */
  environment?: "sandbox" | "production";
  /**
   * Called after card tokenization succeeds. Should persist the paymentReference
   * in the Medusa session (via re-initiation) before the user clicks Place Order.
   */
  onSubmit: (paymentData: { paymentReference: string }) => Promise<void>;
  /** Called on any GlobalPay-level or sequencing error */
  onError: (error: Error) => void;
}

/**
 * React component wrapper around GlobalPay/Heartland Credit Card Form.
 *
 * After successful card tokenization the wrapper calls onSubmit() to persist
 * the paymentReference, then shows a prompt for the user to click Place Order.
 * Order placement is the responsibility of the host's Place Order button.
 */
export function GlobalPayWrapper({
  accessToken,
  environment = "sandbox",
  onSubmit,
  onError,
}: GlobalPayWrapperProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [status, setStatus] = useState<"idle" | "submitting" | "done" | "error">("idle");
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    let cardForm: any = null;

    const init = async () => {
      try {
        const GlobalPayments = await loadGlobalPayScript();

        if (cancelled || !containerRef.current) return;

        GlobalPayments.configure({ accessToken, env: environment });

        cardForm = GlobalPayments.creditCard.form(containerRef.current);

        cardForm.on("token-success", async (resp: any) => {
          const token = resp?.paymentReference;

          if (!token) {
            const msg = "Token creation failed - no payment reference";
            setErrorMsg(msg);
            setStatus("error");
            onError(new Error(msg));
            return;
          }

          const paymentData = { paymentReference: token };

          try {
            setStatus("submitting");
            setErrorMsg(null);
            await onSubmit(paymentData);
            setStatus("done");
          } catch (err) {
            if (!cancelled) {
              const msg = err instanceof Error ? err.message : String(err);
              setErrorMsg(msg);
              setStatus("error");
              onError(err instanceof Error ? err : new Error(msg));
            }
          }
        });

        cardForm.on("token-error", (err: any) => {
          let msg = "Payment tokenization failed";
          if (
            err?.error_code === "ACTION_NOT_AUTHORIZED" ||
            err?.detailed_error_code === "40022"
          ) {
            msg = "Access token lacks tokenization permissions. Contact GlobalPay support to enable PMT_POST_Create permission.";
          } else if (err?.reason || err?.detailed_error_description) {
            msg = err.reason || err.detailed_error_description;
          }
          setErrorMsg(msg);
          setStatus("error");
          onError(new Error(msg));
        });

        GlobalPayments.on("error", (err: any) => {
          const msg = err?.message || "An error occurred";
          setErrorMsg(msg);
          setStatus("error");
          onError(new Error(msg));
        });
      } catch (err) {
        if (!cancelled) {
          const msg = err instanceof Error ? err.message : String(err);
          setErrorMsg(msg);
          setStatus("error");
          onError(err instanceof Error ? err : new Error(msg));
        }
      }
    };

    init();

    return () => {
      cancelled = true;
      if (containerRef.current) {
        containerRef.current.innerHTML = "";
      }
    };
  }, [accessToken, environment, onSubmit, onError]);

  return (
    <div className="globalpay-payment-container">
      <div ref={containerRef} className="globalpay-form-container" />
      {status === "submitting" && (
        <div className="globalpay-status-message text-sm text-blue-600 mt-2">
          Saving card details…
        </div>
      )}
      {status === "done" && (
        <div className="globalpay-status-message text-sm text-green-600 mt-2">
          Card saved — click Place Order to complete your purchase.
        </div>
      )}
      {status === "error" && errorMsg && (
        <div className="globalpay-error-message text-sm text-red-600 mt-2">
          {errorMsg}
        </div>
      )}
    </div>
  );
}
