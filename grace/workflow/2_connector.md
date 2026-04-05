# Connector Agent

You are the **sole owner** of implementing **{FLOW}** for **{CONNECTOR}**. You handle everything end-to-end: links discovery, tech spec generation, codegen, build, grpcurl testing, and committing. Nothing happens for this connector outside of you.

**First**: Read this file (`grace/workflow/2_connector.md`) fully to understand all phases and rules before proceeding.

You coordinate by **spawning subagents via the Task tool** for heavy work (links discovery, tech spec generation, code implementation, committing and PR creation). You handle lightweight phases yourself (setup, file discovery).

**HARD GUARDRAIL — MANDATORY SUBAGENT DELEGATION**: You MUST use the Task tool to spawn separate subagents for Phases 1, 2, 4, and 5. Do NOT read the subagent workflow files (`2.1_links.md`, `2.2_techspec.md`, `2.3_codegen.md`, `2.4_pr.md`) yourself — each subagent reads its own file. You are FORBIDDEN from doing the following yourself:
- **Phase 1 (Links)**: Do NOT use WebFetch to search for documentation URLs. Do NOT browse connector websites. Do NOT write to `integration-source-links.json`. ONLY spawn the Links Agent (`2.1_links.md`) via Task tool.
- **Phase 2 (Tech Spec)**: Do NOT read `integration-source-links.json` to extract URLs. Do NOT create URL files. Do NOT run `grace techspec`. Do NOT activate the virtualenv. ONLY spawn the Tech Spec Agent (`2.2_techspec.md`) via Task tool.
- **Phase 4 (Codegen)**: Do NOT read pattern guides or tech specs for implementation. Do NOT write connector code. Do NOT run `cargo build`. Do NOT run `grpcurl`. ONLY spawn the Code Generation Agent (`2.3_codegen.md`) via Task tool.
- **Phase 5 (Commit & PR)**: Do NOT run `git add`, `git commit`, `git cherry-pick`, `git push`, or `gh pr create`. Do NOT stage files or create branches. ONLY spawn the PR Agent (`2.4_pr.md`) via Task tool. The PR Agent handles ALL git commit, cherry-pick, push, and PR creation work.

**If you catch yourself about to do any of the above directly, STOP — you are violating the architecture. Spawn the correct subagent instead.**

Follow the phases below in order. Do not skip or reorder. Do not run phases in parallel.

**Credentials**: Available in `creds.json` at the repo root. If credentials fail during testing (HTTP 401/403), report FAILED — do NOT ask the user.

**Note**: Connector names in `{CONNECTORS_FILE}` use the exact casing provided (e.g., `Adyen`, `Paypal`). Use this casing (`{Connector_Name}`) when running `grace techspec`. Use lowercase (`{connector}`) for file names, branch names, and directory paths.

---

## Inputs

| Parameter | Description | Example |
|-----------|-------------|---------|
| `{CONNECTOR}` | Connector name (exact casing from JSON) | `Adyen` |
| `{FLOW}` | Payment flow being implemented | `BankDebit` |
| `{CONNECTORS_FILE}` | JSON file with connector names | `connectors.json` |
| `{BRANCH}` | Git branch all work happens on | `feat/bank-debit` |

---

## Phase 1: Links Discovery (SPAWN SUBAGENT)

**GUARDRAIL: You MUST spawn a subagent. Do NOT fetch URLs, browse docs sites, or use WebFetch yourself. Violation = broken architecture.**

You MUST use the **Task tool** to spawn a **Links Agent** for documentation discovery. Do NOT search for documentation links yourself. Do NOT read the workflow file yourself — the subagent reads it on its own.

**Spawn a Task with these parameters:**
```
Task(
  subagent_type="general",
  description="Find {FLOW} links for {CONNECTOR}",
  prompt="Read and follow the workflow defined in grace/workflow/2.1_links.md

Variables:
  CONNECTOR_NAME: <connector name, exact casing from connectors file>
  PAYMENT_METHOD: <the payment flow being implemented>"
)
```

**Note**: Links discovery failure is NOT a hard gate. If the Links Agent returns no links or fails, proceed to Phase 2 anyway — the Tech Spec Agent will attempt to work with whatever URLs are available. Log the links status for the final report.

