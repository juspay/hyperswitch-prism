"""
Base renderer for all languages.

The shared render() loop produces identical behavior across languages.
The language-specific methods (_render_status_rule, _render_field_link, etc.)
handle syntax translation only.
"""

import copy
from abc import ABC, abstractmethod
from typing import Dict, List

from core.models import HydratedScenario, HydratedFlow, StatusRule, StatusAction, FieldLink

# Sentinel prefix for field-link substitutions.
# Values carrying this prefix are variable references, NOT string literals.
REF_SENTINEL = "__REF__:"

# Module-level registry: flow_key → grpc_request class name.
# Populated by configure_from_manifest() at application startup.
# Renderers call get_proto_type() which reads from here — zero hardcoding.
_FLOW_PROTO_TYPES: Dict[str, str] = {}


def configure_from_manifest(flow_metadata: list) -> None:
    """Populate the proto type registry from manifest.json flow_metadata.

    Call this once at startup after loading the manifest.  Every entry in
    flow_metadata that has a 'grpc_request' field is registered.

    Example manifest entry:
        {"flow_key": "authorize", "grpc_request": "PaymentServiceAuthorizeRequest", ...}
    """
    _FLOW_PROTO_TYPES.clear()
    for entry in flow_metadata:
        flow_key = entry.get("flow_key")
        grpc_request = entry.get("grpc_request")
        if flow_key and grpc_request:
            _FLOW_PROTO_TYPES[flow_key] = grpc_request


def _fallback_proto_type(flow_name: str) -> str:
    """Derive a proto request class name when the manifest has no entry.

    Follows the dominant naming pattern: ServiceNameFlowNameRequest.
    This is a last resort — the manifest should cover all real flows.
    """
    return "PaymentService" + "".join(p.capitalize() for p in flow_name.split("_")) + "Request"


