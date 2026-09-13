# PayoutGet Flow Pattern

## Overview

The `PayoutGet` flow retrieves the current state of a previously-created payout from the connector. It is the payout-side analogue of `PSync` / `RSync`: no money moves, no body is typically sent, and the response carries only an updated `PayoutStatus` plus identifiers. The flow is driven by the `PayoutService::get` gRPC handler (`crates/grpc-server/grpc-server/src/server/payouts.rs`) and dispatched through `internal_payout_get` (`crates/grpc-server/grpc-server/src/server/payouts.rs`) under the `FlowName::PayoutGet` marker (`crates/types-traits/domain_types/src/connector_flow.rs`).

**Nine of the ten payout connectors implement this flow for real** — `deutschebank`,
`gotyme_sanlam`, `itaubank`, `loonio`, `paypal`, `santander`, `truelayer`, `trustly` and
`worldpayxml`, all under `crates/integrations/connector-integration/src/payout_connectors/`. Only
`cybersource` still carries a stub. `PayoutGet` is therefore the second-best-covered payout flow
after `PayoutTransfer`, and this document uses `ItaubankPayouts` as the hand-written reference.

Verify the roster before trusting it:

```bash
rg -n 'ConnectorIntegrationV2<\s*PayoutGet|flow_name: PayoutGet' \
   crates/integrations/connector-integration/src/payout_connectors/
```

### Key Components

- **Flow marker**: `domain_types::connector_flow::PayoutGet` — `crates/types-traits/domain_types/src/connector_flow.rs`.
- **Flow data**: `domain_types::payouts::payouts_types::PayoutFlowData` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`.
- **Request data**: `domain_types::payouts::payouts_types::PayoutGetRequest` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`.
- **Response data**: `domain_types::payouts::payouts_types::PayoutGetResponse` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`.
- **Marker trait**: `interfaces::connector_types::PayoutGetV2` — `crates/types-traits/interfaces/src/connector_types.rs`, defined solely as the supertrait binding:

  ```rust
  // crates/types-traits/interfaces/src/connector_types.rs — pub trait PayoutGetV2
  pub trait PayoutGetV2:
      ConnectorIntegrationV2<
      connector_flow::PayoutGet,
      PayoutFlowData,
      PayoutGetRequest,
      PayoutGetResponse,
  >
  {
  }
  ```

- **Service trait**: `interfaces::connector_types::PayoutServiceTrait` — `crates/types-traits/interfaces/src/connector_types.rs`.
- **Integrity object**: `domain_types::payouts::router_request_types::PayoutGetIntegrityObject` — `crates/types-traits/domain_types/src/payouts/router_request_types.rs`.
- **Reference implementation**: `crates/integrations/connector-integration/src/payout_connectors/itaubank.rs` (the block under the `// ===== PAYOUT GET (REAL) =====` banner).

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
PayoutService::get (gRPC)
    │   crates/grpc-server/grpc-server/src/server/payouts.rs
    ▼
internal_payout_get
    │   crates/grpc-server/grpc-server/src/server/payouts.rs
    ▼
(caller first obtains a token over MerchantAuthenticationService and passes it
 back on the payout RPC as `access_token`; there is no should_do_access_token
 hook on the payout path — see the note above)
    ▼
RouterDataV2<PayoutGet, PayoutFlowData,
             PayoutGetRequest, PayoutGetResponse>
    │
    ├─▶ ConnectorIntegrationV2<PayoutGet, ...>::get_url / get_headers
    │       typically HTTP GET, no request body
    │
    ├─▶ transport (HTTP GET)
    │
    └─▶ ConnectorIntegrationV2::handle_response_v2
            -> PayoutGetResponse (payout_status, identifiers, status_code)
