"""
Snippet Examples Generator — SDK integration example generator for connector docs.

All functions are pure (no I/O, no global state). Called by docs/generate.py.

Proto field comments come from manifest["message_schemas"] (populated by field-probe).

DESIGN PRINCIPLES (CRITICAL):
- This is a GENERIC generator script - it must handle ALL connectors and flows generically
- NO hardcoding of flow names, field names, or connector-specific logic
- All type information must be derived from proto metadata (_PROTO_FIELD_TYPES, message_schemas)
- Flow keys, field names, and types must be resolved dynamically from probe data
- When adding type annotations, derive the TypeScript type from proto_type via _PROTO_FIELD_TYPES

Public API
----------
  detect_scenarios(probe_connector) -> list[ScenarioSpec]
    Infer applicable integration scenarios from probe data.

  render_config_section(connector_name) -> list[str]
    4-tab SDK config table — emitted once per connector doc.

  detect_scenarios(probe_connector) -> list[ScenarioSpec]
    Detect applicable integration scenarios for a connector.

  render_pm_reference_section(probe_connector, flow_metadata,
                               message_schemas) -> list[str]
    Per-PM payment_method payload reference block.

  render_llms_txt_entry(connector_name, display_name, probe_connector,
                         scenarios) -> str
    One connector block for docs/llms.txt.

  render_index_entry(connector_name, display_name, probe_connector) -> str
    One connector entry for SUMMARY.md.
"""

from __future__ import annotations

import json
import re
from dataclasses import dataclass
from pathlib import Path


# ── Proto type map (built once from .proto files) ──────────────────────────────

# {message_name: {field_name: proto_type_name}}  — all messages across all protos
_PROTO_FIELD_TYPES: dict[str, dict[str, str]] = {}

# Maps message name -> set of field names that are `repeated` in proto.
# These map to Vec<T> in prost+serde which require the key to be present in
# JSON (no #[serde(default)] generated), so we must always emit "field": [].
_PROTO_REPEATED_FIELDS: dict[str, set[str]] = {}

# Maps message name -> set of field names that are `map<>` in proto.
# These map to HashMap<K,V> in prost, represented as {"key": "value"} in generated code.
_PROTO_MAP_FIELDS: dict[str, set[str]] = {}

# Set of message type names that are "scalar wrappers" (single `value` field).
# These are stored as plain scalars in probe data but must be sent as
# {"value": ...} dicts in ParseDict calls.
_PROTO_WRAPPER_TYPES: set[str] = set()

# Proto messages that wrap a single oneof field.  In prost + serde, the outer
# struct has one field with the same name as the oneof (snake_case), so when
# building serde_json::json!({...}) we must add that extra wrapping layer.
_ONEOF_WRAPPER_FIELD: dict[str, str] = {
    "PaymentMethod": "payment_method",
    "MandateId":     "mandate_id_type",
    "MandateType":   "mandate_type",
}

# Fields that use the `optional` keyword in proto3 (tracked per message).
# Non-optional message fields are still Option<T> in prost; these are the
# non-message fields that become Option<T> only via the explicit `optional` keyword.
_PROTO_OPTIONAL_FIELDS: dict[str, set[str]] = {}

# Maps each proto type name (message or enum) to its defining proto file stem.
# Used to determine which pb2 module to use in generated Python code.
_PROTO_FILE_MAP: dict[str, str] = {}


# ── Rust type mappings for value wrapper types ─────────────────────────────────
#
# Proto wrapper types (messages with single `value` field) that map to specific 
# Rust types with custom constructors. Format:
#   proto_type: (constructor_template, import_path, needs_std_fromstr)
#
# Constructor template uses {val} placeholder for the value expression.
# Examples:
#   - "Secret::new({val}.to_string())" for SecretString
#   - "CardNumber::from_str({val}).unwrap()" for CardNumberType
#
_RUST_WRAPPER_CONSTRUCTORS: dict[str, tuple[str, str, bool]] = {
    # (constructor_expr_template, import_path, needs_FromStr_import)
    "SecretString": ("Secret::new({val}.to_string())", "hyperswitch_masking::Secret", False),
    "CardNumberType": ("CardNumber::from_str({val}).unwrap()", "cards::CardNumber", True),
    "NetworkTokenType": ("NetworkToken::from_str({val}).unwrap()", "cards::NetworkToken", True),
}


# ── Python wrapper types ───────────────────────────────────────────────────────
#
# Proto wrapper types that need special handling in Python (e.g., CardNumberType).
# These wrap a string value and need to be constructed as WrapperType(value="...").
#
_PYTHON_WRAPPER_TYPES: frozenset[str] = frozenset({
    "CardNumberType",
    "NetworkTokenType",
    "SecretString",
})

def _get_client_method(flow_key: str) -> str:
    """Map flow key to ConnectorClient method name.
    
    Handles special prefixes like dispute_*, webhook_*, etc.
    """
    # Strip dispute_ prefix for dispute flows
    if flow_key.startswith("dispute_"):
        return flow_key[8:]  # Remove "dispute_" prefix
    # Add other prefixes as needed
    elif flow_key.startswith("webhook_"):
        return flow_key[8:]  # Remove "webhook_" prefix
    return flow_key

# Flows that are not yet implemented in ConnectorClient
_UNSUPPORTED_FLOWS: frozenset[str] = frozenset({
    "handle_event",
    "verify_redirect",
})


def _generate_connector_config_rust(connector_name: str) -> str:
    """Generate accurate Rust config code using parsed proto metadata.
    
    Returns the config initialization code or None if connector has no Config struct.
    Uses _PROTO_FIELD_TYPES which is populated by load_proto_type_map().
    """
    conn_enum = _conn_enum_rust(connector_name)
    config_name = f"{conn_enum}Config"
    
    # Check if config exists in parsed proto types
    if config_name not in _PROTO_FIELD_TYPES:
        return None
    
    fields = _PROTO_FIELD_TYPES[config_name]
    if not fields:
        return None
    
    # Get repeated and optional field info
    repeated_fields = _PROTO_REPEATED_FIELDS.get(config_name, set())
    optional_fields = _PROTO_OPTIONAL_FIELDS.get(config_name, set())
    
    field_lines = []
    for field_name, field_type in fields.items():
        is_repeated = field_name in repeated_fields
        is_optional = field_name in optional_fields
        
        # Generate appropriate Rust code based on type
        if is_repeated and field_type == 'string':
            # Repeated string fields like Vec<String> or Option<Vec<String>>
            if is_optional:
                field_lines.append(f'                {field_name}: Some(vec!["value".to_string()]),  // Array field')
            else:
                field_lines.append(f'                {field_name}: vec!["value".to_string()],  // Array field')
        elif field_type == 'SecretString':
            field_lines.append(f'                {field_name}: Some(hyperswitch_masking::Secret::new("YOUR_{field_name.upper()}".to_string())),  // Authentication credential')
        elif field_type == 'string':
            field_lines.append(f'                {field_name}: Some("https://sandbox.example.com".to_string()),  // Base URL for API calls')
        elif field_type == 'bool':
            field_lines.append(f'                {field_name}: Some(false),  // Feature flag')
    
    if not field_lines:
        return None
        
    config_code = '\n'.join(field_lines)
    return f'''Some(ConnectorSpecificConfig {{
            config: Some(connector_specific_config::Config::{conn_enum}({config_name} {{
{config_code}
                ..Default::default()
            }})),
        }})'''


def _generate_connector_config_python(connector_name: str) -> str:
    """Generate an inline connector_config= kwarg for the ConnectorConfig constructor.

    When proto metadata is available, produces live (uncommented) code with the real
    credential field names so users only need to replace the placeholder values.
    Falls back to a commented-out placeholder when no Config struct is found.

    The returned string is indented with 4 spaces so it sits flush with the other
    kwargs inside ConnectorConfig(options=..., ...).
    """
    config_name = f"{_conn_display(connector_name)}Config"
    fields = _PROTO_FIELD_TYPES.get(config_name, {})
    repeated = _PROTO_REPEATED_FIELDS.get(config_name, set())

    field_lines: list[str] = []
    for field_name, field_type in fields.items():
        is_repeated = field_name in repeated
        if is_repeated and field_type == "string":
            field_lines.append(f'            {field_name}=["YOUR_{field_name.upper()}"],')
        elif field_type == "SecretString":
            field_lines.append(
                f'            {field_name}=payment_methods_pb2.SecretString(value="YOUR_{field_name.upper()}"),'
            )
        elif field_type == "string":
            field_lines.append(f'            {field_name}="YOUR_{field_name.upper()}",')
        elif field_type == "bool":
            field_lines.append(f'            {field_name}=False,')

    if field_lines:
        fields_str = "\n".join(field_lines)
        return (
            f"    connector_config=payment_pb2.ConnectorSpecificConfig(\n"
            f"        {connector_name}=payment_pb2.{config_name}(\n"
            f"{fields_str}\n"
            f"        ),\n"
            f"    ),"
        )
    # Fallback — no proto metadata found for this connector
    return (
        f"    # connector_config=payment_pb2.ConnectorSpecificConfig(\n"
        f"    #     {connector_name}=payment_pb2.{config_name}(api_key=...),\n"
        f"    # ),"
    )


def _generate_connector_config_typescript(
    connector_name: str, config_field: str = "connectorConfig"
) -> str:
    """Generate an inline TypeScript/JS connector config property for the config object literal.

    Returns a live (uncommented) property with the real credential field names so users only
    need to replace the placeholder values.  config_field is 'connectorConfig' for the new TS
    SDK and 'auth' for the legacy CJS SDK.
    Falls back to a single commented-out line when no proto metadata is found.

    The returned string uses 4-space indentation so it sits flush with 'options:' etc.
    """
    config_name = f"{_conn_display(connector_name)}Config"
    fields = _PROTO_FIELD_TYPES.get(config_name, {})
    repeated = _PROTO_REPEATED_FIELDS.get(config_name, set())

    field_lines: list[str] = []
    for field_name, field_type in fields.items():
        camel = _to_camel(field_name)
        is_repeated = field_name in repeated
        if is_repeated and field_type == "string":
            field_lines.append(f"            {camel}: ['YOUR_{field_name.upper()}'],")
        elif field_type == "SecretString":
            field_lines.append(f"            {camel}: {{ value: 'YOUR_{field_name.upper()}' }},")
        elif field_type == "string":
            field_lines.append(f"            {camel}: 'YOUR_{field_name.upper()}',")
        elif field_type == "bool":
            field_lines.append(f"            {camel}: false,")

    if field_lines:
        fields_str = "\n".join(field_lines)
        return (
            f"    {config_field}: {{\n"
            f"        {connector_name}: {{\n"
            f"{fields_str}\n"
            f"        }}\n"
            f"    }},"
        )
    # Fallback — no proto metadata found for this connector
    return f"    // {config_field}: {{ {connector_name}: {{ apiKey: {{ value: 'YOUR_API_KEY' }} }} }},"


def _generate_connector_config_kotlin(connector_name: str, indent: str = "    ") -> str:
    """Generate inline Kotlin .setConnectorConfig() builder call with actual field names.

    indent controls the base indentation level:
      '    ' (4 spaces) for the consolidated file's top-level _defaultConfig builder
      '        ' (8 spaces) for the per-flow file where the builder sits inside fun main()

    Falls back to a commented-out placeholder when no proto metadata is found.
    """
    config_name = f"{_conn_display(connector_name)}Config"
    conn_display = _conn_display(connector_name)
    fields = _PROTO_FIELD_TYPES.get(config_name, {})
    repeated = _PROTO_REPEATED_FIELDS.get(config_name, set())
    i = indent

    field_lines: list[str] = []
    for field_name, field_type in fields.items():
        pascal = "".join(w.title() for w in field_name.split("_"))
        setter = "set" + pascal
        is_repeated = field_name in repeated
        if is_repeated and field_type == "string":
            # Protobuf repeated string fields use addAll*, not set*
            field_lines.append(f"{i}            .addAll{pascal}(listOf(\"YOUR_{field_name.upper()}\"))")
        elif field_type == "SecretString":
            field_lines.append(
                f"{i}            .{setter}(SecretString.newBuilder().setValue(\"YOUR_{field_name.upper()}\").build())"
            )
        elif field_type == "string":
            field_lines.append(f"{i}            .{setter}(\"YOUR_{field_name.upper()}\")")
        elif field_type == "bool":
            field_lines.append(f"{i}            .{setter}(false)")

    if field_lines:
        fields_str = "\n".join(field_lines)
        return (
            f"{i}.setConnectorConfig(\n"
            f"{i}    ConnectorSpecificConfig.newBuilder()\n"
            f"{i}        .set{conn_display}({config_name}.newBuilder()\n"
            f"{fields_str}\n"
            f"{i}            .build())\n"
            f"{i}        .build()\n"
            f"{i})"
        )
    # Fallback — no proto metadata found for this connector
    return f"{i}// .setConnectorConfig(...) — set your {conn_display} credentials here"


def load_proto_type_map(proto_dir: Path) -> None:
    """Parse all *.proto files in proto_dir to build _PROTO_FIELD_TYPES and _PROTO_WRAPPER_TYPES."""
    global _PROTO_FIELD_TYPES, _PROTO_WRAPPER_TYPES, _PROTO_REPEATED_FIELDS
    global _PROTO_OPTIONAL_FIELDS, _PROTO_FILE_MAP, _PROTO_ONEOF_FIELDS, _PROTO_MAP_FIELDS

    type_map: dict[str, dict[str, str]] = {}
    repeated_map: dict[str, set[str]] = {}
    optional_map: dict[str, set[str]] = {}
    map_map: dict[str, set[str]] = {}
    file_map: dict[str, str] = {}
    _FIELD_RE = re.compile(
        r"^\s*(repeated\s+)?(optional\s+)?([\w<>,\s]+?)\s+(\w+)\s*=\s*\d+"
    )
    _MAP_FIELD_RE = re.compile(
        r"^\s*map<([^>]+)>\s+(\w+)\s*=\s*\d+"
    )
    _SKIP_KEYWORDS = frozenset(
        ["message", "enum", "oneof", "reserved", "option", "extensions",
         "syntax", "import", "package", "service", "rpc", "returns"]
    )

    for proto_file in sorted(proto_dir.glob("*.proto")):
        text = proto_file.read_text(encoding="utf-8")
        # Strip // comments and /* */ blocks
        text = re.sub(r"//[^\n]*", "", text)
        text = re.sub(r"/\*.*?\*/", "", text, flags=re.DOTALL)

        # Track enum types → proto file stem for Python pb2 module lookup
        for em in re.finditer(r"\benum\s+(\w+)\s*\{", text):
            file_map[em.group(1)] = proto_file.stem

        pos = 0
        while pos < len(text):
            m = re.search(r"\bmessage\s+(\w+)\s*\{", text[pos:])
            if not m:
                break
            msg_name = m.group(1)
            body_start = pos + m.end()

            # Find matching closing brace using depth counting
            depth = 1
            i = body_start
            while i < len(text) and depth > 0:
                if text[i] == "{":
                    depth += 1
                elif text[i] == "}":
                    depth -= 1
                i += 1
            body = text[body_start : i - 1]

            # Normalize multi-line field definitions (e.g. "optional FutureUsage f =\n    19;")
            # by joining the field number onto the same line as the type/name.
            body = re.sub(r"=\s*\n\s*(\d+)", r"= \1", body)

            # Extract only top-level lines (not inside nested { })
            # Also parse fields inside oneof blocks (inner_depth == 1 after entering oneof).
            fields: dict[str, str] = {}
            repeated_fields: set[str] = set()
            optional_fields: set[str] = set()
            map_fields: set[str] = set()
            inner_depth = 0
            in_oneof = False
            for line in body.splitlines():
                stripped = line.strip()
                if re.match(r"oneof\s+\w+\s*\{", stripped):
                    in_oneof = True
                inner_depth += line.count("{") - line.count("}")
                if inner_depth == 0:
                    in_oneof = False
                # Parse at depth 0 (regular fields) or inside oneof blocks at depth 1
                if inner_depth > 0 and not in_oneof:
                    continue
                if inner_depth > 1:
                    continue
                # First check for map<> fields
                mm = _MAP_FIELD_RE.match(line)
                if mm:
                    ftype, fname = mm.group(1), mm.group(2)
                    if fname not in _SKIP_KEYWORDS:
                        fields[fname] = f"map<{ftype}>"
                        map_fields.add(fname)
                    continue
                fm = _FIELD_RE.match(line)
                if fm:
                    is_repeated, is_optional, ftype, fname = fm.group(1), fm.group(2), fm.group(3), fm.group(4)
                    if ftype not in _SKIP_KEYWORDS and fname not in _SKIP_KEYWORDS:
                        fields[fname] = ftype
                        if is_repeated:
                            repeated_fields.add(fname)
                        if is_optional:
                            optional_fields.add(fname)

            type_map[msg_name] = fields
            repeated_map[msg_name] = repeated_fields
            optional_map[msg_name] = optional_fields
            map_map[msg_name] = map_fields
            file_map[msg_name] = proto_file.stem
            pos = i  # advance past the entire message body

    # Clear and update the global dicts in-place so existing references see the changes
    _PROTO_FIELD_TYPES.clear()
    _PROTO_FIELD_TYPES.update(type_map)
    _PROTO_REPEATED_FIELDS.clear()
    _PROTO_REPEATED_FIELDS.update(repeated_map)
    _PROTO_OPTIONAL_FIELDS.clear()
    _PROTO_OPTIONAL_FIELDS.update(optional_map)
    _PROTO_MAP_FIELDS.clear()
    _PROTO_MAP_FIELDS.update(map_map)
    _PROTO_FILE_MAP.clear()
    _PROTO_FILE_MAP.update(file_map)
    # Wrapper types: messages whose only field is named "value"
    _PROTO_WRAPPER_TYPES.clear()
    _PROTO_WRAPPER_TYPES.update(
        name for name, fields in type_map.items()
        if set(fields.keys()) == {"value"}
    )


def _py_module_for_type(type_name: str) -> str:
    """Return the pb2 module name for the given proto type (e.g. 'payment_pb2')."""
    stem = _PROTO_FILE_MAP.get(type_name, "payment")
    return f"{stem}_pb2"


# ── Scenario groups ─────────────────────────────────────────────────────────────

# Fallback scenario groups used when manifest.json doesn't provide them
# Use pm_key_variants for authorize to match connectors with PM-specific flows (no default entry)
_FALLBACK_SCENARIO_GROUPS: list[dict] = [
    {
        "key": "checkout_autocapture",
        "title": "One-step Payment (Authorize + Capture)",
        "description": "Simple payment that authorizes and captures in one call. Use for immediate charges.",
        "flows": ["authorize"],
        "pm_key": None,
        "required_flows": [
            {"flow_key": "authorize", "pm_key_variants": ["Card", "Ach", "Sepa", "Bacs", "GooglePay", "ApplePay"]},
        ]
    },
    {
        "key": "checkout_card",
        "title": "Card Payment (Authorize + Capture)",
        "description": "Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.",
        "flows": ["authorize", "capture"],
        "pm_key": "Card",
        "required_flows": [
            {"flow_key": "authorize", "pm_key": "Card"},
            {"flow_key": "capture", "pm_key": None}
        ]
    },
    {
        "key": "refund",
        "title": "Refund",
        "description": "Return funds to the customer for a completed payment.",
        "flows": ["authorize", "refund"],
        "pm_key": None,
        "required_flows": [
            {"flow_key": "authorize", "pm_key_variants": ["Card", "Ach", "Sepa", "Bacs", "GooglePay", "ApplePay"]},
            {"flow_key": "refund", "pm_key": None}
        ]
    },
    {
        "key": "void_payment",
        "title": "Void Payment",
        "description": "Cancel an authorized but not-yet-captured payment.",
        "flows": ["authorize", "void"],
        "pm_key": None,
        "required_flows": [
            {"flow_key": "authorize", "pm_key_variants": ["Card", "Ach", "Sepa", "Bacs", "GooglePay", "ApplePay"]},
            {"flow_key": "void", "pm_key": None}
        ]
    },
    {
        "key": "get_payment",
        "title": "Get Payment Status",
        "description": "Retrieve current payment status from the connector.",
        "flows": ["authorize", "get"],
        "pm_key": None,
        "required_flows": [
            {"flow_key": "authorize", "pm_key_variants": ["Card", "Ach", "Sepa", "Bacs", "GooglePay", "ApplePay"]},
            {"flow_key": "get", "pm_key": None}
        ]
    },
]

# ── Constants ──────────────────────────────────────────────────────────────────

_SERVICE_TO_CLIENT: dict[str, str] = {
    "PaymentService":               "PaymentClient",
    "CustomerService":                    "CustomerClient",
    "DisputeService":                     "DisputeClient",
    "EventService":                       "EventClient",
    "MerchantAuthenticationService":      "MerchantAuthenticationClient",
    "PaymentMethodAuthenticationService": "PaymentMethodAuthenticationClient",
    "PaymentMethodService":               "PaymentMethodClient",
    "RecurringPaymentService":            "RecurringPaymentClient",
    "RefundService":                      "RefundClient",
}

# Status handling for authorize+capture scenarios (separate capture step follows)
_AUTHORIZE_STATUS_HANDLING: dict[str, str] = {
    "AUTHORIZED": "Funds reserved — proceed to Capture to settle",
    "PENDING":    "Awaiting async confirmation — wait for webhook before capturing",
    "FAILED":     "Payment declined — surface error to customer, do not retry without new details",
}

