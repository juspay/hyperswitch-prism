import type { ConnectorDefinition } from "../framework/types.js";
import type { ScenarioId } from "../scenarios/catalog.js";

/** Compile-checked, non-runnable reference for connector configuration. */
export const exampleConnector = {
  id: "example",
  credentialKey: "example-credentials-key",
  credentialEntry: "connector_1",
  card: {
    card_number: "4111111111111111",
  },
  overrides: {
    "payments/create-confirm/card/one-time/no3ds/create-confirm": {
      requests: {
        createPayment: {
          "initial-payment": {
            reason: "Example sandbox requires connector-specific request data",
            omit: [["shipping", "phone"]],
            patch: {
              currency: "EUR",
              description: null,
              metadata: {
                test_case: "synthetic-example",
              },
              connector_metadata: {
                synthetic_program: "example-program",
              },
              order_details: [
                {
                  product_name: "Synthetic product",
                  product_id: "synthetic-product",
                  quantity: 1,
                  amount: 6000,
                  requires_shipping: false,
                },
              ],
            },
          },
        },
      },
      expectations: {
        "confirmation-fields": {
          reason: "Example sandbox returns a documented exact value",
          patch: {
            currency: "EUR",
            description: null,
          },
        },
      },
    },
  },
} as const satisfies ConnectorDefinition<ScenarioId>;

/** Separate because one connector cannot override and exclude the same scenario. */
export const exampleExcludedConnector = {
  id: "example-excluded",
  exclude: {
    "payments/create-confirm/card/one-time/no3ds/create-confirm": {
      reason: "Example connector does not support this documented behavior",
    },
  },
} as const satisfies ConnectorDefinition<ScenarioId>;
