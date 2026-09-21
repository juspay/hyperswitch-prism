# Authenticator Connector Pattern

## Overview

An **authenticator connector** links and verifies a customer's *bank account* — it is **not 3DS**. This is Mechanism 4 in [`README.md`](./README.md): account linking / account verification, living in `crates/integrations/connector-integration/src/authenticator_connectors/`, a **sibling** of `connectors/`, not a subdirectory. Sole member at HEAD: `authenticator_connectors/plaid.rs`.

Nothing in this category authenticates a cardholder. If you are looking for the standalone 3DS trio (`PreAuthenticate` / `Authenticate` / `PostAuthenticate` on `PaymentFlowData`), you want [`pattern_preauthenticate.md`](./pattern_preauthenticate.md) and [`pattern_authentication_dispatch.md`](./pattern_authentication_dispatch.md), not this file. If you are looking for merchant/credential auth (OAuth tokens, wallet sessions), you want [`pattern_server_authentication_token.md`](./pattern_server_authentication_token.md).

`AuthenticatorServiceTrait<T>` requires exactly **three** flow markers, and they do **not** share one `ResourceCommonData`: `ClientAuthenticationToken` uses `MerchantAuthenticationFlowData`, while `PaymentMethodToken` and `GetPaymentMethod` use `PaymentFlowData`. That split is the category's signature trap.

### Key Components

- **Service trait**: `interfaces::connector_types::AuthenticatorServiceTrait<T>` — `crates/types-traits/interfaces/src/connector_types.rs`, `pub trait AuthenticatorServiceTrait`.
- **Boxed form**:
  ```rust
  // From crates/types-traits/interfaces/src/connector_types.rs
  pub type BoxedAuthenticatorConnector = Box<
      &'static (dyn AuthenticatorServiceTrait<domain_types::payment_method_data::DefaultPCIHolder>
                    + Sync),
  >;
  ```
  The `T` is pinned to `DefaultPCIHolder` at the boxing site.
- **Flow data**: `domain_types::merchant_authentication_flow_data::MerchantAuthenticationFlowData` for the token leg; `domain_types::connector_types::PaymentFlowData` for the other two.
- **Flow markers**: `domain_types::connector_flow::{ClientAuthenticationToken, PaymentMethodToken, GetPaymentMethod}` — `crates/types-traits/domain_types/src/connector_flow.rs`.
- **Category enum**: `domain_types::connector_types::AuthenticatorConnectorEnum` — `crates/types-traits/domain_types/src/connector_types.rs`.
- **Provider**: `connector_integration::types::AuthenticatorConnectorData` — `crates/integrations/connector-integration/src/types.rs`.
- **Module file**: `crates/integrations/connector-integration/src/authenticator_connectors.rs` (two lines today).
- **Reference connector**: `authenticator_connectors/plaid.rs`, `plaid/transformers.rs`, `plaid/test.rs`.
- **gRPC services**: `MerchantAuthenticationService` (token leg) and `PaymentMethodService` (the other two) — `crates/types-traits/grpc-api-types/proto/services.proto`.
- **Routing header**: `x-auth-connector` — `common_utils::consts::X_AUTHENTICATOR_CONNECTOR_NAME`.

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
MerchantAuthenticationService::CreateClientAuthenticationToken   PaymentMethodService::{Tokenize, Get}
    │  proto/services.proto                                          │  proto/services.proto
    ▼                                                                ▼
grpc-server/src/server/payments.rs                          grpc-server/src/server/payments.rs
  internal_sdk_session_token                                  handle_tokenize_authenticator_flow
  (connector_data_types: [ConnectorData<…>, AuthenticatorConnectorData])   internal_get_payment_method
    │                                                                ▼
    ▼                                                        AuthenticatorConnectorData
AuthenticatorConnectorData::from_connector_variant  →  variant.as_authenticator()
    │  connector-integration/src/types.rs
    ▼
BoxedAuthenticatorConnector = Box<&'static (dyn AuthenticatorServiceTrait<DefaultPCIHolder> + Sync)>
    │
    ├─▶ RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData,
    │                ClientAuthenticationTokenRequestData, PaymentsResponseData>
    ├─▶ RouterDataV2<PaymentMethodToken, PaymentFlowData,
    │                PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>
    └─▶ RouterDataV2<GetPaymentMethod, PaymentFlowData,
                     GetPaymentMethodData, GetPaymentMethodResponseData>
