#!/usr/bin/env python3
"""Request-shape gate: the connector's declared field domain vs what we emit.

`refusal_gate.py` CAP-02 checks our side of the wire -- that a field the plan says
this run added really appears in the probed request body. It cannot check the other
side: whether the connector *accepts* that field on the `payment_method.type` we
pair it with. A field can be emitted perfectly and still be rejected.

That gap has a measured cost. In run rapyd-854c2a the connector returned
`400 UNKNOWN_PAYMENT_METHOD_FIELD - [AVS_REQUIRED]` on every one of 20 failing rows,
because the request carried `payment_method_options.avs_required` with
`payment_method.type = in_amex_card` -- a type whose declared option domain is
`{3d_required, tavv, expiration_action}`. The same run later rejected
`[3D_VERSION]` for the same reason. Both facts were available from
`GET /v1/payment_methods/in_amex_card/required_fields` before any code was written.

  SHP-00  no capability table for this connector -> not applicable, pass.
          The gate is inert until someone probes; it never invents a domain.
  SHP-01  an emitted `payment_method_options` field must be in the probed domain of
          every `payment_method.type` the connector can emit alongside it.
          Fails only on a POSITIVE probed fact (a type that has a row excluding the
          field), the same discipline as refusal_gate REF-01/CAP-02.
  SHP-02  an emittable type with no probed row is reported for a human, not failed.
          Abstaining is the point: a confident wrong table is worse than no table.

Capability table: grace/rulesbook/codegen/references/<connector>/capabilities.json

  {"connector": "rapyd",
   "source": "GET /v1/payment_methods/{type}/required_fields",
   "types": {"in_amex_card": {"payment_method_options": ["3d_required", "tavv",
                                                        "expiration_action"],
                              "probed_at": "...", "evidence_ref": "..."}}}

A row is evidence, not a belief: it carries `probed_at` and an `evidence_ref` a later
stage can re-run. Re-probe on mismatch rather than trusting it -- the avs_required
defect originated in a *previous* run's reference doc that was trusted and never
re-probed.

Exit codes: 0 pass, 1 fail, 2 could not evaluate (treated as fail by the caller).
Stdlib only.
"""

import argparse
import json
import os
import re
import sys

REF_DIR = "grace/rulesbook/codegen/references"
SRC_DIR = "crates/integrations/connector-integration/src/connectors"


def strip_comments(src):
    """Blank out // and /* */ comments, preserving offsets and newlines.

    Mandatory, not cosmetic: rapyd/transformers.rs names `in_amex_card` in three doc
    comments explaining why it is a placeholder. Parsing raw source would read those
    prose mentions as emitted types.
    """
    out, i, n = [], 0, len(src)
    while i < n:
        two = src[i:i + 2]
        if two == "//":
            j = src.find("\n", i)
            j = n if j < 0 else j
            out.append(" " * (j - i))
            i = j
        elif two == "/*":
            j = src.find("*/", i + 2)
            j = n if j < 0 else j + 2
            out.append("".join(c if c == "\n" else " " for c in src[i:j]))
            i = j
        elif src[i] == '"':
            j = i + 1
            while j < n and src[j] != '"':
                j += 2 if src[j] == "\\" else 1
            out.append(src[i:min(j + 1, n)])
            i = min(j + 1, n)
        else:
            out.append(src[i])
            i += 1
    return "".join(out)


def snake(ident):
    """InAmexCard -> in_amex_card (serde rename_all = "snake_case")."""
    return re.sub(r"(?<!^)(?=[A-Z])", "_", ident).lower()


def load_source(connector, src_dir):
    """Concatenate the connector's .rs sources, comments stripped."""
    paths = [os.path.join(src_dir, connector + ".rs")]
    sub = os.path.join(src_dir, connector)
    if os.path.isdir(sub):
        paths += [os.path.join(sub, f) for f in sorted(os.listdir(sub)) if f.endswith(".rs")]
    found = {p: strip_comments(open(p, encoding="utf-8").read())
             for p in paths if os.path.isfile(p)}
    return found




def _variants(body):
    """Wire names of an enum's variants, independent of line layout.

    Splits on top-level commas, so `A, B` and one-per-line both parse. A
    variant-level serde rename wins over the snake_case default.
    """
    names, depth, chunk = set(), 0, []
    for ch in body + ",":
        if ch in "{([":
            depth += 1
        elif ch in "})]":
            depth -= 1
        if ch == "," and depth == 0:
            text = "".join(chunk)
            ren = re.search(r'rename\s*=\s*"([^"]+)"', text)
            ident = re.search(r"(?:^|\])\s*([A-Z][A-Za-z0-9_]*)", text)
            if ren:
                names.add(ren.group(1))
            elif ident:
                names.add(snake(ident.group(1)))
            chunk = []
        else:
            chunk.append(ch)
    return names


