use common_enums::{self, enums, AttemptStatus, RefundStatus};
use common_utils::{consts, pii::Email, types::FloatMajorUnit};
use domain_types::{
    connector_flow::{
        Authorize, CreateConnectorCustomer, PSync, RSync, Refund, RepeatPayment, SetupMandate,
    },
    connector_types::{
        ConnectorCustomerData, ConnectorCustomerResponse, MandateReference, MandateReferenceId,
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, RepeatPaymentData, ResponseId, SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError, WebhookError},
    payment_method_data::{
        BankDebitData, DefaultPCIHolder, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber,
        VaultTokenHolder, WalletData,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
};

use crate::types::ResponseRouterData;
// Alias to make the transition easier
type HsInterfacesConnectorRequestError = IntegrationError;
use std::str::FromStr;

use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, ExposeOptionInterface, PeekInterface, Secret};
use rand::distributions::{Alphanumeric, DistString};
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;

use super::AuthorizedotnetRouterData;

type Error = error_stack::Report<IntegrationError>;
type ResponseError = error_stack::Report<ConnectorError>;

// Constants
const MAX_ID_LENGTH: usize = 20;
const ADDRESS_MAX_LENGTH: usize = 60; // Authorize.Net address field max length

// Helper function for concatenating address lines with length constraints
fn get_address_line(
    address_line1: &Option<Secret<String>>,
    address_line2: &Option<Secret<String>>,
    address_line3: &Option<Secret<String>>,
) -> Option<Secret<String>> {
    for lines in [
        vec![address_line1, address_line2, address_line3],
        vec![address_line1, address_line2],
    ] {
        let combined: String = lines
            .into_iter()
            .flatten()
            .map(|s| s.clone().expose())
            .collect::<Vec<_>>()
            .join(" ");

        if !combined.is_empty() && combined.len() <= ADDRESS_MAX_LENGTH {
            return Some(Secret::new(combined));
        }
    }
    address_line1.clone()
}

// Extract credit card payment details from refund metadata
fn get_refund_credit_card_payment(
    connector_metadata: &Option<Secret<serde_json::Value>>,
) -> Result<RefundPaymentDetails, Error> {
    let metadata = connector_metadata
        .as_ref()
        .ok_or_else(|| {
            error_stack::report!(HsInterfacesConnectorRequestError::MissingRequiredField {
                field_name: "connector_metadata",
                context: Default::default()
            })
        })?
        .peek();

    // Extract creditCard field (stringified JSON or JSON object)
    let credit_card_value = metadata.get("creditCard").ok_or_else(|| {
        error_stack::report!(HsInterfacesConnectorRequestError::MissingRequiredField {
            field_name: "creditCard",
            context: Default::default()
        })
    })?;

    let credit_card: serde_json::Value = match credit_card_value {
        serde_json::Value::String(credit_card_str) => serde_json::from_str(credit_card_str)
            .inspect_err(|e| {
                tracing::error!(
                    error = %e,
                    credit_card_str = %credit_card_str,
                    "Failed to parse credit card JSON"
                );
            })
            .change_context(HsInterfacesConnectorRequestError::RequestEncodingFailed {
                context: Default::default(),
            })?,
        serde_json::Value::Object(_) => credit_card_value.clone(),
        _ => {
            return Err(error_stack::report!(
                HsInterfacesConnectorRequestError::MissingRequiredField {
                    field_name: "creditCard",
                    context: Default::default()
                }
            ))
        }
    };

    let card_number = credit_card
        .get("cardNumber")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            error_stack::report!(HsInterfacesConnectorRequestError::MissingRequiredField {
                field_name: "cardNumber",
                context: Default::default()
            })
        })?
        .to_string();

    let expiration_date = credit_card
        .get("expirationDate")
        .and_then(|v| v.as_str())
        .unwrap_or("XXXX")
        .to_string();

    Ok(RefundPaymentDetails {
        credit_card: CreditCardInfo {
            card_number,
            expiration_date,
        },
    })
}

fn get_random_string() -> String {
    Alphanumeric.sample_string(&mut rand::thread_rng(), MAX_ID_LENGTH)
}

/// Returns invoice number if length <= MAX_ID_LENGTH, otherwise random string
fn get_invoice_number_or_random(merchant_order_id: Option<String>) -> String {
    match merchant_order_id {
        Some(num) if num.len() <= MAX_ID_LENGTH => num,
        None | Some(_) => get_random_string(),
    }
}

/// Returns customer ID only if length <= MAX_ID_LENGTH
fn validate_customer_id_length(customer_id: Option<String>) -> Option<String> {
    customer_id.filter(|id| id.len() <= MAX_ID_LENGTH)
}

/// Convert metadata to UserFields with optional serialization
fn metadata_to_user_fields(
    metadata: Option<serde_json::Value>,
    needs_serialization: bool,
) -> Result<Option<UserFields>, Error> {
    let meta = match metadata {
        Some(m) => m,
        None => return Ok(None),
    };

    let value = if needs_serialization {
        serde_json::to_value(meta).change_context(IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        })?
    } else {
        meta
    };

    Ok(Some(UserFields {
        user_field: Vec::<UserField>::foreign_try_from(value)?,
    }))
}

// // Helper traits for working with generic types
// trait RawCardNumberExt<T: PaymentMethodDataTypes> {
//     fn peek(&self) -> &str;
// }

// trait CardExt<T: PaymentMethodDataTypes> {
//     fn get_expiry_date_as_yyyymm(&self, separator: &str) -> Secret<String>;
// }

// // Implementations for DefaultPCIHolder
// impl RawCardNumberExt<DefaultPCIHolder> for RawCardNumber<DefaultPCIHolder> {
//     fn peek(&self) -> &str {
//         self.0.peek()
//     }
// }

// impl CardExt<DefaultPCIHolder> for domain_types::payment_method_data::Card<DefaultPCIHolder> {
//     fn get_expiry_date_as_yyyymm(&self, separator: &str) -> Secret<String> {
//         Secret::new(format!("{}{}{}",
//             self.card_exp_year.peek(),
//             separator,
//             self.card_exp_month.peek()
//         ))
//     }
// }

// // Implementations for VaultTokenHolder
// impl RawCardNumberExt<VaultTokenHolder> for RawCardNumber<VaultTokenHolder> {
//     fn peek(&self) -> &str {
//         &self.0
//     }
// }

// impl CardExt<VaultTokenHolder> for domain_types::payment_method_data::Card<VaultTokenHolder> {
//     fn get_expiry_date_as_yyyymm(&self, separator: &str) -> Secret<String> {
//         Secret::new(format!("{}{}{}",
//             self.card_exp_year.peek(),
//             separator,
//             self.card_exp_month.peek()
//         ))
//     }
// }

// Wrapper for RawCardNumber to provide construction methods
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AuthorizedotnetRawCardNumber<T: PaymentMethodDataTypes>(pub RawCardNumber<T>);

impl AuthorizedotnetRawCardNumber<DefaultPCIHolder> {
    pub fn from_card_number_string(card_number: String) -> Result<Self, Error> {
        let card_number = cards::CardNumber::from_str(&card_number).change_context(
            IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            },
        )?;
        Ok(Self(RawCardNumber(card_number)))
    }
}

impl AuthorizedotnetRawCardNumber<VaultTokenHolder> {
    pub fn from_token_string(token: String) -> Self {
        Self(RawCardNumber(token))
    }
}

// Implement From to convert back to RawCardNumber
impl<T: PaymentMethodDataTypes> From<AuthorizedotnetRawCardNumber<T>> for RawCardNumber<T> {
    fn from(wrapper: AuthorizedotnetRawCardNumber<T>) -> Self {
        wrapper.0
    }
}

// Re-export common enums for use in this file
pub mod api_enums {
    pub use common_enums::Currency;
}

pub trait ForeignTryFrom<F>: Sized {
    type Error;

    fn foreign_try_from(from: F) -> Result<Self, Self::Error>;
}

#[skip_serializing_none]
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MerchantAuthentication {
    name: Secret<String>,
    transaction_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for MerchantAuthentication {
    type Error = Error;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Authorizedotnet {
                name,
                transaction_key,
                ..
            } => Ok(Self {
                name: name.clone(),
                transaction_key: transaction_key.clone(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
            )),
        }
    }
}
impl ForeignTryFrom<serde_json::Value> for Vec<UserField> {
    type Error = Error;
    fn foreign_try_from(metadata: serde_json::Value) -> Result<Self, Self::Error> {
        let mut vector = Self::new();

        if let serde_json::Value::Object(obj) = metadata {
            for (key, value) in obj {
                vector.push(UserField {
                    name: key,
                    value: match value {
                        serde_json::Value::String(s) => s,
                        _ => value.to_string(),
                    },
                });
            }
        }

        Ok(vector)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthorizationType {
    Final,
    Pre,
}

impl TryFrom<enums::CaptureMethod> for AuthorizationType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(capture_method: enums::CaptureMethod) -> Result<Self, Self::Error> {
        match capture_method {
            enums::CaptureMethod::Manual => Ok(Self::Pre),
            enums::CaptureMethod::SequentialAutomatic | enums::CaptureMethod::Automatic => {
                Ok(Self::Final)
            }
            enums::CaptureMethod::ManualMultiple | enums::CaptureMethod::Scheduled => {
                Err(error_stack::report!(IntegrationError::NotSupported {
                    message: "Capture method not supported".to_string(),
                    connector: "authorizedotnet",
                    context: Default::default()
                }))?
            }
        }
    }
}

#[skip_serializing_none]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CreditCardDetails<T: PaymentMethodDataTypes> {
    card_number: RawCardNumber<T>,
    expiration_date: Secret<String>, // YYYY-MM
    card_code: Option<Secret<String>>,
}

#[skip_serializing_none]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BankAccountDetails {
    account_type: AccountType,
    routing_number: Secret<String>,
    account_number: Secret<String>,
    name_on_account: Secret<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PayPalDetails {
    pub success_url: Option<String>,
    pub cancel_url: Option<String>,
}

#[derive(Serialize, Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WalletDetails {
    pub data_descriptor: WalletMethod,
    pub data_value: Secret<String>,
}

#[derive(Serialize, Debug, Deserialize, Clone)]
pub enum WalletMethod {
    #[serde(rename = "COMMON.GOOGLE.INAPP.PAYMENT")]
    Googlepay,
    #[serde(rename = "COMMON.APPLE.INAPP.PAYMENT")]
    Applepay,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum PaymentDetails<T: PaymentMethodDataTypes> {
    CreditCard(CreditCardDetails<T>),
    BankAccount(BankAccountDetails),
    OpaqueData(WalletDetails),
    PayPal(PayPalDetails),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum AccountType {
    Checking,
    Savings,
    BusinessChecking,
}

#[skip_serializing_none]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TransactionType {
    AuthOnlyTransaction,
    AuthCaptureTransaction,
    PriorAuthCaptureTransaction,
    VoidTransaction,
    RefundTransaction,
}

#[skip_serializing_none]
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    invoice_number: String,
    description: String,
}

