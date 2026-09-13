# FRM Connector Pattern

## Overview

An **FRM connector** (Fraud & Risk Management) scores a transaction for fraud risk and is notified of the outcome afterwards. Unlike payouts, surcharges and authenticators, FRM has **no directory of its own**: the sole FRM connector at HEAD is `crates/integrations/connector-integration/src/connectors/kount.rs` — inside the ordinary payment-connector directory. What makes it an FRM connector is the trait it implements (`FrmServiceTrait`), the enum it appears in (`FrmConnectorEnum`), and the provider that boxes it (`FrmConnectorData`).

Kount is therefore registered **twice**: once as `ConnectorEnum::Kount` in `ConnectorData::convert_connector`, and once as `FrmConnectorEnum::Kount` in `FrmConnectorData::convert_connector`. It implements both `ConnectorServiceTrait<T>` and `FrmServiceTrait`.

Five FRM flow markers, but only **two** dedicated rpcs (`FraudAndRiskManagementService/PreRiskCheck` and `/PostRiskCheck`). The other three — `FrmPaymentOutcome`, `FrmRefundProcessed`, `FrmChargebackReceived` — arrive through `EventService/NotifyConnector`, selected by `NotifyEventType`.

### Key Components

- **Service trait**: `interfaces::connector_types::FrmServiceTrait` — `crates/types-traits/interfaces/src/connector_types.rs`, `pub trait FrmServiceTrait`.
- **Boxed form**: `pub type BoxedFrmConnector = Box<&'static (dyn FrmServiceTrait + Sync)>` — same file. Non-generic `dyn`; this is why the `FrmServiceTrait` impl is monomorphized (see below).
- **Flow data**: `domain_types::frm::frm_types::FrmFlowData` — `crates/types-traits/domain_types/src/frm/frm_types.rs`, `pub struct FrmFlowData`.
- **Flow markers**: `domain_types::connector_flow::{PreRiskCheck, PostRiskCheck, FrmPaymentOutcome, FrmRefundProcessed, FrmChargebackReceived}` — `crates/types-traits/domain_types/src/connector_flow.rs`.
- **Category enum**: `domain_types::connector_types::FrmConnectorEnum` — `crates/types-traits/domain_types/src/connector_types.rs`.
- **Provider**: `connector_integration::types::FrmConnectorData` — `crates/integrations/connector-integration/src/types.rs`.
- **Directory**: `crates/integrations/connector-integration/src/connectors/` (**the payment directory**), registered in `connectors.rs`.
- **Reference connector**: `connectors/kount.rs` + `connectors/kount/transformers.rs`.
- **gRPC services**: `FraudAndRiskManagementService`, `CompositeFraudAndRiskManagementService`, `EventService` — `crates/types-traits/grpc-api-types/proto/services.proto` and `.../composite_services.proto`.
- **Routing header**: `x-frm-connector` — `common_utils::consts::X_FRM_CONNECTOR_NAME`.

## Table of Contents

