# PayoutTransfer Flow Pattern

## Overview

The `PayoutTransfer` flow moves funds from the processor to the payout beneficiary using a previously-established identifier (or inline payout-method data). It is driven by the `PayoutService::transfer` gRPC handler (`crates/grpc-server/grpc-server/src/server/payouts.rs`) and dispatched through `internal_payout_transfer` (`crates/grpc-server/grpc-server/src/server/payouts.rs`) under the `FlowName::PayoutTransfer` marker (`crates/types-traits/domain_types/src/connector_flow.rs`).

`PayoutTransfer` is the **best-covered payout flow**: all ten payout connectors supply a real,
non-stub `ConnectorIntegrationV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest,
PayoutTransferResponse>` implementation — `cybersource`, `deutschebank`, `gotyme_sanlam`, `itaubank`,
`loonio`, `paypal`, `santander`, `truelayer`, `trustly`, `worldpayxml`, all under
`crates/integrations/connector-integration/src/payout_connectors/`. This pattern documents
`ItaubankPayouts` (Itaú SiSPAG / PIX) as the hand-written reference and
`GotymeSanlamPayouts<T>` / `TrustlyPayouts<T>` / `DeutschebankPayouts<T>` as the macro-driven
reference.

Verify the roster before trusting it:

```bash
rg -n 'ConnectorIntegrationV2<\s*PayoutTransfer|flow_name: PayoutTransfer' \
   crates/integrations/connector-integration/src/payout_connectors/
```

### Key Components

- **Flow marker**: `domain_types::connector_flow::PayoutTransfer` — `crates/types-traits/domain_types/src/connector_flow.rs`.
- **Flow data**: `domain_types::payouts::payouts_types::PayoutFlowData` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`.
- **Request data**: `domain_types::payouts::payouts_types::PayoutTransferRequest` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`.
- **Response data**: `domain_types::payouts::payouts_types::PayoutTransferResponse` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`.
- **Marker trait**: `interfaces::connector_types::PayoutTransferV2` — `crates/types-traits/interfaces/src/connector_types.rs`, defined solely as the supertrait binding:

  ```rust
  // crates/types-traits/interfaces/src/connector_types.rs — pub trait PayoutTransferV2
  pub trait PayoutTransferV2:
      ConnectorIntegrationV2<
      connector_flow::PayoutTransfer,
      PayoutFlowData,
      PayoutTransferRequest,
      PayoutTransferResponse,
  >
  {
  }
  ```

- **Service trait**: `interfaces::connector_types::PayoutServiceTrait` — `crates/types-traits/interfaces/src/connector_types.rs`.
- **Primary connector file**: `crates/integrations/connector-integration/src/payout_connectors/itaubank.rs` (`pub struct ItaubankPayouts;` — non-generic).
- **Primary transformers file**: `crates/integrations/connector-integration/src/payout_connectors/itaubank/transformers.rs`.

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
PayoutService::transfer (gRPC)
    │   crates/grpc-server/grpc-server/src/server/payouts.rs
    ▼
internal_payout_transfer
    │   crates/grpc-server/grpc-server/src/server/payouts.rs
    ▼
(caller first obtains a token over MerchantAuthenticationService and passes it
 back on the payout RPC as `access_token`; see the note below — there is no
 should_do_access_token hook on the payout path)
    ▼
RouterDataV2<PayoutTransfer, PayoutFlowData,
             PayoutTransferRequest, PayoutTransferResponse>
    │
    ├─▶ ConnectorIntegrationV2<PayoutTransfer, ...>::get_url/headers/body
    │       crates/integrations/connector-integration/src/payout_connectors/itaubank.rs
    │
    ├─▶ transport (HTTP POST /v1/transferencias)
    │
    └─▶ ConnectorIntegrationV2::handle_response_v2
            -> PayoutTransferResponse (payout_status, connector_payout_id, status_code)
```

The generic router-data template (per `PATTERN_AUTHORING_SPEC.md` §7):

```rust
RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>
// from crates/types-traits/domain_types/src/router_data_v2.rs
```

### Flow Type

`domain_types::connector_flow::PayoutTransfer` — unit marker struct at `crates/types-traits/domain_types/src/connector_flow.rs`. It is parametric on `RouterDataV2`'s first slot via `PhantomData<PayoutTransfer>` (`crates/types-traits/domain_types/src/router_data_v2.rs`).

### Request Type

