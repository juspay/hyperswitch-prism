# Webhook Testing Suite (handle_event)

This suite tests webhook event handling via `EventService.HandleEvent` on port 50052.

## Architecture

Webhook tests use the **same override system** as every other suite.  There is no
special-case branch in `scenario_api.rs` for webhooks.

```
scenario.json          (generic template with grpc_req placeholders)
     + override.json            (assertion overrides, e.g. source_verified)
     + webhook_payload.json     (connector-specific body, headers, metadata)
     ↓
apply_connector_overrides()     (json_merge_patch + post-merge webhook transforms)
     ↓
EventServiceHandleRequest       (body base64-encoded, HMAC signature computed)
```

### Generic Scenarios (`scenario.json`)

Contains **connector-agnostic** test scenario templates:
- `payment_succeeded` — Successful payment/authorization webhook
- `payment_failed` — Failed payment webhook
- `refund_succeeded` — Refund webhook
- `invalid_signature` — Signature verification failure

Each scenario has a `grpc_req` template with empty `request_details.body` and
`request_details.headers` placeholders, plus default assertions.

### Connector-Specific Payloads (`connector_specs/<connector>/webhook_payload.json`)

Each connector has a `webhook_payload.json` under its connector_specs directory:
- `connector_specs/authorizedotnet/webhook_payload.json`
- `connector_specs/adyen/webhook_payload.json`
- `connector_specs/stripe/webhook_payload.json`
- `connector_specs/paypal/webhook_payload.json`

These files follow the same merge-patch structure as `override.json`:
scenario names as keys, each containing a `grpc_req` patch with the connector's
actual webhook body (as readable JSON), headers, and `merchant_event_id`.

An optional `_webhook_config` key holds connector-level metadata (signature
header name, algorithm, secret key name, etc.) used by post-merge transforms.

### Assertion Overrides (`connector_specs/<connector>/override.json`)

Standard `override.json` entries under `handle_event.<scenario>` override
assertions, e.g. setting `source_verified: { "must_not_exist": true }` for
Stripe (empty IncomingWebhook impl) and PayPal (external verification).

## How It Works

1. Test runner loads generic `scenario.json` template (has `grpc_req` with placeholders)
2. `apply_connector_overrides()` applies `override.json` patches (assertion overrides)
3. `apply_connector_overrides()` loads `webhook_payload.json` and merges `grpc_req` patch
4. Post-merge transform:
   - Serializes `request_details.body` (JSON object) to string and base64-encodes it
   - Computes HMAC signature using `webhook_signatures` module if webhook secret exists
   - Injects `webhook_secrets` at the top level of the request
5. Test runner sends completed `EventServiceHandleRequest` to `EventService.HandleEvent`
6. Assertions from `scenario.json` + `override.json` are evaluated against the response

## Adding a New Connector

### Step 1: Create `webhook_payload.json`

Create `connector_specs/new_connector/webhook_payload.json`:

```json
{
  "_webhook_config": {
    "signature_header": "X-Connector-Signature",
    "signature_algorithm": "new_connector_hmac_sha256",
    "webhook_secret_key": "webhook_secret"
  },
  "payment_succeeded": {
    "grpc_req": {
      "request_details": {
        "headers": {},
        "body": {
          "event": "payment.success",
          "data": { "id": "txn_123", "amount": 2000 }
        }
      },
      "merchant_event_id": "new_connector_payment_001"
    }
  },
  "refund_succeeded": {
    "grpc_req": {
      "request_details": {
        "headers": {},
        "body": {
          "event": "refund.success",
          "data": { "id": "ref_123" }
        }
      },
      "merchant_event_id": "new_connector_refund_001"
    }
  },
  "invalid_signature": {
    "grpc_req": {
      "request_details": {
        "headers": {
          "X-Connector-Signature": "invalid_signature_value"
        },
        "body": {
          "event": "payment.success",
          "data": { "id": "invalid" }
        }
      },
      "merchant_event_id": "new_connector_invalid_sig"
    }
  }
}
```

The `body` is stored as readable JSON; the override system automatically
serializes and base64-encodes it before sending.

### Step 2: Add Signature Generation

In `src/webhook_signatures.rs`, add a match arm for the new connector.

### Step 3: Add to Connector Specs

In `connector_specs/new_connector/specs.json`, add `"handle_event"` to
`supported_suites`.

### Step 4: Add Assertion Overrides (if needed)

If the connector has structural reasons why `source_verified` won't be true
(e.g. empty IncomingWebhook impl, external verification), add to
`connector_specs/new_connector/override.json`:

```json
{
  "handle_event": {
    "payment_succeeded": {
      "assert": {
        "source_verified": { "must_not_exist": true }
      }
    }
  }
}
```

### Step 5: Test

```bash
cargo run --bin test_ucs -- \
  --connector new_connector \
  --suite handle_event \
  --scenario payment_succeeded \
  --endpoint localhost:50052
```

## `webhook_payload.json` Format

```json
{
  "_webhook_config": {
    "signature_header": "Header-Name",
    "signature_location": "header|body",
    "signature_algorithm": "algorithm_name",
    "signature_format": "format_string",
    "signature_encoding": "hex|base64",
    "webhook_secret_key": "secret_key_name",
    "requires_external_verification": false
  },
  "scenario_name": {
    "grpc_req": {
      "request_details": {
        "headers": {},
        "body": { ... }
      },
      "merchant_event_id": "unique_id"
    }
  }
}
```

## Signature Algorithms

| Connector | Algorithm | Format | Header |
|-----------|-----------|--------|--------|
| Stripe | HMAC-SHA256 | `t={timestamp},v1={hex}` | `Stripe-Signature` |
| Authorize.Net | HMAC-SHA512 | `sha512={hex_lowercase}` | `X-ANET-Signature` |
| PayPal | HMAC-SHA256 | base64 | `PAYPAL-TRANSMISSION-SIG` |
| Adyen | HMAC-SHA256 | base64 (in body) | N/A (in `additionalData.hmacSignature`) |
