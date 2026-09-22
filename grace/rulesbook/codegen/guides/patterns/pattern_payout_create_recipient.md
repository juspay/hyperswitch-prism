# Payout Create-Recipient Flow Pattern

## Overview

The Payout Create-Recipient flow registers a beneficiary (recipient) with the connector so subsequent payouts can reference a stable recipient id instead of re-submitting full bank / wallet details on every transfer. This flow is the onboarding step for connectors that distinguish "create recipient" (KYC/verification) from "disburse to recipient" (fund movement). Once created, the connector-assigned recipient id is returned and typically threaded into `PayoutCreateRequest.connector_payout_method_id` or a similar beneficiary-reference field on downstream flows.

This flow is distinct from `PayoutEnrollDisburseAccount`: create-recipient produces a long-lived recipient profile (one-time KYC); enroll-disburse-account attaches a specific bank account to an already-existing recipient. Some connectors fold the two into a single API call; when that happens, expose only one of the two flows and document the choice.

### Key Components

- Flow marker: `PayoutCreateRecipient` — `crates/types-traits/domain_types/src/connector_flow.rs`.
- Request type: `PayoutCreateRecipientRequest` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`.
- Response type: `PayoutCreateRecipientResponse` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`.
- Flow-data type: `PayoutFlowData` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`.
- Marker trait: `PayoutCreateRecipientV2` — `crates/types-traits/interfaces/src/connector_types.rs`, defined solely as the supertrait binding:

  ```rust
  // crates/types-traits/interfaces/src/connector_types.rs — pub trait PayoutCreateRecipientV2
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

- Service trait: `PayoutServiceTrait` — `crates/types-traits/interfaces/src/connector_types.rs`.
- Stub macro arm: the `flow: PayoutCreateRecipient` arm of `expand_payout_implementation!` — `crates/integrations/connector-integration/src/connectors/macros.rs`.
- Reference implementation: `crates/integrations/connector-integration/src/payout_connectors/trustly.rs` (the `macros::macro_connector_implementation!` call under the `// ===== PAYOUT CREATE RECIPIENT (RegisterAccount) =====` banner), with transformers in `crates/integrations/connector-integration/src/payout_connectors/trustly/transformers.rs`.

### Payout connectors live in a sibling registry — and are outside certification

Payout connectors are **not** `connectors/` files. They live in
`crates/integrations/connector-integration/src/payout_connectors/`, a sibling directory of
`connectors/`, and are exported from `payout_connectors.rs` — the module file, a sibling of the `payout_connectors/` directory — as
`pub mod <name>; pub use self::<name>::<Name>Payouts;`. The ten at HEAD are
`cybersource`, `deutschebank`, `gotyme_sanlam`, `itaubank`, `loonio`, `paypal`, `santander`,
`truelayer`, `trustly`, `worldpayxml`.

The service trait is `interfaces::connector_types::PayoutServiceTrait`
(`crates/types-traits/interfaces/src/connector_types.rs`). Read its supertrait list before
assuming payment-side obligations carry over — it is
`ConnectorCommon + ServerAuthentication + PayoutCreateV2 + PayoutTransferV2 + PayoutGetV2 +
PayoutVoidV2 + PayoutStageV2 + PayoutCreateLinkV2 + PayoutCreateRecipientV2 +
PayoutEnrollDisburseAccountV2 + PayoutEligibilityV2` and **nothing else**. In particular it does
**not** require `ValidationTrait`, `IncomingWebhook`, `VerifyRedirectResponse`, `SourceVerification`
or `BodyDecoding`. Do not write those impls for a payout-only connector.

**Registration sites** (all six, in order):

1. `payout_connectors/<name>.rs` and `payout_connectors/<name>/transformers.rs`.
2. `payout_connectors.rs` — the module file, a sibling of the `payout_connectors/` directory: `pub mod <name>; pub use self::<name>::<Name>Payouts;`.
3. `crates/types-traits/domain_types/src/connector_types.rs` — a variant on `pub enum PayoutConnectorEnum`.
4. `crates/integrations/connector-integration/src/types.rs` — an arm in `PayoutConnectorData::convert_connector`.
5. `crates/types-traits/domain_types/src/types.rs` — a field on `pub struct Connectors` (`ConnectorParams`, or `ConnectorParamsWithCaBundle` if the connector needs a CA bundle, as `deutschebank` does) plus the matching `PayoutConnectorEnum` arm in `patch_payout_connector_urls`.
6. `impl PayoutServiceTrait for <Name>Payouts {}` in the connector file.

**No `config/superposition.toml` entry and no `connector_specs/<connector>/specs.json` entry are
required.** Three independent reasons, all in
`crates/internal/integration-tests/src/bin/check_connector_specs.rs`:

1. `main()` enumerates connectors by reading
   `crates/integrations/connector-integration/src/connectors/` only — a connector under
   `payout_connectors/` is never seen, so it is never asked for a spec directory. This is also why
   the merge-blocking `.github/scripts/verify-new-connectors.sh` gate does not fire for one.
2. `const IGNORE_SERVICES: &[&str] = &["PayoutService", "DisputeService"];` — declared identically in
   `check_connector_specs.rs` and `check_coverage.rs` — drops any payout suite that does appear.
3. `flow_to_suites()` has no `Payout*` arm, so payout flow names fall to its `_ => None`, and the
   eight names `PayoutCreate`, `PayoutGet`, `PayoutStage`, `PayoutTransfer`, `PayoutVoid`,
   `PayoutEnrollDisburseAccount`, `PayoutCreateRecipient`, `PayoutCreateLink` are listed in
   `OUT_OF_SCOPE_FLOWS` so the "unknown flow" check passes. (`PayoutEligibility` is **not** in that
   list — it is only ever declared under `payout_connectors/`, which reason 1 keeps out of the scan.)

That exemption is about tooling reach, not merit. Read the source comments precisely: the doc comment
*above* `OUT_OF_SCOPE_FLOWS` says only "A flow reaches this list only by decision", and the eight
payout names sit under the in-list marker `// Payouts — out of scope for the payment suites.` The
sharper phrasing — "Each is a coverage gap, not a decision that it should never be covered" — belongs
to the *third* group in that same list (`VoidPC`, `VerifyWebhookSource`, …), not to the payout group.
The framing still applies: payout flows are unlisted because no integration-test suite exists yet, not
because they should never be built. Do not quote the "coverage gap" comment as if it were attached to
the payout entries. The payout-only connectors
`santander`, `deutschebank` and `gotyme_sanlam` have no `config/superposition.toml` block at all;
base URLs reach them through the `Connectors` field added in step 5. (`loonio`, `trustly`,
`truelayer`, `worldpayxml`, `cybersource` and `paypal` *do* appear in `superposition.toml`, but only
because each also has a payment-side connector under `connectors/`.)

### Access tokens are supplied by the caller, not sequenced by a `ValidationTrait` hook

There is no `should_do_access_token` on the payout path — `PayoutServiceTrait` does not require
`ValidationTrait` at all. Instead:

- `PayoutServiceTrait` requires `ServerAuthentication`, whose binding is
  `ConnectorIntegrationV2<ServerAuthenticationToken, MerchantAuthenticationFlowData,
  ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>`. Note the flow data
  is **`MerchantAuthenticationFlowData`** (`crates/types-traits/domain_types/src/merchant_authentication_flow_data.rs`),
  *not* `PayoutFlowData` — see `impl ServerAuthentication for ItaubankPayouts` and the
  `ConnectorIntegrationV2<ServerAuthenticationToken, …>` block that follows it in
  `payout_connectors/itaubank.rs`.
- The caller obtains a token through `MerchantAuthenticationService`, then passes it back on the
  payout RPC: every payout request message in
  `crates/types-traits/grpc-api-types/proto/payouts.proto` carries an
  `optional SecretString access_token`. The `ForeignTryFrom` impls in
  `crates/types-traits/domain_types/src/payouts/types.rs` copy it into
  `PayoutFlowData.access_token`, which `PayoutFlowData::get_access_token()` then exposes to
  `get_headers`.


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

`PayoutCreateRecipient` — zero-sized marker struct declared at `crates/types-traits/domain_types/src/connector_flow.rs`. Registered in `FlowName::PayoutCreateRecipient` at `crates/types-traits/domain_types/src/connector_flow.rs`.

### Request Type

`PayoutCreateRecipientRequest` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`:

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs
#[derive(Debug, Clone)]
pub struct PayoutCreateRecipientRequest {
    pub merchant_payout_id: Option<String>,
    pub amount: common_utils::types::MinorUnit,
    pub source_currency: common_enums::Currency,
    pub payout_method_data: Option<PayoutMethodData>,
    pub recipient_type: common_enums::PayoutRecipientType,
    pub customer: Option<PayoutCustomer>,
    pub address: Option<PayoutAddress>,
}
```

Seven fields, not five. Notable ones:

- `recipient_type: common_enums::PayoutRecipientType` — `pub enum PayoutRecipientType` in
  `crates/common/common_enums/src/enums.rs`, with exactly seven variants:
  `Individual` (the `#[default]`), `Company`, `NonProfit`, `PublicSector`, `NaturalPerson` (Adyen
  taxonomy) and `Business`, `Personal` (Wise taxonomy).
