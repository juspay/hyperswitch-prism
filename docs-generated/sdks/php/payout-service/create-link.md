# createLink Method

<!--
---
title: createLink (PHP SDK)
description: Create a link for the recipient to claim a payout. Using the PHP SDK.
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

The `createLink` method generates a URL that can be sent to a recipient, allowing them to securely provide their own payout method details (like bank account information) to claim the funds.

## Purpose

Use this operation when you do not have the recipient's payout method details upfront.

| Scenario | Developer Implementation |
|----------|--------------------------|
| Send funds to a user via email | Call `createLink` and share the generated URL with the user. |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantPayoutId` | string | No | Your internal identifier for the payout. |
| `address` | PayoutAddress | Yes | Address information associated with the payout. |
| `connectorFeatureData` | SecretString | No | Connector-specific metadata or feature configurations. |
| `payoutMethodData` | PayoutMethod | No | Specific details of the payout instrument (optional). |
| `connectorQuoteId` | string | No | Pre-negotiated quote ID if applicable. |
| `connectorPayoutId` | string | No | An existing payout identifier from the connector. |
| `amount` | Money | Yes | The amount to be paid out. |
| `destinationCurrency` | Currency | Yes | The currency for the payout. |
| `customer` | Customer | No | Details about the customer. |
| `priority` | PayoutEnums.PayoutPriority | No | Priority of the payout. |
| `connectorPayoutMethodId` | string | No | The connector's unique ID for a stored payout method. |
| `webhookUrl` | string | No | URL where payout status updates should be sent. |
| `browserInfo` | BrowserInformation | No | Information about the user's browser. |
| `accessToken` | SecretString | No | Access token for the connector, if required. |
| `description` | string | No | Description of the payout. |
| `sourceBankData` | SourceBankData | No | Details of the bank account from which the payout is funded. |

## Response Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantPayoutId` | string | No | Your internal identifier passed to the payout processor. |
| `payoutStatus` | PayoutEnums.PayoutStatus | No | The current status of the payout link. |
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
    'amount' => [
        'minorAmount' => 1000,
        'currency' => 'USD'
    ],
    'destinationCurrency' => 'USD'
];

$response = $payoutClient->createLink($request);
```

### Response

```php
[
    'payoutStatus' => 'REQUIRES_PAYOUT_METHOD_DATA',
    'connectorPayoutId' => 'po_1Hh1XYZ2eZvKYlo2C',
    'statusCode' => 200
]
```

## Next Steps

- [Get Payout Status](./get.md)