1. [Overview](#overview)
2. [Architecture Overview](#architecture-overview)
3. [Connectors with Full Implementation](#connectors-with-full-implementation)
4. [Registration Sites](#registration-sites)
5. [Which Macros Apply](#which-macros-apply)
6. [gRPC Surface](#grpc-surface)
7. [Common Implementation Patterns](#common-implementation-patterns)
8. [Code Examples](#code-examples)
9. [Integration Guidelines](#integration-guidelines)
10. [Best Practices](#best-practices)
11. [Common Errors / Gotchas](#common-errors--gotchas)
12. [Testing & Certification Notes](#testing--certification-notes)
13. [Cross-References](#cross-references)

## Architecture Overview

```
FraudAndRiskManagementService::{PreRiskCheck, PostRiskCheck}   EventService::NotifyConnector
CompositeFraudAndRiskManagementService::{PreRiskCheck, PostRiskCheck}  │
    │  proto/services.proto, proto/composite_services.proto            │  proto/services.proto
    ▼                                                                  ▼
grpc-server/src/server/frm.rs (internal_pre_risk_check, …)   grpc-server/src/server/events.rs
composite-service/src/frm.rs  (fetches the OAuth token first)    ├─ FrmPaymentOutcome
    │                                                            ├─ FrmRefundProcessed
    │                                                            └─ FrmChargebackReceived
    ▼                                                                  ▼
FrmConnectorData::from_connector_variant  →  variant.as_frm()
    │  connector-integration/src/types.rs
    ▼
BoxedFrmConnector = Box<&'static (dyn FrmServiceTrait + Sync)>
    ▼
RouterDataV2<FrmFlow, FrmFlowData, Frm*Request, Frm*Response>
    └─▶ ConnectorIntegrationV2<FrmFlow, FrmFlowData, …>
```

### Service Trait and its EXACT supertrait list

```rust
// From crates/types-traits/interfaces/src/connector_types.rs — `pub trait FrmServiceTrait`
pub trait FrmServiceTrait:
    ConnectorCommon
    + ValidationTrait
    + ServerAuthentication
    + PreRiskCheckV2
    + PostRiskCheckV2
    + FrmPaymentOutcomeV2
    + FrmRefundProcessedV2
    + FrmChargebackReceivedV2
    + PaymentPreAuthenticateV2<domain_types::payment_method_data::DefaultPCIHolder>
{
}
```

Nine supertraits. Two are unusual and both are load-bearing:

- **`ServerAuthentication`** — the FRM composite layer fetches an OAuth token before the risk check and threads it onto `FrmFlowData.access_token` (`crates/internal/composite-service/src/frm.rs`, `build_access_token_request` / `should_create_access_token`).
- **`PaymentPreAuthenticateV2<DefaultPCIHolder>`** — **not generic**. It is pinned to `DefaultPCIHolder` because FRM requests never carry payment-method data at a caller-chosen `T`. This is what forces the monomorphized impl described next.

`FrmServiceTrait` does **not** require `IncomingWebhook`, `VerifyRedirectResponse`, `SourceVerification` or `BodyDecoding` — but `kount.rs` implements all of those anyway, because it *also* implements `ConnectorServiceTrait<T>` for the payment registry.

### The monomorphization trap

```rust
// From crates/integrations/connector-integration/src/connectors/kount.rs
// Not generic over `T`: FrmServiceTrait also requires
// `PaymentPreAuthenticateV2<DefaultPCIHolder>` (fixed — Frm requests never carry
// payment-method data), which `Kount<T>` only provides for the matching `T`. This
// impl is restricted to `Kount<DefaultPCIHolder>` to match, which is also the only
// monomorphization `FrmConnectorData::convert_connector` ever constructs.
impl connector_types::FrmServiceTrait
    for Kount<domain_types::payment_method_data::DefaultPCIHolder>
{
}
```

Every *other* trait impl on `Kount` is `impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> … for Kount<T>`. Only `FrmServiceTrait` is written for the concrete `Kount<DefaultPCIHolder>`. Writing it generically is E0277: `Kount<T>` cannot satisfy `PaymentPreAuthenticateV2<DefaultPCIHolder>` for an arbitrary `T`.

The provider agrees:

```rust
// From crates/integrations/connector-integration/src/types.rs — FrmConnectorData::convert_connector
FrmConnectorEnum::Kount => Box::new(connectors::Kount::<
    domain_types::payment_method_data::DefaultPCIHolder,
>::new()),
```

### Flow markers and their exact `ConnectorIntegrationV2` bindings

All five use `FrmFlowData` as `ResourceCommonData`. From `crates/types-traits/interfaces/src/connector_types.rs`:

| Marker | Marker trait | Request | Response | Reached by |
| --- | --- | --- | --- | --- |
| `PreRiskCheck` | `PreRiskCheckV2` | `PreRiskCheckRequest` | `PreRiskCheckResponse` | `FraudAndRiskManagementService/PreRiskCheck` |
| `PostRiskCheck` | `PostRiskCheckV2` | `PostRiskCheckRequest` | `PostRiskCheckResponse` | `FraudAndRiskManagementService/PostRiskCheck` |
| `FrmPaymentOutcome` | `FrmPaymentOutcomeV2` | `FrmPaymentOutcomeRequest` | `FrmPaymentOutcomeResponse` | `EventService/NotifyConnector` + `FRM_PAYMENT_SUCCEEDED` **or** `FRM_PAYMENT_FAILURE` |
| `FrmRefundProcessed` | `FrmRefundProcessedV2` | `FrmRefundProcessedRequest` | `FrmRefundProcessedResponse` | `EventService/NotifyConnector` + `FRM_REFUND_PROCESSED` |
| `FrmChargebackReceived` | `FrmChargebackReceivedV2` | `FrmChargebackReceivedRequest` | `FrmChargebackReceivedResponse` | `EventService/NotifyConnector` + `FRM_CHARGEBACK_RECEIVED` |

Two additional bindings come with the supertrait list and do **not** use `FrmFlowData`:

| Marker | Marker trait | `ResourceCommonData` | Request | Response |
| --- | --- | --- | --- | --- |
| `ServerAuthenticationToken` | `ServerAuthentication` | `MerchantAuthenticationFlowData` | `ServerAuthenticationTokenRequestData` | `ServerAuthenticationTokenResponseData` |
| `PreAuthenticate` | `PaymentPreAuthenticateV2<DefaultPCIHolder>` | `PaymentFlowData` | `PaymentsPreAuthenticateData<DefaultPCIHolder>` | `PaymentsResponseData` |

`PreAuthenticate` here belongs to the **standalone 3DS trio** family (Mechanism 1 in `README.md`) — `PaymentFlowData`, `PaymentMethodAuthenticationService`. Kount uses it to hand back a Device-Data-Collection `<script>` for the shopper's browser; it makes **no** outbound call.

### FlowData type

```rust
// From crates/types-traits/domain_types/src/frm/frm_types.rs — `pub struct FrmFlowData`
pub struct FrmFlowData {
    pub merchant_id: common_utils::id_type::MerchantId,
    pub connectors: Arc<Connectors>,
    pub access_token: Option<ServerAuthenticationTokenResponseData>,
    pub raw_connector_response: Option<Secret<String>>,
    pub typed_connector_response: Option<String>,
    pub raw_connector_request: Option<Secret<String>>,
    pub typed_connector_request: Option<String>,
    pub connector_response_headers: Option<http::HeaderMap>,
}
```

Eight fields. `access_token` is the only non-plumbing field — read the bearer token from there, as `kount.rs`'s `frm_bearer_header` member function does.

### Request / response shapes (abridged — read the file for the full list)

```rust
// From crates/types-traits/domain_types/src/frm/frm_types.rs
pub struct PreRiskCheckResponse {
    pub frm_decision: Option<FrmDecision>,
    pub risk_score: Option<i32>,
    pub reason: Option<String>,
    pub frm_transaction_id: Option<String>,
    pub status_code: u16,
}
// PostRiskCheckResponse is field-identical to PreRiskCheckResponse.

pub struct FrmPaymentOutcomeResponse    { pub status_code: u16 }
pub struct FrmRefundProcessedResponse   { pub status_code: u16 }
pub struct FrmChargebackReceivedResponse { pub status_code: u16 }
```

`PreRiskCheckRequest` has 13 fields and `PostRiskCheckRequest` 12; `FrmPaymentOutcomeRequest` 8, `FrmRefundProcessedRequest` 8, `FrmChargebackReceivedRequest` 7. Copy the field list from `frm_types.rs` rather than from any doc. The `frm_decision` field's type is the **domain** enum `common_enums::FrmDecision` (`crates/common/common_enums/src/enums.rs`, `pub enum FrmDecision`) with exactly four variants — `Approve` (which is also `#[default]`), `Reject`, `Review`, `Error`. It is **not** the proto enum. The proto enum is declared in `crates/types-traits/grpc-api-types/proto/payment.proto` as `enum FrmDecision` with `FRM_DECISION_UNSPECIFIED`, `APPROVE`, `REJECT`, `REVIEW`, `ERROR`, and is reached from Rust as `grpc_api_types::frm::FrmDecision` (not `grpc_api_types::payments::…`); the two are bridged by the `ForeignFrom` impls in `crates/types-traits/domain_types/src/frm/types.rs`, where proto `Unspecified` folds onto domain `Review`. Write `common_enums::FrmDecision` in your transformer.

`frm_transaction_id` is the join key: `PreRiskCheckResponse.frm_transaction_id` is what the three notify requests later send back as `frm_transaction_id`.

## Connectors with Full Implementation

| Connector | Struct | Generic over `T`? | File | Real FRM flows | Stubbed FRM flows |
| --- | --- | --- | --- | --- | --- |
| kount | `Kount<T>` (generated by `create_all_prerequisites!`) | yes, except the `FrmServiceTrait` impl | `connectors/kount.rs` | `PreRiskCheck`, `FrmPaymentOutcome`, `FrmRefundProcessed` | `PostRiskCheck`, `FrmChargebackReceived` (via `frm_flow_not_implemented!`) |

Kount additionally implements `ServerAuthenticationToken` (real, OAuth client-credentials, form-url-encoded) and `PreAuthenticate` (real, local-only DDC script).

## Registration Sites

| # | Site | File | What to add |
| --- | --- | --- | --- |
| 1 | Module file | `crates/integrations/connector-integration/src/connectors.rs` | `pub mod foo;` + `pub use self::foo::Foo;` — the **payment** module file |
| 2 | Connector source | `crates/integrations/connector-integration/src/connectors/foo.rs` (+ `foo/transformers.rs`) | `ConnectorCommon`, `ValidationTrait`, `ServerAuthentication`, five FRM markers, `PaymentPreAuthenticateV2`, `FrmServiceTrait` (monomorphized) |
| 3 | FRM enum | `crates/types-traits/domain_types/src/connector_types.rs`, `pub enum FrmConnectorEnum` | a variant; snake_case strum value is the `x-frm-connector` header value |
| 4 | `ForeignTryFrom<AuthType>` | same file, `impl ForeignTryFrom<AuthType> for FrmConnectorEnum` | `AuthType::Foo(_) => Ok(Self::Foo)` |
| 5 | FRM provider | `crates/integrations/connector-integration/src/types.rs`, `FrmConnectorData::convert_connector` | `FrmConnectorEnum::Foo => Box::new(connectors::Foo::<DefaultPCIHolder>::new())` |
| 6 | URL patcher | `crates/types-traits/domain_types/src/types.rs`, `pub fn patch_frm_connector_urls` | `FrmConnectorEnum::Foo => patched.foo.apply(params_patch),` |
| 7 | `Connectors` struct | same file, `pub struct Connectors` | `pub foo: ConnectorParams,` |
| 8 | `ConnectorSpecificConfig` | `crates/types-traits/domain_types/src/router_data.rs`, `pub enum ConnectorSpecificConfig` | e.g. `Kount { api_key, auth_server_id, base_url }` |
| 9 | `AuthType` → config | same file | `AuthType::Foo(foo) => Ok(Self::Foo { .. })` |
| 10 | proto config message | `crates/types-traits/grpc-api-types/proto/payment.proto` | `message FooConfig { .. }` + oneof field — see `KountConfig kount = 134;` |
| 11 | Config TOMLs | `config/development.toml`, `config/sandbox.toml`, `config/production.toml` | `foo.base_url = "…"` (kount also sets `foo.secondary_base_url` for its OAuth host) |
| 12 | **Payment** enum | `crates/types-traits/domain_types/src/connector_types.rs`, `pub enum ConnectorEnum` | a variant — required because the file lives in `src/connectors/` and `check_connector_specs.rs` enforces parity |
| 13 | **Payment** provider | `crates/integrations/connector-integration/src/types.rs`, `ConnectorData::convert_connector` | `ConnectorEnum::Foo => Box::new(connectors::Foo::<T>::new())` |
| 14 | `ConnectorVariant` | `crates/types-traits/domain_types/src/connector_types.rs`, `impl ForeignTryFrom<AuthType> for ConnectorVariant` | Kount's arm is `AuthType::Kount(_) => Ok(Self::Payment(ConnectorEnum::Kount))` — **Payment**, not `Frm`. FRM routing comes from the `x-frm-connector` header |
| 15 | proto `enum Connector` | `crates/types-traits/grpc-api-types/proto/payment.proto` | `KOUNT = 128;` — required by the payment side |
| 16 | Certification specs | `crates/internal/integration-tests/src/connector_specs/foo/specs.json` | **required** — see below |
| 17 | Alpha list (if no CI creds) | `crates/internal/integration-tests/src/connector_specs/alpha_connectors.json` | `"foo": {}` or `"foo": { "reason": "…" }`, **inside the top-level `"connectors"` object** — that is the path `verify-new-connectors.sh` reads (`.connectors[$n].reason`) |

**Not required:**

- **A new connector directory.** FRM connectors live in `src/connectors/`. There is no `frm_connectors/`.
- **`config/superposition.toml`** — `kount` is absent from that file's `connector` schema enum.
- **FFI registration** — `crates/ffi/ffi/src/macros.rs` dispatches through `FrmConnectorData::get_connector_by_name(&connector)`.

### Why FRM is the one category with a real certification gate

Because the file sits in `crates/integrations/connector-integration/src/connectors/`, `check_connector_specs.rs` sees it (that binary reads `root.join("crates/integrations/connector-integration/src/connectors")` and nothing else). Phase 1 of that check fails if a connector `.rs` file has no `connector_specs/<name>/` directory. So an FRM connector **must** ship a `specs.json`, and `.github/scripts/verify-new-connectors.sh` will gate the PR.

What goes in it: the FRM flow names are all in `OUT_OF_SCOPE_FLOWS` (`"PreRiskCheck"`, `"PostRiskCheck"`, `"FrmPaymentOutcome"`, `"FrmRefundProcessed"`, `"FrmChargebackReceived"`), so they map to no suite. The flows that *do* map are the auth ones. Kount's file:

```json
// From crates/internal/integration-tests/src/connector_specs/kount/specs.json
{
  "connector": "kount",
  "supported_suites": [
    "MerchantAuthenticationService/CreateServerAuthenticationToken",
    "PaymentMethodAuthenticationService/PreAuthenticate"
  ]
}
```

**`OUT_OF_SCOPE_FLOWS` is not a prohibition.** The comment above its second half says: "Each is a coverage gap, not a decision that it should never be covered: add a suite, then move the flow into `flow_to_suites` above." Build the FRM flows; they simply have no certification suite yet.

## Which Macros Apply

| Macro | Applies? | Notes |
| --- | --- | --- |
| `macros::create_all_prerequisites!` | **Yes** | kount declares `ServerAuthenticationToken`, `PreRiskCheck`, `FrmPaymentOutcome`, `FrmRefundProcessed` in its `api: [ … ]`; generates `pub struct Kount<T>` and `Kount::new()` |
| `macros::macro_connector_implementation!` | **Yes** | one per real flow; `resource_common_data: FrmFlowData` for the three FRM flows, `MerchantAuthenticationFlowData` for the token flow |
| `macros::macro_connector_local_flow_implementation!` | **Yes** | for `PreAuthenticate` — no outbound call, response built by `kount::handle_pre_authenticate_response` |
| `macros::frm_flow_not_implemented!` | **Yes** | stubs an unimplemented FRM flow |
| `macros::macro_connector_flow_status_impls!` | **Yes** | required, because kount is also in the payment registry: every payment flow it does not implement must be listed |
| `macros::macro_connector_payout_implementation!` | **Yes** | kount calls it with **no** `payout_flows:` list, taking Arm 1's default all-nine set |
| `macros::create_amount_converter_wrapper!` | **Yes** | `amount_type: StringMinorUnit` |

### `frm_flow_not_implemented!` emits the stub but NOT the marker trait

```rust
// From crates/integrations/connector-integration/src/connectors/macros.rs
//   macro_rules! frm_flow_not_implemented — the whole expansion
impl<$g: $($b)*>
    ::interfaces::connector_integration_v2::ConnectorIntegrationV2<
        $flow,
        ::domain_types::frm::frm_types::FrmFlowData,
        $req,
        $resp,
    > for $c<$g>
{
    fn get_url(&self, …) -> … {
        Err(IntegrationError::connector_flow_not_implemented(…).into())
    }
}
```

There is no `impl PostRiskCheckV2 for …` in that expansion. Contrast `expand_payout_implementation!` and `expand_flow_status_impl!`, which *do* emit the marker impl. So `kount.rs` writes the two missing marker impls by hand, right after the two macro calls:

```rust
// From crates/integrations/connector-integration/src/connectors/kount.rs
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PostRiskCheckV2 for Kount<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::FrmChargebackReceivedV2 for Kount<T>
{
}
```

Forget these two and `impl FrmServiceTrait for Kount<DefaultPCIHolder>` fails with unsatisfied `PostRiskCheckV2` / `FrmChargebackReceivedV2` bounds. The macro's own doc-comment says as much: "`expand_flow_status_impl!` has no arms for FRM flows, so these stubs are hand-written."

## gRPC Surface

```proto
// From crates/types-traits/grpc-api-types/proto/services.proto — service FraudAndRiskManagementService
rpc PreRiskCheck(FrmServicePreRiskCheckRequest) returns (FrmServicePreRiskCheckResponse);
rpc PostRiskCheck(FrmServicePostRiskCheckRequest) returns (FrmServicePostRiskCheckResponse);
```

```proto
// From crates/types-traits/grpc-api-types/proto/composite_services.proto
//   service CompositeFraudAndRiskManagementService
rpc PreRiskCheck(CompositeFrmPreRiskCheckRequest) returns (CompositeFrmPreRiskCheckResponse);
rpc PostRiskCheck(CompositeFrmPostRiskCheckRequest) returns (CompositeFrmPostRiskCheckResponse);
```

Two rpcs (mirrored by the composite service, which additionally fetches the OAuth token first — `crates/internal/composite-service/src/frm.rs`). The other three markers arrive on `EventService/NotifyConnector`:

```proto
// From crates/types-traits/grpc-api-types/proto/payment.proto — enum NotifyEventType
FRM_PAYMENT_SUCCEEDED = 6;     // Payment authorized/captured — update FRM with outcome
FRM_PAYMENT_FAILURE = 7;       // Payment authorized/captured — update FRM with outcome
FRM_REFUND_PROCESSED = 8;      // Refund completed — notify FRM of return
FRM_CHARGEBACK_RECEIVED = 9;   // Chargeback opened — notify FRM of reversal
```

Note **two** event types collapse onto one marker: `FRM_PAYMENT_SUCCEEDED` and `FRM_PAYMENT_FAILURE` are a single match arm in `crates/grpc-server/grpc-server/src/server/events.rs` dispatching to `handle_frm_payment_outcome_notify` → `FrmPaymentOutcome`. The success/failure distinction reaches the connector on `FrmPaymentOutcomeRequest.payment_status` / `.frm_decision`, not on the marker.

`NotifyConnectorRequest.content` carries `FrmNotificationContent frm_notification = 2;` in its `oneof`.

Handlers: `crates/grpc-server/grpc-server/src/server/frm.rs` (`internal_pre_risk_check`, `internal_post_risk_check`), both with `connector_data_types: [FrmConnectorData]`.

Routing: the request must carry `x-frm-connector`. `impl ForeignTryFrom<AuthType> for ConnectorVariant` maps `AuthType::Kount(_)` to `ConnectorVariant::Payment(ConnectorEnum::Kount)`, so **without the header the request routes to the payment registry, not the FRM one** — see `connector_variant_from_config_and_metadata` in `crates/types-traits/ucs_interface_common/src/auth.rs`, whose `X_FRM_CONNECTOR_NAME` branch is what selects `FrmConnectorEnum::foreign_try_from(config)`.

## Common Implementation Patterns

1. `macros::create_amount_converter_wrapper!(connector_name: Foo, amount_type: …);`
2. `macros::create_all_prerequisites!(connector_name: Foo, generic_type: T, api: [ ServerAuthenticationToken, PreRiskCheck, … ], amount_converters: [ amount_converter: StringMinorUnit ], member_functions: { fn frm_bearer_header(&self, token: Option<&Secret<String>>) -> … });`
3. `impl<T: …> ConnectorCommon for Foo<T>`.
4. Base traits — because the file is in the payment registry: `ConnectorServiceTrait<T>`, `ValidationTrait` (override `should_do_access_token` → `true` if the connector uses OAuth), `IncomingWebhook`, `VerifyRedirectResponse`, `SourceVerification`, `BodyDecoding`.
5. `macro_connector_flow_status_impls!` listing every payment flow not implemented.
6. `macro_connector_payout_implementation!` with no `payout_flows:` list (all nine stubbed).
7. One `macro_connector_implementation!` per real FRM flow, `resource_common_data: FrmFlowData`, headers from the member function that reads `FrmFlowData.access_token`.
8. `frm_flow_not_implemented!` for each unimplemented FRM flow, **plus** its bare marker impl.
9. `impl connector_types::FrmServiceTrait for Foo<DefaultPCIHolder> {}` — monomorphized, last.

### If the FRM connector also does device-data collection

Kount's `PreAuthenticate` returns an HTML `<script>` snippet built locally, with no outbound call. It uses `macro_connector_local_flow_implementation!` and overrides `next_authentication_step`:

```rust
// From crates/integrations/connector-integration/src/connectors/kount.rs — impl ValidationTrait
fn next_authentication_step(
    &self,
    _auth_type: common_enums::AuthenticationType,
    _payment_method: common_enums::PaymentMethod,
    _redirect_state: connector_types::RedirectState,
    _completed_step: Option<connector_types::AuthenticationStep>,
) -> connector_types::AuthenticationStep {
    // Kount only runs PreAuthenticate (DDC); the composite loop breaks once
    // the DDC `redirection_data` is present. FRM risk checks run separately
    // via the FraudAndRiskManagementService composite flow.
    connector_types::AuthenticationStep::PreAuthenticate
}
```

Without that override, `ValidationTrait`'s default returns `AuthenticationStep::Authorize` and the `PreAuthenticate` leg is never reached. See `pattern_authentication_dispatch.md`.

## Code Examples

### The two stub macro calls and their hand-written marker impls

```rust
// From crates/integrations/connector-integration/src/connectors/kount.rs
macros::frm_flow_not_implemented!(
    connector: Kount,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    flow: PostRiskCheck,
    request: PostRiskCheckRequest,
    response: PostRiskCheckResponse,
    flow_name: "post_risk_check",
);
macros::frm_flow_not_implemented!(
    connector: Kount,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    flow: FrmChargebackReceived,
    request: FrmChargebackReceivedRequest,
    response: FrmChargebackReceivedResponse,
    flow_name: "frm_chargeback_received",
);
```

### A real FRM flow

```rust
// From crates/integrations/connector-integration/src/connectors/kount.rs
macros::macro_connector_implementation!(
    …
    flow_name: PreRiskCheck,
    resource_common_data: FrmFlowData,
    …
);
```

### Reading the OAuth token off `FrmFlowData`

```rust
// From crates/integrations/connector-integration/src/connectors/kount.rs
//   create_all_prerequisites! member_functions
fn frm_bearer_header(
    &self,
    token: Option<&hyperswitch_masking::Secret<String>>,
) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
    let token = token.ok_or(IntegrationError::FailedToObtainAuthType { … })?;
    Ok(vec![
        (headers::CONTENT_TYPE.to_string(), self.common_get_content_type().to_string().into()),
        (headers::AUTHORIZATION.to_string(), format!("Bearer {}", token.peek()).into_masked()),
    ])
}
```

## Integration Guidelines

1. Read `connectors/kount.rs` end to end. It is the only FRM exemplar and it exercises every macro in the category.
2. Decide whether the connector is FRM-only or also a payment connector. Either way the file goes in `src/connectors/` and needs a `ConnectorEnum` variant plus a `connector_specs/<name>/specs.json` — the certification checker enforces directory/spec parity.
3. Write `connectors/foo/transformers.rs` first: `FooAuthType` with `impl TryFrom<&ConnectorSpecificConfig>`, the OAuth token request/response, the risk-check request/response, the notify request/response, and the `FrmDecision` mapping.
4. Write `connectors/foo.rs` in the order given under "Common Implementation Patterns".
5. Put `impl FrmServiceTrait for Foo<DefaultPCIHolder> {}` last, so the compiler enumerates exactly which of the nine supertraits are still missing.
6. Walk the 17 registration sites.
7. `cargo check -p connector-integration`, then `cargo run --bin check_connector_specs`, then `cargo check --workspace`.
8. Send `x-frm-connector: foo` on every FRM request, or the router will resolve the payment variant instead.

## Best Practices

- **Map `FrmDecision` exhaustively.** No `_ =>` at the mapping layer; give the connector's decision enum a `#[serde(other)] Unknown` variant at the deserialization layer and an explicit arm that maps to `FrmDecision::Error` or `Review`, never silently to `Approve` (`PATTERN_AUTHORING_SPEC.md` §11.9).
- **Return `frm_transaction_id` from `PreRiskCheck`.** All three notify flows key off it; Kount's `kount_order_id` helper falls back to `connector_transaction_id` and errors with a `suggested_action` when both are absent.
- **Never fail the payment on an FRM transport error.** `FrmDecision::Error` (proto `ERROR`) exists precisely for "the FRM check itself failed (network, timeout, misconfiguration)" (proto comment on `enum FrmDecision` in `payment.proto`). Surface it; do not coerce to `Reject`.
- **Notify responses carry only `status_code`** — record the parsed body on the event builder and on `resource_common_data` before discarding it, or a notify failure will be invisible.
- **Keep `should_do_access_token` truthful.** `crates/internal/composite-service/src/frm.rs` calls it to decide whether to run the token leg; returning `false` on an OAuth connector leaves `FrmFlowData.access_token` as `None` and every risk check fails with `FailedToObtainAuthType`.
- **Do not escape user data into a `<script>` by hand.** If you emit a DDC snippet, copy Kount's `js_string_escape` (`connectors/kount.rs`), which encodes `\`, `"`, newlines, `<`, `>` and `&`.

## Common Errors / Gotchas

1. **Writing `impl<T> FrmServiceTrait for Foo<T>`.**
   - *Problem*: E0277 — `FrmServiceTrait` requires `PaymentPreAuthenticateV2<DefaultPCIHolder>`, which `Foo<T>` only satisfies when `T = DefaultPCIHolder`.
   - *Solution*: `impl connector_types::FrmServiceTrait for Foo<domain_types::payment_method_data::DefaultPCIHolder> {}`. That is also the only monomorphization `FrmConnectorData::convert_connector` builds.

2. **Assuming `frm_flow_not_implemented!` emits the marker trait.**
   - *Problem*: after two `frm_flow_not_implemented!` calls, `impl FrmServiceTrait` still fails on `PostRiskCheckV2` / `FrmChargebackReceivedV2`.
   - *Solution*: the macro emits only the `ConnectorIntegrationV2` stub. Add the bare marker impls by hand, as `kount.rs` does.

3. **Looking for an `frm_connectors/` directory.**
   - *Problem*: by analogy with `payout_connectors/`, `surcharge_connectors/`, `authenticator_connectors/`, you expect a fourth sibling.
   - *Solution*: there is none. FRM connectors live in `src/connectors/` and are distinguished by `FrmServiceTrait` + `FrmConnectorEnum` + `FrmConnectorData`.

4. **Expecting `ConnectorVariant` to resolve FRM from the auth config.**
   - *Problem*: `AuthType::Kount(_)` maps to `ConnectorVariant::Payment(ConnectorEnum::Kount)`, so an FRM request without the header lands on `ConnectorData`, and `FrmConnectorData::from_connector_variant` (which calls `variant.as_frm()`) returns `None`.
   - *Solution*: send `x-frm-connector`. The header branch in `connector_variant_from_config_and_metadata` is what produces `ConnectorVariant::Frm(..)`.

5. **Expecting five rpcs.**
   - *Problem*: you look for `FraudAndRiskManagementService/PaymentOutcome` and find nothing.
   - *Solution*: two rpcs only. `FrmPaymentOutcome` / `FrmRefundProcessed` / `FrmChargebackReceived` arrive via `EventService/NotifyConnector`, and `FRM_PAYMENT_SUCCEEDED` + `FRM_PAYMENT_FAILURE` both map to `FrmPaymentOutcome`.

6. **Declaring a flow that `check_connector_specs.rs` cannot see, or vice versa.**
   - *Problem*: `extract_declared_flows` reads only (a) `flow:` lines inside `macros::create_all_prerequisites!(…)` and (b) the first type argument of a literal `ConnectorIntegrationV2<Ident,` in the source text. `kount.rs` contains **zero** occurrences of the second form (`grep -c "ConnectorIntegrationV2<" crates/integrations/connector-integration/src/connectors/kount.rs` → `0`), so its `PreAuthenticate` — declared through `macro_connector_local_flow_implementation!` — is invisible to the checker even though `specs.json` lists the suite.
   - *Solution*: do not rely on the checker to find macro-generated flows. Declare every suite you actually support in `specs.json` yourself, and read the checker output rather than assuming silence means coverage.

7. **Treating `OUT_OF_SCOPE_FLOWS` as "do not implement".**
   - *Problem*: all five FRM markers are in that list, which reads like a scope exclusion.
   - *Solution*: the source comment is explicit — "Each is a coverage gap, not a decision that it should never be covered." Implement them; the certification suites simply do not exist yet.

## Testing & Certification Notes

- **Unit tests** in `connectors/foo/transformers.rs`: `FooAuthType::try_from` on the wrong `ConnectorSpecificConfig` variant; every branch of the decision mapper including `Unknown`; `frm_transaction_id` fallback behaviour; any client-side script escaping. `connectors/kount.rs` ships no test module today (its first line is `pub mod transformers;` and `connectors/kount/` contains only `transformers.rs`); `authenticator_connectors/plaid.rs` shows the layout to copy — a `#[cfg(test)] mod test;` declared on line 1 with the tests in a sibling `test.rs`.
- **Certification is merge-blocking.** `.github/scripts/verify-new-connectors.sh` fires for any connector whose `connector_specs/<name>/` directory did not exist at the merge base, and hard-fails on: `CONNECTOR_AUTH_FILE_PATH` unset; no `specs.json`; empty `supported_suites`; an `alpha_connectors.json` entry with no `reason`; an off-alpha connector with no CI credentials; any declared scenario failing. An FRM connector *does* create such a directory, so plan for it.
- **If there are no CI sandbox credentials**, add the connector under the `"connectors"` object in `crates/internal/integration-tests/src/connector_specs/alpha_connectors.json` — `"kount": {}` is the existing entry — and remove it as soon as credentials land, per that file's `_comment`.
- **Manual scenarios**: OAuth token fetch; pre-risk check returning each of APPROVE / REJECT / REVIEW; pre-risk check with the FRM service down (expect `ERROR`, not `REJECT`); notify payment-succeeded and payment-failure with the `frm_transaction_id` from the pre-check; notify refund-processed; notify with an unknown `frm_transaction_id`.

## Cross-References

- Parent index: [./README.md](./README.md)
- Authoring spec: [./PATTERN_AUTHORING_SPEC.md](./PATTERN_AUTHORING_SPEC.md)
- Sibling category: [pattern_payout_connector.md](./pattern_payout_connector.md)
- Sibling category: [pattern_surcharge_connector.md](./pattern_surcharge_connector.md)
- Sibling category: [pattern_authenticator_connector.md](./pattern_authenticator_connector.md)
- The `PreAuthenticate` leg: [pattern_preauthenticate.md](./pattern_preauthenticate.md)
- Making that leg reachable: [pattern_authentication_dispatch.md](./pattern_authentication_dispatch.md)
- OAuth token leg: [pattern_server_authentication_token.md](./pattern_server_authentication_token.md)
- Macros: [macro_patterns_reference.md](./macro_patterns_reference.md)
- Utility helpers: [../utility_functions_reference.md](../utility_functions_reference.md)
- Types: [../types/types.md](../types/types.md)
