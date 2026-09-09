# UCS Connector Implementation Patterns

This directory contains comprehensive implementation patterns for each payment flow in the UCS (Universal Connector Service) system. Each pattern file provides complete, reusable templates that can be consumed by AI to generate consistent, production-ready connector code.

## 📂 Directory Layout

Flow patterns are **flat files** in this directory. Payment-method patterns for the
Authorize flow live under `authorize/{payment_method}/`. There is no `flows/`
subdirectory.

```
guides/patterns/
├── README.md                        # This file
├── PATTERN_AUTHORING_SPEC.md        # How to write / update a pattern file
├── flow_macro_guide.md              # Shared macro patterns
├── macro_patterns_reference.md      # Complete macro reference
├── pattern_authorize.md             # One flat file per flow
├── pattern_capture.md
├── pattern_psync.md
├── pattern_void.md
├── pattern_void_pc.md
├── pattern_refund.md
├── pattern_rsync.md
├── pattern_preauthenticate.md       # 3DS trio (Mechanism 1)
├── pattern_authenticate.md
├── pattern_postauthenticate.md
├── pattern_authentication_dispatch.md   # next_authentication_step - mandatory with the trio
├── pattern_server_authentication_token.md          # merchant/credential auth (Mechanism 3)
├── pattern_server_session_authentication_token.md
├── pattern_client_authentication_token.md
├── ...                              # see the tables below for the full list
└── authorize/                       # Payment-method patterns for Authorize
    ├── README.md
    ├── card/
    │   ├── pattern_authorize_card.md
    │   └── pattern_authorize_card_ntid.md
    ├── wallet/
    │   ├── pattern_authorize_wallet.md
    │   └── pattern_authorize_wallet_ntid.md
    ├── card_redirect/
    ├── bank_transfer/
    ├── bank_debit/
    ├── bank_redirect/
    ├── open_banking/
    ├── payment_method_token/
    ├── network_token/
    ├── mandate_payment/
    ├── real_time_payment/
    ├── upi/
    ├── bnpl/
    ├── crypto/
    ├── gift_card/
    ├── mobile_payment/
    ├── reward/
    └── voucher/
```

## 🔐 The Three Auth Mechanisms — Read This Before Any Auth Flow

"Authentication" means **three unrelated things** in UCS, plus a fourth category
that is not authentication at all. Conflating them is the single largest source
of broken generated code: each has its own flow markers, its own
`resource_common_data` type, and its own gRPC service. Identify which row you
are in **before** opening a pattern file.

| # | Mechanism | Flow markers (`connector_flow.rs`) | `resource_common_data` | gRPC service (`services.proto`) | Pattern file(s) |
|---|-----------|------------------------------------|------------------------|---------------------------------|-----------------|
| **1** | **Standalone 3DS trio** — cardholder authentication run as its own leg(s) before Authorize | `PreAuthenticate` / `Authenticate` / `PostAuthenticate` | **`PaymentFlowData`** | `PaymentMethodAuthenticationService` (rpcs `PreAuthenticate` / `Authenticate` / `PostAuthenticate`) | [`pattern_preauthenticate.md`](./pattern_preauthenticate.md), [`pattern_authenticate.md`](./pattern_authenticate.md), [`pattern_postauthenticate.md`](./pattern_postauthenticate.md) **+ [`pattern_authentication_dispatch.md`](./pattern_authentication_dispatch.md) (mandatory)** |
| **2** | **In-payment 3DS** — 3DS folded into the Authorize call itself | *(none — no separate marker exists)* | `PaymentFlowData` | `PaymentService.Authorize` | [`pattern_authorize.md`](./pattern_authorize.md) |
| **3** | **Merchant / credential auth** — OAuth tokens, wallet sessions, client-SDK tokens. Authenticates **the merchant to the connector**, never the cardholder | `ServerAuthenticationToken` / `ServerSessionAuthenticationToken` / `ClientAuthenticationToken` | **`MerchantAuthenticationFlowData`** | `MerchantAuthenticationService` (rpcs `CreateServerAuthenticationToken` / `CreateServerSessionAuthenticationToken` / `CreateClientAuthenticationToken`) | [`pattern_server_authentication_token.md`](./pattern_server_authentication_token.md) (canonical), [`pattern_server_session_authentication_token.md`](./pattern_server_session_authentication_token.md), [`pattern_client_authentication_token.md`](./pattern_client_authentication_token.md) |
| **4** | **Authenticator connectors — NOT 3DS.** Bank-account linking / account verification. Lives in `src/authenticator_connectors/`, a **sibling** of `connectors/`, not a subdirectory of it. Sole member: `plaid` | Reuses `ClientAuthenticationToken`, plus `PaymentMethodToken` and `GetPaymentMethod` | `MerchantAuthenticationFlowData` for `ClientAuthenticationToken`; `PaymentFlowData` for the other two | `MerchantAuthenticationService` for the token leg | *No dedicated pattern.* Read `authenticator_connectors/plaid.rs` |

