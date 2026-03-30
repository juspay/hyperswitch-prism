"""Kotlin language renderer — generates protobuf builder pattern code."""

from __future__ import annotations

from .base import BaseRenderer
from ._shared import (
    _SchemaDB, _client_class, _FLOW_KEY_TO_METHOD, _FLOW_KEY_TO_GRPC_REQUEST,
    _STEP_DESCRIPTIONS, _FLOW_BUILDER_EXTRA_PARAM,
    _PROTO_FIELD_TYPES, _PROTO_TYPE_SOURCE, _PROTO_WRAPPER_TYPES, _PROTO_NESTED_IN,
    _KOTLIN_PRIMITIVES,
)


_DEFAULT_PACKAGE = "payments"

# Kotlin-specific override: some services map to different client class names
_KT_CLIENT_CLASS: dict[str, str] = {
    "PaymentService": "DirectPaymentClient",
}

# Types exported via `payments.*` typealiases in the Kotlin SDK.
# Types NOT in this set must be imported from their full Kotlin package.
_KT_PAYMENTS_ALIASES: frozenset[str] = frozenset({
    # Request/Response types (Payments.kt)
    "PaymentServiceAuthorizeRequest", "PaymentServiceAuthorizeResponse",
    "PaymentServiceCaptureRequest", "PaymentServiceCaptureResponse",
    "PaymentServiceVoidRequest", "PaymentServiceVoidResponse",
    "PaymentServiceRefundRequest", "RefundResponse",
    "PaymentServiceReverseRequest", "PaymentServiceReverseResponse",
    "PaymentServiceGetRequest", "PaymentServiceGetResponse",
    "PaymentServiceCreateOrderRequest", "PaymentServiceCreateOrderResponse",
    "PaymentServiceSetupRecurringRequest", "PaymentServiceSetupRecurringResponse",
    "PaymentServiceIncrementalAuthorizationRequest", "PaymentServiceIncrementalAuthorizationResponse",
    "PaymentServiceVerifyRedirectResponseRequest", "PaymentServiceVerifyRedirectResponseResponse",
    "PaymentServiceDisputeRequest", "DisputeResponse",
    "DisputeServiceAcceptRequest", "DisputeServiceAcceptResponse",
    "DisputeServiceDefendRequest", "DisputeServiceDefendResponse",
    "DisputeServiceSubmitEvidenceRequest", "DisputeServiceSubmitEvidenceResponse",
    "MerchantAuthenticationServiceCreateAccessTokenRequest", "MerchantAuthenticationServiceCreateAccessTokenResponse",
    "MerchantAuthenticationServiceCreateSessionTokenRequest", "MerchantAuthenticationServiceCreateSessionTokenResponse",
    "MerchantAuthenticationServiceCreateSdkSessionTokenRequest", "MerchantAuthenticationServiceCreateSdkSessionTokenResponse",
    "PaymentMethodAuthenticationServicePreAuthenticateRequest", "PaymentMethodAuthenticationServicePreAuthenticateResponse",
    "PaymentMethodAuthenticationServiceAuthenticateRequest", "PaymentMethodAuthenticationServiceAuthenticateResponse",
    "PaymentMethodAuthenticationServicePostAuthenticateRequest", "PaymentMethodAuthenticationServicePostAuthenticateResponse",
    "PaymentMethodServiceTokenizeRequest", "PaymentMethodServiceTokenizeResponse",
    "RecurringPaymentServiceChargeRequest", "RecurringPaymentServiceChargeResponse",
    "CustomerServiceCreateRequest", "CustomerServiceCreateResponse",
    # Common message types (Payments.kt)
    "Money", "ErrorInfo", "Customer", "PaymentAddress", "Address", "Identifier",
    "ConnectorState", "AccessToken", "BrowserInformation", "CustomerAcceptance",
    "SessionToken", "ConnectorResponseData", "CardConnectorResponse",
    "AuthenticationData", "Metadata", "ConnectorSpecificConfig",
    # Enum types (Payments.kt)
    "Currency", "CaptureMethod", "AuthenticationType", "PaymentMethodType",
    "PaymentStatus", "RefundStatus", "DisputeStatus", "MandateStatus",
    "AuthorizationStatus", "OperationStatus", "HttpMethod", "FutureUsage",
    "PaymentExperience", "PaymentChannel", "Connector", "ProductType",
    "DisputeStage", "Tokenization", "WebhookEventType", "ThreeDsCompletionIndicator",
    "TransactionStatus", "ExemptionIndicator", "MitCategory", "SyncRequestType",
    "AcceptanceType", "CavvAlgorithm",
    # PaymentMethods types (PaymentMethods.kt)
    "PaymentMethod", "CardDetails", "CardNumberType", "NetworkTokenType",
    "CardRedirect", "CardNetwork", "TokenPaymentMethodType", "CountryAlpha2",
    "SecretString",
})

