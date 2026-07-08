# Voucher Authorize Flow Pattern

## Overview

Voucher payments are an asynchronous, cash-adjacent payment category used to settle online purchases through an offline channel. The customer selects a voucher method at checkout; the acquirer responds with a reference number, barcode or digitable line; the customer then presents that reference at a physical store, ATM, or banking app to pay in cash or from a bank balance. UCS surfaces the family through `PaymentMethodData::Voucher(VoucherData)` — see `crates/types-traits/domain_types/src/payment_method_data.rs:265` — and the continuation data through `VoucherNextStepData` at `crates/types-traits/domain_types/src/payment_method_data.rs:415`.

### Key Characteristics

| Attribute | Value |
|-----------|-------|
| Flow Shape | Async: authorize returns pending + next-step metadata; real settlement arrives via webhook or PSync |
| Customer Experience | Merchant displays barcode / digitable line / URL; customer pays offline |
| Amount Unit (typical) | `MinorUnit` or `StringMajorUnit` depending on connector |
| Redirection | Usually `None` — this is a "present-to-shopper" flow, not a browser redirect |
| Next-Step Metadata | Populated into `connector_metadata` via `VoucherNextStepData` — see `crates/types-traits/domain_types/src/payment_method_data.rs:417` |
| Regional Scope | Brazil (Boleto), Mexico (Oxxo), Colombia (Efecty, PagoEfectivo), Chile (RedCompra, RedPagos), Indonesia (Alfamart, Indomaret), Japan (7-Eleven, Lawson, MiniStop, FamilyMart, Seicomart, PayEasy) |
| Canonical Adyen Response | `PresentToShopperResponse` variant — `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:3802` |

### Key Components

- `VoucherData` enum — 14 variants, at `crates/types-traits/domain_types/src/payment_method_data.rs:440`.
- `VoucherNextStepData` struct — carries `reference`, `barcode`, `digitable_line`, `download_url`, `instructions_url`, `qr_code_url`, `expires_at`, at `crates/types-traits/domain_types/src/payment_method_data.rs:415`.
- `BoletoVoucherData { social_security_number }` — `crates/types-traits/domain_types/src/payment_method_data.rs:392`.
- `AlfamartVoucherData` / `IndomaretVoucherData` / `JCSVoucherData` — all unit-like markers (`{}`) in the source enum, at `crates/types-traits/domain_types/src/payment_method_data.rs:397`, `:400`, `:403`.
- Adyen is the only connector with a real implementation path for any voucher variant at the pinned SHA; see `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1514`.

---

## Variant Enumeration

The source of truth is `VoucherData` at `crates/types-traits/domain_types/src/payment_method_data.rs:440`. There are 14 variants. Every one MUST appear in this table.

| # | Variant | Data Shape | Citation | Used By (connectors) |
|---|---------|------------|----------|----------------------|
| 1 | `Boleto(Box<BoletoVoucherData>)` | Struct with optional `social_security_number: Secret<String>` (Brazilian CPF) | `payment_method_data.rs:441`; struct at `:392` | Adyen (`adyen/transformers.rs:1533`) |
| 2 | `Efecty` | Unit variant | `payment_method_data.rs:442` | (none) — rejected by Adyen at `adyen/transformers.rs:1553`, stripe at `stripe/transformers.rs:1494`, paypal at `paypal/transformers.rs:1249` |
| 3 | `PagoEfectivo` | Unit variant | `payment_method_data.rs:443` | (none) — rejected by Adyen at `adyen/transformers.rs:1554`, stripe at `stripe/transformers.rs:1496`, paypal at `paypal/transformers.rs:1250` |
| 4 | `RedCompra` | Unit variant | `payment_method_data.rs:444` | (none) — rejected by Adyen at `adyen/transformers.rs:1555`, stripe at `stripe/transformers.rs:1497`, paypal at `paypal/transformers.rs:1251` |
| 5 | `RedPagos` | Unit variant | `payment_method_data.rs:445` | (none) — rejected by Adyen at `adyen/transformers.rs:1556`, stripe at `stripe/transformers.rs:1498`, paypal at `paypal/transformers.rs:1252` |
| 6 | `Alfamart(Box<AlfamartVoucherData>)` | Empty struct marker `AlfamartVoucherData {}` | `payment_method_data.rs:446`; struct at `:398` | Adyen (`adyen/transformers.rs:1534`) |
| 7 | `Indomaret(Box<IndomaretVoucherData>)` | Empty struct marker `IndomaretVoucherData {}` | `payment_method_data.rs:447`; struct at `:401` | Adyen (`adyen/transformers.rs:1535`) |
| 8 | `Oxxo` | Unit variant | `payment_method_data.rs:448` | Adyen (`adyen/transformers.rs:1538`) |
| 9 | `SevenEleven(Box<JCSVoucherData>)` | Empty struct marker `JCSVoucherData {}` | `payment_method_data.rs:449`; struct at `:404` | Adyen (`adyen/transformers.rs:1539`) |
| 10 | `Lawson(Box<JCSVoucherData>)` | Empty struct marker `JCSVoucherData {}` | `payment_method_data.rs:450`; struct at `:404` | Adyen (`adyen/transformers.rs:1542`) |
| 11 | `MiniStop(Box<JCSVoucherData>)` | Empty struct marker `JCSVoucherData {}` | `payment_method_data.rs:451`; struct at `:404` | Adyen (`adyen/transformers.rs:1543`) |
| 12 | `FamilyMart(Box<JCSVoucherData>)` | Empty struct marker `JCSVoucherData {}` | `payment_method_data.rs:452`; struct at `:404` | Adyen (`adyen/transformers.rs:1546`) |
| 13 | `Seicomart(Box<JCSVoucherData>)` | Empty struct marker `JCSVoucherData {}` | `payment_method_data.rs:453`; struct at `:404` | Adyen (`adyen/transformers.rs:1549`) |
| 14 | `PayEasy(Box<JCSVoucherData>)` | Empty struct marker `JCSVoucherData {}` | `payment_method_data.rs:454`; struct at `:404` | Adyen (`adyen/transformers.rs:1552`) |

