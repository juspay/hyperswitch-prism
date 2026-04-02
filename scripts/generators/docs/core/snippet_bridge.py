"""
SDK example generators.

Generates runnable process_*(merchant_transaction_id, config) functions
for all supported languages. The smoke tests discover and call these functions.
"""

import json
from pathlib import Path
from typing import Dict, List, Tuple, Optional
import sys

sys.path.insert(0, str(Path(__file__).parent))
from core.models import Scenario
from core.hydrator import ScenarioHydrator
from core.integration import load_scenarios, load_status_mapping
from renderers import PythonRenderer, JavaScriptRenderer, KotlinRenderer, RustRenderer
from renderers.base import _FLOW_PROTO_TYPES, _fallback_proto_type


def _get_proto_type(flow_key: str) -> str:
    """Return proto request class name from the manifest-populated registry."""
    return _FLOW_PROTO_TYPES.get(flow_key, _fallback_proto_type(flow_key))


# JavaScript reserved words
JS_RESERVED = frozenset({
    "void", "delete", "return", "new", "in", "do", "for", "if", 
    "else", "while", "break", "continue", "function", "var", "let", 
    "const", "class", "extends", "super", "this", "import", "export", 
    "default", "try", "catch", "finally", "throw", "switch", "case"
})


# PROTOBUF_REQUEST_MAP kept as a public alias so generate.py's existing import
# still resolves. Delegates to the manifest-populated registry — no hardcoding.
PROTOBUF_REQUEST_MAP = _FLOW_PROTO_TYPES  # type: ignore[assignment]


def _to_camel(snake: str) -> str:
    """Convert snake_case to camelCase."""
    parts = snake.split("_")
    return parts[0] + "".join(p.title() for p in parts[1:])


# Cache for probe data
_PROBE_DATA_CACHE: Dict[str, dict] = {}


def _set_probe_data_cache(cache: Dict[str, dict]):
    """Set probe data cache."""
    global _PROBE_DATA_CACHE
    _PROBE_DATA_CACHE = cache


def _find_probe_data(connector_name: str) -> Optional[dict]:
    """Find probe data for connector."""
    if connector_name in _PROBE_DATA_CACHE:
        return _PROBE_DATA_CACHE[connector_name]
    probe_path = Path(__file__).parent.parent.parent.parent / "data" / "field_probe" / f"{connector_name}.json"
    if probe_path.exists():
        with open(probe_path) as f:
            data = json.load(f)
            _PROBE_DATA_CACHE[connector_name] = data
            return data
    return None


# Dummy ScenarioSpec for compatibility
class ScenarioSpec:
    """Scenario spec compatible with old API."""
    def __init__(self, key, name, description, flows, pm_key=None):
        self.key = key
        self.name = name
        self.description = description
        self.flows = flows
        self.pm_key = pm_key


def detect_scenarios(probe_connector: dict) -> list:
    """Detect supported scenarios from probe data."""
    base_path = Path(__file__).parent.parent
    scenarios = load_scenarios(base_path / "specs")
    connector_name = probe_connector.get("connector", "unknown")
    hydrator = ScenarioHydrator(probe_connector, connector_name)
    
    supported = []
    for scenario in scenarios:
        hydrated = hydrator.hydrate(scenario)
        if hydrated:
            pm_key = None
            for flow in hydrated.flows:
                if flow.name == "authorize":
                    pm_data = flow.payload.get("payment_method_data", {})
                    if pm_data:
                        pm_key = list(pm_data.keys())[0] if pm_data else None
                    break
            
            supported.append(ScenarioSpec(
                key=scenario.key,
                name=scenario.name,
                description=scenario.description,
                flows=[f.name for f in scenario.flows],
                pm_key=pm_key
            ))
    
    return supported


def _extract_flow_payloads(
    connector_name: str,
    scenarios_with_payloads: List[Tuple]
) -> Dict[str, dict]:
    """Extract unique flow payloads from scenarios."""
    flow_payloads = {}
    
    for scenario_spec, _ in scenarios_with_payloads:
        probe_data = _find_probe_data(connector_name)
        if not probe_data:
            continue
        
        base_path = Path(__file__).parent.parent
        all_scenarios = load_scenarios(base_path / "specs")
        scenario = next((s for s in all_scenarios if s.key == scenario_spec.key), None)
        
        if scenario:
            hydrator = ScenarioHydrator(probe_data, connector_name)
            hydrated = hydrator.hydrate(scenario)
            if hydrated:
                for flow in hydrated.flows:
                    if flow.payload and flow.name not in flow_payloads:
                        flow_payloads[flow.name] = flow.payload
    
    return flow_payloads




