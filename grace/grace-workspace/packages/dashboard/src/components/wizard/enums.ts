// Canonical enums for the New Integration wizard.
// Sourced from grace/rulesbook/codegen/guides/workflow_selection.md.
// The Rust scaffolding macros (create_all_prerequisites!,
// macro_connector_implementation!) match these strings byte-for-byte —
// do not casually rename.

export const ALL_FLOWS = [
  "Authorize",
  "PSync",
  "Capture",
  "Void",
  "Refund",
  "RSync",
  "SetupMandate",
  "RepeatPayment",
  "IncomingWebhook",
  "CreateOrder",
  "SessionToken",
  "PaymentMethodToken",
  "DefendDispute",
  "AcceptDispute",
  "DSync",
  "SubmitEvidence",
  "IncrementalAuthorization",
  "VoidPC",
  "CreateAccessToken",
] as const;
export type Flow = (typeof ALL_FLOWS)[number];

// Prerequisites: if a key flow is selected, every value must also be selected.
// The wizard auto-enables prerequisites and locks them.
export const FLOW_PREREQUISITES: Record<string, string[]> = {
  Refund: ["Capture"],
  RSync: ["Refund"],
  PSync: ["Authorize"],
  RepeatPayment: ["SetupMandate"],
  VoidPC: ["Capture"],
  SubmitEvidence: ["DefendDispute"],
};

export const PM_CATEGORIES = {
  Card: ["Credit", "Debit"],
  Wallet: [
    "Apple Pay",
    "Google Pay",
    "PayPal",
    "WeChat Pay",
    "Paze",
    "RevolutPay",
    "AliPay",
    "Samsung Pay",
  ],
  BankTransfer: ["SEPA", "ACH", "Wire", "BACS"],
  BankDebit: ["SEPA Direct Debit", "ACH Debit", "BECS", "BACS Direct Debit"],
  BankRedirect: ["iDEAL", "Sofort", "Giropay", "Bancontact", "EPS", "Trustly"],
  UPI: ["Collect", "Intent"],
  BNPL: ["Klarna", "Afterpay", "Affirm", "Zip"],
  Crypto: ["Bitcoin", "Ethereum", "USDC"],
  GiftCard: ["Givex", "Blackhawk"],
  MobilePayment: ["MPesa", "AirtelMoney"],
  Reward: ["Points", "Cashback"],
} as const;
export type PMCategory = keyof typeof PM_CATEGORIES;

export const AUTH_SCHEMES = [
  "APIKey",
  "OAuth2",
  "BasicAuth",
  "Signature",
  "JWT",
  "Custom",
] as const;
export type AuthScheme = (typeof AUTH_SCHEMES)[number];

export const AUTH_LOCATIONS = ["Header", "Query", "Body", "Custom"] as const;
export type AuthLocation = (typeof AUTH_LOCATIONS)[number];

export const CURRENCY_UNITS = [
  "Minor",
  "StringMinor",
  "StringMajor",
  "Base",
] as const;
export type CurrencyUnit = (typeof CURRENCY_UNITS)[number];

export const REGIONS = ["US", "EU", "UK", "APAC", "LATAM", "MEA", "Global"] as const;
export type Region = (typeof REGIONS)[number];

export const DOC_TYPES = [
  "api_reference",
  "payment_method_guide",
  "authentication_guide",
  "webhooks_guide",
  "testing_guide",
  "error_reference",
] as const;
export type DocType = (typeof DOC_TYPES)[number];

export const COMPLEXITY = ["low", "medium", "high"] as const;
export const PRIORITY = ["critical", "high", "medium", "low"] as const;

// Auto-locked flows triggered by feature toggles.
export const TOGGLE_FORCED_FLOWS = {
  supportsWebhooks: ["IncomingWebhook"],
  supportsRecurring: ["SetupMandate", "RepeatPayment"],
} as const;

// Expand a flow selection to include all its prerequisites (transitive).
export function expandWithPrerequisites(flows: string[]): string[] {
  const out = new Set<string>(flows);
  let changed = true;
  while (changed) {
    changed = false;
    for (const f of Array.from(out)) {
      for (const req of FLOW_PREREQUISITES[f] ?? []) {
        if (!out.has(req)) {
          out.add(req);
          changed = true;
        }
      }
    }
  }
  return ALL_FLOWS.filter((f) => out.has(f));
}

// Which selected flows force a given flow to stay on? Returns the list of
// dependents so the UI can show a tooltip ("Capture is required by Refund").
export function flowDependents(flow: string, selected: Set<string>): string[] {
  const dependents: string[] = [];
  for (const [dependent, prereqs] of Object.entries(FLOW_PREREQUISITES)) {
    if (prereqs.includes(flow) && selected.has(dependent)) dependents.push(dependent);
  }
  return dependents;
}
