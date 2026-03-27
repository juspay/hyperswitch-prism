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
    ("authorize",                "direct_payment",   "authorize",           "_build_authorize_request",              "AUTOMATIC"),
    ("capture",                  "direct_payment",   "capture",             "_build_capture_request",                "txn_id"),
    ("void",                     "direct_payment",   "void",                "_build_void_request",                   "txn_id"),
    ("get",                      "direct_payment",   "get",                 "_build_get_request",                    "txn_id"),
    ("refund",                   "direct_payment",   "refund",              "_build_refund_request",                 "txn_id"),
    ("reverse",                  "direct_payment",   "reverse",             "_build_reverse_request",                "txn_id"),
    ("create_customer",          "customer",         "create",              "_build_create_customer_request",        "none"),
    ("tokenize",                 "payment_method",   "tokenize",            "_build_tokenize_request",               "none"),
    ("setup_recurring",          "direct_payment",   "setup_recurring",     "_build_setup_recurring_request",        "none"),
    ("recurring_charge",         "recurring_payment","charge",              "_build_recurring_charge_request",       "none"),
    ("pre_authenticate",         "payment_method_authentication", "pre_authenticate",  "_build_pre_authenticate_request",  "none"),
    ("authenticate",             "payment_method_authentication", "authenticate",      "_build_authenticate_request",      "none"),
    ("post_authenticate",        "payment_method_authentication", "post_authenticate", "_build_post_authenticate_request", "none"),
    ("handle_event",             "event",            "handle_event",        "_build_handle_event_request",           "none"),
    ("create_access_token",      "merchant_authentication", "create_access_token",  "_build_create_access_token_request",  "none"),
    ("create_session_token",     "merchant_authentication", "create_session_token", "_build_create_session_token_request", "none"),
]

# capture/void: run inline MANUAL authorize first (AUTOMATIC txn_id can't be captured)
SELF_AUTH_FLOWS = {"capture", "void"}
# get/refund/reverse: receive connector txn_id from shared AUTOMATIC pre-run authorize
TXN_ID_FLOWS    = {"get", "refund", "reverse"}


# ── Response formatting ───────────────────────────────────────────────────────


def _format_result(res) -> str:
    """Extract key fields from a gRPC response message for display."""
    try:
        from google.protobuf.json_format import MessageToDict
        d = MessageToDict(res, preserving_proto_field_name=True,
                          including_default_value_fields=False)
    except Exception:
        return f"status_code: {getattr(res, 'status_code', '?')}"

    parts: List[str] = []

    # ID fields — short label, value
    _id_keys = [
        ("connector_transaction_id", "txn_id"),
        ("connector_refund_id",      "refund_id"),
        ("connector_customer_id",    "customer_id"),
        ("payment_method_token",     "token"),
        ("connector_recurring_payment_id", "recurring_id"),
    ]
    for key, label in _id_keys:
        val = d.get(key)
        if val:
            parts.append(f"{label}: {val}")

    # Status enum (string name via MessageToDict)
    if "status" in d:
        parts.append(f"status: {d['status']}")

    # HTTP status code
    sc = d.get("status_code")
    if sc is not None:
        parts.append(f"http: {sc}")

    # Error message (best-effort extraction)
    err = d.get("error", {})
    if err:
        msg = (
            (err.get("unified_details") or {}).get("message")
            or err.get("message", "")
        )
        parts.append(f"error: {msg or str(err)}")

    return ", ".join(parts) if parts else f"status_code: {getattr(res, 'status_code', '?')}"


# ── Request building ──────────────────────────────────────────────────────────


def build_request(mod, builder_fn: str, arg_type: str, arg: str):
    """
    Call build_{flow}_request() from the connector module.
    Returns None if the builder doesn't exist in the module.
    """
    fn = getattr(mod, builder_fn, None)
    if fn is None:
        return None
    if arg_type == "none":
        return fn()
    return fn(arg)


# ── Credentials ───────────────────────────────────────────────────────────────


def load_creds(creds_file: str) -> Dict[str, Any]:
    if not os.path.exists(creds_file):
        return {}
    with open(creds_file, encoding="utf-8") as f:
        raw = json.load(f)
    # Single-connector format: {"connector": "stripe", "endpoint": ...}
    if isinstance(raw.get("connector"), str) and isinstance(raw.get("endpoint"), str):
        return {raw["connector"]: raw}
    return raw


