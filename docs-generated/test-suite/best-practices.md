<!--
---
title: Best Practices
description: Guidelines for writing effective test scenarios
last_updated: 2026-03-12
generated_from: backend/ucs-connector-tests/
auto_generated: false
reviewed_by: engineering
reviewed_at: 2026-03-12
approved: true
---
-->

# Best Practices

## Overview

Following these best practices ensures your tests are maintainable, reliable, and effective at catching issues.

## Writing Test Scenarios

### Use Auto-Generate for Dynamic Values

Always use `auto_generate` for values that should be unique per test run:

**Good**:
```json
{
  "merchant_transaction_id": {"id": "auto_generate"},
  "customer": {
    "name": "auto_generate",
    "email": {"value": "auto_generate"},
    "id": "auto_generate"
  }
}
```

**Avoid**:
```json
{
  "merchant_transaction_id": {"id": "test_txn_123"},
  "customer": {
    "name": "John Doe",
    "email": {"value": "john@example.com"}
  }
}
```

**Why**: Hard-coded values cause conflicts when tests run in parallel or against stateful connectors.

### Set Default Scenario

Always mark one scenario per suite as `is_default: true`:

```json
{
  "no3ds_auto_capture_credit_card": {
    "grpc_req": { },
    "assert": { },
    "is_default": true
  },
  "no3ds_manual_capture": {
    "grpc_req": { },
    "assert": { },
    "is_default": false
  }
}
```

**Why**: Makes it easy to run the primary use case without specifying scenario name.

### Keep Assertions Focused

Test only what matters for the specific operation:

**Good**:
```json
{
  "assert": {
    "status": {"one_of": ["CHARGED", "AUTHORIZED"]},
    "connector_transaction_id": {"must_exist": true},
    "error": {"must_not_exist": true}
  }
}
```

**Avoid**:
```json
{
  "assert": {
    "status": {"equals": "CHARGED"},
    "connector_transaction_id": {"must_exist": true},
    "connector_transaction_id.id": {"must_exist": true},
    "connector_transaction_id.id.length": {"equals": 27},
    "connector_response": {"must_exist": true},
    "merchant_transaction_id": {"echo": "merchant_transaction_id"},
    "amount.minor_amount": {"echo": "amount.minor_amount"},
    "amount.currency": {"echo": "amount.currency"},
    "customer.name": {"must_exist": true}
  }
}
```

**Why**: Over-asserting makes tests brittle and harder to maintain.

### Use Realistic Test Data

Follow payment processor guidelines for test cards:

| Card Number | Type | Expected Result |
|-------------|------|----------------|
| `4111111111111111` | Visa Success | Successful authorization |
| `4000000000000002` | Generic Decline | Declined payment |
| `4000000000003220` | 3DS2 Frictionless | 3D Secure flow |

**Good**:
```json
{
  "payment_method": {
    "card": {
      "card_number": {"value": "4111111111111111"},
      "card_exp_month": {"value": "12"},
      "card_exp_year": {"value": "2027"}
    }
  }
}
```

### Document Edge Cases

Add comments for non-obvious scenarios:

```json
{
  "_comment": "Tests decline handling with insufficient funds",
  "no3ds_insufficient_funds": {
    "grpc_req": {
      "payment_method": {
        "card": {
          "card_number": {"value": "4000000000009995"}
        }
      }
    },
    "assert": {
      "status": {"equals": "FAILURE"},
      "error_message": {"contains": "insufficient funds"}
    }
  }
}
```

## Naming Conventions

### Scenario Names

Use descriptive, structured names:

Format: `{3ds_status}_{capture_type}_{payment_method}_{card_type}_{scenario_type}`

Examples:
- `no3ds_auto_capture_credit_card` - Standard success case
- `no3ds_manual_capture_debit_card` - Manual capture flow
- `3ds_frictionless_auto_capture_wallet` - 3DS wallet payment
- `no3ds_declined_insufficient_funds` - Error case

### Why Consistent Naming Matters

- Easy to understand test coverage
- Simple filtering and selection
- Clear failure identification
- Better reporting

## Managing Dependencies

### Order Matters

Define dependencies in the correct sequence:

**Good**:
```json
{
  "suite": "capture",
  "depends_on": ["authorize"]
}
```

**Avoid**:
```json
{
  "suite": "capture",
  "depends_on": ["create_customer"]
}
```

**Why**: Capture needs the transaction ID from authorize, not just a customer.

### Reuse Dependencies

Chain related tests efficiently:

```
create_access_token -> create_customer -> authorize -> capture -> refund
```

Each step builds on the previous, minimizing setup overhead.

### Use Loose Coupling

Use `strict_dependencies: false` for optional dependencies:

```json
{
  "suite": "get",
  "suite_type": "dependent",
  "depends_on": ["authorize"],
  "strict_dependencies": false
}
```

**Why**: Get can still verify the endpoint works even if authorize failed.

## Credential Management

### Never Commit Credentials

Keep credentials in environment variables or secret stores:

**Good**:
```bash
export CONNECTOR_AUTH_FILE_PATH=~/.ucs-creds.json
# File is in .gitignore
```

**Avoid**:
```bash
# Do not do this
creds.json  # Committed to git
```

### Use Test Environments

Always test against sandbox/test endpoints:

```json
{
  "stripe": {
    "connector_account_details": {
      "api_key": "sk_test_..."
    }
  }
}
```

### Rotate Regularly

Update API keys periodically:

- Set calendar reminders
- Use short-lived tokens where possible
- Audit key usage regularly
- Revoke unused keys

## Test Organization

### Group Related Scenarios

Keep similar scenarios together:

```json
{
  "no3ds_auto_capture_credit_card": { },
  "no3ds_auto_capture_debit_card": { },
  "no3ds_manual_capture_credit_card": { },
  "no3ds_declined_card": { }
}
```

### Separate Happy Path and Edge Cases

Mark primary scenarios as default:

```json
{
  "no3ds_auto_capture_credit_card": {
    "is_default": true
  },
  "no3ds_declined_card": {
    "is_default": false
  }
}
```

### Document Connector Limitations

Use connector specs to document supported features:

```json
{
  "connector": "mynewconnector",
  "supported_suites": [
    "authorize",
    "capture",
    "void"
  ]
}
```

## Debugging

### Enable Debug Mode

When tests fail, enable debug output:

```bash
export UCS_DEBUG_EFFECTIVE_REQ=1
cargo run --bin run_test -- --connector stripe --suite authorize
```

### Print grpcurl Command

Test the exact request manually:

```bash
cargo run --bin run_test ... --print-grpcurl
# Copy and modify the grpcurl command
```

### Check Test Reports

Always check the generated report:

```bash
cat backend/ucs-connector-tests/test_report.md
```

### Isolate Failures

Run a single scenario to isolate issues:

```bash
cargo run --bin run_test \
  --connector stripe \
  --suite authorize \
  --scenario failing_scenario
```

## Performance

### Minimize Dependencies

Only depend on what you actually need:

**Good**:
```json
{
  "suite": "get",
  "depends_on": ["authorize"]
}
```

**Avoid**:
```json
{
  "suite": "get",
  "depends_on": ["create_access_token", "create_customer", "authorize"]
}
```

**Why**: Get only needs the transaction ID from authorize.

### Use Efficient Assertions

Prefer simple assertions over complex ones:

**Good**:
```json
{
  "status": {"one_of": ["CHARGED", "AUTHORIZED"]}
}
```

**Avoid**:
```json
{
  "status": {"equals": "CHARGED"},
  "alternative_status": {"equals": "AUTHORIZED"},
  "tertiary_status": {"equals": "PENDING"}
}
```

## Review Checklist

Before submitting new tests:

- [ ] Uses `auto_generate` for dynamic values
- [ ] Has one `is_default: true` scenario
- [ ] Assertions are focused and minimal
- [ ] Uses realistic test card numbers
- [ ] Follows naming conventions
- [ ] Dependencies are correct
- [ ] Credentials are not committed
- [ ] Test runs locally
- [ ] Report is generated correctly

## Next Steps

- [Configuration](./configuration.md) - Environment and credentials setup
- [Usage](./usage.md) - Running tests
- [CI/CD Integration](./ci-cd.md) - Continuous integration
