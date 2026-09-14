"use client";

import { useEffect, useRef, useState } from "react";
import { loadMollieScript } from "./utils";

interface MollieWrapperProps {
  /** Public Mollie Profile ID (pfl_...), delivered in the payment session. */
  profileId: string;
  /** Mollie test mode (default true). A testmode token pairs with the Test API key. */
  testmode?: boolean;
  /** Locale for the Mollie Components fields (default en_US). */
  locale?: string;
  /**
   * Called after the card is tokenized in-page. Should persist the cardToken on
   * the Medusa session (via re-initiation) and trigger authorization.
   */
  onSubmit: (paymentData: { cardToken: string }) => Promise<void>;
  /** Called on any Mollie-level or sequencing error. */
  onError: (error: Error) => void;
}

/**
 * In-page Mollie card fields via Mollie Components (mollie.js).
 *
 * Mounts the iframed cardHolder / cardNumber / expiryDate / verificationCode
 * fields, and on "Pay" calls mollie.createToken() to obtain a single-use
 * cardToken which is handed to onSubmit(). The card data never touches our
 * server (PCI SAQ-A). Note: one-off card payments still complete via a 3DS
 * redirect handled by the host after onSubmit.
 */
export function MollieWrapper({
  profileId,
  testmode = true,
  locale = "en_US",
  onSubmit,
  onError,
}: MollieWrapperProps) {
  const holderRef = useRef<HTMLDivElement>(null);
  const numberRef = useRef<HTMLDivElement>(null);
  const expiryRef = useRef<HTMLDivElement>(null);
  const cvcRef = useRef<HTMLDivElement>(null);
  const mollieRef = useRef<any>(null);

  const [ready, setReady] = useState(false);
  const [status, setStatus] = useState<"idle" | "submitting" | "done" | "error">("idle");
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    const components: any[] = [];

    const init = async () => {
      try {
        if (!profileId) {
          throw new Error("Mollie profileId missing from session data");
        }
        const Mollie = await loadMollieScript();
        if (cancelled) return;

        const mollie = Mollie(profileId, { locale, testmode });
        mollieRef.current = mollie;

        const mount = (type: string, el: HTMLDivElement | null) => {
          if (!el) return;
          const component = mollie.createComponent(type);
          component.mount(el);
          components.push(component);
        };

        mount("cardHolder", holderRef.current);
        mount("cardNumber", numberRef.current);
        mount("expiryDate", expiryRef.current);
        mount("verificationCode", cvcRef.current);

        if (!cancelled) setReady(true);
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
      components.forEach((c) => {
        try {
          c.unmount();
        } catch {
          /* ignore */
        }
      });
    };
  }, [profileId, testmode, locale, onError]);

  const handlePay = async () => {
    const mollie = mollieRef.current;
    if (!mollie) return;
    setStatus("submitting");
    setErrorMsg(null);
    try {
      const { token, error } = await mollie.createToken();
      if (error) {
        throw new Error(error.message ?? "Card tokenization failed");
      }
      if (!token) {
        throw new Error("Mollie returned no card token");
      }
      await onSubmit({ cardToken: token });
      setStatus("done");
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setErrorMsg(msg);
      setStatus("error");
      onError(err instanceof Error ? err : new Error(msg));
    }
  };

  const fieldStyle: React.CSSProperties = {
    border: "1px solid #ddd",
    borderRadius: 6,
    padding: "10px 12px",
    background: "#fff",
    minHeight: 40,
  };
  const labelStyle: React.CSSProperties = {
    fontSize: 12,
    color: "#666",
    margin: "0 0 4px",
  };

  return (
    <div className="mollie-payment-container" style={{ display: "flex", flexDirection: "column", gap: 12 }}>
      <div>
        <p style={labelStyle}>Cardholder name</p>
        <div ref={holderRef} style={fieldStyle} />
      </div>
      <div>
        <p style={labelStyle}>Card number</p>
        <div ref={numberRef} style={fieldStyle} />
      </div>
      <div style={{ display: "flex", gap: 12 }}>
        <div style={{ flex: 1 }}>
          <p style={labelStyle}>Expiry</p>
          <div ref={expiryRef} style={fieldStyle} />
        </div>
        <div style={{ flex: 1 }}>
          <p style={labelStyle}>CVC</p>
          <div ref={cvcRef} style={fieldStyle} />
        </div>
      </div>

      <button
        type="button"
        data-testid="mollie-pay"
        onClick={handlePay}
        disabled={!ready || status === "submitting" || status === "done"}
        style={{
          padding: "12px 20px",
          background: !ready || status === "submitting" ? "#9bb4ff" : "#0b4dff",
          color: "#fff",
          border: "none",
          borderRadius: 8,
          fontWeight: 600,
          fontSize: 15,
          cursor: !ready || status === "submitting" ? "not-allowed" : "pointer",
        }}
      >
        {status === "submitting" ? "Processing…" : "Pay"}
      </button>

      {status === "done" && (
        <div style={{ fontSize: 13, color: "#276749" }}>
          Card accepted — completing payment…
        </div>
      )}
      {status === "error" && errorMsg && (
        <div data-testid="mollie-error" style={{ fontSize: 13, color: "#c00" }}>
          {errorMsg}
        </div>
      )}
    </div>
  );
}
