# Card Redirect Authorize Flow Pattern

## Overview

Card Redirect is a payment-method category in Grace-UCS where the underlying payment rail is a **card network** (debit, prepaid or domestic scheme) but the customer experience is an **issuer-hosted redirect** instead of direct card-data capture. The merchant does not collect PAN/CVC; instead, the connector returns an issuer URL (e.g. Knet portal, Benefit ATM-card portal, MomoAtm voucher page) to which the shopper is redirected for authentication.

Key Characteristics:

| Property | Value |
|----------|-------|
| Enum | `CardRedirectData` at `crates/types-traits/domain_types/src/payment_method_data.rs:1333` |
| Parent enum arm | `PaymentMethodData::CardRedirect(CardRedirectData)` at `crates/types-traits/domain_types/src/payment_method_data.rs:254` |
| Variant count at pinned SHA | 4 |
| Customer flow | Redirect to issuer / ATM-card portal |
| Merchant PCI scope | Out-of-scope — no PAN or CVC is forwarded |
| Typical response | `RedirectForm::Form { endpoint, method, form_fields }` |
| Settlement | Asynchronous; requires PSync or webhook |

> **Important distinction — CardRedirect is NOT BankRedirect.**
>
> CardRedirect is distinct from BankRedirect — the payment rail is card-based but uses a redirect flow instead of CSE (client-side encryption). In a BankRedirect flow (`BankRedirectData::Ideal`, `Sofort`, `Giropay`, `OpenBanking`, etc.) the shopper authenticates at their retail **bank** and funds move over a bank-transfer rail. In a CardRedirect flow (`CardRedirectData::Knet`, `Benefit`, `MomoAtm`, `CardRedirect`) the shopper authenticates at an **issuer or card-scheme portal** and settlement flows over a card rail (Knet in Kuwait, Benefit in Bahrain/Saudi, Meeza/Momo in Egypt).
>
> Historically, CardRedirect content was misplaced inside `authorize/bank_redirect/pattern_authorize_bank_redirect.md`. That document remains the authoritative home for `BankRedirectData`; **this** document is the authoritative home for `CardRedirectData`. Content covering `CardRedirectData` in the bank-redirect pattern will be removed in Wave 6 and redirected here; see `authorize/bank_redirect/pattern_authorize_bank_redirect.md` (not edited by Wave-3C).

### Why it exists as its own PM

1. Different parent enum arm in `PaymentMethodData` — the router dispatches `PaymentMethodData::CardRedirect(_)` to a separate `TryFrom<&CardRedirectData>` impl on each connector; see the dispatcher at `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:3635`.
2. Different connector-side payment-method tags — e.g. Adyen emits `Knet`, `Benefit`, `momo_atm` as top-level `AdyenPaymentMethod` variants (see `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:266`, `:267`, `:268-269`) rather than a `type: ideal|sofort|giropay` tagged sub-enum used for BankRedirect.
3. No PAN/CVC is collected from the shopper despite settlement occurring over card rails; transformers consequently do **not** reach for `Card::card_number`/`card_cvc` and do **not** require a CSE (client-side encryption) token.

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

All 4 variants of `CardRedirectData` at the pinned SHA (`crates/types-traits/domain_types/src/payment_method_data.rs:1333-1338`):

```rust
// From crates/types-traits/domain_types/src/payment_method_data.rs:1332
#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum CardRedirectData {
    Knet {},
    Benefit {},
    MomoAtm {},
    CardRedirect {},
}
```

