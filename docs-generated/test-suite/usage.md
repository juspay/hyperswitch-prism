<!--
---
title: Test Suite Usage
description: Running tests and command reference
last_updated: 2026-03-12
generated_from: backend/ucs-connector-tests/src/bin/
auto_generated: false
reviewed_by: engineering
reviewed_at: 2026-03-12
approved: true
---
-->

# Usage

## Overview

The test suite provides two binaries for running tests:

- **`run_test`** - Run single tests with fine-grained control
- **`suite_run_test`** - Run complete test suites

## Quick Reference

```bash
# Test specific connector
cargo run --bin run_test -- --connector stripe

# Test all connectors
cargo run --bin suite_run_test -- --all

# Run specific suite
cargo run --bin run_test -- --connector stripe --suite authorize

# Run specific scenario
cargo run --bin run_test -- --connector stripe --suite authorize --scenario no3ds_auto_capture_credit_card
```

## run_test Binary

**Purpose**: Run individual tests with full control over parameters.

### Basic Usage

```bash
cargo run --bin run_test -- [OPTIONS]
```

### Options

| Option | Description | Default |
|--------|-------------|---------|
| `--connector <name>` | Connector to test | `stripe` |
| `--suite <name>` | Suite to run | `authorize` |
| `--scenario <name>` | Scenario to run | First scenario in suite |
| `--endpoint <host:port>` | gRPC server endpoint | `localhost:50051` |
| `--creds-file <path>` | Credentials file path | `CONNECTOR_AUTH_FILE_PATH` env var |
| `--merchant-id <id>` | Merchant ID | `test_merchant` |
| `--tenant-id <id>` | Tenant ID | `default` |
| `--tls` | Use TLS (vs plaintext) | false |
| `--list-scenarios` | List available scenarios | - |
| `--print-curl` | Print curl equivalent | - |
| `--print-grpcurl` | Print grpcurl command | - |

### Examples

#### Test Default Scenario

```bash
cargo run --bin run_test -- --connector stripe
```

Runs the default scenario from the `authorize` suite against Stripe.

#### Test Specific Suite

```bash
cargo run --bin run_test -- --connector stripe --suite capture
```

Runs the default scenario from the `capture` suite.

#### Test Specific Scenario

```bash
cargo run --bin run_test \
  --connector stripe \
  --suite authorize \
  --scenario no3ds_auto_capture_credit_card
```

Runs a specific scenario by name.

#### List Available Scenarios

```bash
cargo run --bin run_test -- --connector stripe --suite authorize --list-scenarios
```

Output:
```
Available scenarios in authorize suite:
  - no3ds_auto_capture_credit_card (default)
  - no3ds_manual_capture_credit_card
  - no3ds_declined_card
```

#### Print grpcurl Command

```bash
cargo run --bin run_test \
  --connector stripe \
  --suite authorize \
  --print-grpcurl
```

Outputs the equivalent grpcurl command for debugging.

#### Custom Endpoint

```bash
cargo run --bin run_test \
  --connector stripe \
  --endpoint staging.ucs.example.com:443 \
  --tls
```

## suite_run_test Binary

**Purpose**: Run complete test suites across multiple connectors.

### Basic Usage

```bash
cargo run --bin suite_run_test -- [COMMAND] [OPTIONS]
```

### Commands

| Command | Description |
|---------|-------------|
| `--suite <name>` | Run all scenarios in a specific suite |
| `--all` | Run all suites for the selected connector(s) |
| `--all-connectors` | Run all suites for all connectors |
| `<suite>` | Shorthand for `--suite <suite>` |

### Options

| Option | Description | Default |
|--------|-------------|---------|
| `--connector <name>` | Connector to test | All configured connectors |
| `--endpoint <host:port>` | gRPC server endpoint | `localhost:50051` |
| `--creds-file <path>` | Credentials file path | `CONNECTOR_AUTH_FILE_PATH` env var |
| `--merchant-id <id>` | Merchant ID | `test_merchant` |
| `--tenant-id <id>` | Tenant ID | `default` |
| `--tls` | Use TLS | false |

### Examples

#### Run Single Suite

```bash
cargo run --bin suite_run_test -- --suite authorize --connector stripe
```

Or shorthand:

```bash
cargo run --bin suite_run_test -- authorize --connector stripe
```

#### Run All Suites for One Connector

```bash
cargo run --bin suite_run_test -- --all --connector stripe
```

#### Run All Suites for All Connectors

```bash
cargo run --bin suite_run_test -- --all-connectors
```

Or:

```bash
cargo run --bin suite_run_test -- --all
```

#### Run Specific Connectors

```bash
export UCS_ALL_CONNECTORS=stripe,adyen,paypal
cargo run --bin suite_run_test -- --all
```

## Environment Variables

### Required

```bash
export CONNECTOR_AUTH_FILE_PATH=/path/to/creds.json
```

### Optional

```bash
# Scenario location
export UCS_SCENARIO_ROOT=/custom/scenarios

# Report output
export UCS_RUN_TEST_REPORT_PATH=/custom/path/report.json

# Connectors to test
export UCS_ALL_CONNECTORS=stripe,adyen,braintree

# Debug mode
export UCS_DEBUG_EFFECTIVE_REQ=1
```

