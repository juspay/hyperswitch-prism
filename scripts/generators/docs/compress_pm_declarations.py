#!/usr/bin/env python3
"""Compress connector `SupportedPaymentMethods` declarations using the
`build_supported_pms!` macro.

For each connector source file:

  1. Locate the `static <NAME>_SUPPORTED_PAYMENT_METHODS: LazyLock<...> =
     LazyLock::new(|| { ... });` block via brace counting.
  2. Parse every entry inside (both `.add(...)` and
     `.entry(...).or_default().insert(...)` styles).
  3. Classify each entry as DEFAULT (uses conservative
     mandates/refunds/capture/specific_features) or CUSTOM (any field
     deviates).
  4. Rewrite the LazyLock body so DEFAULT entries collapse into a single
     `build_supported_pms!` invocation grouped by status, while CUSTOM
     entries remain as explicit `m.entry(...).or_default().insert(...)`
     calls after the macro.

Goal: ~10× reduction in declaration LoC with byte-identical runtime
semantics. Verify via snapshot diff on `data/connector_capabilities/`.

Usage:
    python3 scripts/generators/docs/compress_pm_declarations.py            # dry-run
    python3 scripts/generators/docs/compress_pm_declarations.py --apply    # mutate sources
    python3 scripts/generators/docs/compress_pm_declarations.py stripe     # subset
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]
SRC_DIR = REPO_ROOT / "crates" / "integrations" / "connector-integration" / "src" / "connectors"


# ─── Block locator ───────────────────────────────────────────────────────────


def find_lazylock_body(text: str) -> tuple[int, int, int, int] | None:
    """Find the LazyLock body for the SUPPORTED_PAYMENT_METHODS static.

    Returns `(decl_start, body_open, body_close, decl_end)` byte offsets:
      - decl_start: start of `static <NAME>_SUPPORTED_PAYMENT_METHODS:`
      - body_open:  index of the `{` opening the `LazyLock::new(|| { ... }` closure
      - body_close: index of the matching `}` closing the closure
      - decl_end:   index just past the trailing `;` of the static

    The closure body sits at `text[body_open:body_close+1]`.
    """
    decl = re.search(
        r"static\s+([A-Z][A-Z_0-9]*_SUPPORTED_PAYMENT_METHODS)\s*:[^=]*=\s*",
        text,
    )
    if not decl:
        return None
    decl_start = decl.start()

    # Find LazyLock::new( after the static decl
    after = text.find("LazyLock::new(", decl.end())
    if after < 0:
        return None

    # Find the `||` closure
    close_open = text.find("||", after)
    if close_open < 0:
        return None

    # The first `{` after `||` is the closure body open
    body_open = text.find("{", close_open)
    if body_open < 0:
        return None

    # Walk balanced braces to find body_close
    depth = 1
    i = body_open + 1
    while i < len(text) and depth > 0:
        c = text[i]
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
        i += 1
    if depth != 0:
        return None
    body_close = i - 1

    # Walk forward to consume `})` (closing LazyLock::new's paren) then `;`
    j = body_close + 1
    while j < len(text) and text[j] not in (")",):
        j += 1
    if j < len(text):
        j += 1  # consume `)`
    # Consume optional outer `}` (hyperpg-style `static FOO = { LazyLock::new(...) };`)
    while j < len(text) and text[j] in " \n\t":
        j += 1
    # Consume `;`
    while j < len(text) and text[j] != ";":
        j += 1
    if j < len(text):
        j += 1
    return (decl_start, body_open, body_close, j)


# ─── Entry parser ────────────────────────────────────────────────────────────


# Match an entry: either `<local>.add(PM, PMT, Details);` or
# `<local>.entry(PM).or_default().insert(PMT, Details);`
# We accept any qualifier prefix on PaymentMethod::X / PaymentMethodType::X.

ENTRY_HEAD_PATTERN = re.compile(
    r"(?P<local>\w+)\s*\.\s*"
    r"(?:"
    r"    add\(\s*"
    r"        (?:[\w:]*::)?PaymentMethod::(?P<pm_a>[A-Za-z][A-Za-z0-9]*)\s*,\s*"
    r"        (?:[\w:]*::)?PaymentMethodType::(?P<pmt_a>[A-Za-z][A-Za-z0-9]*)\s*,\s*"
    r"    |"
    r"    entry\(\s*(?:[\w:]*::)?PaymentMethod::(?P<pm_b>[A-Za-z][A-Za-z0-9]*)\s*\)"
    r"        \s*\.\s*or_default\(\)"
    r"        \s*\.\s*insert\(\s*"
    r"            (?:[\w:]*::)?PaymentMethodType::(?P<pmt_b>[A-Za-z][A-Za-z0-9]*)\s*,\s*"
    r")"
    r"(?:[\w:]*::)?PaymentMethodDetails\s*\{",
    re.VERBOSE,
)


def find_next_entry(body: str, from_pos: int) -> dict | None:
    """Scan forward from `from_pos`. Return the next entry's parsed fields.

    Returns a dict:
      {
        'start': absolute offset of the entry's first character,
        'end':   absolute offset just past the entry's trailing `;\\n` (or `;`),
        'local_var': name of the local mutable variable
                     (e.g. `m`, `cashfree_supported_payment_methods`),
        'pm':    PaymentMethod variant (e.g. 'Card'),
        'pmt':   PaymentMethodType variant (e.g. 'BancontactCard'),
        'status': FeatureStatus variant string (e.g. 'Supported'),
        'fields_default': True iff mandates/refunds/capture/specific_features
                          all match the conservative-default shape,
        'raw': verbatim entry source (used when we keep the entry as-is),
      }
    or None when no more entries.
    """
    m = ENTRY_HEAD_PATTERN.search(body, from_pos)
    if not m:
        return None

    pm = m.group("pm_a") or m.group("pm_b")
    pmt = m.group("pmt_a") or m.group("pmt_b")
    local_var = m.group("local")

    # The matched head ends just before the `{` of PaymentMethodDetails.
    # Walk balanced braces to find the matching `}` of the struct.
    struct_open = body.find("{", m.start())
    if struct_open < 0:
        return None
    depth = 1
    i = struct_open + 1
    while i < len(body) and depth > 0:
        c = body[i]
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
        i += 1
    if depth != 0:
        return None
    struct_close = i  # one past the matching `}`

    # Consume the `)` closing the outer call and the trailing `;\n`
    j = struct_close
    while j < len(body) and body[j] not in (")",):
        j += 1
    if j < len(body):
        j += 1
    while j < len(body) and body[j] in " \t":
        j += 1
    if j < len(body) and body[j] == ";":
        j += 1
    if j < len(body) and body[j] == "\n":
        j += 1

    inner = body[struct_open + 1 : struct_close - 1]
    fields = parse_fields(inner)
    fields_default = is_default_shape(fields)

    raw = body[m.start() : j]

    return {
        "start": m.start(),
        "end": j,
        "local_var": local_var,
        "pm": pm,
        "pmt": pmt,
        "status": fields.get("status", "Unknown"),
        "fields_default": fields_default,
        "raw": raw,
    }


def parse_fields(struct_body: str) -> dict[str, str]:
    """Parse a `PaymentMethodDetails { ... }` body into {field_name: raw_value_text}."""
    out: dict[str, str] = {}
    # Match `name: <value-up-to-next-top-level-comma>`. Naïve but works because
    # the bootstrap-generated entries have flat fields.
    # We track brace depth to handle nested values (e.g. `specific_features: Some(...)`).
    pos = 0
    depth = 0
    field_start = 0
    current_name: str | None = None
    name_buf = ""
    in_name = True
    value_start = 0

    # Simpler: split on top-level commas, then each piece is `name: value`.
    pieces: list[str] = []
    start = 0
    for i, c in enumerate(struct_body):
        if c in "({[":
            depth += 1
        elif c in ")}]":
            depth -= 1
        elif c == "," and depth == 0:
            piece = struct_body[start:i].strip()
            if piece:
                pieces.append(piece)
            start = i + 1
    tail = struct_body[start:].strip()
    if tail:
        pieces.append(tail)

    for p in pieces:
        if ":" not in p:
            continue
        name, _, value = p.partition(":")
        name = name.strip()
        value = value.strip()
        out[name] = value
        # Extract just the FeatureStatus variant tail for status/mandates/refunds
        if name in ("status", "mandates", "refunds"):
            tail_match = re.search(r"FeatureStatus::(\w+)$", value)
            if tail_match:
                out[name] = tail_match.group(1)
    return out


# Patterns the conservative-default macro emits for each field:
DEFAULT_MANDATES = "NotImplemented"
DEFAULT_REFUNDS = "NotImplemented"
# capture variants accepted as "default" (single Automatic)
DEFAULT_CAPTURE_PATTERNS = [
    re.compile(r"^vec!\[\s*(?:[\w:]*::)?CaptureMethod::Automatic\s*,?\s*\]$"),
    re.compile(r"^default_capture\.clone\(\)$"),
    re.compile(r"^supported_capture_methods\s*\.clone\(\)?$"),  # fallback if connector reuses local
]
DEFAULT_SPECIFIC_FEATURES = "None"


def is_default_shape(fields: dict[str, str]) -> bool:
    """True iff the PaymentMethodDetails body matches the conservative-default shape."""
    if fields.get("mandates") != DEFAULT_MANDATES:
        return False
    if fields.get("refunds") != DEFAULT_REFUNDS:
        return False
    capture_raw = fields.get("supported_capture_methods", "")
    if not any(p.match(capture_raw) for p in DEFAULT_CAPTURE_PATTERNS):
        return False
    if fields.get("specific_features", "").strip() != DEFAULT_SPECIFIC_FEATURES:
        return False
    return True


# ─── Rewriter ────────────────────────────────────────────────────────────────


def rebuild_body(entries: list[dict], local_var_hint: str) -> str:
    """Given parsed entries, emit a new LazyLock closure body."""
    default_by_status: dict[str, list[tuple[str, str]]] = {}
    custom: list[dict] = []
    for e in entries:
        if e["fields_default"]:
            default_by_status.setdefault(e["status"], []).append((e["pm"], e["pmt"]))
        else:
            custom.append(e)

    # The local var name to use for the rest of the closure.
    # Use the first entry's local var name for continuity, falling back to `m`.
    local = entries[0]["local_var"] if entries else "m"

    lines: list[str] = []
    if default_by_status:
        lines.append(
            f"        let mut {local} = domain_types::build_supported_pms! {{"
        )
        # Emit status groups in a deterministic order: Supported first, then NotImplemented, then anything else.
        status_order = ["Supported", "NotImplemented"] + sorted(
            s for s in default_by_status if s not in ("Supported", "NotImplemented")
        )
        for status in status_order:
            if status not in default_by_status:
                continue
            pairs = default_by_status[status]
            lines.append(f"            {status} => [")
            for pm, pmt in pairs:
                lines.append(f"                ({pm}, {pmt}),")
            lines.append("            ],")
        lines.append("        };")
    else:
        # No default entries — start with an empty map.
        lines.append(
            f"        let mut {local} = domain_types::types::SupportedPaymentMethods::new();"
        )

    # Custom entries: emit verbatim, but ensure their local var name matches.
    for e in custom:
        raw = e["raw"]
        # If the custom entry used a different local var name, rewrite the leading identifier.
        # (The macro output uses `local` from the first entry; subsequent customs need to match.)
        # Simple substitution at the start of the entry.
        if not raw.lstrip().startswith(local):
            raw = re.sub(
                r"^\s*\w+",
                lambda mm: mm.group(0).replace(e["local_var"], local),
                raw,
                count=1,
            )
        lines.append("")
        for ln in raw.rstrip("\n").splitlines():
            lines.append(ln if ln.startswith("        ") else "        " + ln.lstrip())

    lines.append(f"        {local}")
    return "\n".join(lines)


def compress_one_file(path: Path, do_write: bool) -> tuple[int, int, int]:
    """Returns (entries_total, defaulted, custom). Mutates file if do_write."""
    text = path.read_text(encoding="utf-8")
    span = find_lazylock_body(text)
    if span is None:
        return (0, 0, 0)

    decl_start, body_open, body_close, decl_end = span
    body = text[body_open + 1 : body_close]

    # Bail if the body contains a `for ... in` loop or any other dynamic
    # construct that produces entries through loop variables. My regex parses
    # only `PaymentMethod::Foo` literal identifiers — entries driven by loop
    # variables (e.g. PPro's BankRedirect loop, Razorpay's UPI loop) would
    # be silently dropped during migration. Treat such files as
    # "all-custom" and leave them untouched.
    if re.search(r"\bfor\s+\w[\w_]*\s+in\b", body) or re.search(r"\.iter\(\)", body):
        return (0, 0, 0)

    # Parse all entries
    entries: list[dict] = []
    pos = 0
    while True:
        e = find_next_entry(body, pos)
        if e is None:
            break
        entries.append(e)
        pos = e["end"]

    if not entries:
        return (0, 0, 0)

    n_default = sum(1 for e in entries if e["fields_default"])
    n_custom = len(entries) - n_default

    if n_default == 0:
        # Nothing to compress.
        return (len(entries), 0, n_custom)

    # Determine the prefix and suffix surrounding the body — anything before
    # the first entry (e.g. setup like `let default_capture = vec![...];`) and
    # anything after the last entry (e.g. the trailing return identifier).
    body_pre = body[: entries[0]["start"]].rstrip()
    body_post_tail = body[entries[-1]["end"] :].lstrip()

    # We discard body_pre's `let default_capture = ...;` if all default entries
    # are now in the macro (the macro inlines the capture vec itself). Detect
    # and strip such an assignment to avoid `unused_variables` warnings.
    body_pre = re.sub(
        r"let\s+default_capture\s*=\s*vec!\[[^\]]*\]\s*;\s*\n?",
        "",
        body_pre,
    )

    # The trailing `m` (or `<local_var>`) return is captured here; drop it
    # since rebuild_body emits its own trailing return identifier.
    body_post_tail = re.sub(
        r"^\s*[a-z][a-z0-9_]*\s*$",
        "",
        body_post_tail,
        flags=re.MULTILINE,
    ).rstrip()

    new_body_inner = rebuild_body(entries, entries[0]["local_var"])
    new_body = ""
    if body_pre.strip():
        new_body += body_pre.rstrip() + "\n"
    new_body += new_body_inner + "\n"
    if body_post_tail.strip():
        new_body += "    " + body_post_tail.strip() + "\n"

    new_text = (
        text[: body_open + 1]
        + "\n"
        + new_body
        + "    "
        + text[body_close:]
    )

    if do_write:
        path.write_text(new_text, encoding="utf-8")
    return (len(entries), n_default, n_custom)


def main(argv: list[str]) -> int:
    apply = "--apply" in argv
    targets = {a for a in argv if not a.startswith("-")}

    files = sorted(SRC_DIR.glob("*.rs"))
    if targets:
        files = [f for f in files if f.stem in targets or f.stem.replace("_", "") in targets]

    print(f"{'connector.rs':<24s} {'entries':>8s} {'default':>8s} {'custom':>7s}")
    print("-" * 55)
    total_entries = total_def = total_cust = 0
    touched = 0
    for f in files:
        e, d, c = compress_one_file(f, do_write=apply)
        if e == 0:
            continue
        if d:
            touched += 1
        print(f"  {f.name:<22s} {e:>8d} {d:>8d} {c:>7d}")
        total_entries += e
        total_def += d
        total_cust += c

    print()
    print(
        f"Totals: {touched} files {'mutated' if apply else 'would be mutated'}; "
        f"{total_entries} entries ({total_def} default → macro, "
        f"{total_cust} custom → preserved)."
    )
    if not apply:
        print("Dry run — pass --apply to mutate sources.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