| Variant | Data Shape | Citation | Regional Context | Used By (connectors) |
|---------|------------|----------|------------------|----------------------|
| `Knet` | unit-struct `{}` (empty record variant) | `crates/types-traits/domain_types/src/payment_method_data.rs:1334` | Kuwait national debit-card network ("KNET"). Domestic card scheme operated by the Kuwait Banking Association; used by almost every bank debit card issued in Kuwait. Redirect lands the shopper on the KNET portal. | Adyen (full impl, `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1828`) |
| `Benefit` | unit-struct `{}` | `crates/types-traits/domain_types/src/payment_method_data.rs:1335` | Bahrain / GCC debit-card network ("Benefit" by BENEFIT Company B.S.C., also routed for some Saudi ATM-card flows). Issuer-hosted redirect. | Adyen (full impl, `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1829`) |
| `MomoAtm` | unit-struct `{}` | `crates/types-traits/domain_types/src/payment_method_data.rs:1336` | Egyptian Meeza/ATM-card redirect ("momo_atm" on Adyen). Shopper completes payment at an ATM or online Meeza portal. Serde tag on the Adyen side is `momo_atm` (`crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:268-269`). | Adyen (full impl, `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1830`) |
| `CardRedirect` | unit-struct `{}` | `crates/types-traits/domain_types/src/payment_method_data.rs:1337` | Generic catch-all for card-redirect flows that don't have a dedicated variant (issuer 3DS-style redirect for a card not otherwise enumerated). Currently **not implemented** by any connector — all four connectors that match on `CardRedirectData` return `IntegrationError::not_implemented` for this variant. | (none — all connectors error out, see Adyen `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1831-1833`, Stripe `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:1479`, PayPal `crates/integrations/connector-integration/src/connectors/paypal/transformers.rs:1164`) |

Note on "Used By": The reviewer will diff this table against the enum variants at the pinned SHA. All 4 variants are present. Zero variants are omitted.

## Architecture Overview

### Flow Type

`Authorize` — marker from `domain_types::connector_flow::Authorize`.

### Request Type

`PaymentsAuthorizeData<T>` — generic over `T: PaymentMethodDataTypes`. Inside, `payment_method_data: PaymentMethodData<T>` is matched on the `CardRedirect(_)` arm.

### Response Type

`PaymentsResponseData` — the `TransactionResponse` variant is returned with `redirection_data: Some(Box<RedirectForm>)` populated because every supported CardRedirect variant is a redirect flow.

### Resource Common Data

`PaymentFlowData` — `crates/types-traits/domain_types/src/connector_types.rs:422`. Holds the shared connector request reference id, billing/shipping addresses (used for issuer-country hinting on MomoAtm/Benefit), and return-URL metadata.

### Canonical signature

```rust
RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
```

### Where `CardRedirectData` is unwrapped

Every connector that supports this PM unwraps via the same shape:

```rust
// Conceptual dispatch pattern
match &router_data.request.payment_method_data {
    PaymentMethodData::CardRedirect(ref card_redirect_data) => {
        // map CardRedirectData variants to the connector's native payment-method tag
    }
    _ => Err(IntegrationError::not_implemented("payment_method").into()),
}
```

Concrete examples:
- Adyen dispatcher: `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:3635` routes `PaymentMethodData::CardRedirect(ref card_redirect_data)` to `Self::try_from((item, card_redirect_data))` which is the `TryFrom<(AdyenRouterData<...>, &CardRedirectData)> for AdyenPaymentRequest<T>` impl at `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:2918-2947`.
- PayPal dispatcher: `crates/integrations/connector-integration/src/connectors/paypal/transformers.rs:1123` routes to `Self::try_from(card_redirect_data)` (the stub impl at `:1156-1169`).
- Stripe dispatcher: `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:1475` inline-matches all four variants and returns `IntegrationError::not_implemented` (full stub).
- MultiSafepay dispatcher: `crates/integrations/connector-integration/src/connectors/multisafepay/transformers.rs:79` maps `PaymentMethodData::CardRedirect(_) => Type::Redirect` at the top-level `Type` enum, and `:240` maps the gateway to `Gateway::CreditCard` with no per-variant switch.

## Connectors with Full Implementation

| Connector | HTTP Method | Content Type | URL Pattern | Request Type Reuse | Notes |
|-----------|-------------|--------------|-------------|--------------------|-------|
| Adyen | POST | application/json | `/v68/payments` (`base_url` + `"/payments"`) | Reuses `AdyenPaymentRequest<T>` — shared with Card, Wallet, BankRedirect, BankDebit flows on this connector. See `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:2918-2955`. | Only connector at the pinned SHA that has a non-stub `TryFrom<&CardRedirectData>` impl for real variants. Supports `Knet`, `Benefit`, `MomoAtm`. Stubs out `CardRedirect` generic with `IntegrationError::not_implemented("payment_method")` at `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1831-1833`. |

### Stub Implementations

