# Wallet Authorize Flow Pattern — Network Transaction ID (NTID) Sub-Pattern (Decrypted Wallet Token)

## Overview

This sub-pattern documents the **Merchant-Initiated Transaction (MIT)** path that reuses a **Network Transaction ID (NTID)** from a prior **Customer-Initiated Transaction (CIT)** when the underlying payment credential is a **decrypted wallet token** (Apple Pay or Google Pay). It is a variant of the parent **Wallet** PM (see [`pattern_authorize_wallet.md`](./pattern_authorize_wallet.md)) and composes with the **RepeatPayment** flow (see [`pattern_repeat_payment_flow.md`](../../pattern_repeat_payment_flow.md)).

Concretely, a connector enters this sub-pattern when all three of the following hold in the incoming `RouterDataV2`:

1. `request.mandate_reference == MandateReferenceId::NetworkMandateId(network_transaction_id)` — the orchestrator has routed a stored scheme NTI rather than a connector-scoped mandate id.
2. `request.payment_method_data == PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(data)` — i.e. the already-decrypted DPAN (network token) derived from the wallet's original tokenized CIT is replayed alongside the NTI.
3. `data.token_source` identifies the original wallet (`TokenSource::ApplePay` or `TokenSource::GooglePay`) so the connector can signal the correct wallet brand to the scheme.

Unlike the **Card NTID** sub-pattern (see [sibling pattern](../card/pattern_authorize_card_ntid.md)), the credential transmitted is **not** the raw PAN — it is a network-token DPAN minted by the scheme's token service during the wallet's original decryption. There is no CVV (wallet tokens never have one), no 3DS (MIT is out of scope for challenge), no cardholder cryptogram (the original cryptogram from the CIT is single-use and was consumed at that time), and the customer is not present.

This sub-pattern **does not replace** the parent Wallet pattern; it augments it. Status mapping, macro wiring, and the canonical `RouterDataV2<Flow, FlowData, Req, Res>` signatures from the parent Wallet pattern and from [`PATTERN_AUTHORING_SPEC.md`](../../PATTERN_AUTHORING_SPEC.md) §7 still apply. The only things this sub-pattern governs are the match arm on the NTID variant, the mapping of `TokenSource` to the outgoing wallet-brand code, and the MIT-signalling on the outgoing body.

### Key Characteristics

| Characteristic | Value | Citation |
|----------------|-------|----------|
| Triggering enum | `PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId` | `crates/types-traits/domain_types/src/payment_method_data.rs:251-253` |
| Paired mandate variant | `MandateReferenceId::NetworkMandateId(String)` | observed in `checkout/transformers.rs:898, 936` |
| Underlying credential | `cards::NetworkToken` (DPAN) — **not** raw PAN | `payment_method_data.rs:1342` |
| `TokenSource` | `GooglePay` \| `ApplePay` | `payment_method_data.rs:1351-1354` |
| CVV? | No — wallet tokens do not carry CVV | `payment_method_data.rs:1341-1348` (struct has no cvc field) |
| 3DS performed? | No — authentication already happened on CIT | — |
| Cryptogram? | No — original cryptogram is single-use and was consumed during the CIT | `payment_method_data.rs:1341-1348` (no cryptogram field) |
| Whether flow is `Authorize` or `RepeatPayment` | `RepeatPayment` in the only known impl (Checkout) | `checkout.rs:240-244` |

---

## Table of Contents

