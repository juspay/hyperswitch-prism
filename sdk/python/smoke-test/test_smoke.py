#!/usr/bin/env python3
"""
Multi-connector smoke test for the hyperswitch-payments SDK.

Loads connector credentials from external JSON file and runs all scenario
functions found in examples/{connector}/ for each connector.

Each scenario file (checkout_card.py, refund.py, etc.) is auto-generated and
exports a process_{scenario_key}(merchant_transaction_id, config=...) function.
The smoke test injects real credentials via the config parameter — no request
building happens here.

Usage:
    # Test all connectors
    python3 test_smoke.py --creds-file creds.json --all

    # Test specific connectors
    python3 test_smoke.py --creds-file creds.json --connectors stripe,adyen

    # Dry run (skip HTTP calls, just check examples load)
    python3 test_smoke.py --creds-file creds.json --all --dry-run
"""

import argparse
import importlib.util
import json
import os
import sys
import time
from pathlib import Path
from typing import Dict, List, Any, Optional

# ── ANSI color helpers ─────────────────────────────────────────────────────────
_NO_COLOR = not sys.stdout.isatty() or os.environ.get("NO_COLOR")

def _c(code: str, text: str) -> str:
    return text if _NO_COLOR else f"\033[{code}m{text}\033[0m"

def _green(t: str)  -> str: return _c("32", t)
def _yellow(t: str) -> str: return _c("33", t)
def _red(t: str)    -> str: return _c("31", t)
def _grey(t: str)   -> str: return _c("90", t)
def _bold(t: str)   -> str: return _c("1",  t)

# Add parent directory to path for imports when running directly
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..', 'src'))

try:
    from payments import (
        ConnectorConfig,
        ConnectorSpecificConfig,
        SdkOptions,
        Environment,
        IntegrationError,
        ConnectorError,
    )
    from payments.generated.connector_service_ffi import InternalError
except ImportError as e:
    print(f"Error importing payments package: {e}")
    print("Make sure the wheel is installed: pip install dist/hyperswitch_payments-*.whl")
    sys.exit(1)

# Root of the examples directory — overridden at runtime via --examples-dir or
# EXAMPLES_DIR env var.  The file-relative default only works when running from
# the repo directly (not from a temp directory like make test-package uses).
_DEFAULT_EXAMPLES_DIR = Path(__file__).parent.parent.parent.parent / "examples"

# Placeholder values that indicate credentials are not configured
PLACEHOLDER_VALUES = {"", "placeholder", "test", "dummy", "sk_test_placeholder"}


def load_credentials(creds_file: str) -> Dict[str, Any]:
    """Load connector credentials from JSON file."""
    if not os.path.exists(creds_file):
        raise FileNotFoundError(f"Credentials file not found: {creds_file}")

    with open(creds_file, 'r') as f:
        return json.load(f)


def is_placeholder(value: str) -> bool:
    """Check if a value is a placeholder."""
    if not value:
        return True
    return value.lower() in PLACEHOLDER_VALUES or "placeholder" in value.lower()


def has_valid_credentials(auth_config: Dict[str, Any]) -> bool:
    """Check if auth config has valid (non-placeholder) credentials."""
    for key, value in auth_config.items():
        if key in ("metadata", "_comment"):
            continue
        if isinstance(value, dict) and "value" in value:
            if isinstance(value["value"], str) and not is_placeholder(value["value"]):
                return True
        elif isinstance(value, str) and not is_placeholder(value):
            return True
    return False


