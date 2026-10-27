#!/usr/bin/env python3
"""The bug status machine: apply STATUS_UPDATES, or validate a recorded history.

`grace/workflow/2.6d_test_exec.md` Phase 1c enumerates the whole machine -- nine states, an
explicit transition list, an `update_id` dedupe and an `attempts` bump. The executing agent
is a validator there, not a decider: the `to:` value arrives in the file. This is that
specification, executed.

  --verify --run-dir R   replay every bug's recorded `history[]` and confirm each
                         transition was legal. No finished run contains a
                         `rejected_update`, so a rejection here means either this file's
                         rules are wrong or a run applied an illegal transition.
  --apply --bugs B --updates U   apply a STATUS_UPDATES file to a bugs file.

Exit codes mirror grace/rulesbook/codegen/tools/refusal_gate.py:
  0 pass, 1 an illegal transition was found or applied-with-rejections, 2 could not evaluate.

Stdlib only.
"""

import argparse
import json
import os
import sys

# 2.6d:437. Nine states.
STATES = ("open", "rca", "fixing", "retest", "fixed",
          "unresolved", "wont_fix", "invalid", "flaky")
NON_TERMINAL = ("open", "rca", "fixing", "retest")
# "any non-terminal -> unresolved|wont_fix" (2.6d:158)
ANY_NON_TERMINAL_TO = ("unresolved", "wont_fix")

# The explicit list, verbatim from 2.6d:156-160.
TRANSITIONS = {
    "open": ("rca",),
    "rca": ("fixing", "invalid", "flaky", "unresolved", "wont_fix"),
    "fixing": ("retest", "unresolved"),
    "retest": ("fixed", "open"),          # open = failed fix
    "fixed": ("open",),                   # reappearance
}


def legal(frm, to):
    """-> bool. 2.6d:156-160."""
    if to not in STATES:
        return False
    if to in TRANSITIONS.get(frm, ()):
        return True
    # The catch-all: any non-terminal may be abandoned.
    return frm in NON_TERMINAL and to in ANY_NON_TERMINAL_TO


def apply_updates(bugs, updates):
    """-> (applied, rejected, skipped). Mutates each bug's status/attempts/history.

    `bugs` is the list from test/bugs.json; `updates` the STATUS_UPDATES list.
    """
    by_id = {b.get("bug_id"): b for b in bugs}
    applied, rejected, skipped = [], [], []
    for u in updates or []:
        bug = by_id.get(u.get("bug_id"))
        if bug is None:
            rejected.append({"update": u, "why": "no such bug_id"})
            continue
        hist = bug.setdefault("history", [])
        uid = u.get("update_id")
        # "Skip an update_id already in the bug's history" (2.6d:156).
        if uid and any(h.get("update_id") == uid for h in hist):
            skipped.append({"update_id": uid, "bug_id": bug.get("bug_id")})
            continue
        frm, to = bug.get("status"), u.get("to")
        if not legal(frm, to):
            # "A disallowed transition is not applied: history rejected_update" (2.6d:160).
            hist.append({"event": "rejected_update", "from": frm, "to": to,
                         "by": u.get("by"), "ref": u.get("ref"), "update_id": uid,
                         "note": "disallowed transition"})
            rejected.append({"bug_id": bug.get("bug_id"), "from": frm, "to": to,
                             "update_id": uid})
            continue
        bug["status"] = to
        # "Entering fixing increments attempts" (2.6d:159).
        if to == "fixing":
            bug["attempts"] = int(bug.get("attempts") or 0) + 1
        for k, v in (u.get("set") or {}).items():
            bug[k] = v
        hist.append({"event": "status", "from": frm, "to": to, "by": u.get("by"),
                     "ref": u.get("ref"), "update_id": uid, "note": u.get("reason")})
        applied.append({"bug_id": bug.get("bug_id"), "from": frm, "to": to})
    return applied, rejected, skipped


def dedupe_disposition(existing_status):
    """-> (new_status, history_event). 2.6d:146-148, the fingerprint-collision rule."""
    if existing_status == "fixed":
        return "open", "reappeared"
    if existing_status in ("invalid", "wont_fix", "flaky"):
        return existing_status, "seen_again"      # status kept
    return existing_status, "merged"


