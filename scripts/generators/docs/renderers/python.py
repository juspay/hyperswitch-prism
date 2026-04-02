"""
Python renderer for SDK examples.

Translates HydratedScenario into Python code using:
- google.protobuf.json_format.ParseDict for proto building
- raise RuntimeError for error handling
- snake_case variables
- PaymentClient(config) / RecurringPaymentClient(config) etc. for API calls
"""

import json
from typing import Any, Dict, List, Tuple

from core.models import HydratedFlow, StatusRule, StatusAction, FieldLink
from renderers.base import BaseRenderer, REF_SENTINEL

# Fields whose proto type is SecretString / CardNumberType / NetworkTokenType —
# they serialize as {value: "..."} in protobuf JSON, NOT as plain strings.
_SECRET_WRAPPED_FIELDS = frozenset({
    "card_number", "card_cvc", "card_exp_month", "card_exp_year", "card_holder_name",
    "account_number", "routing_number", "bank_account_bic", "bank_account_iban",
    "email", "phone", "name", "line1", "line2", "line3", "city", "state", "zip",
    "token", "cryptogram", "dynamic_data_value", "bank_account_holder_name",
    "payment_method_token",
})


def _wrap_secret_values(obj: Any) -> Any:
    """Recursively wrap bare string values for SecretString/CardNumberType proto fields."""
    if isinstance(obj, dict):
        result = {}
        for key, value in obj.items():
            if isinstance(value, str) and key in _SECRET_WRAPPED_FIELDS:
                result[key] = {"value": value}
            else:
                result[key] = _wrap_secret_values(value)
        return result
    elif isinstance(obj, list):
        return [_wrap_secret_values(item) for item in obj]
    return obj


# Flow → (ClientClass, method_name on that client)
_FLOW_CLIENT_MAP: Dict[str, Tuple[str, str]] = {
    "authorize":             ("PaymentClient", "authorize"),
    "capture":               ("PaymentClient", "capture"),
    "void":                  ("PaymentClient", "void"),
    "get":                   ("PaymentClient", "get"),
    "refund":                ("PaymentClient", "refund"),
    "reverse":               ("PaymentClient", "reverse"),
    "setup_recurring":       ("PaymentClient", "setup_recurring"),
    "create_order":          ("PaymentClient", "create_order"),
    "proxy_authorize":       ("PaymentClient", "proxy_authorize"),
    "proxy_setup_recurring": ("PaymentClient", "proxy_setup_recurring"),
    "token_authorize":       ("PaymentClient", "token_authorize"),
    "token_setup_recurring": ("PaymentClient", "token_setup_recurring"),
    "recurring_charge":      ("RecurringPaymentClient", "charge"),
    "tokenize":              ("PaymentMethodClient", "tokenize"),
    "create_access_token":   ("MerchantAuthenticationClient", "create_access_token"),
    "create_customer":       ("CustomerClient", "create_customer"),
    "pre_authenticate":      ("PaymentMethodAuthenticationClient", "pre_authenticate"),
    "authenticate":          ("PaymentMethodAuthenticationClient", "authenticate"),
    "post_authenticate":     ("PaymentMethodAuthenticationClient", "post_authenticate"),
}

# Client class → local variable name used in generated code
_CLIENT_VAR: Dict[str, str] = {
    "PaymentClient":                       "payment_client",
    "RecurringPaymentClient":              "recurring_client",
    "PaymentMethodClient":                 "payment_method_client",
    "MerchantAuthenticationClient":        "auth_client",
    "CustomerClient":                      "customer_client",
    "PaymentMethodAuthenticationClient":   "pma_client",
}


