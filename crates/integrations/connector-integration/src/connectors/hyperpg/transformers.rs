use crate::{connectors::hyperpg::HyperpgRouterData, types::ResponseRouterData};
use common_enums::{AttemptStatus, RefundStatus};
use common_utils::{request::Method, AmountConvertor, FloatMajorUnit, FloatMajorUnitForConnector};
use domain_types::errors::{ConnectorError, IntegrationError};
use domain_types::router_response_types::RedirectForm;
use domain_types::{
    connector_flow::{Authorize, PSync, RSync, Refund},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, ResponseId,
    },
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    utils,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const JSON: &str = "json";

#[derive(Debug, Clone)]
pub struct HyperpgAuthType {
    pub username: Secret<String>,
    pub password: Secret<String>,
    pub merchant_id: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for HyperpgAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Hyperpg {
                username,
                password,
                merchant_id,
                ..
            } => Ok(Self {
                username: username.to_owned(),
                password: password.to_owned(),
                merchant_id: merchant_id.to_owned(),
            }),
            _other => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
            )),
        }
    }
}

// ===== ERROR RESPONSE =====
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperpgErrorResponse {
    pub error_message: Option<String>,
    pub status: Option<String>,
    pub error_code: Option<String>,
    pub error_info: Option<HyperpgErrorInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperpgErrorInfo {
    pub user_message: Option<String>,
    pub fields: Option<Vec<HyperpgErrorField>>,
    pub request_id: Option<String>,
    pub developer_message: Option<String>,
    pub code: Option<String>,
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperpgErrorField {
    pub field_name: Option<String>,
    pub reason: Option<String>,
}

// ===== STATUS ENUMS =====
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HyperpgPaymentStatus {
    New,
    Pending,
    PendingVbv,
    Charged,
    Failed,
    Cancelled,
    AuthenticationFailed,
    AuthorizationFailed,
    Authorizing,
}

impl From<&HyperpgPaymentStatus> for AttemptStatus {
    fn from(status: &HyperpgPaymentStatus) -> Self {
        match status {
            HyperpgPaymentStatus::Charged => Self::Charged,
            HyperpgPaymentStatus::New
            | HyperpgPaymentStatus::Authorizing
            | HyperpgPaymentStatus::Pending
            | HyperpgPaymentStatus::PendingVbv => Self::Pending,
            HyperpgPaymentStatus::Failed => Self::Failure,
            HyperpgPaymentStatus::Cancelled => Self::Voided,
            HyperpgPaymentStatus::AuthorizationFailed => Self::AuthorizationFailed,
            HyperpgPaymentStatus::AuthenticationFailed => Self::AuthenticationFailed,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum HyperpgRefundStatus {
    Pending,
    Success,
    Failed,
}

impl From<&HyperpgRefundStatus> for RefundStatus {
    fn from(status: &HyperpgRefundStatus) -> Self {
        match status {
            HyperpgRefundStatus::Success => Self::Success,
            HyperpgRefundStatus::Pending => Self::Pending,
            HyperpgRefundStatus::Failed => Self::Failure,
        }
    }
}

// ===== REQUEST TYPES =====

#[derive(Debug, Serialize)]
pub struct HyperpgAuthorizeRequest {
    pub merchant_id: String,
    pub payment_method_type: HyperpgPaymentMethodType,
    pub card_number: Option<Secret<String>>,
    pub card_security_code: Option<Secret<String>>,
    pub card_exp_month: Option<String>,
    pub card_exp_year: Option<String>,
    pub name_on_card: Option<String>,
    pub format: String,
    pub redirect_after_payment: bool,
    pub order: HyperpgOrderData,
}

#[derive(Debug, Serialize)]
pub enum HyperpgPaymentMethodType {
    CARD,
}

#[derive(Debug, Serialize)]
pub struct HyperpgOrderData {
    pub order_id: String,
    pub amount: FloatMajorUnit,
    pub currency: String,
    pub return_url: Option<String>,
}

impl<T: PaymentMethodDataTypes + fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        HyperpgRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for HyperpgAuthorizeRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: HyperpgRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = wrapper.router_data;

        let payment_method_data = router_data.request.payment_method_data.clone();

        let (
            payment_method_type,
            card_number,
            name_on_card,
            card_exp_month,
            card_exp_year,
            card_security_code,
        ) = match payment_method_data {
            PaymentMethodData::Card(card) => {
                let card_number = Some(Secret::new(card.card_number.peek().to_string()));
                let card_exp_month = Some(card.card_exp_month.peek().clone());
                let card_exp_year = Some(card.card_exp_year.peek().clone());
                let card_security_code = Some(card.card_cvc.clone());
                let name_on_card = card.card_holder_name.as_ref().map(|n| n.peek().clone());

                (
                    HyperpgPaymentMethodType::CARD,
                    card_number,
                    name_on_card,
                    card_exp_month,
                    card_exp_year,
                    card_security_code,
                )
            }
            PaymentMethodData::Wallet(_wallet) => {
                return Err(error_stack::report!(IntegrationError::NotImplemented(
                    "Wallet payment method support is not yet implemented".to_string(),
                    Default::default()
                )));
            }
            PaymentMethodData::PayLater(_paylater) => {
                return Err(error_stack::report!(IntegrationError::NotSupported {
                    message: "PayLater payment method is not supported".to_string(),
                    connector: "hyperpg",
                    context: Default::default()
                }));
            }
            PaymentMethodData::Voucher(_voucher) => {
                return Err(error_stack::report!(IntegrationError::NotImplemented(
                    "Voucher payment method support is not yet implemented".to_string(),
                    Default::default()
                )));
            }
            _ => {
                return Err(error_stack::report!(IntegrationError::NotImplemented(
                    "This payment method is not implemented".to_string(),
                    Default::default()
                )));
            }
        };

        // Convert amount using the connector's amount_converter
        let amount = utils::convert_amount(
            wrapper.connector.amount_converter,
            router_data.request.amount,
            router_data.request.currency,
        )?;

        let auth_type = HyperpgAuthType::try_from(&router_data.connector_config)?;

        Ok(Self {
            merchant_id: auth_type.merchant_id.peek().to_string(),
            payment_method_type,
            card_number,
            card_security_code,
            card_exp_month,
            card_exp_year,
            name_on_card,
            format: JSON.to_string(),
            redirect_after_payment: true,
            order: HyperpgOrderData {
                order_id: router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
                amount,
                currency: router_data.request.currency.to_string(),
                return_url: router_data.request.router_return_url.clone(),
            },
        })
    }
}

#[derive(Debug, Serialize)]
pub struct HyperpgVoidRequest {
    pub unique_request_id: String,
    pub amount: FloatMajorUnit,
}

#[derive(Debug, Serialize)]
pub struct HyperpgRefundRequest {
    pub unique_request_id: String,
    pub amount: FloatMajorUnit,
}

impl<T: PaymentMethodDataTypes + fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        HyperpgRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for HyperpgRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: HyperpgRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = wrapper.router_data;

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
            unique_request_id: router_data.request.connector_transaction_id.clone(),
            amount,
        })
    }
}

