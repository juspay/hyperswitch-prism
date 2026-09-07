# Payout Create-Link Flow Pattern

## Overview

The Payout Create-Link flow asks the connector to generate a hosted or pay-by-link URL that the beneficiary can open to claim funds (typically for Interac e-Transfer style rails, Open Banking UK pull-payouts, or PayPal/Venmo send-to-email flows). Unlike `PayoutCreate`, which commits funds to a known beneficiary account, this flow defers beneficiary collection to the connector's hosted page. The response typically contains a connector-side payout id plus, out-of-band, a hosted URL that must be surfaced to the merchant.

Note on current type shape: `pub struct PayoutCreateLinkResponse` in
`crates/types-traits/domain_types/src/payouts/payouts_types.rs` has four fields
(`merchant_payout_id`, `payout_status`, `connector_payout_id`, `status_code`) and **no** URL field —
and, unlike `PayoutCreateRecipientResponse` and `PayoutEligibilityResponse`, no metadata field
either. Connectors that implement this flow must either (a) park the URL inside `connector_payout_id`
with a documented format, or (b) surface it via `raw_connector_response` / `typed_connector_response`
on `PayoutFlowData` (both written by the `RawConnectorRequestResponse` impl in the same file). This
is a current limitation — a future addition of `payout_link_url: Option<String>`, or of a
`payout_connector_metadata` field matching the one on `PayoutCreateRecipientResponse`, would fix it
properly.

### Key Components

- Flow marker: `PayoutCreateLink` — `crates/types-traits/domain_types/src/connector_flow.rs`.
- Request type: `PayoutCreateLinkRequest` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`.
- Response type: `PayoutCreateLinkResponse` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`.
- Flow-data type: `PayoutFlowData` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`.
- Marker trait: `PayoutCreateLinkV2` — `crates/types-traits/interfaces/src/connector_types.rs`, defined solely as the supertrait binding:

  ```rust
  // crates/types-traits/interfaces/src/connector_types.rs — pub trait PayoutCreateLinkV2
  pub trait PayoutCreateLinkV2:
      ConnectorIntegrationV2<
      connector_flow::PayoutCreateLink,
      PayoutFlowData,
      PayoutCreateLinkRequest,
      PayoutCreateLinkResponse,
  >
  {
  }
  ```

- Service trait: `PayoutServiceTrait` — `crates/types-traits/interfaces/src/connector_types.rs`.
- Stub macro arm: the `flow: PayoutCreateLink` arm of `expand_payout_implementation!` — `crates/integrations/connector-integration/src/connectors/macros.rs`.

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

Payout Create-Link is a producer flow: it creates state on the connector side (a pending payout linked to a hosted page) and returns an identifier. It is the alternative to `PayoutCreate` for connectors that prefer hosted beneficiary collection.

### Flow Hierarchy

```
PayoutStage  (optional quote lock; upstream)
        |
        v
PayoutCreateLink  (this flow — generates hosted URL)
        |
        v
<merchant opens URL out-of-band>
        |
        v
PayoutGet  (poll for beneficiary-claim completion)
```

### Flow Type

`PayoutCreateLink` — zero-sized marker struct declared at `crates/types-traits/domain_types/src/connector_flow.rs`. Registered in `FlowName::PayoutCreateLink` at `crates/types-traits/domain_types/src/connector_flow.rs`.

### Request Type

`PayoutCreateLinkRequest` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`:

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs
#[derive(Debug, Clone)]
pub struct PayoutCreateLinkRequest {
    pub merchant_payout_id: Option<String>,
    pub connector_quote_id: Option<String>,
    pub connector_payout_id: Option<String>,
    pub amount: common_utils::types::MinorUnit,
    pub source_currency: common_enums::Currency,
    pub destination_currency: common_enums::Currency,
    pub priority: Option<common_enums::PayoutPriority>,
    pub connector_payout_method_id: Option<String>,
    pub webhook_url: Option<String>,
    pub payout_method_data: Option<PayoutMethodData>,
}
```

Ten fields: exactly `PayoutCreateRequest`'s eleven **minus `source_bank_data`** (`crates/types-traits/domain_types/src/payouts/payouts_types.rs`). The two are otherwise field-for-field identical, so do not copy a `PayoutCreate` transformer wholesale — a create-link body cannot read a debtor account. The semantic difference is the downstream connector behavior: create-link yields a redirect URL while create yields a terminal payout object. `PayoutMethodData` is the enum at `crates/types-traits/domain_types/src/payouts/payout_method_data.rs`.

### Response Type

`PayoutCreateLinkResponse` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`:

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs
#[derive(Debug, Clone)]
pub struct PayoutCreateLinkResponse {
    pub merchant_payout_id: Option<String>,
    pub payout_status: common_enums::PayoutStatus,
    pub connector_payout_id: Option<String>,
    pub status_code: u16,
}
```

The response is structurally identical to `PayoutCreateResponse` at `payouts_types.rs`. A successful link generation should map to `PayoutStatus::RequiresFulfillment` (variant at `crates/common/common_enums/src/enums.rs`) to signal "link exists, awaiting beneficiary action".

### Resource Common Data

`PayoutFlowData` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`. See [pattern_payout_void.md](./pattern_payout_void.md) for the full field-by-field breakdown.