def _build_connector_config(connector_key: str, auth_config: Dict[str, Any]) -> ConnectorConfig:
    """Build a ConnectorConfig from a connector name and creds.json auth block."""
    import payments.generated.payment_pb2 as _payment_pb2
    import payments.generated.payment_methods_pb2 as _payment_methods_pb2

    # Find the connector-specific config class (e.g., StripeConfig for "stripe")
    config_class = None
    target = connector_key.lower() + "config"
    for name in dir(_payment_pb2):
        if name.lower() == target:
            config_class = getattr(_payment_pb2, name)
            break

    if config_class is None:
        connector_specific = ConnectorSpecificConfig()
    else:
        valid_fields = {f.name for f in config_class.DESCRIPTOR.fields}
        kwargs: Dict[str, Any] = {}
        for key, value in auth_config.items():
            if key in ("_comment", "metadata") or key not in valid_fields:
                continue
            if isinstance(value, dict) and "value" in value:
                kwargs[key] = _payment_methods_pb2.SecretString(value=str(value["value"]))
            elif isinstance(value, str):
                kwargs[key] = value
        connector_specific = ConnectorSpecificConfig(**{connector_key.lower(): config_class(**kwargs)})

    return ConnectorConfig(
        connector_config=connector_specific,
        options=SdkOptions(environment=Environment.SANDBOX),
    )


def load_flow_manifest(sdk_root: Path) -> tuple[list[str], dict[str, str | None]]:
    """Load the canonical flow manifest from flows.json.
    
    Returns:
        Tuple of (flows list, flow_to_example_fn mapping)
        flow_to_example_fn maps flow names to example function names (or None if not implemented)
    """
    # Try multiple locations for flows.json:
    # 1. SDK root (normal case when running from repo)
    # 2. Environment variable (CI/packaged test)
    # 3. Current working directory
    
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
            flows = data["flows"]
            # flow_to_example_fn maps flow names to example function names
            # Examples use scenario-based naming (e.g., "checkout_card") not flow-based (e.g., "authorize")
            flow_to_example_fn = data.get("flow_to_example_fn", {})
            return flows, flow_to_example_fn
    
    # If we get here, we couldn't find the file
    searched = "\n  - ".join(str(p) for p in manifest_locations)
    raise FileNotFoundError(
        f"flows.json not found. Searched:\n  - {searched}\n"
        "Run: make generate"
    )


def discover_and_validate_scenarios(
    module,
    connector_name: str,
    manifest: list[str],
    flow_to_example_fn: dict[str, str | None],
) -> list[tuple[str, callable]] | str:
    """
    Discover and validate scenario functions for a connector.
    
    Uses flow_to_example_fn mapping to find which example functions implement each flow.
    Example functions use scenario-based naming (e.g., "checkout_card") not flow-based (e.g., "authorize").
    
    Implements the 3-check algorithm when SUPPORTED_FLOWS is explicitly defined:
      CHECK 1: No declared flow can be missing its implementation
      CHECK 2: No implementation can exist outside the declaration
      CHECK 3: No declared flow can be stale (removed from manifest)
    
    When SUPPORTED_FLOWS is not present, uses legacy mode which scans for
    process_* functions using the flow_to_example_fn mapping.
    
    Returns list of (example_fn_name, fn) or a string error message if checks fail.
    """
    # Get declared flows from SUPPORTED_FLOWS (legacy mode if not present)
    declared = getattr(module, "SUPPORTED_FLOWS", None)
    legacy_mode = declared is None
    
    if legacy_mode:
        # Legacy mode: include ALL flows from manifest
        # We'll check for implementations during iteration
        declared = list(manifest)
    else:
        # Normalize: ensure list, deduplicate, validate
        if not hasattr(declared, '__iter__'):
            return f"COVERAGE ERROR: SUPPORTED_FLOWS must be iterable (list, tuple, etc.)"
        declared = list(dict.fromkeys(declared))  # Deduplicate while preserving order
    
    if not declared:
        # Empty SUPPORTED_FLOWS is valid - no flows to test
        return []
    
    # Validate flow names are lowercase snake_case
    for name in declared:
        if name != name.lower() or ' ' in name or '-' in name:
            return (
                f"COVERAGE ERROR: Flow name '{name}' in SUPPORTED_FLOWS must be "
                f"lowercase snake_case (e.g., 'authorize', 'payout_create')"
            )
    
    # Find flows with implementations
    implemented = set()
    for flow in declared:
        example_fn = flow_to_example_fn.get(flow) if flow_to_example_fn else None
        if example_fn:
            fn = getattr(module, f"process_{example_fn}", None)
            if fn is not None and callable(fn):
                implemented.add(flow)
    
    # CHECK: Find process_* functions not mapped to any flow
    # Only run this check when SUPPORTED_FLOWS is explicitly defined (not legacy mode)
    if not legacy_mode:
        all_process_fns = {
            name[len("process_"):]
            for name in dir(module)
            if name.startswith("process_") and callable(getattr(module, name))
        }
        mapped_example_fns = set(flow_to_example_fn.values()) if flow_to_example_fn else set()
        undeclared = all_process_fns - mapped_example_fns
        if undeclared:
            return (
                f"COVERAGE ERROR: process_* functions exist but not mapped to any flow: "
                f"{sorted(undeclared)}"
            )
    
    # CHECK 3: Find stale flows (declared but not in manifest)
    manifest_set = set(manifest)
    stale = set(declared) - manifest_set
    if stale:
        return (
            f"COVERAGE ERROR: SUPPORTED_FLOWS contains flows that no longer "
            f"exist in flows.json: {sorted(stale)}. Regenerate or update."
        )
    
    # All checks pass - return ordered (example_fn_name, fn) pairs
    # For flows without implementation, return None as the function
    result = []
    for flow in declared:
        example_fn = flow_to_example_fn.get(flow) if flow_to_example_fn else None
        if example_fn:
            fn = getattr(module, f"process_{example_fn}", None)
            if fn is not None and callable(fn):
                result.append((example_fn, fn))
                continue
        # Flow has no implementation
        result.append((flow, None))
    return result


