# UCS Connector Implementation Patterns

This directory contains comprehensive implementation patterns for each payment flow in the UCS (Universal Connector Service) system. Each pattern file provides complete, reusable templates that can be consumed by AI to generate consistent, production-ready connector code.

## 🆕 New Structure (2025)

Patterns are now organized hierarchically for better discoverability and modular workflows:

```
guides/patterns/
├── README.md                    # This file
├── flow_macro_guide.md          # Shared macro patterns
├── macro_patterns_reference.md  # Complete macro reference
└── flows/                       # Flow-specific patterns
    ├── README.md                # Flow patterns index
    ├── authorize/               # Authorization flow with payment methods
    │   ├── pattern_authorize.md # Core authorize pattern
    │   ├── card.md              # Card payments
    │   ├── wallet.md            # Digital wallets (Apple Pay, Google Pay)
    │   ├── bank_transfer.md     # Bank transfers
    │   ├── bank_debit.md        # Bank debits
    │   ├── bank_redirect.md     # Bank redirects (iDEAL, etc.)
    │   ├── upi.md               # UPI payments
    │   ├── bnpl.md              # Buy Now Pay Later
    │   ├── crypto.md            # Cryptocurrency
    │   ├── gift_card.md         # Gift cards
    │   ├── mobile_payment.md    # Mobile payments
    │   └── reward.md            # Reward points
    ├── capture/                 # Capture flow
    ├── refund/                  # Refund flow
    ├── void/                    # Void flow
    ├── psync/                   # Payment sync
    ├── rsync/                   # Refund sync
    ├── setup_mandate/           # Mandate setup
    ├── repeat_payment/          # Repeat payments
    ├── IncomingWebhook/         # Webhook handling
    └── ... (other advanced flows)
```

### Legacy Patterns (Being Migrated)

The flat pattern files in this directory are being migrated to the new `flows/` structure. During migration, both locations are valid, but new implementations should use the `flows/` directory.

## 📚 Available Patterns

### Core Payment Flows

| Flow | Pattern File | Status | Description |
|------|--------------|--------|-------------|
| **Authorize** | [`flows/authorize/pattern_authorize.md`](./flows/authorize/pattern_authorize.md) | ✅ Complete | Complete authorization flow patterns |
| **Capture** | [`flows/capture/pattern_capture.md`](./flows/capture/pattern_capture.md) | ✅ Complete | Payment capture flow patterns |
| **PSync** | [`flows/psync/pattern_psync.md`](./flows/psync/pattern_psync.md) | ✅ Complete | Payment status synchronization |
| **Void** | [`flows/void/pattern_void.md`](./flows/void/pattern_void.md) | ✅ Complete | Void/cancel authorization |
| **Refund** | [`flows/refund/pattern_refund.md`](./flows/refund/pattern_refund.md) | ✅ Complete | Full and partial refunds |
| **RSync** | [`flows/rsync/pattern_rsync.md`](./flows/rsync/pattern_rsync.md) | ✅ Complete | Refund status synchronization |

### Advanced Flows

