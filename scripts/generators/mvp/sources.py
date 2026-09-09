"""UCS-only capability signals for the MVP dashboard.

HARD RULE: nothing in this file may read the separate hyperswitch repo. UCS and
hyperswitch are different codebases with different connector implementations, so
a hyperswitch capability says nothing about the UCS connector of the same name.
build_mvp.py asserts this.

Every signal here is derived from UCS source text. The field probe
(data/field_probe/) is deliberately NOT a scoring source — it records whether a
request could be BUILT, not what is implemented.
"""

from __future__ import annotations

import json
import re
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]
CONNECTORS = REPO_ROOT / "crates/integrations/connector-integration/src/connectors"
SPECS = REPO_ROOT / "crates/internal/integration-tests/src/connector_specs"

YES, NO, UNKNOWN = "yes", "no", "unknown"


def connector_set() -> list:
    """The canonical connector set. `connector_specs/` dirs and `*.rs` files match
    exactly, and extract_flows.assert_healthy cross-checks that they still do."""
    return sorted(p.stem for p in CONNECTORS.glob("*.rs") if p.stem != "macros")


def _sources(name: str) -> str:
    """A connector's full source: its .rs plus everything in its subdirectory."""
    parts = []
    top = CONNECTORS / f"{name}.rs"
    if top.is_file():
        parts.append(top.read_text(errors="ignore"))
    sub = CONNECTORS / name
    if sub.is_dir():
        for f in sorted(sub.rglob("*.rs")):
            parts.append(f.read_text(errors="ignore"))
    return "\n".join(parts)


# --- trait-override signals ---------------------------------------------------

_STUB_MARKERS = ("NotImplemented", "not_implemented", "unimplemented!", "WebhooksNotImplemented")


def trait_override(method: str) -> dict:
    """{connector: yes|no} — overrides `method` with a REAL body.

    Overriding is not implementing. 2 of the 9 process_dispute_webhook overrides
    (phonepe, ppro) return WebhooksNotImplemented with every parameter
    underscore-prefixed. Counting those would overstate dispute support by 29%.
    """
    out = {}
    for c in connector_set():
        text = _sources(c)
        idx = text.find(f"fn {method}(")
        if idx < 0:
            out[c] = NO
            continue
        body = text[idx : idx + 1600]
        # Cut at the end of the fn so we don't read the next one.
        end = body.find("\n    }")
        if end > 0:
            body = body[:end]
        out[c] = NO if any(m in body for m in _STUB_MARKERS) else YES
    return out


# --- source-pattern signals ---------------------------------------------------

def code_signal(pattern: str, reject_pattern: str | None = None) -> dict:
    """{connector: yes|no|unknown} from source-text patterns.

    `pattern` proves support. `reject_pattern` proves the opposite. A connector
    matching neither is UNKNOWN — never silently NO, because "the source does
    not say" and "the source says no" are different facts.

    Rejection is checked FIRST and wins. A connector can both name a capability
    and refuse it: tsys binds the network transaction id and then assigns it to
    `_cit_reference`, never sending it — its own comment says "not sent to TSYS
    today". Checking the positive pattern first would score that as support.
    """
    rx = re.compile(pattern)
    rej = re.compile(reject_pattern) if reject_pattern else None
    out = {}
    for c in connector_set():
        text = _sources(c)
        if rej and rej.search(text):
            out[c] = NO
        elif rx.search(text):
            out[c] = YES
        else:
            out[c] = UNKNOWN
    return out


def flows_signal(flows: dict, wanted, mode: str = "any") -> dict:
    """{connector: yes|no} from the extracted flow matrix.

    Flow declarations partition the whole fleet exhaustively — every connector declares
    not_implemented/not_supported lists — so absence here is a provable NO,
    not an unknown.
    """
    want = [wanted] if isinstance(wanted, str) else list(wanted)
    out = {}
    for c, have in flows.items():
        hit = [w in have for w in want]
        ok = all(hit) if mode == "all" else any(hit)
        out[c] = YES if ok else NO
    return out


def code_unless(reject_pattern: str) -> dict:
    """{connector: yes|no} — YES unless the source explicitly refuses.

    For capabilities where the UCS default IS the capability. CaptureMethod
    defaults to Automatic in Rust, so "supports auto capture" is the absence of
    a rejection rather than a positive assertion — stated as such on the
    dashboard so nobody reads a near-total count as that many positive confirmations.
    """
    rej = re.compile(reject_pattern)
    return {c: (NO if rej.search(_sources(c)) else YES) for c in connector_set()}


