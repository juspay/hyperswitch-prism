# PostAuthenticate Flow Pattern

## Overview

PostAuthenticate is the closing leg of the 3D Secure (3DS) trio. It runs after the browser has completed the ACS challenge kicked off by [`Authenticate`](./pattern_authenticate.md): the router feeds the returned CRes back to the connector so it can validate the signature and hand back the final `AuthenticationData` payload (CAVV, ECI, DS-Trans-ID, XID, EMV 3DS message version) which is subsequently carried into the regular Authorize call.

The flow is keyed off `domain_types::connector_flow::PostAuthenticate` and produces `PaymentsResponseData::PostAuthenticateResponse`. It is the only place in the trio where a response is expected to always carry a populated `AuthenticationData` on success; there is no redirect path out of PostAuthenticate. Connectors that collapse the full trio into two legs (Worldpay's `3dsChallenges` endpoint, Nexixpay's `/orders/3steps/validation`) use PostAuthenticate as the single validation step.

### Key Components
- Flow marker: `PostAuthenticate` at `crates/types-traits/domain_types/src/connector_flow.rs:56`.
- Request type: `PaymentsPostAuthenticateData<T>` at `crates/types-traits/domain_types/src/connector_types.rs:1635`.
- Response type: `PaymentsResponseData::PostAuthenticateResponse` at `crates/types-traits/domain_types/src/connector_types.rs:1424`.
- Resource common data: `PaymentFlowData` at `crates/types-traits/domain_types/src/connector_types.rs:422`.
- Trait implemented by connectors: `connector_types::PaymentPostAuthenticateV2<T>`.

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

Inputs (`PaymentsPostAuthenticateData<T>` at `crates/types-traits/domain_types/src/connector_types.rs:1635`):
- `payment_method_data: Option<PaymentMethodData<T>>`
- `amount: MinorUnit`, `currency: Option<Currency>`, `email: Option<Email>`
- `router_return_url`, `continue_redirection_url: Option<Url>`
- `browser_info: Option<BrowserInformation>`
- `enrolled_for_3ds: bool`
- `redirect_response: Option<ContinueRedirectionResponse>` — CRes payload returned by the browser.
- `capture_method: Option<common_enums::CaptureMethod>`

Notably `PaymentsPostAuthenticateData` **does NOT** carry a pre-existing `authentication_data` field (compare with `PaymentsAuthenticateData` at `crates/types-traits/domain_types/src/connector_types.rs:1552`). The connector's job is to *produce* the final `AuthenticationData`, not consume one.

Outputs (`PaymentsResponseData::PostAuthenticateResponse` at `crates/types-traits/domain_types/src/connector_types.rs:1424`):
- `authentication_data: Option<AuthenticationData>` — the final 3DS2 payload.
- `connector_response_reference_id: Option<String>`
- `status_code: u16`

There is no `redirection_data` field on this variant; PostAuthenticate is the terminal step of the trio.

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
// From crates/types-traits/domain_types/src/connector_flow.rs:55
#[derive(Debug, Clone)]
pub struct PostAuthenticate;
```

### Request Type

```rust
// From crates/types-traits/domain_types/src/connector_types.rs:1635
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

The impl block at `crates/types-traits/domain_types/src/connector_types.rs:1649` adds an `is_auto_capture` helper (same semantics as the Authorize one).

### Response Type

```rust
// From crates/types-traits/domain_types/src/connector_types.rs:1424
PostAuthenticateResponse {
    authentication_data: Option<router_request_types::AuthenticationData>,
    connector_response_reference_id: Option<String>,
    status_code: u16,
},
```

`AuthenticationData` is the shared 3DS container at `crates/types-traits/domain_types/src/router_request_types.rs:136`. The field semantics PostAuthenticate is expected to populate:

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

`PaymentFlowData` (`crates/types-traits/domain_types/src/connector_types.rs:422`). Transformers set `resource_common_data.status` from the connector's CRes validation outcome: `AuthenticationSuccessful` on Y/A with valid CAVV, `AuthenticationFailed` on N/R, or `AuthenticationPending` when the connector signals a need for a second challenge (rare at this stage — Redsys `ChallengeRequiredDecoupledAuthentication`). Nexixpay's mapping is the authoritative reference: `crates/integrations/connector-integration/src/connectors/nexixpay/transformers.rs:1570`.

