#!/usr/bin/env python3
"""
Connector Documentation Generator

Generates connector documentation from field-probe JSON output.

Usage:
    python3 scripts/generators/docs/generate.py stripe adyen
    python3 scripts/generators/docs/generate.py --all
    python3 scripts/generators/docs/generate.py --list

How it works:
  1. Loads probe data from data/field_probe/{connector}.json
  2. All content is derived exclusively from probe data — no manual annotation files
  3. Outputs docs-generated/connectors/{name}.md

To add docs for a new connector:
  - Run field-probe to generate probe data: cd crates/internal/field-probe && cargo r
  - Run: python3 scripts/generators/docs/generate.py {name}
"""

import os
import sys
import json
import shutil
import subprocess
from pathlib import Path
from typing import Optional

sys.path.insert(0, str(Path(__file__).parent.parent))
from snippet_examples import generate as snippets

# ─── Probe Data ───────────────────────────────────────────────────────────────

# Flows that have PM-specific probe results (vs flows that only have a 'default' key)
_PM_AWARE_FLOWS = frozenset(["authorize"])

# Global flow metadata loaded from probe.json (populated by load_probe_data)
_FLOW_METADATA: list[dict] = []

# Global message schemas from manifest (populated by load_probe_data)
_MESSAGE_SCHEMAS: dict = {}

# Global probe data indexed by connector name (populated by load_probe_data)
_PROBE_DATA: dict[str, dict] = {}


def get_flow_metadata() -> dict[str, dict]:
    """
    Get flow metadata as a dict keyed by flow_key.
    Returns {flow_key: {service_rpc, description, service_name, rpc_name}}
    """
    return {m["flow_key"]: m for m in _FLOW_METADATA}


def get_proto_flow_definitions() -> dict[str, tuple[str, str, str]]:
    """
    Get flow definitions in legacy format for compatibility.
    Returns {flow_key: (service_name, rpc_name, description)}
    """
    return {
        m["flow_key"]: (m["service_name"], m["rpc_name"], m["description"])
        for m in _FLOW_METADATA
    }


# Build reverse mapping from flow metadata (populated after load_probe_data)
def _build_flow_name_to_key_mapping() -> dict[str, str]:
    """
    Build mapping from RPC name to flow_key using loaded flow_metadata.
    E.g., "Authorize" -> "authorize", "Get" -> "get"
    """
    mapping = {}
    for m in _FLOW_METADATA:
        rpc_name = m.get("rpc_name", "")
        flow_key = m.get("flow_key", "")
        if rpc_name and flow_key:
            mapping[rpc_name] = flow_key
    return mapping


def get_flow_name_to_key() -> dict[str, str]:
    """Get the flow name to probe key mapping from loaded metadata."""
    if not hasattr(get_flow_name_to_key, '_cache'):
        get_flow_name_to_key._cache = _build_flow_name_to_key_mapping()
    return get_flow_name_to_key._cache

# Payment methods grouped by category for better readability in documentation
# Format: (category_name, [(pm_key, display_name), ...])
_PROBE_PM_BY_CATEGORY: list[tuple[str, list[tuple[str, str]]]] = [
    ("Card", [
        ("Card", "Card"),
        ("BancontactCard", "Bancontact"),
    ]),
    ("Wallet", [
        ("ApplePay", "Apple Pay"),
        ("ApplePayDecrypted", "Apple Pay Dec"),
        ("ApplePayThirdPartySdk", "Apple Pay SDK"),
        ("GooglePay", "Google Pay"),
        ("GooglePayDecrypted", "Google Pay Dec"),
        ("GooglePayThirdPartySdk", "Google Pay SDK"),
        ("PaypalSdk", "PayPal SDK"),
        ("AmazonPayRedirect", "Amazon Pay"),
        ("CashappQr", "Cash App"),
        ("PaypalRedirect", "PayPal"),
        ("WeChatPayQr", "WeChat Pay"),
        ("AliPayRedirect", "Alipay"),
        ("RevolutPay", "Revolut Pay"),
        ("Mifinity", "MiFinity"),
        ("Bluecode", "Bluecode"),
        ("Paze", "Paze"),
        ("SamsungPay", "Samsung Pay"),
        ("MbWay", "MB Way"),
        ("Satispay", "Satispay"),
        ("Wero", "Wero"),
    ]),
    ("BNPL", [
        ("Affirm", "Affirm"),
        ("Afterpay", "Afterpay"),
        ("Klarna", "Klarna"),
    ]),
    ("UPI", [
        ("UpiCollect", "UPI Collect"),
        ("UpiIntent", "UPI Intent"),
        ("UpiQr", "UPI QR"),
    ]),
    ("Online Banking", [
        ("OnlineBankingThailand", "Thailand"),
        ("OnlineBankingCzechRepublic", "Czech"),
        ("OnlineBankingFinland", "Finland"),
        ("OnlineBankingFpx", "FPX"),
        ("OnlineBankingPoland", "Poland"),
        ("OnlineBankingSlovakia", "Slovakia"),
    ]),
    ("Open Banking", [
        ("OpenBankingUk", "UK"),
        ("OpenBankingPis", "PIS"),
        ("OpenBanking", "Generic"),
    ]),
    ("Bank Redirect", [
        ("LocalBankRedirect", "Local"),
        ("Ideal", "iDEAL"),
        ("Sofort", "Sofort"),
        ("Trustly", "Trustly"),
        ("Giropay", "Giropay"),
        ("Eps", "EPS"),
        ("Przelewy24", "Przelewy24"),
        ("Pse", "PSE"),
        ("Blik", "BLIK"),
        ("Interac", "Interac"),
        ("Bizum", "Bizum"),
        ("Eft", "EFT"),
        ("DuitNow", "DuitNow"),
    ]),
    ("Bank Transfer", [
        ("AchBankTransfer", "ACH"),
        ("SepaBankTransfer", "SEPA"),
        ("BacsBankTransfer", "BACS"),
        ("MultibancoBankTransfer", "Multibanco"),
        ("InstantBankTransfer", "Instant"),
        ("InstantBankTransferFinland", "Instant FI"),
        ("InstantBankTransferPoland", "Instant PL"),
        ("Pix", "Pix"),
        ("PermataBankTransfer", "Permata"),
        ("BcaBankTransfer", "BCA"),
        ("BniVaBankTransfer", "BNI VA"),
        ("BriVaBankTransfer", "BRI VA"),
        ("CimbVaBankTransfer", "CIMB VA"),
        ("DanamonVaBankTransfer", "Danamon VA"),
        ("MandiriVaBankTransfer", "Mandiri VA"),
        ("LocalBankTransfer", "Local"),
        ("IndonesianBankTransfer", "Indonesian"),
    ]),
    ("Bank Debit", [
        ("Ach", "ACH"),
        ("Sepa", "SEPA"),
        ("Bacs", "BACS"),
        ("Becs", "BECS"),
        ("SepaGuaranteedDebit", "SEPA Guaranteed"),
    ]),
    ("Alternative", [
        ("Crypto", "Crypto"),
        ("ClassicReward", "Reward"),
        ("Givex", "Givex"),
        ("PaySafeCard", "PaySafeCard"),
        ("EVoucher", "E-Voucher"),
        ("Boleto", "Boleto"),
        ("Efecty", "Efecty"),
        ("PagoEfectivo", "Pago Efectivo"),
        ("RedCompra", "Red Compra"),
        ("RedPagos", "Red Pagos"),
        ("Alfamart", "Alfamart"),
        ("Indomaret", "Indomaret"),
        ("Oxxo", "Oxxo"),
        ("SevenEleven", "7-Eleven"),
        ("Lawson", "Lawson"),
        ("MiniStop", "Mini Stop"),
        ("FamilyMart", "Family Mart"),
        ("Seicomart", "Seicomart"),
        ("PayEasy", "Pay Easy"),
    ]),
]

# Flatten for backward compatibility
_PROBE_PM_DISPLAY: dict[str, str] = {}
for _category, pms in _PROBE_PM_BY_CATEGORY:
    _PROBE_PM_DISPLAY.update(dict(pms))


# Service name prefixes for flow key derivation.
# Most flows follow a standard convention:
#   - PaymentService.* -> just snake_case RPC name (e.g., "Authorize" -> "authorize")
#   - OtherService.* -> prefix + snake_case (e.g., "RefundService.Get" -> "refund_get")
# 
# EXCEPTIONS: Only add entries here when the auto-derived key doesn't match probe data
_FLOW_KEY_OVERRIDES: dict[tuple[str, str], str] = {
    # CustomerService.Create breaks the pattern (would be "customer_create")
    ("CustomerService", "Create"): "create_customer",
    # Eligibility is a short name that doesn't need prefix
    ("PaymentMethodService", "Eligibility"): "eligibility",
    # Tokenize is a short name that doesn't need prefix  
    ("PaymentMethodService", "Tokenize"): "tokenize",
    # EventService.HandleEvent should just be "handle_event" not "event_handle_event"
    ("EventService", "HandleEvent"): "handle_event",
    # EventService.ParseEvent should just be "parse_event" not "event_parse_event"
    ("EventService", "ParseEvent"): "parse_event",
    # VerifyRedirectResponse -> verify_redirect (truncated in probe data)
    ("PaymentService", "VerifyRedirectResponse"): "verify_redirect",
    # MerchantAuthenticationService flows (probe data doesn't use merchant_auth_ prefix)
    ("MerchantAuthenticationService", "CreateServerAuthenticationToken"): "create_server_authentication_token",
    ("MerchantAuthenticationService", "CreateServerSessionAuthenticationToken"): "create_server_session_authentication_token",
    ("MerchantAuthenticationService", "CreateClientAuthenticationToken"): "create_client_authentication_token",
    # PaymentMethodAuthenticationService flows (probe data doesn't use payment_method_auth_ prefix)
    ("PaymentMethodAuthenticationService", "PreAuthenticate"): "pre_authenticate",
    ("PaymentMethodAuthenticationService", "Authenticate"): "authenticate",
    ("PaymentMethodAuthenticationService", "PostAuthenticate"): "post_authenticate",
    # DisputeService flows (probe data uses dispute_ prefix)
    ("DisputeService", "SubmitEvidence"): "dispute_submit_evidence",
    ("DisputeService", "Get"): "dispute_get",
    ("DisputeService", "Defend"): "dispute_defend",
    ("DisputeService", "Accept"): "dispute_accept",
    # RefundService.Get -> refund_get (probe data uses refund_ prefix)
    ("RefundService", "Get"): "refund_get",
    # RecurringPaymentService flows (probe data uses recurring_ prefix)
    ("RecurringPaymentService", "Charge"): "recurring_charge",
    ("RecurringPaymentService", "Revoke"): "recurring_revoke",
}

