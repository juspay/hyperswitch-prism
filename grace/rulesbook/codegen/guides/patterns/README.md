# UCS Connector Implementation Patterns

This directory contains comprehensive implementation patterns for each payment flow in the UCS (Universal Connector Service) system. Each pattern file provides complete, reusable templates that can be consumed by AI to generate consistent, production-ready connector code.

## Structure

Pattern files live flat in this directory:

```
guides/patterns/
├── README.md                             # This file
├── flow_macro_guide.md                   # Shared macro patterns
├── macro_patterns_reference.md           # Complete macro reference
├── pattern_authorize.md                  # Authorization flow
├── pattern_capture.md                    # Capture flow
├── pattern_void.md                       # Void flow
├── pattern_psync.md                      # Payment sync
├── pattern_refund.md                     # Refund flow
├── pattern_rsync.md                      # Refund sync
├── pattern_createorder.md                # CreateOrder pre-auth
├── pattern_CreateAccessToken_flow.md     # CreateAccessToken pre-auth
├── pattern_session_token.md              # CreateSessionToken pre-auth
├── pattern_payment_method_token.md       # PaymentMethodToken pre-auth
├── pattern_setup_mandate.md              # SetupMandate
├── pattern_repeat_payment_flow.md        # RepeatPayment
├── pattern_mandate_revoke.md             # MandateRevoke
├── pattern_void_pc.md                    # VoidPC
├── pattern_IncrementalAuthorization_flow.md  # IncrementalAuthorization
├── pattern_IncomingWebhook_flow.md       # IncomingWebhook
├── pattern_accept_dispute.md             # AcceptDispute
├── pattern_submit_evidence.md            # SubmitEvidence
├── pattern_defend_dispute.md             # DefendDispute
├── pattern_dsync.md                      # DSync
└── authorize/                            # Payment-method-specific authorize patterns
    ├── card/pattern_authorize_card.md
    ├── wallet/pattern_authorize_wallet.md
    ├── bank_debit/pattern_authorize_bank_debit.md
    ├── bank_transfer/pattern_authorize_bank_transfer.md
    ├── bank_redirect/pattern_authorize_bank_redirect.md
    ├── upi/pattern_authorize_upi.md
    ├── bnpl/pattern_authorize_bnpl.md
    ├── crypto/pattern_authorize_crypto.md
    ├── gift_card/pattern_authorize_gift_card.md
    ├── mobile_payment/pattern_authorize_mobile_payment.md
    └── reward/pattern_authorize_reward.md
```

> **Note:** A `flows/` subdirectory layout was planned but not implemented.
> All pattern files currently live flat in this directory.

## 📚 Available Patterns

### Core Payment Flows

| Flow | Pattern File | Status | Description |
|------|--------------|--------|-------------|
| **Authorize** | [`./pattern_authorize.md`](./pattern_authorize.md) | ✅ Complete | Complete authorization flow patterns |
| **Capture** | [`./pattern_capture.md`](./pattern_capture.md) | ✅ Complete | Payment capture flow patterns |
| **PSync** | [`./pattern_psync.md`](./pattern_psync.md) | ✅ Complete | Payment status synchronization |
| **Void** | [`./pattern_void.md`](./pattern_void.md) | ✅ Complete | Void/cancel authorization |
| **Refund** | [`./pattern_refund.md`](./pattern_refund.md) | ✅ Complete | Full and partial refunds |
| **RSync** | [`./pattern_rsync.md`](./pattern_rsync.md) | ✅ Complete | Refund status synchronization |

### Advanced Flows

