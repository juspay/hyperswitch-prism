# Parity Autopilot Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL — use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Tasks use checkbox (`- [ ]`) syntax.

**Goal:** Build a heartbeat-driven autopilot that brings hyperswitch-prism (UCS) into response parity with hyperswitch (oracle) by walking GitHub issue trees, classifying root cause, fixing Rust code in the correct repo, verifying observable behavior on the gRPC wire, and opening draft PRs — one leaf per heartbeat.

**Architecture:** New `packages/parity-core` TypeScript package alongside Byne. Reuses Byne's `StateManager` (for run/heartbeat metadata), `runAI` (for opencode/claude-code LLM subagents), and `SessionManager` (for prism worktrees). Adds its own GitHub state machine, dashboard renderer, gRPC verification harness, and orchestrator. **GitHub labels are the source of truth; local SQLite is a cache.**

**Tech Stack:** TypeScript 5 / Node 18+ / pnpm 9 (workspace member); `gh` CLI for GitHub; `git`, `cargo`, `grpcurl`, `jq` shelled via `execa`; SQLite via Byne's existing `better-sqlite3`.

---

## Context

The user has typed out the full Parity Autopilot spec in chat. Locked decisions from clarification round:

1. **Runtime target:** Local Mac, parameterized paths. No `/home/grace/` hardcoding. All paths come from `config.yml` with env-var overrides.
2. **Wiring:** New `packages/parity-core` package. Reuse `StateManager`, `runAI`, `SessionManager` from `packages/core` (export them through a barrel). New custom orchestrator — not the existing `PipelineEngine` (heartbeat semantics differ from one-shot pipelines).
3. **Scope:** Full pipeline end-to-end — discovery, dashboard, label state-machine, claim, understand/plan/execute (prism + hs-bridge loci), validate, gRPC verify, handoff, sweep, escalation.
4. **Agent runtime:** Reuse `runAI` (opencode / claude-code) for UNDERSTAND, PLAN, EXECUTE phases with tool access.

### Confirmed structural facts (from exploration)

| Fact | Value |
|---|---|
| Prism repo root (local) | `/Users/tushar.shukla/Downloads/Work/euler-ucs/hyperswitch-prism` |
| Connector transformers | `crates/integrations/connector-integration/src/connectors/<name>/transformers.rs` |
| connector-integration package | `cargo build -p connector-integration` |
| gRPC server crate path | `crates/grpc-server/grpc-server/` (nested) |
| gRPC server package | `cargo run -p grpc-server --release`, listens on `0.0.0.0:8000` (metrics on 8080) |
| Proto dir | `crates/types-traits/grpc-api-types/proto/` |
| Creds env var | `CONNECTOR_AUTH_FILE_PATH` (the gRPC server reads this) |
| Hyperswitch oracle | **Not present locally** — must be cloned by operator; bridge file `crates/external_services/src/grpc_client/unified_connector_service.rs` lives in that clone |
| Issue tracker root | `https://github.com/juspay/hyperswitch-cloud/issues/15576` |

### Constraints inherited from spec (non-negotiable)

1. **Hyperswitch is the oracle.** Prism follows hyperswitch. Read-only by default. Only `crates/external_services/src/grpc_client/unified_connector_service.rs` and its private supporting types are editable in the oracle repo — and only when the leaf body declares `target: hyperswitch-bridge` AND classification puts the root cause there.
2. **Forbidden hyperswitch edit surfaces:** `hyperswitch_connectors/`, `hyperswitch_domain_models/`, `api_models/`, `router/`.
3. **No `crates/types-traits/` edits** in prism without explicit human approval.
4. **One leaf = one PR.** Draft PRs only.
5. **No PR without gRPC verification PASS.** Build-only is insufficient.
6. **Never skip hooks** (`--no-verify`, `--no-gpg-sign`). On hook failure, fix the cause and create a NEW commit.
7. **Label + comment = audit log.** No silent transitions.
8. **Idempotent claims.** Re-fetch before write; back off on race.
9. **No issue closure** — humans close.
10. **One work-pick per heartbeat.** Track many, fix one.

---

## File Structure

```
grace/grace-workspace/
├── config.yml                                    # extended with parityAutopilot section
└── packages/
    ├── core/                                     # existing Byne; export selected modules
    │   └── src/
    │       └── index.ts                          # MODIFY: add re-exports for parity-core to consume
    └── parity-core/                              # NEW package
        ├── package.json
        ├── tsconfig.json
        └── src/
            ├── index.ts                          # public API surface
            ├── config.ts                         # parityAutopilot config schema + loader
            ├── github/
            │   ├── client.ts                     # gh CLI wrapper (issue, label, comment, pr)
            │   ├── tree.ts                       # GraphQL sub-issue walker
            │   └── labels.ts                     # label state machine (6 labels, idempotent transitions)
            ├── dashboard/
            │   ├── renderer.ts                   # parity-dashboard.md + connectors/<name>.md
            │   └── derive.ts                     # status derivation from labels + linked-PRs
            ├── cache.ts                          # .cache/ JSON + log artifacts
            ├── locus.ts                          # locus classification + bridge gate
            ├── verify/
            │   ├── grpc.ts                       # grpcurl harness, server lifecycle, jq diff
            │   └── payloads.ts                   # extract payload from leaf body or fixture
            ├── phases/
            │   ├── understand.ts                 # runAI agent + ## Understanding Summary
            │   ├── plan.ts                       # runAI agent + ## Implementation Plan + confidence gate
            │   ├── execute.ts                    # runAI agent + git branch + cargo build/clippy/nextest
            │   ├── validate.ts                   # diff guard + forbidden-surface check
            │   ├── handoff.ts                    # git push + gh pr create --draft
            │   └── sweep.ts                      # poll authored PRs, transition labels
            ├── orchestrator.ts                   # heartbeat loop: discover→refresh→decide→…→sweep
            ├── escalation.ts                     # parity:blocked + structured comment
            ├── persistence.ts                    # extend StateManager schema (heartbeats, leaf_runs)
            └── cli.ts                            # `parity tick`, `parity loop`, `parity dashboard`, `parity status <N>`
        └── tests/                                # vitest
            ├── tree.test.ts
            ├── labels.test.ts
            ├── derive.test.ts
            ├── locus.test.ts
            ├── grpc-verify.test.ts               # uses test gRPC fixture
            └── sweep.test.ts
```

