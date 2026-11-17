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
  run: ["payments/create-confirm/card/one-time/no3ds/create-confirm"],
});
