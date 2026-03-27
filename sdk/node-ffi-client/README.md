# Connector Service - Node.js FFI Client

Node.js bindings for the connector-service FFI, providing high-level and low-level APIs for payment operations.

## Installation

```bash
npm install connector-service-node-ffi
```

## Prerequisites

- Node.js >= 10
- Built native addon at `artifacts/connector_service_ffi.node`

Build the native addon:
```bash
cd crates/ffi/ffi && cargo build --release --features napi
```

## API Levels

This SDK provides two API levels:

### 1. High-Level API (ConnectorClient) - Recommended

Simplified interface that handles the complete request/response flow.

```javascript
const { ConnectorClient } = require('connector-service-node-ffi');

// Create client with metadata
const metadata = {
    connector: 'Stripe',
    connector_auth_type: {
        auth_type: "HeaderKey",
        api_key: "sk_test_xxx"
    }
};

const client = new ConnectorClient(metadata);

// Authorize payment
const payload = {
    request_ref_id: { id: "payment_123" },
    amount: 1000,
    minor_amount: 1000,
    currency: "USD",
    payment_method: {
        payment_method: {
            Card: {
                card_number: "4111111111111111",
                card_exp_month: "12",
                card_exp_year: "2025",
                card_cvc: "123",
                card_holder_name: "John Doe",
                card_network: 1
            }
        }
    },
    capture_method: "AUTOMATIC",
    email: "customer@example.com",
    customer_name: "John Doe",
    auth_type: "NO_THREE_DS",
    enrolled_for_3ds: false,
    test_mode: true,
    order_details: [],
    address: {
        shipping_address: null,
        billing_address: null
    }
};

const result = await client.authorize(payload);
console.log(result);
```

### 2. Low-Level API (FFI Bindings) - Advanced

Direct access to FFI functions for maximum control.

```javascript
const { authorizeReq, authorizeRes } = require('connector-service-node-ffi');
const fetch = require('node-fetch');

const metadata = {
    connector: 'Stripe',
    connector_auth_type: {
        auth_type: "HeaderKey",
        api_key: "sk_test_xxx"
    }
};

const payload = { /* payment payload */ };

// Step 1: Build HTTP request
const requestJson = authorizeReq(payload, metadata);
const { body, headers, method, url } = JSON.parse(requestJson);

// Step 2: Execute HTTP request
const response = await fetch(url, {
    method,
    headers,
    body: body || undefined,
});

// Step 3: Collect response
const responseText = await response.text();
const responseHeaders = {};
response.headers.forEach((value, key) => {
    responseHeaders[key] = value;
});

const formattedResponse = {
    status: response.status,
    headers: responseHeaders,
    body: responseText
};

// Step 4: Parse response
const resultJson = authorizeRes(payload, metadata, formattedResponse);
const result = JSON.parse(resultJson);
console.log(result);
```

## API Reference

### ConnectorClient

#### Constructor

```javascript
new ConnectorClient(metadata)
```

**Parameters:**
- `metadata` (object): Metadata containing connector and authentication info
  - `connector` (string): Connector name (e.g., 'Stripe', 'Adyen')
  - `connector_auth_type` (object): Authentication configuration
    - `auth_type` (string): Authentication type (e.g., 'HeaderKey')
    - `api_key` (string): API key for HeaderKey auth

#### Methods

##### authorize(payload)

Authorize a payment.

**Parameters:**
- `payload` (object): Complete payment payload matching PaymentServiceAuthorizeRequest structure

**Returns:**
- `Promise<object>`: Payment response

**Throws:**
- `Error`: If authorization fails

### Low-Level Functions

#### authorizeReq(payload, metadata)

Build HTTP request for payment authorization.

**Parameters:**
- `payload` (object): Payment payload
- `metadata` (object): Connector metadata

**Returns:**
- `string`: JSON string containing `{ body, headers, method, url }`

#### authorizeRes(payload, metadata, response)

Parse payment authorization response.

**Parameters:**
- `payload` (object): Original payment payload
- `metadata` (object): Connector metadata
- `response` (object): HTTP response object with `{ status, headers, body }`

**Returns:**
- `string`: JSON string containing parsed payment response

## Testing

Run the test suite:

```bash
npm test
```

Or manually:

```bash
node tests/test_node.js
```

## Error Handling

The SDK provides enhanced error messages with context:

```javascript
try {
    const result = await client.authorize(payload);
} catch (error) {
    console.error('Authorization failed:', error.message);
    if (error.cause) {
        console.error('Caused by:', error.cause);
    }
}
```

## License

MIT