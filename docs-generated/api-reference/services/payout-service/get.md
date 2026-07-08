---
title: Get Payout
description: Retrieve the current status and details of a payout.
---

# Get Payout

## Overview

The `Get` RPC allows you to check the current status of a payout. This is essential for syncing your internal systems with the payment processor's state, especially for asynchronous payout methods like bank transfers.

## Purpose

Use this operation to poll for the status of a payout or to verify details before taking further action.

| Scenario | Developer Implementation |
|----------|--------------------------|
| Check if a payout succeeded | Call `Get` with the `merchant_payout_id` or `connector_payout_id`. |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_payout_id` | string | No | Your internal identifier for the payout. |
| `connector_payout_id` | string | No | The unique payout ID assigned by the connector. |
| `access_token` | SecretString | No | Access token for the connector, if required. |

## Response Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_payout_id` | string | No | Your internal identifier passed to the payout processor. |
| `payout_status` | PayoutEnums.PayoutStatus | No | The current status of the payout. |
| `connector_payout_id` | string | No | The unique payout ID assigned by the connector. |
| `error` | ErrorInfo | No | Details of any error that occurred. |
| `status_code` | uint32 | Yes | The HTTP status code returned from the connector. |

## Example

```bash
grpcurl -H "x-connector: stripe" \
  -H "x-connector-config: {\"config\":{\"Stripe\":{\"api_key\":\"$STRIPE_API_KEY\"}}}" \
  -d '{
    "merchant_payout_id": "po_internal_12345"
  }' \
  localhost:8080 types.PayoutService/Get
```

```json
{
  "merchant_payout_id": "po_internal_12345",
  "payout_status": "SUCCESS",
  "connector_payout_id": "po_1Hh1XYZ2eZvKYlo2C",
  "status_code": 200
}
```

## Next Steps

- [Void Payout](./void.md)
