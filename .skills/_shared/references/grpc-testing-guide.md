# gRPC Testing Guide

This guide covers how to test a connector implementation end-to-end against the running
gRPC server. Testing is mandatory -- a passing `cargo build` only proves syntax;
running the flows proves correctness.

This guide can be used as a **subagent prompt** for a dedicated testing agent.

**Do not paste payloads out of this or any other document without checking them.**
The proto tree moves. Derive the shape at runtime (Step 1.5) and treat the examples
below as a starting point, not as truth.

---

## gRPC Service Map

Each flow maps to a specific gRPC service and method. The gRPC package is `types`
(`crates/types-traits/grpc-api-types/proto/services.proto`), so every full path is
`types.<Service>/<Method>`.

| Flow | Service | Method | Full path |
|------|---------|--------|-----------|
| Authorize | PaymentService | Authorize | `types.PaymentService/Authorize` |
| PSync | PaymentService | Get | `types.PaymentService/Get` |
| Capture | PaymentService | Capture | `types.PaymentService/Capture` |
| Void | PaymentService | Void | `types.PaymentService/Void` |
| Refund | PaymentService | Refund | `types.PaymentService/Refund` |
| RSync | RefundService | Get | `types.RefundService/Get` |
| SetupMandate | PaymentService | SetupRecurring | `types.PaymentService/SetupRecurring` |
| RepeatPayment | RecurringPaymentService | Charge | `types.RecurringPaymentService/Charge` |
| ServerAuthenticationToken | MerchantAuthenticationService | CreateServerAuthenticationToken | `types.MerchantAuthenticationService/CreateServerAuthenticationToken` |
| ServerSessionAuthenticationToken | MerchantAuthenticationService | CreateServerSessionAuthenticationToken | `types.MerchantAuthenticationService/CreateServerSessionAuthenticationToken` |
| ClientAuthenticationToken | MerchantAuthenticationService | CreateClientAuthenticationToken | `types.MerchantAuthenticationService/CreateClientAuthenticationToken` |
| CreateOrder | PaymentService | CreateOrder | `types.PaymentService/CreateOrder` |
| CreateConnectorCustomer | CustomerService | Create | `types.CustomerService/Create` |
| PaymentMethodToken | PaymentMethodService | Tokenize | `types.PaymentMethodService/Tokenize` |
| IncomingWebhook | EventService | HandleEvent | `types.EventService/HandleEvent` |
| Accept (`FlowName::AcceptDispute`) | DisputeService | Accept | `types.DisputeService/Accept` |
| SubmitEvidence | DisputeService | SubmitEvidence | `types.DisputeService/SubmitEvidence` |
| DefendDispute | DisputeService | Defend | `types.DisputeService/Defend` |
| DSync (`FlowName::Dsync`) | DisputeService | Get | `types.DisputeService/Get` |

The "Flow" column is the flow marker struct in
`crates/types-traits/domain_types/src/connector_flow.rs`. There is **no**
`CreateAccessToken`, `CreateSessionToken`, `PaymentAccessToken` or
`PaymentSessionToken` anywhere in the codebase -- the token flows are the three
`*AuthenticationToken` rows above. `IncomingWebhook` and `Dsync` exist as `FlowName`
variants only (no `pub struct` marker) — and `Dsync` is spelled with one capital,
not `DSync`.

Never trust this table blindly. The live list is one command away:

```bash
grpcurl -plaintext 127.0.0.1:$PORT list                       # all services
grpcurl -plaintext 127.0.0.1:$PORT list types.PaymentService  # methods on one
```

---

## Step 0: Prefer the first-class test harness

A real integration-test harness exists (`crates/internal/integration-tests`, bin
`test_ucs`). It owns the server lifecycle, credential loading and assertions.
Hand-rolled grpcurl is the **fallback for debugging a single request**, not the
default way to test a connector.

```bash
make test-connector connector={connector_name}                  # all suites
make test-scenario connector={connector_name} suite=authorize \
     scenario=no3ds_auto_capture_credit_card                    # one scenario
./scripts/run-tests --connector {connector_name} --suite authorize
```

`./scripts/run-tests` is installed on PATH as `test-prism` by
`scripts/setup-connector-tests.sh`; run `test-prism --help` for the full flag list
(`--interface grpc|sdk`, `--endpoint`, `--report`, `--no-server`, `--interactive`).

