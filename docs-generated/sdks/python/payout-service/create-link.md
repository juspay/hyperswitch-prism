# create_link Method

<!--
---
title: create_link (Python SDK)
description: Create a link for the recipient to claim a payout. Using the Python SDK.
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

The `create_link` method generates a URL that can be sent to a recipient, allowing them to securely provide their own payout method details (like bank account information) to claim the funds.

## Purpose

Use this operation when you do not have the recipient's payout method details upfront.

| Scenario | Developer Implementation |
|----------|--------------------------|
| Send funds to a user via email | Call `create_link` and share the generated URL with the user. |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_payout_id` | string | No | Your internal identifier for the payout. |
| `address` | PayoutAddress | Yes | Address information associated with the payout. |
| `connector_feature_data` | SecretString | No | Connector-specific metadata or feature configurations. |
| `payout_method_data` | PayoutMethod | No | Specific details of the payout instrument (optional). |
| `connector_quote_id` | string | No | Pre-negotiated quote ID if applicable. |
| `connector_payout_id` | string | No | An existing payout identifier from the connector. |
| `amount` | Money | Yes | The amount to be paid out. |
| `destination_currency` | Currency | Yes | The currency for the payout. |
| `customer` | Customer | No | Details about the customer. |
| `priority` | PayoutEnums.PayoutPriority | No | Priority of the payout. |
| `connector_payout_method_id` | string | No | The connector's unique ID for a stored payout method. |
| `webhook_url` | string | No | URL where payout status updates should be sent. |
| `browser_info` | BrowserInformation | No | Information about the user's browser. |
| `access_token` | SecretString | No | Access token for the connector, if required. |

## Response Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_payout_id` | string | No | Your internal identifier passed to the payout processor. |
| `payout_status` | PayoutEnums.PayoutStatus | No | The current status of the payout link. |
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
    "amount": {
        "minor_amount": 1000,
        "currency": "USD"
    },
    "destination_currency": "USD"
}

response = await payout_client.create_link(request)
```

### Response

```python
{
    "payout_status": "REQUIRES_PAYOUT_METHOD_DATA",
    "connector_payout_id": "po_1Hh1XYZ2eZvKYlo2C",
    "status_code": 200
}
```

## Next Steps

- [Get Payout Status](./get.md)
