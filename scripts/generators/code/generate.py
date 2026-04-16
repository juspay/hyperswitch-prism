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

PROTO_DIR = REPO_ROOT / "crates/types-traits/grpc-api-types/proto"
PROTO_FILES = [
    "services.proto",
    "payment.proto",
    "payouts.proto",
    "payment_methods.proto",
    "sdk_config.proto",
]

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


def build_descriptor_set():
    """
    Build a FileDescriptorSet from proto files using grpc_tools.protoc.
    Returns a FileDescriptorSet object compiled in-memory.
    """
    import tempfile
    import os
    from pathlib import Path
    from grpc_tools import protoc
    from google.protobuf.descriptor_pb2 import FileDescriptorSet

    with tempfile.NamedTemporaryFile(delete=False, suffix=".desc") as tmp:
        tmp_path = tmp.name

    # Find grpc_tools proto includes (e.g., google/protobuf/empty.proto)
    grpc_tools_proto_dir = Path(protoc.__file__).parent / "_proto"

    proto_paths = [str(PROTO_DIR / f) for f in PROTO_FILES]
    args = [
        "protoc",
        f"--proto_path={PROTO_DIR}",
        f"--proto_path={grpc_tools_proto_dir}",
        f"--descriptor_set_out={tmp_path}",
        "--include_source_info",
    ] + proto_paths

    ret = protoc.main(args)
    if ret != 0:
        os.unlink(tmp_path)
        raise RuntimeError(f"protoc failed with exit code {ret}")

    try:
        with open(tmp_path, "rb") as f:
            desc_set = FileDescriptorSet.FromString(f.read())
    finally:
        os.unlink(tmp_path)

    return desc_set


# ── Source parsing ───────────────────────────────────────────────────────────

def to_snake_case(name: str) -> str:
    """'CreateAccessToken' -> 'create_access_token'"""
    s = re.sub("(.)([A-Z][a-z]+)", r"\1_\2", name)
    return re.sub("([a-z0-9])([A-Z])", r"\1_\2", s).lower()


def _service_flow_prefix(service_name: str) -> Optional[str]:
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


