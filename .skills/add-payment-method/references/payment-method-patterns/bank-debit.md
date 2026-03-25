# Bank Debit Authorize Pattern Reference

## Payment Method: Bank Debit (ACH, SEPA, BECS, BACS)

Asynchronous direct debit from customer bank accounts. Requires mandate authorization for recurring.
Settlement: 3-7 business days. Higher chargeback risk than cards.

## BankDebitData Variants

```rust
// crates/types-traits/domain_types/src/payment_method_data.rs
pub enum BankDebitData {
    AchBankDebit {
        account_number: Secret<String>,
        routing_number: Secret<String>,
        card_holder_name: Option<Secret<String>>,
        bank_account_holder_name: Option<Secret<String>>,
        bank_name: Option<common_enums::BankNames>,
        bank_type: Option<common_enums::BankType>,
        bank_holder_type: Option<common_enums::BankHolderType>,
    },
    SepaBankDebit {
        iban: Secret<String>,
        bank_account_holder_name: Option<Secret<String>>,
    },
    BecsBankDebit {
        account_number: Secret<String>,
        bsb_number: Secret<String>,
        bank_account_holder_name: Option<Secret<String>>,
    },
    BacsBankDebit {
        account_number: Secret<String>,
        sort_code: Secret<String>,
        bank_account_holder_name: Option<Secret<String>>,
    },
}
```

| Variant | Region | Key Fields |
|---------|--------|------------|
| AchBankDebit | USA | `account_number`, `routing_number` |
| SepaBankDebit | EU | `iban` |
| BecsBankDebit | Australia | `account_number`, `bsb_number` |
| BacsBankDebit | UK | `account_number`, `sort_code` |

## Connector Support

| Connector | ACH | SEPA | BECS | BACS | Mandate |
|-----------|-----|------|------|------|---------|
| Adyen | Yes | Yes | No | Yes | Yes |
| Stripe | Yes | Yes | Yes | Yes | Yes |
| Novalnet | No | Yes | No | No | Yes |

## PaymentMethodData Match Arm

```rust
PaymentMethodData::BankDebit(ref bank_debit_data) => {
    // Extract variant-specific fields and build connector request
}
```

## Adyen TryFrom Pattern

```rust
impl TryFrom<(&BankDebitData, &RouterDataV2<...>)> for AdyenPaymentMethod<T> {
    fn try_from((bank_debit_data, item): (...)) -> Result<Self, Self::Error> {
        match bank_debit_data {
            BankDebitData::AchBankDebit { account_number, routing_number, .. } => {
                Ok(Self::AchDirectDebit(Box::new(AchDirectDebitData {
                    bank_account_number: account_number.clone(),
                    bank_location_id: routing_number.clone(),
                    owner_name: item.resource_common_data.get_billing_full_name()?,
                })))
            }
            BankDebitData::SepaBankDebit { iban, .. } => {
                Ok(Self::SepaDirectDebit(Box::new(SepaDirectDebitData {
                    owner_name: item.resource_common_data.get_billing_full_name()?,
                    iban_number: iban.clone(),
                })))
            }
            BankDebitData::BacsBankDebit { account_number, sort_code, .. } => {
                Ok(Self::BacsDirectDebit(Box::new(BacsDirectDebitData {
                    bank_account_number: account_number.clone(),
                    bank_location_id: sort_code.clone(),
                    holder_name: item.resource_common_data.get_billing_full_name()?,
                })))
            }
            BankDebitData::BecsBankDebit { .. } => Err(ConnectorError::NotImplemented(...)),
        }
    }
}
```

## Stripe Pattern

