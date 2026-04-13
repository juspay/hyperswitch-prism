use crate::types::ResponseRouterData;
use common_enums::{AttemptStatus, RefundStatus};
use common_utils::types::{AmountConvertor, FloatMajorUnit, FloatMajorUnitForConnector};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, PreAuthenticate, RSync, Refund, Void},
    connector_types::{
        MandateReference, PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, ResponseId,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::{
        BankDebitData, GpayTokenizationData, PaymentMethodData, PaymentMethodDataTypes,
        RawCardNumber, WalletData,
    },
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
    utils::{get_unimplemented_payment_method_error_message, ForeignTryFrom},
};
use grpc_api_types::payments::{Currency, Money};

// Note: Refund and RefundsData are used for the Refund flow implementation
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use serde_json;

// ===== AUTHENTICATION =====

#[derive(Debug, Clone)]
pub struct NmiAuthType {
    pub api_key: Secret<String>,
    pub public_key: Option<Secret<String>>,
}

impl TryFrom<&ConnectorSpecificConfig> for NmiAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Nmi {
                api_key,
                public_key,
                ..
            } => Ok(Self {
                api_key: api_key.to_owned(),
                public_key: public_key.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
            )),
        }
    }
}

// ===== TRANSACTION TYPES =====

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TransactionType {
    Auth,
    Sale,
    Capture,
    Refund,
    Void,
    Validate,
    Credit,
}

// ===== NMI STATUS ENUM =====

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NmiStatus {
    Abandoned,
    Cancelled,
    Pendingsettlement,
    Pending,
    Failed,
    Complete,
    InProgress,
    Unknown,
}

impl From<String> for NmiStatus {
    fn from(value: String) -> Self {
        match value.as_str() {
            "abandoned" => Self::Abandoned,
            "canceled" => Self::Cancelled,
            "in_progress" => Self::InProgress,
            "pendingsettlement" => Self::Pendingsettlement,
            "complete" => Self::Complete,
            "failed" => Self::Failed,
            "unknown" => Self::Unknown,
            // Other than above values only pending is possible, since value is a string handling this as default
            _ => Self::Pending,
        }
    }
}

impl From<NmiStatus> for AttemptStatus {
    fn from(item: NmiStatus) -> Self {
        match item {
            NmiStatus::Abandoned => Self::AuthenticationFailed,
            NmiStatus::Cancelled => Self::Voided,
            NmiStatus::Pending => Self::Authorized,
            NmiStatus::Pendingsettlement | NmiStatus::Complete => Self::Charged,
            NmiStatus::InProgress => Self::AuthenticationPending,
            NmiStatus::Failed | NmiStatus::Unknown => Self::Failure,
        }
    }
}

impl From<NmiStatus> for RefundStatus {
    fn from(item: NmiStatus) -> Self {
        match item {
            NmiStatus::Abandoned
            | NmiStatus::Cancelled
            | NmiStatus::Failed
            | NmiStatus::Unknown => Self::Failure,
            NmiStatus::Pending | NmiStatus::InProgress => Self::Pending,
            NmiStatus::Pendingsettlement | NmiStatus::Complete => Self::Success,
        }
    }
}

// ===== PAYMENT METHOD DATA =====

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum NmiPaymentMethod<T: PaymentMethodDataTypes> {
    Card(Box<CardData<T>>),
    Ach(Box<AchData>),
    GooglePay(Box<GooglePayData>),
    GooglePayDecrypt(Box<GooglePayDecryptedData>),
}

// ===== GOOGLE PAY DATA =====

#[derive(Debug, Serialize)]
pub struct GooglePayData {
    #[serde(rename = "payment_token")]
    payment_token: Secret<String>,
}

#[derive(Debug, Serialize)]
pub struct GooglePayDecryptedData {
    decrypted_googlepay_data: DecryptedDataIndicator,
    ccnumber: Secret<String>,
    ccexp: Secret<String>,
    cavv: Option<Secret<String>>,
    eci: Option<String>,
}

#[derive(Debug, Serialize)]
pub enum DecryptedDataIndicator {
    #[serde(rename = "1")]
    Decrypted,
}

#[derive(Debug, Serialize)]
pub struct CardData<T: PaymentMethodDataTypes> {
    ccnumber: RawCardNumber<T>,
    ccexp: Secret<String>, // MMYY format
    cvv: Secret<String>,
}