The following connectors have a dedicated `TryFrom<&CardRedirectData>` or match arm for `PaymentMethodData::CardRedirect(_)` but return `IntegrationError::not_implemented` for all four variants. They are listed here (per spec §10) rather than in the full table above:

- PayPal — match arm at `crates/integrations/connector-integration/src/connectors/paypal/transformers.rs:1123`, stub impl at `:1156-1169`.
- Stripe — inline match at `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:1475-1483`.
- MultiSafepay — classifies all `CardRedirectData` as `Type::Redirect` / `Gateway::CreditCard` (`crates/integrations/connector-integration/src/connectors/multisafepay/transformers.rs:79`, `:240`) but does not emit a card-redirect-specific request shape.

The following connectors explicitly list `PaymentMethodData::CardRedirect(_)` in their catch-all not-implemented arm (no per-variant logic at all). These are NOT considered CardRedirect implementations:

- ACI (`crates/integrations/connector-integration/src/connectors/aci/transformers.rs:745`)
- Bambora (`crates/integrations/connector-integration/src/connectors/bambora/transformers.rs:283`)
- Bank of America (`crates/integrations/connector-integration/src/connectors/bankofamerica/transformers.rs:601`, `:1764`)
- Billwerk (`crates/integrations/connector-integration/src/connectors/billwerk/transformers.rs:220`)
- Braintree (`crates/integrations/connector-integration/src/connectors/braintree/transformers.rs:598`, `:1593`, `:2615`, `:2800`)
- Cryptopay (`crates/integrations/connector-integration/src/connectors/cryptopay/transformers.rs:96`)
- Cybersource (`crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:308`, `:2174`, `:2273`, `:3012`, `:3289`, `:4309`)
- Dlocal (`crates/integrations/connector-integration/src/connectors/dlocal/transformers.rs:194`)
- Fiserv (`crates/integrations/connector-integration/src/connectors/fiserv/transformers.rs:538`)
- Fiuu (`crates/integrations/connector-integration/src/connectors/fiuu/transformers.rs:661`)
- Forte (`crates/integrations/connector-integration/src/connectors/forte/transformers.rs:298`)
- Hipay (`crates/integrations/connector-integration/src/connectors/hipay/transformers.rs:580`)
- Loonio (`crates/integrations/connector-integration/src/connectors/loonio/transformers.rs:232`)
- Mifinity (`crates/integrations/connector-integration/src/connectors/mifinity/transformers.rs:234`)
- Nexinets (`crates/integrations/connector-integration/src/connectors/nexinets/transformers.rs:727`)
- Noon (`crates/integrations/connector-integration/src/connectors/noon/transformers.rs:363`, `:1248`)
- Placetopay (`crates/integrations/connector-integration/src/connectors/placetopay/transformers.rs:196`)
- Razorpay (`crates/integrations/connector-integration/src/connectors/razorpay/transformers.rs:291`)
- Redsys (`crates/integrations/connector-integration/src/connectors/redsys/transformers.rs:243`)
- Stax (`crates/integrations/connector-integration/src/connectors/stax/transformers.rs:1084`)
- Trustpay (`crates/integrations/connector-integration/src/connectors/trustpay/transformers.rs:1698`)
- Volt (`crates/integrations/connector-integration/src/connectors/volt/transformers.rs:281`)
- Wellsfargo (`crates/integrations/connector-integration/src/connectors/wellsfargo/transformers.rs:583`)
- Worldpay (`crates/integrations/connector-integration/src/connectors/worldpay/transformers.rs:219`)

Nuvei has no references to `CardRedirect` at the pinned SHA (grep of `src/connectors/nuvei/` returns zero hits).

## Per-Variant Implementation Notes

### `Knet {}`

- **Region:** Kuwait national debit-card network.
- **Representation on the wire (Adyen):** `AdyenPaymentMethod::Knet` — unit variant at `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:266`. No serde rename; serializes as `{"type":"Knet"}` in Adyen's `paymentMethod` object. A corresponding `PaymentType::Knet` exists at `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1066` for response-side parsing.
- **Conversion site:** `CardRedirectData::Knet {} => Ok(Self::Knet)` at `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1828`.
- **Request flow:** The outer `AdyenPaymentRequest<T>` is built via `TryFrom<(AdyenRouterData<...>, &CardRedirectData)>` at `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:2918-2955`. It uses `get_amount_data(&item)`, `AdyenAuthType::try_from(&item.router_data.connector_config)?`, `AdyenShopperInteraction::from(&item.router_data)`, and `item.router_data.request.get_router_return_url()?`. No card-number / CVC fields are referenced.
- **Response:** Adyen returns an `Action`/`RedirectResponse` block; the transformer builds a `RedirectForm` so `PaymentsResponseData::TransactionResponse.redirection_data` is populated. Customer is redirected to the KNET portal.
- **Settlement:** Asynchronous. PSync or webhook delivers the final status.

