"""
Snippet Examples Generator — SDK integration example generator for connector docs.

All functions are pure (no I/O, no global state). Called by docs/generate.py.

Proto field comments come from manifest["message_schemas"] (populated by field-probe).

Public API
----------
  detect_scenarios(probe_connector) -> list[ScenarioSpec]
    Infer applicable integration scenarios from probe data.

  render_config_section(connector_name) -> list[str]
    4-tab SDK config table — emitted once per connector doc.

  render_scenario_section(scenario, connector_name, flow_payloads,
                           flow_metadata, message_schemas, ann_scenario) -> list[str]
    Full 4-tab runnable scenario example + status-handling table.

  render_pm_reference_section(probe_connector, flow_metadata,
                               message_schemas) -> list[str]
    Per-PM payment_method payload reference block.

  render_payload_block(flow_key, service_name, grpc_request,
                       proto_request, message_schemas) -> list[str]
    Single annotated payload block (used in Flow Reference section).

  render_llms_txt_entry(connector_name, display_name, probe_connector,
                         scenarios) -> str
    One connector block for docs/llms.txt.
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

# Set of message type names that are "scalar wrappers" (single `value` field).
# These are stored as plain scalars in probe data but must be sent as
# {"value": ...} dicts in ParseDict calls.
_PROTO_WRAPPER_TYPES: frozenset[str] = frozenset()

# Proto messages that wrap a single oneof field.  In prost + serde, the outer
# struct has one field with the same name as the oneof (snake_case), so when
# building serde_json::json!({...}) we must add that extra wrapping layer.
_ONEOF_WRAPPER_FIELD: dict[str, str] = {
    "PaymentMethod": "payment_method",
    "MandateId":     "mandate_id_type",
}


def load_proto_type_map(proto_dir: Path) -> None:
    """Parse all *.proto files in proto_dir to build _PROTO_FIELD_TYPES and _PROTO_WRAPPER_TYPES."""
    global _PROTO_FIELD_TYPES, _PROTO_WRAPPER_TYPES, _PROTO_REPEATED_FIELDS

    type_map: dict[str, dict[str, str]] = {}
    repeated_map: dict[str, set[str]] = {}
    _FIELD_RE = re.compile(
        r"^\s*(repeated\s+)?(?:optional\s+)?(\w+)\s+(\w+)\s*=\s*\d+"
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

            # Extract only top-level lines (not inside nested { })
            fields: dict[str, str] = {}
            repeated_fields: set[str] = set()
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

            type_map[msg_name] = fields
            repeated_map[msg_name] = repeated_fields
            pos = pos + m.start() + 1  # advance past this message keyword

    _PROTO_FIELD_TYPES = type_map
    _PROTO_REPEATED_FIELDS = repeated_map
    # Wrapper types: messages whose only field is named "value"
    _PROTO_WRAPPER_TYPES = frozenset(
        name for name, fields in type_map.items()
        if set(fields.keys()) == {"value"}
    )


# ── Constants ──────────────────────────────────────────────────────────────────

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

# PM keys that represent wallets (first supported one is used for checkout_wallet)
_WALLET_PM_KEYS = ["GooglePay", "ApplePay", "SamsungPay"]

# PM keys that represent bank transfers (first supported one is used for checkout_bank)
_BANK_PM_KEYS = ["Sepa", "Ach", "Bacs", "Becs"]

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
}

# JavaScript reserved words — flow functions whose flow key is reserved get a "Payment" suffix.
JS_RESERVED = frozenset({"void", "delete", "return", "new", "in", "do", "for", "if"})

# Flow keys whose SDK method name differs from the flow key itself.
# All other flows use the flow key directly as the method name (snake_case).
_FLOW_KEY_TO_METHOD: dict[str, str] = {
    "recurring_charge":          "charge",           # RecurringPaymentService.charge()
    "create_customer":           "create",           # CustomerClient.create()
    "dispute_accept":            "accept",           # DisputeClient.accept()
    "dispute_defend":            "defend",           # DisputeClient.defend()
    "dispute_submit_evidence":   "submit_evidence",  # DisputeClient.submit_evidence()
}

# Variable name used for the response of each flow step.
# Defaults to "{first_word_of_flow_key}_response" for most flows.
_FLOW_VAR_NAME: dict[str, str] = {
    "pre_authenticate":  "pre_authenticate_response",
    "authenticate":      "authenticate_response",
    "post_authenticate": "post_authenticate_response",
    "create_customer":   "create_response",
    "setup_recurring":   "setup_response",
    "recurring_charge":  "recurring_response",
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

    Rules:
      checkout_card        — authorize(Card) + capture both supported
      checkout_autocapture — authorize(Card) supported (no separate capture call)
      checkout_wallet      — authorize(GooglePay|ApplePay|SamsungPay) supported
      checkout_bank        — authorize(Sepa|Ach|Bacs|Becs) supported
      refund               — refund supported AND Card authorize supported
      recurring            — setup_recurring + recurring_charge both supported
    """
    flows = probe_connector.get("flows", {})

    def ok(flow_key: str, pm_key: str = "default") -> bool:
        return flows.get(flow_key, {}).get(pm_key, {}).get("status") == "supported"

    def has_payload(flow_key: str, pm_key: str = "default") -> bool:
        return bool(flows.get(flow_key, {}).get(pm_key, {}).get("proto_request"))

    card_ok           = ok("authorize", "Card") and has_payload("authorize", "Card")
    capture_ok        = ok("capture")
    refund_ok         = ok("refund")
    void_ok           = ok("void") and has_payload("void")
    get_ok            = ok("get") and has_payload("get")
    tokenize_ok       = ok("tokenize") and has_payload("tokenize")
    create_customer_ok = ok("create_customer") and has_payload("create_customer")
    setup_ok          = ok("setup_recurring")
    charge_ok         = ok("recurring_charge")
    pre_auth_ok       = ok("pre_authenticate") and has_payload("pre_authenticate")
    auth_ok           = ok("authenticate") and has_payload("authenticate")
    post_auth_ok      = ok("post_authenticate") and has_payload("post_authenticate")

    scenarios: list[ScenarioSpec] = []

    if card_ok and capture_ok:
        scenarios.append(ScenarioSpec(
            key="checkout_card",
            title="Card Payment (Authorize + Capture)",
            flows=["authorize", "capture"],
            pm_key="Card",
            description=(
                "Reserve funds with Authorize, then settle with a separate Capture call. "
                "Use for physical goods or delayed fulfillment where capture happens later."
            ),
            status_handling=_AUTHORIZE_STATUS_HANDLING,
        ))

    if card_ok:
        scenarios.append(ScenarioSpec(
            key="checkout_autocapture",
            title="Card Payment (Automatic Capture)",
            flows=["authorize"],
            pm_key="Card",
            description=(
                "Authorize and capture in one call using `capture_method=AUTOMATIC`. "
                "Use for digital goods or immediate fulfillment."
            ),
            status_handling=_AUTOCAPTURE_STATUS_HANDLING,
        ))

    for wallet_pm in _WALLET_PM_KEYS:
        if ok("authorize", wallet_pm) and has_payload("authorize", wallet_pm):
            scenarios.append(ScenarioSpec(
                key="checkout_wallet",
                title="Wallet Payment (Google Pay / Apple Pay)",
                flows=["authorize"],
                pm_key=wallet_pm,
                description=(
                    "Wallet payments pass an encrypted token from the browser/device SDK. "
                    "Pass the token blob directly — do not decrypt client-side."
                ),
                status_handling=_AUTOCAPTURE_STATUS_HANDLING,
            ))
            break

    for bank_pm in _BANK_PM_KEYS:
        if ok("authorize", bank_pm) and has_payload("authorize", bank_pm):
            scenarios.append(ScenarioSpec(
                key="checkout_bank",
                title="Bank Transfer (SEPA / ACH / BACS)",
                flows=["authorize"],
                pm_key=bank_pm,
                description=(
                    f"Direct bank debit ({bank_pm}). "
                    "Bank transfers typically use `capture_method=AUTOMATIC`."
                ),
                status_handling=_AUTOCAPTURE_STATUS_HANDLING,
            ))
            break

    if refund_ok and card_ok:
        scenarios.append(ScenarioSpec(
            key="refund",
            title="Refund a Payment",
            flows=["authorize", "refund"],
            pm_key="Card",
            description=(
                "Authorize with automatic capture, then refund the captured amount. "
                "`connector_transaction_id` from the Authorize response is reused for the Refund call."
            ),
            status_handling={},
        ))

    if setup_ok and charge_ok:
        scenarios.append(ScenarioSpec(
            key="recurring",
            title="Recurring / Mandate Payments",
            flows=["setup_recurring", "recurring_charge"],
            pm_key=None,
            description=(
                "Store a payment mandate with SetupRecurring, then charge it repeatedly "
                "with RecurringPaymentService.Charge without requiring customer action."
            ),
            status_handling=_SETUP_RECURRING_STATUS_HANDLING,
        ))

    if card_ok and void_ok:
        scenarios.append(ScenarioSpec(
            key="void_payment",
            title="Void a Payment",
            flows=["authorize", "void"],
            pm_key="Card",
            description=(
                "Authorize funds with a manual capture flag, then cancel the authorization "
                "with Void before any capture occurs. Releases the hold on the customer's funds."
            ),
            status_handling={},
        ))

    if card_ok and get_ok:
        scenarios.append(ScenarioSpec(
            key="get_payment",
            title="Get Payment Status",
            flows=["authorize", "get"],
            pm_key="Card",
            description=(
                "Authorize a payment, then poll the connector for its current status using Get. "
                "Use this to sync payment state when webhooks are unavailable or delayed."
            ),
            status_handling={},
        ))

    if create_customer_ok:
        scenarios.append(ScenarioSpec(
            key="create_customer",
            title="Create Customer",
            flows=["create_customer"],
            pm_key=None,
            description=(
                "Register a customer record in the connector system. "
                "Returns a connector_customer_id that can be reused for recurring payments "
                "and tokenized card storage."
            ),
            status_handling={},
        ))

    if tokenize_ok:
        scenarios.append(ScenarioSpec(
            key="tokenize",
            title="Tokenize Payment Method",
            flows=["tokenize"],
            pm_key=None,
            description=(
                "Store card details in the connector's vault and receive a reusable payment token. "
                "Use the returned token for one-click payments and recurring billing "
                "without re-collecting card data."
            ),
            status_handling={},
        ))

    if pre_auth_ok and auth_ok and post_auth_ok:
        scenarios.append(ScenarioSpec(
            key="authentication",
            title="3DS Authentication",
            flows=["pre_authenticate", "authenticate", "post_authenticate"],
            pm_key=None,
            description=(
                "Full 3D Secure authentication flow: PreAuthenticate collects device/browser data, "
                "Authenticate executes the challenge or frictionless verification, "
                "PostAuthenticate validates the result with the issuing bank."
            ),
            status_handling={},
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


# ── Annotated JSON rendering ───────────────────────────────────────────────────

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
) -> list[str]:
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
            # Scalar stored in probe data, but proto field is a wrapper message — needs {"value": ...}
            lines.append(f'{pad}"{out_key}": {{"value": {_json_scalar(val, js=camel_keys)}}}{trailing}{cmt_part}')
        elif child_msg and not isinstance(val, (dict, list)):
            # Scalar for a non-wrapper message — check if msg has one field that is itself a wrapper
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
    pad = "    " * indent
    if style == "kotlin":
        inner = _annotate_before_lines(obj, msg_name, db, indent + 1)
    else:
        cmt = "#" if style == "python" else "//"
        inner = _annotate_inline_lines(obj, msg_name, db, indent + 1, cmt)
    return "\n".join(["{"] + inner + [f"{pad}}}"])


# ── Helpers ────────────────────────────────────────────────────────────────────

def _client_class(service_name: str) -> str:
    return _SERVICE_TO_CLIENT.get(
        service_name,
        service_name.replace("Service", "") + "Client",
    )


def _to_camel(snake: str) -> str:
    parts = snake.split("_")
    return parts[0] + "".join(p.title() for p in parts[1:])


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
from payments.generated import sdk_config_pb2, payment_pb2

config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
)
# Set credentials before running (field names depend on connector auth type):
# config.connector_config.CopyFrom(payment_pb2.ConnectorSpecificConfig(
#     {connector_name}=payment_pb2.{connector_name.title()}Config(api_key=...),
# ))
"""


def _config_javascript(connector_name: str) -> str:
    conn_display = _conn_display(connector_name)
    return f"""\
