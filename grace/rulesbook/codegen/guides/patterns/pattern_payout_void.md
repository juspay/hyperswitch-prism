# Payout Void Flow Pattern

## Overview

The Payout Void flow cancels an in-flight or scheduled payout before the connector has finalized disbursement. It is the payout analogue of the Payments Void flow and must be invoked on a previously created payout reference (obtained from `PayoutCreate` or `PayoutTransfer`). The flow posts a cancellation instruction to the connector, then maps the returned status back to `common_enums::PayoutStatus` so the router can observe the cancelled or still-pending state. Because not every connector supports cancellation at every lifecycle state, the flow MUST surface a connector error when cancellation is rejected rather than fabricating a success.

### Key Components

- Flow marker: `PayoutVoid` — `crates/types-traits/domain_types/src/connector_flow.rs:83`.
- Request type: `PayoutVoidRequest` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:152`.
- Response type: `PayoutVoidResponse` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:158`.
- Flow-data type: `PayoutFlowData` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:13`.
- Marker trait: `PayoutVoidV2` — `crates/types-traits/interfaces/src/connector_types.rs:677`.
- Macro arm: `expand_payout_implementation!` `PayoutVoid` arm — `crates/integrations/connector-integration/src/connectors/macros.rs:1388-1403`.

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

`PayoutVoid` — zero-sized marker struct declared at `crates/types-traits/domain_types/src/connector_flow.rs:83`. It is also registered in `FlowName::PayoutVoid` at `crates/types-traits/domain_types/src/connector_flow.rs:128` for telemetry.

### Request Type

`PayoutVoidRequest` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:152-156`:

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs:151
#[derive(Debug, Clone)]
pub struct PayoutVoidRequest {
    pub merchant_payout_id: Option<String>,
    pub connector_payout_id: Option<String>,
}
```

Both fields are `Option<String>` at the pinned SHA; a connector that requires the connector-side identifier to construct the cancellation URL MUST validate it is `Some` inside `get_url`/`get_request_body` and emit `IntegrationError::MissingRequiredField` otherwise.

### Response Type

`PayoutVoidResponse` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:158-163`:

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs:157
#[derive(Debug, Clone)]
pub struct PayoutVoidResponse {
    pub merchant_payout_id: Option<String>,
    pub payout_status: common_enums::PayoutStatus,
    pub connector_payout_id: Option<String>,
    pub status_code: u16,
}
```

`payout_status` is a `common_enums::PayoutStatus` — `crates/common/common_enums/src/enums.rs:1134-1149`. After a successful void, the expected terminal mapping is `PayoutStatus::Cancelled`. If the connector instead returns a pending-cancellation intermediate state, the transformer MUST map to `PayoutStatus::Pending` (never `Cancelled`) so the router keeps polling.

### Resource Common Data

`PayoutFlowData` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:13-23`:

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs:12
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

Access-token-gated connectors call `PayoutFlowData::get_access_token` at `crates/types-traits/domain_types/src/payouts/payouts_types.rs:54-59` from inside `get_headers` to attach a `Bearer` token.

### RouterDataV2 Shape

```rust
RouterDataV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>
```

This is the canonical four-type-argument envelope per §7 of `PATTERN_AUTHORING_SPEC.md`. Three-argument forms or V1 `RouterData` MUST NOT appear.

## Connectors with Full Implementation

At the pinned SHA `ceb33736ce941775403f241f3f0031acbf2b4527`, **no connector supplies a non-stub `ConnectorIntegrationV2<PayoutVoid, ...>` implementation.** A file-scoped grep across `crates/integrations/connector-integration/src/connectors/` for `ConnectorIntegrationV2<\s*PayoutVoid` returns zero matches. The only connector that registers the `PayoutVoidV2` marker trait is **itaubank**, and it does so purely through the macro-generated default body (see `crates/integrations/connector-integration/src/connectors/itaubank.rs:53-66` and `crates/integrations/connector-integration/src/connectors/macros.rs:1388-1403`). The macro-generated default body inherits the trait defaults and therefore returns `IntegrationError::NotImplemented` at runtime when the flow is invoked.

Current implementation coverage: **0 connectors** (itaubank registers the marker trait only; no URL, headers, body, or response-parsing are provided).

| Connector | HTTP Method | Content Type | URL Pattern | Request Type Reuse | Notes |
| --- | --- | --- | --- | --- | --- |
| _(none)_ | — | — | — | — | See Stub Implementations below. |

### Stub Implementations