def retest_outcome(checks, round_outcomes, fingerprint, failing_fingerprints):
    """-> ('open'|'fixed'|'retest', reason). 2.6d:465-470.

    `checks` is the bug's check set; `round_outcomes` maps check_id -> outcome this round.
    """
    if fingerprint in (failing_fingerprints or ()):
        return "open", "fix did not hold"
    if not checks:
        return "retest", "empty check set"
    seen = [round_outcomes.get(c) for c in checks]
    if all(o == "PASS" for o in seen):
        return "fixed", "every check passed"
    return "retest", "a check did not run or did not pass"


# ---------------------------------------------------------------- verify a recorded run

def verify_history(bugs):
    """-> (checked, illegal[]). Replay each bug's history and test every transition."""
    checked, illegal = 0, []
    for b in bugs:
        for h in (b.get("history") or []):
            if h.get("event") not in ("status", "reappeared"):
                continue
            frm, to = h.get("from"), h.get("to")
            if frm is None or to is None:
                continue
            # `retest -> retest` is 2.6d:470's "stays `retest`, decision row" -- a
            # recorded no-change, not a move. Real runs record it, and testing it
            # against the transition list flagged three legitimate entries.
            if frm == to:
                continue
            checked += 1
            if not legal(frm, to):
                illegal.append({"bug_id": b.get("bug_id"), "from": frm, "to": to,
                                "by": h.get("by"), "update_id": h.get("update_id")})
    return checked, illegal


def main():
    ap = argparse.ArgumentParser(description="Bug status machine")
    ap.add_argument("--run-dir")
    ap.add_argument("--verify", action="store_true")
    ap.add_argument("--bugs")
    ap.add_argument("--updates")
    ap.add_argument("--apply", action="store_true")
    ap.add_argument("--out")
    args = ap.parse_args()

    report = {"pass": True, "checks": [], "unparsed": [], "needs_human": []}

    def emit(code):
        blob = json.dumps(report, indent=1)
        if args.out:
            os.makedirs(os.path.dirname(os.path.abspath(args.out)), exist_ok=True)
            open(args.out, "w", encoding="utf-8").write(blob + "\n")
        print(blob)
        return code

    def load(p):
        try:
            return json.load(open(p, encoding="utf-8"))
        except (OSError, ValueError) as e:
            report["unparsed"].append({"what": p, "why": str(e)})
            return None

    if args.verify:
        path = args.bugs or (os.path.join(args.run_dir or "", "test/bugs.json"))
        doc = load(path)
        if doc is None:
            report["pass"] = False
            return emit(2)
        bugs = doc.get("bugs") if isinstance(doc, dict) else doc
        checked, illegal = verify_history(bugs or [])
        report["checks"] = [{
            "id": "BUGS-01", "name": "recorded_transitions_are_legal",
            "pass": not illegal, "evidence": illegal, "inconclusive": [],
            "message": ("%d of %d recorded transitions are not in the allowed list"
                        % (len(illegal), checked)) if illegal else "ok"}]
        report["summary"] = {"bugs": len(bugs or []), "transitions": checked,
                             "illegal": len(illegal)}
        report["pass"] = not illegal and not report["unparsed"]
        return emit(0 if report["pass"] else 1)

    if args.apply:
        bd, ud = load(args.bugs), load(args.updates)
        if bd is None or ud is None:
            report["pass"] = False
            return emit(2)
        bugs = bd.get("bugs") if isinstance(bd, dict) else bd
        applied, rejected, skipped = apply_updates(bugs or [], ud)
        for r in rejected:
            report["needs_human"].append("rejected update: %r" % (r,))
        report["checks"] = [{
            "id": "BUGS-02", "name": "status_updates_applied",
            "pass": not rejected, "evidence": rejected, "inconclusive": skipped,
            "message": "%d applied, %d rejected, %d already seen"
                       % (len(applied), len(rejected), len(skipped))}]
        report["applied"] = applied
        report["summary"] = {"applied": len(applied), "rejected": len(rejected),
                             "skipped": len(skipped)}
        report["pass"] = not rejected and not report["unparsed"]
        if args.out:
            pass
        return emit(0 if report["pass"] else 1)

    report["unparsed"].append({"what": "mode", "why": "pass --verify or --apply"})
    report["pass"] = False
    return emit(2)


