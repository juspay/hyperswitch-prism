#!/usr/bin/env python3
"""
SDK Code Generator — Generates type-safe client methods for all SDKs.

This generator cross-references:
  1. services.proto (via protoc descriptor) → RPC definitions with types and docs
  2. services/*.rs → which flows have req_transformer implementations

Generates flow methods (authorize, capture, refund, etc.) for each SDK,
and the Rust FFI flow registration files.

Usage:
    # Generate all SDKs + Rust FFI registrations
    python3 scripts/generators/code/generate.py
    make generate

    # Generate specific language only
    python3 scripts/generators/code/generate.py --lang python
    python3 scripts/generators/code/generate.py --lang javascript
    python3 scripts/generators/code/generate.py --lang kotlin
    python3 scripts/generators/code/generate.py --lang rust
    python3 scripts/generators/code/generate.py --lang php
"""

import re
import sys
from pathlib import Path
from typing import Optional

from jinja2 import Environment, FileSystemLoader

REPO_ROOT = Path(__file__).parent.parent.parent.parent
SDK_ROOT = REPO_ROOT / "sdk"
TEMPLATES_DIR = Path(__file__).parent / "templates"

SERVICES_PROTO = REPO_ROOT / "crates/types-traits/grpc-api-types/proto/services.proto"
FFI_SERVICES_DIR = REPO_ROOT / "crates/ffi/ffi/src/services"
PROTO_DESCRIPTOR = Path(__file__).parent / "services.desc"

RUST_HANDLERS_OUT = REPO_ROOT / "crates/ffi/ffi/src/handlers/_generated_flow_registrations.rs"
RUST_FFI_FLOWS_OUT = REPO_ROOT / "crates/ffi/ffi/src/bindings/_generated_ffi_flows.rs"
RUST_GRPC_CLIENT_OUT       = SDK_ROOT  / "rust/src/_generated_grpc_client.rs"
JS_GRPC_CLIENT_OUT         = SDK_ROOT  / "javascript/src/payments/_generated_grpc_client.ts"
JS_GRPC_EXAMPLE_FLOWS_OUT  = REPO_ROOT / "examples/_generated_grpc_example_flows.js"
PY_GRPC_CLIENT_OUT         = SDK_ROOT  / "python/src/payments/_generated_grpc_client.py"
KOTLIN_GRPC_CLIENT_OUT     = SDK_ROOT  / "java/src/main/kotlin/payments/GrpcClient.kt"

# ── Jinja2 environment ──────────────────────────────────────────────────────

env = Environment(
    loader=FileSystemLoader(TEMPLATES_DIR),
    keep_trailing_newline=True,
    lstrip_blocks=True,
    trim_blocks=True,
)


def ensure_descriptor_exists() -> None:
    """Verify proto descriptor file exists and is readable."""
    if not PROTO_DESCRIPTOR.exists():
        print(
            f"ERROR: Proto descriptor not found: {PROTO_DESCRIPTOR}",
            file=sys.stderr,
        )
        print(
            "Run 'make generate' from the sdk directory to generate the descriptor.",
            file=sys.stderr,
        )
        sys.exit(1)


# ── Source parsing ───────────────────────────────────────────────────────────

def to_snake_case(name: str) -> str:
    """'CreateAccessToken' -> 'create_access_token'"""
    s = re.sub("(.)([A-Z][a-z]+)", r"\1_\2", name)
    return re.sub("([a-z0-9])([A-Z])", r"\1_\2", s).lower()


def _service_flow_prefix(service_name: str) -> str | None:
    """
    Derive the transformer name prefix for services whose RPCs collide with
    PaymentService.  Only services of the form ``{Prefix}PaymentService`` get a

    Base services (``PaymentService``, ``RecurringPaymentService``,
    ``PaymentMethodAuthenticationService``, etc.) return ``None``.
    
    Also handles standalone services like ``PayoutService`` → ``"payout"``.
    """
    snake = to_snake_case(service_name)          # e.g. "tokenized_payment_service"
    without_svc = snake.removesuffix("_service") # e.g. "tokenized_payment"
    if without_svc.endswith("_payment"):
        prefix = without_svc.removesuffix("_payment")
        return prefix or None  # guard against plain "payment_service" → ""
    # Handle standalone services like PayoutService, CustomerService, etc.
    return without_svc if without_svc else None