```

### Service Trait and its EXACT supertrait list

```rust
// From crates/types-traits/interfaces/src/connector_types.rs — `pub trait AuthenticatorServiceTrait`
pub trait AuthenticatorServiceTrait<T: PaymentMethodDataTypes>:
    ConnectorCommon + ValidationTrait + ClientAuthentication + PaymentTokenV2<T> + GetPaymentMethodV2
{
}
```

Five supertraits. It requires `ValidationTrait` (like surcharge, unlike payout) and it does **not** require `ServerAuthentication`, `IncomingWebhook`, `VerifyRedirectResponse`, `SourceVerification` or `BodyDecoding`.

`plaid.rs` nevertheless implements `IncomingWebhook`, `VerifyRedirectResponse`, `SourceVerification` and `BodyDecoding` as bare impls. They are not required by the bound; they are cheap defaults that keep the connector usable if it is ever routed through a path that needs them.

### Flow markers and their exact `ConnectorIntegrationV2` bindings

**Read the `ResourceCommonData` column carefully — it changes per row.** From `crates/types-traits/interfaces/src/connector_types.rs`:

| Marker | Marker trait | `ResourceCommonData` | Request | Response |
| --- | --- | --- | --- | --- |
| `ClientAuthenticationToken` | `ClientAuthentication` | **`MerchantAuthenticationFlowData`** | `ClientAuthenticationTokenRequestData` | `PaymentsResponseData` |
| `PaymentMethodToken` | `PaymentTokenV2<T>` | `PaymentFlowData` | `PaymentMethodTokenizationData<T>` | `PaymentMethodTokenResponse` |
| `GetPaymentMethod` | `GetPaymentMethodV2` | `PaymentFlowData` | `GetPaymentMethodData` | `GetPaymentMethodResponseData` |

```rust
// From crates/types-traits/interfaces/src/connector_types.rs — `pub trait ClientAuthentication`
pub trait ClientAuthentication:
    ConnectorIntegrationV2<
    connector_flow::ClientAuthenticationToken,
    MerchantAuthenticationFlowData,
    ClientAuthenticationTokenRequestData,
    PaymentsResponseData,
>
{
}
```

Note the **asymmetry**: the request type is `ClientAuthenticationTokenRequestData` but the response type is the generic `PaymentsResponseData`, not a `…TokenResponseData`. There is no `ClientAuthenticationTokenResponseData` type. The token itself is carried in `PaymentsResponseData` via the `ClientAuthenticationTokenData` enum (`crates/types-traits/domain_types/src/connector_types.rs`, `pub enum ClientAuthenticationTokenData`), whose Plaid arm is a tuple variant:

```rust
// From crates/types-traits/domain_types/src/connector_types.rs — pub enum ClientAuthenticationTokenData
/// Plaid Link token for bank account linking via Plaid Link SDK
Plaid(Box<PlaidClientAuthenticationResponse>),
```

The proto conversion lives in `crates/types-traits/domain_types/src/types.rs`, in the `ClientAuthenticationTokenData::Plaid(plaid_token) => …` arms, which map `link_token`, `expires_in_seconds` and `hosted_link_url` onto `grpc_api_types::payments::PlaidClientAuthenticationResponse`.

```rust
// From crates/types-traits/interfaces/src/connector_types.rs
pub trait PaymentTokenV2<T: PaymentMethodDataTypes>:
    ConnectorIntegrationV2<
    connector_flow::PaymentMethodToken,
    PaymentFlowData,
    PaymentMethodTokenizationData<T>,
    PaymentMethodTokenResponse,
>
{
}

pub trait GetPaymentMethodV2:
    ConnectorIntegrationV2<
    connector_flow::GetPaymentMethod,
    PaymentFlowData,
    GetPaymentMethodData,
    GetPaymentMethodResponseData,
