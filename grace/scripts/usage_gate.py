#!/usr/bin/env python3
"""Stop a batch run before it eats the whole token budget.

GRACE bounds a run by wall-clock (MAX_RUN_HOURS), disk, and attempt caps —
nothing measures tokens. One measured connector run (Nuvei, PR #2332) was
~9h23m and ~528M context tokens, so a 20-connector batch is an order of
magnitude past any sane per-session budget. This is the missing gate.

Usage is read from the Claude Code transcripts under ~/.claude/projects: every
assistant turn carries message.usage. No network, no dependencies, stdlib only.

  usage_gate.py --budget 2e9 [--window-hours 5] [--json]

Exit 0  under the threshold (keep going)
Exit 1  at or over it (stop cleanly and let the ledger resume later)
Exit 2  usage could not be read — reported, never silently treated as 0

Cache reads are counted at full weight by default: they are what a budget
measures, and discounting them would make a batch look affordable right up to
the point it is not. --no-cache-reads counts only fresh input + output.
"""
import argparse, datetime, json, pathlib, sys

PROJECTS = pathlib.Path.home() / ".claude" / "projects"
FIELDS = ("input_tokens", "output_tokens", "cache_creation_input_tokens", "cache_read_input_tokens")


def tokens_used(root: pathlib.Path, since_epoch_ms: float, count_cache_reads: bool):
    """(total, files_read, turns) over transcripts touched inside the window.

    File mtime prefilters; per-turn timestamps do the real filtering, so a long
    session that started before the window only contributes its recent turns.
    """
    total = files = turns = 0
    if not root.is_dir():
        raise FileNotFoundError(root)
    cutoff_s = since_epoch_ms / 1000.0
    for f in root.rglob("*.jsonl"):
        try:
            if f.stat().st_mtime < cutoff_s:
                continue
        except OSError:
            continue
        files += 1
        try:
            handle = f.open(errors="ignore")
        except OSError:
            continue
        with handle:
            for line in handle:
                try:
                    rec = json.loads(line)
                except (json.JSONDecodeError, ValueError):
                    continue
                usage = (rec.get("message") or {}).get("usage")
                if not isinstance(usage, dict):
                    continue
                ts = rec.get("timestamp")
                if isinstance(ts, str):
                    try:
                        when = datetime.datetime.fromisoformat(ts.replace("Z", "+00:00"))
                        if when.timestamp() < cutoff_s:
                            continue
                    except ValueError:
                        pass  # unparseable stamp: count it rather than lose it
                turns += 1
                for k in FIELDS:
                    if k == "cache_read_input_tokens" and not count_cache_reads:
                        continue
                    v = usage.get(k)
                    if isinstance(v, int):
                        total += v
    return total, files, turns


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--budget", type=float, required=True, help="token ceiling for the window")
    ap.add_argument("--threshold", type=float, default=100.0, help="stop at this %% of budget")
    ap.add_argument("--window-hours", type=float, default=5.0)
    ap.add_argument("--root", type=pathlib.Path, default=PROJECTS)
    ap.add_argument("--no-cache-reads", action="store_true")
    ap.add_argument("--json", action="store_true")
    a = ap.parse_args()

    if a.budget <= 0:
        print("budget must be > 0", file=sys.stderr)
        return 2
    since = (datetime.datetime.now() - datetime.timedelta(hours=a.window_hours)).timestamp() * 1000
    try:
        used, files, turns = tokens_used(a.root, since, not a.no_cache_reads)
    except FileNotFoundError as e:
        # Never fall through to "0 tokens used" — that reads as "plenty of
        # budget" and would let a batch run unbounded on a broken path.
        print(f"usage unreadable: no transcript directory at {e}", file=sys.stderr)
        return 2

    pct = 100.0 * used / a.budget
    over = pct >= a.threshold
    if a.json:
        print(json.dumps({
            "used": used, "budget": int(a.budget), "percent": round(pct, 2),
            "threshold": a.threshold, "window_hours": a.window_hours,
            "files": files, "turns": turns, "over": over,
        }, indent=2))
    else:
        print(f"{used:,} / {int(a.budget):,} tokens = {pct:.1f}% of budget "
              f"(threshold {a.threshold:.0f}%, {a.window_hours}h window, "
              f"{turns:,} turns in {files} transcript(s))")
        print("OVER THRESHOLD — stop the batch" if over else "under threshold — continue")
    return 1 if over else 0


if __name__ == "__main__":
    sys.exit(main())
