#!/usr/bin/env python3
"""
Connector Documentation Generator

Generates connector documentation from field-probe JSON output.

Usage:
    python3 scripts/generators/docs/generate.py stripe adyen
    python3 scripts/generators/docs/generate.py --all
    python3 scripts/generators/docs/generate.py --list
    python3 scripts/generators/docs/generate.py --all-connectors-doc

How it works:
  1. Loads probe data from data/field_probe/{connector}.json
  2. All content is derived exclusively from probe data — no manual annotation files
  3. Outputs docs-generated/connectors/{name}.md

To add docs for a new connector:
  - Run field-probe to generate probe data: cd crates/internal/field-probe && cargo r
  - Run: python3 scripts/generators/docs/generate.py {name}
"""

import sys
import json
from pathlib import Path
from typing import Optional

import sys
sys.path.insert(0, str(Path(__file__).parent.parent))
from snippet_examples import generate as snippets
from snippet_examples.renderers import RENDERERS

# ─── Probe Data ───────────────────────────────────────────────────────────────

# Flows that have PM-specific probe results (vs flows that only have a 'default' key)
_PM_AWARE_FLOWS = frozenset(["authorize"])

# Global flow metadata loaded from probe.json (populated by load_probe_data)
_FLOW_METADATA: list[dict] = []

# Global message schemas from manifest (populated by load_probe_data)
_MESSAGE_SCHEMAS: dict = {}

# Global probe data indexed by connector name (populated by load_probe_data)
_PROBE_DATA: dict[str, dict] = {}


def get_flow_metadata() -> dict[str, dict]:
    """
    Get flow metadata as a dict keyed by flow_key.
    Returns {flow_key: {service_rpc, description, service_name, rpc_name}}
    """
    return {m["flow_key"]: m for m in _FLOW_METADATA}


def get_proto_flow_definitions() -> dict[str, tuple[str, str, str]]:
    """
    Get flow definitions in legacy format for compatibility.
    Returns {flow_key: (service_name, rpc_name, description)}
    """
    return {
        m["flow_key"]: (m["service_name"], m["rpc_name"], m["description"])
        for m in _FLOW_METADATA
    }


# Build reverse mapping from flow metadata (populated after load_probe_data)
def _build_flow_name_to_key_mapping() -> dict[str, str]:
    """
    Build mapping from RPC name to flow_key using loaded flow_metadata.
    E.g., "Authorize" -> "authorize", "Get" -> "get"
    """
    mapping = {}
    for m in _FLOW_METADATA:
        rpc_name = m.get("rpc_name", "")
        flow_key = m.get("flow_key", "")
        if rpc_name and flow_key:
            mapping[rpc_name] = flow_key
    return mapping


def get_flow_name_to_key() -> dict[str, str]:
    """Get the flow name to probe key mapping from loaded metadata."""
    if not hasattr(get_flow_name_to_key, '_cache'):
        get_flow_name_to_key._cache = _build_flow_name_to_key_mapping()
    return get_flow_name_to_key._cache

# Mapping from probe PM key to display name (order matters for table columns)
_PROBE_PM_DISPLAY: dict[str, str] = {
    "Card":           "Card",
    "GooglePay":      "Google Pay",
    "ApplePay":       "Apple Pay",
    "Sepa":           "SEPA",
    "Bacs":           "BACS",
    "Ach":            "ACH",
    "Becs":           "BECS",
    "Ideal":          "iDEAL",
    "PaypalRedirect": "PayPal",
    "Blik":           "BLIK",
    "Klarna":         "Klarna",
    "Afterpay":       "Afterpay",
    "UpiCollect":     "UPI",
    "Affirm":         "Affirm",
    "SamsungPay":     "Samsung Pay",
}


def load_probe_data(probe_path: Optional[Path]) -> dict[str, dict]:
    """
    Load probe JSON and index by connector name.

    Expects the split format: data/field_probe/ directory with manifest.json
    and per-connector {connector}.json files.

    Returns {connector_name: connector_data} dict.
    """
    global _FLOW_METADATA, _MESSAGE_SCHEMAS, _PROBE_DATA

    if probe_path is None:
        return {}

    probe_dir = probe_path if probe_path.is_dir() else probe_path
    manifest_path = probe_dir / "manifest.json"

    if not manifest_path.exists():
        print(f"Warning: manifest.json not found in {probe_dir}. Run field-probe first.", file=sys.stderr)
        return {}

    try:
        with open(manifest_path, encoding="utf-8") as f:
            manifest = json.load(f)
        _FLOW_METADATA = manifest.get("flow_metadata", [])
        _MESSAGE_SCHEMAS = manifest.get("message_schemas", {})
        connector_names = manifest.get("connectors", [])
        snippets.set_scenario_groups(manifest.get("scenario_groups", []))

        # Load proto type map for wrapper-type detection (SecretString, CardNumberType, etc.)
        proto_dir = probe_dir.parent.parent / "crates" / "types-traits" / "grpc-api-types" / "proto"
        if proto_dir.exists():
            snippets.load_proto_type_map(proto_dir)

        _PROBE_DATA = {}
        for conn_name in connector_names:
            conn_file = probe_dir / f"{conn_name}.json"
            if conn_file.exists():
                try:
                    with open(conn_file, encoding="utf-8") as f:
                        conn_data = json.load(f)
                    _PROBE_DATA[conn_name] = conn_data
                except Exception as exc:
                    print(f"Warning: failed to load {conn_file}: {exc}", file=sys.stderr)

        return _PROBE_DATA
    except Exception as exc:
        print(f"Warning: failed to load manifest: {exc}", file=sys.stderr)
        return {}


def _probe_pm_support(probe_connector: dict, flow_key: str) -> Optional[dict[str, bool]]:
    """
    Return {pm_key: supported} for a flow that has PM-specific probe results.
    Returns None for flows that only have a 'default' key (no PM breakdown).
    """
    if not flow_key:
        return None
    pms = probe_connector.get("flows", {}).get(flow_key, {})
    if not pms or set(pms.keys()) == {"default"}:
        return None
    return {pm: pms[pm]["status"] == "supported" for pm in _PROBE_PM_DISPLAY if pm in pms}


# Human-readable label per PM key used as the sample heading

# ─── Paths ────────────────────────────────────────────────────────────────────

REPO_ROOT       = Path(__file__).parent.parent.parent.parent
DOCS_DIR     = REPO_ROOT / "docs-generated/connectors"
EXAMPLES_DIR = REPO_ROOT / "examples"
PROTO_DIR    = REPO_ROOT / "crates/types-traits/grpc-api-types/proto"