# Status handling for autocapture/wallet/bank scenarios (no separate capture step)
_AUTOCAPTURE_STATUS_HANDLING: dict[str, str] = {
    "AUTHORIZED": "Payment authorized and captured — funds will be settled automatically",
    "PENDING":    "Payment processing — await webhook for final status before fulfilling",
    "FAILED":     "Payment declined — surface error to customer, do not retry without new details",
}

_SETUP_RECURRING_STATUS_HANDLING: dict[str, str] = {
    "PENDING": "Mandate stored — save connector_transaction_id for future RecurringPaymentService.Charge calls",
    "FAILED":  "Setup failed — customer must re-enter payment details",
}

# Human-readable step descriptions per flow key
_STEP_DESCRIPTIONS: dict[str, str] = {
    "authorize":        "Authorize — reserve funds on the payment method",
    "capture":          "Capture — settle the reserved funds",
    "refund":           "Refund — return funds to the customer",
    "void":             "Void — release reserved funds (cancel authorization)",
    "get":              "Get — retrieve current payment status from the connector",
    "tokenize":         "Tokenize — store card details and return a reusable token",
    "create_customer":  "Create Customer — register customer record in the connector",
    "pre_authenticate": "Pre-Authenticate — initiate 3DS flow (collect device/browser data)",
    "authenticate":     "Authenticate — execute 3DS challenge or frictionless verification",
    "post_authenticate": "Post-Authenticate — validate authentication result with the issuing bank",
    "setup_recurring":  "Setup Recurring — store the payment mandate",
    "recurring_charge": "Recurring Charge — charge against the stored mandate",
    "tokenized_authorize":        "Tokenized Authorize — reserve funds using a connector-issued payment method token",
    "tokenized_setup_recurring":  "Tokenized Setup Recurring — store a mandate using a connector token",
    "proxied_authorize":            "Proxy Authorize — reserve funds using vault alias tokens routed through a proxy",
    "proxied_setup_recurring":      "Proxy Setup Recurring — store a mandate using vault alias tokens via proxy",
}

# JavaScript reserved words — flow functions whose flow key is reserved get a "Payment" suffix.
JS_RESERVED = frozenset({"void", "delete", "return", "new", "in", "do", "for", "if"})

# Flow keys whose SDK method name differs from the flow key itself.
# All other flows use the flow key directly as the method name (snake_case).
_FLOW_KEY_TO_METHOD: dict[str, str] = {
    "recurring_charge":          "charge",                    # RecurringPaymentService.charge()
    "create_customer":           "create",                    # CustomerClient.create()
    "dispute_accept":            "accept",                    # DisputeClient.accept()
    "dispute_defend":            "defend",                    # DisputeClient.defend()
    "dispute_submit_evidence":   "submit_evidence",           # DisputeClient.submit_evidence()
    # probe data uses "verify_redirect" but SDK method is verify_redirect_response
    "verify_redirect":           "verify_redirect_response",  # PaymentClient.verify_redirect_response()
}

# Variable name used for the response of each flow step.
# Defaults to "{first_word_of_flow_key}_response" for most flows.
_FLOW_VAR_NAME: dict[str, str] = {
    "pre_authenticate":        "pre_authenticate_response",
    "authenticate":            "authenticate_response",
    "post_authenticate":       "post_authenticate_response",
    "create_customer":         "create_response",
    "setup_recurring":         "setup_response",
    "recurring_charge":        "recurring_response",
    "tokenized_authorize":     "authorize_response",
    "tokenized_setup_recurring": "setup_response",
    "proxied_authorize":         "authorize_response",
    "proxied_setup_recurring":   "setup_response",
}

# Fields that must reference the response of a previous flow step
# Maps (scenario_key, flow_key, field_name) -> Python expression string
_DYNAMIC_FIELDS: dict[tuple[str, str, str], str] = {
    ("checkout_card",      "capture",          "connector_transaction_id"): "authorize_response.connector_transaction_id",
    ("refund",             "refund",            "connector_transaction_id"): "authorize_response.connector_transaction_id",
    ("void_payment",       "void",             "connector_transaction_id"): "authorize_response.connector_transaction_id",
    ("get_payment",        "get",              "connector_transaction_id"): "authorize_response.connector_transaction_id",
    # Use mandate_reference.connector_mandate_id (pm_xxx) not connector_recurring_payment_id (seti_xxx).
    # connector_recurring_payment_id (field 1) is the SetupIntent/resource ID; the actual
    # PaymentMethod ID used by recurring charges lives in mandate_reference.connector_mandate_id.
    ("recurring",          "recurring_charge",  "connector_recurring_payment_id"): '{"connector_mandate_id": {"connector_mandate_id": setup_response.mandate_reference.connector_mandate_id.connector_mandate_id}}',
}

# Fields to drop from the probe payload for specific (scenario_key, flow_key) pairs.
# These are probe-only placeholder values that would cause connector errors if sent
# (e.g. customer IDs and token PMs that don't correspond to real objects).
_SCENARIO_DROP_FIELDS: dict[tuple[str, str], frozenset[str]] = {
    ("recurring", "recurring_charge"): frozenset({
        # payment_method_type from probe is "PAY_PAL" which is wrong for card mandates.
        "payment_method_type",
        # payment_method token is a static probe placeholder — mandate provides the PM.
        "payment_method",
    }),
}

# Same as _DYNAMIC_FIELDS but using JavaScript camelCase variable names and field access
_DYNAMIC_FIELDS_JS: dict[tuple[str, str, str], str] = {
    ("checkout_card",      "capture",          "connector_transaction_id"): "authorizeResponse.connectorTransactionId",
    ("refund",             "refund",            "connector_transaction_id"): "authorizeResponse.connectorTransactionId",
    ("void_payment",       "void",             "connector_transaction_id"): "authorizeResponse.connectorTransactionId",
    ("get_payment",        "get",              "connector_transaction_id"): "authorizeResponse.connectorTransactionId",
    # Use mandateReference.connectorMandateId (pm_xxx) not connectorRecurringPaymentId (seti_xxx).
    # connectorRecurringPaymentId (field 1) is the SetupIntent/resource ID; the actual
    # PaymentMethod ID used by recurring charges lives in mandateReference.connectorMandateId.
    ("recurring",          "recurring_charge",  "connector_recurring_payment_id"): '{ connectorMandateId: { connectorMandateId: setupResponse.mandateReference?.connectorMandateId?.connectorMandateId } }',
}

# Kotlin builder lines for dynamic scenario step fields.
# Each entry: list of raw Kotlin lines emitted inside the .apply { } block.
_DYNAMIC_FIELDS_KT: dict[tuple[str, str, str], list[str]] = {
    ("checkout_card",  "capture",          "connector_transaction_id"):
        ["connectorTransactionId = authorizeResponse.connectorTransactionId  // from Authorize"],
    ("refund",         "refund",           "connector_transaction_id"):
        ["connectorTransactionId = authorizeResponse.connectorTransactionId  // from Authorize"],
    ("void_payment",   "void",             "connector_transaction_id"):
        ["connectorTransactionId = authorizeResponse.connectorTransactionId  // from Authorize"],
    ("get_payment",    "get",              "connector_transaction_id"):
        ["connectorTransactionId = authorizeResponse.connectorTransactionId  // from Authorize"],
    ("recurring",      "recurring_charge", "connector_recurring_payment_id"):
        ["connectorRecurringPaymentIdBuilder.apply {",
         "    connectorMandateIdBuilder.apply {",
         "        connectorMandateId = setupResponse.mandateReference.connectorMandateId.connectorMandateId  // from SetupRecurring",
         "    }",
         "}"],
}

# Rust struct-literal field lines for dynamic scenario step fields.
_DYNAMIC_FIELDS_RS: dict[tuple[str, str, str], list[str]] = {
    ("checkout_card",  "capture",          "connector_transaction_id"):
        ["connector_transaction_id: Some(authorize_response.connector_transaction_id.clone()),  // from Authorize"],
    ("refund",         "refund",           "connector_transaction_id"):
        ["connector_transaction_id: Some(authorize_response.connector_transaction_id.clone()),  // from Authorize"],
    ("void_payment",   "void",             "connector_transaction_id"):
        ["connector_transaction_id: Some(authorize_response.connector_transaction_id.clone()),  // from Authorize"],
    ("get_payment",    "get",              "connector_transaction_id"):
        ["connector_transaction_id: Some(authorize_response.connector_transaction_id.clone()),  // from Authorize"],
    # Complex nested message — see Python/JS examples for the full mandate_reference extraction.
    ("recurring",      "recurring_charge", "connector_recurring_payment_id"):
        ["// connector_recurring_payment_id: TODO — extract from setup_response.mandate_reference"],
}

# Same as _DYNAMIC_FIELDS_RS but expressed as serde_json::json!({...}) key-value
# lines that are injected directly inside the json! macro call.
_DYNAMIC_FIELDS_RS_JSON: dict[tuple[str, str, str], list[str]] = {
    ("checkout_card",  "capture",          "connector_transaction_id"):
        ['"connector_transaction_id": &authorize_response.connector_transaction_id,  // from Authorize'],
    ("refund",         "refund",           "connector_transaction_id"):
        ['"connector_transaction_id": &authorize_response.connector_transaction_id,  // from Authorize'],
    ("void_payment",   "void",             "connector_transaction_id"):
        ['"connector_transaction_id": &authorize_response.connector_transaction_id,  // from Authorize'],
    ("get_payment",    "get",              "connector_transaction_id"):
        ['"connector_transaction_id": &authorize_response.connector_transaction_id,  // from Authorize'],
    # Mandate ID is a nested message — left as a TODO comment in the JSON block.
    ("recurring",      "recurring_charge", "connector_recurring_payment_id"):
        ['// "connector_recurring_payment_id": ???,  // TODO: extract from setup_response.mandate_reference'],
}

# Flows that get builder functions extracted in consolidated Rust output.
# Maps flow_key -> (extra_param_name, rust_type) for the dynamic field that becomes a param.
_FLOW_BUILDER_EXTRA_PARAM: dict[str, tuple[str, str]] = {
    "authorize": ("capture_method", "&str"),
    "capture":   ("connector_transaction_id", "&str"),
    "void":      ("connector_transaction_id", "&str"),
    "get":       ("connector_transaction_id", "&str"),
    "refund":    ("connector_transaction_id", "&str"),
    "reverse":   ("connector_transaction_id", "&str"),
}

# Scenarios that use the same card-based authorize payload as the primary standalone authorize.
# For these, process_* functions call build_authorize_request(...) instead of inlining the payload.
_CARD_AUTHORIZE_SCENARIOS: frozenset[str] = frozenset({
    "checkout_card", "checkout_autocapture", "refund", "void_payment", "get_payment"
})


# ── Scenario dataclass ─────────────────────────────────────────────────────────

@dataclass
class ScenarioSpec:
    key:             str               # e.g. "checkout_card"
    title:           str               # e.g. "Card Payment (Authorize + Capture)"
    flows:           list[str]         # ordered flow_keys e.g. ["authorize", "capture"]
    pm_key:          str | None        # primary PM for authorize, e.g. "Card"; None for refund/recurring
    description:     str               # one-liner shown in docs
    status_handling: dict[str, str]    # STATUS -> action description (for status table)


# ── Scenario detection ─────────────────────────────────────────────────────────

def detect_scenarios(probe_connector: dict) -> list[ScenarioSpec]:
    """
    Inspect probe data and return the applicable integration scenarios in display order.

    Scenario definitions are loaded from _FALLBACK_SCENARIO_GROUPS or connector-specific config.
    Each scenario group specifies required_flows that must be supported for the scenario to apply.
    """
    flows = probe_connector.get("flows", {})

    def ok(flow_key: str, pm_key: str = "default") -> bool:
        return flows.get(flow_key, {}).get(pm_key, {}).get("status") == "supported"

    def has_payload(flow_key: str, pm_key: str = "default") -> bool:
        return bool(flows.get(flow_key, {}).get(pm_key, {}).get("proto_request"))

    # Backward-compatible status_handling for specific known scenarios
    _STATUS_HANDLING_MAP: dict[str, dict[str, str]] = {
        "checkout_card":        _AUTHORIZE_STATUS_HANDLING,
        "checkout_autocapture": _AUTOCAPTURE_STATUS_HANDLING,
        "checkout_wallet":      _AUTOCAPTURE_STATUS_HANDLING,
        "checkout_bank":        _AUTOCAPTURE_STATUS_HANDLING,
        "recurring":            _SETUP_RECURRING_STATUS_HANDLING,
    }

    scenarios: list[ScenarioSpec] = []

    for group in _FALLBACK_SCENARIO_GROUPS:
        key = group.get("key", "")
        title = group.get("title", "")
        description = group.get("description", "")
        group_flows = group.get("flows", [])
        pm_key_fixed = group.get("pm_key")  # may be None or a string
        required_flows = group.get("required_flows", [])

        # Determine the resolved pm_key and check all required flows
        resolved_pm_key = pm_key_fixed
        supported = True

        for req in required_flows:
            req_flow = req.get("flow_key", "")
            req_pm = req.get("pm_key")
            req_pm_variants = req.get("pm_key_variants")

            if req_pm_variants:
                # Try each variant; use the first supported one
                found_variant = None
                for variant in req_pm_variants:
                    if ok(req_flow, variant) and has_payload(req_flow, variant):
                        found_variant = variant
                        break
                if found_variant is None:
                    supported = False
                    break
                resolved_pm_key = found_variant
            elif req_pm:
                # Specific PM key required
                if not (ok(req_flow, req_pm) and has_payload(req_flow, req_pm)):
                    supported = False
                    break
            else:
                # Default (no PM key) — flow must be supported, payload optional for some flows
                if not ok(req_flow):
                    supported = False
                    break
                # For flows that have a default payload, require it
                # (only skip payload check for flows like capture/refund which may not have payload)
                _PAYLOAD_OPTIONAL_FLOWS = frozenset({"capture", "refund", "setup_recurring", "recurring_charge"})
                if req_flow not in _PAYLOAD_OPTIONAL_FLOWS and not has_payload(req_flow):
                    supported = False
                    break

        if not supported:
            continue

        # For checkout_bank: enrich description with the actual PM name found
        if key == "checkout_bank" and resolved_pm_key:
            description = f"Direct bank debit ({resolved_pm_key}). Bank transfers typically use `capture_method=AUTOMATIC`."

        status_handling = _STATUS_HANDLING_MAP.get(key, {})

        scenarios.append(ScenarioSpec(
            key=key,
            title=title,
            flows=group_flows,
            pm_key=resolved_pm_key,
            description=description,
            status_handling=status_handling,
        ))

    return scenarios


# ── Message schema proxy ───────────────────────────────────────────────────────

class _SchemaDB:
    """Proxy over manifest["message_schemas"] + parsed proto field types."""

    def __init__(self, message_schemas: dict) -> None:
        self._schemas = message_schemas

    def get_comment(self, msg: str, field: str) -> str:
        return self._schemas.get(msg, {}).get("comments", {}).get(field, "")

    def get_type(self, msg: str, field: str) -> str:
        # Try manifest schemas first, fall back to parsed proto type map
        t = self._schemas.get(msg, {}).get("field_types", {}).get(field, "")
        if not t:
            t = _PROTO_FIELD_TYPES.get(msg, {}).get(field, "")
        return t

    def is_wrapper(self, type_name: str) -> bool:
        """Return True if type_name is a single-value wrapper message (e.g. SecretString)."""
        return type_name in _PROTO_WRAPPER_TYPES

    def single_field_wrapper_key(self, type_name: str) -> str | None:
        """If type_name has exactly one field and that field's type is a wrapper, return the field name.
        Used for messages like TokenPaymentMethodType whose single field is a SecretString."""
        fields = _PROTO_FIELD_TYPES.get(type_name, {})
        if len(fields) == 1:
            field_name, field_type = next(iter(fields.items()))
            if self.is_wrapper(field_type):
                return field_name
        return None

    def is_valid_field(self, msg: str, field: str) -> bool:
        """Check if a field exists in the proto schema for the given message type.
        
        Checks both the original field name and the snake_case version since
        probe data may have camelCase keys while proto schemas use snake_case.
        """
        proto_fields = _PROTO_FIELD_TYPES.get(msg, {})
        if field in proto_fields:
            return True
        # Convert camelCase to snake_case and check again
        snake_field = _to_snake(field)
        return snake_field in proto_fields


# ── Annotated JSON rendering ───────────────────────────────────────────────────

def _is_proto_enum(type_name: str) -> bool:
    """Return True if type_name refers to a proto enum (not a message, not a primitive)."""
    if not type_name:
        return False
    _PRIMITIVES = frozenset({"string", "bool", "int32", "uint32", "int64", "uint64",
                              "float", "double", "bytes", "sint32", "sint64",
                              "fixed32", "sfixed32", "fixed64", "sfixed64"})
    return type_name not in _PRIMITIVES and type_name not in _PROTO_FIELD_TYPES and type_name not in _PROTO_WRAPPER_TYPES


def _collect_ts_enum_types(obj: dict, msg_name: str, db: "_SchemaDB") -> set[str]:
    """Recursively collect all proto enum type names used in obj."""
    result: set[str] = set()
    if not isinstance(obj, dict):
        return result
    for key, val in obj.items():
        child_msg = db.get_type(msg_name, key)
        if isinstance(val, dict) and child_msg:
            result |= _collect_ts_enum_types(val, child_msg, db)
        elif isinstance(val, list) and val and isinstance(val[0], dict) and child_msg:
            for item in val:
                result |= _collect_ts_enum_types(item, child_msg, db)
        elif isinstance(val, str) and child_msg and _is_proto_enum(child_msg):
            result.add(child_msg)
    return result


def _json_scalar(val: object, js: bool = False) -> str:
    """Convert a scalar value to its language literal representation."""
    if isinstance(val, bool):
        if js:
            return "true" if val else "false"
        return "True" if val else "False"
    if val is None:
        return "null" if js else "None"
    return json.dumps(val)


def _annotate_inline_lines(
    obj: dict,
    msg_name: str,
    db: _SchemaDB,
    indent: int,
    cmt: str,
    camel_keys: bool = False,
    ts_mode: bool = False,
) -> list[str]:
    pad   = "    " * indent
    lines: list[str] = []

    # Filter out fields that don't exist in the proto schema to avoid TypeScript errors
    items = [(k, v) for k, v in obj.items() if db.is_valid_field(msg_name, k)]
    for idx, (key, val) in enumerate(items):
        trailing  = "," if idx < len(items) - 1 else ""
        comment   = db.get_comment(msg_name, key)
        child_msg = db.get_type(msg_name, key)
        cmt_part  = f"  {cmt} {comment}" if comment else ""
        # protobufjs (TS) keeps underscore before digit-starting segments;
        # Java/Kotlin protobuf uses standard camelCase without that underscore.
        out_key   = (_to_camel_ts(key) if ts_mode else _to_camel(key)) if camel_keys else key

        if isinstance(val, dict):
            if not child_msg:
                # Unknown field — check if it's a wrapper-style dict (single "value" key)
                # If so, preserve the outer key; otherwise flatten as oneof group
                if len(val) == 1 and "value" in val:
                    # Wrapper-style field - preserve the key with inner value
                    lines.append(f'{pad}"{out_key}": {{{cmt_part}')
                    lines.append(f'{pad}    "value": {_json_scalar(val["value"], js=camel_keys)}{trailing}')
                    lines.append(f"{pad}}}")
                else:
                    # Likely a oneof group name (e.g. mandate_id_type) - flatten
                    lines.extend(_annotate_inline_lines(val, msg_name, db, indent, cmt, camel_keys, ts_mode=ts_mode))
            else:
                lines.append(f'{pad}"{out_key}": {{{cmt_part}')
                lines.extend(_annotate_inline_lines(val, child_msg, db, indent + 1, cmt, camel_keys, ts_mode=ts_mode))
                lines.append(f"{pad}}}{trailing}")
        elif isinstance(val, list) and val and isinstance(val[0], dict):
            lines.append(f'{pad}"{out_key}": [{cmt_part}')
            for j, item in enumerate(val):
                item_trailing = "," if j < len(val) - 1 else ""
                lines.append(f"{pad}    {{")
                lines.extend(_annotate_inline_lines(item, child_msg, db, indent + 2, cmt, camel_keys, ts_mode=ts_mode))
                lines.append(f"{pad}    }}{item_trailing}")
            lines.append(f"{pad}]{trailing}")
        elif child_msg and db.is_wrapper(child_msg):
            # Scalar stored in probe data, but proto field is a wrapper message — needs {"value": ...}
            lines.append(f'{pad}"{out_key}": {{"value": {_json_scalar(val, js=camel_keys)}}}{trailing}{cmt_part}')
        elif child_msg and not isinstance(val, (dict, list)):
            # Scalar for a non-wrapper message — check if msg has one field that is itself a wrapper
            if ts_mode and _is_proto_enum(child_msg) and isinstance(val, str):
                lines.append(f'{pad}"{out_key}": {child_msg}.{val}{trailing}{cmt_part}')
            else:
                _sfwk = db.single_field_wrapper_key(child_msg)
                inner_key = ((_to_camel_ts(_sfwk) if ts_mode else _to_camel(_sfwk)) if (camel_keys and _sfwk) else _sfwk)
                if inner_key:
                    lines.append(f'{pad}"{out_key}": {{"{inner_key}": {{"value": {_json_scalar(val, js=camel_keys)}}}}}{trailing}{cmt_part}')
                else:
                    lines.append(f'{pad}"{out_key}": {_json_scalar(val, js=camel_keys)}{trailing}{cmt_part}')
        else:
            if ts_mode and child_msg == "bytes" and isinstance(val, list):
                lines.append(f'{pad}"{out_key}": new Uint8Array({json.dumps(val)}){trailing}{cmt_part}')
            else:
                lines.append(f'{pad}"{out_key}": {_json_scalar(val, js=camel_keys)}{trailing}{cmt_part}')

    return lines


