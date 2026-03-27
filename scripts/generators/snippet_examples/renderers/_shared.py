"""Shared globals, constants, and utilities for snippet renderers."""

from __future__ import annotations

import json
import re
from dataclasses import dataclass
from pathlib import Path
from typing import TYPE_CHECKING, TypedDict

if TYPE_CHECKING:
    from .base import LanguageRenderer


# ── TypedDict type hints for metadata ─────────────────────────────────────────

class FlowMetadata(TypedDict, total=False):
    """Type hints for flow metadata dictionary.
    
    Example:
        {
            "flow_key": "authorize",
            "service_name": "PaymentService",
            "grpc_request": "PaymentServiceAuthorizeRequest",
            "rpc_name": "authorize",
        }
    """
    flow_key: str
    service_name: str
    grpc_request: str
    rpc_name: str
    description: str


class MessageSchema(TypedDict, total=False):
    """Type hints for message schema dictionary.
    
    Example:
        {
            "comments": {"field_name": "Field description"},
            "field_types": {"field_name": "FieldType"},
        }
    """
    comments: dict[str, str]
    field_types: dict[str, str]


class RequiredFlowSpec(TypedDict, total=False):
    """Type hints for required flow specification in scenario groups.
    
    Example:
        {
            "flow_key": "authorize",
            "pm_key": "Card",
            "pm_key_variants": ["Card", "GooglePay"],
        }
    """
    flow_key: str
    pm_key: str
    pm_key_variants: list[str]


class ScenarioGroupSpec(TypedDict, total=False):
    """Type hints for scenario group specification from manifest.
    
    Example:
        {
            "key": "checkout_card",
            "title": "Card Payment",
            "description": "Process a card payment",
            "flows": ["authorize", "capture"],
            "pm_key": "Card",
            "required_flows": [...],
        }
    """
    key: str
    title: str
    description: str
    flows: list[str]
    pm_key: str | None
    required_flows: list[RequiredFlowSpec]


class ConnectorConfigSpec(TypedDict, total=False):
    """Type hints for connector configuration specification.
    
    Example:
        {
            "msg_name": "StripeConfig",
            "required_fields": ["api_key", "api_secret"],
        }
    """
    msg_name: str
    required_fields: list[str]


# ── Proto type map globals ────────────────────────────────────────────────────

# {message_name: {field_name: proto_type_name}} — all messages across all protos
_PROTO_FIELD_TYPES: dict[str, dict[str, str]] = {}

# Maps message name -> set of field names that are `repeated` in proto.
_PROTO_REPEATED_FIELDS: dict[str, set[str]] = {}

# Set of message type names that are "scalar wrappers" (single `value` field).
_PROTO_WRAPPER_TYPES: set[str] = set()

# Proto messages that wrap a single oneof field.
_ONEOF_WRAPPER_FIELD: dict[str, str] = {
    "PaymentMethod": "payment_method",
    "MandateId":     "mandate_id_type",
}

# Connector config field registry
_CONNECTOR_CONFIG_FIELDS: dict[str, dict] = {}

# Maps proto type name → Python import module path (populated by load_proto_type_map)
_PROTO_TYPE_SOURCE: dict[str, str] = {}

# Scenario groups (populated by docs/generate.py via set_scenario_groups())
_SCENARIO_GROUPS: list[dict] = []


# ── Language-agnostic constants ───────────────────────────────────────────────

_SERVICE_TO_CLIENT: dict[str, str] = {
    "PaymentService":                     "PaymentClient",
    "CustomerService":                    "CustomerClient",
    "DisputeService":                     "DisputeClient",
    "EventService":                       "EventClient",
    "MerchantAuthenticationService":      "MerchantAuthenticationClient",
    "PaymentMethodAuthenticationService": "PaymentMethodAuthenticationClient",
    "PaymentMethodService":               "PaymentMethodClient",
    "RecurringPaymentService":            "RecurringPaymentClient",
    "RefundService":                      "RefundClient",
}

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

_FLOW_KEY_TO_METHOD: dict[str, str] = {
    "recurring_charge":          "charge",
    "create_customer":           "create",
    "dispute_accept":            "accept",
    "dispute_defend":            "defend",
    "dispute_submit_evidence":   "submit_evidence",
}

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

# Status handling dicts
_AUTHORIZE_STATUS_HANDLING: dict[str, str] = {
    "AUTHORIZED": "Funds reserved — proceed to Capture to settle",
    "PENDING":    "Awaiting async confirmation — wait for webhook before capturing",
    "FAILED":     "Payment declined — surface error to customer, do not retry without new details",
}

