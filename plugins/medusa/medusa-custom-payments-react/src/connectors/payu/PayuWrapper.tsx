"use client";

import { useState } from "react";

export interface PayuBilling {
  firstName: string;
  lastName?: string;
  email: string;
  phone?: string;
}

export type PayuMethod = "upi_collect" | "upi_intent" | "payu_redirect";

export interface PayuSubmitPayload {
  paymentMethodType: PayuMethod;
  /** Required for UPI Collect — the shopper's Virtual Payment Address. */
  vpa?: string;
  billing: PayuBilling;
}

interface PayuWrapperProps {
  /** Amount in major units, for the button label. */
  amount: number;
  /** ISO 4217 currency (PayU India is INR). */
  currency: string;
  /** Pre-fill the billing form (defaults to an India test buyer). */
  defaultBilling?: Partial<PayuBilling>;
  /**
   * Hand the chosen method + UPI VPA + billing to the host, which persists them
   * on the session and triggers the authorize that returns the UPI/redirect
   * next step. Awaited.
   */
  onSubmit: (payload: PayuSubmitPayload) => void | Promise<void>;
  onError: (error: Error) => void;
}

// PayU sandbox accepts `anything@upi` style VPAs; `success@upi` simulates an
// approved collect in test mode. All fields are editable.
const IN_DEFAULTS: PayuBilling = {
  firstName: "Asha",
  lastName: "Kumar",
  email: "asha.kumar@example.com",
  phone: "9999999999",
};

const METHODS: Array<{ key: PayuMethod; label: string }> = [
  { key: "upi_collect", label: "UPI (VPA)" },
  { key: "upi_intent", label: "UPI (Intent)" },
  { key: "payu_redirect", label: "Wallet / Netbanking" },
];

/**
 * PayU is an India-first UPI / hosted-redirect flow with no client-side
 * tokenization — it needs the buyer's billing (name + email mandatory) and,
 * for UPI Collect, a VPA. This form collects them and hands them to the host,
 * which persists them on the session and triggers the authorize that returns
 * the UPI deep-link / hosted-page redirect.
 */
export function PayuWrapper({
  amount,
  currency,
  defaultBilling,
  onSubmit,
  onError,
}: PayuWrapperProps) {
  const [method, setMethod] = useState<PayuMethod>("upi_collect");
  const [vpa, setVpa] = useState("success@upi");
  const [billing, setBilling] = useState<PayuBilling>({
    ...IN_DEFAULTS,
    ...defaultBilling,
  });
  const [submitting, setSubmitting] = useState(false);

  const set = (key: keyof PayuBilling, value: string) =>
    setBilling((b) => ({ ...b, [key]: value }));

  const handlePay = async () => {
    if (!billing.firstName?.trim() || !billing.email?.trim()) {
      onError(new Error("PayU billing: first name and email are required"));
      return;
    }
    if (method === "upi_collect" && !vpa.trim()) {
      onError(new Error("PayU UPI Collect: a VPA (UPI ID) is required"));
      return;
    }
    setSubmitting(true);
    try {
      await onSubmit({
        paymentMethodType: method,
        ...(method === "upi_collect" ? { vpa: vpa.trim() } : {}),
        billing,
      });
    } catch (err) {
      onError(err instanceof Error ? err : new Error(String(err)));
    } finally {
      setSubmitting(false);
    }
  };

  const field = (
    key: keyof PayuBilling,
    label: string,
    type = "text",
    half = false
  ) => (
    <div style={{ flex: half ? "1 1 calc(50% - 4px)" : "1 1 100%" }}>
      <label
        style={{ display: "block", fontSize: 12, color: "#555", marginBottom: 4 }}
      >
        {label}
      </label>
      <input
        data-testid={`payu-${key}`}
        type={type}
        value={billing[key] ?? ""}
        onChange={(e) => set(key, e.target.value)}
        style={{
          width: "100%",
          padding: "10px 12px",
          border: "1px solid #ddd",
          borderRadius: 6,
          fontSize: 14,
          boxSizing: "border-box",
        }}
      />
    </div>
  );

  return (
    <div className="payu-form" data-testid="payu-form">
      <div style={{ display: "flex", gap: 8, marginBottom: 16 }}>
        {METHODS.map((m) => (
          <button
            key={m.key}
            type="button"
            data-testid={`payu-method-${m.key}`}
            onClick={() => setMethod(m.key)}
            style={{
              flex: 1,
              padding: "10px 8px",
              borderRadius: 6,
              border: method === m.key ? "2px solid #0b051d" : "1px solid #ddd",
              background: method === m.key ? "#f5f5f5" : "#fff",
              fontWeight: method === m.key ? 600 : 400,
              fontSize: 13,
              cursor: "pointer",
            }}
          >
            {m.label}
          </button>
        ))}
      </div>

      {method === "upi_collect" && (
        <div style={{ marginBottom: 12 }}>
          <label
            style={{ display: "block", fontSize: 12, color: "#555", marginBottom: 4 }}
          >
            UPI ID (VPA)
          </label>
          <input
            data-testid="payu-vpa"
            type="text"
            value={vpa}
            onChange={(e) => setVpa(e.target.value)}
            placeholder="name@bank"
            style={{
              width: "100%",
              padding: "10px 12px",
              border: "1px solid #ddd",
              borderRadius: 6,
              fontSize: 14,
              boxSizing: "border-box",
            }}
          />
        </div>
      )}

      <div style={{ display: "flex", flexWrap: "wrap", gap: 8 }}>
        {field("firstName", "First name", "text", true)}
        {field("lastName", "Last name", "text", true)}
        {field("email", "Email", "email")}
        {field("phone", "Phone", "tel", true)}
      </div>

      <button
        type="button"
        data-testid="payu-pay"
        disabled={submitting}
        onClick={handlePay}
        style={{
          width: "100%",
          marginTop: 16,
          padding: "14px 20px",
          border: "none",
          borderRadius: 8,
          background: submitting ? "#a3d39caa" : "#00b050", // PayU green
          color: "#fff",
          fontSize: 16,
          fontWeight: 700,
          cursor: submitting ? "not-allowed" : "pointer",
        }}
      >
        {submitting ? "Processing…" : `Pay ${amount} ${currency} with PayU`}
      </button>
    </div>
  );
}