**Running off the default port:** `test-connector` and `test-scenario` both start the
server on `GRPC_PORT` and pass `--endpoint localhost:$(GRPC_PORT)` to `test_ucs`, so
one override moves both: `make test-connector connector={connector_name} GRPC_PORT=$PORT`.
What they do *not* move is the metrics port — neither recipe exports
`CS__METRICS__PORT`, so a second concurrent run still tries to bind the 8080 default
and dies on the failed bind (see Step 1). Export `CS__METRICS__PORT` into the
environment `make` inherits, or use `./scripts/run-tests --endpoint localhost:$PORT`
(`$PORT` is exported in Step 1).

---

## Step 1: Start the gRPC Server

```bash
# Never assume 8000 is free. The process binds TWO sockets: the gRPC port and a
# Prometheus metrics port (config/development.toml -> [metrics] port = 8080).
# GRPC_PORT only moves the gRPC socket (the recipe sets CS__SERVER__PORT); the
# metrics socket keeps the config value unless you export CS__METRICS__PORT into
# the environment `make` inherits. A second server on the same box therefore still
# tries to bind 8080 -- and because main.rs `try_join!`s the metrics server with
# the gRPC server, that failed bind kills the whole process, not just /metrics.
# Derive both from one per-run slot so concurrent servers can never collide:
export PORT=8000                          # gRPC   -- pick a free slot: 8000, 8001, ...
export CS__METRICS__PORT=$((PORT + 1000)) # metrics -- 9000, 9001, ...; never the 8080 default

make start-grpc GRPC_PORT=$PORT
```

`make start-grpc` builds `cargo build -p grpc-server --profile release-fast --target
$(PLATFORM)` (first run is slow), writes the PID to `.grpc-server.pid`, and already
waits for readiness with a TCP probe (40 x 0.5 s = 20 s budget).

**There is no `/health` HTTP endpoint on the gRPC port.** `config/development.toml`
sets `[server] type = "grpc"` (also the `#[default]` of `ServiceType` in
`crates/common/ucs_env/src/configs.rs`); the axum `/health` route
(`crates/grpc-server/grpc-server/src/http/router.rs`) is only mounted when
`type = "http"`. One socket, one transport. `curl http://localhost:$PORT/health`
can never succeed. Use the gRPC health service:

```bash
# the nc loop is only needed when you launched the binary yourself;
# make start-grpc has already run an identical probe before returning.
for i in $(seq 1 40); do nc -z 127.0.0.1 $PORT 2>/dev/null && break; sleep 0.5; done
grpcurl -plaintext 127.0.0.1:$PORT grpc.health.v1.Health/Check   # {"status":"SERVING"}
grpcurl -plaintext 127.0.0.1:$PORT list                          # services are up
```

Running the binary directly (useful when you already have a debug build):

```bash
CS__SERVER__HOST=127.0.0.1 CS__SERVER__PORT=$PORT \
CS__METRICS__PORT=$((PORT + 1000)) CS__COMMON__ENVIRONMENT=development \
  ./target/debug/grpc-server > /tmp/grpc-server.log 2>&1 &
```

Stop with `make stop-grpc`. Note that `stop-grpc` sends a plain `kill` (SIGTERM).
The gRPC server handles SIGTERM, but the metrics server's graceful-shutdown hook
waits on `tokio::signal::ctrl_c()` -- SIGINT only -- so SIGTERM can release the gRPC
port and leave the process alive still holding the metrics port. If the metrics port
stays bound, `kill -9` the PID (in `.grpc-server.pid`).

If the service fails to start, check build errors and fix before proceeding.

---

## Step 1.5: Load gRPC Request Payloads from Field Probe (PREFERRED)

**Before manually constructing grpcurl requests, check `data/field_probe/{connector_name}.json`.** This file is the authoritative source for correctly-structured gRPC request payloads. Regenerate it with `make field-probe`.

### Structure

Each file contains `{ "connector": "...", "flows": { ... } }` where each flow (e.g., `authorize`, `capture`, `refund`) maps to scenarios (e.g., `Card`, `Ach`, `Sepa`, `GooglePay`). Each scenario has:

- `status`: `"supported"`, `"not_implemented"`, `"not_supported"` or `"error"`
- `proto_request`: The exact JSON payload to use as the `-d` argument in grpcurl.
  **Only `"supported"` scenarios carry it** — for every other status this key is
  absent and `jq '...proto_request'` returns `null`.
- `sample`: The downstream HTTP request the connector sends (useful for debugging).
  Appears on `supported` scenarios only, almost always next to `proto_request` —
  a handful carry `sample` with no `proto_request`, so key off `proto_request`.