// ACH Payment Type Constant
const ACH_PAYMENT_TYPE: &str = "check";

// ACH Bank Debit Data Structure
#[derive(Debug, Serialize)]
pub struct AchData {
    /// Payment type - must be "check" for ACH transactions
    #[serde(rename = "payment")]
    payment_type: &'static str,
    /// Name on the customer's ACH account
    checkname: Secret<String>,
    /// Customer's bank routing number (exactly 9 digits)
    checkaba: Secret<String>,
    /// Customer's bank account number
    checkaccount: Secret<String>,
    /// Type of ACH account holder (business, personal)
    #[serde(skip_serializing_if = "Option::is_none")]
    account_holder_type: Option<common_enums::BankHolderType>,
    /// Type of ACH account (checking, savings)
    #[serde(skip_serializing_if = "Option::is_none")]
    account_type: Option<common_enums::BankType>,
    /// Standard Entry Class code of the ACH transaction (PPD, WEB, TEL, CCD)
    #[serde(skip_serializing_if = "Option::is_none")]
    sec_code: Option<String>,
}

// ===== MERCHANT DEFINED FIELDS =====

#[derive(Debug, Serialize)]
pub struct NmiMerchantDefinedField {
    #[serde(flatten)]
    inner: std::collections::BTreeMap<String, Secret<String>>,
}

impl NmiMerchantDefinedField {
    pub fn new(metadata: &serde_json::Value) -> Self {
        let inner = metadata
            .as_object()
            .map(|obj| {
                obj.iter()
                    .enumerate()
                    .map(|(index, (hs_key, hs_value))| {
                        // Extract string value properly to avoid JSON encoding
                        let value_str = hs_value
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| hs_value.to_string());
                        let nmi_key = format!("merchant_defined_field_{}", index + 1);
                        let nmi_value = format!("{hs_key}={value_str}");
                        (nmi_key, Secret::new(nmi_value))
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self { inner }
    }
}

#[derive(Debug, Serialize)]
pub struct NmiBillingDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    address1: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    address2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    zip: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    country: Option<common_enums::CountryAlpha2>,
    #[serde(skip_serializing_if = "Option::is_none")]
    phone: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<common_utils::pii::Email>,
}

#[derive(Debug, Serialize)]
pub struct NmiShippingDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_firstname: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_lastname: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_address1: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_address2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_state: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_zip: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_country: Option<common_enums::CountryAlpha2>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_email: Option<common_utils::pii::Email>,
}

// ===== PAYMENT REQUEST =====

#[derive(Debug, Serialize)]
pub struct NmiPaymentsRequest<T: PaymentMethodDataTypes> {
    security_key: Secret<String>,
    #[serde(rename = "type")]
    transaction_type: TransactionType,
    amount: FloatMajorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    currency: Option<common_enums::Currency>,
    orderid: String,
    #[serde(flatten)]
    #[serde(skip_serializing_if = "Option::is_none")]
    payment_method: Option<NmiPaymentMethod<T>>,
    #[serde(flatten)]
    #[serde(skip_serializing_if = "Option::is_none")]
    merchant_defined_field: Option<NmiMerchantDefinedField>,
    #[serde(flatten)]
    #[serde(skip_serializing_if = "Option::is_none")]
    billing_details: Option<NmiBillingDetails>,
    #[serde(flatten)]
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_details: Option<NmiShippingDetails>,
    // Fields for 3DS completion (when redirect_response is present)
    #[serde(skip_serializing_if = "Option::is_none")]
    customer_vault_id: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<common_utils::pii::Email>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cardholder_auth: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cavv: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    xid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    eci: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cvv: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    three_ds_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    directory_server_id: Option<Secret<String>>,
}

// Implementation for NmiRouterData wrapper (needed by macros)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::NmiRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NmiPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::NmiRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = NmiAuthType::try_from(&router_data.connector_config)?;

        Self::try_from(&NmiAuthorizeRouterData {
            router_data: router_data.clone(),
            auth,
        })
    }
}