Real-world descriptions:

1. **Boleto** — Brazilian bank-issued voucher (Boleto Bancário). Customer pays the printed slip at any Brazilian bank, ATM, correspondent banking outlet, lottery agency, or via internet banking. Uses an 11-digit CPF number as the shopper's social-security identifier; see `adyen/transformers.rs:3520` for the Adyen validator.
2. **Efecty** — Colombian cash-payment network. Customer pays at a physical Efecty agent.
3. **PagoEfectivo** — Peruvian cash-payment aggregator. Customer receives a CIP code and pays at banks, agents, or via online banking.
4. **RedCompra** — Chilean debit network (Transbank Redcompra) used for cash/debit-at-store flows.
5. **RedPagos** — Uruguayan cash-payment network (Redpagos) with thousands of physical agents.
6. **Alfamart** — Indonesian convenience-store chain used as an offline-payment point. Customer quotes the reference; cashier accepts cash.
7. **Indomaret** — Indonesian convenience-store chain, sibling to Alfamart, same pay-by-reference pattern.
8. **Oxxo** — Mexican convenience-store chain. Customer presents barcode or reference; pays at the cash register.
9. **SevenEleven** — Japanese 7-Eleven convenience stores (econtext rails). Customer pays at a register using the reference.
10. **Lawson** — Japanese Lawson convenience stores, econtext rails.
11. **MiniStop** — Japanese MiniStop convenience stores, econtext rails.
12. **FamilyMart** — Japanese FamilyMart convenience stores, econtext rails.
13. **Seicomart** — Japanese Seicomart convenience stores (Hokkaido-centric), econtext rails.
14. **PayEasy** — Japanese Pay-Easy bank/ATM rail, routed over econtext; customer pays at ATM, net-banking or post-office terminal.

---

## Architecture Overview

### Flow Type

`Authorize` marker from `domain_types::connector_flow`. Voucher authorize is always routed through the same `Authorize` flow as cards/wallets — the pattern divergence happens at the transformer level, not the flow level.

### Request Type

`PaymentsAuthorizeData<T>` where `T: PaymentMethodDataTypes`. See `crates/types-traits/domain_types/src/connector_types.rs` for the struct; the voucher branch lives inside `payment_method_data: PaymentMethodData::Voucher(VoucherData)`.

### Response Type

`PaymentsResponseData`. For voucher flows the usable response is almost always `PaymentsResponseData::TransactionResponse { connector_metadata: Some(<VoucherNextStepData JSON>), redirection_data: None, .. }`. See Adyen's constructor at `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:4503`.

### Resource Common Data

`PaymentFlowData`. Voucher transformers typically read billing fields (`get_billing_first_name`, `get_billing_email`, `get_billing_phone_number`) from `resource_common_data` — see Adyen `JCSVoucherData` build at `adyen/transformers.rs:1578`.

### Canonical RouterDataV2 signature

```rust
RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
```

All four type arguments are fixed; the voucher variant is unwrapped inside the transformer after pattern-matching `PaymentMethodData::Voucher(voucher_data)`. See the top-level dispatch at `adyen/transformers.rs:3675`.

### Variant Unwrap Site

```rust
// From crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:3675
PaymentMethodData::Voucher(ref voucher_data) => {
    Self::try_from((item, voucher_data)).map_err(|err| {
        err.change_context(IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        })
    })
}
```

