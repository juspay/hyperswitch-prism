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
    Err(IntegrationError::NotImplemented(
        "Direct Carrier Billing is not supported by ConnectorName".to_string(),
        Default::default(),
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
                    .ok_or(IntegrationError::MissingRequiredField { field_name: "router_return_url" , context: Default::default() })?,
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
    pub amount: StringMinorUnit,          // whatever the vendor spec's wire format is:
                                          // MinorUnit | StringMinorUnit | StringMajorUnit
                                          // | FloatMajorUnit | StringTwoDecimalUnit
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
// `IntegrationError` has no `InvalidRequestData` variant -- that one belongs to
// `ApiErrorResponse` and is not reachable from a connector transformer. The
// request-side "this value is malformed" variant is `InvalidDataFormat`, whose
// `field_name` is a `&'static str` and whose `context` carries the operator-facing
// remediation string (see crates/types-traits/domain_types/src/errors.rs).
fn validate_msisdn(msisdn: &str) -> Result<(), error_stack::Report<IntegrationError>> {
    let valid_shape = msisdn.starts_with('+')
        && msisdn[1..].chars().all(|c| c.is_ascii_digit())
        && (8..=16).contains(&msisdn.len());
    if !valid_shape {
        Err(IntegrationError::InvalidDataFormat {
            field_name: "msisdn",
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Pass the MSISDN in E.164 form: '+' followed by 7-15 digits."
                        .to_string(),
                ),
                doc_url: None,
                additional_context: None,
            },
        })?
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
// DCB typically does not require redirect.
// TransactionResponse is an enum struct-variant, so functional-update syntax is
// unavailable: all 11 fields must appear or you get E0063.
let payments_response_data = PaymentsResponseData::TransactionResponse {
    resource_id: ResponseId::ConnectorTransactionId(response.transaction_id.clone()),
    redirection_data: None,
    connector_metadata: Some(serde_json::json!({
        "phone_number": response.phone_number,
        "carrier": response.carrier_name.clone(),
    })),
    mandate_reference: None,
    network_txn_id: None,
    network_txn_link_id: None,
    connector_response_reference_id: Some(response.transaction_id),
    incremental_authorization_allowed: None,
    splits: None,
    status_code: item.http_code,
    payment_account_reference: None,
};
```

## Key Implementation Notes

- MSISDN must be E.164 format (+ followed by 7-15 digits)
- DCB has strict per-transaction and monthly amount limits (typically < $50)
- Always async; implement PSync and webhook handling
- High fraud risk; consider PIN verification and velocity checks
- `client_uid` useful for device fingerprinting
- For macro usage, see `macro-reference.md`
