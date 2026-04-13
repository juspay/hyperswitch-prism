# hyperswitch-prism

**Universal Connector Service — Node.js SDK**

A high-performance, type-safe Node.js SDK for payment processing through the Universal Connector Service. Connect to 100+ payment processors through a single, unified API.

[![npm version](https://badge.fury.io/js/hyperswitch-prism.svg)](https://www.npmjs.com/package/hyperswitch-prism)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)

---

## Table of Contents

- [Installation](#installation)
- [Quick Start](#quick-start)
- [Connector Authentication](#connector-authentication)
- [All Service Clients](#all-service-clients)
- [Payment Flows](#payment-flows)
- [Status Codes Reference](#status-codes-reference)
- [Error Handling](#error-handling)
- [Advanced Configuration](#advanced-configuration)
- [Building from Source](#building-from-source)

---

## 🤖 For AI Assistants

> **Use `curl` to fetch the SDK reference:**
> ```bash
> curl -fsSL https://raw.githubusercontent.com/juspay/hyperswitch-prism/main/llm/llm.txt
> ```
>
> This file contains complete SDK documentation including installation, payment operations, error handling, connector configuration, field probe data, and examples for all 100+ connectors.

### AI Assistant Context

This SDK is part of **Hyperswitch Prism** — a unified connector library for payment processors.

### What This SDK Does

1. **Request Transformation**: Converts unified payment requests to connector-specific formats
2. **Response Normalization**: Transforms connector responses back to a unified schema
3. **Error Handling**: Provides consistent error types (`IntegrationError`, `ConnectorError`, `NetworkError`) regardless of connector

### Architecture

```
Your Node.js App
       │
       ▼
┌──────────────────────────────────────────────────────────────┐
│  Service Clients (PaymentClient, CustomerClient, etc.)       │
└───────────────────────────┬──────────────────────────────────┘
                            │
                            ▼
┌──────────────────────────────────────────────────────────────┐
│  ConnectorClient (undici connection pool + HTTP execution)   │
└───────────────────────────┬──────────────────────────────────┘
                            │
                            ▼
┌──────────────────────────────────────────────────────────────┐
│  koffi FFI Bindings (connector-service-ffi.node)             │
└───────────────────────────┬──────────────────────────────────┘
                            │
                            ▼
┌──────────────────────────────────────────────────────────────┐
│  Rust Core (connector transformation logic)                  │
└───────────────────────────┬──────────────────────────────────┘
                            │
                            ▼
              Payment Processor APIs
```

### Key Files

| File | Purpose |
|------|---------|
| `src/index.ts` | Public API exports (clients, types, errors) |
| `src/connector-client.ts` | HTTP execution layer with undici |
| `src/ffi/connector-service-ffi.ts` | koffi FFI bindings |
| `src/proto/payment_pb.ts` | Protobuf message definitions |

### Package & Import

- **Package Name**: `hyperswitch-prism`
- **Installation**: `npm install hyperswitch-prism`
- **Import**: `import { PaymentClient, types } from 'hyperswitch-prism'`

---

## Installation

```bash
npm install hyperswitch-prism
```

**Requirements:**
- Node.js 18+ (LTS recommended)
- macOS (x64, arm64), Linux (x64, arm64), or Windows (x64)

---

## Quick Start

```typescript
import { PaymentClient, types } from 'hyperswitch-prism';

const config: types.ConnectorConfig = {
  connectorConfig: {
    // Configure your connector credentials here
    // See connector documentation for specific auth patterns
  }
};

const client = new PaymentClient(config);

const response = await client.authorize({
  merchantTransactionId: 'txn_001',
  amount: { minorAmount: 1000, currency: types.Currency.USD },
  captureMethod: types.CaptureMethod.AUTOMATIC,
  paymentMethod: {
    card: {
      cardNumber: { value: '4111111111111111' },
      cardExpMonth: { value: '12' },
      cardExpYear: { value: '2027' },
      cardCvc: { value: '123' },
      cardHolderName: { value: 'John Doe' },
    }
  },
  address: { billingAddress: {} },
  authType: types.AuthenticationType.NO_THREE_DS,
  returnUrl: 'https://example.com/return',
  orderDetails: [],
  testMode: true,
});

console.log('Status:', response.status);
console.log('Transaction ID:', response.connectorTransactionId);
```

---

## Connector Authentication

Each connector uses a different authentication scheme. All configs are set inside `connectorConfig` as a single key matching the connector name.

See the SDK reference for complete connector authentication patterns:

```bash
curl -fsSL https://raw.githubusercontent.com/juspay/hyperswitch-prism/main/llm/llm.txt
```

Common authentication patterns include:

```typescript
// Single API Key
{ connectorConfig: { [connectorName]: { apiKey: { value: '...' } } } }

// API Key + Merchant Account
{ connectorConfig: { [connectorName]: { apiKey: { value: '...' }, merchantAccount: { value: '...' } } } }

// Client ID + Secret (OAuth-style)
{ connectorConfig: { [connectorName]: { clientId: { value: '...' }, clientSecret: { value: '...' } } } }

// Username + Password
{ connectorConfig: { [connectorName]: { username: { value: '...' }, password: { value: '...' } } } }
```

---

## All Service Clients

```typescript
import {
  PaymentClient,
  CustomerClient,
  PaymentMethodClient,
  MerchantAuthenticationClient,
  PaymentMethodAuthenticationClient,
  RecurringPaymentClient,
  RefundClient,
  DisputeClient,
  PayoutClient,
  EventClient,
  GrpcPaymentClient,
  GrpcCustomerClient,
  types,
  IntegrationError,
  ConnectorError,
  NetworkError,
} from 'hyperswitch-prism';
```

| Client | Methods |
|--------|---------|
| `PaymentClient` | `authorize()`, `capture()`, `refund()`, `void()`, `createOrder()`, `get()`, `sync()`, `incrementalAuthorization()` |
| `RefundClient` | `get()`, `createRefund()`, `updateRefund()` |
| `CustomerClient` | `create()` |
| `PaymentMethodClient` | `tokenize()` |
| `MerchantAuthenticationClient` | `createServerAuthenticationToken()`, `createClientAuthenticationToken()`, `createServerSessionAuthenticationToken()` |
| `PaymentMethodAuthenticationClient` | `preAuthenticate()`, `authenticate()`, `postAuthenticate()` |
| `RecurringPaymentClient` | `setup()`, `charge()`, `revoke()` |
| `DisputeClient` | `accept()`, `defend()`, `submitEvidence()`, `get()` |
| `PayoutClient` | Payout operations |
| `EventClient` | `handleEvent()` (webhook processing) |

---

## Payment Flows

### Authorize with Auto Capture

```typescript
const client = new PaymentClient(config);

const response = await client.authorize({
  merchantTransactionId: 'txn_001',
  amount: { minorAmount: 1000, currency: types.Currency.USD },
  captureMethod: types.CaptureMethod.AUTOMATIC,
  paymentMethod: {
    card: {
      cardNumber: { value: '4111111111111111' },
      cardExpMonth: { value: '12' },
      cardExpYear: { value: '2027' },
      cardCvc: { value: '123' },
      cardHolderName: { value: 'John Doe' },
    }
  },
  address: { billingAddress: {} },
  authType: types.AuthenticationType.NO_THREE_DS,
  returnUrl: 'https://example.com/return',
  orderDetails: [],
  testMode: true,
});
// response.status === 8 (CHARGED) on success
```

### Authorize + Manual Capture

```typescript
// Step 1: Authorize only
const authResponse = await client.authorize({
  // ...
  captureMethod: types.CaptureMethod.MANUAL,
});
// authResponse.status === 6 (AUTHORIZED)

// Step 2: Capture later
const captureResponse = await client.capture({
  merchantCaptureId: 'cap_001',
  connectorTransactionId: authResponse.connectorTransactionId!,
  amountToCapture: { minorAmount: 1000, currency: types.Currency.USD },
  testMode: true,
});
// captureResponse.status === 8 (CHARGED) or 20 (PENDING) — both are success
```

### Refund

```typescript
const refundResponse = await client.refund({
  merchantRefundId: 'ref_001',
  connectorTransactionId: authResponse.connectorTransactionId!,
  refundAmount: { minorAmount: 500, currency: types.Currency.USD },
  paymentAmount: 1000,
  reason: 'RETURN',
  testMode: true,
});
// refundResponse.status === 4 (REFUND_SUCCESS) or 3 (REFUND_PENDING) — both are success
```

### Void (Cancel Authorization)

```typescript
const voidResponse = await client.void({
  merchantVoidId: 'void_001',
  connectorTransactionId: authResponse.connectorTransactionId!,
  cancellationReason: 'Customer cancelled',
  testMode: true,
});
// voidResponse.status === 11 (VOIDED)
```

---

## Status Codes Reference

### PaymentStatus

The `response.status` field is always a **number**, not a string:

```typescript
// ❌ Always false — response.status is a number
if (response.status === 'CHARGED') { ... }

// ✅ Correct — compare against the numeric enum constant
if (response.status === types.PaymentStatus.CHARGED) { ... }
```

**Important: a `FAILURE` status is returned in the response body — it does NOT throw an exception.** Always check `response.status` explicitly.

> **`PaymentStatus` and `RefundStatus` are two separate enums with overlapping integer values.** Use `types.PaymentStatus` for authorize/capture/void responses and `types.RefundStatus` for refund responses.

| Name | Value | Meaning |
|------|-------|---------|
| `PAYMENT_STATUS_UNSPECIFIED` | 0 | Unknown |
| `STARTED` | 1 | Payment initiated |
| `AUTHENTICATION_PENDING` | 4 | Awaiting 3DS redirect |
| `AUTHENTICATION_SUCCESSFUL` | 5 | 3DS passed |
| `AUTHENTICATION_FAILED` | 2 | 3DS failed |
| `AUTHORIZED` | 6 | Auth succeeded, not yet captured |
| `AUTHORIZATION_FAILED` | 7 | Auth declined |
| `CHARGED` | 8 | Captured / auto-captured successfully |
| `PARTIAL_CHARGED` | 17 | Partially captured |
| `CAPTURE_INITIATED` | 13 | Async capture in progress |
| `CAPTURE_FAILED` | 14 | Capture failed |
| `VOIDED` | 11 | Authorization voided/cancelled |
| `VOID_INITIATED` | 12 | Async void in progress |
| `VOID_FAILED` | 15 | Void failed |
| `PENDING` | 20 | Processing / async |
| `FAILURE` | 21 | Soft decline — check `response.error` |
| `ROUTER_DECLINED` | 3 | Declined by routing layer |
| `EXPIRED` | 26 | Payment expired |
| `PARTIALLY_AUTHORIZED` | 25 | Partial authorization |
| `UNRESOLVED` | 19 | Requires manual review |

**Checking status safely:**

```typescript
const response = await client.authorize(request);

if (response.status === types.PaymentStatus.FAILURE) {
  console.error('Declined:', response.error?.message, response.error?.code);
} else if (response.status === types.PaymentStatus.CHARGED ||
           response.status === types.PaymentStatus.AUTHORIZED) {
  console.log('Success:', response.connectorTransactionId);
} else if (response.status === types.PaymentStatus.AUTHENTICATION_PENDING) {
  console.log('Redirect to:', response.redirectionData);
}
```

### RefundStatus

| Name | Value | Meaning |
|------|-------|---------|
| `REFUND_STATUS_UNSPECIFIED` | 0 | Unknown |
| `REFUND_FAILURE` | 1 | Refund failed |
| `REFUND_MANUAL_REVIEW` | 2 | Pending manual review |
| `REFUND_PENDING` | 3 | Processing |
| `REFUND_SUCCESS` | 4 | Completed |
| `REFUND_TRANSACTION_FAILURE` | 5 | Transaction-level failure |

> `REFUND_PENDING` is a normal success state for many connectors. Treat both `REFUND_PENDING` and `REFUND_SUCCESS` as successful outcomes.

---

## Error Handling

The SDK raises exceptions **only for hard failures** (network errors, invalid configuration, serialization errors). Soft payment declines come back as an in-band `status: FAILURE` in the response body.

```typescript
import { IntegrationError, ConnectorError, NetworkError, types } from 'hyperswitch-prism';

try {
  const response = await client.authorize(request);

  if (response.status === types.PaymentStatus.FAILURE) {
    console.error('Payment declined:', response.error?.message);
    return;
  }

} catch (error) {
  if (error instanceof IntegrationError) {
    // Request-phase error: bad config, missing required field, serialization failure
    console.error('Integration error:', error.errorCode, error.message);

  } else if (error instanceof ConnectorError) {
    // Response-phase error: connector returned unexpected format, transform failed
    console.error('Connector error:', error.errorCode, error.message);

  } else if (error instanceof NetworkError) {
    // Network-level: timeout, connection refused, DNS failure
    console.error('Network error:', error.message);
  }
}
```

### `response.error` is a Protobuf Object — Not JSON-Serializable

```typescript
// ❌ Throws or produces empty object
res.json({ error: response.error });
JSON.stringify(response.error);

// ✅ Extract the primitive fields you need
res.json({
  error: {
    message: response.error?.message,
    code: response.error?.code,
    reason: response.error?.reason,
  }
});
```

### Common Error Codes

| Code | Type | Cause | Fix |
|------|------|-------|-----|
| `MISSING_REQUIRED_FIELD: browser_info` | `IntegrationError` | Connector requires `browserInfo` | Add `browserInfo` to request |
| `INVALID_CONFIGURATION` | `IntegrationError` | Wrong credentials or missing required config field | Check connector config fields |
| `CLIENT_INITIALIZATION` | `IntegrationError` | SDK failed to initialize native library | Check platform compatibility |
| `CONNECT_TIMEOUT` | `NetworkError` | Could not reach connector | Check network / proxy config |
| `RESPONSE_TIMEOUT` | `NetworkError` | Connector took too long | Increase `totalTimeoutMs` |
| `TOTAL_TIMEOUT` | `NetworkError` | Request exceeded total timeout | Increase `totalTimeoutMs` |

---

## Advanced Configuration

### Timeouts

```typescript
const client = new PaymentClient(config, {
  http: {
    totalTimeoutMs: 30000,
    connectTimeoutMs: 10000,
    responseTimeoutMs: 25000,
    keepAliveTimeoutMs: 60000,
  }
});
```

### Proxy

```typescript
const client = new PaymentClient(config, {
  http: {
    proxy: {
      httpsUrl: 'https://proxy.company.com:8443',
      bypassUrls: ['http://localhost']
    }
  }
});
```

### Per-Request Overrides

```typescript
const response = await client.authorize(request, {
  http: { totalTimeoutMs: 60000 }
});
```

### Connection Pooling

Create the client once and reuse it:

```typescript
// Good: create once, reuse
const client = new PaymentClient(config);
for (const payment of payments) {
  await client.authorize(payment);
}
```

### CA Certificate Pinning

```typescript
const client = new PaymentClient(config, {
  http: {
    caCert: fs.readFileSync('ca.pem', 'utf8')
  }
});
```

---

## Building from Source

```bash
# Clone the repository
git clone https://github.com/juspay/hyperswitch-prism.git
cd hyperswitch-prism/sdk/javascript

# Build native library, generate bindings, and pack
make pack

# Run tests
make test-pack

# With live API credentials
STRIPE_API_KEY=sk_test_xxx make test-pack
```