def _annotate_before_lines(
    obj: dict,
    msg_name: str,
    db: _SchemaDB,
    indent: int,
) -> list[str]:
    pad   = "    " * indent
    lines: list[str] = []

    # Filter out fields that don't exist in the proto schema to avoid TypeScript errors
    items = [(k, v) for k, v in obj.items() if db.is_valid_field(msg_name, k)]
    for idx, (key, val) in enumerate(items):
        trailing  = "," if idx < len(items) - 1 else ""
        comment   = db.get_comment(msg_name, key)
        child_msg = db.get_type(msg_name, key)

        if comment:
            lines.append(f"{pad}// {comment}")

        if isinstance(val, dict):
            lines.append(f'{pad}"{key}": {{')
            lines.extend(_annotate_before_lines(val, child_msg, db, indent + 1))
            lines.append(f"{pad}}}{trailing}")
        elif isinstance(val, list) and val and isinstance(val[0], dict):
            lines.append(f'{pad}"{key}": [')
            for j, item in enumerate(val):
                item_trailing = "," if j < len(val) - 1 else ""
                lines.append(f"{pad}    {{")
                lines.extend(_annotate_before_lines(item, child_msg, db, indent + 2))
                lines.append(f"{pad}    }}{item_trailing}")
            lines.append(f"{pad}]{trailing}")
        else:
            lines.append(f'{pad}"{key}": {_json_scalar(val)}{trailing}')

    return lines


# ── Helpers ────────────────────────────────────────────────────────────────────

def _client_class(service_name: str) -> str:
    return _SERVICE_TO_CLIENT.get(
        service_name,
        service_name.replace("Service", "") + "Client",
    )


def _to_camel(snake: str) -> str:
    """Convert snake_case to camelCase (Java/Kotlin protobuf convention).

    Digit-starting segments are capitalized normally:
      enrolled_for_3ds → enrolledFor3Ds   (Java protobuf setter style)
    """
    parts = snake.split("_")
    return parts[0] + "".join(p.title() for p in parts[1:])


def _to_camel_ts(snake: str) -> str:
    """Convert snake_case to camelCase matching protobufjs (TypeScript) conventions.

    protobufjs keeps the underscore before digit-starting segments:
      enrolled_for_3ds → enrolledFor_3ds  (not enrolledFor3Ds)
    """
    parts = snake.split("_")
    result = parts[0]
    for p in parts[1:]:
        if p and p[0].isdigit():
            result += "_" + p   # keep underscore before digit segments
        else:
            result += p.title()
    return result


_CONN_ENUM_OVERRIDES: dict[str, str] = {
    "razorpayv2": "RAZORPAY",  # razorpayv2 uses the same RAZORPAY proto enum
}


def _conn_enum(connector_name: str) -> str:
    return _CONN_ENUM_OVERRIDES.get(connector_name, connector_name.upper())


def _conn_enum_rust(connector_name: str) -> str:
    """Return the PascalCase Rust Connector enum variant (e.g. Stripe, Razorpay)."""
    name = _CONN_ENUM_OVERRIDES.get(connector_name, connector_name)
    return name.replace("_", "").capitalize()


def _conn_display(connector_name: str) -> str:
    return connector_name.replace("_", " ").title().replace(" ", "")


# ── Per-SDK config-only snippet builders ──────────────────────────────────────

def _config_python(connector_name: str) -> str:
    return f"""\
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
{_generate_connector_config_python(connector_name)}
)
"""


def _config_javascript(connector_name: str) -> str:
    conn_enum = _conn_enum(connector_name)
    return f"""\
const {{ PaymentClient }} = require('hyperswitch-prism');
const {{ ConnectorConfig, Environment, Connector }} = require('hyperswitch-prism').types;

const config = ConnectorConfig.create({{
    connector: Connector.{conn_enum},
    environment: Environment.SANDBOX,
{_generate_connector_config_typescript(connector_name, "auth")}
}});"""


def _config_kotlin(connector_name: str) -> str:
    return f"""\
val config = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
{_generate_connector_config_kotlin(connector_name)}
    .build()"""


def _config_rust(connector_name: str) -> str:
    connector_config = _generate_connector_config_rust(connector_name)
    if connector_config is None:
        connector_config = "None,  // TODO: Add your connector config here"
    return f"""\
use grpc_api_types::payments::*;
use grpc_api_types::payments::connector_specific_config;

let config = ConnectorConfig {{
    connector_config: {connector_config},
    options: Some(SdkOptions {{
        environment: Environment::Sandbox.into(),
    }}),
}};"""


# ── HTML table cell builder ────────────────────────────────────────────────────

def _td(label: str, fence_lang: str, code: str) -> str:
    return (
        f'<td valign="top">\n\n'
        f"<details><summary>{label}</summary>\n\n"
        f"```{fence_lang}\n"
        f"{code}\n"
        f"```\n\n"
        f"</details>\n\n"
        f"</td>"
    )


# ── Scenario snippet builders ──────────────────────────────────────────────────

def _scenario_step_python(
    scenario_key: str,
    flow_key: str,
    step_num: int,
    payload: dict,
    grpc_req: str,
    client_var: str,
    db: _SchemaDB,
) -> list[str]:
    """
    Return lines for one step inside a scenario function body (direct type construction).
    Indentation: function body = 4 spaces, constructor args = 8 spaces.
    """
    method   = _FLOW_KEY_TO_METHOD.get(flow_key, flow_key)
    var_name = _FLOW_VAR_NAME.get(flow_key, f"{flow_key.split('_')[0]}_response")
    desc     = _STEP_DESCRIPTIONS.get(flow_key, flow_key)
    
    # Handle missing grpc request type gracefully  
    if grpc_req:
        mod = _py_module_for_type(grpc_req)
        type_path = f"{mod}.{grpc_req}"
    else:
        # grpc_req is empty - this is a bug, flow_metadata should provide it
        # Using placeholder to avoid syntax error
        type_path = f"payment_pb2.TODO_FIX_MISSING_TYPE_{flow_key}"
    
    lines: list[str] = []
    lines.append(f"    # Step {step_num}: {desc}")
    lines.append(f"    {var_name} = await {client_var}.{method}({type_path}(")

    drop_fields = _SCENARIO_DROP_FIELDS.get((scenario_key, flow_key), frozenset())
    # Build a filtered payload, substituting dynamic fields as raw expressions
    static_payload = {k: v for k, v in payload.items() if k not in drop_fields}

    if static_payload:
        for key, val in static_payload.items():
            comment   = db.get_comment(grpc_req, key)
            child_msg = db.get_type(grpc_req, key)
            cmt_part  = f"  # {comment}" if comment else ""

            # Dynamic field: emit raw expression for cross-step references
            dyn = _DYNAMIC_FIELDS.get((scenario_key, flow_key, key))
            if dyn:
                extra = "  # from Authorize response" if "connector_transaction_id" in key else "  # from SetupRecurring response"
                lines.append(f"        {key}={dyn},{extra}")
            else:
                # Use _py_direct_lines for a single field (flatten into inline kwargs)
                sub = _py_direct_lines({key: val}, grpc_req, db, indent=2)
                lines.extend(sub)
    else:
        lines.append("        # No required fields")

    lines.append("    ))")
    lines.append("")

    # Status branching for flows that drive the payment state machine
    if flow_key == "authorize":
        lines.append(f'    if {var_name}.status == "FAILED":')
        lines.append(f'        raise RuntimeError(f"Payment failed: {{{var_name}.error}}")')
        lines.append(f'    if {var_name}.status == "PENDING":')
        lines.append(f'        # Awaiting async confirmation — handle via webhook')
        lines.append(f'        return {{"status": "pending", "transaction_id": {var_name}.connector_transaction_id}}')
        lines.append("")
    elif flow_key == "setup_recurring":
        lines.append(f'    if {var_name}.status == "FAILED":')
        lines.append(f'        raise RuntimeError(f"Recurring setup failed: {{{var_name}.error}}")')
        lines.append(f'    if {var_name}.status == "PENDING":')
        lines.append(f'        # Mandate stored asynchronously — save connector_recurring_payment_id')
        lines.append(f'        return {{"status": "pending", "mandate_id": {var_name}.connector_recurring_payment_id}}')
        lines.append("")
    elif flow_key in ("capture", "refund", "recurring_charge"):
        lines.append(f'    if {var_name}.status == "FAILED":')
        lines.append(f'        raise RuntimeError(f"{flow_key.title()} failed: {{{var_name}.error}}")')
        lines.append("")

    return lines


def _scenario_return_python(scenario: ScenarioSpec) -> str:
    """Return the final return statement for a scenario function."""
    if scenario.key in ("checkout_card",):
        return '    return {"status": getattr(capture_response, "status", ""), "transaction_id": getattr(authorize_response, "connector_transaction_id", ""), "error": getattr(capture_response, "error", None)}'
    elif scenario.key in ("checkout_autocapture", "checkout_wallet", "checkout_bank"):
        return '    return {"status": getattr(authorize_response, "status", ""), "transaction_id": getattr(authorize_response, "connector_transaction_id", ""), "error": getattr(authorize_response, "error", None)}'
    elif scenario.key == "refund":
        return '    return {"status": getattr(refund_response, "status", ""), "error": getattr(refund_response, "error", None)}'
    elif scenario.key == "recurring":
        return '    return {"status": getattr(recurring_response, "status", ""), "transaction_id": getattr(recurring_response, "connector_transaction_id", ""), "error": getattr(recurring_response, "error", None)}'
    elif scenario.key == "void_payment":
        return '    return {"status": getattr(void_response, "status", ""), "transaction_id": getattr(authorize_response, "connector_transaction_id", ""), "error": getattr(void_response, "error", None)}'
    elif scenario.key == "get_payment":
        return '    return {"status": getattr(get_response, "status", ""), "transaction_id": getattr(get_response, "connector_transaction_id", ""), "error": getattr(get_response, "error", None)}'
    elif scenario.key == "create_customer":
        return '    return {"customer_id": getattr(create_response, "connector_customer_id", ""), "error": getattr(create_response, "error", None)}'
    elif scenario.key == "tokenize":
        return '    return {"token": getattr(tokenize_response, "payment_method_token", ""), "error": getattr(tokenize_response, "error", None)}'
    elif scenario.key == "authentication":
        return '    return {"status": getattr(post_authenticate_response, "status", ""), "error": getattr(post_authenticate_response, "error", None)}'
    return '    return {}'


def _scenario_step_javascript(
    scenario_key: str,
    flow_key: str,
    step_num: int,
    payload: dict,
    grpc_req: str,
    db: _SchemaDB,
    client_var: str = "client",
    ts_mode: bool = False,
) -> list[str]:
    """Return lines for one step inside a JavaScript scenario function body."""
    method   = _to_camel(_FLOW_KEY_TO_METHOD.get(flow_key, flow_key))
    _js_var_defaults = {k: _to_camel(v.replace("_response", "Response")) for k, v in _FLOW_VAR_NAME.items()}
    var_name = _js_var_defaults.get(flow_key, f"{flow_key.split('_')[0]}Response")
    desc     = _STEP_DESCRIPTIONS.get(flow_key, flow_key)
    lines: list[str] = []

    lines.append(f"    // Step {step_num}: {desc}")
    lines.append(f"    const {var_name} = await {client_var}.{method}({{")

    drop_fields_js = _SCENARIO_DROP_FIELDS.get((scenario_key, flow_key), frozenset())
    if payload:
        items = [(k, v) for k, v in payload.items() if k not in drop_fields_js]
        for idx, (key, val) in enumerate(items):
            trailing  = "," if idx < len(items) - 1 else ""
            comment   = db.get_comment(grpc_req, key)
            child_msg = db.get_type(grpc_req, key)
            cmt_part  = f"  // {comment}" if comment else ""

            _ck = _to_camel_ts(key) if ts_mode else _to_camel(key)
            dyn = _DYNAMIC_FIELDS_JS.get((scenario_key, flow_key, key))
            if dyn:
                extra = "  // from authorize response" if "authorize" in dyn.lower() else "  // from setup response"
                lines.append(f'        "{_ck}": {dyn},{extra}')
            elif isinstance(val, dict):
                lines.append(f'        "{_ck}": {{{cmt_part}')
                lines.extend(_annotate_inline_lines(val, child_msg, db, indent=3, cmt="//", camel_keys=True, ts_mode=ts_mode))
                lines.append(f'        }}{trailing}')
            elif child_msg and db.is_wrapper(child_msg):
                lines.append(f'        "{_ck}": {{"value": {_json_scalar(val, js=True)}}}{trailing}{cmt_part}')
            elif child_msg and not isinstance(val, (dict, list)):
                if ts_mode and _is_proto_enum(child_msg) and isinstance(val, str):
                    lines.append(f'        "{_ck}": {child_msg}.{val}{trailing}{cmt_part}')
                else:
                    _sfwk = db.single_field_wrapper_key(child_msg)
                    inner_key = (_to_camel_ts(_sfwk) if ts_mode else _to_camel(_sfwk)) if _sfwk else None
                    if inner_key:
                        lines.append(f'        "{_ck}": {{"{inner_key}": {{"value": {_json_scalar(val, js=True)}}}}}{trailing}{cmt_part}')
                    else:
                        lines.append(f'        "{_ck}": {_json_scalar(val, js=True)}{trailing}{cmt_part}')
            else:
                lines.append(f'        "{_ck}": {_json_scalar(val, js=True)}{trailing}{cmt_part}')
    else:
        lines.append('        // No required fields')

    lines.append("    });")
    lines.append("")

    if flow_key == "authorize":
        lines.append(f"    if ({var_name}.status === types.PaymentStatus.FAILURE) {{")
        lines.append(f"        throw new Error(`Payment failed: ${{JSON.stringify({var_name}.error)}}`);")
        lines.append("    }")
        lines.append(f"    if ({var_name}.status === types.PaymentStatus.PENDING) {{")
        lines.append(f"        // Awaiting async confirmation — handle via webhook")
        lines.append(f"        return {{ status: 'pending', transactionId: {var_name}.connectorTransactionId }} as any;")
        lines.append("    }")
        lines.append("")
    elif flow_key == "setup_recurring":
        lines.append(f"    if ({var_name}.status === types.PaymentStatus.FAILURE) {{")
        lines.append(f"        throw new Error(`Recurring setup failed: ${{JSON.stringify({var_name}.error)}}`);")
        lines.append("    }")
        lines.append("")
    elif flow_key == "refund":
        lines.append(f"    if ({var_name}.status === types.RefundStatus.REFUND_FAILURE) {{")
        lines.append(f"        throw new Error(`Refund failed: ${{JSON.stringify({var_name}.error)}}`);")
        lines.append("    }")
        lines.append("")
    elif flow_key in ("capture", "recurring_charge"):
        lines.append(f"    if ({var_name}.status === types.PaymentStatus.FAILURE) {{")
        lines.append(f"        throw new Error(`{flow_key.replace('_', ' ').title()} failed: ${{JSON.stringify({var_name}.error)}}`);")
        lines.append("    }")
        lines.append("")

    return lines


def _scenario_return_javascript(scenario: ScenarioSpec) -> str:
    if scenario.key == "checkout_card":
        return "    return { status: captureResponse.status, transactionId: authorizeResponse.connectorTransactionId!, error: authorizeResponse.error } as any;"
    elif scenario.key in ("checkout_autocapture", "checkout_wallet", "checkout_bank"):
        return "    return { status: authorizeResponse.status, transactionId: authorizeResponse.connectorTransactionId!, error: authorizeResponse.error } as any;"
    elif scenario.key == "refund":
        return "    return { status: refundResponse.status, error: refundResponse.error } as any;"
    elif scenario.key == "recurring":
        return "    return { status: recurringResponse.status, transactionId: recurringResponse.connectorTransactionId ?? '', error: recurringResponse.error } as any;"
    elif scenario.key == "void_payment":
        return "    return { status: voidResponse.status, transactionId: authorizeResponse.connectorTransactionId!, error: voidResponse.error } as any;"
    elif scenario.key == "get_payment":
        return "    return { status: getResponse.status, transactionId: getResponse.connectorTransactionId!, error: getResponse.error } as any;"
    elif scenario.key == "create_customer":
        return "    return { customerId: createResponse.connectorCustomerId!, error: createResponse.error } as any;"
    elif scenario.key == "tokenize":
        return "    return { token: tokenizeResponse.paymentMethodToken!, error: tokenizeResponse.error } as any;"
    elif scenario.key == "authentication":
        return "    return { status: postAuthenticateResponse.status, error: postAuthenticateResponse.error } as any;"
    return "    return {};"


def _scenario_step_kotlin(
    scenario_key: str,
    flow_key: str,
    step_num: int,
    payload: dict,
    grpc_req: str,
    message_schemas: dict,
    client_var: str,
) -> list[str]:
    """Return Kotlin lines for one step inside a scenario function body (indent=1)."""
    pad  = "    "
    pad2 = "        "
    var_name = _to_camel(_FLOW_VAR_NAME.get(flow_key, f"{flow_key.split('_')[0]}_response"))
    desc     = _STEP_DESCRIPTIONS.get(flow_key, flow_key)

    # Collect dynamic overrides for this (scenario, flow)
    dyn_by_field: dict[str, list[str]] = {}
    for (sk, fk, field_name), raw_lines in _DYNAMIC_FIELDS_KT.items():
        if sk == scenario_key and fk == flow_key:
            dyn_by_field[field_name] = raw_lines

    # Static payload: remove dropped and dynamically-overridden fields
    drop_fields    = _SCENARIO_DROP_FIELDS.get((scenario_key, flow_key), frozenset())
    static_payload = {k: v for k, v in payload.items() if k not in drop_fields and k not in dyn_by_field}

    lines: list[str] = []
    lines.append(f"{pad}// Step {step_num}: {desc}")
    # Kotlin SDK uses snake_case method names (same as Python); recurring_charge → charge
    kt_method = _FLOW_KEY_TO_METHOD.get(flow_key, flow_key)
    lines.append(f"{pad}val {var_name} = {client_var}.{kt_method}({grpc_req}.newBuilder().apply {{")
    for body_line in _kotlin_payload_lines(static_payload, grpc_req, message_schemas, indent=2):
        lines.append(body_line)
    for raw_lines in dyn_by_field.values():
        for raw_line in raw_lines:
            lines.append(f"{pad2}{raw_line}")
    lines.append(f"{pad}}}.build())")
    lines.append("")

    if flow_key == "authorize":
        lines.append(f'{pad}when ({var_name}.status.name) {{')
        lines.append(f'{pad}    "FAILED"  -> throw RuntimeException("Payment failed: ${{{var_name}.error.unifiedDetails.message}}")')
        lines.append(f'{pad}    "PENDING" -> return mapOf("status" to "PENDING")  // await webhook before proceeding')
        lines.append(f'{pad}}}')
        lines.append("")
    elif flow_key == "setup_recurring":
        lines.append(f'{pad}if ({var_name}.status.name == "FAILED")')
        lines.append(f'{pad}    throw RuntimeException("Setup failed: ${{{var_name}.error.unifiedDetails.message}}")')
        lines.append("")
    elif flow_key in ("capture", "refund", "recurring_charge"):
        lines.append(f'{pad}if ({var_name}.status.name == "FAILED")')
        lines.append(f'{pad}    throw RuntimeException("{flow_key.replace("_", " ").title()} failed: ${{{var_name}.error.unifiedDetails.message}}")')
        lines.append("")

    return lines


def _scenario_return_kotlin(scenario: "ScenarioSpec") -> str:
    if scenario.key == "checkout_card":
        return '    return mapOf("status" to captureResponse.status.name, "transactionId" to authorizeResponse.connectorTransactionId, "error" to authorizeResponse.error)'
    elif scenario.key in ("checkout_autocapture", "checkout_wallet", "checkout_bank"):
        return '    return mapOf("status" to authorizeResponse.status.name, "transactionId" to authorizeResponse.connectorTransactionId, "error" to authorizeResponse.error)'
    elif scenario.key == "refund":
        return '    return mapOf("status" to refundResponse.status.name, "error" to refundResponse.error)'
    elif scenario.key == "recurring":
        return '    return mapOf("status" to recurringResponse.status.name, "transactionId" to (recurringResponse.connectorTransactionId ?: ""), "error" to recurringResponse.error)'
    elif scenario.key == "void_payment":
        return '    return mapOf("status" to voidResponse.status.name, "transactionId" to authorizeResponse.connectorTransactionId, "error" to voidResponse.error)'
    elif scenario.key == "get_payment":
        return '    return mapOf("status" to getResponse.status.name, "transactionId" to getResponse.connectorTransactionId, "error" to getResponse.error)'
    elif scenario.key == "create_customer":
        return '    return mapOf("customerId" to createResponse.connectorCustomerId, "error" to createResponse.error)'
    elif scenario.key == "tokenize":
        return '    return mapOf("token" to tokenizeResponse.paymentMethodToken, "error" to tokenizeResponse.error)'
    elif scenario.key == "authentication":
        return '    return mapOf("status" to postAuthenticateResponse.status.name, "error" to postAuthenticateResponse.error)'
    return '    return mapOf()'


