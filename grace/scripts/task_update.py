#!/usr/bin/env python3
"""Task ledger updater for GRACE orchestration.

Atomic updates to task.json via temp file + os.replace.
Usage: python3 grace/scripts/task_update.py <id> [k=v]...
"""
import json
import os
import sys
import tempfile

TASK_FILE = "task.json"


def _utc_now():
    from datetime import datetime, timezone
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def load():
    if not os.path.exists(TASK_FILE):
        return {"run_id": None, "connector": None, "invocations": [], "summary": {}}
    with open(TASK_FILE) as f:
        return json.load(f)


def save(d):
    fd, tmp = tempfile.mkstemp(dir=".", suffix=".tmp")
    try:
        with os.fdopen(fd, "w") as f:
            json.dump(d, f, indent=2)
        os.replace(tmp, TASK_FILE)
    except Exception:
        try:
            os.unlink(tmp)
        except OSError:
            pass
        raise


def main():
    if len(sys.argv) < 2:
        print("usage: task_update.py <id> [k=v]...", file=sys.stderr)
        sys.exit(1)
    inv_id = sys.argv[1]
    kvs = sys.argv[2:]

    d = load()
    rows = d.get("invocations", [])
    row = next((r for r in rows if r.get("id") == inv_id), None)
    if row is None:
        print(f"no invocation with id {inv_id}", file=sys.stderr)
        sys.exit(1)

    for kv in kvs:
        if "=" not in kv:
            continue
        k, v = kv.split("=", 1)
        row[k] = v

    # mark started/finished timestamps
    if row.get("status") == "running" and not row.get("started_at"):
        row["started_at"] = _utc_now()
    if row.get("status") in ("success", "failed", "skipped"):
        row["finished_at"] = _utc_now()

    # recompute summary
    counts = {"total": 0, "queued": 0, "running": 0, "success": 0, "failed": 0, "skipped": 0}
    for r in rows:
        counts["total"] += 1
        counts[r.get("status", "queued")] = counts.get(r.get("status", "queued"), 0) + 1
    d["summary"] = counts
    save(d)
    print(json.dumps(row, indent=2))


if __name__ == "__main__":
    main()