>
{
}
```

`PaymentTokenV2<T>` is the only generic marker in the set; `ClientAuthentication` and `GetPaymentMethodV2` are not generic.

### FlowData types

`MerchantAuthenticationFlowData` (`crates/types-traits/domain_types/src/merchant_authentication_flow_data.rs`) deliberately carries **no** payment fields — no amount, no payment-method data, no address. It holds merchant identity, the resolved `connectors` base URLs, `connector_request_reference_id`, `test_mode`, `return_url`, `connector_feature_data`, `order_details` and a request id.

`PaymentFlowData` (`crates/types-traits/domain_types/src/connector_types.rs`, `pub struct PaymentFlowData`) is the ordinary payment flow data used by the other two legs.

### Request / response shapes

```rust
// From crates/types-traits/domain_types/src/connector_types.rs
pub struct PaymentMethodTokenResponse {
    pub token: String,
    pub connector_payment_method_id: Option<String>,
    pub status_code: u16,
}

pub struct GetPaymentMethodData {
    pub merchant_payment_method_id: Option<String>,
    pub connector_payment_method_id: Option<String>,
    pub customer: Option<CustomerInfo>,
    pub payment_method_type: PaymentMethodType,
    pub connector_feature_data: Option<common_utils::pii::SecretSerdeValue>,
    pub payment_method_token: Option<Secret<String>>,
}

pub struct GetPaymentMethodResponseData {
    pub merchant_payment_method_id: Option<String>,
    pub connector_payment_method_id: Option<String>,
    pub customer: Option<CustomerInfo>,
    pub payment_method_details: Option<payment_method_data::PaymentMethodDetails>,
    pub status_code: u16,
}
```

`ClientAuthenticationTokenRequestData` has 13 fields and `PaymentMethodTokenizationData<T>` 13; copy them from `connector_types.rs` rather than from any doc.

### Connector directory

```
crates/integrations/connector-integration/src/
├── authenticator_connectors.rs      # `pub mod plaid;` + `pub use self::plaid::Plaid;`
└── authenticator_connectors/
    ├── plaid.rs
    └── plaid/
        ├── test.rs                  # declared as `#[cfg(test)] mod test;` on line 1 of plaid.rs
        └── transformers.rs
