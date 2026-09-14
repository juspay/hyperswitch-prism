"use client";

import { useEffect, useRef, useState } from "react";
import { decodeCaptureContext, loadCybersourceFlexScript } from "./utils";

interface CybersourceWrapperProps {
  /** Capture-context JWT from the payment session */
  captureContext: string;
  /** Flex library URL (optional — falls back to decoding the capture context) */
  clientLibrary?: string;
  /** SRI hash for the Flex library (optional) */
  clientLibraryIntegrity?: string;
  /**
   * Called after the Microform produces a transient token. Should persist the
   * token in the Medusa session (via re-initiation) before Place Order.
   */
  onSubmit: (paymentData: { transientToken: string }) => Promise<void>;
  /** Called on any Flex-level or sequencing error */
  onError: (error: Error) => void;
}

const FIELD_STYLE: React.CSSProperties = {
  height: 40,
  border: "1px solid #ddd",
  borderRadius: 6,
  padding: "0 12px",
  marginBottom: 12,
  background: "#fff",
};

/**
 * React wrapper around Cybersource Flex Microform (v2, card).
 *
 * Microform hosts the card number and security code as iframes. The expiry is a
 * plain input passed to `createToken`. On success we hand the transient token to
 * `onSubmit`, which persists it in the session; order placement is the host's
 * Place Order button.
 */
export function CybersourceWrapper({
  captureContext,
  clientLibrary,
  clientLibraryIntegrity,
  onSubmit,
  onError,
}: CybersourceWrapperProps) {
  const microformRef = useRef<any>(null);
  const [ready, setReady] = useState(false);
  const [expMonth, setExpMonth] = useState("");
  const [expYear, setExpYear] = useState("");
  const [status, setStatus] = useState<"idle" | "submitting" | "done" | "error">("idle");
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    const init = async () => {
      try {
        const decoded = decodeCaptureContext(captureContext);
        const libUrl = clientLibrary || decoded.clientLibrary || "";
        const libIntegrity = clientLibraryIntegrity || decoded.clientLibraryIntegrity;

        const Flex = await loadCybersourceFlexScript(libUrl, libIntegrity);
        if (cancelled) return;

        const flex = new Flex(captureContext);
        const microform = flex.microform({ styles: { input: { "font-size": "14px" } } });
        microform.createField("number", { placeholder: "Card number" }).load("#cyb-number");
        microform.createField("securityCode", { placeholder: "CVV" }).load("#cyb-cvv");

        microformRef.current = microform;
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
      microformRef.current = null;
    };
  }, [captureContext, clientLibrary, clientLibraryIntegrity, onError]);

  const handlePay = () => {
    const microform = microformRef.current;
    if (!microform) return;

    setStatus("submitting");
    setErrorMsg(null);

    const options = {
      expirationMonth: expMonth.padStart(2, "0"),
      expirationYear: expYear.length === 2 ? `20${expYear}` : expYear,
    };

    microform.createToken(options, async (err: any, token: string) => {
      if (err || !token) {
        const msg = err?.message || "Cybersource tokenisation failed";
        setErrorMsg(msg);
        setStatus("error");
        onError(new Error(msg));
        return;
      }
      try {
        await onSubmit({ transientToken: token });
        setStatus("done");
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        setErrorMsg(msg);
        setStatus("error");
        onError(e instanceof Error ? e : new Error(msg));
      }
    });
  };

  return (
    <div className="cybersource-payment-container">
      <label style={{ fontSize: 12, color: "#666" }}>Card number</label>
      <div id="cyb-number" style={FIELD_STYLE} />
      <div style={{ display: "flex", gap: 12 }}>
        <div style={{ flex: 1 }}>
          <label style={{ fontSize: 12, color: "#666" }}>Exp. month (MM)</label>
          <input
            value={expMonth}
            onChange={(e) => setExpMonth(e.target.value)}
            placeholder="MM"
            inputMode="numeric"
            style={{ ...FIELD_STYLE, width: "100%", boxSizing: "border-box" }}
          />
        </div>
        <div style={{ flex: 1 }}>
          <label style={{ fontSize: 12, color: "#666" }}>Exp. year (YYYY)</label>
          <input
            value={expYear}
            onChange={(e) => setExpYear(e.target.value)}
            placeholder="YYYY"
            inputMode="numeric"
            style={{ ...FIELD_STYLE, width: "100%", boxSizing: "border-box" }}
          />
        </div>
        <div style={{ flex: 1 }}>
          <label style={{ fontSize: 12, color: "#666" }}>CVV</label>
          <div id="cyb-cvv" style={FIELD_STYLE} />
        </div>
      </div>

      <button
        type="button"
        onClick={handlePay}
        disabled={!ready || status === "submitting"}
        data-testid="cybersource-pay"
        style={{
          width: "100%",
          padding: "12px 16px",
          borderRadius: 6,
          border: "none",
          background: !ready || status === "submitting" ? "#999" : "#222",
          color: "#fff",
          fontWeight: 600,
          cursor: !ready || status === "submitting" ? "default" : "pointer",
        }}
      >
        {status === "submitting" ? "Saving card…" : "Save card"}
      </button>

      {status === "done" && (
        <div className="cybersource-status-message" style={{ fontSize: 13, color: "#16a34a", marginTop: 8 }}>
          Card saved — click Place Order to complete your purchase.
        </div>
      )}
      {status === "error" && errorMsg && (
        <div className="cybersource-error-message" style={{ fontSize: 13, color: "#dc2626", marginTop: 8 }}>
          {errorMsg}
        </div>
      )}
    </div>
  );
}
