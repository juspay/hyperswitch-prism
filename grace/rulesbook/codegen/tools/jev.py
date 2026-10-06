#!/usr/bin/env python3
"""Bounded-judgement helper backed by TypeSafe System One ("Jev").

Five sites, chosen with --site. Every option set is a vocabulary this repo already
defines, and every answer has a consumer that already reads it. Nothing here invents a
taxonomy: an earlier version did, classifying spec claims as live_probed/doc_example/
inherited, and no stage read those labels -- `grep -rniE 'confirmed[ _-]?live|warrant'`
over grace/workflow, the codegen guides and the native techspec workflow returns nothing.

  links       score a documentation page against one of the 10 checklist elements
              `2.1_links.md` "2B": YES 1 point / PARTIAL 0.5 / NO 0. The aggregation
              above it stays a threshold (>=7 valid, >=4 problematic, <4 insufficient),
              computed here, not judged.                              ~1000-3000 items
  oracle      does the quoted oracle_ref phrase support the row's expected value?
              `2.6d` Phase 5 row 3. Decides a 2.2 techspec AMEND vs a connector bug.
                                                                        ~10-100 items
  theme       is this review_themes hit a real defect, or correct-as-written?
              `2.3b` Phase 7: every hit is fixed or carries a justified[] entry.
              The only blocking site.                                      ~25 items
  undecided   does source k decide this UNDECIDED field?
              `2.3a` Phase 7's own five-rung precedence walk. The walk and its
              fail-closed terminal rung stay deterministic here; only the leaf
              "does this source decide it" is asked.                    ~20-120 items
  brief       is this per-item brief entry sufficient to implement the item with no
              further file reads? `2.3b` Phase 1 writes the brief; the implementing
              step reads nothing else. Non-blocking: a false widens the brief. The
              only self-calibrating site -- a file read inside the implementing
              step is a recorded false positive.                           ~43 items

Connector-agnostic by construction: it reads GRACE artefact schemas and the repo's own
vocabularies. No connector name appears in its logic.

Keyed questions, never positional: one question per item, id `k<n>`, the item embedded in
its own instructions. Positional references into a long array measure ~27% wrong at 150
items; keyed/embedded forms measure 0/320 wrong at 320.

Exit codes mirror grace/rulesbook/codegen/tools/refusal_gate.py:
  0 pass, 1 at least one item blocks, 2 could not evaluate (treated as fail by the caller).

Stdlib only.
"""

import argparse
import json
import os
import sys
import time
import urllib.error
import urllib.request

# Two ways to reach the same model, same /v1/systemone schema, same listed rate
# (prompt 0.000000042/token, completion 0): only URL, model id and key differ.
PROVIDERS = {
    "typesafe": {
        "url": os.environ.get("TYPESAFE_BASE_URL", "https://api.typesafe.ai") + "/v1/systemone",
        "model": "jev-latest",
        "env": ("TYPESAFE_API_KEY",),
        "keyfile": "~/.config/typesafe/api_key"},
    "openrouter": {
        "url": os.environ.get("OPENROUTER_BASE_URL", "https://openrouter.ai/api") + "/v1/systemone",
        # Versioned on OpenRouter; jev-latest 404s there. NOT typesafe/jev-router, which
        # is a model *router* that returns text -- the opposite of a typed decision.
        "model": "typesafe/jev-1.13",
        "env": ("OPENROUTER_API_KEY", "OPENROUTER_KEY", "OR_API_KEY"),
        "keyfile": "~/.config/openrouter/api_key"},
}

MAX_STATE_CHARS = 100_000          # documented budget ~107,500; stay under it
MAX_QUESTIONS_PER_CALL = 200
COST_PER_MTOK_IN = 0.042           # USD per million input tokens; output is free

# 2.1_links.md "2B" scores each element YES / PARTIAL / NO. `score` wants an ordered
# list, weakest first, so index 0/1/2 maps to 0 / 0.5 / 1 point.
LINK_LEVELS = ["the page does not cover this element at all",
               "the page covers this element partially or only by implication",
               "the page covers this element explicitly and usably"]
LINK_POINTS = [0.0, 0.5, 1.0]

