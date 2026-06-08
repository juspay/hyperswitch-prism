---
title: Void Payout
description: Cancel a pending payout.
---

# Void Payout

## Overview

The `Void` RPC cancels a payout that has been initiated but not yet completed or transferred. This is used when a payout needs to be stopped before the funds leave the merchant's account.

## Purpose

Use this operation to halt a payout process.

| Scenario | Developer Implementation |
|----------|--------------------------|
| Cancel a scheduled payout | Call `Void` with the `merchant_payout_id` and required details. |

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

```bash
grpcurl -H "x-connector: stripe" \
  -H "x-connector-config: {\"config\":{\"Stripe\":{\"api_key\":\"$STRIPE_API_KEY\"}}}" \
  -d '{
    "connector_payout_id": "po_1Hh1XYZ2eZvKYlo2C"
  }' \
  localhost:8080 types.PayoutService/Void
```

```json
{
  "payout_status": "CANCELLED",
  "connector_payout_id": "po_1Hh1XYZ2eZvKYlo2C",
  "status_code": 200
}
```

## Next Steps

- [Get Payout Status](./get.md)
