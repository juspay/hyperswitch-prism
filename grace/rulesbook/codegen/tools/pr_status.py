#!/usr/bin/env python3
"""Derive FLOW_STATUS per unit, PR_STATUS, and specComplianceScore.

All three rules are written out in `grace/workflow/2.8_pr_run.md` as first-match-wins
tables plus one arithmetic formula. This file is that specification, executed; every branch
cites the rule it implements and adds no behaviour of its own.

Why a script: the score is a mean with a clamp and the two status tables are ordered
predicates over structured JSON. A model is strictly worse at all three -- and 2.6d writes
`final.json` while this stage grades from it, so prose executed twice by two agents is two
chances to disagree about one contract.

`--verify` replays a finished run and diffs what this derives against what the run
recorded in `pr/status.json` and `pr/result.json`.

Exit codes mirror grace/rulesbook/codegen/tools/refusal_gate.py:
  0 pass, 1 fail (a mismatch, in --verify), 2 could not evaluate.

Stdlib only.
"""

import argparse
import csv
import json
import os
import sys

NOT_PASSED_GRPC = ("BLOCKED", "FAIL", "NOT_RUN")
# The FLOW_STATUS values 2.8:114-123 defines. A record carrying anything else is not
# graded against: real runs have invented values (an `alpha: true` mock run used
# DELIVERED_MOCK_ONLY, which appears nowhere in grace/ or .skills/), and silently
# accepting or rejecting one would hide that.
SPEC_FLOW_STATUS = ("WITHDRAWN", "no_op", "UNRESOLVED", "DELIVERED_VERIFIED",
                    "DELIVERED_E2E_BLOCKED", "DELIVERED_E2E_SKIPPED")
CLOSED_BUG = ("fixed", "invalid", "wont_fix", "flaky")
BLOCKING_FLAGS = ("TEST_ENV_FAILED", "SECRET_LEAK", "TREE_MODIFIED")


def flow_status(order_status, impl_type, grpc, e2e, has_blocking_bug):
    """-> (FLOW_STATUS | 'no_op', reason). 2.8:114-123, first match wins."""
    if order_status in ("withdrawn", "blocked", "spec_gap") or impl_type == "withdraw":
        return "WITHDRAWN", order_status or impl_type
    if order_status == "no_op":
        return "no_op", "no_op"
    # UNRESOLVED before any DELIVERED_*: a blocking bug or a failed surface outranks
    # whatever else the unit achieved.
    if has_blocking_bug or e2e == "FAILED" or grpc in ("FAIL", "NOT_RUN"):
        return "UNRESOLVED", "blocking bug" if has_blocking_bug else "e2e/grpc"
    if e2e == "SUCCESS":
        return "DELIVERED_VERIFIED", "e2e SUCCESS"
    if e2e == "E2E_BLOCKED" or (grpc == "BLOCKED" and e2e != "SUCCESS"):
        return "DELIVERED_E2E_BLOCKED", "e2e blocked"
    if e2e == "E2E_SKIPPED":
        return "DELIVERED_E2E_SKIPPED", "e2e skipped"
    return "UNRESOLVED", "no delivered surface"


def pr_status(flows, blocking_bugs, review_open, flags, gate_fails,
              hs_pr, hs_changes_open, manifest_empty=False, drift_conflict=False):
    """-> (PR_STATUS, reasons[], undecided[]). 2.8:125-132, first match wins.

    `undecided` holds the inputs this file deliberately refuses to rule on.
    """
    fs = [f for f in flows if f != "no_op"]
    delivered = [f for f in fs if f.startswith("DELIVERED_")]
    reasons, undecided = [], []

    # FAILED: no DELIVERED_* unit, or an empty manifest.
    if not delivered or manifest_empty:
        return "FAILED", ["no delivered unit" if not delivered else "empty manifest"], undecided

    # INCOMPLETE: any one of these.
    if "UNRESOLVED" in fs:
        reasons.append("UNRESOLVED unit")
    if blocking_bugs:
        reasons.append("blocking_bugs=%d" % len(blocking_bugs))
    if review_open:
        reasons.append("open S0/S1 review finding(s): %s" % ",".join(map(str, review_open)))
    for fl in flags or ():
        if fl in BLOCKING_FLAGS:
            reasons.append(fl)
    # NOT decided here: 2.8:130 says "a mandatory gate exit != 0", and whether a given
    # failure is mandatory and non-mechanical is deferred to 2.4_pr.md "1b-2". Two real
    # runs took opposite views of the same `make validate-pre-push-fix` exit 2 (one READY,
    # one INCOMPLETE), so this is a live judgement, not a rule. Surfaced, never guessed.
    if gate_fails:
        undecided.append("gate exit != 0, mechanical-or-not is 2.4_pr.md's call: %s"
                         % "; ".join(g[:70] for g in gate_fails))
    if str(hs_pr or "").startswith("NOT_RAISED:") and hs_changes_open:
        reasons.append("HS_PR %s with %d open hs_changes" % (hs_pr, hs_changes_open))
    if drift_conflict:
        reasons.append("base drift conflict")
    if reasons:
        return "INCOMPLETE", reasons, undecided

    # PARTIAL: >=1 WITHDRAWN, the rest delivered or no_op, no blocking bugs.
    if "WITHDRAWN" in fs:
        return "PARTIAL", ["%d withdrawn" % fs.count("WITHDRAWN")], undecided
    # READY: all non-no_op units delivered, gates 0, no blocking bugs.
    return "READY", [], undecided


