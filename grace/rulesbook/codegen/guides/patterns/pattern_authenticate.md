# Authenticate Flow Pattern

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

Authenticate is the middle leg of the 3D Secure (3DS) trio. After [`PreAuthenticate`](./pattern_preauthenticate.md) has obtained device-data-collection (DDC) output and a 3DS method completion signal, the Authenticate flow runs the actual enrolment lookup and — when the issuer demands it — surfaces a CReq/ACS challenge form to the browser. On success it either produces an already-authenticated `AuthenticationData` payload (frictionless 3DS2) or a `RedirectForm` for the browser challenge whose completion is reported back through [`PostAuthenticate`](./pattern_postauthenticate.md).

This is the flow most sensitive to connector terminology: gateways label it "authentications", "enrolment check", "payer auth check" or "3DS2 lookup". The UCS contract normalises all of these on `domain_types::connector_flow::Authenticate` plus `PaymentsResponseData::AuthenticateResponse`.

### Key Components
- Flow marker: `pub struct Authenticate` in `crates/types-traits/domain_types/src/connector_flow.rs`.
- Request type: `pub struct PaymentsAuthenticateData<T>` in `crates/types-traits/domain_types/src/connector_types.rs` — **16 fields**, the widest of the trio.
- Response type: the `AuthenticateResponse { .. }` variant of `pub enum PaymentsResponseData` in the same file — **6 fields**.
- Resource common data: `pub struct PaymentFlowData` in the same file.
- Trait implemented by connectors: `pub trait PaymentAuthenticateV2<T>` in `crates/types-traits/interfaces/src/connector_types.rs`, a supertrait of
  `ConnectorIntegrationV2<connector_flow::Authenticate, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>`.
- Dispatch gate: `ValidationTrait::next_authentication_step` — see [pattern_authentication_dispatch.md](./pattern_authentication_dispatch.md).

> **Citation policy for this file.** Every reference below anchors to a **symbol name** inside a
> named file, never to a line number. Locate anything cited here with `rg "<symbol>" <path>`.

## Table of Contents

