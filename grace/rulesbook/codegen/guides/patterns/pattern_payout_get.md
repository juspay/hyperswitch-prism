# PayoutGet Flow Pattern

## Overview

The `PayoutGet` flow retrieves the current state of a previously-created payout from the connector. It is the payout-side analogue of `PSync` / `RSync`: no money moves, no body is typically sent, and the response carries only an updated `PayoutStatus` plus identifiers. The flow is driven by the `PayoutService::get` gRPC handler (`crates/grpc-server/grpc-server/src/server/payouts.rs:84-102`) and dispatched through `internal_payout_get` (`crates/grpc-server/grpc-server/src/server/payouts.rs:294-307`) under the `FlowName::PayoutGet` marker (`crates/types-traits/domain_types/src/connector_flow.rs:127`).

At the pinned SHA there are **no connectors with a non-default `ConnectorIntegrationV2<PayoutGet, ...>` implementation**. The `itaubank` connector opts into `PayoutGet` via the payout macro (`crates/integrations/connector-integration/src/connectors/itaubank.rs:53-66`) but the macro emits an empty impl body at `crates/integrations/connector-integration/src/connectors/macros.rs:1372-1387`, so no actual request is ever sent. This document describes the canonical shape any future implementation must follow and references `itaubank`'s `PayoutTransfer` impl (`crates/integrations/connector-integration/src/connectors/itaubank.rs:289-424`) as the nearest in-tree template because `PayoutTransfer` is the only fully implemented payout flow at this SHA.

### Key Components

- **Flow marker**: `domain_types::connector_flow::PayoutGet` — `crates/types-traits/domain_types/src/connector_flow.rs:80`.
- **Flow data**: `domain_types::payouts::payouts_types::PayoutFlowData` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:13`.
- **Request data**: `domain_types::payouts::payouts_types::PayoutGetRequest` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:122-125`.
- **Response data**: `domain_types::payouts::payouts_types::PayoutGetResponse` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:128-133`.
- **Marker trait**: `interfaces::connector_types::PayoutGetV2` — `crates/types-traits/interfaces/src/connector_types.rs:667-675`.
- **Integrity object**: `domain_types::payouts::router_request_types::PayoutGetIntegrityObject` — `crates/types-traits/domain_types/src/payouts/router_request_types.rs:43-46`.

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
    │   crates/grpc-server/grpc-server/src/server/payouts.rs:84
    ▼
internal_payout_get
    │   crates/grpc-server/grpc-server/src/server/payouts.rs:294-307
    ▼
ServerAuthenticationToken (if should_do_access_token == true)
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
// from crates/types-traits/domain_types/src/router_data_v2.rs:5-19
```

### Flow Type

`domain_types::connector_flow::PayoutGet` — unit marker struct at `crates/types-traits/domain_types/src/connector_flow.rs:79-80`. Threaded through `RouterDataV2.flow: PhantomData<PayoutGet>` (`crates/types-traits/domain_types/src/router_data_v2.rs:7`).

### Request Type

`domain_types::payouts::payouts_types::PayoutGetRequest` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:121-125`. Shape:

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs:121-125
#[derive(Debug, Clone)]
pub struct PayoutGetRequest {
    pub merchant_payout_id: Option<String>,
    pub connector_payout_id: Option<String>,
}
```

Unlike `PayoutCreateRequest` and `PayoutTransferRequest`, the `Get` variant carries only identifiers — no amount, no currency, no payout-method data. This matches the read-only semantics of the flow.

### Response Type

`domain_types::payouts::payouts_types::PayoutGetResponse` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:127-133`. Shape:

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs:127-133
#[derive(Debug, Clone)]
pub struct PayoutGetResponse {
    pub merchant_payout_id: Option<String>,
    pub payout_status: common_enums::PayoutStatus,
    pub connector_payout_id: Option<String>,
    pub status_code: u16,
}
```

`common_enums::PayoutStatus` is declared at `crates/common/common_enums/src/enums.rs:1134-1149`.