async def test_connector_scenarios(
    connector_name: str,
    config: ConnectorConfig,
    examples_dir: Path,
    dry_run: bool = False,
    mock: bool = False,
) -> Dict[str, Any]:
    """
    Discover and run all Python scenario functions for a connector.

    Loads examples_dir/{connector_name}/{connector_name}.py and calls each
    process_* function found in _SCENARIO_NAMES order.
    process_{scenario_key}(txn_id, config=config) function.
    """
    result: Dict[str, Any] = {
        "connector": connector_name,
        "status": "pending",
        "scenarios": {},
        "error": None,
    }

    if dry_run:
        result["status"] = "dry_run"
        return result

    connector_dir = examples_dir / connector_name
    if not connector_dir.exists():
        result["status"] = "skipped"
        result["scenarios"] = {"skipped": True, "reason": "no_examples_dir"}
        return result

    consolidated_file = connector_dir / f"{connector_name}.py"
    if not consolidated_file.exists():
        result["status"] = "skipped"
        result["scenarios"] = {"skipped": True, "reason": "no_scenario_files"}
        return result

    spec = importlib.util.spec_from_file_location(
        f"examples.{connector_name}", consolidated_file
    )
    module = importlib.util.module_from_spec(spec)
    try:
        spec.loader.exec_module(module)  # type: ignore[union-attr]
    except Exception as e:
        print(_red(f"    IMPORT ERROR: {e}"), flush=True)
        result["status"] = "failed"
        result["error"] = f"import error: {e}"
        return result

    # Load flow manifest and discover/validate scenarios
    try:
        sdk_root = Path(__file__).parent.parent
        manifest, flow_to_example_fn = load_flow_manifest(sdk_root)
    except FileNotFoundError as e:
        print(_red(f"    MANIFEST ERROR: {e}"), flush=True)
        result["status"] = "failed"
        result["error"] = str(e)
        return result
    
    # In mock mode, generated harnesses use direct flow-based naming (process_{flow})
    # Create a direct mapping from flow to flow (bypassing example naming)
    if mock:
        flow_to_example_fn = {flow: flow for flow in manifest}
    scenarios_or_error = discover_and_validate_scenarios(module, connector_name, manifest, flow_to_example_fn)
    if isinstance(scenarios_or_error, str):
        # Coverage validation failed
        result["status"] = "failed"
        result["error"] = scenarios_or_error
        print(_red(f"    COVERAGE VIILATION: {scenarios_or_error}"), flush=True)
        return result
    
    scenario_fns = scenarios_or_error
    
    # Build a map of example function names to their callable functions
    example_fn_map = {key: fn for key, fn in scenario_fns}
    
    # Test ALL flows from manifest - use flow_to_example_fn mapping to find implementations
    # In mock mode, harnesses use direct flow-based naming (process_{flow}) so bypass mapping
    any_failed = False
    for flow_name in manifest:
        scenario_key = flow_name
        
        # Find the function to call - same logic for both mock and normal mode
        # Try flow name directly first (like mock mode), then fall back to example mapping
        process_fn = example_fn_map.get(flow_name)
        
        if process_fn is None and flow_to_example_fn:
            # Try mapped example function name
            example_fn_name = flow_to_example_fn.get(flow_name)
            if example_fn_name:
                process_fn = example_fn_map.get(example_fn_name)
        
        if process_fn is None:
            # No implementation found for this flow
            print(_grey(f"    [{scenario_key}] NOT IMPLEMENTED — No example function for flow '{flow_name}'"), flush=True)
            result["scenarios"][scenario_key] = {
                "status": "not_implemented",
                "reason": f"No example function for flow '{flow_name}'",
            }
            continue
        
        # Flow is implemented - run it

        txn_id = f"smoke_{scenario_key}_{os.urandom(4).hex()}"
        print(f"    [{scenario_key}] running (txn={txn_id}) ...", flush=True)
        _t0 = time.monotonic()
        try:
            response = await process_fn(txn_id, config=config)
            # Check for error in response (works for both dict and protobuf objects)
            error = None
            if isinstance(response, dict):
                error = response.get('error')
            else:
                error = getattr(response, 'error', None)
            
            # Check if error has meaningful content (not empty/default ErrorInfo)
            error_str = str(error) if error else ""
            # An error is "real" if it contains actual error details like code, message, or reason
            has_error = error_str.strip() and (
                'code:' in error_str or 
                'message:' in error_str or 
                'reason:' in error_str or
                ('error' in error_str.lower() and len(error_str) > 50)  # Long error strings are likely real
            )
            
            if has_error:
                print(_yellow(f"    [{scenario_key}] SKIPPED (connector error)") + f" — {error_str}", flush=True)
                result["scenarios"][scenario_key] = {
                    "status": "skipped",
                    "reason": "connector_error",
                    "detail": error_str,
                }
            else:
                # For display, show the actual response content
                try:
                    response_display = str(response)
                except Exception:
                    response_display = f"<{type(response).__name__}>"
                print(_green(f"    [{scenario_key}] PASSED") + f" — {response_display}", flush=True)
                result["scenarios"][scenario_key] = {
                    "status": "passed",
                    "result": response,
                }
        except IntegrationError as e:
            # Request-phase error — SDK round-trip succeeded, connector rejected.
            detail = f"IntegrationError: {e.error_message} (code={e.error_code}, action={getattr(e, 'suggested_action', None)}, doc={getattr(e, 'doc_url', None)})"
            # IntegrationError is always FAILED — req_transformer failed to build request
            print(_red(f"    [{scenario_key}] FAILED") + f" — {detail}", flush=True)
            result["scenarios"][scenario_key] = {
                "status": "failed",
                "error": detail,
            }
            any_failed = True
        except ConnectorError as e:
            # Response-phase error — SDK round-trip succeeded, connector rejected.
            http_status = getattr(e, 'http_status_code', None)
            detail = f"ConnectorError: {e.error_message} (code={e.error_code}, http={http_status})"
            if mock:
                # In mock mode, ConnectorError means req_transformer successfully built the HTTP request.
                # The error is just from parsing the mock empty response, which is expected.
                print(_green(f"    [{scenario_key}] PASSED") + " — req_transformer OK (mock response)", flush=True)
                result["scenarios"][scenario_key] = {
                    "status": "passed",
                    "reason": "mock_verified",
                    "detail": detail,
                }
            else:
                print(_yellow(f"    [{scenario_key}] SKIPPED (connector error)") + f" — {detail}", flush=True)
                result["scenarios"][scenario_key] = {
                    "status": "skipped",
                    "reason": "connector_error",
                    "detail": detail,
                }
        except InternalError as e:
            # FFI-level connector rejection before HTTP (e.g. InvalidWalletToken)
            msg = getattr(e, 'error_message', None) or str(e)
            # Check if this is a real Rust panic vs a connector-level error
            if "Rust panic:" in msg:
                # Real SDK crash — treat as failure
                detail = f"FFI panic: {msg}"
                print(_red(f"    [{scenario_key}] FAILED") + f" — {detail}", flush=True)
                result["scenarios"][scenario_key] = {
                    "status": "failed",
                    "error": detail,
                }
                any_failed = True
            else:
                # Connector-level error via FFI
                code = getattr(e, 'error_code', None)
                detail = f"InternalError: {code}: {msg}" if code else f"InternalError: {msg}"
                if mock:
                    # In mock mode, non-panic InternalError means req_transformer succeeded
                    print(_green(f"    [{scenario_key}] MOCK VERIFIED"), flush=True)
                    result["scenarios"][scenario_key] = {
                        "status": "passed",
                        "reason": "mock_verified",
                    }
                else:
                    print(_yellow(f"    [{scenario_key}] SKIPPED (connector error)") + f" — {detail}", flush=True)
                    result["scenarios"][scenario_key] = {
                        "status": "skipped",
                        "reason": "connector_error",
                        "detail": detail,
                    }
        except Exception as e:
            # Unexpected Python-level failure — import crash, serialization bug, etc.
            # This indicates a real SDK or example-file problem.
            import traceback
            tb = traceback.format_exc()
            print(_red(f"    [{scenario_key}] FAILED") + f" — {type(e).__name__}: {e}", flush=True)
            print(f"    Traceback: {tb[:500]}", flush=True)  # Print first 500 chars of traceback
            result["scenarios"][scenario_key] = {
                "status": "failed",
                "error": f"{type(e).__name__}: {e}",
            }
            any_failed = True
        # Record timing for the scenario
        _dur_ms = (time.monotonic() - _t0) * 1000
        if scenario_key in result["scenarios"] and isinstance(result["scenarios"][scenario_key], dict):
            result["scenarios"][scenario_key]["duration_ms"] = _dur_ms

    result["status"] = "failed" if any_failed else "passed"
    return result