`domain_types::payouts::payouts_types::PayoutTransferRequest` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`. Shape:

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs
#[derive(Debug, Clone)]
pub struct PayoutTransferRequest {
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
    pub address: Option<PayoutAddress>,
    pub source_bank_data: Option<Bank>,
    pub customer: Option<PayoutCustomer>,
    pub connector_eligibility_reference_id: Option<String>,
    pub payout_connector_metadata: Option<common_utils::pii::SecretSerdeValue>,
}
```

Fifteen fields. `PayoutTransferRequest` is **not** structurally identical to `PayoutCreateRequest`
(eleven fields): only `PayoutTransferRequest` carries `address`, `customer`,
`connector_eligibility_reference_id` and `payout_connector_metadata`. The last two are the hand-off
from the two upstream flows — `connector_eligibility_reference_id` from
`PayoutEligibilityResponse`, `payout_connector_metadata` from `PayoutCreateRecipientResponse`.
`PayoutTransferRequest` also carries accessor helpers defined in the same file
(`get_billing`, `get_billing_address`, `get_billing_first_name`, `get_billing_last_name`,
`get_customer_id`, `get_optional_customer_id`, and the `get_optional_billing_*` family) — use those
rather than re-navigating the `Option` chain.

### Response Type

`domain_types::payouts::payouts_types::PayoutTransferResponse` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`. Shape:

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs
#[derive(Debug, Clone)]
pub struct PayoutTransferResponse {
    pub merchant_payout_id: Option<String>,
    pub payout_status: common_enums::PayoutStatus,
    pub connector_payout_id: Option<String>,
    pub status_code: u16,
}
```

`common_enums::PayoutStatus` is defined at `crates/common/common_enums/src/enums.rs`.

### Resource Common Data

`domain_types::payouts::payouts_types::PayoutFlowData` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs`. Thirteen fields; the same struct backs every payout flow. Note in particular:

- `access_token: Option<ServerAuthenticationTokenResponseData>` — copied straight off the gRPC
  request by the `ForeignTryFrom` impls in
  `crates/types-traits/domain_types/src/payouts/types.rs`; unwrapped via
  `PayoutFlowData::get_access_token`.
- `test_mode: Option<bool>` — consumed by the free function `build_env_specific_endpoint` in
  `payout_connectors/itaubank.rs`.
- `connectors: Arc<Connectors>` — the base-URL bag (an `Arc`, not a bare `Connectors`), read via
  `ConnectorCommon::base_url(&req.resource_common_data.connectors)`.
- `typed_connector_request` / `typed_connector_response: Option<String>` — the observability pair
  written through the `RawConnectorRequestResponse` impl in the same file; `finalize_connector_response!`
  sets the response side for you.

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


## Connectors with Full Implementation

All ten payout connectors implement this flow. Every path below is relative to
`crates/integrations/connector-integration/src/payout_connectors/`.

| Connector | Style | HTTP | URL pattern | Request type | Notes |
| --------- | ----- | ---- | ----------- | ------------ | ----- |
| `ItaubankPayouts` | hand-written | `POST` | `{env_base}/v1/transferencias`, env suffix from `build_env_specific_endpoint` (`/itau-ep9-gtw-sispag-ext` in test, `/sispag` in prod) | `ItaubankTransferRequest` | mTLS via `get_certificate` / `get_certificate_key` off `ItaubankAuthType`; PIX-only beneficiary. `itaubank.rs`, `itaubank/transformers.rs`. |
| `SantanderPayouts` | hand-written | `PATCH` | `…/pix_payments/{id}` | `SantanderTransferRequest` family | Fulfilment leg of a two-step create→transfer product; mTLS. `santander.rs`. |
| `PaypalPayouts` | hand-written | `POST` | PayPal Payouts batch endpoint | `paypal::PaypalFulfillRequest` → `paypal::PaypalFulfillResponse` | `paypal.rs`; transformers in `paypal/transformers.rs`. |
| `CybersourcePayouts` | hand-written | `POST` | Cybersource payouts endpoint | flow-local | Only connector whose `PayoutGet` is still a stub. `cybersource.rs`. |
| `WorldpayxmlPayouts` | hand-written | `POST` | XML endpoint | flow-local | Also the only connector with a real `PayoutVoid`. `worldpayxml.rs`. |
| `LoonioPayouts` | hand-written | `POST` | Loonio transfer endpoint | flow-local | `loonio.rs`. |
| `TruelayerPayouts` | hand-written | `POST` | TrueLayer payouts endpoint | flow-local | Stubs generated by the file-local `macro_rules! impl_unimplemented_payout_flow!`. `truelayer.rs`. |
| `TrustlyPayouts<T>` | macro | `POST` | Trustly JSON-RPC | `AccountPayoutRequest` → `AccountPayoutResponse` | `macros::macro_connector_implementation!` with `flow_name: PayoutTransfer`, `resource_common_data: PayoutFlowData`. `trustly.rs`. |
| `DeutschebankPayouts<T>` | macro | `POST` | SEPA payment endpoint | `DeutschebankSepaPaymentRequest` → `DeutschebankSepaPaymentResponse` | `deutschebank.rs`. |
| `GotymeSanlamPayouts<T>` | macro | `POST` | GoTyme payout endpoint | `GotymeSanlamPayoutTransferRequest` → `GotymeSanlamPayoutResponse` | `gotyme_sanlam.rs`. |

### Current implementation coverage

**10 of 10 payout connectors.** Verified with:

```bash
rg -n 'ConnectorIntegrationV2<\s*PayoutTransfer' crates/integrations/connector-integration/src/payout_connectors/   # 7 hand-written
rg -n 'flow_name: PayoutTransfer'                 crates/integrations/connector-integration/src/payout_connectors/   # 3 macro-driven
```

### Stub Implementations

_(none — every payout connector implements `PayoutTransfer` for real.)_

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

### Hand-written track, step by step (as `ItaubankPayouts` does it)

1. Declare the connector as a non-generic unit struct with a `'static` constructor:
   `pub struct ItaubankPayouts;` / `impl ItaubankPayouts { pub const fn new() -> &'static Self { &Self } }`.
   Do **not** call `macros::create_all_prerequisites!` — itaubank does not.
