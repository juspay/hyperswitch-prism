# UPI Authorize Pattern Reference

## Payment Method: UPI (Unified Payments Interface)

India's real-time payment system for instant bank-to-bank transfers via mobile.
Currency: INR only. Amount: minor units (paise). Response: async (webhook-based).

## UpiData Variants

```rust
// crates/types-traits/domain_types/src/payment_method_data.rs
pub enum UpiData {
    UpiCollect(UpiCollectData),  // Send collect request to customer's VPA
    UpiIntent(UpiIntentData),    // Redirect to UPI app with pre-filled details
    UpiQr(UpiQrData),           // Generate QR code for payment
}

pub struct UpiCollectData {
    pub vpa_id: Option<Secret<String, UpiVpaMaskingStrategy>>,
    pub upi_source: Option<UpiSource>,
}

pub struct UpiIntentData {
    pub upi_source: Option<UpiSource>,
    pub app_name: Option<String>,
}

pub struct UpiQrData {
    pub upi_source: Option<UpiSource>,
}

pub enum UpiSource {
    UpiCc,       // RuPay credit on UPI
    UpiCl,       // UPI Credit Line
    UpiAccount,  // UPI Bank Account (Savings)
    UpiCcCl,     // Credit Card + Credit Line
    UpiPpi,      // Prepaid Payment Instrument
    UpiVoucher,  // UPI Voucher
}
```

## Connector Support

| Connector | Intent | Collect | QR | Pattern | Auth |
|-----------|--------|---------|-----|---------|------|
| RazorpayV2 | Yes | Yes | Yes | Standard JSON | Basic Auth |
| PhonePe | Yes | Yes | Yes | Encrypted Payload | HMAC |
| Cashfree | Yes | Yes | No | Two-Phase Flow | BodyKey |
| Paytm | Yes | Yes | No | Session Token | AES |
| Stripe | Yes | No | No | Standard JSON | Bearer |

## PaymentMethodData Match Arm

```rust
PaymentMethodData::Upi(ref upi_data) => {
    match upi_data {
        UpiData::UpiCollect(collect_data) => {
            let vpa = collect_data.vpa_id.as_ref()
                .ok_or(IntegrationError::MissingRequiredField { field_name: "vpa_id" , context: Default::default() })?
                .peek().to_string();
            (UpiFlowType::Collect, Some(vpa))
        }
        UpiData::UpiIntent(_) | UpiData::UpiQr(_) => {
            (UpiFlowType::Intent, None)
        }
    }
}
```

## Request Structure

```rust
#[derive(Debug, Serialize)]
pub struct ConnectorUpiDetails {
    pub flow: UpiFlowType,                    // "collect" or "intent"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vpa: Option<Secret<String>>,          // Required for Collect
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry_time: Option<i32>,             // Minutes
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpiFlowType { Collect, Intent }
```

## TryFrom Request Transformation

```rust
impl TryFrom<ConnectorRouterData<&RouterDataV2<...>, &Connector<T>>> for ConnectorAuthorizeRequest<T> {
    fn try_from(item: ...) -> Result<Self, Self::Error> {
        let (upi_flow, vpa) = match &router_data.request.payment_method_data {
            PaymentMethodData::Upi(upi_data) => match upi_data {
                UpiData::UpiCollect(collect_data) => {
                    let vpa_string = collect_data.vpa_id.as_ref()
                        .ok_or(IntegrationError::MissingRequiredField { field_name: "vpa_id" , context: Default::default() })?
                        .peek().to_string();
                    (UpiFlowType::Collect, Some(vpa_string))
                }
                UpiData::UpiIntent(_) | UpiData::UpiQr(_) => (UpiFlowType::Intent, None),
            },
            _ => return Err(IntegrationError::NotSupported { ... , context: Default::default() }),
        };

        Ok(Self {
            amount: item.amount,
            currency: router_data.request.currency.to_string(),
            order_id: router_data.resource_common_data.connector_request_reference_id.clone(),
            method: "upi".to_string(),
            upi: ConnectorUpiDetails { flow: upi_flow, vpa: vpa.map(Secret::new), expiry_time: Some(15) },
            customer_email: router_data.resource_common_data.get_billing_email()...,
            customer_contact: router_data.resource_common_data.get_billing_phone_number()...,
            callback_url: router_data.request.get_webhook_url()?,
        })
    }
}
```

## Response Pattern

```rust
let status = match response.status {
    ConnectorPaymentStatus::Created => AttemptStatus::AuthenticationPending,
    ConnectorPaymentStatus::Authorized => AttemptStatus::Authorized,
    ConnectorPaymentStatus::Captured => AttemptStatus::Charged,
    ConnectorPaymentStatus::Failed => AttemptStatus::Failure,
};

// Intent flow returns a deep link for redirect
let redirection_data = response.link.map(|url| Box::new(RedirectForm::Uri { uri: url }));

PaymentsResponseData::TransactionResponse {
    resource_id: ResponseId::ConnectorTransactionId(response.id),
    redirection_data,
    mandate_reference: None,
    ...
}
```

## Two-Phase Flow (Cashfree)

1. **Create Order** -> returns `payment_session_id`
2. **Authorize** -> uses session ID to initiate UPI payment
Amount unit: FloatMajorUnit (INR as float, e.g., 100.00)

## Encrypted Payload Pattern (PhonePe)

- Base64-encoded JSON payload with HMAC-SHA256 checksum
- Custom signature header
- Amount: MinorUnit

## Key Implementation Notes

- UPI is INR-only; validate currency
- UpiCollect requires `vpa_id`; Intent/QR do not
- Always async; implement PSync and webhook handling
- Intent flow returns deep link URL for redirect
- `upi_source` field enables routing to specific UPI rails (credit, prepaid, etc.)
- For macro usage, see `macro-reference.md`