_AUTOCAPTURE_STATUS_HANDLING: dict[str, str] = {
    "AUTHORIZED": "Payment authorized and captured — funds will be settled automatically",
    "PENDING":    "Payment processing — await webhook for final status before fulfilling",
    "FAILED":     "Payment declined — surface error to customer, do not retry without new details",
}

_SETUP_RECURRING_STATUS_HANDLING: dict[str, str] = {
    "PENDING": "Mandate stored — save connector_transaction_id for future RecurringPaymentService.Charge calls",
    "FAILED":  "Setup failed — customer must re-enter payment details",
}

# Scenarios that reuse card authorize payload
_CARD_AUTHORIZE_SCENARIOS: frozenset[str] = frozenset({
    "checkout_card", "checkout_autocapture", "refund", "void_payment", "get_payment"
})

# Flow builder extra parameters
_FLOW_BUILDER_EXTRA_PARAM: dict[str, tuple[str, str]] = {
    "authorize": ("capture_method", "&str"),
    "capture":   ("connector_transaction_id", "&str"),
    "void":      ("connector_transaction_id", "&str"),
    "get":       ("connector_transaction_id", "&str"),
    "refund":    ("connector_transaction_id", "&str"),
}

# Fields to drop per scenario
_SCENARIO_DROP_FIELDS: dict[tuple[str, str], frozenset[str]] = {
    ("recurring", "recurring_charge"): frozenset({
        "payment_method_type",
        "payment_method",
    }),
}

# PM labels for reference section
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

# Dynamic fields for each language
_DYNAMIC_FIELDS: dict[tuple[str, str, str], str] = {
    ("checkout_card",      "capture",          "connector_transaction_id"): "authorize_response.connector_transaction_id",
    ("refund",             "refund",            "connector_transaction_id"): "authorize_response.connector_transaction_id",
    ("void_payment",       "void",             "connector_transaction_id"): "authorize_response.connector_transaction_id",
    ("get_payment",        "get",              "connector_transaction_id"): "authorize_response.connector_transaction_id",
    ("recurring",          "recurring_charge",  "connector_recurring_payment_id"): '{"connector_mandate_id": {"connector_mandate_id": setup_response.mandate_reference.connector_mandate_id.connector_mandate_id}}',
}

_DYNAMIC_FIELDS_JS: dict[tuple[str, str, str], str] = {
    ("checkout_card",      "capture",          "connector_transaction_id"): "authorizeResponse.connectorTransactionId",
    ("refund",             "refund",            "connector_transaction_id"): "authorizeResponse.connectorTransactionId",
    ("void_payment",       "void",             "connector_transaction_id"): "authorizeResponse.connectorTransactionId",
    ("get_payment",        "get",              "connector_transaction_id"): "authorizeResponse.connectorTransactionId",
    ("recurring",          "recurring_charge",  "connector_recurring_payment_id"): '{ connectorMandateId: { connectorMandateId: setupResponse.mandateReference?.connectorMandateId?.connectorMandateId } }',
}

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

_DYNAMIC_FIELDS_RS: dict[tuple[str, str, str], list[str]] = {
    ("checkout_card",  "capture",          "connector_transaction_id"):
        ["connector_transaction_id: Some(authorize_response.connector_transaction_id.clone()),  // from Authorize"],
    ("refund",         "refund",           "connector_transaction_id"):
        ["connector_transaction_id: Some(authorize_response.connector_transaction_id.clone()),  // from Authorize"],
    ("void_payment",   "void",             "connector_transaction_id"):
        ["connector_transaction_id: Some(authorize_response.connector_transaction_id.clone()),  // from Authorize"],
    ("get_payment",    "get",              "connector_transaction_id"):
        ["connector_transaction_id: Some(authorize_response.connector_transaction_id.clone()),  // from Authorize"],
    ("recurring",      "recurring_charge", "connector_recurring_payment_id"):
        ["// connector_recurring_payment_id: TODO — extract from setup_response.mandate_reference"],
}

_DYNAMIC_FIELDS_RS_JSON: dict[tuple[str, str, str], list[str]] = {
    ("checkout_card",  "capture",          "connector_transaction_id"):
        ['"connector_transaction_id": &authorize_response.connector_transaction_id,  // from Authorize'],
    ("refund",         "refund",           "connector_transaction_id"):
        ['"connector_transaction_id": &authorize_response.connector_transaction_id,  // from Authorize'],
    ("void_payment",   "void",             "connector_transaction_id"):
        ['"connector_transaction_id": &authorize_response.connector_transaction_id,  // from Authorize'],
    ("get_payment",    "get",              "connector_transaction_id"):
        ['"connector_transaction_id": &authorize_response.connector_transaction_id,  // from Authorize'],
    ("recurring",      "recurring_charge", "connector_recurring_payment_id"):
        ['// "connector_recurring_payment_id": ???,  // TODO: extract from setup_response.mandate_reference'],
}