> **The `resource_common_data` split is the trap.** Mechanism 1 uses
> `PaymentFlowData`; Mechanism 3 uses `MerchantAuthenticationFlowData`. They are
> **not** interchangeable. `MerchantAuthenticationFlowData`
> (`crates/types-traits/domain_types/src/merchant_authentication_flow_data.rs`)
> deliberately omits every payment field — no amount, no payment-method data, no
> address. It carries only merchant identity, resolved `connectors` base URLs,
> `connector_request_reference_id`, `test_mode`, `return_url`,
> `connector_feature_data`, `order_details`, `merchant_request_id`, plus the
> standard raw/typed connector request-response observability fields
> (`raw_connector_response`, `typed_connector_response`, `raw_connector_request`,
> `typed_connector_request`, `connector_response_headers`). If a pattern file
> tells you to put `PaymentFlowData` on a `*AuthenticationToken` flow, that
> pattern file is wrong — check `connector_types.rs` and fix it.

**Ground truth for the six trait bindings** — all in
`crates/types-traits/interfaces/src/connector_types.rs`, each a supertrait
binding over `ConnectorIntegrationV2<Flow, ResourceCommonData, Request, Response>`:

```rust
pub trait PaymentPreAuthenticateV2<T: PaymentMethodDataTypes>:  ConnectorIntegrationV2<connector_flow::PreAuthenticate,  PaymentFlowData, PaymentsPreAuthenticateData<T>,  PaymentsResponseData> {}
pub trait PaymentAuthenticateV2<T: PaymentMethodDataTypes>:     ConnectorIntegrationV2<connector_flow::Authenticate,     PaymentFlowData, PaymentsAuthenticateData<T>,     PaymentsResponseData> {}
pub trait PaymentPostAuthenticateV2<T: PaymentMethodDataTypes>: ConnectorIntegrationV2<connector_flow::PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData> {}

pub trait ServerAuthentication:        ConnectorIntegrationV2<connector_flow::ServerAuthenticationToken,        MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData,        ServerAuthenticationTokenResponseData> {}
pub trait ServerSessionAuthentication: ConnectorIntegrationV2<connector_flow::ServerSessionAuthenticationToken, MerchantAuthenticationFlowData, ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData> {}
pub trait ClientAuthentication:        ConnectorIntegrationV2<connector_flow::ClientAuthenticationToken,        MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData,        PaymentsResponseData> {} // note the asymmetric response type
```

There is **no** `trait ConnectorFlow` with associated `type Request` / `type
Response` anywhere in this repo. If a pattern file teaches that shape, it is
describing a trait that does not exist — the real mechanism is the
`ConnectorIntegrationV2` supertrait binding shown above.

### Mechanism 1 does not run unless you also override the dispatcher

Implementing the trio is **not** enough to make it execute. `ValidationTrait` in
`crates/types-traits/interfaces/src/connector_types.rs` carries:

```rust
pub enum AuthenticationStep { PreAuthenticate, Authenticate, PostAuthenticate, Authorize }
pub enum RedirectState { InitialRequest, RedirectWithParams, RedirectWithoutParams }

// default method on ValidationTrait:
fn next_authentication_step(
    &self,
    _auth_type: common_enums::AuthenticationType,
    _payment_method: PaymentMethod,
    _redirect_state: RedirectState,
    _completed_step: Option<AuthenticationStep>,
) -> AuthenticationStep {
    AuthenticationStep::Authorize   // default: SKIP every 3DS leg
}
```

The loop that consumes it is `process_composite_authorize` in
`crates/internal/composite-service/src/payments.rs`; it walks the
`AuthenticationStep` arms and halts on `AuthenticationStep::Authorize`.

**Consequence:** a generated connector that implements `PreAuthenticate` /
`Authenticate` / `PostAuthenticate` but never overrides
`next_authentication_step` will compile, pass review, and its 3DS legs will be
**unreachable at runtime**. `connectors/barclaycard.rs` is the canonical
full-trio override. Enumerate the live overrides with:

```bash
grep -ln "fn next_authentication_step" \
  crates/integrations/connector-integration/src/connectors/*.rs
```

This dispatcher is what [`pattern_authentication_dispatch.md`](./pattern_authentication_dispatch.md)
covers. If that file is not present in your checkout, read the two sources named
above directly — the override is still mandatory.

### External 3DS providers do not route through UCS

