# Eligibility Method

<!--
---
title: Eligibility (PHP SDK)
description: Check the eligibility of a payout before initiating it. Using the PHP SDK.
last_updated: 2026-08-14
generated_from: backend/grpc-api-types/proto/services.proto
auto_generated: true
reviewed_by: ''
reviewed_at: ''
approved: false
sdk_language: php
---
-->

## Overview

The `eligibility` method in the Payout Service checks whether a payout can be made to a given payout method before funds are committed. It is used for pre-verification flows such as SEPA Verification of Payee (VoP) and other payee-verification checks.

## Purpose

Use this operation to validate a payee or payout method before initiating a transfer, reducing the risk of failed or misdirected payouts.

| Scenario | Developer Implementation |
|----------|--------------------------|
| Verify a payee before transfer | Call `eligibility` with the payout method and amount, then inspect `payoutEligible`. |
| Pre-check a bank account | Call `eligibility` with the bank details to confirm the account can receive funds. |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantPayoutId` | string | No | Your internal identifier for the eligibility check. |
| `connectorFeatureData` | SecretString | No | Connector-specific metadata passed to the eligibility check. |
| `payoutMethodData` | PayoutMethod | No | Specific details of the payout instrument being checked. |
| `amount` | Money | Yes | The amount to be paid out. |
| `connectorPayoutId` | string | No | An existing payout identifier from the connector. |
| `destinationCurrency` | Currency | Yes | The currency in which the recipient will receive the payout. |
| `accessToken` | SecretString | No | Access token for the connector, if required. |
| `address` | PayoutAddress | No | Address information associated with the payout. |
| `customer` | Customer | No | Details about the customer receiving the payout. |
| `sourceBankData` | SourceBankData | No | Details of the bank account from which the payout is funded. |

## Response Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantPayoutId` | string | No | Your internal identifier for the eligibility check. |
| `payoutStatus` | PayoutEnums.PayoutStatus | No | The current status of the payout. |
| `connectorPayoutId` | string | No | The unique payout ID assigned by the connector (set only when the payee is eligible). |
| `error` | ErrorInfo | No | Details of any error that occurred during the check. |
| `statusCode` | uint32 | Yes | The HTTP status code returned from the connector. |
| `payoutEligible` | bool | No | Whether the payout is eligible. |
| `connectorMetadata` | SecretString | No | Connector-specific details as a JSON object, surfaced to the merchant. |
| `connectorEligibilityReferenceId` | string | No | Connector's reference for the eligibility check itself. Set for every verdict so the check remains traceable for reconciliation. |

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
        'ach' => [
            'bankAccountNumber' => '000123456789',
            'bankRoutingNumber' => '110000000'
        ]
    ]
];

$response = $payoutClient->eligibility($request);
```

### Response

```php
[
    'payoutEligible' => true,
    'connectorEligibilityReferenceId' => 'elig_1Hh1XYZ2eZvKYlo2C',
    'statusCode' => 200
]
```

## Next Steps

- [Transfer Payout](./transfer.md)
- [Create Payout](./create.md)
