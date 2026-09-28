#!/usr/bin/env python3
"""Read the Claude subscription's weekly usage and gate on it.

`claude -p "/usage"` prints, among other lines:

    Current session: 12% used · resets Sep 28 at 9:59pm (Asia/Calcutta)
    Current week (all models): 5% used · resets Oct 5 at 11:29am (Asia/Calcutta)

The weekly figure is the real plan limit, which is why this reads it rather than
estimating a token budget from transcripts: a guessed ceiling is wrong in a
direction nobody can see, and the number that actually stops you is this one.

  usage_gate.py [--threshold 50] [--scope week|session] [--json]

Exit 0  under the threshold  (start the next connector)
Exit 1  at or over it        (hold; re-check later)
Exit 2  could not be read    (hold too — never assume 0%)

Exit 2 is deliberately not a pass. An unreadable usage response treated as "0%
used" would remove the ceiling entirely, which is the one failure this gate
exists to prevent.
"""
import argparse, json, re, subprocess, sys

# "Current week (all models): 5% used · resets Oct 5 at 11:29am (Asia/Calcutta)"
# The separator is a Unicode middle dot and the reset clause is optional, so the
# percent is matched independently of everything that follows it.
WEEK = re.compile(r"Current week \(all models\):\s*([\d.]+)%\s*used", re.I)
SESSION = re.compile(r"Current session:\s*([\d.]+)%\s*used", re.I)
RESETS = {
    "week": re.compile(r"Current week \(all models\):[^\n]*?resets\s+([^(\n]+)", re.I),
    "session": re.compile(r"Current session:[^\n]*?resets\s+([^(\n]+)", re.I),
}


def read_usage(timeout=180):
    """(text, error). Never raises."""
    try:
        r = subprocess.run(
            ["claude", "-p", "/usage", "--output-format", "text"],
            capture_output=True, text=True, timeout=timeout,
        )
    except FileNotFoundError:
        return None, "claude CLI not found on PATH"
    except subprocess.TimeoutExpired:
        return None, f"claude -p /usage timed out after {timeout}s"
    if r.returncode != 0:
        tail = (r.stderr or "").strip().splitlines()
        return None, f"claude exited {r.returncode}: {tail[-1][:160] if tail else 'no stderr'}"
    return r.stdout, None


def parse(text, scope):
    pat = WEEK if scope == "week" else SESSION
    m = pat.search(text or "")
    if not m:
        return None, None
    reset = RESETS[scope].search(text)
    return float(m.group(1)), (reset.group(1).strip() if reset else None)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--threshold", type=float, default=50.0, help="hold at or above this %%")
    ap.add_argument("--scope", choices=("week", "session"), default="week")
    ap.add_argument("--json", action="store_true")
    a = ap.parse_args()

    text, err = read_usage()
    if err:
        print(f"usage unreadable: {err}", file=sys.stderr)
        if a.json:
            print(json.dumps({"ok": False, "error": err, "over": True}))
        return 2

    pct, reset = parse(text, a.scope)
    if pct is None:
        # The wording changed, or the account is not on a subscription. Either
        # way we do not know the number, so we must not let the queue proceed.
        print(f"usage unreadable: no '{a.scope}' percentage in /usage output", file=sys.stderr)
        if a.json:
            print(json.dumps({"ok": False, "error": "unparseable", "over": True,
                              "raw": (text or "")[:400]}))
        return 2

    over = pct >= a.threshold
    if a.json:
        print(json.dumps({"ok": True, "scope": a.scope, "percent": pct,
                          "threshold": a.threshold, "resets": reset, "over": over}))
    else:
        print(f"{a.scope}: {pct:g}% used (threshold {a.threshold:g}%)"
              + (f" · resets {reset}" if reset else ""))
        print("OVER — hold, do not start another connector" if over else "under — clear to start")
    return 1 if over else 0


if __name__ == "__main__":
    sys.exit(main())