1. [Overview](#overview)
2. [3DS Flow Sequence](#3ds-flow-sequence)
3. [Architecture Overview](#architecture-overview)
4. [Fields the older docs omitted](#fields-the-older-docs-omitted)
5. [Connectors with Full Implementation](#connectors-with-full-implementation)
6. [Common Implementation Patterns](#common-implementation-patterns)
7. [Connector-Specific Patterns](#connector-specific-patterns)
8. [Code Examples](#code-examples)
9. [Integration Guidelines](#integration-guidelines)
10. [Best Practices](#best-practices)
11. [Common Errors / Gotchas](#common-errors--gotchas)
12. [Testing Notes](#testing-notes)
13. [Cross-References](#cross-references)

## 3DS Flow Sequence

Authenticate is the middle step; it consumes the correlation id/DDC output that PreAuthenticate produced and either closes out the 3DS handshake frictionlessly or emits a browser challenge whose completion lands in PostAuthenticate.

```
          ┌──────────────────┐
          │ PreAuthenticate  │  (see pattern_preauthenticate.md)
          │  DDC + 3DS method│
          └────────┬─────────┘
                   │  browser completes DDC, returns threeds_method_comp_ind
                   ▼
          ┌──────────────────┐
          │ Authenticate     │  issuer lookup / ACS challenge decision
          │ (this flow)      │
          └────────┬─────────┘
                   │  frictionless → AuthenticationData (CAVV/ECI)
                   │  challenge    → RedirectForm (CReq POST)
                   ▼
          ┌──────────────────┐
          │ PostAuthenticate │  validate CRes, normalise AuthenticationData
          │  (see pattern_postauthenticate.md)
          └────────┬─────────┘
                   ▼
          ┌──────────────────┐
          │ Authorize        │  (pattern_authorize.md)
          └──────────────────┘
```

Inputs — `PaymentsAuthenticateData<T>` in `crates/types-traits/domain_types/src/connector_types.rs`. **All 16 fields, in declaration order:**

1. `payment_method_data: Option<PaymentMethodData<T>>` — the card under authentication. `Option`, because a post-redirect leg (Paysafe's handle re-fetch) carries no card.
2. `amount: MinorUnit`
3. `email: Option<Email>`
4. `currency: Option<Currency>`
5. `payment_method_type: Option<PaymentMethodType>`
6. `router_return_url: Option<Url>`
7. `continue_redirection_url: Option<Url>`
8. `browser_info: Option<BrowserInformation>` — required by 3DS2 browser-channel.
9. `enrolled_for_3ds: bool` — note that the gRPC request builder in `crates/types-traits/domain_types/src/types.rs` hardcodes this to `false` for this flow; do not read decisions out of it.
10. `redirect_response: Option<ContinueRedirectionResponse>` — populated when the browser has returned with 3DS method completion.
11. `capture_method: Option<common_enums::CaptureMethod>`
12. `authentication_data: Option<router_request_types::AuthenticationData>` — DDC/3DS-method data carried in from the previous leg (`threeds_server_transaction_id`, etc.).
13. `webhook_url: Option<String>` — see [Fields the older docs omitted](#fields-the-older-docs-omitted).
14. `domain_data: Option<DomainData>` — vertical-specific payload (education / airline). Same section.
15. `sdk_information: Option<SdkInformation>` — EMV 3DS 2.x **app-channel** SDK block. Same section.
16. `device_channel: Option<DeviceChannel>` — `App` vs `Browser`. Same section.

Its `impl<T: PaymentMethodDataTypes> PaymentsAuthenticateData<T>` block carries three helpers —
`is_auto_capture`, `get_browser_info` (→ `missing_field_err("browser_info")`), and
`get_continue_redirection_url` (→ `missing_field_err("continue_redirection_url")`). These exist on
**this** type only. `PaymentsPreAuthenticateData` has just `is_auto_capture`;
`PaymentsPostAuthenticateData` has `is_auto_capture` **and** `get_redirect_response_payload`
(which reads `redirect_response.payload`, a `SecretSerdeValue` — not `.params`) — see
[pattern_postauthenticate.md § Request Type](./pattern_postauthenticate.md#request-type).

Outputs — the `AuthenticateResponse` variant of `PaymentsResponseData`. **All 6 fields, in declaration order:**

1. `resource_id: Option<ResponseId>` — connector-side transaction reference for this authentication attempt.
2. `redirection_data: Option<Box<RedirectForm>>` — carries the inline comment `/// For friction flow`.
3. `authentication_data: Option<router_request_types::AuthenticationData>` — inline comment `/// For frictionles flow` (sic, typo is in HEAD).
4. `connector_feature_data: Option<serde_json::Value>` — inline comment `/// Connector specific feature data (e.g. cybersource 3DS data) surfaced to HS RouterData`. **This field is absent from older drafts of this document.** It is the carrier that lands on `PaymentsSyncData.connector_feature_data` downstream.
5. `connector_response_reference_id: Option<String>`
6. `status_code: u16`

## Architecture Overview

### Flow Hierarchy

```
ConnectorIntegrationV2<Authenticate, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>
│
├── build_request_v2 ── POST {base_url}{authenticate_endpoint}
│     └── transforms PaymentsAuthenticateData<T> → <Connector>AuthenticateRequest
│       (usually carries CardInformation + order info + consumer_auth_info.return_url)
│
└── handle_response_v2
      └── <Connector>AuthenticateResponse → PaymentsResponseData::AuthenticateResponse
             ├── authentication_data (frictionless path)
             └── redirection_data    (challenge path)
```

### Flow Type

```rust
// crates/types-traits/domain_types/src/connector_flow.rs
#[derive(Debug, Clone)]
pub struct Authenticate;
```

### Request Type

Verbatim from HEAD — **16 fields**:

```rust
// crates/types-traits/domain_types/src/connector_types.rs
#[derive(Debug, Clone)]
pub struct PaymentsAuthenticateData<T: PaymentMethodDataTypes> {
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
    pub authentication_data: Option<router_request_types::AuthenticationData>,
    pub webhook_url: Option<String>,
    /// Domain-specific data (e.g. student fields) for connectors that need it.
    pub domain_data: Option<DomainData>,
    pub sdk_information: Option<SdkInformation>,
    pub device_channel: Option<DeviceChannel>,
}
```

The `impl` block immediately below adds `is_auto_capture`, `get_browser_info`, and
`get_continue_redirection_url`.

### Response Type

Verbatim from HEAD — **6 fields**, including the `connector_feature_data` carrier that earlier
drafts of this file left out:

```rust
// crates/types-traits/domain_types/src/connector_types.rs — pub enum PaymentsResponseData
AuthenticateResponse {
    resource_id: Option<ResponseId>,
    /// For friction flow
    redirection_data: Option<Box<RedirectForm>>,
    /// For frictionles flow
    authentication_data: Option<router_request_types::AuthenticationData>,
    /// Connector specific feature data (e.g. cybersource 3DS data) surfaced to HS RouterData
    connector_feature_data: Option<serde_json::Value>,
    connector_response_reference_id: Option<String>,
    status_code: u16,
},
```

`AuthenticationData` (`crates/types-traits/domain_types/src/router_request_types.rs`) is the same
struct used across the whole trio and has **17** fields. Its full, verbatim field list is reproduced
in [pattern_preauthenticate.md § Response Type](./pattern_preauthenticate.md#response-type); do not
work from the abbreviated 11-field list that older revisions of these documents carried, and note
that it derives no `Default`.

On the carrier chain: `connector_feature_data` set here is what a later PSync reads through
`PaymentsSyncData.connector_feature_data`, which is surfaced by the accessor
`PaymentsSyncData::get_connector_meta()`. There is **no** `connector_meta` field on
`PaymentsSyncData`, and there is no `encoded_data` field on the authorize response — do not write
either name.

### Resource Common Data

`pub struct PaymentFlowData` (`crates/types-traits/domain_types/src/connector_types.rs`) — the same
struct used by every payment flow, and *not* `MerchantAuthenticationFlowData` (mechanism C; see the
banner at the top of this file). Authenticate transformers map the connector's authentication status
onto `AttemptStatus`, producing three outcomes: `AuthenticationPending` (challenge required),
`AuthenticationSuccessful` (frictionless Y/A), or `AuthenticationFailed` (issuer denial). See the
`TryFrom<ResponseRouterData<CybersourceAuthenticateResponse, Self>>` impl in
`connectors/cybersource/transformers.rs`.

## Fields the older docs omitted

Four fields on `PaymentsAuthenticateData<T>` were missing from every previous revision of this
document. Three of them are the EMV 3DS 2.x **app-channel** inputs; without them an app-based
authentication cannot be built at all.

### `device_channel` — which EMV 3DS channel this is

```rust
// crates/types-traits/domain_types/src/connector_types.rs
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum DeviceChannel {
    #[serde(rename = "APP")]
    App,
    #[serde(rename = "BRW")]
    Browser,
}
```

Two variants only. The wire values are the EMV 3DS strings `"APP"` and `"BRW"` — **not** the numeric
`"01"` / `"02"` codes some gateway specs use; if your connector wants the numerics, map them in the
connector's own enum, do not change this one. Note this type is `Copy`; `SdkInformation` below is not.

The branch it drives, from Netcetera's Authenticate request transformer
(`connectors/netcetera/transformers.rs`):

```rust
let is_app = matches!(
    request.device_channel,
    Some(domain_types::connector_types::DeviceChannel::App)
);

let browser_information = if is_app { None } else {
    request.browser_info.clone().map(netcetera_types::Browser::from)
};
let sdk_information = if is_app {
    request.sdk_information.clone().map(netcetera_types::Sdk::from)
} else { None };
```

`browser_info` and `sdk_information` are **mutually exclusive**: send browser info on `Browser`,
SDK info on `App`, never both. Netcetera additionally emits `device_render_options` only in the
`App` branch.

### `sdk_information` — the app-channel SDK block

```rust
// crates/types-traits/domain_types/src/connector_types.rs
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SdkInformation {
    pub sdk_app_id: String,
    pub sdk_enc_data: String,
    pub sdk_ephem_pub_key: std::collections::HashMap<String, String>,
    pub sdk_trans_id: String,
    pub sdk_reference_number: String,
    pub sdk_max_timeout: u8,
    pub sdk_type: Option<SdkType>,
    pub device_details: Option<DeviceDetails>,
}
```

Eight fields. Six are **required** (not `Option`) — `sdk_app_id`, `sdk_enc_data`,
`sdk_ephem_pub_key`, `sdk_trans_id`, `sdk_reference_number`, `sdk_max_timeout`. Two details that bite:

- `sdk_ephem_pub_key` is a `HashMap<String, String>` — the JWK is carried as a flat string map, not a
  typed key or a `serde_json::Value`.
- `sdk_max_timeout` is a `u8`. EMV 3DS specifies minutes with a minimum of 05; a value above 255 is
  not representable, so do not try to store seconds in it.

### `SdkType` and `DeviceDetails`

```rust
// crates/types-traits/domain_types/src/connector_types.rs
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum SdkType {
    #[serde(rename = "01")] DefaultSdk,
    #[serde(rename = "02")] SplitSdk,
    #[serde(rename = "03")] LimitedSdk,
    #[serde(rename = "04")] BrowserSdk,
    #[serde(rename = "05")] ShellSdk,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeviceDetails {
    pub device_type: Option<String>,
    pub device_brand: Option<String>,
    pub device_os: Option<String>,
    pub device_display: Option<String>,
}
```

`SdkType` serialises to the two-digit EMV codes `"01".."05"`. Five variants — match exhaustively;
`connectors/netcetera/netcetera_types.rs` carries `impl From<domain_types::connector_types::SdkType>
for SdkType` as a complete five-arm match, and `impl From<SdkInformation> for Sdk` defaults an absent
`sdk_type` to `DefaultSdk`. Copy that shape.

`DeviceDetails` is four all-optional `String`s. It is *not* the same thing as `BrowserInformation`
and must not be filled from it.

### These are not top-level gRPC fields

`sdk_information` and `device_channel` do **not** arrive as proto fields on
`PaymentMethodAuthenticationServiceAuthenticateRequest`. The request builder in
`crates/types-traits/domain_types/src/types.rs` parses them out of `request.metadata` through a
private helper struct:

```rust
// crates/types-traits/domain_types/src/types.rs
#[derive(serde::Deserialize)]
struct AuthenticateSdkMetadata {
    device_channel: Option<connector_types::DeviceChannel>,
    sdk_information: Option<connector_types::SdkInformation>,
}
```

with `sdk_information: value.metadata.as_ref().and_then(|m| serde_json::from_str::<AuthenticateSdkMetadata>(m.peek()).ok()).and_then(|m| m.sdk_information)`.
The parse is `.ok()`-swallowed: malformed metadata yields `None` silently, not an error. If an
app-channel authentication mysteriously behaves as browser-channel, suspect the metadata JSON shape
before suspecting the connector.

### `domain_data` — vertical payloads

```rust
// crates/types-traits/domain_types/src/connector_types.rs
#[derive(Debug, Clone, Default)]
pub struct DomainData {
    pub airline_data: Option<AirlineData>,
    pub education_data: Option<EducationData>,
}
```

Two verticals at HEAD. `EducationData` holds `student_details: Option<StudentDetails>`, and
`StudentDetails` holds `student_id`, `student_first_name`, `student_last_name` (all
`Option<String>`) plus `student_email: Option<Secret<String>>`.

Flywire is the consumer: its `TryFrom<..> for FlywireCheckoutSessionRequest`
(`connectors/flywire/transformers.rs`) treats `domain_data` as **mandatory**, raising
`IntegrationError::MissingRequiredField { field_name: "domain_data", .. }` with a
`suggested_action` of "Pass `domain_data` on the request." and a `doc_url`, then routes it through
the free function `domain_data_to_recipient_fields`. That function errors on
`"domain_data.education_data.student_details"` when the vertical sub-object is absent.

### `webhook_url`

Plumbed straight through — `webhook_url: value.webhook_url` in the request builder — but **no
connector transformer reads it on this flow at HEAD**
(`rg webhook_url crates/integrations/connector-integration/src/connectors/*/transformers.rs` returns
no Authenticate-flow hit). Treat it as available plumbing, not as an established pattern; if your
gateway needs an async-authentication notification URL, this is the field to read, and you will be
the first.

## Connectors with Full Implementation

**Roster refreshed against HEAD.** Earlier revisions of this file said "only two native
implementations". There are **8**. Regenerate with:

```bash
rg -n "flow_name: Authenticate," crates/integrations/connector-integration/src/connectors/*.rs
```

| Connector | Transport | URL | Request type | Response type | Kind |
| --- | --- | --- | --- | --- | --- |
| Barclaycard | `Json`, POST | `{base}/risk/v1/authentications` | `BarclaycardAuthEnrollmentRequest<T>` | `BarclaycardAuthenticateResponse` | 3DS enrolment |
| Cybersource | `Json`, POST | `{base}risk/v1/authentications` | `CybersourceAuthEnrollmentRequest<T>` | `CybersourceAuthenticateResponse` | 3DS enrolment |
| Flywire | `Json`, POST | `{base}/payments/v1/checkout/sessions` | `FlywireCheckoutSessionRequest` | `FlywireCheckoutSessionResponse` | **not 3DS** — hosted checkout session |
| Getnet | `Json`, POST | `{base}/dpm/security-gwproxy/v2/enrolments-continue` | `GetnetAuthenticateRequest` | `GetnetAuthenticateResponse` | 3DS enrolment |
| Grabpay | `Json`, POST | `{base}` + `CHARGE_INIT_PATH` (`/charge/init`) | `GrabpayAuthenticateRequest` | `GrabpayAuthenticateResponse` | **not 3DS** — wallet charge init |
| Netcetera | `Json`, POST | `{base}/3ds/authentication` | `NetceteraAuthenticateRequest<T>` | `NetceteraAuthenticateResponse` | 3DS enrolment (authentication-only connector) |
| Paysafe | `Json`, **GET** | `{base}v1/paymenthandles?merchantRefNum={connector_request_reference_id}` | `PaysafeAuthenticateRequest` | `PaysafeAuthenticateResponse` (`= PaysafeSyncResponse`) | post-ACS handle re-fetch |
| Redsys | `Json`, POST | `{base}/sis/rest/trataPeticionREST` | `RedsysAuthenticateRequest` (`= RedsysTransaction`) | `RedsysAuthenticateResponse` | 3DS enrolment |

All eight override `ValidationTrait::next_authentication_step`, which is what makes their legs
reachable from the composite authorize loop. See
[pattern_authentication_dispatch.md](./pattern_authentication_dispatch.md).

Three of these deserve loud footnotes, because copying the wrong one produces a nonsense connector:

- **`http_method: Get`.** Paysafe's Authenticate is a GET, not a POST. It re-fetches the payment
  handle after the ACS redirect and reads `paymentHandleToken` for the settling Authorize, keying on
  `resource_common_data.connector_request_reference_id` as `merchantRefNum`. Its
  `payment_method_data` is `None` on that leg — which is exactly why field 1 of
  `PaymentsAuthenticateData` is an `Option`.
- **`Authenticate` is not synonymous with 3DS.** Flywire uses this marker for a hosted-checkout
  session (and requires `domain_data`); Grabpay uses it for a wallet charge init. Both are redirect
  legs that happen to sit in the same dispatch slot. Do not assume a CAVV/ECI shape when a
  specification calls for an `Authenticate` leg — read what the gateway actually returns.
- **Netcetera** is the authentication-only connector: a stub `Authorize` whose `get_url` returns
  `IntegrationError::not_implemented("Authorize flow is not supported by the Netcetera (3DS
  authentication-only) connector", ..)`, plus real PreAuthenticate / Authenticate / PostAuthenticate.
  It lives in the **payment** connector registry, not `authenticator_connectors/`.

### Connectors that declare the flow as not-implemented / not-supported

There is no hand-written "empty impl" idiom in this repo any more, and the connectors this file used
to name as carrying one do not: `rg -c "PaymentAuthenticateV2"
crates/integrations/connector-integration/src/connectors/{stripe,checkout,revolv3}.rs` returns 0 for
each. A connector that does not support the leg declares it through
`macros::macro_connector_flow_status_impls!`, listing `Authenticate` under `not_implemented:` (could
be built, is not yet) or `not_supported:` (the gateway has no such concept). Revolv3
(`connectors/revolv3.rs`) lists all three trio markers under `not_supported:`.

Nexixpay, NMI and Worldpay simply have no `flow: Authenticate` tuple in their
`create_all_prerequisites!` block and no `flow_name: Authenticate` macro invocation — they collapse
three legs into two. That is a legitimate shape; see Pattern C below.

## Common Implementation Patterns

### Pattern A — Dedicated Authenticate endpoint (Cybersource)

Cybersource runs the strict 3-leg variant: PreAuth (`authentication-setups`) → Authenticate (`authentications`) → PostAuth (`authentication-results`). Each leg has its own URL and request/response structs. Authenticate's request `pub struct CybersourceAuthEnrollmentRequest<T>` (`connectors/cybersource/transformers.rs`) adds `consumer_authentication_information` and `order_information` on top of the two-field `CybersourceAuthSetupRequest` shape. Barclaycard's `BarclaycardAuthEnrollmentRequest<T>` is the same pattern and pairs with a `next_authentication_step` override, so prefer it as the copy source.

### Pattern B — Shared transaction body with operation-type discriminator (Redsys)

Redsys sends every non-bootstrap operation to the same `/sis/rest/trataPeticionREST` URL and distinguishes them by the `DS_MERCHANT_TRANSACTIONTYPE` field inside the request body. `pub type RedsysAuthenticateRequest = super::transformers::RedsysTransaction` (`connectors/redsys/requests.rs`, alongside five sibling aliases for Authorize / Capture / Void / Refund / PreAuthenticate), with the `TryFrom<..> for requests::RedsysAuthenticateRequest` impl in `connectors/redsys/transformers.rs` filling in the appropriate discriminator.

### Pattern C — Skipped (Nexixpay, NMI, Worldpay)

Some connectors collapse three 3DS legs into two: the challenge form is issued from PreAuthenticate and the CRes validation is done in PostAuthenticate. Nexixpay, NMI and Worldpay are the examples at HEAD. Do not add a macro wiring for a flow the connector does not need — omit the `flow: Authenticate` prerequisites tuple entirely and list the marker in `macro_connector_flow_status_impls!` under `not_implemented:` or `not_supported:`.

## Connector-Specific Patterns

### Cybersource

- `pub struct CybersourceAuthEnrollmentRequest<T>` (`connectors/cybersource/transformers.rs`) carries exactly four fields: `payment_information: PaymentInformation<T>`, `client_reference_information: ClientReferenceInformation`, `consumer_authentication_information: CybersourceConsumerAuthInformationRequest`, and `order_information: OrderInformationWithBill`.
- Response is `pub enum CybersourceAuthenticateResponse` (same file), untagged, with `ClientAuthCheckInfo(Box<ClientAuthCheckInfoResponse>)` and `ErrorInformation(Box<CybersourceErrorInformationResponse>)`. The success path inspects `info_response.consumer_authentication_information` to decide between a challenge and a frictionless outcome.
- The `TryFrom<ResponseRouterData<CybersourceAuthenticateResponse, Self>>` impl in the same file sets `AttemptStatus` from `info_response.status`.

### Redsys

- Uses the shared `RedsysTransaction` body; the Authenticate response transformer in `connectors/redsys/transformers.rs` emits either a `RedirectForm` carrying the CReq or an early `AuthenticationData` payload.
- The free function `fn to_connector_response_data<T>(..)` in the same file decrypts the base64 `DS_MERCHANT_PARAMETERS` blob and feeds it into the transformer; the decryption is shared by Authenticate and PreAuthenticate.

### External vs native 3DS

- **Native** (Cybersource, Redsys): the UCS router invokes `Authenticate` against the connector's dedicated endpoint.
- **External** (Revolv3 is the in-repo example): `Authenticate` is declared under `not_supported:` in that connector's `macro_connector_flow_status_impls!` call. The 3DS outcome (CAVV/ECI/DS-Trans-ID) is computed by an external authenticator and passed through on the Authorize call via `PaymentsAuthorizeData<T>`. See [pattern_preauthenticate.md § External vs native 3DS](./pattern_preauthenticate.md#external-vs-native-3ds) for the cross-cutting policy, including the rule that the whole external-3DS product class (3dsecure.io, GPayments, Cardinal, CTP, Netcetera-as-router-plugin) never enters UCS at all.

## Code Examples

### 1. Prerequisites tuple (Cybersource)

```rust
// connectors/cybersource.rs — create_all_prerequisites! flows list
(
    flow: Authenticate,
    request_body: CybersourceAuthEnrollmentRequest<T>,
    response_body: CybersourceAuthenticateResponse,
    router_data: RouterDataV2<Authenticate, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>,
),
```

### 2. URL wiring (Cybersource)

```rust
// connectors/cybersource.rs — other_functions of the flow_name: Authenticate macro block
fn get_url(
    &self,
    req: &RouterDataV2<Authenticate, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>,
) -> CustomResult<String, IntegrationError> {
    Ok(format!(
        "{}risk/v1/authentications",
        self.connector_base_url_payments(req)
    ))
}
```

### 3. Response → `AuthenticateResponse` with CAVV/ECI (Redsys)

```rust
// connectors/redsys/transformers.rs — Authenticate response transformer (shape, not verbatim)
Ok(PaymentsResponseData::AuthenticateResponse {
    resource_id: ..., // populated from DS_ORDER / DS_TRANSACTION_ID
    redirection_data: None,
    authentication_data: Some(AuthenticationData {
        trans_status: ..., // from DS_EMV3DS.transStatus
        eci: ...,
        cavv: ...,
        // AuthenticationData does NOT derive Default — spell out all 17 fields,
        // or build it through a connector-local helper. `..Default::default()`
        // here will not compile.
    }),
    connector_feature_data: None,   // sixth field; forgetting it is E0063
    connector_response_reference_id: ...,
    status_code: item.http_code,
})
```

### 4. Challenge branch producing a `RedirectForm` (Cybersource)

```rust
// connectors/cybersource/transformers.rs
// impl TryFrom<ResponseRouterData<CybersourceAuthenticateResponse, Self>> (shape, not verbatim)
CybersourceAuthenticateResponse::ClientAuthCheckInfo(info_response) => {
    let status = common_enums::AttemptStatus::from(info_response.status);
    // ...
    let redirection_data = match (
        info_response
            .consumer_authentication_information
            .acs_url
            .as_ref(),
        /* ... other challenge fields ... */
    ) {
        (Some(acs_url), /* challenge payload */) => Some(Box::new(RedirectForm::Form {
            endpoint: acs_url.clone(),
            method: common_utils::request::Method::Post,
            form_fields: /* CReq + TermUrl + MD */ HashMap::new(),
        })),
        _ => None,
    };
    /* assemble PaymentsResponseData::AuthenticateResponse */
}
```

The exact field-by-field CReq construction lives in that same `TryFrom` impl in `connectors/cybersource/transformers.rs`; it is not reproduced verbatim here but follows the `RedirectForm::Form` shape used everywhere in UCS. `pub enum RedirectForm` is in `crates/types-traits/domain_types/src/router_response_types.rs`; `Form`, `Script`, `Html` and `Uri` are the generic variants, the rest are connector-specific.

### 5. `get_browser_info` helper contract

```rust
// crates/types-traits/domain_types/src/connector_types.rs
// impl<T: PaymentMethodDataTypes> PaymentsAuthenticateData<T>
pub fn get_browser_info(&self) -> Result<BrowserInformation, Error> {
    self.browser_info
        .clone()
        .ok_or_else(missing_field_err("browser_info"))
}
```

### 6. Declaring the flow unsupported (Revolv3)

There is no hand-written empty impl. The flow status macro generates it:

```rust
// connectors/revolv3.rs
macros::macro_connector_flow_status_impls!(
    connector: Revolv3,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [ /* flows that could be built later */ ],
    not_supported: [
        VoidPostRefund,
        Authenticate,
        PostAuthenticate,
        PreAuthenticate,
        ClientAuthenticationToken,
    ],
);
```

`not_implemented:` means "a coverage gap, not a decision that it should never be covered";
`not_supported:` means the gateway genuinely has no such concept. Pick deliberately — the
certification tooling reads these lists.

## Integration Guidelines

1. **Declare the trait.** `impl<...> connector_types::PaymentAuthenticateV2<T> for <Connector><T> {}` in the connector's main file. It is a marker; the `ConnectorIntegrationV2` impl behind it comes from the macro. If the connector relies on external 3DS or collapses three legs into two, skip the marker and list `Authenticate` in `macro_connector_flow_status_impls!` instead.
2. **Register the flow.** Add the `(flow: Authenticate, request_body: ..., response_body: ..., router_data: RouterDataV2<Authenticate, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>)` tuple to `create_all_prerequisites!`.
3. **Emit a `macro_connector_implementation!`** with `flow_name: Authenticate`, `http_method: Post`, and `flow_request: PaymentsAuthenticateData<T>`.
4. **Implement `get_url`** to hit the connector's enrolment-check endpoint. `http_method` is `Post` for six of the eight implementations — Paysafe uses `Get` because its Authenticate leg re-fetches a handle rather than initiating anything.
5. **Write the request `TryFrom`.** Pull card data out of `payment_method_data` (remember it is `Option`), browser info via `request.get_browser_info()?` (on the `impl PaymentsAuthenticateData<T>` block in `crates/types-traits/domain_types/src/connector_types.rs`), and the DDC correlation id out of the incoming `authentication_data` field. If the gateway supports app-channel 3DS, branch on `request.device_channel` and read `request.sdk_information` — see [Fields the older docs omitted](#fields-the-older-docs-omitted).
6. **Write the response `TryFrom`.** Map to `PaymentsResponseData::AuthenticateResponse`. Populate `authentication_data` for the frictionless Y/R path and `redirection_data` (usually `RedirectForm::Form`) for the challenge path. Set `resource_common_data.status` from the connector's authentication outcome — do not hardcode.
7. **Persist the acs_transaction_id / threeds_server_transaction_id** on the `AuthenticationData` payload so PostAuthenticate can correlate the CRes with the original Authenticate call. The struct is `pub struct AuthenticationData` in `crates/types-traits/domain_types/src/router_request_types.rs`; it has 17 fields and no `Default` impl.
8. **Reuse connector-wide signing.** Cybersource's `pub fn build_headers<F, FCD, Req, Res>(..)`, declared in the `member_functions` block of its `create_all_prerequisites!` (`connectors/cybersource.rs`), handles HMAC signing uniformly across all flows; Authenticate must route through that rather than reimplementing the signature locally.
9. **Override `next_authentication_step`.** Without it, `ValidationTrait`'s default returns `AuthenticationStep::Authorize` and the composite authorize loop never reaches this flow. All eight implementations override it. See [pattern_authentication_dispatch.md](./pattern_authentication_dispatch.md).
10. **Errors.** Return `IntegrationError` for request-time failures (`MissingRequiredField` etc.) and `ConnectorError` for response parsing. `pub enum ConnectorError` (`crates/types-traits/domain_types/src/errors.rs`) has exactly FIVE variants: `ResponseDeserializationFailed { context }`, `ResponseHandlingFailed { context }`, `UnexpectedResponseError { context }`, `IntegrityCheckFailed { context, field_names, connector_transaction_id }`, `ConnectorErrorResponse(Box<ErrorResponse>)`. There is no `ConnectorResponseTransformationError` type.

## Best Practices

- Read `browser_info` via `request.get_browser_info()` so the connector surfaces `MissingRequiredField { field_name: "browser_info" }` uniformly rather than panicking (the helper is on `impl PaymentsAuthenticateData<T>` in `crates/types-traits/domain_types/src/connector_types.rs`). On the **app** channel there is no browser info at all — branch on `device_channel` first.
- Keep challenge forms keyed on the fields the ACS expects: `creq`, `threeDSSessionData`, `TermUrl`, `MD`. The generic `RedirectForm::Form { endpoint, method, form_fields }` shape accommodates all of them.
- When the connector supports both 3DS1 and 3DS2, branch inside the response transformer on the connector's `messageVersion` / `paresStatus`; do not force a single mapping. See `impl From<CybersourceParesStatus> for common_enums::TransactionStatus` in `connectors/cybersource/transformers.rs` — an exhaustive match with no wildcard.
- Use `AttemptStatus::AuthenticationSuccessful` only when `trans_status` is `Y` (Success) or `A` (`NotVerified`, attempted with liability shift). Challenge-required outcomes stay on `AuthenticationPending`. The enum is `pub enum TransactionStatus` in `crates/common/common_enums/src/enums.rs` — eight variants, read it there rather than from any markdown.
- When reusing a shared transaction struct across flows (Redsys), keep one alias per flow in `requests.rs` for grepability rather than referencing the underlying struct everywhere.
- For frictionless-only connectors, set `redirection_data: None` explicitly in the response so the router does not accidentally render a blank redirect.

## Common Errors / Gotchas

1. **Problem:** `MissingRequiredField { field_name: "browser_info" }` during Authenticate.
   **Solution:** browser-channel 3DS2 requires browser info. Use `request.get_browser_info()?`, and ensure the caller populates `PaymentsAuthenticateData.browser_info`. If `device_channel` is `App`, browser info is legitimately absent and you must read `sdk_information` instead — calling `get_browser_info()` unconditionally breaks every app-channel authentication.
2. **Problem:** Frictionless path returns CAVV but router still triggers a redirect.
   **Solution:** Place the CAVV on `authentication_data`, not `redirection_data`. The response enum documents this split in-line — `/// For friction flow` vs `/// For frictionles flow` on the `AuthenticateResponse` variant in `crates/types-traits/domain_types/src/connector_types.rs`.
3. **Problem:** PSync returns "unknown transaction" after a successful Authenticate.
   **Solution:** Populate `resource_id: Some(ResponseId::ConnectorTransactionId(..))` in the `AuthenticateResponse` so later flows can correlate. `pub enum ResponseId` lives in `crates/types-traits/domain_types/src/connector_types.rs` with variants `ConnectorTransactionId(String)`, `EncodedData(String)`, and `#[default] NoResponseId`.
4. **Problem:** Status mapping defaults to `AttemptStatus::Started` after Authenticate.
   **Solution:** Drive `resource_common_data.status` from the connector's `authenticationStatus` field. `AttemptStatus::AuthenticationPending` for challenges, `AuthenticationSuccessful` for frictionless Y/A, `AuthenticationFailed` for denial. Never hardcode (spec §11 at `grace/rulesbook/codegen/guides/patterns/PATTERN_AUTHORING_SPEC.md`).
5. **Problem:** A `ConnectorError` variant is used that does not exist (`InvalidData`, `NotImplemented(..)`, `InvalidCard`, `MissingRequiredField`, …) — **E0599**.
   **Solution:** `ConnectorError` has exactly FIVE variants and they are all response-side: `ResponseDeserializationFailed { context }`, `ResponseHandlingFailed { context }`, `UnexpectedResponseError { context }`, `IntegrityCheckFailed { context, field_names, connector_transaction_id }`, `ConnectorErrorResponse(Box<ErrorResponse>)` (`pub enum ConnectorError` in `crates/types-traits/domain_types/src/errors.rs`). The request-side variants people reach for (`MissingRequiredField { field_name, context }`, `NotImplemented(String, IntegrationErrorContext)`, `InvalidDataFormat { field_name, context }`, `FailedToObtainAuthType { context }`, …) belong to `IntegrationError` in the same file. Read the real variant list before substituting, pick the closest REAL variant, and keep the `context` field.
6. **Problem:** Connector has only two legs but pattern author wrote a full Authenticate impl.
   **Solution:** Check the connector docs — Nexixpay, NMI and Worldpay collapse Authenticate into PreAuthenticate. Do not register a `flow: Authenticate` tuple or a `flow_name: Authenticate` macro block; declare the marker under `not_implemented:` / `not_supported:` in `macro_connector_flow_status_impls!` instead (`connectors/nexixpay.rs`).

## Testing Notes

### Unit tests

- `TryFrom` for the connector's Authenticate request: assert return URL, card fields, and any `threeds_server_transaction_id` from the upstream `authentication_data` round-trip.
- `TryFrom` for the success response with challenge: assert `redirection_data.is_some()`, `authentication_data.is_none()`, and `resource_common_data.status == AuthenticationPending`.
- `TryFrom` for the frictionless response: assert `authentication_data.is_some()` with non-empty CAVV and ECI.
- Error-response path.

### Integration scenarios

| Scenario | Inputs | Expected |
| --- | --- | --- |
| Frictionless 3DS2 (Cybersource Visa test card `4456 5300 0000 1005`) | DDC completed | `AuthenticateResponse.authentication_data.trans_status == Y` with CAVV+ECI; `AttemptStatus::AuthenticationSuccessful`. |
| Challenge required (Cybersource Mastercard test card `5200 8282 8282 8210`) | DDC completed | `AuthenticateResponse.redirection_data = Some(RedirectForm::Form { endpoint: ACS_URL, form_fields: { creq, threeDSSessionData } })`; status `AuthenticationPending`. |
| Issuer denied (Redsys `ResultStatus=N`) | Invalid card | `PaymentsResponseData::AuthenticateResponse` with empty `authentication_data` and failure-mapped `AttemptStatus::AuthenticationFailed`. |
| Connector unreachable | DNS / 5xx | Error surfaced via `build_error_response`; body parse produces `ConnectorError::ResponseDeserializationFailed { context }`. |

## Appendix A — `TransactionStatus` discriminator

The `trans_status` field threaded through all three flows is a `common_enums::TransactionStatus` — `pub enum TransactionStatus` in `crates/common/common_enums/src/enums.rs`, eight variants, each carrying its EMV 3DS letter as a `#[serde(rename = ..)]`. Read the variant list there, never from a markdown copy. The mapping onto UCS status handling (cross-checked against `impl From<CybersourceParesStatus> for common_enums::TransactionStatus` in `connectors/cybersource/transformers.rs`):

| `trans_status` | Meaning | `AttemptStatus` after Authenticate |
| --- | --- | --- |
| `Success` (`Y`) | Fully authenticated. | `AuthenticationSuccessful` (frictionless path). |
| `Failure` (`N`) | Not authenticated. | `AuthenticationFailed`. |
| `NotVerified` (`A`) | Attempted (liability shift). | `AuthenticationSuccessful`. |
| `VerificationNotPerformed` (`U`) | ACS unavailable. | `AuthenticationPending`, caller decides whether to fallback. |
| `ChallengeRequired` (`C`) | Issuer demands a challenge. | `AuthenticationPending` with `redirection_data = Some(..)`. |
| `ChallengeRequiredDecoupledAuthentication` (`D`) | Decoupled auth. | `AuthenticationPending` with decoupled redirect form. |
| `InformationOnly` (`I`) | Info-only, no liability shift. | `AuthenticationSuccessful` (no CAVV). |
| `Rejected` (`R`) | Issuer rejected the authentication. | `AuthenticationFailed`. |

## Appendix B — Field map: connector payload → `AuthenticationData`

When writing the response `TryFrom`, the following connector fields typically feed into the canonical `pub struct AuthenticationData` (`crates/types-traits/domain_types/src/router_request_types.rs`, 17 fields):

| `AuthenticationData` field | Source in Cybersource | Source in Redsys |
| --- | --- | --- |
| `trans_status` | `consumer_authentication_information.pares_status` | `DS_EMV3DS.transStatus` |
| `eci` | `consumer_authentication_information.eci` | `DS_EMV3DS.eci` |
| `cavv` | `consumer_authentication_information.cavv` (wrap in `Secret::new`) | `DS_EMV3DS.authValue` |
| `ucaf_collection_indicator` | `consumer_authentication_information.ucaf_collection_indicator` | `DS_EMV3DS.ucafCollectionIndicator` |
| `threeds_server_transaction_id` | `consumer_authentication_information.xid` | `DS_EMV3DS.threeDSServerTransID` |
| `ds_trans_id` | `consumer_authentication_information.directory_server_transaction_id` | `DS_EMV3DS.dsTransID` |
| `acs_transaction_id` | `consumer_authentication_information.acs_transaction_id` | `DS_EMV3DS.acsTransID` |
| `message_version` | `consumer_authentication_information.specification_version` — already typed `Option<SemanticVersion>` on the Cybersource response struct, so no manual parse | `DS_EMV3DS.protocolVersion`, parsed with `str::parse::<SemanticVersion>()` (`SemanticVersion` implements `FromStr`; there is **no** `SemanticVersion::parse` associated function) |
| `transaction_id` | `info_response.id` | `DS_ORDER` |
| `exemption_indicator` | acquirer-driven SCA exemption hint (rarely set) | not populated |

Do NOT fabricate values for fields the connector does not return; `Option<...>::None` is the correct default. Authorize's request transformer is resilient to missing fields.

## Appendix C — Challenge form field conventions

When Authenticate returns a challenge, the `RedirectForm::Form { endpoint, method, form_fields }` payload must follow the EMV 3DS 2.x browser-flow convention. Typical `form_fields` keys consumed by an issuer ACS:

| Key | Origin | Purpose |
| --- | --- | --- |
| `creq` | `consumer_authentication_information.pareq` (Cybersource) | Base64-URL Challenge Request payload to POST to ACS. |
| `threeDSSessionData` | opaque session hint | Opaque token used to correlate CRes with the original CReq. |
| `TermUrl` | `router_return_url` | Where the ACS POSTs the CRes back. |
| `MD` | connector transaction id | Legacy 3DS1 merchant data field; carry when present. |
| `PaReq` | Redsys `DS_MERCHANT_EMV3DS.paReq` | 3DS1 challenge request payload. |

Set `method: common_utils::request::Method::Post` — ACS endpoints universally expect form-POST. The UCS router renders this shape with an auto-submit HTML wrapper.

## Cross-References

- Parent index: [./README.md](./README.md)
- Sibling 3DS flows: [pattern_preauthenticate.md](./pattern_preauthenticate.md), [pattern_postauthenticate.md](./pattern_postauthenticate.md)
- Sibling flow (non-3DS): [pattern_authorize.md](./pattern_authorize.md)
- PSync consumes the correlation id produced here: [pattern_psync.md](./pattern_psync.md)
- Dispatch — **read before wiring any leg**: [pattern_authentication_dispatch.md](./pattern_authentication_dispatch.md)
- PM pattern (shares 3DS prose; do not edit): [authorize/card/pattern_authorize_card.md](./authorize/card/pattern_authorize_card.md) — find its "3D Secure Pattern" heading.
- Authoring spec: [./PATTERN_AUTHORING_SPEC.md](./PATTERN_AUTHORING_SPEC.md)
- Types used by this flow, by symbol:
  - `pub struct PaymentsAuthenticateData`, `pub struct SdkInformation`, `pub struct DeviceDetails`, `pub enum SdkType`, `pub enum DeviceChannel`, `pub struct DomainData`, `pub struct PaymentFlowData`, `pub enum ResponseId`, `pub enum PaymentsResponseData` (variant `AuthenticateResponse`) — `crates/types-traits/domain_types/src/connector_types.rs`
  - `pub struct AuthenticationData` — `crates/types-traits/domain_types/src/router_request_types.rs`
  - `pub enum RedirectForm` — `crates/types-traits/domain_types/src/router_response_types.rs`
  - `pub enum TransactionStatus` — `crates/common/common_enums/src/enums.rs`
  - `pub struct Authenticate` — `crates/types-traits/domain_types/src/connector_flow.rs`
  - `pub trait PaymentAuthenticateV2`, `pub enum AuthenticationStep`, `pub enum RedirectState`, `fn next_authentication_step` — `crates/types-traits/interfaces/src/connector_types.rs`
  - `struct AuthenticateSdkMetadata` (the `sdk_information` / `device_channel` metadata carrier) — `crates/types-traits/domain_types/src/types.rs`
