#!/usr/bin/env python3
"""Run GRACE over a connector queue, one connector at a time, indefinitely.

Takes the next connector off the queue, launches ONE `claude` session for it in
tmux running `grace/workflow/2_connector.md`, waits for that run to raise a PR,
records the result, then checks weekly Claude usage and either starts the next
connector or holds until the quota refreshes.

One session per connector on purpose. A single connector run costs roughly 528M
context tokens (measured, PR #2332), so batching several into one session is not
viable — and 1_orchestrator.md:8 says the same thing: "Single connector? Do not
use this file. Invoke grace/workflow/2_connector.md directly ... so nesting stays
flat." A fresh session also means a crash costs one connector, not the queue.

LINUX ONLY. 2.0_preflight.md:90 aborts with ABORT_NOT_LINUX when uname is not
Linux or /proc/self is missing, so on macOS every run dies at S0 having done
nothing. --dry-run works anywhere for inspecting the gates.

  grace_supervisor.py --config queue.json [--dry-run] [--once]
"""
import argparse, datetime, json, os, pathlib, re, signal, subprocess, sys, time

HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parent.parent
LEDGER = REPO / "task.json"
POLL_SECONDS = 60
HOLD_SECONDS = 30 * 60          # re-check /usage every half hour while held
STALL_HOURS = 4.0               # events.log quiet this long = wedged


def now():
    return datetime.datetime.now().astimezone().isoformat(timespec="seconds")


def log(msg):
    print(f"[{now()}] {msg}", flush=True)


def sh(args, cwd=None, timeout=120):
    """Run a command, never raise. Returns (rc, stdout, stderr)."""
    try:
        r = subprocess.run(args, cwd=cwd or REPO, capture_output=True,
                           text=True, timeout=timeout)
        return r.returncode, r.stdout, r.stderr
    except FileNotFoundError:
        return 127, "", f"{args[0]}: not found"
    except subprocess.TimeoutExpired:
        return 124, "", f"{args[0]}: timed out"


# ---------------------------------------------------------------- the gates

def gate_tree_clean():
    """A dirty tracked tree makes the next run ABORT_DIRTY at S0, instantly.

    Never auto-clean. A run stopped before S7 leaves its claimed edits in the
    tree ON PURPOSE (2_connector.md:809-812) and those edits ARE that run's
    resume state — `git stash` here would trade a loud stop for quietly
    destroying recoverable work. R4 and 2.0 RULES 3 forbid it.
    """
    rc, out, err = sh(["git", "status", "--porcelain", "--untracked-files=no"])
    if rc != 0:
        return False, f"git status failed: {err.strip()[:120]}"
    n = len([l for l in out.splitlines() if l.strip()])
    return (True, None) if n == 0 else (False, f"working tree dirty ({n} tracked path(s))")


def gate_usage(threshold):
    """(clear, detail). Anything but a confident 'under' holds the queue."""
    rc, out, err = sh([sys.executable, str(HERE / "usage_gate.py"),
                       "--threshold", str(threshold), "--json"], timeout=300)
    try:
        d = json.loads(out.strip().splitlines()[-1]) if out.strip() else {}
    except (json.JSONDecodeError, IndexError):
        d = {}
    if rc == 0:
        return True, f"weekly {d.get('percent')}% < {threshold}%"
    if rc == 1:
        resets = d.get("resets")
        return False, (f"weekly {d.get('percent')}% >= {threshold}%"
                       + (f", resets {resets}" if resets else ""))
    return False, f"usage unreadable ({err.strip()[:120] or 'exit 2'}) — holding"


def gate_disk(min_free_gb):
    rc, out, _ = sh(["df", "-Pk", str(REPO)])
    if rc != 0:
        return False, "df failed"
    try:
        free_gb = int(out.splitlines()[1].split()[3]) / (1024 * 1024)
    except (IndexError, ValueError):
        return False, "could not parse df"
    ok = free_gb >= min_free_gb
    return ok, f"{free_gb:.0f} GB free (need {min_free_gb})"


# ------------------------------------------------------------- the ledger

def ledger_call(*args):
    rc, out, err = sh([sys.executable, str(HERE / "task_update.py"), *args])
    return rc, out, err


