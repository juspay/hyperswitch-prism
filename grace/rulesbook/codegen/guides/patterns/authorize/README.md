# Authorize Flow Patterns

This directory contains comprehensive authorize flow patterns organized by payment method type. Each pattern provides complete, reusable templates for implementing authorization flows in UCS connectors.

## 📁 Directory Structure

```
authorize/
├── README.md                          # This file
├── card/
│   ├── pattern_authorize_card.md           # Credit/Debit card payments (Card variant)
│   └── pattern_authorize_card_ntid.md      # Card MIT / NTID (CardDetailsForNetworkTransactionId)
├── card_redirect/
│   └── pattern_authorize_card_redirect.md  # Card redirect flows (CardRedirect variant)
├── payment_method_token/
│   └── pattern_authorize_payment_method_token.md # Pre-tokenized PM references (PaymentMethodToken variant)
├── wallet/
│   ├── pattern_authorize_wallet.md         # Digital wallets (Wallet variant)
│   └── pattern_authorize_wallet_ntid.md    # Wallet NTID / decrypted-token MIT
├── upi/
│   └── pattern_authorize_upi.md       # UPI payments (Upi variant)
├── bank_redirect/
│   └── pattern_authorize_bank_redirect.md  # Bank redirect flows (BankRedirect variant)
├── bank_transfer/
│   └── pattern_authorize_bank_transfer.md  # Bank transfer payments (BankTransfer variant)
├── bank_debit/
│   └── pattern_authorize_bank_debit.md     # ACH, SEPA, BACS direct debit (BankDebit variant)
├── bnpl/
│   └── pattern_authorize_bnpl.md      # Buy Now Pay Later (PayLater variant)
├── gift_card/
│   └── pattern_authorize_gift_card.md # Gift cards (GiftCard variant)
├── crypto/
│   └── pattern_authorize_crypto.md    # Cryptocurrency (Crypto variant)
├── reward/
│   └── pattern_authorize_reward.md    # Reward/loyalty points (Reward variant)
├── mobile_payment/
│   └── pattern_authorize_mobile_payment.md # Mobile carrier billing (MobilePayment variant)
├── voucher/
│   └── pattern_authorize_voucher.md        # Voucher / cash-voucher payments (Voucher variant)
├── real_time_payment/
│   └── pattern_authorize_real_time_payment.md  # Real-time / instant payments (RealTimePayment variant)
├── open_banking/
│   └── pattern_authorize_open_banking.md   # Open Banking PIS (OpenBanking variant)
├── network_token/
│   └── pattern_authorize_network_token.md  # Network-tokenized card (NetworkToken variant)
└── mandate_payment/
    └── pattern_authorize_mandate_payment.md # Mandate-based MIT (MandatePayment variant)
```

The legacy generic authorize pattern is not inside this directory — it lives one
level up at [`../pattern_authorize.md`](../pattern_authorize.md).

## 📋 Pattern Reference