```

The generic router-data template (per `PATTERN_AUTHORING_SPEC.md` §7):

```rust
RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>
// from crates/types-traits/domain_types/src/router_data_v2.rs
```

### Flow Type

`domain_types::connector_flow::PayoutGet` — unit marker struct at `crates/types-traits/domain_types/src/connector_flow.rs`. Threaded through `RouterDataV2.flow: PhantomData<PayoutGet>` (`crates/types-traits/domain_types/src/router_data_v2.rs`).

### Request Type

`domain_types::payouts::payouts_types::PayoutGetRequest` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`. Shape:

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs
#[derive(Debug, Clone)]
pub struct PayoutGetRequest {
    pub merchant_payout_id: Option<String>,
    pub connector_payout_id: Option<String>,
    /// Source (debtor) bank data — required by connectors (e.g. Deutsche Bank)
    /// that need the debtor account to perform a status enquiry.
    pub source_bank_data: Option<Bank>,
}
```

Three fields, not two. Unlike `PayoutCreateRequest` (eleven fields) and `PayoutTransferRequest`
(fifteen), the `Get` variant carries no amount, no currency and no payout-method data — but it does
carry `source_bank_data: Option<Bank>` (`domain_types::payouts::payout_method_data::Bank`), because
some rails (Deutsche Bank's SEPA status enquiry, for one) require the debtor account to look a payout
up. Connectors that take the id from the path — itaubank, santander, paypal — simply ignore it.

### Response Type

`domain_types::payouts::payouts_types::PayoutGetResponse` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`. Shape:

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs
#[derive(Debug, Clone)]
pub struct PayoutGetResponse {
    pub merchant_payout_id: Option<String>,
    pub payout_status: common_enums::PayoutStatus,
    pub connector_payout_id: Option<String>,
    pub status_code: u16,
}
```

`common_enums::PayoutStatus` is declared at `crates/common/common_enums/src/enums.rs`.

### Resource Common Data

`domain_types::payouts::payouts_types::PayoutFlowData` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`. Thirteen fields; identical struct to the one used in `PayoutCreate` and `PayoutTransfer` (note `connectors` is `Arc<Connectors>`). For `PayoutGet` specifically:

- The `payout_id` field is the server-side handle; it is populated from the gRPC request by `PayoutFlowData::foreign_try_from` (`crates/types-traits/domain_types/src/payouts/types.rs`).
- `access_token` gates authenticated reads via `PayoutFlowData::get_access_token` (`crates/types-traits/domain_types/src/payouts/payouts_types.rs`).
- `connector_request_reference_id` is derived from the incoming `merchant_payout_id` by `extract_connector_request_reference_id` (`crates/types-traits/domain_types/src/payouts/types.rs`).

### Integrity object

`PayoutGetIntegrityObject` at `crates/types-traits/domain_types/src/payouts/router_request_types.rs`
carries exactly `merchant_payout_id: Option<String>` and `connector_payout_id: Option<String>` — no
amount, no currency — reflecting the read-only nature of the flow. (`PayoutVoidIntegrityObject` in
the same file has the identical shape.)

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

Every path below is relative to `crates/integrations/connector-integration/src/payout_connectors/`.

