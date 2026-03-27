# create_access_token Method

<!--
---
title: create_access_token (Python SDK)
description: Generate short-lived API access token using the Python SDK
last_updated: 2026-03-21
generated_from: backend/grpc-api-types/proto/services.proto
auto_generated: true
reviewed_by: ''
reviewed_at: ''
approved: false
sdk_language: python
---
-->

## Overview

The `create_access_token` method generates a short-lived authentication token for accessing payment processor APIs.

**Business Use Case:** Your frontend needs to initialize a payment widget. Generate a temporary token for client-side use.

## Purpose

| Scenario | Benefit |
|----------|---------|
| Frontend SDKs | Secure client-side initialization |
| Delegated access | Scoped tokens for third parties |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `scope` | str | No | Token scope |
| `expires_in` | int | No | Token lifetime in seconds |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `access_token` | str | The token string |
| `token_type` | str | Bearer |
| `expires_in` | int | Seconds until expiry |
| `status_code` | int | HTTP status code |

## Example

### SDK Setup

```python
from hyperswitch_prism import MerchantAuthenticationClient

auth_client = MerchantAuthenticationClient(
    connector='stripe',
    api_key='YOUR_API_KEY',
    environment='SANDBOX'
)
```

### Request

```python
request = {
    "scope": "payment:write",
    "expires_in": 3600
}

response = await auth_client.create_access_token(request)
```

### Response

```python
{
    "access_token": "sk_test_xxx",
    "token_type": "Bearer",
    "expires_in": 3600,
    "status_code": 200
}
```

## Next Steps

- [create_session_token](./create-session-token.md) - Create session tokens