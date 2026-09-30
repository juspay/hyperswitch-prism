# PayoutCreate Flow Pattern

## Overview

The `PayoutCreate` flow is the entry point for initiating an outbound money movement (payout) through a payment processor. It is invoked by the `PayoutService::create` gRPC handler in `crates/grpc-server/grpc-server/src/server/payouts.rs` and dispatched through `internal_payout_create` at `crates/grpc-server/grpc-server/src/server/payouts.rs` under the `FlowName::PayoutCreate` marker (`crates/types-traits/domain_types/src/connector_flow.rs`). A connector implementing this flow typically creates a payout resource at the processor, reserving an identifier that can be retrieved later via `PayoutGet` or advanced via `PayoutTransfer`.

Exactly **one** connector supplies a non-stub `ConnectorIntegrationV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>` implementation at HEAD: **`SantanderPayouts`**, in `crates/integrations/connector-integration/src/payout_connectors/santander.rs` (see the block introduced by the `// ===== PAYOUT CREATE (POST — create payout with recipient) =====` banner). Every other payout connector registers the flow as a fail-fast stub. This document documents Santander as the canonical reference and describes the shape any new implementation must follow.

Verify the roster before trusting it:

```bash
rg -n 'impl ConnectorIntegrationV2<PayoutCreate' crates/integrations/connector-integration/src/payout_connectors/
```

### Key Components

- **Flow marker**: `domain_types::connector_flow::PayoutCreate` — `crates/types-traits/domain_types/src/connector_flow.rs`.
- **Flow data**: `domain_types::payouts::payouts_types::PayoutFlowData` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`.
- **Request data**: `domain_types::payouts::payouts_types::PayoutCreateRequest` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`.
- **Response data**: `domain_types::payouts::payouts_types::PayoutCreateResponse` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`.
- **Marker trait**: `interfaces::connector_types::PayoutCreateV2` — `crates/types-traits/interfaces/src/connector_types.rs`. Its full definition is the supertrait binding, and nothing else:

  ```rust
  // crates/types-traits/interfaces/src/connector_types.rs — pub trait PayoutCreateV2
  pub trait PayoutCreateV2:
      ConnectorIntegrationV2<
      connector_flow::PayoutCreate,
      PayoutFlowData,
      PayoutCreateRequest,
      PayoutCreateResponse,
  >
  {
  }
  ```

- **Service trait**: `interfaces::connector_types::PayoutServiceTrait` — `crates/types-traits/interfaces/src/connector_types.rs`.
- **Stub macro entry point**: `macros::macro_connector_payout_implementation!` — `crates/integrations/connector-integration/src/connectors/macros.rs` (delegates per flow to `expand_payout_implementation!` in the same file).
- **Reference implementation**: `crates/integrations/connector-integration/src/payout_connectors/santander.rs` (`impl PayoutCreateV2 for SantanderPayouts` and the `ConnectorIntegrationV2<PayoutCreate, …>` block below it), with transformers in `crates/integrations/connector-integration/src/payout_connectors/santander/transformers.rs`.

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


## Table of Contents

1. [Overview](#overview)
2. [Architecture Overview](#architecture-overview)
3. [Connectors with Full Implementation](#connectors-with-full-implementation)
4. [Common Implementation Patterns](#common-implementation-patterns)
5. [Connector-Specific Patterns](#connector-specific-patterns)
6. [Code Examples](#code-examples)
7. [Integration Guidelines](#integration-guidelines)
8. [Best Practices](#best-practices)
9. [Common Errors / Gotchas](#common-errors--gotchas)
10. [Testing Notes](#testing-notes)
11. [Cross-References](#cross-references)

## Architecture Overview

```
PayoutService::create (gRPC)
    │   crates/grpc-server/grpc-server/src/server/payouts.rs
    ▼
internal_payout_create
    │   crates/grpc-server/grpc-server/src/server/payouts.rs
    ▼
