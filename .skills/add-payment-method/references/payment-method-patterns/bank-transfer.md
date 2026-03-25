# Bank Transfer Authorize Pattern Reference

## Quick Reference

| Aspect | Value |
|--------|-------|
| **Payment Method Type** | `BankTransfer` |
| **Response Type** | Async (typically requires webhook) |
| **Amount Unit** | Minor (most connectors) |
| **3DS / Mandate Support** | No |

## BankTransferData Enum

```rust
// crates/types-traits/domain_types/src/payment_method_data.rs
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BankTransferData {
    AchBankTransfer,
    SepaBankTransfer,
    BacsBankTransfer,
    MultibancoBankTransfer,
    PermataBankTransfer,
    BcaBankTransfer,
    BniVaBankTransfer,
    BriVaBankTransfer,
    CimbVaBankTransfer,
    DanamonVaBankTransfer,
    MandiriVaBankTransfer,
    Pix { pix_key: Option<String>, pix_key_type: Option<String> },
    Pse,
    LocalBankTransfer { bank_name: Option<String> },
    InstantBankTransfer,
    InstantBankTransferFinland,
    InstantBankTransferPoland,
}
```

## Region-Based Variants

| Region | Variant | Flow | Special Requirements |
|--------|---------|------|---------------------|
| US | `AchBankTransfer` | Instructions | Account/routing number |
| EU | `SepaBankTransfer` | Instructions | IBAN/BIC, `billing_address.country` required |
| UK | `BacsBankTransfer` | Instructions | Sort code/account number |
| Portugal | `MultibancoBankTransfer` | Reference + Entity | `billing_address.email` required |
| Indonesia | Various VA variants | Redirect or QR | Virtual account number, `shopper_email` required |
| Brazil | `Pix` | QR Code or Key | Pix key or QR code |

## PaymentMethodData::BankTransfer Match Arm

Top-level match in request body construction:

```rust
match item.router_data.request.payment_method_data {
    PaymentMethodData::Card(ref ccard) => { /* card handling */ },
    PaymentMethodData::BankRedirect(ref bank_redirection_data) => { /* redirect handling */ },
    PaymentMethodData::BankTransfer(ref bank_transfer_data) => {
        get_bank_transfer_request_data(
            item.router_data.clone(),
            bank_transfer_data,
            params,
            amount,
            auth,
        )
    }
    // ... other variants
}
```

## TryFrom: BankTransferData -> Connector Payment Method (Adyen Example)

```rust
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        &BankTransferData,
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    )> for AdyenPaymentMethod<T>
{
    type Error = Error;
    fn try_from(
        (bank_transfer_data, item): (
            &BankTransferData,
            &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
    ) -> Result<Self, Self::Error> {
        match bank_transfer_data {
            BankTransferData::PermataBankTransfer {} => Ok(Self::PermataBankTransfer(Box::new(
                DokuBankData::try_from(item)?,
            ))),
            BankTransferData::BcaBankTransfer {} => Ok(Self::BcaBankTransfer(Box::new(
                DokuBankData::try_from(item)?,
            ))),
            BankTransferData::BniVaBankTransfer {} => {
                Ok(Self::BniVa(Box::new(DokuBankData::try_from(item)?)))
            }
            BankTransferData::BriVaBankTransfer {} => {
                Ok(Self::BriVa(Box::new(DokuBankData::try_from(item)?)))
            }
            BankTransferData::CimbVaBankTransfer {} => {
                Ok(Self::CimbVa(Box::new(DokuBankData::try_from(item)?)))
            }
            BankTransferData::DanamonVaBankTransfer {} => {
                Ok(Self::DanamonVa(Box::new(DokuBankData::try_from(item)?)))
            }
            BankTransferData::MandiriVaBankTransfer {} => {
                Ok(Self::MandiriVa(Box::new(DokuBankData::try_from(item)?)))
            }
            BankTransferData::Pix { .. } => Ok(Self::Pix),
            // Unsupported variants -> NotImplemented error
            BankTransferData::AchBankTransfer {}
            | BankTransferData::SepaBankTransfer {}
            | BankTransferData::BacsBankTransfer {}
            | BankTransferData::MultibancoBankTransfer {}
            | BankTransferData::Pse {}
            | BankTransferData::LocalBankTransfer { .. }
            | BankTransferData::InstantBankTransfer {}
            | BankTransferData::InstantBankTransferFinland {}
            | BankTransferData::InstantBankTransferPoland {} => {
                Err(errors::ConnectorError::NotImplemented(
                    utils::get_unimplemented_payment_method_error_message("Adyen"),
                ).into())
            }
        }
    }
}
```

Helper struct for Indonesian VA bank data:

```rust
#[derive(Debug, Serialize)]
pub struct DokuBankData {
    #[serde(rename = "type")]
    pub payment_method_type: String,
    pub shopper_email: Email,
}
```

## Stripe Variant Handling (Form-Encoded)

Pattern: each variant maps to a `StripeBankTransferData` enum arm with region-specific fields.
Return tuple is `(StripePaymentMethodData, Option<StripePaymentMethodType>, StripeBillingAddress)`.

```rust
PaymentMethodData::BankTransfer(bank_transfer_data) => match bank_transfer_data.deref() {
    BankTransferData::AchBankTransfer {} => Ok((
        StripePaymentMethodData::BankTransfer(StripeBankTransferData::AchBankTransfer(
            Box::new(AchTransferData {
                payment_method_data_type: StripePaymentMethodType::CustomerBalance,
                bank_transfer_type: StripeCreditTransferTypes::AchCreditTransfer,
                payment_method_type: StripePaymentMethodType::CustomerBalance,
                balance_funding_type: BankTransferType::BankTransfers,
            }),
        )),
        None, StripeBillingAddress::default(),
    )),
    BankTransferData::SepaBankTransfer {} => Ok((
        StripePaymentMethodData::BankTransfer(StripeBankTransferData::SepaBankTransfer(
            Box::new(SepaBankTransferData {
                payment_method_data_type: StripePaymentMethodType::CustomerBalance,
                bank_transfer_type: BankTransferType::EuBankTransfer,
                balance_funding_type: BankTransferType::BankTransfers,
                payment_method_type: StripePaymentMethodType::CustomerBalance,
                country: payment_request_details.billing_address.country
                    .ok_or(ConnectorError::MissingRequiredField {
                        field_name: "billing_address.country",
                    })?,
            }),
        )),
        Some(StripePaymentMethodType::CustomerBalance),
        payment_request_details.billing_address,
    )),
    // BacsBankTransfer: same shape, bank_transfer_type = GbBankTransfer
    // MultibancoBankTransfer: uses StripeCreditTransferTypes::Multibanco, requires email
}
```

## Bank Detail Extraction: Response -> NextAction Instructions

The response handler extracts bank details from `next_action` into typed instruction structs:

```rust
// SEPA/BACS: extract from financial_addresses
SepaFinancialDetails { account_holder_name, bic, iban, bank_name }
BacsFinancialDetails { account_holder_name, account_number, sort_code }
// -> BankTransferInstructions::SepaAndBacs(Box::new(...))

// ACH: extract from AchFinancialInformation
AchTransfer { account_number, routing_number, bank_name, swift_code, amount_charged: None }
// -> BankTransferInstructions::AchCreditTransfer(Box::new(...))

// Multibanco: extract reference and entity
MultibancoTransferInstructions { reference, entity }
// -> BankTransferInstructions::Multibanco(Box::new(...))
```

All are wrapped in `NextActionsResponse::DisplayBankTransferInstructions` or
`NextActionsResponse::BankTransferInstructions` and returned via `BankTransferNextStepsData`.

## Webhook Status Mapping

```rust
impl From<WebhookStatus> for enums::AttemptStatus {
    fn from(status: WebhookStatus) -> Self {
        match status {
            WebhookStatus::Pending => Self::Pending,
            WebhookStatus::Received => Self::Authorized,
            WebhookStatus::Completed => Self::Charged,
            WebhookStatus::Failed => Self::Failure,
            WebhookStatus::Refunded => Self::CaptureMethodNotSupported,
            WebhookStatus::Disputed => Self::CaptureMethodNotSupported,
        }
    }
}
```

## Required Field Validation

```rust
// Multibanco requires email
let email = payment_request_details.billing_address.email.ok_or(
    ConnectorError::MissingRequiredField {
        field_name: "billing_address.email",
    },
)?;

// SEPA requires country
let country = payment_request_details.billing_address.country.ok_or(
    ConnectorError::MissingRequiredField {
        field_name: "billing_address.country",
    },
)?;
```

## Key Implementation Notes

1. **Always async**: Return `AuthenticationPending` or `Authorized` initially; never `Charged`. Rely on webhooks for final status.
2. **Variant exhaustiveness**: Match all `BankTransferData` variants. Return `ConnectorError::NotImplemented` for unsupported ones.
3. **Box large variants**: Use `Box::new(...)` when wrapping transfer data structs to avoid large enum sizes.
4. **Dynamic URL routing**: Some connectors need different endpoints per bank transfer sub-type -- use a nested match on `BankTransferData` inside `get_url`.
5. **Timeout**: Bank transfers can take up to 72 hours. Configure extended timeouts accordingly.
6. **Amount precision**: Always use minor units. Include currency in customer-facing instructions.