- `payout_method_data: Option<PayoutMethodData>` — `pub enum PayoutMethodData` in
  `crates/types-traits/domain_types/src/payouts/payout_method_data.rs`, with five variants: `Card`,
  `Bank`, `Wallet`, `BankRedirect`, `Passthrough`. The `Bank` arm has **ten** sub-variants —
  `Ach`, `Bacs`, `Sepa`, `Pix`, `PixKey`, `PixEmv`, `OpenBanking`, `Trustly`, `Payshap`,
  `PayshapProxy`. When `None`, the connector must support recipient creation without a bound account
  (rare) or the transformer MUST reject the request.
- `customer: Option<PayoutCustomer>` and `address: Option<PayoutAddress>` — the KYC payload.
  `PayoutCreateRecipientRequest` also carries the accessor
  `get_optional_billing_address(&self) -> Option<&crate::payment_address::AddressDetails>` (declared
  in the same file); use it rather than re-navigating the `Option` chain by hand.

The presence of `amount` and `source_currency` on a recipient-create request is unusual; the typical connector API only requires KYC fields. These fields exist on `PayoutCreateRecipientRequest` because the platform carries them through the `PayoutFlowData` envelope even when the connector does not need them. Transformers SHOULD ignore `amount` in the create-recipient body when the connector does not accept it.

### Response Type

`PayoutCreateRecipientResponse` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`:

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs
#[derive(Debug, Clone)]
pub struct PayoutCreateRecipientResponse {
    pub merchant_payout_id: Option<String>,
    pub payout_status: common_enums::PayoutStatus,
    pub connector_payout_id: Option<String>,
    pub status_code: u16,
    pub payout_connector_metadata: Option<common_utils::pii::SecretSerdeValue>,
}
```

Five fields. **`payout_connector_metadata` is the one that matters for this flow** — it is the
*only* payout response type that carries it, and it is the documented carrier for the connector's
recipient handle. `PayoutTransferRequest` has a field of the same name and type; that is the
hand-off. Grep it: `payout_connector_metadata` occurs exactly twice in
`crates/types-traits/domain_types/src/payouts/payouts_types.rs` — on `PayoutTransferRequest` and on
`PayoutCreateRecipientResponse`. `payout_connector_metadata` is not `connector_metadata` (a
separate, differently-named field that lives only on `PayoutEligibilityResponse`) and not
`encoded_data` (no such field exists on any payout type).

The live example: `RegisterAccountResponse` on Trustly returns an `accountid`, which
`payout_connectors/trustly/transformers.rs` packs as
`Some(Secret::new(serde_json::json!({ "account_id": account_id })))` into
`payout_connector_metadata`, leaving `connector_payout_id: None`. Downstream, `AccountPayoutRequest`
reads it back out of `PayoutTransferRequest.payout_connector_metadata`.

`payout_status` maps to (all variants of `pub enum PayoutStatus` in
`crates/common/common_enums/src/enums.rs`):

- `PayoutStatus::RequiresCreation` — what Trustly returns: the recipient account is registered but
  no payout exists yet.
- `PayoutStatus::RequiresFulfillment` — recipient created, no bound account yet.
- `PayoutStatus::RequiresVendorAccountCreation` — recipient in KYC-pending state.
- `PayoutStatus::Success` — recipient ready for use (rare on first creation).
- `PayoutStatus::Failure` — KYC rejected.

### Resource Common Data

`PayoutFlowData` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`. See [pattern_payout_void.md](./pattern_payout_void.md) for the full breakdown.

### RouterDataV2 Shape

```rust
RouterDataV2<PayoutCreateRecipient, PayoutFlowData, PayoutCreateRecipientRequest, PayoutCreateRecipientResponse>
```

Canonical four-arg shape per §7 of `PATTERN_AUTHORING_SPEC.md`.

## Connectors with Full Implementation

Exactly **one** payout connector implements this flow for real at HEAD: **`TrustlyPayouts<T>`**, in
`crates/integrations/connector-integration/src/payout_connectors/trustly.rs`.

Verify the roster before trusting it:

```bash
rg -n 'impl ConnectorIntegrationV2<PayoutCreateRecipient|flow_name: PayoutCreateRecipient' \
   crates/integrations/connector-integration/src/payout_connectors/
