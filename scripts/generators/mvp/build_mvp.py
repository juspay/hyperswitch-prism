#!/usr/bin/env python3
"""Build mvp.json — UCS-only, source-derived connector readiness.

Scoring comes exclusively from UCS Rust source and the gRPC proto. The field
probe is carried as a clearly-labelled secondary signal ("request buildable")
and never scores anything: it disagrees with source on ~130 cells and four
connectors with permissive transformers falsely report 103 payment methods.

    python3 scripts/generators/mvp/build_mvp.py
"""

from __future__ import annotations

import argparse
import datetime
import json
import re
from pathlib import Path

import extract_flows as EF
import sources as S

REPO_ROOT = Path(__file__).resolve().parents[3]
DEFAULT_OUT = (
    REPO_ROOT / "grace/grace-workspace/packages/dashboard/src/data/mvp.json"
)
RUBRIC = Path(__file__).resolve().parent / "mvp-rubric.json"
PROBE = REPO_ROOT / "data" / "field_probe"

MET, GAP, UNKNOWN, ALIAS = "met", "gap", "unknown", "alias"
_STATE = {S.YES: MET, S.NO: GAP, S.UNKNOWN: UNKNOWN}


def evaluate(cap: dict, flows: dict) -> dict:
    """{connector: cell state} for one rubric capability."""
    sig = cap["signal"]
    kind = sig["type"]

    if kind == "flow":
        raw = S.flows_signal(flows, sig["flows"], sig.get("mode", "any"))
    elif kind == "trait":
        per = [S.trait_override(m) for m in sig["methods"]]
        mode = sig.get("mode", "any")
        raw = {}
        for c in S.connector_set():
            hits = [p[c] == S.YES for p in per]
            raw[c] = S.YES if (all(hits) if mode == "all" else any(hits)) else S.NO
    elif kind == "code":
        raw = S.code_signal(sig["pattern"], sig.get("reject"))
        if sig.get("close_unknown"):
            # A non-match is a provable NO here: the capability is structurally
            # impossible without a precondition the flow matrix already settles.
            raw = {c: (S.NO if v == S.UNKNOWN else v) for c, v in raw.items()}
    elif kind == "code_unless":
        raw = S.code_unless(sig["reject"])
    elif kind == "field_populated":
        raw = S.field_populated(sig["field"])
    elif kind == "three_ds":
        raw = S.three_ds_signal()
    elif kind == "alias":
        return {c: ALIAS for c in S.connector_set()}
    else:
        raise ValueError(f"unknown signal type {kind!r} for {cap['id']!r}")

    return {c: _STATE[v] for c, v in raw.items()}


def probe_secondary() -> dict:
    """Per-connector 'request buildable' counts. Secondary signal only.

    Probe filenames strip underscores, so `tsys_transit.rs` is
    `tsystransit.json`. Joining on the raw name silently drops three connectors.
    """
    by_norm = {}
    for f in sorted(PROBE.glob("*.json")):
        try:
            d = json.loads(f.read_text())
        except (json.JSONDecodeError, OSError):
            continue
        if isinstance(d.get("flows"), dict):
            by_norm[f.stem.replace("_", "").lower()] = d["flows"]

    out = {}
    for c in S.connector_set():
        fl = by_norm.get(c.replace("_", "").lower())
        if fl is None:
            out[c] = None
            continue
        buildable = sum(
            1
            for v in fl.values()
            if (v.get("default") or {}).get("status") == "supported"
        )
        out[c] = {"buildableFlows": buildable, "probedFlows": len(fl)}
    return out


def assert_no_hyperswitch() -> None:
    """No signal may read the separate hyperswitch repo."""
    bad = []
    for p in Path(__file__).resolve().parent.glob("*.py"):
        for m in re.finditer(r"[\"'](/[^\"']*hyperswitch[^\"']*)[\"']", p.read_text()):
            if "hyperswitch-prism" not in m.group(1):
                bad.append(f"{p.name}: {m.group(1)}")
    if bad:
        raise SystemExit("hyperswitch paths leaked into UCS-only sources:\n  " + "\n  ".join(bad))


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--out", type=Path, default=DEFAULT_OUT)
    args = ap.parse_args()

    assert_no_hyperswitch()
    flows = EF.extract()
    EF.assert_healthy(flows)

    rubric = json.loads(RUBRIC.read_text())
    caps = rubric["capabilities"]
    probe = probe_secondary()
    cells = {cap["id"]: evaluate(cap, flows) for cap in caps}

    # Only proven fields score. A best-effort signal is shown because the
    # information is useful, but letting it move met/gaps/effort would put an
    # unprovable number inside one presented as provable.
    scored_caps = [c for c in caps if c.get("confidence", "proven") == "proven"]

    rows = []
    for c in S.connector_set():
        row = {cap["id"]: cells[cap["id"]][c] for cap in caps}
        met = sum(1 for cap in scored_caps if row[cap["id"]] == MET)
        gap = sum(1 for cap in scored_caps if row[cap["id"]] == GAP)
        effort = sum(cap["weight"] for cap in scored_caps if row[cap["id"]] == GAP)
        scored = met + gap
        rows.append({
            "name": c,
            "cells": row,
            "flows": flows[c],
            "probe": probe[c],
            "met": met,
            "gaps": gap,
            "scored": scored,
            "effort": effort,
            "pct": round(100 * met / scored) if scored else 0,
        })

    rows.sort(key=lambda r: (-r["pct"], r["effort"], r["name"]))
    payload = {
        "generatedAt": datetime.datetime.now().isoformat(timespec="seconds"),
        "source": "UCS only (hyperswitch-prism). Scored from Rust source; the field probe is a secondary signal.",
        "provenance": S.git_provenance(REPO_ROOT) if hasattr(S, "git_provenance") else None,
        "capabilities": [
            {k: c[k] for k in ("id", "label", "description", "weight")}
            | {"signal": c["signal"], "confidence": c.get("confidence", "proven")}
            for c in caps
        ],
        "connectors": rows,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(payload, indent=2) + "\n")

    from collections import Counter
    print(f"✅ {len(rows)} connectors × {len(caps)} capabilities "
          f"({len(scored_caps)} proven, {len(caps) - len(scored_caps)} best-effort) → {args.out}")
    print(f"   at MVP: {sum(1 for r in rows if r['gaps'] == 0)} · "
          f"gaps {sum(r['gaps'] for r in rows)} · effort {sum(r['effort'] for r in rows)} pts")


if __name__ == "__main__":
    main()