Files that change together live together — phases sit next to each other, GitHub plumbing is one folder, dashboard rendering another. No shared utilities folder; if something is used by two phases, put it in the closer phase's folder and import.

---

## Config Schema

Extend `grace/grace-workspace/config.yml`:

```yaml
parityAutopilot:
  # All paths absolute. Override per-machine via env vars (PARITY_*).
  prismPath: /Users/tushar.shukla/Downloads/Work/euler-ucs/hyperswitch-prism
  oracleReadOnlyPath: ""        # PARITY_ORACLE_PATH — empty means oracle access disabled
  bridgeWritePath: ""           # PARITY_BRIDGE_PATH — same clone as oracle, or separate; empty disables hs-bridge locus
  credsPath: ""                 # PARITY_CREDS_PATH — abs path to creds.json; passed via CONNECTOR_AUTH_FILE_PATH

  github:
    owner: juspay
    repo: hyperswitch-cloud
    rootIssue: 15576
    actor: "tushar-shukla"      # GitHub login; used to filter "PRs you authored" in sweep

  grpc:
    port: 8000                  # prism gRPC default
    metricsPort: 8080
    bootTimeoutMs: 30000        # wait this long for "gRPC server listening"
    callTimeoutMs: 60000

  cache:
    dir: .cache                 # relative to grace/grace-workspace root
    treeTtlMs: 21600000         # 6h — invalidate sub-issue tree cache

  llm:
    runner: "claude-code"       # or "opencode" — reuses Byne's runAI
    understandModel: ""         # falls back to top-level llm.model
    planModel: ""
    executeModel: ""

  heartbeat:
    pickOldestFirst: true       # FIFO by createdAt
    maxInflightClaimed: 1       # never hold >1 unfinished claim
    sweepStalePrDays: 7
    rcaStaleHours: 24

  rules:
    forbiddenOracleDirs:        # validation barrier; never edit these even if locus says oracle
      - "crates/hyperswitch_connectors/"
      - "crates/hyperswitch_domain_models/"
      - "crates/api_models/"
      - "crates/router/"
    forbiddenPrismDirs:
      - "crates/types-traits/"
```

Loader in `packages/parity-core/src/config.ts` reads `config.yml` via Byne's existing loader, validates `parityAutopilot.*`, applies env-var overrides, and throws if `prismPath` doesn't exist on disk.

---

## Database Schema (extends Byne's `state.ts`)

Add two tables in a new migration (v3):

```sql
CREATE TABLE IF NOT EXISTS parity_leaves (
  issue_number INTEGER PRIMARY KEY,
  parent_tracking INTEGER,                 -- connector tracking issue
  connector TEXT,
  flow TEXT,
  title TEXT,
  body TEXT,
  labels_json TEXT,
  linked_prs_json TEXT,
  status TEXT,                             -- no-pr | pr-open | pr-merged | blocked | superseded | not-applicable
  locus TEXT,                              -- prism-transformer | hs-bridge | hs-connector | ambiguous | null
  pr_repo TEXT,
  pr_number INTEGER,
  created_at INTEGER,
  updated_at INTEGER,
  last_seen_at INTEGER
);

CREATE TABLE IF NOT EXISTS parity_heartbeats (
  heartbeat_id TEXT PRIMARY KEY,
  started_at INTEGER,
  completed_at INTEGER,
  picked_leaf INTEGER,
  outcome TEXT,                            -- claimed | resumed | escalated | no-work | error
  outcome_detail TEXT
);
```

`parity_leaves` is a derived cache; on heartbeat refresh, rows are upserted from the GraphQL walk + label scan + linked-PR resolution. If a leaf disappears from the tree, the row stays (for audit) but its `last_seen_at` ages.

Migration registers as `v2 → v3` in `state.ts` migration list; `parity-core` imports `StateManager` and calls a new `extendForParity()` method that creates these tables idempotently.

---

## Heartbeat Algorithm

One heartbeat = one pass through these steps. Implemented in `orchestrator.ts:runHeartbeat()`. Each step has a hard timeout; if exceeded, the heartbeat ends with `outcome=error` and the next heartbeat starts fresh.

```
runHeartbeat():
  1. DISCOVER       — GraphQL-walk #15576 → leaves; resolve linked PRs; cache tree-YYYY-MM-DD.json
  2. REFRESH        — upsert parity_leaves; regenerate parity-dashboard.md + connectors/<name>.md atomically
  3. SWEEP          — for every pr-open leaf authored by us: check PR state; transition labels on merge; fix-forward typos; reminder >7d
  4. DECIDE         — pick oldest no-pr leaf NOT under autopilot-skip parent. If holding stale autopilot-claimed → resume that one instead.
  5. CLAIM          — `gh issue edit <N> --add-label parity:autopilot-claimed`; ack comment with prism+oracle SHAs; re-fetch; drop & retry on race.
  6. UNDERSTAND     — runAI agent reads prism+oracle, classifies locus, posts ## Understanding Summary. Gate: HIGH confidence, single locus checkbox, locus != hs-connector and != ambiguous. Else parity:blocked + stop.
  7. PLAN           — runAI agent writes ## Implementation Plan with exact diffs. Gate: 100% confidence. Else parity:blocked + stop. Add parity:rca-complete.
  8. EXECUTE        — branch off origin/main in correct repo per locus; apply edits exactly; cargo build / clippy / nextest. Tail captured.
  9. VALIDATE       — git diff --stat scoped to plan files; forbidden-surface check; commit format check.
 10. GRPC_VERIFY    — boot grpc-server with creds, replay payload via grpcurl, jq-diff targeted field, confirm post-fix function ran in logs. FAIL → return to UNDERSTAND, no PR.
 11. HANDOFF        — git push, gh pr create --draft with full body. On leaf: remove parity:autopilot-claimed, add parity:fix-pr-open, comment PR URL.
 12. EXIT           — no eligible leaf at step 4 → clean exit.
```