# Services that should NOT prefix their RPC names
# (PaymentService is special - it's the "base" service)
_NO_PREFIX_SERVICES = frozenset({
    "PaymentService",
})

# Service prefix mappings (service name -> prefix for flow key)
_SERVICE_PREFIXES: dict[str, str] = {
    "RecurringPaymentService": "recurring",
    "RefundService": "refund",
    "CustomerService": "customer",  # Only used if not in _NO_PREFIX_SERVICES
    "PaymentMethodService": "payment_method",
    "MerchantAuthenticationService": "merchant_auth",
    "PaymentMethodAuthenticationService": "payment_method_auth",
    "DisputeService": "dispute",
    "EventService": "event",
    "PayoutService": "payout",
}


def _derive_flow_key(service_name: str, rpc_name: str) -> str | None:
    """
    Derive probe flow_key from gRPC service and RPC name.
    
    Returns None for services/RPCs that shouldn't be documented (admin, internal, etc.)
    or if we can't determine a reasonable key.
    """
    # Check explicit overrides first
    if (service_name, rpc_name) in _FLOW_KEY_OVERRIDES:
        return _FLOW_KEY_OVERRIDES[(service_name, rpc_name)]
    
    # Skip internal/admin services
    if service_name in {"HealthService", "AdminService", "DebugService"}:
        return None
    
    # Skip composite services (they don't have separate probe entries)
    if service_name.startswith("Composite"):
        return None
    
    # Handle variant flows (Proxy*, Token*, etc.)
    # Convert ProxyAuthorize -> proxy_authorize, TokenAuthorize -> token_authorize
    if any(rpc_name.startswith(prefix) for prefix in ["Proxy", "Token"]):
        return _to_snake(rpc_name)
    
    # Base service: just use snake_case RPC name
    if service_name in _NO_PREFIX_SERVICES:
        return _to_snake(rpc_name)
    
    # Other services: prefix + snake_case
    prefix = _SERVICE_PREFIXES.get(service_name)
    if prefix:
        return f"{prefix}_{_to_snake(rpc_name)}"
    
    # Unknown service - generate a reasonable key but will warn
    return _to_snake(rpc_name)


def _get_category_for_service(service_name: str) -> str:
    """Get category for a service name (ported from flow_metadata.rs)."""
    categories = {
        "PaymentService": "Payments",
        "RecurringPaymentService": "Mandates",
        "RefundService": "Refunds",
        "CustomerService": "Customers",
        "PaymentMethodService": "Payments",
        "MerchantAuthenticationService": "Authentication",
        "PaymentMethodAuthenticationService": "Authentication",
        "DisputeService": "Disputes",
        "EventService": "Events",
    }
    return categories.get(service_name, "Other")


def _to_snake(name: str) -> str:
    """Convert PascalCase to snake_case."""
    import re
    s = re.sub(r"([A-Z]+)([A-Z][a-z])", r"\1_\2", name)
    s = re.sub(r"([a-z\d])([A-Z])", r"\1_\2", s)
    return s.lower()


def _build_proto_metadata(proto_dir: Path) -> tuple[list[dict], dict]:
    """
    Build flow_metadata and message_schemas from proto files via grpc_tools.protoc.
    Returns (flow_metadata_list, message_schemas_dict).
    """
    import tempfile
    import os
    from grpc_tools import protoc
    from google.protobuf.descriptor_pb2 import FileDescriptorSet, FieldDescriptorProto

    grpc_include = os.path.join(os.path.dirname(protoc.__file__), "_proto")
    protos = ["services.proto", "payment.proto", "payment_methods.proto",
              "sdk_config.proto", "payouts.proto", "composite_payment.proto",
              "composite_services.proto"]

    with tempfile.NamedTemporaryFile(suffix=".desc", delete=False) as f:
        desc_path = f.name

    try:
        args = [
            "protoc",
            f"--proto_path={proto_dir}",
            f"--proto_path={grpc_include}",
            f"--descriptor_set_out={desc_path}",
            "--include_source_info",
        ] + [str(proto_dir / p) for p in protos]

        ret = protoc.main(args)
        if ret != 0:
            raise RuntimeError(f"protoc failed with exit code {ret}")

        with open(desc_path, "rb") as f:
            desc_set = FileDescriptorSet.FromString(f.read())
    finally:
        os.unlink(desc_path)

    flow_metadata = _extract_flow_metadata(desc_set)
    message_schemas = _extract_message_schemas(desc_set)
    return flow_metadata, message_schemas


def _extract_flow_metadata(desc_set) -> list[dict]:
    """Extract flow metadata from FileDescriptorSet."""
    metadata = []
    unknown_services = set()  # Track unknown services for warnings
    auto_derived = []  # Track auto-derived mappings for info

    for file_desc in desc_set.file:
        # Build source info lookup for doc comments
        source_info = {}
        if file_desc.source_code_info:
            for location in file_desc.source_code_info.location:
                path = tuple(location.path)
                if location.leading_comments:
                    source_info[path] = location.leading_comments.strip()

        for svc_idx, service in enumerate(file_desc.service):
            service_name = service.name

            for method_idx, method in enumerate(service.method):
                rpc_name = method.name

                # Derive flow key automatically
                flow_key = _derive_flow_key(service_name, rpc_name)
                if flow_key is None:
                    # Silently skip internal/variant flows
                    continue
                
                # Track auto-derived keys for unknown services
                if (service_name, rpc_name) not in _FLOW_KEY_OVERRIDES and \
                   service_name not in _NO_PREFIX_SERVICES and \
                   service_name not in _SERVICE_PREFIXES:
                    unknown_services.add(service_name)
                    auto_derived.append(f"{service_name}.{rpc_name} -> {flow_key}")

                # Get doc comment from source info
                # Path for method: [6 (service), svc_idx, 2 (method), method_idx]
                path = (6, svc_idx, 2, method_idx)
                description = source_info.get(path, "")
                # Clean up description
                description = " ".join(description.split())
                if description and not description.endswith((".", "!", "?")):
                    description += "."

                # Extract request/response type names
                req_type = method.input_type.split(".")[-1]
                res_type = method.output_type.split(".")[-1]

                metadata.append({
                    "flow_key": flow_key,
                    "service_rpc": f"{service_name}.{rpc_name}",
                    "description": description,
                    "service_name": service_name,
                    "rpc_name": rpc_name,
                    "category": _get_category_for_service(service_name),
                    "grpc_request": req_type,
                    "grpc_response": res_type,
                })

    # Info about auto-derived keys
    if auto_derived and unknown_services:
        print(
            f"\nℹ️  INFO: Auto-derived flow keys for {len(auto_derived)} RPCs from unknown services:",
            file=sys.stderr
        )
        for mapping in sorted(auto_derived)[:5]:
            print(f"   - {mapping}", file=sys.stderr)
        if len(auto_derived) > 5:
            print(f"   ... and {len(auto_derived) - 5} more", file=sys.stderr)
        print(
            f"\n   Consider adding these services to _SERVICE_PREFIXES:\n   {sorted(unknown_services)}",
            file=sys.stderr
        )

    return metadata


# Proto scalar types that serialize as JSON scalars
_SCALAR_TYPES = frozenset([
    "string", "int32", "int64", "uint32", "uint64", "bool", "bytes",
    "double", "float", "sint32", "sint64", "fixed32", "fixed64",
    "sfixed32", "sfixed64", "SecretString", "CardNumberType", "NetworkTokenType",
])


