use common_utils::{pii, request::Method, FloatMajorUnit};
use domain_types::{
    connector_flow::{self, Authorize},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId,
    },
    errors::ConnectorError,
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
    utils,
};
use error_stack::ResultExt;
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

use crate::{connectors::dlocal::DlocalRouterData, types::ResponseRouterData};

#[derive(Debug, Default, Eq, PartialEq, Serialize)]
pub struct Payer {
    pub name: Secret<String>,
    pub email: pii::Email,
    pub document: Secret<String>,
}

#[derive(Debug, Default, Eq, Clone, PartialEq, Serialize, Deserialize)]
pub struct Card<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> {
    pub holder_name: Option<Secret<String>>,
    pub number: RawCardNumber<T>,
    pub cvv: Secret<String>,
    pub expiration_month: Secret<String>,
    pub expiration_year: Secret<String>,
    pub capture: String,
}

#[derive(Debug, Default, Eq, PartialEq, Serialize)]
pub struct ThreeDSecureReqData {
    pub force: bool,
}

#[derive(Debug, Serialize, Default, Deserialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum PaymentMethodId {
    #[default]
    Card,
}

#[derive(Debug, Serialize, Default, Deserialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum PaymentMethodFlow {
    #[default]
    Direct,
    ReDirect,
}

#[derive(Default, Debug, Serialize, PartialEq)]
pub struct DlocalPaymentsRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub amount: FloatMajorUnit,
    pub currency: common_enums::Currency,
    pub country: common_enums::CountryAlpha2,
    pub payment_method_id: PaymentMethodId,
    pub payment_method_flow: PaymentMethodFlow,
    pub payer: Payer,
    pub card: Option<Card<T>>,
    pub order_id: String,
    pub three_dsecure: Option<ThreeDSecureReqData>,
    pub callback_url: Option<String>,
    pub description: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        DlocalRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for DlocalPaymentsRequest<T>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: DlocalRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let email = item.router_data.request.get_email()?;
        let address = item
            .router_data
            .resource_common_data
            .get_billing_address()?;
        let country = *address.get_country()?;
        let name = address.get_full_name()?;
        match item.router_data.request.payment_method_data {
            PaymentMethodData::Card(ref ccard) => {
                let should_capture = matches!(
                    item.router_data.request.capture_method,
                    Some(common_enums::CaptureMethod::Automatic)
                        | Some(common_enums::CaptureMethod::SequentialAutomatic)
                );
                let amount = utils::convert_amount(
                    item.connector.amount_converter,
                    item.router_data.request.minor_amount,
                    item.router_data.request.currency,
                )?;
                let payment_request = Self {
                    amount,
                    currency: item.router_data.request.currency,
                    payment_method_id: PaymentMethodId::Card,
                    payment_method_flow: PaymentMethodFlow::Direct,
                    country,
                    payer: Payer {
                        name,
                        email,
                        // [#589]: Allow securely collecting PII from customer in payments request
                        document: get_doc_from_currency(country.to_string()),
                    },
                    card: Some(Card {
                        holder_name: ccard.card_holder_name.clone(),
                        number: ccard.card_number.clone(),
                        cvv: ccard.card_cvc.clone(),
                        expiration_month: ccard.card_exp_month.clone(),
                        expiration_year: ccard.card_exp_year.clone(),
                        capture: should_capture.to_string(),
                    }),
                    order_id: item
                        .router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                    three_dsecure: match item.router_data.resource_common_data.auth_type {
                        common_enums::AuthenticationType::ThreeDs => {
                            Some(ThreeDSecureReqData { force: true })
                        }
                        common_enums::AuthenticationType::NoThreeDs => None,
                    },
                    callback_url: Some(item.router_data.request.get_router_return_url()?),
                    description: item.router_data.resource_common_data.description.clone(),
                };
                Ok(payment_request)
            }
            PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::Wallet(_)
            | PaymentMethodData::PayLater(_)
            | PaymentMethodData::BankRedirect(_)
            | PaymentMethodData::BankDebit(_)
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
            | PaymentMethodData::CardToken(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
                Err(ConnectorError::NotImplemented(
                    crate::utils::get_unimplemented_payment_method_error_message("Dlocal"),
                ))?
            }
        }
    }
}

#[derive(Default, Debug, Serialize, PartialEq)]
pub struct DlocalPaymentsCaptureRequest {
    pub authorization_id: Secret<String>,
    pub amount: FloatMajorUnit,
    pub currency: String,
    pub order_id: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        DlocalRouterData<
            RouterDataV2<
                connector_flow::Capture,
                PaymentFlowData,
                PaymentsCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for DlocalPaymentsCaptureRequest
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: DlocalRouterData<
            RouterDataV2<
                connector_flow::Capture,
                PaymentFlowData,
                PaymentsCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount = utils::convert_amount(
            item.connector.amount_converter,
            item.router_data.request.minor_amount_to_capture,
            item.router_data.request.currency,
        )?;

        Ok(Self {
            authorization_id: Secret::new(
                item.router_data
                    .request
                    .connector_transaction_id
                    .get_connector_transaction_id()
                    .change_context(ConnectorError::MissingConnectorTransactionID)?,
            ),
            amount,
            currency: item.router_data.request.currency.to_string(),
            order_id: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        })
    }
}
// Auth Struct
pub struct DlocalAuthType {
    pub(super) x_login: Secret<String>,
    pub(super) x_trans_key: Secret<String>,
    pub(super) secret: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for DlocalAuthType {
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        if let ConnectorSpecificConfig::Dlocal {
            x_login,
            x_trans_key,
            secret,
            ..
        } = auth_type
        {
            Ok(Self {
                x_login: x_login.to_owned(),
                x_trans_key: x_trans_key.to_owned(),
                secret: secret.to_owned(),
            })
        } else {
            Err(ConnectorError::FailedToObtainAuthType.into())
        }
    }
}
#[derive(Debug, Clone, Eq, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum DlocalPaymentStatus {
    Authorized,
    Paid,
    Cancelled,
    #[default]
    Pending,
    Rejected,
}