Once inside the PM-specific `TryFrom`, the inner `VoucherData` enum is matched variant-by-variant (see `adyen/transformers.rs:1532`).

---

## Connectors with Full Implementation

| Connector | HTTP Method | Content Type | URL Pattern | Request Type Reuse | Notes |
|-----------|-------------|--------------|-------------|--------------------|-------|
| Adyen | POST | `application/json` | `/v68/payments` (Checkout API) | `AdyenPaymentRequest<T>` — reuses the same request struct used by cards/wallets/BNPL; voucher-specific branch at `adyen/transformers.rs:3400`. Payment-method body built via `AdyenPaymentMethod::try_from((voucher_data, item))` at `adyen/transformers.rs:1514` | Uses `PresentToShopperResponse` variant (`adyen/transformers.rs:3802`); populates `connector_metadata` with `VoucherNextStepData` via `get_present_to_shopper_metadata` at `adyen/transformers.rs:7195` |

### Stub Implementations

The connectors below parse the `VoucherData` arm but return `IntegrationError::not_implemented` for every variant at the pinned SHA. They are NOT real implementations; they only exhaustively cover the enum to satisfy the compiler.

- Stripe — rejects `Boleto | Oxxo` at `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:1490`, rejects the remaining 12 variants at `:1494`.
- PayPal — rejects all 14 variants in one arm at `crates/integrations/connector-integration/src/connectors/paypal/transformers.rs:1247`.

No other connector in `crates/integrations/connector-integration/src/connectors/` at the pinned SHA holds a voucher branch that does anything other than reject or fall through the default "payment method not supported" arm.

---

## Per-Variant Implementation Notes

### Boleto

- **Real impl:** Adyen.
- **Payload:** Mapped to the Adyen `BoletoBancario` flat variant (no inner struct) — `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1533`. The Adyen payment-method enum side is declared at `adyen/transformers.rs:272`.
- **CPF handling:** The `social_security_number` from `BoletoVoucherData` is pulled out by `get_social_security_number` at `adyen/transformers.rs:3546` and validated by `is_valid_social_security_number` at `adyen/transformers.rs:3520` (must be exactly 11 ASCII digits). Propagated into the outer `AdyenPaymentRequest.social_security_number` at `adyen/transformers.rs:3494`.
- **Response:** Consumed as `AdyenPaymentResponse::PresentToShopper` (`adyen/transformers.rs:3802`); barcode/reference arrive via `VoucherNextStepData` in `connector_metadata` (`adyen/transformers.rs:7209`).
- **Stubs:** Rejected by stripe at `stripe/transformers.rs:1490`, rejected by paypal at `paypal/transformers.rs:1248`.

### Efecty

- **Real impl:** (none at pinned SHA.)
- **Adyen status:** Explicitly rejected — `adyen/transformers.rs:1553` returns `IntegrationError::not_implemented("Adyen")`.
- **Stubs:** stripe `stripe/transformers.rs:1495`, paypal `paypal/transformers.rs:1249`.
- **Notes:** The variant is defined in UCS but no connector in the tree produces an outbound request body for it. Any PR adding support must introduce a new Adyen branch or implement a dlocal / d-local-style connector.

### PagoEfectivo

- **Real impl:** (none at pinned SHA.)
- **Adyen status:** Explicitly rejected — `adyen/transformers.rs:1554`.
- **Stubs:** stripe `stripe/transformers.rs:1496`, paypal `paypal/transformers.rs:1250`.
- **Notes:** Defined-but-unimplemented.

### RedCompra

- **Real impl:** (none at pinned SHA.)
- **Adyen status:** Explicitly rejected — `adyen/transformers.rs:1555`.
- **Stubs:** stripe `stripe/transformers.rs:1497`, paypal `paypal/transformers.rs:1251`.
- **Notes:** Defined-but-unimplemented.

### RedPagos

- **Real impl:** (none at pinned SHA.)
- **Adyen status:** Explicitly rejected — `adyen/transformers.rs:1556`.
- **Stubs:** stripe `stripe/transformers.rs:1498`, paypal `paypal/transformers.rs:1252`.
- **Notes:** Defined-but-unimplemented.

### Alfamart

- **Real impl:** Adyen.
- **Payload:** Mapped to Adyen's `Alfamart(Box<DokuBankData>)` at `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:274`. The branch arm that builds it is at `adyen/transformers.rs:1534`.
- **Data building:** `DokuBankData::try_from(&RouterDataV2<...>)` populates `{ first_name, last_name, shopper_email }` from `resource_common_data` — see `adyen/transformers.rs:1734`.
- **Response:** Same `PresentToShopperResponse` path; voucher metadata emitted via `adyen/transformers.rs:7205`.
- **Stubs:** paypal `paypal/transformers.rs:1253`, stripe `stripe/transformers.rs:1494`.

