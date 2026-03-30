"""Python language renderer — generates directly-typed proto constructor calls."""

from __future__ import annotations

from .base import BaseRenderer
from ._shared import (
    _SchemaDB, _client_class, _FLOW_KEY_TO_METHOD, _FLOW_KEY_TO_GRPC_REQUEST,
    _STEP_DESCRIPTIONS, _FLOW_BUILDER_EXTRA_PARAM,
    _PROTO_FIELD_TYPES, _PROTO_TYPE_SOURCE, _PROTO_WRAPPER_TYPES, _PROTO_NESTED_IN,
    _KOTLIN_PRIMITIVES,
)

# Re-use Kotlin's primitive set — same proto scalar types apply to Python
_PY_PRIMITIVES = _KOTLIN_PRIMITIVES

_DEFAULT_MODULE = "payments.generated.payment_pb2"


class Renderer(BaseRenderer):
    """Python SDK snippet renderer."""

    lang = "python"
    extension = ".py"

    def config_snippet(self, connector_name: str) -> str:
        return '''from payments import PaymentClient
from payments.generated import sdk_config_pb2

config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
)
client = PaymentClient(config)'''

    def render_consolidated(self, connector_name, scenarios_with_payloads,
                            flow_metadata, message_schemas, flow_items=None):
        """Generate Python file with all scenarios."""
        db = _SchemaDB(message_schemas)

        # Collect client class imports
        services: set[str] = set()
        for scenario, _ in scenarios_with_payloads:
            for fk in scenario.flows:
                svc = flow_metadata.get(fk, {}).get("service_name", "PaymentService")
                services.add(svc)

        client_imports = "\n".join(
            f"from payments import {_client_class(svc)}" for svc in sorted(services)
        )

        # Generate functions and collect all proto type imports
        # imports: module_path → set of type names
        all_imports: dict[str, set[str]] = {}
        functions = []
        for scenario, flow_payloads in scenarios_with_payloads:
            func = self._gen_scenario_func(
                scenario, flow_payloads, flow_metadata, db, all_imports
            )
            functions.append(func)

        # Build proto import block (grouped by module)
        proto_import_lines = []
        for module in sorted(all_imports):
            names = sorted(all_imports[module])
            if len(names) == 1:
                proto_import_lines.append(f"from {module} import {names[0]}")
            else:
                proto_import_lines.append(f"from {module} import (")
                for n in names:
                    proto_import_lines.append(f"    {n},")
                proto_import_lines.append(")")
        proto_imports = "\n".join(proto_import_lines)

        extras = f"\n{proto_imports}" if proto_imports else ""
        return (
            f"# Auto-generated for {connector_name}\n"
            f"import asyncio\n"
            f"{client_imports}{extras}\n"
            f"\n"
            f"{chr(10).join(functions)}\n"
            f"\n"
            f'if __name__ == "__main__":\n'
            f'    asyncio.run(process_checkout_card("order_001"))'
        )

    # Maps client class name → local variable name used in generated code
    _CLIENT_VAR = {
        "PaymentClient": "client",
        "RecurringPaymentClient": "recurring_client",
        "CustomerClient": "customer_client",
        "PaymentMethodClient": "pm_client",
    }

    def _gen_scenario_func(self, scenario, flow_payloads, flow_metadata, db,
                           all_imports: dict[str, set[str]]) -> str:
        """Generate a single scenario function with typed proto constructors."""
        lines = [f"async def process_{scenario.key}(merchant_id, config):"]
        lines.append(f'    """{scenario.title}"""')

        # Determine which client classes are needed
        flow_client_var: dict[str, tuple[str, str]] = {}
        seen_classes: dict[str, str] = {}
        for flow_key in scenario.flows:
            svc = flow_metadata.get(flow_key, {}).get("service_name", "PaymentService")
            cls = _client_class(svc)
            var = self._CLIENT_VAR.get(cls, "client")
            flow_client_var[flow_key] = (cls, var)
            seen_classes[var] = cls

        for var, cls in seen_classes.items():
            lines.append(f"    {var} = {cls}(config)")

        for step_num, flow_key in enumerate(scenario.flows, 1):
            desc = _STEP_DESCRIPTIONS.get(flow_key, flow_key)
            _, var = flow_client_var[flow_key]
            method = _FLOW_KEY_TO_METHOD.get(flow_key, flow_key)
            grpc_req = (flow_metadata.get(flow_key, {}).get("grpc_request", "")
                        or _FLOW_KEY_TO_GRPC_REQUEST.get(flow_key, ""))
            payload = dict(flow_payloads.get(flow_key, {}))

            # Adjust capture_method based on scenario
            if flow_key == "authorize":
                if scenario.key in ("checkout_card", "void_payment", "get_payment"):
                    payload["capture_method"] = "MANUAL"
                elif scenario.key == "refund":
                    payload["capture_method"] = "AUTOMATIC"
                else:
                    payload["capture_method"] = "AUTOMATIC"

            lines.append(f"    # Step {step_num}: {desc}")

            if payload and grpc_req:
                step_imports: dict[str, set[str]] = {}
                expr = self._build_py_constructor(payload, grpc_req, step_imports, indent=2)
                # Merge step imports into all_imports
                for mod, names in step_imports.items():
                    all_imports.setdefault(mod, set()).update(names)
                lines.append(f"    result = await {var}.{method}({expr})")
            elif grpc_req:
                mod = _PROTO_TYPE_SOURCE.get(grpc_req, _DEFAULT_MODULE)
                all_imports.setdefault(mod, set()).add(grpc_req)
                lines.append(f"    result = await {var}.{method}({grpc_req}())")
            else:
                lines.append(f"    result = await {var}.{method}()")

        lines.append("    return result")
        return "\n".join(lines)

    def _build_py_constructor(
        self,
        val: object,
        type_name: str,
        imports: dict[str, set[str]],
        indent: int = 1,
    ) -> str:
        """
        Recursively build a Python proto constructor expression for `val` of `type_name`.

        Side-effect: adds needed (module, TypeName) pairs to `imports`.
        Returns a Python expression string.
        """
        pad = "    " * indent

        # ── Primitives ─────────────────────────────────────────────────────────
        if type_name in _PY_PRIMITIVES or not type_name:
            if isinstance(val, bool):
                return "True" if val else "False"
            if isinstance(val, (int, float)):
                return repr(val)
            return repr(str(val))

        # ── Scalar wrapper (e.g. SecretString with single `value` field) ───────
        if type_name in _PROTO_WRAPPER_TYPES:
            mod = _PROTO_TYPE_SOURCE.get(type_name, _DEFAULT_MODULE)
            imports.setdefault(mod, set()).add(type_name)
            inner = repr(val) if not isinstance(val, dict) else repr(
                next(iter(val.values())) if val else ""
            )
            return f"{type_name}(value={inner})"

        # ── Nested message (e.g. GoogleWallet.PaymentMethodInfo) ───────────────
        # Python protobuf exposes nested types as Parent.NestedType, not as
        # top-level module symbols, so we import the parent and qualify the name.
        if type_name in _PROTO_NESTED_IN:
            parent_name = _PROTO_NESTED_IN[type_name]
            mod = _PROTO_TYPE_SOURCE.get(parent_name, _DEFAULT_MODULE)
            imports.setdefault(mod, set()).add(parent_name)
            qualified = f"{parent_name}.{type_name}"
            known_fields = _PROTO_FIELD_TYPES.get(type_name, {})
            if not isinstance(val, dict) or not val:
                return f"{qualified}()"
            field_parts: list[str] = []
            for k, v in val.items():
                ftype = known_fields.get(k, "")
                fexpr = self._build_py_constructor(v, ftype, imports, indent + 1)
                field_parts.append(f"{pad}    {k}={fexpr},")
            inner_str = "\n".join(field_parts)
            return f"{qualified}(\n{inner_str}\n{pad})"

        # ── Dict value → message constructor ───────────────────────────────────
        if isinstance(val, dict):
            known_fields = _PROTO_FIELD_TYPES.get(type_name, {})

            # Detect oneof-group wrapper: single unknown key wrapping variants
            # e.g. MandateReference probe data: {'mandate_id_type': {'connector_mandate_id': {...}}}
            if (len(val) == 1
                    and known_fields
                    and type_name not in _PROTO_WRAPPER_TYPES):
                outer_key = next(iter(val))
                inner_val = val[outer_key]
                if (outer_key not in known_fields
                        and isinstance(inner_val, dict)
                        and any(k in known_fields for k in inner_val)):
                    val = inner_val  # strip the oneof-group-name wrapper

            # Now build: TypeName(field=expr, ...)
            mod = _PROTO_TYPE_SOURCE.get(type_name, _DEFAULT_MODULE)
            imports.setdefault(mod, set()).add(type_name)

            if not val:
                return f"{type_name}()"

            field_parts: list[str] = []
            for k, v in val.items():
                ftype = known_fields.get(k, "")
                fexpr = self._build_py_constructor(v, ftype, imports, indent + 1)
                field_parts.append(f"{pad}    {k}={fexpr},")

            inner_str = "\n".join(field_parts)
            return f"{type_name}(\n{inner_str}\n{pad})"

        # ── Enum value (string like 'AUTOMATIC') ───────────────────────────────
        if isinstance(val, str) and type_name:
            mod = _PROTO_TYPE_SOURCE.get(type_name, _DEFAULT_MODULE)
            imports.setdefault(mod, set()).add(type_name)
            return f"{type_name}.Value({repr(val)})"

        # ── Fallback for int/float enum values ─────────────────────────────────
        return repr(val)
