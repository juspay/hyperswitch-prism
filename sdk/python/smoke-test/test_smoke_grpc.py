#!/usr/bin/env python3
"""
gRPC smoke test for the hyperswitch-payments Python SDK.

For each supported flow (filtered by data/field_probe/{connector}.json),
calls the connector's _build_*_request() builder to construct the proto
request, then dispatches it directly through the GrpcClient.

No grpc_* wrapper functions are needed in the connector Python file.

Usage:
    python3 test_smoke_grpc.py --connectors stripe --examples-dir /path/to/examples
    python3 test_smoke_grpc.py --creds-file /path/to/creds.json --connectors stripe,adyen
"""

import argparse
import importlib.util
import json
import os
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional, Set, Tuple

# ── ANSI color helpers ────────────────────────────────────────────────────────
_NO_COLOR = bool(os.environ.get("NO_COLOR")) or (
    not bool(os.environ.get("FORCE_COLOR")) and not sys.stdout.isatty()
)

def _c(code: str, text: str) -> str:
    return text if _NO_COLOR else f"\033[{code}m{text}\033[0m"

def _green(t: str) -> str: return _c("32", t)
def _red(t: str)   -> str: return _c("31", t)
def _yellow(t: str) -> str: return _c("33", t)
def _grey(t: str)  -> str: return _c("90", t)
def _bold(t: str)  -> str: return _c("1",  t)

# Add SDK src to path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "src"))

try:
    from payments.grpc_client import GrpcClient, GrpcConfig
except ImportError as e:
    print(f"Error importing payments package: {e}")
    print("Make sure the wheel is installed: pip install dist/hyperswitch_payments-*.whl")
    sys.exit(1)

# ── Flow manifest ─────────────────────────────────────────────────────────────

def load_flow_manifest(sdk_root: Path) -> List[str]:
    """Load the canonical flow manifest from flows.json."""
    # Try multiple locations for flows.json
    manifest_locations = [
        sdk_root / "generated" / "flows.json",
    ]
    
    # Check environment variable
    if env_path := os.environ.get("FLOWS_JSON_PATH"):
        manifest_locations.insert(0, Path(env_path))
    
    # Check cwd as fallback
    manifest_locations.append(Path.cwd() / "flows.json")
    
    for manifest_path in manifest_locations:
        if manifest_path.exists():
            with open(manifest_path) as f:
                data = json.load(f)
            return data["flows"]
    
    # If we get here, we couldn't find the file
    searched = "\n  - ".join(str(p) for p in manifest_locations)
    raise FileNotFoundError(
        f"flows.json not found. Searched:\n  - {searched}\nRun: make generate"
    )

# ── Field-probe support filtering ────────────────────────────────────────────

def load_field_probe(
    connector: str, examples_dir: str
) -> Tuple[Optional[Set[str]], Dict[str, Any]]:
    """
    Load data/field_probe/{connector}.json.

    Returns (supported_flows, probe_requests) where:
      - supported_flows: set of flow keys with at least one 'supported' variant,
                         or None if no probe file exists.
      - probe_requests:  dict of flow_key -> proto_request dict (snake_case) for
                         the first supported variant, used as payload fallback.
    """
    probe_file = Path(examples_dir) / ".." / "data" / "field_probe" / f"{connector}.json"
    if not probe_file.exists():
        return None, {}
    with open(probe_file, encoding="utf-8") as f:
        probe = json.load(f)
    flows = probe.get("flows") or {}
    supported_flows: Set[str] = set()
    probe_requests:  Dict[str, Any] = {}
    for flow_name, variants in flows.items():
        for variant in variants.values():
            if variant.get("status") == "supported":
                supported_flows.add(flow_name)
                if variant.get("proto_request"):
                    probe_requests[flow_name] = variant["proto_request"]
                break
    return supported_flows, probe_requests