impl From<DlocalPaymentStatus> for common_enums::AttemptStatus {
    fn from(item: DlocalPaymentStatus) -> Self {
        match item {
            DlocalPaymentStatus::Authorized => Self::Authorized,
            DlocalPaymentStatus::Paid => Self::Charged,
            DlocalPaymentStatus::Pending => Self::Pending,
            DlocalPaymentStatus::Cancelled => Self::Voided,
            DlocalPaymentStatus::Rejected => Self::Failure,
        }
    }
}

#[derive(Eq, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThreeDSecureResData {
    pub redirect_url: Option<url::Url>,
}

#[derive(Debug, Default, Eq, Clone, PartialEq, Serialize, Deserialize)]
pub struct DlocalPaymentsResponse {
    status: DlocalPaymentStatus,
    id: String,
    three_dsecure: Option<ThreeDSecureResData>,
    order_id: Option<String>,
}

impl<F, T> TryFrom<ResponseRouterData<DlocalPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<DlocalPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let redirection_data = item
            .response
            .three_dsecure
            .and_then(|three_secure_data| three_secure_data.redirect_url)
            .map(|redirect_url| RedirectForm::from((redirect_url, Method::Get)));

        let response = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
            redirection_data: redirection_data.map(Box::new),
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: item.response.order_id.clone(),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: common_enums::AttemptStatus::from(item.response.status),
                ..item.router_data.resource_common_data
            },
            response: Ok(response),
            ..item.router_data
        })
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DlocalPaymentsSyncResponse {
    status: DlocalPaymentStatus,
    id: String,
    order_id: Option<String>,
}

impl<F> TryFrom<ResponseRouterData<DlocalPaymentsSyncResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<DlocalPaymentsSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: common_enums::AttemptStatus::from(item.response.status),
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: item.response.order_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DlocalPaymentsCaptureResponse {
    status: DlocalPaymentStatus,
    id: String,
    order_id: Option<String>,
}

impl<F> TryFrom<ResponseRouterData<DlocalPaymentsCaptureResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<DlocalPaymentsCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: common_enums::AttemptStatus::from(item.response.status),
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: item.response.order_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

pub struct DlocalPaymentsCancelResponse {
    status: DlocalPaymentStatus,
    order_id: String,
}

impl<F> TryFrom<ResponseRouterData<DlocalPaymentsCancelResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<DlocalPaymentsCancelResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: common_enums::AttemptStatus::from(item.response.status),
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.order_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.order_id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// REFUND :
#[derive(Default, Debug, Serialize)]
pub struct DlocalRefundRequest {
    pub amount: FloatMajorUnit,
    pub payment_id: String,
    pub currency: common_enums::Currency,
    pub id: String,
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<DlocalRouterData<RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>, T>>
    for DlocalRefundRequest
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: DlocalRouterData<
            RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount_to_refund = utils::convert_amount(
            item.connector.amount_converter,
            item.router_data.request.minor_refund_amount,
            item.router_data.request.currency,
        )?;

        Ok(Self {
            amount: amount_to_refund,
            payment_id: item.router_data.request.connector_transaction_id.clone(),
            currency: item.router_data.request.currency,
            id: item.router_data.request.refund_id.clone(),
        })
    }
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Default, Deserialize, Clone)]
#[serde(rename_all = "UPPERCASE")]
pub enum RefundStatus {
    Success,
    #[default]
    Pending,
    Rejected,
    Cancelled,
}

impl From<RefundStatus> for common_enums::RefundStatus {
    fn from(item: RefundStatus) -> Self {
        match item {
            RefundStatus::Success => Self::Success,
            RefundStatus::Pending => Self::Pending,
            RefundStatus::Rejected | RefundStatus::Cancelled => Self::Failure,
        }
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct RefundResponse {
    pub id: String,
    pub status: RefundStatus,
}

impl<F> TryFrom<ResponseRouterData<RefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<RefundResponse, Self>) -> Result<Self, Self::Error> {
        let refund_status = common_enums::RefundStatus::from(item.response.status);
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id,
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<RefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<RefundResponse, Self>) -> Result<Self, Self::Error> {
        let refund_status = common_enums::RefundStatus::from(item.response.status);
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id,
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct DlocalErrorResponse {
    pub code: i32,
    pub message: String,
    pub param: Option<String>,
}

fn get_doc_from_currency(country: String) -> Secret<String> {
    let doc = match country.as_str() {
        "BR" => "91483309223",
        "ZA" => "2001014800086",
        "BD" | "GT" | "HN" | "PK" | "SN" | "TH" => "1234567890001",
        "CR" | "SV" | "VN" => "123456789",
        "DO" | "NG" => "12345678901",
        "EG" => "12345678901112",
        "GH" | "ID" | "RW" | "UG" => "1234567890111123",
        "IN" => "NHSTP6374G",
        "CI" => "CA124356789",
        "JP" | "MY" | "PH" => "123456789012",
        "NI" => "1234567890111A",
        "TZ" => "12345678912345678900",
        _ => "12345678",
    };
    Secret::new(doc.to_string())
}
