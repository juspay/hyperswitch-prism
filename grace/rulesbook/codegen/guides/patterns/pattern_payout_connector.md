# Payout Connector Pattern

## Overview

A **payout connector** pushes funds *out* to a beneficiary. It is not a payment connector with extra flows: it is a separate connector category living in its own directory (`crates/integrations/connector-integration/src/payout_connectors/`, a **sibling** of `connectors/`, not a subdirectory), behind its own service trait (`PayoutServiceTrait`), its own enum (`PayoutConnectorEnum`), its own `ConnectorData` provider (`PayoutConnectorData`), and its own gRPC service (`PayoutService`).

Ten payout connectors exist at HEAD. Seven of them are **non-generic unit structs** that hand-write every `ConnectorIntegrationV2` block; only three are generic-over-`T` and can use `create_all_prerequisites!` / `macro_connector_implementation!` / `macro_connector_payout_implementation!`. Pick the style before you write a line — the two styles share almost no code.

The single most consequential difference from a payment connector: **`PayoutServiceTrait` does not require `ValidationTrait`, `IncomingWebhook`, `VerifyRedirectResponse`, `SourceVerification`, or `BodyDecoding`.** Writing those impls on a payout connector is dead code.

### Key Components

- **Service trait**: `interfaces::connector_types::PayoutServiceTrait` — `crates/types-traits/interfaces/src/connector_types.rs`, `pub trait PayoutServiceTrait`.
- **Boxed form**: `interfaces::connector_types::BoxedPayoutConnector` = `Box<&'static (dyn PayoutServiceTrait + Sync)>` — same file, `pub type BoxedPayoutConnector`. Non-generic `dyn`, so a generic connector must be monomorphized at construction.
- **Flow data**: `domain_types::payouts::payouts_types::PayoutFlowData` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`, `pub struct PayoutFlowData`.
- **Flow markers**: `domain_types::connector_flow::{PayoutCreate, PayoutTransfer, PayoutGet, PayoutVoid, PayoutStage, PayoutCreateLink, PayoutCreateRecipient, PayoutEnrollDisburseAccount, PayoutEligibility}` — `crates/types-traits/domain_types/src/connector_flow.rs`.
- **Category enum**: `domain_types::connector_types::PayoutConnectorEnum` — `crates/types-traits/domain_types/src/connector_types.rs`, `pub enum PayoutConnectorEnum`.
- **Provider**: `connector_integration::types::PayoutConnectorData` — `crates/integrations/connector-integration/src/types.rs`, `pub struct PayoutConnectorData`.
- **Module file**: `crates/integrations/connector-integration/src/payout_connectors.rs`.
- **Reference connectors**: `payout_connectors/trustly.rs` (generic + macros), `payout_connectors/gotyme_sanlam.rs` (generic + macros, minimal), `payout_connectors/itaubank.rs` (non-generic, hand-written), `payout_connectors/truelayer.rs` (non-generic, local stub macro).
- **gRPC service**: `PayoutService` — `crates/types-traits/grpc-api-types/proto/services.proto`, `service PayoutService`.
- **Routing header**: `x-payout-connector` — `common_utils::consts::X_PAYOUT_CONNECTOR_NAME` (`crates/common/common_utils/src/consts.rs`).

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
PayoutService::{Create,Transfer,Get,Void,Stage,CreateLink,
                CreateRecipient,EnrollDisburseAccount,Eligibility}   (gRPC)
    │   crates/types-traits/grpc-api-types/proto/services.proto  (service PayoutService)
    ▼
grpc-server handlers  (internal_payout_* via implement_connector_operation!)
    │   crates/grpc-server/grpc-server/src/server/payouts.rs
    ▼
PayoutConnectorData::from_connector_variant  →  variant.as_payout()
    │   crates/integrations/connector-integration/src/types.rs
    ▼
BoxedPayoutConnector = Box<&'static (dyn PayoutServiceTrait + Sync)>
    │
    ▼
RouterDataV2<PayoutFlow, PayoutFlowData, Payout*Request, Payout*Response>
    └─▶ ConnectorIntegrationV2<PayoutFlow, PayoutFlowData, …>
```

### Service Trait and its EXACT supertrait list

