#!/usr/bin/env python3
"""RCA clustering, live-verdict mapping and confidence -- the arithmetic parts of 2.6e.

`grace/workflow/2.6e_rca.md` reads as judgement ("group failures into root causes") and is
not: Phase 3 is a group-by on `cluster_key = <origin>:<ref>` with file:line interval
overlap, Phase 0a is a count over three replays, and `confidence` is a count of three
predicates. All three are written out; this file executes them.

  --verify --run-dir R   replay every `rca/r<N>.json` and check the two invariants the
                         artefacts can be held to: a `cluster_key` agrees with its
                         entry's own `origin`, and `proposed_status` follows the
                         verdict table.

Exit codes mirror grace/rulesbook/codegen/tools/refusal_gate.py:
  0 pass, 1 an invariant is violated, 2 could not evaluate.

Stdlib only.
"""

import argparse
import glob
import json
import os
import re
import sys

# 2.6e:86-92. The verdict is a count over three replays, and it fixes the status.
VERDICT_STATUS = {"confirmed": None,          # continue to Phase 1; a brief is written
                  "flaky": "flaky",           # no brief
                  "invalid": "invalid"}       # no brief
# 2.6e:150-166. Origins, test side then product side.
ORIGINS = ("ENV", "HS_CONFIG", "SCENARIO_DATA", "HARNESS", "HS",
           "LINKS", "TECHSPEC", "PLANNER", "CODEGEN")

_RANGE = re.compile(r"^(?P<file>.+?):(?P<a>\d+)\s*-\s*(?P<b>\d+)$")


def live_verdict(fail_count, runs=3):
    """-> (verdict, proposed_status). 2.6e:86-92: 3/3, 1-2/3, 0/3."""
    if fail_count >= runs:
        return "confirmed", None
    if fail_count == 0:
        return "invalid", "invalid"
    return "flaky", "flaky"


def confidence(confirmed_3of3, truth_quoted, fix_pinned):
    """-> high|medium|low. 2.6e:245-246 -- a count of three predicates, not a feeling."""
    n = sum(bool(x) for x in (confirmed_3of3, truth_quoted, fix_pinned))
    return "high" if n == 3 else ("medium" if n == 2 else "low")


def parse_ref(ref):
    """-> (file, start, end) with start/end None when the ref names no line range."""
    m = _RANGE.match(str(ref or "").strip())
    if m:
        return m.group("file"), int(m.group("a")), int(m.group("b"))
    return str(ref or "").strip(), None, None


def same_root(ref_a, ref_b):
    """-> bool. Same artifact ref: equal, or overlapping file:line ranges (2.6e:183)."""
    fa, a1, a2 = parse_ref(ref_a)
    fb, b1, b2 = parse_ref(ref_b)
    if fa != fb:
        return False
    if None in (a1, a2, b1, b2):
        return True                 # same file, no range on one side -> same root
    return a1 <= b2 and b1 <= a2    # closed-interval overlap


def cluster(items):
    """-> [{cluster_key, origin, ref, bug_ids[]}]. Phase 3's group-by.

    `items` are {bug_id, origin, ref}. Bugs share a cluster when the origin matches and
    the refs name the same root. Transitive: A overlapping B and B overlapping C puts all
    three together, which is what "share one root cause" means.
    """
    out = []
    for it in items:
        for c in out:
            if c["origin"] == it.get("origin") and any(
                    same_root(it.get("ref"), r) for r in c["refs"]):
                c["bug_ids"].append(it.get("bug_id"))
                c["refs"].append(it.get("ref"))
                break
        else:
            out.append({"origin": it.get("origin"), "ref": it.get("ref"),
                        "refs": [it.get("ref")], "bug_ids": [it.get("bug_id")]})
    for n, c in enumerate(out, 1):
        c["cluster_key"] = "%s:%s" % (c["origin"], c["ref"])
        c["brief_id"] = "b-%02d" % n
        c.pop("refs")
    return out


# ---------------------------------------------------------------- verify a recorded run

