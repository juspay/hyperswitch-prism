# Integration Tests

Scenario-driven integration testing for payment connectors. Each connector is exercised against the real sandbox (or SDK) via a shared scenario library, with per-connector overrides for test data and assertions.

---

## Quick start

### 1. Setup (once)

```bash
make setup-connector-tests
```

Installs browser automation deps, `grpcurl`, Netlify CLI (Google Pay), and the `test-prism` runner CLI.

### 2. Run

```bash
# Interactive picker
test-prism --interactive

# One connector, all suites
test-prism --connector stripe

# One suite (suite name uses `Service/Method` — see Suite names below)
test-prism --connector stripe --suite PaymentService/Authorize

# One scenario in one suite
test-prism --connector stripe --suite PaymentService/Authorize \
  --scenario no3ds_auto_capture_credit_card

# All configured connectors
test-prism --all-connectors

# SDK backend instead of gRPC
test-prism --interface sdk --connector stripe

# Write JSON + markdown reports
test-prism --connector stripe --report
```

### 3. Read the output

After a run the harness writes:

- `crates/internal/integration-tests/report.json` — accumulating JSON log. Each test run **appends** entries; inspect the latest block for the scenario you just ran.
- `crates/internal/integration-tests/test_report/connectors/<connector>/<suite>.md` — human-readable markdown per suite with exact request/response pairs.

The hosted dashboard:

- Web UI: https://hyperswitch-prism-testing.netlify.app/
- Latest JSON: https://integ.hyperswitch.io/connector-service/reports/grpc/report_latest.json

---

## Suite names

Suite names use `Service/Method` form (forward slash). The harness rejects underscore form.

| Right | Wrong |
|---|---|
| `PaymentService/Authorize` | `PaymentService_Authorize` |
| `PaymentMethodAuthenticationService/PreAuthenticate` | `PaymentMethodAuthenticationService_PreAuthenticate` |
| `RefundService/Get` | `RefundService_Get` |

The directory names on disk use underscores (`global_suites/PaymentService_Authorize/`), but every CLI flag, override-json key, and spec-json key uses forward slashes.

---

## File layout

```
crates/internal/integration-tests/
├── src/
│   ├── global_suites/                  # Shared scenarios across all connectors
│   │   ├── PaymentService_Authorize/
│   │   │   ├── scenario.json           # Scenario definitions (grpc_req + asserts)
│   │   │   └── suite_spec.json         # Dependency graph, scope, alias info
│   │   ├── PaymentService_Capture/
│   │   └── …
│   │
│   ├── connector_specs/                # Per-connector configuration
│   │   ├── stripe/
│   │   │   ├── specs.json              # supported_suites + spec fields
│   │   │   ├── override.json           # Scenario-level grpc_req / assert overrides
│   │   │   ├── connector_specific_scenarios.json  # Optional: scenarios only this connector has
│   │   │   ├── webhook_payload.json    # Optional: HandleEvent suite test payloads
│   │   │   └── browser_automation_spec.json   # Optional: browser-driven 3DS / redirect hooks
│   │   └── …
│   │
│   ├── harness/                        # Test orchestration engine
│   └── bin/                            # CLI binaries (`test_ucs`, `run_test`, etc.)
│
├── docs/                               # Internal docs (overrides, context-map, code walk)
├── test_report/                        # Generated markdown reports
└── report.json                         # Generated JSON report (accumulating)
```

---

## Configuration files

### `creds.json` (repo root)

The harness expects credentials in **flat proto-native form** — keys map directly to the connector's `*Config` proto message (see `crates/types-traits/grpc-api-types/proto/payment.proto`). The legacy nested `connector_account_details` wrapper is **rejected** with `CredentialError::LegacyFormat`.

```jsonc
{
  "stripe": {
    "api_key": "sk_test_..."
  },
  "redsys": {
    "merchant_id": { "value": "999008881" },
    "terminal_id": "001",
    "sha256_pwd": { "value": "..." }
  },
  "adyen": {
    "api_key": "...",
    "merchant_account": "..."
  }
}
```

