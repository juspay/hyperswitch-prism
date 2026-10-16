#!/usr/bin/env python3
"""Validate the per-item codegen brief and the gate fix log.

`grace/workflow/2.3b_codegen_unit.md` Phase 1 writes `code/<NN>-<unit_fs>.brief.json` so that
Phase 2 can implement one plan item while reading nothing else. That only holds if the brief
actually carries content. The failure this gate exists to catch is a brief that looks complete
and is not -- an entry whose `types[]` names a file instead of quoting a declaration, or whose
`current` does not contain the code the item's action describes. Phase 2 would then either
implement against a guess or go read the file, and the second is the measurable one
(`brief_miss` in the decisions file).

It also checks `code/<NN>-<unit_fs>.fixlog.jsonl`, which is what makes a gate iteration a
standalone spawn: the iteration count, the 3-strike rule and "never rerun without changing code
first" all read that file rather than a conversation.

Contract matches the gates beside it (refusal_gate.py, test_author_gate.py, threeds_gate.py):
stdlib only, exit 0 pass / 1 fail / 2 could-not-evaluate, a JSON report carrying `summary`,
`inconclusive[]` and `needs_human[]`, and `--replay` for a self-check.
"""
import argparse, json, os, sys, subprocess, tempfile

MAX_CURRENT_LINES = 20          # 2.6e_rca.md fix[].current: "<verbatim, <=20 lines, redacted>"
MAX_EXCERPT_LINES = 20
ITEM_FIELDS = ("item_id", "file", "line_start", "line_end", "current")
FIXLOG_FIELDS = ("iteration", "error", "file", "change")
STRIKE = 3                      # 2.3b Phase 4 step 3 / 2.3_codegen.md "3-strike rule"

# A signature must look like a declaration. A path, a heading or a bare name is the exact
# "wrote a pointer where the content belongs" failure the brief exists to prevent.
DECL_TOKENS = ("struct", "enum", "fn", "trait", "impl", "type", "const", "macro_rules",
               "pub ", "message ", "service ", "{", "(")


def _looks_like_path(v):
    v = (v or "").strip()
    if not v or "\n" in v:
        return False
    return ("/" in v and " " not in v) or v.endswith((".rs", ".md", ".proto", ".json"))