# Maps Python module paths (from _PROTO_TYPE_SOURCE) to Kotlin full package names
# for types that are NOT in the `payments.*` typealias set.
_KT_FALLBACK_PACKAGE: dict[str, str] = {
    "payments.generated.payment_pb2": "types.Payment",
    "payments.generated.payment_methods_pb2": "types.PaymentMethods",
    "payments.generated.payouts_pb2": "types.Payouts",
    "payments.generated.sdk_config_pb2": "types.SdkConfig",
}


def _kt_str(val: object) -> str:
    """Return a Kotlin double-quoted string literal for val."""
    s = str(val)
    escaped = s.replace("\\", "\\\\").replace('"', '\\"')
    return f'"{escaped}"'


class Renderer(BaseRenderer):
    """Kotlin SDK snippet renderer."""

    lang = "kotlin"
    extension = ".kt"

    def config_snippet(self, connector_name: str) -> str:
        return '''import payments.DirectPaymentClient
import payments.ConnectorConfig
import payments.Environment

val config = ConnectorConfig.newBuilder()
    .setEnvironment(Environment.SANDBOX)
    .build()
val client = DirectPaymentClient(config)'''

    def render_consolidated(self, connector_name, scenarios_with_payloads,
                           flow_metadata, message_schemas, flow_items=None):
        """Generate Kotlin file with all scenarios."""
        db = _SchemaDB(message_schemas)

        # Collect all imports needed
        all_imports: set[str] = set()
        all_imports.add("payments.ConnectorConfig")
        all_imports.add("payments.Environment")

        # Generate scenario functions
        functions = []
        for scenario, flow_payloads in scenarios_with_payloads:
            func = self._gen_scenario_func(scenario, flow_payloads, flow_metadata, db, all_imports)
            functions.append(func)

        # Generate flow functions if any
        if flow_items:
            for flow_key, proto_req, pm_label in flow_items:
                func = self._gen_flow_func(flow_key, proto_req, flow_metadata, db, pm_label, all_imports)
                functions.append(func)

        # Build imports
        import_lines = sorted(all_imports)

        return f'''// Auto-generated for {connector_name}
package examples.{connector_name}

{chr(10).join(f"import {imp}" for imp in import_lines)}

{chr(10).join(functions)}'''

    def _gen_scenario_func(self, scenario, flow_payloads, flow_metadata, db, all_imports: set[str]) -> str:
        """Generate a single scenario function with protobuf builders."""
        func_name = f"process{''.join(w.title() for w in scenario.key.split('_'))}"

        lines = [f"fun {func_name}(txnId: String, config: ConnectorConfig): Map<String, Any?> {{"]
        lines.append(f'    // {scenario.title}')

        # Determine which client classes are needed
        flow_client_var: dict[str, tuple[str, str]] = {}
        seen_classes: dict[str, str] = {}
        for flow_key in scenario.flows:
            svc = flow_metadata.get(flow_key, {}).get("service_name", "PaymentService")
            cls = _KT_CLIENT_CLASS.get(svc) or _client_class(svc)
            var = cls[0].lower() + cls[1:]
            flow_client_var[flow_key] = (cls, var)
            seen_classes[var] = cls
            all_imports.add(f"payments.{cls}")

        for var, cls in seen_classes.items():
            lines.append(f"    val {var} = {cls}(config)")
        lines.append("")

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

            lines.append(f"    // Step {step_num}: {desc}")

            result_var = f"result{step_num}" if len(scenario.flows) > 1 else "result"
            if grpc_req:
                step_imports: set[str] = set()
                expr = self._build_kt_builder(payload, grpc_req, db, step_imports, indent=1)
                all_imports.update(step_imports)
                lines.append(f"    val {result_var} = {var}.{method}({expr})")
            else:
                lines.append(f"    val {result_var} = {var}.{method}()")

            lines.append(f'    println("[{flow_key}] HTTP ${{{result_var}.statusCode}}")')
            lines.append("")

        last_var = (f"result{len(scenario.flows)}" if len(scenario.flows) > 1 else "result")
        lines.append(f"    return mapOf(\"statusCode\" to {last_var}.statusCode)")
        lines.append("}")
        return "\n".join(lines)

    def _gen_flow_func(self, flow_key, proto_req, flow_metadata, db, pm_label, all_imports: set[str]) -> str:
        """Generate single flow function."""
        meta = flow_metadata.get(flow_key, {})
        svc = meta.get("service_name", "PaymentService")
        grpc_req = (meta.get("grpc_request", "")
                    or _FLOW_KEY_TO_GRPC_REQUEST.get(flow_key, ""))

        cls = _KT_CLIENT_CLASS.get(svc) or _client_class(svc)
        var = cls[0].lower() + cls[1:]
        method = _FLOW_KEY_TO_METHOD.get(flow_key, flow_key)
        func_name = flow_key

        all_imports.add(f"payments.{cls}")

        lines = [f"fun {func_name}(txnId: String, config: ConnectorConfig): Map<String, Any?> {{"]

        if pm_label:
            lines.append(f'    // Flow: {svc}.{flow_key} ({pm_label})')
        else:
            lines.append(f'    // Flow: {svc}.{flow_key}')

        lines.append(f"    val {var} = {cls}(config)")
        lines.append("")

        if proto_req and grpc_req:
            step_imports: set[str] = set()
            expr = self._build_kt_builder(proto_req, grpc_req, db, step_imports, indent=1)
            all_imports.update(step_imports)
            lines.append(f"    val result = {var}.{method}({expr})")
        else:
            lines.append(f"    val result = {var}.{method}()")

        lines.append(f'    println("[{flow_key}] HTTP ${{result.statusCode}}")')
        lines.append("    return mapOf(\"statusCode\" to result.statusCode)")
        lines.append("}")
        return "\n".join(lines)

    def _kt_import_for(self, type_name: str) -> str:
        """Return the correct fully-qualified import path for a Kotlin proto type."""
        if type_name in _KT_PAYMENTS_ALIASES:
            return f"payments.{type_name}"
        # Fall back to the full Kotlin package based on which proto file the type comes from
        src = _PROTO_TYPE_SOURCE.get(type_name, "")
        pkg = _KT_FALLBACK_PACKAGE.get(src, "payments")
        return f"{pkg}.{type_name}"

    def _build_kt_builder(
        self,
        val: object,
        type_name: str,
        db: _SchemaDB,
        imports: set[str],
        indent: int = 1,
    ) -> str:
        """
        Recursively build a Kotlin protobuf builder expression.

        Returns a Kotlin expression string.
        """
        pad = "    " * indent

        # ── Primitives ─────────────────────────────────────────────────────────
        if type_name in _KOTLIN_PRIMITIVES or not type_name:
            if isinstance(val, bool):
                return "true" if val else "false"
            if isinstance(val, (int, float)):
                return str(val)
            return _kt_str(val)

        # ── Scalar wrapper (e.g. SecretString with single `value` field) ───────
        if type_name in _PROTO_WRAPPER_TYPES:
            imports.add(self._kt_import_for(type_name))
            inner_val = val if not isinstance(val, dict) else (
                next(iter(val.values())) if val else ""
            )
            return f"{type_name}.newBuilder().setValue({_kt_str(inner_val)}).build()"

        # ── Nested message (e.g. GoogleWallet.PaymentMethodInfo) ───────────────
        if type_name in _PROTO_NESTED_IN:
            parent_name = _PROTO_NESTED_IN[type_name]
            imports.add(self._kt_import_for(parent_name))
            qualified = f"{parent_name}.{type_name}"
            known_fields = _PROTO_FIELD_TYPES.get(type_name, {})
            if not isinstance(val, dict) or not val:
                return f"{qualified}.newBuilder().build()"

            builder_parts = [f"{qualified}.newBuilder()"]
            for k, v in val.items():
                ftype = known_fields.get(k, "")
                setter = f"set{self._to_pascal(k)}"
                fexpr = self._build_kt_builder(v, ftype, db, imports, indent + 1)
                builder_parts.append(f".{setter}({fexpr})")
            builder_parts.append(".build()")
            return "".join(builder_parts)

        # ── Dict value → message builder ────────────────────────────────────────
        if isinstance(val, dict):
            known_fields = _PROTO_FIELD_TYPES.get(type_name, {})

            # Detect oneof-group wrapper
            if (len(val) == 1
                    and known_fields
                    and type_name not in _PROTO_WRAPPER_TYPES):
                outer_key = next(iter(val))
                inner_val = val[outer_key]
                if (outer_key not in known_fields
                        and isinstance(inner_val, dict)
                        and any(k in known_fields for k in inner_val)):
                    val = inner_val

            imports.add(self._kt_import_for(type_name))

            if not val:
                return f"{type_name}.newBuilder().build()"

            builder_parts = [f"{type_name}.newBuilder()"]
            for k, v in val.items():
                ftype = known_fields.get(k, "")
                setter = f"set{self._to_pascal(k)}"
                fexpr = self._build_kt_builder(v, ftype, db, imports, indent + 1)
                builder_parts.append(f".{setter}({fexpr})")
            builder_parts.append(".build()")
            return "".join(builder_parts)

        # ── Enum value (string like 'AUTOMATIC') ───────────────────────────────
        # In Kotlin protobuf, enum values are accessed as EnumType.VALUE_NAME (no .Value. wrapper)
        if isinstance(val, str) and type_name:
            imports.add(self._kt_import_for(type_name))
            return f"{type_name}.{val}"

        # ── Fallback ───────────────────────────────────────────────────────────
        return _kt_str(val) if isinstance(val, str) else repr(val)

    def _to_pascal(self, snake: str) -> str:
        """Convert snake_case to PascalCase."""
        return "".join(w.title() for w in snake.split("_"))