def spec_compliance_score(units, status):
    """-> float, 2 dp. 2.8:143-146.

    `units` are dicts {flow_status, grpc, e2e} for non-`no_op` units only.
    """
    if status == "FAILED":
        return 0.0
    vals = []
    for u in units:
        fs, grpc, e2e = u.get("flow_status"), u.get("grpc"), u.get("e2e")
        # "except 0 for a unit that passed nothing anywhere" -- this clause comes first so
        # an unverified flow can never sit the PR on the 0.6 gate.
        if e2e != "SUCCESS" and grpc in NOT_PASSED_GRPC:
            vals.append(0.0)
        elif fs == "DELIVERED_VERIFIED":
            vals.append(1.0)
        elif fs == "DELIVERED_E2E_BLOCKED":
            vals.append(0.8)
        elif fs == "DELIVERED_E2E_SKIPPED":
            vals.append(0.6)
        else:
            vals.append(0.0)
    score = (sum(vals) / len(vals)) if vals else 0.0
    if status == "INCOMPLETE":
        score = min(score, 0.5)
    return round(score, 2)


# ---------------------------------------------------------------- run-dir plumbing

def _load(path, default=None):
    try:
        return json.load(open(path, encoding="utf-8"))
    except (OSError, ValueError):
        return default


def _read(path, default=""):
    try:
        return open(path, encoding="utf-8").read().strip()
    except OSError:
        return default


def gate_failures(run_dir):
    """Non-zero exits in pr/gates.tsv (command, exit, duration, sha)."""
    out = []
    try:
        with open(os.path.join(run_dir, "pr/gates.tsv"), encoding="utf-8") as fh:
            for row in csv.reader(fh, delimiter="\t"):
                if len(row) >= 2 and str(row[1]).strip() not in ("0", "", "exit"):
                    out.append(row[0])
    except OSError:
        pass
    return out


def open_review_findings(run_dir, bugs):
    """Open S0/S1 findings whose id is not the review_ref of a closed bug (2.8:128)."""
    f = _load(os.path.join(run_dir, "review/findings.json"))
    if f is None:
        return ["findings.json absent"]
    items = f if isinstance(f, list) else (f.get("findings") or [])
    closed_refs = {b.get("review_ref") for b in bugs
                   if str(b.get("status")) in CLOSED_BUG and b.get("review_ref")}
    return [i.get("id") for i in items
            if str(i.get("sev")) in ("S0", "S1")
            and str(i.get("status") or "open") == "open"
            and i.get("id") not in closed_refs]


