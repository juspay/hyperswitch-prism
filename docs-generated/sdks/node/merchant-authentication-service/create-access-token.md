# createAccessToken Method

<!--
---
title: createAccessToken (Node.js SDK)
description: Generate short-lived API access token using the Node.js SDK
last_updated: 2026-03-21
generated_from: backend/grpc-api-types/proto/services.proto
auto_generated: true
reviewed_by: ''
reviewed_at: ''
approved: false
sdk_language: node
---
-->

## Overview

The `createAccessToken` method generates a short-lived authentication token for accessing payment processor APIs. Use this for temporary, secure access without exposing long-lived credentials.

**Business Use Case:** Your frontend needs to initialize a payment widget. Generate a temporary token with limited scope and short expiry for client-side use.

## Purpose

| Scenario | Benefit |
|----------|---------|
| Frontend SDKs | Secure client-side initialization |
| Delegated access | Scoped tokens for third parties |
| Session-based auth | Time-limited access |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `scope` | string | No | Token scope (e.g., "payment:write") |
| `expiresIn` | number | No | Token lifetime in seconds (default: 3600) |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `accessToken` | string | The token string to use in API calls |
| `tokenType` | string | Bearer |
| `expiresIn` | number | Seconds until expiry |
| `expiresAt` | string | ISO 8601 expiry timestamp |
| `statusCode` | number | HTTP status code |

## Example

### SDK Setup

```javascript
const { MerchantAuthenticationClient } = require('hyperswitch-prism');

const authClient = new MerchantAuthenticationClient({
    connector: 'stripe',
    apiKey: 'YOUR_API_KEY',
    environment: 'SANDBOX'
});
```

### Request

```javascript
const request = {
    scope: "payment:write",
    expiresIn: 3600
};

const response = await authClient.createAccessToken(request);
```

### Response

```javascript
{
    accessToken: "sk_test_xxx",
    tokenType: "Bearer",
    expiresIn: 3600,
    expiresAt: "2024-01-15T11:30:00Z",
    statusCode: 200
}
```

## Security Best Practices

- Use short expiry times (1 hour or less)
- Transmit only over HTTPS
- Never store tokens client-side long-term
- Implement token refresh logic

## Next Steps

- [createSessionToken](./create-session-token.md) - Create session tokens for multi-step flows
- [Payment Service](../payment-service/README.md) - Use tokens for payment operations
