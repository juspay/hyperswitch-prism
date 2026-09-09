# Payout Stage Flow Pattern

## Overview

The Payout Stage flow requests a quote/rate lock from the connector prior to creating or transferring a payout. It is the "price discovery" step in multi-currency or cross-border payout APIs where the connector must first return a quote id (and sometimes a destination amount) before the merchant commits the funds. The quote id flows forward into `PayoutCreate` or `PayoutTransfer` via `connector_quote_id` — the field exists on both `pub struct PayoutCreateRequest` and `pub struct PayoutTransferRequest` in `crates/types-traits/domain_types/src/payouts/payouts_types.rs`. Staging is idempotent and non-binding — staged quotes may expire on the connector side before being consumed.

### Key Components

- Flow marker: `PayoutStage` — `crates/types-traits/domain_types/src/connector_flow.rs`.
- Request type: `PayoutStageRequest` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`.
- Response type: `PayoutStageResponse` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`.
- Flow-data type: `PayoutFlowData` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`.
- Marker trait: `PayoutStageV2` — `crates/types-traits/interfaces/src/connector_types.rs`, defined solely as the supertrait binding:

  ```rust
  // crates/types-traits/interfaces/src/connector_types.rs — pub trait PayoutStageV2
  pub trait PayoutStageV2:
      ConnectorIntegrationV2<
      connector_flow::PayoutStage,
      PayoutFlowData,
      PayoutStageRequest,
      PayoutStageResponse,
  >
  {
  }
  ```

- Service trait: `PayoutServiceTrait` — `crates/types-traits/interfaces/src/connector_types.rs`.
- Stub macro arm: the `flow: PayoutStage` arm of `expand_payout_implementation!` — `crates/integrations/connector-integration/src/connectors/macros.rs`.

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

Payout Stage differs from most payout flows by not requiring a pre-existing connector-side payout object. Unlike `PayoutVoid` or `PayoutGet`, it is invoked at the start of the payout lifecycle with only `amount`, `source_currency`, `destination_currency`, and an optional `merchant_quote_id`.

### Flow Hierarchy

```
PayoutStage  (this flow — the quote id lands in connector_payout_id;
              PayoutStageResponse has no connector_quote_id field)
        |
        v
PayoutCreate  (downstream — consumes quote via connector_quote_id)
        |
        v
PayoutTransfer  (downstream — consumes connector_payout_id)
        |
        v
PayoutGet  (verification)
```

### Flow Type

`PayoutStage` — zero-sized marker struct declared at `crates/types-traits/domain_types/src/connector_flow.rs`. Registered in `FlowName::PayoutStage` at `crates/types-traits/domain_types/src/connector_flow.rs`.

### Request Type

`PayoutStageRequest` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`:

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs
#[derive(Debug, Clone)]
pub struct PayoutStageRequest {
    pub merchant_quote_id: Option<String>,
    pub amount: common_utils::types::MinorUnit,
    pub source_currency: common_enums::Currency,
    pub destination_currency: common_enums::Currency,
}
```

Note: `PayoutStageRequest` is the narrowest payout request in the tree — four fields, and no `payout_method_data`, so connectors cannot branch on beneficiary rails when quoting. This is by design: staging is intended to return an indicative rate only. (`PayoutVoidRequest` and `PayoutGetRequest` are the only other requests with fewer than five fields, at two and three respectively.)

### Response Type

`PayoutStageResponse` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`:

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs
#[derive(Debug, Clone)]
pub struct PayoutStageResponse {
    pub merchant_payout_id: Option<String>,
    pub payout_status: common_enums::PayoutStatus,
    pub connector_payout_id: Option<String>,
    pub status_code: u16,
}
```

`PayoutStageResponse` carries no dedicated `connector_quote_id` field; connectors that return a
quote id put it into `connector_payout_id`, and the caller threads it into
`PayoutCreateRequest.connector_quote_id` / `PayoutTransferRequest.connector_quote_id` on the next
RPC. `payout_status` is `common_enums::PayoutStatus` (`pub enum PayoutStatus` in
`crates/common/common_enums/src/enums.rs`, fourteen variants: `Success`, `Failure`, `Cancelled`,
`Initiated`, `Expired`, `Reversed`, `Pending`, `Ineligible`, `NotPermitted`, `RequiresCreation`,
`RequiresConfirmation`, `RequiresPayoutMethodData`, `RequiresFulfillment`,
`RequiresVendorAccountCreation`); a successful quote maps naturally to
`PayoutStatus::RequiresConfirmation` — "quote ready, not yet committed".

Note that `PayoutStageResponse` is one of the *four-field* payout responses: unlike
`PayoutCreateRecipientResponse` and `PayoutEligibilityResponse` it has no metadata field, so an
opaque connector quote object has nowhere structured to live.

### Resource Common Data

`PayoutFlowData` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`. Identical envelope as all other payout flows; see [pattern_payout_void.md](./pattern_payout_void.md) for a full field-by-field breakdown.