def verify_round(entries):
    """-> (checked, bad[], note[]). The two invariants, plus recorded spec gaps."""
    checked, bad, note = 0, [], []
    for e in entries:
        ck, origin = e.get("cluster_key"), e.get("origin")
        if ck:
            checked += 1
            head = str(ck).split(":", 1)[0]
            if origin and head != origin:
                bad.append({"what": "cluster_key origin", "cluster_key": ck,
                            "origin": origin,
                            "why": "cluster_key is <origin>:<ref>, so its head must be "
                                   "the entry's own origin"})
            elif head not in ORIGINS:
                bad.append({"what": "cluster_key origin", "cluster_key": ck,
                            "why": "%r is not one of the nine origins 2.6e defines" % head})
        vl, ps = e.get("verified_live") or {}, e.get("proposed_status") or {}
        for bug, verdict in (vl.items() if isinstance(vl, dict) else []):
            checked += 1
            want = VERDICT_STATUS.get(verdict, "__unknown__")
            got = ps.get(bug) if isinstance(ps, dict) else None
            if want == "__unknown__":
                note.append({"what": "verified_live", "bug": bug, "verdict": verdict,
                             "why": "not one of confirmed|flaky|invalid"})
            elif want is None:
                # confirmed: a brief is written, so any fixing-side status is consistent.
                if got is not None and got in ("flaky", "invalid"):
                    bad.append({"what": "proposed_status", "bug": bug,
                                "verdict": verdict, "status": got,
                                "why": "a confirmed bug must not propose flaky/invalid"})
            elif got == "fixed" and verdict == "invalid":
                # The status machine reaches `fixed` only via fixing -> retest -> fixed,
                # so a pre-existing bug that this run's code repaired has nowhere to go:
                # runs express it as verdict `invalid` (no longer reproduces) plus
                # proposed `fixed`. Measured at 30 occurrences in one real run. Not a
                # violation of the table -- a gap in it. Reported, never silently passed.
                note.append({"what": "proposed_status", "bug": bug, "verdict": verdict,
                             "status": got,
                             "why": "no terminal state for 'pre-existing defect this run "
                                    "repaired'; the run routed it as invalid+fixed"})
            elif got is not None and got != want:
                bad.append({"what": "proposed_status", "bug": bug, "verdict": verdict,
                            "status": got, "expected": want,
                            "why": "the verdict table fixes this status"})
    return checked, bad, note


def main():
    ap = argparse.ArgumentParser(description="RCA clustering / verdict invariants")
    ap.add_argument("--run-dir", required=True)
    ap.add_argument("--verify", action="store_true")
    ap.add_argument("--out")
    args = ap.parse_args()

    report = {"run_dir": args.run_dir, "pass": True, "checks": [],
              "unparsed": [], "needs_human": []}

    def emit(code):
        blob = json.dumps(report, indent=1)
        if args.out:
            os.makedirs(os.path.dirname(os.path.abspath(args.out)), exist_ok=True)
            open(args.out, "w", encoding="utf-8").write(blob + "\n")
        print(blob)
        return code

    files = sorted(glob.glob(os.path.join(args.run_dir, "rca", "r*.json")))
    if not files:
        report["unparsed"].append({"what": args.run_dir, "why": "no rca/r*.json"})
        report["pass"] = False
        return emit(2)

    checked, bad, notes, rounds = 0, [], [], []
    for f in files:
        try:
            doc = json.load(open(f, encoding="utf-8"))
        except (OSError, ValueError) as e:
            report["unparsed"].append({"what": f, "why": str(e)})
            continue
        entries = doc if isinstance(doc, list) else (doc.get("clusters") or [])
        c, b, nt = verify_round(entries)
        for x in b + nt:
            x["file"] = os.path.basename(f)
        checked += c
        bad += b
        notes += nt
        rounds.append({"file": os.path.basename(f), "entries": len(entries),
                       "bad": len(b), "notes": len(nt)})

    report["rounds"] = rounds
    report["checks"] = [{
        "id": "RCA-01", "name": "cluster_and_verdict_invariants",
        "pass": not bad, "evidence": bad, "inconclusive": notes,
        "message": ("%d of %d invariants violated" % (len(bad), checked))
                   if bad else "ok"}]
    for n in notes:
        report["needs_human"].append("%s %s: %s" % (n.get("file"), n.get("bug"), n["why"]))
    report["summary"] = {"rounds": len(rounds), "invariants_checked": checked,
                         "violations": len(bad), "spec_gaps": len(notes)}
    report["pass"] = not bad and not report["unparsed"]
    return emit(0 if report["pass"] else 1)