- `error`: Why the probe could not build a request. Appears on `not_implemented`,
  `not_supported` and `error` scenarios instead of `proto_request`/`sample` -- but
  it is not guaranteed: some `not_implemented` scenarios carry only `status`.

### Usage

```bash
# List available flows for a connector:
cat data/field_probe/{connector_name}.json | jq '.flows | keys'

# List payment method scenarios for Authorize:
cat data/field_probe/{connector_name}.json | jq '.flows.authorize | keys'

# Get the proto_request for a Card Authorize:
cat data/field_probe/{connector_name}.json | jq '.flows.authorize.Card.proto_request'

# Check which scenarios are supported vs not_implemented:
cat data/field_probe/{connector_name}.json | jq '.flows.authorize | to_entries[] | {scenario: .key, status: .value.status}'
```

### If there is no field probe file yet: describe the message

Server reflection is registered, so with the server running no `-proto` flags are
needed:

```bash
grpcurl -plaintext -msg-template 127.0.0.1:$PORT \
        describe types.PaymentServiceAuthorizeRequest
```

With the server down, the same works offline from the proto tree (`services.proto`
transitively imports `payment.proto` and `payment_methods.proto`):

```bash
grpcurl -import-path crates/types-traits/grpc-api-types/proto -proto services.proto \
        -msg-template describe types.PaymentServiceAuthorizeRequest
```

Shape rules that hold today (re-check them with `describe` before trusting them):

- **There is no `request_ref_id` field.** Any payload containing it is rejected
  client-side with `message type types.PaymentServiceAuthorizeRequest has no known
  field named request_ref_id`. The caller-side identifier is a plain string, named
  per flow: `merchant_transaction_id` (Authorize, PSync), `merchant_capture_id`
  (Capture), `merchant_refund_id` (Refund, RSync), `merchant_void_id` (Void).
- **`amount` is a `Money` message, not a scalar**: `{"minor_amount": 1000,
  "currency": "USD"}`. There is no top-level `minor_amount`/`currency` pair.
  Capture uses `amount_to_capture` (Money); Refund uses `payment_amount` (bare
  `int64`) plus `refund_amount` (Money).
- **Card fields are wrapper messages**: `card_number`, `card_exp_month`,
  `card_exp_year`, `card_cvc`, `card_holder_name` are each `{"value": "..."}`.
  So are the `Address` string fields (`first_name`, `line1`, ...), but
  `country_alpha2_code` is a bare enum (`"US"`).
- **`address` is required on Authorize** -- omitting it fails with
  `Missing required field: address` before the connector is ever contacted. An
  empty `{}` is accepted.
- There is **no top-level `email`** on Authorize; it lives at `customer.email`.
- grpcurl accepts both `snake_case` and `lowerCamelCase` JSON keys.

### Using proto_request in grpcurl

The `proto_request` value is a JSON object ready to use directly:

```bash
PROTO_REQ=$(cat data/field_probe/{connector_name}.json | jq -c '.flows.authorize.Card.proto_request')

grpcurl -plaintext \
  -H "x-connector-config: $CFG" \
  -d "$PROTO_REQ" \
  127.0.0.1:$PORT \
  types.PaymentService/Authorize
```

(see Step 2 for how `$CFG` is built)

**Always prefer field_probe data over manually constructing requests** — it ensures the request structure matches what the connector actually supports. Fall back to manual construction only for new connectors that don't have a field_probe file yet.

---

## Step 2: Load Credentials

Credentials resolve in this order (`crates/internal/integration-tests/src/harness/credentials.rs`):

1. `$CONNECTOR_AUTH_FILE_PATH`
2. `$UCS_CREDS_PATH`
3. `creds.json` at the repo root (gitignored)

```bash
cat creds.json | jq '.{connector_name}'
```

### Path A (preferred): one `x-connector-config` header

A single header carries both connector identity and auth, so no `x-connector` /
`x-auth` pair is needed. Its value is
`{"config":{"<PascalConnector>":{ ...auth fields... }}}`, where `<PascalConnector>`
is the connector name with only its first letter upper-cased
(`pascal_connector_name` in `crates/internal/connector-creds/src/lib.rs`), e.g.
`stripe` -> `Stripe`, `paynearme` -> `Paynearme`.

```bash
grpcurl -plaintext -H "x-connector-config: $CFG" ...
```

