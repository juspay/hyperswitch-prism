# stage Method

<!--
---
title: stage (PHP SDK)
description: Stage a payout for processing. Using the PHP SDK.
last_updated: 2026-08-13
generated_from: backend/grpc-api-types/proto/services.proto
auto_generated: true
reviewed_by: ''
reviewed_at: ''
approved: false
sdk_language: php
---
-->

## Overview

The `stage` method allows you to prepare or stage a payout before it is executed. This can be used in workflows where payouts are reviewed or batched before final transfer.

## Purpose

Use this operation to put a payout into a staged state.

| Scenario | Developer Implementation |
|----------|--------------------------|
| Prepare a payout for review | Call `stage` with the quote details and amount. |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantQuoteId` | string | No | Your internal quote identifier. |
| `address` | PayoutAddress | Yes | Address information associated with the payout. |
| `amount` | Money | Yes | The amount to be staged. |
| `destinationCurrency` | Currency | Yes | The currency in which the payout will be received. |
| `customer` | Customer | No | Details about the customer. |
| `browserInfo` | BrowserInformation | No | Information about the user's browser. |
| `accessToken` | SecretString | No | Access token for the connector, if required. |

## Response Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantPayoutId` | string | No | Your internal identifier passed to the payout processor. |
| `payoutStatus` | PayoutEnums.PayoutStatus | No | The current status of the staged payout. |
| `connectorPayoutId` | string | No | The unique payout ID assigned by the connector. |
| `error` | ErrorInfo | No | Details of any error that occurred. |
| `statusCode` | uint32 | Yes | The HTTP status code returned from the connector. |

## Example

### SDK Setup

```php
use HyperswitchPrism\PayoutClient;

$payoutClient = new PayoutClient([
    'connector' => 'stripe',
    'apiKey' => 'YOUR_API_KEY',
    'environment' => 'SANDBOX'
]);
```

### Request

```php
$request = [
    'merchantQuoteId' => 'quote_123',
    'amount' => [
        'minorAmount' => 1000,
        'currency' => 'USD'
    ],
    'destinationCurrency' => 'USD'
];

$response = $payoutClient->stage($request);
```

### Response

```php
[
    'payoutStatus' => 'PENDING',
    'connectorPayoutId' => 'po_1Hh1XYZ2eZvKYlo2C',
    'statusCode' => 200
]
```

## Next Steps

- [Transfer Payout](./transfer.md)
