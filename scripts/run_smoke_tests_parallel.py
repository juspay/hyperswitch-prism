#!/usr/bin/env python3
"""
Parallel smoke test runner for all SDKs.

Runs smoke tests for Python, JavaScript, Kotlin, and Rust SDKs in parallel,
aggregates results, and displays them in a table format.

Strategy: one batch task per SDK (all connectors at once), all SDKs in parallel.
This avoids concurrent Gradle/Cargo lock contention and eliminates per-connector
npm install overhead.

Usage:
    python3 scripts/run_smoke_tests_parallel.py --connectors stripe,adyen
    python3 scripts/run_smoke_tests_parallel.py --all
    python3 scripts/run_smoke_tests_parallel.py --all --mock
"""

import argparse
import json
import os
import platform
import re
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, List, Optional, Tuple
from concurrent.futures import ThreadPoolExecutor, as_completed
import time
import tempfile
import shutil

# ANSI color helpers
NO_COLOR = not sys.stdout.isatty() or os.environ.get("NO_COLOR")

def c(code: str, text: str) -> str:
    return text if NO_COLOR else f"\033[{code}m{text}\033[0m"

def green(t: str) -> str:
    return c("32", t)

def yellow(t: str) -> str:
    return c("33", t)

def red(t: str) -> str:
    return c("31", t)

def grey(t: str) -> str:
    return c("90", t)

def bold(t: str) -> str:
    return c("1", t)


@dataclass
class FlowResult:
    """Result for a single flow."""
    flow: str
    status: str  # passed, skipped, failed, not_implemented
    detail: Optional[str] = None


@dataclass
class SDKResult:
    """Result for a single SDK × connector pair."""
    sdk: str
    connector: str
    status: str  # passed, failed, error
    flows: List[FlowResult] = field(default_factory=list)
    error: Optional[str] = None


# ── One-time build caches ────────────────────────────────────────────────────

_js_sdk_path: Optional[Path] = None
_kotlin_sdk_prepared: bool = False
_shared_js_env_dir: Optional[Path] = None


def get_all_connectors(repo_root: Path) -> List[str]:
    """Discover all available connectors from the examples directory."""
    examples_dir = repo_root / "examples"
    if not examples_dir.exists():
        return ["stripe"]
    return sorted(
        d.name for d in examples_dir.iterdir()
        if d.is_dir() and not d.name.startswith(".")
    )


def get_platform_triple() -> str:
    """Return the Rust target triple for the current platform."""
    uname = platform.uname()
    if uname.system == "Darwin":
        return "aarch64-apple-darwin" if uname.machine == "arm64" else "x86_64-apple-darwin"
    else:
        return "aarch64-unknown-linux-gnu" if uname.machine == "aarch64" else "x86_64-unknown-linux-gnu"


def get_ffi_lib_path(repo_root: Path) -> Path:
    """Return the expected FFI library path for the current platform."""
    triple = get_platform_triple()
    ext = "dylib" if platform.uname().system == "Darwin" else "so"
    return repo_root / "target" / triple / "release-fast" / f"libconnector_service_ffi.{ext}"


def get_js_sdk_version(repo_root: Path) -> str:
    """Read JavaScript SDK version from package.json."""
    pkg_json = repo_root / "sdk" / "javascript" / "package.json"
    if pkg_json.exists():
        import json
        data = json.loads(pkg_json.read_text())
        return data.get("version", "0.0.1")
    return "0.0.1"


def build_javascript_sdk_once(repo_root: Path) -> Optional[Path]:
    """Build JavaScript SDK once and return the path to the tarball."""
    global _js_sdk_path
    if _js_sdk_path is not None:
        return _js_sdk_path

    version = get_js_sdk_version(repo_root)
    tarball = repo_root / "artifacts" / "sdk-javascript" / f"hyperswitch-prism-{version}.tgz"
    if tarball.exists():
        _js_sdk_path = tarball
        return _js_sdk_path

    print("  Building JavaScript SDK (once)...")
    try:
        result = subprocess.run(
            ["make", "pack-archive"],
            cwd=repo_root / "sdk" / "javascript",
            capture_output=True,
            text=True,
            timeout=300
        )
        if result.returncode == 0 and tarball.exists():
            _js_sdk_path = tarball
            return _js_sdk_path
    except Exception as e:
        print(f"  Warning: Failed to build JavaScript SDK: {e}")

    return None


