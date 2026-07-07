#!/usr/bin/env python3
"""
Test Coverage Summary (grpc CI reports)

Complements coverage_summary.py. Where coverage_summary.py measures *capability*
(does the connector's request transformer build a valid request -- static, from
data/field_probe/*.json), this script measures *test-pass coverage*: what actually
PASSES end-to-end against the sandboxes in the grpc integration reports
(report_YYYYMMDD_HHMM.json).

It takes two reports -- a baseline and a current -- and prints a markdown growth
table. Every coverage metric is expressed in the SAME unit the capability skill
uses: `connector x flow` pairs (API) and `connector x payment-method` pairs (PMT),
so the denominators are the full cross-product (100s of cells), not a count of
distinct flows/methods. Production rows restrict that cross-product to the live
production connector roster.

A report is a JSON object with a top-level "runs" array; each run carries at least
{connector, suite, pm, assertion_result, is_dependency}. Dependency runs and SKIP
results are excluded from every metric.

Usage:
    python3 scripts/generators/docs/test_coverage.py BASE CURRENT
    python3 scripts/generators/docs/test_coverage.py BASE CURRENT --prod adyen,stripe,...

BASE / CURRENT may be a local file path or an http(s) URL.
"""

import sys
import json
import argparse
import urllib.request
from collections import defaultdict

# Production connector roster -- the live connectors whose pass coverage matters
# most for stakeholder reporting. Override with --prod if the roster changes.
DEFAULT_PRODUCTION = [
    "adyen", "authorizedotnet", "braintree", "cashtocode", "cryptopay",
    "cybersource", "fiuu", "mifinity", "noon", "novalnet", "paypal",
    "stripe", "volt", "worldpay",
]


def load_report(src):
    """Load a report from a local path or an http(s) URL."""
    if src.startswith("http://") or src.startswith("https://"):
        with urllib.request.urlopen(src) as resp:
            data = json.loads(resp.read().decode("utf-8"))
    else:
        with open(src, "r", encoding="utf-8") as f:
            data = json.load(f)
    return [r for r in (data.get("runs") or []) if not r.get("is_dependency")]


def _g(run, key):
    v = run.get(key)
    return v if v is not None else ""


def analyze(runs, production):
    """Compute pass-based coverage metrics over a single report's runs."""
    prod = {c.lower() for c in production}

    total = passed = 0
    conns_with_pass = set()

    # connector x flow (API) and connector x payment-method (PMT) cells.
    api_seen, api_pass = set(), set()
    pm_seen, pm_pass = set(), set()
    papi_seen, papi_pass = set(), set()
    ppm_seen, ppm_pass = set(), set()

    for r in runs:
        if _g(r, "assertion_result") == "SKIP":
            continue
        conn = _g(r, "connector")
        api = _g(r, "suite")
        pm = _g(r, "pm")
        ok = _g(r, "assertion_result") == "PASS"

        total += 1
        if ok:
            passed += 1
            conns_with_pass.add(conn)

        api_seen.add((conn, api))
        if ok:
            api_pass.add((conn, api))
        if pm:
            pm_seen.add((conn, pm))
            if ok:
                pm_pass.add((conn, pm))

        if conn.lower() in prod:
            papi_seen.add((conn, api))
            if ok:
                papi_pass.add((conn, api))
            if pm:
                ppm_seen.add((conn, pm))
                if ok:
                    ppm_pass.add((conn, pm))

    return {
        "passed": passed,
        "total": total,
        "conns": len(conns_with_pass),
        "api": (len(api_pass), len(api_seen)),
        "pm": (len(pm_pass), len(pm_seen)),
        "papi": (len(papi_pass), len(papi_seen)),
        "ppm": (len(ppm_pass), len(ppm_seen)),
    }


def _rel(old, new):
    if old == 0:
        return "n/a"
    return f"{100.0 * (new - old) / old:+.0f}%"


def _frac(pair):
    s, t = pair
    pct = (100.0 * s / t) if t else 0.0
    return f"{s}/{t} ({pct:.0f}%)"


def _cov_pct(pair):
    s, t = pair
    return (100.0 * s / t) if t else 0.0


