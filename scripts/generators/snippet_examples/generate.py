"""
Snippet Examples Generator — SDK integration example generator for connector docs.

All functions are pure (no I/O, no global state). Called by docs/generate.py.

Proto field comments come from manifest["message_schemas"] (populated by field-probe).

Public API
----------
  detect_scenarios(probe_connector) -> list[ScenarioSpec]
    Infer applicable integration scenarios from probe data.

  render_config_section(connector_name) -> list[str]
    4-tab SDK config table — emitted once per connector doc.

  render_scenario_section(scenario, connector_name, flow_payloads,
                           flow_metadata, message_schemas, ann_scenario) -> list[str]
    Full 4-tab runnable scenario example + status-handling table.

  render_pm_reference_section(probe_connector, flow_metadata,
                               message_schemas) -> list[str]
    Per-PM payment_method payload reference block.

  render_payload_block(flow_key, service_name, grpc_request,
                       proto_request, message_schemas) -> list[str]
    Single annotated payload block (used in Flow Reference section).

  render_llms_txt_entry(connector_name, display_name, probe_connector,
                         scenarios) -> str
    One connector block for docs/llms.txt.

Backward Compatibility
----------------------
  This module now uses the renderers package internally. The public API remains
  unchanged. Functions like render_consolidated_python(), render_flow_python(),
  etc. are now shims that delegate to the appropriate renderer.
"""

from __future__ import annotations

import sys
from pathlib import Path

# Handle imports for both package use and direct execution
if __package__:
    from .renderers import (
        RENDERERS,
        ScenarioSpec,
        _SchemaDB,
        load_proto_type_map as _load_proto_type_map,
        set_scenario_groups as _set_scenario_groups,
        _build_annotated,
        _conn_enum,
        _conn_display,
        _client_class,
        _td,
        _STEP_DESCRIPTIONS,
        _AUTHORIZE_STATUS_HANDLING,
        _AUTOCAPTURE_STATUS_HANDLING,
        _SETUP_RECURRING_STATUS_HANDLING,
        _PROBE_PM_LABELS,
        _SCENARIO_GROUPS,
        _CARD_AUTHORIZE_SCENARIOS,
    )
    from .renderers._shared import JS_RESERVED
else:
    from renderers import (
        RENDERERS,
        ScenarioSpec,
        _SchemaDB,
        load_proto_type_map as _load_proto_type_map,
        set_scenario_groups as _set_scenario_groups,
        _build_annotated,
        _conn_enum,
        _conn_display,
        _client_class,
        _td,
        _STEP_DESCRIPTIONS,
        _AUTHORIZE_STATUS_HANDLING,
        _AUTOCAPTURE_STATUS_HANDLING,
        _SETUP_RECURRING_STATUS_HANDLING,
        _PROBE_PM_LABELS,
        _SCENARIO_GROUPS,
        _CARD_AUTHORIZE_SCENARIOS,
    )
    from renderers._shared import JS_RESERVED


# Re-export for backward compatibility
detect_scenarios = None  # Will be defined below


def load_proto_type_map(proto_dir: Path) -> None:
    """Parse all *.proto files to build type maps."""
    _load_proto_type_map(proto_dir)


def set_scenario_groups(groups: list[dict]) -> None:
    """Set scenario groups from manifest."""
    _set_scenario_groups(groups)