| Flow | Pattern File | Status | Description |
|------|--------------|--------|-------------|
| **IncomingWebhook** | [`./pattern_IncomingWebhook_flow.md`](./pattern_IncomingWebhook_flow.md) | ✅ Complete | Webhook handling and signature verification |
| **IncrementalAuthorization** | [`pattern_IncrementalAuthorization_flow.md`](./pattern_IncrementalAuthorization_flow.md) | ✅ Complete | Incremental/partial capture authorization |
| **SetupMandate** | [`./pattern_setup_mandate.md`](./pattern_setup_mandate.md) | ✅ Complete | Recurring payment setup |
| **RepeatPayment** | [`./pattern_repeat_payment_flow.md`](./pattern_repeat_payment_flow.md) | ✅ Complete | Process recurring payments |
| **MandateRevoke** | [`./pattern_mandate_revoke.md`](./pattern_mandate_revoke.md) | ✅ Complete | Cancel stored mandates |
| **PaymentMethodToken** | [`./pattern_payment_method_token.md`](./pattern_payment_method_token.md) | ✅ Complete | Payment method tokenization |
| **CreateOrder** | [`./pattern_createorder.md`](./pattern_createorder.md) | ✅ Complete | Multi-step payment initiation |
| **SessionToken** | [`./pattern_session_token.md`](./pattern_session_token.md) | ✅ Complete | Secure session management |
| **DefendDispute** | [`./pattern_defend_dispute.md`](./pattern_defend_dispute.md) | ✅ Complete | Defend against disputes |
| **AcceptDispute** | [`./pattern_accept_dispute.md`](./pattern_accept_dispute.md) | ✅ Complete | Accept chargeback |
| **SubmitEvidence** | [`pattern_submit_evidence.md`](./pattern_submit_evidence.md) | ✅ Complete | Dispute evidence submission |
| **DSync** | [`./pattern_dsync.md`](./pattern_dsync.md) | ✅ Complete | Dispute status sync |

### Payment Method Patterns (Authorize Flow)

| Payment Method | Pattern File | Supported Flows |
|----------------|--------------|-----------------|
| **Card** | [`./authorize/card/pattern_authorize_card.md`](./authorize/card/pattern_authorize_card.md) | All flows |
| **Wallet** | [`./authorize/wallet/pattern_authorize_wallet.md`](./authorize/wallet/pattern_authorize_wallet.md) | Authorize, Refund |
| **Bank Transfer** | [`./authorize/bank_transfer/pattern_authorize_bank_transfer.md`](./authorize/bank_transfer/pattern_authorize_bank_transfer.md) | Authorize, Refund |
| **Bank Debit** | [`./authorize/bank_debit/pattern_authorize_bank_debit.md`](./authorize/bank_debit/pattern_authorize_bank_debit.md) | Authorize, Refund |
| **Bank Redirect** | [`./authorize/bank_redirect/pattern_authorize_bank_redirect.md`](./authorize/bank_redirect/pattern_authorize_bank_redirect.md) | Authorize |
| **UPI** | [`./authorize/upi/pattern_authorize_upi.md`](./authorize/upi/pattern_authorize_upi.md) | Authorize, Refund |
| **BNPL** | [`./authorize/bnpl/pattern_authorize_bnpl.md`](./authorize/bnpl/pattern_authorize_bnpl.md) | Authorize, Refund |
| **Crypto** | [`./authorize/crypto/pattern_authorize_crypto.md`](./authorize/crypto/pattern_authorize_crypto.md) | Authorize |
| **Gift Card** | [`./authorize/gift_card/pattern_authorize_gift_card.md`](./authorize/gift_card/pattern_authorize_gift_card.md) | Authorize |
| **Mobile Payment** | [`./authorize/mobile_payment/pattern_authorize_mobile_payment.md`](./authorize/mobile_payment/pattern_authorize_mobile_payment.md) | Authorize, Refund |
| **Reward** | [`./authorize/reward/pattern_authorize_reward.md`](./authorize/reward/pattern_authorize_reward.md) | Authorize |

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
- [`flow_macro_guide.md`](./flow_macro_guide.md) - Flow patterns index
- [`macro_patterns_reference.md`](./macro_patterns_reference.md) - Complete macro reference

### Quality & Standards
- [`../feedback.md`](../feedback.md) - Quality feedback database and review template
- [`../quality/README.md`](../quality/README.md) - Quality system overview
- [`../quality/CONTRIBUTING_FEEDBACK.md`](../quality/CONTRIBUTING_FEEDBACK.md) - Guide for adding quality feedback

**🛡️ Quality Note**: All implementations using these patterns are reviewed by the Quality Guardian Subagent to ensure UCS compliance and code quality. Review common issues in `feedback.md` before implementing to avoid known anti-patterns.

---

**💡 Pro Tip**: Always choose the right workflow controller for your task. Use `.gracerules` for new connectors, `.gracerules_flow` for adding flows, and `.gracerules_payment_method` for adding payment methods.
