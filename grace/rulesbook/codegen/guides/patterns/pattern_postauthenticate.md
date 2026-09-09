# PostAuthenticate Flow Pattern

> ## Which authentication mechanism is this?
>
> UCS carries **three distinct, non-interchangeable authentication mechanisms**. Conflating them is
> the single most common codegen failure in this area. This file documents **mechanism A only**.
>
> | Mechanism | Flow markers | `resource_common_data` | gRPC service |
> | --- | --- | --- | --- |
> | **A. Standalone 3DS trio** (this file) | `PreAuthenticate` / `Authenticate` / `PostAuthenticate` | `PaymentFlowData` | `PaymentMethodAuthenticationService` |
> | B. In-payment 3DS | none — folded into `Authorize` | `PaymentFlowData` | `PaymentService.Authorize` |
> | C. Merchant / credential auth | `ServerAuthenticationToken` / `ServerSessionAuthenticationToken` / `ClientAuthenticationToken` | **`MerchantAuthenticationFlowData`** | `MerchantAuthenticationService` |
>
> The real trait bindings for mechanism A are in `crates/types-traits/interfaces/src/connector_types.rs`
> (`pub trait PaymentPreAuthenticateV2`, `PaymentAuthenticateV2`, `PaymentPostAuthenticateV2`) — each a
> supertrait of `ConnectorIntegrationV2<connector_flow::<Marker>, PaymentFlowData, Payments<Marker>Data<T>, PaymentsResponseData>`.
> Mechanism C binds `MerchantAuthenticationFlowData` instead
> (`crates/types-traits/domain_types/src/merchant_authentication_flow_data.rs`), which deliberately
> omits every payment field. Never substitute one for the other.
>
> Two further things are **not** mechanism A:
> - `crates/integrations/connector-integration/src/authenticator_connectors/plaid` — bank-account
>   linking, not 3DS at all.
> - **External 3DS** (3dsecure.io, GPayments, Cardinal, Click-to-Pay, and Netcetera when used as a
>   router-side authenticator) runs entirely inside the Hyperswitch router and never reaches UCS. Do
>   not generate UCS flows for that class. The `netcetera` connector *in this repo* is a different
>   thing — a UCS authentication-only payment connector; see the roster below.
>
> **The legs do not run unless the connector opts in.** `ValidationTrait::next_authentication_step`
> (`crates/types-traits/interfaces/src/connector_types.rs`) defaults to
> `AuthenticationStep::Authorize` — i.e. skip the entire trio. A connector that implements this flow
> but does not override that method has an **unreachable** implementation. The dispatch loop lives in
> `process_composite_authorize` (`crates/internal/composite-service/src/payments.rs`) and is
> documented in **[pattern_authentication_dispatch.md](./pattern_authentication_dispatch.md)** — read
> it before wiring any leg of the trio.

## Overview

PostAuthenticate is the closing leg of the 3D Secure (3DS) trio. It runs after the browser has completed the ACS challenge kicked off by [`Authenticate`](./pattern_authenticate.md): the router feeds the returned CRes back to the connector so it can validate the signature and hand back the final `AuthenticationData` payload (CAVV, ECI, DS-Trans-ID, XID, EMV 3DS message version) which is subsequently carried into the regular Authorize call.

The flow is keyed off `domain_types::connector_flow::PostAuthenticate` and produces `PaymentsResponseData::PostAuthenticateResponse`. It is the only place in the trio where a response is expected to always carry a populated `AuthenticationData` on success; there is no redirect path out of PostAuthenticate. Connectors that collapse the full trio into two legs (Worldpay's `3dsChallenges` endpoint, Nexixpay's `/orders/3steps/validation`) use PostAuthenticate as the single validation step.

### Key Components
- Flow marker: `pub struct PostAuthenticate` in `crates/types-traits/domain_types/src/connector_flow.rs`.
- Request type: `pub struct PaymentsPostAuthenticateData<T>` in `crates/types-traits/domain_types/src/connector_types.rs` — **11 fields**, the narrowest of the trio.
- Response type: the `PostAuthenticateResponse { .. }` variant of `pub enum PaymentsResponseData` in the same file — **3 fields**, and no `resource_id`.
- Resource common data: `pub struct PaymentFlowData` in the same file.
- Trait implemented by connectors: `pub trait PaymentPostAuthenticateV2<T>` in `crates/types-traits/interfaces/src/connector_types.rs`, a supertrait of
  `ConnectorIntegrationV2<connector_flow::PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>`.
- Dispatch gate: `ValidationTrait::next_authentication_step` — see [pattern_authentication_dispatch.md](./pattern_authentication_dispatch.md).

> **Citation policy for this file.** Every reference below anchors to a **symbol name** inside a
> named file, never to a line number. Locate anything cited here with `rg "<symbol>" <path>`.

## Table of Contents

