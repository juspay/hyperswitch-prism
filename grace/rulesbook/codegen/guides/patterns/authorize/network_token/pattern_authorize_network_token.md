# Network Token Authorize Flow Pattern

## Overview

`NetworkToken` is the Grace-UCS payment-method arm that carries a **network-tokenized PAN** — a surrogate card number issued by a card-scheme tokenization service (Visa Token Service / VTS, Mastercard Digital Enablement Service / MDES, Amex Token Service / ATS, Discover DTS). Unlike a raw PAN or a connector-specific token, a network token is a 16-19 digit number that looks like a card number, is network-routable (passes BIN-range validation at the acquirer), and is cryptographically bound to a one-time **cryptogram** that attests the transaction is authorized by the token-requestor.

Key characteristics:

| Property | Value | Citation |
|----------|-------|----------|
| Parent enum arm | `PaymentMethodData::NetworkToken(NetworkTokenData)` | `crates/types-traits/domain_types/src/payment_method_data.rs:269` |
| Underlying struct | `NetworkTokenData` (struct, 11 fields) | `crates/types-traits/domain_types/src/payment_method_data.rs:306` |
| Token number type | `cards::NetworkToken` (newtype over `StrongSecret<String, CardNumberStrategy>`) | `crates/types-traits/cards/src/validate.rs:26` |
| Customer flow | Off-session / stored credential, optionally on-session for wallet-decrypted flows (Apple Pay, Google Pay, Paze emit DPAN + cryptogram) |
| PCI scope | **Reduced** — the DPAN is not the funding PAN; the network replaces it at authorization time. Merchants handling only DPANs are still in scope but with less risk exposure than raw-PAN flows. |
| Typical response | Direct authorization success/fail (no redirect); 3DS is usually frictionless because the cryptogram is pre-authenticated. |
| Settlement | Synchronous sync/capture like a standard card auth. |
| Mandatory companion data | `token_cryptogram` + `eci` on every on-session charge for most connectors (Adyen, Cybersource, Trustpay, ACI, Peachpayments); optional for MIT repeat charges (Cybersource, Adyen NetworkTokenWithNTI). |

> **Important distinction — NetworkToken is NOT a wallet-decrypted token.**
>
> When the shopper pays with Apple Pay / Google Pay, the wallet's encrypted blob is decrypted to produce a DPAN + cryptogram. That decrypted DPAN is *also* a network token, but it is carried inside the **wallet** enum arms (`WalletData::ApplePay` → predecrypt, `WalletData::GooglePay` → predecrypt). The `PaymentMethodData::NetworkToken` arm is used when the network token was provisioned **directly** by the merchant / issuer (e.g. via Hyperswitch's own network-token vault, or via VTS/MDES push-provisioning into a merchant's card-on-file) and carried as its own payment method — not wrapped in a wallet envelope.

### Why it exists as its own PM

1. **Different parent enum arm.** The router dispatches `PaymentMethodData::NetworkToken(token_data)` to a dedicated `TryFrom<(&..., &NetworkTokenData)>` impl on each connector; see the dispatch at `crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:2171` and `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:3689`.
2. **Different connector-side payment-method tag.** Connectors carry network-token payloads under a distinct serde tag — Adyen emits `"type": "networkToken"` (`crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:208-209`), Cybersource uses a `PaymentInformation::NetworkToken` untagged variant (`crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:798`), ACI uses `tokenAccount.type = NETWORK` (`crates/integrations/connector-integration/src/connectors/aci/transformers.rs:532-534`).
3. **Mandatory cryptogram/ECI.** Network tokens require a one-time cryptogram and an ECI indicator (EMV 3DS), whereas raw `Card` and `CardToken` do not. ECI is preserved in the struct at `crates/types-traits/domain_types/src/payment_method_data.rs:317` so downstream transformers can copy it into the authorization request.

## Table of Contents