# Category order for grouping flows in documentation
CATEGORY_ORDER = ["Payments", "Refunds", "Mandates", "Customers", "Disputes", "Authentication", "Session", "Other"]

# ─── Display Name ─────────────────────────────────────────────────────────────

_DISPLAY_NAMES = {
    "stripe": "Stripe",
    "adyen": "Adyen",
    "razorpay": "Razorpay",
    "razorpayv2": "Razorpay V2",
    "authorizedotnet": "Authorize.net",
    "braintree": "Braintree",
    "cybersource": "CyberSource",
    "checkout": "Checkout.com",
    "payu": "PayU",
    "novalnet": "Novalnet",
    "nexinets": "Nexinets",
    "noon": "Noon",
    "fiserv": "Fiserv",
    "elavon": "Elavon",
    "xendit": "Xendit",
    "mifinity": "MiFinity",
    "phonepe": "PhonePe",
    "cashfree": "Cashfree",
    "paytm": "Paytm",
    "cashtocode": "CashtoCode",
    "volt": "Volt",
    "dlocal": "dLocal",
    "helcim": "Helcim",
    "placetopay": "PlacetoPay",
    "rapyd": "Rapyd",
    "aci": "ACI",
    "trustpay": "TrustPay",
    "fiuu": "Fiuu",
    "calida": "Calida",
    "cryptopay": "CryptoPay",
}


def display_name(connector_name: str) -> str:
    return _DISPLAY_NAMES.get(connector_name, connector_name.replace("_", " ").title())


# ─── Markdown Generation ──────────────────────────────────────────────────────

def get_flows_from_probe(probe_connector: dict) -> list[str]:
    """Extract list of flow keys that have at least one supported entry in probe data."""
    result = []
    for flow_key, flow_data in probe_connector.get("flows", {}).items():
        if any(entry.get("status") == "supported" for entry in flow_data.values()):
            result.append(flow_key)
    return result


def get_flow_meta(flow_key: str) -> dict:
    """Get flow metadata by flow_key from loaded probe.json data."""
    flow_metadata = get_flow_metadata()
    return flow_metadata.get(flow_key, {})


# Bank PM keys that require a specific currency rather than the probe default (USD).
_BANK_PM_CURRENCY_OVERRIDES: dict[str, str] = {
    "Sepa": "EUR",
    "Bacs": "GBP",
    "Becs": "AUD",
    # Ach is USD — no override needed
}


def _get_flow_proto_requests(
    probe_connector: dict,
    scenario: "snippets.ScenarioSpec",
) -> dict[str, dict]:
    """
    Build flow_key → proto_request dict for the flows in a scenario.

    For authorize: uses the PM-specific entry keyed by scenario.pm_key.
    For all other flows: uses the "default" entry.
    Returns {} for any flow whose payload is missing or status != supported.

    Also applies bank-PM currency overrides so that SEPA uses EUR, BACS uses GBP, etc.
    """
    flows = probe_connector.get("flows", {})
    result: dict[str, dict] = {}
    for flow_key in scenario.flows:
        pm_key = scenario.pm_key if flow_key == "authorize" else "default"
        entry  = flows.get(flow_key, {}).get(pm_key or "default", {})
        if entry.get("status") == "supported":
            payload = dict(entry.get("proto_request") or {})
            # For checkout_bank, override the currency to the PM-appropriate one
            # (probe data defaults to USD which many bank PMs like SEPA don't support).
            if (
                scenario.key == "checkout_bank"
                and flow_key == "authorize"
                and scenario.pm_key in _BANK_PM_CURRENCY_OVERRIDES
                and "amount" in payload
                and isinstance(payload.get("amount"), dict)
            ):
                payload["amount"] = dict(payload["amount"])
                payload["amount"]["currency"] = _BANK_PM_CURRENCY_OVERRIDES[scenario.pm_key]
            result[flow_key] = payload
    return result


def _collect_flow_items(
    probe_connector: dict,
    exclude_keys: set,
) -> list[tuple[str, dict, str]]:
    """Return (flow_key, proto_req, pm_label) for every supported flow not in exclude_keys."""
    flows = probe_connector.get("flows", {})
    items: list[tuple[str, dict, str]] = []
    for flow_key, flow_data in flows.items():
        if flow_key in exclude_keys:
            continue
        if flow_key == "authorize":
            for pm in _AUTHORIZE_PM_PRIORITY:
                entry = flow_data.get(pm, {})
                if entry.get("status") == "supported" and entry.get("proto_request"):
                    items.append((flow_key, entry["proto_request"], pm))
                    break
        else:
            entry = flow_data.get("default", {})
            if entry.get("status") == "supported":
                items.append((flow_key, entry.get("proto_request") or {}, ""))
    return items


def _find_func_line(content: str, search: str) -> int:
    """Return the 1-based line number of the first line containing *search*, or 0 if not found."""
    for i, line in enumerate(content.splitlines(), 1):
        if search in line:
            return i
    return 0


# Maps language → (scenario_key → search string template) so we can locate
# each process_* function inside the generated consolidated file.
_SCENARIO_FUNC_SEARCH: dict[str, str] = {
    "python":     "async def process_{key}(",
    "javascript": "async function process{camel}(",
    "kotlin":     "fun process{camel}(",
    "rust":       "fn process_{key}(",
}

# Same for per-flow functions (used in the Flow Reference section).
_FLOW_FUNC_SEARCH: dict[str, str] = {
    "python":     "async def {key}(",
    "javascript": "async function {camel}(",
    "kotlin":     "fun {camel}(",
    "rust":       "fn {key}(",
}


def _scenario_search(sdk: str, scenario_key: str) -> str:
    """Return the function-name search string for a scenario in a given SDK."""
    tmpl = _SCENARIO_FUNC_SEARCH[sdk]
    camel = "".join(w.capitalize() for w in scenario_key.split("_"))
    return tmpl.format(key=scenario_key, camel=camel)


def _flow_search(sdk: str, flow_key: str) -> str:
    """Return the function-name search string for a flow in a given SDK."""
    tmpl = _FLOW_FUNC_SEARCH[sdk]
    camel = snippets._to_camel(flow_key)  # type: ignore[attr-defined]
    camel = camel[0].lower() + camel[1:]
    # JS reserved words are renamed with a "Payment" suffix in the generated file
    if sdk == "javascript" and flow_key in snippets.JS_RESERVED:  # type: ignore[attr-defined]
        camel = f"{flow_key}Payment"
    return tmpl.format(key=flow_key, camel=camel)


