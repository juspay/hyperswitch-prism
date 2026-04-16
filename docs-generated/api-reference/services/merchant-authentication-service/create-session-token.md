# CreateSessionToken RPC

<!--
---
title: CreateSessionToken
description: Create session token for payment processing to maintain state across multiple operations
last_updated: 2026-03-11
generated_from: crates/types-traits/grpc-api-types/proto/services.proto
auto_generated: false
reviewed_by: engineering
reviewed_at: 2026-03-05
approved: true
---
-->

## Overview

The `CreateSessionToken` RPC creates a session token for payment processing. This token maintains state across multiple payment operations, enabling secure tracking and improved security for multi-step payment flows.

**Business Use Case:** When processing payments that require multiple steps (3DS authentication, redirect flows, wallet payments), you need to maintain session state between requests. This RPC creates a session token that carries context through the entire payment journey.

## Purpose

**Why use session tokens?**

| Scenario | Session Token Benefit |
|----------|----------------------|
| **3DS authentication** | Maintain context through challenge flow |
| **Redirect payments** | Preserve state during bank redirects |
| **Multi-step checkout** | Track progress across pages |
| **Security** | Bind payment to specific session |

**Key outcomes:**
- Session-scoped payment context
- Secure state management
- Cross-request continuity
- Enhanced fraud protection

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_session_id` | string | Yes | Your unique session reference |
| `amount` | Money | Yes | Payment amount for this session |
| `metadata` | SecretString | No | Additional metadata for the connector |
| `connector_feature_data` | SecretString | No | Connector-specific metadata |
| `state` | ConnectorState | No | Existing state to continue session |
| `browser_info` | BrowserInformation | No | Browser details for fraud detection |
| `test_mode` | bool | No | Use test/sandbox environment |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `error` | ErrorInfo | Error details if creation failed |
| `status_code` | uint32 | HTTP-style status code |
| `session_token` | string | Session token for subsequent operations |

## Example

### Request (grpcurl)

```bash
grpcurl -H "x-connector: stripe" \
  -H "x-connector-config: {\"config\":{\"Stripe\":{\"api_key\":\"$STRIPE_API_KEY\"}}}" \
  -d '{
    "merchant_session_id": "session_001",
    "amount": {
      "minor_amount": 10000,
      "currency": "USD"
    },
    "browser_info": {
      "accept_header": "text/html",
      "user_agent": "Mozilla/5.0..."
    },
    "test_mode": true
  }' \
  localhost:8080 \
  types.MerchantAuthenticationService/CreateSessionToken
```

### Response

```json
{
  "session_token": "sess_1234567890abcdef",
  "status_code": 200
}
```

## Next Steps

- [CreateAccessToken](./create-access-token.md) - Generate API access tokens
- [CreateSdkSessionToken](./create-sdk-session-token.md) - Initialize wallet sessions
- [Payment Service](../payment-service/README.md) - Process payments using session
