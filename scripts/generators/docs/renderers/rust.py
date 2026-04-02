"""
Rust renderer for SDK examples.

Translates HydratedScenario into Rust code using:
- serde_json::json! macro for request payloads
- if/return Err() for error handling
- snake_case variables and JSON index access
"""

import json
from typing import List

from core.models import HydratedFlow, StatusRule, StatusAction, FieldLink
from renderers.base import BaseRenderer, REF_SENTINEL


class RustRenderer(BaseRenderer):
    """Render SDK examples in Rust."""

    language_name = "rust"

    def _resolve_return_ref(self, ref: str) -> str:
        """Replace 'response.X' with last_flow_var["X"] JSON indexing."""
        last_var = self._get_last_flow_var()
        if ref.startswith("response."):
            field = ref[len("response."):]
            return f'{last_var}["{field}"]'
        return ref

    def _render_function_signature(self) -> List[str]:
        """Render async function signature."""
        return [
            f"async fn process_{self.scenario.key}() -> Result<serde_json::Value, Box<dyn std::error::Error>> {{",
            f"    // {self.scenario.description}",
        ]

    def _render_prerequisite(self, prereq_flow: HydratedFlow) -> List[str]:
        """Render prerequisite flow with comment."""
        lines = [
            f"    // Prerequisite: {self._get_flow_description(prereq_flow.name)}",
        ]

        if prereq_flow.payload:
            payload_str = self._dict_to_rust(prereq_flow.payload)
            lines.append(f"    let request = {payload_str};")
        else:
            lines.append(f"    let request = serde_json::json!({{}});")

        lines.append(f"    let {prereq_flow.name}_response = client.{prereq_flow.name}(&request).await?;")

        return lines

    def _render_request(self, flow: HydratedFlow) -> List[str]:
        """Render request payload with field links substituted."""
        payload = self._substitute_field_links(flow.payload, flow.field_links)

        if payload:
            payload_str = self._dict_to_rust(payload)
            return [f"    let request = {payload_str};"]
        else:
            return [f"    let request = serde_json::json!({{}});"]

    def _render_api_call(self, flow_name: str, response_var: str) -> List[str]:
        """Render API call."""
        return [f"    let {response_var} = client.{flow_name}(&request).await?;"]

    def _render_status_rule(self, response_var: str, rule: StatusRule) -> List[str]:
        """
        Render status handling.

        Rust uses if expression with as_str() comparison:
            if response["status"].as_str() == Some("FAILED") {
                return Err(format!("Payment failed: {}", response["error"]).into());
            }
        """
        lines = []

        status_checks = [self._map_status_value(s) for s in rule.status]

        if len(status_checks) == 1:
            condition = f'{response_var}["status"].as_str() == Some({status_checks[0]})'
        else:
            statuses = ', '.join(status_checks)
            condition = f'[{statuses}].contains(&{response_var}["status"].as_str().unwrap_or(""))'

        lines.append(f'    if {condition} {{')

        if rule.action == StatusAction.ERROR:
            message = rule.message or "Operation failed"
            message = message.replace('{error}', '{}')
            lines.append(f'        return Err(format!("{message}", {response_var}["error"]).into());')

        elif rule.action == StatusAction.RETURN_EARLY:
            if rule.return_fields:
                fields = ', '.join(
                    f'"{k}": {self._resolve_return_ref(v)}'
                    for k, v in rule.return_fields.items()
                )
                lines.append(f'        return Ok(serde_json::json!({{ {fields} }}));')
            else:
                lines.append(f'        return Ok({response_var}.clone());')

        lines.append('    }')
        lines.append('')
        return lines

    def _render_field_link(self, link: FieldLink) -> str:
        """Render field reference from previous response using JSON indexing."""
        prev_var = self._get_previous_response_var(link.from_flow)
        return f'{prev_var}["{link.from_field}"]'

    def _render_return(self) -> List[str]:
        """Render return statement."""
        if not self.scenario.return_fields:
            return ['    Ok(serde_json::json!(null))', '}']

        fields = ', '.join(
            f'"{key}": {self._resolve_return_ref(value)}'
            for key, value in self.scenario.return_fields.items()
        )
        return [f'    Ok(serde_json::json!({{ {fields} }}))', '}']

    def _map_status_value(self, status: str) -> str:
        """
        Map abstract status to a Rust string literal.

        Uses status_mapping.yaml lookup; falls back to quoted string.
        """
        mapped = self.status_mapping.get(status)
        if mapped:
            return mapped  # Already a quoted string from YAML, e.g. '"FAILED"'
        return f'"{status}"'

    # ─── Helpers ──────────────────────────────────────────────────────────────

    def _dict_to_rust(self, obj) -> str:
        """Convert dict to Rust serde_json::json! macro representation."""
        if obj is None:
            return "serde_json::json!(null)"
        elif isinstance(obj, bool):
            return str(obj).lower()
        elif isinstance(obj, str):
            if obj.startswith(REF_SENTINEL):
                return obj[len(REF_SENTINEL):]  # Strip sentinel, emit as code reference
            return json.dumps(obj)
        elif isinstance(obj, (int, float)):
            return str(obj)
        elif isinstance(obj, dict):
            items = [f'{json.dumps(k)}: {self._dict_to_rust(v)}' for k, v in obj.items()]
            return 'serde_json::json!({' + ', '.join(items) + '})'
        elif isinstance(obj, list):
            items = [self._dict_to_rust(item) for item in obj]
            return 'vec![' + ', '.join(items) + ']'
        else:
            return json.dumps(obj)

    def _get_flow_description(self, flow_name: str) -> str:
        """Get human-readable description for a flow."""
        descriptions = {
            'create_access_token': 'Obtain OAuth2 access token',
            'create_customer': 'Create customer profile',
            'create_order': 'Create order before payment',
        }
        return descriptions.get(flow_name, flow_name.replace('_', ' ').title())
