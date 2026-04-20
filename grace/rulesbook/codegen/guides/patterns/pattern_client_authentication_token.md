# ClientAuthenticationToken Flow Pattern

## Overview

The `ClientAuthenticationToken` flow produces a short-lived, client-safe token (a "client secret", "session data", or SDK-init payload) that a frontend can hand to a connector's browser/mobile SDK to complete the remainder of a payment from the device. The flow is run server-side by UCS before any confirm/authorize step: the connector returns an opaque artifact (for Stripe, a PaymentIntent `client_secret`) and UCS forwards that to the client without exposing the merchant's API keys. Unlike `ServerSessionAuthenticationToken` (which returns wallet-session blobs for Apple Pay / Google Pay / PayPal SDKs) and `ServerAuthentication` (OAuth bearer for subsequent server-to-server calls), `ClientAuthenticationToken` is expressly *client-bound credential issuance for a single checkout context*.

This pattern was introduced by PR #855 with Stripe as the reference implementation; additional full implementations have since landed (Globalpay PR #957, Bluesnap PR #959, Jpmorgan PR #966 — enumerated below). PR #1002 then consolidated all per-connector SDK-init response shapes into the shared `ConnectorSpecificClientAuthenticationResponse` enum so each new connector adds one arm rather than creating a parallel type.