def render_consolidated_python(
    connector_name: str,
    scenarios_with_payloads: List[Tuple],
    flow_metadata: dict,
    message_schemas: dict,
    flow_items: List[Tuple]
) -> str:
    """Generate Python examples using the new renderer architecture."""
    probe_data = _find_probe_data(connector_name)
    if not probe_data:
        return f"# {connector_name} — probe data not found\n"

    base_path = Path(__file__).parent.parent
    all_scenarios = load_scenarios(base_path / "specs")
    status_mapping = load_status_mapping(base_path / "config")
    hydrator = ScenarioHydrator(probe_data, connector_name)

    connector_title = connector_name.title()
    lines = [
        f"# This file is auto-generated. Do not edit manually.",
        f"# Regenerate: python3 scripts/generators/docs/generate.py {connector_name} --probe-path data/field_probe",
        f"# {connector_title} — integration scenarios",
        "",
        "from google.protobuf.json_format import ParseDict",
        "from payments import (",
        "    PaymentClient,",
        "    RecurringPaymentClient,",
        "    PaymentMethodClient,",
        "    MerchantAuthenticationClient,",
        "    CustomerClient,",
        "    PaymentMethodAuthenticationClient,",
        ")",
        "from payments.generated import sdk_config_pb2, payment_pb2",
        "",
        "_default_config = sdk_config_pb2.ConnectorConfig(",
        "    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),",
        ")",
        "ConnectorConfig = sdk_config_pb2.ConnectorConfig",
        "",
    ]

    # Process functions from new renderers
    for scenario in all_scenarios:
        hydrated = hydrator.hydrate(scenario)
        if hydrated:
            code = PythonRenderer(hydrated, status_mapping["python"]).render()
            lines.append(code)
            lines.append("")

    return "\n".join(lines)


def render_consolidated_javascript(
    connector_name: str,
    scenarios_with_payloads: List[Tuple],
    flow_metadata: dict,
    message_schemas: dict,
    flow_items: List[Tuple]
) -> str:
    """Generate JavaScript examples using the new renderer architecture."""
    probe_data = _find_probe_data(connector_name)
    if not probe_data:
        return f"// {connector_name} — probe data not found\n"

    base_path = Path(__file__).parent.parent
    all_scenarios = load_scenarios(base_path / "specs")
    status_mapping = load_status_mapping(base_path / "config")
    hydrator = ScenarioHydrator(probe_data, connector_name)

    lines = [f"// {connector_name} SDK Examples", ""]

    # Builder functions for smoke test compat
    flow_payloads = _extract_flow_payloads(connector_name, scenarios_with_payloads)
    for flow_key, payload in flow_payloads.items():
        camel_flow = "".join(p.capitalize() for p in flow_key.split("_"))
        lines.append(f"function _build{camel_flow}Request(arg) {{")
        lines.append(f"    const payload = {json.dumps(payload, indent=4)};")
        lines.append("    if (arg) {")
        lines.append("        if (arg === 'AUTOMATIC' || arg === 'MANUAL') {")
        lines.append("            payload.capture_method = arg;")
        lines.append("        } else if (typeof arg === 'string') {")
        lines.append("            payload.connector_transaction_id = arg;")
        lines.append("        }")
        lines.append("    }")
        lines.append("    return payload;")
        lines.append("}")
        lines.append("")

    # Process functions from new renderers
    for scenario in all_scenarios:
        hydrated = hydrator.hydrate(scenario)
        if hydrated:
            code = JavaScriptRenderer(hydrated, status_mapping["javascript"]).render()
            lines.append(code)
            lines.append("")

    return "\n".join(lines)


def render_consolidated_kotlin(
    connector_name: str,
    scenarios_with_payloads: List[Tuple],
    flow_metadata: dict,
    message_schemas: dict,
    flow_items: List[Tuple]
) -> str:
    """Generate Kotlin examples using the new renderer architecture."""
    probe_data = _find_probe_data(connector_name)
    if not probe_data:
        return f"// {connector_name} — probe data not found\n"

    base_path = Path(__file__).parent.parent
    all_scenarios = load_scenarios(base_path / "specs")
    status_mapping = load_status_mapping(base_path / "config")
    hydrator = ScenarioHydrator(probe_data, connector_name)

    lines = [f"// {connector_name} SDK Examples", ""]
    lines.append("package examples")
    lines.append("")
    lines.append("import com.payments.sdk.*")
    lines.append("import com.google.protobuf.util.JsonFormat")
    lines.append("")

    # Builder functions for smoke test compat
    flow_payloads = _extract_flow_payloads(connector_name, scenarios_with_payloads)
    for flow_key, payload in flow_payloads.items():
        camel_flow = "".join(p.capitalize() for p in flow_key.split("_"))
        proto_type = _get_proto_type(flow_key)
        lines.append(f"fun _build{camel_flow}Request(arg: String? = null): {proto_type} {{")
        lines.append(f'    val json = """{json.dumps(payload)}""".trimIndent()')
        lines.append(f"    val builder = {proto_type}.newBuilder()")
        lines.append("    JsonFormat.parser().merge(json, builder)")
        lines.append("    if (arg != null) {")
        lines.append('        when (arg) {')
        lines.append('            "AUTOMATIC", "MANUAL" -> builder.captureMethod = arg')
        lines.append('            else -> builder.connectorTransactionId = arg')
        lines.append("        }")
        lines.append("    }")
        lines.append("    return builder.build()")
        lines.append("}")
        lines.append("")

    # Process functions from new renderers
    for scenario in all_scenarios:
        hydrated = hydrator.hydrate(scenario)
        if hydrated:
            code = KotlinRenderer(hydrated, status_mapping["kotlin"]).render()
            lines.append(code)
            lines.append("")

    return "\n".join(lines)