| Directory | Pattern File | PaymentMethodData Variant | Payment Methods Covered | Example Connectors |
|-----------|-------------|---------------------------|------------------------|---------------------|
| `card/` | `pattern_authorize_card.md` | `Card` | Credit Card, Debit Card | Stripe, Adyen, Cybersource, Checkout, etc. |
| `card/` | `pattern_authorize_card_ntid.md` | `CardDetailsForNetworkTransactionId` | Card MIT (NTID-based recurring) | Stripe, Cybersource, Worldpay |
| `card_redirect/` | `pattern_authorize_card_redirect.md` | `CardRedirect` | CarteBancaire, Knet, Benefit (card-redirect) | Adyen, Checkout |
| `payment_method_token/` | `pattern_authorize_payment_method_token.md` | `PaymentMethodToken` | Pre-tokenized payment method reference | Shift4, Globalpay, HiPay, JPMorgan |
| `wallet/` | `pattern_authorize_wallet.md` | `Wallet` | PayPal, Apple Pay, Google Pay, WeChat Pay, Alipay | PayPal, Stripe, Adyen, etc. |
| `wallet/` | `pattern_authorize_wallet_ntid.md` | `DecryptedWalletTokenDetailsForNetworkTransactionId` | Wallet MIT using decrypted network token | Stripe, Adyen |
| `upi/` | `pattern_authorize_upi.md` | `Upi` | UPI Collect, UPI Intent, UPI QR | PhonePe, Razorpay, etc. |
| `bank_redirect/` | `pattern_authorize_bank_redirect.md` | `BankRedirect` | iDEAL, Sofort, Giropay, EPS, Przelewy24 | Trustly, etc. |
| `bank_transfer/` | `pattern_authorize_bank_transfer.md` | `BankTransfer` | Wire Transfer, ACH Transfer, SEPA Credit | Wise, etc. |
| `bank_debit/` | `pattern_authorize_bank_debit.md` | `BankDebit` | ACH Debit, SEPA Direct Debit, BACS Debit | Stripe, Adyen, etc. |
| `bnpl/` | `pattern_authorize_bnpl.md` | `PayLater` | Klarna, Afterpay, Affirm | Klarna, etc. |
| `gift_card/` | `pattern_authorize_gift_card.md` | `GiftCard` | Gift cards | Various |
| `crypto/` | `pattern_authorize_crypto.md` | `Crypto` | Cryptocurrency | Coinbase, etc. |
| `reward/` | `pattern_authorize_reward.md` | `Reward` | Loyalty points, rewards | Various |
| `mobile_payment/` | `pattern_authorize_mobile_payment.md` | `MobilePayment` | Carrier billing, mobile wallets | Various |
| `voucher/` | `pattern_authorize_voucher.md` | `Voucher` | Boleto, OXXO, PayCash, Efecty | Adyen, dLocal |
| `real_time_payment/` | `pattern_authorize_real_time_payment.md` | `RealTimePayment` | Pix, PromptPay, DuitNow, FedNow | Adyen, dLocal |
| `open_banking/` | `pattern_authorize_open_banking.md` | `OpenBanking` | OpenBanking PIS (TrueLayer, Plaid OBIE) | TrueLayer, Trustly |
| `network_token/` | `pattern_authorize_network_token.md` | `NetworkToken` | Network-tokenized card (VTS, MDES) | Stripe, Adyen |
| `mandate_payment/` | `pattern_authorize_mandate_payment.md` | `MandatePayment` | Mandate / CIT-based recurring | Stripe, Adyen, GoCardless |
| `../` (parent dir) | `pattern_authorize.md` | _all_ | Legacy reference pattern | N/A |

## 🎯 Usage Guide

### For New Implementations

1. **Identify Payment Method**: Determine which payment method category your connector supports
2. **Navigate to Pattern**: Open the appropriate directory for your payment method
3. **Follow Pattern**: Use the pattern file as a template for your implementation
4. **Check Examples**: Each pattern includes real-world examples from existing connectors

### Pattern Commands