```

Current implementation coverage: **1 of 10 payout connectors.**

| Connector | Style | HTTP | Content type | URL pattern | Request/response types | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| `TrustlyPayouts<T>` | macro | `POST` | `application/json; charset=UTF-8` (`CONTENT_TYPE_JSON`) | the bare base URL — Trustly is JSON-RPC, method in the body | `RegisterAccountRequest` → `RegisterAccountResponse` | Declared via `macros::create_all_prerequisites!` + `macros::macro_connector_implementation!` with `flow_name: PayoutCreateRecipient`, `resource_common_data: PayoutFlowData`. Returns the account handle in `payout_connector_metadata`, not `connector_payout_id`. |

### Stub Implementations

The other nine connectors register `PayoutCreateRecipientV2` with a fail-fast stub whose only method
is a `get_url` returning
`IntegrationError::connector_flow_not_implemented(self.id(), "payout_create_recipient", …)`:

- **Hand-written** (non-generic connectors, which cannot use the payout macro):
  `cybersource.rs`, `itaubank.rs`, `loonio.rs`, `paypal.rs`, `santander.rs`, `worldpayxml.rs`, and
  `truelayer.rs` (via its file-local `macro_rules! impl_unimplemented_payout_flow!`).
- **Macro** (generic connectors): `deutschebank.rs` and `gotyme_sanlam.rs` list
  `PayoutCreateRecipient` in the `payout_flows: [...]` array of
  `macros::macro_connector_payout_implementation!`. `trustly.rs` deliberately omits it from that
  list — that is how it makes room for the real implementation.

Each stub is a **coverage gap, not a decision that the flow should never be covered**.

## Common Implementation Patterns

### Two authoring styles are in-tree — pick one and stay in it

- **Macro style (generic `<T>`)** — `payout_connectors/{deutschebank,gotyme_sanlam,trustly}.rs`:
  `macros::create_all_prerequisites!` for the struct, one
  `macros::macro_connector_implementation!` per *real* flow (with
  `resource_common_data: PayoutFlowData` and `flow_name: Payout…`), then a single
  `macros::macro_connector_payout_implementation!` with `payout_flows: [...]` listing only the
  flows that stay stubs.
- **Hand-written style (non-generic)** —
  `payout_connectors/{cybersource,itaubank,loonio,paypal,santander,truelayer,worldpayxml}.rs`:
  the connector struct has no type parameter, and every flow — real and stub — is a raw
  `impl ConnectorIntegrationV2<Flow, PayoutFlowData, FlowRequest, FlowResponse> for <Name>Payouts`
  block preceded by `impl Payout<Flow>V2 for <Name>Payouts {}`.

A flow may never be covered twice: if a flow appears in `payout_flows: [...]` it must **not** also
have a hand-written `impl`, or the build fails with a conflicting-implementation error.

### The real pattern: `macro_connector_implementation!`, not `macro_connector_payout_implementation!`

`macro_connector_payout_implementation!` produces **stubs only**. The macro that produces a *working*
flow is `macros::macro_connector_implementation!`, and `TrustlyPayouts<T>` is the in-tree example:

```rust
// crates/integrations/connector-integration/src/payout_connectors/trustly.rs
//   — under the `// ===== PAYOUT CREATE RECIPIENT (RegisterAccount) =====` banner
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: TrustlyPayouts,
    curl_request: Json(RegisterAccountRequest),
    curl_response: RegisterAccountResponse,
    flow_name: PayoutCreateRecipient,
    resource_common_data: PayoutFlowData,
    flow_request: PayoutCreateRecipientRequest,
    flow_response: PayoutCreateRecipientResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<
                PayoutCreateRecipient,
                PayoutFlowData,
                PayoutCreateRecipientRequest,
                PayoutCreateRecipientResponse,
            >,
        ) -> CustomResult<String, IntegrationError> {
            // Trustly is JSON-RPC: the method lives in the body, not the path.
            Ok(self.base_url(&req.resource_common_data.connectors).to_string())
        }

        fn get_headers(
            &self,
            _req: &RouterDataV2<
                PayoutCreateRecipient,
                PayoutFlowData,
                PayoutCreateRecipientRequest,
                PayoutCreateRecipientResponse,
            >,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(self.payout_headers())
        }
    }
);
```

Three preconditions for that call to work:

1. The connector struct is **generic** (`TrustlyPayouts<T>`), built by
   `macros::create_all_prerequisites!` with a matching `api: [(flow: PayoutCreateRecipient,
   request_body: RegisterAccountRequest, response_body: RegisterAccountResponse, router_data:
   RouterDataV2<PayoutCreateRecipient, PayoutFlowData, PayoutCreateRecipientRequest,
   PayoutCreateRecipientResponse>)]` entry.
2. `impl PayoutCreateRecipientV2 for TrustlyPayouts<T> {}` is written by hand alongside.
3. `PayoutCreateRecipient` is **absent** from the `payout_flows: [...]` list of the file's
   `macro_connector_payout_implementation!` call — otherwise the two impls conflict.

For a non-generic payout connector, write the `ConnectorIntegrationV2<PayoutCreateRecipient, …>`
block longhand instead; neither macro can be used.

### What `macro_connector_payout_implementation!` actually emits

`crates/integrations/connector-integration/src/connectors/macros.rs` defines
`macro_connector_payout_implementation!` (three arms: a default arm that supplies all nine flows, a
recursive arm that peels one flow at a time, and an empty-list base case) which delegates per flow to
`expand_payout_implementation!`. Every arm of `expand_payout_implementation!` — including the
`PayoutEligibility` arm — emits **two** items:

```rust
impl<T: ...> ::interfaces::connector_types::Payout<Flow>V2 for $connector<T> {}
impl<T: ...> ConnectorIntegrationV2<PayoutXxx, PayoutFlowData, PayoutXxxRequest, PayoutXxxResponse>
    for $connector<T>
{
    fn get_url(&self, _req: &RouterDataV2<...>) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            ConnectorCommon::id(self), "payout_xxx", IntegrationErrorContext::default(),
        ).into())
    }
}
```

It is **not** an empty `{}` body: the macro overrides `get_url` so the stub fails fast with
`IntegrationError::connector_flow_not_implemented` and a flow-name string, rather than falling
through to a `ConnectorIntegrationV2` trait default. The generic parameter is mandatory — the macro
only matches `$connector<$generic_type>`, so a **non-generic** payout connector (`ItaubankPayouts`,
`PaypalPayouts`, `SantanderPayouts`, `WorldpayxmlPayouts`, `CybersourcePayouts`, `LoonioPayouts`,
`TruelayerPayouts`) cannot use it and must hand-write its stubs. `payout_connectors/truelayer.rs`
solves this with a file-local `macro_rules! impl_unimplemented_payout_flow!`; the other six write the
`impl` blocks out longhand.


### Recipient-Type Branching Pattern

Connectors whose KYC endpoints differ by recipient kind (Individual KYC1 vs Business KYC2) should branch on `req.request.recipient_type`:

```rust
// Reference — branch style mirrors the tax-id branch in
// crates/integrations/connector-integration/src/payout_connectors/itaubank/transformers.rs
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