def _rust_status_check_lines(flow_key: str, var_name: str, pad: str = "    ") -> list[str]:
    """Return Rust status-check lines for a flow response variable (used in both scenarios and builders)."""
    lines: list[str] = []
    if flow_key in ("authorize", "tokenized_authorize", "proxied_authorize"):
        label = "Tokenized authorize" if flow_key == "tokenized_authorize" else ("Proxy authorize" if flow_key == "proxied_authorize" else "Payment")
        lines.append(f'{pad}match {var_name}.status() {{')
        lines.append(f'{pad}    PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => return Err(format!("{label} failed: {{:?}}", {var_name}.error).into()),')
        lines.append(f'{pad}    PaymentStatus::Pending => return Ok("pending — awaiting webhook".to_string()),')
        lines.append(f'{pad}    _                      => {{}},')
        lines.append(f'{pad}}}')
        lines.append("")
    elif flow_key in ("setup_recurring", "tokenized_setup_recurring", "proxied_setup_recurring"):
        label = flow_key.replace("_", " ").title()
        lines.append(f'{pad}if {var_name}.status() == PaymentStatus::Failure {{')
        lines.append(f'{pad}    return Err(format!("{label} failed: {{:?}}", {var_name}.error).into());')
        lines.append(f'{pad}}}')
        lines.append("")
    elif flow_key in ("capture", "recurring_charge"):
        title = flow_key.replace("_", " ").title()
        lines.append(f'{pad}if {var_name}.status() == PaymentStatus::Failure {{')
        lines.append(f'{pad}    return Err(format!("{title} failed: {{:?}}", {var_name}.error).into());')
        lines.append(f'{pad}}}')
        lines.append("")
    elif flow_key == "refund":
        lines.append(f'{pad}if {var_name}.status() == RefundStatus::RefundFailure {{')
        lines.append(f'{pad}    return Err(format!("Refund failed: {{:?}}", {var_name}.error).into());')
        lines.append(f'{pad}}}')
        lines.append("")
    elif flow_key in ("pre_authenticate", "authenticate", "post_authenticate", "create_server_session_authentication_token"):
        label = flow_key.replace("_", " ").title()
        lines.append(f'{pad}if {var_name}.status_code >= 400 {{')
        lines.append(f'{pad}    return Err(format!("{label} failed (status_code={{}})", {var_name}.status_code).into());')
        lines.append(f'{pad}}}')
        lines.append("")
    return lines


def _scenario_step_rust(
    scenario_key: str,
    flow_key: str,
    step_num: int,
    payload: dict,
    grpc_req: str,
    message_schemas: dict,
) -> list[str]:
    """Return Rust lines for one step inside a scenario function body (indent=1).

    Uses direct struct literal construction instead of serde_json::from_value.
    Dynamic cross-step fields (e.g. connector_transaction_id from previous response)
    are injected from _DYNAMIC_FIELDS_RS as pre-written struct field lines.
    """
    pad  = "    "
    pad2 = "        "
    var_name = _FLOW_VAR_NAME.get(flow_key, f"{flow_key.split('_')[0]}_response")
    desc     = _STEP_DESCRIPTIONS.get(flow_key, flow_key)

    # Collect struct-literal dynamic field lines for this (scenario, flow)
    dyn_by_field: dict[str, list[str]] = {}
    for (sk, fk, field_name), raw_lines in _DYNAMIC_FIELDS_RS.items():
        if sk == scenario_key and fk == flow_key:
            dyn_by_field[field_name] = raw_lines

    drop_fields    = _SCENARIO_DROP_FIELDS.get((scenario_key, flow_key), frozenset())
    static_payload = {k: v for k, v in payload.items() if k not in drop_fields and k not in dyn_by_field}

    # Skip flows that aren't implemented in ConnectorClient
    if flow_key in _UNSUPPORTED_FLOWS:
        lines.append(f"{pad}// TODO: {flow_key} not yet implemented in ConnectorClient")
        lines.append(f"{pad}let {var_name} = todo!(\"{flow_key} not implemented\");")
        return lines
    
    lines: list[str] = []
    lines.append(f"{pad}// Step {step_num}: {desc}")
    
    # Handle missing grpc request type gracefully
    if grpc_req:
        rust_type = grpc_req
    else:
        # grpc_req is empty - this is a bug, flow_metadata should provide it
        # Using placeholder to avoid syntax error
        rust_type = f"TODO_FIX_MISSING_TYPE_{flow_key}"
    
    lines.append(f"{pad}let {var_name} = client.{_get_client_method(flow_key)}({rust_type} {{")
    for struct_line in _rust_struct_lines(static_payload, grpc_req, message_schemas, indent=2):
        lines.append(struct_line)
    for raw_lines in dyn_by_field.values():
        for raw_line in raw_lines:
            lines.append(f"{pad2}{raw_line}")
    lines.append(f"{pad}    ..Default::default()")
    lines.append(f"{pad}}}, &HashMap::new(), None).await?;")
    lines.append("")

    lines.extend(_rust_status_check_lines(flow_key, var_name, pad))

    return lines


def _scenario_return_rust(scenario: "ScenarioSpec") -> str:
    if scenario.key == "checkout_card":
        return '    Ok(format!("Payment completed: {}", authorize_response.connector_transaction_id.as_deref().unwrap_or("")))'
    elif scenario.key in ("checkout_autocapture", "checkout_wallet", "checkout_bank"):
        return '    Ok(format!("Payment: {:?} — {}", authorize_response.status(), authorize_response.connector_transaction_id.as_deref().unwrap_or("")))'
    elif scenario.key == "refund":
        return '    Ok(format!("Refunded: {:?}", refund_response.status()))'
    elif scenario.key == "recurring":
        return '    Ok(format!("Charged: {:?}", recurring_response.status()))'
    elif scenario.key == "void_payment":
        return '    Ok(format!("Voided: {:?}", void_response.status()))'
    elif scenario.key == "get_payment":
        return '    Ok(format!("Status: {:?}", get_response.status()))'
    elif scenario.key == "create_customer":
        return '    Ok(format!("Customer: {}", create_response.connector_customer_id))'
    elif scenario.key == "tokenize":
        return '    Ok(format!("Token: {}", tokenize_response.payment_method_token))'
    elif scenario.key == "authentication":
        return '    Ok(format!("Auth: {:?}", post_authenticate_response.status()))'
    elif scenario.key == "tokenized_checkout":
        return '    Ok(format!("Tokenized payment completed: {}", authorize_response.connector_transaction_id.as_deref().unwrap_or("")))'
    elif scenario.key == "tokenized_recurring":
        return '    Ok(format!("Tokenized recurring charged: {:?}", recurring_response.status()))'
    elif scenario.key == "proxy_checkout":
        return '    Ok(format!("Proxy authorized: {}", authorize_response.connector_transaction_id.as_deref().unwrap_or("")))'
    elif scenario.key == "proxy_3ds_checkout":
        return '    Ok(format!("Proxy 3DS authorized: {}", authorize_response.connector_transaction_id.as_deref().unwrap_or("")))'
    return '    Ok("done".to_string())'


# ── Per-language builder function generators ──────────────────────────────────


def _py_direct_lines(
    obj: dict,
    msg_name: str,
    db: "_SchemaDB",
    indent: int,
    variable_fields: frozenset = frozenset(),
) -> list[str]:
    """Generate Python direct constructor keyword-argument lines for a proto message.

    Returns a list of lines like ``field=value,`` suitable for embedding
    directly inside a ``payment_pb2.TypeName(`` call.
    """
    pad = "    " * indent
    lines: list[str] = []

    for key, val in obj.items():
        comment   = db.get_comment(msg_name, key)
        child_msg = db.get_type(msg_name, key)
        cmt_part  = f"  # {comment}" if comment else ""

        if key in variable_fields:
            if child_msg and _is_proto_enum(child_msg):
                em = _py_module_for_type(child_msg)
                lines.append(f"{pad}{key}={em}.{child_msg}.Value({key}),{cmt_part}")
            elif child_msg and db.is_wrapper(child_msg):
                wm = _py_module_for_type(child_msg)
                lines.append(f"{pad}{key}={wm}.{child_msg}(value={key}),{cmt_part}")
            else:
                lines.append(f"{pad}{key}={key},{cmt_part}")
            continue

        if isinstance(val, dict):
            if child_msg and db.is_wrapper(child_msg):
                # SecretString-style wrapper: extract .value from probe dict
                inner_val = val.get("value", "")
                wm = _py_module_for_type(child_msg)
                lines.append(f"{pad}{key}={wm}.{child_msg}(value={json.dumps(inner_val)}),{cmt_part}")
            elif child_msg and child_msg in _ONEOF_WRAPPER_FIELD:
                # Oneof wrapper (PaymentMethod, MandateId): val = {"case": {...}}
                # In Python proto, oneof cases are set directly on the message.
                msg_mod = _py_module_for_type(child_msg)
                lines.append(f"{pad}{key}={msg_mod}.{child_msg}({cmt_part}")
                for case_key, case_val in val.items():
                    case_type = db.get_type(child_msg, case_key)
                    if isinstance(case_val, dict) and case_type:
                        cm = _py_module_for_type(case_type)
                        inner = _py_direct_lines(case_val, case_type, db, indent + 2)
                        if inner:
                            lines.append(f"{pad}    {case_key}={cm}.{case_type}(")
                            lines.extend(inner)
                            lines.append(f"{pad}    ),")
                        else:
                            lines.append(f"{pad}    {case_key}={cm}.{case_type}(),")
                    elif case_val is None or case_val == {}:
                        if case_type:
                            cm = _py_module_for_type(case_type)
                            lines.append(f"{pad}    {case_key}={cm}.{case_type}(),")
                        else:
                            lines.append(f"{pad}    {case_key}=None,")
                    else:
                        lines.append(f"{pad}    {case_key}={json.dumps(case_val)},")
                lines.append(f"{pad}),")
            elif child_msg:
                # Regular nested message — recurse
                cm = _py_module_for_type(child_msg)
                inner = _py_direct_lines(val, child_msg, db, indent + 1)
                if inner:
                    lines.append(f"{pad}{key}={cm}.{child_msg}({cmt_part}")
                    lines.extend(inner)
                    lines.append(f"{pad}),")
                else:
                    lines.append(f"{pad}{key}={cm}.{child_msg}(),{cmt_part}")
            else:
                # Unknown field — likely a oneof group name (not in proto schema as a field).
                # Flatten: treat the inner dict fields as direct fields of the current message.
                lines.extend(_py_direct_lines(val, msg_name, db, indent, variable_fields))
        elif isinstance(val, bool):
            lines.append(f"{pad}{key}={str(val)},{cmt_part}")
        elif isinstance(val, (int, float)):
            lines.append(f"{pad}{key}={val},{cmt_part}")
        elif isinstance(val, str):
            if child_msg and _is_proto_enum(child_msg):
                em = _py_module_for_type(child_msg)
                lines.append(f"{pad}{key}={em}.{child_msg}.Value({json.dumps(val)}),{cmt_part}")
            elif child_msg and child_msg in _PYTHON_WRAPPER_TYPES:
                # Special wrapper types like CardNumberType need value= wrapping
                wm = _py_module_for_type(child_msg)
                lines.append(f"{pad}{key}={wm}.{child_msg}(value={json.dumps(val)}),{cmt_part}")
            else:
                lines.append(f"{pad}{key}={json.dumps(val)},{cmt_part}")
        else:
            lines.append(f"{pad}# {key}: {json.dumps(val)}{cmt_part}")

    return lines


def _py_builder_fn(flow_key: str, proto_req: dict, grpc_req: str, db: "_SchemaDB") -> str:
    """Return a Python private builder function for the given flow (direct type construction)."""
    param_name = _FLOW_BUILDER_EXTRA_PARAM[flow_key][0]  # snake_case
    mod        = _py_module_for_type(grpc_req)
    lines: list[str] = [f"def _build_{flow_key}_request({param_name}: str):"]
    lines.append(f"    return {mod}.{grpc_req}(")
    lines.extend(_py_direct_lines(
        proto_req, grpc_req, db, indent=2,
        variable_fields=frozenset({param_name}),
    ))
    lines.append("    )")
    return "\n".join(lines)


def _expand_oneof_groups(proto_req: dict, msg: str, db: "_SchemaDB") -> list[tuple]:
    """Expand oneof group keys in proto_req to their inner variant fields.

    Proto probe data encodes oneof fields as ``{group_name: {variant: {...}}}``
    where ``group_name`` is NOT a real proto field (it's the oneof keyword name).
    This helper flattens those so only real proto field names remain, e.g.:
      {"domain_context": {"payment": {...}}}  →  [("payment", {...})]
    """
    result: list[tuple] = []
    for k, v in proto_req.items():
        if db.is_valid_field(msg, k):
            result.append((k, v))
        elif isinstance(v, dict):
            # Likely a oneof group name — expand inner fields that are valid proto fields
            for inner_k, inner_v in v.items():
                if db.is_valid_field(msg, inner_k):
                    result.append((inner_k, inner_v))
    return result


def _js_builder_fn(flow_key: str, proto_req: dict, grpc_req: str, db: "_SchemaDB", ts_mode: bool = False) -> str:
    """Return a JavaScript/TypeScript private builder function for the given flow."""
    param_name = _FLOW_BUILDER_EXTRA_PARAM[flow_key][0]  # snake_case
    js_param   = _to_camel(param_name)
    fn_name    = "_build" + "".join(w.title() for w in flow_key.split("_")) + "Request"
    # Expand oneof group names and filter invalid fields
    items = _expand_oneof_groups(proto_req, grpc_req, db)
    # Type annotation for the parameter - derive GENERICALLY from proto type metadata
    # NEVER hardcode field names here - use _PROTO_FIELD_TYPES to look up types
    type_ann = ""
    if ts_mode:
        proto_type = _PROTO_FIELD_TYPES.get(grpc_req, {}).get(param_name, "")
        if proto_type:
            if _is_proto_enum(proto_type):
                type_ann = f": types.{proto_type}"
            elif proto_type in ("string", "bytes"):
                type_ann = ": string"
            elif proto_type in ("int32", "int64", "uint32", "uint64", "sint32", "sint64", "fixed32", "fixed64", "sfixed32", "sfixed64"):
                type_ann = ": number"
            elif proto_type == "bool":
                type_ann = ": boolean"
            elif proto_type in _PROTO_FIELD_TYPES:
                # It's a message type
                type_ann = f": types.I{proto_type}"
            else:
                type_ann = ": any"
    
    # Add return type annotation for TypeScript
    return_type = f": types.I{grpc_req}" if ts_mode else ""
    
    lines: list[str] = [f"function {fn_name}({js_param}{type_ann}){return_type} {{"]
    lines.append("    return {")
    for idx, (key, val) in enumerate(items):
        trailing  = "," if idx < len(items) - 1 else ""
        comment   = db.get_comment(grpc_req, key)
        child_msg = db.get_type(grpc_req, key)
        cmt_part  = f"  // {comment}" if comment else ""
        js_key    = _to_camel_ts(key) if ts_mode else _to_camel(key)
        if key == param_name:
            lines.append(f'        "{js_key}": {js_param}{trailing}{cmt_part}')
        elif isinstance(val, dict):
            lines.append(f'        "{js_key}": {{{cmt_part}')
            lines.extend(_annotate_inline_lines(val, child_msg, db, indent=3, cmt="//", camel_keys=True, ts_mode=ts_mode))
            lines.append(f'        }}{trailing}')
        elif child_msg and db.is_wrapper(child_msg):
            lines.append(f'        "{js_key}": {{"value": {_json_scalar(val, js=True)}}}{trailing}{cmt_part}')
        elif isinstance(val, list) and val and isinstance(val[0], dict):
            lines.append(f'        "{js_key}": [{cmt_part}')
            for j, item in enumerate(val):
                item_trailing = "," if j < len(val) - 1 else ""
                lines.append(f'            {{')
                lines.extend(_annotate_inline_lines(item, child_msg, db, indent=4, cmt="//", camel_keys=True, ts_mode=ts_mode))
                lines.append(f'            }}{item_trailing}')
            lines.append(f'        ]{trailing}')
        elif child_msg and not isinstance(val, (dict, list)):
            if ts_mode and _is_proto_enum(child_msg) and isinstance(val, str):
                lines.append(f'        "{js_key}": {child_msg}.{val}{trailing}{cmt_part}')
            else:
                sfwk      = db.single_field_wrapper_key(child_msg)
                inner_key = (_to_camel_ts(sfwk) if ts_mode else _to_camel(sfwk)) if sfwk else None
                if inner_key:
                    lines.append(f'        "{js_key}": {{"{inner_key}": {{"value": {_json_scalar(val, js=True)}}}}}{trailing}{cmt_part}')
                else:
                    lines.append(f'        "{js_key}": {_json_scalar(val, js=True)}{trailing}{cmt_part}')
        else:
            if ts_mode and child_msg == "bytes" and isinstance(val, list):
                lines.append(f'        "{js_key}": new Uint8Array({json.dumps(val)}){trailing}{cmt_part}')
            else:
                lines.append(f'        "{js_key}": {_json_scalar(val, js=True)}{trailing}{cmt_part}')
    lines.append("    };")
    lines.append("}")
    return "\n".join(lines)


def _py_builder_fn_no_param(flow_key: str, proto_req: dict, grpc_req: str, db: "_SchemaDB") -> str:
    """Return a Python private builder function with no dynamic parameter (direct type construction)."""
    mod   = _py_module_for_type(grpc_req)
    lines: list[str] = [f"def _build_{flow_key}_request():"]
    lines.append(f"    return {mod}.{grpc_req}(")
    lines.extend(_py_direct_lines(proto_req, grpc_req, db, indent=2))
    lines.append("    )")
    return "\n".join(lines)


def _js_builder_fn_no_param(flow_key: str, proto_req: dict, grpc_req: str, db: "_SchemaDB", ts_mode: bool = False) -> str:
    """Return a JavaScript/TypeScript private builder function with no dynamic parameter (arg: none flows)."""
    fn_name = "_build" + "".join(w.title() for w in flow_key.split("_")) + "Request"
    # Expand oneof group names and filter invalid fields
    items = _expand_oneof_groups(proto_req, grpc_req, db)
    # Add return type annotation for TypeScript
    return_type = f": types.I{grpc_req}" if ts_mode else ""
    lines: list[str] = [f"function {fn_name}(){return_type} {{"]
    lines.append("    return {")
    for idx, (key, val) in enumerate(items):
        trailing  = "," if idx < len(items) - 1 else ""
        comment   = db.get_comment(grpc_req, key)
        child_msg = db.get_type(grpc_req, key)
        cmt_part  = f"  // {comment}" if comment else ""
        js_key    = _to_camel_ts(key) if ts_mode else _to_camel(key)
        if isinstance(val, dict):
            lines.append(f'        "{js_key}": {{{cmt_part}')
            lines.extend(_annotate_inline_lines(val, child_msg, db, indent=3, cmt="//", camel_keys=True, ts_mode=ts_mode))
            lines.append(f'        }}{trailing}')
        elif child_msg and db.is_wrapper(child_msg):
            lines.append(f'        "{js_key}": {{"value": {_json_scalar(val, js=True)}}}{trailing}{cmt_part}')
        elif isinstance(val, list) and val and isinstance(val[0], dict):
            lines.append(f'        "{js_key}": [{cmt_part}')
            for j, item in enumerate(val):
                item_trailing = "," if j < len(val) - 1 else ""
                lines.append(f'            {{')
                lines.extend(_annotate_inline_lines(item, child_msg, db, indent=4, cmt="//", camel_keys=True, ts_mode=ts_mode))
                lines.append(f'            }}{item_trailing}')
            lines.append(f'        ]{trailing}')
        elif child_msg and not isinstance(val, (dict, list)):
            if ts_mode and _is_proto_enum(child_msg) and isinstance(val, str):
                lines.append(f'        "{js_key}": {child_msg}.{val}{trailing}{cmt_part}')
            else:
                sfwk      = db.single_field_wrapper_key(child_msg)
                inner_key = (_to_camel_ts(sfwk) if ts_mode else _to_camel(sfwk)) if sfwk else None
                if inner_key:
                    lines.append(f'        "{js_key}": {{"{inner_key}": {{"value": {_json_scalar(val, js=True)}}}}}{trailing}{cmt_part}')
                else:
                    lines.append(f'        "{js_key}": {_json_scalar(val, js=True)}{trailing}{cmt_part}')
        else:
            if ts_mode and child_msg == "bytes" and isinstance(val, list):
                lines.append(f'        "{js_key}": new Uint8Array({json.dumps(val)}){trailing}{cmt_part}')
            else:
                lines.append(f'        "{js_key}": {_json_scalar(val, js=True)}{trailing}{cmt_part}')
    lines.append("    };")
    lines.append("}")
    return "\n".join(lines)


