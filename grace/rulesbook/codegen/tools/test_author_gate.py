#!/usr/bin/env python3
"""Test-authoring gate for GRACE-authored connector test data.

A GRACE run now writes the connector's test cases into committed JSON under
`crates/internal/integration-tests/src/connector_specs/<connector>/` and the repo's own
harness runs them. That closes the seam where a run could declare a suite and ship no
assertion for it -- but it opens a new one: a run can now weaken or waive its own tests.
These checks police the diff the run authored.

  ASSERT-01  every scenario the run added or patched asserts something about the RESPONSE
  ASSERT-02  an `assert: {field: null}` deletion must come with a replacement rule
  CTX-01     a `grpc_req` override must not target a path the suite drives via context_map
  WAIV-01    every `unsupported_scenarios` reason must carry a class and evidence
  WAIV-02    no waiver may be added for a scenario that PASSed earlier in this run
  WAIV-03    no waiver may be added for a scenario a P0 plan hook names
  SUITE-01   every suite the run declares must have at least one executed PASS behind it

Exit codes: 0 pass, 1 fail, 2 could not evaluate (treated as fail by the caller).
Stdlib only.
"""

import argparse
import json
import os
import re
import subprocess
import sys

SPECS = "specs.json"
OVERRIDE = "override.json"
PRIVATE = "connector_specific_scenarios.json"
WEBHOOK = "webhook_payload.json"

SPECS_DIR = "crates/internal/integration-tests/src/connector_specs"
SUITES_DIR = "crates/internal/integration-tests/src/global_suites"

# A waiver must say which kind of thing is missing, and point at evidence.
WAIVER_RE = re.compile(
    r"^(PM_NOT_OFFERED|SANDBOX_NOT_PROVISIONED|FLOW_NOT_IN_SCOPE|CONNECTOR_API_LACKS): .+ [-—] .+"
)

# Money-moving suites: a weakened assertion here is an S0, not an S2.
MONEY_SUITES = {
    "PaymentService/Authorize",
    "PaymentService/Capture",
    "PaymentService/Refund",
    "PaymentService/Void",
    "PaymentService/SetupRecurring",
    "RecurringPaymentService/Charge",
}


def git_show(ref, path):
    """File contents at a ref, or None when it did not exist there."""
    try:
        out = subprocess.run(
            ["git", "show", "%s:%s" % (ref, path)],
            capture_output=True, text=True, check=False)
    except OSError as exc:
        raise RuntimeError("git show failed: %s" % exc)
    if out.returncode != 0:
        return None
    return out.stdout


def load_json(text, what, problems):
    if text is None:
        return None
    try:
        return json.loads(text)
    except ValueError as exc:
        problems.append("%s is not valid JSON: %s" % (what, exc))
        return None


def suite_dir(suite):
    return suite.replace("/", "_")


def context_map_targets(suite):
    """Request paths this suite drives from a dependency's req/res."""
    path = os.path.join(SUITES_DIR, suite_dir(suite), "suite_spec.json")
    if not os.path.isfile(path):
        return set()
    try:
        with open(path, "r", encoding="utf-8") as fh:
            spec = json.load(fh)
    except (OSError, ValueError):
        return set()
    targets = set()
    for dep in spec.get("depends_on") or []:
        if isinstance(dep, dict):
            for key in (dep.get("context_map") or {}):
                targets.add(key)
    return targets


def leaf_paths(obj, prefix=""):
    """Dot-paths of every leaf in a merge patch, so a nested override is comparable
    with a flat context_map key."""
    out = []
    if isinstance(obj, dict):
        for key, val in obj.items():
            here = "%s.%s" % (prefix, key) if prefix else key
            if isinstance(val, dict) and val:
                out.extend(leaf_paths(val, here))
            else:
                out.append(here)
    return out


def response_asserting(rules):
    """True when at least one rule says something about the response body.

    `{"error": {"must_not_exist": true}}` alone is not enough: it passes on an empty
    response, which is exactly the shape that let a suite go green over four real defects.
    """
    for field, rule in (rules or {}).items():
        if not isinstance(rule, dict):
            continue
        if "must_not_exist" in rule:
            continue
        if any(k in rule for k in ("must_exist", "equals", "one_of", "contains", "echo")):
            return True
    return False


