# Surcharge Connector Pattern

## Overview

A **surcharge connector** quotes and reconciles a card surcharge (a "convenience fee" priced from the BIN, amount, and postal code). It is a distinct connector category living in `crates/integrations/connector-integration/src/surcharge_connectors/` — a **sibling** of `connectors/`, not a subdirectory — behind `SurchargeServiceTrait`, `SurchargeConnectorEnum`, `SurchargeConnectorData`, and the `SurchargeService` gRPC service.

There is exactly one surcharge connector at HEAD: `surcharge_connectors/interpayments.rs`. It matters far beyond its size, because **it is the counter-example to GRACE's most-repeated rule**: `InterPayments` is a plain non-generic unit struct that uses **no** `create_all_prerequisites!`, **no** `macro_connector_implementation!`, and **no** `macro_connector_flow_status_impls!`. Every one of its three flows is a hand-written `impl ConnectorIntegrationV2` block. Do not "fix" it, and do not assume the macro path is available here.

Three flow markers, but only **one** dedicated rpc: `SurchargeService/Calculate`. The other two — `SurchargePaymentSucceeded` and `SurchargeRefundSucceeded` — arrive through `EventService/NotifyConnector`, selected by `NotifyEventType`.

### Key Components

- **Service trait**: `interfaces::connector_types::SurchargeServiceTrait` — `crates/types-traits/interfaces/src/connector_types.rs`, `pub trait SurchargeServiceTrait`.
- **Boxed form**: `pub type BoxedSurchargeConnector = Box<&'static (dyn SurchargeServiceTrait + Sync)>` — same file. Non-generic `dyn`.
- **Flow data**: `domain_types::surcharge::surcharge_types::SurchargeFlowData` — `crates/types-traits/domain_types/src/surcharge/surcharge_types.rs`, `pub struct SurchargeFlowData`.
- **Flow markers**: `domain_types::connector_flow::{SurchargeCalculate, SurchargePaymentSucceeded, SurchargeRefundSucceeded}` — `crates/types-traits/domain_types/src/connector_flow.rs`.
- **Category enum**: `domain_types::connector_types::SurchargeConnectorEnum` — `crates/types-traits/domain_types/src/connector_types.rs`.
- **Provider**: `connector_integration::types::SurchargeConnectorData` — `crates/integrations/connector-integration/src/types.rs`.
- **Module file**: `crates/integrations/connector-integration/src/surcharge_connectors.rs` (two lines today).
- **Reference connector**: `surcharge_connectors/interpayments.rs` + `surcharge_connectors/interpayments/transformers.rs`.
- **gRPC services**: `SurchargeService` and `EventService` — `crates/types-traits/grpc-api-types/proto/services.proto`.
- **Routing header**: `x-surcharge-connector` — `common_utils::consts::X_SURCHARGE_CONNECTOR_NAME`.

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
SurchargeService::Calculate (gRPC)              EventService::NotifyConnector (gRPC)
    │  proto/services.proto                          │  proto/services.proto
    ▼                                                ▼
internal_calculate                            match NotifyEventType { … }
    │  grpc-server/src/server/surcharges.rs      │  grpc-server/src/server/events.rs
    │                                            ├─ SurchargePaymentSucceeded
    │                                            └─ SurchargeRefundSucceeded
    ▼                                                ▼
SurchargeConnectorData::from_connector_variant  →  variant.as_surcharge()
    │  connector-integration/src/types.rs
    ▼
BoxedSurchargeConnector = Box<&'static (dyn SurchargeServiceTrait + Sync)>
    ▼
RouterDataV2<SurchargeFlow, SurchargeFlowData, Surcharge*Request, Surcharge*Response>
    └─▶ ConnectorIntegrationV2<SurchargeFlow, SurchargeFlowData, …>