#[skip_serializing_none]
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BillTo {
    first_name: Option<Secret<String>>,
    last_name: Option<Secret<String>>,
    address: Option<Secret<String>>,
    city: Option<Secret<String>>,
    state: Option<Secret<String>>,
    zip: Option<Secret<String>>,
    country: Option<enums::CountryAlpha2>,
}

#[skip_serializing_none]
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShipTo {
    first_name: Option<Secret<String>>,
    last_name: Option<Secret<String>>,
    company: Option<String>,
    address: Option<Secret<String>>,
    city: Option<String>,
    state: Option<String>,
    zip: Option<Secret<String>>,
    country: Option<enums::CountryAlpha2>,
}

#[skip_serializing_none]
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomerDetails {
    id: String,
    email: Option<Email>,
}

#[skip_serializing_none]
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserField {
    name: String,
    value: String,
}

#[skip_serializing_none]
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserFields {
    user_field: Vec<UserField>,
}

#[skip_serializing_none]
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessingOptions {
    is_subsequent_auth: bool,
}

#[skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubsequentAuthInformation {
    original_network_trans_id: Secret<String>,
    reason: Reason,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Reason {
    Resubmission,
    #[serde(rename = "delayedCharge")]
    DelayedCharge,
    Reauthorization,
    #[serde(rename = "noShow")]
    NoShow,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ProfileDetails {
    CreateProfileDetails(CreateProfileDetails),
    CustomerProfileDetails(CustomerProfileDetails),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProfileDetails {
    create_profile: bool,
    customer_profile_id: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomerProfileDetails {
    customer_profile_id: Secret<String>,
    payment_profile: PaymentProfileDetails,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentProfileDetails {
    payment_profile_id: Secret<String>,
}

#[skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetTransactionRequest<T: PaymentMethodDataTypes> {
    // General structure for transaction details in Authorize
    transaction_type: TransactionType,
    amount: Option<FloatMajorUnit>,
    currency_code: Option<api_enums::Currency>,
    payment: Option<PaymentDetails<T>>,
    profile: Option<ProfileDetails>,
    order: Option<Order>,
    customer: Option<CustomerDetails>,
    bill_to: Option<BillTo>,
    user_fields: Option<UserFields>,
    processing_options: Option<ProcessingOptions>,
    subsequent_auth_information: Option<SubsequentAuthInformation>,
    ref_trans_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionSettings {
    setting: Vec<TransactionSetting>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionSetting {
    setting_name: String,
    setting_value: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTransactionRequest<T: PaymentMethodDataTypes> {
    // Used by Authorize Flow, wraps the general transaction request
    merchant_authentication: AuthorizedotnetAuthType,
    ref_id: Option<String>,
    transaction_request: AuthorizedotnetTransactionRequest<T>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetPaymentsRequest<T: PaymentMethodDataTypes> {
    // Top-level wrapper for Authorize Flow
    create_transaction_request: CreateTransactionRequest<T>,
}

// Implementation for owned RouterData that doesn't depend on reference version
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AuthorizedotnetRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AuthorizedotnetPaymentsRequest<T>
{
    type Error = Error;
    fn try_from(
        item: AuthorizedotnetRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let merchant_authentication =
            AuthorizedotnetAuthType::try_from(&item.router_data.connector_config)?;

        let currency_str = item.router_data.request.currency.to_string();
        let currency = api_enums::Currency::from_str(&currency_str).map_err(|_| {
            error_stack::report!(IntegrationError::RequestEncodingFailed {
                context: Default::default()
            })
        })?;

        // Always create regular transaction request (mandate logic moved to RepeatPayment flow)
        let transaction_request = create_regular_transaction_request(&item, currency)?;

        let ref_id = Some(
            item.router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        )
        .filter(|id| id.len() <= MAX_ID_LENGTH);
        let create_transaction_request = CreateTransactionRequest {
            merchant_authentication,
            ref_id,
            transaction_request,
        };

        Ok(Self {
            create_transaction_request,
        })
    }
}

// Helper function to create regular transaction request (non-mandate)
fn create_regular_transaction_request<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: &AuthorizedotnetRouterData<
        RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        T,
    >,
    currency: api_enums::Currency,
) -> Result<AuthorizedotnetTransactionRequest<T>, Error> {
    let payment_details = match &item.router_data.request.payment_method_data {
        PaymentMethodData::Card(card) => {
            let expiry_month = card.card_exp_month.peek().clone();
            let year = card.card_exp_year.peek().clone();
            let expiry_year = if year.len() == 2 {
                format!("20{year}")
            } else {
                year
            };
            let expiration_date = format!("{expiry_year}-{expiry_month}");

            let credit_card_details = CreditCardDetails {
                card_number: card.card_number.clone(),
                expiration_date: Secret::new(expiration_date),
                card_code: Some(card.card_cvc.clone()),
            };

            Ok(PaymentDetails::CreditCard(credit_card_details))
        }
        PaymentMethodData::BankDebit(bank_debit_data) => {
            match bank_debit_data {
                BankDebitData::AchBankDebit {
                    account_number,
                    routing_number,
                    bank_account_holder_name,
                    card_holder_name,
                    bank_type,
                    bank_holder_type,
                    ..
                } => {
                    // Get account holder name from bank_account_holder_name, card_holder_name,
                    // or billing address
                    let name_on_account = bank_account_holder_name
                        .clone()
                        .or_else(|| card_holder_name.clone())
                        .or_else(|| {
                            item.router_data
                                .resource_common_data
                                .get_optional_billing_full_name()
                        })
                        .ok_or_else(|| {
                            error_stack::report!(IntegrationError::MissingRequiredField {
                                field_name: "bank_account_holder_name",
                context: Default::default()
                            })
                        })?;

                    // Map bank_type and bank_holder_type to AccountType
                    // Business accounts with checking should use BusinessChecking
                    let account_type = match (bank_type, bank_holder_type) {
                        (Some(common_enums::BankType::Savings), _) => AccountType::Savings,
                        (_, Some(common_enums::BankHolderType::Business)) => {
                            AccountType::BusinessChecking
                        }
                        _ => AccountType::Checking
};

                    let bank_account_details = BankAccountDetails {
                        account_type,
                        routing_number: routing_number.clone(),
                        account_number: account_number.clone(),
                        name_on_account
};

                    Ok(PaymentDetails::BankAccount(bank_account_details))
                }
                BankDebitData::SepaBankDebit { .. }
                | BankDebitData::SepaGuaranteedBankDebit { .. }
                | BankDebitData::BecsBankDebit { .. }
                | BankDebitData::BacsBankDebit { .. } => {
                    Err(error_stack::report!(IntegrationError::not_implemented(
                        "SEPA, SEPA Guaranteed, BECS, and BACS bank debits are not supported for authorizedotnet"
                            .to_string(),
                    )))
                }
            }
        }
        PaymentMethodData::Wallet(wallet_data) => {
            get_wallet_payment_details(wallet_data, &item.router_data.request.router_return_url)
        }
        pm => Err(error_stack::report!(IntegrationError::not_implemented(
            format!("Payment method {:?}", pm)
        ))),
    }?;

    let transaction_type = match item.router_data.request.capture_method {
        Some(enums::CaptureMethod::Manual) => TransactionType::AuthOnlyTransaction,
        Some(enums::CaptureMethod::Automatic)
        | None
        | Some(enums::CaptureMethod::SequentialAutomatic) => {
            TransactionType::AuthCaptureTransaction
        }
        Some(_) => {
            return Err(error_stack::report!(IntegrationError::NotSupported {
                message: "Capture method not supported".to_string(),
                connector: "authorizedotnet",
                context: Default::default()
            }))
        }
    };

    let order_description = item
        .router_data
        .resource_common_data
        .connector_request_reference_id
        .clone();

    // Get invoice number (random string if > MAX_ID_LENGTH or None)
    let invoice_number =
        get_invoice_number_or_random(item.router_data.request.merchant_order_id.clone());

    let order = Order {
        invoice_number,
        description: order_description,
    };

    // Extract user fields from metadata
    let user_fields = metadata_to_user_fields(
        item.router_data.request.metadata.clone().expose_option(),
        false,
    )?;

    // Process billing address
    let billing_address = item
        .router_data
        .resource_common_data
        .address
        .get_payment_billing();
    let bill_to = billing_address.as_ref().map(|billing| {
        let first_name = billing.address.as_ref().and_then(|a| a.first_name.clone());
        let last_name = billing.address.as_ref().and_then(|a| a.last_name.clone());

        // Concatenate line1, line2, and line3 to form the complete street address
        let address = billing
            .address
            .as_ref()
            .and_then(|a| get_address_line(&a.line1, &a.line2, &a.line3));

        BillTo {
            first_name,
            last_name,
            address,
            city: billing.address.as_ref().and_then(|a| a.city.clone()),
            state: billing.address.as_ref().and_then(|a| a.state.clone()),
            zip: billing.address.as_ref().and_then(|a| a.zip.clone()),
            country: billing
                .address
                .as_ref()
                .and_then(|a| a.country)
                .and_then(|api_country| {
                    enums::CountryAlpha2::from_str(&api_country.to_string()).ok()
                }),
        }
    });

    let customer_details = item
        .router_data
        .request
        .customer_id
        .as_ref()
        .filter(|_| {
            !item
                .router_data
                .request
                .is_customer_initiated_mandate_payment()
        })
        .and_then(|customer| {
            let customer_id = customer.get_string_repr();
            (customer_id.len() <= MAX_ID_LENGTH).then_some(CustomerDetails {
                id: customer_id.to_string(),
                email: item.router_data.request.get_optional_email(),
            })
        });

    // Check if we should create a profile for future mandate usage
    let profile = item
        .router_data
        .request
        .is_customer_initiated_mandate_payment()
        .then(|| {
            ProfileDetails::CreateProfileDetails(CreateProfileDetails {
                create_profile: true,
                customer_profile_id: item
                    .router_data
                    .resource_common_data
                    .connector_customer
                    .as_ref()
                    .map(|cid| Secret::new(cid.to_string())),
            })
        });

    Ok(AuthorizedotnetTransactionRequest {
        transaction_type,
        amount: Some(
            item.connector
                .amount_converter
                .convert(
                    item.router_data.request.minor_amount,
                    item.router_data.request.currency,
                )
                .change_context(IntegrationError::AmountConversionFailed {
                    context: Default::default(),
                })
                .attach_printable("Failed to convert payment amount for authorize transaction")?,
        ),
        currency_code: Some(currency),
        payment: Some(payment_details),
        profile,
        order: Some(order),
        customer: customer_details,
        bill_to,
        user_fields,
        processing_options: None,
        subsequent_auth_information: None,
        ref_trans_id: None,
    })
}

/// Helper function to get payment details from wallet data
fn get_wallet_payment_details<T: PaymentMethodDataTypes>(
    wallet_data: &WalletData,
    return_url: &Option<String>,
) -> Result<PaymentDetails<T>, Error> {
    match wallet_data {
        WalletData::GooglePay(_) => Ok(PaymentDetails::OpaqueData(WalletDetails {
            data_descriptor: WalletMethod::Googlepay,
            data_value: Secret::new(wallet_data.get_encoded_wallet_token().change_context(
                IntegrationError::InvalidWallet {
                    context: Default::default(),
                },
            )?),
        })),
        WalletData::ApplePay(applepay_token) => {
            let apple_pay_encrypted_data = applepay_token
                .payment_data
                .get_encrypted_apple_pay_payment_data_mandatory()
                .change_context(IntegrationError::MissingRequiredField {
                    field_name: "Apple pay encrypted data",
                    context: Default::default(),
                })?;
            Ok(PaymentDetails::OpaqueData(WalletDetails {
                data_descriptor: WalletMethod::Applepay,
                data_value: Secret::new(apple_pay_encrypted_data.clone()),
            }))
        }
        WalletData::PaypalRedirect(_) => Ok(PaymentDetails::PayPal(PayPalDetails {
            success_url: return_url.clone(),
            cancel_url: return_url.clone(),
        })),
        WalletData::AliPayQr(_)
        | WalletData::AliPayRedirect(_)
        | WalletData::AliPayHkRedirect(_)
        | WalletData::AmazonPayRedirect(_)
        | WalletData::BluecodeRedirect {}
        | WalletData::MomoRedirect(_)
        | WalletData::KakaoPayRedirect(_)
        | WalletData::GoPayRedirect(_)
        | WalletData::GcashRedirect(_)
        | WalletData::ApplePayRedirect(_)
        | WalletData::ApplePayThirdPartySdk(_)
        | WalletData::DanaRedirect {}
        | WalletData::GooglePayRedirect(_)
        | WalletData::GooglePayThirdPartySdk(_)
        | WalletData::MbWayRedirect(_)
        | WalletData::MobilePayRedirect(_)
        | WalletData::PaypalSdk(_)
        | WalletData::Paze(_)
        | WalletData::SamsungPay(_)
        | WalletData::TwintRedirect {}
        | WalletData::VippsRedirect {}
        | WalletData::TouchNGoRedirect(_)
        | WalletData::WeChatPayRedirect(_)
        | WalletData::WeChatPayQr(_)
        | WalletData::CashappQr(_)
        | WalletData::SwishQr(_)
        | WalletData::Mifinity(_)
        | WalletData::RevolutPay(_)
        | WalletData::MbWay(_)
        | WalletData::Satispay(_)
        | WalletData::Wero(_) => Err(error_stack::report!(IntegrationError::not_implemented(
            format!(
                "Wallet payment method not supported for authorizedotnet: {:?}",
                wallet_data
            )
        ))),
    }
}

// RepeatPayment request structures
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetRepeatPaymentRequest {
    create_transaction_request: CreateRepeatPaymentRequest,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRepeatPaymentRequest {
    merchant_authentication: AuthorizedotnetAuthType,
    ref_id: Option<String>,
    transaction_request: AuthorizedotnetRepeatPaymentTransactionRequest,
}

#[skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetRepeatPaymentTransactionRequest {
    transaction_type: TransactionType,
    amount: FloatMajorUnit,
    currency_code: api_enums::Currency,
    profile: Option<ProfileDetails>,
    order: Option<Order>,
    customer: Option<CustomerDetails>,
    user_fields: Option<UserFields>,
    processing_options: Option<ProcessingOptions>,
    subsequent_auth_information: Option<SubsequentAuthInformation>,
}

// Implementation for RepeatPayment request conversion
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AuthorizedotnetRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AuthorizedotnetRepeatPaymentRequest
{
    type Error = Error;
    fn try_from(
        item: AuthorizedotnetRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let merchant_authentication =
            AuthorizedotnetAuthType::try_from(&item.router_data.connector_config)?;

        let currency = item.router_data.request.currency;

        // Handle different mandate reference types with appropriate MIT structures
        let (profile, processing_options, subsequent_auth_information) =
            match &item.router_data.request.mandate_reference {
                // Case 1: Mandate-based MIT (using stored customer profile)
                MandateReferenceId::ConnectorMandateId(connector_mandate_ref) => {
                    let mandate_id = connector_mandate_ref
                        .get_connector_mandate_id()
                        .ok_or_else(|| {
                            error_stack::report!(IntegrationError::MissingRequiredField {
                                field_name: "connector_mandate_id",
                                context: Default::default()
                            })
                        })?;

                    // Parse mandate_id to extract customer_profile_id and payment_profile_id
                    let profile = mandate_id
                        .split_once('-')
                        .map(|(customer_profile_id, payment_profile_id)| {
                            ProfileDetails::CustomerProfileDetails(CustomerProfileDetails {
                                customer_profile_id: Secret::from(customer_profile_id.to_string()),
                                payment_profile: PaymentProfileDetails {
                                    payment_profile_id: Secret::from(
                                        payment_profile_id.to_string(),
                                    ),
                                },
                            })
                        })
                        .ok_or_else(|| {
                            error_stack::report!(IntegrationError::MissingRequiredField {
                                field_name: "valid mandate_id format (should contain '-')",
                                context: Default::default()
                            })
                        })?;

                    (
                        Some(profile),
                        Some(ProcessingOptions {
                            is_subsequent_auth: true,
                        }),
                        None, // No network transaction ID for mandate-based flow
                    )
                }

                // Case 2: Network mandate ID flow (PG agnostic with network trans ID)
                MandateReferenceId::NetworkMandateId(network_trans_id) => (
                    None, // No customer profile for network transaction flow
                    Some(ProcessingOptions {
                        is_subsequent_auth: true,
                    }),
                    Some(SubsequentAuthInformation {
                        original_network_trans_id: Secret::new(network_trans_id.clone()),
                        reason: Reason::Resubmission,
                    }),
                ),

                // Case 3: Network token with NTI - NOT SUPPORTED (same as Hyperswitch)
                MandateReferenceId::NetworkTokenWithNTI(_) => {
                    return Err(error_stack::report!(IntegrationError::not_implemented(
                        "Network token with NTI not supported for authorizedotnet".to_string(),
                    )))
                }
            };

        // Order description should be connector_request_reference_id (same as Hyperswitch)
        let order_description = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        // Get invoice number (random string if > MAX_ID_LENGTH or None)
        let invoice_number =
            get_invoice_number_or_random(item.router_data.request.merchant_order_id.clone());

        let order = Order {
            invoice_number,
            description: order_description,
        };

        // Extract user fields from metadata (RepeatPayment metadata is HashMap, needs conversion to Value)
        let user_fields = metadata_to_user_fields(
            item.router_data
                .request
                .metadata
                .clone()
                .map(serde_json::to_value)
                .transpose()
                .change_context(IntegrationError::RequestEncodingFailed {
                    context: Default::default(),
                })?,
            false, // Already serialized above
        )?;

        // ref_id should be connector_request_reference_id with MAX_ID_LENGTH check (same as Authorize flow)
        let ref_id = Some(
            item.router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        )
        .filter(|id| id.len() <= MAX_ID_LENGTH);

        let customer_id_string = validate_customer_id_length(
            item.router_data
                .resource_common_data
                .customer_id
                .as_ref()
                .map(|cid| cid.get_string_repr().to_owned()),
        );

        let customer_details = customer_id_string.map(|cid| CustomerDetails {
            id: cid,
            email: item.router_data.request.email.clone(),
        });

        let transaction_type = match item.router_data.request.capture_method {
            Some(enums::CaptureMethod::Manual) => TransactionType::AuthOnlyTransaction,
            Some(enums::CaptureMethod::Automatic)
            | None
            | Some(enums::CaptureMethod::SequentialAutomatic) => {
                TransactionType::AuthCaptureTransaction
            }
            Some(_) => {
                return Err(error_stack::report!(IntegrationError::NotSupported {
                    message: "Capture method not supported".to_string(),
                    connector: "authorizedotnet",
                    context: Default::default()
                }))
            }
        };

        let transaction_request = AuthorizedotnetRepeatPaymentTransactionRequest {
            transaction_type,
            amount: item
                .connector
                .amount_converter
                .convert(
                    item.router_data.request.minor_amount,
                    item.router_data.request.currency,
                )
                .change_context(IntegrationError::AmountConversionFailed {
                    context: Default::default(),
                })
                .attach_printable(
                    "Failed to convert payment amount for repeat payment transaction",
                )?,
            currency_code: currency,
            profile,
            order: Some(order),
            customer: customer_details,
            user_fields,
            processing_options,
            subsequent_auth_information,
        };

        Ok(Self {
            create_transaction_request: CreateRepeatPaymentRequest {
                merchant_authentication,
                ref_id,
                transaction_request,
            },
        })
    }
}

#[skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetCaptureTransactionInternal {
    // Specific transaction details for Capture
    transaction_type: TransactionType,
    amount: FloatMajorUnit,
    ref_trans_id: String,
}

#[skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCaptureTransactionRequest {
    // Used by Capture Flow, wraps specific capture transaction details
    merchant_authentication: AuthorizedotnetAuthType,
    transaction_request: AuthorizedotnetCaptureTransactionInternal,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetCaptureRequest {
    // Top-level wrapper for Capture Flow
    create_transaction_request: CreateCaptureTransactionRequest,
}

// New direct implementation for capture without relying on the reference version
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AuthorizedotnetRouterData<
            RouterDataV2<
                domain_types::connector_flow::Capture,
                PaymentFlowData,
                PaymentsCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AuthorizedotnetCaptureRequest
{
    type Error = Error;
    fn try_from(
        item: AuthorizedotnetRouterData<
            RouterDataV2<
                domain_types::connector_flow::Capture,
                PaymentFlowData,
                PaymentsCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        let original_connector_txn_id = match &router_data.request.connector_transaction_id {
            ResponseId::ConnectorTransactionId(id) => id.clone(),
            _ => {
                return Err(error_stack::report!(
                    HsInterfacesConnectorRequestError::MissingRequiredField {
                        field_name: "connector_transaction_id",
                        context: Default::default()
                    }
                ));
            }
        };

        let transaction_request_payload = AuthorizedotnetCaptureTransactionInternal {
            transaction_type: TransactionType::PriorAuthCaptureTransaction,
            amount: item
                .connector
                .amount_converter
                .convert(
                    item.router_data.request.minor_amount_to_capture,
                    item.router_data.request.currency,
                )
                .change_context(IntegrationError::AmountConversionFailed {
                    context: Default::default(),
                })
                .attach_printable("Failed to convert capture amount for capture transaction")?,
            ref_trans_id: original_connector_txn_id,
        };

        let merchant_authentication =
            AuthorizedotnetAuthType::try_from(&item.router_data.connector_config)?;

        let create_transaction_request_payload = CreateCaptureTransactionRequest {
            merchant_authentication,
            transaction_request: transaction_request_payload,
        };

        Ok(Self {
            create_transaction_request: create_transaction_request_payload,
        })
    }
}

#[skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetTransactionVoidDetails {
    // Specific transaction details for Void
    transaction_type: TransactionType,
    ref_trans_id: String,
    amount: Option<f64>,
}

#[skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTransactionVoidRequest {
    // Used by Void Flow, wraps specific void transaction details
    merchant_authentication: AuthorizedotnetAuthType,
    ref_id: Option<String>,
    transaction_request: AuthorizedotnetTransactionVoidDetails,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetVoidRequest {
    // Top-level wrapper for Void Flow
    create_transaction_request: CreateTransactionVoidRequest,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetAuthType {
    name: Secret<String>,
    transaction_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for AuthorizedotnetAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        if let ConnectorSpecificConfig::Authorizedotnet {
            name,
            transaction_key,
            ..
        } = auth_type
        {
            Ok(Self {
                name: name.to_owned(),
                transaction_key: transaction_key.to_owned(),
            })
        } else {
            Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            })?
        }
    }
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AuthorizedotnetRouterData<
            RouterDataV2<
                domain_types::connector_flow::Void,
                PaymentFlowData,
                PaymentVoidData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AuthorizedotnetVoidRequest
{
    type Error = Error;

    fn try_from(
        item: AuthorizedotnetRouterData<
            RouterDataV2<
                domain_types::connector_flow::Void,
                PaymentFlowData,
                PaymentVoidData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract transaction ID from the connector_transaction_id string
        // This transaction ID comes from the authorization response
        let transaction_id = match router_data.request.connector_transaction_id.as_str() {
            "" => {
                return Err(error_stack::report!(
                    HsInterfacesConnectorRequestError::MissingRequiredField {
                        field_name: "connector_transaction_id",
                        context: Default::default()
                    }
                ));
            }
            id => id.to_string(),
        };

        let ref_id = Some(
            item.router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        )
        .filter(|id| id.len() <= MAX_ID_LENGTH);

        let transaction_void_details = AuthorizedotnetTransactionVoidDetails {
            transaction_type: TransactionType::VoidTransaction,
            ref_trans_id: transaction_id,
            amount: None,
        };

        let merchant_authentication =
            AuthorizedotnetAuthType::try_from(&router_data.connector_config)?;

        let create_transaction_void_request = CreateTransactionVoidRequest {
            merchant_authentication,
            ref_id,
            transaction_request: transaction_void_details,
        };

        Ok(Self {
            create_transaction_request: create_transaction_void_request,
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionDetails {
    pub merchant_authentication: MerchantAuthentication,
    #[serde(rename = "transId")]
    pub transaction_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetCreateSyncRequest {
    pub get_transaction_details_request: TransactionDetails,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetRSyncRequest {
    pub get_transaction_details_request: TransactionDetails,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AuthorizedotnetRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for AuthorizedotnetCreateSyncRequest
{
    type Error = Error;

    fn try_from(
        item: AuthorizedotnetRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Extract connector_transaction_id from the request
        let connector_transaction_id = match &item.router_data.request.connector_transaction_id {
            ResponseId::ConnectorTransactionId(id) => id.clone(),
            _ => {
                return Err(error_stack::report!(
                    HsInterfacesConnectorRequestError::MissingRequiredField {
                        field_name: "connector_transaction_id",
                        context: Default::default()
                    }
                ))
            }
        };

        let merchant_authentication =
            MerchantAuthentication::try_from(&item.router_data.connector_config)?;

        let payload = Self {
            get_transaction_details_request: TransactionDetails {
                merchant_authentication,
                transaction_id: Some(connector_transaction_id),
            },
        };
        Ok(payload)
    }
}

// Implementation for the RSync flow to support refund synchronization
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AuthorizedotnetRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for AuthorizedotnetRSyncRequest
{
    type Error = Error;

    fn try_from(
        item: AuthorizedotnetRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Extract connector_refund_id from the request
        let connector_refund_id = if !item.router_data.request.connector_refund_id.is_empty() {
            item.router_data.request.connector_refund_id.clone()
        } else {
            return Err(error_stack::report!(
                HsInterfacesConnectorRequestError::MissingRequiredField {
                    field_name: "connector_refund_id",
                    context: Default::default()
                }
            ));
        };

        let merchant_authentication =
            MerchantAuthentication::try_from(&item.router_data.connector_config)?;

        let payload = Self {
            get_transaction_details_request: TransactionDetails {
                merchant_authentication,
                transaction_id: Some(connector_refund_id),
            },
        };
        Ok(payload)
    }
}

// Refund-related structs and implementations
#[skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetRefundCardDetails {
    card_number: Secret<String>,
    expiration_date: Secret<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
enum AuthorizedotnetRefundPaymentDetails<T: PaymentMethodDataTypes> {
    CreditCard(CreditCardDetails<T>),
}

#[skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetRefundTransactionDetails {
    transaction_type: TransactionType,
    amount: FloatMajorUnit,
    currency_code: api_enums::Currency,
    payment: RefundPaymentDetails,
    ref_trans_id: String,
}

#[skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetRefundRequest {
    create_transaction_request: CreateTransactionRefundRequest,
}

#[skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTransactionRefundRequest {
    merchant_authentication: AuthorizedotnetAuthType,
    transaction_request: AuthorizedotnetRefundTransactionDetails,
}

#[skip_serializing_none]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefundPaymentDetails {
    credit_card: CreditCardInfo,
}

#[skip_serializing_none]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreditCardInfo {
    card_number: String,
    expiration_date: String,
}

// Unified generic implementation for all payment method types
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AuthorizedotnetRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for AuthorizedotnetRefundRequest
{
    type Error = Error;

    fn try_from(
        item: AuthorizedotnetRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let merchant_authentication =
            AuthorizedotnetAuthType::try_from(&item.router_data.connector_config)?;

        // Extract payment details from metadata using unified helper
        let connector_metadata_secret = item
            .router_data
            .request
            .connector_feature_data
            .clone()
            .ok_or_else(|| {
                error_stack::report!(HsInterfacesConnectorRequestError::MissingRequiredField {
                    field_name: "connector_feature_data",
                    context: Default::default()
                })
            })?;
        let payment = get_refund_credit_card_payment(&Some(connector_metadata_secret))?;

        // Build the refund transaction request
        let transaction_request = AuthorizedotnetRefundTransactionDetails {
            transaction_type: TransactionType::RefundTransaction,
            amount: item
                .connector
                .amount_converter
                .convert(
                    item.router_data.request.minor_refund_amount,
                    item.router_data.request.currency,
                )
                .change_context(IntegrationError::AmountConversionFailed {
                    context: Default::default(),
                })
                .attach_printable("Failed to convert refund amount for refund transaction")?,
            currency_code: item.router_data.request.currency,
            payment,
            ref_trans_id: item.router_data.request.connector_transaction_id.clone(),
        };

        Ok(Self {
            create_transaction_request: CreateTransactionRefundRequest {
                merchant_authentication,
                transaction_request,
            },
        })
    }
}

// Refund request struct is fully implemented above

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum TransactionResponse {
    AuthorizedotnetTransactionResponse(Box<AuthorizedotnetTransactionResponse>),
    AuthorizedotnetTransactionResponseError(Box<AuthorizedotnetTransactionResponseError>),
}

// Base transaction response - used internally
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionProfileInfo {
    customer_profile_id: String,
    customer_payment_profile_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetTransactionResponse {
    response_code: AuthorizedotnetPaymentStatus,
    #[serde(rename = "transId")]
    transaction_id: String,
    #[serde(default)]
    transaction_status: Option<String>,
    #[serde(default)]
    network_trans_id: Option<Secret<String>>,
    #[serde(default)]
    pub(super) account_number: Option<Secret<String>>,
    #[serde(default)]
    pub(super) account_type: Option<Secret<String>>,
    #[serde(default)]
    pub(super) errors: Option<Vec<ErrorMessage>>,
    #[serde(default)]
    secure_acceptance: Option<SecureAcceptance>,
    #[serde(default)]
    profile: Option<TransactionProfileInfo>,
    #[serde(default, rename = "avsResultCode")]
    avs_result_code: Option<String>,
}

// Create flow-specific response types
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthorizedotnetAuthorizeResponse(pub AuthorizedotnetPaymentsResponse);

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthorizedotnetCaptureResponse(pub AuthorizedotnetPaymentsResponse);

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthorizedotnetVoidResponse(pub AuthorizedotnetPaymentsResponse);

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthorizedotnetRepeatPaymentResponse(pub AuthorizedotnetPaymentsResponse);

// Helper function to get AVS response description based on the code
fn get_avs_response_description(code: &str) -> Option<&'static str> {
    match code {
        "A" => Some("The street address matched, but the postal code did not."),
        "B" => Some("No address information was provided."),
        "E" => Some("The AVS check returned an error."),
        "G" => Some("The card was issued by a bank outside the U.S. and does not support AVS."),
        "N" => Some("Neither the street address nor postal code matched."),
        "P" => Some("AVS is not applicable for this transaction."),
        "R" => Some("Retry — AVS was unavailable or timed out."),
        "S" => Some("AVS is not supported by card issuer."),
        "U" => Some("Address information is unavailable."),
        "W" => Some("The US ZIP+4 code matches, but the street address does not."),
        "X" => Some("Both the street address and the US ZIP+4 code matched."),
        "Y" => Some("The street address and postal code matched."),
        "Z" => Some("The postal code matched, but the street address did not."),
        _ => None,
    }
}

// Convert transaction response to additional payment method connector response
fn convert_to_additional_payment_method_connector_response(
    transaction_response: &AuthorizedotnetTransactionResponse,
) -> Option<domain_types::router_data::AdditionalPaymentMethodConnectorResponse> {
    match transaction_response.avs_result_code.as_deref() {
        Some("P") | None => None,
        Some(code) => {
            let description = get_avs_response_description(code);
            let payment_checks = serde_json::json!({
                "avs_result_code": code,
                "description": description
            });

            Some(
                domain_types::router_data::AdditionalPaymentMethodConnectorResponse::Card {
                    authentication_data: None,
                    payment_checks: Some(payment_checks),
                    card_network: None,
                    domestic_network: None,
                    auth_code: None,
                },
            )
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefundResponse {
    response_code: AuthorizedotnetRefundStatus,
    #[serde(rename = "transId")]
    transaction_id: String,
    network_trans_id: Option<Secret<String>>,
    pub account_number: Option<Secret<String>>,
    pub errors: Option<Vec<ErrorMessage>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetRefundResponse {
    pub transaction_response: RefundResponse,
    pub messages: ResponseMessages,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetCreateConnectorCustomerRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    create_customer_profile_request: AuthorizedotnetZeroMandateRequest<T>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetZeroMandateRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    merchant_authentication: AuthorizedotnetAuthType,
    profile: Profile<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    validation_mode: Option<ValidationMode>,
}

// ShipToList for customer shipping address
#[skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ShipToList {
    first_name: Option<Secret<String>>,
    last_name: Option<Secret<String>>,
    address: Option<Secret<String>>,
    city: Option<Secret<String>>,
    state: Option<Secret<String>>,
    zip: Option<Secret<String>>,
    country: Option<common_enums::CountryAlpha2>,
    phone_number: Option<Secret<String>>,
}

#[skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Profile<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> {
    merchant_customer_id: Option<String>,
    description: Option<String>,
    email: Option<String>,
    payment_profiles: Option<Vec<PaymentProfiles<T>>>,
    ship_to_list: Option<Vec<ShipToList>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PaymentProfiles<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    customer_type: CustomerType,
    payment: PaymentDetails<T>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CustomerType {
    Individual,
    Business,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ValidationMode {
    // testMode performs a Luhn mod-10 check on the card number, without further validation at connector.
    TestMode,
    // liveMode submits a zero-dollar or one-cent transaction (depending on card type and processor support) to confirm that the card number belongs to an active credit or debit account.
    LiveMode,
}

// SetupMandate request structures - adds payment profile to existing customer
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetSetupMandateRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    create_customer_payment_profile_request: AuthorizedotnetPaymentProfileRequest<T>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetPaymentProfileRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    merchant_authentication: AuthorizedotnetAuthType,
    customer_profile_id: Secret<String>,
    payment_profile: PaymentProfile<T>,
    validation_mode: ValidationMode,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentProfile<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(skip_serializing_if = "Option::is_none")]
    bill_to: Option<BillTo>,
    payment: PaymentDetails<T>,
}

// SetupMandate response structure
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetSetupMandateResponse {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub customer_payment_profile_id_list: Vec<String>,
    pub customer_profile_id: Option<String>,
    #[serde(rename = "customerPaymentProfileId")]
    pub customer_payment_profile_id: Option<String>,
    pub validation_direct_response_list: Option<Vec<Secret<String>>>,
    pub messages: ResponseMessages,
}

// PSync response wrapper - Using direct structure instead of wrapping AuthorizedotnetPaymentsResponse
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthorizedotnetPSyncResponse {
    pub transaction: Option<SyncTransactionResponse>,
    pub messages: ResponseMessages,
}

// Implement From/TryFrom for the response types
impl From<AuthorizedotnetPaymentsResponse> for AuthorizedotnetAuthorizeResponse {
    fn from(response: AuthorizedotnetPaymentsResponse) -> Self {
        Self(response)
    }
}

impl From<AuthorizedotnetPaymentsResponse> for AuthorizedotnetCaptureResponse {
    fn from(response: AuthorizedotnetPaymentsResponse) -> Self {
        Self(response)
    }
}

impl From<AuthorizedotnetPaymentsResponse> for AuthorizedotnetVoidResponse {
    fn from(response: AuthorizedotnetPaymentsResponse) -> Self {
        Self(response)
    }
}

impl From<AuthorizedotnetPaymentsResponse> for AuthorizedotnetRepeatPaymentResponse {
    fn from(response: AuthorizedotnetPaymentsResponse) -> Self {
        Self(response)
    }
}

// We no longer need the From implementation for AuthorizedotnetPSyncResponse since we're using the direct structure

// TryFrom implementations for the router data conversions

impl<
        F,
        T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize + Serialize,
    > TryFrom<ResponseRouterData<AuthorizedotnetAuthorizeResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        value: ResponseRouterData<AuthorizedotnetAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        // Use our helper function to convert the response
        let (status, response_result, connector_response_data) =
            convert_to_payments_response_data_or_error(
                &response.0,
                http_code,
                Operation::Authorize,
                router_data.request.capture_method,
                router_data
                    .resource_common_data
                    .raw_connector_response
                    .clone(),
            );

        // Create a new RouterDataV2 with updated fields
        let mut new_router_data = router_data;

        // Update the status and connector_response in resource_common_data
        let mut resource_common_data = new_router_data.resource_common_data.clone();
        resource_common_data.status = status;
        resource_common_data.connector_response = connector_response_data;
        new_router_data.resource_common_data = resource_common_data;

        // Set the response
        new_router_data.response = response_result;

        Ok(new_router_data)
    }
}

impl<F> TryFrom<ResponseRouterData<AuthorizedotnetCaptureResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        value: ResponseRouterData<AuthorizedotnetCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        // Use our helper function to convert the response
        let (status, response_result, connector_response_data) =
            convert_to_payments_response_data_or_error(
                &response.0,
                http_code,
                Operation::Capture,
                None,
                router_data
                    .resource_common_data
                    .raw_connector_response
                    .clone(),
            );

        // Create a new RouterDataV2 with updated fields
        let mut new_router_data = router_data;

        // Update the status and connector_response in resource_common_data
        let mut resource_common_data = new_router_data.resource_common_data.clone();
        resource_common_data.status = status;
        resource_common_data.connector_response = connector_response_data;
        new_router_data.resource_common_data = resource_common_data;

        // Set the response
        new_router_data.response = response_result;

        Ok(new_router_data)
    }
}

impl<F> TryFrom<ResponseRouterData<AuthorizedotnetVoidResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        value: ResponseRouterData<AuthorizedotnetVoidResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;
        // Use our helper function to convert the response
        let (status, response_result, connector_response_data) =
            convert_to_payments_response_data_or_error(
                &response.0,
                http_code,
                Operation::Void,
                None,
                router_data
                    .resource_common_data
                    .raw_connector_response
                    .clone(),
            );

        // Create a new RouterDataV2 with updated fields
        let mut new_router_data = router_data;

        // Update the status and connector_response in resource_common_data
        let mut resource_common_data = new_router_data.resource_common_data.clone();
        resource_common_data.status = status;
        resource_common_data.connector_response = connector_response_data;
        new_router_data.resource_common_data = resource_common_data;

        // Set the response
        new_router_data.response = response_result;

        Ok(new_router_data)
    }
}

impl<
        F,
        T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize + Serialize,
    > TryFrom<ResponseRouterData<AuthorizedotnetRepeatPaymentResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        value: ResponseRouterData<AuthorizedotnetRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        // Dedicated RepeatPayment response handling (matching Hyperswitch)
        let status = get_hs_status(
            &response.0,
            http_code,
            Operation::Authorize,
            router_data.request.capture_method,
        );

        // Extract connector response data
        let connector_response_data = match &response.0.transaction_response {
            Some(TransactionResponse::AuthorizedotnetTransactionResponse(trans_res)) => {
                convert_to_additional_payment_method_connector_response(trans_res)
                    .map(domain_types::router_data::ConnectorResponseData::with_additional_payment_method_data)
            }
            _ => None
};

        let response_result = match &response.0.transaction_response {
            Some(TransactionResponse::AuthorizedotnetTransactionResponse(transaction_response)) => {
                // Check for errors in the response
                let error = transaction_response.errors.as_ref().and_then(|errors| {
                    errors.first().map(|error| ErrorResponse {
                        code: error.error_code.clone(),
                        message: error.error_text.clone(),
                        reason: Some(error.error_text.clone()),
                        status_code: http_code,
                        attempt_status: Some(status),
                        connector_transaction_id: Some(transaction_response.transaction_id.clone()),
                        network_advice_code: None,
                        network_decline_code: None,
                        network_error_message: None,
                    })
                });

                // Extract mandate_reference from transaction_response.profile (RepeatPayment returns profile info)
                let mandate_reference =
                    transaction_response
                        .profile
                        .as_ref()
                        .map(|profile| MandateReference {
                            connector_mandate_id: Some(format!(
                                "{}-{}",
                                profile.customer_profile_id, profile.customer_payment_profile_id
                            )),
                            payment_method_id: None,
                            connector_mandate_request_reference_id: None,
                        });

                // Build connector_metadata from account_number
                let connector_metadata = build_connector_metadata(transaction_response);

                match error {
                    Some(err) => Err(err),
                    None => Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(
                            transaction_response.transaction_id.clone(),
                        ),
                        redirection_data: None,
                        mandate_reference: mandate_reference.map(Box::new),
                        connector_metadata,
                        network_txn_id: transaction_response
                            .network_trans_id
                            .as_ref()
                            .map(|s| s.peek().clone()),
                        connector_response_reference_id: Some(
                            transaction_response.transaction_id.clone(),
                        ),
                        incremental_authorization_allowed: None,
                        status_code: http_code,
                    }),
                }
            }
            Some(TransactionResponse::AuthorizedotnetTransactionResponseError(_)) | None => {
                let (error_code, error_message) = extract_error_details(&response.0, None);
                Err(create_error_response(
                    http_code,
                    error_code,
                    error_message,
                    status,
                    None,
                    router_data
                        .resource_common_data
                        .raw_connector_response
                        .clone(),
                ))
            }
        };

        // Create a new RouterDataV2 with updated fields
        let mut new_router_data = router_data;

        // Update the status and connector_response in resource_common_data
        let mut resource_common_data = new_router_data.resource_common_data.clone();
        resource_common_data.status = status;
        resource_common_data.connector_response = connector_response_data;
        new_router_data.resource_common_data = resource_common_data;

        // Set the response
        new_router_data.response = response_result;

        Ok(new_router_data)
    }
}

