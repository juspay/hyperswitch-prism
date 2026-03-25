---
name: add-payment-method
description: >
  Adds payment method support (Card, Wallet, Bank Transfer, UPI, BNPL, etc.) to an existing
  connector in the connector-service (UCS) Rust codebase. Modifies the Authorize flow transformers
  to handle new payment method data types. Use when a connector exists with Authorize flow
  but needs to support additional payment methods.
license: Apache-2.0
compatibility: Requires Rust toolchain with cargo. Linux or macOS.
metadata:
  author: parallal
  version: "2.0"
  domain: payment-connectors
---

# Add Payment Method

## Overview

Adds payment method support to an existing connector's Authorize flow.

**MANDATORY SUBAGENT DELEGATION: You are the orchestrator. You MUST delegate every step
to a subagent using the prompts in `references/subagent-prompts.md`. Do NOT implement
code, run tests, or review quality yourself. Spawn subagents and coordinate their outputs.**

**Inputs:** connector name + payment methods to add (e.g., "add Apple Pay and Google Pay to AcmePay")
**Output:** payment methods implemented, tested, quality-reviewed
**Prerequisite:** connector must have Authorize flow implemented

## Payment Method Categories

| Category | PaymentMethodData Variant | Pattern File |
|----------|--------------------------|-------------|
| Card | `PaymentMethodData::Card(card)` | `references/payment-method-patterns/card.md` |
| Wallet | `PaymentMethodData::Wallet(wallet)` | `references/payment-method-patterns/wallet.md` |
| BankTransfer | `PaymentMethodData::BankTransfer(bt)` (Box) | `references/payment-method-patterns/bank-transfer.md` |
| BankDebit | `PaymentMethodData::BankDebit(bd)` | `references/payment-method-patterns/bank-debit.md` |
| BankRedirect | `PaymentMethodData::BankRedirect(br)` | `references/payment-method-patterns/bank-redirect.md` |
| UPI | `PaymentMethodData::Upi(upi)` | `references/payment-method-patterns/upi.md` |
| BNPL | `PaymentMethodData::PayLater(pl)` | `references/payment-method-patterns/bnpl.md` |
| Crypto | `PaymentMethodData::Crypto(crypto)` | `references/payment-method-patterns/crypto.md` |
| GiftCard | `PaymentMethodData::GiftCard(gc)` (Box) | `references/payment-method-patterns/gift-card.md` |
| MobilePayment | `PaymentMethodData::MobilePayment(mp)` | `references/payment-method-patterns/mobile-payment.md` |
| Reward | `PaymentMethodData::Reward` | `references/payment-method-patterns/reward.md` |

Full PM name → category mapping: `references/category-mapping.md`

## Critical Rules

- Never use catch-all `_` to silently drop payment methods -- always return `ConnectorError::NotImplemented`
- Each payment method gets its own explicit match arm
- `BankTransferData` and `GiftCardData` are Box-wrapped -- use `.deref()`
- Wallet sub-variants (ApplePay, GooglePay, etc.) each need separate nested match arms
- Validate required fields with `missing_field_err`
- Use `get_unimplemented_payment_method_error_message("ConnectorName")` for error messages

---

## Workflow: Orchestrator Sequence

**Full subagent prompts:** `references/subagent-prompts.md`

### Step 1: Analysis & Category Resolution (Subagent)

> **Subagent prompt:** `references/subagent-prompts.md` → Subagent 1

**Inputs:** connector_name, requested_payment_methods

**What it does:**
1. Verifies connector exists and has Authorize flow
2. Reads tech spec for PM-specific API requirements
3. Maps each PM to its `PaymentMethodData` category (via `references/category-mapping.md`)
4. Identifies which PMs are already supported
5. Checks if Refund/Capture flows need PM-specific changes

**Outputs:** category mapping per PM, existing PMs, implementation plan

**Gates:**
- If tech spec missing → invoke the `generate-tech-spec` skill first. Do NOT proceed without it.
- If Authorize flow missing → invoke the `add-connector-flow` skill to add Authorize first.

---

### Step 2: Payment Method Implementation (MANDATORY subagent per PM or category)

> **CRITICAL: You MUST delegate implementation to a subagent. Do NOT implement code yourself.**
> Read the subagent prompt from `references/subagent-prompts.md` → Subagent 2, fill in the
> variables ({ConnectorName}, {PaymentMethod}, {Category}, pattern file path), and spawn a subagent.
> For multiple PMs in the same category (e.g., Apple Pay + Google Pay = both Wallet), you may
> use one subagent for the whole category.

> **Per-category patterns:** `references/payment-method-patterns/{category}.md`

Each PM subagent:
1. Reads the category pattern file
2. Finds the `match payment_method_data` block in the Authorize TryFrom
3. Adds match arm for the new PM variant
4. Extracts fields, builds connector request
5. Handles unsupported sub-variants with `NotImplemented`
6. Propagates to Refund/Capture if needed
7. Runs `cargo build --package connector-integration`

---

### Step 3: gRPC Testing (MANDATORY subagent)

> **CRITICAL: You MUST delegate testing to a subagent. Do NOT run grpcurl yourself.**
> **Subagent prompt:** `references/subagent-prompts.md` → Subagent 3
> **Testing guide:** `references/grpc-testing-guide.md`

Tests Authorize with each new payment method via grpcurl. The `payment_method` field
in the grpcurl request changes per PM type:

| PM | grpcurl payment_method field |
|----|------------------------------|
| Card | `{"card": {"card_number": {"value":"4111..."}, ...}}` |
| Apple Pay | `{"wallet": {"apple_pay_third_party_sdk": {"payment_data":"..."}}}` |
| Google Pay | `{"wallet": {"google_pay": {"tokenization_data":{"token":"..."}}}}` |
| UPI | `{"upi": {"upi_collect": {"vpa_id":{"value":"test@upi"}}}}` |
| ACH | `{"bank_debit": {"ach": {"account_number":{"value":"..."}, ...}}}` |

Also tests Refund/Capture with new PMs if those flows were modified.

---

### Step 4: Quality Review (MANDATORY subagent)

> **CRITICAL: You MUST delegate quality review to a subagent. Do NOT review yourself.**
> **Subagent prompt:** `references/subagent-prompts.md` → Subagent 4

**Checks:**
- Each PM has explicit match arm (no silent drops)
- Unsupported variants return `NotImplemented` with connector name
- Required fields validated with `missing_field_err`
- Box-wrapped types properly `.deref()`'d
- `cargo build` passes clean

---

## PaymentMethodData Match Pattern

The central pattern for all PM handling is a `match` on `PaymentMethodData` inside the
Authorize flow's `TryFrom`. This is the comprehensive pattern:

```rust
let payment_method_data = &item.router_data.request.payment_method_data;

match payment_method_data {
    // ---- Card ----
    PaymentMethodData::Card(card) => {
        let card_number = card.card_number.clone();
        let expiry_month = card.card_exp_month.clone();
        let cvc = card.card_cvc.clone();
        Ok(ConnectorPaymentsRequest { payment_type: "card", card_number, ... })
    },

    // ---- Wallet (nested match for sub-variants) ----
    PaymentMethodData::Wallet(wallet_data) => match wallet_data {
        WalletData::ApplePayThirdPartySdk(apple_pay) => {
            let token = apple_pay.payment_data.clone()
                .ok_or_else(missing_field_err("apple_pay.payment_data"))?;
            Ok(ConnectorPaymentsRequest { payment_type: "applepay", token, ... })
        },
        WalletData::GooglePay(google_pay) => {
            let token = google_pay.tokenization_data.token.clone();
            Ok(ConnectorPaymentsRequest { payment_type: "googlepay", token, ... })
        },
        _ => Err(errors::ConnectorError::NotImplemented(
            utils::get_unimplemented_payment_method_error_message("ConnectorName"),
        ).into()),
    },

    // ---- Bank Transfer (Box-wrapped, must .deref()) ----
    PaymentMethodData::BankTransfer(bt) => match bt.deref() {
        BankTransferData::SepaBankTransfer { .. } => { ... },
        _ => Err(errors::ConnectorError::NotImplemented(...).into()),
    },

    // ---- Bank Debit ----
    PaymentMethodData::BankDebit(bd) => match bd {
        BankDebitData::SepaBankDebit { iban, .. } => { ... },
        _ => Err(errors::ConnectorError::NotImplemented(...).into()),
    },

    // ---- UPI ----
    PaymentMethodData::Upi(upi) => match upi {
        UpiData::UpiCollect(c) => {
            let vpa = c.vpa_id.clone().ok_or_else(missing_field_err("vpa_id"))?;
            Ok(ConnectorPaymentsRequest { payment_type: "upi_collect", vpa, ... })
        },
        _ => Err(errors::ConnectorError::NotImplemented(...).into()),
    },

    // ---- BNPL ----
    PaymentMethodData::PayLater(pl) => match pl {
        PayLaterData::KlarnaRedirect { .. } => { ... },
        _ => Err(errors::ConnectorError::NotImplemented(...).into()),
    },

    // ---- Catch-all: explicit rejection ----
    _ => Err(errors::ConnectorError::NotImplemented(
        utils::get_unimplemented_payment_method_error_message("ConnectorName"),
    ).into()),
}
```

**Key rules:**
- Outer match dispatches on `PaymentMethodData` variants
- Categories with sub-types need nested match (Wallet, BankTransfer, BankDebit, UPI, BNPL)
- `BankTransferData` and `GiftCardData` are Box-wrapped → `.deref()`
- Every level ends with catch-all returning `NotImplemented`
- `Reward` is a unit variant with no inner data

---

## Reference Index

| Path | Contents |
|------|----------|
| `references/subagent-prompts.md` | Full prompts for all 4 subagents |
| `references/category-mapping.md` | PM name → PaymentMethodData variant mapping |
| `references/grpc-testing-guide.md` | grpcurl templates, test validation, testing subagent prompt |
| `references/macro-reference.md` | Connector macro system reference |
| `references/type-system.md` | RouterDataV2, type system reference |
| `references/payment-method-patterns/card.md` | Card payment patterns |
| `references/payment-method-patterns/wallet.md` | Wallet (Apple Pay, Google Pay) patterns |
| `references/payment-method-patterns/bank-transfer.md` | Bank transfer patterns |
| `references/payment-method-patterns/bank-debit.md` | Bank debit (ACH, SEPA DD) patterns |
| `references/payment-method-patterns/bank-redirect.md` | Bank redirect (iDEAL, Sofort) patterns |
| `references/payment-method-patterns/upi.md` | UPI Collect/Intent patterns |
| `references/payment-method-patterns/bnpl.md` | BNPL (Klarna, Afterpay) patterns |
| `references/payment-method-patterns/crypto.md` | Cryptocurrency patterns |
| `references/payment-method-patterns/gift-card.md` | Gift card patterns |
| `references/payment-method-patterns/mobile-payment.md` | Mobile/carrier billing patterns |
| `references/payment-method-patterns/reward.md` | Reward/loyalty points patterns |
