import { connectorFromFile } from "../../framework/connector.js";
import type { ScenarioId } from "../../scenarios/catalog.js";

const asynchronousCapture = {
  expectations: {
    "capture-fields": {
      reason:
        "Adyen accepts capture asynchronously and returns processing before webhook completion",
      patch: {
        status: "processing",
        amount_capturable: 6000,
        amount_received: null,
      },
    },
    "capture-recorded-outcome": {
      reason:
        "An accepted Adyen capture leaves the active attempt pending until asynchronous completion",
      patch: { attemptStatus: "pending" },
    },
  },
} as const;

export default connectorFromFile<ScenarioId>(import.meta.url, {
  overrides: {
    "payments/cancel/full-void-authorized-payment": {
      expectations: {
        "cancellation-fields": {
          reason:
            "Adyen accepts cancellation asynchronously before webhook completion",
          patch: { status: "processing" },
        },
        "cancellation-recorded-outcome": {
          reason:
            "An accepted Adyen cancellation leaves the active attempt pending until asynchronous completion",
          patch: { attemptStatus: "pending" },
        },
      },
    },
    "payments/create-confirm/card/one-time/no3ds/manual-capture-create-confirm":
      asynchronousCapture,
    "payments/create-confirm/card/one-time/no3ds/manual-capture-create-then-confirm":
      asynchronousCapture,
    "payments/create-confirm/card/one-time/no3ds/guest-manual-capture-create-confirm":
      asynchronousCapture,
    "refunds/create/full-refund-captured-payment": {
      expectations: {
        "refund-submission-fields": {
          reason:
            "Adyen accepts refunds asynchronously before refund synchronization or webhook completion",
          patch: { status: "pending" },
        },
        "refund-recorded-outcome": {
          reason:
            "The accepted Adyen refund remains pending until asynchronous webhook completion",
          patch: { status: "pending" },
        },
      },
    },
  },
});