def _wire_fields(body):
    """Wire names of a struct's fields: a serde rename wins, else the field name."""
    names = set()
    for chunk in re.split(r",\s*(?=(?:#\[|pub\b))", body):
        ren = re.search(r'rename\s*=\s*"([^"]+)"', chunk)
        fld = re.search(r"\bpub\s+([a-z_][a-z0-9_]*)\s*:", chunk)
        if ren:
            names.add(ren.group(1))
        elif fld:
            names.add(fld.group(1))
    return names


def _blocks(src, kind):
    """(name, body) for every `<kind> <name> { ... }` in src, brace-matched."""
    for m in re.finditer(r"\b" + kind + r"\s+([A-Z][A-Za-z0-9_]*)\b[^{;]*\{", src):
        depth, i = 1, m.end()
        while i < len(src) and depth:
            depth += {"{": 1, "}": -1}.get(src[i], 0)
            i += 1
        yield m.group(1), src[m.end():i - 1]


def emittable_types(sources, type_keys):
    """Wire names of the payment_method.type values the connector can emit.

    Self-locating: the capability table's keys ARE the connector's wire type names,
    so the relevant enum is the one whose snake_cased variants intersect them. This
    is why the gate needs no per-connector struct names -- hardcoding `pm_type` and
    `RapydPaymentMethodType` made it a one-connector gate that passed vacuously
    everywhere else.
    """
    types, where = set(), set()
    for path, src in sources.items():
        for name, body in _blocks(src, "enum"):
            variants = _variants(body)
            if variants & set(type_keys):
                types |= variants
                where.add("%s::%s" % (os.path.basename(path), name))
    return types, sorted(where)


def emitted_options(sources, vocab):
    """Wire names of the option fields the connector can emit.

    Self-locating in the same way: the struct to inspect is the one whose serde wire
    names intersect the vocabulary the capability table describes (its allowed fields
    plus any field it records as observed-rejected).
    """
    names, where = set(), set()
    for path, src in sources.items():
        for name, body in _blocks(src, "struct"):
            fields = _wire_fields(body)
            if fields & set(vocab):
                names |= fields
                where.add("%s::%s" % (os.path.basename(path), name))
    return names, sorted(where)


def check(table, types, options):
    """-> (violations, unprobed). A violation names the field, as the connector does."""
    violations, unprobed = [], []
    for t in sorted(types):
        row = table.get(t)
        if row is None:
            unprobed.append(t)
            continue
        allowed = set(row.get("payment_method_options") or [])
        for f in sorted(options):
            if f not in allowed:
                violations.append({"pm_type": t, "field": f, "allowed": sorted(allowed),
                                   "evidence_ref": row.get("evidence_ref"),
                                   "probed_at": row.get("probed_at")})
    return violations, unprobed


def main():
    ap = argparse.ArgumentParser(description="Request-shape gate for UCS connectors")
    ap.add_argument("--connector")
    ap.add_argument("--src", default=SRC_DIR)
    ap.add_argument("--capabilities", help="default: %s/<connector>/capabilities.json" % REF_DIR)
    ap.add_argument("--out", help="write the JSON report here")
    ap.add_argument("--selftest", action="store_true", help="replay run rapyd-854c2a and exit")
    args = ap.parse_args()

    if args.selftest:
        return selftest()
    if not args.connector:
        ap.error("--connector is required (or pass --selftest)")

    sources = load_source(args.connector, args.src)
    if not sources:
        print("shape_gate: no source for connector %r under %s" % (args.connector, args.src),
              file=sys.stderr)
        return 2

    cap_path = args.capabilities or os.path.join(REF_DIR, args.connector, "capabilities.json")
    report = {"connector": args.connector, "pass": True, "checks": [],
              "unparsed": [], "needs_human": []}

    if not os.path.isfile(cap_path):
        report["checks"].append({
            "id": "SHP-00", "name": "not_applicable", "pass": True, "evidence": [],
            "message": "no capability table at %s; probe the connector to enable this gate"
                       % cap_path})
        blob = json.dumps(report, indent=1)
        print(blob)
        return 0

    try:
        table = (json.load(open(cap_path, encoding="utf-8")) or {}).get("types") or {}
    except (OSError, ValueError, AttributeError) as exc:
        report["unparsed"].append({"what": cap_path, "why": str(exc)})
        report["needs_human"].append("capability table unreadable (%s)" % exc)
        report["pass"] = False
        print(json.dumps(report, indent=1))
        return 2

    # Vocabulary the table describes: what it permits, plus anything it records as
    # observed-rejected. Both halves matter -- a field is only checkable if the gate
    # can recognise it in the source.
    vocab = set()
    for row in table.values():
        vocab |= set(row.get("payment_method_options") or [])
        vocab |= {r.get("field") for r in (row.get("observed_rejections") or []) if r.get("field")}

    types, type_where = emittable_types(sources, table.keys())
    options, opt_where = emitted_options(sources, vocab)

    # Could not anchor: report it rather than passing vacuously. A gate that silently
    # finds nothing to check is indistinguishable from a gate that is working.
    if not types:
        report["unparsed"].append({
            "what": "payment_method.type enum",
            "why": "no enum in %s has variants matching the table's type keys (%s)"
                   % (args.connector, ", ".join(sorted(table)[:5]) or "none")})
        report["needs_human"].append(
            "SHP-03: could not locate the type enum; the table's keys must be the "
            "connector's wire type names, or the table names types this connector "
            "cannot emit")
        report["pass"] = False
        print(json.dumps(report, indent=1))
        return 2

    violations, unprobed = check(table, types, options)

    report["checks"].append({
        "id": "SHP-01", "name": "emitted_field_in_probed_domain", "pass": not violations,
        "evidence": violations,
        "message": "%d emitted option field(s) %s x %d emittable type(s) %s"
                   % (len(options), opt_where or "[none located]",
                      len(types), type_where)})
    report["checks"].append({
        "id": "SHP-02", "name": "emittable_type_probed", "pass": True,
        "evidence": [{"pm_type": t} for t in unprobed],
        "message": "%d emittable type(s) have no probed row" % len(unprobed)})
    # One line, not one per type: a connector can emit dozens of types (adyen emits 67)
    # and a needs_human[] of that length is noise. The full list stays in the evidence.
    if unprobed:
        report["needs_human"].append(
            "SHP-02: %d of %d emittable type(s) have no probed row (%s%s); probe them or "
            "carry warrant: doc:<url> AND precedent:<path>. Unprobed is not permitted -- "
            "it is unknown." % (len(unprobed), len(types), ", ".join(unprobed[:5]),
                                ", ..." if len(unprobed) > 5 else ""))
    report["pass"] = all(c["pass"] for c in report["checks"]) and not report["unparsed"]

    blob = json.dumps(report, indent=1)
    if args.out:
        os.makedirs(os.path.dirname(os.path.abspath(args.out)), exist_ok=True)
        with open(args.out, "w", encoding="utf-8") as fh:
            fh.write(blob + "\n")
    print(blob)
    return 0 if report["pass"] else 1