# ------------------------------------------------------------------ report helpers

def report_rows(report_path, unparsed=None):
    """(suite, scenario) -> list of assertion_result, for non-dependency rows.

    "No report was asked for" and "a report was asked for and is not there" are different
    facts and must not collapse into the same empty dict. The second one used to be silent:
    the caller passed a path that did not exist, got {} back, and every report-backed check
    reported "not evaluated" -- which the pass rule then counted as a pass. A gate whose
    input is missing has not checked anything, so it says so through `unparsed`.
    """
    rows = {}
    if not report_path or report_path == "none":
        return rows
    if not os.path.isfile(report_path):
        if unparsed is not None:
            unparsed.append({"what": "report", "why": "path given but missing: %s" % report_path})
        return rows
    try:
        with open(report_path, "r", encoding="utf-8") as fh:
            blob = json.load(fh)
    except (OSError, ValueError) as exc:
        if unparsed is not None:
            unparsed.append({"what": "report", "why": "unreadable: %s: %s" % (report_path, exc)})
        return rows
    for entry in blob.get("runs") or []:
        if entry.get("is_dependency"):
            continue
        key = (entry.get("suite"), entry.get("scenario"))
        rows.setdefault(key, []).append(entry.get("assertion_result"))
    return rows


def prior_passes(run_dir):
    """(suite, scenario) pairs that PASSed in ANY earlier round of this run.

    A waived scenario is removed before the run and leaves no report row at all
    (scenario_api.rs removes it from the scenario map), so a waiver can only be caught
    by looking at rounds where it had not yet been written.
    """
    passed = set()
    if not run_dir or not os.path.isdir(run_dir):
        return passed
    results = os.path.join(run_dir, "test", "results")
    if not os.path.isdir(results):
        return passed
    for root, _dirs, files in os.walk(results):
        for name in files:
            if name != "report.json":
                continue
            for (suite, scenario), outcomes in report_rows(os.path.join(root, name)).items():
                if "PASS" in outcomes:
                    passed.add((suite, scenario))
    return passed


# ------------------------------------------------------------------------- checks

# Strength order for a single assertion rule. A replacement must bind at least as
# tightly as what it replaces: `equals` names the value, `one_of` names a set,
# `contains` names a substring, `must_exist` names only presence. `must_not_exist`
# is an absence claim and can never stand in for a claim about a value.
_STRENGTH = {"equals": 4, "one_of": 3, "contains": 2, "echo": 2, "must_exist": 1}


def rule_strength(rule):
    if not isinstance(rule, dict):
        return 0
    return max([_STRENGTH.get(k, 0) for k in rule] or [0])


def replaced_at_equal_strength(field, base_rules, kept):
    """True when deleting `field` left something at least as binding behind.

    Either the same field survives at equal-or-greater strength, or the patch adds a
    value-level claim (`equals`/`one_of`) on a field the base did not constrain at all --
    a genuinely different assertion, not a weaker restatement of the deleted one.
    """
    want = rule_strength((base_rules or {}).get(field))
    if rule_strength((kept or {}).get(field)) >= want:
        return True
    for f, r in (kept or {}).items():
        if f in (base_rules or {}):
            continue
        if rule_strength(r) >= 3:
            return True
    return False


