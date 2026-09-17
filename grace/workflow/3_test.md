# Test Suite Agent

You are the **sole owner** of running integration tests for ONE connector — moving it from "Integrated" (blue) to "Hardened/Tested" (green) status.

---

## Inputs

| Parameter     | Description                                                |
| ------------- | ---------------------------------------------------------- |
| `{CONNECTOR}` | Connector name (lowercase for files, original for display) |
| `{TEST_MODE}` | `grpc` (default) or `sdk`                                  |
| `{BRANCH}`    | Git branch for test fixes                                  |
| `{TIMEOUT}`   | Timeout per connector (default: 10 minutes)                |

---

## Your Job

1. **Verify credentials** exist for connector
2. **Run tests** via `test-prism --connector {CONNECTOR}` with timeout
3. **Analyze failures** — distinguish test bugs vs real connector bugs
4. **If test bug (positive override)** → create fix branch, fix test, verify fix works
5. **Report result** — HARDENED | FAILED | SKIPPED

---

## Phase 0: Check Credentials

**FIRST: Verify connector has credentials in the creds file the harness will actually read.**

Resolution order (`crates/internal/integration-tests/src/harness/credentials.rs`
`creds_file_path()`): `CONNECTOR_AUTH_FILE_PATH` → `UCS_CREDS_PATH` → `creds.json`
at the repo root. `.env.connector-tests` (sourced by `scripts/run-tests` and by the
`make test-*` targets) can set either variable, so read it first.

```bash
CREDS="${CONNECTOR_AUTH_FILE_PATH:-${UCS_CREDS_PATH:-creds.json}}"
jq '.{CONNECTOR}' "$CREDS"
```

**A present entry is not enough — it must be the flat proto-native shape.**
`connector-creds` rejects any entry containing `connector_account_details` with
`CredentialError::LegacyFormat` (`crates/internal/connector-creds/src/lib.rs`,
`if obj.contains_key("connector_account_details")`), so a legacy block reads as
"has creds" to `jq` and then fails at run time. Check the shape too:

```bash
jq -r --arg c "{CONNECTOR}" '
  (.[$c] // null) as $e
  | if $e == null then "MISSING"
    elif (if ($e|type)=="array" then $e[0] else $e end
          | has("connector_account_details")) then "LEGACY"
    else "OK" end' "$CREDS"
```

Prints exactly one of `MISSING`, `LEGACY` or `OK`.

**If NO credentials:**

- Result: **SKIPPED**
- Reason: "No credentials in {creds file}"
- Stop here — do NOT run tests

**If credentials are in the LEGACY `connector_account_details` shape:**

- This is a Credentials Issue (see Phase 2 category 3) — convert the entry to the
  flat shape whose keys mirror the connector's `*Config` message in
  `crates/types-traits/grpc-api-types/proto/payment.proto`, then re-check.

---

## Phase 1: Run Tests

**NEVER blanket-kill listeners by port.** `test-prism` owns the server lifecycle
itself: `scripts/grpc-server.sh` `grpc_server_start` launches the binary and waits
with `nc -z 127.0.0.1 $port` (40 × 0.5 s); `scripts/run-tests` records that PID in
`.grpc-server.pid` and tears the server down from an `EXIT INT TERM` trap, where
`grpc_server_stop` already escalates `kill` → 10 s wait → `kill -9` **on that PID
only**. A blanket `lsof -ti:8000 | xargs kill -9` kills every listener on the
machine — including other agents' servers and any unrelated service on that port.
Do not do it.

**Choose your own ports instead of contending for the defaults:**

```bash
# gRPC port the server binds. Read by scripts/run-tests (GRPC_PORT, default 8000).
export GRPC_PORT=8000
# Where test_ucs connects. Precedence: --endpoint > UCS_ENDPOINT > saved defaults
# > localhost:8000. It is NOT derived from GRPC_PORT — set it or they diverge.
export UCS_ENDPOINT="localhost:${GRPC_PORT}"
# Prometheus metrics port: config/development.toml [metrics] port = 8080.
# run-tests does NOT parameterise it, so two concurrent runs collide on 8080 even
# with different GRPC_PORTs. It is inherited by the server child process.
export CS__METRICS__PORT=8080
```