```

### Service Trait and its EXACT supertrait list

```rust
// From crates/types-traits/interfaces/src/connector_types.rs — `pub trait SurchargeServiceTrait`
pub trait SurchargeServiceTrait:
    ConnectorCommon
    + ValidationTrait
    + SurchargeCalculateV2
    + SurchargePaymentSucceededV2
    + SurchargeRefundSucceededV2
{
}
```

Five supertraits. Note the asymmetry with `PayoutServiceTrait`: surcharge **does** require `ValidationTrait`, and **does not** require `ServerAuthentication`. It also does not require `IncomingWebhook`, `VerifyRedirectResponse`, `SourceVerification` or `BodyDecoding` — and `interpayments.rs` implements none of them (`grep -rln "SourceVerification\|BodyDecoding" crates/integrations/connector-integration/src/surcharge_connectors/` → empty).

`impl ValidationTrait for InterPayments {}` is a bare impl: every `ValidationTrait` method has a default (`crates/types-traits/interfaces/src/connector_types.rs`, `pub trait ValidationTrait`). Override one only if the surcharge connector genuinely needs it.

### Flow markers and their exact `ConnectorIntegrationV2` bindings

All three use `SurchargeFlowData` as `ResourceCommonData`. From `crates/types-traits/interfaces/src/connector_types.rs`:

| Marker | Marker trait | Request | Response | Reached by |
| --- | --- | --- | --- | --- |
| `SurchargeCalculate` | `SurchargeCalculateV2` | `SurchargeCalculateRequest` | `SurchargeCalculateResponse` | `SurchargeService/Calculate` |
| `SurchargePaymentSucceeded` | `SurchargePaymentSucceededV2` | `SurchargePaymentSucceededRequest` | `SurchargePaymentSucceededResponse` | `EventService/NotifyConnector` + `NotifyEventType::SURCHARGE_PAYMENT_SUCCEEDED` |
| `SurchargeRefundSucceeded` | `SurchargeRefundSucceededV2` | `SurchargeRefundSucceededRequest` | `SurchargeRefundSucceededResponse` | `EventService/NotifyConnector` + `NotifyEventType::SURCHARGE_REFUND_SUCCEEDED` |

```rust
// From crates/types-traits/interfaces/src/connector_types.rs — `pub trait SurchargeCalculateV2`
pub trait SurchargeCalculateV2:
    ConnectorIntegrationV2<
    connector_flow::SurchargeCalculate,
    SurchargeFlowData,
    SurchargeCalculateRequest,
    SurchargeCalculateResponse,
>
{
}
```

### FlowData type

```rust
// From crates/types-traits/domain_types/src/surcharge/surcharge_types.rs — `pub struct SurchargeFlowData`
pub struct SurchargeFlowData {
    pub merchant_id: common_utils::id_type::MerchantId,
    pub connector_request_reference_id: String,
    pub connectors: Arc<Connectors>,
    pub raw_connector_response: Option<Secret<String>>,
    pub typed_connector_response: Option<String>,
    pub raw_connector_request: Option<Secret<String>>,
    pub typed_connector_request: Option<String>,
    pub connector_response_headers: Option<http::HeaderMap>,
}
```

Eight fields — no `access_token`, no `payment_id`, no amount. Everything the connector needs about the transaction is on the *request*, not on the flow data.

### Request / response shapes

```rust
// From crates/types-traits/domain_types/src/surcharge/surcharge_types.rs
pub struct SurchargeCalculateRequest {
    pub amount: MinorUnit,
    pub currency: Currency,
    pub previous_connector_surcharge_id: Option<String>,
    pub surcharge_strategy: Option<SurchargeStrategy>,
    pub card_bin: String,
    pub postal_code: Secret<String>,
    pub country: Option<common_enums::CountryAlpha2>,
}

pub struct SurchargeCalculateResponse {
    pub connector_response_reference_id: Option<String>,
    pub surcharge_amount: MinorUnit,
    pub surcharge_rate_percent: f64,
    pub connector_surcharge_id: String,
    pub currency: Currency,
}

