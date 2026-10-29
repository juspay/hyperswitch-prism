#!/usr/bin/env python3
"""Derive a unit's grpc_status and e2e_status from its checks.

Both rules are written out in `grace/workflow/2.6d_test_exec.md` "Phase 7" as ordered,
first-match-wins lists. This file is that specification, executed. It adds no behaviour:
every branch cites the rule number it implements, and `--verify` replays a finished run
and diffs what this file derives against what the run itself recorded.

Why it exists: the rules are pure functions of a check list, and two stages read the same
contract -- 2.6d writes `final.json`, 2.8_pr_run.md grades from it. Prose executed twice by
two agents is two chances to disagree. A function cannot.

Exit codes mirror grace/rulesbook/codegen/tools/refusal_gate.py:
  0 pass, 1 fail (a mismatch, in --verify), 2 could not evaluate.

Stdlib only.
"""

import argparse
import json
import os
import sys

# `2.6d` Phase 7 names these outcomes; anything else is counted but matches no rule.
PASS, FAIL, NO_ROW, SPEC_GAP = "PASS", "FAIL", "NO_ROW", "SPEC_GAP"
SANDBOX_BLOCKED, NOT_RUN = "SANDBOX_BLOCKED", "NOT_RUN"
GRPC_SURFACES = ("static", "ucs_grpc")


def grpc_status(unit, checks, no_harness_suite=(), is_webhook_unit=False):
    """-> (status, reason). 2.6d:515-528, first match wins.

    `checks` is the list of this unit's check dicts ({surface, outcome, ...}).
    """
    # 1. no_harness_suite -> BLOCKED. "A webhook unit never lands here" (:518-519).
    if unit in (no_harness_suite or ()) and not is_webhook_unit:
        return "BLOCKED", "NO_HARNESS_SUITE"

    rel = [c for c in checks if (c.get("surface") in GRPC_SURFACES)]
    outcomes = [str(c.get("outcome") or "").upper() for c in rel]

    # 2. >=1 NO_ROW -> FAIL (Rule 9: a claim the run never proved).
    if NO_ROW in outcomes:
        return FAIL, "NO_ROW"
    # 2b. >=1 SPEC_GAP and no FAIL -> BLOCKED (routes to links/techspec, not RCA).
    if SPEC_GAP in outcomes and FAIL not in outcomes:
        return "BLOCKED", SPEC_GAP
    # 3. none run -> NOT_RUN.
    ran = [o for o in outcomes if o != NOT_RUN]
    if not ran:
        return NOT_RUN, "none run"
    # 4. all PASS -> PASS. Judged over the checks that ran: a NOT_RUN check means "no
    # result", not a failure, and reading it as one graded units FAIL that the run
    # recorded PASS.
    if all(o == PASS for o in ran):
        return PASS, "all pass"
    # 5. no FAIL and >=1 SANDBOX_BLOCKED -> BLOCKED.
    if FAIL not in ran and SANDBOX_BLOCKED in ran:
        return "BLOCKED", SANDBOX_BLOCKED
    # 6. else -> FAIL.
    return FAIL, "fail present"


def e2e_status(unit, e2e_checks, hs, gates_missing=(), record_status=None):
    """-> (status, reason). 2.6d:529-538, first match wins.

    `hs` is `env/env.json .hs`; `e2e_checks` are this unit's `e2e:<flow>` checks.
    """
    available = (hs or {}).get("available")
    reason = (hs or {}).get("unavailable_reason")

    # 1. The ONLY path to E2E_SKIPPED (:532).
    if available is not True and reason == "NO_CHECKOUT":
        return "E2E_SKIPPED", "no HS: NO_CHECKOUT"
    # 2. A checkout exists, so the primary gate fails closed (:533-534).
    if available is not True:
        return "FAILED", "HS_ENV:%s" % reason
    # 3. No record -> FAILED, unless gates are missing -> E2E_BLOCKED. Never E2E_SKIPPED.
    if not e2e_checks:
        if gates_missing:
            return "E2E_BLOCKED", "HS_UNREACHABLE:%s" % ",".join(map(str, gates_missing))
        return "FAILED", "NO_E2E_RUN"
    # 4. Otherwise the record's own E2E_STATUS, verbatim (:538). `record_status` is the
    # status read from e2e/<N>.json for this unit -- NOT the check's `outcome`, which is
    # 2.6d's own PASS/FAIL grading of that record and uses a different vocabulary.
    if record_status:
        return str(record_status).upper(), "record"
    # A record exists as a check but its status could not be read: fail closed rather
    # than guess, and never read as E2E_SKIPPED.
    return "FAILED", "RECORD_STATUS_UNREADABLE"


# ---------------------------------------------------------------- run-dir plumbing

def _load(path, default=None):
    try:
        return json.load(open(path, encoding="utf-8"))
    except (OSError, ValueError):
        return default