def parse_proto_rpcs(desc_file: Path) -> dict[str, dict]:
    """
    Parse RPC definitions from a protobuf descriptor file.

    Uses protoc-generated descriptor to properly handle:
      - All proto syntax (imports, nested types, options)
      - Request/response type names
      - Parent service name
      - Original PascalCase RPC name
      - Doc comments from SourceCodeInfo

    Returns {snake_case_rpc_name: {...}}.
    First-occurrence wins for the plain snake key.  When a service-variant
    RPC collides with an existing plain key, the entry is
    *also* stored under ``{service_prefix}_{rpc_snake}`` so that transformer
    names like ``proxied_authorize`` or ``tokenized_setup_recurring`` can be
    matched by ``discover_flows()``.
    """
    from google.protobuf.descriptor_pb2 import FileDescriptorSet

    with open(desc_file, 'rb') as f:
        desc_set = FileDescriptorSet.FromString(f.read())

    rpcs: dict[str, dict] = {}

    for file_desc in desc_set.file:
        # Build source info lookup for doc comments
        # Location path: [service_index, method_index]
        source_info = {}
        if file_desc.source_code_info:
            for location in file_desc.source_code_info.location:
                path = tuple(location.path)
                if location.leading_comments:
                    source_info[path] = location.leading_comments.strip()

        for svc_idx, service in enumerate(file_desc.service):
            svc_name = service.name
            if svc_name.endswith("Service"):
                svc_prefix = to_snake_case(svc_name[:-7])
            else:
                svc_prefix = to_snake_case(svc_name)

            for method_idx, method in enumerate(service.method):
                rpc_name = method.name
                snake = to_snake_case(rpc_name)
                full_snake = f"{svc_prefix}_{snake}"

                # Extract type names (remove package prefix)
                req_type = method.input_type.split('.')[-1]
                res_type = method.output_type.split('.')[-1]

                # Get doc comment if available
                # Path for method: [6 (service), svc_idx, 2 (method), method_idx]
                path = (6, svc_idx, 2, method_idx)
                comment = source_info.get(path, f"{service.name}.{rpc_name}")
                # Normalize whitespace to single-line
                comment = ' '.join(comment.split())

                info = {
                    "request": req_type,
                    "response": res_type,
                    "service": service.name,
                    "rpc": rpc_name,
                    "description": comment,
                }

                # Extract type names (remove package prefix)
                req_type = method.input_type.split('.')[-1]
                res_type = method.output_type.split('.')[-1]

                # Get doc comment if available
                # Path for method: [6 (service), svc_idx, 2 (method), method_idx]
                path = (6, svc_idx, 2, method_idx)
                comment = source_info.get(path, f"{service.name}.{rpc_name}")
                comment = ' '.join(comment.split())

                entry = {
                    "request": req_type,
                    "response": res_type,
                    "service": service.name,
                    "rpc": rpc_name,
                    "description": comment,
                }

                prefix = _service_flow_prefix(service.name)
                if snake not in rpcs:
                    # First occurrence — store under plain snake key.
                    rpcs[snake] = entry
                    # Also store under "{prefix}_{snake}" so that transformers
                    # like "payout_stage" or "payout_transfer" can be discovered
                    # even when they don't collide with another service's RPC.
                    if prefix:
                        prefixed = f"{prefix}_{snake}"
                        if prefixed not in rpcs:
                            rpcs[prefixed] = entry
                else:
                    # Collision: the RPC name is shared across services.
                    # Also store under "{service_prefix}_{rpc_snake}" so that
                    # transformers like "proxied_authorize" can be discovered.
                    if prefix:
                        prefixed = f"{prefix}_{snake}"
                        if prefixed not in rpcs:
                            rpcs[prefixed] = entry

    return rpcs