| Connector | Style | HTTP | URL / lookup key | Notes |
| --------- | ----- | ---- | ---------------- | ----- |
| `ItaubankPayouts` | hand-written | `GET` | `{env_base}v1/pagamentos_sispag/{connector_payout_id}` | Requires `connector_payout_id`; `None` raises `IntegrationError::MissingConnectorTransactionID`. mTLS. Response `ItaubankPayoutGetResponse`. `itaubank.rs`. |
| `SantanderPayouts` | hand-written | `GET` | PIX payment by id | Response `SantanderStatusResponse`. `santander.rs`. |
| `PaypalPayouts` | hand-written | `GET` | Payouts batch by id | `paypal.rs`. |
| `WorldpayxmlPayouts` | hand-written | `POST` (XML enquiry) | XML enquiry envelope | `worldpayxml.rs`. |
| `LoonioPayouts` | hand-written | `GET` | payout by id | `loonio.rs`. |
| `TruelayerPayouts` | hand-written | `GET` | payout by id | `truelayer.rs`. |
| `TrustlyPayouts<T>` | macro | `POST` | Trustly JSON-RPC | `flow_name: PayoutGet`, `TrustlyPayoutSyncRequest` → `TrustlyPayoutSyncResponse`. `trustly.rs`. |
| `DeutschebankPayouts<T>` | macro | `POST` | SEPA status enquiry | `DeutschebankStatusRequest` → `DeutschebankStatusResponse`; this is the connector that needs `PayoutGetRequest.source_bank_data`. `deutschebank.rs`. |
| `GotymeSanlamPayouts<T>` | macro | `POST` | GoTyme status enquiry | `GotymeSanlamPayoutGetRequest` → `GotymeSanlamPayoutGetResponse`. `gotyme_sanlam.rs`. |

Note that `PayoutGet` is **not** always an HTTP `GET`: `worldpayxml`, `trustly`, `deutschebank` and
`gotyme_sanlam` all issue `POST`s. Read the connector's API, not the flow name.

### Current implementation coverage

**9 of 10 payout connectors.** Verified with:

```bash
rg -n 'ConnectorIntegrationV2<\s*PayoutGet' crates/integrations/connector-integration/src/payout_connectors/   # 6 hand-written + 1 stub
rg -n 'flow_name: PayoutGet'                 crates/integrations/connector-integration/src/payout_connectors/   # 3 macro-driven
```

### Stub Implementations

- `cybersource.rs` — the only remaining stub. `impl PayoutGetV2 for CybersourcePayouts {}` plus a
  one-method `ConnectorIntegrationV2<PayoutGet, …>` whose `get_url` returns
  `IntegrationError::connector_flow_not_implemented(self.id(), "payout_get", …)`.

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

### Hand-written track, step by step (as `ItaubankPayouts` does it)

1. `impl PayoutGetV2 for <Name>Payouts {}` — the marker.
2. `impl ConnectorIntegrationV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse> for <Name>Payouts`
   with `get_http_method` returning `Method::Get`, `get_content_type`, `get_url`, `get_headers`,
   `handle_response_v2` and `get_error_response_v2`. `get_request_body` is simply omitted — the
   `ConnectorIntegrationV2` default already returns `Ok(None)`, which is what a bodyless `GET` needs.
   itaubank additionally overrides `get_certificate` / `get_certificate_key` for mTLS.
3. Derive the path identifier from `req.request.connector_payout_id`; itaubank raises
   `IntegrationError::MissingConnectorTransactionID` when it is `None` rather than silently falling
   back to `merchant_payout_id`.
4. Parse into a connector-local response struct and hand off to
   `finalize_connector_response!(event_builder, response, data, res.status_code)`.

### Macro track, step by step (as `GotymeSanlamPayouts<T>` does it)

1. Add a `(flow: PayoutGet, request_body: …, response_body: …, router_data: RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>)`
   entry to `macros::create_all_prerequisites!`.
2. `macros::macro_connector_implementation!( … flow_name: PayoutGet, resource_common_data: PayoutFlowData,
   flow_request: PayoutGetRequest, flow_response: PayoutGetResponse, http_method: Post, … )`.
3. Leave `PayoutGet` **out** of `macro_connector_payout_implementation!`'s `payout_flows: [...]`.

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

### itaubank (`ItaubankPayouts`, hand-written — the reference)

- **Full implementation.** See the block under the `// ===== PAYOUT GET (REAL) =====` banner in
  `payout_connectors/itaubank.rs`: `impl PayoutGetV2 for ItaubankPayouts {}` followed by
  `impl ConnectorIntegrationV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse> for ItaubankPayouts`.
