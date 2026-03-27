# VerifyRedirectResponse RPC

<!--
---
title: VerifyRedirectResponse
description: Validate redirect-based payment responses - confirms authenticity of redirect-based payment completions to prevent fraud and tampering
last_updated: 2026-03-11
generated_from: crates/types-traits/grpc-api-types/proto/services.proto
auto_generated: true
reviewed_by: ''
reviewed_at: ''
approved: false
---
-->

## Overview

The `VerifyRedirectResponse` RPC validates the authenticity of payment responses received from redirect-based authentication flows. This includes 3D Secure (3DS) redirects, bank authentication pages, and wallet payment callbacks. It ensures the response genuinely came from the payment provider and hasn't been tampered with during transit.

**Business Use Case:** When a customer completes a 3DS challenge or bank redirect and is redirected back to your application, you need to verify that the response is legitimate. This prevents fraudsters from spoofing successful payment notifications and ensures you only fulfill orders for genuine successful payments.

## Purpose

**Why use VerifyRedirectResponse?**

| Scenario | Developer Implementation |
|----------|-------------------------|
| **3DS completion** | Customer returns from 3DS challenge - call `VerifyRedirectResponse` to validate the authentication result |
| **Bank redirect** | Customer returns from bank authentication page - call `VerifyRedirectResponse` to confirm payment success |
| **Wallet payment** | Customer completes Apple Pay or Google Pay - call `VerifyRedirectResponse` to verify the token |
| **Fraud prevention** | Suspicious redirect parameters detected - call `VerifyRedirectResponse` to validate before fulfilling order |
| **Tampering detection** | URL parameters appear modified - call `VerifyRedirectResponse` to verify integrity |

**Key outcomes:**
- Confirms redirect response authenticity
- Prevents fraudulent payment notifications
- Extracts verified transaction details
- Determines final payment status
- Enables safe order fulfillment

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_order_id` | string | Yes | Your unique order identifier for this verification |
| `request_details` | RequestDetails | Yes | Details of the redirect request including headers, body, and query parameters |
| `redirect_response_secrets` | RedirectResponseSecrets | No | Secrets for validating the redirect response |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `source_verified` | bool | Whether the redirect source is verified as authentic |
| `connector_transaction_id` | string | Connector's transaction ID if verification successful |
| `response_amount` | Money | Amount from the verified response |
| `merchant_order_id` | string | Your order reference (echoed back) |
| `status` | PaymentStatus | Current status of the payment after verification |
| `error` | ErrorInfo | Error details if verification failed |
| `raw_connector_response` | SecretString | Raw API response from connector for debugging |

## Example

### Request (grpcurl)

```bash
grpcurl -H "x-connector: stripe" \
  -H "x-connector-config: {\"config\":{\"Stripe\":{\"api_key\":\"$STRIPE_API_KEY\"}}}" \
  -d '{
    "merchant_order_id": "order_001",
    "request_details": {
      "headers": [
        {"key": "Content-Type", "value": "application/x-www-form-urlencoded"}
      ],
      "query_params": [
        {"key": "payment_intent", "value": "pi_3Oxxx..."},
        {"key": "payment_intent_client_secret", "value": "pi_3Oxxx..._secret_xxx"}
      ],
      "body": ""
    }
  }' \
  localhost:8080 \
  types.PaymentService/VerifyRedirectResponse
```

### Response

```json
{
  "source_verified": true,
  "connector_transaction_id": "pi_3Oxxx...",
  "response_amount": {
    "minor_amount": 1000,
    "currency": "USD"
  },
  "merchant_order_id": "order_001",
  "status": "AUTHORIZED"
}
```

## Next Steps

- [Authorize](./authorize.md) - Initiate a payment that may require redirect authentication
- [Capture](./capture.md) - Finalize the payment after successful verification
- [Get](./get.md) - Check payment status after verification
- [PaymentMethodAuthenticationService](../payment-method-authentication-service/README.md) - Pre-authenticate before authorization
