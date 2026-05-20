# Connector Session (v2 — extended)

You are the **sole owner** of implementing every unimplemented item for **{CONNECTOR}** that survives confidence-score gating. You handle the session end-to-end: link discovery + scoring, **umbrella issue creation in `juspay/grace`**, techspec generation (with fallback when env keys are missing), per-item codegen, grpcurl validation, and committing.

You are invoked **once per connector** by the Orchestrator (`1_orchestrator.md`) via `openswarm exec --local --pipeline`. The OpenSwarm Worker runs you; the OpenSwarm Reviewer (post-Worker) reads your final diff.

**First**: Read this file (`grace/workflow/v2/2_connector.md`) fully to understand all phases and rules before proceeding.

You coordinate by **spawning subagents via the Task tool** for the heavy phases (link scoring, techspec, codegen, grpcurl, PR). The umbrella-issue creation in Phase 1.5 is a lightweight shell step you run yourself.

**HARD GUARDRAIL — MANDATORY SUBAGENT DELEGATION**: For Phases 1, 2, 3, 4, 5 you MUST spawn separate subagents via the Task tool. Do NOT do their work yourself. Each subagent reads its own workflow file — do NOT paste their contents into prompts.

---

## Inputs

| Parameter | Description | Example |
|---|---|---|
| `{CONNECTOR}` | Connector slug (lowercase) | `stripe` |
| `{MANIFEST}` | Path to per-connector manifest from Phase 0 | `grace/workflow/v2/work/stripe_manifest.json` |
| `{BRANCH}` | Shared dev branch | `feat/grace-v2-2026-05-03` |
| `{LINK_THRESHOLD}` | Min confidence (0.0-1.0) to keep an item | `0.70` |

---

## RULES