### Indomaret

- **Real impl:** Adyen.
- **Payload:** Adyen enum variant `Indomaret(Box<DokuBankData>)` at `adyen/transformers.rs:276`; arm at `adyen/transformers.rs:1535`. Same `DokuBankData` struct as Alfamart.
- **Response:** Same `get_present_to_shopper_response` path with voucher metadata.
- **Stubs:** stripe `stripe/transformers.rs:1499`, paypal `paypal/transformers.rs:1254`.

### Oxxo

- **Real impl:** Adyen.
- **Payload:** Simple unit variant on the Adyen side, `Oxxo` at `adyen/transformers.rs:278`. Arm at `adyen/transformers.rs:1538` — no inner struct construction, the payment method body is just `{"type":"oxxo"}`.
- **Response:** `PresentToShopperResponse`; voucher metadata emitted in the matched arm at `adyen/transformers.rs:7208` (Oxxo listed with Alfamart/Indomaret/BoletoBancario as the four "supported voucher payment methods").
- **Stubs:** stripe rejects at `stripe/transformers.rs:1490` (grouped with Boleto), paypal at `paypal/transformers.rs:1255`.

### SevenEleven

- **Real impl:** Adyen.
- **Payload:** Adyen enum variant `SevenEleven(Box<JCSVoucherData>)` with `#[serde(rename = "econtext_seven_eleven")]` at `adyen/transformers.rs:280`. Arm at `adyen/transformers.rs:1539`.
- **Data building:** Adyen's `JCSVoucherData` (distinct from the UCS marker struct — this one carries `first_name`, `last_name`, `shopper_email`, `telephone_number`) is built at `adyen/transformers.rs:1578`.
- **Response:** `PresentToShopperResponse` — but note that `SevenEleven` / `Lawson` are **NOT** in the supported-metadata arm at `adyen/transformers.rs:7205`; they fall into the "return `None` for metadata" arm at `adyen/transformers.rs:7294`. This means the customer sees the raw reference, but no `VoucherNextStepData` is stored.
- **Stubs:** stripe `stripe/transformers.rs:1500`, paypal `paypal/transformers.rs:1256`.

### Lawson

- **Real impl:** Adyen.
- **Payload:** Adyen variant `Lawson(Box<JCSVoucherData>)` with `#[serde(rename = "econtext_stores")]` at `adyen/transformers.rs:282`. Arm at `adyen/transformers.rs:1542`.
- **Response metadata:** Like `SevenEleven`, emits `None` from `get_present_to_shopper_metadata` (`adyen/transformers.rs:7295`).
- **Stubs:** stripe `stripe/transformers.rs:1501`, paypal `paypal/transformers.rs:1257`.

### MiniStop

- **Real impl:** Adyen.
- **Payload:** Adyen variant `MiniStop(Box<JCSVoucherData>)` at `adyen/transformers.rs:284`, serde-tagged as `"econtext_stores"`. Arm at `adyen/transformers.rs:1543`.
- **Note:** Adyen's payload serializes `MiniStop`, `FamilyMart`, `Seicomart`, `PayEasy` all under the same `econtext_stores` rename — the merchant account configuration determines which store is actually used. Variants are kept distinct on the UCS side for clarity but collapse at the wire.
- **Stubs:** stripe `stripe/transformers.rs:1502`, paypal `paypal/transformers.rs:1258`.

### FamilyMart

- **Real impl:** Adyen.
- **Payload:** Adyen variant `FamilyMart(Box<JCSVoucherData>)` at `adyen/transformers.rs:286` (serde `econtext_stores`). Arm at `adyen/transformers.rs:1546`.
- **Stubs:** stripe `stripe/transformers.rs:1503`, paypal `paypal/transformers.rs:1259`.

### Seicomart

- **Real impl:** Adyen.
- **Payload:** Adyen variant `Seicomart(Box<JCSVoucherData>)` at `adyen/transformers.rs:288` (serde `econtext_stores`). Arm at `adyen/transformers.rs:1549`.
- **Stubs:** stripe `stripe/transformers.rs:1504`, paypal `paypal/transformers.rs:1260`.

### PayEasy

- **Real impl:** Adyen.
- **Payload:** Adyen variant `PayEasy(Box<JCSVoucherData>)` at `adyen/transformers.rs:290` (serde `econtext_stores`). Arm at `adyen/transformers.rs:1552`.
- **Stubs:** stripe `stripe/transformers.rs:1505`, paypal `paypal/transformers.rs:1261`.

### Summary of coverage