def _install_mock_intercept():
    """Install mock HTTP intercept for testing req_transformer without real HTTP."""
    import payments.http_client as _http
    from payments.http_client import HttpResponse

    async def _mock(req):
        print(f"      [mock] {req.method} {req.url}", flush=True)
        return HttpResponse(status_code=200, headers={}, body=b'{}', latency_ms=0.0)

    _http._intercept = _mock


async def run_tests_async(
    creds_file: str,
    connectors: Optional[List[str]] = None,
    dry_run: bool = False,
    examples_dir: Optional[Path] = None,
    mock: bool = False,
) -> List[Dict[str, Any]]:
    """Run smoke tests for specified connectors (async version)."""
    credentials = load_credentials(creds_file)
    results: List[Dict[str, Any]] = []
    
    if mock:
        _install_mock_intercept()
        # In mock mode, harnesses are in {test_dir}/examples (copied by Makefile)
        resolved_examples_dir = Path(__file__).parent / "examples"
    else:
        resolved_examples_dir = examples_dir or _DEFAULT_EXAMPLES_DIR

    test_connectors = connectors or list(credentials.keys())

    print(f"\n{'='*60}")
    print(f"Running smoke tests for {len(test_connectors)} connector(s)")
    if mock:
        print(f"Mode: MOCK (HTTP intercepted, using generated harnesses)")
    print(f"Examples dir: {resolved_examples_dir}")
    print(f"{'='*60}\n")

    for connector_name in test_connectors:
        auth_config_value = credentials.get(connector_name)

        if auth_config_value is None:
            print(f"\n{_bold(f'--- Testing {connector_name} ---')}")
            print(_grey(f"  SKIPPED (not found in credentials file)"))
            results.append({"connector": connector_name, "status": "skipped", "reason": "not_found"})
            continue

        print(f"\n{_bold(f'--- Testing {connector_name} ---')}")

        if isinstance(auth_config_value, list):
            # Multi-instance connector
            for i, instance_auth in enumerate(auth_config_value):
                instance_name = f"{connector_name}[{i + 1}]"
                print(f"  Instance: {instance_name}")

                if not mock and not has_valid_credentials(instance_auth):
                    print(_grey(f"  SKIPPED (placeholder credentials)"))
                    results.append({
                        "connector": instance_name,
                        "status": "skipped",
                        "reason": "placeholder_credentials",
                    })
                    continue

                try:
                    config = _build_connector_config(connector_name, instance_auth)
                except ValueError as e:
                    print(_grey(f"  SKIPPED ({e})"))
                    results.append({"connector": instance_name, "status": "skipped", "reason": str(e)})
                    continue

                result = await test_connector_scenarios(instance_name, config, resolved_examples_dir, dry_run, mock)
                results.append(result)
                _print_result(result)
        else:
            # Single-instance connector
            auth_config = auth_config_value

            if not mock and not has_valid_credentials(auth_config):
                print(_grey(f"  SKIPPED (placeholder credentials)"))
                results.append({
                    "connector": connector_name,
                    "status": "skipped",
                    "reason": "placeholder_credentials",
                })
                continue

            try:
                config = _build_connector_config(connector_name, auth_config)
            except ValueError as e:
                print(_grey(f"  SKIPPED ({e})"))
                results.append({"connector": connector_name, "status": "skipped", "reason": str(e)})
                continue

            result = await test_connector_scenarios(connector_name, config, resolved_examples_dir, dry_run, mock)
            results.append(result)
            _print_result(result)

    return results


