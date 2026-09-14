"use client";

import { useState, type CSSProperties } from "react";

interface AuthorizedotnetWrapperProps {
  /**
   * Called with the raw card the buyer entered. Should persist it on the Medusa
   * session (via re-initiation) and trigger authorization. Authorize.Net's UCS
   * connector authorizes the raw PAN directly (PaymentMethodData::Card) — there
   * is no client-side tokenization step.
   */
  onSubmit: (paymentData: {
    cardNumber: string;
    cardExpMonth: string;
    cardExpYear: string;
    cardCvc: string;
  }) => Promise<void>;
  /** Called on any sequencing/authorization error. */
  onError: (error: Error) => void;
}

const inputStyle: CSSProperties = {
  width: "100%",
  height: 40,
  border: "1px solid #ddd",
  borderRadius: 6,
  padding: "0 12px",
  marginBottom: 12,
  boxSizing: "border-box",
  fontSize: 14,
};

/**
 * Authorize.Net raw-card form.
 *
 * The connector authorizes a raw card (no Accept.js opaque-token path exists in
 * the connector), so this collects the card in a plain form and hands it to
 * onSubmit() for a server-side authorize.
 */
export function AuthorizedotnetWrapper({
  onSubmit,
  onError,
}: AuthorizedotnetWrapperProps) {
  const [number, setNumber] = useState("");
  const [month, setMonth] = useState("");
  const [year, setYear] = useState("");
  const [cvc, setCvc] = useState("");
  const [status, setStatus] = useState<
    "idle" | "submitting" | "done" | "error"
  >("idle");
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  const handlePay = async () => {
    const cardNumber = number.replace(/\s+/g, "");
    if (!cardNumber || !month || !year || !cvc) {
      setErrorMsg("Please fill in all card fields");
      setStatus("error");
      return;
    }
    try {
      setStatus("submitting");
      setErrorMsg(null);
      await onSubmit({
        cardNumber,
        cardExpMonth: month.padStart(2, "0"),
        cardExpYear: year.length === 2 ? `20${year}` : year,
        cardCvc: cvc,
      });
      setStatus("done");
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setErrorMsg(msg);
      setStatus("error");
      onError(err instanceof Error ? err : new Error(msg));
    }
  };

  return (
    <div className="authorizedotnet-payment-container">
      <input
        style={inputStyle}
        placeholder="Card number"
        inputMode="numeric"
        autoComplete="cc-number"
        value={number}
        onChange={(e) => setNumber(e.target.value)}
        data-testid="authorizedotnet-card-number"
      />
      <div style={{ display: "flex", gap: 12 }}>
        <input
          style={inputStyle}
          placeholder="MM"
          inputMode="numeric"
          autoComplete="cc-exp-month"
          value={month}
          onChange={(e) => setMonth(e.target.value)}
          data-testid="authorizedotnet-exp-month"
        />
        <input
          style={inputStyle}
          placeholder="YYYY"
          inputMode="numeric"
          autoComplete="cc-exp-year"
          value={year}
          onChange={(e) => setYear(e.target.value)}
          data-testid="authorizedotnet-exp-year"
        />
        <input
          style={inputStyle}
          placeholder="CVV"
          inputMode="numeric"
          autoComplete="cc-csc"
          value={cvc}
          onChange={(e) => setCvc(e.target.value)}
          data-testid="authorizedotnet-cvv"
        />
      </div>
      <button
        type="button"
        onClick={handlePay}
        disabled={status === "submitting"}
        data-testid="authorizedotnet-pay"
        style={{
          width: "100%",
          padding: 12,
          border: "none",
          borderRadius: 6,
          background: "#222",
          color: "#fff",
          fontWeight: 600,
          cursor: status === "submitting" ? "default" : "pointer",
        }}
      >
        {status === "submitting" ? "Processing…" : "Pay"}
      </button>
      {status === "done" && (
        <div style={{ fontSize: 13, color: "#16a34a", marginTop: 8 }}>
          Approved — completing your order…
        </div>
      )}
      {status === "error" && errorMsg && (
        <div style={{ fontSize: 13, color: "#dc2626", marginTop: 8 }}>
          {errorMsg}
        </div>
      )}
    </div>
  );
}