```

## Connectors with Full Implementation

| Connector | Struct | Generic over `T`? | Macros | Flows (all real) | URLs |
| --- | --- | --- | --- | --- | --- |
| plaid | `Plaid<T>` (generated by `create_all_prerequisites!`) | yes | `create_all_prerequisites!`, `macro_connector_implementation!` ×3, `macro_connector_flow_status_impls!`, `create_amount_converter_wrapper!` | `ClientAuthenticationToken`, `PaymentMethodToken`, `GetPaymentMethod` | `POST {base}/link/token/create`, `POST {base}/item/public_token/exchange`, `POST {base}/auth/get` |

No stub implementations of the three required markers exist in this category.

## Registration Sites

| # | Site | File | What to add |
| --- | --- | --- | --- |
| 1 | Module file | `crates/integrations/connector-integration/src/authenticator_connectors.rs` | `pub mod foo;` + `pub use self::foo::Foo;` |
| 2 | Connector source | `crates/integrations/connector-integration/src/authenticator_connectors/foo.rs` (+ `foo/transformers.rs`, optional `foo/test.rs`) | `ConnectorCommon`, `ValidationTrait`, three markers, `AuthenticatorServiceTrait<T>` |
| 3 | Category enum | `crates/types-traits/domain_types/src/connector_types.rs`, `pub enum AuthenticatorConnectorEnum` | a variant; snake_case strum value is the `x-auth-connector` header value |
| 4 | `ForeignTryFrom<AuthType>` | same file, `impl ForeignTryFrom<AuthType> for AuthenticatorConnectorEnum` | `AuthType::Foo(_) => Ok(Self::Foo)` |
| 5 | `ConnectorVariant` | same file, `impl ForeignTryFrom<AuthType> for ConnectorVariant` | `AuthType::Foo(_) => Ok(Self::Authenticator(AuthenticatorConnectorEnum::Foo))` — Plaid's arm is exactly this |
| 6 | Provider | `crates/integrations/connector-integration/src/types.rs`, `AuthenticatorConnectorData::convert_connector` | `AuthenticatorConnectorEnum::Foo => Box::new(authenticator_connectors::Foo::<domain_types::payment_method_data::DefaultPCIHolder>::new())` |
| 7 | `ConnectorSpecificConfig` | `crates/types-traits/domain_types/src/router_data.rs`, `pub enum ConnectorSpecificConfig` | e.g. `Plaid { client_id, secret, client_name, base_url }` |
| 8 | `AuthType` → config | same file | `AuthType::Foo(foo) => Ok(Self::Foo { .. })` |
| 9 | proto config message | `crates/types-traits/grpc-api-types/proto/payment.proto` | `message FooConfig { .. }` + a field in `message ConnectorSpecificConfig`'s `oneof config` — see `PlaidConfig plaid = 141;` |
| 10 | `Connectors` struct | `crates/types-traits/domain_types/src/types.rs`, `pub struct Connectors` | `pub foo: ConnectorParams,` |
| 11 | URL patcher | same file, `pub fn patch_authenticator_connector_urls` | `AuthenticatorConnectorEnum::Foo => patched.foo.apply(params_patch),` |
| 12 | Config TOMLs | `config/development.toml`, `config/sandbox.toml`, `config/production.toml` | `foo.base_url = "…"` in each |
| 13 | Token payload (if the token leg returns a connector-shaped token) | `crates/types-traits/domain_types/src/connector_types.rs`, `pub enum ClientAuthenticationTokenData` — plus its proto conversion arms in `crates/types-traits/domain_types/src/types.rs` and the matching proto message | a tuple variant, as `Plaid(Box<PlaidClientAuthenticationResponse>)` does |

**Not required:**

- **`ConnectorEnum`** (the payment enum). `plaid` is **not** in it (`grep -n "pub enum ConnectorEnum" -A 200 crates/types-traits/domain_types/src/connector_types.rs | grep -i plaid` → the only hit is inside `AuthenticatorConnectorEnum`). It **is** in the proto `enum Connector` as `PLAID = 87;`, but that entry is not what routes authenticator flows.
- **`connector_specs/<name>/specs.json`** — `check_connector_specs.rs` scans only `crates/integrations/connector-integration/src/connectors`, so nothing under `authenticator_connectors/` is seen. `ls crates/internal/integration-tests/src/connector_specs/ | grep plaid` → empty. No certification gate.
- **FFI registration** — `crates/ffi/ffi/src/macros.rs` has no `AuthenticatorConnectorData` dispatch at all.
- **`ServerAuthentication` / `IncomingWebhook` / `VerifyRedirectResponse` / `SourceVerification` / `BodyDecoding`** — not supertraits of `AuthenticatorServiceTrait`.

`config/superposition.toml` **does** list `plaid` in its `connector` schema enum and gives it a `connector_base_url` context block for both default and `environment = "production"`. That is an optional runtime base-URL override, not a requirement — `gotyme_sanlam`, `kount` and `interpayments` all work without one.

## Which Macros Apply

| Macro | Applies? | Notes |
| --- | --- | --- |
| `macros::create_all_prerequisites!` | **Yes** | `plaid.rs` declares all three flows in `api: [ … ]`; the macro generates `pub struct Plaid<T>` and `Plaid::new()` |
| `macros::macro_connector_implementation!` | **Yes** | one per flow. `resource_common_data: MerchantAuthenticationFlowData` for the token leg, `PaymentFlowData` for the other two |
| `macros::macro_connector_flow_status_impls!` | **Yes** | `plaid.rs` uses the **two-list** form: `not_implemented: [ … ]` *and* `not_supported: [ … ]` |
| `crate::common_macros::create_amount_converter_wrapper!` | **Yes** | `plaid.rs`: `amount_type: FloatMajorUnit` |
| `macros::macro_connector_payout_implementation!` | No | payout only |
| `macros::frm_flow_not_implemented!` | No | FRM only |

### The two-list `macro_connector_flow_status_impls!` form

`not_implemented` and `not_supported` produce *different* errors. From `macro_rules! flow_status_emit` in `crates/integrations/connector-integration/src/connectors/macros.rs`, the two arms are byte-identical except for the constructor: `IntegrationError::connector_flow_not_implemented(...)` versus `IntegrationError::connector_flow_not_supported(...)`. Use `not_implemented` for "this connector could do it, nobody has written it yet" and `not_supported` for "the vendor's product has no such concept".

Plaid's split, verbatim:

```rust
// From crates/integrations/connector-integration/src/authenticator_connectors/plaid.rs
macros::macro_connector_flow_status_impls!(
    connector: Plaid,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        Authorize,
        PSync,
        Refund,
        SetupMandate,
        MandateRevoke,
        RepeatPayment,
        CreateConnectorCustomer,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        PaymentMethodEligibility,
    ],
    not_supported: [
        Capture,
        RSync,
        Void,
        VoidPC,
        VoidPostRefund,
        IncrementalAuthorization,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        CreateOrder,
        Accept,
        DefendDispute,
        SubmitEvidence,
    ]
);
```

Note that `PreAuthenticate` / `Authenticate` / `PostAuthenticate` appear under `not_implemented` — a plain confirmation that **this category is not 3DS**.

These stubs are not required to satisfy `AuthenticatorServiceTrait` (which needs only three markers). They are defensive: any accidental route to the connector returns a precise `connector_flow_not_implemented` / `connector_flow_not_supported` error instead of failing somewhere less legible. Keep them.

Beware: some arms of `expand_flow_status_impl!` also emit the *marker* trait. The `ServerAuthenticationToken` arm emits `impl ServerAuthentication for $c<$g> {}` alongside the stub, and the `ServerSessionAuthenticationToken` arm emits `impl ServerSessionAuthentication`. Listing those two in `not_supported` therefore gives you the marker impls for free — do not also write them by hand, or you get a conflicting-implementation error.

## gRPC Surface

The three flows are spread across **two** services, and neither is `PaymentMethodAuthenticationService`.

```proto
// From crates/types-traits/grpc-api-types/proto/services.proto — service MerchantAuthenticationService
rpc CreateClientAuthenticationToken(MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest)
    returns (MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse);
