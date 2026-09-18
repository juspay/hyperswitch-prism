# Grace 

AI-powered connector code generation and payment integration toolkit.

## Installation

```bash
cd grace
uv sync  # if uv not installed: pip install uv
source .venv/bin/activate
```

## Quick Start

### 1. Generate Tech Spec

```bash
# From local docs folder (PDF)
grace techspec <connector-name> -f /path/to/api-docs -v

# Or from a URL
grace techspec <connector-name> -e
```

Output: `rulesbook/codegen/references/specs/<connector-name>.md`

### 2. Run Code Generation

Go back to the connector-service root folder (not `grace/`).

Open `connector-service/` in your AI coding agent and run:

```
integrate <ConnectorName> using grace/rulesbook/codegen/.gracerules
```

The AI agent will run through these phases:
1. **Foundation** → scaffolds files, auth, module registration
2. **Authorize** → payment authorization flow
3. **PSync** → payment status sync
4. **Capture** → capture authorized payments
5. **Refund** → full & partial refunds
6. **RSync** → refund status sync
7. **Void** → cancel authorized payments
8. **Quality** → scores implementation (must be ≥ 60)

### 3. Verify Build

```bash
cargo build
```

### Other Commands

Add a missing flow:
```
add Refund flow to <Connector> using grace/rulesbook/codegen/.gracerules_add_flow
```

Add payment methods:
```
add Wallet:Apple Pay,Google Pay and Card:Credit,Debit to <Connector> using grace/rulesbook/codegen/.gracerules_add_payment_method
```

---

## GRACE v2 Workflow (one connector, many flows, one PR)

Linux only. One run = one connector × many flows: preflight → links (parallel) → one techspec → one plan →
codegen per flow → test/RCA loop (Hyperswitch → UCS → connector is the primary gate) → review → one
hyperswitch-prism PR (+ at most one Hyperswitch PR). State lives in `grace/runs/<connector>-<run6>/`.

### Before your first run

Preflight (`2.0_preflight.md`) checks all of this and aborts with a named reason, but checking first saves a cycle:

| Need | Check |
|---|---|
| Linux, GNU coreutils | `uname -s` |
| CLI tools | `cargo rustup grpcurl jq typos gh ss curl flock nc setsid openssl git` on `PATH`; `git` ≥ 2.38; nightly toolchain (`rustup toolchain list \| grep nightly`) |
| GitHub | `gh auth status`, and push rights on `juspay/hyperswitch-prism` (`origin` must be that repo) |
| Credentials | An entry for the connector (lowercase key) in `CONNECTOR_AUTH_FILE_PATH` → `UCS_CREDS_PATH` → `creds.json`, in the **flat** shape that mirrors the connector's `*Config` proto message. The legacy `connector_account_details` shape aborts the run |
| Disk | ~80 GB free when Hyperswitch is built, ~50 GB otherwise. See the cleanup note below |
| Hyperswitch | A checkout for `HS_REPO_PATH` (optional: without it every HS surface reports `E2E_SKIPPED`, no Hyperswitch PR is raised, and HS parity is never assessed), plus Postgres and Redis (its docker compose, or native) |
| Clean tree | Preflight refuses to start on a dirty tracked tree and never stashes |

**A run changes your machine**: it creates and leaves you on `feat/grace-<connector>-<run6>`, creates a worktree and
branch inside your Hyperswitch checkout, pushes one branch and opens one PR (plus at most one Hyperswitch PR), and takes
hours — the session must stay alive. Full list: `2_connector.md` § *What a run does to the machine*.

**Disk cleanup deletes build output**: preflight removes `target/` directories that are eligible under the policy (the
branch's PR is merged or closed, or the checkout is clean and untouched for `STALE_DAYS`, default 3) — never a dirty
tree, one in use, or the run's own repos. By default only worktrees of this repo, `HS_REPO_PATH` and `GRACE_EXTRA_REPOS`
are candidates. Other checkouts are scanned **only** if you pass `GRACE_SCAN_ROOTS`. To skip cleanup entirely, pass a
`MIN_FREE_GB` below your current free space.

Single connector — run from the top-level session:

```
Read grace/workflow/2_connector.md and follow it exactly.
CONNECTOR: Braintree
FLOWS: Refund,RSync,ThreeDS
HS_REPO_PATH: /path/to/hyperswitch
```

`2_connector.md` § Inputs lists the optional knobs (`MAX_RUN_HOURS`, `MIN_FREE_GB`, `STALE_DAYS`, `GRACE_SCAN_ROOTS`,
`GRACE_EXTRA_REPOS`, `PARALLEL_HS_BUILD`, `RUN_ID`).

Resume an interrupted run by adding `RUN_ID: braintree-a1b2c3` — run ids are the directory names under `grace/runs/`
(gitignored, so `git status` never shows them): `ls grace/runs/`.

Many connectors (the same flows, one connector at a time):

```
Implement {FLOWS} for all connectors in {CONNECTORS_FILE}. Read grace/workflow/1_orchestrator.md and follow it exactly.
HS_REPO_PATH: {HS_REPO_PATH}
```

Batch nests one level deeper (orchestrator → Connector Agent → stages). If it stops with `NO_TASK_TOOL` (the Connector
Agent cannot spawn subagents), run the single-connector prompt above once per remaining connector. If it stops with
`tree dirty`, resume the named run (`RUN_ID`) or discard its edits, then restart the batch.

### Workflow Architecture

```
workflow/
├── 1_orchestrator.md        # Batch: one 2_connector.md run per connector
├── 2_connector.md           # v2 orchestrator: DAG, rules, loop-back protocol, caps, resume
├── 2.0_preflight.md         # S0: tools, creds, disk cleanup, branch, HS worktree, warm builds
├── 2.1_links.md             # S1: links discovery (per flow + common)
├── 2.1a_hs_scout.md         # S1: read-only Hyperswitch reference + reachability
├── 2.2_techspec.md          # S2: one combined tech spec
├── 2.3a_plan.md             # S3: codegen plan
├── 2.3b_codegen_unit.md     # S4: codegen per flow; __hs__ and __finalize__ (CI-parity gate)
├── 2.3_codegen.md           # Single-flow codegen (cited by 2.3a/2.3b)
├── 2.6a_test_env.md         # S5: UCS + Hyperswitch environment, baseline
├── 2.6b_test_design.md      # S5: test cases
├── 2.6c_test_requests.md    # S5: executable requests
├── 2.6d_test_exec.md        # S5: executor + bug bucket
├── 2.6e_rca.md              # S5: root cause → AMEND briefs
├── 2.7_review.md            # S6: review
├── 2.8_pr_run.md            # S7: commit, push, PRs
├── 2.4_pr.md                # Single-flow PR agent (sections reused by 2.8)
└── 2.5_e2e.md               # Single-flow HS → UCS → connector reference (cited by v2 files)
```

---

See [setup.md](setup.md) for the **legacy** paths — the `grace` CLI (`grace techspec`), its API keys and the
`.gracerules` prompts. A v2 run needs none of that: `2.2_techspec.md` falls back to Claude-native generation when
`grace/.env` is unconfigured. For a v2 run, use *Before your first run* above.

`grace-workspace/` is a separate TypeScript engine (dashboard + checkpoints) that drives the single-flow files
`2.1`–`2.4` on its own; it does not run the v2 pipeline.
