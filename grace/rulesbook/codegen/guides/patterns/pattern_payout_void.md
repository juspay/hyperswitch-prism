# Payout Void Flow Pattern

## Overview

The Payout Void flow cancels an in-flight or scheduled payout before the connector has finalized disbursement. It is the payout analogue of the Payments Void flow and must be invoked on a previously created payout reference (obtained from `PayoutCreate` or `PayoutTransfer`). The flow posts a cancellation instruction to the connector, then maps the returned status back to `common_enums::PayoutStatus` so the router can observe the cancelled or still-pending state. Because not every connector supports cancellation at every lifecycle state, the flow MUST surface a connector error when cancellation is rejected rather than fabricating a success.

### Key Components

- Flow marker: `PayoutVoid` — `crates/types-traits/domain_types/src/connector_flow.rs`.
- Request type: `PayoutVoidRequest` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`.
- Response type: `PayoutVoidResponse` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`.
- Flow-data type: `PayoutFlowData` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`.
- Marker trait: `PayoutVoidV2` — `crates/types-traits/interfaces/src/connector_types.rs`, defined solely as the supertrait binding:

  ```rust
  // crates/types-traits/interfaces/src/connector_types.rs — pub trait PayoutVoidV2
  pub trait PayoutVoidV2:
      ConnectorIntegrationV2<
      connector_flow::PayoutVoid,
      PayoutFlowData,
      PayoutVoidRequest,
      PayoutVoidResponse,
  >
  {
  }
  ```

- Service trait: `PayoutServiceTrait` — `crates/types-traits/interfaces/src/connector_types.rs`.
- Stub macro arm: the `flow: PayoutVoid` arm of `expand_payout_implementation!` — `crates/integrations/connector-integration/src/connectors/macros.rs`.
- Integrity object: `PayoutVoidIntegrityObject` — `crates/types-traits/domain_types/src/payouts/router_request_types.rs` (`merchant_payout_id` + `connector_payout_id` only).
- Reference implementation: `crates/integrations/connector-integration/src/payout_connectors/worldpayxml.rs` (`impl PayoutVoidV2 for WorldpayxmlPayouts` and the `ConnectorIntegrationV2<PayoutVoid, …>` block below it), with transformers in `crates/integrations/connector-integration/src/payout_connectors/worldpayxml/transformers.rs`.

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

Payout Void is a side-flow: it neither creates nor mutates balances, it only cancels a pending payout. In UCS it uses the same `PayoutFlowData` envelope as every other payout flow so that access-token propagation, `connector_request_reference_id` correlation, and raw-request/raw-response audit capture are uniform.

### Flow Hierarchy

```
PayoutCreate / PayoutTransfer  (upstream — produces connector_payout_id)
        |
        v
PayoutVoid  (this flow — requires connector_payout_id)
        |
        v
PayoutGet  (downstream verification — optional but recommended)
```

### Flow Type

`PayoutVoid` — zero-sized marker struct declared at `crates/types-traits/domain_types/src/connector_flow.rs`. It is also registered in `FlowName::PayoutVoid` at `crates/types-traits/domain_types/src/connector_flow.rs` for telemetry.

### Request Type

`PayoutVoidRequest` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`:

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs
#[derive(Debug, Clone)]
pub struct PayoutVoidRequest {
    pub merchant_payout_id: Option<String>,
    pub connector_payout_id: Option<String>,
}
```

Both fields are `Option<String>`; a connector that requires the connector-side identifier MUST
validate it is `Some` and emit `IntegrationError::MissingRequiredField` otherwise. Where that check
lives depends on where the id travels: `WorldpayxmlPayouts` puts it in the request `TryFrom`
(`payout_connectors/worldpayxml/transformers.rs`) because the id goes in the SOAP body, while a
REST-shaped cancel endpoint would check inside `get_url`.

### Response Type

`PayoutVoidResponse` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`:

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs
#[derive(Debug, Clone)]
pub struct PayoutVoidResponse {
    pub merchant_payout_id: Option<String>,
    pub payout_status: common_enums::PayoutStatus,
    pub connector_payout_id: Option<String>,
    pub status_code: u16,
}
```

`payout_status` is a `common_enums::PayoutStatus` — `crates/common/common_enums/src/enums.rs`. After a successful void, the expected terminal mapping is `PayoutStatus::Cancelled`. If the connector instead returns a pending-cancellation intermediate state, the transformer MUST map to `PayoutStatus::Pending` (never `Cancelled`) so the router keeps polling.

### Resource Common Data

`PayoutFlowData` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`:

```rust
// crates/types-traits/domain_types/src/payouts/payouts_types.rs — pub struct PayoutFlowData
#[derive(Debug, Clone)]
pub struct PayoutFlowData {
    pub merchant_id: common_utils::id_type::MerchantId,
    pub payout_id: String,
    pub connectors: Arc<Connectors>,
    pub connector_request_reference_id: String,
    pub raw_connector_response: Option<Secret<String>>,
    pub typed_connector_response: Option<String>,
    pub connector_response_headers: Option<http::HeaderMap>,
    pub raw_connector_request: Option<Secret<String>>,
    pub typed_connector_request: Option<String>,
    pub access_token: Option<ServerAuthenticationTokenResponseData>,
    pub test_mode: Option<bool>,
    pub description: Option<String>,
    pub merchant_request_id: Option<String>,
}
```

Thirteen fields — note `connectors` is `Arc<Connectors>`, not a bare `Connectors`, and that the
`typed_*` pair exists alongside the `raw_*` pair (both written through the
`RawConnectorRequestResponse` impl for `PayoutFlowData` in the same file;
`finalize_connector_response!` sets the response side for you).

Access-token-gated connectors call `PayoutFlowData::get_access_token` (`pub fn get_access_token` in the same file) from inside `get_headers` to attach a `Bearer` token.

### RouterDataV2 Shape

```rust
RouterDataV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>
```

This is the canonical four-type-argument envelope per §7 of `PATTERN_AUTHORING_SPEC.md`. Three-argument forms or V1 `RouterData` MUST NOT appear.

## Connectors with Full Implementation

Exactly **one** payout connector supplies a non-stub `ConnectorIntegrationV2<PayoutVoid,
PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>` implementation at HEAD:
**`WorldpayxmlPayouts`**, in
`crates/integrations/connector-integration/src/payout_connectors/worldpayxml.rs`.

Verify the roster before trusting it:

```bash
rg -n 'impl ConnectorIntegrationV2<PayoutVoid|flow_name: PayoutVoid' \
   crates/integrations/connector-integration/src/payout_connectors/
```

Current implementation coverage: **1 of 10 payout connectors.**

| Connector | HTTP Method | Content Type | URL Pattern | Request type | Notes |
| --- | --- | --- | --- | --- | --- |
| `WorldpayxmlPayouts` | `POST` | XML (`CONTENT_TYPE_XML`) | the bare `connectors.worldpayxml.base_url` — Worldpay XML routes by envelope, not path | `requests::WorldpayxmlPayoutVoidRequest` (a `PoCancel` / `cancelRefund` order modification) | Body built by `Self::encode_soap_xml`; response parsed by `Self::parse_xml_response` into `responses::WorldpayxmlPayoutVoidResponse`. |

### Stub Implementations

The other nine connectors register `PayoutVoidV2` with a fail-fast stub whose only method is a
`get_url` returning
`IntegrationError::connector_flow_not_implemented(self.id(), "payout_void", …)`:

- **Hand-written** (non-generic connectors, which cannot use the payout macro):
  `cybersource.rs`, `itaubank.rs`, `loonio.rs`, `paypal.rs`, `santander.rs`, and `truelayer.rs` (via
  its file-local `macro_rules! impl_unimplemented_payout_flow!`).