## Connectors with Full Implementation

| Connector | HTTP Method | Content Type | URL Pattern | Request Type Reuse | Notes |
| --- | --- | --- | --- | --- | --- |
| Cybersource | POST | `application/json;charset=utf-8` | `{base}risk/v1/authentication-results` | `CybersourceAuthValidateRequest<T>` (dedicated; `crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:2935`) | Reuses `CybersourceAuthenticateResponse` as the response type — aliased as `CybersourcePostAuthenticateResponse` at `crates/integrations/connector-integration/src/connectors/cybersource.rs:57`. URL at `crates/integrations/connector-integration/src/connectors/cybersource.rs:690`. |
| Nexixpay | POST | `application/json` | `{base}/orders/3steps/validation` | `NexixpayPostAuthenticateRequest` (dedicated) | Response `NexixpayPostAuthenticateResponse` at `crates/integrations/connector-integration/src/connectors/nexixpay/transformers.rs:1530` carries `operation` + `threeDSAuthResult` (CAVV/ECI/XID/status/version). Transformer mapping at `:1582`. URL at `crates/integrations/connector-integration/src/connectors/nexixpay.rs:774`. |
| Worldpay | POST | `application/json` | `{base}api/payments/{link_data}/3dsChallenges` | `WorldpayPostAuthenticateRequest` (alias of `WorldpayAuthenticateRequest` at `crates/integrations/connector-integration/src/connectors/worldpay/requests.rs:384`) | Body deserialised from `redirect_response.params` urlencoded form (`crates/integrations/connector-integration/src/connectors/worldpay/transformers.rs:1300`). URL uses the link fragment stored from Authorize (`crates/integrations/connector-integration/src/connectors/worldpay.rs:641`). |

### Stub Implementations

These connectors declare `PaymentPostAuthenticateV2<T>` and/or `ConnectorIntegrationV2<PostAuthenticate, ...>` but ship empty bodies at the pinned SHA:

