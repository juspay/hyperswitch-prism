# Payout Enroll-Disburse-Account Flow Pattern

## Overview

The Payout Enroll-Disburse-Account flow registers a specific disbursement account (bank account, wallet, or card destination) with the connector, typically after a recipient has already been created via `PayoutCreateRecipient`. Some connectors expose this as a two-step onboarding: (1) create recipient profile with KYC, (2) enroll a payout destination. This flow is the second step. For connectors that fold the two into a single API call, expose only `PayoutCreateRecipient` and leave this flow registered as a stub.

The flow's outcome is a stable account identifier that downstream payout flows reference via `connector_payout_method_id`. Unlike `PayoutCreateRecipient`, this flow does not carry a `recipient_type` field — the recipient is assumed already registered and is addressed implicitly through the request's `merchant_payout_id` or via the connector's internal session state.

### Key Components

- Flow marker: `PayoutEnrollDisburseAccount` — `crates/types-traits/domain_types/src/connector_flow.rs:95`.
- Request type: `PayoutEnrollDisburseAccountRequest` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:205`.
- Response type: `PayoutEnrollDisburseAccountResponse` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:213`.
- Flow-data type: `PayoutFlowData` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:13`.
- Marker trait: `PayoutEnrollDisburseAccountV2` — `crates/types-traits/interfaces/src/connector_types.rs:717`.
- Macro arm: `expand_payout_implementation!` `PayoutEnrollDisburseAccount` arm — `crates/integrations/connector-integration/src/connectors/macros.rs:1452-1467`.

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

Payout Enroll-Disburse-Account is the producer of the `connector_payout_method_id` that downstream payout flows consume. It runs once per (recipient, account) pair and produces a stable handle so merchants do not re-submit account details on every disbursement.

### Flow Hierarchy

```
PayoutCreateRecipient  (upstream — produces recipient id)
        |
        v
PayoutEnrollDisburseAccount  (this flow — produces connector_payout_method_id)
        |
        v
PayoutCreate / PayoutTransfer  (downstream — reference account via connector_payout_method_id)
        |
        v
PayoutGet
```

### Flow Type

`PayoutEnrollDisburseAccount` — zero-sized marker struct declared at `crates/types-traits/domain_types/src/connector_flow.rs:95`. Registered in `FlowName::PayoutEnrollDisburseAccount` at `crates/types-traits/domain_types/src/connector_flow.rs:132`.

### Request Type

`PayoutEnrollDisburseAccountRequest` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:205-210`:

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs:204
#[derive(Debug, Clone)]
pub struct PayoutEnrollDisburseAccountRequest {
    pub merchant_payout_id: Option<String>,
    pub amount: common_utils::types::MinorUnit,
    pub source_currency: common_enums::Currency,
    pub payout_method_data: Option<PayoutMethodData>,
}
```

Notable fields:

- `payout_method_data: Option<PayoutMethodData>` — the enum at `crates/types-traits/domain_types/src/payouts/payout_method_data.rs:7-13` carrying the concrete account shape (`Card`, `Bank`, `Wallet`, `BankRedirect`, `Passthrough`). This is the payload the connector validates and enrolls.
- No `recipient_type` — the enrollment is account-level, not profile-level.
- `amount` and `source_currency` exist for envelope consistency but most connector enroll APIs do not use them.

### Response Type

`PayoutEnrollDisburseAccountResponse` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:213-218`:

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs:212
#[derive(Debug, Clone)]
pub struct PayoutEnrollDisburseAccountResponse {
    pub merchant_payout_id: Option<String>,
    pub payout_status: common_enums::PayoutStatus,
    pub connector_payout_id: Option<String>,
    pub status_code: u16,
}
```

The enrolled account id is returned in `connector_payout_id` (`Option<String>` at line 216). `payout_status` mapping follows the same scheme as `PayoutCreateRecipient`:

- `PayoutStatus::RequiresVendorAccountCreation` — enrollment submitted, connector-side verification pending (variant at `crates/common/common_enums/src/enums.rs:1148`).
- `PayoutStatus::Success` — account enrolled and ready for disbursement.
- `PayoutStatus::Failure` — enrollment rejected.

### Resource Common Data

`PayoutFlowData` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:13-23`. See [pattern_payout_void.md](./pattern_payout_void.md) for the full breakdown.

### RouterDataV2 Shape

