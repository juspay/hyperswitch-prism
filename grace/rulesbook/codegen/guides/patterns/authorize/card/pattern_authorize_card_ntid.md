# Card Authorize Flow Pattern — Network Transaction ID (NTID) Sub-Pattern


## Overview

This sub-pattern documents the **Merchant-Initiated Transaction (MIT)** path that reuses a **Network Transaction ID (NTID)** from a prior **Customer-Initiated Transaction (CIT)**. It is a variant of the parent **Card** PM (see [`pattern_authorize_card.md`](./pattern_authorize_card.md)) and composes with the **RepeatPayment** flow (see [`pattern_repeat_payment_flow.md`](../../pattern_repeat_payment_flow.md)).

Concretely, a connector enters this sub-pattern when two conditions are met in the incoming `RouterDataV2`:

1. `request.mandate_reference == MandateReferenceId::NetworkMandateId(network_transaction_id)` — i.e. the orchestrator has routed a stored `network_transaction_id` string rather than a connector-scoped mandate id.
2. `request.payment_method_data == PaymentMethodData::CardDetailsForNetworkTransactionId(card)` — i.e. the raw PAN + expiry for the original card are replayed alongside the NTI (no CVV, no 3DS, no customer present).

The connector must then build an authorization request that (a) embeds the raw card details, (b) attaches the prior NTI as a "scheme reference" / "previous transaction id" / "original network transaction id" field whose name is connector-specific, (c) declares the interaction to the scheme as merchant-initiated, and (d) omits cardholder authentication signals (CVV, 3DS).

This sub-pattern **does not replace** the parent Card pattern; it augments it. All canonical request/response types, macro wiring, and status mapping from the parent pattern still apply. The only things this sub-pattern governs are the two match arms for the NTID variant and the MIT-signalling on the outgoing body.

### Key Characteristics

| Characteristic | Value | Citation |
|----------------|-------|----------|
| Triggering enum | `PaymentMethodData::CardDetailsForNetworkTransactionId` | `crates/types-traits/domain_types/src/payment_method_data.rs:250` |
| Paired mandate variant | `MandateReferenceId::NetworkMandateId(String)` | observed pairing in `adyen/transformers.rs:6351-6353`, `checkout/transformers.rs:898-900`, `fiuu/transformers.rs:760-762` |
| CVV present? | No — `card_cvc` is absent from the struct | `crates/types-traits/domain_types/src/payment_method_data.rs:1439-1450` |
| 3DS performed? | No — authentication was already performed on the CIT; MIT is out of scope | `worldpay/transformers.rs:276-277` (explicit `Ok(None)` for 3DS in NTI flow) |
| Scheme indicator | Connector-specific (`recurring`, `Unscheduled`, `SubsequentRecurring`, etc.) | Varies — see Connector Table |
| Customer interaction | Merchant-initiated / stored-credential — `shopper_interaction = ContinuedAuthentication` in Adyen | `adyen/transformers.rs:6309` |
| Whether flow is `Authorize` or `RepeatPayment` | Varies per connector — see Relationship section | — |

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

The NTID sub-pattern is the **third leg** of a three-flow lifecycle. The NTI itself is minted during the CIT (Authorize or SetupMandate) and is consumed during one or more MITs (RepeatPayment or Authorize-with-NetworkMandateId, depending on connector).

### Sequence (ASCII)

```
                 CIT (one-time)                           MIT (one or many)
                 ───────────────                          ───────────────────

┌─────────────┐                               ┌──────────────────────────────┐
│ Merchant    │                               │ Merchant                     │
│ app (CIT)   │                               │ app (MIT scheduler /         │
└─────┬───────┘                               │ recurring job)               │
      │ (1) PaymentsAuthorizeData<T>          └────────┬─────────────────────┘
      │     PaymentMethodData::Card(_)                 │ (5) RepeatPaymentData
      │     setup_future_usage = OffSession            │     mandate_reference =
      ▼                                                │     NetworkMandateId(NTI)
┌─────────────┐   (2) ProcessorResponse                │     payment_method_data =
│ RouterDataV2│   .network_transaction_id ───────┐     │     CardDetailsForNetworkTransactionId
│ <Authorize> │                                  │     ▼
└─────┬───────┘                                  │   ┌─────────────────────────┐
      │                                          │   │ RouterDataV2<RepeatPayment>
      │ — OR — (2') SetupMandate ─────────┐      │   │   or <Authorize> w/ NTI │
      │                                   │      │   └─────┬───────────────────┘
      ▼                                   │      │         │
┌─────────────┐                           │      │         ▼
│ Connector   │                           │      │   ┌──────────────────────────┐
│ CIT call    │                           │      │   │ Connector MIT call       │
│ (3DS, CVV)  │                           │      │   │ NO CVV, NO 3DS,          │
└─────┬───────┘                           │      │   │ scheme indicator =       │
      │ (3) response stores NTI          │      │   │   merchant-initiated,    │
      │     into the connector-mandate   │      │   │   previous NTI attached  │
      │     record / network_txn_id      │      │   └─────┬────────────────────┘
      ▼                                   │      │         │ (6) PaymentsResponseData
┌─────────────┐                           │      │         ▼    (status ∈ Charged/Authorized/Failure)
│ UCS store:  │ ◀─────────────────────────┘      │
│ {mandate_id,│                                   │
│  NTI}       │ ◀─────────────────────────────────┘
└─────────────┘
```

