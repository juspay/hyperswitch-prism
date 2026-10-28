# End-to-end tests

These tests exercise payment flows through Hyperswitch and UCS against processor sandbox environments.

## Run locally

Requirements: Node.js 22+, Docker Compose 2.24.4+, and a Hyperswitch-compatible connector credentials JSON file. The Compose override uses the standard `!override` and `!reset` merge tags.

```sh
cd tests/e2e
npm ci
npm run stack:up
CONNECTOR_AUTH_FILE_PATH=/path/to/connector-auth.json npm test
```

Open the HTML report with `npx playwright show-report`. Stop the isolated stack with `npm run stack:down`; database volumes are retained.

`HS_IMAGE` and `UCS_IMAGE` select container images. Defaults are pinned to tested immutable digests in `stack/stack.mjs`; supporting service images and the gRPC health probe are also pinned. Update pins deliberately and rerun the sandbox scenario. The versioned Diesel installer is checked against its recorded SHA-256 before execution; that installer also verifies the platform binary checksum. Configuration and migrations come from the matching Hyperswitch source revision, cached after the first run. `HS_SOURCE_DIR` can supply an existing matching checkout; `HS_REF` can explicitly select its revision. Use reviewed versions or digests for image overrides in Jenkins.

HS uses its supported `ROUTER__...` environment overrides; upstream TOML is unchanged. The UCS wrapper adds the standard gRPC health probe, and Compose starts HS after UCS is healthy. Redis is internal to the Compose network; PostgreSQL binds to loopback on `HS_DB_PORT` (default `15432`). `HS_PORT` sets the local HS port (default `8080`).

Stack startup reads `connectors/index.ts` and configures HS's `ucs_only_connectors` from that registry. Registering a connector therefore also selects UCS-only execution in this test stack; there is no separate connector list or merchant rollout configuration. Restart the stack after changing the registry. `tsx` lets the startup script import the same TypeScript registry used by Playwright.

Merchant setup enables UCS before payment calls. Merchant, profile, API-key, connector-account, and routing administration remain HS operations. The scenario verifies the payment's recorded UCS gateway. Response metadata and database state describe the payment, so neither alone proves a fresh UCS PSync call. Proving an individual flow requires its correlated UCS events; the current scenario does not assert a fresh PSync call.

## Report

Each test shows **Scenario** and **Verifies** descriptions, ordered API steps, and expected/actual comparison attachments. Group one API call and its assertions inside `report.step(...)`. It displays the call, the checks, and then the correlated audit evidence, including when a check fails. The scenario still owns every call and assertion.

After each payment or refund response, a separate collector matches its `x-request-id` against UCS's processed, masked event JSON. UCS logs this payload at INFO level regardless of Kafka being enabled. The collector accepts the dedicated `ucs_event` field when present, or the exact existing event-publisher message format from `ucs_common_utils::events`. It parses only the event JSON and never attaches the surrounding log record. Attachments are named by API call, flow, and stage (`GrpcRequest` or `ConnectorCall`) and contain the original masked event, including request/response data. Repeated calls are retained and ordered by event timestamp.

Keep UCS console logging enabled at INFO level with JSON log format, as configured by the local stack. `UCS_LOG_CONTAINER` can select the UCS container for a dedicated test stack; otherwise the collector selects the local Compose project's UCS service. Missing events or bounded Docker-log collection failures produce annotations, not empty event attachments. Collection is diagnostic and does not assert completeness or turn a stored-state retrieval into proof of PSync execution. Credentials and unmasked HS request/response bodies are never attached.

`UCS_ENVIRONMENT` selects the UCS service configuration: `development` (default) loads `development.toml`; `sandbox` loads `sandbox.toml`. Set it explicitly when starting a CI stack:

```sh
# Local default
npm run stack:up

# CI runtime configuration
UCS_ENVIRONMENT=sandbox npm run stack:up
```

