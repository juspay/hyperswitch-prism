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
│   │   │   AUTH MECHANISM 3 - merchant/credential auth. The three
│   │   │   *AuthenticationToken markers use MerchantAuthenticationFlowData
│   │   │   and MerchantAuthenticationService. NOT 3DS. See note ³.
│   │   │
│   │   ├── 3DS authentication (PreAuthenticate/Authenticate/PostAuthenticate)
│   │   │   → .gracerules_add_flow
│   │   │   AUTH MECHANISM 1 - standalone 3DS trio. Uses PaymentFlowData and
│   │   │   PaymentMethodAuthenticationService. You MUST also override
│   │   │   next_authentication_step (pattern_authentication_dispatch.md) or
│   │   │   the three legs never execute. See note ³.
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
│       (Card/CardRedirect/PaymentMethodToken/NetworkToken/Wallet/PayLater/
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

> **Path base**: every `patterns/...` path in the two tables below and in
> "Pattern File Locations" is relative to `grace/rulesbook/codegen/guides/`.
> So `patterns/pattern_authorize.md` means
> `grace/rulesbook/codegen/guides/patterns/pattern_authorize.md`.

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
| SessionToken _(alias)_           | -             | _(no marker — see note ¹)_            | `patterns/pattern_server_session_authentication_token.md` — **mechanism 3**, `MerchantAuthenticationFlowData` (note ³) |
| ServerSessionAuthenticationToken | -             | `ServerSessionAuthenticationToken`    | `patterns/pattern_server_session_authentication_token.md` — **mechanism 3**, `MerchantAuthenticationFlowData` (note ³) |
| ServerAuthenticationToken        | -             | `ServerAuthenticationToken`           | `patterns/pattern_server_authentication_token.md` (see "Mapping to connector_flow.rs token markers" section) — **mechanism 3**, `MerchantAuthenticationFlowData` (note ³) |
| ClientAuthenticationToken        | -             | `ClientAuthenticationToken`           | `patterns/pattern_server_authentication_token.md` (canonical) + `patterns/pattern_client_authentication_token.md` (companion) — **mechanism 3**, `MerchantAuthenticationFlowData` (note ³) |
| CreateConnectorCustomer          | -             | `CreateConnectorCustomer`             | `patterns/pattern_create_connector_customer.md`   |
| PaymentMethodToken               | -             | `PaymentMethodToken`                  | `patterns/pattern_payment_method_token.md`        |
| PreAuthenticate                  | -             | `PreAuthenticate`                     | `patterns/pattern_preauthenticate.md` — **mechanism 1**, `PaymentFlowData`; also needs `patterns/pattern_authentication_dispatch.md` (note ³) |
| Authenticate                     | PreAuthenticate | `Authenticate`                      | `patterns/pattern_authenticate.md` — **mechanism 1**, `PaymentFlowData`; also needs `patterns/pattern_authentication_dispatch.md` (note ³) |
| PostAuthenticate                 | Authenticate  | `PostAuthenticate`                    | `patterns/pattern_postauthenticate.md` — **mechanism 1**, `PaymentFlowData`; also needs `patterns/pattern_authentication_dispatch.md` (note ³) |
| _(dispatch override)_ `next_authentication_step` | any of the three above | _(no marker — a default method on `ValidationTrait`)_ | `patterns/pattern_authentication_dispatch.md` — **mandatory with mechanism 1** (note ⁴) |
| DefendDispute                    | -             | `DefendDispute`                       | `patterns/pattern_defend_dispute.md`              |
| AcceptDispute                    | -             | `Accept`                              | `patterns/pattern_accept_dispute.md`              |
| SubmitEvidence                   | AcceptDispute | `SubmitEvidence`                      | `patterns/pattern_submit_evidence.md`             |
| DSync                            | -             | _(FlowName::Dsync — see note ²)_      | `patterns/pattern_dsync.md`                       |
| PayoutCreate                     | -             | `PayoutCreate`                        | `patterns/pattern_payout_create.md`               |
| PayoutTransfer                   | PayoutCreate  | `PayoutTransfer`                      | `patterns/pattern_payout_transfer.md`             |
| PayoutGet                        | PayoutCreate  | `PayoutGet`                           | `patterns/pattern_payout_get.md`                  |
| PayoutVoid                       | PayoutCreate  | `PayoutVoid`                          | `patterns/pattern_payout_void.md`                 |
| PayoutStage                      | PayoutCreate  | `PayoutStage`                         | `patterns/pattern_payout_stage.md`                |
| PayoutCreateLink                 | PayoutCreate  | `PayoutCreateLink`                    | `patterns/pattern_payout_create_link.md`          |
| PayoutCreateRecipient            | -             | `PayoutCreateRecipient`               | `patterns/pattern_payout_create_recipient.md`     |
| PayoutEnrollDisburseAccount      | PayoutCreateRecipient | `PayoutEnrollDisburseAccount` | `patterns/pattern_payout_enroll_disburse_account.md` |

**Marker notes** (checked against `crates/types-traits/domain_types/src/connector_flow.rs`):

- ¹ `SessionToken` is a **trigger alias only**, not a marker. There is no
  `SessionToken` struct and no `FlowName::SessionToken`; a word-boundary search
  finds zero flow-marker hits under `crates/` (the single `crates/` hit is an
  unrelated error-message string in `payu.rs`) and zero under `*.proto`. Use
  `ServerSessionAuthenticationToken` — the real marker — when writing code; the
  alias is kept here only because `.gracerules_add_flow` accepts it as a request
  word. The pattern file is shared with the `ServerSessionAuthenticationToken`
  row on purpose.
- ² The marker is spelled `Dsync` (one capital), not `DSync`. `DSync` has zero
  hits under `crates/` and zero under `*.proto`. `Dsync` is a `FlowName` variant
  only — there is no `pub struct Dsync` in `connector_flow.rs`. The row label
  above is kept as `DSync` because that is the word users type; the code must
  say `FlowName::Dsync`.
- Every other marker in the table above resolves to a `pub struct` in
  `connector_flow.rs`, except `IncomingWebhook`, which — like `Dsync` — is a
  `FlowName` variant only. `AcceptDispute` maps to `pub struct Accept` (and
  `FlowName::AcceptDispute`), as the table's marker column already shows.
- ³ **"Authentication" is three unrelated mechanisms.** Getting the
  `resource_common_data` wrong here is the top codegen failure mode, because the
  two families share the word "authentication" and nothing else. Verified in
  `crates/types-traits/interfaces/src/connector_types.rs`:

  | Mechanism | Markers | `resource_common_data` | gRPC service |
  | --------- | ------- | ---------------------- | ------------ |
  | 1 — standalone 3DS trio (cardholder) | `PreAuthenticate` / `Authenticate` / `PostAuthenticate` | **`PaymentFlowData`** | `PaymentMethodAuthenticationService` |
  | 2 — in-payment 3DS (cardholder) | *(none; folded into Authorize)* | `PaymentFlowData` | `PaymentService.Authorize` |
  | 3 — merchant / credential auth (**not** cardholder) | `ServerAuthenticationToken` / `ServerSessionAuthenticationToken` / `ClientAuthenticationToken` | **`MerchantAuthenticationFlowData`** | `MerchantAuthenticationService` |

  A **fourth** thing is not authentication at all:
  `crates/integrations/connector-integration/src/authenticator_connectors/`
  (sole member `plaid`) does bank-account linking. It is a *sibling* of
  `connectors/`, not a subdirectory, and it is out of scope for
  `.gracerules_add_flow`. Full write-up:
  `patterns/README.md` → "The Three Auth Mechanisms".

  Confirm the split yourself before writing a macro invocation:

  ```bash
  # mechanism 1 — every hit is PaymentFlowData
  grep -n "flow_name: PreAuthenticate\|flow_name: Authenticate,\|flow_name: PostAuthenticate" -A1 \
    crates/integrations/connector-integration/src/connectors/*.rs | grep resource_common_data

  # mechanism 3 — every hit is MerchantAuthenticationFlowData
  grep -rn "flow_name: ServerAuthenticationToken\|flow_name: ServerSessionAuthenticationToken\|flow_name: ClientAuthenticationToken" -A1 \
    crates/integrations/connector-integration/src/ | grep resource_common_data
  ```

  **External 3DS providers do not route through UCS.** The Hyperswitch router's
  own `authentication_connectors` category (3dsecure.io, Gpayments, Cardinal,
  Click-to-Pay / CTP) runs entirely in the router; there are zero UCS connectors
  for those names. Do not generate a UCS connector, a `superposition.toml` entry,
  or a `connector_specs` entry for one. Two carve-outs: `connectors/netcetera.rs`
  *does* exist in UCS as an authentication-only connector (the trio plus a stub
  `Authorize` returning `NotImplemented`, registered in the **payment** registry);
  and the external-vault-proxy (VGS / Basis Theory / Spreedly) path keeps 3DS on
  the UCS side — `services.proto` states in its PROXIED PAYMENT METHODS block
  that the trio *is* available on `ProxyAuthorize` because the proxy substitutes
  the vault alias with the real PAN, whereas the TOKENIZED PAYMENT METHODS block
  says the trio is *not* available on `TokenAuthorize`.
- ⁴ `next_authentication_step` is **not** a flow and has no marker — it is a
  default method on `ValidationTrait` in
  `crates/types-traits/interfaces/src/connector_types.rs` that returns
  `AuthenticationStep::Authorize`, i.e. **skip every 3DS leg**. The consuming
  loop is `process_composite_authorize` in
  `crates/internal/composite-service/src/payments.rs`. A connector that
  implements the trio but does not override this method compiles cleanly and its
  3DS legs are **unreachable at runtime**. `connectors/barclaycard.rs` is the
  canonical full-trio override; list the current ones with
  `grep -ln "fn next_authentication_step" crates/integrations/connector-integration/src/connectors/*.rs`.

**Pattern Files:**

- Flat flow patterns live in `guides/patterns/pattern_{flow_name}.md`
- Payment-method patterns live in `guides/patterns/authorize/{pm}/pattern_authorize_{pm}.md`
- In the tables, both are written from the `guides/` base — i.e. `patterns/...`

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

`PaymentMethodData` in `crates/types-traits/domain_types/src/payment_method_data.rs:362`
has **21** variants. 20 of them have a pattern file below; `CardWithNoCvc`
does not yet (see its row below). The two NTID variants share the `card/` and
`wallet/` directories rather than getting one of their own. Paths in the table are relative to
`grace/rulesbook/codegen/guides/`, the same base as the flow table above.

| Category            | PaymentMethodData Variant | Types                                     | Pattern File                                                            |
| ------------------- | ------------------------- | ----------------------------------------- | ----------------------------------------------------------------------- |
| Card                | `Card`                    | Credit, Debit                             | `patterns/authorize/card/pattern_authorize_card.md`                     |
| Card (no CVC)       | `CardWithNoCvc`           | Raw card without CVC                      | **No pattern file yet — follow the generic card pattern** `patterns/authorize/card/pattern_authorize_card.md` |
| Card (NTID / MIT)   | `CardDetailsForNetworkTransactionId` | Card MIT via NTID              | `patterns/authorize/card/pattern_authorize_card_ntid.md`                |
| CardRedirect        | `CardRedirect`            | CarteBancaire, Knet, Benefit              | `patterns/authorize/card_redirect/pattern_authorize_card_redirect.md`   |
| PaymentMethodToken  | `PaymentMethodToken`      | Pre-tokenized card reference              | `patterns/authorize/payment_method_token/pattern_authorize_payment_method_token.md` |
| NetworkToken        | `NetworkToken`            | VTS / MDES network tokens                 | `patterns/authorize/network_token/pattern_authorize_network_token.md`   |
| Wallet              | `Wallet`                  | Apple Pay, Google Pay, PayPal, WeChat Pay | `patterns/authorize/wallet/pattern_authorize_wallet.md`                 |
| Wallet (NTID / MIT) | `DecryptedWalletTokenDetailsForNetworkTransactionId` | Wallet MIT via decrypted token | `patterns/authorize/wallet/pattern_authorize_wallet_ntid.md` |
| BankTransfer        | `BankTransfer`            | SEPA, ACH, Wire                           | `patterns/authorize/bank_transfer/pattern_authorize_bank_transfer.md`   |
| BankDebit           | `BankDebit`               | SEPA Direct Debit, ACH Debit, BACS        | `patterns/authorize/bank_debit/pattern_authorize_bank_debit.md`         |
| BankRedirect        | `BankRedirect`            | iDEAL, Sofort, Giropay                    | `patterns/authorize/bank_redirect/pattern_authorize_bank_redirect.md`   |
| OpenBanking         | `OpenBanking`             | TrueLayer, Plaid OBIE PIS                 | `patterns/authorize/open_banking/pattern_authorize_open_banking.md`     |
| UPI                 | `Upi`                     | Collect, Intent, QR                       | `patterns/authorize/upi/pattern_authorize_upi.md`                       |
| BNPL                | `PayLater`                | Klarna, Afterpay, Affirm                  | `patterns/authorize/bnpl/pattern_authorize_bnpl.md`                     |
| Crypto              | `Crypto`                  | Bitcoin, Ethereum                         | `patterns/authorize/crypto/pattern_authorize_crypto.md`                 |
| GiftCard            | `GiftCard`                | Gift Card                                 | `patterns/authorize/gift_card/pattern_authorize_gift_card.md`           |
| MobilePayment       | `MobilePayment`           | Carrier Billing                           | `patterns/authorize/mobile_payment/pattern_authorize_mobile_payment.md` |
| Reward              | `Reward`                  | Loyalty Points                            | `patterns/authorize/reward/pattern_authorize_reward.md`                 |
| Voucher             | `Voucher`                 | Boleto, OXXO, PayCash, Efecty             | `patterns/authorize/voucher/pattern_authorize_voucher.md`               |
| RealTimePayment     | `RealTimePayment`         | Pix, PromptPay, DuitNow, FedNow           | `patterns/authorize/real_time_payment/pattern_authorize_real_time_payment.md` |
| MandatePayment      | `MandatePayment`          | Mandate / CIT-based recurring             | `patterns/authorize/mandate_payment/pattern_authorize_mandate_payment.md` |

> **`CardToken` was renamed to `PaymentMethodToken`** by commit `70e0883df`
> (PR #1010). There is no `PaymentMethodData::CardToken` variant and no
> `patterns/authorize/card_token/` directory — use the `PaymentMethodToken` row.
> (A grep for the bare word `CardToken` still hits `worldpay/requests.rs`; that is
> Worldpay's own `PaymentInstrument::CardToken`, unrelated to this enum.)
> Note that the PM-level `PaymentMethodToken` variant above and the flow-level
> `PaymentMethodToken` marker in the Supported Flows table are different things:
> the flow's pattern is `patterns/pattern_payment_method_token.md`.

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

_Category Names:_ Card, CardRedirect, PaymentMethodToken, NetworkToken, Wallet, BankTransfer, BankDebit, BankRedirect, OpenBanking, UPI, BNPL, Crypto, GiftCard, MobilePayment, Reward, Voucher, RealTimePayment, MandatePayment (NTID sub-patterns: `Card:NTID`, `Wallet:NTID`)

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

**Solution:** Use `.gracerules_add_flow`

**Command:**

```bash
add Refund flow to Stripe using grace/rulesbook/codegen/.gracerules_add_flow
```

**What Happens:**

1. AI detects Stripe exists with Authorize and Capture
2. Validates Refund prerequisites (needs Capture - ✅ exists)
3. Implements Refund flow only
4. Integrates with existing code

---

### Scenario 3: Add Payment Method to Existing Connector

**Situation:** Adyen connector supports Cards but needs Apple Pay.

**Solution:** Use `.gracerules_add_payment_method`

**Command:**

```bash
add Wallet:Apple Pay to Adyen using grace/rulesbook/codegen/.gracerules_add_payment_method
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

**Solution:** Use `.gracerules_add_flow` with fix intent

**Command:**

```bash
fix error handling in Stripe Refund flow
```

Or manually edit using the pattern at `patterns/pattern_refund.md` (i.e.
`grace/rulesbook/codegen/guides/patterns/pattern_refund.md`; there is no
`guides/flows/` directory)

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
  (**auth mechanism 1** — standalone 3DS trio, `PaymentFlowData`,
  `PaymentMethodAuthenticationService`)
- `patterns/pattern_authentication_dispatch.md` (**mandatory companion to the trio** —
  the `next_authentication_step` override on `ValidationTrait`; without it the
  three flows above never execute)
- `patterns/pattern_create_connector_customer.md`
- `patterns/pattern_verify_webhook_source.md`
- `patterns/pattern_client_authentication_token.md` (**auth mechanism 3**)
- `patterns/pattern_server_authentication_token.md` (**auth mechanism 3** — merchant /
  credential auth, `MerchantAuthenticationFlowData`, `MerchantAuthenticationService`;
  canonical source for the three token markers `ServerSessionAuthenticationToken`,
  `ServerAuthenticationToken`, and `ClientAuthenticationToken` — see its
  "Mapping to connector_flow.rs token markers" section)
- `patterns/pattern_server_session_authentication_token.md` (**auth mechanism 3** —
  wallet-session bootstrap flow)

> The mechanism labels above are explained in note ³ of "Supported Flows" and in
> full in `patterns/README.md` → "The Three Auth Mechanisms". Mechanism 1 and
> mechanism 3 take **different** `resource_common_data` types
> (`PaymentFlowData` vs `MerchantAuthenticationFlowData`) and are served by
> **different** gRPC services; they are not variants of one another.

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
- `patterns/authorize/payment_method_token/pattern_authorize_payment_method_token.md`
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
- [The Three Auth Mechanisms](./patterns/README.md#-the-three-auth-mechanisms--read-this-before-any-auth-flow) - which of 3DS trio / in-payment 3DS / merchant-credential auth you are actually in, and why external 3DS providers never reach UCS
- [Authorize Patterns README](./patterns/authorize/README.md) - Payment-method pattern index
- [Connector Integration Guide](./connector_integration_guide.md) - Step-by-step integration
- [Quality Guide](./quality/README.md) - Code quality standards
