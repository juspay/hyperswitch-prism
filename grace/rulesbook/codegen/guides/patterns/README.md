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
| **IncomingWebhook** | [`flows/IncomingWebhook/`](./flows/IncomingWebhook/) | ✅ Complete | Webhook handling and signature verification |
| **SetupMandate** | [`flows/setup_mandate/`](./flows/setup_mandate/) | ✅ Complete | Recurring payment setup |
| **RepeatPayment** | [`flows/repeat_payment/`](./flows/repeat_payment/) | ✅ Complete | Process recurring payments |
| **MandateRevoke** | [`flows/mandate_revoke/`](./flows/mandate_revoke/) | ✅ Complete | Cancel stored mandates |
| **PaymentMethodToken** | [`flows/payment_method_token/`](./flows/payment_method_token/) | ✅ Complete | Payment method tokenization |
| **CreateOrder** | [`flows/createorder/`](./flows/createorder/) | ✅ Complete | Multi-step payment initiation |
| **SessionToken** | [`flows/session_token/`](./flows/session_token/) | ✅ Complete | Secure session management |
| **DefendDispute** | [`flows/defend_dispute/`](./flows/defend_dispute/) | ✅ Complete | Defend against disputes |
| **AcceptDispute** | [`flows/accept_dispute/`](./flows/accept_dispute/) | ✅ Complete | Accept chargeback |
| **DSync** | [`flows/dsync/`](./flows/dsync/) | ✅ Complete | Dispute status sync |

### Payment Method Patterns (Authorize Flow)

| Payment Method | Pattern File | Supported Flows |
|----------------|--------------|-----------------|
| **Card** | [`flows/authorize/card.md`](./flows/authorize/card.md) | All flows |
| **Wallet** | [`flows/authorize/wallet.md`](./flows/authorize/wallet.md) | Authorize, Refund |
| **Bank Transfer** | [`flows/authorize/bank_transfer.md`](./flows/authorize/bank_transfer.md) | Authorize, Refund |
| **Bank Debit** | [`flows/authorize/bank_debit.md`](./flows/authorize/bank_debit.md) | Authorize, Refund |
| **Bank Redirect** | [`flows/authorize/bank_redirect.md`](./flows/authorize/bank_redirect.md) | Authorize |
| **UPI** | [`flows/authorize/upi.md`](./flows/authorize/upi.md) | Authorize, Refund |
| **BNPL** | [`flows/authorize/bnpl.md`](./flows/authorize/bnpl.md) | Authorize, Refund |
| **Crypto** | [`flows/authorize/crypto.md`](./flows/authorize/crypto.md) | Authorize |
| **Gift Card** | [`flows/authorize/gift_card.md`](./flows/authorize/gift_card.md) | Authorize |
| **Mobile Payment** | [`flows/authorize/mobile_payment.md`](./flows/authorize/mobile_payment.md) | Authorize, Refund |
| **Reward** | [`flows/authorize/reward.md`](./flows/authorize/reward.md) | Authorize |

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
