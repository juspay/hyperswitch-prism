use super::ForteRouterData;
use common_enums::enums;
use common_enums::BankType;
use common_utils::types::FloatMajorUnit;
use domain_types::{
    connector_flow::{Authorize, Capture, Refund, SetupMandate, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId, SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::{
        BankDebitData, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber,
    },
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    utils,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeOptionInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::types::ResponseRouterData;

type HsInterfacesConnectorRequestError = IntegrationError;

const CAPTURE: &str = "capture";
const VOID: &str = "void";
const REVERSE: &str = "reverse";

impl TryFrom<&Option<serde_json::Value>> for ForteMeta {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(connector_metadata: &Option<serde_json::Value>) -> Result<Self, Self::Error> {
        let config_data = crate::utils::to_connector_meta(connector_metadata.clone())?;
        Ok(config_data)
    }
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum FortePaymentMethod<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    Card(Card<T>),
    Echeck(ForteEcheckWrapper),
}

#[derive(Debug, Serialize)]
pub struct ForteEcheckWrapper {
    echeck: ForteEcheck,
}

#[derive(Debug, Serialize)]
pub struct FortePaymentsRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    action: ForteAction,
    authorization_amount: FloatMajorUnit,
    billing_address: BillingAddress,
    #[serde(flatten)]
    payment_method: FortePaymentMethod<T>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct BillingAddress {
    first_name: Secret<String>,
    last_name: Secret<String>,
}

#[derive(Debug, Serialize)]
pub struct Card<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> {
    card_type: ForteCardType,
    name_on_card: Secret<String>,
    account_number: RawCardNumber<T>,
    expire_month: Secret<String>,
    expire_year: Secret<String>,
    card_verification_value: Secret<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForteCardType {
    Visa,
    MasterCard,
    Amex,
    Discover,
    DinersClub,
    Jcb,
}

#[derive(Debug, Serialize)]
pub struct ForteEcheck {
    sec_code: ForteSecCode,
    #[serde(serialize_with = "serialize_bank_type_pascal")]
    account_type: BankType,
    routing_number: Secret<String>,
    account_number: Secret<String>,
    account_holder: Secret<String>,
}

fn serialize_bank_type_pascal<S>(bank_type: &BankType, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let s = match bank_type {
        BankType::Checking => "Checking",
        BankType::Savings => "Savings",
    };
    serializer.serialize_str(s)
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ForteSecCode {
    WEB,
    PPD,
    TEL,
    CCD,
}

impl TryFrom<utils::CardIssuer> for ForteCardType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(issuer: utils::CardIssuer) -> Result<Self, Self::Error> {
        match issuer {
            utils::CardIssuer::AmericanExpress => Ok(Self::Amex),
            utils::CardIssuer::Master => Ok(Self::MasterCard),
            utils::CardIssuer::Discover => Ok(Self::Discover),
            utils::CardIssuer::Visa => Ok(Self::Visa),
            utils::CardIssuer::DinersClub => Ok(Self::DinersClub),
            utils::CardIssuer::JCB => Ok(Self::Jcb),
            _ => Err(IntegrationError::not_implemented(
                utils::get_unimplemented_payment_method_error_message("Forte"),
            )
            .into()),
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ForteRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for FortePaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: ForteRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        if item.router_data.request.currency != enums::Currency::USD {
            return Err(IntegrationError::NotSupported {
                message: "Only USD currency is supported by Forte".to_string(),
                connector: "Forte",
                context: Default::default(),
            }
            .into());
        }
        match item.router_data.request.payment_method_data {
            PaymentMethodData::Card(ref ccard) => {
                let action = match item.router_data.request.is_auto_capture() {
                    true => ForteAction::Sale,
                    false => ForteAction::Authorize
};
                let card_number = ccard.card_number.peek();
                let card_issuer = utils::get_card_issuer(card_number)?;
                let card_type = ForteCardType::try_from(card_issuer)?;
                let address = item
                    .router_data
                    .resource_common_data
                    .get_billing_address()?;
                let card = Card {
                    card_type,
                    name_on_card: item
                        .router_data
                        .resource_common_data
                        .get_billing_full_name()?,
                    account_number: ccard.card_number.clone(),
                    expire_month: ccard.card_exp_month.clone(),
                    expire_year: ccard.card_exp_year.clone(),
                    card_verification_value: ccard.card_cvc.clone()
};
                let first_name = address.get_first_name()?;
                let billing_address = BillingAddress {
                    first_name: first_name.clone(),
                    last_name: address.get_last_name().unwrap_or(first_name).clone()
};
                let authorization_amount = item
                    .connector
                    .amount_converter
                    .convert(
                        item.router_data.request.minor_amount,
                        item.router_data.request.currency,
                    )
                    .change_context(IntegrationError::RequestEncodingFailed { context: Default::default() })?;
                Ok(Self {
                    action,
                    authorization_amount,
                    billing_address,
                    payment_method: FortePaymentMethod::Card(card)
})
            }
            PaymentMethodData::BankDebit(ref bank_debit_data) => match bank_debit_data {
                BankDebitData::AchBankDebit {
                    account_number,
                    routing_number,
                    bank_account_holder_name,
                    bank_type,
                    ..
                } => {
                    let action = match item.router_data.request.is_auto_capture() {
                        true => ForteAction::Sale,
                        false => ForteAction::Authorize
};

                    let account_holder = bank_account_holder_name
                        .clone()
                        .or(item
                            .router_data
                            .resource_common_data
                            .get_billing_full_name()
                            .ok())
                        .ok_or(IntegrationError::MissingRequiredField {
                            field_name: "bank_account_holder_name",
                context: Default::default()
                        })?;

                    let account_type = bank_type.unwrap_or(BankType::Checking);

                    let echeck = ForteEcheck {
                        sec_code: ForteSecCode::WEB,
                        account_type,
                        routing_number: routing_number.clone(),
                        account_number: account_number.clone(),
                        account_holder
};

                    let address = item
                        .router_data
                        .resource_common_data
                        .get_billing_address()?;
                    let first_name = address.get_first_name()?;
                    let billing_address = BillingAddress {
                        first_name: first_name.clone(),
                        last_name: address.get_last_name().unwrap_or(first_name).clone()
};

                    let authorization_amount = item
                        .connector
                        .amount_converter
                        .convert(
                            item.router_data.request.minor_amount,
                            item.router_data.request.currency,
                        )
                        .change_context(IntegrationError::RequestEncodingFailed { context: Default::default() })?;

                    Ok(Self {
                        action,
                        authorization_amount,
                        billing_address,
                        payment_method: FortePaymentMethod::Echeck(ForteEcheckWrapper { echeck })
})
                }
                BankDebitData::SepaBankDebit { .. } => {
                    Err(IntegrationError::not_implemented(
                        "SEPA bank debit is not supported by Forte. Only ACH (US) bank debits are supported.".to_string(),
                    ))?
                }
                BankDebitData::BecsBankDebit { .. } => {
                    Err(IntegrationError::not_implemented(
                        "BECS bank debit is not supported by Forte. Only ACH (US) bank debits are supported.".to_string(),
                    ))?
                }
                BankDebitData::BacsBankDebit { .. } => {
                    Err(IntegrationError::not_implemented(
                        "BACS bank debit is not supported by Forte. Only ACH (US) bank debits are supported.".to_string(),
                    ))?
                }
                BankDebitData::SepaGuaranteedBankDebit { .. } => {
                    Err(IntegrationError::not_implemented(
                        "SEPA Guaranteed bank debit is not supported by Forte. Only ACH (US) bank debits are supported.".to_string(),
                    ))?
                }
            },
            PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::Wallet(_)
            | PaymentMethodData::PayLater(_)
            | PaymentMethodData::BankRedirect(_)
            | PaymentMethodData::BankTransfer(_)
            | PaymentMethodData::Crypto(_)
            | PaymentMethodData::MandatePayment
            | PaymentMethodData::Reward
            | PaymentMethodData::RealTimePayment(_)
            | PaymentMethodData::MobilePayment(_)
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::PaymentMethodToken(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
                Err(IntegrationError::not_implemented(
                    utils::get_unimplemented_payment_method_error_message("Forte"),
                ))?
            }
        }
    }
}

// Auth Struct
pub struct ForteAuthType {
    pub(super) api_access_id: Secret<String>,
    pub(super) organization_id: Secret<String>,
    pub(super) location_id: Secret<String>,
    pub(super) api_secret_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for ForteAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Forte {
                api_access_id,
                organization_id,
                location_id,
                api_secret_key,
                ..
            } => {
                let organization_id_str = organization_id.peek();
                let location_id_str = location_id.peek();
                Ok(Self {
                    api_access_id: api_access_id.to_owned(),
                    organization_id: if organization_id_str.starts_with("org_") {
                        organization_id.to_owned()
                    } else {
                        Secret::new(format!("org_{organization_id_str}"))
                    },
                    location_id: if location_id_str.starts_with("loc_") {
                        location_id.to_owned()
                    } else {
                        Secret::new(format!("loc_{location_id_str}"))
                    },
                    api_secret_key: api_secret_key.to_owned(),
                })
            }
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            })?,
        }
    }
}
// PaymentsResponse
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FortePaymentStatus {
    Complete,
    Failed,
    Authorized,
    Ready,
    Voided,
    Settled,
}