pub struct SurchargePaymentSucceededRequest { pub connector_surcharge_id: String }
pub struct SurchargeRefundSucceededRequest  { pub connector_surcharge_id: String }
pub struct SurchargePaymentSucceededResponse { pub status_code: u16 }
pub struct SurchargeRefundSucceededResponse  { pub status_code: u16 }
```

The two notify responses carry **only** `status_code`. `interpayments.rs` builds them literally in `handle_response_v2` after parsing and event-logging the connector body — the parsed body is recorded on the event and on `resource_common_data`, then discarded.

### Connector directory

```
crates/integrations/connector-integration/src/
├── surcharge_connectors.rs        # `pub mod interpayments;` + `pub use self::interpayments::InterPayments;`
└── surcharge_connectors/
    ├── interpayments.rs
    └── interpayments/
        └── transformers.rs
```

## Connectors with Full Implementation

| Connector | Struct | Generic over `T`? | Macros used | Flows | URLs |
| --- | --- | --- | --- | --- | --- |
| interpayments | `InterPayments` (unit struct) | **no** | only `common_macros::create_amount_converter_wrapper!` | `SurchargeCalculate`, `SurchargePaymentSucceeded`, `SurchargeRefundSucceeded` — all three real | `POST {base}/ch`, `POST {base}/ch/sale`, `POST {base}/ch/refund` |

There are no stub implementations in this category: the sole connector implements all three markers.

## Registration Sites

| # | Site | File | What to add |
| --- | --- | --- | --- |
| 1 | Module file | `crates/integrations/connector-integration/src/surcharge_connectors.rs` | `pub mod foo;` + `pub use self::foo::Foo;` |
| 2 | Connector source | `crates/integrations/connector-integration/src/surcharge_connectors/foo.rs` (+ `foo/transformers.rs`) | `ConnectorCommon`, `ValidationTrait`, `SurchargeServiceTrait`, the three markers, three `ConnectorIntegrationV2` impls |
| 3 | Category enum | `crates/types-traits/domain_types/src/connector_types.rs`, `pub enum SurchargeConnectorEnum` | a variant; `#[strum(serialize_all = "snake_case")]` yields the `x-surcharge-connector` value |
| 4 | `ForeignTryFrom<AuthType>` | same file, `impl ForeignTryFrom<AuthType> for SurchargeConnectorEnum` | `AuthType::Foo(_) => Ok(Self::Foo)` |
| 5 | `ConnectorVariant` | same file, `impl ForeignTryFrom<AuthType> for ConnectorVariant` | `AuthType::Foo(_) => Ok(Self::Surcharge(SurchargeConnectorEnum::Foo))` — see the existing `AuthType::Interpayments(_)` arm |
| 6 | Provider | `crates/integrations/connector-integration/src/types.rs`, `SurchargeConnectorData::convert_connector` | `SurchargeConnectorEnum::Foo => Box::new(surcharge_connectors::Foo::new())` |
| 7 | `ConnectorSpecificConfig` | `crates/types-traits/domain_types/src/router_data.rs`, `pub enum ConnectorSpecificConfig` | e.g. `Interpayments { api_key: Secret<String>, base_url: Option<String> }` |
| 8 | `AuthType` → config | same file, `AuthType::Foo(foo) => Ok(Self::Foo { .. })` | required fields via `.ok_or_else(err)?` |
| 9 | proto config message | `crates/types-traits/grpc-api-types/proto/payment.proto` | `message FooConfig { .. }` + a field in `message ConnectorSpecificConfig`'s `oneof config` — see `InterpaymentsConfig interpayments = 126;` |
| 10 | `Connectors` struct | `crates/types-traits/domain_types/src/types.rs`, `pub struct Connectors` | `pub foo: ConnectorParams,` |
| 11 | URL patcher | same file, `pub fn patch_surcharge_connector_urls` | `SurchargeConnectorEnum::Foo => patched.foo.apply(params_patch),` |
| 12 | Config TOMLs | `config/development.toml`, `config/sandbox.toml`, `config/production.toml` | `foo.base_url = "…"` in each |