`{ "value": "..." }` wrappers are unwrapped automatically by the loader (`normalize_value`); plain strings work too.

A template lives at `.github/test/template_creds.json`.

The harness reads from the path resolved as: explicit `CONNECTOR_AUTH_FILE_PATH` → `UCS_CREDS_PATH` env var → `creds.json` at the workspace root. `.env.connector-tests` (sourced by `scripts/run-tests`) can also set `UCS_CREDS_PATH` — if your edits don't seem to land, **check that file first** to see which `creds.json` the harness actually loads.

### `connector_specs/<connector>/connector_specific_scenarios.json`

Optional. Scenarios that exist **only** for this connector, run in addition to the
global suites its `specs.json` declares. Same `suite -> scenario` shape as
`override.json`, but the values are whole scenario definitions.

```jsonc
{
  "PaymentService/Authorize": {
    "tsys_soft_decline_retry": {
      "grpc_req": { "amount": { "minor_amount": 5205, "currency": "USD" } },
      "assert": { "status": { "one_of": ["FAILURE"] } }
    }
  }
}
```

Add one only when the case cannot exist for other connectors — a sandbox-specific
trigger, or a production bug pinned as a permanent test. Anything shareable belongs
in `global_suites/` so every connector gets it.

Rules the harness enforces:

- **Additive only.** A name that already exists in the global suite is an error, not
  an override — use `override.json` to change a shared scenario.
- **Same proto schema** as global scenarios.
- **Counted separately** in the run summary (`connector_specific=N`), so private
  coverage never reads as baseline coverage.

### `connector_specs/<connector>/specs.json`

Per-connector spec. All fields except `connector` and `supported_suites` are optional.

```jsonc
{
  "connector": "redsys",
  "supported_suites": [
    "PaymentService/Authorize",
    "PaymentService/Capture",
    "PaymentService/Void",
    "PaymentService/Refund"
  ],

  // When generating the connector_request_reference_id, read it from this
  // field in the request body. If absent or empty, generate a fresh value
  // using `request_id_prefix` + `request_id_length` chars of a UUID and
  // write the generated value back into the body's source_field path so
  // the gRPC server (which reads the ref_id from the body, not the header)
  // sees a consistent id.
  "request_id_source_field": "merchant_transaction_id",
  "request_id_prefix": "0001",
  "request_id_length": 12,

  // Per-suite override of `request_id_source_field` when the connector
  // reads the reference id from a different proto field in some suites
  // (e.g. PreAuthenticate / Authenticate often use merchant_order_id
  // while Authorize uses merchant_transaction_id).
  "request_id_source_field_per_suite": {
    "PaymentMethodAuthenticationService/PreAuthenticate": "merchant_order_id"
  },

  // Scenarios this connector cannot support, as suite -> scenario -> reason.
  // They are skipped instead of run and failed. Lives here rather than in
  // override.json because it states a capability, not a test-data delta: what
  // a connector cannot do is answered by this one file. The reason is the map
  // value, so a declaration without one cannot be written.
  "unsupported_scenarios": {
    "PaymentService/Authorize": {
      "no3ds_auto_capture_upi_qr": "redsys has no UPI support"
    }
  },

  // For Get / sync flows: re-poll until status reaches a terminal value
  // or this budget elapses. Set when the sandbox auto-settles after a delay.
  "sync_poll_until_terminal_seconds": 30,

  // Per-connector additions to suite_spec's depends_on. Prepended at runtime.
  // Useful for connectors whose Authorize requires upstream context that
  // isn't part of the standard global chain.
  "additional_dependencies": {
    "PaymentService/Authorize": [
      {
        "suite": "PaymentMethodAuthenticationService/PreAuthenticate",
        "scenario": "threeds_card_pre_authenticate",
        "context_map": {
          "authentication_data": "res.authentication_data"
        }
      }
    ]
  }
}
```

### `connector_specs/<connector>/override.json`

Per-scenario request/assert overrides. Keyed by full suite name → scenario name. Patch payload uses JSON merge-patch semantics — `null` removes a key, leaf-field edits replace leaves, nested objects are merged.

