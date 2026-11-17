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
    description: "Separate server confirmation with automatic capture and a new customer. POST /payments with confirm=false, then POST /payments/{id}/confirm using the server key.",
    verifies: "Check requires_confirmation before confirmation, then succeeded and a charged UCS attempt. Also verify ownership, error fields, amounts, preserved checkout details and no mandate setup.",
    requirements: { connectorAccount: { paymentMethodsEnabled: [
      { payment_method: "card", payment_method_types: [{ payment_method_type: "credit" }] },
    ] } },
  },
  "payments/create-confirm/card/one-time/no3ds/manual-capture-create-confirm": {
    title: "One-time No3DS card — manual capture, inline confirmation",
    description: "Inline confirmation reserves funds for a new customer without capturing them. POST /payments with confirm=true and capture_method=manual.",
    verifies: "Check requires_capture, the full amount capturable, no received amount and an authorized UCS attempt. Also verify ownership, error fields, amounts, preserved checkout details and no mandate setup.",
    requirements: { connectorAccount: { paymentMethodsEnabled: [
      { payment_method: "card", payment_method_types: [{ payment_method_type: "credit" }] },
    ] } },
  },
  "payments/create-confirm/card/one-time/no3ds/manual-capture-create-then-confirm": {
    title: "One-time No3DS card — manual capture, create then confirm",
    description: "Separate server confirmation reserves funds without capturing them. POST /payments with confirm=false and manual capture, then POST /payments/{id}/confirm.",
    verifies: "Check requires_confirmation followed by requires_capture and an authorized UCS attempt. Also verify ownership, error fields, amounts, preserved checkout details and no mandate setup.",
    requirements: { connectorAccount: { paymentMethodsEnabled: [
      { payment_method: "card", payment_method_types: [{ payment_method_type: "credit" }] },
    ] } },
  },
  "payments/create-confirm/card/one-time/no3ds/guest-automatic-capture-create-confirm": {
    title: "Guest No3DS card — automatic capture, inline confirmation",
    description: "Inline automatic-capture payment without creating or associating a customer. POST /payments with confirm=true, checkout addresses and order details, omitting customer.",
    verifies: "Check succeeded and a charged UCS attempt, while customer fields and customer record remain absent. Also verify ownership, error fields, amounts, preserved checkout details and no mandate setup.",
    requirements: { connectorAccount: { paymentMethodsEnabled: [
      { payment_method: "card", payment_method_types: [{ payment_method_type: "credit" }] },
    ] } },
  },
  "payments/create-confirm/card/one-time/no3ds/guest-manual-capture-create-confirm": {
    title: "Guest No3DS card — manual capture, inline confirmation",
    description: "Inline manual authorization without creating or associating a customer. POST /payments with confirm=true and capture_method=manual, omitting customer.",
    verifies: "Check requires_capture, an authorized UCS attempt and no customer association or customer record. Also verify ownership, error fields, amounts, preserved checkout details and no mandate setup.",
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