1. [Relationship to SetupMandate and RepeatPayment flows](#relationship-to-setupmandate-and-repeatpayment-flows)
2. [Variant Enumeration](#variant-enumeration)
3. [Field Enumeration](#field-enumeration)
4. [Architecture Overview](#architecture-overview)
5. [Connectors with Full Implementation](#connectors-with-full-implementation)
6. [Per-Variant Implementation Notes](#per-variant-implementation-notes)
7. [Common Implementation Patterns](#common-implementation-patterns)
8. [Code Examples](#code-examples)
9. [Best Practices](#best-practices)
10. [Common Errors](#common-errors)
11. [Cross-References](#cross-references)

---

## Relationship to SetupMandate and RepeatPayment flows

The wallet-NTID sub-pattern sits at the end of a three-flow lifecycle very similar to the card-NTID lifecycle, but with an extra layer of indirection at the CIT: the original wallet payload (Apple Pay PKPaymentToken or Google Pay EncryptedPaymentToken) is decrypted by UCS into a `NetworkToken` (DPAN) **before** the CIT call reaches the connector, so even the CIT does not see raw wallet plaintext.

### Sequence (ASCII)

```
                 CIT (one-time)                           MIT (one or many)
                 ───────────────                          ───────────────────

┌─────────────┐                                ┌──────────────────────────────┐
│ Merchant    │                                │ Merchant                     │
│ app (CIT)   │                                │ app (MIT scheduler /         │
│ Apple/Google│                                │ recurring job)               │
│ Pay sheet   │                                └────────┬─────────────────────┘
└─────┬───────┘                                         │ (5) RepeatPaymentData
      │ (1) PKPaymentToken                              │     mandate_reference =
      │     / EncryptedGPayToken                        │     NetworkMandateId(NTI)
      ▼                                                 │     payment_method_data =
┌───────────────────┐                                   │     DecryptedWalletToken…
│ UCS wallet decrypt│                                   │                ForNTI
│ (ApplePay / GPay) │                                   │     token_source =
└─────┬─────────────┘                                   │       Apple|GooglePay
      │ (2) DecryptedToken (DPAN + exp + cryptogram)    ▼
      │     + PaymentMethodData::Wallet(…)        ┌─────────────────────────┐
      ▼                                           │ RouterDataV2<RepeatPayment>
┌─────────────┐   (3) ProcessorResponse           └─────┬───────────────────┘
│ RouterDataV2│   .network_transaction_id ──┐           │
│ <Authorize> │                             │           ▼
│ or          │                             │     ┌──────────────────────────┐
│ <SetupMand> │                             │     │ Connector MIT call       │
└─────┬───────┘                             │     │ decrypted DPAN carried   │
      │                                     │     │ but NO cryptogram,       │
      │ (3') OR zero-dollar SetupMandate    │     │ NO CVV, NO 3DS,          │
      │                                     │     │ wallet-brand from        │
      ▼                                     │     │   TokenSource,           │
┌─────────────┐                             │     │ MIT indicator set,       │
│ Connector   │                             │     │ NTI attached as          │
│ CIT call    │                             │     │   previous_payment_id /  │
│ (cryptogram │                             │     │   equivalent             │
│  consumed)  │                             │     └─────┬────────────────────┘
└─────┬───────┘                             │           │ (6) PaymentsResponseData
      │                                     │           ▼
      ▼                                     │
┌─────────────┐                             │
│ UCS store:  │ ◀───────────────────────────┘
│ {mandate_id,│
│  NTI}       │
└─────────────┘
```

- Step **(1)–(2)** is wallet decryption. The pre-existing UCS wallet layer turns the opaque Apple Pay or Google Pay blob into a `NetworkToken` (DPAN) plus expiry, cryptogram, and `eci`. The **cryptogram is consumed** during the CIT — it cannot be replayed.
- Step **(3)** or **(3')** is the CIT. The connector sees a regular tokenized wallet body; its `network_transaction_id` is extracted from the processor response. This CIT happens through the parent Wallet pattern's [token-based wallet path](./pattern_authorize_wallet.md).
- Step **(4)** — not drawn — is UCS storing the NTI.
- Step **(5)** is this sub-pattern. The orchestrator re-materializes the DPAN + expiry + `token_source` into a `DecryptedWalletTokenDetailsForNetworkTransactionId` and attaches it with `MandateReferenceId::NetworkMandateId(nti)` into the next MIT RouterData.
- Step **(6)** is the MIT response. Status mapping is unchanged from the parent Wallet pattern.

### Which flow consumes the NTID?

The only connector in the codebase that materially handles `DecryptedWalletTokenDetailsForNetworkTransactionId` is **Checkout**, wired into the **RepeatPayment** flow (`checkout.rs:240-244`, dispatch at `checkout/transformers.rs:898, 936`). Other connectors either match the variant in a `NotImplemented` fall-through or do not match it at all.

The analogous card flow covers more connectors and may be wired into either Authorize or RepeatPayment — see [`../card/pattern_authorize_card_ntid.md`](../card/pattern_authorize_card_ntid.md) for that broader spread.

---

## Variant Enumeration

This sub-pattern qualifies a **single** variant of `PaymentMethodData<T>`, declared at `crates/types-traits/domain_types/src/payment_method_data.rs:251-253`:

| Variant | Data Shape | Citation | Used By (connectors) |
|---------|-----------|----------|----------------------|
| `PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId` | `DecryptedWalletTokenDetailsForNetworkTransactionId` struct | `payment_method_data.rs:1340-1348` | checkout (only) |

There is no alternate variant for the decrypted-wallet NTID path. All other connectors that reference the variant name do so in a `NotImplemented` fall-through (see [Stub Implementations](#stub-implementations) below).

### Paired enum: `TokenSource`

```rust
// From crates/types-traits/domain_types/src/payment_method_data.rs:1350-1354
#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum TokenSource {
    GooglePay,
    ApplePay,
}
```

| TokenSource variant | Citation |
|---------------------|----------|
| `GooglePay` | `payment_method_data.rs:1352` |
| `ApplePay` | `payment_method_data.rs:1353` |

Connectors MUST map this to their wallet-brand code. Checkout uses string `"googlepay"` / `"applepay"` at `checkout/transformers.rs:952-962`. Missing `token_source` (None) is treated as a hard error at `checkout/transformers.rs:959-962`.

---

## Field Enumeration

Definition at `crates/types-traits/domain_types/src/payment_method_data.rs:1340-1348`:

```rust
// From crates/types-traits/domain_types/src/payment_method_data.rs:1340
#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize, Default)]
pub struct DecryptedWalletTokenDetailsForNetworkTransactionId {
    pub decrypted_token: cards::NetworkToken,
    pub token_exp_month: Secret<String>,
    pub token_exp_year: Secret<String>,
    pub card_holder_name: Option<Secret<String>>,
    pub eci: Option<String>,
    pub token_source: Option<TokenSource>,
}
```

| Field | Type | Required? | Notes | Citation |
|-------|------|-----------|-------|----------|
| `decrypted_token` | `cards::NetworkToken` | yes | The DPAN — scheme-issued network token, not the raw PAN. Type is `cards::NetworkToken`, distinct from `cards::CardNumber` used in `CardDetailsForNetworkTransactionId`. | `payment_method_data.rs:1342` |
| `token_exp_month` | `Secret<String>` | yes | Token expiry month (two digits). | `payment_method_data.rs:1343` |
| `token_exp_year` | `Secret<String>` | yes | Token expiry year (two or four digits). | `payment_method_data.rs:1344` |
| `card_holder_name` | `Option<Secret<String>>` | no | Not always populated — wallet providers do not always expose cardholder name via their token API. | `payment_method_data.rs:1345` |
| `eci` | `Option<String>` | no | Electronic Commerce Indicator from the original CIT decryption. Usually **not** replayed on MIT because schemes consider the MIT a separate context, but the field is preserved for connectors that demand it. | `payment_method_data.rs:1346` |
| `token_source` | `Option<TokenSource>` | yes-in-practice | `Some(GooglePay)` or `Some(ApplePay)`. Checkout treats `None` as a hard `MissingRequiredField` error at `checkout/transformers.rs:959-962`. | `payment_method_data.rs:1347` |

### Notable absences

Compared to the CIT-side wallet token shapes (e.g. `GooglePayWalletData`, `ApplePayPredecryptData`), this struct deliberately **omits**:

- `tokenization_data` / raw token blob — already consumed during CIT decryption.
- `cryptogram` — single-use at CIT; replaying produces scheme-level decline.
- `message_expiration` — cryptogram-scoped, so irrelevant here.
- `assurance_details` (Google Pay) — only meaningful at CIT.

Implementers MUST NOT invent placeholder values for any of these fields — the scheme's MIT-via-NTI path is specifically designed to function without them.

### Helper methods

The impl block at `crates/types-traits/domain_types/src/payment_method_data.rs:1356-1436` provides:

| Method | Returns | Citation |
|--------|---------|----------|
| `get_card_expiry_year_2_digit()` | `Result<Secret<String>, IntegrationError>` — last 2 digits of `token_exp_year` | `payment_method_data.rs:1357-1371` |
| `get_card_issuer()` | `Result<CardIssuer, Error>` — via `get_card_issuer(decrypted_token)` | `payment_method_data.rs:1372-1374` |
| `get_card_expiry_month_year_2_digit_with_delimiter(delim)` | `Result<Secret<String>, _>` — e.g. `"12/25"` | `payment_method_data.rs:1375-1383` |
| `get_expiry_date_as_yyyymm(delim)` | `Secret<String>` — e.g. `"2025-12"` | `payment_method_data.rs:1384-1389` |
| `get_expiry_date_as_mmyyyy(delim)` | `Secret<String>` — e.g. `"12/2025"` | `payment_method_data.rs:1390-1395` |
| `get_expiry_year_4_digit()` | `Secret<String>` — upgrades `"25"` to `"2025"` | `payment_method_data.rs:1396-1402` |
| `get_expiry_date_as_yymm()` | `Result<Secret<String>, _>` — e.g. `"2512"` | `payment_method_data.rs:1403-1407` |
| `get_expiry_month_as_i8()` | `Result<Secret<i8>, Error>` | `payment_method_data.rs:1408-1421` |
| `get_expiry_year_as_i32()` | `Result<Secret<i32>, Error>` | `payment_method_data.rs:1422-1435` |

These mirror the helper set on `CardDetailsForNetworkTransactionId` exactly (minus `get_expiry_date_as_mmyy`). Implementers MUST use these helpers rather than reformatting expiry dates inline.

---

## Architecture Overview

### Request / Response types

The wallet-NTID sub-pattern does not introduce new `RouterDataV2` type arguments. It reuses the RepeatPayment tuple from [`PATTERN_AUTHORING_SPEC.md`](../../PATTERN_AUTHORING_SPEC.md) §7:

```rust
RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
```

The implementing trait is `ConnectorIntegrationV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>` — inherited unchanged from the RepeatPayment flow pattern (see [`../../pattern_repeat_payment_flow.md`](../../pattern_repeat_payment_flow.md)).

No Authorize-wiring of this variant is observed in the codebase at the pinned SHA.

### Where the variant is unwrapped

Observed in Checkout at `checkout/transformers.rs:898, 936-985`:

```rust
// From crates/integrations/connector-integration/src/connectors/checkout/transformers.rs:898-985 (abbreviated)
match &item.router_data.request.mandate_reference {
    MandateReferenceId::NetworkMandateId(network_transaction_id) => {
        match item.router_data.request.payment_method_data {
            PaymentMethodData::CardDetailsForNetworkTransactionId(ref card_details) => { /* card NTID */ }
            PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(
                ref network_token_data,
            ) => {
                // map token_source → "applepay" / "googlepay"
                let token_type = match network_token_data.token_source {
                    Some(TokenSource::ApplePay)  => "applepay".to_string(),
                    Some(TokenSource::GooglePay) => "googlepay".to_string(),
                    None => Err(IntegrationError::MissingRequiredField {
                        field_name: "token_source",
                        context: Default::default(),
                    })?,
                };
                let exp_month = network_token_data.token_exp_month.clone();
                let expiry_year_4_digit = network_token_data.get_expiry_year_4_digit();
                let payment_source = PaymentSource::DecryptedWalletToken(DecryptedWalletToken {
                    token: network_token_data.decrypted_token.clone(),
                    decrypt_type: "network_token".to_string(),
                    token_type,
                    expiry_month: exp_month,
                    expiry_year: expiry_year_4_digit,
                    billing_address: billing_details,
                });
                // previous_payment_id = NTI, merchant_initiated = Some(true)
                Ok((payment_source, Some(network_transaction_id.clone()), Some(true), p_type, None))
            }
            _ => Err(IntegrationError::not_implemented(
                utils::get_unimplemented_payment_method_error_message("checkout"),
            )),
        }
    }
    // ...
}
```

Always nest the match on `MandateReferenceId` first, then destructure on `PaymentMethodData` — the same ordering as the card sibling ([`../card/pattern_authorize_card_ntid.md`](../card/pattern_authorize_card_ntid.md)).

### MIT-signalling on the outgoing body

Checkout emits the same MIT signals as its card-NTID path:

| Field | Value | Citation |
|-------|-------|----------|
| `previous_payment_id` | `Some(network_transaction_id.clone())` | `checkout/transformers.rs:980` |
| `merchant_initiated` | `Some(true)` | `checkout/transformers.rs:981` |
| `payment_type` | `Unscheduled` / `Recurring` / `Installment` (from `RepeatPaymentData.mit_category`) | `checkout/transformers.rs:939-950` |
| `PaymentSource` variant | `PaymentSource::DecryptedWalletToken(DecryptedWalletToken)` | `checkout/transformers.rs:968-976` |
| `DecryptedWalletToken.decrypt_type` | `"network_token"` | `checkout/transformers.rs:971` |
| `DecryptedWalletToken.token_type` | `"applepay"` or `"googlepay"` from `TokenSource` | `checkout/transformers.rs:952-963` |

All six are required for the MIT-via-NTI wallet body to pass scheme validation; omitting any one produces either a soft-decline or a schema-validation error at Checkout's `/payments` endpoint.

---

## Connectors with Full Implementation

Only connectors that **materially construct** a request body from `DecryptedWalletTokenDetailsForNetworkTransactionId` are listed.

| Connector | HTTP Method | Content Type | URL Pattern | Request Type Reuse | Notes |
|-----------|-------------|--------------|-------------|--------------------|-------|
| **Checkout** | POST | `application/json` | `payments` (same endpoint as Authorize) | Reuses `PaymentsRequest<T>`; source variant is `PaymentSource::DecryptedWalletToken(DecryptedWalletToken)` | See `crates/integrations/connector-integration/src/connectors/checkout/transformers.rs:936-985`. Wired into **RepeatPayment** flow at `checkout.rs:240-244`. `decrypt_type = "network_token"` at `checkout/transformers.rs:971`. `token_type` mapped from `TokenSource` at `checkout/transformers.rs:952-963`. `previous_payment_id = Some(nti)` at `checkout/transformers.rs:980`. `merchant_initiated = Some(true)` at `checkout/transformers.rs:981`. Expiry uses `token_exp_month` (verbatim) + `get_expiry_year_4_digit()` at `checkout/transformers.rs:965-966`. |

### Stub Implementations

Connectors that reference `DecryptedWalletTokenDetailsForNetworkTransactionId` **only** inside a `NotImplemented` / `NotSupported` fall-through (i.e. they compile but refuse the variant at runtime):

- aci (`aci/transformers.rs:750`)
- adyen (`adyen/transformers.rs:3703`, `adyen/transformers.rs:6044`) — note that Adyen handles the **card-NTID** variant but declines the wallet-NTID variant explicitly at `adyen/transformers.rs:6399-6405`
- bambora (`bambora/transformers.rs:299`)
- bankofamerica (`bankofamerica/transformers.rs:616`)
- billwerk (`billwerk/transformers.rs:236`)
- braintree (`braintree/transformers.rs:613`, `braintree/transformers.rs:1610`, `braintree/transformers.rs:2632`, `braintree/transformers.rs:2815`)
- cryptopay (`cryptopay/transformers.rs:112`)
- cybersource (`cybersource/transformers.rs:324`, `cybersource/transformers.rs:2289`, `cybersource/transformers.rs:3028`, `cybersource/transformers.rs:3305`, `cybersource/transformers.rs:4324`) — Cybersource supports card-NTID but not wallet-NTID.
- dlocal (`dlocal/transformers.rs:210`)
- fiserv (`fiserv/transformers.rs:551`)
- fiuu (`fiuu/transformers.rs:675`)
- forte (`forte/transformers.rs:314`)
- hipay (`hipay/transformers.rs:597`)
- loonio (`loonio/transformers.rs:243`)
- mifinity (`mifinity/transformers.rs:250`)
- multisafepay (`multisafepay/transformers.rs:158`, `multisafepay/transformers.rs:338`)
- nexinets (`nexinets/transformers.rs:742`)
- novalnet — no explicit match arm for the decrypted-wallet variant at the pinned SHA; the fall-through default covers it.
- paypal (`paypal/transformers.rs:1144`)
- razorpay (`razorpay/transformers.rs:306`)
- redsys (`redsys/transformers.rs:252`)
- revolv3 — no explicit match arm for the decrypted-wallet variant at the pinned SHA; the fall-through covers it.
- stax, stripe, trustpay, volt, wellsfargo, worldpay — variant referenced only in catch-all `NotImplemented` arms.

---

## Per-Variant Implementation Notes

This sub-pattern qualifies one variant — `DecryptedWalletTokenDetailsForNetworkTransactionId`. Only one connector (Checkout) has a material implementation.

### Checkout (RepeatPayment-wired)

The dispatch entry is at `checkout/transformers.rs:936-985`. The outgoing `PaymentsRequest<T>` (the same struct used by Checkout's Authorize flow at `checkout/transformers.rs:286-314`) is populated with:

- `source: PaymentSource::DecryptedWalletToken(DecryptedWalletToken { ... })` (`checkout/transformers.rs:968-976`).
- `previous_payment_id: Some(network_transaction_id.clone())` (`checkout/transformers.rs:980`).
- `merchant_initiated: Some(true)` (`checkout/transformers.rs:981`).
- `payment_type: CheckoutPaymentType::{Installment|Recurring|Unscheduled}` depending on `RepeatPaymentData.mit_category` (`checkout/transformers.rs:939-950`).
- `store_for_future_use: None` (`checkout/transformers.rs:983`) — MIT does not create a fresh mandate.

The inner `DecryptedWalletToken` struct is defined at `checkout/transformers.rs:160-169`:

```rust
// From crates/integrations/connector-integration/src/connectors/checkout/transformers.rs:160-169
#[derive(Debug, Serialize)]
pub struct DecryptedWalletToken {
    #[serde(rename = "type")]
    decrypt_type: String,
    token: cards::NetworkToken,
    token_type: String,
    expiry_month: Secret<String>,
    expiry_year: Secret<String>,
    pub billing_address: Option<CheckoutAddress>,
}
```

Field mapping from the UCS variant:

| `DecryptedWalletTokenDetailsForNetworkTransactionId` field | `DecryptedWalletToken` field | Citation |
|------------------------------------------------------------|------------------------------|----------|
| `decrypted_token` | `token` | `checkout/transformers.rs:970` |
| (literal string) | `decrypt_type = "network_token"` | `checkout/transformers.rs:971` |
| `token_source` → `"applepay"` / `"googlepay"` | `token_type` | `checkout/transformers.rs:952-963`, `checkout/transformers.rs:972` |
| `token_exp_month` (passed through) | `expiry_month` | `checkout/transformers.rs:965`, `checkout/transformers.rs:973` |
| `get_expiry_year_4_digit()` | `expiry_year` | `checkout/transformers.rs:966`, `checkout/transformers.rs:974` |
| — | `billing_address` = from `PaymentFlowData` via `get_optional_billing_*` helpers | `checkout/transformers.rs:845-870`, `checkout/transformers.rs:975` |

`eci` and `card_holder_name` from the UCS struct are **not** forwarded by Checkout — the Checkout `DecryptedWalletToken` struct has no fields for them. This is intentional: ECI is CIT-scoped and the wallet provider's cardholder name is not reliable across wallet types.

---

## Common Implementation Patterns

### 1. Dual-match skeleton (shared with card-NTID)

```rust
match &router_data.request.mandate_reference {
    MandateReferenceId::NetworkMandateId(network_transaction_id) => {
        match &router_data.request.payment_method_data {
            PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(ref w) => {
                // build outgoing body with:
                //   w.decrypted_token as DPAN
                //   w.token_exp_month / w.get_expiry_year_4_digit() as expiry
                //   token_source mapped to wallet-brand code (reject None)
                //   NO cryptogram, NO eci (unless connector demands),
                //   NO cvv, NO 3DS,
                //   MIT indicator set,
                //   NTI attached to previous_payment_id equivalent
            }
            PaymentMethodData::CardDetailsForNetworkTransactionId(ref c) => { /* card sibling */ }
            _ => Err(IntegrationError::not_implemented(...)),
        }
    }
    _ => { /* other variants */ }
}
```

### 2. `TokenSource` to wallet-brand mapping

```rust
// From crates/integrations/connector-integration/src/connectors/checkout/transformers.rs:952-963
let token_type = match network_token_data.token_source {
    Some(domain_types::payment_method_data::TokenSource::ApplePay)  => "applepay".to_string(),
    Some(domain_types::payment_method_data::TokenSource::GooglePay) => "googlepay".to_string(),
    None => Err(IntegrationError::MissingRequiredField {
        field_name: "token_source",
        context: Default::default(),
    })?,
};
```

`None` MUST be rejected — the connector cannot infer the wallet brand from the DPAN alone for most schemes. Use `IntegrationError::MissingRequiredField` with `field_name: "token_source"` as Checkout does.

### 3. Absence of CIT-only signals

When building the MIT body, do not populate any of:

- `cryptogram` — the CIT's cryptogram was single-use.
- `cvv` — wallet tokens never have CVV.
- `eci` — CIT-scoped for most schemes; Checkout deliberately drops it.
- `three_ds` challenge indicators — MIT is out of scope.
- `assurance_details` / Google Pay `protocolVersion` — CIT-only.

These absences are enforced by the struct shape itself (`payment_method_data.rs:1340-1348`), but connector-side request structs sometimes allow these fields — leave them `None` / default.

### 4. Expiry date formatting

Use the helper methods on `DecryptedWalletTokenDetailsForNetworkTransactionId` (`payment_method_data.rs:1356-1436`). Checkout calls `get_expiry_year_4_digit()` at `checkout/transformers.rs:966` and passes `token_exp_month` through unmodified. Do not reimplement inline.

### 5. MIT indicator

Either hardcode a "merchant-initiated" flag to `true` (Checkout: `merchant_initiated: Some(true)`) or map `RepeatPaymentData.mit_category` to a scheme-level enum (Checkout: `payment_type` at `checkout/transformers.rs:939-950`). Both are acceptable; do not leave the scheme guessing.

---

## Code Examples

### Example 1: Full NTID-wallet transformer (Checkout pattern)

Excerpted verbatim from `checkout/transformers.rs:936-985`:

```rust
// From crates/integrations/connector-integration/src/connectors/checkout/transformers.rs:936-985
PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(
    ref network_token_data,
) => {
    let p_type = match item.router_data.request.mit_category {
        Some(common_enums::MitCategory::Installment) => {
            CheckoutPaymentType::Installment
        }
        Some(common_enums::MitCategory::Recurring) => {
            CheckoutPaymentType::Recurring
        }
        Some(common_enums::MitCategory::Unscheduled) | None => {
            CheckoutPaymentType::Unscheduled
        }
        _ => CheckoutPaymentType::Unscheduled,
    };

    let token_type = match network_token_data.token_source {
        Some(domain_types::payment_method_data::TokenSource::ApplePay) => {
            "applepay".to_string()
        }
        Some(domain_types::payment_method_data::TokenSource::GooglePay) => {
            "googlepay".to_string()
        }
        None => Err(IntegrationError::MissingRequiredField {
            field_name: "token_source",
            context: Default::default(),
        })?,
    };

    let exp_month = network_token_data.token_exp_month.clone();
    let expiry_year_4_digit = network_token_data.get_expiry_year_4_digit();

    let payment_source =
        PaymentSource::DecryptedWalletToken(DecryptedWalletToken {
            token: network_token_data.decrypted_token.clone(),
            decrypt_type: "network_token".to_string(),
            token_type,
            expiry_month: exp_month,
            expiry_year: expiry_year_4_digit,
            billing_address: billing_details,
        });

    Ok((
        payment_source,
        Some(network_transaction_id.clone()),
        Some(true),
        p_type,
        None,
    ))
}
```

### Example 2: Minimal skeleton for a new connector

```rust
// Adapt this for a new connector that supports decrypted-wallet MIT via NTI
impl<T> TryFrom<
    MyConnectorRouterData<
        RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        T,
    >,
> for MyConnectorPaymentsRequest<T>
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: MyConnectorRouterData<
            RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let (source, previous_id, merchant_initiated) =
            match &item.router_data.request.mandate_reference {
                MandateReferenceId::NetworkMandateId(nti) => match &item.router_data.request.payment_method_data {
                    PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(w) => {
                        let brand = match w.token_source {
                            Some(TokenSource::ApplePay)  => "apple_pay",
                            Some(TokenSource::GooglePay) => "google_pay",
                            None => return Err(IntegrationError::MissingRequiredField {
                                field_name: "token_source",
                                context: Default::default(),
                            }.into()),
                        };
                        let src = MyConnectorSource::DecryptedWalletToken {
                            dpan: w.decrypted_token.clone(),
                            exp_month: w.token_exp_month.clone(),
                            exp_year: w.get_expiry_year_4_digit(),
                            wallet_brand: brand.to_string(),
                            // intentionally no cryptogram, no eci, no cvv
                        };
                        (src, Some(nti.clone()), true)
                    }
                    _ => return Err(IntegrationError::not_implemented(
                        "decrypted-wallet NTID not implemented for this PM".to_string(),
                    ).into()),
                },
                _ => return Err(IntegrationError::not_implemented(
                    "non-NTI MIT not implemented".to_string(),
                ).into()),
            };

        Ok(Self {
            source,
            amount: item.amount,
            currency: item.router_data.request.currency,
            previous_payment_id: previous_id,
            merchant_initiated: Some(merchant_initiated),
            // ... other fields unchanged
        })
    }
}
```

### Example 3: Rejecting `token_source = None`

```rust
// Copy-paste pattern, citation: crates/integrations/connector-integration/src/connectors/checkout/transformers.rs:959-962
None => Err(IntegrationError::MissingRequiredField {
    field_name: "token_source",
    context: Default::default(),
})?,
```

Do not synthesize a default wallet brand; do not fall back to card-NTID semantics; do not emit `"unknown"`.

---

## Best Practices

- **Always reject `token_source = None`.** The wallet brand is required for correct scheme routing and cannot be recovered from the DPAN. See `checkout/transformers.rs:959-962`.
- **Always match `MandateReferenceId::NetworkMandateId` first, then `PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId`.** The ordering mirrors the card sibling ([`../card/pattern_authorize_card_ntid.md`](../card/pattern_authorize_card_ntid.md)) and the Checkout implementation at `checkout/transformers.rs:898-985`.
- **Do not forward the ECI or cryptogram.** They are CIT-scoped. The UCS struct preserves `eci` only for the rare connector that demands it; default is to drop it. Checkout drops it at the Checkout `DecryptedWalletToken` struct definition (`checkout/transformers.rs:160-169`).
- **Use the helper methods on `DecryptedWalletTokenDetailsForNetworkTransactionId`** (`payment_method_data.rs:1356-1436`) for expiry formatting. Do not reimplement 2-digit / 4-digit slicing inline.
- **Carry the NTI in the same field you would for card-NTID** if your connector uses one shared request struct for both NTID variants. Checkout's `PaymentsRequest<T>.previous_payment_id` at `checkout/transformers.rs:304` is shared by both variants.
- **Set MIT signals identically to the card-NTID path.** `merchant_initiated = Some(true)` and `payment_type` mapped from `RepeatPaymentData.mit_category` are both observed at `checkout/transformers.rs:939-950` and `checkout/transformers.rs:981`.
- **If the connector does not support decrypted-wallet NTID, return `IntegrationError::not_implemented`** rather than silently falling back to a raw-card-NTID or tokenized-wallet CIT shape. This is the established fall-through pattern across the stub list above.
- **Follow the parent Wallet pattern's status-mapping rules.** The MIT response flows through the same `PaymentsResponseData` / `AttemptStatus` mapping as the parent Wallet pattern; do not hardcode statuses (`PATTERN_AUTHORING_SPEC.md` §11 item 1).
- **Consult [`../../utility_functions_reference.md`](../../../utility_functions_reference.md)** for any shared helper (e.g. BIN-based issuer lookup, billing-address extractors) rather than inlining utility logic.

---

## Common Errors

1. **Problem**: Forwarding the CIT cryptogram into the MIT body.
   **Solution**: The cryptogram is single-use. `DecryptedWalletTokenDetailsForNetworkTransactionId` does not carry one (`payment_method_data.rs:1340-1348`), so there is nothing to copy. If your connector-side request struct has a cryptogram field, set it to `None`.

2. **Problem**: Treating `token_source = None` as equivalent to `Some(GooglePay)` or defaulting to card.
   **Solution**: Reject with `IntegrationError::MissingRequiredField { field_name: "token_source", .. }` as Checkout does at `checkout/transformers.rs:959-962`.

3. **Problem**: Passing the raw PAN via `decrypted_token`.
   **Solution**: `cards::NetworkToken` is a distinct type from `cards::CardNumber`. The orchestrator guarantees `decrypted_token` is a DPAN; do not convert or reformat. Compare the field types at `payment_method_data.rs:1342` (NetworkToken) vs `payment_method_data.rs:1440` (CardNumber, in the card sibling struct).

4. **Problem**: Attaching the NTI to the wrong field (e.g. to a `mandate_id` field that expects connector-scoped ids).
   **Solution**: The NTI goes into the connector's `previous_payment_id` / `original_network_transaction_id` / `scheme_reference` equivalent — identical to the card-NTID sibling. See `checkout/transformers.rs:980` for the Checkout field, and the card-sibling's [§7 table](../card/pattern_authorize_card_ntid.md#common-implementation-patterns) for other connectors' naming.

5. **Problem**: Forwarding 3DS / SCA fields into the MIT body.
   **Solution**: Drop them entirely. If the connector-side request requires a 3DS block, populate a fully-disabled one (Checkout does this by building `CheckoutThreeDS { enabled: false, force_3ds: false, .. }` at `checkout/transformers.rs:996-1004`).

6. **Problem**: Falling back to the parent Wallet pattern's token-based CIT path when `DecryptedWalletTokenDetailsForNetworkTransactionId` is encountered.
   **Solution**: This will either fail at runtime (no cryptogram) or succeed but be declined by the scheme (MIT-context mismatch). Match the NTID variant explicitly and route into this sub-pattern's transformer.

7. **Problem**: Dropping `billing_address` from the request.
   **Solution**: Some wallets (especially Apple Pay) populate billing address on the CIT via the wallet sheet, and the scheme expects it replayed on MIT for AVS. Rebuild it from `PaymentFlowData` helpers — see `checkout/transformers.rs:845-870`.

---

## Cross-References

- Parent PM pattern: [`./pattern_authorize_wallet.md`](./pattern_authorize_wallet.md)
- Parent flow pattern: [`../../pattern_authorize.md`](../../pattern_authorize.md)
- Composed flow pattern: [`../../pattern_repeat_payment_flow.md`](../../pattern_repeat_payment_flow.md)
- Upstream CIT pattern: [`../../pattern_setup_mandate.md`](../../pattern_setup_mandate.md)
- Sibling NTID sub-pattern (card variant): [`../card/pattern_authorize_card_ntid.md`](../card/pattern_authorize_card_ntid.md)
- Sibling PM pattern: [`../card/pattern_authorize_card.md`](../card/pattern_authorize_card.md)
- Authorize-index: [`../README.md`](../README.md)
- Patterns-index: [`../../README.md`](../../README.md)
- Pattern authoring spec: [`../../PATTERN_AUTHORING_SPEC.md`](../../PATTERN_AUTHORING_SPEC.md)
- Utility helpers: [`../../../utility_functions_reference.md`](../../../utility_functions_reference.md)
- Types reference: [`../../../types/types.md`](../../../types/types.md)
