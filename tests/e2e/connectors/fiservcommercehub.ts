import { connectorFromFile } from "../framework/connector.js";
import type { ScenarioId } from "../scenarios/catalog.js";

export default connectorFromFile<ScenarioId>(import.meta.url, {
  overrides: {
    "payments/create-confirm/card/one-time/no3ds/create-confirm": {
      requests: {
        createPayment: {
          "initial-payment": {
            reason:
              "Fiserv Commerce Hub maps merchant invoice metadata into its transaction details",
            patch: {
              metadata: {
                merchant_invoice_id: "synthetic-01",
              },
            },
          },
        },
      },
      expectations: {
        "confirmation-fields": {
          reason:
            "The HS response preserves the synthetic merchant invoice metadata",
          patch: {
            metadata: {
              merchant_invoice_id: "synthetic-01",
            },
          },
        },
      },
    },
  },
});
