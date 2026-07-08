# Payout Create-Link Flow Pattern

## Overview

The Payout Create-Link flow asks the connector to generate a hosted or pay-by-link URL that the beneficiary can open to claim funds (typically for Interac e-Transfer style rails, Open Banking UK pull-payouts, or PayPal/Venmo send-to-email flows). Unlike `PayoutCreate`, which commits funds to a known beneficiary account, this flow defers beneficiary collection to the connector's hosted page. The response typically contains a connector-side payout id plus, out-of-band, a hosted URL that must be surfaced to the merchant.

Note on current type shape: at the pinned SHA `PayoutCreateLinkResponse` (at `crates/types-traits/domain_types/src/payouts/payouts_types.rs:180-185`) does not carry a dedicated URL field. Connectors that implement this flow must either (a) park the URL inside `connector_payout_id` with a documented format, or (b) surface it via `raw_connector_response` on `PayoutFlowData` (see field at line 18). This is a current limitation — future additions to `PayoutCreateLinkResponse` should add a dedicated `payout_link_url: Option<String>` field.

### Key Components

- Flow marker: `PayoutCreateLink` — `crates/types-traits/domain_types/src/connector_flow.rs:89`.
- Request type: `PayoutCreateLinkRequest` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:166`.
- Response type: `PayoutCreateLinkResponse` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:180`.
- Flow-data type: `PayoutFlowData` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:13`.
- Marker trait: `PayoutCreateLinkV2` — `crates/types-traits/interfaces/src/connector_types.rs:697`.
- Macro arm: `expand_payout_implementation!` `PayoutCreateLink` arm — `crates/integrations/connector-integration/src/connectors/macros.rs:1420-1435`.

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

Payout Create-Link is a producer flow: it creates state on the connector side (a pending payout linked to a hosted page) and returns an identifier. It is the alternative to `PayoutCreate` for connectors that prefer hosted beneficiary collection.

### Flow Hierarchy

```
PayoutStage  (optional quote lock; upstream)
        |
        v
PayoutCreateLink  (this flow — generates hosted URL)
        |
        v
<merchant opens URL out-of-band>
        |
        v