def check_brief(doc, plan_item_ids=None, foundation_ids=None):
    """-> (checks, inconclusive, needs_human)."""
    checks, inc, human = [], [], []

    def add(cid, name, ok, msg, evidence=None):
        checks.append({"id": cid, "name": name, "pass": bool(ok), "message": msg,
                       "evidence": evidence or []})

    if not isinstance(doc, dict):
        inc.append("brief is not a JSON object")
        return checks, inc, human

    schema = doc.get("schema")
    items = doc.get("items")
    add("BRIEF-01", "schema_and_shape",
        schema == "grace-brief/1" and isinstance(items, list) and bool(doc.get("unit")),
        "schema=%r unit=%r items=%s" % (schema, doc.get("unit"),
                                        len(items) if isinstance(items, list) else type(items).__name__))
    if not isinstance(items, list):
        return checks, inc, human

    malformed = [i for i, it in enumerate(items) if not isinstance(it, dict)]
    add("BRIEF-08", "items_are_objects", not malformed,
        "non-object entries: %s" % malformed)
    if malformed:
        return checks, inc, human
    invalid_ids = [i for i, it in enumerate(items)
                   if not isinstance(it.get("item_id"), str) or not it["item_id"].strip()]
    add("BRIEF-09", "item_ids_are_nonempty_strings", not invalid_ids,
        "invalid ids at entries: %s" % invalid_ids)
    if invalid_ids:
        return checks, inc, human

    malformed_fields = [i for i, it in enumerate(items)
                        if not isinstance(it.get("file"), str)
                        or not isinstance(it.get("current"), str)
                        or not isinstance(it.get("line_start"), int)
                        or isinstance(it.get("line_start"), bool)
                        or not isinstance(it.get("line_end"), int)
                        or isinstance(it.get("line_end"), bool)
                        or it.get("line_start", 0) < 1
                        or it.get("line_end", 0) < it.get("line_start", 0)
                        or not isinstance(it.get("types", []), list)
                        or any(not isinstance(t, dict) or not isinstance(t.get("signature"), str)
                               for t in it.get("types", []))]
    add("BRIEF-10", "field_shapes", not malformed_fields,
        "malformed fields at entries: %s" % malformed_fields)
    if malformed_fields:
        return checks, inc, human

    missing = [{"i": i, "absent": [f for f in ITEM_FIELDS if not it.get(f)]}
               for i, it in enumerate(items) if isinstance(it, dict)
               and [f for f in ITEM_FIELDS if not it.get(f)]]
    add("BRIEF-02", "required_fields_present", not missing,
        "%d of %d entries incomplete" % (len(missing), len(items)), missing[:8])

    over = [{"item_id": it.get("item_id"), "lines": len(str(it.get("current", "")).splitlines())}
            for it in items if isinstance(it, dict)
            and len(str(it.get("current", "")).splitlines()) > MAX_CURRENT_LINES]
    add("BRIEF-03", "current_within_cap", not over,
        "%d entries exceed %d lines" % (len(over), MAX_CURRENT_LINES), over[:8])

    # the real one: a signature that is a pointer, not a declaration
    bad = []
    for it in items:
        if not isinstance(it, dict):
            continue
        for t in (it.get("types") or []):
            sig = (t or {}).get("signature")
            if not sig or _looks_like_path(sig) or not any(k in sig for k in DECL_TOKENS):
                bad.append({"item_id": it.get("item_id"), "type": (t or {}).get("name"),
                            "signature": (sig or "")[:80]})
    add("BRIEF-04", "signatures_are_declarations_not_paths", not bad,
        "%d type entries carry a pointer or an empty signature" % len(bad), bad[:8])

    longx = [{"item_id": it.get("item_id"),
              "lines": len(str(it.get("guide_excerpt", "")).splitlines())}
             for it in items if isinstance(it, dict)
             and len(str(it.get("guide_excerpt", "")).splitlines()) > MAX_EXCERPT_LINES]
    add("BRIEF-05", "guide_excerpt_within_cap", not longx,
        "%d excerpts exceed %d lines" % (len(longx), MAX_EXCERPT_LINES), longx[:8])

    ids = [it.get("item_id") for it in items if isinstance(it, dict)]
    dupes = sorted({i for i in ids if ids.count(i) > 1})
    add("BRIEF-06", "item_ids_unique", not dupes, "duplicate item_id: %s" % (dupes or "none"), dupes[:8])

    if plan_item_ids is None:
        inc.append("no plan item ids supplied: coverage against plan not checked")
    else:
        allowed = set(plan_item_ids) | set(foundation_ids or ())
        extra = sorted(set(ids) - allowed)
        absent = sorted(set(plan_item_ids) - set(ids))   # foundation absence is not a drop
        add("BRIEF-07", "items_match_plan", not extra and not absent,
            "invented=%s dropped=%s" % (extra or "none", absent or "none"),
            [{"invented": extra[:8], "dropped": absent[:8]}])
    return checks, inc, human


def check_fixlog(lines):
    checks, inc = [], []

    def add(cid, name, ok, msg, evidence=None):
        checks.append({"id": cid, "name": name, "pass": bool(ok), "message": msg,
                       "evidence": evidence or []})

    recs, broken = [], []
    for n, ln in enumerate(lines, 1):
        ln = ln.strip()
        if not ln:
            continue
        try:
            recs.append(json.loads(ln))
        except Exception as e:
            broken.append({"line": n, "error": str(e)[:60]})
    add("FIXLOG-01", "one_json_object_per_line", not broken,
        "%d unparsable lines" % len(broken), broken[:8])

    bad = [{"i": i, "absent": [f for f in FIXLOG_FIELDS if r.get(f) in (None, "")]}
           for i, r in enumerate(recs) if [f for f in FIXLOG_FIELDS if r.get(f) in (None, "")]]
    add("FIXLOG-02", "all_four_fields_filled", not bad,
        "%d of %d entries incomplete (2.3b: 'If you cannot fill all four -> FAILED')"
        % (len(bad), len(recs)), bad[:8])

    errs = [str(r.get("error", "")) for r in recs]
    struck = sorted({e for e in errs if errs.count(e) >= STRIKE})
    add("FIXLOG-03", "no_unreported_three_strike", not struck,
        "%d error(s) at or past %d occurrences: the unit should have returned FAILED"
        % (len(struck), STRIKE), [e[:70] for e in struck][:5])
    return checks, inc, recs


