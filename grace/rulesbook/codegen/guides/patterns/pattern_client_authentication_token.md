# ClientAuthenticationToken Flow Pattern

> **Auth mechanism: C — MERCHANT / CREDENTIAL AUTHENTICATION. This is NOT 3DS.**
>
> UCS has three separate authentication mechanisms. Conflating them is the single largest
> codegen risk in this corpus:
>
> | # | Mechanism | Flow markers | `resource_common_data` | gRPC service |
> |---|---|---|---|---|
> | A | Standalone 3DS trio | `PreAuthenticate` / `Authenticate` / `PostAuthenticate` | `PaymentFlowData` | `PaymentMethodAuthenticationService` |
> | B | In-payment 3DS | none — folded into `Authorize` | `PaymentFlowData` | `PaymentService.Authorize` |
> | **C** | **Merchant / credential auth — THIS FILE** | `ServerAuthenticationToken` / `ServerSessionAuthenticationToken` / `ClientAuthenticationToken` | **`MerchantAuthenticationFlowData`** | `MerchantAuthenticationService` |
>
> Every mechanism-C flow binds **`MerchantAuthenticationFlowData`**, never `PaymentFlowData`.
> `MerchantAuthenticationFlowData` lives in `crates/types-traits/domain_types/src/merchant_authentication_flow_data.rs`
> and its own doc-comment says why: *"This type deliberately omits payment-specific fields
> (`payment_id`, `attempt_id`, `status`, `payment_method`, `address`, `amount`, etc.) because
> merchant-authentication flows have no payment identity."*
>
> Verify before copying anything below:
> ```bash
> rg -n "pub trait (ServerAuthentication|ServerSessionAuthentication|ClientAuthentication):" -A 9 \
>    crates/types-traits/interfaces/src/connector_types.rs
> ```
>
> External 3DS providers (Netcetera, 3dsecure.io, GPayments, Cardinal, CTP) run entirely inside the
> Hyperswitch router and never reach UCS. Do not generate UCS flows for that class.
> `crates/integrations/connector-integration/src/authenticator_connectors/plaid.rs` is bank-account
> linking, not 3DS — it is a mechanism-C connector (see `pattern_client_authentication_token.md`).

## Overview

The `ClientAuthenticationToken` flow produces a short-lived, client-safe token (a "client secret", "session data", or SDK-init payload) that a frontend can hand to a connector's browser/mobile SDK to complete the remainder of a payment from the device. The flow is run server-side by UCS before any confirm/authorize step: the connector returns an opaque artifact (for Stripe, a PaymentIntent `client_secret`) and UCS forwards that to the client without exposing the merchant's API keys. Unlike `ServerSessionAuthenticationToken` (which returns a plain `session_token: String` that the *next server-side Authorize* consumes) and `ServerAuthenticationToken` (an OAuth bearer for subsequent server-to-server calls), `ClientAuthenticationToken` is expressly *client-bound credential issuance for a single checkout context* — and it is the only one of the three whose artifact is not folded back onto the next payment request. All three, however, are mechanism C: they bind `MerchantAuthenticationFlowData` and are served by `MerchantAuthenticationService`.

This pattern was introduced by PR #855 with Stripe as the reference implementation; **20 connectors register the flow at HEAD** (19 payment connectors plus the `plaid` authenticator connector — see [Connectors with Full Implementation](#connectors-with-full-implementation), and re-derive the list rather than trusting any printed roster). PR #1002 then consolidated all per-connector SDK-init response shapes into the shared `ConnectorSpecificClientAuthenticationResponse` enum so each new connector adds one arm rather than creating a parallel type.

Key Components — every reference below is a **symbol anchor**; grep the symbol, never a line number:

| Symbol | File |
|---|---|
| `pub struct ClientAuthenticationToken;` (flow marker) | `crates/types-traits/domain_types/src/connector_flow.rs` |
| `FlowName::ClientAuthenticationToken` (enum entry) | `crates/types-traits/domain_types/src/connector_flow.rs` |
| `pub struct ClientAuthenticationTokenRequestData` (request, 13 fields) | `crates/types-traits/domain_types/src/connector_types.rs` |
| `PaymentsResponseData::ClientAuthenticationTokenResponse { session_data, status_code }` (response variant) | `crates/types-traits/domain_types/src/connector_types.rs` |
| `pub enum ClientAuthenticationTokenData` (session payload, 5 arms incl. `Plaid`) | `crates/types-traits/domain_types/src/connector_types.rs` |
| `pub enum ConnectorSpecificClientAuthenticationResponse` (per-connector discriminator, 26 arms) | `crates/types-traits/domain_types/src/connector_types.rs` |
| `pub struct StripeClientAuthenticationResponse { client_secret: Secret<String> }` | `crates/types-traits/domain_types/src/connector_types.rs` |
| `pub struct PlaidClientAuthenticationResponse { link_token, expires_in_seconds, hosted_link_url }` | `crates/types-traits/domain_types/src/connector_types.rs` |
| `pub trait ClientAuthentication` | `crates/types-traits/interfaces/src/connector_types.rs` |
| `pub struct MerchantAuthenticationFlowData` (resource_common_data) | `crates/types-traits/domain_types/src/merchant_authentication_flow_data.rs` |
| `service MerchantAuthenticationService` → `rpc CreateClientAuthenticationToken` | `crates/types-traits/grpc-api-types/proto/services.proto` |
| Reference payment connector | `crates/integrations/connector-integration/src/connectors/stripe.rs` (+ `stripe/transformers.rs`) |
| Reference authenticator connector | `crates/integrations/connector-integration/src/authenticator_connectors/plaid.rs` (+ `plaid/transformers.rs`) |

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Relationship to other token flows](#relationship-to-other-token-flows)
3. [Connectors with Full Implementation](#connectors-with-full-implementation)
   - [Shared-types consolidation (PR #1002)](#shared-types-consolidation-pr-1002)
4. [Common Implementation Patterns](#common-implementation-patterns)
5. [Connector-Specific Patterns](#connector-specific-patterns)
   - [Plaid — the first-class-arm exception (authenticator connector)](#plaid--the-first-class-arm-exception-authenticator-connector)
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
MerchantAuthenticationFlowData (resource_common_data — mechanism C, NOT PaymentFlowData)
└── ClientAuthenticationToken (flow marker)
    └── request : ClientAuthenticationTokenRequestData
    └── response: PaymentsResponseData::ClientAuthenticationTokenResponse   ← ASYMMETRIC
            └── session_data : ClientAuthenticationTokenData
                    ├── GooglePay(Box<GpayClientAuthenticationResponse>)
                    ├── Paypal(Box<PaypalClientAuthenticationResponse>)
                    ├── ApplePay(Box<ApplepayClientAuthenticationResponse>)
                    ├── ConnectorSpecific(Box<ConnectorSpecificClientAuthenticationResponse>)
                    │       └── 26 per-connector arms, e.g. Stripe(StripeClientAuthenticationResponse { client_secret })
                    └── Plaid(Box<PlaidClientAuthenticationResponse>)       ← FIRST-CLASS arm, not ConnectorSpecific
```

The generic router-data template, per §7 of `PATTERN_AUTHORING_SPEC.md`, is:

```rust
RouterDataV2<
    ClientAuthenticationToken,              // flow marker
    MerchantAuthenticationFlowData,         // resource_common_data (mechanism C)
    ClientAuthenticationTokenRequestData,   // request
    PaymentsResponseData,                   // response (shared enum)
>
```

### Flow Type

`ClientAuthenticationToken` — defined at `crates/types-traits/domain_types/src/connector_flow.rs`:

```rust
// From crates/types-traits/domain_types/src/connector_flow.rs
#[derive(Debug, Clone)]
pub struct ClientAuthenticationToken;
```

It is listed in the `FlowName` enum at `crates/types-traits/domain_types/src/connector_flow.rs` so it can be rendered in telemetry and logs.

### Request Type

`ClientAuthenticationTokenRequestData` — `pub struct ClientAuthenticationTokenRequestData` in
`crates/types-traits/domain_types/src/connector_types.rs`. It has **13** fields at HEAD (older
revisions of this file listed 9, and listed `email` / `customer_name` which no longer exist —
they were folded into `customer: Option<CustomerInfo>`):

```rust
#[derive(Debug, Clone)]
pub struct ClientAuthenticationTokenRequestData {
    pub amount: MinorUnit,
    pub currency: Currency,
    pub country: Option<common_enums::CountryAlpha2>,
    pub order_details: Option<Vec<payment_address::OrderDetailsWithAmount>>,
    pub customer: Option<CustomerInfo>,
    pub order_tax_amount: Option<MinorUnit>,
    pub shipping_cost: Option<MinorUnit>,
    /// The specific payment method type for which the session token is being generated
    pub payment_method_type: Option<PaymentMethodType>,
    pub webhook_url: Option<String>,
    pub country_codes: Vec<common_enums::CountryAlpha2>,
    pub locale: Option<String>,
    /// Connector-specific permissions for client authentication token
    /// e.g., ["PMT_POST_Create_Single"] for GlobalPay hosted fields
    pub permissions: Option<Vec<String>>,
    /// Native app identifier for returning to the client app after the hosted
    /// flow completes (e.g. an Android package name). Sent instead of a return
    /// URL when the merchant integrates from a native app.
    pub native_app_identifier: Option<String>,
}
```

`CustomerInfo` (`pub struct CustomerInfo`, same file) carries `customer_id`, `customer_email`,
`customer_name`, `first_name`, `last_name`, `customer_phone_number`,
`customer_phone_country_code`, `salutation`, `date_of_birth` — reach for those instead of the
retired top-level `email` / `customer_name`.

Note `country_codes` is a bare `Vec`, not an `Option<Vec>`: it is always present and may be empty.

Note the shape is deliberately richer than a minimal session-token request: the connector may need currency/country to decide which wallet offers to surface, or amount to create an authorization envelope that the client-side SDK later confirms.

### Response Type

`PaymentsResponseData::ClientAuthenticationTokenResponse { session_data, status_code }` — `crates/types-traits/domain_types/src/connector_types.rs`:

```rust
// From crates/types-traits/domain_types/src/connector_types.rs
ClientAuthenticationTokenResponse {
    session_data: ClientAuthenticationTokenData,
    status_code: u16,
},
```

`ClientAuthenticationTokenData` — `pub enum ClientAuthenticationTokenData` in
`crates/types-traits/domain_types/src/connector_types.rs`. It is `#[serde(tag = "sdk_type")]` and
has **five** arms at HEAD:

```rust
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
    /// Plaid Link token for bank account linking via Plaid Link SDK
    Plaid(Box<PlaidClientAuthenticationResponse>),
}
```

> **Exception to the "one arm per connector on `ConnectorSpecificClientAuthenticationResponse`" rule.**
> Plaid is a **first-class arm on `ClientAuthenticationTokenData` itself**, sibling to the wallet
> arms — not a `ConnectorSpecific` arm. It is bank-account linking, not a wallet SDK and not a
> card checkout, so it did not fit the per-connector discriminator. See
> `pub struct PlaidClientAuthenticationResponse` in the same file, and
> `ClientAuthenticationTokenData::Plaid(Box::new(plaid_auth_response))` in
> `crates/integrations/connector-integration/src/authenticator_connectors/plaid/transformers.rs`:
> ```rust
> /// Plaid Link token for client-side bank account linking via Plaid Link SDK
> #[derive(Debug, Clone, Serialize, Deserialize)]
> pub struct PlaidClientAuthenticationResponse {
>     /// The Plaid Link token used to initialize Plaid Link
>     pub link_token: Secret<String>,
>     /// Seconds until the link_token expires (relative to when it was issued)
>     pub expires_in_seconds: Option<i64>,
>     /// Hosted Link URL if Plaid hosted Link is enabled
>     pub hosted_link_url: Option<String>,
> }
> ```
> Do not copy Plaid as the template for a new *payment* connector — the default is still to add one
> arm to `ConnectorSpecificClientAuthenticationResponse`.

`ConnectorSpecificClientAuthenticationResponse` is the extension point connectors use when their
SDK-init shape does not match the Google Pay / Apple Pay / PayPal canonical types. At HEAD it has
**26** arms — re-derive before quoting a count:

```bash
awk '/^pub enum ConnectorSpecificClientAuthenticationResponse/,/^}/' \
  crates/types-traits/domain_types/src/connector_types.rs | grep -cE '^\s+[A-Z][A-Za-z0-9]*\('
```

```rust
/// Per-connector SDK initialization data — discriminated by connector
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "connector")]
#[serde(rename_all = "snake_case")]
pub enum ConnectorSpecificClientAuthenticationResponse {
    Stripe(StripeClientAuthenticationResponse),
    Adyen(AdyenClientAuthenticationResponse),
    Checkout(CheckoutClientAuthenticationResponse),
    Cybersource(CybersourceClientAuthenticationResponse),
    Nuvei(NuveiClientAuthenticationResponse),
    Mollie(MollieClientAuthenticationResponse),
    Globalpay(GlobalpayClientAuthenticationResponse),
    Bluesnap(BluesnapClientAuthenticationResponse),
    Rapyd(RapydClientAuthenticationResponse),
    Shift4(Shift4ClientAuthenticationResponse),
    BankOfAmerica(BankOfAmericaClientAuthenticationResponse),
    Wellsfargo(WellsfargoClientAuthenticationResponse),
    Fiserv(FiservClientAuthenticationResponse),
    Elavon(ElavonClientAuthenticationResponse),
    Noon(NoonClientAuthenticationResponse),
    Paysafe(PaysafeClientAuthenticationResponse),
    Bamboraapac(BamboraapacClientAuthenticationResponse),
    Jpmorgan(JpmorganClientAuthenticationResponse),
    Billwerk(BillwerkClientAuthenticationResponse),
    Datatrans(DatatransClientAuthenticationResponse),
    Bambora(BamboraClientAuthenticationResponse),
    Payload(PayloadClientAuthenticationResponse),
    Multisafepay(MultisafepayClientAuthenticationResponse),
    Nexinets(NexinetsClientAuthenticationResponse),
    Nexixpay(NexixpayClientAuthenticationResponse),
    Revolut(RevolutClientAuthenticationResponse),
}

/// Stripe's client_secret for browser-side stripe.confirmPayment()
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StripeClientAuthenticationResponse {
    pub client_secret: Secret<String>,
}
```

An arm on this enum is a *type* declaration, not evidence of a wired flow: nine arms —
`Checkout`, `BankOfAmerica`, `Wellsfargo`, `Fiserv`, `Elavon`, `Noon`, `Paysafe`, `Bamboraapac`
and `Bambora` — exist ahead of, or independently of, a `flow: ClientAuthenticationToken,`
registration in the connector file. (`Cybersource` DOES have one, via a hand-written
`ConnectorIntegrationV2` impl.) See the live roster below.

### Resource Common Data

`MerchantAuthenticationFlowData` — `crates/types-traits/domain_types/src/merchant_authentication_flow_data.rs`.

> **CORRECTED (this file taught `PaymentFlowData` at ~15 sites through v1.1.0, and argued at length
> that piggy-backing on the payment context was the point).** That argument is inverted. The flow
> does **not** share the payment's `resource_common_data`, and there is no "subsequent confirm call
> sees the same merchant/attempt context" — there is no attempt. The type's own doc-comment states
> the intent:
>
> > Resource-common data for all MerchantAuthenticationService flows. Shared by:
> > `ServerAuthenticationToken`, `ServerSessionAuthenticationToken`, `ClientAuthenticationToken`.
> > **This type deliberately omits payment-specific fields (`payment_id`, `attempt_id`, `status`,
> > `payment_method`, `address`, `amount`, etc.) because merchant-authentication flows have no
> > payment identity.**
>
> Verify: `rg -n "flow_name: ClientAuthenticationToken" -A 1 crates/integrations/connector-integration/src/connectors/stripe.rs`
> → `resource_common_data: MerchantAuthenticationFlowData`.

Fields available to a `ClientAuthenticationToken` impl (`pub struct MerchantAuthenticationFlowData`):
`merchant_id`, `connectors`, `connector_request_reference_id`, `test_mode`, `return_url`,
`connector_feature_data`, `order_details`, `merchant_request_id`, plus the five observability
fields (`raw_connector_request` / `raw_connector_response` / `typed_connector_request` /
`typed_connector_response` / `connector_response_headers`). One inherent method: `get_return_url()`.

There is **no** `session_token`, `access_token`, `status`, `payment_id` or `address` on it. Any
generated code reading `req.resource_common_data.session_token` inside this flow fails with E0609.
`PaymentFlowData.session_token` does still exist — but it belongs to the *payment* flows and is
populated by the composite service from the `ServerSessionAuthenticationToken` response; see
`pattern_server_session_authentication_token.md`.

Base-URL access: because `resource_common_data` is not `PaymentFlowData`, use the merchant-auth
helper, not `connector_base_url_payments`. Stripe declares it in `create_all_prerequisites!` and
uses it in the flow's `get_url`:

```rust
// crates/integrations/connector-integration/src/connectors/stripe.rs
Ok(format!("{}{}", self.connector_base_url_merchant_auth(req), "v1/payment_intents"))
```

### Trait

`interfaces::connector_types::ClientAuthentication` — `crates/types-traits/interfaces/src/connector_types.rs`:

```rust
// From crates/types-traits/interfaces/src/connector_types.rs
pub trait ClientAuthentication:
    ConnectorIntegrationV2<
    connector_flow::ClientAuthenticationToken,
    MerchantAuthenticationFlowData,
    ClientAuthenticationTokenRequestData,
    PaymentsResponseData,
>
{
}
```

This trait is a constituent of **`ConnectorServiceTrait`** (payment connectors) *and* of
**`AuthenticatorServiceTrait`** (`ConnectorCommon + ValidationTrait + ClientAuthentication +
PaymentTokenV2<T> + GetPaymentMethodV2`) — both in
`crates/types-traits/interfaces/src/connector_types.rs`. Every connector in either registry must at
least blanket-impl it (even as a stub) to satisfy the trait bound on the overall service.

Note the **asymmetric response**: `ServerAuthentication` and `ServerSessionAuthentication` each bind
a dedicated `*ResponseData` struct, but `ClientAuthentication` binds the shared `PaymentsResponseData`
enum and carries its payload in the `ClientAuthenticationTokenResponse { session_data, status_code }`
variant. There is no `ClientAuthenticationTokenResponseData` type —
`rg -n "ClientAuthenticationTokenResponseData" crates/` returns zero hits.

## Relationship to other token flows

`ClientAuthenticationToken` lives in a family of adjacent but distinct flows. They were renamed together in PR #855 to remove ambiguity between "token issued to our server" and "token issued to the merchant's client". The table below is the authoritative map at the pinned SHA.

| Flow marker (`domain_types::connector_flow::*`) | Trait (`interfaces::connector_types::*`) | `resource_common_data` | Request data (`domain_types::connector_types::*`) | Response data | Audience of the resulting token | Primary purpose |
|---|---|---|---|---|---|---|
| `ClientAuthenticationToken` (this pattern) | `ClientAuthentication` | `MerchantAuthenticationFlowData` | `ClientAuthenticationTokenRequestData` (13 fields) | **`PaymentsResponseData`** (asymmetric) — payload in the `ClientAuthenticationTokenResponse { session_data: ClientAuthenticationTokenData, status_code: u16 }` variant | **Merchant's client device** (browser/mobile SDK) | Issue a short-lived, client-safe artifact (e.g. Stripe `client_secret`, Plaid `link_token`) that the frontend presents to the connector's SDK. |
| `ServerSessionAuthenticationToken` | `ServerSessionAuthentication` | `MerchantAuthenticationFlowData` | `ServerSessionAuthenticationTokenRequestData` (`amount`, `currency`, `browser_info`, `customer_id`, `address`) | `ServerSessionAuthenticationTokenResponseData { session_token: String }` | **Merchant's backend** (sometimes forwarded to client as a wallet session) | Obtain a per-transaction session token consumed by the following Authorize — see `pattern_server_session_authentication_token.md`. |
| `ServerAuthenticationToken` | `ServerAuthentication` | `MerchantAuthenticationFlowData` | `ServerAuthenticationTokenRequestData { grant_type: String }` | `ServerAuthenticationTokenResponseData { access_token: Secret<String>, token_type: Option<String>, expires_in: Option<i64> }` | **Merchant's backend only** (never leaves the server) | OAuth 2.0 bearer acquisition for subsequent server-to-server API calls — see `pattern_server_authentication_token.md`. |
| `CreateAccessToken` (retired name) | `PaymentAccessToken` (retired) | — | `AccessTokenRequestData` (retired) | `AccessTokenResponseData` (retired) | — | Replaced by `ServerAuthenticationToken` + `ServerAuthentication` per PR #855. See "Retired / pre-rename identifiers" below. |
| `CreateSessionToken` (retired name) | `PaymentSessionToken` (retired) | — | `SessionTokenRequestData` (retired) | `SessionTokenResponseData` (retired) | — | Replaced by `ServerSessionAuthenticationToken` + `ServerSessionAuthentication` per PR #855. |
| `SdkSessionToken` (retired name) | `SdkSessionTokenV2` (retired) | — | `PaymentsSdkSessionTokenData` (retired) | `PaymentsResponseData::SdkSessionTokenResponse { session_token: SessionToken }` (retired) | — | Replaced by `ClientAuthenticationToken` + `ClientAuthentication` + `ClientAuthenticationTokenRequestData` + `PaymentsResponseData::ClientAuthenticationTokenResponse` per PR #855. |

All three live markers are unit structs in `crates/types-traits/domain_types/src/connector_flow.rs`
(`pub struct ClientAuthenticationToken;` etc.), all three bind `MerchantAuthenticationFlowData`, and
all three are served by `service MerchantAuthenticationService` in
`crates/types-traits/grpc-api-types/proto/services.proto`
(`CreateClientAuthenticationToken` / `CreateServerSessionAuthenticationToken` /
`CreateServerAuthenticationToken`). Read each trait with
`rg -n "pub trait ClientAuthentication:" -A 9 crates/types-traits/interfaces/src/connector_types.rs`.

Key invariants:

1. **Audience**: If the token is meant to cross the trust boundary to the merchant's end-user device, use `ClientAuthenticationToken`. If it is a server-only credential, use `ServerAuthenticationToken`. If it is a per-transaction session bootstrap consumed by the following Authorize, use `ServerSessionAuthenticationToken`.
2. **Storage location**: none of the three writes to its own `resource_common_data` — `MerchantAuthenticationFlowData` has neither `access_token` nor `session_token`. Each returns its artifact in the response, and `crates/internal/composite-service/src/utils.rs` (`get_access_token`, `get_session_token`) folds it onto the *next* payment request, where it lands on `PaymentFlowData.access_token` / `PaymentFlowData.session_token`. `ClientAuthenticationToken`'s artifact is not folded back at all — it is returned to the caller for the client SDK.
3. **Response envelope**: `ClientAuthenticationToken` reuses the shared `PaymentsResponseData` enum (same as Authorize/Capture/PSync) with a dedicated variant; `ServerSessionAuthenticationToken` and `ServerAuthenticationToken` each use their own dedicated struct type (not `PaymentsResponseData`).

Seeing an older pattern that still talks about `CreateSessionToken` / `SessionTokenRequestData` / `SdkSessionToken` → treat it as pre-rename and translate via the table above before copying.

## Connectors with Full Implementation

**Re-derive the roster before trusting it.** The old count in this file (4 full implementations,
~78 stubs) is long dead:

```bash
rg -l "flow: ClientAuthenticationToken," crates/integrations/connector-integration/src/ \
  | grep -v /macros.rs
```

`connectors/macros.rs` is the macro *definition* file and always matches — exclude it. Do **not**
use `rg -l "flow_name: ClientAuthenticationToken"`: that under-counts, because connectors with a
hand-written `impl ConnectorIntegrationV2<ClientAuthenticationToken, ...>` register the tuple in
`create_all_prerequisites!` but never invoke `macro_connector_implementation!`.

At HEAD there are **20** registrations — 19 payment connectors plus one authenticator connector:

| Registry | Connectors |
|---|---|
| `connectors/` (payment, `ConnectorServiceTrait`) | `adyen`, `billwerk`, `bluesnap`, `braintree`, `cybersource`, `datatrans`, `globalpay`, `jpmorgan`, `mollie`, `multisafepay`, `nexinets`, `nexixpay`, `nuvei`, `payload`, `paypal`, `rapyd`, `revolut`, `shift4`, `stripe` |
| `authenticator_connectors/` (`AuthenticatorServiceTrait`) | `plaid` |

Two of those use a **hand-written** `ConnectorIntegrationV2` impl rather than
`macro_connector_implementation!`: `bluesnap` (it must read the token out of an HTTP response
header) and `cybersource` (see the `// Manual implementation for ClientAuthenticationToken flow.`
comment in `crates/integrations/connector-integration/src/connectors/cybersource.rs`).

Every other connector in either registry carries only the one-line marker impl
`impl ... connector_types::ClientAuthentication for <Connector><T> {}` so the service-trait bound is
satisfied; the default `ConnectorIntegrationV2` bodies apply. **This file previously printed a
hand-maintained ~78-name stub roll-call. It has been deleted rather than refreshed** — it was stale
in both directions (it listed Adyen, Cybersource, Nuvei, Paypal, Rapyd, Revolut and others as stubs
while they were registered, and named connectors that no longer exist). Derive the stub set as
"every connector not in the table above".

Note also: an arm on `ConnectorSpecificClientAuthenticationResponse` is **not** evidence of a wired
flow. `Checkout`, `BankOfAmerica`, `Wellsfargo`, `Fiserv`, `Elavon`, `Noon`, `Paysafe`,
`Bamboraapac` and `Bambora` all have arms but no `flow: ClientAuthenticationToken,` registration at
HEAD.

Rows below are alphabetical by connector (per §10 of `PATTERN_AUTHORING_SPEC.md`). They are worked
examples, not the full roster.

| Connector | HTTP Method | Content Type | URL Pattern | Request Type | Notes |
| --- | --- | --- | --- | --- | --- |
| Bluesnap | POST | `application/json` (empty body) | `{base_url}/services/2/payment-fields-tokens` | `BluesnapClientAuthRequest` (bespoke, empty marker struct — `pub struct BluesnapClientAuthRequest` in `crates/integrations/connector-integration/src/connectors/bluesnap/transformers.rs`) | Bluesnap's Hosted Payment Fields endpoint returns the `pfToken` in the **HTTP `Location` header** (last path segment), not the body. The flow uses a hand-written `ConnectorIntegrationV2` impl (not `macro_connector_implementation!`) so `handle_response_v2` can read `res.headers` — see the `// Location header format: https://sandbox.bluesnap.com/services/2/payment-fields-tokens/<pfToken>` comment in `crates/integrations/connector-integration/src/connectors/bluesnap.rs`. The extracted token is wrapped as `BluesnapClientAuthenticationResponse { pf_token }` → `ConnectorSpecificClientAuthenticationResponse::Bluesnap` → `ClientAuthenticationTokenData::ConnectorSpecific`. The `flow: ClientAuthenticationToken,` tuple is still present in `create_all_prerequisites!` — a hand-written impl does not replace the registration. |
| Globalpay | POST | `application/json` | `{base_url}/accesstoken` | `GlobalpayClientAuthRequest` (bespoke; carries `app_id`, `nonce`, SHA-512 `secret = SHA512(nonce + app_key)`, `grant_type: "client_credentials"` — `pub struct GlobalpayClientAuthRequest` in `crates/integrations/connector-integration/src/connectors/globalpay/transformers.rs`) | Reuses the same `/accesstoken` endpoint as Globalpay's `ServerAuthenticationToken` flow but deserializes into a separate response type so it routes to `ClientAuthenticationTokenResponse` instead of the OAuth carrier chain. Both flows are mechanism C, but they reach the base URL differently at HEAD: the `ServerAuthenticationToken` `get_url` calls `self.connector_base_url_merchant_auth(req)`, while the `ClientAuthenticationToken` `get_url` reads `&req.resource_common_data.connectors.globalpay.base_url` inline. Either is type-correct because `resource_common_data` is `MerchantAuthenticationFlowData` in both; `connector_base_url_payments` is not. The token, type, and `seconds_to_expire` are wrapped as `GlobalpayClientAuthenticationResponse { access_token, token_type, expires_in }` → `ConnectorSpecificClientAuthenticationResponse::Globalpay` → `ClientAuthenticationTokenData::ConnectorSpecific`. |
| Jpmorgan | POST | `application/x-www-form-urlencoded` | `{secondary_base_url}/am/oauth2/alpha/access_token` | `JpmorganClientAuthRequest { grant_type, scope }` — `crates/integrations/connector-integration/src/connectors/jpmorgan/requests.rs`. `scope` is `"jpm:payments:sandbox"` in test mode, `"jpm:payments"` otherwise. | Jpmorgan reuses its OAuth2 token endpoint (Basic auth over `client_id:client_secret`, base64-encoded) to issue a client-side access token. The endpoint lives on a **secondary base URL** distinct from the payments base URL; if `secondary_base_url` is unset, URL building fails with `FailedToObtainIntegrationUrl`. The returned `access_token` / `token_type` are mapped into `JpmorganClientAuthenticationResponse { transaction_id, request_id }` → `ConnectorSpecificClientAuthenticationResponse::Jpmorgan`. |
| Plaid | POST | `application/json` | `{base_url}/link/token/create` | `PlaidLinkTokenRequest` — `crates/integrations/connector-integration/src/authenticator_connectors/plaid/transformers.rs` | **Not a payment connector.** Lives in `authenticator_connectors/`, implements `AuthenticatorServiceTrait`, and is bank-account linking (Plaid Link), not 3DS and not a checkout. Its response takes the **first-class** `ClientAuthenticationTokenData::Plaid(Box::new(...))` arm, bypassing `ConnectorSpecificClientAuthenticationResponse` entirely. Macro block: `flow_name: ClientAuthenticationToken, resource_common_data: MerchantAuthenticationFlowData` in `crates/integrations/connector-integration/src/authenticator_connectors/plaid.rs`. |
| Stripe | POST | `application/x-www-form-urlencoded` | `{base_url}v1/payment_intents` | `StripeClientAuthRequest` (bespoke; wraps `PaymentIntent` creation without `confirm=true` — `pub struct StripeClientAuthRequest` in `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs`) | Returned `client_secret` is wrapped as `StripeClientAuthenticationResponse` → `ConnectorSpecificClientAuthenticationResponse::Stripe` → `ClientAuthenticationTokenData::ConnectorSpecific`. URL is built from `self.connector_base_url_merchant_auth(req)`, not `connector_base_url_payments`. |

### Shared-types consolidation (PR #1002)

PR #1002 (commit `03e9fab77`, "feat(shared): consolidate ClientAuthenticationToken shared types for all connectors") centralised every per-connector SDK-init response struct into a single file so that adding a new connector is a one-arm addition to `ConnectorSpecificClientAuthenticationResponse` rather than a parallel-type proliferation. Before #1002 the discriminator enum held only `Stripe` and `Globalpay`; after #1002 it held 19 arms (Stripe, Adyen, Checkout, Cybersource, Nuvei, Mollie, Globalpay, Bluesnap, Rapyd, Shift4, BankOfAmerica, Wellsfargo, Fiserv, Elavon, Noon, Paysafe, Bamboraapac, Jpmorgan, Billwerk). A follow-on PR #1023 (commit `0af11797d`, "consolidate ClientAuthenticationToken shared types for batch 2 connectors") added Datatrans, Bambora, Payload, Multisafepay, Nexinets, and Nexixpay. **At HEAD the enum has 26 arms**, `Revolut` last in declaration order. Re-count rather than quoting this number.

**Where the consolidated types live** (all in `crates/types-traits/domain_types/src/connector_types.rs` at the pinned SHA):

| Item | Citation | What it replaced |
|---|---|---|
| `ConnectorSpecificClientAuthenticationResponse` discriminator enum (26 arms at HEAD) | `crates/types-traits/domain_types/src/connector_types.rs` | Pre-#1002 two-arm enum (`Stripe`, `Globalpay`) that would have required a separate per-connector domain struct in each connector crate. |
| `AdyenClientAuthenticationResponse { session_id, session_data }` | `crates/types-traits/domain_types/src/connector_types.rs` | Previously a connector-local type inside `adyen/transformers.rs`. |
| `CheckoutClientAuthenticationResponse { payment_session_id, payment_session_token, payment_session_secret }` | `crates/types-traits/domain_types/src/connector_types.rs` | Previously a connector-local type inside `checkout/transformers.rs`. |
| `CybersourceClientAuthenticationResponse { capture_context, client_library, client_library_integrity }` | `crates/types-traits/domain_types/src/connector_types.rs` | Previously a connector-local `CybersourceClientAuthResponse` type. |
| `NuveiClientAuthenticationResponse { session_token }` | `crates/types-traits/domain_types/src/connector_types.rs` | Previously a connector-local type. |
| `MollieClientAuthenticationResponse { payment_id, checkout_url }` | `crates/types-traits/domain_types/src/connector_types.rs` | Previously a connector-local type. |
| `GlobalpayClientAuthenticationResponse { access_token, token_type, expires_in }` (already present pre-#1002, kept unchanged) | `crates/types-traits/domain_types/src/connector_types.rs` | No replacement — this was the template the other connectors were rewritten to match. |
| `BluesnapClientAuthenticationResponse { pf_token }` | `crates/types-traits/domain_types/src/connector_types.rs` | Previously a connector-local type inside `bluesnap/transformers.rs`. |
| `RapydClientAuthenticationResponse { checkout_id, redirect_url }` | `crates/types-traits/domain_types/src/connector_types.rs` | Previously a connector-local type. |
| `Shift4ClientAuthenticationResponse { client_secret }` | `crates/types-traits/domain_types/src/connector_types.rs` | Previously a connector-local type. |
| `BankOfAmericaClientAuthenticationResponse { capture_context }` | `crates/types-traits/domain_types/src/connector_types.rs` | Previously a connector-local type. |
| `WellsfargoClientAuthenticationResponse { capture_context }` | `crates/types-traits/domain_types/src/connector_types.rs` | Previously a connector-local type. |
| `FiservClientAuthenticationResponse { session_id }` | `crates/types-traits/domain_types/src/connector_types.rs` | Previously a connector-local type. |
| `ElavonClientAuthenticationResponse { session_token }` | `crates/types-traits/domain_types/src/connector_types.rs` | Previously a connector-local type. |
| `NoonClientAuthenticationResponse { order_id, checkout_url }` | `crates/types-traits/domain_types/src/connector_types.rs` | Previously a connector-local type. |
| `PaysafeClientAuthenticationResponse { payment_handle_token }` | `crates/types-traits/domain_types/src/connector_types.rs` | Previously a connector-local type. |
| `BamboraapacClientAuthenticationResponse { token }` | `crates/types-traits/domain_types/src/connector_types.rs` | Previously a connector-local type. |
| `JpmorganClientAuthenticationResponse { transaction_id, request_id }` | `crates/types-traits/domain_types/src/connector_types.rs` | Previously a connector-local `JpmorganClientAuthResponseDomain`-style type. |
| `BillwerkClientAuthenticationResponse { session_id }` | `crates/types-traits/domain_types/src/connector_types.rs` | Previously a connector-local type. |

Additional downstream consolidation (same PR #1002): the `From<...>` → `grpc_api_types::payments::ConnectorSpecificClientAuthenticationResponse` converter now lives centrally in `crates/types-traits/domain_types/src/types.rs` (one arm per connector), replacing what would otherwise be per-connector `types.rs` impls.

**What this means for a new connector**: to add a ClientAuthenticationToken implementation today, the author

1. Adds one arm to `ConnectorSpecificClientAuthenticationResponse` in `crates/types-traits/domain_types/src/connector_types.rs` (append only; the enum is not alphabetised and the compiler does not enforce ordering).
   **Exception:** if the artifact is not an SDK-init payload for a card/wallet checkout at all, it may
   warrant a first-class arm on `ClientAuthenticationTokenData` instead — that is what
   `ClientAuthenticationTokenData::Plaid(Box<PlaidClientAuthenticationResponse>)` is. That exception
   has exactly one member today; do not reach for it without a reason as strong as Plaid's.
2. Defines the sibling `<ConnectorName>ClientAuthenticationResponse` struct immediately below the enum (same file), mirroring the patterns above.
3. Adds one arm to the gRPC conversion `match` in `crates/types-traits/domain_types/src/types.rs` — grep `ClientAuthenticationTokenData::` there to find the two match sites (the domain→gRPC `From` impls) that must both gain an arm.
4. Wires the connector-local transformer to wrap the extracted token in `ClientAuthenticationTokenData::ConnectorSpecific(Box::new(ConnectorSpecificClientAuthenticationResponse::<ConnectorName>(<ConnectorName>ClientAuthenticationResponse { ... })))`.

Authors MUST NOT re-introduce a parallel connector-local `<ConnectorName>ClientAuthResponseDomain` type — the pre-#1002 pattern — unless the new connector's response has fields that genuinely cannot be represented by any of the existing structs; in that case, extending the shared enum in `connector_types.rs` is the correct change.

## Common Implementation Patterns

The recommended path at this SHA is the macro-based pattern used by Stripe. The flow is stateless from UCS's perspective (no access-token reuse, no caching), so the entire wiring is exactly two macro blocks plus two `TryFrom` impls.

### Macro wiring (recommended)

Inside the connector's single `macros::create_all_prerequisites!` invocation, add a tuple for the flow. Excerpted from Stripe:

```rust
// From crates/integrations/connector-integration/src/connectors/stripe.rs
(
    flow: ClientAuthenticationToken,
    request_body: StripeClientAuthRequest,
    response_body: StripeClientAuthResponse,
    router_data: RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
)
```

Then, for the concrete integration, a second macro block declares URL/headers/HTTP method:

```rust
// From crates/integrations/connector-integration/src/connectors/stripe.rs
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Stripe,
    curl_request: FormUrlEncoded(StripeClientAuthRequest),
    curl_response: StripeClientAuthResponse,
    flow_name: ClientAuthenticationToken,
    resource_common_data: MerchantAuthenticationFlowData,
    flow_request: ClientAuthenticationTokenRequestData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}{}",
                self.connector_base_url_merchant_auth(req),
                "v1/payment_intents"
            ))
        }
    }
);
```

The `connector_types::ClientAuthentication` trait itself is then a one-liner:

```rust
// From crates/integrations/connector-integration/src/connectors/stripe.rs
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Stripe<T>
{
}
```

### Request transformation (recommended)

Implement `TryFrom<StripeRouterData<RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>, T>>` for the connector-local request struct. The Stripe example at `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs` converts the minor unit amount via the connector's amount converter, lowercases the currency, copies the reference id into Stripe's `metadata[...]` bag, and turns on `automatic_payment_methods[enabled]` so Stripe picks the method at SDK time.

### Response transformation (recommended)

Implement `TryFrom<ResponseRouterData<ConnectorResponse, Self>> for RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>`. Wrap the connector's token in `ClientAuthenticationTokenData::ConnectorSpecific(...)` unless your connector already maps cleanly onto the Google Pay / Apple Pay / PayPal canonical enum arms.

### Alternate: pre-existing canonical wallet response

If the connector returns a Google Pay session payload, produce `ClientAuthenticationTokenData::GooglePay(...)` directly instead of going through `ConnectorSpecific`. Same for PayPal (`Paypal(...)`) and Apple Pay (`ApplePay(...)`). This preserves structured `serde` tagging for downstream UCS consumers. See `crates/types-traits/domain_types/src/connector_types.rs` for the canonical arms.

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

- **Endpoint**: `POST {base_url}v1/payment_intents` — `crates/integrations/connector-integration/src/connectors/stripe.rs`.
- **Wire format**: `application/x-www-form-urlencoded` (Stripe API requires form encoding, not JSON). Enforced at the macro layer via `curl_request: FormUrlEncoded(StripeClientAuthRequest)` at `crates/integrations/connector-integration/src/connectors/stripe.rs`.
- **Request shape**: `StripeClientAuthRequest` at `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs` — fields `amount`, `currency`, `automatic_payment_methods[enabled]` (always `Some(true)` in this flow), and a flattened `meta_data: HashMap<String, String>` populated with the order reference. Notably, `confirm` is deliberately omitted so the PaymentIntent is created in the "requires_confirmation" state; confirmation happens client-side via `stripe.confirmPayment()`.
- **Authentication**: `Bearer {api_key}` header. Stripe accepts either a live or test `sk_...` key; see `ConnectorSpecificConfig::Stripe { api_key, base_url }` at `crates/types-traits/domain_types/src/router_data.rs` (`ConnectorAuthType` was deleted on 2026-03-14, `a7a696c3a`; auth now comes from `req.connector_config`). The `build_headers` helper is shared across all Stripe flows at `crates/integrations/connector-integration/src/connectors/stripe.rs`.
- **Response shape**: `StripeClientAuthResponse` wraps Stripe's `PaymentIntentResponse` — `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs`. The only field read is `client_secret`.
- **Mapping**: The extracted `client_secret` is wrapped as `ClientAuthenticationTokenData::ConnectorSpecific(Box::new(ConnectorSpecificClientAuthenticationResponse::Stripe(StripeClientAuthenticationResponseDomain { client_secret })))` — `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs`. `StripeClientAuthenticationResponseDomain` is not a distinct type: it is a local `use ... as` alias for the shared `domain_types` `StripeClientAuthenticationResponse`, introduced to disambiguate from Stripe's own wire-level `StripeClientAuthResponse`. Do not define a parallel `*Domain` struct.
- **Error mode**: If `response.client_secret` is `None`, the transformer returns `ConnectorError::ResponseDeserializationFailed { context }` — `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs`. HTTP-level errors flow through the standard `build_error_response` shared with Stripe's other flows.

### Globalpay

- **Endpoint**: `POST {base_url}/accesstoken` — `crates/integrations/connector-integration/src/connectors/globalpay.rs`.
- **Wire format**: JSON. Enforced at the macro layer via `curl_request: Json(GlobalpayClientAuthRequest)` at `crates/integrations/connector-integration/src/connectors/globalpay.rs`.
- **Request shape**: `GlobalpayClientAuthRequest { app_id, nonce, secret, grant_type }` at `crates/integrations/connector-integration/src/connectors/globalpay/transformers.rs`. The `nonce` is a 12-character random alphanumeric string (`rand::distributions::Alphanumeric` at `crates/integrations/connector-integration/src/connectors/globalpay/transformers.rs`) and `secret = hex(SHA512(nonce || app_key))` — see `crates/integrations/connector-integration/src/connectors/globalpay/transformers.rs`. `grant_type` is hard-coded to `"client_credentials"`.
- **Authentication**: the `app_id` + `app_key` are both pulled from `ConnectorSpecificConfig::Globalpay` (`crates/integrations/connector-integration/src/connectors/globalpay/transformers.rs`). The only header added beyond the default content-type is `X-GP-Version: {API_VERSION}` (`crates/integrations/connector-integration/src/connectors/globalpay.rs`). No bearer token is sent because this call mints one.
- **Response shape**: `GlobalpayClientAuthResponse { token, type_, seconds_to_expire }` at `crates/integrations/connector-integration/src/connectors/globalpay/transformers.rs` — note the `type_` field uses `#[serde(rename = "type")]` since `type` is a Rust keyword.
- **Mapping**: the response is wrapped as `ClientAuthenticationTokenData::ConnectorSpecific(Box::new(ConnectorSpecificClientAuthenticationResponse::Globalpay(GlobalpayClientAuthenticationResponse { access_token, token_type: Some(type_), expires_in: Some(seconds_to_expire) })))` at `crates/integrations/connector-integration/src/connectors/globalpay/transformers.rs`. The shared `GlobalpayClientAuthenticationResponse` type lives at `crates/types-traits/domain_types/src/connector_types.rs`.
- **Relationship to ServerAuthenticationToken**: Globalpay's access-token endpoint is the same URL used for the server-side OAuth flow. The two flows differ only in (a) their response type so the data lands in `ClientAuthenticationTokenResponse` vs. `ServerAuthenticationTokenResponseData`, and (b) whether the resulting token is returned to the client or stored on `PaymentFlowData.access_token`.

### Bluesnap

- **Endpoint**: `POST {base_url}/services/2/payment-fields-tokens` — `crates/integrations/connector-integration/src/connectors/bluesnap.rs`.
- **Wire format**: JSON, with an **empty body** (`pub struct BluesnapClientAuthRequest {}` at `crates/integrations/connector-integration/src/connectors/bluesnap/transformers.rs`). Bluesnap's Hosted Payment Fields bootstrap does not require any payload.
- **Not macro-wired**: unlike Stripe/Globalpay/Jpmorgan, Bluesnap uses a hand-written `impl ConnectorIntegrationV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData> for Bluesnap<T>` at `crates/integrations/connector-integration/src/connectors/bluesnap.rs`, because the returned `pfToken` lives in the **HTTP `Location` header** rather than the JSON body. Macro-tuple registration at `crates/integrations/connector-integration/src/connectors/bluesnap.rs` is present for type-list completeness but the `handle_response_v2` override on the hand-written impl is what actually extracts the token.
- **Location-header extraction**: `handle_response_v2` fetches `res.headers.get("location")`, then splits on `/` and takes the last segment as the pfToken (`crates/integrations/connector-integration/src/connectors/bluesnap.rs`). A missing header or an un-splittable URL yields `ConnectorError::ResponseDeserializationFailed` with a specific `additional_context` message.
- **Response shape (synthetic)**: the extracted string is packed into `BluesnapClientAuthResponse { pf_token: Some(Secret::new(...)) }` (`crates/integrations/connector-integration/src/connectors/bluesnap.rs`) and then passed through the standard `TryFrom<ResponseRouterData<BluesnapClientAuthResponse, Self>>` impl at `crates/integrations/connector-integration/src/connectors/bluesnap/transformers.rs`, which unwraps the `Option` (erroring on `None`) and wraps the token into `ConnectorSpecificClientAuthenticationResponse::Bluesnap(BluesnapClientAuthenticationResponse { pf_token })`. The shared struct lives at `crates/types-traits/domain_types/src/connector_types.rs`.
- **Authentication**: Basic auth header computed by the shared `build_headers` helper (`crates/integrations/connector-integration/src/connectors/bluesnap.rs`).

### Jpmorgan

- **Endpoint**: `POST {secondary_base_url}/am/oauth2/alpha/access_token` — `crates/integrations/connector-integration/src/connectors/jpmorgan.rs`. Note the use of a **secondary** base URL (`jpmorgan.secondary_base_url`) distinct from the payments base URL; if unset, `FailedToObtainIntegrationUrl` is returned with a documentation pointer (`crates/integrations/connector-integration/src/connectors/jpmorgan.rs`).
- **Wire format**: `application/x-www-form-urlencoded` — enforced via `curl_request: FormUrlEncoded(JpmorganClientAuthRequest)` at `crates/integrations/connector-integration/src/connectors/jpmorgan.rs` and a hard-coded `get_content_type` override plus a literal `headers::CONTENT_TYPE` entry inside the `flow_name: ClientAuthenticationToken` macro block in `crates/integrations/connector-integration/src/connectors/jpmorgan.rs` (the old `574-577` citation pointed at the Authorize flow's `get_url` and has been dropped).
- **Request shape**: `JpmorganClientAuthRequest { grant_type, scope }` at `crates/integrations/connector-integration/src/connectors/jpmorgan/requests.rs`. Scope is `"jpm:payments:sandbox"` when `resource_common_data.test_mode` is `Some(true)` or `None`, and `"jpm:payments"` otherwise (`crates/integrations/connector-integration/src/connectors/jpmorgan/transformers.rs`). `grant_type` is hard-coded to `"client_credentials"`.
- **Authentication**: **Basic auth** over `client_id:client_secret`, base64-encoded — built inline in the `get_headers` override at `crates/integrations/connector-integration/src/connectors/jpmorgan.rs`. This is different from Jpmorgan's payment endpoints which use a Bearer token previously obtained via `ServerAuthenticationToken`.
- **Response shape**: `JpmorganClientAuthResponse { access_token, scope, token_type, expires_in }` at `crates/integrations/connector-integration/src/connectors/jpmorgan/responses.rs`. This is the raw OAuth2 token response from JPMC.
- **Mapping**: `ClientAuthenticationTokenData::ConnectorSpecific(Box::new(ConnectorSpecificClientAuthenticationResponse::Jpmorgan(JpmorganClientAuthenticationResponse { transaction_id: access_token, request_id: token_type })))` at `crates/integrations/connector-integration/src/connectors/jpmorgan/transformers.rs`. Note that the `transaction_id` field of the shared struct is reused here to carry the OAuth `access_token` — this is a deliberate shape choice in the shared type (`crates/types-traits/domain_types/src/connector_types.rs`) and not a mismapping; the shared struct is named for Jpmorgan's dominant per-flow convention (`transaction_id` / `request_id`).
- **Dual endpoint with ServerAuthenticationToken**: Jpmorgan's client-auth call reuses the same OAuth2 token endpoint as the server-auth flow; they differ in which `RouterDataV2` flow marker is used and where the token ends up. Unlike Globalpay, no distinct request field is needed — the endpoint accepts the same client-credentials grant either way.

### Plaid — the first-class-arm exception (authenticator connector)

- **Registry**: `crates/integrations/connector-integration/src/authenticator_connectors/plaid.rs`, a
  *sibling* directory of `connectors/`, not a subdirectory. It implements `AuthenticatorServiceTrait`
  (`ConnectorCommon + ValidationTrait + ClientAuthentication + PaymentTokenV2<T> + GetPaymentMethodV2`),
  which is a much smaller bound than `ConnectorServiceTrait`.
- **Not 3DS.** Plaid Link is bank-account linking. It shares nothing with the
  `PreAuthenticate`/`Authenticate`/`PostAuthenticate` trio (mechanism A).
- **Endpoint**: `POST {connectors.plaid.base_url}/link/token/create`. The `get_url` override reads
  `req.resource_common_data.connectors.plaid.base_url` directly off `MerchantAuthenticationFlowData`.
- **Request**: `pub struct PlaidLinkTokenRequest { client_id, secret, client_name, user: PlaidUser,
  products, country_codes, language, redirect_uri, android_package_name, webhook }` —
  `authenticator_connectors/plaid/transformers.rs`. Note it consumes
  `ClientAuthenticationTokenRequestData`'s newer fields directly: `country_codes`, `locale`
  (→ `language`), `native_app_identifier` (→ `android_package_name`) and `webhook_url` (→ `webhook`).
- **Response mapping — the exception**:

  ```rust
  // authenticator_connectors/plaid/transformers.rs
  let plaid_auth_response = PlaidClientAuthenticationResponse {
      link_token: res.link_token,
      expires_in_seconds,          // derived from res.expiration, an RFC3339 timestamp
      hosted_link_url: res.hosted_link_url,
  };

  let session_data = ClientAuthenticationTokenData::Plaid(Box::new(plaid_auth_response));

  Ok(Self {
      response: Ok(PaymentsResponseData::ClientAuthenticationTokenResponse {
          session_data,
          status_code: item.http_code,
      }),
      ..item.router_data
  })
  ```

  There is **no** `ConnectorSpecificClientAuthenticationResponse::Plaid` arm. The
  "each new connector adds one arm to `ConnectorSpecificClientAuthenticationResponse`" rule stated
  above has this one exception. A new payment connector should still follow the rule.
- **Expiry handling worth copying**: Plaid returns an RFC3339 `expiration` string, not a duration.
  The transformer parses it, converts to seconds-from-now, and **drops** the value (logging at
  `debug`) when the string does not parse or the instant is already past — it does not fail the
  flow and does not emit a negative `expires_in_seconds`.

## Code Examples

### 1. Flow marker (`connector_flow.rs`)

```rust
// From crates/types-traits/domain_types/src/connector_flow.rs
#[derive(Debug, Clone)]
pub struct ClientAuthenticationToken;
```

### 2. Request data (`connector_types.rs`)

```rust
// From crates/types-traits/domain_types/src/connector_types.rs — 13 fields
#[derive(Debug, Clone)]
pub struct ClientAuthenticationTokenRequestData {
    pub amount: MinorUnit,
    pub currency: Currency,
    pub country: Option<common_enums::CountryAlpha2>,
    pub order_details: Option<Vec<payment_address::OrderDetailsWithAmount>>,
    pub customer: Option<CustomerInfo>,
    pub order_tax_amount: Option<MinorUnit>,
    pub shipping_cost: Option<MinorUnit>,
    pub payment_method_type: Option<PaymentMethodType>,
    pub webhook_url: Option<String>,
    pub country_codes: Vec<common_enums::CountryAlpha2>,
    pub locale: Option<String>,
    pub permissions: Option<Vec<String>>,
    pub native_app_identifier: Option<String>,
}
```

### 3. Response envelope (`connector_types.rs`)

```rust
// From crates/types-traits/domain_types/src/connector_types.rs
ClientAuthenticationTokenResponse {
    session_data: ClientAuthenticationTokenData,
    status_code: u16,
},
```

### 4. Session-data enum + Stripe / Plaid arms (`connector_types.rs`)

```rust
// From crates/types-traits/domain_types/src/connector_types.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "sdk_type")]
#[serde(rename_all = "snake_case")]
pub enum ClientAuthenticationTokenData {
    GooglePay(Box<GpayClientAuthenticationResponse>),
    Paypal(Box<PaypalClientAuthenticationResponse>),
    ApplePay(Box<ApplepayClientAuthenticationResponse>),
    ConnectorSpecific(Box<ConnectorSpecificClientAuthenticationResponse>),
    Plaid(Box<PlaidClientAuthenticationResponse>),   // first-class arm, NOT ConnectorSpecific
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "connector")]
#[serde(rename_all = "snake_case")]
pub enum ConnectorSpecificClientAuthenticationResponse {
    Stripe(StripeClientAuthenticationResponse),
    // ... 25 further arms at HEAD; see the full list under "Response Type" above
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StripeClientAuthenticationResponse {
    pub client_secret: Secret<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaidClientAuthenticationResponse {
    pub link_token: Secret<String>,
    pub expires_in_seconds: Option<i64>,
    pub hosted_link_url: Option<String>,
}
```

### 5. Trait definition (`interfaces/src/connector_types.rs`)

```rust
// From crates/types-traits/interfaces/src/connector_types.rs
pub trait ClientAuthentication:
    ConnectorIntegrationV2<
    connector_flow::ClientAuthenticationToken,
    MerchantAuthenticationFlowData,
    ClientAuthenticationTokenRequestData,
    PaymentsResponseData,
>
{
}
```

### 6. Stripe request TryFrom (`stripe/transformers.rs`)

```rust
// From crates/integrations/connector-integration/src/connectors/stripe/transformers.rs
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
                MerchantAuthenticationFlowData,
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
                MerchantAuthenticationFlowData,
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
// From crates/integrations/connector-integration/src/connectors/stripe/transformers.rs
/// Wraps PaymentIntentResponse for the ClientAuthenticationToken flow.
#[derive(Debug, Deserialize, Serialize)]
pub struct StripeClientAuthResponse(PaymentIntentResponse);

impl TryFrom<ResponseRouterData<StripeClientAuthResponse, Self>>
    for RouterDataV2<
        ClientAuthenticationToken,
        MerchantAuthenticationFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<StripeClientAuthResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response.0;

        // The real type is `domain_types::errors::ConnectorError`; there is no
        // `ConnectorResponseTransformationError` anywhere in the tree.
        // `ConnectorError` has exactly FIVE variants (errors.rs):
        // ResponseDeserializationFailed { context }, ResponseHandlingFailed { context },
        // UnexpectedResponseError { context },
        // IntegrityCheckFailed { context, field_names, connector_transaction_id },
        // ConnectorErrorResponse(Box<ErrorResponse>). All carry `context`.
        // Verbatim from connectors/stripe/transformers.rs.
        let client_secret =
            response
                .client_secret
                .ok_or(ConnectorError::ResponseDeserializationFailed {
                    context: Default::default(),
                })?;

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
// From crates/integrations/connector-integration/src/connectors/stripe.rs
(
    flow: ClientAuthenticationToken,
    request_body: StripeClientAuthRequest,
    response_body: StripeClientAuthResponse,
    router_data: RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
)
```

## Integration Guidelines

1. Confirm your connector actually issues a client-bound credential (browser/mobile SDK consumes it). If the token is server-only, use `ServerAuthenticationToken` (OAuth bearer) or `ServerSessionAuthenticationToken` (wallet session) instead — see the relationship table above.
2. In `<connector>.rs`, add `ClientAuthenticationToken` to the `connector_flow` import list and `ClientAuthenticationTokenRequestData` to the `connector_types` import list, and `merchant_authentication_flow_data::MerchantAuthenticationFlowData` to the `domain_types` import list, mirroring Stripe's `use domain_types::{...}` block in `crates/integrations/connector-integration/src/connectors/stripe.rs`.
3. Add a tuple to the connector's existing `macros::create_all_prerequisites!` block with `flow: ClientAuthenticationToken, request_body: <ConnectorName>ClientAuthRequest, response_body: <ConnectorName>ClientAuthResponse, router_data: RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>`. Model on `stripe.rs`.
4. Add a `macros::macro_connector_implementation!` block with `flow_name: ClientAuthenticationToken`, `resource_common_data: MerchantAuthenticationFlowData`, `flow_request: ClientAuthenticationTokenRequestData`, `flow_response: PaymentsResponseData`, `http_method: Post`, and the appropriate `curl_request:`/`curl_response:` variants. Model on `stripe.rs`.
5. In `<connector>/transformers.rs`, define `<ConnectorName>ClientAuthRequest` and `<ConnectorName>ClientAuthResponse` and implement `TryFrom` impls analogous to `stripe/transformers.rs`. Pull `amount` and `currency` out of the request data; do NOT hard-code them.
6. Map the connector's opaque token into `ClientAuthenticationTokenData`. If the token is a canonical Google Pay / Apple Pay / PayPal session, use the matching arm directly; otherwise extend `ConnectorSpecificClientAuthenticationResponse` in `crates/types-traits/domain_types/src/connector_types.rs` with a new connector arm and produce `ClientAuthenticationTokenData::ConnectorSpecific(...)`. Adding a new arm requires updating the enum definition in a dedicated PR.
7. Return `PaymentsResponseData::ClientAuthenticationTokenResponse { session_data, status_code: item.http_code }` from the response transformer — nothing else. There is nothing else you *could* write: `MerchantAuthenticationFlowData` has no `session_token` / `access_token` / `status` field, so `resource_common_data` is passed through unmodified via `..item.router_data`.
8. Blanket-impl `connector_types::ClientAuthentication for <ConnectorName><T>` once — one-line, empty body. The trait bound is structural; no methods to implement.
9. On the response transformer error branch, use `ConnectorError::ResponseDeserializationFailed { context }` when the connector returns HTTP 2xx but the expected token field is absent; use the connector's shared `build_error_response` for HTTP 4xx/5xx.
10. No ValidationTrait toggle is required for `ClientAuthenticationToken` at this SHA (unlike `ServerSessionAuthenticationToken`, which is gated by `ValidationTrait::should_do_session_token(&self, connector_feature_data: Option<&Secret<String>>)` — see `pattern_server_session_authentication_token.md`). The flow is invoked explicitly by the gRPC handler when the caller asks for a client token, so no server-side decision gate is needed.

## Best Practices

- **Use the macro path**, not manual `ConnectorIntegrationV2` impls. Stripe's full implementation is exactly two macro blocks plus two `TryFrom`s (`crates/integrations/connector-integration/src/connectors/stripe.rs`, `stripe.rs`, `stripe/transformers.rs`). Manual impls invite drift.
- **Mask client secrets**: the Stripe response type stores `client_secret: Secret<String>` (`crates/types-traits/domain_types/src/connector_types.rs`). Never log the unwrapped string. Any new `ConnectorSpecificClientAuthenticationResponse` arm MUST wrap its token in `Secret<_>`.
- **Do not confirm on the server**: if your connector's "create intent" endpoint accepts a `confirm` parameter, leave it out. Stripe's transformer comment at `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs` is explicit about this invariant — confirmation is the client's job.
- **Carry the merchant reference in metadata**: use `router_data.resource_common_data.connector_request_reference_id` for idempotency/traceability. Stripe routes it via `get_transaction_metadata` at `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs`.
- **Amount conversion**: always go through the connector's macro-generated amount converter. Stripe uses `StripeAmountConvertor::convert(...)` at `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs`. Hand-rolled amount arithmetic is banned by §12 of the spec.
- **Prefer existing canonical arms** (`GooglePay` / `Paypal` / `ApplePay`) over `ConnectorSpecific` whenever the connector already returns a wallet-standard payload — it reduces per-client deserialization work.
- **Cross-ref `pattern_server_session_authentication_token.md`** for the sibling `ServerSessionAuthenticationToken` flow (wallet sessions for Apple Pay / Google Pay / PayPal) and `pattern_server_authentication_token.md` for OAuth bearer acquisition. Those are NOT interchangeable with this flow.

## Common Errors / Gotchas

1. **Problem**: Using `CreateSessionToken` / `SessionTokenRequestData` / `PaymentSessionToken` / `SessionToken` identifiers.
   **Solution**: Those were retired by PR #855. Replace per the [retired identifiers](#retired--pre-rename-identifiers) table. The review rubric check #5 in `PATTERN_AUTHORING_SPEC.md` FAILs any pattern that references them outside a "retired — do not use" callout.

2. **Problem**: Using `SdkSessionToken` / `SdkSessionTokenV2` / `PaymentsSdkSessionTokenData` / `SdkSessionTokenResponse` identifiers.
   **Solution**: These were the pre-#855 names for this exact flow. Use `ClientAuthenticationToken` / `ClientAuthentication` / `ClientAuthenticationTokenRequestData` / `ClientAuthenticationTokenResponse` respectively.

3. **Problem**: Reaching for `resource_common_data.session_token`, `.access_token`, `.status`, `.payment_id` or `.address` inside this flow (copied from a `PaymentFlowData` flow).
   **Solution**: None of those fields exist on `MerchantAuthenticationFlowData` — the code will not compile (E0609). Read the struct: `crates/types-traits/domain_types/src/merchant_authentication_flow_data.rs`. `ClientAuthenticationToken` returns its artifact via the typed `session_data: ClientAuthenticationTokenData` field of `PaymentsResponseData::ClientAuthenticationTokenResponse`, and passes `resource_common_data` through untouched.

4. **Problem**: Returning a bare `String` or `serde_json::Value` as the token from a new connector.
   **Solution**: Always wrap it in `StripeClientAuthenticationResponse`-style typed struct inside `ConnectorSpecificClientAuthenticationResponse`. Extend `crates/types-traits/domain_types/src/connector_types.rs` with a new arm in a dedicated PR if your connector is neither Stripe nor a canonical wallet.

5. **Problem**: Sending the request as JSON for Stripe.
   **Solution**: Stripe requires `application/x-www-form-urlencoded` on all `/v1/*` endpoints. The macro argument is `curl_request: FormUrlEncoded(StripeClientAuthRequest)` at `crates/integrations/connector-integration/src/connectors/stripe.rs`, not `Json(...)`. Mismatching content types yields a 415 / 400 from Stripe.

6. **Problem**: Calling `confirm=true` on the PaymentIntent to "finish" the flow server-side.
   **Solution**: Never. The whole point of `ClientAuthenticationToken` is that the client confirms. Stripe's request struct at `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs` deliberately omits `confirm`.

7. **Problem**: Stubbing the flow by adding it to `create_all_prerequisites!` and `macro_connector_implementation!` with empty bodies, which then dispatches to a real endpoint.
   **Solution**: To stub, provide only the trait blanket-impl `connector_types::ClientAuthentication for <Connector><T> {}` and nothing else. Leave the macros out entirely. The default `ConnectorIntegrationV2` impl is already what you want.

8. **Problem**: Forgetting to blanket-impl `ClientAuthentication`, then hitting a trait-bound error on `ConnectorServiceTrait`.
   **Solution**: Every connector needs the one-line marker impl (stub or real), because `ClientAuthentication` is a supertrait of both `ConnectorServiceTrait` and `AuthenticatorServiceTrait` in `crates/types-traits/interfaces/src/connector_types.rs`.

9. **Problem**: Mapping all HTTP 2xx to a success even when `client_secret` is absent.
   **Solution**: Stripe treats missing `client_secret` as `ConnectorError::ResponseDeserializationFailed { context }` at `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs`. Mirror this pattern: a "200 with no token" is a parse failure, not a success.

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
- Missing-token path: a 2xx with the token field missing returns `ConnectorError::ResponseDeserializationFailed { context }` (mirroring `stripe/transformers.rs`).

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
| `SdkSessionToken` | flow marker struct | `ClientAuthenticationToken` | `crates/types-traits/domain_types/src/connector_flow.rs` |
| `SdkSessionTokenV2` | trait | `ClientAuthentication` | `crates/types-traits/interfaces/src/connector_types.rs` |
| `PaymentsSdkSessionTokenData` | request-data struct | `ClientAuthenticationTokenRequestData` | `crates/types-traits/domain_types/src/connector_types.rs` |
| `SdkSessionTokenResponse` (variant of `PaymentsResponseData`) | response-data variant | `ClientAuthenticationTokenResponse { session_data, status_code }` | `crates/types-traits/domain_types/src/connector_types.rs` |
| `SessionToken` (enum, the sdk-data payload) | payload enum | `ClientAuthenticationTokenData` | `crates/types-traits/domain_types/src/connector_types.rs` |
| `GpaySessionTokenResponse` | Google Pay payload struct | `GpayClientAuthenticationResponse` | `crates/types-traits/domain_types/src/connector_types.rs` |
| `PaypalSessionTokenResponse` | PayPal payload struct | `PaypalClientAuthenticationResponse` | `crates/types-traits/domain_types/src/connector_types.rs` |
| `ApplepaySessionTokenResponse` | Apple Pay payload struct | `ApplepayClientAuthenticationResponse` | `crates/types-traits/domain_types/src/connector_types.rs` |
| `CreateSessionToken` | flow marker struct | `ServerSessionAuthenticationToken` | `crates/types-traits/domain_types/src/connector_flow.rs` |
| `PaymentSessionToken` | trait | `ServerSessionAuthentication` | `crates/types-traits/interfaces/src/connector_types.rs` |
| `SessionTokenRequestData` | request-data struct | `ServerSessionAuthenticationTokenRequestData` | `crates/types-traits/domain_types/src/connector_types.rs` |
| `SessionTokenResponseData` | response-data struct | `ServerSessionAuthenticationTokenResponseData` | `crates/types-traits/domain_types/src/connector_types.rs` |
| `CreateAccessToken` | flow marker struct | `ServerAuthenticationToken` | `crates/types-traits/domain_types/src/connector_flow.rs` |
| `PaymentAccessToken` | trait | `ServerAuthentication` | `crates/types-traits/interfaces/src/connector_types.rs` |
| `AccessTokenRequestData` | request-data struct | `ServerAuthenticationTokenRequestData` | `crates/types-traits/domain_types/src/connector_types.rs` |
| `AccessTokenResponseData` | response-data struct | `ServerAuthenticationTokenResponseData` | `crates/types-traits/domain_types/src/connector_types.rs` |

Additionally, the following identifiers from `PATTERN_AUTHORING_SPEC.md` §12 "Retired types" remain prohibited for any new pattern:

- `ConnectorError` (monolithic, pre-PR-#765) → use `IntegrationError` (request-time) or `ConnectorError` (response-time; five variants, all carrying `context`).
- `RouterData` (V1) → use `RouterDataV2<...>`.
- `api::ConnectorIntegration` (V1 trait) → use `interfaces::connector_integration_v2::ConnectorIntegrationV2`.
- Hand-rolled amount conversion helpers → use macro-generated `<ConnectorName>AmountConvertor` via `common_utils::types`. There are FIVE unit types — `MinorUnit`, `StringMinorUnit`, `StringMajorUnit`, `FloatMajorUnit`, `StringTwoDecimalUnit` — and no safe default; match the vendor wire format.

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
| 1.1.0 | 2026-04-20 | `60540470c` | Absorbed three new full implementations merged after 1.0.0: Globalpay (PR #957 / commit `dd456e9ae`, `POST {base_url}/accesstoken` with nonce+SHA512 secret derivation, `crates/integrations/connector-integration/src/connectors/globalpay.rs`), Bluesnap (PR #959 / commit `0b1e7958a`, hand-written non-macro `ConnectorIntegrationV2` impl that extracts the pfToken from the HTTP `Location` header, `crates/integrations/connector-integration/src/connectors/bluesnap.rs`), and Jpmorgan (PR #966 / commit `c231dcd78`, OAuth2 token endpoint on `secondary_base_url` using Basic auth of `client_id:client_secret`, `crates/integrations/connector-integration/src/connectors/jpmorgan.rs`). Also added the new "Shared-types consolidation (PR #1002)" subsection documenting the move of per-connector SDK-init response structs into the shared `ConnectorSpecificClientAuthenticationResponse` enum at `crates/types-traits/domain_types/src/connector_types.rs` (commit `03e9fab77`) and the downstream gRPC conversion at `crates/types-traits/domain_types/src/types.rs+`. Stub-implementation roll-call preserved verbatim from 1.0.0 with a "Refresh note" callout listing the graduated connectors. |
| 2.0.0 | 2026-09-07 | HEAD | Mechanism-C correction pass. **`PaymentFlowData` → `MerchantAuthenticationFlowData` at every site** in this flow's generics and macro blocks, and the prose arguing that the flow "piggy-backs on the same `PaymentFlowData`" deleted as inverted (the type's own doc-comment says it deliberately omits payment fields). Corrected `ClientAuthenticationTokenRequestData` to its real 13 fields (`email`/`customer_name` are gone; `customer: Option<CustomerInfo>`, `webhook_url`, `country_codes`, `locale`, `permissions`, `native_app_identifier` are new). Documented the **asymmetric response** (`PaymentsResponseData`, no `ClientAuthenticationTokenResponseData`). Added the fifth `ClientAuthenticationTokenData` arm, **`Plaid`** — a first-class arm, the one exception to the "each new connector adds one arm to `ConnectorSpecificClientAuthenticationResponse`" rule — and Plaid as an `authenticator_connectors/` worked example. Enum arm count corrected 17/25 → **26**. Roster re-derived live: **20 registrations** (19 payment + Plaid), replacing the "4 full / ~78 stub" split; the hand-maintained stub roll-call was deleted rather than refreshed. Noted that `rg "flow_name:"` under-counts vs `rg "flow: "` because `bluesnap`/`cybersource` hand-write the `ConnectorIntegrationV2` impl. Stripe `get_url` corrected to `connector_base_url_merchant_auth`. All numeric `file.rs:NNN` citations re-anchored to symbol names. |