```rust
fn get_bank_debit_data(bank_debit_data: &BankDebitData) -> (StripePaymentMethodType, BankDebitData) {
    match bank_debit_data {
        BankDebitData::AchBankDebit { account_number, routing_number, .. } => {
            (StripePaymentMethodType::Ach, BankDebitData::Ach {
                account_holder_type: "individual".to_string(),
                account_number: account_number.to_owned(),
                routing_number: routing_number.to_owned(),
            })
        }
        BankDebitData::SepaBankDebit { iban, .. } => {
            (StripePaymentMethodType::Sepa, BankDebitData::Sepa { iban: iban.to_owned() })
        }
        BankDebitData::BecsBankDebit { account_number, bsb_number, .. } => {
            (StripePaymentMethodType::Becs, BankDebitData::Becs {
                account_number: account_number.to_owned(),
                bsb_number: bsb_number.to_owned(),
            })
        }
        BankDebitData::BacsBankDebit { account_number, sort_code, .. } => {
            (StripePaymentMethodType::Bacs, BankDebitData::Bacs {
                account_number: account_number.to_owned(),
                sort_code: Secret::new(sort_code.clone().expose().replace('-', "")),
            })
        }
    }
}
```

## Novalnet SEPA-Only Pattern

```rust
PaymentMethodData::BankDebit(ref bank_debit_data) => {
    let (iban, account_holder) = match bank_debit_data {
        BankDebitData::SepaBankDebit { iban, bank_account_holder_name } => {
            let holder = bank_account_holder_name.clone()
                .unwrap_or(router_data.resource_common_data.get_billing_full_name()?);
            (iban.clone(), holder)
        }
        _ => return Err(ConnectorError::NotImplemented(...)),
    };
    // Build NovalnetSepaDebit { account_holder, iban }
}
```

## ACH State Code Handling

For ACH, override billing state with state code:
```rust
let billing_address = match bank_debit_data {
    BankDebitData::AchBankDebit { .. } => billing_address.map(|mut addr| {
        addr.state_or_province = item.router_data.resource_common_data
            .get_optional_billing()
            .and_then(|b| b.address.as_ref())
            .and_then(|address| address.to_state_code_as_optional().ok().flatten())
            .or(addr.state_or_province);
        addr
    }),
    _ => billing_address,
};
```

## Account Holder Name Extraction

```rust
fn get_account_holder_name(bank_debit_data: &BankDebitData, router_data: &...) -> Result<Secret<String>> {
    match bank_debit_data {
        BankDebitData::AchBankDebit { bank_account_holder_name, .. }
        | BankDebitData::SepaBankDebit { bank_account_holder_name, .. }
        | BankDebitData::BecsBankDebit { bank_account_holder_name, .. }
        | BankDebitData::BacsBankDebit { bank_account_holder_name, .. } => {
            bank_account_holder_name.clone()
                .or_else(|| router_data.resource_common_data.get_billing_full_name().ok())
                .ok_or_else(|| ConnectorError::MissingRequiredField {
                    field_name: "bank_account_holder_name",
                })
        }
    }
}
```

## Status Mapping

```rust
impl From<ConnectorBankDebitStatus> for AttemptStatus {
    fn from(status: ConnectorBankDebitStatus) -> Self {
        match status {
            Pending | Processing => Self::Pending,
            Confirmed => Self::Charged,
            Failed => Self::Failure,
            Cancelled => Self::Voided,
        }
    }
}
```

## Response Pattern (with Mandate Reference)

Bank debit responses include mandate references for recurring payments:
```rust
let mandate_reference = response.mandate_id.as_ref().map(|id| MandateReference {
    connector_mandate_id: Some(id.clone()),
    payment_method_id: None,
});

PaymentsResponseData::TransactionResponse {
    resource_id: ResponseId::ConnectorTransactionId(response.transaction_id.clone()),
    redirection_data: response.redirect_url.as_ref().map(|url| RedirectForm::Uri { uri: url.clone() }),
    mandate_reference,
    connector_metadata: None,
    network_txn_id: None,
    connector_response_reference_id: Some(response.reference.clone()),
    incremental_authorization_allowed: None,
    status_code: item.http_code,
}
```

## Key Implementation Notes

- Always extract `bank_account_holder_name` with fallback to billing full name
- Stripe BACS: strip dashes from sort_code (`sort_code.expose().replace('-', "")`)
- Bank debits are async; implement PSync for status polling
- Mandate support is critical for recurring bank debit payments
- For macro usage, see `macro-reference.md`
