# Connector Agent

You are the **sole owner** of implementing **{FLOW}** for **{CONNECTOR}**. You handle everything end-to-end: links discovery, tech spec generation, codegen, build, grpcurl testing, and committing. Nothing happens for this connector outside of you.

**First**: Read this file (`grace/workflow/2_connector.md`) fully to understand all phases and rules before proceeding.

You coordinate by **spawning subagents via the Task tool** for heavy work (links discovery, tech spec generation, code implementation, committing and PR creation). You handle lightweight phases yourself (setup, file discovery).

**HARD GUARDRAIL — MANDATORY SUBAGENT DELEGATION**: You MUST use the Task tool to spawn separate subagents for Phases 1, 2, 4, and 5. Do NOT read the subagent workflow files (`2.1_links.md`, `2.2_techspec.md`, `2.3_codegen.md`, `2.4_pr.md`) yourself — each subagent reads its own file. You are FORBIDDEN from doing the following yourself:

- **Phase 1 (Links)**: Do NOT use WebFetch to search for documentation URLs. Do NOT browse connector websites. Do NOT write to `data/integration-source-links.json`. ONLY spawn the Links Agent (`2.1_links.md`) via Task tool.
- **Phase 2 (Tech Spec)**: Do NOT read `data/integration-source-links.json` to extract URLs. Do NOT create URL files. Do NOT run `grace techspec`. Do NOT activate the virtualenv. Do NOT use WebFetch to scrape connector docs. Do NOT synthesize the spec yourself or write to `grace/rulesbook/codegen/references/`. ONLY spawn the Tech Spec Agent (`2.2_techspec.md`) via Task tool — it picks Path A or Path B internally.
- **Phase 4 (Codegen)**: Do NOT read pattern guides or tech specs for implementation. Do NOT write connector code. Do NOT run `cargo build`. Do NOT run `grpcurl`. ONLY spawn the Code Generation Agent (`2.3_codegen.md`) via Task tool.
- **Phase 5 (Commit & PR)**: Do NOT run `git add`, `git commit`, `git push`, or `gh pr create`. Do NOT stage files or create branches. ONLY spawn the PR Agent (`2.4_pr.md`) via Task tool. The PR Agent handles ALL git commit, push, and PR creation work — it commits **directly on `{BRANCH}`**. Per its own guardrail (`2.4_pr.md`, "NO BRANCH CREATION, NO CHERRY-PICK"), it does NOT run `git checkout -b` and does NOT cherry-pick.

**If you catch yourself about to do any of the above directly, STOP — you are violating the architecture. Spawn the correct subagent instead.**

Follow the phases below in order. Do not skip or reorder. Do not run phases in parallel.

**Credentials**: Resolved the way the test harness resolves them — `CONNECTOR_AUTH_FILE_PATH`, else `UCS_CREDS_PATH`, else `creds.json` at the repo root (`crates/internal/integration-tests/src/harness/credentials.rs`, `creds_file_path()`); the keys are lowercase connector names. If credentials fail during testing (HTTP 401/403), report FAILED — do NOT ask the user.

**Note**: Connector names in `{CONNECTORS_FILE}` use the exact casing provided (e.g., `Adyen`, `Paypal`). Use this casing (`{Connector_Name}`) when invoking the Tech Spec Agent (it flows into both `grace techspec` for Path A and the Claude-native workflow for Path B). Use lowercase (`{connector}`) for file names, branch names, and directory paths.

---

## Inputs

| Parameter           | Description                             | Example           |
| ------------------- | --------------------------------------- | ----------------- |
| `{CONNECTOR}`       | Connector name (exact casing from JSON) | `Adyen`           |
| `{FLOW}`            | Payment flow being implemented (a flow-marker name from `crates/types-traits/domain_types/src/connector_flow.rs`) | `Authorize`       |
| `{PAYMENT_METHOD}`  | Payment method being added; empty for new-flow runs | `BankDebit` or *(empty)* |
| `{CONNECTORS_FILE}` | JSON file with connector names          | `connectors.json` |
| `{BRANCH}`          | Git branch all work happens on          | `feat/bank-debit` |