```rust
// From crates/types-traits/interfaces/src/connector_types.rs — `pub trait PayoutServiceTrait`
pub trait PayoutServiceTrait:
    ConnectorCommon
    + ServerAuthentication
    + PayoutCreateV2
    + PayoutTransferV2
    + PayoutGetV2
    + PayoutVoidV2
    + PayoutStageV2
    + PayoutCreateLinkV2
    + PayoutCreateRecipientV2
    + PayoutEnrollDisburseAccountV2
    + PayoutEligibilityV2
{
}
```

Eleven supertraits: `ConnectorCommon`, `ServerAuthentication`, and the nine payout markers. Compare with `ConnectorServiceTrait` in the same file, which additionally requires `ValidationTrait`, `IncomingWebhook`, `VerifyRedirectResponse` and the whole payment surface. **A payout connector needs none of those.** Verified: `grep -rln "SourceVerification" crates/integrations/connector-integration/src/payout_connectors/` returns nothing, and no payout connector implements `ValidationTrait`.

`ServerAuthentication` is required because payout processors overwhelmingly use OAuth: the token is fetched on the `MerchantAuthenticationService` leg and threaded onto `PayoutFlowData.access_token`. A payout connector that authenticates in-band (Trustly signs the JSON-RPC body) still has to satisfy the bound — with a `not_implemented` stub.

### Flow markers and their exact `ConnectorIntegrationV2` bindings

Every payout binding uses `PayoutFlowData` as `ResourceCommonData`. From `crates/types-traits/interfaces/src/connector_types.rs`:

| Marker | Marker trait | Request | Response |
| --- | --- | --- | --- |
| `PayoutCreate` | `PayoutCreateV2` | `PayoutCreateRequest` | `PayoutCreateResponse` |
| `PayoutTransfer` | `PayoutTransferV2` | `PayoutTransferRequest` | `PayoutTransferResponse` |
| `PayoutGet` | `PayoutGetV2` | `PayoutGetRequest` | `PayoutGetResponse` |
| `PayoutVoid` | `PayoutVoidV2` | `PayoutVoidRequest` | `PayoutVoidResponse` |
| `PayoutStage` | `PayoutStageV2` | `PayoutStageRequest` | `PayoutStageResponse` |
| `PayoutCreateLink` | `PayoutCreateLinkV2` | `PayoutCreateLinkRequest` | `PayoutCreateLinkResponse` |
| `PayoutCreateRecipient` | `PayoutCreateRecipientV2` | `PayoutCreateRecipientRequest` | `PayoutCreateRecipientResponse` |
| `PayoutEnrollDisburseAccount` | `PayoutEnrollDisburseAccountV2` | `PayoutEnrollDisburseAccountRequest` | `PayoutEnrollDisburseAccountResponse` |
| `PayoutEligibility` | `PayoutEligibilityV2` | `PayoutEligibilityRequest` | `PayoutEligibilityResponse` |

All request/response structs live in `crates/types-traits/domain_types/src/payouts/payouts_types.rs`. Each marker trait is declared exactly as:

```rust
// From crates/types-traits/interfaces/src/connector_types.rs — `pub trait PayoutTransferV2`
pub trait PayoutTransferV2:
    ConnectorIntegrationV2<
    connector_flow::PayoutTransfer,
    PayoutFlowData,
    PayoutTransferRequest,
    PayoutTransferResponse,
>
{
}
```

The tenth marker `ServerAuthenticationToken` uses **`MerchantAuthenticationFlowData`**, not `PayoutFlowData`:

```rust
// From crates/types-traits/interfaces/src/connector_types.rs — `pub trait ServerAuthentication`
pub trait ServerAuthentication:
    ConnectorIntegrationV2<
    connector_flow::ServerAuthenticationToken,
    MerchantAuthenticationFlowData,
    ServerAuthenticationTokenRequestData,
    ServerAuthenticationTokenResponseData,
>
{
}
```

`MerchantAuthenticationFlowData` is at `crates/types-traits/domain_types/src/merchant_authentication_flow_data.rs` and deliberately carries no payment or payout fields.

### FlowData type

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs — `pub struct PayoutFlowData`
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

Thirteen fields. `access_token` is where the `ServerAuthenticationToken` leg's result lands; read it there, never from a payment-side accessor. `PayoutFlowData` implements `RawConnectorRequestResponse` in the same file.

### Connector directory

