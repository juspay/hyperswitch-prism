#!/usr/bin/env python3
"""Atomic updater for the repo-root task.json subagent tracker.

Usage:
  task_update.py <invocation_id> field=value [field=value ...]
  task_update.py --validate [run_file ...]
Fields: status, started_at, finished_at, error, retries, pr, notes
Special values: status=running auto-stamps started_at (now) unless given.
                status in {success,failed,skipped} auto-stamps finished_at.
                retries=+1 increments.
--validate loads each run file (default: the repo-root task.json), reports
whether it parses as JSON and which expected keys are present. Archived run
files legitimately omit "invocations"; --validate says so instead of failing.
"""
import json, os, sys, tempfile, datetime

TASK = os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), "task.json")

# Keys this script reads or writes. Only "invocations" is required to update.
REQUIRED_KEYS = ("invocations",)
OPTIONAL_KEYS = ("run_id", "created_at", "updated_at", "flow", "connector", "branch", "summary")
INVOCATION_KEYS = ("id", "status", "started_at", "finished_at", "error", "retries")

def now():
    return datetime.datetime.now().astimezone().isoformat(timespec="seconds")

def load(path):
    """Return (data, error_string). Never raises on a missing or malformed file."""
    try:
        with open(path) as f:
            return json.load(f), None
    except FileNotFoundError:
        return None, "file not found"
    except json.JSONDecodeError as e:
        return None, f"invalid JSON: {e}"
    except OSError as e:
        return None, f"unreadable: {e}"

def validate(paths):
    rc = 0
    for path in paths:
        data, err = load(path)
        if err is not None:
            print(f"{path}: FAIL ({err})")
            rc = 1
            continue
        if not isinstance(data, dict):
            print(f"{path}: FAIL (top level is {type(data).__name__}, expected object)")
            rc = 1
            continue
        present = [k for k in REQUIRED_KEYS + OPTIONAL_KEYS if k in data]
        missing = [k for k in REQUIRED_KEYS + OPTIONAL_KEYS if k not in data]
        invs = data.get("invocations")
        if isinstance(invs, list):
            inv_note = f"invocations: {len(invs)} entry/entries"
            bad = [i for i, e in enumerate(invs) if not isinstance(e, dict) or "id" not in e]
            if bad:
                inv_note += f"; entries without an \"id\": {bad}"
            seen = sorted({k for e in invs if isinstance(e, dict) for k in e})
            if seen:
                inv_note += f"; entry keys seen: {', '.join(seen)}"
            missing_inv = [k for k in INVOCATION_KEYS if k not in seen]
            if missing_inv:
                inv_note += f"; entry keys absent: {', '.join(missing_inv)}"
        elif invs is None:
            inv_note = 'invocations: ABSENT (not updatable; archive-only run file)'
        else:
            inv_note = f"invocations: WRONG TYPE ({type(invs).__name__}, expected list)"
            rc = 1
        print(f"{path}: OK (parses)")
        print(f"  present: {', '.join(present) or '(none)'}")
        print(f"  absent : {', '.join(missing) or '(none)'}")
        print(f"  {inv_note}")
    return rc

def main():
    argv = sys.argv[1:]
    if argv and argv[0] == "--validate":
        sys.exit(validate(argv[1:] or [TASK]))
    if len(argv) < 2:
        print(__doc__); sys.exit(2)
    inv_id, kvs = argv[0], argv[1:]
    data, err = load(TASK)
    if err is not None:
        print(f"ERROR: {TASK}: {err}"); sys.exit(1)
    if not isinstance(data, dict):
        print(f"ERROR: {TASK}: top level is {type(data).__name__}, expected object"); sys.exit(1)
    invocations = data.setdefault("invocations", [])
    if not isinstance(invocations, list):
        print(f"ERROR: {TASK}: \"invocations\" is {type(invocations).__name__}, expected list"); sys.exit(1)
    inv = next((i for i in invocations if isinstance(i, dict) and i.get("id") == inv_id), None)
    if inv is None:
        ids = [i.get("id") for i in invocations if isinstance(i, dict)]
        print(f"ERROR: no invocation with id {inv_id}")
        print(f"  {TASK} has {len(invocations)} invocation(s): {ids or '(none)'}")
        sys.exit(1)
    for kv in kvs:
        k, _, v = kv.partition("=")
        if k == "retries" and v == "+1":
            inv["retries"] = int(inv.get("retries") or 0) + 1
        elif k == "retries":
            inv["retries"] = int(v)
        elif v == "null":
            inv[k] = None
        else:
            inv[k] = v
    if inv.get("status") == "running" and not inv.get("started_at"):
        inv["started_at"] = now()
    if inv.get("status") in ("success", "failed", "skipped") and not inv.get("finished_at"):
        inv["finished_at"] = now()
    data["updated_at"] = now()
    counts = {}
    for i in invocations:
        if not isinstance(i, dict):
            continue
        s = i.get("status") or "unknown"
        counts[s] = counts.get(s, 0) + 1
    data["summary"] = counts
    d = os.path.dirname(TASK)
    fd, tmp = tempfile.mkstemp(dir=d, prefix=".task.json.")
    try:
        with os.fdopen(fd, "w") as f:
            json.dump(data, f, indent=2)
            f.write("\n")
            f.flush(); os.fsync(f.fileno())
        os.replace(tmp, TASK)
    except BaseException:
        if os.path.exists(tmp): os.unlink(tmp)
        raise
    print(json.dumps(inv, indent=2))

main()
