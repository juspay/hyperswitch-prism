"""
Markdown documentation generators.

Generates:
- Connector documentation (per-connector markdown files)
- All-connectors coverage matrix
- LLMs.txt navigation index
"""

import sys
from pathlib import Path
from typing import Optional, List


def generate_connector_doc(
    connector_name: str,
    probe_data: Optional[dict] = None,
    scenario_line_numbers: Optional[dict[str, dict[str, int]]] = None,
    flow_line_numbers: Optional[dict[str, dict[str, int]]] = None,
    display_name_fn=None,
    get_flows_fn=None,
    get_flow_meta_fn=None,
    get_flow_metadata_fn=None,
    probe_pm_display=None,
    probe_pm_by_category=None,
    message_schemas=None,
    pm_aware_flows=None,
    render_config_section_fn=None,
    detect_scenarios_fn=None,
    render_llms_txt_entry_fn=None,
) -> Optional[str]:
    """Generate complete markdown documentation for a connector.

    Args:
        connector_name: Name of the connector
        probe_data: Probe data dict (optional, uses global if not provided)
        scenario_line_numbers: Line numbers for scenario examples
        flow_line_numbers: Line numbers for flow examples
        display_name_fn: Function to get display name
        get_flows_fn: Function to get flows from probe
        get_flow_meta_fn: Function to get flow metadata
        get_flow_metadata_fn: Function to get all flow metadata
        probe_pm_display: PM display mapping
        probe_pm_by_category: PM categories
        message_schemas: Message schemas
        pm_aware_flows: Set of PM-aware flow keys
        
    Returns:
        Markdown string or None if connector has no flows
    """
    scenario_line_numbers = scenario_line_numbers or {}
    flow_line_numbers     = flow_line_numbers or {}
    probe_connector = (probe_data or {}).get(connector_name, {})
    
    # Get flows from probe data
    flows = get_flows_fn(probe_connector) if get_flows_fn else []
    if not flows:
        print(f"  No flows found for '{connector_name}' – skipping.", file=sys.stderr)
        return None

    name = display_name_fn(connector_name) if display_name_fn else connector_name

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
    config_fn = render_config_section_fn if render_config_section_fn else lambda x: [f"## SDK Configuration", "", f"Connector: {x}"]
    for line in config_fn(connector_name):
        a(line)

    # ── Integration Scenarios ────────────────────────────────────────────────
    detect_fn = detect_scenarios_fn if detect_scenarios_fn else lambda x: []
    scenarios = detect_fn(probe_connector)
    flow_metadata = get_flow_metadata_fn() if get_flow_metadata_fn else {}
    if scenarios:
        a("## Integration Scenarios")
        a("")
        a(
            "Complete, runnable examples for common integration patterns. "
            "Each example shows the full flow with status handling. "
            "Copy-paste into your app and replace placeholder values."
        )
        a("")

    # ── API Reference ────────────────────────────────────────────────────────
    CATEGORY_ORDER = ["Payment", "Recurring Payment", "Refund", "Customer", "Payment Method", "Authentication", "Other"]
    
    a("## API Reference")
    a("")
    a("| Flow (Service.RPC) | Category | gRPC Request Message |")
    a("|--------------------|----------|----------------------|")
    for f in flows:
        meta = get_flow_meta_fn(f) if get_flow_meta_fn else {}
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
        meta = get_flow_meta_fn(f) if get_flow_meta_fn else {}
        cat = meta.get("category", "Other")
        by_cat.setdefault(cat, []).append(f)

    for cat in CATEGORY_ORDER:
        if cat not in by_cat:
            continue
        a(f"### {cat}")
        a("")

        for f in by_cat[cat]:
            meta = get_flow_meta_fn(f) if get_flow_meta_fn else {}

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
            pm_support = _probe_pm_support(probe_connector, f, probe_pm_display or {})
            if pm_support:
                a("**Supported payment method types:**")
                a("")
                a("| Payment Method | Supported |")
                a("|----------------|:---------:|")
                for pm_key, pm_label in (probe_pm_display or {}).items():
                    if pm_key in pm_support:
                        pm_status = probe_connector.get("flows", {}).get("authorize", {}).get(pm_key, {}).get("status", "unknown")
                        mark = _status_to_mark(pm_status)
                        a(f"| {pm_label} | {mark} |")
                a("")

            # Inline PM reference right after Authorize (where it's most useful)
            if f == "authorize":
                # TODO: Implement PM reference with new architecture
                pass

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
                base_js = f"../../examples/{connector_name}/javascript/{connector_name}.js"
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