# Kotlin-specific constants
_KT_FLOW_STATUS_BLOCK: dict[str, str] = {
    "tokenize":                  '    println("Token: ${response.paymentMethodToken}")',
    "create_customer":           '    println("Customer: ${response.connectorCustomerId}")',
    "dispute_accept":            '    println("Dispute status: ${response.disputeStatus.name}")',
    "dispute_defend":            '    println("Dispute status: ${response.disputeStatus.name}")',
    "dispute_submit_evidence":   '    println("Dispute status: ${response.disputeStatus.name}")',
    "create_access_token":       '    println("Access token obtained (statusCode=${response.statusCode})")',
    "create_session_token":      '    println("Session token obtained (statusCode=${response.statusCode})")',
    "create_order":              '    println("Order: ${response.connectorOrderId}")',
}

_KOTLIN_PRIMITIVES = frozenset({
    "string", "bool", "int32", "int64", "uint32", "uint64",
    "float", "double", "bytes", "sint32", "sint64",
    "fixed32", "fixed64", "sfixed32", "sfixed64",
})

# Rust-specific constants
_RUST_KEYWORDS = frozenset({
    "as", "break", "const", "continue", "crate", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod",
    "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct",
    "super", "trait", "true", "type", "unsafe", "use", "where", "while",
})

# JavaScript reserved words
JS_RESERVED = frozenset({"void", "delete", "return", "new", "in", "do", "for", "if"})


# ── Scenario dataclass ────────────────────────────────────────────────────────

@dataclass
class ScenarioSpec:
    key:             str
    title:           str
    flows:           list[str]
    pm_key:          str | None
    description:     str
    status_handling: dict[str, str]


# ── SchemaDB proxy ────────────────────────────────────────────────────────────

class _SchemaDB:
    """Proxy over message_schemas + parsed proto field types."""

    def __init__(self, message_schemas: dict) -> None:
        self._schemas = message_schemas

    def get_comment(self, msg: str, field: str) -> str:
        return self._schemas.get(msg, {}).get("comments", {}).get(field, "")

    def get_type(self, msg: str, field: str) -> str:
        t = self._schemas.get(msg, {}).get("field_types", {}).get(field, "")
        if not t:
            t = _PROTO_FIELD_TYPES.get(msg, {}).get(field, "")
        return t

    def is_wrapper(self, type_name: str) -> bool:
        """Return True if type_name is a single-value wrapper message."""
        return type_name in _PROTO_WRAPPER_TYPES

    def single_field_wrapper_key(self, type_name: str) -> str | None:
        """If type_name has exactly one field and that field's type is a wrapper, return the field name."""
        fields = _PROTO_FIELD_TYPES.get(type_name, {})
        if len(fields) == 1:
            field_name, field_type = next(iter(fields.items()))
            if self.is_wrapper(field_type):
                return field_name
        return None


# ── Utility functions ─────────────────────────────────────────────────────────

def _json_scalar(val: object, js: bool = False) -> str:
    """Convert a scalar value to its language literal representation."""
    if isinstance(val, bool):
        if js:
            return "true" if val else "false"
        return "True" if val else "False"
    if val is None:
        return "null" if js else "None"
    return json.dumps(val)


def _to_camel(snake: str) -> str:
    parts = snake.split("_")
    return parts[0] + "".join(p.title() for p in parts[1:])


def _snake_to_camel(s: str) -> str:
    parts = s.split("_")
    return parts[0] + "".join(p.capitalize() for p in parts[1:])


def _snake_to_pascal(s: str) -> str:
    return "".join(p.capitalize() for p in s.split("_"))


def _conn_enum(connector_name: str) -> str:
    overrides = {"razorpayv2": "RAZORPAY"}
    return overrides.get(connector_name, connector_name.upper())


def _conn_enum_rust(connector_name: str) -> str:
    """Return the PascalCase Rust Connector enum variant."""
    overrides = {"razorpayv2": "razorpay"}
    name = overrides.get(connector_name, connector_name)
    return name.replace("_", "").capitalize()


def _conn_display(connector_name: str) -> str:
    return connector_name.replace("_", " ").title().replace(" ", "")


def _client_class(service_name: str) -> str:
    return _SERVICE_TO_CLIENT.get(
        service_name,
        service_name.replace("Service", "") + "Client",
    )