def derive_run(run_dir):
    final = _load(os.path.join(run_dir, "test/final.json"))
    plan = _load(os.path.join(run_dir, "plan/plan.json"))
    if final is None or plan is None:
        return None, "test/final.json or plan/plan.json missing"
    run = _load(os.path.join(run_dir, "run.json"), {}) or {}
    bugs_doc = _load(os.path.join(run_dir, "test/bugs.json"), {}) or {}
    bugs = bugs_doc.get("bugs") if isinstance(bugs_doc, dict) else (bugs_doc or [])
    bb = _load(os.path.join(run_dir, "pr/blocking_bugs.json"), []) or []
    if isinstance(bb, dict):
        bb = bb.get("bugs") or []

    order = {o.get("unit"): o.get("status") for o in (plan.get("order") or [])}
    markers = {u.get("unit"): (u.get("markers") or []) for u in (plan.get("units") or [])}

    fu = final.get("units") or {}
    fitems = fu.items() if isinstance(fu, dict) else [(u.get("unit"), u) for u in fu]

    # A unit's blocking bugs = entries whose flows[] intersect its markers (2.8:112).
    def unit_blocked(unit, rec):
        if rec.get("blocking_open_bugs"):
            return True
        mk = set(markers.get(unit) or [unit])
        for b in bb:
            fl = set(b.get("flows") or [])
            if not fl or (fl & mk):
                return True
        return False

    rows, score_units, flows = [], [], []
    for unit, rec in fitems:
        rec = rec or {}
        impl = None
        g, e = rec.get("grpc_status"), rec.get("e2e_status")
        fs, why = flow_status(order.get(unit), impl, g, e, unit_blocked(unit, rec))
        flows.append(fs)
        if fs != "no_op":
            score_units.append({"flow_status": fs, "grpc": g, "e2e": e})
        rows.append({"unit": unit, "derived": fs, "grpc": g, "e2e": e, "reason": why})

    st, reasons, undecided = pr_status(
        flows, bb, open_review_findings(run_dir, bugs), run.get("flags") or [],
        gate_failures(run_dir), _read(os.path.join(run_dir, "pr/hs_pr.txt")),
        len([h for h in (plan.get("hs_changes") or []) if not h.get("withdrawn")]))
    return {"rows": rows, "pr_status": st, "reasons": reasons, "undecided": undecided,
            "score": spec_compliance_score(score_units, st)}, None


def main():
    ap = argparse.ArgumentParser(description="Derive FLOW_STATUS / PR_STATUS / score")
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

    got, err = derive_run(args.run_dir)
    if got is None:
        report["unparsed"].append({"what": args.run_dir, "why": err})
        report["pass"] = False
        return emit(2)

    rec_status = _load(os.path.join(args.run_dir, "pr/status.json"), {}) or {}
    rec_result = _load(os.path.join(args.run_dir, "pr/result.json"), {}) or {}
    rec_flow = {u.get("unit"): u.get("flow_status") for u in (rec_status.get("units") or [])}

    mism, incon = [], []
    for r in got["rows"]:
        want = rec_flow.get(r["unit"])
        r["recorded"] = want
        if want is None or r["derived"] == want:
            continue
        if want not in SPEC_FLOW_STATUS:
            r["why"] = "recorded %r is not a FLOW_STATUS 2.8 defines" % want
            incon.append(r)
            report["needs_human"].append(
                "%s: run recorded FLOW_STATUS %r, which the workflow does not define"
                % (r["unit"], want))
        else:
            mism.append(r)
    sm = []
    if rec_result.get("prStatus") and got["pr_status"] != rec_result["prStatus"]:
        row = {"field": "prStatus", "derived": got["pr_status"],
               "recorded": rec_result["prStatus"], "reasons": got["reasons"]}
        # If the only thing separating us is a judgement this file refused to make, that is
        # an unanswered question, not a disagreement.
        if got["undecided"] and not got["reasons"]:
            row["why"] = "differs only on a deferred judgement"
            incon.append(row)
        else:
            sm.append(row)
    rs = rec_result.get("specComplianceScore")
    if rs is not None and abs(float(rs) - got["score"]) > 0.005:
        row = {"field": "specComplianceScore", "derived": got["score"], "recorded": rs}
        # The score is a function of PR_STATUS, so an undecided status makes it undecided.
        (incon if (incon or got["undecided"]) else sm).append(row)

    report.update(got)
    report["checks"] = [{
        "id": "PRST-01", "name": "pr_status_derivation_matches_record",
        "pass": (not mism and not sm) if args.verify else True,
        "evidence": mism + sm, "inconclusive": incon,
        "message": ("%d unit(s) and %d run-level field(s) differ from the record"
                    % (len(mism), len(sm))) if (mism or sm) else "ok"}]
    report["needs_human"].extend(got["undecided"])
    report["summary"] = {"units": len(got["rows"]), "unit_mismatches": len(mism),
                         "field_mismatches": len(sm), "inconclusive": len(incon),
                         "pr_status": got["pr_status"],
                         "score": got["score"], "verify": bool(args.verify)}
    report["pass"] = all(c["pass"] for c in report["checks"]) and not report["unparsed"]
    return emit(0 if report["pass"] else 1)