def parse_service_flows(services_dir: Path) -> dict[str, str]:
    """
    Scan services/payments.rs for every req_transformer implementation.

    Two patterns are matched:
    1. ``fn_name: {flow}_req_transformer`` — inside a ``req_transformer!`` macro
       invocation (the standard codegen path).
    2. ``pub fn {flow}_req_transformer`` — an explicit wrapper function used when
       a pre-conversion step is needed before delegating to the standard transformer
    """
    flows = {}
    for service_file in services_dir.glob("*.rs"):
        module = service_file.stem
        text = service_file.read_text()
        for m in re.finditer(
            r"(?:fn_name:\s*|pub fn )(\w+)_req_transformer\b", text
        ):
            flows[m.group(1)] = module
    return flows


def parse_single_flows(services_dir: Path) -> dict[str, str]:
    """
    Scan all .rs files in services directory for hand-written single-step transformers.
    These are `pub fn {flow}_transformer` functions that are NOT req/res macros —
    they take the request directly and return the response without an HTTP round-trip
    (e.g. webhook processing via `handle_transformer`).

    Explicitly excludes ``_req_transformer`` and ``_res_transformer`` functions
    (those are handled by ``parse_service_flows``).
    """
    flows = {}
    for service_file in services_dir.glob("*.rs"):
        module = service_file.stem
        text = service_file.read_text()
        for m in re.finditer(r"^pub fn (\w+)_transformer\b", text, re.MULTILINE):
            if not m.group(1).endswith(("_req", "_res")):
                flows[m.group(1)] = module
    return flows


def discover_flows() -> tuple[list[dict], list[dict]]:
    """
    Cross-reference proto RPCs with implemented service transformers.
    Returns (standard_flows, single_flows) — both sorted by name.
    Standard flows use req+HTTP+res; single flows call the transformer directly.
    """
    proto_rpcs = parse_proto_rpcs(PROTO_DESCRIPTOR)
    service_flows = parse_service_flows(FFI_SERVICES_DIR)
    single_flow_names = parse_single_flows(FFI_SERVICES_DIR)

    errors = []
    flows = []
    for flow in sorted(service_flows):
        if flow not in proto_rpcs:
            errors.append(
                f"  ERROR: '{flow}_req_transformer' exists in services/*.rs but has no matching RPC in services.proto"
            )
            continue
        flows.append({"name": flow, "module": service_flows[flow], **proto_rpcs[flow]})

    single_flows = []
    for flow in sorted(single_flow_names):
        if flow not in proto_rpcs:
            errors.append(
                f"  ERROR: '{flow}_transformer' exists in services/*.rs but has no matching RPC in services.proto"
            )
            continue
        single_flows.append({"name": flow, "module": single_flow_names[flow], **proto_rpcs[flow]})

    if errors:
        for e in errors:
            print(e, file=sys.stderr)
        sys.exit(1)

    implemented = set(service_flows.keys()) | set(single_flow_names.keys())
    unimplemented = sorted(set(proto_rpcs) - implemented)
    if unimplemented:
        print(f"  Proto RPCs not yet implemented (skipped): {unimplemented}")

    return flows, single_flows


# ── Helpers ──────────────────────────────────────────────────────────────────

def write(path: Path, content: str) -> None:
    path.write_text(content)
    print(f"  wrote {path.relative_to(REPO_ROOT)}")


def service_to_client_name(service: str) -> str:
    """'PaymentService' -> 'PaymentClient', 'MerchantAuthenticationService' -> 'MerchantAuthenticationClient'"""
    return service[:-7] + "Client" if service.endswith("Service") else service + "Client"


def group_by_service(flows: list[dict]) -> dict[str, list[dict]]:
    """Group flows by their proto service name. Returns {service_name: [flow, ...]}."""
    groups: dict[str, list[dict]] = {}
    for f in flows:
        groups.setdefault(f["service"], []).append(f)
    return groups


