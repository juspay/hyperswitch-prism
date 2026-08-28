# enroll_disburse_account Method

<!--
---
title: enroll_disburse_account (Python SDK)
description: Enroll an account for disbursement of funds. Using the Python SDK.
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

The `enroll_disburse_account` method registers and verifies a destination account (like a bank account) to receive disbursements from your platform.

## Purpose

Use this operation to securely enroll payout destinations.

| Scenario | Developer Implementation |
|----------|--------------------------|
| Link a vendor's bank account | Call `enroll_disburse_account` with their bank details. |

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
    "amount": {
        "minor_amount": 0,
        "currency": "USD"
    },
    "payout_method_data": {
        "ach": {
            "bank_account_number": "000123456789",
            "bank_routing_number": "110000000"
        }
    }
}

response = await payout_client.enroll_disburse_account(request)
```

### Response

```python
{
    "payout_status": "SUCCESS",
    "connector_payout_id": "ba_1Hh1XYZ2eZvKYlo2C",
    "status_code": 200
}
```

## Next Steps

- [Transfer Payout](./transfer.md)