def _replay():
    """Self-check of the three rules."""
    # the verdict counts
    assert live_verdict(3) == ("confirmed", None)
    assert live_verdict(2) == ("flaky", "flaky")
    assert live_verdict(1) == ("flaky", "flaky")
    assert live_verdict(0) == ("invalid", "invalid")

    # confidence is a count, not a feeling
    assert confidence(True, True, True) == "high"
    assert confidence(True, True, False) == "medium"
    assert confidence(True, False, False) == "low"
    assert confidence(False, False, False) == "low"

    # ref parsing and interval overlap
    assert parse_ref("a/b.rs:410-431") == ("a/b.rs", 410, 431)
    assert parse_ref("plan/plan.md#P-01") == ("plan/plan.md#P-01", None, None)
    assert same_root("a.rs:10-20", "a.rs:20-30")        # touching counts as overlapping
    assert same_root("a.rs:10-20", "a.rs:15-16")
    assert not same_root("a.rs:10-20", "a.rs:21-30")
    assert not same_root("a.rs:10-20", "b.rs:10-20")
    assert same_root("plan/plan.md#P-01", "plan/plan.md#P-01")

    # clustering groups by origin AND root, transitively
    items = [{"bug_id": "B1", "origin": "CODEGEN", "ref": "a.rs:10-20"},
             {"bug_id": "B2", "origin": "CODEGEN", "ref": "a.rs:18-25"},
             {"bug_id": "B3", "origin": "CODEGEN", "ref": "a.rs:40-50"},
             {"bug_id": "B4", "origin": "PLANNER", "ref": "a.rs:10-20"}]
    cs = cluster(items)
    assert len(cs) == 3, cs
    b12 = [c for c in cs if set(c["bug_ids"]) == {"B1", "B2"}]
    assert b12 and b12[0]["cluster_key"] == "CODEGEN:a.rs:10-20", cs
    # same ref, different origin -> different cluster
    assert any(c["origin"] == "PLANNER" and c["bug_ids"] == ["B4"] for c in cs)
    # transitive: 10-20, 18-25, 24-30 are one root
    cs2 = cluster([{"bug_id": "B%d" % i, "origin": "CODEGEN", "ref": r}
                   for i, r in enumerate(["a.rs:10-20", "a.rs:18-25", "a.rs:24-30"])])
    assert len(cs2) == 1 and len(cs2[0]["bug_ids"]) == 3, cs2

    # the verify invariants
    ok = [{"origin": "PLANNER", "cluster_key": "PLANNER:plan/plan.md#P-01",
           "verified_live": {"B1": "confirmed"}, "proposed_status": {"B1": "fixing"}},
          {"origin": "CODEGEN", "cluster_key": "CODEGEN:a.rs:1-9",
           "verified_live": {"B2": "invalid"}, "proposed_status": {"B2": "invalid"}}]
    c, bad, nt = verify_round(ok)
    assert not bad, bad
    # a cluster_key whose head disagrees with the entry's origin
    c, bad, nt = verify_round([{"origin": "CODEGEN", "cluster_key": "PLANNER:x"}])
    assert len(bad) == 1 and bad[0]["what"] == "cluster_key origin"
    # a verdict the table maps elsewhere
    c, bad, nt = verify_round([{"verified_live": {"B1": "invalid"},
                                "proposed_status": {"B1": "fixing"}}])
    assert len(bad) == 1 and bad[0]["expected"] == "invalid", bad
    # invalid+fixed is the recorded missing-state workaround: a note, not a violation
    c, bad, nt = verify_round([{"verified_live": {"B1": "invalid"},
                                "proposed_status": {"B1": "fixed"}}])
    assert not bad and len(nt) == 1 and "pre-existing" in nt[0]["why"], (bad, nt)
    # a confirmed bug must not be proposed invalid
    c, bad, nt = verify_round([{"verified_live": {"B1": "confirmed"},
                                "proposed_status": {"B1": "invalid"}}])
    assert len(bad) == 1, bad

    print("replay OK: verdict counts fix the status, confidence is a count of three, "
          "interval overlap is transitive and origin-scoped, and a cluster_key must "
          "agree with its own origin")
    return 0


if __name__ == "__main__":
    sys.exit(_replay() if sys.argv[1:2] == ["--replay"] else main())
