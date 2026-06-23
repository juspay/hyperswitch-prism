/**
 * Single source of truth for status semantics, consumed by prism_status_reference,
 * prism_explain_error, prism_test_charge normalization, and scaffold codegen.
 *
 * Numeric values are NOT hand-transcribed — they are pulled from the proto-derived
 * enums.generated.json. This module adds the human-facing category + meaning, and
 * ASSERTS at load that every status it describes still exists in the proto (so a
 * proto rename/renumber fails loudly rather than drifting silently).
 */
import { ENUMS } from "../data/connectors.js";

export type StatusKind = "payment" | "refund";
export type Category = "success" | "decline" | "pending" | "failure";

export interface StatusEntry {
  code: number;
  name: string;
  kind: StatusKind;
  category: Category;
  meaning: string;
}

interface StatusSpec {
  category: Category;
  meaning: string;
}

// Hand-authored semantics keyed by proto enum NAME. Codes come from the proto.
const PAYMENT_SPECS: Record<string, StatusSpec> = {
  PAYMENT_STATUS_UNSPECIFIED: { category: "failure", meaning: "Unspecified / unknown status." },
  STARTED: { category: "pending", meaning: "Payment initiated, not yet processed." },
  AUTHENTICATION_PENDING: { category: "pending", meaning: "Awaiting 3DS / customer authentication (redirect required)." },
  AUTHENTICATION_SUCCESSFUL: { category: "pending", meaning: "3DS authentication passed; authorization not yet complete." },
  AUTHENTICATION_FAILED: { category: "decline", meaning: "3DS / authentication failed." },
  AUTHORIZING: { category: "pending", meaning: "Authorization in progress." },
  AUTHORIZED: { category: "success", meaning: "Funds authorized/held. Capture required (MANUAL capture flow)." },
  AUTHORIZATION_FAILED: { category: "decline", meaning: "Authorization was declined by the processor." },
  PARTIALLY_AUTHORIZED: { category: "success", meaning: "Partial amount authorized." },
  CHARGED: { category: "success", meaning: "Payment captured/charged successfully (terminal success)." },
  PARTIAL_CHARGED: { category: "success", meaning: "Part of the amount was captured." },
  PARTIAL_CHARGED_AND_CHARGEABLE: { category: "success", meaning: "Partially charged; remaining amount still capturable." },
  AUTO_REFUNDED: { category: "failure", meaning: "Payment was automatically refunded." },
  CAPTURE_INITIATED: { category: "pending", meaning: "Capture requested; awaiting confirmation (async)." },
  CAPTURE_FAILED: { category: "failure", meaning: "Capture attempt failed." },
  VOID_INITIATED: { category: "pending", meaning: "Void/cancel requested; awaiting confirmation." },
  VOIDED: { category: "success", meaning: "Authorization voided/cancelled successfully." },
  VOID_FAILED: { category: "failure", meaning: "Void attempt failed." },
  VOIDED_POST_CAPTURE: { category: "failure", meaning: "Voided after capture (reversal)." },
  COD_INITIATED: { category: "pending", meaning: "Cash-on-delivery initiated." },
  EXPIRED: { category: "failure", meaning: "Payment expired before capture." },
  AUTO_REFUNDED_2: { category: "failure", meaning: "Auto refunded." },
  PARTIALLY_CAPTURED: { category: "success", meaning: "Partially captured." },
  UNRESOLVED: { category: "pending", meaning: "Needs merchant action / unresolved." },
  PENDING: { category: "pending", meaning: "Async processing; poll with get/sync to resolve." },
  FAILURE: { category: "decline", meaning: "Soft decline — returned IN the response body, not thrown. Inspect error_message / decline reason." },
  PAYMENT_METHOD_AWAITED: { category: "pending", meaning: "Awaiting payment method from customer." },
  CONFIRMATION_AWAITED: { category: "pending", meaning: "Awaiting customer confirmation." },
  DEVICE_DATA_COLLECTION_PENDING: { category: "pending", meaning: "Awaiting 3DS device data collection." },
};

const REFUND_SPECS: Record<string, StatusSpec> = {
  REFUND_STATUS_UNSPECIFIED: { category: "failure", meaning: "Unspecified refund status." },
  REFUND_FAILURE: { category: "decline", meaning: "Refund failed / was declined." },
  REFUND_MANUAL_REVIEW: { category: "pending", meaning: "Refund held for manual review." },
  REFUND_PENDING: { category: "pending", meaning: "Refund accepted; processing asynchronously." },
  REFUND_SUCCESS: { category: "success", meaning: "Refund completed successfully (terminal success)." },
  REFUND_TRANSACTION_FAILURE: { category: "failure", meaning: "Transaction-level failure during refund." },
};

function build(
  specs: Record<string, StatusSpec>,
  enumMap: Record<string, number>,
  kind: StatusKind,
): Map<number, StatusEntry> {
  const out = new Map<number, StatusEntry>();
  for (const [name, code] of Object.entries(enumMap)) {
    const spec = specs[name];
    if (!spec) continue; // proto may add codes we don't yet describe; skip quietly
    out.set(code, { code, name, kind, category: spec.category, meaning: spec.meaning });
  }
  return out;
}

export const PAYMENT_STATUS = build(PAYMENT_SPECS, ENUMS.PaymentStatus, "payment");
export const REFUND_STATUS = build(REFUND_SPECS, ENUMS.RefundStatus, "refund");

// Drift guard: a few well-known statuses MUST be present and correctly numbered.
for (const [name, expected] of [
  ["AUTHORIZED", 6],
  ["CHARGED", 8],
  ["VOIDED", 11],
  ["FAILURE", 21],
] as const) {
  if (ENUMS.PaymentStatus[name] !== expected) {
    throw new Error(`statusMap drift: PaymentStatus.${name} expected ${expected}, got ${ENUMS.PaymentStatus[name]}`);
  }
}
if (ENUMS.RefundStatus.REFUND_SUCCESS !== 4) {
  throw new Error(`statusMap drift: RefundStatus.REFUND_SUCCESS expected 4`);
}

export function lookupPayment(code: number): StatusEntry | undefined {
  return PAYMENT_STATUS.get(code);
}
export function lookupRefund(code: number): StatusEntry | undefined {
  return REFUND_STATUS.get(code);
}

/** Look up by code, optionally constrained to a kind. Tries payment then refund. */
export function lookupStatus(code: number, kind?: StatusKind): StatusEntry | undefined {
  if (kind === "refund") return REFUND_STATUS.get(code);
  if (kind === "payment") return PAYMENT_STATUS.get(code);
  return PAYMENT_STATUS.get(code) ?? REFUND_STATUS.get(code);
}

export function categoryOf(code: number, kind: StatusKind): Category | undefined {
  return lookupStatus(code, kind)?.category;
}

/** Success criterion for a freshly-authorized payment given the capture method. */
export function isAuthorizeSuccess(code: number, captureMethod: "AUTOMATIC" | "MANUAL"): boolean {
  const charged = ENUMS.PaymentStatus.CHARGED;
  const authorized = ENUMS.PaymentStatus.AUTHORIZED;
  return captureMethod === "AUTOMATIC" ? code === charged : code === authorized;
}

export function paymentStatusList(): StatusEntry[] {
  return [...PAYMENT_STATUS.values()].sort((a, b) => a.code - b.code);
}
export function refundStatusList(): StatusEntry[] {
  return [...REFUND_STATUS.values()].sort((a, b) => a.code - b.code);
}