def _kt_builder_fn(flow_key: str, proto_req: dict, grpc_req: str, message_schemas: dict) -> str:
    """Return a Kotlin private builder function for the given flow."""
    param_name  = _FLOW_BUILDER_EXTRA_PARAM[flow_key][0]  # snake_case
    # Use a "Str" suffix to avoid shadowing the proto builder's own field with the same name
    # inside the apply { } block (e.g. captureMethod param vs builder.captureMethod field).
    kt_param    = _to_camel(param_name) + "Str"
    fn_name     = "build" + "".join(w.title() for w in flow_key.split("_")) + "Request"
    body_lines  = _kotlin_payload_lines(
        proto_req, grpc_req, message_schemas, indent=2,
        variable_fields=frozenset({param_name}),
        variable_field_values={param_name: kt_param},
    )
    body = "\n".join(body_lines)
    return (
        f"private fun {fn_name}({kt_param}: String): {grpc_req} {{\n"
        f"    return {grpc_req}.newBuilder().apply {{\n"
        f"{body}\n"
        f"    }}.build()\n"
        f"}}"
    )


# ── Public API: Consolidated scenario files ────────────────────────────────────

def render_consolidated_python(
    connector_name: str,
    flow_items: "list[tuple[str, dict, str]]",
    flow_metadata: dict[str, dict],
    message_schemas: dict,
    scenarios_with_payloads: "list[tuple[ScenarioSpec, dict[str, dict]]] | None" = None,
) -> str:
    """Return one Python file containing all flow functions (and scenario functions) for a connector."""
    db        = _SchemaDB(message_schemas)
    conn_enum = _conn_enum(connector_name)
    scenarios_with_payloads = scenarios_with_payloads or []

    # Merged imports — collect every service needed across scenarios AND flows
    all_service_names: list[str] = []
    for scenario, _ in scenarios_with_payloads:
        for fk in scenario.flows:
            svc = flow_metadata.get(fk, {}).get("service_name", "PaymentService")
            if svc not in all_service_names:
                all_service_names.append(svc)
    for fk, _, _ in (flow_items or []):
        svc = flow_metadata.get(fk, {}).get("service_name", "PaymentService")
        if svc not in all_service_names:
            all_service_names.append(svc)

    client_imports = "\n".join(
        f"from payments import {_client_class(svc)}" for svc in all_service_names
    )

    # Build one function block per scenario
    func_blocks: list[str] = []
    func_names:  list[str] = []

    # Build private builder functions for ALL supported gRPC flows — both standalone
    # flow_items AND flows that only appear inside multi-step scenarios (e.g. refund,
    # create_customer, tokenize, setup_recurring, recurring_charge).
    builder_fns:  list[str] = []
    has_builder:  set[str]  = set()

    # Pass 1: flows from flow_items (standalone flow examples).
    for flow_key, proto_req, _ in (flow_items or []):
        # Skip unsupported flows (not yet implemented in ConnectorClient)
        if flow_key in _UNSUPPORTED_FLOWS:
            continue
        grpc_req_b = flow_metadata.get(flow_key, {}).get("grpc_request", "")
        if not grpc_req_b:
            continue
        if flow_key in _FLOW_BUILDER_EXTRA_PARAM:
            builder_fns.append(_py_builder_fn(flow_key, proto_req, grpc_req_b, db))
        else:
            builder_fns.append(_py_builder_fn_no_param(flow_key, proto_req, grpc_req_b, db))
        has_builder.add(flow_key)

    # Pass 2: flows from scenarios not already covered by flow_items.
    for scenario, flow_payloads in scenarios_with_payloads:
        for fk in scenario.flows:
            if fk in has_builder:
                continue
            # Skip unsupported flows (not yet implemented in ConnectorClient)
            if fk in _UNSUPPORTED_FLOWS:
                continue
            grpc_req_b = flow_metadata.get(fk, {}).get("grpc_request", "")
            if not grpc_req_b:
                continue
            proto_req = flow_payloads.get(fk, {})
            if not proto_req:
                continue
            if fk in _FLOW_BUILDER_EXTRA_PARAM:
                builder_fns.append(_py_builder_fn(fk, proto_req, grpc_req_b, db))
            else:
                builder_fns.append(_py_builder_fn_no_param(fk, proto_req, grpc_req_b, db))
            has_builder.add(fk)

    def _py_step_with_builder(scenario_key: str, flow_key: str, step_num: int,
                               payload: dict, client_var: str, method: str,
                               var_name: str) -> list[str]:
        """Return Python body lines for a scenario step using a pre-built builder fn."""
        pad  = "    "
        desc = _STEP_DESCRIPTIONS.get(flow_key, flow_key)
        slines: list[str] = [f"{pad}# Step {step_num}: {desc}"]

        if flow_key == "authorize" and scenario_key in _CARD_AUTHORIZE_SCENARIOS:
            cm = {"checkout_card": "MANUAL", "void_payment": "MANUAL",
                  "get_payment": "MANUAL", "refund": "AUTOMATIC"}.get(scenario_key, "AUTOMATIC")
            call_arg = f'"{cm}"'
        else:
            call_arg = "authorize_response.connector_transaction_id"

        slines.append(f"{pad}{var_name} = await {client_var}.{method}(_build_{flow_key}_request({call_arg}))")
        slines.append("")

        if flow_key == "authorize":
            slines.append(f'{pad}if {var_name}.status == "FAILED":')
            slines.append(f'{pad}    raise RuntimeError(f"Payment failed: {{{var_name}.error}}")')
            slines.append(f'{pad}if {var_name}.status == "PENDING":')
            slines.append(f'{pad}    # Awaiting async confirmation — handle via webhook')
            slines.append(f'{pad}    return {{"status": "pending", "transaction_id": {var_name}.connector_transaction_id}}')
            slines.append("")
        elif flow_key == "setup_recurring":
            slines.append(f'{pad}if {var_name}.status == "FAILED":')
            slines.append(f'{pad}    raise RuntimeError(f"Recurring setup failed: {{{var_name}.error}}")')
            slines.append("")
        elif flow_key in ("capture", "refund", "recurring_charge"):
            _fk_title = flow_key.replace("_", " ").title()
            slines.append(f'{pad}if {var_name}.status == "FAILED":')
            slines.append(f'{pad}    raise RuntimeError(f"{_fk_title} failed: {{{var_name}.error}}")')
            slines.append("")

        return slines

    for scenario, flow_payloads in scenarios_with_payloads:
        func_name = f"process_{scenario.key}"
        func_names.append(func_name)

        svc_names: list[str] = []
        for fk in scenario.flows:
            svc = flow_metadata.get(fk, {}).get("service_name", "PaymentService")
            if svc not in svc_names:
                svc_names.append(svc)

        body_lines: list[str] = []
        for svc in svc_names:
            cls = _client_class(svc)
            var = cls.lower().replace("client", "_client")
            body_lines.append(f"    {var} = {cls}(config)")
        body_lines.append("")

        for step_num, flow_key in enumerate(scenario.flows, 1):
            meta       = flow_metadata.get(flow_key, {})
            svc        = meta.get("service_name", "PaymentService")
            grpc_req   = meta.get("grpc_request", "")
            client_var = _client_class(svc).lower().replace("client", "_client")
            method     = _FLOW_KEY_TO_METHOD.get(flow_key, flow_key)
            var_name   = _FLOW_VAR_NAME.get(flow_key, f"{flow_key.split('_')[0]}_response")

            payload = dict(flow_payloads.get(flow_key, {}))
            if flow_key == "authorize":
                if scenario.key in ("checkout_card", "void_payment", "get_payment"):
                    payload["capture_method"] = "MANUAL"
                elif scenario.key == "refund":
                    payload["capture_method"] = "AUTOMATIC"

            if flow_key in has_builder and scenario.key in _CARD_AUTHORIZE_SCENARIOS | {"checkout_card", "refund", "void_payment", "get_payment"}:
                body_lines.extend(_py_step_with_builder(
                    scenario.key, flow_key, step_num, payload, client_var, method, var_name
                ))
            else:
                body_lines.extend(_scenario_step_python(
                    scenario.key, flow_key, step_num, payload, grpc_req, client_var, db
                ))

        body_lines.append(_scenario_return_python(scenario))
        body = "\n".join(body_lines)

        func_blocks.append(
            f"async def {func_name}(merchant_transaction_id: str, "
            f"config: sdk_config_pb2.ConnectorConfig = _default_config):\n"
            f'    """{scenario.title}\n\n'
            f"    {scenario.description}\n"
            f'    """\n'
            f"{body}\n"
        )

    # Append individual flow functions (flows not already covered by a scenario)
    for flow_key, proto_req, pm_label in (flow_items or []):
        # Skip unsupported flows (not yet implemented in ConnectorClient)
        if flow_key in _UNSUPPORTED_FLOWS:
            continue
        meta       = flow_metadata.get(flow_key, {})
        svc        = meta.get("service_name", "PaymentService")
        grpc_req   = meta.get("grpc_request", "")
        rpc_name   = meta.get("rpc_name", flow_key)
        client_cls = _client_class(svc)
        client_var = client_cls.lower().replace("client", "_client")
        method     = _FLOW_KEY_TO_METHOD.get(flow_key, flow_key)
        resp_var   = f"{flow_key.split('_')[0]}_response"
        pm_part    = f" ({pm_label})" if pm_label else ""

        body_lines: list[str] = [f"    {client_var} = {client_cls}(config)", ""]
        if flow_key in has_builder:
            if flow_key in _FLOW_BUILDER_EXTRA_PARAM:
                param_name  = _FLOW_BUILDER_EXTRA_PARAM[flow_key][0]
                default_val = proto_req.get(param_name, "AUTOMATIC" if param_name == "capture_method" else "probe_connector_txn_001")
                body_lines.append(f'    {resp_var} = await {client_var}.{method}(_build_{flow_key}_request("{default_val}"))')
            else:
                body_lines.append(f'    {resp_var} = await {client_var}.{method}(_build_{flow_key}_request())')
            body_lines.append("")
        else:
            body_lines.extend(_scenario_step_python("_standalone_", flow_key, 1, proto_req, grpc_req, client_var, db))
        if flow_key == "authorize":
            body_lines.append(f'    return {{"status": {resp_var}.status, "transaction_id": {resp_var}.connector_transaction_id}}')
        elif flow_key == "setup_recurring":
            body_lines.append(f'    return {{"status": {resp_var}.status, "mandate_id": {resp_var}.connector_transaction_id}}')
        else:
            body_lines.append(f'    return {{"status": {resp_var}.status}}')

        body = "\n".join(body_lines)
        process_fn_name = f"process_{flow_key}"
        func_names.append(process_fn_name)
        func_blocks.append(
            f"async def {process_fn_name}(merchant_transaction_id: str, "
            f"config: sdk_config_pb2.ConnectorConfig = _default_config):\n"
            f'    """Flow: {svc}.{rpc_name}{pm_part}"""\n'
            f"{body}\n"
        )

    builders_text  = "\n\n".join(builder_fns)
    builders_section = f"\n\n{builders_text}\n" if builder_fns else ""
    functions_text = "\n\n".join(func_blocks)
    first_scenario = func_names[0][8:] if func_names and func_names[0].startswith("process_") else func_names[0] if func_names else "checkout_autocapture"
    supported_flows_list = json.dumps([fk for fk, _, _ in (flow_items or []) if fk not in _UNSUPPORTED_FLOWS])

    return f"""\
# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py {connector_name}
#
# {connector_name.title()} — all integration scenarios and flows in one file.
# Run a scenario:  python3 {connector_name}.py checkout_card

import asyncio
import sys
{client_imports}
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

SUPPORTED_FLOWS = {supported_flows_list}

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
{_generate_connector_config_python(connector_name)}
)


{builders_section}{functions_text}
if __name__ == "__main__":
    scenario = sys.argv[1] if len(sys.argv) > 1 else "{first_scenario}"
    fn = globals().get(f"process_{{scenario}}")
    if not fn:
        available = [k[8:] for k in globals() if k.startswith("process_")]
        print(f"Unknown scenario: {{scenario}}. Available: {{available}}", file=sys.stderr)
        sys.exit(1)
    asyncio.run(fn("order_001"))
"""


def render_consolidated_javascript(
    connector_name: str,
    flow_items: "list[tuple[str, dict, str]]",
    flow_metadata: dict[str, dict],
    message_schemas: dict,
    scenarios_with_payloads: "list[tuple[ScenarioSpec, dict[str, dict]]] | None" = None,
) -> str:
    """Return one JavaScript/TypeScript file containing all flow functions (and scenario functions) for a connector."""
    db        = _SchemaDB(message_schemas)
    conn_enum = _conn_enum(connector_name)
    scenarios_with_payloads = scenarios_with_payloads or []

    # Merged imports — scenarios + flows
    all_service_names: list[str] = []
    for scenario, _ in scenarios_with_payloads:
        for fk in scenario.flows:
            svc = flow_metadata.get(fk, {}).get("service_name", "PaymentService")
            if svc not in all_service_names:
                all_service_names.append(svc)
    for fk, _, _ in (flow_items or []):
        svc = flow_metadata.get(fk, {}).get("service_name", "PaymentService")
        if svc not in all_service_names:
            all_service_names.append(svc)

    client_imports = ", ".join(_client_class(svc) for svc in all_service_names)

    # Collect enum types used in payload fields (for Currency.USD, CaptureMethod.MANUAL, etc.)
    ts_enum_types: set[str] = set()
    for scenario, flow_payloads in scenarios_with_payloads:
        for fk in scenario.flows:
            payload = dict(flow_payloads.get(fk, {}))
            if fk == "authorize":
                if scenario.key in ("checkout_card", "void_payment", "get_payment"):
                    payload["capture_method"] = "MANUAL"
                elif scenario.key == "refund":
                    payload["capture_method"] = "AUTOMATIC"
            grpc_req = flow_metadata.get(fk, {}).get("grpc_request", "")
            ts_enum_types.update(_collect_ts_enum_types(payload, grpc_req, db))
    for flow_key, proto_req, _ in (flow_items or []):
        grpc_req = flow_metadata.get(flow_key, {}).get("grpc_request", "")
        ts_enum_types.update(_collect_ts_enum_types(proto_req, grpc_req, db))
    
    # Build enum imports string (only enums and values that exist at runtime)
    enum_imports = ", ".join(sorted(ts_enum_types)) if ts_enum_types else ""
    types_imports = "Environment"
    if enum_imports:
        types_imports += f", {enum_imports}"

    # Build one function block per scenario
    func_blocks:   list[str] = []
    func_names_js: list[str] = []

    # Build private builder functions (one per gRPC flow with a supported proto_request).
    # We need builders for ALL supported gRPC flows — both those in flow_items AND those
    # that appear only in multi-step scenarios (e.g. refund, create_customer, tokenize).
    # Builders let the gRPC smoke test call connector code directly without grpc_* wrappers.
    js_builder_fns: list[str] = []
    js_has_builder: set[str]  = set()

    # Pass 1: flows from flow_items (standalone flow examples).
    for flow_key, proto_req, _ in (flow_items or []):
        grpc_req_b = flow_metadata.get(flow_key, {}).get("grpc_request", "")
        if not grpc_req_b:
            continue
        if flow_key in _FLOW_BUILDER_EXTRA_PARAM:
            js_builder_fns.append(_js_builder_fn(flow_key, proto_req, grpc_req_b, db, ts_mode=True))
        else:
            js_builder_fns.append(_js_builder_fn_no_param(flow_key, proto_req, grpc_req_b, db, ts_mode=True))
        js_has_builder.add(flow_key)

    # Pass 2: flows from scenarios that don't have a standalone flow item.
    # This ensures refund, create_customer, tokenize, setup_recurring, recurring_charge, etc.
    # all get builder functions even when they only appear inside multi-step scenarios.
    for scenario, flow_payloads in scenarios_with_payloads:
        for fk in scenario.flows:
            if fk in js_has_builder:
                continue
            grpc_req_b = flow_metadata.get(fk, {}).get("grpc_request", "")
            if not grpc_req_b:
                continue
            proto_req = flow_payloads.get(fk, {})
            if not proto_req:
                continue
            if fk in _FLOW_BUILDER_EXTRA_PARAM:
                js_builder_fns.append(_js_builder_fn(fk, proto_req, grpc_req_b, db, ts_mode=True))
            else:
                js_builder_fns.append(_js_builder_fn_no_param(fk, proto_req, grpc_req_b, db, ts_mode=True))
            js_has_builder.add(fk)

    _js_var_defaults = {k: _to_camel(v.replace("_response", "Response")) for k, v in _FLOW_VAR_NAME.items()}

    def _js_step_with_builder(scenario_key: str, flow_key: str, step_num: int,
                               client_var: str) -> list[str]:
        """Return JS body lines for a scenario step using a pre-built builder fn."""
        method   = _to_camel(_FLOW_KEY_TO_METHOD.get(flow_key, flow_key))
        var_name = _js_var_defaults.get(flow_key, f"{flow_key.split('_')[0]}Response")
        desc     = _STEP_DESCRIPTIONS.get(flow_key, flow_key)
        fn_name  = "_build" + "".join(w.title() for w in flow_key.split("_")) + "Request"
        grpc_req = flow_metadata.get(flow_key, {}).get("grpc_request", "")

        # Get the dynamic parameter for this flow (if any) from _FLOW_BUILDER_EXTRA_PARAM
        if flow_key in _FLOW_BUILDER_EXTRA_PARAM:
            param_name = _FLOW_BUILDER_EXTRA_PARAM[flow_key][0]
            # Look up the proto type for this parameter GENERICALLY
            proto_type = _PROTO_FIELD_TYPES.get(grpc_req, {}).get(param_name, "")
            
            if flow_key == "authorize" and scenario_key in _CARD_AUTHORIZE_SCENARIOS:
                # Scenario-specific capture method values - this is business logic, not type info
                cm = {"checkout_card": "MANUAL", "void_payment": "MANUAL",
                      "get_payment": "MANUAL", "refund": "AUTOMATIC"}.get(scenario_key, "AUTOMATIC")
                # Use the actual proto enum type name, not hardcoded "CaptureMethod"
                if proto_type and _is_proto_enum(proto_type):
                    call_arg = f"{proto_type}.{cm}"
                else:
                    call_arg = f"'{cm}'"
            else:
                call_arg = "authorizeResponse.connectorTransactionId!"
        else:
            call_arg = ""

        slines: list[str] = [
            f"    // Step {step_num}: {desc}",
            f"    const {var_name} = await {client_var}.{method}({fn_name}({call_arg}));",
            "",
        ]
        if flow_key == "authorize":
            slines += [
                f"    if ({var_name}.status === types.PaymentStatus.FAILURE) {{",
                f"        throw new Error(`Payment failed: ${{JSON.stringify({var_name}.error)}}`);",
                "    }",
                f"    if ({var_name}.status === types.PaymentStatus.PENDING) {{",
                "        // Awaiting async confirmation — handle via webhook",
                f"        return {{ status: 'pending', connectorTransactionId: {var_name}.connectorTransactionId }};",
                "    }",
                "",
            ]
        elif flow_key == "setup_recurring":
            slines += [
                f"    if ({var_name}.status === types.PaymentStatus.FAILURE) {{",
                f"        throw new Error(`Recurring setup failed: ${{JSON.stringify({var_name}.error)}}`);",
                "    }",
                "",
            ]
        elif flow_key == "refund":
            slines += [
                f"    if ({var_name}.status === types.RefundStatus.REFUND_FAILURE) {{",
                f"        throw new Error(`Refund failed: ${{JSON.stringify({var_name}.error)}}`);",
                "    }",
                "",
            ]
        elif flow_key in ("capture", "recurring_charge"):
            slines += [
                f"    if ({var_name}.status === types.PaymentStatus.FAILURE) {{",
                f"        throw new Error(`{flow_key.replace('_', ' ').title()} failed: ${{JSON.stringify({var_name}.error)}}`);",
                "    }",
                "",
            ]
        return slines

    for scenario, flow_payloads in scenarios_with_payloads:
        func_name = _to_camel(f"process_{scenario.key}")
        func_names_js.append(func_name)

        svc_names: list[str] = []
        for fk in scenario.flows:
            svc = flow_metadata.get(fk, {}).get("service_name", "PaymentService")
            if svc not in svc_names:
                svc_names.append(svc)

        body_lines: list[str] = []
        for svc in svc_names:
            cls = _client_class(svc)
            var = cls[0].lower() + cls[1:]
            body_lines.append(f"    const {var} = new {cls}(config);")
        body_lines.append("")

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

            if flow_key in js_has_builder and scenario.key in _CARD_AUTHORIZE_SCENARIOS | {"checkout_card", "refund", "void_payment", "get_payment"}:
                body_lines.extend(_js_step_with_builder(scenario.key, flow_key, step_num, client_var))
            else:
                body_lines.extend(_scenario_step_javascript(
                    scenario.key, flow_key, step_num, payload, grpc_req, db, client_var
                ))

        body_lines.append(_scenario_return_javascript(scenario))
        body = "\n".join(body_lines)
        
        # Determine return type GENERICALLY from the last flow's response type
        # The return type should be based on the primary flow response (last step or main flow)
        last_flow_key = scenario.flows[-1] if scenario.flows else ""
        last_flow_meta = flow_metadata.get(last_flow_key, {})
        grpc_response = last_flow_meta.get("grpc_response", "")
        if grpc_response:
            scenario_return_type = f"Promise<types.I{grpc_response}>"
        else:
            scenario_return_type = "Promise<any>"

        func_blocks.append(
            f"// {scenario.title}\n"
            f"// {scenario.description}\n"
            + f"async function {func_name}(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {{\n"
            + f"{body}\n"
            + "}"
        )

    # Append individual flow functions
    _JS_RESERVED = JS_RESERVED
    for flow_key, proto_req, pm_label in (flow_items or []):
        meta       = flow_metadata.get(flow_key, {})
        svc        = meta.get("service_name", "PaymentService")
        grpc_req   = meta.get("grpc_request", "")
        rpc_name   = meta.get("rpc_name", flow_key)
        cls        = _client_class(svc)
        client_var = cls[0].lower() + cls[1:]
        func_name  = _to_camel(flow_key) if flow_key not in _JS_RESERVED else f"{flow_key}Payment"
        var_name   = f"{flow_key.split('_')[0]}Response"
        pm_part    = f" ({pm_label})" if pm_label else ""

        if flow_key in js_has_builder:
            fn_name = "_build" + "".join(w.title() for w in flow_key.split("_")) + "Request"
            method  = _to_camel(_FLOW_KEY_TO_METHOD.get(flow_key, flow_key))
            if flow_key in _FLOW_BUILDER_EXTRA_PARAM:
                param_name  = _FLOW_BUILDER_EXTRA_PARAM[flow_key][0]
                default_val = proto_req.get(param_name, "AUTOMATIC" if param_name == "capture_method" else "probe_connector_txn_001")
                if param_name == "capture_method":
                    call_expr = f"{fn_name}(CaptureMethod.{default_val})"
                else:
                    call_expr = f"{fn_name}('{default_val}')"
            else:
                call_expr   = f"{fn_name}()"
            body_lines  = [
                f"    const {client_var} = new {cls}(config);",
                "",
                f"    const {var_name} = await {client_var}.{method}({call_expr});",
                "",
            ]
        else:
            body_lines = list(_scenario_step_javascript("_standalone_", flow_key, 1, proto_req, grpc_req, db, client_var, ts_mode=True))
        # These standalone flow functions return the raw response to avoid type issues
        # with responses that don't have a status field (e.g., EventServiceHandleResponse)
        body_lines.append(f"    return {var_name};")

        body = "\n".join(body_lines)
        func_names_js.append(func_name)
        # Return the actual response object for type compatibility
        func_blocks.append(
            f"// Flow: {svc}.{rpc_name}{pm_part}\n"
            + f"async function {func_name}(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {{\n"
            + f"{body}\n"
            + "}"
        )

    # Export both process* functions and _build*Request helpers
    export_items = func_names_js + [
        "_build" + "".join(w.title() for w in fk.split("_")) + "Request"
        for fk in sorted(js_has_builder)
    ]
    exports = ", ".join(export_items)
    js_builders_text = "\n\n".join(js_builder_fns)
    js_builders_section = f"\n\n{js_builders_text}\n" if js_builder_fns else ""
    funcs_text       = "\n\n".join(func_blocks)
    first_scenario   = scenarios_with_payloads[0][0].key if scenarios_with_payloads else "checkout_autocapture"
    supported_flows_js = json.dumps([fk for fk, _, _ in (flow_items or []) if fk not in _UNSUPPORTED_FLOWS])

    return f"""\
// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py {connector_name}
//
// {connector_name.title()} — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx {connector_name}.ts {first_scenario}

import {{ {client_imports}, types }} from 'hyperswitch-prism';
const {{ {types_imports} }} = types;
export const SUPPORTED_FLOWS = {supported_flows_js};

const _defaultConfig: types.IConnectorConfig = {{
    options: {{
        environment: Environment.SANDBOX,
    }},
{_generate_connector_config_typescript(connector_name)}
}};
{js_builders_section}

// ANCHOR: scenario_functions
{funcs_text}


// Export all process* functions for the smoke test
export {{
    {exports}
}};

// CLI runner
if (require.main === module) {{
    const scenario = process.argv[2] || '{first_scenario}';
    const key = 'process' + scenario.replace(/_([a-z])/g, (_, l) => l.toUpperCase()).replace(/^(.)/, c => c.toUpperCase());
    const fn = (globalThis as any)[key] || (exports as any)[key];
    if (!fn) {{
        const available = Object.keys(exports).map(k =>
            k.replace(/^process/, '').replace(/([A-Z])/g, '_$1').toLowerCase().replace(/^_/, '')
        );
        console.error(`Unknown scenario: ${{scenario}}. Available: ${{available.join(', ')}}`);
        process.exit(1);
    }}
    fn('order_001').catch(console.error);
}}
"""