## Output Files

### Report Files

| File | Location | Description |
|------|----------|-------------|
| `report.json` | `backend/ucs-connector-tests/` | Machine-readable test results |
| `test_report.md` | `backend/ucs-connector-tests/` | Human-readable markdown summary |

### Report Structure

```
backend/ucs-connector-tests/
├── report.json              # Raw test data
├── test_report.md           # Markdown report
└── docs/test-reports/       # Historical reports (optional)
    ├── stripe-report.md
    ├── adyen-report.md
    └── ...
```

## Reading Test Reports

### Summary Section

```markdown
## Summary

| Metric | Count |
|--------|------:|
| Connectors Tested | 3 |
| Total Scenarios | 15 |
| Passed | 42 |
| Failed | 3 |
| Pass Rate | 93.3% |
```

### Scenario Performance Matrix

```markdown
| Scenario | Suite | Service | PM | PMT | Connectors Tested | Passed | Failed | Pass Rate |
|:---------|:------|:--------|:--:|:---:|------------------:|------:|------:|---------:|
| no3ds_auto_capture | authorize | PaymentService/Authorize | card | credit | 3 | 3 | 0 | 100.0% |
```

### Test Matrix

```markdown
| Scenario | Suite | Service | PM | PMT | stripe | adyen | paypal |
|:---------|:------|:--------|:--:|:---:|-------:|------:|-------:|
| no3ds_auto_capture | authorize | PaymentService/Authorize | card | credit | PASS | PASS | PASS |
```

## Debugging Tests

### Enable Debug Output

```bash
export UCS_DEBUG_EFFECTIVE_REQ=1
cargo run --bin run_test -- --connector stripe --suite authorize
```

Shows the effective request JSON after auto-generation and context injection.

### Print grpcurl Command

```bash
cargo run --bin run_test \
  --connector stripe \
  --suite authorize \
  --scenario no3ds_auto_capture_credit_card \
  --print-grpcurl
```

Output:
```bash
grpcurl -plaintext \
  -H "x-connector: stripe" \
  -H "x-connector-auth: {...}" \
  -d @ localhost:50051 payment.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": {"id": "mti_a1b2c3d4"},
  ...
}
JSON
```

### Check Dependencies

```bash
cargo run --bin run_test \
  --connector stripe \
  --suite capture \
  --list-dependencies
```

Shows the dependency chain for a suite.

## Common Workflows

### Add New Test Scenario

1. Edit suite scenario.json:
   ```bash
   vim backend/ucs-connector-tests/src/global_suites/authorize_suite/scenario.json
   ```

2. Add new scenario:
   ```json
   {
     "my_new_scenario": {
       "grpc_req": { ... },
       "assert": { ... },
       "is_default": false
     }
   }
   ```

3. Run the new scenario:
   ```bash
   cargo run --bin run_test \
     --connector stripe \
     --suite authorize \
     --scenario my_new_scenario
   ```

### Test New Connector

1. Create connector spec:
   ```bash
   cat > backend/ucs-connector-tests/src/connector_specs/mynewconnector.json << 'EOF'
   {
     "connector": "mynewconnector",
     "supported_suites": ["authorize", "capture"]
   }
   EOF
   ```

2. Add credentials:
   ```bash
   # Edit creds.json and add mynewconnector credentials
   ```

3. Run tests:
   ```bash
   cargo run --bin suite_run_test -- --all --connector mynewconnector
   ```

### Debug Failing Test

1. Run with debug output:
   ```bash
   export UCS_DEBUG_EFFECTIVE_REQ=1
   cargo run --bin run_test -- --connector stripe --suite authorize --scenario failing_scenario
   ```

2. Check the report:
   ```bash
   cat backend/ucs-connector-tests/test_report.md
   ```

3. Print grpcurl to test manually:
   ```bash
   cargo run --bin run_test ... --print-grpcurl
   ```

## Troubleshooting

### "Connector not found in credentials file"

**Cause**: Credentials file doesn't contain the connector.

**Solution**:
```bash
# Check credentials file contains the connector
jq 'keys' $CONNECTOR_AUTH_FILE_PATH

# Add connector credentials to file
```

### "Scenario not found in suite"

**Cause**: Scenario name doesn't exist in the suite.

**Solution**:
```bash
# List available scenarios
cargo run --bin run_test -- --suite authorize --list-scenarios
```

### "Suite spec missing"

**Cause**: `suite_spec.json` file is missing or invalid.

**Solution**:
```bash
# Check suite directory exists
ls backend/ucs-connector-tests/src/global_suites/authorize_suite/

# Should contain: scenario.json, suite_spec.json
```

### "Dependency scenario not found"

**Cause**: Referenced dependency scenario doesn't exist.

**Solution**:
```bash
# Check the depends_on in suite_spec.json
# Verify the referenced scenario exists in the dependency suite
```

## Next Steps

- [CI/CD Integration](./ci-cd.md) - Continuous integration setup
- [Best Practices](./best-practices.md) - Writing effective tests
- [Configuration](./configuration.md) - Environment setup