- **itaubank** — macro-registered stub only. The `PayoutVoid` arm of `expand_payout_implementation!` (`crates/integrations/connector-integration/src/connectors/macros.rs:1388-1403`) emits both `impl PayoutVoidV2 for Itaubank<T> {}` and `impl ConnectorIntegrationV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse> for Itaubank<T> {}` with empty bodies. All methods (`get_url`, `get_headers`, `get_request_body`, `handle_response_v2`) fall through to the `ConnectorIntegrationV2` trait defaults defined in `crates/types-traits/interfaces/src/connector_integration_v2.rs`. Invoking the flow against itaubank therefore returns a `NotImplemented` error.

Because there is no full implementation, the "Connector-Specific Patterns" section below is intentionally sparse — it documents only the itaubank registration mechanics.

## Common Implementation Patterns

### Macro-Based Pattern (Recommended)

Connectors that want to expose `PayoutVoid` MUST first opt into the `PayoutVoidV2` marker via the payout macro, then write an explicit `ConnectorIntegrationV2` body that overrides the default trait methods. The macro-registration is achieved by including `PayoutVoid` in the `payout_flows:` list:

```rust
// From crates/integrations/connector-integration/src/connectors/itaubank.rs:53
macros::macro_connector_payout_implementation!(
    connector: Itaubank,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    payout_flows: [
        PayoutCreate,
        PayoutGet,
        PayoutVoid,       // <-- registers PayoutVoidV2 + empty ConnectorIntegrationV2 impl
        PayoutStage,
        PayoutCreateLink,
        PayoutCreateRecipient,
        PayoutEnrollDisburseAccount
    ]
);
```

The recursion in `macro_connector_payout_implementation!` at `crates/integrations/connector-integration/src/connectors/macros.rs:1266-1319` peels each flow name off the list and forwards to the corresponding `expand_payout_implementation!` arm. The `PayoutVoid` arm at `macros.rs:1388-1403` generates:

```rust
// Expansion emitted by the PayoutVoid arm of expand_payout_implementation!
impl<T: ...> ::interfaces::connector_types::PayoutVoidV2 for Itaubank<T> {}
impl<T: ...>
    ::interfaces::connector_integration_v2::ConnectorIntegrationV2<
        ::domain_types::connector_flow::PayoutVoid,
        ::domain_types::payouts::payouts_types::PayoutFlowData,
        ::domain_types::payouts::payouts_types::PayoutVoidRequest,
        ::domain_types::payouts::payouts_types::PayoutVoidResponse,
    > for Itaubank<T>
{}
```

To move from stub to full, the connector author adds a second `impl ConnectorIntegrationV2<PayoutVoid, ...> for <Connector><T>` block in `<connector>.rs` that overrides `get_http_method`, `get_url`, `get_headers`, `get_request_body`, `handle_response_v2`, and `get_error_response_v2`. This is the same shape used for the full `PayoutTransfer` implementation on itaubank at `crates/integrations/connector-integration/src/connectors/itaubank.rs:294-424`.

### Manual Pattern (Override Alternative)

When a connector does not want the macro default (e.g. needs custom trait bounds), omit the flow from the `payout_flows:` list and write both the marker impl and the integration impl by hand. The manual shape mirrors itaubank's `PayoutTransfer` block at `itaubank.rs:289-424`: a `PayoutVoidV2` impl followed by a `ConnectorIntegrationV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>` impl with all six override methods.

### Request-Body Strategies Observed for Payout-Flow Cancellations

The three strategies typically seen in payout APIs at connector-API level:

1. **Path-only cancel** — cancellation endpoint embeds the connector payout id and the body is empty (`{}` or no body). URL shape `POST {base_url}/payouts/{connector_payout_id}/cancel`.
2. **Id-in-body cancel** — cancellation endpoint is static and the body carries the id. URL shape `POST {base_url}/payouts/cancel` with `{ "id": "<connector_payout_id>" }`.
3. **PATCH-status cancel** — connector exposes a generic status-update endpoint. URL shape `PATCH {base_url}/payouts/{connector_payout_id}` with `{ "status": "cancelled" }`.

No current connector-service connector implements any of these shapes for `PayoutVoid` at the pinned SHA. The list above is a design note, not a citation of in-repo behavior.

## Connector-Specific Patterns

### itaubank

- `itaubank` includes `PayoutVoid` in its `payout_flows:` list (`crates/integrations/connector-integration/src/connectors/itaubank.rs:60`) but provides no custom `ConnectorIntegrationV2<PayoutVoid, ...>` impl. The connector's Itau SiSPAG integration document does not expose a cancellation endpoint, and the transformer file `crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs` contains no `PayoutVoidRequest` / `PayoutVoidResponse` `TryFrom` blocks at any line. The flow is therefore registered-but-inert.

