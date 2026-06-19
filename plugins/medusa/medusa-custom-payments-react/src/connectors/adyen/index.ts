import type { PaymentConnector, InitiateParams, InitiateResult, ConfirmParams, ConfirmResult } from "../../types";
import { loadAdyenScript, injectAdyenStyles } from "./utils";

let adyenDropin: any = null;

export const adyenConnector: PaymentConnector = {
  name: "adyen",

  initiate: async (params: InitiateParams): Promise<InitiateResult> => {
    const res = await fetch("/api/payments/initiate", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        connector: "adyen",
        currency_code: params.currency_code,
        amount: params.amount,
        data: {},
        context: { cart_id: params.cartId, ...params.context },
      }),
    });

    if (!res.ok) {
      const err = await res.json().catch(() => ({}));
      throw new Error(err.error || `Adyen initiate failed: ${res.status}`);
    }

    return res.json();
  },

  render: (containerId, sessionData, { onSubmit, onError }) => {
    injectAdyenStyles();

    const container = document.getElementById(containerId);
    if (!container) {
      onError(new Error(`Adyen: container #${containerId} not found`));
      return;
    }

    loadAdyenScript()
      .then((AdyenCheckout) => {
        const data = sessionData.data as Record<string, any>;

        const configuration = {
          environment: "test",
          clientKey: data?.clientKey ?? "",
          paymentMethodsResponse: data?.paymentMethods ?? {},
          amount: {
            value: data?.minorAmount ?? 0,
            currency: data?.currency ?? "USD",
          },
          onSubmit: (state: any) => {
            onSubmit(state.data);
          },
          onError: (error: any) => {
            onError(
              error instanceof Error ? error : new Error(String(error))
            );
          },
        };

        const checkout = new AdyenCheckout(configuration);
        adyenDropin = checkout.create("dropin").mount(container);
      })
      .catch(onError);
  },

  destroy: () => {
    if (adyenDropin?.unmount) {
      adyenDropin.unmount();
    }
    adyenDropin = null;
  },

  confirm: async (params: ConfirmParams): Promise<ConfirmResult> => {
    const sessionId =
      (params.data as any)?.paymentReference ??
      (params.data as any)?.paymentData ??
      "";

    const res = await fetch(`/api/payments/${sessionId}/authorize`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        connector: "adyen",
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
