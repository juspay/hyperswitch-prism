<!--
---
title: Global Test Suites
description: Reusable test scenarios and dependency management
last_updated: 2026-03-12
generated_from: backend/integration-tests/src/global_suites/
auto_generated: false
reviewed_by: engineering
reviewed_at: 2026-03-12
approved: true
---
-->

# Global Test Suites

## Overview

**Global suites** are reusable test scenarios stored in `src/global_suites/` that can be executed against any connector. They define the "happy path" and common edge cases for each payment operation.

**Key Concept**: Write once, run everywhere. A single global suite scenario validates Stripe, Adyen, PayPal, and all other connectors that support that operation.

## Directory Structure

```
backend/integration-tests/src/global_suites/
├── authorize_suite/
│   ├── scenario.json          # Test cases for authorization
│   └── suite_spec.json        # Suite metadata and dependencies
├── capture_suite/
│   ├── scenario.json
│   └── suite_spec.json
├── void_suite/
├── refund_suite/
├── get_suite/
├── create_access_token_suite/
├── create_customer_suite/
├── setup_recurring_suite/
├── recurring_charge_suite/
└── refund_sync_suite/
```

## Available Suites

| Suite | Service | Type | Dependencies | Description |
|-------|---------|------|--------------|-------------|
| **authorize** | PaymentService/Authorize | Dependent | create_access_token, create_customer | Payment authorization |
| **capture** | PaymentService/Capture | Dependent | authorize | Capture authorized payments |
| **void** | PaymentService/Void | Dependent | authorize | Void authorized payments |
| **refund** | PaymentService/Refund | Dependent | capture | Refund captured payments |
| **get** | PaymentService/Get | Dependent | authorize | Retrieve payment status |
| **create_access_token** | MerchantAuthenticationService | Independent | none | Create authentication tokens |
| **create_customer** | CustomerService/Create | Dependent | create_access_token | Create customer profiles |
| **setup_recurring** | PaymentService/SetupRecurring | Dependent | authorize | Setup recurring mandates |
| **recurring_charge** | RecurringPaymentService/Charge | Dependent | setup_recurring | Charge using mandate |
| **refund_sync** | RefundService/Get | Dependent | refund | Sync refund status |

## Suite Types

### Independent Suites

Can run standalone without any dependencies:

```json
{
  "suite": "create_access_token",
  "suite_type": "independent",
  "depends_on": [],
  "strict_dependencies": false
}
```

**Examples**:
- `create_access_token` - No prerequisites needed

### Dependent Suites

Require other suites to run first:

```json
{
  "suite": "capture",
  "suite_type": "dependent",
  "depends_on": ["authorize"],
  "strict_dependencies": true
}
```

**Examples**:
- `authorize` depends on `create_access_token`, `create_customer`
- `capture` depends on `authorize`
- `refund` depends on `capture`

## Dependency Pipeline

When running dependent suites, the test harness automatically:

1. **Executes dependencies first**: Runs `create_access_token` → `create_customer` → `authorize`
2. **Captures context**: Stores response values (transaction IDs, customer IDs) from each step
3. **Injects into requests**: Automatically populates dependent request fields
4. **Prunes unresolved fields**: Removes context fields that couldn't be resolved

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│ create_access_  │────▶│   authorize     │────▶│    capture      │
│     token       │     │                 │     │                 │
└────────┬────────┘     └────────┬────────┘     └─────────────────┘
         │                       │
         │ access_token          │ connector_transaction_id
         ▼                       ▼
    Injected into           Injected into
    authorize request       capture request
```

### Context Injection

Fields from dependency responses are automatically injected into dependent requests:

| Dependency | Response Field | Injected Into |
|------------|----------------|---------------|
| create_access_token | `access_token.token.value` | `state.access_token.token.value` |
| create_customer | `connector_customer_id` | `customer.connector_customer_id` |
| authorize | `connector_transaction_id.id` | `connector_transaction_id.id` |
| capture | `connector_transaction_id.id` | `connector_transaction_id.id` |
| setup_recurring | `mandate_reference` | `mandate_reference` |

### Dependency Specification Formats

**Simple** - Just the suite name:

```json
{
  "depends_on": ["create_access_token", "create_customer"]
}
```

**With Scenario** - Specific scenario from the suite:

```json
{
  "depends_on": [
    {
      "suite": "authorize",
      "scenario": "no3ds_manual_capture_credit_card"
    }
  ]
}
```

This runs the `no3ds_manual_capture_credit_card` scenario from the authorize suite before capture.

## Strict vs Loose Dependencies

### Strict Dependencies

```json
{
  "strict_dependencies": true
}
```

- Fail the test if any dependency fails
- Use for critical prerequisites

### Loose Dependencies

```json
{
  "strict_dependencies": false
}
```

- Continue even if dependencies fail
- Use for optional prerequisites
- Useful for testing error scenarios

## Connector Specifications

Define which suites each connector supports:

**File**: `src/connector_specs/{connector}.json`

```json
{
  "connector": "stripe",
  "supported_suites": [
    "authorize",
    "capture",
    "void",
    "refund",
    "get",
    "refund_sync"
  ]
}
```

### Purpose

- Skip unsupported suites during `--all` runs
- Document connector capabilities
- Enable gradual rollout of new tests

### Available Connector Specs

The framework includes specs for 25+ connectors:
- Stripe, Adyen, PayPal, Braintree
- Authorize.Net, Checkout.com, Cybersource
- And more...

## Writing Global Suite Scenarios

### Best Practices

1. **Use `auto_generate` for dynamic values**:
   ```json
   {
     "merchant_transaction_id": {"id": "auto_generate"},
     "customer": {
       "email": {"value": "auto_generate"}
     }
   }
   ```

2. **Set `is_default: true`** for the primary scenario:
   ```json
   {
     "scenario_name": {
       "grpc_req": { ... },
       "assert": { ... },
       "is_default": true
     }
   }
   ```

3. **Keep assertions focused**:
   - Test only what matters for the operation
   - Don't over-assert on connector-specific fields

4. **Use realistic test data**:
   - Standard test card numbers
   - Realistic amounts (e.g., 6000 for $60.00)
   - Valid country codes

### Scenario Naming Conventions

Use descriptive names with structure:

```
{3ds_status}_{capture_type}_{payment_method}
```

Examples:
- `no3ds_auto_capture_credit_card`
- `no3ds_manual_capture_debit_card`
- `3ds_frictionless_auto_capture_wallet`

## Next Steps

- [Test Structure](./test-structure.md) - Scenario and assertion details
- [Configuration](./configuration.md) - Test data and credentials
- [Overrides](./overrides.md) - Customizing scenarios per connector
