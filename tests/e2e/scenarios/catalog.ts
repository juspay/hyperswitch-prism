import {
  createScenarioResolver,
  defineScenarios,
} from "../framework/scenario.js";

export const scenarios = defineScenarios({
  "payments/capture/full-capture": {
    title: "Full capture of an authorized No3DS card payment",
    description: "Authorize a richly populated card payment, request full capture and verify its immediate API and recorded outcome. Asynchronous webhook completion and settlement are separate coverage.",
    verifies: "Verify authorization, capture identity, exact status, received/capturable amounts and active attempt state. Connector overrides explicitly document asynchronous acceptance when applicable.",
    requirements: { connectorAccount: { paymentMethodsEnabled: [
      { payment_method: "card", payment_method_types: [{ payment_method_type: "credit" }] },
    ] } },
  },
  "payments/create-confirm/card/one-time/no3ds/automatic-capture-create-then-confirm": {
    title: "One-time No3DS card — automatic capture, create then confirm",
    description: "Create a synthetic No3DS payment and verify the requested capture mode and confirmation sequence through HS and UCS.",
    verifies: "Check the immediate response, customer and order fields, ownership, errors, amounts, and the recorded active attempt without forcing synchronization.",
    requirements: { connectorAccount: { paymentMethodsEnabled: [
      { payment_method: "card", payment_method_types: [{ payment_method_type: "credit" }] },
    ] } },
  },
  "payments/create-confirm/card/one-time/no3ds/manual-capture-create-confirm": {
    title: "One-time No3DS card — manual capture, inline confirmation",
    description: "Create a synthetic No3DS payment and verify the requested capture mode and confirmation sequence through HS and UCS.",
    verifies: "Check the immediate response, customer and order fields, ownership, errors, amounts, and the recorded active attempt without forcing synchronization.",
    requirements: { connectorAccount: { paymentMethodsEnabled: [
      { payment_method: "card", payment_method_types: [{ payment_method_type: "credit" }] },
    ] } },
  },
  "payments/create-confirm/card/one-time/no3ds/manual-capture-create-then-confirm": {
    title: "One-time No3DS card — manual capture, create then confirm",
    description: "Create a synthetic No3DS payment and verify the requested capture mode and confirmation sequence through HS and UCS.",
    verifies: "Check the immediate response, customer and order fields, ownership, errors, amounts, and the recorded active attempt without forcing synchronization.",
    requirements: { connectorAccount: { paymentMethodsEnabled: [
      { payment_method: "card", payment_method_types: [{ payment_method_type: "credit" }] },
    ] } },
  },
  "payments/create-confirm/card/one-time/no3ds/guest-automatic-capture-create-confirm": {
    title: "Guest No3DS card — automatic capture, inline confirmation",
    description: "Create a synthetic No3DS payment and verify the requested capture mode and confirmation sequence through HS and UCS.",
    verifies: "Check the immediate response, customer and order fields, ownership, errors, amounts, and the recorded active attempt without forcing synchronization.",
    requirements: { connectorAccount: { paymentMethodsEnabled: [
      { payment_method: "card", payment_method_types: [{ payment_method_type: "credit" }] },
    ] } },
  },
  "payments/create-confirm/card/one-time/no3ds/guest-manual-capture-create-confirm": {
    title: "Guest No3DS card — manual capture, inline confirmation",
    description: "Create a synthetic No3DS payment and verify the requested capture mode and confirmation sequence through HS and UCS.",
    verifies: "Check the immediate response, customer and order fields, ownership, errors, amounts, and the recorded active attempt without forcing synchronization.",
    requirements: { connectorAccount: { paymentMethodsEnabled: [
      { payment_method: "card", payment_method_types: [{ payment_method_type: "credit" }] },
    ] } },
  },
  "payments/create-confirm/card/one-time/no3ds/create-confirm": {
    title:
      "One-time No3DS card payment — create and confirm with customer and order details",
    description:
      "Create and confirm a one-time No3DS card payment with a new customer, billing, shipping and order details in one API request. Verify succeeded status, the full amount received and the recorded charged attempt through UCS.",
    verifies:
      "Verify customer creation and association, request details preserved in the response, full amount received with nothing left capturable, merchant and profile ownership, one attempt, no errors or mandate setup, and a recorded UCS gateway and connector transaction ID.",
    requirements: {
      connectorAccount: {
        paymentMethodsEnabled: [
          {
            payment_method: "card",
            payment_method_types: [{ payment_method_type: "credit" }],
          },
        ],
      },
    },
  },
});

export type ScenarioId = keyof typeof scenarios;

/** The only supported constructor for executable scenarios. */
export const scenarioFromFile = createScenarioResolver(scenarios);