# ── Flow metadata ─────────────────────────────────────────────────────────────
# Canonical ordering matches Rust build.rs and JS FLOW_META.
# (flow_key, client_attr, method_name, builder_fn_name, arg_type)
#   arg_type: "AUTOMATIC" | "MANUAL" | "txn_id" | "none"

FLOW_META = [
    ("authorize",                "payment",   "authorize",           "_build_authorize_request",              "AUTOMATIC"),
    ("capture",                  "payment",   "capture",             "_build_capture_request",                "txn_id"),
    ("void",                     "payment",   "void",                "_build_void_request",                   "txn_id"),
    ("get",                      "payment",   "get",                 "_build_get_request",                    "txn_id"),
    ("refund",                   "payment",   "refund",              "_build_refund_request",                 "txn_id"),
    ("reverse",                  "payment",   "reverse",             "_build_reverse_request",                "txn_id"),
    ("create_customer",          "customer",         "create",              "_build_create_customer_request",        "none"),
    ("tokenize",                 "payment_method",   "tokenize",            "_build_tokenize_request",               "none"),
    ("setup_recurring",          "payment",   "setup_recurring",     "_build_setup_recurring_request",        "none"),
    ("recurring_charge",         "recurring_payment","charge",              "_build_recurring_charge_request",       "none"),
    ("pre_authenticate",         "payment_method_authentication", "pre_authenticate",  "_build_pre_authenticate_request",  "none"),
    ("authenticate",             "payment_method_authentication", "authenticate",      "_build_authenticate_request",      "none"),
    ("post_authenticate",        "payment_method_authentication", "post_authenticate", "_build_post_authenticate_request", "none"),
    ("handle_event",             "event",            "handle_event",        "_build_handle_event_request",           "none"),
    ("create_access_token",      "payment",   "create_access_token", "_build_create_access_token_request",    "none"),
    ("create_session_token",     "payment",   "create_session_token","_build_create_session_token_request",   "none"),
    ("create_sdk_session_token", "payment",   "create_sdk_session_token", "_build_create_sdk_session_token_request", "none"),
]

TXN_ID_FLOWS   = {"capture", "void", "get", "refund", "reverse"}
SELF_AUTH_FLOWS = {"capture", "void"}


def _format_result(res) -> str:
    """Format a gRPC response for display."""
    if hasattr(res, "connector_transaction_id") and res.connector_transaction_id:
        return f"txn_id: {res.connector_transaction_id}, status_code: {res.status_code}"
    if hasattr(res, "connector_refund_id") and res.connector_refund_id:
        return f"refund_id: {res.connector_refund_id}, status_code: {res.status_code}"
    if hasattr(res, "connector_customer_id") and res.connector_customer_id:
        return f"customer_id: {res.connector_customer_id}, status_code: {res.status_code}"
    if hasattr(res, "payment_method_token") and res.payment_method_token:
        return f"token: {res.payment_method_token}, status_code: {res.status_code}"
    return f"status_code: {res.status_code}"


def build_request(mod, builder_fn: str, arg_type: str, arg: str):
    """Call the connector's builder function with the appropriate argument."""
    fn = getattr(mod, builder_fn, None)
    if fn is None:
        return None
    if arg_type == "none":
        return fn()
    return fn(arg)


# ── Credentials loading ───────────────────────────────────────────────────────

def load_creds(creds_file: str) -> Dict[str, Any]:
    if not os.path.exists(creds_file):
        return {}
    with open(creds_file, encoding="utf-8") as f:
        return json.load(f)


