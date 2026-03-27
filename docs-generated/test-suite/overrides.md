<!--
---
title: Test Overrides
description: Customizing test scenarios per connector
last_updated: 2026-03-12
generated_from: backend/ucs-connector-tests/docs/scenario-json-core-readme.md
auto_generated: false
reviewed_by: engineering
reviewed_at: 2026-03-12
approved: true
---
-->

# Overrides

## Overview

**Overrides** allow customizing global suite scenarios for specific connectors or edge cases without modifying the base scenario files. They enable:

- Connector-specific request modifications (e.g., different test card numbers)
- Connector-specific assertion adjustments (e.g., different error message formats)
- Edge case handling (e.g., specific decline codes)

## Override Types

| Type | Purpose | Location |
|------|---------|----------|
| **Request Override** | Modify request payload fields | Planned: `src/overrides/{connector}/{suite}.json` |
| **Assert Override** | Modify assertion rules | Planned: `src/overrides/{connector}/{suite}.json` |
| **Dependency Override** | Override specific dependency scenario | `suite_spec.json` `depends_on` |

## Request Override (Planned)

Modify request fields for specific connectors:

```json
{
  "scenario_name": "no3ds_auto_capture_credit_card",
  "request_override": {
    "payment_method.card.card_number.value": "4000000000003220",
    "payment_method.card.card_type": "debit"
  }
}
```

### Use Cases

- Different test card numbers per connector
- Connector-specific payment method types
- Regional variations (e.g., EUR vs USD)

## Assert Override (Planned)

Modify assertion rules for specific connectors:

```json
{
  "scenario_name": "no3ds_auto_capture_credit_card",
  "assert_override": {
    "status": {
      "remove": ["equals"],
      "add": { "one_of": ["AUTHENTICATION_FAILED", "DECLINED"] }
    },
    "error_message": {
      "add": { "contains": "insufficient_funds" }
    }
  }
}
```

### Use Cases

- Connector-specific error message formats
- Different status codes per connector
- Relaxed assertions for known limitations

### Assert Override Strategy

Assert overrides use a **merge strategy** with base assertions:

1. Start with base assertions from global suite
2. Apply connector-specific additions
3. Remove overridden fields
4. Execute merged assertions

```rust
// Pseudo-code for assert merging
fn merge_asserts(base: AssertMap, override: AssertMap) -> AssertMap {
    let mut merged = base.clone();
    for (field, rules) in override {
        if rules.remove_all {
            merged.remove(&field);
        } else {
            merged.insert(field, rules);
        }
    }
    merged
}
```

## Dependency Scenario Override

Reference a specific scenario from a dependency suite:

```json
{
  "suite": "capture",
  "suite_type": "dependent",
  "depends_on": [
    {
      "suite": "authorize",
      "scenario": "no3ds_manual_capture_credit_card"
    }
  ],
  "strict_dependencies": true
}
```

This runs the `no3ds_manual_capture_credit_card` scenario from the authorize suite before capture.

### Use Cases

- Test specific authorization scenarios
- Test edge cases that require specific setup
- Override default dependency behavior

## Context Overrides

Context maps define how data flows from dependency responses to current requests:

```json
{
  "suite": "capture",
  "suite_type": "dependent",
  "depends_on": [
    {
      "suite": "authorize",
      "scenario": "no3ds_auto_capture",
      "context_map": {
        "connector_transaction_id.id": "res.connectorTransactionId.id",
        "state.access_token.token.value": "res.access_token"
      }
    }
  ]
}
```

### Default Context Mappings

The framework automatically handles common mappings:

| Source (Dependency) | Target (Request) |
|--------------------|--------------------|
| `create_access_token.access_token` | `state.access_token.token.value` |
| `create_customer.connector_customer_id` | `customer.connector_customer_id` |
| `authorize.connector_transaction_id.id` | `connector_transaction_id.id` |
| `capture.connector_transaction_id.id` | `connector_transaction_id.id` |

## Override File Structure (Planned)

Future releases will support dedicated override files:

```
src/overrides/
├── stripe/
│   ├── authorize.json      # Stripe-specific authorize overrides
│   └── capture.json        # Stripe-specific capture overrides
├── adyen/
│   ├── authorize.json
│   └── refund.json
└── common/
    └── edge_cases.json     # Shared edge case overrides
```

### Example Override File

```json
{
  "overrides": [
    {
      "scenario": "no3ds_auto_capture_credit_card",
      "request_override": {
        "payment_method.card.card_number.value": "4000000000003220"
      },
      "assert_override": {
        "status": {
          "override": { "one_of": ["AUTHENTICATION_REQUIRED", "DECLINED"] }
        }
      }
    },
    {
      "scenario": "no3ds_declined_card",
      "request_override": {
        "payment_method.card.card_number.value": "4000000000000002"
      },
      "assert_override": {
        "error_message": {
          "add": { "contains": "card declined" }
        }
      }
    }
  ]
}
```

## Current Workarounds

Until full override support is implemented:

### 1. Create Connector-Specific Scenarios

Add connector-specific scenarios directly in the global suite:

```json
{
  "no3ds_auto_capture_credit_card_stripe": {
    "grpc_req": { ... },
    "assert": { ... }
  },
  "no3ds_auto_capture_credit_card_adyen": {
    "grpc_req": { ... },
    "assert": { ... }
  }
}
```

### 2. Use Connector Specs

Define connector-specific test coverage:

```json
{
  "connector": "stripe",
  "supported_suites": ["authorize_stripe", "capture"]
}
```

### 3. Environment Variables

Use env vars to modify behavior:

```bash
export UCS_USE_STRIPE_3DS_CARDS=1
```

## Best Practices

### When to Use Overrides

✅ **Good Use Cases**:
- Connector-specific test cards
- Regional payment method variations
- Known connector limitations
- Different error message formats

❌ **Avoid**:
- Overriding core business logic
- Masking real bugs
- Making tests too connector-specific

### Organizing Overrides

1. **Group by connector**: One file per connector
2. **Document reasons**: Add comments explaining why override is needed
3. **Keep minimal**: Only override what's necessary
4. **Version control**: Track overrides in git

### Testing Overrides

Always test overrides independently:

```bash
# Test with override
cargo run --bin run_test -- --connector stripe --suite authorize

# Test without override (base scenario)
cargo run --bin run_test -- --connector adyen --suite authorize
```

## Future Enhancements

Planned features for overrides:

1. **Dedicated override files** with automatic loading
2. **Override inheritance** (common → connector-specific)
3. **Conditional overrides** based on environment
4. **Override templates** for common patterns
5. **Visual override editor** in test reports

## Next Steps

- [Configuration](./configuration.md) - Environment and credentials setup
- [Global Suites](./global-suites.md) - Understanding base scenarios
- [Usage](./usage.md) - Running tests with different configurations
