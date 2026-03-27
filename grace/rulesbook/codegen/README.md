# Global Rapid Agentic Connector Exchange for UCS (GRACE-UCS)

GRACE-UCS is a specialized AI-assisted system for UCS (Universal Connector Service) connector development that supports **complete connector lifecycle management** - from initial implementation to granular flow additions and payment method expansions.

## 🎯 Core Purpose

GRACE-UCS enables:
- **Full connector implementation** from scratch (all 6 core flows)
- **Granular flow addition** - Add just one flow to existing connectors
- **Payment method expansion** - Add specific payment methods to existing connectors
- **All payment method support** (cards, wallets, bank transfers, BNPL, etc.)
- **Complete flow coverage** (authorize, capture, void, refund, sync, webhooks, etc.)
- **UCS-specific patterns** tailored for gRPC-based stateless architecture

## 🆕 New: Modular Workflow Controllers (2025)

GRACE now supports **three specialized workflow controllers** for different use cases:

| Workflow | File | Purpose | When to Use |
|----------|------|---------|-------------|
| **Complete Integration** | `.gracerules` | New connector from scratch | Building new connector |
| **Flow Addition** | `.gracerules_add_flow` | Add specific flow(s) | Adding Refund, Webhook, etc. |
| **Payment Method Addition** | `.gracerules_add_payment_method` | Add payment method(s) | Adding Apple Pay, Cards, etc. |

### Quick Command Reference

**Explicit Form (Required)**
```bash
# 1. New connector from scratch
integrate [ConnectorName] using grace/rulesbook/codegen/.gracerules

# 2. Add specific flow(s) to existing connector
add [flow_name] flow to [ConnectorName] using grace/rulesbook/codegen/.gracerules_add_flow
add [flow1] and [flow2] flows to [ConnectorName] using grace/rulesbook/codegen/.gracerules_add_flow

# 3. Add payment method(s) to existing connector
# Category prefix syntax (required)
add [Category]:[pm1],[pm2] and [Category2]:[pm3] to [ConnectorName] using grace/rulesbook/codegen/.gracerules_add_payment_method
```

## 🏗️ UCS Architecture Overview

The UCS connector-service uses a modern, stateless architecture:

```
crates/
├── connector-integration/     # Connector-specific logic
│   ├── src/connectors/       # Individual connector implementations
│   │   ├── {connector}.rs    # Main connector file
│   │   └── {connector}/
│   │       └── transformers.rs  # Request/response transformations
│   └── src/types.rs          # Common types and utilities
├── domain-types/             # Domain models and data structures
├── grpc-server/             # gRPC service implementation
└── grpc-api-types/          # Protocol buffer definitions
```

### Key UCS Characteristics:
- **gRPC-first**: All communication via Protocol Buffers
- **Stateless**: No database dependencies in connector logic
- **RouterDataV2**: Enhanced type-safe data flow
- **ConnectorIntegrationV2**: Modern trait-based integration
- **Domain-driven**: Clear separation of concerns
- **Macro-based**: Uses `macro_connector_implementation!` for consistency

## 🚀 Usage Scenarios

### Scenario 1: New Connector Implementation

Use when building a connector from scratch.

```bash
integrate Stripe using grace/rulesbook/codegen/.gracerules
```

**What happens:**
1. Creates connector foundation (struct, auth, common traits)
2. Implements all 6 core flows: Authorize → PSync → Capture → Refund → RSync → Void
3. Runs quality review
4. Returns complete, production-ready connector

**Prerequisites:**
- Place tech spec in `grace/rulesbook/codegen/references/stripe/technical_specification.md`

---

### Scenario 2: Add Flow to Existing Connector

Use when a connector exists but is missing specific flows.

```bash
add Refund flow to Stripe using grace/rulesbook/codegen/.gracerules_add_flow
```

**What happens:**
1. Analyzes existing Stripe connector state
2. Checks prerequisites (Refund needs Capture)
3. Implements only the Refund flow
4. Integrates with existing code

**Other examples:**
```bash
add IncomingWebhook flow to Adyen using grace/rulesbook/codegen/.gracerules_add_flow
add SetupMandate and RepeatPayment flows to Stripe using grace/rulesbook/codegen/.gracerules_add_flow
add Void flow to PayPal using grace/rulesbook/codegen/.gracerules_add_flow
```