### `Benefit {}`

- **Region:** Bahrain (primary) and GCC markets via the BENEFIT Company B.S.C. scheme; also routed for some Saudi-originated ATM-card flows.
- **Representation on the wire (Adyen):** `AdyenPaymentMethod::Benefit` at `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:267`. Response-side `PaymentType::Benefit` at `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1067`.
- **Conversion site:** `CardRedirectData::Benefit {} => Ok(Self::Benefit)` at `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1829`.
- **Request flow:** Shares the `TryFrom<(AdyenRouterData<...>, &CardRedirectData)> for AdyenPaymentRequest<T>` constructor with `Knet` / `MomoAtm` at `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:2918-2955`. Billing address is passed through via `get_address_info(item.router_data.resource_common_data.get_optional_billing())` at `:2956-2958`.
- **Response:** Issuer-hosted redirect; use `PaymentsResponseData::TransactionResponse.redirection_data = Some(Box::new(RedirectForm::Form{..}))`.
- **Settlement:** Asynchronous; PSync required.

### `MomoAtm {}`

- **Region:** Egyptian Meeza / ATM-card redirect scheme. The name on Adyen's side is `momo_atm` (serde-renamed from the Rust identifier `MomoAtm`).
- **Representation on the wire (Adyen):** `AdyenPaymentMethod::MomoAtm` with `#[serde(rename = "momo_atm")]` at `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:268-269`. Response-side `PaymentType::MomoAtm` with the same rename at `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1025-1026`.
- **Conversion site:** `CardRedirectData::MomoAtm {} => Ok(Self::MomoAtm)` at `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1830`.
- **Request flow:** Same `AdyenPaymentRequest<T>` constructor (`crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:2918-2955`).
- **Serde caveat:** The snake-case serde rename `momo_atm` MUST match Adyen's expected tag. Authors changing the enum must preserve the rename — Adyen will reject a payload with `"type":"MomoAtm"`.
- **Response / Settlement:** Redirect flow; async settlement; handled identically to `Knet` and `Benefit`.
- **Do not confuse with wallet `Momo`:** Adyen has a separate `AdyenPaymentMethod::Momo` with serde rename `momo_wallet` at `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1023-1024`. That is a **wallet** (`WalletData::MomoRedirection`), not a card redirect. `MomoAtm` is the ATM-card variant; `Momo` is the MoMo e-wallet. Keep them distinct.

### `CardRedirect {}`

- **Region:** Generic / unclassified card-redirect flow.
- **Status:** Unimplemented across all connectors at the pinned SHA. This is the catch-all variant and every `TryFrom<&CardRedirectData>` impl in the tree returns `IntegrationError::not_implemented` for it:
  - Adyen: `CardRedirectData::CardRedirect {} => Err(IntegrationError::not_implemented("payment_method").into())` at `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1831-1833`.
  - Stripe: combined not-implemented arm at `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:1479-1482`.
  - PayPal: combined not-implemented arm at `crates/integrations/connector-integration/src/connectors/paypal/transformers.rs:1164-1167`.
  - MultiSafepay: matched only at the top-level `Type` dispatcher (`crates/integrations/connector-integration/src/connectors/multisafepay/transformers.rs:79`); no per-variant code path.
- **Why it exists:** Placeholder for future issuer-hosted card redirects that don't have a dedicated variant (e.g. a new regional debit network). Authors adding support MUST pick one of:
  1. Add a dedicated variant (e.g. `Troy`, `RuPayRedirect`) and migrate connectors.
  2. Pass through a `connector_metadata` field to identify the specific scheme.