- **URL**: `build_env_specific_endpoint(self.base_url(&req.resource_common_data.connectors), req.resource_common_data.test_mode)`
  then `format!("{}v1/pagamentos_sispag/{}", base_url, connector_payout_id)`. Note there is **no**
  slash between the base and `v1/` — `build_env_specific_endpoint` already returns a suffixed path.
- **Missing id is a hard error**: `req.request.connector_payout_id.clone().ok_or(IntegrationError::MissingConnectorTransactionID { context: Default::default() })?`.
  It does not fall back to `merchant_payout_id`.
- **mTLS**: `get_certificate` / `get_certificate_key` return `ItaubankAuthType::try_from(&req.connector_config)?`'s
  `certificates` / `private_key`.
- **Headers** are the same five as the `PayoutTransfer` impl, including the connector-specific
  `headers::X_ITAU_API_KEY` carrying `auth.client_id`.
- **Response**: `ItaubankPayoutGetResponse` (with nested `ItaubankPayoutGetData` /
  `ItaubankPayoutDetails`) in `payout_connectors/itaubank/transformers.rs`, mapped through
  `ItaubankPayoutStatus::get_payout_status()`.

### cybersource (`CybersourcePayouts`, the one remaining stub)

- `payout_connectors/cybersource.rs` implements `PayoutTransfer` for real but leaves `PayoutGet` as a
  hand-written stub. It is the only payout connector in that state; treat it as a coverage gap, not
  as a decision that the flow should never be covered.

### deutschebank (`DeutschebankPayouts<T>`, macro-driven)

- The only connector that reads `PayoutGetRequest.source_bank_data`: the SEPA status enquiry needs
  the debtor account. Its `macro_connector_implementation!` call uses
  `curl_request: Json(DeutschebankStatusRequest)`, `curl_response: DeutschebankStatusResponse`,
  `flow_name: PayoutGet`, `resource_common_data: PayoutFlowData`, `http_method: Post`.

## Code Examples

### Example 1: Canonical `ConnectorIntegrationV2<PayoutGet, ...>` skeleton

Transcribed from the live `ItaubankPayouts` `PayoutGet` impl in
`crates/integrations/connector-integration/src/payout_connectors/itaubank.rs` (under the
`// ===== PAYOUT GET (REAL) =====` banner). Note the marker impl that must precede it, and that the
connector struct is a **non-generic** unit struct.

