# hyperswitch-prism

**Universal Connector Service — Node.js SDK**

A high-performance, type-safe Node.js SDK for payment processing through the Universal Connector Service. Connect to 50+ payment processors (Stripe, PayPal, Adyen, and more) through a single, unified API.

[![npm version](https://badge.fury.io/js/hyperswitch-prism.svg)](https://www.npmjs.com/package/hyperswitch-prism)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)

---

## Features

- 🚀 **High Performance** — Direct UniFFI FFI bindings to Rust core, zero NAPI overhead
- 🔌 **50+ Connectors** — Single SDK for Stripe, PayPal, Adyen, and more
- 📘 **TypeScript Native** — Full type definitions with IntelliSense support
- ⚡ **Connection Pooling** — Built-in HTTP connection pooling for optimal throughput
- 🛡️ **Type-Safe** — Protobuf-based request/response serialization
- 🔧 **Configurable** — Per-request or global configuration for timeouts, proxies, and auth

---

## Installation

```bash
npm install hyperswitch-prism
```

**Requirements:**
- Node.js 18+ (LTS recommended)
- Rust toolchain (for building native bindings from source)

**Platform Support:**
- ✅ macOS (x64, arm64)
- ✅ Linux (x64, arm64)
- ✅ Windows (x64)

---

## Quick Start

### 1. Configure the Client

```typescript
import { PaymentClient, types } from 'hyperswitch-prism';

const { ConnectorConfig, RequestConfig, Environment, Connector } = types;

// Configure connector identity and authentication
const config = ConnectorConfig.create({
  connector: Connector.STRIPE,
  auth: {
    stripe: {
      apiKey: { value: 'sk_test_your_stripe_key' }
    }
  },
  environment: Environment.SANDBOX,
});

// Optional: Request defaults for timeouts
const defaults = RequestConfig.create({
  http: {
    totalTimeoutMs: 30000,
    connectTimeoutMs: 10000,
  }
});
```

### 2. Process a Payment

```typescript
const client = new PaymentClient(config, defaults);

const { PaymentServiceAuthorizeRequest, Currency, CaptureMethod } = types;

const request = PaymentServiceAuthorizeRequest.create({
  merchantTransactionId: 'txn_order_001',
  amount: {
    minorAmount: 1000,  // $10.00
    currency: Currency.USD,
  },
  captureMethod: CaptureMethod.AUTOMATIC,
  paymentMethod: {
    card: {
      cardNumber: { value: '4111111111111111' },
      cardExpMonth: { value: '12' },
      cardExpYear: { value: '2027' },
      cardCvc: { value: '123' },
      cardHolderName: { value: 'John Doe' },
    }
  },
  customer: {
    email: { value: 'customer@example.com' },
    name: 'John Doe',
  },
  testMode: true,
});

const response = await client.authorize(request);
console.log('Status:', response.status);
console.log('Transaction ID:', response.connectorTransactionId);
```

---

## Service Clients

The SDK provides specialized clients for different service domains:

| Client | Purpose | Key Methods |
|--------|---------|-------------|
| `PaymentClient` | Core payment operations | `authorize()`, `capture()`, `refund()`, `void()` |
| `CustomerClient` | Customer management | `create()` |
| `PaymentMethodClient` | Secure tokenization | `tokenize()` |
| `MerchantAuthenticationClient` | Auth token management | `createAccessToken()`, `createSessionToken()` |
| `EventClient` | Webhook processing | `handleEvent()` |
| `RecurringPaymentClient` | Subscription billing | `charge()` |
| `PaymentMethodAuthenticationClient` | 3DS authentication | `preAuthenticate()`, `authenticate()`, `postAuthenticate()` |

---

## Authentication Examples

### Stripe (HeaderKey)

```typescript
const config = ConnectorConfig.create({
  connector: Connector.STRIPE,
  auth: {
    stripe: {
      apiKey: { value: 'sk_test_xxx' }
    }
  },
  environment: Environment.SANDBOX,
});
```

### PayPal (SignatureKey)

```typescript
const config = ConnectorConfig.create({
  connector: Connector.PAYPAL,
  auth: {
    paypal: {
      clientId: { value: 'client_id' },
      clientSecret: { value: 'client_secret' }
    }
  },
  environment: Environment.SANDBOX,
});
```

---

## Advanced Configuration

### Proxy Settings

```typescript
const defaults = RequestConfig.create({
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
  http: {
    totalTimeoutMs: 60000,  // Override for this request only
  }
});
```

### Connection Pooling

Each client instance maintains its own connection pool. For best performance:

```typescript
// ✅ Create client once, reuse for multiple requests
const client = new PaymentClient(config, defaults);

for (const payment of payments) {
  await client.authorize(payment);
}
```

---

## Error Handling

```typescript
import { ConnectorError } from 'hyperswitch-prism';

try {
  const response = await client.authorize(request);
} catch (error) {
  if (error instanceof ConnectorError) {
    console.error('Code:', error.errorCode);
    console.error('Status:', error.statusCode);
    console.error('Message:', error.message);
  }
}
```

### Error Codes

| Code | Description |
|------|-------------|
| `CONNECT_TIMEOUT` | Failed to establish connection |
| `RESPONSE_TIMEOUT` | No response received from gateway |
| `TOTAL_TIMEOUT` | Overall request timeout exceeded |
| `NETWORK_FAILURE` | General network error |
| `INVALID_CONFIGURATION` | Configuration error |
| `CLIENT_INITIALIZATION` | SDK initialization failed |

---

## Complete Example: PayPal with Access Token

```typescript
import {
  PaymentClient,
  MerchantAuthenticationClient,
  types
} from 'hyperswitch-prism';

const { ConnectorConfig, Environment, Connector, Currency,
        CaptureMethod, SecretString, AccessToken, ConnectorState } = types;

const config = ConnectorConfig.create({
  connector: Connector.PAYPAL,
  auth: {
    paypal: {
      clientId: { value: 'YOUR_CLIENT_ID' },
      clientSecret: { value: 'YOUR_CLIENT_SECRET' }
    }
  },
  environment: Environment.SANDBOX,
});

// Step 1: Get access token
const authClient = new MerchantAuthenticationClient(config);
const tokenResponse = await authClient.createAccessToken({
  merchantAccessTokenId: 'token_001',
  connector: Connector.PAYPAL,
  testMode: true,
});

// Step 2: Authorize with access token
const paymentClient = new PaymentClient(config);
const paymentResponse = await paymentClient.authorize({
  merchantTransactionId: 'txn_001',
  amount: {
    minorAmount: 1000,
    currency: Currency.USD,
  },
  captureMethod: CaptureMethod.AUTOMATIC,
  paymentMethod: {
    card: {
      cardNumber: { value: '4111111111111111' },
      cardExpMonth: { value: '12' },
      cardExpYear: { value: '2027' },
      cardCvc: { value: '123' },
    }
  },
  state: ConnectorState.create({
    accessToken: AccessToken.create({
      token: SecretString.create({ value: tokenResponse.accessToken.value }),
      tokenType: 'Bearer',
      expiresInSeconds: tokenResponse.expiresInSeconds,
    }),
  }),
  testMode: true,
});

console.log('Payment status:', paymentResponse.status);
```

---

## Architecture

```
Your App → Service Client → ConnectorClient → UniFFI FFI → Rust Core → Connector API
                ↓
         Connection Pool (undici)
```

The SDK uses:
- **koffi** — FFI bindings to Rust
- **protobufjs** — Protocol buffer serialization
- **undici** — High-performance HTTP client with connection pooling

---

## Building from Source

```bash
# Clone the repository
git clone https://github.com/juspay/connector-service.git
cd connector-service/sdk/javascript

# Build native library, generate bindings, and pack
make pack

# Run tests
make test-pack

# With live API credentials
STRIPE_API_KEY=sk_test_xxx make test-pack
```