def build_grpc_config(connector: str, cred: Dict[str, Any]) -> GrpcConfig:
    def s(*keys):
        for k in keys:
            v = cred.get(k)
            if isinstance(v, str) and v:
                return v
            if isinstance(v, dict):
                inner = v.get("value")
                if isinstance(inner, str) and inner:
                    return inner
        return None

    # Build connector-specific config for x-connector-config header
    # Capitalize first letter of connector name for the config key
    connector_variant = connector[0].upper() + connector[1:] if connector else connector
    
    api_key = s("api_key", "apiKey") or "placeholder"
    api_secret = s("api_secret", "apiSecret")
    key1 = s("key1")
    merchant_id = s("merchant_id", "merchantId")
    tenant_id = s("tenant_id", "tenantId")
    
    # Build the connector-specific config object
    connector_specific_config: Dict[str, Any] = {"api_key": api_key}
    if api_secret:
        connector_specific_config["api_secret"] = api_secret
    if key1:
        connector_specific_config["key1"] = key1
    if merchant_id:
        connector_specific_config["merchant_id"] = merchant_id
    if tenant_id:
        connector_specific_config["tenant_id"] = tenant_id
    
    # Build the full x-connector-config format
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


# ── Connector runner ──────────────────────────────────────────────────────────


def run_connector(connector_name: str, examples_dir: str, cred: Dict[str, Any]) -> bool:
    py_file = Path(examples_dir) / connector_name / "python" / f"{connector_name}.py"
    if not py_file.exists():
        print(_grey(f"  [{connector_name}] No Python file found at {py_file}, skipping."))
        return True

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
        print(_grey(f"  [{connector_name}] No flows to run, skipping."))
        return True

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
            res = client.direct_payment.authorize(req)
            authorize_txn_id = res.connector_transaction_id or txn_id
            result = _format_result(res)
            if res.status_code >= 400:
                print(f"{_yellow('~ connector error')} {_grey(f'— {result}')}")
            else:
                print(f"{_green('✓ ok')} {_grey(f'— {result}')}")
        except Exception as e:
            err_str = str(e)
            is_transport = any(x in err_str.lower() for x in [
                "unavailable", "deadlineexceeded", "connection refused", 
                "transport error", "dns error", "connection reset"
            ])
            if is_transport:
                print(f"{_red('✗ FAILED')} {_grey(f'— {err_str}')}")
                pre_run_failed = True
            else:
                print(f"{_yellow('~ connector error')} {_grey(f'— {err_str}')}")
                pre_run_failed = True

    all_passed = True

    for flow_key, client_attr, method_name, builder_fn, arg_type in present_flows:
        # Skip authorize — already handled in pre-run above
        if flow_key == "authorize" and has_authorize and has_dependents:
            if not pre_run_failed:
                print(f"  [{flow_key}] running … ", end="", flush=True)
                print(f"{_green('✓ ok')} {_grey(f'— txn_id: {authorize_txn_id}')}")
            continue

        print(f"  [{flow_key}] running … ", end="", flush=True)
        
        try:
            if flow_key in SELF_AUTH_FLOWS:
                # capture/void: inline MANUAL authorize first
                auth_req = build_request(mod, "_build_authorize_request", "MANUAL", "MANUAL")
                if auth_req is None:
                    raise RuntimeError("_build_authorize_request not found")
                auth = client.direct_payment.authorize(auth_req)
                if auth.status_code >= 400:
                    raise RuntimeError(f"inline authorize failed (status {auth.status_code})")
                self_txn_id = auth.connector_transaction_id or txn_id
                req = build_request(mod, builder_fn, arg_type, self_txn_id)
                if req is None:
                    raise RuntimeError(f"{builder_fn} not found in connector module")
                sub_client = getattr(client, client_attr)
                res = getattr(sub_client, method_name)(req)
                result = _format_result(res)

            else:
                arg = authorize_txn_id if flow_key in TXN_ID_FLOWS else txn_id
                req = build_request(mod, builder_fn, arg_type, arg)
                if req is None:
                    raise RuntimeError(f"{builder_fn} not found in connector module")
                sub_client = getattr(client, client_attr)
                res = getattr(sub_client, method_name)(req)
                result = _format_result(res)

            print(f"{_green('✓ ok')} {_grey(f'— {result}')}")

        except Exception as e:
            err_str = str(e)
            # Check if it's a transport error vs connector error
            is_transport = any(x in err_str.lower() for x in [
                "unavailable", "deadlineexceeded", "connection refused", 
                "transport error", "dns error", "connection reset"
            ])
            if is_transport:
                print(f"{_red('✗ FAILED')} {_grey(f'— {err_str}')}")
                all_passed = False
            else:
                print(f"{_yellow('~ connector error')} {_grey(f'— {err_str}')}")

    return all_passed


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

    print(_bold("hyperswitch gRPC smoke test (Python)"))
    print(_grey(f"connectors: {', '.join(connectors)}"))
    print()

    any_failed = False

    for connector in connectors:
        print(_bold(f"── {connector} ──"))
        raw   = all_creds.get(connector)
        creds = raw if isinstance(raw, list) else ([raw] if raw else [{}])
        for cred in creds:
            passed = run_connector(connector, examples_dir, cred)
            if not passed:
                any_failed = True
        print()

    if any_failed:
        print(_red("Some gRPC tests FAILED."), file=sys.stderr)
        sys.exit(1)
    else:
        print(_green("All gRPC tests passed."))


if __name__ == "__main__":
    main()