- **Variants with at least one real implementation (9/14):** Boleto, Alfamart, Indomaret, Oxxo, SevenEleven, Lawson, MiniStop, FamilyMart, Seicomart, PayEasy. That's 10; corrected count below.
- **Variants defined-but-unimplemented (4/14):** Efecty, PagoEfectivo, RedCompra, RedPagos. All four are Latin-American cash/debit rails currently without a connector binding at this SHA.

Accurate count: 10 variants with a real connector arm, 4 variants stub-only everywhere.

---

## Common Implementation Patterns

### Pattern 1 — "Present-to-shopper" response path (Adyen canonical)

Voucher authorize ≠ redirect. The connector responds synchronously with the voucher's identifying artifacts (reference number, barcode, download URL) and the customer completes offline later. The canonical wiring is:

1. Request body: a standard `AdyenPaymentRequest<T>` whose `payment_method` field is built by matching `VoucherData` — `adyen/transformers.rs:1514`.
2. Response body: `AdyenPaymentResponse::PresentToShopper(Box<PresentToShopperResponse>)` — `adyen/transformers.rs:3802`.
3. Response translator: `get_present_to_shopper_response` at `adyen/transformers.rs:4468` writes `VoucherNextStepData` into `connector_metadata` (via `get_present_to_shopper_metadata`, `adyen/transformers.rs:7195`) and sets `redirection_data: None` (`adyen/transformers.rs:4508`).
4. Status: derived from `get_adyen_payment_status` and typically lands on `AttemptStatus::Pending` or `AuthenticationPending` depending on manual-capture flag.

Any new voucher-capable connector should follow the same three-step skeleton: (a) build request, (b) decode a "next-step"-flavored response, (c) pack a `VoucherNextStepData` into `connector_metadata`.

### Pattern 2 — Explicit enum exhaustiveness & polite rejection

Even connectors that do not implement Voucher must compile, which requires exhaustive matching. Stripe and PayPal do so by returning `IntegrationError::not_implemented(get_unimplemented_payment_method_error_message("<connector>"))` — see `stripe/transformers.rs:1485`, `paypal/transformers.rs:1261`. New connectors MUST either provide a real arm or follow this rejection shape; silently dropping the `VoucherData` arm is a compile error.

### Pattern 3 — Billing-field lift for Japanese (JCS) vouchers

Japanese econtext-rail vouchers need name/email/phone from the billing address. The Adyen `JCSVoucherData` helper at `adyen/transformers.rs:1567` lifts them via `resource_common_data.get_billing_first_name()`, `get_billing_email()`, `get_billing_phone_number()`. Any future JP voucher integration should reuse the same accessor pattern rather than re-reading address parts manually.

### Pattern 4 — CPF validation for Boleto

Boleto uniquely carries an inner struct (`BoletoVoucherData`) with an optional `social_security_number` field — see `payment_method_data.rs:392`. Adyen validates the field as 11 ASCII digits (`adyen/transformers.rs:3522`). If the field is present and invalid, Adyen drops the value rather than erroring — see the `tracing::warn!` branches at `adyen/transformers.rs:3528` and `:3537`. A new Boleto integration should either validate similarly or surface a structured error.

### Pattern 5 — Split metadata emission (supported vs. unsupported)

`get_present_to_shopper_metadata` at `adyen/transformers.rs:7195` intentionally distinguishes variants that should populate `VoucherNextStepData` (Alfamart, Indomaret, BoletoBancario, Oxxo — `adyen/transformers.rs:7205`) from variants that return `None` (SevenEleven, Lawson, etc. — `adyen/transformers.rs:7294`). Authors of new voucher flows must explicitly decide which variants produce metadata and which fall through; do not blanket-populate — some connectors only return useful artifacts for a subset.

---

## Code Examples

### Example 1 — Variant dispatch in Adyen (real impl)

```rust
// From crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1531
match voucher_data {
    VoucherData::Boleto(_) => Ok(Self::BoletoBancario),
    VoucherData::Alfamart(_) => Ok(Self::Alfamart(Box::new(DokuBankData::try_from(item)?))),
    VoucherData::Indomaret(_) => {
        Ok(Self::Indomaret(Box::new(DokuBankData::try_from(item)?)))
    }
    VoucherData::Oxxo => Ok(Self::Oxxo),
    VoucherData::SevenEleven(_) => {
        Ok(Self::SevenEleven(Box::new(JCSVoucherData::try_from(item)?)))
    }
    VoucherData::Lawson(_) => Ok(Self::Lawson(Box::new(JCSVoucherData::try_from(item)?))),
    VoucherData::MiniStop(_) => {
        Ok(Self::MiniStop(Box::new(JCSVoucherData::try_from(item)?)))
    }
    VoucherData::FamilyMart(_) => {
        Ok(Self::FamilyMart(Box::new(JCSVoucherData::try_from(item)?)))
    }
    VoucherData::Seicomart(_) => {
        Ok(Self::Seicomart(Box::new(JCSVoucherData::try_from(item)?)))
    }
    VoucherData::PayEasy(_) => Ok(Self::PayEasy(Box::new(JCSVoucherData::try_from(item)?))),
    VoucherData::Efecty
    | VoucherData::PagoEfectivo
    | VoucherData::RedCompra
    | VoucherData::RedPagos => Err(IntegrationError::not_implemented(
        utils::get_unimplemented_payment_method_error_message("Adyen"),
    )
    .into()),
}
```