def _extract_message_schemas(desc_set) -> dict:
    """Extract message schemas from FileDescriptorSet with field comments."""
    from google.protobuf.descriptor_pb2 import FieldDescriptorProto

    schemas = {}

    for file_desc in desc_set.file:
        # Build source info lookup for field comments
        source_info = {}
        if file_desc.source_code_info:
            for location in file_desc.source_code_info.location:
                path = tuple(location.path)
                if location.leading_comments:
                    source_info[path] = location.leading_comments.strip()

        def process_message(msg_type, msg_name: str, msg_index: int, parent_path: tuple, is_nested: bool = False) -> None:
            """Process a single message type and its nested types."""
            comments = {}
            field_types = {}

            # Path for this message
            # Top-level: [4 (message_type), msg_index]
            # Nested: [3 (nested_type), nested_index] within parent
            if is_nested:
                msg_path = parent_path + (3, msg_index)
            else:
                msg_path = parent_path + (4, msg_index)

            for field_idx, field in enumerate(msg_type.field):
                field_name = field.name  # snake_case from proto
                camel_name = _to_camel(field_name)  # camelCase for JSON/proto3

                # Get field comment from source info
                # Path for field: [..., 2 (field), field_idx]
                field_path = msg_path + (2, field_idx)
                
                # Build lookup for both leading and trailing comments
                leading_comment = ""
                trailing_comment = ""
                for loc in file_desc.source_code_info.location:
                    if tuple(loc.path) == field_path:
                        if loc.leading_comments:
                            leading_comment = loc.leading_comments.strip()
                        if loc.trailing_comments:
                            trailing_comment = loc.trailing_comments.strip()
                
                # Prefer trailing comment (field-specific) over leading comment (group header)
                comment = trailing_comment or leading_comment
                
                if comment:
                    # Clean up comment
                    comment = " ".join(comment.split())
                    if comment and not comment.endswith((".", "!", "?")):
                        comment += "."
                    # Store under both snake_case and camelCase for compatibility
                    # Probe data uses snake_case, but proto JSON uses camelCase
                    comments[field_name] = comment
                    comments[camel_name] = comment

                # Check if it's a message type (not scalar)
                if field.type == FieldDescriptorProto.TYPE_MESSAGE:
                    type_name = field.type_name.split(".")[-1]
                    if type_name not in _SCALAR_TYPES:
                        # Store field types under both naming conventions
                        field_types[field_name] = type_name
                        field_types[camel_name] = type_name

            if comments or field_types:
                schemas[msg_name] = {
                    "comments": comments,
                    "field_types": field_types,
                }

            # Process nested types
            for nested_idx, nested in enumerate(msg_type.nested_type):
                process_message(nested, nested.name, nested_idx, msg_path, is_nested=True)

        for msg_idx, msg_type in enumerate(file_desc.message_type):
            process_message(msg_type, msg_type.name, msg_idx, ())

    return schemas


def _to_camel(snake: str) -> str:
    """Convert snake_case to camelCase."""
    import re
    return re.sub(r"_([a-z])", lambda m: m.group(1).upper(), snake)


def load_probe_data(probe_path: Optional[Path]) -> dict[str, dict]:
    """
    Load probe JSON and index by connector name.

    Discovers connectors from filesystem and derives flow_metadata/message_schemas
    from proto files using grpc_tools.protoc at runtime.

    Returns {connector_name: connector_data} dict.
    """
    global _FLOW_METADATA, _MESSAGE_SCHEMAS, _PROBE_DATA

    if probe_path is None:
        return {}

    # Already loaded — skip expensive proto compilation and JSON parsing
    if _PROBE_DATA:
        return _PROBE_DATA

    probe_dir = probe_path if probe_path.is_dir() else probe_path

    # Discover connectors from filesystem (no manifest needed)
    connector_names = [f.stem for f in sorted(probe_dir.glob("*.json"))]

    # Build flow metadata and message schemas from proto files
    proto_dir = probe_dir.parent.parent / "crates" / "types-traits" / "grpc-api-types" / "proto"
    if proto_dir.exists():
        try:
            print("  Compiling proto metadata…", end=" ", flush=True)
            _FLOW_METADATA, _MESSAGE_SCHEMAS = _build_proto_metadata(proto_dir)
            snippets.load_proto_type_map(proto_dir)
            print("✓")
        except Exception as exc:
            print(f"✗\nWarning: failed to build proto metadata: {exc}", file=sys.stderr)
            _FLOW_METADATA = []
            _MESSAGE_SCHEMAS = {}
    else:
        print(f"Warning: proto dir not found at {proto_dir}", file=sys.stderr)
        _FLOW_METADATA = []
        _MESSAGE_SCHEMAS = {}

    # Load per-connector probe data
    _PROBE_DATA = {}
    for conn_name in connector_names:
        conn_file = probe_dir / f"{conn_name}.json"
        try:
            with open(conn_file, encoding="utf-8") as f:
                conn_data = json.load(f)
            _PROBE_DATA[conn_name] = conn_data
        except Exception as exc:
            print(f"Warning: failed to load {conn_file}: {exc}", file=sys.stderr)

    return _PROBE_DATA


def _probe_pm_support(probe_connector: dict, flow_key: str) -> Optional[dict[str, bool]]:
    """
    Return {pm_key: supported} for a flow that has PM-specific probe results.
    Returns None for flows that only have a 'default' key (no PM breakdown).
    """
    if not flow_key:
        return None
    pms = probe_connector.get("flows", {}).get(flow_key, {})
    if not pms or set(pms.keys()) == {"default"}:
        return None
    return {pm: pms[pm]["status"] == "supported" for pm in _PROBE_PM_DISPLAY if pm in pms}


# Human-readable label per PM key used as the sample heading

# ─── Paths ────────────────────────────────────────────────────────────────────

REPO_ROOT    = Path(__file__).parent.parent.parent.parent
DOCS_DIR     = REPO_ROOT / "docs-generated/connectors"
EXAMPLES_DIR = REPO_ROOT / "examples"
PROTO_DIR    = REPO_ROOT / "crates/types-traits/grpc-api-types/proto"

# Category order for grouping flows in documentation
CATEGORY_ORDER = ["Payments", "Refunds", "Mandates", "Customers", "Disputes", "Authentication", "Session", "Other"]

# ─── Display Name ─────────────────────────────────────────────────────────────

_DISPLAY_NAMES = {
    "stripe": "Stripe",
    "adyen": "Adyen",
    "razorpay": "Razorpay",
    "razorpayv2": "Razorpay V2",
    "authorizedotnet": "Authorize.net",
    "braintree": "Braintree",
    "cybersource": "CyberSource",
    "checkout": "Checkout.com",
    "payu": "PayU",
    "novalnet": "Novalnet",
    "nexinets": "Nexinets",
    "noon": "Noon",
    "fiserv": "Fiserv",
    "elavon": "Elavon",
    "xendit": "Xendit",
    "mifinity": "MiFinity",
    "phonepe": "PhonePe",
    "cashfree": "Cashfree",
    "paytm": "Paytm",
    "cashtocode": "CashtoCode",
    "volt": "Volt",
    "dlocal": "dLocal",
    "helcim": "Helcim",
    "placetopay": "PlacetoPay",
    "rapyd": "Rapyd",
    "aci": "ACI",
    "trustpay": "TrustPay",
    "fiuu": "Fiuu",
    "calida": "Calida",
    "cryptopay": "CryptoPay",
}


def display_name(connector_name: str) -> str:
    return _DISPLAY_NAMES.get(connector_name, connector_name.replace("_", " ").title())


# ─── Markdown Generation ──────────────────────────────────────────────────────

def get_flows_from_probe(probe_connector: dict) -> list[str]:
    """Extract list of flow keys that have at least one supported entry in probe data."""
    result = []
    for flow_key, flow_data in probe_connector.get("flows", {}).items():
        if any(entry.get("status") == "supported" for entry in flow_data.values()):
            result.append(flow_key)
    return result


def get_flow_meta(flow_key: str) -> dict:
    """Get flow metadata by flow_key from loaded probe.json data."""
    flow_metadata = get_flow_metadata()
    return flow_metadata.get(flow_key, {})


# Bank PM keys that require a specific currency rather than the probe default (USD).
_BANK_PM_CURRENCY_OVERRIDES: dict[str, str] = {
    "Sepa": "EUR",
    "Bacs": "GBP",
    "Becs": "AUD",
    # Ach is USD — no override needed
}


def _get_flow_proto_requests(
    probe_connector: dict,
    scenario: "snippets.ScenarioSpec",
) -> dict[str, dict]:
    """
    Build flow_key → proto_request dict for the flows in a scenario.

    For authorize: uses the PM-specific entry keyed by scenario.pm_key.
    For all other flows: uses the "default" entry.
    Returns {} for any flow whose payload is missing or status != supported.

    Also applies bank-PM currency overrides so that SEPA uses EUR, BACS uses GBP, etc.
    """
    flows = probe_connector.get("flows", {})
    result: dict[str, dict] = {}
    for flow_key in scenario.flows:
        pm_key = scenario.pm_key if flow_key == "authorize" else "default"
        entry  = flows.get(flow_key, {}).get(pm_key or "default", {})
        if entry.get("status") == "supported":
            payload = dict(entry.get("proto_request") or {})
            # For checkout_bank, override the currency to the PM-appropriate one
            # (probe data defaults to USD which many bank PMs like SEPA don't support).
            if (
                scenario.key == "checkout_bank"
                and flow_key == "authorize"
                and scenario.pm_key in _BANK_PM_CURRENCY_OVERRIDES
                and "amount" in payload
                and isinstance(payload.get("amount"), dict)
            ):
                payload["amount"] = dict(payload["amount"])
                payload["amount"]["currency"] = _BANK_PM_CURRENCY_OVERRIDES[scenario.pm_key]
            result[flow_key] = payload
    return result