1. **Working directory**: All commands from `connector-service` repo root. Exception: `grace techspec` runs from `grace/` with venv activated (delegated to Tech Spec Agent — you don't activate the venv yourself).
2. **No retry loops without diagnosis**: NEVER retry the same `cargo build` or `grpcurl` call without a code change. If a stage fails, read logs, identify root cause, fix, then retest. Item-codegen agents handle their own internal retries; you do not loop them externally.
3. **Scoped git**: Per-item commits stage only the touched files. The PR Agent (Phase 5) handles all `git add` / `git commit` / `git cherry-pick` / `git push` / `gh pr create`. You do NOT run those.
4. **Credentials**: `creds.json` at repo root. If creds for `{CONNECTOR}` are absent, return `STATUS: SKIPPED` with reason `"no credentials"` immediately and skip all phases.
5. **Phases run in order**: 0 → 1 → 1.5 → 2 → 3 → 4 → 5 → 6. Do not skip or reorder.
6. **One results file**: All phases append to `grace/workflow/v2/work/{CONNECTOR}_results.json`. The orchestrator reads this file.
7. **Issue creation is non-fatal**: if `gh` auth fails or network blips, set `ISSUE_NUMBER`/`ISSUE_URL` empty and continue. Phases 2/3 detect empty values and skip their comment-posting sub-phases cleanly.
8. **Time-budgeted runs are permitted (with carry-over)**: a single Worker session has a wall-clock budget that may be exceeded by large `KEPT_ITEMS` arrays (47 items × ~3 min each ≈ 2.5h is realistic; ≥100 items routinely exceed any reasonable session window). When the Worker detects budget exhaustion mid-Phase-3, it MUST mark the un-attempted items with `codegen_status: "SKIPPED"` and `reason: "carry_over_time_budget"` (NOT `"session_time_budget_exceeded"`, which earlier ad-hoc runs invented and is hereby retired). The orchestrator treats `carry_over_time_budget` items as eligible for re-pickup on the next run (see Phase 6 reporting). Aborting Phase 3 on a single FAILED item is still forbidden; aborting on time exhaustion is permitted iff carry-over is recorded.
9. **Worker status reporting must mirror the per-item gate**: the Worker's stdout-summary boolean / `success` flag MUST be derived from the per-item results, not asserted independently. Definition: `success = (N_success > 0 AND N_failed == 0 AND N_carry_over == 0)`. If any item is FAILED or carried over, the connector status is `PARTIAL` (or `FAILED` if all attempted items failed), and the Worker's summary boolean MUST be `false`. PR titles MUST embed the same `(N_success, N_failed, N_carry_over)` triple — no run may report `success: true` while the PR title contradicts it.

---

## Phase 0: Pre-flight (you do this yourself)

Verify branch and creds.

```bash
pwd && ls Cargo.toml crates/ Makefile
git branch --show-current
# expect: {BRANCH}. If not on {BRANCH}, switch:
git checkout {BRANCH}

jq -e --arg c "{CONNECTOR}" '.[$c] // .[($c | ascii_upcase)] // .[($c | ascii_downcase)]' creds.json >/dev/null \
  || { echo "no credentials for {CONNECTOR} — exiting SKIPPED"; exit 0; }

jq '.counts' {MANIFEST}
```

If `{MANIFEST}` does not exist or has zero counts, exit with `STATUS: SKIPPED`, reason `"no unimplemented items"`.

---

## Phase 1: Link Discovery + Confidence Scoring (SPAWN SUBAGENT)

**Idempotency**: Before spawning the agent, check `grace/workflow/v2/work/{CONNECTOR}_links.json`. If it exists AND its mtime is within 24 hours, DO NOT spawn the Link-Scoring Agent. Load `KEPT_ITEMS` from `[.items[] | select(.decision=="implement")]` of the existing JSON and `SKIPPED_ITEMS` from the `decision=="skip"` filter, then proceed to Phase 1.5. This saves ~$5 and ~10 min per connector on retries. To force a fresh run: `rm grace/workflow/v2/work/{CONNECTOR}_links.json` before invoking.

**GUARDRAIL: Spawn a subagent. Do NOT use WebFetch / WebSearch yourself to discover or score links.**

Spawn the Link-Scoring Agent via Task tool with a minimal prompt:

```
Task(
  subagent_type="general",
  description="Score links for {CONNECTOR}",
  prompt="Read and follow the workflow defined in grace/workflow/v2/2.1_link_scoring.md

Variables:
  CONNECTOR: {CONNECTOR}
  MANIFEST: {MANIFEST}
  THRESHOLD: {LINK_THRESHOLD}
  OUTPUT: grace/workflow/v2/work/{CONNECTOR}_links.json"
)
```

Wait for the agent to finish. It writes `{CONNECTOR}_links.json` whose `items[]` array contains one entry per manifest item with either `decision: "implement"` (a link scored ≥ threshold) or `decision: "skip"` (with `reason`).

**Gate**: If the JSON has zero `decision: "implement"` items, report `STATUS: SKIPPED`, reason `"all items below confidence threshold"`, and go to Phase 6.

Capture from the JSON:
- `KEPT_ITEMS` = array of items with `decision: "implement"`
- `SKIPPED_ITEMS` = array of items with `decision: "skip"` (for the final report)

---

## Phase 1.5: Create Umbrella Tracking Issue (you do this yourself)

**Skip cleanly** if `gh` is unauthenticated. Run once per connector right after Phase 1 — `KEPT_ITEMS` is now known.

```bash
LINKS_JSON="grace/workflow/v2/work/{CONNECTOR}_links.json"
ISSUE_NUMBER=""
ISSUE_URL=""

if gh auth status >/dev/null 2>&1; then
  TITLE="[grace-v2] {CONNECTOR} — implementation tracking ($(date -u +%Y-%m-%d))"
  BODY_FILE=$(mktemp -t grace-v2-issue-{CONNECTOR}-XXXXXX.md)
  N_KEPT=$(jq '[.items[] | select(.decision=="implement")] | length' "$LINKS_JSON")
  N_SKIP=$(jq '[.items[] | select(.decision=="skip")]      | length' "$LINKS_JSON")

  {
    echo "## [grace-v2] {CONNECTOR} — implementation tracking"
    echo
    echo "> Generated by Grace v2 workflow on $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo
    echo "### Connector"
    echo "- Slug: \`{CONNECTOR}\`"
    echo "- Dev branch: \`{BRANCH}\`"
    echo "- Confidence threshold: {LINK_THRESHOLD}"
    echo "- Items kept: $N_KEPT"
    echo "- Items skipped (low confidence / no candidates): $N_SKIP"
    echo
    echo "### Items in scope (will be implemented)"
    echo
    echo "#### API Flows"
    jq -r '.items[] | select(.decision=="implement" and .kind=="flow") | "- [ ] `\(.id)` — **\(.name)** — `\(.source_flow)` — score \(.best_link.score) — [docs](\(.best_link.url))"' "$LINKS_JSON"
    echo
    echo "#### Payment Methods (grouped by source_flow)"
    jq -r '
      [.items[] | select(.decision=="implement" and .kind=="payment_method")]
      | group_by(.source_flow)[]
      | "##### \(.[0].source_flow)\n" + (map("- [ ] `\(.id)` — **\(.name)** — score \(.best_link.score) — [docs](\(.best_link.url))") | join("\n"))
    ' "$LINKS_JSON"
    echo
    echo "### Items skipped at gate (low confidence)"
    echo "<details><summary>$N_SKIP items below threshold {LINK_THRESHOLD}</summary>"
    echo
    jq -r '.items[] | select(.decision=="skip") | "- `\(.id)` — \(.name) — best score \(.best_score // 0) — reason: \(.reason)"' "$LINKS_JSON"
    echo
    echo "</details>"
    echo
    echo "### Phase progress"
    echo "- [x] Phase 1 — link discovery + scoring"
    echo "- [ ] Phase 2 — techspec generation (per-item comments will land below)"
    echo "- [ ] Phase 3 — codegen (per-item plan comments before edits, then cargo build)"
    echo "- [ ] Phase 4 — grpcurl validation"
    echo "- [ ] Phase 5 — PR opened"
  } > "$BODY_FILE"

  ISSUE_URL=$(gh issue create \
      --repo juspay/grace \
      --title "$TITLE" \
      --body-file "$BODY_FILE" \
      --label "connector-implementation,enhancement" 2>&1) || ISSUE_URL=""
  ISSUE_NUMBER=$(echo "$ISSUE_URL" | grep -oE '[0-9]+$')
  rm -f "$BODY_FILE"

  echo "[Phase 1.5] umbrella issue: $ISSUE_URL (#$ISSUE_NUMBER)"
else
  echo "[Phase 1.5] gh unauthenticated — skipping umbrella issue creation"
fi
```

`ISSUE_NUMBER` and `ISSUE_URL` are now in scope for Phases 2 and 3. Their emptiness is acceptable; downstream agents handle that.

---

## Phase 2: TechSpec Generation (SPAWN ONE SUBAGENT PER KEPT ITEM)

**GUARDRAIL: Spawn a subagent per item. Do NOT run `grace techspec` yourself.**

For each item in `KEPT_ITEMS`, spawn the Tech Spec Agent in a SEPARATE message (sequential, one at a time):

```
Task(
  subagent_type="general",
  description="Techspec {CONNECTOR}/{ITEM_NAME}",
  prompt="Read and follow the workflow defined in grace/workflow/v2/2.2_techspec.md

Variables:
  CONNECTOR: {CONNECTOR}
  ITEM_ID: <item.id>
  ITEM_NAME: <item.name>
  ITEM_KIND: <item.kind: 'flow' or 'payment_method'>
  SOURCE_FLOW: <item.source_flow>
  LINK_URL: <item.best_link.url>
  OUTPUT_DIR: grace/rulesbook/codegen/references/{CONNECTOR}/v2
  ISSUE_NUMBER: $ISSUE_NUMBER
  ISSUE_URL: $ISSUE_URL"
)
```

Each agent returns `STATUS: SUCCESS` (with `TECHSPEC_PATH`, `TECHSPEC_SOURCE`, `COMMENT_URL`) or `STATUS: FAILED` (with reason). **Drop FAILED items** — record them in the results JSON as `SKIPPED (techspec_failed)`.

After this phase, you have `READY_ITEMS` = items with both a high-confidence link AND a successful techspec, with their `techspec_path`, `techspec_source`, and `comment_url` carried alongside.

---

## Phase 3: Per-Item Code Generation (SPAWN ONE SUBAGENT PER READY ITEM, SEQUENTIAL)

**GUARDRAIL: Spawn a subagent per item. Do NOT write Rust code or run `cargo build` yourself.**

**HARD GUARDRAIL — SEQUENTIAL ONLY**: Codegen must be sequential per item within a connector — `cargo target/` is single, parallel `cargo build` against the same tree corrupts artifacts.

For each item in `READY_ITEMS`, in a separate message, spawn the Item Codegen Agent:

```
Task(
  subagent_type="general",
  description="Codegen {CONNECTOR}/{ITEM_NAME}",
  prompt="Read and follow the workflow defined in grace/workflow/v2/2.3_item_codegen.md

Variables:
  CONNECTOR: {CONNECTOR}
  ITEM_ID: <item.id>
  ITEM_NAME: <item.name>
  ITEM_KIND: <flow or payment_method>
  SOURCE_FLOW: <item.source_flow>
  TECHSPEC_PATH: <from Phase 2>
  TECHSPEC_SOURCE: <from Phase 2: grace_cli or claude_fallback>
  ISSUE_NUMBER: $ISSUE_NUMBER
  ISSUE_URL: $ISSUE_URL
  GRPCURL_PAYLOAD_OUT: grace/workflow/v2/work/{CONNECTOR}_payloads/<item.id>.json"
)
```

Each agent returns one of:
- `STATUS: SUCCESS` — code written, `cargo build` passed, grpcurl payload template emitted, plan comment posted (or skipped if no issue).
- `STATUS: FAILED` — with reason (build error, conflict with existing flow, etc.).
- `STATUS: SKIPPED` — already implemented (codegen detected the item is already in the source).

Record each item's status to the in-memory results structure including `plan_comment_url` (may be empty). **Do not abort the phase on a single FAILED**; continue with the next item.

**Time-budget exit (per RULE 8)**: at the start of each item's iteration the Worker checks its remaining wall-clock budget (`session_deadline - now()`). If less than the worst-case per-item codegen budget (~10 min) remains, stop spawning new codegen agents and mark every remaining item as:

```json
{ "codegen_status": "SKIPPED", "status": "SKIPPED", "reason": "carry_over_time_budget" }
```

These items MUST appear in the results JSON and MUST be listed in the umbrella issue under a new `### Carry-over (next run)` section so the next Worker invocation can pick them up. They MUST NOT be silently dropped, and the reason string MUST be exactly `"carry_over_time_budget"` (NOT `"session_time_budget_exceeded"` — that legacy string is retired and the orchestrator/reviewer will treat it as an error).

After all items: `IMPLEMENTED_ITEMS` = items with `STATUS: SUCCESS`.

---

## Phase 4: grpcurl Validation (SPAWN SUBAGENT)

**GUARDRAIL: Spawn a subagent. Do NOT run `make run` or `grpcurl` yourself.**

Spawn the Grpcurl Runner ONCE for the connector (it iterates internally):

```
Task(
  subagent_type="general",
  description="grpcurl runner for {CONNECTOR}",
  prompt="Read and follow the workflow defined in grace/workflow/v2/2.5_grpcurl_runner.md

Variables:
  CONNECTOR: {CONNECTOR}
  ITEMS: <JSON array of IMPLEMENTED_ITEMS with their grpcurl payload paths>
  PAYLOADS_DIR: grace/workflow/v2/work/{CONNECTOR}_payloads
  OUTPUT: grace/workflow/v2/work/{CONNECTOR}_results.json"
)
```

The runner returns per-item validation status. Update the results JSON with `grpcurl_status` per item.

**Gate**: An item with `cargo build` SUCCESS but `grpcurl` FAILED is still considered FAILED for the final report.

---

## Phase 5: Commit & PR (SPAWN SUBAGENT)

**GUARDRAIL: Spawn a subagent. Do NOT run `git add` / `git commit` / `git push` / `gh pr create` yourself.**

Spawn the PR Agent ONCE:

```
Task(
  subagent_type="general",
  description="PR for {CONNECTOR} v2 batch",
  prompt="Read and follow the workflow defined in grace/workflow/v2/2.4_pr.md

Variables:
  CONNECTOR: {CONNECTOR}
  DEV_BRANCH: {BRANCH}
  RESULTS: grace/workflow/v2/work/{CONNECTOR}_results.json
  ITEMS_FOR_PR: <JSON array of items with grpcurl_status SUCCESS or FAILED>
  ISSUE_URL: $ISSUE_URL"
)
```

The PR Agent commits per-item changes on `{BRANCH}`, cherry-picks to a clean per-connector PR branch, scrubs creds, pushes, opens a PR on `juspay/connector-service`. The PR description should reference `$ISSUE_URL` so the umbrella issue and PR are cross-linked.

Capture `PR_URL` from the agent's output.

---

## Phase 6: Verify Branch + Report

After Phase 5, verify you are back on `{BRANCH}`:

```bash
git branch --show-current
# if not {BRANCH}:
git checkout {BRANCH}
```

Write the final results JSON. The schema must be:

```json
{
  "connector": "{CONNECTOR}",
  "status": "SUCCESS|PARTIAL|FAILED|SKIPPED",
  "branch": "{BRANCH}",
  "issue_number": "<umbrella issue number or empty>",
  "issue_url": "<umbrella issue URL or empty>",
  "pr_url": "<url or empty>",
  "items": [
    {
      "id": "f5z",
      "name": "Wallet - Paypal",
      "kind": "payment_method",
      "source_flow": "PaymentService.Authorize",
      "decision": "implement",
      "techspec_status": "SUCCESS|FAILED|SKIPPED",
      "techspec_source": "grace_cli|claude_fallback|",
      "techspec_comment_url": "<url or empty>",
      "codegen_status": "SUCCESS|FAILED|SKIPPED",
      "plan_comment_url": "<url or empty>",
      "grpcurl_status": "SUCCESS|FAILED|SKIPPED",
      "status": "SUCCESS|FAILED|SKIPPED",
      "reason": "<empty if SUCCESS>"
    }
  ]
}
```

**status definitions** (per item):
- `SUCCESS` — codegen + cargo build + grpcurl all SUCCESS.
- `FAILED` — any phase failed after attempting it.
- `SKIPPED` — pre-empted (low confidence, already implemented, techspec missing, no creds, **or** carry-over per RULE 8 with `reason: "carry_over_time_budget"`).

**connector status**:
- `SUCCESS` — all kept items SUCCESS.
- `PARTIAL` — some SUCCESS, some FAILED/SKIPPED (including carry-over).
- `FAILED` — every attempted item FAILED **and** no carry-over (a 0/all-failed batch with carry-over is still PARTIAL because the carry-over may succeed next run).
- `SKIPPED` — Phase 0 or 1 short-circuited.

**Worker-summary `success` boolean** (mandated by RULE 9): derived strictly as

```
success = (N_success > 0) AND (N_failed == 0) AND (N_carry_over == 0)
```

The summary boolean is NEVER asserted independently of the items array. If the PR title is `"…— 0 success, 3 failed"` the summary boolean MUST be `false`. The reviewer treats a `success: true` summary that contradicts the per-item gate as a **revise-blocking** misreport.

Print the summary to stdout (the OpenSwarm Worker captures this for the Reviewer):

```
=== {CONNECTOR} ===
Status: {status}
Issue: {ISSUE_URL or "none"}
Items: implement={N_kept}, success={N_success}, failed={N_failed}, skipped={N_skipped}, carry_over={N_carry_over}
PR: {PR_URL or "none"}
Worker-success: {true|false}   # MUST satisfy: true iff N_success>0 AND N_failed==0 AND N_carry_over==0
```

---

## Subagent Reference

| Agent | File | Purpose |
|-------|------|---------|
| Link-Scoring Agent | `2.1_link_scoring.md` | Discover candidate URLs per item, score, drop below-threshold |
| Tech Spec Agent | `2.2_techspec.md` | Run `grace techspec` for one item, fall back to WebFetch when CLI missing keys, post tech spec as a comment on umbrella issue |
| Item Codegen Agent | `2.3_item_codegen.md` | Post implementation plan comment, implement one item, cargo build |
| Grpcurl Runner | `2.5_grpcurl_runner.md` | Start service, grpcurl per implemented item, write results JSON |
| PR Agent | `2.4_pr.md` | Commit + cherry-pick + push + open PR for the batch (referencing umbrella issue) |
