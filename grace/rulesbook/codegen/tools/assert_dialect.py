#!/usr/bin/env python3
"""Check plan §8 assertions against the dialect the harness actually implements.

Why this exists. Two independent GRACE runs on the same connector wrote §8 assertions in a
dialect `crates/internal/integration-tests/src/harness` does not implement -- rule names
`not_one_of` / `not_contains_text`, and path operators `#json` / `#b64json` / `a|b`. The
first run deleted the scenario file and never tracked it; the second escalated three
PLAN_CONFLICTs and burned three plan revisions discovering the same thing one layer at a
time. Neither had a mechanical check, because `2.3a_plan.md` Phase 9 authors assertions
without being required to read the code that must run them.

The failure is not graceful. `FieldAssert` is `#[serde(untagged)]`, and its own source says
`deny_unknown_fields` cannot be applied to untagged variants -- so one unknown rule name does
not fail that rule, it fails the WHOLE scenario file with `ScenarioFileParse`, taking out
every suite for that connector.

The second failure class is `ScenarioNotFound`: a §8 id that resolves to nothing. The plan's
own jq self-check cannot catch it -- it compares `connector_scenarios[]` against the plan's
`global_scenarios[]` list, and jq cannot open `global_suites/<dir>/scenario.json`. So a run
escalated it as a PLAN_CONFLICT and burned a plan revision rediscovering it. DIAL-03/04 do
the literal `load_scenario` lookup, in both directions.

The supported set is read FROM THE HARNESS SOURCE, never hardcoded here, so the gate tracks
the harness as it changes (the precedent is `2.3b_codegen_unit.md`'s awk extraction over
`review_themes.md`: "Do not hardcode the check set here. It is read from the guide").

Contract matches the gates beside it: stdlib only, exit 0 pass / 1 fail / 2 could-not-evaluate,
a JSON report with `summary` / `inconclusive[]` / `needs_human[]`, and `--replay`.
"""
import argparse, difflib, json, os, re, sys, tempfile

HARNESS = "crates/internal/integration-tests/src/harness"
TYPES = HARNESS + "/scenario_types.rs"
ASSERT = HARNESS + "/scenario_assert.rs"
# load_scenario() resolves ids here; env var mirrors scenario_root() in scenario_loader.rs
SUITES = "crates/internal/integration-tests/src/global_suites"


def supported_rules(src):
    """-> the serde keys of FieldAssert's variants, read from the enum body."""
    # tolerate indentation before the closing brace: the enum is at column 0 in the real
    # source but a reformat (or a fixture) must not silently make this return None
    m = re.search(r"pub enum FieldAssert\s*\{(.*?)\n\s*\}", src, re.S)
    if not m:
        return None
    # struct-variant field names are the serde keys of an untagged enum
    return sorted(set(re.findall(r"\{\s*([a-z_][a-z0-9_]*)\s*:", m.group(1))))


def _fn_body(src, name):
    r"""The full body of `fn <name>`, by brace matching.

    A lazy regex cannot do this: `fn lookup_json_path` opens with an
    `if path.is_empty() { ... }` guard, so `.*?\n\s*\}` stops 135 chars in and silently
    reports the whole dialect as unsupported -- a check that passes for the wrong reason.
    """
    i = src.find("fn " + name)
    if i < 0:
        return ""
    j = src.find("{", i)
    if j < 0:
        return ""
    depth = 0
    for k in range(j, len(src)):
        if src[k] == "{":
            depth += 1
        elif src[k] == "}":
            depth -= 1
            if depth == 0:
                return src[i:k + 1]
    return src[i:]


def path_features(src):
    """-> what lookup_json_path understands. Absence is the finding, so report booleans."""
    body = _fn_body(src, "lookup_json_path")
    return {"dot_split": "split('.')" in body or 'split(\'.\')' in body,
            "numeric_index": "parse::<usize>()" in body,
            "hash_decoder": "#json" in src or "b64json" in src,
            "alternation": "split('|')" in body or 'alternation' in body.lower()}


def rules_in(assert_obj):
    """Every rule key an `assert` block names, with its field path."""
    out = []
    if not isinstance(assert_obj, dict):
        return out
    for field, rule in assert_obj.items():
        if isinstance(rule, dict):
            for k in rule:
                out.append((field, k))
        elif isinstance(rule, list):
            for r in rule:
                if isinstance(r, dict):
                    for k in r:
                        out.append((field, k))
        else:
            out.append((field, "<scalar>"))
    return out


