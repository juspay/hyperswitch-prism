import type {
  PaymentConnector,
  InitiateParams,
  InitiateResult,
  ConfirmParams,
  ConfirmResult,
} from "../../types";
import { decodeCaptureContext, loadCybersourceFlexScript } from "./utils";

export { CybersourceWrapper } from "./CybersourceWrapper";
export { loadCybersourceFlexScript, decodeCaptureContext } from "./utils";

let microform: any = null;

/**
 * Generic PaymentConnector for Cybersource (Flex Microform, card).
 *
 * Note: the Medusa test client renders <CybersourceWrapper> directly; this
 * registry entry exists for consumers using the generic PaymentContainer path.
 */
export const cybersourceConnector: PaymentConnector = {
  name: "cybersource",

  initiate: async (params: InitiateParams): Promise<InitiateResult> => {
    const res = await fetch("/api/payments/initiate", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        connector: "cybersource",
        currency_code: params.currency_code,
        amount: params.amount,
        data: {},
        context: { cart_id: params.cartId, ...params.context },
      }),
    });

    if (!res.ok) {
      const err = await res.json().catch(() => ({}));
      throw new Error(err.error || `Cybersource initiate failed: ${res.status}`);
    }

    return res.json();
  },

  render: (containerId, sessionData, { onSubmit, onError }) => {
    const container = document.getElementById(containerId);
    if (!container) {
      onError(new Error(`Cybersource: container #${containerId} not found`));
      return;
    }

    const data = sessionData.data as Record<string, any>;
    const captureContext = data?.captureContext ?? "";
    if (!captureContext) {
      onError(new Error("Cybersource: missing captureContext in session data"));
      return;
    }

    const decoded = decodeCaptureContext(captureContext);
    const libUrl = data?.clientLibrary || decoded.clientLibrary || "";
    const libIntegrity = data?.clientLibraryIntegrity || decoded.clientLibraryIntegrity;

    container.innerHTML = `
      <div id="cyb-number" style="height:40px;border:1px solid #ddd;border-radius:6px;padding:0 12px;margin-bottom:12px;background:#fff"></div>
      <div style="display:flex;gap:12px">
        <input id="cyb-exp-month" placeholder="MM" style="flex:1;height:40px;border:1px solid #ddd;border-radius:6px;padding:0 12px;margin-bottom:12px" />
        <input id="cyb-exp-year" placeholder="YYYY" style="flex:1;height:40px;border:1px solid #ddd;border-radius:6px;padding:0 12px;margin-bottom:12px" />
        <div id="cyb-cvv" style="flex:1;height:40px;border:1px solid #ddd;border-radius:6px;padding:0 12px;margin-bottom:12px;background:#fff"></div>
      </div>
      <button type="button" id="cyb-pay" style="width:100%;padding:12px;border:none;border-radius:6px;background:#222;color:#fff;font-weight:600;cursor:pointer">Save card</button>
    `;

    loadCybersourceFlexScript(libUrl, libIntegrity)
      .then((Flex) => {
        const flex = new Flex(captureContext);
        microform = flex.microform({ styles: { input: { "font-size": "14px" } } });
        microform.createField("number").load("#cyb-number");
        microform.createField("securityCode").load("#cyb-cvv");

        container.querySelector("#cyb-pay")?.addEventListener("click", () => {
          const month = (container.querySelector("#cyb-exp-month") as HTMLInputElement)?.value ?? "";
          const year = (container.querySelector("#cyb-exp-year") as HTMLInputElement)?.value ?? "";
          microform.createToken(
            {
              expirationMonth: month.padStart(2, "0"),
              expirationYear: year.length === 2 ? `20${year}` : year,
            },
            (err: any, token: string) => {
              if (err || !token) {
                onError(new Error(err?.message || "Cybersource tokenisation failed"));
                return;
              }
              onSubmit({ transientToken: token });
            }
          );
        });
      })
      .catch((err: any) =>
        onError(err instanceof Error ? err : new Error(String(err)))
      );
  },

  destroy: () => {
    microform = null;
  },

  confirm: async (params: ConfirmParams): Promise<ConfirmResult> => {
    const transientToken = (params.data as any)?.transientToken ?? "";

    const res = await fetch(`/api/payments/authorize`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        connector: "cybersource",
        data: { transientToken, ...(params.data as Record<string, any>) },
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