def _replay():
    """Self-check of all three rules, including the traps 2.8 calls out."""
    def _ps(*a, **k):
        return pr_status(*a, **k)[:2]

    # FLOW_STATUS order: WITHDRAWN and no_op come before any delivery judgement.
    assert flow_status("withdrawn", None, "PASS", "SUCCESS", False)[0] == "WITHDRAWN"
    assert flow_status("spec_gap", None, "PASS", "SUCCESS", False)[0] == "WITHDRAWN"
    assert flow_status("planned", "withdraw", "PASS", "SUCCESS", False)[0] == "WITHDRAWN"
    assert flow_status("no_op", None, None, None, False)[0] == "no_op"
    # UNRESOLVED outranks a successful surface.
    assert flow_status("planned", None, "PASS", "SUCCESS", True)[0] == "UNRESOLVED"
    assert flow_status("planned", None, "FAIL", "SUCCESS", False)[0] == "UNRESOLVED"
    assert flow_status("planned", None, "PASS", "FAILED", False)[0] == "UNRESOLVED"
    assert flow_status("planned", None, "PASS", "SUCCESS", False)[0] == "DELIVERED_VERIFIED"
    assert flow_status("planned", None, "PASS", "E2E_BLOCKED", False)[0] == "DELIVERED_E2E_BLOCKED"
    # "GRPC_STATUS=BLOCKED and E2E_STATUS != SUCCESS" is also E2E_BLOCKED.
    assert flow_status("planned", None, "BLOCKED", "E2E_SKIPPED", False)[0] == "DELIVERED_E2E_BLOCKED"
    assert flow_status("planned", None, "PASS", "E2E_SKIPPED", False)[0] == "DELIVERED_E2E_SKIPPED"

    # PR_STATUS: FAILED when nothing is delivered, whatever else is clean.
    assert _ps(["UNRESOLVED"], [], [], [], [], "", 0)[0] == "FAILED"
    assert _ps(["DELIVERED_VERIFIED"], [], [], [], [], "", 0)[0] == "READY"
    assert _ps(["DELIVERED_VERIFIED", "UNRESOLVED"], [], [], [], [], "", 0)[0] == "INCOMPLETE"
    assert _ps(["DELIVERED_VERIFIED"], [{"bug_id": "B1"}], [], [], [], "", 0)[0] == "INCOMPLETE"
    assert _ps(["DELIVERED_VERIFIED"], [], [], ["SECRET_LEAK"], [], "", 0)[0] == "INCOMPLETE"
    # a non-zero gate is NOT decided here -- it lands in undecided, and the status stays
    # whatever the other predicates say.
    st, rs, und = pr_status(["DELIVERED_VERIFIED"], [], [], [], ["make x"], "", 0)
    assert st == "READY" and not rs and len(und) == 1, (st, rs, und)
    # NOT_RAISED only blocks while open hs_changes exist.
    assert _ps(["DELIVERED_VERIFIED"], [], [], [], [], "NOT_RAISED:x", 2)[0] == "INCOMPLETE"
    assert _ps(["DELIVERED_VERIFIED"], [], [], [], [], "NOT_RAISED:x", 0)[0] == "READY"
    # PARTIAL needs a withdrawal and an otherwise clean board.
    assert _ps(["DELIVERED_VERIFIED", "WITHDRAWN"], [], [], [], [], "", 0)[0] == "PARTIAL"
    assert _ps(["DELIVERED_VERIFIED", "no_op"], [], [], [], [], "", 0)[0] == "READY"

    # score: the mean, the zero clause, and both clamps.
    v = {"flow_status": "DELIVERED_VERIFIED", "grpc": "PASS", "e2e": "SUCCESS"}
    b = {"flow_status": "DELIVERED_E2E_BLOCKED", "grpc": "PASS", "e2e": "E2E_BLOCKED"}
    assert spec_compliance_score([v] * 9, "READY") == 1.0
    assert spec_compliance_score([v] * 7 + [b] * 3, "READY") == 0.94       # worldpayraft
    assert spec_compliance_score([v] * 9 + [b], "READY") == 0.98           # nuvei
    assert spec_compliance_score([v] * 8 + [b], "INCOMPLETE") == 0.5       # shift4 clamp
    assert spec_compliance_score([v] * 9, "FAILED") == 0.0
    # a unit that passed nothing anywhere scores 0 even if its flow_status looks delivered.
    nothing = {"flow_status": "DELIVERED_E2E_SKIPPED", "grpc": "FAIL", "e2e": "E2E_SKIPPED"}
    assert spec_compliance_score([nothing], "READY") == 0.0

    print("replay OK: FLOW_STATUS order holds, PR_STATUS first-match holds, and the score "
          "reproduces 1.00 / 0.94 / 0.98 / 0.50 / 0.00 from five real runs")
    return 0


if __name__ == "__main__":
    sys.exit(_replay() if sys.argv[1:2] == ["--replay"] else main())
