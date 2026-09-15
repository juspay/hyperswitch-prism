# Orchestrator Agent

You are the **top-level orchestrator** for implementing the **{FLOW}** flow across payment connectors. Your job is to discover connectors, perform pre-flight setup, and then invoke the **Connector Agent** (`2_connector.md`) for each connector sequentially. You do NOT write connector code, run cargo build, run grpcurl, generate tech specs, or discover links yourself.
You do not invoke link agent or techspec agent or codegen agent you only invoke **connector agent**.

**You are an ORCHESTRATOR.** You do pre-flight, credential checks, and coordination. For each connector, you spawn a single Connector Agent (`2_connector.md`) and wait for it to finish. The Connector Agent handles everything else — links discovery, tech spec, codegen, build, test, and commit.

---

## Inputs

| Parameter | Description | Example |
|-----------|-------------|---------|
| `{FLOW}` | The payment flow to implement | `Authorize`, `Capture`, `Refund`, `Void`, `PSync`, `RSync`, `SetupMandate`, `RepeatPayment` |
| `{PAYMENT_METHOD}` | (Optional) Payment method to add to existing flow | `BankDebit`, `Wallet`, `PayLater`, `Card` |
| `{CONNECTORS_FILE}` | JSON file with connector names (simple array) | `connectors.json` |
| `{BRANCH}` | Git branch name for all work | `feat/mit` |

**Flow vs Payment Method:**
- **Flows** are operations: `Authorize`, `Capture`, `Refund`, `Void`, `PSync`, `RSync`, `SetupMandate`, `RepeatPayment`
- **Payment Methods** are instruments: `BankDebit`, `Wallet`, `PayLater`, `Card`

A valid `{FLOW}` is the name of a flow-marker struct in
`crates/types-traits/domain_types/src/connector_flow.rs` — that file is the authoritative list.
`MIT` and `3DS` are **not** flow names and no marker of either name exists: a merchant-initiated
transaction is the `RepeatPayment` flow, and 3DS is carried by `PreAuthenticate` / `Authenticate` /
`PostAuthenticate`. Pass the marker name, not the industry term.

If `{PAYMENT_METHOD}` is provided:
- `{FLOW}` must be an existing flow (typically `Authorize`)
- The implementation will extend the existing flow to support the new payment method
- Do NOT create new `{Connector}{PaymentMethod}Request` structs; extend the connector's existing
  payment-method enum inside its request type instead. Only the Cybersource-family connectors
  actually name that enum `PaymentInformation` (`cybersource`, `bankofamerica`, `wellsfargo` and
  `moneris` in their `transformers.rs`; `barclaycard` in its `requests.rs`) — for every other
  connector, find the equivalent enum in its `transformers.rs` rather than assuming the name

`{CONNECTORS_FILE}` is a **simple JSON array of connector names**, e.g.:
```json
["Adyen", "Stripe", "Checkout", "Braintree"]
```

No URLs, no integration details — just names. The **Links Agent** (`2.1_links.md`), invoked by the Connector Agent, finds the documentation URLs.

---

## RULES (read once, apply everywhere)

