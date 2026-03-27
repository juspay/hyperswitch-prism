<!--
---
title: Test Configuration
description: Test data, credentials, and environment setup
last_updated: 2026-03-12
generated_from: backend/integration-tests/src/harness/credentials.rs
auto_generated: false
reviewed_by: engineering
reviewed_at: 2026-03-12
approved: true
---
-->

# Configuration

## Overview

Test configuration is split across three locations:

1. **Global Suites** - Reusable test scenarios
2. **Connector Specs** - Per-connector suite configuration
3. **Credentials** - API keys and authentication secrets (external file)

## Test Data Locations

| Location | Purpose | Files |
|----------|---------|-------|
| **Global Suites** | Reusable scenarios for all connectors | `src/global_suites/*/scenario.json` |
| **Connector Specs** | Per-connector suite support | `src/connector_specs/{connector}.json` |
| **Credentials** | API keys and secrets | External JSON file (via env var) |

## Environment Variables

### Required Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `CONNECTOR_AUTH_FILE_PATH` | Path to credentials JSON file | `/path/to/creds.json` |

### Alternative Variables

| Variable | Description | Fallback |
|----------|-------------|----------|
| `UCS_CREDS_PATH` | Alternative credentials path | `CONNECTOR_AUTH_FILE_PATH` |

### Optional Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `UCS_SCENARIO_ROOT` | Override scenario files location | `src/global_suites/` |
| `UCS_RUN_TEST_REPORT_PATH` | Custom report output path | `report.json` |
| `UCS_ALL_CONNECTORS` | Comma-separated connector list | `stripe,paypal,authorizedotnet` |
| `UCS_DEBUG_EFFECTIVE_REQ` | Print request JSON | unset |
| `UCS_RUN_TEST_DEFAULTS_PATH` | Override defaults config | unset |

### Connector Label Selection

Select specific account configuration for connectors with multiple setups:

```bash
export UCS_CONNECTOR_LABEL_CYBERSOURCE=connector_2
export UCS_CONNECTOR_LABEL_STRIPE=sandbox_account
```

## Credentials Configuration

### Credentials File Location

The framework searches for credentials in this order:

1. `CONNECTOR_AUTH_FILE_PATH` environment variable
2. `UCS_CREDS_PATH` environment variable
3. Default: `creds.json` in the repo root

### Credentials File Format

```json
{
  "stripe": {
    "connector_account_details": {
      "auth_type": "HeaderKey",
      "api_key": "sk_test_..."
    }
  },
  "adyen": {
    "connector_account_details": {
      "auth_type": "SignatureKey",
      "api_key": "...",
      "key1": "...",
      "api_secret": "..."
    }
  },
  "braintree": {
    "connector_1": {
      "connector_account_details": {
        "auth_type": "BodyKey",
        "api_key": "...",
        "key1": "..."
      }
    }
  }
}
```

### Authentication Types

| Type | Fields | Use Case | Example Connectors |
|------|--------|----------|-------------------|
| **HeaderKey** | `api_key` | Simple API key auth | Stripe, PayPal |
| **BodyKey** | `api_key`, `key1` | Key pair auth | Braintree, Authorize.Net |
| **SignatureKey** | `api_key`, `key1`, `api_secret` | HMAC signed requests | Adyen, Checkout.com |

### HeaderKey Example

```json
{
  "stripe": {
    "connector_account_details": {
      "auth_type": "HeaderKey",
      "api_key": "sk_test_51H..."
    }
  }
}
```

### BodyKey Example

```json
{
  "braintree": {
    "connector_account_details": {
      "auth_type": "BodyKey",
      "api_key": "merchant_id_123",
      "key1": "public_key_abc"
    }
  }
}
```

### SignatureKey Example

```json
{
  "adyen": {
    "connector_account_details": {
      "auth_type": "SignatureKey",
      "api_key": "AQE...",
      "key1": "merchant_account_name",
      "api_secret": "secret_for_signing"
    }
  }
}
```

