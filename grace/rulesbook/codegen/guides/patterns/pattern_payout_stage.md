# Payout Stage Flow Pattern

## Overview

The Payout Stage flow requests a quote/rate lock from the connector prior to creating or transferring a payout. It is the "price discovery" step in multi-currency or cross-border payout APIs where the connector must first return a quote id (and sometimes a destination amount) before the merchant commits the funds. The quote id flows forward into `PayoutCreate` or `PayoutTransfer` via `connector_quote_id` (see `PayoutCreateRequest.connector_quote_id` at `crates/types-traits/domain_types/src/payouts/payouts_types.rs:79` and `PayoutTransferRequest.connector_quote_id` at line 102). Staging is idempotent and non-binding — staged quotes may expire on the connector side before being consumed.

### Key Components

- Flow marker: `PayoutStage` — `crates/types-traits/domain_types/src/connector_flow.rs:86`.
- Request type: `PayoutStageRequest` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:136`.
- Response type: `PayoutStageResponse` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:144`.
- Flow-data type: `PayoutFlowData` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:13`.
- Marker trait: `PayoutStageV2` — `crates/types-traits/interfaces/src/connector_types.rs:687`.
- Macro arm: `expand_payout_implementation!` `PayoutStage` arm — `crates/integrations/connector-integration/src/connectors/macros.rs:1404-1419`.

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

Payout Stage differs from most payout flows by not requiring a pre-existing connector-side payout object. Unlike `PayoutVoid` or `PayoutGet`, it is invoked at the start of the payout lifecycle with only `amount`, `source_currency`, `destination_currency`, and an optional `merchant_quote_id`.

### Flow Hierarchy

```
PayoutStage  (this flow — produces connector_payout_id or connector_quote_id)
        |
        v
PayoutCreate  (downstream — consumes quote via connector_quote_id)
        |
        v
PayoutTransfer  (downstream — consumes connector_payout_id)
        |
        v
PayoutGet  (verification)
```

### Flow Type

`PayoutStage` — zero-sized marker struct declared at `crates/types-traits/domain_types/src/connector_flow.rs:86`. Registered in `FlowName::PayoutStage` at `crates/types-traits/domain_types/src/connector_flow.rs:129`.

### Request Type

`PayoutStageRequest` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:136-141`:

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs:135
#[derive(Debug, Clone)]
pub struct PayoutStageRequest {
    pub merchant_quote_id: Option<String>,
    pub amount: common_utils::types::MinorUnit,
    pub source_currency: common_enums::Currency,
    pub destination_currency: common_enums::Currency,
}
```

Note: `PayoutStageRequest` is the narrowest payout request at the pinned SHA — it has no `payout_method_data` field, so connectors cannot branch on beneficiary rails when quoting. This is by design: staging is intended to return an indicative rate only.

### Response Type

`PayoutStageResponse` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:144-149`:

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs:143
#[derive(Debug, Clone)]
pub struct PayoutStageResponse {
    pub merchant_payout_id: Option<String>,
    pub payout_status: common_enums::PayoutStatus,
    pub connector_payout_id: Option<String>,
    pub status_code: u16,
}
```

`PayoutStageResponse` does not currently carry a dedicated `connector_quote_id` field; connectors that return a quote id put it into `connector_payout_id` and downstream `PayoutCreate` lifts it into `PayoutCreateRequest.connector_quote_id` at the router layer. The `payout_status` field at line 147 uses `common_enums::PayoutStatus` from `crates/common/common_enums/src/enums.rs:1134-1149`; a successful quote typically maps to `PayoutStatus::RequiresConfirmation` (line 1145) to signal "quote ready, not yet committed".

### Resource Common Data

`PayoutFlowData` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:13-23`. Identical envelope as all other payout flows; see [pattern_payout_void.md](./pattern_payout_void.md) for a full field-by-field breakdown.

### RouterDataV2 Shape

```rust
RouterDataV2<PayoutStage, PayoutFlowData, PayoutStageRequest, PayoutStageResponse>
```

Canonical four-arg shape per §7 of `PATTERN_AUTHORING_SPEC.md`.

## Connectors with Full Implementation

At the pinned SHA, **no connector supplies a non-stub `ConnectorIntegrationV2<PayoutStage, ...>` implementation.** A grep across `crates/integrations/connector-integration/src/connectors/` for `ConnectorIntegrationV2<\s*PayoutStage` returns zero matches. The only connector that registers the `PayoutStageV2` marker trait is **itaubank**, and it does so through the macro-generated default body at `crates/integrations/connector-integration/src/connectors/itaubank.rs:53-66` plus `crates/integrations/connector-integration/src/connectors/macros.rs:1404-1419`.

