#!/usr/bin/env python3
"""
Generate flow harnesses for mock mode testing.

Leverages snippet_examples/generate.py for type-safe request builders.
Harness files contain:
- SUPPORTED_FLOWS list for each connector
- Process_* functions that call the SDK client with minimal requests

Usage:
    python3 scripts/generators/code/generate_harnesses.py
    python3 scripts/generators/code/generate_harnesses.py --connector stripe
"""

import json
import sys
import re
from pathlib import Path
from typing import Optional

REPO_ROOT = Path(__file__).parent.parent.parent.parent
sys.path.insert(0, str(REPO_ROOT / "scripts" / "generators"))

# Import from docs generator which has full flow metadata extraction
sys.path.insert(0, str(REPO_ROOT / "scripts" / "generators" / "docs"))
from generate import (
    _build_proto_metadata,
)

from snippet_examples.generate import (
    load_proto_type_map,
    render_consolidated_python,
    render_consolidated_javascript,
    render_consolidated_kotlin,
    render_consolidated_rust,
    _to_camel,
    _SchemaDB,
    _annotate_inline_lines,
    _json_scalar,
)

FIELD_PROBE_DIR = REPO_ROOT / "data" / "field_probe"
FLOWS_JSON = REPO_ROOT / "sdk" / "generated" / "flows.json"
PROTO_DIR = REPO_ROOT / "crates" / "types-traits" / "grpc-api-types" / "proto"
SDK_ROOT = REPO_ROOT / "sdk"

PYTHON_HARNESS_DIR = SDK_ROOT / "python" / "smoke-test" / "generated"
JS_HARNESS_DIR = SDK_ROOT / "javascript" / "smoke-test" / "generated"
KOTLIN_HARNESS_DIR = SDK_ROOT / "java" / "smoke-test" / "generated"
RUST_HARNESS_DIR = SDK_ROOT / "rust" / "smoke-test" / "generated"

PROTO_METADATA_CACHE = REPO_ROOT / "sdk" / "generated" / "proto_metadata.json"


def _compute_proto_hash() -> str:
    """Compute a hash of all proto files to detect changes."""
    import hashlib
    hasher = hashlib.sha256()
    proto_files = sorted(PROTO_DIR.glob("*.proto"))
    for proto_file in proto_files:
        hasher.update(proto_file.read_bytes())
    return hasher.hexdigest()[:16]


def get_proto_metadata():
    """Get proto metadata from cache or build it if proto files changed."""
    current_hash = _compute_proto_hash()
    
    # Try to load from cache
    if PROTO_METADATA_CACHE.exists():
        try:
            cache_data = json.loads(PROTO_METADATA_CACHE.read_text())
            if cache_data.get("proto_hash") == current_hash:
                print(f"  Using cached proto metadata ({PROTO_METADATA_CACHE.name})")
                return cache_data["flow_metadata"], cache_data["message_schemas"]
        except (json.JSONDecodeError, KeyError):
            pass  # Cache corrupted, rebuild
    
    # Build proto metadata
    print(f"  Building proto metadata from {PROTO_DIR}...")
    flow_metadata_list, message_schemas = _build_proto_metadata(PROTO_DIR)
    
    # Save to cache
    cache_data = {
        "proto_hash": current_hash,
        "flow_metadata": flow_metadata_list,
        "message_schemas": message_schemas,
    }
    PROTO_METADATA_CACHE.write_text(json.dumps(cache_data, indent=2))
    print(f"  Cached proto metadata to {PROTO_METADATA_CACHE.name}")
    
    return flow_metadata_list, message_schemas


def load_flows_manifest() -> set:
    """Load the set of valid flows from flows.json manifest."""
    if not FLOWS_JSON.exists():
        return set()
    data = json.loads(FLOWS_JSON.read_text())
    return set(data.get("flows", []))


def load_field_probe(connector_name: str) -> Optional[dict]:
    """Load field probe data for a connector."""
    probe_file = FIELD_PROBE_DIR / f"{connector_name}.json"
    if not probe_file.exists():
        return None
    return json.loads(probe_file.read_text())