```rust
// Shape from crates/integrations/connector-integration/src/payout_connectors/itaubank.rs
impl PayoutGetV2 for MyConnectorPayouts {}

impl ConnectorIntegrationV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>
    for MyConnectorPayouts
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Get
    }

    fn get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn get_url(
        &self,
        req: &RouterDataV2<
            PayoutGet,
            PayoutFlowData,
            PayoutGetRequest,
            PayoutGetResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        let base_url = self.base_url(&req.resource_common_data.connectors);
        let connector_payout_id = req.request.connector_payout_id.clone().ok_or(
            IntegrationError::MissingConnectorTransactionID {
                context: Default::default(),
            },
        )?;
        Ok(format!("{base_url}/v1/payouts/{connector_payout_id}"))
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<
            PayoutGet,
            PayoutFlowData,
            PayoutGetRequest,
            PayoutGetResponse,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        // The caller supplied the token on the RPC; PayoutFlowData surfaces it.
        let access_token = req.resource_common_data.get_access_token().map_err(|_| {
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
        })?;
        Ok(vec![
            (headers::CONTENT_TYPE.to_string(), "application/json".to_string().into()),
            (headers::AUTHORIZATION.to_string(), format!("Bearer {access_token}").into_masked()),
        ])
    }

    // `get_request_body` is deliberately omitted: the `ConnectorIntegrationV2` trait default
    // already returns `Ok(None)`, which is exactly what a bodyless GET needs. If you do override
    // it, the return type is `CustomResult<Option<ConnectorRequestData>, IntegrationError>` —
    // see `crates/types-traits/interfaces/src/connector_integration_v2.rs`.

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            PayoutGet,
            PayoutFlowData,
            PayoutGetRequest,
            PayoutGetResponse,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
        ConnectorError,
    > {
        let response: MyConnectorGetResponse = res
            .response
            .parse_struct("MyConnectorGetResponse")
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
`RouterDataV2` through `TryFrom<ResponseRouterData<MyConnectorGetResponse, …>>`, sets the event
response data, and writes `typed_connector_response` on the `PayoutFlowData`. Declare that `TryFrom`
in the connector's `transformers.rs`; it is where `merchant_payout_id`, `payout_status`,
`connector_payout_id` and `status_code` (from `item.http_code`) get populated.

### Example 2: The macro-generated stub (what `cybersource` has for this flow)

`cybersource` writes its stub by hand because `CybersourcePayouts` is non-generic, but the shape is
identical to what the macro emits for a generic connector:

```rust
// crates/integrations/connector-integration/src/connectors/macros.rs
//   — macro_rules! expand_payout_implementation, the `flow: PayoutGet` arm
(
    connector: $connector: ident,
    flow: PayoutGet,
    generic_type: $generic_type:tt,
    [ $($bounds:tt)* ]
) => {
    impl<$generic_type: $($bounds)*> ::interfaces::connector_types::PayoutGetV2 for $connector<$generic_type> {}
    impl<$generic_type: $($bounds)*>
        ::interfaces::connector_integration_v2::ConnectorIntegrationV2<
            ::domain_types::connector_flow::PayoutGet,
            ::domain_types::payouts::payouts_types::PayoutFlowData,
            ::domain_types::payouts::payouts_types::PayoutGetRequest,
            ::domain_types::payouts::payouts_types::PayoutGetResponse,
        > for $connector<$generic_type>
    {
        fn get_url(
            &self,
            _req: &::domain_types::router_data_v2::RouterDataV2<
                ::domain_types::connector_flow::PayoutGet,
                ::domain_types::payouts::payouts_types::PayoutFlowData,
                ::domain_types::payouts::payouts_types::PayoutGetRequest,
                ::domain_types::payouts::payouts_types::PayoutGetResponse,
            >,
        ) -> ::common_utils::CustomResult<String, ::domain_types::errors::IntegrationError> {
            Err(::domain_types::errors::IntegrationError::connector_flow_not_implemented(
                ::interfaces::api::ConnectorCommon::id(self),
                "payout_get",
                ::domain_types::errors::IntegrationErrorContext::default(),
            ).into())
        }
    }
};
```

The body is **not** empty: `get_url` is overridden so the stub fails fast with
`IntegrationError::connector_flow_not_implemented(id, "payout_get", …)` instead of falling through
to a trait default. And because the macro matches `$connector<$generic_type>`, only the three generic
payout connectors (`DeutschebankPayouts<T>`, `GotymeSanlamPayouts<T>`, `TrustlyPayouts<T>`) can use
it — non-generic connectors such as `CybersourcePayouts` must hand-write the same shape.

### Example 3: Status-mapper template (mirroring itaubank's real `PayoutTransfer` mapper)

The in-tree idiom puts the mapping on the **status enum**, as an inherent `get_payout_status`
method — see `impl ItaubankPayoutStatus` in
`crates/integrations/connector-integration/src/payout_connectors/itaubank/transformers.rs` and
`impl SantanderPayoutStatus` in `payout_connectors/santander/transformers.rs`:

```rust
// Shape from crates/integrations/connector-integration/src/payout_connectors/itaubank/transformers.rs
impl MyConnectorGetStatus {
    pub fn get_payout_status(&self) -> common_enums::PayoutStatus {
        match self {
            MyConnectorGetStatus::Succeeded | MyConnectorGetStatus::Paid => {
                common_enums::PayoutStatus::Success
            }
            MyConnectorGetStatus::Pending | MyConnectorGetStatus::Processing => {
                common_enums::PayoutStatus::Pending
            }
            MyConnectorGetStatus::Failed | MyConnectorGetStatus::Rejected => {
                common_enums::PayoutStatus::Failure
            }
            MyConnectorGetStatus::Cancelled => common_enums::PayoutStatus::Cancelled,
            // Safe default: do not escalate unknown states to Failure.
            // Rationale mirrors crates/integrations/connector-integration/src/payout_connectors/itaubank/transformers.rs.
            _ => common_enums::PayoutStatus::Pending,
        }
    }
}
```

### Example 4: Connector-local response struct shape

```rust
// Pattern derived from crates/integrations/connector-integration/src/payout_connectors/itaubank/transformers.rs
#[derive(Debug, Deserialize, Serialize)]
pub struct MyConnectorGetResponse {
    #[serde(alias = "id", alias = "payout_id")]
    pub id: String,
    #[serde(alias = "status", alias = "payout_status")]
    pub status: MyConnectorGetStatus,
}
```

Using `#[serde(alias = ...)]` for dual field names is the idiom used by `ItaubankTransferResponse`
(`id`/`cod_pagamento`, `status`/`status_pagamento`) and is applicable to any `PayoutGet` response
struct. itaubank's own `PayoutGet` response is `ItaubankPayoutGetResponse`, with the nested
`ItaubankPayoutGetData` / `ItaubankPayoutDetails` structs, all declared in
`payout_connectors/itaubank/transformers.rs`. Give the status enum a `#[serde(other)] Unknown`
variant so an unrecognised state deserializes rather than failing the parse.

