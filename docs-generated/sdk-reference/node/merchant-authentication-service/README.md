# Merchant Authentication Service

<!--
---
title: Merchant Authentication Service (Node.js SDK)
description: Generate access tokens and session credentials using the Node.js SDK
last_updated: 2026-03-21
generated_from: backend/grpc-api-types/proto/services.proto
auto_generated: true
reviewed_by: ''
reviewed_at: ''
approved: false
sdk_language: node
---
-->

## Overview

The Merchant Authentication Service generates secure credentials for accessing payment processor APIs using the Node.js SDK. These short-lived tokens provide secure access without storing secrets client-side.

**Business Use Cases:**
- **Frontend SDKs** - Generate tokens for client-side payment flows
- **Wallet payments** - Initialize Apple Pay, Google Pay sessions
- **Session management** - Maintain secure state across payment operations
- **Multi-party payments** - Secure delegated access

## Operations

| Operation | Description | Use When |
|-----------|-------------|----------|
| [`createAccessToken`](./create-access-token.md) | Generate short-lived connector authentication token. Provides secure API access credentials. | Need temporary API access token |
| [`createSessionToken`](./create-session-token.md) | Create session token for payment processing. Maintains session state across operations. | Starting a multi-step payment flow |
| [`createSdkSessionToken`](./create-sdk-session-token.md) | Initialize wallet payment sessions. Sets up Apple Pay, Google Pay context. | Enabling wallet payments |

## SDK Setup

```javascript
const { MerchantAuthenticationClient } = require('hyperswitch-prism');

const authClient = new MerchantAuthenticationClient({
    connector: 'stripe',
    apiKey: 'YOUR_API_KEY',
    environment: 'SANDBOX'
});
```

## Security Best Practices

- Never store tokens long-term
- Use tokens immediately after creation
- Handle token expiration gracefully
- Use HTTPS for all token transmissions

## Next Steps

- [Payment Service](../payment-service/README.md) - Use tokens for payment operations
- [Payment Method Authentication Service](../payment-method-authentication-service/README.md) - 3D Secure authentication