**If the port is genuinely stuck, kill only the PID this workflow started:**

```bash
if [ -f .grpc-server.pid ]; then kill -9 "$(cat .grpc-server.pid)" 2>/dev/null || true; rm -f .grpc-server.pid; fi
```

**Run with timeout (5-10 minutes per connector):**

```bash
# `UCS_TEST_TIMEOUT` is read by NOTHING in this repo — do not set it.
# The per-request budget the harness really reads is UCS_GRPC_TIMEOUT_SECS
# (default 30), passed to grpcurl as `-max-time`
# (crates/internal/integration-tests/src/harness/scenario_api.rs).
export UCS_GRPC_TIMEOUT_SECS=60

# The wall-clock budget for the whole run is yours to enforce:
timeout 600 test-prism --connector {CONNECTOR} --interface {TEST_MODE} --report
```

`test-prism` is `scripts/run-tests` (installed on PATH by
`scripts/setup-connector-tests.sh`); it wraps
`cargo run -p integration-tests --bin test_ucs`. `make test-connector connector=X`
and `make test-scenario connector=X suite=Y scenario=Z` run the same binary and
honour the port on **both** sides: the Makefile declares `GRPC_PORT ?= 8000` and
passes `CS__SERVER__PORT=$(GRPC_PORT)` to the server it starts and
`--endpoint localhost:$(GRPC_PORT)` to the client. Because it is `?=`, an exported
`GRPC_PORT` and a command-line `GRPC_PORT=9090` both win. What the make targets do
NOT read is `UCS_ENDPOINT` — they always pass an explicit `--endpoint`, which takes
precedence over it. So either

```bash
make test-connector connector={CONNECTOR} GRPC_PORT=9090
```

or drive `test-prism` directly with `--endpoint` / `UCS_ENDPOINT` as above.

**Or for a specific suite:**

```bash
# IMPORTANT: Suite names use FORWARD SLASHES, not underscores
# WRONG: test-prism --connector nmi --suite PaymentService_Authorize
# RIGHT: test-prism --connector nmi --suite PaymentService/Authorize
test-prism --connector {CONNECTOR} --suite PaymentService/Authorize
```

**Capture the full output** — save test results for analysis.

**Read the latest report carefully:**

- `crates/internal/integration-tests/report.json` accumulates entries across runs
- Always inspect the latest block for the scenario you just reran (usually near the end of the file)
- Do NOT justify a fix from an older matching scenario block
- **Markdown reports** are generated at `crates/internal/integration-tests/test_report/connectors/{connector}/` — these provide human-readable summaries with exact request/response pairs for each scenario. The filename is the suite name run through `sanitize_anchor` (`harness/report.rs`): lowercased, every non-alphanumeric run collapsed to a single `-`. So `PaymentService/Authorize` becomes `paymentservice-authorize.md`
- **To debug failures**, examine:
  1. The markdown report in `test_report/connectors/{connector}/<sanitized-suite>.md` for request/response details
  2. The JSON report for raw proto payloads
  3. The connector transformer code in `crates/integrations/connector-integration/src/connectors/{connector}/` to understand expected fields

**View results in UI:**

- Web: https://hyperswitch-prism-testing.netlify.app/
- Latest JSON: https://integ.hyperswitch.io/connector-service/reports/grpc/report_latest.json

---

## Phase 2: Analyze Results

**Research the failure before classifying it. Do NOT guess.**

Use this checklist for every failing scenario:

1. Read the **latest** matching block in `crates/internal/integration-tests/report.json`.
2. Inspect the connector's **effective request** with:

```bash
UCS_DEBUG_EFFECTIVE_REQ=1 test-prism --connector {CONNECTOR} --interface {TEST_MODE} --report
```

3. Compare the base scenario with the connector override:
   - Base: `crates/internal/integration-tests/src/global_suites/<Service>_<Flow>/scenario.json`
     (directory name = the suite name with `/` replaced by `_`, e.g.
     `PaymentService_Authorize/scenario.json`; `suite_spec.json` sits beside it)
   - Override: `crates/internal/integration-tests/src/connector_specs/{CONNECTOR}/override.json`
4. Read the harness docs/code that explain what the runner really sends:
   - `crates/internal/integration-tests/docs/connector-overrides.md`
   - `crates/internal/integration-tests/docs/code-walkthrough.md`
   - `crates/internal/integration-tests/src/harness/connector_override/mod.rs`
   - `crates/internal/integration-tests/src/harness/scenario_api.rs`
5. Read the connector implementation to identify:
   - required request fields sourced from scenario input
   - fields sourced from creds/config/header generation instead of `override.json`
   - explicitly unsupported payment methods / flows
   - strict response parsing that can fail after the request is sent
6. Use local reference material when available:
   - integration-test docs/files under `crates/internal/integration-tests/docs/*`, `src/*`, `README.md`, `TESTING_PLAN.md`, `test_suite.sh`

**Important facts while analyzing:**

- `override.json` is merged into the effective `grpc_req` **before** execution.
- `test-prism --interface grpc` sends the harness-built request through gRPC/grpcurl.
- You may add request fields via `override.json` **only if** they are valid in the proto request shape and sourced from scenario input.
- `override.json` cannot directly fix:
  - connector config / creds-derived fields
  - generated headers or request-reference IDs
  - connector code branches that return an `IntegrationError` refusal —
    `NotSupported` / `NotImplemented` / `FlowNotSupported` /
    `CaptureMethodNotSupported` / `CurrencyNotSupported`
    (`crates/types-traits/domain_types/src/errors.rs`)
  - response deserialization mismatches after the connector responds
  - harness/core dependency propagation bugs
- Downstream suites (`Capture`, `Get`, `Refund`, `Void`) can use authorize-derived IDs **only if** authorize succeeded and the prior response exposes the ID in a path the harness can reuse. If authorize fails, omits the ID, or returns it in an unmapped shape, later suites will still fail with missing transaction/refund IDs.
- Treat known connector behavior from references as real evidence. Example: Payload duplicate-sensitive flows need a delay window; NMI SetupMandate must be exactly zero amount.

### If ALL tests pass:

- Result: **HARDENED**
- The connector is now fully tested and can move to "Tested" status in docs

### Debugging Failed Tests - Where to Look

When tests fail, follow this investigation order:

1. **Check Markdown Reports** (immediate visibility):

   ```bash
   # Find the connector's markdown report
   ls -la crates/internal/integration-tests/test_report/connectors/{CONNECTOR}/

   # Read specific suite report
   cat crates/internal/integration-tests/test_report/connectors/{CONNECTOR}/paymentservice-authorize.md
   ```

   These reports show:
   - Exact gRPC request sent
   - Exact gRPC response received
   - Assertion failures with field-level detail

2. **Check JSON Report** (raw payloads):

   ```bash
   cat crates/internal/integration-tests/report.json | jq '.[-10:]'
   ```

   Shows the raw proto serialization for debugging override issues

3. **Check Connector Code** (why request fails):

   ```bash
   # Find connector transformer
   ls crates/integrations/connector-integration/src/connectors/{CONNECTOR}/

   # Read the transformer to understand expected fields
   cat crates/integrations/connector-integration/src/connectors/{CONNECTOR}/transformers.rs
   ```

   Look for:
   - Required fields that must come from scenario input (vs creds/config)
   - Explicit refusal branches — any `IntegrationError::NotSupported` /
     `NotImplemented` / `FlowNotSupported` construction, or the flow appearing in this
     connector's `macro_connector_flow_status_impls!` `not_implemented` / `not_supported` list
     in `{CONNECTOR}.rs`
   - Payment method type checking that rejects certain PMs
   - Response parsing that can fail if connector returns unexpected shape

