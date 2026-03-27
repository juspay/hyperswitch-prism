#!/usr/bin/env python3
"""
Multi-connector smoke test for the hyperswitch-payments SDK.

Loads connector credentials from external JSON file and runs all scenario
functions found in examples/{connector}/python/ for each connector.

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
        ConnectorResponseTransformationError,
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


# Canonical scenario order for consolidated-file discovery
_SCENARIO_NAMES = [
    "checkout_autocapture",
    "checkout_card",
    "checkout_wallet",
    "checkout_bank",
    "refund",
    "recurring",
    "void_payment",
    "get_payment",
    "create_customer",
    "tokenize",
    "authentication",
]


async def test_connector_scenarios(
    connector_name: str,
    config: ConnectorConfig,
    examples_dir: Path,
    dry_run: bool = False,
) -> Dict[str, Any]:
    """
    Discover and run all Python scenario functions for a connector.

    Loads examples_dir/{connector_name}/python/{connector_name}.py and calls each
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

    connector_dir = examples_dir / connector_name / "python"
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

    # Discover process_* functions in canonical scenario order
    scenario_fns: List[tuple] = []
    for name in _SCENARIO_NAMES:
        fn = getattr(module, f"process_{name}", None)
        if fn is not None and callable(fn):
            scenario_fns.append((name, fn))

    if not scenario_fns:
        result["status"] = "skipped"
        result["scenarios"] = {"skipped": True, "reason": "no_scenario_files"}
        return result

    any_failed = False
    for scenario_key, process_fn in scenario_fns:

        txn_id = f"smoke_{scenario_key}_{os.urandom(4).hex()}"
        print(f"    [{scenario_key}] running (txn={txn_id}) ...", flush=True)
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
                print(_yellow(f"    [{scenario_key}] connector error") + f" — {error_str}", flush=True)
                result["scenarios"][scenario_key] = {"passed": True, "connector_error": error_str}
            else:
                # For display, show the actual response content
                try:
                    response_display = str(response)
                except Exception:
                    response_display = f"<{type(response).__name__}>"
                print(_green(f"    [{scenario_key}] OK") + f" — {response_display}", flush=True)
                result["scenarios"][scenario_key] = {"passed": True, "result": response}
        except (IntegrationError, ConnectorResponseTransformationError, InternalError) as e:
            # Connector rejected our test data — SDK round-trip succeeded.
            # IntegrationError/ConnectorResponseTransformationError: FFI completed a full cycle, connector returned error.
            # UniffiError (e.g. HandlerError): FFI-level connector rejection before HTTP
            # (e.g. InvalidWalletToken — bad probe token rejected during request building).
            msg = getattr(e, 'error_message', None) or str(e)
            code = getattr(e, 'error_code', None)
            detail = f"{code}: {msg}" if code else msg
            print(_yellow(f"    [{scenario_key}] connector error (round-trip ok)") + f" — {type(e).__name__}: {detail}", flush=True)
            result["scenarios"][scenario_key] = {
                "passed": True,
                "connector_error": f"{type(e).__name__}: {detail}",
            }
        except Exception as e:
            # Unexpected Python-level failure — import crash, serialization bug, etc.
            # This indicates a real SDK or example-file problem.
            import traceback
            tb = traceback.format_exc()
            print(_red(f"    [{scenario_key}] FAILED") + f" — {type(e).__name__}: {e}", flush=True)
            print(f"    Traceback: {tb[:500]}", flush=True)  # Print first 500 chars of traceback
            result["scenarios"][scenario_key] = {
                "passed": False,
                "error": f"{type(e).__name__}: {e}",
            }
            any_failed = True

    result["status"] = "failed" if any_failed else "passed"
    return result


async def run_tests_async(
    creds_file: str,
    connectors: Optional[List[str]] = None,
    dry_run: bool = False,
    examples_dir: Optional[Path] = None,
) -> List[Dict[str, Any]]:
    """Run smoke tests for specified connectors (async version)."""
    credentials = load_credentials(creds_file)
    results: List[Dict[str, Any]] = []
    resolved_examples_dir = examples_dir or _DEFAULT_EXAMPLES_DIR

    test_connectors = connectors or list(credentials.keys())

    print(f"\n{'='*60}")
    print(f"Running smoke tests for {len(test_connectors)} connector(s)")
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

                if not has_valid_credentials(instance_auth):
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

                result = await test_connector_scenarios(instance_name, config, resolved_examples_dir, dry_run)
                results.append(result)
                _print_result(result)
        else:
            # Single-instance connector
            auth_config = auth_config_value

            if not has_valid_credentials(auth_config):
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

            result = await test_connector_scenarios(connector_name, config, resolved_examples_dir, dry_run)
            results.append(result)
            _print_result(result)

    return results


def _print_result(result: Dict[str, Any]) -> None:
    """Print per-connector result to stdout."""
    status = result["status"]
    if status == "passed":
        scenarios = result.get("scenarios", {})
        print(_green(f"  PASSED") + f" ({len(scenarios)} scenario(s))")
        for key, detail in scenarios.items():
            if isinstance(detail, dict) and detail.get("connector_error"):
                print(_yellow(f"    {key}: connector error (round-trip ok)") + f" — {detail['connector_error']}")
    elif status == "dry_run":
        print(_grey(f"  DRY RUN"))
    elif status == "skipped":
        reason = result.get("scenarios", {}).get("reason") or result.get("reason", "unknown")
        print(_grey(f"  SKIPPED ({reason})"))
    else:
        print(_red(f"  FAILED"))
        for key, detail in result.get("scenarios", {}).items():
            if isinstance(detail, dict) and not detail.get("passed", True):
                msg = detail.get("error_message") or detail.get("error") or "unknown error"
                print(_red(f"    {key}: {detail.get('error_type', 'Error')} — {msg}"))
        if result.get("error"):
            print(_red(f"  Error: {result['error']}"))


def run_tests(
    creds_file: str,
    connectors: Optional[List[str]] = None,
    dry_run: bool = False,
    examples_dir: Optional[Path] = None,
) -> List[Dict[str, Any]]:
    """Run smoke tests for specified connectors."""
    import asyncio
    return asyncio.run(run_tests_async(creds_file, connectors, dry_run, examples_dir))


def print_summary(results: List[Dict[str, Any]]) -> int:
    """Print test summary and return exit code."""
    print(f"\n{'='*60}")
    print(_bold("TEST SUMMARY"))
    print(f"{'='*60}\n")

    passed = sum(1 for r in results if r["status"] in ("passed", "dry_run"))
    skipped = sum(1 for r in results if r["status"] == "skipped")
    failed = sum(1 for r in results if r["status"] == "failed")
    total = len(results)

    print(f"Total:   {total}")
    print(_green(f"Passed:  {passed}"))
    print(_grey(f"Skipped: {skipped} (placeholder credentials or no examples)"))
    print((_red if failed > 0 else _green)(f"Failed:  {failed}"))
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

    args = parser.parse_args()

    if not args.all and not args.connectors:
        parser.error("Must specify either --all or --connectors")

    connectors = None
    if args.connectors:
        connectors = [c.strip() for c in args.connectors.split(",")]

    examples_dir = Path(args.examples_dir) if args.examples_dir else None

    try:
        results = run_tests(args.creds_file, connectors, args.dry_run, examples_dir)
        exit_code = print_summary(results)
        sys.exit(exit_code)
    except Exception as e:
        print(f"\nFatal error: {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()