| Flow | Pattern File | Status | Description |
|------|--------------|--------|-------------|
| **IncomingWebhook** | [`pattern_IncomingWebhook_flow.md`](./pattern_IncomingWebhook_flow.md) | ✅ Complete | Webhook handling and signature verification |
| **VerifyWebhookSource** | [`pattern_verify_webhook_source.md`](./pattern_verify_webhook_source.md) | ✅ Complete | Verify webhook signatures / source authenticity |
| **SetupMandate** | [`pattern_setup_mandate.md`](./pattern_setup_mandate.md) | ✅ Complete | Recurring payment setup |
| **RepeatPayment** | [`pattern_repeat_payment_flow.md`](./pattern_repeat_payment_flow.md) | ✅ Complete | Process recurring payments |
| **MandateRevoke** | [`pattern_mandate_revoke.md`](./pattern_mandate_revoke.md) | ✅ Complete | Cancel stored mandates |
| **PaymentMethodToken** | [`pattern_payment_method_token.md`](./pattern_payment_method_token.md) | ✅ Complete | Payment method tokenization |
| **CreateOrder** | [`pattern_createorder.md`](./pattern_createorder.md) | ✅ Complete | Multi-step payment initiation |
| **SessionToken** | [`pattern_session_token.md`](./pattern_session_token.md) | ✅ Complete | Secure session management |
| **CreateAccessToken** | [`pattern_CreateAccessToken_flow.md`](./pattern_CreateAccessToken_flow.md) | ✅ Complete | OAuth / access-token acquisition. Canonical source for the `ServerSessionAuthenticationToken`, `ServerAuthenticationToken`, and `ClientAuthenticationToken` flow markers (see the "Mapping to connector_flow.rs token markers" section). |
| **ClientAuthenticationToken** | [`pattern_client_authentication_token.md`](./pattern_client_authentication_token.md) | ✅ Complete | Client-side auth-token flow marker companion pattern |
| **CreateConnectorCustomer** | [`pattern_create_connector_customer.md`](./pattern_create_connector_customer.md) | ✅ Complete | Create customer on connector side before payment |
| **IncrementalAuthorization** | [`pattern_IncrementalAuthorization_flow.md`](./pattern_IncrementalAuthorization_flow.md) | ✅ Complete | Incremental authorization on existing auth |
| **VoidPC** | [`pattern_void_pc.md`](./pattern_void_pc.md) | ✅ Complete | Void pre-capture / pre-confirm |
| **DefendDispute** | [`pattern_defend_dispute.md`](./pattern_defend_dispute.md) | ✅ Complete | Defend against disputes |
| **AcceptDispute** | [`pattern_accept_dispute.md`](./pattern_accept_dispute.md) | ✅ Complete | Accept chargeback |
| **SubmitEvidence** | [`pattern_submit_evidence.md`](./pattern_submit_evidence.md) | ✅ Complete | Submit dispute evidence |
| **DSync** | [`pattern_dsync.md`](./pattern_dsync.md) | ✅ Complete | Dispute status sync |

### Authentication Flows (3DS / EMV3DS)

| Flow | Pattern File | Status | Description |
|------|--------------|--------|-------------|
| **PreAuthenticate** | [`pattern_preauthenticate.md`](./pattern_preauthenticate.md) | ✅ Complete | 3DS pre-authentication / version lookup |
| **Authenticate** | [`pattern_authenticate.md`](./pattern_authenticate.md) | ✅ Complete | 3DS authentication / challenge |
| **PostAuthenticate** | [`pattern_postauthenticate.md`](./pattern_postauthenticate.md) | ✅ Complete | 3DS post-authentication result retrieval |

### Payout Flows

| Flow | Pattern File | Status | Description |
|------|--------------|--------|-------------|
| **PayoutCreate** | [`pattern_payout_create.md`](./pattern_payout_create.md) | ✅ Complete | Create a payout |
| **PayoutTransfer** | [`pattern_payout_transfer.md`](./pattern_payout_transfer.md) | ✅ Complete | Transfer / execute a payout |
| **PayoutGet** | [`pattern_payout_get.md`](./pattern_payout_get.md) | ✅ Complete | Fetch / sync payout status |
| **PayoutVoid** | [`pattern_payout_void.md`](./pattern_payout_void.md) | ✅ Complete | Cancel a queued / pending payout |
| **PayoutStage** | [`pattern_payout_stage.md`](./pattern_payout_stage.md) | ✅ Complete | Stage payout prior to execution |
| **PayoutCreateLink** | [`pattern_payout_create_link.md`](./pattern_payout_create_link.md) | ✅ Complete | Generate payout link for recipient |
| **PayoutCreateRecipient** | [`pattern_payout_create_recipient.md`](./pattern_payout_create_recipient.md) | ✅ Complete | Create / register a payout recipient |
| **PayoutEnrollDisburseAccount** | [`pattern_payout_enroll_disburse_account.md`](./pattern_payout_enroll_disburse_account.md) | ✅ Complete | Enroll recipient disbursement account |

### Payment Method Patterns (Authorize Flow)

Every `PaymentMethodData` variant from
`crates/types-traits/domain_types/src/payment_method_data.rs` has a dedicated
pattern directory. The table below lists the canonical pattern per variant.

