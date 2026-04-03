# Card Payment Pattern Reference

## Card Data Structure

```rust
// crates/types-traits/domain_types/src/payment_method_data.rs
pub struct Card<T: PaymentMethodDataTypes> {
    pub card_number: RawCardNumber<T>,
    pub card_exp_month: Secret<String>,
    pub card_exp_year: Secret<String>,
    pub card_cvc: Secret<String>,
    pub card_issuer: Option<String>,
    pub card_network: Option<CardNetwork>,
    pub card_type: Option<String>,
    pub card_issuing_country: Option<String>,
    pub bank_code: Option<String>,
    pub nick_name: Option<Secret<String>>,
    pub card_holder_name: Option<Secret<String>>,
    pub co_badged_card_data: Option<CoBadgedCardData>,
}
```

Card variants: `Card<DefaultPCIHolder>` (raw PAN via `RawCardNumber<DefaultPCIHolder>`), `CardToken<DefaultPCIHolder>` (tokenized),
`NetworkToken` (DPAN, Apple/Google Pay), `CardDetailsForNetworkTransactionId` (recurring).

## Match Arm Pattern

```rust
match &router_data.request.payment_method_data {
    PaymentMethodData::Card(card) => {
        let card_details = ConnectorCard {
            card_number: card.card_number.clone(),
            expiration_month: card.card_exp_month.clone(),
            expiration_year: card.card_exp_year.clone(),
            security_code: card.card_cvc.clone(),
            card_holder_name: router_data.request.get_billing_full_name(),
        };
        // ... build request using card_details
    }
    _ => Err(IntegrationError::NotImplemented(
        get_unimplemented_payment_method_error_message("connector_name", Default::default())
    ).into()),
}
```

## Card Helper Methods

```rust
// 2-digit expiry year
let year = card.get_card_expiry_year_2_digit()?.expose();

// 2-digit expiry month
let month = card.get_card_expiry_month_2_digit()?.expose();

// Direct access
let exp_month = card.card_exp_month.peek();
let exp_year = card.card_exp_year.peek();
```

## Expiry Date Formats

| Format | Code | Used By |
|--------|------|---------|
| MM, YYYY (separate fields) | `card.card_exp_month.clone()`, `card.card_exp_year.clone()` | Most connectors |
| YYMM | `format!("{year}{month}")` using 2-digit helpers | Redsys |
| YYYY-MM | `format!("{}-{}", exp_year, exp_month)` | ISO-style connectors |
| MM/YY | `format!("{}/{}", exp_month, &exp_year[len-2..])` | Display-style connectors |

## Card Network Mapping

Connectors typically require mapping `CardNetwork` / `CardIssuer` to their own codes.

```rust
// Numeric code style (e.g., Cybersource)
fn card_issuer_to_string(card_issuer: CardIssuer) -> String {
    match card_issuer {
        CardIssuer::AmericanExpress => "003",
        CardIssuer::Master => "002",
        CardIssuer::Maestro => "042",
        CardIssuer::Visa => "001",
        CardIssuer::Discover => "004",
        CardIssuer::DinersClub => "005",
        CardIssuer::CarteBlanche => "006",
        CardIssuer::JCB => "007",
        CardIssuer::CartesBancaires => "036",
        CardIssuer::UnionPay => "062",
    }.to_string()
}
```

```rust
// Enum/brand style (e.g., Adyen)
impl TryFrom<&domain_utils::CardIssuer> for CardBrand {
    type Error = Error;
    fn try_from(card_issuer: &domain_utils::CardIssuer) -> Result<Self, Self::Error> {
        match card_issuer {
            domain_utils::CardIssuer::AmericanExpress => Ok(Self::Amex),
            domain_utils::CardIssuer::Master => Ok(Self::MC),
            domain_utils::CardIssuer::Visa => Ok(Self::Visa),
            domain_utils::CardIssuer::Maestro => Ok(Self::Maestro),
            domain_utils::CardIssuer::Discover => Ok(Self::Discover),
            domain_utils::CardIssuer::DinersClub => Ok(Self::Diners),
            domain_utils::CardIssuer::JCB => Ok(Self::Jcb),
            domain_utils::CardIssuer::CarteBlanche => Ok(Self::Cartebancaire),
            domain_utils::CardIssuer::CartesBancaires => Ok(Self::Cartebancaire),
            domain_utils::CardIssuer::UnionPay => Ok(Self::Cup),
        }
    }
}
```

Fallback pattern when `card_network` is not provided:

```rust
let card_type = card
    .card_network
    .clone()
    .and_then(get_connector_card_type)
    .unwrap_or_else(|| {
        domain_types::utils::get_card_issuer(&card_number_string)
            .ok()
            .map(card_issuer_to_string)
    });
```