def _collect_flow_items(
    probe_connector: dict,
    exclude_keys: set,
) -> list[tuple[str, dict, str]]:
    """Return (flow_key, proto_req, pm_label) for every supported flow not in exclude_keys."""
    flows = probe_connector.get("flows", {})
    items: list[tuple[str, dict, str]] = []
    for flow_key, flow_data in flows.items():
        if flow_key in exclude_keys:
            continue
        if flow_key == "authorize":
            for pm in _AUTHORIZE_PM_PRIORITY:
                entry = flow_data.get(pm, {})
                if entry.get("status") == "supported" and entry.get("proto_request"):
                    items.append((flow_key, entry["proto_request"], pm))
                    break
        else:
            entry = flow_data.get("default", {})
            if entry.get("status") == "supported":
                items.append((flow_key, entry.get("proto_request") or {}, ""))
    return items


def _find_func_line(content: str, search: str) -> int:
    """Return the 1-based line number of the first line containing *search*, or 0 if not found."""
    for i, line in enumerate(content.splitlines(), 1):
        if search in line:
            return i
    return 0


# Maps language → (scenario_key → search string template) so we can locate
# each process_* function inside the generated consolidated file.
_SCENARIO_FUNC_SEARCH: dict[str, str] = {
    "python":     "async def process_{key}(",
    "javascript": "async function process{camel}(",
    "typescript": "async function process{camel}(",
    "kotlin":     "fun process{camel}(",
    "rust":       "fn process_{key}(",
}

# Same for per-flow functions (used in the Flow Reference section).
_FLOW_FUNC_SEARCH: dict[str, str] = {
    "python":     "async def {key}(",
    "javascript": "async function {camel}(",
    "typescript": "async function {camel}(",
    "kotlin":     "fun {camel}(",
    "rust":       "fn {key}(",
}


def _scenario_search(sdk: str, scenario_key: str) -> str:
    """Return the function-name search string for a scenario in a given SDK."""
    tmpl = _SCENARIO_FUNC_SEARCH[sdk]
    camel = "".join(w.capitalize() for w in scenario_key.split("_"))
    return tmpl.format(key=scenario_key, camel=camel)


def _flow_search(sdk: str, flow_key: str) -> str:
    """Return the function-name search string for a flow in a given SDK."""
    tmpl = _FLOW_FUNC_SEARCH[sdk]
    camel = snippets._to_camel(flow_key)  # type: ignore[attr-defined]
    camel = camel[0].lower() + camel[1:]
    # JS reserved words are renamed with a "Payment" suffix in the generated file
    if sdk == "javascript" and flow_key in snippets.JS_RESERVED:  # type: ignore[attr-defined]
        camel = f"{flow_key}Payment"
    return tmpl.format(key=flow_key, camel=camel)


def generate_scenario_files(
    connector_name: str,
    probe_connector: dict,
    examples_dir: Path,
) -> tuple[list[Path], dict[str, dict[str, int]], dict[str, dict[str, int]]]:
    """
    Write one consolidated examples/{connector}/{connector}.py and
    examples/{connector}/{connector}.ts containing all scenarios
    plus individual flow functions.  Deletes stale per-scenario files.

    Returns (paths, scenario_lines, flow_lines) where:
      scenario_lines[scenario_key][sdk] = 1-based line of the process_* function
      flow_lines[flow_key][sdk]         = 1-based line of the flow function (py/js only)
    """
    flow_metadata = get_flow_metadata()
    scenarios     = snippets.detect_scenarios(probe_connector)

    # Pair each scenario with its payloads; skip scenarios with no data
    scenarios_with_payloads = [
        (s, fp)
        for s in scenarios
        for fp in [_get_flow_proto_requests(probe_connector, s)]
        if fp
    ]

    if not scenarios_with_payloads:
        return [], {}, {}

    # Collect ALL flows for standalone function generation (don't exclude scenario keys)
    # This ensures flows like "refund" that are both scenarios AND standalone flows
    # get their standalone functions generated in Python/JS
    flow_items    = _collect_flow_items(probe_connector, exclude_keys=set())
    written: list[Path] = []
    # scenario_lines[scenario_key][sdk] = 1-based line number of process_* function
    scenario_lines: dict[str, dict[str, int]] = {}
    # flow_lines[flow_key][sdk] = 1-based line number of flow function (py/js)
    flow_lines: dict[str, dict[str, int]] = {}

    return written, scenario_lines, flow_lines


# PM priority for selecting the representative authorize payload
_AUTHORIZE_PM_PRIORITY = [
    "Card", "GooglePay", "ApplePay", "SamsungPay",
    "Sepa", "Ach", "Bacs", "Becs",
    "Ideal", "PaypalRedirect", "Blik", "Klarna", "Afterpay", "UpiCollect", "Affirm",
]


def generate_flow_files(
    connector_name: str,
    probe_connector: dict,
    examples_dir: Path,
) -> tuple[list[Path], dict[str, dict[str, int]], dict[str, dict[str, int]]]:
    """
    Write one consolidated examples/{connector}/{connector}.kt and
    examples/{connector}/{connector}.rs containing all scenario and flow functions.
    Deletes stale per-flow files for all languages.

    Returns (list_of_written_paths, flow_line_numbers, scenario_line_numbers_kt_rs) where:
      flow_line_numbers[flow_key][sdk]           = 1-based line of the flow function
      scenario_line_numbers_kt_rs[scenario_key][sdk] = 1-based line of the process_* function
    """
    flow_metadata = get_flow_metadata()
    # ALL flows must appear as standalone functions — including flows whose names match a
    # scenario key (e.g. "refund").
    flow_items = _collect_flow_items(probe_connector, exclude_keys=set())

    # Compute scenarios_with_payloads (same logic as generate_scenario_files)
    scenarios = snippets.detect_scenarios(probe_connector)
    scenarios_with_payloads = [
        (s, fp)
        for s in scenarios
        for fp in [_get_flow_proto_requests(probe_connector, s)]
        if fp
    ]

    if not flow_items and not scenarios_with_payloads:
        return [], {}, {}

    written: list[Path] = []
    # flow_line_numbers[flow_key][sdk] = 1-based line number
    flow_line_numbers: dict[str, dict[str, int]] = {}
    # scenario_line_numbers[scenario_key][sdk] = 1-based line number of process_* function
    scenario_line_numbers: dict[str, dict[str, int]] = {}

    # All flow keys (including those matching scenario names) — needed for cleanup
    all_flow_keys = set(probe_connector.get("flows", {}).keys())

    for sdk, ext, render_fn in [
        ("python",     "py", snippets.render_consolidated_python),
        ("kotlin",     "kt", snippets.render_consolidated_kotlin),
        ("rust",       "rs", snippets.render_consolidated_rust),
        ("typescript", "ts", snippets.render_consolidated_javascript),
    ]:
        out_dir  = examples_dir / connector_name
        out_dir.mkdir(parents=True, exist_ok=True)
        out_path = out_dir / f"{connector_name}.{ext}"
        content  = render_fn(
            connector_name, flow_items, flow_metadata, _MESSAGE_SCHEMAS,
            scenarios_with_payloads=scenarios_with_payloads,
        )
        out_path.write_text(content, encoding="utf-8")
        written.append(out_path)

        # Record line numbers for each flow function
        for flow_key, _, _ in flow_items:
            lineno = _find_func_line(content, _flow_search(sdk, flow_key))
            if lineno:
                flow_line_numbers.setdefault(flow_key, {})[sdk] = lineno

        # Record line numbers for each scenario function
        for scenario, _ in scenarios_with_payloads:
            lineno = _find_func_line(content, _scenario_search(sdk, scenario.key))
            if lineno:
                scenario_line_numbers.setdefault(scenario.key, {})[sdk] = lineno

        # Delete ALL stale per-flow files (including scenario-named ones like refund.kt)
        for flow_key in all_flow_keys:
            stale = out_dir / f"{flow_key}.{ext}"
            if stale.exists():
                stale.unlink()

    return written, flow_line_numbers, scenario_line_numbers


def generate_llms_txt(probe_data: dict[str, dict], docs_dir: Path) -> None:
    """
    Write docs/llms.txt — a machine-readable navigation index for AI assistants.
    """
    lines: list[str] = [
        "# Connector Service — LLM Navigation Index",
        f"# Connectors: {len(probe_data)}",
        "#",
        "# This file helps AI coding assistants navigate connector-service documentation.",
        "# Each connector block lists: doc path, scenarios, supported payment methods,",
        "# supported flows, and paths to runnable Python/JavaScript examples.",
        "#",
        "# Usage: fetch this file first, then fetch the specific connector doc or example.",
        "",
        "overview:",
        f"  total_connectors: {len(probe_data)}",
        "  docs_root: docs-generated/connectors/",
        "  examples_root: examples/",
        "  all_connectors_matrix: docs-generated/all_connector.md",
        "",
        "integration_pattern:",
        "  1. Configure ConnectorConfig with connector name and credentials",
        "  2. Call flows in sequence per scenario (see Integration Scenarios in connector doc)",
        "  3. Branch on response.status: AUTHORIZED / PENDING / FAILED",
        "  4. PENDING means await webhook or poll Get before capturing",
        "  5. Pass connector_transaction_id from Authorize response to Capture/Refund",
        "",
        "---",
        "",
    ]

    for connector_name in sorted(probe_data.keys()):
        probe_connector = probe_data[connector_name]
        name            = display_name(connector_name)
        scenarios       = snippets.detect_scenarios(probe_connector)
        entry           = snippets.render_llms_txt_entry(
            connector_name, name, probe_connector, scenarios
        )
        lines.append(entry)

    out_path = docs_dir.parent / "llms.txt"
    out_path.write_text("\n".join(lines), encoding="utf-8")
    print(f"  llms.txt → {out_path.relative_to(REPO_ROOT)}")