```bash
# Card payments
implement authorize flow for [ConnectorName] using authorize/card/pattern_authorize_card.md

# Wallet payments
implement authorize flow for [ConnectorName] using authorize/wallet/pattern_authorize_wallet.md

# UPI payments
implement authorize flow for [ConnectorName] using authorize/upi/pattern_authorize_upi.md

# Bank redirect
implement authorize flow for [ConnectorName] using authorize/bank_redirect/pattern_authorize_bank_redirect.md

# Bank transfer
implement authorize flow for [ConnectorName] using authorize/bank_transfer/pattern_authorize_bank_transfer.md

# Bank debit
implement authorize flow for [ConnectorName] using authorize/bank_debit/pattern_authorize_bank_debit.md

# BNPL
implement authorize flow for [ConnectorName] using authorize/bnpl/pattern_authorize_bnpl.md

# Gift card
implement authorize flow for [ConnectorName] using authorize/gift_card/pattern_authorize_gift_card.md

# Crypto
implement authorize flow for [ConnectorName] using authorize/crypto/pattern_authorize_crypto.md

# Reward/loyalty
implement authorize flow for [ConnectorName] using authorize/reward/pattern_authorize_reward.md

# Mobile payment
implement authorize flow for [ConnectorName] using authorize/mobile_payment/pattern_authorize_mobile_payment.md

# Voucher (Boleto, OXXO, PayCash)
implement authorize flow for [ConnectorName] using authorize/voucher/pattern_authorize_voucher.md

# Real-time payment (Pix, PromptPay, FedNow)
implement authorize flow for [ConnectorName] using authorize/real_time_payment/pattern_authorize_real_time_payment.md

# Card redirect
implement authorize flow for [ConnectorName] using authorize/card_redirect/pattern_authorize_card_redirect.md

# Payment method token (pre-tokenized payment method reference)
implement authorize flow for [ConnectorName] using authorize/payment_method_token/pattern_authorize_payment_method_token.md

# Open Banking (PIS)
implement authorize flow for [ConnectorName] using authorize/open_banking/pattern_authorize_open_banking.md

# Network token (VTS / MDES)
implement authorize flow for [ConnectorName] using authorize/network_token/pattern_authorize_network_token.md

# Mandate payment (MIT/CIT)
implement authorize flow for [ConnectorName] using authorize/mandate_payment/pattern_authorize_mandate_payment.md

# Card MIT via NTID
implement authorize flow for [ConnectorName] using authorize/card/pattern_authorize_card_ntid.md

# Wallet MIT via decrypted wallet token
implement authorize flow for [ConnectorName] using authorize/wallet/pattern_authorize_wallet_ntid.md
```

## 🔄 Cross-Cutting Concerns

Some patterns may share common elements:

- **Authentication**: API keys, OAuth, signatures. Auth is read from
  `req.connector_config: ConnectorSpecificConfig` (a per-connector enum variant in
  `crates/types-traits/domain_types/src/router_data.rs`). `RouterDataV2::connector_auth_type`
  no longer exists, and `ConnectorCommon::get_auth_header` takes
  `&ConnectorSpecificConfig` (`crates/types-traits/interfaces/src/api.rs:25`).
- **Idempotency**: Common pattern across all payment methods
- **Error Handling**: Two distinct enums in `crates/types-traits/domain_types/src/errors.rs`.
  `IntegrationError` covers the **request** side (every variant carries a
  `context: IntegrationErrorContext`); `ConnectorError` covers the **response** side and has
  exactly five variants — `ResponseDeserializationFailed`, `ResponseHandlingFailed`,
  `UnexpectedResponseError`, `IntegrityCheckFailed` (all struct variants requiring `context`)
  and `ConnectorErrorResponse(Box<ErrorResponse>)`. There is no `ConnectorError::InvalidData`,
  `::NotImplemented` or `::InvalidCard`.
- **Currency Handling**: there are **five** amount types in
  `crates/common/common_utils/src/types.rs` — `MinorUnit` (`:170`), `StringMinorUnit`
  (`:305`), `FloatMajorUnit` (`:336`), `StringMajorUnit` (`:374`) and `StringTwoDecimalUnit`
  (`:443`). There is no safe default. Two independent counts over
  `crates/integrations/connector-integration/src/connectors/` at HEAD agree that
  `StringMinorUnit` is the *least* common of the major formats:

  | Measure | StringMajorUnit | FloatMajorUnit | MinorUnit | StringMinorUnit | StringTwoDecimalUnit |
  |---|---|---|---|---|---|
  | `create_amount_converter_wrapper!` declarations (33) | 10 | 4 | 10 | 9 | 0 |
  | connectors mentioning the type at all | 33 | 32 | 68 | 14 | 1 |

  So "default to `StringMinorUnit` if unclear" is wrong the large majority of the time.
  **Read the vendor spec and match its wire format** — do not guess.