### RouterDataV2 Shape

```rust
RouterDataV2<PayoutCreateLink, PayoutFlowData, PayoutCreateLinkRequest, PayoutCreateLinkResponse>
```

Canonical four-arg shape per §7 of `PATTERN_AUTHORING_SPEC.md`.

## Connectors with Full Implementation

**None.** All ten payout connectors register `PayoutCreateLinkV2` with a fail-fast stub. Hosted payout-link pages are rare: no payout connector in the tree exposes one.

Verify before trusting this:

```bash
rg -n 'ConnectorIntegrationV2<\s*PayoutCreateLink|flow_name: PayoutCreateLink' \
   crates/integrations/connector-integration/src/payout_connectors/
```

Every hit is a stub whose only method is a `get_url` returning
`IntegrationError::connector_flow_not_implemented(self.id(), "payout_create_link", IntegrationErrorContext::default())`.

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
  `impl PayoutCreateLinkV2 for <Name>Payouts {}` plus a one-method
  `ConnectorIntegrationV2<PayoutCreateLink, PayoutFlowData, PayoutCreateLinkRequest, PayoutCreateLinkResponse>` block longhand;
  `payout_connectors/truelayer.rs` generates the same shape from its file-local
  `macro_rules! impl_unimplemented_payout_flow!`.
- **Macro** — the three generic payout connectors list `PayoutCreateLink` in the `payout_flows: [...]`
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
        PayoutCreateLink,  // <-- registers PayoutCreateLinkV2 + empty ConnectorIntegrationV2 impl
        PayoutCreateRecipient,
        PayoutEnrollDisburseAccount
    ]
);
```

The `PayoutCreateLink` arm at `crates/integrations/connector-integration/src/connectors/macros.rs` produces:

```rust
// From crates/integrations/connector-integration/src/connectors/macros.rs
(
    connector: $connector: ident,
    flow: PayoutCreateLink,
    generic_type: $generic_type:tt,
    [ $($bounds:tt)* ]
) => {
    impl<$generic_type: $($bounds)*> ::interfaces::connector_types::PayoutCreateLinkV2 for $connector<$generic_type> {}
    impl<$generic_type: $($bounds)*>
        ::interfaces::connector_integration_v2::ConnectorIntegrationV2<
            ::domain_types::connector_flow::PayoutCreateLink,
            ::domain_types::payouts::payouts_types::PayoutFlowData,
            ::domain_types::payouts::payouts_types::PayoutCreateLinkRequest,
            ::domain_types::payouts::payouts_types::PayoutCreateLinkResponse,
        > for $connector<$generic_type>
    {
        fn get_url(
            &self,
            _req: &::domain_types::router_data_v2::RouterDataV2<
                ::domain_types::connector_flow::PayoutCreateLink,
                ::domain_types::payouts::payouts_types::PayoutFlowData,
                ::domain_types::payouts::payouts_types::PayoutCreateLinkRequest,
                ::domain_types::payouts::payouts_types::PayoutCreateLinkResponse,
            >,
        ) -> ::common_utils::CustomResult<String, ::domain_types::errors::IntegrationError> {
            Err(::domain_types::errors::IntegrationError::connector_flow_not_implemented(
                ::interfaces::api::ConnectorCommon::id(self),
                "payout_create_link",
                ::domain_types::errors::IntegrationErrorContext::default(),
            ).into())
        }
    }
};
```

The body is **not** empty: `get_url` is overridden so the stub fails fast with
`IntegrationError::connector_flow_not_implemented(id, "payout_create_link", …)` rather than falling through to a
`ConnectorIntegrationV2` trait default. And the emitted target is `$connector<$generic_type>` — the
macro matches only a **generic** connector struct.

### Moving from stub to real

A flow is stub **or** real, never both; two impls of the same
`ConnectorIntegrationV2<PayoutCreateLink, …>` will not compile.

- **Generic connector** (`DeutschebankPayouts<T>`, `GotymeSanlamPayouts<T>`, `TrustlyPayouts<T>`):
  delete `PayoutCreateLink` from `payout_flows: [...]`, add a matching
  `(flow: PayoutCreateLink, request_body: …, response_body: …, router_data: RouterDataV2<PayoutCreateLink,
  PayoutFlowData, PayoutCreateLinkRequest, PayoutCreateLinkResponse>)` entry to
  `macros::create_all_prerequisites!`, write `impl PayoutCreateLinkV2 for <Name>Payouts<T> {}`, and add a
  `macros::macro_connector_implementation!` call with `flow_name: PayoutCreateLink` and
  `resource_common_data: PayoutFlowData`. `payout_connectors/trustly.rs` does exactly this for
  `PayoutCreateRecipient`, `PayoutTransfer` and `PayoutGet`.
- **Non-generic connector** (the other seven): delete the hand-written stub `impl` and replace it
  with a longhand `impl ConnectorIntegrationV2<PayoutCreateLink, PayoutFlowData, PayoutCreateLinkRequest, PayoutCreateLinkResponse> for
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