/// Wrapper struct to distinguish 3DS completion from regular authorize
struct NmiAuthorizeRouterData<T: PaymentMethodDataTypes> {
    router_data:
        RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    auth: NmiAuthType,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<&NmiAuthorizeRouterData<T>> for NmiPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(data: &NmiAuthorizeRouterData<T>) -> Result<Self, Self::Error> {
        let router_data = &data.router_data;
        let auth = &data.auth;

        if router_data.request.redirect_response.is_some() {
            // 3DS completion flow
            let redirect_response =
                router_data
                    .request
                    .redirect_response
                    .as_ref()
                    .ok_or_else(|| {
                        error_stack::report!(IntegrationError::MissingRequiredField {
                            field_name: "redirect_response",
                            context: Default::default(),
                        })
                    })?;

            let payload_data = redirect_response.payload.clone().ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                    field_name: "redirect_response.payload",
                    context: Default::default(),
                })
            })?;

            let three_ds_data: NmiRedirectResponseData = serde_json::from_value(
                payload_data.expose(),
            )
            .change_context(IntegrationError::MissingRequiredField {
                field_name: "three_ds_data",
                context: Default::default(),
            })?;

            let cvv = match &router_data.request.payment_method_data {
                PaymentMethodData::Card(card_data) => Some(card_data.card_cvc.clone()),
                _ => None,
            };

            let converter = FloatMajorUnitForConnector;
            let amount = converter
                .convert(
                    router_data.request.minor_amount,
                    router_data.request.currency,
                )
                .change_context(IntegrationError::RequestEncodingFailed {
                    context: Default::default(),
                })?;

            let transaction_type = if router_data.request.is_auto_capture() {
                TransactionType::Sale
            } else {
                TransactionType::Auth
            };

            Ok(Self {
                security_key: auth.api_key.clone(),
                transaction_type,
                amount,
                currency: None,
                orderid: three_ds_data.order_id.ok_or_else(|| {
                    error_stack::report!(IntegrationError::MissingRequiredField {
                        field_name: "order_id",
                        context: Default::default(),
                    })
                })?,
                payment_method: None,
                merchant_defined_field: None,
                billing_details: None,
                shipping_details: None,
                customer_vault_id: Some(three_ds_data.customer_vault_id),
                email: router_data.request.email.clone(),
                cardholder_auth: three_ds_data.card_holder_auth,
                cavv: three_ds_data.cavv,
                xid: three_ds_data.xid,
                eci: three_ds_data.eci,
                cvv,
                three_ds_version: three_ds_data.three_ds_version,
                directory_server_id: three_ds_data.directory_server_id,
            })
        } else {
            // Regular authorization flow
            let (payment_method, transaction_type) = match &router_data.request.payment_method_data
            {
                PaymentMethodData::BankDebit(bank_debit_data) => {
                    let ach_data = create_ach_data(bank_debit_data, router_data)?;
                    (
                        NmiPaymentMethod::Ach(Box::new(ach_data)),
                        TransactionType::Sale,
                    )
                }
                PaymentMethodData::Wallet(WalletData::GooglePay(google_pay_data)) => {
                    match &google_pay_data.tokenization_data {
                        GpayTokenizationData::Decrypted(decrypted_data) => {
                            let ccexp = decrypted_data.get_expiry_date_as_mmyy().change_context(
                                IntegrationError::RequestEncodingFailed {
                                    context: Default::default(),
                                },
                            )?;
                            (
                                NmiPaymentMethod::GooglePayDecrypt(Box::new(
                                    GooglePayDecryptedData {
                                        decrypted_googlepay_data: DecryptedDataIndicator::Decrypted,
                                        ccnumber: Secret::new(
                                            decrypted_data
                                                .application_primary_account_number
                                                .get_card_no(),
                                        ),
                                        ccexp,
                                        cavv: decrypted_data.cryptogram.clone(),
                                        eci: decrypted_data.eci_indicator.clone(),
                                    },
                                )),
                                TransactionType::Sale,
                            )
                        }
                        GpayTokenizationData::Encrypted(encrypted_data) => (
                            NmiPaymentMethod::GooglePay(Box::new(GooglePayData {
                                payment_token: Secret::new(encrypted_data.token.clone()),
                            })),
                            if router_data.request.is_auto_capture() {
                                TransactionType::Sale
                            } else {
                                TransactionType::Auth
                            },
                        ),
                    }
                }
                _ => {
                    let txn_type = if router_data.request.is_auto_capture() {
                        TransactionType::Sale
                    } else {
                        TransactionType::Auth
                    };
                    (
                        NmiPaymentMethod::try_from(&router_data.request.payment_method_data)?,
                        txn_type,
                    )
                }
            };

            let converter = FloatMajorUnitForConnector;
            let amount = converter
                .convert(
                    router_data.request.minor_amount,
                    router_data.request.currency,
                )
                .change_context(IntegrationError::RequestEncodingFailed {
                    context: Default::default(),
                })?;

            Ok(Self {
                security_key: auth.api_key.clone(),
                transaction_type,
                amount,
                currency: Some(router_data.request.currency),
                orderid: router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
                payment_method: Some(payment_method),
                merchant_defined_field: router_data
                    .request
                    .metadata
                    .as_ref()
                    .map(|m| NmiMerchantDefinedField::new(m.peek())),
                billing_details: Some(NmiBillingDetails {
                    address1: router_data
                        .resource_common_data
                        .get_optional_billing_line1(),
                    address2: router_data
                        .resource_common_data
                        .get_optional_billing_line2(),
                    city: router_data.resource_common_data.get_optional_billing_city(),
                    state: router_data
                        .resource_common_data
                        .get_optional_billing_state(),
                    zip: router_data.resource_common_data.get_optional_billing_zip(),
                    country: router_data
                        .resource_common_data
                        .get_optional_billing_country(),
                    phone: router_data
                        .resource_common_data
                        .get_optional_billing_phone_number(),
                    email: router_data
                        .resource_common_data
                        .get_optional_billing_email(),
                }),
                shipping_details: Some(NmiShippingDetails {
                    shipping_firstname: router_data
                        .resource_common_data
                        .get_optional_shipping_first_name(),
                    shipping_lastname: router_data
                        .resource_common_data
                        .get_optional_shipping_last_name(),
                    shipping_address1: router_data
                        .resource_common_data
                        .get_optional_shipping_line1(),
                    shipping_address2: router_data
                        .resource_common_data
                        .get_optional_shipping_line2(),
                    shipping_city: router_data
                        .resource_common_data
                        .get_optional_shipping_city(),
                    shipping_state: router_data
                        .resource_common_data
                        .get_optional_shipping_state(),
                    shipping_zip: router_data.resource_common_data.get_optional_shipping_zip(),
                    shipping_country: router_data
                        .resource_common_data
                        .get_optional_shipping_country(),
                    shipping_email: router_data
                        .resource_common_data
                        .get_optional_shipping_email(),
                }),
                customer_vault_id: None,
                email: None,
                cardholder_auth: None,
                cavv: None,
                xid: None,
                eci: None,
                cvv: None,
                three_ds_version: None,
                directory_server_id: None,
            })
        }
    }
}

