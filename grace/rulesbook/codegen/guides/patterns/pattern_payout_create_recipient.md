# Payout Create-Recipient Flow Pattern

## Overview

The Payout Create-Recipient flow registers a beneficiary (recipient) with the connector so subsequent payouts can reference a stable recipient id instead of re-submitting full bank / wallet details on every transfer. This flow is the onboarding step for connectors that distinguish "create recipient" (KYC/verification) from "disburse to recipient" (fund movement). Once created, the connector-assigned recipient id is returned and typically threaded into `PayoutCreateRequest.connector_payout_method_id` or a similar beneficiary-reference field on downstream flows.

This flow is distinct from `PayoutEnrollDisburseAccount`: create-recipient produces a long-lived recipient profile (one-time KYC); enroll-disburse-account attaches a specific bank account to an already-existing recipient. Some connectors fold the two into a single API call; when that happens, expose only one of the two flows and document the choice.

### Key Components

- Flow marker: `PayoutCreateRecipient` — `crates/types-traits/domain_types/src/connector_flow.rs:92`.
- Request type: `PayoutCreateRecipientRequest` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:188`.
- Response type: `PayoutCreateRecipientResponse` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:197`.
- Flow-data type: `PayoutFlowData` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:13`.
- Marker trait: `PayoutCreateRecipientV2` — `crates/types-traits/interfaces/src/connector_types.rs:707`.
- Macro arm: `expand_payout_implementation!` `PayoutCreateRecipient` arm — `crates/integrations/connector-integration/src/connectors/macros.rs:1436-1451`.

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

Payout Create-Recipient is a long-lived producer: the connector persists recipient KYC state, and the returned id must be retained by the merchant (typically in their own mapping store). Unlike `PayoutCreate`, this flow usually returns immediately with a recipient id rather than a transfer id.

### Flow Hierarchy

```
PayoutCreateRecipient  (this flow — produces recipient id)
        |
        v
[optional: PayoutEnrollDisburseAccount — attach bank account to recipient]
        |
        v
PayoutCreate / PayoutTransfer  (downstream — reference the recipient by connector_payout_method_id)
        |
        v
PayoutGet
```

### Flow Type

`PayoutCreateRecipient` — zero-sized marker struct declared at `crates/types-traits/domain_types/src/connector_flow.rs:92`. Registered in `FlowName::PayoutCreateRecipient` at `crates/types-traits/domain_types/src/connector_flow.rs:131`.

### Request Type

`PayoutCreateRecipientRequest` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:188-194`:

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs:187
#[derive(Debug, Clone)]
pub struct PayoutCreateRecipientRequest {
    pub merchant_payout_id: Option<String>,
    pub amount: common_utils::types::MinorUnit,
    pub source_currency: common_enums::Currency,
    pub payout_method_data: Option<PayoutMethodData>,
    pub recipient_type: common_enums::PayoutRecipientType,
}
```

Notable fields:

- `recipient_type: common_enums::PayoutRecipientType` — enum at `crates/common/common_enums/src/enums.rs:1191-1203`. Variants include `Individual`, `Company`, `NonProfit`, `PublicSector`, `NaturalPerson` (Adyen taxonomy) and `Business`, `Personal` (Wise taxonomy). The first-seen merchant variant is `Individual`.
- `payout_method_data: Option<PayoutMethodData>` — enum at `crates/types-traits/domain_types/src/payouts/payout_method_data.rs:7-13` with variants `Card`, `Bank`, `Wallet`, `BankRedirect`, `Passthrough`. When `None`, the connector must support recipient creation without a bound account (rare) or the transformer MUST reject the request.

The presence of `amount` and `source_currency` on a recipient-create request is unusual; the typical connector API only requires KYC fields. These fields exist on `PayoutCreateRecipientRequest` because the platform carries them through the `PayoutFlowData` envelope even when the connector does not need them. Transformers SHOULD ignore `amount` in the create-recipient body when the connector does not accept it.

### Response Type

`PayoutCreateRecipientResponse` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:197-202`:

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs:196
#[derive(Debug, Clone)]
pub struct PayoutCreateRecipientResponse {
    pub merchant_payout_id: Option<String>,
    pub payout_status: common_enums::PayoutStatus,
    pub connector_payout_id: Option<String>,
    pub status_code: u16,
}
```

The connector-assigned recipient id is placed into `connector_payout_id` (`Option<String>` at line 200). `payout_status` maps to:

- `PayoutStatus::RequiresFulfillment` — recipient created, no bound account yet (variant at `crates/common/common_enums/src/enums.rs:1147`).
- `PayoutStatus::RequiresVendorAccountCreation` — recipient in KYC-pending state (variant at `crates/common/common_enums/src/enums.rs:1148`).
- `PayoutStatus::Success` — recipient ready for use (rare on first creation).
- `PayoutStatus::Failure` — KYC rejected.

### Resource Common Data

`PayoutFlowData` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:13-23`. See [pattern_payout_void.md](./pattern_payout_void.md) for the full breakdown.

