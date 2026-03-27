# Grace Workflow Selection Guide

This guide helps you choose the right Grace workflow controller for your UCS connector task.

## Quick Decision Tree

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
│   │   └── Use .gracerules_add_flow
│       Command: add {flow} flow to {Connector} using grace/rulesbook/codegen/.gracerules_add_flow
│   │
│   └── Add a payment method (Apple Pay, Cards, etc.)?
│       └── Use .gracerules_add_payment_method
│           Command: add {payment_method} to {Connector} using grace/rulesbook/codegen/.gracerules_add_payment_method
│
└── Fix or improve existing connector?
    └── Use .gracerules_flow (for flow fixes) or manual editing
```

> **Note**: Always use explicit form with full path to the workflow file to avoid ambiguity.

## Workflow Controllers

### 1. `.gracerules` - New Connector Integration

**When to Use:**

- Building a new connector from scratch
- Connector doesn't exist yet in the codebase
- Need complete implementation (all core flows)

**What It Does:**

1. Creates connector foundation (using `add_connector.sh`)
2. Implements all 6 core flows in sequence:
   - Authorize → PSync → Capture → Refund → RSync → Void
3. Runs quality review

**Trigger Commands:**

```bash
# Explicit form (recommended)
integrate {ConnectorName} using grace/rulesbook/codegen/.gracerules
integrate Stripe using grace/rulesbook/codegen/.gracerules
```

**Prerequisites:**

- Tech spec placed in `grace/rulesbook/codegen/references/{connector_name}/technical_specification.md`

**Output:**

- Complete connector with all core flows
- Ready for testing

---

### 2. `.gracerules_add_flow` - Add Specific Flows

**When to Use:**

- Connector already exists
- Need to add one or more missing flows
- Resume partial implementation
- Fix/improve existing flow

**What It Does:**

1. Analyzes existing connector state
2. Validates prerequisites for requested flow
3. Implements only the requested flow(s)
4. Ensures integration with existing code

**Trigger Commands:**

```bash
# Explicit form (recommended)
add {flow_name} flow to {connector_name} using grace/rulesbook/codegen/.gracerules_add_flow
add Refund flow to Stripe using grace/rulesbook/codegen/.gracerules_add_flow
add Capture and Void flows to Adyen using grace/rulesbook/codegen/.gracerules_add_flow
```

**Supported Flows:**

| Flow               | Prerequisites | Description                        |
| ------------------ | ------------- | ---------------------------------- |
| Authorize          | None          | Payment authorization (foundation) |
| PSync              | Authorize     | Payment status sync                |
| Capture            | Authorize     | Capture authorized payments        |
| Void               | Authorize     | Cancel authorized payments         |
| Refund             | Capture       | Refund captured payments           |
| RSync              | Refund        | Refund status sync                 |
| SetupMandate       | Authorize     | Set up recurring payments          |
| RepeatPayment      | SetupMandate  | Process recurring payments         |
| IncomingWebhook    | PSync         | Webhook handling                   |
| CreateOrder        | -             | Multi-step payment initiation      |
| SessionToken       | -             | Secure session management          |
| PaymentMethodToken | -             | Tokenize payment methods           |
| DefendDispute      | -             | Defend chargebacks                 |
| AcceptDispute      | -             | Accept chargebacks                 |
| DSync              | -             | Dispute status sync                |

**Pattern Files:**

- `guides/patterns/{flow_name}/pattern_{flow_name}.md`

---

### 3. `.gracerules_add_payment_method` - Add Payment Methods

**When to Use:**

- Connector exists with Authorize flow
- Need to add support for new payment method(s)
- Expand payment method coverage

**What It Does:**

1. Analyzes existing connector state
2. Checks which flows need the payment method
3. Implements payment method handling in transformers
4. Adds PM-specific request/response handling

**Trigger Commands:**

```bash
# Explicit form (required) - Category prefix syntax
add {Category}:{payment_method1},{payment_method2} to {connector_name} using grace/rulesbook/codegen/.gracerules_add_payment_method
add Wallet:Apple Pay,Google Pay and Card:Credit,Debit to Stripe using grace/rulesbook/codegen/.gracerules_add_payment_method
add Wallet:PayPal and BankTransfer:SEPA,ACH to Wise using grace/rulesbook/codegen/.gracerules_add_payment_method
add UPI:Collect,Intent to PhonePe using grace/rulesbook/codegen/.gracerules_add_payment_method
```

**Supported Payment Methods:**

| Category      | Types                                     | Pattern File                                                   |
| ------------- | ----------------------------------------- | -------------------------------------------------------------- |
| Card          | Credit, Debit                             | `authorize/card/pattern_authorize_card.md`                     |
| Wallet        | Apple Pay, Google Pay, PayPal, WeChat Pay | `authorize/wallet/pattern_authorize_wallet.md`                 |
| BankTransfer  | SEPA, ACH, Wire                           | `authorize/bank_transfer/pattern_authorize_bank_transfer.md`   |
| BankDebit     | SEPA Direct Debit, ACH Debit              | `authorize/bank_debit/pattern_authorize_bank_debit.md`         |
| BankRedirect  | iDEAL, Sofort, Giropay                    | `authorize/bank_redirect/pattern_authorize_bank_redirect.md`   |
| UPI           | Collect, Intent                           | `authorize/upi/pattern_authorize_upi.md`                       |
| BNPL          | Klarna, Afterpay, Affirm                  | `authorize/bnpl/pattern_authorize_bnpl.md`                     |
| Crypto        | Bitcoin, Ethereum                         | `authorize/crypto/pattern_authorize_crypto.md`                 |
| GiftCard      | Gift Card                                 | `authorize/gift_card/pattern_authorize_gift_card.md`           |
| MobilePayment | Carrier Billing                           | `authorize/mobile_payment/pattern_authorize_mobile_payment.md` |
| Reward        | Loyalty Points                            | `authorize/reward/pattern_authorize_reward.md`                 |

**Payment Method Specification Syntax:**

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
add Wallet:Apple Pay,Google Pay and Card:Credit,Debit and BankTransfer:ACH to Stripe
```

