# Crypto Authorize Pattern Reference

## Payment Method: Cryptocurrency

Async redirect-based payments using cryptocurrency. Customer redirected to hosted payment page,
sends crypto from wallet, blockchain confirms. Amount: StringMajorUnit for fiat.

## CryptoData Structure

```rust
// crates/types-traits/domain_types/src/payment_method_data.rs
pub struct CryptoData {
    pub pay_currency: Option<String>,  // Crypto code: "BTC", "ETH", "USDC"
    pub network: Option<String>,       // Blockchain: "mainnet", "erc20"
}

impl CryptoData {
    pub fn get_pay_currency(&self) -> Result<String, Error> {
        self.pay_currency.clone().ok_or_else(missing_field_err("crypto_data.pay_currency"))
    }
}
```

## Connector Support

| Connector | Status | Notes |
|-----------|--------|-------|
| Cryptopay | Full support | Primary reference implementation |
| Others | NotImplemented | Return error |

## PaymentMethodData Match Arm

```rust
PaymentMethodData::Crypto(ref crypto_data) => {
    let pay_currency = crypto_data.get_pay_currency()?;
    let network = crypto_data.network.clone();
    // Build request with fiat amount + crypto currency
}
```

## Request Structure

```rust
#[derive(Default, Debug, Serialize)]
pub struct CryptoPaymentsRequest {
    pub price_amount: StringMajorUnit,              // Fiat amount: "100.00"
    pub price_currency: common_enums::Currency,     // Fiat: USD, EUR
    pub pay_currency: String,                       // Crypto: "BTC", "ETH"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,                    // "mainnet", "erc20"
    pub success_redirect_url: Option<String>,
    pub unsuccess_redirect_url: Option<String>,
    pub custom_id: String,                          // Reference ID
}
```

## TryFrom Implementation

```rust
impl TryFrom<CryptoRouterData<...>> for CryptoPaymentsRequest {
    fn try_from(item: ...) -> Result<Self, Self::Error> {
        match item.router_data.request.payment_method_data {
            PaymentMethodData::Crypto(ref crypto_data) => {
                let pay_currency = crypto_data.get_pay_currency()?;
                let amount = converter.convert(minor_amount, currency)?;
                Ok(Self {
                    price_amount: amount,
                    price_currency: item.router_data.request.currency,
                    pay_currency,
                    network: crypto_data.network.to_owned(),
                    success_redirect_url: router_return_url.clone(),
                    unsuccess_redirect_url: router_return_url.clone(),
                    custom_id: connector_request_reference_id.clone(),
                })
            }
            _ => Err(IntegrationError::NotImplemented(..., Default::default())),
        }
    }
}
```

## Response Structure

```rust
pub struct CryptoPaymentResponseData {
    pub id: String,
    pub status: CryptoPaymentStatus,
    pub hosted_page_url: Option<Url>,       // Customer redirect URL
    pub price_amount: Option<StringMajorUnit>,
    pub pay_amount: Option<StringMajorUnit>,
    pub address: Option<Secret<String>>,    // Payment address
    pub network: Option<String>,
}
```

## Status Mapping

```rust
impl From<CryptoPaymentStatus> for AttemptStatus {
    fn from(status: CryptoPaymentStatus) -> Self {
        match status {
            CryptoPaymentStatus::New => Self::AuthenticationPending,
            CryptoPaymentStatus::Completed => Self::Charged,
            CryptoPaymentStatus::Cancelled => Self::Failure,
            CryptoPaymentStatus::Unresolved | CryptoPaymentStatus::Refunded => Self::Unresolved,
        }
    }
}
```

## Response Transformation

```rust
// Redirect form from hosted_page_url
let redirection_data = crypto_response.data.hosted_page_url
    .map(|url| RedirectForm::from((url, Method::Get)));

// For Charged status, extract amount_captured
if status == AttemptStatus::Charged {
    let amount_captured = convert_back(price_amount, currency)?;
    // Set amount_captured and minor_amount_captured on PaymentFlowData
}
```

## Key Implementation Notes

- Crypto is always async redirect; implement PSync and webhook handling
- Amount is StringMajorUnit (fiat side), not minor units
- `pay_currency` is required; `network` is optional
- URL endpoint typically `/api/invoices` or `/payments`
- Webhook events: `TransactionCreated`, `TransactionConfirmed`, `StatusChanged`
- For macro usage, see `macro-reference.md`