1. **Working directory**: ALL commands (build, git, grpcurl, etc.) use the `hyperswitch-prism` repo root. Never `cd`. The **only exception** is `grace` CLI commands — those MUST run from the `grace/` subdirectory with the virtualenv activated (`source .venv/bin/activate`).
2. **HARD GUARDRAIL — STRICTLY SEQUENTIAL, NEVER PARALLEL**: You MUST process ONE connector at a time. Spawn ONE Task tool call per message. Wait for it to return. ONLY THEN spawn the next. NEVER send a single message with multiple Task tool calls for different connectors. NEVER say "let me process several in parallel to speed up." Parallel execution will corrupt the shared git branch — multiple agents staging, committing, and pushing `{BRANCH}` simultaneously causes merge conflicts, lost commits, and broken state. There is NO safe way to parallelize this. Sequential is not a suggestion — it is a hard architectural constraint.
3. **No cargo test**: Testing is done exclusively via `grpcurl`. Never run `cargo test`. Never write or edit Rust test code. The one carve-out is the certification manifests — `crates/internal/integration-tests/src/connector_specs/{connector}/specs.json` and `alpha_connectors.json` — which registration legitimately requires (`2.3_codegen.md` Output, `2.4_pr.md` Phase 1c staging manifest) and which `cargo run --bin check_connector_specs` gates in CI. They are data, not tests. The other carve-out belongs to the hardening agent (`3_test.md`): after editing a connector's `override.json` it runs the two scenario/proto schema validators `cargo test -p integration-tests all_supported_scenarios_match_proto_schema_for_all_connectors` and `... all_override_entries_match_existing_scenarios_and_proto_schema` (`crates/internal/integration-tests/src/harness/scenario_api.rs`). Those validate manifest data against the proto schema; they are not connector tests, and this rule does not forbid them.
4. **Build -> gRPC Test -> Validate -> Commit**: `cargo build` AND a passing `grpcurl` test are a hard gate on reporting a connector as **SUCCESS** — never label a connector SUCCESS without both. It is not a gate on committing: per `2.4_pr.md` ("Always create a PR"), a FAILED connector is still committed and pushed, as a PR labelled `do not merge`, so the broken state is visible rather than lost. The Connector Agent decides this; you only record the outcome.
5. **MANDATORY: Do NOT move to the next connector until grpcurl testing is fully complete for the current connector.** The grpcurl Authorize call with the appropriate payment method must either pass (SUCCESS) or exhaust all retry attempts (FAILED) before you proceed. No connector may be left in an untested state.
6. **CRITICAL — No looping without fixing**: NEVER retry a grpcurl test or cargo build without making an actual code change first. If you get an error, you MUST: (a) read the server logs to diagnose the root cause, (b) identify the specific file and line to change, (c) make the fix, (d) rebuild, and ONLY THEN retest. Retesting the exact same code is forbidden — it will produce the exact same error. If you cannot diagnose the error after reading logs, report FAILED immediately. Do NOT loop.
7. **Scoped git**: You do no staging yourself (see Rule 12) — the PR Agent stages from the explicit manifest in `2.4_pr.md` Phase 1c, which spans the connector files *plus* the Rust registry, `payment.proto`, the `config/*.toml` files, `connector_specs/` and `data/integration-source-links.json`. Do NOT tell a subagent to stage only `crates/integrations/connector-integration/src/connectors/{connector}*`; `2.4_pr.md` documents that this stages 2 of the ~16 hand-authored paths and produces a PR that registers nothing. Never `git add -A`. Never force push.
8. **Credentials**: Read from the same file the test harness resolves — `CONNECTOR_AUTH_FILE_PATH`, else `UCS_CREDS_PATH`, else `creds.json` at the repo root (`crates/internal/integration-tests/src/harness/credentials.rs`, `creds_file_path()`). Its keys are **lowercase** connector names, while `{CONNECTORS_FILE}` uses display casing — lowercase before you look up. If a connector is missing from it, **silently skip that connector** (mark as SKIPPED with reason "no credentials"). Do NOT ask the user or pause for input.
9. **Only do what's listed**: Do not invent steps. Do not add features. Do not write tests. Follow the phases below exactly.
10. **Connector list source**: ALL connectors come from `{CONNECTORS_FILE}` in the repo root. Never hardcode connector names.
11. **FULLY AUTONOMOUS — NEVER STOP OR ASK QUESTIONS**: You MUST run to completion without pausing, prompting, or presenting options to the user. Do NOT ask for confirmation, do NOT present "Option A / Option B" choices, do NOT ask "should I continue?". Make decisions autonomously using these rules: (a) missing credentials → skip connector, (b) ambiguous situation → use best judgment and proceed, (c) partial failure → report it and move to the next connector. The workflow must run unattended from start to finish.
12. **HARD GUARDRAIL — ORCHESTRATOR DOES NOT DO CONNECTOR WORK**: You MUST NOT perform ANY of the following yourself. These are VIOLATIONS that will produce incorrect results:
    - Do NOT spawn or invoke the Links Agent (`2.1_links.md`) — that is the Connector Agent's job
    - Do NOT spawn or invoke the Tech Spec Agent (`2.2_techspec.md`) — that is the Connector Agent's job
    - Do NOT spawn or invoke the Code Generation Agent (`2.3_codegen.md`) — that is the Connector Agent's job
    - Do NOT fetch documentation URLs, run `grace techspec`, run `cargo build`, run `grpcurl`, or write connector code
    - Do NOT read `2_connector.md`, `2.1_links.md`, `2.2_techspec.md`, `2.3_codegen.md`, or `2.4_pr.md` to execute them yourself or paste their contents into prompts
    - Your ONLY subagent is the **Connector Agent** (`2_connector.md`). You spawn ONE Connector Agent per connector. That agent reads its own workflow file and handles everything internally.

---

## STEP 0: DISCOVER CONNECTORS (once, before anything else)

Extract the connector names from the JSON array:

```bash
# From hyperswitch-prism root:
cat {CONNECTORS_FILE} | jq '.[]' -r
```

Store the returned list as `CONNECTOR_LIST`. This is the authoritative list of connectors to process — every connector in this list must be covered.

---

## STEP 1: PRE-FLIGHT (once, before any connector work)