def generate_connector_doc(
    connector_name: str,
    probe_data: Optional[dict] = None,
    scenario_line_numbers: Optional[dict[str, dict[str, int]]] = None,
    flow_line_numbers: Optional[dict[str, dict[str, int]]] = None,
) -> Optional[str]:
    """Generate complete markdown documentation for a connector.

    scenario_line_numbers: {scenario_key: {sdk: line_number}} — from generate_scenario_files.
    flow_line_numbers:      {flow_key:     {sdk: line_number}} — from generate_flow_files.
    """
    scenario_line_numbers = scenario_line_numbers or {}
    flow_line_numbers     = flow_line_numbers or {}
    probe_connector = (probe_data or {}).get(connector_name, {})
    
    # Get flows from probe data
    flows = get_flows_from_probe(probe_connector)
    if not flows:
        print(f"  No flows found for '{connector_name}' – skipping.", file=sys.stderr)
        return None

    name = display_name(connector_name)

    out: list[str] = []
    a = out.append  # shorthand

    # ── Front-matter comment ────────────────────────────────────────────────
    a(f"# {name}")
    a("")
    a("<!--")
    a("This file is auto-generated. Do not edit by hand.")
    a(f"Source: data/field_probe/{connector_name}.json")
    a(f"Regenerate: python3 scripts/generators/docs/generate.py {connector_name}")
    a("-->")
    a("")

    # ── SDK Configuration (once per connector) ──────────────────────────────
    for line in snippets.render_config_section(connector_name):
        a(line)

    # ── Integration Scenarios ────────────────────────────────────────────────
    scenarios     = snippets.detect_scenarios(probe_connector)
    flow_metadata = get_flow_metadata()
    if scenarios:
        a("## Integration Scenarios")
        a("")
        a(
            "Complete, runnable examples for common integration patterns. "
            "Each example shows the full flow with status handling. "
            "Copy-paste into your app and replace placeholder values."
        )
        a("")
        for scenario in scenarios:
            flow_payloads = _get_flow_proto_requests(probe_connector, scenario)
            for line in snippets.render_scenario_section(
                scenario, connector_name, flow_payloads,
                flow_metadata, _MESSAGE_SCHEMAS, {},
                line_numbers=scenario_line_numbers.get(scenario.key, {}),
            ):
                a(line)

    # ── API Reference ────────────────────────────────────────────────────────
    a("## API Reference")
    a("")
    a("| Flow (Service.RPC) | Category | gRPC Request Message |")
    a("|--------------------|----------|----------------------|")
    for f in flows:
        meta = get_flow_meta(f)
        cat = meta.get("category", "Other")
        req_msg = meta.get("grpc_request", "—")
        service = meta.get("service_name", "")
        rpc = meta.get("rpc_name", f)
        flow_display = f"{service}.{rpc}" if service else f
        # VS Code/GitHub auto-generate anchors: lowercase, remove dots/special chars
        anchor = flow_display.lower().replace(".", "").replace(" ", "-")
        a(f"| [{flow_display}](#{anchor}) | {cat} | `{req_msg}` |")
    a("")

    # ── Per-flow detail ──────────────────────────────────────────────────────
    # Group by category
    by_cat: dict[str, list[str]] = {}
    for f in flows:
        meta = get_flow_meta(f)
        cat = meta.get("category", "Other")
        by_cat.setdefault(cat, []).append(f)

    for cat in CATEGORY_ORDER:
        if cat not in by_cat:
            continue
        a(f"### {cat}")
        a("")

        for f in by_cat[cat]:
            meta = get_flow_meta(f)

            service = meta.get("service_name", "")
            rpc = meta.get("rpc_name", f)
            flow_heading = f"{service}.{rpc}" if service else f
            a(f"#### {flow_heading}")
            a("")

            if meta.get("description"):
                a(meta["description"])
                a("")

            # gRPC messages
            if meta.get("grpc_request"):
                a(f"| | Message |")
                a(f"|---|---------|")
                a(f"| **Request** | `{meta['grpc_request']}` |")
                a(f"| **Response** | `{meta.get('grpc_response', '—')}` |")
                a("")

            # Payment method type support (from field-probe)
            pm_support = _probe_pm_support(probe_connector, f)
            if pm_support:
                a("**Supported payment method types:**")
                a("")
                a("| Payment Method | Supported |")
                a("|----------------|:---------:|")
                for pm_key, pm_label in _PROBE_PM_DISPLAY.items():
                    if pm_key in pm_support:
                        pm_status = probe_connector.get("flows", {}).get("authorize", {}).get(pm_key, {}).get("status", "unknown")
                        mark = _status_to_mark(pm_status)
                        a(f"| {pm_label} | {mark} |")
                a("")

            # Inline PM reference right after Authorize (where it's most useful)
            if f == "authorize":
                for line in snippets.render_pm_reference_section(
                    probe_connector, flow_metadata, _MESSAGE_SCHEMAS
                ):
                    a(line)

            # Link to per-flow example files
            flow_data = probe_connector.get("flows", {}).get(f, {})
            has_payload = (
                flow_data.get("default", {}).get("status") == "supported"
                or any(
                    v.get("status") == "supported"
                    for k, v in flow_data.items()
                    if k != "default"
                )
            )
            if has_payload:
                base_py = f"../../examples/{connector_name}/{connector_name}.py"
                base_ts = f"../../examples/{connector_name}/{connector_name}.ts"
                base_kt = f"../../examples/{connector_name}/{connector_name}.kt"
                base_rs = f"../../examples/{connector_name}/{connector_name}.rs"
                
                # Get line numbers from flow_line_numbers
                flow_lines = flow_line_numbers.get(f, {}) if flow_line_numbers else {}
                ln_py = flow_lines.get("python", 0)
                ln_ts = flow_lines.get("typescript", 0)
                ln_kt = flow_lines.get("kotlin", 0)
                ln_rs = flow_lines.get("rust", 0)
                
                # Build links with line numbers when available
                py_link = f"{base_py}#L{ln_py}" if ln_py else base_py
                ts_link = f"{base_ts}#L{ln_ts}" if ln_ts else base_ts
                kt_link = f"{base_kt}#L{ln_kt}" if ln_kt else base_kt
                rs_link = f"{base_rs}#L{ln_rs}" if ln_rs else base_rs
                
                a(f"**Examples:** [Python]({py_link}) · [TypeScript]({ts_link}) · [Kotlin]({kt_link}) · [Rust]({rs_link})")
                a("")

    return "\n".join(out)


# ─── Connector Discovery ──────────────────────────────────────────────────────

def list_connectors() -> list[str]:
    """Return sorted list of all connector names from probe data."""
    return sorted(_PROBE_DATA.keys())


# ─── CLI ─────────────────────────────────────────────────────────────────────