### RouterDataV2 Shape

```rust
RouterDataV2<PayoutCreateRecipient, PayoutFlowData, PayoutCreateRecipientRequest, PayoutCreateRecipientResponse>
```

Canonical four-arg shape per §7 of `PATTERN_AUTHORING_SPEC.md`.

## Connectors with Full Implementation

At the pinned SHA, **no connector supplies a non-stub `ConnectorIntegrationV2<PayoutCreateRecipient, ...>` implementation.** A grep across `crates/integrations/connector-integration/src/connectors/` for `ConnectorIntegrationV2<\s*PayoutCreateRecipient` returns zero matches. The only connector registering the `PayoutCreateRecipientV2` marker is **itaubank** at `crates/integrations/connector-integration/src/connectors/itaubank.rs:63` via the macro at `crates/integrations/connector-integration/src/connectors/macros.rs:1436-1451`.

Current implementation coverage: **0 connectors** (itaubank registers the marker trait only; no URL/headers/body/response-parsing are provided).

| Connector | HTTP Method | Content Type | URL Pattern | Request Type Reuse | Notes |
| --- | --- | --- | --- | --- | --- |
| _(none)_ | — | — | — | — | See Stub Implementations below. |

### Stub Implementations

- **itaubank** — macro-registered stub. The `PayoutCreateRecipient` arm of `expand_payout_implementation!` (`crates/integrations/connector-integration/src/connectors/macros.rs:1436-1451`) emits `impl PayoutCreateRecipientV2 for Itaubank<T> {}` and `impl ConnectorIntegrationV2<PayoutCreateRecipient, PayoutFlowData, PayoutCreateRecipientRequest, PayoutCreateRecipientResponse> for Itaubank<T> {}` with empty bodies.

## Common Implementation Patterns

### Macro-Based Pattern (Recommended)

