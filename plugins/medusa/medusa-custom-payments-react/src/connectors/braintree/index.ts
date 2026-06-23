import type {
  PaymentConnector,
  InitiateParams,
  InitiateResult,
  ConfirmParams,
  ConfirmResult,
} from "../../types";

/**
 * Braintree is wallet-only (PayPal / Google Pay / Apple Pay). Its checkout UI
 * is the React <BraintreeWrapper>, mounted by the storefront / panel — the
 * generic imperative `render()` path is not used (mirrors how Stripe is excluded
 * from the generic registry). `initiate`/`confirm` are provided for API symmetry
 * with the other connectors.
 */
export const braintreeConnector: PaymentConnector = {
  name: "braintree",

  initiate: async (params: InitiateParams): Promise<InitiateResult> => {
    const res = await fetch("/api/payments/initiate", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        connector: "braintree",
        currency_code: params.currency_code,
        amount: params.amount,
        data: {},
        context: { cart_id: params.cartId, ...params.context },
      }),
    });

    if (!res.ok) {
      const err = await res.json().catch(() => ({}));
      throw new Error(err.error || `Braintree initiate failed: ${res.status}`);
    }

    return res.json();
  },

  render: (_containerId, _sessionData, { onError }) => {
    onError(
      new Error(
        "Braintree uses the React <BraintreeWrapper> (mounted via the connector panel), not the generic render() path."
      )
    );
  },

  destroy: () => {
    // Wrapper handles its own teardown on unmount.
  },

  confirm: async (params: ConfirmParams): Promise<ConfirmResult> => {
    const id = (params.data as any)?.id ?? (params.data as any)?.merchantClientSessionId ?? "";

    const res = await fetch(`/api/payments/${id}/authorize`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        connector: "braintree",
        data: params.data,
        context: params.idempotencyKey
          ? { idempotency_key: params.idempotencyKey }
          : {},
      }),
    });

    if (!res.ok) {
      const err = await res.json().catch(() => ({}));
      return { status: "failed", error: err.error || String(res.status) };
    }

    return res.json();
  },
};

export { BraintreeWrapper } from "./BraintreeWrapper";
export type {
  BraintreeWalletType,
  BraintreeSubmitPayload,
} from "./BraintreeWrapper";
export {
  loadBraintreeClient,
  loadBraintreePayPalCheckout,
  loadBraintreeGooglePayment,
  loadBraintreeApplePay,
  loadGooglePayJs,
} from "./utils";