RouterDataV2<PayoutCreate, PayoutFlowData,
             PayoutCreateRequest, PayoutCreateResponse>
    │
    ├─▶ ConnectorIntegrationV2<PayoutCreate, ...>::get_url/headers/body
    │       (connector-specific impl on Connector<T>)
    │
    ├─▶ transport (HTTP)
    │
    └─▶ ConnectorIntegrationV2::handle_response_v2
            -> PayoutCreateResponse (status, ids, status_code)
```

The generic router-data template is fixed at four type arguments (see `PATTERN_AUTHORING_SPEC.md` §7):

```rust
RouterDataV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>
// from crates/types-traits/domain_types/src/router_data_v2.rs
```

### Flow Type

`domain_types::connector_flow::PayoutCreate` — unit marker struct declared at `crates/types-traits/domain_types/src/connector_flow.rs`. It is carried through `RouterDataV2` in a `PhantomData<PayoutCreate>` field (`crates/types-traits/domain_types/src/router_data_v2.rs`).

### Request Type

`domain_types::payouts::payouts_types::PayoutCreateRequest` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`. Shape:

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs
#[derive(Debug, Clone)]
pub struct PayoutCreateRequest {
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
    pub source_bank_data: Option<Bank>,
}
```

`Bank` comes from `domain_types::payouts::payout_method_data::Bank`; `source_bank_data` carries the
debtor (source) account for rails such as PIX and SEPA.

### Response Type

`domain_types::payouts::payouts_types::PayoutCreateResponse` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`. Shape:

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs
#[derive(Debug, Clone)]
pub struct PayoutCreateResponse {
    pub merchant_payout_id: Option<String>,
    pub payout_status: common_enums::PayoutStatus,
    pub connector_payout_id: Option<String>,
    pub status_code: u16,
}
```

`common_enums::PayoutStatus` is the target status enum (`crates/common/common_enums/src/enums.rs`).

### Resource Common Data

`domain_types::payouts::payouts_types::PayoutFlowData` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`. This is the canonical flow-data type for all payout flows and replaces the payment-side `PaymentFlowData` for the `PayoutCreate` / `PayoutTransfer` / `PayoutGet` trio. Unlike `PaymentFlowData`, it does not carry an `AttemptStatus`; status is carried inside the typed response (`PayoutCreateResponse.payout_status`). Shape:

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

Thirteen fields — note `connectors` is `Arc<Connectors>`, not `Connectors`, and that the `typed_*`
pair exists alongside the `raw_*` pair (they are written by
`RawConnectorRequestResponse`, implemented for `PayoutFlowData` in the same file).

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


## Connectors with Full Implementation

| Connector | HTTP Method | Content Type | URL Pattern | Request Type | Notes |
| --------- | ----------- | ------------ | ----------- | ------------ | ----- |
| `SantanderPayouts` | `POST` | `application/json` | `{base_url}/management_payments_partners/v1/workspaces/{workspace_id}/pix_payments` | `SantanderCreateRequest` (flow-local) | Only full impl. Non-generic unit struct; mutual-TLS via `get_certificate` / `get_certificate_key` reading `SantanderAuthType`; `workspace_id` comes from the auth type, not the request. Response handled through the `finalize_connector_response!` macro (`crates/integrations/connector-integration/src/utils.rs`). |

### Current implementation coverage

**1 of 10 payout connectors.** Verified with:

```bash
rg -n 'impl ConnectorIntegrationV2<PayoutCreate' crates/integrations/connector-integration/src/payout_connectors/
```

which returns `cybersource.rs`, `itaubank.rs`, `loonio.rs`, `paypal.rs`, `santander.rs` and
`worldpayxml.rs` — of which only `santander.rs` overrides more than `get_url`.

### Stub Implementations

- **Hand-written stubs** (non-generic connectors, which cannot use the payout macro):
  `cybersource.rs`, `itaubank.rs`, `loonio.rs`, `paypal.rs`, `worldpayxml.rs` each carry an
  `impl PayoutCreateV2 for <Name>Payouts {}` plus a one-method
  `ConnectorIntegrationV2<PayoutCreate, …>` block whose `get_url` returns
  `IntegrationError::connector_flow_not_implemented(self.id(), "payout_create", …)`.
  `truelayer.rs` generates the same shape from its file-local
  `macro_rules! impl_unimplemented_payout_flow!`.
