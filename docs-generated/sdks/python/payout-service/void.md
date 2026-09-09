# void Method

<!--
---
title: void (Python SDK)
description: Cancel a pending payout. Using the Python SDK.
last_updated: 2026-08-13
generated_from: backend/grpc-api-types/proto/services.proto
auto_generated: true
reviewed_by: ''
reviewed_at: ''
approved: false
sdk_language: python
---
-->

## Overview

The `void` method cancels a payout that has been initiated but not yet completed or transferred. This is used when a payout needs to be stopped before the funds leave the merchant's account.

## Purpose

Use this operation to halt a payout process.

| Scenario | Developer Implementation |
|----------|--------------------------|
| Cancel a scheduled payout | Call `void` with the `merchant_payout_id` and required details. |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_payout_id` | string | No | Your internal identifier for the payout. |
| `address` | PayoutAddress | Yes | Address information associated with the payout. |
| `connector_feature_data` | SecretString | No | Connector-specific metadata or feature configurations. |
| `connector_payout_id` | string | No | The unique payout ID assigned by the connector. |
| `access_token` | SecretString | No | Access token for the connector, if required. |

## Response Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_payout_id` | string | No | Your internal identifier passed to the payout processor. |
| `payout_status` | PayoutEnums.PayoutStatus | No | The current status of the payout (e.g., CANCELLED). |
| `connector_payout_id` | string | No | The unique payout ID assigned by the connector. |
| `error` | ErrorInfo | No | Details of any error that occurred. |
| `status_code` | uint32 | Yes | The HTTP status code returned from the connector. |

## Example

### SDK Setup

```python
from hyperswitch_prism import PayoutClient

payout_client = PayoutClient(
    connector='stripe',
    api_key='[REDACTED_ENV_SECRET]',
    environment='SANDBOX'
)
```

### Request

```python
request = {
    "connector_payout_id": "po_1Hh1XYZ2eZvKYlo2C"
}

response = await payout_client.void(request)
```

### Response

```python
{
    "payout_status": "CANCELLED",
    "connector_payout_id": "po_1Hh1XYZ2eZvKYlo2C",
    "status_code": 200
}
```

## Next Steps

- [Get Payout Status](./get.md)