**Supported flows:** Authorize, Capture, Refund, Void, PSync, RSync, SetupMandate, RepeatPayment, IncomingWebhook, CreateOrder, SessionToken, PaymentMethodToken, DefendDispute, AcceptDispute, DSync, and more.

---

### Scenario 3: Add Payment Method to Existing Connector

Use when a connector supports some payment methods but not others.

**Category prefix syntax (required):**
```bash
add Wallet:Apple Pay,Google Pay,PayPal to Stripe using grace/rulesbook/codegen/.gracerules_add_payment_method
add Card:Credit,Debit to Adyen using grace/rulesbook/codegen/.gracerules_add_payment_method
add BankTransfer:SEPA,ACH to Wise using grace/rulesbook/codegen/.gracerules_add_payment_method
add Wallet:Apple Pay,Google Pay and Card:Credit,Debit to Stripe using grace/rulesbook/codegen/.gracerules_add_payment_method
add Wallet:PayPal and BankTransfer:SEPA,ACH to Wise using grace/rulesbook/codegen/.gracerules_add_payment_method
add UPI:Collect,Intent to PhonePe using grace/rulesbook/codegen/.gracerules_add_payment_method
```

**What happens:**
1. Analyzes existing connector state
2. Checks that Authorize flow exists (required)
3. Parses category and payment method types from command
4. Reads corresponding pattern file for each category
5. Adds payment method handling in transformers
6. Adds to applicable flows (Authorize, Refund, etc.)

**Supported Categories:** Card, Wallet, BankTransfer, BankDebit, BankRedirect, UPI, BNPL, Crypto, GiftCard, MobilePayment, Reward

---

### Scenario 4: Resume Partial Implementation

Use when you started a connector and need to continue.

```bash
# Option A: Add missing flows
add Refund and RSync flows to MyConnector using grace/rulesbook/codegen/.gracerules_add_flow

# Option B: Add payment methods
add Wallet:Apple Pay to MyConnector using grace/rulesbook/codegen/.gracerules_add_payment_method

# Option C: Continue with complete integration
integrate MyConnector using grace/rulesbook/codegen/.gracerules
```

---

### Scenario 5: Debug/Fix Issues

```bash
fix error handling in Stripe Refund flow
debug PayPal connector - getting timeout errors
```

## 📋 Comprehensive Flow Support

### Core Payment Flows (6 Essential Flows)

| Flow | Pattern File | Dependencies | Description |
|------|--------------|--------------|-------------|
| **Authorize** | `flows/authorize/pattern_authorize.md` | None | Initial payment authorization |
| **PSync** | `flows/psync/pattern_psync.md` | Authorize | Payment status synchronization |
| **Capture** | `flows/capture/pattern_capture.md` | Authorize | Capture authorized payments |
| **Void** | `flows/void/pattern_void.md` | Authorize | Cancel authorized payments |
| **Refund** | `flows/refund/pattern_refund.md` | Capture | Full and partial refunds |
| **RSync** | `flows/rsync/pattern_rsync.md` | Refund | Refund status synchronization |

### Advanced Flows

| Flow | Pattern File | Dependencies | Description |
|------|--------------|--------------|-------------|
| **SetupMandate** | `flows/setup_mandate/` | Authorize | Set up recurring payments |
| **RepeatPayment** | `flows/repeat_payment/` | SetupMandate | Process recurring payments |
| **IncomingWebhook** | `flows/IncomingWebhook/` | PSync | Real-time event handling |
| **CreateOrder** | `flows/createorder/` | - | Multi-step payment initiation |
| **SessionToken** | `flows/session_token/` | - | Secure session management |
| **PaymentMethodToken** | `flows/payment_method_token/` | - | Tokenize payment methods |
| **DefendDispute** | `flows/defend_dispute/` | - | Defend chargebacks |
| **AcceptDispute** | `flows/accept_dispute/` | - | Accept chargebacks |
| **DSync** | `flows/dsync/` | - | Dispute status synchronization |
| **MandateRevoke** | `flows/mandate_revoke/` | SetupMandate | Cancel stored mandates |
| **IncrementalAuthorization** | `flows/IncrementalAuthorization/` | Authorize | Incremental auth flow |
| **VoidPC** | `flows/void_pc/` | Capture | Void post-capture |

### Payment Method Patterns (for Authorize Flow)

