// The unified payment library
// ─────────────────────────────────────────────────────────────────────────────
// This is the heart of the workshop. Every demo talks to PSPs through these few
// functions and NEVER mentions a specific processor. Swapping Stripe for Adyen
// changes nothing here — only the PSP name passed in.
//
// Each call returns a normalized `UnifiedResult` regardless of which processor
// ran or whether it succeeded, declined, or errored. The orchestrator (routing
// + retry) is built entirely on top of these results.
// ─────────────────────────────────────────────────────────────────────────────

import {
  PaymentClient,
  types,
  IntegrationError,
  ConnectorError,
  NetworkError,
} from 'hyperswitch-prism';

import { getPsp, type PspName } from '../../config/psp-registry.js';
import { toSdkCard, toSdkCurrency, type Order } from './cards.js';
import { paymentStatusText, refundStatusText } from './format.js';

export interface UnifiedResult {
  ok: boolean; // approved / authorized / charged (a usable success)
  psp: PspName;
  operation: 'authorize' | 'capture' | 'refund' | 'void';
  status: number;
  statusText: string;
  transactionId?: string;
  pending?: boolean;
  error?: string;
}

// Statuses we treat as a usable success for a one-step (auto-capture) payment.
const APPROVED_PAYMENT_STATUSES = new Set<number>([
  types.PaymentStatus.CHARGED,
  types.PaymentStatus.AUTHORIZED,
  types.PaymentStatus.PARTIAL_CHARGED,
]);

const PENDING_PAYMENT_STATUSES = new Set<number>([
  types.PaymentStatus.PENDING,
  types.PaymentStatus.AUTHENTICATION_PENDING,
  types.PaymentStatus.CAPTURE_INITIATED,
]);

// `response.error` is a protobuf object, not plain JSON. Pull out the message.
function extractError(error: any): string | undefined {
  if (!error) return undefined;
  return (
    error?.unifiedDetails?.message ||
    error?.issuerDetails?.message ||
    error?.connectorDetails?.message ||
    error?.message ||
    'Unknown connector error'
  );
}

// Turn any thrown SDK error into a normalized failed result. This lets the
// orchestrator reason about "did it work?" uniformly without try/catch
// scattered everywhere.
function errorToResult(
  psp: PspName,
  operation: UnifiedResult['operation'],
  e: unknown,
): UnifiedResult {
  let error = 'Unexpected error';
  if (e instanceof IntegrationError) error = `IntegrationError: ${e.message}`;
  else if (e instanceof ConnectorError) error = `ConnectorError: ${e.message}`;
  else if (e instanceof NetworkError) error = `NetworkError: ${e.message}`;
  else if (e instanceof Error) error = e.message;
  return { ok: false, psp, operation, status: -1, statusText: 'ERROR', error };
}

// ── Pure normalizer (unit-tested directly, no network) ──────────────────────
export function normalizeAuthorize(psp: PspName, response: any): UnifiedResult {
  const status: number = response?.status ?? -1;
  const ok = APPROVED_PAYMENT_STATUSES.has(status);
  const pending = PENDING_PAYMENT_STATUSES.has(status);
  return {
    ok,
    psp,
    operation: 'authorize',
    status,
    statusText: paymentStatusText(status),
    transactionId: response?.connectorTransactionId ?? undefined,
    pending,
    error: ok ? undefined : extractError(response?.error),
  };
}

// ── Real SDK calls (each returns a UnifiedResult, never throws) ──────────────

export interface AuthorizeOptions {
  // AUTOMATIC = authorize + capture in one call (default).
  // MANUAL    = authorize only; capture later (see capture()).
  captureMethod?: types.CaptureMethod;
}

