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
- R2 **Fan-out only for independent work**: several Task calls in one message only for the S1 wave (≤6 per message). Background agents only for: warm builds, test_env BASELINE, the test lane (2.6b→2.6c), HS router build.
- R3 **The UCS working tree is sequential**: S1m, codegen units, AMENDs, `__finalize__`, `__promote__` and 2.8 run one at a time; test exec never overlaps a tree writer or a cargo build. Background agents write only the run dir or `hs-wt`.
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
{ sed -nE 's/^pub struct ([A-Za-z0-9]+);.*/\1/p' crates/types-traits/domain_types/src/connector_flow.rs | awk '{print tolower($0) "\t" $0}'
  awk -F'|' '/^## FLOW-GROUP MAP \(Authoritative\)/{f=1;next} f&&/^## /{exit} f&&$2~/`/{g=$2; gsub(/[` ]/,"",g)
    n=split($3 "," g, a, ","); for(i=1;i<=n;i++){x=a[i]; gsub(/[` ]/,"",x); if(x!="") print tolower(x) "\t" g}}' \
    grace/rulesbook/codegen/.gracerules_add_flow
  printf '%s\tIncomingWebhook\n' incomingwebhook webhook webhooks; } > "$R/.names.tsv"
UNITS=$(printf '%s' '{FLOWS}' | tr -d '[]"' | tr ',' '\n' | awk -F'\t' 'NR==FNR{m[$1]=$2;next} {gsub(/^ +| +$/,"")
  if($0=="")next; f=$0; p=""; i=index(f,"/"); if(i){p=substr(f,i); f=substr(f,1,i-1)} if(tolower(f) in m) f=m[tolower(f)]
  u=f p; if(!(u in s)){s[u]=1; o=o (o?",":"") u}} END{print o}' "$R/.names.tsv" -)
```

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
          amend_codegen{<unit>}, inner_input{<case_id>}, inner_request{<case_id>}, env_repairs, missing{<file>},
          crash{<unit_fs>}},
 flags[] (TEST_ENV_FAILED | TEST_DESIGN_FAILED | TEST_REQUESTS_FAILED | SECRET_LEAK | TREE_MODIFIED), blocking_open[],
 plan_order[{seq, unit, status}],
 rows[{id, status: pending|running|done|failed|skipped|blocked|invalidated, result, attempt, bg, started_at, ended_at,
       output, mode, brief, units[]}],
 bugs{<bug_id>: {origins[], briefs[], reappeared, escalated}}}
```

Row ids: `S0`, `S1:links:<unit_fs|common>`, `S1:hs_scout`, `S1m:<k>`, `S2:<k>`, `BASELINE`, `S3:<k>`, `T:design:<k>`,
`T:requests:<k>`, `S4:<NN>:<unit_fs>`, `S5:env:<N>:<k>`, `S5:exec:<N>`, `S5:rca:<N>`, `S5:probe:<bug_id>`,
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

**`orch.sh`**:

```bash
R=grace/runs/<run_id>; RUN_ID=<run_id>
now() { date -u +%FT%TZ; }
ev() { printf '%s %s %s %s\n' "$(now)" "$1" "$2" "${3:-}" >> "$R/events.log"; }
rj() { jq "$@" "$R/run.json" > "$R/run.json.tmp" && mv "$R/run.json.tmp" "$R/run.json"; }
late() { [ "$(date -u +%s)" -ge "$(date -u -d "$(jq -r .deadline_at "$R/run.json")" +%s)" ]; }   # R9
bump() { rj --arg a "$1" --arg b "${2:-}" '(["counters",$a]+(if $b=="" then [] else [$b] end)) as $p | setpath($p; (getpath($p)//0)+1)'
  jq -r --arg a "$1" --arg b "${2:-}" '.counters[$a] | if $b=="" then . else .[$b] end' "$R/run.json"; }
row_start() {  # id mode brief units bg(true|false); "-" = none. Stamp: Resume counts only outputs newer than it
  mkdir -p "$R/.stamp" && touch "$R/.stamp/$1"
  rj --arg id "$1" --arg m "$2" --arg b "$3" --arg u "$4" --argjson bg "$5" --arg t "$(now)" '
    def opt: if . == "-" or . == "" then null else . end;
    ([.rows[] | select(.id == $id)][0].attempt // 0) as $a
    | ("S4:pending:" + ($id | sub("^S4:[0-9]+:"; ""))) as $pend
    | .rows = [.rows[] | select(.id != $id and .id != $pend)] + [{id: $id, status: "running", result: null, attempt: ($a + 1), bg: $bg,
        started_at: $t, ended_at: null, output: null, mode: ($m | opt), brief: ($b | opt),
        units: ($u | split(",") | map(select(. != "" and . != "-")))}]'
  ev SPAWN "$1" "attempt=$(jq -r --arg id "$1" '.rows[] | select(.id == $id) | .attempt' "$R/run.json") mode=$2 brief=$3 units=$4"; }
row_end() {  # id row_status agent_STATUS output
  rj --arg id "$1" --arg s "$2" --arg r "$3" --arg o "$4" --arg t "$(now)" \
    '(.rows[] | select(.id == $id)) |= (.status = $s | .result = $r | .ended_at = $t | .output = $o)'
  ev RETURN "$1" "status=$3 output=$4"; }
claimed_ucs() { [ -f "$R/claimed.tsv" ] && cut -f1 "$R/claimed.tsv" | grep -v '^hs:' | sort -u; }
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
note_exec() { rj --slurpfile r "$R/test/results/r$1.json" 'reduce $r[0].bugs.reappeared[] as $b (.; .bugs[$b].reappeared += 1)'; }
note_rca() { rj --slurpfile a "$R/rca/r$1.json" 'reduce ($a[0][] | . as $e | .bug_ids[] | {b: ., o: $e.origin, r: $e.brief_ref}) as $x (.;
    .bugs[$x.b].origins += [$x.o // empty] | .bugs[$x.b].briefs += [$x.r // empty])'; }
prev_origin() {  # bug_ids_csv → {"<bug_id>": "<last origin>"} for first reappearances, once per bug
  jq -c --arg ids "$1" '[.bugs | to_entries[] | select((.key | IN($ids | split(",")[])) and .value.reappeared == 1
    and (.value.escalated | not) and (.value.origins | length) > 0) | {(.key): .value.origins[-1]}] | add // {}' "$R/run.json"
  rj --arg ids "$1" 'reduce ($ids | split(",")[]) as $b (.; if .bugs[$b].reappeared == 1 and (.bugs[$b].origins | length) > 0
    then .bugs[$b].escalated = true else . end)'; }
select_cases() {  # N bug_ids_csv changed_units_csv → test/select/r<N>.json = (R1 ∪ R2 ∪ R3 ∪ R4) − withdrawn units
  jq -n --arg b "$2" --arg u "$3" --slurpfile c "$R/test/cases.json" --slurpfile g "$R/test/bugs.json" --slurpfile p "$R/plan/plan.json" '
    ($b | split(",")) as $B | ($u | split(",")) as $U | $c[0].cases as $K
    | ([$g[0].bugs[] | select(.bug_id | IN($B[])) | .cases[]] + [$K[] | select(any(.regression_for[]?; IN($B[]))) | .case_id]) as $r1
    | [$K[] | select(.unit | IN($U[])) | .case_id] as $r2
    | def deps($s): ([$K[] | select(any(.depends_on[]?; .case_id | IN($s[]))) | .case_id] + $s | unique);
      ($r1 + $r2 | unique | until(deps(.) == .; deps(.))) as $r3
    | [$K[] | select((.unit | IN($U[]) | not) and (.surface | IN("ucs_grpc", "static"))) | .case_id] as $r4
    | [$p[0].order[] | select(.status == "withdrawn") | .unit] as $W
    | ($r3 + $r4 | unique) - [$K[] | select(.unit | IN($W[])) | .case_id]' > "$R/test/select/r$1.json.tmp" \
  && mv "$R/test/select/r$1.json.tmp" "$R/test/select/r$1.json"; }
kill_warm() {  # [ucs|hs …] (default both) — R12 carve-out: the setsid bash wrapper leads its process group
  local n p; for n in ${@:-ucs hs}; do p=$(cat "$R/warm/$n.pid" 2>/dev/null) || continue
    case "$(readlink "/proc/$p/exe" 2>/dev/null)" in */bash) ;; *) continue;; esac
    { tr '\0' ' ' < "/proc/$p/cmdline"; } 2>/dev/null | grep -qF "$R/warm/warm.sh" && kill -TERM -- "-$p" 2>/dev/null; done; }
build_units() {  # build_log ""|hs: → JSON array of the units whose claimed files the compiler errors name, else ["__finalize__"]
  local u; u=$(grep -oE -- '--> [^:]+' "$1" | cut -c5- | sed "s#^#$2#" | sort -u \
    | awk -F'\t' 'NR==FNR{if($3!="-")c[$1]=c[$1] $3 ","; next} ($0 in c){printf "%s", c[$0]}' "$R/claimed.tsv" - \
    | tr ',' '\n' | grep -v '^$' | sort -u | jq -Rcn '[inputs]'); [ "$u" = '[]' ] && u='["__finalize__"]'; echo "$u"; }
```

`snapshots.tsv` = `seq ref stage unit commit tree utc`. Snapshot after every UCS tree-writing return: S1m (`s1m`), each
UCS-unit S4 spawn with non-empty `CHANGED_UNITS` (`<NN>-<unit_fs>`), S4z and every later `__finalize__`
(`<NN>-finalize`), `__promote__` (`<NN>-promote`), and before S6 when `snap_tree` ≠ the
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
(`plan/plan.rev<rev-1>.json` → `plan/plan.json`): §4 for 2.3b, §8 test hooks for 2.6b" (`u`; 2.3b applies a non-empty
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
   BASELINE, `T:design`, `T:requests` and `__hs__` spawns carry `ROW_ID: <row id>`; the agent runs
   `test -e {RUN_DIR}cancelled/{ROW_ID}` before every final `mv` (2.6a also before a server start or stop; 2.3b also
   before each edit batch and gate launch) and, when present, returns `FAILED`, REASON `CANCELLED`, writing nothing more (no task-stop tool needed; use one too if the
   harness has it).

| Join | Blocks | Satisfied by | Cap (`## Caps`) |
|---|---|---|---|
| warm UCS build | BASELINE spawn | `warm/ucs_build.exit` (`2.0_preflight.md` "Phase 9: Background warm builds") | `warm_build_wait_min` → `kill_warm ucs`, spawn BASELINE anyway |
| BASELINE | S3, first S4 spawn | `BASELINE` row terminal | `baseline_join_min` from the later of S2's return and BASELINE's spawn → join rule 4 |
| test lane | S4z Spec-gap round and code audit, S5 | `test/requests/index.json` exists (2.6c moves it last) and no `T:*` row `running`, or flag `TEST_DESIGN_FAILED`/`TEST_REQUESTS_FAILED` | `test_lane_join_min` → flag `TEST_DESIGN_FAILED` (`T:design` running) else `TEST_REQUESTS_FAILED` |
| `__hs__` | S5 | its `S4:<NN>:__hs__` row terminal; **or** no `S4:*:__hs__` row and (`run.json .hs_mode` ≠ `worktree` or no `__hs__` entry in `plan_order`) → satisfied at once, `ev JOIN __hs__ present`, no timer | `hs_join_min` → `plan.json .hs_changes` stay unresolved; S5 continues |

## Pipeline

```
init ─ S0 ─┬─ S1 wave (links common | links × unit | hs_scout) ─ S1m ─ S2 ─ S3 ─ S4 U0…Un ─ S4z ─ S5 ─ promote ─ S6 ─ S7
           │                                                              └ bg __hs__ (after U0)
           ├─ waiter warm/ucs_build.exit ─ bg BASELINE ──────────── joined by S3 and S4
           └─ bg test lane after S3: 2.6b CREATE → 2.6c CREATE ──── joined by S4z audit and S5
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

`ABORT_CREDS` → stop → SKIPPED. Other `ABORT_*` or `FAILED` → stop → FAILED. No PR either way. `DONE` → copy, start the
warm-build waiter (cap `warm_build_wait_min`), → S1:

```bash
rj --slurpfile p "$R/preflight.json" '.branch = $p[0].branch | .base_sha = $p[0].base_sha | .ports = $p[0].ports
  | .workflow_dir = ($p[0].workflow.dir // "grace/workflow") | .hs_mode = $p[0].hs.mode'
```

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
**and** S2's return, before S3: `jq -e '[.[] | select(.probe // "" | startswith("deferred:"))] | length > 0'
"$R/test/capabilities.json"` → one foreground 2.6a spawn (`S5:env:0:<k>`), the same template with `PROBE_ONLY: 1`, so
the entries become `provisioned`/`not_provisioned` before 2.6b reads them (without it no case can ever be
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

Record `plan_order` (`jq '[.order[] | {seq, unit, status}]' "$R/plan/plan.json"`); pre-create a row
`{id: "S4:pending:<unit_fs>", status: "pending", attempt: 0}` per `planned` unit with no `S4:*:<unit_fs>` row yet. `DONE`/`PARTIAL` → every unit `no_op` → stop → SKIPPED ("already implemented"); else test lane → S4.
`SPEC_GAP` → once, within caps: `rca/briefs/o-s3-gap.json` (the `spec_gap` units, `finding` = REASON, `required_change`
"answer <topic> in the spec") → links per unit (FOCUS = topic) → S1m → S2 `AMEND` → S3 `AMEND` (same brief) → test
lane → S4; units still `spec_gap` are not implemented. `BLOCKED` → S7 gate. `FAILED` → stop → FAILED, no PR.

### Test lane — `2.6b_test_design.md` → `2.6c_test_requests.md` (background after S3; foreground for later AMENDs)

```
T:design:    CONNECTOR: <connector_lc>
             UNITS: <units csv>
             RUN_DIR: {RUN_DIR}
             TECHSPEC_PATH: {TECHSPEC_PATH}
             MODE: CREATE | AMEND
             AMEND_BRIEF: <brief>  |  AMEND_CASES: <ids | __code_audit__>          (AMEND only, one of them)
             ROW_ID: <row id>
T:requests:  CONNECTOR: <connector_lc>
             RUN_DIR: {RUN_DIR}
             MODE: CREATE | AMEND | PROBE
             AMEND_BRIEF: <brief>  |  AMEND_CASES: <ids | __cases_rev__>           (AMEND only, one of them)
             PROBE_BUG_ID: <bug_id>                                                (PROBE only)
             ROW_ID: <row id>
```

`T:design` `CREATE` `DONE`/`SPEC_GAP` → `T:requests` `CREATE` (cases of open gaps are skipped; S4z's **Spec-gap
round** answers them). Every `T:design` `AMEND` `DONE` → `T:requests` `AMEND_CASES: __cases_rev__` (`NO_CHANGE` → none).
`T:requests` `CREATE` `DONE`/`PARTIAL` → lane done. `BLOCKED`/`FAILED` → one re-spawn; still: a `CREATE` → flag
`TEST_DESIGN_FAILED` (`T:design`) or `TEST_REQUESTS_FAILED` (`T:requests`), either of which skips the S4z rounds and S5
(→ S6). An `AMEND` or `PROBE` never sets a flag; the run continues: in a loop-back, Loop-back rule 4 (brief chain
dropped, its bugs `unresolved`); in the inner loop, its case ids join `test/select/not_run.json` (JSON array, `.tmp` +
`mv`; 2.6d runs them as `NOT_RUN` with a decision row); elsewhere (S4z rounds, code audits, probes) that step ends.
The entry is per-failure, not permanent: every later `T:design`/`T:requests` spawn returning `DONE` first rewrites
`test/select/not_run.json` (`.tmp` + `mv`) without the case ids it re-rendered (its `CHANGED_UNITS`' cases, or
`test/requests/index.json .requests[]` for a `__cases_rev__`), so a transient test-agent failure cannot sink a case
for the rest of the run.

**Code audit**, unless a `TEST_*_FAILED` flag: after every `__finalize__` except the promotion's, and after every round
of 2.3b AMENDs (loop-back rank 5, S5 build fix) → `T:design` `AMEND_CASES: __code_audit__`.

### S4 — `2.3b_codegen_unit.md`, one unit per message in `plan_order` (foreground)

Before the first spawn: BASELINE join, R10 (`DISK_TAG: pre-S4`). Spawn only `plan_order` entries with
`status == "planned"` (plus the `__hs__` rule below); every other entry (`no_op`, `blocked`, `spec_gap`, `withdrawn`)
is left out of the run with `ev SKIP S4:<NN>:<unit_fs> reason=unit:<status>` and no spawn. Every 2.3b spawn (NEW or
AMEND, from S4, S4z, S5, loop-back or promotion) uses this template with a fresh `NN` = `printf %02d $(( $(bump nn) - 1 ))`.

```
  CONNECTOR: <connector_lc>
  UNIT: <unit | __hs__ | __finalize__ | __promote__>
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
| `FAILED`, REASON `foundation smoke: …` | AMEND with `rca/briefs/o-smoke-<NN>.json` (`required_change` "make the foundation smoke pass: <REASON>", evidence `code/<NN>-<unit_fs>.smoke.*`) and `SMOKE: 1`, so `DONE` means the smoke re-ran green. That AMEND `NO_CHANGE`, or at cap `amend_codegen_per_unit` → `SKIP cap:amend_codegen_per_unit`, next unit; round 1 `FULL_RUN` re-runs the unit's `ucs_grpc` happy case, which files the bug for RCA |
| `FAILED` (other) | AMEND with `rca/briefs/o-gate-<NN>.json` (`required_change` "finish plan §4[<unit>]: <REASON>"); at cap → withdraw |
| `PLAN_CONFLICT` | S3 `AMEND` with `rca/briefs/o-pc-<NN>.json` (`origin: PLANNER`, `required_change` "resolve: <REASON>") → S3 follow-up; conflict again at cap → withdraw |
| `SPEC_GAP` | `rca/briefs/o-sg-<NN>.json` → links for the unit (FOCUS = REASON topic) → S1m → S2 `AMEND` → S3 `AMEND` → S3 follow-up; at cap → withdraw |
| `BLOCKED` | recorded: `withdraw: shared code` → unit unresolved; `no hs-wt` → `__hs__` skipped |

Withdraw = `rca/briefs/o-wd-<unit_fs>.json` (`withdraw: true`, `fix: []`, `required_change` "<unit>: <why>") → S3 `AMEND`
→ S3 follow-up → S4z. 2.3a also marks every §9 `hs_changes[]` item whose `flows_unblocked` are all withdrawn
`withdrawn: true` (`2.3a_plan.md` "Phase 12: AMEND" step 3), so no HS PR is raised for a flow UCS no longer
implements; an `hs-wt` commit already made for such an item is simply never pushed. **S3 follow-up** of an S3 `AMEND` on brief `B` (here and in Loop-back rules 2 and 4): 2.3b `AMEND`
of `B`'s units (its 2.3b `amend_targets[].scope.units`, else `units`) with `B` (S3 `NO_CHANGE` → `B.nc`),
then of every other unit of that S3 `AMEND`'s `CHANGED_UNITS` with an `S4:*` row started before it, with `B.u`. §8 hook changes
from these S3 `AMEND`s (`test_hooks_changed`) reach 2.6b in S4z's hooks round.

### S4z — `__finalize__` (foreground)

After the last UCS unit: R10 (`pre-S4z-<k>`), S4 template with `UNIT: __finalize__`, `MODE: NEW`; `guard`, snapshot.
`DONE` or `FAILED` → continue (S7 gates again). Join the test lane; unless a `TEST_*_FAILED` flag is set: the
hooks round, the Spec-gap round, then the code audit.

**Hooks round** — `H=$(jq -c '[.amendments[]?.test_hooks_changed[]?] | unique' "$R/plan/plan.json")`
`!= '[]'` and no `rca/briefs/o-hooks.json` yet → write it (`origin: PLANNER`, `units` = `H`, evidence
[`plan/plan.json`], `required_change` "reconcile cases to plan §8 test hooks of these units (plan rev <rev>)") →
`T:design` `AMEND_BRIEF` = it → `__cases_rev__`.

**Spec-gap round** — only `test/cases.json` `spec_gaps[]` entries with `status: "open"` count:
`jq '[.spec_gaps[]? | select(.status == "open")] | length' "$R/test/cases.json"` > 0 and no `rca/briefs/o-tgap.json` yet →
within caps (`amend_links`, `amend_techspec`): `rca/briefs/o-tgap.json` (`origin: LINKS`, `units` = the open gaps'
units, `finding` = their `question`s, evidence [`test/cases.json`], `required_change` "answer in <section>: <question>"
per gap) → links per gap unit (FOCUS = its questions) → S1m → S2 `AMEND` → `T:design` `AMEND_BRIEF` (same brief)
→ `__cases_rev__`. A target `FAILED` ends the round. Gaps still open are ingested in round 1 and RCA routes them once.

### S5 — test loop

Every `2.6d_test_exec.md` spawn, in this order: `N=$(bump exec_round)`; for `MODE: ROUND` write its selection to
`test/select/r$N.json` (`select_cases $N …` for a retest; otherwise a JSON array of case ids via `.tmp` + `mv`, "same
selection" = a copy of the previous file); stamp `S5:exec:$N`; spawn with `ROUND: $N`. `ONLY_CASES: none` is a
bookkeeping call that writes no `test/results/r<N>.json`.

**ENV repairs**: every `REPAIR` spawn first `bump env_repairs`; over `caps.env_repairs_per_round` →
`SKIP cap:env_repairs_per_round`, no spawn: in step 3 its `env` issues go on to RCA, elsewhere flag `TEST_ENV_FAILED`
→ S6. `bump rca_rounds` also runs `rj '.counters.env_repairs = 0'`.

**1. Env** — before the first exec and after every loop-back round that changed code, `hs-wt` or the env: R10
(`pre-r<N>`), join the test lane (a `TEST_*_FAILED` flag → skip S5 → S6) and `__hs__`, then `2.6a_test_env.md`
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
  ONLY_CASES: {RUN_DIR}test/select/r<N>.json   (ROUND only; omitted for FULL_RUN — `none` is the bookkeeping sentinel of the S7 prelude, never a FULL_RUN value)
  INGEST: <baseline,spec_gaps | review>
  STATUS_UPDATES: {RUN_DIR}test/status_updates/u<k>.json
```

Round 1 is `FULL_RUN` with `INGEST` = `baseline` (BASELINE `done` and `test/baseline_bugs.json` exists) + `spec_gaps`
(`test/cases.json` has a `spec_gaps[]` entry with `status: "open"`). Every `DONE`/`PARTIAL` that wrote `test/results/r<N>.json` → `note_exec <N>`. `DONE` → 3. `PARTIAL` → `REPAIR` (`RESULTS`) → `ROUND` over its
`NOT_RUN` cases. `BLOCKED` (`ENV`) → `REPAIR` (`rca/briefs/o-env-r<N>.json`) → same selection. `FAILED` `ROUND_EXISTS` →
same spawn, next `N`. `FAILED` `SECRET_LEAK` → flag `TEST_ENV_FAILED` + `SECRET_LEAK` → S6 (as the env path at step 1:
`TEST_ENV_FAILED` is what makes `exec_ready` false, so the S7 prelude does not re-enter the same security gate, and
what 2.8 keys `INCOMPLETE` on). `FAILED` `MISSING <file>` → once per file
(`bump missing <file>` = 1) its owner, then the same spawn with the next `N`: `env/env.json` → Env `POST_CODEGEN`;
`test/cases.json` → `T:design` `CREATE` (→ `T:requests` `CREATE`); `test/requests/index.json` → `T:requests` `CREATE`.
Again, or `MISSING creds` (no owning stage) → flag `TEST_ENV_FAILED` → S6.

**3. Inner loop** — `jq '{env: .env_issues, design: .test_suspect.design, request: .test_suspect.request, stale: .test_suspect.stale_root}' "$R/test/results/r<N>.json"`;
`stale` (uncapped) → one `T:requests` `AMEND_CASES: __cases_rev__` (2.6c re-stages the stale roots; no counter bump; a
case stale again in the next `ROUND` counts as `request`). Only cases under cap (`inner_request_per_case`, `inner_input_per_case`, ENV repairs): `request` →
`T:requests` `AMEND_CASES`; `design` (+ `test/requests/index.json .unrenderable`) → `T:design` `AMEND_CASES` →
`T:requests` `__cases_rev__`; `env` → `REPAIR` (`RESULTS`). Each capped AMEND bumps `inner_request`/`inner_input` per case id;
one returning `NO_CHANGE` sets its cases' counters to the cap (still `BLOCKED`/`FAILED` after its re-spawn: Test lane). Then one `ROUND` over those ids (`stale` included) plus the suspects
already at cap (its own `test/select/r$N.json`): 2.6d compares `run.json .counters.inner_input{<case_id>}` /
`inner_request{<case_id>}` with `.caps` and files an at-cap suspect as a bug (design → `spec_gap`, request →
`request_unrenderable`). Repeat while suspects under cap remain; when only at-cap suspects remain, that one `ROUND`, then 4.

**4. RCA** — `2.6e_rca.md` (`S5:rca:<N>`, `N` = latest exec round with `test/results/r<N>.json` and no `rca/r<N>.json`).
`BUG_IDS` = in-scope bugs whose `flows[]` are not all markers of withdrawn units, with status `open`
(or `rca` after `needs_probe`), `duplicate_of` null, `attempts` <
`fix_attempts_per_bug` (`source: spec_gap`: `attempts` < 1 — one RCA round; its gap cases stay unexecuted until
the retest), `run.json .bugs[].reappeared` ≤ 1. Over the fix cap → `unresolved`;
reappeared twice → `unresolved` + withdraw brief for its units. No ids left, or `rca_rounds` at cap → 6.

```
  RUN_DIR: {RUN_DIR}
  ROUND: <N>
  BUG_IDS: <csv>
  PREVIOUS_ORIGIN: <prev_origin <BUG_IDS>; omit when {}>
```

`DONE`/`PARTIAL` → `bump rca_rounds`, `note_rca <N>`. `PARTIAL` with `NEXT` `2.6c_test_requests.md PROBE <bug_id>` →
per bug `S5:probe:<bug_id>` (`T:requests` `MODE: PROBE`, `PROBE_BUG_ID: <bug_id>`), then RCA again with those bugs: in
the retest's RCA (their ids join `select_cases`' bug ids), or, when this RCA wrote no brief, after a `ROUND` over their cases.
`BLOCKED` (`ENV`) → `REPAIR` → same RCA spawn. `FAILED` → one re-spawn, still → 6. Briefs → **Loop-back protocol** →
retest `ROUND` (`N=$(bump exec_round); select_cases $N <routed bug ids> <changed units>`, spawn with `ROUND: $N`) → 3.

**5. Stop early** — after each retest (withdrawn units' bugs excluded, as in 2.8's `pr/blocking_bugs.json`):

```bash
jq --slurpfile p "$R/plan/plan.json" '([$p[0].order[] | select(.status == "withdrawn") | .unit] as $w
   | [$p[0].units[] | select(.unit | IN($w[])) | .markers[]]) as $W
 | [.bugs[] | select(.blocking and .in_scope and (.status | IN("open","rca","fixing","retest"))
     and ((.flows // []) as $f | $f == [] or (($f - $W) | length > 0)))] | length' "$R/test/bugs.json"
```

→ append to `blocking_open`; not lower than the previous value → `SKIP stop_early` → 6.

**6. Converged** → Env if needed → one `FULL_RUN` carrying pending `STATUS_UPDATES`. New blocking bugs, `rca_rounds`
under cap and no stop-early → 4, then retests only (no second full run). Otherwise → 7.

**7. Promotion** (before S6) —
`jq --slurpfile f "$R/test/final.json" '[.promotions[]? | select($f[0].cases[.case_id].outcome == "PASS")]' "$R/test/requests/index.json"`;
empty or a file absent → S6. Else write `rca/briefs/o-promote.json` (`origin: REQUEST_GEN`, `units: ["__promote__"]`,
`finding` "staged connector_specs deltas whose cases PASS", evidence [`test/requests/index.json`, `test/final.json`],
`required_change` "apply promotions[] to crates/internal/integration-tests/src/connector_specs/<connector_lc>/",
`promotions` = that list) → 2.3b `UNIT: __promote__`, `MODE: AMEND`, `AMEND_BRIEF` = it (2.3b applies the staged deltas
and claims them); `guard`, snapshot → `__finalize__` again (its schema validators run because `connector_specs`
changed); `guard`, snapshot → S6. `FAILED` from either → recorded, S6.

### S6 — `2.7_review.md` (foreground)

```
  RUN_DIR: {RUN_DIR}
  MODE: FULL | INCREMENTAL
  BASE_SHA: <run.json .base_sha>
  SNAPSHOT_REF: <last ref in snapshots.tsv>
  PREVIOUS_SNAPSHOT_REF: <run.json .review_ref>                (INCREMENTAL only)
```

Take the `review` snapshot first; stamping the `FULL` spawn also sets `review_ref` = its `SNAPSHOT_REF`. `FULL` `DONE`
with S0/S1 findings (`jq '[.[] | select(.sev | IN("S0","S1"))] | length' "$R/review/findings.json"`), time left,
`exec_ready` and `bump review_rounds` ≤ `caps.review_remediation_rounds` → exec `ROUND`, `INGEST: review`, `ONLY_CASES` = `P0` cases of the findings'
units (all units when none) → RCA (step 4) → loop-back → retest → `INCREMENTAL`. No S0/S1 → S7. `FAILED` → one
re-spawn; still → S7.

**`exec_ready`** (guards every 2.6d spawn from S6 on): no `TEST_*_FAILED` flag is set and `test/cases.json`,
`test/requests/index.json` and `env/env.json` all exist — without them 2.6d returns `FAILED  MISSING <file|creds>`
(`2.6d_test_exec.md` "Inputs"), and the `FAILED` routing of the stage that just failed would re-spawn it. Not
`exec_ready` → no remediation round and no prelude exec: `ev SKIP S5:exec:<N> reason=flag:<TEST_*_FAILED>` (or
`reason=missing:<file>`) → S7. Skipping is fail-closed: the bugs keep their non-terminal status, which 2.8's
`pr/blocking_bugs.json` query already treats as blocking, and unfixed S0/S1 review findings block there too
(`2.8_pr_run.md` "Phase 1: Status → `pr/status.json`").

**S7 prelude** (`exec_ready` only): bookkeeping execs (`ONLY_CASES: none`, no `r<N>.json`): after every S6 return that
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
   `__finalize__` after codegen, and a `T:design` AMEND for every brief (each adds regression cases). Spawn by rank
   across all briefs (briefs ordered by earliest origin), one spawn at a time: 1 `2.1_links.md` → 2 S1m →
   3 `2.2_techspec.md` → 4 `2.3a_plan.md` → 5 `2.3b_codegen_unit.md` (`plan_order`, `__hs__` last, foreground) →
   6 `__finalize__` → 7 `2.6b_test_design.md` (each `DONE` followed by `T:requests` `__cases_rev__`; after any rank-5
   spawn also `__code_audit__`) → 8 `2.6c_test_requests.md` → 9 `2.6a_test_env.md` `REPAIR` → 10 Env `POST_CODEGEN`
   (**one rebuild per round**). Log `LOOPBACK <brief_id> origin= targets= units= round=` before a brief's first spawn.
2. **Scoped to affected units only.** Links: one spawn per `scope.units` entry (`links/common.json` when none), FOCUS =
   `scope.sections`, `AMEND_BRIEF`. 2.2 and 2.3a: one spawn per brief. 2.3b: after a 2.3a AMEND its S3 follow-up (S4),
   else `scope.units` with the brief. 2.6b: brief units ∪ the 2.3a AMEND's `test_hooks_changed` (`u` brief).
3. **NO_CHANGE propagation**: a target returning `NO_CHANGE` hands the `nc` brief (`upstream_no_change: true`) to the
   brief's later targets; live evidence decides.
4. **Caps before each spawn** (R8): at cap, or an `amend_targets[]` stage returning `FAILED` (a `T:*` target:
   `BLOCKED`/`FAILED` after its re-spawn) → **drop**: `SKIP cap:<key>`, the brief's remaining targets dropped, its bugs
   → `unresolved`; the run continues. Recorded, chain continues: `__finalize__` `FAILED` (S7 gates again, as in S4z);
   the added regression `T:design` (not in `amend_targets[]`) or its `__cases_rev__` failing (the bugs' own cases still
   retest). Escalations (`<N>` = this RCA round) run at once, within caps, once per brief; the same escalation again,
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
   `unresolved`, nothing withdrawn. Withdrawn units leave every retest (`select_cases`) **and every `FULL_RUN`**
   (`2.6d_test_exec.md` "Phase 2: Select cases and build chains" item 1), and their bugs count in neither step 4's
   `BUG_IDS` nor step 5's blocking count — otherwise step 6's convergence run re-executes a flow that is now
   `not_implemented`, files fresh bugs and routes RCA into re-implementing it.
6. **Reappearing fingerprint** (`r<N>.json .bugs.reappeared`): first time → RCA with `PREVIOUS_ORIGIN`, which moves the
   origin exactly one stage upstream (`2.6e_rca.md` "Phase 4: Escalation (`{PREVIOUS_ORIGIN}`)"); second time, or a
   hunk reversing an earlier fix (RCA compares snapshots) → `unresolved` + withdraw brief if the unit's code is not shared.
7. **Status updates** before the retest spawn, `test/status_updates/u<bump status_update>.json` (shape:
   `2.6d_test_exec.md` "Phase 1: Bookkeeping (INGEST, STATUS_UPDATES)"): per bug of `rca/r<N>.json` `open→rca`,
   `rca→<proposed_status>`, then `fixing→retest` (chain completed) or `fixing→unresolved` (dropped); plus the
   `unresolved` moves of 4–6. `update_id` = `u<k>-<bug_id>-<to>`, `by: orchestrator`, `ref` = brief or `rca/r<N>.json`.
8. **Re-test** (`select_cases`): **R1** the bugs' cases + their regression cases; **R2** all cases of changed units
   (`CHANGED_UNITS` of every AMEND this round); **R3** transitive `depends_on` dependents; **R4** fast replay of
   `ucs_grpc`/`static` cases in untouched units (their HS surfaces run only in the final full run).

## Caps

Single source of truth; stage files cite this section. Copied into `run.json .caps` at init.

| Cap | Default | `run.json .caps` keys |
|---|---|---|
| RCA rounds / fix attempts per bug | 3 / 2 | `rca_rounds` / `fix_attempts_per_bug` |
| AMEND: links / techspec / plan / codegen per unit / HS | 2 / 2 / 3 / 3 / 2 | `amend_links` / `amend_techspec` / `amend_plan` / `amend_codegen_per_unit` / `amend_hs` |
| Gate iterations per codegen spawn / finalize | 5 / 3 | `gate_iterations_codegen` / `gate_iterations_finalize` |
| Plan validator fix iterations (2.3a Phase 11, per spawn) | 3 | `validator_fix_iterations` |
| Test inner loops per case (input / request) / ENV repairs per RCA round | 2 / 2 / 2 | `inner_input_per_case` / `inner_request_per_case` / `env_repairs_per_round` |
| Review remediation rounds / crash re-spawn per stage | 1 / 1 | `review_remediation_rounds` / `crash_respawn_per_stage` |
| Stop early | a round where the blocking open count doesn't fall | `stop_early` |
| Warm UCS build wait / BASELINE join wait (minutes) | 120 / 180 | `warm_build_wait_min` / `baseline_join_min` |
| Test lane join wait / `__hs__` join wait (minutes) | 120 / 120 | `test_lane_join_min` / `hs_join_min` |
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
   `done` with output, or with result `NO_CHANGE` (2.6b/2.6c `NO_CHANGE` rewrites nothing) → skip; `done` otherwise →
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
5. Re-derive spawns whose trigger died with the session: latest `T:design:*` result `DONE`/`SPEC_GAP` and no
   `T:requests:*` row started after it → its `T:requests` follow-up (`CREATE` after `CREATE`, `__cases_rev__` after
   `AMEND`); S3 `done` and no `T:design:*` row → the test lane; U0 `done`, `__hs__` `planned`, `hs_mode = worktree`, no
   `S4:*:__hs__` row → `__hs__`; S0 `done` and no `BASELINE` row → the warm-build waiter.
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
| `T:design:*` | `2.6b_test_design.md` | `test_design` | `test/cases.json` | `CREATE`, `AMEND` |
| `T:requests:*`, `S5:probe:*` | `2.6c_test_requests.md` | `test_requests` | `test/requests/index.json` | `CREATE`, `AMEND`, `PROBE` |
| `S5:exec:*` | `2.6d_test_exec.md` | `test_exec` | `test/results/r<N>.json` | `FULL_RUN`, `ROUND` |
| `S5:rca:*` | `2.6e_rca.md` | `rca` | `rca/r<N>.json` | — |
| `S6:review:*` | `2.7_review.md` | `S6` | `review/findings.json` | `FULL`, `INCREMENTAL` |
| `S7` | `2.8_pr_run.md` | `S7` | `pr/result.json` | — |

Cited by stage files, never spawned by this run: `2.3_codegen.md`, `2.4_pr.md`, `2.5_e2e.md`.
