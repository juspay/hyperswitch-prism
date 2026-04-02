"""
Kotlin renderer for SDK examples.

Translates HydratedScenario into Kotlin code using:
- Builder pattern (.newBuilder().apply { })
- throw RuntimeException() for error handling
- camelCase variables
"""

import json
from typing import List

from core.models import HydratedFlow, StatusRule, StatusAction, FieldLink
from renderers.base import BaseRenderer, REF_SENTINEL


class KotlinRenderer(BaseRenderer):
    """Render SDK examples in Kotlin."""

    language_name = "kotlin"

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
        """Render suspend function signature."""
        func_name = "process" + "".join(p.capitalize() for p in self.scenario.key.split("_"))
        return [
            f"suspend fun {func_name}() {{",
            f"    // {self.scenario.description}",
        ]

    def _render_prerequisite(self, prereq_flow: HydratedFlow) -> List[str]:
        """Render prerequisite flow with comment."""
        lines = [
            f"    // Prerequisite: {self._get_flow_description(prereq_flow.name)}",
        ]

        if prereq_flow.payload:
            lines.extend(self._render_request_payload(prereq_flow.payload, "request", prereq_flow.name))
        else:
            proto_type = self._get_proto_type(prereq_flow.name)
            lines.append(f"    val request = {proto_type}.newBuilder().build()")

        camel_var = self._to_camel_case(f"{prereq_flow.name}_response")
        lines.append(f"    val {camel_var} = client.{prereq_flow.name}(request)")

        return lines

    def _render_request(self, flow: HydratedFlow) -> List[str]:
        """Render request payload with field links substituted."""
        payload = self._substitute_field_links(flow.payload, flow.field_links)

        if payload:
            return self._render_request_payload(payload, "request", flow.name)
        else:
            proto_type = self._get_proto_type(flow.name)
            return [f"    val request = {proto_type}.newBuilder().build()"]

    def _render_api_call(self, flow_name: str, response_var: str) -> List[str]:
        """Render API call."""
        return [f"    val {response_var} = client.{flow_name}(request)"]

    def _render_status_rule(self, response_var: str, rule: StatusRule) -> List[str]:
        """
        Render status handling.

        Kotlin uses when expression:
            when (response.status) {
                "FAILED", "AUTHORIZATION_FAILED" -> throw RuntimeException("Payment failed: $error")
                else -> {}
            }
        """
        lines = []

        status_checks = [self._map_status_value(s) for s in rule.status]
        statuses_str = ', '.join(status_checks)

        lines.append(f"    when ({response_var}.status) {{")

        if rule.action == StatusAction.ERROR:
            message = rule.message or "Operation failed"
            message = message.replace('{error}', '$error')
            lines.append(f'        {statuses_str} -> throw RuntimeException("{message}")')

        elif rule.action == StatusAction.RETURN_EARLY:
            if rule.return_fields:
                fields = ', '.join(
                    f'"{k}" to {self._resolve_return_ref(v)}'
                    for k, v in rule.return_fields.items()
                )
                lines.append(f'        {statuses_str} -> return mapOf({fields})')
            else:
                lines.append(f'        {statuses_str} -> return {response_var}')

        elif rule.action == StatusAction.CONTINUE:
            pass  # No-op: handled in base render() loop

        lines.append("        else -> {}")
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
            return ['    return true', '}']

        lines = ['    return mapOf(']
        for key, value in self.scenario.return_fields.items():
            resolved = self._resolve_return_ref(value)
            lines.append(f'        "{key}" to {resolved},')
        lines.append('    )')
        lines.append('}')
        return lines

    def _map_status_value(self, status: str) -> str:
        """Map status to Kotlin string."""
        mapped = self.status_mapping.get(status)
        if mapped:
            return mapped
        return f'"{status}"'

    # ─── Helpers ──────────────────────────────────────────────────────────────

    def _render_request_payload(self, payload: dict, var_name: str, flow_name: str) -> List[str]:
        """Render request payload using Kotlin builder pattern."""
        proto_type = self.get_proto_type(flow_name)
        lines = [f"    val {var_name} = {proto_type}.newBuilder().apply {{"]

        for key, value in payload.items():
            kotlin_value = self._value_to_kotlin(value)
            setter = self._to_camel_case(key)
            lines.append(f"        {setter} = {kotlin_value}")

        lines.append("    }.build()")
        return lines

    def _value_to_kotlin(self, obj) -> str:
        """Convert value to Kotlin representation."""
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
            # For nested objects, use builder pattern placeholder
            first_key = next(iter(obj.keys())) if obj else "unknown"
            return f"{self._to_camel_case(first_key)}.newBuilder().build()"
        elif isinstance(obj, list):
            items = [self._value_to_kotlin(item) for item in obj]
            return f"listOf({', '.join(items)})"
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
        if len(components) == 1:
            return components[0]
        return components[0] + ''.join(x.capitalize() for x in components[1:])

    def _to_title_case(self, snake_str: str) -> str:
        """Convert snake_case to TitleCase (PascalCase)."""
        return ''.join(x.capitalize() for x in snake_str.split('_'))