def check_assertions(cur_priv, base_priv, cur_ovr, base_ovr):
    """ASSERT-01 and ASSERT-02 over the scenarios this run added or patched."""
    ev01, ev02 = [], []

    # Private scenarios are authored whole: every one the run added must assert a response.
    for suite, scenarios in (cur_priv or {}).items():
        base_suite = (base_priv or {}).get(suite, {})
        for name, defn in (scenarios or {}).items():
            if base_suite.get(name) == defn:
                continue
            rules = (defn or {}).get("assert") or {}
            if not rules:
                ev01.append({"file": PRIVATE, "suite": suite, "scenario": name,
                             "detail": "no assert block (the loader will reject this)"})
            elif not response_asserting(rules):
                ev01.append({"file": PRIVATE, "suite": suite, "scenario": name,
                             "detail": "assert says nothing about the response body"})

    # Override assert patches: a null deletes a rule.
    for suite, scenarios in (cur_ovr or {}).items():
        base_suite = (base_ovr or {}).get(suite, {})
        for name, patch in (scenarios or {}).items():
            if base_suite.get(name) == patch:
                continue
            rules = (patch or {}).get("assert")
            if not isinstance(rules, dict):
                continue
            deleted = [f for f, r in rules.items() if r is None]
            kept = {f: r for f, r in rules.items() if r is not None}
            if not deleted:
                continue

            # A money suite does not get to trade one assertion for another. The old rule
            # accepted any surviving response assertion as cover, so a row could delete
            # `status: one_of [REFUND_SUCCESS, ...]`, keep `error: {must_exist: true}`, and
            # pass -- leaving a refund row whose entire claim was that something went wrong.
            if suite in MONEY_SUITES:
                ev02.append({
                    "file": OVERRIDE, "suite": suite, "scenario": name,
                    "detail": "deletes %s on a money suite; no replacement is accepted there"
                              % deleted,
                    "severity": "S0"})
                continue

            base_rules = (base_suite.get(name) or {}).get("assert") or {}
            weak = [f for f in deleted
                    if not replaced_at_equal_strength(f, base_rules, kept)]
            if weak and not response_asserting(kept):
                ev02.append({
                    "file": OVERRIDE, "suite": suite, "scenario": name,
                    "detail": "deletes %s and adds no replacement response assertion" % deleted,
                    "severity": "S1"})
            elif weak:
                ev02.append({
                    "file": OVERRIDE, "suite": suite, "scenario": name,
                    "detail": "deletes %s; what remains does not constrain those fields at "
                              "equal or greater strength" % weak,
                    "severity": "S1"})
    return ev01, ev02


def check_context_map(cur_ovr, base_ovr):
    """CTX-01: an override on a context_map target silently decouples the dependency."""
    evidence = []
    for suite, scenarios in (cur_ovr or {}).items():
        targets = context_map_targets(suite)
        if not targets:
            continue
        base_suite = (base_ovr or {}).get(suite, {})
        for name, patch in (scenarios or {}).items():
            if base_suite.get(name) == patch:
                continue
            if (patch or {}).get("same_endpoint_justified"):
                continue
            for path in leaf_paths((patch or {}).get("grpc_req") or {}):
                for target in targets:
                    if path == target or path.startswith(target + "."):
                        evidence.append({
                            "file": OVERRIDE, "suite": suite, "scenario": name,
                            "detail": "grpc_req overrides '%s', which this suite drives from a "
                                      "dependency via context_map ('%s'); the override is applied "
                                      "after the context map and wins" % (path, target)})
    return evidence


def check_private_context_map(cur_priv, base_priv):
    """Advisory: a private scenario setting a context_map target has that value silently
    replaced, because the context map is applied after the scenario's own grpc_req.
    Futile rather than dangerous -- a warning, not a failure."""
    notes = []
    for suite, scenarios in (cur_priv or {}).items():
        targets = context_map_targets(suite)
        if not targets:
            continue
        base_suite = (base_priv or {}).get(suite, {})
        for name, defn in (scenarios or {}).items():
            if base_suite.get(name) == defn:
                continue
            for path in leaf_paths((defn or {}).get("grpc_req") or {}):
                for target in targets:
                    if path == target or path.startswith(target + "."):
                        notes.append(
                            "%s/%s sets '%s', which this suite drives from a dependency via "
                            "context_map; the scenario's value is replaced at run time"
                            % (suite, name, path))
    return notes


def check_waivers(cur_specs, base_specs, passed_before, plan):
    """WAIV-01/02/03 over unsupported_scenarios entries this run added."""
    ev01, ev02, ev03 = [], [], []
    cur = (cur_specs or {}).get("unsupported_scenarios") or {}
    base = (base_specs or {}).get("unsupported_scenarios") or {}

    p0 = set()
    for hook in ((plan or {}).get("test_hooks") or []):
        for group in ("global_scenarios", "connector_scenarios"):
            for row in (hook.get(group) or []):
                if row.get("priority") == "P0":
                    p0.add((row.get("suite"), row.get("scenario")))

    for suite, scenarios in cur.items():
        base_suite = base.get(suite, {})
        for name, reason in (scenarios or {}).items():
            if base_suite.get(name) == reason:
                continue
            if not isinstance(reason, str) or not WAIVER_RE.match(reason.strip()):
                ev01.append({"file": SPECS, "suite": suite, "scenario": name,
                             "detail": "reason does not parse as '<CLASS>: <evidence> - <line>': %r"
                                       % (reason if isinstance(reason, str) else reason)})
            if (suite, name) in passed_before:
                ev02.append({"file": SPECS, "suite": suite, "scenario": name,
                             "detail": "this scenario PASSed earlier in this run; waiving it now "
                                       "would bury a working test"})
            if (suite, name) in p0:
                ev03.append({"file": SPECS, "suite": suite, "scenario": name,
                             "detail": "a P0 plan hook names this scenario; it may not be waived"})
    return ev01, ev02, ev03