| Payment Method | Pattern File | Supported Flows |
|----------------|--------------|-----------------|
| **Card** | `flows/authorize/card.md` | All flows |
| **Wallet** | `flows/authorize/wallet.md` | Authorize, Refund |
| **Bank Transfer** | `flows/authorize/bank_transfer.md` | Authorize, Refund |
| **Bank Debit** | `flows/authorize/bank_debit.md` | Authorize, Refund |
| **Bank Redirect** | `flows/authorize/bank_redirect.md` | Authorize |
| **UPI** | `flows/authorize/upi.md` | Authorize, Refund |
| **BNPL** | `flows/authorize/bnpl.md` | Authorize, Refund |
| **Crypto** | `flows/authorize/crypto.md` | Authorize |
| **Gift Card** | `flows/authorize/gift_card.md` | Authorize |
| **Mobile Payment** | `flows/authorize/mobile_payment.md` | Authorize, Refund |
| **Reward** | `flows/authorize/reward.md` | Authorize |

## 🔄 Workflow Selection Guide

Choose the right workflow based on your task:

```
What do you need to do?
│
├── New connector from scratch?
│   └── Use .gracerules
│       Command: integrate {Connector} using grace/rulesbook/codegen/.gracerules
│
├── Add to existing connector?
│   │
│   ├── Add a flow (Authorize, Capture, Refund, etc.)?
│   │   └── Use .gracerules_flow
│       Command: add {flow} flow to {Connector} using grace/rulesbook/codegen/.gracerules_flow
│   │
│   └── Add a payment method (Apple Pay, Cards, etc.)?
│       └── Use .gracerules_payment_method
│           Command: add {payment_method} to {Connector} using grace/rulesbook/codegen/.gracerules_payment_method
│
└── Fix or improve existing connector?
    └── Use appropriate workflow or manual editing
```

### Decision Matrix

| Scenario | Workflow | Prerequisites | Output |
|----------|----------|---------------|--------|
| New connector | `.gracerules` | Tech spec | Complete connector (6 flows) |
| Add Refund flow | `.gracerules_flow` | Authorize, Capture | Refund flow only |
| Add Apple Pay | `.gracerules_payment_method` | Authorize flow | PM support in Authorize (+Refund) |
| Add Webhook | `.gracerules_flow` | PSync | IncomingWebhook flow |
| Add Mandate | `.gracerules_flow` | Authorize | SetupMandate + RepeatPayment |

## 🛠️ Implementation States

GRACE-UCS tracks and can resume from any implementation state:

### State 1: **Initial Setup**
- Basic connector structure created
- Auth configuration defined
- Base trait implementations stubbed

### State 2: **Core Flows Implemented**
- Authorization flow working
- Basic error handling in place
- Request/response transformations for primary flow

### State 3: **Extended Flows**
- Capture, void, refund flows implemented
- Sync operations working
- Status mapping complete

### State 4: **Payment Methods**
- Multiple payment method support
- Proper validation and transformation
- Payment method specific handling

### State 5: **Advanced Features**
- Webhook implementation
- 3DS handling
- Mandate/recurring support
- Comprehensive error handling

### State 6: **Production Ready**
- Full test coverage
- All edge cases handled
- Performance optimized
- Documentation complete

## 📖 How to Use GRACE

### For New Implementation:

1. **Prepare Tech Spec**
   ```bash
   mkdir -p grace/rulesbook/codegen/references/{connector_name}/
   # Place technical_specification.md in this directory
   ```

2. **Run Integration Command**
   ```bash
   integrate {ConnectorName} using grace/rulesbook/codegen/.gracerules
   ```

3. **AI will execute:**
   - Phase 1: Tech spec validation
   - Phase 2: Foundation setup
   - Phase 3: Sequential flow implementation (6 core flows)
   - Phase 4: Quality review

### For Adding Flows:

1. **Identify missing flow(s)**
2. **Run flow addition command**
   ```bash
   add {flow_name} flow to {ConnectorName}
   ```
3. **AI will:**
   - Analyze existing connector
   - Check prerequisites
   - Implement only the requested flow(s)
   - Run quality review

### For Adding Payment Methods:

1. **Verify Authorize flow exists** (required)
2. **Run payment method command**
   ```bash
   add {payment_method} to {ConnectorName}
   ```
3. **AI will:**
   - Analyze current payment methods
   - Add PM handling in transformers
   - Update relevant flows
   - Run quality review

### For Debugging:

1. **Describe the issue**
   ```bash
   fix {ConnectorName} connector - {error_description}
   ```
