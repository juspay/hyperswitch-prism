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


def _body(src, kind, name):
    """Brace-matched body of `<kind> <name> { ... }`, or None.

    Brace-matched rather than `[^}]*` so a variant or field carrying a braced
    payload does not truncate the body.
    """
    m = re.search(r"\b" + kind + r"\s+" + re.escape(name) + r"\b[^{]*\{", src)
    if not m:
        return None
    depth, i = 1, m.end()
    while i < len(src) and depth:
        depth += {"{": 1, "}": -1}.get(src[i], 0)
        i += 1
    return src[m.end():i - 1]


def emitted_options(sources):
    """Wire names of the payment_method_options fields the connector can emit."""
    names = set()
    for src in sources.values():
        body = _body(src, "struct", "PaymentMethodOptions")
        if body is None:
            continue
        # A serde rename wins; otherwise the field name is the wire name.
        for chunk in re.split(r",\s*(?=(?:#\[|pub\b))", body):
            ren = re.search(r'rename\s*=\s*"([^"]+)"', chunk)
            fld = re.search(r"\bpub\s+([a-z_][a-z0-9_]*)\s*:", chunk)
            if ren:
                names.add(ren.group(1))
            elif fld:
                names.add(fld.group(1))
    return names


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


def emittable_types(sources):
    """Wire names of the payment_method.type values the connector can emit."""
    enum_names, types = set(), set()
    for src in sources.values():
        enum_names |= set(re.findall(r"\bpm_type\s*:\s*([A-Za-z_][A-Za-z0-9_]*)", src))
    for src in sources.values():
        for enum in enum_names:
            body = _body(src, "enum", enum)
            if body is not None:
                types |= _variants(body)
    return types


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

    options, types = emitted_options(sources), emittable_types(sources)
    violations, unprobed = check(table, types, options)

    report["checks"].append({
        "id": "SHP-01", "name": "emitted_field_in_probed_domain", "pass": not violations,
        "evidence": violations,
        "message": "%d emitted payment_method_options x %d emittable payment_method.type"
                   % (len(options), len(types))})
    report["checks"].append({
        "id": "SHP-02", "name": "emittable_type_probed", "pass": True,
        "evidence": [{"pm_type": t} for t in unprobed],
        "message": "%d emittable type(s) have no probed row" % len(unprobed)})
    for t in unprobed:
        report["needs_human"].append(
            "SHP-02: %s is emittable but unprobed; probe it or carry "
            "warrant: doc:<url> AND precedent:<path>" % t)
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

    # Parsing: a type named only in a doc comment is not emittable.
    src = {"x.rs": strip_comments(
        '/// uses `in_amex_card` as a placeholder\n'
        'pub struct PaymentMethodOptions { #[serde(rename = "3d_required")] pub three_ds: bool,\n'
        '  pub avs_required: bool }\n'
        'struct PaymentMethod { pub pm_type: RapydPaymentMethodType }\n'
        'enum RapydPaymentMethodType { InAmexCard, InCreditVisaCard }\n')}
    assert emitted_options(src) == {"3d_required", "avs_required"}, emitted_options(src)
    assert emittable_types(src) == {"in_amex_card", "in_credit_visa_card"}, emittable_types(src)

    print("shape_gate selftest OK: both rapyd-854c2a schema cascades caught, "
          "green shapes pass, unprobed types abstain, comment mentions ignored")
    return 0


if __name__ == "__main__":
    sys.exit(main())