2. `impl PayoutTransferV2 for ItaubankPayouts {}` — the marker.
3. `impl ConnectorIntegrationV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse> for ItaubankPayouts`
   overriding `get_http_method`, `get_content_type`, `get_certificate`, `get_certificate_key`,
   `get_url`, `get_headers`, `get_request_body`, `handle_response_v2`, `get_error_response_v2`.
4. Hand-write the eight stub flows (marker impl + one-method `ConnectorIntegrationV2` whose `get_url`
   returns `IntegrationError::connector_flow_not_implemented`).
5. `TryFrom<&RouterDataV2<PayoutTransfer, …>>` for the connector-local request struct, and
   `TryFrom<ResponseRouterData<…>>` for the `RouterDataV2`, both in
   `payout_connectors/itaubank/transformers.rs`.

### Macro track, step by step (as `GotymeSanlamPayouts<T>` does it)

1. `macros::create_all_prerequisites!(connector_name: GotymeSanlamPayouts, generic_type: T, api: [ … ])`
   with one `api` entry per real flow: `(flow: PayoutTransfer, request_body: …, response_body: …,
   router_data: RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>)`.
2. `macros::macro_connector_implementation!( … flow_name: PayoutTransfer, resource_common_data: PayoutFlowData,
   flow_request: PayoutTransferRequest, flow_response: PayoutTransferResponse, http_method: Post, … )`
   for each real flow.
3. `macros::macro_connector_payout_implementation!` with `payout_flows: [...]` listing **only** the
   remaining stub flows.

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


## Connector-Specific Patterns

All paths below are relative to `crates/integrations/connector-integration/src/payout_connectors/`.

### itaubank (`ItaubankPayouts`, hand-written)

- **Non-generic struct.** `pub struct ItaubankPayouts;` with `pub const fn new() -> &'static Self`.
  It cannot use `macro_connector_payout_implementation!`, so its eight stub flows are written out
  longhand in `itaubank.rs`.
- **URL construction is env-sensitive.** The free function
  `build_env_specific_endpoint(base_url, test_mode)` in `itaubank.rs` appends
  `/itau-ep9-gtw-sispag-ext` when `test_mode.unwrap_or(true)` and `/sispag` otherwise; `get_url`
  then formats `{base_url}/v1/transferencias`.
- **Mutual TLS.** `get_certificate` and `get_certificate_key` build `ItaubankAuthType` from
  `req.connector_config` and return `auth.certificates` / `auth.private_key`.
- **Access token is read, not sequenced.** `get_headers` calls
  `req.resource_common_data.get_access_token()` and maps the error to
  `IntegrationError::FailedToObtainAuthType`. There is no `ValidationTrait` impl on
  `ItaubankPayouts` and no `should_do_access_token` hook anywhere on the payout path.
- **Amount is serialized as `StringMajorUnit`** via `StringMajorUnitForConnector` in the request
  `TryFrom` (`itaubank/transformers.rs`).