- **Macro stubs** (generic connectors): `deutschebank.rs`, `gotyme_sanlam.rs` and `trustly.rs` list
  `PayoutCreate` in the `payout_flows: [...]` array of
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


## Connector-Specific Patterns

### santander (the only full implementation)

- **Struct**: `pub struct SantanderPayouts;` — a non-generic unit struct with
  `pub const fn new() -> &'static Self`. Because it has no type parameter it cannot use
  `macro_connector_payout_implementation!`; all nine flows are written out by hand.
- **Two-step product**: `PayoutCreate` (`POST …/pix_payments`) creates the PIX payment; the real
  fulfilment is `PayoutTransfer` (`PATCH`, see the `// ===== PAYOUT TRANSFER` banner in the same
  file). Do not model `PayoutCreate` as terminal.
- **mTLS**: `get_certificate` and `get_certificate_key` both build `SantanderAuthType` from
  `req.connector_config` and return `auth.certificates` / `auth.private_key`. This is the payout-side
  idiom for client-certificate connectors.
- **Access token**: `get_headers` calls `req.resource_common_data.get_access_token()?` and passes it
  to the file-local `get_api_headers` helper alongside `auth.client_id`.
- **Response**: `handle_response_v2` parses `SantanderPayoutResponse` and then delegates entirely to
  `finalize_connector_response!(event_builder, response, data, res.status_code)`, which builds the
  `RouterDataV2` through `TryFrom<ResponseRouterData<…>>` and sets `typed_connector_response`.

### itaubank

- **Current coverage**: hand-written `PayoutCreate` stub only; itaubank's real logic is
  `PayoutTransfer` and `PayoutGet` in
  `crates/integrations/connector-integration/src/payout_connectors/itaubank.rs`.
- **Env-specific URL**: the free function `build_env_specific_endpoint(base_url, test_mode)` in
  `payout_connectors/itaubank.rs` switches the path suffix on
  `resource_common_data.test_mode`; a `PayoutCreate` URL builder would reuse it.
- There is **no** `ValidationTrait` impl on `ItaubankPayouts` and no `should_do_access_token` hook on
  the payout path — see the access-token note above.

## Code Examples

### Example 1: Canonical `ConnectorIntegrationV2<PayoutCreate, ...>` skeleton

Transcribed from the live `SantanderPayouts` impl in
`crates/integrations/connector-integration/src/payout_connectors/santander.rs` (the block under the
`// ===== PAYOUT CREATE` banner). Note the marker impl that must precede it, and that
`get_request_body` returns **`Option<ConnectorRequestData>`**, not `Option<RequestContent>` — see
`fn get_request_body` on `ConnectorIntegrationV2` in
`crates/types-traits/interfaces/src/connector_integration_v2.rs`.