def prepare_kotlin_sdk_once(repo_root: Path) -> bool:
    """
    Run `make generate-all install` in sdk/java once.
    This builds the FFI lib, generates Kotlin bindings, and publishes the SDK
    to Maven local — the prerequisite for running Kotlin smoke tests.
    """
    global _kotlin_sdk_prepared
    if _kotlin_sdk_prepared:
        return True

    lib = get_ffi_lib_path(repo_root)
    if lib.exists():
        print(f"  FFI library: {lib.name} (cached)")
    else:
        print("  FFI library not found — will build via Kotlin SDK prep")

    print("  Preparing Kotlin SDK (generate-all install)...")
    try:
        result = subprocess.run(
            # Pass PROFILE=release-fast explicitly so this always matches the
            # pre-built artifacts (CI env may carry PROFILE=dev from the job env).
            ["make", "PROFILE=release-fast", "generate-all", "install"],
            cwd=repo_root / "sdk" / "java",
            capture_output=True,
            text=True,
            timeout=600
        )
        if result.returncode == 0:
            _kotlin_sdk_prepared = True
            print("  Kotlin SDK ready")
            return True
        print(f"  Warning: Kotlin SDK prep failed (rc={result.returncode})")
        if result.stderr:
            print(f"  {result.stderr[-300:]}")
    except Exception as e:
        print(f"  Warning: Kotlin SDK prep exception: {e}")

    return False


_rust_smoke_test_prepared: bool = False
_rust_valid_connectors: List[str] = []


def get_valid_rust_connectors(repo_root: Path, connectors: List[str]) -> List[str]:
    """
    Return only connectors whose Rust examples pass build.rs validation:
    every flow in SUPPORTED_FLOWS must have a matching pub async fn process_<flow>.
    """
    examples_dir = repo_root / "examples"
    valid = []
    for connector in connectors:
        rs_file = examples_dir / connector / f"{connector}.rs"
        if not rs_file.exists():
            continue
        content = rs_file.read_text()

        marker = "pub const SUPPORTED_FLOWS: &[&str] = &["
        start = content.find(marker)
        if start == -1:
            continue  # build.rs skips connectors without SUPPORTED_FLOWS

        after = content[start + len(marker):]
        end_idx = after.find("];")
        if end_idx == -1:
            end_idx = after.find("]")
        flows = [
            f.strip().strip('"')
            for f in after[:end_idx].split(",")
            if f.strip().strip('"')
        ]

        if all(f"pub async fn process_{flow}(" in content for flow in flows) and flows:
            valid.append(connector)

    return valid


def prepare_rust_smoke_test_once(repo_root: Path, connectors: List[str]) -> bool:
    """
    Build the Rust smoke-test binary with all valid connectors compiled in.
    Connectors whose harnesses fail build.rs validation are silently excluded.
    """
    global _rust_smoke_test_prepared, _rust_valid_connectors

    # Check if we need to rebuild (connector list changed from cached)
    if _rust_smoke_test_prepared and _rust_valid_connectors:
        current_set = set(connectors)
        cached_set = set(_rust_valid_connectors)
        if current_set != cached_set:
            print(f"  Connector list changed ({len(cached_set)} → {len(current_set)} connectors), rebuilding...")
            _rust_smoke_test_prepared = False
            _rust_valid_connectors = []
        else:
            return True

    valid = get_valid_rust_connectors(repo_root, connectors)
    if not valid:
        print("  Warning: no valid Rust harnesses found, skipping Rust build")
        return False

    skipped = set(connectors) - set(valid)
    if skipped:
        print(f"  Skipping Rust harnesses with coverage errors: {', '.join(sorted(skipped))}")

    print(f"  Building Rust smoke-test with CONNECTORS={','.join(valid)}...")
    env = os.environ.copy()
    env["CONNECTORS"] = ",".join(valid)
    env["HARNESS_DIR"] = str(repo_root / "examples")

    try:
        result = subprocess.run(
            # Build only the FFI smoke-test binary; skip grpc-smoke-test which may
            # have unrelated compile errors (different binary target, same package).
            # Use --target to match the platform triple used everywhere else so that
            # Cargo reuses already-compiled artifacts from the explicit build step.
            ["cargo", "build", "--profile", "release-fast",
             "--target", get_platform_triple(),
             "-p", "hyperswitch-smoke-test", "--bin", "hyperswitch-smoke-test"],
            cwd=repo_root,
            capture_output=True, text=True, timeout=300, env=env
        )
        if result.returncode == 0:
            _rust_smoke_test_prepared = True
            _rust_valid_connectors = valid
            print(f"  Rust smoke-test ready ({len(valid)} connectors)")
            return True
        print(f"  Warning: Rust smoke-test build failed (rc={result.returncode})")
        if result.stderr:
            print(f"  {result.stderr[-300:]}")
    except Exception as e:
        print(f"  Warning: Rust smoke-test build exception: {e}")

    return False



