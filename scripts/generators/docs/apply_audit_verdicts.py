#!/usr/bin/env python3
"""Apply Change 10 audit verdicts back to connector source files.

Reads `data/audit.json` (workflow output from the source-vs-declaration
audit) and edits each connector's source `.rs`:

  - `remove`           (confidence ≥ medium): strip the entry block.
  - `downgrade_to_ni`  (confidence ≥ medium): change `status:
                        FeatureStatus::Supported` →
                        `status: FeatureStatus::NotImplemented` for that
                        (PM, PMType) entry.
  - `keep` or `ambiguous` or `low`-confidence: no-op.

The script handles two declaration shapes interchangeably:
  - `<local>.add(PaymentMethod::X, PaymentMethodType::Y, PaymentMethodDetails { ... });`
  - `<local>.entry(PaymentMethod::X).or_default().insert(PaymentMethodType::Y, PaymentMethodDetails { ... });`

For both shapes, the script anchors on the PM/PMType pair and the
`PaymentMethodDetails { ... }` block immediately after.

Usage:
    python3 scripts/generators/docs/apply_audit_verdicts.py            # dry-run
    python3 scripts/generators/docs/apply_audit_verdicts.py --apply    # mutate sources
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]
AUDIT = REPO_ROOT / "data" / "audit.json"
SRC_DIR = REPO_ROOT / "crates" / "integrations" / "connector-integration" / "src" / "connectors"

# Probe-stem → source-file-stem (some probe JSON names drop underscores
# the source filename keeps).
STEM_MAP: dict[str, str] = {
    "absasanlam": "absa_sanlam",
    "pinelabsonline": "pinelabs_online",
    "twoctwoppaco": "twoc_twop_paco",
}


def find_entry_block(src: str, pm: str, pmt: str) -> tuple[int, int] | None:
    """Locate the source span of a `(pm, pmt)` entry.

    Returns `(start, end)` byte offsets of the full block including the
    trailing semicolon and newline, or `None` if not found.

    Matches both `.add(...)` and `.entry(...).or_default().insert(...)`
    declaration shapes, with any qualifier prefix on the enum paths
    (`PaymentMethod::X`, `enums::PaymentMethod::X`,
    `common_enums::enums::PaymentMethod::X`).
    """
    # Allow optional path qualifier before `PaymentMethod::` and `PaymentMethodType::`.
    pm_pat = rf"(?:[\w:]*::)?PaymentMethod::{re.escape(pm)}"
    pmt_pat = rf"(?:[\w:]*::)?PaymentMethodType::{re.escape(pmt)}"

    # Pattern A: `<local>.add(\n <PM>,\n <PMType>,\n <Details>{...},\n);`
    pattern_a = re.compile(
        r"(\w+)\.add\(\s*\n"
        r"\s*" + pm_pat + r",\s*\n"
        r"\s*" + pmt_pat + r",\s*\n"
        r"\s*(?:[\w:]*::)?PaymentMethodDetails\s*\{",
        re.MULTILINE,
    )
    # Pattern B: `<local>.entry(<PM>).or_default().insert(\n <PMType>,\n <Details>{...},\n);`
    # Some bootstrap outputs put .insert(\n PMType,\n PaymentMethodDetails { ...
    # others put .insert(PMType,\n PaymentMethodDetails { ...
    pattern_b = re.compile(
        r"(\w+)\s*\n?\s*\.entry\(" + pm_pat + r"\)\s*\n?"
        r"\s*\.or_default\(\)\s*\n?"
        r"\s*\.insert\(\s*\n?"
        r"\s*" + pmt_pat + r",\s*\n"
        r"\s*(?:[\w:]*::)?PaymentMethodDetails\s*\{",
        re.MULTILINE,
    )

    m = pattern_a.search(src) or pattern_b.search(src)
    if m is None:
        return None

    # Now walk forward from the `{` of PaymentMethodDetails to find the
    # matching `}`, then expect `,\n   );\n` or similar.
    open_brace = src.find("{", m.start())
    if open_brace < 0:
        return None
    depth = 1
    i = open_brace + 1
    while i < len(src) and depth > 0:
        ch = src[i]
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
        i += 1
    if depth != 0:
        return None
    # i now points just past the matching `}` of PaymentMethodDetails.
    # Expect optional `,` then closing `)` of the outer call (`.add` or `.insert`),
    # then `;` and a trailing newline.
    close = src.find(")", i)
    if close < 0:
        return None
    end = src.find(";", close)
    if end < 0:
        return None
    end += 1
    # Include the trailing newline if present.
    if end < len(src) and src[end] == "\n":
        end += 1
    return (m.start(), end)


def downgrade_status_in_block(src: str, start: int, end: int) -> str:
    """Replace `status: FeatureStatus::Supported` (with any qualifier
    prefix) → `status: FeatureStatus::NotImplemented` inside the block
    `src[start:end]`. Leaves other fields untouched.
    """
    block = src[start:end]
    new_block, n = re.subn(
        r"status:\s*(?:[\w:]*::)?FeatureStatus::Supported\b",
        lambda m: m.group(0).replace("Supported", "NotImplemented"),
        block,
        count=1,
    )
    if n == 0:
        # Not a Supported entry — nothing to downgrade. Leave it alone.
        return src
    return src[:start] + new_block + src[end:]


def apply_connector(connector: dict, do_write: bool) -> tuple[int, int, int]:
    """Apply verdicts for one connector. Returns (removed, downgraded, skipped)."""
    name = connector["connector"]
    source_stem = STEM_MAP.get(name, name)
    path = SRC_DIR / f"{source_stem}.rs"
    if not path.exists():
        print(f"  {name:<14s} SKIP: source {path.name} not found")
        return (0, 0, len(connector["verdicts"]))

    src = path.read_text(encoding="utf-8")
    removed = downgraded = skipped = 0

    # Sort verdicts: do REMOVES before DOWNGRADES, in reverse-position order
    # so earlier removals don't shift later block offsets. Easiest: rebuild
    # offsets after each edit.
    actionable = [
        v for v in connector["verdicts"]
        if v["verdict"] in ("remove", "downgrade_to_ni")
        and v["confidence"] in ("high", "medium")
    ]

    for v in actionable:
        pm, pmt = v["pm"], v["pmt"]
        span = find_entry_block(src, pm, pmt)
        if span is None:
            skipped += 1
            continue
        start, end = span
        if v["verdict"] == "remove":
            src = src[:start] + src[end:]
            removed += 1
        elif v["verdict"] == "downgrade_to_ni":
            new_src = downgrade_status_in_block(src, start, end)
            if new_src == src:
                skipped += 1
            else:
                src = new_src
                downgraded += 1

    if (removed or downgraded) and do_write:
        path.write_text(src, encoding="utf-8")

    return (removed, downgraded, skipped)


def main(argv: list[str]) -> int:
    do_write = "--apply" in argv

    if not AUDIT.exists():
        print(f"Error: {AUDIT} not found. Run the audit workflow first.", file=sys.stderr)
        return 1
    audit = json.loads(AUDIT.read_text(encoding="utf-8"))

    print(f"{'connector':<22s} {'mode':<7s} {'removed':>8s} {'dgni':>5s} {'skip':>5s}")
    print("-" * 55)
    total_r = total_d = total_s = 0
    for c in audit:
        r, d, s = apply_connector(c, do_write=do_write)
        if r or d or s:
            verb = "WROTE" if do_write else "DRY-RUN"
            print(f"  {c['connector']:<20s} {verb:<7s} {r:>8d} {d:>5d} {s:>5d}")
        total_r += r
        total_d += d
        total_s += s

    print()
    print(
        f"Totals: {total_r} removed, {total_d} downgraded, {total_s} skipped "
        f"({'mutated' if do_write else 'dry run — pass --apply'})."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
