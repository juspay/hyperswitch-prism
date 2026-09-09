# get Method

<!--
---
title: get (Python SDK)
description: Retrieve the current status and details of a payout. Using the Python SDK.
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

The `get` method allows you to check the current status of a payout. This is essential for syncing your internal systems with the payment processor's state, especially for asynchronous payout methods like bank transfers.

## Purpose

Use this operation to poll for the status of a payout or to verify details before taking further action.

| Scenario | Developer Implementation |
|----------|--------------------------|
| Check if a payout succeeded | Call `get` with the `merchant_payout_id` or `connector_payout_id`. |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_payout_id` | string | No | Your internal identifier for the payout. |
| `connector_payout_id` | string | No | The unique payout ID assigned by the connector. |
| `access_token` | SecretString | No | Access token for the connector, if required. |
| `source_bank_data` | SourceBankData | No | Source (debtor) bank data. Some connectors (e.g. Deutsche Bank) require the debtor account details to perform a status enquiry. |

## Response Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_payout_id` | string | No | Your internal identifier passed to the payout processor. |
| `payout_status` | PayoutEnums.PayoutStatus | No | The current status of the payout. |
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
    "merchant_payout_id": "po_internal_12345"
}

response = await payout_client.get(request)
```

### Response

```python
{
    "merchant_payout_id": "po_internal_12345",
    "payout_status": "SUCCESS",
    "connector_payout_id": "po_1Hh1XYZ2eZvKYlo2C",
    "status_code": 200
}
```

## Next Steps

- [Void Payout](./void.md)