def generate_all_connector_doc(
    probe_data: dict[str, dict],
    output_dir: Path,
    display_names: dict[str, str],
    get_proto_flow_defs_fn,
    probe_pm_by_category: list,
    repo_root: Path,
) -> None:
    """
    Generate all_connector.md - a comprehensive connector-wise flow coverage document.
    """
    _PM_AWARE_FLOWS = frozenset(["authorize"])
    
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
    
    # ── Per-Service Flow Coverage Tables ───────────────────────────────────────
    a("## Flow Coverage")
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
    proto_flow_defs = get_proto_flow_defs_fn() if get_proto_flow_defs_fn else {}
    service_flows: dict[str, list[tuple[str, str, str]]] = {}
    for flow_key, (service_name, rpc_name, description) in proto_flow_defs.items():
        service_flows.setdefault(service_name, []).append((flow_key, rpc_name, description))
    
    # Separate PM-aware flows from simple status flows
    pm_aware_flows_data = []
    simple_flows_data = []
    
    for service_name in services_order:
        if service_name not in service_flows:
            continue
        
        flows_in_service = service_flows[service_name]
        
        for flow_key, rpc_name, description in flows_in_service:
            # Check if any connector has data for this flow
            has_data = any(
                probe_data[c].get("flows", {}).get(flow_key)
                for c in connectors_with_probe
            )
            if not has_data:
                continue
            
            if flow_key in _PM_AWARE_FLOWS:
                pm_aware_flows_data.append((service_name, flow_key, rpc_name, description))
            else:
                simple_flows_data.append((service_name, flow_key, rpc_name, description))
    
    # Render PM-aware flows (like Authorize) with full payment method breakdown
    for service_name, flow_key, rpc_name, description in pm_aware_flows_data:
        a(f"### {service_name}.{rpc_name}")
        a("")
        a(description)
        a("")
        
        # Build display names with category prefix for clarity
        pm_display_with_category = []
        pm_keys_ordered = []
        for category, pm_list in probe_pm_by_category:
            for pm_key, pm_name in pm_list:
                pm_keys_ordered.append(pm_key)
                # Shorten category names for compact display
                short_cat = {
                    "Card": "CARD",
                    "Wallet": "WALLET", 
                    "BNPL": "BNPL",
                    "UPI": "UPI",
                    "Online Banking": "Online Banking",
                    "Open Banking": "Open Banking",
                    "Bank Redirect": "Bank Redirect",
                    "Bank Transfer": "Bank Transfer",
                    "Bank Debit": "Bank Debit",
                    "Alternative": "Alternate PMs "
                }.get(category, category[:4].upper())
                pm_display_with_category.append(f"{short_cat} / {pm_name}")
        
        # Legend at top for clarity
        a("**Legend:** ✓ Supported | x Not Supported | ⚠ Not Implemented | ? Error / Missing required fields")
        a("")
        
        a("| Connector | " + " | ".join(pm_display_with_category) + " |")
        a("|-----------|" + "|".join([":---:" for _ in pm_display_with_category]) + "|")
        
        for conn_name in connectors_with_probe:
            conn_data = probe_data[conn_name]
            flow_data = conn_data.get("flows", {}).get(flow_key, {})
            
            display = display_names.get(conn_name, conn_name.replace("_", " ").title())
            row = [f"[{display}](connectors/{conn_name}.md)"]
            
            for pm_key in pm_keys_ordered:
                pm_data = flow_data.get(pm_key, {})
                status = pm_data.get("status", "unknown")
                row.append(_status_to_mark(status))
            
            a("| " + " | ".join(row) + " |")
        a("")
    
    # Render consolidated table for all simple flows (Get, Void, Refund, etc.)
    if simple_flows_data:
        a("### Other Flows")
        a("")
        a("Consolidated view of Get, Void, Refund, Capture, Reverse, CreateOrder, and other non-payment flows.")
        a("")
        
        # Build header with flow names
        flow_headers = []
        for service_name, flow_key, rpc_name, description in simple_flows_data:
            # Shorten service name for compact display
            short_service = service_name.replace("Service", "").replace("Payment", "Pay").replace("Recurring", "Rec")
            flow_headers.append(f"{short_service}.{rpc_name}")
        
        # Legend at top for clarity
        a("**Legend:** ✓ Supported | x Not Supported | ⚠ Not Implemented | ? Error / Missing required fields")
        a("")
        
        a("| Connector | " + " | ".join(flow_headers) + " |")
        a("|-----------|" + "|".join([":---:" for _ in simple_flows_data]) + "|")
        
        for conn_name in connectors_with_probe:
            conn_data = probe_data[conn_name]
            flows = conn_data.get("flows", {})
            
            display = display_names.get(conn_name, conn_name.replace("_", " ").title())
            row = [f"[{display}](connectors/{conn_name}.md)"]
            
            for service_name, flow_key, rpc_name, description in simple_flows_data:
                status_mark = _get_flow_status_simple(flows, flow_key)
                row.append(status_mark)
            
            a("| " + " | ".join(row) + " |")
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
    print(f"  ✓ Generated {out_path.relative_to(repo_root)}")