- Step **(1)–(3)** is the **CIT**. The prior payment's `network_transaction_id` is lifted from the processor response. For connectors that support `SetupMandate`, the CIT can be a zero-dollar verification (see [`pattern_setup_mandate.md`](../../pattern_setup_mandate.md)); otherwise the NTI is captured during a regular `Authorize`.
- Step **(4)** — not drawn — is the storage of the NTI. Cybersource writes it via `ProcessorResponse.network_transaction_id` at `cybersource/transformers.rs:2668`, with the assignment flowing through `cybersource/transformers.rs:2772` and `cybersource/transformers.rs:3735`. The NTI ends up on the orchestrator-side `recurring_mandate_payment_data`.
- Step **(5)** is this sub-pattern. The orchestrator injects `MandateReferenceId::NetworkMandateId(String)` and a freshly re-materialized `CardDetailsForNetworkTransactionId` (raw PAN + exp) into the next MIT RouterData. The connector unwraps both and builds the outgoing request.
- Step **(6)** is the MIT response. Status mapping is unchanged from the parent Card pattern — the connector should not hardcode `AttemptStatus::Charged` (see `PATTERN_AUTHORING_SPEC.md` §11 item 1).

### Which flow consumes the NTID?

Two distinct wiring strategies are observed in the codebase, and this sub-pattern covers both:

1. **Wired into `Authorize`** — the connector handles `MandateReferenceId::NetworkMandateId` inside the same `TryFrom` that builds its regular `Authorize` request. Adyen takes this approach (`adyen/transformers.rs:6351-6353`), as does Worldpay (`worldpay/transformers.rs:124`, `worldpay/transformers.rs:383`).
2. **Wired into `RepeatPayment`** — the connector exposes a dedicated RepeatPayment flow and matches `CardDetailsForNetworkTransactionId` inside the RepeatPayment `TryFrom`. Cybersource (`cybersource/transformers.rs:4305`), Checkout (`checkout/transformers.rs:900`), Fiuu (`fiuu/transformers.rs:762`), Novalnet (`novalnet/transformers.rs:2358`), and Revolv3 (`revolv3/transformers.rs:1120`) take this approach.

Either wiring is acceptable; the choice is driven by whether the connector's upstream API exposes a dedicated MIT endpoint or expects MIT signalling on the same endpoint as one-time Authorize. The sub-pattern is identical in both cases at the transformer-body level.

---

## Variant Enumeration

This sub-pattern qualifies a **single** variant of `PaymentMethodData<T>`, declared at `crates/types-traits/domain_types/src/payment_method_data.rs:250`:

| Variant | Data Shape | Citation | Used By (connectors) |
|---------|-----------|----------|----------------------|
| `PaymentMethodData::CardDetailsForNetworkTransactionId` | `CardDetailsForNetworkTransactionId` struct | `crates/types-traits/domain_types/src/payment_method_data.rs:1439-1450` | adyen, checkout, cybersource, fiuu, novalnet, revolv3, worldpay (see table below) |