The Hyperswitch **router** keeps its own `authentication_connectors` category
for external 3DS / EMV3DS providers. **With the single exception called out in
carve-out 1 below, that class runs inside the router and never touches UCS** —
there is no UCS connector to generate for it. There is no file under
`crates/integrations/connector-integration/src/connectors/` for 3dsecure.io,
Gpayments, Cardinal or Click-to-Pay / CTP. Those names **do** appear as variants
of the proto `Connector` enum (`GPAYMENTS`, `THREEDSECUREIO`, `CTP_MASTERCARD`,
`CTP_VISA` in `crates/types-traits/grpc-api-types/proto/payment.proto`) — that
enum mirrors the full Hyperswitch connector list, so a variant there is **not**
evidence that UCS implements the connector.

Do **not** create a UCS connector, a `superposition.toml` entry, or a
`connector_specs` entry for a router-side authentication connector.

Two carve-outs, both real:

1. **Netcetera is the one name from that class with a real UCS connector.**
   `connectors/netcetera.rs` exists and is **authentication-only**: it implements
   the standalone 3DS trio plus a **stub `Authorize`** that returns
   `NotImplemented` before any HTTP call, present purely to satisfy the
   `ConnectorServiceTrait` bound. It is registered in the **payment** registry
   (`pub mod netcetera;` in `connectors.rs`), *not* in
   `authenticator_connectors.rs` — Mechanism 4 is a different category. It also
   overrides `next_authentication_step`. Treat it as the reference shape for an
   authentication-only connector, not as licence to port every external 3DS
   vendor into UCS.

2. **The external-vault-proxy (VGS) variant keeps 3DS on the UCS side.** For
   merchants proxying card data through VGS / Basis Theory / Spreedly,
   `PaymentService.ProxyAuthorize` and `ProxySetupRecurring` take vault-aliased
   card data, and the PROXIED PAYMENT METHODS block in `services.proto` states
   that the 3DS flows (`PreAuthenticate`, `Authenticate`, `PostAuthenticate`)
   **are** available there, because the vault proxy substitutes the alias with
   the real PAN before forwarding to the 3DS server. Contrast the TOKENIZED
   PAYMENT METHODS block immediately above it: on `TokenAuthorize`, 3DS flows are
   **not** available, because a PSP token cannot be handed to an external 3DS
   directory server — for 3DS on stored tokens the proto directs you to
   `connector_feature_data` and connector-side delegated authentication.

## 📚 Available Patterns

### Core Payment Flows

| Flow | Pattern File | Status | Description |
|------|--------------|--------|-------------|
| **Authorize** | [`pattern_authorize.md`](./pattern_authorize.md) | ✅ Complete | Complete authorization flow patterns |
| **Capture** | [`pattern_capture.md`](./pattern_capture.md) | ✅ Complete | Payment capture flow patterns |
| **PSync** | [`pattern_psync.md`](./pattern_psync.md) | ✅ Complete | Payment status synchronization |
| **Void** | [`pattern_void.md`](./pattern_void.md) | ✅ Complete | Void/cancel authorization |
| **Refund** | [`pattern_refund.md`](./pattern_refund.md) | ✅ Complete | Full and partial refunds |
| **RSync** | [`pattern_rsync.md`](./pattern_rsync.md) | ✅ Complete | Refund status synchronization |

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
| **SessionToken** (FlowName-only) / **ServerSessionAuthenticationToken** | [`pattern_server_session_authentication_token.md`](./pattern_server_session_authentication_token.md) | ✅ Complete | **Mechanism 3** (`MerchantAuthenticationFlowData`, `MerchantAuthenticationService`). Wallet-session bootstrap (Apple Pay / Google Pay / PayPal) |
| **ServerAuthenticationToken** | [`pattern_server_authentication_token.md`](./pattern_server_authentication_token.md) | ✅ Complete | **Mechanism 3** (`MerchantAuthenticationFlowData`, `MerchantAuthenticationService`). OAuth / access-token acquisition. Canonical source for the `ServerSessionAuthenticationToken`, `ServerAuthenticationToken`, and `ClientAuthenticationToken` flow markers (see the "Mapping to connector_flow.rs token markers" section). |
| **ClientAuthenticationToken** | [`pattern_client_authentication_token.md`](./pattern_client_authentication_token.md) | ✅ Complete | **Mechanism 3** (`MerchantAuthenticationFlowData`, `MerchantAuthenticationService`; note the asymmetric `PaymentsResponseData` response). Client-side auth-token flow marker companion pattern |
| **CreateConnectorCustomer** | [`pattern_create_connector_customer.md`](./pattern_create_connector_customer.md) | ✅ Complete | Create customer on connector side before payment |
| **IncrementalAuthorization** | [`pattern_IncrementalAuthorization_flow.md`](./pattern_IncrementalAuthorization_flow.md) | ✅ Complete | Incremental authorization on existing auth |
| **VoidPC** | [`pattern_void_pc.md`](./pattern_void_pc.md) | ✅ Complete | Void pre-capture / pre-confirm |
| **DefendDispute** | [`pattern_defend_dispute.md`](./pattern_defend_dispute.md) | ✅ Complete | Defend against disputes |
| **AcceptDispute** | [`pattern_accept_dispute.md`](./pattern_accept_dispute.md) | ✅ Complete | Accept chargeback |
| **SubmitEvidence** | [`pattern_submit_evidence.md`](./pattern_submit_evidence.md) | ✅ Complete | Submit dispute evidence |
| **DSync** | [`pattern_dsync.md`](./pattern_dsync.md) | ✅ Complete | Dispute status sync |

