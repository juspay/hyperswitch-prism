# HTTP Client Sanity Tests

This folder contains the HTTP Client Sanity Testing framework for certifying SDK HTTP clients across all languages.

**Prerequisites** (for a fresh setup):
```bash
# Generate proto stubs for each SDK (gitignored, must be generated locally):
make -C sdk/javascript generate-proto    # proto.js  (JS runner)
make -C sdk/python generate-proto        # *_pb2.py  (Python runner)
make -C sdk/java generate-proto          # Java stubs (Kotlin runner)
# Rust: no step needed — cargo compiles proto via build.rs automatically
```

**Run**: `make certify-client-sanity` from the repository root.

**Key Files**:
- `manifest.json` - Single source of truth for all test scenarios
- `run_client_certification.js` - Orchestrates test execution across all SDKs
- `echo_server.js` - Mock server that captures requests and returns expected responses
- `judge.js` - Validates SDK behavior against golden truth
- `generate_golden.js` - Generates golden capture files from manifest
- `simple_proxy.js` - HTTP proxy for testing proxy configurations

**Generated Files** (in `artifacts/`, git-ignored):
- `golden_<scenario>.json` - Expected behavior from manifest (golden truth)
- `actual_<lang>_<scenario>.json` - SDK's actual output (response or error)
- `capture_<lang>_<scenario>.json` - Echo server's captured request
- `REPORT.md` - Certification results

**Note**: All generated files are in the `artifacts/` subfolder to keep source files separate from test outputs.

**Note**: This is distinct from other sanity tests. These tests specifically verify HTTP client behavior (request/response handling, error codes, timeouts, proxies, etc.).