4. **Debug Request with Effective Req**:

   ```bash
   UCS_DEBUG_EFFECTIVE_REQ=1 test-prism --connector {CONNECTOR} --suite {SUITE} --report
   ```

   Shows exactly what the harness builds before sending to gRPC server

5. **Check Override JSON** (verify test data):
   ```bash
   cat crates/internal/integration-tests/src/connector_specs/{CONNECTOR}/override.json
   ```
   Compare against base scenario to ensure override is valid

### If tests FAIL:

**AUTOMATIC FIX TRIGGER — NO EXCEPTIONS:**

> **If you identify ANY test bug, you MUST FIX IT IMMEDIATELY. NO EXCEPTIONS. No reporting until all fixable bugs are fixed and tests rerun.**
>
> - Found = "FIX" → IMMEDIATELY fix → RERUN → Verify → Then proceed
> - Do not list the bug and leave it. Do not "note for later." DO IT NOW.
> - If you find test bugs and DONT immediately fix them, you FAILED.
> - **Finding a test bug without fixing it = CRITICAL FAILURE. Report only after fixing.**
> - There is no "later" or "after thinking about it." FIX. NOW.

**ENFORCEMENT — You are inside the loop until tests pass:**

- **If tests FAIL, you are STILL inside the loop. You MUST fix test bugs, rerun tests, and verify. You CANNOT return a final status yet. Returning HARDENED after failures without attempting fixes is a CRITICAL VIOLATION.**
- **If you find yourself about to report without fixing test bugs first → STOP → Fix them → Rerun → Verify → Only THEN return final status**
- **Do NOT report failures without attempting fixes.** An error means something can be fixed — not that reporting is the next step. Fix → Rerun → Verify.
- **You MUST make a test change between retries.** Never rerun tests without changing test data. No change = same result = STOP → return FAILED.

**MANDATORY SEQUENCE:**

1. **First:** Identify fixable issues (test data, credentials)
2. **Second:** For each fixable issue → FIX IT in override.json/creds.json → RERUN tests
3. **Third:** Only after reruns pass → return HARDENED
4. **Fourth:** Only if CANNOT fix → return FAILED with evidence of attempted fixes

- If test fails due to test data (positive override) → FIX IT NOW, don't ask
- If test fails due to connector code bug → FAILED (report)
- If test fails due to framework bug → REPORT_TO_MASTER (stop)
- **NEVER present options to user — NEVER ask "do you want me to"**
- **If you find yourself about to ask a question, STOP and fix it instead**

**Determine failure type:**

**Before classifying a failure, verify the actual suite entrypoint and request shape:**

- Confirm whether the failing suite is a standalone entrypoint (for example `PaymentService/CreateOrder`) or a scenario dependency
- Do NOT assume `CreateOrder`, `Authorize`, or session-token flows are chained together unless the scenario explicitly wires them together
- Check the request payload before deciding what should exist in the response; for example, payment-method-specific fields may be the reason `session_data` appears in one scenario and not another
- For missing required fields, verify whether the field comes from:
  - scenario request data (`override.json` can help), or
  - connector config / generated headers / request-reference plumbing (`override.json` cannot help)
- For missing `connector_transaction_id`, verify the upstream authorize/latest dependency response before changing a downstream scenario. Missing downstream IDs are often symptoms of an earlier failure, not independent override bugs.

1. **Test Bug — POSITIVE Override Issue (FIX):**
   - Test uses wrong field names → fix the test data
   - Missing required fields in test data → add field
   - Test assertion logic is wrong → fix assertion to match expected behavior
   - Missing connector config in test → add config
   - **Key: The fix makes the test correct, not just asserts failure**
   - Removing an invalid success-only assertion is allowed only when the real failing payload remains visible after the change
   - Widening a status assertion is allowed only when the latest report block AND connector/UCS mapping both support that status
   - **→ FIX IMMEDIATELY, DO NOT WAIT → RERUN → THEN proceed**
   - **→ FIX NOW, proceed to Phase 3 immediately**