// ===== PAYMENT METHOD TRANSFORMATION =====

impl<T: PaymentMethodDataTypes> TryFrom<&PaymentMethodData<T>> for NmiPaymentMethod<T> {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(pm_data: &PaymentMethodData<T>) -> Result<Self, Self::Error> {
        match pm_data {
            PaymentMethodData::Card(card_data) => {
                // Extract expiry date in MMYY format using framework utility
                let ccexp =
                    card_data.get_card_expiry_month_year_2_digit_with_delimiter("".to_string())?;

                let card = CardData {
                    ccnumber: card_data.card_number.clone(),
                    ccexp,
                    cvv: card_data.card_cvc.clone(),
                };
                Ok(Self::Card(Box::new(card)))
            }
            PaymentMethodData::BankDebit(
                BankDebitData::SepaBankDebit { .. }
                | BankDebitData::BecsBankDebit { .. }
                | BankDebitData::BacsBankDebit { .. },
            ) => Err(error_stack::report!(IntegrationError::NotImplemented(
                "Bank Debit type not supported for NMI".to_string(),
                Default::default(),
            ))),
            _ => Err(error_stack::report!(IntegrationError::NotImplemented(
                "Payment method not supported".to_string(),
                Default::default()
            ))),
        }
    }
}

/// Helper function to create ACH data from BankDebitData with access to router data for billing name fallback
fn create_ach_data<T: PaymentMethodDataTypes>(
    bank_debit_data: &BankDebitData,
    router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
) -> Result<AchData, error_stack::Report<IntegrationError>> {
    match bank_debit_data {
        BankDebitData::AchBankDebit {
            account_number,
            routing_number,
            bank_account_holder_name,
            bank_holder_type,
            bank_type,
            ..
        } => {
            // Get account holder name: use bank_account_holder_name or fall back to billing name
            let checkname = bank_account_holder_name
                .clone()
                .or_else(|| {
                    router_data
                        .resource_common_data
                        .get_billing_full_name()
                        .ok()
                })
                .ok_or_else(|| {
                    error_stack::report!(IntegrationError::MissingRequiredField {
                        field_name: "bank_account_holder_name",
                        context: Default::default(),
                    })
                })?;

            let ach_data = AchData {
                payment_type: ACH_PAYMENT_TYPE,
                checkname,
                checkaba: routing_number.clone(),
                checkaccount: account_number.clone(),
                account_holder_type: *bank_holder_type,
                account_type: *bank_type,
                sec_code: None, // Can be set if needed: PPD, WEB, TEL, CCD
            };
            Ok(ach_data)
        }
        _ => Err(error_stack::report!(IntegrationError::NotImplemented(
            "Only ACH Bank Debit is supported for NMI".to_string(),
            Default::default(),
        ))),
    }
}