def setup_shared_js_env(
    repo_root: Path, tarball: Path, connectors: List[str]
) -> Optional[Path]:
    """
    Install node_modules once into a shared tmpdir, copy all harnesses.
    Reused by run_javascript_test_batch to avoid per-connector npm install.
    """
    global _shared_js_env_dir
    if _shared_js_env_dir is not None:
        return _shared_js_env_dir

    print("  Setting up shared JS environment (npm install once)...")
    tmpdir = Path(tempfile.mkdtemp(prefix="hs-js-env-"))

    try:
        subprocess.run(
            ["npm", "init", "-y", "--silent"],
            cwd=tmpdir, capture_output=True, timeout=30
        )
        result = subprocess.run(
            ["npm", "install", str(tarball),
             "typescript", "@types/node", "tsx", "--silent"],
            cwd=tmpdir, capture_output=True, timeout=120
        )
        if result.returncode != 0:
            print("  Warning: npm install failed")
            shutil.rmtree(tmpdir, ignore_errors=True)
            return None

        # Copy test infrastructure
        shutil.copy(
            repo_root / "sdk" / "javascript" / "smoke-test" / "test_smoke.ts", tmpdir
        )
        shutil.copy(
            repo_root / "sdk" / "javascript" / "smoke-test" / "tsconfig.json", tmpdir
        )
        shutil.copy(repo_root / "sdk" / "generated" / "flows.json", tmpdir)

        creds_src = repo_root / "creds.json"
        if creds_src.exists():
            shutil.copy(creds_src, tmpdir / "creds.json")

        # Copy all examples
        for connector in connectors:
            example_src = repo_root / "examples" / connector / f"{connector}.ts"
            if example_src.exists():
                example_dir = tmpdir / "examples" / connector
                example_dir.mkdir(parents=True, exist_ok=True)
                shutil.copy(example_src, example_dir / f"{connector}.ts")

        _shared_js_env_dir = tmpdir
        print(f"  JS environment ready")
        return _shared_js_env_dir

    except Exception as e:
        print(f"  Warning: JS env setup failed: {e}")
        shutil.rmtree(tmpdir, ignore_errors=True)
        return None


# ── Output parsers ───────────────────────────────────────────────────────────

def parse_json_result(sdk: str, connector: str, data: List[Dict]) -> SDKResult:
    """Parse JSON result from Python smoke test (single connector)."""
    flows = []

    if not data or not isinstance(data, list):
        return SDKResult(sdk, connector, "error", error="invalid JSON structure")

    for conn_result in data:
        if not isinstance(conn_result, dict):
            continue

        scenarios = conn_result.get("scenarios", {})
        if not isinstance(scenarios, dict):
            continue

        for flow_name, flow_data in scenarios.items():
            if not isinstance(flow_data, dict):
                continue

            status = flow_data.get("status", "unknown")

            if status == "passed":
                flows.append(FlowResult(flow_name, "passed"))
            elif status == "skipped":
                flows.append(FlowResult(flow_name, "skipped"))
            elif status == "failed":
                detail = flow_data.get("error", "unknown error")
                flows.append(FlowResult(flow_name, "failed", detail))
            elif status == "not_implemented":
                flows.append(FlowResult(flow_name, "not_implemented"))

    if any(f.status == "failed" for f in flows):
        status = "failed"
    elif any(f.status == "passed" for f in flows):
        status = "passed"
    else:
        status = "skipped"

    return SDKResult(sdk, connector, status, flows)


