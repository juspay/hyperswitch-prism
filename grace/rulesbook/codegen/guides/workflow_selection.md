# Grace Workflow Selection Guide

This guide helps you choose the right Grace workflow controller for your UCS connector task.

<!-- PR #855 rename absorbed (commit c9e1025e3, 2026-04-02): CreateAccessToken →
ServerAuthenticationToken, CreateSessionToken → ServerSessionAuthenticationToken,
SdkSessionToken → ClientAuthenticationToken (plus matching traits and request/
response data types). See pattern_client_authentication_token.md for the full map. -->

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
│   ├── Add a flow?
│   │   │
│   │   ├── Core payment flow (Authorize/PSync/Capture/Refund/RSync/Void/VoidPC)
│   │   │   → .gracerules_add_flow
│   │   │
│   │   ├── Mandate / recurring (SetupMandate/RepeatPayment/MandateRevoke/
│   │   │   IncrementalAuthorization)
│   │   │   → .gracerules_add_flow
│   │   │
│   │   ├── Pre-authorization (CreateOrder/SessionToken/CreateConnectorCustomer/
│   │   │   PaymentMethodToken/
│   │   │   ServerSessionAuthenticationToken/ServerAuthenticationToken/
│   │   │   ClientAuthenticationToken)
│   │   │   → .gracerules_add_flow  (token markers map to
│   │   │                            pattern_server_authentication_token.md)
│   │   │
│   │   ├── 3DS authentication (PreAuthenticate/Authenticate/PostAuthenticate)
│   │   │   → .gracerules_add_flow
│   │   │
│   │   ├── Webhook (IncomingWebhook/VerifyWebhookSource)
│   │   │   → .gracerules_add_flow
│   │   │
│   │   ├── Dispute (AcceptDispute/SubmitEvidence/DefendDispute/DSync)
│   │   │   → .gracerules_add_flow
│   │   │
│   │   └── Payouts (PayoutCreate/PayoutTransfer/PayoutGet/PayoutVoid/
│   │       PayoutStage/PayoutCreateLink/PayoutCreateRecipient/
│   │       PayoutEnrollDisburseAccount)
│   │       → .gracerules_add_flow
│   │
│   └── Add a payment method?
│       (Card/CardRedirect/CardToken/NetworkToken/Wallet/PayLater/
│        BankRedirect/OpenBanking/BankDebit/BankTransfer/Upi/Crypto/
│        GiftCard/MobilePayment/Reward/Voucher/RealTimePayment/
│        MandatePayment, plus Card-NTID / Wallet-NTID sub-patterns)
│       → .gracerules_add_payment_method
│           Command: add {Category}:{types} to {Connector} using \
│                    grace/rulesbook/codegen/.gracerules_add_payment_method
│
└── Fix or improve existing connector?
    └── Use .gracerules_add_flow (for flow fixes) or manual editing
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