def ledger_read():
    try:
        return json.loads(LEDGER.read_text())
    except (json.JSONDecodeError, OSError):
        return None


def next_connector(run_id, queue):
    """First queue entry not already recorded success/failed FOR THIS RUN."""
    d = ledger_read() or {}
    done = set()
    if d.get("run_id") == run_id:
        for i in d.get("invocations") or []:
            if isinstance(i, dict) and i.get("status") in ("success", "failed", "skipped"):
                done.add((i.get("connector") or "").lower())
    for c in queue:
        if c.lower() not in done:
            return c
    return None


# --------------------------------------------------------------- the run

def build_prompt(connector, flows, hs_repo_path):
    """The documented single-connector form (grace/README.md:98-104).

    WORKFLOW_DIR is deliberately omitted: left empty, 2.0_preflight.md:204
    freezes its own workflow copy into the run dir, which is the drift
    protection the batch path needed WORKFLOW_DIR for.
    """
    lines = [
        "Read grace/workflow/2_connector.md and follow it exactly.",
        "",
        "Variables:",
        f"  CONNECTOR: {connector}",
        f"  FLOWS: {flows}",
        f"  HS_REPO_PATH: {hs_repo_path or ''}",
    ]
    return "\n".join(lines)


def tmux_session(connector):
    return f"grace-{connector.lower()}"


def tmux_alive(session):
    rc, _, _ = sh(["tmux", "has-session", "-t", session], timeout=30)
    return rc == 0


def launch(connector, prompt, log_path, extra_args):
    """One detached tmux session. `tmux attach -t grace-<connector>` to watch."""
    session = tmux_session(connector)
    sh(["tmux", "kill-session", "-t", session], timeout=30)  # no-op if absent
    inner = " ".join([
        "claude", "-p", _q(prompt),
        "--permission-mode", "bypassPermissions",
        *extra_args,
        "2>&1", "|", "tee", _q(str(log_path)),
    ])
    rc, _, err = sh(["tmux", "new-session", "-d", "-s", session,
                     "-c", str(REPO), inner], timeout=60)
    return (rc == 0), (err.strip()[:200] if rc else None)


def _q(s):
    return "'" + s.replace("'", "'\\''") + "'"


def find_run_dir(connector, since_epoch):
    """glob grace/runs/<clc>-* for a run.json newer than launch.

    RUN_ID cannot be pre-assigned — 2_connector.md:90 draws it from
    /dev/urandom, and passing RUN_ID in means *resume*, not "use this id".
    """
    clc = connector.lower()
    best, best_m = None, -1
    for d in (REPO / "grace" / "runs").glob(f"{clc}-*"):
        rj = d / "run.json"
        try:
            m = rj.stat().st_mtime
        except OSError:
            continue
        if m >= since_epoch - 5 and m > best_m:
            best, best_m = d, m
    return best


def read_json(path, retries=1):
    """Agent-written files are tmp+mv, but tolerate one bad read anyway."""
    for attempt in range(retries + 1):
        try:
            return json.loads(path.read_text())
        except FileNotFoundError:
            return None
        except (json.JSONDecodeError, OSError):
            if attempt == retries:
                return None
            time.sleep(2)
    return None


def observe(run_dir):
    """Snapshot of what the run has produced so far."""
    if run_dir is None:
        return {"stage": "starting"}
    rj = read_json(run_dir / "run.json") or {}
    pr = read_json(run_dir / "pr" / "pr.json") or {}
    res = read_json(run_dir / "pr" / "result.json") or {}
    try:
        ev_mtime = (run_dir / "events.log").stat().st_mtime
    except OSError:
        ev_mtime = None
    return {
        "stage": "running",
        "ended_at": rj.get("ended_at"),
        "status": rj.get("status"),
        "stopped": rj.get("stopped"),
        "mode": rj.get("mode"),
        "branch": rj.get("branch"),
        "running_rows": [r.get("id") for r in (rj.get("rows") or [])
                         if isinstance(r, dict) and r.get("status") == "running"],
        "pr_url": (pr.get("ucs") or {}).get("url") or res.get("prUrl"),
        "pr_number": (pr.get("ucs") or {}).get("number"),
        "pr_status": res.get("prStatus"),
        "summary": (run_dir / "summary.md").exists(),
        "events_mtime": ev_mtime,
    }


