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
    api/
      payments/
        one-time/
          card/
            no3ds-automatic-create-confirm.spec.ts
            no3ds-automatic-create-then-confirm.spec.ts
            no3ds-manual-create-confirm-then-capture.spec.ts
            no3ds-manual-create-confirm-then-cancel.spec.ts
            no3ds-automatic-create-confirm-then-refund.spec.ts
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
- `scenarios/api/` describes journeys driven through direct HS API calls, using server or client credentials as appropriate. The path identifies the product area, lifecycle, and payment-method family. Future tests operating an actual Web SDK checkout belong under `scenarios/web-sdk/checkout/`.
- `stack/` operates the isolated local services required by the tests and remains independent of payment behavior.

Each scenario has its own file. `create-confirm` means a single create request with `confirm: true`; `create-then-confirm` means separate create and confirm requests. `automatic` and `manual` describe capture mode. Retrieval and verification steps are declared within each scenario.

The five card scenario files are empty placeholders. The first implementation targets `scenarios/api/payments/one-time/card/no3ds-automatic-create-confirm.spec.ts`. Adding another compatible connector reuses that scenario.

The mandate and recurring directories are reserved extension points and do not represent implemented coverage in this scaffold. New flows should be added to the matching lifecycle rather than copied into connector directories.

The `.gitkeep` files preserve the empty directories until their implementations are added.