- **Status mapping**: model the connector's status as a typed enum with `#[serde(other)]
  Unknown` at the *deserialization* layer, then match it exhaustively with **no catch-all
  `_ =>` arm** at the *mapping* layer. Reviewers require both halves.
  Exemplar: `crates/integrations/connector-integration/src/connectors/flywire/transformers.rs:468`.
- **In-band 2xx failures**: when a 200 response carries a declined payment, return
  `Err(ErrorResponse { .. })` from the response transformer, branching on
  `utils::is_payment_failure` (`crates/types-traits/domain_types/src/utils.rs:231`).
  `ErrorResponse::attempt_status` is `Option<FlowStatus>`, not `Option<AttemptStatus>`.

## 📊 Payment Method Coverage

Based on `payment_methods.proto` categorization:

| Category | Proto IDs | Pattern Location |
|----------|-----------|------------------|
| Card Methods | 1-9 | `card/` (also `card_redirect/`, `payment_method_token/`, `network_token/`) |
| Digital Wallets | 10-29 | `wallet/` |
| UPI | 30-39 | `upi/` |
| Online Banking | 40-59 | `bank_redirect/`, `open_banking/` |
| Mobile Payments | 60-69 | `mobile_payment/` |
| Cryptocurrency | 70-79 | `crypto/` |
| Rewards | 80-89 | `reward/` |
| Bank Transfer | 90-99 | `bank_transfer/` |
| Direct Debit | 100-109 | `bank_debit/` |
| BNPL | 110-119 | `bnpl/` |
| Vouchers | 120-129 | `voucher/` |
| Gift Cards | 130-139 | `gift_card/` |
| Real-Time Payments | 140-149 | `real_time_payment/` |
| Mandate / MIT | n/a (flow-level) | `mandate_payment/` (plus `card/pattern_authorize_card_ntid.md`, `wallet/pattern_authorize_wallet_ntid.md`) |

### PaymentMethodData Variant Coverage (`payment_method_data.rs`)

Every one of the 20 `PaymentMethodData` variants now has a dedicated authorize
pattern directory. See the detailed variant-to-directory mapping in
[`../README.md`](../README.md#payment-method-patterns-authorize-flow).

## 🔗 Related Patterns

- **Capture**: `../pattern_capture.md`
- **Refund**: `../pattern_refund.md`
- **Void**: `../pattern_void.md`
- **Psync**: `../pattern_psync.md`
- **Setup Mandate**: `../pattern_setup_mandate.md`

## 💡 Best Practices

1. **Always use the specific pattern** for your payment method rather than the generic pattern
2. **Follow macro-based implementation** for consistency across connectors
3. **Test with real payloads** from the connector's sandbox environment
4. **Document any deviations** from the standard pattern in connector comments
5. **Update patterns** when you discover new edge cases or better approaches

## 🛡️ Quality Assurance

All authorize implementations should:
- Follow the pattern structure exactly
- Include proper error handling
- Handle currency units correctly
- Map all relevant fields from connector response to UCS types
- Use `NO_ERROR_CODE` / `NO_ERROR_MESSAGE`
  (`crates/common/common_utils/src/consts.rs:154`) as the fallback for a missing connector
  error code/message — never `.unwrap_or_default()`, which yields an empty string
- List every field of `PaymentsResponseData::TransactionResponse`
  (11 fields, `crates/types-traits/domain_types/src/connector_types.rs:2009`) — it is an
  *enum* struct-variant, so there is no `..Default::default()` shortcut and an omitted field
  is an E0063. `RefundsResponseData` (`:2759`) is a plain struct with 4 fields but derives
  only `Debug, Clone`, so it has no `Default` to fall back on either
- Write exactly **one** non-generic `SourceVerification` impl and one `BodyDecoding` impl per
  connector — these traits take no type parameters
  (`crates/types-traits/interfaces/src/verification.rs:20`); one impl per flow is an E0107
- Pass the Quality Guardian review

---

**Note**: The `../pattern_authorize.md` file (one level up, in `patterns/`) is kept for backward compatibility and reference. New implementations should use the specific payment method patterns in their respective directories.