## Integration Guidelines

1. **Create the connector under `payout_connectors/`, not `connectors/`**, and export it from
   `payout_connectors.rs` (the module file, a sibling of the `payout_connectors/` directory).
2. **Choose a style.** Hand-written non-generic (`ItaubankPayouts` and six others) or generic +
   macro (`DeutschebankPayouts<T>`, `GotymeSanlamPayouts<T>`, `TrustlyPayouts<T>`). A flow is covered
   exactly once — never both in `payout_flows: [...]` and as a hand-written impl.
3. **Write `impl PayoutGetV2 for <Name>Payouts {}`.** Satisfies the marker trait at
   `crates/types-traits/interfaces/src/connector_types.rs`.
4. **Write the `ConnectorIntegrationV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>`
   impl.** Use Example 1 as the skeleton. `get_http_method` returns whatever the connector's status
   API actually uses — `Method::Get` for itaubank, santander, paypal, loonio and truelayer;
   `Method::Post` for worldpayxml, trustly, deutschebank and gotyme_sanlam. Omit `get_request_body`
   entirely for a bodyless read; the trait default already returns `Ok(None)`.
5. **Resolve the payout identifier in `get_url` from `req.request.connector_payout_id`.** itaubank
   returns `IntegrationError::MissingConnectorTransactionID` when it is `None` rather than falling
   back to `merchant_payout_id`; prefer that. If the connector needs the debtor account instead of an
   id, read `req.request.source_bank_data` (this is why the field exists — see
   `DeutschebankStatusRequest`).
6. **Read the access token with `req.resource_common_data.get_access_token()`.** Do not add a
   `ValidationTrait` impl; there is no `should_do_access_token` hook on the payout path.
7. **Parse into a connector-local response struct** with `#[serde(alias = …)]` where the connector
   alternates field names, and a `#[serde(other)] Unknown` variant on the status enum.
8. **Map status via `get_payout_status()` on the status enum, not inline literals.**
9. **Return `finalize_connector_response!(event_builder, response, data, res.status_code)` from
   `handle_response_v2`**, backed by a `TryFrom<ResponseRouterData<…>>` in `transformers.rs` that
   copies `item.http_code` into `PayoutGetResponse.status_code`.
