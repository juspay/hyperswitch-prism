# End-to-end tests

These tests exercise payment flows through Hyperswitch and UCS against processor sandbox environments.

## Run locally

Requirements: Node.js 22+, Docker with Compose, and a Hyperswitch-compatible connector credentials JSON file.

```sh
cd tests/e2e
npm ci
npm run stack:up
CONNECTOR_AUTH_FILE_PATH=/path/to/connector-auth.json npm test
```

Open the HTML report with `npx playwright show-report`. Stop the isolated stack with `npm run stack:down`; database volumes are retained.

`HS_IMAGE` and `UCS_IMAGE` select container images. Defaults use the latest published images. Configuration and migrations come from the matching Hyperswitch source revision, cached after the first run. `HS_SOURCE_DIR` can supply an existing matching checkout; `HS_REF` can explicitly select its revision.

HS uses its supported `ROUTER__...` environment overrides; upstream TOML is unchanged. The UCS wrapper adds the standard gRPC health probe, and Compose starts HS after UCS is healthy. Redis is internal to the Compose network; PostgreSQL binds to loopback on `HS_DB_PORT` (default `15432`). `HS_PORT` sets the local HS port (default `8080`). `UCS_CONNECTORS` selects connectors routed to UCS in this local stack (default `adyen`); extend it when registering connectors.

For an existing dedicated test stack, set `HS_BASE_URL`, `HS_ADMIN_API_KEY`, and `HS_DATABASE_URL` as appropriate.

## Credentials

`CONNECTOR_AUTH_FILE_PATH` always points to a local, decrypted HS connector credentials JSON file, not UCS regression TOML. Local runs can export that variable or set it on the `npm test` command.

The loader accepts a direct entry such as `adyen.connector_account_details`, or a nested entry such as `adyen.connector_1.connector_account_details`. A single nested credential entry is selected automatically. If the connector has several entries, set `credentialEntry: "connector_1"` alongside `credentialKey: "adyen"` in its definition. Multiple entries are never selected by object ordering.

The credential entry's optional `metadata` is forwarded to the merchant connector account during setup. This is distinct from payment `metadata` or `connector_metadata` in a payment request. The account fields `additional_merchant_data` and `connector_webhook_details` are also forwarded when supplied in the external entry. Supply actual connector-required account data in that file; use request overrides only for justified payment-request differences. Absent or null optional account fields are omitted.

### CI preparation

CI should fetch and decrypt credentials before starting the test process. It then supplies the same local file path used by local runs. This keeps AWS access and decryption outside payment tests; a Jenkins pipeline can use this preparation without changing the framework.

For an encrypted JSON object in S3, provide an AWS identity with access to that object, `CONNECTOR_AUTH_S3_URI`, and a secret file path in `CONNECTOR_AUTH_PASSPHRASE_FILE`. The URI and secret file come from CI configuration, not this repository. With AWS CLI and GnuPG available:

```sh
set +x
set -euo pipefail
umask 077
credentials_dir="$(mktemp -d)"
trap 'rm -f "$credentials_dir/creds.json" "$credentials_dir/creds.json.gpg"; rmdir "$credentials_dir"' EXIT

aws s3 cp "$CONNECTOR_AUTH_S3_URI" \
  "$credentials_dir/creds.json.gpg" --only-show-errors
gpg --batch --quiet --pinentry-mode loopback \
  --passphrase-file "$CONNECTOR_AUTH_PASSPHRASE_FILE" \
  --output "$credentials_dir/creds.json" \
  --decrypt "$credentials_dir/creds.json.gpg"

CONNECTOR_AUTH_FILE_PATH="$credentials_dir/creds.json" npm test
```

If CI already provides a decrypted JSON secret file, set `CONNECTOR_AUTH_FILE_PATH` to that file directly. Do not include downloaded or decrypted files in test artifacts.

## Scenarios

Before payment calls, fixtures create an isolated merchant, issue an API key expiring in one day, use its default business profile (or create one if absent), create the sandbox connector account, and activate a single-connector routing rule. Setup validates the returned merchant association, profile, sandbox mode, and enabled payment methods.

Each scenario declares `paymentMethodsEnabled`, which is sent as the connector account's `payment_methods_enabled`. The current scenario enables card credit only. Declare additional methods in scenarios that exercise them; the common setup does not assume every test uses cards. Optional merchant contact fields, webhook subscriptions, parent merchants, tokenization, and mandate settings are not prerequisites for this fresh-card scenario and are not enabled implicitly. Customer creation belongs in a journey that actually uses a customer.

Each file contains one complete shared scenario. Read it top to bottom to see the requests, response checks, polling, and database evidence. Calls run in the order written.