- **Request body is Portuguese-named.** `ItaubankTransferRequest` carries `valor_pagamento`,
  `data_pagamento`, `chave`, `referencia_empresa`, `identificacao_comprovante`,
  `tipo_de_identificacao_do_recebedor`, `pagador`, `recebedor`, `emv`. Beneficiary details nest under
  `ItaubankRecebedor`; the debtor side nests under `ItaubankPagador`, built from
  `req.request.source_bank_data` (not from `payout_method_data`).
- **Three PIX shapes are handled**, matched on `req.request.payout_method_data`:
  `Bank::Pix(PixBankTransfer)` fills `recebedor`; `Bank::PixEmv(PixEmvBankTransfer)` fills `emv`;
  `Bank::PixKey(PixKeyBankTransfer)` fills `chave`. Anything else yields all three as `None`.
- **Person type is inferred from tax-ID length.** 11-digit IDs map to
  `ItaubankRecipientType::Individual` (`#[serde(rename = "F")]`), everything else to
  `ItaubankRecipientType::LegalEntity` (`"J"`). The enum is `ItaubankRecipientType`, **not**
  `ItaubankPersonType`, and its non-individual variant is `LegalEntity`, not `Company`.
- **Response parsing tolerates dual field names.** `ItaubankTransferResponse` uses
  `#[serde(alias = …)]` to accept both `id`/`cod_pagamento` and `status`/`status_pagamento`.
- **Status mapping** lives on `impl ItaubankPayoutStatus { pub fn get_payout_status(&self) -> common_enums::PayoutStatus }`,
  with a `#[serde(other)] Unknown` variant mapping to `Pending`.

### santander (`SantanderPayouts`, hand-written)

- `PayoutTransfer` is the **fulfilment** leg of a two-step product: `PayoutCreate` `POST`s the PIX
  payment, `PayoutTransfer` `PATCH`es it to authorize. Both use mTLS via `SantanderAuthType`.

### trustly / deutschebank / gotyme_sanlam (generic, macro-driven)

- These three are the only payout connectors declared with a generic parameter and built through
  `macros::create_all_prerequisites!`. Their real flows come from
  `macros::macro_connector_implementation!` with `resource_common_data: PayoutFlowData`:
  `trustly.rs` (`AccountPayoutRequest`/`AccountPayoutResponse`), `deutschebank.rs`
  (`DeutschebankSepaPaymentRequest`/`DeutschebankSepaPaymentResponse`), `gotyme_sanlam.rs`
  (`GotymeSanlamPayoutTransferRequest`/`GotymeSanlamPayoutResponse`). Only these three can pass their
  remaining flows to `macro_connector_payout_implementation!`.

## Code Examples

### Example 1: itaubank trait wiring (marker + integration impl header)

```rust
// crates/integrations/connector-integration/src/payout_connectors/itaubank.rs
//   — the block under the `// ===== PAYOUT TRANSFER (REAL) =====` banner
impl PayoutTransferV2 for ItaubankPayouts {}