def to_camel(snake: str) -> str:
    """'create_access_token' -> 'createAccessToken'"""
    return re.sub(r"_([a-z])", lambda m: m.group(1).upper(), snake)


def service_to_tonic_mod(service: str) -> str:
    """'PaymentService' -> 'payment_service_client'"""
    return to_snake_case(service) + "_client"


def service_to_grpc_struct(service: str) -> str:
    """'PaymentService' -> 'GrpcPaymentClient'"""
    base = service[:-7] if service.endswith("Service") else service
    return f"Grpc{base}Client"


def service_to_grpc_field(service: str) -> str:
    """'PaymentService' -> 'payment'  |  'RecurringPaymentService' -> 'recurring_payment'"""
    base = service[:-7] if service.endswith("Service") else service
    return to_snake_case(base)


def service_to_grpc_js_field(service: str) -> str:
    """'RecurringPaymentService' -> 'recurringPayment' (camelCase JS field on GrpcClient)"""
    return to_camel(service_to_grpc_field(service))


def grpc_method_path(service: str, rpc_name: str) -> str:
    """{service_field}/{rpc_name} — matches the Rust FFI dispatch table."""
    return f"{service_to_grpc_field(service)}/{rpc_name}"


# Special-case grpc_* function names for flows where the bare RPC name is
# ambiguous or non-descriptive (e.g. CustomerService.Create → "create_customer").
_GRPC_EXAMPLE_FN_OVERRIDES: dict[tuple[str, str], str] = {
    ("CustomerService",        "create"): "create_customer",
    ("RecurringPaymentService", "charge"): "recurring_charge",
}


def grpc_example_fn_name(service: str, rpc_name: str) -> str:
    """Canonical grpc_* smoke-test function suffix for a (service, rpc) pair."""
    return _GRPC_EXAMPLE_FN_OVERRIDES.get((service, rpc_name), rpc_name)


# Register helpers as Jinja2 globals so templates can call them directly.
env.globals["service_to_client_name"]    = service_to_client_name
env.globals["service_to_tonic_mod"]      = service_to_tonic_mod
env.globals["service_to_grpc_struct"]    = service_to_grpc_struct
env.globals["service_to_grpc_field"]     = service_to_grpc_field
env.globals["service_to_grpc_js_field"]  = service_to_grpc_js_field
env.globals["grpc_method_path"]          = grpc_method_path
env.globals["grpc_example_fn_name"]      = grpc_example_fn_name
env.globals["to_camel"]      = to_camel
env.globals["to_snake_case"] = to_snake_case


# ── Generators ───────────────────────────────────────────────────────────────

def render(template_name: str, output_path: Path, **kwargs) -> None:
    """Render a Jinja2 template and write to output_path."""
    tmpl = env.get_template(template_name)
    content = tmpl.render(**kwargs).rstrip("\n") + "\n"
    write(output_path, content)


def gen_python(flows: list[dict], single_flows: list[dict]) -> None:
    render(
        "python/flows.py.j2",
        SDK_ROOT / "python/src/payments/_generated_flows.py",
        groups=group_by_service(flows),
        single_groups=group_by_service(single_flows),
    )


def gen_python_clients(flows: list[dict], single_flows: list[dict]) -> None:
    """Generate _generated_service_clients.py — per-service client classes."""
    groups = group_by_service(flows)
    single_groups = group_by_service(single_flows)
    all_groups = {**groups}
    for service in single_groups:
        all_groups.setdefault(service, [])

    render(
        "python/clients.py.j2",
        SDK_ROOT / "python/src/payments/_generated_service_clients.py",
        all_services=sorted(all_groups),
        groups=groups,
        single_groups=single_groups,
    )