def emit(report, out, code):
    if out:
        os.makedirs(os.path.dirname(os.path.abspath(out)) or ".", exist_ok=True)
        tmp = out + ".tmp"
        with open(tmp, "w") as fh:
            json.dump(report, fh, indent=1, sort_keys=False)
        os.replace(tmp, out)
    json.dump(report, sys.stdout, indent=1)
    sys.stdout.write("\n")
    return code


def main():
    ap = argparse.ArgumentParser(description="Validate a codegen brief and its gate fix log")
    ap.add_argument("--brief")
    ap.add_argument("--fixlog")
    ap.add_argument("--plan", help="plan.json, to check the brief covers exactly its unit's items")
    ap.add_argument("--unit", help="with --plan: the unit whose items to expect")
    ap.add_argument("--out")
    a = ap.parse_args()

    if not a.brief and not a.fixlog:
        return emit({"pass": False, "checks": [], "inconclusive": ["nothing to check"],
                     "needs_human": [], "summary": {}}, a.out, 2)

    checks, inc, human = [], [], []
    plan_ids = None
    foundation = None     # must exist even if the plan fails to load, below
    if a.plan:
        try:
            pd = json.load(open(a.plan))
            if not isinstance(pd, dict):
                raise ValueError("plan must be an object")
            units = pd.get("units", pd.get("code_units", []))
            hs_items = pd.get("hs_changes", [])
            if not isinstance(units, list) or not isinstance(hs_items, list):
                raise ValueError("units and hs_changes must be arrays")
            all_items = []
            selected = []
            for unit in units:
                if not isinstance(unit, dict) or not isinstance(unit.get("unit"), str):
                    raise ValueError("malformed unit")
                entries = unit.get("items", [])
                if not isinstance(entries, list):
                    raise ValueError("unit items must be an array")
                for it in entries:
                    if not isinstance(it, dict) or it.get("unit", unit["unit"]) != unit["unit"]:
                        raise ValueError("malformed or cross-unit plan item")
                    all_items.append(it)
                    if a.unit is None or unit["unit"] == a.unit:
                        selected.append(it)
            for it in hs_items:
                if not isinstance(it, dict) or it.get("unit", "__hs__") != "__hs__":
                    raise ValueError("malformed or cross-unit HS item")
                all_items.append(it)
                if a.unit is None or a.unit == "__hs__":
                    selected.append(it)
            known_units = {unit["unit"] for unit in units}
            if hs_items:
                known_units.add("__hs__")
            if a.unit is not None and a.unit not in known_units:
                raise ValueError("unknown selected unit")
            foundation_entries = pd.get("foundation", [])
            if not isinstance(foundation_entries, list):
                raise ValueError("foundation must be an array")
            all_items += foundation_entries
            ids = []
            for it in all_items:
                if not isinstance(it, dict) or not isinstance(it.get("id"), str) or not it["id"].strip():
                    raise ValueError("malformed plan item id")
                ids.append(it["id"])
            if len(ids) != len(set(ids)):
                raise ValueError("duplicate plan item id")
            plan_ids = [it["id"] for it in selected if not it.get("withdrawn")]
            # plan §2 foundation items are carried by the FIRST planned unit's brief
            # (2.3b RULES 1: "NEW implements plan §4[UNIT] (+ §2 for the first unit)"),
            # so they are legitimate brief entries, not invented ones.
            foundation = [it.get("id") for it in (pd.get("foundation") or [])
                          if not it.get("withdrawn")]
            # Foundation declarations belong to the first canonical planned code
            # unit. Aggregate validation (no --unit) may include them as before.
            ordinary_units = [u["unit"] for u in units
                              if u["unit"] not in ("__hs__", "__finalize__")]
            if "order" in pd:
                order = pd["order"]
                if not isinstance(order, list) or any(
                    not isinstance(row, dict)
                    or not isinstance(row.get("unit"), str)
                    or isinstance(row.get("seq"), bool)
                    or not ((isinstance(row.get("seq"), int) and row["seq"] >= 0)
                            or (isinstance(row.get("seq"), str) and row["seq"].isascii()
                                and row["seq"].isdigit()))
                    for row in order
                ):
                    raise ValueError("malformed canonical unit order")
                ordinals = [int(row["seq"]) for row in order]
                if len(ordinals) != len(set(ordinals)):
                    raise ValueError("duplicate canonical unit ordinal")
                first_owner = next((row["unit"] for row in sorted(order, key=lambda row: int(row["seq"]))
                                    if row.get("status") == "planned"
                                    and row["unit"] in ordinary_units), None)
            else:
                # Older plans without order use their canonical unit declaration
                # order; never grant foundation ownership to HS/finalization.
                first_owner = next(iter(ordinary_units), None)
            if a.unit is not None and a.unit != first_owner:
                foundation = []
        except Exception as e:
            return emit({"pass": False, "checks": [], "inconclusive": [],
                         "needs_human": ["plan invalid: %s" % str(e)[:100]],
                         "summary": {}}, a.out, 2)

    n_items = 0
    if a.brief:
        try:
            doc = json.load(open(a.brief))
        except Exception as e:
            return emit({"pass": False, "checks": [], "inconclusive": [],
                         "needs_human": ["brief unreadable: %s" % str(e)[:80]],
                         "summary": {}}, a.out, 2)
        if a.unit and (not isinstance(doc, dict) or doc.get("unit") != a.unit):
            return emit({"pass": False, "checks": [], "inconclusive": [],
                         "needs_human": ["brief unit does not match --unit"],
                         "summary": {}}, a.out, 2)
        c, i, h = check_brief(doc, plan_ids, foundation)
        checks += c; inc += i; human += h
        n_items = len(doc.get("items") or [])

    n_iter = 0
    if a.fixlog:
        if not os.path.exists(a.fixlog):
            inc.append("no fixlog: the gate ran zero iterations")
        else:
            c, i, recs = check_fixlog(open(a.fixlog).read().splitlines())
            checks += c; inc += i; n_iter = len(recs)

    ok = all(c["pass"] for c in checks)
    report = {"pass": ok, "checks": checks, "inconclusive": inc, "needs_human": human,
              "summary": {"items": n_items, "gate_iterations": n_iter,
                          "checks": len(checks), "failed": sum(1 for c in checks if not c["pass"])}}
    if human:
        return emit(report, a.out, 2)
    return emit(report, a.out, 0 if ok else 1)


