# Eligibility Method

<!--
---
title: Eligibility (Python SDK)
description: Check the eligibility of a payout before initiating it. Using the Python SDK.
last_updated: 2026-08-14
generated_from: backend/grpc-api-types/proto/services.proto
auto_generated: true
reviewed_by: ''
reviewed_at: ''
approved: false
sdk_language: python
---
-->

## Overview

The `eligibility` method in the Payout Service checks whether a payout can be made to a given payout method before funds are committed. It is used for pre-verification flows such as SEPA Verification of Payee (VoP) and other payee-verification checks.

## Purpose

Use this operation to validate a payee or payout method before initiating a transfer, reducing the risk of failed or misdirected payouts.

| Scenario | Developer Implementation |
|----------|--------------------------|
| Verify a payee before transfer | Call `eligibility` with the payout method and amount, then inspect `payout_eligible`. |
| Pre-check a bank account | Call `eligibility` with the bank details to confirm the account can receive funds. |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_payout_id` | string | No | Your internal identifier for the eligibility check. |
| `connector_feature_data` | SecretString | No | Connector-specific metadata passed to the eligibility check. |
| `payout_method_data` | PayoutMethod | No | Specific details of the payout instrument being checked. |
| `amount` | Money | Yes | The amount to be paid out. |
| `connector_payout_id` | string | No | An existing payout identifier from the connector. |
| `destination_currency` | Currency | Yes | The currency in which the recipient will receive the payout. |
| `access_token` | SecretString | No | Access token for the connector, if required. |
| `address` | PayoutAddress | No | Address information associated with the payout. |
| `customer` | Customer | No | Details about the customer receiving the payout. |
| `source_bank_data` | SourceBankData | No | Details of the bank account from which the payout is funded. |

## Response Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_payout_id` | string | No | Your internal identifier for the eligibility check. |
| `payout_status` | PayoutEnums.PayoutStatus | No | The current status of the payout. |
| `connector_payout_id` | string | No | The unique payout ID assigned by the connector (set only when the payee is eligible). |
| `error` | ErrorInfo | No | Details of any error that occurred during the check. |
| `status_code` | uint32 | Yes | The HTTP status code returned from the connector. |
| `payout_eligible` | bool | No | Whether the payout is eligible. |
| `connector_metadata` | SecretString | No | Connector-specific details as a JSON object, surfaced to the merchant. |
| `connector_eligibility_reference_id` | string | No | Connector's reference for the eligibility check itself. Set for every verdict so the check remains traceable for reconciliation. |

## Example

### SDK Setup

```python
from hyperswitch_prism import PayoutClient

payout_client = PayoutClient(
    connector='stripe',
    api_key='YOUR_API_KEY',
    environment='SANDBOX'
)
```

### Request

```python
request = {
    "amount": {
        "minor_amount": 1000,
        "currency": "USD"
    },
    "destination_currency": "USD",
    "payout_method_data": {
        "ach": {
            "bank_account_number": "000123456789",
            "bank_routing_number": "110000000"
        }
    }
}

response = await payout_client.eligibility(request)
```

### Response

```python
{
    "payout_eligible": True,
    "connector_eligibility_reference_id": "elig_1Hh1XYZ2eZvKYlo2C",
    "status_code": 200
}
```

## Next Steps

- [Transfer Payout](./transfer.md)
- [Create Payout](./create.md)