- Stripe — empty impl at `crates/integrations/connector-integration/src/connectors/stripe.rs:1079` (plus downstream trait declarations).
- Checkout — empty impl at `crates/integrations/connector-integration/src/connectors/checkout.rs:711` (trait declared at `:193`).
- Revolv3 — empty impl at `crates/integrations/connector-integration/src/connectors/revolv3.rs:325`. External 3DS connector: see [external vs native 3DS](#external-vs-native-3ds).
- NMI — no PostAuthenticate registration in `create_all_prerequisites!` (`crates/integrations/connector-integration/src/connectors/nmi.rs:245` ends at PreAuthenticate).
- Redsys — no PostAuthenticate registration either; its 3DS flow terminates at the `Authenticate` step which already returns final `AuthenticationData` (`crates/integrations/connector-integration/src/connectors/redsys.rs:236`).

## Common Implementation Patterns

### Pattern A — Dedicated validation endpoint (Cybersource, Nexixpay)

The connector exposes a discrete "validate CRes" endpoint. The request carries the challenge transaction id; the response carries the final CAVV/ECI. This is the cleanest pattern and the one the `PaymentsResponseData::PostAuthenticateResponse` shape is designed around.

Cybersource posts `consumer_authentication_information: { authentication_transaction_id }` (`crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:2929`) to `risk/v1/authentication-results` (`crates/integrations/connector-integration/src/connectors/cybersource.rs:694`). Nexixpay posts a body containing the PaRes and operation id to `/orders/3steps/validation` (`crates/integrations/connector-integration/src/connectors/nexixpay.rs:774`) and reads back `threeDSAuthResult` (`crates/integrations/connector-integration/src/connectors/nexixpay/transformers.rs:1533`).

### Pattern B — Challenge-completion as part of the main resource (Worldpay)

Worldpay scopes `3dsChallenges` under the payment resource (`api/payments/{link_data}/3dsChallenges`). The POST body is the urlencoded CRes the browser returned, deserialised straight out of `redirect_response.params` by `serde_urlencoded::from_str` (`crates/integrations/connector-integration/src/connectors/worldpay/transformers.rs:1325`). The `link_data` fragment carries the original payment reference so no separate correlation id is needed.

### Pattern C — Skipped (Redsys, NMI)

Some connectors produce final `AuthenticationData` already at the Authenticate step and do not expose a dedicated validation endpoint. In that case do not add a macro wiring for `PostAuthenticate` — keep the `PaymentPostAuthenticateV2<T>` trait with an empty body. The router will not invoke this flow for such connectors.

## Connector-Specific Patterns

### Cybersource

- Request type `CybersourceAuthValidateRequest<T>` (`crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:2935`) carries `payment_information`, `client_reference_information`, `consumer_authentication_information: { authentication_transaction_id }`, and `order_information`.
- Response type is re-used from Authenticate via the alias `CybersourceAuthenticateResponse as CybersourcePostAuthenticateResponse` (`crates/integrations/connector-integration/src/connectors/cybersource.rs:57`) — a deliberate choice because Cybersource returns the same payload shape for both stages. A dedicated `TryFrom<ResponseRouterData<CybersourceAuthenticateResponse, Self>>` targeting `PaymentsPostAuthenticateData<T>` exists at `crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:3402`.
- `AttemptStatus` is derived from `info_response.status` (same enum as Authenticate); terminal mapping produces `AuthenticationSuccessful` / `AuthenticationFailed`.

### Nexixpay

- Request `NexixpayPostAuthenticateRequest` is a small JSON envelope carrying the PaRes-equivalent and `operationId`.
- Response `NexixpayPostAuthenticateResponse` (`crates/integrations/connector-integration/src/connectors/nexixpay/transformers.rs:1530`) carries `operation` plus `threeDSAuthResult: { authenticationValue, eci, xid, status, version }`.
- Transformer at `crates/integrations/connector-integration/src/connectors/nexixpay/transformers.rs:1582` populates `PaymentsResponseData::PostAuthenticateResponse.authentication_data` from `threeDSAuthResult`: `cavv <- authenticationValue`, `eci <- eci`, `threeds_server_transaction_id <- xid`, `trans_status <- status.parse::<TransactionStatus>()`, `message_version <- version.parse::<SemanticVersion>()`.
- PaRes is read directly from `redirect_response` in the subsequent Authorize flow, so `ds_trans_id` is intentionally left `None` (see inline comment at `:1598`).

### Worldpay

- `WorldpayPostAuthenticateRequest` is a type alias over `WorldpayAuthenticateRequest` (`crates/integrations/connector-integration/src/connectors/worldpay/requests.rs:384`). The body is not synthesised from `PaymentsPostAuthenticateData` — the transformer at `crates/integrations/connector-integration/src/connectors/worldpay/transformers.rs:1314` deserialises the browser's CRes form POST out of `redirect_response.params` into that struct.
- URL construction at `crates/integrations/connector-integration/src/connectors/worldpay.rs:641` reuses `Self::extract_link_data_from_metadata(req)?` to pick up the payment reference.
- Response type alias `WorldpayPostAuthenticateResponse = WorldpayPaymentsResponse` (`crates/integrations/connector-integration/src/connectors/worldpay/response.rs:493`) — i.e. the same payment-state payload used everywhere else. The CRes validation outcome is signalled via the same `outcome` field as Authorize.

### External vs native 3DS

- **Native** (Cybersource, Nexixpay, Worldpay): the connector owns the full trio; UCS invokes PostAuthenticate against its dedicated endpoint.
- **External** (Revolv3 per PR #815): PostAuthenticate stub (`crates/integrations/connector-integration/src/connectors/revolv3.rs:325`). An external authenticator produces the final `AuthenticationData` and the caller feeds it into `PaymentsAuthorizeData<T>` directly. The trio traits are declared but empty at the pinned SHA. See [pattern_preauthenticate.md § External vs native 3DS](./pattern_preauthenticate.md#external-vs-native-3ds) for the cross-cutting policy.

## Code Examples

### 1. Prerequisites tuple (Nexixpay)

```rust
// From crates/integrations/connector-integration/src/connectors/nexixpay.rs:109
(
    flow: PostAuthenticate,
    request_body: NexixpayPostAuthenticateRequest,
    response_body: NexixpayPostAuthenticateResponse,
    router_data: RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>,
),
```

### 2. URL wiring (Nexixpay)

```rust
// From crates/integrations/connector-integration/src/connectors/nexixpay.rs:770
fn get_url(
    &self,
    req: &RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>,
) -> CustomResult<String, IntegrationError> {
    Ok(format!("{}/orders/3steps/validation", self.connector_base_url_payments(req)))
}
```

### 3. Response transformer producing `AuthenticationData` (Nexixpay)

```rust
// From crates/integrations/connector-integration/src/connectors/nexixpay/transformers.rs:1582
Ok(Self {
    response: Ok(PaymentsResponseData::PostAuthenticateResponse {
        authentication_data: response.three_ds_auth_result.as_ref().map(|auth_result| {
            AuthenticationData {
                trans_status: auth_result.status.as_ref()
                    .and_then(|s| s.parse::<common_enums::TransactionStatus>().ok()),
                eci: auth_result.eci.clone(),
                cavv: auth_result.authentication_value.clone().map(Secret::new),
                ucaf_collection_indicator: None,
                threeds_server_transaction_id: auth_result.xid.clone(),
                message_version: auth_result.version.as_ref()
                    .and_then(|v| v.parse::<common_utils::types::SemanticVersion>().ok()),
                // PaRes now read directly from redirect_response in Authorize
                ds_trans_id: None,
                /* other fields default */
                ..Default::default()
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
// From crates/integrations/connector-integration/src/connectors/worldpay/transformers.rs:1314
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
// From crates/integrations/connector-integration/src/connectors/nexixpay/transformers.rs:1570
let status = match &operation.operation_result {
    NexixpayPaymentStatus::ThreedsValidated => AttemptStatus::AuthenticationSuccessful,
    NexixpayPaymentStatus::ThreedsFailed => AttemptStatus::AuthenticationFailed,
    NexixpayPaymentStatus::Declined | NexixpayPaymentStatus::DeniedByRisk => {
        AttemptStatus::AuthenticationFailed
    }
    _ => AttemptStatus::AuthenticationPending,
};
```

### 6. Cybersource request type (shares `PaymentInformation` with PreAuth/Auth)

```rust
// From crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:2933
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

1. **Declare the trait.** `impl<...> connector_types::PaymentPostAuthenticateV2<T> for <Connector><T> {}`. Leave empty for connectors that terminate 3DS at Authenticate (Redsys) or that use external 3DS (Revolv3).
2. **Register the flow.** Add `(flow: PostAuthenticate, request_body: ..., response_body: ..., router_data: RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>)` to `create_all_prerequisites!`.
3. **Emit the `macro_connector_implementation!`** with `flow_name: PostAuthenticate`, `http_method: Post`, and `flow_request: PaymentsPostAuthenticateData<T>`.
4. **Implement `get_url`** to point at the connector's CRes validation endpoint.
5. **Write the request `TryFrom`.** Read the browser CRes out of `request.redirect_response.params`; if your connector needs the authentication transaction id, grab it from the `connector_feature_data` your Authenticate transformer stashed.
6. **Write the response `TryFrom`.** Populate `PaymentsResponseData::PostAuthenticateResponse.authentication_data` with a fully-filled `AuthenticationData`. Map `cavv`, `eci`, `threeds_server_transaction_id`, `ds_trans_id`, `trans_status`, and `message_version` at minimum.
7. **Drive `resource_common_data.status`** from the connector's validation result — `AuthenticationSuccessful`, `AuthenticationFailed`, or `AuthenticationPending` if a second challenge is required. Never hardcode.
8. **Correlate with Authorize.** The final `AuthenticationData` is what downstream Authorize will embed in its card-payer-auth block; ensure `connector_response_reference_id` lines up with the id the subsequent Authorize will receive so PSync can tie everything back together. See the ResponseId shape at `crates/types-traits/domain_types/src/connector_types.rs:1380`.

## Best Practices

- Populate at least `trans_status`, `cavv`, `eci`, and `threeds_server_transaction_id` on the `AuthenticationData` payload; downstream Authorize expects them present per the contract at `crates/types-traits/domain_types/src/router_request_types.rs:136`.
- Fail fast if the connector returns an empty CRes validation payload — a successful 2xx with all fields `None` is almost always an upstream misconfiguration. Return `ConnectorResponseTransformationError`.
- Use `SemanticVersion::parse` for `message_version` rather than string comparisons (see Nexixpay usage at `crates/integrations/connector-integration/src/connectors/nexixpay/transformers.rs:1596`).
- Wrap `cavv` in `Secret<String>` — it is PII-like data. The `AuthenticationData.cavv` field is typed `Option<Secret<String>>` at `crates/types-traits/domain_types/src/router_request_types.rs:139`.
- When a connector emits no dedicated PostAuthenticate endpoint, do not invent one; keep the trait stub. Redsys's `Authenticate` transformer at `crates/integrations/connector-integration/src/connectors/redsys/transformers.rs:688` already emits final `AuthenticationData`.
- Persist `connector_response_reference_id` consistently across Authorize, PreAuthenticate, Authenticate and PostAuthenticate so that PSync (see [pattern_psync.md](./pattern_psync.md)) can stitch the full lifecycle together.

## Common Errors / Gotchas

1. **Problem:** `IntegrationError::MissingRequiredField { field_name: "redirect_response.params" }`.
   **Solution:** The CRes urlencoded payload must be forwarded via `PaymentsPostAuthenticateData.redirect_response.params`. Worldpay's transformer at `crates/integrations/connector-integration/src/connectors/worldpay/transformers.rs:1320` and the Nexixpay comment at `:1598` both assume this convention.
2. **Problem:** Authorize fails with "missing CAVV" right after a successful PostAuthenticate.
   **Solution:** Ensure `PaymentsResponseData::PostAuthenticateResponse.authentication_data.cavv` is populated; downstream Authorize reads it from there (see `crates/integrations/connector-integration/src/connectors/nexixpay/transformers.rs:1591`).
3. **Problem:** `PaymentsResponseData::PostAuthenticateResponse` missing `resource_id`.
   **Solution:** This variant intentionally has no `resource_id` field (`crates/types-traits/domain_types/src/connector_types.rs:1424`). Carry the transaction id on `connector_response_reference_id` instead; PSync stitches using that.
4. **Problem:** Downstream PSync cannot correlate with the authenticated payment.
   **Solution:** Populate `connector_response_reference_id` with the same id that Authorize will emit, and make sure Authorize's `resource_id` and PSync's `connector_transaction_id` align. See Nexixpay's handling at `crates/integrations/connector-integration/src/connectors/nexixpay/transformers.rs:1402`.
5. **Problem:** Use of retired monolithic `ConnectorError` when parsing the CRes response.
   **Solution:** Spec §12 retires that type. Use `ConnectorResponseTransformationError` for response-parse-time failures and `IntegrationError` for request-time failures.
6. **Problem:** `common_enums::TransactionStatus` parsing fails on lowercase strings.
   **Solution:** Normalise the case on the connector payload before calling `.parse::<TransactionStatus>()`. Nexixpay accepts the value as-is because the gateway returns the EMV 3DS single-letter codes verbatim (`crates/integrations/connector-integration/src/connectors/nexixpay/transformers.rs:1587`).
7. **Problem:** Authoring a PostAuthenticate impl for a connector whose trio is only two legs.
   **Solution:** Do not register a macro wiring. Leave `PaymentPostAuthenticateV2<T>` empty (see Redsys at `crates/integrations/connector-integration/src/connectors/redsys.rs:130`).

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

## Appendix A — Two-leg vs three-leg 3DS trio

Not every connector exposes three dedicated HTTP endpoints for 3DS. The connectors at the pinned SHA fall into three camps:

| Connector | Pre | Auth | Post | 3DS trio shape |
| --- | --- | --- | --- | --- |
| Cybersource | yes | yes | yes | Three-leg: `authentication-setups` → `authentications` → `authentication-results`. |
| Nexixpay | yes | stub | yes | Two-leg: `/orders/3steps/init` returns ACS URL; validation is at `/orders/3steps/validation`. |
| Worldpay | yes | stub | yes | Two-leg: `/3dsDeviceData` for DDC; `/3dsChallenges` for CRes validation. |
| Redsys | yes | yes | stub | Two-leg: `/iniciaPeticionREST` for bootstrap; `/trataPeticionREST` handles authenticate+authorize in one shot. |
| NMI | yes (vault) | stub | stub | Single-leg: the vault-add call replaces the full trio for cards that pre-bind to a customer. |
| Revolv3 (external) | stub | stub | stub | Zero-leg: external 3DS provider computes `AuthenticationData` and it is injected directly into Authorize. |
| Stripe, Checkout | stub | stub | stub | No native 3DS trio wiring at the pinned SHA. |

PostAuthenticate is the final step only in the three-leg case. In two-leg connectors, PostAuthenticate is where the browser returns and the connector validates; in single-leg or zero-leg connectors, leave the trait empty and do not register a macro wiring.

## Appendix B — Status mapping reference

Final `AttemptStatus` after PostAuthenticate should be one of:

| `AttemptStatus` | When to emit |
| --- | --- |
| `AuthenticationSuccessful` | CRes validated; `trans_status` is `Y` or `A`; CAVV present. See Nexixpay at `crates/integrations/connector-integration/src/connectors/nexixpay/transformers.rs:1571`. |
| `AuthenticationFailed` | `trans_status` is `N` or `R`; or connector explicitly denied. See Nexixpay at `:1572`. |
| `AuthenticationPending` | Connector requires a second decoupled challenge (rare); or validation response ambiguous. |

Never emit `Started`, `Authorizing`, `Charged`, or any non-auth status from PostAuthenticate — the subsequent Authorize is the only flow that is allowed to transition to `Authorized`/`Charged`. Spec §11 at `grace/rulesbook/codegen/guides/patterns/PATTERN_AUTHORING_SPEC.md` bans hardcoding.

## Appendix C — Correlation id chain

PostAuthenticate sits in the middle of a larger correlation chain. Each step MUST preserve one connector-side identifier so that PSync can trace back through the full lifecycle:

```
Authorize (initial) → PreAuthenticate → Authenticate → PostAuthenticate → Authorize (final) → PSync
         \_____________________________ same correlation id ___________________________/
```

How connectors carry this id:

- **Cybersource** — `id` field on `ClientAuthCheckInfoResponse` flows through to `consumer_authentication_information.authentication_transaction_id` consumed by the PostAuthenticate request (`crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:2929`). Then `connector_response_reference_id` is set from `info_response.client_reference_information.code` (`:3131`).
- **Nexixpay** — `operationId` persisted in `PaymentFlowData.preprocessing_id` at PreAuthenticate (`crates/integrations/connector-integration/src/connectors/nexixpay/transformers.rs:1409`) and consumed at PostAuthenticate when building the `/validation` body.
- **Worldpay** — `link_data` URL fragment stored in `connector_feature_data` at Authorize time; consumed by `Self::extract_link_data_from_metadata(req)?` at every 3DS URL builder (`crates/integrations/connector-integration/src/connectors/worldpay.rs:605`, `:641`).

Never re-invent this scheme per-flow; reuse whatever correlation token your PreAuthenticate / Authenticate transformers already set.

## Cross-References

- Parent index: [./README.md](./README.md)
- Sibling 3DS flows: [pattern_preauthenticate.md](./pattern_preauthenticate.md), [pattern_authenticate.md](./pattern_authenticate.md)
- Sibling flow (non-3DS): [pattern_authorize.md](./pattern_authorize.md)
- Follow-on flow that consumes the AuthenticationData: [pattern_authorize.md](./pattern_authorize.md), with PSync correlation described in [pattern_psync.md](./pattern_psync.md)
- PM pattern (shares 3DS prose; do not edit): [authorize/card/pattern_authorize_card.md](./authorize/card/pattern_authorize_card.md) — see "3D Secure Pattern" around line 445.
- Authoring spec: [./PATTERN_AUTHORING_SPEC.md](./PATTERN_AUTHORING_SPEC.md)
- Types used by this flow: `crates/types-traits/domain_types/src/connector_types.rs:1635` (request), `:1424` (response variant), `:422` (flow data); `crates/types-traits/domain_types/src/router_request_types.rs:136` (AuthenticationData); `crates/types-traits/domain_types/src/connector_flow.rs:56` (marker).