```
crates/integrations/connector-integration/src/
├── connectors/                # payment connectors           (NOT here)
├── payout_connectors.rs       # module file — one `pub mod` + one `pub use` per connector
└── payout_connectors/
    ├── <name>.rs              # connector: ConnectorCommon + PayoutServiceTrait + flow impls
    └── <name>/
        └── transformers.rs    # request/response structs, `<Name>AuthType`, status mapping
```

## Connectors with Full Implementation

Derived live:

```bash
grep -o "flow_name: Payout[A-Za-z]*" crates/integrations/connector-integration/src/payout_connectors/*.rs
grep -o "ConnectorIntegrationV2<\s*Payout[A-Za-z]*" crates/integrations/connector-integration/src/payout_connectors/*.rs
```

| Connector | Struct | Generic over `T`? | Style | Real flows | Stubbed flows |
| --- | --- | --- | --- | --- | --- |
| cybersource | `CybersourcePayouts` | no | hand-written `impl ConnectorIntegrationV2` blocks | `PayoutTransfer` | other 8 |
| deutschebank | `DeutschebankPayouts<T>` | yes | `create_all_prerequisites!` + `macro_connector_implementation!` | `PayoutTransfer`, `PayoutGet`, `PayoutEligibility` | other 6 via `macro_connector_payout_implementation!` |
| gotyme_sanlam | `GotymeSanlamPayouts<T>` | yes | `create_all_prerequisites!` + `macro_connector_implementation!` | `PayoutTransfer`, `PayoutGet` | other 7 via `macro_connector_payout_implementation!` |
| itaubank | `ItaubankPayouts` | no | hand-written | `PayoutTransfer`, `PayoutGet` | other 7 |
| loonio | `LoonioPayouts` | no | hand-written | `PayoutTransfer`, `PayoutGet` | other 7 |
| paypal | `PaypalPayouts` | no | hand-written | `PayoutTransfer`, `PayoutGet` | other 7 |
| santander | `SantanderPayouts` | no | hand-written | `PayoutCreate`, `PayoutTransfer`, `PayoutGet` | other 6 |
| truelayer | `TruelayerPayouts` | no | hand-written + file-local `impl_unimplemented_payout_flow!` | `PayoutTransfer`, `PayoutGet` | other 7 via the local macro |
| trustly | `TrustlyPayouts<T>` | yes | `create_all_prerequisites!` + `macro_connector_implementation!` | `PayoutCreateRecipient`, `PayoutTransfer`, `PayoutGet` | 5 via `macro_connector_payout_implementation!`, `PayoutEligibility` hand-written |
| worldpayxml | `WorldpayxmlPayouts` | no | hand-written (SOAP/XML) | `PayoutTransfer`, `PayoutGet`, `PayoutVoid` | other 6 |

Seven of ten are **non-generic**. `create_all_prerequisites!` and `macro_connector_implementation!` both take a mandatory `generic_type: $generic_type:tt` parameter (`crates/integrations/connector-integration/src/connectors/macros.rs`, `macro_rules! create_all_prerequisites` and `macro_rules! macro_connector_implementation`), so a `pub struct FooPayouts;` unit struct **cannot** use them. Choose generic-over-`T` if you want the macro path.

## Registration Sites

A new payout connector `foo` touches exactly these places. Everything else is optional.