def parse_text_output(sdk: str, connector: str, output: str) -> SDKResult:
    """Parse text output from JS/Kotlin/Rust smoke tests for a single connector section."""
    flows = []
    in_summary = False

    lines = output.split('\n')
    for i, line in enumerate(lines):
        line_stripped = line.strip()

        if line_stripped.startswith('[') and ']' in line_stripped and not in_summary:
            flow_end = line_stripped.find(']')
            if flow_end > 1:
                flow_name = line_stripped[1:flow_end]
                rest = line_stripped[flow_end+1:].strip()

                if 'NOT IMPLEMENTED' in rest.upper():
                    flows.append(FlowResult(flow_name, "not_implemented"))
                elif 'running' in rest.lower():
                    # Check for pass/fail status - but ignore status messages like "Status: MANDATE_REVOKE_FAILED"
                    # which contain "FAILED" but aren't actual test failures
                    is_status_message = 'status:' in rest.lower()
                    
                    if 'PASSED' in rest or 'MOCK VERIFIED' in rest:
                        flows.append(FlowResult(flow_name, "passed"))
                    elif 'SKIPPED' in rest:
                        flows.append(FlowResult(flow_name, "skipped"))
                    elif 'FAILED' in rest and not is_status_message:
                        detail = rest.split('—', 1)[1].strip() if '—' in rest else None
                        flows.append(FlowResult(flow_name, "failed", detail))
                    elif i + 1 < len(lines):
                        next_line = lines[i + 1].strip()
                        if 'PASSED' in next_line or 'MOCK VERIFIED' in next_line:
                            flows.append(FlowResult(flow_name, "passed"))
                        elif 'SKIPPED' in next_line:
                            flows.append(FlowResult(flow_name, "skipped"))
                        elif 'FAILED' in next_line and 'status:' not in next_line.lower():
                            detail = next_line.split('—', 1)[1].strip() if '—' in next_line else None
                            flows.append(FlowResult(flow_name, "failed", detail))

        elif 'passed' in line_stripped.lower() and 'skipped' in line_stripped.lower() and (
            'passed,' in line_stripped.lower() or 'passed)' in line_stripped.lower()
        ):
            in_summary = True

        elif in_summary and ': ' in line_stripped:
            clean_line = re.sub(r'\x1b\[[0-9;]*m', '', line_stripped)
            parts = clean_line.split(': ', 1)
            if len(parts) == 2:
                flow_name = parts[0].strip()
                status_indicator = parts[1].strip()

                if not any(f.flow == flow_name for f in flows):
                    if '✓' in status_indicator or '✔' in status_indicator:
                        flows.append(FlowResult(flow_name, "passed"))
                    elif status_indicator == 'N/A':
                        flows.append(FlowResult(flow_name, "not_implemented"))

    if any(f.status == "failed" for f in flows):
        status = "failed"
    elif any(f.status == "passed" for f in flows):
        status = "passed"
    else:
        status = "skipped"

    return SDKResult(sdk, connector, status, flows)


def parse_multi_connector_text_output(
    sdk: str, connectors: List[str], output: str
) -> List[SDKResult]:
    """
    Split multi-connector output by '--- Testing <connector> ---' boundaries
    and parse each section individually.
    """
    # Strip ANSI escape codes before splitting so color formatting doesn't interfere
    clean = re.sub(r'\x1b\[[0-9;]*m', '', output)
    sections = re.split(r'--- Testing (\w+) ---', clean)

    connector_outputs: Dict[str, str] = {}
    for i in range(1, len(sections), 2):
        if i + 1 < len(sections):
            connector_outputs[sections[i]] = sections[i + 1]

    results = []
    for connector in connectors:
        if connector in connector_outputs:
            results.append(parse_text_output(sdk, connector, connector_outputs[connector]))
        else:
            results.append(SDKResult(sdk, connector, "error", error="no output section found"))
    return results


# ── Batch SDK runners (one call per SDK for all connectors) ──────────────────