Each step is a free-standing function returning `{ ok: boolean, transition?: "blocked" | "next-heartbeat", artifacts: ... }`. The orchestrator chains them; any `ok=false` ends the heartbeat after the appropriate audit comment.

---

## Tasks

### Task 1 — Scaffold `packages/parity-core`

**Files:**
- Create: `packages/parity-core/package.json`
- Create: `packages/parity-core/tsconfig.json`
- Create: `packages/parity-core/src/index.ts`
- Modify: `packages/core/src/index.ts` (add barrel re-exports for `StateManager`, `runAI`, `loadConfig`, `SessionManager`)
- (No change needed to `pnpm-workspace.yaml` — already globs `packages/*`)

- [ ] **Step 1.1: Write `packages/parity-core/package.json`**

```json
{
  "name": "@byne/parity-core",
  "private": true,
  "version": "0.1.0",
  "main": "dist/index.js",
  "types": "dist/index.d.ts",
  "scripts": {
    "build": "tsc -p tsconfig.json",
    "dev": "tsc -p tsconfig.json --watch",
    "test": "vitest run",
    "typecheck": "tsc -p tsconfig.json --noEmit"
  },
  "dependencies": {
    "@byne/core": "workspace:*",
    "execa": "^9.0.0",
    "yaml": "^2.5.0"
  },
  "devDependencies": {
    "@types/node": "^20.0.0",
    "typescript": "^5.4.0",
    "vitest": "^1.6.0"
  }
}
```

- [ ] **Step 1.2: Write `packages/parity-core/tsconfig.json`** extending `../../tsconfig.base.json`, `rootDir: src`, `outDir: dist`, `composite: true`, `references: [{ "path": "../core" }]`.

- [ ] **Step 1.3: Write `packages/parity-core/src/index.ts`** — empty re-export shell to be filled later: `export * from './config'; export * from './orchestrator'; export * from './cli';`

- [ ] **Step 1.4: Confirm `packages/core/package.json` has `name: "@byne/core"`** — if not, rename it. Add re-exports in `packages/core/src/index.ts` for `StateManager`, `runAI`, `loadConfig`, `SessionManager`.

- [ ] **Step 1.5: Build the workspace**

Run: `pnpm install && pnpm -r build`
Expected: parity-core builds with empty surface, no type errors.

- [ ] **Step 1.6: Commit**

```bash
git add packages/parity-core packages/core/src/index.ts
git commit -m "feat(parity-core): scaffold package with @byne/core dependency"
```

---

### Task 2 — Config schema + loader

**Files:**
- Modify: `grace/grace-workspace/config.yml` (add `parityAutopilot:` block, see schema above)
- Create: `packages/parity-core/src/config.ts`
- Create: `packages/parity-core/tests/config.test.ts`

- [ ] **Step 2.1: Add `parityAutopilot` block to `config.yml`** with empty `oracleReadOnlyPath` and `bridgeWritePath` (operator fills via env or by editing).

- [ ] **Step 2.2: Implement loader in `config.ts`**

```ts
import { existsSync } from "node:fs";
import { getConfig as getBaseConfig } from "@byne/core";

export interface ParityConfig {
  prismPath: string;
  oracleReadOnlyPath: string;
  bridgeWritePath: string;
  credsPath: string;
  github: { owner: string; repo: string; rootIssue: number; actor: string };
  grpc: { port: number; metricsPort: number; bootTimeoutMs: number; callTimeoutMs: number };
  cache: { dir: string; treeTtlMs: number };
  llm: { runner: "claude-code" | "opencode"; understandModel: string; planModel: string; executeModel: string };
  heartbeat: { pickOldestFirst: boolean; maxInflightClaimed: number; sweepStalePrDays: number; rcaStaleHours: number };
  rules: { forbiddenOracleDirs: string[]; forbiddenPrismDirs: string[] };
}

export function loadParityConfig(): ParityConfig {
  const raw = (getBaseConfig() as any).parityAutopilot;
  if (!raw) throw new Error("config.yml: missing parityAutopilot section");

  const cfg: ParityConfig = {
    ...raw,
    prismPath: process.env.PARITY_PRISM_PATH ?? raw.prismPath,
    oracleReadOnlyPath: process.env.PARITY_ORACLE_PATH ?? raw.oracleReadOnlyPath ?? "",
    bridgeWritePath: process.env.PARITY_BRIDGE_PATH ?? raw.bridgeWritePath ?? "",
    credsPath: process.env.PARITY_CREDS_PATH ?? raw.credsPath ?? "",
  };

  if (!existsSync(cfg.prismPath)) throw new Error(`prismPath does not exist: ${cfg.prismPath}`);
  return cfg;
}
```

- [ ] **Step 2.3: Test** — write `config.test.ts` that sets `PARITY_PRISM_PATH` to `/tmp` and asserts override wins; sets it to `/no/such/dir` and asserts throw.

- [ ] **Step 2.4: Run tests**

Run: `pnpm -C packages/parity-core test`
Expected: 2 passes.

- [ ] **Step 2.5: Commit**

```bash
git add grace/grace-workspace/config.yml packages/parity-core/src/config.ts packages/parity-core/tests/config.test.ts
git commit -m "feat(parity-core): add parityAutopilot config + loader with env-var overrides"
```

---

### Task 3 — GitHub client + tree walker

**Files:**
- Create: `packages/parity-core/src/github/client.ts`
- Create: `packages/parity-core/src/github/tree.ts`
- Create: `packages/parity-core/src/cache.ts`
- Create: `packages/parity-core/tests/tree.test.ts`

- [ ] **Step 3.1: `client.ts` — thin `gh` wrapper**

Implement `ghJson<T>(args: string[]): Promise<T>` shelling `gh` via `execa`. One function per primitive: `getIssue(n)`, `addLabel(n, label)`, `removeLabel(n, label)`, `comment(n, body)`, `createPrDraft(opts)`, `viewPr(n, fields)`. Each takes a `repo` arg defaulting to `<owner>/<repo>` from config.