1. [Overview](#overview)
2. [3DS Flow Sequence](#3ds-flow-sequence)
3. [Architecture Overview](#architecture-overview)
4. [Connectors with Full Implementation](#connectors-with-full-implementation)
5. [Common Implementation Patterns](#common-implementation-patterns)
6. [Connector-Specific Patterns](#connector-specific-patterns)
7. [Code Examples](#code-examples)
8. [Integration Guidelines](#integration-guidelines)
9. [Best Practices](#best-practices)
10. [Common Errors / Gotchas](#common-errors--gotchas)
11. [Testing Notes](#testing-notes)
12. [Cross-References](#cross-references)

## 3DS Flow Sequence

PostAuthenticate is the third and final step. It consumes the browser's CRes redirect payload carried in `redirect_response` and produces the canonical `AuthenticationData` that will be consumed by Authorize.

```
          ┌──────────────────┐
          │ PreAuthenticate  │  (see pattern_preauthenticate.md)
          │  DDC / 3DS method│
          └────────┬─────────┘
                   ▼
          ┌──────────────────┐
          │ Authenticate     │  (see pattern_authenticate.md)
          │  enrolment check │
          └────────┬─────────┘
                   │  browser completes ACS challenge, returns CRes
                   ▼
          ┌──────────────────┐
          │ PostAuthenticate │  validate CRes → AuthenticationData
          │ (this flow)      │
          └────────┬─────────┘
                   │  final CAVV/ECI/trans_status
                   ▼
          ┌──────────────────┐
          │ Authorize        │  (pattern_authorize.md) — uses AuthenticationData
          └──────────────────┘
```

Inputs — `PaymentsPostAuthenticateData<T>` in `crates/types-traits/domain_types/src/connector_types.rs`. **All 11 fields, in declaration order:**

1. `payment_method_data: Option<PaymentMethodData<T>>`
2. `amount: MinorUnit`
3. `email: Option<Email>`
4. `currency: Option<Currency>`
5. `payment_method_type: Option<PaymentMethodType>`
6. `router_return_url: Option<Url>`
7. `continue_redirection_url: Option<Url>`
8. `browser_info: Option<BrowserInformation>`
9. `enrolled_for_3ds: bool`
10. `redirect_response: Option<ContinueRedirectionResponse>` — CRes payload returned by the browser.
11. `capture_method: Option<common_enums::CaptureMethod>`

This is the same first eleven fields as `PaymentsPreAuthenticateData` **minus** `mandate_reference`,
`merchant_transaction_id` and `metadata`, and the same first eleven as `PaymentsAuthenticateData`
**minus** `authentication_data`, `webhook_url`, `domain_data`, `sdk_information` and
`device_channel`. Concretely: `PaymentsPostAuthenticateData` **does NOT** carry an incoming
`authentication_data`. The connector's job here is to *produce* the final `AuthenticationData`, not
to consume one. It also has no `sdk_information` / `device_channel`, so an app-channel result
lookup must carry whatever it needs on the connector's own request body or in `redirect_response`.

Outputs — the `PostAuthenticateResponse` variant of `PaymentsResponseData`. **All 3 fields, in declaration order:**

1. `authentication_data: Option<router_request_types::AuthenticationData>` — the final 3DS2 payload.
2. `connector_response_reference_id: Option<String>`
3. `status_code: u16`

This variant has neither `resource_id` nor `redirection_data` — the two fields authors most often
try to add out of muscle memory from `PreAuthenticateResponse` (5 fields, `resource_id` first) and
`AuthenticateResponse` (6 fields). PostAuthenticate is the terminal step of the trio; carry the
transaction id on `connector_response_reference_id`.

## Architecture Overview

### Flow Hierarchy

```
ConnectorIntegrationV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>
│
├── build_request_v2 ── POST {base_url}{postauth_endpoint}
│     └── transforms PaymentsPostAuthenticateData<T> → <Connector>PostAuthenticateRequest
│        (usually carries authenticationTransactionId from the challenge)
│
└── handle_response_v2
      └── <Connector>PostAuthenticateResponse → PaymentsResponseData::PostAuthenticateResponse
             └── authentication_data (final CAVV/ECI/XID)
```

### Flow Type

```rust
// crates/types-traits/domain_types/src/connector_flow.rs
#[derive(Debug, Clone)]
pub struct PostAuthenticate;
```

### Request Type

Verbatim from HEAD — **11 fields**:

```rust
// crates/types-traits/domain_types/src/connector_types.rs
#[derive(Debug, Clone)]
pub struct PaymentsPostAuthenticateData<T: PaymentMethodDataTypes> {
    pub payment_method_data: Option<PaymentMethodData<T>>,
    pub amount: MinorUnit,
    pub email: Option<Email>,
    pub currency: Option<Currency>,
    pub payment_method_type: Option<PaymentMethodType>,
    pub router_return_url: Option<Url>,
    pub continue_redirection_url: Option<Url>,
    pub browser_info: Option<BrowserInformation>,
    pub enrolled_for_3ds: bool,
    pub redirect_response: Option<ContinueRedirectionResponse>,
    pub capture_method: Option<common_enums::CaptureMethod>,
}
```

The `impl<T: PaymentMethodDataTypes> PaymentsPostAuthenticateData<T>` block below the struct adds
**two** helpers, not one:

- `is_auto_capture` — same semantics as the Authorize one.
- `get_redirect_response_payload(&self) -> Result<common_utils::pii::SecretSerdeValue, Error>` —
  reaches `redirect_response.as_ref().and_then(|res| res.payload.to_owned())` and otherwise raises
  `IntegrationError::MissingRequiredField { field_name: "request.redirect_response.payload", .. }`.
  Note the field name in that error is the fully-qualified `"request.redirect_response.payload"`,
  and it reads `.payload` — the JSON blob — not `.params`, the urlencoded string. `ContinueRedirectionResponse`
  carries both: `params: Option<Secret<String>>` and `payload: Option<SecretSerdeValue>`. Pick the one
  your gateway actually returns.

### Response Type

Verbatim from HEAD — **3 fields**:

```rust
// crates/types-traits/domain_types/src/connector_types.rs — pub enum PaymentsResponseData
PostAuthenticateResponse {
    authentication_data: Option<router_request_types::AuthenticationData>,
    connector_response_reference_id: Option<String>,
    status_code: u16,
},
```

`pub struct AuthenticationData` is the shared 3DS container in
`crates/types-traits/domain_types/src/router_request_types.rs`. It has **17** fields — not the 11
that older revisions of these documents listed — and derives **no `Default`**, so `..Default::default()`
in an `AuthenticationData` literal will not compile. The verbatim field list is reproduced in
[pattern_preauthenticate.md § Response Type](./pattern_preauthenticate.md#response-type). The eight
fields PostAuthenticate is normally expected to populate:

| Field | Meaning after PostAuthenticate |
| --- | --- |
| `trans_status` | EMV 3DS `transStatus` (Y/A/N/U/C/D/I/R). |
| `eci` | Electronic Commerce Indicator returned by the ACS. |
| `cavv` | Cardholder Authentication Verification Value (liability-shift proof). |
| `threeds_server_transaction_id` | XID equivalent — identifies this EMV 3DS transaction. |
| `ds_trans_id` | Directory-Server transaction id. |
| `message_version` | EMV 3DS message version (`2.1.0`, `2.2.0`, `2.3.0`). |
| `ucaf_collection_indicator` | Mastercard-specific UCAF presence flag. |
| `exemption_indicator` | SCA exemption if the acquirer requested one. |

### Resource Common Data

`pub struct PaymentFlowData` (`crates/types-traits/domain_types/src/connector_types.rs`) — the same struct used by every payment flow, and *not* `MerchantAuthenticationFlowData` (mechanism C; see the banner at the top of this file). Transformers set `resource_common_data.status` from the connector's CRes validation outcome: `AuthenticationSuccessful` on Y/A with valid CAVV, `AuthenticationFailed` on N/R, or `AuthenticationPending` when the connector signals a need for a second challenge (rare at this stage). Nexixpay's PostAuthenticate response transformer (`connectors/nexixpay/transformers.rs`, the `TryFrom<ResponseRouterData<NexixpayPostAuthenticateResponse, Self>>` impl) is the working reference — but read the wildcard warning under [Code Examples](#code-examples) before copying its status match.

## Connectors with Full Implementation

**Roster refreshed against HEAD.** Earlier revisions of this file listed three connectors; there are
**7**. Regenerate with:

```bash
rg -n "flow_name: PostAuthenticate" crates/integrations/connector-integration/src/connectors/*.rs
```

| Connector | Transport | URL | Request type | Response type | Overrides `next_authentication_step`? |
| --- | --- | --- | --- | --- | --- |
| Barclaycard | `Json`, POST | `{base}/risk/v1/authentication-results` | `BarclaycardAuthValidateRequest<T>` | `BarclaycardPostAuthenticateResponse` | **yes** — canonical full-trio reference |
| Cybersource | `Json`, POST | `{base}risk/v1/authentication-results` | `CybersourceAuthValidateRequest<T>` | `CybersourcePostAuthenticateResponse` (imported as `CybersourceAuthenticateResponse as CybersourcePostAuthenticateResponse` in `connectors/cybersource.rs`) | yes |
| Getnet | `Json`, POST | `{base}/dpm/security-gwproxy/v2/validations` | `GetnetPostAuthenticateRequest` | `GetnetPostAuthenticateResponse` (`= GetnetThreeDsResponse`) | yes |
| Moneris | `Json`, POST | `{base}/three-d-secure/authentication-value-lookups` | `MonerisPostAuthenticateRequest` | `MonerisPostAuthenticateResponse` | yes |
| Netcetera | `Json`, POST | `{base}/3ds/results` | `NetceteraPostAuthenticateRequest` | `NetceteraPostAuthenticateResponse` | yes |
| Nexixpay | `Json`, POST | `{base}/orders/3steps/validation` | `NexixpayPostAuthenticateRequest<T>` | `NexixpayPostAuthenticateResponse` | no |
| Worldpay | `Json`, POST | `{base}api/payments/{link_data}/3dsChallenges` | `WorldpayPostAuthenticateRequest` (`= WorldpayAuthenticateRequest`) | `WorldpayPostAuthenticateResponse` (`= WorldpayPaymentsResponse`) | no |

Unlike the other two legs, every PostAuthenticate implementation is a plain
`macro_connector_implementation!` with `http_method: Post` — there is no local-flow variant and no
`Get` here.

Three of the seven reuse an existing type rather than declaring a fresh one: Cybersource re-imports
its Authenticate response under a PostAuthenticate alias (the payload shape is identical), Getnet
aliases `GetnetThreeDsResponse`, and Worldpay aliases both its request (`WorldpayAuthenticateRequest`)
and its response (`WorldpayPaymentsResponse`, i.e. the ordinary payment-state payload). Aliasing is
the idiomatic move when the gateway returns the same body — declare the alias in `requests.rs` /
`responses.rs` so the macro wiring stays greppable, rather than naming the underlying type in the
macro.

### The two that do not override `next_authentication_step`

Nexixpay and Worldpay do not override `ValidationTrait::next_authentication_step`. The trait default
returns `AuthenticationStep::Authorize`, so the composite authorize loop
(`process_composite_authorize` in `crates/internal/composite-service/src/payments.rs`) never
dispatches their PostAuthenticate leg; it is reachable only through the direct
`PaymentMethodAuthenticationService.PostAuthenticate` RPC, whose handler does not consult the
dispatch hook. **A newly generated connector with a PostAuthenticate leg and no override has an
unreachable flow.** See [pattern_authentication_dispatch.md](./pattern_authentication_dispatch.md).

### Connectors that declare the flow as not-implemented / not-supported

There is no hand-written "empty impl" idiom in this repo any more. `rg -c "PaymentPostAuthenticateV2"
crates/integrations/connector-integration/src/connectors/{stripe,checkout,revolv3}.rs` returns 0 for
each — the earlier claim in this file that they ship empty bodies is stale. A connector that does not
support the leg declares it through `macros::macro_connector_flow_status_impls!`, listing
`PostAuthenticate` under `not_implemented:` (a coverage gap, could be built) or `not_supported:` (the
gateway has no such concept). Revolv3 (`connectors/revolv3.rs`) lists all three trio markers under
`not_supported:`.

NMI, Redsys, Ilixium, Paysafe, Saferpay and Worldpayxml have no `flow: PostAuthenticate` tuple in
`create_all_prerequisites!` and no `flow_name: PostAuthenticate` macro block: their 3DS handshake
terminates earlier. That is a legitimate shape; see Pattern C below.

## Common Implementation Patterns

### Pattern A — Dedicated validation endpoint (Cybersource, Nexixpay)

The connector exposes a discrete "validate CRes" endpoint. The request carries the challenge transaction id; the response carries the final CAVV/ECI. This is the cleanest pattern and the one the `PaymentsResponseData::PostAuthenticateResponse` shape is designed around.

Cybersource posts `consumer_authentication_information: CybersourceConsumerAuthInformationValidateRequest` to `risk/v1/authentication-results`. Nexixpay posts a body containing the PaRes and operation id to `/orders/3steps/validation` and reads back `three_ds_auth_result`. Barclaycard, Getnet, Moneris and Netcetera are the same shape against their own endpoints (see the roster table).

### Pattern B — Challenge-completion as part of the main resource (Worldpay)

Worldpay scopes `3dsChallenges` under the payment resource (`api/payments/{link_data}/3dsChallenges`). The POST body is the urlencoded CRes the browser returned, deserialised straight out of `redirect_response.params` by `serde_urlencoded::from_str` in the `TryFrom<..> for WorldpayPostAuthenticateRequest` impl (`connectors/worldpay/transformers.rs`, marked `// PostAuthenticate request transformer (for 3dsChallenges)`). The `link_data` fragment carries the original payment reference so no separate correlation id is needed.

### Pattern C — Skipped (Redsys, NMI)

Some connectors produce final `AuthenticationData` already at the Authenticate step and do not expose a dedicated validation endpoint. In that case do not add a macro wiring for `PostAuthenticate` and do not add the `PaymentPostAuthenticateV2<T>` marker: list `PostAuthenticate` in that connector's `macros::macro_connector_flow_status_impls!` call under `not_implemented:` or `not_supported:`. Redsys, NMI, Ilixium, Paysafe, Saferpay and Worldpayxml are the examples at HEAD.

## Connector-Specific Patterns

### Cybersource

- Request type `pub struct CybersourceAuthValidateRequest<T>` (`connectors/cybersource/transformers.rs`) carries exactly four fields: `payment_information: PaymentInformation<T>`, `client_reference_information: ClientReferenceInformation`, `consumer_authentication_information: CybersourceConsumerAuthInformationValidateRequest`, and `order_information: OrderInformation` — note `OrderInformation`, not the `OrderInformationWithBill` that the Authenticate request uses.
- Response type is re-used from Authenticate via the import alias `CybersourceAuthenticateResponse as CybersourcePostAuthenticateResponse` in `connectors/cybersource.rs` — a deliberate choice because Cybersource returns the same payload shape for both stages. A dedicated `TryFrom<ResponseRouterData<CybersourceAuthenticateResponse, Self>>` targeting `RouterDataV2<.., PaymentsPostAuthenticateData<T>, ..>` lives in `connectors/cybersource/transformers.rs`.
- `AttemptStatus` is derived from `info_response.status` (same enum as Authenticate); terminal mapping produces `AuthenticationSuccessful` / `AuthenticationFailed`.

### Nexixpay

- Request `NexixpayPostAuthenticateRequest` is a small JSON envelope carrying the PaRes-equivalent and `operationId`.
- Response `pub struct NexixpayPostAuthenticateResponse` (`connectors/nexixpay/transformers.rs`) carries exactly two fields: `operation: NexixpayOperation` and `three_ds_auth_result: Option<NexixpayThreeDSAuthResult>` (`#[serde(rename = "threeDSAuthResult")]`). `NexixpayThreeDSAuthResult` is five all-optional strings: `authentication_value`, `eci`, `xid`, `status`, `version`.
- The `TryFrom<ResponseRouterData<NexixpayPostAuthenticateResponse, Self>>` impl in the same file populates `PostAuthenticateResponse.authentication_data`: `cavv <- authentication_value.map(Secret::new)`, `eci <- eci`, `threeds_server_transaction_id <- xid`, `trans_status <- status.parse::<TransactionStatus>().ok()`, `message_version <- version.parse::<SemanticVersion>().ok()`, and `transaction_id <- operation.operation_id` (the Authorize leg reads the operationId back out of there).
- PaRes is read directly from `redirect_response` in the subsequent Authorize flow, so `ds_trans_id` is intentionally left `None` (see the `// PaRes now read directly from redirect_response in Authorize` comment inside the `TryFrom<ResponseRouterData<NexixpayPostAuthenticateResponse, Self>>` impl).

### Worldpay

- `pub type WorldpayPostAuthenticateRequest = WorldpayAuthenticateRequest` (`connectors/worldpay/requests.rs`). The body is not synthesised from `PaymentsPostAuthenticateData` — the `TryFrom<..> for WorldpayPostAuthenticateRequest` impl in `connectors/worldpay/transformers.rs` deserialises the browser's CRes form POST out of `redirect_response.params` into that struct.
- URL construction in the `flow_name: PostAuthenticate` macro block (`connectors/worldpay.rs`) reuses `Self::extract_link_data_from_metadata(req)?` to pick up the payment reference.
- `pub type WorldpayPostAuthenticateResponse = WorldpayPaymentsResponse` (`connectors/worldpay/response.rs`) — i.e. the same payment-state payload used everywhere else. The CRes validation outcome is signalled via the same `outcome` field as Authorize.

### External vs native 3DS

- **Native** (Cybersource, Nexixpay, Worldpay): the connector owns the full trio; UCS invokes PostAuthenticate against its dedicated endpoint.
- **External** (Revolv3 is the in-repo example): `PostAuthenticate` is declared under `not_supported:` in that connector's `macro_connector_flow_status_impls!` call (`connectors/revolv3.rs`); there is no hand-written empty impl. An external authenticator produces the final `AuthenticationData` and the caller feeds it into `PaymentsAuthorizeData<T>` directly. See [pattern_preauthenticate.md § External vs native 3DS](./pattern_preauthenticate.md#external-vs-native-3ds) for the cross-cutting policy, including the rule that the whole external-3DS product class (3dsecure.io, GPayments, Cardinal, CTP, Netcetera-as-router-plugin) never enters UCS at all.

## Code Examples

### 1. Prerequisites tuple (Nexixpay)

```rust
// connectors/nexixpay.rs — create_all_prerequisites! flows list
(
    flow: PostAuthenticate,
    request_body: NexixpayPostAuthenticateRequest,
    response_body: NexixpayPostAuthenticateResponse,
    router_data: RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>,
),
```

### 2. URL wiring (Nexixpay)

```rust
// connectors/nexixpay.rs — other_functions of the flow_name: PostAuthenticate macro block
fn get_url(
    &self,
    req: &RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>,
) -> CustomResult<String, IntegrationError> {
    Ok(format!("{}/orders/3steps/validation", self.connector_base_url_payments(req)))
}
```

### 3. Response transformer producing `AuthenticationData` (Nexixpay)

Verbatim from HEAD. Note that **every one of the 17 `AuthenticationData` fields is spelled out** —
the struct has no `Default` impl, so the `..Default::default()` shorthand that an earlier revision of
this document showed here does not compile:

```rust
// connectors/nexixpay/transformers.rs
// impl TryFrom<ResponseRouterData<NexixpayPostAuthenticateResponse, Self>>
Ok(Self {
    response: Ok(PaymentsResponseData::PostAuthenticateResponse {
        authentication_data: response.three_ds_auth_result.as_ref().map(|auth_result| {
            AuthenticationData {
                trans_status: auth_result
                    .status
                    .as_ref()
                    .and_then(|s| s.parse::<common_enums::TransactionStatus>().ok()),
                eci: auth_result.eci.clone(),
                cavv: auth_result.authentication_value.clone().map(Secret::new),
                ucaf_collection_indicator: None,
                threeds_server_transaction_id: auth_result.xid.clone(),
                message_version: auth_result
                    .version
                    .as_ref()
                    .and_then(|v| v.parse::<common_utils::types::SemanticVersion>().ok()),
                // PaRes now read directly from redirect_response in Authorize
                ds_trans_id: None,
                acs_transaction_id: None,
                // CRITICAL FIX: Store operationId in transaction_id for Authorize flow
                transaction_id: Some(operation.operation_id.clone()),
                exemption_indicator: None,
                network_params: None,
                created_at: None,
                challenge_code: None,
                challenge_cancel: None,
                challenge_code_reason: None,
                message_extension: None,
                authentication_type: None,
            }
        }),
        connector_response_reference_id: Some(operation.order_id.clone()),
        status_code: item.http_code,
    }),
    resource_common_data: PaymentFlowData {
        status, // mapped from operation.operation_result
        ..item.router_data.resource_common_data
    },
    ..item.router_data
})
```

### 4. Request body from browser urlencoded form (Worldpay)

```rust
// connectors/worldpay/transformers.rs — TryFrom<..> for WorldpayPostAuthenticateRequest
let params = item
    .router_data
    .request
    .redirect_response
    .as_ref()
    .and_then(|redirect_response| redirect_response.params.as_ref())
    .ok_or(IntegrationError::MissingRequiredField {
        field_name: "redirect_response.params",
        context: Default::default(),
    })?;

let parsed_request = serde_urlencoded::from_str::<Self>(params.peek()).change_context(
    IntegrationError::BodySerializationFailed { context: Default::default() },
)?;
```

### 5. Status mapping from connector result (Nexixpay)

```rust
// connectors/nexixpay/transformers.rs — TryFrom<ResponseRouterData<NexixpayPostAuthenticateResponse, Self>>
let status = match &operation.operation_result {
    NexixpayPaymentStatus::ThreedsValidated => AttemptStatus::AuthenticationSuccessful,
    NexixpayPaymentStatus::ThreedsFailed => AttemptStatus::AuthenticationFailed,
    NexixpayPaymentStatus::Declined | NexixpayPaymentStatus::DeniedByRisk => {
        AttemptStatus::AuthenticationFailed
    }
    _ => AttemptStatus::AuthenticationPending,
};
```

> **Do not copy the `_ =>` arm into new code.** This is quoted verbatim from HEAD, and the
> wildcard is the one thing about it a reviewer will reject in a new connector. Two halves are
> required: (1) the connector status enum carries `#[serde(other)] Unknown` at the
> deserialization layer, and (2) the status-mapping `match` is exhaustive over that enum with an
> explicit `Unknown` arm and no wildcard. Exemplars: `TravelhubResult` /
> `pub enum TravelhubResult` (which carries `#[serde(other)]`) and `fn map_travelhub_status` in
> `connectors/travelhub/transformers.rs`. Nexixpay itself does the exhaustive thing in its main
> `impl From<NexixpayPaymentStatus> for AttemptStatus` (`connectors/nexixpay/transformers.rs`) —
> copy that one, not this one. Note also that the
> wildcard here lands on the NON-terminal `AuthenticationPending`, which is the only reason it
> is survivable; a wildcard that lands on `Failure` or `Charged` is a bug.

### 6. Cybersource request type (shares `PaymentInformation` with PreAuth/Auth)

```rust
// connectors/cybersource/transformers.rs
pub struct CybersourceAuthValidateRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    payment_information: PaymentInformation<T>,
    client_reference_information: ClientReferenceInformation,
    consumer_authentication_information: CybersourceConsumerAuthInformationValidateRequest,
    order_information: OrderInformation,
}
```

## Integration Guidelines

1. **Declare the trait.** `impl<...> connector_types::PaymentPostAuthenticateV2<T> for <Connector><T> {}` in the connector's main file — a marker; the `ConnectorIntegrationV2` impl behind it comes from the macro. For connectors that terminate 3DS at Authenticate (Redsys) or use external 3DS (Revolv3), skip the marker and list `PostAuthenticate` in `macro_connector_flow_status_impls!` instead.
2. **Register the flow.** Add `(flow: PostAuthenticate, request_body: ..., response_body: ..., router_data: RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>)` to `create_all_prerequisites!`.
3. **Emit the `macro_connector_implementation!`** with `flow_name: PostAuthenticate`, `http_method: Post`, and `flow_request: PaymentsPostAuthenticateData<T>`.
4. **Implement `get_url`** to point at the connector's CRes validation endpoint.
5. **Write the request `TryFrom`.** Read the browser CRes out of `request.redirect_response.params`; if your connector needs the authentication transaction id, grab it from the `connector_feature_data` your Authenticate transformer stashed.
6. **Write the response `TryFrom`.** Populate `PaymentsResponseData::PostAuthenticateResponse.authentication_data` with a fully-filled `AuthenticationData`. Map `cavv`, `eci`, `threeds_server_transaction_id`, `ds_trans_id`, `trans_status`, and `message_version` at minimum.
7. **Drive `resource_common_data.status`** from the connector's validation result — `AuthenticationSuccessful`, `AuthenticationFailed`, or `AuthenticationPending` if a second challenge is required. Never hardcode.
8. **Correlate with Authorize.** The final `AuthenticationData` is what downstream Authorize will embed in its card-payer-auth block; ensure `connector_response_reference_id` lines up with the id the subsequent Authorize will receive so PSync can tie everything back together. `pub enum ResponseId` (`crates/types-traits/domain_types/src/connector_types.rs`) has variants `ConnectorTransactionId(String)`, `EncodedData(String)`, `#[default] NoResponseId` — but note this response variant has no `resource_id` slot to put one in.
9. **Override `next_authentication_step`.** Without it the trait default returns `AuthenticationStep::Authorize` and the composite authorize loop never dispatches this leg. Five of the seven implementations override it; the two that do not (Nexixpay, Worldpay) are reachable only via the direct RPC. See [pattern_authentication_dispatch.md](./pattern_authentication_dispatch.md).

## Best Practices

- Populate at least `trans_status`, `cavv`, `eci`, and `threeds_server_transaction_id` on the `AuthenticationData` payload; downstream Authorize expects them present. The contract is `pub struct AuthenticationData` in `crates/types-traits/domain_types/src/router_request_types.rs` — 17 fields, no `Default`, so write all of them out explicitly and set the ones you cannot fill to `None`.
- Fail fast if the connector returns an empty CRes validation payload — a successful 2xx with all fields `None` is almost always an upstream misconfiguration. Return `ConnectorError`.
- Parse `message_version` via `str::parse::<common_utils::types::SemanticVersion>()` rather than string comparisons — see the `message_version:` line in Nexixpay's PostAuthenticate response transformer.
- Wrap `cavv` in `Secret<String>` — it is PII-like data. `AuthenticationData.cavv` is typed `Option<Secret<String>>` (`crates/types-traits/domain_types/src/router_request_types.rs`). `message_extension` is likewise `Option<Secret<serde_json::Value>>`.
- When a connector emits no dedicated PostAuthenticate endpoint, do not invent one. Redsys's Authenticate response transformer (`connectors/redsys/transformers.rs`) already emits final `AuthenticationData`; declare `PostAuthenticate` in `macro_connector_flow_status_impls!` and stop there.
- Persist `connector_response_reference_id` consistently across Authorize, PreAuthenticate, Authenticate and PostAuthenticate so that PSync (see [pattern_psync.md](./pattern_psync.md)) can stitch the full lifecycle together.

## Common Errors / Gotchas

1. **Problem:** `IntegrationError::MissingRequiredField { field_name: "redirect_response.params" }`.
   **Solution:** The CRes urlencoded payload must be forwarded via `PaymentsPostAuthenticateData.redirect_response.params`. `TryFrom<..> for WorldpayPostAuthenticateRequest` (`connectors/worldpay/transformers.rs`) assumes this convention. Note the sibling helper `PaymentsPostAuthenticateData::get_redirect_response_payload` reads `redirect_response.payload` (a `SecretSerdeValue`) rather than `.params` and raises `MissingRequiredField { field_name: "request.redirect_response.payload" }` — two different fields, two different error strings; read which one your connector actually wants.
2. **Problem:** Authorize fails with "missing CAVV" right after a successful PostAuthenticate.
   **Solution:** Ensure `PaymentsResponseData::PostAuthenticateResponse.authentication_data.cavv` is populated; downstream Authorize reads it from there (the `cavv:` line of Nexixpay's PostAuthenticate response transformer).
3. **Problem:** `PaymentsResponseData::PostAuthenticateResponse` missing `resource_id`.
   **Solution:** This variant intentionally has only three fields and no `resource_id` — unlike `PreAuthenticateResponse` (5 fields, `resource_id` first) and `AuthenticateResponse` (6 fields, `resource_id` first). Carry the transaction id on `connector_response_reference_id`, or on `AuthenticationData.transaction_id` as Nexixpay does with its `operationId`; PSync stitches using those.
4. **Problem:** Downstream PSync cannot correlate with the authenticated payment.
   **Solution:** Populate `connector_response_reference_id` with the same id that Authorize will emit, and make sure Authorize's `resource_id` and PSync's `connector_transaction_id` align. Nexixpay stashes its `operationId` on `PaymentFlowData.preprocessing_id` at PreAuthenticate (grep `preprocessing_id: Some(operation.operation_id.clone())`) and on `AuthenticationData.transaction_id` here.
5. **Problem:** Reaching for a request-side error variant (`MissingRequiredField`, `NotImplemented`, `InvalidDataFormat`, …) on `ConnectorError` — **E0599**.
   **Solution:** The two error enums in `crates/types-traits/domain_types/src/errors.rs` split by phase. `pub enum ConnectorError` has exactly five, all response-side, variants: `ResponseDeserializationFailed { context }`, `ResponseHandlingFailed { context }`, `UnexpectedResponseError { context }`, `IntegrityCheckFailed { context, field_names, connector_transaction_id }`, `ConnectorErrorResponse(Box<ErrorResponse>)`. Everything request-side (`MissingRequiredField { field_name, context }`, `NotImplemented(..)`, `InvalidDataFormat { .. }`, `BodySerializationFailed { context }`, …) belongs to `IntegrationError` in the same file. Use `ConnectorError` when parsing the CRes response and `IntegrationError` when building the request.
6. **Problem:** `common_enums::TransactionStatus` parsing fails on lowercase strings.
   **Solution:** Normalise the case on the connector payload before calling `.parse::<TransactionStatus>()`. Nexixpay accepts the value as-is because the gateway returns the EMV 3DS single-letter codes verbatim. `pub enum TransactionStatus` lives in `crates/common/common_enums/src/enums.rs` and has eight variants, each `#[serde(rename = ..)]`d to its letter: `Y` Success, `N` Failure, `U` VerificationNotPerformed, `A` NotVerified, `R` Rejected, `C` ChallengeRequired, `D` ChallengeRequiredDecoupledAuthentication, `I` InformationOnly. `Failure` is the `#[default]`.
7. **Problem:** Authoring a PostAuthenticate impl for a connector whose trio is only two legs.
   **Solution:** Do not register a macro wiring and do not add the marker trait. Declare `PostAuthenticate` in `macros::macro_connector_flow_status_impls!` under `not_implemented:` or `not_supported:` (Redsys, `connectors/redsys.rs`).

## Testing Notes

### Unit tests

- Request `TryFrom`: assert that the CRes is lifted out of `redirect_response.params` (Worldpay) or the `authenticationTransactionId` is pulled from metadata (Cybersource).
- Response `TryFrom` (success): assert `authentication_data.is_some()` and that `cavv`, `eci`, `threeds_server_transaction_id`, `message_version` round-trip into UCS types.
- Response `TryFrom` (failure): assert `resource_common_data.status == AuthenticationFailed` and the error path is emitted via `ErrorResponse`, not the retired `ConnectorError`.

### Integration scenarios

| Scenario | Inputs | Expected |
| --- | --- | --- |
| Successful CRes validation (Nexixpay `ThreedsValidated`) | Completed ACS challenge with valid CRes | `PostAuthenticateResponse.authentication_data` fully populated (CAVV, ECI, XID, version); `AttemptStatus::AuthenticationSuccessful`. |
| Issuer denial (Nexixpay `ThreedsFailed`) | CRes with trans_status `N` | `authentication_data.trans_status == Some(TransactionStatus::Failure)`; `AttemptStatus::AuthenticationFailed`. |
| Cybersource `authentication-results` validation | Completed challenge, valid `authentication_transaction_id` | `authentication_data.cavv.is_some()`, `eci.is_some()`; status `AuthenticationSuccessful`. |
| Worldpay `3dsChallenges` | Browser POST of urlencoded CRes into `redirect_response.params` | Status derived from Worldpay `outcome` field; `authentication_data` populated from the successful payment response. |
| Missing `redirect_response.params` | Caller forgets to forward CRes | `IntegrationError::MissingRequiredField` surfaced to the caller. |

### Sandbox requirements

- Cybersource: "Payer Auth" sandbox enabled; test authentication transaction ids are returned by the Authenticate step.
- Nexixpay: sandbox `xpay` credentials; the three-step flow must be enabled on the merchant profile.
- Worldpay: sandbox must be configured with `dsNotificationUrl` pointing at the router's redirect endpoint.

## Appendix A — Trio shapes across every connector at HEAD

Not every connector exposes three dedicated HTTP endpoints for 3DS. This table is the union of the
three rosters in this pattern set, refreshed against HEAD. "—" means the flow marker appears in that
connector's `macro_connector_flow_status_impls!` list rather than in a
`macro_connector_implementation!` block. The `Dispatch` column is
`ValidationTrait::next_authentication_step`: **no** means the leg is unreachable from
`process_composite_authorize` and only callable through the direct
`PaymentMethodAuthenticationService` RPC.

| Connector | Pre | Auth | Post | Dispatch | Shape |
| --- | --- | --- | --- | --- | --- |
| Barclaycard | yes | yes | yes | yes | Three-leg: `authentication-setups` → `authentications` → `authentication-results`. The canonical full-trio reference. |
| Cybersource | yes | yes | yes | yes | Three-leg, same endpoint triple as Barclaycard. |
| Getnet | yes | yes | yes | yes | Three-leg: `enrolments-initial` → `enrolments-continue` → `validations`. |
| Netcetera | yes | yes | yes | yes | Three-leg: `/3ds/versioning` → `/3ds/authentication` → `/3ds/results`. Authentication-**only** connector — stub `Authorize`. |
| Moneris | yes | — | yes | yes | Two-leg: `/three-d-secure/authentications` then `/three-d-secure/authentication-value-lookups`. |
| Redsys | yes | yes | — | yes | Two-leg: `/iniciaPeticionREST` bootstrap, `/trataPeticionREST` handles authenticate + authorize. |
| Paysafe | yes | yes (GET) | — | yes | Two-leg: `v1/paymenthandles` POST, then a **GET** re-fetch of the handle after the ACS redirect. |
| Kount | yes (local) | — | — | yes | Single-leg, **no outbound call**: `macro_connector_local_flow_implementation!` emits a DDC script. FRM connector. |
| Worldpayxml | yes (local) | — | — | yes | Single-leg local DDC, same macro as Kount. |
| Flywire | — | yes | — | yes | Not 3DS: `Authenticate` is a hosted-checkout session; requires `domain_data`. |
| Grabpay | — | yes | — | yes | Not 3DS: `Authenticate` is a wallet `charge/init`. |
| Nexixpay | yes | — | yes | **no** | Two-leg: `/orders/3steps/init` returns the ACS URL; validation at `/orders/3steps/validation`. |
| Worldpay | yes | — | yes | **no** | Two-leg: `/3dsDeviceData` for DDC, `/3dsChallenges` for CRes validation. |
| NMI | yes (vault) | — | — | **no** | Single-leg: the customer-vault add replaces the trio for cards that pre-bind to a customer. |
| Ilixium | yes | — | — | **no** | Single-leg against `/direct/auth`. |
| Saferpay | yes | — | — | **no** | Single-leg against `/Payment/v1/Transaction/Initialize`. |
| Revolv3 | — | — | — | n/a | Zero-leg: all three markers under `not_supported:`; external 3DS injects `AuthenticationData` into Authorize. |

Two readings to take away:

1. **PostAuthenticate is the final step only in the three-leg case.** In two-leg connectors it is
   where the browser returns and the connector validates. In single-leg or zero-leg connectors, do
   not register a macro wiring at all.
2. **A `no` in the Dispatch column is a bug in anything newly generated.** It is tolerable in the
   older connectors listed above because their integrations predate the dispatch hook; it is not a
   template to copy. See [pattern_authentication_dispatch.md](./pattern_authentication_dispatch.md).

Regenerate the first three columns with:

```bash
rg -n "flow_name: (Pre|Post)?Authenticate,?" crates/integrations/connector-integration/src/connectors/*.rs
rg -ln "fn next_authentication_step" crates/integrations/connector-integration/src/connectors/*.rs
```

## Appendix B — Status mapping reference

Final `AttemptStatus` after PostAuthenticate should be one of:

| `AttemptStatus` | When to emit |
| --- | --- |
| `AuthenticationSuccessful` | CRes validated; `trans_status` is `Y` (Success) or `A` (NotVerified — attempted with liability shift); CAVV present. See the `NexixpayPaymentStatus::ThreedsValidated` arm of Nexixpay's PostAuthenticate status match. |
| `AuthenticationFailed` | `trans_status` is `N` (Failure) or `R` (Rejected); or connector explicitly denied. See the `ThreedsFailed` / `Declined` / `DeniedByRisk` arms of the same match. |
| `AuthenticationPending` | Connector requires a second decoupled challenge (rare); or validation response ambiguous. |

Never emit `Started`, `Authorizing`, `Charged`, or any non-auth status from PostAuthenticate — the subsequent Authorize is the only flow that is allowed to transition to `Authorized`/`Charged`. Spec §11 at `grace/rulesbook/codegen/guides/patterns/PATTERN_AUTHORING_SPEC.md` bans hardcoding.

## Appendix C — Correlation id chain

PostAuthenticate sits in the middle of a larger correlation chain. Each step MUST preserve one connector-side identifier so that PSync can trace back through the full lifecycle:

```
Authorize (initial) → PreAuthenticate → Authenticate → PostAuthenticate → Authorize (final) → PSync
         \_____________________________ same correlation id ___________________________/
```

How connectors carry this id:

- **Cybersource** — the `id` field on `ClientAuthCheckInfoResponse` flows through to `consumer_authentication_information.authentication_transaction_id` on `CybersourceConsumerAuthInformationValidateRequest`, consumed by the PostAuthenticate request. `connector_response_reference_id` is then set from `info_response.client_reference_information.code`. Both live in `connectors/cybersource/transformers.rs`.
- **Nexixpay** — `operationId` persisted in `PaymentFlowData.preprocessing_id` at PreAuthenticate (grep `preprocessing_id: Some(operation.operation_id.clone())` in `connectors/nexixpay/transformers.rs`) and consumed at PostAuthenticate when building the `/validation` body, then re-emitted on `AuthenticationData.transaction_id`.
- **Worldpay** — `link_data` URL fragment stored in `connector_feature_data` at Authorize time; consumed by `Self::extract_link_data_from_metadata(req)?` (a `member_functions` helper in Worldpay's `create_all_prerequisites!`) at every 3DS URL builder in `connectors/worldpay.rs`.

The carrier chain, stated once and correctly: `connector_metadata` → `PaymentsSyncData.connector_feature_data` → the gRPC `connector_feature_data` field. `PaymentsSyncData` has **no** `connector_meta` field — `get_connector_meta()` is an accessor that returns `connector_feature_data`. And there is no `encoded_data` field on the authorize response. Do not write either name.

Never re-invent this scheme per-flow; reuse whatever correlation token your PreAuthenticate / Authenticate transformers already set.

## Cross-References

- Parent index: [./README.md](./README.md)
- Sibling 3DS flows: [pattern_preauthenticate.md](./pattern_preauthenticate.md), [pattern_authenticate.md](./pattern_authenticate.md)
- Sibling flow (non-3DS): [pattern_authorize.md](./pattern_authorize.md)
- Follow-on flow that consumes the AuthenticationData: [pattern_authorize.md](./pattern_authorize.md), with PSync correlation described in [pattern_psync.md](./pattern_psync.md)
- Dispatch — **read before wiring any leg**: [pattern_authentication_dispatch.md](./pattern_authentication_dispatch.md)
- PM pattern (shares 3DS prose; do not edit): [authorize/card/pattern_authorize_card.md](./authorize/card/pattern_authorize_card.md) — find its "3D Secure Pattern" heading.
- Authoring spec: [./PATTERN_AUTHORING_SPEC.md](./PATTERN_AUTHORING_SPEC.md)
- Types used by this flow, by symbol:
  - `pub struct PaymentsPostAuthenticateData`, `pub struct ContinueRedirectionResponse`, `pub struct PaymentFlowData`, `pub enum ResponseId`, `pub enum PaymentsResponseData` (variant `PostAuthenticateResponse`) — `crates/types-traits/domain_types/src/connector_types.rs`
  - `pub struct AuthenticationData` — `crates/types-traits/domain_types/src/router_request_types.rs`
  - `pub enum TransactionStatus` — `crates/common/common_enums/src/enums.rs`
  - `pub struct PostAuthenticate` — `crates/types-traits/domain_types/src/connector_flow.rs`
  - `pub trait PaymentPostAuthenticateV2`, `pub enum AuthenticationStep`, `fn next_authentication_step` — `crates/types-traits/interfaces/src/connector_types.rs`