**Not required:**

- **`ConnectorEnum`** and the proto **`enum Connector`** — `interpayments` is in neither. Verify: `awk '/^enum Connector \{/,/^\}/' crates/types-traits/grpc-api-types/proto/payment.proto | grep -i interpayments` → empty.
- **`config/superposition.toml`** — `interpayments` is absent from that file's `connector` schema enum.
- **`connector_specs/<name>/specs.json`** — `check_connector_specs.rs` reads only `src/connectors`, so a surcharge connector never appears there and is never certification-gated. (Note that `SurchargeService` is *not* in `IGNORE_SERVICES`, which is `&["PayoutService", "DisputeService"]` — surcharge is exempt only because the directory is not scanned.)
- **FFI registration** — `crates/ffi/ffi/src/macros.rs` dispatches through `SurchargeConnectorData::get_connector_by_name(&connector)`.
- **`IncomingWebhook`, `VerifyRedirectResponse`, `ServerAuthentication`, `SourceVerification`, `BodyDecoding`** — not supertraits of `SurchargeServiceTrait`.

## Which Macros Apply

| Macro | Applies? | Why |
| --- | --- | --- |
| `macros::create_all_prerequisites!` | **No** | mandatory `generic_type:` parameter; `InterPayments` is a unit struct |
| `macros::macro_connector_implementation!` | **No** | same |
| `macros::macro_connector_flow_status_impls!` | **No** | its arms cover payment flows only; there is no surcharge arm in `macro_rules! expand_flow_status_impl` |
| `macros::macro_connector_payout_implementation!` | **No** | payout only |
| `common_macros::create_amount_converter_wrapper!` | **Yes** | `interpayments.rs` uses `common_macros::create_amount_converter_wrapper!(connector_name: InterPayments, amount_type: FloatMajorUnit);` |
| `crate::connectors::macros::serialize_typed_connector_payload` | **Yes** (a function, not a macro) | used inside `build_error_response` |
| `crate::with_error_response_body!` / `crate::finalize_connector_response!` | **Yes** | both used by `interpayments.rs` |

**This is the carve-out.** GRACE's general rule — "always use `create_all_prerequisites!` + `macro_connector_implementation!`, and always emit `macro_connector_flow_status_impls!`" — is a rule about **payment** connectors in `src/connectors/`. It does not hold here. `PATTERN_AUTHORING_SPEC.md` §11.13 ("all 111 connectors on HEAD invoke `macro_connector_flow_status_impls!`") counts payment connectors; `grep -rln "macro_connector_flow_status_impls" crates/integrations/connector-integration/src/surcharge_connectors/` returns nothing.

If you want the macro path for a *new* surcharge connector, you would first have to make the struct generic over `T` — and `create_all_prerequisites!` generates the struct for you, including a `_marker: PhantomData<T>` field. That is untested territory for this category: no surcharge connector does it. Following `interpayments.rs` is the safe route.

## gRPC Surface

```proto
// From crates/types-traits/grpc-api-types/proto/services.proto — service SurchargeService
rpc Calculate(SurchargeServiceCalculateRequest) returns (SurchargeServiceCalculateResponse);
```

One rpc. The other two flows arrive on `EventService`:

```proto
// From crates/types-traits/grpc-api-types/proto/services.proto — service EventService
rpc NotifyConnector(NotifyConnectorRequest) returns (NotifyConnectorResponse);
```

```proto
// From crates/types-traits/grpc-api-types/proto/payment.proto — enum NotifyEventType
SURCHARGE_PAYMENT_SUCCEEDED = 1; // Surcharge payment succeeded
SURCHARGE_REFUND_SUCCEEDED = 2;  // Surcharge refund succeeded
```

