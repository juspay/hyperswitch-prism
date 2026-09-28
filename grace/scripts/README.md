# GRACE supervisor

Runs connectors through GRACE one at a time, indefinitely, on a **Linux** host.

```
queue.json ──► grace_supervisor.py ──► tmux: claude -p "2_connector.md …"
                      │                          │
                      │                          └─► PR on juspay/hyperswitch-prism
                      ├─ usage_gate.py   (claude -p "/usage", weekly %)
                      ├─ task_update.py  (task.json ledger, run_id scoped)
                      └─ build_tracker.py in the dashboard repo → /tracker
```

## Run it

```bash
cp grace/scripts/queue.example.json grace/scripts/queue.json   # edit paths
python3 grace/scripts/grace_supervisor.py --config grace/scripts/queue.json --dry-run
python3 grace/scripts/grace_supervisor.py --config grace/scripts/queue.json
tmux attach -t grace-braintree      # watch the live run
```

`--dry-run` prints the gate results and the exact launch command, changing
nothing, and works on macOS. `--once` runs a single connector and exits.

## Gates, checked before every launch

| gate | on failure |
|---|---|
| tracked tree clean | **stop the queue** |
| weekly usage < threshold | **hold**, re-check every 30 min |
| free disk ≥ `min_free_gb` | **stop the queue** |

The dirty-tree gate never auto-cleans. A run stopped before S7 leaves its
claimed edits in the tree on purpose (`2_connector.md:809-812`) and those edits
are that run's resume state — `git stash` would trade a loud stop for quietly
destroying recoverable work. Recover by resuming that `RUN_ID`, or by discarding
after reading `grace/runs/<run_id>/claimed.tsv`.

## Exit codes

`0` queue drained · `1` held on usage (with `--once`) · `2` halted, needs a human.

`RestartPreventExitStatus=2` in the unit: a restart loop cannot fix a dirty tree
or a full disk, it can only burn a session per attempt.

## Two things that look like bugs and are not

**`STATUS: FAILED` does not mean no PR.** An `INCOMPLETE` run raises one
deliberately so the breakage is visible (`1_orchestrator.md:84`). The ledger
records the PR whenever `pr/pr.json` exists, whatever the verdict.

**`RUN_ID` is not chosen by the supervisor.** `2_connector.md:90` draws it from
`/dev/urandom`, and passing `RUN_ID` in means *resume this run*, not *use this
id*. The run directory is found by globbing `grace/runs/<connector_lc>-*` for a
`run.json` newer than the launch timestamp.
