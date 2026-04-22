# Open Banking Authorize Flow Pattern

## Overview

Open Banking in Grace-UCS represents the Payment Method where a merchant pulls funds from a customer's bank account via a regulated Payment Initiation Service (PIS), after the customer authenticates on their bank (ASPSP) consent screen. The PM arm `PaymentMethodData::OpenBanking(OpenBankingData)` (see `crates/types-traits/domain_types/src/payment_method_data.rs:268`) wraps a dedicated `OpenBankingData` enum that today carries a single marker variant, `OpenBankingPIS {}`, reflecting that account-detail selection, bank choice, and authentication redirection all happen on the connector/ASPSP side rather than being pushed by UCS in the request body.

Unlike Card (PAN/CVC) or BankDebit (IBAN/routing+account), Open Banking authorize requests contain almost no payment-instrument fields on the UCS side — the interesting fields are the return URL for the consent redirect, the merchant-account beneficiary details (kept in connector metadata), and the amount/currency. All connectors that actually implement PIS flows today wire them through `PaymentMethodData::BankRedirect(BankRedirectData::OpenBankingUk { .. })` or `PaymentMethodData::BankRedirect(BankRedirectData::OpenBanking {})` rather than through `PaymentMethodData::OpenBanking(_)`, as verified below.

### Key Characteristics

| Property | Value |
|----------|-------|
| Enum | `OpenBankingData` at `crates/types-traits/domain_types/src/payment_method_data.rs:290` |
| Variants at pinned SHA | 1 (`OpenBankingPIS {}`) |
| PaymentMethodData arm | `OpenBanking(OpenBankingData)` at `crates/types-traits/domain_types/src/payment_method_data.rs:268` |
| Typical flow shape | Async + redirect (consent on ASPSP) |
| Regulatory frame | PSD2 (EU), UK Open Banking Standard, OBIE/CMA9 |
| Sensitive data held by merchant | None — no PAN, no IBAN entered by UCS |
| Current UCS connectors consuming `OpenBankingData::OpenBankingPIS` | (none) — all PIS traffic flows through `BankRedirectData` |
| UCS connectors serving PIS via `BankRedirectData` | `truelayer`, `volt` (see "Per-Variant Implementation Notes" below) |

---

## Table of Contents