def gen_python_stub(flows: list[dict], single_flows: list[dict] = []) -> None:
    """Generate connector_client.pyi — per-service client stubs for IDE completions."""
    groups = group_by_service(flows)
    single_groups = group_by_service(single_flows)

    types: set[str] = set()
    for f in flows + single_flows:
        types.add(f["request"])
        types.add(f["response"])

    render(
        "python/stub.pyi.j2",
        SDK_ROOT / "python/src/payments/connector_client.pyi",
        imports=sorted(types),
        all_services=sorted(set(groups) | set(single_groups)),
        groups=groups,
        single_groups=single_groups,
    )


def gen_javascript(flows: list[dict], single_flows: list[dict]) -> None:
    gen_flows_js(flows, single_flows)
    gen_connector_client_ts(flows, single_flows)
    gen_uniffi_client_ts(flows, single_flows)
    gen_javascript_grpc_client()


def gen_flows_js(flows: list[dict], single_flows: list[dict]) -> None:
    """Generate _generated_flows.js — flow metadata used by UniffiClient for FFI symbol dispatch."""
    max_len = max((len(f["name"]) for f in flows), default=0)
    max_len_s = max((len(f["name"]) for f in single_flows), default=0)

    render(
        "javascript/flows.js.j2",
        SDK_ROOT / "javascript/src/payments/_generated_flows.js",
        flows=flows,
        single_flows=single_flows,
        max_len=max_len,
        max_len_s=max_len_s,
    )


def gen_connector_client_ts(flows: list[dict], single_flows: list[dict]) -> None:
    """Generate _generated_connector_client_flows.ts — per-service client classes."""
    groups = group_by_service(flows)
    single_groups = group_by_service(single_flows)
    all_services = sorted(set(groups) | set(single_groups))

    render(
        "javascript/connector_client.ts.j2",
        SDK_ROOT / "javascript/src/payments/_generated_connector_client_flows.ts",
        all_services=all_services,
        groups=groups,
        single_groups=single_groups,
    )


def gen_uniffi_client_ts(flows: list[dict], single_flows: list[dict]) -> None:
    """Generate _generated_uniffi_client_flows.ts — UniffiClient subclass with typed flow methods."""
    render(
        "javascript/uniffi_client.ts.j2",
        SDK_ROOT / "javascript/src/payments/_generated_uniffi_client_flows.ts",
        flows=flows,
        single_flows=single_flows,
    )


KOTLIN_UNIFFI_BINDINGS = SDK_ROOT / "java/src/main/kotlin/generated/uniffi/connector_service_ffi/connector_service_ffi.kt"


def _available_uniffi_transformers() -> Optional[set[str]]:
    """
    Parse the generated uniffi Kotlin bindings to find which transformer
    functions are actually available. Returns None if the file doesn't exist
    (treated as "all available" — don't filter).
    """
    if not KOTLIN_UNIFFI_BINDINGS.exists():
        return None
    text = KOTLIN_UNIFFI_BINDINGS.read_text()
    found: set[str] = set()
    # Matches both standard flows (foo_req_transformer) and single-step flows (foo_transformer)
    for m in re.finditer(r"fn_func_(\w+_transformer)\b", text):
        found.add(m.group(1))
    return found


def gen_kotlin(flows: list[dict], single_flows: list[dict] = []) -> None:
    available = _available_uniffi_transformers()

    if available is not None:
        filtered_flows = []
        for f in flows:
            symbol = f"{f['name']}_req_transformer"
            if symbol in available:
                filtered_flows.append(f)
            else:
                print(
                    f"  WARNING: '{symbol}' not in uniffi bindings — skipping '{f['name']}' "
                    "from Kotlin SDK. Run 'make -C sdk/java generate-bindings' to include it.",
                    file=sys.stderr,
                )
        flows = filtered_flows

        filtered_single = []
        for f in single_flows:
            symbol = f"{f['name']}_transformer"
            if symbol in available:
                filtered_single.append(f)
            else:
                print(
                    f"  WARNING: '{symbol}' not in uniffi bindings — skipping '{f['name']}' "
                    "from Kotlin SDK. Run 'make -C sdk/java generate-bindings' to include it.",
                    file=sys.stderr,
                )
        single_flows = filtered_single

    groups = group_by_service(flows)
    single_groups = group_by_service(single_flows)
    all_services = sorted(set(groups) | set(single_groups))

    render(
        "kotlin/flows.kt.j2",
        SDK_ROOT / "java/src/main/kotlin/GeneratedFlows.kt",
        flows=flows,
        single_flows=single_flows,
        all_services=all_services,
        groups=groups,
        single_groups=single_groups,
    )


