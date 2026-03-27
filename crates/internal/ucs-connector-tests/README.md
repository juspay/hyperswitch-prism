# UCS Connector Tests

Scenario-driven connector integration harness for UCS. The harness reads suite/scenario JSON, applies connector overrides, executes flows, validates assertions, and writes JSON + markdown reports.

## Core layout

- Global suites: `src/global_suites/<suite>_suite/scenario.json` + `suite_spec.json`
- Connector support matrix: `src/connector_specs/<connector>/specs.json`
- Connector overrides: `src/connector_specs/<connector>/override.json`
- Assertion DSL: inside each scenario `assert` block

More details:

- `docs/scenario-json-core-readme.md`
- `docs/connector-overrides.md`
- `docs/code-walkthrough.md`

## Prerequisites

- Run commands from repo root.
- Rust toolchain installed.
- `grpcurl` installed for grpcurl backend runs.
- Connector credentials file available (shape: `.github/test/template_creds.json`).

## Non-interactive usage (recommended)

Run one scenario:

```bash
cargo run -p ucs-connector-tests --bin run_test -- \
  --suite authorize \
  --scenario no3ds_auto_capture_credit_card \
  --connector stripe
```

Run one suite (all scenarios) for one connector:

```bash
cargo run -p ucs-connector-tests --bin suite_run_test -- \
  --suite authorize \
  --connector stripe
```

Run all suites for one connector:

```bash
cargo run -p ucs-connector-tests --bin suite_run_test -- --all --connector stripe
```

Run all suites for all configured connectors:

```bash
cargo run -p ucs-connector-tests --bin suite_run_test -- --all-connectors
```

Run with SDK/FFI backend (non-interactive):

```bash
cargo run -p ucs-connector-tests --bin sdk_run_test -- --all --connector stripe
```

SDK-supported connectors today: `stripe`, `authorizedotnet`, `paypal`.

## Interactive usage (optional)

```bash
cargo run -p ucs-connector-tests --bin test_ucs
```

or:

```bash
make test-ucs
```

## Environment variables

| Variable | Required | Purpose | Example |
|---|---|---|---|
| `CONNECTOR_AUTH_FILE_PATH` | yes (or `UCS_CREDS_PATH`) | primary creds file path | `export CONNECTOR_AUTH_FILE_PATH="$PWD/.github/test/creds.json"` |
| `UCS_CREDS_PATH` | yes (fallback) | secondary creds file path | `export UCS_CREDS_PATH="$PWD/.github/test/creds.json"` |
| `UCS_ALL_CONNECTORS` | optional | connector list for `--all-connectors` | `export UCS_ALL_CONNECTORS="stripe,paypal,authorizedotnet"` |
| `UCS_CONNECTOR_LABEL_<CONNECTOR>` | optional | select nested connector account label | `export UCS_CONNECTOR_LABEL_PAYPAL=connector_1` |
| `UCS_RUN_TEST_REPORT_PATH` | optional | custom report json path | `export UCS_RUN_TEST_REPORT_PATH="$PWD/backend/ucs-connector-tests/report.json"` |
| `UCS_RUN_TEST_DEFAULTS_PATH` | optional | saved defaults path for `run_test` | `export UCS_RUN_TEST_DEFAULTS_PATH="$PWD/.tmp/run_test_defaults.json"` |
| `UCS_SCENARIO_ROOT` | optional | custom suite root path | `export UCS_SCENARIO_ROOT="$PWD/backend/ucs-connector-tests/src/global_suites"` |
| `UCS_CONNECTOR_SPECS_ROOT` | optional | custom connector specs root | `export UCS_CONNECTOR_SPECS_ROOT="$PWD/backend/ucs-connector-tests/src/connector_specs"` |
| `UCS_CONNECTOR_OVERRIDE_ROOT` | optional | custom override root | `export UCS_CONNECTOR_OVERRIDE_ROOT="$PWD/backend/ucs-connector-tests/src/connector_specs"` |
| `UCS_DEBUG_EFFECTIVE_REQ` | optional | print effective request payload | `export UCS_DEBUG_EFFECTIVE_REQ=1` |
| `UCS_SDK_ENVIRONMENT` | optional | SDK env (`sandbox` default / `production`) | `export UCS_SDK_ENVIRONMENT=sandbox` |

### Local setup example

```bash
export CONNECTOR_AUTH_FILE_PATH="$PWD/.github/test/creds.json"
export UCS_ALL_CONNECTORS="stripe,paypal,authorizedotnet"
export UCS_SDK_ENVIRONMENT=sandbox
```

### CI setup example

