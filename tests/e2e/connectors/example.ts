import type { ConnectorDefinition } from "../framework/types.js";

/**
 * Compile-checked reference for connector configuration.
 *
 * This connector is intentionally not registered in connectors/index.ts, so it
 * is never provisioned or executed. Copy only the fields a real connector
 * needs; do not copy the complete example by default.
 */
export const exampleConnector = {
  id: "example",

  // Omit both fields when the external credentials file uses the connector ID
  // and contains one unambiguous entry.
  credentialKey: "example-credentials-key",
  credentialEntry: "connector_1",

  // The shared sandbox card is the default. Override only fields required by
  // this connector's sandbox; unspecified card fields remain shared.
  card: {
    card_number: "4111111111111111",
  },

  overrides: {
    "example-single-call-scenario": {
      requests: {
        createPayment: {
          "initial-payment": {
            reason: "Example sandbox requires connector-specific request data",
            // Omission happens before patching. Paths are field-name arrays,
            // so nested boundaries are explicit and validated at runtime.
            omit: [["shipping", "phone"], ["connector_metadata"]],
            patch: {
              currency: "EUR",
              description: null,
              metadata: {
                test_case: "synthetic-example",
              },
              connector_metadata: {
                synthetic_program: "example-program",
              },
              // Arrays replace the shared array instead of merging by index.
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

    // Stable call IDs distinguish repeated uses of the same operation.
    "example-multi-call-scenario": {
      requests: {
        createPayment: {
          "mandate-setup": {
            reason: "Example mandate setup requires sandbox metadata",
            patch: {
              connector_metadata: {
                synthetic_mode: "setup",
              },
            },
          },
          "recurring-payment": {
            reason: "Example recurring charge requires different metadata",
            patch: {
              metadata: {
                test_case: "synthetic-recurring",
              },
            },
          },
        },
      },
    },
  },

  exclude: {
    "example-unsupported-scenario": {
      reason: "Example connector sandbox does not support this journey",
    },
  },
} as const satisfies ConnectorDefinition;
