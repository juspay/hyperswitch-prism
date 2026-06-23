"use client";

import { useState } from "react";

export interface MollieKlarnaBilling {
  firstName: string;
  lastName: string;
  email: string;
  line1: string;
  city: string;
  postalCode: string;
  /** ISO 3166-1 alpha-2 (e.g. "NL"). Klarna via Mollie is EU-only. */
  country: string;
}

interface MollieKlarnaFormProps {
  /** Amount in major units, for the button label. */
  amount: number;
  /** ISO 4217 currency (Klarna via Mollie requires an EU currency, e.g. EUR). */
  currency: string;
  /** Pre-fill the billing form (defaults to a Netherlands test address). */
  defaultBilling?: Partial<MollieKlarnaBilling>;
  /** Submit the billing details to start the Klarna redirect. Awaited. */
  onSubmit: (billing: MollieKlarnaBilling) => void | Promise<void>;
  onError: (error: Error) => void;
}

// Klarna via Mollie only operates in EU markets. Default to a Netherlands test
// address so the sandbox redirect works out of the box; all fields are editable.
const NL_DEFAULTS: MollieKlarnaBilling = {
  firstName: "Test",
  lastName: "Klarna",
  email: "test.klarna@example.com",
  line1: "Keizersgracht 313",
  city: "Amsterdam",
  postalCode: "1016 EE",
  country: "NL",
};

const FIELDS: Array<{
  key: keyof MollieKlarnaBilling;
  label: string;
  type?: string;
  half?: boolean;
}> = [
  { key: "firstName", label: "First name", half: true },
  { key: "lastName", label: "Last name", half: true },
  { key: "email", label: "Email", type: "email" },
  { key: "line1", label: "Address" },
  { key: "city", label: "City", half: true },
  { key: "postalCode", label: "Postal code", half: true },
  { key: "country", label: "Country (ISO-2)", half: true },
];

/**
 * Klarna (Pay later) via Mollie is a redirect flow with no client-side
 * tokenization — Klarna requires the buyer's billing details up front. This form
 * collects them and hands them to the host, which persists them on the session
 * and triggers the authorize that returns the Klarna hosted-checkout redirect.
 */
export function MollieKlarnaForm({
  amount,
  currency,
  defaultBilling,
  onSubmit,
  onError,
}: MollieKlarnaFormProps) {
  const [billing, setBilling] = useState<MollieKlarnaBilling>({
    ...NL_DEFAULTS,
    ...defaultBilling,
  });
  const [submitting, setSubmitting] = useState(false);

  const set = (key: keyof MollieKlarnaBilling, value: string) =>
    setBilling((b) => ({ ...b, [key]: value }));

  const handlePay = async () => {
    const missing = (Object.keys(NL_DEFAULTS) as Array<keyof MollieKlarnaBilling>).filter(
      (k) => !billing[k]?.trim()
    );
    if (missing.length) {
      onError(new Error(`Klarna billing: missing ${missing.join(", ")}`));
      return;
    }
    setSubmitting(true);
    try {
      await onSubmit({ ...billing, country: billing.country.toUpperCase() });
    } catch (err) {
      onError(err instanceof Error ? err : new Error(String(err)));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="mollie-klarna-form" data-testid="mollie-klarna-form">
      <p style={{ color: "#666", fontSize: 13, margin: "0 0 12px" }}>
        Pay later with Klarna. Enter your billing details (EU only).
      </p>
      <div style={{ display: "flex", flexWrap: "wrap", gap: 8 }}>
        {FIELDS.map((f) => (
          <div
            key={f.key}
            style={{ flex: f.half ? "1 1 calc(50% - 4px)" : "1 1 100%" }}
          >
            <label
              style={{ display: "block", fontSize: 12, color: "#555", marginBottom: 4 }}
            >
              {f.label}
            </label>
            <input
              data-testid={`klarna-${f.key}`}
              type={f.type ?? "text"}
              value={billing[f.key]}
              onChange={(e) => set(f.key, e.target.value)}
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
        ))}
      </div>
      <button
        type="button"
        data-testid="klarna-pay"
        disabled={submitting}
        onClick={handlePay}
        style={{
          width: "100%",
          marginTop: 16,
          padding: "14px 20px",
          border: "none",
          borderRadius: 8,
          background: submitting ? "#ffb3c7aa" : "#ffb3c7", // Klarna pink
          color: "#0b051d",
          fontSize: 16,
          fontWeight: 700,
          cursor: submitting ? "not-allowed" : "pointer",
        }}
      >
        {submitting ? "Redirecting…" : `Pay ${amount} ${currency} with Klarna`}
      </button>
    </div>
  );
}