impl TryFrom<ResponseRouterData<AuthorizedotnetRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        value: ResponseRouterData<AuthorizedotnetRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        let transaction_response = &response.transaction_response;
        let refund_status = RefundStatus::from(transaction_response.response_code.clone());

        let error = transaction_response.errors.clone().and_then(|errors| {
            errors.first().map(|error| ErrorResponse {
                code: error.error_code.clone(),
                message: error.error_text.clone(),
                reason: Some(error.error_text.clone()),
                status_code: http_code,
                attempt_status: Some(AttemptStatus::Failure),
                connector_transaction_id: Some(transaction_response.transaction_id.clone()),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            })
        });

        // Create a new RouterDataV2 with updated fields
        let mut new_router_data = router_data;

        // Update the status in resource_common_data
        let mut resource_common_data = new_router_data.resource_common_data.clone();
        resource_common_data.status = refund_status;
        new_router_data.resource_common_data = resource_common_data;

        // Set the response based on whether there was an error
        new_router_data.response = match error {
            Some(err) => Err(err),
            None => Ok(RefundsResponseData {
                connector_refund_id: transaction_response.transaction_id.clone(),
                refund_status,
                status_code: http_code,
            }),
        };

        Ok(new_router_data)
    }
}

// Implementation for PSync flow
impl<F> TryFrom<ResponseRouterData<AuthorizedotnetPSyncResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        value: ResponseRouterData<AuthorizedotnetPSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        // No need to transform the response since we're using the direct structure
        // Use the clean approach with the From trait implementation
        match response.transaction {
            Some(transaction) => {
                let payment_status = AttemptStatus::from(transaction.transaction_status);

                // Create a new RouterDataV2 with updated fields
                let mut new_router_data = router_data;

                // Update the status in resource_common_data
                let mut resource_common_data = new_router_data.resource_common_data.clone();
                resource_common_data.status = payment_status;
                new_router_data.resource_common_data = resource_common_data;

                // Set the response
                new_router_data.response = Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        transaction.transaction_id.clone(),
                    ),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    connector_response_reference_id: Some(transaction.transaction_id.clone()),
                    incremental_authorization_allowed: None,
                    status_code: http_code,
                });

                Ok(new_router_data)
            }
            None => {
                // Handle missing transaction response
                let status = match response.messages.result_code {
                    ResultCode::Error => AttemptStatus::Failure,
                    ResultCode::Ok => AttemptStatus::Pending,
                };

                let error_response = ErrorResponse {
                    status_code: http_code,
                    code: response
                        .messages
                        .message
                        .first()
                        .map(|m| m.code.clone())
                        .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
                    message: response
                        .messages
                        .message
                        .first()
                        .map(|m| m.text.clone())
                        .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
                    reason: Some(
                        response
                            .messages
                            .message
                            .first()
                            .map(|m| m.text.clone())
                            .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
                    ),
                    attempt_status: Some(status),
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                };

                // Update router data with status and error response
                let mut new_router_data = router_data;
                let mut resource_common_data = new_router_data.resource_common_data.clone();
                resource_common_data.status = status;
                new_router_data.resource_common_data = resource_common_data;
                new_router_data.response = Err(error_response);

                Ok(new_router_data)
            }
        }
    }
}

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub enum AuthorizedotnetPaymentStatus {
    #[serde(rename = "1")]
    Approved,
    #[serde(rename = "2")]
    Declined,
    #[serde(rename = "3")]
    Error,
    #[serde(rename = "4")]
    #[default]
    HeldForReview,
    #[serde(rename = "5")]
    RequiresAction, // Maps to hyperswitch_common_enums::enums::AttemptStatus::AuthenticationPending
}

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub enum AuthorizedotnetRefundStatus {
    #[serde(rename = "1")]
    Approved,
    #[serde(rename = "2")]
    Declined,
    #[serde(rename = "3")]
    Error,
    #[serde(rename = "4")]
    #[default]
    HeldForReview,
}