2. **AI will analyze and fix**

## 🔧 UCS-Specific Patterns

### Pattern Organization

```
guides/patterns/
├── README.md                    # Pattern overview
├── flow_macro_guide.md          # Macro usage reference
├── macro_patterns_reference.md  # Complete macro documentation
└── flows/                       # Flow-specific patterns
    ├── README.md                # Flow patterns index
    ├── authorize/
    │   ├── pattern_authorize.md # Core authorize flow
    │   ├── card.md              # Card payments
    │   ├── wallet.md            # Digital wallets
    │   └── ...                  # Other payment methods
    ├── capture/
    ├── refund/
    ├── void/
    ├── psync/
    ├── rsync/
    └── ...                      # Advanced flows
```

### Pattern Usage

Each pattern file provides:
- **🎯 Quick Start Guide** with placeholder replacement examples
- **📋 Prerequisites** - What must exist before implementation
- **🏗️ Modern Macro-Based Templates** for consistent implementations
- **🔧 Legacy Manual Patterns** for special cases
- **🧪 Testing Strategies** and integration checklists
- **✅ Validation Steps** and quality checks

### Using Patterns with AI

```bash
# New connector - uses patterns automatically
integrate NewPayment using grace/rulesbook/codegen/.gracerules

# Add specific flow - uses flow pattern
add Refund flow to ExistingConnector

# Add payment method - uses PM pattern
add Apple Pay to ExistingConnector
```

### Connector Structure

```rust
// Main connector file: crates/integrations/connector-integration/src/connectors/{connector}.rs
pub mod transformers;

pub struct ConnectorName<T> {
    phantom: std::marker::PhantomData<T>,
}

// Macro-based flow implementation
macros::macro_connector_implementation!(
    connector: ConnectorName,
    flow_name: Authorize,
    // ... parameters
);

// Transformers: crates/integrations/connector-integration/src/connectors/{connector}/transformers.rs
// Request/response transformations for all payment methods and flows
```

### Data Flow

```
gRPC Request → RouterDataV2 → Connector Transform → HTTP Request → External API
External Response → Connector Transform → RouterDataV2 → gRPC Response
```

## 📁 Directory Structure

```
grace/rulesbook/codegen/
├── .gracerules                          # New connector integration
├── .gracerules_flow                     # Add specific flows
├── .gracerules_payment_method           # Add payment methods
├── README.md                            # This file
├── guides/
│   ├── workflow_selection.md            # How to choose workflow
│   ├── feedback.md                      # Quality feedback database
│   ├── quality/                         # Quality system docs
│   │   ├── README.md
│   │   ├── quality_review_template.md
│   │   └── CONTRIBUTING_FEEDBACK.md
│   ├── connector_integration_guide.md   # Step-by-step integration
│   ├── patterns/                        # Flow-specific patterns
│   │   ├── README.md                    # Pattern directory index
│   │   ├── flow_macro_guide.md          # Macro patterns
│   │   ├── macro_patterns_reference.md  # Macro reference
│   │   └── flows/                       # Flow patterns (new structure)
│   │       ├── README.md
│   │       ├── authorize/
│   │       │   ├── pattern_authorize.md
│   │       │   ├── card.md
│   │       │   ├── wallet.md
│   │       │   └── ...
│   │       ├── capture/
│   │       ├── refund/
│   │       ├── void/
│   │       └── ...
│   ├── learnings/learnings.md           # Implementation lessons
│   └── types/types.md                   # UCS type system
├── connector_integration/
│   └── template/
│       ├── tech_spec.md                 # Tech spec template
│       └── planner_steps.md             # Planning template
└── references/
    └── {connector_name}/                 # Connector-specific docs
        ├── technical_specification.md
        ├── api_docs.md
        └── webhook_spec.json
```

## 🎯 Key Benefits

1. **🎯 Granular Control**: Add just one flow or payment method, not everything
2. **🔄 Resumable Development**: Pick up exactly where you left off
3. **📦 Complete Coverage**: All payment methods and flows supported
4. **🏗️ UCS-Optimized**: Patterns specific to UCS architecture
5. **🤖 AI-Assisted**: Intelligent code generation and problem solving
6. **✅ Quality Assured**: Automated quality reviews ensure high standards
7. **🚀 Production-Ready**: Follows UCS best practices and patterns
8. **📈 Extensible**: Easy to add new flows and payment methods
9. **📚 Continuous Learning**: Feedback system captures lessons learned