```rust
// Shape from crates/integrations/connector-integration/src/payout_connectors/santander.rs
impl PayoutCreateV2 for MyConnectorPayouts {}

impl ConnectorIntegrationV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>
    for MyConnectorPayouts
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
    }

    fn get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn get_url(
        &self,
        req: &RouterDataV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>,
    ) -> CustomResult<String, IntegrationError> {
        let base_url = self.base_url(&req.resource_common_data.connectors);
        let auth = MyConnectorAuthType::try_from(&req.connector_config)?;
        let workspace_id = &auth.workspace_id;
        Ok(format!("{base_url}/v1/workspaces/{workspace_id}/payments"))
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        // The token was fetched by MerchantAuthenticationService and handed back on the
        // payout RPC; PayoutFlowData::get_access_token() surfaces it.
        let access_token = req.resource_common_data.get_access_token()?;
        let auth = MyConnectorAuthType::try_from(&req.connector_config)?;
        Ok(get_api_headers(&access_token, &auth.client_id.expose()))
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>,
    ) -> CustomResult<Option<ConnectorRequestData>, IntegrationError> {
        let connector_req = MyConnectorCreateRequest::try_from(req)?;
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
        data: &RouterDataV2<
            PayoutCreate,
            PayoutFlowData,
            PayoutCreateRequest,
            PayoutCreateResponse,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>,
        ConnectorError,
    > {
        let response: MyConnectorPayoutResponse = res
            .response
            .parse_struct("MyConnectorPayoutResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: ResponseTransformationErrorContext {
                    additional_context: Some("Failed to deserialize payout response".to_string()),
                    http_status_code: Some(res.status_code),
                },
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

`finalize_connector_response!` is defined in
`crates/integrations/connector-integration/src/utils.rs`; it builds the `RouterDataV2` through
`TryFrom<ResponseRouterData<…>>`, sets the event response data and writes
`typed_connector_response`. Prefer it over hand-rolling the twelve-line block.

### Example 2: How the macro emits the current `PayoutCreate` stub on itaubank

```rust
// crates/integrations/connector-integration/src/connectors/macros.rs
//   — macro_rules! expand_payout_implementation, the `flow: PayoutCreate` arm
macro_rules! expand_payout_implementation {
    (
        connector: $connector: ident,
        flow: PayoutCreate,
        generic_type: $generic_type:tt,
        [ $($bounds:tt)* ]
    ) => {
        impl<$generic_type: $($bounds)*> ::interfaces::connector_types::PayoutCreateV2 for $connector<$generic_type> {}
        impl<$generic_type: $($bounds)*>
            ::interfaces::connector_integration_v2::ConnectorIntegrationV2<
                ::domain_types::connector_flow::PayoutCreate,
                ::domain_types::payouts::payouts_types::PayoutFlowData,
                ::domain_types::payouts::payouts_types::PayoutCreateRequest,
                ::domain_types::payouts::payouts_types::PayoutCreateResponse,
            > for $connector<$generic_type>
        {
            fn get_url(
                &self,
                _req: &::domain_types::router_data_v2::RouterDataV2<
                    ::domain_types::connector_flow::PayoutCreate,
                    ::domain_types::payouts::payouts_types::PayoutFlowData,
                    ::domain_types::payouts::payouts_types::PayoutCreateRequest,
                    ::domain_types::payouts::payouts_types::PayoutCreateResponse,
                >,
            ) -> ::common_utils::CustomResult<String, ::domain_types::errors::IntegrationError> {
                Err(::domain_types::errors::IntegrationError::connector_flow_not_implemented(
                    ::interfaces::api::ConnectorCommon::id(self),
                    "payout_create",
                    ::domain_types::errors::IntegrationErrorContext::default(),
                ).into())
            }
        }
    };
```

The stub is **not** an empty `{}` body: `get_url` is overridden so the call fails immediately with
`IntegrationError::connector_flow_not_implemented(id, "payout_create", …)`. Any connector that lists
`PayoutCreate` in the macro's `payout_flows: [...]` array is in this state — and, because the macro
matches `$connector<$generic_type>`, only connectors with a generic parameter
(`DeutschebankPayouts<T>`, `GotymeSanlamPayouts<T>`, `TrustlyPayouts<T>`) can use it at all.

### Example 3: Request-struct `TryFrom` shape (mirroring itaubank `PayoutTransfer`)

The live reference is `impl TryFrom<&RouterDataV2<PayoutCreate, …>> for SantanderCreateRequest` in
`crates/integrations/connector-integration/src/payout_connectors/santander/transformers.rs`; the
itaubank equivalent for `PayoutTransfer` (`ItaubankTransferRequest`, in
`payout_connectors/itaubank/transformers.rs`) has the same shape.

```rust
// Shape from crates/integrations/connector-integration/src/payout_connectors/santander/transformers.rs
//   — impl TryFrom<&RouterDataV2<PayoutCreate, ...>> for SantanderCreateRequest
impl TryFrom<
    &RouterDataV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>,
> for MyConnectorCreateRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        req: &RouterDataV2<
            PayoutCreate,
            PayoutFlowData,
            PayoutCreateRequest,
            PayoutCreateResponse,
        >,
    ) -> Result<Self, Self::Error> {
        let converter = StringMajorUnitForConnector;
        let amount = converter
            .convert(req.request.amount, req.request.source_currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;
        Ok(Self {
            amount,
            currency: req.request.destination_currency.to_string(),
            reference: req.request.merchant_payout_id.clone(),
            payout_method: map_payout_method(&req.request.payout_method_data)?,
        })
    }
}
```

### Example 4: Status mapping idiom (mirroring itaubank)

Map a connector-local enum into `common_enums::PayoutStatus` rather than hardcoding it. The
in-tree idiom is an inherent `get_payout_status()` method on the connector's status enum:

```rust
// crates/integrations/connector-integration/src/payout_connectors/santander/transformers.rs
//   — impl SantanderPayoutStatus
impl SantanderPayoutStatus {
    pub fn get_payout_status(&self) -> common_enums::PayoutStatus {
        match self {
            Self::Authorized | Self::PendingConfirmation => common_enums::PayoutStatus::Pending,
            Self::ReadyToPay => common_enums::PayoutStatus::RequiresFulfillment,
            Self::Payed => common_enums::PayoutStatus::Success,
            Self::Rejected => common_enums::PayoutStatus::Failure,
        }
    }
}
```

`payout_connectors/itaubank/transformers.rs` uses the identical shape —
`impl ItaubankPayoutStatus { pub fn get_payout_status(&self) -> common_enums::PayoutStatus }`,
mapping `Aprovado`/`Confirmado`/`Efetivado`/`Sucesso` → `Success`,
`Pendente`/`EmProcessamento` → `Pending`, `Rejeitado`/`Cancelado`/`NaoIncluido` → `Failure`, and the
`#[serde(other)] Unknown` catch-all → `Pending`.