def build_grpc_config(connector: str, cred: Dict[str, Any]) -> GrpcConfig:
    """Build GrpcConfig from credentials entry."""
    def s(*keys: str) -> Optional[str]:
        for k in keys:
            v = cred.get(k)
            if isinstance(v, dict):
                v = v.get("value")
            if isinstance(v, str) and v:
                return v
        return None

    # Build connector-specific config for x-connector-config header
    connector_variant = connector[0].upper() + connector[1:] if connector else connector
    
    api_key = s("api_key", "apiKey") or "placeholder"
    api_secret = s("api_secret", "apiSecret")
    key1 = s("key1")
    merchant_id = s("merchant_id", "merchantId")
    tenant_id = s("tenant_id", "tenantId")
    
    connector_specific_config: Dict[str, Any] = {"api_key": api_key}
    if api_secret:
        connector_specific_config["api_secret"] = api_secret
    if key1:
        connector_specific_config["key1"] = key1
    if merchant_id:
        connector_specific_config["merchant_id"] = merchant_id
    if tenant_id:
        connector_specific_config["tenant_id"] = tenant_id
    
    connector_config = {
        "config": {
            connector_variant: connector_specific_config
        }
    }
    
    return GrpcConfig(
        endpoint=s("endpoint") or "http://localhost:8000",
        connector=connector,
        connector_config=connector_config,
    )


# ── Scenario result tracking ─────────────────────────────────────────────────

class ScenarioResult:
    def __init__(self, status: str, message: Optional[str] = None, 
                 reason: Optional[str] = None, error: Optional[str] = None):
        self.status = status  # "passed", "skipped", "failed"
        self.message = message
        self.reason = reason
        self.error = error

class ConnectorResult:
    def __init__(self, connector: str):
        self.connector = connector
        self.status = "passed"  # "passed", "failed", "skipped"
        self.scenarios: Dict[str, ScenarioResult] = {}
        self.error: Optional[str] = None


def is_transport_error(err_str: str) -> bool:
    """Check if error is a transport-level error (vs connector error)."""
    return any(x in err_str.lower() for x in [
        "unavailable", "deadlineexceeded", "connection refused", 
        "transport error", "dns error", "connection reset", "tonic transport"
    ])


# ── Connector runner ──────────────────────────────────────────────────────────