impl
    ConnectorIntegrationV2<
        PayoutTransfer,
        PayoutFlowData,
        PayoutTransferRequest,
        PayoutTransferResponse,
    > for ItaubankPayouts
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
    }

    fn get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn get_certificate(
        &self,
        req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> CustomResult<Option<hyperswitch_masking::Secret<String>>, IntegrationError> {
        let auth = ItaubankAuthType::try_from(&req.connector_config)?;
        Ok(auth.certificates)
    }

    fn get_url(
        &self,
        req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        let base_url = build_env_specific_endpoint(
            self.base_url(&req.resource_common_data.connectors),
            req.resource_common_data.test_mode,
        );
        Ok(format!("{base_url}/v1/transferencias"))
    }
```

### Example 2: Access-token header construction

```rust
// crates/integrations/connector-integration/src/payout_connectors/itaubank.rs
//   — fn get_headers on the PayoutTransfer integration impl
fn get_headers(
    &self,
    req: &RouterDataV2<
        PayoutTransfer,
        PayoutFlowData,
        PayoutTransferRequest,
        PayoutTransferResponse,
    >,
) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
    let access_token = req.resource_common_data.get_access_token().map_err(|_| {
        IntegrationError::FailedToObtainAuthType {
            context: Default::default(),
        }
    })?;
    let auth = ItaubankAuthType::try_from(&req.connector_config)?;

    Ok(vec![
        (
            headers::CONTENT_TYPE.to_string(),
            "application/json".to_string().into(),
        ),
        (headers::ACCEPT.to_string(), "*/*".to_string().into()),
        (
            headers::AUTHORIZATION.to_string(),
            format!("Bearer {access_token}").into_masked(),
        ),
        (
            headers::USER_AGENT.to_string(),
            "Hyperswitch".to_string().into(),
        ),
    ])
}
```

### Example 3: Request `TryFrom` with amount conversion and PIX beneficiary assembly

```rust
// crates/integrations/connector-integration/src/payout_connectors/itaubank/transformers.rs
//   — impl TryFrom<&RouterDataV2<PayoutTransfer, ...>> for ItaubankTransferRequest
impl
    TryFrom<
        &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    > for ItaubankTransferRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> Result<Self, Self::Error> {
        let converter = StringMajorUnitForConnector;
        let valor_pagamento = converter
            .convert(req.request.amount, req.request.source_currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        let data_pagamento = common_utils::date_time::date_as_yyyymmddthhmmssmmmz()
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        // The debtor side comes from `source_bank_data`, not `payout_method_data`.
        let pagador = match req.request.source_bank_data.clone() {
            Some(Bank::Pix(PixBankTransfer { tax_id, bank_branch, bank_account_number, .. })) => {
                // ... tipo_pessoa inferred from tax-ID length; ItaubankPagador assembled
                Some(ItaubankPagador { /* fields */ })
            }
            _ => None,
        };

        // Three PIX shapes are handled; each fills exactly one of the three slots.
        let (recebedor, emv, chave) = match req.request.payout_method_data.clone() {
            Some(PayoutMethodData::Bank(Bank::Pix(PixBankTransfer {
                tax_id,
                bank_branch,
                bank_account_number,
                bank_name,
                ispb,
                ..
            }))) => {
                let tipo_pessoa = tax_id.clone().expose_option().map(|id| {
                    if id.len() == 11 {
                        ItaubankRecipientType::Individual
                    } else {
                        ItaubankRecipientType::LegalEntity
                    }
                });
                (
                    Some(ItaubankRecebedor {
                        ispb,
                        banco: bank_name.map(|bank| bank.to_string()),
                        tipo_conta: Some(ItaubankAccountType::Checking),
                        agencia: bank_branch,
                        conta: Some(bank_account_number),
                        tipo_pessoa,
                        documento: tax_id,
                        nome: req.request.customer.as_ref().and_then(|c| c.name.clone()),
                    }),
                    None,
                    None,
                )
            }
            Some(PayoutMethodData::Bank(Bank::PixEmv(PixEmvBankTransfer { emv }))) => (None, Some(emv), None),
            Some(PayoutMethodData::Bank(Bank::PixKey(PixKeyBankTransfer { pix_key }))) => (None, None, Some(pix_key)),
            _ => (None, None, None),
        };

        Ok(Self {
            valor_pagamento,
            data_pagamento,
            tipo_de_identificacao_do_recebedor: recebedor.as_ref().and_then(|d| d.tipo_pessoa),
            referencia_empresa: req.request.merchant_payout_id.clone(),
            identificacao_comprovante: req.request.merchant_payout_id.clone().map(Secret::new),
            recebedor,
            emv,
            chave,
            pagador,
        })
    }
}
```

Note the beneficiary name comes from `req.request.customer` — a field that exists on
`PayoutTransferRequest` but **not** on `PayoutCreateRequest`.

### Example 4: Response handling and status mapping

Both `get_request_body` and `handle_response_v2` use helpers rather than hand-rolled boilerplate.
`get_request_body` returns **`Option<ConnectorRequestData>`** (see `fn get_request_body` on
`ConnectorIntegrationV2` in `crates/types-traits/interfaces/src/connector_integration_v2.rs`), and
`handle_response_v2` delegates to `finalize_connector_response!`
(`crates/integrations/connector-integration/src/utils.rs`).

```rust
// crates/integrations/connector-integration/src/payout_connectors/itaubank.rs
//   — fn get_request_body / fn handle_response_v2 on the PayoutTransfer integration impl
fn get_request_body(
    &self,
    req: &RouterDataV2<
        PayoutTransfer,
        PayoutFlowData,
        PayoutTransferRequest,
        PayoutTransferResponse,
    >,
) -> CustomResult<Option<ConnectorRequestData>, IntegrationError> {
    let connector_req = ItaubankTransferRequest::try_from(req)?;
    let typed =
        events::MaskedSerdeValue::from_masked_optional(&connector_req, "typed_connector_request");
    Ok(Some(ConnectorRequestData::new(
        RequestContent::Json(Box::new(connector_req)),
        typed,
    )))
}