Key choice: shell to `gh` rather than use the GitHub REST API directly, because `gh` already handles auth, retries, and rate-limiting and matches the spec verbatim.

- [ ] **Step 3.2: `tree.ts` — GraphQL walker**

```ts
const TREE_QUERY = `
  query($owner:String!,$repo:String!,$num:Int!){
    repository(owner:$owner,name:$repo){
      issue(number:$num){
        number title state createdAt
        labels(first:50){ nodes{ name } }
        subIssues(first:100){ nodes { number title state url } }
      }
    }
  }`;

export interface Leaf {
  number: number;
  title: string;
  body: string;
  labels: string[];
  createdAt: string;
  linkedPRs: { repo: string; number: number; state: "open" | "merged" | "closed"; mergedAt?: string }[];
  parentTracking: number;
  connector: string;            // parsed from title or path
  flow: string;                 // parsed from title or path
}

export async function walkTree(rootIssue: number, cfg: ParityConfig): Promise<Leaf[]>;
```

Walk recursively until `subIssues.nodes` is empty (leaf). Fall back to parsing `- [ ] #N` / full URLs from issue body when native sub-issues are empty. Resolve linked PRs via a second GraphQL query against each leaf's `timelineItems(itemTypes: CROSS_REFERENCED_EVENT)`.

- [ ] **Step 3.3: `cache.ts` — atomic JSON cache**

`writeAtomic(path, obj)` writes `<path>.tmp` then `rename` — guarantees readers never see partial JSON. `readIfFresh(path, ttlMs)` returns `null` if stale or missing.

- [ ] **Step 3.4: Test the walker against a mocked `gh api graphql`**

In `tree.test.ts`, mock `execa` to return a canned 3-level tree (tracking → connector → leaf), assert flattening yields the expected leaves with `parentTracking` correctly inherited. Add a fallback test: native subIssues empty, body has `- [ ] juspay/hyperswitch-cloud#15741` — leaf detected.

- [ ] **Step 3.5: Run tests, then exercise live**

```bash
pnpm -C packages/parity-core test -- tree
# Live smoke test (requires gh auth):
node -e "import('./packages/parity-core/dist/index.js').then(m => m.walkTree(15576, m.loadParityConfig()).then(ls => console.log(ls.length, 'leaves')))"
```

Expected: leaves count > 0.

- [ ] **Step 3.6: Commit**

```bash
git add packages/parity-core/src/github packages/parity-core/src/cache.ts packages/parity-core/tests/tree.test.ts
git commit -m "feat(parity-core): GitHub tree walker with sub-issue + task-list fallback"
```

---

### Task 4 — Label state machine

**Files:**
- Create: `packages/parity-core/src/github/labels.ts`
- Create: `packages/parity-core/tests/labels.test.ts`

- [ ] **Step 4.1: Define labels + transitions**

```ts
export const LABELS = {
  CLAIMED: "parity:autopilot-claimed",
  RCA_DONE: "parity:rca-complete",
  PR_OPEN:  "parity:fix-pr-open",
  MERGED:   "parity:fix-merged",
  BLOCKED:  "parity:blocked",
  SKIP:     "parity:autopilot-skip",
} as const;

// Every transition: (issueNumber, fromLabel?, toLabel, auditComment) → re-fetch → mutate → re-fetch.
export async function transition(opts: {
  issue: number;
  remove?: string[];
  add: string[];
  comment: string;
}): Promise<{ raced: boolean }>;
```

Implementation:
1. `getIssue(n)` to fetch current labels.
2. If `add` includes `CLAIMED` and current labels already include it AND the comment author of the last claim comment is not us → return `{ raced: true }` (someone else claimed it).
3. `gh issue edit --add-label … --remove-label …` (one call, multiple flags).
4. `gh issue comment --body …` — body is required; "label change + comment = audit log".
5. `getIssue(n)` again; if expected labels not present → return `{ raced: true }`.

- [ ] **Step 4.2: Tests**

Mock `gh issue view` / `gh issue edit` / `gh issue comment` via `execa` stubs. Assert: (a) happy-path adds expected label, posts comment; (b) race detected when claim already held; (c) no transition without a comment body throws synchronously.

- [ ] **Step 4.3: Commit**

```bash
git add packages/parity-core/src/github/labels.ts packages/parity-core/tests/labels.test.ts
git commit -m "feat(parity-core): idempotent label state machine with race detection"
```

---

### Task 5 — Status derivation + dashboard renderer

**Files:**
- Create: `packages/parity-core/src/dashboard/derive.ts`
- Create: `packages/parity-core/src/dashboard/renderer.ts`
- Create: `packages/parity-core/tests/derive.test.ts`

- [ ] **Step 5.1: `derive.ts`**

```ts
export type LeafStatus = "no-pr" | "pr-open" | "pr-merged" | "blocked" | "superseded" | "not-applicable";
export type Locus = "prism-transformer" | "hs-bridge" | "hs-connector" | "ambiguous" | null;

export function deriveStatus(leaf: Leaf): LeafStatus {
  if (leaf.labels.includes(LABELS.MERGED) || leaf.linkedPRs.some(p => p.state === "merged")) return "pr-merged";
  if (leaf.labels.includes(LABELS.PR_OPEN) || leaf.linkedPRs.some(p => p.state === "open")) return "pr-open";
  if (leaf.labels.includes(LABELS.BLOCKED)) return "blocked";
  if (leaf.labels.includes(LABELS.SKIP)) return "not-applicable";
  return "no-pr";
}

export function deriveLocus(leaf: Leaf): Locus {
  // Parsed from latest "## Understanding Summary" comment's locus checkbox.
  // Fall back to null if no understanding posted yet.
}
```

Backfill out-of-sync labels: if PR is merged but `MERGED` label missing → return a `pendingLabelFix` that the orchestrator applies.

- [ ] **Step 5.2: `renderer.ts`**

