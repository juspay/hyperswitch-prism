# Connector Scenario Overrides

This harness supports connector-specific scenario overrides through a trait-based engine backed by JSON merge patches.

## Goals

- Keep `src/global_suites/*` as the single global baseline.
- Let connectors override only what differs.
- Allow connector-side extra keys in request/assert payloads.
- Patch existing scenarios or add named variants in `override.json` using `extends`.
- Use `connector_specific_scenarios.json` only for standalone cases without a suitable base.
  New variants and standalone cases cannot shadow existing scenario names.
- Keep capability statements out of this file; they live in `specs.json`.

## Which file to use

Four files can carry connector-specific test data. Pick the first one that fits:

| Situation | File | Notes |
|---|---|---|
| A global scenario already matches, and its assertions are true for this connector | nothing — just list the suite in `specs.json` `supported_suites` | the common case; prefer it |
| The global scenario needs a connector-specific **input** (test card, metadata blob, 3DS lever) | `override.json` `grpc_req` | RFC 7396 merge patch. **Do not** patch a path that the suite's `suite_spec.json` lists as a `context_map` target — the override is applied *after* the context map (`scenario_api.rs`: `apply_context_map` then `apply_connector_overrides`), so it silently decouples the case from its dependency |
| The global scenario's **expected outcome** differs for documented reasons | `override.json` `assert` | narrow or re-target a rule. A bare `null` deletion removes the rule entirely — replace it, do not just delete it |
| An additional case differs from an existing scenario | `override.json` with `extends` | inherits the base request, assertions, and connector overrides; supply only differences. Optional `depends_on` and `dependency_scope` control prerequisites |
| A standalone case has no suitable base | `connector_specific_scenarios.json` | additive only; a name that collides with a global scenario is a hard error. Both `grpc_req` and `assert` are required |
| The scenario genuinely does not apply | `specs.json` `unsupported_scenarios` | skips, does not fail. The reason string is mandatory and is the only record of why |

A waived scenario produces **no row in `report.json`** — it is removed before the run — so a waiver is
invisible to anything reading the report. Diff `specs.json` to find one.

## When to use override

Use overrides only when connector behavior differs from global baseline, for example:

- test card number differs per connector
- error message assertion differs
- connector needs extra request field
- connector cannot support one assertion field from baseline

Do not duplicate full scenario payload unless necessary.

An entry in `override.json` does not make the scenario connector-specific. For
example, `no3ds_auto_capture_credit_card` remains a shared Authorize scenario;
Elavon's override supplies its sandbox card number, expiry, and CVC. Keep those
values out of the global baseline.

Use named variants for address omission, expiry formats, saving a card for future
payments, and different amounts or metadata. They inherit the connector's card
fixtures instead of copying those fixtures into every case. New payment methods
such as crypto, FPX/DuitNow, and Google Pay tokenization can still have shared
baselines selected through `supported_payment_methods`.

Fiuu's signed authorization, pending-payment, and failed-refund callbacks are named
variants of its payment/refund webhook cases. They inherit the normal fixture
setup and change only the callback body, capture context, and expected outcome.

## Directory layout

```text
backend/integration-tests/src/
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

## Named variants and prerequisites

A new scenario name with `extends` inherits an existing scenario in the **same
suite**, including that connector's base overrides. Bases may be global scenarios,
standalone connector scenarios, or other named variants. Forward references work;
unknown bases, cycles, and names that shadow existing definitions are errors.
An unknown scenario name without `extends` is also rejected, catching typos.

```json
{
  "PaymentService/Capture": {
    "capture_partial_fresh_authorization": {
      "extends": "capture_partial_amount",
      "dependency_scope": "scenario",
      "grpc_req": {
        "amount_to_capture": { "minor_amount": 1500 }
      }
    },
    "capture_custom_authorization": {
      "extends": "capture_partial_fresh_authorization",
      "depends_on": [
        {
          "suite": "PaymentService/Authorize",
          "scenario": "no3ds_manual_capture_credit_card",
          "context_map": {
            "connector_transaction_id": "res.connector_transaction_id",
            "amount_to_capture.currency": "req.amount.currency"
          }
        }
      ]
    }
  }
}
```

- With neither dependency field, a variant inherits its base's settings and then
  the effective connector suite prerequisites. Suite-scoped prerequisites are shared.
- `dependency_scope: "scenario"` reruns the inherited chain for that variant.
- `depends_on` replaces the entire inherited chain and always runs independently.
  Entries use the existing suite dependency format, including named scenarios and
  `context_map`. Include any authentication/tokenization steps the case needs.
- `depends_on: []` explicitly runs without prerequisites.
- `required_payment_method` and `pre_request_http` can be customized.
  Hooks inherit from the nearest ancestor that defines one. A variant is never
  implicitly made the suite default.

The CLI, single-scenario runner, suite runner, and schema checks all discover named
variants. Explicit connector-local variants run in addition to filtered global
scenarios; `unsupported_scenarios` still skips them by their own name.

For changes shared by a whole connector suite, `specs.json` supports
`suite_dependencies: { "Suite/Method": { "before": [], "after": [],
"replace": [], "dependency_scope": "scenario" } }`. All four fields are optional.
The effective chain is `before + (replace or global chain) + after`.
Omit `replace` to retain the global chain; an empty `replace` removes it.

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
cargo run -p integration-tests --bin suite_run_test -- --suite authorize --connector stripe

# strict proto/schema checks
cargo test -p integration-tests all_supported_scenarios_match_proto_schema_for_all_connectors
cargo test -p integration-tests all_override_entries_match_existing_scenarios_and_proto_schema
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

## Declaring a scenario unsupported

Not an override. Use `unsupported_scenarios` in `connector_specs/<connector>/specs.json`
— it states a capability, so it belongs with `supported_suites` and
`supported_payment_methods` rather than in this file.

Do not reach for an `assert` override that expects the failure instead. It passes,
carries no reason, and turns a future real regression green.

## Trait and registry

Core trait: `src/harness/connector_override/mod.rs`

- `ConnectorOverride::apply_overrides(...)` default implementation reads JSON patches.
- `OverrideRegistry` resolves to a generic default strategy for every connector.
- No connector-specific Rust files are required.

## Runtime usage

When loading a scenario for a connector:

1. Load base from `global_suites/<suite>_suite/scenario.json`.
2. Resolve any `extends` chain and execution metadata from `override.json`.
3. Execute the selected prerequisites and apply dependency context.
4. Apply ancestor request/assertion patches before the variant's own patches.
   For webhook scenarios, each ancestor's `webhook_payload.json` fixture is also
   applied before the next variant layer.
5. Execute and assert using the variant's name.

## Common mistakes

- Suite key typo (example: `authorise` instead of `authorize`).
- A new scenario name without `extends`, or a misspelled base scenario.
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