# ── Public API: Config section ─────────────────────────────────────────────────

def render_config_section(connector_name: str) -> list[str]:
    """
    Return markdown lines for the SDK Configuration section (once per connector doc).
    """
    cells = [
        _td("Python",     "python",     _config_python(connector_name)),
        _td("JavaScript", "javascript", _config_javascript(connector_name)),
        _td("Kotlin",     "kotlin",     _config_kotlin(connector_name)),
        _td("Rust",       "rust",       _config_rust(connector_name)),
    ]

    header_row = "<tr>" + "".join(
        f"<td><b>{label}</b></td>"
        for label in ("Python", "JavaScript", "Kotlin", "Rust")
    ) + "</tr>"

    return [
        "## SDK Configuration",
        "",
        "Use this config for all flows in this connector. "
        "Replace `YOUR_API_KEY` with your actual credentials.",
        "",
        "<table>",
        header_row,
        "<tr>",
        "\n".join(cells),
        "</tr>",
        "</table>",
        "",
    ]


# ── Public API: Scenario section ───────────────────────────────────────────────

def render_scenario_section(
    scenario: ScenarioSpec,
    connector_name: str,
    flow_payloads: dict[str, dict],
    flow_metadata: dict[str, dict],
    message_schemas: dict,
    ann_scenario: dict,
    line_numbers: dict[str, int] | None = None,
) -> list[str]:
    """
    Return markdown lines for one scenario subsection inside ## Integration Scenarios.
    Emits links to the pre-generated example files instead of embedding code.
    """
    title      = ann_scenario.get("title", scenario.title)
    description = ann_scenario.get("description", scenario.description)
    status_hdl  = ann_scenario.get("status_handling", scenario.status_handling)

    out: list[str] = []
    a = out.append

    a(f"### {title}")
    a("")
    a(description)
    a("")

    if status_hdl:
        a("**Response status handling:**")
        a("")
        a("| Status | Recommended action |")
        a("|--------|-------------------|")
        for status, action in status_hdl.items():
            a(f"| `{status}` | {action} |")
        a("")

    # Link to example files with line numbers if available
    scenario_key = scenario.key
    camel_scenario = "".join(w.capitalize() for w in scenario_key.split("_"))
    base_py = f"../../examples/{connector_name}/{connector_name}.py"
    base_js = f"../../examples/{connector_name}/{connector_name}.js"
    base_kt = f"../../examples/{connector_name}/{connector_name}.kt"
    base_rs = f"../../examples/{connector_name}/{connector_name}.rs"
    
    # Get line numbers from the generated files
    ln_py = line_numbers.get("python", 0) if line_numbers else 0
    ln_js = line_numbers.get("javascript", 0) if line_numbers else 0
    ln_kt = line_numbers.get("kotlin", 0) if line_numbers else 0
    ln_rs = line_numbers.get("rust", 0) if line_numbers else 0
    
    # Build links with line numbers when available
    py_link = f"{base_py}#L{ln_py}" if ln_py else base_py
    js_link = f"{base_js}#L{ln_js}" if ln_js else base_js
    kt_link = f"{base_kt}#L{ln_kt}" if ln_kt else base_kt
    rs_link = f"{base_rs}#L{ln_rs}" if ln_rs else base_rs
    
    a(f"**Examples:** [Python]({py_link}) · [JavaScript]({js_link}) · [Kotlin]({kt_link}) · [Rust]({rs_link})")
    a("")

    return out


# Per-flow status block for flows that don't have a PaymentStatus status field.
_KT_FLOW_STATUS_BLOCK: dict[str, str] = {
    "tokenize":                             '    println("Token: ${response.paymentMethodToken}")',
    "create_customer":                      '    println("Customer: ${response.connectorCustomerId}")',
    "dispute_accept":                       '    println("Dispute status: ${response.disputeStatus.name}")',
    "dispute_defend":                       '    println("Dispute status: ${response.disputeStatus.name}")',
    "dispute_submit_evidence":              '    println("Dispute status: ${response.disputeStatus.name}")',
    "create_access_token":                              '    println("Access token obtained (statusCode=${response.statusCode})")',
    "create_session_token":                             '    println("Session token obtained (statusCode=${response.statusCode})")',
    "create_server_authentication_token":               '    println("StatusCode: ${response.statusCode}")',
    "create_server_session_authentication_token":       '    println("Session token: ${response.sessionToken} (statusCode=${response.statusCode})")',
    "create_client_authentication_token":               '    println("StatusCode: ${response.statusCode}")',
    "create_order":                         '    println("Order: ${response.connectorOrderId}")',
    # EventServiceHandleResponse has event_status, not status
    "handle_event":                         '    println("Event status: ${response.eventStatus.name}")',
    # VerifyRedirectResponseResponse has no status field — report source_verified
    "verify_redirect":                      '    println("Source verified: ${response.sourceVerified}")',
}


def _preprocess_kt_payload(flow_key: str, proto_req: dict) -> dict:
    """Fix probe data encodings that are invalid for Kotlin builder generation.

    Specifically, oneof group names (e.g. mandate_id_type in MandateReference) appear as
    extra nesting levels in probe JSON but have no corresponding builder in the Kotlin API.
    """
    if flow_key == "recurring_charge" and "connector_recurring_payment_id" in proto_req:
        cri = proto_req["connector_recurring_payment_id"]
        if isinstance(cri, dict) and "mandate_id_type" in cri:
            mit = cri["mandate_id_type"]
            if isinstance(mit, dict) and "connector_mandate_id" in mit:
                # Rewrite: { mandate_id_type: { connector_mandate_id: val } }
                # →        { connector_mandate_id: { connector_mandate_id: val } }
                processed = dict(proto_req)
                processed["connector_recurring_payment_id"] = {
                    "connector_mandate_id": {
                        "connector_mandate_id": mit["connector_mandate_id"],
                    }
                }
                return processed
    return proto_req


# Proto primitive type names — fields of these types use direct Kotlin assignment
_KOTLIN_PRIMITIVES = frozenset({
    "string", "bool", "int32", "int64", "uint32", "uint64",
    "float", "double", "bytes", "sint32", "sint64",
    "fixed32", "fixed64", "sfixed32", "sfixed64",
})


def _kotlin_collect_enum_types(obj: dict, msg_name: str, db: "_SchemaDB") -> set[str]:
    """Recursively collect enum type names used in a payload dict (for import generation)."""
    types: set[str] = set()
    for key, val in obj.items():
        child_msg = db.get_type(msg_name, key)
        if isinstance(val, dict):
            types.update(_kotlin_collect_enum_types(val, child_msg or "", db))
        elif isinstance(val, str) and child_msg:
            if (not db.is_wrapper(child_msg)
                    and child_msg not in _KOTLIN_PRIMITIVES
                    and child_msg not in _PROTO_FIELD_TYPES):
                types.add(child_msg)
    return types


def _to_snake(name: str) -> str:
    """Convert camelCase to snake_case for proto field name lookups.
    
    Handles complex cases like:
    - enrolledFor3Ds -> enrolled_for_3ds (handle numbers in camelCase)
    - enrolledFor_3ds -> enrolled_for_3ds (normalize existing underscores)
    """
    # Handle numbers: insert underscore before digits when preceded by a letter
    # This handles cases like "For3Ds" -> "For_3Ds" (will become "for_3ds" after lower())
    step1 = re.sub(r'([a-z])(\d)', r'\1_\2', name)
    
    # Handle uppercase letters: insert underscore before them (except first char)
    step2 = re.sub(r'(?<!^)(?=[A-Z])', '_', step1).lower()
    
    return step2


def _kotlin_payload_lines(
    obj: dict,
    msg_name: str,
    message_schemas: dict,
    indent: int,
    variable_fields: frozenset = frozenset(),
    variable_field_values: dict | None = None,
) -> list[str]:
    """Recursively build Kotlin builder apply-block lines for a proto payload dict.

    variable_field_values: maps proto field name → Kotlin expression to use on the RHS
    instead of the field name itself (avoids shadowing when param == field name).
    """
    pad    = "    " * indent
    db     = _SchemaDB(message_schemas)
    lines: list[str] = []

    for key, val in obj.items():
        camel     = _to_camel(key)
        comment   = db.get_comment(msg_name, key)
        child_msg = db.get_type(msg_name, key)
        # If lookup fails and key is camelCase, try snake_case fallback for proto schemas
        if not child_msg:
            snake_key = _to_snake(key)
            child_msg = db.get_type(msg_name, snake_key)
        cmt_part  = f"  // {comment}" if comment else ""

        if key in variable_fields:
            # RHS expression: use the overridden name if provided, else fall back to camel
            rhs = (variable_field_values or {}).get(key, camel)
            # Emit variable reference instead of literal value
            if child_msg and child_msg not in _KOTLIN_PRIMITIVES and child_msg not in _PROTO_FIELD_TYPES:
                # Enum: CaptureMethod.valueOf(captureMethodStr)
                lines.append(f'{pad}{camel} = {child_msg}.valueOf({rhs}){cmt_part}')
            else:
                # String/plain: connectorTransactionId = txnId
                lines.append(f'{pad}{camel} = {rhs}{cmt_part}')
            continue

        if isinstance(val, dict):
            # Check if this is a map type
            is_map = child_msg and child_msg.startswith("map<")
            
            # Also check from _PROTO_FIELD_TYPES (handles cases where message_schemas 
            # returns synthetic entry type name instead of map<>)
            proto_type = _PROTO_FIELD_TYPES.get(msg_name, {}).get(key, "")
            if proto_type.startswith("map<"):
                is_map = True
            
            if not child_msg and not is_map:
                # Unknown field — likely a oneof group name (e.g. mandate_id_type).
                # Flatten by processing inner fields at the current message level.
                lines.extend(_kotlin_payload_lines(val, msg_name, message_schemas, indent, variable_fields, variable_field_values))
            elif is_map:
                # Map type — use putAll() with mapOf() for Kotlin protobuf builders
                map_entries = []
                for k, v in val.items():
                    map_entries.append(f'{json.dumps(k)} to {json.dumps(v)}')
                lines.append(f"{pad}putAll{camel.capitalize()}(mapOf({', '.join(map_entries)})){cmt_part}")
            else:
                lines.append(f"{pad}{camel}Builder.apply {{{cmt_part}")
                lines.extend(_kotlin_payload_lines(val, child_msg, message_schemas, indent + 1, variable_fields, variable_field_values))
                lines.append(f"{pad}}}")
        elif isinstance(val, bool):
            lines.append(f"{pad}{camel} = {str(val).lower()}{cmt_part}")
        elif isinstance(val, int):
            # Use Long (L) suffix only for 64-bit proto types; 32-bit types use plain Int
            _INT32_TYPES = frozenset({"int32", "uint32", "sint32", "fixed32", "sfixed32"})
            if child_msg and child_msg in _INT32_TYPES:
                lines.append(f"{pad}{camel} = {val}{cmt_part}")
            else:
                lines.append(f"{pad}{camel} = {val}L{cmt_part}")
        elif isinstance(val, float):
            lines.append(f"{pad}{camel} = {val}{cmt_part}")
        elif isinstance(val, str):
            if child_msg and db.is_wrapper(child_msg):
                # Wrapper message (e.g. SecretString) — uses Builder.value
                lines.append(f'{pad}{camel}Builder.value = {json.dumps(val)}{cmt_part}')
            elif child_msg and child_msg not in _KOTLIN_PRIMITIVES and child_msg not in _PROTO_FIELD_TYPES:
                # Enum type — use EnumType.ENUM_VALUE format
                lines.append(f'{pad}{camel} = {child_msg}.{val}{cmt_part}')
            elif child_msg and child_msg not in _KOTLIN_PRIMITIVES:
                # Message type with a string scalar — check single_field_wrapper pattern
                sfwk = db.single_field_wrapper_key(child_msg)
                if sfwk:
                    inner_camel = _to_camel(sfwk)
                    lines.append(f'{pad}{camel}Builder.apply {{{cmt_part}')
                    lines.append(f'{pad}    {inner_camel}Builder.value = {json.dumps(val)}')
                    lines.append(f'{pad}}}')
                else:
                    lines.append(f'{pad}{camel} = {json.dumps(val)}{cmt_part}')
            else:
                # Plain string field — direct assignment
                lines.append(f'{pad}{camel} = {json.dumps(val)}{cmt_part}')
        else:
            lines.append(f"{pad}// {camel}: {json.dumps(val)}{cmt_part}")

    return lines


# Rust reserved keywords that may collide with proto field names.
_RUST_KEYWORDS = frozenset({
    "as", "break", "const", "continue", "crate", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod",
    "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct",
    "super", "trait", "true", "type", "unsafe", "use", "where", "while",
})


def _rust_field(key: str) -> str:
    """Return the Rust field name, prefixing reserved keywords with r#."""
    return f"r#{key}" if key in _RUST_KEYWORDS else key


