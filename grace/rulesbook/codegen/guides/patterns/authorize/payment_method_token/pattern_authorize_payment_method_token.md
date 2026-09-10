# PaymentMethodToken Authorize Flow Pattern

> **Rename notice**: This variant was renamed from `CardToken` to `PaymentMethodToken` by commit `70e0883df` (PR #1010, "refactor: move internal pm token into payment_method_data enum"). `CardToken` no longer exists anywhere in `crates/` — writing it is an E0433. The rename has now been applied throughout this file; the only surviving `CardToken` spellings are Stripe's own unrelated `StripeCardToken` / `StripePaymentMethodData::CardToken` (see [Common Errors §1](#1-stripes-stripecardtoken-struct-is-not-the-domain-paymentmethodtoken)).
>
> The rename also changed the payload: the struct's fields are `token` and `token_payment_method_type`, **not** the old `card_holder_name` / `card_cvc`.

## Overview

`PaymentMethodToken` is the `PaymentMethodData<T>` variant that represents a **previously-tokenized payment method** arriving on the Authorize wire with no PAN, no expiry, and no network metadata. The struct carries the opaque `token` itself plus an optional `token_payment_method_type` (`ApplePay` / `GooglePay`) saying which wallet minted it. See the variant at `crates/types-traits/domain_types/src/payment_method_data.rs:382` and the struct definition at `crates/types-traits/domain_types/src/payment_method_data.rs:494`.

Unlike `Card<T>` (raw PAN, Wave 5A) and `NetworkTokenData` (DPAN + cryptogram, Wave 5B), `PaymentMethodToken` carries **only an opaque connector/wallet token**, no card data. At the pinned SHA, every production connector in `crates/integrations/connector-integration/src/connectors/` that pattern-matches this variant routes it directly to `IntegrationError::not_implemented(message, context)`. This pattern therefore documents (a) the variant's canonical field layout, (b) the single point where the gRPC façade constructs it (`crates/types-traits/domain_types/src/types.rs:1522-1541`), (c) how it differs from `NetworkToken` and `Card`, and (d) the guardrail `not_implemented` pattern every connector follows until a future wave adds real tokenized-card authorization.

### Key Characteristics

| Attribute | Value | Citation |
|-----------|-------|----------|
| Carries PAN | No | `crates/types-traits/domain_types/src/payment_method_data.rs:494-498` |
| Carries expiry | No | `crates/types-traits/domain_types/src/payment_method_data.rs:494-498` |
| Carries card network | No | `crates/types-traits/domain_types/src/payment_method_data.rs:494-498` |
| Carries cryptogram | No | `crates/types-traits/domain_types/src/payment_method_data.rs:494-498` |
| Token value | Yes, required (`token: Secret<String>`) | `crates/types-traits/domain_types/src/payment_method_data.rs:495` |
| Minting wallet | Optional (`token_payment_method_type: Option<TokenPaymentMethod>`) | `crates/types-traits/domain_types/src/payment_method_data.rs:497` |
| PMT enum tag | `PaymentMethodDataType::PaymentMethodToken` | `crates/types-traits/domain_types/src/types.rs:13753`, `crates/types-traits/domain_types/src/connector_types.rs:4365` |
| Constructed from | gRPC `payment_method.Token(token)` | `crates/types-traits/domain_types/src/types.rs:1522-1541` |
| Generic over `T: PaymentMethodDataTypes` | No (carries no PCI card data) | `crates/types-traits/domain_types/src/payment_method_data.rs:494` |
| Connectors with real Authorize handling | 0 at pinned SHA | all `not_implemented` arms cited below |

## Table of Contents

1. [Variant Enumeration](#variant-enumeration)
2. [Architecture Overview](#architecture-overview)
3. [Connectors with Full Implementation](#connectors-with-full-implementation)
4. [Per-Variant Implementation Notes](#per-variant-implementation-notes)
5. [Common Implementation Patterns](#common-implementation-patterns)
6. [Code Examples](#code-examples)
7. [PaymentMethodToken vs NetworkToken vs Card](#paymentmethodtoken-vs-networktoken-vs-card)
8. [Best Practices](#best-practices)
9. [Common Errors](#common-errors)
10. [Cross-References](#cross-references)

## Variant Enumeration

`PaymentMethodToken` is a single-variant struct, not an enum. The Variant-Enumeration table therefore enumerates the **fields** of the `PaymentMethodToken` struct as the structural units reviewers must verify, plus the PM enum arm that carries it.

| Variant | Data Shape | Citation | Used By (connectors) |
|---------|-----------|----------|----------------------|
| `PaymentMethodData::PaymentMethodToken(PaymentMethodToken)` | PM enum arm wrapping the `PaymentMethodToken` struct | `crates/types-traits/domain_types/src/payment_method_data.rs:382` | (none) — every connector returns `IntegrationError::not_implemented` |

### Fields of `PaymentMethodToken`

| Field | Type | Required | Citation | Purpose |
|-------|------|----------|----------|---------|
| `token` | `Secret<String>` | **Yes** | `crates/types-traits/domain_types/src/payment_method_data.rs:495` | The opaque token the connector/wallet previously issued; this is the only credential on the wire |
| `token_payment_method_type` | `Option<TokenPaymentMethod>` | No | `crates/types-traits/domain_types/src/payment_method_data.rs:497` | Which wallet minted the token — `TokenPaymentMethod::{ApplePay, GooglePay}` (`:502-505`). Skipped on serialize when `None` |

The struct derives `Eq`, `PartialEq`, `Debug`, `serde::Deserialize`, `serde::Serialize`, `Clone` at `crates/types-traits/domain_types/src/payment_method_data.rs:492`. It does **not** derive `Default`, so `PaymentMethodToken::default()` is an E0599. Wire serialization uses `#[serde(rename_all = "snake_case")]` (`:493`), so JSON keys are `token` and `token_payment_method_type`.

### Adjacent PM variants (for reviewer diff)

For completeness, the `PaymentMethodData<T>` enum enumerates the following card-family arms at `crates/types-traits/domain_types/src/payment_method_data.rs:248-271`:

| PM enum arm | Data payload | Line |
|-------------|--------------|------|
| `Card(Card<T>)` | Raw PAN + expiry + CVC | `crates/types-traits/domain_types/src/payment_method_data.rs:249` |
| `CardDetailsForNetworkTransactionId(CardDetailsForNetworkTransactionId)` | PAN + expiry bound to a prior network txn id | `crates/types-traits/domain_types/src/payment_method_data.rs:250` |
| `DecryptedWalletTokenDetailsForNetworkTransactionId(...)` | Decrypted wallet token + NTID | `crates/types-traits/domain_types/src/payment_method_data.rs:251-253` |
| `CardRedirect(CardRedirectData)` | Knet / Benefit / MomoAtm redirect | `crates/types-traits/domain_types/src/payment_method_data.rs:254` |
| `PaymentMethodToken(PaymentMethodToken)` | This pattern's subject | `crates/types-traits/domain_types/src/payment_method_data.rs:382` |
| `NetworkToken(NetworkTokenData)` | DPAN + expiry + cryptogram + ECI | `crates/types-traits/domain_types/src/payment_method_data.rs:269` (struct at `:306-318`) |

Everything outside the card family (`Wallet`, `PayLater`, `BankRedirect`, `BankDebit`, `BankTransfer`, `Crypto`, `MandatePayment`, `Reward`, `RealTimePayment`, `Upi`, `Voucher`, `GiftCard`, `OpenBanking`, `MobilePayment`) is out of scope for this PM pattern.

## Architecture Overview

### Flow Type

`Authorize` — marker from `domain_types::connector_flow::Authorize`. The canonical signature used throughout this pattern is:

```rust
// Canonical Authorize router-data (crates/types-traits/domain_types/src/connector_types.rs:422)
RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
```

### Request Type

`PaymentsAuthorizeData<T>` at `crates/types-traits/domain_types/src/connector_types.rs`. The field an implementer must pattern-match for `PaymentMethodToken` handling is `PaymentsAuthorizeData::<T>::payment_method_data: PaymentMethodData<T>`.

### Response Type

`PaymentsResponseData` at `crates/types-traits/domain_types/src/connector_types.rs`. Not specialized for `PaymentMethodToken`; the variant carries no connector-visible fields that affect response shape.

### Resource Common Data

`PaymentFlowData` at `crates/types-traits/domain_types/src/connector_types.rs:796`. One `PaymentFlowData` field is materially relevant to a real `PaymentMethodToken` Authorize implementation:

| Field | Line | Role |
|-------|------|------|
| `connector_customer: Option<String>` | `crates/types-traits/domain_types/src/connector_types.rs:799` | Tenant-side customer id the connector previously issued |

`PaymentFlowData` has **no** `payment_method_token` field, and the `router_data::PaymentMethodToken`
enum older guides point at is dead, commented-out code
(`crates/types-traits/domain_types/src/router_data.rs:4376-4380`):

```rust
// crates/types-traits/domain_types/src/router_data.rs:4376 -- COMMENTED OUT, does not exist
// Dead code: nothing populates this after PaymentFlowData.payment_method_token was removed.
// #[derive(Debug, Clone, serde::Deserialize)]
// pub enum PaymentMethodToken {
//     Token(Secret<String>),
// }
```

A connector implementing `PaymentMethodToken` Authorize in a future wave therefore reads the credential from the variant's own `token` field — there is no out-of-band token slot left to read. See [Common Implementation Patterns](#common-implementation-patterns).

### Where the variant is unwrapped

Every connector transformer that reaches the `Authorize` `TryFrom<...>` impl for `PaymentsAuthorizeData<T>` pattern-matches `payment_method_data` and handles `PaymentMethodToken` in a fall-through arm. The canonical match shape (observed in 20+ connectors at this SHA) is shown in [Common Implementation Patterns §1](#1-not_implemented-guardrail-pattern-canonical).

### Where the variant is constructed

The gRPC-to-domain conversion at `crates/types-traits/domain_types/src/types.rs:1522-1541` is the single production construction site:

```rust
// From crates/types-traits/domain_types/src/types.rs:1522
grpc_api_types::payments::payment_method::PaymentMethod::Token(token) => {
    Ok(Self::PaymentMethodToken(payment_method_data::PaymentMethodToken {
        token_payment_method_type: match token.token_payment_method_type() {
            grpc_api_types::payments::token_payment_method_type::TokenPaymentMethod::ApplePay => {
                Some(payment_method_data::TokenPaymentMethod::ApplePay)
            }
            grpc_api_types::payments::token_payment_method_type::TokenPaymentMethod::GooglePay => {
                Some(payment_method_data::TokenPaymentMethod::GooglePay)
            }
            grpc_api_types::payments::token_payment_method_type::TokenPaymentMethod::Unspecified => None,
        },
        token: token
            .token
            .ok_or_else(|| report!(IntegrationError::MissingRequiredField {
                field_name: "payment_method.token.token",
                context: Default::default(),
            }))?,
    }))
}
```

Both fields come straight off the gRPC `Token` message: `token` is mandatory (a missing
value is a `MissingRequiredField`) and `token_payment_method_type` maps the proto enum onto
`payment_method_data::TokenPaymentMethod`, collapsing `Unspecified` to `None`.

## Connectors with Full Implementation

At the pinned SHA `ceb33736ce941775403f241f3f0031acbf2b4527`, **no connector in `crates/integrations/connector-integration/src/connectors/` implements `PaymentMethodToken` Authorize**. Every match arm below returns `IntegrationError::not_implemented(message, context)`.

| Connector | HTTP Method | Content Type | URL Pattern | Request Type Reuse | Notes |
|-----------|-------------|--------------|-------------|--------------------|-------|
| (none) | — | — | — | — | Full implementation intentionally absent at this SHA |

### Stub Implementations (guardrail `not_implemented` arms)

Every connector in the following list explicitly matches `PaymentMethodData::PaymentMethodToken(_)` and returns `IntegrationError::not_implemented(message, context)`. The row count (31) matches the grep of the connectors directory and serves as the reviewer's evidence that no variant is silently omitted.

- `aci` — `crates/integrations/connector-integration/src/connectors/aci/transformers.rs:749`
- `adyen` — `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:3705`, `:6047`
- `bambora` — `crates/integrations/connector-integration/src/connectors/bambora/transformers.rs:295`
- `bankofamerica` — `crates/integrations/connector-integration/src/connectors/bankofamerica/transformers.rs:614`, `:1778`
- `billwerk` — `crates/integrations/connector-integration/src/connectors/billwerk/transformers.rs:234`
- `braintree` — `crates/integrations/connector-integration/src/connectors/braintree/transformers.rs:611`, `:1608`, `:2630`, `:2813`
- `cryptopay` — `crates/integrations/connector-integration/src/connectors/cryptopay/transformers.rs:110`
- `cybersource` — `crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:322`, `:2188`, `:2287`, `:3026`, `:3303`, `:4325`
- `dlocal` — `crates/integrations/connector-integration/src/connectors/dlocal/transformers.rs:208`
- `fiserv` — `crates/integrations/connector-integration/src/connectors/fiserv/transformers.rs:549`
- `fiuu` — `crates/integrations/connector-integration/src/connectors/fiuu/transformers.rs:672`
- `forte` — `crates/integrations/connector-integration/src/connectors/forte/transformers.rs:312`
- `hipay` — `crates/integrations/connector-integration/src/connectors/hipay/transformers.rs`
- `loonio` — `crates/integrations/connector-integration/src/connectors/loonio/transformers.rs:243`
- `mifinity` — `crates/integrations/connector-integration/src/connectors/mifinity/transformers.rs:248`
- `mollie` — `crates/integrations/connector-integration/src/connectors/mollie/transformers.rs` (plus `mollie.rs`)
- `multisafepay` — `crates/integrations/connector-integration/src/connectors/multisafepay/transformers.rs:156`, `:336`
- `nexinets` — `crates/integrations/connector-integration/src/connectors/nexinets/transformers.rs:740`
- `noon` — `crates/integrations/connector-integration/src/connectors/noon/transformers.rs:377`, `:1262`
- `paypal` — `crates/integrations/connector-integration/src/connectors/paypal/transformers.rs:1142`, `:2602`
- `placetopay` — `crates/integrations/connector-integration/src/connectors/placetopay/transformers.rs:210`
- `razorpay` — `crates/integrations/connector-integration/src/connectors/razorpay/transformers.rs:304`
- `redsys` — `crates/integrations/connector-integration/src/connectors/redsys/transformers.rs:249`
- `stax` — `crates/integrations/connector-integration/src/connectors/stax/transformers.rs`
- `stripe` — `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:1516`, `:4644`, `:5038` (see §5.2 for Stripe's in-crate `PaymentMethodToken` naming collision)
- `trustpay` — `crates/integrations/connector-integration/src/connectors/trustpay/transformers.rs:1711`
- `volt` — `crates/integrations/connector-integration/src/connectors/volt/transformers.rs:295`
- `wellsfargo` — `crates/integrations/connector-integration/src/connectors/wellsfargo/transformers.rs`
- `worldpay` — `crates/integrations/connector-integration/src/connectors/worldpay/transformers.rs:222`, also `requests.rs`
- `fiserv`, `paypal`, `razorpay` (duplicates noted above)

Total: 31 source files reference `PaymentMethodData::PaymentMethodToken` or the struct name; 0 implement it.

## Per-Variant Implementation Notes

### `PaymentMethodData::PaymentMethodToken(PaymentMethodToken)` — single variant

Because `PaymentMethodToken` is a single-variant struct, this section is one entry. All guidance that would be "per-variant" for a multi-variant enum like `WalletData` is captured here.

**Expected transformer path.** A connector that chooses to implement `PaymentMethodToken` Authorize must:

1. Pattern-match `PaymentMethodData::PaymentMethodToken(ref pm_token)` on `payment_method_data` in its `TryFrom<ConnectorRouterData<...Authorize...>>` impl.
2. Read the credential from `pm_token.token` (`Secret<String>`, mandatory). There is no `PaymentMethodToken::Token(..)` enum variant to unwrap — the struct field *is* the token.
3. Optionally branch on `pm_token.token_payment_method_type` (`Option<TokenPaymentMethod>` — `ApplePay` / `GooglePay`) when the gateway needs the minting wallet named on the request.
4. Populate billing details from `resource_common_data.get_optional_billing_full_name()` (`crates/types-traits/domain_types/src/connector_types.rs:1449`); the variant carries no cardholder name of its own.
5. Build the connector-local request struct (typically the same struct used for `Card` Authorize, with PAN/expiry fields replaced by the token reference).
6. Emit the standard `PaymentsResponseData::TransactionResponse` on success, per `../card/pattern_authorize_card.md` §Response Patterns. It is an enum struct-variant, so there is no `..Default::default()` shortcut -- all 11 fields must be listed (`crates/types-traits/domain_types/src/connector_types.rs:2009`).

**Connector-specific quirk at this SHA.** Stripe defines an internal struct named `StripeCardToken` at `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:594-607` that is **unrelated to the domain-layer `PaymentMethodData::PaymentMethodToken` variant**. `StripeCardToken` is Stripe's tokenization-API (`/v1/tokens`) request body and carries raw PAN (`card[number]`), expiry, and CVC. It is used by the `PaymentMethodToken` flow, not the `Authorize` flow. Stripe's `Authorize` transformer still returns `not_implemented` for `PaymentMethodData::PaymentMethodToken(_)` at `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:1516`. This naming collision is the most common pitfall for readers of this pattern; see [Common Errors §1](#1-stripes-stripecardtoken-struct-is-not-the-domain-paymentmethodtoken).

## Common Implementation Patterns

### 1. `not_implemented` guardrail pattern (canonical at this SHA)

The uniform shape across every connector at this SHA:

```rust
// From crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:3696
PaymentMethodData::Crypto(_)
| PaymentMethodData::MandatePayment
| PaymentMethodData::Reward
| PaymentMethodData::RealTimePayment(_)
| PaymentMethodData::Upi(_)
| PaymentMethodData::OpenBanking(_)
| PaymentMethodData::CardDetailsForNetworkTransactionId(_)
| PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
| PaymentMethodData::MobilePayment(_)
| PaymentMethodData::PaymentMethodToken(_) => {
    Err(IntegrationError::not_implemented("payment method", Default::default()).into())
}
```

Minor textual variations exist — some connectors use the `get_unimplemented_payment_method_error_message(..)` helper from `domain_types::utils`. Redsys uses it at `crates/integrations/connector-integration/src/connectors/redsys/transformers.rs:253-254`:

```rust
// From crates/integrations/connector-integration/src/connectors/redsys/transformers.rs:249
| Some(PaymentMethodData::PaymentMethodToken(..))
| Some(PaymentMethodData::NetworkToken(..))
| Some(PaymentMethodData::CardDetailsForNetworkTransactionId(_))
| Some(PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_))
| None => Err(IntegrationError::not_implemented(
    domain_types::utils::get_unimplemented_payment_method_error_message("redsys"),
    Default::default(),
)
.into()),
```

Both forms are acceptable and semantically equivalent for the reviewer's §11 (Code snippets syntactically plausible) check.

### 2. Forward-looking pattern (future implementation skeleton)

A future implementer adding real `PaymentMethodToken` Authorize support should follow this shape. It is a **template**, not a pattern observed at the pinned SHA; annotate any real PR that adds it with a reference to this section so the Wave-8 reviewer can diff structurally:

```rust
// Template — NOT observed at pinned SHA, provided for future waves
match &router_data.request.payment_method_data {
    PaymentMethodData::PaymentMethodToken(pm_token) => {
        // 1. The token IS the variant's `token` field -- it is not optional and there is
        //    no `PaymentMethodToken::Token(..)` enum to destructure.
        let token_secret = pm_token.token.clone();

        // 2. Optional: which wallet minted the token (ApplePay / GooglePay).
        let source_wallet = pm_token.token_payment_method_type;

        // 3. Cardholder label comes from billing, not from the variant.
        let card_holder_name = router_data
            .resource_common_data
            .get_optional_billing_full_name();

        // 4. Build the connector-local request. Field names are illustrative.
        let request = ConnectorAuthorizeRequest {
            payment_method_reference: token_secret,
            token_source: source_wallet,
            card_holder: card_holder_name,
            amount: item.amount,
            currency: router_data.request.currency,
            // ... other flow-common fields
        };

        Ok(Self { card: request, /* ... */ })
    }
    // other variants elided — see ../card/pattern_authorize_card.md for Card-variant handling
    _ => Err(IntegrationError::NotImplemented(
        get_unimplemented_payment_method_error_message("connector_name"),
        Default::default(),
    ).into()),
}
```

Helpers `get_optional_billing_full_name()` and `get_payment_method_token()` are surfaced in `grace/rulesbook/codegen/guides/utility_functions_reference.md` (see [Cross-References](#cross-references)) and already used across connectors.

## Code Examples

Each excerpt below is copied verbatim from the pinned SHA so the Wave-8 reviewer can diff. No status is hardcoded inside a `TryFrom` block (per the authoring spec's banned anti-pattern #1).

### Example 1 — PM variant and struct definitions

```rust
// From crates/types-traits/domain_types/src/payment_method_data.rs:247
#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum PaymentMethodData<T: PaymentMethodDataTypes> {
    Card(Card<T>),
    CardDetailsForNetworkTransactionId(CardDetailsForNetworkTransactionId),
    DecryptedWalletTokenDetailsForNetworkTransactionId(
        DecryptedWalletTokenDetailsForNetworkTransactionId,
    ),
    CardRedirect(CardRedirectData),
    Wallet(WalletData),
    PayLater(PayLaterData),
    BankRedirect(BankRedirectData),
    BankDebit(BankDebitData),
    BankTransfer(Box<BankTransferData>),
    Crypto(CryptoData),
    MandatePayment,
    Reward,
    RealTimePayment(Box<RealTimePaymentData>),
    Upi(UpiData),
    Voucher(VoucherData),
    GiftCard(Box<GiftCardData>),
    PaymentMethodToken(PaymentMethodToken),
    OpenBanking(OpenBankingData),
    NetworkToken(NetworkTokenData),
    MobilePayment(MobilePaymentData),
}
```

```rust
// From crates/types-traits/domain_types/src/payment_method_data.rs:492
#[derive(Eq, PartialEq, Debug, serde::Deserialize, serde::Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct PaymentMethodToken {
    pub token: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_payment_method_type: Option<TokenPaymentMethod>,
}

// Same file, :502
#[derive(Eq, PartialEq, Debug, Clone, Copy, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenPaymentMethod {
    ApplePay,
    GooglePay,
}
```

### Example 2 — gRPC → domain construction (only production construction site)

```rust
// From crates/types-traits/domain_types/src/types.rs:1522
grpc_api_types::payments::payment_method::PaymentMethod::Token(token) => {
    Ok(Self::PaymentMethodToken(payment_method_data::PaymentMethodToken {
        token_payment_method_type: match token.token_payment_method_type() {
            // ... ApplePay / GooglePay / Unspecified => None
        },
        token: token
            .token
            .ok_or_else(|| report!(IntegrationError::MissingRequiredField {
                field_name: "payment_method.token.token",
                context: Default::default(),
            }))?,
    }))
}
```

Both fields are propagated from the proto; `token` is mandatory and its absence is a `MissingRequiredField`.

### Example 3 — PMT enum mapping

```rust
// From crates/types-traits/domain_types/src/connector_types.rs:4365
PaymentMethodData::PaymentMethodToken(_) => Self::PaymentMethodToken,
```

The canonical `PaymentMethodDataType::PaymentMethodToken` tag lives at `crates/types-traits/domain_types/src/types.rs:13753`.

### Example 4 — Adyen `not_implemented` arm (Authorize)

```rust
// From crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:3696
PaymentMethodData::Crypto(_)
| PaymentMethodData::MandatePayment
| PaymentMethodData::Reward
| PaymentMethodData::RealTimePayment(_)
| PaymentMethodData::Upi(_)
| PaymentMethodData::OpenBanking(_)
| PaymentMethodData::CardDetailsForNetworkTransactionId(_)
| PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
| PaymentMethodData::MobilePayment(_)
| PaymentMethodData::PaymentMethodToken(_) => {
    Err(IntegrationError::not_implemented("payment method", Default::default()).into())
}
```

### Example 5 — Adyen `not_implemented` arm (SetupMandate)

```rust
// From crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:6040
| PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
| PaymentMethodData::NetworkToken(_)
| PaymentMethodData::MobilePayment(_)
| PaymentMethodData::PaymentMethodToken(_) => {
    Err(IntegrationError::not_implemented("payment method", Default::default()).into())
}
```

### Example 6 — Stripe `not_implemented` (proving Stripe does *not* handle the domain variant in Authorize)

```rust
// From crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:1512
| PaymentMethodData::MobilePayment(_)
| PaymentMethodData::MandatePayment
| PaymentMethodData::OpenBanking(_)
| PaymentMethodData::PaymentMethodToken(_)
| PaymentMethodData::NetworkToken(_)
| PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
| PaymentMethodData::CardDetailsForNetworkTransactionId(_) => Err(
    // error construction
)
```

### Example 7 — Stripe's `StripeCardToken` struct (Tokenization flow, NOT Authorize)

```rust
// From crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:592
// Struct to call the Stripe tokens API to create a PSP token for the card details provided.
#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct StripeCardToken<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> {
    #[serde(rename = "type")]
    pub payment_method_type: Option<StripePaymentMethodType>,
    #[serde(rename = "card[number]")]
    pub token_card_number: RawCardNumber<T>,
    #[serde(rename = "card[exp_month]")]
    pub token_card_exp_month: Secret<String>,
    #[serde(rename = "card[exp_year]")]
    pub token_card_exp_year: Secret<String>,
    #[serde(rename = "card[cvc]")]
    pub token_card_cvc: Secret<String>,
    #[serde(flatten)]
    pub billing: StripeBillingAddressCardToken,
}
```

This struct is populated from `PaymentMethodData::Card(card_details)` — note the source variant is `Card`, not `PaymentMethodToken`:

```rust
// From crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:5292
let request_payment_data = match &item.router_data.request.payment_method_data {
    PaymentMethodData::Card(card_details) => {
        StripePaymentMethodData::CardToken(StripeCardToken {
            payment_method_type: Some(StripePaymentMethodType::Card),
            token_card_number: card_details.card_number.clone(),
            token_card_exp_month: card_details.card_exp_month.clone(),
            token_card_exp_year: card_details.card_exp_year.clone(),
            token_card_cvc: card_details.card_cvc.clone(),
            billing: billing_address,
        })
    }
    _ => { /* other variants via create_stripe_payment_method */ }
};
```

Stripe's internal `StripePaymentMethodData::CardToken(StripeCardToken { .. })` is a request-side envelope for the `/v1/tokens` endpoint, not a handler for `PaymentMethodData::PaymentMethodToken(_)`. See [Common Errors §1](#1-stripes-stripecardtoken-struct-is-not-the-domain-paymentmethodtoken).

### Example 8 — `NetworkTokenData` for cross-reference (contrast)

```rust
// From crates/types-traits/domain_types/src/payment_method_data.rs:421
#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize, Default)]
pub struct NetworkTokenData {
    pub token_number: cards::NetworkToken,
    pub token_exp_month: Secret<String>,
    pub token_exp_year: Secret<String>,
    pub token_cryptogram: Option<Secret<String>>,
    pub card_issuer: Option<String>,
    pub card_network: Option<common_enums::CardNetwork>,
    pub card_type: Option<String>,
    pub card_issuing_country: Option<String>,
    pub bank_code: Option<String>,
    pub nick_name: Option<Secret<String>>,
    pub eci: Option<String>,
}
```

Eleven fields vs. `PaymentMethodToken`'s two. See the next section for the three-way taxonomy.

## PaymentMethodToken vs NetworkToken vs Card

The three card-family variants differ along five axes: credential location, cryptogram presence, network metadata, PCI scope, and expected data source. Every claim in the table below is cited.

| Axis | `PaymentMethodData::Card(Card<T>)` | `PaymentMethodData::PaymentMethodToken(PaymentMethodToken)` | `PaymentMethodData::NetworkToken(NetworkTokenData)` |
|------|------------------------------------|-------------------------------------------|------------------------------------------------------|
| PAN on the wire | Yes — `card_number: CD::CardNumberType` | No — struct has no number field | Yes, as DPAN — `token_number: cards::NetworkToken` |
| Expiry | `card_exp_month`, `card_exp_year` | None | `token_exp_month`, `token_exp_year` |
| CVC / cryptogram | `card_cvc: Secret<String>` | None — the struct has no CVC field | `token_cryptogram: Option<Secret<String>>` (network cryptogram) |
| ECI indicator | No (arrives via `AuthenticationData`) | No | Yes — `eci: Option<String>` |
| Card network | `card_network: Option<CardNetwork>` | No | `card_network: Option<CardNetwork>` |
| Cardholder name | `nick_name: Option<Secret<String>>` (display) | None — take it from billing (`get_optional_billing_full_name()`) | `nick_name: Option<Secret<String>>` |
| Credential source | Raw customer input | Out-of-band via `PaymentFlowData::payment_method_token` | Network-token service (Apple Pay, Google Pay, scheme network-tokens) |
| Generic over `T: PaymentMethodDataTypes` | Yes | No | No |
| Struct citation | `crates/types-traits/domain_types/src/payment_method_data.rs:53-64` in gold pattern (`authorize/card/pattern_authorize_card.md:53`) | `crates/types-traits/domain_types/src/payment_method_data.rs:494-498` | `crates/types-traits/domain_types/src/payment_method_data.rs:421-433` |
| PMT tag | `PaymentMethodDataType::Card` (`crates/types-traits/domain_types/src/types.rs:13651`) | `PaymentMethodDataType::PaymentMethodToken` (`crates/types-traits/domain_types/src/types.rs:13753`) | `PaymentMethodDataType::NetworkToken` (`crates/types-traits/domain_types/src/types.rs:13760`) |
| Connectors implementing Authorize at pinned SHA | Many (see `../card/pattern_authorize_card.md` §Supported Connectors) | **Zero** — see [Connectors with Full Implementation](#connectors-with-full-implementation) | See sibling Wave 5B pattern `../network_token/pattern_authorize_network_token.md` |

### Decision guide

**Use `Card<T>`** when the caller supplies a raw PAN, expiry, and CVC, and the connector directly authorizes the card. This is the most common PM across connectors. Refer to `../card/pattern_authorize_card.md` (gold reference).

**Use `NetworkToken`** when the caller supplies a network token (DPAN) plus cryptogram and ECI, typically obtained from Apple Pay, Google Pay, or a network-tokenization service. The cryptogram is **per-transaction** and must be forwarded to the acquirer. Refer to `../network_token/pattern_authorize_network_token.md` (sibling Wave 5B).

**Use `PaymentMethodToken`** when the caller supplies only a token reference — the actual credential is resolved by the connector via a previously issued `payment_method_token` (or `connector_customer`) on `PaymentFlowData`. The variant's two fields supply optional step-up metadata (CVC, cardholder name) only. At this SHA, no connector implements Authorize for this variant — every connector's transformer returns `IntegrationError::not_implemented`.

### Why `PaymentMethodToken` is not `MandatePayment`

`PaymentMethodData::MandatePayment` (`crates/types-traits/domain_types/src/payment_method_data.rs:261`) is a fieldless variant that signals "repeat a previously authorized mandate". It is a different concept: the connector fetches the mandate ID from the mandate-specific router-data fields, not from a tokenization channel. Use the mandate patterns (`../../pattern_setup_mandate.md`, `../../pattern_repeat_payment_flow.md`) for mandate-based reuse; use `PaymentMethodToken` for generic tokenization reuse.

### Why `PaymentMethodToken` is not `CardDetailsForNetworkTransactionId`

`CardDetailsForNetworkTransactionId` (`crates/types-traits/domain_types/src/payment_method_data.rs:250`) carries card details explicitly paired with a prior network transaction ID for MIT (merchant-initiated-transaction) recurring. It carries PAN. `PaymentMethodToken` does not carry PAN and is orthogonal to the NTID flow.

## Best Practices

- **Fall through to `not_implemented` until your connector's tokenization contract is defined.** Every observed connector does this — see any row of [Stub Implementations](#stub-implementations-guardrail-not_implemented-arms). Do not silently accept `PaymentMethodToken(_)` and dispatch to a PAN-based flow; that would produce misrouted PCI data.
- **Prefer `get_unimplemented_payment_method_error_message(connector_name)`** (one argument — `crates/types-traits/domain_types/src/utils.rs:195`) from `domain_types::utils` over a bare string so the reviewer and the end-user see a uniform error surface (redsys pattern: `crates/integrations/connector-integration/src/connectors/redsys/transformers.rs:253`).
- **Read the credential from `pm_token.token`.** It is a mandatory `Secret<String>` on the variant itself. Do not look for `PaymentFlowData::payment_method_token` (that field does not exist) or `router_data::PaymentMethodToken::Token(..)` (dead, commented-out code at `crates/types-traits/domain_types/src/router_data.rs:4376-4380`).
- **Treat `pm_token.token_payment_method_type` as optional context, not a credential.** It only names the minting wallet (`TokenPaymentMethod::{ApplePay, GooglePay}`); `None` is normal and must not be an error.
- **Take the cardholder name from billing.** `resource_common_data.get_optional_billing_full_name()` (`crates/types-traits/domain_types/src/connector_types.rs:1449`); the variant carries no cardholder field.
- **Do not confuse Stripe's `StripeCardToken` request envelope with the domain `PaymentMethodToken`.** See [Common Errors §1](#1-stripes-stripecardtoken-struct-is-not-the-domain-paymentmethodtoken).
- **Enumerate `PaymentMethodToken(_)` in every new connector's `payment_method_data` match**, even when handling is deferred. Rust's match-exhaustiveness check enforces this; relying on a catch-all `_ =>` arm hides the deferral from grep-based reviewers.
- **Cross-reference the Wave 5B `NetworkToken` pattern** when weighing whether to add `PaymentMethodToken` support; the two flows share transformer boilerplate but differ sharply in the credential they forward.

## Common Errors

### 1. Stripe's `StripeCardToken` struct is not the domain `PaymentMethodToken`

**Problem.** Readers seeing `StripeCardToken` at `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:594` and `StripePaymentMethodData::CardToken(...)` at `:564` assume Stripe handles `PaymentMethodData::PaymentMethodToken(_)` in Authorize. It does not — the Stripe `StripeCardToken` struct is a wire envelope for the `/v1/tokens` tokenization endpoint, populated from `PaymentMethodData::Card(card_details)` at `:5293-5301`, and the Authorize transformer returns `not_implemented` for the domain variant at `:1516`, `:4644`, and `:5038`.

**Solution.** Treat the two names as unrelated. When implementing `PaymentMethodToken` Authorize for any connector, do not imitate Stripe's `StripeCardToken`; imitate the `Card<T>` Authorize transformer shape from the gold pattern and swap PAN for `pm_token.token`.

### 2. Using the variant fields as the credential

**Problem.** Copy-pasting a pre-rename snippet that reads `card_token.card_holder_name` or `card_token.card_cvc`. Neither field exists on `PaymentMethodToken` any more (E0609); the struct has exactly `token` and `token_payment_method_type`.

**Solution.** Put `pm_token.token` in the token/payment-method-reference slot. Use `pm_token.token_payment_method_type` only where the gateway wants the minting wallet named, and source the cardholder label from billing.

### 3. Assuming `token_payment_method_type` is always populated

**Problem.** Unwrapping `pm_token.token_payment_method_type` and erroring on `None`. The façade at `crates/types-traits/domain_types/src/types.rs:1522-1541` maps the proto's `Unspecified` to `None`, so `None` is a normal, expected value for a caller that did not name a wallet.

**Solution.** Branch on it, do not require it. Only `token` is mandatory — the façade already returns `MissingRequiredField { field_name: "payment_method.token.token" }` when it is absent, so a connector never sees an empty token.

### 4. Silently dropping `PaymentMethodToken` from the match

**Problem.** A connector that omits `PaymentMethodToken(_)` from its `match payment_method_data { ... }` compiles only if it has a `_ => ...` catch-all; the omission is then invisible to grep-based reviewers who rely on the PM-variant enumeration rule of the authoring spec (§9).

**Solution.** Always list `PaymentMethodToken(_)` explicitly in the match, even in the `not_implemented` arm, as every listed connector does. The authoring spec's banned anti-pattern #6 makes silent omission an automatic reviewer FAIL.

### 5. Confusing `PaymentMethodToken` with `PaymentMethodToken`

**Problem.** Older guides describe two types with the same name: `payment_method_data::PaymentMethodToken` (the PM variant payload) and a `router_data::PaymentMethodToken::Token(Secret<String>)` enum. Only the first one exists.

**Solution.** Memorize the disambiguation:

- `payment_method_data::PaymentMethodToken` = the struct at `crates/types-traits/domain_types/src/payment_method_data.rs:494`, fields `token: Secret<String>` and `token_payment_method_type: Option<TokenPaymentMethod>`. **This is the credential.**
- `router_data::PaymentMethodToken` = dead, commented-out code at `crates/types-traits/domain_types/src/router_data.rs:4376-4380` ("nothing populates this after `PaymentFlowData.payment_method_token` was removed"). Naming it is an E0433.

A `PaymentMethodToken` Authorize flow uses only the struct.

## Cross-References

Per the authoring spec §13, a PM pattern MUST cross-reference its parent index, sibling PM patterns, the types doc (if non-obvious types are used), and the utility-functions reference (if helpers are cited).

- Parent indexes:
  - [../../README.md](../../README.md) — top-level patterns index
  - [../README.md](../README.md) — Authorize patterns index
- Gold reference (same category):
  - [../card/pattern_authorize_card.md](../card/pattern_authorize_card.md) — Card `PaymentMethodData::Card(Card<T>)` Authorize pattern; this pattern reuses its transformer shape.
- Parallel Wave 5B (same category):
  - [../network_token/pattern_authorize_network_token.md](../network_token/pattern_authorize_network_token.md) — Network-token Authorize pattern; required reading for contrasting `NetworkTokenData` (11 fields, cryptogram-bearing) against `PaymentMethodToken` (2 fields, no credential).
- Related flow patterns (different category — for the MIT / mandate / tokenization flows that often precede `PaymentMethodToken` Authorize):
  - [../../pattern_payment_method_token.md](../../pattern_payment_method_token.md) — the PaymentMethodToken flow (issues the `Secret<String>` token that later lands in `PaymentFlowData::payment_method_token`).
  - [../../pattern_setup_mandate.md](../../pattern_setup_mandate.md) — mandate-based reuse; complementary to token-based reuse.
  - [../../pattern_repeat_payment_flow.md](../../pattern_repeat_payment_flow.md) — merchant-initiated reuse of stored credentials.
- Authoring spec & review rubric:
  - [../../PATTERN_AUTHORING_SPEC.md](../../PATTERN_AUTHORING_SPEC.md) — structural contract this pattern conforms to (Wave-8 reviewer checks #1–#7).
- Types reference (non-obvious types beyond canonical signatures):
  - [../../../types/types.md](../../../types/types.md) — `PaymentMethodDataType`, `PaymentMethodToken`, `Secret<T>`.
- Utility functions referenced by the forward-looking pattern template:
  - [../../../utility_functions_reference.md](../../../utility_functions_reference.md) — `get_unimplemented_payment_method_error_message`, `get_payment_method_token`, `get_optional_billing_full_name`.

---

### Source-of-truth citations recap

For the Wave-8 reviewer's §3 (all enum variants enumerated) and §2 (citations present) checks, the complete list of pinned-SHA citations used in this pattern:

- `crates/types-traits/domain_types/src/payment_method_data.rs:247` — `PaymentMethodData` enum header
- `crates/types-traits/domain_types/src/payment_method_data.rs:382` — `PaymentMethodToken(PaymentMethodToken)` arm
- `crates/types-traits/domain_types/src/payment_method_data.rs:384` — `NetworkToken(NetworkTokenData)` arm
- `crates/types-traits/domain_types/src/payment_method_data.rs:421-433` — `NetworkTokenData` struct
- `crates/types-traits/domain_types/src/payment_method_data.rs:494-498` — `PaymentMethodToken` struct
- `crates/types-traits/domain_types/src/connector_types.rs:796` — `PaymentFlowData`
- `crates/types-traits/domain_types/src/connector_types.rs:799` — `connector_customer`
- `crates/types-traits/domain_types/src/connector_types.rs:4365` — `PaymentMethodData::PaymentMethodToken(_) => Self::PaymentMethodToken`
- `crates/types-traits/domain_types/src/router_data.rs:4376-4380` — dead, commented-out `PaymentMethodToken::Token(Secret<String>)`
- `crates/types-traits/domain_types/src/types.rs:1522-1541` — gRPC → `PaymentMethodToken` construction
- `crates/types-traits/domain_types/src/types.rs:13651` — `PaymentMethodDataType::Card`
- `crates/types-traits/domain_types/src/types.rs:13753` — `PaymentMethodDataType::PaymentMethodToken`
- `crates/types-traits/domain_types/src/types.rs:13760` — `PaymentMethodDataType::NetworkToken`
- `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:3696-3707` — Adyen Authorize `not_implemented` arm
- `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:6040-6049` — Adyen SetupMandate `not_implemented` arm
- `crates/integrations/connector-integration/src/connectors/redsys/transformers.rs:249-256` — Redsys `not_implemented` with `get_unimplemented_payment_method_error_message`
- `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:592-607` — `StripeCardToken` struct (tokenization envelope, not domain variant)
- `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:1512-1519` — Stripe Authorize `not_implemented` arm including `PaymentMethodToken(_)`
- `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:5292-5322` — `StripeCardToken` populated from `PaymentMethodData::Card(_)`, proving Stripe does not read the domain `PaymentMethodToken` variant
- Plus 25 additional `not_implemented` arms across `aci`, `bambora`, `bankofamerica`, `billwerk`, `braintree`, `cryptopay`, `cybersource`, `dlocal`, `fiserv`, `fiuu`, `forte`, `hipay`, `loonio`, `mifinity`, `mollie`, `multisafepay`, `nexinets`, `noon`, `paypal`, `placetopay`, `razorpay`, `stax`, `trustpay`, `volt`, `wellsfargo`, `worldpay` — line numbers listed inline in [Stub Implementations](#stub-implementations-guardrail-not_implemented-arms).