def generate_scenario_files(
    connector_name: str,
    probe_connector: dict,
    examples_dir: Path,
) -> tuple[list[Path], dict[str, dict[str, int]], dict[str, dict[str, int]]]:
    """
    Write one consolidated examples/{connector}/python/{connector}.py and
    examples/{connector}/javascript/{connector}.ts containing all scenarios
    plus individual flow functions.  Deletes stale per-scenario files.

    Returns (paths, scenario_lines, flow_lines) where:
      scenario_lines[scenario_key][sdk] = 1-based line of the process_* function
      flow_lines[flow_key][sdk]         = 1-based line of the flow function (py/js only)
    """
    flow_metadata = get_flow_metadata()
    scenarios     = snippets.detect_scenarios(probe_connector)

    # Pair each scenario with its payloads; skip scenarios with no data
    scenarios_with_payloads = [
        (s, fp)
        for s in scenarios
        for fp in [_get_flow_proto_requests(probe_connector, s)]
        if fp
    ]

    if not scenarios_with_payloads:
        return [], {}, {}

    # Collect ALL flows for standalone function generation (don't exclude scenario keys)
    # This ensures flows like "refund" that are both scenarios AND standalone flows
    # get their standalone functions generated in Python/JS
    flow_items    = _collect_flow_items(probe_connector, exclude_keys=set())
    written: list[Path] = []
    # scenario_lines[scenario_key][sdk] = 1-based line number of process_* function
    scenario_lines: dict[str, dict[str, int]] = {}
    # flow_lines[flow_key][sdk] = 1-based line number of flow function (py/js)
    flow_lines: dict[str, dict[str, int]] = {}

    # Use dynamic renderer dispatch instead of hardcoded functions
    for sdk, ext in [
        ("python",     "py"),
        ("javascript", "ts"),
    ]:
        out_dir  = examples_dir / connector_name / sdk
        out_dir.mkdir(parents=True, exist_ok=True)
        out_path = out_dir / f"{connector_name}.{ext}"
        renderer = RENDERERS[sdk]
        content  = renderer.render_consolidated(connector_name, scenarios_with_payloads, flow_metadata, _MESSAGE_SCHEMAS, flow_items)
        out_path.write_text(content, encoding="utf-8")
        written.append(out_path)

        # Record line numbers for each scenario function
        for scenario, _ in scenarios_with_payloads:
            lineno = _find_func_line(content, _scenario_search(sdk, scenario.key))
            if lineno:
                scenario_lines.setdefault(scenario.key, {})[sdk] = lineno

        # Record line numbers for each flow function (py/js only; kt/rs from generate_flow_files)
        for flow_key, _, _ in flow_items:
            lineno = _find_func_line(content, _flow_search(sdk, flow_key))
            if lineno:
                flow_lines.setdefault(flow_key, {})[sdk] = lineno

        # Remove stale per-scenario and per-flow files
        for scenario, _ in scenarios_with_payloads:
            stale = out_dir / f"{scenario.key}.{ext}"
            if stale.exists():
                stale.unlink()
        for flow_key, _, _ in flow_items:
            stale = out_dir / f"{flow_key}.{ext}"
            if stale.exists():
                stale.unlink()

    return written, scenario_lines, flow_lines


# PM priority for selecting the representative authorize payload
_AUTHORIZE_PM_PRIORITY = [
    "Card", "GooglePay", "ApplePay", "SamsungPay",
    "Sepa", "Ach", "Bacs", "Becs",
    "Ideal", "PaypalRedirect", "Blik", "Klarna", "Afterpay", "UpiCollect", "Affirm",
]


def generate_flow_files(
    connector_name: str,
    probe_connector: dict,
    examples_dir: Path,
) -> tuple[list[Path], dict[str, dict[str, int]], dict[str, dict[str, int]]]:
    """
    Write one consolidated examples/{connector}/kotlin/{connector}.kt and
    examples/{connector}/rust/{connector}.rs containing all scenario and flow functions.
    Deletes stale per-flow files for all languages.

    Returns (list_of_written_paths, flow_line_numbers, scenario_line_numbers_kt_rs) where:
      flow_line_numbers[flow_key][sdk]           = 1-based line of the flow function
      scenario_line_numbers_kt_rs[scenario_key][sdk] = 1-based line of the process_* function
    """
    flow_metadata = get_flow_metadata()
    # ALL flows must appear as standalone functions — including flows whose names match a
    # scenario key (e.g. "refund").
    flow_items = _collect_flow_items(probe_connector, exclude_keys=set())

    # Compute scenarios_with_payloads (same logic as generate_scenario_files)
    scenarios = snippets.detect_scenarios(probe_connector)
    scenarios_with_payloads = [
        (s, fp)
        for s in scenarios
        for fp in [_get_flow_proto_requests(probe_connector, s)]
        if fp
    ]

    if not flow_items and not scenarios_with_payloads:
        return [], {}, {}

    written: list[Path] = []
    # flow_line_numbers[flow_key][sdk] = 1-based line number
    flow_line_numbers: dict[str, dict[str, int]] = {}
    # scenario_line_numbers[scenario_key][sdk] = 1-based line number of process_* function
    scenario_line_numbers: dict[str, dict[str, int]] = {}

    # All flow keys (including those matching scenario names) — needed for cleanup
    all_flow_keys = set(probe_connector.get("flows", {}).keys())

    # Use dynamic renderer dispatch instead of hardcoded functions
    for sdk, ext in [
        ("kotlin", "kt"),
        ("rust",   "rs"),
    ]:
        out_dir  = examples_dir / connector_name / sdk
        out_dir.mkdir(parents=True, exist_ok=True)
        out_path = out_dir / f"{connector_name}.{ext}"
        renderer = RENDERERS[sdk]
        content  = renderer.render_consolidated(
            connector_name, scenarios_with_payloads, flow_metadata, _MESSAGE_SCHEMAS,
            flow_items=flow_items,
        )
        out_path.write_text(content, encoding="utf-8")
        written.append(out_path)

        # Record line numbers for each flow function
        for flow_key, _, _ in flow_items:
            lineno = _find_func_line(content, _flow_search(sdk, flow_key))
            if lineno:
                flow_line_numbers.setdefault(flow_key, {})[sdk] = lineno

        # Record line numbers for each scenario function
        for scenario, _ in scenarios_with_payloads:
            lineno = _find_func_line(content, _scenario_search(sdk, scenario.key))
            if lineno:
                scenario_line_numbers.setdefault(scenario.key, {})[sdk] = lineno

        # Delete ALL stale per-flow files (including scenario-named ones like refund.kt)
        for flow_key in all_flow_keys:
            stale = out_dir / f"{flow_key}.{ext}"
            if stale.exists():
                stale.unlink()

    return written, flow_line_numbers, scenario_line_numbers