1. [PSD2 / Open Banking background](#psd2--open-banking-background)
2. [Variant Enumeration](#variant-enumeration)
3. [Architecture Overview](#architecture-overview)
4. [Connectors with Full Implementation](#connectors-with-full-implementation)
5. [Per-Variant Implementation Notes](#per-variant-implementation-notes)
6. [Common Implementation Patterns](#common-implementation-patterns)
7. [Relationship with `BankRedirectData`](#relationship-with-bankredirectdata)
8. [Code Examples](#code-examples)
9. [Best Practices](#best-practices)
10. [Common Errors](#common-errors)
11. [Cross-References](#cross-references)

---

## PSD2 / Open Banking background

**PSD2 and SCA.** The EU Revised Payment Services Directive (PSD2, Directive (EU) 2015/2366) establishes two licensed third-party service roles: the Payment Initiation Service Provider (PISP) and the Account Information Service Provider (AISP). A PISP initiates a credit transfer from a Payment Service User's account at their bank; an AISP reads account balances and transaction history with the user's consent. The directive also mandates Strong Customer Authentication (SCA) — two of (knowledge, possession, inherence) — for any electronic payment and for any account access by a third party. SCA is what makes PIS flows redirect- or decoupled-authentication-driven: the user must finish authentication on their own bank, not on the merchant page.

**PIS vs AIS, TPP and ASPSP.** The two sides of every Open Banking call are the TPP (Third-Party Provider, that is, the PISP or AISP — in UCS that role is played by the connector, not by UCS itself) and the ASPSP (Account Servicing Payment Service Provider, the user's bank). For authorize flows the PM category Open Banking corresponds strictly to PIS: the connector, acting as PISP, files a payment-initiation request against the user's ASPSP and returns a hosted consent URL. AIS never appears in a `PaymentsAuthorizeData` flow and is out of scope for this pattern.

**Consent flow.** A canonical PIS authorize looks like: (1) merchant calls UCS Authorize with PM `OpenBanking` (or `BankRedirect::OpenBanking{Uk}`); (2) UCS dispatches to the connector transformer, which builds a "create payment" request including amount, currency, merchant beneficiary, and a `return_url`; (3) the connector responds with a redirect (`RedirectForm`) to a hosted ASPSP/consent page; (4) the user selects their bank, authenticates with SCA, and authorises the specific payment; (5) the ASPSP settles the credit transfer and notifies the connector via webhook; (6) UCS learns the terminal status through PSync or the incoming webhook. UK Open Banking (OBIE) and the Berlin Group NextGenPSD2 scheme implement the same high-level shape with different field names and endpoint layouts.

---

## Variant Enumeration

The `OpenBankingData` enum at the pinned SHA contains a single variant. The reviewer will diff this table against `crates/types-traits/domain_types/src/payment_method_data.rs:290-292` exactly.

| Variant | Data Shape | Citation | Used By (connectors) |
|---------|-----------|----------|----------------------|
| `OpenBankingPIS {}` | Unit-struct marker variant, no fields. Signals a PSD2/UK Open Banking Payment Initiation request. | `crates/types-traits/domain_types/src/payment_method_data.rs:291` | (none) — no connector at the pinned SHA matches `PaymentMethodData::OpenBanking(_)` as a success path; every connector listed in the "Grep OpenBanking(_)" sweep below maps this arm to `IntegrationError::not_implemented(...)`. PIS flows that do succeed (truelayer, volt) route through `BankRedirectData`, see below. |

**Parent arm.** The enum is wrapped in `PaymentMethodData::OpenBanking(OpenBankingData)` at `crates/types-traits/domain_types/src/payment_method_data.rs:268`.

**Note to reviewer.** A single-variant PM enum is rare but not unprecedented (the `OpenBankingPIS {}` marker exists as a placeholder for a future richer shape, e.g. fields for bank selection hints, country, or pre-selected ASPSP). Per PATTERN_AUTHORING_SPEC §9 the row is still required and the "Used By" cell must read "(none)" rather than being omitted.

---

## Architecture Overview

### Flow Type

`Authorize` marker, from `domain_types::connector_flow::Authorize`. The full router-data shape is:

```rust
RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
```

where `T: PaymentMethodDataTypes`. The marker and all four type parameters are mandated by PATTERN_AUTHORING_SPEC §7.

### Request Type

`PaymentsAuthorizeData<T>` — see `crates/types-traits/domain_types/src/connector_types.rs` for the full struct. The key fields an Open Banking transformer reads are:

- `payment_method_data: PaymentMethodData<T>` — unwrapped with `PaymentMethodData::OpenBanking(open_banking_data)`.
- `amount: MinorUnit` — Open Banking is a pull-credit-transfer, so amount is always minor.
- `currency: common_enums::Currency` — the ASPSP settlement currency.
- `router_return_url: Option<String>` — where the ASPSP hosted consent page sends the user after authentication. Required for redirect flows.
- `email`, `customer_name` — surfaced on the consent screen by some ASPSPs; required by some connectors (`truelayer` requires at least one of `email` or `phone`, see below).

### Response Type

`PaymentsResponseData` — in particular the `TransactionResponse` variant with `redirection_data: Some(Box::new(RedirectForm::Form { .. }))` for the bank-consent redirect, and `resource_id: ResponseId::ConnectorTransactionId(_)` for later PSync/webhook correlation.

### Resource Common Data

`PaymentFlowData` — see `crates/types-traits/domain_types/src/connector_types.rs:422`. Open Banking transformers typically read `resource_common_data.address` for billing, `resource_common_data.connectors.<name>` for the configured `base_url`, and `resource_common_data.get_connector_customer_id()` for a merchant-side customer reference.

### Variant unwrap sketch

```rust
match &item.router_data.request.payment_method_data {
    // --- The typed Open Banking arm: accepted by no connector at the pinned SHA ---
    PaymentMethodData::OpenBanking(OpenBankingData::OpenBankingPIS {}) => {
        // If a future connector wires this, build the PIS create-payment request here.
        Err(IntegrationError::not_implemented(
            domain_types::utils::get_unimplemented_payment_method_error_message("<connector>"),
        )
        .into())
    }
    // --- The actual PIS success paths at the pinned SHA ---
    PaymentMethodData::BankRedirect(BankRedirectData::OpenBankingUk { .. }) => { /* ... */ }
    PaymentMethodData::BankRedirect(BankRedirectData::OpenBanking {}) => { /* ... */ }
    _ => Err(IntegrationError::not_implemented(...).into()),
}
```

---

## Connectors with Full Implementation

At the pinned SHA, **no connector implements `PaymentMethodData::OpenBanking(_)` as a success path**. A file-by-file grep of `crates/integrations/connector-integration/src/connectors/` for the pattern `PaymentMethodData::OpenBanking(_)` returns only rejection arms, all returning `IntegrationError::not_implemented(...)` or the helper `get_unimplemented_payment_method_error_message(...)`.

For completeness and to keep the table non-empty, we list below the connectors that serve the regulatory-equivalent PIS flow through `PaymentMethodData::BankRedirect(_)`. These rows are informational only; the spec-mandated table header is preserved.

| Connector | HTTP Method | Content Type | URL Pattern | Request Type Reuse | Notes |
|-----------|-------------|--------------|-------------|--------------------|-------|
| Truelayer | POST | application/json | `{base_url}/v3/payments` | `TruelayerPaymentsRequestData` (no reuse across flows) | Wires UK Open Banking via `PaymentMethodData::BankRedirect(BankRedirectData::OpenBankingUk { .. })`. See `crates/integrations/connector-integration/src/connectors/truelayer/transformers.rs:335`. URL at `crates/integrations/connector-integration/src/connectors/truelayer.rs:506-507`. |
| Volt | POST | application/json | `{base_url}/payments` | `VoltPaymentsRequest` (no reuse across flows) | Accepts both `BankRedirectData::OpenBankingUk { .. }` and `BankRedirectData::OpenBanking {}`, selecting `PaymentSystem::OpenBankingUk` or `PaymentSystem::OpenBankingEu` by currency. See `crates/integrations/connector-integration/src/connectors/volt/transformers.rs:182-207`. URL at `crates/integrations/connector-integration/src/connectors/volt.rs:410-411`. |

### Stub Implementations

Every other connector in `crates/integrations/connector-integration/src/connectors/` matches `PaymentMethodData::OpenBanking(_)` only in a rejection arm. Non-exhaustive list derived from the grep `PaymentMethodData::OpenBanking\(_\)`:

- `aci` — `crates/integrations/connector-integration/src/connectors/aci/transformers.rs:748`
- `adyen` — `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:3701`, `:6042`
- `bambora` — `crates/integrations/connector-integration/src/connectors/bambora/transformers.rs:298`
- `bankofamerica` — `crates/integrations/connector-integration/src/connectors/bankofamerica/transformers.rs:613`, `:1777`
- `billwerk` — `crates/integrations/connector-integration/src/connectors/billwerk/transformers.rs:233`
- `braintree` — `crates/integrations/connector-integration/src/connectors/braintree/transformers.rs:610`, `:1601`, `:2623`, `:2812`
- `cryptopay` — `crates/integrations/connector-integration/src/connectors/cryptopay/transformers.rs:109`
- `cybersource` — `crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:321`, `:2186`, `:2286`, `:3025`, `:3302`, `:4323`
- `dlocal` — `crates/integrations/connector-integration/src/connectors/dlocal/transformers.rs:207`
- `fiserv` — `crates/integrations/connector-integration/src/connectors/fiserv/transformers.rs:548`
- `fiuu` — `crates/integrations/connector-integration/src/connectors/fiuu/transformers.rs:673`
- `forte` — `crates/integrations/connector-integration/src/connectors/forte/transformers.rs:311`
- `hipay` — `crates/integrations/connector-integration/src/connectors/hipay/transformers.rs:595`
- `loonio` — `crates/integrations/connector-integration/src/connectors/loonio/transformers.rs:246`
- `mifinity` — `crates/integrations/connector-integration/src/connectors/mifinity/transformers.rs:247`
- `multisafepay` — `crates/integrations/connector-integration/src/connectors/multisafepay/transformers.rs:155`, `:335`
- `nexinets` — `crates/integrations/connector-integration/src/connectors/nexinets/transformers.rs:739`
- `noon` — `crates/integrations/connector-integration/src/connectors/noon/transformers.rs:376`, `:1261`
- `paypal` — `crates/integrations/connector-integration/src/connectors/paypal/transformers.rs:1141`, `:2605`
- `placetopay` — `crates/integrations/connector-integration/src/connectors/placetopay/transformers.rs:209`
- `razorpay` — `crates/integrations/connector-integration/src/connectors/razorpay/transformers.rs:309`
- `redsys` — `crates/integrations/connector-integration/src/connectors/redsys/transformers.rs:248`
- `stax` — `crates/integrations/connector-integration/src/connectors/stax/transformers.rs:1097`
- `stripe` — `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:1515`, `:4643`, `:5037`
- `trustpay` — `crates/integrations/connector-integration/src/connectors/trustpay/transformers.rs:1710`
- `truelayer` — rejects `PaymentMethodData::OpenBanking(_)` on the fall-through `_` arm (see `crates/integrations/connector-integration/src/connectors/truelayer/transformers.rs:429-433`).
- `volt` — `crates/integrations/connector-integration/src/connectors/volt/transformers.rs:294`
- `wellsfargo` — `crates/integrations/connector-integration/src/connectors/wellsfargo/transformers.rs:595`
- `worldpay` — `crates/integrations/connector-integration/src/connectors/worldpay/transformers.rs:221`

This is a load-bearing finding: adding a new connector on top of `OpenBankingData::OpenBankingPIS` is a legitimate greenfield — the field is reserved but unused. In the meantime every Authorize-time OB flow in the repository is served by the BankRedirect pattern.

---

## Per-Variant Implementation Notes

### `OpenBankingPIS {}`

- **Shape**: unit-struct variant, no fields. Defined at `crates/types-traits/domain_types/src/payment_method_data.rs:291`.
- **Semantics**: "the caller intends a PSD2 / UK-Open-Banking Payment-Initiation Service payment; the connector is expected to pick the country/scheme, present the ASPSP list, and drive SCA on its hosted page."
- **Current status at the pinned SHA**: no connector consumes this arm on a success path. Every `match` against `PaymentMethodData` that mentions `OpenBanking(_)` lists it alongside `CardToken(_)`, `NetworkToken(_)` etc. in the not-implemented branch.
- **Transformer obligations for a future connector** consuming `OpenBankingData::OpenBankingPIS` on Authorize:
  1. Read `amount` and `currency` from `PaymentsAuthorizeData`.
  2. Read `router_return_url` — required, because the ASPSP consent page must redirect the user back.
  3. Read `resource_common_data.connector_config` / metadata for the merchant beneficiary (account name, account identifier, sort code or IBAN depending on scheme).
  4. Pick the scheme (UK OBIE or EU Berlin Group) from currency/country. The volt transformer at `crates/integrations/connector-integration/src/connectors/volt/transformers.rs:192-206` demonstrates this exact switch: `GBP` → `PaymentSystem::OpenBankingUk`, anything else → `PaymentSystem::OpenBankingEu`.
  5. Build the connector's PIS-create request, issue it, and return `PaymentsResponseData::TransactionResponse` with `redirection_data: Some(Box::new(RedirectForm::Form { endpoint, method: Method::Get or Post, form_fields }))` populated from the connector response.
  6. Propagate the resulting status as `AttemptStatus::AuthenticationPending` (or an equivalent "waiting for user at ASPSP" state) until webhook or PSync confirms settlement.

- **Gotcha**: there is no per-bank issuer field on `OpenBankingData::OpenBankingPIS`, unlike `BankRedirectData::Przelewy24 { bank_name: Option<BankNames> }` (`crates/types-traits/domain_types/src/payment_method_data.rs:649-651`) or `BankRedirectData::OpenBankingUk { issuer, country }` (`crates/types-traits/domain_types/src/payment_method_data.rs:645-648`). If a connector needs the user to pre-select a bank, either (a) require the user to select it on the connector's hosted page, or (b) route the request via `BankRedirectData::OpenBankingUk` where the `issuer` / `country` fields are available.

---

## Common Implementation Patterns

### Pattern 1: Route via `BankRedirectData` (current practice)

Every successful PIS Authorize path at the pinned SHA follows this shape:

```rust
match &item.router_data.request.payment_method_data {
    PaymentMethodData::BankRedirect(BankRedirectData::OpenBankingUk { .. }) => {
        // Build PIS create-payment request against the connector's UK endpoint.
    }
    PaymentMethodData::BankRedirect(BankRedirectData::OpenBanking {}) => {
        // Build PIS create-payment request against the connector's EU endpoint,
        // picking UK vs EU by currency.
    }
    _ => Err(IntegrationError::not_implemented(...).into()),
}
```

Seen in `crates/integrations/connector-integration/src/connectors/truelayer/transformers.rs:335` (UK only) and `crates/integrations/connector-integration/src/connectors/volt/transformers.rs:186-206` (UK + EU dispatch).

### Pattern 2: Reject `OpenBanking(_)` and hand the caller an error

All other connectors in the grep list above. The idiomatic rejection is:

```rust
// From crates/integrations/connector-integration/src/connectors/bankofamerica/transformers.rs:613
PaymentMethodData::OpenBanking(_) => Err(IntegrationError::not_implemented(
    domain_types::utils::get_unimplemented_payment_method_error_message("Bank of America"),
).into()),
```

Authors **must** include `OpenBanking(_)` in the exhaustive match for any new connector even if it is not supported; the `PaymentMethodData` enum is `#[non_exhaustive]`-adjacent (each new arm in `payment_method_data.rs:268` must be reached) and an omitted arm triggers a compile-time non-exhaustive match error.

### Pattern 3: Placeholder for future richer shape

If a future PR enriches `OpenBankingData`, e.g. with a `OpenBankingPIS { country: Option<CountryAlpha2>, preferred_aspsp: Option<BankNames> }` payload, this pattern file must be refreshed per PATTERN_AUTHORING_SPEC §4 (variant-enumeration refresh obligation). Until then, authors may not invent fields on `OpenBankingPIS`.

---

## Relationship with `BankRedirectData`

`OpenBankingUk` is **not** a variant of `OpenBankingData`. It is a variant of `BankRedirectData` at `crates/types-traits/domain_types/src/payment_method_data.rs:645-648`:

```rust
OpenBankingUk {
    issuer: Option<common_enums::BankNames>,
    country: Option<CountryAlpha2>,
},
```

Similarly `BankRedirectData::OpenBanking {}` at `crates/types-traits/domain_types/src/payment_method_data.rs:669` is a unit-struct variant inside the BankRedirect category.

These two BankRedirect variants share the *regulatory concept* "PSD2 / UK Open Banking Payment Initiation" with `OpenBankingData::OpenBankingPIS`, but they live under a different PM category and are consumed by different PM patterns:

- The `BankRedirectData::OpenBankingUk { .. }` and `BankRedirectData::OpenBanking {}` variants are documented in the sibling PM pattern [`../bank_redirect/pattern_authorize_bank_redirect.md`](../bank_redirect/pattern_authorize_bank_redirect.md).
- Per-connector transformer matches on the BankRedirect versions (truelayer, volt) belong there, not here.
- This pattern file owns only `PaymentMethodData::OpenBanking(OpenBankingData::OpenBankingPIS {})`.

If a reader is looking for the "open banking that actually works today", they need the bank_redirect pattern. If they are wiring up a connector that *explicitly* exposes a PIS-only SKU independent of country (for instance a PSP that only offers UK OB and has its own scheme-agnostic endpoint), the `OpenBankingData` arm is the correct tagging.

**Why two places for the same concept?** The domain enum layout predates this pattern-authoring pass. `BankRedirectData` encodes "user is redirected to a bank-style screen" generically, and historically the Open Banking subset was added there for connectors that already had a BankRedirect code path. `OpenBankingData` was subsequently added as a parallel PM arm for connectors that want to distinguish PIS-specific routing from legacy bank-redirect variants like Giropay or Sofort. The two shapes are intentionally separate in the type system; UCS does not auto-fold one into the other.

---

## Code Examples

### Example 1: Enum definition at the pinned SHA

```rust
// From crates/types-traits/domain_types/src/payment_method_data.rs:288
#[derive(Debug, Clone, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenBankingData {
    OpenBankingPIS {},
}
```

### Example 2: Parent `PaymentMethodData` arm

```rust
// From crates/types-traits/domain_types/src/payment_method_data.rs:268
pub enum PaymentMethodData<T: PaymentMethodDataTypes> {
    // ...
    CardToken(CardToken),
    OpenBanking(OpenBankingData),
    NetworkToken(NetworkTokenData),
    // ...
}
```

### Example 3: Actual PIS request in `truelayer` (via BankRedirect, for reference)

```rust
// From crates/integrations/connector-integration/src/connectors/truelayer/transformers.rs:334
match &item.router_data.request.payment_method_data {
    PaymentMethodData::BankRedirect(BankRedirectData::OpenBankingUk { .. }) => {
        let currency = item.router_data.request.currency;
        let amount_in_minor = item.router_data.request.amount;

        let hosted_page = HostedPage {
            return_uri: item.router_data.request.router_return_url.clone().ok_or(
                IntegrationError::MissingRequiredField {
                    field_name: "return_url",
                    context: Default::default(),
                },
            )?,
        };

        let metadata = TruelayerMetadata::try_from(&item.router_data.connector_config)?;

        let payment_method = PaymentMethod {
            _type: "bank_transfer".to_string(),
            provider_selection: ProviderSelection {
                _type: "user_selected".to_string(),
            },
            beneficiary: Beneficiary {
                _type: "merchant_account".to_string(),
                merchant_account_id: metadata.merchant_account_id.clone(),
                account_holder_name: metadata.account_holder_name.clone(),
            },
        };
        // ... user, email, phone ...
        Ok(Self { amount_in_minor, currency, hosted_page, payment_method, user })
    }
    _ => Err(IntegrationError::not_implemented(
        utils::get_unimplemented_payment_method_error_message("Truelayer"),
    ).into()),
}
```

The Truelayer Authorize URL is constructed at `crates/integrations/connector-integration/src/connectors/truelayer.rs:506-507` as `{base_url}/v3/payments`, POST, `application/json`.

### Example 4: Volt's UK-vs-EU dispatch by currency

```rust
// From crates/integrations/connector-integration/src/connectors/volt/transformers.rs:186
let (payment_system, open_banking_u_k, open_banking_e_u) = match bank_redirect {
    BankRedirectData::OpenBankingUk { .. } => Ok((
        PaymentSystem::OpenBankingUk,
        Some(OpenBankingUk { transaction_type }),
        None,
    )),
    BankRedirectData::OpenBanking {} => {
        if matches!(currency, common_enums::Currency::GBP) {
            Ok((
                PaymentSystem::OpenBankingUk,
                Some(OpenBankingUk { transaction_type }),
                None,
            ))
        } else {
            Ok((
                PaymentSystem::OpenBankingEu,
                None,
                Some(OpenBankingEu { transaction_type }),
            ))
        }
    }
    // ... other BankRedirectData variants rejected ...
}?;
```

### Example 5: Canonical rejection arm for a non-OB connector

```rust
// From crates/integrations/connector-integration/src/connectors/bankofamerica/transformers.rs:613
| PaymentMethodData::OpenBanking(_)
// ... other unsupported arms ...
=> Err(IntegrationError::not_implemented(
    domain_types::utils::get_unimplemented_payment_method_error_message("Bank of America"),
).into())
```

### Example 6: Volt also rejects the typed `OpenBanking` arm despite supporting PIS

```rust
// From crates/integrations/connector-integration/src/connectors/volt/transformers.rs:294
| PaymentMethodData::OpenBanking(_)
// ...
=> Err(IntegrationError::not_implemented(...).into())
```

This is the load-bearing confirmation that `OpenBankingData::OpenBankingPIS {}` has no consumer at the pinned SHA: even the connector whose whole identity is PIS (`volt`) rejects this arm and expects callers to use `BankRedirectData` instead.

---

## Best Practices

- **Always unwrap via exhaustive match.** When adding or editing a connector transformer, explicitly list `PaymentMethodData::OpenBanking(_)` in the match so the compiler flags any future `OpenBankingData` variant addition. Confirmed in `crates/integrations/connector-integration/src/connectors/trustpay/transformers.rs:1710`.
- **Reject with `get_unimplemented_payment_method_error_message`.** For connectors that do not support PIS, use the shared helper `domain_types::utils::get_unimplemented_payment_method_error_message("<connector name>")`. This keeps error wording consistent. See `crates/integrations/connector-integration/src/connectors/bankofamerica/transformers.rs:618-620`.
- **Require `router_return_url` up front.** PIS cannot complete without a return URL from the ASPSP consent page. Fail fast with `IntegrationError::MissingRequiredField { field_name: "return_url", .. }` as truelayer does at `crates/integrations/connector-integration/src/connectors/truelayer/transformers.rs:340-346`.
- **Require contact info when the ASPSP demands it.** UK OBIE flows typically surface the payer's email or phone on the consent screen. Truelayer enforces "at least one of email/phone" at `crates/integrations/connector-integration/src/connectors/truelayer/transformers.rs:381-387`. Reuse that pattern rather than silently omitting the field.
- **Derive scheme from currency, not from caller input.** Volt's `match currency { GBP => UK, _ => EU }` switch at `crates/integrations/connector-integration/src/connectors/volt/transformers.rs:192-206` is the reference pattern when `OpenBankingData::OpenBankingPIS` is eventually wired up to a connector that offers both schemes.
- **Surface the consent redirect as `RedirectForm::Form`.** PIS is never sync-success at authorize time. Always populate `PaymentsResponseData::TransactionResponse { redirection_data: Some(Box::new(RedirectForm::Form { .. })), .. }`. The response-mapping pattern is documented in the Card pattern's "Redirect Pattern" section; see [`../card/pattern_authorize_card.md`](../card/pattern_authorize_card.md).
- **Map status to `AuthenticationPending` before the webhook.** PIS authorize is asynchronous; the connector status at create-time is typically `pending`/`authorization_required`/`submitted`, which maps to `AttemptStatus::AuthenticationPending`. Terminal statuses arrive via webhook or PSync.

---

## Common Errors

### 1. Missing return_url

- **Problem**: Connector rejects the Authorize call because no `return_uri` was supplied.
- **Solution**: Fail in the transformer with `IntegrationError::MissingRequiredField { field_name: "return_url", context: Default::default() }` before the HTTP call, mirroring `crates/integrations/connector-integration/src/connectors/truelayer/transformers.rs:340-346`.

### 2. Routing PIS through `OpenBankingData` instead of `BankRedirectData`

- **Problem**: A caller passes `PaymentMethodData::OpenBanking(OpenBankingData::OpenBankingPIS {})` and UCS returns "not implemented" because no connector consumes that arm at the pinned SHA.
- **Solution**: Use `PaymentMethodData::BankRedirect(BankRedirectData::OpenBankingUk { .. })` or `PaymentMethodData::BankRedirect(BankRedirectData::OpenBanking {})` for any Authorize that must succeed today. Consult [`../bank_redirect/pattern_authorize_bank_redirect.md`](../bank_redirect/pattern_authorize_bank_redirect.md) for field-level guidance.

### 3. Treating PIS as sync

- **Problem**: Transformer maps create-payment HTTP 200 directly to `AttemptStatus::Charged`.
- **Solution**: PIS is always async and redirect-driven. Map create-time status to `AttemptStatus::AuthenticationPending` and rely on webhook/PSync for the terminal state. See the Redsys 3DS redirect build at `crates/integrations/connector-integration/src/connectors/redsys/transformers.rs` for the shape of `RedirectForm::Form` emission (adapted: Open Banking uses the ASPSP URL, not `acs_u_r_l`).

### 4. Exhaustive-match drift after enum refresh

- **Problem**: A PR adds a second variant to `OpenBankingData` (for example `OpenBankingAISExtended { .. }`) and every connector's rejection arm silently continues to compile because `OpenBanking(_)` still matches.
- **Solution**: When enriching `OpenBankingData`, PR authors must refresh this pattern file (rule §4 of PATTERN_AUTHORING_SPEC) and update connectors that need per-variant behaviour to match by variant, not by the opaque `(_)` wildcard.

### 5. Confusing `OpenBankingUk` with `OpenBankingData`

- **Problem**: Author references `OpenBankingData::OpenBankingUk` — a type that does not exist.
- **Solution**: `OpenBankingUk` is only `BankRedirectData::OpenBankingUk { issuer, country }` at `crates/types-traits/domain_types/src/payment_method_data.rs:645-648`. The only variant of `OpenBankingData` is `OpenBankingPIS {}` at `crates/types-traits/domain_types/src/payment_method_data.rs:291`.

---

## Cross-References

- Pattern authoring spec: [`../../PATTERN_AUTHORING_SPEC.md`](../../PATTERN_AUTHORING_SPEC.md)
- Authorize PM index: [`../README.md`](../README.md)
- Patterns index: [`../../README.md`](../../README.md)
- Sibling PM pattern (authoritative for today's PIS traffic): [`../bank_redirect/pattern_authorize_bank_redirect.md`](../bank_redirect/pattern_authorize_bank_redirect.md)
- Sibling PM pattern (redirect response-mapping reference): [`../card/pattern_authorize_card.md`](../card/pattern_authorize_card.md)
- Sibling PM pattern (another async+hosted-page flow): [`../wallet/pattern_authorize_wallet.md`](../wallet/pattern_authorize_wallet.md)

---

## Appendix: Summary of verification commands at the pinned SHA

The claims in this document were verified against `ceb33736ce941775403f241f3f0031acbf2b4527` with these searches:

- `rg -n 'pub enum OpenBankingData' crates/types-traits/domain_types/src/payment_method_data.rs` — located the enum at line 290.
- `rg -n 'OpenBankingPIS' crates/` — a single hit at `crates/types-traits/domain_types/src/payment_method_data.rs:291`. No connector references the variant by name.
- `rg -n 'PaymentMethodData::OpenBanking\(' crates/integrations/connector-integration/src/connectors/` — 40+ hits, all in rejection arms.
- `rg -n 'BankRedirectData::OpenBankingUk|BankRedirectData::OpenBanking ' crates/integrations/connector-integration/src/connectors/` — hits in `truelayer/transformers.rs:335`, `volt/transformers.rs:187,192`.
- `rg -n 'OpenBanking' crates/integrations/connector-integration/src/connectors/stripe/transformers.rs` — one hit at `:877` referring to `common_enums::PaymentMethodType::OpenBankingPIS`, which is a PMT classifier (not the `PaymentMethodData` arm) and outside the scope of this pattern.