def _replay():
    good = {"schema": "grace-brief/1", "unit": "Refund", "impl_type": "flow_completion",
            "items": [{"item_id": "P-Refund-01", "file": "x/t.rs", "line_start": 10,
                       "line_end": 14, "current": "let a = 1;\nlet b = 2;",
                       "types": [{"name": "RefundsData", "file": "c.rs", "line_start": 1,
                                  "line_end": 9,
                                  "signature": "pub struct RefundsData { pub amount: MinorUnit }"}],
                       "guide_excerpt": "Refund amount is minor units.",
                       "notes": "enum struct-variant"}],
            "deviations": []}
    c, i, h = check_brief(good, ["P-Refund-01"])
    assert all(x["pass"] for x in c), [x for x in c if not x["pass"]]
    assert not i and not h

    # the central trap: a signature that is a path, not a declaration
    bad = json.loads(json.dumps(good))
    bad["items"][0]["types"][0]["signature"] = "crates/types-traits/domain_types/src/connector_types.rs"
    c, _, _ = check_brief(bad, ["P-Refund-01"])
    assert not [x for x in c if x["id"] == "BRIEF-04"][0]["pass"], "a path must fail BRIEF-04"
    # ...and an empty one
    bad["items"][0]["types"][0]["signature"] = ""
    c, _, _ = check_brief(bad, ["P-Refund-01"])
    assert not [x for x in c if x["id"] == "BRIEF-04"][0]["pass"]

    # current over the fix[] cap of 20 lines
    big = json.loads(json.dumps(good))
    big["items"][0]["current"] = "\n".join("l%d" % n for n in range(25))
    c, _, _ = check_brief(big, ["P-Refund-01"])
    assert not [x for x in c if x["id"] == "BRIEF-03"][0]["pass"]

    # coverage both ways: an invented item and a dropped one
    c, _, _ = check_brief(good, ["P-Refund-01", "P-Refund-02"])
    assert not [x for x in c if x["id"] == "BRIEF-07"][0]["pass"], "a dropped item must fail"
    c, _, _ = check_brief(good, ["P-Refund-99"])
    assert not [x for x in c if x["id"] == "BRIEF-07"][0]["pass"], "an invented item must fail"
    # plan §2 foundation items in the first unit's brief are legitimate, not invented --
    # this is what the live run caught: BRIEF-07 reported invented=[P-foundation-01..07]
    foundation = json.loads(json.dumps(good))
    foundation["items"].append(dict(good["items"][0], item_id="P-foundation-01"))
    c, _, _ = check_brief(foundation, ["P-Refund-01"], ["P-foundation-01"])
    assert [x for x in c if x["id"] == "BRIEF-07"][0]["pass"], "foundation ids must be allowed"
    # ...but a genuinely invented id still fails even with foundation allowed
    inv = json.loads(json.dumps(good))
    inv["items"].append(dict(good["items"][0], item_id="P-Nonsense-99"))
    c, _, _ = check_brief(inv, ["P-Refund-01"], ["P-foundation-01"])
    assert not [x for x in c if x["id"] == "BRIEF-07"][0]["pass"], "invented id must still fail"
    # ...and an absent foundation id is NOT a drop (only non-first units omit them)
    c, _, _ = check_brief(good, ["P-Refund-01"], ["P-foundation-01"])
    assert [x for x in c if x["id"] == "BRIEF-07"][0]["pass"], "absent foundation is not a drop"

    # no plan supplied -> inconclusive, never a silent pass
    c, i, _ = check_brief(good, None)
    assert i and not any(x["id"] == "BRIEF-07" for x in c)

    # duplicate ids
    dup = json.loads(json.dumps(good))
    dup["items"].append(dict(dup["items"][0]))
    c, _, _ = check_brief(dup, ["P-Refund-01"])
    assert not [x for x in c if x["id"] == "BRIEF-06"][0]["pass"]

    # fixlog
    ok_lines = ['{"iteration":1,"error":"E0277 x","file":"a.rs","change":"added From"}',
                '{"iteration":2,"error":"E0308 y","file":"a.rs","change":"fixed type"}']
    c, _, recs = check_fixlog(ok_lines)
    assert all(x["pass"] for x in c) and len(recs) == 2
    c, _, _ = check_fixlog(ok_lines + ["{not json"])
    assert not [x for x in c if x["id"] == "FIXLOG-01"][0]["pass"]
    c, _, _ = check_fixlog(['{"iteration":1,"error":"E1","file":"a.rs"}'])
    assert not [x for x in c if x["id"] == "FIXLOG-02"][0]["pass"], "a missing field must fail"
    same = ['{"iteration":%d,"error":"E0277 same","file":"a.rs","change":"c"}' % n for n in (1, 2, 3)]
    c, _, _ = check_fixlog(same)
    assert not [x for x in c if x["id"] == "FIXLOG-03"][0]["pass"], "3 strikes must fail"
    c, _, _ = check_fixlog(same[:2])
    assert [x for x in c if x["id"] == "FIXLOG-03"][0]["pass"], "2 strikes must pass"
    # blank lines are not records
    c, _, recs = check_fixlog(["", ok_lines[0], "  "])
    assert len(recs) == 1 and all(x["pass"] for x in c)

    # Exercise actual CLI selection and fail-closed validation, not only helpers.
    with tempfile.TemporaryDirectory(prefix="grace-brief-replay-") as directory:
        plan_path = os.path.join(directory, "plan.json")
        brief_path = os.path.join(directory, "brief.json")
        hs = json.loads(json.dumps(good))
        hs["unit"] = "__hs__"
        hs["items"][0]["item_id"] = "P-__hs__-01"
        plan = {"units": [{"unit": "Refund", "items": [{"id": "P-Refund-01"}]}],
                "hs_changes": [{"id": "P-__hs__-01", "unit": "__hs__"}], "foundation": []}

        def cli(brief, document, expected, selected_unit="__hs__"):
            with open(brief_path, "w") as handle:
                json.dump(brief, handle)
            with open(plan_path, "w") as handle:
                json.dump(document, handle)
            result = subprocess.run([sys.executable, os.path.abspath(__file__), "--brief",
                                     brief_path, "--plan", plan_path]
                                    + (["--unit", selected_unit] if selected_unit is not None else []),
                                    capture_output=True, text=True, check=False)
            report = json.loads(result.stdout)
            assert result.returncode == expected, report
            assert report["pass"] == (expected == 0), report

        cli(hs, plan, 0)
        scoped = json.loads(json.dumps(plan))
        scoped["foundation"] = [{"id": "P-foundation-01"}]
        scoped["units"].append({"unit": "Capture", "items": [{"id": "P-Capture-01"}]})
        scoped["order"] = [{"seq": 2, "unit": "Capture", "status": "planned"},
                           {"seq": 1, "unit": "Refund", "status": "planned"}]
        foundation_item = dict(good["items"][0], item_id="P-foundation-01")
        first = json.loads(json.dumps(good))
        first["items"].append(foundation_item)
        cli(first, scoped, 0, "Refund")
        numeric_strings = json.loads(json.dumps(scoped))
        for row in numeric_strings["order"]:
            row["seq"] = "%02d" % row["seq"]
        cli(first, numeric_strings, 0, "Refund")
        malformed_order = json.loads(json.dumps(scoped))
        malformed_order["order"][0]["seq"] = "first"
        cli(first, malformed_order, 2, "Refund")
        for invalid_seq in (-1, True, 1.5):
            invalid_order = json.loads(json.dumps(scoped))
            invalid_order["order"][0]["seq"] = invalid_seq
            cli(first, invalid_order, 2, "Refund")
        duplicate_order = json.loads(json.dumps(scoped))
        duplicate_order["order"][0]["seq"] = "00"
        duplicate_order["order"][1]["seq"] = 0
        cli(first, duplicate_order, 2, "Refund")
        later = json.loads(json.dumps(good))
        later["unit"] = "Capture"
        later["items"][0]["item_id"] = "P-Capture-01"
        cli(later, scoped, 0, "Capture")
        later["items"].append(foundation_item)
        cli(later, scoped, 1, "Capture")
        wrong_foundation = json.loads(json.dumps(hs))
        wrong_foundation["items"].append(foundation_item)
        cli(wrong_foundation, scoped, 1)
        aggregate = json.loads(json.dumps(first))
        aggregate["items"].append(dict(good["items"][0], item_id="P-Capture-01"))
        aggregate["items"].append(dict(good["items"][0], item_id="P-__hs__-01"))
        cli(aggregate, scoped, 0, None)
        empty = dict(hs, unit="MisspelledUnit", items=[])
        with open(brief_path, "w") as handle:
            json.dump(empty, handle)
        with open(plan_path, "w") as handle:
            json.dump(plan, handle)
        result = subprocess.run([sys.executable, os.path.abspath(__file__), "--brief",
                                 brief_path, "--plan", plan_path, "--unit", "MisspelledUnit"],
                                capture_output=True, text=True, check=False)
        assert result.returncode == 2 and not json.loads(result.stdout)["pass"]
        malformed = json.loads(json.dumps(hs))
        malformed["items"][0]["types"][0]["signature"] = 42
        cli(malformed, plan, 1)
        for invalid in ("P-invented-99", "P-Refund-01"):
            copy = json.loads(json.dumps(hs))
            copy["items"][0]["item_id"] = invalid
            cli(copy, plan, 1)
        copy = json.loads(json.dumps(hs))
        copy["items"].append(dict(copy["items"][0]))
        cli(copy, plan, 1)
        copy = json.loads(json.dumps(hs))
        copy["items"] = ["malformed"]
        cli(copy, plan, 1)
        copy = json.loads(json.dumps(plan))
        copy["hs_changes"].append(dict(copy["hs_changes"][0]))
        cli(hs, copy, 2)
        copy = json.loads(json.dumps(plan))
        copy["hs_changes"][0]["unit"] = "Refund"
        cli(hs, copy, 2)
        copy = json.loads(json.dumps(hs))
        copy["unit"] = "Refund"
        cli(copy, plan, 2)

    print("replay OK: a path or empty signature fails BRIEF-04, current obeys the fix[] 20-line "
          "cap, plan coverage fails on both invented and dropped items and is inconclusive "
          "without a plan, and the fixlog enforces four filled fields and 3 strikes but not 2")
    return 0


if __name__ == "__main__":
    sys.exit(_replay() if sys.argv[1:2] == ["--replay"] else main())