| # | Site | File | What to add |
| --- | --- | --- | --- |
| 1 | Module file | `crates/integrations/connector-integration/src/payout_connectors.rs` | `pub mod foo;` + `pub use self::foo::FooPayouts;` |
| 2 | Connector source | `crates/integrations/connector-integration/src/payout_connectors/foo.rs` (+ `foo/transformers.rs`) | `ConnectorCommon`, `PayoutServiceTrait`, all 9 payout markers, `ServerAuthentication` |
| 3 | Category enum | `crates/types-traits/domain_types/src/connector_types.rs`, `pub enum PayoutConnectorEnum` | a variant (`#[strum(serialize_all = "snake_case")]` gives the `x-payout-connector` header value) |
| 4 | `ForeignTryFrom<AuthType>` | same file, `impl ForeignTryFrom<AuthType> for PayoutConnectorEnum` | `AuthType::Foo(_) => Ok(Self::Foo)` |
| 5 | `ConnectorVariant` | same file, `impl ForeignTryFrom<AuthType> for ConnectorVariant` | `AuthType::Foo(_) => Ok(Self::Payout(PayoutConnectorEnum::Foo))` — see the existing `AuthType::Deutschebank` / `AuthType::Santander` arms |
| 6 | Provider | `crates/integrations/connector-integration/src/types.rs`, `PayoutConnectorData::convert_connector` | `PayoutConnectorEnum::Foo => Box::new(payout_connectors::FooPayouts::new())`. If the struct is generic, monomorphize: `FooPayouts::<domain_types::payment_method_data::DefaultPCIHolder>::new()` |
| 7 | `ConnectorSpecificConfig` | `crates/types-traits/domain_types/src/router_data.rs`, `pub enum ConnectorSpecificConfig` | a variant with the credential fields — **skip if the connector already exists as a payment connector** (`payout_connectors/trustly.rs` reuses `ConnectorSpecificConfig::Trustly`) |
| 8 | `AuthType` → config | same file, the `AuthType::Foo(foo) => Ok(Self::Foo { .. })` arm | field-by-field, `.ok_or_else(err)?` for required fields |
| 9 | proto config message | `crates/types-traits/grpc-api-types/proto/payment.proto` | `message FooConfig { .. }` + a field in `message ConnectorSpecificConfig`'s `oneof config` — see `GotymeSanlamConfig gotyme_sanlam = 150;` |
| 10 | `Connectors` struct | `crates/types-traits/domain_types/src/types.rs`, `pub struct Connectors` | `pub foo: ConnectorParams,` (or `ConnectorParamsWithCaBundle`, as `deutschebank` does) |
| 11 | URL patcher | same file, `pub fn patch_payout_connector_urls` | `PayoutConnectorEnum::Foo => patched.foo.apply(params_patch),` |
| 12 | Config TOMLs | `config/development.toml`, `config/sandbox.toml`, `config/production.toml` | `foo.base_url = "…"` in each |

**Not required:**

- **`ConnectorEnum`** (the payment enum) — only if the connector is *also* a payment connector. `PayoutConnectorEnum` is independent. Note the separate `impl TryFrom<ConnectorEnum> for PayoutConnectorEnum` in `connector_types.rs`: it covers only Loonio, Paypal, Itaubank, Worldpayxml, Cybersource, Truelayer, Trustly, and exists so that `PayoutConnectorData::from_connector_variant` can fall back from `as_payment()`. Deutschebank, Santander and GotymeSanlam are deliberately absent from it — they are payout-only.
- **proto `enum Connector`** — `gotyme_sanlam` has no entry there (`awk '/^enum Connector \{/,/^\}/' crates/types-traits/grpc-api-types/proto/payment.proto | grep -i gotyme` → empty) and works.
- **`config/superposition.toml`** — the `connector` schema enum there lists 90 names and includes neither `gotyme_sanlam` nor `deutschebank` nor `santander` nor `itaubank`. An entry only adds a runtime base-URL override.
- **`connector_specs/<name>/specs.json`** — `check_connector_specs.rs` scans only `crates/integrations/connector-integration/src/connectors`, and `const IGNORE_SERVICES: &[&str] = &["PayoutService", "DisputeService"]` drops every `PayoutService/*` suite. All eight non-eligibility payout flow names are additionally listed in `OUT_OF_SCOPE_FLOWS`.
- **FFI registration** — `crates/ffi/ffi/src/macros.rs` dispatches generically through `PayoutConnectorData::get_connector_by_name(&connector)`; no per-connector arm.
- **`IncomingWebhook`, `VerifyRedirectResponse`, `ValidationTrait`, `SourceVerification`, `BodyDecoding`** — not in the supertrait list; writing them is dead code.

## Which Macros Apply

| Macro | Applies? | Notes |
| --- | --- | --- |
| `macros::create_all_prerequisites!` | Only for generic-over-`T` connectors | mandatory `generic_type:` parameter (`connectors/macros.rs`, `macro_rules! create_all_prerequisites`) |
| `macros::macro_connector_implementation!` | Only for generic-over-`T` connectors | same constraint |
| `macros::macro_connector_payout_implementation!` | Only for generic-over-`T` connectors | emits marker-trait impl **and** a `connector_flow_not_implemented` `ConnectorIntegrationV2` stub for each listed flow (`macro_rules! expand_payout_implementation`) |
| `macros::macro_connector_flow_status_impls!` | Optional | only `gotyme_sanlam.rs` uses it in this directory, for `not_implemented: [ServerAuthenticationToken]`; the arm also emits `impl ServerAuthentication` (`macro_rules! expand_flow_status_impl`, arm `flow: ServerAuthenticationToken`) |
| `macros::create_amount_converter_wrapper!` | Yes if you need unit conversion | |
| `macros::macro_connector_local_flow_implementation!` | Rarely | no outbound call |