| Flow                             | Prerequisites | connector_flow.rs Marker              | Pattern File                                      |
| -------------------------------- | ------------- | ------------------------------------- | ------------------------------------------------- |
| Authorize                        | None          | `Authorize`                           | `patterns/pattern_authorize.md`                   |
| PSync                            | Authorize     | `PSync`                               | `patterns/pattern_psync.md`                       |
| Capture                          | Authorize     | `Capture`                             | `patterns/pattern_capture.md`                     |
| Void                             | Authorize     | `Void`                                | `patterns/pattern_void.md`                        |
| VoidPC                           | Authorize     | `VoidPC`                              | `patterns/pattern_void_pc.md`                     |
| Refund                           | Capture       | `Refund`                              | `patterns/pattern_refund.md`                      |
| RSync                            | Refund        | `RSync`                               | `patterns/pattern_rsync.md`                       |
| SetupMandate                     | Authorize     | `SetupMandate`                        | `patterns/pattern_setup_mandate.md`               |
| RepeatPayment                    | SetupMandate  | `RepeatPayment`                       | `patterns/pattern_repeat_payment_flow.md`         |
| MandateRevoke                    | SetupMandate  | `MandateRevoke`                       | `patterns/pattern_mandate_revoke.md`              |
| IncrementalAuthorization         | Authorize     | `IncrementalAuthorization`            | `patterns/pattern_IncrementalAuthorization_flow.md` |
| IncomingWebhook                  | PSync         | _(FlowName::IncomingWebhook)_         | `patterns/pattern_IncomingWebhook_flow.md`        |
| VerifyWebhookSource              | IncomingWebhook | `VerifyWebhookSource`               | `patterns/pattern_verify_webhook_source.md`       |
| CreateOrder                      | -             | `CreateOrder`                         | `patterns/pattern_createorder.md`                 |
| SessionToken                     | -             | _(FlowName-only)_                     | `patterns/pattern_server_session_authentication_token.md` |
| ServerSessionAuthenticationToken | -             | `ServerSessionAuthenticationToken`    | `patterns/pattern_server_session_authentication_token.md` |
| ServerAuthenticationToken        | -             | `ServerAuthenticationToken`           | `patterns/pattern_server_authentication_token.md` (see "Mapping to connector_flow.rs token markers" section) |
| ClientAuthenticationToken        | -             | `ClientAuthenticationToken`           | `patterns/pattern_server_authentication_token.md` (canonical) + `patterns/pattern_client_authentication_token.md` (companion) |
| CreateConnectorCustomer          | -             | `CreateConnectorCustomer`             | `patterns/pattern_create_connector_customer.md`   |
| PaymentMethodToken               | -             | `PaymentMethodToken`                  | `patterns/pattern_payment_method_token.md`        |
| PreAuthenticate                  | -             | `PreAuthenticate`                     | `patterns/pattern_preauthenticate.md`             |
| Authenticate                     | PreAuthenticate | `Authenticate`                      | `patterns/pattern_authenticate.md`                |
| PostAuthenticate                 | Authenticate  | `PostAuthenticate`                    | `patterns/pattern_postauthenticate.md`            |
| DefendDispute                    | -             | `DefendDispute`                       | `patterns/pattern_defend_dispute.md`              |
| AcceptDispute                    | -             | `Accept`                              | `patterns/pattern_accept_dispute.md`              |
| SubmitEvidence                   | AcceptDispute | `SubmitEvidence`                      | `patterns/pattern_submit_evidence.md`             |
| DSync                            | -             | _(FlowName::Dsync)_                   | `patterns/pattern_dsync.md`                       |
| PayoutCreate                     | -             | `PayoutCreate`                        | `patterns/pattern_payout_create.md`               |
| PayoutTransfer                   | PayoutCreate  | `PayoutTransfer`                      | `patterns/pattern_payout_transfer.md`             |
| PayoutGet                        | PayoutCreate  | `PayoutGet`                           | `patterns/pattern_payout_get.md`                  |
| PayoutVoid                       | PayoutCreate  | `PayoutVoid`                          | `patterns/pattern_payout_void.md`                 |
| PayoutStage                      | PayoutCreate  | `PayoutStage`                         | `patterns/pattern_payout_stage.md`                |
| PayoutCreateLink                 | PayoutCreate  | `PayoutCreateLink`                    | `patterns/pattern_payout_create_link.md`          |
| PayoutCreateRecipient            | -             | `PayoutCreateRecipient`               | `patterns/pattern_payout_create_recipient.md`     |
| PayoutEnrollDisburseAccount      | PayoutCreateRecipient | `PayoutEnrollDisburseAccount` | `patterns/pattern_payout_enroll_disburse_account.md` |

**Pattern Files:**

- Flat flow patterns live in `guides/patterns/pattern_{flow_name}.md`
- Payment-method patterns live in `guides/patterns/authorize/{pm}/pattern_authorize_{pm}.md`

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

Every `PaymentMethodData` variant from
`crates/types-traits/domain_types/src/payment_method_data.rs` (20 variants) has
a dedicated pattern directory. Coverage is 100%.