### Example 2 — Billing-field lift for Japanese econtext vouchers

```rust
// From crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1570
fn try_from(
    item: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
) -> Result<Self, Self::Error> {
    Ok(Self {
        first_name: item.resource_common_data.get_billing_first_name()?,
        last_name: item.resource_common_data.get_optional_billing_last_name(),
        shopper_email: item.resource_common_data.get_billing_email()?,
        telephone_number: item.resource_common_data.get_billing_phone_number()?,
    })
}
```

### Example 3 — DokuBankData lift for Alfamart / Indomaret

```rust
// From crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1748
fn try_from(
    item: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
) -> Result<Self, Self::Error> {
    let first_name = item.resource_common_data.get_billing_first_name()?;
    let last_name = item.resource_common_data.get_optional_billing_last_name();
    let shopper_email = item.resource_common_data.get_billing_email()?;
    Ok(Self {
        first_name,
        last_name,
        shopper_email,
    })
}
```

### Example 4 — Boleto CPF extraction & validation

```rust
// From crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:3520
fn is_valid_social_security_number(social_security_number: &str) -> bool {
    match (
        social_security_number.len() == 11,
        social_security_number.chars().all(|c| c.is_ascii_digit()),
    ) {
        (false, _) => { /* tracing warn + false */ false }
        (_, false) => { /* tracing warn + false */ false }
        (true, true) => true,
    }
}

// From adyen/transformers.rs:3546
fn get_social_security_number(voucher_data: &VoucherData) -> Option<Secret<String>> {
    match voucher_data {
        VoucherData::Boleto(boleto_data) => match &boleto_data.social_security_number {
            Some(ssn) if is_valid_social_security_number(ssn.peek()) => Some(ssn.clone()),
            _ => None,
        },
        // all other variants -> None
        _ => None,
    }
}
```

### Example 5 — Present-to-shopper response packer

```rust
// From crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:4502
// We don't get connector transaction id for redirections in Adyen.
let payments_response_data = PaymentsResponseData::TransactionResponse {
    resource_id: match response.psp_reference.as_ref() {
        Some(psp) => ResponseId::ConnectorTransactionId(psp.to_string()),
        None => ResponseId::NoResponseId,
    },
    redirection_data: None,
    connector_metadata,            // serialized VoucherNextStepData for supported variants
    network_txn_id: None,
    connector_response_reference_id: response
        .merchant_reference
        .clone()
        .or(response.psp_reference),
    incremental_authorization_allowed: None,
    mandate_reference: None,
    status_code,
};
```

### Example 6 — VoucherNextStepData construction

```rust
// From crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:7205
PaymentType::Alfamart
| PaymentType::Indomaret
| PaymentType::BoletoBancario
| PaymentType::Oxxo => {
    let voucher_data = VoucherNextStepData {
        expires_at,
        reference,
        download_url: response.action.download_url.clone().map(|u| u.to_string()),
        instructions_url: response
            .action
            .instructions_url
            .clone()
            .map(|u| u.to_string()),
        entry_date: None,
        digitable_line: None,
        qr_code_url: None,
        barcode: None,
        expiry_date: None,
    };
    Some(voucher_data.encode_to_value())
        .transpose()
        .change_context(
            ConnectorResponseTransformationError::response_handling_failed_http_status_unknown(),
        )
}
```

### Example 7 — Exhaustive "rejecting" stub (Stripe)

```rust
// From crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:1489
PaymentMethodData::Voucher(voucher_data) => match voucher_data {
    VoucherData::Boleto(_) | VoucherData::Oxxo => Err(IntegrationError::not_implemented(
        get_unimplemented_payment_method_error_message("stripe"),
    )
    .into()),
    VoucherData::Alfamart(_)
    | VoucherData::Efecty
    | VoucherData::PagoEfectivo
    | VoucherData::RedCompra
    | VoucherData::RedPagos
    | VoucherData::Indomaret(_)
    | VoucherData::SevenEleven(_)
    | VoucherData::Lawson(_)
    | VoucherData::MiniStop(_)
    | VoucherData::FamilyMart(_)
    | VoucherData::Seicomart(_)
    | VoucherData::PayEasy(_) => Err(IntegrationError::not_implemented(
        get_unimplemented_payment_method_error_message("stripe"),
    )
    .into()),
},
```

