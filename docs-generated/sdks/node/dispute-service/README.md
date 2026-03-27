# Dispute Service

<!--
---
title: Dispute Service (Node.js SDK)
description: Manage chargeback lifecycle and defend against fraudulent transaction claims using the Node.js SDK
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

The Dispute Service helps you manage chargeback disputes across payment processors using the Node.js SDK. When customers dispute transactions with their banks, this service enables you to track dispute status, submit evidence, and make informed decisions.

**Business Use Cases:**
- **E-commerce fraud defense** - Submit delivery proof and receipts to contest illegitimate chargebacks
- **Service businesses** - Provide service documentation and customer communication records
- **Subscription disputes** - Submit recurring transaction agreements and cancellation policies
- **Revenue recovery** - Defend valid transactions to minimize chargeback losses

## Operations

| Operation | Description | Use When |
|-----------|-------------|----------|
| [`submitEvidence`](./submit-evidence.md) | Upload evidence to dispute customer chargeback. Provides documentation to contest fraudulent transaction claims. | You have proof of delivery, service, or customer acceptance |
| [`get`](./get.md) | Retrieve dispute status and evidence submission state. Tracks dispute progress through bank review process. | Check dispute status, review evidence deadlines |
| [`defend`](./defend.md) | Submit defense with reason code for dispute. Presents formal argument against customer's chargeback claim. | Contesting the dispute with formal defense |
| [`accept`](./accept.md) | Concede dispute and accepts chargeback loss. Acknowledges liability when evidence is insufficient. | Evidence is insufficient, cost of defense exceeds dispute amount |

## SDK Setup

```javascript
const { DisputeClient } = require('hyperswitch-prism');

const disputeClient = new DisputeClient({
    connector: 'stripe',
    apiKey: 'YOUR_API_KEY',
    environment: 'SANDBOX'
});
```

## Next Steps

- [Payment Service](../payment-service/README.md) - Review original payment details
- [Refund Service](../refund-service/README.md) - Process voluntary refunds to avoid disputes
- [Event Service](../event-service/README.md) - Handle dispute webhook notifications