fn handle_response_v2(
    &self,
    data: &RouterDataV2<
        PayoutTransfer,
        PayoutFlowData,
        PayoutTransferRequest,
        PayoutTransferResponse,
    >,
    event_builder: Option<&mut events::Event>,
    res: Response,
) -> CustomResult<
    RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>,
    ConnectorError,
> {
    let response: ItaubankTransferResponse = res
        .response
        .parse_struct("ItaubankTransferResponse")
        .change_context(ConnectorError::ResponseDeserializationFailed {
            context: Default::default(),
        })?;

    finalize_connector_response!(event_builder, response, data, res.status_code)
}
```

`finalize_connector_response!` builds the `RouterDataV2` through
`TryFrom<ResponseRouterData<ItaubankTransferResponse, …>>` (declared in
`payout_connectors/itaubank/transformers.rs`), sets the event response data, and writes
`typed_connector_response` on the `PayoutFlowData`.

### Example 5: Status enum mapping (no hardcoded status)

The mapping lives on the **status enum**, as an inherent `get_payout_status` method — not on the
response struct:

```rust
// crates/integrations/connector-integration/src/payout_connectors/itaubank/transformers.rs
//   — impl ItaubankPayoutStatus
impl ItaubankPayoutStatus {
    pub fn get_payout_status(&self) -> common_enums::PayoutStatus {
        match self {
            Self::Aprovado | Self::Confirmado | Self::Efetivado | Self::Sucesso => {
                common_enums::PayoutStatus::Success
            }
            Self::Pendente | Self::EmProcessamento => common_enums::PayoutStatus::Pending,
            Self::Rejeitado | Self::Cancelado | Self::NaoIncluido => {
                common_enums::PayoutStatus::Failure
            }
            Self::Unknown => common_enums::PayoutStatus::Pending,
        }
    }
}
```

`ItaubankPayoutStatus` is `#[serde(rename_all = "SCREAMING_SNAKE_CASE")]` with per-variant
`#[serde(alias = …)]` lists and a `#[serde(other)] Unknown` catch-all, so an unrecognised connector
status deserializes to `Unknown` and maps to `Pending` rather than failing the parse.
`SantanderPayouts` uses the identical shape (`impl SantanderPayoutStatus { pub fn get_payout_status(&self) … }`
in `payout_connectors/santander/transformers.rs`).

## Integration Guidelines

1. **Create the connector under `payout_connectors/`, not `connectors/`**, and export it from
   `payout_connectors.rs` — the module file, a sibling of the `payout_connectors/` directory (`pub mod <name>; pub use self::<name>::<Name>Payouts;`).
2. **Choose a style and commit to it.**
   - Hand-written (`ItaubankPayouts`, `SantanderPayouts`, …): a non-generic unit struct with
     `pub const fn new() -> &'static Self`; every flow, real and stub, written longhand.
   - Macro (`TrustlyPayouts<T>`, `DeutschebankPayouts<T>`, `GotymeSanlamPayouts<T>`):
     `macros::create_all_prerequisites!` + `macros::macro_connector_implementation!` per real flow +
     one `macros::macro_connector_payout_implementation!` for the remaining stubs.
3. **A flow is covered exactly once.** `PayoutTransfer` must never appear both in
   `payout_flows: [...]` and as a hand-written impl — that is a conflicting-implementation compile
   error.
4. **Implement the marker.** `impl PayoutTransferV2 for <Name>Payouts {}`.
5. **Implement `ConnectorIntegrationV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest,
   PayoutTransferResponse>`.** `get_request_body` returns `Option<ConnectorRequestData>`, built with
   `ConnectorRequestData::new(RequestContent::Json(Box::new(req)), typed)`. Use Examples 1, 2 and 4.
6. **Do not add a `ValidationTrait` impl.** `PayoutServiceTrait` does not require it and nothing on
   the payout path reads `should_do_access_token`. Read the token with
   `req.resource_common_data.get_access_token()`; the caller supplies it on the RPC.
7. **Implement `ServerAuthentication` if the connector is OAuth-style** —
   `ConnectorIntegrationV2<ServerAuthenticationToken, MerchantAuthenticationFlowData,
   ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>`. It is a required
   supertrait of `PayoutServiceTrait`, and its flow data is `MerchantAuthenticationFlowData`, not
   `PayoutFlowData`.
