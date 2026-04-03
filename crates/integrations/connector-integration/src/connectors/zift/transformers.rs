use crate::{connectors::zift::ZiftRouterData, types::ResponseRouterData};
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::CustomResult,
    types::{MinorUnit, StringMinorUnit},
};
use error_stack::{report, Report, ResultExt};
use std::fmt::Debug;

use domain_types::{
    connector_flow::{Authorize, Capture, PSync, Refund, RepeatPayment, SetupMandate, Void},
    connector_types::{
        MandateReference, PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundsData,
        RefundsResponseData, RepeatPaymentData, ResponseId, SetupMandateRequestData,
    },
    errors::{ConnectorResponseTransformationError, IntegrationError, IntegrationErrorContext},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
};

pub trait ConnectorTransactionIdExt {
    fn get_connector_transaction_id_i64(&self) -> CustomResult<i64, IntegrationError>;
}

impl ConnectorTransactionIdExt for String {
    fn get_connector_transaction_id_i64(&self) -> CustomResult<i64, IntegrationError> {
        self.parse::<i64>()
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "connector_transaction_id",
                context: IntegrationErrorContext {
                    additional_context: Some("Expected numeric transaction ID".to_owned()),
                    ..Default::default()
                },
            })
            .attach_printable(format!(
                "Failed to parse connector_transaction_id: {} as i64",
                self
            ))
    }
}

impl ConnectorTransactionIdExt for Option<String> {
    fn get_connector_transaction_id_i64(&self) -> CustomResult<i64, IntegrationError> {
        self.as_ref()
            .ok_or_else(|| {
                report!(IntegrationError::MissingRequiredField {
                    field_name: "connector_transaction_id",
                    context: Default::default(),
                })
            })?
            .get_connector_transaction_id_i64()
    }
}

impl ConnectorTransactionIdExt for ResponseId {
    fn get_connector_transaction_id_i64(&self) -> CustomResult<i64, IntegrationError> {
        self.get_connector_transaction_id()
            .change_context(IntegrationError::MissingRequiredField {
                field_name: "connector_transaction_id",
                context: Default::default(),
            })?
            .get_connector_transaction_id_i64()
    }
}