There is no automatic Jenkins detection. `stack:up` accepts only `development` and `sandbox`, rejecting `production`. In both modes, merchant connector accounts use `test_mode: true` and sandbox credentials. Changing the service configuration does not change that payment-test setting or select a credentials source; CI downloads credentials separately as described below. The UCS build context is limited to `stack/`; its Dockerfile adds a health probe to the published image without compiling UCS.

For an existing dedicated test stack, set `HS_BASE_URL`, `HS_ADMIN_API_KEY`, and `HS_DATABASE_URL` as appropriate.

## Credentials

`CONNECTOR_AUTH_FILE_PATH` always points to a local, decrypted HS connector credentials JSON file, not UCS regression TOML. Local runs can export that variable or set it on the `npm test` command.

The loader uses the connector's `id` as the credentials key by default. Set the optional `credentialKey` only when the external file uses a different key. It accepts a direct entry such as `adyen.connector_account_details`, or a nested entry such as `adyen.connector_1.connector_account_details`. A single nested credential entry is selected automatically. If the connector has several entries, set `credentialEntry: "connector_1"` in its definition. Multiple entries are never selected by object ordering.

The credential entry's optional `metadata` is forwarded to the merchant connector account during setup. This is distinct from payment `metadata` or `connector_metadata` in a payment request. The account fields `additional_merchant_data` and `connector_webhook_details` are also forwarded when supplied in the external entry. Supply actual connector-required account data in that file; use request overrides only for justified payment-request differences. Absent or null optional account fields are omitted.

### CI preparation

CI should fetch and decrypt credentials before starting the test process. It then supplies the same local file path used by local runs. This keeps AWS access and decryption outside payment tests; a Jenkins pipeline can use this preparation without changing the framework.

Use disposable Jenkins agents with sandbox-only secrets, minimal permissions, and restricted network access. Do not give untrusted fork code access to credentials or the Docker daemon.

For Jenkins, provide an AWS identity with access to the encrypted object, `CONNECTOR_AUTH_S3_URI`, and either a secret file path in `CONNECTOR_AUTH_PASSPHRASE_FILE` or the `CONNECTOR_AUTH_PASSPHRASE` secret used by HS Cypress. If both are supplied, the file takes precedence. With AWS CLI and GnuPG available:

```sh
npm run test:s3
```

The command downloads and decrypts into a private temporary directory, runs `npm test` with `CONNECTOR_AUTH_FILE_PATH`, and removes the downloaded and decrypted files on exit, including test failures. It forwards test arguments, such as `npm run test:s3 -- --grep adyen`. The URI and passphrase come from Jenkins configuration, not this repository; no Jenkins pipeline is included. If Jenkins already provides a decrypted JSON secret file, set `CONNECTOR_AUTH_FILE_PATH` and run `npm test` directly. Do not include downloaded or decrypted files in test artifacts.

## Scenarios

Before payment calls, fixtures create an isolated merchant, issue an API key expiring in one hour, use its default business profile (or create one if absent), create the sandbox connector account, and activate a single-connector routing rule. Setup validates the returned merchant association, profile, sandbox mode, and enabled payment methods.

Each scenario declares `paymentMethodsEnabled`, which is sent as the connector account's `payment_methods_enabled`. The current scenario enables card credit only. Declare additional methods in scenarios that exercise them; the common setup does not assume every test uses cards. Optional merchant contact fields, webhook subscriptions, parent merchants, tokenization, and mandate settings are not prerequisites for this fresh-card scenario and are not enabled implicitly. Customer creation belongs in a journey that actually uses a customer.

The implemented Adyen scenario requires `succeeded` in the create/confirm response, the full amount received, and zero amount left capturable. It includes inline customer creation, full billing and shipping, browser information, description, metadata, a merchant order reference and order items. It checks their returned values, merchant/profile ownership, one attempt, a normalized return URL, null error/mandate fields, generated reference presence and timestamp validity, then verifies the recorded charged attempt through UCS and existence of the associated customer record. Generated processor references are reported only as presence checks. It does not retrieve or synchronize the payment, accept `processing`, poll through intermediate states, or exercise failure scenarios.