`crates/internal/connector-creds` is what builds `$CFG` for the test harness
(`build_wrapped_config`): it takes the connector's `creds.json` entry (first
element if the entry is an array), drops `metadata`, unwraps any `{"value": "..."}`
into the bare string, and wraps the rest under the PascalCase key. **Read the entry
before hand-building the header** — a `creds.json` entry whose top level is
`connector_account_details` is rejected outright with
`CredentialError::LegacyFormat`, and an entry nested under `connector_1` /
`connector_2` will produce a config the server does not understand. Only a flat
`{"api_key": "...", "key1": "..."}` entry maps straight through.

### Path B (legacy): discrete headers

`x-connector` is required, then `x-auth`, then the keys that auth type names.
Omitting them fails with `Missing required field: x-connector` / `x-auth` /
`x-api-key`.

| creds.json field | gRPC header |
|-----------------|-------------|
| `api_key` | `-H 'x-api-key: <value>'` |
| `key1` | `-H 'x-key1: <value>'` |
| `key2` | `-H 'x-key2: <value>'` |
| `api_secret` | `-H 'x-api-secret: <value>'` |
| `merchant_id` | `-H 'x-merchant-id: <value>'` |

Always include: `-H 'x-connector: {connector_name}'` **and** `-H 'x-auth: <type>'`.
Valid `x-auth` values (`crates/types-traits/ucs_interface_common/src/auth.rs`) are
kebab-case: `header-key`, `body-key`, `signature-key`, `multi-auth-key`, `no-key`,
`temporary-auth`, `currency-auth-key`. Anything else, including `certificate-auth`,
is rejected.

`x-merchant-id`, `x-tenant-id` and `x-request-id` are **optional** (they default to
`DefaultMerchantId`, `public`, and a generated uuid v7) — send them anyway so the
server logs stay greppable.

Only include headers that exist in creds.json. Do not guess or add unused headers.

---

## Step 3: Test Authorize

The examples below use `$HDRS` for whichever header set you chose in Step 2, e.g.
`HDRS=(-H "x-connector-config: $CFG")` or
`HDRS=(-H 'x-connector: stripe' -H 'x-auth: header-key' -H "x-api-key: $API_KEY")`.

```bash
grpcurl -plaintext "${HDRS[@]}" \
  -d '{
    "merchant_transaction_id": "test_{connector}_auth_001",
    "amount": {"minor_amount": 1000, "currency": "USD"},
    "payment_method": {
      "card": {
        "card_number": {"value": "4111111111111111"},
        "card_exp_month": {"value": "12"},
        "card_exp_year": {"value": "2030"},
        "card_cvc": {"value": "123"},
        "card_holder_name": {"value": "John Doe"}
      }
    },
    "customer": {"email": {"value": "test@example.com"}},
    "address": {
      "billing_address": {
        "first_name": {"value": "John"},
        "last_name": {"value": "Doe"},
        "line1": {"value": "123 Test St"},
        "city": {"value": "Test City"},
        "state": {"value": "CA"},
        "zip_code": {"value": "12345"},
        "country_alpha2_code": "US"
      }
    },
    "capture_method": "AUTOMATIC",
    "auth_type": "NO_THREE_DS",
    "enrolled_for_3ds": false,
    "return_url": "https://example.com/return",
    "webhook_url": "https://example.com/webhook"
  }' \
  127.0.0.1:$PORT \
  types.PaymentService/Authorize
```

**Adapt per connector:**
- Replace card data with connector's sandbox test card numbers
- Change currency/country to match connector's supported regions
- Add `metadata` if connector requires it (check tech spec)
- For non-card payment methods, replace the `payment_method` object accordingly
  (`payment_method` is a oneof — run `describe types.PaymentMethod` for the variants)

---

## Step 4: Test PSync (Payment Status)

Use the `connector_transaction_id` from the Authorize response:

```bash
grpcurl -plaintext "${HDRS[@]}" \
  -d '{
    "merchant_transaction_id": "test_{connector}_psync_001",
    "connector_transaction_id": "<id_from_authorize_response>"
  }' \
  127.0.0.1:$PORT \
  types.PaymentService/Get
```

---

## Step 5: Test Capture

```bash
grpcurl -plaintext "${HDRS[@]}" \
  -d '{
    "merchant_capture_id": "test_{connector}_capture_001",
    "connector_transaction_id": "<id_from_authorize_response>",
    "amount_to_capture": {"minor_amount": 1000, "currency": "USD"}
  }' \
  127.0.0.1:$PORT \
  types.PaymentService/Capture
```

---

## Step 6: Test Refund