PayoutGet  (poll for beneficiary-claim completion)
```

### Flow Type

`PayoutCreateLink` — zero-sized marker struct declared at `crates/types-traits/domain_types/src/connector_flow.rs:89`. Registered in `FlowName::PayoutCreateLink` at `crates/types-traits/domain_types/src/connector_flow.rs:130`.

### Request Type

`PayoutCreateLinkRequest` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:166-177`:

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs:165
#[derive(Debug, Clone)]
pub struct PayoutCreateLinkRequest {
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

This is a structural duplicate of `PayoutCreateRequest` (`crates/types-traits/domain_types/src/payouts/payouts_types.rs:77-89`); the only semantic difference is the downstream connector behavior — create-link yields a redirect URL while create yields a terminal payout object. `PayoutMethodData` is the enum at `crates/types-traits/domain_types/src/payouts/payout_method_data.rs:7-13`.

### Response Type

`PayoutCreateLinkResponse` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:180-185`:

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs:179
#[derive(Debug, Clone)]
pub struct PayoutCreateLinkResponse {
    pub merchant_payout_id: Option<String>,
    pub payout_status: common_enums::PayoutStatus,
    pub connector_payout_id: Option<String>,
    pub status_code: u16,
}
```

The response is structurally identical to `PayoutCreateResponse` at `payouts_types.rs:92-97`. A successful link generation should map to `PayoutStatus::RequiresFulfillment` (variant at `crates/common/common_enums/src/enums.rs:1147`) to signal "link exists, awaiting beneficiary action".

### Resource Common Data

`PayoutFlowData` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:13-23`. See [pattern_payout_void.md](./pattern_payout_void.md) for the full field-by-field breakdown.

### RouterDataV2 Shape

```rust
RouterDataV2<PayoutCreateLink, PayoutFlowData, PayoutCreateLinkRequest, PayoutCreateLinkResponse>
```

Canonical four-arg shape per §7 of `PATTERN_AUTHORING_SPEC.md`.

## Connectors with Full Implementation

At the pinned SHA, **no connector supplies a non-stub `ConnectorIntegrationV2<PayoutCreateLink, ...>` implementation.** A grep across `crates/integrations/connector-integration/src/connectors/` for `ConnectorIntegrationV2<\s*PayoutCreateLink` returns zero matches. The only connector registering the `PayoutCreateLinkV2` marker is **itaubank** at `crates/integrations/connector-integration/src/connectors/itaubank.rs:62` via the macro at `crates/integrations/connector-integration/src/connectors/macros.rs:1420-1435`.

Current implementation coverage: **0 connectors** (itaubank registers the marker trait only; no URL/headers/body/response-parsing are provided).

| Connector | HTTP Method | Content Type | URL Pattern | Request Type Reuse | Notes |
| --- | --- | --- | --- | --- | --- |
| _(none)_ | — | — | — | — | See Stub Implementations below. |

### Stub Implementations

- **itaubank** — macro-registered stub. The `PayoutCreateLink` arm of `expand_payout_implementation!` (`crates/integrations/connector-integration/src/connectors/macros.rs:1420-1435`) emits `impl PayoutCreateLinkV2 for Itaubank<T> {}` and `impl ConnectorIntegrationV2<PayoutCreateLink, PayoutFlowData, PayoutCreateLinkRequest, PayoutCreateLinkResponse> for Itaubank<T> {}` with empty bodies. All methods fall back to trait defaults.

## Common Implementation Patterns

### Macro-Based Pattern (Recommended)

Register the marker trait by including `PayoutCreateLink` in `payout_flows:`:

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
        PayoutStage,
        PayoutCreateLink,  // <-- registers PayoutCreateLinkV2 + empty ConnectorIntegrationV2 impl
        PayoutCreateRecipient,
        PayoutEnrollDisburseAccount
    ]
);
```

The `PayoutCreateLink` arm at `crates/integrations/connector-integration/src/connectors/macros.rs:1420-1435` produces:

```rust
// From crates/integrations/connector-integration/src/connectors/macros.rs:1420
(
    connector: $connector: ident,
    flow: PayoutCreateLink,
    generic_type: $generic_type:tt,
    [ $($bounds:tt)* ]
) => {
    impl<$generic_type: $($bounds)*> ::interfaces::connector_types::PayoutCreateLinkV2 for $connector<$generic_type> {}
    impl<$generic_type: $($bounds)*>
        ::interfaces::connector_integration_v2::ConnectorIntegrationV2<
            ::domain_types::connector_flow::PayoutCreateLink,
            ::domain_types::payouts::payouts_types::PayoutFlowData,
            ::domain_types::payouts::payouts_types::PayoutCreateLinkRequest,
            ::domain_types::payouts::payouts_types::PayoutCreateLinkResponse,
        > for $connector<$generic_type>
    {}
};
```

To move from stub to full, provide a concrete `impl ConnectorIntegrationV2<PayoutCreateLink, ...>` (and remove the marker from `payout_flows:` to avoid duplicate impls). Reference shape: itaubank's full `PayoutTransfer` at `crates/integrations/connector-integration/src/connectors/itaubank.rs:294-424`.

### Hosted-Link URL-Surfacing Strategies

Because `PayoutCreateLinkResponse` does not carry a dedicated URL field at the pinned SHA, connectors that implement this flow must pick one:

1. **Raw-response capture** — let the platform capture the full response body into `PayoutFlowData.raw_connector_response` (the `RawConnectorRequestResponse` trait impl at `crates/types-traits/domain_types/src/payouts/payouts_types.rs:25-41` does this automatically). Callers parse the URL out of `raw_connector_response`.
2. **Composite id** — concatenate `id|url` into `connector_payout_id` with a documented separator. Not recommended because downstream `PayoutGet` expects a clean id.
3. **Merchant webhook** — rely on the connector to push the hosted URL to the merchant via its own webhook rather than returning it synchronously.

Strategy (1) is the cleanest given the current type shape. Strategy (2) is a compatibility hazard and must be flagged in PR review.

## Connector-Specific Patterns

### itaubank

- itaubank includes `PayoutCreateLink` in its `payout_flows:` list at `crates/integrations/connector-integration/src/connectors/itaubank.rs:62`. Itau's SiSPAG product is a direct-credit rail and does not expose a hosted payout-link page, so the flow is registered as a stub only. The transformers file `crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs` contains no `PayoutCreateLinkRequest`/`PayoutCreateLinkResponse` `TryFrom` blocks.

No other connector in `crates/integrations/connector-integration/src/connectors/` registers `PayoutCreateLinkV2`.

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

### 2. Marker trait definition

```rust
// From crates/types-traits/interfaces/src/connector_types.rs:697
pub trait PayoutCreateLinkV2:
    ConnectorIntegrationV2<
    connector_flow::PayoutCreateLink,
    PayoutFlowData,
    PayoutCreateLinkRequest,
    PayoutCreateLinkResponse,
>
{
}
```

### 3. Request type

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs:165
#[derive(Debug, Clone)]
pub struct PayoutCreateLinkRequest {
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

### 4. Reference implementation shape (adapted from `PayoutTransfer`)

```rust
// Adapted shape — see crates/integrations/connector-integration/src/connectors/itaubank.rs:294-424
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PayoutCreateLink,
        PayoutFlowData,
        PayoutCreateLinkRequest,
        PayoutCreateLinkResponse,
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
        req: &RouterDataV2<PayoutCreateLink, PayoutFlowData, PayoutCreateLinkRequest, PayoutCreateLinkResponse>,
    ) -> CustomResult<String, IntegrationError> {
        let base_url = self.base_url(&req.resource_common_data.connectors);
        Ok(format!("{base_url}/v1/payout-links"))
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<PayoutCreateLink, PayoutFlowData, PayoutCreateLinkRequest, PayoutCreateLinkResponse>,
    ) -> CustomResult<Option<RequestContent>, IntegrationError> {
        let connector_req = <ConnectorLinkRequest>::try_from(req)?;
        Ok(Some(RequestContent::Json(Box::new(connector_req))))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<PayoutCreateLink, PayoutFlowData, PayoutCreateLinkRequest, PayoutCreateLinkResponse>,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<PayoutCreateLink, PayoutFlowData, PayoutCreateLinkRequest, PayoutCreateLinkResponse>,
        ConnectorResponseTransformationError,
    > {
        // Parse link response; the platform retains the raw body on PayoutFlowData.raw_connector_response.
        todo!("connector-specific link-response parsing")
    }
}
```

### 5. Amount-conversion note

`PayoutCreateLinkRequest.amount` is `common_utils::types::MinorUnit`. Convert to the connector-specific shape with the amount-converter macro as documented in `utility_functions_reference.md`. Itaubank's `PayoutTransfer` does this at `crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:163-168`.

## Integration Guidelines

1. Confirm the connector exposes a hosted-link endpoint. Many payout APIs do NOT — staging + create + get is the common shape. If unsupported, leave this flow as the macro-registered stub.
2. Add `PayoutCreateLink` to the `payout_flows:` list in `<connector>.rs` (reference: `crates/integrations/connector-integration/src/connectors/itaubank.rs:57-65`).
3. Write a concrete `impl ConnectorIntegrationV2<PayoutCreateLink, ...>` block and REMOVE `PayoutCreateLink` from `payout_flows:` to avoid duplicate-impl errors.
4. Decide how to surface the hosted URL: prefer relying on `PayoutFlowData.raw_connector_response` (auto-captured via the `RawConnectorRequestResponse` impl at `crates/types-traits/domain_types/src/payouts/payouts_types.rs:25-41`) rather than overloading `connector_payout_id`.
5. Map the success status to `PayoutStatus::RequiresFulfillment` (variant at `crates/common/common_enums/src/enums.rs:1147`). Do NOT use `PayoutStatus::Success` — the beneficiary has not yet acted.
6. Pipe `webhook_url` (field at `payouts_types.rs:175`) into the connector's webhook-callback field if exposed, so the beneficiary-claim event lands back on the router.
7. Write unit tests covering: link creation success, unsupported currency, link creation with `payout_method_data = None` (if the connector allows it), and malformed `webhook_url`.
8. Write an integration test that creates a link, polls via `PayoutGet`, and asserts eventual `Success`.

## Best Practices

- Reuse the `PayoutCreateRequest` transformer if the connector's link and create endpoints accept the same body shape. Both request types carry the same fields — see the type definitions at `crates/types-traits/domain_types/src/payouts/payouts_types.rs:77-89` and `payouts_types.rs:166-177`.
- Always propagate `webhook_url` — links without a webhook force long-polling via `PayoutGet`. Field is `Option<String>` at line 175.
- Surface the hosted URL via `raw_connector_response` rather than overloading `connector_payout_id`. The raw-capture mechanism is wired on the flow-data type at `crates/types-traits/domain_types/src/payouts/payouts_types.rs:25-41`.
- See sibling flow [pattern_payout_create.md](./pattern_payout_create.md) for the direct-credit alternative that doesn't require a hosted page.

## Common Errors / Gotchas

1. **Problem:** Beneficiary never claims the link and `payout_status` stays `RequiresFulfillment` forever.
   **Solution:** Expected. The router polls via `PayoutGet`; connector-side link-expiry events surface via the connector webhook (wired through `webhook_url` at `crates/types-traits/domain_types/src/payouts/payouts_types.rs:175`). Cross-ref [pattern_payout_get.md](./pattern_payout_get.md).

2. **Problem:** `connector_payout_id` stuffed with `"id|https://..."` format breaks downstream `PayoutGet` URL construction.
   **Solution:** Stash the URL in `raw_connector_response` instead. Do not overload `connector_payout_id`.

3. **Problem:** `PayoutStatus::Success` mapped to successful link creation.
   **Solution:** A created link is `RequiresFulfillment`, not `Success`. Variants listed at `crates/common/common_enums/src/enums.rs:1134-1149`.

4. **Problem:** Compile error "conflicting implementations of trait `ConnectorIntegrationV2<PayoutCreateLink, ...>`".
   **Solution:** Remove `PayoutCreateLink` from the `payout_flows:` macro list when writing the full impl. See `crates/integrations/connector-integration/src/connectors/macros.rs:1266-1319`.

5. **Problem:** `payout_method_data = None` at the router but the connector requires it for the link page template (e.g. to pre-fill beneficiary email).
   **Solution:** Return `IntegrationError::MissingRequiredField { field_name: "payout_method_data", .. }` in `get_request_body` so the router surfaces a 4xx instead of silently failing downstream. `IntegrationError` variants at `crates/types-traits/domain_types/src/errors.rs:168` onward.

## Testing Notes

### Unit Tests

Each connector implementing PayoutCreateLink should cover:

- `TryFrom<&RouterDataV2<PayoutCreateLink, ...>>` with a complete request — asserts URL and body.
- `TryFrom<&RouterDataV2<PayoutCreateLink, ...>>` with `webhook_url = Some(...)` — asserts the webhook URL propagates into the connector body.
- Response parsing for a successful link creation → `PayoutStatus::RequiresFulfillment` with `connector_payout_id` populated.
- Error path — connector rejects with "unsupported currency" → `ErrorResponse`.

### Integration Scenarios

| Scenario | Inputs | Expected `payout_status` | Expected `status_code` |
| --- | --- | --- | --- |
| Create link for domestic transfer | amount=10000, USD→USD, Interac | `RequiresFulfillment` | 200 |
| Create link with webhook | amount=10000, USD→USD, webhook_url=Some(...) | `RequiresFulfillment` | 200 |
| Create link, unsupported pair | amount=10000, USD→ZMW | — (error) | 4xx |
| Beneficiary claims link (downstream) | poll via PayoutGet | `Success` | 200 |

No connector in connector-service exercises these scenarios at the pinned SHA.

## Cross-References

- Parent index: [../README.md](./README.md)
- Sibling core payout flow: [pattern_payout_create.md](./pattern_payout_create.md)
- Sibling core payout flow: [pattern_payout_transfer.md](./pattern_payout_transfer.md)
- Sibling core payout flow: [pattern_payout_get.md](./pattern_payout_get.md)
- Sibling side-flow: [pattern_payout_void.md](./pattern_payout_void.md)
- Sibling side-flow: [pattern_payout_stage.md](./pattern_payout_stage.md)
- Sibling side-flow: [pattern_payout_create_recipient.md](./pattern_payout_create_recipient.md)
- Macro reference: [macro_patterns_reference.md](./macro_patterns_reference.md)
- Utility helpers: [utility_functions_reference.md](../utility_functions_reference.md)
- Authoring spec: [PATTERN_AUTHORING_SPEC.md](./PATTERN_AUTHORING_SPEC.md)