Each file contains one complete shared scenario. Read it top to bottom to see the requests, response checks, polling, and database evidence. Calls run in the order written.

```ts
const payment = await report.step("Create and confirm payment", async () => {
  const response = await payments.createPayment({
    profile_id: merchant.profileId,
    amount: 6000,
    currency: "USD",
    confirm: true,
    payment_method: "card",
    payment_method_data: { card: createCard(connector.card) },
  });
  await report.check("Confirmation", {
    actual: response.status,
    expected: "succeeded",
  }, ({ actual, expected }) => expect(actual).toBe(expected));
  return response;
});

await report.step("Verify recorded payment outcome", async () => {
  const recorded = await payments.readRecordedPayment(payment.payment_id);
  await report.check("Recorded payment outcome", {
    actual: recorded,
    expected: {
      gateway: "unified_connector_service",
      attemptStatus: "charged",
      connectorTransactionIdPresent: true,
      customerRecordExists: true,
    },
  }, ({ actual, expected }) => expect(actual).toEqual(expected));
});
```

Responses are ordinary objects. Any returned field can be passed to a later operation using normal TypeScript variables. Requests use HS API field names. Operations apply configured connector request differences before sending; they do not duplicate the full HS API schema or add payment defaults. The shared operations use the provisioned merchant's server API key.

Use native Playwright `expect` for equality, partial nested objects, arrays, ranges and other assertions. `report.check(name, { actual, expected }, assertion)` runs the supplied assertion and attaches those selected values even if it fails. It does not interpret matchers or define another assertion language. Mutating requests are never retried automatically.

Choose `toMatchObject` for partial nested objects and `toEqual` for exact object/array equality. A range can use `report.check("Amount received > 0", { actual: response.amount_received, expected: 0 }, ({ actual, expected }) => expect(actual).toBeGreaterThan(expected))`. Expected/actual values are explicitly selected for the report: do not attach entire payment responses, credentials or card data. Use boolean presence checks for sensitive references. The scenario still owns all matching logic.

Only `scenarios/payments/create-confirm/card/one-time/no3ds/create-confirm.spec.ts` is implemented. Other specs and reserved directories are empty placeholders. The current sequence is:

```text
POST /payments with confirm:true
  -> check success, customer, addresses, browser information, order details and payment fields
Read the payment and its active attempt from the test HS database
  -> check gateway, attempt status, connector transaction ID presence and customer record existence
```

`payments.readRecordedPayment(payment.payment_id)` returns the recorded `gateway`, `attemptStatus`, `connectorTransactionIdPresent`, and `customerRecordExists`. Customer existence is checked against the payment's merchant and customer ID; customer contact data is not read. The boolean is computed from `connector_transaction_id IS NOT NULL AND connector_transaction_id <> ''`; the identifier itself is not returned. Status is asserted in the spec rather than used to filter the query. The connection uses a read-only session; an existing stack should also supply SELECT-only credentials through `HS_DATABASE_URL` for least privilege. The local default remains the stack's database account.

The database read does not trigger or prove a fresh processor PSync call. It requires the corresponding HS database schema.

## Add coverage

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


Follow [CONTRIBUTING.md](CONTRIBUTING.md) for the reference pattern, file boundaries, and review checklist. The supported public fixture API is `test`, `connector`, `scenario`, `merchant` identifiers, `payments`, and `report`.

Scenarios belong to the shared superset under `scenarios/`. Connectors automatically inherit all registered scenarios. Keep connector differences in `connectors/`; do not copy scenarios into connector directories.

