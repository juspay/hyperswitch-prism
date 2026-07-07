---
title: Stage Payout
description: Stage a payout for processing.
---

# Stage Payout

## Overview

The `Stage` RPC allows you to prepare or stage a payout before it is executed. This can be used in workflows where payouts are reviewed or batched before final transfer.

## Purpose

Use this operation to put a payout into a staged state.

| Scenario | Developer Implementation |
|----------|--------------------------|
| Prepare a payout for review | Call `Stage` with the quote details and amount. |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_quote_id` | string | No | Your internal quote identifier. |
| `address` | PayoutAddress | Yes | Address information associated with the payout. |
| `amount` | Money | Yes | The amount to be staged. |
| `destination_currency` | Currency | Yes | The currency in which the payout will be received. |
| `customer` | Customer | No | Details about the customer. |
| `browser_info` | BrowserInformation | No | Information about the user's browser. |
| `access_token` | SecretString | No | Access token for the connector, if required. |

## Response Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_payout_id` | string | No | Your internal identifier passed to the payout processor. |
| `payout_status` | PayoutEnums.PayoutStatus | No | The current status of the staged payout. |
| `connector_payout_id` | string | No | The unique payout ID assigned by the connector. |
| `error` | ErrorInfo | No | Details of any error that occurred. |
| `status_code` | uint32 | Yes | The HTTP status code returned from the connector. |

## Example

```bash
grpcurl -H "x-connector: stripe" \
  -H "x-connector-config: {\"config\":{\"Stripe\":{\"api_key\":\"$STRIPE_API_KEY\"}}}" \
  -d '{
    "merchant_quote_id": "quote_123",
    "amount": {"minor_amount": 1000, "currency": "USD"},
    "destination_currency": "USD"
  }' \
  localhost:8080 types.PayoutService/Stage
```

```json
{
  "payout_status": "PENDING",
  "connector_payout_id": "po_1Hh1XYZ2eZvKYlo2C",
  "status_code": 200
}
```

## Next Steps

- [Transfer Payout](./transfer.md)