// ===== PAYMENT RESPONSE =====

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct StandardResponse {
    pub response: String, // "1" = approved, "2" = declined, "3" = error
    pub responsetext: String,
    pub authcode: Option<String>,
    pub transactionid: String,
    pub avsresponse: Option<String>,
    pub cvvresponse: Option<String>,
    pub orderid: String,
    pub response_code: String,
    #[serde(default)]
    pub customer_vault_id: Option<Secret<String>>,
}

// Type alias for consistency with nmi.rs
pub type NmiPaymentsResponse = StandardResponse;

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<StandardResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<StandardResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;

        // Determine status based on response code
        let status = match response.response.as_str() {
            "1" => {
                // Approved - check if it was auth or sale
                // For auth type, status is Authorized
                // For sale type, status is Charged
                // We need to check the original request's auto_capture flag
                if item.router_data.request.is_auto_capture() {
                    AttemptStatus::Charged
                } else {
                    AttemptStatus::Authorized
                }
            }
            "2" | "3" => AttemptStatus::Failure, // Declined or Error
            _ => AttemptStatus::Pending,
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.transactionid.clone()),
                redirection_data: None,
                mandate_reference: response.customer_vault_id.as_ref().map(|vault_id| {
                    Box::new(MandateReference {
                        connector_mandate_id: Some(vault_id.clone().expose()),
                        payment_method_id: None,
                        connector_mandate_request_reference_id: None,
                    })
                }),
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(response.orderid.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== PAYMENT SYNC (PSYNC) REQUEST =====

#[derive(Debug, Serialize)]
pub struct NmiSyncRequest {
    security_key: Secret<String>,
    order_id: String, // Uses attempt_id, NOT connector_transaction_id
}

// Implementation for NmiRouterData wrapper (needed by macros)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::NmiRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for NmiSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::NmiRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = NmiAuthType::try_from(&router_data.connector_config)?;

        // PSync uses attempt_id as order_id (NOT connector_transaction_id)
        // The connector_transaction_id contains the attempt_id for sync operations
        let order_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        Ok(Self {
            security_key: auth.api_key,
            order_id,
        })
    }
}

// ===== PAYMENT SYNC (PSYNC) RESPONSE =====

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename = "nm_response")]
pub struct SyncResponse {
    #[serde(default)]
    pub transaction: Vec<SyncTransactionData>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SyncTransactionData {
    pub transaction_id: String,
    pub condition: String, // Maps to status
}

impl TryFrom<ResponseRouterData<SyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<SyncResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;

