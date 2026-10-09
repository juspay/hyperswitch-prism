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
  SUITE-01   every suite the run declares must have at least one COMMITTED scenario's PASS
             behind it (a row with no `suite_ref` came from an ad-hoc matrix, not the suite)

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

def load_report(report_path):
    """(blob, error) for a report path. error is None only when a blob was read."""
    if not report_path or report_path == "none":
        return None, None                      # legitimately not given
    if not os.path.isfile(report_path):
        return None, "file does not exist"
    try:
        with open(report_path, "r", encoding="utf-8") as fh:
            return json.load(fh), None
    except (OSError, ValueError) as exc:
        return None, "unreadable or not JSON (%s)" % exc.__class__.__name__


def report_rows(report_path):
    """(suite, case) -> list of outcomes.

    Reads the gRPC agent's round file (`test/grpc/r<N>.json`, `2.6f_grpc_agent.md`
    Phase 6), whose `rows[]` carry `method` (a `Service/Method`, the same shape
    `specs.json` `supported_suites` uses), `case_id` and `outcome`.

    The legacy harness report (`runs[]` with `suite`/`scenario`/`assertion_result`) is
    still accepted so an older run directory stays readable; nothing in the current
    pipeline writes one.
    """
    rows = {}
    blob, _error = load_report(report_path)
    if blob is None:
        return rows
    for entry in blob.get("rows") or []:
        key = (entry.get("method"), entry.get("case_id"))
        rows.setdefault(key, []).append(entry.get("outcome"))
    for entry in blob.get("runs") or []:          # legacy harness report
        if entry.get("is_dependency"):
            continue
        key = (entry.get("suite"), entry.get("scenario"))
        rows.setdefault(key, []).append(entry.get("assertion_result"))
    return rows


def scenario_passes(blob):
    """(suite, scenario) pairs that PASSed in one report blob.

    A gRPC round row is keyed by `case_id`, which is a different namespace from a
    `specs.json` scenario name -- so WAIV-02 would never fire on it. The row carries
    `suite_ref` ("<suite>/<scenario>") for exactly this join; rows with none came from
    no committed scenario and cannot be waived.
    """
    passed = set()
    for entry in (blob or {}).get("rows") or []:
        ref = entry.get("suite_ref")
        if ref and entry.get("outcome") == "PASS" and "/" in ref:
            suite, _, scenario = ref.rpartition("/")
            passed.add((suite, scenario))
    for entry in (blob or {}).get("runs") or []:          # legacy harness report
        if entry.get("is_dependency"):
            continue
        if entry.get("assertion_result") == "PASS":
            passed.add((entry.get("suite"), entry.get("scenario")))
    return passed


def prior_passes(run_dir):
    """(suite, scenario) pairs that PASSed in ANY earlier round of this run.

    A waived scenario is removed before the run and leaves no report row at all
    (scenario_api.rs removes it from the scenario map), so a waiver can only be caught
    by looking at rounds where it had not yet been written.
    """
    passed = set()
    if not run_dir or not os.path.isdir(run_dir):
        return passed
    for sub_dir in ("grpc", "results"):
        root_dir = os.path.join(run_dir, "test", sub_dir)
        if not os.path.isdir(root_dir):
            continue
        for root, _dirs, files in os.walk(root_dir):
            for name in files:
                # 2.6f round files are test/grpc/r<N>.json; the legacy harness
                # wrote test/results/**/report.json.
                if not (name == "report.json"
                        or (name.startswith("r") and name.endswith(".json"))):
                    continue
                blob, _error = load_report(os.path.join(root, name))
                passed |= scenario_passes(blob)
    return passed


# ------------------------------------------------------------------------- checks

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
            if deleted and not response_asserting(kept):
                ev02.append({
                    "file": OVERRIDE, "suite": suite, "scenario": name,
                    "detail": "deletes %s and adds no replacement response assertion" % deleted,
                    "severity": "S0" if suite in MONEY_SUITES else "S1"})
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


