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
│   │   │   │   │   ├── create-confirm.spec.ts
│   │   │   │   │   ├── automatic-capture-create-then-confirm.spec.ts
│   │   │   │   │   ├── manual-capture-create-confirm.spec.ts
│   │   │   │   │   └── manual-capture-create-then-confirm.spec.ts
│   │   │   │   └── three-ds/
│   │   │   └── recurring/
│   │   │       ├── connector-mandate/
│   │   │       │   ├── no3ds/
│   │   │       │   │   ├── zero-amount-setup-mandate-then-recurring.spec.ts
│   │   │       │   │   └── nonzero-amount-setup-mandate-then-recurring.spec.ts
│   │   │       │   └── three-ds/
│   │   │       └── network-transaction-id/
│   │   │           ├── no3ds/
│   │   │           └── three-ds/
│   │   └── wallet/
│   │       ├── apple-pay/
│   │       ├── google-pay/
│   │       └── paypal/
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


`create-confirm.spec.ts` exercises a fully populated one-time inline-confirmation request. It uses automatic capture for immediate success; it is not a dedicated capture-mode test. Mutually exclusive journeys such as mandate setup, manual capture and SDK checkout have separate specs.

Order item `amount` is per unit: the sum of `amount * quantity` must equal the payment amount. The scenario sends two units at 3000 minor units each and checks the returned item fields. Inline customer creation supplies a fresh customer ID; processor-side customer creation is connector-dependent and is not universally required.

`create-confirm/` groups initial payment journeys. In filenames, `create-confirm` means creation with `confirm: true`; `create-then-confirm` means separate requests. Use the explicit filename prefixes `automatic-capture` and `manual-capture`.

Recurring specs belong under their reference mechanism and mandate-setup authentication, for example `recurring/connector-mandate/no3ds/zero-amount-setup-mandate-then-recurring.spec.ts`. Each owns mandate setup followed by a subsequent payment. The `no3ds` and `three-ds` branches describe setup authentication, not authentication repeated on the subsequent recurring payment. Add network-transaction-ID variants only where supported.

Later operations have separate directories. Their specs declare required payment state and history, including authentication where relevant, rather than repeating every authorization dimension. Keep capture, cancel, retrieve and refund scenarios flat until implemented method-specific behavior warrants grouping. Wallets use wallet-specific variations instead of universally inheriting card's authentication tree.

Payment retrieval and forced synchronization share `payments/retrieve/`, following HS's GET payment endpoint and its `force_sync` option. This directory remains empty until those dedicated scenarios are implemented.

SDK-driven journeys can share these same payment groupings. Make SDK execution explicit in the filename, for example `automatic-capture-create-then-confirm-with-sdk.spec.ts`, and exercise the actual browser checkout. Reuse shared setup, API operations and checks; keep browser interactions and the journey visible in the spec. SDK setup and implementation will be introduced with the first such scenario.

Empty specs and `.gitkeep` directories are extension placeholders, not implemented coverage. Add one shared spec per journey; connectors inherit it through the registry.

The first implementation targets `scenarios/payments/create-confirm/card/one-time/no3ds/create-confirm.spec.ts`. Every spec in this scaffold remains empty.

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