### RouterDataV2 Shape

```rust
RouterDataV2<PayoutStage, PayoutFlowData, PayoutStageRequest, PayoutStageResponse>
```

Canonical four-arg shape per §7 of `PATTERN_AUTHORING_SPEC.md`.

## Connectors with Full Implementation

**None.** All ten payout connectors register `PayoutStageV2` with a fail-fast stub. Stage/quote endpoints are rare: no payout connector in the tree exposes one.

Verify before trusting this:

```bash
rg -n 'ConnectorIntegrationV2<\s*PayoutStage|flow_name: PayoutStage' \
   crates/integrations/connector-integration/src/payout_connectors/
```

Every hit is a stub whose only method is a `get_url` returning
`IntegrationError::connector_flow_not_implemented(self.id(), "payout_stage", IntegrationErrorContext::default())`.

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
  `impl PayoutStageV2 for <Name>Payouts {}` plus a one-method
  `ConnectorIntegrationV2<PayoutStage, PayoutFlowData, PayoutStageRequest, PayoutStageResponse>` block longhand;
  `payout_connectors/truelayer.rs` generates the same shape from its file-local
  `macro_rules! impl_unimplemented_payout_flow!`.
- **Macro** — the three generic payout connectors list `PayoutStage` in the `payout_flows: [...]`
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
        PayoutStage,     // <-- registers PayoutStageV2 + a fail-fast ConnectorIntegrationV2 impl
        PayoutCreateLink,
        PayoutCreateRecipient,
        PayoutEnrollDisburseAccount
    ]
);
```

The recursion in `macro_connector_payout_implementation!` at `crates/integrations/connector-integration/src/connectors/macros.rs` drives each token to `expand_payout_implementation!`. The `PayoutStage` arm at `macros.rs` reads:

```rust
// From crates/integrations/connector-integration/src/connectors/macros.rs
(
    connector: $connector: ident,
    flow: PayoutStage,
    generic_type: $generic_type:tt,
    [ $($bounds:tt)* ]
) => {
    impl<$generic_type: $($bounds)*> ::interfaces::connector_types::PayoutStageV2 for $connector<$generic_type> {}
    impl<$generic_type: $($bounds)*>
        ::interfaces::connector_integration_v2::ConnectorIntegrationV2<
            ::domain_types::connector_flow::PayoutStage,
            ::domain_types::payouts::payouts_types::PayoutFlowData,
            ::domain_types::payouts::payouts_types::PayoutStageRequest,
            ::domain_types::payouts::payouts_types::PayoutStageResponse,
        > for $connector<$generic_type>
    {
        fn get_url(
            &self,
            _req: &::domain_types::router_data_v2::RouterDataV2<
                ::domain_types::connector_flow::PayoutStage,
                ::domain_types::payouts::payouts_types::PayoutFlowData,
                ::domain_types::payouts::payouts_types::PayoutStageRequest,
                ::domain_types::payouts::payouts_types::PayoutStageResponse,
            >,
        ) -> ::common_utils::CustomResult<String, ::domain_types::errors::IntegrationError> {
            Err(::domain_types::errors::IntegrationError::connector_flow_not_implemented(
                ::interfaces::api::ConnectorCommon::id(self),
                "payout_stage",
                ::domain_types::errors::IntegrationErrorContext::default(),
            ).into())
        }
    }
};
```

The body is **not** empty: `get_url` is overridden so the stub fails fast with
`IntegrationError::connector_flow_not_implemented(id, "payout_stage", …)` rather than falling through to a
`ConnectorIntegrationV2` trait default. And the emitted target is `$connector<$generic_type>` — the
macro matches only a **generic** connector struct.

### Moving from stub to real

A flow is stub **or** real, never both; two impls of the same
`ConnectorIntegrationV2<PayoutStage, …>` will not compile.

- **Generic connector** (`DeutschebankPayouts<T>`, `GotymeSanlamPayouts<T>`, `TrustlyPayouts<T>`):
  delete `PayoutStage` from `payout_flows: [...]`, add a matching
  `(flow: PayoutStage, request_body: …, response_body: …, router_data: RouterDataV2<PayoutStage,
  PayoutFlowData, PayoutStageRequest, PayoutStageResponse>)` entry to
  `macros::create_all_prerequisites!`, write `impl PayoutStageV2 for <Name>Payouts<T> {}`, and add a
  `macros::macro_connector_implementation!` call with `flow_name: PayoutStage` and
  `resource_common_data: PayoutFlowData`. `payout_connectors/trustly.rs` does exactly this for
  `PayoutCreateRecipient`, `PayoutTransfer` and `PayoutGet`.
- **Non-generic connector** (the other seven): delete the hand-written stub `impl` and replace it
  with a longhand `impl ConnectorIntegrationV2<PayoutStage, PayoutFlowData, PayoutStageRequest, PayoutStageResponse> for
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