def _print_result(result: Dict[str, Any]) -> None:
    """Print per-connector result to stdout."""
    status = result["status"]
    if status == "passed":
        scenarios = result.get("scenarios", {})
        passed_count = sum(1 for d in scenarios.values() if isinstance(d, dict) and d.get("status") == "passed")
        skipped_count = sum(1 for d in scenarios.values() if isinstance(d, dict) and d.get("status") == "skipped")
        not_impl_count = sum(1 for d in scenarios.values() if isinstance(d, dict) and d.get("status") == "not_implemented")
        print(_green(f"  PASSED") + f" ({passed_count} passed, {skipped_count} skipped, {not_impl_count} not implemented)")
        for key, detail in scenarios.items():
            if isinstance(detail, dict):
                if detail.get("status") == "passed":
                    result_data = detail.get("result")
                    if result_data:
                        result_str = str(result_data)
                        print(_green(f"    {key}: ✓") + _grey(f" — {result_str}"))
                    else:
                        print(_green(f"    {key}: ✓"))
                elif detail.get("status") == "skipped":
                    reason = detail.get("reason", "unknown")
                    error_detail = detail.get("detail")
                    if error_detail:
                        print(_yellow(f"    {key}: ~ skipped ({reason})") + _grey(f" — {error_detail}"))
                    else:
                        print(_yellow(f"    {key}: ~ skipped ({reason})"))
                elif detail.get("status") == "not_implemented":
                    print(_grey(f"    {key}: N/A"))
    elif status == "dry_run":
        print(_grey(f"  DRY RUN"))
    elif status == "skipped":
        reason = result.get("scenarios", {}).get("reason") or result.get("reason", "unknown")
        print(_grey(f"  SKIPPED ({reason})"))
    else:
        print(_red(f"  FAILED"))
        scenarios = result.get("scenarios", {})
        for key, detail in scenarios.items():
            if isinstance(detail, dict) and detail.get("status") == "failed":
                msg = detail.get("error") or "unknown error"
                print(_red(f"    {key}: ✗ FAILED — {msg}"))
        if result.get("error"):
            print(_red(f"  Error: {result['error']}"))