def run_python_test_batch(
    repo_root: Path, connectors: List[str], mock: bool
) -> List[SDKResult]:
    """Run Python smoke test for all connectors at once, parse JSON output."""
    with tempfile.TemporaryDirectory() as tmpdir:
        tmpdir_path = Path(tmpdir)

        smoke_test_dir = tmpdir_path / "smoke-test"
        smoke_test_dir.mkdir(parents=True, exist_ok=True)
        shutil.copy(
            repo_root / "sdk" / "python" / "smoke-test" / "test_smoke.py",
            smoke_test_dir
        )
        shutil.copy(
            repo_root / "creds.json",
            tmpdir_path / "creds.json"
        )

        generated_dir = tmpdir_path / "generated"
        generated_dir.mkdir()
        shutil.copy(
            repo_root / "sdk" / "generated" / "flows.json",
            generated_dir / "flows.json"
        )

        for connector in connectors:
            examples_dest = smoke_test_dir / "examples" / connector
            examples_dest.mkdir(parents=True, exist_ok=True)
            example_src = repo_root / "examples" / connector / f"{connector}.py"
            if example_src.exists():
                shutil.copy(example_src, examples_dest / f"{connector}.py")

        cmd = [
            sys.executable, "smoke-test/test_smoke.py",
            "--connectors", ",".join(connectors),
            "--json-output",
            "--creds-file", "creds.json",
            "--examples-dir", "smoke-test/examples",
        ]
        if mock:
            cmd.append("--mock")

        env = os.environ.copy()
        env["PYTHONPATH"] = str(repo_root / "sdk" / "python" / "src")
        env["NO_COLOR"] = "1"

        try:
            result = subprocess.run(
                cmd, cwd=tmpdir_path,
                capture_output=True, text=True, timeout=300, env=env
            )

            # Verbose mode: show full stdout/stderr
            if hasattr(__builtins__, '_VERBOSE') and __builtins__._VERBOSE:
                if result.stdout:
                    print(f"\n  [Python stdout]\n{result.stdout[-10000:]}")
                if result.stderr:
                    print(f"\n  [Python stderr]\n{result.stderr[-10000:]}")

            # Find where the JSON array starts (may be multi-line pretty-printed output).
            # Flow markers look like "[authorize] running..." — we only want the bare
            # JSON array starter: a line that is exactly "[" or starts with "[{".
            lines = result.stdout.split('\n')
            json_start = None
            for i, line in enumerate(lines):
                stripped = line.strip()
                if stripped == '[' or stripped.startswith('[{'):
                    json_start = i
                    break

            if json_start is not None:
                json_text = '\n'.join(lines[json_start:]).strip()
                try:
                    data = json.loads(json_text)
                    if isinstance(data, list):
                        return [
                            parse_json_result(
                                "python",
                                conn_result.get("connector", "unknown"),
                                [conn_result]
                            )
                            for conn_result in data
                        ]
                except json.JSONDecodeError:
                    pass

            return [
                SDKResult("python", c, "error", error="JSON parse failed")
                for c in connectors
            ]

        except subprocess.TimeoutExpired:
            return [SDKResult("python", c, "error", error="timeout") for c in connectors]
        except Exception as e:
            return [SDKResult("python", c, "error", error=str(e)[:200]) for c in connectors]


def run_javascript_test_batch(
    connectors: List[str], mock: bool, js_env: Path
) -> List[SDKResult]:
    """Run JS smoke test for all connectors using a pre-installed shared node_modules."""
    env = os.environ.copy()
    env["NODE_PATH"] = str(js_env / "node_modules")
    env["NO_COLOR"] = "1"

    examples_dir = js_env / "examples"
    cmd = [
        "npx", "tsx", "test_smoke.ts",
        "--connectors", ",".join(connectors),
        "--creds-file", "creds.json",
        "--examples-dir", str(examples_dir),
    ]
    if mock:
        cmd.append("--mock")

    try:
        result = subprocess.run(
            cmd, cwd=js_env,
            capture_output=True, text=True, timeout=300, env=env
        )

        # Verbose mode: show full stdout/stderr
        if hasattr(__builtins__, '_VERBOSE') and __builtins__._VERBOSE:
            if result.stdout:
                print(f"\n  [JavaScript stdout]\n{result.stdout[-10000:]}")
            if result.stderr:
                print(f"\n  [JavaScript stderr]\n{result.stderr[-10000:]}")

        return parse_multi_connector_text_output(
            "javascript", connectors, result.stdout + result.stderr
        )
    except subprocess.TimeoutExpired:
        return [SDKResult("javascript", c, "error", error="timeout") for c in connectors]
    except Exception as e:
        return [SDKResult("javascript", c, "error", error=str(e)[:200]) for c in connectors]