### Hosted-Link URL-Surfacing Strategies

Because `PayoutCreateLinkResponse` carries neither a URL field nor a metadata field, connectors that implement this flow must pick one:

1. **Raw-response capture** — let the platform capture the full response body into `PayoutFlowData.raw_connector_response` (the `RawConnectorRequestResponse` trait impl at `crates/types-traits/domain_types/src/payouts/payouts_types.rs` does this automatically). Callers parse the URL out of `raw_connector_response`.
2. **Composite id** — concatenate `id|url` into `connector_payout_id` with a documented separator. Not recommended because downstream `PayoutGet` expects a clean id.
3. **Merchant webhook** — rely on the connector to push the hosted URL to the merchant via its own webhook rather than returning it synchronously.

Strategy (1) is the cleanest given the current type shape. Strategy (2) is a compatibility hazard and must be flagged in PR review.

## Connector-Specific Patterns

### itaubank

- `ItaubankPayouts` is a non-generic unit struct and carries a **hand-written** `PayoutCreateLink`
  stub in `crates/integrations/connector-integration/src/payout_connectors/itaubank.rs` (under the
  `// ===== PAYOUT STUB FLOWS =====` banner) — it does not call
  `macro_connector_payout_implementation!` at all. Itaú's SiSPAG product is a direct-credit rail with
  no hosted payout-link page, so `payout_connectors/itaubank/transformers.rs` contains no
  `PayoutCreateLinkRequest`/`PayoutCreateLinkResponse` `TryFrom` blocks.

### Everyone else

All nine remaining payout connectors are in the same state — see the Stub Implementations list above
for which produce their stub by hand and which by macro. Note that `payout_connectors/trustly.rs`
lists `PayoutCreateLink` in its `payout_flows: [PayoutCreate, PayoutVoid, PayoutStage,
PayoutCreateLink, PayoutEnrollDisburseAccount]`, so the macro stub covers it there.

## Code Examples

### 1. Stub registration via the macro (generic connectors only)