| Payment Method Variant | Pattern File | Supported Flows |
|------------------------|--------------|-----------------|
| **Card** | [`authorize/card/pattern_authorize_card.md`](./authorize/card/pattern_authorize_card.md) | All flows |
| **CardDetailsForNetworkTransactionId (NTID)** | [`authorize/card/pattern_authorize_card_ntid.md`](./authorize/card/pattern_authorize_card_ntid.md) | Authorize (MIT / recurring), RepeatPayment |
| **DecryptedWalletTokenDetailsForNetworkTransactionId (Wallet NTID)** | [`authorize/wallet/pattern_authorize_wallet_ntid.md`](./authorize/wallet/pattern_authorize_wallet_ntid.md) | Authorize (MIT / recurring), RepeatPayment |
| **CardRedirect** | [`authorize/card_redirect/pattern_authorize_card_redirect.md`](./authorize/card_redirect/pattern_authorize_card_redirect.md) | Authorize |
| **Wallet** | [`authorize/wallet/pattern_authorize_wallet.md`](./authorize/wallet/pattern_authorize_wallet.md) | Authorize, Refund |
| **PayLater (BNPL)** | [`authorize/bnpl/pattern_authorize_bnpl.md`](./authorize/bnpl/pattern_authorize_bnpl.md) | Authorize, Refund |
| **BankRedirect** | [`authorize/bank_redirect/pattern_authorize_bank_redirect.md`](./authorize/bank_redirect/pattern_authorize_bank_redirect.md) | Authorize |
| **BankDebit** | [`authorize/bank_debit/pattern_authorize_bank_debit.md`](./authorize/bank_debit/pattern_authorize_bank_debit.md) | Authorize, Refund |
| **BankTransfer** | [`authorize/bank_transfer/pattern_authorize_bank_transfer.md`](./authorize/bank_transfer/pattern_authorize_bank_transfer.md) | Authorize, Refund |
| **Crypto** | [`authorize/crypto/pattern_authorize_crypto.md`](./authorize/crypto/pattern_authorize_crypto.md) | Authorize |
| **MandatePayment** | [`authorize/mandate_payment/pattern_authorize_mandate_payment.md`](./authorize/mandate_payment/pattern_authorize_mandate_payment.md) | Authorize (MIT), RepeatPayment |
| **Reward** | [`authorize/reward/pattern_authorize_reward.md`](./authorize/reward/pattern_authorize_reward.md) | Authorize |
| **RealTimePayment** | [`authorize/real_time_payment/pattern_authorize_real_time_payment.md`](./authorize/real_time_payment/pattern_authorize_real_time_payment.md) | Authorize |
| **Upi** | [`authorize/upi/pattern_authorize_upi.md`](./authorize/upi/pattern_authorize_upi.md) | Authorize, Refund |
| **Voucher** | [`authorize/voucher/pattern_authorize_voucher.md`](./authorize/voucher/pattern_authorize_voucher.md) | Authorize |
| **GiftCard** | [`authorize/gift_card/pattern_authorize_gift_card.md`](./authorize/gift_card/pattern_authorize_gift_card.md) | Authorize |
| **CardToken** | [`authorize/card_token/pattern_authorize_card_token.md`](./authorize/card_token/pattern_authorize_card_token.md) | Authorize |
| **OpenBanking** | [`authorize/open_banking/pattern_authorize_open_banking.md`](./authorize/open_banking/pattern_authorize_open_banking.md) | Authorize |
| **NetworkToken** | [`authorize/network_token/pattern_authorize_network_token.md`](./authorize/network_token/pattern_authorize_network_token.md) | Authorize |
| **MobilePayment** | [`authorize/mobile_payment/pattern_authorize_mobile_payment.md`](./authorize/mobile_payment/pattern_authorize_mobile_payment.md) | Authorize, Refund |

## 🎯 Workflow Controllers

Grace now supports multiple workflow controllers for different use cases:

| Controller | Purpose | Trigger Pattern |
|------------|---------|-----------------|
| `.gracerules` | New connector integration | "integrate {connector}" |
| `.gracerules_add_flow` | Add specific flow(s) to existing connector | "add {flow} flow to {connector}" |
| `.gracerules_add_payment_method` | Add payment method(s) to existing connector | "add {Category}:{payment_method} to {connector}" |

### Payment Method Specification Syntax

The `.gracerules_add_payment_method` workflow **requires** category prefix syntax:

```bash
add {Category}:{type1},{type2} and {Category2}:{type3} to {connector}
```

**Examples:**
```bash
add Wallet:Apple Pay,Google Pay,PayPal to Stripe
add Card:Credit,Debit to Adyen
add BankTransfer:SEPA,ACH to Wise
add Wallet:Apple Pay,Google Pay and Card:Credit,Debit to Stripe
add Wallet:PayPal and BankTransfer:SEPA,ACH to Wise
add UPI:Collect,Intent to PhonePe
```

**Available Categories:** Card, Wallet, BankTransfer, BankDebit, BankRedirect, UPI, BNPL, Crypto, GiftCard, MobilePayment, Reward

## 🎯 Pattern Usage

### For New Implementations

Use `.gracerules` for complete new connector integration:

```bash
integrate {ConnectorName} using grace/rulesbook/codegen/.gracerules
```

This implements all core flows in sequence.

