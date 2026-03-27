# Bank Redirect Authorize Pattern Reference

## Payment Method: Bank Redirect

Customer redirected to bank's online banking interface to authenticate and authorize payment.
Async nature; final status via webhook or PSync. No card data handled by merchant.

## BankRedirectData Variants

| Variant | Region | Variant | Region |
|---------|--------|---------|--------|
| `BancontactCard` | Belgium | `OnlineBankingCzechRepublic` | Czech Republic |
| `Bizum` | Spain | `OnlineBankingFinland` | Finland |
| `Blik` | Poland | `OnlineBankingPoland` | Poland |
| `Eft` | South Africa | `OnlineBankingSlovakia` | Slovakia |
| `Eps` | Austria | `OpenBanking` | EU |
| `Giropay` | Germany | `OpenBankingUk` | UK |
| `Ideal` | Netherlands | `Przelewy24` | Poland |
| `Interac` | Canada | `Sofort` | DE/AT/CH |
| `Trustly` | Europe | `OnlineBankingFpx` | Malaysia |
| `OnlineBankingThailand` | Thailand | `LocalBankRedirect` | Various |

## Connector Support

| Connector | Format | Auth | Key Sub-types |
|-----------|--------|------|---------------|
| Adyen | JSON | API Key | eps, giropay, ideal, sofort, trustly |
| Stripe | JSON | API Key | ideal, sofort, bancontact, giropay, p24, eps |
| Trustpay | Dynamic JSON/Form | API Key + OAuth | OpenBanking, OpenBankingUk |
| Mollie | JSON | API Key | ideal, sofort, bancontact, eps, giropay, p24 |
| Volt | JSON | OAuth | OpenBanking, OpenBankingUk |
| Gigadat | FormUrlEncoded | API Key | Interac |

## PaymentMethodData Match Arm

```rust
PaymentMethodData::BankRedirect(ref bank_redirect_data) => {
    match bank_redirect_data {
        BankRedirectData::Ideal { bank_name, .. } => { /* build ideal request */ }
        BankRedirectData::Sofort { .. } => { /* build sofort request */ }
        BankRedirectData::Eps { bank_name, .. } => { /* build eps request */ }
        BankRedirectData::BancontactCard { .. } => { /* build bancontact request */ }
        BankRedirectData::Przelewy24 { bank_name, .. } => { /* build p24 request */ }
        BankRedirectData::OpenBankingUk { .. } => { /* build open banking UK */ }
        BankRedirectData::OpenBanking {} => { /* build open banking EU */ }
        _ => Err(ConnectorError::NotImplemented(...)),
    }
}
```

## Standard JSON Request Pattern (Adyen/Mollie style)

```rust
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum PaymentMethodDetails {
    #[serde(rename = "ideal")]
    Ideal { issuer: String },
    #[serde(rename = "sofort")]
    Sofort { country: String },
    #[serde(rename = "giropay")]
    Giropay { bic: Option<String> },
    #[serde(rename = "eps")]
    Eps { bank: String },
    #[serde(rename = "bancontact")]
    Bancontact,
    #[serde(rename = "p24")]
    Przelewy24 { bank: String },
}
```

## Dynamic Content Type Pattern (Trustpay)

Trustpay uses different content types and URLs based on payment method:
```rust
fn get_dynamic_content_type(&self, req: &RouterDataV2<...>) -> CustomResult<DynamicContentType, ConnectorError> {
    match req.resource_common_data.payment_method {
        PaymentMethod::BankRedirect | PaymentMethod::BankTransfer => Ok(DynamicContentType::Json),
        _ => Ok(DynamicContentType::FormUrlEncoded),
    }
}

// Headers: BankRedirect uses Bearer token from access_token; Cards use API key
// URL: BankRedirect -> "{base}/api/Payments/Payment"; Cards -> "{base}/api/v1/purchase"
```

## Open Banking Pattern (Volt)

```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaymentSystem {
    OpenBankingEu,
    OpenBankingUk,
}

// Route based on variant and currency
match bank_redirect {
    BankRedirectData::OpenBankingUk { .. } => (PaymentSystem::OpenBankingUk, Some(OpenBankingUk { .. }), None),
    BankRedirectData::OpenBanking {} => {
        if matches!(currency, Currency::GBP) {
            (PaymentSystem::OpenBankingUk, Some(OpenBankingUk { .. }), None)
        } else {
            (PaymentSystem::OpenBankingEu, None, Some(OpenBankingEu { .. }))
        }
    }
    _ => Err(ConnectorError::NotImplemented(...)),
}
```

## Redirect Response Pattern

```rust
let status = match response.status {
    BankRedirectStatus::Pending | BankRedirectStatus::Processing => AttemptStatus::Pending,
    BankRedirectStatus::Redirect => AttemptStatus::AuthenticationPending,
    BankRedirectStatus::Completed => AttemptStatus::Charged,
    BankRedirectStatus::Failed => AttemptStatus::Failure,
};

let redirection_data = if status == AttemptStatus::AuthenticationPending {
    Some(RedirectForm::Form {
        endpoint: response.redirect_url.expose(),
        method: Method::Get,
        form_fields: Default::default(),
    })
} else {
    None
};
```

## Detailed Status Mapping (Trustpay-style)

```rust
fn get_attempt_status(item: BankRedirectPaymentStatus) -> AttemptStatus {
    match item {
        Received | Settled => AttemptStatus::Charged,
        Completed | DelayedAtBank | AuthorisedByUser | ApprovedByRisk => AttemptStatus::Pending,
        NewPayment | BankRedirect | AwaitingCheckoutAuthorisation
            | AdditionalAuthorizationRequired => AttemptStatus::AuthenticationPending,
        RefusedByBank | RefusedByRisk | NotReceived | ErrorAtBank
            | CancelledByUser | AbandonedByUser | Failed
            | ProviderCommunicationError => AttemptStatus::Failure,
        Unknown => AttemptStatus::Unspecified,
    }
}
```

## Key Implementation Notes

- Amount unit varies: StringMajorUnit (Adyen, Mollie), MinorUnit (Volt, Stripe)
- Always provide `return_url` for redirect callback
- Bank redirect is always async; implement PSync and webhook handling
- Some connectors require access tokens (Trustpay, Volt) obtained via OAuth
- Variant-specific fields: `bank_name` for Ideal/Eps/P24, `bic` for Giropay, `country` for Sofort
- For macro usage, see `macro-reference.md`