```rust
// crates/integrations/connector-integration/src/payout_connectors/trustly.rs
//   — under the `// ===== PAYOUT STUB FLOWS (not supported by Trustly) =====` banner
macros::macro_connector_payout_implementation!(
    connector: TrustlyPayouts,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    payout_flows: [
        PayoutCreate,
        PayoutVoid,
        PayoutStage,
        PayoutCreateLink,             // <-- registers the marker + a fail-fast integration impl
        PayoutEnrollDisburseAccount
    ]
);
```

Five entries, not nine. `PayoutCreateRecipient`, `PayoutTransfer` and `PayoutGet` are **absent**
because each has its own `macros::macro_connector_implementation!` call earlier in the file, and
`PayoutEligibility` is absent because `trustly.rs` writes that one stub by hand. Listing a real flow
here as well is a conflicting-implementation compile error — copy the list from the connector you are
editing, never from this page.

### 2. Marker trait definition

```rust
// From crates/types-traits/interfaces/src/connector_types.rs
pub trait PayoutCreateLinkV2:
    ConnectorIntegrationV2<
    connector_flow::PayoutCreateLink,
    PayoutFlowData,
    PayoutCreateLinkRequest,
    PayoutCreateLinkResponse,
>
{
}
```

### 3. Request type

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs
#[derive(Debug, Clone)]
pub struct PayoutCreateLinkRequest {
    pub merchant_payout_id: Option<String>,
    pub connector_quote_id: Option<String>,
    pub connector_payout_id: Option<String>,
    pub amount: common_utils::types::MinorUnit,
    pub source_currency: common_enums::Currency,
    pub destination_currency: common_enums::Currency,
    pub priority: Option<common_enums::PayoutPriority>,
    pub connector_payout_method_id: Option<String>,
    pub webhook_url: Option<String>,
    pub payout_method_data: Option<PayoutMethodData>,
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
        PayoutCreateLink,
        PayoutFlowData,
        PayoutCreateLinkRequest,
        PayoutCreateLinkResponse,
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
        req: &RouterDataV2<PayoutCreateLink, PayoutFlowData, PayoutCreateLinkRequest, PayoutCreateLinkResponse>,
    ) -> CustomResult<String, IntegrationError> {
        let base_url = self.base_url(&req.resource_common_data.connectors);
        Ok(format!("{base_url}/v1/payout-links"))
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<PayoutCreateLink, PayoutFlowData, PayoutCreateLinkRequest, PayoutCreateLinkResponse>,
    ) -> CustomResult<Option<ConnectorRequestData>, IntegrationError> {
        let connector_req = <ConnectorLinkRequest>::try_from(req)?;
        Ok(Some(RequestContent::Json(Box::new(connector_req))))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<PayoutCreateLink, PayoutFlowData, PayoutCreateLinkRequest, PayoutCreateLinkResponse>,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<PayoutCreateLink, PayoutFlowData, PayoutCreateLinkRequest, PayoutCreateLinkResponse>,
        ConnectorError,
    > {
        // Parse link response; the platform retains the raw body on PayoutFlowData.raw_connector_response.
        let response: MyConnectorLinkResponse = res
            .response
            .parse_struct("MyConnectorLinkResponse")
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
`RouterDataV2` through `TryFrom<ResponseRouterData<MyConnectorLinkResponse, …>>`, sets the event response data
and writes `typed_connector_response`. Declare that `TryFrom` in the connector's `transformers.rs`;
it is where `PayoutCreateLinkResponse`'s fields — including `status_code`, from
`item.http_code` — get populated.

### 5. Amount-conversion note

`PayoutCreateLinkRequest.amount` is `common_utils::types::MinorUnit` (see
`pub struct PayoutCreateLinkRequest` in
`crates/types-traits/domain_types/src/payouts/payouts_types.rs` — ten fields, exactly
`PayoutCreateRequest`'s eleven minus `source_bank_data`). Convert with the appropriate converter from `common_utils::types`; both in-tree payout
examples use `StringMajorUnitForConnector` — `ItaubankTransferRequest::try_from` in
`payout_connectors/itaubank/transformers.rs` and `SantanderCreateRequest::try_from` in
`payout_connectors/santander/transformers.rs`.

## Integration Guidelines

1. Confirm the connector exposes a hosted-link endpoint. Many payout APIs do NOT — transfer + get is
   the common shape in this tree. If unsupported, leave the flow as a registered stub.
2. **Remove** the existing `PayoutCreateLink` stub — every payout connector already has one. For a generic connector delete `PayoutCreateLink` from the `payout_flows: [...]` list; for a non-generic one delete the hand-written stub `impl`. Leaving both is a conflicting-implementation compile error.
3. Write `impl PayoutCreateLinkV2 for <Name>Payouts {}` plus the real
   `impl ConnectorIntegrationV2<PayoutCreateLink, PayoutFlowData, PayoutCreateLinkRequest, PayoutCreateLinkResponse>`
   block (longhand for a non-generic connector; `macros::macro_connector_implementation!` with
   `flow_name: PayoutCreateLink` and `resource_common_data: PayoutFlowData` for a generic one).
4. Decide how to surface the hosted URL: prefer relying on `PayoutFlowData.raw_connector_response` (auto-captured via the `RawConnectorRequestResponse` impl at `crates/types-traits/domain_types/src/payouts/payouts_types.rs`) rather than overloading `connector_payout_id`.
5. Map the success status to `PayoutStatus::RequiresFulfillment` (variant at `crates/common/common_enums/src/enums.rs`). Do NOT use `PayoutStatus::Success` — the beneficiary has not yet acted.
6. Pipe `webhook_url` (on `pub struct PayoutCreateLinkRequest` in
   `crates/types-traits/domain_types/src/payouts/payouts_types.rs`) into the connector's
   webhook-callback field if exposed, so the beneficiary-claim event lands back on the caller.
7. Write unit tests covering: link creation success, unsupported currency, link creation with `payout_method_data = None` (if the connector allows it), and malformed `webhook_url`.
8. Write an integration test alongside `crates/grpc-server/grpc-server/tests/payout_flows_test.rs`
   that creates a link, polls via `PayoutGet`, and asserts eventual `Success`.
9. **Register the connector at all six sites** listed in the scope note above (the four shared wiring sites below plus the connector file itself and its `payout_connectors.rs` export), and add
   **no** `config/superposition.toml` entry and **no** `connector_specs/<connector>/specs.json`
   entry — `PayoutService` is in `IGNORE_SERVICES` in both
   `crates/internal/integration-tests/src/bin/check_connector_specs.rs` and `check_coverage.rs`.

## Best Practices

- Reuse the `PayoutCreateRequest` transformer if the connector's link and create endpoints accept the
  same body shape. `PayoutCreateLinkRequest` is `PayoutCreateRequest` minus `source_bank_data` — ten
  fields against eleven — so a shared builder only works when the connector does not need the debtor
  account. Read both `pub struct PayoutCreateLinkRequest` and `pub struct PayoutCreateRequest` in
  `crates/types-traits/domain_types/src/payouts/payouts_types.rs` before sharing code.
- Always propagate `webhook_url` — links without a webhook force long-polling via `PayoutGet`. The field is `webhook_url: Option<String>` on `pub struct PayoutCreateLinkRequest` in `crates/types-traits/domain_types/src/payouts/payouts_types.rs`.
- Surface the hosted URL via `raw_connector_response` rather than overloading `connector_payout_id`. The raw-capture mechanism is wired on the flow-data type at `crates/types-traits/domain_types/src/payouts/payouts_types.rs`.
- See sibling flow [pattern_payout_create.md](./pattern_payout_create.md) for the direct-credit alternative that doesn't require a hosted page.

## Common Errors / Gotchas

1. **Problem:** Beneficiary never claims the link and `payout_status` stays `RequiresFulfillment` forever.
   **Solution:** Expected. The router polls via `PayoutGet`; connector-side link-expiry events surface via the connector webhook (wired through `webhook_url` at `crates/types-traits/domain_types/src/payouts/payouts_types.rs`). Cross-ref [pattern_payout_get.md](./pattern_payout_get.md).

2. **Problem:** `connector_payout_id` stuffed with `"id|https://..."` format breaks downstream `PayoutGet` URL construction.
   **Solution:** Stash the URL in `raw_connector_response` instead. Do not overload `connector_payout_id`.

3. **Problem:** `PayoutStatus::Success` mapped to successful link creation.
   **Solution:** A created link is `RequiresFulfillment`, not `Success`. Variants listed at `crates/common/common_enums/src/enums.rs`.

4. **Problem:** Compile error "conflicting implementations of trait `ConnectorIntegrationV2<PayoutCreateLink, ...>`".
   **Solution:** A stub already exists on every payout connector. Remove `PayoutCreateLink` from the
   `payout_flows: [...]` list (generic connector) or delete the hand-written stub `impl`
   (non-generic connector) before writing the real one. Rust has no specialization here.

4a. **Problem:** `macro_connector_payout_implementation!` does not expand.
   **Solution:** It matches only `$connector<$generic_type>`. Seven of the ten payout connectors are
   non-generic unit structs (`pub struct ItaubankPayouts;`) and must hand-write their stubs — or use
   a file-local helper macro, as `payout_connectors/truelayer.rs` does with
   `macro_rules! impl_unimplemented_payout_flow!`.

4b. **Problem:** `get_request_body` written to return `Option<RequestContent>`.
   **Solution:** The signature is
   `CustomResult<Option<ConnectorRequestData>, IntegrationError>`
   (`crates/types-traits/interfaces/src/connector_integration_v2.rs`); wrap with
   `ConnectorRequestData::new(RequestContent::Json(Box::new(req)), typed)`.

5. **Problem:** `payout_method_data = None` at the router but the connector requires it for the link page template (e.g. to pre-fill beneficiary email).
   **Solution:** Return `IntegrationError::MissingRequiredField { field_name: "payout_method_data", .. }` in `get_request_body` so the router surfaces a 4xx instead of silently failing downstream. `IntegrationError` variants at `crates/types-traits/domain_types/src/errors.rs` onward.

## Testing Notes

### Unit Tests

Each connector implementing PayoutCreateLink should cover:

- `TryFrom<&RouterDataV2<PayoutCreateLink, ...>>` with a complete request — asserts URL and body.
- `TryFrom<&RouterDataV2<PayoutCreateLink, ...>>` with `webhook_url = Some(...)` — asserts the webhook URL propagates into the connector body.
- Response parsing for a successful link creation → `PayoutStatus::RequiresFulfillment` with `connector_payout_id` populated.
- Error path — connector rejects with "unsupported currency" → `ErrorResponse`.

### Integration Scenarios

| Scenario | Inputs | Expected `payout_status` | Expected `status_code` |
| --- | --- | --- | --- |
| Create link for domestic transfer | amount=10000, USD→USD, Interac | `RequiresFulfillment` | 200 |
| Create link with webhook | amount=10000, USD→USD, webhook_url=Some(...) | `RequiresFulfillment` | 200 |
| Create link, unsupported pair | amount=10000, USD→ZMW | — (error) | 4xx |
| Beneficiary claims link (downstream) | poll via PayoutGet | `Success` | 200 |

No payout connector exercises these scenarios today. There is no `connector_specs` suite for
payouts either — `crates/internal/integration-tests/src/bin/check_connector_specs.rs` enumerates
connectors from `connectors/` only, never `payout_connectors/`, and `PayoutService` is in its
`IGNORE_SERVICES` (and `check_coverage.rs`'s), so this flow sits outside the merge-blocking
certification gate. The in-repo gRPC-level harness is
`crates/grpc-server/grpc-server/tests/payout_flows_test.rs`.

## Cross-References

- Parent index: [../README.md](./README.md)
- Registry: `crates/integrations/connector-integration/src/payout_connectors.rs`
- Closest full-implementation templates: `payout_connectors/santander.rs` (`PayoutCreate`),
  `payout_connectors/itaubank.rs` (`PayoutTransfer`, `PayoutGet`)
- Sibling core payout flow: [pattern_payout_create.md](./pattern_payout_create.md)
- Sibling core payout flow: [pattern_payout_transfer.md](./pattern_payout_transfer.md)
- Sibling core payout flow: [pattern_payout_get.md](./pattern_payout_get.md)
- Sibling side-flow: [pattern_payout_void.md](./pattern_payout_void.md)
- Sibling side-flow: [pattern_payout_stage.md](./pattern_payout_stage.md)
- Sibling side-flow: [pattern_payout_create_recipient.md](./pattern_payout_create_recipient.md)
- Macro reference: [macro_patterns_reference.md](./macro_patterns_reference.md)
- Utility helpers: [utility_functions_reference.md](../utility_functions_reference.md)
- Authoring spec: [PATTERN_AUTHORING_SPEC.md](./PATTERN_AUTHORING_SPEC.md)
