# Authorize Flow Patterns

This directory contains comprehensive authorize flow patterns organized by payment method type. Each pattern provides complete, reusable templates for implementing authorization flows in UCS connectors.

## 📁 Directory Structure

```
authorize/
├── README.md                          # This file
├── card/
│   └── pattern_authorize_card.md      # Credit/Debit card payments
├── wallet/
│   └── pattern_authorize_wallet.md    # Digital wallets (PayPal, Apple Pay, etc.)
├── upi/
│   └── pattern_authorize_upi.md       # UPI payments (India)
├── bank_redirect/
│   └── pattern_authorize_bank_redirect.md  # Bank redirect flows (iDEAL, Sofort, etc.)
├── bank_transfer/
│   └── pattern_authorize_bank_transfer.md  # Bank transfer payments
├── bank_debit/
│   └── pattern_authorize_bank_debit.md     # ACH, SEPA, BACS direct debit
├── bnpl/
│   └── pattern_authorize_bnpl.md      # Buy Now Pay Later (Klarna, Afterpay, etc.)
├── gift_card/
│   └── pattern_authorize_gift_card.md # Gift card payments
├── crypto/
│   └── pattern_authorize_crypto.md    # Cryptocurrency payments
├── reward/
│   └── pattern_authorize_reward.md    # Reward/loyalty points
├── mobile_payment/
│   └── pattern_authorize_mobile_payment.md # Mobile carrier billing
├── format_specific/
│   └── (reserved for format-specific patterns: XML, Form-encoded, etc.)
└── generic/
    └── pattern_authorize.md           # Legacy generic authorize pattern (reference)
```

## 📋 Pattern Reference

| Directory | Pattern File | Payment Methods Covered | Connectors |
|-----------|-------------|------------------------|------------|
| `card/` | `pattern_authorize_card.md` | Credit Card, Debit Card | Stripe, Adyen, Cybersource, Checkout, etc. |
| `wallet/` | `pattern_authorize_wallet.md` | PayPal, Apple Pay, Google Pay, WeChat Pay, Alipay | PayPal, Stripe, Adyen, etc. |
| `upi/` | `pattern_authorize_upi.md` | UPI Collect, UPI Intent | PhonePe, Razorpay, etc. |
| `bank_redirect/` | `pattern_authorize_bank_redirect.md` | iDEAL, Sofort, Giropay, EPS, Przelewy24 | Trustly, etc. |
| `bank_transfer/` | `pattern_authorize_bank_transfer.md` | Wire Transfer, ACH Transfer | Wise, etc. |
| `bank_debit/` | `pattern_authorize_bank_debit.md` | ACH Debit, SEPA Direct Debit, BACS Debit | Stripe, Adyen, etc. |
| `bnpl/` | `pattern_authorize_bnpl.md` | Klarna, Afterpay, Affirm | Klarna, etc. |
| `gift_card/` | `pattern_authorize_gift_card.md` | Gift cards | Various |
| `crypto/` | `pattern_authorize_crypto.md` | Cryptocurrency | Coinbase, etc. |
| `reward/` | `pattern_authorize_reward.md` | Loyalty points, rewards | Various |
| `mobile_payment/` | `pattern_authorize_mobile_payment.md` | Carrier billing, mobile wallets | Various |
| `generic/` | `pattern_authorize.md` | Legacy reference pattern | N/A |

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
| Card Methods | 1-9 | `card/` |
| Digital Wallets | 10-29 | `wallet/` |
| UPI | 30-39 | `upi/` |
| Online Banking | 40-59 | `bank_redirect/` |
| Mobile Payments | 60-69 | `mobile_payment/` |
| Cryptocurrency | 70-79 | `crypto/` |
| Rewards | 80-89 | `reward/` |
| Bank Transfer | 90-99 | `bank_transfer/` |
| Direct Debit | 100-109 | `bank_debit/` |
| BNPL | 110-119 | `bnpl/` |
| Gift Cards | 130-139 | `gift_card/` |

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
