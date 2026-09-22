#!/usr/bin/env python3
"""Money Framework CI lints for crates/integrations/connector-integration.

Lint 1: connector code must never import `proto_boundary` (the domain<->proto
raw-i64 boundary). That module is reserved for `domain_types` and `grpc-server`.

Lint 2: connector code must never declare a *struct field* of the opaque
domain `MinorUnit` type — connector wire structs use `ConnectorMinorUnit`,
`StringMinorUnit`, `StringMajorUnit`, `FloatMajorUnit`, `StringTwoDecimalUnit`,
obtained via `AmountConvertor`. This intentionally does NOT flag every bare
`MinorUnit` token in connector code: connectors legitimately read
domain-typed request fields (`request.minor_amount: MinorUnit`), compare
them (`amount == MinorUnit::default()`), pass them as function parameters
into local conversion helpers, and reference the type name in doc comments,
error-message strings, and the `amount_type: MinorUnit` / `amount_converter:
MinorUnit` macro-argument shorthand that `create_all_prerequisites!` /
`create_amount_converter_wrapper!` use to mean "wire type = ConnectorMinorUnit".
None of those construct a wire struct that would let raw MinorUnit reach the
network. Only an actual `pub field: MinorUnit` (or `Option<MinorUnit>`)
struct-field declaration does that — struct fields are the only place `pub`
can precede a `name: Type,` binding in Rust, which is what this lint keys on.

Both lints skip `#[cfg(test)]` module bodies (brace-matched, not just files
named test.rs/tests.rs) and `*/test.rs`, `*/tests.rs` files, since test code
is explicitly allowed to construct raw domain values to build fixtures.

Exit code 0 = clean, 1 = violations found (printed to stderr).
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

CONNECTOR_ROOT = Path("crates/integrations")

TEST_FILE_RE = re.compile(r"(^|/)(test|tests)\.rs$")
CFG_TEST_RE = re.compile(r"#\[cfg\(test\)\]")

PROTO_BOUNDARY_RE = re.compile(r"\bproto_boundary\b")

# A `pub` struct-field declaration typed (Option<)?MinorUnit(>)? — the only
# shape that actually embeds the opaque domain type into something that can
# be #[derive(Serialize)]'d onto the wire. `pub` cannot precede a function
# parameter in Rust, so this alone rules out helper-function signatures,
# doc comments, error strings, macro arguments (`amount_type: MinorUnit`),
# and domain-value comparisons (`amount == MinorUnit::default()`).
WIRE_FIELD_RE = re.compile(
    r"^pub(?:\([^)]*\))?\s+\w+\s*:\s*(Option<MinorUnit>|Vec<MinorUnit>|MinorUnit)\s*,?\s*$"
)

# Files that implement the money-framework machinery itself, not connector
# business logic — they legitimately take a domain MinorUnit as the *input*
# to a conversion routine.
FRAMEWORK_FILES = {
    "connectors/macros.rs",
    "common_macros.rs",
}


def strip_cfg_test_blocks(text: str) -> str:
    """Replace the body of every `#[cfg(test)] mod ... { ... }` block with
    blank lines (preserving line numbers for any future diagnostics), using
    brace matching rather than assuming tests sit at end-of-file."""
    out = []
    i = 0
    n = len(text)
    while i < n:
        m = CFG_TEST_RE.search(text, i)
        if not m:
            out.append(text[i:])
            break
        out.append(text[i : m.start()])
        # Find the next `{` after the attribute (skips `mod tests` etc.).
        brace_start = text.find("{", m.end())
        if brace_start == -1:
            out.append(text[m.start() :])
            break
        depth = 0
        j = brace_start
        while j < n:
            if text[j] == "{":
                depth += 1
            elif text[j] == "}":
                depth -= 1
                if depth == 0:
                    j += 1
                    break
            j += 1
        block = text[m.start() : j]
        out.append("\n" * block.count("\n"))
        i = j
    return "".join(out)


def iter_rs_files():
    for path in CONNECTOR_ROOT.rglob("*.rs"):
        if TEST_FILE_RE.search(str(path)):
            continue
        yield path


DERIVE_RE = re.compile(r"#\[derive\(([^)]*)\)\]")
STRUCT_RE = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?struct\s+\w")


def find_wire_field_violations(lines: list[str]) -> list[tuple[int, str]]:
    """Flag `pub field: MinorUnit` only when the enclosing struct's nearest
    preceding #[derive(...)] includes Serialize or Deserialize."""
    violations = []
    pending_derive_has_serde = False
    brace_depth = 0
    wire_struct_until_depth = None  # brace_depth at which the wire struct closes
    for lineno, raw in enumerate(lines, start=1):
        line = raw.strip()
        m = DERIVE_RE.search(line)
        if m:
            pending_derive_has_serde = bool(
                re.search(r"\bSerialize\b|\bDeserialize\b", m.group(1))
            )
        elif STRUCT_RE.match(line):
            if pending_derive_has_serde and wire_struct_until_depth is None:
                wire_struct_until_depth = brace_depth
            pending_derive_has_serde = False
        elif line and not line.startswith("#["):
            # Any other real code line clears a pending derive that never
            # attached to a struct (e.g. it was on an enum or fn instead).
            if not STRUCT_RE.match(line):
                pending_derive_has_serde = False

        if wire_struct_until_depth is not None and WIRE_FIELD_RE.match(line):
            violations.append((lineno, "MinorUnit wire-struct field"))

        brace_depth += raw.count("{") - raw.count("}")
        if wire_struct_until_depth is not None and brace_depth <= wire_struct_until_depth:
            wire_struct_until_depth = None
    return violations


def check_file(path: Path) -> list[tuple[int, str, str]]:
    violations = []
    text = path.read_text()
    stripped = strip_cfg_test_blocks(text)
    lines = stripped.splitlines()
    for lineno, line in enumerate(lines, start=1):
        if PROTO_BOUNDARY_RE.search(line):
            violations.append((lineno, "proto_boundary", line.strip()))
    if not any(str(path).endswith(f) for f in FRAMEWORK_FILES):
        for lineno, kind in find_wire_field_violations(lines):
            violations.append((lineno, kind, lines[lineno - 1].strip()))
    return violations


def main() -> int:
    had_violations = False
    for path in sorted(iter_rs_files()):
        for lineno, kind, line in check_file(path):
            had_violations = True
            print(f"{path}:{lineno}: {kind}: {line}", file=sys.stderr)
    if had_violations:
        print(
            "\nMoney Framework lint failed: connector code must not import "
            "proto_boundary or use the bare MinorUnit type. Use "
            "ConnectorMinorUnit / StringMinorUnit / StringMajorUnit / "
            "FloatMajorUnit via AmountConvertor instead.",
            file=sys.stderr,
        )
        return 1
    print("Money Framework lint: clean.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