---

## Phase 1: Links Discovery (SPAWN SUBAGENT)

**GUARDRAIL: You MUST spawn a subagent. Do NOT fetch URLs, browse docs sites, or use WebFetch yourself. Violation = broken architecture.**

You MUST use the **Task tool** to spawn a **Links Agent** for documentation discovery. Do NOT search for documentation links yourself. Do NOT read the workflow file yourself — the subagent reads it on its own.

**Spawn a Task with these parameters:**

```
Task(
  subagent_type="general-purpose",
  description="Find {FLOW} links for {CONNECTOR}",
  prompt="Read and follow the workflow defined in grace/workflow/2.1_links.md

Variables:
  CONNECTOR_NAME: <connector name, exact casing from connectors file>
  PAYMENT_METHOD: <{PAYMENT_METHOD} if it is set, otherwise {FLOW}>"
)
```

`2.1_links.md` takes only `{{CONNECTOR_NAME}}` and `{{PAYMENT_METHOD}}` — it has no `FLOW` input, and it searches documentation for whatever `{{PAYMENT_METHOD}}` names. So pass `{PAYMENT_METHOD}` on a payment-method-addition run, and fall back to `{FLOW}` only when `{PAYMENT_METHOD}` is empty. Passing `Authorize` while `{PAYMENT_METHOD}` is `BankDebit` sends the Links Agent hunting for the wrong docs.

**Note**: Links discovery failure is NOT a hard gate. If the Links Agent returns no links or fails, proceed to Phase 2 anyway — the Tech Spec Agent will attempt to work with whatever URLs are available. Log the links status for the final report.

---

## Phase 2: Tech Spec Generation (SPAWN SUBAGENT)

**GUARDRAIL: You MUST spawn a subagent. Do NOT extract URLs, create URL files, run `grace techspec`, activate any virtualenv, scrape docs via WebFetch, or synthesize a spec yourself. Violation = broken architecture.**

You MUST use the **Task tool** to spawn a **Tech Spec Agent**. Do NOT extract URLs, run `grace techspec`, scrape docs, or do any tech spec work yourself. Do NOT read the workflow file yourself — the subagent reads it on its own.

**Note**: `2.2_techspec.md` detects the grace CLI environment internally and picks one of two paths: Path A (`grace techspec`) when `grace/.env` is configured with real API keys, or Path B (Claude-native, following `.skills/generate-tech-spec/references/techspec-generation-native.md`) when it is not. Both paths produce a spec under `grace/rulesbook/codegen/references/` that this Connector Agent can read in Phase 3 — no behaviour change is required on the orchestrator side.

**Spawn a Task with these parameters:**

```
Task(
  subagent_type="general-purpose",
  description="Generate techspec for {CONNECTOR}",
  prompt="Read and follow the workflow defined in grace/workflow/2.2_techspec.md

Variables:
  CONNECTOR: <connector name, exact casing>
  FLOW: <the payment flow>
  PAYMENT_METHOD: <payment method being added, or empty for new-flow runs>"
)
```

**Gate**: If the tech spec agent returns FAILED (no spec generated), report this connector as FAILED and go directly to Phase 6 (report). No code was generated, so there is nothing to commit or PR.

---

## Phase 3: Setup & Discover Files (you do this yourself)

### 3a: Verify directory and branch

```bash
pwd && ls Cargo.toml crates/ Makefile     # verify directory
git status                                  # verify on {BRANCH} branch
```

If not on `{BRANCH}`, something is wrong — do NOT create a new branch, report FAILED.

### 3b: Find (or create) the tech spec

The tech spec is a **hard dependency** for Phase 4 (codegen) — without it, there is nothing for the Code Generation Agent to read. The normal flow already creates it in Phase 2, and the Tech Spec Agent is idempotent (re-running it is a no-op if a spec already exists). If for any reason no spec is present at this point, recover by re-running the dependency chain — do NOT skip straight to SKIPPED.

**Important**: All searches must run from the repo root (where `Cargo.toml` is). Verify with `pwd` if unsure. Do NOT skip this search — actually run it.