def run_connector(connector_name: str, examples_dir: str, cred: Dict[str, Any], 
                  manifest: List[str]) -> ConnectorResult:
    result = ConnectorResult(connector_name)
    
    # Try both new structure (with python/ subdirectory) and old structure (flat)
    py_file = Path(examples_dir) / connector_name / "python" / f"{connector_name}.py"
    if not py_file.exists():
        py_file = Path(examples_dir) / connector_name / f"{connector_name}.py"
    if not py_file.exists():
        result.status = "skipped"
        result.error = f"No Python file found at {py_file}"
        print(_grey(f"  [{connector_name}] No Python file found at {py_file}, skipping."))
        return result

    spec = importlib.util.spec_from_file_location(connector_name, py_file)
    mod  = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)

    config = build_grpc_config(connector_name, cred)
    client = GrpcClient(config)

    # Field-probe filtering
    supported_flows, probe_requests = load_field_probe(connector_name, examples_dir)
    if supported_flows is not None:
        print(_grey(f"  [{connector_name}] field_probe: {len(supported_flows)} supported flows"))

    present_flows = [
        (fk, ca, mn, bfn, at)
        for fk, ca, mn, bfn, at in FLOW_META
        if supported_flows is None or fk in supported_flows
    ]

    if not present_flows:
        result.status = "skipped"
        result.error = "No flows to run"
        print(_grey(f"  [{connector_name}] No flows to run, skipping."))
        return result

    import time
    txn_id         = f"probe_py_grpc_{int(time.time() * 1000)}"
    authorize_txn_id = txn_id

    has_authorize  = any(fk == "authorize" for fk, *_ in present_flows)
    has_dependents = any(fk in TXN_ID_FLOWS for fk, *_ in present_flows)

    # Pre-run AUTOMATIC authorize to get a real connector txn_id for get/refund/reverse
    pre_run_failed = False
    if has_authorize and has_dependents:
        print("  [authorize] running … ", end="", flush=True)
        try:
            req = build_request(mod, "_build_authorize_request", "AUTOMATIC", "AUTOMATIC")
            if req is None:
                raise RuntimeError("_build_authorize_request not found in connector module")
            res = client.payment.authorize(req)
            authorize_txn_id = res.connector_transaction_id or txn_id
            result_str = _format_result(res)
            if res.status_code >= 400:
                print(f"{_yellow('SKIPPED (connector error)')} {_grey(f'— {result_str}')}")
                result.scenarios["authorize"] = ScenarioResult(
                    status="skipped", reason="connector_error", message=result_str
                )
            else:
                print(f"{_green('PASSED')} {_grey(f'— {result_str}')}")
                result.scenarios["authorize"] = ScenarioResult(
                    status="passed", message=result_str
                )
        except Exception as e:
            err_str = str(e)
            if is_transport_error(err_str):
                print(f"{_red('FAILED')} {_grey(f'— {err_str}')}")
                result.scenarios["authorize"] = ScenarioResult(
                    status="failed", error=err_str
                )
                pre_run_failed = True
                result.status = "failed"
            else:
                print(f"{_yellow('SKIPPED (connector error)')} {_grey(f'— {err_str}')}")
                result.scenarios["authorize"] = ScenarioResult(
                    status="skipped", reason="connector_error", message=err_str
                )

    for flow_key, client_attr, method_name, builder_fn, arg_type in present_flows:
        # Skip authorize — already handled in pre-run above
        if flow_key == "authorize" and has_authorize and has_dependents:
            continue

        print(f"  [{flow_key}] running … ", end="", flush=True)
        
        try:
            if flow_key in SELF_AUTH_FLOWS:
                # capture/void: inline MANUAL authorize first
                auth_req = build_request(mod, "_build_authorize_request", "MANUAL", "MANUAL")
                if auth_req is None:
                    raise RuntimeError("_build_authorize_request not found")
                auth = client.payment.authorize(auth_req)
                if auth.status_code >= 400:
                    raise RuntimeError(f"inline authorize failed (status {auth.status_code})")
                self_txn_id = auth.connector_transaction_id or txn_id
                req = build_request(mod, builder_fn, arg_type, self_txn_id)
                if req is None:
                    raise RuntimeError(f"{builder_fn} not found in connector module")
                sub_client = getattr(client, client_attr)
                res = getattr(sub_client, method_name)(req)
                result_str = _format_result(res)

            else:
                arg = authorize_txn_id if flow_key in TXN_ID_FLOWS else txn_id
                req = build_request(mod, builder_fn, arg_type, arg)
                if req is None:
                    raise RuntimeError(f"{builder_fn} not found in connector module")
                sub_client = getattr(client, client_attr)
                res = getattr(sub_client, method_name)(req)
                result_str = _format_result(res)

            print(f"{_green('PASSED')} {_grey(f'— {result_str}')}")
            result.scenarios[flow_key] = ScenarioResult(
                status="passed", message=result_str
            )

        except Exception as e:
            err_str = str(e)
            if is_transport_error(err_str):
                print(f"{_red('FAILED')} {_grey(f'— {err_str}')}")
                result.scenarios[flow_key] = ScenarioResult(
                    status="failed", error=err_str
                )
                result.status = "failed"
            else:
                print(f"{_yellow('SKIPPED (connector error)')} {_grey(f'— {err_str}')}")
                result.scenarios[flow_key] = ScenarioResult(
                    status="skipped", reason="connector_error", message=err_str
                )

    # Update connector status based on scenarios
    if result.status != "failed":
        if all(s.status in ("passed", "skipped") for s in result.scenarios.values()):
            result.status = "passed"
        else:
            result.status = "failed"
    
    return result


def print_result(result: ConnectorResult) -> None:
    """Print connector result summary."""
    if result.status == "passed":
        passed_count = sum(1 for s in result.scenarios.values() if s.status == "passed")
        skipped_count = sum(1 for s in result.scenarios.values() if s.status == "skipped")
        print(_green(f"  PASSED") + f" ({passed_count} passed, {skipped_count} skipped)")
        for key, scenario in result.scenarios.items():
            if scenario.status == "passed":
                print(_green(f"    {key}: ✓"))
            elif scenario.status == "skipped":
                print(_yellow(f"    {key}: ~ skipped ({scenario.reason})"))
    elif result.status == "skipped":
        print(_grey(f"  SKIPPED ({result.error or 'unknown'})"))
    else:
        print(_red(f"  FAILED"))
        for key, scenario in result.scenarios.items():
            if scenario.status == "failed":
                print(_red(f"    {key}: ✗ FAILED — {scenario.error}"))
        if result.error:
            print(_red(f"  Error: {result.error}"))