def gen_rust_handlers(flows: list[dict]) -> None:
    """Generate _generated_flow_registrations.rs — included by handlers/payments.rs."""
    modules = {}
    for f in flows:
        mod = f["module"]
        if mod not in modules:
            modules[mod] = set()
        modules[mod].add(f["request"])
        modules[mod].add(f["response"])

    modules_sorted = {}
    for mod in modules:
        modules_sorted[mod] = sorted(modules[mod])

    flows_by_module = {}
    for f in flows:
        mod = f["module"]
        if mod not in flows_by_module:
            flows_by_module[mod] = []
        flows_by_module[mod].append(f)

    render(
        "rust/handlers.rs.j2",
        RUST_HANDLERS_OUT,
        flows=flows,
        modules=modules_sorted,
        flows_by_module=flows_by_module,
    )


def gen_rust_ffi_flows(flows: list[dict]) -> None:
    """Generate _generated_ffi_flows.rs — included by bindings/uniffi.rs."""
    modules_req = {}
    for f in flows:
        mod = f["module"]
        if mod not in modules_req:
            modules_req[mod] = set()
        modules_req[mod].add(f["request"])

    modules_req_sorted = {}
    for mod in modules_req:
        modules_req_sorted[mod] = sorted(modules_req[mod])

    render(
        "rust/ffi_flows.rs.j2",
        RUST_FFI_FLOWS_OUT,
        flows=flows,
        modules_req=modules_req_sorted,
    )


# ── PHP generators ───────────────────────────────────────────────────────────

def gen_php(flows: list[dict], single_flows: list[dict]) -> None:
    gen_php_flows(flows, single_flows)
    gen_php_service_clients(flows, single_flows)


def gen_php_flows(flows: list[dict], single_flows: list[dict]) -> None:
    """Generate _GeneratedFlows.php — PHP flow registry used by UniffiClient."""
    max_len = max((len(f["name"]) for f in flows), default=0)
    max_len_s = max((len(f["name"]) for f in single_flows), default=0)
    render(
        "php/flows.php.j2",
        SDK_ROOT / "php/src/Payments/_GeneratedFlows.php",
        flows=flows,
        single_flows=single_flows,
        max_len=max_len,
        max_len_s=max_len_s,
    )


def gen_php_service_clients(flows: list[dict], single_flows: list[dict]) -> None:
    """Generate _GeneratedServiceClients.php — per-service PHP client classes."""
    groups = group_by_service(flows)
    single_groups = group_by_service(single_flows)
    all_services = sorted(set(groups) | set(single_groups))

    render(
        "php/clients.php.j2",
        SDK_ROOT / "php/src/Payments/_GeneratedServiceClients.php",
        all_services=all_services,
        groups=groups,
        single_groups=single_groups,
    )


# ── gRPC generators ─────────────────────────────────────────────────────────

