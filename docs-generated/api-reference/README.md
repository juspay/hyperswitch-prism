# API Reference

Complete reference for all Prism API services, request/response types, and error handling.

## Overview

Prism provides a unified gRPC API for payment processing across 100+ payment processors. The API is organized into services that handle different aspects of the payment lifecycle.

## Services

| Service | Description | Key Operations |
|---------|-------------|----------------|
| [Payment Service](./services/payment-service/) | Core payment lifecycle | Authorize, Capture, Void, Refund, CreateOrder |
| [Refund Service](./services/refund-service/) | Refund operations | Get refund status |
| [Recurring Payment Service](./services/recurring-payment-service/) | Stored payment methods | Charge, Revoke mandate |
| [Dispute Service](./services/dispute-service/) | Chargeback handling | Accept, Defend, SubmitEvidence |
| [Event Service](./services/event-service/) | Webhook processing | Handle connector events |
| [Customer Service](./services/customer-service/) | Customer management | Create customer |
| [Payment Method Service](./services/payment-method-service/) | Payment method storage | Tokenize |
| [Payment Method Authentication Service](./services/payment-method-authentication-service/) | 3DS authentication | Pre-authenticate, Authenticate, Post-authenticate |
| [Merchant Authentication Service](./services/merchant-authentication-service/) | Session management | CreateAccessToken, CreateSessionToken, CreateSdkSessionToken |

## Error Object

All API errors return a structured `ErrorInfo` object with detailed information about what went wrong.

### ErrorInfo Fields

| Field | Type | Description |
|-------|------|-------------|
| `unified_details` | `UnifiedErrorDetails` | Machine-readable unified error code and user-facing message |
| `issuer_details` | `IssuerErrorDetails` | Card issuer-specific error information (scheme, network details) |
| `connector_details` | `ConnectorErrorDetails` | Connector-specific error code and message from the PSP |

### UnifiedErrorDetails Fields

| Field | Type | Description |
|-------|------|-------------|
| `code` | `string` | Machine-readable error code (e.g., `INSUFFICIENT_FUNDS`) |
| `message` | `string` | Human-readable error message |
| `description` | `string` | Detailed explanation of the error |
| `user_guidance_message` | `string` | User-facing message with guidance on next steps |

### IssuerErrorDetails Fields

| Field | Type | Description |
|-------|------|-------------|
| `code` | `string` | Card scheme code (e.g., `VISA`, `MASTERCARD`) |
| `message` | `string` | Human-readable message from the issuer |
| `network_details` | `NetworkErrorDetails` | Network-specific error details (advice code, decline code) |

### NetworkErrorDetails Fields

| Field | Type | Description |
|-------|------|-------------|
| `advice_code` | `string` | Network advice code for retry logic |
| `decline_code` | `string` | Card scheme decline code |
| `error_message` | `string` | Network-specific error details |

### ConnectorErrorDetails Fields

| Field | Type | Description |
|-------|------|-------------|
| `code` | `string` | Connector-specific error code (e.g., Stripe's `card_declined`) |
| `message` | `string` | Human-readable message from the connector |
| `reason` | `string` | Detailed explanation of why the error occurred |

### Error Response Example

```json
{
  "error": {
    "unified_details": {
      "code": "INSUFFICIENT_FUNDS",
      "message": "Your card has insufficient funds.",
      "description": "The payment was declined because the card does not have sufficient available credit or balance to complete the transaction.",
      "user_guidance_message": "Please try a different payment method or contact your bank."
    },
    "connector_details": {
      "code": "card_declined",
      "message": "Your card was declined.",
      "reason": "insufficient_funds"
    },
    "issuer_details": {
      "code": "VISA",
      "message": "Decline",
      "network_details": {
        "advice_code": "01",
        "decline_code": "51",
        "error_message": "Insufficient funds"
      }
    }
  }
}
```

## Domain Schema

See [Domain Schema](./domain-schema/README.md) for complete documentation of all data types, enums, and structures used across the API.

## Proto Files

The API is defined in Protocol Buffer files located at:
- `backend/grpc-api-types/proto/payment.proto` - Core payment types and errors
- `backend/grpc-api-types/proto/sdk_config.proto` - SDK and network error types
- `backend/grpc-api-types/proto/services.proto` - Service definitions
