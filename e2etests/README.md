# Hyperswitch → UCS end-to-end tests

Playwright API tests call the same Hyperswitch v1 payment APIs a customer integration uses. Hyperswitch runs from a configurable Docker image and sends connector calls to a configurable UCS gRPC endpoint. No direct UCS payment client is used. Browser/SDK scenarios can later use Playwright's `page` fixture alongside these helpers.

The first cut covers fresh-card, No3DS, one-time payments only. The supplied payment-flow sheet provides 48 merchant/connector profiles across Adyen, Stripe, PayPal, Authorize.net, Cybersource, and Checkout. Each profile runs automatic capture, automatic capture + full refund/RSync, authorization + full capture, and authorization + void, including forced payment sync. Lufthansa adds EUR and CHF request variants: 196 discovered scenarios altogether.

These are sandbox profiles labelled with the source merchant name. They do not recreate production accounts. All profiles inherit one connector baseline; seven profiles include request differences observed in a bounded US2 sample. The remaining profiles need more production-shape research before claiming exact merchant parity. The sheet's flow lists describe merchant-wide activity and do not prove that every flow occurred on an OTP transaction.

## Local run

Prerequisites: Node.js 22+, Docker Compose v2, a running UCS gRPC server accessible from Docker, and the same sandbox `creds.json` used by HS main. No browser download is needed for this API-only cut.

```sh
cd e2etests
npm ci

# Option A: existing decrypted HS-main credentials
export CONNECTOR_AUTH_FILE_PATH=/absolute/path/to/creds.json

# Option B: download/decrypt the same S3 object used by HS main
# Supply AWS credentials/region through the usual AWS environment/profile.
export CONNECTOR_CREDS_S3_BUCKET_URI='s3://YOUR_HS_MAIN_BUCKET'
# Set CONNECTOR_AUTH_PASSPHRASE through your secret manager/environment.
npm run credentials
export CONNECTOR_AUTH_FILE_PATH="$PWD/.secrets/creds.json"

# UCS must listen on an interface reachable from the container.
export UCS_BASE_URL=http://host.docker.internal:8000
npm run stack:up

# Start small, then run the complete matrix.
E2E_MERCHANT=gooutdoors npm test -- otp/adyen --grep 'automatic @'
npm test -- otp/adyen
npm test
npm run stack:down
```

`stack:up` clones HS main into ignored `.runtime/hyperswitch` once and reuses its Compose dependencies, config, superposition seed, and migrations. It starts only the router and its dependencies. The router image defaults to `docker.juspay.io/juspaydotin/hyperswitch-router:latest`; it is never compiled locally. Config forces these six connectors into `ucs_only_connectors`. Tests create `ucs_enabled=true` when absent and fail if an existing value disables UCS. Use a dedicated local/sandbox HS instance.

The helper records the source commit and requested image in `.runtime/versions.json`. Set `HS_REF` to a release/commit matching `HS_IMAGE` for reproducibility; `latest` and moving main may drift. A previously cloned checkout is deliberately retained; to change its revision, update that checkout or provide `HS_SOURCE_DIR`. Existing ports 8080, 5432, 6379, or 8081 may conflict with upstream Compose defaults.

Tests verify `payment_intent.feature_metadata.gateway_system` in the HS database equals `unified_connector_service`. A successful payment through the native HS connector fails this check. This read requires access to the HS PostgreSQL database. Reports contain explicit payment IDs and gateway evidence; raw API bodies, credentials, and API traces are excluded.

## Configuration