def checks_by_unit(final, units=()):
    """final.json .checks is an object keyed by check_id.

    A check whose `unit` is `"*"` is run-wide and counts for every unit -- observed in real
    runs, where each unit's own `counts.pass` includes them. INFERRED: no workflow line
    states the wildcard, so --verify is what keeps this honest.
    """
    out, wide = {}, []
    raw = final.get("checks") or {}
    items = raw.items() if isinstance(raw, dict) else [(c.get("check_id"), c) for c in raw]
    for cid, c in items:
        if not isinstance(c, dict):
            continue
        c = dict(c, check_id=cid)
        if c.get("unit") == "*":
            wide.append(c)
        else:
            out.setdefault(c.get("unit"), []).append(c)
    for u in set(list(out) + list(units)):
        out.setdefault(u, [])
        out[u] = out[u] + wide
    return out


def is_e2e(c):
    return (str(c.get("check_id") or "").startswith("e2e:")
            or str(c.get("surface") or "") in ("hs_rest", "hs_webhook")
            or str(c.get("kind") or "") == "e2e")


def e2e_record_status(run_dir):
    """-> {unit: status} from e2e/<N>.json `records[]`.

    INFERRED RULE, not a quoted one: the same unit appears in several record files with
    different statuses, and no artifact marks which record supersedes which (2.5 Phase 6
    writes no supersession field). Latest file number wins, which is what the orchestrator
    does in prose. Any divergence this causes shows up in --verify rather than silently.
    """
    out, d = {}, os.path.join(run_dir, "e2e")
    try:
        names = sorted(
            (n for n in os.listdir(d) if n.endswith(".json") and n[:-5].isdigit()),
            key=lambda n: int(n[:-5]))
    except OSError:
        return out
    for n in names:                     # ascending, so later files overwrite earlier
        rec = _load(os.path.join(d, n), {}) or {}
        for r in (rec.get("records") or []):
            u = r.get("unit")
            st = r.get("e2e_status") or r.get("E2E_STATUS") or r.get("status")
            if u and st:
                out[u] = st
    return out


def derive_run(run_dir):
    """-> (rows, problems). One row per unit: derived vs recorded."""
    final = _load(os.path.join(run_dir, "test/final.json"))
    if final is None:
        return None, ["test/final.json missing or unreadable"]
    env = _load(os.path.join(run_dir, "env/env.json"), {}) or {}
    scout = _load(os.path.join(run_dir, "hs/scout.json"), {}) or {}
    hs = env.get("hs") or {}
    nhs = final.get("no_harness_suite") or []
    if isinstance(nhs, dict):
        nhs = list(nhs)


    units = final.get("units") or {}
    items = units.items() if isinstance(units, dict) else [(u.get("unit"), u) for u in units]
    by_unit = checks_by_unit(final, [u for u, _ in items])

    su = scout.get("units") or {}
    rec_status = e2e_record_status(run_dir)

    rows = []
    for unit, rec in items:
        cs = by_unit.get(unit, [])
        grpc_cs = [c for c in cs if not is_e2e(c)]
        e2e_cs = [c for c in cs if is_e2e(c)]
        gm = ((su.get(unit) or {}).get("gates_missing") or []) if isinstance(su, dict) else []
        g, greason = grpc_status(unit, grpc_cs, nhs, is_webhook_unit=(unit == "IncomingWebhook"))
        e, ereason = e2e_status(unit, e2e_cs, hs, gm, rec_status.get(unit))
        rows.append({
            "unit": unit,
            "grpc_derived": g, "grpc_recorded": (rec or {}).get("grpc_status"),
            "grpc_reason": greason,
            "e2e_derived": e, "e2e_recorded": (rec or {}).get("e2e_status"),
            "e2e_reason": ereason,
            "n_grpc_checks": len(grpc_cs), "n_e2e_checks": len(e2e_cs),
        })
    return rows, []


def main():
    ap = argparse.ArgumentParser(description="Derive grpc_status / e2e_status per unit")
    ap.add_argument("--run-dir", required=True)
    ap.add_argument("--verify", action="store_true",
                    help="diff against what the run recorded; non-zero on mismatch")
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

    rows, problems = derive_run(args.run_dir)
    if rows is None:
        report["unparsed"].append({"what": args.run_dir, "why": "; ".join(problems)})
        report["pass"] = False
        return emit(2)

    mism = [r for r in rows
            if (r["grpc_recorded"] is not None and r["grpc_derived"] != r["grpc_recorded"])
            or (r["e2e_recorded"] is not None and r["e2e_derived"] != r["e2e_recorded"])]
    report["rows"] = rows
    report["checks"] = [{
        "id": "DERIVE-01", "name": "status_derivation_matches_record",
        "pass": not mism if args.verify else True,
        "evidence": mism, "inconclusive": [],
        "message": ("%d of %d units derive differently from what the run recorded"
                    % (len(mism), len(rows))) if mism else "ok"}]
    report["summary"] = {"units": len(rows), "mismatches": len(mism),
                         "verify": bool(args.verify)}
    report["pass"] = all(c["pass"] for c in report["checks"]) and not report["unparsed"]
    return emit(0 if report["pass"] else 1)