def generate_llms_txt(probe_data: dict[str, dict], docs_dir: Path) -> None:
    """
    Write docs/llms.txt — a machine-readable navigation index for AI assistants.
    """
    lines: list[str] = [
        "# Connector Service — LLM Navigation Index",
        f"# Connectors: {len(probe_data)}",
        "#",
        "# This file helps AI coding assistants navigate connector-service documentation.",
        "# Each connector block lists: doc path, scenarios, supported payment methods,",
        "# supported flows, and paths to runnable Python/JavaScript examples.",
        "#",
        "# Usage: fetch this file first, then fetch the specific connector doc or example.",
        "",
        "overview:",
        f"  total_connectors: {len(probe_data)}",
        "  docs_root: docs-generated/connectors/",
        "  examples_root: examples/",
        "  all_connectors_matrix: docs-generated/all_connector.md",
        "",
        "integration_pattern:",
        "  1. Configure ConnectorConfig with connector name and credentials",
        "  2. Call flows in sequence per scenario (see Integration Scenarios in connector doc)",
        "  3. Branch on response.status: AUTHORIZED / PENDING / FAILED",
        "  4. PENDING means await webhook or poll Get before capturing",
        "  5. Pass connector_transaction_id from Authorize response to Capture/Refund",
        "",
        "---",
        "",
    ]

    for connector_name in sorted(probe_data.keys()):
        probe_connector = probe_data[connector_name]
        name            = display_name(connector_name)
        scenarios       = snippets.detect_scenarios(probe_connector)
        entry           = snippets.render_llms_txt_entry(
            connector_name, name, probe_connector, scenarios
        )
        lines.append(entry)

    out_path = docs_dir.parent / "llms.txt"
    out_path.write_text("\n".join(lines), encoding="utf-8")
    print(f"  llms.txt → {out_path.relative_to(REPO_ROOT)}")


def generate_connector_doc(
    connector_name: str,
    probe_data: Optional[dict] = None,
    scenario_line_numbers: Optional[dict[str, dict[str, int]]] = None,
    flow_line_numbers: Optional[dict[str, dict[str, int]]] = None,
) -> Optional[str]:
    """Generate complete markdown documentation for a connector.

    scenario_line_numbers: {scenario_key: {sdk: line_number}} — from generate_scenario_files.
    flow_line_numbers:      {flow_key:     {sdk: line_number}} — from generate_flow_files.
    """
    scenario_line_numbers = scenario_line_numbers or {}
    flow_line_numbers     = flow_line_numbers or {}
    probe_connector = (probe_data or {}).get(connector_name, {})
    
    # Get flows from probe data
    flows = get_flows_from_probe(probe_connector)
    if not flows:
        print(f"  No flows found for '{connector_name}' – skipping.", file=sys.stderr)
        return None

    name = display_name(connector_name)

    out: list[str] = []
    a = out.append  # shorthand

    # ── Front-matter comment ────────────────────────────────────────────────
    a(f"# {name}")
    a("")
    a("<!--")
    a("This file is auto-generated. Do not edit by hand.")
    a(f"Source: data/field_probe/{connector_name}.json")
    a(f"Regenerate: python3 scripts/generators/docs/generate.py {connector_name}")
    a("-->")
    a("")

    # ── SDK Configuration (once per connector) ──────────────────────────────
    for line in snippets.render_config_section(connector_name):
        a(line)

    # ── Integration Scenarios ────────────────────────────────────────────────
    scenarios     = snippets.detect_scenarios(probe_connector)
    flow_metadata = get_flow_metadata()
    if scenarios:
        a("## Integration Scenarios")
        a("")
        a(
            "Complete, runnable examples for common integration patterns. "
            "Each example shows the full flow with status handling. "
            "Copy-paste into your app and replace placeholder values."
        )
        a("")
        for scenario in scenarios:
            flow_payloads = _get_flow_proto_requests(probe_connector, scenario)
            for line in snippets.render_scenario_section(
                scenario, connector_name, flow_payloads,
                flow_metadata, _MESSAGE_SCHEMAS, {},
                line_numbers=scenario_line_numbers.get(scenario.key, {}),
            ):
                a(line)

    # ── API Reference ────────────────────────────────────────────────────────
    a("## API Reference")
    a("")
    a("| Flow (Service.RPC) | Category | gRPC Request Message |")
    a("|--------------------|----------|----------------------|")
    for f in flows:
        meta = get_flow_meta(f)
        cat = meta.get("category", "Other")
        req_msg = meta.get("grpc_request", "—")
        service = meta.get("service_name", "")
        rpc = meta.get("rpc_name", f)
        flow_display = f"{service}.{rpc}" if service else f
        # VS Code/GitHub auto-generate anchors: lowercase, remove dots/special chars
        anchor = flow_display.lower().replace(".", "").replace(" ", "-")
        a(f"| [{flow_display}](#{anchor}) | {cat} | `{req_msg}` |")
    a("")

    # ── Per-flow detail ──────────────────────────────────────────────────────
    # Group by category
    by_cat: dict[str, list[str]] = {}
    for f in flows:
        meta = get_flow_meta(f)
        cat = meta.get("category", "Other")
        by_cat.setdefault(cat, []).append(f)

    for cat in CATEGORY_ORDER:
        if cat not in by_cat:
            continue
        a(f"### {cat}")
        a("")

        for f in by_cat[cat]:
            meta = get_flow_meta(f)

            service = meta.get("service_name", "")
            rpc = meta.get("rpc_name", f)
            flow_heading = f"{service}.{rpc}" if service else f
            a(f"#### {flow_heading}")
            a("")

            if meta.get("description"):
                a(meta["description"])
                a("")

            # gRPC messages
            if meta.get("grpc_request"):
                a(f"| | Message |")
                a(f"|---|---------|")
                a(f"| **Request** | `{meta['grpc_request']}` |")
                a(f"| **Response** | `{meta.get('grpc_response', '—')}` |")
                a("")

            # Payment method type support (from field-probe)
            pm_support = _probe_pm_support(probe_connector, f)
            if pm_support:
                a("**Supported payment method types:**")
                a("")
                a("| Payment Method | Supported |")
                a("|----------------|:---------:|")
                for pm_key, pm_label in _PROBE_PM_DISPLAY.items():
                    if pm_key in pm_support:
                        pm_status = probe_connector.get("flows", {}).get("authorize", {}).get(pm_key, {}).get("status", "unknown")
                        mark = _status_to_mark(pm_status)
                        a(f"| {pm_label} | {mark} |")
                a("")

            # Inline PM reference right after Authorize (where it's most useful)
            if f == "authorize":
                for line in snippets.render_pm_reference_section(
                    probe_connector, flow_metadata, _MESSAGE_SCHEMAS
                ):
                    a(line)

            # Link to per-flow example files
            flow_data = probe_connector.get("flows", {}).get(f, {})
            has_payload = (
                flow_data.get("default", {}).get("status") == "supported"
                or any(
                    v.get("status") == "supported"
                    for k, v in flow_data.items()
                    if k != "default"
                )
            )
            if has_payload:
                base_py = f"../../examples/{connector_name}/python/{connector_name}.py"
                base_js = f"../../examples/{connector_name}/javascript/{connector_name}.ts"
                base_kt = f"../../examples/{connector_name}/kotlin/{connector_name}.kt"
                base_rs = f"../../examples/{connector_name}/rust/{connector_name}.rs"
                
                # Get line numbers from flow_line_numbers
                flow_lines = flow_line_numbers.get(f, {}) if flow_line_numbers else {}
                ln_py = flow_lines.get("python", 0)
                ln_js = flow_lines.get("javascript", 0)
                ln_kt = flow_lines.get("kotlin", 0)
                ln_rs = flow_lines.get("rust", 0)
                
                # Build links with line numbers when available
                py_link = f"{base_py}#L{ln_py}" if ln_py else base_py
                js_link = f"{base_js}#L{ln_js}" if ln_js else base_js
                kt_link = f"{base_kt}#L{ln_kt}" if ln_kt else base_kt
                rs_link = f"{base_rs}#L{ln_rs}" if ln_rs else base_rs
                
                a(f"**Examples:** [Python]({py_link}) · [JavaScript]({js_link}) · [Kotlin]({kt_link}) · [Rust]({rs_link})")
                a("")

    return "\n".join(out)