def run_tests(
    creds_file: str,
    connectors: Optional[List[str]] = None,
    dry_run: bool = False,
    examples_dir: Optional[Path] = None,
    mock: bool = False,
) -> List[Dict[str, Any]]:
    """Run smoke tests for specified connectors."""
    import asyncio
    return asyncio.run(run_tests_async(creds_file, connectors, dry_run, examples_dir, mock))


def print_summary(results: List[Dict[str, Any]]) -> int:
    """Print test summary and return exit code."""
    print(f"\n{'='*60}")
    print(_bold("TEST SUMMARY"))
    print(f"{'='*60}\n")

    passed = sum(1 for r in results if r["status"] in ("passed", "dry_run"))
    skipped = sum(1 for r in results if r["status"] == "skipped")
    failed = sum(1 for r in results if r["status"] == "failed")
    total = len(results)

    # Count per-scenario statuses across all connectors
    total_flows_passed = 0
    total_flows_skipped = 0
    total_flows_failed = 0
    for r in results:
        for scenario in r.get("scenarios", {}).values():
            if isinstance(scenario, dict):
                status = scenario.get("status")
                if status == "passed":
                    total_flows_passed += 1
                elif status == "skipped":
                    total_flows_skipped += 1
                elif status == "failed":
                    total_flows_failed += 1

    print(f"Total connectors:   {total}")
    print(_green(f"Passed:  {passed}"))
    print(_grey(f"Skipped: {skipped} (placeholder credentials or no examples)"))
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
            if result["status"] == "failed":
                print(_red(f"  - {result['connector']}: {result.get('error', 'see scenarios above')}"))
        print()
        return 1

    if passed == 0 and skipped > 0:
        print(_yellow("All tests skipped (no valid credentials found)"))
        print("Update creds.json with real credentials to run tests")
        return 1

    print(_green("All tests completed successfully!"))
    return 0