All seven `PayoutRecipientType` variants must be matched (no wildcard catchalls) per §11 of `PATTERN_AUTHORING_SPEC.md` — silent omission of enum variants is banned. Variants enumerated at `crates/common/common_enums/src/enums.rs`.

## Connector-Specific Patterns

### trustly (`TrustlyPayouts<T>`, the only full implementation)

- **Generic struct** built by `macros::create_all_prerequisites!(connector_name: TrustlyPayouts,
  generic_type: T, api: [ … ])` — one of only three generic payout connectors, and therefore one of
  only three that can use the payout macros at all.
- **JSON-RPC transport.** `get_url` returns the bare base URL; the RPC method (`RegisterAccount`) is
  a field on `RegisterAccountRequest`. `get_headers` delegates to the member function
  `self.payout_headers()`. Content type is the file constant
  `const CONTENT_TYPE_JSON: &str = "application/json; charset=UTF-8";`.
- **The recipient handle goes into `payout_connector_metadata`, not `connector_payout_id`.** The
  response `TryFrom` in `payout_connectors/trustly/transformers.rs` matches on
  `RegisterAccountResponse::Success(response)`, reads `response.result.data.accountid`, and packs it
  as `Some(Secret::new(serde_json::json!({ "account_id": account_id })))`, setting
  `connector_payout_id: None`.
- **Status is `PayoutStatus::RequiresCreation`** — the account is registered, but no payout exists
  yet.
- **Errors are a response variant, not an HTTP status.** `RegisterAccountResponse` is an enum with
  `Success` and `Error` arms; the `Error` arm returns
  `Err(build_error_from_response(&error_response, item.http_code))` from inside the response
  `TryFrom`.
- The `payout_flows: [...]` list in `trustly.rs` is
  `[PayoutCreate, PayoutVoid, PayoutStage, PayoutCreateLink, PayoutEnrollDisburseAccount]` —
  `PayoutCreateRecipient`, `PayoutTransfer` and `PayoutGet` are absent because each has a real
  `macro_connector_implementation!` call, and `PayoutEligibility` is absent because its stub is
  written by hand.

### itaubank

