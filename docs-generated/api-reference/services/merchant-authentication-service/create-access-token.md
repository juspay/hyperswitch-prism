# CreateAccessToken RPC

<!--
---
title: CreateAccessToken
description: Generate short-lived connector authentication token for secure client-side API access
last_updated: 2026-03-11
generated_from: crates/types-traits/grpc-api-types/proto/services.proto
auto_generated: false
reviewed_by: engineering
reviewed_at: 2026-03-05
approved: true
---
-->

## Overview

The `CreateAccessToken` RPC generates a short-lived authentication token for connector API access. These tokens expire quickly (typically 1 hour) and can be safely used in client applications without exposing your main API credentials.

**Business Use Case:** When building client-side payment flows (browser checkout, mobile apps), you need to give clients limited access to the payment processor without exposing your full API keys. This RPC generates temporary tokens that clients can use for operations like card tokenization.

## Purpose

**Why use short-lived access tokens?**

| Scenario | Risk Without Tokens | Solution |
|----------|---------------------|----------|
| **Browser checkout** | API keys exposed in JavaScript | Temporary token with limited scope |
| **Mobile apps** | API keys in app bundle | Token generated per session |
| **Third-party integrations** | Full API access granted | Scoped token with expiration |

**Key outcomes:**
- Temporary access token (1 hour typical)
- Limited scope permissions
- Safe for client-side use
- Automatic expiration

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_access_token_id` | string | Yes | Your unique token reference |
| `connector` | Connector | Yes | Target connector (STRIPE, ADYEN, etc.) |
| `metadata` | SecretString | No | Additional metadata for the connector |
| `connector_feature_data` | SecretString | No | Connector-specific metadata |
| `test_mode` | bool | No | Generate test/sandbox token |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `access_token` | SecretString | The access token string (e.g., "pk_live_...") |
| `token_type` | string | Token type (e.g., "Bearer", "Basic") |
| `expires_in_seconds` | int64 | Expiration timestamp (Unix epoch) |
| `status` | OperationStatus | Status of token creation |
| `error` | ErrorInfo | Error details if creation failed |
| `status_code` | uint32 | HTTP-style status code |
| `merchant_access_token_id` | string | Your token reference (echoed back) |

## Example

### Request (grpcurl)

```bash
grpcurl -H "x-connector: stripe" \
  -H "x-connector-config: {\"config\":{\"Stripe\":{\"api_key\":\"$STRIPE_API_KEY\"}}}" \
  -d '{
    "merchant_access_token_id": "token_001",
    "connector": "STRIPE",
    "test_mode": true
  }' \
  localhost:8080 \
  types.MerchantAuthenticationService/CreateAccessToken
```

### Response

```json
{
  "access_token": "pk_test_1234567890abcdef",
  "token_type": "Bearer",
  "expires_in_seconds": 1704153600,
  "status": "SUCCESS",
  "status_code": 200
}
```

## Next Steps

- [CreateSessionToken](./create-session-token.md) - Create session tokens for payment flows
- [CreateSdkSessionToken](./create-sdk-session-token.md) - Initialize wallet payment sessions
