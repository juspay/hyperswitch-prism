# UCS Connector Tests

Scenario-driven integration testing for payment connectors.

## Quick Start

### 1. First-Time Setup

Run setup once (auto-installs everything you need):

```bash
make setup-connector-tests
# or: ./scripts/run-tests
```

**What it installs:**
- Browser automation dependencies (Node, Playwright)
- `grpcurl` for gRPC testing
- Netlify CLI (for Google Pay tests)
- `test-prism` command (test runner)

### 2. Run Tests

After setup, use the `test-prism` command:

```bash
# Interactive mode (recommended for first-time users)
test-prism --interactive

# Run all tests for a connector
test-prism --connector stripe

# Run specific suite
test-prism --connector stripe --suite authorize

# Run specific scenario
test-prism --connector stripe --suite authorize --scenario no3ds_auto_capture_credit_card

# Run all configured connectors
test-prism --all-connectors

# Use SDK instead of gRPC
test-prism --interface sdk --connector stripe
```

## All Available Commands

### Setup Commands

```bash
# Initial setup (run once)
make setup-connector-tests

# Re-run setup anytime
test-prism --setup

# Skip Google Pay setup (faster, disables GPay tests)
SKIP_NETLIFY_DEPLOY=1 make setup-connector-tests
```

### Test Commands

```bash
# Interactive wizard (step-by-step selection)
test-prism --interactive

# Run by connector
test-prism --connector <name>              # One connector, all suites
test-prism --all-connectors                # All configured connectors

# Run by suite
test-prism --connector stripe --suite authorize
test-prism --connector stripe --suite authorize --scenario <name>

# Choose backend
test-prism --interface grpc --connector stripe    # gRPC (default)
test-prism --interface sdk --connector stripe     # SDK/FFI

# Generate reports
test-prism --connector stripe --report

# Combine options
test-prism --all-connectors --interface sdk --report
```

### Direct Cargo Commands (Advanced)

For debugging individual scenarios, use `make cargo` to auto-load environment:

```bash
# Run one scenario
make cargo ARGS="run -p integration-tests --bin run_test -- \
  --connector stripe \
  --suite authorize \
  --scenario no3ds_auto_capture_credit_card"

# Run entire suite
make cargo ARGS="run -p integration-tests --bin suite_run_test -- \
  --connector stripe \
  --suite authorize"

# Run all suites for connector
make cargo ARGS="run -p integration-tests --bin suite_run_test -- \
  --connector stripe \
  --all"

# Run with SDK
make cargo ARGS="run -p integration-tests --bin sdk_run_test -- \
  --connector stripe \
  --all"
```

### Report Commands

```bash
# Tests automatically generate reports at:
# - backend/integration-tests/report.json
# - backend/integration-tests/test_report/*.md

# Regenerate markdown from existing report.json (without running tests)
cargo run -p integration-tests --bin render_report
```

### Scenario Display Names

Scenario display names are human-readable labels shown in markdown reports instead of raw scenario keys.

Style A format used by the generator:
- Payment-style scenarios: `<Subject> | <Auth Type> | <Capture Mode>`
- Example: `Credit Card | No 3DS | Automatic Capture`

```bash
# Generate display names for all suites
cargo run -p integration-tests --bin generate_scenario_display_names

# Generate for one suite only
cargo run -p integration-tests --bin generate_scenario_display_names -- --suite authorize

# Preview changes without writing
cargo run -p integration-tests --bin generate_scenario_display_names -- --check

# Generate/update display names and regenerate markdown in one command
cargo run -p integration-tests --bin generate_scenario_display_names -- --render-markdown
```

How it works:
- Reads `scenario.json` files under `src/global_suites/*_suite/`
- Generates or updates each scenario's `display_name`
- Uses `display_name` in markdown reports (falls back to generated Style A when missing)
- Escapes `|` in markdown tables to keep columns aligned

### Help Commands

```bash
# See all test-prism options
test-prism --help

# See specific binary options
cargo run -p integration-tests --bin run_test -- --help
cargo run -p integration-tests --bin suite_run_test -- --help
cargo run -p integration-tests --bin sdk_run_test -- --help
```

## Configuration

### Environment Variables

The setup script creates `.env.connector-tests` with auto-configured values. You can also set:

```bash
# Credentials (required)
export CONNECTOR_AUTH_FILE_PATH="$PWD/creds.json"

# Connector list for --all-connectors
export UCS_ALL_CONNECTORS="stripe,paypal,authorizedotnet"

# SDK environment
export UCS_SDK_ENVIRONMENT=sandbox  # or: production

# Debug flags
export UCS_DEBUG_EFFECTIVE_REQ=1    # Print request payloads
```

### Credentials File

At runtime, integration tests load credentials from `creds.json` in the repo root. A starter template is available at `.github/test/template_creds.json`.

Create `creds.json` in the repo root, for example by copying the template:

```bash
cp .github/test/template_creds.json creds.json
```

Then update it with your real connector credentials:

```json
{
  "stripe": {
    "connector_account_details": {
      "auth_type": "HeaderKey",
      "api_key": "sk_test_..."
    }
  },
  "paypal": { ... }
}
```

See `.github/test/template_creds.json` for the full structure.

For Google Pay encrypted-token scenarios, the connector entry in `creds.json` must also include a `metadata.google_pay` block. The expected shape is documented in `browser-automation-engine/src/gpay-token-gen.ts`, and the easiest way to add it is to copy the `metadata.google_pay` block from another connector in your local `creds.json` that already has Google Pay configured, then replace the gateway-specific values for your connector.

## Project Structure

```
backend/integration-tests/
├── src/
│   ├── global_suites/           # Test scenarios (JSON)
│   │   ├── authorize_suite/
│   │   ├── capture_suite/
│   │   └── ...
│   ├── connector_specs/         # Connector-specific configs
│   │   ├── stripe/
│   │   │   ├── specs.json       # Supported suites
│   │   │   └── override.json    # Connector-specific test data
│   │   └── ...
│   └── harness/                 # Test execution engine
├── test_report/                 # Generated markdown reports
└── report.json                  # Generated JSON report
```

## Common Scenarios

### First-time setup on new machine
```bash
make setup-connector-tests
test-prism --interactive
```

### Run tests for new connector
```bash
test-prism --connector <connector-name> --report
```

### Debug failing scenario
```bash
export UCS_DEBUG_EFFECTIVE_REQ=1
test-prism --connector stripe --suite authorize --scenario <failing-scenario>
```

### CI/CD usage
```bash
export CONNECTOR_AUTH_FILE_PATH="/path/to/creds.json"
export UCS_ALL_CONNECTORS="stripe,paypal"
export SKIP_NETLIFY_DEPLOY=1
make setup-connector-tests
test-prism --all-connectors --report
```

## Adding Tests

### Add connector-specific override
Edit `src/connector_specs/<connector>/override.json`:

```json
{
  "authorize": {
    "no3ds_fail_payment": {
      "grpc_req": {
        "payment_method": {
          "card": {
            "card_number": { "value": "4000000000000002" }
          }
        }
      },
      "assert": {
        "error.connector_details.message": { "contains": "declined" }
      }
    }
  }
}
```

### Validate changes
```bash
# Run affected tests
test-prism --connector <connector> --suite authorize

# Run schema validation
cargo test -p integration-tests all_supported_scenarios_match_proto_schema_for_all_connectors
```

## Troubleshooting

### Setup fails
```bash
# Re-run setup
test-prism --setup

# Skip optional components
SKIP_NETLIFY_DEPLOY=1 make setup-connector-tests
```

### Tests fail with "credentials not found"
```bash
# Check credentials file exists
ls -la creds.json

# Set path explicitly
export CONNECTOR_AUTH_FILE_PATH="$PWD/creds.json"
```

### Google Pay tests are skipped
```bash
# Verify Netlify URL is set
cat .env.connector-tests | grep GPAY_HOSTED_URL

# If the skip mentions missing metadata.google_pay,
# add metadata.google_pay under that connector in creds.json.
# Refer to browser-automation-engine/src/gpay-token-gen.ts
# for the expected shape and copy an existing configured connector
# entry in creds.json as a starting point.

# Re-run Netlify deployment
cd browser-automation-engine
netlify deploy --prod
```

### grpcurl not found
```bash
# macOS
brew install grpcurl

# Linux
# Download from: https://github.com/fullstorydev/grpcurl/releases

# Or re-run setup
test-prism --setup
```

## Documentation

- Scenario JSON format: `docs/scenario-json-core-readme.md`
- Connector overrides: `docs/connector-overrides.md`
- Code walkthrough: `docs/code-walkthrough.md`
- Context mapping: `docs/context-mapping.md`

## Support

- GitHub Issues: https://github.com/juspay/connector-service/issues
- Run `test-prism --help` for all options
