# Mobile Payment Authorize Pattern Reference

## Payment Method: Mobile Payment (Direct Carrier Billing)

Charges to customer's mobile phone bill or prepaid balance.
Async flow requiring carrier confirmation. High fraud risk; typically for micro-transactions.

## MobilePaymentData Structure

```rust
// crates/types-traits/domain_types/src/payment_method_data.rs
pub enum MobilePaymentData {
    DirectCarrierBilling {
        msisdn: String,                // Phone number (E.164 format: +1234567890)
        client_uid: Option<String>,    // Optional client identifier
    },
}
```

## Connector Support

Most connectors return `NotImplemented`. No primary reference implementation exists yet.

## PaymentMethodData Match Arm

```rust
// For unsupported connectors (most common):
PaymentMethodData::MobilePayment(_) => {
    Err(ConnectorError::NotImplemented(
        "Direct Carrier Billing is not supported by ConnectorName".to_string()
    ))?
}

// For supported connectors:
PaymentMethodData::MobilePayment(ref mobile_data) => {
    match mobile_data {
        MobilePaymentData::DirectCarrierBilling { msisdn, client_uid } => {
            validate_msisdn(msisdn)?;
            Ok(Self {
                amount: item.amount,
                currency: router_data.request.currency.to_string(),
                msisdn: msisdn.clone(),
                client_uid: client_uid.clone(),
                callback_url: router_data.request.router_return_url.clone()
                    .ok_or(ConnectorError::MissingRequiredField { field_name: "router_return_url" })?,
                reference: router_data.resource_common_data.connector_request_reference_id.clone(),
                ...
            })
        }
    }
}
```

## Request Structure

```rust
#[derive(Debug, Serialize)]
pub struct MobilePaymentAuthorizeRequest {
    pub amount: StringMinorUnit,          // or MinorUnit
    pub currency: String,
    pub reference: String,
    pub msisdn: String,                   // E.164 phone number
    pub client_uid: Option<String>,
    pub callback_url: String,             // Required for async notifications
    pub description: Option<String>,
    pub merchant_id: String,
}
```

## MSISDN Validation

```rust
fn validate_msisdn(msisdn: &str) -> Result<(), ConnectorError> {
    if !msisdn.starts_with('+') || !msisdn[1..].chars().all(|c| c.is_ascii_digit()) {
        return Err(ConnectorError::InvalidRequestData {
            message: format!("Invalid MSISDN format: {}", msisdn),
        });
    }
    if msisdn.len() < 8 || msisdn.len() > 16 {
        return Err(ConnectorError::InvalidRequestData {
            message: "MSISDN length must be 8-16 characters including +".to_string(),
        });
    }
    Ok(())
}
```

## Status Mapping

```rust
impl From<ConnectorMobilePaymentStatus> for AttemptStatus {
    fn from(status: ConnectorMobilePaymentStatus) -> Self {
        match status {
            Pending => Self::Pending,
            Approved => Self::Authorized,
            Completed => Self::Charged,
            Rejected | Failed => Self::Failure,
        }
    }
}
```

## Response Pattern

```rust
// DCB typically does not require redirect
let payments_response_data = PaymentsResponseData::TransactionResponse {
    resource_id: ResponseId::ConnectorTransactionId(response.transaction_id.clone()),
    redirection_data: None,
    mandate_reference: None,
    connector_metadata: Some(serde_json::json!({
        "phone_number": response.phone_number,
        "carrier": response.carrier_name.clone(),
    })),
    ...
};
```

## Key Implementation Notes

- MSISDN must be E.164 format (+ followed by 7-15 digits)
- DCB has strict per-transaction and monthly amount limits (typically < $50)
- Always async; implement PSync and webhook handling
- High fraud risk; consider PIN verification and velocity checks
- `client_uid` useful for device fingerprinting
- For macro usage, see `macro-reference.md`