def _detect_scenarios(probe_connector: dict) -> list[ScenarioSpec]:
    """
    Inspect probe data and return the applicable integration scenarios.
    
    This function is defined here rather than in renderers to maintain
    the exact same logic as the original.
    """
    flows = probe_connector.get("flows", {})

    def ok(flow_key: str, pm_key: str = "default") -> bool:
        return flows.get(flow_key, {}).get(pm_key, {}).get("status") == "supported"

    def has_payload(flow_key: str, pm_key: str = "default") -> bool:
        return bool(flows.get(flow_key, {}).get(pm_key, {}).get("proto_request"))

    _STATUS_HANDLING_MAP: dict[str, dict[str, str]] = {
        "checkout_card":        _AUTHORIZE_STATUS_HANDLING,
        "checkout_autocapture": _AUTOCAPTURE_STATUS_HANDLING,
        "checkout_wallet":      _AUTOCAPTURE_STATUS_HANDLING,
        "checkout_bank":        _AUTOCAPTURE_STATUS_HANDLING,
        "recurring":            _SETUP_RECURRING_STATUS_HANDLING,
    }

    scenarios: list[ScenarioSpec] = []

    for group in _SCENARIO_GROUPS:
        key = group.get("key", "")
        title = group.get("title", "")
        description = group.get("description", "")
        group_flows = group.get("flows", [])
        pm_key_fixed = group.get("pm_key")
        required_flows = group.get("required_flows", [])

        resolved_pm_key = pm_key_fixed
        supported = True

        for req in required_flows:
            req_flow = req.get("flow_key", "")
            req_pm = req.get("pm_key")
            req_pm_variants = req.get("pm_key_variants")

            if req_pm_variants:
                found_variant = None
                for variant in req_pm_variants:
                    if ok(req_flow, variant) and has_payload(req_flow, variant):
                        found_variant = variant
                        break
                if found_variant is None:
                    supported = False
                    break
                resolved_pm_key = found_variant
            elif req_pm:
                if not (ok(req_flow, req_pm) and has_payload(req_flow, req_pm)):
                    supported = False
                    break
            else:
                if not ok(req_flow):
                    supported = False
                    break
                _PAYLOAD_OPTIONAL_FLOWS = frozenset({"capture", "refund", "setup_recurring", "recurring_charge"})
                if req_flow not in _PAYLOAD_OPTIONAL_FLOWS and not has_payload(req_flow):
                    supported = False
                    break

        if not supported:
            continue

        if key == "checkout_bank" and resolved_pm_key:
            description = f"Direct bank debit ({resolved_pm_key}). Bank transfers typically use `capture_method=AUTOMATIC`."

        status_handling = _STATUS_HANDLING_MAP.get(key, {})

        scenarios.append(ScenarioSpec(
            key=key,
            title=title,
            flows=group_flows,
            pm_key=resolved_pm_key,
            description=description,
            status_handling=status_handling,
        ))

    return scenarios


# Assign the function
import sys
sys.modules[__name__].detect_scenarios = _detect_scenarios


# ── Backward-compat public API shims ───────────────────────────────────────────

def render_config_section(connector_name: str) -> list[str]:
    """Return markdown lines for the SDK Configuration section.
    
    Automatically includes all registered languages from RENDERERS.
    When a new language is added, it automatically appears here.
    """
    # Generate cells for ALL registered renderers dynamically
    cells = [
        _td(lang.title(), lang, renderer.config_snippet(connector_name))
        for lang, renderer in RENDERERS.items()
    ]

    header_row = "<tr>" + "".join(
        f"<td><b>{lang.title()}</b></td>"
        for lang in RENDERERS.keys()
    ) + "</tr>"

    return [
        "## SDK Configuration",
        "",
        "Use this config for all flows in this connector. "
        "Replace `YOUR_API_KEY` with your actual credentials.",
        "",
        "<table>",
        header_row,
        "<tr>",
        "\n".join(cells),
        "</tr>",
        "</table>",
        "",
    ]


def render_scenario_section(
    scenario: ScenarioSpec,
    connector_name: str,
    flow_payloads: dict[str, dict],
    flow_metadata: dict[str, dict],
    message_schemas: dict,
    ann_scenario: dict,
    line_numbers: dict[str, int] | None = None,
) -> list[str]:
    """Return markdown lines for one scenario subsection."""
    title      = ann_scenario.get("title", scenario.title)
    description = ann_scenario.get("description", scenario.description)
    status_hdl  = ann_scenario.get("status_handling", scenario.status_handling)

    out: list[str] = []
    a = out.append

    a(f"### {title}")
    a("")
    a(description)
    a("")

    if status_hdl:
        a("**Response status handling:**")
        a("")
        a("| Status | Recommended action |")
        a("|--------|-------------------|")
        for status, action in status_hdl.items():
            a(f"| `{status}` | {action} |")
        a("")

    # Link to example files with line numbers if available
    scenario_key = scenario.key
    base_py = f"../../examples/{connector_name}/python/{connector_name}.py"
    base_js = f"../../examples/{connector_name}/javascript/{connector_name}.js"
    base_kt = f"../../examples/{connector_name}/kotlin/{connector_name}.kt"
    base_rs = f"../../examples/{connector_name}/rust/{connector_name}.rs"
    
    ln_py = line_numbers.get("python", 0) if line_numbers else 0
    ln_js = line_numbers.get("javascript", 0) if line_numbers else 0
    ln_kt = line_numbers.get("kotlin", 0) if line_numbers else 0
    ln_rs = line_numbers.get("rust", 0) if line_numbers else 0
    
    py_link = f"{base_py}#L{ln_py}" if ln_py else base_py
    js_link = f"{base_js}#L{ln_js}" if ln_js else base_js
    kt_link = f"{base_kt}#L{ln_kt}" if ln_kt else base_kt
    rs_link = f"{base_rs}#L{ln_rs}" if ln_rs else base_rs
    
    a(f"**Examples:** [Python]({py_link}) · [JavaScript]({js_link}) · [Kotlin]({kt_link}) · [Rust]({rs_link})")
    a("")

    return out


