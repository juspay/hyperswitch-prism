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

**Run with timeout (5-10 minutes per connector):**

```bash
# Set timeout
export UCS_TEST_TIMEOUT=600  # 10 minutes

# Run all test suites for the connector
test-prism --connector {CONNECTOR} --interface {TEST_MODE} --report
```

**Or for a specific suite:**

```bash
test-prism --connector {CONNECTOR} --suite authorize
```

**Capture the full output** — save test results for analysis.

**View results in UI:**

- Web: https://hyperswitch-prism-testing.netlify.app/
- Latest JSON: https://integ.hyperswitch.io/connector-service/reports/grpc/report_latest.json

---

## Phase 2: Analyze Results

### If ALL tests pass:
- Result: **HARDENED**
- The connector is now fully tested and can move to "Tested" status in docs

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

1. **Test Bug — POSITIVE Override Issue (FIX):**
   - Test uses wrong field names → fix the test data
   - Missing required fields in test data → add field
   - Test assertion logic is wrong → fix assertion to match expected behavior
   - Missing connector config in test → add config
   - **Key: The fix makes the test correct, not just asserts failure**
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

- Location: `testing/integration-tests/src/connector_specs/{CONNECTOR}/`
- Allowed to edit:
  - `override.json` — test data overrides
  - `specs.json` — connector-specific specs
  - Scenario JSON files in the connector spec folder

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
- **Positive overrides only** — fix assertions, not just assert failure
- **NEVER touch UCS core** — only test files
- **NEVER touch framework core** — only connector_specs/
- Test credentials must exist in `creds.json` or connector will be skipped
- Use `UCS_DEBUG_EFFECTIVE_REQ=1` to debug request payloads