class BaseRenderer(ABC):
    """
    Base class for all language renderers.

    Each language implements the abstract methods to translate the shared
    behavioral specification (HydratedScenario) into language-specific syntax.

    The render() loop is shared — all languages must implement the same
    StatusRule actions and cross-flow references, just with different syntax.
    """

    language_name: str = "base"  # Subclasses override

    def get_proto_type(self, flow_name: str) -> str:
        """Return the proto request class name for a flow.

        Populated at startup from manifest.json via configure_from_manifest().
        Falls back to a predictable pattern so renderers always produce something.
        """
        return _FLOW_PROTO_TYPES.get(flow_name, _fallback_proto_type(flow_name))

    def __init__(self, scenario: HydratedScenario, status_mapping: dict):
        """
        Initialize renderer with hydrated scenario and status mapping.

        Args:
            scenario: The hydrated scenario to render
            status_mapping: Language-specific status enum mappings
        """
        self.scenario = scenario
        self.status_mapping = status_mapping
        self.flow_vars: Dict[str, str] = {}  # flow_name -> response_variable

    def render(self) -> str:
        """
        Render the complete scenario as executable code.

        The render loop is IDENTICAL across all languages:
        1. Render prerequisite flows (auto-discovered from flow_graph)
        2. Render each flow with:
           - Request payload with field links resolved
           - API call
           - Status handling (from StatusRule)
           - Return early if configured
        3. Return final values (from scenario.return_fields)

        Only the syntax differs.
        """
        lines = []

        # Function signature
        lines.extend(self._render_function_signature())

        # Client/context setup (language-specific: creates SDK client from config)
        lines.extend(self._render_client_setup())

        # Prerequisite flows (from flow_graph auto-discovery)
        for prereq in self.scenario.prerequisite_flows:
            lines.extend(self._render_prerequisite(prereq))
            lines.append("")

        # Main flows
        for flow in self.scenario.flows:
            response_var = self._assign_flow_variable(flow.name)

            # Build request with field links substituted
            lines.extend(self._render_request(flow))

            # Make API call
            lines.extend(self._render_api_call(flow.name, response_var))

            # Status handling — CONTINUE rules are no-ops, skip them
            for rule in flow.status_handling:
                if rule.action == StatusAction.CONTINUE:
                    continue  # No-op: continue means "this is expected, proceed"
                lines.extend(self._render_status_rule(response_var, rule))

            lines.append("")

        # Return final values
        lines.extend(self._render_return())

        return "\n".join(lines)

    def _assign_flow_variable(self, flow_name: str) -> str:
        """Assign a snake_case variable name for this flow's response and cache it."""
        var_name = f"{flow_name}_response"
        self.flow_vars[flow_name] = var_name
        return var_name

    def _get_previous_response_var(self, flow_name: str) -> str:
        """Get the variable name for a previous flow's response."""
        return self.flow_vars.get(flow_name, f"{flow_name}_response")

    def _get_last_flow_var(self) -> str:
        """Return the variable name of the last flow that was assigned."""
        if self.flow_vars:
            return list(self.flow_vars.values())[-1]
        return "response"

    def _resolve_return_ref(self, ref: str) -> str:
        """
        Replace 'response.X' with last_flow_var.X.

        Subclasses override to apply camelCase (JS/Kotlin) or JSON indexing (Rust).
        """
        last_var = self._get_last_flow_var()
        if ref.startswith("response."):
            field = ref[len("response."):]
            return f"{last_var}.{field}"
        return ref

    @staticmethod
    def _set_nested(obj: dict, path: str, value) -> None:
        """Set a value at a dot-separated path in a nested dict."""
        parts = path.split(".")
        for part in parts[:-1]:
            if part not in obj or not isinstance(obj[part], dict):
                obj[part] = {}
            obj = obj[part]
        obj[parts[-1]] = value

    def _substitute_field_links(self, payload: dict, field_links: dict) -> dict:
        """
        Replace payload values at target_path with REF_SENTINEL-tagged references.

        Handles nested paths like 'state.access_token.token.value'.
        The sentinel is stripped by each language's dict serializer.
        """
        result = copy.deepcopy(payload)
        for target_path, link in field_links.items():
            ref = REF_SENTINEL + self._render_field_link(link)
            self._set_nested(result, target_path, ref)
        return result

    def get_manifest(self) -> "RenderManifest":
        """Return a RenderManifest for structural parity checking."""
        from core.models import RenderManifest
        status_checks = {
            flow.name: len([r for r in flow.status_handling if r.action != StatusAction.CONTINUE])
            for flow in self.scenario.flows
        }
        cross_flow_refs = [
            f"{f.name}.{field}"
            for f in self.scenario.flows
            for field in f.use_from_previous
        ]
        return RenderManifest(
            scenario_key=self.scenario.key,
            language=self.language_name,
            flow_count=len(self.scenario.flows),
            status_checks=status_checks,
            return_field_count=len(self.scenario.return_fields),
            cross_flow_refs=cross_flow_refs,
            prerequisite_count=len(self.scenario.prerequisite_flows),
        )

    # ─── Abstract Methods: Language-Specific Syntax ─────────────────────────────

    def _render_client_setup(self) -> List[str]:
        """Render SDK client creation lines after the function signature.

        Default: no-op (e.g. Rust passes client as a parameter).
        Override in Python/JS/Kotlin where the function creates its own client.
        """
        return []

    @abstractmethod
    def _render_function_signature(self) -> List[str]:
        """Render the function/method signature."""
        pass

    @abstractmethod
    def _render_prerequisite(self, prereq_flow: HydratedFlow) -> List[str]:
        """Render a prerequisite flow with explanatory comment."""
        pass

    @abstractmethod
    def _render_request(self, flow: HydratedFlow) -> List[str]:
        """
        Render the request payload construction.

        Must substitute field_links with references to previous responses.
        """
        pass

    @abstractmethod
    def _render_api_call(self, flow_name: str, response_var: str) -> List[str]:
        """Render the API call."""
        pass

    @abstractmethod
    def _render_status_rule(self, response_var: str, rule: StatusRule) -> List[str]:
        """
        Render status handling for a StatusRule.

        ALL languages must implement the SAME logic:
        - Check if response.status is in rule.status list
        - If action == ERROR: raise/throw exception with rule.message
        - If action == RETURN_EARLY: return specified fields

        Only syntax differs.
        """
        pass

    @abstractmethod
    def _render_field_link(self, link: FieldLink) -> str:
        """
        Render a reference to a field from a previous response.

        Example outputs:
        - Python: authorize_response.connector_transaction_id
        - JavaScript: authorizeResponse.connectorTransactionId
        - Kotlin: authorizeResponse.connectorTransactionId
        - Rust: authorize_response["connector_transaction_id"]
        """
        pass

    @abstractmethod
    def _render_return(self) -> List[str]:
        """Render the return statement with return_fields."""
        pass

    @abstractmethod
    def _map_status_value(self, status: str) -> str:
        """
        Map abstract status value to language-specific enum/constant.

        Example: "FAILED" ->
        - Python: '"FAILED"'
        - Rust: '"FAILED"' (string literal)
        """
        pass