def _annotate_inline_lines(
    obj: dict,
    msg_name: str,
    db: _SchemaDB,
    indent: int,
    cmt: str,
    camel_keys: bool = False,
) -> list[str]:
    """Render object dict as annotated JSON lines."""
    pad   = "    " * indent
    lines: list[str] = []

    items = list(obj.items())
    for idx, (key, val) in enumerate(items):
        trailing  = "," if idx < len(items) - 1 else ""
        comment   = db.get_comment(msg_name, key)
        child_msg = db.get_type(msg_name, key)
        cmt_part  = f"  {cmt} {comment}" if comment else ""
        out_key   = _to_camel(key) if camel_keys else key

        if isinstance(val, dict):
            lines.append(f'{pad}"{out_key}": {{{cmt_part}')
            lines.extend(_annotate_inline_lines(val, child_msg, db, indent + 1, cmt, camel_keys))
            lines.append(f"{pad}}}{trailing}")
        elif isinstance(val, list) and val and isinstance(val[0], dict):
            lines.append(f'{pad}"{out_key}": [{cmt_part}')
            for j, item in enumerate(val):
                item_trailing = "," if j < len(val) - 1 else ""
                lines.append(f"{pad}    {{")
                lines.extend(_annotate_inline_lines(item, child_msg, db, indent + 2, cmt, camel_keys))
                lines.append(f"{pad}    }}{item_trailing}")
            lines.append(f"{pad}]{trailing}")
        elif child_msg and db.is_wrapper(child_msg):
            lines.append(f'{pad}"{out_key}": {{"value": {_json_scalar(val, js=camel_keys)}}}{trailing}{cmt_part}')
        elif child_msg and not isinstance(val, (dict, list)):
            _sfwk = db.single_field_wrapper_key(child_msg)
            inner_key = _to_camel(_sfwk) if (camel_keys and _sfwk) else _sfwk
            if inner_key:
                lines.append(f'{pad}"{out_key}": {{"{inner_key}": {{"value": {_json_scalar(val, js=camel_keys)}}}}}{trailing}{cmt_part}')
            else:
                lines.append(f'{pad}"{out_key}": {_json_scalar(val, js=camel_keys)}{trailing}{cmt_part}')
        else:
            lines.append(f'{pad}"{out_key}": {_json_scalar(val, js=camel_keys)}{trailing}{cmt_part}')

    return lines


def _annotate_before_lines(
    obj: dict,
    msg_name: str,
    db: _SchemaDB,
    indent: int,
) -> list[str]:
    """Render object dict with before-comment annotations."""
    pad   = "    " * indent
    lines: list[str] = []

    items = list(obj.items())
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


def _build_annotated(
    obj: dict,
    msg_name: str,
    db: _SchemaDB,
    style: str,
    indent: int = 0,
) -> str:
    """Build fully annotated payload string."""
    pad = "    " * indent
    if style == "kotlin":
        inner = _annotate_before_lines(obj, msg_name, db, indent + 1)
    else:
        cmt = "#" if style == "python" else "//"
        inner = _annotate_inline_lines(obj, msg_name, db, indent + 1, cmt)
    return "\n".join(["{"] + inner + [f"{pad}}}"])


# ── Proto loading functions ───────────────────────────────────────────────────