// ===== RESPONSE TYPES =====

#[derive(Debug, Deserialize, Serialize)]
pub struct HyperpgAuthorizeResponse {
    pub order_id: String,
    pub status: HyperpgPaymentStatus,
    pub txn_id: String,
    pub payment: Option<PaymentResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PaymentResponse {
    pub authentication: Option<Authentication>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Authentication {
    pub url: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct HyperpgSyncResponse {
    pub order_id: String,
    pub status: HyperpgPaymentStatus,
    pub txn_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct HyperpgRefundSyncResponse {
    pub order_id: String,
    pub txn_id: Option<String>,
    pub refunded: bool,
    pub amount_refunded: FloatMajorUnit,
    pub refunds: Option<Vec<HyperpgRefundItem>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct HyperpgRefundResponse {
    pub order_id: String,
    pub refunded: bool,
    pub amount_refunded: FloatMajorUnit,
    pub refunds: Option<Vec<HyperpgRefundItem>>,
    pub txn_id: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct HyperpgRefundItem {
    pub status: HyperpgRefundStatus,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HyperpgMeta {
    pub order_id: Option<String>,
}

// ===== RESPONSE TRANSFORMERS =====

impl<T: PaymentMethodDataTypes + fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<HyperpgAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<HyperpgAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = item.router_data;

        let status = AttemptStatus::from(&response.status);

        let connector_metadata = serde_json::json!(HyperpgMeta {
            order_id: Some(response.order_id.clone())
        });

        let redirection_data = response.payment.as_ref().and_then(|links| {
            links.authentication.as_ref().map(|authentication| {
                Box::new(RedirectForm::Form {
                    endpoint: authentication.url.clone(),
                    method: Method::Get,
                    form_fields: Default::default(),
                })
            })
        });

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.txn_id.clone()),
                connector_response_reference_id: Some(response.order_id.clone()),
                redirection_data,
                mandate_reference: None,
                connector_metadata: Some(connector_metadata),
                network_txn_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data.clone()
            },
            ..router_data
        })
    }
}

impl TryFrom<ResponseRouterData<HyperpgSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<HyperpgSyncResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = item.router_data;

        let status = AttemptStatus::from(&response.status);

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::NoResponseId,
                connector_response_reference_id: None,
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data.clone()
            },
            ..router_data
        })
    }
}

impl TryFrom<ResponseRouterData<HyperpgRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<HyperpgRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        // doc - The status of the refund initiated. Initial status will always be PENDING - doc link - https://docs.hyperpg.in/integration-doc/docs/base-integration/refund-order-api
        let refund_status = RefundStatus::Pending;

        let connector_refund_id = response.order_id.clone();

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: item.http_code,
            }),
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..item.router_data.resource_common_data.clone()
            },
            ..item.router_data
        })
    }
}

impl TryFrom<ResponseRouterData<HyperpgRefundSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<HyperpgRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;

        let refund_previous_status = item.router_data.resource_common_data.status;

        let refund_status = response
            .refunds
            .as_ref()
            .and_then(|refunds| refunds.first())
            .map(|refund| RefundStatus::from(&refund.status))
            .unwrap_or(refund_previous_status);

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: response.order_id.clone(),
                refund_status,
                status_code: item.http_code,
            }),
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..item.router_data.resource_common_data.clone()
            },
            ..item.router_data
        })
    }
}