impl From<FortePaymentStatus> for enums::AttemptStatus {
    fn from(item: FortePaymentStatus) -> Self {
        match item {
            FortePaymentStatus::Complete | FortePaymentStatus::Settled => Self::Charged,
            FortePaymentStatus::Failed => Self::Failure,
            FortePaymentStatus::Ready => Self::Pending,
            FortePaymentStatus::Authorized => Self::Authorized,
            FortePaymentStatus::Voided => Self::Voided,
        }
    }
}

fn get_status(response_code: ForteResponseCode, action: ForteAction) -> enums::AttemptStatus {
    match response_code {
        ForteResponseCode::A01 => match action {
            ForteAction::Authorize => enums::AttemptStatus::Authorized,
            ForteAction::Sale => enums::AttemptStatus::Pending,
            ForteAction::Verify | ForteAction::Capture => enums::AttemptStatus::Charged,
        },
        ForteResponseCode::A05 | ForteResponseCode::A06 => enums::AttemptStatus::Pending,
        _ => enums::AttemptStatus::Failure,
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CardResponse {
    pub name_on_card: Option<Secret<String>>,
    pub last_4_account_number: String,
    pub masked_account_number: String,
    pub card_type: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct EcheckResponse {
    pub account_holder: Option<Secret<String>>,
    pub masked_account_number: Secret<String>,
    pub last_4_account_number: Secret<String>,
    pub routing_number: String,
    pub account_type: String,
    pub sec_code: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ForteResponseCode {
    A01,
    A05,
    A06,
    U13,
    U14,
    U18,
    U20,
    #[serde(other)]
    Unknown,
}

impl From<ForteResponseCode> for enums::AttemptStatus {
    fn from(item: ForteResponseCode) -> Self {
        match item {
            ForteResponseCode::A01 | ForteResponseCode::A05 | ForteResponseCode::A06 => {
                Self::Pending
            }
            ForteResponseCode::U13
            | ForteResponseCode::U14
            | ForteResponseCode::U18
            | ForteResponseCode::U20 => Self::Failure,
            ForteResponseCode::Unknown => Self::Failure,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ResponseStatus {
    pub environment: String,
    pub response_type: String,
    pub response_code: ForteResponseCode,
    pub response_desc: String,
    pub authorization_code: String,
    pub avs_result: Option<String>,
    pub cvv_result: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ForteAction {
    Sale,
    Authorize,
    Verify,
    Capture,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FortePaymentsResponse {
    pub transaction_id: String,
    pub location_id: Secret<String>,
    pub action: ForteAction,
    pub authorization_amount: Option<FloatMajorUnit>,
    pub authorization_code: String,
    pub entered_by: String,
    pub billing_address: Option<BillingAddress>,
    pub card: Option<CardResponse>,
    pub echeck: Option<EcheckResponse>,
    pub response: ResponseStatus,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ForteMeta {
    pub auth_id: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<FortePaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<FortePaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response_code = item.response.response.response_code;
        let action = item.response.action;
        let transaction_id = &item.response.transaction_id;
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: get_status(response_code, action),
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(transaction_id.to_string()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: Some(serde_json::json!(ForteMeta {
                    auth_id: item.response.authorization_code
                })),
                network_txn_id: None,
                connector_response_reference_id: Some(transaction_id.to_string()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

//PsyncResponse
#[derive(Debug, Deserialize, Serialize)]
pub struct FortePaymentsSyncResponse {
    pub transaction_id: String,
    pub organization_id: Secret<String>,
    pub location_id: Secret<String>,
    pub original_transaction_id: Option<String>,
    pub status: FortePaymentStatus,
    pub action: ForteAction,
    pub authorization_code: String,
    pub authorization_amount: Option<FloatMajorUnit>,
    pub billing_address: Option<BillingAddress>,
    pub entered_by: String,
    pub received_date: String,
    pub origination_date: Option<String>,
    pub card: Option<CardResponse>,
    pub echeck: Option<EcheckResponse>,
    pub attempt_number: i64,
    pub response: ResponseStatus,
    pub links: ForteLink,
    pub biller_name: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ForteLink {
    pub disputes: String,
    pub settlements: String,
    #[serde(rename = "self")]
    pub self_url: String,
}

impl<F> TryFrom<ResponseRouterData<FortePaymentsSyncResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<FortePaymentsSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let transaction_id = &item.response.transaction_id;
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: enums::AttemptStatus::from(item.response.status),
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(transaction_id.to_string()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: Some(serde_json::json!(ForteMeta {
                    auth_id: item.response.authorization_code
                })),
                network_txn_id: None,
                connector_response_reference_id: Some(transaction_id.to_string()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// Capture

#[derive(Debug, Serialize)]
pub struct ForteCaptureRequest {
    action: String,
    transaction_id: String,
    authorization_code: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ForteRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for ForteCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: ForteRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let minor_amount_authorized = item
            .router_data
            .resource_common_data
            .minor_amount_capturable
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "amount",
                context: Default::default(),
            })?;

        if item.router_data.request.minor_amount_to_capture != minor_amount_authorized {
            return Err(IntegrationError::NotSupported {
                message: "Forte only supports full captures.".to_string(),
                connector: "Forte",
                context: Default::default(),
            }
            .into());
        }

        let trn_id = item
            .router_data
            .request
            .connector_transaction_id
            .clone()
            .get_connector_transaction_id()
            .change_context(
                HsInterfacesConnectorRequestError::MissingConnectorTransactionID {
                    context: Default::default(),
                },
            )?;
        let connector_auth_id: ForteMeta = ForteMeta::try_from(
            &item
                .router_data
                .resource_common_data
                .connector_feature_data
                .as_ref()
                .map(|s| s.peek().clone()),
        )?;
        let auth_code = connector_auth_id.auth_id;
        Ok(Self {
            action: CAPTURE.to_string(),
            transaction_id: trn_id,
            authorization_code: auth_code,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CaptureResponseStatus {
    pub environment: String,
    pub response_type: String,
    pub response_code: ForteResponseCode,
    pub response_desc: String,
    pub authorization_code: String,
}
// Capture Response
#[derive(Debug, Deserialize, Serialize)]
pub struct ForteCaptureResponse {
    pub transaction_id: String,
    pub original_transaction_id: String,
    pub entered_by: String,
    pub authorization_code: String,
    pub response: CaptureResponseStatus,
}

impl<F, T> TryFrom<ResponseRouterData<ForteCaptureResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<ForteCaptureResponse, Self>) -> Result<Self, Self::Error> {
        let transaction_id = &item.response.transaction_id;
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: enums::AttemptStatus::from(item.response.response.response_code),
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(transaction_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: Some(serde_json::json!(ForteMeta {
                    auth_id: item.response.authorization_code
                })),
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.transaction_id.to_string()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

//Cancel

#[derive(Debug, Serialize)]
pub struct ForteCancelRequest {
    action: String,
    authorization_code: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ForteRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for ForteCancelRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: ForteRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let action = VOID.to_string();
        let metadata: ForteMeta = ForteMeta::try_from(
            &item
                .router_data
                .resource_common_data
                .connector_feature_data
                .as_ref()
                .map(|s| s.peek().clone()),
        )?;
        let authorization_code = metadata.auth_id;
        Ok(Self {
            action,
            authorization_code,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CancelResponseStatus {
    pub response_type: String,
    pub response_code: ForteResponseCode,
    pub response_desc: String,
    pub authorization_code: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ForteCancelResponse {
    pub transaction_id: String,
    pub location_id: Secret<String>,
    pub action: String,
    pub authorization_code: String,
    pub entered_by: String,
    pub response: CancelResponseStatus,
}

impl<F, T> TryFrom<ResponseRouterData<ForteCancelResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<ForteCancelResponse, Self>) -> Result<Self, Self::Error> {
        let transaction_id = &item.response.transaction_id;
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: enums::AttemptStatus::from(item.response.response.response_code),
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(transaction_id.to_string()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: Some(serde_json::json!(ForteMeta {
                    auth_id: item.response.authorization_code
                })),
                network_txn_id: None,
                connector_response_reference_id: Some(transaction_id.to_string()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// REFUND :
#[derive(Default, Debug, Serialize)]
pub struct ForteRefundRequest {
    action: String,
    authorization_amount: FloatMajorUnit,
    original_transaction_id: String,
    authorization_code: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ForteRouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for ForteRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: ForteRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let trn_id = match item
            .router_data
            .request
            .connector_feature_data
            .clone()
            .expose_option()
        {
            Some(metadata) => metadata.as_str().map(|id| id.to_string()),
            None => None,
        }
        .ok_or(HsInterfacesConnectorRequestError::NoConnectorMetaData {
            context: Default::default(),
        })?;
        let connector_auth_id: ForteMeta = ForteMeta::try_from(
            &item
                .router_data
                .request
                .connector_feature_data
                .as_ref()
                .map(|s| s.peek().clone()),
        )?;
        let auth_code = connector_auth_id.auth_id;
        let authorization_amount = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.minor_refund_amount,
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;
        Ok(Self {
            action: REVERSE.to_string(),
            authorization_amount,
            original_transaction_id: trn_id,
            authorization_code: auth_code,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RefundStatus {
    Complete,
    Ready,
    Failed,
}

impl From<RefundStatus> for enums::RefundStatus {
    fn from(item: RefundStatus) -> Self {
        match item {
            RefundStatus::Complete => Self::Success,
            RefundStatus::Ready => Self::Pending,
            RefundStatus::Failed => Self::Failure,
        }
    }
}
impl From<ForteResponseCode> for enums::RefundStatus {
    fn from(item: ForteResponseCode) -> Self {
        match item {
            ForteResponseCode::A01 | ForteResponseCode::A05 | ForteResponseCode::A06 => {
                Self::Pending
            }
            _ => Self::Failure,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RefundResponse {
    pub transaction_id: String,
    pub original_transaction_id: String,
    pub action: String,
    pub authorization_amount: Option<FloatMajorUnit>,
    pub authorization_code: String,
    pub response: ResponseStatus,
}

impl<F> TryFrom<ResponseRouterData<RefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<RefundResponse, Self>) -> Result<Self, Self::Error> {
        Ok(Self {
            resource_common_data: RefundFlowData {
                status: enums::RefundStatus::from(item.response.response.response_code.clone()),
                ..item.router_data.resource_common_data
            },
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.transaction_id,
                refund_status: enums::RefundStatus::from(item.response.response.response_code),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RefundSyncResponse {
    status: RefundStatus,
    transaction_id: String,
}

impl<F> TryFrom<ResponseRouterData<RefundSyncResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<RefundSyncResponse, Self>) -> Result<Self, Self::Error> {
        Ok(Self {
            resource_common_data: RefundFlowData {
                status: enums::RefundStatus::from(item.response.status.clone()),
                ..item.router_data.resource_common_data
            },
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.transaction_id,
                refund_status: enums::RefundStatus::from(item.response.status),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ErrorResponseStatus {
    pub environment: String,
    pub response_type: Option<String>,
    pub response_code: Option<String>,
    pub response_desc: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ForteErrorResponse {
    pub response: Option<ErrorResponseStatus>,
}

// ===== SETUP MANDATE (SetupRecurring) =====
// Forte does not expose a true tokenize / stored-credential endpoint; the
// /transactions resource supports only Sale/Authorize/Capture/Verify. The
// "verify" action requires a specific sandbox entitlement that our test
// account does not have, so SetupMandate uses action=authorize with a
// nominal $1.00 amount. The resulting transaction_id can be used by the
// caller to void the hold if desired. Reuses the Authorize request shape.

#[derive(Debug, Serialize)]
pub struct ForteSetupMandateCardWrapper<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    card: Card<T>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ForteSetupMandatePaymentMethod<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    Card(ForteSetupMandateCardWrapper<T>),
    Echeck(ForteEcheckWrapper),
}

#[derive(Debug, Serialize)]
pub struct ForteSetupMandateRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    action: ForteAction,
    authorization_amount: FloatMajorUnit,
    billing_address: BillingAddress,
    #[serde(flatten)]
    payment_method: ForteSetupMandatePaymentMethod<T>,
}

pub type ForteSetupMandateResponse = FortePaymentsResponse;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ForteRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for ForteSetupMandateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: ForteRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        if item.router_data.request.currency != enums::Currency::USD {
            return Err(IntegrationError::NotSupported {
                message: "Only USD currency is supported by Forte".to_string(),
                connector: "Forte",
                context: Default::default(),
            }
            .into());
        }
        match item.router_data.request.payment_method_data {
            PaymentMethodData::Card(ref ccard) => {
                let card_number = ccard.card_number.peek();
                let card_issuer = utils::get_card_issuer(card_number)?;
                let card_type = ForteCardType::try_from(card_issuer)?;
                let address = item
                    .router_data
                    .resource_common_data
                    .get_billing_address()?;
                let card = Card {
                    card_type,
                    name_on_card: item
                        .router_data
                        .resource_common_data
                        .get_billing_full_name()?,
                    account_number: ccard.card_number.clone(),
                    expire_month: ccard.card_exp_month.clone(),
                    expire_year: ccard.card_exp_year.clone(),
                    card_verification_value: ccard.card_cvc.clone(),
                };
                let first_name = address.get_first_name()?;
                let billing_address = BillingAddress {
                    first_name: first_name.clone(),
                    last_name: address.get_last_name().unwrap_or(first_name).clone(),
                };
                // Forte SetupMandate uses action=authorize with a nominal
                // $1.00 authorization_amount. Verify action is not enabled
                // on the sandbox account used for integration testing.
                let minor_amount = item
                    .router_data
                    .request
                    .minor_amount
                    .unwrap_or_else(|| common_utils::types::MinorUnit::new(100));
                let authorization_amount = item
                    .connector
                    .amount_converter
                    .convert(minor_amount, item.router_data.request.currency)
                    .change_context(IntegrationError::RequestEncodingFailed {
                        context: Default::default(),
                    })?;
                Ok(Self {
                    action: ForteAction::Authorize,
                    authorization_amount,
                    billing_address,
                    payment_method: ForteSetupMandatePaymentMethod::Card(
                        ForteSetupMandateCardWrapper { card },
                    ),
                })
            }
            PaymentMethodData::BankDebit(ref bank_debit_data) => match bank_debit_data {
                BankDebitData::AchBankDebit {
                    account_number,
                    routing_number,
                    bank_account_holder_name,
                    bank_type,
                    ..
                } => {
                    let account_holder = bank_account_holder_name
                        .clone()
                        .or(item
                            .router_data
                            .resource_common_data
                            .get_billing_full_name()
                            .ok())
                        .ok_or(IntegrationError::MissingRequiredField {
                            field_name: "bank_account_holder_name",
                            context: Default::default(),
                        })?;
                    let account_type = bank_type.unwrap_or(BankType::Checking);
                    let echeck = ForteEcheck {
                        sec_code: ForteSecCode::WEB,
                        account_type,
                        routing_number: routing_number.clone(),
                        account_number: account_number.clone(),
                        account_holder,
                    };
                    let address = item
                        .router_data
                        .resource_common_data
                        .get_billing_address()?;
                    let first_name = address.get_first_name()?;
                    let billing_address = BillingAddress {
                        first_name: first_name.clone(),
                        last_name: address.get_last_name().unwrap_or(first_name).clone(),
                    };
                    let minor_amount = item
                        .router_data
                        .request
                        .minor_amount
                        .unwrap_or_else(|| common_utils::types::MinorUnit::new(100));
                    let authorization_amount = item
                        .connector
                        .amount_converter
                        .convert(minor_amount, item.router_data.request.currency)
                        .change_context(IntegrationError::RequestEncodingFailed {
                            context: Default::default(),
                        })?;
                    Ok(Self {
                        action: ForteAction::Authorize,
                        authorization_amount,
                        billing_address,
                        payment_method: ForteSetupMandatePaymentMethod::Echeck(
                            ForteEcheckWrapper { echeck },
                        ),
                    })
                }
                _ => Err(IntegrationError::not_implemented(
                    "Only ACH (US) bank debits are supported for Forte SetupMandate.".to_string(),
                ))?,
            },
            _ => Err(IntegrationError::not_implemented(
                utils::get_unimplemented_payment_method_error_message("Forte"),
            ))?,
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<ForteSetupMandateResponse, Self>>
    for RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<ForteSetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response_code = item.response.response.response_code;
        let action = item.response.action;
        let transaction_id = &item.response.transaction_id;
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: get_status(response_code, action),
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(transaction_id.to_string()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: Some(serde_json::json!(ForteMeta {
                    auth_id: item.response.authorization_code
                })),
                network_txn_id: None,
                connector_response_reference_id: Some(transaction_id.to_string()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}