```

```proto
// From crates/types-traits/grpc-api-types/proto/services.proto — service PaymentMethodService
rpc Tokenize(PaymentMethodServiceTokenizeRequest) returns (PaymentMethodServiceTokenizeResponse);
rpc Get(PaymentMethodServiceGetRequest) returns (PaymentMethodServiceGetResponse);
```

**No authenticator flow arrives via `EventService/NotifyConnector`.** `enum NotifyEventType` (`crates/types-traits/grpc-api-types/proto/payment.proto`) has only surcharge and FRM members — contrast [`pattern_surcharge_connector.md`](./pattern_surcharge_connector.md) and [`pattern_frm_connector.md`](./pattern_frm_connector.md), where two and three markers respectively are notify-driven.

Handlers, all in `crates/grpc-server/grpc-server/src/server/payments.rs`:

- `internal_sdk_session_token` — `flow_marker: ClientAuthenticationToken`, `resource_common_data_type: MerchantAuthenticationFlowData`, `connector_data_types: [ConnectorData<DefaultPCIHolder>, AuthenticatorConnectorData]`.
- `handle_tokenize_authenticator_flow` — a hand-written handler that resolves `AuthenticatorConnectorData::from_connector_variant(&metadata_payload.connector)` and errors with "Invalid connector type for authenticator tokenize flow" when the variant is not `Authenticator`.
- `internal_get_payment_method` — `flow_marker: GetPaymentMethod`, `connector_data_types: [ConnectorData<DefaultPCIHolder>, AuthenticatorConnectorData]`.

The two-entry `connector_data_types` list means the payment registry is tried first and the authenticator registry second, so an authenticator connector is only reached when the request resolves to `ConnectorVariant::Authenticator(..)`.

Composite entry points that consult the authenticator registry, all in `crates/internal/composite-service/src/payment_methods.rs`: its `create_server_authentication_token` helper branches on `ConnectorVariant::Authenticator(c)` to call `should_do_access_token` on `AuthenticatorConnectorData`, and is reached from `process_create`, `process_get`, `process_recharge` **and** `process_eligibility` — i.e. all four `CompositePaymentMethodService` rpcs (`Create`, `Get`, `Recharge`, `Eligibility`). The second branch, in `create_payment_method_token`, calls `should_do_payment_method_token` and is reached only from `process_get`.

Routing: send `x-auth-connector: foo`, or supply an `AuthType` whose `ConnectorVariant` conversion yields `Authenticator(..)`. The header branch lives in `crates/types-traits/ucs_interface_common/src/metadata.rs` under `consts::X_AUTHENTICATOR_CONNECTOR_NAME`.

## Common Implementation Patterns

1. `crate::common_macros::create_amount_converter_wrapper!(connector_name: Foo, amount_type: …);`
2. `macros::create_all_prerequisites!(connector_name: Foo, generic_type: T, api: [ ClientAuthenticationToken, PaymentMethodToken, GetPaymentMethod ], amount_converters: [], member_functions: {});` — the macro declares `pub struct Foo<T>`; do not write one yourself.
3. `impl<T: …> ConnectorCommon for Foo<T>` — `id`, `get_currency_unit`, `common_get_content_type`, `base_url`, `get_auth_header`, `build_error_response`.
4. `impl<T: …> ValidationTrait for Foo<T>` — override `should_do_payment_method_token` to `true` if the composite layer must run the tokenize leg (Plaid does exactly this).
5. Optional bare impls: `IncomingWebhook`, `VerifyRedirectResponse`, `SourceVerification`, `BodyDecoding`.
6. Bare marker impls: `ClientAuthentication`, `PaymentTokenV2<T>`, `GetPaymentMethodV2`, then `AuthenticatorServiceTrait<T>`.
7. Three `macro_connector_implementation!` calls — **watch the `resource_common_data` per flow**.
8. One `macro_connector_flow_status_impls!` with both lists.

### Credentials in the body, not the headers

Plaid inlines `client_id` / `secret` in every JSON body, so `get_auth_header` validates the config and returns only `Content-Type`:

```rust
// From crates/integrations/connector-integration/src/authenticator_connectors/plaid.rs — impl ConnectorCommon
fn get_auth_header(
    &self,
    auth_type: &ConnectorSpecificConfig,
) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
    // Plaid inlines credentials in the JSON body, not HTTP headers.
    // Auth is handled per-request in transformers; return just Content-Type here.
    let _ = plaid::PlaidAuthType::try_from(auth_type)?;
    Ok(vec![(
        headers::CONTENT_TYPE.to_string(),
        self.common_get_content_type().to_string().into(),
    )])
}
```

The `let _ = …try_from(auth_type)?;` is deliberate: it makes a misconfigured merchant fail at header-build time with `FailedToObtainAuthType` rather than sending an unauthenticated request.

## Code Examples

### The flow with the *other* flow data

```rust
// From crates/integrations/connector-integration/src/authenticator_connectors/plaid.rs
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Plaid,
    curl_request: Json(plaid::PlaidLinkTokenRequest),
    curl_response: plaid::PlaidLinkTokenResponse,
    flow_name: ClientAuthenticationToken,
    resource_common_data: MerchantAuthenticationFlowData,
    flow_request: ClientAuthenticationTokenRequestData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<
                ClientAuthenticationToken,
                MerchantAuthenticationFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}/link/token/create",
                req.resource_common_data.connectors.plaid.base_url
            ))
        }
        …
    }
);
```

### …and the two that use `PaymentFlowData`

```rust
// From crates/integrations/connector-integration/src/authenticator_connectors/plaid.rs
    flow_name: PaymentMethodToken,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentMethodTokenizationData<T>,
    flow_response: PaymentMethodTokenResponse,