/// Helper function to extract error code and message from response
fn extract_error_details(
    response: &AuthorizedotnetPaymentsResponse,
    trans_res: Option<&AuthorizedotnetTransactionResponse>,
) -> (String, String) {
    let error_code = trans_res
        .and_then(|tr| {
            tr.errors
                .as_ref()
                .and_then(|e| e.first().map(|e| e.error_code.clone()))
        })
        .or_else(|| response.messages.message.first().map(|m| m.code.clone()))
        .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string());

    let error_message = trans_res
        .and_then(|tr| {
            tr.errors
                .as_ref()
                .and_then(|e| e.first().map(|e| e.error_text.clone()))
        })
        .or_else(|| response.messages.message.first().map(|m| m.text.clone()))
        .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string());

    (error_code, error_message)
}

/// Helper function to create error response
fn create_error_response(
    http_status_code: u16,
    error_code: String,
    error_message: String,
    status: AttemptStatus,
    connector_transaction_id: Option<String>,
    _raw_connector_response: Option<Secret<String>>,
) -> ErrorResponse {
    ErrorResponse {
        status_code: http_status_code,
        code: error_code,
        message: error_message.clone(),
        reason: Some(error_message),
        attempt_status: Some(status),
        connector_transaction_id,
        network_decline_code: None,
        network_advice_code: None,
        network_error_message: None,
    }
}