> The three `*AuthenticationToken` rows above are **Mechanism 3** — merchant /
> credential authentication. They use `MerchantAuthenticationFlowData`, **not**
> `PaymentFlowData`, and they authenticate the merchant to the connector, never
> the cardholder. Do not confuse them with the 3DS trio in the next section.

### Authentication Flows (3DS / EMV3DS) — Mechanism 1 only

> These are **Mechanism 1** (standalone 3DS trio) from
> "[The Three Auth Mechanisms](#-the-three-auth-mechanisms--read-this-before-any-auth-flow)"
> above: `resource_common_data: PaymentFlowData`, served by
> `PaymentMethodAuthenticationService`. They are **not** the
> `*AuthenticationToken` flows — those are Mechanism 3, use
> `MerchantAuthenticationFlowData`, and are listed under Advanced Flows.

| Flow | Pattern File | Status | Description |
|------|--------------|--------|-------------|
| **PreAuthenticate** | [`pattern_preauthenticate.md`](./pattern_preauthenticate.md) | ✅ Complete | 3DS pre-authentication / version lookup |
| **Authenticate** | [`pattern_authenticate.md`](./pattern_authenticate.md) | ✅ Complete | 3DS authentication / challenge |
| **PostAuthenticate** | [`pattern_postauthenticate.md`](./pattern_postauthenticate.md) | ✅ Complete | 3DS post-authentication result retrieval |
| **Authentication dispatch** — `next_authentication_step` *(no flow marker; a `ValidationTrait` method)* | [`pattern_authentication_dispatch.md`](./pattern_authentication_dispatch.md) | ⚠️ **Mandatory companion** | How the trio is actually scheduled. **Without this override the three flows above compile but never execute** — the default returns `AuthenticationStep::Authorize`, skipping every 3DS leg. |

Derive the live roster of connectors implementing each leg — the counts in
individual pattern files go stale quickly:

```bash
grep -n "flow_name: PreAuthenticate\|flow_name: Authenticate,\|flow_name: PostAuthenticate" \
  crates/integrations/connector-integration/src/connectors/*.rs
```

`connectors/netcetera.rs` is the authentication-only connector (the trio plus a
stub `Authorize` returning `NotImplemented`); it is registered in the **payment**
registry, not `authenticator_connectors.rs`.

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

Almost every `PaymentMethodData` variant from
`crates/types-traits/domain_types/src/payment_method_data.rs` has a dedicated
pattern directory. The table below lists the canonical pattern per variant.
(`CardWithNoCvc` has no pattern file yet - follow the generic card pattern.)

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
| **PaymentMethodToken** | [`authorize/payment_method_token/pattern_authorize_payment_method_token.md`](./authorize/payment_method_token/pattern_authorize_payment_method_token.md) | Authorize |
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
- [`PATTERN_AUTHORING_SPEC.md`](./PATTERN_AUTHORING_SPEC.md) - How to write / update a pattern file
- [`authorize/README.md`](./authorize/README.md) - Payment-method pattern index for Authorize
- [`pattern_authentication_dispatch.md`](./pattern_authentication_dispatch.md) - `next_authentication_step` dispatch; required alongside the 3DS trio
- [`flow_macro_guide.md`](./flow_macro_guide.md) - Macro usage reference
- [`macro_patterns_reference.md`](./macro_patterns_reference.md) - Complete macro documentation

### Quality & Standards
- [`../feedback.md`](../feedback.md) - Quality feedback database and review template
- [`../quality/README.md`](../quality/README.md) - Quality system overview
- [`../quality/CONTRIBUTING_FEEDBACK.md`](../quality/CONTRIBUTING_FEEDBACK.md) - Guide for adding quality feedback

**🛡️ Quality Note**: All implementations using these patterns are reviewed by the Quality Guardian Subagent to ensure UCS compliance and code quality. Review common issues in `feedback.md` before implementing to avoid known anti-patterns.

---

**💡 Pro Tip**: Always choose the right workflow controller for your task. Use `.gracerules` for new connectors, `.gracerules_flow` for adding flows, and `.gracerules_payment_method` for adding payment methods.