def selftest():
    """Replay the two schema cascades of run rapyd-854c2a. Facts from that run's traffic."""
    # 708 agent quotations in the run's traffic agree on this row; none claims
    # avs_required is allowed on in_amex_card.
    table = {"in_amex_card": {"payment_method_options":
                              ["3d_required", "tavv", "expiration_action"]}}

    # Cascade 1, the r1 ledger: payment_method_options {"3d_required":false,
    # "avs_required":true} on in_amex_card -> 400 UNKNOWN_PAYMENT_METHOD_FIELD
    # - [AVS_REQUIRED], all 20 failing rows.
    v, _ = check(table, {"in_amex_card"}, {"3d_required", "avs_required"})
    assert [x["field"] for x in v] == ["avs_required"], v

    # Cascade 2, external 3DS: 3d_version, cavv, eci, ds_trans_id on in_amex_card
    # -> 400 UNKNOWN_PAYMENT_METHOD_FIELD - [3D_VERSION].
    v, _ = check(table, {"in_amex_card"}, {"3d_version", "cavv", "eci", "ds_trans_id"})
    assert sorted(x["field"] for x in v) == ["3d_version", "cavv", "ds_trans_id", "eci"], v

    # Must not over-reject the shapes that actually passed.
    assert check(table, {"in_amex_card"}, {"3d_required"}) == ([], [])
    assert check(table, {"in_amex_card"}, set()) == ([], [])

    # Abstention: the run proved these fields DO succeed on gb_visa_card, yet with no
    # probed row the gate must report unprobed rather than infer from a sibling type.
    # Inferring from a sibling is how avs_required shipped.
    v, un = check(table, {"gb_visa_card"}, {"3d_version", "cavv"})
    assert (v, un) == ([], ["gb_visa_card"]), (v, un)

    # Self-location, and the regression that motivated it: the anchors are found via
    # the table's own vocabulary, not via hardcoded rapyd identifiers. A type named
    # only in a doc comment is not emittable.
    src = {"x.rs": strip_comments(
        '/// uses `in_amex_card` as a placeholder\n'
        'pub struct Anything { #[serde(rename = "3d_required")] pub three_ds: bool,\n'
        '  pub avs_required: bool }\n'
        'enum WhateverItIsCalled { InAmexCard, InCreditVisaCard }\n')}
    vocab = {"3d_required", "tavv", "expiration_action"}
    opts, ow = emitted_options(src, vocab)
    tys, tw = emittable_types(src, table.keys())
    assert opts == {"3d_required", "avs_required"}, opts
    assert tys == {"in_amex_card", "in_credit_visa_card"}, tys
    assert ow and tw, (ow, tw)

    # A connector that shares no vocabulary with the table anchors nothing. main()
    # turns this into SHP-03 + exit 2; it must never read as a silent pass.
    other = {"y.rs": "pub struct Req { pub amount: i64 }\nenum Foo { Bar, Baz }\n"}
    assert emittable_types(other, table.keys())[0] == set()
    assert emitted_options(other, vocab)[0] == set()

    print("shape_gate selftest OK: both rapyd-854c2a schema cascades caught, "
          "green shapes pass, unprobed types abstain, anchors self-located, comments ignored")
    return 0


if __name__ == "__main__":
    sys.exit(main())
