# Authenticate Flow Pattern

## Overview

Authenticate is the middle leg of the 3D Secure (3DS) trio. After [`PreAuthenticate`](./pattern_preauthenticate.md) has obtained device-data-collection (DDC) output and a 3DS method completion signal, the Authenticate flow runs the actual enrolment lookup and — when the issuer demands it — surfaces a CReq/ACS challenge form to the browser. On success it either produces an already-authenticated `AuthenticationData` payload (frictionless 3DS2) or a `RedirectForm` for the browser challenge whose completion is reported back through [`PostAuthenticate`](./pattern_postauthenticate.md).

This is the flow most sensitive to connector terminology: gateways label it "authentications", "enrolment check", "payer auth check" or "3DS2 lookup". The UCS contract normalises all of these on `domain_types::connector_flow::Authenticate` plus `PaymentsResponseData::AuthenticateResponse`.

### Key Components
- Flow marker: `Authenticate` at `crates/types-traits/domain_types/src/connector_flow.rs:53`.
- Request type: `PaymentsAuthenticateData<T>` at `crates/types-traits/domain_types/src/connector_types.rs:1552`.
- Response type: `PaymentsResponseData::AuthenticateResponse` at `crates/types-traits/domain_types/src/connector_types.rs:1415`.
- Resource common data: `PaymentFlowData` at `crates/types-traits/domain_types/src/connector_types.rs:422`.
- Trait implemented by connectors: `connector_types::PaymentAuthenticateV2<T>`.

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

Inputs (`PaymentsAuthenticateData<T>` at `crates/types-traits/domain_types/src/connector_types.rs:1552`):
- `payment_method_data: Option<PaymentMethodData<T>>` — the card under authentication.
- `amount: MinorUnit`, `currency: Option<Currency>`, `email: Option<Email>` — order-context for 3DS2 risk scoring.
- `router_return_url`, `continue_redirection_url: Option<Url>` — where the browser returns after the challenge.
- `browser_info: Option<BrowserInformation>` — required by 3DS2; helpers `get_browser_info` / `get_continue_redirection_url` at `crates/types-traits/domain_types/src/connector_types.rs:1584` and `:1590` surface `MissingRequiredField` when absent.
- `enrolled_for_3ds: bool` — reflects the router's decision from PreAuthenticate.
- `redirect_response: Option<ContinueRedirectionResponse>` — populated when the browser has returned with 3DS method completion.
- `authentication_data: Option<AuthenticationData>` — DDC/3DS method data from the previous flow (`threeds_server_transaction_id`, etc.).