| Category          | PaymentMethodData Variant | Types                                     | Pattern File                                                   |
| ----------------- | ------------------------- | ----------------------------------------- | -------------------------------------------------------------- |
| Card              | `Card`                    | Credit, Debit                             | `authorize/card/pattern_authorize_card.md`                     |
| Card (NTID / MIT) | `CardDetailsForNetworkTransactionId` | Card MIT via NTID             | `authorize/card/pattern_authorize_card_ntid.md`                |
| CardRedirect      | `CardRedirect`            | CarteBancaire, Knet, Benefit              | `authorize/card_redirect/pattern_authorize_card_redirect.md`   |
| CardToken         | `CardToken`               | Pre-tokenized card reference              | `authorize/card_token/pattern_authorize_card_token.md`         |
| NetworkToken      | `NetworkToken`            | VTS / MDES network tokens                 | `authorize/network_token/pattern_authorize_network_token.md`   |
| Wallet            | `Wallet`                  | Apple Pay, Google Pay, PayPal, WeChat Pay | `authorize/wallet/pattern_authorize_wallet.md`                 |
| Wallet (NTID / MIT) | `DecryptedWalletTokenDetailsForNetworkTransactionId` | Wallet MIT via decrypted token | `authorize/wallet/pattern_authorize_wallet_ntid.md` |
| BankTransfer      | `BankTransfer`            | SEPA, ACH, Wire                           | `authorize/bank_transfer/pattern_authorize_bank_transfer.md`   |
| BankDebit         | `BankDebit`               | SEPA Direct Debit, ACH Debit, BACS        | `authorize/bank_debit/pattern_authorize_bank_debit.md`         |
| BankRedirect      | `BankRedirect`            | iDEAL, Sofort, Giropay                    | `authorize/bank_redirect/pattern_authorize_bank_redirect.md`   |
| OpenBanking       | `OpenBanking`             | TrueLayer, Plaid OBIE PIS                 | `authorize/open_banking/pattern_authorize_open_banking.md`     |
| UPI               | `Upi`                     | Collect, Intent, QR                       | `authorize/upi/pattern_authorize_upi.md`                       |
| BNPL              | `PayLater`                | Klarna, Afterpay, Affirm                  | `authorize/bnpl/pattern_authorize_bnpl.md`                     |
| Crypto            | `Crypto`                  | Bitcoin, Ethereum                         | `authorize/crypto/pattern_authorize_crypto.md`                 |
| GiftCard          | `GiftCard`                | Gift Card                                 | `authorize/gift_card/pattern_authorize_gift_card.md`           |
| MobilePayment     | `MobilePayment`           | Carrier Billing                           | `authorize/mobile_payment/pattern_authorize_mobile_payment.md` |
| Reward            | `Reward`                  | Loyalty Points                            | `authorize/reward/pattern_authorize_reward.md`                 |
| Voucher           | `Voucher`                 | Boleto, OXXO, PayCash, Efecty             | `authorize/voucher/pattern_authorize_voucher.md`               |
| RealTimePayment   | `RealTimePayment`         | Pix, PromptPay, DuitNow, FedNow           | `authorize/real_time_payment/pattern_authorize_real_time_payment.md` |
| MandatePayment    | `MandatePayment`          | Mandate / CIT-based recurring             | `authorize/mandate_payment/pattern_authorize_mandate_payment.md` |

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

_Category Names:_ Card, CardRedirect, CardToken, NetworkToken, Wallet, BankTransfer, BankDebit, BankRedirect, OpenBanking, UPI, BNPL, Crypto, GiftCard, MobilePayment, Reward, Voucher, RealTimePayment, MandatePayment (NTID sub-patterns: `Card:NTID`, `Wallet:NTID`)

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

### Flow Patterns (flat layout)

```
guides/patterns/pattern_{flow_name}.md
```

Examples:

- `patterns/pattern_authorize.md`
- `patterns/pattern_capture.md`
- `patterns/pattern_refund.md`
- `patterns/pattern_payout_create.md`, `patterns/pattern_payout_transfer.md`, ...
- `patterns/pattern_preauthenticate.md`, `patterns/pattern_authenticate.md`, `patterns/pattern_postauthenticate.md`
- `patterns/pattern_create_connector_customer.md`
- `patterns/pattern_verify_webhook_source.md`
- `patterns/pattern_client_authentication_token.md`
- `patterns/pattern_server_authentication_token.md` (canonical source for the three
  token markers `ServerSessionAuthenticationToken`, `ServerAuthenticationToken`,
  and `ClientAuthenticationToken` — see its "Mapping to connector_flow.rs
  token markers" section)
- `patterns/pattern_server_session_authentication_token.md` (wallet-session bootstrap flow)

### Payment Method Patterns (authorize/ tree)

```
guides/patterns/authorize/{payment_method}/pattern_authorize_{payment_method}.md
```

Examples:

- `patterns/authorize/card/pattern_authorize_card.md`
- `patterns/authorize/card/pattern_authorize_card_ntid.md`
- `patterns/authorize/wallet/pattern_authorize_wallet.md`
- `patterns/authorize/wallet/pattern_authorize_wallet_ntid.md`
- `patterns/authorize/bank_transfer/pattern_authorize_bank_transfer.md`
- `patterns/authorize/voucher/pattern_authorize_voucher.md`
- `patterns/authorize/real_time_payment/pattern_authorize_real_time_payment.md`
- `patterns/authorize/card_redirect/pattern_authorize_card_redirect.md`
- `patterns/authorize/open_banking/pattern_authorize_open_banking.md`
- `patterns/authorize/network_token/pattern_authorize_network_token.md`
- `patterns/authorize/card_token/pattern_authorize_card_token.md`
- `patterns/authorize/mandate_payment/pattern_authorize_mandate_payment.md`

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
