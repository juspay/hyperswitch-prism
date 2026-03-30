"""Rust language renderer."""

from __future__ import annotations

from .base import BaseRenderer
from ._shared import (
    _SchemaDB, _conn_enum_rust, _CONNECTOR_CONFIG_FIELDS, _STEP_DESCRIPTIONS,
    _FLOW_KEY_TO_METHOD, _FLOW_KEY_TO_GRPC_REQUEST, _json_scalar
)


class Renderer(BaseRenderer):
    """Rust SDK snippet renderer."""
    
    lang = "rust"
    extension = ".rs"

    def config_snippet(self, connector_name: str) -> str:
        """Generate Rust SDK config snippet with actual connector fields."""
        info = _CONNECTOR_CONFIG_FIELDS.get(connector_name, {})
        fields = info.get("required_fields", [])
        msg_name = info.get("msg_name", "")
        
        if fields and msg_name:
            # Generate actual config with fields
            variant = connector_name.capitalize()
            field_lines = "\n".join(
                f'                {f}: Some(Secret::new("YOUR_{f.upper()}".to_string())),'
                for f in fields
            )
            return f'''use grpc_api_types::payments::{{connector_specific_config, *}};
use hyperswitch_payments_client::ConnectorClient;
use hyperswitch_masking::Secret;

let config = ConnectorConfig {{
    connector_config: Some(ConnectorSpecificConfig {{
        config: Some(connector_specific_config::Config::{variant}({msg_name} {{
{field_lines}
            ..Default::default()
        }})),
    }}),
    options: Some(SdkOptions {{ environment: Environment::Sandbox.into() }}),
}};
let client = ConnectorClient::new(config, None).unwrap();'''
        
        # Fallback for connectors without specific config
        return '''use grpc_api_types::payments::*;
use hyperswitch_payments_client::ConnectorClient;

let config = ConnectorConfig {
    connector_config: None,  // TODO: set connector credentials
    options: Some(SdkOptions { environment: Environment::Sandbox.into() }),
};
let client = ConnectorClient::new(config, None).unwrap();'''

    def _build_json_body(self, proto_req: dict, msg_name: str, message_schemas: dict, indent: int = 1) -> list[str]:
        """Build serde_json::json! body from proto request."""
        from ._shared import _PROTO_WRAPPER_TYPES, _ONEOF_WRAPPER_FIELD, _PROTO_REPEATED_FIELDS
        
        pad = "    " * indent
        db = _SchemaDB(message_schemas)
        lines = []
        items = list(proto_req.items())
        
        for idx, (key, val) in enumerate(items):
            trailing = ","  # serde_json::json! allows trailing commas; always add for safety
            comment = db.get_comment(msg_name, key)
            child_msg = db.get_type(msg_name, key)
            cmt_part = f"  // {comment}" if comment else ""
            
            if isinstance(val, dict):
                if child_msg and child_msg in _PROTO_WRAPPER_TYPES:
                    # Wrapper - extract value
                    inner_val = val.get("value", "")
                    lines.append(f'{pad}"{key}": {_json_scalar(inner_val)}{trailing}{cmt_part}')
                elif child_msg and child_msg in _ONEOF_WRAPPER_FIELD:
                    # Oneof wrapper - add wrapper key
                    wrapper_key = _ONEOF_WRAPPER_FIELD[child_msg]
                    inner = self._build_json_body(val, child_msg, message_schemas, indent + 2)
                    lines.append(f'{pad}"{key}": {{{cmt_part}')
                    lines.append(f'{pad}    "{wrapper_key}": {{')
                    lines.extend(inner)
                    lines.append(f'{pad}    }}')
                    lines.append(f'{pad}}}{trailing}')
                else:
                    # Regular nested object
                    inner = self._build_json_body(val, child_msg, message_schemas, indent + 1)
                    lines.append(f'{pad}"{key}": {{{cmt_part}')
                    lines.extend(inner)
                    lines.append(f'{pad}}}{trailing}')
            elif isinstance(val, bool):
                lines.append(f'{pad}"{key}": {str(val).lower()}{trailing}{cmt_part}')
            elif isinstance(val, (int, float)):
                lines.append(f'{pad}"{key}": {val}{trailing}{cmt_part}')
            elif isinstance(val, str):
                lines.append(f'{pad}"{key}": {_json_scalar(val)}{trailing}{cmt_part}')
        
        # Add empty arrays for repeated fields
        msg_repeated = _PROTO_REPEATED_FIELDS.get(msg_name, set())
        for rep_field in sorted(msg_repeated - set(proto_req.keys())):
            comment = db.get_comment(msg_name, rep_field)
            cmt_part = f"  // {comment}" if comment else ""
            lines.append(f'{pad}"{rep_field}": [],{cmt_part}')
        
        return lines

    def render_consolidated(self, connector_name, scenarios_with_payloads,
                           flow_metadata, message_schemas, flow_items=None):
        """Generate Rust file with all scenarios and flows."""
        db = _SchemaDB(message_schemas)
        
        # Generate scenario functions
        scenario_funcs = []
        for scenario, flow_payloads in scenarios_with_payloads:
            func = self._gen_scenario_func(scenario, flow_payloads, flow_metadata, db)
            scenario_funcs.append(func)
        
        # Generate flow functions
        flow_funcs = []
        if flow_items:
            for flow_key, proto_req, pm_label in flow_items:
                func = self._gen_flow_func(flow_key, proto_req, flow_metadata, db, pm_label)
                flow_funcs.append(func)
        
        # Assemble file
        funcs_text = "\n\n".join(scenario_funcs + flow_funcs)
        
        # Get imports based on whether we have config fields
        info = _CONNECTOR_CONFIG_FIELDS.get(connector_name, {})
        if info.get("required_fields"):
            imports = '''use grpc_api_types::payments::{connector_specific_config, *};
use hyperswitch_payments_client::ConnectorClient;
use hyperswitch_masking::Secret;
use std::collections::HashMap;'''
        else:
            imports = '''use grpc_api_types::payments::*;
use hyperswitch_payments_client::ConnectorClient;
use std::collections::HashMap;'''
        
        return f'''// Auto-generated for {connector_name}
// Run: cargo run --example {connector_name} -- process_checkout_card

{imports}

fn build_client() -> ConnectorClient {{
    let config = ConnectorConfig {{
        connector_config: None,  // TODO: set credentials
        options: Some(SdkOptions {{ environment: Environment::Sandbox.into() }}),
    }};
    ConnectorClient::new(config, None).unwrap()
}}

{funcs_text}

#[tokio::main]
async fn main() {{
    let client = build_client();
    let flow = std::env::args().nth(1).unwrap_or_else(|| "authorize".to_string());
    println!("Running flow: {{}}", flow);
}}'''

    def _gen_scenario_func(self, scenario, flow_payloads, flow_metadata, db):
        """Generate single scenario function with actual payload."""
        func_name = f"process_{scenario.key}"
        
        lines = [
            f"#[allow(dead_code)]",
            f"pub async fn {func_name}(client: &ConnectorClient, _merchant_id: &str) -> Result<String, Box<dyn std::error::Error>> {{",
            f"    // {scenario.title}",
        ]
        
        for step_num, flow_key in enumerate(scenario.flows, 1):
            desc = _STEP_DESCRIPTIONS.get(flow_key, flow_key)
            meta = flow_metadata.get(flow_key, {})
            grpc_req = (meta.get("grpc_request", "")
                        or _FLOW_KEY_TO_GRPC_REQUEST.get(flow_key, ""))

            # Get payload for this flow
            payload = flow_payloads.get(flow_key, {})
            if flow_key == "authorize":
                # Adjust capture_method based on scenario
                if scenario.key in ("checkout_card", "void_payment", "get_payment"):
                    payload = dict(payload)
                    payload["capture_method"] = "MANUAL"
                elif scenario.key == "refund":
                    payload = dict(payload)
                    payload["capture_method"] = "AUTOMATIC"
            
            lines.append(f"    // Step {step_num}: {desc}")
            
            if grpc_req:
                # Generate actual JSON body
                json_lines = self._build_json_body(payload, grpc_req, {})
                json_body = "\n".join(json_lines)
                lines.append(f"    let response = client.{flow_key}(")
                # Use ::from_value with proper JSON - json!({...}) produces Value, from_value converts it
                lines.append(f"        serde_json::from_value::<{grpc_req}>(serde_json::json!({{")
                if json_lines:
                    lines.append(json_body)
                lines.append("        })).unwrap_or_default(),")
                lines.append("        &HashMap::new(), None")
                lines.append("    ).await?;")
            else:
                lines.append(f"    let response = client.{flow_key}(todo!(), &HashMap::new(), None).await?;")
            
            lines.append("")
        
        lines.append('    Ok("success".to_string())')
        lines.append("}")
        
        return "\n".join(lines)

    def _gen_flow_func(self, flow_key, proto_req, flow_metadata, db, pm_label):
        """Generate single flow function with actual payload."""
        meta = flow_metadata.get(flow_key, {})
        svc = meta.get("service_name", "PaymentService")
        rpc = meta.get("rpc_name", flow_key)
        grpc_req = (meta.get("grpc_request", "")
                    or _FLOW_KEY_TO_GRPC_REQUEST.get(flow_key, ""))
        
        comment = f"// Flow: {svc}.{rpc}"
        if pm_label:
            comment += f" ({pm_label})"
        
        lines = [
            f"#[allow(dead_code)]",
            f"pub async fn {flow_key}(client: &ConnectorClient, _merchant_id: &str) -> Result<String, Box<dyn std::error::Error>> {{",
            f"    {comment}",
        ]
        
        if proto_req:
            # Generate actual JSON body
            json_lines = self._build_json_body(proto_req, grpc_req or "Request", {})
            json_body = "\n".join(json_lines)
            lines.append(f"    let response = client.{flow_key}(")
            if grpc_req:
                lines.append(f"        serde_json::from_value::<{grpc_req}>(serde_json::json!({{")
            else:
                lines.append(f"        serde_json::json!({{")
            lines.append(json_body)
            if grpc_req:
                lines.append("        })).unwrap_or_default(),")
            else:
                lines.append("        }).into(),")
            lines.append("        &HashMap::new(), None")
            lines.append("    ).await?;")
        else:
            lines.append(f"    let response = client.{flow_key}(todo!(), &HashMap::new(), None).await?;")
        
        lines.append('    Ok("success".to_string())')
        lines.append("}")
        
        return "\n".join(lines)
