# Wallet Payment Method Pattern

Wallet payments use `PaymentMethodData::Wallet(ref wallet_data)` and branch on `WalletData` variants.

Four implementation categories:
1. **Token-Based**: ApplePay, GooglePay, SamsungPay, Paze -- encrypted payment tokens, sync response
2. **Redirect**: PayPal, AliPay, WeChat Pay, GoPay, Gcash, KakaoPay, Momo -- async redirect response
3. **SDK-Based**: PaypalSdk, GooglePayThirdPartySdk, ApplePayThirdPartySdk
4. **Specialized**: Mifinity -- requires additional customer data (DOB)

---

## WalletData Variants

| Variant | Data Type | Category |
|---------|-----------|----------|
| `ApplePay` | `ApplePayWalletData` | Token |
| `GooglePay` | `GooglePayWalletData` | Token |
| `SamsungPay` | `Box<SamsungPayWalletData>` | Token |
| `Paze` | `PazeWalletData` | Token |
| `PaypalRedirect` | `PaypalRedirection` | Redirect |
| `PaypalSdk` | `PayPalWalletData` | SDK |
| `AliPayRedirect` | `AliPayRedirection` | Redirect |
| `AliPayQr` | `Box<AliPayQr>` | QR |
| `AliPayHkRedirect` | `AliPayHkRedirection` | Redirect |
| `WeChatPayRedirect` | `Box<WeChatPayRedirection>` | Redirect |
| `WeChatPayQr` | `Box<WeChatPayQr>` | QR |
| `AmazonPayRedirect` | `Box<AmazonPayRedirectData>` | Redirect |
| `GoPayRedirect` | `GoPayRedirection` | Redirect |
| `GcashRedirect` | `GcashRedirection` | Redirect |
| `KakaoPayRedirect` | `KakaoPayRedirection` | Redirect |
| `MomoRedirect` | `MomoRedirection` | Redirect |
| `MbWayRedirect` | `Box<MbWayRedirection>` | Redirect |
| `MobilePayRedirect` | `Box<MobilePayRedirection>` | Redirect |
| `ApplePayRedirect` | `Box<ApplePayRedirectData>` | Redirect |
| `GooglePayRedirect` | `Box<GooglePayRedirectData>` | Redirect |
| `ApplePayThirdPartySdk` | `Box<ApplePayThirdPartySdkData>` | SDK |
| `GooglePayThirdPartySdk` | `Box<GooglePayThirdPartySdkData>` | SDK |
| `DanaRedirect` | `{}` | Redirect |
| `BluecodeRedirect` | `{}` | Redirect |
| `TwintRedirect` | `{}` | Redirect |
| `VippsRedirect` | `{}` | Redirect |
| `TouchNGoRedirect` | `Box<TouchNGoRedirection>` | Redirect |
| `CashappQr` | `Box<CashappQr>` | QR |
| `SwishQr` | `SwishQrData` | QR |
| `Mifinity` | `MifinityData` | Specialized |
| `RevolutPay` | `RevolutPayData` | Token |

---

## Token Extraction Patterns

### Apple Pay

Check for pre-decrypted token from vault first, then fall back to encrypted data:

```rust
WalletData::ApplePay(apple_pay_data) => {
    match item.router_data.resource_common_data.payment_method_token.clone() {
        Some(PaymentMethodToken::ApplePayDecrypt(decrypt_data)) => {
            // Pre-decrypted: use decrypt_data fields
            // decrypt_data.application_primary_account_number
            // decrypt_data.get_four_digit_expiry_year()
        }
        _ => {
            // Encrypted: extract token string
            let token = apple_pay_data
                .payment_data
                .get_encrypted_apple_pay_payment_data_mandatory()?;
        }
    };
}
```

### Google Pay

```rust
WalletData::GooglePay(gpay_data) => {
    let token = gpay_data
        .tokenization_data
        .get_encrypted_google_pay_token()?;
}
```

### Samsung Pay

```rust
WalletData::SamsungPay(samsung_pay_data) => {
    let token_data = &samsung_pay_data.payment_credential.token_data;
    // Encode as base64 fluid data if connector requires it:
    // let fluid_data_value = SamsungPayFluidDataValue {
    //     public_key_hash: /* from JWT header */,
    //     version: token_data.version.clone(),
    //     data: Secret::new(BASE64_ENGINE.encode(token_data.data.peek())),
    // };
    // let encoded = BASE64_ENGINE.encode(serde_json::to_string(&fluid_data_value)?);
}
```

### Paze

Always requires pre-decrypted data from vault:

```rust
WalletData::Paze(_paze_data) => {
    match item.router_data.resource_common_data.payment_method_token.clone() {
        Some(PaymentMethodToken::PazeDecrypt(paze_decrypted)) => {
            // Use paze_decrypted fields
        }
        _ => Err(IntegrationError::MissingRequiredField {
            field_name: "paze_decrypted_data",
        , context: Default::default() })?
    }
}
```

---

## TryFrom Request Implementation

### Token-Based Wallet