def load_proto_type_map(proto_dir: Path) -> None:
    """Parse all *.proto files to build type maps."""
    global _PROTO_WRAPPER_TYPES  # frozenset→set still needs global for .clear()/.update()

    type_map: dict[str, dict[str, str]] = {}
    repeated_map: dict[str, set[str]] = {}
    source_map: dict[str, str] = {}
    _FIELD_RE = re.compile(
        r"^\s*(repeated\s+)?(?:optional\s+)?(\w+)\s+(\w+)\s*=\s*\d+"
    )
    _SKIP_KEYWORDS = frozenset(
        ["message", "enum", "oneof", "reserved", "option", "extensions",
         "syntax", "import", "package", "service", "rpc", "returns"]
    )
    _ONEOF_RE = re.compile(r"\boneof\s+\w+\s*\{([^}]*)\}", re.DOTALL)

    for proto_file in sorted(proto_dir.glob("*.proto")):
        # Derive the Python module path from the proto filename
        stem = proto_file.stem.replace("-", "_")
        module_path = f"payments.generated.{stem}_pb2"

        text = proto_file.read_text(encoding="utf-8")
        text = re.sub(r"//[^\n]*", "", text)
        text = re.sub(r"/\*.*?\*/", "", text, flags=re.DOTALL)

        # Track enum type names defined in this file
        for em in re.finditer(r"\benum\s+(\w+)\s*\{", text):
            source_map[em.group(1)] = module_path

        pos = 0
        while pos < len(text):
            m = re.search(r"\bmessage\s+(\w+)\s*\{", text[pos:])
            if not m:
                break
            msg_name = m.group(1)
            body_start = pos + m.end()

            depth = 1
            i = body_start
            while i < len(text) and depth > 0:
                if text[i] == "{":
                    depth += 1
                elif text[i] == "}":
                    depth -= 1
                i += 1
            body = text[body_start : i - 1]

            fields: dict[str, str] = {}
            repeated_fields: set[str] = set()

            # Parse top-level fields (skip nested message/enum/oneof blocks)
            inner_depth = 0
            for line in body.splitlines():
                inner_depth += line.count("{") - line.count("}")
                if inner_depth > 0:
                    continue
                fm = _FIELD_RE.match(line)
                if fm:
                    is_repeated, ftype, fname = fm.group(1), fm.group(2), fm.group(3)
                    if ftype not in _SKIP_KEYWORDS and fname not in _SKIP_KEYWORDS:
                        fields[fname] = ftype
                        if is_repeated:
                            repeated_fields.add(fname)

            # Also parse fields inside oneof blocks (variant fields)
            for oneof_m in _ONEOF_RE.finditer(body):
                for line in oneof_m.group(1).splitlines():
                    fm = _FIELD_RE.match(line)
                    if fm:
                        is_repeated, ftype, fname = fm.group(1), fm.group(2), fm.group(3)
                        if ftype not in _SKIP_KEYWORDS and fname not in _SKIP_KEYWORDS:
                            fields[fname] = ftype

            type_map[msg_name] = fields
            repeated_map[msg_name] = repeated_fields
            source_map[msg_name] = module_path
            pos = pos + m.start() + 1

    # Mutate in-place so module-level imports in renderers stay valid
    _PROTO_FIELD_TYPES.clear()
    _PROTO_FIELD_TYPES.update(type_map)
    _PROTO_REPEATED_FIELDS.clear()
    _PROTO_REPEATED_FIELDS.update(repeated_map)
    _PROTO_TYPE_SOURCE.clear()
    _PROTO_TYPE_SOURCE.update(source_map)
    _PROTO_WRAPPER_TYPES.clear()
    _PROTO_WRAPPER_TYPES.update(
        name for name, fields in type_map.items()
        if set(fields.keys()) == {"value"}
    )

    _build_connector_config_fields(proto_dir)


def _build_connector_config_fields(proto_dir: Path) -> None:
    """Parse payment.proto to build connector config registry."""
    global _CONNECTOR_CONFIG_FIELDS

    payment_proto = proto_dir / "payment.proto"
    if not payment_proto.exists():
        return

    text = payment_proto.read_text(encoding="utf-8")
    text_clean = re.sub(r"//[^\n]*", "", text)

    msg_required: dict[str, list[str]] = {}
    pos = 0
    while pos < len(text_clean):
        m = re.search(r"\bmessage\s+(\w+Config)\s*\{", text_clean[pos:])
        if not m:
            break
        msg_name = m.group(1)
        body_start = pos + m.end()
        depth, i = 1, body_start
        while i < len(text_clean) and depth > 0:
            if text_clean[i] == "{":
                depth += 1
            elif text_clean[i] == "}":
                depth -= 1
            i += 1
        body = text_clean[body_start : i - 1]

        required: list[str] = []
        for line in body.splitlines():
            line = line.strip()
            if not line or line.startswith("optional") or line.startswith("repeated"):
                continue
            fm = re.match(r"(\w+)\s+(\w+)\s*=\s*\d+", line)
            if fm and fm.group(2) != "base_url":
                required.append(fm.group(2))
        msg_required[msg_name] = required
        pos = pos + m.start() + 1

    oneof_m = re.search(
        r"message ConnectorSpecificConfig\s*\{.*?oneof config\s*\{(.*?)\}",
        text_clean, re.DOTALL,
    )
    if oneof_m:
        for line in oneof_m.group(1).splitlines():
            fm = re.match(r"\s*(\w+Config)\s+(\w+)\s*=\s*\d+", line)
            if fm:
                msg_name, conn_name = fm.group(1), fm.group(2)
                _CONNECTOR_CONFIG_FIELDS[conn_name] = {
                    "msg_name": msg_name,
                    "required_fields": msg_required.get(msg_name, []),
                }


def set_scenario_groups(groups: list[dict]) -> None:
    """Set scenario groups from manifest (mutates in-place so imported references stay valid)."""
    _SCENARIO_GROUPS.clear()
    _SCENARIO_GROUPS.extend(groups)


# ── HTML table cell builder ───────────────────────────────────────────────────

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