def print_performance_summary(results: List[Dict[str, Any]]) -> None:
    """Print performance timing summary table for all executed scenarios."""
    timings = []
    for r in results:
        connector = r.get("connector", "?")
        for key, detail in r.get("scenarios", {}).items():
            if isinstance(detail, dict) and "duration_ms" in detail:
                timings.append((connector, key, detail["duration_ms"], detail.get("status", "?")))

    if not timings:
        return

    print(f"\n{'═' * 60}")
    print(_bold("PERFORMANCE SUMMARY"))
    print(f"{'═' * 60}\n")

    print(f"  {'Connector':<20} {'Flow':<30} {'Duration':>10}  {'Status'}")
    print(f"  {'─' * 20} {'─' * 30} {'─' * 10}  {'─' * 10}")

    for connector, flow, dur, status in timings:
        color_fn = _green if status == "passed" else (_yellow if status == "skipped" else _grey)
        print(f"  {connector:<20} {flow:<30} {dur:>8.1f}ms  {color_fn(status)}")

    executed = [(c, f, d, s) for c, f, d, s in timings if s in ("passed", "skipped", "failed")]
    if executed:
        durations = [d for _, _, d, _ in executed]
        print(f"\n  Executed: {len(executed)} flows")
        print(f"  Total:   {sum(durations):.1f}ms")
        print(f"  Average: {sum(durations) / len(durations):.1f}ms")
        print(f"  Min:     {min(durations):.1f}ms  ({next(f for _, f, d, _ in executed if d == min(durations))})")
        print(f"  Max:     {max(durations):.1f}ms  ({next(f for _, f, d, _ in executed if d == max(durations))})")

    # FFI breakdown from connector client perf log
    try:
        from payments.connector_client import get_perf_log, clear_perf_log
        perf = get_perf_log()
        if perf:
            print(f"\n{'═' * 60}")
            print(_bold("FFI OVERHEAD BREAKDOWN"))
            print(f"{'═' * 60}\n")
            print(f"  {'Flow':<30} {'req_ffi':>10} {'HTTP':>10} {'res_ffi':>10} {'Overhead':>10} {'Total':>10}")
            print(f"  {'─' * 30} {'─' * 10} {'─' * 10} {'─' * 10} {'─' * 10} {'─' * 10}")
            total_req = total_http = total_res = 0.0
            for e in perf:
                overhead = e['req_ffi_ms'] + e['res_ffi_ms']
                total_req += e['req_ffi_ms']
                total_http += e['http_ms']
                total_res += e['res_ffi_ms']
                print(f"  {e['flow']:<30} {e['req_ffi_ms']:>8.2f}ms {e['http_ms']:>8.2f}ms {e['res_ffi_ms']:>8.2f}ms {overhead:>8.2f}ms {e['total_ms']:>8.2f}ms")
            n = len(perf)
            total_overhead = total_req + total_res
            total_all = total_req + total_http + total_res
            pct = (total_overhead / total_all * 100) if total_all > 0 else 0
            print(f"\n  Average req_ffi:  {total_req/n:.2f}ms")
            print(f"  Average res_ffi:  {total_res/n:.2f}ms")
            print(f"  Average overhead: {total_overhead/n:.2f}ms ({pct:.1f}% of total)")
            # Write perf data for cross-SDK comparison
            try:
                perf_dir = Path("/tmp/sdk-perf")
                perf_dir.mkdir(exist_ok=True)
                (perf_dir / "python.json").write_text(json.dumps({"sdk": "Python", "flows": perf}))
            except Exception:
                pass
            clear_perf_log()
    except ImportError:
        pass
    print()