def _rust_struct_lines(
    obj: dict,
    msg_name: str,
    message_schemas: dict,
    indent: int,
    variable_fields: frozenset = frozenset(),
) -> list[str]:
    """Recursively build Rust struct literal field-assignment lines.

    Generates correct prost-style struct literals with:
    - ``Some(T)`` wrapping for optional-in-prost fields
      (all message types are Option<T>; non-message fields follow the
      ``optional`` keyword tracked in ``_PROTO_OPTIONAL_FIELDS``)
    - ``EnumType::from_str_name("VAL").unwrap_or_default().into()`` for enums
    - ``Secret::new("val".to_string())`` for SecretString / wrapper types
    - Sub-module qualified enum variants for oneof wrappers
      e.g. ``payment_method::PaymentMethod::Card(CardDetails { ... })``
    - Variable fields (e.g. ``capture_method: &str``) as type-aware references
    """
    pad = "    " * indent
    db  = _SchemaDB(message_schemas)
    lines: list[str] = []
    msg_optional = _PROTO_OPTIONAL_FIELDS.get(msg_name, set())
    msg_map_fields = _PROTO_MAP_FIELDS.get(msg_name, set())

    for key, val in obj.items():
        comment   = db.get_comment(msg_name, key)
        child_msg = db.get_type(msg_name, key)
        cmt_part  = f"  // {comment}" if comment else ""
        field     = _rust_field(key)

        # Determine whether this field is Option<T> in prost:
        # - All message types (including wrapper types) are always Option<T>
        # - Non-message fields are Option<T> only when marked `optional` in proto3
        # - Map fields are NEVER Option<T> (they default to empty HashMap)
        is_map = key in msg_map_fields
        is_msg = bool(child_msg and (
            child_msg in _PROTO_FIELD_TYPES or child_msg in _PROTO_WRAPPER_TYPES
        )) and not is_map
        is_opt = is_msg or (key in msg_optional)

        def wrap(expr: str) -> str:
            return f"Some({expr})" if is_opt else expr

        # Variable field (function parameter) — emit type-aware expression
        if key in variable_fields:
            if child_msg and _is_proto_enum(child_msg):
                expr = f"{child_msg}::from_str_name({key}).unwrap_or_default().into()"
            elif child_msg and child_msg in _RUST_WRAPPER_CONSTRUCTORS:
                # Special Rust wrapper type with custom constructor (e.g., CardNumberType)
                template, _, _ = _RUST_WRAPPER_CONSTRUCTORS[child_msg]
                expr = template.format(val=key)
            elif child_msg and child_msg in _PROTO_WRAPPER_TYPES:
                expr = f"Secret::new({key}.to_string())"
            else:
                expr = f"{key}.to_string()"
            lines.append(f"{pad}{field}: {wrap(expr)},{cmt_part}")
            continue

        if isinstance(val, dict):
            if child_msg and child_msg in _RUST_WRAPPER_CONSTRUCTORS:
                # Special Rust wrapper type with custom constructor
                template, _, _ = _RUST_WRAPPER_CONSTRUCTORS[child_msg]
                inner_val = val.get("value", "")
                expr = template.format(val=json.dumps(inner_val))
                lines.append(f"{pad}{field}: {wrap(expr)},{cmt_part}")

            elif child_msg and child_msg in _PROTO_WRAPPER_TYPES:
                # SecretString: extract the inner value from probe's {"value": "..."} dict
                inner_val = val.get("value", "")
                expr = f"Secret::new({json.dumps(inner_val)}.to_string())"
                lines.append(f"{pad}{field}: {wrap(expr)},{cmt_part}")

            elif child_msg and child_msg in _ONEOF_WRAPPER_FIELD:
                # Oneof-wrapper message (e.g. PaymentMethod, MandateId).
                # The probe dict has {case_key: inner_dict}; prost generates a
                # sub-module named after the oneof field holding the variant enum.
                wrapper_field = _ONEOF_WRAPPER_FIELD[child_msg]
                # Enum type name = PascalCase of wrapper_field
                enum_type = "".join(w.title() for w in wrapper_field.split("_"))
                module    = wrapper_field  # prost sub-module

                case_items = list(val.items())
                if case_items:
                    case_key, case_val = case_items[0]
                    case_type = db.get_type(child_msg, case_key)
                    variant   = "".join(w.title() for w in case_key.split("_"))
                    lines.append(f"{pad}{field}: Some({child_msg} {{{cmt_part}")
                    if isinstance(case_val, dict) and case_type:
                        inner = _rust_struct_lines(
                            case_val, case_type, message_schemas, indent + 2
                        )
                        lines.append(
                            f"{pad}    {wrapper_field}: Some({module}::{enum_type}::{variant}({case_type} {{"
                        )
                        if inner:
                            lines.extend(inner)
                        # Check if all proto fields are present for case_type - if so, no need for Default::default()
                        proto_fields = _PROTO_FIELD_TYPES.get(case_type, {})
                        payload_fields = {_to_snake(k) for k in case_val.keys()}
                        missing_inner = set(proto_fields.keys()) - payload_fields
                        if missing_inner:
                            lines.append(f"{pad}        ..Default::default()")
                        lines.append(f"{pad}    }})),")
                    elif case_val is None or case_val == {}:
                        lines.append(
                            f"{pad}    {wrapper_field}: Some({module}::{enum_type}::{variant}(Default::default())),"
                        )
                    else:
                        lines.append(f"{pad}    // {wrapper_field}: {case_key} = {json.dumps(case_val)}")
                    # Check if all proto fields are present for child_msg - if so, no need for Default::default()
                    proto_child_fields = _PROTO_FIELD_TYPES.get(child_msg, {})
                    child_payload_fields = {_to_snake(k) for k in val.keys()}
                    missing_child = set(proto_child_fields.keys()) - child_payload_fields
                    if missing_child:
                        lines.append(f"{pad}    ..Default::default()")
                    lines.append(f"{pad}}}),")
                else:
                    lines.append(
                        f"{pad}{field}: Some({child_msg} {{ ..Default::default() }}),{cmt_part}"
                    )

            elif is_map and isinstance(val, dict):
                # Map field — convert dict to Rust HashMap literal (NOT wrapped in Some())
                map_entries = []
                for k, v in val.items():
                    map_entries.append(f"({json.dumps(k)}.to_string(), {json.dumps(v)}.to_string())")
                hashmap_expr = f'[{" ,".join(map_entries)}].into_iter().collect::<HashMap<_, _>>()'
                lines.append(f"{pad}{field}: {hashmap_expr},{cmt_part}")

            elif child_msg:
                # Regular nested message — recurse
                inner = _rust_struct_lines(val, child_msg, message_schemas, indent + 1)
                lines.append(f"{pad}{field}: Some({child_msg} {{{cmt_part}")
                if inner:
                    lines.extend(inner)
                # Check if all proto fields are present - if so, no need for Default::default()
                proto_fields = _PROTO_FIELD_TYPES.get(child_msg, {})
                # Convert payload keys to snake_case for comparison
                payload_fields = {_to_snake(k) for k in val.keys()}
                missing_fields = set(proto_fields.keys()) - payload_fields
                # Special case: if currency is missing from Money type, add default
                if child_msg == "Money" and "currency" not in val and "currency" not in {_to_snake(k) for k in val.keys()}:
                    lines.append(f'{pad}    currency: Currency::Usd.into(),  // Default currency for tests')
                    missing_fields.discard("currency")
                if missing_fields:
                    lines.append(f"{pad}    ..Default::default()")
                lines.append(f"{pad}}}),")

            else:
                # Unknown type (no proto metadata) — emit comment
                lines.append(f"{pad}// {field}: {json.dumps(val)}{cmt_part}")

        elif isinstance(val, bool):
            lines.append(f"{pad}{field}: {wrap('true' if val else 'false')},{cmt_part}")
        elif isinstance(val, (int, float)):
            lines.append(f"{pad}{field}: {wrap(str(val))},{cmt_part}")
        elif isinstance(val, str):
            if child_msg and _is_proto_enum(child_msg):
                # For proto enums, convert string value to enum variant
                # e.g., "USD" -> Currency::Usd
                variant = "".join(word.capitalize() for word in val.lower().split("_"))
                expr = f"{child_msg}::{variant}.into()"
                lines.append(f"{pad}{field}: {wrap(expr)},{cmt_part}")
                continue
            elif child_msg and child_msg in _RUST_WRAPPER_CONSTRUCTORS:
                # Special Rust wrapper type with custom constructor
                template, _, _ = _RUST_WRAPPER_CONSTRUCTORS[child_msg]
                expr = template.format(val=json.dumps(val))
                lines.append(f"{pad}{field}: {wrap(expr)},{cmt_part}")
                continue
            elif child_msg and child_msg in _PROTO_WRAPPER_TYPES:
                # Wrapper type with plain string value (SecretString, etc.)
                expr = f"Secret::new({json.dumps(val)}.to_string())"
                lines.append(f"{pad}{field}: {wrap(expr)},{cmt_part}")
                continue
            else:
                expr = f"{json.dumps(val)}.to_string()"
                lines.append(f"{pad}{field}: {wrap(expr)},{cmt_part}")
        else:
            lines.append(f"{pad}// {field}: {json.dumps(val)}{cmt_part}")

    return lines


def _rust_payload_lines(
    obj: dict,
    msg_name: str,
    message_schemas: dict,
    indent: int,
) -> list[str]:
    """Recursively build Rust struct literal lines for a proto payload dict."""
    pad    = "    " * indent
    db     = _SchemaDB(message_schemas)
    lines: list[str] = []

    for key, val in obj.items():
        comment   = db.get_comment(msg_name, key)
        child_msg = db.get_type(msg_name, key)
        cmt_part  = f"  // {comment}" if comment else ""
        field     = _rust_field(key)

        if isinstance(val, dict):
            inner = _rust_payload_lines(val, child_msg, message_schemas, indent + 1)
            inner_str = "\n".join(inner)
            lines.append(f"{pad}{field}: Some({child_msg or 'Message'} {{{cmt_part}")
            lines.append(inner_str)
            lines.append(f"{pad}    ..Default::default()")
            lines.append(f"{pad}}}),")
        elif isinstance(val, bool):
            lines.append(f"{pad}{field}: Some({str(val).lower()}),{cmt_part}")
        elif isinstance(val, (int, float)):
            lines.append(f"{pad}{field}: Some({val}),{cmt_part}")
        elif isinstance(val, str):
            # json.dumps handles escaping of embedded quotes, backslashes, etc.
            lines.append(f"{pad}{field}: Some({json.dumps(val)}.to_string()),{cmt_part}")
        else:
            lines.append(f"{pad}// {field}: {json.dumps(val)}{cmt_part}")

    return lines


def _rust_json_lines(
    obj: dict,
    msg_name: str,
    message_schemas: dict,
    indent: int,
    variable_fields: frozenset = frozenset(),
) -> list[str]:
    """Recursively build serde_json::json!({...}) key-value content lines.

    Probe data maps almost directly to prost's serde JSON format except for
    oneof-wrapper messages (e.g. PaymentMethod), which need an extra field
    wrapping layer: {"card": {...}} → {"payment_method": {"card": {...}}}.

    If *variable_fields* is provided, keys in that set are emitted as bare
    Rust variable references (``key: key,``) instead of JSON-encoded literals.
    """
    pad    = "    " * indent
    db     = _SchemaDB(message_schemas)
    lines: list[str] = []

    for key, val in obj.items():
        comment   = db.get_comment(msg_name, key)
        child_msg = db.get_type(msg_name, key)
        cmt_part  = f"  // {comment}" if comment else ""
        json_key  = json.dumps(key)

        if key in variable_fields:
            lines.append(f"{pad}{json_key}: {key},{cmt_part}")
            continue

        if isinstance(val, dict):
            if child_msg and db.is_wrapper(child_msg):
                # Wrapper message (e.g. SecretString) stored as {"value": "..."} in probe data.
                # Secret<String> deserializes from a plain string in serde, not an object.
                inner_val = val.get("value", "")
                lines.append(f"{pad}{json_key}: {json.dumps(inner_val)},{cmt_part}")
            elif child_msg and child_msg in _ONEOF_WRAPPER_FIELD:
                # Oneof wrapper: add the struct field name that holds the enum.
                wrapper_key = _ONEOF_WRAPPER_FIELD[child_msg]
                inner = _rust_json_lines(val, child_msg, message_schemas, indent + 2)
                lines.append(f"{pad}{json_key}: {{{cmt_part}")
                lines.append(f'{pad}    "{wrapper_key}": {{')
                lines.extend(inner)
                lines.append(f"{pad}    }}")
                lines.append(f"{pad}}},")
            else:
                inner = _rust_json_lines(val, child_msg, message_schemas, indent + 1)
                lines.append(f"{pad}{json_key}: {{{cmt_part}")
                lines.extend(inner)
                lines.append(f"{pad}}},")
        elif isinstance(val, bool):
            lines.append(f"{pad}{json_key}: {str(val).lower()},{cmt_part}")
        elif isinstance(val, (int, float)):
            lines.append(f"{pad}{json_key}: {val},{cmt_part}")
        elif isinstance(val, str):
            lines.append(f"{pad}{json_key}: {json.dumps(val)},{cmt_part}")
        else:
            lines.append(f"{pad}// {json_key}: {json.dumps(val)}{cmt_part}")

    # prost+serde does not add #[serde(default)] to repeated (Vec<T>) fields,
    # so serde_json::from_value requires them to be present even when empty.
    # Emit "field": [] for any repeated fields not already in obj.
    msg_repeated = _PROTO_REPEATED_FIELDS.get(msg_name, set())
    for rep_field in sorted(msg_repeated - set(obj.keys())):
        comment  = db.get_comment(msg_name, rep_field)
        cmt_part = f"  // {comment}" if comment else ""
        lines.append(f'{pad}"{rep_field}": []{cmt_part}')

    return lines


# Human-readable label per PM key (order defines display order in PM Reference section)
_PROBE_PM_LABELS: dict[str, str] = {
    "Card":           "Card (Raw PAN)",
    "GooglePay":      "Google Pay",
    "ApplePay":       "Apple Pay",
    "Sepa":           "SEPA Direct Debit",
    "Bacs":           "BACS Direct Debit",
    "Ach":            "ACH Direct Debit",
    "Becs":           "BECS Direct Debit",
    "Ideal":          "iDEAL",
    "PaypalRedirect": "PayPal Redirect",
    "Blik":           "BLIK",
    "Klarna":         "Klarna",
    "Afterpay":       "Afterpay / Clearpay",
    "UpiCollect":     "UPI Collect",
    "Affirm":         "Affirm",
    "SamsungPay":     "Samsung Pay",
}


# ── Public API: PM reference section ──────────────────────────────────────────

def render_pm_reference_section(
    probe_connector: dict,
    flow_metadata: dict[str, dict],
    message_schemas: dict,
) -> list[str]:
    """
    Return markdown lines for ## Payment Method Reference.

    For each PM supported in authorize, shows only the payment_method object
    with annotated fields — not the full request payload.
    """
    flows    = probe_connector.get("flows", {})
    auth_pms = flows.get("authorize", {})
    grpc_req = flow_metadata.get("authorize", {}).get("grpc_request", "PaymentServiceAuthorizeRequest")
    db       = _SchemaDB(message_schemas)

    out: list[str] = []
    a = out.append

    rendered_any = False
    for pm_key, label in _PROBE_PM_LABELS.items():
        entry = auth_pms.get(pm_key, {})
        if entry.get("status") != "supported":
            continue
        proto_req = entry.get("proto_request", {})
        pm_payload = proto_req.get("payment_method", {})
        if not pm_payload:
            continue

        if not rendered_any:
            a("**Payment method objects** — use these in the `payment_method` field of the Authorize request.")
            a("")
            rendered_any = True

        a(f"##### {label}")
        a("")
        a("```python")
        a(f'"payment_method": {json.dumps(pm_payload, indent=2)}')
        a("```")
        a("")

    return out


def render_consolidated_kotlin(
    connector_name: str,
    flow_items: "list[tuple[str, dict, str]]",
    flow_metadata: dict[str, dict],
    message_schemas: dict,
    scenarios_with_payloads: "list[tuple[ScenarioSpec, dict[str, dict]]] | None" = None,
) -> str:
    """Return one Kotlin file containing all scenario and flow functions for a connector."""
    conn_enum = _conn_enum(connector_name)
    db = _SchemaDB(message_schemas)

    # Collect unique client classes from scenarios and flows
    all_client_cls: list[str] = []
    for scenario, _ in (scenarios_with_payloads or []):
        for fk in scenario.flows:
            cls = _client_class(flow_metadata.get(fk, {}).get("service_name", "PaymentService"))
            if cls not in all_client_cls:
                all_client_cls.append(cls)
    for flow_key, _, _ in flow_items:
        cls = _client_class(flow_metadata.get(flow_key, {}).get("service_name", "PaymentService"))
        if cls not in all_client_cls:
            all_client_cls.append(cls)

    # Collect grpc request types (needed for .newBuilder() calls)
    grpc_req_types: list[str] = []
    for scenario, flow_payloads in (scenarios_with_payloads or []):
        for fk in scenario.flows:
            grpc_req = flow_metadata.get(fk, {}).get("grpc_request", "")
            if grpc_req and grpc_req not in grpc_req_types:
                grpc_req_types.append(grpc_req)
    for flow_key, _, _ in flow_items:
        grpc_req = flow_metadata.get(flow_key, {}).get("grpc_request", "")
        if grpc_req and grpc_req not in grpc_req_types:
            grpc_req_types.append(grpc_req)

    # Collect enum types used in payload fields (for Currency.USD, CaptureMethod.MANUAL, etc.)
    enum_types: set[str] = set()
    for scenario, flow_payloads in (scenarios_with_payloads or []):
        for fk in scenario.flows:
            payload = dict(flow_payloads.get(fk, {}))
            if fk == "authorize":
                if scenario.key in ("checkout_card", "void_payment", "get_payment"):
                    payload["capture_method"] = "MANUAL"
                elif scenario.key == "refund":
                    payload["capture_method"] = "AUTOMATIC"
            grpc_req = flow_metadata.get(fk, {}).get("grpc_request", "")
            enum_types.update(_kotlin_collect_enum_types(payload, grpc_req, db))
    for flow_key, proto_req, _ in flow_items:
        grpc_req = flow_metadata.get(flow_key, {}).get("grpc_request", "")
        enum_types.update(_kotlin_collect_enum_types(proto_req, grpc_req, db))

    # Client classes and enum types live in the `payments` package.
    # grpc_req_types may include types (e.g. EventServiceHandleRequest) that are
    # NOT re-exported as typealiases in `payments` — they only exist in types.Payment.*.
    # Use wildcard imports (matching the SDK's own GeneratedFlows.kt pattern) so all
    # proto-generated classes resolve regardless of whether they have a payments typealias.
    client_imports = "\n".join(f"import payments.{t}" for t in all_client_cls)
    enum_imports   = "\n".join(f"import payments.{t}" for t in sorted(enum_types))
    imports_parts  = [p for p in [client_imports, enum_imports] if p]
    imports        = "\n".join(imports_parts)

    # Connector-specific config imports (only needed when proto metadata exists)
    conn_display    = _conn_display(connector_name)
    config_name     = f"{conn_display}Config"
    has_config_meta = bool(_PROTO_FIELD_TYPES.get(config_name))
    has_secret      = any(
        ft == "SecretString"
        for ft in _PROTO_FIELD_TYPES.get(config_name, {}).values()
    )
    connector_config_imports: list[str] = []
    if has_config_meta:
        connector_config_imports.append("import payments.ConnectorSpecificConfig")
        connector_config_imports.append(f"import types.Payment.{config_name}")
        if has_secret:
            connector_config_imports.append("import payments.SecretString")
    connector_config_import_block = "\n".join(connector_config_imports)

    func_blocks: list[str] = []
    func_names:  list[str] = []

    # Generate scenario function blocks first
    # Build private builder functions (one per flow that has a dynamic param)
    kt_builder_fns: list[str] = []
    kt_has_builder: set[str]  = set()
    for flow_key, proto_req, _ in flow_items:
        if flow_key not in _FLOW_BUILDER_EXTRA_PARAM:
            continue
        grpc_req_b = flow_metadata.get(flow_key, {}).get("grpc_request", "")
        if not grpc_req_b:
            continue
        kt_builder_fns.append(_kt_builder_fn(flow_key, proto_req, grpc_req_b, message_schemas))
        kt_has_builder.add(flow_key)

    def _kt_step_with_builder(scenario_key: str, flow_key: str, step_num: int,
                               client_var: str) -> list[str]:
        """Return Kotlin body lines for a scenario step using a pre-built builder fn."""
        pad      = "    "
        var_name = _to_camel(_FLOW_VAR_NAME.get(flow_key, f"{flow_key.split('_')[0]}_response"))
        desc     = _STEP_DESCRIPTIONS.get(flow_key, flow_key)
        method   = _FLOW_KEY_TO_METHOD.get(flow_key, flow_key)
        fn_name  = "build" + "".join(w.title() for w in flow_key.split("_")) + "Request"

        if flow_key == "authorize" and scenario_key in _CARD_AUTHORIZE_SCENARIOS:
            cm = {"checkout_card": "MANUAL", "void_payment": "MANUAL",
                  "get_payment": "MANUAL", "refund": "AUTOMATIC"}.get(scenario_key, "AUTOMATIC")
            call_arg = f'"{cm}"'
        else:
            call_arg = "authorizeResponse.connectorTransactionId ?: \"\""

        slines: list[str] = [
            f"{pad}// Step {step_num}: {desc}",
            f"{pad}val {var_name} = {client_var}.{method}({fn_name}({call_arg}))",
            "",
        ]
        if flow_key == "authorize":
            slines += [
                f'{pad}when ({var_name}.status.name) {{',
                f'{pad}    "FAILED"  -> throw RuntimeException("Payment failed: ${{{var_name}.error.unifiedDetails.message}}")',
                f'{pad}    "PENDING" -> return mapOf("status" to "PENDING")  // await webhook before proceeding',
                f'{pad}}}',
                "",
            ]
        elif flow_key == "setup_recurring":
            slines += [
                f'{pad}if ({var_name}.status.name == "FAILED")',
                f'{pad}    throw RuntimeException("Setup failed: ${{{var_name}.error.unifiedDetails.message}}")',
                "",
            ]
        elif flow_key in ("capture", "refund", "recurring_charge"):
            slines += [
                f'{pad}if ({var_name}.status.name == "FAILED")',
                f'{pad}    throw RuntimeException("{flow_key.replace("_", " ").title()} failed: ${{{var_name}.error.unifiedDetails.message}}")',
                "",
            ]
        return slines

    for scenario, flow_payloads in (scenarios_with_payloads or []):
        func_name = _to_camel(f"process_{scenario.key}")
        func_names.append(func_name)

        # Collect unique service names for this scenario
        svc_names: list[str] = []
        for fk in scenario.flows:
            svc = flow_metadata.get(fk, {}).get("service_name", "PaymentService")
            if svc not in svc_names:
                svc_names.append(svc)

        body_lines: list[str] = []
        # Instantiate clients
        for svc in svc_names:
            cls = _client_class(svc)
            var = cls[0].lower() + cls[1:]
            body_lines.append(f"    val {var} = {cls}(config)")
        body_lines.append("")

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

            if flow_key in kt_has_builder and scenario.key in _CARD_AUTHORIZE_SCENARIOS | {"checkout_card", "refund", "void_payment", "get_payment"}:
                body_lines.extend(_kt_step_with_builder(scenario.key, flow_key, step_num, client_var))
            else:
                body_lines.extend(_scenario_step_kotlin(
                    scenario.key, flow_key, step_num, payload, grpc_req, message_schemas, client_var
                ))

        body_lines.append(_scenario_return_kotlin(scenario))
        body = "\n".join(body_lines)

        func_blocks.append(
            f"// Scenario: {scenario.title}\n"
            f"// {scenario.description}\n"
            f"fun {func_name}(txnId: String, config: ConnectorConfig = _defaultConfig): Map<String, Any?> {{\n"
            f"{body}\n"
            f"}}"
        )

    # Generate standalone flow function blocks
    for flow_key, proto_req, pm_label in flow_items:
        meta       = flow_metadata.get(flow_key, {})
        svc        = meta.get("service_name", "PaymentService")
        grpc_req   = meta.get("grpc_request", "")
        rpc_name   = meta.get("rpc_name", flow_key)
        
        # Skip flows without proper metadata (not in manifest.json)
        if not grpc_req:
            continue
            
        client_cls = _client_class(svc)
        # SDK method (e.g. "defend" for dispute_defend); function name uses camelCase flow key
        method     = _FLOW_KEY_TO_METHOD.get(flow_key, flow_key)
        func_name  = _to_camel(flow_key)
        pm_part    = f" ({pm_label})" if pm_label else ""

        if flow_key == "authorize":
            status_block = (
                '    when (response.status.name) {\n'
                '        "FAILED"  -> throw RuntimeException("Authorize failed: ${response.error.unifiedDetails.message}")\n'
                '        "PENDING" -> println("Pending — await webhook before proceeding")\n'
                '        else      -> println("Authorized: ${response.connectorTransactionId}")\n'
                '    }'
            )
        elif flow_key == "setup_recurring":
            status_block = (
                '    when (response.status.name) {\n'
                '        "FAILED" -> throw RuntimeException("Setup failed: ${response.error.unifiedDetails.message}")\n'
                '        else     -> println("Mandate stored: ${response.connectorRecurringPaymentId}")\n'
                '    }'
            )
        elif flow_key in ("capture", "refund", "recurring_charge", "void"):
            status_block = (
                f'    if (response.status.name == "FAILED")\n'
                f'        throw RuntimeException("{flow_key.title()} failed: ${{response.error.unifiedDetails.message}}")\n'
                f'    println("Done: ${{response.status.name}}")'
            )
        else:
            status_block = _KT_FLOW_STATUS_BLOCK.get(flow_key, '    println("Status: ${response.status.name}")')

        func_names.append(func_name)
        if flow_key in kt_has_builder:
            param_name  = _FLOW_BUILDER_EXTRA_PARAM[flow_key][0]
            fn_name     = "build" + "".join(w.title() for w in flow_key.split("_")) + "Request"
            default_val = proto_req.get(param_name, "AUTOMATIC" if param_name == "capture_method" else "probe_connector_txn_001")
            param_type  = _FLOW_BUILDER_EXTRA_PARAM[flow_key][1]
            kt_default  = f'"{default_val}"' if param_type == "&str" else default_val
            func_blocks.append(
                f"// Flow: {svc}.{rpc_name}{pm_part}\n"
                f"fun {func_name}(txnId: String) {{\n"
                f"    val client = {client_cls}(_defaultConfig)\n"
                f"    val request = {fn_name}({kt_default})\n"
                f"    val response = client.{method}(request)\n"
                f"{status_block}\n"
                f"}}"
            )
        else:
            processed_req = _preprocess_kt_payload(flow_key, proto_req)
            body_lines = _kotlin_payload_lines(processed_req, grpc_req, message_schemas, indent=2)
            body       = "\n".join(body_lines)
            func_blocks.append(
                f"// Flow: {svc}.{rpc_name}{pm_part}\n"
                f"fun {func_name}(txnId: String) {{\n"
                f"    val client = {client_cls}(_defaultConfig)\n"
                f"    val request = {grpc_req}.newBuilder().apply {{\n"
                f"{body}\n"
                f"    }}.build()\n"
                f"    val response = client.{method}(request)\n"
                f"{status_block}\n"
                f"}}"
            )

    kt_builders_text    = "\n\n".join(kt_builder_fns)
    kt_builders_section = f"\n\n{kt_builders_text}\n" if kt_builder_fns else ""
    funcs_text    = "\n\n".join(func_blocks)
    first         = func_names[0] if func_names else "authorize"
    when_branches_main = "\n".join(f'        "{n}" -> {n}(txnId)' for n in func_names)
    kt_supported_flows = ", ".join(f'"{fk}"' for fk, _, _ in flow_items if fk not in _UNSUPPORTED_FLOWS)

    return f"""\
// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py {connector_name}
//
// {connector_name.title()} — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="{connector_name} processCheckoutCard"

package examples.{connector_name}

import types.Payment.*
import types.PaymentMethods.*
{imports}
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment
{connector_config_import_block}

val SUPPORTED_FLOWS = listOf<String>({kt_supported_flows})

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
{_generate_connector_config_kotlin(connector_name)}
    .build()

{kt_builders_section}
{funcs_text}


fun main(args: Array<String>) {{
    val txnId = "order_001"
    val flow = args.firstOrNull() ?: "{first}"
    when (flow) {{
{when_branches_main}
        else -> System.err.println("Unknown flow: $flow. Available: {', '.join(func_names)}")
    }}
}}
"""