```rust
RouterDataV2<PayoutEnrollDisburseAccount, PayoutFlowData, PayoutEnrollDisburseAccountRequest, PayoutEnrollDisburseAccountResponse>
```

Canonical four-arg shape per §7 of `PATTERN_AUTHORING_SPEC.md`.

## Connectors with Full Implementation

At the pinned SHA, **no connector supplies a non-stub `ConnectorIntegrationV2<PayoutEnrollDisburseAccount, ...>` implementation.** A grep across `crates/integrations/connector-integration/src/connectors/` for `ConnectorIntegrationV2<\s*PayoutEnrollDisburseAccount` returns zero matches. The only connector registering the `PayoutEnrollDisburseAccountV2` marker is **itaubank** at `crates/integrations/connector-integration/src/connectors/itaubank.rs:64` via the macro at `crates/integrations/connector-integration/src/connectors/macros.rs:1452-1467`.

Current implementation coverage: **0 connectors** (itaubank registers the marker trait only; no URL/headers/body/response-parsing are provided).

| Connector | HTTP Method | Content Type | URL Pattern | Request Type Reuse | Notes |
| --- | --- | --- | --- | --- | --- |
| _(none)_ | — | — | — | — | See Stub Implementations below. |

### Stub Implementations

- **itaubank** — macro-registered stub. The `PayoutEnrollDisburseAccount` arm of `expand_payout_implementation!` (`crates/integrations/connector-integration/src/connectors/macros.rs:1452-1467`) emits `impl PayoutEnrollDisburseAccountV2 for Itaubank<T> {}` and `impl ConnectorIntegrationV2<PayoutEnrollDisburseAccount, PayoutFlowData, PayoutEnrollDisburseAccountRequest, PayoutEnrollDisburseAccountResponse> for Itaubank<T> {}` with empty bodies.

## Common Implementation Patterns

### Macro-Based Pattern (Recommended)

Register by listing `PayoutEnrollDisburseAccount` in `payout_flows:`:

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
        PayoutCreateRecipient,
        PayoutEnrollDisburseAccount   // <-- registers marker + empty integration impl
    ]
);
```

The `PayoutEnrollDisburseAccount` arm at `crates/integrations/connector-integration/src/connectors/macros.rs:1452-1467` produces:

```rust
// From crates/integrations/connector-integration/src/connectors/macros.rs:1452
(
    connector: $connector: ident,
    flow: PayoutEnrollDisburseAccount,
    generic_type: $generic_type:tt,
    [ $($bounds:tt)* ]
) => {
    impl<$generic_type: $($bounds)*> ::interfaces::connector_types::PayoutEnrollDisburseAccountV2 for $connector<$generic_type> {}
    impl<$generic_type: $($bounds)*>
        ::interfaces::connector_integration_v2::ConnectorIntegrationV2<
            ::domain_types::connector_flow::PayoutEnrollDisburseAccount,
            ::domain_types::payouts::payouts_types::PayoutFlowData,
            ::domain_types::payouts::payouts_types::PayoutEnrollDisburseAccountRequest,
            ::domain_types::payouts::payouts_types::PayoutEnrollDisburseAccountResponse,
        > for $connector<$generic_type>
    {}
};
```

To move from stub to full, add a concrete `impl ConnectorIntegrationV2<PayoutEnrollDisburseAccount, ...> for <Connector><T>` and remove `PayoutEnrollDisburseAccount` from the `payout_flows:` list. Reference shape: itaubank's `PayoutTransfer` at `crates/integrations/connector-integration/src/connectors/itaubank.rs:294-424`.

### Payout-Method-Data Branching Pattern

The request's `payout_method_data` must be matched exhaustively over `PayoutMethodData` variants. Itaubank's `PayoutTransfer` transformer demonstrates the branch shape at `crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:175-211` for one variant (`Bank::Pix`) with an explicit fallback arm. For enroll-disburse-account, authors MUST explicitly list every variant rather than relying on a wildcard:

```rust
// Reference structure — mirrors itaubank's Bank::Pix branch at
// crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:176
match req.request.payout_method_data.clone() {
    Some(PayoutMethodData::Bank(Bank::Ach(ach))) => {
        // build ACH enroll body from ach.bank_account_number, ach.bank_routing_number
    }
    Some(PayoutMethodData::Bank(Bank::Bacs(bacs))) => { /* BACS */ }
    Some(PayoutMethodData::Bank(Bank::Sepa(sepa))) => { /* SEPA */ }
    Some(PayoutMethodData::Bank(Bank::Pix(pix))) => { /* PIX */ }
    Some(PayoutMethodData::Card(card)) => { /* card destination */ }
    Some(PayoutMethodData::Wallet(wallet)) => { /* wallet destination */ }
    Some(PayoutMethodData::BankRedirect(br)) => { /* BankRedirect destination */ }
    Some(PayoutMethodData::Passthrough(token)) => { /* passthrough PSP token */ }
    None => {
        return Err(IntegrationError::MissingRequiredField {
            field_name: "payout_method_data",
            context: Default::default(),
        }.into());
    }
}
```

The `Bank` enum has four variants — `Ach`, `Bacs`, `Sepa`, `Pix` — enumerated at `crates/types-traits/domain_types/src/payouts/payout_method_data.rs:40-45`. The outer `PayoutMethodData` has five variants enumerated at `crates/types-traits/domain_types/src/payouts/payout_method_data.rs:7-13`. All must be listed.

## Connector-Specific Patterns

### itaubank

- itaubank includes `PayoutEnrollDisburseAccount` in its `payout_flows:` list at `crates/integrations/connector-integration/src/connectors/itaubank.rs:64`. Itau SiSPAG accepts beneficiary account details inline on every transfer (see `ItaubankRecebedor` at `crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:133-141`) and has no separate account-enrollment endpoint, so this flow is registered as a stub only. The transformers file contains no `PayoutEnrollDisburseAccountRequest`/`PayoutEnrollDisburseAccountResponse` `TryFrom` blocks.

No other connector in `crates/integrations/connector-integration/src/connectors/` registers `PayoutEnrollDisburseAccountV2`.

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
// From crates/types-traits/interfaces/src/connector_types.rs:717
pub trait PayoutEnrollDisburseAccountV2:
    ConnectorIntegrationV2<
    connector_flow::PayoutEnrollDisburseAccount,
    PayoutFlowData,
    PayoutEnrollDisburseAccountRequest,
    PayoutEnrollDisburseAccountResponse,
>
{
}
```

