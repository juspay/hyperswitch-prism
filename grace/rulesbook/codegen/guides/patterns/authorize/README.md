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
├── card_token/
│   └── pattern_authorize_card_token.md     # Pre-tokenized card references (CardToken variant)
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
├── mandate_payment/
│   └── pattern_authorize_mandate_payment.md # Mandate-based MIT (MandatePayment variant)
├── format_specific/
│   └── (reserved for format-specific patterns: XML, Form-encoded, etc.)
└── generic/
    └── pattern_authorize.md           # Legacy generic authorize pattern (reference)
```

## 📋 Pattern Reference

| Directory | Pattern File | PaymentMethodData Variant | Payment Methods Covered | Example Connectors |
|-----------|-------------|---------------------------|------------------------|---------------------|
| `card/` | `pattern_authorize_card.md` | `Card` | Credit Card, Debit Card | Stripe, Adyen, Cybersource, Checkout, etc. |
| `card/` | `pattern_authorize_card_ntid.md` | `CardDetailsForNetworkTransactionId` | Card MIT (NTID-based recurring) | Stripe, Cybersource, Worldpay |
| `card_redirect/` | `pattern_authorize_card_redirect.md` | `CardRedirect` | CarteBancaire, Knet, Benefit (card-redirect) | Adyen, Checkout |
| `card_token/` | `pattern_authorize_card_token.md` | `CardToken` | Pre-tokenized card reference | Stripe (pm_...), Adyen (stored payment method) |
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
| `generic/` | `pattern_authorize.md` | _all_ | Legacy reference pattern | N/A |

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

# Card token (pre-tokenized card reference)
implement authorize flow for [ConnectorName] using authorize/card_token/pattern_authorize_card_token.md

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

- **Authentication**: API keys, OAuth, signatures (refer to connector-specific auth)
- **Idempotency**: Common pattern across all payment methods
- **Error Handling**: Standard error mapping to UCS error types
- **Currency Handling**: MinorUnit vs MajorUnit vs StringMinorUnit

## 📊 Payment Method Coverage

Based on `payment_methods.proto` categorization:

| Category | Proto IDs | Pattern Location |
|----------|-----------|------------------|
| Card Methods | 1-9 | `card/` (also `card_redirect/`, `card_token/`, `network_token/`) |
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
- Pass the Quality Guardian review

---

**Note**: The `generic/pattern_authorize.md` file is kept for backward compatibility and reference. New implementations should use the specific payment method patterns in their respective directories.