def get_supported_flows(probe_data: dict, manifest_flows: set) -> list[str]:
    """Extract supported flows from field probe data, filtered by manifest.
    
    Excludes webhook-only flows (e.g., handle_event) which receive incoming webhooks
    rather than making outgoing HTTP requests. These flows don't have proto_requests
    since they don't use the standard req_transformer pattern.
    """
    # Webhook-only flows that should not be included in harness tests
    WEBHOOK_ONLY_FLOWS = {"handle_event", "verify_redirect_response"}
    
    flows = probe_data.get("flows", {})
    supported = []

    for flow_name, flow_data in flows.items():
        if flow_name not in manifest_flows:
            continue
        
        # Skip webhook-only flows - they can't be tested via harnesses
        if flow_name in WEBHOOK_ONLY_FLOWS:
            continue

        if isinstance(flow_data, dict):
            default = flow_data.get("default", {})
            if default.get("status") == "supported":
                supported.append(flow_name)
            else:
                for variant, variant_data in flow_data.items():
                    if variant != "default" and isinstance(variant_data, dict):
                        if variant_data.get("status") == "supported":
                            supported.append(flow_name)
                            break

    return sorted(supported)


def build_flow_data(connector_name: str, supported_flows: list[str], probe_data: dict):
    """Build flow items and metadata for snippet_examples generators."""
    # Get cached proto metadata (built once)
    flow_metadata_list, message_schemas = get_proto_metadata()
    
    flow_metadata_dict = {m["flow_key"]: m for m in flow_metadata_list}
    
    flow_items = []
    for flow in supported_flows:
        flow_probe_data = probe_data.get("flows", {}).get(flow, {})
        
        # Check if this flow is supported (has status == "supported")
        is_supported = False
        proto_req = {}
        
        if isinstance(flow_probe_data, dict):
            default = flow_probe_data.get("default", {})
            if default.get("status") == "supported":
                is_supported = True
                proto_req = default.get("proto_request") or {}
            else:
                for variant, variant_data in flow_probe_data.items():
                    if variant != "default" and isinstance(variant_data, dict):
                        if variant_data.get("status") == "supported":
                            is_supported = True
                            proto_req = variant_data.get("proto_request") or {}
                            break
        
        # Include the flow if it's supported (even with empty proto_request).
        # Empty proto_requests are fine - the builder will use default values.
        if is_supported:
            flow_items.append((flow, proto_req, None))
    
    return flow_items, flow_metadata_dict, message_schemas


def unwrap_oneof_groups(proto_req: dict, message_schemas: dict, msg_type: str) -> dict:
    """Unwrap oneof group fields in proto_request data.
    
    Field probe data uses oneof group names (e.g., domain_context) but proto expects
    the variant field name (e.g., payment). This function unwraps those groups.
    """
    if not isinstance(proto_req, dict):
        return proto_req
    
    result = {}
    schema = message_schemas.get(msg_type, {})
    field_types = schema.get("field_types", {})
    
    for key, value in proto_req.items():
        if isinstance(value, dict):
            # Check if this is a oneof group (field_type is empty but has nested fields)
            child_type = field_types.get(key, "")
            if not child_type and len(value) == 1:
                # This might be a oneof group - unwrap it
                variant_key = list(value.keys())[0]
                result[variant_key] = unwrap_oneof_groups(value[variant_key], message_schemas, "")
            else:
                # Normal nested message
                result[key] = unwrap_oneof_groups(value, message_schemas, child_type)
        elif isinstance(value, list):
            result[key] = [unwrap_oneof_groups(item, message_schemas, "") for item in value]
        else:
            result[key] = value
    
    return result