def render_consolidated_rust(
    connector_name: str,
    scenarios_with_payloads: List[Tuple],
    flow_metadata: dict,
    message_schemas: dict,
    flow_items: List[Tuple]
) -> str:
    """Generate Rust examples using the new renderer architecture."""
    probe_data = _find_probe_data(connector_name)
    if not probe_data:
        return f"// {connector_name} — probe data not found\n"

    base_path = Path(__file__).parent.parent
    all_scenarios = load_scenarios(base_path / "specs")
    status_mapping = load_status_mapping(base_path / "config")
    hydrator = ScenarioHydrator(probe_data, connector_name)

    lines = [f"// {connector_name} SDK Examples", ""]
    lines.append("use serde_json::json;")
    lines.append("")

    # Builder functions for smoke test compat
    flow_payloads = _extract_flow_payloads(connector_name, scenarios_with_payloads)
    for flow_key, payload in flow_payloads.items():
        lines.append(f"fn _build_{flow_key}_request(arg: Option<&str>) -> serde_json::Value {{")
        lines.append(f"    let mut payload = serde_json::json!({json.dumps(payload)});")
        lines.append("    if let Some(a) = arg {")
        lines.append("        match a {")
        lines.append('            "AUTOMATIC" | "MANUAL" => { payload["capture_method"] = json!(a); }')
        lines.append('            _ => { payload["connector_transaction_id"] = json!(a); }')
        lines.append("        }")
        lines.append("    }")
        lines.append("    payload")
        lines.append("}")
        lines.append("")

    # Process functions from new renderers
    for scenario in all_scenarios:
        hydrated = hydrator.hydrate(scenario)
        if hydrated:
            code = RustRenderer(hydrated, status_mapping["rust"]).render()
            lines.append(code)
            lines.append("")

    return "\n".join(lines)


def validate_all_language_parity(connector_name: str) -> List[str]:
    """
    Render all scenarios in all 4 languages and check structural parity.
    Returns list of inconsistency errors. Empty = consistent.
    """
    from core.validator import validate_structural_parity

    probe_data = _find_probe_data(connector_name)
    if not probe_data:
        return [f"No probe data for {connector_name}"]

    base_path = Path(__file__).parent.parent
    all_scenarios = load_scenarios(base_path / "specs")
    status_mapping = load_status_mapping(base_path / "config")
    hydrator = ScenarioHydrator(probe_data, connector_name)

    all_errors = []

    for scenario in all_scenarios:
        hydrated = hydrator.hydrate(scenario)
        if not hydrated:
            continue

        manifests = [
            PythonRenderer(hydrated, status_mapping["python"]).get_manifest(),
            JavaScriptRenderer(hydrated, status_mapping["javascript"]).get_manifest(),
            KotlinRenderer(hydrated, status_mapping["kotlin"]).get_manifest(),
            RustRenderer(hydrated, status_mapping["rust"]).get_manifest(),
        ]

        errors = validate_structural_parity(manifests)
        all_errors.extend(errors)

    return all_errors


def render_config_section(connector_name: str) -> List[str]:
    """Render SDK configuration section."""
    return [
        f"## SDK Configuration",
        "",
        f"Configure the SDK for {connector_name}:",
        "",
        "```python",
        "from payments import PaymentClient, ConnectorConfig, Environment",
        "",
        f"config = ConnectorConfig(connector='{connector_name}')",
        "client = PaymentClient(config)",
        "```",
        "",
    ]


def render_llms_txt_entry(
    connector_name: str,
    display_name: str,
    probe_connector: dict,
    scenarios: list
) -> str:
    """Generate llms.txt entry."""
    flows = list(probe_connector.get("flows", {}).keys())
    scenarios_list = [s.key for s in scenarios]
    
    lines = [
        f"",
        f"connector: {connector_name}",
        f"  display_name: {display_name}",
        f"  doc: docs-generated/connectors/{connector_name}.md",
        f"  examples:",
        f"    python: examples/{connector_name}/python/{connector_name}.py",
        f"    javascript: examples/{connector_name}/javascript/{connector_name}.js",
        f"    kotlin: examples/{connector_name}/kotlin/{connector_name}.kt",
        f"    rust: examples/{connector_name}/rust/{connector_name}.rs",
        f"  flows: {', '.join(flows[:10])}{'...' if len(flows) > 10 else ''}",
        f"  scenarios: {', '.join(scenarios_list)}",
    ]
    return "\n".join(lines)


# Dummy functions for compatibility
def set_scenario_groups(groups: list):
    pass


def load_proto_type_map(proto_dir: Path):
    pass