10. **Delegate `get_error_response_v2` to `ConnectorCommon::build_error_response`.**
11. **Register in the four wiring sites**: `PayoutConnectorEnum`
    (`crates/types-traits/domain_types/src/connector_types.rs`),
    `PayoutConnectorData::convert_connector`
    (`crates/integrations/connector-integration/src/types.rs`), the `Connectors` field and the
    `patch_payout_connector_urls` arm (`crates/types-traits/domain_types/src/types.rs`).
12. **No `config/superposition.toml` entry and no `connector_specs/<connector>/specs.json` entry.**
    `check_connector_specs.rs` enumerates connectors from `connectors/` only, never
    `payout_connectors/`, and `PayoutService` is in its `IGNORE_SERVICES` (and
    `check_coverage.rs`'s).

## Best Practices

- **Treat `PayoutGet` as idempotent and side-effect-free.** The gRPC contract
  (`rpc Get(PayoutServiceGetRequest) returns (PayoutServiceGetResponse)` in
  `crates/types-traits/grpc-api-types/proto/services.proto`) is a read. The HTTP verb, however, is
  the connector's business: `worldpayxml`, `trustly`, `deutschebank` and `gotyme_sanlam` all `POST` a
  status-enquiry envelope. Use `POST` when the connector's status API requires it — just never let
  the call mutate the payout.
- **Default unknown connector statuses to `Pending`.** The same argument as for `PayoutTransfer` applies — see `crates/integrations/connector-integration/src/payout_connectors/itaubank/transformers.rs`.
- **Derive the URL identifier from the typed request, never from `resource_common_data.payout_id`.** `PayoutGetRequest` is the authoritative source per `crates/types-traits/domain_types/src/payouts/payouts_types.rs`.
- **Mask the `Authorization` header with `into_masked()`** (`crates/integrations/connector-integration/src/payout_connectors/itaubank.rs`).
- **Do not invent a body for a bodyless read.** Omit `get_request_body` entirely; the
  `ConnectorIntegrationV2` default returns `Ok(None)`. If you do need a body, the signature is
  `CustomResult<Option<ConnectorRequestData>, IntegrationError>` — **not** `Option<RequestContent>`
  — and the content is wrapped with
  `ConnectorRequestData::new(RequestContent::Json(Box::new(req)), typed)`.
- **Link to `utility_functions_reference.md` for shared helpers** such as environment-specific URL builders; do not duplicate bodies inline (`PATTERN_AUTHORING_SPEC.md` §11).

## Common Errors / Gotchas

1. **Problem**: `get_url` fails because `connector_payout_id` is `None` on `PayoutGetRequest`.
   **Solution**: Return `IntegrationError::MissingConnectorTransactionID { context: Default::default() }`
   — that is what `payout_connectors/itaubank.rs` does. Do not fall back to an empty string, which
   would produce a malformed URL.

2. **Problem**: Double impl of `ConnectorIntegrationV2<PayoutGet, ...>` because both a stub and a
   real block are present.
   **Solution**: For a generic connector, drop `PayoutGet` from the `payout_flows: [...]` argument to
   `macros::macro_connector_payout_implementation!`. For a non-generic connector, delete the
   hand-written stub `impl` (the one whose only method is a `get_url` returning
   `connector_flow_not_implemented`).

2a. **Problem**: Calling `macro_connector_payout_implementation!` on a non-generic payout connector.
   **Solution**: The macro matches only `$connector<$generic_type>`. `pub struct ItaubankPayouts;`
   has no type parameter, so the invocation will not expand — hand-write the stubs, or copy the
   file-local `macro_rules! impl_unimplemented_payout_flow!` from `payout_connectors/truelayer.rs`.

3. **Problem**: The connector responds with `200 OK` and an empty body on a successfully-completed payout poll; `parse_struct` fails.
   **Solution**: Either make each response-struct field `Option<_>` with `serde(default)`, or treat the empty-body case as `PayoutStatus::Pending` before calling `parse_struct`.

4. **Problem**: Silent `AttemptStatus::Failure` on transient HTTP 5xx from the connector during polling.
   **Solution**: Delegate to `build_error_response` from `get_error_response_v2` (`crates/integrations/connector-integration/src/payout_connectors/itaubank.rs`) so the error surface is propagated as a real `ErrorResponse` rather than being collapsed into a status field.

5. **Problem**: Hardcoding `payout_status` in `handle_response_v2` because the response struct does not have a status field.
   **Solution**: If the connector truly exposes only HTTP status codes, map `res.status_code` in a named helper (e.g. `status_from_http(res.status_code)`) with comments explaining the mapping — never inline a literal `PayoutStatus::Success`. Refer to the status-mapping discipline in `pattern_capture.md`.

6. **Problem**: Trying to make `PayoutGet` fetch its own access token, or adding
   `impl ValidationTrait for <Name>Payouts { fn should_do_access_token(&self) -> bool { true } }`.
   **Solution**: `PayoutServiceTrait` does not require `ValidationTrait` and nothing reads
   `should_do_access_token` on the payout path. `PayoutFlowData.access_token` is filled from the
   RPC's `optional SecretString access_token` by the `ForeignTryFrom` impls in
   `crates/types-traits/domain_types/src/payouts/types.rs`; it is read-only here. The token itself
   comes from the connector's `ServerAuthentication` flow, whose flow data is
   `MerchantAuthenticationFlowData`, not `PayoutFlowData`.

## Testing Notes

### Unit-test shape

There is no `connector_specs` suite for payouts — `PayoutService` is in `IGNORE_SERVICES` in
`crates/internal/integration-tests/src/bin/check_connector_specs.rs`, so `PayoutGet` is outside the
merge-blocking certification gate. The in-repo gRPC-level coverage is
`crates/grpc-server/grpc-server/tests/payout_flows_test.rs`. For a new implementation, recommended
unit coverage:

- **URL construction** — one test with `connector_payout_id` present and one with it absent
  (expect `MissingConnectorTransactionID`).
- **Status-mapper exhaustiveness** — one test per branch of the connector-status match, plus the unknown/default branch.
- **Response parsing** — one test per serde alias combination, plus empty-body handling.
- **Error-response parsing** — confirm that the connector's 4xx bodies round-trip through `build_error_response`.

### Integration-test scenarios

| Scenario | Setup | Expected outcome |
| -------- | ----- | ---------------- |
| Happy path — active payout | Valid sandbox access token; `connector_payout_id` set | `PayoutGetResponse` with `payout_status` mapped from response body |
| Unknown payout ID | Non-existent ID | `ErrorResponse` with connector 404 mapped through `build_error_response` |
| Missing identifier | `connector_payout_id = None` | `IntegrationError::MissingConnectorTransactionID` |
| Missing access token | `PayoutFlowData.access_token = None` | `IntegrationError::FailedToObtainAuthType` |
| Still-processing payout | Sandbox returns intermediate state | `payout_status = Pending` (never hardcoded) |
| Malformed JSON response | Mocked non-parsable body | `ConnectorError::ResponseDeserializationFailed` |

Integration tests MUST describe real sandbox flows per `PATTERN_AUTHORING_SPEC.md` §11.

## Cross-References

- Parent index: [./README.md](./README.md)
- Authoring spec: [./PATTERN_AUTHORING_SPEC.md](./PATTERN_AUTHORING_SPEC.md)
- Sibling flow: [pattern_payout_create.md](./pattern_payout_create.md)
- Sibling flow: [pattern_payout_transfer.md](./pattern_payout_transfer.md)
- Utility helpers: [../utility_functions_reference.md](../utility_functions_reference.md)
- Reference implementation: `crates/integrations/connector-integration/src/payout_connectors/itaubank.rs`
- Registry: `crates/integrations/connector-integration/src/payout_connectors.rs`