def check_suites(cur_specs, base_specs, rows, touched_suites=None, needs_human=None):
    """SUITE-01: a suite this run declares must have an executed PASS behind it.

    Every declared suite is audited, not only the ones this run added. A weak declaration
    does not become true by ageing, and scoping the check to `cur - base` meant the longer
    an unproven claim survived the less anything looked at it.

    A suite the run neither added nor touched is grandfathered: it is reported through
    `needs_human` rather than failing the gate, so inherited debt is visible without making
    every unrelated run red. Touch any scenario or override under it and it is audited like
    a new declaration -- editing a suite is adopting it.
    """
    evidence = []
    cur = set((cur_specs or {}).get("supported_suites") or [])
    base = set((base_specs or {}).get("supported_suites") or [])
    touched = set(touched_suites or ())
    for suite in sorted(cur):
        passes = [r for (s, _sc), outs in rows.items() if s == suite for r in outs if r == "PASS"]
        if passes:
            continue
        detail = ("declared in supported_suites but no scenario of it PASSed in the report "
                  "(declaring a suite with nothing behind it is what lets a green run hide "
                  "an untested flow)")
        if suite in (cur - base) or suite in touched:
            evidence.append({"file": SPECS, "suite": suite, "scenario": None, "detail": detail})
        elif needs_human is not None:
            needs_human.append(
                "SUITE-01 grandfathered: %s was declared before this run and still has no "
                "executed PASS" % suite)
    return evidence


