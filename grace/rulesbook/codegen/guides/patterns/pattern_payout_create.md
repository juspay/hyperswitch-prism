# PayoutCreate Flow Pattern

## Overview

The `PayoutCreate` flow is the entry point for initiating an outbound money movement (payout) through a payment processor. It is invoked by the `PayoutService::create` gRPC handler in `crates/grpc-server/grpc-server/src/server/payouts.rs:44-62` and dispatched through `internal_payout_create` at `crates/grpc-server/grpc-server/src/server/payouts.rs:264-277` under the `FlowName::PayoutCreate` marker (`crates/types-traits/domain_types/src/connector_flow.rs:125`). A connector implementing this flow typically creates a payout resource at the processor, reserving an identifier that can be retrieved later via `PayoutGet` or advanced via `PayoutTransfer`.

At the pinned SHA there are **no connectors with a non-default `ConnectorIntegrationV2<PayoutCreate, ...>` implementation**. The `itaubank` connector opts into the `PayoutCreate` trait through the macro framework (`crates/integrations/connector-integration/src/connectors/itaubank.rs:53-66`) but the macro emits an empty impl block (see `crates/integrations/connector-integration/src/connectors/macros.rs:1339-1355`), which means the trait is present for compile-time wiring but does not carry request/response transformation logic. `itaubank`'s real payout behaviour lives on `PayoutTransfer`. This document therefore describes the canonical shape connectors are expected to follow when a full `PayoutCreate` implementation is added, and cross-references the only related full implementation currently in the tree (`PayoutTransfer` on itaubank).

### Key Components

- **Flow marker**: `domain_types::connector_flow::PayoutCreate` — `crates/types-traits/domain_types/src/connector_flow.rs:74`.
- **Flow data**: `domain_types::payouts::payouts_types::PayoutFlowData` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:13`.
- **Request data**: `domain_types::payouts::payouts_types::PayoutCreateRequest` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:77`.
- **Response data**: `domain_types::payouts::payouts_types::PayoutCreateResponse` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:92`.
- **Marker trait**: `interfaces::connector_types::PayoutCreateV2` — `crates/types-traits/interfaces/src/connector_types.rs:364-372`.
- **Macro entry point**: `macros::macro_connector_payout_implementation!` — `crates/integrations/connector-integration/src/connectors/macros.rs:1266-1322`.

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
    │   crates/grpc-server/grpc-server/src/server/payouts.rs:44
    ▼
internal_payout_create
    │   crates/grpc-server/grpc-server/src/server/payouts.rs:264-277
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
// from crates/types-traits/domain_types/src/router_data_v2.rs:5-19
```

### Flow Type

`domain_types::connector_flow::PayoutCreate` — unit marker struct declared at `crates/types-traits/domain_types/src/connector_flow.rs:73-74`. It is carried through `RouterDataV2` in a `PhantomData<PayoutCreate>` field (`crates/types-traits/domain_types/src/router_data_v2.rs:7`).

### Request Type

`domain_types::payouts::payouts_types::PayoutCreateRequest` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:77-89`. Shape:

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs:76-89
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
}
```

### Response Type

`domain_types::payouts::payouts_types::PayoutCreateResponse` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:91-97`. Shape:

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs:91-97
#[derive(Debug, Clone)]
pub struct PayoutCreateResponse {
    pub merchant_payout_id: Option<String>,
    pub payout_status: common_enums::PayoutStatus,
    pub connector_payout_id: Option<String>,
    pub status_code: u16,
}
```

`common_enums::PayoutStatus` is the target status enum (`crates/common/common_enums/src/enums.rs:1134-1149`).

### Resource Common Data

`domain_types::payouts::payouts_types::PayoutFlowData` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:12-23`. This is the canonical flow-data type for all payout flows and replaces the payment-side `PaymentFlowData` for the `PayoutCreate` / `PayoutTransfer` / `PayoutGet` trio. Unlike `PaymentFlowData`, it does not carry an `AttemptStatus`; status is carried inside the typed response (`PayoutCreateResponse.payout_status`). Shape:

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs:12-23
#[derive(Debug, Clone)]
pub struct PayoutFlowData {
    pub merchant_id: common_utils::id_type::MerchantId,
    pub payout_id: String,
    pub connectors: Connectors,
    pub connector_request_reference_id: String,
    pub raw_connector_response: Option<Secret<String>>,
    pub connector_response_headers: Option<http::HeaderMap>,
    pub raw_connector_request: Option<Secret<String>>,
    pub access_token: Option<ServerAuthenticationTokenResponseData>,
    pub test_mode: Option<bool>,
}
```

