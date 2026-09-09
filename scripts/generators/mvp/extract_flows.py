#!/usr/bin/env python3
"""Per-connector implemented-flow extraction from UCS Rust source.

This is the ground truth for "does connector X implement flow Y". It exists
because none of the obvious alternatives is correct:

  - data/field_probe/*.json records whether a request could be BUILT, not what
    is implemented. It disagrees with source on ~130 cells; truelayer's Void
    reads not_implemented while the Rust plainly implements it.
  - connector_specs/*/specs.json is curated and can over-claim: CI's
    check_connector_specs asserts only flows subset-of supported_suites, never
    the reverse, so under-declaration is invisible (worldpayxml is missing
    PreAuthenticate today).
  - `cargo expand` / rustc trait enumeration is WORSE than source text:
    macro_connector_flow_status_impls! emits real ConnectorIntegrationV2 impls
    for every stub across the whole fleet, so the compiler's view is
    saturated with NotImplemented.

The algorithm is check_connector_specs.rs:172-246 with its three known defects
fixed (see FIXES below).
"""

from __future__ import annotations

import json
import re
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]
CONNECTORS = REPO_ROOT / "crates/integrations/connector-integration/src/connectors"

# Payout flows are wired as blanket no-op stubs for ~100 connectors by
# macro_connector_payout_implementation!, which takes no flow list at all.
# They are never a capability signal.
PAYOUT_PREFIX = "Payout"


def _balanced(text: str, macro: str):
    """Yield the parenthesised body of each `macro(...)` invocation."""
    i = 0
    while True:
        j = text.find(macro, i)
        if j < 0:
            return
        k = text.find("(", j)
        if k < 0:
            return
        depth, end = 0, k
        while end < len(text):
            if text[end] == "(":
                depth += 1
            elif text[end] == ")":
                depth -= 1
                if depth == 0:
                    break
            end += 1
        yield text[k : end + 1]
        i = j + 1


_FLOW_KV = re.compile(r"^\s*flow:\s*(\w+)", re.M)
_FLOW_NAME_KV = re.compile(r"flow_name:\s*(\w+)")
_IMPL_HEAD = re.compile(r"\bimpl\b")
_CIV2 = re.compile(r"ConnectorIntegrationV2<\s*(\w+)\s*,")
_STUB_LIST = re.compile(r"not_(?:implemented|supported):\s*\[([^\]]*)\]")
_IDENT = re.compile(r"\b([A-Z]\w*)")
_COMMENT = re.compile(r"//[^\n]*")


def _impl_header_flows(text: str) -> set:
    """Flows from hand-written `impl ConnectorIntegrationV2<F, ...> for X`.

    FIX 3: check_connector_specs uses a bare file-wide regex, which also matches
    `where Self: ConnectorIntegrationV2<Void, ...>` bounds (fiservcommercehub,
    razorpay). We only look between an `impl` keyword and the `for`/`{` that
    closes its header, so trait bounds in where-clauses cannot leak in.
    """
    found = set()
    for m in _IMPL_HEAD.finditer(text):
        # header ends at the `for` of `impl Trait for Type`, else at `{`
        window = text[m.end() : m.end() + 400]
        cut = len(window)
        for token in (" for ", "\nfor ", "{", ";", "where"):
            p = window.find(token)
            if p >= 0:
                cut = min(cut, p)
        for f in _CIV2.findall(window[:cut]):
            found.add(f)
    return found