def render_pm_reference_section(
    probe_connector: dict,
    flow_metadata: dict[str, dict],
    message_schemas: dict,
) -> list[str]:
    """Return markdown lines for ## Payment Method Reference."""
    flows    = probe_connector.get("flows", {})
    auth_pms = flows.get("authorize", {})
    grpc_req = flow_metadata.get("authorize", {}).get("grpc_request", "PaymentServiceAuthorizeRequest")
    db       = _SchemaDB(message_schemas)

    out: list[str] = []
    a = out.append

    rendered_any = False
    for pm_key, label in _PROBE_PM_LABELS.items():
        entry = auth_pms.get(pm_key, {})
        if entry.get("status") != "supported":
            continue
        proto_req = entry.get("proto_request", {})
        pm_payload = proto_req.get("payment_method", {})
        if not pm_payload:
            continue

        if not rendered_any:
            a("**Payment method objects** — use these in the `payment_method` field of the Authorize request.")
            a("")
            rendered_any = True

        a(f"##### {label}")
        a("")
        pm_msg = db.get_type(grpc_req, "payment_method")
        annotated = _build_annotated(pm_payload, pm_msg, db, style="python", indent=0)
        a("```python")
        a(f'"payment_method": {annotated}')
        a("```")
        a("")

    return out


def render_payload_block(
    flow_key: str,
    service_name: str,
    grpc_request: str,
    proto_request: dict,
    message_schemas: dict,
) -> list[str]:
    """Return markdown lines for a single annotated request payload block."""
    if not proto_request or not grpc_request:
        return []

    db           = _SchemaDB(message_schemas)
    client_cls   = _client_class(service_name)
    camel_method = _to_camel(flow_key)
    payload      = _build_annotated(proto_request, grpc_request, db, style="python", indent=0)

    return [
        "",
        f"> **Client call:** `{client_cls}.{camel_method}(request)`",
        "",
        "```python",
        payload,
        "```",
        "",
    ]


def render_llms_txt_entry(
    connector_name: str,
    display_name: str,
    probe_connector: dict,
    scenarios: list[ScenarioSpec],
) -> str:
    """Return one connector's block for docs/llms.txt."""
    flows    = probe_connector.get("flows", {})
    auth_pms = flows.get("authorize", {})

    supported_pms = [
        pm for pm in auth_pms
        if pm != "default" and auth_pms[pm].get("status") == "supported"
    ]
    supported_flows = [
        fk for fk, fdata in flows.items()
        if any(v.get("status") == "supported" for v in fdata.values())
    ]
    scenario_keys = [s.key for s in scenarios]
    example_paths = [f"examples/{connector_name}/python/{connector_name}.py"] if scenario_keys else []

    lines = [
        f"## {display_name}",
        f"connector_id: {connector_name}",
        f"doc: docs/connectors/{connector_name}.md",
        f"scenarios: {', '.join(scenario_keys) if scenario_keys else 'none'}",
        f"payment_methods: {', '.join(supported_pms) if supported_pms else 'none'}",
        f"flows: {', '.join(supported_flows)}",
        f"examples_python: {', '.join(example_paths) if example_paths else 'none'}",
        "",
    ]
    return "\n".join(lines)


# ── Dynamic render functions (replaces language-specific shims) ─────────────────