def parse_proto_rpcs(desc_set) -> dict[str, dict]:
    """
    Parse RPC definitions from a protobuf FileDescriptorSet.

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


def discover_flows(desc_set=None) -> tuple[list[dict], list[dict]]:
    """
    Cross-reference proto RPCs with implemented service transformers.
    Returns (standard_flows, single_flows) — both sorted by name.
    Standard flows use req+HTTP+res; single flows call the transformer directly.
    
    If desc_set is not provided, it will be built automatically from proto files.
    """
    if desc_set is None:
        desc_set = build_descriptor_set()
    proto_rpcs = parse_proto_rpcs(desc_set)
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
    # Suppressed: shows prefixed RPC names that are alternate lookup keys
    # if unimplemented:
    #     print(f"  Proto RPCs not yet implemented (skipped): {unimplemented}")

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


# Special-case flow names where the bare RPC name is ambiguous or non-descriptive.
# Used for SDK method names and gRPC examples.
_FLOW_NAME_OVERRIDES: dict[tuple[str, str], str] = {
    ("CustomerService", "Create"): "create_customer",
    ("RecurringPaymentService", "Charge"): "recurring_charge",
    ("RecurringPaymentService", "Revoke"): "recurring_revoke",
    ("RefundService", "Get"): "refund_get",
    ("PayoutService", "Get"): "payout_get",
    ("PayoutService", "Create"): "payout_create",
    ("PayoutService", "Void"): "payout_void",
}


def get_flow_method_name(service: str, rpc_name: str) -> str:
    """Get the SDK method name for a flow, applying overrides if needed."""
    return _FLOW_NAME_OVERRIDES.get((service, rpc_name), to_snake_case(rpc_name))


# Register helpers as Jinja2 globals so templates can call them directly.
env.globals["service_to_client_name"]    = service_to_client_name
env.globals["service_to_tonic_mod"]      = service_to_tonic_mod
env.globals["service_to_grpc_struct"]    = service_to_grpc_struct
env.globals["service_to_grpc_field"]     = service_to_grpc_field
env.globals["service_to_grpc_js_field"]  = service_to_grpc_js_field
env.globals["grpc_method_path"]          = grpc_method_path
env.globals["get_flow_method_name"]      = get_flow_method_name
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


RUST_CONNECTOR_CLIENT_OUT = SDK_ROOT / "rust/src/_generated_connector_client.rs"


def gen_rust_connector_client(flows: list[dict], single_flows: list[dict] = []) -> None:
    """Generate connector_client.rs — auto-generated ConnectorClient with all flow methods."""
    import subprocess

    groups = group_by_service(flows)
    single_groups = group_by_service(single_flows)
    all_services = sorted(set(groups) | set(single_groups))

    render(
        "rust/connector_client.rs.j2",
        RUST_CONNECTOR_CLIENT_OUT,
        flows=flows,
        single_flows=single_flows,
        all_services=all_services,
        groups=groups,
        single_groups=single_groups,
    )

    # Format with rustfmt so the file matches `cargo fmt` output exactly.
    result = subprocess.run(
        ["rustfmt", "--edition", "2021", str(RUST_CONNECTOR_CLIENT_OUT)],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        print(f"  warning: rustfmt failed: {result.stderr.strip()}")


def _grpc_groups(desc_set=None) -> tuple[list[str], dict[str, list[dict]]]:
    """Shared helper: all proto RPCs grouped by service (used by JS + Rust gRPC generators).
    
    Returns all RPCs grouped by service. For services where the simple RPC name
    is already taken by another service (e.g., PaymentService.Authorize), uses
    the prefixed name (e.g., tokenized_authorize, proxied_authorize).
    
    If desc_set is not provided, it will be built automatically from proto files.
    """
    if desc_set is None:
        desc_set = build_descriptor_set()
    all_rpcs = parse_proto_rpcs(desc_set)
    
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


def gen_python_grpc_client(desc_set=None) -> None:
    """Generate _generated_grpc_client.py — Python gRPC sub-clients and GrpcClient from proto RPCs."""
    services, groups = _grpc_groups(desc_set)
    render(
        "python/grpc_client.py.j2",
        PY_GRPC_CLIENT_OUT,
        services=services,
        groups=groups,
    )


def gen_kotlin_grpc_client(desc_set=None) -> None:
    """Generate GrpcClient.kt — Kotlin gRPC sub-clients and GrpcClient from proto RPCs."""
    services, groups = _grpc_groups(desc_set)
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
    desc_set,
) -> tuple[dict[str, list[str]], dict[str, dict[str, str]]]:
    """
    Parse proto descriptor in one pass and return:
      secret_fields:  {MessageName: [camelCaseFieldName]}  — fields typed SecretString
      msg_field_types:{MessageName: {camelCaseFieldName: NestedTypeName}} — other message fields
    Both maps are keyed by short message name (e.g. "Ach", not ".types.Ach").
    """
    from google.protobuf.descriptor_pb2 import FieldDescriptorProto

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


def gen_javascript_grpc_client(desc_set=None) -> None:
    """Generate _generated_grpc_client.ts — JS gRPC sub-clients and GrpcClient from proto RPCs."""
    services, groups = _grpc_groups(desc_set)
    all_types = sorted({t for flows in groups.values() for f in flows for t in (f["request"], f["response"])})
    secret_string_fields, msg_field_types = _collect_proto_field_maps(desc_set if desc_set else build_descriptor_set())
    render(
        "javascript/grpc_client.ts.j2",
        JS_GRPC_CLIENT_OUT,
        services=services,
        groups=groups,
        all_types=all_types,
        secret_string_fields=secret_string_fields,
        msg_field_types=msg_field_types,
    )


def gen_javascript_grpc_example_flows(desc_set=None) -> None:
    """Generate examples/_generated_grpc_example_flows.js — generic grpc_* smoke-test functions."""
    services, groups = _grpc_groups(desc_set)
    render(
        "javascript/grpc_example_flows.js.j2",
        JS_GRPC_EXAMPLE_FLOWS_OUT,
        services=services,
        groups=groups,
    )


def gen_rust_grpc_client(desc_set=None) -> None:
    """Generate _generated_grpc_client.rs from all proto RPCs (not filtered by FFI impl)."""
    import subprocess

    services, groups = _grpc_groups(desc_set)
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


# ── Flow manifest generator ──────────────────────────────────────────────────

FLOW_MANIFEST_OUT = SDK_ROOT / "generated" / "flows.json"


def emit_flow_manifest(flows: list[dict], single_flows: list[dict]) -> None:
    """
    Writes sdk/generated/flows.json with the canonical list of implemented flows.
    Called after all flow discovery is complete.
    
    A flow appears in flows.json ONLY if generate.py finds both:
      (a) a matching RPC in services.proto, AND
      (b) a *_req_transformer implementation in crates/ffi/ffi/src/services/*.rs
    
    Also includes flow_to_example_fn mapping for smoke tests to know which
    example function to call for each flow (since examples use scenario-based
    naming like 'checkout_card' instead of flow-based naming like 'authorize').
    """
    import json
    
    # Combine standard flows and single-step flows
    all_flow_names = sorted(set(f["name"] for f in flows) | set(f["name"] for f in single_flows))
    
    # Mapping from flow name to example function name
    # Examples use scenario-based naming (e.g., checkout_card) not flow-based (e.g., authorize)
    flow_to_example_fn = {
        "accept": None,  # Not implemented in examples
        "authenticate": None,
        "authorize": "checkout_card",  # Primary card-based authorize example
        "capture": "checkout_card",    # Part of checkout_card scenario
        "charge": None,
        "create": None,
        "create_client_authentication_token": None,
        "create_order": None,
        "create_server_authentication_token": None,
        "create_server_session_authentication_token": None,
        "defend": None,
        "get": "get_payment",
        "handle_event": None,
        "incremental_authorization": None,
        "payout_create": None,
        "payout_create_link": None,
        "payout_create_recipient": None,
        "payout_enroll_disburse_account": None,
        "payout_get": None,
        "payout_stage": None,
        "payout_transfer": None,
        "payout_void": None,
        "post_authenticate": None,
        "pre_authenticate": None,
        "proxy_authorize": None,
        "proxy_setup_recurring": None,
        "recurring_revoke": None,
        "refund": "refund",
        "refund_get": None,
        "reverse": None,
        "setup_recurring": None,
        "submit_evidence": None,
        "token_authorize": None,
        "token_setup_recurring": None,
        "tokenize": None,
        "verify_redirect_response": None,
        "void": "void_payment",
    }
    
    manifest = {
        "schema_version": 2,
        "generated_from": "services.proto",
        "flows": all_flow_names,
        "flow_to_example_fn": flow_to_example_fn,
        "note": "flow_to_example_fn maps flow names to example function names. Null means no example implementation.",
    }
    
    FLOW_MANIFEST_OUT.parent.mkdir(parents=True, exist_ok=True)
    FLOW_MANIFEST_OUT.write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"  Wrote {FLOW_MANIFEST_OUT.relative_to(REPO_ROOT)} ({len(all_flow_names)} flows)")


# ── Entry point ──────────────────────────────────────────────────────────────

def main() -> None:
    import argparse
    parser = argparse.ArgumentParser(
        description="SDK codegen — regenerate SDK clients from services.proto ∩ services/*.rs"
    )

    parser.add_argument(
        "--lang",
        choices=["python", "javascript", "kotlin", "rust", "grpc", "all"],
        default="all",
        help="Which language/SDK to generate (default: all)"
    )
    args = parser.parse_args()

    print(f"Parsing: {SERVICES_PROTO.relative_to(REPO_ROOT)}")
    print(f"Parsing: {FFI_SERVICES_DIR.relative_to(REPO_ROOT)}/*.rs")
    print()

    desc_set = build_descriptor_set()
    flows, single_flows = discover_flows(desc_set)
    
    # Generate flow manifest for smoke test coverage
    print("Generating flow manifest...")
    emit_flow_manifest(flows, single_flows)
    print()

    print(f"Discovered {len(flows)} flows: {[f['name'] for f in flows]}")
    if single_flows:
        print(f"Discovered {len(single_flows)} single-step flows: {[f['name'] for f in single_flows]}")
    print()

    if args.lang in ("rust", "all"):
        print("Generating Rust FFI flow registrations...")
        gen_rust_handlers(flows)
        gen_rust_ffi_flows(flows)
        print("Generating Rust connector client...")
        gen_rust_connector_client(flows, single_flows)

    if args.lang in ("grpc", "all"):
        print("Generating Rust gRPC client...")
        gen_rust_grpc_client(desc_set)
        print("Generating JavaScript gRPC client...")
        gen_javascript_grpc_client(desc_set)
        print("Generating Python gRPC client...")
        gen_python_grpc_client(desc_set)
        print("Generating Kotlin gRPC client...")
        gen_kotlin_grpc_client(desc_set)

    if args.lang in ("python", "all"):
        print("Generating Python SDK...")
        gen_python(flows, single_flows)
        gen_python_stub(flows, single_flows)
        gen_python_clients(flows, single_flows)
        gen_python_grpc_client(desc_set)

    if args.lang in ("javascript", "all"):
        print("Generating JavaScript SDK...")
        gen_javascript(flows, single_flows)

    if args.lang in ("kotlin", "all"):
        print("Generating Kotlin SDK...")
        gen_kotlin(flows, single_flows)

    print("\nDone.")


if __name__ == "__main__":
    main()
