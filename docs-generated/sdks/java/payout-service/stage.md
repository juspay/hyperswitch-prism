# stage Method

<!--
---
title: stage (Java SDK)
description: Stage a payout for processing. Using the Java SDK.
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
request.put("merchantQuoteId", "quote_123");
request.put("amount", Map.of("minorAmount", 1000, "currency", "USD"));
request.put("destinationCurrency", "USD");

Response response = payoutClient.stage(request);
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

- [Transfer Payout](./transfer.md)