- Add a connector definition and register it in `connectors/index.ts`.
- Use the shared sandbox card by default; supply optional `card` field overrides only when required by the connector's sandbox.
- Exclude an unsupported scenario explicitly with a nonempty reason. Exclusions appear as skipped tests.
- Add a new shared scenario for a new payment journey. Its function owns the ordered calls and expected results.
- Add reusable operations or setup helpers when the new journey needs them. Operation helpers perform individual calls; they do not decide the journey.

Connector overrides are ordinary data, keyed by shared scenario ID:

```ts
overrides: {
  "scenario-id": {
    requests: {
      createPayment: { metadata: { order_type: "standard" } },
    },
  },
},
exclude: {
  "another-scenario-id": { reason: "Explain the unsupported behavior" },
},
```

Operations apply their request overrides automatically. Overrides replace complete top-level fields; nested objects are not implicitly merged. Shared expected outcomes remain in the spec and cannot be replaced by connector configuration. Cards, credentials, and connector metadata do not justify a separate connector-specific lifecycle. Features needing extra setup should use a shared setup helper and explicit scenario requirements when introduced.

`create-confirm` means a single create request with `confirm:true`; `create-then-confirm` means separate requests. `automatic` and `manual` describe capture mode. The filename identifies the scenario; its code states the full journey.

`scenarios/` contains journeys driven through HS API calls.

## File responsibilities

`test-data.ts` supplies fresh address, contact, customer, browser and sandbox-card data. Specs select factories explicitly and assemble requests themselves. Customer IDs are generated per call. Keep amounts, capture mode, order items, scenario-specific metadata and expected outcomes in each spec; data factories do not submit requests or choose journeys.

| File | Responsibility |
| --- | --- |
| `test-data.ts` | Reusable test-data factories and a default sandbox card; no API calls or shared mutable response state. |
| `scenarios/**/*.spec.ts` | Own the complete journey, request data, use of responses, and expected results. |
| `connectors/adyen.ts` | Declare sandbox data, optional request overrides, and reasoned exclusions. |
| `connectors/index.ts` | Register connector definitions. |
| `framework/types.ts` | Define scenario metadata and data-only connector configuration types. |
| `framework/fixtures.ts` | Standard Playwright fixtures apply exclusions, provision context, and release resources. Export a typed public API without internal HTTP/keys/polling. No payment sequence. |
| `framework/payment-operations.ts` | Execute individual payment operations and return responses; expose recorded-payment reads. |
| `framework/merchant-setup.ts` | Configure UCS and provision the merchant, API key, profile, connector account, and routing rule. |
| `framework/hyperswitch-client.ts` | Send authenticated HS HTTP requests and handle transport/HTTP errors. |
| `framework/ucs-audit.ts` | Correlate and attach UCS's existing masked structured events; never attach raw logs. |
| `framework/report.ts` | Group API steps, attach selected expected/actual values around native assertions, then attach correlated audit evidence, including on failure. |
| `framework/payment-records.ts` | Read the payment gateway, active-attempt status, connector transaction ID presence and associated customer record existence from the dedicated test database. |
| `framework/connector-credentials.ts` | Load external sandbox credentials. |
| `framework/configuration.ts` | Resolve runtime URLs and options. |
| `stack/compose.yml` | Configure local services and readiness dependencies. |
| `stack/ucs.Dockerfile` | Add the gRPC health-check executable to the published UCS image. |
| `stack/stack.mjs` | Prepare matching runtime assets and start or stop Compose. |
| `scripts/test-with-s3-credentials.sh` | Download/decrypt Jenkins credentials, run tests, and remove temporary credentials. |
| `playwright.config.ts` | Configure discovery, execution, and reporters. |
| `package.json`, `package-lock.json`, `tsconfig.json` | Commands, dependencies, and compiler settings. |

Connector credentials are read only at runtime. Do not commit credentials, reports, runtime files, or copied production data. API tracing is disabled because it can capture credentials and payment-method data.
