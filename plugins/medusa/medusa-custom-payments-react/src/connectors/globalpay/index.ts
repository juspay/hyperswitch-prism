import type { PaymentConnector, InitiateParams, InitiateResult, ConfirmParams, ConfirmResult } from "../../types";
import { loadGlobalPayScript } from "./utils";

export { GlobalPayWrapper } from "./GlobalPayWrapper";
export { loadGlobalPayScript } from "./utils";

let cardForm: any = null;

export const globalpayConnector: PaymentConnector = {
  name: "globalpay",

  initiate: async (params: InitiateParams): Promise<InitiateResult> => {
    const res = await fetch("/api/payments/initiate", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        connector: "globalpay",
        currency_code: params.currency_code,
        amount: params.amount,
        data: {},
        context: { cart_id: params.cartId, ...params.context },
      }),
    });

    if (!res.ok) {
      const err = await res.json().catch(() => ({}));
      throw new Error(err.error || `GlobalPay initiate failed: ${res.status}`);
    }

    return res.json();
  },

  render: (containerId, sessionData, { onSubmit, onError }) => {
    const container = document.getElementById(containerId);
    if (!container) {
      onError(new Error(`GlobalPay: container #${containerId} not found`));
      return;
    }

    loadGlobalPayScript()
      .then((GlobalPayments) => {
        const data = sessionData.data as Record<string, any>;
        const accessToken = data?.accessToken ?? "";

        GlobalPayments.configure({
          accessToken,
          env: "sandbox",
        });

        cardForm = GlobalPayments.creditCard.form(container);

        cardForm.on("token-success", (resp: any) => {
          const token = resp?.paymentReference;
          if (token) {
            onSubmit({ paymentReference: token });
          } else {
            onError(new Error("GlobalPay: no paymentReference in token response"));
          }
        });

        cardForm.on("token-error", (err: any) => {
          const msg = err?.reason || err?.detailed_error_description || "Tokenization failed";
          onError(new Error(msg));
        });

        GlobalPayments.on("error", (err: any) => {
          onError(new Error(err?.message || "GlobalPay SDK error"));
        });
      })
      .catch(onError);
  },

  destroy: () => {
    // Hosted fields cleanup: clear the container contents
    cardForm = null;
  },

  confirm: async (params: ConfirmParams): Promise<ConfirmResult> => {
    const paymentReference =
      (params.data as any)?.paymentReference ?? "";

    const res = await fetch(`/api/payments/authorize`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        connector: "globalpay",
        data: { paymentReference, ...(params.data as Record<string, any>) },
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