use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZiftAuthType {
    user_name: Secret<String>,
    password: Secret<String>,
    account_id: Secret<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RequestType {
    Sale,
    #[serde(rename = "sale-auth")]
    Auth,
    Capture,
    Refund,
    Find,
    Void,
    #[serde(rename = "account-verification")]
    AccountVerification,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PaymentRequestType {
    Sale,
    #[serde(rename = "sale-auth")]
    Auth,
    Capture,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum AccountType {
    #[serde(rename = "R")]
    PaymentCard,
    #[serde(rename = "S")]
    Savings,
    #[serde(rename = "C")]
    Checking,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionIndustryType {
    #[serde(rename = "DM")]
    CardNotPresent,
    #[serde(rename = "RE")]
    CardPresent,
    #[serde(rename = "RS")]
    Restaurant,
    #[serde(rename = "LD")]
    Lodging,
    #[serde(rename = "PT")]
    Petroleum,
    #[serde(rename = "EC")]
    Ecommerce,
}
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum HolderType {
    #[serde(rename = "P")]
    Personal,
    #[serde(rename = "O")]
    Organizational,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ZiftPaymentsRequest<T: PaymentMethodDataTypes + Serialize + Debug> {
    Card(ZiftCardPaymentRequest<T>),
    Mandate(ZiftMandatePaymentRequest),
    ExternalThreeDs(ZiftExternalThreeDsPaymentRequest<T>),
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ZiftRepeatPaymentsRequest<T: PaymentMethodDataTypes + Serialize + Debug> {
    Card(ZiftCardPaymentRequest<T>),
    Mandate(ZiftMandatePaymentRequest),
    ExternalThreeDs(ZiftExternalThreeDsPaymentRequest<T>),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZiftCardPaymentRequest<T: PaymentMethodDataTypes + Serialize + Debug> {
    request_type: RequestType,
    #[serde(flatten)]
    auth: ZiftAuthType,
    account_type: AccountType,
    account_number: RawCardNumber<T>,
    account_accessory: Secret<String>,
    transaction_code: String,
    csc: Secret<String>,
    transaction_industry_type: TransactionIndustryType,
    transaction_category_code: TransactionCategoryCode,
    holder_name: Secret<String>,
    holder_type: HolderType,
    amount: StringMinorUnit,
    //Billing address fields are intentionally not passed to Zift.As confirmed by the Zift connector team, billing-related parameters must not be sent in payment or mandate requests. Passing billing address details was causing transaction failures in production. To ensure successful processing and alignment with Zift’s API expectations, all billing address fields have been removed.
}
// Mandate payment (MIT - Merchant Initiated)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZiftMandatePaymentRequest {
    request_type: RequestType,
    #[serde(flatten)]
    auth: ZiftAuthType,
    account_type: AccountType,
    token: Secret<String>,
    account_accessory: Secret<String>,
    // NO csc for MIT payments
    transaction_industry_type: TransactionIndustryType,
    transaction_category_code: TransactionCategoryCode,
    holder_name: Secret<String>,
    holder_type: HolderType,
    amount: StringMinorUnit,
    transaction_mode_type: TransactionModeType,
    transaction_code: String,

    // Required for MIT
    transaction_category_type: TransactionCategoryType,
    sequence_number: i32,
    //Billing address fields are intentionally not passed to Zift.As confirmed by the Zift connector team, billing-related parameters must not be sent in payment or mandate requests. Passing billing address details was causing transaction failures in production. To ensure successful processing and alignment with Zift’s API expectations, all billing address fields have been removed.
}

// External 3DS payment request
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZiftExternalThreeDsPaymentRequest<T: PaymentMethodDataTypes + Serialize + Debug> {
    request_type: RequestType,
    #[serde(flatten)]
    auth: ZiftAuthType,
    account_type: AccountType,
    account_number: RawCardNumber<T>,
    account_accessory: Secret<String>,
    transaction_industry_type: TransactionIndustryType,
    transaction_category_code: TransactionCategoryCode,
    holder_name: Secret<String>,
    holder_type: HolderType,
    transaction_code: String,
    amount: StringMinorUnit,
    // 3DS authentication fields
    authentication_status: AuthenticationStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    authentication_code: Option<Secret<String>>,
    authentication_verification_value: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    authentication_version: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
pub enum AuthenticationStatus {
    #[serde(rename = "Y")]
    Success,
    #[serde(rename = "A")]
    Attempted,
    #[serde(rename = "U")]
    Unavailable,
}

#[derive(Debug, Serialize)]
pub enum TransactionModeType {
    #[serde(rename = "P")]
    CardPresent,
    #[serde(rename = "N")]
    CardNotPresent,
}
#[derive(Debug, Serialize)]
pub enum TransactionCategoryType {
    #[serde(rename = "R")]
    Recurring,
    #[serde(rename = "I")]
    Installment,
    #[serde(rename = "B")]
    BillPayment,
}
#[derive(Debug, Serialize)]
pub enum TransactionCategoryCode {
    #[serde(rename = "EC")]
    Ecommerce,
}

pub trait ResponseCodeExt {
    fn is_pending(&self) -> bool;
    fn is_approved(&self) -> bool;
    fn is_failed(&self) -> bool;
}

impl ResponseCodeExt for String {
    fn is_pending(&self) -> bool {
        self == "X02"
    }
    fn is_approved(&self) -> bool {
        self.starts_with('A')
    }
    fn is_failed(&self) -> bool {
        !(self.is_approved() || self.is_pending())
    }
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ZiftErrorResponse {
    pub response_code: String,
    pub response_message: String,
    pub failure_code: String,
    pub failure_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ZiftAuthPaymentsResponse {
    pub response_code: String,
    pub response_message: String,
    pub transaction_id: Option<String>,
    pub transaction_code: Option<String>,
    pub token: Option<String>,
}

impl ZiftAuthPaymentsResponse {
    pub fn get_transaction_id(
        &self,
        http_code: u16,
    ) -> CustomResult<String, ConnectorResponseTransformationError> {
        self.transaction_id.clone().ok_or_else(|| {
            Report::new(
                ConnectorResponseTransformationError::response_handling_failed_with_context(
                    http_code,
                    Some("missing transaction_id in connector response".to_string()),
                ),
            )
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ZiftCaptureResponse {
    pub response_code: String,
    pub response_message: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZiftRefundRequest {
    request_type: RequestType,
    #[serde(flatten)]
    auth: ZiftAuthType,
    transaction_id: String,
    amount: StringMinorUnit,
    transaction_code: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZiftRefundResponse {
    transaction_id: Option<String>,
    response_code: String,
    response_message: Option<String>,
    transaction_code: Option<String>,
}

impl ZiftRefundResponse {
    pub fn get_transaction_id(
        &self,
        http_code: u16,
    ) -> CustomResult<String, ConnectorResponseTransformationError> {
        self.transaction_id.clone().ok_or_else(|| {
            Report::new(
                ConnectorResponseTransformationError::response_handling_failed_with_context(
                    http_code,
                    Some("missing transaction_id in connector response".to_string()),
                ),
            )
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionStatus {
    #[serde(rename = "N")]
    Pending,
    #[serde(rename = "P")]
    Processed,
    #[serde(rename = "C")]
    Cancelled,
    #[serde(rename = "R")]
    InRebill,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZiftSyncRequest {
    request_type: RequestType,
    #[serde(flatten)]
    auth: ZiftAuthType,
    transaction_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZiftSyncResponse {
    pub transaction_status: TransactionStatus,
    pub transaction_type: PaymentRequestType,
    pub response_message: Option<String>,
    pub response_code: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZiftCaptureRequest {
    request_type: RequestType,
    #[serde(flatten)]
    auth: ZiftAuthType,
    transaction_id: i64,
    amount: StringMinorUnit,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZiftVoidRequest {
    request_type: RequestType,
    #[serde(flatten)]
    auth: ZiftAuthType,
    transaction_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ZiftVoidResponse {
    pub response_code: String,
    pub response_message: String,
}

// Enum for payment method specific fields
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum SetupMandatePaymentMethod<T: PaymentMethodDataTypes + Serialize + Debug> {
    Card(CardVerificationDetails<T>),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZiftSetupMandateRequest<T: PaymentMethodDataTypes + Serialize + Debug> {
    request_type: RequestType,
    #[serde(flatten)]
    auth: ZiftAuthType,
    transaction_industry_type: TransactionIndustryType,
    transaction_category_code: TransactionCategoryCode,
    holder_name: Secret<String>,
    holder_type: HolderType,
    transaction_code: String,
    #[serde(flatten)]
    payment_method_details: SetupMandatePaymentMethod<T>,
    //Billing address fields are intentionally not passed to Zift.As confirmed by the Zift connector team, billing-related parameters must not be sent in payment or mandate requests. Passing billing address details was causing transaction failures in production. To ensure successful processing and alignment with Zift’s API expectations, all billing address fields have been removed.
}

// Card specific fields for account verification
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardVerificationDetails<T: PaymentMethodDataTypes + Serialize + Debug> {
    account_type: AccountType,
    account_number: RawCardNumber<T>,
    account_accessory: Secret<String>,
    csc: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for ZiftAuthType {
    type Error = Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        if let ConnectorSpecificConfig::Zift {
            user_name,
            password,
            account_id,
            ..
        } = auth_type
        {
            Ok(Self {
                user_name: user_name.to_owned(),
                password: password.to_owned(),
                account_id: account_id.to_owned(),
            })
        } else {
            Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            })?
        }
    }
}

impl TryFrom<&domain_types::router_request_types::AuthenticationData> for AuthenticationStatus {
    type Error = Report<IntegrationError>;

    fn try_from(
        auth_data: &domain_types::router_request_types::AuthenticationData,
    ) -> Result<Self, Self::Error> {
        // Map authentication status based on trans_status field
        let authentication_status = match auth_data.trans_status {
            Some(common_enums::TransactionStatus::Success) => Self::Success,
            Some(common_enums::TransactionStatus::NotVerified) => Self::Attempted,
            Some(common_enums::TransactionStatus::VerificationNotPerformed)
            | Some(common_enums::TransactionStatus::Rejected)
            | Some(common_enums::TransactionStatus::InformationOnly)
            | Some(common_enums::TransactionStatus::Failure)
            | Some(common_enums::TransactionStatus::ChallengeRequired)
            | Some(common_enums::TransactionStatus::ChallengeRequiredDecoupledAuthentication)
            | None => Self::Unavailable,
        };
        Ok(authentication_status)
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ZiftRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for ZiftPaymentsRequest<T>
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: ZiftRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data.clone();
        let request_data = &router_data.request;
        let auth = ZiftAuthType::try_from(&item.router_data.connector_config)?;
        let request_type = if item.router_data.request.is_auto_capture() {
            RequestType::Sale
        } else {
            RequestType::Auth
        };
        let amount = item
            .connector
            .amount_converter
            .convert(request_data.minor_amount, request_data.currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        match item.router_data.request.payment_method_data.clone() {
            PaymentMethodData::Card(card) => {
                match (
                    item.router_data.resource_common_data.is_three_ds(),
                    item.router_data.request.authentication_data.is_some(),
                ) {
                    (true, false) => Err(IntegrationError::NotSupported {
                        message: "3DS flow".to_string(),
                        connector: "Zift",
                        context: Default::default(),
                    }
                    .into()),
                    (true, true) => {
                        let auth_data = item
                            .router_data
                            .request
                            .authentication_data
                            .as_ref()
                            .ok_or(IntegrationError::MissingRequiredField {
                                field_name: "authentication_data",
                                context: Default::default(),
                            })?;

                        let authentication_status = AuthenticationStatus::try_from(auth_data)?;

                        let external_3ds_request = ZiftExternalThreeDsPaymentRequest {
                            request_type,
                            auth,
                            account_number: card.card_number.clone(),
                            account_accessory: card
                                .get_card_expiry_month_year_2_digit_with_delimiter(
                                    "".to_string(),
                                )?,
                            transaction_industry_type: TransactionIndustryType::Ecommerce,
                            transaction_category_code: TransactionCategoryCode::Ecommerce,
                            holder_name: item
                                .router_data
                                .resource_common_data
                                .get_billing_full_name()?,
                            amount,
                            account_type: AccountType::PaymentCard,
                            holder_type: HolderType::Personal,
                            authentication_status,
                            authentication_code: auth_data.ds_trans_id.clone().map(Secret::new),
                            authentication_verification_value: auth_data.cavv.clone().ok_or(
                                IntegrationError::MissingRequiredField {
                                    field_name: "cavv",
                                    context: Default::default(),
                                },
                            )?,
                            authentication_version: auth_data
                                .message_version
                                .as_ref()
                                .map(|v| Secret::new(v.to_string())),
                            transaction_code: item
                                .router_data
                                .resource_common_data
                                .connector_request_reference_id
                                .clone(),
                        };
                        Ok(Self::ExternalThreeDs(external_3ds_request))
                    }
                    _ => {
                        let card_request = ZiftCardPaymentRequest {
                            request_type,
                            auth,
                            account_number: card.card_number.clone(),
                            account_accessory: card
                                .get_card_expiry_month_year_2_digit_with_delimiter(
                                    "".to_string(),
                                )?,
                            transaction_industry_type: TransactionIndustryType::Ecommerce,
                            transaction_category_code: TransactionCategoryCode::Ecommerce,
                            holder_name: item
                                .router_data
                                .resource_common_data
                                .get_billing_full_name()?,
                            amount,
                            account_type: AccountType::PaymentCard,
                            holder_type: HolderType::Personal,
                            csc: card.card_cvc,
                            transaction_code: item
                                .router_data
                                .resource_common_data
                                .connector_request_reference_id
                                .clone(),
                        };
                        Ok(Self::Card(card_request))
                    }
                }
            }
            _ => Err(error_stack::report!(IntegrationError::not_implemented(
                "Payment method".to_string()
            ),)),
        }
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<ZiftAuthPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorResponseTransformationError>;
    fn try_from(
        item: ResponseRouterData<ZiftAuthPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let is_approved = item.response.response_code.is_approved();
        let is_auto_capture = item.router_data.request.is_auto_capture();

        let status = match (is_approved, is_auto_capture) {
            (true, true) => common_enums::AttemptStatus::Charged,
            (true, false) => common_enums::AttemptStatus::Authorized,
            _ if item.response.response_code.is_pending() => common_enums::AttemptStatus::Pending,
            _ => common_enums::AttemptStatus::Failure,
        };

        match status {
            common_enums::AttemptStatus::Failure => Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                response: Err(ErrorResponse {
                    code: item.response.response_code.clone(),
                    message: item.response.response_message.clone(),
                    reason: Some(item.response.response_message.clone()),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: item.response.transaction_id.map(|id| id.to_string()),
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                }),
                ..item.router_data
            }),
            _ => {
                let mandate_reference = if item
                    .router_data
                    .request
                    .is_customer_initiated_mandate_payment()
                {
                    item.response.token.clone().map(|token| {
                        Box::new(MandateReference {
                            connector_mandate_id: Some(token),
                            payment_method_id: None,
                            connector_mandate_request_reference_id: None,
                        })
                    })
                } else {
                    None
                };

                let transaction_id = item.response.get_transaction_id(item.http_code)?;

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        ..item.router_data.resource_common_data
                    },
                    response: Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(transaction_id.to_string()),
                        redirection_data: None,
                        mandate_reference,
                        connector_metadata: None,
                        network_txn_id: None,
                        connector_response_reference_id: item.response.transaction_code.clone(),
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                    }),
                    ..item.router_data
                })
            }
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ZiftRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for ZiftRepeatPaymentsRequest<T>
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: ZiftRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data.clone();
        let request_data = &router_data.request;
        let auth = ZiftAuthType::try_from(&item.router_data.connector_config)?;
        let request_type = if item.router_data.request.is_auto_capture() {
            RequestType::Sale
        } else {
            RequestType::Auth
        };
        let amount = item
            .connector
            .amount_converter
            .convert(request_data.minor_amount, request_data.currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        match item.router_data.request.payment_method_data.clone() {
            PaymentMethodData::MandatePayment => {
                let card_details = match &item.router_data.request.payment_method_data {
                    PaymentMethodData::Card(card) => Ok(card),
                    _ => Err(error_stack::report!(IntegrationError::NotSupported {
                        message: "Payment Method Not Supported".to_string(),
                        connector: "Zift",
                        context: Default::default()
                    })),
                }?;

                let mandate_request = ZiftMandatePaymentRequest {
                    request_type,
                    auth,
                    account_type: AccountType::PaymentCard,
                    token: Secret::new(item.router_data.request.connector_mandate_id().ok_or(
                        IntegrationError::MissingRequiredField {
                            field_name: "connector_mandate_id",
                            context: Default::default(),
                        },
                    )?),
                    account_accessory: card_details
                        .get_card_expiry_month_year_2_digit_with_delimiter("".to_string())?,
                    transaction_industry_type: TransactionIndustryType::Ecommerce,
                    transaction_category_code: TransactionCategoryCode::Ecommerce,
                    holder_name: card_details.get_cardholder_name()?,
                    holder_type: HolderType::Personal,
                    amount,
                    transaction_mode_type: TransactionModeType::CardNotPresent,
                    transaction_category_type: TransactionCategoryType::Recurring,
                    sequence_number: 2, // Its required for MIT
                    transaction_code: item
                        .router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                };
                Ok(Self::Mandate(mandate_request))
            }
            _ => Err(error_stack::report!(IntegrationError::not_implemented(
                "Payment method".to_string()
            ),)),
        }
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<ZiftAuthPaymentsResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorResponseTransformationError>;
    fn try_from(
        item: ResponseRouterData<ZiftAuthPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let is_approved = item.response.response_code.is_approved();
        let is_auto_capture = item.router_data.request.is_auto_capture();

        let status = match (is_approved, is_auto_capture) {
            (true, true) => common_enums::AttemptStatus::Charged,
            (true, false) => common_enums::AttemptStatus::Authorized,
            _ if item.response.response_code.is_pending() => common_enums::AttemptStatus::Pending,
            _ => common_enums::AttemptStatus::Failure,
        };
        match status {
            common_enums::AttemptStatus::Failure => Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                response: Err(ErrorResponse {
                    code: item.response.response_code.clone(),
                    message: item.response.response_message.clone(),
                    reason: Some(item.response.response_message.clone()),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: item.response.transaction_id.map(|id| id.to_string()),
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                }),
                ..item.router_data
            }),

            _ => {
                let transaction_id = item.response.get_transaction_id(item.http_code)?;

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        ..item.router_data.resource_common_data
                    },
                    response: Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(transaction_id.to_string()),
                        redirection_data: None,
                        mandate_reference: None,
                        connector_metadata: None,
                        network_txn_id: None,
                        connector_response_reference_id: item.response.transaction_code.clone(),
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                    }),
                    ..item.router_data
                })
            }
        }
    }
}

impl TryFrom<ResponseRouterData<ZiftSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = Report<ConnectorResponseTransformationError>;
    fn try_from(item: ResponseRouterData<ZiftSyncResponse, Self>) -> Result<Self, Self::Error> {
        let attempt_status = match item.response.transaction_type {
            // Sale transactions
            PaymentRequestType::Sale => match item.response.transaction_status {
                TransactionStatus::Processed => common_enums::AttemptStatus::Charged,
                TransactionStatus::Pending | TransactionStatus::InRebill => {
                    common_enums::AttemptStatus::Pending
                }
                TransactionStatus::Cancelled => common_enums::AttemptStatus::Failure,
            },

            // Auth transactions (sale-auth)
            PaymentRequestType::Auth => match item.response.transaction_status {
                TransactionStatus::Processed => common_enums::AttemptStatus::Authorized,
                TransactionStatus::Pending | TransactionStatus::InRebill => {
                    common_enums::AttemptStatus::Pending
                }
                TransactionStatus::Cancelled => common_enums::AttemptStatus::Failure,
            },

            // Capture transactions
            PaymentRequestType::Capture => match item.response.transaction_status {
                TransactionStatus::Processed => common_enums::AttemptStatus::Charged,
                TransactionStatus::Pending | TransactionStatus::InRebill => {
                    common_enums::AttemptStatus::CaptureInitiated
                }
                TransactionStatus::Cancelled => common_enums::AttemptStatus::CaptureFailed,
            },
        };
        let payments_response = if attempt_status == common_enums::AttemptStatus::Failure {
            Err(ErrorResponse {
                code: item
                    .response
                    .response_code
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: item
                    .response
                    .response_message
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: item.response.response_message,
                status_code: item.http_code,
                attempt_status: Some(attempt_status),
                connector_transaction_id: None,
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            })
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::NoResponseId,
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            })
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: attempt_status,
                ..item.router_data.resource_common_data
            },
            response: payments_response,
            ..item.router_data
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ZiftRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for ZiftSyncRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: ZiftRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = ZiftAuthType::try_from(&item.router_data.connector_config)?;
        Ok(Self {
            request_type: RequestType::Find,
            auth,
            transaction_id: item
                .router_data
                .request
                .connector_transaction_id
                .get_connector_transaction_id_i64()?,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ZiftRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for ZiftCaptureRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: ZiftRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = ZiftAuthType::try_from(&item.router_data.connector_config)?;
        let amount = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.minor_amount_to_capture,
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;
        Ok(Self {
            request_type: RequestType::Capture,
            auth,
            transaction_id: item
                .router_data
                .request
                .connector_transaction_id
                .get_connector_transaction_id_i64()?,
            amount,
        })
    }
}

impl<F> TryFrom<ResponseRouterData<ZiftCaptureResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = Report<ConnectorResponseTransformationError>;
    fn try_from(item: ResponseRouterData<ZiftCaptureResponse, Self>) -> Result<Self, Self::Error> {
        let capture_response = &item.response;

        match capture_response.response_code.is_approved() {
            true => Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::Charged,
                    ..item.router_data.resource_common_data
                },
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::NoResponseId,
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    connector_response_reference_id: None,
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                }),
                ..item.router_data
            }),

            false => Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::CaptureFailed,
                    ..item.router_data.resource_common_data
                },
                response: Err(ErrorResponse {
                    code: capture_response.response_code.clone(),
                    message: capture_response.response_message.clone(),
                    reason: Some(capture_response.response_message.clone()),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                }),
                ..item.router_data
            }),
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ZiftRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for ZiftSetupMandateRequest<T>
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: ZiftRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        if item.router_data.request.amount.unwrap_or(0) > 0 {
            return Err(IntegrationError::FlowNotSupported {
                flow: "Setup Mandate with non zero amount".to_string(),
                connector: "Zift".to_string(),
                context: Default::default(),
            }
            .into());
        }
        let auth = ZiftAuthType::try_from(&item.router_data.connector_config)?;

        let (transaction_industry_type, transaction_category_code, payment_method_details) =
            match &item.router_data.request.payment_method_data {
                PaymentMethodData::Card(card) => (
                    TransactionIndustryType::Ecommerce,
                    TransactionCategoryCode::Ecommerce,
                    SetupMandatePaymentMethod::Card(CardVerificationDetails {
                        account_type: AccountType::PaymentCard,
                        account_number: card.card_number.clone(),
                        account_accessory: card
                            .get_card_expiry_month_year_2_digit_with_delimiter("".to_string())?,
                        csc: card.card_cvc.clone(),
                    }),
                ),
                _ => Err(IntegrationError::NotSupported {
                    message: "Only card supported for mandate setup".to_string(),
                    connector: "Zift",
                    context: Default::default(),
                })?,
            };

        Ok(Self {
            request_type: RequestType::AccountVerification,
            auth,
            transaction_industry_type,
            transaction_category_code,
            holder_name: item
                .router_data
                .resource_common_data
                .get_billing_full_name()?,
            holder_type: HolderType::Personal,
            transaction_code: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            payment_method_details,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<ZiftAuthPaymentsResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = Report<ConnectorResponseTransformationError>;
    fn try_from(
        item: ResponseRouterData<ZiftAuthPaymentsResponse, Self>,
    ) -> Result<Self, Report<ConnectorResponseTransformationError>> {
        let status = if item.response.response_code.is_approved() {
            common_enums::AttemptStatus::Charged
        } else if item.response.response_code.is_pending() {
            common_enums::AttemptStatus::Pending
        } else {
            common_enums::AttemptStatus::Failure
        };
        if status != common_enums::AttemptStatus::Failure {
            let transaction_id = item.response.get_transaction_id(item.http_code)?;

            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(transaction_id.to_string()),
                    redirection_data: None,
                    mandate_reference: item.response.token.clone().map(|token| {
                        Box::new(MandateReference {
                            connector_mandate_id: Some(token),
                            payment_method_id: None,
                            connector_mandate_request_reference_id: None,
                        })
                    }),
                    connector_metadata: None,
                    network_txn_id: None,
                    connector_response_reference_id: item.response.transaction_code.clone(),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                }),
                ..item.router_data
            })
        } else {
            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::Failure,
                    ..item.router_data.resource_common_data
                },
                response: Err(ErrorResponse {
                    code: item.response.response_code.clone(),
                    message: item.response.response_message.clone(),
                    reason: Some(item.response.response_message.clone()),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: item.response.transaction_id.map(|id| id.to_string()),
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                }),
                ..item.router_data
            })
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ZiftRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for ZiftVoidRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: ZiftRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = ZiftAuthType::try_from(&item.router_data.connector_config)?;
        Ok(Self {
            request_type: RequestType::Void,
            auth,
            transaction_id: item
                .router_data
                .request
                .connector_transaction_id
                .get_connector_transaction_id_i64()?,
        })
    }
}

impl<F> TryFrom<ResponseRouterData<ZiftVoidResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = Report<ConnectorResponseTransformationError>;

    fn try_from(item: ResponseRouterData<ZiftVoidResponse, Self>) -> Result<Self, Self::Error> {
        let void_response = &item.response;
        let status = if void_response.response_code.is_approved() {
            common_enums::AttemptStatus::Voided
        } else {
            common_enums::AttemptStatus::Failure
        };
        let response = if void_response.response_code.is_approved() {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::NoResponseId,
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            })
        } else {
            Err(ErrorResponse {
                code: void_response.response_code.to_string(),
                message: void_response.response_message.clone(),
                reason: Some(void_response.response_message.clone()),
                status_code: item.http_code,
                attempt_status: None,
                connector_transaction_id: None,
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            })
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ZiftRouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for ZiftRefundRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: ZiftRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = ZiftAuthType::try_from(&item.router_data.connector_config)?;
        let amount = item
            .connector
            .amount_converter
            .convert(
                MinorUnit::new(item.router_data.request.refund_amount),
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;
        Ok(Self {
            request_type: RequestType::Refund,
            auth,
            transaction_id: item.router_data.request.connector_transaction_id.clone(),
            amount,
            transaction_code: item.router_data.request.refund_id.clone(),
        })
    }
}

impl<F> TryFrom<ResponseRouterData<ZiftRefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = Report<ConnectorResponseTransformationError>;

    fn try_from(item: ResponseRouterData<ZiftRefundResponse, Self>) -> Result<Self, Self::Error> {
        let refund_response = &item.response;

        let refund_status = if refund_response.response_code.is_approved() {
            common_enums::RefundStatus::Success
        } else if refund_response.response_code.is_pending() {
            common_enums::RefundStatus::Pending
        } else {
            common_enums::RefundStatus::Failure
        };
        let response = if refund_response.response_code.is_approved() {
            Ok(RefundsResponseData {
                connector_refund_id: item
                    .response
                    .transaction_id
                    .clone()
                    .or(item.response.transaction_code.clone())
                    .ok_or_else(|| {
                        Report::new(
                            ConnectorResponseTransformationError::response_handling_failed_with_context(
                                item.http_code,
                                Some(
                                    "missing connector refund id in connector response".to_string(),
                                ),
                            ),
                        )
                    })?,
                refund_status,
                status_code: item.http_code,
            })
        } else {
            Err(ErrorResponse {
                code: refund_response.response_code.clone(),
                message: refund_response
                    .response_message
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: refund_response.response_message.clone(),
                status_code: item.http_code,
                attempt_status: None,
                connector_transaction_id: None,
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            })
        };
        Ok(Self {
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}