def supervise(connector, run_dir_hint, session, since, stall_hours, poll):
    """Block until the run ends. Returns the final observation."""
    run_dir, announced_pr = run_dir_hint, False
    while True:
        if run_dir is None:
            run_dir = find_run_dir(connector, since)
            if run_dir:
                log(f"  run dir: {run_dir.relative_to(REPO)}")
        obs = observe(run_dir)

        if obs.get("pr_url") and not announced_pr:
            announced_pr = True
            log(f"  PR RAISED: {obs['pr_url']}")

        # Terminal: summary.md is written strictly AFTER run.json.status, so its
        # presence means the status field is already final.
        if obs.get("summary") and obs.get("ended_at"):
            return run_dir, obs, "finished"

        if not tmux_alive(session):
            # Session gone. ended_at null means it died mid-stage rather than
            # exiting; that run is resumable later by its RUN_ID.
            time.sleep(5)
            obs = observe(run_dir)
            if obs.get("summary") and obs.get("ended_at"):
                return run_dir, obs, "finished"
            return run_dir, obs, "crashed"

        ev = obs.get("events_mtime")
        if ev and (time.time() - ev) > stall_hours * 3600:
            return run_dir, obs, "stalled"

        time.sleep(poll)


# ----------------------------------------------------------------- driver

class Supervisor:
    def __init__(self, cfg, dry_run=False, once=False):
        self.cfg = cfg
        self.dry_run = dry_run
        self.once = once
        self.cancelled = False
        signal.signal(signal.SIGTERM, self._stop)
        signal.signal(signal.SIGINT, self._stop)

    def _stop(self, *_):
        log("signal received — finishing the current wait, then stopping")
        self.cancelled = True

    def sleep(self, seconds):
        """Interruptible: wake on a signal instead of sitting out the whole nap."""
        end = time.time() + seconds
        while time.time() < end and not self.cancelled:
            time.sleep(min(5, end - time.time()))

    def run(self):
        cfg, run_id = self.cfg, self.cfg["run_id"]
        queue = cfg["connectors"]
        log(f"run_id={run_id}  queue={len(queue)} connector(s)  threshold={cfg['usage_threshold']}%")

        if not (REPO / "task.json").exists() or (ledger_read() or {}).get("run_id") != run_id:
            if self.dry_run:
                log("dry-run: would seed the ledger")
            else:
                ledger_call("--seed", run_id, ",".join(queue))

        while not self.cancelled:
            connector = next_connector(run_id, queue)
            if connector is None:
                log("queue drained — nothing left to run")
                return 0

            ok, detail = gate_tree_clean()
            if not ok:
                # Systemic: it will repeat for every connector. Stop, don't skip.
                log(f"STOP: {detail}")
                log("      resume the stopped run by its RUN_ID, or discard after "
                    "reading grace/runs/<run_id>/claimed.tsv")
                return 2

            ok, detail = gate_disk(cfg["min_free_gb"])
            if not ok:
                log(f"STOP: disk — {detail}")
                return 2
            log(f"disk: {detail}")

            ok, detail = gate_usage(cfg["usage_threshold"])
            if not ok:
                log(f"HOLD: {detail}")
                if self.once or self.dry_run:
                    return 1
                self.sleep(HOLD_SECONDS)
                continue
            log(f"usage: {detail}")

            prompt = build_prompt(connector, cfg["flows"], cfg.get("hs_repo_path"))
            session = tmux_session(connector)
            logs_dir = REPO / "grace" / "runs" / "_supervisor"
            log_path = logs_dir / f"{connector.lower()}-{int(time.time())}.log"

            if self.dry_run:
                log(f"dry-run: next connector is {connector}")
                log(f"  tmux new-session -d -s {session} -c {REPO} \\")
                log(f"    claude -p <prompt> --permission-mode bypassPermissions | tee {log_path}")
                log("  --- prompt ---")
                for line in prompt.splitlines():
                    log(f"  | {line}")
                return 0

            logs_dir.mkdir(parents=True, exist_ok=True)
            ledger_call(f"connector-agent-{connector.lower()}", "status=running")
            self.publish()
            since = time.time()
            started, err = launch(connector, prompt, log_path, cfg.get("claude_args", []))
            if not started:
                log(f"FAILED to launch tmux for {connector}: {err}")
                ledger_call(f"connector-agent-{connector.lower()}",
                            "status=failed", f"error=tmux launch failed: {err}")
                self.publish()
                continue

            log(f"launched {connector} in tmux '{session}' — attach with: tmux attach -t {session}")
            run_dir, obs, how = supervise(connector, None, session, since,
                                          cfg["stall_hours"], cfg["poll_seconds"])

            self.record(connector, obs, how, run_dir)
            self.publish()

            if self.once:
                return 0
        return 0

    def record(self, connector, obs, how, run_dir):
        inv = f"connector-agent-{connector.lower()}"
        pr_url = obs.get("pr_url")
        pr_status = obs.get("pr_status")
        rd = run_dir.name if run_dir else "none"

        # A FAILED run can still carry a PR: an INCOMPLETE run raises one
        # deliberately so the broken state is visible (1_orchestrator.md:84).
        # Record the PR whenever one exists, whatever the verdict.
        args = [inv]
        if how == "finished" and obs.get("status") == "SUCCESS":
            args += ["status=success"]
        else:
            reason = {"crashed": "session died mid-stage",
                      "stalled": "no events.log progress within the stall window"}.get(
                          how, obs.get("stopped") or obs.get("status") or "unknown")
            args += ["status=failed", f"error={how}: {reason} (run {rd})"]
        if pr_url:
            args += [f"pr={pr_url}"]
        ledger_call(*args)

        log(f"{connector}: {how} · run.status={obs.get('status')} · "
            f"pr_status={pr_status} · pr={pr_url or 'none'}")
        if how == "crashed" and run_dir:
            log(f"  resumable: re-launch with RUN_ID: {rd}")

    def publish(self):
        """Regenerate and push the dashboard's tracker.json, if configured."""
        dash = self.cfg.get("dashboard_repo")
        if not dash or self.dry_run:
            return
        dash = pathlib.Path(dash)
        gen = dash / "scripts" / "generators" / "tracker" / "build_tracker.py"
        if not gen.exists():
            log(f"  dashboard: generator missing at {gen}")
            return
        rc, _, err = sh([sys.executable, str(gen), "--ledger", str(LEDGER)],
                        cwd=dash, timeout=300)
        if rc != 0:
            log(f"  dashboard: generate failed — {err.strip()[:140]}")
            return
        sh(["git", "add", "--", "src/data/tracker.json"], cwd=dash)
        rc, _, _ = sh(["git", "diff", "--cached", "--quiet"], cwd=dash)
        if rc == 0:
            return  # nothing moved
        sh(["git", "commit", "-m", "chore(data): supervisor state"], cwd=dash)
        rc, _, err = sh(["git", "push"], cwd=dash, timeout=300)
        log("  dashboard: pushed" if rc == 0 else
            f"  dashboard: push failed — {err.strip()[:140]}")