8. **Write the connector-local request struct** (derive `Serialize`) and its
   `TryFrom<&RouterDataV2<PayoutTransfer, …>>`. Convert `MinorUnit` with the right converter
   (`StringMajorUnitForConnector` for itaubank and santander), and build the beneficiary from
   `payout_method_data` and the debtor from `source_bank_data`.
9. **Write the connector-local response struct** (derive `Deserialize`) plus
   `TryFrom<ResponseRouterData<…>>` for the `RouterDataV2`, then call
   `finalize_connector_response!(event_builder, response, data, res.status_code)` from
   `handle_response_v2`.
10. **Map `payout_status` from a `get_payout_status()` method on the connector status enum.** Never
    hardcode.
11. **Delegate `get_error_response_v2` to `ConnectorCommon::build_error_response`.**
12. **Register in the four wiring sites**: `PayoutConnectorEnum`
    (`crates/types-traits/domain_types/src/connector_types.rs`),
    `PayoutConnectorData::convert_connector`
    (`crates/integrations/connector-integration/src/types.rs`), the `Connectors` field and the
    `patch_payout_connector_urls` arm (`crates/types-traits/domain_types/src/types.rs`).
13. **No `config/superposition.toml` entry and no `connector_specs/<connector>/specs.json` entry.**
    `PayoutService` is in `IGNORE_SERVICES` in both
    `crates/internal/integration-tests/src/bin/check_connector_specs.rs` and `check_coverage.rs`;
    `flow_to_suites()` maps every payout flow to `None`. Payout connectors sit outside the
    merge-blocking connector-certification gate.

## Best Practices

- **Always guard `get_access_token` with `map_err(|_| IntegrationError::FailedToObtainAuthType { … })`.** `get_headers` on itaubank's `PayoutTransfer` impl does this; santander instead lets the `?` propagate `missing_field_err("access_token")`. Pick one and stay consistent within a connector.
- **Reuse `ConnectorCommon::base_url` for the URL prefix**, then compose with helper functions such as `build_env_specific_endpoint` (`crates/integrations/connector-integration/src/payout_connectors/itaubank.rs`). Do not hardcode `req.resource_common_data.connectors.{name}.base_url` inside each flow.
- **Default unknown response statuses to `Pending`, not `Failure`.** See `crates/integrations/connector-integration/src/payout_connectors/itaubank/transformers.rs`; this avoids spurious failures when the connector adds new states.
- **Mask sensitive header values** with `into_masked()` (`crates/integrations/connector-integration/src/payout_connectors/itaubank.rs`) and wrap request-body secrets in `Secret<String>` (`crates/integrations/connector-integration/src/payout_connectors/itaubank/transformers.rs`).
- **Log parse failures with `tracing::error!`** before returning `ResponseDeserializationFailed` (`crates/integrations/connector-integration/src/payout_connectors/itaubank.rs`).
- **Prefer `change_context(IntegrationError::...)` over `map_err`** when propagating amount-conversion or date-formatting errors (`crates/integrations/connector-integration/src/payout_connectors/itaubank/transformers.rs`).

## Common Errors / Gotchas

1. **Problem**: Double-impl of `ConnectorIntegrationV2<PayoutTransfer, ...>` because both the macro and a manual block are present.
   **Solution**: For a generic connector, remove `PayoutTransfer` from the `payout_flows: [...]` list passed to `macros::macro_connector_payout_implementation!` — `payout_connectors/{deutschebank,gotyme_sanlam,trustly}.rs` each omit it for exactly this reason. For a non-generic connector such as `ItaubankPayouts` there is no such list (`payout_connectors/itaubank.rs` contains no `macro_connector_payout_implementation!` call at all); delete the hand-written stub `impl` instead.

2. **Problem**: `get_headers` fails because `PayoutFlowData.access_token` is `None`, and the author
   reaches for `ValidationTrait::should_do_access_token`.
   **Solution**: There is no such hook on the payout path — `PayoutServiceTrait` does not require
   `ValidationTrait`. The token is fetched by the caller over `MerchantAuthenticationService` (the
   `ServerAuthentication` binding on the connector, which uses `MerchantAuthenticationFlowData`) and
   returned on the payout RPC's `optional SecretString access_token`
   (`crates/types-traits/grpc-api-types/proto/payouts.proto`); the `ForeignTryFrom` impls in
   `crates/types-traits/domain_types/src/payouts/types.rs` copy it into `PayoutFlowData`.

3. **Problem**: Sandbox/production URL mismatch because `test_mode` is ignored.
   **Solution**: Read `req.resource_common_data.test_mode` and branch in a helper — mirror `build_env_specific_endpoint` (`crates/integrations/connector-integration/src/payout_connectors/itaubank.rs`).