SITES = {
    "links": {
        "type": "score", "criteria": LINK_LEVELS, "blocks": False,
        "instructions":
            "Score how well this documentation page covers the named checklist element. "
            "Judge only the page text given. A page that merely mentions the topic without "
            "the detail an integrator needs is partial, not full coverage."},
    # 2.6d row 3 reads "absent from OR contradicted by", treating the two alike. Asked
    # that way as one yes/no it flagged 45% of a matrix the run passed, because
    # `expected.value` bundles several claims and a quote often supports some of them.
    # Splitting the answer keeps the useful signal: only `contradicts` is a spec defect,
    # `silent` is recorded as the weaker reading it is.
    "oracle": {
        "type": "choice", "blocks": False,
        "criteria": {
            "supports": "the quoted phrase states, or directly implies, what this "
                        "expected value asserts",
            "silent": "the quoted phrase is about the right subject but does not settle "
                      "this particular value -- it neither states nor denies it",
            "contradicts": "the quoted phrase asserts something incompatible with this "
                           "expected value, or is about an entirely different subject"},
        "instructions":
            "A test row asserts an expected value and cites a specification phrase as its "
            "oracle. Decide the relationship between the quoted phrase and the expected "
            "value. Judge only the quoted phrase given. The expected value may bundle "
            "several claims: answer `supports` when the quote covers the substantive "
            "ones, `silent` when it is on-topic but does not settle them, and "
            "`contradicts` only when it is incompatible or plainly about something else."},
    "theme": {
        "type": "noul", "blocks": True,
        "criteria": {"true": "a real defect: the code at this line should be changed",
                     "false": "correct as written: the pattern is right here and the hit "
                              "is explainable"},
        "instructions":
            "A recurring-review-theme check matched the line marked '>' in "
            "surrounding_code. A hit is not automatically a defect. Judge the marked line "
            "in the context of the lines around it -- a value that looks hardcoded on its "
            "own is often correct once the enclosing function and the type it builds are "
            "visible. Decide whether it is a real defect, or correct as written: for "
            "example a .first() on a collection the API documents as holding exactly one "
            "element, or an amount the wire format really does send as a string."},
    "undecided": {
        "type": "noul", "blocks": False,
        "criteria": {"true": "this source settles the field: it states what the value must be",
                     "false": "this source does not settle it"},
        "instructions":
            "An open design question about a connector's request field is being resolved "
            "by consulting sources in a fixed order. Decide whether THIS source settles "
            "THIS field. Judge only the evidence given for this source."},
    # 2.3b Phase 1 writes a brief per plan item so Phase 2 can implement it without
    # re-reading the techspec, the pattern guides or the domain-type files. This asks
    # whether that brief actually suffices. Non-blocking by design: a `false` widens the
    # brief, it never fails a unit. It is the only site that calibrates itself -- a Bash
    # call inside the implementing spawn is a recorded false positive.
    "brief": {
        "type": "noul", "blocks": False,
        "criteria": {"true": "sufficient: every name, signature and line this item needs "
                             "to be implemented is present in the brief entry",
                     "false": "insufficient: implementing this item would require opening "
                              "a file the brief does not quote"},
        "instructions":
            "A plan item is about to be implemented by a step that may read NOTHING but "
            "the brief entry given here. Decide whether the entry is sufficient. It is "
            "sufficient when the anchor text to be changed is quoted, and every type, "
            "field, variant or helper the change must name is present with its real "
            "signature -- not merely referenced by path. Answer `false` when the item "
            "names a type whose definition is absent, when the quoted anchor does not "
            "contain the code the action describes, or when the action requires a "
            "convention (an amount unit, a status mapping, an error envelope) that "
            "neither the item nor the brief states. Judge only what is given: a path, a "
            "file name or a heading reference is not the content it points at."},
}

# 2.3a Phase 7's precedence walk, in order. The fifth rung is the fail-closed terminal
# and is never asked of the model.
UNDECIDED_RUNGS = [
    ("hs", "the Hyperswitch reference implementation's behaviour at the cited anchor"),
    ("spec", "the connector's own specification or documentation text"),
    ("proto", "the proto contract: whether the RPC request even carries a field for it"),
    ("precedent", "a UCS connector of the same integration pattern"),
]


def api_key(explicit, prov):
    """--api-key, then the provider's env var(s), then its key file."""
    if explicit:
        return explicit
    for name in prov["env"]:
        if os.environ.get(name):
            return os.environ[name]
    try:
        return open(os.path.expanduser(prov["keyfile"]), encoding="utf-8").read().strip() or None
    except OSError:
        return None