Current implementation coverage: **0 connectors** (itaubank registers the marker trait only; no URL/headers/body/response-parsing are provided).

| Connector | HTTP Method | Content Type | URL Pattern | Request Type Reuse | Notes |
| --- | --- | --- | --- | --- | --- |
| _(none)_ | — | — | — | — | See Stub Implementations below. |

### Stub Implementations

- **itaubank** — macro-registered stub. The `PayoutStage` arm of `expand_payout_implementation!` (`crates/integrations/connector-integration/src/connectors/macros.rs:1404-1419`) emits `impl PayoutStageV2 for Itaubank<T> {}` and `impl ConnectorIntegrationV2<PayoutStage, PayoutFlowData, PayoutStageRequest, PayoutStageResponse> for Itaubank<T> {}` with empty bodies. All overridable methods fall back to `ConnectorIntegrationV2` trait defaults → `IntegrationError::NotImplemented` at runtime.

## Common Implementation Patterns

### Macro-Based Pattern (Recommended)

To register the marker trait, include `PayoutStage` in the `payout_flows:` list:

```rust
// From crates/integrations/connector-integration/src/connectors/itaubank.rs:53
macros::macro_connector_payout_implementation!(
    connector: Itaubank,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    payout_flows: [
        PayoutCreate,
        PayoutGet,
        PayoutVoid,
        PayoutStage,     // <-- registers PayoutStageV2 + empty ConnectorIntegrationV2 impl
        PayoutCreateLink,
        PayoutCreateRecipient,
        PayoutEnrollDisburseAccount
    ]
);
```

The recursion in `macro_connector_payout_implementation!` at `crates/integrations/connector-integration/src/connectors/macros.rs:1266-1319` drives each token to `expand_payout_implementation!`. The `PayoutStage` arm at `macros.rs:1404-1419` reads:

```rust
// From crates/integrations/connector-integration/src/connectors/macros.rs:1404
(
    connector: $connector: ident,
    flow: PayoutStage,
    generic_type: $generic_type:tt,
    [ $($bounds:tt)* ]
) => {
    impl<$generic_type: $($bounds)*> ::interfaces::connector_types::PayoutStageV2 for $connector<$generic_type> {}
    impl<$generic_type: $($bounds)*>
        ::interfaces::connector_integration_v2::ConnectorIntegrationV2<
            ::domain_types::connector_flow::PayoutStage,
            ::domain_types::payouts::payouts_types::PayoutFlowData,
            ::domain_types::payouts::payouts_types::PayoutStageRequest,
            ::domain_types::payouts::payouts_types::PayoutStageResponse,
        > for $connector<$generic_type>
    {}
};
```

To move from stub to full, supply a concrete `impl ConnectorIntegrationV2<PayoutStage, ...> for <Connector><T>` (and remove `PayoutStage` from the macro list to avoid the duplicate-impl compile error). Reference shape: itaubank's full `PayoutTransfer` at `crates/integrations/connector-integration/src/connectors/itaubank.rs:294-424`.

### Request-Body Strategies Observed for Quote-Style Flows

1. **Quote GET** — connectors model staging as a read. `GET {base_url}/quotes?source=...&target=...&amount=...`. No body.
2. **Quote POST** — connectors accept a body of `{ source_currency, destination_currency, amount }`. `POST {base_url}/quotes`.
3. **Combined create+quote** — some connectors merge staging into `PayoutCreate` and skip `PayoutStage` entirely.

No connector in connector-service implements any of these shapes for `PayoutStage` at the pinned SHA.

## Connector-Specific Patterns

### itaubank

- itaubank includes `PayoutStage` in its `payout_flows:` list at `crates/integrations/connector-integration/src/connectors/itaubank.rs:61`, but the SiSPAG integration is a single-currency BRL-only product and does not expose a quote endpoint. The transformers file `crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs` contains no `PayoutStageRequest`/`PayoutStageResponse` `TryFrom` blocks at any line. The flow is registered-but-inert.

No other connector in `crates/integrations/connector-integration/src/connectors/` registers `PayoutStageV2`.

## Code Examples

### 1. Macro registration

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

### 2. Marker trait definition

```rust
// From crates/types-traits/interfaces/src/connector_types.rs:687
pub trait PayoutStageV2:
    ConnectorIntegrationV2<
    connector_flow::PayoutStage,
    PayoutFlowData,
    PayoutStageRequest,
    PayoutStageResponse,
>
{
}
```