```bash
export CONNECTOR_AUTH_FILE_PATH="$WORKSPACE/creds.json"
export UCS_ALL_CONNECTORS="stripe,paypal"
cargo run -p ucs-connector-tests --bin suite_run_test -- --all-connectors
```

## Common CLI options

Applies to `suite_run_test` and `sdk_run_test`:

- `--suite <suite>` run one suite
- `--all` run all suites for one connector
- `--all-connectors` run all connectors from configured list
- `--connector <name>` connector name
- `--endpoint <host:port>` gRPC endpoint (grpcurl path)
- `--creds-file <path>` credentials file path
- `--merchant-id <id>` defaults to `test_merchant`
- `--tenant-id <id>` defaults to `default`
- `--tls` use TLS instead of plaintext
- `--report <path>` custom report output path

## Reports

- JSON report: `backend/ucs-connector-tests/report.json` (or `UCS_RUN_TEST_REPORT_PATH` / `--report`)
- Markdown overview: `backend/ucs-connector-tests/test_report/test_overview.md`
- Connector flow pages: `backend/ucs-connector-tests/test_report/connectors/<connector>/<suite>.md`
- Scenario details: `backend/ucs-connector-tests/test_report/connectors/<connector>/<suite>/<scenario>.md`

Reports are cleared at run start and updated as scenarios execute.

Regenerate markdown from an existing `report.json` (without executing tests):

```bash
cargo run -p ucs-connector-tests --bin render_report
```

Optional custom report path:

```bash
cargo run -p ucs-connector-tests --bin render_report -- --path backend/ucs-connector-tests/report.json
```

## Dependency field mapping (`context_map`)

Dependency data is propagated in two steps:

1. Implicit mapping (`add_context`):
   - Tries same-path lookup first.
   - Applies built-in aliases for known shape differences (snake/camel/pascal case,
     `state.*` flattening, identifier oneof wrappers, etc.).
2. Explicit mapping (`context_map`):
   - Declared in `suite_spec.json` under `depends_on`.
   - Applied after implicit mapping, so explicit values override inferred ones.

When one suite depends on another, request/response names can still differ across
flows (for example `setup_recurring` response uses `mandate_reference.*`, while
`recurring_charge` request expects `connector_recurring_payment_id.*`).

Use `context_map` inside `depends_on` to make these mappings explicit and reviewable in JSON:

```json
{
  "suite": "recurring_charge",
  "depends_on": [
    {
      "suite": "setup_recurring",
      "context_map": {
        "connector_recurring_payment_id.connector_mandate_id.connector_mandate_id": "res.mandate_reference.connector_mandate_id.connector_mandate_id"
      }
    }
  ]
}
```

Rules:

- Left side: target path in downstream request.
- Right side: source path in dependency payload (`res.` or `req.` prefix; `res.` default).
- Explicit `context_map` values are applied after implicit auto-matching, so explicit mappings win.
- If source path is missing/null, mapping is skipped.

When to use `context_map`:

- Required: source and target names/paths differ (`refund_id <- res.connector_refund_id`).
- Required: one dependency field can come from multiple places and source must be pinned.
- Optional: exact same-name, unambiguous fields (implicit mapping is enough).

Detailed walkthrough: `docs/context-mapping.md`

## Proto/schema drift checks

Run strict scenario-to-proto compatibility checks:

```bash
cargo test -p ucs-connector-tests all_supported_scenarios_match_proto_schema_for_all_connectors
cargo test -p ucs-connector-tests all_override_entries_match_existing_scenarios_and_proto_schema
```

These tests fail when:

- a scenario field no longer exists in proto request shape
- an enum value is invalid
- an override points to missing suite/scenario

## Adding connector override

Quick flow:

1. Locate base scenario in `src/global_suites/<suite>_suite/scenario.json`.
2. Edit `src/connector_specs/<connector>/override.json`.
3. Add only connector-specific delta in `grpc_req` and/or `assert`.
4. Validate with non-interactive command.
5. Run strict schema checks above.

Minimal example:

```json
{
  "authorize": {
    "no3ds_fail_payment": {
      "grpc_req": {
        "payment_method": {
          "card": {
            "card_number": { "value": "4000000000000002" }
          }
        }
      },
      "assert": {
        "error.connector_details.message": { "contains": "declin" },
        "status": null
      }
    }
  }
}
```

Detailed override guide: `docs/connector-overrides.md`

## Helpful commands

```bash
# Help
cargo run -p ucs-connector-tests --bin run_test -- --help
cargo run -p ucs-connector-tests --bin suite_run_test -- --help
cargo run -p ucs-connector-tests --bin sdk_run_test -- --help
cargo run -p ucs-connector-tests --bin render_report -- --help

# Full crate tests
cargo test -p ucs-connector-tests
```