def generate_llms_txt(
    probe_data: dict[str, dict],
    docs_dir: Path,
    display_name_fn,
    repo_root: Path,
) -> None:
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
        name            = display_name_fn(connector_name)
        scenarios       = detect_scenarios(probe_connector)
        entry           = render_llms_txt_entry(
            connector_name, name, probe_connector, scenarios
        )
        lines.append(entry)

    out_path = docs_dir.parent / "llms.txt"
    out_path.write_text("\n".join(lines), encoding="utf-8")
    print(f"  llms.txt → {out_path.relative_to(repo_root)}")


# ─── Helper Functions ─────────────────────────────────────────────────────────

def _status_to_mark(status: str) -> str:
    """Convert status string to markdown table mark."""
    return {
        "supported": "✓",
        "not_supported": "x", 
        "not_implemented": "⚠",
        "error": "?",
    }.get(status, "?")


def _get_flow_status(flows: dict, flow_key: str) -> tuple[str, str]:
    """Get status mark and description for a flow."""
    flow_data = flows.get(flow_key, {})
    
    # Check default entry
    if "default" in flow_data:
        status = flow_data["default"].get("status", "unknown")
        return _status_to_mark(status), status
    
    # Check for any supported PM variant
    for key, value in flow_data.items():
        if key == "default":
            continue
        if isinstance(value, dict) and value.get("status") == "supported":
            return "✓", "supported"
    
    return "?", "unknown"


def _get_flow_status_simple(flows: dict, flow_key: str) -> str:
    """Get status mark for a flow (simplified version returning just the mark)."""
    mark, _ = _get_flow_status(flows, flow_key)
    return mark


def _probe_pm_support(probe_connector: dict, flow_key: str, probe_pm_display: dict) -> Optional[dict[str, bool]]:
    """Check which payment methods are supported for a flow (from probe data)."""
    flows = probe_connector.get("flows", {})
    flow_data = flows.get(flow_key, {})
    
    if not flow_data:
        return None
    
    # Collect all PM keys that have data
    support = {}
    for pm_key in probe_pm_display.keys():
        pm_data = flow_data.get(pm_key, {})
        if pm_data and pm_data.get("status") == "supported":
            support[pm_key] = True
    
    return support if support else None


def _get_flow_proto_requests(probe_connector: dict, scenario, probe_pm_display: dict, pm_aware_flows: set) -> dict:
    """Get proto request payloads for each flow in a scenario."""
    flows = probe_connector.get("flows", {})
    result = {}
    
    # scenario.flows can be a list of strings or objects with .name attribute
    for flow in scenario.flows:
        # Handle both string flow names and objects with .name attribute
        flow_name = flow.name if hasattr(flow, 'name') else flow
        flow_entry = flows.get(flow_name, {})
        
        if not flow_entry:
            continue
        
        # For PM-aware flows, try to find a supported PM
        if flow_name in pm_aware_flows:
            # Try to find a supported PM variant
            for pm_key in probe_pm_display.keys():
                pm_data = flow_entry.get(pm_key, {})
                if pm_data and pm_data.get("status") == "supported":
                    result[flow_name] = pm_data.get("proto_request", {})
                    break
        else:
            # Use default entry
            default_data = flow_entry.get("default", {})
            if default_data and default_data.get("status") == "supported":
                result[flow_name] = default_data.get("proto_request", {})
    
    return result