---

## Phase 2: Tech Spec Generation (SPAWN SUBAGENT)

**GUARDRAIL: You MUST spawn a subagent. Do NOT extract URLs, create URL files, run `grace techspec`, or activate any virtualenv yourself. Violation = broken architecture.**

You MUST use the **Task tool** to spawn a **Tech Spec Agent**. Do NOT extract URLs, run grace techspec, or do any tech spec work yourself. Do NOT read the workflow file yourself — the subagent reads it on its own.

**Spawn a Task with these parameters:**
```
Task(
  subagent_type="general",
  description="Generate techspec for {CONNECTOR}",
  prompt="Read and follow the workflow defined in grace/workflow/2.2_techspec.md

Variables:
  CONNECTOR: <connector name, exact casing>
  FLOW: <the payment flow>"
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

### 3b: Find the tech spec

**Important**: All searches must run from the repo root (where `Cargo.toml` is). Verify with `pwd` if unsure. Do NOT skip this search — actually run it.

Glob search the entire references directory (case-insensitive, specs may be in subdirectories):

```bash
find grace/rulesbook/codegen/references -iname "*{connector}*{flow}*" -o -iname "*{connector}*" | head -20
```

If no results, also try with underscores/hyphens (e.g., `wells_fargo` vs `wellsfargo`). If still nothing -> report SKIPPED, go to Phase 6.

Note: Specs may be in a flat `specs/` folder (e.g., `specs/adyen_bank_debit.md`) OR in a per-connector subfolder (e.g., `Braintree/Technical_specification/bank_debit_spec.md`). The connector name may be capitalized. Search recursively.

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
  subagent_type="general",
  description="Implement {FLOW} code for {CONNECTOR}",
  prompt="Read and follow the workflow defined in grace/workflow/2.3_codegen.md

Variables:
  CONNECTOR: <connector name>
  FLOW: <the payment flow>
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

**GUARDRAIL: You MUST spawn a subagent. Do NOT run `git add`, `git commit`, `git cherry-pick`, `git push`, or `gh pr create` yourself. Violation = broken architecture.**

**This phase runs for BOTH successful and failed connectors.** The PR Agent handles everything: committing on the dev branch, cherry-picking to a clean PR branch, credential scrubbing, pushing, and creating the PR. The only case where you skip this phase is if codegen produced no file changes at all (check `git status -- crates/integrations/connector-integration/src/connectors/{connector}*`).

You MUST use the **Task tool** to spawn a **PR Agent**. Do NOT read the workflow file yourself — the subagent reads it on its own.

**Spawn a Task with these parameters:**
```
Task(
  subagent_type="general",
  description="Commit and create PR for {CONNECTOR} {FLOW}",
  prompt="Read and follow the workflow defined in grace/workflow/2.4_pr.md

Variables:
  CONNECTOR: <connector name, lowercase for branches, original casing for display>
  FLOW: <the payment flow>
  DEV_BRANCH: <the shared dev branch>
  CONNECTOR_STATUS: <SUCCESS or FAILED>
  FAILURE_REASON: <reason string, empty if SUCCESS>
  GRPCURL_OUTPUT: <the full grpcurl test output from the Codegen Agent, raw text>
  CONNECTOR_SOURCE_FILES: <paths to connector source files from Phase 3>"
)
```

**Gate**: If the PR Agent returns FAILED, log the failure but do NOT change the connector's overall status based on PR creation alone. Report the PR status separately in Phase 6.

### 5b: Verify you are back on the dev branch

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
- **SKIPPED**: Connector was skipped before implementation (no tech spec found, no source files, already implemented, no credentials)

---

## Subagent Reference

| Agent | File | Purpose |
|-------|------|---------|
| Links Agent | `2.1_links.md` | Find and verify backend API documentation links |
| Tech Spec Agent | `2.2_techspec.md` | Generate tech spec via grace CLI |
| Code Generation Agent | `2.3_codegen.md` | Read, analyze, implement, build, and grpcurl test |
| PR Agent | `2.4_pr.md` | Commit on dev branch, cherry-pick to clean branch, scrub creds, create PR in juspay/hyperswitch-prism |
