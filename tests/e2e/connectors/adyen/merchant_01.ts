import { connectorFromFile } from "../../framework/connector.js";
import type { ScenarioId } from "../../scenarios/catalog.js";

export default connectorFromFile<ScenarioId>(import.meta.url, {
  overrides: {
    "payments/capture/full-capture": {
      expectations: {
        "capture-fields": {
          reason: "Adyen /captures returns received for asynchronous processing. Verify acceptance only; CAPTURE webhook completion is outside this scenario. https://docs.adyen.com/online-payments/capture",
          patch: { status: "processing", amount_capturable: 6000, amount_received: null },
        },
        "capture-recorded-outcome": {
          reason: "The accepted Adyen capture leaves the active attempt pending until asynchronous completion; this is not a terminal capture assertion.",
          patch: { attemptStatus: "pending" },
        },
      },
    },
  },
});
