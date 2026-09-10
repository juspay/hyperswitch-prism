# Payout Enroll-Disburse-Account Flow Pattern

## Overview

The Payout Enroll-Disburse-Account flow registers a specific disbursement account (bank account, wallet, or card destination) with the connector, typically after a recipient has already been created via `PayoutCreateRecipient`. Some connectors expose this as a two-step onboarding: (1) create recipient profile with KYC, (2) enroll a payout destination. This flow is the second step. For connectors that fold the two into a single API call, expose only `PayoutCreateRecipient` and leave this flow registered as a stub.

The flow's outcome is a stable account identifier that downstream payout flows reference via `connector_payout_method_id`. Unlike `PayoutCreateRecipient`, this flow does not carry a `recipient_type` field — the recipient is assumed already registered and is addressed implicitly through the request's `merchant_payout_id` or via the connector's internal session state.

### Key Components

- Flow marker: `PayoutEnrollDisburseAccount` — `crates/types-traits/domain_types/src/connector_flow.rs`.
- Request type: `PayoutEnrollDisburseAccountRequest` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`.
- Response type: `PayoutEnrollDisburseAccountResponse` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`.
- Flow-data type: `PayoutFlowData` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`.
- Marker trait: `PayoutEnrollDisburseAccountV2` — `crates/types-traits/interfaces/src/connector_types.rs`, defined solely as the supertrait binding:

  ```rust
  // crates/types-traits/interfaces/src/connector_types.rs — pub trait PayoutEnrollDisburseAccountV2
  pub trait PayoutEnrollDisburseAccountV2:
      ConnectorIntegrationV2<
      connector_flow::PayoutEnrollDisburseAccount,
      PayoutFlowData,
      PayoutEnrollDisburseAccountRequest,
      PayoutEnrollDisburseAccountResponse,
  >
  {
  }
  ```

- Service trait: `PayoutServiceTrait` — `crates/types-traits/interfaces/src/connector_types.rs`.
- Stub macro arm: the `flow: PayoutEnrollDisburseAccount` arm of `expand_payout_implementation!` — `crates/integrations/connector-integration/src/connectors/macros.rs`.

### Payout connectors live in a sibling registry — and are outside certification

Payout connectors are **not** `connectors/` files. They live in
`crates/integrations/connector-integration/src/payout_connectors/`, a sibling directory of
`connectors/`, and are exported from `payout_connectors.rs` — the module file, a sibling of the `payout_connectors/` directory — as
`pub mod <name>; pub use self::<name>::<Name>Payouts;`. The ten at HEAD are
`cybersource`, `deutschebank`, `gotyme_sanlam`, `itaubank`, `loonio`, `paypal`, `santander`,
`truelayer`, `trustly`, `worldpayxml`.

The service trait is `interfaces::connector_types::PayoutServiceTrait`
(`crates/types-traits/interfaces/src/connector_types.rs`). Read its supertrait list before
assuming payment-side obligations carry over — it is
`ConnectorCommon + ServerAuthentication + PayoutCreateV2 + PayoutTransferV2 + PayoutGetV2 +
PayoutVoidV2 + PayoutStageV2 + PayoutCreateLinkV2 + PayoutCreateRecipientV2 +
PayoutEnrollDisburseAccountV2 + PayoutEligibilityV2` and **nothing else**. In particular it does
**not** require `ValidationTrait`, `IncomingWebhook`, `VerifyRedirectResponse`, `SourceVerification`
or `BodyDecoding`. Do not write those impls for a payout-only connector.

**Registration sites** (all six, in order):

1. `payout_connectors/<name>.rs` and `payout_connectors/<name>/transformers.rs`.
2. `payout_connectors.rs` — the module file, a sibling of the `payout_connectors/` directory: `pub mod <name>; pub use self::<name>::<Name>Payouts;`.
3. `crates/types-traits/domain_types/src/connector_types.rs` — a variant on `pub enum PayoutConnectorEnum`.
4. `crates/integrations/connector-integration/src/types.rs` — an arm in `PayoutConnectorData::convert_connector`.
5. `crates/types-traits/domain_types/src/types.rs` — a field on `pub struct Connectors` (`ConnectorParams`, or `ConnectorParamsWithCaBundle` if the connector needs a CA bundle, as `deutschebank` does) plus the matching `PayoutConnectorEnum` arm in `patch_payout_connector_urls`.
6. `impl PayoutServiceTrait for <Name>Payouts {}` in the connector file.

**No `config/superposition.toml` entry and no `connector_specs/<connector>/specs.json` entry are
required.** Three independent reasons, all in
`crates/internal/integration-tests/src/bin/check_connector_specs.rs`:

1. `main()` enumerates connectors by reading
   `crates/integrations/connector-integration/src/connectors/` only — a connector under
   `payout_connectors/` is never seen, so it is never asked for a spec directory. This is also why
   the merge-blocking `.github/scripts/verify-new-connectors.sh` gate does not fire for one.
2. `const IGNORE_SERVICES: &[&str] = &["PayoutService", "DisputeService"];` — declared identically in
   `check_connector_specs.rs` and `check_coverage.rs` — drops any payout suite that does appear.
3. `flow_to_suites()` has no `Payout*` arm, so payout flow names fall to its `_ => None`, and the
   eight names `PayoutCreate`, `PayoutGet`, `PayoutStage`, `PayoutTransfer`, `PayoutVoid`,
   `PayoutEnrollDisburseAccount`, `PayoutCreateRecipient`, `PayoutCreateLink` are listed in
   `OUT_OF_SCOPE_FLOWS` so the "unknown flow" check passes. (`PayoutEligibility` is **not** in that
   list — it is only ever declared under `payout_connectors/`, which reason 1 keeps out of the scan.)

That exemption is about tooling reach, not merit. Read the source comments precisely: the doc comment
*above* `OUT_OF_SCOPE_FLOWS` says only "A flow reaches this list only by decision", and the eight
payout names sit under the in-list marker `// Payouts — out of scope for the payment suites.` The
sharper phrasing — "Each is a coverage gap, not a decision that it should never be covered" — belongs
to the *third* group in that same list (`VoidPC`, `VerifyWebhookSource`, …), not to the payout group.
The framing still applies: payout flows are unlisted because no integration-test suite exists yet, not
because they should never be built. Do not quote the "coverage gap" comment as if it were attached to
the payout entries. The payout-only connectors
`santander`, `deutschebank` and `gotyme_sanlam` have no `config/superposition.toml` block at all;
base URLs reach them through the `Connectors` field added in step 5. (`loonio`, `trustly`,
`truelayer`, `worldpayxml`, `cybersource` and `paypal` *do* appear in `superposition.toml`, but only
because each also has a payment-side connector under `connectors/`.)