def generate_python_harness(connector_name: str, supported_flows: list[str], flow_items: list, flow_metadata: dict, message_schemas: dict) -> str:
    """Generate Python harness using snippet_examples consolidated generator."""
    # Load proto types first so wrapper type detection works
    load_proto_type_map(PROTO_DIR)
    
    # Unwrap oneof groups in flow items
    unwrapped_flow_items = []
    for flow_key, proto_req, pm_label in flow_items:
        grpc_req = flow_metadata.get(flow_key, {}).get("grpc_request", "")
        unwrapped_req = unwrap_oneof_groups(proto_req, message_schemas, grpc_req)
        unwrapped_flow_items.append((flow_key, unwrapped_req, pm_label))
    
    # Use consolidated generator which handles types correctly
    consolidated = render_consolidated_python(
        connector_name=connector_name,
        flow_items=unwrapped_flow_items,
        flow_metadata=flow_metadata,
        message_schemas=message_schemas,
        scenarios_with_payloads=[],  # No scenarios for harness
    )
    
    # Post-process: rename flow functions to add process_ prefix
    # The consolidated generator produces functions like `async def authorize(...)`
    # but we need `async def process_authorize(...)` for smoke test compatibility
    lines = consolidated.split('\n')
    processed_lines = []
    
    for line in lines:
        # Rename async def flow_key( to async def process_flow_key(
        for flow in supported_flows:
            if line.startswith(f"async def {flow}("):
                line = line.replace(f"async def {flow}(", f"async def process_{flow}(")
                break
        processed_lines.append(line)
    
    # Find import section end
    insert_idx = 0
    for i, line in enumerate(processed_lines):
        if line.startswith('from ') or line.startswith('import '):
            insert_idx = i + 1
    
    # Insert SUPPORTED_FLOWS
    flow_list = json.dumps(supported_flows)
    processed_lines.insert(insert_idx, "")
    processed_lines.insert(insert_idx + 1, f"SUPPORTED_FLOWS = {flow_list}")
    processed_lines.insert(insert_idx + 2, "")
    
    return '\n'.join(processed_lines)


def generate_javascript_harness(connector_name: str, supported_flows: list[str], flow_items: list, flow_metadata: dict, message_schemas: dict) -> str:
    """Generate JavaScript harness."""
    # Load proto types first so wrapper type detection works
    load_proto_type_map(PROTO_DIR)
    
    # Unwrap oneof groups in flow items
    unwrapped_flow_items = []
    for flow_key, proto_req, pm_label in flow_items:
        grpc_req = flow_metadata.get(flow_key, {}).get("grpc_request", "")
        unwrapped_req = unwrap_oneof_groups(proto_req, message_schemas, grpc_req)
        unwrapped_flow_items.append((flow_key, unwrapped_req, pm_label))
    
    consolidated = render_consolidated_javascript(
        connector_name=connector_name,
        flow_items=unwrapped_flow_items,
        flow_metadata=flow_metadata,
        message_schemas=message_schemas,
        scenarios_with_payloads=[],
    )
    
    # Post-process: rename flow functions to add process prefix
    lines = consolidated.split('\n')
    processed_lines = []
    
    # Map from flow key to original function name generated by snippet_examples
    from snippet_examples.generate import _to_camel, JS_RESERVED
    flow_to_func_name = {}
    flow_to_proc_name = {}
    for flow in supported_flows:
        camel = _to_camel(flow)
        # Reserved words get "Payment" suffix
        if flow in JS_RESERVED:
            func_name = f"{flow}Payment"
        else:
            func_name = camel
        flow_to_func_name[flow] = func_name
        pascal = ''.join(word.capitalize() for word in flow.split('_'))
        flow_to_proc_name[flow] = f"process{pascal}"
    
    # Build ordered replacement list (longest first to avoid substring issues)
    replacements = sorted(
        [(flow_to_func_name[f], flow_to_proc_name[f]) for f in supported_flows],
        key=lambda x: len(x[0]),
        reverse=True
    )
    
    for line in lines:
        # Rename async function declarations with word boundary
        for orig_name, proc_name in replacements:
            # Skip lines that already have the process prefix
            if f"process{proc_name.replace('process', '')}" in line and "async function" in line:
                continue
            
            # Rename async function declarations
            if re.search(rf'\basync function {re.escape(orig_name)}\(', line):
                line = re.sub(rf'\b(async function ){re.escape(orig_name)}(\()', rf'\g<1>{proc_name}\g<2>', line)
                break
            # Rename export async function declarations
            elif re.search(rf'\bexport async function {re.escape(orig_name)}\(', line):
                line = re.sub(rf'\b(export async function ){re.escape(orig_name)}(\()', rf'\g<1>{proc_name}\g<2>', line)
                break
        
        # Handle export { ... } list or lines containing export names - replace all names in one pass
        # This handles both "export {" lines and continuation lines with the names
        for orig_name, proc_name in replacements:
            # Use word boundaries to avoid partial matches
            pattern = rf'\b{re.escape(orig_name)}\b'
            if re.search(pattern, line):
                line = re.sub(pattern, proc_name, line)
        
        processed_lines.append(line)
    
    # Insert SUPPORTED_FLOWS after imports (only top-level imports, not const declarations in code)
    insert_idx = 0
    for i, line in enumerate(processed_lines):
        # Only count import statements, not const declarations that are part of code
        if line.startswith('import '):
            insert_idx = i + 1
    
    flow_list = json.dumps(supported_flows)
    processed_lines.insert(insert_idx, "")
    processed_lines.insert(insert_idx + 1, f"export const SUPPORTED_FLOWS = {flow_list};")
    processed_lines.insert(insert_idx + 2, "")
    
    return '\n'.join(processed_lines)


