"""
JavaScript/TypeScript renderer for SDK examples.

Translates HydratedScenario into JavaScript code using:
- Object literals for request payloads
- throw new Error() for error handling
- camelCase variables
"""

import json
from typing import List

from core.models import HydratedFlow, StatusRule, StatusAction, FieldLink
from renderers.base import BaseRenderer, REF_SENTINEL


class JavaScriptRenderer(BaseRenderer):
    """Render SDK examples in JavaScript/TypeScript."""

    language_name = "javascript"

    def _assign_flow_variable(self, flow_name: str) -> str:
        """Assign a camelCase variable name for this flow's response."""
        var = self._to_camel_case(f"{flow_name}_response")
        self.flow_vars[flow_name] = var
        return var

    def _resolve_return_ref(self, ref: str) -> str:
        """Replace 'response.X' with camelCase last_flow_var.camelField."""
        last_var = self._get_last_flow_var()  # already camelCase
        if ref.startswith("response."):
            field = ref[len("response."):]
            return f"{last_var}.{self._to_camel_case(field)}"
        return ref

    def _render_function_signature(self) -> List[str]:
        """Render async function signature."""
        func_name = "process" + "".join(p.capitalize() for p in self.scenario.key.split("_"))
        return [
            f"async function {func_name}() {{",
            f"    // {self.scenario.description}",
        ]

    def _render_prerequisite(self, prereq_flow: HydratedFlow) -> List[str]:
        """Render prerequisite flow with comment."""
        lines = [
            f"    // Prerequisite: {self._get_flow_description(prereq_flow.name)}",
        ]

        if prereq_flow.payload:
            payload_str = self._dict_to_js(prereq_flow.payload)
            lines.append(f"    const request = {payload_str};")
        else:
            lines.append(f"    const request = {{}};")

        camel_var = self._to_camel_case(f"{prereq_flow.name}_response")
        lines.append(f"    const {camel_var} = await client.{prereq_flow.name}(request);")

        return lines

    def _render_request(self, flow: HydratedFlow) -> List[str]:
        """Render request payload with field links substituted."""
        payload = self._substitute_field_links(flow.payload, flow.field_links)
        req_var = self._to_camel_case(f"{flow.name}_request")
        if payload:
            payload_str = self._dict_to_js(payload)
            return [f"    const {req_var} = {payload_str};"]
        else:
            return [f"    const {req_var} = {{}};"]

    def _render_api_call(self, flow_name: str, response_var: str) -> List[str]:
        """Render API call."""
        req_var = self._to_camel_case(f"{flow_name}_request")
        return [f"    const {response_var} = await client.{flow_name}({req_var});"]

    def _render_status_rule(self, response_var: str, rule: StatusRule) -> List[str]:
        """
        Render status handling.

        JavaScript uses if/throw pattern:
            if (["FAILED", "AUTHORIZATION_FAILED"].includes(response.status)) {
                throw new Error("Payment failed: " + error);
            }
        """
        lines = []

        status_checks = [self._map_status_value(s) for s in rule.status]

        if len(status_checks) == 1:
            condition = f'{response_var}.status === {status_checks[0]}'
        else:
            statuses_str = ', '.join(status_checks)
            condition = f'[{statuses_str}].includes({response_var}.status)'

        lines.append(f"    if ({condition}) {{")

        if rule.action == StatusAction.ERROR:
            message = rule.message or "Operation failed"
            message = message.replace('{error}', f'" + {response_var}.error?.message + "')
            lines.append(f'        throw new Error("{message}");')

        elif rule.action == StatusAction.RETURN_EARLY:
            if rule.return_fields:
                fields = ', '.join(
                    f'{k}: {self._resolve_return_ref(v)}'
                    for k, v in rule.return_fields.items()
                )
                lines.append(f'        return {{ {fields} }};')
            else:
                lines.append(f'        return {response_var};')

        elif rule.action == StatusAction.CONTINUE:
            pass  # No-op: handled in base render() loop

        lines.append("    }")

        return lines

    def _render_field_link(self, link: FieldLink) -> str:
        """Render field reference from previous response (camelCase)."""
        prev_var = self._get_previous_response_var(link.from_flow)
        field_camel = self._to_camel_case(link.from_field)
        return f'{prev_var}.{field_camel}'

    def _render_return(self) -> List[str]:
        """Render return statement."""
        if not self.scenario.return_fields:
            return ['    return true;', '}']

        lines = ['    return {']
        for key, value in self.scenario.return_fields.items():
            resolved = self._resolve_return_ref(value)
            lines.append(f'        {key}: {resolved},')
        lines.append('    };')
        lines.append('}')
        return lines

    def _map_status_value(self, status: str) -> str:
        """Map status to JavaScript string."""
        mapped = self.status_mapping.get(status)
        if mapped:
            return mapped
        return f'"{status}"'

    # ─── Helpers ──────────────────────────────────────────────────────────────

    def _dict_to_js(self, obj) -> str:
        """Convert dict to JavaScript literal representation."""
        if obj is None:
            return "null"
        elif isinstance(obj, bool):
            return str(obj).lower()
        elif isinstance(obj, str):
            if obj.startswith(REF_SENTINEL):
                return obj[len(REF_SENTINEL):]  # Strip sentinel, emit as code reference
            return json.dumps(obj)
        elif isinstance(obj, (int, float)):
            return str(obj)
        elif isinstance(obj, dict):
            items = [f'{k}: {self._dict_to_js(v)}' for k, v in obj.items()]
            return '{' + ', '.join(items) + '}'
        elif isinstance(obj, list):
            items = [self._dict_to_js(item) for item in obj]
            return '[' + ', '.join(items) + ']'
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

    def _to_camel_case(self, snake_str: str) -> str:
        """Convert snake_case to camelCase."""
        components = snake_str.split('_')
        return components[0] + ''.join(x.capitalize() for x in components[1:])