# ─── Connector Discovery ──────────────────────────────────────────────────────

def list_connectors() -> list[str]:
    """Return sorted list of all connector names from probe data."""
    return sorted(_PROBE_DATA.keys())


# ─── CLI ─────────────────────────────────────────────────────────────────────

def check_example_syntax(examples_dir: Path) -> None:
    """Run syntax checks on all generated example files."""
    import subprocess

    py_files = sorted(examples_dir.rglob("*.py"))
    ts_files = sorted(examples_dir.rglob("*.ts"))
    kt_files = sorted(examples_dir.rglob("*.kt"))
    rs_files = sorted(examples_dir.rglob("*.rs"))

    errors: list[str] = []

    # Python — full AST parse
    for f in py_files:
        result = subprocess.run(
            [sys.executable, "-m", "py_compile", str(f)],
            capture_output=True, text=True,
        )
        if result.returncode != 0:
            errors.append(f"Python: {f.relative_to(examples_dir.parent)}: {result.stderr.strip()}")

    # TypeScript — syntax check via tsc --noEmit (if available)
    tsc_ok = False
    try:
        subprocess.run(["tsc", "--version"], capture_output=True, check=True)
        tsc_ok = True
    except (subprocess.CalledProcessError, FileNotFoundError):
        pass
    if tsc_ok:
        for f in ts_files:
            result = subprocess.run(
                ["tsc", "--noEmit", "--strict", "--esModuleInterop", "--target", "ES2020",
                 "--module", "commonjs", "--moduleResolution", "node", str(f)],
                capture_output=True, text=True,
            )
            if result.returncode != 0:
                errors.append(f"TS: {f.relative_to(examples_dir.parent)}: {result.stderr.strip()}")

    # Kotlin — full compile via Gradle (preferred) or kotlinc syntax check (fallback).
    # Standalone kotlinc cannot resolve payments.* SDK imports, so only Gradle gives
    # accurate type-checking results.
    kt_ok = False
    sdk_java_dir = examples_dir.parent / "sdk" / "java"
    gradlew = sdk_java_dir / "gradlew"
    if gradlew.exists():
        kt_ok = True
        result = subprocess.run(
            [str(gradlew), ":smoke-test:compileKotlin", "--rerun-tasks", "-q"],
            capture_output=True, text=True,
            cwd=str(sdk_java_dir),
        )
        if result.returncode != 0:
            # Parse errors from Gradle output (lines starting with "e: file://...")
            for line in (result.stdout + result.stderr).splitlines():
                if line.startswith("e: file://"):
                    # Shorten the absolute path for readability
                    short = line.replace("e: file://" + str(examples_dir.parent) + "/", "")
                    errors.append(f"Kotlin: {short}")
    else:
        try:
            subprocess.run(["kotlinc", "-version"], capture_output=True, check=True)
            kt_ok = True
        except (subprocess.CalledProcessError, FileNotFoundError):
            pass
        if kt_ok:
            for f in kt_files:
                result = subprocess.run(
                    ["kotlinc", "-nowarn", str(f), "-d", "/dev/null"],
                    capture_output=True, text=True,
                )
                if result.returncode != 0:
                    errors.append(f"Kotlin: {f.relative_to(examples_dir.parent)}: {result.stderr.strip()}")

    # Rust — format check via rustfmt (syntax-level); full compile needs cargo check
    # Full compilation: cargo check -p hyperswitch-payments-client (from repo root)
    rustfmt_ok = False
    try:
        subprocess.run(["rustfmt", "--version"], capture_output=True, check=True)
        rustfmt_ok = True
    except (subprocess.CalledProcessError, FileNotFoundError):
        pass
    if rustfmt_ok:
        for f in rs_files:
            result = subprocess.run(
                ["rustfmt", "--check", "--edition", "2021", str(f)],
                capture_output=True, text=True,
            )
            # rustfmt --check exits 1 only on formatting diffs, not syntax errors.
            # Run rustfmt without --check to detect parse errors.
            result2 = subprocess.run(
                ["rustfmt", "--edition", "2021", "--check", str(f)],
                capture_output=True, text=True,
            )
            if "error" in result2.stderr.lower():
                errors.append(f"Rust: {f.relative_to(examples_dir.parent)}: {result2.stderr.strip()}")

    if errors:
        print(f"\n  Syntax errors in {len(errors)} example file(s):")
        for e in errors:
            print(f"    {e}")
    else:
        checks = f"{len(py_files)} Python, {len(ts_files)} TypeScript, {len(kt_files)} Kotlin, {len(rs_files)} Rust"
        ts_note = "" if tsc_ok else " (tsc unavailable — TypeScript skipped)"
        kt_note = "" if kt_ok else " (Gradle/kotlinc unavailable — Kotlin skipped)"
        rs_note = "" if rustfmt_ok else " (rustfmt unavailable — Rust skipped)"
        print(f"  ✓ Syntax check passed ({checks}){ts_note}{kt_note}{rs_note}")