        // Get the requested transaction_id to find the correct transaction
        let requested_transaction_id = item
            .router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: Default::default(),
            })?;

        // Find the transaction matching the requested transaction_id
        // If not found, use the most recent one (last in list)
        let transaction = response
            .transaction
            .iter()
            .find(|txn| txn.transaction_id == requested_transaction_id)
            .or_else(|| {
                // Log when using fallback to most recent transaction
                if let Some(last_txn) = response.transaction.last() {
                    tracing::warn!(
                        requested_txn = %requested_transaction_id,
                        fallback_txn = %last_txn.transaction_id,
                        "PSync: Transaction not found in response, using most recent transaction instead"
                    );
                }
                response.transaction.last()
            });

        // Handle empty response (means AuthenticationPending) or transaction data
        let (status, transaction_id) = if let Some(transaction) = transaction {
            // Map condition field from XML to AttemptStatus using NmiStatus enum
            let status = AttemptStatus::from(NmiStatus::from(transaction.condition.clone()));
            (status, Some(transaction.transaction_id.clone()))
        } else {
            // Empty XML response = AuthenticationPending (during 3DS flow)
            (AttemptStatus::AuthenticationPending, None)
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: transaction_id
                    .map(ResponseId::ConnectorTransactionId)
                    .unwrap_or(ResponseId::NoResponseId),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== CAPTURE REQUEST =====

#[derive(Debug, Serialize)]
pub struct NmiCaptureRequest {
    security_key: Secret<String>,
    #[serde(rename = "type")]
    transaction_type: TransactionType,
    transactionid: String,
    amount: FloatMajorUnit,
}

// Implementation for NmiRouterData wrapper (needed by macros)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::NmiRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for NmiCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::NmiRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = NmiAuthType::try_from(&router_data.connector_config)?;

        // Get the original transaction ID from connector_transaction_id
        let transactionid = router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(IntegrationError::MissingRequiredField {
                field_name: "connector_transaction_id",
                context: Default::default(),
            })?;

        // Convert amount from minor to major units using framework converter
        let converter = FloatMajorUnitForConnector;
        let amount = converter
            .convert(
                router_data.request.minor_amount_to_capture,
                router_data.request.currency,
            )
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        Ok(Self {
            security_key: auth.api_key,
            transaction_type: TransactionType::Capture,
            transactionid,
            amount,
        })
    }
}

// ===== CAPTURE RESPONSE =====

impl TryFrom<ResponseRouterData<StandardResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<StandardResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;

        // Capture success = Charged status
        // Capture failure = Failure status
        let status = match response.response.as_str() {
            "1" => AttemptStatus::Charged,       // Capture successful
            "2" | "3" => AttemptStatus::Failure, // Capture failed
            _ => AttemptStatus::Pending,
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.transactionid.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(response.orderid.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== REFUND REQUEST =====

#[derive(Debug, Serialize)]
pub struct NmiRefundRequest {
    security_key: Secret<String>,
    #[serde(rename = "type")]
    transaction_type: TransactionType,
    transactionid: String,
    orderid: String,
    amount: FloatMajorUnit, // 0.00 for full refund
    #[serde(skip_serializing_if = "Option::is_none")]
    payment: Option<PaymentType>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PaymentType {
    Creditcard,
    Check,
}

// Implementation for NmiRouterData wrapper (needed by macros)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::NmiRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for NmiRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::NmiRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = NmiAuthType::try_from(&router_data.connector_config)?;

        // Get the original payment transaction ID
        let transactionid = router_data.request.connector_transaction_id.clone();

        // Get the refund ID (refund_id) as orderid
        // If refund_id is not present, use connector_request_reference_id as fallback
        let orderid = router_data
            .resource_common_data
            .refund_id
            .clone()
            .unwrap_or_else(|| {
                tracing::debug!("Refund: refund_id not present, using connector_request_reference_id as orderid");
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone()
            });

        // Convert amount from minor to major units using framework converter
        let converter = FloatMajorUnitForConnector;
        let amount = converter
            .convert(
                router_data.request.minor_refund_amount,
                router_data.request.currency,
            )
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        Ok(Self {
            security_key: auth.api_key,
            transaction_type: TransactionType::Refund,
            transactionid,
            orderid,
            amount,
            payment: None, // NMI infers payment type from the referenced transaction
        })
    }
}

// ===== REFUND RESPONSE =====

impl TryFrom<ResponseRouterData<StandardResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<StandardResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;

        // Map response code to RefundStatus
        // "1" = Success, "2"/"3" = Failure
        let status = match response.response.as_str() {
            "1" => RefundStatus::Success,
            "2" | "3" => RefundStatus::Failure,
            _ => RefundStatus::Pending,
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: response.transactionid.clone(),
                refund_status: status,
                status_code: item.http_code,
            }),
            resource_common_data: RefundFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== REFUND SYNC (RSYNC) REQUEST =====

#[derive(Debug, Serialize)]
pub struct NmiRefundSyncRequest {
    security_key: Secret<String>,
    order_id: String, // Uses connector_refund_id
}

// Implementation for NmiRouterData wrapper (needed by macros)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::NmiRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for NmiRefundSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::NmiRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = NmiAuthType::try_from(&router_data.connector_config)?;

        // RSync uses connector_refund_id as order_id (per tech spec section 3.6)
        let order_id = router_data.request.connector_refund_id.clone();

        Ok(Self {
            security_key: auth.api_key,
            order_id,
        })
    }
}