def render_consolidated_rust(
    connector_name: str,
    flow_items: "list[tuple[str, dict, str]]",
    flow_metadata: dict[str, dict],
    message_schemas: dict,
    scenarios_with_payloads: "list[tuple[ScenarioSpec, dict[str, dict]]] | None" = None,
) -> str:
    """Return one Rust file containing all scenario and flow functions for a connector."""
    conn_enum = _conn_enum_rust(connector_name)

    # Collect all grpc_req types for use imports (from scenarios and flows)
    grpc_reqs: list[str] = []
    for scenario, flow_payloads in (scenarios_with_payloads or []):
        for fk in scenario.flows:
            grpc_req = flow_metadata.get(fk, {}).get("grpc_request", "")
            if grpc_req and grpc_req not in grpc_reqs:
                grpc_reqs.append(grpc_req)
    for flow_key, _, _ in flow_items:
        grpc_req = flow_metadata.get(flow_key, {}).get("grpc_request", "")
        if grpc_req and grpc_req not in grpc_reqs:
            grpc_reqs.append(grpc_req)

    # ── Build private request builder functions ────────────────────────────────
    builder_fns: list[str] = []
    has_builder: set[str] = set()        # flows with a dynamic param builder
    has_no_param_builder: set[str] = set()  # flows with a no-param builder

    def _make_builder_with_param(flow_key: str, proto_req: dict, grpc_req_b: str,
                                  param_name: str, param_type: str) -> str:
        struct_lines = _rust_struct_lines(
            proto_req, grpc_req_b, message_schemas, indent=2,
            variable_fields=frozenset({param_name}),
        )
        struct_body = "\n".join(struct_lines)
        # Check if all proto fields are present - if so, no need for Default::default()
        proto_fields = _PROTO_FIELD_TYPES.get(grpc_req_b, {})
        payload_fields = {_to_snake(k) for k in proto_req.keys()}
        # Also account for the variable field that's passed as a parameter
        missing = set(proto_fields.keys()) - payload_fields - {param_name}
        default_suffix = "\n        ..Default::default()" if missing else ""
        return (
            f"pub fn build_{flow_key}_request({param_name}: {param_type}) -> {grpc_req_b} {{\n"
            f"    {grpc_req_b} {{\n"
            f"{struct_body}{default_suffix}\n"
            f"    }}\n"
            f"}}"
        )

    def _make_builder_no_param(flow_key: str, proto_req: dict, grpc_req_b: str) -> str:
        struct_lines = _rust_struct_lines(proto_req, grpc_req_b, message_schemas, indent=2)
        struct_body = "\n".join(struct_lines)
        # Check if all proto fields are present - if so, no need for Default::default()
        proto_fields = _PROTO_FIELD_TYPES.get(grpc_req_b, {})
        payload_fields = {_to_snake(k) for k in proto_req.keys()}
        missing = set(proto_fields.keys()) - payload_fields
        default_suffix = "\n        ..Default::default()" if missing else ""
        return (
            f"pub fn build_{flow_key}_request() -> {grpc_req_b} {{\n"
            f"    {grpc_req_b} {{\n"
            f"{struct_body}{default_suffix}\n"
            f"    }}\n"
            f"}}"
        )

    # Pass 1: flows from flow_items
    for flow_key, proto_req, pm_label in flow_items:
        grpc_req_b = flow_metadata.get(flow_key, {}).get("grpc_request", "")
        if not grpc_req_b:
            continue
        if flow_key in _FLOW_BUILDER_EXTRA_PARAM:
            param_name, param_type = _FLOW_BUILDER_EXTRA_PARAM[flow_key]
            builder_fns.append(_make_builder_with_param(flow_key, proto_req, grpc_req_b, param_name, param_type))
            has_builder.add(flow_key)
        else:
            builder_fns.append(_make_builder_no_param(flow_key, proto_req, grpc_req_b))
            has_no_param_builder.add(flow_key)

    # Pass 2: flows from scenarios not already covered by Pass 1
    for scenario, flow_payloads in (scenarios_with_payloads or []):
        for fk in scenario.flows:
            if fk in has_builder or fk in has_no_param_builder:
                continue
            grpc_req_b = flow_metadata.get(fk, {}).get("grpc_request", "")
            if not grpc_req_b:
                continue
            proto_req = flow_payloads.get(fk, {})
            if not proto_req:
                continue
            if fk in _FLOW_BUILDER_EXTRA_PARAM:
                param_name, param_type = _FLOW_BUILDER_EXTRA_PARAM[fk]
                builder_fns.append(_make_builder_with_param(fk, proto_req, grpc_req_b, param_name, param_type))
                has_builder.add(fk)
            else:
                builder_fns.append(_make_builder_no_param(fk, proto_req, grpc_req_b))
                has_no_param_builder.add(fk)

    func_blocks: list[str] = []
    func_names:  list[str] = []
    match_arms:  list[str] = []

    def _scenario_step_with_builder(
        scenario_key: str,
        flow_key: str,
        step_num: int,
        payload: dict,
        flow_payload_for_cm: dict,
    ) -> list[str]:
        """Return body lines for a scenario step, using a builder fn if available."""
        pad      = "    "
        var_name = _FLOW_VAR_NAME.get(flow_key, f"{flow_key.split('_')[0]}_response")
        desc     = _STEP_DESCRIPTIONS.get(flow_key, flow_key)
        grpc_req = flow_metadata.get(flow_key, {}).get("grpc_request", "")

        builder_call = None
        if flow_key == "authorize" and "authorize" in has_builder and scenario_key in _CARD_AUTHORIZE_SCENARIOS:
            cm_overrides = {
                "checkout_card":  "MANUAL",
                "void_payment":   "MANUAL",
                "get_payment":    "MANUAL",
                "refund":         "AUTOMATIC",
            }
            cm = cm_overrides.get(scenario_key) or flow_payload_for_cm.get("capture_method", "AUTOMATIC")
            builder_call = f'build_authorize_request("{cm}")'
        elif flow_key in ("capture", "void", "get", "refund") and flow_key in has_builder:
            txn = 'authorize_response.connector_transaction_id.as_deref().unwrap_or("")'
            builder_call = f'build_{flow_key}_request({txn})'

        if builder_call is None:
            return _scenario_step_rust(scenario_key, flow_key, step_num, payload, grpc_req, message_schemas)

        step_lines: list[str] = [
            f"{pad}// Step {step_num}: {desc}",
            f"{pad}let {var_name} = client.{_get_client_method(flow_key)}({builder_call}, &HashMap::new(), None).await?;",
            "",
        ]
        step_lines.extend(_rust_status_check_lines(flow_key, var_name, pad))
        return step_lines

    # Generate scenario function blocks first
    # Both scenarios and flows use process_ prefix to match smoke-test expectations
    for scenario, flow_payloads in (scenarios_with_payloads or []):
        process_scenario_key = f"process_{scenario.key}"
        func_names.append(process_scenario_key)
        match_arms.append(f'        "{process_scenario_key}" => {process_scenario_key}(&client, "order_001").await,')

        body_lines: list[str] = []
        for step_num, flow_key in enumerate(scenario.flows, 1):
            meta     = flow_metadata.get(flow_key, {})
            grpc_req = meta.get("grpc_request", "")

            payload = dict(flow_payloads.get(flow_key, {}))
            if flow_key == "authorize":
                if scenario.key in ("checkout_card", "void_payment", "get_payment"):
                    payload["capture_method"] = "MANUAL"
                elif scenario.key == "refund":
                    payload["capture_method"] = "AUTOMATIC"

            body_lines.extend(_scenario_step_with_builder(
                scenario.key, flow_key, step_num, payload, flow_payloads.get(flow_key, {})
            ))

        body_lines.append(_scenario_return_rust(scenario))
        body = "\n".join(body_lines)

        func_blocks.append(
            f"// Scenario: {scenario.title}\n"
            f"// {scenario.description}\n"
            f"#[allow(dead_code)]\n"
            f"pub async fn {process_scenario_key}(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {{\n"
            f"{body}\n"
            f"}}"
        )

    # Generate standalone flow function blocks
    seen_funcs = set(func_names)  # Track already-added function names
    for flow_key, proto_req, pm_label in flow_items:
        # Skip flows that aren't implemented in ConnectorClient
        if flow_key in _UNSUPPORTED_FLOWS:
            continue
        process_fn_name = f"process_{flow_key}"
        if process_fn_name in seen_funcs:
            continue  # Skip duplicates
        seen_funcs.add(process_fn_name)
        func_names.append(process_fn_name)
        match_arms.append(f'        "{process_fn_name}" => {process_fn_name}(&client, "txn_001").await,')

        meta      = flow_metadata.get(flow_key, {})
        svc       = meta.get("service_name", "PaymentService")
        grpc_req  = meta.get("grpc_request", "")
        rpc_name  = meta.get("rpc_name", flow_key)
        pm_part   = f" ({pm_label})" if pm_label else ""

        if flow_key == "authorize":
            status_block = (
                '    match response.status() {\n'
                '        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed\n'
                '            => Err(format!("Authorize failed: {:?}", response.error).into()),\n'
                '        PaymentStatus::Pending => Ok("pending — await webhook".to_string()),\n'
                '        _  => Ok(format!("Authorized: {}", response.connector_transaction_id.as_deref().unwrap_or(""))),\n'
                '    }'
            )
        elif flow_key == "setup_recurring":
            status_block = (
                '    if response.status() == PaymentStatus::Failure {\n'
                '        return Err(format!("Setup failed: {:?}", response.error).into());\n'
                '    }\n'
                '    Ok(format!("Mandate: {}", response.connector_recurring_payment_id.as_deref().unwrap_or("")))'
            )
        elif flow_key == "tokenize":
            status_block = '    Ok(format!("token: {}", response.payment_method_token))'
        elif flow_key == "create_customer":
            status_block = '    Ok(format!("customer_id: {}", response.connector_customer_id))'
        elif flow_key in ("dispute_accept", "dispute_defend", "dispute_submit_evidence"):
            status_block = '    Ok(format!("dispute_status: {:?}", response.dispute_status()))'
        elif flow_key in ("create_access_token", "create_session_token", "create_client_authentication_token", "create_server_session_authentication_token"):
            status_block = '    Ok(format!("status: {:?}", response.status_code))'
        else:
            status_block = '    Ok(format!("status: {:?}", response.status()))'

        if flow_key in has_builder:
            param_name, _ = _FLOW_BUILDER_EXTRA_PARAM[flow_key]
            if param_name == "capture_method":
                default_val = proto_req.get(param_name, "AUTOMATIC")
            else:
                default_val = proto_req.get(param_name, "probe_connector_txn_001")
            builder_call = f'build_{flow_key}_request("{default_val}")'
            func_blocks.append(
                f"// Flow: {svc}.{rpc_name}{pm_part}\n"
                f"#[allow(dead_code)]\n"
                f"pub async fn {process_fn_name}(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {{\n"
                f"    let response = client.{_get_client_method(flow_key)}({builder_call}, &HashMap::new(), None).await?;\n"
                f"{status_block}\n"
                f"}}"
            )
        elif flow_key in has_no_param_builder:
            func_blocks.append(
                f"// Flow: {svc}.{rpc_name}{pm_part}\n"
                f"#[allow(dead_code)]\n"
                f"pub async fn {process_fn_name}(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {{\n"
                f"    let response = client.{_get_client_method(flow_key)}(build_{flow_key}_request(), &HashMap::new(), None).await?;\n"
                f"{status_block}\n"
                f"}}"
            )
        else:
            struct_lines = _rust_struct_lines(proto_req, grpc_req, message_schemas, indent=2)
            struct_body  = "\n".join(struct_lines)
            
            # Handle missing grpc request type gracefully
            if grpc_req:
                rust_type = grpc_req
            else:
                # grpc_req is empty - this is a bug, flow_metadata should provide it
                # Using placeholder to avoid syntax error
                rust_type = f"TODO_FIX_MISSING_TYPE_{flow_key}"
            
            func_blocks.append(
                f"// Flow: {svc}.{rpc_name}{pm_part}\n"
                f"#[allow(dead_code)]\n"
                f"pub async fn {process_fn_name}(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {{\n"
                f"    let response = client.{_get_client_method(flow_key)}({rust_type} {{\n"
                f"{struct_body}\n"
                f"        ..Default::default()\n"
                f"    }}, &HashMap::new(), None).await?;\n"
                f"{status_block}\n"
                f"}}"
            )

    funcs_text     = "\n\n".join(func_blocks)
    builders_text  = "\n\n".join(builder_fns)
    builders_section = f"\n\n{builders_text}\n" if builder_fns else ""
    match_arms_str = "\n".join(match_arms)
    first          = func_names[0] if func_names else "authorize"

    # Build SUPPORTED_FLOWS constant from func_names (all functions have process_ prefix)
    # Strip 'process_' prefix to get the names for the manifest
    flow_names = [fn[8:] for fn in func_names if fn.startswith("process_")]
    # Note: SUPPORTED_FLOWS is inserted by generate_harnesses.py for build.rs validation
    # Don't add it here to avoid duplication
    
    # Determine which additional imports the generated code needs
    all_generated = builders_text + "\n\n".join(func_blocks)
    need_secret         = "Secret::new" in all_generated
    need_payment_method = "payment_method::" in all_generated
    need_mandate_id     = "mandate_id_type::" in all_generated

    extra_imports = ""
    if need_secret:
        extra_imports += "\nuse hyperswitch_masking::Secret;"
    if need_payment_method:
        extra_imports += "\nuse grpc_api_types::payments::payment_method;"
    if need_mandate_id:
        extra_imports += "\nuse grpc_api_types::payments::mandate_id_type;"
    
    # Dynamically add imports for special Rust wrapper types based on what's used
    # Exclude types already handled above (e.g., Secret from hyperswitch_masking)
    _HANDLED_IMPORTS = {"hyperswitch_masking::Secret"}
    extra_rust_imports: set[str] = set()
    need_fromstr = False
    for proto_type, (_, import_path, uses_fromstr) in _RUST_WRAPPER_CONSTRUCTORS.items():
        if import_path in _HANDLED_IMPORTS:
            continue
        # Check if this type's constructor is used in the generated code
        type_name = import_path.split("::")[-1]  # e.g., "CardNumber" from "ucs_cards::CardNumber"
        if type_name in all_generated:
            extra_rust_imports.add(import_path)
            if uses_fromstr:
                need_fromstr = True
    
    for import_path in sorted(extra_rust_imports):
        extra_imports += f"\nuse {import_path};"
    if need_fromstr:
        extra_imports += "\nuse std::str::FromStr;"

    # Generate connector config if available
    connector_config = _generate_connector_config_rust(connector_name)
    if connector_config is None:
        connector_config = "None,  // TODO: Add your connector config here"

    rs_supported_flows = ", ".join(f'"{fk}"' for fk, _, _ in (flow_items or []) if fk not in _UNSUPPORTED_FLOWS)

    return f"""\
// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py {connector_name}
//
// {connector_name.title()} — all scenarios and flows in one file.
// Run a scenario:  cargo run --example {connector_name} -- process_checkout_card
use grpc_api_types::payments::*;
use grpc_api_types::payments::connector_specific_config;
use hyperswitch_payments_client::ConnectorClient;
use std::collections::HashMap;{extra_imports}

#[allow(dead_code)]
pub const SUPPORTED_FLOWS: &[&str] = &[{rs_supported_flows}];

#[allow(dead_code)]
fn build_client() -> ConnectorClient {{
    // Configure the connector with authentication
    let config = ConnectorConfig {{
        connector_config: {connector_config},
        options: Some(SdkOptions {{
            environment: Environment::Sandbox.into(),
        }}),
    }};
    ConnectorClient::new(config, None).unwrap()
}}{builders_section}

{funcs_text}

#[allow(dead_code)]
#[tokio::main]
async fn main() {{
    let client = build_client();
    let flow = std::env::args().nth(1).unwrap_or_else(|| "{first}".to_string());
    let result: Result<String, Box<dyn std::error::Error>> = match flow.as_str() {{
{match_arms_str}
        _ => {{ eprintln!("Unknown flow: {{}}. Available: {', '.join(func_names)}", flow); return; }}
    }};
    match result {{
        Ok(msg) => println!("✓ {{msg}}"),
        Err(e) => eprintln!("✗ {{e}}"),
    }}
}}
"""

# ── Public API: llms.txt entry ────────────────────────────────────────────────

def render_llms_txt_entry(
    connector_name: str,
    display_name: str,
    probe_connector: dict,
    scenarios: list[ScenarioSpec],
) -> str:
    """
    Return one connector's block for docs/llms.txt.
    """
    flows    = probe_connector.get("flows", {})
    auth_pms = flows.get("authorize", {})

    supported_pms = [
        pm for pm in auth_pms
        if pm != "default" and auth_pms[pm].get("status") == "supported"
    ]
    supported_flows = [
        fk for fk, fdata in flows.items()
        if any(v.get("status") == "supported" for v in fdata.values())
    ]
    scenario_keys = [s.key for s in scenarios]
    example_paths = [f"examples/{connector_name}/{connector_name}.py"] if scenario_keys else []

    lines = [
        f"## {display_name}",
        f"connector_id: {connector_name}",
        f"doc: docs/connectors/{connector_name}.md",
        f"scenarios: {', '.join(scenario_keys) if scenario_keys else 'none'}",
        f"payment_methods: {', '.join(supported_pms) if supported_pms else 'none'}",
        f"flows: {', '.join(supported_flows)}",
        f"examples_python: {', '.join(example_paths) if example_paths else 'none'}",
        "",
    ]
    return "\n".join(lines)