### `macro_connector_payout_implementation!` — the default arm covers all nine

```rust
// From crates/integrations/connector-integration/src/connectors/macros.rs
//   macro_rules! macro_connector_payout_implementation — Arm 1 (no explicit list)
payout_flows: [
    PayoutCreate,
    PayoutTransfer,
    PayoutGet,
    PayoutVoid,
    PayoutStage,
    PayoutCreateLink,
    PayoutCreateRecipient,
    PayoutEnrollDisburseAccount,
    PayoutEligibility
]
```

`expand_payout_implementation!` has an arm for each of those nine, **including `PayoutEligibility`** — that arm was added in `3df5eb702` ("feat(connector): [GoTyme] Add gotyme_sanlam payout connector", #1983). Verify with:

```bash
sed -n '/macro_rules! expand_payout_implementation/,/^pub(crate) use expand_payout_implementation/p' \
  crates/integrations/connector-integration/src/connectors/macros.rs | grep "flow: Payout"
```

`payout_connectors/trustly.rs` still hand-writes its `PayoutEligibility` stub and carries a comment saying the arm does not exist. That comment predates `3df5eb702`; **do not copy it**. `gotyme_sanlam.rs` lists `PayoutEligibility` in its `payout_flows:` array and is the current-correct model.

## gRPC Surface

```proto
// From crates/types-traits/grpc-api-types/proto/services.proto — service PayoutService
rpc Create(PayoutServiceCreateRequest) returns (PayoutServiceCreateResponse);
rpc Transfer(PayoutServiceTransferRequest) returns (PayoutServiceTransferResponse);
rpc Get(PayoutServiceGetRequest) returns (PayoutServiceGetResponse);
rpc Void(PayoutServiceVoidRequest) returns (PayoutServiceVoidResponse);
rpc Stage(PayoutServiceStageRequest) returns (PayoutServiceStageResponse);
rpc CreateLink(PayoutServiceCreateLinkRequest) returns (PayoutServiceCreateLinkResponse);
rpc CreateRecipient(PayoutServiceCreateRecipientRequest) returns (PayoutServiceCreateRecipientResponse);
rpc EnrollDisburseAccount(PayoutServiceEnrollDisburseAccountRequest) returns (PayoutServiceEnrollDisburseAccountResponse);
rpc Eligibility(PayoutMethodEligibilityRequest) returns (PayoutMethodEligibilityResponse);
```

Nine rpcs, one per marker — **every payout flow has a dedicated rpc.** Unlike surcharge and FRM, no payout flow arrives through `EventService/NotifyConnector`: `enum NotifyEventType` (`crates/types-traits/grpc-api-types/proto/payment.proto`) has only surcharge and FRM members.

Handlers: `crates/grpc-server/grpc-server/src/server/payouts.rs`, nine `implement_connector_operation!` invocations (`internal_payout_create` … `internal_payout_eligibility`), each with `connector_data_types: [PayoutConnectorData]`.

Routing: the request must carry `x-payout-connector` (or an `AuthType` that maps to a `ConnectorVariant::Payout`). See `connector_variant_from_config_and_metadata` in `crates/types-traits/ucs_interface_common/src/auth.rs` and the header branch in `crates/types-traits/ucs_interface_common/src/metadata.rs`.

## Common Implementation Patterns

### Pattern A — generic + macros (`trustly.rs`, `gotyme_sanlam.rs`, `deutschebank.rs`)

1. `macros::create_all_prerequisites!(connector_name: FooPayouts, generic_type: T, api: [ … ], amount_converters: [ … ], member_functions: { … });` — this **declares** `pub struct FooPayouts<T>` and its `pub const fn new() -> &'static Self`; do not write a struct definition of your own.
2. `impl<T: …> ConnectorCommon for FooPayouts<T>`.
3. `impl<T: …> PayoutServiceTrait for FooPayouts<T> {}` plus a bare `impl` for each *real* marker trait (`PayoutTransferV2`, `PayoutGetV2`, …).
4. One `macros::macro_connector_implementation!` per real flow, with `resource_common_data: PayoutFlowData`.
5. One `macros::macro_connector_payout_implementation!` listing every flow you did **not** implement — that macro emits both the marker impl and the stub.
6. `ServerAuthentication`: either a real `macro_connector_implementation!` with `resource_common_data: MerchantAuthenticationFlowData`, or a stub. `gotyme_sanlam.rs` stubs it with `macro_connector_flow_status_impls!(… not_implemented: [ServerAuthenticationToken])`; `trustly.rs` hand-writes both the marker impl and the `get_url` stub.

### Pattern B — non-generic + hand-written (`itaubank.rs`, `paypal.rs`, `santander.rs`, `worldpayxml.rs`, `cybersource.rs`, `loonio.rs`)

1. `pub struct FooPayouts;` with `pub const fn new() -> &'static Self { &Self }`, written by hand (`payout_connectors/itaubank.rs`, `pub struct ItaubankPayouts;`).
2. `impl ConnectorCommon for FooPayouts`.
3. `impl PayoutServiceTrait for FooPayouts {}` + `impl ServerAuthentication for FooPayouts {}`.
4. A full `impl ConnectorIntegrationV2<Flow, PayoutFlowData, Req, Resp> for FooPayouts` for each real flow — `get_headers`, `get_content_type`, `get_url`, `get_request_body`, `build_request_v2`, `handle_response_v2`, `get_error_response_v2`.
5. A stub `impl ConnectorIntegrationV2<…>` whose only method is `get_url` returning `IntegrationError::connector_flow_not_implemented(...)` for each unimplemented flow, plus the bare marker-trait impl.

### Pattern C — non-generic with a file-local stub macro (`truelayer.rs`)

`truelayer.rs` declares its own `macro_rules! impl_unimplemented_payout_flow` and applies it to the seven flows it does not implement. This is the cleanest way to keep Pattern B from ballooning; copy it rather than repeating the stub body seven times.

## Code Examples

### Marker impl + macro flow (trustly)

```rust
// From crates/integrations/connector-integration/src/payout_connectors/trustly.rs
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutServiceTrait
    for TrustlyPayouts<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: TrustlyPayouts,
    curl_request: Json(AccountPayoutRequest),
    curl_response: AccountPayoutResponse,
    flow_name: PayoutTransfer,
    resource_common_data: PayoutFlowData,
    flow_request: PayoutTransferRequest,
    flow_response: PayoutTransferResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<
                PayoutTransfer,
                PayoutFlowData,
                PayoutTransferRequest,
                PayoutTransferResponse,
            >,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self
                .base_url(&req.resource_common_data.connectors)
                .to_string())
        }
    }
);
```

### Stub set (gotyme_sanlam — current-correct, includes `PayoutEligibility`)

```rust
// From crates/integrations/connector-integration/src/payout_connectors/gotyme_sanlam.rs
macros::macro_connector_payout_implementation!(
    connector: GotymeSanlamPayouts,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    payout_flows: [
        PayoutCreate,
        PayoutVoid,
        PayoutStage,
        PayoutCreateLink,
        PayoutCreateRecipient,
        PayoutEnrollDisburseAccount,
        PayoutEligibility
    ]
);

macros::macro_connector_flow_status_impls!(
    connector: GotymeSanlamPayouts,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [ServerAuthenticationToken],
);
```

### Provider registration (generic connectors must be monomorphized)

```rust
// From crates/integrations/connector-integration/src/types.rs
//   PayoutConnectorData::convert_connector
PayoutConnectorEnum::Santander => Box::new(payout_connectors::SantanderPayouts::new()),
PayoutConnectorEnum::Trustly => Box::new(payout_connectors::TrustlyPayouts::<
    domain_types::payment_method_data::DefaultPCIHolder,
>::new()),
```

## Integration Guidelines

1. Read `payout_connectors/gotyme_sanlam.rs` end-to-end (265 lines, the smallest complete example) and `payout_connectors/trustly.rs` (382 lines, the fullest macro example).
2. Decide generic-over-`T` vs non-generic. **Generic is the recommended default** — it is the only way to use the macros, and `PayoutConnectorData` monomorphizes at `DefaultPCIHolder` for you.
3. Create `payout_connectors/foo.rs` and `payout_connectors/foo/transformers.rs`.
4. In `transformers.rs`: `pub struct FooAuthType { .. }` with `impl TryFrom<&ConnectorSpecificConfig> for FooAuthType` matching **only** your variant and returning `IntegrationError::FailedToObtainAuthType` otherwise (`payout_connectors/trustly/transformers.rs`, `impl TryFrom<&ConnectorSpecificConfig> for TrustlyAuthType`).
5. Implement `ConnectorCommon`: `id`, `get_currency_unit`, `common_get_content_type`, `base_url` (reads `connectors.foo.base_url`), `get_auth_header`, `build_error_response`.
6. Implement the real flows.
7. Stub every remaining payout flow — via `macro_connector_payout_implementation!` (generic) or hand-written stubs / a local macro (non-generic).
8. Satisfy `ServerAuthentication`: real flow or stub.
9. `impl PayoutServiceTrait for FooPayouts {}` last — the compiler will now tell you exactly which of the eleven supertraits you missed.
10. Walk the 12 registration sites in the table above, in order.
11. Add `foo.base_url` to all three `config/*.toml` files.
12. `cargo check -p connector-integration` then `cargo check --workspace`.

## Best Practices

- **Read the access token off `PayoutFlowData`, never from a payment-side accessor.** The field is `access_token: Option<ServerAuthenticationTokenResponseData>`, so do not interpolate it into a header directly — use the accessor `PayoutFlowData::get_access_token()` (returns `Result<String, Error>`; `crates/types-traits/domain_types/src/payouts/payouts_types.rs`), or `get_access_token_data()` for the whole struct. `itaubank.rs` and `cybersource.rs` both do `let access_token = req.resource_common_data.get_access_token()...;` then `format!("Bearer {access_token}").into_masked()`.
- **Reuse the payment connector's error-response type** when one exists: `payout_connectors/trustly.rs` imports `crate::connectors::trustly::transformers::TrustlyErrorResponse` rather than duplicating it.
- **Reuse the existing `ConnectorSpecificConfig` variant** when the connector already exists as a payment connector. Adding a `FooPayouts` variant alongside `Foo` splits credentials across two shapes for no benefit.
- **Do not implement `ValidationTrait` / `IncomingWebhook` / `VerifyRedirectResponse` / `SourceVerification` / `BodyDecoding`.** Not in the supertrait list; no payout connector at HEAD has any of them.
- **Use `NO_ERROR_CODE` / `NO_ERROR_MESSAGE`** (`common_utils::consts`) in `build_error_response`, never `unwrap_or_default()` (`PATTERN_AUTHORING_SPEC.md` §11.7).
- **Map `payout_status` from the connector payload**, exhaustively, with an explicit `Unknown` arm and no `_ =>` at the mapping layer (`PATTERN_AUTHORING_SPEC.md` §11.9).
- **Prefer a file-local stub macro over repeated stub bodies** if you go non-generic (`truelayer.rs`, `macro_rules! impl_unimplemented_payout_flow`).

## Common Errors / Gotchas

1. **Assuming `PayoutServiceTrait` mirrors `ConnectorServiceTrait`.**
   - *Problem*: you write `impl ValidationTrait`, `impl IncomingWebhook`, `impl SourceVerification` for the payout connector and wonder why `should_do_access_token` is never called.
   - *Solution*: the supertrait list is `ConnectorCommon + ServerAuthentication + 9 payout markers`. Nothing else. The payout access-token decision is made by the composite/grpc layer, not by `ValidationTrait` on the payout connector.

2. **Copying trustly's stale `PayoutEligibility` comment.**
   - *Problem*: `payout_connectors/trustly.rs` says "`PayoutEligibility` has no arm in `macro_connector_payout_implementation!`, so its stub is still written out by hand." That was true before `3df5eb702`.
   - *Solution*: the arm exists. List `PayoutEligibility` in `payout_flows:` like `gotyme_sanlam.rs` does, or omit `payout_flows:` entirely to get the nine-flow default arm.

3. **Trying to use `create_all_prerequisites!` on a unit struct.**
   - *Problem*: `pub struct FooPayouts;` + `create_all_prerequisites!(connector_name: FooPayouts, generic_type: T, …)` produces a wall of "cannot find type `T`" / arity errors.
   - *Solution*: `create_all_prerequisites!` **generates** the connector struct itself — `pub struct $connector<$generic_type: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>` with a `_marker: PhantomData` field and a `pub const fn new() -> &'static Self` (see the `paste::paste!` block at the end of `macro_rules! create_all_prerequisites`). So do **not** declare the struct yourself when using the macro; and if you want a plain `pub struct FooPayouts;` unit struct, drop the macros and hand-write, as seven of the ten existing payout connectors do.

4. **Forgetting to monomorphize in `PayoutConnectorData::convert_connector`.**
   - *Problem*: `BoxedPayoutConnector` is `Box<&'static (dyn PayoutServiceTrait + Sync)>` — a non-generic trait object. `Box::new(FooPayouts::new())` on a generic struct will not infer.
   - *Solution*: `Box::new(payout_connectors::FooPayouts::<domain_types::payment_method_data::DefaultPCIHolder>::new())`, as Deutschebank / Trustly / GotymeSanlam do.

5. **Adding the connector to `ConnectorEnum` "so routing works".**
   - *Problem*: routing for payouts goes through `PayoutConnectorEnum`, selected by the `x-payout-connector` header or by `ForeignTryFrom<AuthType> for ConnectorVariant` returning `ConnectorVariant::Payout(..)`.
   - *Solution*: add to `ConnectorEnum` only if the connector also processes payments. `impl TryFrom<ConnectorEnum> for PayoutConnectorEnum` is a *fallback* for dual-role connectors, not a requirement.

6. **Chasing a certification failure that does not exist.**
   - *Problem*: you look for `connector_specs/foo/specs.json` and find no payout suites.
   - *Solution*: `IGNORE_SERVICES` in `crates/internal/integration-tests/src/bin/check_connector_specs.rs` contains `"PayoutService"`, and the eight non-eligibility payout flows are in `OUT_OF_SCOPE_FLOWS`. A payout-only connector never appears in `connector_specs/` at all, because that checker reads only `src/connectors`.

7. **Treating "out of scope for certification" as "do not build".**
   - *Problem*: `OUT_OF_SCOPE_FLOWS` is easy to misread as a prohibition.
   - *Solution*: the source says otherwise — the comment above the second half of that list reads "Each is a coverage gap, not a decision that it should never be covered." Payout flows are listed because no integration-test suite exists yet, not because they should not be implemented.

## Testing & Certification Notes

- **Unit tests** go in `payout_connectors/foo/transformers.rs` (or a `#[cfg(test)] mod test`): request-shape assertions, one test per status-mapping branch including the `Unknown` branch, and auth-type extraction failure on the wrong `ConnectorSpecificConfig` variant.
- **No `connector_specs` entry, no CI certification.** `.github/scripts/verify-new-connectors.sh` keys off a new spec directory under `crates/internal/integration-tests/src/connector_specs/`; a payout-only connector creates none, so it is not merge-gated by certification. That is a coverage gap, not permission to skip validation — record a manual sandbox run in the PR.
- **Integration scenarios worth exercising by hand**: happy-path transfer; transfer with `PayoutFlowData.access_token = None` (expect `FailedToObtainAuthType` if the connector needs OAuth); `PayoutGet` on an unknown payout id; processor decline mapped through `build_error_response`; malformed 2xx body → `ConnectorError::ResponseDeserializationFailed`.

## Cross-References

- Parent index: [./README.md](./README.md)
- Authoring spec: [./PATTERN_AUTHORING_SPEC.md](./PATTERN_AUTHORING_SPEC.md)
- Sibling category: [pattern_surcharge_connector.md](./pattern_surcharge_connector.md)
- Sibling category: [pattern_frm_connector.md](./pattern_frm_connector.md)
- Sibling category: [pattern_authenticator_connector.md](./pattern_authenticator_connector.md)
- Per-flow payout patterns: [pattern_payout_transfer.md](./pattern_payout_transfer.md), [pattern_payout_get.md](./pattern_payout_get.md), [pattern_payout_create.md](./pattern_payout_create.md)
- Merchant/credential auth: [pattern_server_authentication_token.md](./pattern_server_authentication_token.md)
- Macros: [macro_patterns_reference.md](./macro_patterns_reference.md)
- Utility helpers: [../utility_functions_reference.md](../utility_functions_reference.md)
- Types: [../types/types.md](../types/types.md)