const {{ ConnectorClient }} = require('connector-service-node-ffi');

// Reuse this client for all flows
const client = new ConnectorClient({{
    connector: '{conn_display}',
    environment: 'sandbox',
    connector_auth_type: {{
        header_key: {{ api_key: 'YOUR_API_KEY' }},
    }},
}});"""


def _config_kotlin(connector_name: str) -> str:
    conn_display = _conn_display(connector_name)
    return f"""\
val config = ConnectorConfig.newBuilder()
    .setConnector("{conn_display}")
    .setEnvironment(Environment.SANDBOX)
    .setAuth(
        ConnectorAuthType.newBuilder()
            .setHeaderKey(HeaderKey.newBuilder().setApiKey("YOUR_API_KEY"))
    )
    .build()"""


def _config_rust(connector_name: str) -> str:
    conn_display = _conn_display(connector_name)
    return f"""\
use connector_service_sdk::{{ConnectorClient, ConnectorConfig}};

let config = ConnectorConfig {{
    connector: "{conn_display}".to_string(),
    environment: Environment::Sandbox,
    auth: ConnectorAuth::HeaderKey {{ api_key: "YOUR_API_KEY".into() }},
    ..Default::default()
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
    Return lines for one step inside a scenario function body.
    Indentation: function body = 4 spaces, ParseDict args = 8 spaces, payload fields = 12 spaces.
    """
    method   = _FLOW_KEY_TO_METHOD.get(flow_key, flow_key)  # Python SDK uses snake_case method names
    var_name = _FLOW_VAR_NAME.get(flow_key, f"{flow_key.split('_')[0]}_response")
    desc     = _STEP_DESCRIPTIONS.get(flow_key, flow_key)
    lines: list[str] = []

    lines.append(f"    # Step {step_num}: {desc}")
    lines.append(f"    {var_name} = await {client_var}.{method}(ParseDict(")
    lines.append("        {")

    drop_fields = _SCENARIO_DROP_FIELDS.get((scenario_key, flow_key), frozenset())
    if payload:
        items = [(k, v) for k, v in payload.items() if k not in drop_fields]
        for idx, (key, val) in enumerate(items):
            trailing  = "," if idx < len(items) - 1 else ""
            comment   = db.get_comment(grpc_req, key)
            child_msg = db.get_type(grpc_req, key)
            cmt_part  = f"  # {comment}" if comment else ""

            # Check if this field should reference a previous response
            dyn = _DYNAMIC_FIELDS.get((scenario_key, flow_key, key))
            if dyn:
                extra = f"  # from Authorize response" if "connector_transaction_id" in key else f"  # from SetupRecurring response"
                lines.append(f'            "{key}": {dyn},{extra}')
            elif isinstance(val, dict):
                lines.append(f'            "{key}": {{{cmt_part}')
                lines.extend(_annotate_inline_lines(val, child_msg, db, indent=4, cmt="#"))
                lines.append(f'            }}{trailing}')
            elif child_msg and db.is_wrapper(child_msg):
                # Scalar stored in probe data, but proto type is a wrapper message
                lines.append(f'            "{key}": {{"value": {_json_scalar(val)}}}{trailing}{cmt_part}')
            elif child_msg and not isinstance(val, (dict, list)):
                # Scalar for a non-wrapper message — check if msg has one field that is itself a wrapper
                inner_key = db.single_field_wrapper_key(child_msg)
                if inner_key:
                    lines.append(f'            "{key}": {{"{inner_key}": {{"value": {_json_scalar(val)}}}}}{trailing}{cmt_part}')
                else:
                    lines.append(f'            "{key}": {_json_scalar(val)}{trailing}{cmt_part}')
            else:
                lines.append(f'            "{key}": {_json_scalar(val)}{trailing}{cmt_part}')
    else:
        lines.append('            # No required fields')

    lines.append("        },")
    if grpc_req:
        lines.append(f"        payment_pb2.{grpc_req}(),")
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


def render_scenario_python(
    scenario: ScenarioSpec,
    connector_name: str,
    flow_payloads: dict[str, dict],
    flow_metadata: dict[str, dict],
    message_schemas: dict,
) -> str:
    """Return the full content of a runnable Python scenario file."""
    db         = _SchemaDB(message_schemas)
    conn_enum  = _conn_enum(connector_name)
    func_name  = f"process_{scenario.key}"

    # Collect unique service names and their client classes
    service_names: list[str] = []
    for fk in scenario.flows:
        svc = flow_metadata.get(fk, {}).get("service_name", "PaymentService")
        if svc not in service_names:
            service_names.append(svc)

    client_imports = "\n".join(
        f"from payments import {_client_class(svc)}" for svc in service_names
    )

    # Build function body
    body_lines: list[str] = []

    # Instantiate clients
    for svc in service_names:
        cls     = _client_class(svc)
        var     = cls.lower().replace("client", "_client")
        body_lines.append(f"    {var} = {cls}(config)")
    body_lines.append("")

    # One step per flow
    for step_num, flow_key in enumerate(scenario.flows, 1):
        meta       = flow_metadata.get(flow_key, {})
        svc        = meta.get("service_name", "PaymentService")
        grpc_req   = meta.get("grpc_request", "")
        client_var = _client_class(svc).lower().replace("client", "_client")

        payload = dict(flow_payloads.get(flow_key, {}))
        if flow_key == "authorize":
            if scenario.key in ("checkout_card", "void_payment", "get_payment"):
                # reserve funds only — capture/void/get happens in the next step
                payload["capture_method"] = "MANUAL"
            elif scenario.key == "refund":
                # refund scenario needs the payment already captured
                payload["capture_method"] = "AUTOMATIC"

        body_lines.extend(_scenario_step_python(
            scenario.key, flow_key, step_num, payload, grpc_req, client_var, db
        ))

    body_lines.append(_scenario_return_python(scenario))
    body = "\n".join(body_lines)

    return f"""\
# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py {connector_name}
#
# Scenario: {scenario.title}
# {scenario.description}

import asyncio
from google.protobuf.json_format import ParseDict
{client_imports}
from payments.generated import sdk_config_pb2, payment_pb2

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
)
# Standalone credentials (field names depend on connector auth type):
# _default_config.connector_config.CopyFrom(payment_pb2.ConnectorSpecificConfig(
#     {connector_name}=payment_pb2.{connector_name.title()}Config(api_key=...),
# ))


async def {func_name}(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    \"\"\"{scenario.title}

    {scenario.description}
    \"\"\"
{body}


if __name__ == "__main__":
    asyncio.run({func_name}("order_001"))
"""


def _scenario_step_javascript(
    scenario_key: str,
    flow_key: str,
    step_num: int,
    payload: dict,
    grpc_req: str,
    db: _SchemaDB,
    client_var: str = "client",
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

            dyn = _DYNAMIC_FIELDS_JS.get((scenario_key, flow_key, key))
            if dyn:
                extra = "  // from authorize response" if "authorize" in dyn.lower() else "  // from setup response"
                lines.append(f'        "{_to_camel(key)}": {dyn},{extra}')
            elif isinstance(val, dict):
                lines.append(f'        "{_to_camel(key)}": {{{cmt_part}')
                lines.extend(_annotate_inline_lines(val, child_msg, db, indent=3, cmt="//", camel_keys=True))
                lines.append(f'        }}{trailing}')
            elif child_msg and db.is_wrapper(child_msg):
                # Scalar stored in probe data, but proto type is a wrapper message — needs {"value": ...}
                lines.append(f'        "{_to_camel(key)}": {{"value": {_json_scalar(val, js=True)}}}{trailing}{cmt_part}')
            elif child_msg and not isinstance(val, (dict, list)):
                # Scalar for a non-wrapper message — check if msg has one field that is itself a wrapper
                _sfwk = db.single_field_wrapper_key(child_msg)
                inner_key = _to_camel(_sfwk) if _sfwk else None
                if inner_key:
                    lines.append(f'        "{_to_camel(key)}": {{"{inner_key}": {{"value": {_json_scalar(val, js=True)}}}}}{trailing}{cmt_part}')
                else:
                    lines.append(f'        "{_to_camel(key)}": {_json_scalar(val, js=True)}{trailing}{cmt_part}')
            else:
                lines.append(f'        "{_to_camel(key)}": {_json_scalar(val, js=True)}{trailing}{cmt_part}')
    else:
        lines.append('        // No required fields')

    lines.append("    });")
    lines.append("")

    if flow_key == "authorize":
        lines.append(f"    if ({var_name}.status === 'FAILED') {{")
        lines.append(f"        throw new Error(`Payment failed: ${{{var_name}.error?.message}}`);")
        lines.append("    }")
        lines.append(f"    if ({var_name}.status === 'PENDING') {{")
        lines.append(f"        // Awaiting async confirmation — handle via webhook")
        lines.append(f"        return {{ status: 'pending', transactionId: {var_name}.connectorTransactionId }};")
        lines.append("    }")
        lines.append("")
    elif flow_key == "setup_recurring":
        lines.append(f"    if ({var_name}.status === 'FAILED') {{")
        lines.append(f"        throw new Error(`Recurring setup failed: ${{{var_name}.error?.message}}`);")
        lines.append("    }")
        lines.append("")
    elif flow_key in ("capture", "refund", "recurring_charge"):
        lines.append(f"    if ({var_name}.status === 'FAILED') {{")
        lines.append(f"        throw new Error(`{flow_key.title()} failed: ${{{var_name}.error?.message}}`);")
        lines.append("    }")
        lines.append("")

    return lines


def _scenario_return_javascript(scenario: ScenarioSpec) -> str:
    if scenario.key == "checkout_card":
        return "    return { status: captureResponse.status, transactionId: authorizeResponse.connectorTransactionId, error: authorizeResponse.error };"
    elif scenario.key in ("checkout_autocapture", "checkout_wallet", "checkout_bank"):
        return "    return { status: authorizeResponse.status, transactionId: authorizeResponse.connectorTransactionId, error: authorizeResponse.error };"
    elif scenario.key == "refund":
        return "    return { status: refundResponse.status, error: refundResponse.error };"
    elif scenario.key == "recurring":
        return "    return { status: recurringResponse.status, transactionId: recurringResponse.connectorTransactionId ?? '', error: recurringResponse.error };"
    elif scenario.key == "void_payment":
        return "    return { status: voidResponse.status, transactionId: authorizeResponse.connectorTransactionId, error: voidResponse.error };"
    elif scenario.key == "get_payment":
        return "    return { status: getResponse.status, transactionId: getResponse.connectorTransactionId, error: getResponse.error };"
    elif scenario.key == "create_customer":
        return "    return { customerId: createResponse.connectorCustomerId, error: createResponse.error };"
    elif scenario.key == "tokenize":
        return "    return { token: tokenizeResponse.paymentMethodToken, error: tokenizeResponse.error };"
    elif scenario.key == "authentication":
        return "    return { status: postAuthenticateResponse.status, error: postAuthenticateResponse.error };"
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
    if flow_key == "authorize":
        lines.append(f'{pad}match {var_name}.status() {{')
        lines.append(f'{pad}    PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => return Err(format!("Payment failed: {{:?}}", {var_name}.error).into()),')
        lines.append(f'{pad}    PaymentStatus::Pending => return Ok("pending — awaiting webhook".to_string()),')
        lines.append(f'{pad}    _                      => {{}},')
        lines.append(f'{pad}}}')
        lines.append("")
    elif flow_key == "setup_recurring":
        lines.append(f'{pad}if {var_name}.status() == PaymentStatus::Failure {{')
        lines.append(f'{pad}    return Err(format!("Setup failed: {{:?}}", {var_name}.error).into());')
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
    return lines


def _scenario_step_rust(
    scenario_key: str,
    flow_key: str,
    step_num: int,
    payload: dict,
    grpc_req: str,
    message_schemas: dict,
) -> list[str]:
    """Return Rust lines for one step inside a scenario function body (indent=1)."""
    pad  = "    "
    pad2 = "        "
    var_name = _FLOW_VAR_NAME.get(flow_key, f"{flow_key.split('_')[0]}_response")
    desc     = _STEP_DESCRIPTIONS.get(flow_key, flow_key)

    # Collect dynamic JSON-format overrides for this (scenario, flow)
    dyn_by_field: dict[str, list[str]] = {}
    for (sk, fk, field_name), raw_lines in _DYNAMIC_FIELDS_RS_JSON.items():
        if sk == scenario_key and fk == flow_key:
            dyn_by_field[field_name] = raw_lines

    drop_fields    = _SCENARIO_DROP_FIELDS.get((scenario_key, flow_key), frozenset())
    static_payload = {k: v for k, v in payload.items() if k not in drop_fields and k not in dyn_by_field}

    lines: list[str] = []
    lines.append(f"{pad}// Step {step_num}: {desc}")
    lines.append(f"{pad}let {var_name} = client.{flow_key}(serde_json::from_value::<{grpc_req}>(serde_json::json!({{")
    for json_line in _rust_json_lines(static_payload, grpc_req, message_schemas, indent=2):
        lines.append(json_line)
    for raw_lines in dyn_by_field.values():
        for raw_line in raw_lines:
            lines.append(f"{pad2}{raw_line}")
    lines.append(f"{pad}}})).unwrap_or_default(), &HashMap::new(), None).await?;")
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
    return '    Ok("done".to_string())'


def render_scenario_javascript(
    scenario: ScenarioSpec,
    connector_name: str,
    flow_payloads: dict[str, dict],
    flow_metadata: dict[str, dict],
    message_schemas: dict,
) -> str:
    """Return the full content of a runnable JavaScript scenario file."""
    db        = _SchemaDB(message_schemas)
    conn_enum = _conn_enum(connector_name)
    func_name = _to_camel(f"process_{scenario.key}")

    # Collect unique services and their client classes
    service_names: list[str] = []
    for fk in scenario.flows:
        svc = flow_metadata.get(fk, {}).get("service_name", "PaymentService")
        if svc not in service_names:
            service_names.append(svc)

    client_names = [_client_class(svc) for svc in service_names]
    client_imports = ", ".join(client_names)

    body_lines: list[str] = []

    # Instantiate clients
    for svc in service_names:
        cls     = _client_class(svc)
        var     = cls[0].lower() + cls[1:].replace("Client", "Client")  # camelCase var
        var     = var[0].lower() + var[1:]  # ensure starts lowercase
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

        body_lines.extend(_scenario_step_javascript(
            scenario.key, flow_key, step_num, payload, grpc_req, db, client_var
        ))

    body_lines.append(_scenario_return_javascript(scenario))
    body = "\n".join(body_lines)

    return f"""\
// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py {connector_name}
//
// Scenario: {scenario.title}
// {scenario.description}
'use strict';

const {{ {client_imports} }} = require('hs-playlib');
const {{ ConnectorConfig, Environment, Connector }} = require('hs-playlib').types;

const _defaultConfig = ConnectorConfig.create({{
    connector: Connector.{conn_enum},
    environment: Environment.SANDBOX,
}});
// Standalone credentials (field names depend on connector auth type):
// _defaultConfig.auth = {{ {connector_name}: {{ apiKey: {{ value: 'YOUR_API_KEY' }} }} }};


async function {func_name}(merchantTransactionId, config = _defaultConfig) {{
    // {scenario.title}
    // {scenario.description}

{body}
}}

module.exports = {{ {func_name} }};

if (require.main === module) {{
    {func_name}('order_001').catch(console.error);
}}
"""


# ── Per-language builder function generators ──────────────────────────────────

def _py_builder_fn(flow_key: str, proto_req: dict, grpc_req: str, db: "_SchemaDB") -> str:
    """Return a Python private builder function for the given flow."""
    param_name = _FLOW_BUILDER_EXTRA_PARAM[flow_key][0]  # snake_case
    items = list(proto_req.items())
    lines: list[str] = [f"def _build_{flow_key}_request({param_name}: str):"]
    lines.append("    return ParseDict(")
    lines.append("        {")
    for idx, (key, val) in enumerate(items):
        trailing  = "," if idx < len(items) - 1 else ""
        comment   = db.get_comment(grpc_req, key)
        child_msg = db.get_type(grpc_req, key)
        cmt_part  = f"  # {comment}" if comment else ""
        if key == param_name:
            lines.append(f'            "{key}": {param_name}{trailing}{cmt_part}')
        elif isinstance(val, dict):
            lines.append(f'            "{key}": {{{cmt_part}')
            lines.extend(_annotate_inline_lines(val, child_msg, db, indent=4, cmt="#"))
            lines.append(f'            }}{trailing}')
        elif child_msg and db.is_wrapper(child_msg):
            lines.append(f'            "{key}": {{"value": {_json_scalar(val)}}}{trailing}{cmt_part}')
        elif child_msg and not isinstance(val, (dict, list)):
            inner_key = db.single_field_wrapper_key(child_msg)
            if inner_key:
                lines.append(f'            "{key}": {{"{inner_key}": {{"value": {_json_scalar(val)}}}}}{trailing}{cmt_part}')
            else:
                lines.append(f'            "{key}": {_json_scalar(val)}{trailing}{cmt_part}')
        else:
            lines.append(f'            "{key}": {_json_scalar(val)}{trailing}{cmt_part}')
    lines.append("        },")
    if grpc_req:
        lines.append(f"        payment_pb2.{grpc_req}(),")
    lines.append("    )")
    return "\n".join(lines)


def _js_builder_fn(flow_key: str, proto_req: dict, grpc_req: str, db: "_SchemaDB") -> str:
    """Return a JavaScript private builder function for the given flow."""
    param_name = _FLOW_BUILDER_EXTRA_PARAM[flow_key][0]  # snake_case
    js_param   = _to_camel(param_name)
    fn_name    = "_build" + "".join(w.title() for w in flow_key.split("_")) + "Request"
    items      = list(proto_req.items())
    lines: list[str] = [f"function {fn_name}({js_param}) {{"]
    lines.append("    return {")
    for idx, (key, val) in enumerate(items):
        trailing  = "," if idx < len(items) - 1 else ""
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
    lines.append("    };")
    lines.append("}")
    return "\n".join(lines)


def _py_builder_fn_no_param(flow_key: str, proto_req: dict, grpc_req: str, db: "_SchemaDB") -> str:
    """Return a Python private builder function with no dynamic parameter (arg: none flows)."""
    items = list(proto_req.items())
    lines: list[str] = [f"def _build_{flow_key}_request():"]
    lines.append("    return ParseDict(")
    lines.append("        {")
    for idx, (key, val) in enumerate(items):
        trailing  = "," if idx < len(items) - 1 else ""
        comment   = db.get_comment(grpc_req, key)
        child_msg = db.get_type(grpc_req, key)
        cmt_part  = f"  # {comment}" if comment else ""
        if isinstance(val, dict):
            lines.append(f'            "{key}": {{{cmt_part}')
            lines.extend(_annotate_inline_lines(val, child_msg, db, indent=4, cmt="#"))
            lines.append(f'            }}{trailing}')
        elif child_msg and db.is_wrapper(child_msg):
            lines.append(f'            "{key}": {{"value": {_json_scalar(val)}}}{trailing}{cmt_part}')
        elif child_msg and not isinstance(val, (dict, list)):
            inner_key = db.single_field_wrapper_key(child_msg)
            if inner_key:
                lines.append(f'            "{key}": {{"{inner_key}": {{"value": {_json_scalar(val)}}}}}{trailing}{cmt_part}')
            else:
                lines.append(f'            "{key}": {_json_scalar(val)}{trailing}{cmt_part}')
        else:
            lines.append(f'            "{key}": {_json_scalar(val)}{trailing}{cmt_part}')
    lines.append("        },")
    if grpc_req:
        lines.append(f"        payment_pb2.{grpc_req}(),")
    lines.append("    )")
    return "\n".join(lines)


def _js_builder_fn_no_param(flow_key: str, proto_req: dict, grpc_req: str, db: "_SchemaDB") -> str:
    """Return a JavaScript private builder function with no dynamic parameter (arg: none flows)."""
    fn_name = "_build" + "".join(w.title() for w in flow_key.split("_")) + "Request"
    items   = list(proto_req.items())
    lines: list[str] = [f"function {fn_name}() {{"]
    lines.append("    return {")
    for idx, (key, val) in enumerate(items):
        trailing  = "," if idx < len(items) - 1 else ""
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
    scenarios_with_payloads: list[tuple["ScenarioSpec", dict[str, dict]]],
    flow_metadata: dict[str, dict],
    message_schemas: dict,
    flow_items: "list[tuple[str, dict, str]] | None" = None,
) -> str:
    """Return one Python file containing all scenario functions (and flow functions) for a connector."""
    db        = _SchemaDB(message_schemas)
    conn_enum = _conn_enum(connector_name)

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
        func_names.append(flow_key)
        func_blocks.append(
            f"async def {flow_key}(merchant_transaction_id: str, "
            f"config: sdk_config_pb2.ConnectorConfig = _default_config):\n"
            f'    """Flow: {svc}.{rpc_name}{pm_part}"""\n'
            f"{body}\n"
        )

    builders_text  = "\n\n".join(builder_fns)
    builders_section = f"\n\n{builders_text}\n" if builder_fns else ""
    functions_text = "\n\n".join(func_blocks)
    first_scenario = func_names[0][8:] if func_names[0].startswith("process_") else func_names[0] if func_names else "checkout_autocapture"

    return f"""\
# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py {connector_name}
#
# {connector_name.title()} — all integration scenarios and flows in one file.
# Run a scenario:  python3 {connector_name}.py checkout_card

import asyncio
import sys
from google.protobuf.json_format import ParseDict
{client_imports}
from payments.generated import sdk_config_pb2, payment_pb2

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
)
# Standalone credentials (field names depend on connector auth type):
# _default_config.connector_config.CopyFrom(payment_pb2.ConnectorSpecificConfig(
#     {connector_name}=payment_pb2.{connector_name.title()}Config(api_key=...),
# ))


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
    scenarios_with_payloads: list[tuple["ScenarioSpec", dict[str, dict]]],
    flow_metadata: dict[str, dict],
    message_schemas: dict,
    flow_items: "list[tuple[str, dict, str]] | None" = None,
) -> str:
    """Return one JavaScript file containing all scenario functions (and flow functions) for a connector."""
    db        = _SchemaDB(message_schemas)
    conn_enum = _conn_enum(connector_name)

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
            js_builder_fns.append(_js_builder_fn(flow_key, proto_req, grpc_req_b, db))
        else:
            js_builder_fns.append(_js_builder_fn_no_param(flow_key, proto_req, grpc_req_b, db))
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
                js_builder_fns.append(_js_builder_fn(fk, proto_req, grpc_req_b, db))
            else:
                js_builder_fns.append(_js_builder_fn_no_param(fk, proto_req, grpc_req_b, db))
            js_has_builder.add(fk)

    _js_var_defaults = {k: _to_camel(v.replace("_response", "Response")) for k, v in _FLOW_VAR_NAME.items()}

    def _js_step_with_builder(scenario_key: str, flow_key: str, step_num: int,
                               client_var: str) -> list[str]:
        """Return JS body lines for a scenario step using a pre-built builder fn."""
        method   = _to_camel(_FLOW_KEY_TO_METHOD.get(flow_key, flow_key))
        var_name = _js_var_defaults.get(flow_key, f"{flow_key.split('_')[0]}Response")
        desc     = _STEP_DESCRIPTIONS.get(flow_key, flow_key)
        fn_name  = "_build" + "".join(w.title() for w in flow_key.split("_")) + "Request"

        if flow_key == "authorize" and scenario_key in _CARD_AUTHORIZE_SCENARIOS:
            cm = {"checkout_card": "MANUAL", "void_payment": "MANUAL",
                  "get_payment": "MANUAL", "refund": "AUTOMATIC"}.get(scenario_key, "AUTOMATIC")
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

        func_blocks.append(
            f"// {scenario.title}\n"
            f"// {scenario.description}\n"
            f"async function {func_name}(merchantTransactionId, config = _defaultConfig) {{\n"
            f"{body}\n"
            f"}}"
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
                call_expr   = f"{fn_name}('{default_val}')"
            else:
                call_expr   = f"{fn_name}()"
            body_lines  = [
                f"    const {client_var} = new {cls}(config);",
                "",
                f"    const {var_name} = await {client_var}.{method}({call_expr});",
                "",
            ]
        else:
            body_lines = list(_scenario_step_javascript("_standalone_", flow_key, 1, proto_req, grpc_req, db, client_var))
        if flow_key == "authorize":
            body_lines.append(f"    return {{ status: {var_name}.status, transactionId: {var_name}.connectorTransactionId }};")
        elif flow_key == "setup_recurring":
            body_lines.append(f"    return {{ status: {var_name}.status, mandateId: {var_name}.connectorTransactionId }};")
        else:
            body_lines.append(f"    return {{ status: {var_name}.status }};")

        body = "\n".join(body_lines)
        func_names_js.append(func_name)
        func_blocks.append(
            f"// Flow: {svc}.{rpc_name}{pm_part}\n"
            f"async function {func_name}(merchantTransactionId, config = _defaultConfig) {{\n"
            f"{body}\n"
            f"}}"
        )

    # Also export _build*Request helpers so the gRPC smoke test can call them directly.
    builder_export_names = [
        "_build" + "".join(w.title() for w in fk.split("_")) + "Request"
        for fk in sorted(js_has_builder)
    ]
    exports          = ", ".join(func_names_js + builder_export_names)
    js_builders_text = "\n\n".join(js_builder_fns)
    js_builders_section = f"\n\n{js_builders_text}\n" if js_builder_fns else ""
    funcs_text       = "\n\n".join(func_blocks)
    first_scenario   = scenarios_with_payloads[0][0].key if scenarios_with_payloads else "checkout_autocapture"

    return f"""\
// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py {connector_name}
//
// {connector_name.title()} — all integration scenarios and flows in one file.
// Run a scenario:  node {connector_name}.js {first_scenario}
'use strict';

const {{ {client_imports} }} = require('hs-playlib');
const {{ ConnectorConfig, ConnectorSpecificConfig, SdkOptions, Environment }} = require('hs-playlib').types;

const _defaultConfig = ConnectorConfig.create({{
    options: SdkOptions.create({{ environment: Environment.SANDBOX }}),
}});
// Standalone credentials (field names depend on connector auth type):
// _defaultConfig.connectorConfig = ConnectorSpecificConfig.create({{
//     {connector_name}: {{ apiKey: {{ value: 'YOUR_API_KEY' }} }}
// }});
{js_builders_section}

// ANCHOR: scenario_functions
{funcs_text}


module.exports = {{ {exports} }};

if (require.main === module) {{
    const scenario = process.argv[2] || '{first_scenario}';
    const key = 'process' + scenario.replace(/_([a-z])/g, (_, l) => l.toUpperCase()).replace(/^(.)/, c => c.toUpperCase());
    const fn = module.exports[key];
    if (!fn) {{
        const available = Object.keys(module.exports).map(k =>
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
    base_py = f"../../examples/{connector_name}/python/{connector_name}.py"
    base_js = f"../../examples/{connector_name}/javascript/{connector_name}.js"
    base_kt = f"../../examples/{connector_name}/kotlin/{connector_name}.kt"
    base_rs = f"../../examples/{connector_name}/rust/{connector_name}.rs"
    
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


# ── Public API: Per-flow example file renderers ────────────────────────────────

def render_flow_python(
    flow_key: str,
    connector_name: str,
    proto_req: dict,
    flow_metadata: dict[str, dict],
    message_schemas: dict,
    pm_label: str = "",
) -> str:
    """Return the full content of a runnable Python file for a single flow."""
    db         = _SchemaDB(message_schemas)
    meta       = flow_metadata.get(flow_key, {})
    svc        = meta.get("service_name", "PaymentService")
    grpc_req   = meta.get("grpc_request", "")
    rpc_name   = meta.get("rpc_name", flow_key)
    conn_enum  = _conn_enum(connector_name)
    client_cls = _client_class(svc)
    client_var = client_cls.lower().replace("client", "_client")

    body_lines: list[str] = [f"    {client_var} = {client_cls}(config)", ""]
    body_lines.extend(_scenario_step_python("_standalone_", flow_key, 1, proto_req, grpc_req, client_var, db))

    resp_var = f"{flow_key.split('_')[0]}_response"
    if flow_key == "authorize":
        body_lines.append(f'    return {{"status": {resp_var}.status, "transaction_id": {resp_var}.connector_transaction_id}}')
    elif flow_key == "setup_recurring":
        body_lines.append(f'    return {{"status": {resp_var}.status, "mandate_id": {resp_var}.connector_transaction_id}}')
    else:
        body_lines.append(f'    return {{"status": {resp_var}.status}}')

    body      = "\n".join(body_lines)
    svc_label = f"{svc}.{rpc_name}"
    pm_part   = f" ({pm_label})" if pm_label else ""

    return f"""\
# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py {connector_name}
#
# Flow: {svc_label}{pm_part}

import asyncio
from google.protobuf.json_format import ParseDict
from payments import {client_cls}
from payments.generated import sdk_config_pb2, payment_pb2

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
)
# Standalone credentials (field names depend on connector auth type):
# _default_config.connector_config.CopyFrom(payment_pb2.ConnectorSpecificConfig(
#     {connector_name}=payment_pb2.{connector_name.title()}Config(api_key=...),
# ))


