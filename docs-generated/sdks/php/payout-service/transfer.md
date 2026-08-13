# transfer Method

<!--
---
title: transfer (PHP SDK)
description: Transfer funds for an existing payout. Using the PHP SDK.
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

The `transfer` method in the Payout Service is used to execute the actual fund transfer for a previously initiated payout, or to perform a combined create and transfer operation depending on the processor's flow.

## Purpose

Use this operation to move funds to the destination payout method.

| Scenario | Developer Implementation |
|----------|--------------------------|
| Execute a payout transfer | Call `transfer` with the required payout details and amount. |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantPayoutId` | string | No | Your internal identifier passed to the payout processor. |
| `address` | PayoutAddress | Yes | Address information associated with the payout. |
| `payoutMethodData` | PayoutMethod | No | Specific details of the payout instrument. |
| `connectorQuoteId` | string | No | Pre-negotiated quote ID if applicable. |
| `amount` | Money | Yes | The amount to be transferred. |
| `connectorPayoutId` | string | No | An existing payout identifier from the connector. |
| `destinationCurrency` | Currency | Yes | The currency in which the recipient will receive the payout. |
| `customer` | Customer | No | Details about the customer receiving the payout. |
| `priority` | PayoutEnums.PayoutPriority | No | Priority of the payout transfer. |
| `connectorPayoutMethodId` | string | No | The connector's unique ID for a stored payout method. |
| `webhookUrl` | string | No | URL where payout status updates should be sent. |
| `browserInfo` | BrowserInformation | No | Information about the user's browser. |
| `accessToken` | SecretString | No | Access token for the connector, if required. |
| `sourceBankData` | SourceBankData | No | Details of the bank account from which the payout is funded. |

## Response Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantPayoutId` | string | No | Your internal identifier passed to the payout processor. |
| `payoutStatus` | PayoutEnums.PayoutStatus | No | The current status of the payout transfer. |
| `connectorPayoutId` | string | No | The unique payout ID assigned by the connector. |
| `error` | ErrorInfo | No | Details of any error that occurred during transfer. |
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
    'amount' => [
        'minorAmount' => 1000,
        'currency' => 'USD'
    ],
    'destinationCurrency' => 'USD',
    'payoutMethodData' => [
        'card' => [
            'cardNumber' => '4242424242424242',
            'cardExpMonth' => '12',
            'cardExpYear' => '2027'
        ]
    ]
];

$response = $payoutClient->transfer($request);
```

### Response

```php
[
    'payoutStatus' => 'SUCCESS',
    'connectorPayoutId' => 'tr_1Hh1XYZ2eZvKYlo2C',
    'statusCode' => 200
]
```

## Next Steps

- [Get Payout Status](./get.md)