def run_kotlin_test_batch(
    repo_root: Path, connectors: List[str], mock: bool
) -> List[SDKResult]:
    """
    Run all connectors in a single Kotlin JVM process.
    Requires prepare_kotlin_sdk_once() and copy_kotlin_harnesses() to have run first.
    """
    smoke_test_dir = repo_root / "sdk" / "java" / "smoke-test"

    creds_src = repo_root / "creds.json"
    if creds_src.exists():
        shutil.copy(creds_src, smoke_test_dir / "creds.json")

    run_args = f"--connectors {','.join(connectors)}"
    if mock:
        run_args += " --mock"

    env = os.environ.copy()
    env["NO_COLOR"] = "1"

    try:
        result = subprocess.run(
            ["../gradlew", "run", f"--args={run_args}"],
            cwd=str(smoke_test_dir),
            capture_output=True, text=True, timeout=300, env=env
        )

        # Verbose mode: show full stdout/stderr
        if hasattr(__builtins__, '_VERBOSE') and __builtins__._VERBOSE:
            if result.stdout:
                print(f"\n  [Kotlin stdout]\n{result.stdout[-10000:]}")
            if result.stderr:
                print(f"\n  [Kotlin stderr]\n{result.stderr[-10000:]}")

        return parse_multi_connector_text_output(
            "kotlin", connectors, result.stdout + result.stderr
        )
    except subprocess.TimeoutExpired:
        return [SDKResult("kotlin", c, "error", error="timeout") for c in connectors]
    except Exception as e:
        return [SDKResult("kotlin", c, "error", error=str(e)[:200]) for c in connectors]


def run_rust_test_batch(
    repo_root: Path, connectors: List[str], mock: bool
) -> List[SDKResult]:
    """
    Run all valid connectors in a single Rust process.
    Binary is pre-built by prepare_rust_smoke_test_once() with valid connectors compiled in.
    Connectors excluded from the binary (bad harnesses) are reported as errors.
    """
    # Resolve which connectors were actually compiled in
    valid = _rust_valid_connectors if _rust_valid_connectors else connectors
    invalid_connectors = [c for c in connectors if c not in valid]

    # Use the pre-built binary directly to avoid any cargo lock during parallel tests.
    # Check both locations: with and without platform subdirectory (CI uses --target flag)
    binary_no_platform = repo_root / "target" / "release-fast" / "hyperswitch-smoke-test"
    
    # Look for binary in any platform-specific subdirectory
    binary_with_platform = None
    target_dir = repo_root / "target"
    if target_dir.exists():
        for platform_dir in target_dir.iterdir():
            if platform_dir.is_dir() and "-" in platform_dir.name:  # Looks like a target triple
                candidate = platform_dir / "release-fast" / "hyperswitch-smoke-test"
                if candidate.exists():
                    binary_with_platform = candidate
                    break
    
    binary = binary_with_platform if binary_with_platform else binary_no_platform

    if not binary.exists():
        # Fall back to cargo run if binary not found. Use --target to match the
        # pre-built artifact directory so Cargo reuses compiled objects.
        binary_cmd = ["cargo", "run", "--profile", "release-fast",
                      "--target", get_platform_triple(),
                      "-p", "hyperswitch-smoke-test", "--bin", "hyperswitch-smoke-test", "--"]
    else:
        binary_cmd = [str(binary)]

    # Connectors excluded from the build are reported as errors immediately
    error_results = [
        SDKResult("rust", c, "error", error="harness coverage error (skipped from build)")
        for c in invalid_connectors
    ]

    if not valid:
        return error_results

    cmd = binary_cmd + [
        "--creds-file", str(repo_root / "creds.json"),
        "--connectors", ",".join(valid),
    ]
    if mock:
        cmd.append("--mock")

    env = os.environ.copy()
    env["HARNESS_DIR"] = str(repo_root / "examples")
    env["CONNECTORS"] = ",".join(valid)
    env["NO_COLOR"] = "1"

    try:
        result = subprocess.run(
            cmd, cwd=repo_root,
            capture_output=True, text=True, timeout=300, env=env
        )

        # Verbose mode: show full stdout/stderr
        if hasattr(__builtins__, '_VERBOSE') and __builtins__._VERBOSE:
            if result.stdout:
                print(f"\n  [Rust stdout]\n{result.stdout[-10000:]}")
            if result.stderr:
                print(f"\n  [Rust stderr]\n{result.stderr[-10000:]}")

        valid_results = parse_multi_connector_text_output(
            "rust", valid, result.stdout + result.stderr
        )
        return valid_results + error_results
    except subprocess.TimeoutExpired:
        return [SDKResult("rust", c, "error", error="timeout") for c in valid] + error_results
    except Exception as e:
        return [SDKResult("rust", c, "error", error=str(e)[:200]) for c in valid] + error_results