def check_example_syntax(examples_dir: Path, connectors: Optional[list[str]] = None) -> bool:
    """Run syntax/compilation checks on generated example files.

    If *connectors* is given, only files under examples_dir/{connector}/ are checked.
    Returns True when all checks pass, False when any error is found.
    """
    import subprocess

    if connectors:
        subdirs = [examples_dir / c for c in connectors if (examples_dir / c).is_dir()]
        py_files = sorted(f for d in subdirs for f in d.rglob("*.py"))
        ts_files = sorted(f for d in subdirs for f in d.rglob("*.ts"))
        kt_files = sorted(f for d in subdirs for f in d.rglob("*.kt"))
        rs_files = sorted(f for d in subdirs for f in d.rglob("*.rs"))
    else:
        py_files = sorted(examples_dir.rglob("*.py"))
        ts_files = sorted(examples_dir.rglob("*.ts"))
        kt_files = sorted(examples_dir.rglob("*.kt"))
        rs_files = sorted(examples_dir.rglob("*.rs"))

    errors: list[str] = []

    # ── Python — full AST parse via py_compile ─────────────────────────────────
    if py_files:
        print(f"  Checking Python ({len(py_files)} files) ...", end=" ", flush=True)
        py_errors: list[str] = []
        for f in py_files:
            result = subprocess.run(
                [sys.executable, "-m", "py_compile", str(f)],
                capture_output=True, text=True,
            )
            if result.returncode != 0:
                py_errors.append(f"Python: {f.relative_to(examples_dir.parent)}: {result.stderr.strip()}")
        if py_errors:
            print(f"✗ ({len(py_errors)} error(s))")
            errors.extend(py_errors)
        else:
            print("✓")

    # ── Python — mypy type checking ────────────────────────────────────────────
    # Check if mypy is available before attempting type checking
    mypy_available = False
    try:
        subprocess.run([sys.executable, "-m", "mypy", "--version"], capture_output=True, check=True)
        mypy_available = True
    except (subprocess.CalledProcessError, FileNotFoundError):
        pass
    
    if py_files:
        if mypy_available:
            print(f"  Checking Python types ({len(py_files)} files) ...", end=" ", flush=True)
            mypy_errors: list[str] = []
            for f in py_files:
                result = subprocess.run(
                    [sys.executable, "-m", "mypy", "--ignore-missing-imports", "--follow-imports=skip", str(f)],
                    capture_output=True, text=True,
                )
                if result.returncode != 0:
                    err_lines = [line for line in (result.stdout + result.stderr).splitlines() if line.strip()]
                    mypy_errors.append(f"Python Types: {f.relative_to(examples_dir.parent)}: {'; '.join(err_lines)}")
            if mypy_errors:
                print(f"✗ ({len(mypy_errors)} error(s))")
                errors.extend(mypy_errors)
            else:
                print("✓")
        else:
            print(f"  Checking Python types ({len(py_files)} files) ... skipped (mypy unavailable)")

    # ── TypeScript — tsc --noEmit via SDK's local tsc installation ──────────────
    # The generated .ts files import 'hyperswitch-prism', which is resolved via
    # path mappings in sdk/javascript/tsconfig.json.  We must run tsc from that
    # directory using a temporary tsconfig that extends it and adds the example
    # files as additional includes.
    sdk_js_dir = examples_dir.parent / "sdk" / "javascript"
    local_tsc  = sdk_js_dir / "node_modules" / ".bin" / "tsc"
    tsc_cmd: list[str] | None = None
    # Prefer the SDK-local tsc (already installed); fall back to PATH/npx.
    for candidate in ([str(local_tsc)], ["tsc"], ["npx", "--yes", "tsc"]):
        try:
            subprocess.run(candidate + ["--version"], capture_output=True, check=True)
            tsc_cmd = candidate
            break
        except (subprocess.CalledProcessError, FileNotFoundError):
            pass
    tsc_ok = tsc_cmd is not None
    if ts_files:
        if tsc_ok and sdk_js_dir.exists():
            import json as _json, tempfile as _tempfile
            print(f"  Checking TypeScript ({len(ts_files)} files) ...", end=" ", flush=True)
            ts_errors: list[str] = []
            # Build relative paths from sdk/javascript to each example .ts file.
            # Examples live outside sdk/javascript, so use os.path.relpath.
            import os as _os
            rel_includes = [
                _os.path.relpath(str(f), str(sdk_js_dir)).replace("\\", "/")
                for f in ts_files
            ]
            # rootDir must cover both sdk/javascript/src and ../../examples,
            # so set it to the repo root (../../ relative to sdk/javascript).
            tmp_cfg = {
                "extends": "./tsconfig.json",
                "compilerOptions": {
                    "noEmit": True,
                    "rootDir": "../../",
                },
                "include": rel_includes,
            }
            with _tempfile.NamedTemporaryFile(
                mode="w", suffix=".json", dir=str(sdk_js_dir), delete=False
            ) as fh:
                _json.dump(tmp_cfg, fh)
                tmp_cfg_path = fh.name
            try:
                result = subprocess.run(
                    tsc_cmd + ["--project", tmp_cfg_path],
                    capture_output=True, text=True, cwd=str(sdk_js_dir),
                )
                if result.returncode != 0:
                    # Parse tsc output lines: "path(line,col): error TSxxxx: msg"
                    for line in (result.stdout + result.stderr).splitlines():
                        line = line.strip()
                        if not line or line.startswith("Found "):
                            continue
                        # Skip deprecation warnings about baseUrl and TS5101
                        if "TS5101" in line or "deprecated" in line.lower():
                            continue
                        # Skip info/help URLs and messages without error codes
                        if line.startswith("Visit http") or "aka.ms" in line:
                            continue
                        # Normalize absolute path back to examples/connector/file.ts
                        for f in ts_files:
                            rel = str(f.relative_to(examples_dir.parent))
                            if str(f) in line or rel in line:
                                ts_errors.append(f"TypeScript: {line}")
                                break
                        else:
                            ts_errors.append(f"TypeScript: {line}")
            finally:
                _os.unlink(tmp_cfg_path)
            if ts_errors:
                print(f"✗ ({len(ts_errors)} error(s))")
                errors.extend(ts_errors)
            else:
                print("✓")
        elif not tsc_ok:
            print(f"  Checking TypeScript ({len(ts_files)} files) ... skipped (tsc/npx unavailable)")
        else:
            print(f"  Checking TypeScript ({len(ts_files)} files) ... skipped (sdk/javascript not found)")

    # ── Kotlin — Gradle (preferred) or kotlinc fallback ───────────────────────
    # smoke-test is a standalone Gradle project at sdk/java/smoke-test/ that
    # depends on the published SDK JAR. We run it via --project-dir so the root
    # sdk/java/gradlew drives it without needing a separate wrapper.
    kt_ok = False
    sdk_java_dir = examples_dir.parent / "sdk" / "java"
    smoke_test_dir = sdk_java_dir / "smoke-test"
    gradlew = sdk_java_dir / "gradlew"
    if kt_files:
        if gradlew.exists() and smoke_test_dir.exists():
            kt_ok = True
            print(f"  Checking Kotlin ({len(kt_files)} files) via Gradle ...", end=" ", flush=True)
            # Ensure the SDK JAR is in Maven local so smoke-test can resolve it.
            # Clean first so new proto-generated classes (e.g. EventServiceParseRequest)
            # are always compiled from the regenerated Payment.java, not a stale cache.
            subprocess.run(
                [str(gradlew), "clean", "publishToMavenLocal", "-q"],
                capture_output=True, text=True,
                cwd=str(sdk_java_dir),
            )
            result = subprocess.run(
                [str(gradlew), "--project-dir", str(smoke_test_dir),
                 "compileKotlin", "--rerun-tasks", "-q"],
                capture_output=True, text=True,
                cwd=str(sdk_java_dir),
            )
            if result.returncode != 0:
                kt_errors: list[str] = []
                for line in (result.stdout + result.stderr).splitlines():
                    if line.startswith("e: file://"):
                        short = line.replace("e: file://" + str(examples_dir.parent) + "/", "")
                        kt_errors.append(f"Kotlin: {short}")
                if kt_errors:
                    print(f"✗ ({len(kt_errors)} error(s))")
                    errors.extend(kt_errors)
                else:
                    # Non-zero exit but no parseable errors — surface raw output
                    raw = (result.stdout + result.stderr).strip()[:300]
                    errors.append(f"Kotlin (Gradle): {raw}")
                    print("✗")
            else:
                print("✓")
        else:
            try:
                subprocess.run(["kotlinc", "-version"], capture_output=True, check=True)
                kt_ok = True
            except (subprocess.CalledProcessError, FileNotFoundError):
                pass
            if kt_ok:
                print(f"  Checking Kotlin ({len(kt_files)} files) via kotlinc ...", end=" ", flush=True)
                kt_errors = []
                for f in kt_files:
                    result = subprocess.run(
                        ["kotlinc", "-nowarn", str(f), "-d", "/dev/null"],
                        capture_output=True, text=True,
                    )
                    if result.returncode != 0:
                        kt_errors.append(f"Kotlin: {f.relative_to(examples_dir.parent)}: {result.stderr.strip()}")
                if kt_errors:
                    print(f"✗ ({len(kt_errors)} error(s))")
                    errors.extend(kt_errors)
                else:
                    print("✓")
            else:
                print(f"  Checking Kotlin ({len(kt_files)} files) ... skipped (Gradle/kotlinc unavailable)")

    # ── Rust — rustfmt syntax check ────────────────────────────────────────────
    rustfmt_ok = False
    try:
        subprocess.run(["rustfmt", "--version"], capture_output=True, check=True)
        rustfmt_ok = True
    except (subprocess.CalledProcessError, FileNotFoundError):
        pass
    if rs_files:
        if rustfmt_ok:
            print(f"  Checking Rust format ({len(rs_files)} files) ...", end=" ", flush=True)
            fmt_errors: list[str] = []
            for f in rs_files:
                result2 = subprocess.run(
                    ["rustfmt", "--edition", "2021", "--check", str(f)],
                    capture_output=True, text=True,
                )
                if "error" in result2.stderr.lower():
                    fmt_errors.append(f"Rust: {f.relative_to(examples_dir.parent)}: {result2.stderr.strip()}")
            if fmt_errors:
                print(f"✗ ({len(fmt_errors)} error(s))")
                errors.extend(fmt_errors)
            else:
                print("✓")
        else:
            print(f"  Checking Rust format ({len(rs_files)} files) ... skipped (rustfmt unavailable)")

    # Note: Cargo check removed - examples are validated during actual smoke test execution
    # which catches compilation errors naturally without duplicate compilation overhead

    if errors:
        print(f"\n  ✗ {len(errors)} compilation error(s) found:")
        for e in errors:
            print(f"    {e}")
        return False
    return True


def cmd_list():
    connectors = list_connectors()
    print(f"Available connectors ({len(connectors)}):\n")
    for name in connectors:
        print(f"  {name}")