def generate_kotlin_harness(connector_name: str, supported_flows: list[str], flow_items: list, flow_metadata: dict, message_schemas: dict) -> str:
    """Generate Kotlin harness."""
    # Load proto types first so wrapper type detection works
    load_proto_type_map(PROTO_DIR)
    
    consolidated = render_consolidated_kotlin(
        connector_name=connector_name,
        flow_items=flow_items,
        flow_metadata=flow_metadata,
        message_schemas=message_schemas,
        scenarios_with_payloads=[],
    )
    
    # Post-process: rename flow functions to add process prefix
    # The consolidated generator produces functions like `fun authorize(...)`
    # but we need `fun processAuthorize(...)` for smoke test compatibility
    lines = consolidated.split('\n')
    processed_lines = []
    
    # Map from flow key to function name generated by snippet_examples
    # Kotlin uses camelCase (same as JS _to_camel) but doesn't have reserved word handling
    from snippet_examples.generate import _to_camel
    flow_to_func_name = {}
    flow_to_proc_name = {}
    for flow in supported_flows:
        # Kotlin consolidated generator uses _to_camel directly without reserved word handling
        func_name = _to_camel(flow)
        pascal = ''.join(word.capitalize() for word in flow.split('_'))
        flow_to_func_name[flow] = func_name
        flow_to_proc_name[flow] = f"process{pascal}"
    
    # Build ordered replacement list (longest first to avoid substring issues)
    replacements = sorted(
        [(flow_to_func_name[f], flow_to_proc_name[f]) for f in supported_flows],
        key=lambda x: len(x[0]),
        reverse=True
    )
    
    # Single pass: rename functions, track function bodies, and replace _harnessConfig
    in_func_body = False
    func_brace_count = 0
    just_entered_function = False
    
    for line in lines:
        # Change package from examples.{connector} to harness.{connector} to avoid conflicts
        if line.startswith('package examples.'):
            line = line.replace('package examples.', 'package harness.')
        
        # Rename function declarations and add config parameter
        # This transforms: fun authorize(txnId: String) -> fun processAuthorize(txnId: String, config: ConnectorConfig = _harnessConfig)
        for orig_name, proc_name in replacements:
            pattern = rf'\bfun {re.escape(orig_name)}\(txnId: String\)'
            if re.search(pattern, line):
                line = re.sub(rf'\b(fun ){re.escape(orig_name)}(\(txnId: String\))', 
                            rf'\g<1>{proc_name}(txnId: String, config: ConnectorConfig = _harnessConfig)', line)
                in_func_body = True
                just_entered_function = True
                func_brace_count = line.count('{') - line.count('}')
                break
            # Also handle in when branches like "authorize" -> authorize(
            if f'"{orig_name}"' in line and f'-> {orig_name}(' in line:
                line = re.sub(rf'"{re.escape(orig_name)}" -> {re.escape(orig_name)}\(', 
                            rf'"{orig_name}" -> {proc_name}(', line)
                break
        
        # Rename _defaultConfig to _harnessConfig BEFORE checking function body
        # (we want the global variable renamed everywhere)
        line = line.replace("_defaultConfig", "_harnessConfig")
        
        # Check for function start/end to track if we're in a function body
        # (for functions that were already renamed in a previous iteration)
        if not in_func_body and not just_entered_function and re.search(r'\bfun process\w+\(txnId: String,', line):
            in_func_body = True
            func_brace_count = line.count('{') - line.count('}')
        elif in_func_body and not just_entered_function:
            func_brace_count += line.count('{') - line.count('}')
            if func_brace_count == 0:
                in_func_body = False
        
        # Replace _harnessConfig with config inside function bodies (but not in function signature line)
        if in_func_body and not re.search(r'fun process\w+\(.*config: ConnectorConfig', line):
            line = line.replace('_harnessConfig', 'config')
        
        # Reset the flag after processing the line where we entered the function
        just_entered_function = False
        
        processed_lines.append(line)
    
    # Insert SUPPORTED_FLOWS after package/import
    insert_idx = 0
    for i, line in enumerate(processed_lines):
        if line.startswith('import '):
            insert_idx = i + 1
    
    flow_list = ', '.join(f'"{f}"' for f in supported_flows)
    processed_lines.insert(insert_idx, "")
    processed_lines.insert(insert_idx + 1, f"val SUPPORTED_FLOWS = listOf({flow_list})")
    processed_lines.insert(insert_idx + 2, "")
    
    return '\n'.join(processed_lines)