```bash
grpcurl -plaintext "${HDRS[@]}" \
  -d '{
    "merchant_refund_id": "test_{connector}_refund_001",
    "connector_transaction_id": "<id_from_authorize_response>",
    "payment_amount": 1000,
    "refund_amount": {"minor_amount": 1000, "currency": "USD"},
    "reason": "requested_by_customer"
  }' \
  127.0.0.1:$PORT \
  types.PaymentService/Refund
```

---

## Step 7: Test RSync (Refund Status)

```bash
grpcurl -plaintext "${HDRS[@]}" \
  -d '{
    "merchant_refund_id": "test_{connector}_rsync_001",
    "connector_transaction_id": "<id_from_authorize_response>",
    "refund_id": "test_{connector}_refund_001",
    "connector_refund_id": "<connector_refund_id_from_refund_response>"
  }' \
  127.0.0.1:$PORT \
  types.RefundService/Get
```

---

## Step 8: Test Void

```bash
grpcurl -plaintext "${HDRS[@]}" \
  -d '{
    "merchant_void_id": "test_{connector}_void_001",
    "connector_transaction_id": "<id_from_authorize_response>",
    "cancellation_reason": "requested_by_customer"
  }' \
  127.0.0.1:$PORT \
  types.PaymentService/Void
```

Note: Void requires an authorized-but-not-captured payment. Run Authorize with
`"capture_method": "MANUAL"` first, then Void that transaction.

---

## Validating Test Results

### Reading grpcurl errors

grpcurl reports every UCS status error as `Code: Internal` with a
`grpc-status-details-bin mismatch` preamble. **That is a grpcurl artifact, not the
server's status.** The real status and message are inside that same line:

```
Code: Internal
Message: grpc-status-details-bin mismatch: grpc-status=InvalidArgument,
         grpc-message="Missing required field: address", ...
```

`-format-error` does not fix it. Read `grpc-status=` / `grpc-message=`, and always
read the grpc-server log as well.

A client-side error looks different and never reaches the server, e.g.
`Error invoking method ...: error getting request data: message type ... has no
known field named <x>` — that means your JSON is wrong, not the connector.

### PASS criteria (ALL must be true):
- No `Error invoking method` or `Failed to` in output
- Response contains valid JSON with a `status` field
- `status_code` is 2xx (200-299)
- `status` is a healthy `types.PaymentStatus` value for the flow under test:
  `AUTHORIZED`, `CHARGED`, `PARTIAL_CHARGED`, `PENDING`, `AUTHENTICATION_PENDING`,
  `CAPTURE_INITIATED`, `VOIDED` (for Refund/RSync the enum is `types.RefundStatus`:
  `REFUND_SUCCESS`, `REFUND_PENDING`). Run `describe types.PaymentStatus` for the
  full list — enum names are UPPER_SNAKE_CASE in both request and response JSON.
- No `error` object (or `error` is null/empty)

### FAIL indicators (ANY means test failed):
- `Error invoking method` -- grpcurl itself failed (wrong field, wrong method, connection refused)
- `status_code` not 2xx -- connector rejected the request
- `grpc-status=` anything other than `OK` (remember: the printed `Code: Internal` is not the real status)
- `"status": "FAILURE"`, `AUTHORIZATION_FAILED`, `AUTHENTICATION_FAILED`,
  `CAPTURE_FAILED`, `VOID_FAILED`, `ROUTER_DECLINED`, `UNRESOLVED`,
  `REFUND_FAILURE`, `REFUND_TRANSACTION_FAILURE`
- Non-null `error` object with a message
- No JSON response (empty output, timeout, crash)

### The in-band 2xx trap

The two lists above are not independent checks -- their *combination* is what catches the most
damaging connector bug. A 2xx `status_code` with `"status": "CHARGED"` is only a PASS if the
connector's raw body actually said the payment succeeded.

Many gateways return **HTTP 200 with a declined body**. If the connector's response `TryFrom`
returns `Ok(..)` unconditionally instead of branching on a failure predicate, UCS reports that
decline as `AUTHORIZED` or `CHARGED` and the test above passes while the money never moved.

So when a sandbox decline card or a deliberately-invalid amount is used:

- [ ] The response `status` is a failure value (`FAILURE`, `AUTHORIZATION_FAILED`,
      `CAPTURE_FAILED`, ...) **and** the `error` object is populated -- not a healthy status