_Category Names:_ Card, Wallet, BankTransfer, BankDebit, BankRedirect, UPI, BNPL, Crypto, GiftCard, MobilePayment, Reward

**Prerequisites:**

- Authorize flow must be implemented (required foundation)

---

## Common Scenarios

### Scenario 1: New Connector Integration

**Situation:** You need to integrate a new payment gateway (e.g., "NewPay") that doesn't exist in UCS.

**Solution:** Use `.gracerules`

**Steps:**

1. Create tech spec at `grace/rulesbook/codegen/references/newpay/technical_specification.md`
2. Run: `integrate NewPay using grace/rulesbook/codegen/.gracerules`
3. AI will create complete connector with all 6 core flows

---

### Scenario 2: Add Missing Flow to Existing Connector

**Situation:** Stripe connector has Authorize, Capture, but is missing Refund.

**Solution:** Use `.gracerules_flow`

**Command:**

```bash
add Refund flow to Stripe
```

**What Happens:**

1. AI detects Stripe exists with Authorize and Capture
2. Validates Refund prerequisites (needs Capture - ✅ exists)
3. Implements Refund flow only
4. Integrates with existing code

---

### Scenario 3: Add Payment Method to Existing Connector

**Situation:** Adyen connector supports Cards but needs Apple Pay.

**Solution:** Use `.gracerules_payment_method`

**Command:**

```bash
add Apple Pay to Adyen
```

**What Happens:**

1. AI detects Adyen exists with Authorize flow
2. Adds Apple Pay handling in Authorize transformers
3. Adds to Refund if applicable

---

### Scenario 4: Resume Partial Implementation

**Situation:** You started integrating a connector but only completed Authorize and Capture.

**Solution:** Depends on what's missing

**Option A - Add specific flows:**

```bash
add Refund and Void flows to MyConnector
```

**Option B - Continue with complete integration:**

```bash
integrate MyConnector using grace/rulesbook/codegen/.gracerules
```

(Will detect existing flows and continue from there)

---

### Scenario 5: Fix Error Handling in Existing Flow

**Situation:** Stripe's Refund flow has incorrect error mapping.

**Solution:** Use `.gracerules_flow` with fix intent

**Command:**

```bash
fix error handling in Stripe Refund flow
```

Or manually edit using patterns from `guides/flows/refund/`

---

## Workflow Comparison

| Aspect               | `.gracerules`         | `.gracerules_add_flow` | `.gracerules_add_payment_method`  |
| -------------------- | --------------------- | ---------------------- | --------------------------------- |
| **Purpose**          | New connector         | Add flows              | Add payment methods               |
| **Starting Point**   | Empty/foundation only | Existing connector     | Existing connector with Authorize |
| **What It Adds**     | All core flows        | Specific flow(s)       | Payment method handling           |
| **Files Modified**   | Creates new files     | Modifies existing      | Modifies transformers             |
| **Prerequisites**    | Tech spec             | Connector exists       | Authorize flow exists             |
| **Typical Duration** | Full integration      | Single flow            | Single payment method             |

## Pattern File Locations

### Flow Patterns

```
guides/flows/{flow_name}/pattern_{flow_name}.md
```

Examples:

- `flows/authorize/pattern_authorize.md`
- `flows/capture/pattern_capture.md`
- `flows/refund/pattern_refund.md`

### Payment Method Patterns

```
guides/flows/authorize/{payment_method}.md
```

Examples:

- `flows/authorize/card.md`
- `flows/authorize/wallet.md`
- `flows/authorize/bank_transfer.md`

## Tips for Best Results

1. **Always start with the right workflow** - Using wrong workflow wastes time
2. **Check prerequisites** - Flows have dependencies (e.g., Refund needs Capture)
3. **Payment methods need Authorize** - Can't add PM without Authorize flow
4. **Be specific** - "Add Refund flow to Stripe" is better than "fix Stripe"
5. **One task at a time** - Complete one workflow before starting another

## Troubleshooting

### "Connector not found"

- Check connector name spelling
- Verify connector exists in `crates/integrations/connector-integration/src/connectors/`
- If new connector, use `.gracerules` instead

### "Prerequisites not met"

- Check flow dependencies table
- Implement prerequisite flows first
- Example: Can't add Refund without Capture

### "Payment method already supported"

- Check existing transformers.rs
- May need to add to additional flows
- Or PM is already implemented

## Related Documentation

- [Patterns README](./patterns/README.md) - Pattern overview
- [Flows README](./flows/README.md) - Flow patterns index
- [Connector Integration Guide](./connector_integration_guide.md) - Step-by-step integration
- [Quality Guide](./quality/README.md) - Code quality standards