def check(plan, rules, feats):
    checks, bad_rule, bad_path = [], [], []
    scanned = 0
    for h in (plan.get("test_hooks") or []):
        unit = h.get("unit")
        blocks = []
        for sc in (h.get("connector_scenarios") or []):
            blocks.append((sc.get("scenario"), sc.get("assert")))
        for ov in (h.get("overrides") or []):
            blocks.append((ov.get("scenario") or ov.get("suite"), ov.get("assert")))
        for name, a in blocks:
            for field, rule in rules_in(a):
                scanned += 1
                if rule not in rules and rule != "<scalar>":
                    bad_rule.append({"unit": unit, "scenario": name, "field": field,
                                     "rule": rule, "supported": rules})
                if "#" in str(field) and not feats["hash_decoder"]:
                    bad_path.append({"unit": unit, "scenario": name, "field": field,
                                     "why": "no decoder operator in lookup_json_path"})
                elif "|" in str(field) and not feats["alternation"]:
                    bad_path.append({"unit": unit, "scenario": name, "field": field,
                                     "why": "no alternation in lookup_json_path"})

    checks.append({"id": "DIAL-01", "name": "every_assert_rule_exists_in_harness",
                   "pass": not bad_rule,
                   "message": ("%d rule(s) the harness cannot parse; FieldAssert is "
                               "#[serde(untagged)] so ONE of these fails the whole scenario "
                               "file with ScenarioFileParse" % len(bad_rule)) if bad_rule
                              else "all %d rules are FieldAssert variants" % scanned,
                   "evidence": bad_rule[:10]})
    checks.append({"id": "DIAL-02", "name": "every_assert_path_is_resolvable",
                   "pass": not bad_path,
                   "message": "%d path(s) use operators lookup_json_path does not implement"
                              % len(bad_path) if bad_path else "all paths are dot/index only",
                   "evidence": bad_path[:10]})
    return checks, scanned


def suite_root(repo_root):
    """Mirror `scenario_root()`: the env var wins, else the in-repo default."""
    return os.environ.get("UCS_SCENARIO_ROOT") or os.path.join(repo_root, SUITES)


def disk_scenarios(root, suite):
    """-> (set of scenario ids, error) for one suite, exactly as `load_scenario` resolves it.

    `suite_dir_name` is `suite.replace('/', '_')` and the file is a top-level
    `BTreeMap<String, ScenarioDef>`, so its keys ARE the loadable ids. Returns the error
    rather than raising: a suite the plan names but the harness lacks is a finding, while a
    scenario.json we cannot parse is could-not-evaluate -- those must not collapse together.
    """
    path = os.path.join(root, suite.replace("/", "_"), "scenario.json")
    if not os.path.isdir(os.path.dirname(path)):
        return None, "no global suite directory %s" % os.path.dirname(path)
    try:
        obj = json.load(open(path))
    except Exception as e:
        return None, "UNPARSEABLE %s: %s" % (path, str(e)[:80])
    if not isinstance(obj, dict):
        return None, "UNPARSEABLE %s: top level is %s, not a scenario map" % (path, type(obj).__name__)
    return set(obj), None


