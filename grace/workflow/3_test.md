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

**FIRST: Verify connector has credentials in `creds.json`:**

```bash
cat creds.json | jq '.${CONNECTOR}'
```

**If NO credentials:**

- Result: **SKIPPED**
- Reason: "No credentials in creds.json"
- Stop here — do NOT run tests

---

## Phase 1: Run Tests

**Before the first run (and again on any readiness failure), clear stale listeners on BOTH ports:**

```bash
# test-prism waits for gRPC on 8000, but stale listeners on 8080 can also kill startup
lsof -ti:8000 | xargs kill -9 2>/dev/null || true
lsof -ti:8080 | xargs kill -9 2>/dev/null || true
```

**Run with timeout (5-10 minutes per connector):**

```bash
# Set timeout
export UCS_TEST_TIMEOUT=600  # 10 minutes

# Run all test suites for the connector
test-prism --connector {CONNECTOR} --interface {TEST_MODE} --report
```

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
- **Markdown reports** are generated at `crates/internal/integration-tests/test_report/connectors/{connector}/` — these provide human-readable summaries with exact request/response pairs for each scenario
- **To debug failures**, examine:
  1. The markdown report in `test_report/connectors/{connector}/{suite}.md` for request/response details
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
   - Base: `crates/internal/integration-tests/src/global_suites/<suite>_suite/scenario.json`
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
  - connector code branches that return `NotSupported` / `NotImplemented`
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
   - Explicit `NotSupported` or `NotImplemented` branches
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
- ❌ DO NOT: Touch UCS core code (`crates/connector-integration/`)
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

**Validate override changes before rerunning the connector:**

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
- Commit fix: `git add -A && git commit -m "fix({CONNECTOR}): fix positive override test bug in {description}"`
- Push: `git push -u origin fix/test-{connector}-{issue}`

**MANDATORY: After pushing, CREATE PR:**

```
# Create PR after pushing
gh pr create --title "fix({CONNECTOR}): positive override test fixes" --body "- Test fixes applied for {connector}" --repo juspay/hyperswitch-prism

# If outcome is unsure → Create DRAFT PR
gh pr create --draft --title "fix({CONNECTOR}): test fixes [WIP]" --repo juspay/hyperswitch-prism

# Add label
gh pr label add "grace" --repo juspay/hyperswitch-prism
```

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
- **Set timeout** — 5-10 minutes per connector
- **If server readiness fails, inspect both 8000 and 8080** — stale metrics listeners can look like a gRPC 8000 problem
- **Positive overrides only** — fix assertions, not just assert failure
- **NEVER touch UCS core** — only test files
- **NEVER touch framework core** — only connector_specs/
- Test credentials must exist in `creds.json` or connector will be skipped
- Use `UCS_DEBUG_EFFECTIVE_REQ=1` to debug request payloads
