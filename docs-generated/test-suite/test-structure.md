<!--
---
title: Test Structure
description: Scenarios, suites, assertions, and test data format
last_updated: 2026-03-12
generated_from: backend/integration-tests/src/global_suites/
auto_generated: false
reviewed_by: engineering
reviewed_at: 2026-03-12
approved: true
---
-->

# Test Structure

## Overview

Tests in the Connector Service Test Suite are organized into **suites**, each containing multiple **scenarios**. A scenario defines a single test case with a request payload and assertion rules.

## Scenario Definition

### File Location

Each suite has a `scenario.json` file containing test cases:

```
src/global_suites/{suite_name}_suite/scenario.json
```

### Scenario Format

```json
{
  "scenario_name": {
    "grpc_req": {
      // Request payload sent to gRPC service
    },
    "assert": {
      // Assertion rules for validating response
    },
    "is_default": true
  }
}
```

### Example Scenario

```json
{
  "no3ds_auto_capture_credit_card": {
    "grpc_req": {
      "merchant_transaction_id": {"id": "auto_generate"},
      "amount": {"minor_amount": 6000, "currency": "USD"},
      "order_tax_amount": 0,
      "shipping_cost": 0,
      "payment_method": {
        "card": {
          "card_number": {"value": "4111111111111111"},
          "card_exp_month": {"value": "08"},
          "card_exp_year": {"value": "30"},
          "card_cvc": {"value": "999"},
          "card_holder_name": {"value": "auto_generate"},
          "card_type": "credit"
        }
      },
      "capture_method": "AUTOMATIC",
      "customer": {
        "name": "auto_generate",
        "email": {"value": "auto_generate"},
        "id": "auto_generate",
        "phone_number": "auto_generate"
      },
      "address": {
        "shipping_address": {
          "first_name": {"value": "auto_generate"},
          "last_name": {"value": "auto_generate"},
          "line1": {"value": "auto_generate"},
          "city": {"value": "auto_generate"},
          "state": {"value": "CA"},
          "zip_code": {"value": "auto_generate"},
          "country_alpha2_code": "US"
        },
        "billing_address": {
          "first_name": {"value": "auto_generate"},
          "last_name": {"value": "auto_generate"},
          "line1": {"value": "auto_generate"},
          "city": {"value": "auto_generate"},
          "state": {"value": "CA"},
          "zip_code": {"value": "auto_generate"},
          "country_alpha2_code": "US"
        }
      }
    },
    "assert": {
      "status": {"one_of": ["CHARGED", "AUTHORIZED"]},
      "connector_transaction_id": {"must_exist": true},
      "error": {"must_not_exist": true}
    },
    "is_default": true
  }
}
```

## Suite Specification

### File Location

```
src/global_suites/{suite_name}_suite/suite_spec.json
```

### Suite Spec Format

```json
{
  "suite": "authorize",
  "suite_type": "dependent",
  "depends_on": ["create_access_token", "create_customer"],
  "strict_dependencies": false
}
```

### Suite Spec Fields

| Field | Type | Description |
|-------|------|-------------|
| `suite` | string | Suite name (must match directory name) |
| `suite_type` | string | `independent` or `dependent` |
| `depends_on` | string[] | List of suites that must run first |
| `strict_dependencies` | boolean | If `true`, fail if dependencies fail |

### Suite Types

| Type | Description | Example |
|------|-------------|---------|
| **Independent** | No dependencies, can run standalone | `create_access_token` |
| **Dependent** | Requires other suites to run first | `authorize` depends on `create_access_token` |

## Assertion Types

The test framework supports six assertion rules for validating responses:

### Must Exist

Field must be present in the response:

```json
{
  "connector_transaction_id": {"must_exist": true}
}
```

### Must Not Exist

Field must be absent from the response:

```json
{
  "error_code": {"must_not_exist": true}
}
```

### Equals

Field must equal a specific value:

```json
{
  "status": {"equals": "SUCCESS"},
  "amount.minor_amount": {"equals": 6000}
}
```

### One Of

Field must match one of the allowed values:

```json
{
  "status": {"one_of": ["CHARGED", "AUTHORIZED", "PENDING"]},
  "payment_method_type": {"one_of": ["credit", "debit"]}
}
```

### Contains

String field must contain a substring:

```json
{
  "error_message": {"contains": "declined"},
  "error.connector_details.message": {"contains": "card_error"}
}
```

### Echo

Field must match the corresponding field from the request:

```json
{
  "merchant_transaction_id": {"echo": "merchant_transaction_id"},
  "amount.minor_amount": {"echo": "amount.minor_amount"}
}
```

### Complete Assertion Example

```json
{
  "assert": {
    "status": {"one_of": ["CHARGED", "AUTHORIZED"]},
    "connector_transaction_id": {"must_exist": true},
    "connector_transaction_id.id": {"must_exist": true},
    "error": {"must_not_exist": true},
    "error_code": {"must_not_exist": true},
    "error_message": {"must_not_exist": true},
    "merchant_transaction_id": {"echo": "merchant_transaction_id"},
    "amount.minor_amount": {"echo": "amount.minor_amount"},
    "amount.currency": {"echo": "amount.currency"}
  }
}
```

## Auto-Generated Values

Use `"auto_generate"` for dynamic test data that should be unique per test run.

### Auto-Generate Placeholders

| Field Type | Generated Format | Example |
|------------|------------------|---------|
| **Transaction IDs** | `{prefix}_{uuid}` | `mti_a1b2c3d4`, `mri_e5f6g7h8` |
| **Customer Emails** | `{name}.{number}@{domain}` | `alex.1234@example.com` |
| **Phone Numbers** | `{country_code}{number}` | `+15551234567` |
| **Names** | `{First} {Last}` | `Emma Johnson` |
| **Addresses** | `{number} {street} {suffix}` | `123 Main St` |
| **Cities** | Random US city | `San Francisco` |
| **ZIP Codes** | 5-digit number | `94102` |

### ID Prefixes

| Field Path | Prefix | Example |
|------------|--------|---------|
| `merchant_transaction_id.id` | `mti` | `mti_a1b2c3d4` |
| `merchant_refund_id.id` | `mri` | `mri_b2c3d4e5` |
| `merchant_capture_id.id` | `mci` | `mci_c3d4e5f6` |
| `merchant_customer_id.id` | `mcui` | `mcui_d4e5f6g7` |
| `merchant_access_token_id.id` | `mati` | `mati_e5f6g7h8` |
| `customer.id` | `cust` | `cust_f6g7h8i9` |

### Example with Auto-Generate

```json
{
  "grpc_req": {
    "merchant_transaction_id": {"id": "auto_generate"},
    "customer": {
      "name": "auto_generate",
      "email": {"value": "auto_generate"},
      "id": "auto_generate",
      "phone_number": "auto_generate"
    },
    "address": {
      "shipping_address": {
        "first_name": {"value": "auto_generate"},
        "last_name": {"value": "auto_generate"},
        "line1": {"value": "auto_generate"},
        "city": {"value": "auto_generate"},
        "zip_code": {"value": "auto_generate"}
      }
    }
  }
}
```

## Deferred Fields

Some fields are not auto-generated but resolved from **dependency responses**:

| Field Path | Resolved From |
|------------|---------------|
| `customer.connector_customer_id` | `create_customer` response |
| `state.connector_customer_id` | `create_customer` response |
| `state.access_token.token.value` | `create_access_token` response |
| `state.access_token.token_type` | `create_access_token` response |
| `connector_transaction_id.id` | `authorize` response |
| `refund_id` | `refund` response |

These fields are populated automatically when running dependent suites.

## Test Card Numbers

Use these standard test card numbers:

| Card Number | Type | Expected Result |
|-------------|------|----------------|
| `4111111111111111` | Visa Success | Successful authorization |
| `4000000000000002` | Generic Decline | Declined payment |
| `4000000000003220` | 3DS2 Frictionless | 3D Secure flow |

## Next Steps

- [Global Suites](./global-suites.md) - Reusable test scenarios
- [Configuration](./configuration.md) - Test data and credentials
- [Usage](./usage.md) - Running tests