# ── Results table ────────────────────────────────────────────────────────────

def print_results_table(results: List[SDKResult], sdks: List[str]) -> None:
    """Print results in a clean table format."""
    by_connector: Dict[str, List[SDKResult]] = {}
    for r in results:
        if r.connector not in by_connector:
            by_connector[r.connector] = []
        by_connector[r.connector].append(r)

    all_flows = set()
    for r in results:
        for f in r.flows:
            all_flows.add(f.flow)
    all_flows_sorted = sorted(all_flows)

    flow_col_width = max((len(f) for f in all_flows_sorted), default=20) + 2
    sdk_col_width = 16

    print("\n" + "=" * 100)
    print(bold("PARALLEL SMOKE TEST RESULTS"))
    print("=" * 100)

    for connector, sdk_results in sorted(by_connector.items()):
        print(f"\n{bold(connector.upper())}")
        print("-" * 120)

        header = f"{'Flow':<{flow_col_width}}"
        for sdk in sdks:
            header += f" | {sdk:^{sdk_col_width}}"
        print(header)
        print("-" * 120)

        for flow in all_flows_sorted:
            row = f"{flow:<{flow_col_width}}"

            for sdk in sdks:
                sdk_result = next((r for r in sdk_results if r.sdk == sdk), None)
                if sdk_result:
                    flow_result = next((f for f in sdk_result.flows if f.flow == flow), None)
                    if flow_result:
                        if flow_result.status == "passed":
                            text = "✓ PASS"
                            padding = (sdk_col_width - len(text)) // 2
                            row += " | " + " " * padding + green(text) + " " * (sdk_col_width - len(text) - padding)
                        elif flow_result.status == "failed":
                            text = "✗ FAIL"
                            padding = (sdk_col_width - len(text)) // 2
                            row += " | " + " " * padding + red(text) + " " * (sdk_col_width - len(text) - padding)
                        elif flow_result.status == "skipped":
                            text = "~ SKIP"
                            padding = (sdk_col_width - len(text)) // 2
                            row += " | " + " " * padding + yellow(text) + " " * (sdk_col_width - len(text) - padding)
                        elif flow_result.status == "not_implemented":
                            text = "N/A"
                            padding = (sdk_col_width - len(text)) // 2
                            row += " | " + " " * padding + grey(text) + " " * (sdk_col_width - len(text) - padding)
                    else:
                        text = "—"
                        padding = (sdk_col_width - len(text)) // 2
                        row += " | " + " " * padding + text + " " * (sdk_col_width - len(text) - padding)
                else:
                    text = "MISSING"
                    padding = (sdk_col_width - len(text)) // 2
                    row += " | " + " " * padding + grey(text) + " " * (sdk_col_width - len(text) - padding)

            print(row)

        print("-" * 120)
        overall_row = f"{'OVERALL':<{flow_col_width}}"
        for sdk in sdks:
            sdk_result = next((r for r in sdk_results if r.sdk == sdk), None)
            if sdk_result:
                if sdk_result.status == "passed":
                    overall_row += f" | {green('PASS'):^{sdk_col_width}}"
                elif sdk_result.status == "failed":
                    overall_row += f" | {red('FAIL'):^{sdk_col_width}}"
                else:
                    overall_row += f" | {yellow(sdk_result.status.upper()):^{sdk_col_width}}"
            else:
                overall_row += f" | {grey('MISSING'):^{sdk_col_width}}"
        print(overall_row)

    print("\n" + "=" * 100)
    print(bold("SUMMARY BY SDK"))
    print("-" * 100)

    for sdk in sdks:
        sdk_results = [r for r in results if r.sdk == sdk]
        passed = sum(1 for r in sdk_results if r.status == "passed")
        failed = sum(1 for r in sdk_results if r.status == "failed")
        total = len(sdk_results)

        status_str = green("PASS") if failed == 0 else red("FAIL")
        print(f"{sdk:<15} {passed}/{total} connectors passed {status_str}")

    print("=" * 100 + "\n")


