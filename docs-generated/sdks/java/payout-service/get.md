# get Method

<!--
---
title: get (Java SDK)
description: Retrieve the current status and details of a payout. Using the Java SDK.
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

The `get` method allows you to check the current status of a payout. This is essential for syncing your internal systems with the payment processor's state, especially for asynchronous payout methods like bank transfers.

## Purpose

Use this operation to poll for the status of a payout or to verify details before taking further action.

| Scenario | Developer Implementation |
|----------|--------------------------|
| Check if a payout succeeded | Call `get` with the `merchantPayoutId` or `connectorPayoutId`. |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantPayoutId` | string | No | Your internal identifier for the payout. |
| `connectorPayoutId` | string | No | The unique payout ID assigned by the connector. |
| `accessToken` | SecretString | No | Access token for the connector, if required. |
| `sourceBankData` | SourceBankData | No | Source (debtor) bank data. Some connectors (e.g. Deutsche Bank) require the debtor account details to perform a status enquiry. |

## Response Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantPayoutId` | string | No | Your internal identifier passed to the payout processor. |
| `payoutStatus` | PayoutEnums.PayoutStatus | No | The current status of the payout. |
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
request.put("merchantPayoutId", "po_internal_12345");

Response response = payoutClient.get(request);
```

### Response

```java
{
    "merchantPayoutId": "po_internal_12345",
    "payoutStatus": "SUCCESS",
    "connectorPayoutId": "po_1Hh1XYZ2eZvKYlo2C",
    "statusCode": 200
}
```

## Next Steps

- [Void Payout](./void.md)