```ts
const createdPayment = await payments.createPayment({
  profile_id: merchant.profileId,
  amount: 6000,
  currency: "USD",
  confirm: true,
  payment_method: "card",
  payment_method_data: { card: connector.card },
});

const payment = await payments.waitForPaymentStatus(
  createdPayment.payment_id,
  { force_sync: true },
  "succeeded",
);

await checks.fields("Payment result", payment, {
  status: "succeeded",
  amount: 6000,
  connector: connector.id,
});
```

Responses are ordinary objects. Any returned field can be passed to a later operation using normal TypeScript variables. Requests use HS API field names. Operations apply configured connector request differences before sending; they do not duplicate the full HS API schema or add payment defaults. The shared operations use the provisioned merchant's server API key.

`checks.fields(name, actual, expected)` compares fields and attaches the same expected/actual values to the report. `checks.oneOf(name, actual, allowed)` checks membership. `waitForPaymentStatus` polls the payment GET endpoint, for at most 90 seconds, and returns the response with the expected status. Mutating requests are never retried automatically.

Only `scenarios/api/payments/one-time/card/no3ds-automatic-create-confirm.spec.ts` is implemented. The four sibling files are empty placeholders. The current sequence is:

```text
POST /payments with confirm:true
  -> check confirmation
GET /payments/{id}?force_sync=true until succeeded
  -> check terminal payment fields
Read the payment and its active attempt from the test HS database
  -> check gateway, attempt status, and connector transaction ID presence
```

`payments.readRecordedPayment(payment.payment_id)` returns the recorded `gateway`, `attemptStatus`, and `connectorTransactionIdPresent`. The boolean is computed from `connector_transaction_id IS NOT NULL AND connector_transaction_id <> ''`; the identifier itself is not returned. Status is asserted in the spec rather than used to filter the query. The connection uses a read-only session; an existing stack should also supply SELECT-only credentials through `HS_DATABASE_URL` for least privilege. The local default remains the stack's database account.

The database read does not trigger or prove a fresh processor PSync call. It requires the corresponding HS database schema.

## Add coverage

Follow [CONTRIBUTING.md](CONTRIBUTING.md) for the reference pattern, file boundaries, and review checklist. The supported public fixture API is `test`, `connector`, `scenario`, `merchant` identifiers, `payments`, and `checks`.

Scenarios belong to the shared superset under `scenarios/`. Connectors automatically inherit all registered scenarios. Keep connector differences in `connectors/`; do not copy scenarios into connector directories.

- Add a connector definition and register it in `connectors/index.ts`.
- Supply sandbox card data and any necessary scenario request overrides.
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

`scenarios/api/` covers direct HS API calls. Future tests loading and operating the actual Web SDK belong under `scenarios/web-sdk/checkout/`. Browser authentication within an API flow does not itself constitute SDK coverage.

## File responsibilities

| File | Responsibility |
| --- | --- |
| `scenarios/api/**/*.spec.ts` | Own the complete journey, request data, use of responses, and expected results. |
| `connectors/adyen.ts` | Declare sandbox data, optional request overrides, and reasoned exclusions. |
| `connectors/index.ts` | Register connector definitions. |
| `framework/types.ts` | Define scenario metadata and data-only connector configuration types. |
| `framework/fixtures.ts` | Standard Playwright fixtures apply exclusions, provision context, and release resources. Export a typed public API without internal HTTP/keys/polling. No payment sequence. |
| `framework/payment-operations.ts` | Execute individual payment operations and return responses; expose explicitly named status polling and recorded-payment reads. |
| `framework/merchant-setup.ts` | Configure UCS and provision the merchant, API key, profile, connector account, and routing rule. |
| `framework/hyperswitch-client.ts` | Send authenticated HS HTTP requests and handle transport/HTTP errors. |
| `framework/payment-records.ts` | Read the payment gateway, active-attempt status, and connector transaction ID presence from the dedicated test database. |
| `framework/assertions.ts` | Shared comparisons, bounded polling, and expected/actual report attachments. |
| `framework/connector-credentials.ts` | Load external sandbox credentials. |
| `framework/configuration.ts` | Resolve runtime URLs and options. |
| `stack/compose.yml` | Configure local services and readiness dependencies. |
| `stack/ucs.Dockerfile` | Add the gRPC health-check executable to the published UCS image. |
| `stack/stack.mjs` | Prepare matching runtime assets and start or stop Compose. |
| `playwright.config.ts` | Configure discovery, execution, and reporters. |
| `package.json`, `package-lock.json`, `tsconfig.json` | Commands, dependencies, and compiler settings. |

Connector credentials are read only at runtime. Do not commit credentials, reports, runtime files, or copied production data. API tracing is disabled because it can capture credentials and payment-method data.