def print_summary(results: List[ConnectorResult]) -> int:
    """Print test summary and return exit code."""
    print(f"\n{'='*60}")
    print(_bold("TEST SUMMARY"))
    print(f"{'='*60}\n")

    passed = sum(1 for r in results if r.status == "passed")
    skipped = sum(1 for r in results if r.status == "skipped")
    failed = sum(1 for r in results if r.status == "failed")

    # Count per-scenario statuses
    total_flows_passed = 0
    total_flows_skipped = 0
    total_flows_failed = 0
    for r in results:
        for scenario in r.scenarios.values():
            if scenario.status == "passed":
                total_flows_passed += 1
            elif scenario.status == "skipped":
                total_flows_skipped += 1
            elif scenario.status == "failed":
                total_flows_failed += 1

    print(f"Total connectors:   {len(results)}")
    print(_green(f"Passed:  {passed}"))
    print(_grey(f"Skipped: {skipped} (no examples or placeholder credentials)"))
    print((_red if failed > 0 else _green)(f"Failed:  {failed}"))
    print()
    print(f"Flow results:")
    print(_green(f"  {total_flows_passed} flows PASSED"))
    if total_flows_skipped > 0:
        print(_yellow(f"  {total_flows_skipped} flows SKIPPED (connector errors)"))
    if total_flows_failed > 0:
        print(_red(f"  {total_flows_failed} flows FAILED"))
    print()

    if failed > 0:
        print(_red("Failed connectors:"))
        for result in results:
            if result.status == "failed":
                print(_red(f"  - {result.connector}: {result.error or 'see scenarios above'}"))
        print()
        return 1

    if passed == 0 and skipped > 0:
        print(_yellow("All tests skipped (no valid flows found)"))
        return 1

    print(_green("All tests completed successfully!"))
    return 0


# ── Main ──────────────────────────────────────────────────────────────────────


def main() -> None:
    parser = argparse.ArgumentParser(description="hyperswitch gRPC smoke test (Python)")
    parser.add_argument("--connectors", default="stripe",
                        help="Comma-separated connector names (default: stripe)")
    parser.add_argument("--examples-dir",
                        default=str(Path(__file__).parent / "../../../examples"),
                        help="Path to examples/ directory")
    parser.add_argument("--creds-file", default=None,
                        help="JSON credentials file (default: creds.json at cwd)")
    args = parser.parse_args()

    connectors   = [c.strip() for c in args.connectors.split(",")]
    examples_dir = args.examples_dir

    creds_file = args.creds_file or os.path.join(os.getcwd(), "creds.json")

    all_creds = load_creds(creds_file)
    
    # Load flow manifest
    try:
        sdk_root = Path(__file__).parent.parent
        manifest = load_flow_manifest(sdk_root)
    except FileNotFoundError as e:
        print(_red(f"MANIFEST ERROR: {e}"))
        sys.exit(1)

    print(_bold("hyperswitch gRPC smoke test (Python)"))
    print(_grey(f"connectors: {', '.join(connectors)}"))
    print()

    results: List[ConnectorResult] = []

    for connector in connectors:
        print(_bold(f"── {connector} ──"))
        raw   = all_creds.get(connector)
        creds = raw if isinstance(raw, list) else ([raw] if raw else [{}])
        for cred in creds:
            result = run_connector(connector, examples_dir, cred, manifest)
            results.append(result)
            print_result(result)
        print()

    exit_code = print_summary(results)
    sys.exit(exit_code)


if __name__ == "__main__":
    main()