Two functions:
- `renderDashboard(leaves: Leaf[]): string` → markdown for `parity-dashboard.md`. Connector-by-connector counts table + FIFO backlog priority list.
- `renderConnector(name: string, leaves: Leaf[]): string` → markdown for `connectors/<name>.md`. Per-leaf row table + escalations section.

Both use `writeAtomic`. Files written to `$GRACE_WORKSPACE` root (default `grace/grace-workspace/` from config) and `grace/grace-workspace/connectors/`.

- [ ] **Step 5.3: Tests**

`derive.test.ts`: small set of synthetic `Leaf`s covering each status branch + a locus-parse case (one with `[x] Prism transformer` in a comment block).

- [ ] **Step 5.4: Commit**

```bash
git add packages/parity-core/src/dashboard packages/parity-core/tests/derive.test.ts
git commit -m "feat(parity-core): dashboard renderer + status/locus derivation"
```

---

### Task 6 — Locus classification + bridge gate

**File:**
- Create: `packages/parity-core/src/locus.ts`
- Create: `packages/parity-core/tests/locus.test.ts`

- [ ] **Step 6.1: Implement bridge gate**

```ts
export type LocusTarget = "prism" | "hyperswitch-bridge" | "escalate";

export function classifyTarget(opts: {
  declaredTarget?: "prism" | "hyperswitch-bridge";   // from leaf body if present
  understoodLocus: Locus;                            // from agent's checkbox
  bridgeAvailable: boolean;                          // cfg.bridgeWritePath set?
}): { target: LocusTarget; reclassified: boolean; reason?: string };
```

Rules:
- `understoodLocus === "hs-connector"` → `escalate` (oracle disagreement; never edit).
- `understoodLocus === "ambiguous"` → `escalate`.
- `understoodLocus === "prism-transformer"` → `prism`.
- `understoodLocus === "hs-bridge"`:
  - If `bridgeAvailable` → `hyperswitch-bridge`.
  - Else → `escalate` with reason "bridge path not configured".
- If `declaredTarget` disagrees with `understoodLocus` → `reclassified=true`, trust the understood locus, instruct orchestrator to post `## Re-classification` comment.

- [ ] **Step 6.2: Forbidden-surface checks**

```ts
export function validateDiff(opts: {
  changedFiles: string[];
  target: LocusTarget;
  rules: ParityConfig["rules"];
}): { ok: boolean; violations: string[] };
```

- target=prism: every changed path must start with `crates/integrations/connector-integration/` and none with `crates/types-traits/`.
- target=hyperswitch-bridge: every changed path must be `crates/external_services/src/grpc_client/unified_connector_service.rs` (or under a supporting types module in the same crate); none under `forbiddenOracleDirs`.

- [ ] **Step 6.3: Tests** — both rules with synthetic file lists.

- [ ] **Step 6.4: Commit**

```bash
git add packages/parity-core/src/locus.ts packages/parity-core/tests/locus.test.ts
git commit -m "feat(parity-core): locus classification + forbidden-surface guard"
```

---

### Task 7 — UNDERSTAND / PLAN / EXECUTE via runAI

**Files:**
- Create: `packages/parity-core/src/phases/understand.ts`
- Create: `packages/parity-core/src/phases/plan.ts`
- Create: `packages/parity-core/src/phases/execute.ts`
- Create: `packages/parity-core/src/prompts/{understand,plan,execute}.ts`

- [ ] **Step 7.1: System prompts**

Each prompt is a long string literal mirroring the spec's Step 5 / Step 6 / Step 7 templates. Critical bits per prompt:

- **understand:** "You may read prism + oracle but NEVER edit them in this phase. Output ONLY a markdown block starting with `## Understanding Summary`. The locus checkbox must have exactly one `[x]`. If you can't reach HIGH confidence, output a `## Need Info` block instead — do not guess."
- **plan:** "Output ONLY a markdown block starting with `## Implementation Plan`. Show current code (quoted) and proposed code (full new function body). Include cargo verification commands. If you have any `TBD`, `TODO`, or 'similar to' — output `## Still Missing` instead. 100% confidence or nothing."
- **execute:** "Apply the plan exactly. You may run `cargo build -p connector-integration`, `cargo clippy -p connector-integration -- -D warnings`, `cargo nextest run -p connector-integration`. Stop immediately if any of these fail and report the tail. Do NOT change files outside the plan's named paths."

- [ ] **Step 7.2: Implement phase wrappers**

Each phase:
```ts
export async function runUnderstand(ctx: HeartbeatCtx): Promise<UnderstandResult> {
  const result = await runAI(/* task surrogate */, {
    system: UNDERSTAND_SYSTEM,
    user: buildUnderstandUser(ctx.leaf, ctx.cfg),
    label: `parity/understand/${ctx.leaf.number}`,
    sessionId: ctx.sessionIds.understand,    // resume across heartbeats
  });

  const parsed = parseUnderstandingSummary(result.output);
  if (!parsed.ok) return { ok: false, escalation: parsed.reason };
  await github.comment(ctx.leaf.number, parsed.markdown);
  return { ok: true, locus: parsed.locus, confidence: parsed.confidence };
}
```

- `runAI` and `friendlySessionName` come from `@byne/core`.
- `sessionIds.understand` is persisted in `parity_leaves.metadata` so the same Claude conversation resumes if the leaf is re-entered.

- [ ] **Step 7.3: Gates**

- Understand gate: `confidence === "HIGH"` AND exactly one locus checkbox AND locus ∉ {"hs-connector","ambiguous"}. Else → escalate.
- Plan gate: 100% confidence sentinel present AND no `Still Missing` block AND every code block has a file path. Else → escalate.

- [ ] **Step 7.4: Execute wrapping**