```json
{
  "PaymentService/Authorize": {
    "no3ds_fail_payment": {
      "grpc_req": {
        "payment_method": {
          "card": { "card_number": { "value": "4000000000000002" } }
        }
      },
      "assert": {
        "status": { "one_of": ["FAILURE", "AUTHORIZATION_FAILED"] },
        "error.connector_details.message": { "must_exist": true }
      }
    }
  }
}
```

Also supports `pre_request_http`:

```json
"pre_request_http": {
  "url": "https://sandbox.example/simulate",
  "method": "POST",
  "body": "{ \"order_id\": \"{{dep_res.1.merchantOrderId}}\" }",
  "timeout_secs": 31
}
```

Fire-and-forget HTTP call (or, when `url` is absent, a `timeout_secs` sleep) before the scenario's gRPC request. The sleep form is useful for connectors that reject duplicate requests within a window.

Validate after editing:

```bash
cargo test -p integration-tests all_override_entries_match_existing_scenarios_and_proto_schema
cargo test -p integration-tests all_supported_scenarios_match_proto_schema_for_all_connectors
```

### `connector_specs/<connector>/browser_automation_spec.json`

Optional. Drives a headless browser through a redirect/challenge step between two suite invocations — typically for 3DS challenge pages. See `connector_specs/stripe/browser_automation_spec.json` for the simplest reference (single waitFor → click → waitFor pattern) and `connector_specs/nexixpay/browser_automation_spec.json` for a more involved post-authenticate pattern.

---

## Workflow patterns

### First-time setup on a new machine

```bash
make setup-connector-tests
test-prism --interactive
```

### Run tests for a new connector

```bash
test-prism --connector <connector-name> --report
```

### Debug a failing scenario

```bash
# Print the harness-built request payload
export UCS_DEBUG_EFFECTIVE_REQ=1
test-prism --connector stripe --suite PaymentService/Authorize \
  --scenario no3ds_auto_capture_credit_card

# Inspect only the latest run entry for that scenario
python3 -c "
import json
r = json.load(open('crates/internal/integration-tests/report.json'))
last = [x for x in r['runs'] if x['connector']=='stripe'][-1]
print(json.dumps(last, indent=2))
"
```

### Add a connector-specific override

1. Identify the base scenario key in `src/global_suites/<Suite_Name>/scenario.json`.
2. Open or create `src/connector_specs/<connector>/override.json`.
3. Add a leaf-field patch under `<Suite/Name>` → `<scenario_name>`.
4. Run the two schema-validation tests above.
5. Re-run the targeted scenario.

### CI / automation

```bash
export CONNECTOR_AUTH_FILE_PATH="/path/to/creds.json"
export UCS_ALL_CONNECTORS="stripe,paypal,adyen"
export SKIP_NETLIFY_DEPLOY=1
make setup-connector-tests
test-prism --all-connectors --report
```

---

## Direct cargo invocations (advanced)

For low-level debugging without the `test-prism` wrapper:

```bash
# One scenario
make cargo ARGS="run -p integration-tests --bin run_test -- \
  --connector stripe --suite PaymentService/Authorize \
  --scenario no3ds_auto_capture_credit_card"

# One suite, all scenarios
make cargo ARGS="run -p integration-tests --bin suite_run_test -- \
  --connector stripe --suite PaymentService/Authorize"

# All suites for one connector
make cargo ARGS="run -p integration-tests --bin suite_run_test -- \
  --connector stripe --all"

# SDK backend
make cargo ARGS="run -p integration-tests --bin sdk_run_test -- \
  --connector stripe --all"
```

### Regenerate markdown report from JSON only

```bash
cargo run -p integration-tests --bin render_report
```

### Generate / refresh scenario display names

Display names are the human-readable labels shown in markdown reports (e.g. `Credit Card | No 3DS | Automatic Capture`). They live under each scenario's `display_name` field.

