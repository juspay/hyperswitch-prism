# Connector Scenario Overrides

This harness supports connector-specific scenario overrides through a trait-based engine backed by JSON merge patches.

## Goals

- Keep `src/global_suites/*` as the single global baseline.
- Let connectors override only what differs.
- Allow connector-side extra keys in request/assert payloads.
- Restrict overrides to existing scenarios (no connector-only scenario creation).

## When to use override

Use overrides only when connector behavior differs from global baseline, for example:

- test card number differs per connector
- error message assertion differs
- connector needs extra request field
- connector cannot support one assertion field from baseline

Do not duplicate full scenario payload unless necessary.

## Directory layout

```text
backend/ucs-connector-tests/src/
  global_suites/
    <suite>_suite/scenario.json
  connector_specs/
    <connector>/
      specs.json
      override.json
```

Example:

```text
src/connector_specs/stripe/override.json
```

## Override file format

Each connector `override.json` is a map from `suite_name -> scenario_name -> patch payload`.

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
        "status": { "one_of": ["FAILURE"] },
        "error.connector_details.message": { "contains": "declin" }
      }
    }
  }
}
```

## Add override: step by step

1. Identify the global scenario key in `src/global_suites/<suite>_suite/scenario.json`.
2. Open (or create) `src/connector_specs/<connector>/override.json`.
3. Add `<suite> -> <scenario>` patch entry.
4. Put request delta under `grpc_req`.
5. Put assertion delta under `assert`.
6. Validate with non-interactive run.
7. Run strict schema checks.

Example validation commands:

```bash
# run one suite for one connector
cargo run -p ucs-connector-tests --bin suite_run_test -- --suite authorize --connector stripe

# strict proto/schema checks
cargo test -p ucs-connector-tests all_supported_scenarios_match_proto_schema_for_all_connectors
cargo test -p ucs-connector-tests all_override_entries_match_existing_scenarios_and_proto_schema
```

## Merge semantics

`grpc_req` uses JSON Merge Patch semantics:

- Object keys are merged recursively.
- Scalars/arrays replace existing values.
- `null` removes a key.
- Keys missing in the base are allowed and added.

`assert` supports:

- Add new assertion fields.
- Replace existing assertion rule for a field.
- Remove assertion field by setting its value to `null`.

Example: remove one baseline assertion rule

```json
{
  "authorize": {
    "no3ds_fail_payment": {
      "assert": {
        "status": null
      }
    }
  }
}
```

## Trait and registry

Core trait: `src/harness/connector_override/mod.rs`

- `ConnectorOverride::apply_overrides(...)` default implementation reads JSON patches.
- `OverrideRegistry` resolves to a generic default strategy for every connector.
- No connector-specific Rust files are required.

## Runtime usage

When loading a scenario for a connector:

1. Load base from `global_suites/<suite>_suite/scenario.json`.
2. Load connector patch from `connector_specs/<connector>/override.json`.
3. Apply request patch + assertion patch for that scenario.
4. Execute with normal dependency/context pipeline.

## Common mistakes

- Suite key typo (example: `authorise` instead of `authorize`).
- Scenario key typo that does not exist in global suite file.
- Wrong enum string value (case mismatch) in patched request.
- Adding field paths that no longer exist in proto request shape.
- Replacing entire nested objects when only a leaf override was intended.

Schema compatibility tests will catch these during CI.

## Configurable root

Override root can be changed with:

```text
UCS_CONNECTOR_OVERRIDE_ROOT=/absolute/path/to/connector_specs
```

If unset, default root is `src/connector_specs/`.

## Related docs

- `../README.md`
- `./scenario-json-core-readme.md`
- `./code-walkthrough.md`
