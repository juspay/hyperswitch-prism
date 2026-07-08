# RealTimePayment Authorize Flow Pattern

## Overview

"Real-time payment" (RTP) rails are bank-operated instant-transfer networks, usually regulated by a central bank or clearing house, that settle 24/7 within seconds and are normally presented to the payer as a QR code or a bank-app intent. Unlike UPI (which is India-specific and has its own enum) or generic `BankTransfer`/`BankRedirect` flows, the four variants enumerated under `RealTimePaymentData` are closed account-to-account rails with per-country branding requirements. The enum at `crates/types-traits/domain_types/src/payment_method_data.rs:511-516` intentionally defines the four variants as empty unit-like structs (`DuitNow {}`, `Fps {}`, `PromptPay {}`, `VietQr {}`) because, at the pinned SHA, the rail identity is the only piece of information the connector needs — payer credentials are collected off-platform by the payer's bank app or by a scanned QR.

Key Characteristics:

| Attribute | Value |
|-----------|-------|
| Enum | `RealTimePaymentData` |
| Enum location | `crates/types-traits/domain_types/src/payment_method_data.rs:511` |
| PM wrapper | `PaymentMethodData::RealTimePayment(Box<RealTimePaymentData>)` (`payment_method_data.rs:263`) |
| Variant count | 4 (`DuitNow`, `Fps`, `PromptPay`, `VietQr`) |
| Payer-side artifact | QR code or bank-app deep link |
| Typical response | Async; connector returns a redirect URL or QR payload, final status arrives via PSync or webhook |
| Currencies | Each variant is tied to one local currency (MYR, GBP, THB, VND); see Variant Enumeration below |
| Capture | Auto-capture only in practice; RTP rails do not support manual capture at the pinned SHA |
| Mandates | None of the four variants supports recurring/MIT at the pinned SHA |

## Table of Contents