…
    flow_name: GetPaymentMethod,
    resource_common_data: PaymentFlowData,
    flow_request: GetPaymentMethodData,
    flow_response: GetPaymentMethodResponseData,
```

### Marker impls, in order

```rust
// From crates/integrations/connector-integration/src/authenticator_connectors/plaid.rs
impl<T: …> connector_types::ClientAuthentication for Plaid<T> {}
impl<T: …> connector_types::PaymentTokenV2<T> for Plaid<T> {}
impl<T: …> connector_types::GetPaymentMethodV2 for Plaid<T> {}
impl<T: …> connector_types::AuthenticatorServiceTrait<T> for Plaid<T> {}
```

Unlike `FrmServiceTrait`, `AuthenticatorServiceTrait<T>` **is** implemented generically — it takes `T` as a parameter rather than pinning it, so `Plaid<T>` satisfies it for every `T`. Only the boxing site pins `DefaultPCIHolder`.

### Provider registration

```rust
// From crates/integrations/connector-integration/src/types.rs — AuthenticatorConnectorData::convert_connector
AuthenticatorConnectorEnum::Plaid => Box::new(authenticator_connectors::Plaid::<
    domain_types::payment_method_data::DefaultPCIHolder,
>::new()),
```

## Integration Guidelines

1. Confirm you are in this category and not one of the three auth mechanisms in [`README.md`](./README.md). Account linking → here. Cardholder 3DS → `pattern_preauthenticate.md`. Merchant OAuth → `pattern_server_authentication_token.md`.
2. Read `authenticator_connectors/plaid.rs` end to end (about 400 lines) plus `plaid/transformers.rs`.
3. Create `authenticator_connectors/foo.rs`, `foo/transformers.rs`, and `foo/test.rs` (declare it as `#[cfg(test)] mod test;` on line 1, as `plaid.rs` does).
4. In `transformers.rs`: `FooAuthType` + `impl TryFrom<&ConnectorSpecificConfig>`, the three request/response pairs, and `FooErrorResponse`.
5. In `foo.rs`, follow the eight steps under "Common Implementation Patterns".
6. Put `impl AuthenticatorServiceTrait<T> for Foo<T> {}` last so the compiler enumerates the missing supertraits.
7. Walk the 13 registration sites.
8. Add `foo.base_url` to all three `config/*.toml` files.
9. `cargo check -p connector-integration` then `cargo check --workspace`.
10. Exercise `MerchantAuthenticationService/CreateClientAuthenticationToken`, then `PaymentMethodService/Tokenize`, then `PaymentMethodService/Get`, all with `x-auth-connector: foo`.