Outputs (`PaymentsResponseData::AuthenticateResponse` at `crates/types-traits/domain_types/src/connector_types.rs:1415`):
- `resource_id: Option<ResponseId>` — connector-side transaction reference for this authentication attempt.
- `redirection_data: Option<Box<RedirectForm>>` — "For friction flow" per the inline comment (`connector_types.rs:1417`).
- `authentication_data: Option<AuthenticationData>` — "For frictionles flow" per the inline comment (`connector_types.rs:1419`).
- `connector_response_reference_id: Option<String>`, `status_code: u16`.

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
// From crates/types-traits/domain_types/src/connector_flow.rs:52
#[derive(Debug, Clone)]
pub struct Authenticate;
```

### Request Type

```rust
// From crates/types-traits/domain_types/src/connector_types.rs:1552
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
}
```

The impl block at `crates/types-traits/domain_types/src/connector_types.rs:1567` adds `is_auto_capture`, `get_browser_info`, and `get_continue_redirection_url` helpers.

### Response Type

```rust
// From crates/types-traits/domain_types/src/connector_types.rs:1415
AuthenticateResponse {
    resource_id: Option<ResponseId>,
    /// For friction flow
    redirection_data: Option<Box<RedirectForm>>,
    /// For frictionles flow
    authentication_data: Option<router_request_types::AuthenticationData>,
    connector_response_reference_id: Option<String>,
    status_code: u16,
},
```

`AuthenticationData` is the same struct used across the 3DS trio (`crates/types-traits/domain_types/src/router_request_types.rs:136`). See [pattern_preauthenticate.md](./pattern_preauthenticate.md) and [pattern_postauthenticate.md](./pattern_postauthenticate.md) for how the field set evolves across flows.

### Resource Common Data

`PaymentFlowData` (`crates/types-traits/domain_types/src/connector_types.rs:422`). Authenticate transformers map the connector's "authenticationStatus" onto `AttemptStatus`. The full implementations at the pinned SHA produce three outcomes: `AuthenticationPending` (challenge required), `AuthenticationSuccessful` (frictionless Y), or `AuthenticationFailed` (issuer denial). See Cybersource at `crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:3110`.

## Connectors with Full Implementation

| Connector | HTTP Method | Content Type | URL Pattern | Request Type Reuse | Notes |
| --- | --- | --- | --- | --- | --- |
| Cybersource | POST | `application/json;charset=utf-8` | `{base}risk/v1/authentications` | `CybersourceAuthEnrollmentRequest<T>` (dedicated; `crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:2912`) | Produces `AuthenticateResponse` with `redirection_data` for `ChallengeRequired` or `authentication_data` for frictionless; mapping at `crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:3109`. URL wired at `crates/integrations/connector-integration/src/connectors/cybersource.rs:659`. |
| Redsys | POST | `application/json` | `{base}/sis/rest/trataPeticionREST` | `RedsysAuthenticateRequest` (alias of `RedsysTransaction` at `crates/integrations/connector-integration/src/connectors/redsys/requests.rs:7`) | Reuses the `trataPeticionREST` endpoint also used by Authorize; discriminator is carried in the body. URL at `crates/integrations/connector-integration/src/connectors/redsys.rs:400`. Transformer at `crates/integrations/connector-integration/src/connectors/redsys/transformers.rs:688` emits `PaymentsResponseData::AuthenticateResponse`. |

### Stub Implementations

At the pinned SHA, Authenticate has only two native implementations. The following connectors declare `PaymentAuthenticateV2<T>` and/or `ConnectorIntegrationV2<Authenticate, ...>` but ship empty bodies:

- Nexixpay — empty impl at `crates/integrations/connector-integration/src/connectors/nexixpay.rs:740`. Nexixpay skips this flow because its `/orders/3steps/init` response in PreAuthenticate already carries the ACS URL; the second trip is only needed for PostAuthenticate validation.
- Worldpay — the Authenticate trait appears only in the macro registration for PreAuthenticate/PostAuthenticate; there is no `flow: Authenticate` tuple in its `create_all_prerequisites!` block (`crates/integrations/connector-integration/src/connectors/worldpay.rs:236` jumps from PreAuthenticate straight to PostAuthenticate).
- Stripe — empty impl at `crates/integrations/connector-integration/src/connectors/stripe.rs:1079`.
- Checkout — empty impl at `crates/integrations/connector-integration/src/connectors/checkout.rs:701`.
- Revolv3 — empty impl at `crates/integrations/connector-integration/src/connectors/revolv3.rs:295`. See the [external vs native 3DS](#external-vs-native-3ds) discussion below.
- NMI — NMI does not expose a separate enrolment-check endpoint; Authenticate is not registered in its prerequisites block (`crates/integrations/connector-integration/src/connectors/nmi.rs:245`).

## Common Implementation Patterns

### Pattern A — Dedicated Authenticate endpoint (Cybersource)

Cybersource runs the strict 3-leg variant: PreAuth (`authentication-setups`) → Authenticate (`authentications`) → PostAuth (`authentication-results`). Each leg has its own URL and request/response structs. Authenticate's request `CybersourceAuthEnrollmentRequest<T>` (`crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:2912`) adds `consumer_authentication_information` and `order_information` on top of the `CybersourceAuthSetupRequest` shape.

### Pattern B — Shared transaction body with operation-type discriminator (Redsys)

Redsys sends every non-bootstrap operation to the same `/sis/rest/trataPeticionREST` URL and distinguishes them by the `DS_MERCHANT_TRANSACTIONTYPE` field inside the request body. `RedsysAuthenticateRequest` is therefore a type alias over `RedsysTransaction` (`crates/integrations/connector-integration/src/connectors/redsys/requests.rs:7`), with the Authenticate transformer at `crates/integrations/connector-integration/src/connectors/redsys/transformers.rs:928` filling in the appropriate discriminator.

### Pattern C — Skipped (Nexixpay, Worldpay)

Some connectors collapse three 3DS legs into two. In these the `Authenticate` trait exists but has an empty body; the challenge form is issued from PreAuthenticate and the CRes validation is done in PostAuthenticate. Do not add a macro wiring for a flow the connector does not need.

## Connector-Specific Patterns

### Cybersource

- `CybersourceAuthEnrollmentRequest<T>` carries `payment_information`, `client_reference_information`, `consumer_authentication_information: CybersourceConsumerAuthInformationRequest` (only `return_url` + `reference_id`; see `crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:2906`), and `order_information: OrderInformationWithBill`.
- Response enum `CybersourceAuthenticateResponse` (`crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:3093`) is untagged with `ClientAuthCheckInfo` and `ErrorInformation` variants. The success path inspects `info_response.consumer_authentication_information` to decide between a challenge (`CardChallenged` → `ChallengeRequired`) and a frictionless outcome (mapping at `:3138`).
- The response transformer sets `AttemptStatus` directly from `info_response.status` at `crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:3110`.

### Redsys

- Uses the shared `RedsysTransaction` body; transformer at `crates/integrations/connector-integration/src/connectors/redsys/transformers.rs:688` emits either a `RedirectForm` carrying the CReq or an early `AuthenticationData` payload.
- `to_connector_response_data` at `crates/integrations/connector-integration/src/connectors/redsys/transformers.rs:868` decrypts the base64 `DS_MERCHANT_PARAMETERS` blob and feeds it into the transformer; the decryption is shared by Authenticate and PreAuthenticate.

### External vs native 3DS

- **Native** (Cybersource, Redsys): the UCS router invokes `Authenticate` against the connector's dedicated endpoint.
- **External** (Revolv3 per PR #815): Authenticate is a stub. The 3DS outcome (CAVV/ECI/DS-Trans-ID) is computed by an external authenticator and passed through on the Authorize call via `PaymentsAuthorizeData<T>`. See [pattern_preauthenticate.md § External vs native 3DS](./pattern_preauthenticate.md#external-vs-native-3ds) for the cross-cutting policy.

## Code Examples

### 1. Prerequisites tuple (Cybersource)

```rust
// From crates/integrations/connector-integration/src/connectors/cybersource.rs:243
(
    flow: Authenticate,
    request_body: CybersourceAuthEnrollmentRequest<T>,
    response_body: CybersourceAuthenticateResponse,
    router_data: RouterDataV2<Authenticate, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>,
),
```

### 2. URL wiring (Cybersource)

```rust
// From crates/integrations/connector-integration/src/connectors/cybersource.rs:659
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
// From crates/integrations/connector-integration/src/connectors/redsys/transformers.rs:688
Ok(PaymentsResponseData::AuthenticateResponse {
    resource_id: ..., // populated from DS_ORDER / DS_TRANSACTION_ID
    redirection_data: None,
    authentication_data: Some(AuthenticationData {
        trans_status: ..., // from DS_EMV3DS.transStatus
        eci: ...,
        cavv: ...,
        ..Default::default()
    }),
    connector_response_reference_id: ...,
    status_code: item.http_code,
})
```

### 4. Challenge branch producing a `RedirectForm` (Cybersource)

```rust
// From crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:3109
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