export async function authorize(
  psp: PspName,
  order: Order,
  opts: AuthorizeOptions = {},
): Promise<UnifiedResult> {
  try {
    const client = new PaymentClient(getPsp(psp).buildConfig());
    const response = await client.authorize({
      merchantTransactionId: order.merchantTransactionId,
      amount: { minorAmount: order.minorAmount, currency: toSdkCurrency(order.currency) },
      paymentMethod: { card: toSdkCard(order.card) },
      captureMethod: opts.captureMethod ?? types.CaptureMethod.AUTOMATIC,
      address: { billingAddress: {} },
      authType: types.AuthenticationType.NO_THREE_DS,
      returnUrl: 'https://example.com/return',
      // Some processors require these. Including them here keeps the unified
      // request portable across every PSP in the registry (e.g. Cybersource
      // needs customer.email, Adyen needs browserInfo).
      customer: { email: { value: 'jane.workshop@example.com' } },
      browserInfo: {
        colorDepth: 24,
        screenHeight: 900,
        screenWidth: 1440,
        javaEnabled: false,
        javaScriptEnabled: true,
        language: 'en-US',
        timeZoneOffsetMinutes: -480,
        acceptHeader: 'application/json',
        userAgent: 'Mozilla/5.0 (workshop)',
        acceptLanguage: 'en-US,en;q=0.9',
        ipAddress: '1.2.3.4',
      },
    });
    return normalizeAuthorize(psp, response);
  } catch (e) {
    return errorToResult(psp, 'authorize', e);
  }
}

export async function capture(
  psp: PspName,
  order: Order,
  connectorTransactionId: string,
): Promise<UnifiedResult> {
  try {
    const client = new PaymentClient(getPsp(psp).buildConfig());
    const response = await client.capture({
      merchantCaptureId: `${order.merchantTransactionId}_cap`,
      connectorTransactionId,
      amountToCapture: { minorAmount: order.minorAmount, currency: toSdkCurrency(order.currency) },
    });
    const status: number = response?.status ?? -1;
    const ok =
      status === types.PaymentStatus.CHARGED ||
      status === types.PaymentStatus.PARTIAL_CHARGED ||
      status === types.PaymentStatus.PENDING;
    return {
      ok,
      psp,
      operation: 'capture',
      status,
      statusText: paymentStatusText(status),
      transactionId: connectorTransactionId,
      error: ok ? undefined : extractError(response?.error),
    };
  } catch (e) {
    return errorToResult(psp, 'capture', e);
  }
}

// ★ STEP 7 — "ADD A NEW FLOW" ★
// Void cancels an authorization that has NOT been captured yet. It did not exist
// in the unified library until we added it here — and notice the shape is
// identical to the other flows: build the PSP's PaymentClient, call the matching
// SDK method, normalize the result. That uniformity is what makes the library
// extensible. The SDK already exposes `client.void(...)`; we just surface it.
export async function voidPayment(
  psp: PspName,
  order: Order,
  connectorTransactionId: string,
): Promise<UnifiedResult> {
  try {
    const client = new PaymentClient(getPsp(psp).buildConfig());
    const response = await client.void({
      merchantVoidId: `${order.merchantTransactionId}_void`,
      connectorTransactionId,
    });
    const status: number = response?.status ?? -1;
    const ok =
      status === types.PaymentStatus.VOIDED ||
      status === types.PaymentStatus.VOID_INITIATED;
    return {
      ok,
      psp,
      operation: 'void',
      status,
      statusText: paymentStatusText(status),
      transactionId: connectorTransactionId,
      error: ok ? undefined : extractError(response?.error),
    };
  } catch (e) {
    return errorToResult(psp, 'void', e);
  }
}

export async function refund(
  psp: PspName,
  order: Order,
  connectorTransactionId: string,
  refundMinorAmount?: number,
): Promise<UnifiedResult> {
  try {
    const client = new PaymentClient(getPsp(psp).buildConfig());
    const amount = refundMinorAmount ?? order.minorAmount;
    const response = await client.refund({
      merchantRefundId: `${order.merchantTransactionId}_ref`,
      connectorTransactionId,
      paymentAmount: order.minorAmount,
      refundAmount: { minorAmount: amount, currency: toSdkCurrency(order.currency) },
      reason: 'customer_request',
    });
    const status: number = response?.status ?? -1;
    const ok =
      status === types.RefundStatus.REFUND_SUCCESS ||
      status === types.RefundStatus.REFUND_PENDING;
    return {
      ok,
      psp,
      operation: 'refund',
      status,
      statusText: refundStatusText(status),
      transactionId: connectorTransactionId,
      error: ok ? undefined : extractError(response?.error),
    };
  } catch (e) {
    return errorToResult(psp, 'refund', e);
  }
}
