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
  threeDsChallenge: {
    password: "password",
    frameSelector: "iframe:visible",
  },
  exclude: {
    "payments/retrieve/force-sync-payment": {
      reason:
        "Adyen PSync requires customer-returned redirect encoded_data; an unfinished 3DS payment is rejected by UCS pre-flight without calling Adyen.",
    },
  },
  overrides: {
    "payments/create-confirm/card/one-time/three-ds/automatic-capture-create-confirm":
      {
        requests: {
          createPayment: {
            "three-ds-payment": {
              reason:
                "Adyen's documented web 3DS2 challenge card is required to exercise the redirect flow",
              patch: {
                payment_method_data: {
                  card: {
                    card_number: "4917610000000000",
                    card_exp_month: "03",
                    card_exp_year: "30",
                    card_holder_name: "Test Customer",
                    card_cvc: "737",
                  },
                },
              },
            },
          },
        },
      },
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
    "payments/create-confirm/card/recurring/connector-mandate/no3ds/nonzero-amount-manual-capture-setup-mandate-then-recurring":
      {
        expectations: {
          "setup-capture-fields": {
            reason:
              "Adyen accepts manual capture asynchronously and completes it after webhook delivery",
            patch: {
              status: "processing",
              amount_capturable: 6000,
              amount_received: null,
            },
          },
          "setup-capture-recorded-outcome": {
            reason:
              "Adyen manual capture remains pending until the asynchronous completion webhook",
            patch: { attemptStatus: "pending" },
          },
        },
      },
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
