import { connectorFromFile } from "../../framework/connector.js";
import type { ScenarioId } from "../../scenarios/catalog.js";

export default connectorFromFile<ScenarioId>(import.meta.url, {
  exclude: {
    "payments/capture/full-capture": { reason: "Sandbox certification for this connector is deferred to its follow-up coverage change." },
    "payments/create-confirm/card/one-time/no3ds/automatic-capture-create-then-confirm": { reason: "Sandbox certification for this connector is deferred to its follow-up coverage change." },
    "payments/create-confirm/card/one-time/no3ds/manual-capture-create-confirm": { reason: "Sandbox certification for this connector is deferred to its follow-up coverage change." },
    "payments/create-confirm/card/one-time/no3ds/manual-capture-create-then-confirm": { reason: "Sandbox certification for this connector is deferred to its follow-up coverage change." },
    "payments/create-confirm/card/one-time/no3ds/guest-automatic-capture-create-confirm": { reason: "Sandbox certification for this connector is deferred to its follow-up coverage change." },
    "payments/create-confirm/card/one-time/no3ds/guest-manual-capture-create-confirm": { reason: "Sandbox certification for this connector is deferred to its follow-up coverage change." },
  },
  card: {
    card_number: "4012000098765439",
    card_exp_month: "12",
    card_exp_year: "28",
    card_holder_name: "Test Customer",
    card_cvc: "999",
  },
});