def main():
    ap = argparse.ArgumentParser(description="Test-authoring gate for connector test data")
    ap.add_argument("--connector", required=True)
    ap.add_argument("--base-sha", required=True,
                    help="the run's base_sha; the diff is computed against it")
    ap.add_argument("--specs-dir", default=SPECS_DIR)
    ap.add_argument("--report", default="none",
                    help="this round's report.json; omit to skip the report-backed checks")
    ap.add_argument("--run-dir", default="none",
                    help="RUN_DIR, so WAIV-02 can see earlier rounds' reports")
    ap.add_argument("--plan", default="none")
    ap.add_argument("--out")
    ap.add_argument("--allow-unevaluated", action="store_true",
                    help="treat a check that could not run as non-failing. Legitimate only "
                         "before the first execution round (run.json .counters.exec_round == 0), "
                         "when no report exists yet. Never pass it to make a red gate green: "
                         "the unevaluated ids are recorded in needs_human[] either way.")
    args = ap.parse_args()

    base = os.path.join(args.specs_dir, args.connector)
    problems = []
    report = {"connector": args.connector, "pass": True, "checks": [],
              "unparsed": [], "needs_human": []}

    def read_pair(name):
        cur_path = os.path.join(base, name)
        cur_text = None
        if os.path.isfile(cur_path):
            with open(cur_path, "r", encoding="utf-8") as fh:
                cur_text = fh.read()
        return (load_json(cur_text, name, problems),
                load_json(git_show(args.base_sha, cur_path), "%s@base" % name, problems))

    cur_specs, base_specs = read_pair(SPECS)
    cur_ovr, base_ovr = read_pair(OVERRIDE)
    cur_priv, base_priv = read_pair(PRIVATE)

    if cur_specs is None:
        report["unparsed"].append({"what": SPECS, "why": "missing or unreadable"})
        report["needs_human"].append(
            "%s/%s is missing; the connector declares no suites and nothing can be checked"
            % (args.connector, SPECS))
    for problem in problems:
        report["unparsed"].append({"what": "json", "why": problem})

    plan = None
    if args.plan and args.plan != "none" and os.path.isfile(args.plan):
        with open(args.plan, "r", encoding="utf-8") as fh:
            plan = json.load(fh)

    rows = report_rows(args.report, report["unparsed"])
    have_report = bool(rows)
    passed_before = prior_passes(None if args.run_dir == "none" else args.run_dir)

    ev01, ev02 = check_assertions(cur_priv, base_priv, cur_ovr, base_ovr)
    ctx = check_context_map(cur_ovr, base_ovr)
    report["needs_human"].extend(check_private_context_map(cur_priv, base_priv))
    w1, w2, w3 = check_waivers(cur_specs, base_specs, passed_before, plan)

    def add(cid, name, evidence, message, skipped=None):
        report["checks"].append({
            "id": cid, "name": name,
            # None, not False, when the check could not run -- a reader must be able to tell
            # "did not evaluate" from "evaluated and failed".
            "pass": None if skipped else not evidence,
            "skipped": skipped, "evidence": evidence,
            "message": message if evidence else ("not evaluated: %s" % skipped if skipped
                                                 else "ok")})

    add("ASSERT-01", "scenario_asserts_response", ev01,
        "A scenario that asserts nothing about the response passes on an empty response. "
        "Assert a response field, not only the absence of an error.")
    add("ASSERT-02", "assert_deletion_replaced", ev02,
        "Deleting an assertion without replacing it weakens the suite silently. "
        "On a money-moving suite that is an S0.")
    add("CTX-01", "override_vs_context_map", ctx,
        "The override is applied after the context map, so overriding a context_map target "
        "detaches the scenario from the dependency that was supposed to feed it.")
    add("WAIV-01", "waiver_reason_parses", w1,
        "A waiver is the only record of why a scenario is not run. It must name a class and cite "
        "evidence.")
    add("WAIV-03", "waiver_not_p0", w3,
        "A P0 hook is the run's own statement that this scenario matters.")

    if have_report or passed_before:
        add("WAIV-02", "waiver_not_over_a_pass", w2,
            "Waiving a scenario that already passed in this run hides a working test.")
    else:
        add("WAIV-02", "waiver_not_over_a_pass", [], "", skipped="no report from an earlier round")

    if have_report:
        touched_suites = set(cur_priv or {}) | set(cur_ovr or {})
        add("SUITE-01", "declared_suite_has_a_pass",
            check_suites(cur_specs, base_specs, rows, touched_suites, report["needs_human"]),
            "A declared suite with no passing scenario behind it is an untested flow that reads "
            "as covered.")
    else:
        add("SUITE-01", "declared_suite_has_a_pass", [], "", skipped="no --report given")

    for chk in report["checks"]:
        if chk.get("skipped"):
            report["needs_human"].append("%s not evaluated: %s" % (chk["id"], chk["skipped"]))

    # A check that could not run has not passed. `pass` is None for those (see add()), and
    # `None is not False` is True -- so the old rule counted every unevaluated check as green.
    # That is how SUITE-01 reported a pass in four consecutive rounds while its --report path
    # did not exist: the one check that asks whether a declared suite has anything behind it
    # was the check most able to go missing, and going missing is what made it agreeable.
    unevaluated = [c["id"] for c in report["checks"] if c["pass"] is None]
    report["unevaluated"] = unevaluated
    report["pass"] = all(c["pass"] is True for c in report["checks"]) \
        and not report["unparsed"]

    # Before the first execution round there is genuinely no report to read, and the
    # report-backed checks cannot bind yet. That is the one case where unevaluated is not a
    # failure -- and the caller has to say so deliberately rather than it being the default.
    if unevaluated and args.allow_unevaluated:
        report["pass"] = all(c["pass"] is not False for c in report["checks"]) \
            and not report["unparsed"]
        report["needs_human"].append(
            "unevaluated checks allowed by --allow-unevaluated: %s" % ", ".join(unevaluated))

    blob = json.dumps(report, indent=1)
    if args.out:
        os.makedirs(os.path.dirname(os.path.abspath(args.out)), exist_ok=True)
        with open(args.out, "w", encoding="utf-8") as fh:
            fh.write(blob + "\n")
    print(blob)

    if report["unparsed"]:
        return 2
    if report.get("unevaluated") and not args.allow_unevaluated:
        return 2
    return 0 if report["pass"] else 1


if __name__ == "__main__":
    sys.exit(main())