`NotifyConnectorRequest.content` is a `NotifyConnectorContent` whose `oneof` carries `SurchargeContent surcharge_content = 1;`, and `message SurchargeContent` holds a single `string connector_surcharge_id = 1;` — which is exactly the single field of both notify request structs.

Dispatch: `crates/grpc-server/grpc-server/src/server/events.rs` matches `NotifyEventType` and calls `Self::handle_payment_surcharge_notify` / `Self::handle_refund_surcharge_notify`. Verify:

```bash
grep -n "NotifyEventType::Surcharge" crates/grpc-server/grpc-server/src/server/events.rs
```

`SurchargeService/Calculate` is served by `internal_calculate` in `crates/grpc-server/grpc-server/src/server/surcharges.rs`, an `implement_connector_operation!` with `connector_data_types: [SurchargeConnectorData]`.

## Common Implementation Patterns

The whole category has one pattern — the **non-generic, no-macro** shape:

1. `pub struct Foo;` + `impl Foo { pub const fn new() -> &'static Self { &Self } }`.
2. `impl ConnectorCommon for Foo` — `id`, `get_currency_unit`, `base_url`, `get_auth_header`, `build_error_response`.
3. `common_macros::create_amount_converter_wrapper!(connector_name: Foo, amount_type: …);` — pick the unit that matches the vendor wire format.
4. Bare marker impls: `impl ValidationTrait for Foo {}`, `impl SurchargeServiceTrait for Foo {}`, `impl SurchargeCalculateV2 for Foo {}`, `impl SurchargePaymentSucceededV2 for Foo {}`, `impl SurchargeRefundSucceededV2 for Foo {}`.
5. A helper for URL building over the flow-generic router data:
   ```rust
   // From crates/integrations/connector-integration/src/surcharge_connectors/interpayments.rs
   impl InterPayments {
       pub fn connector_base_url_payments<'a, F, Req, Res>(
           &self,
           req: &'a RouterDataV2<F, SurchargeFlowData, Req, Res>,
       ) -> &'a str {
           &req.resource_common_data.connectors.interpayments.base_url
       }
   }
   ```
6. Three full `impl ConnectorIntegrationV2<Marker, SurchargeFlowData, Req, Resp> for Foo` blocks, each writing out `get_headers`, `get_content_type`, `get_url`, `get_request_body`, `build_request_v2`, `handle_response_v2`, `get_error_response_v2` by hand.

Because there is no macro, `build_request_v2` is your responsibility. The `interpayments.rs` body is the template — copy it verbatim and change only the HTTP method and URL:

```rust
// From crates/integrations/connector-integration/src/surcharge_connectors/interpayments.rs
let request_data = self.get_request_body(req)?;
let (body, typed_request_value) = match request_data {
    Some(data) => (
        Some(data.content),
        data.typed_request.map(|msv| msv.inner().clone()),
    ),
    None => (None, None),
};
Ok(Some(
    RequestBuilder::new()
        .method(Method::Post)
        .url(self.get_url(req)?.as_str())
        .attach_default_headers()
        .headers(self.get_headers(req)?)
        .set_optional_body(body)
        .set_typed_connector_request(typed_request_value)
        .build(),
))
```

## Code Examples

### Auth type (transformers)

```rust
// From crates/integrations/connector-integration/src/surcharge_connectors/interpayments/transformers.rs
pub struct InterpaymentsAuthType {
    pub api_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for InterpaymentsAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(item: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        if let ConnectorSpecificConfig::Interpayments { api_key, .. } = item {
            Ok(Self { api_key: api_key.to_owned() })
        } else {
            Err(IntegrationError::FailedToObtainAuthType {
                context: domain_types::errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to obtain InterPayments authentication credentials".to_string(),
                    ),
                    suggested_action: None,
                    doc_url: None,
                },
            })?
        }
    }
}
```