- **Until then:** Do not route new connectors through the generic `CardRedirect {}` variant — route through `Knet`, `Benefit`, or `MomoAtm` if one of those matches, otherwise open an enum-extension PR.

## Common Implementation Patterns

### Pattern 1: Map variant → connector payment-method tag, then delegate to shared request builder

Observed in Adyen. The connector already has a general `AdyenPaymentRequest<T>` used for most `PaymentMethodData` arms; CardRedirect reuses it by contributing only a `PaymentMethod::AdyenPaymentMethod(Box<AdyenPaymentMethod::Knet|Benefit|MomoAtm>)`.

```rust
// From crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1821-1836
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<&CardRedirectData> for AdyenPaymentMethod<T>
{
    type Error = Error;

    fn try_from(card_redirect_data: &CardRedirectData) -> Result<Self, Self::Error> {
        match card_redirect_data {
            CardRedirectData::Knet {} => Ok(Self::Knet),
            CardRedirectData::Benefit {} => Ok(Self::Benefit),
            CardRedirectData::MomoAtm {} => Ok(Self::MomoAtm),
            CardRedirectData::CardRedirect {} => {
                Err(IntegrationError::not_implemented("payment_method").into())
            }
        }
    }
}
```

### Pattern 2: Top-level dispatch from `PaymentMethodData`

The outer `TryFrom<AdyenRouterData<...>> for AdyenPaymentRequest<T>` dispatches on `payment_method_data` and routes CardRedirect to its sub-builder:

```rust
// From crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:3635-3641
PaymentMethodData::CardRedirect(ref card_redirect_data) => {
    Self::try_from((item, card_redirect_data)).map_err(|err| {
        err.change_context(IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        })
    })
}
```

### Pattern 3: Stub pattern — combined not-implemented arm

Every connector that acknowledges `CardRedirectData` but does not implement it uses a single combined match:

```rust
// From crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:1475-1483
PaymentMethodData::CardRedirect(cardredirect_data) => match cardredirect_data {
    CardRedirectData::Knet {}
    | CardRedirectData::Benefit {}
    | CardRedirectData::MomoAtm {}
    | CardRedirectData::CardRedirect {} => Err(IntegrationError::not_implemented(
        get_unimplemented_payment_method_error_message("stripe"),
    )
    .into()),
},
```

### Pattern 4: Redirect-response construction (shared with BankRedirect response shape)

The wire shape of the authorize response is identical to other redirect-flow PMs: populate `redirection_data: Some(Box<RedirectForm>)` on `PaymentsResponseData::TransactionResponse`. See the generic redirect-response example in `authorize/bank_redirect/pattern_authorize_bank_redirect.md` Pattern 1 ("Redirect Response"); the construction is mechanically the same (only the issuer URL host differs).

## Code Examples

### Example A — Full conversion chain (Adyen, `Knet`)

```rust
// From crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:2918-2955
// TryFrom implementation for converting CardRedirectData to AdyenPaymentRequest
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        AdyenRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
        &CardRedirectData,
    )> for AdyenPaymentRequest<T>
{
    type Error = Error;
    fn try_from(
        value: (
            AdyenRouterData<
                RouterDataV2<
                    Authorize,
                    PaymentFlowData,
                    PaymentsAuthorizeData<T>,
                    PaymentsResponseData,
                >,
                T,
            >,
            &CardRedirectData,
        ),
    ) -> Result<Self, Self::Error> {
        let (item, card_redirect_data) = value;
        let amount = get_amount_data(&item);
        let auth_type = AdyenAuthType::try_from(&item.router_data.connector_config)?;
        let shopper_interaction = AdyenShopperInteraction::from(&item.router_data);
        let return_url = item.router_data.request.get_router_return_url()?;
        let payment_method = PaymentMethod::AdyenPaymentMethod(Box::new(
            AdyenPaymentMethod::try_from(card_redirect_data)?,
        ));
        // ... address + metadata assembly (lines 2956+)
```

### Example B — Connector-side payment-method variant tags

```rust
// From crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:266-269
    Knet,
    Benefit,
    #[serde(rename = "momo_atm")]
    MomoAtm,
```

```rust
// From crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1025-1026
    #[serde(rename = "momo_atm")]
    MomoAtm,
```

```rust
// From crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1066-1067
    Knet,
    Benefit,
```