DEFAULTS = {
    "usage_threshold": 50.0,
    "min_free_gb": 20,
    "stall_hours": STALL_HOURS,
    "poll_seconds": POLL_SECONDS,
    "flows": "",
    "hs_repo_path": "",
    "claude_args": [],
}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--config", type=pathlib.Path, required=True)
    ap.add_argument("--dry-run", action="store_true",
                    help="print the gates and the launch command, change nothing")
    ap.add_argument("--once", action="store_true", help="one connector, then exit")
    a = ap.parse_args()

    try:
        cfg = {**DEFAULTS, **json.loads(a.config.read_text())}
    except (json.JSONDecodeError, OSError) as e:
        print(f"bad config {a.config}: {e}", file=sys.stderr)
        return 2
    for key in ("run_id", "connectors"):
        if not cfg.get(key):
            print(f"config missing required key: {key}", file=sys.stderr)
            return 2

    if not a.dry_run and sys.platform != "linux":
        print("GRACE v2 is Linux-only (2.0_preflight.md:90 ABORT_NOT_LINUX). "
              f"This is {sys.platform}; every run would abort at S0. "
              "Use --dry-run here, and run the daemon on the Linux host.",
              file=sys.stderr)
        return 2

    return Supervisor(cfg, dry_run=a.dry_run, once=a.once).run()


if __name__ == "__main__":
    sys.exit(main())
