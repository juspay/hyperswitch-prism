# VerifyWebhookSource Flow Pattern

| Field | Value |
|-------|-------|
| Version | 1.0.0 |
| Generated | 2026-04-20 |
| Pinned SHA | `ceb33736ce941775403f241f3f0031acbf2b4527` (connector-service) |
| Spec-Version | 1 |
| Applies-To | flow pattern |

## Overview

`VerifyWebhookSource` is the authenticity-verification step for incoming connector webhooks. It answers a single yes/no question: *"Did this HTTP request really originate from the connector?"* and returns `VerifyWebhookStatus::SourceVerified` or `VerifyWebhookStatus::SourceNotVerified` (see `crates/types-traits/domain_types/src/router_response_types.rs:90`).

This is distinct from the `IncomingWebhook` flow:

- **VerifyWebhookSource** = signature verification only. It consumes `webhook_headers`, `webhook_body`, and `merchant_secret` and produces a boolean-equivalent verdict (`crates/types-traits/domain_types/src/router_request_types.rs:466`).
- **IncomingWebhook** = full payload parsing/dispatch — event-type detection, transaction-ID extraction, refund/dispute routing, status mapping (see `pattern_IncomingWebhook_flow.md`).

A typical webhook handler invokes `VerifyWebhookSource` first, then `IncomingWebhook` only if verification passes. The orchestration is done in `crates/grpc-server/grpc-server/src/server/events.rs:111-148`: the gRPC `EventService` picks the verification path (in-band trait vs. out-of-band flow) and passes a `source_verified` boolean into `process_webhook_event`.

### Key Components