def cmd_list():
    connectors = list_connectors()
    print(f"Available connectors ({len(connectors)}):\n")
    for name in connectors:
        print(f"  {name}")


def cmd_generate(connectors: list[str], output_dir: Path, probe_path: Optional[Path] = None):
    probe_data = load_probe_data(probe_path)
    if not probe_data:
        print("Error: No probe data available. Run field-probe first.", file=sys.stderr)
        sys.exit(1)
    
    print(f"Loaded probe data for {len(probe_data)} connectors from {probe_path}\n")

    output_dir.mkdir(parents=True, exist_ok=True)

    ok = 0
    skip = 0
    for name in connectors:
        print(f"  {name} ... ", end="", flush=True)
        probe_connector = probe_data.get(name, {})
        n_flows         = len(get_flows_from_probe(probe_connector))

        # Generate example files first so we can compute line numbers for doc links.
        scenario_files, scenario_lines, flow_lines_py_js     = generate_scenario_files(name, probe_connector, EXAMPLES_DIR)
        flow_files, flow_lines_kt_rs, scenario_lines_kt_rs   = generate_flow_files(name, probe_connector, EXAMPLES_DIR)

        # Merge py/js and kt/rs flow line numbers into one dict.
        merged_flow_lines: dict[str, dict[str, int]] = {}
        for flow_key, langs in flow_lines_py_js.items():
            merged_flow_lines.setdefault(flow_key, {}).update(langs)
        for flow_key, langs in flow_lines_kt_rs.items():
            merged_flow_lines.setdefault(flow_key, {}).update(langs)

        # Merge kt/rs scenario line numbers into the main scenario_lines dict.
        for scenario_key, langs in scenario_lines_kt_rs.items():
            scenario_lines.setdefault(scenario_key, {}).update(langs)

        doc = generate_connector_doc(
            name,
            probe_data=probe_data,
            scenario_line_numbers=scenario_lines,
            flow_line_numbers=merged_flow_lines,
        )
        if doc:
            out = output_dir / f"{name}.md"
            out.write_text(doc, encoding="utf-8")
            n_scenarios  = len(scenario_files) // 2  # python + js per scenario
            n_flow_files = len(flow_files)
            print(f"✓  ({n_flows} flows, {n_scenarios} scenarios, {n_flow_files} flow examples → {out.relative_to(REPO_ROOT)})")
            ok += 1
        else:
            print("skipped")
            skip += 1

    generate_llms_txt(probe_data, output_dir)
    print(f"\nDone: {ok} generated, {skip} skipped.")
    check_example_syntax(EXAMPLES_DIR)


# ─── All Connectors Coverage Document ─────────────────────────────────────────

def _status_to_mark(status: str) -> str:
    """Map a probe status string to a display icon."""
    if status == "supported":
        return "✓"
    elif status == "not_supported":
        return "x"
    elif status == "not_implemented":
        return "⚠"
    else:
        return "?"


def _get_flow_status(flows: dict, flow_key: str) -> tuple[str, str]:
    """
    Get the status of a flow from probe data.
    Returns (status_mark, notes) tuple.
    """
    flow_data = flows.get(flow_key, {})

    # For PM-aware flows, check if there's any supported PM
    if flow_key in _PM_AWARE_FLOWS:
        supported_pms = [
            pm for pm, data in flow_data.items()
            if pm != "default" and data.get("status") == "supported"
        ]
        if supported_pms:
            return ("✓", f"{len(supported_pms)} PMs")
        # Check if all PM entries are not_implemented → ⚠
        pm_entries = [pm for pm in flow_data.keys() if pm != "default"]
        if pm_entries:
            statuses = {flow_data[pm].get("status") for pm in pm_entries}
            if statuses == {"not_implemented"}:
                return ("⚠", "")
            return ("x", "")

    # For flows with only 'default' entry
    default_entry = flow_data.get("default", {})
    status = default_entry.get("status", "unknown")

    if status == "supported":
        return ("✓", "")
    elif status == "error":
        error_msg = default_entry.get("error", "")
        if len(error_msg) > 60:
            error_msg = error_msg[:57] + "..."
        return ("?", error_msg if error_msg else "Error")
    elif status == "not_supported":
        return ("x", "")
    elif status == "not_implemented":
        return ("⚠", "")
    else:
        return ("?", "")