2. **Test Bug — NEGATIVE Override Issue (DO NOT FIX):**
   - Just assert the test to fail to make it pass
   - This is wrong — do NOT do this, report as FAILED

3. **Credentials Issue (FIX):**
   - The creds.json format is incorrect
   - Not a code bug — creds just need correct structure
   - **→ Fix creds.json to match what connector expects**

4. **Real Bug (NOT your job to fix):**
   - Connector implementation has actual bugs
   - API behavior changed on connector side
   - Missing required connector setup (merchant config, etc.)
   - **→ STOP, report to master, do NOT fix connector code**

5. **Payment Method Not Supported (REPORT_TO_MASTER):**
   - Payment method not implemented in connector
   - Flow not supported by connector API
   - **→ STOP, report to master that PM is not implemented**

6. **UCS Code Bug (NOT your job to fix):**
   - Bug requires change to connector implementation code
   - Requires change to testing framework core (harness, global_suites)
   - **→ STOP, report to master, do NOT modify codebase**

**For POSITIVE Override Test Bugs** → Proceed to Phase 3
**For Credentials Issues** → Fix creds.json, rerun tests
**For NEGATIVE Override** → Result: **FAILED** (report, don't fix)
**For Real Bugs** → Result: **FAILED** (report, don't fix connector)
**For Payment Method Not Supported** → Result: **REPORT_TO_MASTER** (notify not implemented)
**For UCS Code Bugs** → Result: **REPORT_TO_MASTER** (stop, notify)

---

## Phase 3: Fix Positive Override Issues

**GUARDRAILS (STRICT):**

- ✅ DO: Fix test data, assertions, field names (positive overrides)
- ❌ DO NOT: Touch UCS core code (`crates/integrations/connector-integration/`)
- ❌ DO NOT: Touch testing framework core code ( harness, global_suites)
- ❌ DO NOT: Create negative overrides (assert failure to pass)
- ❌ DO NOT: Fix bugs in connector implementation code
- **If bug is in UCS code or requires testing framework core change → STOP, report to master, do NOT modify**

**Create a fix branch:**

```bash
git checkout -b fix/test-{connector}-{issue}
```

**Fix the test files (ONLY positive overrides):**

- Location: `crates/internal/integration-tests/src/connector_specs/{CONNECTOR}/`
- Allowed to edit:
  - `override.json` — test data overrides
  - `specs.json` — connector-specific specs
  - Scenario JSON files in the connector spec folder

**How to update `override.json` safely:**

1. Start from the base global scenario and patch only the connector-specific delta.
2. Follow `crates/internal/integration-tests/docs/connector-overrides.md` exactly:
   - key shape is `suite -> scenario -> { grpc_req, assert }`
   - `grpc_req` uses JSON Merge Patch semantics
   - `null` removes a key
3. Prefer **leaf-field** edits over replacing whole nested objects.
4. Use `crates/internal/integration-tests/src/connector_specs/stripe/override.json` as the reference style.
5. Do not use `override.json` to hide a real connector failure. The request must remain truthful.
6. If the connector code shows the field comes from creds/config/header generation, stop — that is **not** an override fix.
7. If the connector code explicitly rejects the PM/flow, stop — that is **not** an override fix.

**Validate override changes before rerunning the connector.** These two are the sanctioned
exception to the workflow's "never run `cargo test`" rule (`1_orchestrator.md` Rule 3) — they
validate manifest data against the proto schema and run no connector code:

```bash
cargo test -p integration-tests all_supported_scenarios_match_proto_schema_for_all_connectors
cargo test -p integration-tests all_override_entries_match_existing_scenarios_and_proto_schema
```

**Verify fix:**

```bash
test-prism --connector {CONNECTOR} --report
```

**If tests now pass:**

- Result: **HARDENED**
- Commit fix — stage an EXPLICIT pathspec. Never `git add -A`: this working tree
  carries untracked GRACE reports, worktrees and scratch files that would be swept in.

  ```bash
  git add crates/internal/integration-tests/src/connector_specs/{CONNECTOR}/
  git commit -m "fix({CONNECTOR}): fix positive override test bug in {description}"
  ```

  **Never stage the creds file.** A credentials repair is a local-only change: it
  stays in the working tree and is never committed. `creds.json` is gitignored
  (`.gitignore`), so `git add creds.json` fails outright with
  `The following paths are ignored by one of your .gitignore files` — do not reach
  for `-f`. The same holds for whatever `$CREDS` resolves to and for any `.env`
  file. Do NOT stage `crates/internal/integration-tests/report.json` or
  `crates/internal/integration-tests/test_report/` either — those are run
  artifacts (`report.json` is gitignored; `test_report/` is not, so it *can* be
  swept in by a careless add).
- Push: `git push -u origin fix/test-{connector}-{issue}`

**MANDATORY: After pushing, CREATE PR:**

```
# Create PR after pushing
gh pr create --title "fix({CONNECTOR}): positive override test fixes" --body "- Test fixes applied for {connector}" --repo juspay/hyperswitch-prism

# If the outcome is unsure, do NOT reach for --draft (see below). Open a normal
# PR and say so in the title SUFFIX and in a label:
gh pr create --title "fix({CONNECTOR}): test fixes [WIP]" \
  --label "GRACE" --body "- Test fixes applied for {connector}" \
  --repo juspay/hyperswitch-prism

# Add a label — the label is spelled "GRACE" (that is the name 2.4_pr.md creates
# on the repo). `gh pr label add` is NOT a real command.
# The label subcommand does not exist; use `gh pr edit`:
gh pr edit <number|url|branch> --add-label "GRACE" --repo juspay/hyperswitch-prism

# …or set it at creation time with `gh pr create -l/--label`:
#   gh pr create --label "GRACE" --title "…" --body "…" --repo juspay/hyperswitch-prism
```

**On `--draft`:** the flag is real, but every meaningful job in
`.github/workflows/ci.yml` is guarded by `!github.event.pull_request.draft` —
`typos` (Spell check), `clippy`, `auto-fix`, `check` (Compilation Check), `test`
(Run Tests) and `sdk-test` — so a draft PR skips all of them, while the `CI Result`
gate (`ci-gate`, `if: always()`) still reports green: it fails only on
`failure`/`cancelled`, never on `skipped`. A draft PR is a green PR that proved
nothing. Prefer a non-draft PR with a label, or run `gh pr ready <number>` before
treating any CI result as signal. Also note the PR **title** is checked by
`cog verify` (`.github/workflows/pr-convention-checks.yml`), so a marker must be a
SUFFIX: `fix({CONNECTOR}): test fixes [WIP]` parses, a `[…]` prefix does not, and
`wip:` / `draft:` are not among the commit types `cog.toml` allows (feat, fix,
perf, refactor, test, docs, proto, chore, build, revert, ci).

**If still failing:**

- Result: **FAILED** (could not fix)
- Revert: `git checkout {BRANCH}` to return to working branch

---

## Phase 4: Report

**Return result:**

| Field      | Value                       |
| ---------- | --------------------------- | ------ | ------- | ---------------- | ----------------- |
| CONNECTOR  | {connector}                 |
| STATUS     | HARDENED                    | FAILED | SKIPPED | REPORT_TO_MASTER | CREDENTIALS_FIXED |
| REASON     | {explanation}               |
| FIX_COMMIT | {commit hash if applicable} |

---

## Notes

- **ALWAYS check creds first** — no creds = SKIPPED
- **Set timeout** — `UCS_GRPC_TIMEOUT_SECS` (per gRPC request, default 30) plus a
  wall-clock `timeout 600 …` around the run. `UCS_TEST_TIMEOUT` is read by nothing
- **If server readiness fails, inspect both `$GRPC_PORT` and `$CS__METRICS__PORT`** — the process binds two sockets (gRPC + Prometheus metrics), and a stale metrics listener on 8080 looks like a gRPC-port problem. Scope any kill to `.grpc-server.pid`, never to a port
- **Positive overrides only** — fix assertions, not just assert failure
- **NEVER touch UCS core** — only test files
- **NEVER touch framework core** — only connector_specs/
- Test credentials must exist in `creds.json` or connector will be skipped
- Use `UCS_DEBUG_EFFECTIVE_REQ=1` to debug request payloads

---

## Generic Investigation Tips

- **When unsure about a flow** for a specific connector, check the connector implementation under `crates/integrations/connector-integration/src/connectors/{CONNECTOR}/` and the connector's integration docs. If needed, pull up the original integration PR for that connector — the PR description usually has reference cURLs / grpcurls and expected request/response shapes you can diff against what the harness is producing.

- **Confirm the creds file the harness actually reads.** `.env.connector-tests` at the repo root can override `UCS_CREDS_PATH`. Read it before editing `creds.json` — otherwise your edits land in the wrong file.

- **Creds must use the flat proto-native shape.** The harness rejects any creds block containing `connector_account_details` with `LegacyFormat`. Fields map directly to the proto config message — read `crates/types-traits/grpc-api-types/proto/payment.proto` for the connector's `*Config` message to know the expected keys.

- **If outbound request bodies are masked in logs** (e.g. `*** alloc::string::String ***`) and you need to compare against a known-good payload, add a temporary `tracing::error!` at the encoding boundary inside the connector's transformer to dump the unmasked string. Revert before committing.

- **Short reference-ID constraints** — when the connector caps the order/reference id length, declare it in the connector's `specs.json` via `request_id_source_field` + `request_id_prefix` + `request_id_length`. The harness will generate a unique short id and write it into the proto body's source field (only when that field exists in the suite's `scenario.json`).

- **Card detail combinations matter to the test issuer.** Some sandboxes reject specific expiry / cvc / holder-name combinations even when the card number is on the documented list. Always check the connector's integration PR for the exact triples that were verified to work — and use those in the override rather than base-spec defaults.

- **Capture/Void cascade is by design.** Those suites depend on `no3ds_manual_capture_credit_card` (you can't capture/void an auto-captured payment). If Capture is failing but Refund passes, look at the upstream `manual_capture` authorize first — that is what the cascade needs.

- **Connectors that need upstream context** (e.g. a 3DS pre-step before Authorize) — there is no per-connector way to declare one. An `additional_dependencies` map in `specs.json` is a plausible-sounding invention: no such field exists on `ConnectorSuiteSpec` (`crates/internal/integration-tests/src/harness/scenario_types.rs`), no connector under `connector_specs/` sets it, and the runner reads dependencies only from the global `suite_spec.json` (`target_suite_spec.depends_on` in `harness/scenario_api.rs`). `ConnectorSuiteSpec` is a plain `Deserialize` with no `deny_unknown_fields`, so such a block is silently dropped at load time rather than rejected — you get no error and no behaviour change. `crates/internal/integration-tests/README.md` says exactly this: unknown keys are dropped, and "dependencies between suites are declared in the global `suite_spec.json` (`depends_on`), never per connector." Treat missing upstream context as a suite-spec / harness matter → REPORT_TO_MASTER, not an `override.json` fix.

- **Browser-driven 3DS testing** — refer to `connector_specs/stripe/browser_automation_spec.json` for the reference shape. Some connectors expose an SCA-exemption path that lets you skip the Device Data Collection iframe and drive only the ACS challenge UI — check the connector implementation and integration PR before assuming full DDC automation is required.