def _replay():
    """Self-check of both rules, including the traps the workflow calls out."""
    # grpc rule order: NO_ROW beats everything below it.
    assert grpc_status("U", [{"surface": "ucs_grpc", "outcome": "NO_ROW"},
                             {"surface": "ucs_grpc", "outcome": "PASS"}])[0] == FAIL
    # 2b: SPEC_GAP blocks only when no FAIL is present.
    assert grpc_status("U", [{"surface": "ucs_grpc", "outcome": "SPEC_GAP"}])[0] == "BLOCKED"
    assert grpc_status("U", [{"surface": "ucs_grpc", "outcome": "SPEC_GAP"},
                             {"surface": "ucs_grpc", "outcome": "FAIL"}])[0] == FAIL
    assert grpc_status("U", [])[0] == NOT_RUN
    assert grpc_status("U", [{"surface": "ucs_grpc", "outcome": "NOT_RUN"}])[0] == NOT_RUN
    assert grpc_status("U", [{"surface": "ucs_grpc", "outcome": "PASS"},
                             {"surface": "static", "outcome": "PASS"}])[0] == PASS
    assert grpc_status("U", [{"surface": "ucs_grpc", "outcome": "PASS"},
                             {"surface": "ucs_grpc", "outcome": "SANDBOX_BLOCKED"}])[0] == "BLOCKED"
    assert grpc_status("U", [{"surface": "ucs_grpc", "outcome": "FAIL"}])[0] == FAIL
    # a NOT_RUN check alongside passes is not a failure (rule 4 over checks that ran).
    assert grpc_status("U", [{"surface": "ucs_grpc", "outcome": "PASS"},
                             {"surface": "ucs_grpc", "outcome": "NOT_RUN"}])[0] == PASS
    # the run-wide "*" check reaches every unit.
    f = {"checks": {"a": {"unit": "*", "surface": "ucs_grpc", "outcome": "PASS"},
                    "b": {"unit": "U1", "surface": "ucs_grpc", "outcome": "PASS"}}}
    bu = checks_by_unit(f, ["U1", "U2"])
    assert len(bu["U1"]) == 2 and len(bu["U2"]) == 1, bu
    assert "*" not in bu, bu
    # e2e surfaces must not be counted as grpc checks.
    assert grpc_status("U", [{"surface": "hs_rest", "outcome": "FAILED"}])[0] == NOT_RUN
    # rule 1 is BLOCKED, and a webhook unit never lands there (:518-519).
    assert grpc_status("Dispute", [], ["Dispute"])[0] == "BLOCKED"
    assert grpc_status("IncomingWebhook", [{"surface": "ucs_grpc", "outcome": "PASS"}],
                       ["IncomingWebhook"], is_webhook_unit=True)[0] == PASS

    # e2e rule 1 is the ONLY path to E2E_SKIPPED.
    assert e2e_status("U", [], {"available": False,
                               "unavailable_reason": "NO_CHECKOUT"})[0] == "E2E_SKIPPED"
    # rule 2: a checkout exists -> fail closed, never skipped.
    s, r = e2e_status("U", [], {"available": False, "unavailable_reason": "HS_BOOT_FAILED"})
    assert s == "FAILED" and "HS_BOOT_FAILED" in r, (s, r)
    # rule 3: no record -> FAILED, or E2E_BLOCKED when gates are missing. Never skipped.
    assert e2e_status("U", [], {"available": True})[0] == "FAILED"
    assert e2e_status("U", [], {"available": True}, ["is_pre_auth"])[0] == "E2E_BLOCKED"
    # the sequencing trap: a missing record must never score as E2E_SKIPPED (:540-543).
    for hs in ({"available": True}, {"available": True, "unavailable_reason": None}):
        assert e2e_status("U", [], hs)[0] != "E2E_SKIPPED"
    # rule 4: the RECORD's status, not the check's outcome vocabulary.
    assert e2e_status("U", [{"outcome": "PASS"}], {"available": True},
                      record_status="SUCCESS")[0] == "SUCCESS"
    assert e2e_status("U", [{"outcome": "PASS"}], {"available": True},
                      record_status="E2E_BLOCKED")[0] == "E2E_BLOCKED"
    # a check exists but no readable record status -> fail closed, never E2E_SKIPPED.
    s4, r4 = e2e_status("U", [{"outcome": "PASS"}], {"available": True})
    assert s4 == "FAILED" and r4 == "RECORD_STATUS_UNREADABLE", (s4, r4)

    print("replay OK: grpc first-match order holds, SPEC_GAP yields to FAIL, webhook units "
          "escape NO_HARNESS_SUITE, and a missing e2e record can never read as E2E_SKIPPED")
    return 0


if __name__ == "__main__":
    sys.exit(_replay() if sys.argv[1:2] == ["--replay"] else main())