def generate_rust_harness(connector_name: str, supported_flows: list[str], flow_items: list, flow_metadata: dict, message_schemas: dict) -> str:
    """Generate Rust harness using snippet_examples consolidated generator."""
    # Load proto types for proper type handling
    load_proto_type_map(PROTO_DIR)
    
    consolidated = render_consolidated_rust(
        connector_name=connector_name,
        flow_items=flow_items,
        flow_metadata=flow_metadata,
        message_schemas=message_schemas,
        scenarios_with_payloads=[],
    )
    
    # Post-process: rename flow functions to add process_ prefix
    # The consolidated generator produces functions like `pub async fn authorize(...)`
    # but we need `pub async fn process_authorize(...)` for smoke test compatibility
    lines = consolidated.split('\n')
    processed_lines = []
    
    for line in lines:
        # Rename pub async fn flow_key( to pub async fn process_flow_key(
        for flow in supported_flows:
            if f"pub async fn {flow}(" in line:
                line = line.replace(f"pub async fn {flow}(", f"pub async fn process_{flow}(")
                break
            # Also rename the standalone flow functions
            if line.startswith(f"async fn {flow}("):
                line = line.replace(f"async fn {flow}(", f"async fn process_{flow}(")
                break
            # Also rename calls in the main function match arms
            if f'"{flow}" => {flow}(' in line:
                line = line.replace(f'"{flow}" => {flow}(', f'"{flow}" => process_{flow}(')
                break
        processed_lines.append(line)
    
    # Insert SUPPORTED_FLOWS after use statements for build.rs 3-check validation
    insert_idx = 0
    for i, line in enumerate(processed_lines):
        if line.startswith('use '):
            insert_idx = i + 1
    
    flow_list = ', '.join(f'"{f}"' for f in supported_flows)
    processed_lines.insert(insert_idx, "")
    processed_lines.insert(insert_idx + 1, "// Used by build.rs for 3-check validation, not used at runtime")
    processed_lines.insert(insert_idx + 2, "#[allow(dead_code)]")
    processed_lines.insert(insert_idx + 3, f"pub const SUPPORTED_FLOWS: &[&str] = &[{flow_list}];")
    processed_lines.insert(insert_idx + 4, "")
    
    return '\n'.join(processed_lines)