### Request-Body Strategies Observed for Quote-Style Flows

1. **Quote GET** — connectors model staging as a read. `GET {base_url}/quotes?source=...&target=...&amount=...`. No body.
2. **Quote POST** — connectors accept a body of `{ source_currency, destination_currency, amount }`. `POST {base_url}/quotes`.
3. **Combined create+quote** — some connectors merge staging into `PayoutCreate` and skip `PayoutStage` entirely.

No payout connector implements any of these shapes for `PayoutStage`. The list is a design note, not a citation of in-repo behaviour.

## Connector-Specific Patterns

### itaubank

- `ItaubankPayouts` is a non-generic unit struct and carries a **hand-written** `PayoutStage` stub in
  `crates/integrations/connector-integration/src/payout_connectors/itaubank.rs` (under the
  `// ===== PAYOUT STUB FLOWS =====` banner) — it does not call
  `macro_connector_payout_implementation!` at all. The Itaú SiSPAG integration is a single-currency
  BRL product with no quote endpoint, so
  `payout_connectors/itaubank/transformers.rs` contains no
  `PayoutStageRequest`/`PayoutStageResponse` `TryFrom` blocks. The flow is registered-but-inert.

### Everyone else

All nine remaining payout connectors are in the same state — see the Stub Implementations list above
for which produce their stub by hand and which by macro. `payout_connectors/santander.rs` is the
closest structural template for a future implementation: it is the only connector with a real
`PayoutCreate`, and `PayoutStage` sits immediately upstream of it in the lifecycle.

## Code Examples

### 1. Stub registration via the macro (generic connectors only)