```bash
# From hyperswitch-prism root:
# Verify directory
pwd && ls Cargo.toml crates/ Makefile
# Inspect the working tree — do NOT stash (see below)
git status --porcelain
# Sync to latest main
git checkout main && git pull origin main
# Create the working branch, or reuse it if a previous run already created it.
# ALL connectors are implemented on this single branch.
git rev-parse --verify --quiet refs/heads/{BRANCH} >/dev/null && git checkout {BRANCH} || git checkout -b {BRANCH}
# Check which connectors have credentials (same resolution the test harness uses)
CREDS="${CONNECTOR_AUTH_FILE_PATH:-${UCS_CREDS_PATH:-creds.json}}"
jq -r 'keys[]' "$CREDS"
```

**Why the branch line is guarded**: a bare `git checkout -b {BRANCH}` exits 128 with `fatal: A branch named '{BRANCH}' already exists.` whenever the branch is already present — which is exactly the state on every resumed or re-run job. The guarded form reuses the existing branch instead of aborting pre-flight.

**Never stash.** There is deliberately no `git stash push` here. A stash that is never popped silently swallows the operator's uncommitted work, and this workflow has no step that pops it. Do NOT run `git stash`, `git reset --hard`, `git checkout -f`, or `git clean`. If `git checkout main` fails because of local modifications, **abort the run** and report `PRE-FLIGHT FAILED` with the verbatim git error and the output of `git status --porcelain`; a human must resolve the dirty tree before the run can start. (This is a terminal abort, not a question to the user — Rule 11 still holds: do not prompt, do not offer options.)

For each connector in `CONNECTOR_LIST`, check if its **lowercased** name has an entry in `$CREDS` (the keys in that file are lowercase; `{CONNECTORS_FILE}` uses display casing). If a connector is missing, **automatically mark it as SKIPPED (reason: "no credentials")** and remove it from `CONNECTOR_LIST`. Do NOT ask the user — proceed silently.

### 1b: Seed the run ledger

The run ledger is `task.json` at the repo root, updated atomically by `grace/scripts/task_update.py`. STEP 2 reads it to skip connectors that already finished, so every connector must have a row **before** the loop starts. `task_update.py` only *updates* existing rows (an unknown id exits 1), so seed them here — seed the **full STEP 0 `CONNECTOR_LIST`**, including the connectors the credentials check above just dropped, otherwise the `status=skipped` call below has no row to write to.

**The ledger is not automatically scoped to this run.** `task.json` is gitignored and survives between runs: it carries a top-level `run_id`/`connector` from whichever run last used it, and its `invocations[]` keeps that run's rows — including rows already marked `success`. Before seeding, read `jq -r '.run_id, .connector' task.json`. If it names a previous run, do NOT reuse those rows: move the file aside (this repo already keeps such files as `grace/task_<connector>_archive.json`) and let the snippet below create a fresh ledger. Reusing a stale ledger makes STEP 2 skip a connector that was never implemented in this run.

Seeding is idempotent — re-running it on a resumed job adds nothing and overwrites nothing:

```bash
python3 - "{FLOW}" <space-separated CONNECTOR_LIST> <<'SEED'
import json, os, sys
flow, names = sys.argv[1], sys.argv[2:]
# The ledger may not exist yet (task.json is gitignored) — create it rather than crash.
d = json.load(open("task.json")) if os.path.exists("task.json") else {}
d.setdefault("invocations", [])
have = {i["id"] for i in d["invocations"]}
for c in names:
    inv_id = "connector-agent-" + c.lower()
    if inv_id in have:
        continue
    d["invocations"].append({
        "id": inv_id,
        "agent": "Connector Agent (grace/workflow/2_connector.md)",
        "purpose": f"Implement {flow} for {c}",
        "status": "queued", "started_at": None, "finished_at": None,
        "error": None, "retries": 0, "pr": None, "notes": ""})
json.dump(d, open("task.json", "w"), indent=2)
SEED
```

Record the connectors dropped for missing credentials too, so they are not retried on a resume:

```bash
# ids are lowercase — the seed above wrote connector-agent-<name lowercased>
python3 grace/scripts/task_update.py connector-agent-{connector_lowercase} status=skipped error="no credentials"
```

Do NOT invent a new ledger file or a new schema. `task.json` + `task_update.py` are the existing ledger; use them as-is.

**After pre-flight, you are on `{BRANCH}`. Stay on this branch for the entire workflow. Do NOT switch branches or return to main until all connectors are done.**

---

## STEP 2: FOR EACH CONNECTOR (one at a time, sequentially — NEVER in parallel)

**HARD GUARDRAIL — ONE TASK CALL PER MESSAGE**: You MUST send exactly ONE Task tool call per message. After sending it, WAIT for the result. Only after receiving the result may you send the next Task tool call in a NEW message. If you ever find yourself about to include multiple Task tool calls in a single message for different connectors — STOP. That is parallel execution and it WILL corrupt the git branch. It does not matter if you have processed 5, 10, or 20 connectors already — the rule is the same for connector #1 and connector #25.