def extract(connectors_dir: Path = CONNECTORS) -> dict:
    """{connector: sorted[flow]} for every connector, stubs removed."""
    out = {}
    for path in sorted(connectors_dir.glob("*.rs")):
        if path.name == "macros.rs":
            continue
        text = path.read_text(errors="ignore")

        positive = set()
        # Form 1 — the prerequisites macro declares request/response bridges.
        for body in _balanced(text, "create_all_prerequisites!"):
            positive |= set(_FLOW_KV.findall(body))
        # Form 2 — the real integration impls.
        real_impls = set()
        for body in _balanced(text, "macro_connector_implementation!"):
            real_impls |= set(_FLOW_NAME_KV.findall(body))
        positive |= real_impls
        # FIX 1 — locally-handled flows. kount and worldpayxml declare
        # PreAuthenticate only here, and neither file contains the string
        # "ConnectorIntegrationV2<", so both of the upstream regexes miss them.
        for body in _balanced(text, "macro_connector_local_flow_implementation!"):
            positive |= set(_FLOW_NAME_KV.findall(body))
        # Form 4 — hand-written impls. razorpay/razorpayv2 have no
        # create_all_prerequisites! at all and would otherwise read as empty.
        positive |= _impl_header_flows(text)

        # FIX 2 — subtract explicitly declared stubs. Without this, truelayer
        # reports Void, which it declares in the prerequisites macro and then
        # lists under not_implemented.
        stubs = set()
        for macro in (
            "macro_connector_flow_status_impls!",
            "impl_unsupported_connector_flow!",   # maya-local
            "frm_flow_not_implemented!",
        ):
            for body in _balanced(text, macro):
                for lst in _STUB_LIST.findall(body):
                    # Strip `//` comments FIRST. These lists carry prose such as
                    # "// Same /v1/reversals endpoint as Refund; deferred to a
                    # follow-up." — matching identifiers in that text wrongly
                    # marked boost/Refund and saferpay/Authorize as stubs, both
                    # of which have real impls.
                    stubs |= set(_IDENT.findall(_COMMENT.sub("", lst)))

        # A real `macro_connector_implementation!` always wins over a stub-list
        # mention. In this tree the two never genuinely intersect, so this is a
        # belt-and-braces guard against another parsing slip.
        stubs -= real_impls

        flows = {f for f in positive - stubs if not f.startswith(PAYOUT_PREFIX)}
        out[path.stem] = sorted(flows)
    return out


_NOT_IMPL_LIST = re.compile(r"not_implemented:\s*\[([^\]]*)\]")
_NOT_SUP_LIST = re.compile(r"not_supported:\s*\[([^\]]*)\]")


# A bare `impl connector_types::<Trait>V2 for X<T> {}` with an EMPTY body is a
# stub, not a capability — maya.rs:661 says so in words: "Stub implementations
# for service-trait bounds that are not exercised by Maya." The empty body means
# the trait default (which errors) stands. maya is the only connector that
# declares flows this way, and it is the sole reason a flow row fails to account
# for all 108 connectors.
_MARKER_IMPL = re.compile(
    r"impl\s*<[^>]*>\s*(?:\w+::)?(\w+?)V2(?:<[^>]*>)?\s+for\s+\w+<[^>]*>\s*\{\s*\}",
    re.S,
)


def _marker_stubs(text: str, known_flows) -> set:
    """Flows a connector stubs out via an empty marker impl."""
    out = set()
    for trait in _MARKER_IMPL.findall(text):
        # RefundVoidPostRefundV2 -> VoidPostRefund: match the longest known flow
        # that the trait name ends with, so prefixed traits resolve correctly.
        hits = [f for f in known_flows if trait.endswith(f)]
        if hits:
            out.add(max(hits, key=len))
    return out


def partition(connectors_dir: Path = CONNECTORS) -> dict:
    """{flow: {"impl": set, "not_impl": set, "not_sup": set}} — the compiler-forced split.

    ConnectorServiceTrait has no default methods, so the compiler forces every
    connector into exactly one bucket per flow it declares. That makes the
    extraction self-checking: for any flow the status macro covers, the three
    buckets must partition the whole connector set. A flow whose buckets do not
    sum means a declaration form was missed — which is the failure this whole
    module exists to prevent.
    """
    impl = extract(connectors_dir)
    buckets = {}
    for path in sorted(connectors_dir.glob("*.rs")):
        if path.name == "macros.rs":
            continue
        name = path.stem
        text = path.read_text(errors="ignore")
        for macro in ("macro_connector_flow_status_impls!", "impl_unsupported_connector_flow!"):
            for body in _balanced(text, macro):
                for rx, key in ((_NOT_IMPL_LIST, "not_impl"), (_NOT_SUP_LIST, "not_sup")):
                    for lst in rx.findall(body):
                        for flow in _IDENT.findall(_COMMENT.sub("", lst)):
                            buckets.setdefault(flow, {"impl": set(), "not_impl": set(), "not_sup": set()})
                            buckets[flow][key].add(name)
    known = set(buckets) | {f for v in impl.values() for f in v}
    for path in sorted(connectors_dir.glob("*.rs")):
        if path.name == "macros.rs":
            continue
        for flow in _marker_stubs(path.read_text(errors="ignore"), known):
            buckets.setdefault(flow, {"impl": set(), "not_impl": set(), "not_sup": set()})
            buckets[flow]["not_impl"].add(path.stem)

    for name, flows in impl.items():
        for flow in flows:
            buckets.setdefault(flow, {"impl": set(), "not_impl": set(), "not_sup": set()})
            buckets[flow]["impl"].add(name)
    # A connector cannot be both: a real impl outranks a stub-list mention.
    for flow, b in buckets.items():
        b["not_impl"] -= b["impl"]
        b["not_sup"] -= b["impl"] | b["not_impl"]
    return buckets


