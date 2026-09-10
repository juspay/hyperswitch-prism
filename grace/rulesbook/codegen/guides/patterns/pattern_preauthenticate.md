# PreAuthenticate Flow Pattern

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

The PreAuthenticate flow is the first leg of the 3D Secure (3DS) authentication trio. Its job is to initiate the authentication session with the connector, collect whatever preliminary data the issuer's ACS (Access Control Server) requires, and hand back either device-data-collection (DDC) instructions or an early frictionless-success signal. The flow is keyed off `domain_types::connector_flow::PreAuthenticate` and produces `PaymentsResponseData::PreAuthenticateResponse` whose primary role is to carry a `RedirectForm` (for DDC) and/or an early `AuthenticationData` payload for the subsequent `Authenticate` step.

This flow corresponds to concepts such as "Payer Authentication Setup", "Device Data Collection initiation", or "3DS Method URL retrieval" depending on the gateway. On success the orchestrator either completes DDC via a browser-driven iframe and invokes [`Authenticate`](./pattern_authenticate.md), or (for connectors that perform enrolment lookup here) skips straight to [`PostAuthenticate`](./pattern_postauthenticate.md).

### Key Components
- Flow marker: `pub struct PreAuthenticate` in `crates/types-traits/domain_types/src/connector_flow.rs`.
- Request type: `pub struct PaymentsPreAuthenticateData<T>` in `crates/types-traits/domain_types/src/connector_types.rs` — **14 fields**.
- Response type: the `PreAuthenticateResponse { .. }` variant of `pub enum PaymentsResponseData` in `crates/types-traits/domain_types/src/connector_types.rs` — **5 fields**.
- Resource common data: `pub struct PaymentFlowData` in `crates/types-traits/domain_types/src/connector_types.rs`.
- Trait implemented by connectors: `pub trait PaymentPreAuthenticateV2<T>` in `crates/types-traits/interfaces/src/connector_types.rs`, a supertrait of
  `ConnectorIntegrationV2<connector_flow::PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>`.
- Dispatch gate: `ValidationTrait::next_authentication_step` — see [pattern_authentication_dispatch.md](./pattern_authentication_dispatch.md).

> **Citation policy for this file.** Every reference below anchors to a **symbol name** inside a
> named file, never to a line number. Line numbers in this corpus rot within days; symbol names
> survive. Locate anything cited here with `rg "<symbol>" <path>`.

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

The 3DS trio executes strictly in order. Each step depends on data produced by the previous one and ends either in a frictionless CAVV/ECI that can be passed to the regular `Authorize` flow or in a challenge redirect that must be completed in a browser context.

```
          ┌──────────────────┐
          │ PreAuthenticate  │  collect device data / 3DS method URL
          │ (this flow)      │
          └────────┬─────────┘
                   │  DDC iframe / frictionless signal
                   ▼
          ┌──────────────────┐
          │ Authenticate     │  issuer lookup: enrolment, ACS challenge
          │                  │
          └────────┬─────────┘
                   │  CReq/CRes browser challenge (if required)
                   ▼
          ┌──────────────────┐
          │ PostAuthenticate │  validate CRes, produce CAVV/ECI
          │                  │
          └────────┬─────────┘
                   │  authenticated payment data
                   ▼
          ┌──────────────────┐
          │ Authorize        │  regular payment authorization
          │ (pattern_authorize.md) │
          └──────────────────┘
```

Inputs to `PreAuthenticate` — `PaymentsPreAuthenticateData<T>` in `crates/types-traits/domain_types/src/connector_types.rs`. **All 14 fields, in declaration order:**