The exact field-by-field CReq construction lives in the Cybersource transformer around `:3138`; it is not reproduced verbatim here but follows the `RedirectForm::Form` shape used everywhere in UCS.

### 5. `get_browser_info` helper contract

```rust
// From crates/types-traits/domain_types/src/connector_types.rs:1584
pub fn get_browser_info(&self) -> Result<BrowserInformation, Error> {
    self.browser_info
        .clone()
        .ok_or_else(missing_field_err("browser_info"))
}
```

### 6. External-3DS stub (Revolv3)

```rust
// From crates/integrations/connector-integration/src/connectors/revolv3.rs:295
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        connector_flow::Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    > for Revolv3<T>
{
}
```

## Integration Guidelines

1. **Declare the trait.** `impl<...> connector_types::PaymentAuthenticateV2<T> for <Connector><T> {}`. Leave it empty only if the connector relies on external 3DS or collapses three legs into two.
2. **Register the flow.** Add the `(flow: Authenticate, request_body: ..., response_body: ..., router_data: RouterDataV2<Authenticate, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>)` tuple to `create_all_prerequisites!`.
3. **Emit a `macro_connector_implementation!`** with `flow_name: Authenticate`, `http_method: Post`, and `flow_request: PaymentsAuthenticateData<T>`.
4. **Implement `get_url`** to hit the connector's enrolment-check endpoint.
5. **Write the request `TryFrom`.** Pull card data out of `payment_method_data`, browser info via `request.get_browser_info()?` (defined at `crates/types-traits/domain_types/src/connector_types.rs:1584`), and the DDC correlation id out of the incoming `authentication_data` field.
6. **Write the response `TryFrom`.** Map to `PaymentsResponseData::AuthenticateResponse`. Populate `authentication_data` for the frictionless Y/R path and `redirection_data` (usually `RedirectForm::Form`) for the challenge path. Set `resource_common_data.status` from the connector's authentication outcome — do not hardcode.
7. **Persist the acs_transaction_id / threeds_server_transaction_id** on the `AuthenticationData` payload so PostAuthenticate can correlate the CRes with the original Authenticate call. See the shape at `crates/types-traits/domain_types/src/router_request_types.rs:136`.
8. **Reuse connector-wide signing.** Cybersource shows how the `build_headers` helper at `crates/integrations/connector-integration/src/connectors/cybersource.rs:300` handles HMAC signing uniformly across all flows; Authenticate must route through that rather than reimplementing the signature locally.
9. **Errors.** Return `IntegrationError` for request-time failures (`MissingRequiredField` etc.) and `ConnectorResponseTransformationError` for response parsing; the retired monolithic `ConnectorError` MUST NOT appear.