- **Flow marker**: `VerifyWebhookSource` unit struct — `crates/types-traits/domain_types/src/connector_flow.rs:71`.
- **Request type**: `VerifyWebhookSourceRequestData` — `crates/types-traits/domain_types/src/router_request_types.rs:466`.
- **Response type**: `VerifyWebhookSourceResponseData` + `VerifyWebhookStatus` — `crates/types-traits/domain_types/src/router_response_types.rs:90`, `:95`.
- **Flow-data (resource_common_data)**: `VerifyWebhookSourceFlowData` — `crates/types-traits/domain_types/src/connector_types.rs:2688`.
- **In-band trait**: `IncomingWebhook::verify_webhook_source` — `crates/types-traits/interfaces/src/connector_types.rs:375`.
- **Out-of-band trait**: `VerifyWebhookSourceV2: ConnectorIntegrationV2<VerifyWebhookSource, VerifyWebhookSourceFlowData, VerifyWebhookSourceRequestData, VerifyWebhookSourceResponseData>` — `crates/types-traits/interfaces/src/connector_types.rs:354-362`.
- **Default-impl macro**: `default_impl_verify_webhook_source_v2!` — `crates/integrations/connector-integration/src/default_implementations.rs:27-46`, applied to 80+ connectors at `:50-127`.
- **Orchestration selector**: `requires_external_webhook_verification` — `crates/types-traits/interfaces/src/connector_types.rs:139`.
- **Signature-scheme primitives**: `common_utils::crypto::{HmacSha256, HmacSha512, Sha256, Md5, SignMessage, VerifySignature}` (referenced by connectors at `bluesnap.rs:233`, `noon.rs:245`, `payload.rs:780`, `fiuu.rs:843`, etc.).

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Signature Schemes](#signature-schemes)
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

`VerifyWebhookSource` has *two* runtime shapes in this codebase. Authors must understand both before implementing.

### Flow Hierarchy

```
EventService::handle (grpc-server)
  └── requires_external_webhook_verification(connector_id, config)
      │   (crates/types-traits/interfaces/src/connector_types.rs:139)
      │
      ├── TRUE → verify_webhook_source_external  (out-of-band)
      │           (crates/grpc-server/grpc-server/src/server/events.rs:167)
      │     builds RouterDataV2<VerifyWebhookSource, VerifyWebhookSourceFlowData,
      │                         VerifyWebhookSourceRequestData,
      │                         VerifyWebhookSourceResponseData>
      │     dispatches to ConnectorIntegrationV2 → makes an HTTP call
      │     to the connector (e.g. PayPal `/v1/notifications/verify-webhook-signature`)
      │     ⇒ VerifyWebhookStatus::SourceVerified | SourceNotVerified
      │
      └── FALSE → IncomingWebhook::verify_webhook_source  (in-band)
                  (crates/types-traits/interfaces/src/connector_types.rs:375)
             computes HMAC/SHA/MD5 signature locally, compares
             to header-borne signature, returns Ok(bool)
```

The selector is configuration-driven: only connectors listed in
`config.webhook_source_verification_call.connectors_with_webhook_source_verification_call`
take the external path (`crates/grpc-server/grpc-server/src/server/events.rs:104-109`).

### Flow Type

`VerifyWebhookSource` — marker unit struct at `crates/types-traits/domain_types/src/connector_flow.rs:71`. Re-exported from `domain_types::connector_flow`.

### Request Type

`VerifyWebhookSourceRequestData` — `crates/types-traits/domain_types/src/router_request_types.rs:466`:

```rust
// From crates/types-traits/domain_types/src/router_request_types.rs:465
#[derive(Debug, Clone)]
pub struct VerifyWebhookSourceRequestData {
    pub webhook_headers: std::collections::HashMap<String, String>,
    pub webhook_body: Vec<u8>,
    pub merchant_secret: ConnectorWebhookSecrets,
    pub webhook_uri: Option<String>,
}
```

- `webhook_headers` carries the raw HTTP headers (signature header lives here, e.g. `paypal-transmission-sig`, `tl-signature`, `bls-signature`).
- `webhook_body` is the raw byte payload (must be the unparsed body to keep HMAC digests stable).
- `merchant_secret` is a `ConnectorWebhookSecrets { secret: Vec<u8>, additional_secret: Option<Secret<String>> }` (`crates/types-traits/domain_types/src/connector_types.rs:1928`). For out-of-band verifiers this may hold a webhook id rather than a shared key — PayPal stores the webhook id here (`paypal/transformers.rs:3244`).
- `webhook_uri` is the full URL the connector posted to (needed by schemes that sign the URL — e.g. TrueLayer JWS, `truelayer/transformers.rs:1292-1306`).

### Response Type

`VerifyWebhookSourceResponseData` — `crates/types-traits/domain_types/src/router_response_types.rs:90`:

```rust
// From crates/types-traits/domain_types/src/router_response_types.rs:89
#[derive(Debug, Clone)]
pub struct VerifyWebhookSourceResponseData {
    pub verify_webhook_status: VerifyWebhookStatus,
}

// From crates/types-traits/domain_types/src/router_response_types.rs:94
#[derive(Debug, Clone, PartialEq)]
pub enum VerifyWebhookStatus {
    SourceVerified,
    SourceNotVerified,
}
```

Only two terminal states — there is no `Pending`.

### Resource Common Data

`VerifyWebhookSourceFlowData` — `crates/types-traits/domain_types/src/connector_types.rs:2688`:

```rust
// From crates/types-traits/domain_types/src/connector_types.rs:2687
#[derive(Debug, Clone)]
pub struct VerifyWebhookSourceFlowData {
    pub connectors: Connectors,
    pub connector_request_reference_id: String,
    pub raw_connector_response: Option<Secret<String>>,
    pub raw_connector_request: Option<Secret<String>>,
    pub connector_response_headers: Option<http::HeaderMap>,
}
```

Impls `RawConnectorRequestResponse` at `connector_types.rs:2696` and `ConnectorResponseHeaders` at `:2714`. The flow-data intentionally omits a body field: the body is already inside `VerifyWebhookSourceRequestData.webhook_body`.

### Integrity

`VerifyWebhookSourceIntegrityObject { webhook_id: String }` is defined at `crates/types-traits/domain_types/src/router_request_types.rs:473` and compared by `crates/types-traits/interfaces/src/integrity.rs:956-976`. The request-side integrity object is built by extracting the UTF-8 form of `merchant_secret.secret` (`integrity.rs:398-408`) — this matches the PayPal semantics where `secret` actually holds a webhook id, not a shared HMAC key. The response-side object is `None` because connector verify-webhook endpoints do not echo the webhook id (`integrity.rs:399-401`).

### Canonical RouterDataV2 signature

```rust
// Canonical (matches PATTERN_AUTHORING_SPEC.md §7)
RouterDataV2<
    VerifyWebhookSource,              // connector_flow.rs:71
    VerifyWebhookSourceFlowData,      // connector_types.rs:2688
    VerifyWebhookSourceRequestData,   // router_request_types.rs:466
    VerifyWebhookSourceResponseData,  // router_response_types.rs:90
>
```

Built at `crates/grpc-server/grpc-server/src/server/events.rs:197-208`.

## Signature Schemes

Enumeration of authenticity schemes actually observed in `src/connectors/` at the pinned SHA. The column "Where implemented" distinguishes in-band (inside `IncomingWebhook::verify_webhook_source`) from out-of-band (inside `ConnectorIntegrationV2<VerifyWebhookSource, ...>`).

| Scheme | Connectors | Where implemented | Citation |
|--------|-----------|-------------------|----------|
| **HMAC-SHA256** | Adyen | In-band (`IncomingWebhook::verify_webhook_source`) | `crates/integrations/connector-integration/src/connectors/adyen.rs:773` |
| HMAC-SHA256 | Bluesnap | In-band | `crates/integrations/connector-integration/src/connectors/bluesnap.rs:234` |
| HMAC-SHA256 | Revolut | In-band | `crates/integrations/connector-integration/src/connectors/revolut.rs:296` |
| HMAC-SHA256 | Trustpay | In-band | `crates/integrations/connector-integration/src/connectors/trustpay.rs:210` |
| HMAC-SHA256 | Cryptopay | In-band | `crates/integrations/connector-integration/src/connectors/cryptopay.rs:468` |
| HMAC-SHA256 | Ppro | In-band | `crates/integrations/connector-integration/src/connectors/ppro.rs:563` |
| HMAC-SHA256 | Authipay (request signing, not webhook but same crate primitive) | Request-side | `crates/integrations/connector-integration/src/connectors/authipay/transformers.rs:50` |
| HMAC-SHA256 | Fiservcommercehub / Fiservemea / Redsys (request signing) | Request-side | `fiservcommercehub/transformers.rs:48`, `fiservemea/transformers.rs:45`, `redsys/transformers.rs:324` |
| **HMAC-SHA512** | Authorizedotnet | In-band, header `X-ANET-Signature` | `crates/integrations/connector-integration/src/connectors/authorizedotnet.rs:185-186` |
| HMAC-SHA512 | Noon | In-band | `crates/integrations/connector-integration/src/connectors/noon.rs:245` |
| **SHA-256 (plain digest, not HMAC)** | Novalnet | In-band — secret-suffixed message digest | `crates/integrations/connector-integration/src/connectors/novalnet.rs:689` |
| SHA-256 | Payload | In-band — body-only digest | `crates/integrations/connector-integration/src/connectors/payload.rs:780` |
| **MD5** | Fiuu | In-band — concatenated-field digest including secret | `crates/integrations/connector-integration/src/connectors/fiuu.rs:843` |
| **Ed25519 / ECDSA-P521 JWS (JWK-fetched public key)** | Truelayer | Out-of-band — fetches connector JWKS, verifies JWS over `tl-signature` header | `crates/integrations/connector-integration/src/connectors/truelayer.rs:878-989`, `truelayer/transformers.rs:1192-1327` |
| **RSA/certificate-based webhook-id echo** (PayPal `verify-webhook-signature` API) | Paypal | Out-of-band — delegates crypto to PayPal's verification endpoint | `crates/integrations/connector-integration/src/connectors/paypal.rs:1482-1597`, `paypal/transformers.rs:3194-3283` |
| **Plain shared secret / no local crypto** (default trait body returns `Ok(false)`) | All connectors covered by `default_impl_verify_webhook_source_v2!` that do not override the in-band method | Trait default body | `crates/types-traits/interfaces/src/connector_types.rs:375-382` |

### Notes on each scheme

**HMAC-SHA256** is the most common in-band scheme. Connectors hex- or base64-decode the header-borne signature, sign the raw body (or a constructed message string) with the merchant secret, and compare byte-for-byte via `HmacSha256::sign_message` or `HmacSha256::verify_signature`. Example: Bluesnap compares `HmacSha256.sign_message(secret, timestamp ++ body)` against `hex::decode("bls-signature")` at `bluesnap.rs:234-251`.

**HMAC-SHA512** is Auth.net and Noon. Auth.net hex-decodes `X-ANET-Signature` and signs `request.body` directly with `HmacSha512.sign_message(secret, body)` at `authorizedotnet.rs:184-201`. Noon uses `HmacSha512.verify_signature` with the connector's documented concatenation order at `noon.rs:244-264`.

**Plain SHA-256 digest (not HMAC)** is used by Novalnet and Payload. Novalnet concatenates fields *including* the reversed webhook secret and digests the result (`novalnet.rs:689` uses `crypto::Sha256`). Payload digests the body without a key, using `crypto::Sha256::verify_signature` at `payload.rs:780-797`. These schemes rely on the secret being present *inside* the message, not as an HMAC key — they are weaker than HMAC and must use constant-time comparison inside `VerifySignature::verify_signature`.

**MD5** is Fiuu-only and is retained only for API compatibility with the connector. `fiuu.rs:843` calls `crypto::Md5.verify_signature(secret, signature, message)` where `message` is a ConnectorName-specific field concatenation including the secret (`fiuu.rs:820-833`). Authors of new connectors should not use MD5 unless the connector API mandates it.

**Ed25519 / ECDSA-P521 JWS** appears only in Truelayer. The `tl-signature` header is a detached JWS; the JWS header carries a `jku` URL pointing to a connector-hosted JWKS. Verification is split across two pieces:

1. The `ConnectorIntegrationV2<VerifyWebhookSource, ...>::get_url` parses `tl-signature`, extracts `jku`, validates it against `truelayer::ALLOWED_JKUS`, and returns the JKU as the outgoing URL (`truelayer.rs:890-940`, `truelayer/transformers.rs:1090`).
2. `handle_response_v2` parses the fetched `Jwks`, finds the key whose `kid` matches the JWS header, rebuilds the SEC1 EC point via `build_uncompressed_ec1_point`, rebuilds the signing input (`POST <uri>\n<tl-headers>\n<body>`), converts the P1363 signature to DER, and runs SHA-512 + ECDSA verify (`truelayer/transformers.rs:1192-1327`).

**PayPal webhook-id echo** (`paypal.rs:1482-1597`) posts the received transmission headers + raw webhook event JSON + stored `webhook_id` to PayPal's `v1/notifications/verify-webhook-signature` endpoint, and PayPal returns `verification_status: SUCCESS | FAILURE`. The local code performs no RSA math; it relies on PayPal to verify the attached cert chain. The response is mapped via `PaypalSourceVerificationStatus → VerifyWebhookStatus` at `paypal/transformers.rs:3254-3261`.

**Plain shared secret / no crypto** is the default. `IncomingWebhook::verify_webhook_source` returns `Ok(false)` if not overridden (`connector_types.rs:375-382`), and the `default_impl_verify_webhook_source_v2!` macro emits an empty `ConnectorIntegrationV2` impl (`default_implementations.rs:27-46`). Connectors in this bucket effectively skip verification and fall back to PSync for truth.

## Connectors with Full Implementation

"Full implementation" = the connector provides a non-stub body for `verify_webhook_source` (in-band) *or* a real `ConnectorIntegrationV2<VerifyWebhookSource, ...>` impl (out-of-band). Connectors that only have the macro-generated empty impl are listed under *Stub Implementations*.

### Out-of-band (ConnectorIntegrationV2<VerifyWebhookSource,…>)

| Connector | HTTP Method | Content Type | URL Pattern | Request Type Reuse | Notes |
|-----------|-------------|--------------|-------------|---------------------|-------|
| Paypal | POST | application/json | `{base_url}v1/notifications/verify-webhook-signature` | `paypal::PaypalSourceVerificationRequest` (dedicated) | Uses Basic auth (`client_id:client_secret`) per `paypal.rs:1516-1532`; sends back `SUCCESS`/`FAILURE` mapped at `paypal/transformers.rs:3254`. |
| Truelayer | GET | (none — GET fetch of JWKS) | `jku` extracted from `tl-signature` JWS header; must be in `ALLOWED_JKUS` | No request body; response `truelayer::Jwks` | Verification math runs inside `TryFrom<ResponseRouterData<Jwks, …>>` at `truelayer/transformers.rs:1229-1328`. `get_url` at `truelayer.rs:890-940`. |

### In-band (IncomingWebhook::verify_webhook_source override)

| Connector | Scheme | Signature location | Message construction | Citation |
|-----------|--------|--------------------|----------------------|----------|
| Adyen | HMAC-SHA256, base64 | Extracted from notif body | `"{psp_ref}:{orig_ref}:{account}:{amount}:{currency}:{event_code}:{success}"` | `adyen.rs:766-805`; message built at `:746-764` |
| Authorizedotnet | HMAC-SHA512, hex | Header `X-ANET-Signature` | raw `request.body` | `authorizedotnet.rs:184-201` |
| Bluesnap | HMAC-SHA256, hex | Header `bls-signature` | `timestamp ++ body` | `bluesnap.rs:216-267` |
| Cryptopay | HMAC-SHA256 | Header (X-Cryptopay-Signature) | body-specific message | `cryptopay.rs:462-490` |
| Fiuu | MD5 | Field `skey` in form body | concat(transaction_id, order_id, status, merchant_id, amount, secret) | `fiuu.rs:837-861`; message at `:817-834` |
| Noon | HMAC-SHA512 | body field | connector-documented concatenation | `noon.rs:239-264` |
| Novalnet | SHA-256 (plain) | body | amount+currency+...+reversed_secret | `novalnet.rs:683-750` |
| Payload | SHA-256 (plain) | header | raw body | `payload.rs:774-798` |
| Ppro | HMAC-SHA256, hex | Header `Webhook-Signature` | raw body | `ppro.rs:546-574` |
| Revolut | HMAC-SHA256 | Header (`Revolut-Signature`) | `"v1.{timestamp}.{body}"` | `revolut.rs:268-339` |
| Trustpay | HMAC-SHA256 | extracted from body | sorted payload values joined by `/` | `trustpay.rs:196-220` |

### Stub Implementations

Connectors relying on the default `default_impl_verify_webhook_source_v2!` macro (`crates/integrations/connector-integration/src/default_implementations.rs:50-127`) — these have *no* verification logic beyond the trait's `Ok(false)` default:

Aci, Airwallex, Authipay, Bambora, Bamboraapac, Bankofamerica, Barclaycard, Billwerk, Braintree, Calida, Cashfree, Cashtocode, Celero, Checkout, Cybersource, Datatrans, Dlocal, Elavon, Fiserv, Fiservcommercehub, Fiservemea, Forte, Getnet, Gigadat, Globalpay, Helcim, Hipay, Hyperpg, Iatapay, Itaubank, Jpmorgan, Loonio, Mifinity, Mollie, Multisafepay, Nexinets, Nexixpay, Nmi, Nuvei, Paybox, Payme, Paysafe, Paytm, Payu, Peachpayments, Phonepe, Placetopay, Powertranz, Rapyd, Razorpay, RazorpayV2, Redsys, Revolv3, Finix, Shift4, Silverflow, Stax, Stripe, Trustpayments, Tsys, Volt, Wellsfargo, Worldpay, Worldpayvantiv, Worldpayxml, Xendit, Zift.

(Itaubank and Peachpayments have *named* but *empty* impl blocks — `itaubank.rs:738-746`, `peachpayments.rs:650-658` — and are still stubs in behavior.)

## Common Implementation Patterns

There are two families. Pick based on whether your connector does local crypto (family 1) or requires a round-trip HTTP call (family 2).

### Family 1 — In-band (local crypto via `IncomingWebhook::verify_webhook_source`)

Recommended for 95 % of connectors. No network round-trip, no extra plumbing; the default connector-selector path (`crates/grpc-server/grpc-server/src/server/events.rs:122-141`) calls the trait method directly.

Required overrides (from trait `IncomingWebhook` at `crates/types-traits/interfaces/src/connector_types.rs:374-411`):

1. `fn verify_webhook_source(&self, request, webhook_secret, _account_details) -> Result<bool, Report<WebhookError>>` — the entry point.
2. `fn get_webhook_source_verification_signature(&self, request, secret) -> Result<Vec<u8>, Report<WebhookError>>` — decode header signature (hex, base64, or body field).
3. `fn get_webhook_source_verification_message(&self, request, secret) -> Result<Vec<u8>, Report<WebhookError>>` — construct the canonical bytes to sign.

Inside `verify_webhook_source`, build an algorithm and call `SignMessage::sign_message` + `eq`, or `VerifySignature::verify_signature`. Use `common_utils::crypto::{HmacSha256, HmacSha512, Md5, Sha256}`.

Reference: `crates/integrations/connector-integration/src/connectors/bluesnap.rs:216-267` (HMAC-SHA256 timestamp-prefixed body, hex header).

### Family 2 — Out-of-band (`ConnectorIntegrationV2<VerifyWebhookSource, ...>`)

Required when the connector performs verification via its own API (cert-chain, JWKS fetch, or proprietary crypto). Two connectors use this: PayPal (`paypal.rs:1482-1597`) and Truelayer (`truelayer.rs:878-989`).

Integration requirements:

1. Implement `ConnectorIntegrationV2<VerifyWebhookSource, VerifyWebhookSourceFlowData, VerifyWebhookSourceRequestData, VerifyWebhookSourceResponseData>` with real `get_url`, `get_headers`, `get_request_body`, `handle_response_v2`, `get_error_response_v2`.
2. Implement `connector_types::VerifyWebhookSourceV2` as the marker-only trait (no methods).
3. Add the connector to the *external-verification* config set so that `requires_external_webhook_verification` returns `true` (`crates/types-traits/interfaces/src/connector_types.rs:139-151`).
4. Remove the connector from the `default_impl_verify_webhook_source_v2!` macro list to avoid duplicate impls (`default_implementations.rs:50-127`; PayPal is explicitly absent per the comment at `:128`).
5. Provide a `TryFrom<&VerifyWebhookSourceRequestData>` for the connector's verification-request struct (`paypal/transformers.rs:3195-3252`).
6. Provide a `TryFrom<ResponseRouterData<ConnectorVerifyResponse, Self>>` that sets `response: Ok(VerifyWebhookSourceResponseData { verify_webhook_status })` (`paypal/transformers.rs:3263-3283`, `truelayer/transformers.rs:1229-1328`).

## Connector-Specific Patterns

### Paypal (out-of-band)

File: `crates/integrations/connector-integration/src/connectors/paypal.rs:1482-1597`; transformers at `crates/integrations/connector-integration/src/connectors/paypal/transformers.rs:3154-3283`.

- `get_url` always returns `{base_url}v1/notifications/verify-webhook-signature` (`paypal.rs:1499-1505`).
- `get_headers` overrides the normal bearer-token auth with **Basic auth** (`paypal.rs:1516-1532`). This is the one PayPal endpoint that demands `client_id:client_secret`, not an access token.
- Header normalization is critical: incoming webhook headers are lowercased at `paypal/transformers.rs:3185-3192` before key lookup, because PayPal transmission headers are documented in lowercase (`paypal-transmission-id`, etc. at `:3019-3026`).
- `webhook_event` is re-serialized to JSON with preserved field order (`preserve_order` feature → `IndexMap`, see comment at `paypal/transformers.rs:3198-3199`). Reordering would break signature verification on PayPal's side.
- The stored `merchant_secret.secret` is the **webhook id**, not a shared key; it is UTF-8-decoded at `paypal/transformers.rs:3244-3248` and echoed back to PayPal.
- Response mapping: `SUCCESS → SourceVerified`, `FAILURE → SourceNotVerified` (`paypal/transformers.rs:3254-3261`).

### Truelayer (out-of-band, detached JWS)

File: `crates/integrations/connector-integration/src/connectors/truelayer.rs:878-989`; verification math at `crates/integrations/connector-integration/src/connectors/truelayer/transformers.rs:1192-1328`.

- URL dispatch: GET to the JKU from the JWS header, but only if the JKU is in the allow-list `ALLOWED_JKUS` (`truelayer/transformers.rs:1090-1093`). This defends against SSRF via attacker-chosen `jku` values.
- The verification response type is a JWKS (`truelayer/transformers.rs:1063-1066`). The matching JWK is located by `kid` (`:1266-1273`).
- Signing input format: `"POST <uri>\n{tl-headers}\n<body>"` where `tl-headers` is lifted from the JWS header's `tl_headers` list (`truelayer/transformers.rs:1200-1215`).
- Signature is P1363 → DER-converted (`:1221`) and verified with SHA-512 + ECDSA (`:1224`).
- The URI is tried twice: with and without a `PREFIX` prepended (`:1299-1315`). This handles connectors that configure a hostname prefix inconsistently.

### In-band HMAC variants

See per-connector rows in the table above. The only meaningful differences are (a) where the signature lives (header name, body field, or base64 inside a JSON field), (b) what bytes are fed into the HMAC (raw body, `timestamp ++ body`, sorted fields joined by `/`, etc.), and (c) which digest (SHA-256 vs SHA-512).

## Code Examples

### Example 1 — PayPal out-of-band (full impl)

```rust
// From crates/integrations/connector-integration/src/connectors/paypal.rs:1482
// VerifyWebhookSource implementation using ConnectorIntegrationV2
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VerifyWebhookSource,
        VerifyWebhookSourceFlowData,
        VerifyWebhookSourceRequestData,
        VerifyWebhookSourceResponseData,
    > for Paypal<T>
{
    fn get_url(
        &self,
        req: &RouterDataV2<
            VerifyWebhookSource,
            VerifyWebhookSourceFlowData,
            VerifyWebhookSourceRequestData,
            VerifyWebhookSourceResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        let base_url = self.base_url(&req.resource_common_data.connectors);
        Ok(format!(
            "{}v1/notifications/verify-webhook-signature",
            base_url
        ))
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<
            VerifyWebhookSource,
            VerifyWebhookSourceFlowData,
            VerifyWebhookSourceRequestData,
            VerifyWebhookSourceResponseData,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        // PayPal verify-webhook-signature uses Basic Auth (client_id:client_secret),
        // not Bearer token.
        let auth = transformers::PaypalAuthType::try_from(&req.connector_config).change_context(
            IntegrationError::FailedToObtainAuthType { context: Default::default() },
        )?;
        let credentials = auth.get_credentials()?;
        let auth_val = credentials.generate_authorization_value();
        Ok(vec![
            (headers::CONTENT_TYPE.to_string(), "application/json".to_string().into()),
            (headers::AUTHORIZATION.to_string(), auth_val.into_masked()),
        ])
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<…>,
    ) -> CustomResult<Option<RequestContent>, IntegrationError> {
        let verification_request = paypal::PaypalSourceVerificationRequest::try_from(&req.request)?;
        Ok(Some(RequestContent::Json(Box::new(verification_request))))
    }

    fn handle_response_v2(&self, data, event_builder, res) -> CustomResult<RouterDataV2<…>, _> {
        let verification_response: paypal::PaypalSourceVerificationResponse =
            res.response.parse_struct("PaypalSourceVerificationResponse")?;
        // From :1579
        RouterDataV2::try_from(ResponseRouterData { response: verification_response, router_data: data.clone(), http_code: res.status_code })
    }
}
```

### Example 2 — PayPal request-builder transformer (header extraction)

```rust
// From crates/integrations/connector-integration/src/connectors/paypal/transformers.rs:3194
impl TryFrom<&VerifyWebhookSourceRequestData> for PaypalSourceVerificationRequest {
    type Error = Report<IntegrationError>;
    fn try_from(req: &VerifyWebhookSourceRequestData) -> Result<Self, Self::Error> {
        let webhook_event = serde_json::from_slice(&req.webhook_body)
            .change_context(IntegrationError::not_implemented("webhook body decoding failed".to_string()))?;
        let headers = webhook_headers_lowercase(&req.webhook_headers);
        Ok(Self {
            transmission_id: headers
                .get(webhook_headers::PAYPAL_TRANSMISSION_ID)
                .ok_or(IntegrationError::MissingRequiredField { field_name: webhook_headers::PAYPAL_TRANSMISSION_ID, context: Default::default() })?
                .clone(),
            transmission_time: headers.get(webhook_headers::PAYPAL_TRANSMISSION_TIME).cloned().ok_or(/* … */)?,
            cert_url:          headers.get(webhook_headers::PAYPAL_CERT_URL).cloned().ok_or(/* … */)?,
            transmission_sig:  headers.get(webhook_headers::PAYPAL_TRANSMISSION_SIG).cloned().ok_or(/* … */)?,
            auth_algo:         headers.get(webhook_headers::PAYPAL_AUTH_ALGO).cloned().ok_or(/* … */)?,
            webhook_id: String::from_utf8(req.merchant_secret.secret.to_vec())?,
            webhook_event,
        })
    }
}
```

### Example 3 — Truelayer JKU allow-list and JWS routing

```rust
// From crates/integrations/connector-integration/src/connectors/truelayer.rs:890
fn get_url(&self, req: &RouterDataV2<VerifyWebhookSource, …>) -> CustomResult<String, IntegrationError> {
    let tl_signature_header = req.request.webhook_headers.get("tl-signature")
        .ok_or(IntegrationError::MissingRequiredField { field_name: "tl-signature", context: Default::default() })?;
    let parts: Vec<&str> = tl_signature_header.as_str().splitn(3, '.').collect();
    let header_b64 = parts.first().ok_or(IntegrationError::InvalidDataFormat { field_name: "tl-signature", context: Default::default() })?;
    let header_json = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(header_b64)
        .change_context(IntegrationError::InvalidDataFormat { field_name: "tl-signature", context: Default::default() })?;
    let jws_header: truelayer::JwsHeaderWebhooks = serde_json::from_slice(&header_json)?;
    let jku = jws_header.jku.ok_or(IntegrationError::MissingRequiredField { field_name: "jku", context: Default::default() })?;
    if truelayer::ALLOWED_JKUS.contains(&jku.as_str()) {
        Ok(jku)
    } else {
        Err(IntegrationError::InvalidDataFormat { field_name: "jku", context: Default::default() }.into())
    }
}
```

### Example 4 — Bluesnap in-band HMAC-SHA256 (canonical shape)

```rust
// From crates/integrations/connector-integration/src/connectors/bluesnap.rs:216
fn verify_webhook_source(
    &self,
    request: RequestDetails,
    connector_webhook_secret: Option<ConnectorWebhookSecrets>,
    _connector_account_details: Option<ConnectorSpecificConfig>,
) -> CustomResult<bool, WebhookError> {
    let secret = connector_webhook_secret
        .ok_or_else(|| report!(WebhookError::WebhookVerificationSecretNotFound))
        .attach_printable("Connector webhook secret not configured")?;
    let signature = self.get_webhook_source_verification_signature(&request, &secret)?;
    let message   = self.get_webhook_source_verification_message(&request, &secret)?;
    use common_utils::crypto::{HmacSha256, SignMessage};
    HmacSha256.sign_message(&secret.secret, &message)
        .change_context(WebhookError::WebhookSourceVerificationFailed)
        .attach_printable("Failed to sign webhook message with HMAC-SHA256")
        .map(|expected| expected.eq(&signature))
}
```

### Example 5 — Out-of-band router-data construction (grpc-server)

```rust
// From crates/grpc-server/grpc-server/src/server/events.rs:167
async fn verify_webhook_source_external(
    config: &Config,
    connector_data: &ConnectorData<DefaultPCIHolder>,
    request_details: &RequestDetails,
    webhook_secrets: Option<ConnectorWebhookSecrets>,
    connector_config: &ConnectorSpecificConfig,
    metadata_payload: &utils::MetadataPayload,
    service_name: &str,
) -> Result<bool, tonic::Status> {
    let verify_webhook_flow_data = VerifyWebhookSourceFlowData {
        connectors: config.connectors.clone(),
        connector_request_reference_id: format!("webhook_verify_{}", metadata_payload.request_id),
        raw_connector_response: None,
        raw_connector_request: None,
        connector_response_headers: None,
    };
    let merchant_secret = webhook_secrets.unwrap_or_else(|| ConnectorWebhookSecrets {
        secret: "default_secret".to_string().into_bytes(),
        additional_secret: None,
    });
    let verify_webhook_request = VerifyWebhookSourceRequestData {
        webhook_headers: request_details.headers.clone(),
        webhook_body:    request_details.body.clone(),
        merchant_secret,
        webhook_uri:     request_details.uri.clone(),
    };
    let verify_webhook_router_data = RouterDataV2::<VerifyWebhookSource, VerifyWebhookSourceFlowData, VerifyWebhookSourceRequestData, VerifyWebhookSourceResponseData> {
        flow: std::marker::PhantomData,
        resource_common_data: verify_webhook_flow_data,
        connector_config: connector_config.clone(),
        request: verify_webhook_request,
        response: Err(ErrorResponse::default()),
    };
    /* … dispatched via connector.get_connector_integration_v2() … */
}
```

## Integration Guidelines

Ordered steps for implementing `VerifyWebhookSource` for a new connector.

1. **Decide which family.** If the connector publishes an HMAC/SHA/MD5 algorithm + shared secret + header, you want Family 1 (in-band). If it requires fetching a JWKS or posting to a verification endpoint, you want Family 2 (out-of-band).
2. **Add the connector to `default_impl_verify_webhook_source_v2!`** if Family 1 (`default_implementations.rs:50-127`). This provides the empty `ConnectorIntegrationV2` impl you still need for trait-object dispatch. Skip this step for Family 2 — write your own impl and keep the connector out of the macro list.
3. **Implement `IncomingWebhook::verify_webhook_source`** (Family 1) in `src/connectors/<connector>.rs`. Return `Ok(true)` when signature matches, `Ok(false)` when it does not. Use `report!(WebhookError::WebhookVerificationSecretNotFound)` when `connector_webhook_secret` is `None` **only if** the connector actually requires a secret. Compare by equality of `Vec<u8>`, never by string comparison of hex.
4. **Implement `get_webhook_source_verification_signature`** to extract + decode the signature. Most connectors hex-decode a header (`bluesnap.rs:246-251`); some read a JSON body field (Trustpay pattern in `pattern_IncomingWebhook_flow.md` §Pattern 3). Return `WebhookError::WebhookSignatureNotFound` if the header/field is absent.
5. **Implement `get_webhook_source_verification_message`** to build the canonical message bytes the connector signed. This is where most bugs hide — match the connector's documentation byte-for-byte (line endings, delimiters, URL-encoded field values). For Family 2, this step is irrelevant: the connector does the math.
6. **Choose the algorithm.** Import from `common_utils::crypto`: `HmacSha256`, `HmacSha512`, `Sha256` (plain), or `Md5`. Prefer HMAC over plain digest. For new integrations **do not** choose MD5.
7. **(Family 2 only) Implement `ConnectorIntegrationV2<VerifyWebhookSource, …>`** with `get_url`, `get_headers`, `get_request_body`, `handle_response_v2`. `get_http_method` defaults to POST; override to GET for JWKS fetch (see `truelayer.rs:886-888`).
8. **(Family 2 only) Implement `TryFrom<&VerifyWebhookSourceRequestData> for <Connector>VerificationRequest`** (`paypal/transformers.rs:3195`), and `TryFrom<ResponseRouterData<<Connector>VerificationResponse, Self>>` that maps to `VerifyWebhookSourceResponseData { verify_webhook_status: … }` (`paypal/transformers.rs:3263`).
9. **(Family 2 only) Register the connector** in `config.webhook_source_verification_call.connectors_with_webhook_source_verification_call`. Without this, the gRPC path at `events.rs:111` takes the in-band branch and your out-of-band impl is never invoked.
10. **Implement `connector_types::VerifyWebhookSourceV2`** as a marker impl for Family 2 connectors (`paypal.rs:1599-1601`, `truelayer.rs:873-876`). Family 1 connectors get this for free via the default-impl macro.
11. **Wire the rest of IncomingWebhook.** Even for Family 2, you still implement `get_event_type`, `process_payment_webhook`, `process_refund_webhook`, `process_dispute_webhook` from `IncomingWebhook` — the verification flow runs *before* those (`events.rs:143-149`). See `pattern_IncomingWebhook_flow.md` for the non-verification pieces.
12. **Document the signature scheme** inline with a `// Scheme: HMAC-SHA256 over `{timestamp}{body}`, header `bls-signature`, hex-encoded` comment directly above the impl.

## Best Practices

- **Never modify the body bytes before HMAC.** Keep `VerifyWebhookSourceRequestData.webhook_body` as the unparsed `Vec<u8>`; parsing and re-serializing will change whitespace, field order, or Unicode escapes and break verification. Bluesnap does this correctly: `body_str = String::from_utf8_lossy(&request.body)` in `bluesnap.rs:264` is only for logging/concatenation after signature math.
- **Lowercase header lookups for HTTP header names.** HTTP headers are case-insensitive per RFC 7230; PayPal's `webhook_headers_lowercase` at `paypal/transformers.rs:3185-3192` is the reference pattern. Bluesnap's `.get("bls-signature")` only works because the gRPC intake normalizes keys upstream.
- **Decode signatures before byte comparison.** `hex::decode` (Bluesnap at `bluesnap.rs:251`) or `base64` (Adyen converts in the reverse direction at `adyen.rs:796-804`). Never compare hex strings directly: different casing will falsely fail.
- **Use `verify_signature` when available.** `common_utils::crypto::VerifySignature::verify_signature` implements constant-time comparison. Explicit `.eq(&signature)` after `sign_message` is acceptable for `Vec<u8>`-of-equal-length since the built-in `PartialEq` short-circuits but is typically acceptable given the inputs are already same-length digests. Prefer `verify_signature` unless you need to log the computed signature (see Adyen at `adyen.rs:791-804` for a case where logging required `sign_message`).
- **Reject missing webhook secret with a real error for Family 2**, but consider `Ok(false)` for Family 1 connectors that log-and-continue (Novalnet pattern at `novalnet.rs:694-700`). The trait's default `Ok(false)` is explicitly designed to be non-fatal (`connector_types.rs:381`).
- **Keep the out-of-band list short.** Only PayPal is on the external list at the pinned SHA. Every new entry adds an HTTP round-trip to *every* webhook; prefer in-band where possible.
- **Allow-list JKU-style URLs.** When fetching keys from a connector-controlled URL, validate it against a fixed allow-list (`truelayer::ALLOWED_JKUS` at `truelayer/transformers.rs:1090`) to prevent SSRF/key-substitution.
- **Normalize but preserve order for JSON-signed payloads.** PayPal's `preserve_order` serde feature at `paypal/transformers.rs:3198-3199` is load-bearing: reordering JSON keys breaks PayPal's cert-based verification.
- **Re-export the scheme in a dedicated `// Scheme: …` comment**, not in prose; reviewers should be able to determine the crypto at a glance.

## Common Errors / Gotchas

1. **Signature-extraction returns the wrong bytes.**
   - **Problem**: Using `hex::decode` on a base64-encoded signature, or vice versa. `adyen.rs:796-804` deliberately compares *base64 strings*, not bytes, because Adyen encodes the signature in base64; connectors that hex-encode (Bluesnap, Ppro, Authorizedotnet) must `hex::decode` first.
   - **Solution**: Read the connector docs for the signature encoding. When in doubt, `println!` the header once in a dev harness to see if it matches `/^[0-9a-f]+$/` (hex) or has `+/=` characters (base64).

2. **Message construction doesn't byte-match connector's string.**
   - **Problem**: Off-by-one in delimiters (colon vs. dot vs. newline), missing timestamp, truncated body, or UTF-8 re-encoding that replaces invalid sequences with `U+FFFD` (`String::from_utf8_lossy` at `bluesnap.rs:264`).
   - **Solution**: For binary-safe algorithms, work at the `&[u8]` level, not `String`. For connectors that require string concatenation (Bluesnap: `{timestamp}{body}`), verify that the body is guaranteed UTF-8 (most JSON webhooks are) or switch to `format!("{timestamp}{body}").as_bytes()` via raw byte concatenation. Test against connector-provided sample payloads.

3. **PayPal Basic Auth vs. Bearer Token mix-up.**
   - **Problem**: PayPal's standard API uses OAuth Bearer tokens, but the webhook-verification endpoint requires Basic Auth with `client_id:client_secret`. Using the default header builder produces a 401.
   - **Solution**: Override `get_headers` for the `VerifyWebhookSource` impl only (`paypal.rs:1516-1532`); do not touch the headers used by other flows.

4. **Truelayer JKU attacker-choice.**
   - **Problem**: A malicious sender could craft a `tl-signature` whose `jku` points at an attacker-controlled JWKS, fooling the connector into verifying against attacker-chosen keys.
   - **Solution**: Allow-list `jku` against `truelayer::ALLOWED_JKUS` before making the network call (`truelayer.rs:931-940`). Never trust an unverified `jku` URL from a webhook header.

5. **Secret is the webhook id, not a shared key (PayPal).**
   - **Problem**: For PayPal, `ConnectorWebhookSecrets.secret` holds the webhook id string, not HMAC bytes. HMAC-style in-band verification with this secret will always fail.
   - **Solution**: PayPal must use the out-of-band flow. The secret is UTF-8-decoded into `webhook_id` inside the request builder (`paypal/transformers.rs:3244-3248`) and echoed back to PayPal.

6. **Missing webhook-uri breaks URI-signing schemes.**
   - **Problem**: `VerifyWebhookSourceRequestData.webhook_uri` is `Option<String>` (`router_request_types.rs:470`). For TrueLayer, signing input includes the URI path; an absent URI causes `IntegrationError::MissingRequiredField { field_name: "webhook_uri" }` (`truelayer/transformers.rs:1292-1297`).
   - **Solution**: The gRPC intake populates `webhook_uri` from `request_details.uri` (`events.rs:194`). Ensure upstream code (SDK-side or gRPC handler) forwards the full webhook URL.

7. **Forgetting to register external-verification connectors.**
   - **Problem**: A Family-2 connector with a real `ConnectorIntegrationV2<VerifyWebhookSource, …>` impl is never invoked because `requires_external_webhook_verification` returns `false` by default.
   - **Solution**: Add the connector to `config.webhook_source_verification_call.connectors_with_webhook_source_verification_call`. The selector is at `events.rs:104-109` and reads the config at runtime — no code change to the selector is required.

8. **Duplicate `ConnectorIntegrationV2<VerifyWebhookSource, …>` impl from the macro.**
   - **Problem**: If you write a real impl *and* leave the connector inside the `default_impl_verify_webhook_source_v2!` list, you will get a `conflicting implementations` compile error.
   - **Solution**: Remove the connector from the macro invocation at `default_implementations.rs:50-127`. PayPal is explicitly commented-out per `:128-129`; do the same for your connector.

9. **Hardcoded `true`/`false` in `verify_webhook_source`.**
   - **Problem**: Returning `Ok(true)` unconditionally defeats the purpose of verification; returning `Ok(false)` unconditionally makes every valid webhook silently drop to the PSync fallback.
   - **Solution**: Always compute the real signature comparison. If the connector has no webhook-signing mechanism, leave the trait default (`connector_types.rs:381`: `Ok(false)`) and rely on PSync fallback rather than faking a verification.

10. **Cloning the response-router-data mutates the flow-data.**
    - **Problem**: In `handle_response_v2`, replacing `response` on a `data.clone()` loses any mutations to `resource_common_data` performed upstream.
    - **Solution**: Follow the PayPal pattern (`paypal/transformers.rs:3276-3281`): `Ok(Self { response: Ok(VerifyWebhookSourceResponseData { … }), ..item.router_data })`. The `..item.router_data` preserves flow-data intact.

## Testing Notes

### Unit-test shape

For **in-band** connectors, unit tests live inline with the connector and hit `verify_webhook_source` directly with a `RequestDetails` constructed from known-good fixtures. Example shape (see `ppro/test.rs:376` for the `HmacSha256::sign_message` production-side usage):

```rust
#[test]
fn test_bluesnap_webhook_verification() {
    let connector = Bluesnap::<DefaultPCIHolder>::new();
    let body = b"{\"event\":\"payment.success\"}";
    let timestamp = "1700000000";
    let secret = b"test-shared-secret";

    // Compute the expected signature using the same primitive as the impl.
    let expected = HmacSha256.sign_message(secret, format!("{timestamp}{body_str}").as_bytes()).unwrap();
    let signature_hex = hex::encode(&expected);

    let req = RequestDetails {
        headers: HashMap::from([
            ("bls-signature".into(), signature_hex),
            ("bls-ipn-timestamp".into(), timestamp.into()),
        ]),
        body: body.to_vec(),
        method: "POST".into(),
        url: "/webhooks".into(),
    };
    let secrets = ConnectorWebhookSecrets { secret: secret.to_vec(), additional_secret: None };
    assert!(connector.verify_webhook_source(req, Some(secrets), None).unwrap());
}
```

For **out-of-band** connectors, unit-test the `TryFrom<&VerifyWebhookSourceRequestData> for <Connector>VerificationRequest` conversion (header extraction) and the `TryFrom<ResponseRouterData<_>>` mapping (verification-status → `VerifyWebhookStatus`). A network-free test of `get_url` and `get_headers` is typically enough; the HTTP round-trip belongs in integration tests.

### Integration-test scenarios

| Scenario | In-band expectation | Out-of-band expectation |
|----------|---------------------|--------------------------|
| Valid signature + correct secret | `Ok(true)` → `SourceVerified` | HTTP 200 → `verification_status: SUCCESS` → `SourceVerified` |
| Invalid signature | `Ok(false)` → `SourceNotVerified` | HTTP 200 → `verification_status: FAILURE` → `SourceNotVerified` |
| Missing signature header | `Err(WebhookSignatureNotFound)` | `Err(MissingRequiredField { field_name })` from request-builder |
| Missing webhook secret | `Err(WebhookVerificationSecretNotFound)` (or `Ok(false)` with warn log, see Novalnet pattern at `novalnet.rs:694-700`) | Request builder still fires with a default secret (`events.rs:185-188`) |
| Tampered body (one byte changed) | `Ok(false)` | Connector returns FAILURE |
| Unknown JKU (Truelayer only) | N/A | `Err(InvalidDataFormat { field_name: "jku" })` from `get_url` (`truelayer.rs:934-938`) |
| Connector endpoint 5xx (out-of-band only) | N/A | `verify_webhook_source_external` returns `Ok(false)` with warn log (`events.rs:262-270`) |

## Cross-References

- Parent index: [../README.md](./README.md)
- Spec: [PATTERN_AUTHORING_SPEC.md](./PATTERN_AUTHORING_SPEC.md) — authoring rules this file follows.
- Sibling flow (webhook pipeline after verification): [pattern_IncomingWebhook_flow.md](./pattern_IncomingWebhook_flow.md)
- Sibling flow (signature-related, request-signing context): [pattern_authorize.md](./pattern_authorize.md)
- Sibling flow (integrity-object comparison reference): [pattern_capture.md](./pattern_capture.md)
- Types reference: [../types/types.md](../types/types.md) — `VerifyWebhookSourceRequestData`, `VerifyWebhookSourceResponseData`, `ConnectorWebhookSecrets`, `WebhookError`.
- Utility functions: [../utility_functions_reference.md](../utility_functions_reference.md) — `common_utils::crypto::{HmacSha256, HmacSha512, Sha256, Md5}` primitives.