# ── Main ─────────────────────────────────────────────────────────────────────

def main() -> None:
    parser = argparse.ArgumentParser(
        description="Run smoke tests in parallel across all SDKs"
    )
    parser.add_argument("--connectors", help="Comma-separated list of connectors to test")
    parser.add_argument("--all", action="store_true", help="Test all available connectors")
    parser.add_argument("--mock", action="store_true", help="Run in mock mode")
    parser.add_argument("--verbose", "-v", action="store_true", help="Show detailed error output")
    parser.add_argument(
        "--sdks", default="python,javascript,kotlin,rust",
        help="Comma-separated list of SDKs to test"
    )

    args = parser.parse_args()
    sdks = [s.strip() for s in args.sdks.split(",")]
    repo_root = Path(__file__).parent.parent
    
    # Store verbose flag globally for use in test functions
    import builtins
    builtins._VERBOSE = args.verbose

    if args.all:
        connectors = get_all_connectors(repo_root)
    elif args.connectors:
        connectors = [c.strip() for c in args.connectors.split(",")]
    else:
        print("Error: Must specify --connectors or --all", file=sys.stderr)
        sys.exit(1)

    print(f"Running smoke tests for {len(connectors)} connector(s) across {len(sdks)} SDK(s)...")
    print(f"Mode: {'MOCK' if args.mock else 'NORMAL'}")
    print(f"SDKs: {', '.join(sdks)}")
    print(f"Connectors: {', '.join(connectors)}")
    print()

    # ── Pre-build phase (sequential, each step is idempotent) ────────────────
    print("Pre-build phase...")

    if "kotlin" in sdks:
        prepare_kotlin_sdk_once(repo_root)

    if "rust" in sdks:
        prepare_rust_smoke_test_once(repo_root, connectors)

    js_env: Optional[Path] = None
    if "javascript" in sdks:
        js_tarball = build_javascript_sdk_once(repo_root)
        if js_tarball:
            print(f"  JavaScript SDK: {js_tarball.name}")
            js_env = setup_shared_js_env(repo_root, js_tarball, connectors)
        else:
            print("  Warning: JavaScript SDK not available")

    print()

    # ── Test phase: one batch task per SDK, all SDKs run in parallel ─────────
    start_time = time.time()
    all_results: List[SDKResult] = []

    def sdk_task(sdk: str) -> Tuple[str, List[SDKResult]]:
        if sdk == "python":
            return sdk, run_python_test_batch(repo_root, connectors, args.mock)
        elif sdk == "javascript":
            if js_env:
                return sdk, run_javascript_test_batch(connectors, args.mock, js_env)
            return sdk, [
                SDKResult("javascript", c, "error", error="JS env not available")
                for c in connectors
            ]
        elif sdk == "kotlin":
            return sdk, run_kotlin_test_batch(repo_root, connectors, args.mock)
        elif sdk == "rust":
            return sdk, run_rust_test_batch(repo_root, connectors, args.mock)
        return sdk, []

    with ThreadPoolExecutor(max_workers=len(sdks)) as executor:
        futures = {executor.submit(sdk_task, sdk): sdk for sdk in sdks}
        completed = 0

        for future in as_completed(futures):
            completed += 1
            sdk = futures[future]
            try:
                sdk_name, sdk_results = future.result()
                all_results.extend(sdk_results)

                passed = sum(1 for r in sdk_results if r.status == "passed")
                failed = sum(1 for r in sdk_results if r.status == "failed")
                icon = "✓" if failed == 0 else "✗"
                print(
                    f"  [{completed}/{len(sdks)}] {icon} {sdk_name}: "
                    f"{passed}/{len(sdk_results)} connectors passed",
                    flush=True
                )
            except Exception as e:
                print(f"  [{completed}/{len(sdks)}] ✗ {sdk}: {e}", flush=True)
                all_results.extend([
                    SDKResult(sdk, c, "error", error=str(e)) for c in connectors
                ])

    duration = time.time() - start_time

    print_results_table(all_results, sdks)
    print(f"Total duration: {duration:.1f}s")

    any_failed = any(r.status == "failed" for r in all_results)
    sys.exit(1 if any_failed else 0)


if __name__ == "__main__":
    main()