### Example 8 — Top-level dispatch to voucher branch (Adyen)

```rust
// From crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:3675
PaymentMethodData::Voucher(ref voucher_data) => {
    Self::try_from((item, voucher_data)).map_err(|err| {
        err.change_context(IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        })
    })
}
```

---

## Best Practices

- **Always match exhaustively on `VoucherData` — include every one of the 14 variants.** Missing a variant will not compile; a wildcard `_ =>` arm will compile but makes future variants silently fall through. Follow Adyen's explicit-group style at `adyen/transformers.rs:1553` (Efecty / PagoEfectivo / RedCompra / RedPagos grouped) over a wildcard.
- **Prefer `PresentToShopperResponse`-style handling over redirect handling.** Voucher authorize is not a browser redirect; `redirection_data` should stay `None` and `connector_metadata` should carry the artifact. See `adyen/transformers.rs:4508`.
- **Serialize `VoucherNextStepData` into `connector_metadata` using `encode_to_value`.** Adyen does this at `adyen/transformers.rs:7225`; downstream consumers (SDK, Hyperswitch router) expect JSON-encoded next-step data in that slot.
- **Validate CPF / tax identifiers before forwarding.** Follow Adyen's pattern at `adyen/transformers.rs:3520` — reject 11-digit constraint violations. Do NOT forward an invalid CPF verbatim; silently drop or raise per connector semantics.
- **Lift billing fields consistently.** Use `resource_common_data.get_billing_first_name()` / `get_billing_email()` / `get_billing_phone_number()` accessors rather than walking `address.address.first_name` manually; this matches the cross-cutting pattern at `adyen/transformers.rs:1578` and `:1748`.
- **Document which variants are unsupported.** When adding a new connector, explicitly enumerate the rejected variants in a `| VoucherData::X | VoucherData::Y | ... => Err(...)` arm — reviewers then know the variant was considered rather than missed. See the sibling `authorize/bank_redirect/pattern_authorize_bank_redirect.md` for the same discipline applied to bank redirects.
- **Do not hard-code `AttemptStatus::Pending` in transformer bodies.** Derive it from the connector response; Adyen does this through `get_adyen_payment_status` called from `adyen/transformers.rs:4474`. Hardcoded statuses are a banned anti-pattern under `PATTERN_AUTHORING_SPEC.md` §11.
- **Prefer `MinorUnit` unless the connector API demands `StringMajorUnit`.** Adyen's voucher flow uses the shared `AdyenRouterData<_, T>` amount converter set up in the connector-level `create_amount_converter_wrapper!` — no voucher-specific unit choice.
- **Keep Japanese (JCS) variants distinct even when they share a wire tag.** UCS preserves `SevenEleven`, `Lawson`, `MiniStop`, `FamilyMart`, `Seicomart`, `PayEasy` separately; Adyen collapses them to `econtext_stores` (`adyen/transformers.rs:283`-`:290`). Future connectors may discriminate on these at their own wire layer — keep the UCS-side enum granular.

---

## Common Errors

### 1. Missing billing fields for JCS vouchers

- **Problem:** `JCSVoucherData::try_from(item)` returns an error because `get_billing_first_name()` or `get_billing_email()` is unset.
- **Why it happens:** The UCS marker `JCSVoucherData` (`payment_method_data.rs:404`) is an empty struct — all required shopper data lives on the billing address, not in the voucher payload.
- **Solution:** Validate billing-address presence at the orchestration layer before routing to a JCS variant; if absent, reject with `IntegrationError::MissingRequiredField { field_name: "billing.email", ... }`. See the accessor contract at `adyen/transformers.rs:1581`.

### 2. Silently accepting an invalid Boleto CPF