async def {flow_key}(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
{body}


if __name__ == "__main__":
    asyncio.run({flow_key}("order_001"))
"""


def render_flow_javascript(
    flow_key: str,
    connector_name: str,
    proto_req: dict,
    flow_metadata: dict[str, dict],
    message_schemas: dict,
    pm_label: str = "",
) -> str:
    """Return the full content of a runnable JavaScript file for a single flow."""
    db           = _SchemaDB(message_schemas)
    meta         = flow_metadata.get(flow_key, {})
    svc          = meta.get("service_name", "PaymentService")
    grpc_req     = meta.get("grpc_request", "")
    rpc_name     = meta.get("rpc_name", flow_key)
    conn_display = _conn_display(connector_name)
    _JS_RESERVED = JS_RESERVED
    func_name    = (_to_camel(flow_key) if flow_key not in _JS_RESERVED else f"{flow_key}Payment")
    var_name     = f"{flow_key.split('_')[0]}Response"

    body_lines: list[str] = list(_scenario_step_javascript("_standalone_", flow_key, 1, proto_req, grpc_req, db))

    if flow_key == "authorize":
        body_lines.append(f"    return {{ status: {var_name}.status, transactionId: {var_name}.connector_transaction_id }};")
    elif flow_key == "setup_recurring":
        body_lines.append(f"    return {{ status: {var_name}.status, mandateId: {var_name}.connector_transaction_id }};")
    else:
        body_lines.append(f"    return {{ status: {var_name}.status }};")

    body      = "\n".join(body_lines)
    svc_label = f"{svc}.{rpc_name}"
    pm_part   = f" ({pm_label})" if pm_label else ""

    return f"""\
// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py {connector_name}
//
// Flow: {svc_label}{pm_part}

const {{ ConnectorClient }} = require('connector-service-node-ffi');

const client = new ConnectorClient({{
    connector: '{conn_display}',
    environment: 'sandbox',
    connector_auth_type: {{
        header_key: {{ api_key: 'YOUR_API_KEY' }},
    }},
}});

async function {func_name}(merchantTransactionId) {{
{body}
}}

{func_name}("order_001").catch(console.error);
"""


# Per-flow status block for flows that don't have a PaymentStatus status field.
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


def render_flow_kotlin(
    flow_key: str,
    connector_name: str,
    proto_req: dict,
    flow_metadata: dict[str, dict],
    message_schemas: dict,
    pm_label: str = "",
) -> str:
    """Return the full content of a runnable Kotlin file for a single flow."""
    meta       = flow_metadata.get(flow_key, {})
    svc        = meta.get("service_name", "PaymentService")
    grpc_req   = meta.get("grpc_request", "")
    rpc_name   = meta.get("rpc_name", flow_key)
    conn_enum  = _conn_enum(connector_name)
    client_cls = _client_class(svc)
    # Kotlin SDK uses snake_case method names; recurring_charge → charge
    method     = _FLOW_KEY_TO_METHOD.get(flow_key, flow_key)

    processed_req = _preprocess_kt_payload(flow_key, proto_req)
    body_lines = _kotlin_payload_lines(processed_req, grpc_req, message_schemas, indent=2)
    body       = "\n".join(body_lines)

    svc_label  = f"{svc}.{rpc_name}"
    pm_part    = f" ({pm_label})" if pm_label else ""

    # Status handling for flows that return payment status
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

    return f"""\
// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py {connector_name}
//
// Flow: {svc_label}{pm_part}
//
// SDK: sdk/java (Kotlin/JVM — uses UniFFI protobuf builder pattern)
// Build: ./gradlew compileKotlin  (from sdk/java/)

import payments.{client_cls}
import payments.ConnectorConfig
import payments.Connector
import payments.Environment

fun main() {{
    val config = ConnectorConfig.newBuilder()
        .setConnector(Connector.{conn_enum})
        .setEnvironment(Environment.SANDBOX)
        // .setAuth(...) — set your connector auth here
        .build()

    val client = {client_cls}(config)

    val request = {grpc_req}.newBuilder().apply {{
{body}
    }}.build()

    val response = client.{method}(request)
{status_block}
}}
"""


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
            if not child_msg:
                # Unknown field — likely a oneof group name (e.g. mandate_id_type).
                # Flatten by processing inner fields at the current message level.
                lines.extend(_kotlin_payload_lines(val, msg_name, message_schemas, indent, variable_fields, variable_field_values))
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


def render_flow_rust(
    flow_key: str,
    connector_name: str,
    proto_req: dict,
    flow_metadata: dict[str, dict],
    message_schemas: dict,
    pm_label: str = "",
) -> str:
    """Return the full content of a runnable Rust file for a single flow."""
    meta      = flow_metadata.get(flow_key, {})
    svc       = meta.get("service_name", "PaymentService")
    grpc_req  = meta.get("grpc_request", "")
    rpc_name  = meta.get("rpc_name", flow_key)
    conn_enum = _conn_enum_rust(connector_name)
    method    = flow_key  # Rust uses snake_case

    json_lines = _rust_json_lines(proto_req, grpc_req, message_schemas, indent=1)
    json_body  = "\n".join(json_lines)

    svc_label = f"{svc}.{rpc_name}"
    pm_part   = f" ({pm_label})" if pm_label else ""

    if flow_key == "authorize":
        status_block = (
            '    match response.status() {\n'
            '        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed\n'
            '            => eprintln!("Authorize failed: {:?}", response.error),\n'
            '        PaymentStatus::Pending => println!("Pending — await webhook"),\n'
            '        _  => println!("Authorized: {}", response.connector_transaction_id.as_deref().unwrap_or("")),\n'
            '    }'
        )
    elif flow_key == "setup_recurring":
        status_block = (
            '    if response.status() == PaymentStatus::Failure {\n'
            '        eprintln!("Setup failed: {:?}", response.error);\n'
            '    } else {\n'
            '        println!("Mandate: {}", response.connector_recurring_payment_id.as_deref().unwrap_or(""));\n'
            '    }'
        )
    elif flow_key == "tokenize":
        status_block = '    println!("token: {}", response.payment_method_token);'
    elif flow_key == "create_customer":
        status_block = '    println!("customer_id: {}", response.connector_customer_id);'
    elif flow_key in ("dispute_accept", "dispute_defend", "dispute_submit_evidence"):
        status_block = '    println!("dispute_status: {:?}", response.dispute_status());'
    else:
        status_block = '    println!("Status: {:?}", response.status());'

    return f"""\
// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py {connector_name}
//
// Flow: {svc_label}{pm_part}
//
// SDK: sdk/rust (native Rust — uses hyperswitch_payments_client)
// Build: cargo check -p hyperswitch-payments-client  (from repo root)

use grpc_api_types::payments::*;
use hyperswitch_payments_client::ConnectorClient;
use std::collections::HashMap;

#[tokio::main]
async fn main() {{
    let config = ConnectorConfig {{
        connector: Connector::{conn_enum}.into(),
        environment: Environment::Sandbox.into(),
        // auth: Some(ConnectorAuth {{ ... }})  — set your connector auth here
        ..Default::default()
    }};

    let client = ConnectorClient::new(config, None).unwrap();

    // Build request from probe-verified field values via serde_json deserialization.
    // See sdk/rust/examples/basic.rs for the type-safe struct construction pattern.
    let response = client.{method}(
        serde_json::from_value::<{grpc_req}>(serde_json::json!({{
{json_body}
        }})).unwrap_or_default(),
        &HashMap::new(), None,
    ).await.unwrap();
{status_block}
}}
"""


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
            if child_msg and child_msg in _ONEOF_WRAPPER_FIELD:
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
        # Render just the payment_method sub-object
        pm_msg = db.get_type(grpc_req, "payment_method")
        annotated = _build_annotated(pm_payload, pm_msg, db, style="python", indent=0)
        a("```python")
        a(f'"payment_method": {annotated}')
        a("```")
        a("")

    return out


# ── Public API: Payload block (Flow Reference) ─────────────────────────────────

def render_payload_block(
    flow_key: str,
    service_name: str,
    grpc_request: str,
    proto_request: dict,
    message_schemas: dict,
) -> list[str]:
    """
    Return markdown lines for a single annotated request payload block.
    Used in the Flow Reference section.
    """
    if not proto_request or not grpc_request:
        return []

    db           = _SchemaDB(message_schemas)
    client_cls   = _client_class(service_name)
    camel_method = _to_camel(flow_key)
    payload      = _build_annotated(proto_request, grpc_request, db, style="python", indent=0)

    return [
        "",
        f"> **Client call:** `{client_cls}.{camel_method}(request)`",
        "",
        "```python",
        payload,
        "```",
        "",
    ]


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

    all_import_types = all_client_cls + grpc_req_types + sorted(enum_types)
    imports = "\n".join(f"import payments.{t}" for t in all_import_types)

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
    when_branches = "\n".join(f'        "{n}" -> {n}(txnId)' for n in func_names)
    first         = func_names[0] if func_names else "authorize"

    when_branches_main = "\n".join(f'        "{n}" -> {n}(txnId)' for n in func_names)

    return f"""\
// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py {connector_name}
//
// {connector_name.title()} — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="{connector_name} processCheckoutCard"

package examples.{connector_name}

{imports}
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment
{kt_builders_section}
val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your connector config here
    .build()


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

    # Pass 1: flows from flow_items
    for flow_key, proto_req, pm_label in flow_items:
        grpc_req_b = flow_metadata.get(flow_key, {}).get("grpc_request", "")
        if not grpc_req_b:
            continue
        if flow_key in _FLOW_BUILDER_EXTRA_PARAM:
            param_name, param_type = _FLOW_BUILDER_EXTRA_PARAM[flow_key]
            json_lines_b = _rust_json_lines(
                proto_req, grpc_req_b, message_schemas, indent=1,
                variable_fields=frozenset({param_name}),
            )
            json_body_b = "\n".join(json_lines_b)
            builder_fns.append(
                f"pub fn build_{flow_key}_request({param_name}: {param_type}) -> {grpc_req_b} {{\n"
                f"    serde_json::from_value::<{grpc_req_b}>(serde_json::json!({{\n"
                f"{json_body_b}\n"
                f"    }})).unwrap_or_default()\n"
                f"}}"
            )
            has_builder.add(flow_key)
        else:
            json_lines_b = _rust_json_lines(proto_req, grpc_req_b, message_schemas, indent=1)
            json_body_b = "\n".join(json_lines_b)
            builder_fns.append(
                f"pub fn build_{flow_key}_request() -> {grpc_req_b} {{\n"
                f"    serde_json::from_value::<{grpc_req_b}>(serde_json::json!({{\n"
                f"{json_body_b}\n"
                f"    }})).unwrap_or_default()\n"
                f"}}"
            )
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
                json_lines_b = _rust_json_lines(
                    proto_req, grpc_req_b, message_schemas, indent=1,
                    variable_fields=frozenset({param_name}),
                )
                json_body_b = "\n".join(json_lines_b)
                builder_fns.append(
                    f"pub fn build_{fk}_request({param_name}: {param_type}) -> {grpc_req_b} {{\n"
                    f"    serde_json::from_value::<{grpc_req_b}>(serde_json::json!({{\n"
                    f"{json_body_b}\n"
                    f"    }})).unwrap_or_default()\n"
                    f"}}"
                )
                has_builder.add(fk)
            else:
                json_lines_b = _rust_json_lines(proto_req, grpc_req_b, message_schemas, indent=1)
                json_body_b = "\n".join(json_lines_b)
                builder_fns.append(
                    f"pub fn build_{fk}_request() -> {grpc_req_b} {{\n"
                    f"    serde_json::from_value::<{grpc_req_b}>(serde_json::json!({{\n"
                    f"{json_body_b}\n"
                    f"    }})).unwrap_or_default()\n"
                    f"}}"
                )
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
            f"{pad}let {var_name} = client.{flow_key}({builder_call}, &HashMap::new(), None).await?;",
            "",
        ]
        step_lines.extend(_rust_status_check_lines(flow_key, var_name, pad))
        return step_lines

    # Generate scenario function blocks first
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
    for flow_key, proto_req, pm_label in flow_items:
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
        elif flow_key in ("create_access_token", "create_session_token"):
            status_block = '    Ok(format!("Session token obtained (statusCode={})", response.status_code))'
        else:
            status_block = '    Ok(format!("status: {:?}", response.status()))'

        func_names.append(flow_key)
        match_arms.append(f'        "{flow_key}" => {flow_key}(&client, "order_001").await,')

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
                f"pub async fn {flow_key}(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {{\n"
                f"    let response = client.{flow_key}({builder_call}, &HashMap::new(), None).await?;\n"
                f"{status_block}\n"
                f"}}"
            )
        elif flow_key in has_no_param_builder:
            func_blocks.append(
                f"// Flow: {svc}.{rpc_name}{pm_part}\n"
                f"#[allow(dead_code)]\n"
                f"pub async fn {flow_key}(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {{\n"
                f"    let response = client.{flow_key}(build_{flow_key}_request(), &HashMap::new(), None).await?;\n"
                f"{status_block}\n"
                f"}}"
            )
        else:
            json_lines = _rust_json_lines(proto_req, grpc_req, message_schemas, indent=1)
            json_body  = "\n".join(json_lines)
            func_blocks.append(
                f"// Flow: {svc}.{rpc_name}{pm_part}\n"
                f"#[allow(dead_code)]\n"
                f"pub async fn {flow_key}(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {{\n"
                f"    let response = client.{flow_key}(serde_json::from_value::<{grpc_req}>(serde_json::json!({{\n"
                f"{json_body}\n"
                f"    }})).unwrap_or_default(), &HashMap::new(), None).await?;\n"
                f"{status_block}\n"
                f"}}"
            )

    funcs_text     = "\n\n".join(func_blocks)
    builders_text  = "\n\n".join(builder_fns)
    builders_section = f"\n\n{builders_text}\n" if builder_fns else ""
    match_arms_str = "\n".join(match_arms)
    first          = func_names[0] if func_names else "authorize"

    return f"""\
// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py {connector_name}
//
// {connector_name.title()} — all scenarios and flows in one file.
// Run a scenario:  cargo run --example {connector_name} -- process_checkout_card

use grpc_api_types::payments::*;
use hyperswitch_payments_client::ConnectorClient;
use std::collections::HashMap;

#[allow(dead_code)]
fn build_client() -> ConnectorClient {{
    // Set connector_config to authenticate: use ConnectorSpecificConfig with your {conn_enum}Config
    let config = ConnectorConfig {{
        connector_config: None,  // TODO: Some(ConnectorSpecificConfig {{ config: Some(...) }})
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
    example_paths = [f"examples/{connector_name}/python/{connector_name}.py"] if scenario_keys else []

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