// ===== REFUND SYNC (RSYNC) RESPONSE =====
// Reusing SyncResponse structure as XML format is same (per tech spec section 3.9)

impl TryFrom<ResponseRouterData<SyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<SyncResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;

        // Try to find exact match first, fallback to last transaction
        let transaction = response
            .transaction
            .iter()
            .find(|txn| txn.transaction_id == item.router_data.request.connector_refund_id)
            .or_else(|| response.transaction.last());

        // Map condition field from XML to RefundStatus using NmiStatus enum
        let (status, connector_refund_id) = if let Some(transaction) = transaction {
            let status = RefundStatus::from(NmiStatus::from(transaction.condition.clone()));
            (status, transaction.transaction_id.clone())
        } else {
            // Empty response - treat as pending with proper error for connector_refund_id
            return Err(error_stack::report!(
                ConnectorError::ResponseDeserializationFailed {
                    context: Default::default(),
                }
            ));
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status: status,
                status_code: item.http_code,
            }),
            resource_common_data: RefundFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== VOID REQUEST =====

#[derive(Debug, Serialize)]
pub struct NmiVoidRequest {
    security_key: Secret<String>,
    #[serde(rename = "type")]
    transaction_type: TransactionType,
    transactionid: String,
    void_reason: VoidReason,
    #[serde(skip_serializing_if = "Option::is_none")]
    payment: Option<PaymentType>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VoidReason {
    Fraud,
    UserCancel,
    IccRejected,
}

// Implementation for NmiRouterData wrapper (needed by macros)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::NmiRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for NmiVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::NmiRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = NmiAuthType::try_from(&router_data.connector_config)?;

        // Get the original payment transaction ID
        let transactionid = router_data.request.connector_transaction_id.clone();

        // Map cancellation reason to NMI's void reason
        let void_reason = router_data
            .request
            .cancellation_reason
            .as_ref()
            .and_then(|reason| match reason.as_str() {
                "fraud" => Some(VoidReason::Fraud),
                "user_cancel" | "requested_by_customer" => Some(VoidReason::UserCancel),
                _ => None,
            })
            .unwrap_or(VoidReason::UserCancel); // Default to UserCancel

        Ok(Self {
            security_key: auth.api_key,
            transaction_type: TransactionType::Void,
            transactionid,
            void_reason,
            payment: None, // NMI infers payment type from the referenced transaction
        })
    }
}

// ===== VOID RESPONSE =====

