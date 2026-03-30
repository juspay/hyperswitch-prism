"""TypeScript language renderer (replaces the old JavaScript renderer).

Generates .ts example files using ES module imports and proto-derived
type annotations instead of CommonJS require / untyped plain JS.
"""

from __future__ import annotations

from ._shared import (
    JS_RESERVED,
    _CARD_AUTHORIZE_SCENARIOS,
    _CONNECTOR_CONFIG_FIELDS,
    _DYNAMIC_FIELDS_JS,
    _FLOW_BUILDER_EXTRA_PARAM,
    _FLOW_KEY_TO_METHOD,
    _FLOW_VAR_NAME,
    _SCENARIO_DROP_FIELDS,
    _STEP_DESCRIPTIONS,
    _SchemaDB,
    _annotate_inline_lines,
    _client_class,
    _json_scalar,
    _snake_to_camel,
    _snake_to_pascal,
    _to_camel,
)
from .base import BaseRenderer


class Renderer(BaseRenderer):
    """TypeScript SDK snippet renderer."""

    lang = "javascript"   # keeps the existing directory name (examples/*/javascript/)
    extension = ".ts"

    # ------------------------------------------------------------------
    # Public interface
    # ------------------------------------------------------------------

    def config_snippet(self, connector_name: str) -> str:
        info   = _CONNECTOR_CONFIG_FIELDS.get(connector_name, {})
        fields = info.get("required_fields", [])
        msg    = info.get("msg_name", "")
        if fields and msg:
            field_lines = "\n".join(
                f"        {_snake_to_camel(f)}: {{ value: 'YOUR_{f.upper()}' }},"
                for f in fields
            )
            cred_block = (
                f"    connectorConfig: types.ConnectorSpecificConfig.create({{\n"
                f"        {connector_name}: {{\n{field_lines}\n        }},\n    }})"
            )
        else:
            cred_block = f"    // connectorConfig: set your {connector_name} credentials here"

        return f"""\
import {{ DirectPaymentClient, types }} from 'hyperswitch-prism';

const config: types.IConnectorConfig = types.ConnectorConfig.create({{
    options: types.SdkOptions.create({{ environment: types.Environment.SANDBOX }}),
{cred_block},
}});
const client = new DirectPaymentClient(config);"""

    def render_consolidated(
        self,
        connector_name: str,
        scenarios_with_payloads,
        flow_metadata: dict,
        message_schemas: dict,
        flow_items=None,
    ) -> str:
        """Generate a TypeScript file containing all scenarios and flow functions."""
        db = _SchemaDB(message_schemas)

        # --- Collect all client class names used ----------------------
        all_svcs: list[str] = []
        for scenario, _ in scenarios_with_payloads:
            for fk in scenario.flows:
                svc = flow_metadata.get(fk, {}).get("service_name", "PaymentService")
                if svc not in all_svcs:
                    all_svcs.append(svc)
        for fk, _, _ in (flow_items or []):
            svc = flow_metadata.get(fk, {}).get("service_name", "PaymentService")
            if svc not in all_svcs:
                all_svcs.append(svc)
        client_imports = ", ".join(_client_class(s) for s in all_svcs)

        # --- Build connector config block -----------------------------
        info   = _CONNECTOR_CONFIG_FIELDS.get(connector_name, {})
        fields = info.get("required_fields", [])
        msg    = info.get("msg_name", "")
        if fields and msg:
            field_lines = "\n".join(
                f"            {_snake_to_camel(f)}: {{ value: 'YOUR_{f.upper()}' }},"
                for f in fields
            )
            conn_config_block = (
                f"const _defaultConfig: types.IConnectorConfig = ConnectorConfig.create({{\n"
                f"    options: SdkOptions.create({{ environment: Environment.SANDBOX }}),\n"
                f"    connectorConfig: ConnectorSpecificConfig.create({{\n"
                f"        {connector_name}: {{\n{field_lines}\n        }},\n    }}),\n}});"
            )
        else:
            conn_config_block = (
                f"const _defaultConfig: types.IConnectorConfig = ConnectorConfig.create({{\n"
                f"    options: SdkOptions.create({{ environment: Environment.SANDBOX }}),\n"
                f"    // connectorConfig: ConnectorSpecificConfig.create({{\n"
                f"    //     {connector_name}: {{ apiKey: {{ value: 'YOUR_API_KEY' }} }}\n"
                f"    // }}),\n}});"
            )

        # --- Build private builder functions --------------------------
        builder_fns:  list[str] = []
        has_builder:  set[str]  = set()

        for flow_key, proto_req, _ in (flow_items or []):
            grpc_req = flow_metadata.get(flow_key, {}).get("grpc_request", "")
            if not grpc_req:
                continue
            if flow_key in _FLOW_BUILDER_EXTRA_PARAM:
                builder_fns.append(self._builder_fn(flow_key, proto_req, grpc_req, db))
            else:
                builder_fns.append(self._builder_fn_no_param(flow_key, proto_req, grpc_req, db))
            has_builder.add(flow_key)

        for scenario, flow_payloads in scenarios_with_payloads:
            for fk in scenario.flows:
                if fk in has_builder:
                    continue
                grpc_req = flow_metadata.get(fk, {}).get("grpc_request", "")
                if not grpc_req:
                    continue
                proto_req = flow_payloads.get(fk, {})
                if not proto_req:
                    continue
                if fk in _FLOW_BUILDER_EXTRA_PARAM:
                    builder_fns.append(self._builder_fn(fk, proto_req, grpc_req, db))
                else:
                    builder_fns.append(self._builder_fn_no_param(fk, proto_req, grpc_req, db))
                has_builder.add(fk)

        _js_var_defaults = {
            k: _to_camel(v.replace("_response", "Response"))
            for k, v in _FLOW_VAR_NAME.items()
        }

        # --- Build scenario functions ---------------------------------
        func_blocks:  list[str] = []
        func_names:   list[str] = []

        for scenario, flow_payloads in scenarios_with_payloads:
            func_name = _to_camel(f"process_{scenario.key}")
            func_names.append(func_name)

            svcs_used: list[str] = []
            for fk in scenario.flows:
                s = flow_metadata.get(fk, {}).get("service_name", "PaymentService")
                if s not in svcs_used:
                    svcs_used.append(s)

            body: list[str] = []
            for svc in svcs_used:
                cls = _client_class(svc)
                var = cls[0].lower() + cls[1:]
                body.append(f"    const {var} = new {cls}(config);")
            body.append("")

            for step_num, flow_key in enumerate(scenario.flows, 1):
                meta       = flow_metadata.get(flow_key, {})
                svc        = meta.get("service_name", "PaymentService")
                grpc_req   = meta.get("grpc_request", "")
                cls        = _client_class(svc)
                client_var = cls[0].lower() + cls[1:]

                payload = dict(flow_payloads.get(flow_key, {}))
                if flow_key == "authorize":
                    if scenario.key in ("checkout_card", "void_payment", "get_payment"):
                        payload["capture_method"] = "MANUAL"
                    elif scenario.key == "refund":
                        payload["capture_method"] = "AUTOMATIC"

                reuse_builder = (
                    flow_key in has_builder
                    and scenario.key in (_CARD_AUTHORIZE_SCENARIOS | {"checkout_card", "refund", "void_payment", "get_payment"})
                )
                if reuse_builder:
                    body.extend(self._step_with_builder(scenario.key, flow_key, step_num, client_var, _js_var_defaults))
                else:
                    body.extend(self._scenario_step(scenario.key, flow_key, step_num, payload, grpc_req, db, client_var))

            body.append(self._scenario_return(scenario))
            body_str = "\n".join(body)

            func_blocks.append(
                f"// {scenario.title}\n"
                f"// Flow: {' → '.join(scenario.flows)}\n"
                f"// {scenario.description}\n"
                f"async function {func_name}(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig): Promise<unknown> {{\n"
                f"{body_str}\n"
                f"}}"
            )

        # --- Build individual flow functions --------------------------
        for flow_key, proto_req, pm_label in (flow_items or []):
            meta       = flow_metadata.get(flow_key, {})
            svc        = meta.get("service_name", "PaymentService")
            grpc_req   = meta.get("grpc_request", "")
            rpc_name   = meta.get("rpc_name", flow_key)
            cls        = _client_class(svc)
            client_var = cls[0].lower() + cls[1:]
            func_name  = _to_camel(flow_key) if flow_key not in JS_RESERVED else f"{flow_key}Payment"
            var_name   = f"{flow_key.split('_')[0]}Response"
            pm_part    = f" ({pm_label})" if pm_label else ""

            if flow_key in has_builder:
                fn_name = "_build" + "".join(w.title() for w in flow_key.split("_")) + "Request"
                method  = _to_camel(_FLOW_KEY_TO_METHOD.get(flow_key, flow_key))
                if flow_key in _FLOW_BUILDER_EXTRA_PARAM:
                    param    = _FLOW_BUILDER_EXTRA_PARAM[flow_key][0]
                    default  = proto_req.get(param, "AUTOMATIC" if param == "capture_method" else "probe_connector_txn_001")
                    call_arg = f"'{default}'"
                    call_expr = f"{fn_name}({call_arg})"
                else:
                    call_expr = f"{fn_name}()"
                flow_body = [
                    f"    const {client_var} = new {cls}(config);",
                    "",
                    f"    const {var_name} = await {client_var}.{method}({call_expr});",
                    "",
                ]
            else:
                flow_body = list(self._scenario_step("_standalone_", flow_key, 1, proto_req, grpc_req, db, client_var))

            if flow_key == "authorize":
                flow_body.append(f"    return {{ status: {var_name}.status, transactionId: {var_name}.connectorTransactionId }};")
            elif flow_key == "setup_recurring":
                flow_body.append(f"    return {{ status: {var_name}.status, mandateId: {var_name}.connectorTransactionId }};")
            else:
                flow_body.append(f"    return {{ status: {var_name}.status }};")

            func_names.append(func_name)
            func_blocks.append(
                f"// Flow: {svc}.{rpc_name}{pm_part}\n"
                f"async function {func_name}(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig): Promise<unknown> {{\n"
                + "\n".join(flow_body) + "\n}"
            )

        # --- Builder export names ------------------------------------
        builder_export_names = [
            "_build" + "".join(w.title() for w in fk.split("_")) + "Request"
            for fk in sorted(has_builder)
        ]
        all_exports     = func_names + builder_export_names
        exports_str     = ", ".join(all_exports)
        builders_text   = ("\n\n" + "\n\n".join(builder_fns) + "\n") if builder_fns else ""
        funcs_text      = "\n\n".join(func_blocks)
        first_scenario  = scenarios_with_payloads[0][0].key if scenarios_with_payloads else "checkout_autocapture"

        # Main block — works with ts-node (CommonJS compilation, default)
        func_map_entries = "\n".join(
            f"    {fn},"
            for fn in func_names
            if fn.startswith("process")
        )
        main_block = (
            f"const _scenarioMap: Record<string, (id: string) => Promise<unknown>> = {{\n"
            f"{func_map_entries}\n}};\n\n"
            f"if (require.main === module) {{\n"
            f"    const scenario = process.argv[2] || '{first_scenario}';\n"
            f"    const key = 'process' + scenario.replace(/_([a-z])/g, (_: string, l: string) => l.toUpperCase()).replace(/^(.)/, (c: string) => c.toUpperCase());\n"
            f"    const fn = _scenarioMap[key];\n"
            f"    if (!fn) {{\n"
            f"        const available = Object.keys(_scenarioMap).map(k =>\n"
            f"            k.replace(/^process/, '').replace(/([A-Z])/g, '_$1').toLowerCase().replace(/^_/, '')\n"
            f"        );\n"
            f"        console.error(`Unknown scenario: ${{scenario}}. Available: ${{available.join(', ')}}`);\n"
            f"        process.exit(1);\n"
            f"    }}\n"
            f"    fn('order_001').catch(console.error);\n"
            f"}}"
        )

        return (
            f"// This file is auto-generated. Do not edit manually.\n"
            f"// Replace placeholder credentials with real values.\n"
            f"// Regenerate: python3 scripts/generate-connector-docs.py {connector_name}\n"
            f"//\n"
            f"// {connector_name.title()} — all integration scenarios and flows in one file.\n"
            f"// Run a scenario:  npx ts-node {connector_name}.ts {first_scenario}\n\n"
            f"import {{ {client_imports} }} from 'hyperswitch-prism';\n"
            f"import {{ types }} from 'hyperswitch-prism';\n\n"
            f"const {{ ConnectorConfig, ConnectorSpecificConfig, SdkOptions, Environment }} = types;\n\n"
            f"{conn_config_block}\n"
            f"{builders_text}\n\n"
            f"// ANCHOR: scenario_functions\n"
            f"{funcs_text}\n\n\n"
            f"export {{ {exports_str} }};\n\n"
            f"{main_block}\n"
        )

    # ------------------------------------------------------------------
    # Private helpers
    # ------------------------------------------------------------------

    def _connector_config_ts_properties(self, connector_name: str) -> str:
        """Return the connectorConfig property for ConnectorConfig.create({...})."""
        info   = _CONNECTOR_CONFIG_FIELDS.get(connector_name, {})
        fields = info.get("required_fields", [])
        if not fields:
            return f"    // connectorConfig: set credentials for {connector_name}"
        field_entries = ", ".join(
            f"{_snake_to_camel(f)}: {{ value: 'YOUR_{f.upper()}' }}"
            for f in fields
        )
        return (
            f"    connectorConfig: ConnectorSpecificConfig.create({{\n"
            f"        {connector_name}: {{ {field_entries} }}\n    }})"
        )

    def _scenario_step(
        self,
        scenario_key: str,
        flow_key: str,
        step_num: int,
        payload: dict,
        grpc_req: str,
        db: _SchemaDB,
        client_var: str = "client",
    ) -> list[str]:
        """Lines for one flow step inside a scenario function body."""
        method   = _to_camel(_FLOW_KEY_TO_METHOD.get(flow_key, flow_key))
        var_name = _to_camel(_FLOW_VAR_NAME.get(flow_key, f"{flow_key.split('_')[0]}_response").replace("_response", "Response"))
        desc     = _STEP_DESCRIPTIONS.get(flow_key, flow_key)
        lines:   list[str] = []

        lines.append(f"    // Step {step_num}: {desc}")
        lines.append(f"    const {var_name} = await {client_var}.{method}({{")

        drop = _SCENARIO_DROP_FIELDS.get((scenario_key, flow_key), frozenset())
        items = [(k, v) for k, v in payload.items() if k not in drop] if payload else []
        for idx, (key, val) in enumerate(items):
            trailing  = "," if idx < len(items) - 1 else ""
            comment   = db.get_comment(grpc_req, key)
            child_msg = db.get_type(grpc_req, key)
            cmt_part  = f"  // {comment}" if comment else ""
            js_key    = _to_camel(key)

            dyn = _DYNAMIC_FIELDS_JS.get((scenario_key, flow_key, key))
            if dyn:
                extra = "  // from authorize response" if "authorize" in dyn.lower() else "  // from setup response"
                lines.append(f'        "{js_key}": {dyn},{extra}')
            elif isinstance(val, dict):
                lines.append(f'        "{js_key}": {{{cmt_part}')
                lines.extend(_annotate_inline_lines(val, child_msg, db, indent=3, cmt="//", camel_keys=True))
                lines.append(f'        }}{trailing}')
            elif child_msg and db.is_wrapper(child_msg):
                lines.append(f'        "{js_key}": {{"value": {_json_scalar(val, js=True)}}}{trailing}{cmt_part}')
            elif child_msg and not isinstance(val, (dict, list)):
                sfwk      = db.single_field_wrapper_key(child_msg)
                inner_key = _to_camel(sfwk) if sfwk else None
                if inner_key:
                    lines.append(f'        "{js_key}": {{"{inner_key}": {{"value": {_json_scalar(val, js=True)}}}}}{trailing}{cmt_part}')
                else:
                    lines.append(f'        "{js_key}": {_json_scalar(val, js=True)}{trailing}{cmt_part}')
            else:
                lines.append(f'        "{js_key}": {_json_scalar(val, js=True)}{trailing}{cmt_part}')

        if not items:
            lines.append('        // No required fields')
        lines.append("    });")
        lines.append("")

        if flow_key == "authorize":
            lines += [
                f"    if ({var_name}.status === 'FAILED') {{",
                f"        throw new Error(`Payment failed: ${{{var_name}.error?.message}}`);",
                "    }",
                f"    if ({var_name}.status === 'PENDING') {{",
                "        // Awaiting async confirmation — handle via webhook",
                f"        return {{ status: 'pending', transactionId: {var_name}.connectorTransactionId }};",
                "    }",
                "",
            ]
        elif flow_key == "setup_recurring":
            lines += [
                f"    if ({var_name}.status === 'FAILED') {{",
                f"        throw new Error(`Recurring setup failed: ${{{var_name}.error?.message}}`);",
                "    }",
                "",
            ]
        elif flow_key in ("capture", "refund", "recurring_charge"):
            lines += [
                f"    if ({var_name}.status === 'FAILED') {{",
                f"        throw new Error(`{flow_key.replace('_', ' ').title()} failed: ${{{var_name}.error?.message}}`);",
                "    }",
                "",
            ]

        return lines

    def _step_with_builder(
        self,
        scenario_key: str,
        flow_key: str,
        step_num: int,
        client_var: str,
        js_var_defaults: dict,
    ) -> list[str]:
        """Lines for a scenario step that delegates to a pre-built _build*Request helper."""
        method   = _to_camel(_FLOW_KEY_TO_METHOD.get(flow_key, flow_key))
        var_name = js_var_defaults.get(flow_key, f"{flow_key.split('_')[0]}Response")
        desc     = _STEP_DESCRIPTIONS.get(flow_key, flow_key)
        fn_name  = "_build" + "".join(w.title() for w in flow_key.split("_")) + "Request"

        if flow_key == "authorize" and scenario_key in _CARD_AUTHORIZE_SCENARIOS:
            cm       = {"checkout_card": "MANUAL", "void_payment": "MANUAL", "get_payment": "MANUAL", "refund": "AUTOMATIC"}.get(scenario_key, "AUTOMATIC")
            call_arg = f"'{cm}'"
        else:
            call_arg = "authorizeResponse.connectorTransactionId"

        slines: list[str] = [
            f"    // Step {step_num}: {desc}",
            f"    const {var_name} = await {client_var}.{method}({fn_name}({call_arg}));",
            "",
        ]
        if flow_key == "authorize":
            slines += [
                f"    if ({var_name}.status === 'FAILED') {{",
                f"        throw new Error(`Payment failed: ${{{var_name}.error?.message}}`);",
                "    }",
                f"    if ({var_name}.status === 'PENDING') {{",
                "        // Awaiting async confirmation — handle via webhook",
                f"        return {{ status: 'pending', transactionId: {var_name}.connectorTransactionId }};",
                "    }",
                "",
            ]
        elif flow_key == "setup_recurring":
            slines += [
                f"    if ({var_name}.status === 'FAILED') {{",
                f"        throw new Error(`Recurring setup failed: ${{{var_name}.error?.message}}`);",
                "    }",
                "",
            ]
        elif flow_key in ("capture", "refund", "recurring_charge"):
            slines += [
                f"    if ({var_name}.status === 'FAILED') {{",
                f"        throw new Error(`{flow_key.replace('_', ' ').title()} failed: ${{{var_name}.error?.message}}`);",
                "    }",
                "",
            ]
        return slines

    def _scenario_return(self, scenario) -> str:
        """Return statement at the end of a scenario function."""
        key = scenario.key
        if key == "checkout_card":
            return "    return { status: captureResponse.status, transactionId: authorizeResponse.connectorTransactionId, error: authorizeResponse.error };"
        if key in ("checkout_autocapture", "checkout_wallet", "checkout_bank"):
            return "    return { status: authorizeResponse.status, transactionId: authorizeResponse.connectorTransactionId, error: authorizeResponse.error };"
        if key == "refund":
            return "    return { status: refundResponse.status, error: refundResponse.error };"
        if key == "recurring":
            return "    return { status: recurringResponse.status, transactionId: recurringResponse.connectorTransactionId ?? '', error: recurringResponse.error };"
        if key == "void_payment":
            return "    return { status: voidResponse.status, transactionId: authorizeResponse.connectorTransactionId, error: voidResponse.error };"
        if key == "get_payment":
            return "    return { status: getResponse.status, transactionId: getResponse.connectorTransactionId, error: getResponse.error };"
        if key == "create_customer":
            return "    return { customerId: createResponse.connectorCustomerId, error: createResponse.error };"
        if key == "tokenize":
            return "    return { token: tokenizeResponse.paymentMethodToken, error: tokenizeResponse.error };"
        if key == "authentication":
            return "    return { status: postAuthenticateResponse.status, error: postAuthenticateResponse.error };"
        return "    return {};"

    def _builder_fn(self, flow_key: str, proto_req: dict, grpc_req: str, db: _SchemaDB) -> str:
        """Private TypeScript builder function for flows with a dynamic parameter."""
        param_name = _FLOW_BUILDER_EXTRA_PARAM[flow_key][0]
        js_param   = _to_camel(param_name)
        fn_name    = "_build" + "".join(w.title() for w in flow_key.split("_")) + "Request"
        lines      = [f"function {fn_name}({js_param}: string) {{", "    return {"]
        for idx, (key, val) in enumerate(proto_req.items()):
            trailing  = "," if idx < len(proto_req) - 1 else ""
            comment   = db.get_comment(grpc_req, key)
            child_msg = db.get_type(grpc_req, key)
            cmt_part  = f"  // {comment}" if comment else ""
            js_key    = _to_camel(key)
            if key == param_name:
                lines.append(f'        "{js_key}": {js_param}{trailing}{cmt_part}')
            elif isinstance(val, dict):
                lines.append(f'        "{js_key}": {{{cmt_part}')
                lines.extend(_annotate_inline_lines(val, child_msg, db, indent=3, cmt="//", camel_keys=True))
                lines.append(f'        }}{trailing}')
            elif child_msg and db.is_wrapper(child_msg):
                lines.append(f'        "{js_key}": {{"value": {_json_scalar(val, js=True)}}}{trailing}{cmt_part}')
            elif child_msg and not isinstance(val, (dict, list)):
                sfwk      = db.single_field_wrapper_key(child_msg)
                inner_key = _to_camel(sfwk) if sfwk else None
                if inner_key:
                    lines.append(f'        "{js_key}": {{"{inner_key}": {{"value": {_json_scalar(val, js=True)}}}}}{trailing}{cmt_part}')
                else:
                    lines.append(f'        "{js_key}": {_json_scalar(val, js=True)}{trailing}{cmt_part}')
            else:
                lines.append(f'        "{js_key}": {_json_scalar(val, js=True)}{trailing}{cmt_part}')
        lines += ["    };", "}"]
        return "\n".join(lines)

    def _builder_fn_no_param(self, flow_key: str, proto_req: dict, grpc_req: str, db: _SchemaDB) -> str:
        """Private TypeScript builder function with no dynamic parameter."""
        fn_name = "_build" + "".join(w.title() for w in flow_key.split("_")) + "Request"
        lines   = [f"function {fn_name}() {{", "    return {"]
        for idx, (key, val) in enumerate(proto_req.items()):
            trailing  = "," if idx < len(proto_req) - 1 else ""
            comment   = db.get_comment(grpc_req, key)
            child_msg = db.get_type(grpc_req, key)
            cmt_part  = f"  // {comment}" if comment else ""
            js_key    = _to_camel(key)
            if isinstance(val, dict):
                lines.append(f'        "{js_key}": {{{cmt_part}')
                lines.extend(_annotate_inline_lines(val, child_msg, db, indent=3, cmt="//", camel_keys=True))
                lines.append(f'        }}{trailing}')
            elif child_msg and db.is_wrapper(child_msg):
                lines.append(f'        "{js_key}": {{"value": {_json_scalar(val, js=True)}}}{trailing}{cmt_part}')
            elif child_msg and not isinstance(val, (dict, list)):
                sfwk      = db.single_field_wrapper_key(child_msg)
                inner_key = _to_camel(sfwk) if sfwk else None
                if inner_key:
                    lines.append(f'        "{js_key}": {{"{inner_key}": {{"value": {_json_scalar(val, js=True)}}}}}{trailing}{cmt_part}')
                else:
                    lines.append(f'        "{js_key}": {_json_scalar(val, js=True)}{trailing}{cmt_part}')
            else:
                lines.append(f'        "{js_key}": {_json_scalar(val, js=True)}{trailing}{cmt_part}')
        lines += ["    };", "}"]
        return "\n".join(lines)
