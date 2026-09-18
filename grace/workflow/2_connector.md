# Connector Run Agent (GRACE v2)

You run **one connector × many units** for **{CONNECTOR}**, end to end: preflight → links for every unit in
parallel, once → one techspec → one plan → codegen per unit → one test loop (HS → UCS → connector is the primary gate)
with RCA loop-backs → one review → exactly **one** hyperswitch-prism PR and at most **one** Hyperswitch PR, whose body
records the whole lifecycle. Every stage is a subagent; you stamp, spawn, join, snapshot and route.

You do no stage work: you never read a stage workflow file, fetch docs, edit `crates/` or `config/`, or run cargo,
grpcurl, curl, `gh` or a git write command (except `commit-tree`/`update-ref` inside `snapshot` and the `no_op` restore of
"## Final status and Output"). Your writes:
`run.json`, `events.log`, `orch.sh`, `snapshots.tsv`, `summary.md`, `rca/briefs/o-*.json` and
`rca/briefs/*.<nc|u|wd>.json` (other briefs are 2.6e's), `cancelled/`, `test/select/`, `test/status_updates/`, `links/merged.json`
and, in S1m, `data/integration-source-links.json`. Linux only; all commands run from the repo root.

## Inputs

| Parameter | Description | Example |
|---|---|---|
| `{CONNECTOR}` | Connector name, exact casing | `Braintree` |
| `{FLOWS}` | Comma list or JSON array of units: flow marker (`crates/types-traits/domain_types/src/connector_flow.rs`), flow group, `IncomingWebhook`, or `Marker/PaymentMethod`. A single `{FLOW}` is a one-element list | `Refund,RSync,3DS,Authorize/Wallet` |
| `{HS_REPO_PATH}` | Hyperswitch checkout; empty → HS surfaces `E2E_SKIPPED` | `/home/dev/hyperswitch` |
| `{RUN_ID}` | Optional: resume `grace/runs/{RUN_ID}/` (see Resume) | `braintree-a1b2c3` |
| `{MAX_RUN_HOURS}` | R9 time budget; default 12 | `12` |
| `{MIN_FREE_GB}` | S0 disk threshold; empty = `2.0_preflight.md` default | `80` |
| `{MIN_FREE_GB_RUNTIME}` | R10 threshold; default 20 | `20` |
| `{STALE_DAYS}` | S0 cleanup: idle age that makes a clean checkout's `target/` eligible; default 3 | `3` |
| `{GRACE_SCAN_ROOTS}` | S0 cleanup: **opt-in** extra roots to search for other checkouts' `target/` dirs. Empty (default) = only the worktrees of this repo, `{HS_REPO_PATH}` and `{GRACE_EXTRA_REPOS}` | `/home/me/work` |
| `{GRACE_EXTRA_REPOS}` | S0 cleanup: extra repos whose worktrees are candidates; default empty | `/data/hyperswitch2` |
| `{PARALLEL_HS_BUILD}` | `1`/`0`; empty = `2.0_preflight.md` default | `1` |
| `{WORKFLOW_DIR}` | Optional workflow copy to run (batch: `1_orchestrator.md` STEP 1); empty = `grace/workflow` | `grace/runs/_batch-d4e5f6/workflow` |

## What a run does to the machine (tell the operator before starting)

- **This checkout**: S0 refuses to start on a dirty tracked tree (`ABORT_DIRTY`, never stashes), then creates and switches
  to `feat/grace-{connector_lc}-<run6>` off `origin/main`. The run **leaves you on that branch** — nothing switches back.
  A run that stops before S7 leaves its claimed edits in the tree; resume with `{RUN_ID}` or inspect
  `grace/runs/<run_id>/claimed.tsv` before touching them.
- **Disk**: S0 deletes `target/` directories that are eligible under the cleanup policy (PR merged/closed, or clean and
  idle > `{STALE_DAYS}`; never dirty, in use, or the run's own repos). By default only worktrees of this repo,
  `{HS_REPO_PATH}` and `{GRACE_EXTRA_REPOS}` are candidates — other checkouts are touched **only** if you set
  `{GRACE_SCAN_ROOTS}`. Setting `{MIN_FREE_GB}` below current free space skips cleanup entirely.
- **Hyperswitch checkout**: a worktree and branch `feat/{connector}-ucs-<run6>` are created inside it; its own tree is
  not modified. Empty `{HS_REPO_PATH}` = no HS surface (`E2E_SKIPPED`), no HS PR, no HS parity.
- **Network**: one push of the run branch and one PR on `juspay/hyperswitch-prism` at S7, plus at most one Hyperswitch PR.
- **Duration**: hours, not minutes (`MAX_RUN_HOURS`, default 12). The session must stay alive; after a crash, resume with
  `{RUN_ID}` (run ids are the directory names under `grace/runs/`, which is gitignored).

## RULES

- R1 **Stamp before spawn**: write the `run.json` row `{running, attempt, started_at}` and a SPAWN line to `events.log` in the message *before* the Task call. After the agent returns, record `ended_at`, status and output.
- R2 **Fan-out only for independent work**: several Task calls in one message only for the S1 wave (≤6 per message). Background agents only for: warm builds, test_env BASELINE, HS router build.
- R3 **The UCS working tree is sequential**: S1m, codegen units, AMENDs, `__finalize__` and 2.8 run one at a time; test exec never overlaps a tree writer or a cargo build. Background agents write only the run dir or `hs-wt`.
- R4 **No commits or pushes before S7** in either repo; no stash/reset/checkout -f/clean/restore.
- R5 **Never poll or re-message a finished agent**. A completion notification is handled once; if the row is already `done`, make no tool call. No TaskOutput/SendMessage on done rows, and no progress checks on running agents.
- R6 **Join on files**: don't advance past a join until the output file exists; while waiting, wait for the notification. No sleep loops, no polling.
- R7 **Context hygiene**: read only return blocks (≤8 lines, ≤2k chars) and `run.json`; pass paths, never contents.
- R8 **Bounded loops**: check caps in `run.json` counters **before** spawning; if a cap is exceeded, mark unresolved and continue.
- R9 **Time budget** `MAX_RUN_HOURS` (default 12): when exceeded, finish the current stage and go to S6/S7.
- R10 **Disk guard** before S4, S4z and each S5 build: if free space < `MIN_FREE_GB_RUNTIME` (20), re-run the 2.0 cleanup; if still low, stop at the stage boundary (resumable).
- R11 **Autonomous**: no questions; every ambiguity is decided and recorded in the stage's `decisions.md`.
- R12 **Process ownership**: kill only PIDs recorded in this run dir whose `/proc/<pid>/exe` is under this repo.

How they apply here: **R2** `__hs__` also runs in background (it writes only `hs-wt`). **R4** `snapshot` writes
objects and `refs/grace/<run_id>/*`, never a branch; one carve-out, the `no_op` restore ("## Final status and
Output"). **R7** `jq` projections of ids/counts (≤20 lines) are allowed for routing. **R8** every AMEND, REPAIR, inner-loop or withdraw spawn bumps its `counters` key before the stamp
(2.3b: `amend_hs` for `__hs__`, else `amend_codegen <unit>` against `amend_codegen_per_unit`);
"unresolved" = bug status `unresolved`, unit unfinished. **R9** before every `row_start` except `DISK:*` and the
R9 targets themselves (S4z, the promotion spawns, the S7 prelude execs, S6 `FULL`, S7), `late` → `ev SKIP <row id> reason=time` and jump:
before S4z → S4z (if a unit edited the tree), S6 `FULL` without remediation, S7 prelude, S7; in S5 → promotion, S6 `FULL`
without remediation, S7 prelude, S7; in S6 → S7 prelude, S7. **R10** `df -Pk . | awk 'NR==2{print int($4/1048576)}'` ≥ threshold → no spawn. **R11** your own calls are `events.log`
lines; stage decisions are `<output basename>.decisions.md`. **R12** carve-out: `kill_warm` may kill the process groups recorded in `warm/*.pid`
(`setsid` bash wrappers whose `/proc/<pid>/exe` is bash and whose cmdline runs this run's `warm/warm.sh`) when the run
ends or a warm build must be cancelled (the warm-build join timeout); `waitx` may kill the process group of a `long` job past `detached_wait_min`
(leader pid from the `<prefix>.pid` `long` wrote under `{RUN_DIR}` or `~/.local/state/grace/<run_id>/`, exe bash, cmdline
naming `<prefix>`; detached-job contract of `grace/workflow/2.0_preflight.md` "Phase 9: Background warm builds"); never an HS pid except via `own_stop` at run end. **Stage-end guard**: `guard` after every return from S0
`DONE` until S7 is spawned; `GIT_STATE_VIOLATION` → stop (resumable).

## Units and run directory

**First check, new run or resume**: no subagent spawn tool (Task), or a Bash tool without `run_in_background` → create and edit
nothing; return the Final block with `STATUS: FAILED`, `REASON: NO_TASK_TOOL`, `RUN_DIR: none` (a nested Connector
Agent cannot spawn stages; the caller runs this file from a top-level session).

**New run** (no `{RUN_ID}`):

```bash
ls Cargo.toml crates/ Makefile >/dev/null || echo NOT_REPO_ROOT
CLC=$(printf %s "{CONNECTOR}" | tr 'A-Z' 'a-z'); RUN_ID=$CLC-$(od -An -N3 -tx1 /dev/urandom | tr -d ' \n')
R=grace/runs/$RUN_ID; mkdir -p "$R/rca/briefs" "$R/test/select" "$R/test/status_updates"
WD="{WORKFLOW_DIR}"; [ -n "$WD" ] && [ "${WD%/}" != grace/workflow ] && cp -a "$WD" "$R/workflow.tmp" && mv "$R/workflow.tmp" "$R/workflow"
```

`UNITS` = `{FLOWS}` (comma list or JSON array) split on `,` and trimmed, each entry resolved **case-insensitively**
on the part before any `/` (the `/PaymentMethod` suffix is kept as written), then deduplicated keeping first-seen
order:

- a flow marker → its exact casing, from the `pub struct <Marker>;` lines of
  `crates/types-traits/domain_types/src/connector_flow.rs`;
- a flow group **or any member of one** → the group name, from the `## FLOW-GROUP MAP (Authoritative)` table of
  `grace/rulesbook/codegen/.gracerules_add_flow`. The group map wins over the marker list, so a member resolves to
  its group (`PreAuthenticate` → `ThreeDS`), never to itself;
- `incomingwebhook`, `webhook`, `webhooks` → `IncomingWebhook`;
- anything else passes through unchanged.

A flow group (`grace/rulesbook/codegen/.gracerules_add_flow` "## FLOW-GROUP MAP (Authoritative)", aliases
case-insensitive) is **one** unit, never split; `IncomingWebhook` (aliases `webhook`, `webhooks`, case-insensitive) is the
non-marker webhook unit (`2.3a_plan.md` Phase 2); unknown names pass through (`2.3a_plan.md` blocks them). `unit_fs` =
unit with `/` → `__`; `{RUN_DIR}` = `$R/`; `{TECHSPEC_PATH}` =
`grace/rulesbook/codegen/references/<connector_lc>/technical_specification.md`. Write
`run.json` (`deadline_at` = `date -u -d "+{MAX_RUN_HOURS} hours" +%FT%TZ`, `caps` from
"## Caps") and `orch.sh` (`R`, `RUN_ID` filled in); every orchestrator bash call starts with `. <R>/orch.sh`. Every
other run-dir path has one writer, the stage owning it (Subagent Reference; `hs-wt/`, `warm/`, `disk/`, `workflow/` →
2.0, which keeps a `workflow/` that init seeded; `env/`, `test/baseline*.json`, `test/capabilities.json` → 2.6a).
Tree-writing stages append `claimed.tsv`.

**`run.json`** (top-level `ended_at` and `status` are `null` until the run ends or stops; 2.0 treats a run without
`ended_at` as active):

```
{schema: 1, run_id, connector, connector_lc, units[], inputs{flows, hs_repo_path, max_run_hours, min_free_gb,
 min_free_gb_runtime, parallel_hs_build}, started_at, deadline_at, ended_at, status (null | SUCCESS | FAILED | SKIPPED), stopped (null | cause),
 workflow_dir, branch, base_sha, ports{grpc, metrics}, hs_mode, review_ref, caps{<key>: n},
 counters{nn, exec_round, rca_rounds, status_update, review_rounds, amend_links, amend_techspec, amend_plan, amend_hs,
          amend_codegen{<unit>}, e2e, env_repairs, missing{<file>},
          crash{<unit_fs>}},
 flags[] (TEST_ENV_FAILED | SECRET_LEAK | TREE_MODIFIED), blocking_open[],
 plan_order[{seq, unit, status}],
 rows[{id, status: pending|running|done|failed|skipped|blocked|invalidated, result, attempt, bg, started_at, ended_at,
       output, mode, brief, units[]}],
 bugs{<bug_id>: {origins[], briefs[], reappeared, escalated}}}
```

Row ids: `S0`, `S1:links:<unit_fs|common>`, `S1:hs_scout`, `S1m:<k>`, `S2:<k>`, `BASELINE`, `S3:<k>`,
`S4:<NN>:<unit_fs>`, `S5:env:<N>:<k>`, `S5:exec:<N>`, `S5:rca:<N>`, `S5:e2e:<N>`,
`S6:review:<FULL|INCREMENTAL>`, `S7`, `DISK:<tag>`, `S4:pending:<unit_fs>` (`<k>` = 1 + earlier rows with that
prefix). A crash (no return block, or `DONE` without its OUTPUT file) re-spawns the same id, `attempt + 1`, only while
`attempt ≤ caps.crash_respawn_per_stage`; over → row `failed`, `SKIP <id> reason=cap:crash_respawn_per_stage`, then
that stage's `FAILED` action; an `S4:*` crash follows Resume step 4a instead (fresh `NN`). `result` = agent STATUS; `status`:
`DONE|READY|NO_CHANGE|PARTIAL` → `done`, `FAILED|ABORT_*` → `failed`, `BLOCKED|SPEC_GAP|PLAN_CONFLICT` → `blocked`, not
spawned → `skipped`, S3's `S4:pending:*` rows → `pending` (`row_start` of that unit's first S4 row deletes it), output
vanished or unit withdrawn → `invalidated` (never joined).

**`events.log`**: `<utc> SPAWN|RETURN|LOOPBACK|SKIP|JOIN <row id | brief id | file> <detail>` — SPAWN
`attempt= mode= brief= units=`; RETURN `status= output=`; LOOPBACK `origin= targets=<agent files> units= round=`;
JOIN `present|timeout`; SKIP `reason=cap:<key>|time|unit:<status>|stop:<cause>|flag:<flag>|missing:<file>`.

**`orch.sh`** starts `R=grace/runs/<run_id>; RUN_ID=<run_id>`. Its bookkeeping helpers are ordinary shell; write
them to this contract (`now` = `date -u +%FT%TZ`):

| Helper | Contract |
|---|---|
| `now` | `date -u +%FT%TZ` (a function: the verbatim block below calls it) |
| `ev <kind> <what> [detail]` | append one `events.log` line: `<utc> <kind> <what> <detail>` |
| `rj <jq args…>` | rewrite `run.json` through `run.json.tmp` + `mv` |
| `late` | true when now ≥ `run.json .deadline_at` (R9) |
| `bump <key> [<sub>]` | `+1` on `counters.<key>` (or `counters.<key>.<sub>`), creating it at 0, and print the new value |
| `row_start <id> <mode> <brief> <units_csv> <bg>` | `touch "$R/.stamp/<id>"` (Resume step 3 dates outputs against this stamp); drop any existing row `<id>` **and** the `S4:pending:<unit_fs>` row of the same unit; append `{id, status: "running", result: null, attempt: <that id's last attempt> + 1, bg, started_at: now, ended_at: null, output: null, mode, brief, units}` — `-` or empty `mode`/`brief` → `null`, `units` = the csv split with empties **and `-`** dropped; then `ev SPAWN <id> attempt= mode= brief= units=` |
| `row_end <id> <row status> <agent STATUS> <output>` | set that row's `status`, `result`, `ended_at`, `output`; then `ev RETURN <id> status= output=` |
| `claimed_ucs` | column 1 of `claimed.tsv` without the `hs:` rows, sorted unique (file absent → nothing) |
| `note_exec <N>` | `+1` on `bugs[<b>].reappeared` for every `<b>` in `test/results/r<N>.json .bugs.reappeared[]` |
| `note_rca <N>` | for each entry of `rca/r<N>.json` and each id in its `bug_ids[]`, append the entry's `origin` to `bugs[<id>].origins` and its `brief_ref` to `bugs[<id>].briefs` (nulls dropped) |
| `build_units <build log> <"" \| hs:>` | JSON array of the units whose `claimed.tsv` paths the log's `--> <path>` lines name (arg 2 prefixes those paths, for `hs:` claims), **ignoring rows whose unit column is `-`**; none → `["__finalize__"]` |

The rest are not bookkeeping — git safety, selection rules and process ownership. Write them as given:

```bash
snap_tree() { local i="$R/.snap.index"; rm -f "$i"; GIT_INDEX_FILE="$i" git read-tree HEAD
  claimed_ucs | GIT_INDEX_FILE="$i" xargs git update-index --add --remove -- 2>/dev/null
  GIT_INDEX_FILE="$i" git write-tree; rm -f "$i"; }
snapshot() {  # label stage unit — HEAD, index and working tree untouched; never pushed
  local n t c ref; n=$(printf %03d $(( $(cat "$R/snapshots.tsv" 2>/dev/null | wc -l) + 1 )))
  t=$(snap_tree); c=$(git commit-tree "$t" -p HEAD -m "grace snapshot $RUN_ID $n-$1"); ref="refs/grace/$RUN_ID/$n-$1"
  git update-ref "$ref" "$c" && printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$n" "$ref" "$2" "$3" "$c" "$t" "$(now)" >> "$R/snapshots.tsv"; }
guard() { [ "$(git branch --show-current)" = "$(jq -r .branch "$R/run.json")" ] \
  && [ "$(git rev-parse HEAD)" = "$(jq -r .base_sha "$R/run.json")" ] \
  && [ "$(git stash list | wc -l)" -eq "$(jq -r .stash_count "$R/preflight.json")" ] \
  || { ev SKIP guard stop:GIT_STATE_VIOLATION; rj '.stopped = "GIT_STATE_VIOLATION"'; echo GIT_STATE_VIOLATION; }; }
prev_origin() {  # bug_ids_csv → {"<bug_id>": "<last origin>"} for first reappearances, once per bug
  jq -c --arg ids "$1" '[.bugs | to_entries[] | select((.key | IN($ids | split(",")[])) and .value.reappeared == 1
    and (.value.escalated | not) and (.value.origins | length) > 0) | {(.key): .value.origins[-1]}] | add // {}' "$R/run.json"
  rj --arg ids "$1" 'reduce ($ids | split(",")[]) as $b (.; if .bugs[$b].reappeared == 1 and (.bugs[$b].origins | length) > 0
    then .bugs[$b].escalated = true else . end)'; }
select_checks() {  # N bug_ids_csv changed_units_csv → test/select/r<N>.json = (R1 ∪ R2) − withdrawn units
  jq -n --arg b "$2" --arg u "$3" --slurpfile f "$R/test/final.json" --slurpfile g "$R/test/bugs.json" --slurpfile p "$R/plan/plan.json" '
    ($b | split(",")) as $B | ($u | split(",")) as $U | $f[0].checks as $K
    | [$g[0].bugs[] | select(.bug_id | IN($B[])) | .checks[]] as $r1
    | [$K | to_entries[] | select(.value.unit | IN($U[])) | .key] as $r2
    | [$p[0].order[] | select(.status == "withdrawn") | .unit] as $W
    | ($r1 + $r2 | unique) - [$K | to_entries[] | select(.value.unit | IN($W[])) | .key]' > "$R/test/select/r$1.json.tmp" \
  && mv "$R/test/select/r$1.json.tmp" "$R/test/select/r$1.json"; }
# R3 (transitive dependents) and R4 (fast replay) are gone. The harness resolves and executes each scenario's
# `depends_on` itself, so a selected scenario always brings its own prerequisites; and a full sweep is one
# command, so there is nothing a "fast replay" subset saves.
kill_warm() {  # [ucs|hs …] (default both) — R12 carve-out: the setsid bash wrapper leads its process group
  local n p; for n in ${@:-ucs hs}; do p=$(cat "$R/warm/$n.pid" 2>/dev/null) || continue
    case "$(readlink "/proc/$p/exe" 2>/dev/null)" in */bash) ;; *) continue;; esac
    { tr '\0' ' ' < "/proc/$p/cmdline"; } 2>/dev/null | grep -qF "$R/warm/warm.sh" && kill -TERM -- "-$p" 2>/dev/null; done; }
```

`snapshots.tsv` = `seq ref stage unit commit tree utc`. Snapshot after every UCS tree-writing return: S1m (`s1m`), each
UCS-unit S4 spawn with non-empty `CHANGED_UNITS` (`<NN>-<unit_fs>`), S4z and every later `__finalize__`
(`<NN>-finalize`), and before S6 when `snap_tree` ≠ the
last row's tree (`review`).

## Spawning and joins

Every spawn has this shape; the stage templates below list only the `Variables:` lines (omit a line with no value):

```
Task(
  subagent_type="general-purpose",
  description="<row id> {CONNECTOR}",
  prompt="Read and follow the workflow defined in {WORKFLOW_DIR}/<file>

Variables:
  <NAME>: <value>")
```

`{WORKFLOW_DIR}` = `{RUN_DIR}workflow` for S0 when init seeded it, else `grace/workflow`; afterwards
`run.json .workflow_dir` (= `preflight.json .workflow.dir`, fallback `grace/workflow`; 2.0 copies `grace/workflow` into
`{RUN_DIR}workflow/` when the run branch's copy differs, e.g. lacks the v2 files, and keeps an existing copy). When it is not `grace/workflow`, add the line
`WORKFLOW_DIR: {WORKFLOW_DIR} (read every grace/workflow/<file> this workflow cites from here)`. Background spawns: use
the spawn tool's background mode where it exists (`run_in_background=true`); in harnesses where subagents already run
asynchronously, do not wait.

**Orchestrator briefs** `rca/briefs/o-<label>.json` (next to 2.6e's `b<N>-<nn>` briefs, so every stage's
`AMEND_BRIEF` and the 2.8 bug ledger see them) carry the AMEND contract fields `{brief_id, bug_ids: [], units,
origin, finding, evidence, required_change, fix: [], do_not_touch: [], upstream_no_change: false}` plus
`amend_targets` and `withdraw: false`. `units` is always a JSON array of unit ids (every reader iterates it), never a
comma-joined string. **Derived briefs** `rca/briefs/<brief_id>.<nc|u|wd>.json` copy a brief with
`upstream_no_change: true` (`nc`); extended `units`, `fix: []`, `withdraw: false` and `required_change` "reconcile to plan rev <rev>
(`plan/plan.rev<rev-1>.json` → `plan/plan.json`): §4 for 2.3b, §8 test spec for 2.3b Phase 2t" (`u`; 2.3b applies a non-empty
`fix[]` only, so units outside the original brief get this one); or `withdraw: true` and `fix: []` (`wd`; 2.3a and
2.3b key their withdraw procedure on `withdraw == true`).

**Join rules**
1. Progress arrives only as notifications: a foreground Task return, a background Task completion, or the exit of a
   background Bash (`run_in_background: true`) waiter or timer, whose loop costs no turns:
   waiter `timeout <MIN>m bash -c 'until [ -e "$0" ]; do sleep 30; done' <file>; echo "JOIN <file> exit=$?"`;
   timer `sleep $((<MIN>*60)); echo "TIMER <name>"`.
2. Act once per completion: the row is already terminal (`done|failed|skipped|blocked|invalidated`) or its join is
   already satisfied → no tool call.
3. A join is satisfied only by its file, a terminal row or the flag in its row; record `JOIN <what> present|timeout`.
4. A capped join's timer starts the first time a stage waits on it unsatisfied (BASELINE: as stated in its row); on
   `TIMER`: row `failed`, `JOIN <what> timeout`, `mkdir -p "$R/cancelled" && : > "$R/cancelled/<row id>"` for each
   cut row; its late completion notification is ignored (rule 2); then the row's timeout action. **Cancel marker**:
   BASELINE, `S5:e2e` and `__hs__` spawns carry `ROW_ID: <row id>`; the agent runs
   `test -e {RUN_DIR}cancelled/{ROW_ID}` before every final `mv` (2.6a also before a server start or stop; 2.3b also
   before each edit batch and gate launch) and, when present, returns `FAILED`, REASON `CANCELLED`, writing nothing more (no task-stop tool needed; use one too if the
   harness has it).

| Join | Blocks | Satisfied by | Cap (`## Caps`) |
|---|---|---|---|
| warm UCS build | BASELINE spawn | `warm/ucs_build.exit` (`2.0_preflight.md` "Phase 9: Background warm builds") | `warm_build_wait_min` → `kill_warm ucs`, spawn BASELINE anyway |
| BASELINE | S3, first S4 spawn | `BASELINE` row terminal | `baseline_join_min` from the later of S2's return and BASELINE's spawn → join rule 4 |
| e2e | S5 step 5 (stop-early) and S6 | every `S5:e2e:*` row terminal; **or** `env/env.json .hs.available` not true → satisfied at once, `ev JOIN e2e skipped`, no timer | `e2e_join_min` → the units with no `e2e/<N>.json` get `e2e_status: FAILED` (`NO_E2E_RUN`); S5 continues |
| `__hs__` | S5 | its `S4:<NN>:__hs__` row terminal; **or** no `S4:*:__hs__` row and (`run.json .hs_mode` ≠ `worktree` or no `__hs__` entry in `plan_order`) → satisfied at once, `ev JOIN __hs__ present`, no timer | `hs_join_min` → `plan.json .hs_changes` stay unresolved; S5 continues |

## Pipeline

```
init ─ S0 ─┬─ S1 wave (links common | links × unit | hs_scout) ─ S1m ─ S2 ─ S3 ─ S4 U0…Un ─ S4z ─ S5 ─ S6 ─ S7
           │                                                              └ bg __hs__ (after U0)
           ├─ waiter warm/ucs_build.exit ─ bg BASELINE ──────────── joined by S3 and S4
           └─ S5 spawns 2.5_e2e.md per unit group ───────────────── joined by S5 step 5 and S6

There is no test lane. Test *data* is written by S4 itself (`2.3b_codegen_unit.md` Phase 2t) into the
connector's committed `connector_specs/` JSON, from plan §8. Nothing in this run designs a case, renders a
request or stages a harness root — the repo's harness owns all three, and the data it runs is in the diff.
```

### S0 — `2.0_preflight.md` (foreground)

```
  CONNECTOR: {CONNECTOR}
  UNITS: <units as JSON array>
  HS_REPO_PATH: {HS_REPO_PATH}
  RUN_DIR: {RUN_DIR}
  MODE: FULL
  MIN_FREE_GB: {MIN_FREE_GB}
  STALE_DAYS: {STALE_DAYS}
  GRACE_SCAN_ROOTS: {GRACE_SCAN_ROOTS}
  GRACE_EXTRA_REPOS: {GRACE_EXTRA_REPOS}
  PARALLEL_HS_BUILD: {PARALLEL_HS_BUILD}
```

`ABORT_CREDS` → stop → SKIPPED. Other `ABORT_*` or `FAILED` → stop → FAILED. No PR either way. `DONE` → `rj` these
five from `preflight.json` into `run.json`: `branch`, `base_sha`, `ports`, `workflow_dir` (= its
`.workflow.dir`, fallback `grace/workflow`) and `hs_mode` (= its `.hs.mode`); start the
warm-build waiter (cap `warm_build_wait_min`); → S1.

### S1 wave — `2.1_links.md` × (common + each unit), `2.1a_hs_scout.md` (foreground)

Stamp every row in one Bash call, then send the Task calls ≤6 per message.

```
links common:  CONNECTOR_NAME: {CONNECTOR}
               OUTPUT_PATH: {RUN_DIR}links/common.json
               FOCUS: authentication, environments and base URLs, error body and codes, idempotency, webhooks and signature verification, sandbox test data
links <unit>:  CONNECTOR_NAME: {CONNECTOR}
               PAYMENT_METHOD: <part after "/">
               OUTPUT_PATH: {RUN_DIR}links/<unit_fs>.json
               FOCUS: <unit markers; a group lists its markers>: endpoints, request and response schema, statuses
               AMEND_BRIEF: <brief>                                     (loop-back only)
hs_scout:      CONNECTOR: {CONNECTOR}
               UNITS: <units csv>
               HS_REPO_PATH: {HS_REPO_PATH}
               RUN_DIR: {RUN_DIR}
               OUTPUT_PATH: {RUN_DIR}hs/scout.json
```

Not a gate: `FAILED` is recorded. Every wave row terminal → S1m.

### S1m — merge (orchestrator, tree writer)

```bash
. grace/runs/<run_id>/orch.sh; row_start "S1m:<k>" - - - false; CLC=$(jq -r .connector_lc "$R/run.json")
find "$R/links" -maxdepth 1 -name '*.json' ! -name merged.json -exec cat {} + \
  | jq -s '[.[] | .unit as $u | .links[]? | select(.verdict == "verified") | {url, category, unit: $u}]
           | group_by(.url) | map({url: .[0].url, category: .[0].category, units: (map(.unit) | unique)})' \
  > "$R/links/merged.json.tmp" && mv "$R/links/merged.json.tmp" "$R/links/merged.json"
L=data/integration-source-links.json; [ -f "$L" ] || echo '{}' > "$L"
jq --arg a "{CONNECTOR}" --arg b "$CLC" --slurpfile m "$R/links/merged.json" '
  (if has($a) then $a elif has($b) then $b else $a end) as $k
  | .[$k] = reduce ((.[$k] // []) + [$m[0][].url])[] as $u ([]; if index([$u]) then . else . + [$u] end)' "$L" \
  > "$R/links.json.tmp" && mv "$R/links.json.tmp" "$L"
git diff --quiet -- "$L" || grep -qx "$L" <(claimed_ucs) || printf '%s\tS1m\t-\n' "$L" >> "$R/claimed.tsv"
snapshot s1m S1m -; guard; row_end "S1m:<k>" done DONE links/merged.json
jq -r --arg a "{CONNECTOR}" --arg b "$CLC" '(.[$a] // .[$b] // []) | length' "$L"
```

The union never drops a URL (`2.1_links.md` "PHASE 3: WRITE TO SHARED LINKS FILE"). Count 0 → stop → FAILED ("no
documentation links"), no PR.

### S2 — `2.2_techspec.md` (foreground)

`{TECHSPEC_PATH}` absent → `REGENERATE`. Present → `AMEND` with `rca/briefs/o-s2.json` (`origin: LINKS`, all units,
`finding` "existing spec predates this run", evidence [`links/merged.json`], `required_change` "add `### <Flow>`
subsections for the units the spec does not cover, from links/merged.json").

```
  CONNECTOR: {CONNECTOR}
  FLOWS: <units csv>
  MODE: REGENERATE | AMEND
  HS_REFERENCE: {RUN_DIR}hs/scout.json        (only if it exists)
  AMEND_BRIEF: <brief>                         (AMEND only)
  RESULT_PATH: {RUN_DIR}spec/result.json
```

`DONE`/`NO_CHANGE` → S3 (after the BASELINE join). `PARTIAL` → once, within caps: links per unit of
`spec/result.json .flows_missing` (FOCUS `<unit>: missing endpoints, request and response schema, statuses`) → S1m →
S2 `AMEND` with `rca/briefs/o-s2-missing.json`; units still missing are blocked by 2.3a → S3. `FAILED` → continue with an
existing `{TECHSPEC_PATH}` (`SKIP`), else stop → FAILED, no PR.

### BASELINE — `2.6a_test_env.md` (background)

On the warm-build `JOIN` (present or timeout); if S2 has already returned, start the BASELINE join timer now.

```
  CONNECTOR: <connector_lc>
  UNITS: <units csv>
  RUN_DIR: {RUN_DIR}
  MODE: BASELINE
  ROUND: 0
  PROBE_ONLY: 0
  HS_REPO_PATH: {HS_REPO_PATH}
  ROW_ID: BASELINE
```

Any return satisfies the join; only a `done` BASELINE is ingested in round 1.

**Deferred capability probe.** BASELINE races S2, so 5e often runs before `{TECHSPEC_PATH}` exists. After the join
**and** S2's return, before S3: any `test/capabilities.json` entry whose `.probe` starts with `deferred:`
→ one foreground 2.6a spawn (`S5:env:0:<k>`), the same template with `PROBE_ONLY: 1`, so
the entries become `provisioned`/`not_provisioned` before 2.6d reads them (without it no check can ever be
`sandbox_blocked`, and a provisioning refusal classifies as `PRODUCT_BUG`). `FAILED` → `SKIP`, entries stay `error`.

### S3 — `2.3a_plan.md` (foreground, after S2 and the BASELINE join)

```
  CONNECTOR: <connector_lc>
  UNITS: <units csv>
  RUN_DIR: {RUN_DIR}
  TECHSPEC_PATH: {TECHSPEC_PATH}
  MODE: NEW | AMEND
  AMEND_BRIEF: <brief>                         (AMEND only)
```

Record `plan_order` = `plan/plan.json .order[]` as `{seq, unit, status}`; pre-create a row
`{id: "S4:pending:<unit_fs>", status: "pending", attempt: 0}` per `planned` unit with no `S4:*:<unit_fs>` row yet. `DONE`/`PARTIAL` → every unit `no_op` → stop → SKIPPED ("already implemented"); else test lane → S4.
`SPEC_GAP` → once, within caps: `rca/briefs/o-s3-gap.json` (the `spec_gap` units, `finding` = REASON, `required_change`
"answer <topic> in the spec") → links per unit (FOCUS = topic) → S1m → S2 `AMEND` → S3 `AMEND` (same brief) → test
lane → S4; units still `spec_gap` are not implemented. `BLOCKED` → S7 gate. `FAILED` → stop → FAILED, no PR.

### S4 — `2.3b_codegen_unit.md`, one unit per message in `plan_order` (foreground)

Before the first spawn: BASELINE join, R10 (`DISK_TAG: pre-S4`). Spawn only `plan_order` entries with
`status == "planned"` (plus the `__hs__` rule below); every other entry (`no_op`, `blocked`, `spec_gap`, `withdrawn`)
is left out of the run with `ev SKIP S4:<NN>:<unit_fs> reason=unit:<status>` and no spawn. Every 2.3b spawn (NEW or
AMEND, from S4, S4z, S5, loop-back or promotion) uses this template with a fresh `NN` = `printf %02d $(( $(bump nn) - 1 ))`.

```
  CONNECTOR: <connector_lc>
  UNIT: <unit | __hs__ | __finalize__>
  RUN_DIR: {RUN_DIR}
  MODE: NEW | AMEND
  AMEND_BRIEF: <brief>                         (AMEND only)
  NN: <NN>
  TECHSPEC_PATH: {TECHSPEC_PATH}
  SMOKE: 1                                     (plan.json .foundation_smoke != null, UNIT = its .unit: the NEW spawn and every o-smoke, o-gate or o-resume AMEND)
  HS_REPO_PATH: {HS_REPO_PATH}                 (every __hs__ spawn, NEW or AMEND)
  ROW_ID: <row id>                             (every __hs__ spawn)
```

R10 spawn `DISK:<tag>` (`2.0_preflight.md`): `CONNECTOR`, `UNITS` (JSON array), `HS_REPO_PATH`, `RUN_DIR`,
`MODE: DISK_ONLY`, `DISK_TAG: <pre-S4 | pre-S4z-<k> | pre-r<N>>`, `MIN_FREE_GB_RUNTIME`, `STALE_DAYS: {STALE_DAYS}`,
`GRACE_SCAN_ROOTS: {GRACE_SCAN_ROOTS}`, `GRACE_EXTRA_REPOS: {GRACE_EXTRA_REPOS}`. `ABORT_DISK` → stop → FAILED (resumable).

`__hs__` (`planned` and `hs_mode = worktree`) spawns in background after U0 returns. Otherwise no spawn: `rj` a row
`{id: "S4:00:__hs__", status: "skipped", result: null, attempt: 0}`, delete `S4:pending:__hs__`,
`ev SKIP S4:00:__hs__ reason=unit:hs_mode=<mode>` — a definite row, so the `__hs__` join and 2.8 Phase 8's
`^S4:[0-9]+:__hs__$` query both see it. After each UCS return: `guard`, snapshot.

| STATUS | Action |
|---|---|
| `DONE`, `NO_CHANGE` | next unit |
| `FAILED`, REASON `foundation smoke: …` | AMEND with `rca/briefs/o-smoke-<NN>.json` (`required_change` "make the foundation smoke pass: <REASON>", evidence `code/<NN>-<unit_fs>.smoke.*`) and `SMOKE: 1`, so `DONE` means the smoke re-ran green. That AMEND `NO_CHANGE`, or at cap `amend_codegen_per_unit` → `SKIP cap:amend_codegen_per_unit`, next unit; round 1 `FULL_RUN` re-runs the unit's suites, which files the bug for RCA |
| `FAILED` (other) | AMEND with `rca/briefs/o-gate-<NN>.json` (`required_change` "finish plan §4[<unit>]: <REASON>"); at cap → withdraw |
| `PLAN_CONFLICT` | S3 `AMEND` with `rca/briefs/o-pc-<NN>.json` (`origin: PLANNER`, `required_change` "resolve: <REASON>") → S3 follow-up; conflict again at cap → withdraw |
| `SPEC_GAP` | `rca/briefs/o-sg-<NN>.json` → links for the unit (FOCUS = REASON topic) → S1m → S2 `AMEND` → S3 `AMEND` → S3 follow-up; at cap → withdraw |
| `BLOCKED` | recorded: `withdraw: shared code` → unit unresolved; `no hs-wt` → `__hs__` skipped |

Withdraw = `rca/briefs/o-wd-<unit_fs>.json` (`withdraw: true`, `fix: []`, `required_change` "<unit>: <why>") → S3 `AMEND`
→ S3 follow-up → S4z. 2.3a also marks every §9 `hs_changes[]` item whose `flows_unblocked` are all withdrawn
`withdrawn: true` (`2.3a_plan.md` "Phase 12: AMEND" step 3), so no HS PR is raised for a flow UCS no longer
implements; an `hs-wt` commit already made for such an item is simply never pushed. **S3 follow-up** of an S3 `AMEND` on brief `B` (here and in Loop-back rules 2 and 4): 2.3b `AMEND`
of `B`'s units (its 2.3b `amend_targets[].scope.units`, else `units`) with `B` (S3 `NO_CHANGE` → `B.nc`),
then of every other unit of that S3 `AMEND`'s `CHANGED_UNITS` with an `S4:*` row started before it, with `B.u`. §8 changes
from these S3 `AMEND`s (`test_hooks_changed`) reach 2.3b Phase 2t in S4z's hooks round.

### S4z — `__finalize__` (foreground)

After the last UCS unit: R10 (`pre-S4z-<k>`), S4 template with `UNIT: __finalize__`, `MODE: NEW`; `guard`, snapshot.
`DONE` or `FAILED` → continue (S7 gates again), then the hooks round.

**Hooks round** — `H=$(jq -c '[.amendments[]?.test_hooks_changed[]?] | unique' "$R/plan/plan.json")`
`!= '[]'` and no `rca/briefs/o-hooks.json` yet → write it (`origin: PLANNER`, `units` = `H`, evidence
[`plan/plan.json`], `required_change` "reconcile `connector_specs/` to plan §8 for these units (plan rev <rev>)")
→ 2.3b `AMEND` per unit in `H` with it (cap `amend_codegen_per_unit`) → `__finalize__` again, whose schema
validators and `test_author_gate.py` run because `connector_specs` changed.

There is no Spec-gap round and no code audit. A spec gap is a question the *plan's* oracle could not answer,
and plan §8's validators now refuse a hook with no `oracle_ref` — so the gap is caught at S3, one stage before
any code exists, instead of being discovered by a case designer reading the code. The code audit was the test
designer re-reading generated code to adjust its cases, which is exactly the self-grading loop this workflow
removed; nothing replaces it, because nothing should.

### S5 — test loop

Every `2.6d_test_exec.md` spawn, in this order: `N=$(bump exec_round)`; for `MODE: ROUND` write its selection to
`test/select/r$N.json` (`select_checks $N …` for a retest; otherwise a JSON array of check ids via `.tmp` + `mv`,
"same selection" = a copy of the previous file); stamp `S5:exec:$N`; spawn with `ROUND: $N`. `ONLY_CHECKS: none`
is a bookkeeping call that writes no `test/results/r<N>.json`.

**ENV repairs**: every `REPAIR` spawn first `bump env_repairs`; over `caps.env_repairs_per_round` →
`SKIP cap:env_repairs_per_round`, no spawn: in step 3 its `env` issues go on to RCA, elsewhere flag `TEST_ENV_FAILED`
→ S6. `bump rca_rounds` also runs `rj '.counters.env_repairs = 0'`.

**1. Env** — before the first exec and after every loop-back round that changed code, `hs-wt` or the env: R10
(`pre-r<N>`), join `__hs__` (a `TEST_ENV_FAILED` flag → skip S5 → S6), then `2.6a_test_env.md`
(`S5:env:<N>:<k>`):

```
  CONNECTOR: <connector_lc>
  UNITS: <units csv>
  RUN_DIR: {RUN_DIR}
  MODE: POST_CODEGEN | REPAIR
  ROUND: <N>
  HS_REPO_PATH: {HS_REPO_PATH}
  RESULTS: {RUN_DIR}test/results/r<N>.json  |  AMEND_BRIEF: <brief>       (REPAIR only, one of them)
```

`READY` → 2. `PARTIAL` → 2, except a **retryable** HS failure — `env/env.json .hs.unavailable_reason` in the
retryable set of `2.6a_test_env.md` "## Inputs" mode table, `POST_CODEGEN` row, or `.hs.ready` false with
`not_ready_class: "HS_CONFIG"` — which gets one `REPAIR` with `rca/briefs/o-env-r<N>.json` first (2.6a Phase 6 re-runs
the bootstrap; 2.6d files no `env_issues[]` for an unavailable HS, so nothing else would ever retry it), then 2
whatever it returns — at the `env_repairs_per_round` cap, `SKIP` then 2, no `TEST_ENV_FAILED` flag (the HS surface
stays failed; that is the PARTIAL's own verdict). `FAILED` `TEST_ENV_FAILED` → `REPAIR` with `rca/briefs/o-env-r<N>.json` (ENV repairs);
at cap, `SECRET_LEAK` or `TREE_MODIFIED` → flag `TEST_ENV_FAILED` (+ that token) → S6.
`POST_CODEGEN` `FAILED` `BUILD_FAILED:<ucs|hs>`, no `rca/briefs/o-build-<N>.json` yet → write it (`origin: CODEGEN`,
`finding` = REASON, evidence [`<log>` = `$R/<REASON's run-dir path>`; `hs` → `$R/env/hs-build-r<N>.log`], `units` =
`build_units "<log>" <""|hs:>`, `required_change` "fix the compile errors in <log>") → 2.3b `AMEND` per unit in
`plan_order` (`__hs__` last; at cap `amend_codegen_per_unit` → `SKIP`) → `__finalize__` (once; it is the whole list when
`build_units` found none) → code audit → `POST_CODEGEN` once more; `BUILD_FAILED` again → flag `TEST_ENV_FAILED` → S6.

**2. Exec** — `2.6d_test_exec.md` (`S5:exec:<N>`):

```
  RUN_DIR: {RUN_DIR}
  ROUND: <N>
  MODE: FULL_RUN | ROUND
  ONLY_CHECKS: {RUN_DIR}test/select/r<N>.json  (ROUND only; omitted for FULL_RUN — `none` is the bookkeeping sentinel of the S7 prelude, never a FULL_RUN value)
  INGEST: <baseline | review>
  STATUS_UPDATES: {RUN_DIR}test/status_updates/u<k>.json
```

Round 1 is `FULL_RUN` with `INGEST` = `baseline` (BASELINE `done` and `test/baseline_bugs.json` exists). Every `DONE`/`PARTIAL` that wrote `test/results/r<N>.json` → `note_exec <N>`. `DONE` → 3. `PARTIAL` → `REPAIR` (`RESULTS`) → `ROUND` over its
`NOT_RUN` cases. `BLOCKED` (`ENV`) → `REPAIR` (`rca/briefs/o-env-r<N>.json`) → same selection. `FAILED` `ROUND_EXISTS` →
same spawn, next `N`. `FAILED` `SECRET_LEAK` → flag `TEST_ENV_FAILED` + `SECRET_LEAK` → S6 (as the env path at step 1:
`TEST_ENV_FAILED` is what makes `exec_ready` false, so the S7 prelude does not re-enter the same security gate, and
what 2.8 keys `INCOMPLETE` on). `FAILED` `MISSING <file>` → once per file
(`bump missing <file>` = 1) its owner, then the same spawn with the next `N`: `env/env.json` → Env `POST_CODEGEN`.
`plan/plan.json`, `MISSING creds` and `MISSING binary` have no owning stage that S5 may re-enter → flag
`TEST_ENV_FAILED` → S6.

**3. Triage** — `jq '{env: [.env_issues[] | select(.class == "ENV")], hs_config: [.env_issues[] | select(.class == "HS_CONFIG")], scenario_data: [.checks[] | select(.class == "SCENARIO_DATA") | .check_id]}' "$R/test/results/r<N>.json"`.

2.6d has already classified every failure into exactly one of `ENV | HS_CONFIG | SCENARIO_DATA | SANDBOX_BLOCKED |
PRODUCT_BUG` (`2.6d_test_exec.md` "## Phase 5: Triage each failure"). Route the first two and the third; the last
two need no orchestration (one is not a bug, the other goes to RCA in step 4).

- `env` → `2.6a_test_env.md` `REPAIR` (`RESULTS`), under `env_repairs_per_round`.
- `hs_config` → `2.6a_test_env.md` `REPAIR` likewise; a second one in the same round is not a repair, it is a
  finding — let it reach RCA.
- `scenario_data` → **2.3b `AMEND`** per affected unit, brief `rca/briefs/o-scen-r<N>-<unit_fs>.json`
  (`origin: SCENARIO_DATA`, `units: [<unit>]`, evidence the failing checks' transcripts, `required_change`
  "correct the scenario data for <check ids> against plan §8's oracle; do not weaken an assertion"), under
  `amend_codegen_per_unit` → `__finalize__` (the gates re-run over the edited `connector_specs/`).

Then one `ROUND` over the routed check ids plus the ids of every unit an AMEND changed. Repeat while routable
failures under cap remain; when only at-cap ones remain, that one `ROUND`, then 4.

**There is no inner loop any more.** The old one existed because two agents could rewrite a case until it passed,
and both are gone: the scenario is committed data, codegen owns it, and every edit goes back through
`test_author_gate.py`, which forbids weakening an assertion or waiving a scenario that passed in an earlier round.
That is why `SCENARIO_DATA` shares the ordinary codegen amend cap instead of a private per-case one — the run gets
the same small number of attempts to fix the data as it gets to fix the code, and no separate budget for
convincing the oracle.

**4. RCA** — `2.6e_rca.md` (`S5:rca:<N>`, `N` = latest exec round with `test/results/r<N>.json` and no `rca/r<N>.json`).
`BUG_IDS` = in-scope bugs whose `flows[]` are not all markers of withdrawn units, with status `open`
(or `rca` after `needs_probe`), `duplicate_of` null, `attempts` <
`fix_attempts_per_bug`, `run.json .bugs[].reappeared` ≤ 1. Over the fix cap → `unresolved`;
reappeared twice → `unresolved` + withdraw brief for its units. No ids left, or `rca_rounds` at cap → 6.

```
  RUN_DIR: {RUN_DIR}
  ROUND: <N>
  BUG_IDS: <csv>
  PREVIOUS_ORIGIN: <prev_origin <BUG_IDS>; omit when {}>
```

`DONE`/`PARTIAL` → `bump rca_rounds`, `note_rca <N>`. `PARTIAL` with `NEXT` `needs a sandbox example for <bug_id>`
→ those bugs get no brief this round; they re-enter the next RCA round on the same evidence, and at
`fix_attempts_per_bug` they become `unresolved`. There is no PROBE stage to spawn: RCA runs its own probes now,
as a targeted `test_ucs --suite --scenario` re-run or a direct sandbox call (`2.6e_rca.md` Phase 0b), neither of
which needs an agent or a request file.
`BLOCKED` (`ENV`) → `REPAIR` → same RCA spawn. `FAILED` → one re-spawn, still → 6. Briefs → **Loop-back protocol** →
retest `ROUND` (`N=$(bump exec_round); select_checks $N <routed bug ids> <changed units>`, spawn with `ROUND: $N`) → 3.

**5. E2E, then stop early** — before the first stop-early evaluation, spawn the **E2E stage** below and join it
(`e2e_join_min`); its records are what `e2e_status` is derived from, and a unit with no record fails closed.

Then, after each retest (withdrawn units' bugs excluded, as in 2.8's `pr/blocking_bugs.json`):

```bash
jq --slurpfile p "$R/plan/plan.json" '([$p[0].order[] | select(.status == "withdrawn") | .unit] as $w
   | [$p[0].units[] | select(.unit | IN($w[])) | .markers[]]) as $W
 | [.bugs[] | select(.blocking and .in_scope and (.status | IN("open","rca","fixing","retest"))
     and ((.flows // []) as $f | $f == [] or (($f - $W) | length > 0)))] | length' "$R/test/bugs.json"
```

→ append to `blocking_open`; not lower than the previous value → `SKIP stop_early` → 6.

**6. Converged** → Env if needed → one `FULL_RUN` carrying pending `STATUS_UPDATES`. New blocking bugs, `rca_rounds`
under cap and no stop-early → 4, then retests only (no second full run). Otherwise → 7.

**7. Converged** → S6.

### E2E stage — `2.5_e2e.md` (`S5:e2e:<N>`, background, one spawn per flow group)

`env/env.json .hs.available` not true → no spawn at all: `rj` a row `{id: "S5:e2e:00", status: "skipped"}`,
`ev SKIP S5:e2e reason=hs:<unavailable_reason>`. 2.6d then derives `E2E_SKIPPED` only for `NO_CHECKOUT` and
`FAILED` otherwise, which is the fail-closed half of the rule and must not be softened here.

Otherwise one spawn per flow group of the non-withdrawn units (a group = the units 2.5's spec table maps to the
same Cypress specs, so `Refund` and `RSync` share one), `<N>` = `printf %02d $(bump e2e)`:

```
  RUN_DIR: {RUN_DIR}
  E2E_ID: <N>
  CONNECTOR: <connector_lc>
  FLOW: <flow marker or group>
  UNITS: <units csv>
  PAYMENT_METHOD: <pm | "">
  HS_REPO_PATH: {HS_REPO_PATH}
  BRANCH: <run.json .branch>
  ROW_ID: <row id>
```

`DONE`/`PARTIAL` → its `e2e/<N>.json` is read by the next 2.6d spawn. `FAILED`/`BLOCKED` → one re-spawn; still →
the row stays terminal and its units have no record, which 2.6d turns into `e2e_status: FAILED` with reason
`NO_E2E_RUN`. **Do not convert that into a skip.** `HS_PR` from any record is carried to 2.8.

A 2.5 spawn may edit the HS worktree (its Phase 5, including the Cypress harness defects). Those edits are
`__hs__`'s territory in every other stage, so a 2.5 spawn runs only when no `S4:*:__hs__` row is `running`, and
2.8 commits them on the same HS branch.

### S6 — `2.7_review.md` (foreground)

```
  RUN_DIR: {RUN_DIR}
  MODE: FULL | INCREMENTAL
  BASE_SHA: <run.json .base_sha>
  SNAPSHOT_REF: <last ref in snapshots.tsv>
  PREVIOUS_SNAPSHOT_REF: <run.json .review_ref>                (INCREMENTAL only)
```

Take the `review` snapshot first; stamping the `FULL` spawn also sets `review_ref` = its `SNAPSHOT_REF`. `FULL` `DONE`
with S0/S1 findings (`review/findings.json` entries whose `.sev` is `S0` or `S1`), time left,
`exec_ready` and `bump review_rounds` ≤ `caps.review_remediation_rounds` → exec `ROUND`, `INGEST: review`, `ONLY_CHECKS` = the checks of the findings'
units (all units when none) → RCA (step 4) → loop-back → retest → `INCREMENTAL`. No S0/S1 → S7. `FAILED` → one
re-spawn; still → S7.

**`exec_ready`** (guards every 2.6d spawn from S6 on): no `TEST_ENV_FAILED` flag is set and `plan/plan.json` and
`env/env.json` both exist — without them 2.6d returns `FAILED  MISSING <file|creds|binary>`
(`2.6d_test_exec.md` "Inputs"), and the `FAILED` routing of the stage that just failed would re-spawn it. Not
`exec_ready` → no remediation round and no prelude exec: `ev SKIP S5:exec:<N> reason=flag:TEST_ENV_FAILED` (or
`reason=missing:<file>`) → S7. Skipping is fail-closed: the bugs keep their non-terminal status, which 2.8's
`pr/blocking_bugs.json` query already treats as blocking, and unfixed S0/S1 review findings block there too
(`2.8_pr_run.md` "Phase 1: Status → `pr/status.json`").

**S7 prelude** (`exec_ready` only): bookkeeping execs (`ONLY_CHECKS: none`, no `r<N>.json`): after every S6 return that
wrote `review/findings.json` — `FULL` without remediation included — first one with `INGEST: review` only; then one
whose `STATUS_UPDATES`, built from the resulting `test/bugs.json`, move every non-terminal, non-`fixed` bug to
`unresolved`. `test/bugs.json` and `test/final.json` are then final.

### S7 — `2.8_pr_run.md` (foreground)

Gate: `claimed_ucs` lists a path other than `data/integration-source-links.json`; else stop → FAILED ("nothing
implemented"), no PR.

```
  RUN_DIR: {RUN_DIR}
  HS_REPO_PATH: {HS_REPO_PATH}
```

No `guard` from here (2.8 commits and guards itself). `DONE`/`PARTIAL` → Final. `FAILED` → one re-spawn (2.8 resumes
from `pr/commits.tsv`); still, or `ABORT_*` → Final with that reason.

## Loop-back protocol

Input: the briefs of `rca/r<N>.json` (`brief_ref` non-null) plus this round's orchestrator withdraw briefs.

1. **Group by earliest origin, run upstream first.** A brief's chain = its `amend_targets[]`, plus S1m after links,
   `__finalize__` after codegen, and — because every brief adds a regression scenario (`retest.add_checks`) — a
   2.3b AMEND for the units those scenarios belong to. Spawn by rank across all briefs (briefs ordered by earliest
   origin), one spawn at a time: 1 `2.1_links.md` → 2 S1m → 3 `2.2_techspec.md` → 4 `2.3a_plan.md` →
   5 `2.3b_codegen_unit.md` (`plan_order`, `__hs__` last, foreground; code *and* its `connector_specs/` data) →
   6 `__finalize__` → 7 `2.6a_test_env.md` `REPAIR` → 8 Env `POST_CODEGEN` (**one rebuild per round**). Log `LOOPBACK <brief_id> origin= targets= units= round=` before a brief's first spawn.
2. **Scoped to affected units only.** Links: one spawn per `scope.units` entry (`links/common.json` when none), FOCUS =
   `scope.sections`, `AMEND_BRIEF`. 2.2 and 2.3a: one spawn per brief. 2.3b: after a 2.3a AMEND its S3 follow-up (S4),
   else `scope.units` with the brief, ∪ the 2.3a AMEND's `test_hooks_changed` (`u` brief) and the units of the
   brief's `retest.add_checks[]`.
3. **NO_CHANGE propagation**: a target returning `NO_CHANGE` hands the `nc` brief (`upstream_no_change: true`) to the
   brief's later targets; live evidence decides.
4. **Caps before each spawn** (R8): at cap, or an `amend_targets[]` stage returning `FAILED` → **drop**:
   `SKIP cap:<key>`, the brief's remaining targets dropped, its bugs → `unresolved`; the run continues. Recorded,
   chain continues: `__finalize__` `FAILED` (S7 gates again, as in S4z); the regression-scenario 2.3b AMEND (not in
   `amend_targets[]`) failing — the bugs' own checks still retest. Escalations (`<N>` = this RCA round) run at once, within caps, once per brief; the same escalation again,
   or an escalation spawn `FAILED`/`BLOCKED`/`SPEC_GAP`/`PLAN_CONFLICT` → drop:
   - 2.3a or 2.3b `SPEC_GAP` → `rca/briefs/o-lb-sg-<N>-<brief_id>.json` (`origin: LINKS`, `units` = the escalating units,
     `required_change` "answer <REASON topic> in the spec") → links per unit (FOCUS = REASON topic, `amend_links`) → S1m
     → 2.2 `AMEND` with it (`amend_techspec`) → 2.3a `AMEND` with the brief once more → the chain resumes (2.3b
     escalated: its S3 follow-up).
   - 2.3b `PLAN_CONFLICT` → `rca/briefs/o-lb-pc-<N>-<brief_id>.json` (the brief's `bug_ids`, `units`, `fix`,
     `do_not_touch`; `origin: PLANNER`, `required_change` "resolve: <REASON>") → 2.3a `AMEND` with it → its S3
     follow-up (the unit again).
   - 2.3a `BLOCKED` → drop. 2.3b `BLOCKED` → drop, plus a withdraw brief (rule 5) for a code unit unless REASON is
     `withdraw: shared code`.
5. **Withdraw**: a brief with `withdraw: true` → `wd` brief to 2.3a and 2.3b; 2.3b `BLOCKED` (shared code) → bugs
   `unresolved`, nothing withdrawn. Withdrawn units leave every retest (`select_checks`) **and every `FULL_RUN`**
   (`2.6d_test_exec.md` "Phase 2: Build the expected-check set" item 4), and their bugs count in neither step 4's
   `BUG_IDS` nor step 5's blocking count — otherwise step 6's convergence run re-executes a flow that is now
   `not_implemented`, files fresh bugs and routes RCA into re-implementing it.
6. **Reappearing fingerprint** (`r<N>.json .bugs.reappeared`): first time → RCA with `PREVIOUS_ORIGIN`, which moves the
   origin exactly one stage upstream (`2.6e_rca.md` "Phase 4: Escalation (`{PREVIOUS_ORIGIN}`)"); second time, or a
   hunk reversing an earlier fix (RCA compares snapshots) → `unresolved` + withdraw brief if the unit's code is not shared.
7. **Status updates** before the retest spawn, `test/status_updates/u<bump status_update>.json` (shape:
   `2.6d_test_exec.md` "Phase 1: Bookkeeping (INGEST, STATUS_UPDATES)"): per bug of `rca/r<N>.json` `open→rca`,
   `rca→<proposed_status>`, then `fixing→retest` (chain completed) or `fixing→unresolved` (dropped); plus the
   `unresolved` moves of 4–6. `update_id` = `u<k>-<bug_id>-<to>`, `by: orchestrator`, `ref` = brief or `rca/r<N>.json`.
8. **Re-test** (`select_checks`): **R1** the bugs' checks; **R2** all checks of changed units (`CHANGED_UNITS` of
   every AMEND this round). That is the whole rule.

## Caps

Single source of truth; stage files cite this section. Copied into `run.json .caps` at init.

| Cap | Default | `run.json .caps` keys |
|---|---|---|
| RCA rounds / fix attempts per bug | 3 / 2 | `rca_rounds` / `fix_attempts_per_bug` |
| AMEND: links / techspec / plan / codegen per unit / HS | 2 / 2 / 3 / 4 / 2 | `amend_links` / `amend_techspec` / `amend_plan` / `amend_codegen_per_unit` / `amend_hs` |
| Gate iterations per codegen spawn / finalize | 5 / 3 | `gate_iterations_codegen` / `gate_iterations_finalize` |
| Plan validator fix iterations (2.3a Phase 11, per spawn) | 3 | `validator_fix_iterations` |
| ENV repairs per RCA round | 2 | `env_repairs_per_round` |
| Review remediation rounds / crash re-spawn per stage | 1 / 1 | `review_remediation_rounds` / `crash_respawn_per_stage` |
| Stop early | a round where the blocking open count doesn't fall | `stop_early` |
| Warm UCS build wait / BASELINE join wait (minutes) | 120 / 180 | `warm_build_wait_min` / `baseline_join_min` |
| E2E join wait / `__hs__` join wait (minutes) | 60 / 120 | `e2e_join_min` / `hs_join_min` |
| CI auto-fix wait (2.8) | 30 min | `ci_autofix_wait_min` |
| Detached job wait per launch (minutes) | 120 | `detached_wait_min` |

`baseline_join_min` must exceed the `2.6a_test_env.md` BASELINE harness `timeout 3600` (60) plus a 90-minute boot margin.
At the cap: non-blocking → PR "Known issues"; blocking unresolved or `TEST_ENV_FAILED` → PR `INCOMPLETE`.

## Resume

`{RUN_ID}` given → `R=grace/runs/{RUN_ID}`; no `run.json` → stop (a new run takes no `RUN_ID`).

1. `rj '.ended_at = null | .status = null | .stopped = null'` (the run is active again for 2.0). `workflow_dir` ≠
   `grace/workflow` → continue from `<workflow_dir>/2_connector.md`.
2. S0 not `done` → re-spawn S0. S7 spawned → re-spawn S7 (2.8 resumes; no `guard`). Otherwise `guard` must pass, else
   stop: the operator restores branch/HEAD/stash (never `checkout -f`).
3. Output present = `[ <output> -nt "$R/.stamp/<id>" ]` (an older file is an earlier spawn's). Rows other than `S4:*`:
   `done` with output, or with result `NO_CHANGE` (nothing was rewritten) → skip; `done` otherwise →
   `invalidated`, re-spawn; `running` without `ended_at` → output
   present → `done` with result `DONE`, except `preflight.json`/`disk/*.json` `.abort` non-null (→ `failed`, result =
   that abort); else re-spawn `attempt + 1` within `crash_respawn_per_stage`, over → `failed`. Background rows died with
   the session: same rule.
4. **4a** `S4:*` `running` without `ended_at`: output present → `code/*.json` (result = `.status`, row status by the "Row ids"
   mapping) or `gate/ci_parity.json` (`.pass` false → result `FAILED`, row `done`: S4z continues either way). Else row
   `invalidated`, `bump crash <unit_fs>` (over `crash_respawn_per_stage` → `SKIP cap:crash_respawn_per_stage`, unit
   unresolved) and a fresh `NN`: the invalidated row's brief has `withdraw: true`, or that unit's `plan.json .order[]`
   status is `withdrawn` → re-spawn `AMEND` with that same withdraw brief (or its `.wd` derivative) and **never** an
   `o-resume` brief — the withdraw procedure is idempotent by construction, the reconcile is not and would
   re-implement the unit the run just removed (the one carve-out from S4's "Spawn only `plan_order` entries with
   `status == planned`"); else a code unit with `snap_tree` ≠ the last `snapshots.tsv` tree → AMEND with
   `rca/briefs/o-resume-<NN>.json` (`required_change` "reconcile partial edits to plan §2+§4[<unit>]"), else the row's mode
   and brief again; neither bumps `amend_codegen`.
   **4b** Then, whether or not 4a applied, finish what the crash cut, in this order:
   - *S3 follow-up*: the latest `S3:*` row is a `done` `AMEND` whose brief is `o-pc-*`, `o-sg-*`, `o-wd-*`, `o-lb-*`
     or a 2.6e `b<N>-<nn>` → the spawns of its S3 follow-up that no `S4:*` row started after it carries (brief = that
     brief or its `.u`/`.nc`/`.wd` derivative, per unit; write a missing derivative first). An `invalidated` row never
     counts as carrying a brief — 4a re-spawned it, or nothing did. A `done` S3 `AMEND`'s
     `CHANGED_UNITS` = `plan.json | [.amendments[] | select(.brief_id == <brief_id>)] | last | .units` (the newest
     entry: a brief amended twice, e.g. an `o-lb-sg` escalation, has several).
   - *S4 table*: none left, or no such row → with no `S4:*:__finalize__` row, by the last UCS `S4:*` row (`started_at`):
     `failed`, or `blocked` with result ≠ `BLOCKED` → its table row, skipping each step with a `done` row started after
     it (S1m any; links, S2, S3 with the action's brief).
   - `S4:pending:*` units, `NEW` in `plan_order`.
   - *Loop-back*: the latest `S5:rca:<N>` row is `done`, `rca/r<N>.json` has a non-null `brief_ref` and no `S5:exec:*`
     row started after it → the Loop-back protocol for round `N` again, minus every spawn a `done` row started after
     the RCA row already covers: same stage and brief (or its `.u`/`.nc`/`.wd` derivative; an escalation
     `o-lb-*-<N>-<brief_id>` counts as that brief), or, for S1m, `__finalize__` and Env, a `done` row of that stage
     started after the latest spawn of an earlier rank. Then rule 7 (reuse a `test/status_updates/u*.json` newer than
     the RCA row's stamp) and the retest.

   Any other `snap_tree` ≠ the last tree → snapshot now. Never restore.
5. Re-derive spawns whose trigger died with the session: U0 `done`, `__hs__` `planned`, `hs_mode = worktree`, no
   `S4:*:__hs__` row → `__hs__`; S0 `done` and no `BASELINE` row → the warm-build waiter; an `S5:exec:*` row `done`
   with no `S5:e2e:*` row and `env/env.json .hs.available` true → the E2E stage.
6. Re-create the waiter or timer of every unsatisfied join (timers with the remaining minutes).
7. The next exec `N` = `counters.exec_round + 1` (bookkeeping rounds write no `r<N>.json`). `deadline_at` is kept.

## Final status and Output

On every terminal exit (success, skip, abort, failure, any stop), under R12: `kill_warm`; when `env/env.json` exists,
`. "$R/env/.vars.sh"` and `own_stop "$R/env/ucs.pid" "$(jq -r .ucs.binary "$R/env/env.json")" 20` (`2.6a_test_env.md`
Phase 0), the same for `env/hs.pid` with `.hs.binary` and 30 when `.hs.pid` is non-null (run-owned), and `docker stop`
each `.hs.db_containers[]` (2.6a REPAIR restarts them on Resume); after S7 returned, also `docker rm` them and
`rm -f ~/.local/state/grace/<run_id>/secrets.env` (earlier stops keep both for Resume);
`rj --arg s <STATUS> '.ended_at = (now | todate) | .status = $s'` (STATUS from the table below); write `summary.md`
(`.tmp` + `mv`) from files only — this return block, the per-unit table of `pr/status.json` (else `test/final.json`),
the `run.json` rows (id, result, attempt, started, ended), the `events.log` `SKIP` and `LOOPBACK` lines, and `flags`.

| Condition | STATUS |
|---|---|
| `pr/result.json .prStatus` `READY` or `PARTIAL` | `SUCCESS` |
| S0 `ABORT_CREDS`; every unit `no_op` | `SKIPPED` |
| anything else (`INCOMPLETE`, `FAILED`, S7 `ABORT_*`, any stop) | `FAILED` |

**`no_op` restore** (R4 carve-out): on `SKIPPED` because every unit is `no_op`, when `claimed_ucs` lists only
`data/integration-source-links.json` → `git restore --worktree -- data/integration-source-links.json`,
`ev SKIP S1m reason=unit:no_op`. Otherwise a stop after S1m and before S7 leaves its claimed edits (S1m's links file at
least) in the tree, and R4 forbids restoring them: append `dirty=<git status --porcelain --untracked-files=no | wc -l>`
to REASON. The next run on this checkout returns `ABORT_DIRTY` until the operator resumes this `RUN_ID` (resumable stops) or discards the edits.

```
CONNECTOR: {CONNECTOR}
STATUS: SUCCESS | FAILED | SKIPPED
LINKS: found | missing | <n> links          (connector entry of data/integration-source-links.json)
PR: <pr/result.json .prUrl | not created>
E2E: <unit>=<test/final.json .units[].e2e_status>, …    (untested unit: E2E_SKIPPED)
HS_PR: <pr/result.json .hsPrUrl | none required (HS_CHANGES_REQUIRED none) | not assessed (no HS checkout) | not created>
HS_CHANGES_REQUIRED: <.what of plan.json .hs_changes[] with withdrawn != true, joined with "; " | none | not assessed (plan.json .hs_changes is null)>
REASON: <PR_STATUS and pr/status.json .reasons | stop cause | NO_TASK_TOOL>
RUN_DIR: {RUN_DIR} | none
UNITS: <unit>=<FLOW_STATUS>, …              (pr/status.json; no PR → UNRESOLVED, no_op units → no_op)
OPEN_BUGS: <bug_id>(<severity>,<status>), … | none
```

## Subagent Reference

| Row id | File | Return `STAGE` | Primary output | Modes |
|---|---|---|---|---|
| `S0`, `DISK:*` | `2.0_preflight.md` | `S0` | `preflight.json`, `disk/<tag>.json` | `FULL`, `DISK_ONLY` |
| `S1:links:*` | `2.1_links.md` | `links` | `links/<unit_fs>.json` | re-run, `AMEND_BRIEF` |
| `S1:hs_scout` | `2.1a_hs_scout.md` | `hs_scout` | `hs/scout.json` | — |
| `S2:*` | `2.2_techspec.md` | `techspec` | `spec/result.json` | `REGENERATE`, `AMEND` |
| `S3:*` | `2.3a_plan.md` | `S3` | `plan/plan.md` | `NEW`, `AMEND` |
| `S4:*` | `2.3b_codegen_unit.md` | `S4`, `S4z` | `code/<NN>-<unit_fs>.json`, `gate/ci_parity.json` | `NEW`, `AMEND` |
| `BASELINE`, `S5:env:*` | `2.6a_test_env.md` | `test_env` | `test/baseline.json`, `env/env.json` | `BASELINE`, `POST_CODEGEN`, `REPAIR` |
| `S5:exec:*` | `2.6d_test_exec.md` | `test_exec` | `test/results/r<N>.json` | `FULL_RUN`, `ROUND` |
| `S5:rca:*` | `2.6e_rca.md` | `rca` | `rca/r<N>.json` | — |
| `S5:e2e:*` | `2.5_e2e.md` | `e2e` | `e2e/<N>.json` | — |
| `S6:review:*` | `2.7_review.md` | `S6` | `review/findings.json` | `FULL`, `INCREMENTAL` |
| `S7` | `2.8_pr_run.md` | `S7` | `pr/result.json` | — |

Cited by stage files, never spawned by this run: `2.3_codegen.md`, `2.4_pr.md`, `2.5_e2e.md`.