```bash
# All suites
cargo run -p integration-tests --bin generate_scenario_display_names

# One suite
cargo run -p integration-tests --bin generate_scenario_display_names -- \
  --suite PaymentService/Authorize

# Preview without writing
cargo run -p integration-tests --bin generate_scenario_display_names -- --check

# Generate and regenerate markdown in one go
cargo run -p integration-tests --bin generate_scenario_display_names -- --render-markdown
```

### Coverage check

```bash
cargo run --bin check_coverage
```

(Currently ignores PayoutService and DisputeService.)

---

## Environment variables

| Variable | Purpose |
|---|---|
| `CONNECTOR_AUTH_FILE_PATH` | Path to `creds.json` (highest priority) |
| `UCS_CREDS_PATH` | Fallback path to `creds.json` |
| `UCS_ALL_CONNECTORS` | Comma list for `--all-connectors` (`stripe,paypal,…`) |
| `UCS_SDK_ENVIRONMENT` | `sandbox` (default) or `production` |
| `UCS_DEBUG_EFFECTIVE_REQ=1` | Print the harness-built effective request before sending |
| `UCS_DEBUG_PRE_REQUEST_HOOK=1` | Log `pre_request_http` hook outcomes |
| `UCS_DEBUG_GRPCURL_PAYLOAD=1` | Log the full grpcurl command + payload |
| `UCS_CONNECTOR_OVERRIDE_ROOT` | Override the path under which `connector_specs/<connector>/override.json` is read |
| `UCS_CONNECTOR_SPECS_ROOT` | Override the path under which `connector_specs/<connector>/specs.json` is read |
| `UCS_SCENARIO_ROOT` | Override the path to `global_suites/` |
| `SKIP_NETLIFY_DEPLOY=1` | Skip the Netlify step during setup (disables Google Pay tests) |

---

## Common pitfalls

- **Edits to `creds.json` not landing.** `.env.connector-tests` (sourced by `scripts/run-tests`) may set `UCS_CREDS_PATH` to a different file. Read `.env.connector-tests` first.
- **`CredentialError::LegacyFormat`.** Your creds use the nested `connector_account_details` wrapper. Use the flat shape — keys map directly to the connector's proto config message.
- **`Missing required field` after override.** The override patches `grpc_req` *before* the harness prunes `"auto_generate"` sentinels and resolves auto-gen. Don't override required fields with `"auto_generate"` if you want a real value; either inline the real value or rely on the auto-gen path for that field.
- **Short reference IDs (e.g. 12-char order IDs).** Set `request_id_source_field` + `request_id_prefix` + `request_id_length` in `specs.json`. The harness generates a unique short id per request and writes it into the body's source field (only when the field exists in that suite's `scenario.json`, so downstream protos that don't have the field aren't polluted).
- **Cascading failures (`Capture` is 0/x but `Authorize` works).** `Capture`/`Void` depend on `PaymentService/Authorize/no3ds_manual_capture_credit_card` by design — you can't capture an auto-captured payment. Check the manual-capture authorize first.
- **Issuer rejection at low amounts.** The test issuer can reject specific expiry / cvc / holder-name combos even with a valid card number. Use the exact triples documented in the connector's integration PR (cURLs/grpcurls in the description) rather than the base scenario defaults.
- **`Address already in use` when restarting the gRPC server.** The metrics endpoint also binds (defaults to 8080). Kill listeners on both: `lsof -ti:8000 | xargs kill -9; lsof -ti:8080 | xargs kill -9`.
- **Response body is masked (`*** alloc::string::String ***`).** Sensitive fields are wrapped in `Secret`. For diagnosis, add a temporary `tracing::error!` at the encoding boundary in the connector's transformer, run, then revert. Never commit the trace line.

---

## Further reading

- `docs/scenario-json-core-readme.md` — scenario.json format and runner semantics
- `docs/connector-overrides.md` — override.json patch rules
- `docs/code-walkthrough.md` — how the harness builds a request
- `docs/context-mapping.md` — dependency context propagation
- `grace/workflow/3_test.md` — operational workflow for moving a connector to "Hardened"

## Support

- Issues: https://github.com/juspay/connector-service/issues
- `test-prism --help` for the full flag list