## 🚀 Getting Started

### Quick Start Examples

```bash
# 1. New connector
integrate Stripe using grace/rulesbook/codegen/.gracerules

# 2. Add missing flow
add Refund flow to MyConnector

# 3. Add payment method
add Apple Pay to Stripe

# 4. Multiple flows
add SetupMandate and RepeatPayment flows to Stripe

# 5. Multiple payment methods
add Apple Pay and Google Pay to Adyen
```

### Step-by-Step: New Connector

1. Create tech spec:
   ```bash
   mkdir -p grace/rulesbook/codegen/references/mypayment/
   # Create technical_specification.md with:
   # - Connector name: MyPayment
   # - Base URL: https://api.mypayment.com
   # - Auth type: API Key / OAuth / etc.
   # - Supported flows
   # - Supported payment methods
   # - API endpoints
   ```

2. Run integration:
   ```bash
   integrate MyPayment using grace/rulesbook/codegen/.gracerules
   ```

3. Wait for completion:
   - AI creates foundation
   - AI implements 6 core flows
   - AI runs quality review
   - AI provides completion report

### Step-by-Step: Add Flow

1. Identify missing flow (e.g., Refund)

2. Check prerequisites:
   - Refund needs Capture
   - Verify Capture exists

3. Run command:
   ```bash
   add Refund flow to MyConnector
   ```

4. AI implements only Refund flow

### Step-by-Step: Add Payment Method

1. Verify Authorize flow exists

2. Run command:
   ```bash
   add Apple Pay to MyConnector
   ```

3. AI adds Apple Pay handling to transformers

---

## 🛡️ Quality Enforcement System

GRACE-UCS includes an automated **Quality Guardian Subagent** that ensures every implementation meets high quality standards.

### Quality Review Process

```
Foundation → Flow Implementation → All Flows Complete → Cargo Build ✅
                                                              ↓
                                                    Quality Guardian Review
                                                              ↓
                                        Quality Score ≥ 60? ──┬── Yes → ✅ Approved
                                                              │
                                                              └── No → ❌ Blocked (Fix Required)
```

### When Quality Review Runs

- **New Connector**: After all 6 flows implemented
- **Flow Addition**: After requested flow(s) implemented
- **Payment Method Addition**: After PM implementation complete

### Quality Scoring System

```
Quality Score = 100 - (Critical Issues × 20) - (Warnings × 5) - (Suggestions × 1)

Thresholds:
95-100: Excellent ✨ - Auto-approve, document success patterns
80-94:  Good ✅ - Approve with minor notes
60-79:  Fair ⚠️ - Approve with warnings, recommend fixes
40-59:  Poor ❌ - Block until critical issues fixed
0-39:   Critical 🚨 - Block immediately, requires rework
```

### What Gets Reviewed

**UCS Pattern Compliance:**
- RouterDataV2 usage (not RouterData)
- ConnectorIntegrationV2 usage (not ConnectorIntegration)
- domain_types imports (not hyperswitch_*)
- Generic connector struct pattern
- Macro-based implementation (not manual traits)

**Code Quality:**
- No code duplication
- Consistent error handling
- Proper status mapping
- Specific error messages (not generic)
- No fields hardcoded to None

**Security & Performance:**
- No exposed credentials
- Efficient resource usage
- Proper input validation

### Feedback Database

All quality issues and success patterns are captured in `guides/feedback.md`:

```
guides/feedback.md
├── Quality Review Template
├── Section 1: Critical Patterns (Must Follow)
├── Section 2: UCS-Specific Guidelines
├── Section 3: Flow-Specific Best Practices
├── Section 4: Payment Method Patterns
├── Section 5: Common Anti-Patterns
├── Section 6: Success Patterns
└── Section 7: Historical Feedback Archive
```

---

## 💡 Pro Tips

1. **Choose the right workflow** - Don't use `.gracerules` for adding a single flow
2. **Check prerequisites** - Refund needs Capture, Payment Methods need Authorize
3. **Be specific** - "add Refund flow to Stripe" is better than "fix Stripe"
4. **One task at a time** - Complete one workflow before starting another
5. **Use tech specs** - Always provide comprehensive tech specs for new connectors
6. **Review feedback.md** - Learn from past issues before implementing

---

**GRACE-UCS makes UCS connector development efficient, granular, and resumable at any stage.**
