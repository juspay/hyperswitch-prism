# Payout Service

<!--
---
title: Payout Service (Python SDK)
description: Process payouts and fund transfers using the Python SDK
last_updated: 2026-03-21
generated_from: backend/grpc-api-types/proto/services.proto
auto_generated: true
reviewed_by: ''
reviewed_at: ''
approved: false
sdk_language: python
---
-->

## Overview

The Payout Service enables you to send funds to recipients using the Python SDK. Use this for marketplace payouts, refunds to bank accounts, supplier payments, and other fund disbursement needs.

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
| [`create_link`](./create-link.md) | Create link between recipient and payout. Associate payout with recipient. | Setting up recipient relationships |
| [`create_recipient`](./create-recipient.md) | Create payout recipient. Store recipient bank/payment details. | First time paying a new recipient |
| [`enroll_disburse_account`](./enroll-disburse-account.md) | Enroll disburse account. Set up account for payouts. | Onboarding new payout accounts |

## SDK Setup

```python
from hyperswitch_prism import PayoutClient

payout_client = PayoutClient(
    connector='stripe',
    api_key='YOUR_API_KEY',
    environment='SANDBOX'
)
```

## Common Patterns

### Marketplace Payout Flow

```mermaid
sequenceDiagram
    participant App as Your App
    participant CS as Prism
    participant PP as Payment Provider
    participant R as Recipient

    Note over App: Customer pays seller
    App->>CS: 1. create_recipient
    CS->>PP: Store recipient details
    PP-->>CS: Return recipient_id
    CS-->>App: Return recipient_id
    Note over App: Ready to pay seller
    App->>CS: 2. create payout
    CS->>PP: Initiate transfer
    PP-->>CS: Return: PENDING
    CS-->>App: Return payout_id
    Note over PP,R: Funds transferred
    PP->>R: Deposit funds
    PP-->>App: Webhook: SUCCEEDED
```

**Flow Explanation:**

1. **Create recipient** - Store seller's payout details (bank account, etc.).

2. **Create payout** - Initiate the fund transfer to the seller.

3. **Monitor status** - Track payout status until funds arrive.

## Payout Methods

| Method | Speed | Typical Use |
|--------|-------|-------------|
| **Bank transfer** | 1-3 business days | Standard payouts, large amounts |
| **Instant transfer** | Minutes | Same-day needs, existing recipients |
| **Card payout** | Instant | Prepaid cards, debit cards |

## Next Steps

- [create_recipient](./create-recipient.md) - Set up your first recipient
- [create](./create.md) - Send your first payout
- [Event Service](../event-service/README.md) - Handle payout webhooks
