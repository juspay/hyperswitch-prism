#!/usr/bin/env python3
"""Report what a GRACE run spent in tokens, per stage and per spawn.

A run's cost is `calls x context`, and context only grows: every call re-sends everything the agent
has accumulated. One measured run came to 734M tokens, of which 96.6% was cache read — context
re-sent — against 21M of material that was ever new. Nothing in a run surfaces that today, so a
regression is invisible. This prints it.

**Advisory only.** It is not a gate. It never fails a stage, never changes a status, and when the
transcripts are not where it looked it says so and exits 0. A run that cannot produce this report is
a run with no report, not a failed run.

WHAT IT READS
  The harness's subagent transcripts: one JSONL file per spawn, each line a message, assistant
  messages carrying a `usage` block. **That location is not a stable contract** — pass
  `--transcripts <dir>`, or let the default discovery try `$CLAUDE_TASK_OUTPUT_DIR` and the usual
  scratch path. If discovery fails the report says "not available" and stops.

HOW IT COUNTS
  Tokens per call = `input + cache_read + cache_creation`; output is counted separately.
  **Usage is deduplicated by message id.** A streamed message repeats its id across lines, and
  counting those twice inflates every total by roughly 2x — the one mistake that makes this report
  worse than no report.

WHAT THE NUMBERS MEAN
  `floor`       = the agent's first-call context, re-sent on every later call. Shrinking it (smaller
                  stage files, smaller inputs) pays on every call of every agent.
  `accumulated` = everything added while working, then re-sent. This is where long agents get
                  expensive: 80% of context in the measured run, and it rises with agent length.
  `lookups`     = calls whose tool was a read/grep/jq-style lookup. Each costs the whole pile, so a
                  high count with small outputs is the signature of exploration, not of big files.

Usage:
  run_token_report.py --run-dir grace/runs/<id>/ [--transcripts DIR] [--stdout]
Writes <run-dir>report/tokens.json and tokens.md.
"""

import argparse
import glob
import json
import os
import re
import sys
from collections import Counter, defaultdict

LOOKUP_TOOLS = {"Read", "Grep", "Glob", "NotebookRead"}
LOOKUP_BASH = re.compile(r"\b(cat|sed -n|head|tail|jq|grep|rg|ls|find)\b")


def discover_transcripts():
    """Best-effort: the harness does not promise a location, so try, then give up cleanly."""
    env = os.environ.get("CLAUDE_TASK_OUTPUT_DIR")
    if env and os.path.isdir(env):
        return env
    hits = sorted(glob.glob("/tmp/claude-*/*/*/tasks"), key=os.path.getmtime, reverse=True)
    return hits[0] if hits else None


def stage_of(prompt):
    """Map a spawn's opening prompt to the workflow stage that produced it."""
    m = re.search(r"grace/workflow/([0-9a-z._]+)\.md", prompt or "")
    if not m:
        return "other"
    stage = m.group(1)
    u = re.search(r"UNIT:\s*(\S+)", prompt or "")
    if stage == "2.3b_codegen_unit" and u:
        unit = u.group(1)
        return stage + "/" + ("finalize" if unit == "__finalize__" else "hs" if unit == "__hs__" else "unit")
    return stage


def read_transcript(path):
    """-> dict for one spawn, or None when it holds no usage at all."""
    usage = {}          # message id -> (context tokens, output tokens)
    order = []          # message ids, first seen first
    first_prompt = None
    tools = Counter()
    lookups = 0
    pending = {}
    for line in open(path, errors="replace"):
        try:
            entry = json.loads(line)
        except ValueError:
            continue
        if not isinstance(entry, dict):
            continue          # a transcript may carry bare JSON scalars; they hold no usage
        kind = entry.get("type")
        if kind == "assistant":
            msg = entry.get("message") or {}
            mid = msg.get("id")
            use = msg.get("usage")
            if use and mid and mid not in usage:
                usage[mid] = (
                    (use.get("input_tokens") or 0)
                    + (use.get("cache_read_input_tokens") or 0)
                    + (use.get("cache_creation_input_tokens") or 0),
                    use.get("output_tokens") or 0,
                )
                order.append(mid)
            for block in msg.get("content") or []:
                if isinstance(block, dict) and block.get("type") == "tool_use":
                    name = block.get("name") or "?"
                    tools[name] += 1
                    pending[block.get("id")] = name
                    if name in LOOKUP_TOOLS:
                        lookups += 1
                    elif name == "Bash" and LOOKUP_BASH.search((block.get("input") or {}).get("command", "")):
                        lookups += 1
        elif kind == "user" and first_prompt is None:
            content = (entry.get("message") or {}).get("content")
            if isinstance(content, str):
                first_prompt = content
            elif isinstance(content, list):
                first_prompt = " ".join(b.get("text", "") for b in content if isinstance(b, dict))
    if not usage:
        return None
    ctx = [usage[m][0] for m in order]
    out = sum(usage[m][1] for m in order)
    calls = len(ctx)
    floor = ctx[0] * calls
    total_ctx = sum(ctx)
    return {
        "file": os.path.basename(path),
        "stage": stage_of(first_prompt),
        "calls": calls,
        "context_tokens": total_ctx,
        "output_tokens": out,
        "tokens": total_ctx + out,
        "first_context": ctx[0],
        "last_context": ctx[-1],
        "peak_context": max(ctx),
        "floor_tokens": floor,
        "accumulated_tokens": max(total_ctx - floor, 0),
        "lookup_calls": lookups,
        "tools": dict(tools.most_common(6)),
    }