```ts
export async function runExecute(ctx, plan): Promise<ExecuteResult> {
  const target = classifyTarget(...);
  if (target.target === "escalate") return { ok: false, escalation: ... };

  const repoRoot = target.target === "prism" ? cfg.prismPath : cfg.bridgeWritePath;
  await git.fetch(repoRoot);
  const branch = `parity/${ctx.leaf.connector}/${ctx.leaf.flow}-${slug(ctx.leaf.title)}`;
  await git.checkoutNewBranch(repoRoot, branch, "origin/main");

  // Hand the plan to the agent; it does the writes + cargo runs.
  const result = await runAI(..., {
    system: EXECUTE_SYSTEM,
    user: buildExecuteUser(plan, repoRoot, target.target),
    sessionId: ctx.sessionIds.execute,
  });

  const diff = await git.diffStat(repoRoot);
  const violations = validateDiff({ changedFiles: diff.files, target: target.target, rules: cfg.rules });
  if (!violations.ok) return { ok: false, escalation: `forbidden surface: ${violations.violations.join(",")}` };

  // Re-run cargo deterministically (not just trusting the agent).
  const build = await cargo.build(repoRoot, target.target);
  const clippy = await cargo.clippy(repoRoot, target.target);
  const tests  = await cargo.nextest(repoRoot, target.target);

  return { ok: build.ok && clippy.ok && tests.ok, branch, diff, tails: { build, clippy, tests } };
}
```

★ Insight ─────────────────────────────────────
Even though the LLM agent runs cargo itself, the orchestrator re-runs it after the agent finishes. Two reasons: (1) the agent's transcript can lie or be truncated; (2) tails captured deterministically by the orchestrator are what go into the PR body. Trust but verify is structural here.
─────────────────────────────────────────────────

- [ ] **Step 7.5: Commit**

```bash
git add packages/parity-core/src/phases packages/parity-core/src/prompts
git commit -m "feat(parity-core): understand/plan/execute phases via runAI with strict gates"
```

---

### Task 8 — gRPC verification harness

**Files:**
- Create: `packages/parity-core/src/verify/grpc.ts`
- Create: `packages/parity-core/src/verify/payloads.ts`
- Create: `packages/parity-core/tests/grpc-verify.test.ts`

This is the **load-bearing gate** that distinguishes Parity Autopilot from a build-only CI bot. No PR without PASS.

- [ ] **Step 8.1: Server lifecycle**

```ts
export async function bootGrpcServer(cfg: ParityConfig): Promise<GrpcServerHandle> {
  const logPath = `${cfg.cache.dir}/grpc-server-${ctx.leaf.number}.log`;
  const child = execa("cargo", ["run", "-p", "grpc-server", "--release"], {
    cwd: cfg.prismPath,
    env: { ...process.env, CONNECTOR_AUTH_FILE_PATH: cfg.credsPath },
  });

  child.stdout?.pipe(createWriteStream(logPath, { flags: "a" }));
  child.stderr?.pipe(createWriteStream(logPath, { flags: "a" }));

  await waitForLogLine(logPath, /gRPC server listening/, cfg.grpc.bootTimeoutMs);
  return { pid: child.pid!, logPath, child };
}

export async function teardown(h: GrpcServerHandle): Promise<void> {
  h.child.kill("SIGTERM");
  await h.child.catch(() => {}); // ignore non-zero from SIGTERM
}
```

- [ ] **Step 8.2: Payload extraction**

`extractPayload(leaf: Leaf, cfg)`:
1. First try parsing a fenced ` ```json ... ``` ` block in the leaf body.
2. Fallback: look up a fixture under `cfg.prismPath/crates/grpc-server/grpc-server/tests/fixtures/<connector>/<flow>.json`.
3. If neither → escalate.

- [ ] **Step 8.3: grpcurl replay + jq diff**

```ts
export async function replay(opts: {
  endpoint: string;                     // `localhost:8000`
  protoImportPath: string;              // crates/types-traits/grpc-api-types/proto
  protoFile: string;                    // e.g. payment.proto
  service: string;                      // ucs.PaymentService
  rpc: string;                          // Authorize
  payload: unknown;
  logPath: string;
}): Promise<{ raw: any; logTail: string }> {
  const payloadFile = await writeTmpJson(opts.payload);
  const res = await execa("grpcurl", [
    "-plaintext",
    "-d", `@${payloadFile}`,
    "-import-path", opts.protoImportPath,
    "-proto", opts.protoFile,
    opts.endpoint,
    `${opts.service}/${opts.rpc}`,
  ]);
  await appendFile(opts.logPath, `\n--- grpcurl stdout ---\n${res.stdout}\n--- stderr ---\n${res.stderr}\n`);
  return { raw: JSON.parse(res.stdout), logTail: res.stdout };
}

export function diffField(observed: any, expected: any, path: string): { match: boolean; observedVal: any; expectedVal: any };
```

Use jq-style path traversal (split `path` on `.` then walk objects/arrays). Only diff the field(s) named in the leaf body — other fields may legitimately differ.

- [ ] **Step 8.4: Post-fix log inspection**