def _grpc_groups() -> tuple[list[str], dict[str, list[dict]]]:
    """Shared helper: all proto RPCs grouped by service (used by JS + Rust gRPC generators).
    
    Returns all RPCs grouped by service. For services where the simple RPC name
    is already taken by another service (e.g., PaymentService.Authorize), uses
    the prefixed name (e.g., tokenized_authorize, proxied_authorize).
    """
    all_rpcs = parse_proto_rpcs(PROTO_DESCRIPTOR)
    
    # First pass: identify which service owns each simple RPC name
    simple_name_to_service: dict[str, str] = {}
    for flow_name, meta in all_rpcs.items():
        rpc_simple_name = to_snake_case(meta["rpc"])
        if flow_name == rpc_simple_name:
            simple_name_to_service[rpc_simple_name] = meta["service"]
    
    # Second pass: group flows by service
    groups: dict[str, list[dict]] = {}
    for flow_name, meta in sorted(all_rpcs.items(), key=lambda kv: kv[1]["service"]):
        rpc_simple_name = to_snake_case(meta["rpc"])
        service = meta["service"]
        
        # If this flow uses the simple name, always include it
        if flow_name == rpc_simple_name:
            groups.setdefault(service, []).append({"name": flow_name, **meta})
        # If this flow uses a prefixed name and the simple name belongs to a DIFFERENT service,
        # include it (e.g., tokenized_authorize when authorize belongs to PaymentService)
        elif simple_name_to_service.get(rpc_simple_name) != service:
            groups.setdefault(service, []).append({"name": flow_name, **meta})
        # Otherwise skip (it's a duplicate like payment_authorize when authorize exists)
    
    return list(groups.keys()), groups


def gen_python_grpc_client() -> None:
    """Generate _generated_grpc_client.py — Python gRPC sub-clients and GrpcClient from proto RPCs."""
    services, groups = _grpc_groups()
    render(
        "python/grpc_client.py.j2",
        PY_GRPC_CLIENT_OUT,
        services=services,
        groups=groups,
    )


def gen_kotlin_grpc_client() -> None:
    """Generate GrpcClient.kt — Kotlin gRPC sub-clients and GrpcClient from proto RPCs."""
    services, groups = _grpc_groups()
    render(
        "kotlin/grpc_client.kt.j2",
        KOTLIN_GRPC_CLIENT_OUT,
        services=services,
        groups=groups,
    )


# All proto message types that serialize as plain strings in Rust serde but need
# {value: "..."} wrapping for protobufjs fromObject.
_VALUE_WRAPPER_TYPES = frozenset([
    ".types.SecretString",
    ".types.CardNumberType",
    ".types.NetworkTokenType",
])


def _collect_proto_field_maps(
    desc_file: Path,
) -> tuple[dict[str, list[str]], dict[str, dict[str, str]]]:
    """
    Parse proto descriptor in one pass and return:
      secret_fields:  {MessageName: [camelCaseFieldName]}  — fields typed SecretString
      msg_field_types:{MessageName: {camelCaseFieldName: NestedTypeName}} — other message fields
    Both maps are keyed by short message name (e.g. "Ach", not ".types.Ach").
    """
    from google.protobuf.descriptor_pb2 import FileDescriptorSet, FieldDescriptorProto

    with open(desc_file, "rb") as f:
        desc_set = FileDescriptorSet.FromString(f.read())

    secret_fields: dict[str, list[str]] = {}
    msg_field_types: dict[str, dict[str, str]] = {}

    def collect(message_type) -> None:
        secrets: list[str] = []
        nested_msgs: dict[str, str] = {}
        for field in message_type.field:
            if field.type != FieldDescriptorProto.TYPE_MESSAGE:
                continue
            camel = to_camel(field.name)
            if field.type_name in _VALUE_WRAPPER_TYPES:
                # SecretString, CardNumberType, NetworkTokenType — all {value: string} wrappers
                secrets.append(camel)
            else:
                # Short name: ".types.Ach" → "Ach"
                nested_msgs[camel] = field.type_name.split(".")[-1]
        if secrets:
            secret_fields[message_type.name] = secrets
        if nested_msgs:
            msg_field_types[message_type.name] = nested_msgs
        for nested in message_type.nested_type:
            collect(nested)

    for file_desc in desc_set.file:
        for message_type in file_desc.message_type:
            collect(message_type)

    return secret_fields, msg_field_types


