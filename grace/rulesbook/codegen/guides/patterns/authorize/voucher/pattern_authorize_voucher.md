# Voucher Authorize Flow Patterns

## Overview

Voucher payments are prepaid payment methods where customers receive a payment voucher with a reference number that can be used to complete the payment at physical stores, banks, or online. The customer presents the voucher reference at a payment location to complete the transaction.

### Key Characteristics

- **Customer Experience**: Customer receives a voucher with reference number, then completes payment at a physical location
- **Async Nature**: Voucher payments are asynchronous - final status comes via webhook or PSync
- **Regional Focus**: Popular in Latin America (Boleto, Oxxo) and Asia (Alfamart, Indomaret)
- **No Card Data**: No card numbers or sensitive payment data is handled

### Voucher Variants

| Variant | Description | Common Regions |
|---------|-------------|----------------|
| `Boleto` | Brazilian bank slip payment | Brazil |
| `Oxxo` | Mexican convenience store payment | Mexico |
| `Efecty` | Colombian cash payment | Colombia |
| `PagoEfectivo` | Peruvian cash payment | Peru |
| `RedCompra` | Chilean payment network | Chile |
| `RedPagos` | Uruguayan payment network | Uruguay |
| `Alfamart` | Indonesian convenience store | Indonesia |
| `Indomaret` | Indonesian convenience store | Indonesia |
| `SevenEleven` | Japanese convenience store | Japan |
| `Lawson` | Japanese convenience store | Japan |
| `MiniStop` | Japanese convenience store | Japan |
| `FamilyMart` | Japanese convenience store | Japan |
| `Seicomart` | Japanese convenience store | Japan |
| `PayEasy` | Japanese payment network | Japan |

## Table of Contents