Register by listing `PayoutCreateRecipient` in `payout_flows:`:

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
        PayoutCreateLink,
        PayoutCreateRecipient,   // <-- registers marker + empty integration impl
        PayoutEnrollDisburseAccount
    ]
);
```

The `PayoutCreateRecipient` arm at `crates/integrations/connector-integration/src/connectors/macros.rs:1436-1451` produces:

```rust
// From crates/integrations/connector-integration/src/connectors/macros.rs:1436
(
    connector: $connector: ident,
    flow: PayoutCreateRecipient,
    generic_type: $generic_type:tt,
    [ $($bounds:tt)* ]
) => {
    impl<$generic_type: $($bounds)*> ::interfaces::connector_types::PayoutCreateRecipientV2 for $connector<$generic_type> {}
    impl<$generic_type: $($bounds)*>
        ::interfaces::connector_integration_v2::ConnectorIntegrationV2<
            ::domain_types::connector_flow::PayoutCreateRecipient,
            ::domain_types::payouts::payouts_types::PayoutFlowData,
            ::domain_types::payouts::payouts_types::PayoutCreateRecipientRequest,
            ::domain_types::payouts::payouts_types::PayoutCreateRecipientResponse,
        > for $connector<$generic_type>
    {}
};
```

To move from stub to full, add a concrete `impl ConnectorIntegrationV2<PayoutCreateRecipient, ...> for <Connector><T>` block, and remove `PayoutCreateRecipient` from the `payout_flows:` list to avoid the duplicate-impl compile error. Reference shape: itaubank's `PayoutTransfer` at `crates/integrations/connector-integration/src/connectors/itaubank.rs:294-424`.

### Recipient-Type Branching Pattern

Connectors whose KYC endpoints differ by recipient kind (Individual KYC1 vs Business KYC2) should branch on `req.request.recipient_type`:

```rust
// Reference — branch style mirrors itaubank's tax-id branch at
// crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:183-189
match req.request.recipient_type {
    common_enums::PayoutRecipientType::Individual
    | common_enums::PayoutRecipientType::Personal
    | common_enums::PayoutRecipientType::NaturalPerson => {
        // build individual KYC body
    }
    common_enums::PayoutRecipientType::Company
    | common_enums::PayoutRecipientType::Business => {
        // build business KYC body
    }
    common_enums::PayoutRecipientType::NonProfit
    | common_enums::PayoutRecipientType::PublicSector => {
        // build nonprofit/public KYC body
    }
}
```

All seven `PayoutRecipientType` variants must be matched (no wildcard catchalls) per §11 of `PATTERN_AUTHORING_SPEC.md` — silent omission of enum variants is banned. Variants enumerated at `crates/common/common_enums/src/enums.rs:1191-1203`.

## Connector-Specific Patterns

### itaubank

- itaubank includes `PayoutCreateRecipient` in its `payout_flows:` list at `crates/integrations/connector-integration/src/connectors/itaubank.rs:63`. Itau SiSPAG identifies beneficiaries per-transfer (via `documento` on `ItaubankRecebedor` at `crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:134-141`) and has no separate recipient-onboarding endpoint, so the flow is registered as a stub only. The transformers file contains no `PayoutCreateRecipientRequest`/`PayoutCreateRecipientResponse` `TryFrom` blocks.

No other connector in `crates/integrations/connector-integration/src/connectors/` registers `PayoutCreateRecipientV2`.

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
// From crates/types-traits/interfaces/src/connector_types.rs:707
pub trait PayoutCreateRecipientV2:
    ConnectorIntegrationV2<
    connector_flow::PayoutCreateRecipient,
    PayoutFlowData,
    PayoutCreateRecipientRequest,
    PayoutCreateRecipientResponse,
>
{
}
```

### 3. PayoutRecipientType enum (must be exhaustively matched)

```rust
// From crates/common/common_enums/src/enums.rs:1191
pub enum PayoutRecipientType {
    /// Adyen
    #[default]
    Individual,
    Company,
    NonProfit,
    PublicSector,
    NaturalPerson,

    /// Wise
    Business,
    Personal,
}
```

### 4. Reference implementation shape (adapted)

```rust
// Adapted shape — see crates/integrations/connector-integration/src/connectors/itaubank.rs:294-424
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PayoutCreateRecipient,
        PayoutFlowData,
        PayoutCreateRecipientRequest,
        PayoutCreateRecipientResponse,
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
        req: &RouterDataV2<PayoutCreateRecipient, PayoutFlowData, PayoutCreateRecipientRequest, PayoutCreateRecipientResponse>,
    ) -> CustomResult<String, IntegrationError> {
        let base_url = self.base_url(&req.resource_common_data.connectors);
        Ok(format!("{base_url}/v1/recipients"))
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<PayoutCreateRecipient, PayoutFlowData, PayoutCreateRecipientRequest, PayoutCreateRecipientResponse>,
    ) -> CustomResult<Option<RequestContent>, IntegrationError> {
        let connector_req = <ConnectorRecipientRequest>::try_from(req)?;
        Ok(Some(RequestContent::Json(Box::new(connector_req))))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<PayoutCreateRecipient, PayoutFlowData, PayoutCreateRecipientRequest, PayoutCreateRecipientResponse>,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<PayoutCreateRecipient, PayoutFlowData, PayoutCreateRecipientRequest, PayoutCreateRecipientResponse>,
        ConnectorResponseTransformationError,
    > {
        // Parse recipient-id response; map connector KYC state to PayoutStatus.
        todo!("connector-specific recipient-response parsing")
    }
}
```