### 3. PayoutMethodData and Bank enums (must be exhaustively matched)

```rust
// From crates/types-traits/domain_types/src/payouts/payout_method_data.rs:6
pub enum PayoutMethodData {
    Card(CardPayout),
    Bank(Bank),
    Wallet(Wallet),
    BankRedirect(BankRedirect),
    Passthrough(Passthrough),
}

// From crates/types-traits/domain_types/src/payouts/payout_method_data.rs:39
pub enum Bank {
    Ach(AchBankTransfer),
    Bacs(BacsBankTransfer),
    Sepa(SepaBankTransfer),
    Pix(PixBankTransfer),
}
```

### 4. Reference implementation shape (adapted from `PayoutTransfer`)

```rust
// Adapted shape — see crates/integrations/connector-integration/src/connectors/itaubank.rs:294-424
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PayoutEnrollDisburseAccount,
        PayoutFlowData,
        PayoutEnrollDisburseAccountRequest,
        PayoutEnrollDisburseAccountResponse,
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
        req: &RouterDataV2<PayoutEnrollDisburseAccount, PayoutFlowData, PayoutEnrollDisburseAccountRequest, PayoutEnrollDisburseAccountResponse>,
    ) -> CustomResult<String, IntegrationError> {
        let base_url = self.base_url(&req.resource_common_data.connectors);
        Ok(format!("{base_url}/v1/recipients/accounts"))
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<PayoutEnrollDisburseAccount, PayoutFlowData, PayoutEnrollDisburseAccountRequest, PayoutEnrollDisburseAccountResponse>,
    ) -> CustomResult<Option<RequestContent>, IntegrationError> {
        let connector_req = <ConnectorEnrollAccountRequest>::try_from(req)?;
        Ok(Some(RequestContent::Json(Box::new(connector_req))))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<PayoutEnrollDisburseAccount, PayoutFlowData, PayoutEnrollDisburseAccountRequest, PayoutEnrollDisburseAccountResponse>,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<PayoutEnrollDisburseAccount, PayoutFlowData, PayoutEnrollDisburseAccountRequest, PayoutEnrollDisburseAccountResponse>,
        ConnectorResponseTransformationError,
    > {
        // Parse account-id response; map to PayoutStatus::RequiresVendorAccountCreation (pending)
        // or Success (instant verify), never hardcoded.
        todo!("connector-specific enroll-response parsing")
    }
}
```