### The notify-flow response shape

```rust
// From crates/integrations/connector-integration/src/surcharge_connectors/interpayments.rs
//   ConnectorIntegrationV2<SurchargePaymentSucceeded, …>::handle_response_v2
let response_data = SurchargePaymentSucceededResponse {
    status_code: res.status_code,
};

let mut data = data.clone();
data.response = Ok(response_data);
data.resource_common_data
    .set_typed_connector_response(masked.as_ref().map(|m| m.inner().to_string()));
Ok(data)
```

### Marker impls

```rust
// From crates/integrations/connector-integration/src/surcharge_connectors/interpayments.rs
impl ValidationTrait for InterPayments {}
impl SurchargeServiceTrait for InterPayments {}
impl SurchargeCalculateV2 for InterPayments {}
impl SurchargePaymentSucceededV2 for InterPayments {}
impl SurchargeRefundSucceededV2 for InterPayments {}
```

## Integration Guidelines

1. Read `surcharge_connectors/interpayments.rs` in full before writing anything. It is the only exemplar and it is short enough to read end-to-end.
2. Create `surcharge_connectors/foo.rs` and `surcharge_connectors/foo/transformers.rs`.
3. In `transformers.rs`: request/response structs, `FooAuthType` with `impl TryFrom<&ConnectorSpecificConfig>`, `FooErrorResponse`, and the `TryFrom<&RouterDataV2<…>>` conversions.
4. In `foo.rs`: `pub struct Foo;`, `ConnectorCommon`, the amount-converter wrapper, the five bare marker impls, and the three `ConnectorIntegrationV2` blocks.
5. Walk the 12 registration sites above.
6. Add `foo.base_url` to all three `config/*.toml` files.
7. `cargo check -p connector-integration` then `cargo check --workspace`.
8. Exercise `SurchargeService/Calculate` with `x-surcharge-connector: foo`, then the two `EventService/NotifyConnector` event types.

## Best Practices

- **Keep the struct non-generic.** No surcharge connector carries a `T`; `BoxedSurchargeConnector` is a non-generic `dyn`, and `SurchargeConnectorData::convert_connector` calls a bare `Foo::new()`.
- **Return `Err(ErrorResponse { .. })` on an in-band 2xx failure** rather than a success with a zero surcharge. Branch on the mapped result, not on the presence of an `error` key (`PATTERN_AUTHORING_SPEC.md` §11.11).
- **Use `NO_ERROR_MESSAGE`** for a missing message, as `interpayments.rs` does: `response.message.clone().unwrap_or(NO_ERROR_MESSAGE.to_string())`.
- **Record the connector body on both notify flows** even though the response type is only `status_code` — `interpayments.rs` sets `event_builder.response_data` and `resource_common_data.set_typed_connector_response(..)` before discarding the parsed struct. Without this, a notify failure is invisible in logs.
- **Pick the amount unit from the vendor spec.** `interpayments` uses `FloatMajorUnit`; do not default to `StringMinorUnit` (`PATTERN_AUTHORING_SPEC.md` §11.10).
- **`connector_surcharge_id` is the join key** across all three flows: `SurchargeCalculateResponse.connector_surcharge_id` is what the caller later sends back as `SurchargePaymentSucceededRequest.connector_surcharge_id` / `SurchargeRefundSucceededRequest.connector_surcharge_id`, transported as `SurchargeContent.connector_surcharge_id`. Surface it verbatim.

## Common Errors / Gotchas

1. **Reaching for `create_all_prerequisites!` / `macro_connector_implementation!`.**
   - *Problem*: both macros require `generic_type: $generic_type:tt`, and the surcharge connector is a unit struct. The macro also *generates* the struct, so combining it with your own `pub struct Foo;` is a duplicate-definition error.
   - *Solution*: hand-write the three `ConnectorIntegrationV2` impls, following `interpayments.rs`.