4. **Problem**: The connector rejects the payload because `MinorUnit` was serialized numerically but the API expects a decimal string.
   **Solution**: Use `StringMajorUnitForConnector.convert(...)` in the request `TryFrom` (`crates/integrations/connector-integration/src/payout_connectors/itaubank/transformers.rs`).

4a. **Problem**: `get_request_body` is written to return `Option<RequestContent>`.
   **Solution**: The trait signature is
   `fn get_request_body(&self, …) -> CustomResult<Option<ConnectorRequestData>, IntegrationError>`
   (`crates/types-traits/interfaces/src/connector_integration_v2.rs`). Wrap the content with
   `ConnectorRequestData::new(RequestContent::Json(Box::new(req)), typed)`, where `typed` comes from
   `events::MaskedSerdeValue::from_masked_optional(&req, "typed_connector_request")`.

5. **Problem**: Response parsing fails intermittently because the connector alternates field names between docs versions.
   **Solution**: Use `#[serde(alias = "...", alias = "...")]` on each potentially-renamed field, as at `crates/integrations/connector-integration/src/payout_connectors/itaubank/transformers.rs`.

6. **Problem**: `PayoutStatus::Failure` is returned for rows the processor is still processing asynchronously.
   **Solution**: Map both the unknown sentinel and the `Pending`/`Processing` family to `PayoutStatus::Pending`, as at `crates/integrations/connector-integration/src/payout_connectors/itaubank/transformers.rs`.

## Testing Notes

### Unit-test shape

There is no `connector_specs` suite for payouts — `PayoutService` is in `IGNORE_SERVICES` in
`crates/internal/integration-tests/src/bin/check_connector_specs.rs`, so `PayoutTransfer` is outside
the merge-blocking certification gate. The in-repo gRPC-level coverage is
`crates/grpc-server/grpc-server/tests/payout_flows_test.rs`. Recommended unit-test surface for a new
implementation:

- **Request `TryFrom` per payout-method variant.** One test per supported variant of `PayoutMethodData` (`crates/types-traits/domain_types/src/payouts/payout_method_data.rs`) plus one negative test for unsupported variants.
- **Amount-converter behaviour.** At least one currency with decimal places (e.g. USD) and one without (e.g. JPY).
- **Status mapper exhaustiveness.** One test per branch of the connector-status match, including the `Unknown`/`None` default branch.
- **Date formatting.** Assert the string shape `common_utils::date_time::date_as_yyyymmddthhmmssmmmz` produces (itaubank uses this at `crates/integrations/connector-integration/src/payout_connectors/itaubank/transformers.rs`).

### Integration-test scenarios

| Scenario | Setup | Expected `payout_status` |
| -------- | ----- | ------------------------ |
| Happy path — PIX transfer | Valid sandbox access token, `PayoutMethodData::Bank(Bank::Pix(...))` | `Success` or `Pending` (mapped from connector status) |
| Missing access token | `PayoutFlowData.access_token = None` | `IntegrationError::FailedToObtainAuthType` |
| Unsupported payout method | `PayoutMethodData::Card(...)` | Request succeeds but `recebedor = None`; processor-side rejection mapped to `Failure` |
| `test_mode = Some(true)` | Sandbox env | URL prefix from `build_env_specific_endpoint` test branch; request accepted |
| Processor rejection | Sandbox triggers decline | `ErrorResponse` surfaced via `build_error_response`; `payout_status` unset |
| Malformed response body | Mocked non-JSON 2xx | `ConnectorError::ResponseDeserializationFailed` |

Integration tests MUST describe real sandbox flows (`PATTERN_AUTHORING_SPEC.md` §11).

## Cross-References

- Parent index: [./README.md](./README.md)
- Authoring spec: [./PATTERN_AUTHORING_SPEC.md](./PATTERN_AUTHORING_SPEC.md)
- Sibling flow: [pattern_payout_create.md](./pattern_payout_create.md)
- Sibling flow: [pattern_payout_get.md](./pattern_payout_get.md)
- Utility helpers: [../utility_functions_reference.md](../utility_functions_reference.md)
- Hand-written reference: `crates/integrations/connector-integration/src/payout_connectors/itaubank.rs`
- Macro-driven reference: `crates/integrations/connector-integration/src/payout_connectors/gotyme_sanlam.rs`
- Registry: `crates/integrations/connector-integration/src/payout_connectors.rs`