- **Macro** (generic connectors): `deutschebank.rs`, `gotyme_sanlam.rs` and `trustly.rs` list
  `PayoutVoid` in the `payout_flows: [...]` array of
  `macros::macro_connector_payout_implementation!`.

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

### A flow is stub **or** real — never both

A stub and a real implementation of the same flow are conflicting `impl` blocks and will not
compile. To move `PayoutVoid` from stub to real:

- **Generic connector**: delete `PayoutVoid` from the `payout_flows: [...]` list, then write
  `impl PayoutVoidV2 for <Name>Payouts<T> {}` plus the real
  `ConnectorIntegrationV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>` block
  (or a `macros::macro_connector_implementation!` with `flow_name: PayoutVoid`,
  `resource_common_data: PayoutFlowData`).
- **Non-generic connector**: delete the hand-written stub `impl` block (the one whose only method is
  a `get_url` returning `connector_flow_not_implemented`) and replace it with the real one, exactly
  as `payout_connectors/worldpayxml.rs` does.

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


### Request-Body Strategies Observed for Payout-Flow Cancellations

The three strategies typically seen in payout APIs at connector-API level:

1. **Path-only cancel** — cancellation endpoint embeds the connector payout id and the body is empty (`{}` or no body). URL shape `POST {base_url}/payouts/{connector_payout_id}/cancel`.
2. **Id-in-body cancel** — cancellation endpoint is static and the body carries the id. URL shape `POST {base_url}/payouts/cancel` with `{ "id": "<connector_payout_id>" }`.
3. **PATCH-status cancel** — connector exposes a generic status-update endpoint. URL shape `PATCH {base_url}/payouts/{connector_payout_id}` with `{ "status": "cancelled" }`.

The one in-repo implementation, `WorldpayxmlPayouts`, follows none of these three: Worldpay's XML
gateway routes by envelope rather than by path, so `get_url` returns the bare
`connectors.worldpayxml.base_url` and the connector payout id travels inside the SOAP body as the
`order_code` of a `cancelRefund` order modification. Read the connector's API before assuming a
REST-shaped cancel endpoint.

## Connector-Specific Patterns

### worldpayxml (`WorldpayxmlPayouts`, the only full implementation)

- **Non-generic unit struct** — `pub struct WorldpayxmlPayouts;`. Every one of its nine payout flows
  is written longhand; the payout macro cannot be used.
- **`get_url` returns the bare base URL**:
  `req.resource_common_data.connectors.worldpayxml.base_url.to_string()`. There is no path segment —
  the Worldpay XML gateway dispatches on the SOAP envelope.
- **`get_headers`** delegates to the associated function `Self::xml_headers(&req.connector_config)`.
- **`get_request_body`** builds `requests::WorldpayxmlPayoutVoidRequest::try_from(req)?`, produces
  the masked `typed_connector_request` with `events::MaskedSerdeValue::from_masked_optional`, encodes
  with `Self::encode_soap_xml(&connector_req)?` and returns
  `ConnectorRequestData::new(content, typed)` — note the return type is
  `Option<ConnectorRequestData>`, not `Option<RequestContent>`.
- **The connector payout id is mandatory**: `WorldpayxmlPayoutVoidRequest::try_from` in
  `payout_connectors/worldpayxml/transformers.rs` does
  `router_data.request.connector_payout_id.clone().ok_or(IntegrationError::MissingRequiredField { field_name: "connector_payout_id", context: Default::default() })?`
  and uses it as the `order_code`.
- **`handle_response_v2`** parses with `Self::parse_xml_response(res.response, res.status_code)?`
  into `responses::WorldpayxmlPayoutVoidResponse`, then hands off to
  `finalize_connector_response!(event_builder, response, data, res.status_code)`.