def generate_all_connector_doc(probe_data: dict[str, dict], output_dir: Path) -> None:
    """
    Generate all_connector.md - a comprehensive connector-wise flow coverage document.
    
    This creates a unified view showing:
    - For each flow, which connectors support which payment methods
    - Summary statistics for each connector and flow
    - Flow names follow proto service definitions from services.proto
    """
    out: list[str] = []
    a = out.append
    
    # ── Header ────────────────────────────────────────────────────────────────
    a("# Connector Flow Coverage")
    a("")
    a("<!--")
    a("This file is auto-generated. Do not edit by hand.")
    a("Source: data/field_probe/")
    a("Regenerate: python3 scripts/generators/docs/generate.py --all-connectors-doc")
    a("-->")
    a("")
    a("This document provides a comprehensive overview of payment method support")
    a("across all connectors for each payment flow. Flow names follow the gRPC")
    a("service definitions from `crates/types-traits/grpc-api-types/proto/services.proto`.")
    a("")
    
    # Get all connectors that have probe data
    connectors_with_probe = sorted(probe_data.keys())
    
    if not connectors_with_probe:
        a("No probe data available.")
        output_dir.mkdir(parents=True, exist_ok=True)
        out_path = output_dir.parent / "all_connector.md"
        out_path.write_text("\n".join(out), encoding="utf-8")
        return
    
    # ── Summary Table ──────────────────────────────────────────────────────────
    # Use proto-based flow names for summary
    summary_flows = [
        ("authorize", "PaymentService.Authorize"),
        ("capture", "PaymentService.Capture"),
        ("get", "PaymentService.Get"),
        ("refund", "PaymentService.Refund"),
        ("void", "PaymentService.Void"),
    ]
    
    a("## Summary")
    a("")
    # Header with service-prefixed flow names
    header_parts = ["Connector"]
    for _, flow_display in summary_flows:
        # Extract just the RPC name for brevity in header
        rpc_name = flow_display.split(".")[-1]
        header_parts.append(rpc_name)
    a("| " + " | ".join(header_parts) + " |")
    a("|" + "|".join(["-----------"] + [":---:" for _ in summary_flows]) + "|")
    
    for conn_name in connectors_with_probe:
        conn_data = probe_data[conn_name]
        flows = conn_data.get("flows", {})
        
        display = _DISPLAY_NAMES.get(conn_name, conn_name.replace("_", " ").title())
        row = [f"[{display}](connectors/{conn_name}.md)"]
        
        for flow_key, _ in summary_flows:
            status_mark, _ = _get_flow_status(flows, flow_key)
            row.append(status_mark)
        
        a("| " + " | ".join(row) + " |")
    a("")
    
    # ── Per-Service Flow Coverage Tables ───────────────────────────────────────
    a("## Flow Details")
    a("")
    a("Flow names follow the gRPC service definitions. Each flow is prefixed with")
    a("its service name (e.g., `PaymentService.Authorize`, `RefundService.Get`).")
    a("")
    
    # Group flows by service
    services_order = [
        "PaymentService",
        "RecurringPaymentService", 
        "RefundService",
        "CustomerService",
        "PaymentMethodService",
        "MerchantAuthenticationService",
        "PaymentMethodAuthenticationService",
        "DisputeService",
    ]
    
    # Build service -> flows mapping from flow_metadata loaded from probe.json
    proto_flow_defs = get_proto_flow_definitions()
    if not proto_flow_defs:
        print("Warning: No flow metadata loaded from probe.json", file=sys.stderr)
    service_flows: dict[str, list[tuple[str, str, str]]] = {}
    for flow_key, (service_name, rpc_name, description) in proto_flow_defs.items():
        service_flows.setdefault(service_name, []).append((flow_key, rpc_name, description))
    
    for service_name in services_order:
        if service_name not in service_flows:
            continue
        
        flows_in_service = service_flows[service_name]
        
        # Check if any flow in this service has probe data
        service_has_data = any(
            any(
                probe_data[c].get("flows", {}).get(flow_key)
                for c in connectors_with_probe
            )
            for flow_key, _, _ in flows_in_service
        )
        if not service_has_data:
            continue
        
        a(f"### {service_name}")
        a("")
        
        for flow_key, rpc_name, description in flows_in_service:
            # Check if any connector has data for this flow
            has_data = any(
                probe_data[c].get("flows", {}).get(flow_key)
                for c in connectors_with_probe
            )
            if not has_data:
                continue
            
            # Flow heading with full service.rpc name
            a(f"#### {service_name}.{rpc_name}")
            a("")
            a(description)
            a("")
            
            if flow_key in _PM_AWARE_FLOWS:
                # For PM-aware flows, show full PM breakdown
                a("| Connector | " + " | ".join(_PROBE_PM_DISPLAY.values()) + " |")
                a("|-----------|" + "|".join([":---:" for _ in _PROBE_PM_DISPLAY]) + "|")
                
                for conn_name in connectors_with_probe:
                    conn_data = probe_data[conn_name]
                    flow_data = conn_data.get("flows", {}).get(flow_key, {})
                    
                    display = _DISPLAY_NAMES.get(conn_name, conn_name.replace("_", " ").title())
                    row = [f"[{display}](connectors/{conn_name}.md)"]
                    
                    for pm_key in _PROBE_PM_DISPLAY:
                        pm_data = flow_data.get(pm_key, {})
                        status = pm_data.get("status", "unknown")
                        row.append(_status_to_mark(status))
                    
                    a("| " + " | ".join(row) + " |")
                a("")
                
                # Legend
                a("**Legend:** ✓ Supported | x Not Supported | ⚠ Not Implemented | ? Error / Missing required fields")
                a("")
            else:
                # For other flows, show simple supported/not supported
                a("| Connector | Supported | Notes |")
                a("|-----------|:---------:|-------|")
                
                for conn_name in connectors_with_probe:
                    conn_data = probe_data[conn_name]
                    flows = conn_data.get("flows", {})
                    status_mark, notes = _get_flow_status(flows, flow_key)
                    
                    display = _DISPLAY_NAMES.get(conn_name, conn_name.replace("_", " ").title())
                    a(f"| [{display}](connectors/{conn_name}.md) | {status_mark} | {notes} |")
                a("")
    
    # ── Payment Method Legend ─────────────────────────────────────────────────
    a("## Payment Methods")
    a("")
    a("Payment methods probed for authorize flow (configured in `crates/internal/field-probe/probe-config.toml`):")
    a("")
    a("| Key | Display Name | Description |")
    a("|-----|--------------|-------------|")
    a("| Card | Card | Credit/Debit card payments |")
    a("| GooglePay | Google Pay | Google Pay digital wallet |")
    a("| ApplePay | Apple Pay | Apple Pay digital wallet |")
    a("| Sepa | SEPA | SEPA Direct Debit (EU bank transfers) |")
    a("| Bacs | BACS | BACS Direct Debit (UK bank transfers) |")
    a("| Ach | ACH | ACH Direct Debit (US bank transfers) |")
    a("| Becs | BECS | BECS Direct Debit (AU bank transfers) |")
    a("| Ideal | iDEAL | iDEAL (Netherlands bank redirect) |")
    a("| PaypalRedirect | PayPal | PayPal redirect payments |")
    a("| Blik | BLIK | BLIK (Polish mobile payment) |")
    a("| Klarna | Klarna | Klarna Buy Now Pay Later |")
    a("| Afterpay | Afterpay | Afterpay/Clearpay BNPL |")
    a("| UpiCollect | UPI | UPI Collect (India) |")
    a("| Affirm | Affirm | Affirm BNPL |")
    a("| SamsungPay | Samsung Pay | Samsung Pay digital wallet |")
    a("")
    
    # ── Services Reference ─────────────────────────────────────────────────────
    a("## Services Reference")
    a("")
    a("Flow definitions are derived from `crates/types-traits/grpc-api-types/proto/services.proto`:")
    a("")
    a("| Service | Description |")
    a("|---------|-------------|")
    a("| PaymentService | Process payments from authorization to settlement |")
    a("| RecurringPaymentService | Charge and revoke recurring payments |")
    a("| RefundService | Retrieve and synchronize refund statuses |")
    a("| CustomerService | Create and manage customer profiles |")
    a("| PaymentMethodService | Tokenize and retrieve payment methods |")
    a("| MerchantAuthenticationService | Generate access tokens and session credentials |")
    a("| PaymentMethodAuthenticationService | Execute 3D Secure authentication flows |")
    a("| DisputeService | Manage chargeback disputes |")
    a("")
    
    # Write output
    output_dir.mkdir(parents=True, exist_ok=True)
    out_path = output_dir.parent / "all_connector.md"
    out_path.write_text("\n".join(out), encoding="utf-8")
    print(f"  ✓ Generated {out_path.relative_to(REPO_ROOT)}")


