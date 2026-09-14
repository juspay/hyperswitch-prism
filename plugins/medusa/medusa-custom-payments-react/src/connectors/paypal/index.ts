import type {
  PaymentConnector,
  InitiateParams,
  InitiateResult,
  ConfirmParams,
  ConfirmResult,
} from "../../types";
import { loadPayPalScript } from "./utils";

export const paypalConnector: PaymentConnector = {
  name: "paypal",

  initiate: async (params: InitiateParams): Promise<InitiateResult> => {
    const res = await fetch("/api/payments/initiate", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        connector: "paypal",
        currency_code: params.currency_code,
        amount: params.amount,
        data: {},
        context: { cart_id: params.cartId, ...params.context },
      }),
    });

    if (!res.ok) {
      const err = await res.json().catch(() => ({}));
      throw new Error(err.error || `PayPal initiate failed: ${res.status}`);
    }

    return res.json();
  },

  render: (containerId, sessionData, { onSubmit, onError }) => {
    const container = document.getElementById(containerId);
    if (!container) {
      onError(new Error(`PayPal: container #${containerId} not found`));
      return;
    }

    const data = sessionData.data as Record<string, any>;
    const clientId = data?.clientId ?? "";
    const currency = data?.currency ?? "USD";
    const orderId = data?.orderId ?? data?.id ?? "";

    if (!clientId) {
      onError(new Error("PayPal: missing clientId in session data"));
      return;
    }

    loadPayPalScript(clientId, currency)
      .then((paypal) => {
        paypal
          .Buttons({
            style: { layout: "vertical" },
            createOrder: () => orderId,
            onApprove: (approveData: any, actions: any) => {
              return actions.order.capture().then((details: any) => {
                onSubmit({
                  orderId: approveData.orderID,
                  payerId: approveData.payerID,
                  details,
                });
              });
            },
            onError: (err: any) => {
              onError(
                err instanceof Error ? err : new Error(String(err))
              );
            },
          })
          .render(container);
      })
      .catch(onError);
  },

  destroy: () => {
    // PayPal SDK does not expose an unmount API.
    // Caller should clear the DOM container if needed.
  },

  confirm: async (params: ConfirmParams): Promise<ConfirmResult> => {
    const orderId =
      (params.data as any)?.orderId ??
      (params.data as any)?.orderID ??
      "";

    const res = await fetch(`/api/payments/${orderId}/authorize`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        connector: "paypal",
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