def cmd_generate(connectors: list[str], output_dir: Path, probe_path: Optional[Path] = None, update_llms: bool = True, syntax_check: bool = True):
    probe_data = load_probe_data(probe_path)
    if not probe_data:
        print("Error: No probe data available. Run field-probe first.", file=sys.stderr)
        sys.exit(1)
    
    print(f"Loaded probe data for {len(probe_data)} connectors from {probe_path}\n")

    output_dir.mkdir(parents=True, exist_ok=True)

    ok = 0
    skip = 0
    for name in connectors:
        print(f"  {name} ... ", end="", flush=True)
        probe_connector = probe_data.get(name, {})
        n_flows         = len(get_flows_from_probe(probe_connector))

        # Generate example files first so we can compute line numbers for doc links.
        scenario_files, scenario_lines, flow_lines_py_js     = generate_scenario_files(name, probe_connector, EXAMPLES_DIR)
        flow_files, flow_lines_kt_rs, scenario_lines_kt_rs   = generate_flow_files(name, probe_connector, EXAMPLES_DIR)

        # Supplement py/js line numbers from existing files when generate_scenario_files
        # returned early (e.g. scenario_groups missing from manifest).
        flow_items = _collect_flow_items(probe_connector, exclude_keys=set())
        for sdk, ext in [("python", "py"), ("javascript", "js")]:
            existing = EXAMPLES_DIR / name / sdk / f"{name}.{ext}"
            if existing.exists():
                content = existing.read_text(encoding="utf-8")
                for flow_key, _, _ in flow_items:
                    if sdk not in flow_lines_py_js.get(flow_key, {}):
                        lineno = _find_func_line(content, _flow_search(sdk, flow_key))
                        if lineno:
                            flow_lines_py_js.setdefault(flow_key, {})[sdk] = lineno

        # Merge py/js and kt/rs flow line numbers into one dict.
        merged_flow_lines: dict[str, dict[str, int]] = {}
        for flow_key, langs in flow_lines_py_js.items():
            merged_flow_lines.setdefault(flow_key, {}).update(langs)
        for flow_key, langs in flow_lines_kt_rs.items():
            merged_flow_lines.setdefault(flow_key, {}).update(langs)

        # Merge kt/rs scenario line numbers into the main scenario_lines dict.
        for scenario_key, langs in scenario_lines_kt_rs.items():
            scenario_lines.setdefault(scenario_key, {}).update(langs)

        doc = generate_connector_doc(
            name,
            probe_data=probe_data,
            scenario_line_numbers=scenario_lines,
            flow_line_numbers=merged_flow_lines,
        )
        if doc:
            out = output_dir / f"{name}.md"
            out.write_text(doc, encoding="utf-8")
            n_scenarios  = len(scenario_files) // 2  # python + js per scenario
            n_flow_files = len(flow_files)
            print(f"✓  ({n_flows} flows, {n_scenarios} scenarios, {n_flow_files} flow examples → {out.relative_to(REPO_ROOT)})")
            ok += 1
        else:
            print("skipped")
            skip += 1

    if update_llms:
        generate_llms_txt(probe_data, output_dir)
    print(f"\nDone: {ok} generated, {skip} skipped.")

    # Format generated Rust files before syntax check so the check sees clean files
    rs_files = list(EXAMPLES_DIR.rglob("*.rs"))
    if rs_files:
        print("  Formatting generated Rust files ...", end=" ", flush=True)
        try:
            result = subprocess.run(
                ["rustfmt", "--edition", "2021"] + [str(f) for f in rs_files],
                capture_output=True, text=True,
            )
            if result.returncode == 0:
                print("✓")
            else:
                print(f"✗ (some files may have formatting issues)")
        except FileNotFoundError:
            print("skipped (rustfmt not found)")

    if syntax_check:
        ok_syntax = check_example_syntax(EXAMPLES_DIR, connectors=connectors)
        if not ok_syntax:
            sys.exit(1)

    print("\n▶ Updating all_connector.md coverage matrix…")
    cmd_all_connectors_doc(output_dir, probe_path)


# ─── All Connectors Coverage Document ─────────────────────────────────────────

def _status_to_mark(status: str) -> str:
    """Map a probe status string to a display icon."""
    if status == "supported":
        return "✓"
    elif status == "not_supported":
        return "x"
    elif status == "not_implemented":
        return "⚠"
    else:
        return "?"


def _get_flow_status(flows: dict, flow_key: str) -> tuple[str, str]:
    """
    Get the status of a flow from probe data.
    Returns (status_mark, notes) tuple.
    """
    flow_data = flows.get(flow_key, {})
    
    # Handle case where flow exists in probe data but has no entries
    if not flow_data:
        return ("x", "")  # Not supported (no probe data for this flow)

    # For PM-aware flows, check if there's any supported PM
    if flow_key in _PM_AWARE_FLOWS:
        supported_pms = [
            pm for pm, data in flow_data.items()
            if pm != "default" and data.get("status") == "supported"
        ]
        if supported_pms:
            return ("✓", f"{len(supported_pms)} PMs")
        # Check if all PM entries are not_implemented → ⚠
        pm_entries = [pm for pm in flow_data.keys() if pm != "default"]
        if pm_entries:
            statuses = {flow_data[pm].get("status") for pm in pm_entries}
            if statuses == {"not_implemented"}:
                return ("⚠", "")
            if statuses == {"not_supported"}:
                return ("x", "")
            # Mixed or other statuses
            if "supported" in statuses:
                return ("✓", "")
            if "error" in statuses:
                return ("?", "Error")
            return ("x", "")
        # No PM entries at all
        return ("x", "")

    # For flows with PM-specific entries (non-PM-aware flows that have PM data)
    pm_entries = {pm: data for pm, data in flow_data.items() if pm != "default"}
    if pm_entries:
        # Aggregate status across all PMs
        statuses = {data.get("status") for data in pm_entries.values()}
        if "supported" in statuses:
            return ("✓", "")
        if statuses == {"not_implemented"}:
            return ("⚠", "")
        if statuses == {"not_supported"}:
            return ("x", "")
        if "error" in statuses:
            return ("?", "Error")
        return ("x", "")

    # For flows with only 'default' entry
    default_entry = flow_data.get("default", {})
    status = default_entry.get("status", "unknown")

    if status == "supported":
        return ("✓", "")
    elif status == "error":
        error_msg = default_entry.get("error", "")
        if len(error_msg) > 60:
            error_msg = error_msg[:57] + "..."
        return ("?", error_msg if error_msg else "Error")
    elif status == "not_supported":
        return ("x", "")
    elif status == "not_implemented":
        return ("⚠", "")
    else:
        return ("x", "")  # Default to not supported for unknown status


def generate_all_connector_doc(probe_data: dict[str, dict], output_dir: Path) -> None:
    """
    Generate all_connector.md - a comprehensive connector-wise flow coverage document.
    
    This creates a unified view showing:
    - For each flow, which connectors support which payment methods
    - Summary statistics for each connector and flow
    - Flow names follow proto service definitions from services.proto
    """
    out: list[str] = []
    a = out.append
    
    # ── Header ────────────────────────────────────────────────────────────────
    a("# Connector Flow Coverage")
    a("")
    a("<!--")
    a("This file is auto-generated. Do not edit by hand.")
    a("Source: data/field_probe/")
    a("Regenerate: make docs")
    a("-->")
    a("")
    a("This document provides a comprehensive overview of payment method support")
    a("across all connectors for each payment flow. Flow names follow the gRPC")
    a("service definitions from `crates/types-traits/grpc-api-types/proto/services.proto`.")
    a("")
    
    # Get all connectors that have probe data
    connectors_with_probe = sorted(probe_data.keys())
    
    if not connectors_with_probe:
        a("No probe data available.")
        output_dir.mkdir(parents=True, exist_ok=True)
        out_path = output_dir.parent / "all_connector.md"
        out_path.write_text("\n".join(out), encoding="utf-8")
        return
    
    # ── Per-Service Flow Coverage Tables ───────────────────────────────────────
    a("## Flow Coverage")
    a("")
    a("Flow names follow the gRPC service definitions. Each flow is prefixed with")
    a("its service name (e.g., `PaymentService.Authorize`, `RefundService.Get`).")
    a("")
    
    # Group flows by service
    services_order = [
        "PaymentService",
        "RecurringPaymentService",
        "RefundService",
        "CustomerService",
        "PaymentMethodService",
        "MerchantAuthenticationService",
        "PaymentMethodAuthenticationService",
        "DisputeService",
        "EventService",
    ]
    
    # Build service -> flows mapping from flow_metadata loaded from probe.json
    proto_flow_defs = get_proto_flow_definitions()
    if not proto_flow_defs:
        print("Warning: No flow metadata loaded from probe.json", file=sys.stderr)
    service_flows: dict[str, list[tuple[str, str, str]]] = {}
    for flow_key, (service_name, rpc_name, description) in proto_flow_defs.items():
        service_flows.setdefault(service_name, []).append((flow_key, rpc_name, description))
    
    # Separate PM-aware flows from simple status flows
    pm_aware_flows_data = []
    simple_flows_data = []
    
    for service_name in services_order:
        if service_name not in service_flows:
            continue
        
        flows_in_service = service_flows[service_name]
        
        for flow_key, rpc_name, description in flows_in_service:
            if flow_key in _PM_AWARE_FLOWS:
                pm_aware_flows_data.append((service_name, flow_key, rpc_name, description))
            else:
                simple_flows_data.append((service_name, flow_key, rpc_name, description))
    
    # Render PM-aware flows (like Authorize) with full payment method breakdown
    for service_name, flow_key, rpc_name, description in pm_aware_flows_data:
        a(f"### {service_name}.{rpc_name}")
        a("")
        a(description)
        a("")
        
        # Build display names with category prefix for clarity
        pm_display_with_category = []
        pm_keys_ordered = []
        for category, pm_list in _PROBE_PM_BY_CATEGORY:
            for pm_key, pm_name in pm_list:
                pm_keys_ordered.append(pm_key)
                # Shorten category names for compact display
                short_cat = {
                    "Card": "CARD",
                    "Wallet": "WALLET", 
                    "BNPL": "BNPL",
                    "UPI": "UPI",
                    "Online Banking": "Online Banking",
                    "Open Banking": "Open Banking",
                    "Bank Redirect": "Bank Redirect",
                    "Bank Transfer": "Bank Transfer",
                    "Bank Debit": "Bank Debit",
                    "Alternative": "Alternate PMs "
                }.get(category, category[:4].upper())
                pm_display_with_category.append(f"{short_cat} / {pm_name}")
        
        # Legend at top for clarity
        a("**Legend:** ✓ Supported | x Not Supported | ⚠ Not Implemented | ? Error / Missing required fields")
        a("")
        
        a("| Connector | " + " | ".join(pm_display_with_category) + " |")
        a("|-----------|" + "|".join([":---:" for _ in pm_display_with_category]) + "|")
        
        for conn_name in connectors_with_probe:
            conn_data = probe_data[conn_name]
            flow_data = conn_data.get("flows", {}).get(flow_key, {})
            
            display = _DISPLAY_NAMES.get(conn_name, conn_name.replace("_", " ").title())
            row = [f"[{display}](connectors/{conn_name}.md)"]
            
            for pm_key in pm_keys_ordered:
                pm_data = flow_data.get(pm_key, {})
                status = pm_data.get("status", "unknown")
                row.append(_status_to_mark(status))
            
            a("| " + " | ".join(row) + " |")
        a("")
    
    # Render consolidated table for all simple flows (Get, Void, Refund, etc.)
    if simple_flows_data:
        a("### Other Flows")
        a("")
        a("Consolidated view of Get, Void, Refund, Capture, Reverse, CreateOrder, and other non-payment flows.")
        a("")
        
        # Build header with flow names
        flow_headers = []
        for service_name, flow_key, rpc_name, description in simple_flows_data:
            # Shorten service name for compact display
            short_service = service_name.replace("Service", "").replace("Payment", "Pay").replace("Recurring", "Rec")
            flow_headers.append(f"{short_service}.{rpc_name}")
        
        # Legend at top for clarity
        a("**Legend:** ✓ Supported | x Not Supported | ⚠ Not Implemented | ? Error / Missing required fields")
        a("")
        
        a("| Connector | " + " | ".join(flow_headers) + " |")
        a("|-----------|" + "|".join([":---:" for _ in simple_flows_data]) + "|")
        
        for conn_name in connectors_with_probe:
            conn_data = probe_data[conn_name]
            flows = conn_data.get("flows", {})
            
            display = _DISPLAY_NAMES.get(conn_name, conn_name.replace("_", " ").title())
            row = [f"[{display}](connectors/{conn_name}.md)"]
            
            for service_name, flow_key, rpc_name, description in simple_flows_data:
                status_mark, _ = _get_flow_status(flows, flow_key)
                row.append(status_mark)
            
            a("| " + " | ".join(row) + " |")
        a("")
    
    # ── Services Reference ─────────────────────────────────────────────────────
    a("## Services Reference")
    a("")
    a("Flow definitions are derived from `crates/types-traits/grpc-api-types/proto/services.proto`:")
    a("")
    a("| Service | Description |")
    a("|---------|-------------|")
    a("| PaymentService | Process payments from authorization to settlement |")
    a("| RecurringPaymentService | Charge and revoke recurring payments |")
    a("| RefundService | Retrieve and synchronize refund statuses |")
    a("| CustomerService | Create and manage customer profiles |")
    a("| PaymentMethodService | Tokenize and retrieve payment methods |")
    a("| MerchantAuthenticationService | Generate access tokens and session credentials |")
    a("| PaymentMethodAuthenticationService | Execute 3D Secure authentication flows |")
    a("| DisputeService | Manage chargeback disputes |")
    a("| EventService | Handle connector webhook events |")
    a("")
    
    # Write output
    output_dir.mkdir(parents=True, exist_ok=True)
    out_path = output_dir.parent / "all_connector.md"
    out_path.write_text("\n".join(out), encoding="utf-8")
    print(f"  ✓ Generated {out_path.relative_to(REPO_ROOT)}")


