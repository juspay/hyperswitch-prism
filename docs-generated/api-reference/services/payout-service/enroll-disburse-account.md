---
title: Enroll Disburse Account
description: Enroll an account for disbursement of funds.
---

# Enroll Disburse Account

## Overview

The `EnrollDisburseAccount` RPC registers and verifies a destination account (like a bank account) to receive disbursements from your platform.

## Purpose

Use this operation to securely enroll payout destinations.

| Scenario | Developer Implementation |
|----------|--------------------------|
| Link a vendor's bank account | Call `EnrollDisburseAccount` with their bank details. |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_payout_id` | string | No | Your internal identifier for the operation. |
| `address` | PayoutAddress | Yes | Address information associated with the account. |
| `payout_method_data` | PayoutMethod | No | Specific details of the payout instrument being enrolled. |
| `amount` | Money | Yes | The amount to be paid out (if enrolling inline). |
| `customer` | Customer | No | Details about the customer/account holder. |
| `access_token` | SecretString | No | Access token for the connector, if required. |

## Response Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_payout_id` | string | No | Your internal identifier. |
| `payout_status` | PayoutEnums.PayoutStatus | No | The status of the enrollment. |
| `connector_payout_id` | string | No | The unique identifier assigned by the connector. |
| `error` | ErrorInfo | No | Details of any error that occurred. |
| `status_code` | uint32 | Yes | The HTTP status code returned from the connector. |

## Example

```bash
grpcurl -H "x-connector: stripe" \
  -H "x-connector-config: {\"config\":{\"Stripe\":{\"api_key\":\"$STRIPE_API_KEY\"}}}" \
  -d '{
    "amount": {"minor_amount": 0, "currency": "USD"},
    "payout_method_data": {
      "ach": {
        "bank_account_number": "000123456789",
        "bank_routing_number": "110000000"
      }
    }
  }' \
  localhost:8080 types.PayoutService/EnrollDisburseAccount
```

```json
{
  "payout_status": "SUCCESS",
  "connector_payout_id": "ba_1Hh1XYZ2eZvKYlo2C",
  "status_code": 200
}
```

## Next Steps

- [Transfer Payout](./transfer.md)