- **Status is `Pending`, deliberately.** The `TryFrom<ResponseRouterData<…>>` in
  `worldpayxml/transformers.rs` sets `payout_status: common_enums::PayoutStatus::Pending` because a
  Worldpay `cancelReceived` element acknowledges receipt of the cancellation, not its completion.
  This is the documented exception to "never hardcode a status": the connector contract is
  explicitly *acknowledgement*, and the terminal state must be confirmed with `PayoutGet`. It also
  branches on `response.reply.error` first, returning
  `crate::utils::build_error_response(error.code, error.message, item.http_code, None)`.

### itaubank

- `itaubank` carries a hand-written `PayoutVoid` stub in
  `payout_connectors/itaubank.rs`. The Itaú SiSPAG integration does not expose a cancellation
  endpoint, and `payout_connectors/itaubank/transformers.rs` contains no `PayoutVoidRequest` /
  `PayoutVoidResponse` `TryFrom` blocks. The flow is registered-but-inert.

Treat every stub as a **coverage gap, not a decision that the flow should never be covered** — the
same framing `crates/internal/integration-tests/src/bin/check_connector_specs.rs` applies to
uncovered flows.

## Code Examples

### 1. Stub registration via the macro (generic connectors only)

`ItaubankPayouts` is **not** an example of this — it is a non-generic unit struct and hand-writes its
stubs. The macro is used by the three generic payout connectors; `TrustlyPayouts<T>` is the live
example:

```rust
// crates/integrations/connector-integration/src/payout_connectors/trustly.rs
//   — under the `// ===== PAYOUT STUB FLOWS (not supported by Trustly) =====` banner
macros::macro_connector_payout_implementation!(
    connector: TrustlyPayouts,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    payout_flows: [
        PayoutCreate,
        PayoutVoid,       // <-- registers PayoutVoidV2 + a fail-fast ConnectorIntegrationV2 impl
        PayoutStage,
        PayoutCreateLink,
        PayoutEnrollDisburseAccount
    ]
);
```

The flows Trustly really implements — `PayoutCreateRecipient`, `PayoutTransfer`, `PayoutGet` — are
deliberately absent from that list; each has its own `macros::macro_connector_implementation!` call
earlier in the file. `PayoutEligibility` is also absent: `trustly.rs` writes that stub by hand.
(An in-file comment there claims `PayoutEligibility` "has no arm in
`macro_connector_payout_implementation!`" — that comment is stale. The arm exists in
`expand_payout_implementation!`; the hand-written stub is harmless only because `PayoutEligibility`
is omitted from the `payout_flows: [...]` list.)

### 2. The macro arm that expands `PayoutVoid`

```rust
// crates/integrations/connector-integration/src/connectors/macros.rs
//   — macro_rules! expand_payout_implementation, the `flow: PayoutVoid` arm
(
    connector: $connector: ident,
    flow: PayoutVoid,
    generic_type: $generic_type:tt,
    [ $($bounds:tt)* ]
) => {
    impl<$generic_type: $($bounds)*> ::interfaces::connector_types::PayoutVoidV2 for $connector<$generic_type> {}
    impl<$generic_type: $($bounds)*>
        ::interfaces::connector_integration_v2::ConnectorIntegrationV2<
            ::domain_types::connector_flow::PayoutVoid,
            ::domain_types::payouts::payouts_types::PayoutFlowData,
            ::domain_types::payouts::payouts_types::PayoutVoidRequest,
            ::domain_types::payouts::payouts_types::PayoutVoidResponse,
        > for $connector<$generic_type>
    {
        fn get_url(
            &self,
            _req: &::domain_types::router_data_v2::RouterDataV2<
                ::domain_types::connector_flow::PayoutVoid,
                ::domain_types::payouts::payouts_types::PayoutFlowData,
                ::domain_types::payouts::payouts_types::PayoutVoidRequest,
                ::domain_types::payouts::payouts_types::PayoutVoidResponse,
            >,
        ) -> ::common_utils::CustomResult<String, ::domain_types::errors::IntegrationError> {
            Err(::domain_types::errors::IntegrationError::connector_flow_not_implemented(
                ::interfaces::api::ConnectorCommon::id(self),
                "payout_void",
                ::domain_types::errors::IntegrationErrorContext::default(),
            ).into())
        }
    }
};
```

Two things to note. First, the emitted target is `$connector<$generic_type>` — the macro only matches
a **generic** connector struct, which is why the seven non-generic payout connectors cannot use it.
Second, the body is **not** empty: `get_url` is overridden so the stub fails fast with
`IntegrationError::connector_flow_not_implemented(id, "payout_void", …)` rather than falling through
to a `ConnectorIntegrationV2` trait default.

### 3. Marker trait definition

```rust
// From crates/types-traits/interfaces/src/connector_types.rs
pub trait PayoutVoidV2:
    ConnectorIntegrationV2<
    connector_flow::PayoutVoid,
    PayoutFlowData,
    PayoutVoidRequest,
    PayoutVoidResponse,