def render(base, curr, base_label, curr_label):
    out = []
    out.append(f"### Test Coverage Summary  ({base_label} → {curr_label})\n")
    out.append("| Metric | Base | Current | Change | Growth |")
    out.append("| --- | ---: | ---: | ---: | ---: |")

    # (label, base-cell, current-cell, change-cell, growth-cell)
    rows = []
    rows.append((
        "Passing tests",
        str(base["passed"]), str(curr["passed"]),
        f"{curr['passed'] - base['passed']:+d}", _rel(base["passed"], curr["passed"]),
    ))
    rows.append((
        "Connectors with ≥1 pass",
        str(base["conns"]), str(curr["conns"]),
        f"{curr['conns'] - base['conns']:+d}", _rel(base["conns"], curr["conns"]),
    ))
    rows.append((
        "API coverage (connector × flow)",
        _frac(base["api"]), _frac(curr["api"]),
        f"{curr['api'][0] - base['api'][0]:+d}", _rel(base["api"][0], curr["api"][0]),
    ))
    rows.append((
        "PMT coverage (connector × method)",
        _frac(base["pm"]), _frac(curr["pm"]),
        f"{curr['pm'][0] - base['pm'][0]:+d}", _rel(base["pm"][0], curr["pm"][0]),
    ))
    rows.append((
        "Production API coverage",
        _frac(base["papi"]), _frac(curr["papi"]),
        f"{curr['papi'][0] - base['papi'][0]:+d}", _rel(base["papi"][0], curr["papi"][0]),
    ))
    rows.append((
        "Production PMT coverage",
        _frac(base["ppm"]), _frac(curr["ppm"]),
        f"{curr['ppm'][0] - base['ppm'][0]:+d}", _rel(base["ppm"][0], curr["ppm"][0]),
    ))
    base_rate = (100.0 * base["passed"] / base["total"]) if base["total"] else 0.0
    curr_rate = (100.0 * curr["passed"] / curr["total"]) if curr["total"] else 0.0
    rows.append((
        "Overall run pass rate",
        f"{base_rate:.1f}% ({base['passed']}/{base['total']})",
        f"{curr_rate:.1f}% ({curr['passed']}/{curr['total']})",
        f"{curr_rate - base_rate:+.1f}pp", _rel(base_rate, curr_rate),
    ))

    for label, b, c, ch, gr in rows:
        out.append(f"| {label} | {b} | {c} | {ch} | {gr} |")

    out.append("")
    out.append("> **Unit:** coverage rows count `connector × flow` (API) and `connector × payment-method` (PMT)")
    out.append("> cells that have ≥1 passing scenario, over every such cell attempted — the same cross-product")
    out.append("> unit the capability skill uses, so denominators are the full grid, not a count of distinct")
    out.append("> flows/methods. SKIP runs and dependency setup runs are excluded. *Growth* is relative to base;")
    out.append("> the trustworthy signal is the **counts / Change**, since the % base shifts as the test matrix grows.")
    return "\n".join(out)


def main(argv=None):
    ap = argparse.ArgumentParser(description="Pass-based test coverage summary from two grpc CI reports.")
    ap.add_argument("base", help="Baseline report (file path or http(s) URL)")
    ap.add_argument("current", help="Current report (file path or http(s) URL)")
    ap.add_argument("--prod", default="", help="Comma-separated production connector roster (overrides default)")
    ap.add_argument("--base-label", default=None, help="Label for the baseline column header")
    ap.add_argument("--current-label", default=None, help="Label for the current column header")
    args = ap.parse_args(argv)

    production = [c.strip() for c in args.prod.split(",") if c.strip()] or DEFAULT_PRODUCTION

    base_runs = load_report(args.base)
    curr_runs = load_report(args.current)

    base = analyze(base_runs, production)
    curr = analyze(curr_runs, production)

    base_label = args.base_label or f"baseline ({args.base.split('/')[-1]})"
    curr_label = args.current_label or f"current ({args.current.split('/')[-1]})"

    print(render(base, curr, base_label, curr_label))
    return 0


if __name__ == "__main__":
    sys.exit(main())
