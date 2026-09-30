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

`PaymentMethodData` (`crates/types-traits/domain_types/src/payment_method_data.rs`,
`pub enum PaymentMethodData<T: PaymentMethodDataTypes>`) has **21** variants at HEAD. Every one
of them has a pattern file:

| Category | PaymentMethodData Variant | Pattern File (`references/payment-method-patterns/`) |
|----------|--------------------------|-------------|
| Card | `PaymentMethodData::Card(card)` | `card.md` |
| Card (no CVC) | `PaymentMethodData::CardWithNoCvc(card)` | `card-no-cvc.md` |
| Card (stored NTID) | `PaymentMethodData::CardDetailsForNetworkTransactionId(c)` | `card-ntid.md` → |
| Wallet token (stored NTID) | `PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(d)` | `wallet-ntid.md` → |
| CardRedirect | `PaymentMethodData::CardRedirect(cr)` | `card-redirect.md` → |
| Wallet | `PaymentMethodData::Wallet(wallet)` | `wallet.md` |
| BNPL | `PaymentMethodData::PayLater(pl)` | `bnpl.md` |
| BankRedirect | `PaymentMethodData::BankRedirect(br)` | `bank-redirect.md` |
| BankDebit | `PaymentMethodData::BankDebit(bd)` | `bank-debit.md` |
| BankTransfer | `PaymentMethodData::BankTransfer(bt)` (Box) | `bank-transfer.md` |
| Crypto | `PaymentMethodData::Crypto(crypto)` | `crypto.md` |
| MandatePayment | `PaymentMethodData::MandatePayment` (unit) | `mandate-payment.md` → |
| Reward | `PaymentMethodData::Reward` (unit) | `reward.md` |
| RealTimePayment | `PaymentMethodData::RealTimePayment(rtp)` (Box) | `real-time-payment.md` → |
| UPI | `PaymentMethodData::Upi(upi)` | `upi.md` |
| Voucher | `PaymentMethodData::Voucher(v)` | `voucher.md` → |
| GiftCard | `PaymentMethodData::GiftCard(gc)` (Box) | `gift-card.md` |
| PaymentMethodToken | `PaymentMethodData::PaymentMethodToken(t)` | `payment-method-token.md` → |
| OpenBanking | `PaymentMethodData::OpenBanking(ob)` | `open-banking.md` → |
| NetworkToken | `PaymentMethodData::NetworkToken(nt)` | `network-token.md` → |
| MobilePayment | `PaymentMethodData::MobilePayment(mp)` | `mobile-payment.md` |

Rows marked **→** are symlinks into the wider rulesbook corpus at
`grace/rulesbook/codegen/guides/patterns/authorize/<category>/`. That corpus is the
authoritative, longer-form set; the unmarked files here are the condensed skill-local
versions. **When a condensed file does not answer your question, read the rulesbook file for
the same category** —
`grace/rulesbook/codegen/guides/patterns/authorize/<category>/pattern_authorize_<category>.md`,
with `<category>` in **snake_case** there (`bank-transfer.md` here → `authorize/bank_transfer/`).
All 11 condensed categories have a rulesbook counterpart. If a category has neither, there is
no pattern and you must derive it from `payment_method_data.rs` directly.

Full PM name → category mapping: `references/category-mapping.md`

## Critical Rules

- Never use catch-all `_` to silently drop payment methods -- always return
  `IntegrationError::NotImplemented`. The variant is a **tuple** variant with two elements:
  `NotImplemented(String, IntegrationErrorContext)` (`domain_types/src/errors.rs`). Most
  other `IntegrationError` variants are struct variants that also require a `context` field
- `IntegrationError` (request side) and `ConnectorError` (response side) are different enums.
  `ConnectorError` has exactly five variants -- `ResponseDeserializationFailed`,
  `ResponseHandlingFailed`, `UnexpectedResponseError`, `IntegrityCheckFailed`,
  `ConnectorErrorResponse` -- and `InvalidData` / `InvalidCard` / `InvalidRequestData` are
  not variants of either one. Read the real list before substituting
- Each payment method gets its own explicit match arm
- `BankTransferData` and `GiftCardData` are Box-wrapped -- use `.deref()`
- Wallet sub-variants (ApplePay, GooglePay, etc.) each need separate nested match arms
- Validate required fields with `missing_field_err`
- Use `get_unimplemented_payment_method_error_message("ConnectorName")` for error messages --
  it takes ONE argument (`domain_types/src/utils.rs:195`); the `IntegrationErrorContext` is
  the *second argument of `NotImplemented`*, not an argument of this helper