- itaubank carries a hand-written `PayoutCreateRecipient` stub in
  `payout_connectors/itaubank.rs`. Itaú SiSPAG identifies beneficiaries per-transfer (via `documento`
  on `ItaubankRecebedor` in `payout_connectors/itaubank/transformers.rs`) and has no separate
  recipient-onboarding endpoint, so the flow is registered-but-inert. The transformers file contains
  no `PayoutCreateRecipientRequest`/`PayoutCreateRecipientResponse` `TryFrom` blocks.

## Code Examples

### 1. Stub registration via the macro (generic connectors only)

```rust
// crates/integrations/connector-integration/src/payout_connectors/deutschebank.rs
macros::macro_connector_payout_implementation!(
    connector: DeutschebankPayouts,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    payout_flows: [
        PayoutCreate,
        PayoutVoid,
        PayoutStage,
        PayoutCreateLink,
        PayoutCreateRecipient,   // <-- registers marker + fail-fast integration impl
        PayoutEnrollDisburseAccount
    ]
);
```

`PayoutTransfer`, `PayoutGet` and `PayoutEligibility` are absent from that list: Deutsche Bank
implements all three for real via `macros::macro_connector_implementation!`.

### 2. Marker trait definition

```rust
// From crates/types-traits/interfaces/src/connector_types.rs
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
// From crates/common/common_enums/src/enums.rs
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

### 4. Hand-written implementation shape (non-generic connectors)

Seven of the ten payout connectors are non-generic unit structs and cannot use either macro; they
write the block out longhand. Adapted from the `PayoutTransfer` impl in
`crates/integrations/connector-integration/src/payout_connectors/itaubank.rs`:

```rust
impl PayoutCreateRecipientV2 for MyConnectorPayouts {}

impl
    ConnectorIntegrationV2<
        PayoutCreateRecipient,
        PayoutFlowData,
        PayoutCreateRecipientRequest,
        PayoutCreateRecipientResponse,
    > for MyConnectorPayouts
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
    ) -> CustomResult<Option<ConnectorRequestData>, IntegrationError> {
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
        ConnectorError,
    > {
        let response: MyConnectorRecipientResponse = res
            .response
            .parse_struct("MyConnectorRecipientResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: Default::default(),
            })?;

        finalize_connector_response!(event_builder, response, data, res.status_code)
    }
}
```

Two signature points that are easy to get wrong: `get_request_body` returns
**`Option<ConnectorRequestData>`** (see `fn get_request_body` on `ConnectorIntegrationV2` in
`crates/types-traits/interfaces/src/connector_integration_v2.rs`), built with
`ConnectorRequestData::new(RequestContent::Json(Box::new(req)), typed)`; and `handle_response_v2`
should delegate to `finalize_connector_response!`
(`crates/integrations/connector-integration/src/utils.rs`) rather than hand-assembling the
`RouterDataV2`. The recipient handle and `PayoutStatus` are populated in the
`TryFrom<ResponseRouterData<…>>` that macro invokes.

### 5. payout_method_data enum (must be exhaustively matched in branches)

```rust
// crates/types-traits/domain_types/src/payouts/payout_method_data.rs — pub enum PayoutMethodData
pub enum PayoutMethodData {
    Card(CardPayout),
    Bank(Bank),
    Wallet(Wallet),
    BankRedirect(BankRedirect),
    Passthrough(Passthrough),
}