1. [Field Enumeration](#field-enumeration)
2. [NetworkToken vs Card vs CardToken](#networktoken-vs-card-vs-cardtoken)
3. [Architecture Overview](#architecture-overview)
4. [Helper Methods](#helper-methods)
5. [Connectors With Full Implementation](#connectors-with-full-implementation)
6. [Connectors Returning Not-Implemented](#connectors-returning-not-implemented)
7. [Request Construction Patterns](#request-construction-patterns)
8. [Response Patterns](#response-patterns)
9. [MIT / Repeat Payment with Network Token](#mit--repeat-payment-with-network-token)
10. [Common Pitfalls](#common-pitfalls)
11. [Implementation Checklist](#implementation-checklist)
12. [Cross-References](#cross-references)

## Field Enumeration

`NetworkTokenData` is a **struct** (not an enum), so this section replaces the "Variant Enumeration" used by enum-based PM pattern docs (e.g. `CardRedirectData`, `WalletData`). All 11 fields at the pinned SHA, sourced from `crates/types-traits/domain_types/src/payment_method_data.rs:306-318`:

```rust
// crates/types-traits/domain_types/src/payment_method_data.rs:305
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

| # | Field | Type | Required | Purpose | Citation |
|---|-------|------|----------|---------|----------|
| 1 | `token_number` | `cards::NetworkToken` (newtype over `StrongSecret<String, CardNumberStrategy>`) | Yes | The **DPAN** — the network-tokenized 16-19 digit surrogate for the funding PAN. Carried to the connector as the card "number" field on the wire. Validated by `sanitize_card_number` on construction (`crates/types-traits/cards/src/validate.rs:165-174`). | `crates/types-traits/domain_types/src/payment_method_data.rs:307` |
| 2 | `token_exp_month` | `Secret<String>` | Yes | The expiration month of the **token** (MM). Can differ from the funding card's expiry — networks rotate/refresh tokens independently of the underlying card. Forwarded as `expirationMonth` / `expiry_month` / `token_exp_month` depending on connector. | `crates/types-traits/domain_types/src/payment_method_data.rs:308` |
| 3 | `token_exp_year` | `Secret<String>` | Yes | The expiration year of the **token**, in either `YY` or `YYYY` format. Helpers `get_expiry_year_4_digit` (`crates/types-traits/domain_types/src/payment_method_data.rs:325-331`) and `get_token_expiry_year_2_digit` (`crates/types-traits/domain_types/src/payment_method_data.rs:332-346`) normalize it. | `crates/types-traits/domain_types/src/payment_method_data.rs:309` |
| 4 | `token_cryptogram` | `Option<Secret<String>>` | Conditionally required | The one-time **TAVV cryptogram** (Token Authentication Verification Value) generated by the network token service for this transaction. Required for on-session authorization on every full-impl connector (Cybersource `crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:1390`, Adyen flows using `NetworkTokenWithNTI` derive from the NTI ref, Trustpay requires it as `threeDSecureVerificationId` — `crates/integrations/connector-integration/src/connectors/trustpay/transformers.rs:1689`). For pure MIT repeats driven by NTI, it may be absent and the `network_payment_reference` replaces it. | `crates/types-traits/domain_types/src/payment_method_data.rs:310` |
| 5 | `card_issuer` | `Option<String>` | No | Free-form issuing-bank name/identifier for the **funding** card behind the token (e.g. "Chase", "HDFC"). Rarely consumed by connectors today; used primarily for BIN-lookup enrichment and analytics. | `crates/types-traits/domain_types/src/payment_method_data.rs:311` |
| 6 | `card_network` | `Option<common_enums::CardNetwork>` | Recommended | The scheme that issued the token — `Visa`, `Mastercard`, `Amex`, `Discover`, `JCB`, `DinersClub`, `CartesBancaires`, `UnionPay`, `RuPay`, `Interac`, etc. Used to pick the connector-side brand code (ACI `PaymentBrand`, Adyen `CardBrand`, Peachpayments `CardNetworkLowercase` at `crates/integrations/connector-integration/src/connectors/peachpayments/transformers.rs:311-317`). When absent, transformers can fall back to BIN-range detection via `domain_types::utils::get_card_issuer` on the `token_number` (Cybersource does this at `crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:1377-1382`). | `crates/types-traits/domain_types/src/payment_method_data.rs:312` |
| 7 | `card_type` | `Option<String>` | No | Funding instrument type — `"credit"`, `"debit"`, `"prepaid"` — as reported by BIN data. Useful for routing / interchange but not required for auth. | `crates/types-traits/domain_types/src/payment_method_data.rs:313` |
| 8 | `card_issuing_country` | `Option<String>` | No | ISO-3166-1 alpha-2 country code of the issuing bank. Consumed by connectors with region-specific routing rules; most connectors ignore it on NetworkToken. | `crates/types-traits/domain_types/src/payment_method_data.rs:314` |
| 9 | `bank_code` | `Option<String>` | No | Bank identifier code (not IBAN/BIC — scheme-specific identifier for the issuing bank). Informational; not read by any current connector NT impl at the pinned SHA. | `crates/types-traits/domain_types/src/payment_method_data.rs:315` |
| 10 | `nick_name` | `Option<Secret<String>>` | No | Merchant-facing nickname for the saved token (e.g. "My work card"). Not forwarded to connectors. Informational for UX/dashboard. | `crates/types-traits/domain_types/src/payment_method_data.rs:316` |
| 11 | `eci` | `Option<String>` | Conditionally required | **E-Commerce Indicator** — 2-digit string (`"05"`, `"02"`, `"06"`, `"07"` etc.) that tells the acquirer the authentication level obtained during tokenization. Required by Trustpay (`crates/integrations/connector-integration/src/connectors/trustpay/transformers.rs:1682-1687`), Peachpayments (`crates/integrations/connector-integration/src/connectors/peachpayments/transformers.rs:310`). Not forwarded by Adyen's `AdyenNetworkTokenData` (`crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:173-182`) or Cybersource's `NetworkTokenizedCard` (`crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:637-643`) at the pinned SHA — those connectors infer ECI from the cryptogram type server-side. | `crates/types-traits/domain_types/src/payment_method_data.rs:317` |

Note on "Required" column: the "Yes / No / Conditionally" determination is based on what the struct's type system enforces (all `Option<T>` fields are syntactically optional) combined with what connector transformers actually demand at runtime. Fields 4 and 11 are typed `Option<_>` but are frequently `.ok_or(MissingRequiredField)`'d by connectors — see citations above.

## NetworkToken vs Card vs CardToken

These three `PaymentMethodData` arms are often confused because all three represent "a card-like credential." They are fundamentally different and live in separate enum arms of `PaymentMethodData<T>` (`crates/types-traits/domain_types/src/payment_method_data.rs:247-271`):

```rust
// crates/types-traits/domain_types/src/payment_method_data.rs:247-271
pub enum PaymentMethodData<T: PaymentMethodDataTypes> {
    Card(Card<T>),                                              // raw PAN
    CardDetailsForNetworkTransactionId(CardDetailsForNetworkTransactionId),
    // ...
    CardToken(CardToken),                                       // connector-vault reference
    // ...
    NetworkToken(NetworkTokenData),                             // scheme-issued DPAN + cryptogram
    // ...
}
```

### Side-by-side comparison

| Dimension | `Card<T>` | `CardToken` | `NetworkToken` |
|-----------|-----------|-------------|----------------|
| **Enum arm** | `PaymentMethodData::Card(Card<T>)` at line 249 | `PaymentMethodData::CardToken(CardToken)` at line 267 | `PaymentMethodData::NetworkToken(NetworkTokenData)` at line 269 |
| **Struct definition** | `Card<T>` (generic over PCI holder) at `crates/types-traits/domain_types/src/payment_method_data.rs` — raw PAN + CVV + expiry | `CardToken` at `crates/types-traits/domain_types/src/payment_method_data.rs:383-389` — just `card_holder_name` and `card_cvc`; the token itself is referenced elsewhere (mandate_id / payment_method_token field on router data) | `NetworkTokenData` at `crates/types-traits/domain_types/src/payment_method_data.rs:306-318` — DPAN + cryptogram + ECI |
| **Who issues the token?** | N/A — raw PAN, not a token | **Connector / PSP vault** (Stripe `pm_xxx`, Checkout `src_xxx`, Adyen `recurringDetailReference`). The token is **opaque** and only meaningful to the issuing connector. | **Card network** (Visa VTS, Mastercard MDES, Amex ATS). The token is a real BIN-routable PAN surrogate that *any* downstream processor on the network rail can understand. |
| **PAN surfaced on the wire?** | Yes — raw PAN travels to acquirer | No — connector dereferences server-side | Yes — but it's a DPAN, not the funding PAN |
| **Cryptogram required?** | No | No | **Yes** (on-session); sometimes optional for MIT |
| **ECI required?** | Only when 3DS was performed on this PAN | No (connector handles 3DS internally if at all) | **Yes** (almost always — tokenization is the 3DS substitute) |
| **Portability across PSPs?** | Trivial (it's just a PAN) | **Not portable** — connector-specific token ID | **Portable within a network** — Visa token usable via any Visa-connected acquirer |
| **PCI scope for merchant** | Full PCI DSS SAQ-D | Reduced — never handle PAN | Reduced — DPAN is not the funding PAN |
| **Typical lifetime** | Card-expiry bound | Connector-vault lifetime (indefinite until deleted) | Token-expiry bound (independent of funding-card expiry; networks rotate tokens) |
| **Grace-UCS dispatch site example** | `PaymentMethodData::Card(card) => Self::try_from((&item, card))` (Cybersource, Adyen, every connector) | `PaymentMethodData::CardToken(_)` — at the pinned SHA **no connector has a full CardToken impl** in the authorize flow; all return `IntegrationError::not_implemented` (see Stripe `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:1516`, Cybersource `crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:2188`). Sibling pattern being authored in parallel by Wave 5C at `authorize/card_token/pattern_authorize_card_token.md`. | `PaymentMethodData::NetworkToken(token_data) => Self::try_from((&item, token_data))` (Cybersource `crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:2171`, Adyen `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:3689`, ACI `crates/integrations/connector-integration/src/connectors/aci/transformers.rs:719`, Trustpay `crates/integrations/connector-integration/src/connectors/trustpay/transformers.rs:1669`) |
| **Related pattern doc** | `authorize/card/pattern_authorize_card.md` | `authorize/card_token/pattern_authorize_card_token.md` (Wave 5C) | This document |

### Decision tree: which arm am I looking at?

```
Is the credential a raw 16-19 digit PAN + CVV?
├── YES → Card<T>
└── NO
    ├── Is it an opaque, connector-specific reference string (e.g. "pm_1234", "tok_abc")?
    │   └── YES → CardToken
    └── Is it a 16-19 digit network-issued surrogate that looks like a PAN, with an
        accompanying cryptogram + ECI and originates from VTS/MDES/ATS?
        └── YES → NetworkToken
```

### Why the confusion exists

The wallet-decrypted flows (`WalletData::ApplePay` predecrypt, `WalletData::GooglePay` predecrypt) also produce a DPAN + cryptogram (see `crates/integrations/connector-integration/src/connectors/checkout/transformers.rs:172-182` and `:184-195`) — but those DPANs are carried *inside* the wallet arm, not the `NetworkToken` arm. The `NetworkToken` arm is reserved for the merchant-provisioned / card-on-file network-token flow, where tokenization happened **before** and **independently of** any wallet interaction.

See also `CardDetailsForNetworkTransactionId` at `crates/types-traits/domain_types/src/payment_method_data.rs:1439-1450` — that is the **raw-PAN-plus-NTI** variant used exclusively for MIT/repeat flows where the original transaction was not network-tokenized. It is NOT a NetworkToken; it's a Card with an attached network_transaction_id.

## Architecture Overview

### Flow type

`Authorize` — `domain_types::connector_flow::Authorize`.

### Request type

`PaymentsAuthorizeData<T>` — generic over `T: PaymentMethodDataTypes`. The NetworkToken branch is reached when `request.payment_method_data == PaymentMethodData::NetworkToken(NetworkTokenData)`.

### Response type

`PaymentsResponseData::TransactionResponse`. Network-token authorizations are almost always **direct** (no `redirection_data`) because the cryptogram is already a proof of authentication — the issuer rarely challenges a tokenized transaction.

Exception: **Trustpay** carries an `enrollment_status` / `authentication_status` flag and can still step up to 3DS frictionless; see `crates/integrations/connector-integration/src/connectors/trustpay/transformers.rs:1204-1211`.

### Resource common data

`PaymentFlowData`. Billing address is still required by most connectors (Cybersource builds a `BillTo` regardless — `crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:1371-1375`).

### Canonical signature

```rust
RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
```

### Where `NetworkTokenData` is unwrapped

Each connector that supports network tokens implements at least one of these dispatches, all in the `authorize` module `transformers.rs`:

| Connector | Dispatch site | Impl site |
|-----------|---------------|-----------|
| Cybersource | `crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:2171` | `crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:1346-1414` |
| Adyen | `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:3689` | `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1366-1383` |
| ACI | `crates/integrations/connector-integration/src/connectors/aci/transformers.rs:719` | `crates/integrations/connector-integration/src/connectors/aci/transformers.rs:481-528` |
| Trustpay | `crates/integrations/connector-integration/src/connectors/trustpay/transformers.rs:1669` | inline in same match (lines 1669-1697) |
| Peachpayments | `crates/integrations/connector-integration/src/connectors/peachpayments/transformers.rs:290` | inline (lines 290-325) |

## Helper Methods

Implemented on `NetworkTokenData` at `crates/types-traits/domain_types/src/payment_method_data.rs:320-363`:

| Helper | Returns | Purpose | Citation |
|--------|---------|---------|----------|
| `get_card_issuer(&self)` | `Result<CardIssuer, Report<IntegrationError>>` | Detect the scheme by BIN-prefix of the DPAN — `get_card_issuer(self.token_number.peek())`. Needed when `card_network` is `None`. | `crates/types-traits/domain_types/src/payment_method_data.rs:321-323` |
| `get_expiry_year_4_digit(&self)` | `Secret<String>` | Normalizes `YY` or `YYYY` → `YYYY` by prepending `"20"` if length == 2. Used by Adyen (`crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1376`) and ACI (`crates/integrations/connector-integration/src/connectors/aci/transformers.rs:517`). | `crates/types-traits/domain_types/src/payment_method_data.rs:325-331` |
| `get_token_expiry_year_2_digit(&self)` | `Result<Secret<String>, IntegrationError>` | Takes last 2 chars of year. Used by Peachpayments (`crates/integrations/connector-integration/src/connectors/peachpayments/transformers.rs:304`). | `crates/types-traits/domain_types/src/payment_method_data.rs:332-346` |
| `get_network_token(&self)` | `cards::NetworkToken` (clone) | Clone of the DPAN — used as the `number` / `pan` / `token` field on the wire. | `crates/types-traits/domain_types/src/payment_method_data.rs:348-350` |
| `get_network_token_expiry_month(&self)` | `Secret<String>` | Clone of `token_exp_month`. | `crates/types-traits/domain_types/src/payment_method_data.rs:352-354` |
| `get_network_token_expiry_year(&self)` | `Secret<String>` | Clone of raw `token_exp_year` (not normalized — use `get_expiry_year_4_digit` for normalization). | `crates/types-traits/domain_types/src/payment_method_data.rs:356-358` |
| `get_cryptogram(&self)` | `Option<Secret<String>>` | Clone of `token_cryptogram`. | `crates/types-traits/domain_types/src/payment_method_data.rs:360-362` |

On `cards::NetworkToken` itself (`crates/types-traits/cards/src/validate.rs:105-127`):

| Helper | Returns | Purpose |
|--------|---------|---------|
| `get_card_isin()` | `String` | First 6 digits — token BIN. |
| `get_extended_card_bin()` | `String` | First 8 digits. |
| `get_card_no()` | `String` | Full DPAN (raw chars — caller is responsible for secrecy, since the `.peek()` escape is done here). |
| `get_last4()` | `String` | Last 4 digits — for UI display / receipts. |

## Connectors With Full Implementation

At the pinned SHA, the following connectors implement a complete `PaymentMethodData::NetworkToken` branch in the Authorize flow:

| Connector | Wire format | Auth request shape | Citation |
|-----------|------------|--------------------|----------|
| **Cybersource** | JSON, `StringMajorUnit` amount | `PaymentInformation::NetworkToken(Box<NetworkTokenPaymentInformation { tokenized_card: NetworkTokenizedCard { number, expiration_month, expiration_year, cryptogram, transaction_type } }>)` | `crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:637-649`, `:1385-1393`, `:798` |
| **Adyen** | JSON, `MinorUnit` amount | `AdyenPaymentMethod::NetworkToken(Box<AdyenNetworkTokenData { number, expiry_month, expiry_year, holder_name, brand?, network_payment_reference? }>)` with serde tag `"networkToken"` | `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:171-182`, `:208-209`, `:1366-1383` |
| **ACI** | Form-URL-encoded | `PaymentDetails::AciNetworkToken(Box<AciNetworkTokenData { tokenAccount.type=NETWORK, tokenAccount.number, tokenAccount.expiryMonth, tokenAccount.expiryYear, tokenAccount.cryptogram, paymentBrand }>)` | `crates/integrations/connector-integration/src/connectors/aci/transformers.rs:155`, `:481-528`, `:536-551` |
| **Trustpay** | JSON, `StringMajorUnit` amount | `TrustpayPaymentsRequest::NetworkTokenPaymentRequest(Box<PaymentRequestNetworkToken { amount, currency, pan, expiry_date, redirect_url, enrollment_status, eci, authentication_status, verification_id }>)` — note `verification_id` is where Trustpay accepts the cryptogram | `crates/integrations/connector-integration/src/connectors/trustpay/transformers.rs:1195-1212`, `:1222`, `:1669-1697` |
| **Peachpayments** | JSON | `PeachpaymentsTransactionData::NetworkToken(PeachpaymentsNetworkTokenData { merchant_information, routing_reference, network_token_data: { token, expiry_year, expiry_month, cryptogram, eci, scheme }, amount })` | `crates/integrations/connector-integration/src/connectors/peachpayments/transformers.rs:290-325` |

## Connectors Returning Not-Implemented

Every other connector that matches on `PaymentMethodData` has `PaymentMethodData::NetworkToken(_)` in the "unsupported" arm and returns `IntegrationError::not_implemented`. Non-exhaustive list at the pinned SHA:

| Connector | Citation |
|-----------|----------|
| Stripe | `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:1517` |
| Checkout | (no direct match arm — the `NetworkToken` enum name in checkout refers to `CheckoutSourceTypes::NetworkToken` for wallet-decrypted flows, not `PaymentMethodData::NetworkToken`) — `crates/integrations/connector-integration/src/connectors/checkout/transformers.rs:202` |
| Worldpay | `crates/integrations/connector-integration/src/connectors/worldpay/transformers.rs:224` |
| Bank of America | `crates/integrations/connector-integration/src/connectors/bankofamerica/transformers.rs:615` |
| Braintree | `crates/integrations/connector-integration/src/connectors/braintree/transformers.rs:612` |
| Wellsfargo | `crates/integrations/connector-integration/src/connectors/wellsfargo/transformers.rs:578` |
| Fiserv | `crates/integrations/connector-integration/src/connectors/fiserv/transformers.rs:550` |
| PayPal | `crates/integrations/connector-integration/src/connectors/paypal/transformers.rs:1143` |
| Razorpay | `crates/integrations/connector-integration/src/connectors/razorpay/transformers.rs:307` |
| Redsys | `crates/integrations/connector-integration/src/connectors/redsys/transformers.rs:250` |
| Nexinets | `crates/integrations/connector-integration/src/connectors/nexinets/transformers.rs:741` |
| Noon | `crates/integrations/connector-integration/src/connectors/noon/transformers.rs:378` |
| Hipay | `crates/integrations/connector-integration/src/connectors/hipay/transformers.rs:596` |
| Stax | `crates/integrations/connector-integration/src/connectors/stax/transformers.rs:1099` |
| Volt | `crates/integrations/connector-integration/src/connectors/volt/transformers.rs:296` |
| Mifinity | `crates/integrations/connector-integration/src/connectors/mifinity/transformers.rs:249` |
| Billwerk | `crates/integrations/connector-integration/src/connectors/billwerk/transformers.rs:235` |
| Bambora | `crates/integrations/connector-integration/src/connectors/bambora/transformers.rs:296` |
| Fiuu | `crates/integrations/connector-integration/src/connectors/fiuu/transformers.rs:674` |
| Dlocal | `crates/integrations/connector-integration/src/connectors/dlocal/transformers.rs:209` |
| Cryptopay | `crates/integrations/connector-integration/src/connectors/cryptopay/transformers.rs:111` |
| Forte | `crates/integrations/connector-integration/src/connectors/forte/transformers.rs:313` |

## Request Construction Patterns

### Pattern A: Direct Tokenized Card (Cybersource)

Wraps `NetworkTokenizedCard` under `PaymentInformation::NetworkToken`. CVV is absent (tokens never carry CVV). `transaction_type` distinguishes `StoredCredentials` (off-session / MIT) from `InApp` (on-session).

```rust
// crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:1384-1393
let payment_information =
    PaymentInformation::NetworkToken(Box::new(NetworkTokenPaymentInformation {
        tokenized_card: NetworkTokenizedCard {
            number: token_data.get_network_token(),
            expiration_month: token_data.get_network_token_expiry_month(),
            expiration_year: token_data.get_network_token_expiry_year(),
            cryptogram: token_data.get_cryptogram().clone(),
            transaction_type,
        },
    }));
```

See the `NetworkTokenizedCard` struct at `crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:637-643` and the parent enum at `:798`.

### Pattern B: Tagged Payment Method (Adyen)

Serde-tagged via `#[serde(tag = "type")]` with tag value `"networkToken"`. The expiry year is normalized to 4 digits because Adyen's API expects YYYY.

```rust
// crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1366-1383
impl<T: PaymentMethodDataTypes + ...>
    TryFrom<(&NetworkTokenData, Option<Secret<String>>)> for AdyenPaymentMethod<T>
{
    fn try_from(
        (token_data, card_holder_name): (&NetworkTokenData, Option<Secret<String>>),
    ) -> Result<Self, Self::Error> {
        let adyen_network_token = AdyenNetworkTokenData {
            number: token_data.get_network_token(),
            expiry_month: token_data.get_network_token_expiry_month(),
            expiry_year: token_data.get_expiry_year_4_digit(),  // <-- YYYY normalization
            holder_name: card_holder_name,
            brand: None,                     // only for NTI-mandate flows
            network_payment_reference: None, // only for mandate flows
        };
        Ok(Self::NetworkToken(Box::new(adyen_network_token)))
    }
}
```

### Pattern C: Flattened Form Fields (ACI)

ACI uses form-URL-encoded wire format and flattens fields with dotted-path serde renames (`tokenAccount.type`, `tokenAccount.number`, ...). Brand is derived from `card_network`:

```rust
// crates/integrations/connector-integration/src/connectors/aci/transformers.rs:513-526
let aci_network_token_data = AciNetworkTokenData {
    token_type: AciTokenAccountType::Network,
    token_number,
    token_expiry_month: network_token_data.get_network_token_expiry_month(),
    token_expiry_year: network_token_data.get_expiry_year_4_digit(),
    token_cryptogram: Some(
        network_token_data.get_cryptogram().clone().unwrap_or_default(),
    ),
    payment_brand,
};
```

Definition at `crates/integrations/connector-integration/src/connectors/aci/transformers.rs:536-551`.

### Pattern D: ECI + Verification ID Inlined (Trustpay)

Trustpay is the only full-impl connector that *requires* ECI to be forwarded and treats `token_cryptogram` as `threeDSecureVerificationId` (both `.ok_or(MissingRequiredField)`d):

```rust
// crates/integrations/connector-integration/src/connectors/trustpay/transformers.rs:1674-1696
Ok(Self::NetworkTokenPaymentRequest(Box::new(
    PaymentRequestNetworkToken {
        amount,
        currency: item.router_data.request.currency,
        pan: token_data.get_network_token(),
        expiry_date,
        redirect_url: item.router_data.request.get_router_return_url()?,
        enrollment_status: STATUS,
        eci: token_data.eci.clone().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "eci",
                context: Default::default(),
            },
        )?,
        authentication_status: STATUS,
        verification_id: token_data.get_cryptogram().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "verification_id",
                context: Default::default(),
            },
        )?,
    },
)))
```

Struct at `crates/integrations/connector-integration/src/connectors/trustpay/transformers.rs:1195-1212`.

### Pattern E: Scheme-Enum + ECI Forwarded (Peachpayments)

Peachpayments forwards ECI and the card network as a lowercase scheme string, and uses `get_token_expiry_year_2_digit` (the YY helper):

```rust
// crates/integrations/connector-integration/src/connectors/peachpayments/transformers.rs:301-318
network_token_data: requests::PeachpaymentsNetworkTokenDetails {
    token: Secret::new(token_data.token_number.peek().clone()),
    expiry_year: token_data.get_token_expiry_year_2_digit()
        .change_context(IntegrationError::RequestEncodingFailed { context: Default::default() })?,
    expiry_month: token_data.token_exp_month,
    cryptogram: token_data.token_cryptogram,
    eci: token_data.eci,
    scheme: token_data.card_network
        .map(requests::CardNetworkLowercase::try_from)
        .transpose()
        .change_context(IntegrationError::RequestEncodingFailed { context: Default::default() })?,
},
```

## Response Patterns

Network-token authorizations return a standard card-flow response — there is no redirect path in the happy case. All five full-impl connectors reuse the same `PaymentsResponseData::TransactionResponse` handling as the `Card` arm. Since the response does not depend on the request's payment-method data, refer to:

- `authorize/card/pattern_authorize_card.md` → Response Patterns and Status Mapping sections.

Trustpay's 3DS frictionless edge case does populate `redirection_data` when `enrollment_status == 'Y'`; see the success branch of `TrustpayPaymentsResponse` handling in `crates/integrations/connector-integration/src/connectors/trustpay/transformers.rs` (same dispatch as the Card arm).

## MIT / Repeat Payment with Network Token

Network tokens shine in MIT flows — the cryptogram can be dropped when the merchant has a stored Network Transaction Identifier (NTI) from the initial consent transaction. The Grace-UCS type `MandateReferenceId::NetworkTokenWithNTI` carries this combination.

### MandateReferenceId variants

```rust
// crates/types-traits/domain_types/src/connector_types.rs:337-342
#[derive(Eq, PartialEq, Debug, serde::Deserialize, serde::Serialize, Clone)]
pub enum MandateReferenceId {
    ConnectorMandateId(ConnectorMandateReferenceId),   // connector-side vault id
    NetworkMandateId(String),                           // raw PAN + NTI
    NetworkTokenWithNTI(NetworkTokenWithNTIRef),       // DPAN + NTI  <-- NetworkToken MIT
}

// crates/types-traits/domain_types/src/connector_types.rs:330-335
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Eq, PartialEq)]
pub struct NetworkTokenWithNTIRef {
    pub network_transaction_id: String,
    pub token_exp_month: Option<Secret<String>>,
    pub token_exp_year: Option<Secret<String>>,
}
```

### Adyen NetworkTokenWithNTI handler

Adyen's MIT handler populates `brand` (derived from BIN via `get_card_issuer`) and `network_payment_reference` from the NTI ref:

```rust
// crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:6408-6446
MandateReferenceId::NetworkTokenWithNTI(network_mandate_id) => {
    match &item.router_data.request.payment_method_data {
        PaymentMethodData::NetworkToken(ref token_data) => {
            let card_issuer = token_data.get_card_issuer()...?;
            let brand = CardBrand::try_from(&card_issuer)...?;
            let card_holder_name = item.router_data.resource_common_data
                .get_optional_billing_full_name();
            let adyen_network_token = AdyenNetworkTokenData {
                number: token_data.get_network_token(),
                expiry_month: token_data.get_network_token_expiry_month(),
                expiry_year: token_data.get_expiry_year_4_digit(),
                holder_name: test_holder_name.or(card_holder_name),
                brand: Some(brand),  // now present (contrast with one-shot auth)
                network_payment_reference: Some(Secret::new(
                    network_mandate_id.network_transaction_id.clone(),
                )),
            };
            ...
        }
        _ => return Err(IntegrationError::NotSupported { ... })
    }
}
```

### Cybersource RepeatPayment with NetworkToken

Cybersource uses `RepeatPaymentInformation::NetworkToken` for the RepeatPayment flow; the body is identical to the one-shot auth body (the NTI lives in a separate `ProcessingInformation` field):

```rust
// crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:4260-4265
pub enum RepeatPaymentInformation {
    MandatePayment(Box<MandatePaymentInformation>),
    Cards(Box<CardWithNtiPaymentInformation>),
    NetworkToken(Box<NetworkTokenPaymentInformation>),
}
```

Dispatch at `crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:4308`, impl at `:4522-4568`.

## Common Pitfalls

### 1. Forgetting to normalize expiry year

`NetworkTokenData::token_exp_year` can be `"25"` or `"2025"`. If your connector expects 4-digit year, call `get_expiry_year_4_digit` (`crates/types-traits/domain_types/src/payment_method_data.rs:325-331`); for 2-digit call `get_token_expiry_year_2_digit` (`:332-346`). Using `.clone()` of the raw field will leak format mismatch bugs to production.

### 2. Treating `token_cryptogram` as always-present

The field is typed `Option<Secret<String>>` (`crates/types-traits/domain_types/src/payment_method_data.rs:310`) because MIT flows can omit it. For on-session auth, always `.ok_or(MissingRequiredField { field_name: "token_cryptogram" })?` before forwarding, or default-construct an empty secret if the connector tolerates that (ACI does — `crates/integrations/connector-integration/src/connectors/aci/transformers.rs:518-523`). Silently passing `None` through will cause the acquirer to decline with an ECI/cryptogram-missing code.

### 3. Treating NetworkToken as a Wallet

Apple Pay / Google Pay decrypted flows *also* produce DPANs, but they are carried in `WalletData::ApplePay { ApplePayPredecrypt }` / `WalletData::GooglePay { GooglePayPredecrypt }`, not in `PaymentMethodData::NetworkToken`. Check the Checkout structs at `crates/integrations/connector-integration/src/connectors/checkout/transformers.rs:172-195` to see this distinction: `GooglePayPredecrypt` uses `cards::CardNumber` while `DecryptedWalletToken` uses `cards::NetworkToken` — the type-level separation mirrors the PM-level separation.

### 4. Not forwarding ECI when the connector needs it

Adyen and Cybersource do not forward `eci` on `NetworkToken` at the pinned SHA (they infer it). Trustpay (`crates/integrations/connector-integration/src/connectors/trustpay/transformers.rs:1682-1687`) and Peachpayments (`crates/integrations/connector-integration/src/connectors/peachpayments/transformers.rs:310`) **do** require it. When adding a new connector, check the connector spec before deciding whether to forward or infer.

### 5. BIN-sniffing the DPAN for funding-card issuer

`domain_types::utils::get_card_issuer(token_data.token_number.peek())` returns the **token's** BIN, which maps to the scheme (Visa/MC/etc.) but **not** the funding card's issuing bank. `NetworkTokenData::card_issuer` is the authoritative source for issuing-bank name — do not substitute a BIN lookup.

### 6. Confusing with `CardDetailsForNetworkTransactionId`

`CardDetailsForNetworkTransactionId` at `crates/types-traits/domain_types/src/payment_method_data.rs:1439-1450` carries a *raw PAN* plus a network_transaction_id for NTI-based MIT. It is NOT network-tokenized — there is no cryptogram, no ECI. Do not route NetworkToken requests through the CardDetailsForNetworkTransactionId impl or vice versa.

### 7. Using `cards::CardNumber` instead of `cards::NetworkToken` on the wire struct

The type system helps here: `NetworkTokenData::token_number` is `cards::NetworkToken` (`crates/types-traits/cards/src/validate.rs:26`), not `cards::CardNumber` (`:22`). Your connector's request struct should use the same `cards::NetworkToken` type so the serde `Serialize` impl masks the value correctly in logs. Every full-impl does this — e.g. Cybersource `NetworkTokenizedCard.number: cards::NetworkToken` (`crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:638`), Trustpay `PaymentRequestNetworkToken.pan: cards::NetworkToken` (`:1199`), ACI `AciNetworkTokenData.token_number: cards::NetworkToken` (`:542`).

## Implementation Checklist

When adding NetworkToken support to a new connector's Authorize flow:

- [ ] Locate the `match item.router_data.request.payment_method_data` in the connector's `try_from` for the Authorize request.
- [ ] Move `PaymentMethodData::NetworkToken(_)` out of the fallback `not_implemented` arm.
- [ ] Implement a dedicated `TryFrom<(&ConnectorRouterData<...>, &NetworkTokenData)> for <ConnectorRequestBody>` (or inline the construction if the connector keeps everything in one big TryFrom).
- [ ] Decide on expiry year format (2-digit or 4-digit) and use the correct helper.
- [ ] Forward `token_number` via the connector's card-number or dedicated network-token field using type `cards::NetworkToken`.
- [ ] Forward `token_cryptogram` — `.ok_or(MissingRequiredField)?` if the connector requires it; otherwise map `Option<Secret<String>>` directly.
- [ ] Forward `eci` if the connector's spec includes it.
- [ ] Derive `card_network`/brand: prefer `NetworkTokenData::card_network`; fall back to `get_card_issuer(token_number.peek())` when `None`.
- [ ] Set `transaction_type` / `payment_type`: `StoredCredentials` / `Unscheduled` for off-session, `InApp` / `Regular` for on-session — gate on `request.off_session`.
- [ ] If the connector supports MIT, add a `MandateReferenceId::NetworkTokenWithNTI(_)` handler that attaches `network_payment_reference`.
- [ ] Reuse the existing Card-flow response handler — no NT-specific response logic is needed in the happy path.
- [ ] Add negative-path tests: missing cryptogram, missing ECI (if required), unknown card_network.

## Cross-References

### Sibling PM patterns

- **Card** — `authorize/card/pattern_authorize_card.md`. Raw-PAN path. Response patterns and status-mapping are shared with NetworkToken.
- **CardToken** — `authorize/card_token/pattern_authorize_card_token.md` (authored in parallel by Wave 5C; path committed here for forward reference). Connector-vault reference path; not yet implemented by any connector at the pinned SHA.
- **Wallet** — `authorize/wallet/pattern_authorize_wallet.md`. Covers Apple Pay / Google Pay predecrypt flows that *also* produce DPANs but are routed via `WalletData`, not `NetworkToken`.

### Source types

- Struct definition — `crates/types-traits/domain_types/src/payment_method_data.rs:305-318`.
- Helper methods — `crates/types-traits/domain_types/src/payment_method_data.rs:320-363`.
- Parent enum arm — `crates/types-traits/domain_types/src/payment_method_data.rs:269`.
- `cards::NetworkToken` newtype — `crates/types-traits/cards/src/validate.rs:26` and impl at `:105-127`.
- `MandateReferenceId::NetworkTokenWithNTI` — `crates/types-traits/domain_types/src/connector_types.rs:341`.
- `NetworkTokenWithNTIRef` — `crates/types-traits/domain_types/src/connector_types.rs:330-335`.

### Full-impl connectors

- Cybersource — `crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs` (see `:637-649`, `:798`, `:1346-1414`, `:4260-4265`, `:4522-4568`).
- Adyen — `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs` (see `:171-182`, `:208-209`, `:1366-1383`, `:3689-3695`, `:6408-6446`).
- ACI — `crates/integrations/connector-integration/src/connectors/aci/transformers.rs` (see `:155`, `:481-528`, `:536-551`, `:719-721`).
- Trustpay — `crates/integrations/connector-integration/src/connectors/trustpay/transformers.rs` (see `:1195-1212`, `:1222`, `:1669-1697`).
- Peachpayments — `crates/integrations/connector-integration/src/connectors/peachpayments/transformers.rs` (see `:290-325`).

### Spec

Grace-UCS has no connector-agnostic external spec document for NetworkToken at the pinned SHA (confirmed by `ls grace/rulesbook/codegen/references/specs/` — no `network_token*.md` entry). Per-connector network-token behavior is documented in each connector's spec (e.g. `grace/rulesbook/codegen/references/specs/Adyen.md`). This pattern doc is the single source of truth for the Grace-UCS-internal `NetworkTokenData` contract.
