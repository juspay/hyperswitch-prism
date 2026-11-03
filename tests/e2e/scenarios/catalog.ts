import { defineScenario } from "../framework/scenario.js";

export const scenarios = {
  no3dsCreateConfirm: defineScenario({
    id: "no3ds-create-confirm",
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
  }),
} as const;

export type ScenarioId =
  (typeof scenarios)[keyof typeof scenarios]["id"];