### Example C — Stub (PayPal)

```rust
// From crates/integrations/connector-integration/src/connectors/paypal/transformers.rs:1155-1169
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<&CardRedirectData> for PaypalPaymentsRequest<T>
{
    type Error = Report<IntegrationError>;
    fn try_from(value: &CardRedirectData) -> Result<Self, Self::Error> {
        match value {
            CardRedirectData::Knet {}
            | CardRedirectData::Benefit {}
            | CardRedirectData::MomoAtm {}
            | CardRedirectData::CardRedirect {} => Err(IntegrationError::not_implemented(
                utils::get_unimplemented_payment_method_error_message("Paypal"),
            )
            .into()),
        }
    }
}
```

### Example D — MultiSafepay dispatching on the parent arm only

```rust
// From crates/integrations/connector-integration/src/connectors/multisafepay/transformers.rs:77-80
    let payment_type = match payment_method_data {
        PaymentMethodData::Card(_) => Type::Direct,
        PaymentMethodData::CardRedirect(_) => Type::Redirect,
        PaymentMethodData::MandatePayment => Type::Direct,
```

```rust
// From crates/integrations/connector-integration/src/connectors/multisafepay/transformers.rs:240-243
        PaymentMethodData::CardRedirect(_) => {
            // Card redirect payments use generic credit card gateway
            Gateway::CreditCard
        }
```

## Best Practices

- **Use the enum's record-variant syntax exactly.** All four `CardRedirectData` variants are *record* variants (`Knet {}`, not `Knet`); matching MUST use `CardRedirectData::Knet {}` — see `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1828`. A tuple-style match (`CardRedirectData::Knet(_)`) will not compile.
- **Do not borrow card fields.** CardRedirect transformers MUST NOT reach through to `Card::card_number` or `Card::card_cvc`; the shopper never supplies them. Adyen's `TryFrom<&CardRedirectData> for AdyenPaymentMethod<T>` does not accept a `Card<T>` or a `RouterDataV2` (only `&CardRedirectData`), enforcing this at the type level — see `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1822-1826`.
- **Preserve serde renames.** `momo_atm` on Adyen (`crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:268-269`) is the issuer's expected tag. `Knet` and `Benefit` are NOT renamed (they serialize pascal-case on Adyen's wire by design).
- **Always populate `return_url`.** CardRedirect is a redirect flow; the authorize request requires `item.router_data.request.get_router_return_url()?` — omission is a runtime failure (`crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:2952`).
- **Pass billing address where available.** For issuer-country hinting, thread `get_address_info(item.router_data.resource_common_data.get_optional_billing())` into the request — see `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:2956-2958`.
- **When stubbing, use a combined match arm.** If your connector doesn't implement any CardRedirect variant, mirror Stripe's single-arm stub (`crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:1475-1483`) rather than four separate arms.
- **Do not conflate with `WalletData::MomoRedirection`.** Adyen's `Momo` (rename `momo_wallet`, line 1023-1024) is the MoMo e-wallet; this pattern is strictly about `CardRedirectData`. If the shopper chose a wallet, `PaymentMethodData::Wallet(_)` is the correct arm.

## Common Errors

### 1. Misclassifying CardRedirect as BankRedirect

- **Problem:** Developer places Knet/Benefit/MomoAtm logic inside the `PaymentMethodData::BankRedirect(_)` arm. This compiles (because `BankRedirectData` is a separate enum) but the `payment_method_data` never actually hits it for card-redirect flows, so the code is unreachable and the real CardRedirect arm falls through to the not-implemented case.
- **Solution:** Unwrap via `PaymentMethodData::CardRedirect(ref card_redirect_data)` specifically; see the Adyen dispatcher at `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:3635`.

### 2. Forgetting the record-variant braces

- **Problem:** `match card_redirect_data { CardRedirectData::Knet => ... }` — missing `{}`. Compile error `expected tuple struct or tuple variant, found unit variant`.
- **Solution:** Always `CardRedirectData::Knet {}`, `CardRedirectData::Benefit {}`, etc. — see `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1828-1831`.

### 3. Dropping the `momo_atm` serde rename

