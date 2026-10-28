# End-to-end tests

This scaffold reserves directories for the E2E framework. It contains documentation and empty placeholders only. Executable tests, dependencies, credentials and runtime configuration are added separately.

## Layout

```text
tests/e2e/
├── framework/       # Fixtures, merchant setup, API operations, checks and reporting
├── connectors/      # Data-only definitions and connector registry
├── stack/           # Local services and readiness
├── scripts/         # Runtime preparation
└── scenarios/       # Shared journeys
```

## Scenarios

Organize scenarios by resource, operation under test, payment method, and relevant variation. Each spec owns the complete journey, including prerequisites; a capture spec creates its own authorized payment before capturing it.

```text
scenarios/
├── payments/
│   ├── create-confirm/
│   │   ├── card/
│   │   │   ├── one-time/
│   │   │   │   ├── no3ds/
│   │   │   │   │   ├── automatic-create-confirm.spec.ts
│   │   │   │   │   ├── automatic-create-then-confirm.spec.ts
│   │   │   │   │   ├── manual-create-confirm.spec.ts
│   │   │   │   │   └── manual-create-then-confirm.spec.ts
│   │   │   │   └── three-ds/
│   │   │   └── recurring/
│   │   │       ├── connector-mandate/
│   │   │       │   ├── zero-amount-setup-then-recurring.spec.ts
│   │   │       │   └── nonzero-amount-setup-then-recurring.spec.ts
│   │   │       └── network-transaction-id/
│   │   ├── wallet/
│   │   │   ├── apple-pay/
│   │   │   ├── google-pay/
│   │   │   └── paypal/
│   │   ├── bank-debit/
│   │   ├── bank-redirect/
│   │   ├── bank-transfer/
│   │   ├── pay-later/
│   │   └── upi/
│   ├── retrieve/
│   ├── capture/
│   │   └── full-capture.spec.ts
│   └── cancel/
│       └── cancel-authorized-payment.spec.ts
└── refunds/
    ├── create/
    │   └── full-refund-after-automatic-capture.spec.ts
    └── sync/
```


`create-confirm/` groups initial payment journeys. In filenames, `create-confirm` means creation with `confirm: true`; `create-then-confirm` means separate requests. Automatic and manual describe capture mode.

Recurring specs belong under their reference mechanism, for example `recurring/connector-mandate/zero-amount-setup-then-recurring.spec.ts` and `nonzero-amount-setup-then-recurring.spec.ts`. Each owns setup followed by a subsequent payment. Add network-transaction-ID variants where supported. Make authentication variations explicit in the relevant filename or subdirectory.

Later operations have separate directories. Their specs declare required payment state and history, including authentication where relevant, rather than repeating every authorization dimension. Keep capture, cancel, retrieve and refund scenarios flat until implemented method-specific behavior warrants grouping. Wallets use wallet-specific variations instead of universally inheriting card's authentication tree.

Payment retrieval and forced synchronization share `payments/retrieve/`, following HS's GET payment endpoint and its `force_sync` option. This directory remains empty until those dedicated scenarios are implemented.

Empty specs and `.gitkeep` directories are extension placeholders, not implemented coverage. Add one shared spec per journey; connectors inherit it through the registry.

The first implementation targets `scenarios/payments/create-confirm/card/one-time/no3ds/automatic-create-confirm.spec.ts`. Every spec in this scaffold remains empty.

## Responsibilities

- Specs own the complete call sequence, request data, response dependencies and expected results.
- `framework/fixtures.ts` owns setup and cleanup. `merchant-setup.ts` provisions an isolated merchant, profile, API key, sandbox connector account and routing.
- `hyperswitch-client.ts` owns HTTP transport. Resource modules such as `payment-operations.ts` execute individual calls and return responses.
- `assertions.ts` produces shared assertions and expected/actual comparisons. `report.ts` groups calls and checks; `ucs-audit.ts` attaches correlated masked evidence afterward.
- `payment-records.ts` reads recorded payment facts; the spec asserts them.
- `connector-credentials.ts` loads external sandbox credentials, `configuration.ts` resolves runtime options, and `types.ts` defines shared types.
- Connector definitions supply data, justified request overrides and exclusions with reasons. The registry selects connectors; shared specs retain journeys and expected outcomes.
- `stack/` owns service startup and readiness; `scripts/` owns preparation such as external credential download/decryption.

Use ordinary TypeScript, standard Playwright fixtures and typed helpers. Ordinary journey additions require a shared spec; add endpoint helpers when needed. Do not add a custom scenario interpreter or source-scanning enforcement. Never commit credentials or production data.