def _replay():
    """Self-check of the machine, including the rules a reader most easily drops."""
    # the explicit list
    assert legal("open", "rca")
    assert not legal("open", "fixing")              # must go through rca
    assert not legal("open", "fixed")
    for t in ("fixing", "invalid", "flaky", "unresolved", "wont_fix"):
        assert legal("rca", t), t
    assert legal("fixing", "retest") and legal("fixing", "unresolved")
    assert not legal("fixing", "fixed")             # only retest reaches fixed
    assert legal("retest", "fixed") and legal("retest", "open")
    assert legal("fixed", "open")                   # reappearance
    assert not legal("fixed", "rca")
    # the catch-all applies to non-terminals only
    for s in NON_TERMINAL:
        assert legal(s, "unresolved") and legal(s, "wont_fix"), s
    for s in ("invalid", "wont_fix", "flaky"):
        assert not legal(s, "unresolved"), s
    assert not legal("open", "nonsense")

    # apply: dedupe on update_id, attempts bump, rejection is recorded not applied
    bugs = [{"bug_id": "B1", "status": "open", "attempts": 0, "history": []}]
    a, r, sk = apply_updates(bugs, [{"update_id": "u1", "bug_id": "B1", "to": "rca"}])
    assert len(a) == 1 and bugs[0]["status"] == "rca" and not r
    a, r, sk = apply_updates(bugs, [{"update_id": "u1", "bug_id": "B1", "to": "fixing"}])
    assert sk and bugs[0]["status"] == "rca", (sk, bugs)      # same update_id skipped
    a, r, sk = apply_updates(bugs, [{"update_id": "u2", "bug_id": "B1", "to": "fixing"}])
    assert bugs[0]["status"] == "fixing" and bugs[0]["attempts"] == 1
    a, r, sk = apply_updates(bugs, [{"update_id": "u3", "bug_id": "B1", "to": "fixed"}])
    assert r and bugs[0]["status"] == "fixing", (r, bugs)     # illegal, not applied
    assert bugs[0]["history"][-1]["event"] == "rejected_update"

    # dedupe dispositions
    assert dedupe_disposition("fixed") == ("open", "reappeared")
    assert dedupe_disposition("invalid") == ("invalid", "seen_again")
    assert dedupe_disposition("wont_fix") == ("wont_fix", "seen_again")
    assert dedupe_disposition("open") == ("open", "merged")

    # retest: the empty set stays in retest rather than passing vacuously
    assert retest_outcome([], {}, "f", [])[0] == "retest"
    assert retest_outcome(["c1"], {"c1": "PASS"}, "f", [])[0] == "fixed"
    assert retest_outcome(["c1", "c2"], {"c1": "PASS"}, "f", [])[0] == "retest"
    assert retest_outcome(["c1"], {"c1": "PASS"}, "f", ["f"])[0] == "open"

    # a recorded history that is legal verifies; an illegal one is caught
    ok = [{"bug_id": "B1", "history": [
        {"event": "created", "from": None, "to": "open"},
        {"event": "status", "from": "open", "to": "rca"},
        {"event": "status", "from": "rca", "to": "wont_fix"}]}]
    assert verify_history(ok) == (2, [])
    bad = [{"bug_id": "B2", "history": [{"event": "status", "from": "open", "to": "fixed"}]}]
    c, ill = verify_history(bad)
    assert c == 1 and len(ill) == 1
    # a recorded "stays" is not a transition and must not be flagged
    stay = [{"bug_id": "B3", "history": [{"event": "status", "from": "retest",
                                          "to": "retest"}]}]
    assert verify_history(stay) == (0, [])

    print("replay OK: the transition list holds, fixed is reachable only through retest, "
          "the non-terminal catch-all does not leak to terminal states, a repeated "
          "update_id is skipped, entering fixing bumps attempts, and an illegal "
          "transition is recorded rather than applied")
    return 0


if __name__ == "__main__":
    sys.exit(_replay() if sys.argv[1:2] == ["--replay"] else main())
