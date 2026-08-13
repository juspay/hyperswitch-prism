# create Method

<!--
---
title: create (Java SDK)
description: Create a new payout to transfer funds to a customer or vendor. Using the Java SDK.
last_updated: 2026-08-13
generated_from: backend/grpc-api-types/proto/services.proto
auto_generated: true
reviewed_by: ''
reviewed_at: ''
approved: false
sdk_language: java
---
-->

## Overview

The `create` method in the Payout Service is used to initiate a transfer of funds from your merchant account to a customer, vendor, or third-party entity. This is commonly used in marketplaces, gig-economy platforms, or any business model requiring external disbursements.

## Purpose

This operation is the first step in the payout lifecycle. Use this when you have sufficient funds and wish to send money to a registered payout method (like a bank account, card, or wallet).

| Scenario | Developer Implementation |
|----------|--------------------------|
| Send funds to a vendor | Call `create` with the vendor's bank account details and payout amount. |
| Refund a customer via an alternative method | Provide the customer's wallet or card details in the `payoutMethodData`. |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantPayoutId` | string | No | Your internal identifier passed to the payout processor. |
| `address` | PayoutAddress | Yes | The shipping and billing address associated with the payout. |
| `connectorFeatureData` | SecretString | No | Connector-specific metadata or feature configurations. |
| `payoutMethodData` | PayoutMethod | No | Specific details of the payout instrument (e.g., Card, ACH, Pix). |
| `connectorQuoteId` | string | No | Pre-negotiated quote ID if applicable. |
| `connectorPayoutId` | string | No | An existing payout identifier from the connector, if any. |
| `amount` | Money | Yes | The amount to be paid out along with the source currency. |
| `destinationCurrency` | Currency | Yes | The currency in which the recipient will receive the payout. |
| `customer` | Customer | No | Details about the customer receiving the payout. |
| `priority` | PayoutEnums.PayoutPriority | No | Priority of the payout (e.g., INSTANT, WIRE). |
| `connectorPayoutMethodId` | string | No | The connector's unique ID for a stored payout method. |
| `webhookUrl` | string | No | URL where payout status updates should be sent. |
| `browserInfo` | BrowserInformation | No | Information about the user's browser, used for fraud prevention. |
| `accessToken` | SecretString | No | Access token for the connector, if required. |
| `sourceBankData` | SourceBankData | No | Details of the bank account from which the payout is funded. |
| `description` | string | No | Description of the payout. |

## Response Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantPayoutId` | string | No | Your internal identifier passed to the payout processor. |
| `payoutStatus` | PayoutEnums.PayoutStatus | No | The current status of the payout (e.g., PENDING, SUCCESS). |
| `connectorPayoutId` | string | No | The unique payout ID assigned by the connector. |
| `error` | ErrorInfo | No | Details of any error that occurred during creation. |
| `statusCode` | uint32 | Yes | The HTTP status code returned from the connector. |

## Example

### SDK Setup

```java
import com.hyperswitch.prism.PayoutClient;
import java.util.Map;
import java.util.HashMap;

PayoutClient payoutClient = PayoutClient.builder()
    .connector("stripe")
    .apiKey("YOUR_API_KEY")
    .environment("SANDBOX")
    .build();
```

### Request

```java
Map<String, Object> request = new HashMap<>();
request.put("amount", Map.of("minorAmount", 1000, "currency", "USD"));
request.put("destinationCurrency", "USD");
request.put("payoutMethodData", Map.of("card", Map.of("cardNumber", "4242424242424242", "cardExpMonth", "12", "cardExpYear", "2027")));

Response response = payoutClient.create(request);
```

### Response

```java
{
    "payoutStatus": "PENDING",
    "connectorPayoutId": "po_1Hh1XYZ2eZvKYlo2C",
    "statusCode": 200
}
```

## Next Steps

- [Get Payout Status](./get.md)
- [Void Payout](./void.md)