// same file — pub enum Bank (ten sub-variants)
pub enum Bank {
    Ach(AchBankTransfer),
    Bacs(BacsBankTransfer),
    Sepa(SepaBankTransfer),
    Pix(PixBankTransfer),
    PixKey(PixKeyBankTransfer),
    PixEmv(PixEmvBankTransfer),
    OpenBanking(OpenBanking),
    Trustly(TrustlyBankTransfer),
    Payshap(PayshapBankTransfer),
    PayshapProxy(PayshapProxyBankTransfer),
}
```

### 6. Returning the recipient handle (`TrustlyPayouts`)

```rust
// crates/integrations/connector-integration/src/payout_connectors/trustly/transformers.rs
//   — impl TryFrom<ResponseRouterData<RegisterAccountResponse, Self>>
//     for RouterDataV2<PayoutCreateRecipient, PayoutFlowData, PayoutCreateRecipientRequest, PayoutCreateRecipientResponse>
match item.response {
    RegisterAccountResponse::Success(response) => {
        let account_id = response.result.data.accountid;
        let payout_connector_metadata = Some(Secret::new(serde_json::json!({
            "account_id": account_id,
        })));
        Ok(Self {
            response: Ok(PayoutCreateRecipientResponse {
                merchant_payout_id: item.router_data.request.merchant_payout_id.clone(),
                payout_status: common_enums::PayoutStatus::RequiresCreation,
                connector_payout_id: None,
                status_code: item.http_code,
                payout_connector_metadata,
            }),
            ..item.router_data
        })
    }
    RegisterAccountResponse::Error(error_response) => Ok(Self {
        response: Err(build_error_from_response(&error_response, item.http_code)),
        ..item.router_data
    }),
}
```

The handle travels forward on `PayoutTransferRequest.payout_connector_metadata`
(`crates/types-traits/domain_types/src/payouts/payouts_types.rs`) — the only field on the transfer
request designed to carry it.

## Integration Guidelines

1. Confirm the connector exposes a recipient-onboarding endpoint. If recipient identity is
   per-transfer (as with itaubank's `documento` on `ItaubankRecebedor`), leave this flow as a
   registered stub and put beneficiary data on `PayoutCreate`/`PayoutTransfer` instead.
2. **Remove the existing stub.** Every payout connector already registers `PayoutCreateRecipientV2`,
   so implementing it is a *replacement*:
   - generic connector: delete `PayoutCreateRecipient` from the `payout_flows: [...]` list passed to
     `macros::macro_connector_payout_implementation!`, exactly as `trustly.rs` does;
   - non-generic connector: delete the hand-written stub `impl` whose only method is a `get_url`
     returning `connector_flow_not_implemented`.
   Leaving both is a conflicting-implementation compile error.
3. Write `impl PayoutCreateRecipientV2 for <Name>Payouts {}` plus the real integration impl —
   `macros::macro_connector_implementation!` with `flow_name: PayoutCreateRecipient` and
   `resource_common_data: PayoutFlowData` for a generic connector (see `trustly.rs`), or a longhand
   `impl ConnectorIntegrationV2<PayoutCreateRecipient, PayoutFlowData, PayoutCreateRecipientRequest,
   PayoutCreateRecipientResponse>` block for a non-generic one.
4. In `payout_connectors/<name>/transformers.rs`:
   - Add `TryFrom<&RouterDataV2<PayoutCreateRecipient, …>>` for the connector's recipient-create
     struct.
   - Branch on `req.request.recipient_type` exhaustively — all seven variants of
     `pub enum PayoutRecipientType` in `crates/common/common_enums/src/enums.rs`.
   - Branch on `req.request.payout_method_data` exhaustively over the five `PayoutMethodData`
     variants (and the ten `Bank` sub-variants where relevant) in
     `crates/types-traits/domain_types/src/payouts/payout_method_data.rs`. Emit
     `IntegrationError::NotSupported { message, connector, context }`
     (`crates/types-traits/domain_types/src/errors.rs`) for any rail the connector does not accept.
   - Use the request's own `get_optional_billing_address()` accessor rather than re-navigating
     `address.billing_address.address`.
5. Add `TryFrom<ResponseRouterData<…>>` that lifts the connector's recipient handle into
   **`payout_connector_metadata`** (or `connector_payout_id`, if the connector's handle really is the
   payout id) and maps KYC state into `PayoutStatus`. Call
   `finalize_connector_response!(event_builder, response, data, res.status_code)` from
   `handle_response_v2`.
6. Ignore `amount` and `source_currency` when the connector does not require them for recipient
   creation. They exist on `PayoutCreateRecipientRequest` for envelope consistency and are not
   semantically meaningful to every connector.
7. Write unit tests covering each supported `recipient_type × payout_method_data` combination.
8. Document the downstream mapping: whatever this flow returns must be threaded into the
   `PayoutTransferRequest` — `payout_connector_metadata` for a Trustly-style opaque handle, or
   `connector_payout_method_id` for a connector that returns a plain id
   (`crates/types-traits/domain_types/src/payouts/payouts_types.rs`).
9. **No `config/superposition.toml` entry and no `connector_specs/<connector>/specs.json` entry.**
   `PayoutService` is in `IGNORE_SERVICES` in both
   `crates/internal/integration-tests/src/bin/check_connector_specs.rs` and `check_coverage.rs`, and
   `flow_to_suites()` maps every payout flow to `None`, so payouts sit outside the merge-blocking
   certification gate.

## Best Practices

- Exhaustively match every `PayoutRecipientType` variant listed at `crates/common/common_enums/src/enums.rs`. Wildcards are banned by §11 of `PATTERN_AUTHORING_SPEC.md`.
- Exhaustively match every `PayoutMethodData` variant at `crates/types-traits/domain_types/src/payouts/payout_method_data.rs`. For unsupported rails, emit `IntegrationError::NotSupported { message, connector, context }` (errors.rs) with a message that names the rail.
- Map KYC-pending connector responses to `PayoutStatus::RequiresVendorAccountCreation` (variant at `crates/common/common_enums/src/enums.rs`). Do NOT map to `Success` until the connector has verified.
- Return the connector's recipient handle so the router can persist it. `PayoutCreateRecipientResponse`
  carries `payout_connector_metadata: Option<common_utils::pii::SecretSerdeValue>` for exactly this —
  `payout_connectors/trustly/transformers.rs` puts the Trustly `accountid` there and leaves
  `connector_payout_id: None`. `PayoutTransferRequest` has a matching
  `payout_connector_metadata` field. Use `connector_payout_id` +
  `PayoutCreateRequest.connector_payout_method_id` only when the connector's handle really is a plain
  payout-method id.
- See sibling flow [pattern_payout_enroll_disburse_account.md](./pattern_payout_enroll_disburse_account.md) for the follow-on step of attaching a bank account to the newly-created recipient.

## Common Errors / Gotchas

1. **Problem:** `PayoutCreateRecipientResponse.payout_status = PayoutStatus::Success` right after recipient creation.
   **Solution:** Most KYC rails return a pending status first. Map to `PayoutStatus::RequiresVendorAccountCreation` (variant at `crates/common/common_enums/src/enums.rs`) and only promote to `Success` after the connector confirms. Read the connector's status enum, don't guess.

2. **Problem:** Rust compile error on missing arm when matching `PayoutRecipientType`.
   **Solution:** Seven variants. Enumerated at `crates/common/common_enums/src/enums.rs`. Authors MUST list all of them explicitly. Do not use a wildcard.

3. **Problem:** `payout_method_data = None` at runtime and the connector requires an account.
   **Solution:** Emit `IntegrationError::MissingRequiredField { field_name: "payout_method_data", .. }`. `IntegrationError` enum at `crates/types-traits/domain_types/src/errors.rs` onward.

4. **Problem:** Compile error "conflicting implementations of trait `ConnectorIntegrationV2<PayoutCreateRecipient, ...>`".
   **Solution:** A stub already exists. Remove `PayoutCreateRecipient` from the `payout_flows: [...]`
   list (generic connector) or delete the hand-written stub `impl` (non-generic connector) before
   writing the real one. Rust has no specialization here.

4a. **Problem:** `macros::macro_connector_payout_implementation!` or
   `macros::macro_connector_implementation!` does not expand.
   **Solution:** Both require a **generic** connector struct. Only `TrustlyPayouts<T>`,
   `DeutschebankPayouts<T>` and `GotymeSanlamPayouts<T>` qualify; the other seven payout connectors
   are non-generic unit structs (`pub struct ItaubankPayouts;`) and must write their impls longhand.

5. **Problem:** Recipient created successfully but the downstream transfer sends a fresh beneficiary
   body and the connector errors "duplicate recipient".
   **Solution:** Thread the handle forward. `PayoutCreateRecipientResponse.payout_connector_metadata`
   → `PayoutTransferRequest.payout_connector_metadata` is the Trustly path; a plain id goes
   `connector_payout_id` → `PayoutCreateRequest.connector_payout_method_id`. Both fields are in
   `crates/types-traits/domain_types/src/payouts/payouts_types.rs`. When the reference is present,
   the downstream transformer should omit inline beneficiary details.

6. **Problem:** Writing `get_request_body` to return `Option<RequestContent>`.
   **Solution:** The trait signature is
   `CustomResult<Option<ConnectorRequestData>, IntegrationError>`
   (`crates/types-traits/interfaces/src/connector_integration_v2.rs`); wrap with
   `ConnectorRequestData::new(RequestContent::Json(Box::new(req)), typed)`.

7. **Problem:** Adding `impl ValidationTrait for <Name>Payouts` to force an access-token exchange.
   **Solution:** `PayoutServiceTrait` does not require `ValidationTrait`, and nothing reads
   `should_do_access_token` on the payout path. The token arrives on the RPC's
   `optional SecretString access_token` and is read with
   `req.resource_common_data.get_access_token()`.

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

`TrustlyPayouts<T>` is the only connector that can exercise these scenarios today; the other nine
return `connector_flow_not_implemented`. Trustly's own happy path returns
`PayoutStatus::RequiresCreation` with the account handle in `payout_connector_metadata`. The in-repo
gRPC-level harness is `crates/grpc-server/grpc-server/tests/payout_flows_test.rs`; there is no
`connector_specs` suite for payouts, since `PayoutService` is in `IGNORE_SERVICES` in
`crates/internal/integration-tests/src/bin/check_connector_specs.rs`.

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
- Reference implementation: `crates/integrations/connector-integration/src/payout_connectors/trustly.rs`
- Registry: `crates/integrations/connector-integration/src/payout_connectors.rs`