impl From<AuthorizedotnetRefundStatus> for RefundStatus {
    fn from(item: AuthorizedotnetRefundStatus) -> Self {
        match item {
            AuthorizedotnetRefundStatus::Declined | AuthorizedotnetRefundStatus::Error => {
                Self::Failure
            }
            AuthorizedotnetRefundStatus::Approved | AuthorizedotnetRefundStatus::HeldForReview => {
                Self::Pending
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorMessage {
    pub error_code: String,
    pub error_text: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct AuthorizedotnetTransactionResponseError {
    _supplemental_data_qualification_indicator: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SecureAcceptance {
    // Define fields for SecureAcceptance if it's actually used and its structure is known
}

#[derive(Debug, Default, Clone, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseMessage {
    pub code: String,
    pub text: String,
}

#[derive(Debug, Default, Clone, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum ResultCode {
    #[default]
    Ok,
    Error,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseMessages {
    result_code: ResultCode,
    pub message: Vec<ResponseMessage>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetPaymentsResponse {
    pub transaction_response: Option<TransactionResponse>,
    pub profile_response: Option<AuthorizedotnetNonZeroMandateResponse>,
    pub messages: ResponseMessages,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetNonZeroMandateResponse {
    customer_profile_id: Option<String>,
    customer_payment_profile_id_list: Option<Vec<String>>,
    pub messages: ResponseMessages,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Operation {
    Authorize,
    Capture,
    Void,
    Refund,
}

fn get_hs_status(
    response: &AuthorizedotnetPaymentsResponse,
    _http_status_code: u16,
    operation: Operation,
    capture_method: Option<enums::CaptureMethod>,
) -> AttemptStatus {
    // Return failure immediately if result code is Error
    if response.messages.result_code == ResultCode::Error {
        return AttemptStatus::Failure;
    }

    // Handle case when transaction_response is None
    if response.transaction_response.is_none() {
        return match operation {
            Operation::Void => AttemptStatus::Voided,
            Operation::Authorize | Operation::Capture => AttemptStatus::Pending,
            Operation::Refund => AttemptStatus::Failure,
        };
    }

    // Now handle transaction_response cases
    // Safety: transaction_response is checked above to be Some
    match response.transaction_response.as_ref() {
        Some(trans_resp) => match trans_resp {
            TransactionResponse::AuthorizedotnetTransactionResponseError(_) => {
                AttemptStatus::Failure
            }
            TransactionResponse::AuthorizedotnetTransactionResponse(trans_res) => {
                match trans_res.response_code {
                    AuthorizedotnetPaymentStatus::Declined
                    | AuthorizedotnetPaymentStatus::Error => AttemptStatus::Failure,
                    AuthorizedotnetPaymentStatus::HeldForReview => AttemptStatus::Pending,
                    AuthorizedotnetPaymentStatus::RequiresAction => {
                        AttemptStatus::AuthenticationPending
                    }
                    AuthorizedotnetPaymentStatus::Approved => {
                        // For Approved status, determine specific status based on operation and capture method
                        match operation {
                            Operation::Authorize => match capture_method {
                                Some(enums::CaptureMethod::Manual) => AttemptStatus::Authorized,
                                _ => AttemptStatus::Charged, // Automatic or None defaults to Charged
                            },
                            Operation::Capture | Operation::Refund => AttemptStatus::Charged,
                            Operation::Void => AttemptStatus::Voided,
                        }
                    }
                }
            }
        },
        None => AttemptStatus::Pending,
    }
}

// Simple structs for connector_metadata (no validation, accepts masked cards like "XXXX2346")
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConnectorMetadataCreditCard {
    card_number: Secret<String>,
    expiration_date: Secret<String>,
}

#[derive(Debug, Serialize)]
struct ConnectorMetadataPayment {
    #[serde(rename = "creditCard")]
    credit_card: ConnectorMetadataCreditCard,
}

// Build connector_metadata from transaction response
// Uses simple structs without validation to handle masked card numbers like "XXXX2346"
fn build_connector_metadata(
    transaction_response: &AuthorizedotnetTransactionResponse,
) -> Option<serde_json::Value> {
    let card_number = transaction_response
        .account_number
        .as_ref()?
        .peek()
        .to_string();

    let payment = ConnectorMetadataPayment {
        credit_card: ConnectorMetadataCreditCard {
            card_number: Secret::new(card_number),
            expiration_date: Secret::new("XXXX".to_string()),
        },
    };

    serde_json::to_value(payment)
        .inspect_err(|e| {
            tracing::warn!(
                error = %e,
                "Failed to serialize connector_metadata payment"
            );
        })
        .ok()
}

type PaymentConversionResult = (
    AttemptStatus,
    Result<PaymentsResponseData, ErrorResponse>,
    Option<domain_types::router_data::ConnectorResponseData>,
);

pub fn convert_to_payments_response_data_or_error(
    response: &AuthorizedotnetPaymentsResponse,
    http_status_code: u16,
    operation: Operation,
    capture_method: Option<enums::CaptureMethod>,
    raw_connector_response: Option<Secret<String>>,
) -> PaymentConversionResult {
    let status = get_hs_status(response, http_status_code, operation, capture_method);

    let is_successful_status = matches!(
        status,
        AttemptStatus::Authorized
            | AttemptStatus::Pending
            | AttemptStatus::AuthenticationPending
            | AttemptStatus::Charged
            | AttemptStatus::Voided
    );

    // Extract connector response data from transaction response if available
    let connector_response_data = match &response.transaction_response {
        Some(TransactionResponse::AuthorizedotnetTransactionResponse(trans_res)) => {
            convert_to_additional_payment_method_connector_response(trans_res)
                .map(domain_types::router_data::ConnectorResponseData::with_additional_payment_method_data)
        }
        _ => None
};

    let response_payload_result = match &response.transaction_response {
        Some(TransactionResponse::AuthorizedotnetTransactionResponse(trans_res))
            if is_successful_status =>
        {
            let connector_metadata = build_connector_metadata(trans_res);

            // Extract mandate_reference from profile_response if available
            let mandate_reference = response.profile_response.as_ref().map(|profile_response| {
                let payment_profile_id = profile_response
                    .customer_payment_profile_id_list
                    .as_ref()
                    .and_then(|list| list.first().cloned());

                MandateReference {
                    connector_mandate_id: profile_response.customer_profile_id.as_ref().and_then(
                        |customer_profile_id| {
                            payment_profile_id.map(|payment_profile_id| {
                                format!("{customer_profile_id}-{payment_profile_id}")
                            })
                        },
                    ),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                }
            });

            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(trans_res.transaction_id.clone()),
                redirection_data: None,
                connector_metadata,
                mandate_reference: mandate_reference.map(Box::new),
                network_txn_id: trans_res
                    .network_trans_id
                    .as_ref()
                    .map(|s| s.peek().clone()),
                connector_response_reference_id: Some(trans_res.transaction_id.clone()),
                incremental_authorization_allowed: None,
                status_code: http_status_code,
            })
        }
        Some(TransactionResponse::AuthorizedotnetTransactionResponse(trans_res)) => {
            // Failure status or other non-successful statuses
            let (error_code, error_message) = extract_error_details(response, Some(trans_res));
            Err(create_error_response(
                http_status_code,
                error_code,
                error_message,
                status,
                Some(trans_res.transaction_id.clone()),
                raw_connector_response.clone(),
            ))
        }
        Some(TransactionResponse::AuthorizedotnetTransactionResponseError(_)) => {
            let (error_code, error_message) = extract_error_details(response, None);
            Err(create_error_response(
                http_status_code,
                error_code,
                error_message,
                status,
                None,
                raw_connector_response.clone(),
            ))
        }
        None if status == AttemptStatus::Voided && operation == Operation::Void => {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::NoResponseId,
                redirection_data: None,
                connector_metadata: None,
                mandate_reference: None,
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: http_status_code,
            })
        }
        None => {
            let (error_code, error_message) = extract_error_details(response, None);
            Err(create_error_response(
                http_status_code,
                error_code,
                error_message,
                status,
                None,
                raw_connector_response.clone(),
            ))
        }
    };

    (status, response_payload_result, connector_response_data)
}

// Transaction details for sync response used in PSync implementation

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SyncStatus {
    CapturedPendingSettlement,
    SettledSuccessfully,
    AuthorizedPendingCapture,
    Declined,
    Voided,
    CouldNotVoid,
    GeneralError,
    RefundSettledSuccessfully,
    RefundPendingSettlement,
    #[serde(rename = "FDSPendingReview")]
    FDSPendingReview,
    #[serde(rename = "FDSAuthorizedPendingReview")]
    FDSAuthorizedPendingReview,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncTransactionResponse {
    #[serde(rename = "transId")]
    pub transaction_id: String,
    #[serde(rename = "transactionStatus")]
    pub transaction_status: SyncStatus,
    pub response_code: Option<u8>,
    pub response_reason_code: Option<u8>,
    pub response_reason_description: Option<String>,
    pub network_trans_id: Option<String>,
    // Additional fields available but not needed for our implementation
}

impl From<SyncStatus> for AttemptStatus {
    fn from(transaction_status: SyncStatus) -> Self {
        match transaction_status {
            SyncStatus::SettledSuccessfully | SyncStatus::CapturedPendingSettlement => {
                Self::Charged
            }
            SyncStatus::AuthorizedPendingCapture => Self::Authorized,
            SyncStatus::Declined => Self::AuthenticationFailed,
            SyncStatus::Voided => Self::Voided,
            SyncStatus::CouldNotVoid => Self::VoidFailed,
            SyncStatus::GeneralError => Self::Failure,
            SyncStatus::RefundSettledSuccessfully
            | SyncStatus::RefundPendingSettlement
            | SyncStatus::FDSPendingReview
            | SyncStatus::FDSAuthorizedPendingReview => Self::Pending,
        }
    }
}

// Removing duplicate implementation

// RSync related types for Refund Sync
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RSyncStatus {
    RefundSettledSuccessfully,
    RefundPendingSettlement,
    Declined,
    GeneralError,
    Voided,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RSyncTransactionResponse {
    #[serde(rename = "transId")]
    transaction_id: String,
    transaction_status: RSyncStatus,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthorizedotnetRSyncResponse {
    transaction: Option<RSyncTransactionResponse>,
    messages: ResponseMessages,
}

impl From<RSyncStatus> for RefundStatus {
    fn from(transaction_status: RSyncStatus) -> Self {
        match transaction_status {
            RSyncStatus::RefundSettledSuccessfully => Self::Success,
            RSyncStatus::RefundPendingSettlement => Self::Pending,
            RSyncStatus::Declined | RSyncStatus::GeneralError | RSyncStatus::Voided => {
                Self::Failure
            }
        }
    }
}

impl TryFrom<ResponseRouterData<AuthorizedotnetRSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        value: ResponseRouterData<AuthorizedotnetRSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        match response.transaction {
            Some(transaction) => {
                let refund_status = RefundStatus::from(transaction.transaction_status);

                // Create a new RouterDataV2 with updated fields
                let mut new_router_data = router_data;

                // Update the status in resource_common_data
                let mut resource_common_data = new_router_data.resource_common_data.clone();
                resource_common_data.status = refund_status;
                new_router_data.resource_common_data = resource_common_data;

                // Set the response
                new_router_data.response = Ok(RefundsResponseData {
                    connector_refund_id: transaction.transaction_id,
                    refund_status,
                    status_code: http_code,
                });

                Ok(new_router_data)
            }
            None => {
                // Handle error response
                let error_response = ErrorResponse {
                    status_code: http_code,
                    code: response
                        .messages
                        .message
                        .first()
                        .map(|m| m.code.clone())
                        .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
                    message: response
                        .messages
                        .message
                        .first()
                        .map(|m| m.text.clone())
                        .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
                    reason: Some(
                        response
                            .messages
                            .message
                            .first()
                            .map(|m| m.text.clone())
                            .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
                    ),
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                };

                // Update router data with error response
                let mut new_router_data = router_data;
                let mut resource_common_data = new_router_data.resource_common_data.clone();
                resource_common_data.status = RefundStatus::Failure;
                new_router_data.resource_common_data = resource_common_data;
                new_router_data.response = Err(error_response);

                Ok(new_router_data)
            }
        }
    }
}

// SetupMandate (Zero Mandate) implementation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AuthorizedotnetRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AuthorizedotnetSetupMandateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: AuthorizedotnetRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, error_stack::Report<IntegrationError>> {
        let ccard = match &item.router_data.request.payment_method_data {
            PaymentMethodData::Card(card) => card,
            pm => {
                return Err(error_stack::report!(IntegrationError::not_implemented(
                    format!("Payment method {:?}", pm)
                )))
            }
        };

        let merchant_authentication =
            AuthorizedotnetAuthType::try_from(&item.router_data.connector_config)?;

        let validation_mode = match item.router_data.resource_common_data.test_mode {
            Some(true) | None => ValidationMode::TestMode,
            Some(false) => ValidationMode::LiveMode,
        };
        let customer_profile_id = item
            .router_data
            .resource_common_data
            .connector_customer
            .as_ref()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "connector_customer_id is missing",
                context: Default::default(),
            })?
            .clone();

        // Build billing address if present - use get_optional_billing() method
        let bill_to = item
            .router_data
            .resource_common_data
            .get_optional_billing()
            .and_then(|billing| billing.address.as_ref())
            .map(|address| BillTo {
                first_name: address.first_name.clone(),
                last_name: address.last_name.clone(),
                address: get_address_line(&address.line1, &address.line2, &address.line3),
                city: address.city.clone(),
                state: address.state.clone(),
                zip: address.zip.clone(),
                country: address.country,
            });

        // Create expiry date manually since we can't use the trait method generically
        let expiry_month = ccard.card_exp_month.peek().clone();
        let year = ccard.card_exp_year.peek().clone();
        let expiry_year = if year.len() == 2 {
            format!("20{year}")
        } else {
            year
        };
        let expiration_date = format!("{expiry_year}-{expiry_month}");

        let payment_profile = PaymentProfile {
            bill_to,
            payment: PaymentDetails::CreditCard(CreditCardDetails {
                card_number: ccard.card_number.clone(),
                expiration_date: Secret::new(expiration_date),
                card_code: Some(ccard.card_cvc.clone()),
            }),
        };

        Ok(Self {
            create_customer_payment_profile_request: AuthorizedotnetPaymentProfileRequest {
                merchant_authentication,
                customer_profile_id: Secret::new(customer_profile_id),
                payment_profile,
                validation_mode,
            },
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetCreateConnectorCustomerResponse {
    pub customer_profile_id: Option<String>,
    pub customer_payment_profile_id_list: Option<Vec<String>>,
    pub validation_direct_response_list: Option<Vec<Secret<String>>>,
    pub messages: ResponseMessages,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<AuthorizedotnetSetupMandateResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        value: ResponseRouterData<AuthorizedotnetSetupMandateResponse, Self>,
    ) -> Result<Self, error_stack::Report<ConnectorError>> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        // Get connector customer ID from resource_common_data - we need it to build mandate reference
        let connector_customer_id = router_data
            .resource_common_data
            .connector_customer
            .as_ref()
            .ok_or_else(|| {
                error_stack::report!(ConnectorError::ResponseHandlingFailed {
                    context: domain_types::errors::ResponseTransformationErrorContext {
                        http_status_code: Some(http_code),
                        additional_context: Some("connector_customer_id required to build mandate reference for authorizedotnet".to_string()),
                    },
                })
            })?
            .clone();

        // Check if we have a successful response:
        // 1. resultCode == "Ok" (normal success)
        // 2. OR we have customer profile ID AND payment profile ID (E00039 duplicate case)
        //    E00039 = "A duplicate customer payment profile already exists"
        //    This is acceptable for idempotent SetupMandate - profile is available for use
        let is_success = response.messages.result_code == ResultCode::Ok
            || (response.customer_profile_id.is_some()
                && (response.customer_payment_profile_id.is_some()
                    || !response.customer_payment_profile_id_list.is_empty()));

        let status = if is_success {
            AttemptStatus::Charged
        } else {
            AttemptStatus::Failure
        };

        let mut new_router_data = router_data;
        let mut resource_common_data = new_router_data.resource_common_data.clone();
        resource_common_data.status = status;
        new_router_data.resource_common_data = resource_common_data;

        if response.customer_profile_id.is_some() {
            // Extract payment profile ID from response
            let payment_profile_id = response
                .customer_payment_profile_id_list
                .first()
                .or(response.customer_payment_profile_id.as_ref())
                .ok_or_else(|| {
                    error_stack::report!(crate::utils::response_handling_fail_for_connector(
                        http_code,
                        "authorizedotnet"
                    ))
                })?;

            // Create composite mandate ID: {customer_profile_id}-{payment_profile_id}
            let connector_mandate_id = format!("{connector_customer_id}-{payment_profile_id}");

            new_router_data.response = Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::NoResponseId,
                redirection_data: None,
                connector_metadata: None,
                mandate_reference: Some(Box::new(MandateReference {
                    connector_mandate_id: Some(connector_mandate_id),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                })),
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: http_code,
            });
        } else {
            let error_response = ErrorResponse {
                status_code: http_code,
                code: response
                    .messages
                    .message
                    .first()
                    .map(|m| m.code.clone())
                    .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
                message: response
                    .messages
                    .message
                    .first()
                    .map(|m| m.text.clone())
                    .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
                reason: Some(
                    response
                        .messages
                        .message
                        .first()
                        .map(|m| m.text.clone())
                        .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
                ),
                attempt_status: Some(AttemptStatus::Failure),
                connector_transaction_id: None,
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            };
            new_router_data.response = Err(error_response);
        }

        Ok(new_router_data)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetErrorResponse {
    pub messages: ResponseMessages,
}

// Webhook-related structures
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetWebhookObjectId {
    pub webhook_id: String,
    pub event_type: AuthorizedotnetWebhookEvent,
    pub payload: AuthorizedotnetWebhookPayload,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetWebhookPayload {
    pub id: Option<String>,
    // Fields specific to customer creation webhooks
    pub payment_profiles: Option<Vec<PaymentProfileInfo>>,
    pub merchant_customer_id: Option<String>,
    pub description: Option<String>,
    pub entity_name: Option<String>,
    // Fields specific to customer payment profile creation webhooks
    pub customer_profile_id: Option<u64>,
    pub customer_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentProfileInfo {
    pub id: String,
    pub customer_type: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetWebhookEventType {
    pub event_type: AuthorizedotnetIncomingWebhookEventType,
}

#[derive(Debug, Clone, Deserialize)]
pub enum AuthorizedotnetWebhookEvent {
    #[serde(rename = "net.authorize.payment.authorization.created")]
    AuthorizationCreated,
    #[serde(rename = "net.authorize.payment.priorAuthCapture.created")]
    PriorAuthCapture,
    #[serde(rename = "net.authorize.payment.authcapture.created")]
    AuthCapCreated,
    #[serde(rename = "net.authorize.payment.capture.created")]
    CaptureCreated,
    #[serde(rename = "net.authorize.payment.void.created")]
    VoidCreated,
    #[serde(rename = "net.authorize.payment.refund.created")]
    RefundCreated,
    #[serde(rename = "net.authorize.customer.created")]
    CustomerCreated,
    #[serde(rename = "net.authorize.customer.paymentProfile.created")]
    CustomerPaymentProfileCreated,
}

/// Including Unknown to map unknown webhook events
#[derive(Debug, Clone, Deserialize)]
pub enum AuthorizedotnetIncomingWebhookEventType {
    #[serde(rename = "net.authorize.payment.authorization.created")]
    AuthorizationCreated,
    #[serde(rename = "net.authorize.payment.priorAuthCapture.created")]
    PriorAuthCapture,
    #[serde(rename = "net.authorize.payment.authcapture.created")]
    AuthCapCreated,
    #[serde(rename = "net.authorize.payment.capture.created")]
    CaptureCreated,
    #[serde(rename = "net.authorize.payment.void.created")]
    VoidCreated,
    #[serde(rename = "net.authorize.payment.refund.created")]
    RefundCreated,
    #[serde(rename = "net.authorize.customer.created")]
    CustomerCreated,
    #[serde(rename = "net.authorize.customer.paymentProfile.created")]
    CustomerPaymentProfileCreated,
    #[serde(other)]
    Unknown,
}

impl From<AuthorizedotnetIncomingWebhookEventType> for interfaces::webhooks::IncomingWebhookEvent {
    fn from(event_type: AuthorizedotnetIncomingWebhookEventType) -> Self {
        match event_type {
            AuthorizedotnetIncomingWebhookEventType::AuthorizationCreated
            | AuthorizedotnetIncomingWebhookEventType::PriorAuthCapture
            | AuthorizedotnetIncomingWebhookEventType::AuthCapCreated
            | AuthorizedotnetIncomingWebhookEventType::CaptureCreated
            | AuthorizedotnetIncomingWebhookEventType::VoidCreated
            | AuthorizedotnetIncomingWebhookEventType::CustomerCreated
            | AuthorizedotnetIncomingWebhookEventType::CustomerPaymentProfileCreated => {
                Self::PaymentIntentSuccess
            }
            AuthorizedotnetIncomingWebhookEventType::RefundCreated => Self::RefundSuccess,
            AuthorizedotnetIncomingWebhookEventType::Unknown => Self::EventNotSupported,
        }
    }
}

impl From<AuthorizedotnetWebhookEvent> for AttemptStatus {
    // status mapping reference https://developer.authorize.net/api/reference/features/webhooks.html#Event_Types_and_Payloads
    fn from(event_type: AuthorizedotnetWebhookEvent) -> Self {
        match event_type {
            AuthorizedotnetWebhookEvent::AuthorizationCreated => Self::Authorized,
            AuthorizedotnetWebhookEvent::CaptureCreated
            | AuthorizedotnetWebhookEvent::AuthCapCreated
            | AuthorizedotnetWebhookEvent::PriorAuthCapture => Self::Charged,
            AuthorizedotnetWebhookEvent::VoidCreated => Self::Voided,
            AuthorizedotnetWebhookEvent::RefundCreated => Self::PartialCharged, // This will be used for refund status
            AuthorizedotnetWebhookEvent::CustomerCreated => Self::Charged, // Customer profile creation indicates successful setup mandate
            AuthorizedotnetWebhookEvent::CustomerPaymentProfileCreated => Self::Charged, // Payment profile creation indicates successful setup mandate
        }
    }
}

impl From<AuthorizedotnetWebhookEvent> for SyncStatus {
    // status mapping reference https://developer.authorize.net/api/reference/features/webhooks.html#Event_Types_and_Payloads
    fn from(event_type: AuthorizedotnetWebhookEvent) -> Self {
        match event_type {
            AuthorizedotnetWebhookEvent::AuthorizationCreated => Self::AuthorizedPendingCapture,
            AuthorizedotnetWebhookEvent::CaptureCreated
            | AuthorizedotnetWebhookEvent::AuthCapCreated => Self::CapturedPendingSettlement,
            AuthorizedotnetWebhookEvent::PriorAuthCapture => Self::SettledSuccessfully,
            AuthorizedotnetWebhookEvent::VoidCreated => Self::Voided,
            AuthorizedotnetWebhookEvent::RefundCreated => Self::RefundSettledSuccessfully,
            AuthorizedotnetWebhookEvent::CustomerCreated => Self::SettledSuccessfully, // Customer profile successfully created and settled
            AuthorizedotnetWebhookEvent::CustomerPaymentProfileCreated => Self::SettledSuccessfully, // Payment profile successfully created and settled
        }
    }
}

pub fn get_trans_id(details: &AuthorizedotnetWebhookObjectId) -> Result<String, WebhookError> {
    match details.event_type {
        AuthorizedotnetWebhookEvent::CustomerPaymentProfileCreated => {
            // For payment profile creation, use the customer_profile_id as the primary identifier
            if let Some(customer_profile_id) = details.payload.customer_profile_id {
                tracing::debug!(
                    target: "authorizedotnet_webhook",
                    "Extracted customer profile ID {} for payment profile creation webhook",
                    customer_profile_id
                );
                Ok(customer_profile_id.to_string())
            } else {
                match details.payload.id.clone() {
                    Some(id) => {
                        tracing::debug!(
                            target: "authorizedotnet_webhook",
                            "Extracted transaction ID {} from payment profile webhook payload",
                            id
                        );
                        Ok(id)
                    }
                    None => {
                        tracing::error!(
                            target: "authorizedotnet_webhook",
                            "No customer_profile_id or id found in CustomerPaymentProfileCreated webhook payload"
                        );
                        Err(WebhookError::WebhookReferenceIdNotFound)
                    }
                }
            }
        }
        _ => {
            // For all other events, use the standard id field
            match details.payload.id.clone() {
                Some(id) => {
                    tracing::debug!(
                        target: "authorizedotnet_webhook",
                        "Extracted transaction ID {} for webhook event type: {:?}",
                        id,
                        details.event_type
                    );
                    Ok(id)
                }
                None => {
                    tracing::error!(
                        target: "authorizedotnet_webhook",
                        "No transaction ID found in webhook payload for event type: {:?}",
                        details.event_type
                    );
                    Err(WebhookError::WebhookReferenceIdNotFound)
                }
            }
        }
    }
}

impl TryFrom<AuthorizedotnetWebhookObjectId> for AuthorizedotnetPSyncResponse {
    type Error = error_stack::Report<WebhookError>;
    fn try_from(item: AuthorizedotnetWebhookObjectId) -> Result<Self, Self::Error> {
        Ok(Self {
            transaction: Some(SyncTransactionResponse {
                transaction_id: get_trans_id(&item)?,
                transaction_status: SyncStatus::from(item.event_type),
                response_code: Some(1),
                response_reason_code: Some(1),
                response_reason_description: Some("Approved".to_string()),
                network_trans_id: None,
            }),
            messages: ResponseMessages {
                result_code: ResultCode::Ok,
                message: vec![ResponseMessage {
                    code: "I00001".to_string(),
                    text: "Successful.".to_string(),
                }],
            },
        })
    }
}

// Helper function to extract customer profile ID from error message
// Message format: "A duplicate record with ID 933042598 already exists."
fn extract_customer_id_from_error(error_text: &str) -> Option<String> {
    // Look for pattern "ID <numbers>"
    error_text
        .split_whitespace()
        .skip_while(|&word| word != "ID")
        .nth(1) // Get the word after "ID"
        .and_then(|id_str| {
            // Remove any trailing punctuation and validate it's numeric
            let cleaned = id_str.trim_end_matches(|c: char| !c.is_numeric());
            if cleaned.chars().all(char::is_numeric) && !cleaned.is_empty() {
                Some(cleaned.to_string())
            } else {
                None
            }
        })
}

// TryFrom implementations for CreateConnectorCustomer flow

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AuthorizedotnetRouterData<
            RouterDataV2<
                CreateConnectorCustomer,
                PaymentFlowData,
                ConnectorCustomerData,
                ConnectorCustomerResponse,
            >,
            T,
        >,
    > for AuthorizedotnetCreateConnectorCustomerRequest<T>
{
    type Error = Error;
    fn try_from(
        item: AuthorizedotnetRouterData<
            RouterDataV2<
                CreateConnectorCustomer,
                PaymentFlowData,
                ConnectorCustomerData,
                ConnectorCustomerResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let merchant_authentication =
            AuthorizedotnetAuthType::try_from(&item.router_data.connector_config)?;

        // Build ship_to_list from shipping address if available
        let ship_to_list = item
            .router_data
            .resource_common_data
            .address
            .get_shipping()
            .and_then(|shipping| {
                shipping.address.as_ref().map(|address| {
                    vec![ShipToList {
                        first_name: address.first_name.clone(),
                        last_name: address.last_name.clone(),
                        address: get_address_line(&address.line1, &address.line2, &address.line3),
                        city: address.city.clone(),
                        state: address.state.clone(),
                        zip: address.zip.clone(),
                        country: address.country,
                        phone_number: shipping
                            .phone
                            .as_ref()
                            .and_then(|phone| phone.number.clone()),
                    }]
                })
            });

        // Conditionally send merchant_customer_id (matching Hyperswitch parity)
        // Only send if customer_id exists and length <= MAX_ID_LENGTH (20 chars)
        let merchant_customer_id = validate_customer_id_length(
            item.router_data
                .request
                .customer_id
                .as_ref()
                .map(|id| id.peek().clone()),
        );

        // Create a customer profile without payment method (zero mandate)
        Ok(Self {
            create_customer_profile_request: AuthorizedotnetZeroMandateRequest {
                merchant_authentication,
                profile: Profile {
                    merchant_customer_id,
                    description: None,
                    email: item
                        .router_data
                        .request
                        .email
                        .as_ref()
                        .map(|e| e.peek().clone().expose().expose()),
                    payment_profiles: None,
                    ship_to_list,
                },
                validation_mode: None,
            },
        })
    }
}

impl TryFrom<ResponseRouterData<AuthorizedotnetCreateConnectorCustomerResponse, Self>>
    for RouterDataV2<
        CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    >
{
    type Error = ResponseError;
    fn try_from(
        value: ResponseRouterData<AuthorizedotnetCreateConnectorCustomerResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        let mut new_router_data = router_data;

        if let Some(profile_id) = response.customer_profile_id {
            // Success - return the connector customer ID
            new_router_data.response = Ok(ConnectorCustomerResponse {
                connector_customer_id: profile_id,
            });
        } else {
            // Check if this is a "duplicate customer" error (E00039)
            let first_error = response.messages.message.first();
            let error_code = first_error.map(|m| m.code.as_str()).unwrap_or("");
            let error_text = first_error.map(|m| m.text.as_str()).unwrap_or("");

            if error_code == "E00039" {
                // Extract customer profile ID from error message
                // Message format: "A duplicate record with ID 933042598 already exists."
                if let Some(existing_profile_id) = extract_customer_id_from_error(error_text) {
                    tracing::info!(
                        "Customer profile already exists with ID: {}, treating as success",
                        existing_profile_id
                    );
                    new_router_data.response = Ok(ConnectorCustomerResponse {
                        connector_customer_id: existing_profile_id,
                    });
                } else {
                    // Couldn't extract ID, return error
                    new_router_data.response = Err(ErrorResponse {
                        status_code: http_code,
                        code: error_code.to_string(),
                        message: error_text.to_string(),
                        reason: Some(error_text.to_string()),
                        attempt_status: Some(AttemptStatus::Failure), // Marking attempt as failure since we couldn't confirm existing profile ID
                        connector_transaction_id: None,
                        network_decline_code: None,
                        network_advice_code: None,
                        network_error_message: None,
                    });
                }
            } else {
                // Other error - return error response
                new_router_data.response = Err(ErrorResponse {
                    status_code: http_code,
                    code: error_code.to_string(),
                    message: error_text.to_string(),
                    reason: Some(error_text.to_string()),
                    attempt_status: Some(AttemptStatus::Failure), // Marking attempt as failure for non-duplicate errors
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                });
            }
        }

        Ok(new_router_data)
    }
}
