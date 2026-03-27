# UCS Connector Tests: Code Walkthrough

This document explains the harness as implementation documentation: what each module does, why key conditions exist, and how major variables/fields flow across execution.

## 1) High-level flow

1. CLI binary parses args and resolves defaults/env.
2. Scenario template is loaded from `src/global_suites/<suite>_suite/scenario.json`.
3. Connector override patch (if present) is merged into request + assertions.
4. `auto_generate` placeholders are resolved for non-context-deferred fields.
5. Dependency suites/scenarios run first (based on `suite_spec.json`).
6. Dependency response/request values are mapped into the target request
   (implicit matching + explicit `context_map`).
7. Request executes through selected backend:
   - grpcurl backend
   - SDK/FFI backend
8. Assertions run against response JSON.
9. `ReportEntry` is appended to `report.json`, and `test_report.md` is regenerated.

## 2) Module map

- `src/harness/scenario_api.rs`
  - Core orchestration: loading scenarios, applying dependencies, execution, and result shaping.
  - Important public APIs: `run_suite_test_with_options`, `run_scenario_test_with_options`, `run_all_suites_with_options`, `run_all_connectors_with_options`.
- `src/harness/scenario_loader.rs`
  - File-system and JSON loaders for suite scenarios/specs and connector specs.
  - Handles root path env overrides and compatibility fallbacks.
- `src/harness/scenario_assert.rs`
  - Assertion engine (`must_exist`, `must_not_exist`, `equals`, `one_of`, `contains`, `echo`).
- `src/harness/auto_gen.rs`
  - Sentinel resolver for `auto_generate` values with type/path-aware generated data.
- `src/harness/connector_override/*`
  - Connector-specific request/assertion patches and merge semantics.
- `src/harness/sdk_executor.rs`
  - SDK/FFI backend execution pipeline.
- `src/harness/report.rs`
  - JSON report append + Markdown generation.
- `src/harness/credentials.rs`
  - Credential file loading and auth-shape normalization.
- `src/harness/metadata.rs`
  - Header injection contract for connector/auth metadata.
- `src/harness/server.rs`, `src/harness/executor.rs`
  - In-process UCS server bootstrap and tonic request helpers.
- `src/bin/*.rs`
  - CLI modes (`run_test`, `suite_run_test`, `sdk_run_test`, `test_ucs`).

## 3) Key variables and conditions (why they exist)

### 3.1 Scenario + dependency execution

- `dependency_scope` in `suite_spec.json`
  - `suite`: run dependencies once before suite scenarios.
  - `scenario`: run dependencies before every scenario.
- `strict_dependencies`
  - If true, dependency failure is treated as hard blocker.
  - If false, suite can continue depending on orchestration path.
- `context_map`
  - Explicit mapping from dependency output/input paths into target request fields.
  - Example: map `res.connector_refund_id` to `refund_id`.
  - Applied after implicit mapping so explicit values override inferred ones.
- `add_context` (implicit mapping)
  - Default propagation path used for all dependencies.
  - Matches same-name fields first, then known alias candidates.
  - Alias examples: `refund_id <- connector_refund_id`,
    `state.access_token.token_type <- token_type`,
    `*.id <- *.id_type.id`.

#### When explicit `context_map` is needed

- Keep explicit entries for cross-flow name/path mismatches or when source selection must be deterministic.
- Skip explicit entries for exact same-name and unambiguous fields; implicit mapping already covers those.

### 3.2 Auto-generation behavior

- Condition: `is_auto_generate_sentinel(...)`
  - Only marked fields are synthesized.
- Condition: `is_context_deferred_path(...)`
  - Skips generation for fields expected from dependencies (avoids generating incorrect placeholders).
- Variable: `lower_path`
  - Normalized path used for stable path-pattern matching.

### 3.3 Connector override behavior

- Condition: scenario patch exists for `(connector, suite, scenario)`
  - If absent: no-op.
  - If present: request patch and assertion patch are both applied.
- Condition: assertion patch value is `null`
  - Removes that assertion rule.
- JSON merge semantics:
  - object/object -> recursive merge
  - `null` -> delete key
  - non-object mismatch -> replace target with patch value

### 3.4 Assertion behavior

- `MustExist`: fails if value missing or null.
- `MustNotExist`: fails if value exists and non-null.
- `OneOf`: fails if value missing or not in expected set.
- `Contains`: string-only case-insensitive containment check.
- `Echo`: compares response field to request field looked up by path.

### 3.5 Report behavior

- `is_dependency` rows are recorded but excluded from matrix-level deduped scenario rows.
- Dedup key includes `(suite, scenario, connector)` and keeps latest run by timestamp/index.
- Scenario details section is generated from deduped rows and suite specs.

## 4) Core data structures

### 4.1 `ScenarioDef` (`scenario_types.rs`)

- `grpc_req`: request template JSON for one scenario.
- `assert_rules`: field assertion map.
- `is_default`: identifies the suite default scenario.

### 4.2 `SuiteSpec` (`scenario_types.rs`)

- `suite`: suite name.
- `suite_type`: descriptive classification.
- `depends_on`: dependency list.
- `strict_dependencies`: dependency strictness policy.
- `dependency_scope`: suite-level vs scenario-level dependency execution.

### 4.3 `ReportEntry` (`report.rs`)

- Execution identity: `run_at_epoch_ms`, `suite`, `scenario`, `connector`, `endpoint`.
- Derived request metadata: `pm`, `pmt`.
- Result metadata: `assertion_result`, `response_status`, `error`.
- Execution context: `is_dependency`, `dependency`, `req_body`, `res_body`.

## 5) CLI mode responsibilities

- `run_test.rs`
  - Single `(suite, scenario, connector)` run.
  - Prints grpcurl command + response and records one report entry.
- `suite_run_test.rs`
  - grpcurl backend for suite/all/all-connectors modes.
- `sdk_run_test.rs`
  - Same suite modes but SDK backend (`ExecutionBackend::SdkFfi`).
- `test_ucs.rs`
  - Interactive selection UX for connector/suite/scenario/backend.

## 6) Environment variables used

- `UCS_SCENARIO_ROOT`
- `UCS_CONNECTOR_SPECS_ROOT`
- `UCS_CONNECTOR_OVERRIDE_ROOT`
- `CONNECTOR_AUTH_FILE_PATH`
- `UCS_CREDS_PATH`
- `UCS_ALL_CONNECTORS`
- `UCS_RUN_TEST_REPORT_PATH`
- `UCS_SDK_ENVIRONMENT`

## 7) Extension checklist

When adding new suite/scenario coverage:

1. Add/modify `scenario.json` under `src/global_suites/<suite>_suite/`.
2. Keep exactly one default scenario per suite.
3. Update `suite_spec.json` dependencies/context mapping where needed.
4. Add connector-specific differences to `connector_specs/<connector>/override.json`.
5. Validate with `cargo test -p ucs-connector-tests`.

When adding brand-new RPC suites:

1. Add new suite files (`scenario.json`, `suite_spec.json`).
2. Wire execution + method mapping in `scenario_api.rs`.
3. Update report service mapping/order in `report.rs`.
4. Add connector support in `connector_specs/*/specs.json`.