There is no alternate variant for the card-NTID path. Connectors that do not support NTID replay map this variant to `IntegrationError::not_implemented` (observed pattern in bankofamerica, braintree, paypal, trustpay, billwerk, dlocal, and others — see [Best Practices §5](#best-practices)).

---

## Field Enumeration

Definition at `crates/types-traits/domain_types/src/payment_method_data.rs:1439-1450`:

```rust
// From crates/types-traits/domain_types/src/payment_method_data.rs:1438
#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize, Default)]
pub struct CardDetailsForNetworkTransactionId {
    pub card_number: cards::CardNumber,
    pub card_exp_month: Secret<String>,
    pub card_exp_year: Secret<String>,
    pub card_issuer: Option<String>,
    pub card_network: Option<CardNetwork>,
    pub card_type: Option<String>,
    pub card_issuing_country: Option<String>,
    pub bank_code: Option<Secret<String>>,
    pub nick_name: Option<Secret<String>>,
    pub card_holder_name: Option<Secret<String>>,
}
```

| Field | Type | Required? | Notes | Citation |
|-------|------|-----------|-------|----------|
| `card_number` | `cards::CardNumber` | yes | Raw PAN. Unlike `Card<T>`, this is **not** generic over `PaymentMethodDataTypes` — it is always a raw `cards::CardNumber` because the scheme needs the clear PAN to match against the prior NTI. | `payment_method_data.rs:1440` |
| `card_exp_month` | `Secret<String>` | yes | Two-digit month string; helpers in impl block. | `payment_method_data.rs:1441` |
| `card_exp_year` | `Secret<String>` | yes | Two- or four-digit year string; helpers return both forms. | `payment_method_data.rs:1442` |
| `card_issuer` | `Option<String>` | no | Populated from bin-lookup if available; read by connectors like Adyen to pick card-brand code. | `payment_method_data.rs:1443` |
| `card_network` | `Option<CardNetwork>` | no | Used for scheme-aware routing. Adyen reads this at `adyen/transformers.rs:6357` before falling back to `get_card_issuer()`. | `payment_method_data.rs:1444` |
| `card_type` | `Option<String>` | no | Credit / debit string. | `payment_method_data.rs:1445` |
| `card_issuing_country` | `Option<String>` | no | ISO alpha-2 country. | `payment_method_data.rs:1446` |
| `bank_code` | `Option<Secret<String>>` | no | Scheme-internal bank code; rarely populated. | `payment_method_data.rs:1447` |
| `nick_name` | `Option<Secret<String>>` | no | Card nickname for display only. | `payment_method_data.rs:1448` |
| `card_holder_name` | `Option<Secret<String>>` | no | Split in Checkout via `split_account_holder_name` at `checkout/transformers.rs:901-902` when populated. | `payment_method_data.rs:1449` |

### Helper methods

The impl block at `crates/types-traits/domain_types/src/payment_method_data.rs:1452-1546` provides:

| Method | Returns | Citation |
|--------|---------|----------|
| `get_card_expiry_year_2_digit()` | `Result<Secret<String>, IntegrationError>` — last 2 digits of year | `payment_method_data.rs:1453-1467` |
| `get_card_issuer()` | `Result<CardIssuer, Report<IntegrationError>>` — via `get_card_issuer(pan)` | `payment_method_data.rs:1468-1470` |
| `get_card_expiry_month_year_2_digit_with_delimiter(delim)` | `Result<Secret<String>, _>` — e.g. `"12/25"` | `payment_method_data.rs:1471-1482` |
| `get_expiry_date_as_yyyymm(delim)` | `Secret<String>` — e.g. `"2025-12"` | `payment_method_data.rs:1483-1491` |
| `get_expiry_date_as_mmyyyy(delim)` | `Secret<String>` — e.g. `"12/2025"` | `payment_method_data.rs:1492-1500` |
| `get_expiry_year_4_digit()` | `Secret<String>` — upgrades `"25"` to `"2025"` | `payment_method_data.rs:1501-1507` |
| `get_expiry_date_as_yymm()` | `Result<Secret<String>, _>` — e.g. `"2512"` | `payment_method_data.rs:1508-1512` |
| `get_expiry_date_as_mmyy()` | `Result<Secret<String>, _>` — e.g. `"1225"` | `payment_method_data.rs:1513-1517` |
| `get_expiry_month_as_i8()` | `Result<Secret<i8>, Error>` | `payment_method_data.rs:1518-1531` |
| `get_expiry_year_as_i32()` | `Result<Secret<i32>, Error>` | `payment_method_data.rs:1532-1545` |

Implementers MUST use these helpers rather than reformatting expiry dates inline — see [`pattern_authorize_card.md`](./pattern_authorize_card.md) "Expiry Date Formats" for the broader rationale.

---

## Architecture Overview

### Request / Response types

The NTID sub-pattern does **not** introduce new `RouterDataV2` type arguments. It reuses either the parent PM's Authorize tuple or the RepeatPayment tuple from §7 of the spec:

```rust
// Wiring 1: inside Authorize — used by adyen, worldpay
RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>

// Wiring 2: inside RepeatPayment — used by cybersource, checkout, fiuu, novalnet, revolv3
RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
```

Both are drawn verbatim from the canonical signatures in [`PATTERN_AUTHORING_SPEC.md`](../../PATTERN_AUTHORING_SPEC.md) §7. The trait implemented is still `ConnectorIntegrationV2<Flow, FlowData, RequestData, ResponseData>`.

### Where the variant is unwrapped

```rust
// Wiring 1 shape (from adyen/transformers.rs:6351-6356)
match mandate_ref_id {
    MandateReferenceId::NetworkMandateId(network_mandate_id) => {
        match &item.router_data.request.payment_method_data {
            PaymentMethodData::CardDetailsForNetworkTransactionId(
                ref card_details_for_network_transaction_id,
            ) => { /* build connector card with NTI */ }
            _ => { /* IntegrationError::NotSupported */ }
        }
    }
    // ...
}

// Wiring 2 shape (from checkout/transformers.rs:898-900)
match &item.router_data.request.mandate_reference {
    MandateReferenceId::NetworkMandateId(network_transaction_id) => {
        match item.router_data.request.payment_method_data {
            PaymentMethodData::CardDetailsForNetworkTransactionId(ref card_details) => {
                /* build connector card with NTI */
            }
            // ...
        }
    }
    // ...
}
```

Always nest the match `MandateReferenceId` first, then destructure on `PaymentMethodData`. Inverting this nesting produces harder-to-read error branches (see [Common Errors §3](#common-errors)).

### `ProcessingInformation` / MIT-signalling

Each connector adds a `commerce_indicator` / `processing_type` / `type` field whose value tells the scheme that this is an MIT. Values observed:

| Connector | Field | Value for NTI MIT | Citation |
|-----------|-------|--------------------|----------|
| Adyen | `AdyenShopperInteraction` | `ContinuedAuthentication` | `adyen/transformers.rs:6309` |
| Cybersource | `commerce_indicator` | `"recurring"` | `cybersource/transformers.rs:4792` |
| Cybersource | `merchant_initiated_transaction.reason` | `Some("7".to_string())` | `cybersource/transformers.rs:4803` |
| Checkout | `CheckoutPaymentType` | `Unscheduled` / `Recurring` / `Installment` (driven by `mit_category`) | `checkout/transformers.rs:916-927` |
| Revolv3 | `PaymentProcessingType` | `Recurring` | `revolv3/transformers.rs:1115` |
| Worldpay | `CustomerAgreementType` | `Unscheduled` | `worldpay/transformers.rs:391` |

All of these are standard scheme-level MIT indicators. Hardcoding them at the transformer level (rather than mapping from `RepeatPaymentData.mit_category` or explicit config) is acceptable when the connector's NTID flow is definitionally MIT.

---

## Connectors with Full Implementation

Only connectors that **materially construct** a request body from `CardDetailsForNetworkTransactionId` are listed. Connectors that return `IntegrationError::not_implemented` for this variant are excluded (they appear in the broader Card pattern but have no NTID implementation).

| Connector | HTTP Method | Content Type | URL Pattern | Request Type Reuse | Notes |
|-----------|-------------|--------------|-------------|--------------------|-------|
| **Adyen** | POST | `application/json` | `v68/payments` (Authorize endpoint) | Reuses `AdyenPaymentRequest` — same struct as one-time Authorize; NTI rides on `AdyenCard.network_payment_reference` | See `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:6353-6397`. Wired into `flow: Authorize` at `adyen.rs` — no dedicated RepeatPayment variant struct. CVV is set to `None` at `adyen/transformers.rs:6390`. Card brand chosen from `card_network` with fallback to `get_card_issuer()` at `adyen/transformers.rs:6356-6374`. |
| **Checkout** | POST | `application/json` | `payments` (same endpoint as Authorize) | Reuses `PaymentsRequest<T>`; source variant is `PaymentSource::RawCardForNTI(CheckoutRawCardDetails)` | See `checkout/transformers.rs:900-934`. Wired into **RepeatPayment** flow at `checkout.rs:240-244`. NTI is carried in `previous_payment_id: Option<String>` at `checkout/transformers.rs:304` and assigned at `checkout/transformers.rs:930`. `merchant_initiated = Some(true)` at `checkout/transformers.rs:931`. `cvv: None` at `checkout/transformers.rs:909`. Account-holder name is split via `split_account_holder_name` at `checkout/transformers.rs:901-914`. |
| **Cybersource** | POST | `application/json;charset=utf-8` | `pts/v2/payments/` (same as Authorize) | Dedicated `CybersourceRepeatPaymentRequest`; inner `RepeatPaymentInformation::Cards(CardWithNtiPaymentInformation)` | See `cybersource/transformers.rs:4305-4307` (dispatch), `cybersource/transformers.rs:4424-4492` (builder), and `cybersource/transformers.rs:4471-4480` (card shape). `security_code: None` is explicit at `cybersource/transformers.rs:4476`. `type_selection_indicator: Some("1".to_owned())` at `cybersource/transformers.rs:4478`. `previous_transaction_id: Some(Secret::new(network_transaction_id))` at `cybersource/transformers.rs:4805`. `commerce_indicator = "recurring"` at `cybersource/transformers.rs:4792`. |
| **Fiuu** | POST | form-url-encoded (Fiuu API) | `RMS/API/Direct/1.4.0/index.php` | Reuses `FiuuPaymentRequest<T>`; inner `FiuuPaymentMethodData` from `(&CardDetailsForNetworkTransactionId, String)` | See `fiuu/transformers.rs:762-767`. Wired into RepeatPayment flow at `fiuu.rs:263`. Dispatch sits inside `MandateReferenceId::NetworkMandateId(network_transaction_id)` arm at `fiuu/transformers.rs:760`. |
| **Novalnet** | POST | `application/json` | Novalnet payment endpoint | Reuses `NovalnetPaymentsRequest<T>`; inner `NovalNetPaymentData::RawCardForNTI(NovalnetRawCardDetails { scheme_tid, ... })` | See `novalnet/transformers.rs:2358-2396`. `scheme_tid: network_transaction_id.into()` at `novalnet/transformers.rs:2364`. `payment_type: NovalNetPaymentTypes::CREDITCARD` at `novalnet/transformers.rs:2369`. Wired into RepeatPayment flow at `novalnet.rs:256`. |
| **Revolv3** | POST | `application/json` | Revolv3 `/sale` or `/auth` | Dedicated `Revolv3RepeatPaymentRequest<T>`; inner `Revolv3PaymentMethodData::set_credit_card_data_for_ntid` | See `revolv3/transformers.rs:1119-1137`. 3DS is rejected explicitly for NTID at `revolv3/transformers.rs:1121-1127` (MIT is definitionally no-3DS). `NetworkProcessingData { processing_type: Some(PaymentProcessingType::Recurring), original_network_transaction_id: item.router_data.request.get_network_mandate_id() }` at `revolv3/transformers.rs:1114-1117`. Wired into RepeatPayment flow at `revolv3.rs:444`. |
| **Worldpay** | POST | `application/json` | Worldpay payments endpoint | Reuses `WorldpayAuthorizeRequest<T>`; inner `PaymentInstrument::RawCardForNTI(RawCardDetails)` plus `CustomerAgreement` | See `worldpay/transformers.rs:124-143` (card unwrap) and `worldpay/transformers.rs:383-398` (CustomerAgreement). `scheme_reference: Some(network_transaction_id.into())` at `worldpay/transformers.rs:392`. `agreement_type: CustomerAgreementType::Unscheduled` at `worldpay/transformers.rs:391`. 3DS is suppressed for NTI at `worldpay/transformers.rs:277`. Wired into **Authorize** flow at `worldpay.rs:203`. |

### Stub Implementations

Connectors that accept the `CardDetailsForNetworkTransactionId` variant only via a fall-through `NotImplemented` arm:

- aci (`aci/transformers.rs:751`)
- adyen — on non-NTI branches, e.g. `adyen/transformers.rs:3702`, `adyen/transformers.rs:6043`
- bambora (`bambora/transformers.rs:300`)
- bankofamerica (`bankofamerica/transformers.rs:617`, `bankofamerica/transformers.rs:1781`)
- billwerk (`billwerk/transformers.rs:237`)
- braintree (`braintree/transformers.rs:614`, `braintree/transformers.rs:1611`, `braintree/transformers.rs:2633`, `braintree/transformers.rs:2816`)
- cryptopay (`cryptopay/transformers.rs:113`)
- dlocal (`dlocal/transformers.rs:211`)
- fiserv (`fiserv/transformers.rs:552`)
- forte (`forte/transformers.rs:315`)
- hipay (`hipay/transformers.rs:598`)
- loonio (`loonio/transformers.rs:244`)
- mifinity (`mifinity/transformers.rs:251`)
- multisafepay (`multisafepay/transformers.rs:159`, `multisafepay/transformers.rs:339`)
- nexinets (`nexinets/transformers.rs:743`)
- paypal (`paypal/transformers.rs:1145`, `paypal/transformers.rs:2603`)
- razorpay (`razorpay/transformers.rs:305`)
- redsys (`redsys/transformers.rs:251`)
- trustpay (`trustpay/transformers.rs:1713`)
- volt (`volt/transformers.rs:298`)

---

## Per-Variant Implementation Notes

This sub-pattern qualifies one variant — `CardDetailsForNetworkTransactionId`. This section details the per-connector quirks.

### Adyen (Authorize-wired)

Adyen does not expose a dedicated RepeatPayment flow for NTID; the `Authorize` transformer handles both CIT and NTID-MIT. The key entry is at `adyen/transformers.rs:6351-6397`. The outgoing `AdyenCard` is built with:

- `number` from `card_details.card_number` (`adyen/transformers.rs:6379-6381`),
- `expiry_month` from `card_details.card_exp_month` (`adyen/transformers.rs:6384-6386`),
- `expiry_year` from `card_details.get_expiry_year_4_digit()` (`adyen/transformers.rs:6387-6389`),
- `cvc: None` (`adyen/transformers.rs:6390`),
- `holder_name` = test-override `test_holder_name` falling back to billing full name (`adyen/transformers.rs:6391`, `adyen/transformers.rs:6375-6378`),
- `brand` from `card_network` fallback to `CardBrand::try_from(&get_card_issuer()?)` (`adyen/transformers.rs:6356-6374`),
- `network_payment_reference: Some(Secret::new(network_mandate_id))` (`adyen/transformers.rs:6393`).

The outer `AdyenPaymentRequest` also sets `shopper_interaction = AdyenShopperInteraction::ContinuedAuthentication` (`adyen/transformers.rs:6309`) — this is the scheme-level "this is MIT" flag.

### Checkout (RepeatPayment-wired)

Checkout exposes a dedicated RepeatPayment flow (`checkout.rs:240-244`) but **reuses the Authorize request struct** `PaymentsRequest<T>`. The NTID branch is at `checkout/transformers.rs:900-934`:

- `PaymentSource::RawCardForNTI(CheckoutRawCardDetails)` is selected (`checkout/transformers.rs:904`).
- `cvv: None` (`checkout/transformers.rs:909`) — MIT has no CVV.
- `previous_payment_id = Some(network_transaction_id.clone())` (`checkout/transformers.rs:930`) — this is the NTI.
- `merchant_initiated = Some(true)` (`checkout/transformers.rs:931`).
- `payment_type` is mapped from `request.mit_category`:
  - `Some(Installment)` → `CheckoutPaymentType::Installment` (`checkout/transformers.rs:917-919`),
  - `Some(Recurring)` → `CheckoutPaymentType::Recurring` (`checkout/transformers.rs:920-922`),
  - `Some(Unscheduled) | None` → `CheckoutPaymentType::Unscheduled` (`checkout/transformers.rs:923-925`).

### Cybersource (RepeatPayment-wired)

Cybersource has the most structured NTID implementation. The RepeatPayment dispatch at `cybersource/transformers.rs:4305-4307` delegates to a 3-arg `TryFrom` at `cybersource/transformers.rs:4424-4492`. Key fields:

- `RepeatPaymentInformation::Cards(Box::new(CardWithNtiPaymentInformation { card: CardWithNti { number, expiration_month, expiration_year, security_code: None, card_type, type_selection_indicator: Some("1") } }))` (`cybersource/transformers.rs:4471-4480`).
- `card_type` is derived via `card_issuer_to_string(ccard.get_card_issuer()?)` (`cybersource/transformers.rs:4464-4468`).
- The NTI rides on `processing_information.authorization_options.merchant_initiated_transaction.previous_transaction_id` at `cybersource/transformers.rs:4805`.
- `merchant_initiated_transaction.reason = Some("7".to_string())` (`cybersource/transformers.rs:4803`) — Cybersource-specific MIT reason code.
- `initiator_type: Some(CybersourcePaymentInitiatorTypes::Merchant)` and `stored_credential_used: Some(true)` at `cybersource/transformers.rs:4797-4800`.
- `commerce_indicator` is overwritten to `"recurring"` at `cybersource/transformers.rs:4792`.
- For Discover (card-network code `"004"`), Cybersource requires `original_authorized_amount` sourced from `recurring_mandate_payment_data` at `cybersource/transformers.rs:4748-4762` — other networks treat it as optional.

### Fiuu (RepeatPayment-wired)

Fiuu delegates entirely to a tuple `TryFrom`:

```rust
// From crates/integrations/connector-integration/src/connectors/fiuu/transformers.rs:762-767
PaymentMethodData::CardDetailsForNetworkTransactionId(ref raw_card_details) => {
    FiuuPaymentMethodData::try_from((
        raw_card_details,
        network_transaction_id.clone(),
    ))
}
```

The outer `FiuuPaymentRequest<T>` is identical to the Authorize request. Signature calculation (`fiuu/transformers.rs:743-746`) is unchanged. Transaction-type selection uses `is_auto_capture()` (`fiuu/transformers.rs:747-750`).

### Novalnet (RepeatPayment-wired)

Novalnet's RepeatPayment path reuses `NovalnetPaymentsRequest<T>` but picks a different inner variant:

```rust
// From crates/integrations/connector-integration/src/connectors/novalnet/transformers.rs:2358-2365
PaymentMethodData::CardDetailsForNetworkTransactionId(ref raw_card_details) => {
    let novalnet_card =
        NovalNetPaymentData::RawCardForNTI(NovalnetRawCardDetails {
            card_number: raw_card_details.card_number.clone(),
            card_expiry_month: raw_card_details.card_exp_month.clone(),
            card_expiry_year: raw_card_details.card_exp_year.clone(),
            scheme_tid: network_transaction_id.into(),
        });
```

The outer transaction is `NovalNetPaymentTypes::CREDITCARD` (`novalnet/transformers.rs:2369`) with `create_token: Some(CREATE_TOKEN_REQUIRED)` (`novalnet/transformers.rs:2382`) — Novalnet mints a new token on each MIT.

### Revolv3 (RepeatPayment-wired)

Revolv3 is the only connector in the cohort that **rejects 3DS inside the NTID branch** with an explicit typed error:

```rust
// From crates/integrations/connector-integration/src/connectors/revolv3/transformers.rs:1121-1127
if item.router_data.resource_common_data.is_three_ds() {
    Err(IntegrationError::NotSupported {
        message: "Cards No3DS".to_string(),
        connector: "revolv3",
        context: Default::default(),
    })?
};
```

This is defensive — the orchestrator should never route a 3DS flag through MIT — but it codifies the invariant. Revolv3 also uses the generic `request.get_network_mandate_id()` accessor to fetch the NTI rather than destructuring `MandateReferenceId` (`revolv3/transformers.rs:1116`), which is a valid alternative style.

### Worldpay (Authorize-wired)

Worldpay handles both CIT and NTID-MIT inside the `Authorize` flow. The card-unwrap branch at `worldpay/transformers.rs:124-143` produces `PaymentInstrument::RawCardForNTI(RawCardDetails { payment_type: PaymentType::Plain, expiry_date, card_number })` — no CVV field exists on this variant. The scheme-reference attachment happens in a separate helper at `worldpay/transformers.rs:383-398`, where a `CustomerAgreement { agreement_type: Unscheduled, scheme_reference: Some(network_transaction_id.into()), stored_card_usage: None }` is emitted when `MandateReferenceId::NetworkMandateId` is present. 3DS is suppressed at `worldpay/transformers.rs:276-277`.

---

## Common Implementation Patterns

### 1. Dual-match skeleton

Every connector that supports the NTID sub-pattern uses a nested match of the form:

```rust
match &router_data.request.mandate_reference {
    MandateReferenceId::NetworkMandateId(network_transaction_id) => {
        match &router_data.request.payment_method_data {
            PaymentMethodData::CardDetailsForNetworkTransactionId(ref card) => {
                // build outgoing body with
                //   raw card number + expiry
                //   NTI attached to the scheme-reference field
                //   CVV absent
                //   3DS absent
                //   MIT indicator set
            }
            _ => Err(IntegrationError::not_implemented(...).into()),
        }
    }
    _ => { /* handle other MandateReferenceId variants or fall through */ }
}
```

### 2. CVV handling

CVV is **always** absent in the NTID path. Connectors express this differently:

| Connector | How CVV absence is expressed | Citation |
|-----------|------------------------------|----------|
| Adyen | `cvc: None` on `AdyenCard` | `adyen/transformers.rs:6390` |
| Checkout | `cvv: None` on `CheckoutRawCardDetails` | `checkout/transformers.rs:909` |
| Cybersource | `security_code: None` on `CardWithNti` | `cybersource/transformers.rs:4476` |
| Worldpay | `RawCardDetails` struct has no `cvv` field at all | `worldpay/transformers.rs:135-142` |

This is not optional — the scheme explicitly forbids CVV on MIT-via-NTI because the customer is not present.

### 3. Expiry date formatting

Always use the `CardDetailsForNetworkTransactionId` helper methods (`payment_method_data.rs:1452-1546`) rather than inline `format!`. Seen in practice:

- Adyen uses `get_expiry_year_4_digit()` at `adyen/transformers.rs:6387-6389`.
- Worldpay uses `get_expiry_month_as_i8()` + `get_expiry_year_4_digit().peek().parse::<i32>()` at `worldpay/transformers.rs:126-133`.
- Cybersource passes `card_exp_month` and `card_exp_year` through unmodified at `cybersource/transformers.rs:4474-4475` (Cybersource accepts the raw two-digit strings).

### 4. Card-brand resolution

For schemes that require a brand code alongside the PAN:

```rust
// From crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:6356-6374
let brand = match card_details_for_network_transaction_id
    .card_network
    .clone()
    .and_then(get_adyen_card_network)
{
    Some(card_network) => card_network,
    None => CardBrand::try_from(
        &card_details_for_network_transaction_id
            .get_card_issuer()
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?,
    )
    .change_context(IntegrationError::RequestEncodingFailed {
        context: Default::default(),
    })?,
};
```

Pattern: prefer `card_network`, fall back to `get_card_issuer()` (which runs BIN detection on the PAN). This mirrors the parent Card pattern's "Card Network Mapping" section (`pattern_authorize_card.md:530-570`).

### 5. Scheme-reference attachment

Every connector has a single JSON / form field that carries the NTI into the outgoing body. Name varies:

| Connector | Field path | Citation |
|-----------|------------|----------|
| Adyen | `AdyenCard.network_payment_reference` | `adyen/transformers.rs:6393` |
| Checkout | top-level `PaymentsRequest.previous_payment_id` | `checkout/transformers.rs:930` |
| Cybersource | `processing_information.authorization_options.merchant_initiated_transaction.previous_transaction_id` | `cybersource/transformers.rs:4805` |
| Novalnet | `NovalnetRawCardDetails.scheme_tid` | `novalnet/transformers.rs:2364` |
| Revolv3 | `NetworkProcessingData.original_network_transaction_id` | `revolv3/transformers.rs:1116` |
| Worldpay | `CustomerAgreement.scheme_reference` | `worldpay/transformers.rs:392` |

Implementers SHOULD document the chosen field in a comment next to the assignment.

---

## Code Examples

### Example 1: Minimal NTID transformer (RepeatPayment wiring)

Adapted from the Cybersource pattern at `cybersource/transformers.rs:4424-4492`:

```rust
// Wiring 2: RepeatPayment flow — NTID card path
impl<T> TryFrom<(
    &MyConnectorRouterData<
        RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        T,
    >,
    &CardDetailsForNetworkTransactionId,
)> for MyConnectorRepeatPaymentRequest
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        (item, card): (
            &MyConnectorRouterData<
                RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
                T,
            >,
            &CardDetailsForNetworkTransactionId,
        ),
    ) -> Result<Self, Self::Error> {
        // Extract the NTI from the outer request
        let nti = match &item.router_data.request.mandate_reference {
            MandateReferenceId::NetworkMandateId(nti) => nti.clone(),
            _ => return Err(IntegrationError::MissingRequiredField {
                field_name: "network_transaction_id",
                context: Default::default(),
            }.into()),
        };

        // Build the scheme card (no CVV, no 3DS)
        let card_body = MyConnectorCardNti {
            number: card.card_number.clone(),
            expiry_month: card.card_exp_month.clone(),
            expiry_year: card.get_expiry_year_4_digit(),
            brand: card.card_network.clone().map(map_network),
            // CVV deliberately absent per scheme rules for MIT
        };

        Ok(Self {
            card: card_body,
            // The NTI attaches here — rename per your API's field
            previous_transaction_id: Some(Secret::new(nti)),
            // MIT indicator
            merchant_initiated: true,
            amount: item.amount.clone(),
            currency: item.router_data.request.currency,
        })
    }
}
```

### Example 2: Authorize-wired NTID (Adyen style)

```rust
// Wiring 1: Authorize flow — NTID card branch
// Adapted from crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:6328-6406
match mandate_ref_id {
    MandateReferenceId::NetworkMandateId(network_mandate_id) => {
        match &item.router_data.request.payment_method_data {
            PaymentMethodData::CardDetailsForNetworkTransactionId(ref c) => {
                let brand = c
                    .card_network
                    .clone()
                    .and_then(get_adyen_card_network)
                    .map(Ok)
                    .unwrap_or_else(|| CardBrand::try_from(&c.get_card_issuer()?))?;

                let adyen_card = AdyenCard {
                    number: RawCardNumber(c.card_number.clone()),
                    expiry_month: c.card_exp_month.clone(),
                    expiry_year: c.get_expiry_year_4_digit(),
                    cvc: None,
                    holder_name: item
                        .router_data
                        .resource_common_data
                        .get_optional_billing_full_name(),
                    brand: Some(brand),
                    network_payment_reference: Some(Secret::new(network_mandate_id)),
                };
                PaymentMethod::AdyenPaymentMethod(Box::new(
                    AdyenPaymentMethod::AdyenCard(Box::new(adyen_card)),
                ))
            }
            _ => return Err(IntegrationError::NotSupported {
                message: "Network tokenization for payment method".to_string(),
                connector: "Adyen",
                context: Default::default(),
            }.into()),
        }
    }
    // ... other MandateReferenceId arms
}
```

Note that the outer `AdyenPaymentRequest` must also set `shopper_interaction = ContinuedAuthentication` — that field is constructed earlier in the same `TryFrom` at `adyen/transformers.rs:6309`.

### Example 3: 3DS guard for NTID

Revolv3's explicit rejection of 3DS-on-MIT is worth copying verbatim when the connector's scheme contract forbids it:

```rust
// From crates/integrations/connector-integration/src/connectors/revolv3/transformers.rs:1119-1132
PaymentMethodData::CardDetailsForNetworkTransactionId(ref card_data) => {
    if item.router_data.resource_common_data.is_three_ds() {
        Err(IntegrationError::NotSupported {
            message: "Cards No3DS".to_string(),
            connector: "revolv3",
            context: Default::default(),
        })?
    };
    Revolv3PaymentMethodData::set_credit_card_data_for_ntid(
        card_data.clone(),
        &item.router_data.resource_common_data,
    )?
}
```

---

## Best Practices

- **Always match `MandateReferenceId::NetworkMandateId` first, then `PaymentMethodData::CardDetailsForNetworkTransactionId`.** Inverting the nesting obscures the contract that these two must co-occur. See `adyen/transformers.rs:6351-6356` and `checkout/transformers.rs:898-900` for the established ordering.
- **Set CVV to `None` explicitly** or omit the field entirely from your NTI card struct. Never pass `card_cvc` through — the struct doesn't have it (`payment_method_data.rs:1439-1450`) and the scheme forbids it. See `adyen/transformers.rs:6390`, `cybersource/transformers.rs:4476`, `checkout/transformers.rs:909`.
- **Use the `CardDetailsForNetworkTransactionId` helper methods for expiry formatting** — never reimplement YYMM / YYYYMM slicing inline. The helpers at `payment_method_data.rs:1452-1546` handle 2- and 4-digit year inputs uniformly.
- **Do not reuse the NTI as a connector-scoped mandate id.** NTI and `connector_mandate_id` are distinct concepts: NTI is scheme-issued (14 digits for Visa, etc.), `connector_mandate_id` is connector-issued. Mixing them produces hard-to-debug downgrades. Compare the distinct fields at `cybersource/transformers.rs:4364` (`CybersoucrePaymentInstrument.id = connector_mandate_id`) vs `cybersource/transformers.rs:4805` (`previous_transaction_id = network_transaction_id`).
- **Map `RepeatPaymentData.mit_category` to the connector's scheme indicator when one exists.** Checkout does this at `checkout/transformers.rs:916-927`; cybersource's `"recurring"` `commerce_indicator` at `cybersource/transformers.rs:4792` is a simpler fixed mapping. See also the RepeatPayment pattern's "2a. MIT variants observed in the 2026-04-20 cohort" section in [`pattern_repeat_payment_flow.md`](../../pattern_repeat_payment_flow.md).
- **If the connector does not support NTID, return `IntegrationError::not_implemented` or `IntegrationError::NotSupported`** rather than silently building a CIT-shaped body. This is the established fall-through pattern — see `aci/transformers.rs:751`, `bankofamerica/transformers.rs:617`, `trustpay/transformers.rs:1713`.
- **Follow the parent pattern's status-mapping rules.** The MIT response still flows through the same `PaymentsResponseData` / `AttemptStatus` mapping as the parent Card pattern; do not hardcode `Charged`. See [`pattern_authorize_card.md`](./pattern_authorize_card.md) "Status Mapping" and `PATTERN_AUTHORING_SPEC.md` §11 item 1.
- **Use `utility_functions_reference.md` helpers** for any card-issuer lookup that you do not already have on `CardDetailsForNetworkTransactionId::get_card_issuer()` (`payment_method_data.rs:1468-1470`). See [`../../utility_functions_reference.md`](../../utility_functions_reference.md).

---

## Common Errors

1. **Problem**: Sending CVV in the MIT request body.
   **Solution**: Set the corresponding field to `None` on the outgoing struct. `CardDetailsForNetworkTransactionId` does not carry a CVC field at all (`payment_method_data.rs:1439-1450`), so there is nothing to copy over. Most scheme acquirers soft-decline MITs that carry a CVC.

2. **Problem**: Attaching the NTI to the wrong field (e.g. `connector_mandate_id` instead of `previous_transaction_id`).
   **Solution**: Audit the assignment against the connector's API docs; the two fields are mutually exclusive for MIT. See the field table in §7 above.

3. **Problem**: Inverted match nesting — destructuring `PaymentMethodData` first, then `MandateReferenceId`.
   **Solution**: Keep `MandateReferenceId` on the outside. Checkout uses the correct shape at `checkout/transformers.rs:898-900`; Adyen at `adyen/transformers.rs:6351-6353`. This keeps the error branches symmetric with the other mandate variants (`ConnectorMandateId`, `NetworkTokenWithNTI`).

4. **Problem**: Forwarding `is_three_ds() == true` into an MIT body.
   **Solution**: Either suppress the 3DS block entirely (Worldpay — `worldpay/transformers.rs:276-277`: `Ok(None)`) or reject with `IntegrationError::NotSupported` (Revolv3 — `revolv3/transformers.rs:1121-1127`).

5. **Problem**: Hardcoding `AttemptStatus::Charged` on the MIT response.
   **Solution**: Route the response through the same status mapper used for the parent Card pattern. Any hardcoded status violates `PATTERN_AUTHORING_SPEC.md` §11 item 1.

6. **Problem**: Forgetting Discover's `original_authorized_amount` requirement (Cybersource only).
   **Solution**: Branch on card-network code `"004"` and read `recurring_mandate_payment_data.get_original_payment_amount()` / `.get_original_payment_currency()` — see `cybersource/transformers.rs:4748-4762`. Other networks treat the field as optional.

7. **Problem**: Using `card.get_expiry_year_4_digit().peek().clone()` twice and concatenating manually.
   **Solution**: Use `get_expiry_date_as_yymm()`, `get_expiry_date_as_mmyyyy(delim)`, or `get_expiry_date_as_yyyymm(delim)` from the impl block at `payment_method_data.rs:1483-1517`.

---

## Cross-References

- Parent PM pattern: [`./pattern_authorize_card.md`](./pattern_authorize_card.md)
- Parent flow pattern: [`../../pattern_authorize.md`](../../pattern_authorize.md)
- Composed flow pattern: [`../../pattern_repeat_payment_flow.md`](../../pattern_repeat_payment_flow.md)
- Upstream CIT pattern: [`../../pattern_setup_mandate.md`](../../pattern_setup_mandate.md)
- Sibling NTID sub-pattern (wallet variant): [`../wallet/pattern_authorize_wallet_ntid.md`](../wallet/pattern_authorize_wallet_ntid.md)
- Sibling PM pattern: [`../wallet/pattern_authorize_wallet.md`](../wallet/pattern_authorize_wallet.md)
- Authorize-index: [`../README.md`](../README.md)
- Patterns-index: [`../../README.md`](../../README.md)
- Pattern authoring spec: [`../../PATTERN_AUTHORING_SPEC.md`](../../PATTERN_AUTHORING_SPEC.md)
- Utility helpers: [`../../../utility_functions_reference.md`](../../../utility_functions_reference.md)
- Types reference: [`../../../types/types.md`](../../../types/types.md)