def _http_post(payload, key, url):
    req = urllib.request.Request(
        url, data=json.dumps(payload).encode("utf-8"),
        headers={"Authorization": "Bearer %s" % key, "Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=60) as r:
        return json.loads(r.read().decode("utf-8"))


def ask(state, questions, key, prov, transport=_http_post):
    """-> (answers, input_tokens, ms, error, reported_cost). Never raises on transport."""
    payload = {"state": state, "model": prov["model"], "questions": questions}
    t0 = time.time()
    try:
        body = transport(payload, key, prov["url"])
    except (urllib.error.HTTPError, urllib.error.URLError, OSError, ValueError) as e:
        return None, 0, int((time.time() - t0) * 1000), "%s: %s" % (type(e).__name__, e), None
    ms = int((time.time() - t0) * 1000)
    if not isinstance(body, dict) or "answers" not in body:
        return None, 0, ms, "response has no answers object", None
    usage = body.get("usage") or {}
    return body["answers"], usage.get("input_tokens", 0), ms, None, usage.get("cost")


def with_context(items, repo_root, n):
    """Attach n lines either side of each item's file:line.

    A hit line alone is not always decidable: a TH-06 hit read as
    `let attempt_status = (res.status_code == 400)` looks like a hardcoded status, while
    the next line carries `FlowStatus::Refund(...)` that makes it flow-aware. Measured on
    a real run, the single-line form disagreed with the full-context agent on exactly that
    case. gate/themes.json records only {file, line, text}, so the context is read here.
    """
    if not n:
        return items
    cache = {}
    out = []
    for it in items:
        if not isinstance(it, dict) or not it.get("file") or not it.get("line"):
            out.append(it)
            continue
        path = os.path.join(repo_root or "", str(it["file"]))
        if path not in cache:
            try:
                cache[path] = open(path, encoding="utf-8", errors="replace").read().splitlines()
            except OSError:
                cache[path] = None
        lines = cache[path]
        if not lines:
            out.append(it)
            continue
        try:
            ln = int(it["line"])
        except (TypeError, ValueError):
            out.append(it)
            continue
        lo, hi = max(0, ln - 1 - n), min(len(lines), ln + n)
        snippet = "\n".join("%s%d: %s" % ("> " if (i + 1) == ln else "  ", i + 1, lines[i])
                             for i in range(lo, hi))
        out.append(dict(it, surrounding_code=snippet))
    return out


def build(site, items):
    """Keyed questions, one per item, each item embedded in its own instructions."""
    spec = SITES[site]
    q = {}
    for n, it in enumerate(items):
        text = it if isinstance(it, str) else json.dumps(it, ensure_ascii=False, sort_keys=True)
        q["k%d" % n] = {"type": spec["type"],
                        "instructions": "%s\n\nITEM:\n%s" % (spec["instructions"], text),
                        "criteria": spec["criteria"]}
    return q


def decide(site, ans, threshold):
    """-> (value, probability, blocks)."""
    spec = SITES[site]
    if spec["type"] == "choice":
        v = ans.get("choice")
        return v, (ans.get("probabilities") or {}).get(v), bool(spec["blocks"])
    if spec["type"] == "score":
        raw = ans.get("score")
        idx = int(round(float(raw))) if raw is not None else 0
        idx = max(0, min(len(LINK_POINTS) - 1, idx))
        return LINK_POINTS[idx], (None if raw is None else float(raw)), False
    p = ans.get("noul")
    p = 0.0 if p is None else float(p)
    yes = p >= threshold
    return yes, p, bool(yes and spec["blocks"])


def chunks(questions, state_chars):
    keys, out, cur, size = list(questions), [], {}, 0
    room = max(1, MAX_STATE_CHARS - state_chars)
    for k in keys:
        qlen = len(json.dumps(questions[k], ensure_ascii=False))
        if cur and (len(cur) >= MAX_QUESTIONS_PER_CALL or size + qlen > room):
            out.append(cur)
            cur, size = {}, 0
        cur[k] = questions[k]
        size += qlen
    if cur:
        out.append(cur)
    return out


def run(site, state, items, key, threshold=0.5, transport=_http_post, prov=None):
    """-> (results, meta)."""
    prov = prov or PROVIDERS["typesafe"]
    state_s = state if isinstance(state, str) else json.dumps(state, ensure_ascii=False)
    questions = build(site, items)
    meta = {"calls": 0, "input_tokens": 0, "latency_ms": [], "errors": [],
            "reported_cost_usd": 0.0, "cost_source": "derived"}
    answers = {}
    for batch in chunks(questions, len(state_s)):
        a, toks, ms, err, rcost = ask(state_s, batch, key, prov, transport)
        meta["calls"] += 1
        meta["latency_ms"].append(ms)
        meta["input_tokens"] += toks
        if rcost is not None:
            meta["reported_cost_usd"] += float(rcost)
            meta["cost_source"] = "provider"
        if err:
            meta["errors"].append(err)
            continue
        answers.update(a)
    results = []
    for n, it in enumerate(items):
        a = answers.get("k%d" % n)
        if a is None:
            results.append({"item": it, "value": None, "probability": None,
                            "confidence": None, "blocks": False, "unanswered": True})
            continue
        v, p, b = decide(site, a, threshold)
        results.append({"item": it, "value": v, "probability": p,
                        "confidence": a.get("confidence"), "blocks": b})
    meta["cost_usd"] = (round(meta["reported_cost_usd"], 8) if meta["cost_source"] == "provider"
                        else round(meta["input_tokens"] / 1e6 * COST_PER_MTOK_IN, 8))
    return results, meta


# ---------------------------------------------------------------- site post-processing

def link_bands(results):
    """Aggregate element points per URL and apply 2.1_links.md's own threshold.

    The threshold is a rule, not a judgement: score = 10 * points / elements_in_scope,
    then >=7 valid, >=4 problematic, else insufficient (`2.1_links.md` 2B, :155-158).
    """
    per = {}
    for r in results:
        it = r["item"] if isinstance(r["item"], dict) else {}
        url = it.get("url", "?")
        d = per.setdefault(url, {"points": 0.0, "elements": 0})
        if r.get("value") is not None:
            d["points"] += float(r["value"])
            d["elements"] += 1
    out = {}
    for url, d in per.items():
        score = (10.0 * d["points"] / d["elements"]) if d["elements"] else 0.0
        out[url] = {"points": round(d["points"], 2), "elements": d["elements"],
                    "score": round(score, 2),
                    "status": "valid" if score >= 7 else
                              ("problematic" if score >= 4 else "insufficient")}
    return out


def undecided_items(plan):
    """-> items for --site undecided, from plan.json `undecided[]` x the four rungs."""
    items = []
    for u in plan.get("undecided") or []:
        ev = u.get("evidence")
        ev = [str(x) for x in ev] if isinstance(ev, list) else ([str(ev)] if ev else [])
        for rung, desc in UNDECIDED_RUNGS:
            # Give the model only the evidence lines that belong to this rung.
            mine = [e for e in ev if e.lower().startswith(rung + ":")] or \
                   ([e for e in ev if ":" not in e.split("/")[0][:12]] if rung == "spec" else [])
            items.append({"field_id": u.get("id"), "unit": u.get("unit"),
                          "field": u.get("field"), "source": rung,
                          "source_means": desc,
                          "evidence_for_this_source": mine or "none cited"})
    return items


def undecided_walk(results):
    """Apply 2.3a's precedence deterministically over the leaf answers.

    First rung answered yes wins; none -> the fail-closed option with verify_live, which
    is the workflow's own terminal rung and is never asked of the model.
    """
    by_field = {}
    for r in results:
        it = r["item"] if isinstance(r["item"], dict) else {}
        by_field.setdefault(it.get("field_id"), {})[it.get("source")] = r
    out = {}
    for fid, got in by_field.items():
        chosen = None
        for rung, _ in UNDECIDED_RUNGS:
            r = got.get(rung)
            if r and r.get("value") is True:
                chosen = {"resolved_by": rung, "probability": r.get("probability")}
                break
        out[fid] = chosen or {"resolved_by": "fail_closed",
                              "note": "no source decided it; require the field and refuse "
                                      "via a guard when absent, verify_live: true"}
    return out


# ---------------------------------------------------------------- CLI

def main():
    ap = argparse.ArgumentParser(description="Jev bounded-judgement helper")
    ap.add_argument("--site", required=True, choices=sorted(SITES))
    ap.add_argument("--items", help="JSON file: a list of items to judge")
    ap.add_argument("--from-plan", dest="from_plan",
                    help="--site undecided: build items from a GRACE plan.json")
    ap.add_argument("--state", default="none")
    ap.add_argument("--threshold", type=float, default=0.5)
    ap.add_argument("--provider", default=os.environ.get("JEV_PROVIDER", "typesafe"),
                    choices=sorted(PROVIDERS))
    ap.add_argument("--api-key")
    ap.add_argument("--out")
    ap.add_argument("--receipts")
    ap.add_argument("--context-lines", dest="context_lines", type=int, default=0,
                    help="for --site theme: lines of source context either side of a hit")
    ap.add_argument("--repo-root", dest="repo_root", default=".")
    args = ap.parse_args()

    report = {"site": args.site, "provider": args.provider, "pass": True,
              "checks": [], "unparsed": [], "needs_human": []}

    def emit(code):
        blob = json.dumps(report, indent=1)
        if args.out:
            os.makedirs(os.path.dirname(os.path.abspath(args.out)), exist_ok=True)
            open(args.out, "w", encoding="utf-8").write(blob + "\n")
        print(blob)
        return code

    # ---- items
    try:
        if args.from_plan:
            if args.site != "undecided":
                raise ValueError("--from-plan is only for --site undecided")
            plan = json.load(open(args.from_plan, encoding="utf-8"))
            items = undecided_items(plan)
            # Fail closed: a plan with open questions that yields no items means the
            # extractor has drifted from the schema, not that there is nothing to ask.
            if not items and (plan.get("undecided") or []):
                raise ValueError("plan has undecided[] but the extractor produced no items; "
                                 "the schema has drifted and a check that checks nothing "
                                 "does not pass")
        elif args.items:
            items = json.load(open(args.items, encoding="utf-8"))
        else:
            raise ValueError("pass --items or --from-plan")
        if not isinstance(items, list):
            raise ValueError("input must yield a JSON list")
    except (OSError, ValueError) as e:
        report["unparsed"].append({"what": args.from_plan or args.items or "input",
                                   "why": str(e)})
        report["pass"] = False
        return emit(2)

    state = ""
    if args.state and args.state != "none":
        try:
            state = open(args.state, encoding="utf-8").read()
        except OSError as e:
            report["unparsed"].append({"what": args.state, "why": str(e)})
            report["pass"] = False
            return emit(2)

    prov = PROVIDERS[args.provider]
    key = api_key(args.api_key, prov)
    if not key:
        report["unparsed"].append({
            "what": "%s API key" % args.provider,
            "why": "not in --api-key, $%s or %s; a check that cannot reach its model does "
                   "not pass" % ("/$".join(prov["env"]), prov["keyfile"])})
        report["pass"] = False
        return emit(2)

    if not items:
        report["summary"] = {"items": 0, "note": "nothing to judge"}
        report["checks"] = [{"id": "JEV-01", "name": "bounded_judgement_%s" % args.site,
                             "pass": True, "evidence": [], "inconclusive": [],
                             "message": "no items"}]
        return emit(0)

    items = with_context(items, args.repo_root, args.context_lines)
    results, meta = run(args.site, state, items, key, args.threshold, prov=prov)

    if meta["errors"]:
        report["unparsed"].append({"what": "jev transport",
                                   "why": "; ".join(sorted(set(meta["errors"])))})
    unanswered = [r for r in results if r.get("unanswered")]
    blocked = [r for r in results if r["blocks"]]
    for r in unanswered:
        report["needs_human"].append("no answer returned for %r" % (r["item"],))

    report["results"] = results
    if args.site == "links":
        report["bands"] = link_bands(results)
    if args.site == "undecided":
        report["resolution"] = undecided_walk(results)

    report["checks"] = [{
        "id": "JEV-01", "name": "bounded_judgement_%s" % args.site,
        "pass": not blocked,
        "evidence": blocked, "inconclusive": unanswered,
        "message": ("%d of %d items are real defects" % (len(blocked), len(results)))
                   if blocked else "ok"}]
    could_not_evaluate = bool(report["unparsed"]) or (results and not blocked
                                                      and len(unanswered) == len(results))
    report["pass"] = all(c["pass"] for c in report["checks"]) and not report["unparsed"]
    report["summary"] = {"items": len(results), "blocked": len(blocked),
                         "could_not_evaluate": could_not_evaluate,
                         "unanswered": len(unanswered), "calls": meta["calls"],
                         "input_tokens": meta["input_tokens"], "cost_usd": meta["cost_usd"],
                         "cost_source": meta["cost_source"],
                         "latency_ms": meta["latency_ms"], "threshold": args.threshold}

    if args.receipts:
        os.makedirs(args.receipts, exist_ok=True)
        rc = os.path.join(args.receipts, "%s-%d.json" % (args.site, int(time.time())))
        open(rc, "w", encoding="utf-8").write(json.dumps(
            {"site": args.site, "provider": args.provider, "state_chars": len(state),
             "threshold": args.threshold, "results": results, "meta": meta,
             "bands": report.get("bands"), "resolution": report.get("resolution")},
            indent=1) + "\n")
        report["summary"]["receipt"] = rc

    # 0 pass · 1 real defects found · 2 could not evaluate.
    if could_not_evaluate:
        return emit(2)
    return emit(0 if report["pass"] else 1)


def _replay():
    """Offline self-check of this file's own logic: mapping, thresholds, the link
    threshold, the undecided precedence walk, keyed ids and batching. It cannot test
    Jev's accuracy -- that needs the network and is what a run measures."""

    def stub(pick):
        def t(payload, key, url):
            assert url.endswith("/v1/systemone"), url
            ans = {}
            for qid, q in payload["questions"].items():
                assert qid.startswith("k"), qid          # keyed, never positional
                item = q["instructions"].split("ITEM:", 1)[-1]
                if q["type"] == "choice":
                    c = pick(item) if callable(pick) else pick
                    ans[qid] = {"type": "choice", "choice": c,
                                "probabilities": {c: 0.9}, "confidence": 0.8}
                elif q["type"] == "score":
                    lvl = pick(item) if callable(pick) else pick
                    ans[qid] = {"type": "score", "score": lvl, "confidence": 0.8}
                else:
                    p = pick(item) if callable(pick) else pick
                    ans[qid] = {"type": "noul", "noul": p, "confidence": 0.8}
            return {"model": payload["model"],
                    "usage": {"input_tokens": 10 * len(payload["questions"]),
                              "cost": 4.2e-7},
                    "answers": ans}
        return t

    # links: the three levels map to 0 / 0.5 / 1 and feed the file's own threshold.
    els = [{"url": "https://v.example/a", "element": "e%d" % i} for i in range(10)]
    r, m = run("links", {}, els, "k", transport=stub(2))
    assert all(x["value"] == 1.0 for x in r) and not any(x["blocks"] for x in r)
    assert link_bands(r)["https://v.example/a"]["status"] == "valid"
    r, _ = run("links", {}, els, "k", transport=stub(1))
    b = link_bands(r)["https://v.example/a"]
    assert b["points"] == 5.0 and b["score"] == 5.0 and b["status"] == "problematic", b
    r, _ = run("links", {}, els, "k", transport=stub(0))
    assert link_bands(r)["https://v.example/a"]["status"] == "insufficient"
    assert m["cost_usd"] > 0 and m["cost_source"] == "provider"

    # oracle: three readings, none of them blocking -- it routes a triage class.
    for pick in ("supports", "silent", "contradicts"):
        r, _ = run("oracle", {}, [{"case_id": "c1", "expected": "x", "oracle_ref": "y"}],
                   "k", transport=stub(pick))
        assert r[0]["value"] == pick and r[0]["blocks"] is False, r[0]

    # theme: the one blocking site.
    r, _ = run("theme", {}, [{"theme": "TH-08", "file": "a.rs", "line": 1}],
               "k", transport=stub(0.9))
    assert r[0]["value"] is True and r[0]["blocks"] is True
    r, _ = run("theme", {}, [{"theme": "TH-08", "file": "a.rs", "line": 1}],
               "k", transport=stub(0.1))
    assert r[0]["value"] is False and r[0]["blocks"] is False

    # undecided: items are field x rung, and the WALK is deterministic.
    plan = {"undecided": [
        {"id": "UD-01", "unit": "U", "field": "country", "verify_live": True,
         "evidence": ["hs:crates/x/transformers.rs:1-9", "spec:### 1. country"]},
        {"id": "UD-02", "unit": "U", "field": "a constant", "verify_live": False,
         "evidence": ["doc:https://vendor.example/api"]}]}
    items = undecided_items(plan)
    assert len(items) == 2 * len(UNDECIDED_RUNGS), len(items)
    assert {i["source"] for i in items} == {r for r, _ in UNDECIDED_RUNGS}
    # only the hs rung has evidence for UD-01 -> hs wins, higher rungs never consulted
    r, _ = run("undecided", {}, items, "k",
               transport=stub(lambda it: 0.9 if '"source": "hs"' in it and "UD-01" in it
                              else 0.1))
    w = undecided_walk(r)
    assert w["UD-01"]["resolved_by"] == "hs", w
    assert w["UD-02"]["resolved_by"] == "fail_closed", w
    # spec beats proto when both answer yes (precedence, not probability)
    r, _ = run("undecided", {}, [i for i in items if i["field_id"] == "UD-02"], "k",
               transport=stub(lambda it: 0.9 if ('"source": "spec"' in it
                                                 or '"source": "proto"' in it) else 0.1))
    assert undecided_walk(r)["UD-02"]["resolved_by"] == "spec"

    # a transport failure yields no answers, and must read as "could not evaluate"
    def dead(payload, key, url):
        raise urllib.error.HTTPError(url, 503, "Service Unavailable", {}, None)
    r, m = run("theme", {}, [{"theme": "TH-06", "file": "a.rs", "line": 1}], "k",
               transport=dead)
    assert all(x.get("unanswered") for x in r), r
    assert not any(x["blocks"] for x in r), r
    assert m["errors"] and "503" in m["errors"][0], m["errors"]

    # context attachment: the marked line plus its neighbours, or the item untouched
    import tempfile
    with tempfile.TemporaryDirectory() as td:
        os.makedirs(os.path.join(td, "x"), exist_ok=True)
        f = os.path.join(td, "x", "a.rs")
        open(f, "w").write("\n".join("line%d" % i for i in range(1, 21)))
        got = with_context([{"theme": "TH-06", "file": "x/a.rs", "line": 10}], td, 2)
        sc = got[0]["surrounding_code"]
        assert "> 10: line10" in sc, sc           # the hit line is marked
        assert "  8: line8" in sc and "  12: line12" in sc, sc
        assert "line7" not in sc and "line13" not in sc, sc
        # n=0 leaves items exactly as they were
        same = [{"theme": "TH-06", "file": "x/a.rs", "line": 10}]
        assert with_context(same, td, 0) == same
        # a missing file degrades to the bare item rather than failing
        assert "surrounding_code" not in with_context(
            [{"file": "nope.rs", "line": 3}], td, 2)[0]
        # a hit at line 1 must not underflow
        assert "> 1: line1" in with_context(
            [{"file": "x/a.rs", "line": 1}], td, 3)[0]["surrounding_code"]

    # brief: a noul site that must never block -- an insufficient brief widens the
    # brief, it never fails a unit. theme stays the only blocking site.
    assert SITES["brief"]["type"] == "noul" and SITES["brief"]["blocks"] is False
    assert [k for k, v in SITES.items() if v["blocks"]] == ["theme"], \
        "theme must remain the only blocking site"
    for p_ in (0.04, 0.5, 0.96):
        v, got, blk = decide("brief", {"noul": p_}, 0.5)
        assert v is (p_ >= 0.5) and got == p_ and blk is False, (p_, v, got, blk)
    # one question per item, keyed, with the item embedded (never positionally referenced)
    bq = build("brief", [{"item_id": "P-Authorize-04", "current": "fn a() {}"},
                         {"item_id": "P-Refund-01", "current": "fn b() {}"}])
    assert sorted(bq) == ["k0", "k1"] and len(bq) == 2, bq
    assert "P-Authorize-04" in bq["k0"]["instructions"], bq["k0"]
    assert "P-Refund-01" in bq["k1"]["instructions"], bq["k1"]
    assert bq["k0"]["criteria"] == SITES["brief"]["criteria"]

    # batching stays inside the documented budget
    big = chunks(build("links", els * 400), MAX_STATE_CHARS - 5_000)
    assert len(big) > 1 and all(len(c) <= MAX_QUESTIONS_PER_CALL for c in big)

    print("replay OK: link levels map to 0/0.5/1 and feed the file's own threshold, "
          "theme is the only blocking site, the undecided walk honours precedence over "
          "probability and falls closed, brief never blocks, keyed ids and "
          "batching hold")
    return 0


if __name__ == "__main__":
    sys.exit(_replay() if sys.argv[1:2] == ["--replay"] else main())