def build(transcripts):
    spawns = []
    for path in sorted(glob.glob(os.path.join(transcripts, "*.output"))
                       + glob.glob(os.path.join(transcripts, "*.jsonl"))):
        try:
            row = read_transcript(path)
        except (OSError, ValueError, KeyError, TypeError, AttributeError):
            continue          # one unreadable transcript costs that spawn's row, never the report
        if row:
            spawns.append(row)
    if not spawns:
        return None
    by_stage = defaultdict(lambda: dict(spawns=0, calls=0, tokens=0, accumulated=0, lookups=0))
    for s in spawns:
        acc = by_stage[s["stage"]]
        acc["spawns"] += 1
        acc["calls"] += s["calls"]
        acc["tokens"] += s["tokens"]
        acc["accumulated"] += s["accumulated_tokens"]
        acc["lookups"] += s["lookup_calls"]
    total = sum(s["tokens"] for s in spawns)
    calls = sum(s["calls"] for s in spawns)
    accum = sum(s["accumulated_tokens"] for s in spawns)
    ctx = sum(s["context_tokens"] for s in spawns)
    return {
        "totals": {
            "spawns": len(spawns),
            "calls": calls,
            "tokens": total,
            "avg_context_per_call": round(ctx / calls) if calls else 0,
            "accumulated_share": round(100.0 * accum / ctx, 1) if ctx else 0.0,
            "lookup_calls": sum(s["lookup_calls"] for s in spawns),
        },
        "by_stage": {k: v for k, v in sorted(by_stage.items(), key=lambda kv: -kv[1]["tokens"])},
        "spawns": sorted(spawns, key=lambda s: -s["tokens"]),
    }


def render(report):
    t = report["totals"]
    out = ["# Run token report", "",
           f"**{t['tokens']:,} tokens** over {t['calls']:,} calls in {t['spawns']} spawns — "
           f"average context {t['avg_context_per_call']:,} per call.", "",
           f"{t['accumulated_share']}% of context was accumulated while working (the rest is each "
           f"agent's first-call context, re-sent every call). {t['lookup_calls']:,} calls were lookups; "
           f"every one of them costs the whole accumulated context, not just what it returned.", "",
           "| stage | spawns | calls | tokens | accumulated | lookups |", "|---|---|---|---|---|---|"]
    for stage, v in report["by_stage"].items():
        out.append(f"| {stage} | {v['spawns']} | {v['calls']} | {v['tokens']:,} | "
                   f"{v['accumulated']:,} | {v['lookups']} |")
    out += ["", "## Longest spawns", "",
            "| spawn | stage | calls | first ctx | last ctx | tokens |", "|---|---|---|---|---|---|"]
    for s in report["spawns"][:10]:
        out.append(f"| {s['file']} | {s['stage']} | {s['calls']} | {s['first_context']:,} | "
                   f"{s['last_context']:,} | {s['tokens']:,} |")
    out.append("")
    return "\n".join(out)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--run-dir", help="grace/runs/<id>/ — report goes to <run-dir>report/")
    ap.add_argument("--transcripts", help="directory of per-spawn transcripts (default: discovered)")
    ap.add_argument("--stdout", action="store_true", help="also print the markdown")
    args = ap.parse_args()

    transcripts = args.transcripts or discover_transcripts()
    if not transcripts or not os.path.isdir(transcripts):
        print("run_token_report: transcripts not available (looked in %r); no report written"
              % (transcripts or "$CLAUDE_TASK_OUTPUT_DIR, /tmp/claude-*/*/*/tasks"))
        return 0

    report = build(transcripts)
    if report is None:
        print("run_token_report: no transcripts with usage found in %s; no report written" % transcripts)
        return 0

    body = render(report)
    if args.run_dir:
        out_dir = os.path.join(args.run_dir, "report")
        try:
            os.makedirs(out_dir, exist_ok=True)
            for name, blob in (("tokens.json", json.dumps(report, indent=1)), ("tokens.md", body)):
                tmp = os.path.join(out_dir, name + ".tmp")
                with open(tmp, "w", encoding="utf-8") as fh:
                    fh.write(blob)
                os.replace(tmp, os.path.join(out_dir, name))
            print("run_token_report: wrote %s/tokens.json and tokens.md" % out_dir)
        except OSError as exc:
            print("run_token_report: could not write the report (%s); numbers follow" % exc)
            args.stdout = True
    if args.stdout or not args.run_dir:
        print(body)
    return 0


if __name__ == "__main__":
    sys.exit(main())