def check_suites(cur_specs, base_specs, passes):
    """SUITE-01: a suite this run declares must have a committed scenario's PASS behind it.

    `passes` is scenario_passes()'s (suite, scenario) set, which counts a gRPC row only
    when it carries a `suite_ref` -- that is, only when a scenario committed to
    specs.json/override.json drove it. Joining on the row's `method` instead would accept
    an ad-hoc grpcurl matrix, because `method` and `supported_suites` share the
    `Service/Method` shape: the suite would read as proven while `make test-connector`
    reproduced none of it. That is the exact hole this check exists to close, so the join
    has to be the same one WAIV-02 already uses.
    """
    if isinstance(passes, dict):
        # report_rows()'s dict is keyed by (method, case_id), so iterating it yields 2-tuples
        # too and the comprehension below would silently reinstate the `method` join. Refuse
        # the shape instead of accepting it quietly: the caller treats a crash as a failure.
        raise TypeError(
            "check_suites takes scenario_passes()'s (suite, scenario) set, not report_rows()'s "
            "method-keyed dict; that dict's keys unpack the same way and would restore the "
            "ad-hoc-matrix join SUITE-01 exists to reject")
    evidence = []
    cur = set((cur_specs or {}).get("supported_suites") or [])
    base = set((base_specs or {}).get("supported_suites") or [])
    proven = {suite for suite, _scenario in (passes or set())}
    for suite in sorted(cur - base):
        if suite not in proven:
            evidence.append({
                "file": SPECS, "suite": suite, "scenario": None,
                "detail": "declared in supported_suites but no committed scenario of it PASSed in "
                          "the report (declaring a suite with nothing behind it is what lets a "
                          "green run hide an untested flow; a PASS whose row carries no "
                          "suite_ref came from an ad-hoc matrix and is not the suite)"})
    return evidence


def main():
    ap = argparse.ArgumentParser(description="Test-authoring gate for connector test data")
    ap.add_argument("--connector", required=True)
    ap.add_argument("--base-sha", required=True,
                    help="the run's base_sha; the diff is computed against it")
    ap.add_argument("--specs-dir", default=SPECS_DIR)
    ap.add_argument("--report", default="none",
                    help="this round's test/grpc/r<N>.json; omit before the first test round")
    ap.add_argument("--run-dir", default="none",
                    help="RUN_DIR, so WAIV-02 can see earlier rounds' reports")
    ap.add_argument("--plan", default="none")
    ap.add_argument("--out")
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

    # A --report path that was given but does not exist must fail, not skip. A skipped
    # check carries pass: None, which `all(... is not False ...)` counts as passing -- so
    # a mistyped or stale path would silently turn SUITE-01 and WAIV-02 off while the gate
    # still reported pass. "none" (or omitted) stays a legitimate "no round has run yet".
    # A --report that was given but yields nothing must fail, not skip. A skipped check
    # carries pass: None, which `all(... is not False ...)` counts as passing -- so a stale
    # path, a truncated file or a round that executed no rows would silently turn SUITE-01
    # and WAIV-02 off while the gate still reported pass. "none" (or omitted) stays a
    # legitimate "no round has run yet".
    report_blob, report_error = load_report(args.report)
    rows = report_rows(args.report)
    if args.report and args.report != "none" and not rows:
        report_error = report_error or "contains no rows"
    report_missing = bool(report_error)
    if report_missing:
        report["unparsed"].append({
            "what": "--report %s" % args.report,
            "why": "%s, so the checks that need it could not run; that is a failure, not a skip"
                   % report_error})
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
        add("WAIV-02", "waiver_not_over_a_pass", [], "",
            skipped=("--report %s: %s" % (args.report, report_error)) if report_missing
            else "no report from an earlier round")

    if have_report:
        add("SUITE-01", "declared_suite_has_a_pass",
            check_suites(cur_specs, base_specs, scenario_passes(report_blob)),
            "A declared suite with no passing scenario behind it is an untested flow that reads "
            "as covered.")
    else:
        add("SUITE-01", "declared_suite_has_a_pass", [], "",
            skipped=("--report %s: %s" % (args.report, report_error)) if report_missing
            else "no --report given")

    for chk in report["checks"]:
        if chk.get("skipped"):
            report["needs_human"].append("%s not evaluated: %s" % (chk["id"], chk["skipped"]))

    report["pass"] = all(c["pass"] is not False for c in report["checks"]) \
        and not report["unparsed"]

    blob = json.dumps(report, indent=1)
    if args.out:
        os.makedirs(os.path.dirname(os.path.abspath(args.out)), exist_ok=True)
        with open(args.out, "w", encoding="utf-8") as fh:
            fh.write(blob + "\n")
    print(blob)

    if report["unparsed"]:
        return 2
    return 0 if report["pass"] else 1