Note the `access_token: Option<ServerAuthenticationTokenResponseData>` field — payout connectors commonly require a prior `ServerAuthenticationToken` exchange, and the helper `PayoutFlowData::get_access_token` (`crates/types-traits/domain_types/src/payouts/payouts_types.rs:54-59`) surfaces it for use in `get_headers`.

## Connectors with Full Implementation

| Connector | HTTP Method | Content Type | URL Pattern | Request Type Reuse | Notes |
| --------- | ----------- | ------------ | ----------- | ------------------ | ----- |
| _(none at this SHA)_ | — | — | — | — | No connector provides a non-default `ConnectorIntegrationV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>` impl. |

### Current implementation coverage

A grep for `PayoutCreate` across `crates/integrations/connector-integration/src/connectors/` at this SHA returns matches only from (a) the `itaubank` payout macro call at `crates/integrations/connector-integration/src/connectors/itaubank.rs:53-66`, (b) the `macros.rs` expansion itself at `crates/integrations/connector-integration/src/connectors/macros.rs:1339-1355`, and (c) a webhook-event symbol in `revolut.rs:358` that is unrelated to the outbound flow. The only connector with an active payout HTTP pipeline is `itaubank`, and that pipeline is realized on `PayoutTransfer` (`crates/integrations/connector-integration/src/connectors/itaubank.rs:289-424`), not `PayoutCreate`.

### Stub Implementations

- `itaubank` — via `macro_connector_payout_implementation!` at `crates/integrations/connector-integration/src/connectors/itaubank.rs:53-66`; expands to the empty `{}` impl body at `crates/integrations/connector-integration/src/connectors/macros.rs:1339-1355`.

## Common Implementation Patterns

The expected shape for a full `PayoutCreate` implementation mirrors the real `PayoutTransfer` pipeline on itaubank. Two pattern tracks are available:

### Pattern A — Macro-generated stub + manual `ConnectorIntegrationV2`

This is the track used by `itaubank` for its actual `PayoutTransfer` impl:

1. Use `macros::create_all_prerequisites!` to wire the connector struct (`crates/integrations/connector-integration/src/connectors/itaubank.rs:45-51`).
2. Call `macros::macro_connector_payout_implementation!` with the list of flows that should get default empty impls (`crates/integrations/connector-integration/src/connectors/itaubank.rs:53-66`).
3. For the flow you actually implement, write a manual `impl ConnectorIntegrationV2<Flow, PayoutFlowData, FlowRequest, FlowResponse> for Connector<T>` block that overrides `get_http_method`, `get_content_type`, `get_url`, `get_headers`, `get_request_body`, `handle_response_v2`, and `get_error_response_v2`. The itaubank `PayoutTransfer` impl at `crates/integrations/connector-integration/src/connectors/itaubank.rs:294-424` is the reference.
4. Drop the implemented flow from the macro's `payout_flows: [...]` list so the manual impl does not collide with the macro-generated empty impl.

### Pattern B — All flows manual

If no default stubs are needed, omit `macro_connector_payout_implementation!` entirely and write each `impl ConnectorIntegrationV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>` by hand. This is the path indicated by the authoring spec for connectors that cover only one or two payout flows; there is no connector demonstrating it in-tree at this SHA for `PayoutCreate`.

## Connector-Specific Patterns

### itaubank

- **Current coverage**: opts into `PayoutCreate` stub only; real logic is on `PayoutTransfer` (`crates/integrations/connector-integration/src/connectors/itaubank.rs:289-424`).
- **Access-token dependency**: `ValidationTrait::should_do_access_token` returns `true` (`crates/integrations/connector-integration/src/connectors/itaubank.rs:141-147`), so any future `PayoutCreate` impl would run the `ServerAuthenticationToken` flow first and read the token via `PayoutFlowData::get_access_token` (`crates/types-traits/domain_types/src/payouts/payouts_types.rs:54-59`).
- **Env-specific URL**: `build_env_specific_endpoint` at `crates/integrations/connector-integration/src/connectors/itaubank.rs:426-432` switches path suffix by `resource_common_data.test_mode`; a `PayoutCreate` URL builder would reuse the same helper.