No other connector in `crates/integrations/connector-integration/src/connectors/` registers `PayoutVoidV2`.

## Code Examples

### 1. Macro registration (itaubank)

```rust
// From crates/integrations/connector-integration/src/connectors/itaubank.rs:53-66
macros::macro_connector_payout_implementation!(
    connector: Itaubank,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    payout_flows: [
        PayoutCreate,
        PayoutGet,
        PayoutVoid,
        PayoutStage,
        PayoutCreateLink,
        PayoutCreateRecipient,
        PayoutEnrollDisburseAccount
    ]
);
```

### 2. The macro arm that expands `PayoutVoid`

```rust
// From crates/integrations/connector-integration/src/connectors/macros.rs:1388
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
        > for $connector<$connector>
    {}
};
```

### 3. Marker trait definition

```rust
// From crates/types-traits/interfaces/src/connector_types.rs:677
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

### 4. Reference implementation shape (from `PayoutTransfer` on itaubank)

```rust
// Adapted shape — see crates/integrations/connector-integration/src/connectors/itaubank.rs:294-424
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PayoutVoid,
        PayoutFlowData,
        PayoutVoidRequest,
        PayoutVoidResponse,
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
        req: &RouterDataV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>,
    ) -> CustomResult<String, IntegrationError> {
        let base_url = self.base_url(&req.resource_common_data.connectors);
        let connector_payout_id = req
            .request
            .connector_payout_id
            .as_ref()
            .ok_or_else(|| IntegrationError::MissingRequiredField {
                field_name: "connector_payout_id",
                context: Default::default(),
            })?;
        Ok(format!("{base_url}/v1/payouts/{connector_payout_id}/cancel"))
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let access_token = req.resource_common_data.get_access_token().map_err(|_| {
            IntegrationError::FailedToObtainAuthType { context: Default::default() }
        })?;
        Ok(vec![
            (headers::CONTENT_TYPE.to_string(), "application/json".to_string().into()),
            (headers::AUTHORIZATION.to_string(), format!("Bearer {access_token}").into_masked()),
        ])
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>,
        ConnectorResponseTransformationError,
    > {
        // Parse connector response and map to PayoutVoidResponse with
        // connector-mapped PayoutStatus (NEVER hardcoded).
        todo!("connector-specific response parsing")
    }
}
```

Status mapping MUST be derived from the connector response. Hardcoding `payout_status: PayoutStatus::Cancelled` is banned by §11 of `PATTERN_AUTHORING_SPEC.md` item 1.

## Integration Guidelines

Follow this ordered sequence when wiring a new connector's `PayoutVoid`:

1. Confirm the connector's cancel/refund-of-payout endpoint in its API docs and record the HTTP method, URL template, auth requirements, and whether a body is expected.
2. Add `PayoutVoid` to the `payout_flows:` list passed to `macros::macro_connector_payout_implementation!` in `<connector>.rs`. If the connector already lists `PayoutCreate`/`PayoutTransfer`/`PayoutGet`, append `PayoutVoid` — see the reference list at `crates/integrations/connector-integration/src/connectors/itaubank.rs:57-65`.
3. Write a dedicated `impl ConnectorIntegrationV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse> for <Connector><T>` block alongside the `PayoutTransfer` impl. This second impl shadows the macro-generated empty impl because Rust's specialization rules allow a concrete impl when the macro one has the same signature — so you MUST remove `PayoutVoid` from the `payout_flows:` list to avoid a duplicate-impl compile error, OR opt into the manual pattern entirely. The macros.rs documentation at `macros.rs:1326-1338` flags that the default body falls back to `NotImplemented`; authors override it by supplying a concrete impl **in place of** the macro registration.
4. In `<connector>/transformers.rs`, add a `TryFrom<&RouterDataV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>>` impl that produces the connector's cancel-request struct. If the API expects an empty body, return `Ok(None)` from `get_request_body` instead of serializing an empty struct.
5. Add a `TryFrom<ResponseRouterData<<ConnectorCancelResponse>, Self>>` that maps the connector status enum to `common_enums::PayoutStatus` following the Cancelled / Pending / Failure convention described in §Response Type.
6. Wire an explicit error-response path using `build_error_response` (see the itaubank common helper at `crates/integrations/connector-integration/src/connectors/itaubank.rs:95-137`) so that cancellation rejections surface as `ErrorResponse` with the connector's `code`/`message`/`reason`.
7. Add unit tests in `<connector>/transformers.rs` covering (a) missing `connector_payout_id`, (b) successful cancellation, (c) connector reports "cannot cancel, already settled".
8. Add an integration test in `backend/grpc-server/tests/` that threads the router-produced `PayoutVoid` gRPC call through the connector sandbox.

## Best Practices

- Always validate `connector_payout_id.is_some()` in `get_url` or `get_request_body`; the `Option` in `PayoutVoidRequest` means the router can legally forward `None`, and the connector will otherwise build a malformed URL. See the request-type definition at `crates/types-traits/domain_types/src/payouts/payouts_types.rs:152-156`.
- Reuse the connector's access-token helper from `PayoutFlowData::get_access_token` (`crates/types-traits/domain_types/src/payouts/payouts_types.rs:54-59`) exactly as the full `PayoutTransfer` does at `crates/integrations/connector-integration/src/connectors/itaubank.rs:335-339`. Do not re-parse auth tokens out of `ConnectorSpecificConfig` inside a void flow.
- Propagate `res.status_code` into `PayoutVoidResponse.status_code` so callers can audit HTTP-level outcomes; itaubank's `PayoutTransfer` implementation does this at `crates/integrations/connector-integration/src/connectors/itaubank.rs:396`.
- Use `build_error_response` for both the integration arm's `get_error_response_v2` and the `ConnectorCommon` implementation, exactly as itaubank does at `itaubank.rs:95-137` and `itaubank.rs:420-423`.
- When mapping connector status, prefer the enum-driven mapping pattern from itaubank's `ItaubankPayoutStatus::status()` helper at `crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:258-274` so Cancellation → `Cancelled` / Pending → `Pending` / Rejection → `Failure` is centralized.
- See sibling flow [pattern_payout_create.md](./pattern_payout_create.md) for the upstream producer of the `connector_payout_id` this flow consumes.

## Common Errors / Gotchas

1. **Problem:** Compile error "conflicting implementations of trait `ConnectorIntegrationV2<PayoutVoid, ...>`".
   **Solution:** The macro already emitted an empty impl. Either remove `PayoutVoid` from the `payout_flows:` list before writing your full impl, or wrap the registration in the default arm. See `crates/integrations/connector-integration/src/connectors/macros.rs:1266-1319` for the macro recursion that generates the conflicting impl.

2. **Problem:** `PayoutVoidResponse.payout_status = PayoutStatus::Cancelled` even though the connector is still processing.
   **Solution:** Do not hardcode status. Map from the connector response enum. Pattern mirrors itaubank's `ItaubankPayoutStatus::status()` helper at `crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:258-274`, where `Pendente`/`EmProcessamento` map to `PayoutStatus::Pending` and only confirmed final states map to terminal outcomes. The corresponding terminal for a void is `PayoutStatus::Cancelled` — see the enum variants at `crates/common/common_enums/src/enums.rs:1134-1149`.

3. **Problem:** `connector_payout_id` missing at runtime because the upstream call never captured it.
   **Solution:** Emit `IntegrationError::MissingRequiredField { field_name: "connector_payout_id", .. }`. The `IntegrationError` variant set is at `crates/types-traits/domain_types/src/errors.rs:168` onward. This is a request-time error, NOT a `ConnectorResponseTransformationError` — keep the error categories distinct per `PATTERN_AUTHORING_SPEC.md` §12.

4. **Problem:** Void succeeded but `PayoutGet` polled right after returns `Pending`.
   **Solution:** Do not force-overwrite the local status based on the void HTTP 200. Return what the connector body says; if the connector's cancel endpoint is async, `Pending` → eventual `Cancelled` via webhook or subsequent `PayoutGet` is the correct sequence. Cross-reference sibling flow [pattern_payout_get.md](./pattern_payout_get.md).

5. **Problem:** Silent `NotImplemented` in production because only the marker trait was added.
   **Solution:** Confirm a concrete override of `get_url`/`get_headers`/`get_request_body`/`handle_response_v2` exists. The macro-only registration at `macros.rs:1388-1403` emits an empty integration body whose default methods raise `IntegrationError::NotImplemented`. Itaubank's `PayoutVoid` is exactly this state at pinned SHA.

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

No connector in the repo exercises these scenarios yet. An integration harness analogous to `backend/grpc-server/tests/payout_transfer.rs` (if/when that lands) is the recommended fixture scaffold.

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