class PythonRenderer(BaseRenderer):
    """Render SDK examples in Python."""

    language_name = "python"

    # ─── Function signature & client setup ────────────────────────────────────

    def _render_function_signature(self) -> List[str]:
        return [
            f"async def process_{self.scenario.key}("
            f"merchant_transaction_id: str, "
            f"config: \"ConnectorConfig\" = _default_config"
            f"):",
            f'    """',
            f'    {self.scenario.description}',
            f'    """',
        ]

    def _render_client_setup(self) -> List[str]:
        """Create one client instance per unique ClientClass needed by this scenario."""
        needed: Dict[str, str] = {}  # ClientClass → var_name (ordered, deduped)
        all_flows = list(self.scenario.prerequisite_flows) + list(self.scenario.flows)
        for flow in all_flows:
            cls, _ = _FLOW_CLIENT_MAP.get(flow.name, ("PaymentClient", flow.name))
            if cls not in needed:
                needed[cls] = _CLIENT_VAR.get(cls, "payment_client")
        return [f"    {var} = {cls}(config)" for cls, var in needed.items()]

    # ─── Flow rendering ────────────────────────────────────────────────────────

    def _render_prerequisite(self, prereq_flow: HydratedFlow) -> List[str]:
        lines = [f"    # Prerequisite: {self._get_flow_description(prereq_flow.name)}"]
        client_var, method = self._get_client_and_method(prereq_flow.name)
        proto_type = self.get_proto_type(prereq_flow.name)

        if prereq_flow.payload:
            wrapped = _wrap_secret_values(prereq_flow.payload)
            payload_str = self._dict_to_python(wrapped)
            lines.append(f"    request = ParseDict({payload_str}, payment_pb2.{proto_type}())")
        else:
            lines.append(f"    request = payment_pb2.{proto_type}()")

        lines.append(
            f"    {prereq_flow.name}_response = await {client_var}.{method}(request)"
        )
        return lines

    def _render_request(self, flow: HydratedFlow) -> List[str]:
        # Wrap secret fields BEFORE substituting field links so sentinel values are preserved
        wrapped = _wrap_secret_values(flow.payload)
        payload = self._substitute_field_links(wrapped, flow.field_links)
        proto_type = self.get_proto_type(flow.name)
        if payload:
            payload_str = self._dict_to_python(payload)
            return [f"    request = ParseDict({payload_str}, payment_pb2.{proto_type}())"]
        return [f"    request = payment_pb2.{proto_type}()"]

    def _render_api_call(self, flow_name: str, response_var: str) -> List[str]:
        client_var, method = self._get_client_and_method(flow_name)
        return [f"    {response_var} = await {client_var}.{method}(request)"]

    def _render_status_rule(self, response_var: str, rule: StatusRule) -> List[str]:
        lines = []
        status_checks = [self._map_status_value(s) for s in rule.status]

        if len(status_checks) == 1:
            condition = f"{response_var}.status == {status_checks[0]}"
        else:
            statuses_str = ", ".join(status_checks)
            condition = f"{response_var}.status in [{statuses_str}]"

        lines.append(f"    if {condition}:")

        if rule.action == StatusAction.ERROR:
            message = (rule.message or "Operation failed").replace(
                "{error}", f'" + str({response_var}.error) + "'
            )
            lines.append(f'        raise RuntimeError("{message}")')

        elif rule.action == StatusAction.RETURN_EARLY:
            if rule.return_fields:
                fields = ", ".join(
                    f'"{k}": {self._resolve_return_ref_safe(v)}'
                    for k, v in rule.return_fields.items()
                )
                lines.append(f"        return {{ {fields} }}")
            else:
                lines.append(f"        return {response_var}")

        return lines

    def _render_field_link(self, link: FieldLink) -> str:
        prev_var = self._get_previous_response_var(link.from_flow)
        return f"{prev_var}.{link.from_field}"

    def _render_return(self) -> List[str]:
        if not self.scenario.return_fields:
            return ["    return True"]
        lines = ["    return {"]
        for key, value in self.scenario.return_fields.items():
            resolved = self._resolve_return_ref_safe(value)
            lines.append(f'        "{key}": {resolved},')
        lines.append("    }")
        return lines

    def _resolve_return_ref_safe(self, ref: str) -> str:
        """Like _resolve_return_ref but emits getattr(var, field, None) for proto safety."""
        last_var = self._get_last_flow_var()
        if ref.startswith("response."):
            field = ref[len("response."):]
            return f'getattr({last_var}, "{field}", None)'
        return ref

    def _map_status_value(self, status: str) -> str:
        mapped = self.status_mapping.get(status)
        if mapped:
            return mapped
        return f'"{status}"'

    # ─── Helpers ──────────────────────────────────────────────────────────────

    def _get_client_and_method(self, flow_name: str) -> Tuple[str, str]:
        """Return (client_var_name, method_name) for a given flow."""
        cls, method = _FLOW_CLIENT_MAP.get(flow_name, ("PaymentClient", flow_name))
        return _CLIENT_VAR.get(cls, "payment_client"), method

    def _dict_to_python(self, obj) -> str:
        """Convert dict to Python literal representation."""
        if obj is None:
            return "None"
        elif isinstance(obj, bool):
            return str(obj)
        elif isinstance(obj, str):
            if obj.startswith(REF_SENTINEL):
                return obj[len(REF_SENTINEL):]
            return json.dumps(obj)
        elif isinstance(obj, (int, float)):
            return str(obj)
        elif isinstance(obj, dict):
            items = [f"{json.dumps(k)}: {self._dict_to_python(v)}" for k, v in obj.items()]
            return "{" + ", ".join(items) + "}"
        elif isinstance(obj, list):
            items = [self._dict_to_python(item) for item in obj]
            return "[" + ", ".join(items) + "]"
        else:
            return json.dumps(obj)

    def _get_flow_description(self, flow_name: str) -> str:
        descriptions = {
            "create_access_token": "Obtain OAuth2 access token",
            "create_customer": "Create customer profile",
            "create_order": "Create order before payment",
        }
        return descriptions.get(flow_name, flow_name.replace("_", " ").title())