### 5. payout_method_data enum (must be exhaustively matched in branches)

```rust
// From crates/types-traits/domain_types/src/payouts/payout_method_data.rs:6
pub enum PayoutMethodData {
    Card(CardPayout),
    Bank(Bank),
    Wallet(Wallet),
    BankRedirect(BankRedirect),
    Passthrough(Passthrough),
}
```

## Integration Guidelines

1. Confirm the connector exposes a recipient-onboarding endpoint. If recipient identity is per-transfer (as with itaubank), leave this flow as a registered stub and implement beneficiary data on `PayoutCreate`/`PayoutTransfer` instead.
2. Add `PayoutCreateRecipient` to the `payout_flows:` list in `<connector>.rs`.
3. Write the concrete `impl ConnectorIntegrationV2<PayoutCreateRecipient, ...>` block and remove `PayoutCreateRecipient` from the `payout_flows:` list.
4. In `<connector>/transformers.rs`:
   - Add a `TryFrom<&RouterDataV2<PayoutCreateRecipient, ...>>` impl that produces the connector's recipient-create struct.
   - Branch on `req.request.recipient_type` exhaustively — all seven variants of `PayoutRecipientType` at `crates/common/common_enums/src/enums.rs:1191-1203` must be handled.
   - Branch on `req.request.payout_method_data` exhaustively over `PayoutMethodData` variants at `crates/types-traits/domain_types/src/payouts/payout_method_data.rs:7-13` — `Card`, `Bank`, `Wallet`, `BankRedirect`, `Passthrough`. Emit `IntegrationError::FeatureNotSupported` for any variant the connector does not accept.
5. Add response parsing that lifts the connector's recipient id into `connector_payout_id` and maps KYC state into `PayoutStatus`.
6. Ignore `amount` and `source_currency` on the request when the connector does not require them for recipient creation. These fields exist on the request type (lines 190-191 of payouts_types.rs) for envelope consistency but are not semantically meaningful to every connector.
7. Write unit tests covering each supported `recipient_type × payout_method_data` combination.
8. Document the downstream mapping: the returned `connector_payout_id` from this flow becomes `connector_payout_method_id` on subsequent `PayoutCreate` calls — see field at `crates/types-traits/domain_types/src/payouts/payouts_types.rs:85`.

## Best Practices

- Exhaustively match every `PayoutRecipientType` variant listed at `crates/common/common_enums/src/enums.rs:1191-1203`. Wildcards are banned by §11 of `PATTERN_AUTHORING_SPEC.md`.
- Exhaustively match every `PayoutMethodData` variant at `crates/types-traits/domain_types/src/payouts/payout_method_data.rs:7-13`. For unsupported rails, emit `IntegrationError::FeatureNotSupported` with a message that names the rail.
- Map KYC-pending connector responses to `PayoutStatus::RequiresVendorAccountCreation` (variant at `crates/common/common_enums/src/enums.rs:1148`). Do NOT map to `Success` until the connector has verified.
- Return the connector's recipient id in `connector_payout_id` so the router can persist it. Downstream `PayoutCreate` consumes it via `connector_payout_method_id` at `crates/types-traits/domain_types/src/payouts/payouts_types.rs:85`.
- See sibling flow [pattern_payout_enroll_disburse_account.md](./pattern_payout_enroll_disburse_account.md) for the follow-on step of attaching a bank account to the newly-created recipient.

## Common Errors / Gotchas