def check_loadability(plan, root):
    """DIAL-03/04: every §8 scenario id resolves the way `load_scenario` resolves it.

    The plan's own jq self-check compares `connector_scenarios[]` against the plan's
    `global_scenarios[]` list only. It cannot open the suite file, so an id that matches
    nothing on disk (or collides with something on disk) passes planning and dies later --
    one run escalated it as a PLAN_CONFLICT and spent a plan revision rediscovering it.
    """
    cache, unresolved, shadow, inconclusive = {}, [], [], []

    def ids_for(suite):
        if suite not in cache:
            cache[suite] = disk_scenarios(root, suite)
        return cache[suite]

    seen = 0
    for h in (plan.get("test_hooks") or []):
        unit = h.get("unit")
        # must EXIST on disk: these are references to global scenarios
        for key in ("global_scenarios", "overrides", "waivers"):
            for e in (h.get(key) or []):
                suite, name = e.get("suite"), e.get("scenario")
                if not suite or not name:
                    continue
                ids, err = ids_for(suite)
                if ids is None:
                    (inconclusive if err.startswith("UNPARSEABLE") else unresolved).append(
                        {"unit": unit, "bucket": key, "suite": suite, "scenario": name,
                         "why": err})
                    continue
                seen += 1
                if name not in ids:
                    unresolved.append({
                        "unit": unit, "bucket": key, "suite": suite, "scenario": name,
                        "why": "load_scenario would raise ScenarioNotFound",
                        # nearest by edit distance, not alphabetical: the first five ids in a
                        # 27-scenario suite are useless to whoever has to fix this
                        "did_you_mean": difflib.get_close_matches(name, ids, 5, 0.4)
                                        or sorted(ids)[:5],
                        "legal_outcomes": [
                            "rewrite it as an overrides[] entry against a scenario that does "
                            "exist in this suite",
                            "record it in moved_assertions[] naming where the claim is "
                            "covered instead"]})
        # must NOT exist on disk: connector_specific_scenarios.json is additive
        for e in (h.get("connector_scenarios") or []):
            suite, name = e.get("suite"), e.get("scenario")
            if not suite or not name:
                continue
            ids, err = ids_for(suite)
            if ids is None:
                if err.startswith("UNPARSEABLE"):
                    inconclusive.append({"unit": unit, "bucket": "connector_scenarios",
                                         "suite": suite, "scenario": name, "why": err})
                continue  # a missing suite dir is already reported by the reference pass
            seen += 1
            if name in ids:
                shadow.append({"unit": unit, "suite": suite, "scenario": name,
                               "why": "already a key in the global suite file; the loader "
                                      "rejects a colliding connector scenario outright",
                               "legal_outcomes": [
                                   "move it to overrides[] against that global scenario",
                                   "rename it so it does not collide"]})

    checks = [
        {"id": "DIAL-03", "name": "every_referenced_scenario_loads",
         "pass": not unresolved,
         "message": ("%d referenced scenario(s) do not resolve via load_scenario" % len(unresolved))
                    if unresolved else "all %d referenced ids resolve on disk" % seen,
         "evidence": unresolved[:10]},
        {"id": "DIAL-04", "name": "connector_scenarios_do_not_shadow_disk",
         "pass": not shadow,
         "message": ("%d connector scenario(s) collide with a global scenario on disk "
                     "(the plan's own jq check only compares against global_scenarios[], so "
                     "it cannot see this)" % len(shadow)) if shadow
                    else "no connector scenario collides with the suite file",
         "evidence": shadow[:10]},
    ]
    return checks, inconclusive


def emit(report, out, code):
    if out:
        os.makedirs(os.path.dirname(os.path.abspath(out)) or ".", exist_ok=True)
        tmp = out + ".tmp"
        with open(tmp, "w") as fh:
            json.dump(report, fh, indent=1)
        os.replace(tmp, out)
    json.dump(report, sys.stdout, indent=1)
    sys.stdout.write("\n")
    return code


def main():
    ap = argparse.ArgumentParser(description="Validate plan §8 assertions against the harness")
    ap.add_argument("--plan", required=True)
    ap.add_argument("--repo-root", default=".")
    ap.add_argument("--out")
    a = ap.parse_args()

    tp = os.path.join(a.repo_root, TYPES)
    ap_ = os.path.join(a.repo_root, ASSERT)
    for p in (tp, ap_):
        if not os.path.exists(p):
            return emit({"pass": False, "checks": [], "inconclusive": [],
                         "needs_human": ["harness source not found: %s -- cannot derive the "
                                         "supported dialect, and hardcoding it here is exactly "
                                         "what this gate refuses to do" % p],
                         "summary": {}}, a.out, 2)
    rules = supported_rules(open(tp).read())
    if not rules:
        return emit({"pass": False, "checks": [], "inconclusive": [],
                     "needs_human": ["could not parse `pub enum FieldAssert` from %s" % TYPES],
                     "summary": {}}, a.out, 2)
    feats = path_features(open(ap_).read())
    try:
        plan = json.load(open(a.plan))
    except Exception as e:
        return emit({"pass": False, "checks": [], "inconclusive": [],
                     "needs_human": ["plan unreadable: %s" % str(e)[:80]], "summary": {}}, a.out, 2)

    checks, scanned = check(plan, rules, feats)
    load_checks, inconclusive = check_loadability(plan, suite_root(a.repo_root))
    checks += load_checks
    ok = all(c["pass"] for c in checks)
    # an unparseable suite file means we could not evaluate loadability -- reporting that as a
    # pass would be the same "passes for the wrong reason" failure the brace matcher above fixes
    if inconclusive:
        return emit({"pass": False, "checks": checks, "inconclusive": inconclusive,
                     "needs_human": ["could not read a global suite file; loadability is "
                                     "unevaluated for the ids listed in inconclusive[]"],
                     "summary": {"rules_scanned": scanned, "supported_rules": rules,
                                 "path_features": feats}}, a.out, 2)
    return emit({"pass": ok, "checks": checks, "inconclusive": [], "needs_human": [],
                 "summary": {"rules_scanned": scanned, "supported_rules": rules,
                             "path_features": feats,
                             "failed": sum(1 for c in checks if not c["pass"])}}, a.out,
                0 if ok else 1)


