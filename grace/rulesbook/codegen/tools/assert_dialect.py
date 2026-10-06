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

The supported set is read FROM THE HARNESS SOURCE, never hardcoded here, so the gate tracks
the harness as it changes (the precedent is `2.3b_codegen_unit.md`'s awk extraction over
`review_themes.md`: "Do not hardcode the check set here. It is read from the guide").

Contract matches the gates beside it: stdlib only, exit 0 pass / 1 fail / 2 could-not-evaluate,
a JSON report with `summary` / `inconclusive[]` / `needs_human[]`, and `--replay`.
"""
import argparse, json, os, re, sys

HARNESS = "crates/internal/integration-tests/src/harness"
TYPES = HARNESS + "/scenario_types.rs"
ASSERT = HARNESS + "/scenario_assert.rs"


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
    ok = all(c["pass"] for c in checks)
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

    print("replay OK: the supported set is parsed from the harness enum (not hardcoded), an "
          "unknown rule fails DIAL-01 naming the untagged ScenarioFileParse blast radius, "
          "#json/alternation paths fail DIAL-02, the negation asymmetry is asserted, and a rule "
          "added to the harness stops being a finding with no edit here")
    return 0


if __name__ == "__main__":
    sys.exit(_replay() if sys.argv[1:2] == ["--replay"] else main())