>
{
}
```

### 4. The real implementation (`WorldpayxmlPayouts`)

```rust
// crates/integrations/connector-integration/src/payout_connectors/worldpayxml.rs
impl PayoutVoidV2 for WorldpayxmlPayouts {}

impl ConnectorIntegrationV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>
    for WorldpayxmlPayouts
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
    }

    fn get_content_type(&self) -> &'static str {
        CONTENT_TYPE_XML
    }

    fn get_url(
        &self,
        req: &RouterDataV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>,
    ) -> CustomResult<String, IntegrationError> {
        // Worldpay XML routes by SOAP envelope, not by path.
        Ok(req
            .resource_common_data
            .connectors
            .worldpayxml
            .base_url
            .to_string())
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        Self::xml_headers(&req.connector_config)
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>,
    ) -> CustomResult<Option<ConnectorRequestData>, IntegrationError> {
        let connector_req = requests::WorldpayxmlPayoutVoidRequest::try_from(req)?;
        let typed = events::MaskedSerdeValue::from_masked_optional(
            &connector_req,
            "typed_connector_request",
        );
        let content = Self::encode_soap_xml(&connector_req)?;
        Ok(Some(ConnectorRequestData::new(content, typed)))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>,
        ConnectorError,
    > {
        let response: responses::WorldpayxmlPayoutVoidResponse =
            Self::parse_xml_response(res.response, res.status_code)?;
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

Note `get_request_body` returns **`Option<ConnectorRequestData>`** — see `fn get_request_body` on
`ConnectorIntegrationV2` in `crates/types-traits/interfaces/src/connector_integration_v2.rs`. The
mandatory-id check lives in the request `TryFrom` rather than in `get_url`, because the id travels in
the body:

```rust
// crates/integrations/connector-integration/src/payout_connectors/worldpayxml/transformers.rs
//   — impl TryFrom<&RouterDataV2<PayoutVoid, ...>> for requests::WorldpayxmlPayoutVoidRequest
let order_code = router_data.request.connector_payout_id.clone().ok_or(
    IntegrationError::MissingRequiredField {
        field_name: "connector_payout_id",
        context: Default::default(),
    },
)?;
```

Status mapping MUST be derived from the connector response — but read the connector contract first.
Worldpay's `cancelReceived` element is an **acknowledgement**, not a completion, so the response
`TryFrom` in `worldpayxml/transformers.rs` sets `payout_status: common_enums::PayoutStatus::Pending`
and expects the terminal state to be confirmed by `PayoutGet`. Hardcoding
`payout_status: PayoutStatus::Cancelled` on a 200 is what §11 of `PATTERN_AUTHORING_SPEC.md` bans.

## Integration Guidelines

Follow this ordered sequence when wiring a new connector's `PayoutVoid`:

1. Confirm the connector's cancel endpoint in its API docs and record the HTTP method, URL template,
   auth requirements, and whether a body is expected — and whether the response acknowledges or
   completes the cancellation.
2. **Remove the existing stub for `PayoutVoid`.** Every payout connector already registers the flow,
   so this is a *replacement*, never an addition:
   - generic connector (`DeutschebankPayouts<T>`, `GotymeSanlamPayouts<T>`, `TrustlyPayouts<T>`):
     delete `PayoutVoid` from the `payout_flows: [...]` list passed to
     `macros::macro_connector_payout_implementation!`;
   - non-generic connector (the other seven): delete the hand-written stub `impl` block whose only
     method is a `get_url` returning `connector_flow_not_implemented`.
   Leaving both is a conflicting-implementation compile error — Rust has no specialization here.
3. Write `impl PayoutVoidV2 for <Name>Payouts {}` plus the real
   `impl ConnectorIntegrationV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse> for <Name>Payouts`
   block, overriding `get_http_method`, `get_content_type`, `get_url`, `get_headers`,
   `get_request_body`, `handle_response_v2` and `get_error_response_v2`. Model it on
   `payout_connectors/worldpayxml.rs`. `get_request_body` returns
   `CustomResult<Option<ConnectorRequestData>, IntegrationError>`; if the API expects no body, omit
   the method entirely and let the trait default return `Ok(None)`.
4. In `payout_connectors/<name>/transformers.rs`, add
   `TryFrom<&RouterDataV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>>` for
   the connector's cancel-request struct, validating `connector_payout_id` with
   `IntegrationError::MissingRequiredField { field_name: "connector_payout_id", .. }`.
5. Add `TryFrom<ResponseRouterData<<ConnectorCancelResponse>, Self>>` for the `RouterDataV2` that
   maps the connector status to `common_enums::PayoutStatus`, and call
   `finalize_connector_response!(event_builder, response, data, res.status_code)` from
   `handle_response_v2`.
6. Delegate `get_error_response_v2` to `ConnectorCommon::build_error_response` so cancellation
   rejections surface as a real `ErrorResponse` with the connector's `code`/`message`/`reason`.
   For XML connectors, branch on the error element inside the response `TryFrom` first — see
   `crate::utils::build_error_response(error.code, error.message, item.http_code, None)` in
   `payout_connectors/worldpayxml/transformers.rs`.
7. Add unit tests in `payout_connectors/<name>/transformers.rs` covering (a) missing
   `connector_payout_id`, (b) successful cancellation, (c) connector reports "cannot cancel, already
   settled".
8. Add an integration test alongside
   `crates/grpc-server/grpc-server/tests/payout_flows_test.rs`. Note that **no `connector_specs`
   entry is required or possible**: `crates/internal/integration-tests/src/bin/check_connector_specs.rs` enumerates connectors from
   `connectors/` only, never `payout_connectors/`, and `PayoutService` is in its `IGNORE_SERVICES`
   (and `check_coverage.rs`'s), so payouts sit outside the merge-blocking certification gate. No
   `config/superposition.toml` entry is required either.

## Best Practices

- Always validate `connector_payout_id.is_some()` in `get_url` or `get_request_body`; the `Option` in `PayoutVoidRequest` means the router can legally forward `None`, and the connector will otherwise build a malformed URL. See the request-type definition at `crates/types-traits/domain_types/src/payouts/payouts_types.rs`.
- Reuse the connector's access-token helper `PayoutFlowData::get_access_token`
  (`crates/types-traits/domain_types/src/payouts/payouts_types.rs`) exactly as the `PayoutTransfer`
  impl in `payout_connectors/itaubank.rs` does. Do not re-parse auth tokens out of
  `ConnectorSpecificConfig` inside a void flow, and do not add a `ValidationTrait` impl — there is no
  `should_do_access_token` hook on the payout path.
- Propagate `res.status_code` into `PayoutVoidResponse.status_code` so callers can audit HTTP-level
  outcomes. With `finalize_connector_response!` this happens in the response `TryFrom`, which copies
  `item.http_code` — see `payout_connectors/worldpayxml/transformers.rs`.
- Use `ConnectorCommon::build_error_response` from `get_error_response_v2`, and for XML/SOAP
  connectors also branch on the error element inside the response `TryFrom`
  (`crate::utils::build_error_response(...)`), as `worldpayxml/transformers.rs` does.
- When mapping connector status, prefer the enum-driven pattern —
  `impl <Connector>PayoutStatus { pub fn get_payout_status(&self) -> common_enums::PayoutStatus }`,
  as in `payout_connectors/itaubank/transformers.rs` and
  `payout_connectors/santander/transformers.rs` — so Cancellation → `Cancelled` /
  Pending → `Pending` / Rejection → `Failure` is centralized. Where the connector only *acknowledges*
  the cancel, map to `Pending` and let `PayoutGet` confirm.
- See sibling flow [pattern_payout_create.md](./pattern_payout_create.md) for the upstream producer of the `connector_payout_id` this flow consumes.

## Common Errors / Gotchas

1. **Problem:** Compile error "conflicting implementations of trait `ConnectorIntegrationV2<PayoutVoid, ...>`".
   **Solution:** A stub already exists. Remove it before writing the real impl — from the
   `payout_flows: [...]` list for a generic connector, or by deleting the hand-written stub `impl`
   for a non-generic one. Rust has no specialization here; the two impls cannot coexist.

1a. **Problem:** `macro_connector_payout_implementation!` silently fails to expand.
   **Solution:** The macro matches only `$connector<$generic_type>`. Seven of the ten payout
   connectors (`CybersourcePayouts`, `ItaubankPayouts`, `LoonioPayouts`, `PaypalPayouts`,
   `SantanderPayouts`, `TruelayerPayouts`, `WorldpayxmlPayouts`) are non-generic unit structs and
   must hand-write their stubs — or use a file-local helper macro, as
   `payout_connectors/truelayer.rs` does with `macro_rules! impl_unimplemented_payout_flow!`.

2. **Problem:** `PayoutVoidResponse.payout_status = PayoutStatus::Cancelled` even though the connector is still processing.
   **Solution:** Do not hardcode status. Map from the connector response enum. The in-tree idiom is
   `impl ItaubankPayoutStatus { pub fn get_payout_status(&self) -> common_enums::PayoutStatus }` in
   `payout_connectors/itaubank/transformers.rs`, where `Pendente`/`EmProcessamento` map to
   `PayoutStatus::Pending` and only confirmed final states map to terminal outcomes. The terminal
   value for a completed void is `PayoutStatus::Cancelled` — see `pub enum PayoutStatus` in
   `crates/common/common_enums/src/enums.rs`, whose variants are `Success`, `Failure`, `Cancelled`,
   `Initiated`, `Expired`, `Reversed`, `Pending`, `Ineligible`, `NotPermitted`, `RequiresCreation`,
   `RequiresConfirmation`, `RequiresPayoutMethodData`, `RequiresFulfillment`,
   `RequiresVendorAccountCreation`.

3. **Problem:** `connector_payout_id` missing at runtime because the upstream call never captured it.
   **Solution:** Emit `IntegrationError::MissingRequiredField { field_name: "connector_payout_id", .. }`. The `IntegrationError` variant set is at `crates/types-traits/domain_types/src/errors.rs` onward. This is a request-time error, NOT a `ConnectorError` — keep the error categories distinct per `PATTERN_AUTHORING_SPEC.md` §12.

4. **Problem:** Void succeeded but `PayoutGet` polled right after returns `Pending`.
   **Solution:** Do not force-overwrite the local status based on the void HTTP 200. Return what the connector body says; if the connector's cancel endpoint is async, `Pending` → eventual `Cancelled` via webhook or subsequent `PayoutGet` is the correct sequence. Cross-reference sibling flow [pattern_payout_get.md](./pattern_payout_get.md).

5. **Problem:** `connector_flow_not_implemented` in production because only the stub was registered.
   **Solution:** Confirm a concrete override of
   `get_url`/`get_headers`/`get_request_body`/`handle_response_v2` exists. The stub's `get_url`
   returns `IntegrationError::connector_flow_not_implemented(id, "payout_void", …)` — loud, not
   silent. Nine of ten payout connectors are in that state today; each is a **coverage gap, not a
   decision that the flow should never be covered**.

6. **Problem:** `get_request_body` written to return `Option<RequestContent>`.
   **Solution:** The signature is
   `fn get_request_body(&self, …) -> CustomResult<Option<ConnectorRequestData>, IntegrationError>`
   (`crates/types-traits/interfaces/src/connector_integration_v2.rs`). Wrap the content with
   `ConnectorRequestData::new(content, typed)` where `typed` comes from
   `events::MaskedSerdeValue::from_masked_optional(&connector_req, "typed_connector_request")`.

## Testing Notes

### Unit Tests (in `<connector>/transformers.rs`)

Each connector that implements PayoutVoid should cover:

- `TryFrom<&RouterDataV2<PayoutVoid, ...>>` with `connector_payout_id = Some("abc")` — success path, asserts the serialized body/URL.
- `TryFrom<&RouterDataV2<PayoutVoid, ...>>` with `connector_payout_id = None` — asserts `IntegrationError::MissingRequiredField`.
- Response parsing for connector "cancellation accepted" → `PayoutStatus::Cancelled`.
- Response parsing for connector "cannot cancel, already paid" → error-response branch returning `ErrorResponse`.

### Integration Scenarios

| Scenario | Inputs | Expected `payout_status` | Expected `status_code` |
| --- | --- | --- | --- |
| Cancel pending payout | valid `connector_payout_id`, state=Pending | `Cancelled` | 200 |
| Cancel already-settled payout | valid `connector_payout_id`, state=Success | — (error path, `ErrorResponse`) | 4xx |
| Cancel unknown payout | non-existent `connector_payout_id` | — (error path) | 404 |
| Missing `connector_payout_id` | request has `None` | N/A — `IntegrationError::MissingRequiredField` before HTTP | N/A |

`WorldpayxmlPayouts` is the only connector that can exercise these scenarios today; the other nine
return `connector_flow_not_implemented`. The in-repo gRPC-level harness is
`crates/grpc-server/grpc-server/tests/payout_flows_test.rs`. There is no `connector_specs` suite for
payouts — `PayoutService` is in `IGNORE_SERVICES` in
`crates/internal/integration-tests/src/bin/check_connector_specs.rs`, so `PayoutVoid` sits outside
the merge-blocking certification gate.

## Cross-References

- Parent index: [../README.md](./README.md)
- Sibling core payout flow: [pattern_payout_create.md](./pattern_payout_create.md)
- Sibling core payout flow: [pattern_payout_transfer.md](./pattern_payout_transfer.md)
- Sibling core payout flow: [pattern_payout_get.md](./pattern_payout_get.md)
- Sibling side-flow: [pattern_payout_stage.md](./pattern_payout_stage.md)
- Sibling side-flow: [pattern_payout_create_link.md](./pattern_payout_create_link.md)
- Payments Void analogue: [pattern_void.md](./pattern_void.md)
- Refund cancellation analogue: [pattern_rsync.md](./pattern_rsync.md)
- Macro reference: [macro_patterns_reference.md](./macro_patterns_reference.md)
- Authoring spec: [PATTERN_AUTHORING_SPEC.md](./PATTERN_AUTHORING_SPEC.md)
- Reference implementation: `crates/integrations/connector-integration/src/payout_connectors/worldpayxml.rs`
- Registry: `crates/integrations/connector-integration/src/payout_connectors.rs`
