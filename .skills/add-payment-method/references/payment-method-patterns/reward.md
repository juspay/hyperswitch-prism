# Reward Authorize Pattern Reference

## Payment Method: Reward (ClassicReward, Evoucher)

Cash-based / prepaid payment solutions via redirect. Customer completes payment
at partner locations (ClassicReward) or via digital voucher (Evoucher).
Amount: FloatMajorUnit. Response: async redirect.

## PaymentMethodData Variant

```rust
pub enum PaymentMethodData<T: PaymentMethodDataTypes> {
    Reward,  // No inner data; sub-type determined by PaymentMethodType
}

pub enum PaymentMethodType {
    ClassicReward,  // Physical cash at partner network
    Evoucher,       // Digital voucher redemption
}
```

## Connector Support

| Connector | Status | Sub-types | Auth |
|-----------|--------|-----------|------|
| CashToCode | Full | ClassicReward, Evoucher | CurrencyAuthKey (per-currency) |
| All others | NotImplemented | - | - |

## PaymentMethodData Match Arm

```rust
match item.router_data.resource_common_data.payment_method {
    common_enums::PaymentMethod::Reward => {
        // Extract payment_method_type for sub-type handling
        // Get sub-type-specific merchant credentials
        // Build redirect-based request
    }
    _ => Err(IntegrationError::NotImplemented(..., Default::default())),
}
```

## CashToCode Authentication (Sub-type Specific)

```rust
pub struct CashtocodeAuth {
    pub password_classic: Option<Secret<String>>,
    pub password_evoucher: Option<Secret<String>>,
    pub username_classic: Option<Secret<String>>,
    pub username_evoucher: Option<Secret<String>>,
    pub merchant_id_classic: Option<Secret<String>>,
    pub merchant_id_evoucher: Option<Secret<String>>,
}

// Per-currency auth
pub struct CashtocodeAuthType {
    pub auths: HashMap<common_enums::Currency, CashtocodeAuth>,
}

// Sub-type specific auth headers
let auth_header = match payment_method_type {
    Some(PaymentMethodType::ClassicReward) => construct_basic_auth(
        auth_type.username_classic, auth_type.password_classic),
    Some(PaymentMethodType::Evoucher) => construct_basic_auth(
        auth_type.username_evoucher, auth_type.password_evoucher),
    _ => return Err(IntegrationError::MissingPaymentMethodType)?,
};

// Sub-type specific merchant ID
fn get_mid(connector_config, payment_method_type, currency) -> Result<Secret<String>> {
    match payment_method_type {
        Some(PaymentMethodType::ClassicReward) => Ok(auth.merchant_id_classic...),
        Some(PaymentMethodType::Evoucher) => Ok(auth.merchant_id_evoucher...),
        _ => Err(IntegrationError::FailedToObtainAuthType { context: Default::default() }),
    }
}
```

## Request Structure

```rust
#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CashtocodePaymentsRequest {
    amount: FloatMajorUnit,                     // e.g., 10.50 for $10.50
    transaction_id: String,
    user_id: Secret<id_type::CustomerId>,
    currency: common_enums::Currency,
    first_name: Option<Secret<String>>,
    last_name: Option<Secret<String>>,
    user_alias: Secret<id_type::CustomerId>,
    requested_url: String,                      // Success/callback URL
    cancel_url: String,
    email: Option<Email>,
    mid: Secret<String>,                        // Sub-type specific merchant ID
}
```

## Response and Status

```rust
pub enum CashtocodePaymentStatus {
    Succeeded,
    #[default] Processing,
}

impl From<CashtocodePaymentStatus> for AttemptStatus {
    fn from(item: CashtocodePaymentStatus) -> Self {
        match item {
            Succeeded => Self::Charged,
            Processing => Self::AuthenticationPending,
        }
    }
}

// Response returns a pay_url for redirect
pub struct CashtocodePaymentsResponseData {
    pub pay_url: url::Url,
}
```

## Sub-type Redirect Form Differences

```rust
fn get_redirect_form_data(payment_method_type, response_data) -> Result<RedirectForm> {
    match payment_method_type {
        PaymentMethodType::ClassicReward => Ok(RedirectForm::Form {
            endpoint: response_data.pay_url.to_string(),
            method: Method::Post,
            form_fields: Default::default(),  // Query params in URL
        }),
        PaymentMethodType::Evoucher => Ok(RedirectForm::from((
            response_data.pay_url,
            Method::Get,  // Query params as form fields
        ))),
        _ => Err(IntegrationError::NotImplemented(..., Default::default())),
    }
}
```

## ClassicReward vs Evoucher Summary

| Aspect | ClassicReward | Evoucher |
|--------|---------------|----------|
| Auth credentials | `username/password_classic` | `username/password_evoucher` |
| Merchant ID | `merchant_id_classic` | `merchant_id_evoucher` |
| Redirect method | POST (empty form fields) | GET (query params as form fields) |
| Payment flow | Cash at partner locations | Digital voucher |

## Key Implementation Notes

- `PaymentMethodData::Reward` has no inner data; differentiate by `payment_method_type`
- Amount is FloatMajorUnit (e.g., 10.50), not minor units
- Credentials are per-currency AND per-sub-type
- Always validate `payment_method_type` is present
- Redirect-based; implement webhook handling for payment confirmation
- For macro usage, see `macro-reference.md`