def field_populated(field: str) -> dict:
    """{connector: yes|no} — does the connector ever set `field` to a real value?

    Written as code, not a regex: `field:\\s*(?!None)` looks right and is wrong,
    because `\\s*` can match zero characters and the lookahead then succeeds at
    the space before `None`. That scored nearly every connector as populating
    network_advice_code when the true number is 8.

    Two population idioms both count:
      - struct literal:  `network_advice_code: response.advice_code`
      - let binding then shorthand init, which is what checkout, cybersource,
        barclaycard and bankofamerica use:
            let network_advice_code = processor_information.as_ref()...;
            ... ErrorResponse { network_advice_code, .. }
        A struct-literal-only regex sees just the `: None` sites and scores all
        four as NO.

    JSON test fixtures (`"network_advice_code": null`) are excluded — razorpay's
    only occurrences are fixtures, so it is a NO despite three textual hits.
    """
    lit = re.compile(rf"(?<![\"']){re.escape(field)}\s*:\s*([A-Za-z_][\w:]*)")
    binding = re.compile(rf"\blet\s+{re.escape(field)}\s*=")
    out = {}
    for c in connector_set():
        src = _sources(c)
        vals = {m.group(1) for m in lit.finditer(src)}
        out[c] = YES if (vals - {"None"}) or binding.search(src) else NO
    return out


def three_ds_signal() -> dict:
    """{connector: yes|no|unknown} for native 3DS, ranked by evidence strength.

    A single pattern cannot separate "refuses 3DS entirely" from "refuses it on
    one path while supporting it on another" — paysafe, zift and moneris all
    carry a 3DS rejection somewhere AND genuinely support 3DS. So the rule is
    ordered by how direct the evidence is:

      1. STRONG positive — reads request.authentication_data (the cryptogram
         itself) or runs the AuthenticationStep dispatcher. This outranks a
         rejection: you cannot forward a CAVV you do not accept.
      2. Explicit rejection — an error whose message names 3DS.
      3. WEAK positive — maps processor statuses that name 3DS (maya's
         3DS_PAYMENT_SUCCESS). The connector never models 3DS itself.
      4. Structural NO — no Card handling at all (3DS is card-only), or Card
         with neither a challenge redirect nor any cryptogram field.
      5. UNKNOWN — emits a generic processor redirect URL. UCS cannot tell
         whether 3DS runs behind it; closing these needs connector API docs.

    Note `is_three_ds()` is NOT positive evidence on its own: it is equally the
    guard used to REJECT 3DS (tsys, payconex both branch on it to error).
    """
    strong = re.compile(r"request\.authentication_data|AuthenticationStep")
    rej = re.compile(r"(?:NotImplemented|NotSupported)[\s\S]{0,160}?(?i:3ds|three_?ds)")
    weak = re.compile(r'"[A-Z0-9_]*3DS[A-Z0-9_]*"|AuthenticationType::ThreeDs')
    card = re.compile(r"PaymentMethodData::Card")
    redirect = re.compile(r"RedirectForm::")
    token = re.compile(r"\bcavv\b|\beci\b|ds_transaction_id")
    out = {}
    for c in connector_set():
        src = _sources(c)
        if strong.search(src):
            out[c] = YES
        elif rej.search(src):
            out[c] = NO
        elif weak.search(src):
            out[c] = YES
        elif not card.search(src):
            out[c] = NO
        elif not redirect.search(src) and not token.search(src):
            out[c] = NO
        else:
            out[c] = UNKNOWN
    return out


def git_provenance(root: Path) -> dict:
    """Branch / commit / dirtiness of the UCS checkout these numbers came from."""
    import subprocess

    def git(*args):
        try:
            return subprocess.run(["git", "-C", str(root), *args],
                                  capture_output=True, text=True, timeout=10).stdout.strip()
        except Exception:
            return ""

    if not (root / ".git").exists():
        return {"path": str(root), "branch": "", "commit": "", "dirty": None}
    return {
        "path": str(root),
        "branch": git("rev-parse", "--abbrev-ref", "HEAD"),
        "commit": git("rev-parse", "--short", "HEAD"),
        "committedAt": git("log", "-1", "--format=%ad", "--date=short"),
        "dirty": bool(git("status", "--porcelain")),
    }