def cmd_all_connectors_doc(output_dir: Path, probe_path: Optional[Path] = None):
    """Generate the all_connector.md coverage document."""
    probe_data = load_probe_data(probe_path)
    if not probe_data:
        print("Error: No probe data available. Run field-probe first.", file=sys.stderr)
        sys.exit(1)
    
    print(f"Generating all_connector.md from {len(probe_data)} connectors\n")
    generate_all_connector_doc(probe_data, output_dir)
    print("\nDone.")


def generate_rust_build_auth(proto_dir: Path, out_file: Path) -> None:
    """Generate sdk/rust/smoke-test/src/build_auth.rs from payment.proto."""
    import re as _re

    proto_text = (proto_dir / "payment.proto").read_text(encoding="utf-8")
    # Strip comments
    proto_text = _re.sub(r"//[^\n]*", "", proto_text)
    proto_text = _re.sub(r"/\*.*?\*/", "", proto_text, flags=_re.DOTALL)

    # Find ConnectorSpecificConfig oneof body
    csc_m = _re.search(
        r"message\s+ConnectorSpecificConfig\s*\{.*?oneof\s+config\s*\{(.*?)\}\s*\}",
        proto_text, _re.DOTALL,
    )
    if not csc_m:
        print("  WARNING: ConnectorSpecificConfig oneof not found, skipping build_auth.rs")
        return

    oneof_body = csc_m.group(1)
    # Extract: TypeName field_name = num;  (e.g. AdyenConfig adyen = 1;)
    config_variants = _re.findall(r"(\w+Config)\s+(\w+)\s*=\s*\d+\s*;", oneof_body)

    # Parse each *Config message for its fields
    config_fields: dict[str, list[tuple[str, str]]] = {}
    for type_name, field_name in config_variants:
        msg_m = _re.search(
            rf"message\s+{type_name}\s*\{{(.*?)\}}",
            proto_text, _re.DOTALL
        )
        if msg_m:
            body = msg_m.group(1)
            # field lines: string api_key = 1;
            fields = _re.findall(r"(\w+)\s+(\w+)\s*=\s*\d+", body)
            config_fields[field_name] = fields
        else:
            config_fields[field_name] = []

    # Build auth.rs content
    lines: list[str] = [
        "// Auto-generated from payment.proto — do not edit manually",
        "// Regenerate: python3 scripts/generators/docs/generate.py",
        "",
        "use payments::generated::payment_pb2;",
        "use std::env;",
        "",
        "/// Build ConnectorAuthType from environment variables",
        "/// Environment variable format: {CONNECTOR_NAME}_API_KEY, etc.",
        "pub fn build_auth(connector_name: &str) -> payment_pb2::ConnectorAuthType {",
        "    match connector_name {",
    ]
    for field_name in sorted(config_fields.keys()):
        conn_var = field_name.lower()
        env_prefix = conn_var.upper()
        lines.append(f'        "{conn_var}" => {{')
        lines.append(f'            let mut auth = payment_pb2::ConnectorAuthType::new();')
        lines.append(f'            let mut specific = payment_pb2::ConnectorSpecificConfig::new();')
        lines.append(f'            let mut config = payment_pb2::{conn_var.title()}Config::new();')
        for _, fname in config_fields[field_name]:
            env_var = f"{env_prefix}_{fname.upper()}"
            lines.append(f'            config.{fname} = env::var("{env_var}").unwrap_or_default();')
        lines.append(f'            specific.set_{conn_var}(config);')
        lines.append(f'            auth.set_connector_specific_config(specific);')
        lines.append(f'            auth')
        lines.append(f'        }}')
    lines.append('        _ => payment_pb2::ConnectorAuthType::new(),')
    lines.append('    }')
    lines.append('}')
    lines.append('')

    out_file.parent.mkdir(parents=True, exist_ok=True)
    out_file.write_text("\n".join(lines), encoding="utf-8")
    print(f"  {out_file.relative_to(REPO_ROOT)}")


def main():
    import argparse
    parser = argparse.ArgumentParser(
        description="Generate connector documentation from field-probe data"
    )
    
    parser.add_argument(
        "connectors",
        nargs="*",
        help="Connector names to generate docs for"
    )
    parser.add_argument(
        "--all",
        action="store_true",
        help="Generate docs for all connectors"
    )
    parser.add_argument(
        "--list",
        action="store_true",
        help="List all available connectors"
    )
    parser.add_argument(
        "--all-connectors-doc",
        action="store_true",
        help="Generate the all_connector.md coverage document"
    )
    parser.add_argument(
        "--probe-path",
        type=Path,
        default=REPO_ROOT / "data" / "field_probe",
        help="Path to field-probe output directory (default: data/field_probe)"
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=DOCS_DIR,
        help="Output directory for generated docs (default: docs-generated/connectors)"
    )
    
    args = parser.parse_args()
    
    load_probe_data(args.probe_path)
    
    if args.list:
        cmd_list()
        return
    
    if args.all_connectors_doc:
        cmd_all_connectors_doc(args.output_dir, args.probe_path)
        return
    
    if args.all:
        connectors = list_connectors()
        if not connectors:
            print("Error: No connectors found. Run field-probe first.", file=sys.stderr)
            sys.exit(1)
        cmd_generate(connectors, args.output_dir, args.probe_path)
    elif args.connectors:
        cmd_generate(args.connectors, args.output_dir, args.probe_path)
    else:
        parser.print_help()
        sys.exit(1)


if __name__ == "__main__":
    main()