def cmd_all_connectors_doc(output_dir: Path, probe_path: Optional[Path] = None):
    """Generate the all_connector.md coverage document."""
    probe_data = load_probe_data(probe_path)
    if not probe_data:
        print("Error: No probe data available. Run field-probe first.", file=sys.stderr)
        sys.exit(1)
    
    print(f"Generating all_connector.md from {len(probe_data)} connectors\n")
    generate_all_connector_doc(probe_data, output_dir)
    print("\nDone.")


def generate_rust_build_auth(proto_dir: Path, out_file: Path) -> None:
    """Generate sdk/rust/smoke-test/src/build_auth.rs from payment.proto."""
    import re as _re

    proto_text = (proto_dir / "payment.proto").read_text(encoding="utf-8")
    # Strip comments
    proto_text = _re.sub(r"//[^\n]*", "", proto_text)
    proto_text = _re.sub(r"/\*.*?\*/", "", proto_text, flags=_re.DOTALL)

    # Find ConnectorSpecificConfig oneof body
    csc_m = _re.search(
        r"message\s+ConnectorSpecificConfig\s*\{.*?oneof\s+config\s*\{(.*?)\}\s*\}",
        proto_text, _re.DOTALL,
    )
    if not csc_m:
        print("  WARNING: ConnectorSpecificConfig oneof not found, skipping build_auth.rs")
        return

    oneof_body = csc_m.group(1)
    # Extract: TypeName field_name = num;  (e.g. AdyenConfig adyen = 1;)
    config_variants = _re.findall(r"(\w+Config)\s+(\w+)\s*=\s*\d+\s*;", oneof_body)

    # Parse each *Config message for its fields
    config_fields: dict[str, list[tuple[str, str]]] = {}
    for type_name, field_name in config_variants:
        msg_m = _re.search(
            rf"message\s+{type_name}\s*\{{(.*?)\}}",
            proto_text, _re.DOTALL
        )
        if msg_m:
            body = msg_m.group(1)
            # field lines: string api_key = 1;
            fields = _re.findall(r"(\w+)\s+(\w+)\s*=\s*\d+", body)
            config_fields[field_name] = fields
        else:
            config_fields[field_name] = []

    # Build auth.rs content
    lines: list[str] = [
        "// Auto-generated from payment.proto — do not edit manually",
        "// Regenerate: python3 scripts/generators/docs/generate.py",
        "",
        "use payments::generated::payment_pb2;",
        "use std::env;",
        "",
        "/// Build ConnectorAuthType from environment variables",
        "/// Environment variable format: {CONNECTOR_NAME}_API_KEY, etc.",
        "pub fn build_auth(connector_name: &str) -> payment_pb2::ConnectorAuthType {",
        "    match connector_name {",
    ]
    for field_name in sorted(config_fields.keys()):
        conn_var = field_name.lower()
        env_prefix = conn_var.upper()
        lines.append(f'        "{conn_var}" => {{')
        lines.append(f'            let mut auth = payment_pb2::ConnectorAuthType::new();')
        lines.append(f'            let mut specific = payment_pb2::ConnectorSpecificConfig::new();')
        lines.append(f'            let mut config = payment_pb2::{conn_var.title()}Config::new();')
        for _, fname in config_fields[field_name]:
            env_var = f"{env_prefix}_{fname.upper()}"
            lines.append(f'            config.{fname} = env::var("{env_var}").unwrap_or_default();')
        lines.append(f'            specific.set_{conn_var}(config);')
        lines.append(f'            auth.set_connector_specific_config(specific);')
        lines.append(f'            auth')
        lines.append(f'        }}')
    lines.append('        _ => payment_pb2::ConnectorAuthType::new(),')
    lines.append('    }')
    lines.append('}')
    lines.append('')

    out_file.parent.mkdir(parents=True, exist_ok=True)
    out_file.write_text("\n".join(lines), encoding="utf-8")
    print(f"  {out_file.relative_to(REPO_ROOT)}")


def main():
    import argparse
    parser = argparse.ArgumentParser(
        description="Generate connector documentation from field-probe data"
    )
    
    parser.add_argument(
        "connectors",
        nargs="*",
        help="Connector names to generate docs for"
    )
    parser.add_argument(
        "--all",
        action="store_true",
        help="Generate docs for all connectors"
    )
    parser.add_argument(
        "--list",
        action="store_true",
        help="List all available connectors"
    )
    parser.add_argument(
        "--probe-path",
        type=Path,
        default=REPO_ROOT / "data" / "field_probe",
        help="Path to field-probe output directory (default: data/field_probe)"
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=DOCS_DIR,
        help="Output directory for generated docs (default: docs-generated/connectors)"
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Run compilation checks on existing generated examples without regenerating"
    )
    parser.add_argument(
        "--no-syntax-check",
        action="store_true",
        help="Skip compilation checks after generation (useful for fast local iteration)"
    )

    args = parser.parse_args()

    syntax_check = not args.no_syntax_check

    # Standalone check mode — validate existing examples without regenerating
    if args.check:
        connectors = args.connectors or None
        ok = check_example_syntax(EXAMPLES_DIR, connectors=connectors)
        sys.exit(0 if ok else 1)

    load_probe_data(args.probe_path)

    if args.list:
        cmd_list()
        return

    if args.all:
        connectors = list_connectors()
        if not connectors:
            print("Error: No connectors found. Run field-probe first.", file=sys.stderr)
            sys.exit(1)
        cmd_generate(connectors, args.output_dir, args.probe_path, syntax_check=syntax_check)
    elif args.connectors:
        cmd_generate(args.connectors, args.output_dir, args.probe_path, update_llms=False, syntax_check=syntax_check)
    else:
        parser.print_help()
        sys.exit(1)


if __name__ == "__main__":
    main()