Glob search the entire references directory (case-insensitive, specs may be in subdirectories):

```bash
find grace/rulesbook/codegen/references -iname "*{connector}*{flow}*" -o -iname "*{connector}*" | head -20
```

If no results, also try with underscores/hyphens (e.g., `wells_fargo` vs `wellsfargo`).

Note: both generation paths in `2.2_techspec.md` write two copies — the raw spec at `grace/rulesbook/codegen/references/specs/{Connector_Name}.md` (display casing, e.g. `specs/Dlocal.md`) and the canonical copy at `grace/rulesbook/codegen/references/{connector}/technical_specification.md` (lowercase directory, e.g. `paynearme/technical_specification.md`). **Prefer the canonical copy** — it is the only path the downstream `new-connector` skill accepts. The connector name may be capitalized in either location, so search recursively and case-insensitively.

**Recovery — if the spec is still missing after the search above:**

The tech spec depends on documentation URLs (produced by the Links Agent), which feed the Tech Spec Agent. Re-run that chain via the same subagent spawns used in Phases 1 and 2:

1. **(Re-)spawn the Links Agent** (`2.1_links.md`) — unless `data/integration-source-links.json` already has a non-empty entry for `{Connector_Name}` (in which case skip this step and reuse the existing entry).
2. **(Re-)spawn the Tech Spec Agent** (`2.2_techspec.md`) — it picks Path A (`grace techspec`) or Path B (Claude-native, per `.skills/generate-tech-spec/references/techspec-generation-native.md`) based on its internal environment check.
3. **Re-run the `find` command above.**

Only if the spec is still missing after this recovery → report SKIPPED, go to Phase 6 with reason `"Tech spec could not be created — Links Agent or Tech Spec Agent failed during recovery."` Capture the failure reason from whichever recovery subagent returned FAILED so it surfaces in the final report.

### 3c: Find connector source files

```
Search: crates/integrations/connector-integration/src/connectors/*{connector}*
```

Note the actual name (e.g., `wells_fargo` vs `wellsfargo`). If not found -> report SKIPPED, go to Phase 6.

Store `{TECHSPEC_PATH}` and `{CONNECTOR_SOURCE_FILES}` for the next phase.

---

## Phase 4: Code Generation (SPAWN SUBAGENT)

**GUARDRAIL: You MUST spawn a subagent. Do NOT read pattern guides, write Rust code, run `cargo build`, or run `grpcurl` yourself. Violation = broken architecture.**

You MUST use the **Task tool** to spawn a **Code Generation Agent**. Do NOT read pattern guides, write implementation code, run cargo build, or run grpcurl yourself. Do NOT read the workflow file yourself — the subagent reads it on its own.

**Spawn a Task with these parameters:**

```
Task(
  subagent_type="general-purpose",
  description="Implement {FLOW} code for {CONNECTOR}",
  prompt="Read and follow the workflow defined in grace/workflow/2.3_codegen.md

Variables:
  CONNECTOR: <connector name>
  FLOW: <the payment flow>
  PAYMENT_METHOD: <payment method being added, or empty for new-flow runs>
  TECHSPEC_PATH: <path to the tech spec file found in Phase 3>
  CONNECTOR_SOURCE_FILES: <paths to connector source files found in Phase 3>"
)
```

**Gate**: If the Code Generation Agent returns FAILED, proceed to Phase 5 (Commit & PR) anyway — the PR Agent will commit the incomplete code and create a "do not merge" PR for visibility.

Store the codegen result:

- `{CODEGEN_STATUS}` = `SUCCESS` or `FAILED`
- `{CODEGEN_FAILURE_REASON}` = reason string (empty if SUCCESS)
- `{CODEGEN_GRPCURL_OUTPUT}` = the full `GRPCURL_OUTPUT` section from the codegen agent's output — this MUST include the complete grpcurl command(s) with headers and payload, plus the complete response JSON. This is passed to the PR Agent for the PR description. If the codegen agent did not return `GRPCURL_OUTPUT`, extract whatever grpcurl output is visible in the agent's response.

---