- [ ] `error.code` / `error.message` carry the connector's own values, not `"No error code"` /
      `"No error message"` (those constants mean the connector *had* an error body and the
      transformer did not read it) and not empty strings (which means someone wrote
      `.unwrap_or_default()`)
- [ ] The failure status is the **flow-specific** one: a failed Capture reports `CAPTURE_FAILED`,
      a failed Void reports `VOID_FAILED`, a declined authorization reports
      `AUTHORIZATION_FAILED` -- not the generic `FAILURE`
- [ ] A refund flow reports a `types.RefundStatus` (`REFUND_FAILURE`,
      `REFUND_TRANSACTION_FAILURE`), never a `PaymentStatus`

Always diff the connector's raw body (visible in the grpc-server log) against the status UCS
reported. A decline test that "passes" with a healthy status is a failing test.

---

## Build-Test Loop (Anti-Loop Safeguards)

When a test fails, you MUST fix the code and rebuild before retrying:

```
1. Build: cargo build --package connector-integration
2. If build fails -> read error -> fix code -> go to 1
3. Start service (if not running) -> load creds -> run the test
4. If test fails -> read SERVER LOGS -> identify root cause -> fix code -> go to 1
5. If credential error -> ask user for correct creds -> go to 3
6. Both pass -> SUCCESS
```

**Hard rules:**
- NEVER rerun the same request without changing code first. Same code = same result.
- 3-strike rule: same error 3 times = FAILED immediately
- Maximum 7 total loop iterations = FAILED regardless
- Always read server logs (not just grpcurl output) to diagnose errors
- Maintain a fix log: (1) error seen, (2) file changed, (3) what and why

### Error Classification

| Type | Signs | Action |
|------|-------|--------|
| gRPC config | Connection refused, wrong method | Fix the command, check `grpcurl list` |
| Request JSON | `has no known field named ...` | Client-side; re-derive shape with `describe` |
| Metadata | `Missing required field: x-connector` / `x-auth` | Add the Step 2 headers |
| Credentials | 401/403, `Unauthenticated`, "unauthorized" | Ask user for correct creds |
| Request format | 400/422, `Missing required field: <proto field>` | Check server logs, fix request struct in transformers.rs, rebuild |
| Response parsing | Deserialization error, panic | Check server logs, fix response struct, rebuild |
| Server error | 500/502/503, `grpc-status=Internal` / `Unknown` / `Unavailable` | Check server logs for root cause, fix connector code, rebuild |

---

## Subagent Prompt Template

Use this to delegate testing to a separate subagent after implementation:

```
Test the {ConnectorName} connector's {FlowName} flow.

## Context
- Connector: {connector_name}
- Credentials: creds.json (field: {connector_name}); override with
  CONNECTOR_AUTH_FILE_PATH or UCS_CREDS_PATH
- Field probe data: data/field_probe/{connector_name}.json (use proto_request for gRPC payloads)
- Tech spec: grace/rulesbook/codegen/references/{connector_name}/technical_specification.md
- Testing guide: .skills/_shared/references/grpc-testing-guide.md
- Connector source: crates/integrations/connector-integration/src/connectors/{connector_name}.rs
- Transformers: crates/integrations/connector-integration/src/connectors/{connector_name}/transformers.rs
- Proto tree: crates/types-traits/grpc-api-types/proto/

## Instructions
1. Read the testing guide at .skills/_shared/references/grpc-testing-guide.md
2. Try the first-class harness first: make test-connector connector={connector_name}
   (or ./scripts/run-tests --connector {connector_name} --endpoint localhost:$PORT).
   Only drop to raw grpcurl to debug a single failing request.
3. Start the gRPC server if not running (make start-grpc GRPC_PORT=$PORT); wait on
   the TCP probe + grpc.health.v1.Health/Check — there is no /health HTTP endpoint
4. Load credentials from creds.json and build the x-connector-config header
   (or the legacy x-connector + x-auth + key headers)
5. Load gRPC request payloads from data/field_probe/{connector_name}.json; if absent,
   derive the shape with `grpcurl -msg-template describe <RequestType>` — never paste
   a payload literal out of a document
6. Run the test for {FlowName} using the correct service/method from the guide
7. Validate the response against PASS/FAIL criteria; remember grpcurl misreports the
   status code — read grpc-status= / grpc-message= out of the message text
8. If FAILED: read server logs, diagnose root cause, fix code, rebuild, retest
9. Follow anti-loop safeguards (3-strike rule, max 7 iterations, always change code between retries)
10. Report: PASS or FAIL with details, command output, and fix log if applicable
```