Key Components:
- Flow marker struct: `ClientAuthenticationToken` — `crates/types-traits/domain_types/src/connector_flow.rs:62`.
- Request data: `ClientAuthenticationTokenRequestData` — `crates/types-traits/domain_types/src/connector_types.rs:1607`.
- Response data: `PaymentsResponseData::ClientAuthenticationTokenResponse { session_data, status_code }` — `crates/types-traits/domain_types/src/connector_types.rs:1404`.
- Session-data payload enum: `ClientAuthenticationTokenData` — `crates/types-traits/domain_types/src/connector_types.rs:3417`.
- Per-connector discriminator: `ConnectorSpecificClientAuthenticationResponse` — `crates/types-traits/domain_types/src/connector_types.rs:3432`.
- Stripe SDK-init shape: `StripeClientAuthenticationResponse { client_secret: Secret<String> }` — `crates/types-traits/domain_types/src/connector_types.rs:3439`.
- Trait: `interfaces::connector_types::ClientAuthentication` — `crates/types-traits/interfaces/src/connector_types.rs:174`.
- Flow-name enum entry: `FlowName::ClientAuthenticationToken` — `crates/types-traits/domain_types/src/connector_flow.rs:122`.
- Primary connector file: `crates/integrations/connector-integration/src/connectors/stripe.rs`.
- Primary connector transformers: `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs`.

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Relationship to other token flows](#relationship-to-other-token-flows)
3. [Connectors with Full Implementation](#connectors-with-full-implementation)
   - [Shared-types consolidation (PR #1002)](#shared-types-consolidation-pr-1002)
4. [Common Implementation Patterns](#common-implementation-patterns)
5. [Connector-Specific Patterns](#connector-specific-patterns)
6. [Code Examples](#code-examples)
7. [Integration Guidelines](#integration-guidelines)
8. [Best Practices](#best-practices)
9. [Common Errors / Gotchas](#common-errors--gotchas)
10. [Testing Notes](#testing-notes)
11. [Retired / pre-rename identifiers](#retired--pre-rename-identifiers)
12. [Cross-References](#cross-references)
13. [Change Log](#change-log)

## Architecture Overview

### Flow Hierarchy

```
PaymentFlowData (shared resource_common_data)
└── ClientAuthenticationToken (flow marker)
    └── request : ClientAuthenticationTokenRequestData
    └── response: PaymentsResponseData::ClientAuthenticationTokenResponse
            └── session_data : ClientAuthenticationTokenData
                    ├── GooglePay(Box<GpayClientAuthenticationResponse>)
                    ├── Paypal(Box<PaypalClientAuthenticationResponse>)
                    ├── ApplePay(Box<ApplepayClientAuthenticationResponse>)
                    └── ConnectorSpecific(Box<ConnectorSpecificClientAuthenticationResponse>)
                            └── Stripe(StripeClientAuthenticationResponse { client_secret })
```

The generic router-data template, per §7 of `PATTERN_AUTHORING_SPEC.md`, is:

```rust
RouterDataV2<
    ClientAuthenticationToken,              // flow marker
    PaymentFlowData,                        // resource_common_data
    ClientAuthenticationTokenRequestData,   // request
    PaymentsResponseData,                   // response (shared enum)
>
```

### Flow Type

`ClientAuthenticationToken` — defined at `crates/types-traits/domain_types/src/connector_flow.rs:62`:

```rust
// From crates/types-traits/domain_types/src/connector_flow.rs:61-62
#[derive(Debug, Clone)]
pub struct ClientAuthenticationToken;
```

It is listed in the `FlowName` enum at `crates/types-traits/domain_types/src/connector_flow.rs:122` so it can be rendered in telemetry and logs.

### Request Type

`ClientAuthenticationTokenRequestData` — `crates/types-traits/domain_types/src/connector_types.rs:1607`:

```rust
// From crates/types-traits/domain_types/src/connector_types.rs:1606-1618
#[derive(Debug, Clone)]
pub struct ClientAuthenticationTokenRequestData {
    pub amount: MinorUnit,
    pub currency: Currency,
    pub country: Option<common_enums::CountryAlpha2>,
    pub order_details: Option<Vec<payment_address::OrderDetailsWithAmount>>,
    pub email: Option<Email>,
    pub customer_name: Option<Secret<String>>,
    pub order_tax_amount: Option<MinorUnit>,
    pub shipping_cost: Option<MinorUnit>,
    /// The specific payment method type for which the session token is being generated
    pub payment_method_type: Option<PaymentMethodType>,
}
```

Note the shape is deliberately richer than a minimal session-token request: the connector may need currency/country to decide which wallet offers to surface, or amount to create an authorization envelope that the client-side SDK later confirms.

### Response Type

`PaymentsResponseData::ClientAuthenticationTokenResponse { session_data, status_code }` — `crates/types-traits/domain_types/src/connector_types.rs:1404-1407`:

```rust
// From crates/types-traits/domain_types/src/connector_types.rs:1404-1407
ClientAuthenticationTokenResponse {
    session_data: ClientAuthenticationTokenData,
    status_code: u16,
},
```

`ClientAuthenticationTokenData` is defined at `crates/types-traits/domain_types/src/connector_types.rs:3417` and is `#[serde(tag = "sdk_type")]`:

```rust
// From crates/types-traits/domain_types/src/connector_types.rs:3414-3426
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "sdk_type")]
#[serde(rename_all = "snake_case")]
pub enum ClientAuthenticationTokenData {
    /// The session response structure for Google Pay
    GooglePay(Box<GpayClientAuthenticationResponse>),
    /// The session response structure for PayPal
    Paypal(Box<PaypalClientAuthenticationResponse>),
    /// The session response structure for Apple Pay
    ApplePay(Box<ApplepayClientAuthenticationResponse>),
    /// Generic connector-specific SDK initialization data
    ConnectorSpecific(Box<ConnectorSpecificClientAuthenticationResponse>),
}
```

`ConnectorSpecificClientAuthenticationResponse` is the extension point connectors use when their SDK-init shape does not match the Google Pay / Apple Pay / PayPal canonical types:

```rust
// From crates/types-traits/domain_types/src/connector_types.rs:3428-3441
/// Per-connector SDK initialization data — discriminated by connector
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "connector")]
#[serde(rename_all = "snake_case")]
pub enum ConnectorSpecificClientAuthenticationResponse {
    /// Stripe SDK initialization data
    Stripe(StripeClientAuthenticationResponse),
}

/// Stripe's client_secret for browser-side stripe.confirmPayment()
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StripeClientAuthenticationResponse {
    pub client_secret: Secret<String>,
}
```

### Resource Common Data

`PaymentFlowData` — `crates/types-traits/domain_types/src/connector_types.rs:422`. The flow piggy-backs on the same `PaymentFlowData` used by Authorize/Capture/PSync/Void so that a subsequent confirm call sees the same merchant/attempt context. Note that `PaymentFlowData.session_token: Option<String>` exists at `crates/types-traits/domain_types/src/connector_types.rs:441`, but `ClientAuthenticationToken` does NOT write to it — the artifact is returned inside `PaymentsResponseData` via the typed `session_data` field.

### Trait

`interfaces::connector_types::ClientAuthentication` — `crates/types-traits/interfaces/src/connector_types.rs:174-182`:

```rust
// From crates/types-traits/interfaces/src/connector_types.rs:174-182
pub trait ClientAuthentication:
    ConnectorIntegrationV2<
    connector_flow::ClientAuthenticationToken,
    PaymentFlowData,
    ClientAuthenticationTokenRequestData,
    PaymentsResponseData,
>
{
}
```

This trait is a constituent of `ConnectorServiceTrait` at `crates/types-traits/interfaces/src/connector_types.rs:79`, meaning every full connector must at least blanket-impl it (even as a stub) to satisfy the trait-bound on the overall service.

## Relationship to other token flows

`ClientAuthenticationToken` lives in a family of adjacent but distinct flows. They were renamed together in PR #855 to remove ambiguity between "token issued to our server" and "token issued to the merchant's client". The table below is the authoritative map at the pinned SHA.

| Flow marker (`domain_types::connector_flow::*`) | Trait (`interfaces::connector_types::*`) | Request data (`domain_types::connector_types::*`) | Response data | Audience of the resulting token | Primary purpose |
|---|---|---|---|---|---|
| `ClientAuthenticationToken` (this pattern) — `connector_flow.rs:62` | `ClientAuthentication` — `interfaces::connector_types.rs:174` | `ClientAuthenticationTokenRequestData` — `connector_types.rs:1607` | `PaymentsResponseData::ClientAuthenticationTokenResponse { session_data: ClientAuthenticationTokenData, .. }` — `connector_types.rs:1404` | **Merchant's client device** (browser/mobile SDK) | Issue a short-lived, client-safe artifact (e.g. Stripe `client_secret`) that the frontend presents to the connector's SDK to confirm the payment. |
| `ServerSessionAuthenticationToken` — `connector_flow.rs:38` | `ServerSessionAuthentication` — `interfaces::connector_types.rs:164` | `ServerSessionAuthenticationTokenRequestData` — `connector_types.rs:1688` | `ServerSessionAuthenticationTokenResponseData { session_token: String }` — `connector_types.rs:1703` | **Merchant's backend** (sometimes forwarded to client as wallet session) | Obtain a session token used as input to wallet-session bootstraps (Apple Pay / Google Pay / PayPal) — see `pattern_server_session_authentication_token.md`. |
| `ServerAuthenticationToken` — `connector_flow.rs:41` | `ServerAuthentication` — `interfaces::connector_types.rs:184` | `ServerAuthenticationTokenRequestData { grant_type: String }` — `connector_types.rs:1708` | `ServerAuthenticationTokenResponseData { access_token: Secret<String>, token_type, expires_in }` — `connector_types.rs:1713` | **Merchant's backend only** (never leaves the server) | OAuth 2.0 bearer acquisition for subsequent server-to-server API calls. Stored on `PaymentFlowData.access_token` — see `pattern_server_authentication_token.md`. |
| `CreateAccessToken` (retired name) | `PaymentAccessToken` (retired) | `AccessTokenRequestData` (retired) | `AccessTokenResponseData` (retired) | — | Replaced by `ServerAuthenticationToken` + `ServerAuthentication` per PR #855. See "Retired / pre-rename identifiers" below. Cross-ref to `pattern_server_authentication_token.md` (renamed from `pattern_CreateAccessToken_flow.md` during PR #855 absorption) which documents the same OAuth flow semantics under the post-rename identifiers. |
| `CreateSessionToken` (retired name) | `PaymentSessionToken` (retired) | `SessionTokenRequestData` (retired) | `SessionTokenResponseData` (retired) | — | Replaced by `ServerSessionAuthenticationToken` + `ServerSessionAuthentication` per PR #855. See `pattern_server_session_authentication_token.md` (renamed from `pattern_session_token.md` during PR #855 absorption) for the post-rename prose. |
| `SdkSessionToken` (retired name) | `SdkSessionTokenV2` (retired) | `PaymentsSdkSessionTokenData` (retired) | `PaymentsResponseData::SdkSessionTokenResponse { session_token: SessionToken }` (retired) | — | Replaced by `ClientAuthenticationToken` + `ClientAuthentication` + `ClientAuthenticationTokenRequestData` + `PaymentsResponseData::ClientAuthenticationTokenResponse` per PR #855. |

Key invariants:

1. **Audience**: If the token is meant to cross the trust boundary to the merchant's end-user device, use `ClientAuthenticationToken`. If it is a server-only credential, use `ServerAuthenticationToken`. If it is a wallet-specific session bootstrap consumed server-side, use `ServerSessionAuthenticationToken`.
2. **Storage location**: `ServerAuthenticationToken` writes to `PaymentFlowData.access_token` (`connector_types.rs:440`). `ServerSessionAuthenticationToken` returns `session_token: String` in its response struct. `ClientAuthenticationToken` returns a typed `ClientAuthenticationTokenData` enum inside `PaymentsResponseData` and does NOT populate `PaymentFlowData.session_token`.
3. **Response envelope**: `ClientAuthenticationToken` reuses the shared `PaymentsResponseData` enum (same as Authorize/Capture/PSync) with a dedicated variant; `ServerSessionAuthenticationToken` and `ServerAuthenticationToken` each use their own dedicated struct type (not `PaymentsResponseData`).

Seeing an older pattern that still talks about `CreateSessionToken` / `SessionTokenRequestData` / `SdkSessionToken` → treat it as pre-rename and translate via the table above before copying.

## Connectors with Full Implementation

At the pinned SHA, Stripe, Globalpay, Bluesnap, and Jpmorgan all have full `ClientAuthenticationToken` implementations (added by PRs #855, #957, #959, and #966 respectively). The remaining connectors listed in `crates/integrations/connector-integration/src/connectors/` blanket-impl the `ClientAuthentication` trait as a stub so they satisfy the `ConnectorServiceTrait` bound (`crates/types-traits/interfaces/src/connector_types.rs:79`) without wiring a real endpoint. A stub impl does not register the flow into `macros::create_all_prerequisites!` nor use `macros::macro_connector_implementation!` for this flow; the default `ConnectorIntegrationV2` method bodies (no-op / `Err(IntegrationError::NotImplemented ...)`) apply.

Rows are alphabetical by connector (per §10 of `PATTERN_AUTHORING_SPEC.md`).

| Connector | HTTP Method | Content Type | URL Pattern | Request Type Reuse | Notes |
| --- | --- | --- | --- | --- | --- |
| Bluesnap | POST | `application/json` (empty body) | `{base_url}/services/2/payment-fields-tokens` | `BluesnapClientAuthRequest` (bespoke, empty marker struct — `crates/integrations/connector-integration/src/connectors/bluesnap/transformers.rs:927-928`) | Bluesnap's Hosted Payment Fields endpoint returns the `pfToken` in the **HTTP `Location` header** (last path segment), not the body. The flow uses a hand-written `ConnectorIntegrationV2` impl (not the macro) so `handle_response_v2` can extract the header. The extracted token is wrapped as `BluesnapClientAuthenticationResponse { pf_token }` → `ConnectorSpecificClientAuthenticationResponse::Bluesnap` → `ClientAuthenticationTokenData::ConnectorSpecific`. See `crates/integrations/connector-integration/src/connectors/bluesnap.rs:684-802` (hand-written impl) and `crates/integrations/connector-integration/src/connectors/bluesnap/transformers.rs:921-1011` (TryFrom impls). Macro-tuple registration is still present at `crates/integrations/connector-integration/src/connectors/bluesnap.rs:432-437`. Trait blanket-impl at `crates/integrations/connector-integration/src/connectors/bluesnap.rs:100-103`. |
| Globalpay | POST | `application/json` | `{base_url}/accesstoken` | `GlobalpayClientAuthRequest` (bespoke; carries `app_id`, `nonce`, SHA-512 `secret = SHA512(nonce + app_key)`, `grant_type: "client_credentials"` — `crates/integrations/connector-integration/src/connectors/globalpay/transformers.rs:1074-1080`, nonce+secret derivation at `:1115-1132`) | Reuses the same `/accesstoken` endpoint as Globalpay's `ServerAuthenticationToken` flow but deserializes into a separate response type so it routes to `ClientAuthenticationTokenResponse` instead of populating `PaymentFlowData.access_token`. The token, type, and `seconds_to_expire` are wrapped as `GlobalpayClientAuthenticationResponse { access_token, token_type, expires_in }` → `ConnectorSpecificClientAuthenticationResponse::Globalpay` → `ClientAuthenticationTokenData::ConnectorSpecific`. See `crates/integrations/connector-integration/src/connectors/globalpay.rs:657-694` and `crates/integrations/connector-integration/src/connectors/globalpay/transformers.rs:1069-1185`. Macro-tuple registration at `crates/integrations/connector-integration/src/connectors/globalpay.rs:106-111`. Trait blanket-impl at `crates/integrations/connector-integration/src/connectors/globalpay.rs:265-268`. |
| Jpmorgan | POST | `application/x-www-form-urlencoded` | `{secondary_base_url}/am/oauth2/alpha/access_token` | `JpmorganClientAuthRequest { grant_type: "client_credentials", scope }` — `crates/integrations/connector-integration/src/connectors/jpmorgan/requests.rs:9-13`. The `scope` is `"jpm:payments:sandbox"` in test mode, `"jpm:payments"` otherwise (`crates/integrations/connector-integration/src/connectors/jpmorgan/transformers.rs:941-950`). | Jpmorgan reuses its OAuth2 token endpoint (Basic auth over `client_id:client_secret`, base64-encoded) to issue a client-side access token. The endpoint lives on a **secondary base URL** distinct from the payments base URL — if `secondary_base_url` is unset, URL building fails with `FailedToObtainIntegrationUrl` carrying a documentation pointer (`crates/integrations/connector-integration/src/connectors/jpmorgan.rs:587-611`). The returned `access_token` and `token_type` are mapped into `JpmorganClientAuthenticationResponse { transaction_id, request_id }` → `ConnectorSpecificClientAuthenticationResponse::Jpmorgan` → `ClientAuthenticationTokenData::ConnectorSpecific` (`crates/integrations/connector-integration/src/connectors/jpmorgan/transformers.rs:958-989`). See also macro block at `crates/integrations/connector-integration/src/connectors/jpmorgan.rs:546-618` and macro-tuple registration at `crates/integrations/connector-integration/src/connectors/jpmorgan.rs:320-325`. Trait blanket-impl at `crates/integrations/connector-integration/src/connectors/jpmorgan.rs:118-121`. |
| Stripe | POST | `application/x-www-form-urlencoded` | `{base_url}v1/payment_intents` | `StripeClientAuthRequest` (bespoke; wraps `PaymentIntent` creation without `confirm=true` — see `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:5350`) | Returned `client_secret` is wrapped as `StripeClientAuthenticationResponse` → `ConnectorSpecificClientAuthenticationResponse::Stripe` → `ClientAuthenticationTokenData::ConnectorSpecific`. See `crates/integrations/connector-integration/src/connectors/stripe.rs:993-1023` and `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:5343-5447`. |

### Stub Implementations

The following connectors `impl ClientAuthentication` as empty bodies only (grep `connector_types::ClientAuthentication for <Connector>` in `crates/integrations/connector-integration/src/connectors/`). They do not implement the actual flow and will fall through to the default `ConnectorIntegrationV2` no-op:

Aci, Adyen, Airwallex, Authipay, Authorizedotnet, Bambora, BamboraAPAC, Bankofamerica, Barclaycard, Billwerk, Bluesnap, Braintree, Calida, Cashfree, Cashtocode, Celero, Checkout, Cryptopay, Cybersource, Datatrans, Dlocal, Elavon, Finix, Fiserv, Fiservcommercehub, Fiservemea, Fiuu, Forte, Getnet, Gigadat, Globalpay, Helcim, Hipay, Hyperpg, Iatapay, Itaubank, Jpmorgan, Loonio, Mifinity, Mollie, Multisafepay, Nexinets, Nexixpay, Nmi, Noon, Novalnet, Nuvei, Paybox, Payload, Payme, Paypal, Paysafe, Paytm, Payu, Peachpayments, Phonepe, Placetopay, Powertranz, Ppro, Rapyd, Razorpay, RazorpayV2, Redsys, Revolut, Revolv3, Shift4, Silverflow, Stax, Trustpay, Trustpayments, Truelayer, Tsys, Volt, Wellsfargo, Worldpay, Worldpayxml, Xendit, Zift.

(Each row is verified by grepping `connector_types::ClientAuthentication` under `crates/integrations/connector-integration/src/connectors/` at the pinned SHA.)

**Refresh note (1.1.0, pinned SHA `60540470c`)**: The list above is preserved verbatim from pattern version 1.0.0 for historical continuity. As of this refresh, the following connectors have graduated from stub to full implementation and should be treated as fully implemented, not as stubs: `Bluesnap` (PR #959, trait blanket-impl at `crates/integrations/connector-integration/src/connectors/bluesnap.rs:100-103` is still one-line but the connector now provides a hand-written `ConnectorIntegrationV2<ClientAuthenticationToken, ...>` impl at `crates/integrations/connector-integration/src/connectors/bluesnap.rs:684-802`), `Globalpay` (PR #957, macro block at `crates/integrations/connector-integration/src/connectors/globalpay.rs:657-694`), and `Jpmorgan` (PR #966, macro block at `crates/integrations/connector-integration/src/connectors/jpmorgan.rs:546-618`). See the full-implementation table above for details. Readers authoring a new stub must not copy these three as "stub examples".

### Shared-types consolidation (PR #1002)

PR #1002 (commit `03e9fab77`, "feat(shared): consolidate ClientAuthenticationToken shared types for all connectors") centralised every per-connector SDK-init response struct into a single file so that adding a new connector is a one-arm addition to `ConnectorSpecificClientAuthenticationResponse` rather than a parallel-type proliferation. Before #1002 the discriminator enum held only `Stripe` and `Globalpay`; after #1002 it holds 17 arms (Stripe, Adyen, Checkout, Cybersource, Nuvei, Mollie, Globalpay, Bluesnap, Rapyd, Shift4, BankOfAmerica, Wellsfargo, Fiserv, Elavon, Noon, Paysafe, Bamboraapac, Jpmorgan, Billwerk). A follow-on PR #1023 (commit `0af11797d`, "consolidate ClientAuthenticationToken shared types for batch 2 connectors") extended the enum further to include Datatrans, Bambora, Payload, Multisafepay, Nexinets, and Nexixpay; these arms are visible at the pinned SHA in the same file.

**Where the consolidated types live** (all in `crates/types-traits/domain_types/src/connector_types.rs` at the pinned SHA):

| Item | Citation | What it replaced |
|---|---|---|
| `ConnectorSpecificClientAuthenticationResponse` discriminator enum (expanded to 25 arms including the three new connectors) | `crates/types-traits/domain_types/src/connector_types.rs:3429-3480` | Pre-#1002 two-arm enum (`Stripe`, `Globalpay`) that would have required a separate per-connector domain struct in each connector crate. |
| `AdyenClientAuthenticationResponse { session_id, session_data }` | `crates/types-traits/domain_types/src/connector_types.rs:3489-3495` | Previously a connector-local type inside `adyen/transformers.rs`. |
| `CheckoutClientAuthenticationResponse { payment_session_id, payment_session_token, payment_session_secret }` | `crates/types-traits/domain_types/src/connector_types.rs:3498-3506` | Previously a connector-local type inside `checkout/transformers.rs`. |
| `CybersourceClientAuthenticationResponse { capture_context, client_library, client_library_integrity }` | `crates/types-traits/domain_types/src/connector_types.rs:3509-3517` | Previously a connector-local `CybersourceClientAuthResponse` type. |
| `NuveiClientAuthenticationResponse { session_token }` | `crates/types-traits/domain_types/src/connector_types.rs:3520-3524` | Previously a connector-local type. |
| `MollieClientAuthenticationResponse { payment_id, checkout_url }` | `crates/types-traits/domain_types/src/connector_types.rs:3527-3533` | Previously a connector-local type. |
| `GlobalpayClientAuthenticationResponse { access_token, token_type, expires_in }` (already present pre-#1002, kept unchanged) | `crates/types-traits/domain_types/src/connector_types.rs:3536-3544` | No replacement — this was the template the other connectors were rewritten to match. |
| `BluesnapClientAuthenticationResponse { pf_token }` | `crates/types-traits/domain_types/src/connector_types.rs:3547-3551` | Previously a connector-local type inside `bluesnap/transformers.rs`. |
| `RapydClientAuthenticationResponse { checkout_id, redirect_url }` | `crates/types-traits/domain_types/src/connector_types.rs:3554-3560` | Previously a connector-local type. |
| `Shift4ClientAuthenticationResponse { client_secret }` | `crates/types-traits/domain_types/src/connector_types.rs:3563-3567` | Previously a connector-local type. |
| `BankOfAmericaClientAuthenticationResponse { capture_context }` | `crates/types-traits/domain_types/src/connector_types.rs:3570-3574` | Previously a connector-local type. |
| `WellsfargoClientAuthenticationResponse { capture_context }` | `crates/types-traits/domain_types/src/connector_types.rs:3577-3581` | Previously a connector-local type. |
| `FiservClientAuthenticationResponse { session_id }` | `crates/types-traits/domain_types/src/connector_types.rs:3584-3588` | Previously a connector-local type. |
| `ElavonClientAuthenticationResponse { session_token }` | `crates/types-traits/domain_types/src/connector_types.rs:3591-3595` | Previously a connector-local type. |
| `NoonClientAuthenticationResponse { order_id, checkout_url }` | `crates/types-traits/domain_types/src/connector_types.rs:3598-3604` | Previously a connector-local type. |
| `PaysafeClientAuthenticationResponse { payment_handle_token }` | `crates/types-traits/domain_types/src/connector_types.rs:3607-3611` | Previously a connector-local type. |
| `BamboraapacClientAuthenticationResponse { token }` | `crates/types-traits/domain_types/src/connector_types.rs:3614-3618` | Previously a connector-local type. |
| `JpmorganClientAuthenticationResponse { transaction_id, request_id }` | `crates/types-traits/domain_types/src/connector_types.rs:3621-3627` | Previously a connector-local `JpmorganClientAuthResponseDomain`-style type. |
| `BillwerkClientAuthenticationResponse { session_id }` | `crates/types-traits/domain_types/src/connector_types.rs:3630-3634` | Previously a connector-local type. |

Additional downstream consolidation (same PR #1002): the `From<...>` → `grpc_api_types::payments::ConnectorSpecificClientAuthenticationResponse` converter now lives centrally at `crates/types-traits/domain_types/src/types.rs:10645-10929` (approx., arms per connector), replacing what would otherwise be per-connector `types.rs` impls.

**What this means for a new connector**: to add a ClientAuthenticationToken implementation today, the author

1. Adds one arm to `ConnectorSpecificClientAuthenticationResponse` at `crates/types-traits/domain_types/src/connector_types.rs:3429` (append only, alphabetical by connector name is preferred but not enforced by the compiler).
2. Defines the sibling `<ConnectorName>ClientAuthenticationResponse` struct immediately below the enum (same file), mirroring the patterns above.
3. Adds one arm to the gRPC conversion `match` at `crates/types-traits/domain_types/src/types.rs:10645+`.
4. Wires the connector-local transformer to wrap the extracted token in `ClientAuthenticationTokenData::ConnectorSpecific(Box::new(ConnectorSpecificClientAuthenticationResponse::<ConnectorName>(<ConnectorName>ClientAuthenticationResponse { ... })))`.

Authors MUST NOT re-introduce a parallel connector-local `<ConnectorName>ClientAuthResponseDomain` type — the pre-#1002 pattern — unless the new connector's response has fields that genuinely cannot be represented by any of the existing structs; in that case, extending the shared enum in `connector_types.rs` is the correct change.

## Common Implementation Patterns

The recommended path at this SHA is the macro-based pattern used by Stripe. The flow is stateless from UCS's perspective (no access-token reuse, no caching), so the entire wiring is exactly two macro blocks plus two `TryFrom` impls.

### Macro wiring (recommended)

Inside the connector's single `macros::create_all_prerequisites!` invocation, add a tuple for the flow. Excerpted from Stripe:

```rust
// From crates/integrations/connector-integration/src/connectors/stripe.rs:300-305
(
    flow: ClientAuthenticationToken,
    request_body: StripeClientAuthRequest,
    response_body: StripeClientAuthResponse,
    router_data: RouterDataV2<ClientAuthenticationToken, PaymentFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
)
```

Then, for the concrete integration, a second macro block declares URL/headers/HTTP method:

```rust
// From crates/integrations/connector-integration/src/connectors/stripe.rs:993-1023
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Stripe,
    curl_request: FormUrlEncoded(StripeClientAuthRequest),
    curl_response: StripeClientAuthResponse,
    flow_name: ClientAuthenticationToken,
    resource_common_data: PaymentFlowData,
    flow_request: ClientAuthenticationTokenRequestData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<ClientAuthenticationToken, PaymentFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<ClientAuthenticationToken, PaymentFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}{}",
                self.connector_base_url_payments(req),
                "v1/payment_intents"
            ))
        }
    }
);
```

The `connector_types::ClientAuthentication` trait itself is then a one-liner:

```rust
// From crates/integrations/connector-integration/src/connectors/stripe.rs:72-75
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Stripe<T>
{
}
```

### Request transformation (recommended)

Implement `TryFrom<StripeRouterData<RouterDataV2<ClientAuthenticationToken, PaymentFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>, T>>` for the connector-local request struct. The Stripe example at `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:5359-5407` converts the minor unit amount via the connector's amount converter, lowercases the currency, copies the reference id into Stripe's `metadata[...]` bag, and turns on `automatic_payment_methods[enabled]` so Stripe picks the method at SDK time.

### Response transformation (recommended)

Implement `TryFrom<ResponseRouterData<ConnectorResponse, Self>> for RouterDataV2<ClientAuthenticationToken, PaymentFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>`. Wrap the connector's token in `ClientAuthenticationTokenData::ConnectorSpecific(...)` unless your connector already maps cleanly onto the Google Pay / Apple Pay / PayPal canonical enum arms.

### Alternate: pre-existing canonical wallet response

If the connector returns a Google Pay session payload, produce `ClientAuthenticationTokenData::GooglePay(...)` directly instead of going through `ConnectorSpecific`. Same for PayPal (`Paypal(...)`) and Apple Pay (`ApplePay(...)`). This preserves structured `serde` tagging for downstream UCS consumers. See `crates/types-traits/domain_types/src/connector_types.rs:3417-3426` for the canonical arms.

### Stub pattern (when a connector does not support the flow)

A bare impl without a macro expansion satisfies the `ConnectorServiceTrait` bound but does NOT register the flow. Do not add `ClientAuthenticationToken` to the connector's `create_all_prerequisites!` tuple list when stubbing.

```rust
// Pattern seen on most connectors at this SHA
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for MyConnector<T>
{
}
```

## Connector-Specific Patterns

### Stripe

- **Endpoint**: `POST {base_url}v1/payment_intents` — `crates/integrations/connector-integration/src/connectors/stripe.rs:1015-1021`.
- **Wire format**: `application/x-www-form-urlencoded` (Stripe API requires form encoding, not JSON). Enforced at the macro layer via `curl_request: FormUrlEncoded(StripeClientAuthRequest)` at `crates/integrations/connector-integration/src/connectors/stripe.rs:996`.
- **Request shape**: `StripeClientAuthRequest` at `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:5349-5357` — fields `amount`, `currency`, `automatic_payment_methods[enabled]` (always `Some(true)` in this flow), and a flattened `meta_data: HashMap<String, String>` populated with the order reference. Notably, `confirm` is deliberately omitted so the PaymentIntent is created in the "requires_confirmation" state; confirmation happens client-side via `stripe.confirmPayment()`.
- **Authentication**: `Bearer {api_key}` header. Stripe accepts either a live or test `sk_...` key; see `ConnectorAuthType::HeaderKey` at `crates/types-traits/domain_types/src/router_data.rs:22-24`. The `build_headers` helper is shared across all Stripe flows at `crates/integrations/connector-integration/src/connectors/stripe.rs:309-320`.
- **Response shape**: `StripeClientAuthResponse` wraps Stripe's `PaymentIntentResponse` — `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:5409-5411`. The only field read is `client_secret`.
- **Mapping**: The extracted `client_secret` is wrapped as `ClientAuthenticationTokenData::ConnectorSpecific(Box::new(ConnectorSpecificClientAuthenticationResponse::Stripe(StripeClientAuthenticationResponseDomain { client_secret })))` — `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:5433-5437`.
- **Error mode**: If `response.client_secret` is `None`, the transformer returns `ConnectorResponseTransformationError::ResponseDeserializationFailed` — `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:5427-5431`. HTTP-level errors flow through the standard `build_error_response` shared with Stripe's other flows.

### Globalpay

- **Endpoint**: `POST {base_url}/accesstoken` — `crates/integrations/connector-integration/src/connectors/globalpay.rs:686-692`.
- **Wire format**: JSON. Enforced at the macro layer via `curl_request: Json(GlobalpayClientAuthRequest)` at `crates/integrations/connector-integration/src/connectors/globalpay.rs:661`.
- **Request shape**: `GlobalpayClientAuthRequest { app_id, nonce, secret, grant_type }` at `crates/integrations/connector-integration/src/connectors/globalpay/transformers.rs:1074-1080`. The `nonce` is a 12-character random alphanumeric string (`rand::distributions::Alphanumeric` at `crates/integrations/connector-integration/src/connectors/globalpay/transformers.rs:1116-1117`) and `secret = hex(SHA512(nonce || app_key))` — see `crates/integrations/connector-integration/src/connectors/globalpay/transformers.rs:1119-1126`. `grant_type` is hard-coded to `"client_credentials"` (`:1131`).
- **Authentication**: the `app_id` + `app_key` are both pulled from `ConnectorSpecificConfig::Globalpay` (`crates/integrations/connector-integration/src/connectors/globalpay/transformers.rs:1109-1112`). The only header added beyond the default content-type is `X-GP-Version: {API_VERSION}` (`crates/integrations/connector-integration/src/connectors/globalpay.rs:681-683`). No bearer token is sent because this call mints one.
- **Response shape**: `GlobalpayClientAuthResponse { token, type_, seconds_to_expire }` at `crates/integrations/connector-integration/src/connectors/globalpay/transformers.rs:1144-1150` — note the `type_` field uses `#[serde(rename = "type")]` since `type` is a Rust keyword.
- **Mapping**: the response is wrapped as `ClientAuthenticationTokenData::ConnectorSpecific(Box::new(ConnectorSpecificClientAuthenticationResponse::Globalpay(GlobalpayClientAuthenticationResponse { access_token, token_type: Some(type_), expires_in: Some(seconds_to_expire) })))` at `crates/integrations/connector-integration/src/connectors/globalpay/transformers.rs:1167-1175`. The shared `GlobalpayClientAuthenticationResponse` type lives at `crates/types-traits/domain_types/src/connector_types.rs:3536-3544`.
- **Relationship to ServerAuthenticationToken**: Globalpay's access-token endpoint is the same URL used for the server-side OAuth flow. The two flows differ only in (a) their response type so the data lands in `ClientAuthenticationTokenResponse` vs. `ServerAuthenticationTokenResponseData`, and (b) whether the resulting token is returned to the client or stored on `PaymentFlowData.access_token`.

### Bluesnap

- **Endpoint**: `POST {base_url}/services/2/payment-fields-tokens` — `crates/integrations/connector-integration/src/connectors/bluesnap.rs:720-723`.
- **Wire format**: JSON, with an **empty body** (`pub struct BluesnapClientAuthRequest {}` at `crates/integrations/connector-integration/src/connectors/bluesnap/transformers.rs:927-928`). Bluesnap's Hosted Payment Fields bootstrap does not require any payload.
- **Not macro-wired**: unlike Stripe/Globalpay/Jpmorgan, Bluesnap uses a hand-written `impl ConnectorIntegrationV2<ClientAuthenticationToken, PaymentFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData> for Bluesnap<T>` at `crates/integrations/connector-integration/src/connectors/bluesnap.rs:684-802`, because the returned `pfToken` lives in the **HTTP `Location` header** rather than the JSON body. Macro-tuple registration at `crates/integrations/connector-integration/src/connectors/bluesnap.rs:432-437` is present for type-list completeness but the `handle_response_v2` override on the hand-written impl is what actually extracts the token.
- **Location-header extraction**: `handle_response_v2` fetches `res.headers.get("location")`, then splits on `/` and takes the last segment as the pfToken (`crates/integrations/connector-integration/src/connectors/bluesnap.rs:746-774`). A missing header or an un-splittable URL yields `ConnectorError::ResponseDeserializationFailed` with a specific `additional_context` message.
- **Response shape (synthetic)**: the extracted string is packed into `BluesnapClientAuthResponse { pf_token: Some(Secret::new(...)) }` (`crates/integrations/connector-integration/src/connectors/bluesnap.rs:776-778`) and then passed through the standard `TryFrom<ResponseRouterData<BluesnapClientAuthResponse, Self>>` impl at `crates/integrations/connector-integration/src/connectors/bluesnap/transformers.rs:968-1011`, which unwraps the `Option` (erroring on `None`) and wraps the token into `ConnectorSpecificClientAuthenticationResponse::Bluesnap(BluesnapClientAuthenticationResponse { pf_token })`. The shared struct lives at `crates/types-traits/domain_types/src/connector_types.rs:3547-3551`.
- **Authentication**: Basic auth header computed by the shared `build_headers` helper (`crates/integrations/connector-integration/src/connectors/bluesnap.rs:700-710`).

### Jpmorgan

- **Endpoint**: `POST {secondary_base_url}/am/oauth2/alpha/access_token` — `crates/integrations/connector-integration/src/connectors/jpmorgan.rs:589-591`. Note the use of a **secondary** base URL (`jpmorgan.secondary_base_url`) distinct from the payments base URL; if unset, `FailedToObtainIntegrationUrl` is returned with a documentation pointer (`crates/integrations/connector-integration/src/connectors/jpmorgan.rs:592-611`).
- **Wire format**: `application/x-www-form-urlencoded` — enforced via `curl_request: FormUrlEncoded(JpmorganClientAuthRequest)` at `crates/integrations/connector-integration/src/connectors/jpmorgan.rs:549` and a hard-coded content-type header at `crates/integrations/connector-integration/src/connectors/jpmorgan.rs:559-561, 574-577`.
- **Request shape**: `JpmorganClientAuthRequest { grant_type, scope }` at `crates/integrations/connector-integration/src/connectors/jpmorgan/requests.rs:9-13`. Scope is `"jpm:payments:sandbox"` when `resource_common_data.test_mode` is `Some(true)` or `None`, and `"jpm:payments"` otherwise (`crates/integrations/connector-integration/src/connectors/jpmorgan/transformers.rs:941-950`). `grant_type` is hard-coded to `"client_credentials"` (`:952`).
- **Authentication**: **Basic auth** over `client_id:client_secret`, base64-encoded — built inline in the `get_headers` override at `crates/integrations/connector-integration/src/connectors/jpmorgan.rs:567-582`. This is different from Jpmorgan's payment endpoints which use a Bearer token previously obtained via `ServerAuthenticationToken`.
- **Response shape**: `JpmorganClientAuthResponse { access_token, scope, token_type, expires_in }` at `crates/integrations/connector-integration/src/connectors/jpmorgan/responses.rs:164-170`. This is the raw OAuth2 token response from JPMC.
- **Mapping**: `ClientAuthenticationTokenData::ConnectorSpecific(Box::new(ConnectorSpecificClientAuthenticationResponse::Jpmorgan(JpmorganClientAuthenticationResponse { transaction_id: access_token, request_id: token_type })))` at `crates/integrations/connector-integration/src/connectors/jpmorgan/transformers.rs:972-979`. Note that the `transaction_id` field of the shared struct is reused here to carry the OAuth `access_token` — this is a deliberate shape choice in the shared type (`crates/types-traits/domain_types/src/connector_types.rs:3621-3627`) and not a mis-mapping; the shared struct is named for Jpmorgan's dominant per-flow convention (`transaction_id` / `request_id`).
- **Dual endpoint with ServerAuthenticationToken**: Jpmorgan's client-auth call reuses the same OAuth2 token endpoint as the server-auth flow; they differ in which `RouterDataV2` flow marker is used and where the token ends up. Unlike Globalpay, no distinct request field is needed — the endpoint accepts the same client-credentials grant either way.

## Code Examples

### 1. Flow marker (`connector_flow.rs`)

```rust
// From crates/types-traits/domain_types/src/connector_flow.rs:61-62
#[derive(Debug, Clone)]
pub struct ClientAuthenticationToken;
```

### 2. Request data (`connector_types.rs`)

```rust
// From crates/types-traits/domain_types/src/connector_types.rs:1606-1618
#[derive(Debug, Clone)]
pub struct ClientAuthenticationTokenRequestData {
    pub amount: MinorUnit,
    pub currency: Currency,
    pub country: Option<common_enums::CountryAlpha2>,
    pub order_details: Option<Vec<payment_address::OrderDetailsWithAmount>>,
    pub email: Option<Email>,
    pub customer_name: Option<Secret<String>>,
    pub order_tax_amount: Option<MinorUnit>,
    pub shipping_cost: Option<MinorUnit>,
    pub payment_method_type: Option<PaymentMethodType>,
}
```

### 3. Response envelope (`connector_types.rs`)

```rust
// From crates/types-traits/domain_types/src/connector_types.rs:1404-1407
ClientAuthenticationTokenResponse {
    session_data: ClientAuthenticationTokenData,
    status_code: u16,
},
```

### 4. Session-data enum + Stripe arm (`connector_types.rs`)

```rust
// From crates/types-traits/domain_types/src/connector_types.rs:3414-3441
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "sdk_type")]
#[serde(rename_all = "snake_case")]
pub enum ClientAuthenticationTokenData {
    GooglePay(Box<GpayClientAuthenticationResponse>),
    Paypal(Box<PaypalClientAuthenticationResponse>),
    ApplePay(Box<ApplepayClientAuthenticationResponse>),
    ConnectorSpecific(Box<ConnectorSpecificClientAuthenticationResponse>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "connector")]
#[serde(rename_all = "snake_case")]
pub enum ConnectorSpecificClientAuthenticationResponse {
    Stripe(StripeClientAuthenticationResponse),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StripeClientAuthenticationResponse {
    pub client_secret: Secret<String>,
}
```

### 5. Trait definition (`interfaces/src/connector_types.rs`)

```rust
// From crates/types-traits/interfaces/src/connector_types.rs:174-182
pub trait ClientAuthentication:
    ConnectorIntegrationV2<
    connector_flow::ClientAuthenticationToken,
    PaymentFlowData,
    ClientAuthenticationTokenRequestData,
    PaymentsResponseData,
>
{
}
```

### 6. Stripe request TryFrom (`stripe/transformers.rs`)

```rust
// From crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:5343-5407
// ---- ClientAuthenticationToken flow types ----

/// Creates an unconfirmed PaymentIntent. `confirm` is intentionally omitted —
/// confirmation happens browser-side via `stripe.confirmPayment()` using the
/// returned `client_secret`.
#[serde_with::skip_serializing_none]
#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct StripeClientAuthRequest {
    pub amount: MinorUnit,
    pub currency: String,
    #[serde(rename = "automatic_payment_methods[enabled]")]
    pub automatic_payment_methods_enabled: Option<bool>,
    #[serde(flatten)]
    pub meta_data: HashMap<String, String>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        StripeRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                PaymentFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for StripeClientAuthRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: StripeRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                PaymentFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;

        let amount = StripeAmountConvertor::convert(
            router_data.request.amount,
            router_data.request.currency,
        )?;

        let currency = router_data.request.currency.to_string().to_lowercase();

        let order_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        let meta_data = get_transaction_metadata(None, order_id);

        Ok(Self {
            amount,
            currency,
            automatic_payment_methods_enabled: Some(true),
            meta_data,
        })
    }
}
```

### 7. Stripe response TryFrom (`stripe/transformers.rs`)

```rust
// From crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:5409-5447
/// Wraps PaymentIntentResponse for the ClientAuthenticationToken flow.
#[derive(Debug, Deserialize, Serialize)]
pub struct StripeClientAuthResponse(PaymentIntentResponse);

impl TryFrom<ResponseRouterData<StripeClientAuthResponse, Self>>
    for RouterDataV2<
        ClientAuthenticationToken,
        PaymentFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;
    fn try_from(
        item: ResponseRouterData<StripeClientAuthResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response.0;

        let client_secret = response.client_secret.ok_or(
            ConnectorResponseTransformationError::ResponseDeserializationFailed {
                context: Default::default(),
            },
        )?;

        let session_data = ClientAuthenticationTokenData::ConnectorSpecific(Box::new(
            ConnectorSpecificClientAuthenticationResponse::Stripe(
                StripeClientAuthenticationResponseDomain { client_secret },
            ),
        ));

        Ok(Self {
            response: Ok(PaymentsResponseData::ClientAuthenticationTokenResponse {
                session_data,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}
```

### 8. Stripe connector-file macro block (excerpt)

```rust
// From crates/integrations/connector-integration/src/connectors/stripe.rs:300-305
(
    flow: ClientAuthenticationToken,
    request_body: StripeClientAuthRequest,
    response_body: StripeClientAuthResponse,
    router_data: RouterDataV2<ClientAuthenticationToken, PaymentFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
)
```

## Integration Guidelines

1. Confirm your connector actually issues a client-bound credential (browser/mobile SDK consumes it). If the token is server-only, use `ServerAuthenticationToken` (OAuth bearer) or `ServerSessionAuthenticationToken` (wallet session) instead — see the relationship table above.
2. In `<connector>.rs`, add `ClientAuthenticationToken` to the `connector_flow` import list and `ClientAuthenticationTokenRequestData` to the `connector_types` import list, mirroring Stripe at `crates/integrations/connector-integration/src/connectors/stripe.rs:14-34`.
3. Add a tuple to the connector's existing `macros::create_all_prerequisites!` block with `flow: ClientAuthenticationToken, request_body: <ConnectorName>ClientAuthRequest, response_body: <ConnectorName>ClientAuthResponse, router_data: RouterDataV2<ClientAuthenticationToken, PaymentFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>`. Model on `stripe.rs:300-305`.
4. Add a `macros::macro_connector_implementation!` block with `flow_name: ClientAuthenticationToken`, `resource_common_data: PaymentFlowData`, `flow_request: ClientAuthenticationTokenRequestData`, `flow_response: PaymentsResponseData`, `http_method: Post`, and the appropriate `curl_request:`/`curl_response:` variants. Model on `stripe.rs:993-1023`.
5. In `<connector>/transformers.rs`, define `<ConnectorName>ClientAuthRequest` and `<ConnectorName>ClientAuthResponse` and implement `TryFrom` impls analogous to `stripe/transformers.rs:5343-5447`. Pull `amount` and `currency` out of the request data; do NOT hard-code them.
6. Map the connector's opaque token into `ClientAuthenticationTokenData`. If the token is a canonical Google Pay / Apple Pay / PayPal session, use the matching arm directly; otherwise extend `ConnectorSpecificClientAuthenticationResponse` in `crates/types-traits/domain_types/src/connector_types.rs:3430-3435` with a new connector arm and produce `ClientAuthenticationTokenData::ConnectorSpecific(...)`. Adding a new arm requires updating the enum definition in a dedicated PR.
7. Return `PaymentsResponseData::ClientAuthenticationTokenResponse { session_data, status_code: item.http_code }` from the response transformer — nothing else. Do NOT populate `PaymentFlowData.session_token` (that field is for `ServerSessionAuthenticationToken`).
8. Blanket-impl `connector_types::ClientAuthentication for <ConnectorName><T>` once — one-line, empty body. The trait bound is structural; no methods to implement.
9. On the response transformer error branch, use `ConnectorResponseTransformationError::ResponseDeserializationFailed` when the connector returns HTTP 2xx but the expected token field is absent; use the connector's shared `build_error_response` for HTTP 4xx/5xx.
10. No ValidationTrait toggle is required for `ClientAuthenticationToken` at this SHA (unlike `ServerSessionAuthenticationToken`, which is still gated by `should_do_session_token()` — see `pattern_server_session_authentication_token.md`). The flow is invoked explicitly by the gRPC handler when the caller asks for a client token, so no server-side decision gate is needed.

## Best Practices

- **Use the macro path**, not manual `ConnectorIntegrationV2` impls. Stripe's full implementation is exactly two macro blocks plus two `TryFrom`s (`crates/integrations/connector-integration/src/connectors/stripe.rs:300-305`, `stripe.rs:993-1023`, `stripe/transformers.rs:5343-5447`). Manual impls invite drift.
- **Mask client secrets**: the Stripe response type stores `client_secret: Secret<String>` (`crates/types-traits/domain_types/src/connector_types.rs:3440`). Never log the unwrapped string. Any new `ConnectorSpecificClientAuthenticationResponse` arm MUST wrap its token in `Secret<_>`.
- **Do not confirm on the server**: if your connector's "create intent" endpoint accepts a `confirm` parameter, leave it out. Stripe's transformer comment at `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:5345-5347` is explicit about this invariant — confirmation is the client's job.
- **Carry the merchant reference in metadata**: use `router_data.resource_common_data.connector_request_reference_id` for idempotency/traceability. Stripe routes it via `get_transaction_metadata` at `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:5398`.
- **Amount conversion**: always go through the connector's macro-generated amount converter. Stripe uses `StripeAmountConvertor::convert(...)` at `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:5386-5389`. Hand-rolled amount arithmetic is banned by §12 of the spec.
- **Prefer existing canonical arms** (`GooglePay` / `Paypal` / `ApplePay`) over `ConnectorSpecific` whenever the connector already returns a wallet-standard payload — it reduces per-client deserialization work.
- **Cross-ref `pattern_server_session_authentication_token.md`** for the sibling `ServerSessionAuthenticationToken` flow (wallet sessions for Apple Pay / Google Pay / PayPal) and `pattern_server_authentication_token.md` for OAuth bearer acquisition. Those are NOT interchangeable with this flow.

## Common Errors / Gotchas

1. **Problem**: Using `CreateSessionToken` / `SessionTokenRequestData` / `PaymentSessionToken` / `SessionToken` identifiers.
   **Solution**: Those were retired by PR #855. Replace per the [retired identifiers](#retired--pre-rename-identifiers) table. The review rubric check #5 in `PATTERN_AUTHORING_SPEC.md:184` FAILs any pattern that references them outside a "retired — do not use" callout.

2. **Problem**: Using `SdkSessionToken` / `SdkSessionTokenV2` / `PaymentsSdkSessionTokenData` / `SdkSessionTokenResponse` identifiers.
   **Solution**: These were the pre-#855 names for this exact flow. Use `ClientAuthenticationToken` / `ClientAuthentication` / `ClientAuthenticationTokenRequestData` / `ClientAuthenticationTokenResponse` respectively.

3. **Problem**: Writing to `PaymentFlowData.session_token: Option<String>` from the response transformer.
   **Solution**: That field is for `ServerSessionAuthenticationToken` only. `ClientAuthenticationToken` returns its artifact via the typed `session_data: ClientAuthenticationTokenData` field of `PaymentsResponseData::ClientAuthenticationTokenResponse` — see `crates/types-traits/domain_types/src/connector_types.rs:1404-1407`. Populating `session_token: String` silently loses the type discrimination and breaks the frontend contract.

4. **Problem**: Returning a bare `String` or `serde_json::Value` as the token from a new connector.
   **Solution**: Always wrap it in `StripeClientAuthenticationResponse`-style typed struct inside `ConnectorSpecificClientAuthenticationResponse`. Extend `crates/types-traits/domain_types/src/connector_types.rs:3432` with a new arm in a dedicated PR if your connector is neither Stripe nor a canonical wallet.

5. **Problem**: Sending the request as JSON for Stripe.
   **Solution**: Stripe requires `application/x-www-form-urlencoded` on all `/v1/*` endpoints. The macro argument is `curl_request: FormUrlEncoded(StripeClientAuthRequest)` at `crates/integrations/connector-integration/src/connectors/stripe.rs:996`, not `Json(...)`. Mismatching content types yields a 415 / 400 from Stripe.

6. **Problem**: Calling `confirm=true` on the PaymentIntent to "finish" the flow server-side.
   **Solution**: Never. The whole point of `ClientAuthenticationToken` is that the client confirms. Stripe's request struct at `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:5349-5357` deliberately omits `confirm`.

7. **Problem**: Stubbing the flow by adding it to `create_all_prerequisites!` and `macro_connector_implementation!` with empty bodies, which then dispatches to a real endpoint.
   **Solution**: To stub, provide only the trait blanket-impl `connector_types::ClientAuthentication for <Connector><T> {}` and nothing else. Leave the macros out entirely. The default `ConnectorIntegrationV2` impl is already what you want.

8. **Problem**: Forgetting to blanket-impl `ClientAuthentication`, then hitting a trait-bound error on `ConnectorServiceTrait`.
   **Solution**: Every full connector needs the one-line blanket impl (stub or real), because `ClientAuthentication` is a supertrait of `ConnectorServiceTrait` at `crates/types-traits/interfaces/src/connector_types.rs:79`.

9. **Problem**: Mapping all HTTP 2xx to a success even when `client_secret` is absent.
   **Solution**: Stripe treats missing `client_secret` as `ConnectorResponseTransformationError::ResponseDeserializationFailed` at `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:5427-5431`. Mirror this pattern: a "200 with no token" is a parse failure, not a success.

10. **Problem**: Logging the `client_secret` via `Debug`.
    **Solution**: `Secret<String>` masks its `Debug` output automatically. Do not unwrap via `.expose()` / `.peek()` into log statements.

## Testing Notes

### Unit-test shape

Unit tests for the request transformer should verify that:
- `ClientAuthenticationTokenRequestData::amount` is converted through the connector's amount converter (no panic on boundary values).
- `currency` is lowercased (Stripe) or normalized per the connector contract.
- `connector_request_reference_id` ends up in whatever metadata/idempotency slot the connector provides.

Unit tests for the response transformer should cover:
- Success path: a response containing the token produces a `ClientAuthenticationTokenData::ConnectorSpecific(...)` (or the appropriate canonical arm).
- Missing-token path: a 2xx with the token field missing returns `ConnectorResponseTransformationError::ResponseDeserializationFailed` (mirroring `stripe/transformers.rs:5427-5431`).

### Integration-test scenarios

| Scenario | Expected HTTP | Expected `session_data` | Expected `PaymentsResponseData` variant |
|---|---|---|---|
| Valid merchant key + supported amount/currency | 200 | `ClientAuthenticationTokenData::ConnectorSpecific(... Stripe { client_secret })` (or equivalent for new connector) | `ClientAuthenticationTokenResponse { session_data, status_code: 200 }` |
| Invalid merchant key | 401/403 | — (response is `Err(ErrorResponse { .. })`) | N/A — `response: Err(..)` |
| Unsupported currency | 4xx | — | N/A — `response: Err(..)` |
| Connector 2xx with no token field | 200 | — (transformer returns `ResponseDeserializationFailed`) | N/A — transformer Err |
| Amount zero / below minimum | 4xx | — | N/A — `response: Err(..)` |

Real sandboxes MUST be used. Per §11 anti-pattern #2 of `PATTERN_AUTHORING_SPEC.md`, mocking the connector HTTP layer inside a "integration test" is banned; mock only in pure unit tests of the transformers.

## Retired / pre-rename identifiers

The following names were renamed or replaced by PR #855 (commit `c9e1025e3`) and MUST NOT appear in any new pattern or connector code at the pinned SHA. This enumeration is drawn from the actual #855 diff on `crates/types-traits/domain_types/src/connector_flow.rs`, `crates/types-traits/domain_types/src/connector_types.rs`, and `crates/types-traits/interfaces/src/connector_types.rs`.

| Retired identifier | Kind | Replacement at pinned SHA | Replacement citation |
|---|---|---|---|
| `SdkSessionToken` | flow marker struct | `ClientAuthenticationToken` | `crates/types-traits/domain_types/src/connector_flow.rs:62` |
| `SdkSessionTokenV2` | trait | `ClientAuthentication` | `crates/types-traits/interfaces/src/connector_types.rs:174` |
| `PaymentsSdkSessionTokenData` | request-data struct | `ClientAuthenticationTokenRequestData` | `crates/types-traits/domain_types/src/connector_types.rs:1607` |
| `SdkSessionTokenResponse` (variant of `PaymentsResponseData`) | response-data variant | `ClientAuthenticationTokenResponse { session_data, status_code }` | `crates/types-traits/domain_types/src/connector_types.rs:1404` |
| `SessionToken` (enum, the sdk-data payload) | payload enum | `ClientAuthenticationTokenData` | `crates/types-traits/domain_types/src/connector_types.rs:3417` |
| `GpaySessionTokenResponse` | Google Pay payload struct | `GpayClientAuthenticationResponse` | `crates/types-traits/domain_types/src/connector_types.rs:3445` |
| `PaypalSessionTokenResponse` | PayPal payload struct | `PaypalClientAuthenticationResponse` | `crates/types-traits/domain_types/src/connector_types.rs:3712` |
| `ApplepaySessionTokenResponse` | Apple Pay payload struct | `ApplepayClientAuthenticationResponse` | `crates/types-traits/domain_types/src/connector_types.rs:3584` |
| `CreateSessionToken` | flow marker struct | `ServerSessionAuthenticationToken` | `crates/types-traits/domain_types/src/connector_flow.rs:38` |
| `PaymentSessionToken` | trait | `ServerSessionAuthentication` | `crates/types-traits/interfaces/src/connector_types.rs:164` |
| `SessionTokenRequestData` | request-data struct | `ServerSessionAuthenticationTokenRequestData` | `crates/types-traits/domain_types/src/connector_types.rs:1688` |
| `SessionTokenResponseData` | response-data struct | `ServerSessionAuthenticationTokenResponseData` | `crates/types-traits/domain_types/src/connector_types.rs:1703` |
| `CreateAccessToken` | flow marker struct | `ServerAuthenticationToken` | `crates/types-traits/domain_types/src/connector_flow.rs:41` |
| `PaymentAccessToken` | trait | `ServerAuthentication` | `crates/types-traits/interfaces/src/connector_types.rs:184` |
| `AccessTokenRequestData` | request-data struct | `ServerAuthenticationTokenRequestData` | `crates/types-traits/domain_types/src/connector_types.rs:1708` |
| `AccessTokenResponseData` | response-data struct | `ServerAuthenticationTokenResponseData` | `crates/types-traits/domain_types/src/connector_types.rs:1713` |

Additionally, the following identifiers from `PATTERN_AUTHORING_SPEC.md` §12 "Retired types" remain prohibited for any new pattern:

- `ConnectorError` (monolithic, pre-PR-#765) → use `IntegrationError` (request-time) or `ConnectorResponseTransformationError` (response-time).
- `RouterData` (V1) → use `RouterDataV2<...>`.
- `api::ConnectorIntegration` (V1 trait) → use `interfaces::connector_integration_v2::ConnectorIntegrationV2`.
- Hand-rolled amount conversion helpers → use macro-generated `<ConnectorName>AmountConvertor` via `common_utils::types` (`MinorUnit`, `StringMinorUnit`, etc.).

If you see any of the above in an older pattern file (pre-rename filenames were `pattern_session_token.md` and `pattern_CreateAccessToken_flow.md`, since renamed per PR #855 absorption), treat it as pre-rename prose and translate before copying.

## Cross-References

- Parent index: [README.md](./README.md)
- Sibling token flow — OAuth server-to-server: [pattern_server_authentication_token.md](./pattern_server_authentication_token.md) (renamed from `pattern_CreateAccessToken_flow.md` in PR #855 absorption; documents `ServerAuthenticationToken` + `ServerAuthentication`).
- Sibling token flow — wallet session bootstrap: [pattern_server_session_authentication_token.md](./pattern_server_session_authentication_token.md) (renamed from `pattern_session_token.md` in PR #855 absorption; documents `ServerSessionAuthenticationToken` + `ServerSessionAuthentication`).
- Sibling flow — Authorize (the call a client SDK performs after receiving the token): [pattern_authorize.md](./pattern_authorize.md).
- Sibling flow — Capture: [pattern_capture.md](./pattern_capture.md) (gold reference for section order).
- Authoring spec (must-read before edits): [PATTERN_AUTHORING_SPEC.md](./PATTERN_AUTHORING_SPEC.md).
- Macro reference (for `create_all_prerequisites!` and `macro_connector_implementation!`): [macro_patterns_reference.md](./macro_patterns_reference.md).
- Flow-macro implementation guide: [flow_macro_guide.md](./flow_macro_guide.md).

## Change Log

| Version | Generated | Pinned SHA | Changes |
|---|---|---|---|
| 1.0.0 | 2026-04-20 | `ceb33736c` | Initial flow pattern capturing the PR #855 rename (`SdkSessionToken` → `ClientAuthenticationToken`) and Stripe as the sole full implementation. All other connectors listed as stubs. |
| 1.1.0 | 2026-04-20 | `60540470c` | Absorbed three new full implementations merged after 1.0.0: Globalpay (PR #957 / commit `dd456e9ae`, `POST {base_url}/accesstoken` with nonce+SHA512 secret derivation, `crates/integrations/connector-integration/src/connectors/globalpay.rs:657-694`), Bluesnap (PR #959 / commit `0b1e7958a`, hand-written non-macro `ConnectorIntegrationV2` impl that extracts the pfToken from the HTTP `Location` header, `crates/integrations/connector-integration/src/connectors/bluesnap.rs:684-802`), and Jpmorgan (PR #966 / commit `c231dcd78`, OAuth2 token endpoint on `secondary_base_url` using Basic auth of `client_id:client_secret`, `crates/integrations/connector-integration/src/connectors/jpmorgan.rs:546-618`). Also added the new "Shared-types consolidation (PR #1002)" subsection documenting the move of per-connector SDK-init response structs into the shared `ConnectorSpecificClientAuthenticationResponse` enum at `crates/types-traits/domain_types/src/connector_types.rs:3429-3480` (commit `03e9fab77`) and the downstream gRPC conversion at `crates/types-traits/domain_types/src/types.rs:10645+`. Stub-implementation roll-call preserved verbatim from 1.0.0 with a "Refresh note" callout listing the graduated connectors. |