## Phase 5: Commit & Pull Request (SPAWN SUBAGENT — ALWAYS, for both SUCCESS and FAILED)

**GUARDRAIL: You MUST spawn a subagent. Do NOT run `git add`, `git commit`, `git push`, or `gh pr create` yourself. Violation = broken architecture.**

**This phase runs for BOTH successful and failed connectors.** The PR Agent handles everything: committing directly on `{BRANCH}` (the branch preflight created — it creates no branch of its own and does NOT cherry-pick), credential scrubbing, pushing `{BRANCH}` to `origin`, and creating a same-repo PR on `juspay/hyperswitch-prism`. The only case where you skip this phase is if codegen produced no file changes **anywhere** in the working tree (check plain `git status --porcelain`). Do NOT scope that check to `crates/integrations/connector-integration/src/connectors/{connector}*`: per `2.4_pr.md` Phase 1a, a connector change is never confined to that directory — the Rust registry, `payment.proto`, the `config/*.toml` files and `connector_specs/` change too, so a narrow check would wrongly skip the PR for a run that touched only those.

You MUST use the **Task tool** to spawn a **PR Agent**. Do NOT read the workflow file yourself — the subagent reads it on its own.

**Spawn a Task with these parameters:**

```
Task(
  subagent_type="general-purpose",
  description="Commit and create PR for {CONNECTOR} {FLOW}",
  prompt="Read and follow the workflow defined in grace/workflow/2.4_pr.md

Variables:
  CONNECTOR: <connector name, lowercase for branches, original casing for display>
  FLOW: <the payment flow>
  BRANCH: <the branch the implementation checkpoint committed on — same as {BRANCH} in this agent's Inputs>
  CONNECTOR_STATUS: <SUCCESS or FAILED>
  FAILURE_REASON: <reason string, empty if SUCCESS>
  GRPCURL_OUTPUT: <the full grpcurl test output from the Codegen Agent, raw text>
  CONNECTOR_SOURCE_FILES: <paths to connector source files from Phase 3>"
)
```

**Gate**: If the PR Agent returns FAILED, log the failure but do NOT change the connector's overall status based on PR creation alone. Report the PR status separately in Phase 6.

### 5b: Verify you are back on the dev branch

This check is still required: `2.4_pr.md` Phase 1a runs `git checkout {BRANCH}` when it finds itself on another branch, so the PR Agent can legitimately move HEAD. Verify, do not assume.

After the PR Agent finishes, verify you are on `{BRANCH}`:

```bash
git branch --show-current
```

If not on `{BRANCH}`, switch back:

```bash
git checkout {BRANCH}
```

---

## Phase 6: Report

**Return result:**

```
CONNECTOR: {connector}
STATUS: SUCCESS | FAILED | SKIPPED
LINKS: {found/missing} | {link_count} links
PR: {PR_URL or "not created"}
REASON: <if not SUCCESS, explain why>
```

**STATUS definitions (strict):**

- **SUCCESS**: Build passed AND grpcurl Authorize passed AND code was committed AND PR was created. All must be true. No exceptions.
- **FAILED**: Any phase failed after attempting it (build errors, test errors, service won't start, credentials rejected, PR creation failed, etc.)
- **SKIPPED**: Connector was skipped before implementation (no tech spec and creation recovery in Phase 3b also failed, no source files, already implemented, no credentials)

---

## Subagent Reference

| Agent                 | File              | Purpose                                                                                               |
| --------------------- | ----------------- | ----------------------------------------------------------------------------------------------------- |
| Links Agent           | `2.1_links.md`    | Find and verify backend API documentation links                                                       |
| Tech Spec Agent       | `2.2_techspec.md` | Generate tech spec via grace CLI (Path A) or Claude-native fallback (Path B); selected by env check   |
| Code Generation Agent | `2.3_codegen.md`  | Read, analyze, implement, build, and grpcurl test                                                     |
| PR Agent              | `2.4_pr.md`       | Commit directly on `{BRANCH}` (no branch creation, no cherry-pick), scrub creds, push, create PR in juspay/hyperswitch-prism |