def _replay():
    """Self-check over the seven checks, exercising each one's pass AND fail path.

    The checks are pure functions over parsed JSON, so they are testable without a repo: the
    only disk dependency is `context_map_targets`, which reads SUITES_DIR, and that is a module
    global a fixture can point elsewhere.
    """
    global SUITES_DIR
    import tempfile

    # -- response_asserting: the trap this gate exists for -------------------------------
    # `{"error": {"must_not_exist": true}}` passes on an EMPTY response, which is how a suite
    # went green over four real defects. It must not count as asserting the response.
    assert not response_asserting({"error": {"must_not_exist": True}})
    assert not response_asserting({}) and not response_asserting(None)
    assert response_asserting({"status": {"equals": "APPROVED"}})
    assert response_asserting({"id": {"must_exist": True}})
    # a non-dict rule must not crash or count
    assert not response_asserting({"status": "APPROVED"})
    # the `continue` is load-bearing, not decorative: a field carrying must_not_exist is
    # skipped WHOLE, so a contradictory rule cannot smuggle itself in via a sibling key.
    assert not response_asserting({"error": {"must_not_exist": True, "equals": "x"}})
    # ...while a must_not_exist field alongside a SEPARATE asserting field is fine
    assert response_asserting({"error": {"must_not_exist": True},
                               "status": {"equals": "OK"}})

    # -- leaf_paths: a nested override must be comparable with a flat context_map key ----
    assert sorted(leaf_paths({"a": {"b": 1}, "c": 2})) == ["a.b", "c"]
    assert leaf_paths({"a": {}}) == ["a"], "an empty dict is a leaf, not a branch"

    # -- ASSERT-01: private scenarios the run added -------------------------------------
    base_priv = {"PaymentService/Authorize": {"old": {"assert": {"id": {"must_exist": True}}}}}
    cur_priv = {"PaymentService/Authorize": {
        "old":      {"assert": {"id": {"must_exist": True}}},        # unchanged -> skipped
        "no_block": {"grpc_req": {}},                                 # no assert at all
        "silent":   {"assert": {"error": {"must_not_exist": True}}},  # says nothing of the body
        "good":     {"assert": {"status": {"equals": "APPROVED"}}},
    }}
    ev01, _ = check_assertions(cur_priv, base_priv, {}, {})
    names = sorted(e["scenario"] for e in ev01)
    assert names == ["no_block", "silent"], names
    assert "loader will reject" in [e for e in ev01 if e["scenario"] == "no_block"][0]["detail"]

    # -- ASSERT-02: a null deletion needs a replacement response assertion ---------------
    cur_ovr = {"PaymentService/Authorize": {"s": {"assert": {"status": None}}}}
    _, ev02 = check_assertions({}, {}, cur_ovr, {})
    assert len(ev02) == 1 and ev02[0]["severity"] == "S0", ev02
    # same shape off a money suite is S1, not S0
    _, ev02b = check_assertions({}, {}, {"CustomerService/Create": {"s": {"assert": {"x": None}}}}, {})
    assert ev02b and ev02b[0]["severity"] == "S1", ev02b
    # a deletion WITH a replacement response assertion is fine
    _, ev02c = check_assertions({}, {}, {"PaymentService/Authorize":
        {"s": {"assert": {"status": None, "id": {"must_exist": True}}}}}, {})
    assert ev02c == [], ev02c

    # -- CTX-01 / private context_map: need a fixture suite_spec.json --------------------
    saved = SUITES_DIR
    try:
        tmp = tempfile.mkdtemp()
        os.makedirs(os.path.join(tmp, "PaymentService_Capture"))
        with open(os.path.join(tmp, "PaymentService_Capture", "suite_spec.json"), "w") as fh:
            json.dump({"depends_on": [{"context_map": {"amount_to_capture.value": "res.amount"}}]}, fh)
        SUITES_DIR = tmp
        assert context_map_targets("PaymentService/Capture") == {"amount_to_capture.value"}
        assert context_map_targets("PaymentService/Nope") == set(), "a missing suite yields no targets"

        # a nested grpc_req override on that target is caught via leaf_paths
        hit = check_context_map({"PaymentService/Capture":
            {"s": {"grpc_req": {"amount_to_capture": {"value": 100}}}}}, {})
        assert len(hit) == 1 and "context_map" in hit[0]["detail"], hit
        # the documented escape hatch is honoured
        assert check_context_map({"PaymentService/Capture":
            {"s": {"grpc_req": {"amount_to_capture": {"value": 1}},
                   "same_endpoint_justified": True}}}, {}) == []
        # an unrelated path is not caught
        assert check_context_map({"PaymentService/Capture": {"s": {"grpc_req": {"other": 1}}}}, {}) == []
        # a PRIVATE scenario on the same target is advisory only -- a note, never evidence
        notes = check_private_context_map({"PaymentService/Capture":
            {"s": {"grpc_req": {"amount_to_capture": {"value": 1}}}}}, {})
        assert len(notes) == 1 and "replaced at run time" in notes[0], notes
    finally:
        SUITES_DIR = saved

    # -- WAIV-01/02/03 -------------------------------------------------------------------
    plan = {"test_hooks": [{"global_scenarios": [
        {"suite": "PaymentService/Authorize", "scenario": "p0_case", "priority": "P0"}]}]}
    cur_specs = {"unsupported_scenarios": {"PaymentService/Authorize": {
        "bad_reason": "just because",
        "was_passing": "PM_NOT_OFFERED: docs p4 - the method is not offered",
        "p0_case":     "PM_NOT_OFFERED: docs p4 - the method is not offered",
    }}}
    passed_before = {("PaymentService/Authorize", "was_passing")}
    ev1, ev2, ev3 = check_waivers(cur_specs, {}, passed_before, plan)
    assert [e["scenario"] for e in ev1] == ["bad_reason"], ev1
    assert [e["scenario"] for e in ev2] == ["was_passing"], ev2
    assert [e["scenario"] for e in ev3] == ["p0_case"], ev3
    # the class list and BOTH dash forms are accepted
    for dash in ("-", "\u2014"):
        assert WAIVER_RE.match("CONNECTOR_API_LACKS: spec:x %s no endpoint" % dash)
    assert not WAIVER_RE.match("NOT_A_CLASS: spec:x - y")
    # an unchanged waiver is not re-reported
    assert check_waivers(cur_specs, cur_specs, set(), {}) == ([], [], [])

    # -- SUITE-01: a declared suite needs an executed PASS behind it ---------------------
    cur_specs2 = {"supported_suites": ["PaymentService/Authorize", "PaymentService/Void"]}
    base_specs2 = {"supported_suites": ["PaymentService/Authorize"]}
    ev = check_suites(cur_specs2, base_specs2, set())
    assert len(ev) == 1 and ev[0]["suite"] == "PaymentService/Void", ev
    assert check_suites(cur_specs2, base_specs2, {("PaymentService/Void", "void_ok")}) == []
    # a suite already in base is not re-checked even with no PASS
    assert check_suites(base_specs2, base_specs2, set()) == []

    # the join must be scenario_passes', not the row's `method`: a gRPC row with no
    # suite_ref came from an ad-hoc matrix, and `method` shares supported_suites' shape,
    # so joining on it would read the suite as proven while the committed suite is empty.
    blob = {"rows": [{"method": "PaymentService/Void", "case_id": "Void/card/ok",
                      "outcome": "PASS", "suite_ref": None}]}
    assert scenario_passes(blob) == set(), scenario_passes(blob)
    assert len(check_suites(cur_specs2, base_specs2, scenario_passes(blob))) == 1
    blob["rows"][0]["suite_ref"] = "PaymentService/Void/void_ok"
    assert check_suites(cur_specs2, base_specs2, scenario_passes(blob)) == []
    # the legacy harness report still counts: those rows ARE committed scenarios
    legacy = {"runs": [{"suite": "PaymentService/Void", "scenario": "void_ok",
                        "assertion_result": "PASS"}]}
    assert check_suites(cur_specs2, base_specs2, scenario_passes(legacy)) == []
    # report_rows()'s shape is refused outright -- its keys unpack like the pass set, so
    # accepting it would reinstate the `method` join without changing a visible line
    try:
        check_suites(cur_specs2, base_specs2, {("PaymentService/Void", "x"): ["PASS"]})
    except TypeError:
        pass
    else:
        raise AssertionError("check_suites must refuse report_rows()'s method-keyed dict")

    print("replay OK: response_asserting rejects a must_not_exist-only block (the shape that let "
          "a suite go green over four defects), ASSERT-01/02 fire only on scenarios this run "
          "changed and grade money suites S0, CTX-01 matches nested overrides against "
          "context_map targets and honours same_endpoint_justified while the private-scenario "
          "case stays advisory, WAIV-01/02/03 catch a malformed reason / waiving a passing test / "
          "waiving a P0 hook, and SUITE-01 refuses a suite declared with no COMMITTED PASS behind it "
          "(a suite_ref-less ad-hoc row does not count)")
    return 0


if __name__ == "__main__":
    sys.exit(_replay() if sys.argv[1:2] == ["--replay"] else main())