### Access tokens are supplied by the caller, not sequenced by a `ValidationTrait` hook

There is no `should_do_access_token` on the payout path — `PayoutServiceTrait` does not require
`ValidationTrait` at all. Instead:

- `PayoutServiceTrait` requires `ServerAuthentication`, whose binding is
  `ConnectorIntegrationV2<ServerAuthenticationToken, MerchantAuthenticationFlowData,
  ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>`. Note the flow data
  is **`MerchantAuthenticationFlowData`** (`crates/types-traits/domain_types/src/merchant_authentication_flow_data.rs`),
  *not* `PayoutFlowData` — see `impl ServerAuthentication for ItaubankPayouts` and the
  `ConnectorIntegrationV2<ServerAuthenticationToken, …>` block that follows it in
  `payout_connectors/itaubank.rs`.
- The caller obtains a token through `MerchantAuthenticationService`, then passes it back on the
  payout RPC: every payout request message in
  `crates/types-traits/grpc-api-types/proto/payouts.proto` carries an
  `optional SecretString access_token`. The `ForeignTryFrom` impls in
  `crates/types-traits/domain_types/src/payouts/types.rs` copy it into
  `PayoutFlowData.access_token`, which `PayoutFlowData::get_access_token()` then exposes to
  `get_headers`.


## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Connectors with Full Implementation](#connectors-with-full-implementation)
3. [Common Implementation Patterns](#common-implementation-patterns)
4. [Connector-Specific Patterns](#connector-specific-patterns)
5. [Code Examples](#code-examples)
6. [Integration Guidelines](#integration-guidelines)
7. [Best Practices](#best-practices)
8. [Common Errors / Gotchas](#common-errors--gotchas)
9. [Testing Notes](#testing-notes)
10. [Cross-References](#cross-references)

## Architecture Overview

Payout Enroll-Disburse-Account is the producer of the `connector_payout_method_id` that downstream payout flows consume. It runs once per (recipient, account) pair and produces a stable handle so merchants do not re-submit account details on every disbursement.

### Flow Hierarchy

```
PayoutCreateRecipient  (upstream — produces recipient id)
        |
        v
PayoutEnrollDisburseAccount  (this flow — produces connector_payout_method_id)
        |
        v
PayoutCreate / PayoutTransfer  (downstream — reference account via connector_payout_method_id)
        |
        v
PayoutGet
```

### Flow Type

`PayoutEnrollDisburseAccount` — zero-sized marker struct declared at `crates/types-traits/domain_types/src/connector_flow.rs`. Registered in `FlowName::PayoutEnrollDisburseAccount` at `crates/types-traits/domain_types/src/connector_flow.rs`.

### Request Type

`PayoutEnrollDisburseAccountRequest` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`:

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs
#[derive(Debug, Clone)]
pub struct PayoutEnrollDisburseAccountRequest {
    pub merchant_payout_id: Option<String>,
    pub amount: common_utils::types::MinorUnit,
    pub source_currency: common_enums::Currency,
    pub payout_method_data: Option<PayoutMethodData>,
}
```

Notable fields:

- `payout_method_data: Option<PayoutMethodData>` — the enum at `crates/types-traits/domain_types/src/payouts/payout_method_data.rs` carrying the concrete account shape (`Card`, `Bank`, `Wallet`, `BankRedirect`, `Passthrough`). This is the payload the connector validates and enrolls.
- No `recipient_type` — the enrollment is account-level, not profile-level.
- `amount` and `source_currency` exist for envelope consistency but most connector enroll APIs do not use them.

### Response Type

`PayoutEnrollDisburseAccountResponse` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`:

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs
#[derive(Debug, Clone)]
pub struct PayoutEnrollDisburseAccountResponse {
    pub merchant_payout_id: Option<String>,
    pub payout_status: common_enums::PayoutStatus,
    pub connector_payout_id: Option<String>,
    pub status_code: u16,
}
```

The enrolled account id is returned in `connector_payout_id: Option<String>` on
`pub struct PayoutEnrollDisburseAccountResponse`
(`crates/types-traits/domain_types/src/payouts/payouts_types.rs`) — four fields, and note that
unlike `PayoutCreateRecipientResponse` this response has **no** `payout_connector_metadata` field, so
an opaque connector handle has nowhere structured to live. `payout_status` mapping follows the same
scheme as `PayoutCreateRecipient`:

- `PayoutStatus::RequiresVendorAccountCreation` — enrollment submitted, connector-side verification pending (variant at `crates/common/common_enums/src/enums.rs`).
- `PayoutStatus::Success` — account enrolled and ready for disbursement.
- `PayoutStatus::Failure` — enrollment rejected.

### Resource Common Data

`PayoutFlowData` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`. See [pattern_payout_void.md](./pattern_payout_void.md) for the full breakdown.

### RouterDataV2 Shape

```rust
RouterDataV2<PayoutEnrollDisburseAccount, PayoutFlowData, PayoutEnrollDisburseAccountRequest, PayoutEnrollDisburseAccountResponse>
```

Canonical four-arg shape per §7 of `PATTERN_AUTHORING_SPEC.md`.

## Connectors with Full Implementation

**None.** All ten payout connectors register `PayoutEnrollDisburseAccountV2` with a fail-fast stub. Separate account-enrollment endpoints are rare: no payout connector in the tree exposes one.

Verify before trusting this:

```bash
rg -n 'ConnectorIntegrationV2<\s*PayoutEnrollDisburseAccount|flow_name: PayoutEnrollDisburseAccount' \
   crates/integrations/connector-integration/src/payout_connectors/
```

Every hit is a stub whose only method is a `get_url` returning
`IntegrationError::connector_flow_not_implemented(self.id(), "payout_enroll_disburse_account", IntegrationErrorContext::default())`.

Current implementation coverage: **0 of 10 payout connectors.**

| Connector | HTTP Method | Content Type | URL Pattern | Request Type | Notes |
| --- | --- | --- | --- | --- | --- |
| _(none)_ | — | — | — | — | See Stub Implementations below. |

This is a **coverage gap, not a decision that the flow should never be covered** — the same framing
`crates/internal/integration-tests/src/bin/check_connector_specs.rs` applies to uncovered flows.

### Stub Implementations

All ten, split by how the stub is produced:

- **Hand-written** — the seven non-generic payout connectors, which cannot use the payout macro:
  `payout_connectors/{cybersource,itaubank,loonio,paypal,santander,worldpayxml}.rs` write
  `impl PayoutEnrollDisburseAccountV2 for <Name>Payouts {}` plus a one-method
  `ConnectorIntegrationV2<PayoutEnrollDisburseAccount, PayoutFlowData, PayoutEnrollDisburseAccountRequest, PayoutEnrollDisburseAccountResponse>` block longhand;
  `payout_connectors/truelayer.rs` generates the same shape from its file-local
  `macro_rules! impl_unimplemented_payout_flow!`.
- **Macro** — the three generic payout connectors list `PayoutEnrollDisburseAccount` in the `payout_flows: [...]`
  array of `macros::macro_connector_payout_implementation!`:
  `payout_connectors/{deutschebank,gotyme_sanlam,trustly}.rs`.

The only payout flows with real implementations at HEAD are `PayoutTransfer` (all ten connectors),
`PayoutGet` (nine — all but `cybersource`), `PayoutCreate` (`santander` only), `PayoutVoid`
(`worldpayxml` only), `PayoutCreateRecipient` (`trustly` only) and `PayoutEligibility`
(`deutschebank` only). Use those as templates.

## Common Implementation Patterns

### Two authoring styles are in-tree — pick one and stay in it

- **Macro style (generic `<T>`)** — `payout_connectors/{deutschebank,gotyme_sanlam,trustly}.rs`:
  `macros::create_all_prerequisites!` for the struct, one
  `macros::macro_connector_implementation!` per *real* flow (with
  `resource_common_data: PayoutFlowData` and `flow_name: Payout…`), then a single
  `macros::macro_connector_payout_implementation!` with `payout_flows: [...]` listing only the
  flows that stay stubs.
- **Hand-written style (non-generic)** —
  `payout_connectors/{cybersource,itaubank,loonio,paypal,santander,truelayer,worldpayxml}.rs`:
  the connector struct has no type parameter, and every flow — real and stub — is a raw
  `impl ConnectorIntegrationV2<Flow, PayoutFlowData, FlowRequest, FlowResponse> for <Name>Payouts`
  block preceded by `impl Payout<Flow>V2 for <Name>Payouts {}`.

A flow may never be covered twice: if a flow appears in `payout_flows: [...]` it must **not** also
have a hand-written `impl`, or the build fails with a conflicting-implementation error.

### How the stub is registered today (generic connectors)

`ItaubankPayouts` is **not** an example of this — it is a non-generic unit struct and hand-writes its
stubs. The macro is used only by the three generic payout connectors:

```rust
// crates/integrations/connector-integration/src/payout_connectors/deutschebank.rs
macros::macro_connector_payout_implementation!(
    connector: DeutschebankPayouts,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    payout_flows: [
        PayoutCreate,
        PayoutVoid,
        PayoutStage,
        PayoutCreateLink,
        PayoutCreateRecipient,
        PayoutEnrollDisburseAccount   // <-- registers marker + empty integration impl
    ]
);
```

The `PayoutEnrollDisburseAccount` arm at `crates/integrations/connector-integration/src/connectors/macros.rs` produces:

```rust
// From crates/integrations/connector-integration/src/connectors/macros.rs
(
    connector: $connector: ident,
    flow: PayoutEnrollDisburseAccount,
    generic_type: $generic_type:tt,
    [ $($bounds:tt)* ]
) => {
    impl<$generic_type: $($bounds)*> ::interfaces::connector_types::PayoutEnrollDisburseAccountV2 for $connector<$generic_type> {}
    impl<$generic_type: $($bounds)*>
        ::interfaces::connector_integration_v2::ConnectorIntegrationV2<
            ::domain_types::connector_flow::PayoutEnrollDisburseAccount,
            ::domain_types::payouts::payouts_types::PayoutFlowData,
            ::domain_types::payouts::payouts_types::PayoutEnrollDisburseAccountRequest,
            ::domain_types::payouts::payouts_types::PayoutEnrollDisburseAccountResponse,
        > for $connector<$generic_type>
    {
        fn get_url(
            &self,
            _req: &::domain_types::router_data_v2::RouterDataV2<
                ::domain_types::connector_flow::PayoutEnrollDisburseAccount,
                ::domain_types::payouts::payouts_types::PayoutFlowData,
                ::domain_types::payouts::payouts_types::PayoutEnrollDisburseAccountRequest,
                ::domain_types::payouts::payouts_types::PayoutEnrollDisburseAccountResponse,
            >,
        ) -> ::common_utils::CustomResult<String, ::domain_types::errors::IntegrationError> {
            Err(::domain_types::errors::IntegrationError::connector_flow_not_implemented(
                ::interfaces::api::ConnectorCommon::id(self),
                "payout_enroll_disburse_account",
                ::domain_types::errors::IntegrationErrorContext::default(),
            ).into())
        }
    }
};
```

The body is **not** empty: `get_url` is overridden so the stub fails fast with
`IntegrationError::connector_flow_not_implemented(id, "payout_enroll_disburse_account", …)` rather than falling through to a
`ConnectorIntegrationV2` trait default. And the emitted target is `$connector<$generic_type>` — the
macro matches only a **generic** connector struct.

### Moving from stub to real

A flow is stub **or** real, never both; two impls of the same
`ConnectorIntegrationV2<PayoutEnrollDisburseAccount, …>` will not compile.

- **Generic connector** (`DeutschebankPayouts<T>`, `GotymeSanlamPayouts<T>`, `TrustlyPayouts<T>`):
  delete `PayoutEnrollDisburseAccount` from `payout_flows: [...]`, add a matching
  `(flow: PayoutEnrollDisburseAccount, request_body: …, response_body: …, router_data: RouterDataV2<PayoutEnrollDisburseAccount,
  PayoutFlowData, PayoutEnrollDisburseAccountRequest, PayoutEnrollDisburseAccountResponse>)` entry to
  `macros::create_all_prerequisites!`, write `impl PayoutEnrollDisburseAccountV2 for <Name>Payouts<T> {}`, and add a
  `macros::macro_connector_implementation!` call with `flow_name: PayoutEnrollDisburseAccount` and
  `resource_common_data: PayoutFlowData`. `payout_connectors/trustly.rs` does exactly this for
  `PayoutCreateRecipient`, `PayoutTransfer` and `PayoutGet`.
- **Non-generic connector** (the other seven): delete the hand-written stub `impl` and replace it
  with a longhand `impl ConnectorIntegrationV2<PayoutEnrollDisburseAccount, PayoutFlowData, PayoutEnrollDisburseAccountRequest, PayoutEnrollDisburseAccountResponse> for
  <Name>Payouts` block. The closest in-tree template is the `PayoutTransfer` impl in
  `payout_connectors/itaubank.rs` (under the `// ===== PAYOUT TRANSFER (REAL) =====` banner) or the
  `PayoutCreate` impl in `payout_connectors/santander.rs`.

Two signature points that trip up ports from the payments side:
`get_request_body` returns `CustomResult<Option<ConnectorRequestData>, IntegrationError>` — **not**
`Option<RequestContent>` (see `fn get_request_body` on `ConnectorIntegrationV2` in
`crates/types-traits/interfaces/src/connector_integration_v2.rs`) — and `handle_response_v2` should
delegate to `finalize_connector_response!(event_builder, response, data, res.status_code)` from
`crates/integrations/connector-integration/src/utils.rs` rather than hand-assembling the
`RouterDataV2`.

### What `macro_connector_payout_implementation!` actually emits

`crates/integrations/connector-integration/src/connectors/macros.rs` defines
`macro_connector_payout_implementation!` (three arms: a default arm that supplies all nine flows, a
recursive arm that peels one flow at a time, and an empty-list base case) which delegates per flow to
`expand_payout_implementation!`. Every arm of `expand_payout_implementation!` — including the
`PayoutEligibility` arm — emits **two** items:

```rust
impl<T: ...> ::interfaces::connector_types::Payout<Flow>V2 for $connector<T> {}
impl<T: ...> ConnectorIntegrationV2<PayoutXxx, PayoutFlowData, PayoutXxxRequest, PayoutXxxResponse>
    for $connector<T>
{
    fn get_url(&self, _req: &RouterDataV2<...>) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            ConnectorCommon::id(self), "payout_xxx", IntegrationErrorContext::default(),
        ).into())
    }
}
```

It is **not** an empty `{}` body: the macro overrides `get_url` so the stub fails fast with
`IntegrationError::connector_flow_not_implemented` and a flow-name string, rather than falling
through to a `ConnectorIntegrationV2` trait default. The generic parameter is mandatory — the macro
only matches `$connector<$generic_type>`, so a **non-generic** payout connector (`ItaubankPayouts`,
`PaypalPayouts`, `SantanderPayouts`, `WorldpayxmlPayouts`, `CybersourcePayouts`, `LoonioPayouts`,
`TruelayerPayouts`) cannot use it and must hand-write its stubs. `payout_connectors/truelayer.rs`
solves this with a file-local `macro_rules! impl_unimplemented_payout_flow!`; the other six write the
`impl` blocks out longhand.


### Payout-Method-Data Branching Pattern

The request's `payout_method_data` must be matched exhaustively over `PayoutMethodData` variants. Itaubank's `PayoutTransfer` transformer demonstrates the branch shape at `crates/integrations/connector-integration/src/payout_connectors/itaubank/transformers.rs` for one variant (`Bank::Pix`) with an explicit fallback arm. For enroll-disburse-account, authors MUST explicitly list every variant rather than relying on a wildcard:

```rust
// Reference structure — mirrors itaubank's Bank::Pix branch at
// crates/integrations/connector-integration/src/payout_connectors/itaubank/transformers.rs
match req.request.payout_method_data.clone() {
    Some(PayoutMethodData::Bank(Bank::Ach(ach))) => {
        // build ACH enroll body from ach.bank_account_number, ach.bank_routing_number
    }
    Some(PayoutMethodData::Bank(Bank::Bacs(bacs))) => { /* BACS */ }
    Some(PayoutMethodData::Bank(Bank::Sepa(sepa))) => { /* SEPA */ }
    Some(PayoutMethodData::Bank(Bank::Pix(pix))) => { /* PIX */ }
    Some(PayoutMethodData::Bank(Bank::PixKey(k))) => { /* PIX key */ }
    Some(PayoutMethodData::Bank(Bank::PixEmv(e))) => { /* PIX EMV */ }
    Some(PayoutMethodData::Bank(Bank::OpenBanking(ob))) => { /* open banking */ }
    Some(PayoutMethodData::Bank(Bank::Trustly(t))) => { /* Trustly */ }
    Some(PayoutMethodData::Bank(Bank::Payshap(p))) => { /* PayShap */ }
    Some(PayoutMethodData::Bank(Bank::PayshapProxy(p))) => { /* PayShap proxy */ }
    Some(PayoutMethodData::Card(card)) => { /* card destination */ }
    Some(PayoutMethodData::Wallet(wallet)) => { /* wallet destination */ }
    Some(PayoutMethodData::BankRedirect(br)) => { /* BankRedirect destination */ }
    Some(PayoutMethodData::Passthrough(token)) => { /* passthrough PSP token */ }
    None => {
        return Err(IntegrationError::MissingRequiredField {
            field_name: "payout_method_data",
            context: Default::default(),
        }.into());
    }
}
```

The `Bank` enum has **ten** variants, not four — `Ach`, `Bacs`, `Sepa`, `Pix`, `PixKey`, `PixEmv`,
`OpenBanking`, `Trustly`, `Payshap`, `PayshapProxy` — see `pub enum Bank` in
`crates/types-traits/domain_types/src/payouts/payout_method_data.rs`. The outer `PayoutMethodData`
has five: `Card`, `Bank`, `Wallet`, `BankRedirect`, `Passthrough`. `BankRedirect` in turn has two
(`Interac`, `OpenBankingUk`). Read the enums; do not copy the variant list from a doc.

## Connector-Specific Patterns

### itaubank

- `ItaubankPayouts` is a non-generic unit struct and carries a **hand-written**
  `PayoutEnrollDisburseAccount` stub in
  `crates/integrations/connector-integration/src/payout_connectors/itaubank.rs` (under the
  `// ===== PAYOUT STUB FLOWS =====` banner) — it does not call
  `macro_connector_payout_implementation!` at all. Itaú SiSPAG accepts beneficiary account details
  inline on every transfer (`ItaubankRecebedor` in
  `payout_connectors/itaubank/transformers.rs`) and has no separate account-enrollment endpoint, so
  the transformers file contains no
  `PayoutEnrollDisburseAccountRequest`/`PayoutEnrollDisburseAccountResponse` `TryFrom` blocks.

### trustly — the nearest analogue that *is* implemented

- `TrustlyPayouts<T>` also lists `PayoutEnrollDisburseAccount` as a macro stub, but it implements the
  neighbouring `PayoutCreateRecipient` for real (`RegisterAccount`), returning the account handle in
  `PayoutCreateRecipientResponse.payout_connector_metadata`. If a connector's account-enrollment API
  looks like Trustly's `RegisterAccount`, implement `PayoutCreateRecipient` rather than this flow —
  it is the only one of the two whose response type can carry an opaque handle.

### Everyone else

All remaining payout connectors are in the same stub state — see the Stub Implementations list above
for which produce their stub by hand and which by macro.

## Code Examples

### 1. Stub registration via the macro (generic connectors only)

```rust
// crates/integrations/connector-integration/src/payout_connectors/deutschebank.rs
//   — under the `// ===== PAYOUT STUB FLOWS =====` banner
macros::macro_connector_payout_implementation!(
    connector: DeutschebankPayouts,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    payout_flows: [
        PayoutCreate,
        PayoutVoid,
        PayoutStage,
        PayoutCreateLink,
        PayoutCreateRecipient,
        PayoutEnrollDisburseAccount   // <-- registers the marker + a fail-fast integration impl
    ]
);
```

Six entries, not nine. `PayoutTransfer`, `PayoutGet` and `PayoutEligibility` are **absent** because
Deutsche Bank implements all three for real via `macros::macro_connector_implementation!` earlier in
the file. Listing a real flow here as well is a conflicting-implementation compile error — copy the
list from the connector you are editing, never from this page.

### 2. Marker trait definition

```rust
// From crates/types-traits/interfaces/src/connector_types.rs
pub trait PayoutEnrollDisburseAccountV2:
    ConnectorIntegrationV2<
    connector_flow::PayoutEnrollDisburseAccount,
    PayoutFlowData,
    PayoutEnrollDisburseAccountRequest,
    PayoutEnrollDisburseAccountResponse,
>
{
}
```

### 3. PayoutMethodData and Bank enums (must be exhaustively matched)

```rust
// From crates/types-traits/domain_types/src/payouts/payout_method_data.rs
pub enum PayoutMethodData {
    Card(CardPayout),
    Bank(Bank),
    Wallet(Wallet),
    BankRedirect(BankRedirect),
    Passthrough(Passthrough),
}

// From crates/types-traits/domain_types/src/payouts/payout_method_data.rs
pub enum Bank {
    Ach(AchBankTransfer),
    Bacs(BacsBankTransfer),
    Sepa(SepaBankTransfer),
    Pix(PixBankTransfer),
}
```

### 4. Reference implementation shape (adapted from `PayoutTransfer`)

```rust
// Adapted shape — see the PayoutTransfer impl in
// crates/integrations/connector-integration/src/payout_connectors/itaubank.rs
// and the PayoutCreate impl in
// crates/integrations/connector-integration/src/payout_connectors/santander.rs
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PayoutEnrollDisburseAccount,
        PayoutFlowData,
        PayoutEnrollDisburseAccountRequest,
        PayoutEnrollDisburseAccountResponse,
    > for <Connector><T>
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
    }

    fn get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn get_url(
        &self,
        req: &RouterDataV2<PayoutEnrollDisburseAccount, PayoutFlowData, PayoutEnrollDisburseAccountRequest, PayoutEnrollDisburseAccountResponse>,
    ) -> CustomResult<String, IntegrationError> {
        let base_url = self.base_url(&req.resource_common_data.connectors);
        Ok(format!("{base_url}/v1/recipients/accounts"))
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<PayoutEnrollDisburseAccount, PayoutFlowData, PayoutEnrollDisburseAccountRequest, PayoutEnrollDisburseAccountResponse>,
    ) -> CustomResult<Option<ConnectorRequestData>, IntegrationError> {
        let connector_req = <ConnectorEnrollAccountRequest>::try_from(req)?;
        Ok(Some(RequestContent::Json(Box::new(connector_req))))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<PayoutEnrollDisburseAccount, PayoutFlowData, PayoutEnrollDisburseAccountRequest, PayoutEnrollDisburseAccountResponse>,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<PayoutEnrollDisburseAccount, PayoutFlowData, PayoutEnrollDisburseAccountRequest, PayoutEnrollDisburseAccountResponse>,
        ConnectorError,
    > {
        // Parse account-id response; map to PayoutStatus::RequiresVendorAccountCreation (pending)
        // or Success (instant verify), never hardcoded.
        let response: MyConnectorEnrollResponse = res
            .response
            .parse_struct("MyConnectorEnrollResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: Default::default(),
            })?;

        finalize_connector_response!(event_builder, response, data, res.status_code)
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, connector_config)
    }
}
```

`finalize_connector_response!` (`crates/integrations/connector-integration/src/utils.rs`) builds the
`RouterDataV2` through `TryFrom<ResponseRouterData<MyConnectorEnrollResponse, …>>`, sets the event response data
and writes `typed_connector_response`. Declare that `TryFrom` in the connector's `transformers.rs`;
it is where `PayoutEnrollDisburseAccountResponse`'s fields — including `status_code`, from
`item.http_code` — get populated.

### 5. itaubank's PIX branch (shows a real-world branching pattern to mirror)

```rust
// crates/integrations/connector-integration/src/payout_connectors/itaubank/transformers.rs
//   — impl TryFrom<&RouterDataV2<PayoutTransfer, ...>> for ItaubankTransferRequest
let recebedor = match req.request.payout_method_data.clone() {
    Some(PayoutMethodData::Bank(Bank::Pix(PixBankTransfer {
        tax_id,
        bank_branch,
        bank_account_number,
        bank_name,
        ..
    }))) => {
        // ... build ItaubankRecebedor
        Some(ItaubankRecebedor { /* ... */ })
    }
    _ => None,
};
```

Note: itaubank falls through with `_ => (None, None, None)` for the `PayoutTransfer` flow because
the rail is PIX-only — it handles exactly `Bank::Pix`, `Bank::PixEmv` and `Bank::PixKey`, and the
person-type enum it fills is `ItaubankRecipientType` (`Individual` / `LegalEntity`), not
`ItaubankPersonType`. For `PayoutEnrollDisburseAccount` on a multi-rail connector, the `_` arm is
inappropriate — authors MUST list every variant explicitly (see §Payout-Method-Data Branching Pattern
above).

## Integration Guidelines

1. Confirm the connector exposes a two-step onboarding (create-recipient + enroll-account). If the connector folds these into one endpoint, implement `PayoutCreateRecipient` only and leave this flow as a stub.
2. **Remove** the existing `PayoutEnrollDisburseAccount` stub — every payout connector already has
   one. For a generic connector delete it from the `payout_flows: [...]` list; for a non-generic one
   delete the hand-written stub `impl`. Leaving both is a conflicting-implementation compile error.
3. Write `impl PayoutEnrollDisburseAccountV2 for <Name>Payouts {}` plus the real
   `impl ConnectorIntegrationV2<PayoutEnrollDisburseAccount, PayoutFlowData,
   PayoutEnrollDisburseAccountRequest, PayoutEnrollDisburseAccountResponse>` block (longhand for a
   non-generic connector; `macros::macro_connector_implementation!` with
   `flow_name: PayoutEnrollDisburseAccount` and `resource_common_data: PayoutFlowData` for a generic
   one). `get_request_body` returns `Option<ConnectorRequestData>`, and `handle_response_v2` should
   delegate to `finalize_connector_response!`.
4. In `payout_connectors/<connector>/transformers.rs`:
   - Add a `TryFrom<&RouterDataV2<PayoutEnrollDisburseAccount, ...>>` impl.
   - Match every variant of `pub enum PayoutMethodData` in
     `crates/types-traits/domain_types/src/payouts/payout_method_data.rs` — five variants, no
     wildcards.
   - Within the `Bank` arm, match every variant of `pub enum Bank` in the same file — **ten**
     variants, no wildcards.
   - For any unsupported variant, emit `IntegrationError::NotSupported { message, connector, context }`
     (`crates/types-traits/domain_types/src/errors.rs`) naming the rail.
5. Add response parsing that lifts the enrolled account id into `connector_payout_id` and maps the connector's verification state to `PayoutStatus::RequiresVendorAccountCreation` (pending) or `PayoutStatus::Success` (ready).
6. Propagate the enrolled account id back to the router so subsequent `PayoutCreate`/`PayoutTransfer` calls can reference it via `connector_payout_method_id` at `crates/types-traits/domain_types/src/payouts/payouts_types.rs`.
7. Write unit tests for each supported `payout_method_data` variant.
8. Write an integration test chain alongside
   `crates/grpc-server/grpc-server/tests/payout_flows_test.rs`:
   `PayoutCreateRecipient` → `PayoutEnrollDisburseAccount` → `PayoutTransfer`.
9. **Register the connector at all six sites** listed in the scope note above (the four shared wiring sites below plus the connector file itself and its `payout_connectors.rs` export), and add
   **no** `config/superposition.toml` entry and **no** `connector_specs/<connector>/specs.json`
   entry — `PayoutService` is in `IGNORE_SERVICES` in both
   `crates/internal/integration-tests/src/bin/check_connector_specs.rs` and `check_coverage.rs`.

## Best Practices

- Match every variant of `PayoutMethodData` (enum at `crates/types-traits/domain_types/src/payouts/payout_method_data.rs`) and every variant of `Bank` (enum at `crates/types-traits/domain_types/src/payouts/payout_method_data.rs`). Use explicit arms, not wildcards, per §11 of `PATTERN_AUTHORING_SPEC.md`.
- When the connector's enrollment is asynchronous, map to `PayoutStatus::RequiresVendorAccountCreation` (variant at `crates/common/common_enums/src/enums.rs`). Only use `Success` if the connector explicitly returns a verified state.
- Lift the enrolled account id into `connector_payout_id`. Downstream `PayoutCreate` consumes it via `PayoutCreateRequest.connector_payout_method_id` at `crates/types-traits/domain_types/src/payouts/payouts_types.rs`.
- Return `ConnectorCommon::build_error_response` from `get_error_response_v2`, exactly as the `PayoutTransfer` impl in `crates/integrations/connector-integration/src/payout_connectors/itaubank.rs` does, for connector-side enrollment errors (e.g. "invalid routing number").
- See upstream pattern [pattern_payout_create_recipient.md](./pattern_payout_create_recipient.md) for the recipient-profile step that must precede this flow in most connector APIs.

## Common Errors / Gotchas

1. **Problem:** Rust compile error "non-exhaustive patterns: `Some(PayoutMethodData::Wallet(_))` not covered" when matching `payout_method_data`.
   **Solution:** Five top-level variants — `Card`, `Bank`, `Wallet`, `BankRedirect`, `Passthrough` — on `pub enum PayoutMethodData` in `crates/types-traits/domain_types/src/payouts/payout_method_data.rs`. The `Bank` arm has **ten** sub-variants (`Ach`, `Bacs`, `Sepa`, `Pix`, `PixKey`, `PixEmv`, `OpenBanking`, `Trustly`, `Payshap`, `PayshapProxy`) and `BankRedirect` has two (`Interac`, `OpenBankingUk`). Enumerate all explicitly.

2. **Problem:** Wildcard `_ => None` silently drops a supported rail on refactor.
   **Solution:** Do not use wildcards in `payout_method_data` branches for this flow. The `_ => (None, None, None)` arm in `ItaubankTransferRequest::try_from` (`payout_connectors/itaubank/transformers.rs`) is acceptable for that connector because Itaú is a PIX-only integration; a multi-rail enroll flow MUST NOT copy that pattern.

3. **Problem:** Enrolled account returns `connector_payout_id = None` because the transformer forgot to lift the id.
   **Solution:** The field is `Option<String>` at `crates/types-traits/domain_types/src/payouts/payouts_types.rs` and is the primary handle the router persists. Returning `None` on a success path silently breaks downstream `PayoutCreate`.

4. **Problem:** `payout_status = PayoutStatus::Success` for an async-verification connector.
   **Solution:** Map to `PayoutStatus::RequiresVendorAccountCreation` at `crates/common/common_enums/src/enums.rs` when the connector indicates pending verification. Promote to `Success` only via subsequent webhook or `PayoutGet` polling.

5. **Problem:** Compile error "conflicting implementations of trait `ConnectorIntegrationV2<PayoutEnrollDisburseAccount, ...>`".
   **Solution:** Remove `PayoutEnrollDisburseAccount` from the `payout_flows:` macro list when writing the full impl. Macro recursion at `crates/integrations/connector-integration/src/connectors/macros.rs`.

6. **Problem:** Enrollment succeeded but downstream `PayoutCreate` includes inline beneficiary details AND `connector_payout_method_id`, causing the connector to reject with "duplicate account details".
   **Solution:** In the `PayoutCreate`/`PayoutTransfer` transformers, when
   `connector_payout_method_id` is `Some`, omit inline beneficiary serialization and reference the id
   only. The field is `connector_payout_method_id: Option<String>` on both
   `pub struct PayoutCreateRequest` and `pub struct PayoutTransferRequest` in
   `crates/types-traits/domain_types/src/payouts/payouts_types.rs`. If the connector's handle is
   opaque rather than a plain id, implement `PayoutCreateRecipient` instead — it is the only
   neighbouring flow whose response carries `payout_connector_metadata`.

7. **Problem:** Compile error "conflicting implementations of trait
   `ConnectorIntegrationV2<PayoutEnrollDisburseAccount, ...>`".
   **Solution:** A stub already exists on every payout connector. Remove
   `PayoutEnrollDisburseAccount` from the `payout_flows: [...]` list (generic connector) or delete
   the hand-written stub `impl` (non-generic connector) before writing the real one.

8. **Problem:** `get_request_body` written to return `Option<RequestContent>`.
   **Solution:** The signature is
   `CustomResult<Option<ConnectorRequestData>, IntegrationError>`
   (`crates/types-traits/interfaces/src/connector_integration_v2.rs`); wrap with
   `ConnectorRequestData::new(RequestContent::Json(Box::new(req)), typed)`.

9. **Problem:** Adding `impl ValidationTrait for <Name>Payouts` to force an access-token exchange.
   **Solution:** `PayoutServiceTrait` does not require `ValidationTrait`, and nothing reads
   `should_do_access_token` on the payout path. The token arrives on the RPC's
   `optional SecretString access_token` and is read with
   `req.resource_common_data.get_access_token()`.

## Testing Notes

### Unit Tests

Each connector implementing PayoutEnrollDisburseAccount should cover:

- `TryFrom<&RouterDataV2<PayoutEnrollDisburseAccount, ...>>` for each supported `PayoutMethodData` variant the connector accepts (Card, Bank::Ach, Bank::Sepa, Bank::Pix, Wallet, Passthrough as applicable).
- Each unsupported variant — expect `IntegrationError::NotSupported { message, connector, context }` (errors.rs) with a clear field name.
- `payout_method_data = None` — expect `IntegrationError::MissingRequiredField { field_name: "payout_method_data", .. }`.
- Response parsing: pending-verification → `PayoutStatus::RequiresVendorAccountCreation`; instant-verified → `PayoutStatus::Success`; rejected → `PayoutStatus::Failure`.

### Integration Scenarios

| Scenario | Inputs | Expected `payout_status` | Expected `status_code` |
| --- | --- | --- | --- |
| Enroll ACH bank account | Bank(Ach) with valid routing | `RequiresVendorAccountCreation` | 201 |
| Enroll SEPA account | Bank(Sepa) with valid IBAN | `RequiresVendorAccountCreation` | 201 |
| Enroll wallet | Wallet(Paypal { email: ... }) | `RequiresVendorAccountCreation` | 201 |
| Enroll with no payout_method_data | None | — (request-time error) | N/A |
| Invalid routing number | Bank(Ach) with 8-digit routing | `Failure` | 4xx |
| Enroll → PayoutCreate chain | chained | `Success` on create | 200 |

No payout connector exercises these scenarios today. There is no `connector_specs` suite for
payouts either — `crates/internal/integration-tests/src/bin/check_connector_specs.rs` enumerates
connectors from `connectors/` only, never `payout_connectors/`, and `PayoutService` is in its
`IGNORE_SERVICES` (and `check_coverage.rs`'s), so this flow sits outside the merge-blocking
certification gate. The in-repo gRPC-level harness is
`crates/grpc-server/grpc-server/tests/payout_flows_test.rs`.

## Cross-References

- Parent index: [../README.md](./README.md)
- Registry: `crates/integrations/connector-integration/src/payout_connectors.rs`
- Closest full-implementation templates: `payout_connectors/trustly.rs` (`PayoutCreateRecipient`),
  `payout_connectors/santander.rs` (`PayoutCreate`), `payout_connectors/itaubank.rs`
  (`PayoutTransfer`, `PayoutGet`)
- Sibling core payout flow: [pattern_payout_create.md](./pattern_payout_create.md)
- Sibling core payout flow: [pattern_payout_transfer.md](./pattern_payout_transfer.md)
- Sibling core payout flow: [pattern_payout_get.md](./pattern_payout_get.md)
- Sibling side-flow: [pattern_payout_create_recipient.md](./pattern_payout_create_recipient.md) — immediate upstream
- Sibling side-flow: [pattern_payout_create_link.md](./pattern_payout_create_link.md)
- Sibling side-flow: [pattern_payout_void.md](./pattern_payout_void.md)
- Sibling side-flow: [pattern_payout_stage.md](./pattern_payout_stage.md)
- Macro reference: [macro_patterns_reference.md](./macro_patterns_reference.md)
- Utility helpers: [utility_functions_reference.md](../utility_functions_reference.md)
- Authoring spec: [PATTERN_AUTHORING_SPEC.md](./PATTERN_AUTHORING_SPEC.md)
