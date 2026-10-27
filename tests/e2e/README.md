# End-to-end tests

This directory reserves the structure for end-to-end payment tests. This PR adds directories and documentation only; executable tests, dependencies, credentials, and CI configuration are added separately.

```text
tests/e2e/
  framework/
    configuration.ts          # Runtime URLs and options
    credentials.ts            # External sandbox credential loading
    hyperswitch.ts             # Typed Hyperswitch API access
    merchant-setup.ts          # Merchant, profile, API key, connector, and routing setup
    scenario.ts                # Typed scenario and connector contracts
    runner.ts                  # Shared lifecycle execution and assertions
  connectors/
    adyen.ts                   # Connector capabilities and sandbox test data
    index.ts                   # Registered connectors
  scenarios/
    payments/
      one-time/
        server/
          card.spec.ts         # Server-side one-time card scenarios
      mandates/
        customer-initiated/
          card.spec.ts
        zero-amount/
          card.spec.ts
      recurring/
        connector-mandate/
          card.spec.ts
        network-transaction-id/
          card.spec.ts
  stack/
    compose.yml                # Isolated local dependencies
    stack.mjs                  # Stack startup and shutdown
  playwright.config.ts
  package.json
  package-lock.json
  tsconfig.json
```

## Structure

- `framework/` owns common setup, API calls, lifecycle execution, assertions, and reporting. Scenario and connector declarations contain data, not custom execution callbacks.
- `connectors/` declares connector capabilities, credential keys, and sandbox test data. A connector does not duplicate scenario execution.
- `scenarios/` describes payment behavior. The path identifies the product area, lifecycle, integration channel, and payment-method family; typed fields describe variants such as No3DS or 3DS and automatic or manual capture.
- `stack/` operates the isolated local services required by the tests and remains independent of payment behavior.

The first implementation targets `scenarios/payments/one-time/server/card.spec.ts`. A scenario declaration describes the intended behavior, while the shared runner owns its API sequence and assertions. Adding another compatible connector reuses that scenario.

The mandate and recurring directories are reserved extension points and do not represent implemented coverage in this scaffold. New flows should be added to the matching lifecycle rather than copied into connector directories.

The `.gitkeep` files preserve the empty directories until their implementations are added.