- **Problem:** A non-11-digit or non-numeric CPF is passed through verbatim; Adyen rejects with 422.
- **Solution:** Call the validator at `adyen/transformers.rs:3522` before emitting the request. If invalid, either drop the field (Adyen's behavior) or return a structured error — do not let the connector reject with an opaque API error.

### 3. Trying to render voucher metadata as a redirect

- **Problem:** Setting `redirection_data: Some(...)` on a voucher response; the customer gets bounced to the acquirer's URL instead of seeing a barcode.
- **Solution:** Keep `redirection_data: None` and put the artifacts into `connector_metadata` as a serialized `VoucherNextStepData`. See `adyen/transformers.rs:4508` / `:4509`.

### 4. Falling through a new VoucherData variant silently

- **Problem:** A future variant is added to `VoucherData`; a connector written with `_ => Err(...)` keeps compiling but treats the new variant as unsupported without review.
- **Solution:** Never wildcard-match `VoucherData`. Group rejected variants explicitly as Adyen does at `adyen/transformers.rs:1553` — the compiler then forces every future variant through an explicit decision at each connector.

### 5. Overpopulating `VoucherNextStepData` for unsupported variants

- **Problem:** Emitting `VoucherNextStepData` for SevenEleven / Lawson / etc. when the connector response does not actually provide `download_url` / `instructions_url`.
- **Solution:** Split the metadata emitter by variant, as Adyen does — only the four "supported voucher" `PaymentType`s (`Alfamart | Indomaret | BoletoBancario | Oxxo` at `adyen/transformers.rs:7205`) get `Some(voucher_data)`; the rest return `None` at `adyen/transformers.rs:7296`.

### 6. Mismatch between PaymentMethodType and VoucherData variant

- **Problem:** Routing layer sends `PaymentMethodType::Oxxo` but the inner `VoucherData::Boleto(...)` — downstream transformer builds a Boleto payload instead.
- **Solution:** Validate payment-method-type consistency at the top of each voucher transformer arm, or rely on a connector-wide check like Adyen's `get_adyen_payment_status` path which already defensively keys off the variant tag.

### 7. Hard-coding `AttemptStatus::Pending` for voucher responses

- **Problem:** Literal `status: AttemptStatus::Pending` in transformer body. This is a §11 banned anti-pattern in `PATTERN_AUTHORING_SPEC.md`.
- **Solution:** Derive the status from the connector's result code — Adyen routes through `get_adyen_payment_status(is_manual_capture, result_code, pmt)` called from `adyen/transformers.rs:4474`.

---

## Cross-References

- Authoring spec: [../../PATTERN_AUTHORING_SPEC.md](../../PATTERN_AUTHORING_SPEC.md)
- Parent authorize index: [../README.md](../README.md)
- Patterns top-level index: [../../README.md](../../README.md)
- Sibling PM pattern (gold reference): [../card/pattern_authorize_card.md](../card/pattern_authorize_card.md)
- Sibling PM pattern (closest shape — async / multi-variant regional): [../bank_redirect/pattern_authorize_bank_redirect.md](../bank_redirect/pattern_authorize_bank_redirect.md)
- Sibling PM pattern (another async regional PM for contrast): [../bank_transfer/pattern_authorize_bank_transfer.md](../bank_transfer/pattern_authorize_bank_transfer.md)
- Flow pattern (parent flow): [../../pattern_authorize.md](../../pattern_authorize.md)
- Flow pattern (downstream — voucher flows settle via PSync/webhook): [../../pattern_psync.md](../../pattern_psync.md)

### Key source files cited

- `crates/types-traits/domain_types/src/payment_method_data.rs:265` — `PaymentMethodData::Voucher(VoucherData)` variant declaration.
- `crates/types-traits/domain_types/src/payment_method_data.rs:392` — `BoletoVoucherData` struct.
- `crates/types-traits/domain_types/src/payment_method_data.rs:397` — `AlfamartVoucherData` (unit struct).
- `crates/types-traits/domain_types/src/payment_method_data.rs:400` — `IndomaretVoucherData` (unit struct).
- `crates/types-traits/domain_types/src/payment_method_data.rs:403` — `JCSVoucherData` (unit struct).
- `crates/types-traits/domain_types/src/payment_method_data.rs:415` — `VoucherNextStepData`.
- `crates/types-traits/domain_types/src/payment_method_data.rs:440` — `VoucherData` enum (14 variants).
- `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:270`-`:291` — Adyen payment-method enum voucher arms.
- `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:301` — Adyen's own `JCSVoucherData` (distinct from UCS marker).
- `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:495` — Adyen's `DokuBankData`.
- `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1514` — Voucher → `AdyenPaymentMethod` `TryFrom`.
- `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1734` — `DokuBankData::try_from(&RouterDataV2<...>)`.
- `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:3400` — `(AdyenRouterData, &VoucherData) -> AdyenPaymentRequest`.
- `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:3520` / `:3546` — CPF validator / extractor.
- `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:3675` — top-level `PaymentMethodData::Voucher` dispatch.
- `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:3802` — `PresentToShopperResponse` variant.
- `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:4468` — `get_present_to_shopper_response`.
- `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:7195` — `get_present_to_shopper_metadata` (variant-gated metadata emission).
- `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:1489` — stripe voucher stub (all-reject).
- `crates/integrations/connector-integration/src/connectors/paypal/transformers.rs:1243` — paypal voucher stub (all-reject).
