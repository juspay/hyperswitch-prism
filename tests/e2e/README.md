# End-to-end tests

These tests exercise payment flows through Hyperswitch and UCS against processor sandbox environments.

## Run locally

Requirements: Node.js 22+, Docker, and a Hyperswitch-compatible connector credentials JSON file.

```sh
cd tests/e2e
npm ci
CONNECTOR_AUTH_FILE_PATH=/path/to/connector-auth.json npm run stack:up
CONNECTOR_AUTH_FILE_PATH=/path/to/connector-auth.json npm test
```

Open the HTML report with `npx playwright show-report`. Stop the isolated stack with `npm run stack:down`.

`HS_IMAGE`, `UCS_IMAGE`, `HS_BASE_URL`, and `HS_SOURCE_DIR` are optional. The defaults run the latest published Hyperswitch and UCS images locally. The source checkout supplies configuration and database migrations matching the Hyperswitch image and is cached after the first run.

HS configuration uses its supported `ROUTER__...` environment overrides. The upstream TOML remains unchanged. `stack/ucs.Dockerfile` adds the standard gRPC health probe to the published UCS image; Compose waits for UCS to report healthy before starting HS. PostgreSQL uses the loopback port `HS_DB_PORT` (default `15432`); Redis is available only inside the Compose network. `HS_PORT` sets the local HS port (default `8080`). `UCS_CONNECTORS` controls the local HS connector list (default `adyen`).

## Extend coverage

Each scenario has its own file. `create-confirm` means a single create request with `confirm: true`; `create-then-confirm` means separate create and confirm requests. `automatic` and `manual` describe capture mode. Retrieval and verification steps are declared within each scenario.

Only `scenarios/api/payments/one-time/card/no3ds-automatic-create-confirm.spec.ts` is implemented. Its four sibling files are empty placeholders for future scenarios.

- `scenarios/api/` declares journeys driven through direct HS API calls, using server or client credentials as appropriate. Scenario files contain typed data and register tests through the shared runner. Future tests operating an actual Web SDK checkout belong under `scenarios/web-sdk/checkout/`.
- `connectors/` declares supported capabilities and connector-required test data. It does not implement payment lifecycles.
- `framework/` supplies reusable setup, step execution, API access, and checks.
- `stack/` owns the isolated local services.

All compatible registered connectors automatically receive a shared scenario. Add connector-specific data only when the processor requires it; do not copy the scenario or runner.

### File responsibilities

| File | Responsibility |
| --- | --- |
| `scenarios/api/**/*.spec.ts` | Declare the complete ordered journey: each operation, request fields, expected response fields, polling target, and routing checks. Register it for compatible connectors. |
| `connectors/adyen.ts` | Declare connector capabilities and sandbox card data. |
| `connectors/index.ts` | Register connector definitions. |
| `framework/scenario.ts` | Define data types for scenarios, supported operations, requests, expectations, and connector compatibility. |
| `framework/runner.ts` | Set up the test, execute exactly the declared steps in order, resolve saved payment IDs and connector data, and release the HTTP client. |
| `framework/merchant-setup.ts` | Configure UCS and provision the merchant, API key, profile, connector account, and routing rule. |
| `framework/hyperswitch.ts` | Send authenticated HS HTTP requests and handle transport/HTTP errors. |
| `framework/payment-records.ts` | Read payment routing and processor-reference facts from the test database. |
| `framework/checks.ts` | Compare declared expectations with observed results, perform bounded status polling, and produce expected/actual report attachments. |
| `framework/credentials.ts` | Load external connector credentials. |
| `framework/configuration.ts` | Resolve runtime URLs and options from environment variables. |
| `stack/compose.yml` | Define local service configuration and startup dependencies. |
| `stack/ucs.Dockerfile` | Add the gRPC health-check executable to the published UCS image. |
| `stack/stack.mjs` | Prepare matching runtime assets and start or stop the Compose project. |
| `playwright.config.ts` | Configure test discovery, execution, and reporters. |
| `package.json`, `package-lock.json`, `tsconfig.json` | Define commands, dependencies, and compiler settings. |

The runner does not select a payment lifecycle or insert additional payment calls. `steps` are executed in declaration order. `saveAs: "payment"` saves the created payment identifier; `payment: "payment"` references it in subsequent steps. `$connector.card` explicitly selects the registered connector's sandbox card, while the framework supplies the provisioned profile and server API key.

Within a step, `name` is the report label, `action` selects the reusable operation, `request` contains HS API request fields, and `expect` contains response comparisons. `saveAs` and `payment` link operations by payment identifier. The request type documents core card fields and permits additional JSON fields, which are forwarded unchanged. HS remains responsible for API validation; the framework does not duplicate the full HS request schema. Framework substitutions such as `$connector.card` and `{ from: "connector.id" }` are resolved before the HTTP call or comparison.

Each step's `expect` entries drive both assertions and report attachments. `waitForStatus` declares a retrieval polling target; only retrieval is repeated. Mutating requests are never retried automatically. The currently implemented operations are `createPayment`, `retrievePayment`, and `verifyPaymentRoute`; additional operations belong in the shared framework when their first scenario is added.

Connector compatibility is derived from the payment request in the scenario. Routing verification reads the dedicated test HS database through `HS_DATABASE_URL` or the local default. This explicitly checks HS storage fields for the recorded gateway and processor reference; it does not prove that a retrieval caused a fresh processor sync. Run against an HS version with the corresponding database schema.

The current scenario is a server-side, one-time No3DS card payment with automatic capture:

```text
create payment with confirm:true -> retrieve payment -> verify success and UCS routing
```

Connector credentials are read only at runtime. Do not commit credentials, reports, runtime files, or copied production data.
