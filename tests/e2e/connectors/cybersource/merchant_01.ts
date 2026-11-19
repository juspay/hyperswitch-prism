import { connectorFromFile } from "../../framework/connector.js";
import type { ScenarioId } from "../../scenarios/catalog.js";

export default connectorFromFile<ScenarioId>(import.meta.url, {
  exclude: {
    "payments/cancel/full-void-authorized-payment": {
      reason:
        "Sandbox certification for this connector is deferred to its follow-up coverage change.",
    },
    "payments/create-confirm/card/one-time/three-ds/automatic-capture-create-confirm":
      {
        reason:
          "Completed browser 3DS sandbox certification is deferred to its follow-up coverage change.",
      },
    "payments/create-confirm/card/recurring/connector-mandate/no3ds/nonzero-amount-setup-mandate-then-recurring":
      {
        reason:
          "Connector-mandate recurring sandbox certification is deferred to its follow-up coverage change.",
      },
    "payments/retrieve/force-sync-payment": {
      reason:
        "3DS PSync sandbox certification for this connector is deferred to its follow-up coverage change.",
    },
    "refunds/create/full-refund-captured-payment": {
      reason:
        "Sandbox certification for this connector is deferred to its follow-up coverage change.",
    },
    "payments/create-confirm/card/one-time/no3ds/automatic-capture-create-then-confirm":
      {
        reason:
          "Sandbox certification for this connector is deferred to its follow-up coverage change.",
      },
    "payments/create-confirm/card/one-time/no3ds/manual-capture-create-confirm":
      {
        reason:
          "Sandbox certification for this connector is deferred to its follow-up coverage change.",
      },
    "payments/create-confirm/card/one-time/no3ds/manual-capture-create-then-confirm":
      {
        reason:
          "Sandbox certification for this connector is deferred to its follow-up coverage change.",
      },
    "payments/create-confirm/card/one-time/no3ds/guest-automatic-capture-create-confirm":
      {
        reason:
          "Sandbox certification for this connector is deferred to its follow-up coverage change.",
      },
    "payments/create-confirm/card/one-time/no3ds/guest-manual-capture-create-confirm":
      {
        reason:
          "Sandbox certification for this connector is deferred to its follow-up coverage change.",
      },
  },
  credentialEntry: "connector_1",
});