impl TryFrom<ResponseRouterData<StandardResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<StandardResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;

        // Void success = Voided status
        // Void failure = VoidFailed status
        let status = match response.response.as_str() {
            "1" => AttemptStatus::Voided,           // Void successful
            "2" | "3" => AttemptStatus::VoidFailed, // Void failed
            _ => AttemptStatus::Pending,
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.transactionid.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(response.orderid.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

pub type NmiVaultResponse = NmiVaultResponseStruct;
pub type NmiPreAuthenticateResponse = NmiVaultResponse;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Response {
    #[serde(alias = "1")]
    Approved,
    #[serde(alias = "2")]
    Declined,
    #[serde(alias = "3")]
    Error,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CustomerAction {
    AddCustomer,
    UpdateCustomer,
}

#[derive(Debug, Serialize)]
pub struct NmiVaultRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    security_key: Secret<String>,
    ccnumber: RawCardNumber<T>,
    ccexp: Secret<String>,
    cvv: Secret<String>,
    first_name: Secret<String>,
    last_name: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    address1: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    address2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    zip: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    country: Option<common_enums::CountryAlpha2>,
    customer_vault: CustomerAction,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NmiVaultResponseStruct {
    pub response: Response,
    pub responsetext: String,
    pub customer_vault_id: Option<Secret<String>>,
    pub response_code: String,
    pub transactionid: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum NmiRedirectResponse {
    NmiRedirectResponseData(NmiRedirectResponseData),
    NmiErrorResponseData(NmiErrorResponseData),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NmiErrorResponseData {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NmiRedirectResponseData {
    cavv: Option<String>,
    xid: Option<String>,
    eci: Option<String>,
    card_holder_auth: Option<String>,
    three_ds_version: Option<String>,
    order_id: Option<String>,
    directory_server_id: Option<Secret<String>>,
    customer_vault_id: Secret<String>,
}

type CardDetails<T> = common_utils::CustomResult<
    (RawCardNumber<T>, Secret<String>, Secret<String>),
    IntegrationError,
>;

fn get_card_details<T: PaymentMethodDataTypes>(
    payment_method_data: Option<&PaymentMethodData<T>>,
) -> CardDetails<T> {
    match payment_method_data {
        Some(PaymentMethodData::Card(ref card_details)) => Ok((
            card_details.card_number.clone(),
            card_details.get_card_expiry_month_year_2_digit_with_delimiter("".to_string())?,
            card_details.card_cvc.clone(),
        )),
        _ => Err(IntegrationError::NotImplemented(
            get_unimplemented_payment_method_error_message("NMI"),
            Default::default(),
        )
        .into()),
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::NmiRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NmiVaultRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::NmiRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = NmiAuthType::try_from(&router_data.connector_config)?;
        let (ccnumber, ccexp, cvv) =
            get_card_details(router_data.request.payment_method_data.as_ref())?;

        let billing_address = router_data.resource_common_data.get_billing_address()?;

        let first_name = billing_address.get_first_name()?;
        let last_name = billing_address.get_last_name().unwrap_or(first_name);

        Ok(Self {
            security_key: auth.api_key,
            ccnumber,
            ccexp,
            cvv,
            first_name: first_name.clone(),
            last_name: last_name.clone(),
            address1: billing_address.line1.clone(),
            address2: billing_address.line2.clone(),
            city: billing_address.city.clone(),
            state: billing_address.state.clone(),
            zip: billing_address.zip.clone(),
            country: billing_address.country,
            customer_vault: CustomerAction::AddCustomer,
        })
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<NmiVaultResponse, Self>>
    for RouterDataV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<NmiVaultResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;

        let (status, payment_response) = match response.response {
            Response::Approved => {
                let auth_type = NmiAuthType::try_from(&item.router_data.connector_config)
                    .change_context(ConnectorError::ResponseHandlingFailed {
                        context: Default::default(),
                    })?;
                let amount_data = item.router_data.request.amount;
                let currency_data = item.router_data.request.currency.ok_or(
                    ConnectorError::ResponseHandlingFailed {
                        context: Default::default(),
                    },
                )?;
                let customer_vault_id = response.customer_vault_id.clone().ok_or_else(|| {
                    error_stack::report!(ConnectorError::UnexpectedResponseError {
                        context: Default::default(),
                    })
                })?;

                (
                    AttemptStatus::AuthenticationPending,
                    Ok(PaymentsResponseData::PreAuthenticateResponse {
                        authentication_data: None,
                        redirection_data: Some(Box::new(RedirectForm::Nmi {
                            amount: Money {
                                minor_amount: amount_data.get_amount_as_i64(),
                                currency: Currency::foreign_try_from(currency_data)
                                    .map_err(|_| {
                                        error_stack::report!(
                                            ConnectorError::ResponseHandlingFailed {
                                                context: Default::default(),
                                            }
                                        )
                                    })?
                                    .into(),
                            },
                            public_key: auth_type.public_key.ok_or(
                                ConnectorError::ResponseHandlingFailed {
                                    context: Default::default(),
                                },
                            )?,
                            customer_vault_id: customer_vault_id.peek().to_string(),
                            order_id: item
                                .router_data
                                .resource_common_data
                                .connector_request_reference_id
                                .clone(),
                            continue_redirection_url: item
                                .router_data
                                .request
                                .continue_redirection_url
                                .as_ref()
                                .map(|url| url.to_string())
                                .ok_or_else(|| {
                                    error_stack::report!(ConnectorError::ResponseHandlingFailed {
                                        context: Default::default(),
                                    })
                                })?,
                        })),
                        connector_response_reference_id: Some(response.transactionid.clone()),
                        status_code: item.http_code,
                    }),
                )
            }
            Response::Declined | Response::Error => (
                AttemptStatus::Failure,
                Err(domain_types::router_data::ErrorResponse {
                    code: response.response_code.clone(),
                    message: response.responsetext.clone(),
                    reason: Some(response.responsetext.clone()),
                    status_code: item.http_code,
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: Some(response.transactionid.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
            ),
        };

        Ok(Self {
            response: payment_response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}