1. [Variant Enumeration](#variant-enumeration)
2. [Architecture Overview](#architecture-overview)
3. [Connectors with Full Implementation](#connectors-with-full-implementation)
4. [Per-Variant Implementation Notes](#per-variant-implementation-notes)
5. [Common Implementation Patterns](#common-implementation-patterns)
6. [Code Examples](#code-examples)
7. [Best Practices](#best-practices)
8. [Common Errors](#common-errors)
9. [Cross-References](#cross-references)

## Variant Enumeration

The source enum, from `crates/types-traits/domain_types/src/payment_method_data.rs:511-516`:

```rust
// From crates/types-traits/domain_types/src/payment_method_data.rs:511
#[derive(Debug, Clone, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum RealTimePaymentData {
    DuitNow {},
    Fps {},
    PromptPay {},
    VietQr {},
}
```

Every variant is an empty brace-style struct variant. The connector MUST match on variant identity only; there are no inner fields to read at the pinned SHA. The PM pattern ALWAYS reaches this enum through `PaymentMethodData::RealTimePayment(Box<RealTimePaymentData>)` (`payment_method_data.rs:263`), so the dereference in transformers is `match &**rtp_data { ... }` (see iatapay example below).

| Variant | Data Shape | Citation | Real-world context | Used By (connectors) |
|---------|-----------|----------|--------------------|----------------------|
| `DuitNow {}` | Unit-like struct variant, no fields | `crates/types-traits/domain_types/src/payment_method_data.rs:512` | Malaysia. DuitNow is operated by PayNet (Payments Network Malaysia Sdn Bhd) under Bank Negara Malaysia oversight. Real-time account-to-account rail; the consumer presents a DuitNow QR ("MALAYSIA NATIONAL QR") which the payer scans with their bank app. Settles within seconds. Currency: MYR. | fiuu (full, QR-only) at `crates/integrations/connector-integration/src/connectors/fiuu/transformers.rs:569-573`; iatapay (country-mapping at `crates/integrations/connector-integration/src/connectors/iatapay/transformers.rs:208-209`). Stripe and Adyen list `PaymentMethodData::RealTimePayment(_)` only in `NotImplemented` fall-through arms — no variant-specific handling. |
| `Fps {}` | Unit-like struct variant, no fields | `crates/types-traits/domain_types/src/payment_method_data.rs:513` | Hong Kong. Faster Payment System, operated by the Hong Kong Interbank Clearing Limited (HKICL) under HKMA. 24/7 account-to-account transfer using a proxy ID (phone, email) or FPS QR. Settles near-instantly. Currency: HKD (the `common_enums::PaymentMethodType::Fps` variant in Stripe's fall-through list at `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:917` confirms the rail is a known type even though no transformer handles it). Note: iatapay's country map routes `Fps {}` to `CountryAlpha2::HK`. The brief's "UK Faster Payments" phrasing refers to the UK rail of the same marketing name; the enum variant as used in this codebase maps to Hong Kong per iatapay at `crates/integrations/connector-integration/src/connectors/iatapay/transformers.rs:211`. | iatapay (country-map only, no request-level handling) at `crates/integrations/connector-integration/src/connectors/iatapay/transformers.rs:211`; fiuu returns `not_implemented` for `Fps {}` at `crates/integrations/connector-integration/src/connectors/fiuu/transformers.rs:574-579`. No connector at the pinned SHA sends an Fps-specific request body. |
| `PromptPay {}` | Unit-like struct variant, no fields | `crates/types-traits/domain_types/src/payment_method_data.rs:514` | Thailand. PromptPay is operated by National ITMX under the Bank of Thailand. QR-based instant transfer using mobile number, citizen ID, or tax ID as the proxy; widely deployed at merchants. Settles within seconds. Currency: THB. | iatapay (country-map only) at `crates/integrations/connector-integration/src/connectors/iatapay/transformers.rs:213`; fiuu returns `not_implemented` for `PromptPay {}` at `crates/integrations/connector-integration/src/connectors/fiuu/transformers.rs:574-579`. No connector sends a PromptPay-specific request body at the pinned SHA. |
| `VietQr {}` | Unit-like struct variant, no fields | `crates/types-traits/domain_types/src/payment_method_data.rs:515` | Vietnam. VietQR is a unified QR-code standard for domestic account-to-account transfers over the Napas 247 rail, coordinated by Napas (National Payment Corporation of Vietnam) under the State Bank of Vietnam. Settlement is real-time during business hours and near-instant otherwise. Currency: VND. | iatapay (country-map only) at `crates/integrations/connector-integration/src/connectors/iatapay/transformers.rs:215`; fiuu returns `not_implemented` for `VietQr {}` at `crates/integrations/connector-integration/src/connectors/fiuu/transformers.rs:574-579`. No connector sends a VietQr-specific request body at the pinned SHA. |

Summary of coverage at the pinned SHA:

- `DuitNow`: one connector (fiuu) issues a variant-specific request (`TxnChannel::RppDuitNowQr`) and parses a QR response.
- `Fps`, `PromptPay`, `VietQr`: zero connectors issue a variant-specific request body. They are only observed as (a) inputs to the iatapay country-resolution helper, or (b) `NotImplemented` arms.

## Architecture Overview

### Flow Type

`Authorize` marker, from `domain_types::connector_flow`. RTP payments take the standard authorize path; the async nature of settlement is handled through the PSync flow (not covered here).

### Request Type

`PaymentsAuthorizeData<T>` — `crates/types-traits/domain_types/src/connector_types.rs`. Inside, `payment_method_data` is `PaymentMethodData<T>`, and the RTP branch is reached via `PaymentMethodData::RealTimePayment(Box<RealTimePaymentData>)` (`crates/types-traits/domain_types/src/payment_method_data.rs:263`).

### Response Type

`PaymentsResponseData`. RTP connectors overwhelmingly return `PaymentsResponseData::TransactionResponse` with either:

1. A `redirection_data: Some(Box<RedirectForm::Form { endpoint, method, form_fields }>)` pointing at the bank app / hosted QR page (iatapay returns this for generic RTP today at `crates/integrations/connector-integration/src/connectors/iatapay/transformers.rs:388-398`), or
2. A `connector_metadata` JSON blob containing a QR payload for client-side rendering (fiuu builds this via `get_qr_metadata` at `crates/integrations/connector-integration/src/connectors/fiuu/transformers.rs:2229-2260`).

### Resource Common Data

`PaymentFlowData` — the standard payment flow data struct.

### Canonical RouterDataV2 signature

```rust
RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
```

Per-§7 of the Pattern Authoring Spec, this is the only acceptable shape.

### Where the variant is unwrapped

All RTP transformers follow the same dereference pattern because `RealTimePayment` is boxed:

```rust
// From crates/integrations/connector-integration/src/connectors/iatapay/transformers.rs:207-216
PaymentMethodData::RealTimePayment(rtp_data) => match &**rtp_data {
    RealTimePaymentData::DuitNow {} => Ok(CountryAlpha2::MY),
    RealTimePaymentData::Fps {} => Ok(CountryAlpha2::HK),
    RealTimePaymentData::PromptPay {} => Ok(CountryAlpha2::TH),
    RealTimePaymentData::VietQr {} => Ok(CountryAlpha2::VN),
},
```

or the clone-then-match variant used by fiuu (see `crates/integrations/connector-integration/src/connectors/fiuu/transformers.rs:567-580`). Both are equivalent — the `&**` form avoids the clone.

## Connectors with Full Implementation

At the pinned SHA, only one connector issues a variant-specific RTP request body (fiuu, DuitNow only) and only one connector routes RTP payloads in any meaningful way (iatapay, via country resolution). All other connectors that reference `PaymentMethodData::RealTimePayment(_)` do so only inside `NotImplemented`/"match-anything-else" fall-through arms and are listed under "Stub Implementations" below.

| Connector | HTTP Method | Content Type | URL Pattern | Request Type Reuse | Notes |
|-----------|-------------|--------------|-------------|--------------------|-------|
| fiuu | POST | multipart/form-data | `{base_url}RMS/API/Direct/1.4.0/index.php` | `FiuuPaymentRequest<T>` — reused for card + QR flows; RTP routed via `FiuuPaymentMethodData::FiuuQRData` | Only `DuitNow` supported. `Fps`/`PromptPay`/`VietQr` map to `IntegrationError::not_implemented`. Response demultiplexes on `FiuuPaymentsResponse::QRPaymentResponse(Box<DuitNowQrCodeResponse>)` (`crates/integrations/connector-integration/src/connectors/fiuu/transformers.rs:962-966`). URL from `crates/integrations/connector-integration/src/connectors/fiuu.rs:450-453`; content type from `fiuu.rs:389-391`. |
| iatapay | POST | application/json | `{base_url}/payments/` | `IatapayPaymentsRequest` — single request body for UPI, BankRedirect, and RealTimePayment; rail identity is erased and only the country code is sent to iatapay | RTP participates through the `get_country_from_payment_method` helper at `crates/integrations/connector-integration/src/connectors/iatapay/transformers.rs:185-222`; the outgoing body carries only `country` + `locale`, not a variant discriminator. URL from `crates/integrations/connector-integration/src/connectors/iatapay.rs:424-429`. |

### Stub Implementations

Connectors that mention `PaymentMethodData::RealTimePayment(_)` only in `NotImplemented`/fall-through arms at the pinned SHA (i.e. the PM is declared unsupported):

- aci — `crates/integrations/connector-integration/src/connectors/aci/transformers.rs:742`
- adyen — `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:3699` and `:6040`
- bambora — `crates/integrations/connector-integration/src/connectors/bambora/transformers.rs:291`
- bankofamerica — `crates/integrations/connector-integration/src/connectors/bankofamerica/transformers.rs:608`, `:1772`, `:2155`
- billwerk — `crates/integrations/connector-integration/src/connectors/billwerk/transformers.rs:228`
- braintree — `crates/integrations/connector-integration/src/connectors/braintree/transformers.rs:605`, `:1603`, `:2625`, `:2807`
- cryptopay — `crates/integrations/connector-integration/src/connectors/cryptopay/transformers.rs:104`
- cybersource — `crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:316`, `:2181`, `:2281`, `:3020`, `:3297`, `:4318`
- dlocal — `crates/integrations/connector-integration/src/connectors/dlocal/transformers.rs:202`
- fiserv — `crates/integrations/connector-integration/src/connectors/fiserv/transformers.rs:543`
- forte — `crates/integrations/connector-integration/src/connectors/forte/transformers.rs:306`
- hipay — `crates/integrations/connector-integration/src/connectors/hipay/transformers.rs:589`
- loonio — `crates/integrations/connector-integration/src/connectors/loonio/transformers.rs:239`
- mifinity — `crates/integrations/connector-integration/src/connectors/mifinity/transformers.rs:242`
- multisafepay — `crates/integrations/connector-integration/src/connectors/multisafepay/transformers.rs:150`, `:330`
- nexinets — `crates/integrations/connector-integration/src/connectors/nexinets/transformers.rs:734`
- noon — `crates/integrations/connector-integration/src/connectors/noon/transformers.rs:371`, `:1256`
- paypal — `crates/integrations/connector-integration/src/connectors/paypal/transformers.rs:1137`, `:2598`
- placetopay — `crates/integrations/connector-integration/src/connectors/placetopay/transformers.rs:204`
- razorpay — `crates/integrations/connector-integration/src/connectors/razorpay/transformers.rs:300`
- redsys — `crates/integrations/connector-integration/src/connectors/redsys/transformers.rs:245`
- stax — `crates/integrations/connector-integration/src/connectors/stax/transformers.rs:1092`
- stripe — `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:1512`, `:4637`, `:5032` (also references `common_enums::PaymentMethodType::{Fps,DuitNow,PromptPay,VietQr}` at `stripe/transformers.rs:917-920` but only for enum completeness in filter lists, not for request construction)
- trustpay — `crates/integrations/connector-integration/src/connectors/trustpay/transformers.rs:1705`
- volt — `crates/integrations/connector-integration/src/connectors/volt/transformers.rs:289`
- wellsfargo — `crates/integrations/connector-integration/src/connectors/wellsfargo/transformers.rs:591`
- worldpay — `crates/integrations/connector-integration/src/connectors/worldpay/transformers.rs:215`

## Per-Variant Implementation Notes

### `RealTimePaymentData::DuitNow {}`

**Implemented fully by**: fiuu.

**Transformer path**: Authorize request builder → `PaymentMethodData::RealTimePayment(...)` branch → `match *real_time_payment_data.clone()` → `RealTimePaymentData::DuitNow {} =>` arm → construct `FiuuPaymentMethodData::FiuuQRData(Box::new(FiuuQRData { txn_channel: TxnChannel::RppDuitNowQr }))`. The `RPP_DUITNOWQR` wire value comes from the `TxnChannel` enum at `crates/integrations/connector-integration/src/connectors/fiuu/transformers.rs:104-112`.

Citations:

- Match arm: `crates/integrations/connector-integration/src/connectors/fiuu/transformers.rs:569-573`.
- Channel enum: `crates/integrations/connector-integration/src/connectors/fiuu/transformers.rs:109-111`.
- Response type: `FiuuPaymentsResponse::QRPaymentResponse(Box<DuitNowQrCodeResponse>)` at `crates/integrations/connector-integration/src/connectors/fiuu/transformers.rs:964` and struct at `:938-947`.
- QR metadata builder: `get_qr_metadata` at `crates/integrations/connector-integration/src/connectors/fiuu/transformers.rs:2229-2260` — decodes the QR string to a colored image and returns a `QrCodeInformation::QrColorDataUrl` serialized to a `serde_json::Value`.
- Branding constants: `DUIT_NOW_BRAND_COLOR = "#ED2E67"` and `DUIT_NOW_BRAND_TEXT = "MALAYSIA NATIONAL QR"` at `crates/integrations/connector-integration/src/connectors/fiuu/transformers.rs:2518-2520`.

**Partially implemented by**: iatapay — maps `DuitNow {}` to `CountryAlpha2::MY` in the country-resolution helper (`crates/integrations/connector-integration/src/connectors/iatapay/transformers.rs:208-209`), but the outgoing `IatapayPaymentsRequest` carries only `country` and `locale` — no DuitNow-specific field (`crates/integrations/connector-integration/src/connectors/iatapay/transformers.rs:122-136`, request assembled at `:303-322`).

### `RealTimePaymentData::Fps {}`

**Fully implemented by**: none.

**Partially implemented by**: iatapay — country-only mapping to `CountryAlpha2::HK` at `crates/integrations/connector-integration/src/connectors/iatapay/transformers.rs:210-211`.

**Explicitly rejected by**: fiuu — `Fps {}` is in the `Err(IntegrationError::not_implemented(...))` arm at `crates/integrations/connector-integration/src/connectors/fiuu/transformers.rs:574-579`.

**Note on naming**: the spec brief describes `Fps` as "UK Faster Payments", but the only place in the connector-service codebase that resolves `Fps {}` to a country — iatapay — maps it to Hong Kong. The enum definition at `payment_method_data.rs:513` carries no documentation of its own; treat `Fps {}` as rail-agnostic at the type level and let the connector decide, matching iatapay's precedent, until the enum gains a doc-comment at a future SHA.

### `RealTimePaymentData::PromptPay {}`

**Fully implemented by**: none.

**Partially implemented by**: iatapay — country-only mapping to `CountryAlpha2::TH` at `crates/integrations/connector-integration/src/connectors/iatapay/transformers.rs:212-213`. iatapay's response handler also contains QR-code-URL branching at `crates/integrations/connector-integration/src/connectors/iatapay/transformers.rs:364-387` — specifically a `redirect_url.to_lowercase().ends_with("qr")` check that produces `connector_metadata = { "qr_code_url": <url> }` instead of `redirection_data`. That branch is the likely delivery mechanism for PromptPay/VietQr when iatapay returns a QR-style checkout URL, though the code does not gate it on the RTP variant.

**Explicitly rejected by**: fiuu at `crates/integrations/connector-integration/src/connectors/fiuu/transformers.rs:574-579`.

### `RealTimePaymentData::VietQr {}`

**Fully implemented by**: none.

**Partially implemented by**: iatapay — country-only mapping to `CountryAlpha2::VN` at `crates/integrations/connector-integration/src/connectors/iatapay/transformers.rs:214-215`.

**Explicitly rejected by**: fiuu at `crates/integrations/connector-integration/src/connectors/fiuu/transformers.rs:574-579`.

## Common Implementation Patterns

Across the two connectors that do anything meaningful with `RealTimePaymentData`, three patterns recur.

### 1. Country-resolution helper (iatapay)

The connector does not model the rail identity in its wire request; instead it derives a country code from the variant and sends that. Useful when the gateway routes to the correct local rail internally based on country + currency.

```rust
// From crates/integrations/connector-integration/src/connectors/iatapay/transformers.rs:185-222
fn get_country_from_payment_method<T>(
    payment_method_data: &PaymentMethodData<T>,
) -> Result<CountryAlpha2, Report<IntegrationError>>
where
    T: PaymentMethodDataTypes,
{
    match payment_method_data {
        PaymentMethodData::Upi(_) => Ok(CountryAlpha2::IN),
        PaymentMethodData::BankRedirect(redirect_data) => match redirect_data { /* ... */ },
        PaymentMethodData::RealTimePayment(rtp_data) => match &**rtp_data {
            RealTimePaymentData::DuitNow {} => Ok(CountryAlpha2::MY),
            RealTimePaymentData::Fps {} => Ok(CountryAlpha2::HK),
            RealTimePaymentData::PromptPay {} => Ok(CountryAlpha2::TH),
            RealTimePaymentData::VietQr {} => Ok(CountryAlpha2::VN),
        },
        _ => Err(Report::new(IntegrationError::not_implemented(
            "Payment method not supported by Iatapay".to_string(),
        ))),
    }
}
```

### 2. Variant-to-channel mapping (fiuu)

The connector exposes a dedicated per-rail channel and builds a QR-specific payload. This is the "honest" RTP pattern — the request shape is different from a card request because the connector generates a scannable QR on its side.

```rust
// From crates/integrations/connector-integration/src/connectors/fiuu/transformers.rs:567-580
PaymentMethodData::RealTimePayment(ref real_time_payment_data) => {
    match *real_time_payment_data.clone() {
        RealTimePaymentData::DuitNow {} => {
            Ok(FiuuPaymentMethodData::FiuuQRData(Box::new(FiuuQRData {
                txn_channel: TxnChannel::RppDuitNowQr,
            })))
        }
        RealTimePaymentData::Fps {}
        | RealTimePaymentData::PromptPay {}
        | RealTimePaymentData::VietQr {} => Err(IntegrationError::not_implemented(
            utils::get_unimplemented_payment_method_error_message("fiuu"),
        )
        .into()),
    }
}
```

### 3. NotImplemented fall-through (all stub connectors)

The most common shape in the codebase. The connector lists `PaymentMethodData::RealTimePayment(_)` among the `| or-pattern` arms of a big `match` in a "we don't support this" branch. Canonical example:

```rust
// From crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:6040
| PaymentMethodData::RealTimePayment(_)
```

Authors adding a new connector that does not support RTP SHOULD adopt this shape — listing the variant explicitly in a `NotImplemented` arm is preferred over a wildcard `_` arm because it keeps the match exhaustive when the enum gains new variants at a future SHA.

## Code Examples

### Example 1 — fiuu DuitNow request construction

```rust
// From crates/integrations/connector-integration/src/connectors/fiuu/transformers.rs:104-112
#[derive(Serialize, Deserialize, Display, Debug, Clone)]
enum TxnChannel {
    #[serde(rename = "CREDITAN")]
    #[strum(serialize = "CREDITAN")]
    Creditan,
    #[serde(rename = "RPP_DUITNOWQR")]
    #[strum(serialize = "RPP_DUITNOWQR")]
    RppDuitNowQr,
}
```

```rust
// From crates/integrations/connector-integration/src/connectors/fiuu/transformers.rs:567-580
PaymentMethodData::RealTimePayment(ref real_time_payment_data) => {
    match *real_time_payment_data.clone() {
        RealTimePaymentData::DuitNow {} => {
            Ok(FiuuPaymentMethodData::FiuuQRData(Box::new(FiuuQRData {
                txn_channel: TxnChannel::RppDuitNowQr,
            })))
        }
        RealTimePaymentData::Fps {}
        | RealTimePaymentData::PromptPay {}
        | RealTimePaymentData::VietQr {} => Err(IntegrationError::not_implemented(
            utils::get_unimplemented_payment_method_error_message("fiuu"),
        )
        .into()),
    }
}
```

### Example 2 — fiuu DuitNow response decoding into QR metadata

```rust
// From crates/integrations/connector-integration/src/connectors/fiuu/transformers.rs:938-947
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DuitNowQrCodeResponse {
    pub reference_no: String,
    pub txn_type: TxnType,
    pub txn_currency: Currency,
    pub txn_amount: StringMajorUnit,
    pub txn_channel: String,
    #[serde(rename = "TxnID")]
    pub txn_id: String,
    pub txn_data: QrTxnData,
}
```

```rust
// From crates/integrations/connector-integration/src/connectors/fiuu/transformers.rs:2229-2260
pub fn get_qr_metadata(
    response: &DuitNowQrCodeResponse,
) -> CustomResult<Option<Value>, ConnectorResponseTransformationError> {
    let image_data = QrImage::new_colored_from_data(
        response.txn_data.request_data.qr_data.peek().clone(),
        DUIT_NOW_BRAND_COLOR,
    )
    .change_context(
        ConnectorResponseTransformationError::response_handling_failed_http_status_unknown(),
    )?;

    let image_data_url = Url::parse(image_data.data.clone().as_str()).ok();
    let display_to_timestamp = None;

    if let Some(color_image_data_url) = image_data_url {
        let qr_code_info = QrCodeInformation::QrColorDataUrl {
            color_image_data_url,
            display_to_timestamp,
            display_text: Some(DUIT_NOW_BRAND_TEXT.to_string()),
            border_color: Some(DUIT_NOW_BRAND_COLOR.to_string()),
        };

        Some(qr_code_info.encode_to_value())
            .transpose()
            .change_context(
                ConnectorResponseTransformationError::response_handling_failed_http_status_unknown(
                ),
            )
    } else {
        Ok(None)
    }
}
```

### Example 3 — fiuu response demultiplexer

```rust
// From crates/integrations/connector-integration/src/connectors/fiuu/transformers.rs:960-967
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FiuuPaymentsResponse {
    PaymentResponse(Box<PaymentsResponse>),
    QRPaymentResponse(Box<DuitNowQrCodeResponse>),
    Error(FiuuErrorResponse),
    RecurringResponse(Vec<Box<FiuuRecurringResponse>>),
}
```

The `untagged` variant discrimination lets fiuu accept either a regular card-style response or a DuitNow QR response at the same endpoint — the `DuitNow` request triggers the `QRPaymentResponse` variant to be deserialized server-side.

### Example 4 — iatapay country resolution

```rust
// From crates/integrations/connector-integration/src/connectors/iatapay/transformers.rs:205-216
// Real-time payment methods
PaymentMethodData::RealTimePayment(rtp_data) => match &**rtp_data {
    // DuitNow → Malaysia
    RealTimePaymentData::DuitNow {} => Ok(CountryAlpha2::MY),
    // FPS → Hong Kong
    RealTimePaymentData::Fps {} => Ok(CountryAlpha2::HK),
    // PromptPay → Thailand
    RealTimePaymentData::PromptPay {} => Ok(CountryAlpha2::TH),
    // VietQR → Vietnam
    RealTimePaymentData::VietQr {} => Ok(CountryAlpha2::VN),
},
```

### Example 5 — iatapay response QR branching

The iatapay response handler does not gate on `RealTimePaymentData` at all — it inspects the returned `redirect_url` and promotes it to `connector_metadata.qr_code_url` when the URL ends in `qr`:

```rust
// From crates/integrations/connector-integration/src/connectors/iatapay/transformers.rs:364-400
let payments_response_data = match &response.checkout_methods {
    Some(checkout_methods) => {
        let form_fields = HashMap::new();
        let (connector_metadata, redirection_data) = match checkout_methods
            .redirect
            .redirect_url
            .to_lowercase()
            .ends_with("qr")
        {
            true => {
                // QR code flow - store in metadata
                let mut metadata_map = HashMap::new();
                metadata_map.insert(
                    "qr_code_url".to_string(),
                    Value::String(checkout_methods.redirect.redirect_url.clone()),
                );
                let metadata_value = serde_json::to_value(metadata_map).change_context(
                    crate::utils::response_handling_fail_for_connector(
                        item.http_code,
                        "iatapay",
                    ),
                )?;
                (Some(metadata_value), None)
            }
            false => {
                // Standard redirect flow
                (
                    None,
                    Some(Box::new(RedirectForm::Form {
                        endpoint: checkout_methods.redirect.redirect_url.clone(),
                        method: Method::Get,
                        // ...
                    })),
                )
            }
        };
```

### Example 6 — Stripe's PaymentMethodType filter list (RTP as known-but-unsupported)

```rust
// From crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:917-920
| common_enums::PaymentMethodType::Fps
| common_enums::PaymentMethodType::DuitNow
| common_enums::PaymentMethodType::PromptPay
| common_enums::PaymentMethodType::VietQr
```

Stripe enumerates all four `PaymentMethodType` aliases for completeness but does not build a request body for any of them; the fall-through at `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:1512` confirms the PM-data variant is rejected.

## Best Practices

- **Always dereference the `Box` with `&**`**, not by cloning. The `Box<RealTimePaymentData>` wrapper at `crates/types-traits/domain_types/src/payment_method_data.rs:263` is already on the heap; the idiomatic form is `match &**rtp_data { ... }` as iatapay uses at `crates/integrations/connector-integration/src/connectors/iatapay/transformers.rs:207`. The `match *real_time_payment_data.clone()` form used by fiuu at `crates/integrations/connector-integration/src/connectors/fiuu/transformers.rs:568` costs an extra clone of an empty struct; prefer the iatapay form in new code.
- **Use exhaustive variant matching, never wildcards**, so the Rust compiler flags new variants at future SHAs. Fiuu's `RealTimePaymentData::Fps {} | RealTimePaymentData::PromptPay {} | RealTimePaymentData::VietQr {}` arm at `crates/integrations/connector-integration/src/connectors/fiuu/transformers.rs:574-576` is the reference pattern: list every unsupported variant by name even when they share an error arm.
- **Return `IntegrationError::not_implemented` with `get_unimplemented_payment_method_error_message(<connector>)`** for unsupported variants, as fiuu does at `crates/integrations/connector-integration/src/connectors/fiuu/transformers.rs:576-579`. See [../../utility_functions_reference.md](../../utility_functions_reference.md) for the helper.
- **Do not hardcode a status like `AttemptStatus::Pending` in the `TryFrom` block** for RTP responses. RTP settles within seconds but can still fail (rejected by payer's bank, expired QR). Always map from a response status field — fiuu does this via the `PaymentsResponse`/`QRPaymentResponse` discriminator; iatapay does it via `IatapayPaymentStatus` (`crates/integrations/connector-integration/src/connectors/iatapay/transformers.rs:97-119`).
- **Model QR-returning rails distinctly from redirect-returning rails.** If the rail ends in a scannable QR (DuitNow, PromptPay QR mode, VietQr), produce `connector_metadata` with a `qr_code_url` or a serialized `QrCodeInformation` (fiuu's `get_qr_metadata` at `crates/integrations/connector-integration/src/connectors/fiuu/transformers.rs:2229-2260`). If the rail ends in a bank-app deep link (Fps with proxy-ID flow), produce `redirection_data: Some(Box<RedirectForm::Form { ... }>)`. Mixing them confuses the SDK's rendering pipeline.
- **Pair every RTP Authorize with a PSync implementation.** RTP responses are async; the terminal state does not arrive in the authorize HTTP response. Iatapay demonstrates the canonical pairing by registering both flows in `crates/integrations/connector-integration/src/connectors/iatapay.rs:237-246`.
- **Use the connector's declared amount unit.** Iatapay uses `FloatMajorUnit` (`crates/integrations/connector-integration/src/connectors/iatapay/transformers.rs:128`); fiuu uses `StringMajorUnit` in its DuitNow response (`crates/integrations/connector-integration/src/connectors/fiuu/transformers.rs:942`). Never hand-roll unit conversions; use the macro-generated amount converter per `macros::create_amount_converter_wrapper!` as described in [../card/pattern_authorize_card.md](../card/pattern_authorize_card.md).

## Common Errors

1. **Problem**: Treating `RealTimePaymentData::DuitNow {}` as the same request shape as a card payload.
   **Solution**: Branch on `PaymentMethodData::RealTimePayment(...)` before the card arm and produce a distinct payload, e.g. fiuu's `FiuuPaymentMethodData::FiuuQRData` variant at `crates/integrations/connector-integration/src/connectors/fiuu/transformers.rs:569-573`, which is a separate untagged enum arm from the card-derived `FiuuPaymentMethodData` variants.

2. **Problem**: Wildcard `_ => Err(...)` in the `RealTimePaymentData` match arm swallows future variants silently.
   **Solution**: Enumerate every variant by name. Fiuu's handler at `crates/integrations/connector-integration/src/connectors/fiuu/transformers.rs:574-576` lists `Fps {} | PromptPay {} | VietQr {}` explicitly; when a fifth rail variant is added, the compiler flags this site.

3. **Problem**: Deserializing the connector's QR-flow response with the card-flow struct.
   **Solution**: Use `#[serde(untagged)]` demultiplexing as fiuu does at `crates/integrations/connector-integration/src/connectors/fiuu/transformers.rs:960-967` so that `DuitNowQrCodeResponse` and `PaymentsResponse` can both be accepted at the same endpoint.

4. **Problem**: Declaring RTP support in the connector trait matrix but forgetting to add a PSync implementation.
   **Solution**: RTP is always async — final status arrives via PSync or webhook. Register both flows in the `macros::create_all_prerequisites!` invocation, as iatapay does at `crates/integrations/connector-integration/src/connectors/iatapay.rs:237-246`.

5. **Problem**: Hardcoding `AttemptStatus::Charged` in the RTP response `TryFrom`.
   **Solution**: Map from the connector's status field. This is the anti-pattern explicitly banned by `PATTERN_AUTHORING_SPEC.md` §11.1. For iatapay, the mapping is the `impl From<IatapayPaymentStatus> for AttemptStatus` block at `crates/integrations/connector-integration/src/connectors/iatapay/transformers.rs:108-119`.

6. **Problem**: Using `Fps` in the codebase assuming the UK rail when iatapay routes it to Hong Kong.
   **Solution**: Until the enum gains a doc-comment or a PR disambiguates, follow iatapay's precedent (`Fps {}` → HK) for country-based routing in new connectors. If the UK rail needs explicit support, open an issue to split the variant (e.g. `FpsHk {}` / `FpsUk {}`) rather than overloading `Fps {}` with connector-specific interpretation.

7. **Problem**: Dropping the `Box<>` around `RealTimePaymentData` in a new transformer signature.
   **Solution**: The PM enum at `crates/types-traits/domain_types/src/payment_method_data.rs:263` is `RealTimePayment(Box<RealTimePaymentData>)`. Dereference it with `&**` (iatapay) or `*rtp_data.clone()` (fiuu) but do NOT change the upstream type — that would require edits to `payment_method_data.rs` and a rebuild of every connector.

## Cross-References

- Pattern Authoring Spec: [../../PATTERN_AUTHORING_SPEC.md](../../PATTERN_AUTHORING_SPEC.md) — the authoritative structural spec this pattern conforms to.
- Patterns index: [../../README.md](../../README.md)
- Authorize index: [../README.md](../README.md)
- Sibling PM pattern — UPI (adjacent instant/QR-rail concept, India): [../upi/pattern_authorize_upi.md](../upi/pattern_authorize_upi.md)
- Sibling PM pattern — BankTransfer (adjacent account-to-account concept, global): [../bank_transfer/pattern_authorize_bank_transfer.md](../bank_transfer/pattern_authorize_bank_transfer.md)
- Sibling PM pattern — Card (canonical reference shape): [../card/pattern_authorize_card.md](../card/pattern_authorize_card.md)
- Flow pattern — Authorize: [../../pattern_authorize.md](../../pattern_authorize.md)
- Flow pattern — PSync (required companion for RTP, since settlement is async): [../../pattern_psync.md](../../pattern_psync.md)
- Utility helpers (for `get_unimplemented_payment_method_error_message` and amount converters): [../../../utility_functions_reference.md](../../../utility_functions_reference.md)
- Canonical type signatures: [../../../types/types.md](../../../types/types.md)

---

## Change Log

| Date | Pinned SHA | Change |
|------|------------|--------|
| 2026-04-20 | `60540470cf84a350cc02b0d41565e5766437eb95` | Advanced Pinned SHA in header metadata from `ceb33736ce941775403f241f3f0031acbf2b4527` to `60540470cf84a350cc02b0d41565e5766437eb95`. Verification agent confirmed all 4 `RealTimePaymentData` variants (`DuitNow`, `Fps`, `PromptPay`, `VietQr`) remain present in the Variant Enumeration; no variant-level additions or removals at the new pin. Connector citations (fiuu DuitNow impl; iatapay country-resolution; NotImplemented stub connectors) remain valid at the new SHA. |