## Best Practices

- Read `browser_info` via `request.get_browser_info()` so the connector surfaces `MissingRequiredField { field_name: "browser_info" }` uniformly rather than panicking (see `crates/types-traits/domain_types/src/connector_types.rs:1584`).
- Keep challenge forms keyed on the fields the ACS expects: `creq`, `threeDSSessionData`, `TermUrl`, `MD`. The generic `RedirectForm::Form { endpoint, method, form_fields }` shape accommodates all of them.
- When the connector supports both 3DS1 and 3DS2, branch inside the response transformer on the connector's `messageVersion` / `paresStatus`; do not force a single mapping. See `CybersourceParesStatus::CardChallenged → TransactionStatus::ChallengeRequired` at `crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:402`.
- Use `AttemptStatus::AuthenticationSuccessful` only when `trans_status` is `Y` (Success) or `A` (Attempted with liability shift). Challenge-required outcomes stay on `AuthenticationPending`. The enum is defined alongside `AuthenticationData` and is documented in `grace/rulesbook/codegen/guides/patterns/authorize/card/pattern_authorize_card.md` around line 488.
- When reusing a shared transaction struct across flows (Redsys), keep one alias per flow in `requests.rs` for grepability rather than referencing the underlying struct everywhere.
- For frictionless-only connectors, set `redirection_data: None` explicitly in the response so the router does not accidentally render a blank redirect.

## Common Errors / Gotchas

1. **Problem:** `MissingRequiredField { field_name: "browser_info" }` during Authenticate.
   **Solution:** 3DS2 requires browser info. Use `request.get_browser_info()?` at `crates/types-traits/domain_types/src/connector_types.rs:1584`, and ensure the caller populates `PaymentsAuthenticateData.browser_info`.
2. **Problem:** Frictionless path returns CAVV but router still triggers a redirect.
   **Solution:** Place the CAVV on `authentication_data`, not `redirection_data`. The response enum documents this split in-line ("For friction flow" vs "For frictionles flow" at `crates/types-traits/domain_types/src/connector_types.rs:1417`).
