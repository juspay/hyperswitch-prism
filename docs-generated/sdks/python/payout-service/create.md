# create Method

<!--
---
title: create (Python SDK)
description: Create a new payout to transfer funds to a customer or vendor. Using the Python SDK.
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

The `create` method in the Payout Service is used to initiate a transfer of funds from your merchant account to a customer, vendor, or third-party entity. This is commonly used in marketplaces, gig-economy platforms, or any business model requiring external disbursements.

## Purpose

This operation is the first step in the payout lifecycle. Use this when you have sufficient funds and wish to send money to a registered payout method (like a bank account, card, or wallet).

| Scenario | Developer Implementation |
|----------|--------------------------|
| Send funds to a vendor | Call `create` with the vendor's bank account details and payout amount. |
| Refund a customer via an alternative method | Provide the customer's wallet or card details in the `payout_method_data`. |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_payout_id` | string | No | Your internal identifier passed to the payout processor. |
| `address` | PayoutAddress | Yes | The shipping and billing address associated with the payout. |
| `connector_feature_data` | SecretString | No | Connector-specific metadata or feature configurations. |
| `payout_method_data` | PayoutMethod | No | Specific details of the payout instrument (e.g., Card, ACH, Pix). |
| `connector_quote_id` | string | No | Pre-negotiated quote ID if applicable. |
| `connector_payout_id` | string | No | An existing payout identifier from the connector, if any. |
| `amount` | Money | Yes | The amount to be paid out along with the source currency. |
| `destination_currency` | Currency | Yes | The currency in which the recipient will receive the payout. |
| `customer` | Customer | No | Details about the customer receiving the payout. |
| `priority` | PayoutEnums.PayoutPriority | No | Priority of the payout (e.g., INSTANT, WIRE). |
| `connector_payout_method_id` | string | No | The connector's unique ID for a stored payout method. |
| `webhook_url` | string | No | URL where payout status updates should be sent. |
| `browser_info` | BrowserInformation | No | Information about the user's browser, used for fraud prevention. |
| `access_token` | SecretString | No | Access token for the connector, if required. |
| `source_bank_data` | SourceBankData | No | Details of the bank account from which the payout is funded. |
| `description` | string | No | Description of the payout. |

## Response Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_payout_id` | string | No | Your internal identifier passed to the payout processor. |
| `payout_status` | PayoutEnums.PayoutStatus | No | The current status of the payout (e.g., PENDING, SUCCESS). |
| `connector_payout_id` | string | No | The unique payout ID assigned by the connector. |
| `error` | ErrorInfo | No | Details of any error that occurred during creation. |
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

response = await payout_client.create(request)
```

### Response

```python
{
    "payout_status": "PENDING",
    "connector_payout_id": "po_1Hh1XYZ2eZvKYlo2C",
    "status_code": 200
}
```

## Next Steps

- [Get Payout Status](./get.md)
- [Void Payout](./void.md)