1. **Problem:** `PayoutCreateRecipientResponse.payout_status = PayoutStatus::Success` right after recipient creation.
   **Solution:** Most KYC rails return a pending status first. Map to `PayoutStatus::RequiresVendorAccountCreation` (variant at `crates/common/common_enums/src/enums.rs:1148`) and only promote to `Success` after the connector confirms. Read the connector's status enum, don't guess.

2. **Problem:** Rust compile error on missing arm when matching `PayoutRecipientType`.
   **Solution:** Seven variants. Enumerated at `crates/common/common_enums/src/enums.rs:1191-1203`. Authors MUST list all of them explicitly. Do not use a wildcard.

3. **Problem:** `payout_method_data = None` at runtime and the connector requires an account.
   **Solution:** Emit `IntegrationError::MissingRequiredField { field_name: "payout_method_data", .. }`. `IntegrationError` enum at `crates/types-traits/domain_types/src/errors.rs:168` onward.

4. **Problem:** Compile error "conflicting implementations of trait `ConnectorIntegrationV2<PayoutCreateRecipient, ...>`".
   **Solution:** Remove `PayoutCreateRecipient` from the `payout_flows:` list when writing the full impl. Macro expansion at `crates/integrations/connector-integration/src/connectors/macros.rs:1266-1319`.

5. **Problem:** Recipient created successfully but downstream `PayoutCreate` sends a fresh beneficiary body and the connector errors "duplicate recipient".
   **Solution:** Router-level mapping: the `connector_payout_id` returned here must be plumbed into `PayoutCreateRequest.connector_payout_method_id` at `crates/types-traits/domain_types/src/payouts/payouts_types.rs:85`. When `connector_payout_method_id` is set, downstream create transformers should omit inline beneficiary details and use the reference instead.

## Testing Notes

### Unit Tests

Each connector implementing PayoutCreateRecipient should cover:

- `TryFrom<&RouterDataV2<PayoutCreateRecipient, ...>>` for each supported `recipient_type` variant — assert the KYC body is shaped correctly.
- `TryFrom<&RouterDataV2<PayoutCreateRecipient, ...>>` for each supported `payout_method_data` variant.
- Response parsing: KYC-pending → `PayoutStatus::RequiresVendorAccountCreation`; KYC-complete → `PayoutStatus::Success`; KYC-failed → `PayoutStatus::Failure`.
- `payout_method_data = None` → request-time error.

### Integration Scenarios

| Scenario | Inputs | Expected `payout_status` | Expected `status_code` |
| --- | --- | --- | --- |
| Individual recipient with ACH bank | Individual, Bank(Ach) | `RequiresVendorAccountCreation` | 201 |
| Business recipient with SEPA | Business, Bank(Sepa) | `RequiresVendorAccountCreation` | 201 |
| NonProfit recipient with Passthrough token | NonProfit, Passthrough | `RequiresVendorAccountCreation` | 201 |
| Individual with no payout_method_data | Individual, None | — (error) | 4xx |
| KYC rejected | Individual, Bank(Ach), bad SSN | `Failure` | 4xx |

No connector in connector-service exercises these scenarios at the pinned SHA.

## Cross-References

- Parent index: [../README.md](./README.md)
- Sibling core payout flow: [pattern_payout_create.md](./pattern_payout_create.md)
- Sibling core payout flow: [pattern_payout_transfer.md](./pattern_payout_transfer.md)
- Sibling core payout flow: [pattern_payout_get.md](./pattern_payout_get.md)
- Sibling side-flow: [pattern_payout_enroll_disburse_account.md](./pattern_payout_enroll_disburse_account.md)
- Sibling side-flow: [pattern_payout_create_link.md](./pattern_payout_create_link.md)
- Sibling side-flow: [pattern_payout_void.md](./pattern_payout_void.md)
- Macro reference: [macro_patterns_reference.md](./macro_patterns_reference.md)
- Utility helpers: [utility_functions_reference.md](../utility_functions_reference.md)
- Authoring spec: [PATTERN_AUTHORING_SPEC.md](./PATTERN_AUTHORING_SPEC.md)