- When a PM's response shape forces you to build
  `PaymentsResponseData::TransactionResponse`, list all 11 fields -- an enum struct-variant
  has no `..Default::default()`, so every omission is E0063. `redirection_data` is
  `Option<Box<RedirectForm>>` and `mandate_reference` is `Option<Box<MandateReference>>`:
  the `Box` is not optional
- Status enums for a new PM need `#[serde(other)] Unknown` at the DESERIALIZATION layer and
  an exhaustive, catch-all-free `match` at the STATUS-MAPPING layer. Both halves
- Amount unit comes from the vendor spec, never a default. The five types in
  `crates/common/common_utils/src/types.rs` are `MinorUnit`, `StringMinorUnit`,
  `StringMajorUnit`, `FloatMajorUnit`, `StringTwoDecimalUnit`

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
- Unsupported variants return `IntegrationError::NotImplemented(msg, context)` with the
  connector name in the message
- Required fields validated with `missing_field_err`
- Box-wrapped types properly `.deref()`'d
- New wire status enums carry `#[serde(other)] Unknown`; the status-mapping match has no
  catch-all `_ =>`
- Any `TransactionResponse` / `RefundsResponseData` literal lists every field
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
        _ => Err(errors::IntegrationError::NotImplemented(
            utils::get_unimplemented_payment_method_error_message("ConnectorName"),
            Default::default(),
        ).into()),
    },

    // ---- Bank Transfer (Box-wrapped, must .deref()) ----
    PaymentMethodData::BankTransfer(bt) => match bt.deref() {
        BankTransferData::SepaBankTransfer { .. } => { ... },
        _ => Err(errors::IntegrationError::NotImplemented(..., Default::default()).into()),
    },

    // ---- Bank Debit ----
    PaymentMethodData::BankDebit(bd) => match bd {
        BankDebitData::SepaBankDebit { iban, .. } => { ... },
        _ => Err(errors::IntegrationError::NotImplemented(..., Default::default()).into()),
    },

    // ---- UPI ----
    PaymentMethodData::Upi(upi) => match upi {
        UpiData::UpiCollect(c) => {
            let vpa = c.vpa_id.clone().ok_or_else(missing_field_err("vpa_id"))?;
            Ok(ConnectorPaymentsRequest { payment_type: "upi_collect", vpa, ... })
        },
        _ => Err(errors::IntegrationError::NotImplemented(..., Default::default()).into()),
    },

    // ---- BNPL ----
    PaymentMethodData::PayLater(pl) => match pl {
        PayLaterData::KlarnaRedirect { .. } => { ... },
        _ => Err(errors::IntegrationError::NotImplemented(..., Default::default()).into()),
    },

    // ---- Catch-all: explicit rejection ----
    _ => Err(errors::IntegrationError::NotImplemented(
        utils::get_unimplemented_payment_method_error_message("ConnectorName"),
        Default::default(),
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
| `references/payment-method-patterns/card-no-cvc.md` | `CardWithNoCvc` — cards with no CVC field |
| `references/payment-method-patterns/card-ntid.md` | Stored card + network transaction ID (symlink) |
| `references/payment-method-patterns/card-redirect.md` | Knet / Benefit / MomoAtm card redirects (symlink) |
| `references/payment-method-patterns/wallet.md` | Wallet (Apple Pay, Google Pay) patterns |
| `references/payment-method-patterns/wallet-ntid.md` | Decrypted wallet token + network transaction ID (symlink) |
| `references/payment-method-patterns/bank-transfer.md` | Bank transfer patterns |
| `references/payment-method-patterns/bank-debit.md` | Bank debit (ACH, SEPA DD) patterns |
| `references/payment-method-patterns/bank-redirect.md` | Bank redirect (iDEAL, Sofort) patterns |
| `references/payment-method-patterns/upi.md` | UPI Collect/Intent patterns |
| `references/payment-method-patterns/bnpl.md` | BNPL (Klarna, Afterpay) patterns |
| `references/payment-method-patterns/crypto.md` | Cryptocurrency patterns |
| `references/payment-method-patterns/gift-card.md` | Gift card patterns |
| `references/payment-method-patterns/voucher.md` | Boleto / OXXO / konbini vouchers (symlink) |
| `references/payment-method-patterns/mobile-payment.md` | Mobile/carrier billing patterns |
| `references/payment-method-patterns/real-time-payment.md` | DuitNow / FPS / PromptPay / VietQr (symlink) |
| `references/payment-method-patterns/open-banking.md` | OpenBanking PIS (symlink) |
| `references/payment-method-patterns/network-token.md` | Network token patterns (symlink) |
| `references/payment-method-patterns/payment-method-token.md` | `PaymentMethodToken` struct patterns (symlink) |
| `references/payment-method-patterns/mandate-payment.md` | `MandatePayment` unit variant (symlink) |
| `references/payment-method-patterns/reward.md` | Reward/loyalty points patterns |