### 5. itaubank's PIX branch (shows a real-world branching pattern to mirror)

```rust
// From crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:176
let recebedor = match req.request.payout_method_data.clone() {
    Some(PayoutMethodData::Bank(Bank::Pix(PixBankTransfer {
        tax_id,
        bank_branch,
        bank_account_number,
        bank_name,
        ..
    }))) => {
        // ... build ItaubankRecebedor
        Some(ItaubankRecebedor { /* ... */ })
    }
    _ => None,
};
```

Note: itaubank falls through with `_ => None` for the `PayoutTransfer` flow because the rail is PIX-only. For `PayoutEnrollDisburseAccount` on a multi-rail connector, the `_` arm is inappropriate — authors MUST list every variant explicitly (see §Payout-Method-Data Branching Pattern above).

## Integration Guidelines

1. Confirm the connector exposes a two-step onboarding (create-recipient + enroll-account). If the connector folds these into one endpoint, implement `PayoutCreateRecipient` only and leave this flow as a stub.
2. Add `PayoutEnrollDisburseAccount` to the `payout_flows:` list in `<connector>.rs`.
3. Write the concrete `impl ConnectorIntegrationV2<PayoutEnrollDisburseAccount, ...>` block and remove the flow from `payout_flows:`.
4. In `<connector>/transformers.rs`:
   - Add a `TryFrom<&RouterDataV2<PayoutEnrollDisburseAccount, ...>>` impl.
   - Match every variant of `PayoutMethodData` at `crates/types-traits/domain_types/src/payouts/payout_method_data.rs:7-13` — five variants, no wildcards.
   - Within the `Bank` arm, match every variant of `Bank` at `crates/types-traits/domain_types/src/payouts/payout_method_data.rs:40-45` — four variants, no wildcards.
   - For any unsupported variant, emit `IntegrationError::FeatureNotSupported` naming the rail.
5. Add response parsing that lifts the enrolled account id into `connector_payout_id` and maps the connector's verification state to `PayoutStatus::RequiresVendorAccountCreation` (pending) or `PayoutStatus::Success` (ready).
6. Propagate the enrolled account id back to the router so subsequent `PayoutCreate`/`PayoutTransfer` calls can reference it via `connector_payout_method_id` at `crates/types-traits/domain_types/src/payouts/payouts_types.rs:85`.
7. Write unit tests for each supported `payout_method_data` variant.
8. Write an integration test chain: `PayoutCreateRecipient` → `PayoutEnrollDisburseAccount` → `PayoutCreate`.

## Best Practices

- Match every variant of `PayoutMethodData` (enum at `crates/types-traits/domain_types/src/payouts/payout_method_data.rs:7-13`) and every variant of `Bank` (enum at `crates/types-traits/domain_types/src/payouts/payout_method_data.rs:40-45`). Use explicit arms, not wildcards, per §11 of `PATTERN_AUTHORING_SPEC.md`.
- When the connector's enrollment is asynchronous, map to `PayoutStatus::RequiresVendorAccountCreation` (variant at `crates/common/common_enums/src/enums.rs:1148`). Only use `Success` if the connector explicitly returns a verified state.
- Lift the enrolled account id into `connector_payout_id`. Downstream `PayoutCreate` consumes it via `PayoutCreateRequest.connector_payout_method_id` at `crates/types-traits/domain_types/src/payouts/payouts_types.rs:85`.
- Reuse `build_error_response` exactly as itaubank does at `crates/integrations/connector-integration/src/connectors/itaubank.rs:95-137` for connector-side enrollment errors (e.g. "invalid routing number").
- See upstream pattern [pattern_payout_create_recipient.md](./pattern_payout_create_recipient.md) for the recipient-profile step that must precede this flow in most connector APIs.

## Common Errors / Gotchas

1. **Problem:** Rust compile error "non-exhaustive patterns: `Some(PayoutMethodData::Wallet(_))` not covered" when matching `payout_method_data`.
   **Solution:** Five top-level variants — `Card`, `Bank`, `Wallet`, `BankRedirect`, `Passthrough` — at `crates/types-traits/domain_types/src/payouts/payout_method_data.rs:7-13`. Additionally the `Bank` arm has four sub-variants at lines 40-45. Enumerate all explicitly.