def render_consolidated(
    lang: str,
    connector_name: str,
    scenarios_with_payloads: list,
    flow_metadata: dict[str, dict],
    message_schemas: dict,
    flow_items: list | None = None,
) -> str:
    """Render consolidated file for any language.
    
    This replaces render_consolidated_python(), render_consolidated_javascript(), etc.
    
    Args:
        lang: Language identifier (e.g., 'python', 'go', 'ruby')
        connector_name: Name of the connector
        scenarios_with_payloads: List of (scenario, flow_payloads) tuples
        flow_metadata: Flow metadata dictionary
        message_schemas: Message schemas dictionary
        flow_items: Optional list of flow items
        
    Returns:
        Rendered consolidated file content
        
    Example:
        >>> content = render_consolidated('python', 'stripe', ...)
        >>> # Instead of: render_consolidated_python('stripe', ...)
    """
    return RENDERERS[lang].render_consolidated(
        connector_name, scenarios_with_payloads, flow_metadata, message_schemas, flow_items
    )


def render_scenario(
    lang: str,
    scenario: ScenarioSpec,
    connector_name: str,
    proto_requests: dict,
    flow_metadata: dict[str, dict],
    message_schemas: dict,
) -> str:
    """Render scenario file for any language.
    
    This replaces render_scenario_python(), render_scenario_javascript(), etc.
    """
    return RENDERERS[lang].render_scenario(
        scenario, connector_name, proto_requests, flow_metadata, message_schemas
    )


def render_flow(
    lang: str,
    flow_key: str,
    connector_name: str,
    proto_req: dict,
    flow_metadata: dict[str, dict],
    message_schemas: dict,
    pm_label: str = "",
) -> str:
    """Render single flow file for any language.
    
    This replaces render_flow_python(), render_flow_javascript(), etc.
    """
    return RENDERERS[lang].render_flow(
        flow_key, connector_name, proto_req, flow_metadata, message_schemas, pm_label
    )


# ── Legacy shims for backward compatibility (deprecated) ───────────────────────

def _make_legacy_shim(lang: str, method: str):
    """Create a legacy shim function for backward compatibility."""
    def shim(*args, **kwargs):
        import warnings
        warnings.warn(
            f"render_{method}_{lang}() is deprecated. Use render_{method}(lang='{lang}', ...)",
            DeprecationWarning,
            stacklevel=2
        )
        renderer = RENDERERS[lang]
        return getattr(renderer, f"render_{method}")(*args, **kwargs)
    return shim


# Create deprecated legacy functions for backward compatibility
render_consolidated_python = _make_legacy_shim("python", "consolidated")
render_consolidated_javascript = _make_legacy_shim("javascript", "consolidated")
render_consolidated_kotlin = _make_legacy_shim("kotlin", "consolidated")
render_consolidated_rust = _make_legacy_shim("rust", "consolidated")
render_scenario_python = _make_legacy_shim("python", "scenario")
render_scenario_javascript = _make_legacy_shim("javascript", "scenario")
render_flow_python = _make_legacy_shim("python", "flow")
render_flow_javascript = _make_legacy_shim("javascript", "flow")
render_flow_kotlin = _make_legacy_shim("kotlin", "flow")
render_flow_rust = _make_legacy_shim("rust", "flow")


# Helper needed for render_payload_block
def _to_camel(snake: str) -> str:
    """Convert snake_case to camelCase."""
    parts = snake.split("_")
    return parts[0] + "".join(p.title() for p in parts[1:])


# ── Re-exports for docs/generate.py ────────────────────────────────────────────

__all__ = [
    # Core public API
    "ScenarioSpec",
    "detect_scenarios",
    "load_proto_type_map",
    "set_scenario_groups",
    "render_config_section",
    "render_scenario_section",
    "render_pm_reference_section",
    "render_payload_block",
    "render_llms_txt_entry",
    # Dynamic render functions (new preferred API)
    "render_consolidated",
    "render_scenario", 
    "render_flow",
    # Legacy functions (deprecated, for backward compatibility)
    "render_consolidated_python",
    "render_consolidated_javascript",
    "render_consolidated_kotlin",
    "render_consolidated_rust",
    "render_scenario_python",
    "render_scenario_javascript",
    "render_flow_python",
    "render_flow_javascript",
    "render_flow_kotlin",
    "render_flow_rust",
]
