# PreAuthenticate Flow Pattern

## Overview

The PreAuthenticate flow is the first leg of the 3D Secure (3DS) authentication trio. Its job is to initiate the authentication session with the connector, collect whatever preliminary data the issuer's ACS (Access Control Server) requires, and hand back either device-data-collection (DDC) instructions or an early frictionless-success signal. The flow is keyed off `domain_types::connector_flow::PreAuthenticate` and produces `PaymentsResponseData::PreAuthenticateResponse` whose primary role is to carry a `RedirectForm` (for DDC) and/or an early `AuthenticationData` payload for the subsequent `Authenticate` step.

This flow corresponds to concepts such as "Payer Authentication Setup", "Device Data Collection initiation", or "3DS Method URL retrieval" depending on the gateway. On success the orchestrator either completes DDC via a browser-driven iframe and invokes [`Authenticate`](./pattern_authenticate.md), or (for connectors that perform enrolment lookup here) skips straight to [`PostAuthenticate`](./pattern_postauthenticate.md).

### Key Components
- Flow marker: `PreAuthenticate` at `crates/types-traits/domain_types/src/connector_flow.rs:50`.
- Request type: `PaymentsPreAuthenticateData<T>` at `crates/types-traits/domain_types/src/connector_types.rs:1518`.
- Response type: `PaymentsResponseData::PreAuthenticateResponse` at `crates/types-traits/domain_types/src/connector_types.rs:1408`.
- Resource common data: `PaymentFlowData` at `crates/types-traits/domain_types/src/connector_types.rs:422`.
- Trait implemented by connectors: `connector_types::PaymentPreAuthenticateV2<T>` (see each connector's main file for its declaration).

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

Inputs to `PreAuthenticate` (see `PaymentsPreAuthenticateData<T>` at `crates/types-traits/domain_types/src/connector_types.rs:1518`):
- `payment_method_data: Option<PaymentMethodData<T>>` — card being authenticated.
- `amount: MinorUnit`, `currency: Option<Currency>`, `email: Option<Email>` — transaction context.
- `router_return_url`, `continue_redirection_url: Option<Url>` — return targets for the browser redirect completion.
- `browser_info: Option<BrowserInformation>` — UA/screen info for 3DS2 risk scoring.
- `enrolled_for_3ds: bool` — whether the router expects 3DS to apply.
- `redirect_response: Option<ContinueRedirectionResponse>` — when the previous step's redirect payload needs to be forwarded (see Worldpay's DDC form submit below).
- `mandate_reference: Option<MandateReferenceId>` — when authenticating a stored mandate.

Outputs from `PreAuthenticate` (see `PaymentsResponseData::PreAuthenticateResponse` at `crates/types-traits/domain_types/src/connector_types.rs:1408`):
- `redirection_data: Option<Box<RedirectForm>>` — DDC form or 3DS method iframe. When present the caller renders the form client-side.
- `authentication_data: Option<AuthenticationData>` — populated when the connector already returns a CAVV/ECI (frictionless exit path).
- `connector_response_reference_id: Option<String>` — correlation id used by downstream `Authenticate`/`PostAuthenticate`.
- `status_code: u16` — HTTP status propagated for diagnostics.

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

`PreAuthenticate` is the zero-sized marker struct declared at `crates/types-traits/domain_types/src/connector_flow.rs:50`:

```rust
// From crates/types-traits/domain_types/src/connector_flow.rs:49
#[derive(Debug, Clone)]
pub struct PreAuthenticate;
```

### Request Type

