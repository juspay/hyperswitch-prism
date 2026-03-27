# Payout Service

<!--
---
title: Payout Service (Node.js SDK)
description: Process payouts and fund transfers using the Node.js SDK
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

The Payout Service enables you to send funds to recipients using the Node.js SDK. Use this for marketplace payouts, refunds to bank accounts, supplier payments, and other fund disbursement needs.

**Business Use Cases:**
- **Marketplace payouts** - Pay sellers/merchants on your platform
- **Supplier payments** - Disburse funds to vendors and suppliers
- **Payroll** - Employee and contractor payments
- **Instant payouts** - Same-day transfers to connected accounts

## Operations

| Operation | Description | Use When |
|-----------|-------------|----------|
| [`create`](./create.md) | Create a payout. Initiates fund transfer to recipient. | Sending money to a recipient |
| [`transfer`](./transfer.md) | Create a payout fund transfer. Move funds between accounts. | Transferring between internal accounts |
| [`get`](./get.md) | Retrieve payout details. Check status and tracking. | Monitoring payout progress |
| [`void`](./void.md) | Cancel a pending payout. Stop before processing. | Aborting an incorrect payout |
| [`stage`](./stage.md) | Stage a payout for later processing. Prepare without sending. | Delayed payouts, batch processing |
| [`createLink`](./create-link.md) | Create link between recipient and payout. Associate payout with recipient. | Setting up recipient relationships |
| [`createRecipient`](./create-recipient.md) | Create payout recipient. Store recipient bank/payment details. | First time paying a new recipient |
| [`enrollDisburseAccount`](./enroll-disburse-account.md) | Enroll disburse account. Set up account for payouts. | Onboarding new payout accounts |

## SDK Setup

```javascript
const { PayoutClient } = require('hyperswitch-prism');

const payoutClient = new PayoutClient({
    connector: 'stripe',
    apiKey: 'YOUR_API_KEY',
    environment: 'SANDBOX'
});
```

## Payout Methods

| Method | Speed | Typical Use |
|--------|-------|-------------|
| **Bank transfer** | 1-3 business days | Standard payouts, large amounts |
| **Instant transfer** | Minutes | Same-day needs, existing recipients |
| **Card payout** | Instant | Prepaid cards, debit cards |

## Next Steps

- [createRecipient](./create-recipient.md) - Set up your first recipient
- [create](./create.md) - Send your first payout
- [Event Service](../event-service/README.md) - Handle payout webhooks