```rust
impl TryFrom<&ConnectorRouterData<&PaymentsAuthorizeRouterDataV2<'_, T>>>
    for ConnectorPaymentsRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: &ConnectorRouterData<&PaymentsAuthorizeRouterDataV2<'_, T>>) -> Result<Self, Self::Error> {
        let amount = item.amount;
        let currency = item.router_data.request.currency;

        match &item.router_data.request.payment_method_data {
            PaymentMethodData::Wallet(wallet_data) => match wallet_data {
                WalletData::ApplePay(data) => {
                    let token = data.payment_data
                        .get_encrypted_apple_pay_payment_data_mandatory()?;
                    Ok(Self { amount, currency, payment_type: "applepay", token })
                }
                WalletData::GooglePay(data) => {
                    let token = data.tokenization_data
                        .get_encrypted_google_pay_token()?;
                    Ok(Self { amount, currency, payment_type: "googlepay", token })
                }
                _ => Err(IntegrationError::NotImplemented(
                    "Wallet not supported".to_string(, Default::default())
                ).into())
            },
            _ => Err(IntegrationError::NotImplemented(
                "Payment method not supported".to_string(, Default::default())
            ).into())
        }
    }
}
```

### Redirect Wallet (PayPal example)

```rust
WalletData::PaypalRedirect(_) => {
    let payment_source = Some(PaymentSourceItem::Paypal(
        PaypalRedirectionRequest {
            experience_context: ContextStruct {
                return_url: item.router_data.request.complete_authorize_url.clone(),
                cancel_url: item.router_data.request.complete_authorize_url.clone(),
                user_action: Some(UserAction::PayNow),
                shipping_preference: ShippingPreference::SetProvidedAddress,
            },
            attributes: match item.router_data.request.setup_future_usage {
                Some(FutureUsage::OffSession) => Some(Attributes { /* vault config */ }),
                _ => None,
            },
        }
    ));
    Ok(Self { intent, purchase_units, payment_source })
}
```

### Specialized Wallet (Mifinity)

```rust
WalletData::Mifinity(data) => {
    let client = MifinityClient {
        first_name: item.router_data.resource_common_data.get_billing_first_name()?,
        last_name: item.router_data.resource_common_data.get_billing_last_name()?,
        phone: phone_details.get_number()?,
        dialing_code: phone_details.get_country_code()?,
        nationality: billing_country,
        email_address: item.router_data.resource_common_data.get_billing_email()?,
        dob: data.date_of_birth.clone(),
    };
    // Build request with client, money, address, etc.
}
```

---

## Response Handling

### Sync Response (Token Wallets)

```rust
impl TryFrom<ResponseRouterData<ConnectorWalletResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<...>) -> Result<Self, Self::Error> {
        let status = map_wallet_status(&item.response.status)?;
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.id),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}
```

### Redirect Response (Async Wallets)

Return a redirect URL for the customer to authenticate:

```rust
let link = get_redirect_url(item.response.links)?;
Ok(Self {
    resource_common_data: PaymentFlowData {
        status: AttemptStatus::AuthenticationPending, // or map from connector status
        ..item.router_data.resource_common_data
    },
    response: Ok(PaymentsResponseData::TransactionResponse {
        resource_id: ResponseId::ConnectorTransactionId(item.response.id),
        redirection_data: Some(Box::new(RedirectForm::from((
            link.ok_or(ConnectorError::ResponseDeserializationFailed { context: Default::default() })?,
            Method::Get,
        )))),
        mandate_reference: None,
        connector_metadata: Some(connector_meta),
        network_txn_id: None,
        connector_response_reference_id: Some(item.response.id),
        incremental_authorization_allowed: None,
        status_code: item.http_code,
    }),
    ..item.router_data
})
```

---

## Status Mapping

Token wallets (sync):
- `succeeded` / `completed` -> `AttemptStatus::Charged`
- `pending` -> `AttemptStatus::Pending`
- `failed` -> `AttemptStatus::Failure`
- `requires_action` -> `AttemptStatus::AuthenticationPending`

Redirect wallets (async):
- `PAYER_ACTION_REQUIRED` -> `AttemptStatus::AuthenticationPending`
- `COMPLETED` -> `AttemptStatus::Charged`
- `PENDING` -> `AttemptStatus::Pending`

---

## Wallet Data Structures

```rust
pub struct ApplePayWalletData {
    pub payment_data: ApplePayPaymentData,       // Encrypted or Decrypted enum
    pub payment_method: ApplepayPaymentMethod,    // display_name, network, pm_type
    pub transaction_identifier: String,
}

pub struct GooglePayWalletData {
    pub pm_type: String,                          // e.g. "CARD"
    pub description: String,                      // e.g. "Visa 1234"
    pub info: GooglePayPaymentMethodInfo,         // card_network, card_details
    pub tokenization_data: GpayTokenizationData,  // Encrypted or Decrypted enum
}

pub struct SamsungPayWalletData {
    pub payment_credential: SamsungPayWalletCredentials,
    // credential contains: method, card_brand, card_last_four_digits, token_data
    // token_data contains: three_ds_type, version, data (Secret<String>)
}

pub struct MifinityData {
    pub date_of_birth: Secret<Date>,
    pub language_preference: Option<String>,
}
```

---

## Key Rules

1. Always check `payment_method_token` for pre-decrypted data before using encrypted tokens (ApplePay, Paze).
2. Redirect wallets must set `return_url` and `cancel_url` from `complete_authorize_url`.
3. Redirect wallet responses must return `redirection_data` with the provider URL.
4. Use `AuthenticationPending` status for redirect flows, not `Charged`.
5. Unsupported wallet variants must return `IntegrationError::NotImplemented`.
6. Samsung Pay token data may need base64 fluid data encoding depending on the connector.
