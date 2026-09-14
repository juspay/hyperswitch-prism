---
title: Create Payout Recipient
description: Create a recipient entity for payouts.
---

# Create Payout Recipient

## Overview

The `CreateRecipient` RPC registers a new recipient entity (individual or business) with the payment processor. This is often required before funds can be transferred to them.

## Purpose

Use this operation to set up a vendor, contractor, or user in the processor's system.

| Scenario | Developer Implementation |
|----------|--------------------------|
| Onboard a new seller | Call `CreateRecipient` with their details and `recipient_type`. |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_payout_id` | string | No | Your internal identifier for the payout/recipient operation. |
| `address` | PayoutAddress | Yes | Address information associated with the recipient. |
| `payout_method_data` | PayoutMethod | No | Specific details of the payout instrument for the recipient. |
| `amount` | Money | Yes | The amount to be paid out (if creating recipient inline with a payout). |
| `recipient_type` | PayoutEnums.PayoutRecipientType | Yes | Type of entity (e.g., INDIVIDUAL, COMPANY). |
| `customer` | Customer | No | Details about the customer/recipient. |
| `access_token` | SecretString | No | Access token for the connector, if required. |

## Response Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_payout_id` | string | No | Your internal identifier passed to the payout processor. |
| `payout_status` | PayoutEnums.PayoutStatus | No | The status of the recipient creation. |
| `connector_payout_id` | string | No | The unique identifier assigned to the recipient or payout. |
| `error` | ErrorInfo | No | Details of any error that occurred. |
| `status_code` | uint32 | Yes | The HTTP status code returned from the connector. |

## Example

```bash
grpcurl -H "x-connector: stripe" \
  -H "x-connector-config: {\"config\":{\"Stripe\":{\"api_key\":\"$STRIPE_API_KEY\"}}}" \
  -d '{
    "recipient_type": "INDIVIDUAL",
    "amount": {"minor_amount": 0, "currency": "USD"}
  }' \
  localhost:8080 types.PayoutService/CreateRecipient
```

```json
{
  "payout_status": "SUCCESS",
  "connector_payout_id": "acct_1Hh1XYZ2eZvKYlo2C",
  "status_code": 200
}
```

## Next Steps

- [Transfer Payout](./transfer.md)