1. [Quick Reference](#quick-reference)
2. [Supported Connectors](#supported-connectors)
3. [Request Patterns](#request-patterns)
4. [Response Patterns](#response-patterns)
5. [Implementation Templates](#implementation-templates)
6. [Sub-type Variations](#sub-type-variations)
7. [Common Pitfalls](#common-pitfalls)
8. [Testing Patterns](#testing-patterns)

## Quick Reference

### Pattern Summary Table

| Pattern | Request Format | Response Type | Amount Unit | Connectors |
|---------|---------------|---------------|-------------|------------|
| Standard JSON | JSON | VoucherNextStepData | StringMajorUnit | Adyen, Zen |
| Float Amount | JSON | VoucherNextStepData | FloatMajorUnit | Dlocal |
| Redirect Required | JSON | Redirect + VoucherNextStepData | StringMajorUnit | Santander |

## Supported Connectors

| Connector | Request Format | Auth Method | Voucher Types Supported | Webhook Support |
|-----------|---------------|-------------|------------------------|-----------------|
| **Adyen** | JSON | API Key | Boleto, Alfamart, Indomaret, Oxxo, Japanese stores | Yes |
| **Santander** | JSON | API Key | Boleto | Yes |
| **Zen** | JSON | API Key | Boleto, Efecty, PagoEfectivo, RedCompra, RedPagos | Yes |
| **Shift4** | JSON | API Key | Boleto | Yes |
| **Dlocal** | JSON | API Key | Oxxo | Yes |

## Request Patterns

### Pattern 1: Standard JSON Request (Adyen-style)

**Applies to**: Adyen, Zen

**Characteristics**:
- Request Format: JSON
- Authentication: API Key in headers
- Amount Unit: StringMajorUnit
- Content-Type: `application/json`

#### Implementation Template

```rust
// REQUIRED IMPORTS for UCS connectors
use common_utils::pii;
use domain_types::{
    connector_types::{PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, VoucherData},
    router_data_v2::RouterDataV2,
};
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

// Voucher payment request structure
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoucherPaymentRequest {
    pub amount: StringMajorUnit,
    pub currency: common_enums::Currency,
    pub reference: String,
    pub payment_method: VoucherPaymentMethod,
    pub return_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shopper_email: Option<pii::Email>,  // CRITICAL: Use pii::Email, not bare Email
}

// Payment method enum for voucher types
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum VoucherPaymentMethod {
    #[serde(rename = "boleto")]
    Boleto { social_security_number: Secret<String> },
    #[serde(rename = "oxxo")]
    Oxxo,
    #[serde(rename = "alfamart")]
    Alfamart(AlfamartData),
    #[serde(rename = "indomaret")]
    Indomaret(IndomaretData),
}

// Alfamart/Indomaret data with billing info
#[derive(Debug, Serialize)]
pub struct AlfamartData {
    pub first_name: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<Secret<String>>,
    pub shopper_email: pii::Email,  // CRITICAL: Use pii::Email
}

#[derive(Debug, Serialize)]
pub struct IndomaretData {
    pub first_name: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<Secret<String>>,
    pub shopper_email: pii::Email,  // CRITICAL: Use pii::Email
}

// Request transformer
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ConnectorRouterData<
            RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
            T,
        >,
    > for VoucherPaymentRequest
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ConnectorRouterData<
            RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount = item
            .connector
            .amount_converter
            .convert(item.router_data.request.minor_amount, item.router_data.request.currency)
            .change_context(ConnectorError::AmountConversionFailed)?;

        let payment_method = match &item.router_data.request.payment_method_data {
            PaymentMethodData::Voucher(voucher_data) => {
                Self::build_voucher_payment_method(voucher_data, &item)?
            }
            _ => Err(ConnectorError::NotImplemented(
                utils::get_unimplemented_payment_method_error_message("connector_name"),
            ))?,
        };

        Ok(Self {
            amount,
            currency: item.router_data.request.currency,
            reference: item.router_data.resource_common_data.connector_request_reference_id.clone(),
            payment_method,
            return_url: item.router_data.request.router_return_url.clone(),
            shopper_email: item.router_data.resource_common_data.get_optional_billing_email(),
        })
    }
}

impl VoucherPaymentRequest {
    fn build_voucher_payment_method<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>(
        voucher_data: &VoucherData,
        item: &ConnectorRouterData<
            RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
            T,
        >,
    ) -> Result<VoucherPaymentMethod, ConnectorError> {
        match voucher_data {
            VoucherData::Boleto(boleto_data) => {
                let ssn = boleto_data.social_security_number.clone().ok_or(
                    ConnectorError::MissingRequiredField {
                        field_name: "social_security_number",
                    },
                )?;
                Ok(VoucherPaymentMethod::Boleto {
                    social_security_number: ssn,
                })
            }
            VoucherData::Oxxo => Ok(VoucherPaymentMethod::Oxxo),
            VoucherData::Alfamart(_) => {
                let billing = item.router_data.resource_common_data.get_billing_address()?;
                Ok(VoucherPaymentMethod::Alfamart(AlfamartData {
                    first_name: billing.get_first_name()?.to_owned(),
                    last_name: billing.get_optional_last_name().map(|s| s.to_owned()),
                    shopper_email: item.router_data.request.get_email()?,
                }))
            }
            VoucherData::Indomaret(_) => {
                let billing = item.router_data.resource_common_data.get_billing_address()?;
                Ok(VoucherPaymentMethod::Indomaret(IndomaretData {
                    first_name: billing.get_first_name()?.to_owned(),
                    last_name: billing.get_optional_last_name().map(|s| s.to_owned()),
                    shopper_email: item.router_data.request.get_email()?,
                }))
            }
            // Unsupported types - return specific error
            VoucherData::Efecty
            | VoucherData::PagoEfectivo
            | VoucherData::RedCompra
            | VoucherData::RedPagos => {
                Err(ConnectorError::NotSupported {
                    message: format!("{:?} voucher type is not supported by this connector", voucher_data),
                    connector: "connector_name",
                })?
            }
            _ => Err(ConnectorError::NotImplemented(
                utils::get_unimplemented_payment_method_error_message("connector_name"),
            ))?,
        }
    }
}
```

### Pattern 2: FloatMajorUnit Amount (Dlocal-style)

**Applies to**: Dlocal

**Characteristics**:
- Request Format: JSON
- Amount Unit: FloatMajorUnit (decimal amount as float)
- Payment method ID is a separate field

```rust
use common_utils::{pii, FloatMajorUnit};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DlocalVoucherRequest {
    pub amount: FloatMajorUnit,
    pub currency: common_enums::Currency,
    pub country: common_enums::CountryAlpha2,
    pub payment_method_id: DlocalPaymentMethodId,
    pub payment_method_flow: DlocalPaymentMethodFlow,
    pub payer: DlocalPayer,
    pub order_id: String,
    pub callback_url: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum DlocalPaymentMethodId {
    Oxxo,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum DlocalPaymentMethodFlow {
    Direct,
}

#[derive(Debug, Serialize)]
pub struct DlocalPayer {
    pub name: Secret<String>,
    pub email: pii::Email,  // CRITICAL: Use pii::Email
    pub document: Secret<String>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<DlocalRouterData<RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>, T>>
    for DlocalVoucherRequest
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: DlocalRouterData<RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>, T>,
    ) -> Result<Self, Self::Error> {
        let email = item.router_data.request.get_email()?;
        let billing = item.router_data.resource_common_data.get_billing_address()?;
        let country = *billing.get_country()?;

        match &item.router_data.request.payment_method_data {
            PaymentMethodData::Voucher(voucher_data) => match voucher_data {
                VoucherData::Oxxo => {
                    let amount = utils::convert_amount(
                        item.connector.amount_converter,
                        item.router_data.request.minor_amount,
                        item.router_data.request.currency,
                    )?;

                    Ok(Self {
                        amount,
                        currency: item.router_data.request.currency,
                        country,
                        payment_method_id: DlocalPaymentMethodId::Oxxo,
                        payment_method_flow: DlocalPaymentMethodFlow::Direct,
                        payer: DlocalPayer {
                            name: billing.get_full_name()?.to_owned(),
                            email,
                            document: Secret::new("".to_string()), // May be required for some regions
                        },
                        order_id: item.router_data.resource_common_data.connector_request_reference_id.clone(),
                        callback_url: Some(item.router_data.request.get_webhook_url()?),
                    })
                }
                _ => Err(ConnectorError::NotImplemented(
                    utils::get_unimplemented_payment_method_error_message("dlocal"),
                ))?,
            },
            _ => Err(ConnectorError::NotImplemented(
                utils::get_unimplemented_payment_method_error_message("dlocal"),
            ))?,
        }
    }
}
```

### Pattern 3: Japanese Convenience Stores (JCS)

**Applies to**: Adyen (SevenEleven, Lawson, MiniStop, FamilyMart, Seicomart, PayEasy)

```rust
// JCS voucher data structure
#[derive(Debug, Serialize)]
pub struct JcsVoucherData {
    pub first_name: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<Secret<String>>,
    pub shopper_email: pii::Email,  // CRITICAL: Use pii::Email
    pub telephone_number: Secret<String>,
}

impl TryFrom<&RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>>
    for JcsVoucherData
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    ) -> Result<Self, Self::Error> {
        let billing = item.resource_common_data.get_billing_address()?;

        Ok(Self {
            first_name: billing.get_first_name()?.to_owned(),
            last_name: billing.get_optional_last_name().map(|s| s.to_owned()),
            shopper_email: item.request.get_email()?,
            telephone_number: billing.get_phone_number()?.to_owned(),
        })
    }
}
```

## Response Patterns

### Pattern 1: VoucherNextStepData Response

**CRITICAL**: Voucher payments MUST return `PaymentsResponseData::VoucherNextStepData`

```rust
use domain_types::connector_types::{PaymentsResponseData, VoucherNextStepData};

// Voucher response structure
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoucherPaymentResponse {
    pub reference: String,  // REQUIRED - cannot be empty
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions_url: Option<String>,
    // Boleto-specific fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digitable_line: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub barcode: Option<String>,
    // Pix-enabled Boleto fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qr_code_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_qr_data: Option<String>,
}

impl TryFrom<ResponseRouterData<VoucherPaymentResponse, RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<VoucherPaymentResponse, RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        // Build VoucherNextStepData
        let voucher_data = VoucherNextStepData {
            reference: response.reference,  // REQUIRED
            expires_at: response.expires_at,
            expiry_date: response.expiry_date.and_then(|d| {
                PrimitiveDateTime::parse(&d, &format_description!("[year]-[month]-[day]")).ok()
            }),
            download_url: response.download_url.and_then(|u| Url::parse(&u).ok()),
            instructions_url: response.instructions_url.and_then(|u| Url::parse(&u).ok()),
            digitable_line: response.digitable_line.map(Secret::new),
            barcode: response.barcode.map(Secret::new),
            qr_code_url: response.qr_code_url.and_then(|u| Url::parse(&u).ok()),
            raw_qr_data: response.raw_qr_data,
            entry_date: None,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::AuthenticationPending,  // Vouchers require customer action
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::VoucherNextStepData(Box::new(voucher_data))),
            ..item.router_data
        })
    }
}
```

### Pattern 2: Status Mapping for Vouchers

```rust
// Voucher payment statuses
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VoucherStatus {
    Pending,
    Processing,
    Authorized,
    Charged,
    Failed,
    Expired,
    Cancelled,
}

fn map_voucher_status_to_attempt_status(status: &VoucherStatus) -> AttemptStatus {
    match status {
        VoucherStatus::Pending | VoucherStatus::Processing => AttemptStatus::Pending,
        VoucherStatus::Authorized => AttemptStatus::Authorized,
        VoucherStatus::Charged => AttemptStatus::Charged,
        VoucherStatus::Failed => AttemptStatus::Failure,
        VoucherStatus::Expired | VoucherStatus::Cancelled => AttemptStatus::Voided,
    }
}
```

## Implementation Templates

### Macro-Based Implementation

```rust
// In connector.rs - Add to create_all_prerequisites! macro
macros::create_all_prerequisites!(
    connector_name: YourConnector,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: VoucherPaymentRequest,
            response_body: VoucherPaymentResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
    ],
    amount_converters: [
        amount_converter: StringMajorUnit
    ],
    member_functions: {
        // ... existing functions
    }
);

// Implement the authorize flow
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: YourConnector,
    curl_request: Json(VoucherPaymentRequest),
    curl_response: VoucherPaymentResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            Ok(format!(
                "{}/payments",
                self.base_url(&req.resource_common_data.connectors)
            ))
        }
    }
);
```

## Sub-type Variations

### Boleto (Brazil)

| Field | Required | Source |
|-------|----------|--------|
| `social_security_number` | Yes | VoucherData::Boleto |
| `due_date` | Optional | VoucherData::Boleto |
| `document_type` | Optional | VoucherData::Boleto |

### Oxxo (Mexico)

| Field | Required | Source |
|-------|----------|--------|
| None | - | - |

### Alfamart/Indomaret (Indonesia)

| Field | Required | Source |
|-------|----------|--------|
| `first_name` | Yes | billing.first_name |
| `last_name` | Optional | billing.last_name |
| `shopper_email` | Yes | request.email |

### Japanese Convenience Stores

| Field | Required | Source |
|-------|----------|--------|
| `first_name` | Yes | billing.first_name |
| `last_name` | Optional | billing.last_name |
| `shopper_email` | Yes | request.email |
| `telephone_number` | Yes | billing.phone |

## Common Pitfalls

### 1. Wrong Email Type

**Problem**: Using bare `Email` type instead of `pii::Email`.

**Wrong**:
```rust
pub shopper_email: Email,  // ERROR: Email not in scope
```

**Correct**:
```rust
use common_utils::pii;
pub shopper_email: pii::Email,  // CORRECT
```

### 2. Missing VoucherNextStepData

**Problem**: Not returning `PaymentsResponseData::VoucherNextStepData`.

**Wrong**:
```rust
response: Ok(PaymentsResponseData::TransactionResponse { ... })  // WRONG for vouchers
```

**Correct**:
```rust
response: Ok(PaymentsResponseData::VoucherNextStepData(Box::new(voucher_data)))  // CORRECT
```

### 3. Missing Reference Field

**Problem**: The `reference` field is required but empty or missing.

**Solution**: Always ensure `reference` is populated from the connector response:
```rust
let voucher_data = VoucherNextStepData {
    reference: response.reference,  // REQUIRED - cannot be empty
    // ...
};
```

### 4. Wrong Status

**Problem**: Setting wrong attempt status for vouchers.

**Correct**: Vouchers should return `AttemptStatus::AuthenticationPending` because the customer needs to complete payment at a physical location:
```rust
status: AttemptStatus::AuthenticationPending,
```

### 5. Generic Error Messages

**Problem**: Using generic "Not supported" error messages.

**Wrong**:
```rust
Err(ConnectorError::NotImplemented("Not supported".to_string()))
```

**Correct**:
```rust
Err(ConnectorError::NotSupported {
    message: "Boleto is not supported by this connector".to_string(),
    connector: "connector_name",
})
```

## Testing Patterns

### Unit Tests for Request Building

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boleto_request_building() {
        let boleto_data = VoucherData::Boleto(Box::new(BoletoVoucherData {
            social_security_number: Some(Secret::new("123.456.789-00".to_string())),
            ..Default::default()
        }));

        // Test request building
    }

    #[test]
    fn test_oxxo_request_building() {
        let oxxo_data = VoucherData::Oxxo;

        // Test request building
    }
}
```

## Implementation Checklist

When implementing Voucher for a connector:

- [ ] Identify supported voucher types
- [ ] Use `pii::Email` type (not bare `Email`)
- [ ] Implement request type matching for each sub-type
- [ ] Return `PaymentsResponseData::VoucherNextStepData`
- [ ] Ensure `reference` field is always populated
- [ ] Set status to `AuthenticationPending`
- [ ] Use specific error messages for unsupported types
- [ ] Test each voucher type individually
- [ ] Verify webhook handling for async status updates

## Cross-References

- [pattern_authorize.md](../pattern_authorize.md) - General authorize flow patterns
- [pattern_webhooks.md](../pattern_webhooks.md) - Webhook handling patterns
- [pattern_psync.md](../pattern_psync.md) - Payment sync patterns for async payments

---

**Document Version**: 1.0
**Last Updated**: 2026-03-24
**Maintained By**: Grace-UCS Connector Team