2. **Problem:** Wildcard `_ => None` silently drops a supported rail on refactor.
   **Solution:** Do not use wildcards in `payout_method_data` branches for this flow. The itaubank `PayoutTransfer` wildcard at `crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:210` is acceptable for that connector because Itau is a single-rail integration; a multi-rail enroll flow MUST NOT copy that pattern.

3. **Problem:** Enrolled account returns `connector_payout_id = None` because the transformer forgot to lift the id.
   **Solution:** The field is `Option<String>` at `crates/types-traits/domain_types/src/payouts/payouts_types.rs:216` and is the primary handle the router persists. Returning `None` on a success path silently breaks downstream `PayoutCreate`.

4. **Problem:** `payout_status = PayoutStatus::Success` for an async-verification connector.
   **Solution:** Map to `PayoutStatus::RequiresVendorAccountCreation` at `crates/common/common_enums/src/enums.rs:1148` when the connector indicates pending verification. Promote to `Success` only via subsequent webhook or `PayoutGet` polling.

5. **Problem:** Compile error "conflicting implementations of trait `ConnectorIntegrationV2<PayoutEnrollDisburseAccount, ...>`".
   **Solution:** Remove `PayoutEnrollDisburseAccount` from the `payout_flows:` macro list when writing the full impl. Macro recursion at `crates/integrations/connector-integration/src/connectors/macros.rs:1266-1319`.

6. **Problem:** Enrollment succeeded but downstream `PayoutCreate` includes inline beneficiary details AND `connector_payout_method_id`, causing the connector to reject with "duplicate account details".
   **Solution:** In `PayoutCreate`/`PayoutTransfer` transformers, when `connector_payout_method_id` is `Some`, omit inline beneficiary serialization and reference the id only. See the downstream request field at `crates/types-traits/domain_types/src/payouts/payouts_types.rs:85`.

## Testing Notes

### Unit Tests

Each connector implementing PayoutEnrollDisburseAccount should cover:

- `TryFrom<&RouterDataV2<PayoutEnrollDisburseAccount, ...>>` for each supported `PayoutMethodData` variant the connector accepts (Card, Bank::Ach, Bank::Sepa, Bank::Pix, Wallet, Passthrough as applicable).
- Each unsupported variant — expect `IntegrationError::FeatureNotSupported` with a clear field name.
- `payout_method_data = None` — expect `IntegrationError::MissingRequiredField { field_name: "payout_method_data", .. }`.
- Response parsing: pending-verification → `PayoutStatus::RequiresVendorAccountCreation`; instant-verified → `PayoutStatus::Success`; rejected → `PayoutStatus::Failure`.

### Integration Scenarios

| Scenario | Inputs | Expected `payout_status` | Expected `status_code` |
| --- | --- | --- | --- |
| Enroll ACH bank account | Bank(Ach) with valid routing | `RequiresVendorAccountCreation` | 201 |
| Enroll SEPA account | Bank(Sepa) with valid IBAN | `RequiresVendorAccountCreation` | 201 |
| Enroll wallet | Wallet(Paypal { email: ... }) | `RequiresVendorAccountCreation` | 201 |
| Enroll with no payout_method_data | None | — (request-time error) | N/A |
| Invalid routing number | Bank(Ach) with 8-digit routing | `Failure` | 4xx |
| Enroll → PayoutCreate chain | chained | `Success` on create | 200 |

No connector in connector-service exercises these scenarios at the pinned SHA.

## Cross-References

- Parent index: [../README.md](./README.md)
- Sibling core payout flow: [pattern_payout_create.md](./pattern_payout_create.md)
- Sibling core payout flow: [pattern_payout_transfer.md](./pattern_payout_transfer.md)
- Sibling core payout flow: [pattern_payout_get.md](./pattern_payout_get.md)
- Sibling side-flow: [pattern_payout_create_recipient.md](./pattern_payout_create_recipient.md) — immediate upstream
- Sibling side-flow: [pattern_payout_create_link.md](./pattern_payout_create_link.md)
- Sibling side-flow: [pattern_payout_void.md](./pattern_payout_void.md)
- Sibling side-flow: [pattern_payout_stage.md](./pattern_payout_stage.md)
- Macro reference: [macro_patterns_reference.md](./macro_patterns_reference.md)
- Utility helpers: [utility_functions_reference.md](../utility_functions_reference.md)
- Authoring spec: [PATTERN_AUTHORING_SPEC.md](./PATTERN_AUTHORING_SPEC.md)