### For Adding Specific Flows

Use `.gracerules_flow` when adding flows to an existing connector:

```bash
add {flow_name} flow to {ConnectorName}
# Example: "add Refund flow to Stripe"
```

Available flows: Authorize, Capture, Refund, Void, PSync, RSync, SetupMandate, IncomingWebhook, etc.

### For Adding Payment Methods

Use `.gracerules_payment_method` when adding payment methods:

```bash
add {payment_method} to {ConnectorName}
# Example: "add Apple Pay to Stripe"
```

Available payment methods: Card, Wallet, BankTransfer, BankDebit, UPI, BNPL, Crypto, etc.

### AI Integration Commands

```bash
# New connector - complete integration
integrate {ConnectorName} using grace/rulesbook/codegen/.gracerules

# Add specific flow
add {flow_name} flow to {ConnectorName}

# Add payment method
add {payment_method} to {ConnectorName}

# Examples:
integrate Stripe using grace/rulesbook/codegen/.gracerules
add Refund flow to Stripe
add Apple Pay to Stripe
```

## 📖 Pattern Structure

Each pattern file follows a consistent structure:

### 1. **Quick Start Guide**
- Placeholder replacement guide
- Example implementations
- Time-to-completion estimates

### 2. **Prerequisites**
- Required flows that must be implemented first
- Dependencies and requirements
- What must exist before using this pattern

### 3. **Modern Macro-Based Pattern**
- Recommended implementation approach
- Complete code templates
- Type-safe implementations
- Integration with existing code

### 4. **Request/Response Patterns**
- Data structure examples
- Transformation patterns
- Payment method specific handling

### 5. **Error Handling**
- Error mapping strategies
- Specific error messages
- Common pitfalls

### 6. **Testing Patterns**
- Unit test templates
- Integration test patterns
- Validation checklists

### 7. **Integration Checklist**
- Pre-implementation requirements
- Step-by-step implementation guide
- Quality validation steps

## 🔄 Workflow Selection Guide

Choose the right workflow based on your needs:

| Scenario | Use This | Workflow File |
|----------|----------|---------------|
| New connector from scratch | Complete Integration | `.gracerules` |
| Add missing flow to existing connector | Flow Addition | `.gracerules_flow` |
| Add payment method to existing connector | Payment Method Addition | `.gracerules_payment_method` |
| Resume partial implementation | Depends on state | Use appropriate workflow |

## 💡 Contributing to Patterns

When implementing new connectors or flows:

1. **Document new patterns** discovered during implementation
2. **Update existing patterns** with improvements or edge cases
3. **Add real-world examples** to pattern files
4. **Enhance checklists** based on implementation experience

## 🎨 Pattern Quality Standards

All pattern files maintain:

- **🎯 Completeness**: Cover all aspects of flow implementation
- **📖 Clarity**: Clear explanations and examples
- **🔄 Reusability**: Templates work for any connector
- **✅ Validation**: Comprehensive testing and quality checks
- **🏗️ UCS-specific**: Tailored for UCS architecture and patterns
- **🚀 Production-ready**: Battle-tested in real implementations

## 🔗 Related Documentation

### Integration & Implementation
- [`../connector_integration_guide.md`](../connector_integration_guide.md) - Complete UCS integration process
- [`../types/types.md`](../types/types.md) - UCS type system reference
- [`../learnings/learnings.md`](../learnings/learnings.md) - Implementation lessons learned
- [`../../README.md`](../../README.md) - GRACE-UCS overview and usage

### Pattern Reference
- [`flows/README.md`](./flows/README.md) - Flow patterns index
- [`flow_macro_guide.md`](./flow_macro_guide.md) - Macro usage reference
- [`macro_patterns_reference.md`](./macro_patterns_reference.md) - Complete macro documentation

### Quality & Standards
- [`../feedback.md`](../feedback.md) - Quality feedback database and review template
- [`../quality/README.md`](../quality/README.md) - Quality system overview
- [`../quality/CONTRIBUTING_FEEDBACK.md`](../quality/CONTRIBUTING_FEEDBACK.md) - Guide for adding quality feedback

**🛡️ Quality Note**: All implementations using these patterns are reviewed by the Quality Guardian Subagent to ensure UCS compliance and code quality. Review common issues in `feedback.md` before implementing to avoid known anti-patterns.

---

**💡 Pro Tip**: Always choose the right workflow controller for your task. Use `.gracerules` for new connectors, `.gracerules_flow` for adding flows, and `.gracerules_payment_method` for adding payment methods.