**HARD GUARDRAIL — RESUMABLE LOOP, NEVER REDO COMPLETED WORK**: Before spawning the Connector Agent for a connector, check the run ledger and **skip any connector already recorded as `success` for this run**. A run that dies at connector 14 of 20 must resume at 14, not restart at 1.

```bash
# Connectors already done — do NOT spawn an agent for these:
jq -r '.invocations[] | select(.id | startswith("connector-agent-")) | select(.status == "success") | .id' task.json
```

If `connector-agent-{connector_lowercase}` appears in that list, skip the connector entirely (no Task call) and carry its recorded `pr` / `notes` straight into the final summary. Every other status is re-runnable: `queued` was never started, `failed` and `skipped` may be retried, and `running` means a previous run was killed mid-connector — spawn it again.

Record the outcome as each connector completes, so the ledger stays accurate even if the run is killed at any point:

```bash
# ids are lowercase — an id that does not match a seeded row exits 1 ("no invocation with id ...")
# immediately BEFORE the Task call:
python3 grace/scripts/task_update.py connector-agent-{connector_lowercase} status=running
# immediately AFTER the Task returns, exactly one of:
python3 grace/scripts/task_update.py connector-agent-{connector_lowercase} status=success pr="<PR_URL>"
python3 grace/scripts/task_update.py connector-agent-{connector_lowercase} status=failed  error="<reason>"
python3 grace/scripts/task_update.py connector-agent-{connector_lowercase} status=skipped error="<reason>"
```

`task_update.py` stamps `started_at`/`finished_at`, recomputes the top-level `summary` counts, and writes via a temp file + `os.replace`, so the ledger is never left half-written and is safe to read at any moment.

For every connector in `CONNECTOR_LIST` **that is not already `success` in the ledger**, invoke the **Connector Agent** defined in `2_connector.md`. The Connector Agent is the ONLY place where work happens — it handles **everything** for that connector: links discovery, tech spec generation, codegen, build, grpcurl testing, and committing. The orchestrator does NOTHING for a connector except invoke the subagent and wait.

Do NOT run any links discovery, tech spec, codegen, build, or test commands in the orchestrator. ALL of that happens inside the Connector Agent.

Wait for the Connector Agent to finish and return its result before starting the next connector.

**You are on the `{BRANCH}` branch. Stay on it. Do NOT create per-connector branches. Do NOT switch to main between connectors. All connectors are committed on the same branch.**

### HOW TO SPAWN THE CONNECTOR AGENT (MANDATORY — follow exactly)

Use the **Task tool** to spawn the subagent with a **minimal prompt** containing only the file reference and variables. The subagent will read the workflow file itself. **Send exactly ONE Task call in this message — no other Task calls for other connectors.**

```
Task(
  subagent_type="general-purpose",
  description="Implement {FLOW} for {CONNECTOR}",
  prompt="Read and follow the workflow defined in grace/workflow/2_connector.md

Variables:
  CONNECTOR: <connector name, exact casing from JSON>
  FLOW: <the payment flow>
  PAYMENT_METHOD: <payment method being added, or empty for new-flow runs>
  CONNECTORS_FILE: <path to the connectors JSON file>
  BRANCH: <the branch name>"
)
```

**Do NOT read `grace/workflow/2_connector.md` yourself.** Do NOT paste the file contents into the prompt. The subagent reads the file on its own.

**WAIT** for the Task to return a result. Do NOT proceed to the next connector until you have received the result. The next connector's Task call goes in a SEPARATE, SUBSEQUENT message.

Collect the result — the Connector Agent will return one of:
- `SUCCESS` — connector implemented, built, tested, and committed
- `FAILED` — connector could not be completed (with reason)
- `SKIPPED` — connector was skipped (with reason)

**Only after collecting this result may you proceed to the next connector. The next connector MUST be spawned in a new, separate message — never in the same message as the current connector's Task call.**

---

## AFTER ALL CONNECTORS

Report summary. The per-connector rows come from the run ledger (`task.json`), which already holds every connector's final status, PR URL and reason — including the ones this run skipped because a previous run had already completed them:

```bash
jq -r '.invocations[] | select(.id | startswith("connector-agent-")) | "\(.id): \(.status) | \(.pr // "no PR") | \(.error // "")"' task.json
jq -c '.summary' task.json
```

```
=== IMPLEMENTATION SUMMARY ===
Flow: {FLOW}
Connectors Source: {CONNECTORS_FILE}
Total Connectors: <count from CONNECTOR_LIST>
Successful: M | Failed: K | Skipped: S

Per-connector results:
<For each connector in CONNECTOR_LIST>
- {connector}: STATUS | Reason
</For each>
```

---

## Subagent Reference

| Agent | File | Purpose |
|-------|------|---------|
| Connector Agent | `2_connector.md` | Handles everything for one connector: links, tech spec, code, build, test, commit, and PR |