3. **Problem:** PSync returns "unknown transaction" after a successful Authenticate.
   **Solution:** Populate `resource_id: Some(ResponseId::ConnectorTransactionId(..))` in the `AuthenticateResponse` so later flows can correlate; see the ResponseId type at `crates/types-traits/domain_types/src/connector_types.rs:1380`.
4. **Problem:** Status mapping defaults to `AttemptStatus::Started` after Authenticate.
   **Solution:** Drive `resource_common_data.status` from the connector's `authenticationStatus` field. `AttemptStatus::AuthenticationPending` for challenges, `AuthenticationSuccessful` for frictionless Y/A, `AuthenticationFailed` for denial. Never hardcode (spec §11 at `grace/rulesbook/codegen/guides/patterns/PATTERN_AUTHORING_SPEC.md`).
5. **Problem:** Retired `ConnectorError` type referenced in error-handling code.
   **Solution:** Replace with `IntegrationError` (request-time) and `ConnectorResponseTransformationError` (response-parse-time) per spec §12.
6. **Problem:** Connector has only two legs but pattern author wrote a full Authenticate impl.
   **Solution:** Check the connector docs — Nexixpay and Worldpay collapse Authenticate into PreAuthenticate. Keep the trait impl empty (see `crates/integrations/connector-integration/src/connectors/nexixpay.rs:740`).

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
| Connector unreachable | DNS / 5xx | Error surfaced via `build_error_response`; body parse produces `ConnectorResponseTransformationError`. |

## Appendix A — `TransactionStatus` discriminator

The `trans_status` field threaded through all three flows is a `common_enums::TransactionStatus` whose variants encode the EMV 3DS ACS outcome. The card PM pattern documents the enum; this flow maps its values onto UCS status handling as follows (all rows verified against the canonical variant set referenced in `grace/rulesbook/codegen/guides/patterns/authorize/card/pattern_authorize_card.md` around line 488 and reproduced in Cybersource's pares mapping at `crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:402`):

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

When writing the response `TryFrom`, the following connector fields typically feed into the canonical `AuthenticationData` shape (`crates/types-traits/domain_types/src/router_request_types.rs:136`):

| `AuthenticationData` field | Source in Cybersource | Source in Redsys |
| --- | --- | --- |
| `trans_status` | `consumer_authentication_information.pares_status` | `DS_EMV3DS.transStatus` |
| `eci` | `consumer_authentication_information.eci` | `DS_EMV3DS.eci` |
| `cavv` | `consumer_authentication_information.cavv` (wrap in `Secret::new`) | `DS_EMV3DS.authValue` |
| `ucaf_collection_indicator` | `consumer_authentication_information.ucaf_collection_indicator` | `DS_EMV3DS.ucafCollectionIndicator` |
| `threeds_server_transaction_id` | `consumer_authentication_information.xid` | `DS_EMV3DS.threeDSServerTransID` |
| `ds_trans_id` | `consumer_authentication_information.directory_server_transaction_id` | `DS_EMV3DS.dsTransID` |
| `acs_transaction_id` | `consumer_authentication_information.acs_transaction_id` | `DS_EMV3DS.acsTransID` |
| `message_version` | `consumer_authentication_information.specification_version` (parsed via `SemanticVersion::parse`) | `DS_EMV3DS.protocolVersion` |
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
- PM pattern (shares 3DS prose; do not edit): [authorize/card/pattern_authorize_card.md](./authorize/card/pattern_authorize_card.md) — see the "3D Secure Pattern" around line 445 and the `TransactionStatus` enum around line 488.
- Authoring spec: [./PATTERN_AUTHORING_SPEC.md](./PATTERN_AUTHORING_SPEC.md)
- Types used by this flow: `crates/types-traits/domain_types/src/connector_types.rs:1552` (request), `:1415` (response variant), `:422` (flow data); `crates/types-traits/domain_types/src/router_request_types.rs:136` (AuthenticationData); `crates/types-traits/domain_types/src/connector_flow.rs:53` (marker).