## Integration Guidelines

1. **Create the connector under `payout_connectors/`, not `connectors/`.** Add
   `payout_connectors/<name>.rs` and `payout_connectors/<name>/transformers.rs`, then export both
   from `payout_connectors.rs` — the module file, a sibling of the `payout_connectors/` directory (`pub mod <name>; pub use self::<name>::<Name>Payouts;`).
2. **Pick an authoring style.** A non-generic unit struct (`pub struct <Name>Payouts;` with
   `pub const fn new() -> &'static Self`) means every flow is hand-written — that is what
   `santander.rs` does. A generic struct built by `macros::create_all_prerequisites!` can use
   `macros::macro_connector_implementation!` per real flow and
   `macros::macro_connector_payout_implementation!` for the rest — that is what
   `deutschebank.rs`, `gotyme_sanlam.rs` and `trustly.rs` do.
3. **Implement `PayoutCreateV2`.** `impl PayoutCreateV2 for <Name>Payouts {}` satisfies the marker;
   the real work is the `ConnectorIntegrationV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest,
   PayoutCreateResponse>` block. If the macro is already generating `PayoutCreate`, remove it from
   `payout_flows: [...]` first or the build fails with a conflicting implementation.
4. **Cover the other eight flows.** `PayoutServiceTrait` requires all nine `Payout*V2` markers plus
   `ConnectorCommon` and `ServerAuthentication`; a missing one is a compile error on
   `impl PayoutServiceTrait for <Name>Payouts {}`, not a runtime gap.
5. **Add request/response transformers** with `TryFrom<&RouterDataV2<PayoutCreate, …>>` for the
   request struct and `TryFrom<&ResponseRouterData<…>>` for `PayoutCreateResponse` — see
   `payout_connectors/santander/transformers.rs`.
6. **Map `payout_status` from the connector payload.** Never hardcode `common_enums::PayoutStatus`;
   use a `get_payout_status()` method on the connector status enum.
7. **Propagate `res.status_code` into `PayoutCreateResponse.status_code`** — `finalize_connector_response!`
   plus a `TryFrom<&ResponseRouterData<…>>` that copies `item.http_code` does this for you.
8. **Return `ConnectorCommon::build_error_response` from `get_error_response_v2`.**
9. **Register the connector at all six sites** listed in the scope note above (the four shared wiring sites below plus the connector file itself and its `payout_connectors.rs` export):
   `PayoutConnectorEnum` (`crates/types-traits/domain_types/src/connector_types.rs`),
   `PayoutConnectorData::convert_connector` (`crates/integrations/connector-integration/src/types.rs`),
   the `Connectors` field and the `patch_payout_connector_urls` arm
   (`crates/types-traits/domain_types/src/types.rs`).