def _replay():
    TYPES_SRC = """
    #[derive(Debug, Clone, Deserialize)]
    #[serde(untagged)]
    pub enum FieldAssert {
        MustExist { must_exist: bool },
        MustNotExist { must_not_exist: bool },
        Equals { equals: Value },
        OneOf { one_of: Vec<Value> },
        Contains { contains: String },
        Echo { echo: String },
    }
    """
    ASSERT_SRC = """
    pub fn lookup_json_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
        for segment in path.split('.') {
            current = if let Ok(index) = segment.parse::<usize>() { current.get(index)? }
                      else { lookup_object_segment(current, segment)? };
        }
        Some(current)
    }
    """
    rules = supported_rules(TYPES_SRC)
    assert rules == ["contains", "echo", "equals", "must_exist", "must_not_exist", "one_of"], rules
    feats = path_features(ASSERT_SRC)
    assert feats["dot_split"] and feats["numeric_index"], feats
    assert not feats["hash_decoder"] and not feats["alternation"], feats
    # the trap: a leading guard clause must not truncate the body (a lazy regex stopped at
    # the guard's closing brace and reported every feature absent -- passing for the wrong
    # reason). This fixture reproduces the real function's shape.
    GUARDED = """
    pub fn lookup_json_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
        if path.is_empty() {
            return Some(value);
        }
        for segment in path.split('.') {
            current = if let Ok(index) = segment.parse::<usize>() { current.get(index)? }
                      else { lookup_object_segment(current, segment)? };
        }
        Some(current)
    }
    """
    g = path_features(GUARDED)
    assert g["dot_split"] and g["numeric_index"], ("guard clause truncated the body", g)

    good = {"test_hooks": [{"unit": "Refunds", "connector_scenarios": [
        {"scenario": "r1", "assert": {"status": {"equals": "CLO"},
                                      "amount": {"must_exist": True}}}]}]}
    c, n = check(good, rules, feats)
    assert all(x["pass"] for x in c) and n == 2, (c, n)

    # the real defect: rule names the harness has no variant for
    bad = {"test_hooks": [{"unit": "Payments", "connector_scenarios": [
        {"scenario": "p1", "assert": {"status": {"not_one_of": ["X"]},
                                      "body": {"not_contains_text": "ship"}}}]}]}
    c, _ = check(bad, rules, feats)
    d1 = [x for x in c if x["id"] == "DIAL-01"][0]
    assert not d1["pass"] and len(d1["evidence"]) == 2, d1
    assert "ScenarioFileParse" in d1["message"], d1["message"]

    # path operators that lookup_json_path cannot resolve
    badp = {"test_hooks": [{"unit": "ThreeDS", "overrides": [
        {"scenario": "t1", "assert": {"body#json.eci": {"equals": "05"},
                                      "a|b": {"must_exist": True}}}]}]}
    c, _ = check(badp, rules, feats)
    d2 = [x for x in c if x["id"] == "DIAL-02"][0]
    assert not d2["pass"] and len(d2["evidence"]) == 2, d2

    # negation asymmetry is the trap worth naming: existence can be negated, matching cannot
    assert "must_not_exist" in rules and "not_equals" not in rules and "not_one_of" not in rules

    # a harness that GAINS a rule must make the gate pass without editing this file
    rules2 = supported_rules(TYPES_SRC.replace(
        "Echo { echo: String },", "Echo { echo: String },\n NotOneOf { not_one_of: Vec<Value> },"))
    c, _ = check(bad, rules2, feats)
    assert len([e for e in [x for x in c if x["id"] == "DIAL-01"][0]["evidence"]]) == 1, \
        "adding the variant to the harness must drop it from the findings"

    # --- loadability (DIAL-03/04) against a fixture suite tree -------------------------
    with tempfile.TemporaryDirectory() as root:
        d = os.path.join(root, "PaymentService_Authorize")
        os.makedirs(d)
        with open(os.path.join(d, "scenario.json"), "w") as fh:
            json.dump({"no3ds_auto_capture_credit_card": {}, "threeds_manual_capture_credit_card": {}}, fh)

        ok_plan = {"test_hooks": [{"unit": "Payments",
            "global_scenarios": [{"suite": "PaymentService/Authorize",
                                  "scenario": "no3ds_auto_capture_credit_card"}],
            "connector_scenarios": [{"suite": "PaymentService/Authorize",
                                     "scenario": "rapyd_avs_required_reject"}]}]}
        c, inc = check_loadability(ok_plan, root)
        assert all(x["pass"] for x in c) and not inc, (c, inc)

        # ScenarioNotFound: the id resolves to nothing, and the fix names both legal outcomes
        miss = {"test_hooks": [{"unit": "Payments", "global_scenarios": [
            {"suite": "PaymentService/Authorize", "scenario": "o-scen-r1-Payments"}]}]}
        c, _ = check_loadability(miss, root)
        d3 = [x for x in c if x["id"] == "DIAL-03"][0]
        assert not d3["pass"] and len(d3["evidence"]) == 1, d3
        assert "ScenarioNotFound" in d3["evidence"][0]["why"], d3
        assert len(d3["evidence"][0]["legal_outcomes"]) == 2, d3
        assert d3["evidence"][0]["did_you_mean"], "must suggest ids that do exist"
        near = {"test_hooks": [{"unit": "Payments", "global_scenarios": [
            {"suite": "PaymentService/Authorize",
             "scenario": "threeds_manual_capture_credit_cards"}]}]}
        ev = [x for x in check_loadability(near, root)[0]
              if x["id"] == "DIAL-03"][0]["evidence"][0]
        assert ev["did_you_mean"][0] == "threeds_manual_capture_credit_card", \
            ("a near miss must suggest the near match first", ev["did_you_mean"])

        # a suite the harness has no global suite for
        nosuite = {"test_hooks": [{"unit": "Mandates", "overrides": [
            {"suite": "PaymentService/Imaginary", "scenario": "x"}]}]}
        c, _ = check_loadability(nosuite, root)
        assert not [x for x in c if x["id"] == "DIAL-03"][0]["pass"]

        # THE GAP the plan's jq cannot see: a collision with a scenario on DISK, while the
        # plan's own global_scenarios[] is empty so the jq shadow check finds nothing.
        shadow = {"test_hooks": [{"unit": "Payments", "global_scenarios": [],
            "connector_scenarios": [{"suite": "PaymentService/Authorize",
                                     "scenario": "threeds_manual_capture_credit_card"}]}]}
        c, _ = check_loadability(shadow, root)
        d4 = [x for x in c if x["id"] == "DIAL-04"][0]
        assert not d4["pass"] and len(d4["evidence"]) == 1, d4
        assert [x for x in c if x["id"] == "DIAL-03"][0]["pass"], "a collision is not an absence"

        # a waiver for a scenario that does not exist is dead config, not a silent pass
        w = {"test_hooks": [{"unit": "Refunds", "waivers": [
            {"suite": "PaymentService/Authorize", "scenario": "never_existed",
             "reason": "PM_NOT_OFFERED: docs - n/a"}]}]}
        assert not [x for x in check_loadability(w, root)[0] if x["id"] == "DIAL-03"][0]["pass"]

        # an unparseable suite file is could-not-evaluate, never a pass
        with open(os.path.join(d, "scenario.json"), "w") as fh:
            fh.write("{ not json")
        c, inc = check_loadability(ok_plan, root)
        assert inc and all(i["why"].startswith("UNPARSEABLE") for i in inc), inc
        assert all(x["pass"] for x in c), "an unreadable file must not manufacture findings"

    print("replay OK: the supported set is parsed from the harness enum (not hardcoded), an "
          "unknown rule fails DIAL-01 naming the untagged ScenarioFileParse blast radius, "
          "#json/alternation paths fail DIAL-02, the negation asymmetry is asserted, and a rule "
          "added to the harness stops being a finding with no edit here; DIAL-03 does the literal "
          "load_scenario lookup and names both legal outcomes, DIAL-04 catches the on-disk "
          "collision the plan's jq cannot see, and an unparseable suite file is "
          "could-not-evaluate rather than a pass")
    return 0


if __name__ == "__main__":
    sys.exit(_replay() if sys.argv[1:2] == ["--replay"] else main())