def generate_harnesses_for_connector(connector_name: str) -> None:
    """Generate harness files for a single connector."""
    print(f"Generating harnesses for {connector_name}...")

    manifest_flows = load_flows_manifest()
    if not manifest_flows:
        print(f"  WARNING: Could not load flows manifest")

    probe_data = load_field_probe(connector_name)
    if not probe_data:
        print(f"  WARNING: No field probe data found for {connector_name}")
        return

    supported_flows = get_supported_flows(probe_data, manifest_flows)
    if not supported_flows:
        print(f"  WARNING: No supported flows found for {connector_name}")
        return

    print(f"  Found {len(supported_flows)} supported flows")
    
    # Build flow data for generators
    flow_items, flow_metadata, message_schemas = build_flow_data(
        connector_name, supported_flows, probe_data
    )

    # Generate Python harness
    python_harness = generate_python_harness(
        connector_name, supported_flows, flow_items, flow_metadata, message_schemas
    )
    python_connector_dir = PYTHON_HARNESS_DIR / connector_name
    python_connector_dir.mkdir(parents=True, exist_ok=True)
    python_out = python_connector_dir / f"{connector_name}.py"
    python_out.write_text(python_harness)

    # Generate JavaScript harness
    js_harness = generate_javascript_harness(
        connector_name, supported_flows, flow_items, flow_metadata, message_schemas
    )
    js_connector_dir = JS_HARNESS_DIR / connector_name
    js_connector_dir.mkdir(parents=True, exist_ok=True)
    js_out = js_connector_dir / f"{connector_name}.ts"
    js_out.write_text(js_harness)

    # Generate Kotlin harness
    kotlin_harness = generate_kotlin_harness(
        connector_name, supported_flows, flow_items, flow_metadata, message_schemas
    )
    kotlin_connector_dir = KOTLIN_HARNESS_DIR / connector_name
    kotlin_connector_dir.mkdir(parents=True, exist_ok=True)
    kotlin_out = kotlin_connector_dir / f"{connector_name}.kt"
    kotlin_out.parent.mkdir(parents=True, exist_ok=True)
    kotlin_out.write_text(kotlin_harness)

    # Generate Rust harness
    rust_harness = generate_rust_harness(
        connector_name, supported_flows, flow_items, flow_metadata, message_schemas
    )
    rust_connector_dir = RUST_HARNESS_DIR / connector_name
    rust_connector_dir.mkdir(parents=True, exist_ok=True)
    rust_out = rust_connector_dir / f"{connector_name}.rs"
    rust_out.write_text(rust_harness)


def main():
    import argparse
    parser = argparse.ArgumentParser(
        description="Generate flow harnesses for mock mode testing"
    )
    parser.add_argument(
        "--connector",
        help="Generate harness for specific connector only (deprecated, use --connectors)"
    )
    parser.add_argument(
        "--connectors",
        help="Comma-separated list of connectors to generate harnesses for"
    )
    parser.add_argument(
        "--all",
        action="store_true",
        help="Generate harnesses for all connectors with field probe data"
    )
    args = parser.parse_args()

    if args.connector:
        # Backward compatibility
        generate_harnesses_for_connector(args.connector)
    elif args.connectors:
        connectors = [c.strip() for c in args.connectors.split(",")]
        print(f"Generating harnesses for {len(connectors)} connectors...")
        print()
        for connector in connectors:
            generate_harnesses_for_connector(connector)
            print()
    elif args.all:
        if not FIELD_PROBE_DIR.exists():
            print(f"ERROR: Field probe directory not found: {FIELD_PROBE_DIR}")
            sys.exit(1)

        connectors = sorted(p.stem for p in FIELD_PROBE_DIR.glob("*.json"))
        print(f"Generating harnesses for {len(connectors)} connectors...")
        print()

        for connector in connectors:
            generate_harnesses_for_connector(connector)
            print()
    else:
        parser.print_help()
        sys.exit(1)

    print("Done.")


if __name__ == "__main__":
    main()