1. `payment_method_data: Option<PaymentMethodData<T>>` — card being authenticated.
2. `amount: MinorUnit`
3. `email: Option<Email>`
4. `currency: Option<Currency>`
5. `payment_method_type: Option<PaymentMethodType>`
6. `router_return_url: Option<Url>` — the PSync `/response` target.
7. `continue_redirection_url: Option<Url>` — the `/complete` continuation target. Use **this** one for `ReturnUrl` form fields.
8. `browser_info: Option<BrowserInformation>` — UA/screen info for 3DS2 risk scoring.
9. `enrolled_for_3ds: bool`
10. `redirect_response: Option<ContinueRedirectionResponse>` — the browser's form POST, forwarded back in (see Worldpay's DDC submit below).
11. `capture_method: Option<common_enums::CaptureMethod>`
12. `mandate_reference: Option<MandateReferenceId>` — when authenticating a stored mandate.
13. `merchant_transaction_id: Option<String>` — merchant transaction id; Kount derives the FRM DDC `sessionId` from it.
14. `metadata: Option<common_utils::pii::SecretSerdeValue>` — merchant-supplied connector metadata, mirroring `PaymentsAuthorizeData::metadata`. Carries fields with no home in the UCS payment model (Ilixium's schema-mandatory `customer.dateOfBirth`, for one).

There is **no** `authentication_data` and **no** `sdk_information` on the PreAuthenticate request — those belong to `PaymentsAuthenticateData` (see [pattern_authenticate.md](./pattern_authenticate.md)).

Outputs from `PreAuthenticate` — the `PreAuthenticateResponse` variant of `PaymentsResponseData`. **All 5 fields, in declaration order:**

1. `resource_id: Option<ResponseId>` — **declared first**, and frequently forgotten. Older drafts of this corpus claimed the variant had "exactly four fields"; that is wrong. Omitting `resource_id` from a struct-variant literal is a hard compile error (E0063).
2. `authentication_data: Option<router_request_types::AuthenticationData>` — populated when the connector already returns a CAVV/ECI (frictionless exit path).
3. `redirection_data: Option<Box<RedirectForm>>` — DDC form, 3DS method iframe, or DDC script. Carries the inline comment `/// For Device Data Collection`.
4. `connector_response_reference_id: Option<String>` — correlation id used by downstream `Authenticate` / `PostAuthenticate`.
5. `status_code: u16`

For the contract of the downstream flows see [pattern_authenticate.md](./pattern_authenticate.md) and [pattern_postauthenticate.md](./pattern_postauthenticate.md).

## Architecture Overview

### Flow Hierarchy

```
ConnectorIntegrationV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>
│
├── build_request_v2  ── POST {base_url}{preauth_endpoint}
│     └── transforms PaymentsPreAuthenticateData<T> → <Connector>PreAuthenticateRequest
│
└── handle_response_v2
      └── <Connector>PreAuthenticateResponse → PaymentsResponseData::PreAuthenticateResponse
             ├── redirection_data (DDC form)
             └── authentication_data (frictionless-exit payload)
```

### Flow Type

`PreAuthenticate` is the zero-sized marker struct declared in `crates/types-traits/domain_types/src/connector_flow.rs`:

```rust
// crates/types-traits/domain_types/src/connector_flow.rs
#[derive(Debug, Clone)]
pub struct PreAuthenticate;
```

### Request Type

`PaymentsPreAuthenticateData<T>` lives in `crates/types-traits/domain_types/src/connector_types.rs` and carries the caller inputs enumerated in [3DS Flow Sequence](#3ds-flow-sequence). Verbatim from HEAD — **14 fields**:

```rust
// crates/types-traits/domain_types/src/connector_types.rs
#[derive(Debug, Clone)]
pub struct PaymentsPreAuthenticateData<T: PaymentMethodDataTypes> {
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
    pub mandate_reference: Option<MandateReferenceId>,
    /// Merchant transaction id, used to derive the FRM DDC sessionId (e.g. Kount).
    pub merchant_transaction_id: Option<String>,
    /// Merchant-supplied connector metadata, mirroring `PaymentsAuthorizeData::metadata`.
    pub metadata: Option<common_utils::pii::SecretSerdeValue>,
}
```

The `impl<T: PaymentMethodDataTypes> PaymentsPreAuthenticateData<T>` block immediately below the
struct supplies exactly one helper, `is_auto_capture`, which mirrors the Authorize one and returns
`IntegrationError::CaptureMethodNotSupported` for `ManualMultiple` / `Scheduled`. There are **no**
`get_browser_info` / `get_continue_redirection_url` helpers on this type — those exist only on
`PaymentsAuthenticateData`.

### Response Type

`PaymentsResponseData::PreAuthenticateResponse` — a variant of the shared `pub enum PaymentsResponseData` in `crates/types-traits/domain_types/src/connector_types.rs` — has **five** fields, and `resource_id` is the **first** of them. (This corpus previously said "exactly four" and omitted `resource_id`; constructing the variant that way is E0063.) Verbatim from HEAD:

```rust
// crates/types-traits/domain_types/src/connector_types.rs — pub enum PaymentsResponseData
PreAuthenticateResponse {
    resource_id: Option<ResponseId>,
    authentication_data: Option<router_request_types::AuthenticationData>,
    /// For Device Data Collection
    redirection_data: Option<Box<RedirectForm>>,
    connector_response_reference_id: Option<String>,
    status_code: u16,
},
```

`pub struct AuthenticationData` is defined in `crates/types-traits/domain_types/src/router_request_types.rs` and is shared by all three legs of the trio. It has **17** fields (not 11, as earlier drafts claimed). Verbatim from HEAD:

```rust
// crates/types-traits/domain_types/src/router_request_types.rs
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AuthenticationData {
    pub trans_status: Option<common_enums::TransactionStatus>,
    pub eci: Option<String>,
    pub cavv: Option<Secret<String>>,
    // This is mastercard specific field
    pub ucaf_collection_indicator: Option<String>,
    pub threeds_server_transaction_id: Option<String>,
    pub message_version: Option<SemanticVersion>,
    pub ds_trans_id: Option<String>,
    pub acs_transaction_id: Option<String>,
    pub transaction_id: Option<String>,
    pub network_params: Option<NetworkParams>,
    pub exemption_indicator: Option<common_enums::ExemptionIndicator>,
    pub created_at: Option<time::PrimitiveDateTime>,
    pub challenge_code: Option<String>,
    pub challenge_cancel: Option<String>,
    pub challenge_code_reason: Option<String>,
    pub message_extension: Option<Secret<serde_json::Value>>,
    pub authentication_type: Option<common_enums::DecoupledAuthenticationType>,
}
```

`AuthenticationData` derives no `Default`; build it field-by-field, or via the connector's own
helper. `impl AuthenticationData` carries one method, `get_cavv_algorithm`, which digs the
single-digit Cartes Bancaires CAVV algorithm code out of `network_params`.

### Resource Common Data

`pub struct PaymentFlowData` (`crates/types-traits/domain_types/src/connector_types.rs`) is the
same struct used by every payment flow; authors must not redefine it, and must not reach for
`MerchantAuthenticationFlowData` (mechanism C — see the banner at the top of this file).

In PreAuthenticate, transformers set `resource_common_data.status` to
`AttemptStatus::AuthenticationPending` on the challenge/DDC path — see the
`TryFrom<ResponseRouterData<CybersourceAuthSetupResponse, Self>>` impl in
`connectors/cybersource/transformers.rs`. The one documented exception is a device-data-collection
leg that makes no outbound call, which stamps `AttemptStatus::DeviceDataCollectionPending` instead
— see `handle_pre_authenticate_response` in `connectors/kount/transformers.rs` and
[Pattern D](#pattern-d--local-flow-no-outbound-call-kount-worldpayxml).

## Connectors with Full Implementation

**Roster refreshed against HEAD.** Earlier revisions of this file listed five connectors; there are
**14**. Regenerate the list with:

```bash
rg -n "flow_name: PreAuthenticate" crates/integrations/connector-integration/src/connectors/*.rs
```

| Connector | Transport | URL | Request type | Response type | Overrides `next_authentication_step`? |
| --- | --- | --- | --- | --- | --- |
| Barclaycard | `Json`, POST | `{base}/risk/v1/authentication-setups` | `BarclaycardAuthSetupRequest<T>` | `BarclaycardAuthSetupResponse` | **yes** — canonical full-trio reference |
| Cybersource | `Json`, POST | `{base}risk/v1/authentication-setups` | `CybersourceAuthSetupRequest<T>` | `CybersourceAuthSetupResponse` | yes |
| Getnet | `Json`, POST | `{base}/dpm/security-gwproxy/v2/enrolments-initial` | `GetnetPreAuthenticateRequest` | `GetnetPreAuthenticateResponse` | yes |
| Ilixium | `Json`, POST | `{base}` + `AUTH_ENDPOINT` (`/direct/auth`) | `IlixiumPreAuthenticateRequest` | `IlixiumPreAuthenticateResponse` | no |
| Kount | **none — local flow** | n/a | n/a | n/a | yes |
| Moneris | `Json`, POST | `{base}/three-d-secure/authentications` | `MonerisPreAuthenticateRequest<T>` | `MonerisPreAuthenticateResponse` | yes |
| Netcetera | `Json`, POST | `{base}/3ds/versioning` | `NetceteraPreAuthenticateRequest<T>` | `NetceteraPreAuthenticateResponse` | yes |
| Nexixpay | `Json`, POST | `{base}/orders/3steps/init` | `NexixpayPreAuthenticateRequest<T>` | `NexixpayPreAuthenticateResponse` | no |
| NMI | `FormUrlEncoded`, POST | `{base}` + `endpoints::TRANSACT` | `NmiVaultRequest` | `NmiPreAuthenticateResponse` (`= NmiVaultResponse`) | no |
| Paysafe | `Json`, POST | `{base}v1/paymenthandles` | `PaysafePreAuthenticateRequest` | `PaysafePreAuthenticateResponse` | yes |
| Redsys | `Json`, POST | `{base}/sis/rest/iniciaPeticionREST` | `RedsysPreAuthenticateRequest` (`= RedsysTransaction`) | `RedsysPreAuthenticateResponse` | yes |
| Saferpay | `Json`, POST | `{base}` + `PATH_INITIALIZE` (`/Payment/v1/Transaction/Initialize`) | `SaferpayPreAuthenticateRequest<T>` | `SaferpayPreAuthenticateResponse` | no |
| Worldpay | `Json`, POST | `{base}api/payments/{link_data}/3dsDeviceData` | `WorldpayPreAuthenticateRequest` (`= WorldpayAuthenticateRequest`) | `WorldpayPreAuthenticateResponse` | no |
| Worldpayxml | **none — local flow** | n/a | n/a | n/a | yes |

Every URL above is quoted from the `other_functions: { fn get_url(..) }` block of that connector's
`macro_connector_implementation!` with `flow_name: PreAuthenticate` (except Kount and Worldpayxml,
which use `macro_connector_local_flow_implementation!` and therefore have no `get_url`).

### The `next_authentication_step` column is not decoration

Five of the fourteen — Ilixium, Nexixpay, NMI, Saferpay, Worldpay — do **not** override
`ValidationTrait::next_authentication_step`. The trait default returns
`AuthenticationStep::Authorize`, so the composite authorize loop
(`process_composite_authorize` in `crates/internal/composite-service/src/payments.rs`) never
dispatches their PreAuthenticate leg. Those legs are reachable **only** through the direct
`PaymentMethodAuthenticationService.PreAuthenticate` RPC, whose handler
(`internal_pre_authenticate`, generated by `implement_connector_operation!` in
`crates/grpc-server/grpc-server/src/server/payments.rs`) does not consult the dispatch hook at all.

**If you generate a new connector with a PreAuthenticate leg and do not also override
`next_authentication_step`, the leg is unreachable from the composite payment path.** This is the
single most common way a freshly generated 3DS integration "compiles and does nothing". See
[pattern_authentication_dispatch.md](./pattern_authentication_dispatch.md).

### Connectors that declare the flow as not-implemented / not-supported

There is no longer a hand-written "empty impl" idiom in this repo. A connector that does not
support a trio leg declares it through `macros::macro_connector_flow_status_impls!`, listing the
flow marker under `not_implemented:` (could be built, is not yet) or `not_supported:` (the gateway
has no such concept). Example, from `connectors/revolv3.rs`:

```rust
macros::macro_connector_flow_status_impls!(
    connector: Revolv3,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [ /* ... */ ],
    not_supported: [
        VoidPostRefund,
        Authenticate,
        PostAuthenticate,
        PreAuthenticate,
        ClientAuthenticationToken,
    ],
);
```

`rg -c "PaymentPreAuthenticateV2" crates/integrations/connector-integration/src/connectors/checkout.rs`
returns 0 — the earlier claim in this file that Checkout, Stripe, Adyen, Braintree, Paypal, Shift4
and Trustpay each carry an "empty `PaymentPreAuthenticateV2<T>` declaration" is stale and has been
removed. Do not go looking for those impls; generate the `macro_connector_flow_status_impls!` entry
instead.

`netcetera` deserves its own note: it is the **authentication-only** connector. It ships a stub
`Authorize` (whose `get_url` returns
`IntegrationError::not_implemented("Authorize flow is not supported by the Netcetera (3DS
authentication-only) connector", ..)`) plus real implementations of all three trio legs, and it
lives in the **payment** connector registry, not `authenticator_connectors/`. Use it as the
exemplar when a spec describes a pure 3DS server.

## Common Implementation Patterns

### Pattern A — Macro-based with dedicated request/response

This is the recommended path and the one used by twelve of the fourteen implementations. The flow
is registered inside `create_all_prerequisites!` with the full
`RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>`
tuple, and wiring for URL/headers is provided in a matching `macro_connector_implementation!` block
whose `flow_name:` is `PreAuthenticate`.

```rust
// connectors/redsys.rs — create_all_prerequisites! flows list
(
    flow: PreAuthenticate,
    request_body: RedsysPreAuthenticateRequest,
    response_body: RedsysPreAuthenticateResponse,
    router_data: RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
),
```

```rust
// connectors/redsys.rs — the PreAuthenticate macro block
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Redsys,
    curl_request: Json(RedsysPreAuthenticateRequest),
    curl_response: RedsysPreAuthenticateResponse,
    flow_name: PreAuthenticate,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsPreAuthenticateData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/sis/rest/iniciaPeticionREST", self.connector_base_url_payments(req)))
        }
    }
);
```

### Pattern B — Form-encoded adapter on a shared vault endpoint (NMI)

NMI does not expose a dedicated 3DS-setup endpoint. Instead it treats PreAuthenticate as "create a
customer vault entry from a card" and reuses the `endpoints::TRANSACT` URL with a
`customer_vault=add_customer` form value. Its `macro_connector_implementation!` block with
`flow_name: PreAuthenticate` (`connectors/nmi.rs`) sets `curl_request: FormUrlEncoded(NmiVaultRequest)`.
The response is a URL-encoded blob which the prerequisites-level `preprocess_response_bytes`
function — declared inside NMI's `create_all_prerequisites!` `member_functions` block
(`connectors/nmi.rs`) — re-serialises to JSON before the generated `TryFrom` runs.

### Pattern C — Dual endpoint with link-data routing (Worldpay)

Worldpay encodes the payment reference into a URL path segment returned in the previous step's
`_links` block. The connector stores that segment in `connector_feature_data` and unpacks it with
`Self::extract_link_data_from_metadata(req)` (defined in the `member_functions` block of Worldpay's
`create_all_prerequisites!`, `connectors/worldpay.rs`). The PreAuthenticate URL joins it onto
`api/payments/{}/3dsDeviceData` with `urlencoding::encode(&link_data)`. The request body is not
synthesised from `PaymentsPreAuthenticateData` directly; instead the
`TryFrom<..> for WorldpayPreAuthenticateRequest` impl in `connectors/worldpay/transformers.rs`
reads the browser's urlencoded form POST out of `redirect_response.params`.

### Pattern D — Local flow, no outbound call (Kount, Worldpayxml)

Some PreAuthenticate legs perform **device-data collection only**: they hand the browser a script or
form and never talk to the gateway. For those, `macro_connector_implementation!` is the wrong tool —
it would demand a `curl_request`, a `curl_response`, an `http_method` and a `get_url`. Use
`macros::macro_connector_local_flow_implementation!` instead. Its argument keys, verbatim from
`connectors/macros.rs`:

```rust
macros::macro_connector_local_flow_implementation!(
    connector: Kount,
    flow_name: PreAuthenticate,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsPreAuthenticateData<T>,
    flow_response: PaymentsResponseData,
    handle_response: kount::handle_pre_authenticate_response,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
);
```

There are exactly **eight** keys — `connector`, `flow_name`, `resource_common_data`, `flow_request`,
`flow_response`, `handle_response`, `generic_type`, and the trailing bounds list. There is no
`curl_request`, no `curl_response`, no `http_method`, and no `other_functions`. Passing any of those
is a macro-match failure, not a helpful error.

What the macro emits (see the doc comment above `macro_rules! macro_connector_local_flow_implementation`
in `connectors/macros.rs`):

- `get_call_connector_action` → `common_enums::CallConnectorAction::HandleResponseWithoutBuildRequest`
- `build_request_v2` → `Ok(None)` (this is what makes it "no outbound call")
- `get_url` → `Err(IntegrationError::NotImplemented(..))`, unreachable because `build_request_v2` returned `None`
- `handle_response_v2` → forwards straight to `$handle_response`

**The macro does not emit the marker-trait impl.** Its doc comment says so explicitly: "The connector
file still owns the marker-trait impl (e.g. `impl PaymentPreAuthenticateV2<G> for C<G> {}`)". Both
`connectors/kount.rs` and `connectors/worldpayxml.rs` carry
`impl<T: ..> connector_types::PaymentPreAuthenticateV2<T> for <Connector><T> {}` next to the macro
invocation. Forget it and you get E0277 at the registry.

`$handle_response` must be a path to a connector-owned free function whose signature matches
(again from the macro's doc comment):

```text
fn(data: &RouterDataV2<$flow, $resource_common_data, $request, $response>,
   event_builder: Option<&mut Event>,
   res: Response)
 -> CustomResult<RouterDataV2<$flow, $resource_common_data, $request, $response>, ConnectorError>
```

Kount's `handle_pre_authenticate_response` (`connectors/kount/transformers.rs`) ignores both
`event_builder` and `res` — there was no request, so there is no response to parse — and instead:

1. Derives a DDC `sessionId` by hashing `request.merchant_transaction_id`, falling back to
   `resource_common_data.connector_request_reference_id`. This is the one place field 13 of
   `PaymentsPreAuthenticateData` is load-bearing.
2. Reads the access token from `resource_common_data.access_token` to derive `client_id` and the
   sandbox-vs-production `environment` — **not** a hardcoded environment.
3. Stamps `resource_common_data.status = AttemptStatus::DeviceDataCollectionPending` (not
   `AuthenticationPending`).
4. Returns `PaymentsResponseData::PreAuthenticateResponse` with `resource_id: None`,
   `authentication_data: None`, `redirection_data: Some(Box::new(RedirectForm::Script { script_data }))`,
   `connector_response_reference_id` = the connector request reference id, and a synthesised
   `status_code: 200`.
5. Calls `resource_common_data.set_typed_connector_response(None)`.

Note also that Kount is registered in **both** registries in
`crates/integrations/connector-integration/src/types.rs` — `ConnectorEnum::Kount` (payments) and
`FrmConnectorEnum::Kount` (FRM, where it is the only entry) — and its PreAuthenticate leg is
reachable on the payment-method authentication service because
`implement_connector_operation!` for `internal_pre_authenticate`
(`crates/grpc-server/grpc-server/src/server/payments.rs`) declares
`connector_data_types: [ConnectorData, FrmConnectorData]`. Do not infer from this that arbitrary FRM
flows are reachable on the payment services.

## Connector-Specific Patterns

### Cybersource

- Dedicated request type `pub struct CybersourceAuthSetupRequest<T>` (`connectors/cybersource/transformers.rs`) with exactly two fields, `payment_information` and `client_reference_information`. No amount or order info is posted at this stage — those belong to `Authenticate`.
- Response is the `#[serde(untagged)] pub enum CybersourceAuthSetupResponse` (same file) with two variants: `ClientAuthSetupInfo(Box<ClientAuthSetupInfoResponse>)` — carrying `access_token`, `device_data_collection_url`, `reference_id` under `consumer_authentication_information` — and `ErrorInformation(Box<CybersourceErrorInformationResponse>)`.
- Successful mapping produces `PaymentsResponseData::PreAuthenticateResponse` with `redirection_data = Some(Box::new(RedirectForm::CybersourceAuthSetup { access_token, ddc_url, reference_id }))`; see the `TryFrom<ResponseRouterData<CybersourceAuthSetupResponse, Self>>` impl in the same file.
- That same impl stamps `AttemptStatus::AuthenticationPending` regardless of whether a challenge will follow, and passes `resource_id: None` — the first field of the variant.
- Barclaycard's `BarclaycardAuthSetupRequest<T>` / `BarclaycardAuthSetupResponse` pair mirrors this shape one-for-one and is the better exemplar to copy, because Barclaycard also overrides `next_authentication_step` for the full trio.

### Redsys

- `pub type RedsysPreAuthenticateRequest = super::transformers::RedsysTransaction` (`connectors/redsys/requests.rs`). The same struct is reused for every flow that hits `/sis/rest`, with a different `DS_MERCHANT_TRANSACTIONTYPE` field discriminating operations.
- The PreAuth endpoint is `/sis/rest/iniciaPeticionREST` (the `get_url` in Redsys's `flow_name: PreAuthenticate` macro block) — Redsys's "iniciaPeticion" is the 3DS enrolment bootstrap.
- Response mapping is concentrated in the free function `fn get_preauthenticate_response(..)` in `connectors/redsys/transformers.rs`. It returns one of three shapes: (a) no `Ds_EMV3DS` block at all → every field `None`; (b) a `three_d_s_method_u_r_l` present → `build_threeds_invoke_response`, which emits the 3DS-method `RedirectForm`; (c) no method URL → `build_threeds_exempt_response`, which emits an `AuthenticationData` carrying `threeds_server_transaction_id` and `message_version`. Its caller — the `TryFrom<..> for RouterDataV2<PreAuthenticate, ..>` impl in the same file — sets `resource_common_data.status` to `AuthenticationPending`.

### Worldpay

- Requests are not constructed from `PaymentsPreAuthenticateData` fields directly. Worldpay expects the browser to POST an urlencoded form; the `TryFrom<..> for WorldpayPreAuthenticateRequest` impl in `connectors/worldpay/transformers.rs` (marked with the comment `// PreAuthenticate request transformer (for 3dsDeviceData/DDC)`) pulls `redirect_response.params`, errors with `IntegrationError::MissingRequiredField { field_name: "redirect_response.params", .. }` when absent, and otherwise runs `serde_urlencoded::from_str::<Self>(params.peek())`, mapping failure to `IntegrationError::BodySerializationFailed`. `pub type WorldpayPreAuthenticateRequest = WorldpayAuthenticateRequest` lives in `connectors/worldpay/requests.rs`.
- The URL includes a `link_data` segment extracted from `connector_feature_data` via `Self::extract_link_data_from_metadata(req)?` and then `urlencoding::encode`d. This is the Worldpay-specific way of chaining Authorize → PreAuthenticate → PostAuthenticate — the linkage is carried entirely in URL path segments, never in the body.

### Nexixpay

- Request is `pub struct NexixpayPreAuthenticateRequest` (`connectors/nexixpay/transformers.rs`), a minimal JSON body posted to `/orders/3steps/init`.
- Response type `pub struct NexixpayPreAuthenticateResponse` (same file) carries `operation`, `three_ds_enrollment_status`, `three_ds_auth_request`, `three_ds_auth_url`. When the ACS URL is present the response transformer emits a `RedirectForm::Form` whose `form_fields` are exactly `ThreeDsRequest`, `ReturnUrl` (from `continue_redirection_url`, **not** `router_return_url` — the in-code comment spells out why), and `transactionId`.
- The NexiXPay `operationId` is persisted to `PaymentFlowData.preprocessing_id` for the subsequent Authorize call — grep `preprocessing_id: Some(operation.operation_id.clone())` in that file.
- Nexixpay does **not** override `next_authentication_step`, so this leg is only reachable via the direct `PaymentMethodAuthenticationService` RPC.

### NMI

- Uses form-urlencoded transport. The request is `pub struct NmiVaultRequest<T>` (`connectors/nmi/transformers.rs`), which vaults the card and returns a `customer_vault_id` used later by Authorize. `pub type NmiPreAuthenticateResponse = NmiVaultResponse` (declared in both `connectors/nmi.rs` and `connectors/nmi/transformers.rs`) keeps the macro happy without introducing a distinct type.
- NMI's `preprocess_response_bytes` member function (`connectors/nmi.rs`) converts the urlencoded body to JSON before the generated response `TryFrom` runs.

### Kount (device-data collection, no outbound call)

Kount's PreAuthenticate is the one implementation in the roster that talks to nothing. It is covered
in full under [Pattern D](#pattern-d--local-flow-no-outbound-call-kount-worldpayxml); the short form:

- Wired with `macros::macro_connector_local_flow_implementation!`, not `macro_connector_implementation!`.
- `handle_response: kount::handle_pre_authenticate_response` — a free function in
  `connectors/kount/transformers.rs` that ignores its `event_builder` and `res` arguments.
- Emits `RedirectForm::Script { script_data }` with a DDC snippet whose `sessionId` is a hash of
  `request.merchant_transaction_id` and whose `environment` is derived from
  `resource_common_data.access_token`, never hardcoded.
- Stamps `AttemptStatus::DeviceDataCollectionPending` and a synthesised `status_code: 200`.
- Kount is registered as both a payment connector (`ConnectorEnum::Kount`) and the sole FRM
  connector (`FrmConnectorEnum::Kount`); the leg is reachable on the payment-method
  authentication service because `internal_pre_authenticate` declares
  `connector_data_types: [ConnectorData, FrmConnectorData]`.

Worldpayxml uses the identical macro and handler shape for its own DDC leg.

### External vs native 3DS

Two implementation styles exist in this repo:

- **Native 3DS** (Cybersource, Redsys, Worldpay, Nexixpay, NMI): the connector drives the full 3DS1/3DS2 handshake — DDC, ACS challenge, CRes validation — and the UCS pipeline invokes PreAuthenticate → Authenticate → PostAuthenticate against that connector's own endpoints.
- **External 3DS** (Revolv3 is the in-repo example): the connector accepts an externally-computed `AuthenticationData` (CAVV/ECI/DS-Trans-ID produced by a third-party authenticator) and uses it during the regular Authorize call. At HEAD, Revolv3 declares all three trio markers under `not_supported:` in its `macro_connector_flow_status_impls!` call (`connectors/revolv3.rs`) — there are no hand-written empty impls. Implementers of external-3DS connectors MUST do the same and surface 3DS data via `PaymentsAuthorizeData<T>` instead.

  A stronger form of the rule: the **whole external-3DS product category** — 3dsecure.io, GPayments, Cardinal, Click-to-Pay, and Netcetera used as a router-side authenticator — executes inside the Hyperswitch router and never enters UCS. If a technical specification describes one of those, do not generate any UCS flow for it. The `netcetera` **connector** in this repo is not that; it is an authentication-only UCS payment connector.

## Code Examples

### 1. Macro registration (Cybersource)

```rust
// connectors/cybersource.rs — create_all_prerequisites! flows list
(
    flow: PreAuthenticate,
    request_body: CybersourceAuthSetupRequest<T>,
    response_body: CybersourceAuthSetupResponse,
    router_data: RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
),
```

### 2. URL builder (Cybersource)

```rust
// connectors/cybersource.rs — other_functions of the flow_name: PreAuthenticate macro block
fn get_url(
    &self,
    req: &RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
) -> CustomResult<String, IntegrationError> {
    Ok(format!(
        "{}risk/v1/authentication-setups",
        self.connector_base_url_payments(req)
    ))
}
```

### 3. Response transformer producing DDC form (Cybersource)

```rust
// connectors/cybersource/transformers.rs
// impl TryFrom<ResponseRouterData<CybersourceAuthSetupResponse, Self>>
//   for RouterDataV2<F, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>
CybersourceAuthSetupResponse::ClientAuthSetupInfo(info_response) => Ok(Self {
    resource_common_data: PaymentFlowData {
        status: common_enums::AttemptStatus::AuthenticationPending,
        ..item.router_data.resource_common_data
    },
    response: Ok(PaymentsResponseData::PreAuthenticateResponse {
        // `resource_id` is the FIRST field of the variant. Omitting it is E0063.
        resource_id: None,
        redirection_data: Some(Box::new(RedirectForm::CybersourceAuthSetup {
            access_token: info_response.consumer_authentication_information.access_token.expose(),
            ddc_url: info_response.consumer_authentication_information.device_data_collection_url,
            reference_id: info_response.consumer_authentication_information.reference_id,
        })),
        connector_response_reference_id: Some(
            info_response.client_reference_information.code
                .unwrap_or(info_response.id.clone()),
        ),
        status_code: item.http_code,
        authentication_data: None,
    }),
    ..item.router_data
}),
```

### 4. Redirect-form injection for issuer ACS (Nexixpay)

```rust
// connectors/nexixpay/transformers.rs — PreAuthenticate response transformer
let authentication_data = if let Some(auth_url) = &response.three_ds_auth_url {
    let mut form_fields = HashMap::new();
    form_fields.insert(
        "ThreeDsRequest".to_string(),
        response.three_ds_auth_request.clone().unwrap_or_default(),
    );
    if let Some(continue_url) = &item.router_data.request.continue_redirection_url {
        form_fields.insert("ReturnUrl".to_string(), continue_url.to_string());
    }
    form_fields.insert("transactionId".to_string(), operation.operation_id.clone());

    Some(Box::new(
        domain_types::router_response_types::RedirectForm::Form {
            endpoint: auth_url.clone(),
            method: common_utils::request::Method::Post,
            form_fields,
        },
    ))
} else {
    None
};
```

### 5. Reusing a shared transaction body (Redsys)

```rust
// connectors/redsys/requests.rs
pub type RedsysPreAuthenticateRequest = super::transformers::RedsysTransaction;
pub type RedsysAuthenticateRequest   = super::transformers::RedsysTransaction;
```

## Integration Guidelines

1. **Declare the trait.** Add `impl<...> connector_types::PaymentPreAuthenticateV2<T> for <Connector><T> {}` to the connector's main file. Keep the body empty only if the connector relies on external 3DS (see [external vs native 3DS](#external-vs-native-3ds)); otherwise implement it via the macro path below.
2. **Register the flow in `create_all_prerequisites!`.** Add the tuple `(flow: PreAuthenticate, request_body: <Connector>PreAuthenticateRequest, response_body: <Connector>PreAuthenticateResponse, router_data: RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>)`.
3. **Emit a `macro_connector_implementation!`** with `flow_name: PreAuthenticate`, `flow_request: PaymentsPreAuthenticateData<T>`, `flow_response: PaymentsResponseData`, `resource_common_data: PaymentFlowData`, and `http_method: Post`.
4. **Implement `get_url`** to return the connector's "auth setup" endpoint. If the connector requires a link fragment from the previous step (Worldpay), fetch it from `connector_feature_data`.
5. **Implement `get_headers`** via the shared `build_headers` helper so that the content type and auth header match Authorize. Most of the roster does this verbatim (`self.build_headers(req)`); Getnet passes an access token instead (`self.build_headers(&access_token)`), Moneris routes through `self.get_headers_from_access_token(..)`, and NMI hand-writes a single `Content-Type: application/x-www-form-urlencoded` header because its leg is form-encoded. Reuse the connector-wide helper rather than inventing a per-flow one.
6. **Write `TryFrom` for the request**, mapping card data, browser info and return URLs from `PaymentsPreAuthenticateData<T>` to the connector-specific body. Use the generic `T: PaymentMethodDataTypes` bound and extract the `Card<T>` variant via the utilities in `grace/rulesbook/codegen/guides/utility_functions_reference.md`.
7. **Write `TryFrom` for the response** producing `PaymentsResponseData::PreAuthenticateResponse`. Populate `redirection_data` with either a `RedirectForm::Form` (HTML-form POST) or a connector-specific variant (e.g. `RedirectForm::CybersourceAuthSetup`). Set `resource_common_data.status` to `AttemptStatus::AuthenticationPending` in the pending path.
8. **Persist correlation ids.** Store whatever identifier the connector will need for [`Authenticate`](./pattern_authenticate.md) (e.g. `operationId`, `referenceId`, `customer_vault_id`) in `PaymentFlowData.connector_feature_data` or `preprocessing_id`.
9. **Wire error mapping** to the connector-wide `build_error_response` hook; do not re-implement `IntegrationError` or `ConnectorError` per flow.

## Best Practices

- Use `AttemptStatus::AuthenticationPending` whenever the flow ends with a redirect or a challenge requirement (the PreAuthenticate response transformers in `connectors/redsys/transformers.rs` and `connectors/cybersource/transformers.rs` both do this); reserve `AuthenticationFailed` for explicit issuer/ACS denial, and `DeviceDataCollectionPending` for a DDC-only leg (`connectors/kount/transformers.rs`).
- Read return URLs from `continue_redirection_url` (the `/complete` path) not `router_return_url` (the `/response` PSync path). Nexixpay's PreAuthenticate response transformer documents this distinction inline (`connectors/nexixpay/transformers.rs`, next to the `"ReturnUrl"` form field insert).
- Reuse connector-level helpers (`build_headers`, `connector_base_url_payments`) defined once in the `member_functions` block of `create_all_prerequisites!` — do not duplicate header construction per flow (`connectors/redsys.rs`).
- When a connector shares a request struct across 3DS steps (Redsys's `RedsysTransaction`, Worldpay's `WorldpayAuthenticateRequest`) expose the aliases in one place (`requests.rs`) so that the macro wiring remains readable.
- Persist correlation ids in `connector_feature_data` via `Secret::new(...)` only for PII-sensitive payloads; plain identifiers like `operationId` may use `preprocessing_id` (grep `preprocessing_id: Some(operation.operation_id.clone())` in `connectors/nexixpay/transformers.rs`).
- Prefer `RedirectForm::Form { endpoint, method, form_fields }` over ad-hoc HTML generation; see `grace/rulesbook/codegen/guides/patterns/authorize/card/pattern_authorize_card.md` for the authoritative list of form shapes accepted by UCS.

## Common Errors / Gotchas

1. **Problem:** `IntegrationError::MissingRequiredField { field_name: "redirect_response.params" }` on Worldpay PreAuthenticate.
   **Solution:** `TryFrom<..> for WorldpayPreAuthenticateRequest` (`connectors/worldpay/transformers.rs`) requires the browser's DDC form POST to be fed through `redirect_response.params`. Ensure the router forwards the urlencoded body; do not synthesise the request from `PaymentsPreAuthenticateData` alone.
2. **Problem:** Authentication succeeds but Authorize later fails with "missing operationId".
   **Solution:** Persist the connector's transaction correlation id in `PaymentFlowData.preprocessing_id` (Nexixpay) or `connector_feature_data` (Worldpay `link_data`). See the step 8 guideline above.
3. **Problem:** Empty `PaymentPreAuthenticateV2<T>` impl compiles but runtime calls return `IntegrationError::NotImplemented`.
   **Solution:** Connectors that do not support native 3DS declare the marker under `not_implemented:` / `not_supported:` in `macro_connector_flow_status_impls!`. If you need 3DS for such a connector, move the marker out of that list and add the macro block; if you are deliberately using external 3DS, leave it there and surface CAVV/ECI via `PaymentsAuthorizeData<T>` — see [external vs native 3DS](#external-vs-native-3ds).
4. **Problem:** Hardcoded `status: AttemptStatus::AuthenticationSuccessful` in the transformer.
   **Solution:** The spec at `grace/rulesbook/codegen/guides/patterns/PATTERN_AUTHORING_SPEC.md` §11 bans hardcoded statuses. Map from the connector response — e.g. `NexixpayPaymentStatus::ThreedsValidated` → `AuthenticationSuccessful` — using the exhaustive `impl From<NexixpayPaymentStatus> for AttemptStatus` in `connectors/nexixpay/transformers.rs`, not a wildcard arm.
5. **Problem:** Browser form submit lands on `/response` (PSync) instead of `/complete` (CompleteAuthorize).
   **Solution:** Use `request.continue_redirection_url`, not `router_return_url`, when populating `ReturnUrl` form fields (documented in-line in `connectors/nexixpay/transformers.rs`).

## Testing Notes

### Unit tests

Unit tests in the `connectors/<name>/tests.rs` files should cover at least:
- A `TryFrom` from `PaymentsPreAuthenticateData<T>` to the connector's PreAuthenticate request, asserting the card, browser info and return URLs round-trip.
- A `TryFrom` from the connector's PreAuthenticate response (success variant) to `PaymentsResponseData::PreAuthenticateResponse`, asserting `redirection_data.is_some()` and the `AttemptStatus::AuthenticationPending` stamp.
- The error-response path, asserting `ErrorResponse` from `domain_types::router_data` is emitted (never the retired `ConnectorError`).

### Integration test scenarios

| Scenario | Inputs | Expected output |
| --- | --- | --- |
| Frictionless exit (Cybersource/Redsys 3DS2) | Enrolled BIN, browser supplies DDC token | `PreAuthenticateResponse.authentication_data = Some(AuthenticationData { trans_status: Y, cavv, eci })`, `AttemptStatus::AuthenticationPending`. |
| Challenge required (Nexixpay) | Challenge-BIN Visa test card | `PreAuthenticateResponse.redirection_data = Some(RedirectForm::Form { endpoint: ACS URL, form_fields: [ThreeDsRequest, ReturnUrl, transactionId] })`. |
| Issuer denied (Redsys `ChallengeRequiredDecoupledAuthentication`) | Test card returning status `D` | `resource_common_data.status = AuthenticationPending` with a decoupled-auth redirect. |
| Connector unreachable | DNS failure | Error propagated via `build_error_response`; `status_code` surfaced on `ErrorResponse`. |

### Sandbox requirements

- Cybersource: sandbox API key with "Payer Auth" enabled; DDC test cards documented in `grace/rulesbook/codegen/references/cybersource/technical_specification.md` (when present).
- Redsys: `DS_MERCHANT_MERCHANTCODE` and test-mode `SANDBOX` URL `https://sis-t.redsys.es:25443`.
- Worldpay: Worldpay-Connect sandbox with `link_data` fully populated in the Authorize response.

## Cross-References

- Parent index: [./README.md](./README.md)
- Sibling 3DS flows: [pattern_authenticate.md](./pattern_authenticate.md), [pattern_postauthenticate.md](./pattern_postauthenticate.md)
- Sibling flow (non-3DS): [pattern_authorize.md](./pattern_authorize.md)
- Dispatch — **read before wiring any leg**: [pattern_authentication_dispatch.md](./pattern_authentication_dispatch.md)
- PM pattern (shares 3DS prose; do not edit): [authorize/card/pattern_authorize_card.md](./authorize/card/pattern_authorize_card.md) — find its "3D Secure Pattern" heading.
- Authoring spec: [./PATTERN_AUTHORING_SPEC.md](./PATTERN_AUTHORING_SPEC.md)
- Types used by this flow, by symbol:
  - `pub struct PaymentsPreAuthenticateData` — `crates/types-traits/domain_types/src/connector_types.rs`
  - `pub enum PaymentsResponseData`, variant `PreAuthenticateResponse` — same file
  - `pub struct PaymentFlowData` — same file
  - `pub struct AuthenticationData` — `crates/types-traits/domain_types/src/router_request_types.rs`
  - `pub struct PreAuthenticate` — `crates/types-traits/domain_types/src/connector_flow.rs`
  - `pub trait PaymentPreAuthenticateV2`, `pub enum AuthenticationStep`, `pub enum RedirectState`, `fn next_authentication_step` — `crates/types-traits/interfaces/src/connector_types.rs`
  - `macro_rules! macro_connector_local_flow_implementation` — `crates/integrations/connector-integration/src/connectors/macros.rs`