- **Problem:** Serializing `AdyenPaymentMethod::MomoAtm` as `"MomoAtm"` — Adyen responds with `422 Unprocessable Entity` because its schema expects `momo_atm`.
- **Solution:** Keep `#[serde(rename = "momo_atm")]` on both the request-side enum variant (`crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:268`) AND the response-side `PaymentType` variant (`crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1025`).

### 4. Swallowing the generic `CardRedirect {}` variant

- **Problem:** Implementer adds cases for `Knet`, `Benefit`, `MomoAtm` and forgets `CardRedirect {}`, producing a non-exhaustive match compile error.
- **Solution:** Explicitly return `IntegrationError::not_implemented(...)` for `CardRedirectData::CardRedirect {}` until a dedicated scheme emerges — mirror `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1831-1833`.

### 5. Expecting synchronous settlement

- **Problem:** Handler sets `AttemptStatus::Charged` on the authorize response and does not schedule a PSync, then reports success to the merchant before the shopper completes the redirect.
- **Solution:** Return `AttemptStatus::AuthenticationPending` + `redirection_data` on authorize; rely on PSync / webhook to transition to `Charged`. The response-construction shape is the same as in `authorize/bank_redirect/pattern_authorize_bank_redirect.md` "Redirect Response" pattern.

### 6. Using `ConnectorError` in new code

- **Problem:** Copying an older snippet that references monolithic `ConnectorError`. Per the spec's banned-types list, that type is retired (PR #765).
- **Solution:** Use `IntegrationError` for request-side failures (`IntegrationError::not_implemented`, `IntegrationError::RequestEncodingFailed`, `IntegrationError::MissingRequiredField`) and `ConnectorResponseTransformationError` for response-parse failures.

## Cross-References

- Parent README (PM index): [../README.md](../README.md)
- Patterns index: [../../README.md](../../README.md)
- **Closest sibling — Card flow:** [authorize/card/pattern_authorize_card.md](../card/pattern_authorize_card.md). Card handles direct card-data capture (PAN/CVC + 3DS). CardRedirect (this pattern) handles the same card rail but WITHOUT direct card-data capture — the issuer collects PAN/CVC in its own hosted portal. A connector implementing both will usually reuse the same outer `<Connector>PaymentRequest` struct and switch only on the `payment_method` tag (see Adyen reusing `AdyenPaymentRequest<T>` for both at `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:2918-2955` for CardRedirect and other arms of the dispatcher at `:3629-3630` for Card).
- **Adjacent but distinct sibling — BankRedirect:** [authorize/bank_redirect/pattern_authorize_bank_redirect.md](../bank_redirect/pattern_authorize_bank_redirect.md). BankRedirect (`BankRedirectData`) uses bank-transfer rails; CardRedirect (`CardRedirectData`) uses card rails. Prior to Wave-3C, CardRedirect content was **incorrectly** blended into that pattern; Wave 6 will remove it there. Do not look for Knet/Benefit/MomoAtm specifics in the bank-redirect pattern; this document is authoritative. Citation: the bank-redirect pattern at its current form does not enumerate `CardRedirectData` variants — see the file for structural reference only.
- **Sibling — Wallet flow:** [authorize/wallet/pattern_authorize_wallet.md](../wallet/pattern_authorize_wallet.md). Wallets (MoMo e-wallet, ApplePay, GooglePay) are a separate PM. Note specifically that Adyen's `Momo` (rename `momo_wallet`) lives under `WalletData::MomoRedirection` — see `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1023-1024` — whereas `MomoAtm` lives under `CardRedirectData` and is documented here.
- Type reference: [../../types/types.md](../../types/types.md). For `PaymentMethodData<T>`, `PaymentsAuthorizeData<T>`, `PaymentFlowData`, `RedirectForm`.
- Utility functions: [../../utility_functions_reference.md](../../utility_functions_reference.md). For `get_unimplemented_payment_method_error_message`, `get_address_info`, `get_router_return_url`.
- Flow-level pattern: [../../pattern_authorize.md](../../pattern_authorize.md). Generic Authorize flow semantics (dispatch loop, status mapping, redirect handling).

---

**Pattern Version:** 1.0.0
**Pinned SHA:** `ceb33736ce941775403f241f3f0031acbf2b4527`
**Authored:** Wave-3C (2026-04-20)
**Maintained By:** Grace-UCS Connector Team