## Code Examples

### Example 1: Canonical `ConnectorIntegrationV2<PayoutCreate, ...>` skeleton

Derived from the live itaubank `PayoutTransfer` impl at `crates/integrations/connector-integration/src/connectors/itaubank.rs:294-424`; swap the flow marker and request/response types.

```rust
// Shape derived from crates/integrations/connector-integration/src/connectors/itaubank.rs:294-424
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PayoutCreate,
        PayoutFlowData,
        PayoutCreateRequest,
        PayoutCreateResponse,
    > for MyConnector<T>
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
    }

    fn get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn get_url(
        &self,
        req: &RouterDataV2<
            PayoutCreate,
            PayoutFlowData,
            PayoutCreateRequest,
            PayoutCreateResponse,
        >,
    ) -> CustomResult<String, errors::IntegrationError> {
        let base_url = self.base_url(&req.resource_common_data.connectors);
        Ok(format!("{base_url}/v1/payouts"))
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<
            PayoutCreate,
            PayoutFlowData,
            PayoutCreateRequest,
            PayoutCreateResponse,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
        // Mirrors itaubank.rs:326-356: read access token from PayoutFlowData
        let access_token = req.resource_common_data.get_access_token().map_err(|_| {
            errors::IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
        })?;
        Ok(vec![
            (headers::CONTENT_TYPE.to_string(), "application/json".to_string().into()),
            (headers::AUTHORIZATION.to_string(), format!("Bearer {access_token}").into_masked()),
        ])
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<
            PayoutCreate,
            PayoutFlowData,
            PayoutCreateRequest,
            PayoutCreateResponse,
        >,
    ) -> CustomResult<Option<RequestContent>, errors::IntegrationError> {
        let connector_req = MyConnectorCreateRequest::try_from(req)?;
        Ok(Some(RequestContent::Json(Box::new(connector_req))))
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
        errors::ConnectorResponseTransformationError,
    > {
        // Parallel to itaubank.rs:371-415
        let response: Result<MyConnectorCreateResponse, _> =
            res.response.parse_struct("MyConnectorCreateResponse");
        match response {
            Ok(create_res) => {
                event_builder.map(|i| i.set_connector_response(&create_res));
                Ok(RouterDataV2 {
                    response: Ok(PayoutCreateResponse {
                        merchant_payout_id: None,
                        payout_status: create_res.to_payout_status(),
                        connector_payout_id: Some(create_res.id),
                        status_code: res.status_code,
                    }),
                    ..data.clone()
                })
            }
            Err(_) => Err(
                errors::ConnectorResponseTransformationError::ResponseDeserializationFailed {
                    context: Default::default(),
                }
                .into(),
            ),
        }
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, errors::ConnectorResponseTransformationError> {
        self.build_error_response(res, event_builder)
    }
}
```

### Example 2: How the macro emits the current `PayoutCreate` stub on itaubank

```rust
// From crates/integrations/connector-integration/src/connectors/macros.rs:1339-1355
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
        {}
    };
```

This empty impl satisfies the trait bound required by `ConnectorServiceTrait`-style wiring but all methods fall through to `ConnectorIntegrationV2` defaults (which return `NotImplemented`-style errors). Any connector listed only in the macro's `payout_flows: [...]` array is in this state.

### Example 3: Request-struct `TryFrom` shape (mirroring itaubank `PayoutTransfer`)

The itaubank request transformer at `crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:143-223` is the closest in-tree reference. For `PayoutCreate`, the structure is identical modulo flow marker and target type:

```rust
// Shape derived from crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:143-223
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

The itaubank transformer maps a connector-local enum into `common_enums::PayoutStatus` rather than hardcoding it — the same pattern applies to `PayoutCreate`:

```rust
// From crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:258-274
impl ItaubankTransferResponse {
    pub fn status(&self) -> common_enums::PayoutStatus {
        match self.transfer_status {
            Some(ItaubankPayoutStatus::Aprovado)
            | Some(ItaubankPayoutStatus::Confirmado)
            | Some(ItaubankPayoutStatus::Efetivado)
            | Some(ItaubankPayoutStatus::Sucesso) => common_enums::PayoutStatus::Success,
            Some(ItaubankPayoutStatus::Pendente) | Some(ItaubankPayoutStatus::EmProcessamento) => {
                common_enums::PayoutStatus::Pending
            }
            Some(ItaubankPayoutStatus::Rejeitado)
            | Some(ItaubankPayoutStatus::Cancelado)
            | Some(ItaubankPayoutStatus::NaoIncluido) => common_enums::PayoutStatus::Failure,
            Some(ItaubankPayoutStatus::Unknown) | None => common_enums::PayoutStatus::Pending,
        }
    }
}
```

## Integration Guidelines

1. **Declare payout support in `create_all_prerequisites!`.** Follow `crates/integrations/connector-integration/src/connectors/itaubank.rs:45-51`. For connectors not reusing the payments macro API list, pass `api: []` and keep the payout implementation in a dedicated `impl` block.
2. **Decide whether to use the payout macro.** Call `macros::macro_connector_payout_implementation!` (`crates/integrations/connector-integration/src/connectors/macros.rs:1266-1322`) with `payout_flows: [...]` listing only flows you want as empty stubs; for `PayoutCreate`, omit it from that list and write the impl manually (see Example 1).
3. **Implement `PayoutCreateV2`.** Provide `impl PayoutCreateV2 for Connector<T> {}` so the marker trait from `crates/types-traits/interfaces/src/connector_types.rs:364-372` is satisfied. If the macro is generating this for `PayoutCreate`, remove `PayoutCreate` from its list before adding the manual impl.
4. **Write the `ConnectorIntegrationV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>` impl.** Override all six methods from Example 1. Mirror the `PayoutTransfer` method ordering used at `crates/integrations/connector-integration/src/connectors/itaubank.rs:302-423`.
5. **Add request/response transformers.** Create `ConnectorNameCreateRequest` (Serialize) and `ConnectorNameCreateResponse` (Deserialize) with `TryFrom` impls against the canonical `RouterDataV2` signature, as in `crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:143-223` (request) and `:251-256` (response).
6. **Map `payout_status` from the connector payload.** Never hardcode `common_enums::PayoutStatus`; use an enum-to-enum mapping function as in `crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:258-274`.
7. **Propagate `res.status_code` into `PayoutCreateResponse.status_code`.** Follow `crates/integrations/connector-integration/src/connectors/itaubank.rs:396`.
8. **Reuse `ConnectorCommon::build_error_response`.** Return it from `get_error_response_v2`, as at `crates/integrations/connector-integration/src/connectors/itaubank.rs:417-423`.
9. **Register the flow in `FlowName`.** Already done at `crates/types-traits/domain_types/src/connector_flow.rs:125`; no author action needed, but verify the name is plumbed through the gRPC layer at `crates/grpc-server/grpc-server/src/server/payouts.rs:264-277`.

## Best Practices

- **Treat access-token handling as a precondition, not a field copy.** Use `PayoutFlowData::get_access_token` (`crates/types-traits/domain_types/src/payouts/payouts_types.rs:54-59`) and fail with `IntegrationError::FailedToObtainAuthType` if the token is missing — the itaubank `PayoutTransfer` impl does this at `crates/integrations/connector-integration/src/connectors/itaubank.rs:335-339`.
- **Keep the request body construction in `TryFrom`, not in `get_request_body`.** `get_request_body` should only wrap the output in `RequestContent::Json` and propagate errors, mirroring `crates/integrations/connector-integration/src/connectors/itaubank.rs:358-369`.
- **Use `StringMajorUnitForConnector` (or the correct converter) for amount conversion, then bubble errors with `change_context`.** See `crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:163-168`.
- **Fall back to `common_enums::PayoutStatus::Pending` on unknown connector statuses rather than `Failure`.** This matches the itaubank behaviour at `crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:271-272` and prevents false negatives on async processing.
- **Do not hand-roll amount math or error types.** Use `common_utils::types` amount converters and `IntegrationError` / `ConnectorResponseTransformationError` exclusively (see `PATTERN_AUTHORING_SPEC.md` §12 retired-types list).
- **Cross-reference `utility_functions_reference.md` for shared helpers such as `build_env_specific_endpoint`-style URL builders.**

## Common Errors / Gotchas

1. **Problem**: Macro-generated empty impl masks a missing real implementation, so the connector appears to "support" `PayoutCreate` but all calls return a default error.
   **Solution**: Remove `PayoutCreate` from the `payout_flows: [...]` list of `macro_connector_payout_implementation!` (`crates/integrations/connector-integration/src/connectors/macros.rs:1266-1322`) as soon as you add a real impl; otherwise the two impls collide at compile time, but if you never add a real one the connector silently answers with defaults.

2. **Problem**: Confusing `PayoutCreateRequest` with `PaymentsAuthorizeData<T>` — they are different types on different traits.
   **Solution**: Always use the four-argument form `RouterDataV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>`. The request type is defined at `crates/types-traits/domain_types/src/payouts/payouts_types.rs:77-89` and has no generic parameter, unlike `PaymentsAuthorizeData<T>`.

3. **Problem**: Missing access token at request time because the connector never ran the `ServerAuthenticationToken` flow first.
   **Solution**: Ensure `ValidationTrait::should_do_access_token` returns `true` for payouts (`crates/integrations/connector-integration/src/connectors/itaubank.rs:141-147`); the orchestrator uses this to sequence the token exchange before `PayoutCreate`.

4. **Problem**: Serializing `common_utils::types::MinorUnit` directly to a connector that expects a string-formatted major unit.
   **Solution**: Convert via `StringMajorUnitForConnector` (or the appropriate converter) and store the converted value in the connector-local request struct, as at `crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:163-168`.

5. **Problem**: Hardcoding `payout_status: PayoutStatus::Success` in `handle_response_v2`.
   **Solution**: Map from a typed response-status enum — see the `ItaubankTransferResponse::status` function at `crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:258-274` and return the mapped value in the `PayoutCreateResponse`.

## Testing Notes

### Unit-test shape

At the pinned SHA there are no in-repo unit tests that target `PayoutCreate` directly (only the `PayoutTransfer` integration test artefacts exist in workspace-level markdown files listed in `git status`). For a new `PayoutCreate` implementation, the minimum unit-test surface is:

- Request-struct `TryFrom` coverage: one test per supported `PayoutMethodData` variant (`crates/types-traits/domain_types/src/payouts/payout_method_data.rs:6-13`).
- Status mapping coverage: one test per branch of the connector-status → `common_enums::PayoutStatus` match.
- Amount-conversion coverage: verify `StringMajorUnit` (or other) formatting for each supported currency.

### Integration-test scenarios

| Scenario | Setup | Expected `payout_status` |
| -------- | ----- | ------------------------ |
| Happy path — full create with valid payout method | Real access token; valid `payout_method_data` | `Success` or `Pending` (never hardcoded) |
| Missing access token | `PayoutFlowData.access_token = None` | `IntegrationError::FailedToObtainAuthType` |
| Invalid payout method mapping | `payout_method_data` variant the connector does not support | `IntegrationError::InvalidDataFormat` |
| Connector rejection | Mocked 4xx response | `ErrorResponse` surfaced via `build_error_response` |
| Malformed JSON response | Mocked non-parsable 2xx body | `ConnectorResponseTransformationError::ResponseDeserializationFailed` |

Integration tests MUST describe real sandbox flows (see `PATTERN_AUTHORING_SPEC.md` §11); do not mock the HTTP layer for documented tests.

## Cross-References

- Parent index: [./README.md](./README.md)
- Authoring spec: [./PATTERN_AUTHORING_SPEC.md](./PATTERN_AUTHORING_SPEC.md)
- Sibling flow: [pattern_payout_transfer.md](./pattern_payout_transfer.md)
- Sibling flow: [pattern_payout_get.md](./pattern_payout_get.md)
- Utility helpers: [../utility_functions_reference.md](../utility_functions_reference.md)
