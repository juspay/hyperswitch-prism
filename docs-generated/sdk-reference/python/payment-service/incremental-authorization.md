# incremental_authorization Method

<!--
---
title: incremental_authorization (Python SDK)
description: Increase authorized amount using the Python SDK
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

The `incremental_authorization` method increases the authorized amount on an existing authorization. Use for hospitality, tips, or add-on charges.

**Business Use Case:** A hotel guest adds room service charges. Increase the authorization hold to cover additional charges.

## Purpose

| Scenario | Benefit |
|----------|---------|
| Hotel incidentals | Add room service charges |
| Restaurant tips | Add post-dining tip |
| Add-on services | Cover additional costs |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_authorization_id` | str | Yes | Your unique incremental auth ID |
| `connector_transaction_id` | str | Yes | Original authorization ID |
| `amount` | Money | Yes | New total amount (not incremental) |
| `reason` | str | No | Reason for increase |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `connector_authorization_id` | str | Connector's authorization ID |
| `status` | AuthorizationStatus | AUTHORIZED, FAILED |
| `status_code` | int | HTTP status code |

## Example

### SDK Setup

```python
from hyperswitch_prism import PaymentClient

payment_client = PaymentClient(
    connector='stripe',
    api_key='YOUR_API_KEY',
    environment='SANDBOX'
)
```

### Request

```python
request = {
    "merchant_authorization_id": "incr_auth_001",
    "connector_transaction_id": "pi_3Oxxx...",
    "amount": {
        "minor_amount": 1500,  # New total: $15.00
        "currency": "USD"
    },
    "reason": "Room service charges added"
}

response = await payment_client.incremental_authorization(request)
```

### Response

```python
{
    "connector_authorization_id": "pi_3Oxxx...",
    "status": "AUTHORIZED",
    "status_code": 200
}
```

## Next Steps

- [authorize](./authorize.md) - Create initial authorization
- [capture](./capture.md) - Finalize with increased amount