10. **Do not add a `config/superposition.toml` block or a `connector_specs/<connector>/specs.json`.**
    `check_connector_specs.rs` enumerates connectors from `connectors/` only, never
    `payout_connectors/`, and `PayoutService` is in its `IGNORE_SERVICES` (and
    `check_coverage.rs`'s). Payout connectors therefore sit outside the merge-blocking
    connector-certification gate that `.github/scripts/verify-new-connectors.sh` enforces.
11. **No gRPC wiring is needed.** `service PayoutService` in
    `crates/types-traits/grpc-api-types/proto/services.proto` and the nine
    `implement_connector_operation!` invocations in
    `crates/grpc-server/grpc-server/src/server/payouts.rs` are flow-generic and already dispatch
    through `PayoutConnectorData`.

## Best Practices

- **Read the access token, do not sequence it.** Call
  `req.resource_common_data.get_access_token()?` (`PayoutFlowData::get_access_token` in
  `crates/types-traits/domain_types/src/payouts/payouts_types.rs`) and let its
  `missing_field_err("access_token")` surface; the token arrives on the payout RPC itself, so there
  is no `should_do_access_token` hook to set. `payout_connectors/santander.rs` does exactly this in
  `get_headers`.
- **Keep request-body construction in `TryFrom`, not in `get_request_body`.** `get_request_body`
  should build the connector struct, wrap it with `ConnectorRequestData::new(RequestContent::Json(…),
  typed)` and propagate errors — mirror `payout_connectors/santander.rs`.
- **Use `StringMajorUnitForConnector` (or the correct converter) for amount conversion, then bubble
  errors with `change_context`.** See
  `payout_connectors/santander/transformers.rs` (`SantanderCreateRequest::try_from`) and
  `payout_connectors/itaubank/transformers.rs` (`ItaubankTransferRequest::try_from`).
- **Fall back to `common_enums::PayoutStatus::Pending` on unknown connector statuses rather than
  `Failure`.** `ItaubankPayoutStatus` declares a `#[serde(other)] Unknown` variant precisely so the
  catch-all can map to `Pending`; see `ItaubankPayoutStatus::get_payout_status` in
  `payout_connectors/itaubank/transformers.rs`.
- **Do not write `ValidationTrait`, `IncomingWebhook`, `VerifyRedirectResponse`, `SourceVerification`
  or `BodyDecoding` impls for a payout-only connector.** `PayoutServiceTrait` does not require any of
  them; they are payment-side obligations.
- **Do not hand-roll amount math or error types.** Use `common_utils::types` amount converters and `IntegrationError` / `ConnectorError` exclusively (see `PATTERN_AUTHORING_SPEC.md` §12 retired-types list).
- **Cross-reference `utility_functions_reference.md` for shared helpers such as `build_env_specific_endpoint`-style URL builders.**

## Common Errors / Gotchas

1. **Problem**: A macro- or hand-written stub masks a missing real implementation, so the connector
   appears to "support" `PayoutCreate` while every call fails.
   **Solution**: The stub is loud, not silent — it returns
   `IntegrationError::connector_flow_not_implemented(id, "payout_create", …)` from `get_url`. Remove
   `PayoutCreate` from `payout_flows: [...]` (or delete the hand-written stub `impl`) *before*
   adding the real one; leaving both is a conflicting-implementation compile error.

1a. **Problem**: Calling `macro_connector_payout_implementation!` on a non-generic connector struct.
   **Solution**: The macro only matches `$connector<$generic_type>`. `pub struct SantanderPayouts;`
   has no type parameter, so the invocation will not expand. Either give the struct a
   `PhantomData<T>`-style parameter (as `TrustlyPayouts<T>` does) or hand-write the stubs — see the
   file-local `macro_rules! impl_unimplemented_payout_flow!` in
   `payout_connectors/truelayer.rs`.

2. **Problem**: Confusing `PayoutCreateRequest` with `PaymentsAuthorizeData<T>` — they are different types on different traits.
   **Solution**: Always use the four-argument form `RouterDataV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>`. The request type is defined at `crates/types-traits/domain_types/src/payouts/payouts_types.rs` and has no generic parameter, unlike `PaymentsAuthorizeData<T>`.

3. **Problem**: Copying the payment-side access-token idiom and writing
   `impl ValidationTrait for <Name>Payouts { fn should_do_access_token(&self) -> bool { true } }`.
   **Solution**: `PayoutServiceTrait` does not require `ValidationTrait` and nothing on the payout
   path reads `should_do_access_token`. The token is fetched over `MerchantAuthenticationService`
   (the `ServerAuthentication` binding uses `MerchantAuthenticationFlowData`, not `PayoutFlowData`)
   and handed back on the payout RPC's `optional SecretString access_token`
   (`crates/types-traits/grpc-api-types/proto/payouts.proto`), which
   `crates/types-traits/domain_types/src/payouts/types.rs` copies into
   `PayoutFlowData.access_token`.

4. **Problem**: Serializing `common_utils::types::MinorUnit` directly to a connector that expects a string-formatted major unit.
   **Solution**: Convert via `StringMajorUnitForConnector` (or the appropriate converter) and store the converted value in the connector-local request struct, as at `crates/integrations/connector-integration/src/payout_connectors/itaubank/transformers.rs`.

5. **Problem**: Hardcoding `payout_status: PayoutStatus::Success` in `handle_response_v2`.
   **Solution**: Map from a typed response-status enum — see the `ItaubankTransferResponse::status` function at `crates/integrations/connector-integration/src/payout_connectors/itaubank/transformers.rs` and return the mapped value in the `PayoutCreateResponse`.

## Testing Notes

### Unit-test shape

There is no `connector_specs` suite for payouts — `PayoutService` is in `IGNORE_SERVICES` in
`crates/internal/integration-tests/src/bin/check_connector_specs.rs`, so the merge-blocking
certification gate does not exercise this flow. The in-repo gRPC-level coverage is
`crates/grpc-server/grpc-server/tests/payout_flows_test.rs`. For a new `PayoutCreate`
implementation, the minimum unit-test surface is:

- Request-struct `TryFrom` coverage: one test per supported `PayoutMethodData` variant (`crates/types-traits/domain_types/src/payouts/payout_method_data.rs`).
- Status mapping coverage: one test per branch of the connector-status → `common_enums::PayoutStatus` match.
- Amount-conversion coverage: verify `StringMajorUnit` (or other) formatting for each supported currency.

### Integration-test scenarios

| Scenario | Setup | Expected `payout_status` |
| -------- | ----- | ------------------------ |
| Happy path — full create with valid payout method | Real access token; valid `payout_method_data` | `Success` or `Pending` (never hardcoded) |
| Missing access token | `PayoutFlowData.access_token = None` | `IntegrationError::FailedToObtainAuthType` |
| Invalid payout method mapping | `payout_method_data` variant the connector does not support | `IntegrationError::InvalidDataFormat` |
| Connector rejection | Mocked 4xx response | `ErrorResponse` surfaced via `build_error_response` |
| Malformed JSON response | Mocked non-parsable 2xx body | `ConnectorError::ResponseDeserializationFailed` |

Integration tests MUST describe real sandbox flows (see `PATTERN_AUTHORING_SPEC.md` §11); do not mock the HTTP layer for documented tests.

## Cross-References

- Parent index: [./README.md](./README.md)
- Authoring spec: [./PATTERN_AUTHORING_SPEC.md](./PATTERN_AUTHORING_SPEC.md)
- Sibling flow: [pattern_payout_transfer.md](./pattern_payout_transfer.md)
- Sibling flow: [pattern_payout_get.md](./pattern_payout_get.md)
- Utility helpers: [../utility_functions_reference.md](../utility_functions_reference.md)
- Reference implementation: `crates/integrations/connector-integration/src/payout_connectors/santander.rs`
- Registry: `crates/integrations/connector-integration/src/payout_connectors.rs`
