# IncrementalAuthorization RPC

<!--
---
title: IncrementalAuthorization
description: Increase authorized amount if still in authorized state - allows adding charges for hospitality, tips, or incremental services
last_updated: 2026-03-11
generated_from: crates/types-traits/grpc-api-types/proto/services.proto
auto_generated: true
reviewed_by: ''
reviewed_at: ''
approved: false
---
-->

## Overview

The `IncrementalAuthorization` RPC increases the authorized amount on an existing authorization that is still in `AUTHORIZED` status. This allows you to add charges to an existing hold without creating a new authorization, essential for hospitality, transportation, and service industries where the final amount may exceed the initial estimate.

**Business Use Case:** When a hotel guest adds room service charges to their folio, a ride-share passenger adds a tip after the ride, or a customer adds items to an order that hasn't shipped yet. Incremental authorization adjusts the hold amount upward, ensuring sufficient funds are available for the final capture.

## Purpose

**Why use IncrementalAuthorization?**

| Scenario | Developer Implementation |
|----------|-------------------------|
| **Hotel incidentals** | Guest adds room service or mini-bar charges - call `IncrementalAuthorization` to increase hold on their card |
| **Restaurant tips** | Post-dining tip adjustment - call `IncrementalAuthorization` to add tip amount to original authorization |
| **Add-on services** | Customer adds expedited shipping or warranty - call `IncrementalAuthorization` to cover additional costs |
| **Metered services** | Usage exceeds initial estimate - call `IncrementalAuthorization` to extend hold for actual consumption |
| **Subscription upgrades** | Customer upgrades plan mid-cycle - call `IncrementalAuthorization` to cover prorated difference |

**Key outcomes:**
- Increased hold amount without new authorization
- No additional card verification required
- Combined final capture covers all charges
- Reduces declined captures due to insufficient authorization
- Single transaction record for customer statement

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_authorization_id` | string | Yes | Your unique identifier for this incremental authorization |
| `connector_transaction_id` | string | Yes | The connector's transaction ID from the original authorization |
| `amount` | Money | Yes | New total amount to be authorized (in minor currency units) |
| `reason` | string | No | Reason for increasing the authorized amount |
| `connector_feature_data` | SecretString | No | Connector-specific metadata for the transaction |
| `state` | ConnectorState | No | State from previous multi-step flow |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `connector_authorization_id` | string | Connector's authorization ID |
| `status` | AuthorizationStatus | Current status of the authorization |
| `error` | ErrorInfo | Error details if incremental authorization failed |
| `status_code` | uint32 | HTTP-style status code (200, 402, etc.) |
| `response_headers` | map<string, string> | Connector-specific response headers |
| `state` | ConnectorState | State to pass to next request in multi-step flow |

## Example

### Request (grpcurl)

```bash
grpcurl -H "x-connector: stripe" \
  -H "x-connector-config: {\"config\":{\"Stripe\":{\"api_key\":\"$STRIPE_API_KEY\"}}}" \
  -d '{
    "merchant_authorization_id": "incr_auth_001",
    "connector_transaction_id": "pi_3Oxxx...",
    "amount": {
      "minor_amount": 1500,
      "currency": "USD"
    },
    "reason": "Room service charges added"
  }' \
  localhost:8080 \
  types.PaymentService/IncrementalAuthorization
```

### Response

```json
{
  "connector_authorization_id": "pi_3Oxxx...",
  "status": "AUTHORIZED",
  "status_code": 200
}
```

## Next Steps

- [Authorize](./authorize.md) - Create initial authorization (must set `request_incremental_authorization: true`)
- [Capture](./capture.md) - Finalize the payment with the increased amount
- [Get](./get.md) - Check current authorization status before incremental request
- [Void](./void.md) - Cancel the authorization if no longer needed