`PaymentsPreAuthenticateData<T>` lives at `crates/types-traits/domain_types/src/connector_types.rs:1518` and carries the caller inputs enumerated in [3DS Flow Sequence](#3ds-flow-sequence).

```rust
// From crates/types-traits/domain_types/src/connector_types.rs:1518
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
}
```

The `is_auto_capture` helper at `crates/types-traits/domain_types/src/connector_types.rs:1533` mirrors the one used by regular Authorize and returns `IntegrationError::CaptureMethodNotSupported` for the multi/scheduled variants.

### Response Type

`PaymentsResponseData::PreAuthenticateResponse` — the variant of the shared `PaymentsResponseData` enum — is defined at `crates/types-traits/domain_types/src/connector_types.rs:1408`. It has exactly four fields: `authentication_data`, `redirection_data` (comment at line 1410 reads `/// For Device Data Collection`), `connector_response_reference_id`, and `status_code`.

```rust
// From crates/types-traits/domain_types/src/connector_types.rs:1408
PreAuthenticateResponse {
    authentication_data: Option<router_request_types::AuthenticationData>,
    /// For Device Data Collection
    redirection_data: Option<Box<RedirectForm>>,
    connector_response_reference_id: Option<String>,
    status_code: u16,
},
```

`AuthenticationData` is defined at `crates/types-traits/domain_types/src/router_request_types.rs:136` with the 3DS fields used by all three flows (`trans_status`, `eci`, `cavv`, `ucaf_collection_indicator`, `threeds_server_transaction_id`, `message_version`, `ds_trans_id`, `acs_transaction_id`, `transaction_id`, `network_params`, `exemption_indicator`).

### Resource Common Data

`PaymentFlowData` (`crates/types-traits/domain_types/src/connector_types.rs:422`) is the same struct used by every payment flow; authors must not redefine it. In PreAuthenticate, transformers typically set `resource_common_data.status` to `AttemptStatus::AuthenticationPending` (see `crates/integrations/connector-integration/src/connectors/redsys/transformers.rs:890` and `crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:2834`).

## Connectors with Full Implementation

| Connector | HTTP Method | Content Type | URL Pattern | Request Type Reuse | Notes |
| --- | --- | --- | --- | --- | --- |
| Cybersource | POST | `application/json;charset=utf-8` | `{base}risk/v1/authentication-setups` | `CybersourceAuthSetupRequest<T>` (dedicated; see `crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:2198`) | Emits `RedirectForm::CybersourceAuthSetup` for DDC (`crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:2838`). URL wired at `crates/integrations/connector-integration/src/connectors/cybersource.rs:628`. |
| Nexixpay | POST | `application/json` | `{base}/orders/3steps/init` | `NexixpayPreAuthenticateRequest` (dedicated; see `crates/integrations/connector-integration/src/connectors/nexixpay/transformers.rs:1314` for response pair) | Returns `RedirectForm::Form` embedding `ThreeDsRequest`, `ReturnUrl`, `transactionId` (see `crates/integrations/connector-integration/src/connectors/nexixpay/transformers.rs:1369`). URL at `crates/integrations/connector-integration/src/connectors/nexixpay.rs:734`. |
| NMI | POST | `application/x-www-form-urlencoded` | `{base}{TRANSACT}` (customer-vault add) | `NmiVaultRequest<T>` (reused from customer-vault add; alias `NmiPreAuthenticateResponse = NmiVaultResponse` at `crates/integrations/connector-integration/src/connectors/nmi/transformers.rs:1281`) | Prerequisite registration at `crates/integrations/connector-integration/src/connectors/nmi.rs:246`. Macro wiring at `crates/integrations/connector-integration/src/connectors/nmi.rs:570`. |
| Redsys | POST | `application/json` | `{base}/sis/rest/iniciaPeticionREST` | `RedsysPreAuthenticateRequest` (alias `= super::transformers::RedsysTransaction` at `crates/integrations/connector-integration/src/connectors/redsys/requests.rs:6`) | URL wired at `crates/integrations/connector-integration/src/connectors/redsys.rs:371`. Response mapping at `crates/integrations/connector-integration/src/connectors/redsys/transformers.rs:873`. |
| Worldpay | POST | `application/json` | `{base}api/payments/{link_data}/3dsDeviceData` | `WorldpayPreAuthenticateRequest` (alias `= WorldpayAuthenticateRequest` at `crates/integrations/connector-integration/src/connectors/worldpay/requests.rs:383`) | Pulls request body from `redirect_response.params` urlencoded string (`crates/integrations/connector-integration/src/connectors/worldpay/transformers.rs:1267`). Endpoint uses `link_data` extracted from feature data (`crates/integrations/connector-integration/src/connectors/worldpay.rs:601`). |

### Stub Implementations

These connectors declare `PaymentPreAuthenticateV2<T>` (and/or `ConnectorIntegrationV2<PreAuthenticate, ...>`) but ship empty bodies at the pinned SHA; they MUST NOT be counted as full implementations.

- Checkout — trait declared at `crates/integrations/connector-integration/src/connectors/checkout.rs:183`; empty `ConnectorIntegrationV2<PreAuthenticate, ...>` at `crates/integrations/connector-integration/src/connectors/checkout.rs:691`.
- Stripe — trait declared at `crates/integrations/connector-integration/src/connectors/stripe.rs:169`; empty impl at `crates/integrations/connector-integration/src/connectors/stripe.rs:1069`.
- Revolv3 — trait declared at `crates/integrations/connector-integration/src/connectors/revolv3.rs:170`; empty impl at `crates/integrations/connector-integration/src/connectors/revolv3.rs:335`. See the note on [external vs native 3DS](#external-vs-native-3ds) below.
- Other connectors carrying an empty `PaymentPreAuthenticateV2<T>` declaration include Adyen, Braintree, Checkout, Paypal, Shift4, Trustpay, and the remaining roster that still awaits 3DS enablement.

## Common Implementation Patterns

### Pattern A — Macro-based with dedicated request/response

This is the recommended path and the one used by Cybersource, Redsys, Worldpay and Nexixpay. The flow is registered inside `create_all_prerequisites!` with the full `RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>` tuple, and wiring for URL/headers is provided in a matching `macro_connector_implementation!` block whose `flow_name:` is `PreAuthenticate`.

```rust
// From crates/integrations/connector-integration/src/connectors/redsys.rs:230
(
    flow: PreAuthenticate,
    request_body: RedsysPreAuthenticateRequest,
    response_body: RedsysPreAuthenticateResponse,
    router_data: RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
),
```

```rust
// From crates/integrations/connector-integration/src/connectors/redsys.rs:347
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

NMI does not expose a dedicated 3DS-setup endpoint. Instead it treats PreAuthenticate as "create a customer vault entry from a card" and reuses the `/transact` URL with a `customer_vault=add_customer` form value. The macro block at `crates/integrations/connector-integration/src/connectors/nmi.rs:570` sets `curl_request: FormUrlEncoded(NmiVaultRequest)` and `content_type = application/x-www-form-urlencoded`. The response is a URL-encoded blob which the prerequisites-level `preprocess_response_bytes` function at `crates/integrations/connector-integration/src/connectors/nmi.rs:256` re-serialises to JSON before the generated `TryFrom` runs.

### Pattern C — Dual endpoint with link-data routing (Worldpay)

Worldpay encodes the payment reference into a URL path segment returned in the previous step's `_links` block. The connector stores that segment in `connector_feature_data` and unpacks it with `Self::extract_link_data_from_metadata(req)`. The PreAuthenticate URL is built by joining it onto `api/payments/{link_data}/3dsDeviceData` (see `crates/integrations/connector-integration/src/connectors/worldpay.rs:607`). The request body is not synthesised from `PaymentsPreAuthenticateData` directly; instead the transformer at `crates/integrations/connector-integration/src/connectors/worldpay/transformers.rs:1253` reads the browser's urlencoded form POST out of `redirect_response.params`.

## Connector-Specific Patterns

### Cybersource

- Dedicated request type `CybersourceAuthSetupRequest<T>` (`crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:2198`) with fields `payment_information` and `client_reference_information` only. No amount or order info is posted at this stage — those belong to `Authenticate`.
- Response is an untagged enum `CybersourceAuthSetupResponse` (`crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:2647`) with two variants: `ClientAuthSetupInfo` (success, carries `access_token`, `device_data_collection_url`, `reference_id`) and `ErrorInformation`.
- Successful mapping produces `PaymentsResponseData::PreAuthenticateResponse` with `redirection_data = Some(RedirectForm::CybersourceAuthSetup { access_token, ddc_url, reference_id })` at `crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:2838`.
- PreAuth status is `AttemptStatus::AuthenticationPending` regardless of whether a challenge will follow (`crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:2834`).

### Redsys

- `RedsysPreAuthenticateRequest` is a type alias over the shared `RedsysTransaction` body (`crates/integrations/connector-integration/src/connectors/redsys/requests.rs:6`). The same struct is reused for every flow that hits `/sis/rest`, with a different `DS_MERCHANT_TRANSACTIONTYPE` field discriminating operations.
- The PreAuth endpoint is `/sis/rest/iniciaPeticionREST` (`crates/integrations/connector-integration/src/connectors/redsys.rs:371`) — Redsys's "iniciaPeticion" is the 3DS enrolment bootstrap.
- Response mapping is concentrated in `get_preauthenticate_response` at `crates/integrations/connector-integration/src/connectors/redsys/transformers.rs:496`. It can return one of three shapes, producing either a `RedirectForm` (challenge required), a `ChallengeRequiredDecoupledAuthentication` signal, or an already-authenticated `AuthenticationData` payload. In all cases `resource_common_data.status` is set to `AuthenticationPending` at the call site (`crates/integrations/connector-integration/src/connectors/redsys/transformers.rs:890`).

### Worldpay

- Requests are not constructed from `PaymentsPreAuthenticateData` fields directly. Worldpay expects the browser to POST an urlencoded form containing the `sessionState`/`acsTransactionId` produced by its DDC iframe; the transformer at `crates/integrations/connector-integration/src/connectors/worldpay/transformers.rs:1278` deserialises that payload via `serde_urlencoded::from_str` into `WorldpayPreAuthenticateRequest` (= `WorldpayAuthenticateRequest`, `crates/integrations/connector-integration/src/connectors/worldpay/requests.rs:383`).
- The URL includes a `link_data` segment that must be extracted from `connector_feature_data` via `Self::extract_link_data_from_metadata(req)?` (`crates/integrations/connector-integration/src/connectors/worldpay.rs:605`). This is the Worldpay-specific way of chaining Authorize → PreAuthenticate → PostAuthenticate — the linkage is carried entirely in URL path segments, never in the body.

### Nexixpay

- Request is a minimal JSON body posted to `/orders/3steps/init` (`crates/integrations/connector-integration/src/connectors/nexixpay.rs:734`).
- Response type `NexixpayPreAuthenticateResponse` (`crates/integrations/connector-integration/src/connectors/nexixpay/transformers.rs:1314`) carries `operation`, `threeDSEnrollmentStatus`, `threeDSAuthRequest`, `threeDSAuthUrl`. When the ACS-URL is present the transformer emits a `RedirectForm::Form` whose `form_fields` include `ThreeDsRequest`, `ReturnUrl` (from `continue_redirection_url`, NOT `router_return_url`), and `transactionId` (`crates/integrations/connector-integration/src/connectors/nexixpay/transformers.rs:1369`).
- The NexiXPay `operationId` is persisted to `PaymentFlowData.preprocessing_id` for the subsequent Authorize call (`crates/integrations/connector-integration/src/connectors/nexixpay/transformers.rs:1409`).

### NMI

- Uses form-urlencoded transport. The request is `NmiVaultRequest<T>` (`crates/integrations/connector-integration/src/connectors/nmi/transformers.rs:1301`), which vaults the card and returns a `customer_vault_id` used later by Authorize. The alias `NmiPreAuthenticateResponse = NmiVaultResponse` at `crates/integrations/connector-integration/src/connectors/nmi/transformers.rs:1281` keeps the macro happy without introducing a distinct type.
- NMI's `preprocess_response_bytes` hook at `crates/integrations/connector-integration/src/connectors/nmi.rs:256` converts the urlencoded body to JSON before the generated response `TryFrom` runs.

### External vs native 3DS

Two implementation styles exist at the pinned SHA:

- **Native 3DS** (Cybersource, Redsys, Worldpay, Nexixpay, NMI): the connector drives the full 3DS1/3DS2 handshake — DDC, ACS challenge, CRes validation — and the UCS pipeline invokes PreAuthenticate → Authenticate → PostAuthenticate against that connector's own endpoints.
- **External 3DS** (Revolv3, per PR #815): the connector accepts an externally-computed `AuthenticationData` (CAVV/ECI/DS-Trans-ID produced by a third-party authenticator) and uses it during the regular Authorize call. The 3DS trio traits are declared (`crates/integrations/connector-integration/src/connectors/revolv3.rs:170`, `:165`) but their `ConnectorIntegrationV2<...>` impls are empty (`crates/integrations/connector-integration/src/connectors/revolv3.rs:335` for PreAuthenticate, `:325` for PostAuthenticate, `:295` for Authenticate). Implementers of external-3DS connectors MUST keep these stubs and surface 3DS data via `PaymentsAuthorizeData<T>` instead.

## Code Examples

### 1. Macro registration (Cybersource)

```rust
// From crates/integrations/connector-integration/src/connectors/cybersource.rs:237
(
    flow: PreAuthenticate,
    request_body: CybersourceAuthSetupRequest<T>,
    response_body: CybersourceAuthSetupResponse,
    router_data: RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
),
```

### 2. URL builder (Cybersource)

```rust
// From crates/integrations/connector-integration/src/connectors/cybersource.rs:628
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
// From crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:2832
CybersourceAuthSetupResponse::ClientAuthSetupInfo(info_response) => Ok(Self {
    resource_common_data: PaymentFlowData {
        status: common_enums::AttemptStatus::AuthenticationPending,
        ..item.router_data.resource_common_data
    },
    response: Ok(PaymentsResponseData::PreAuthenticateResponse {
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
// From crates/integrations/connector-integration/src/connectors/nexixpay/transformers.rs:1369
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
// From crates/integrations/connector-integration/src/connectors/redsys/requests.rs:6
pub type RedsysPreAuthenticateRequest = super::transformers::RedsysTransaction;
pub type RedsysAuthenticateRequest   = super::transformers::RedsysTransaction;
```

## Integration Guidelines

1. **Declare the trait.** Add `impl<...> connector_types::PaymentPreAuthenticateV2<T> for <Connector><T> {}` to the connector's main file. Keep the body empty only if the connector relies on external 3DS (see [external vs native 3DS](#external-vs-native-3ds)); otherwise implement it via the macro path below.
2. **Register the flow in `create_all_prerequisites!`.** Add the tuple `(flow: PreAuthenticate, request_body: <Connector>PreAuthenticateRequest, response_body: <Connector>PreAuthenticateResponse, router_data: RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>)`.
3. **Emit a `macro_connector_implementation!`** with `flow_name: PreAuthenticate`, `flow_request: PaymentsPreAuthenticateData<T>`, `flow_response: PaymentsResponseData`, `resource_common_data: PaymentFlowData`, and `http_method: Post`.
4. **Implement `get_url`** to return the connector's "auth setup" endpoint. If the connector requires a link fragment from the previous step (Worldpay), fetch it from `connector_feature_data`.
5. **Implement `get_headers`** via the shared `build_headers` helper so that the content type and auth header match Authorize (all five full implementations reuse their connector-wide `build_headers`).
6. **Write `TryFrom` for the request**, mapping card data, browser info and return URLs from `PaymentsPreAuthenticateData<T>` to the connector-specific body. Use the generic `T: PaymentMethodDataTypes` bound and extract the `Card<T>` variant via the utilities in `grace/rulesbook/codegen/guides/utility_functions_reference.md`.
7. **Write `TryFrom` for the response** producing `PaymentsResponseData::PreAuthenticateResponse`. Populate `redirection_data` with either a `RedirectForm::Form` (HTML-form POST) or a connector-specific variant (e.g. `RedirectForm::CybersourceAuthSetup`). Set `resource_common_data.status` to `AttemptStatus::AuthenticationPending` in the pending path.
8. **Persist correlation ids.** Store whatever identifier the connector will need for [`Authenticate`](./pattern_authenticate.md) (e.g. `operationId`, `referenceId`, `customer_vault_id`) in `PaymentFlowData.connector_feature_data` or `preprocessing_id`.
9. **Wire error mapping** to the connector-wide `build_error_response` hook; do not re-implement `IntegrationError` or `ConnectorResponseTransformationError` per flow.

## Best Practices

- Use `AttemptStatus::AuthenticationPending` whenever the flow ends with a redirect or a challenge requirement (see `crates/integrations/connector-integration/src/connectors/redsys/transformers.rs:890` and `crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:2834`); reserve `AuthenticationFailed` for explicit issuer/ACS denial.
- Read return URLs from `continue_redirection_url` (the `/complete` path) not `router_return_url` (the `/response` PSync path). Nexixpay's transformer at `crates/integrations/connector-integration/src/connectors/nexixpay/transformers.rs:1381` documents this distinction inline.
- Reuse connector-level helpers (`build_headers`, `connector_base_url_payments`) defined once in `create_all_prerequisites!` — do not duplicate header construction per flow (see `crates/integrations/connector-integration/src/connectors/redsys.rs:269`).
- When a connector shares a request struct across 3DS steps (Redsys's `RedsysTransaction`, Worldpay's `WorldpayAuthenticateRequest`) expose the aliases in one place (`requests.rs`) so that the macro wiring remains readable.
- Persist correlation ids in `connector_feature_data` via `Secret::new(...)` only for PII-sensitive payloads; plain identifiers like `operationId` may use `preprocessing_id` (see `crates/integrations/connector-integration/src/connectors/nexixpay/transformers.rs:1409`).
- Prefer `RedirectForm::Form { endpoint, method, form_fields }` over ad-hoc HTML generation; see `grace/rulesbook/codegen/guides/patterns/authorize/card/pattern_authorize_card.md` for the authoritative list of form shapes accepted by UCS.

## Common Errors / Gotchas

1. **Problem:** `IntegrationError::MissingRequiredField { field_name: "redirect_response.params" }` on Worldpay PreAuthenticate.
   **Solution:** The Worldpay transformer at `crates/integrations/connector-integration/src/connectors/worldpay/transformers.rs:1273` requires the browser's DDC form POST to be fed through `redirect_response.params`. Ensure the router forwards the urlencoded body; do not synthesise the request from `PaymentsPreAuthenticateData` alone.
2. **Problem:** Authentication succeeds but Authorize later fails with "missing operationId".
   **Solution:** Persist the connector's transaction correlation id in `PaymentFlowData.preprocessing_id` (Nexixpay) or `connector_feature_data` (Worldpay `link_data`). See the step 8 guideline above.
3. **Problem:** Empty `PaymentPreAuthenticateV2<T>` impl compiles but runtime calls return `IntegrationError::NotImplemented`.
   **Solution:** Stub impls exist for connectors that do not support native 3DS (Stripe, Checkout, Revolv3). If you need 3DS for such a connector, implement the macro block; if you are deliberately using external 3DS, keep the stub and surface CAVV/ECI via `PaymentsAuthorizeData<T>` — see [external vs native 3DS](#external-vs-native-3ds).
4. **Problem:** Hardcoded `status: AttemptStatus::AuthenticationSuccessful` in the transformer.
   **Solution:** The spec at `grace/rulesbook/codegen/guides/patterns/PATTERN_AUTHORING_SPEC.md` §11 bans hardcoded statuses. Map from the connector response (e.g. `NexixpayPaymentStatus::ThreedsValidated` → `AuthenticationSuccessful`, everything else → `AuthenticationPending`, cf. `crates/integrations/connector-integration/src/connectors/nexixpay/transformers.rs:1345`).
5. **Problem:** Browser form submit lands on `/response` (PSync) instead of `/complete` (CompleteAuthorize).
   **Solution:** Use `request.continue_redirection_url`, not `router_return_url`, when populating `ReturnUrl` form fields (documented in-line at `crates/integrations/connector-integration/src/connectors/nexixpay/transformers.rs:1378`).

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
- PM pattern (shares 3DS prose; do not edit): [authorize/card/pattern_authorize_card.md](./authorize/card/pattern_authorize_card.md) — see §"3D Secure Pattern" around `pattern_authorize_card.md:445`.
- Authoring spec: [./PATTERN_AUTHORING_SPEC.md](./PATTERN_AUTHORING_SPEC.md)
- Types used by this flow: `crates/types-traits/domain_types/src/connector_types.rs:1518` (request), `:1408` (response variant), `:422` (flow data); `crates/types-traits/domain_types/src/router_request_types.rs:136` (AuthenticationData); `crates/types-traits/domain_types/src/connector_flow.rs:50` (marker).