| Variable                   | Default / purpose                                                                       |
| -------------------------- | --------------------------------------------------------------------------------------- |
| `HS_IMAGE`                 | `docker.juspay.io/juspaydotin/hyperswitch-router:latest`                                |
| `HS_REF`                   | `main`; used when initially cloning config/migrations                                   |
| `HS_SOURCE_DIR`            | Optional existing HS checkout for config/migrations                                     |
| `UCS_BASE_URL`             | `http://host.docker.internal:8000`; Docker-to-UCS gRPC address                          |
| `HS_BASE_URL`              | `http://localhost:8080`; Playwright-to-HS API address                                   |
| `HS_ADMIN_API_KEY`         | `test_admin`, matching upstream local config                                            |
| `HS_DATABASE_URL`          | `postgresql://db_user:db_pass@localhost:5432/hyperswitch_db`                            |
| `CONNECTOR_AUTH_FILE_PATH` | Required decrypted HS main JSON; accepts `CYPRESS_CONNECTOR_AUTH_FILE_PATH` as fallback |
| `E2E_MERCHANT`             | Optional exact source merchant name                                                     |
| `E2E_OVERRIDES_FILE`       | Optional absolute path to connector/merchant overrides JSON                             |
| `S3_SOURCE_FILE_NAME`      | HS main's `b06ec672-ca83-405a-a834-856c3fe18dad.json.gpg`; change when rotated          |

For an existing HS deployment, configure its UCS endpoint and the six `ucs_only_connectors`, enable UCS, and set `HS_BASE_URL`, `HS_ADMIN_API_KEY`, and `HS_DATABASE_URL`. Skip `stack:up`. Database gateway evidence remains mandatory.

## Fixtures and overrides

```text
e2etests/
  otp/
    merchants.ts
    overrides.ts
    adyen/card-no3ds/payment.spec.ts
    stripe/card-no3ds/payment.spec.ts
    paypal/card-no3ds/payment.spec.ts
    authorizedotnet/card-no3ds/payment.spec.ts
    cybersource/card-no3ds/payment.spec.ts
    checkout/card-no3ds/payment.spec.ts
  support/                  # HS client, gateway evidence, shared OTP lifecycle
  scripts/                  # Docker setup and HS-main S3 credentials
  unit/                     # Offline harness checks
```

Add a merchant in `otp/merchants.ts`. Add a request variant below `otp/<connector>/<variant>/`; reuse `otpSuite` for the common lifecycle. CIT nonzero, CIT zero, and MIT (connector mandate, NTI, NTI + card) can later get sibling directories to `otp/`. Wallets, 3DS, browser SDK, and Jenkins/nightly wiring are deferred.

Overrides merge in this order: shared sandbox baseline → built-in merchant shape → external connector overrides → external merchant overrides. Nested objects merge, arrays replace, and `null` clears a field. `paymentVariants` adds named request patches, merged last. Identity, connector selection, sandbox mode, and capture-method selection are controlled by the harness. A guard rejects mandate/setup fields and 3DS/wallet overrides.

Copy `overrides.example.json` to an ignored `overrides.local.json`, replace sandbox placeholders, then set `E2E_OVERRIDES_FILE`. `credential` selects a named account such as `connector_1` within the HS credentials JSON. Without it, the loader uses the direct connector entry or first account with `connector_account_details`, matching HS Cypress. Connector metadata starts with the selected sandbox credential metadata, preserving merchant-account settings.

US2 research on 2026-10-16 used one-hour `Authorize` samples and projected field names/currencies, never production values into fixtures. Narvar GBP/order metadata, Zurich HKD/AUD/EUR/CHF and metadata keys, and a Convesio Stripe descriptor suffix informed `otp/overrides.ts`. This sample also showed Stripe uses tokenized cards inside HS; these tests submit fresh cards to HS and leave its tokenization/customer calls to the router.

## Validation and artifacts

```sh
npm run check
npm run test:unit
npm run list
npm run stack:prepare      # Generate Compose override/TOML without starting Docker
```

HTML reports go to `playwright-report/`; JUnit XML goes to `test-results/results.xml`. Missing credentials and unsupported/failed payment flows fail tests rather than silently skipping. Refund settlement rules can cause immediate refund scenarios to fail for some sandbox accounts; those failures need connector-specific investigation, not broad success-status allowances.

Each scenario creates an isolated merchant, profile, MCA, routing rule, and one-day API key. Accounts/payments are retained in the dedicated test database for debugging; `stack:down` preserves volumes. Test retries are disabled to avoid resubmitting payment operations automatically. There is no CI trigger in this PR; the runner can be invoked by a nightly Jenkins job later.