### 3. Request-type shape

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs:135
#[derive(Debug, Clone)]
pub struct PayoutStageRequest {
    pub merchant_quote_id: Option<String>,
    pub amount: common_utils::types::MinorUnit,
    pub source_currency: common_enums::Currency,
    pub destination_currency: common_enums::Currency,
}
```

### 4. Reference implementation shape (adapted from `PayoutTransfer` on itaubank)

```rust
// Adapted shape — see crates/integrations/connector-integration/src/connectors/itaubank.rs:294-424
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PayoutStage,
        PayoutFlowData,
        PayoutStageRequest,
        PayoutStageResponse,
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
        req: &RouterDataV2<PayoutStage, PayoutFlowData, PayoutStageRequest, PayoutStageResponse>,
    ) -> CustomResult<String, IntegrationError> {
        let base_url = self.base_url(&req.resource_common_data.connectors);
        Ok(format!("{base_url}/v1/quotes"))
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<PayoutStage, PayoutFlowData, PayoutStageRequest, PayoutStageResponse>,
    ) -> CustomResult<Option<RequestContent>, IntegrationError> {
        let connector_req = <ConnectorQuoteRequest>::try_from(req)?;
        Ok(Some(RequestContent::Json(Box::new(connector_req))))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<PayoutStage, PayoutFlowData, PayoutStageRequest, PayoutStageResponse>,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<PayoutStage, PayoutFlowData, PayoutStageRequest, PayoutStageResponse>,
        ConnectorResponseTransformationError,
    > {
        // Map the connector's quote-ready state to PayoutStatus::RequiresConfirmation.
        todo!("connector-specific quote-response parsing")
    }
}
```

Status mapping MUST be derived from the connector response; per §11 of `PATTERN_AUTHORING_SPEC.md` a literal such as `payout_status: PayoutStatus::RequiresConfirmation` inside the `TryFrom` block is banned unless it is the documented "every 2xx means quote-ready" contract for that connector (and even then the HTTP status code branch must be explicit, following the itaubank precedent at `itaubank.rs:388-414`).

### 5. Amount-conversion note

`PayoutStageRequest.amount` is `common_utils::types::MinorUnit` (see line 138 of payouts_types.rs). Convert to the connector-specific shape with the amount-converter macro (`create_amount_converter_wrapper!`) as documented in `utility_functions_reference.md`; itaubank's `PayoutTransfer` does this with `StringMajorUnitForConnector` at `crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:163-168`.

## Integration Guidelines

1. Confirm the connector exposes a quote/staging endpoint. If the connector's payout API folds staging into `PayoutCreate` (common for single-currency domestic rails), skip `PayoutStage` and wire only the downstream flows.
2. Add `PayoutStage` to the `payout_flows:` list in `<connector>.rs` (see `crates/integrations/connector-integration/src/connectors/itaubank.rs:57-65`).
3. Write a dedicated `impl ConnectorIntegrationV2<PayoutStage, PayoutFlowData, PayoutStageRequest, PayoutStageResponse> for <Connector><T>` block. Because `PayoutStage` produces no connector-side payout object yet, URL construction typically does NOT embed an id — it is a bare `/quotes`-style POST.
4. Because `PayoutStageRequest` has no `payout_method_data`, the connector's quote request struct MUST be derivable purely from `amount`/`source_currency`/`destination_currency`/`merchant_quote_id`. If the connector also needs a beneficiary for a quote, signal this via `IntegrationError::FeatureNotSupported` to the router — do not invent `payout_method_data` from thin air.
5. In `<connector>/transformers.rs`, add a `TryFrom<&RouterDataV2<PayoutStage, ...>>` impl that produces the connector's quote-request struct. Use the same amount-conversion pattern as itaubank's `PayoutTransfer` at `itaubank/transformers.rs:163-168`.
6. Add a response-side `TryFrom<ResponseRouterData<..>, Self>>` that maps the quote id (usually) into `connector_payout_id` and sets `payout_status` to `PayoutStatus::RequiresConfirmation`.
7. Plumb `merchant_quote_id` into whatever "reference" field the connector exposes so the quote can be correlated by merchant systems.
8. Write unit tests for the quote success and quote-rejection paths.

## Best Practices

- Prefer a minimal connector-side quote request. The request type at `crates/types-traits/domain_types/src/payouts/payouts_types.rs:136-141` only has four fields; respect that minimalism and do not fabricate beneficiary defaults.
- Map quote id → `connector_payout_id`. Downstream `PayoutCreate` is already written to lift `connector_payout_id` into `connector_quote_id` via `PayoutCreateRequest.connector_quote_id` at `crates/types-traits/domain_types/src/payouts/payouts_types.rs:79`.
- Use `PayoutStatus::RequiresConfirmation` (variant at `crates/common/common_enums/src/enums.rs:1145`) for a successful staged quote, NOT `PayoutStatus::Success`. A staged quote is not a completed payout.
- Use `build_error_response` exactly as itaubank does at `crates/integrations/connector-integration/src/connectors/itaubank.rs:95-137` when the connector rejects a quote (e.g. "unsupported currency pair").
- See the downstream pattern [pattern_payout_create.md](./pattern_payout_create.md) for how `connector_quote_id` is consumed after staging.

## Common Errors / Gotchas

1. **Problem:** `PayoutStageResponse.payout_status = PayoutStatus::Success` even though the connector only returned a quote.
   **Solution:** A quote is not a settled payout. Map successful quote responses to `PayoutStatus::RequiresConfirmation` (variant at `crates/common/common_enums/src/enums.rs:1145`). The router uses this status to decide whether to auto-progress to `PayoutCreate` or require a merchant-side confirmation.

2. **Problem:** Connector's quote endpoint requires a beneficiary but `PayoutStageRequest` has no `payout_method_data`.
   **Solution:** Mark the staging flow as unsupported for that connector. Do not synthesize a dummy beneficiary. Emit `IntegrationError::FeatureNotSupported` with a message naming the missing field. See the `IntegrationError` enum at `crates/types-traits/domain_types/src/errors.rs:168` onward.

3. **Problem:** Quote id is returned but `connector_payout_id` in `PayoutStageResponse` is `None`, and downstream `PayoutCreate` fails because `connector_quote_id` is also `None`.
   **Solution:** Populate `connector_payout_id: Some(quote_id)` inside the response `TryFrom`. The type is `Option<String>` at line 147 of payouts_types.rs; returning `None` on a success path is a contract violation.

4. **Problem:** Compile error "conflicting implementations of trait `ConnectorIntegrationV2<PayoutStage, ...>`".
   **Solution:** The macro already emitted an empty impl. Remove `PayoutStage` from the `payout_flows:` list before writing the full impl. See `crates/integrations/connector-integration/src/connectors/macros.rs:1266-1319`.

5. **Problem:** Quote expires between `PayoutStage` and `PayoutCreate` and the merchant sees an opaque "quote not found" error.
   **Solution:** In `PayoutCreate` transformers, detect the connector's "quote expired" error code and re-issue `PayoutStage`. Cross-ref [pattern_payout_create.md](./pattern_payout_create.md).

## Testing Notes

### Unit Tests

Each connector implementing PayoutStage should cover:

- `TryFrom<&RouterDataV2<PayoutStage, ...>>` — valid USD-to-EUR quote, asserts body has correct currency codes and minor-unit amount.
- `TryFrom<ResponseRouterData<ConnectorQuoteResponse, Self>>` — maps quote id → `connector_payout_id` and status → `PayoutStatus::RequiresConfirmation`.
- Unsupported currency pair — connector returns 422 → error path emits `ErrorResponse` with the connector's code and reason.

### Integration Scenarios

| Scenario | Inputs | Expected `payout_status` | Expected `status_code` |
| --- | --- | --- | --- |
| Stage cross-border quote | amount=10000 MinorUnit, USD → EUR | `RequiresConfirmation` | 200 |
| Stage unsupported pair | amount=10000, USD → ZMW (unsupported) | — (error) | 4xx |
| Stage with merchant_quote_id | amount=10000, USD→EUR, merchant_quote_id=Some("abc") | `RequiresConfirmation` | 200 |
| Zero-amount stage | amount=0 | — (error, `IntegrationError::InvalidRequestData`) | 422 |

No connector in connector-service exercises these scenarios at the pinned SHA.

## Cross-References

- Parent index: [../README.md](./README.md)
- Sibling core payout flow: [pattern_payout_create.md](./pattern_payout_create.md)
- Sibling core payout flow: [pattern_payout_transfer.md](./pattern_payout_transfer.md)
- Sibling core payout flow: [pattern_payout_get.md](./pattern_payout_get.md)
- Sibling side-flow: [pattern_payout_void.md](./pattern_payout_void.md)
- Sibling side-flow: [pattern_payout_create_link.md](./pattern_payout_create_link.md)
- Macro reference: [macro_patterns_reference.md](./macro_patterns_reference.md)
- Utility helpers: [utility_functions_reference.md](../utility_functions_reference.md)
- Authoring spec: [PATTERN_AUTHORING_SPEC.md](./PATTERN_AUTHORING_SPEC.md)
