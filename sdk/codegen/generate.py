
#!/usr/bin/env python3
"""
SDK codegen — auto-discovers payment flows and generates type-safe client methods.

Cross-references:
  1. services.proto (via protoc descriptor) → RPC definitions with types and docs
  2. services/payments.rs → which flows have req_transformer implementations

Generates flow methods (authorize, capture, refund, etc.) for each SDK,
and the Rust FFI flow registration files.

Usage:
    # Generate all SDKs + Rust FFI registrations
    python3 sdk/codegen/generate.py
    make generate

    # Generate specific language only
    python3 sdk/codegen/generate.py --lang python
    python3 sdk/codegen/generate.py --lang javascript
    python3 sdk/codegen/generate.py --lang kotlin
    python3 sdk/codegen/generate.py --lang rust

    # Via individual SDK Makefiles
    make -C sdk/python generate
    make -C sdk/javascript generate
    make -C sdk/java generate
"""

import re
import sys
from pathlib import Path

from jinja2 import Environment, FileSystemLoader

REPO_ROOT = Path(__file__).parent.parent.parent
SDK_ROOT = REPO_ROOT / "sdk"
SERVICES_PROTO = REPO_ROOT / "backend/grpc-api-types/proto/services.proto"
FFI_SERVICES = REPO_ROOT / "backend/ffi/src/services/payments.rs"
PROTO_DESCRIPTOR = REPO_ROOT / "sdk/codegen/services.desc"

RUST_HANDLERS_OUT = REPO_ROOT / "backend/ffi/src/handlers/_generated_flow_registrations.rs"
RUST_FFI_FLOWS_OUT = REPO_ROOT / "backend/ffi/src/bindings/_generated_ffi_flows.rs"

TEMPLATES_DIR = Path(__file__).parent / "templates"

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
    First-occurrence wins on name collision.
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
            for method_idx, method in enumerate(service.method):
                rpc_name = method.name
                snake = to_snake_case(rpc_name)

                if snake not in rpcs:
                    # Extract type names (remove package prefix)
                    req_type = method.input_type.split('.')[-1]
                    res_type = method.output_type.split('.')[-1]

                    # Get doc comment if available
                    # Path for method: [6 (service), svc_idx, 2 (method), method_idx]
                    path = (6, svc_idx, 2, method_idx)
                    comment = source_info.get(path, f"{service.name}.{rpc_name}")
                    # Normalize whitespace to single-line
                    comment = ' '.join(comment.split())

                    rpcs[snake] = {
                        "request": req_type,
                        "response": res_type,
                        "service": service.name,
                        "rpc": rpc_name,
                        "description": comment,
                    }

    return rpcs


def parse_service_flows(service_file: Path) -> set[str]:
    """
    Scan services/payments.rs for every req_transformer! invocation.
    Captures the flow name from `fn_name: {flow}_req_transformer`.
    """
    text = service_file.read_text()
    return {
        m.group(1)
        for m in re.finditer(
            r"fn_name:\s*(\w+)_req_transformer\b", text
        )
    }


def parse_single_flows(service_file: Path) -> set[str]:
    """
    Scan services/payments.rs for hand-written single-step transformers.
    These are `pub fn {flow}_transformer` functions that are NOT req/res macros —
    they take the request directly and return the response without an HTTP round-trip
    (e.g. webhook processing via `handle_transformer`).
    """
    text = service_file.read_text()
    return {
        m.group(1)
        for m in re.finditer(r"^pub fn (\w+)_transformer\b", text, re.MULTILINE)
    }


def discover_flows() -> tuple[list[dict], list[dict]]:
    """
    Cross-reference proto RPCs with implemented service transformers.
    Returns (standard_flows, single_flows) — both sorted by name.
    Standard flows use req+HTTP+res; single flows call the transformer directly.
    """
    proto_rpcs = parse_proto_rpcs(PROTO_DESCRIPTOR)
    service_flows = parse_service_flows(FFI_SERVICES)
    single_flow_names = parse_single_flows(FFI_SERVICES)

    flows = []
    for flow in sorted(service_flows):
        if flow not in proto_rpcs:
            print(
                f"  WARNING: '{flow}_req_transformer' exists in services/payments.rs but has no matching RPC in services.proto",
                file=sys.stderr,
            )
            continue
        flows.append({"name": flow, **proto_rpcs[flow]})

    single_flows = []
    for flow in sorted(single_flow_names):
        if flow not in proto_rpcs:
            print(
                f"  WARNING: '{flow}_transformer' exists in services/payments.rs but has no matching RPC in services.proto",
                file=sys.stderr,
            )
            continue
        single_flows.append({"name": flow, **proto_rpcs[flow]})

    implemented = service_flows | single_flow_names
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


# Register helpers as Jinja2 globals so templates can call them directly.
env.globals["service_to_client_name"] = service_to_client_name
env.globals["to_camel"] = to_camel


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


def gen_kotlin(flows: list[dict], single_flows: list[dict] = []) -> None:
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
    all_types = sorted({t for f in flows for t in (f["request"], f["response"])})

    render(
        "rust/handlers.rs.j2",
        RUST_HANDLERS_OUT,
        flows=flows,
        all_types=all_types,
    )


def gen_rust_ffi_flows(flows: list[dict]) -> None:
    """Generate _generated_ffi_flows.rs — included by bindings/uniffi.rs."""
    req_types = sorted({f["request"] for f in flows})

    render(
        "rust/ffi_flows.rs.j2",
        RUST_FFI_FLOWS_OUT,
        flows=flows,
        req_types=req_types,
    )


# ── Entry point ──────────────────────────────────────────────────────────────

def main() -> None:
    import argparse
    parser = argparse.ArgumentParser(
        description="SDK codegen — regenerate SDK clients from services.proto ∩ services/payments.rs"
    )

    parser.add_argument(
        "--lang",
        choices=["python", "javascript", "kotlin", "rust", "all"],
        default="all",
        help="Which language/SDK to generate (default: all)"
    )
    args = parser.parse_args()

    ensure_descriptor_exists()

    print(f"Parsing: {SERVICES_PROTO.relative_to(REPO_ROOT)}")
    print(f"Parsing: {FFI_SERVICES.relative_to(REPO_ROOT)}")
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

    if args.lang in ("python", "all"):
        print("Generating Python SDK...")
        gen_python(flows, single_flows)
        gen_python_stub(flows, single_flows)
        gen_python_clients(flows, single_flows)

    if args.lang in ("javascript", "all"):
        print("Generating JavaScript SDK...")
        gen_javascript(flows, single_flows)

    if args.lang in ("kotlin", "all"):
        print("Generating Kotlin SDK...")
        gen_kotlin(flows, single_flows)

    print("\nDone.")


if __name__ == "__main__":
    main()