def assert_partitioned(buckets: dict, total: int, min_coverage: int | None = None) -> None:
    """Fail if a well-covered flow's three buckets do not account for every connector.

    Only flows the status macro covers broadly are checked: a flow declared by a
    handful of connectors and by no stub list legitimately does not partition.
    `maya` hand-writes its marker impls, so a small shortfall attributable to it
    alone is tolerated rather than masked.
    """
    # only flows the status macro covers broadly can partition
    min_coverage = min_coverage if min_coverage is not None else int(total * 0.92)
    problems = []
    for flow, b in sorted(buckets.items()):
        seen = b["impl"] | b["not_impl"] | b["not_sup"]
        if len(seen) < min_coverage:
            continue
        if len(seen) != total:
            problems.append(
                f"{flow}: {len(b['impl'])}+{len(b['not_impl'])}+{len(b['not_sup'])}"
                f"={len(seen)} != {total}"
            )
    if problems:
        raise SystemExit(
            "flow partition does not account for every connector — a declaration "
            "form was missed:\n  - " + "\n  - ".join(problems)
        )


def assert_healthy(flows: dict) -> None:
    """Fail loudly if the extraction silently degrades.

    Each assertion encodes a defect that was actually found in this tree, so a
    regression re-introduces a known-bad number rather than an obvious crash.
    """
    problems = []
    specs = REPO_ROOT / "crates/internal/integration-tests/src/connector_specs"
    expected = {d.name for d in specs.iterdir() if d.is_dir()} if specs.is_dir() else set()
    if expected:
        # connectors/*.rs and connector_specs/*/ are independent lists that CI
        # keeps in step; disagreement means one of them (or this parser) is wrong.
        missing, extra = expected - set(flows), set(flows) - expected
        if missing or extra:
            problems.append(
                f"connector set disagrees with connector_specs/ "
                f"(missing {sorted(missing)}, extra {sorted(extra)})"
            )
    elif len(flows) < 100:
        problems.append(f"only {len(flows)} connectors found — extraction likely broke")
    pairs = sum(len(v) for v in flows.values())
    per = pairs / max(len(flows), 1)
    if not 5.0 <= per <= 9.0:
        problems.append(
            f"{per:.1f} flows per connector ({pairs} pairs over {len(flows)}) "
            f"outside the sane 5-9 band — extraction likely broke"
        )
    if "PreAuthenticate" not in flows.get("kount", []):
        problems.append("kount lost PreAuthenticate — local-flow macro form not parsed")
    if "PreAuthenticate" not in flows.get("worldpayxml", []):
        problems.append("worldpayxml lost PreAuthenticate — local-flow macro form not parsed")
    if "Void" in flows.get("truelayer", []):
        problems.append("truelayer regained Void — stub subtraction not applied")
    if not flows.get("razorpay"):
        problems.append("razorpay empty — hand-written impls not parsed")
    if problems:
        raise SystemExit("flow extraction is unhealthy:\n  - " + "\n  - ".join(problems))


if __name__ == "__main__":
    f = extract()
    assert_healthy(f)
    assert_partitioned(partition(), total=len(f))
    from collections import Counter

    c = Counter(x for v in f.values() for x in v)
    print(f"✅ {len(f)} connectors · {sum(len(v) for v in f.values())} (connector, flow) pairs")
    for name, n in c.most_common(12):
        print(f"   {name:34s} {n}")
