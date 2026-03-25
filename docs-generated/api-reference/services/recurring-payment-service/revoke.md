# Revoke RPC

<!--
---
title: Revoke
description: Cancel an existing recurring payment mandate - stop future automatic charges on customer's stored consent for subscription cancellations
last_updated: 2026-03-11
generated_from: crates/types-traits/grpc-api-types/proto/services.proto
auto_generated: false
reviewed_by: engineering
reviewed_at: 2026-03-05
approved: true
---
-->

## Overview

The `Revoke` RPC cancels an existing recurring payment mandate, stopping all future automatic charges associated with that mandate. Use this when customers cancel their subscriptions, end their service agreements, or request to stop recurring billing.

**Business Use Case:** When a customer decides to cancel their subscription or service, you need to stop all future recurring charges. The Revoke RPC cancels the mandate at the payment processor, ensuring no further charges can be made using that stored consent. This is a critical compliance requirement for subscription businesses.

## Purpose

**Why use Revoke?**

| Scenario | Developer Implementation |
|----------|-------------------------|
| **Subscription cancellation** | Customer cancels SaaS subscription - call `Revoke` to stop all future charges |
| **Service termination** | Customer ends gym membership - call `Revoke` to cancel mandate |
| **Customer request** | Customer emails asking to stop recurring billing - call `Revoke` immediately |
| **Failed payment cleanup** | After multiple failed retries, clean up by revoking the mandate |
| **Plan downgrade** | Customer switches to non-recurring plan - revoke old mandate |

**Key outcomes:**
- Mandate cancelled at the payment processor
- No future charges can be processed using this mandate
- Compliance with card network requirements for subscription cancellations
- Clear audit trail of when consent was revoked

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_revoke_id` | string | Yes | Your unique identifier for this revoke operation |
| `mandate_id` | string | Yes | The mandate ID to revoke (your internal reference) |
| `connector_mandate_id` | string | No | The connector's mandate ID (if different from mandate_id) |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `status` | MandateStatus | Current status of the mandate: REVOKED, PENDING, FAILED |
| `error` | ErrorInfo | Error details if revocation failed |
| `status_code` | uint32 | HTTP-style status code (200, 400, etc.) |
| `response_headers` | map<string,string> | Connector-specific response headers |
| `network_transaction_id` | string | Card network transaction reference |
| `merchant_revoke_id` | string | Your revoke reference (echoed back) |
| `raw_connector_response` | SecretString | Raw API response from connector (debugging) |
| `raw_connector_request` | SecretString | Raw API request sent to connector (debugging) |

## Example

### Request (grpcurl)

```bash
grpcurl -H "x-connector: stripe" \
  -H "x-connector-config: {\"config\":{\"Stripe\":{\"api_key\":\"$STRIPE_API_KEY\"}}}" \
  -d '{
    "merchant_revoke_id": "revoke_001",
    "mandate_id": "mandate_sub_001",
    "connector_mandate_id": "seti_3Oxxx..."
  }' \
  localhost:8080 \
  types.RecurringPaymentService/Revoke
```

### Response (Success)

```json
{
  "status": "REVOKED",
  "status_code": 200,
  "merchant_revoke_id": "revoke_001",
  "network_transaction_id": "txn_xxx..."
}
```

### Response (Mandate Not Found)

```json
{
  "status": "FAILED",
  "status_code": 404,
  "merchant_revoke_id": "revoke_001",
  "error": {
    "code": "mandate_not_found",
    "message": "The mandate was not found or has already been revoked."
  }
}
```

## Status Values

| Status | Description |
|--------|-------------|
| `REVOKED` | Mandate successfully cancelled, no future charges possible |
| `PENDING` | Revocation is being processed (rare, most are immediate) |
| `FAILED` | Revocation could not be completed |

## Compliance Considerations

**When to Revoke:**

| Situation | Action Required |
|-----------|----------------|
| Customer explicitly cancels | Revoke immediately upon confirmation |
| Customer requests via email/support | Revoke within 24 hours of request |
| Failed payments (multiple) | Revoke after grace period if not resolved |
| Account closure | Revoke all mandates for the customer |
| Plan downgrade to non-recurring | Revoke existing mandate |

**Best Practices:**

1. **Immediate revocation** - Always revoke mandates as soon as cancellation is confirmed
2. **Customer confirmation** - Send email confirmation when mandate is revoked
3. **Idempotency** - Revoking an already-revoked mandate should not error
4. **Audit logging** - Log all revocation events for compliance audits
5. **Grace periods** - Consider allowing customers to "undo" cancellation within a short window

## Important Notes

- **Revoke is immediate** - Once revoked, the mandate cannot be reactivated; customer must go through SetupRecurring again
- **Existing charges unaffected** - Revocation stops future charges only; past charges remain valid
- **Partial refunds** - Consider refunding unused subscription time after revocation
- **Data retention** - Keep records of revoked mandates for compliance and auditing purposes

## Next Steps

- [Charge](./charge.md) - Process recurring payments before revoking
- [SetupRecurring](../payment-service/setup-recurring.md) - Create new mandates if customer resubscribes
- [Refund](../payment-service/refund.md) - Process refunds for unused subscription time