## TryFrom Implementation: JSON Connector

Uses the match arm pattern above within a full TryFrom. Define a connector-specific card
struct, extract fields from `Card<T>`, and compose into the request alongside amount/currency.

```rust
impl<T: PaymentMethodDataTypes>
    TryFrom<ConnectorRouterData<Authorize, PaymentsAuthorizeData<T>>>
    for ConnectorPaymentRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: ConnectorRouterData<Authorize, PaymentsAuthorizeData<T>>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card) => Ok(Self {
                card: ConnectorCard {
                    card_number: card.card_number.clone(),
                    expiration_month: card.card_exp_month.clone(),
                    expiration_year: card.card_exp_year.clone(),
                    security_code: card.card_cvc.clone(),
                    card_holder_name: router_data.request.get_billing_full_name(),
                },
                amount: Amount { value: item.amount, currency: router_data.request.currency },
            }),
            _ => Err(IntegrationError::NotImplemented(
                get_unimplemented_payment_method_error_message("connector_name", Default::default())
            ).into()),
        }
    }
}
```

## TryFrom Implementation: Form-Encoded Connector

```rust
#[derive(Debug, Serialize)]
pub struct FormEncodedCardRequest {
    #[serde(rename = "card[number]")]
    pub card_number: String,
    #[serde(rename = "card[exp_month]")]
    pub exp_month: String,
    #[serde(rename = "card[exp_year]")]
    pub exp_year: String,
    #[serde(rename = "card[cvc]")]
    pub cvc: String,
    pub amount: MinorUnit,
    pub currency: String,
}

// Stripe-style: TryFrom on the Card directly
impl<T: PaymentMethodDataTypes> TryFrom<&Card<T>> for StripeCardData {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(card: &Card<T>) -> Result<Self, Self::Error> {
        Ok(Self {
            number: card.card_number.clone(),
            exp_month: card.card_exp_month.clone(),
            exp_year: card.card_exp_year.clone(),
            cvc: card.card_cvc.clone(),
        })
    }
}
```

## 3DS Handling

### AuthenticationData Structure

```rust
pub struct AuthenticationData {
    pub threeds_server_transaction_id: Option<String>,
    pub message_version: Option<SemanticVersion>,
    pub trans_status: Option<TransactionStatus>,
    pub eci: Option<String>,
    pub cavv: Option<Secret<String>>,
    pub ucaf_collection_indicator: Option<Secret<String>>,
    pub ds_trans_id: Option<String>,
    pub acs_transaction_id: Option<String>,
    pub transaction_id: Option<String>,
    pub exemption_indicator: Option<String>,
    pub network_params: Option<NetworkTransactionReference>,
}

pub enum TransactionStatus {
    Success,                        // Y
    Failure,                        // N
    NotVerified,                    // A
    VerificationNotPerformed,       // U
    ChallengeRequired,              // C
    ChallengeRequiredDecoupledAuthentication, // D
    InformationOnly,                // I
    Rejected,                       // R
}
```

### 3DS Redirect Form Construction

```rust
fn build_threeds_form(
    acs_url: String,
    creq: String,
) -> Result<RedirectForm, Error> {
    let mut form_fields = std::collections::HashMap::new();
    form_fields.insert("creq".to_string(), creq);

    Ok(RedirectForm::Form {
        endpoint: acs_url,
        method: common_utils::request::Method::Post,
        form_fields,
    })
}
```

### 3DS Status Check in Response

```rust
match status {
    AttemptStatus::AuthenticationPending => {
        // Build redirect form for 3DS challenge
        let redirect_form = build_threeds_form(acs_url, creq)?;
        // Set redirection_data: Some(Box::new(redirect_form))
    }
    _ => {
        // No redirect needed
        // Set redirection_data: None
    }
}
```

## CVV Handling for Tokenized Cards

Make CVV optional when supporting tokenized card flows:

```rust
#[derive(Debug, Serialize)]
pub struct CardDetails {
    pub number: String,
    pub exp_month: String,
    pub exp_year: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cvc: Option<String>,  // Optional for tokenized cards
}
```

## Common Field Mappings

| Grace-UCS Field | Typical Connector Field Names |
|-----------------|------------------------------|
| `card.card_number` | `number`, `cardNumber`, `pan`, `card_number` |
| `card.card_exp_month` | `exp_month`, `expirationMonth`, `expiry_month` |
| `card.card_exp_year` | `exp_year`, `expirationYear`, `expiry_year` |
| `card.card_cvc` | `cvc`, `cvv`, `CVV`, `security_code`, `cvv2` |
| `card.card_network` | `brand`, `card_type`, `card_brand`, `network` |
| `billing.full_name` | `card_holder_name`, `holderName`, `name` |