### Multiple Connector Configurations

For connectors with multiple accounts:

```json
{
  "cybersource": {
    "connector_1": {
      "connector_account_details": {
        "auth_type": "SignatureKey",
        "api_key": "sandbox_key",
        "key1": "sandbox_merchant",
        "api_secret": "sandbox_secret"
      }
    },
    "connector_2": {
      "connector_account_details": {
        "auth_type": "SignatureKey",
        "api_key": "production_key",
        "key1": "production_merchant",
        "api_secret": "production_secret"
      }
    }
  }
}
```

Select the active configuration:

```bash
export UCS_CONNECTOR_LABEL_CYBERSOURCE=connector_1
```

### Account Selection Priority

When multiple accounts exist, the framework selects in this order:

1. Environment variable: `UCS_CONNECTOR_LABEL_{CONNECTOR}`
2. Connector-specific preferred labels (e.g., `connector_1` for most, `connector_2` for Cybersource)
3. First account with `connector_account_details`

## Test Data Configuration

### Global Suite Data

Each scenario in `scenario.json` defines:

```json
{
  "scenario_name": {
    "grpc_req": {
      // Request payload
    },
    "assert": {
      // Assertion rules
    },
    "is_default": true
  }
}
```

### Auto-Generated Values

Use `"auto_generate"` for dynamic data:

| Field Type | Generated Format | Example |
|------------|------------------|---------|
| Transaction IDs | `{prefix}_{uuid}` | `mti_a1b2c3d4` |
| Customer Emails | `{name}.{number}@{domain}` | `alex.1234@example.com` |
| Phone Numbers | `{country_code}{number}` | `+15551234567` |
| Names | `{First} {Last}` | `Emma Johnson` |
| Addresses | `{number} {street} {suffix}` | `123 Main St` |

### Deferred Fields

Fields resolved from dependency responses (not auto-generated):

| Field | Resolved From |
|-------|---------------|
| `customer.connector_customer_id` | `create_customer` response |
| `state.access_token.token.value` | `create_access_token` response |
| `connector_transaction_id.id` | `authorize` response |
| `refund_id` | `refund` response |

## Connector Specifications

### Purpose

Define which suites each connector supports:

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

### File Location

```
src/connector_specs/{connector}.json
```

### Benefits

- Skip unsupported suites during `--all` runs
- Document connector capabilities
- Enable gradual rollout of new tests

## Setup Checklist

Before running tests:

- [ ] Create credentials JSON file
- [ ] Set `CONNECTOR_AUTH_FILE_PATH` environment variable
- [ ] Verify connector credentials are valid
- [ ] Ensure test endpoints are accessible
- [ ] (Optional) Configure `UCS_SCENARIO_ROOT` if using custom scenarios

## Example Setup

```bash
# 1. Create credentials file
cat > ~/.ucs-creds.json << 'EOF'
{
  "stripe": {
    "connector_account_details": {
      "auth_type": "HeaderKey",
      "api_key": "sk_test_..."
    }
  }
}
EOF

# 2. Set environment variable
export CONNECTOR_AUTH_FILE_PATH=~/.ucs-creds.json

# 3. Run tests
cargo run --bin run_test -- --connector stripe
```

## Security Best Practices

1. **Never commit credentials**: Keep credentials file out of git
2. **Use environment variables**: Load credentials via env vars
3. **Use test/sandbox accounts**: Never use production credentials
4. **Rotate regularly**: Update API keys periodically
5. **Limit access**: Restrict credentials file permissions

```bash
# Set restrictive permissions
chmod 600 ~/.ucs-creds.json

# Add to .gitignore
echo "*.creds.json" >> .gitignore
```

## Next Steps

- [Global Suites](./global-suites.md) - Understanding test scenarios
- [Usage](./usage.md) - Running tests
- [Overrides](./overrides.md) - Customizing scenarios