Add a `tracing::debug!("parity:<flow>:<field> hit")` in the changed function (allowed in-scope per spec). After replay, grep `logPath` for this string. If missing → FAIL (the patched code didn't run).

- [ ] **Step 8.5: Compose `verifyLeaf(leaf, ctx)` returning the markdown block for the PR/issue comment**

Failure ladder:
1. Server didn't boot in `bootTimeoutMs` → FAIL ("boot timeout"), abort, return to UNDERSTAND.
2. grpcurl returns non-zero → FAIL ("rpc error"), include stderr tail.
3. Targeted field doesn't match → FAIL ("oracle divergence"), return to UNDERSTAND.
4. Log doesn't show post-fix line → FAIL ("code path not executed").
5. All green → PASS.

- [ ] **Step 8.6: Tests**

Stub `execa` with canned outputs for grpcurl. Cases:
- Happy path: targeted field matches oracle, log shows hit line → PASS.
- Field mismatch → FAIL with clear reason.
- Boot timeout → FAIL.

(No live gRPC test — that runs in the live smoke described in Task 11.)

- [ ] **Step 8.7: Commit**

```bash
git add packages/parity-core/src/verify packages/parity-core/tests/grpc-verify.test.ts
git commit -m "feat(parity-core): gRPC verification harness with boot/replay/jq-diff/log-grep"
```

---

### Task 9 — Handoff (draft PR) + sweep

**Files:**
- Create: `packages/parity-core/src/phases/handoff.ts`
- Create: `packages/parity-core/src/phases/sweep.ts`
- Create: `packages/parity-core/tests/sweep.test.ts`

- [ ] **Step 9.1: `handoff.ts`**

```ts
export async function runHandoff(ctx, execResult, verifyResult): Promise<HandoffResult> {
  await git.commitAll(execResult.repoRoot, `parity(${connector}/${flow}): ${oneLiner} (#${leaf.number})`);
  await git.push(execResult.repoRoot, execResult.branch);

  const body = renderPrBody({
    leaf, plan, execResult, verifyResult,
    acceptance: [
      "Build passes",
      "Clippy passes",
      "Tests pass",
      "gRPC verification PASS (target field matches oracle, logs show post-fix code path)",
      "Shadow-replay produces zero diff (human verification gate)",
    ],
  });

  const targetRepo = execResult.target === "prism" ? "juspay/hyperswitch-prism" : "juspay/hyperswitch";
  const prUrl = await gh.createPrDraft({
    repo: targetRepo,
    title: `parity(${connector}/${flow}): ${oneLiner} (#${leaf.number})`,
    body,
    head: execResult.branch,
    base: "main",
    draft: true,
  });

  await transition({
    issue: leaf.number,
    remove: [LABELS.CLAIMED],
    add: [LABELS.PR_OPEN],
    comment: `Draft PR opened: ${prUrl}`,
  });

  return { prUrl };
}
```

★ Insight ─────────────────────────────────────
Note `git.commitAll` — never `--no-verify`, never `--no-gpg-sign`. If a hook fails, the spec says "fix the cause and create a NEW commit" — so the orchestrator must NOT silently retry the commit; it must return failure, post `## Hook Failure` on the issue, and let the heartbeat exit. Subsequent heartbeats can re-enter the leaf cleanly because the branch is local-only until push.
─────────────────────────────────────────────────

- [ ] **Step 9.2: `sweep.ts`**

For every leaf where `status === "pr-open"` AND the linked PR's author is `cfg.github.actor`:
1. `gh pr view <N> --json mergedAt,state,statusCheckRollup,headRefName`.
2. Merged → label transition `PR_OPEN → MERGED`, comment merge URL.
3. CI failed AND failure is fmt/clippy/typo (parsed from check names) → run the fix-forward path (regenerate via execute phase scoped to the failing rule). Otherwise → `BLOCKED` + comment.
4. Open >7d, CI green, no merge → polite reminder comment, no destructive action.

For every leaf where `labels.includes(CLAIMED)` AND no linked PR:
- Look at last `## Understanding Summary` comment timestamp. If `>rcaStaleHours` and no `## Implementation Plan` → re-enter PLAN phase next heartbeat (heartbeat orchestrator records this in `parity_leaves.metadata.resume_at`).
- If no understanding summary → re-enter UNDERSTAND.

- [ ] **Step 9.3: Tests**

Sweep test cases:
- PR merged but `MERGED` label missing → fix the label, no destructive action.
- PR open with clippy failure → fix-forward branch triggered.
- PR open 8d, green → reminder comment posted exactly once (idempotent — check if reminder comment already exists).

- [ ] **Step 9.4: Commit**

```bash
git add packages/parity-core/src/phases/handoff.ts packages/parity-core/src/phases/sweep.ts packages/parity-core/tests/sweep.test.ts
git commit -m "feat(parity-core): PR handoff with structured body + sweep of in-flight PRs"
```

---

### Task 10 — Orchestrator + escalation

**Files:**
- Create: `packages/parity-core/src/orchestrator.ts`
- Create: `packages/parity-core/src/escalation.ts`
- Create: `packages/parity-core/src/persistence.ts`

- [ ] **Step 10.1: `persistence.ts`** — `extendForParity(state: StateManager)` adds the two tables described above. Helpers: `upsertLeaf`, `getLeaf`, `recordHeartbeat`.

- [ ] **Step 10.2: `escalation.ts`**

```ts
export async function escalate(opts: {
  leaf: Leaf;
  step: "understand" | "plan" | "execute" | "validate" | "grpc-verify";
  blocker: string;
  tried: string;
  question: string;
  cc?: string;
}): Promise<void>;
```

Posts a `## Autopilot Escalation` comment matching the spec template, then applies `parity:blocked`.

- [ ] **Step 10.3: `orchestrator.ts`**

```ts
export async function runHeartbeat(cfg: ParityConfig): Promise<HeartbeatOutcome> {
  const state = await openParityState();
  const hbId = newHeartbeatId();
  recordHeartbeatStart(state, hbId);

  try {
    const leaves = await discoverAndCache(cfg);
    await refreshDashboard(cfg, leaves, state);
    await runSweep(cfg, leaves, state);

    const pick = decideNextLeaf(leaves, cfg, state);
    if (!pick) return finish(state, hbId, "no-work");

    const claimResult = await claim(pick, cfg);
    if (claimResult.raced) return finish(state, hbId, "raced");

    const understand = await runUnderstand({ leaf: pick, cfg, state, sessionIds: pick.sessions });
    if (!understand.ok) { await escalate({ leaf: pick, step: "understand", ...understand.escalation }); return finish(state, hbId, "escalated"); }

    const plan = await runPlan({ leaf: pick, cfg, understand });
    if (!plan.ok) { await escalate({ leaf: pick, step: "plan", ...plan.escalation }); return finish(state, hbId, "escalated"); }

    const exec = await runExecute({ leaf: pick, cfg, plan });
    if (!exec.ok) { await escalate({ leaf: pick, step: "execute", ...exec.escalation }); return finish(state, hbId, "escalated"); }

    const verify = await verifyLeaf(pick, { cfg, exec });
    if (!verify.ok) { return finish(state, hbId, "verify-failed"); } // re-enters next heartbeat — no PR

    await runHandoff({ leaf: pick, cfg, exec, verify });
    return finish(state, hbId, "pr-opened");
  } catch (err) {
    return finish(state, hbId, "error", String(err));
  }
}

export async function runLoop(cfg: ParityConfig, intervalMs: number): Promise<never> {
  while (true) {
    await runHeartbeat(cfg);
    await sleep(intervalMs);
  }
}
```

- [ ] **Step 10.4: Decide policy in `decideNextLeaf`**

1. Look for in-flight `parity:autopilot-claimed` we own → resume that one (return the leaf).
2. Else filter to `status === "no-pr"`, not under a `parity:autopilot-skip` parent connector, sort by `createdAt` ascending, take first.
3. None → return null.

`maxInflightClaimed` (default 1) caps how many open claims we tolerate at once.

- [ ] **Step 10.5: Commit**

```bash
git add packages/parity-core/src/orchestrator.ts packages/parity-core/src/escalation.ts packages/parity-core/src/persistence.ts
git commit -m "feat(parity-core): heartbeat orchestrator + escalation + persistence"
```

---

### Task 11 — CLI + live smoke test

**Files:**
- Create: `packages/parity-core/src/cli.ts`
- Modify: `packages/cli/src/index.ts` (register `parity` subcommand group)

- [ ] **Step 11.1: CLI commands**

```ts
// `byne parity tick`        — run one heartbeat, log outcome, exit
// `byne parity loop --interval 5m`  — run forever, sleep between heartbeats
// `byne parity dashboard`   — refresh dashboard files only (no claim, no edits)
// `byne parity status <N>`  — print all known state for one leaf (DB + GitHub re-fetch)
// `byne parity sweep`       — run sweep phase only (PR polling, label fixes, reminders)
```

Wire as subcommands under `byne` via commander/yargs (whatever `packages/cli` uses today).

- [ ] **Step 11.2: Smoke checklist (manual, against live GitHub + local prism)**

Prerequisites:
- `gh auth status` is logged in.
- `PARITY_CREDS_PATH` exports a path to a real `creds.json`.
- `cargo --version` works; `grpcurl --version` works.

Steps:
1. `byne parity dashboard` — confirm `grace/grace-workspace/parity-dashboard.md` and `grace/grace-workspace/connectors/<name>.md` files are produced and look right.
2. `byne parity tick` — confirm it picks an oldest no-pr leaf, posts `## Understanding Summary`, gates correctly, and either escalates or proceeds.
3. If it proceeds: confirm a branch is created in `cfg.prismPath`, cargo runs, gRPC server boots, grpcurl replay runs, comparison happens, and (on PASS) a draft PR is opened with the correct body.
4. `byne parity sweep` — separately, point this at an already-merged PR with missing `parity:fix-merged` label to confirm the backfill works.

- [ ] **Step 11.3: Commit**

```bash
git add packages/parity-core/src/cli.ts packages/cli/src/index.ts
git commit -m "feat(parity-core): CLI surface (tick/loop/dashboard/status/sweep)"
```

---

## Verification

### Unit tests (vitest in `packages/parity-core/tests/`)

| Module | Coverage |
|---|---|
| `config` | env-var override, missing path throws |
| `tree` | 3-level walk + task-list fallback + linked-PR resolution |
| `labels` | claim, race detection, missing comment throws |
| `derive` | each LeafStatus branch, locus parsing |
| `locus` | each rule + reclassification + forbidden-surface guard |
| `grpc-verify` | boot timeout, field match, field mismatch, log-grep miss |
| `sweep` | merged-label backfill, fix-forward dispatch, idempotent reminder |

Run all: `pnpm -C packages/parity-core test`.

### End-to-end (manual)

Follow the smoke checklist in Task 11.2 against the real `juspay/hyperswitch-cloud#15576` tree and the local prism. Pick a leaf with a known prism-transformer fix where the oracle behavior is unambiguous (find one from `git log --grep parity origin/main` if any have already landed).

### Non-goals / explicit out-of-scope

- Filing new parity issues (validation-service does that).
- Running shadow-replay harness (humans).
- Promoting PRs, merging, closing issues (humans).
- New connector scaffolding (covered by `new-connector` skill).
- Editing hyperswitch outside `crates/external_services/src/grpc_client/unified_connector_service.rs`.
- Multi-user concurrency (one operator per `cfg.github.actor`).

---

## Open Questions / Pre-requisites

1. **Hyperswitch oracle clone:** does not exist locally. Before any `hs-bridge` work, operator must clone `juspay/hyperswitch` somewhere and set `PARITY_ORACLE_PATH` + `PARITY_BRIDGE_PATH` (often the same path). Until then, autopilot escalates all `hs-bridge` locus leaves.
2. **`tracing::debug!` insertion:** allowed by spec ("a permitted in-scope edit"). Plan assumes the EXECUTE agent adds it; if the changed function already has a tracing line, reuse it.
3. **gRPC payload fixtures:** spec says "from the leaf issue body, or the validation-service fixture". This plan supports both; if neither exists, the leaf escalates. Future: a `parity-fixtures/` directory keyed by connector × flow.
4. **GitHub rate limits:** GraphQL costs are bounded (~100 sub-issues per page). Heartbeat cadence ≥ 5 min keeps us well inside limits.
5. **Resumability across heartbeats:** session IDs for understand/plan/execute are stored in `parity_leaves.metadata` so the same Claude conversation resumes. Verify Byne's `runAI` supports cross-process resume of the same session ID (Phase 12 persistence).

---

## Self-review checklist

- [x] Spec coverage: discovery, dashboard, label state-machine, claim, understand, plan, execute, validate, grpc-verify, handoff, sweep, escalation — all mapped to tasks.
- [x] No placeholders: every task has exact file paths, concrete code shapes, exact commands.
- [x] Type consistency: `Leaf`, `LeafStatus`, `Locus`, `LocusTarget`, `ParityConfig` all defined once and referenced consistently.
- [x] Bridge gate is encoded structurally (locus.ts + validateDiff) — not just documentation.
- [x] gRPC verification is a hard gate, not a nice-to-have.
- [x] All transitions go through `transition()` (label + comment together) — no silent label edits possible.
