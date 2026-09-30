# transfer Method

<!--
---
title: transfer (Python SDK)
description: Transfer funds for an existing payout. Using the Python SDK.
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

The `transfer` method in the Payout Service is used to execute the actual fund transfer for a previously initiated payout, or to perform a combined create and transfer operation depending on the processor's flow.

## Purpose

Use this operation to move funds to the destination payout method.

| Scenario | Developer Implementation |
|----------|--------------------------|
| Execute a payout transfer | Call `transfer` with the required payout details and amount. |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_payout_id` | string | No | Your internal identifier passed to the payout processor. |
| `address` | PayoutAddress | Yes | Address information associated with the payout. |
| `payout_method_data` | PayoutMethod | No | Specific details of the payout instrument. |
| `connector_quote_id` | string | No | Pre-negotiated quote ID if applicable. |
| `amount` | Money | Yes | The amount to be transferred. |
| `connector_payout_id` | string | No | An existing payout identifier from the connector. |
| `destination_currency` | Currency | Yes | The currency in which the recipient will receive the payout. |
| `customer` | Customer | No | Details about the customer receiving the payout. |
| `priority` | PayoutEnums.PayoutPriority | No | Priority of the payout transfer. |
| `connector_payout_method_id` | string | No | The connector's unique ID for a stored payout method. |
| `webhook_url` | string | No | URL where payout status updates should be sent. |
| `browser_info` | BrowserInformation | No | Information about the user's browser. |
| `access_token` | SecretString | No | Access token for the connector, if required. |
| `source_bank_data` | SourceBankData | No | Details of the bank account from which the payout is funded. |
| `description` | string | No | Description of the payout. |
| `connector_eligibility_reference_id` | string | No | Connector's reference for a prior eligibility check, when the processor requires it to authorise the transfer. |

## Response Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_payout_id` | string | No | Your internal identifier passed to the payout processor. |
| `payout_status` | PayoutEnums.PayoutStatus | No | The current status of the payout transfer. |
| `connector_payout_id` | string | No | The unique payout ID assigned by the connector. |
| `error` | ErrorInfo | No | Details of any error that occurred during transfer. |
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
        "minor_amount": 1000,
        "currency": "USD"
    },
    "destination_currency": "USD",
    "payout_method_data": {
        "card": {
            "card_number": "4242424242424242",
            "card_exp_month": "12",
            "card_exp_year": "2027"
        }
    }
}

response = await payout_client.transfer(request)
```

### Response

```python
{
    "payout_status": "SUCCESS",
    "connector_payout_id": "tr_1Hh1XYZ2eZvKYlo2C",
    "status_code": 200
}
```

## Next Steps

- [Get Payout Status](./get.md)