def gen_javascript_grpc_client() -> None:
    """Generate _generated_grpc_client.ts — JS gRPC sub-clients and GrpcClient from proto RPCs."""
    services, groups = _grpc_groups()
    all_types = sorted({t for flows in groups.values() for f in flows for t in (f["request"], f["response"])})
    secret_string_fields, msg_field_types = _collect_proto_field_maps(PROTO_DESCRIPTOR)
    render(
        "javascript/grpc_client.ts.j2",
        JS_GRPC_CLIENT_OUT,
        services=services,
        groups=groups,
        all_types=all_types,
        secret_string_fields=secret_string_fields,
        msg_field_types=msg_field_types,
    )


def gen_javascript_grpc_example_flows() -> None:
    """Generate examples/_generated_grpc_example_flows.js — generic grpc_* smoke-test functions."""
    services, groups = _grpc_groups()
    render(
        "javascript/grpc_example_flows.js.j2",
        JS_GRPC_EXAMPLE_FLOWS_OUT,
        services=services,
        groups=groups,
    )


def gen_rust_grpc_client() -> None:
    """Generate _generated_grpc_client.rs from all proto RPCs (not filtered by FFI impl)."""
    import subprocess

    services, groups = _grpc_groups()
    all_types = sorted({t for flows in groups.values() for f in flows for t in (f["request"], f["response"])})

    render(
        "rust/grpc_client.rs.j2",
        RUST_GRPC_CLIENT_OUT,
        services=services,
        groups=groups,
        all_types=all_types,
    )

    # Format with rustfmt so the file matches `cargo fmt` output exactly.
    result = subprocess.run(
        ["rustfmt", "--edition", "2021", str(RUST_GRPC_CLIENT_OUT)],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        print(f"  warning: rustfmt failed: {result.stderr.strip()}")


# ── Entry point ──────────────────────────────────────────────────────────────

def main() -> None:
    import argparse
    parser = argparse.ArgumentParser(
        description="SDK codegen — regenerate SDK clients from services.proto ∩ services/*.rs"
    )

    parser.add_argument(
        "--lang",
        choices=["python", "javascript", "kotlin", "rust", "grpc", "php", "all"],
        default="all",
        help="Which language/SDK to generate (default: all)"
    )
    args = parser.parse_args()

    ensure_descriptor_exists()

    print(f"Parsing: {SERVICES_PROTO.relative_to(REPO_ROOT)}")
    print(f"Parsing: {FFI_SERVICES_DIR.relative_to(REPO_ROOT)}/*.rs")
    print()

    flows, single_flows = discover_flows()

    print(f"Discovered {len(flows)} flows: {[f['name'] for f in flows]}")
    if single_flows:
        print(f"Discovered {len(single_flows)} single-step flows: {[f['name'] for f in single_flows]}")
    print()

    if args.lang in ("rust", "all"):
        print("Generating Rust FFI flow registrations...")
        gen_rust_handlers(flows)
        gen_rust_ffi_flows(flows)

    if args.lang in ("grpc", "all"):
        print("Generating Rust gRPC client...")
        gen_rust_grpc_client()
        print("Generating JavaScript gRPC client...")
        gen_javascript_grpc_client()
        print("Generating Python gRPC client...")
        gen_python_grpc_client()
        print("Generating Kotlin gRPC client...")
        gen_kotlin_grpc_client()

    if args.lang in ("python", "all"):
        print("Generating Python SDK...")
        gen_python(flows, single_flows)
        gen_python_stub(flows, single_flows)
        gen_python_clients(flows, single_flows)
        gen_python_grpc_client()

    if args.lang in ("javascript", "all"):
        print("Generating JavaScript SDK...")
        gen_javascript(flows, single_flows)

    if args.lang in ("kotlin", "all"):
        print("Generating Kotlin SDK...")
        gen_kotlin(flows, single_flows)

    if args.lang in ("php", "all"):
        print("Generating PHP SDK...")
        gen_php(flows, single_flows)

    print("\nDone.")


if __name__ == "__main__":
    main()