### Resource Common Data

`domain_types::payouts::payouts_types::PayoutFlowData` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:12-23`. Identical struct to the one used in `PayoutCreate` and `PayoutTransfer`. For `PayoutGet` specifically:

- The `payout_id` field is the server-side handle; it is populated from the gRPC request by `PayoutFlowData::foreign_try_from` (`crates/types-traits/domain_types/src/payouts/types.rs:27-29`).
- `access_token` gates authenticated reads via `PayoutFlowData::get_access_token` (`crates/types-traits/domain_types/src/payouts/payouts_types.rs:54-59`).
- `connector_request_reference_id` is derived from the incoming `merchant_payout_id` by `extract_connector_request_reference_id` (`crates/types-traits/domain_types/src/payouts/types.rs:31-33`).

### Integrity object

`PayoutGetIntegrityObject` at `crates/types-traits/domain_types/src/payouts/router_request_types.rs:43-46` uses identifier fields only (not amount/currency), reflecting the read-only nature of the flow.

## Connectors with Full Implementation

| Connector | HTTP Method | Content Type | URL Pattern | Request Type Reuse | Notes |
| --------- | ----------- | ------------ | ----------- | ------------------ | ----- |
| _(none at this SHA)_ | — | — | — | — | No connector provides a non-default `ConnectorIntegrationV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>` impl. |

### Current implementation coverage

A grep for `PayoutGet` across `crates/integrations/connector-integration/src/connectors/` at this SHA returns matches only from (a) the `itaubank` payout macro call at `crates/integrations/connector-integration/src/connectors/itaubank.rs:53-66`, and (b) the macro's `expand_payout_implementation!` arm at `crates/integrations/connector-integration/src/connectors/macros.rs:1372-1387`. The only connector with an active payout HTTP pipeline is `itaubank`, and that pipeline is realized on `PayoutTransfer` (`crates/integrations/connector-integration/src/connectors/itaubank.rs:289-424`), not `PayoutGet`. `itaubank`'s `PayoutGet` impl is the empty `{}` stub that the macro emits.

### Stub Implementations

- `itaubank` — opts in via `macro_connector_payout_implementation!` at `crates/integrations/connector-integration/src/connectors/itaubank.rs:53-66`; the macro expands the flow at `crates/integrations/connector-integration/src/connectors/macros.rs:1372-1387` into an empty impl body, so all methods use `ConnectorIntegrationV2` defaults.

## Common Implementation Patterns

### Pattern A — Manual `ConnectorIntegrationV2` over a GET endpoint

This is the expected track for any real `PayoutGet` implementation. It mirrors `PSync` on the payments side but with `PayoutFlowData` and no generic `T` parameter on the request:

1. Use `macros::create_all_prerequisites!` to wire the connector (`crates/integrations/connector-integration/src/connectors/itaubank.rs:45-51`).
2. Drop `PayoutGet` from the `payout_flows: [...]` list passed to `macros::macro_connector_payout_implementation!` (`crates/integrations/connector-integration/src/connectors/itaubank.rs:53-66`) so the macro does not emit an empty colliding impl.
3. Write `impl PayoutGetV2 for Connector<T> {}` and a manual `impl ConnectorIntegrationV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>` with `get_http_method` returning `Method::Get` and `get_request_body` returning `Ok(None)`.
4. Derive the path identifier from `req.request.connector_payout_id` (preferred) or `req.request.merchant_payout_id` (fallback) inside `get_url`.
5. Reuse the access-token header pattern from the itaubank `PayoutTransfer` impl (`crates/integrations/connector-integration/src/connectors/itaubank.rs:326-356`).
6. Parse the response into a connector-local struct and map its status via an `impl` method analogous to `ItaubankTransferResponse::status` at `crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:258-274`.

### Pattern B — Macro-only stub (current state)

Connectors that list `PayoutGet` in the payout-macro's `payout_flows: [...]` array inherit an empty impl from `crates/integrations/connector-integration/src/connectors/macros.rs:1372-1387`. All methods fall through to `ConnectorIntegrationV2` defaults; no real HTTP request is ever built. This is the current state for `itaubank`.

## Connector-Specific Patterns

### itaubank

- **Stub only at this SHA.** `PayoutGet` is included in the macro's flow list (`crates/integrations/connector-integration/src/connectors/itaubank.rs:53-66`) and therefore gets the empty impl at `crates/integrations/connector-integration/src/connectors/macros.rs:1372-1387`. The `itaubank` module does not contain any manual `get_url` / `handle_response_v2` for `PayoutGet`.
- **Access-token infrastructure is already in place.** If a full impl is added later it can call `req.resource_common_data.get_access_token()` directly (`crates/types-traits/domain_types/src/payouts/payouts_types.rs:54-59`) — the upstream `ServerAuthenticationToken` flow is implemented at `crates/integrations/connector-integration/src/connectors/itaubank.rs:161-286`.
- **Env-specific URL helper is available.** `build_env_specific_endpoint` (`crates/integrations/connector-integration/src/connectors/itaubank.rs:426-432`) can be reused verbatim for `PayoutGet` URL construction.

## Code Examples

### Example 1: Canonical `ConnectorIntegrationV2<PayoutGet, ...>` skeleton

Derived from the itaubank `PayoutTransfer` impl at `crates/integrations/connector-integration/src/connectors/itaubank.rs:294-424`; adapted for HTTP `GET` with no body.

```rust
// Shape derived from crates/integrations/connector-integration/src/connectors/itaubank.rs:294-424
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PayoutGet,
        PayoutFlowData,
        PayoutGetRequest,
        PayoutGetResponse,
    > for MyConnector<T>
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
    ) -> CustomResult<String, errors::IntegrationError> {
        let base_url = self.base_url(&req.resource_common_data.connectors);
        let payout_id = req
            .request
            .connector_payout_id
            .clone()
            .or_else(|| req.request.merchant_payout_id.clone())
            .ok_or(errors::IntegrationError::MissingRequiredField {
                field_name: "connector_payout_id",
                context: Default::default(),
            })?;
        Ok(format!("{base_url}/v1/payouts/{payout_id}"))
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<
            PayoutGet,
            PayoutFlowData,
            PayoutGetRequest,
            PayoutGetResponse,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
        // Mirrors crates/integrations/connector-integration/src/connectors/itaubank.rs:326-356
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
        _req: &RouterDataV2<
            PayoutGet,
            PayoutFlowData,
            PayoutGetRequest,
            PayoutGetResponse,
        >,
    ) -> CustomResult<Option<RequestContent>, errors::IntegrationError> {
        // GET flow: no body.
        Ok(None)
    }

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
        errors::ConnectorResponseTransformationError,
    > {
        // Parallel to crates/integrations/connector-integration/src/connectors/itaubank.rs:371-415
        let response: Result<MyConnectorGetResponse, _> =
            res.response.parse_struct("MyConnectorGetResponse");
        match response {
            Ok(get_res) => {
                event_builder.map(|i| i.set_connector_response(&get_res));
                Ok(RouterDataV2 {
                    response: Ok(PayoutGetResponse {
                        merchant_payout_id: data.request.merchant_payout_id.clone(),
                        payout_status: get_res.to_payout_status(),
                        connector_payout_id: Some(get_res.id),
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

### Example 2: Current macro-generated stub (what `itaubank` gets today)

```rust
// From crates/integrations/connector-integration/src/connectors/macros.rs:1372-1387
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
    {}
};
```

Because the impl body is `{}`, every method falls back to the `ConnectorIntegrationV2` trait defaults — there is no real URL, no real request, and no real response mapping on the current `itaubank` `PayoutGet`.

### Example 3: Status-mapper template (mirroring itaubank's real `PayoutTransfer` mapper)

```rust
// Shape derived from crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:258-274
impl MyConnectorGetResponse {
    pub fn to_payout_status(&self) -> common_enums::PayoutStatus {
        match self.status {
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
            // Rationale mirrors crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:271-272.
            _ => common_enums::PayoutStatus::Pending,
        }
    }
}
```

### Example 4: Connector-local response struct shape

```rust
// Pattern derived from crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:250-256
#[derive(Debug, Deserialize, Serialize)]
pub struct MyConnectorGetResponse {
    #[serde(alias = "id", alias = "payout_id")]
    pub id: String,
    #[serde(alias = "status", alias = "payout_status")]
    pub status: MyConnectorGetStatus,
}
```

Using `serde(alias = ...)` for dual field names is the idiom used by `ItaubankTransferResponse` and is applicable to any `PayoutGet` response struct.

## Integration Guidelines

1. **Declare prerequisites.** Invoke `macros::create_all_prerequisites!` as at `crates/integrations/connector-integration/src/connectors/itaubank.rs:45-51`.
2. **Remove `PayoutGet` from the payout macro's flow list.** Otherwise the manual impl below will collide at compile time with the stub at `crates/integrations/connector-integration/src/connectors/macros.rs:1372-1387`.
3. **Write `impl PayoutGetV2 for Connector<T> {}`.** Satisfies the marker trait at `crates/types-traits/interfaces/src/connector_types.rs:667-675`.
4. **Write the manual `ConnectorIntegrationV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>` impl.** Use Example 1 as skeleton. `get_http_method` MUST return `Method::Get`; `get_request_body` MUST return `Ok(None)` unless the connector requires a body on GET (unusual).
5. **Resolve the payout identifier in `get_url`.** Prefer `req.request.connector_payout_id`; fall back to `merchant_payout_id`. Return `IntegrationError::MissingRequiredField` when both are absent.
6. **Reuse access-token handling.** Mirror `crates/integrations/connector-integration/src/connectors/itaubank.rs:335-339`.
7. **Parse into a connector-local response struct with `#[serde(alias = ...)]` where needed.** Model on `ItaubankTransferResponse` (`crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:250-256`).
8. **Map status via an `impl` method, not inline literals.** Copy the shape of `ItaubankTransferResponse::status` (`crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:258-274`).
9. **Propagate `res.status_code` to `PayoutGetResponse.status_code`** (pattern at `crates/integrations/connector-integration/src/connectors/itaubank.rs:396`).
10. **Delegate error parsing to `build_error_response`** from `get_error_response_v2` (`crates/integrations/connector-integration/src/connectors/itaubank.rs:417-423`).

## Best Practices

- **Treat `PayoutGet` as idempotent and side-effect-free.** Never issue a `POST` / `PUT` / `DELETE` from this flow, even if the connector's URL layout would allow it. The gRPC contract at `crates/grpc-server/grpc-server/src/server/payouts.rs:84-102` is a read.
- **Default unknown connector statuses to `Pending`.** The same argument as for `PayoutTransfer` applies — see `crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:271-272`.
- **Derive the URL identifier from the typed request, never from `resource_common_data.payout_id`.** `PayoutGetRequest` is the authoritative source per `crates/types-traits/domain_types/src/payouts/payouts_types.rs:121-125`.
- **Mask the `Authorization` header with `into_masked()`** (`crates/integrations/connector-integration/src/connectors/itaubank.rs:349`).
- **Do not invent an `Option<RequestContent>` body for GET.** Return `Ok(None)` — the `ConnectorIntegrationV2` default body handling is correct for GET.
- **Link to `utility_functions_reference.md` for shared helpers** such as environment-specific URL builders; do not duplicate bodies inline (`PATTERN_AUTHORING_SPEC.md` §11).

## Common Errors / Gotchas

1. **Problem**: `get_url` fails because both `merchant_payout_id` and `connector_payout_id` are `None` on `PayoutGetRequest`.
   **Solution**: Return `IntegrationError::MissingRequiredField { field_name: "connector_payout_id", ... }` with a clear message; do not silently fall back to an empty string, which would produce a malformed URL.

2. **Problem**: Double impl of `ConnectorIntegrationV2<PayoutGet, ...>` because both the macro stub and a manual block are present.
   **Solution**: Drop `PayoutGet` from the `payout_flows: [...]` argument to `macros::macro_connector_payout_implementation!` (`crates/integrations/connector-integration/src/connectors/macros.rs:1266-1322`).

3. **Problem**: The connector responds with `200 OK` and an empty body on a successfully-completed payout poll; `parse_struct` fails.
   **Solution**: Either make each response-struct field `Option<_>` with `serde(default)`, or treat the empty-body case as `PayoutStatus::Pending` before calling `parse_struct`.

4. **Problem**: Silent `AttemptStatus::Failure` on transient HTTP 5xx from the connector during polling.
   **Solution**: Delegate to `build_error_response` from `get_error_response_v2` (`crates/integrations/connector-integration/src/connectors/itaubank.rs:417-423`) so the error surface is propagated as a real `ErrorResponse` rather than being collapsed into a status field.

5. **Problem**: Hardcoding `payout_status` in `handle_response_v2` because the response struct does not have a status field.
   **Solution**: If the connector truly exposes only HTTP status codes, map `res.status_code` in a named helper (e.g. `status_from_http(res.status_code)`) with comments explaining the mapping — never inline a literal `PayoutStatus::Success`. Refer to the status-mapping discipline in `pattern_capture.md`.

6. **Problem**: Invalidating the access token on every poll because `get_access_token` is called synchronously with no caching.
   **Solution**: The orchestrator is responsible for token reuse across flows; do not clear the token inside `PayoutGet`. `PayoutFlowData.access_token` is populated by the prior `ServerAuthenticationToken` exchange and should be read-only here.

## Testing Notes

### Unit-test shape

At the pinned SHA there are no in-repo unit tests for `PayoutGet` (the workspace contains only MIT-related markdown result files in `git status`). For a new implementation, recommended unit coverage:

- **URL construction** — one test each for `connector_payout_id` present, `merchant_payout_id` present, both absent (expect error), both present (expect `connector_payout_id` wins).
- **Status-mapper exhaustiveness** — one test per branch of the connector-status match, plus the unknown/default branch.
- **Response parsing** — one test per serde alias combination, plus empty-body handling.
- **Error-response parsing** — confirm that the connector's 4xx bodies round-trip through `build_error_response`.

### Integration-test scenarios

| Scenario | Setup | Expected outcome |
| -------- | ----- | ---------------- |
| Happy path — active payout | Valid sandbox access token; `connector_payout_id` set | `PayoutGetResponse` with `payout_status` mapped from response body |
| Unknown payout ID | Non-existent ID | `ErrorResponse` with connector 404 mapped through `build_error_response` |
| Missing identifiers | `merchant_payout_id = None`, `connector_payout_id = None` | `IntegrationError::MissingRequiredField` |
| Missing access token | `PayoutFlowData.access_token = None` | `IntegrationError::FailedToObtainAuthType` |
| Still-processing payout | Sandbox returns intermediate state | `payout_status = Pending` (never hardcoded) |
| Malformed JSON response | Mocked non-parsable body | `ConnectorResponseTransformationError::ResponseDeserializationFailed` |

Integration tests MUST describe real sandbox flows per `PATTERN_AUTHORING_SPEC.md` §11.

## Cross-References

- Parent index: [./README.md](./README.md)
- Authoring spec: [./PATTERN_AUTHORING_SPEC.md](./PATTERN_AUTHORING_SPEC.md)
- Sibling flow: [pattern_payout_create.md](./pattern_payout_create.md)
- Sibling flow: [pattern_payout_transfer.md](./pattern_payout_transfer.md)
- Utility helpers: [../utility_functions_reference.md](../utility_functions_reference.md)