2. **Emitting `macro_connector_flow_status_impls!`.**
   - *Problem*: GRACE's payment-connector checklist says it is mandatory. `expand_flow_status_impl!` has no surcharge arm, and `SurchargeServiceTrait` requires no payment flows, so the invocation is both unnecessary and unexpandable.
   - *Solution*: omit it. Verified: no file under `surcharge_connectors/` mentions it.

3. **Expecting three rpcs.**
   - *Problem*: you look for `SurchargeService/PaymentSucceeded` and `SurchargeService/RefundSucceeded` and cannot find them.
   - *Solution*: `service SurchargeService` has exactly one rpc, `Calculate`. The other two markers are driven by `EventService/NotifyConnector` with `NotifyEventType::SURCHARGE_PAYMENT_SUCCEEDED` / `SURCHARGE_REFUND_SUCCEEDED`.

4. **Implementing `IncomingWebhook` for the notify flows.**
   - *Problem*: "notify" sounds like a webhook. It is not: `NotifyConnector` is an **outgoing** call UCS makes *to* the connector. The proto comment on `enum NotifyEventType` says so: "Type of event for outgoing connector notifications (not webhooks)".
   - *Solution*: implement `SurchargePaymentSucceededV2` / `SurchargeRefundSucceededV2` as ordinary outbound flows. `IncomingWebhook` is not a `SurchargeServiceTrait` supertrait.

5. **Adding the connector to `ConnectorEnum` or the proto `enum Connector`.**
   - *Problem*: you assume every connector needs an entry there.
   - *Solution*: `interpayments` is in neither and routes fine, via `SurchargeConnectorEnum` + the `x-surcharge-connector` header, or via `ForeignTryFrom<AuthType> for ConnectorVariant` returning `ConnectorVariant::Surcharge(..)`.

6. **Forgetting `build_request_v2`.**
   - *Problem*: with the macro path, `build_request_v2` is generated. Hand-written, its default is not what you want.
   - *Solution*: write it in every one of the three impls. Copy the `RequestBuilder` chain from `interpayments.rs` verbatim.

## Testing & Certification Notes

- **Unit tests** in `surcharge_connectors/foo/transformers.rs`: `SurchargeCalculateRequest` → connector request shape (BIN, postal code masking, amount unit); connector response → `SurchargeCalculateResponse` including `surcharge_rate_percent`; `FooAuthType::try_from` on a *wrong* `ConnectorSpecificConfig` variant returning `FailedToObtainAuthType`.
- **No certification gate.** `.github/scripts/verify-new-connectors.sh` keys off a new directory under `crates/internal/integration-tests/src/connector_specs/`, and `check_connector_specs.rs` scans only `crates/integrations/connector-integration/src/connectors` — a surcharge connector creates no spec directory. Treat this as a coverage gap: record a manual sandbox transcript in the PR for all three flows.
- **Manual scenarios**: calculate with a known BIN; calculate with `previous_connector_surcharge_id` set (re-quote); notify payment-succeeded with the id returned by calculate; notify refund-succeeded; notify with an unknown id (expect the connector's error mapped through `build_error_response`).

## Cross-References

- Parent index: [./README.md](./README.md)
- Authoring spec: [./PATTERN_AUTHORING_SPEC.md](./PATTERN_AUTHORING_SPEC.md)
- Sibling category: [pattern_payout_connector.md](./pattern_payout_connector.md)
- Sibling category: [pattern_frm_connector.md](./pattern_frm_connector.md)
- Sibling category: [pattern_authenticator_connector.md](./pattern_authenticator_connector.md)
- Webhooks (contrast — incoming, not notify): [pattern_IncomingWebhook_flow.md](./pattern_IncomingWebhook_flow.md)
- Macros: [macro_patterns_reference.md](./macro_patterns_reference.md)
- Utility helpers: [../utility_functions_reference.md](../utility_functions_reference.md)
- Types: [../types/types.md](../types/types.md)