## Best Practices

- **Get the `resource_common_data` right per flow.** One `MerchantAuthenticationFlowData`, two `PaymentFlowData`. A copy-paste across the three `macro_connector_implementation!` blocks silently changes the binding: see the table under [Flow markers and their exact `ConnectorIntegrationV2` bindings](#architecture-overview) and re-read `plaid.rs`'s three `resource_common_data:` lines before you build.
- **Do not invent `ClientAuthenticationTokenResponseData`.** It does not exist; the response slot is `PaymentsResponseData`. Return the connector's token through a `ClientAuthenticationTokenData` variant (`connector_types.rs`).
- **Override `should_do_payment_method_token`** if the linking flow must run before a payment can use the account. Plaid returns `true`; the composite layer reads it through `AuthenticatorConnectorData` (`crates/internal/composite-service/src/payment_methods.rs`).
- **Validate the auth config even when credentials go in the body** — the `let _ = FooAuthType::try_from(auth_type)?;` idiom.
- **Use `NO_ERROR_CODE` / `NO_ERROR_MESSAGE`** in `build_error_response`, as `plaid.rs` does, never `unwrap_or_default()` (`PATTERN_AUTHORING_SPEC.md` §11.7).
- **Ship the tests.** With no certification gate for this category, `foo/test.rs` is the only automated coverage the connector will have.

## Common Errors / Gotchas

1. **Treating this as 3DS.**
   - *Problem*: you reach for `PreAuthenticate` / `Authenticate` / `PostAuthenticate` and `PaymentMethodAuthenticationService`.
   - *Solution*: those three markers are in Plaid's `not_implemented` list. The category's markers are `ClientAuthenticationToken`, `PaymentMethodToken`, `GetPaymentMethod`, served by `MerchantAuthenticationService` and `PaymentMethodService`.

2. **Using one `resource_common_data` for all three flows.**
   - *Problem*: `MerchantAuthenticationFlowData` for `PaymentMethodToken` (or `PaymentFlowData` for `ClientAuthenticationToken`) produces an unsatisfied `ConnectorIntegrationV2` bound with a long, opaque type error.
   - *Solution*: consult the binding table above for each `macro_connector_implementation!` call.

3. **Expecting a symmetric token response type.**
   - *Problem*: you write `ClientAuthenticationTokenResponseData` by analogy with `ServerAuthenticationTokenResponseData`.
   - *Solution*: `pub trait ClientAuthentication` binds `PaymentsResponseData` as the response. Only the *server*-side token traits have matching `…ResponseData` types.

4. **Adding the connector to `ConnectorEnum`.**
   - *Problem*: you assume every connector needs a payment-enum entry.
   - *Solution*: `plaid` is not in `ConnectorEnum`. Routing goes through `AuthenticatorConnectorEnum` + `x-auth-connector`, or through `ForeignTryFrom<AuthType> for ConnectorVariant` returning `ConnectorVariant::Authenticator(..)`.

5. **Hand-writing `impl ServerAuthentication` while also listing `ServerAuthenticationToken` in `not_supported`.**
   - *Problem*: conflicting implementations — the `expand_flow_status_impl!` arm for `ServerAuthenticationToken` already emits `impl ServerAuthentication for $c<$g> {}` (and the `ServerSessionAuthenticationToken` arm emits `impl ServerSessionAuthentication`).
   - *Solution*: pick one. Listing it in the macro is the shorter path.

6. **Looking for a certification failure to fix.**
   - *Problem*: you cannot find `connector_specs/foo/`.
   - *Solution*: `check_connector_specs.rs` reads only `crates/integrations/connector-integration/src/connectors`, so `authenticator_connectors/` is invisible to it and `.github/scripts/verify-new-connectors.sh` never fires. That is a coverage gap, not a pass — write real tests in `foo/test.rs` and record a manual sandbox transcript in the PR.

7. **Forgetting the `x-auth-connector` header.**
   - *Problem*: `handle_tokenize_authenticator_flow` returns `IntegrationError::NotSupported { message: "Invalid connector type for authenticator tokenize flow", .. }`.
   - *Solution*: the request resolved to a non-`Authenticator` `ConnectorVariant`. Send the header, or check your `ForeignTryFrom<AuthType> for ConnectorVariant` arm.

## Testing & Certification Notes

- **Unit tests** in `authenticator_connectors/foo/test.rs` — this is the only category-wide automated coverage available. `plaid/test.rs` is the model.
- **What to cover**: `FooAuthType::try_from` on the wrong `ConnectorSpecificConfig` variant → `FailedToObtainAuthType`; link-token request shape including inlined credentials; token-exchange response → `PaymentMethodTokenResponse { token, connector_payment_method_id, status_code }`; account-fetch response → `GetPaymentMethodResponseData`; error-body parsing with both a present and an absent `display_message`.
- **No certification gate.** No `connector_specs/` directory is created for this category, so `verify-new-connectors.sh` does not gate the PR. Compensate with tests and a documented manual run of all three legs.
- **Manual scenarios**: create link token; exchange a public token; fetch account details for the exchanged item; exchange an already-consumed public token (expect the connector's error mapped through `build_error_response`); malformed 2xx body → `ConnectorError::ResponseDeserializationFailed`.

## Cross-References

- Parent index: [./README.md](./README.md)
- Authoring spec: [./PATTERN_AUTHORING_SPEC.md](./PATTERN_AUTHORING_SPEC.md)
- Sibling category: [pattern_payout_connector.md](./pattern_payout_connector.md)
- Sibling category: [pattern_surcharge_connector.md](./pattern_surcharge_connector.md)
- Sibling category: [pattern_frm_connector.md](./pattern_frm_connector.md)
- The same marker in the merchant-auth mechanism: [pattern_client_authentication_token.md](./pattern_client_authentication_token.md)
- Token flow: [pattern_payment_method_token.md](./pattern_payment_method_token.md)
- Not this category — cardholder 3DS: [pattern_preauthenticate.md](./pattern_preauthenticate.md)
- Macros: [macro_patterns_reference.md](./macro_patterns_reference.md)
- Utility helpers: [../utility_functions_reference.md](../utility_functions_reference.md)
- Types: [../types/types.md](../types/types.md)