`ItaubankPayouts` is not an example — it is a non-generic unit struct and hand-writes its stubs.

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
        PayoutEnrollDisburseAccount
    ]
);
```

`PayoutTransfer`, `PayoutGet` and `PayoutEligibility` are absent from that list because Deutsche Bank
implements all three for real via `macros::macro_connector_implementation!`.

### 2. Marker trait definition

```rust
// From crates/types-traits/interfaces/src/connector_types.rs
pub trait PayoutStageV2:
    ConnectorIntegrationV2<
    connector_flow::PayoutStage,
    PayoutFlowData,
    PayoutStageRequest,
    PayoutStageResponse,
>
{
}
```

### 3. Request-type shape

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs
#[derive(Debug, Clone)]
pub struct PayoutStageRequest {
    pub merchant_quote_id: Option<String>,
    pub amount: common_utils::types::MinorUnit,
    pub source_currency: common_enums::Currency,
    pub destination_currency: common_enums::Currency,
}
```

### 4. Reference implementation shape (adapted from `PayoutTransfer` on itaubank)

```rust
// Adapted shape — see crates/integrations/connector-integration/src/payout_connectors/itaubank.rs
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PayoutStage,
        PayoutFlowData,
        PayoutStageRequest,
        PayoutStageResponse,
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
        req: &RouterDataV2<PayoutStage, PayoutFlowData, PayoutStageRequest, PayoutStageResponse>,
    ) -> CustomResult<String, IntegrationError> {
        let base_url = self.base_url(&req.resource_common_data.connectors);
        Ok(format!("{base_url}/v1/quotes"))
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<PayoutStage, PayoutFlowData, PayoutStageRequest, PayoutStageResponse>,
    ) -> CustomResult<Option<ConnectorRequestData>, IntegrationError> {
        let connector_req = <ConnectorQuoteRequest>::try_from(req)?;
        let typed = events::MaskedSerdeValue::from_masked_optional(
            &connector_req,
            "typed_connector_request",
        );
        Ok(Some(ConnectorRequestData::new(
            RequestContent::Json(Box::new(connector_req)),
            typed,
        )))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<PayoutStage, PayoutFlowData, PayoutStageRequest, PayoutStageResponse>,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<PayoutStage, PayoutFlowData, PayoutStageRequest, PayoutStageResponse>,
        ConnectorError,
    > {
        // Map the connector's quote-ready state to PayoutStatus::RequiresConfirmation.
        let response: MyConnectorQuoteResponse = res
            .response
            .parse_struct("MyConnectorQuoteResponse")
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
`RouterDataV2` through `TryFrom<ResponseRouterData<MyConnectorQuoteResponse, …>>`, sets the event response data
and writes `typed_connector_response`. Declare that `TryFrom` in the connector's `transformers.rs`;
it is where `PayoutStageResponse`'s fields — including `status_code`, from
`item.http_code` — get populated.

Status mapping MUST be derived from the connector response; per §11 of `PATTERN_AUTHORING_SPEC.md` a
literal such as `payout_status: PayoutStatus::RequiresConfirmation` inside the `TryFrom` block is
banned unless it is the documented "every 2xx means quote-ready" contract for that connector. Two
in-tree precedents for the deliberate-literal case exist:
`payout_connectors/worldpayxml/transformers.rs` sets `PayoutStatus::Pending` on a `PayoutVoid`
because Worldpay's `cancelReceived` is an acknowledgement, and
`payout_connectors/trustly/transformers.rs` sets `PayoutStatus::RequiresCreation` on a
`PayoutCreateRecipient` because a registered account is not yet a payout. Both branch on the
response's own success/error variant first. The enum-driven alternative is
`impl <Connector>PayoutStatus { pub fn get_payout_status(&self) -> common_enums::PayoutStatus }`, as
in `payout_connectors/itaubank/transformers.rs` and `payout_connectors/santander/transformers.rs`.

### 5. Amount-conversion note

`PayoutStageRequest.amount` is `common_utils::types::MinorUnit` (see `pub struct PayoutStageRequest`
in `crates/types-traits/domain_types/src/payouts/payouts_types.rs`). Convert to the connector-specific
shape with the appropriate converter from `common_utils::types`; both in-tree payout examples use
`StringMajorUnitForConnector` — `ItaubankTransferRequest::try_from` in
`payout_connectors/itaubank/transformers.rs` and `SantanderCreateRequest::try_from` in
`payout_connectors/santander/transformers.rs` — with the error bubbled via
`change_context(IntegrationError::RequestEncodingFailed { .. })`.

## Integration Guidelines

1. Confirm the connector exposes a quote/staging endpoint. If the connector's payout API folds staging into `PayoutCreate` (common for single-currency domestic rails), skip `PayoutStage` and wire only the downstream flows.
2. **Remove** the existing `PayoutStage` stub — every payout connector already has one. For a generic connector delete `PayoutStage` from the `payout_flows: [...]` list; for a non-generic one delete the hand-written stub `impl`. Leaving both is a conflicting-implementation compile error.
3. Write `impl PayoutStageV2 for <Name>Payouts {}` plus the real
   `impl ConnectorIntegrationV2<PayoutStage, PayoutFlowData, PayoutStageRequest, PayoutStageResponse>`
   block — longhand on a non-generic connector, or `macros::macro_connector_implementation!` with
   `flow_name: PayoutStage` and `resource_common_data: PayoutFlowData` on a generic one. Because `PayoutStage` produces no connector-side payout object yet, URL construction typically does NOT embed an id — it is a bare `/quotes`-style POST.
4. Because `PayoutStageRequest` has no `payout_method_data`, the connector's quote request struct MUST be derivable purely from `amount`/`source_currency`/`destination_currency`/`merchant_quote_id`. If the connector also needs a beneficiary for a quote, signal this via `IntegrationError::NotSupported { message, connector, context }` (errors.rs) to the router — do not invent `payout_method_data` from thin air.
5. In `payout_connectors/<connector>/transformers.rs`, add a `TryFrom<&RouterDataV2<PayoutStage, ...>>` impl that produces the connector's quote-request struct, plus a `TryFrom<ResponseRouterData<…>>` for the `RouterDataV2`. Use the same amount-conversion pattern as `ItaubankTransferRequest::try_from` in `payout_connectors/itaubank/transformers.rs`, and call `finalize_connector_response!` from `handle_response_v2`.
6. Add a response-side `TryFrom<ResponseRouterData<..>, Self>>` that maps the quote id (usually) into `connector_payout_id` and sets `payout_status` to `PayoutStatus::RequiresConfirmation`.
7. Plumb `merchant_quote_id` into whatever "reference" field the connector exposes so the quote can be correlated by merchant systems.
8. Write unit tests for the quote success and quote-rejection paths, and an integration test
   alongside `crates/grpc-server/grpc-server/tests/payout_flows_test.rs`.
9. **Register the connector at all six sites** listed in the scope note above (the four shared wiring sites below plus the connector file itself and its `payout_connectors.rs` export), and add
   **no** `config/superposition.toml` entry and **no** `connector_specs/<connector>/specs.json`
   entry — `PayoutService` is in `IGNORE_SERVICES` in both
   `crates/internal/integration-tests/src/bin/check_connector_specs.rs` and `check_coverage.rs`.

## Best Practices

- Prefer a minimal connector-side quote request. The request type at `crates/types-traits/domain_types/src/payouts/payouts_types.rs` only has four fields; respect that minimalism and do not fabricate beneficiary defaults.
- Map quote id → `connector_payout_id`. Downstream `PayoutCreate` is already written to lift `connector_payout_id` into `connector_quote_id` via `PayoutCreateRequest.connector_quote_id` at `crates/types-traits/domain_types/src/payouts/payouts_types.rs`.
- Use `PayoutStatus::RequiresConfirmation` (variant at `crates/common/common_enums/src/enums.rs`) for a successful staged quote, NOT `PayoutStatus::Success`. A staged quote is not a completed payout.
- Return `ConnectorCommon::build_error_response` from `get_error_response_v2`, exactly as the `PayoutTransfer` impl in `crates/integrations/connector-integration/src/payout_connectors/itaubank.rs` does, when the connector rejects a quote (e.g. "unsupported currency pair").
- See the downstream pattern [pattern_payout_create.md](./pattern_payout_create.md) for how `connector_quote_id` is consumed after staging.

## Common Errors / Gotchas

1. **Problem:** `PayoutStageResponse.payout_status = PayoutStatus::Success` even though the connector only returned a quote.
   **Solution:** A quote is not a settled payout. Map successful quote responses to `PayoutStatus::RequiresConfirmation` (variant at `crates/common/common_enums/src/enums.rs`). The router uses this status to decide whether to auto-progress to `PayoutCreate` or require a merchant-side confirmation.

2. **Problem:** Connector's quote endpoint requires a beneficiary but `PayoutStageRequest` has no `payout_method_data`.
   **Solution:** Mark the staging flow as unsupported for that connector. Do not synthesize a dummy beneficiary. Emit `IntegrationError::NotSupported { message, connector, context }` (errors.rs) with a message naming the missing field. See the `IntegrationError` enum at `crates/types-traits/domain_types/src/errors.rs` onward.

3. **Problem:** Quote id is returned but `connector_payout_id` in `PayoutStageResponse` is `None`, and downstream `PayoutCreate` fails because `connector_quote_id` is also `None`.
   **Solution:** Populate `connector_payout_id: Some(quote_id)` inside the response `TryFrom`. The field is `connector_payout_id: Option<String>` on `pub struct PayoutStageResponse` in `crates/types-traits/domain_types/src/payouts/payouts_types.rs`; returning `None` on a success path is a contract violation.

4. **Problem:** Compile error "conflicting implementations of trait `ConnectorIntegrationV2<PayoutStage, ...>`".
   **Solution:** The macro already emitted an empty impl. Remove `PayoutStage` from the `payout_flows:` list before writing the full impl. See `crates/integrations/connector-integration/src/connectors/macros.rs`.

5. **Problem:** Quote expires between `PayoutStage` and `PayoutCreate` and the merchant sees an opaque "quote not found" error.
   **Solution:** In `PayoutCreate` transformers, detect the connector's "quote expired" error code and re-issue `PayoutStage`. Cross-ref [pattern_payout_create.md](./pattern_payout_create.md).

## Testing Notes

### Unit Tests

Each connector implementing PayoutStage should cover:

- `TryFrom<&RouterDataV2<PayoutStage, ...>>` — valid USD-to-EUR quote, asserts body has correct currency codes and minor-unit amount.
- `TryFrom<ResponseRouterData<ConnectorQuoteResponse, Self>>` — maps quote id → `connector_payout_id` and status → `PayoutStatus::RequiresConfirmation`.
- Unsupported currency pair — connector returns 422 → error path emits `ErrorResponse` with the connector's code and reason.

### Integration Scenarios

| Scenario | Inputs | Expected `payout_status` | Expected `status_code` |
| --- | --- | --- | --- |
| Stage cross-border quote | amount=10000 MinorUnit, USD → EUR | `RequiresConfirmation` | 200 |
| Stage unsupported pair | amount=10000, USD → ZMW (unsupported) | — (error) | 4xx |
| Stage with merchant_quote_id | amount=10000, USD→EUR, merchant_quote_id=Some("abc") | `RequiresConfirmation` | 200 |
| Zero-amount stage | amount=0 | — (error, `IntegrationError::InvalidDataFormat { field_name, context }` (errors.rs)) | 422 |

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
- Sibling side-flow: [pattern_payout_create_link.md](./pattern_payout_create_link.md)
- Macro reference: [macro_patterns_reference.md](./macro_patterns_reference.md)
- Utility helpers: [utility_functions_reference.md](../utility_functions_reference.md)
- Authoring spec: [PATTERN_AUTHORING_SPEC.md](./PATTERN_AUTHORING_SPEC.md)