def main():
    parser = argparse.ArgumentParser(
        description="Multi-connector smoke test for hyperswitch-payments SDK"
    )
    parser.add_argument(
        "--creds-file",
        default="creds.json",
        help="Path to connector credentials JSON file (default: creds.json)",
    )
    parser.add_argument(
        "--connectors",
        type=str,
        help="Comma-separated list of connectors to test (e.g., stripe,adyen)",
    )
    parser.add_argument(
        "--all",
        action="store_true",
        help="Test all connectors in the credentials file",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Skip HTTP calls; verify only that example files load correctly",
    )
    parser.add_argument(
        "--examples-dir",
        type=str,
        default=None,
        help="Path to the examples/ directory (default: auto-detected from repo root)",
    )
    parser.add_argument(
        "--mock",
        action="store_true",
        help="Intercept HTTP; verify req_transformer only. Uses generated/ harnesses.",
    )
    parser.add_argument(
        "--json-output",
        action="store_true",
        help="Output results as JSON for programmatic consumption",
    )

    args = parser.parse_args()

    if not args.all and not args.connectors:
        parser.error("Must specify either --all or --connectors")

    connectors = None
    if args.connectors:
        connectors = [c.strip() for c in args.connectors.split(",")]

    examples_dir = Path(args.examples_dir) if args.examples_dir else None

    try:
        results = run_tests(args.creds_file, connectors, args.dry_run, examples_dir, args.mock)
        
        if args.json_output:
            # Output results as JSON for programmatic consumption
            print(json.dumps(results, indent=2, default=str))
            sys.exit(0)
        else:
            exit_code = print_summary(results)
            print_performance_summary(results)
            sys.exit(exit_code)
    except Exception as e:
        if args.json_output:
            print(json.dumps({"error": str(e)}))
        else:
            print(f"\nFatal error: {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()
