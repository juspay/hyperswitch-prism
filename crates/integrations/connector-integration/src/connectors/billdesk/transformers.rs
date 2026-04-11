use common_enums::AttemptStatus;
use domain_types::{
    connector_flow::{Authorize, PSync, RSync, Refund},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, ResponseId,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::{BankRedirectData, PaymentMethodData, PaymentMethodDataTypes, UpiData},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::{report, Report};
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::types::ResponseRouterData;

pub struct BilldeskAuthType {
    pub merchant_id: Secret<String>,
    pub secret_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for BilldeskAuthType {
    type Error = Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Billdesk {
                api_key,
                secret_key,
                ..
            } => Ok(Self {
                merchant_id: api_key.clone(),
                secret_key: secret_key.clone(),
            }),
            _ => Err(report!(IntegrationError::FailedToObtainAuthType {
                context: Default::default()
            })),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BilldeskErrorResponse {
    #[serde(rename = "error_code")]
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct BilldeskPaymentObject {
    pub payment_method_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vpa_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BilldeskDevice {
    pub init_channel: String,
    pub ip: String,
    pub user_agent: String,
}

#[derive(Debug, Serialize)]
pub struct BilldeskPaymentsRequest {
    pub mercid: String,
    pub orderid: String,
    pub amount: String,
    pub currency: String,
    pub ru: String,
    pub itemcode: String,
    pub txnid: String,
    pub payment: BilldeskPaymentObject,
    pub device: BilldeskDevice,
}

fn get_billdesk_payment_object<T: PaymentMethodDataTypes>(
    item: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
) -> Result<BilldeskPaymentObject, Report<IntegrationError>> {
    match &item.request.payment_method_data {
        PaymentMethodData::Upi(upi_data) => match upi_data {
            UpiData::UpiIntent(_) => Ok(BilldeskPaymentObject {
                payment_method_type: "UPI".to_string(),
                payment_method: None,
                flow: Some("intent".to_string()),
                vpa_id: None,
            }),
            UpiData::UpiCollect(data) => Ok(BilldeskPaymentObject {
                payment_method_type: "UPI".to_string(),
                payment_method: None,
                flow: Some("collect".to_string()),
                vpa_id: data
                    .vpa_id
                    .as_ref()
                    .map(|v| v.clone().expose()),
            }),
            UpiData::UpiQr(_) => Ok(BilldeskPaymentObject {
                payment_method_type: "UPI".to_string(),
                payment_method: None,
                flow: Some("qr".to_string()),
                vpa_id: None,
            }),
        },
        PaymentMethodData::BankRedirect(bank_redirect) => match bank_redirect {
            BankRedirectData::Netbanking { issuer } => Ok(BilldeskPaymentObject {
                payment_method_type: "NET_BANKING".to_string(),
                payment_method: Some(issuer.to_string()),
                flow: None,
                vpa_id: None,
            }),
            _ => Err(report!(IntegrationError::NotSupported {
                message: "Unsupported bank redirect type".to_string(),
                connector: "billdesk",
                context: Default::default(),
            })),
        },
        _ => Err(report!(IntegrationError::NotSupported {
            message: "Unsupported payment method".to_string(),
            connector: "billdesk",
            context: Default::default(),
        })),
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        crate::connectors::billdesk::BilldeskRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for BilldeskPaymentsRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        wrapper: crate::connectors::billdesk::BilldeskRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &wrapper.router_data;
        let auth = BilldeskAuthType::try_from(&item.connector_config)?;
        let payment = get_billdesk_payment_object(item)?;

        let amount_f64 = item.request.amount.0 as f64 / 100.0;

        let return_url = item
            .request
            .router_return_url
            .clone()
            .unwrap_or_default();

        Ok(Self {
            mercid: auth.merchant_id.expose(),
            orderid: item
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            amount: format!("{:.2}", amount_f64),
            currency: item.request.currency.to_string(),
            ru: return_url,
            itemcode: "DIRECT".to_string(),
            txnid: item.resource_common_data.payment_id.clone(),
            payment,
            device: BilldeskDevice {
                init_channel: "internet".to_string(),
                ip: item
                    .request
                    .browser_info
                    .as_ref()
                    .and_then(|b| b.ip_address.as_ref())
                    .map(|ip| ip.to_string())
                    .unwrap_or_else(|| "127.0.0.1".to_string()),
                user_agent: item
                    .request
                    .browser_info
                    .as_ref()
                    .and_then(|b| b.user_agent.clone())
                    .unwrap_or_else(|| "Mozilla/5.0".to_string()),
            },
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BilldeskLink {
    pub href: Option<String>,
    pub rel: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BilldeskPaymentsResponse {
    #[serde(rename = "transactionid")]
    pub transaction_id: Option<String>,
    pub auth_status: Option<String>,
    pub next_step: Option<String>,
    pub links: Option<Vec<BilldeskLink>>,
    pub transaction_error_type: Option<String>,
    pub transaction_error_desc: Option<String>,
}

fn get_redirect_url(links: &Option<Vec<BilldeskLink>>) -> Option<String> {
    links.as_ref().and_then(|links| {
        links
            .iter()
            .find(|l| l.rel.as_deref() == Some("redirect"))
            .and_then(|l| l.href.clone())
    })
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ResponseRouterData<
            BilldeskPaymentsResponse,
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        >,
    >
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            BilldeskPaymentsResponse,
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        >,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let (status, redirection_data) = match response.auth_status.as_deref() {
            Some("0300") => (AttemptStatus::Charged, None),
            Some("0002") => {
                if let Some(url) = get_redirect_url(&response.links) {
                    let parsed = url::Url::parse(&url).map_err(|_| {
                        ConnectorError::response_handling_failed_with_context(
                            item.http_code,
                            Some("Failed to parse redirect URL".to_string()),
                        )
                    })?;
                    (
                        AttemptStatus::AuthenticationPending,
                        Some(Box::new(RedirectForm::Form {
                            endpoint: parsed.to_string(),
                            method: common_utils::Method::Get,
                            form_fields: std::collections::HashMap::from_iter(
                                parsed
                                    .query_pairs()
                                    .map(|(k, v)| (k.to_string(), v.to_string())),
                            ),
                        })),
                    )
                } else {
                    (AttemptStatus::Pending, None)
                }
            }
            Some("0399") => (AttemptStatus::AuthorizationFailed, None),
            _ => (AttemptStatus::Pending, None),
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    response.transaction_id.clone().unwrap_or_default(),
                ),
                redirection_data,
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

#[derive(Debug, Serialize)]
pub struct BilldeskSyncRequest {
    pub mercid: String,
    pub transactionid: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        crate::connectors::billdesk::BilldeskRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for BilldeskSyncRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        wrapper: crate::connectors::billdesk::BilldeskRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &wrapper.router_data;
        let auth = BilldeskAuthType::try_from(&item.connector_config)?;
        Ok(Self {
            mercid: auth.merchant_id.expose(),
            transactionid: item
                .request
                .get_connector_transaction_id()
                .map_err(|_| {
                    report!(IntegrationError::MissingConnectorTransactionID { context: Default::default() })
                })?
                .to_string(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BilldeskSyncResponse {
    #[serde(rename = "transactionid")]
    pub transaction_id: Option<String>,
    pub auth_status: Option<String>,
    pub transaction_error_type: Option<String>,
    pub transaction_error_desc: Option<String>,
}

impl
    TryFrom<
        ResponseRouterData<
            BilldeskSyncResponse,
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        >,
    > for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            BilldeskSyncResponse,
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let status = match response.auth_status.as_deref() {
            Some("0300") => AttemptStatus::Charged,
            Some("0002") => AttemptStatus::Pending,
            Some("0399") => AttemptStatus::AuthorizationFailed,
            _ => AttemptStatus::Pending,
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    response.transaction_id.clone().unwrap_or_default(),
                ),
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

#[derive(Debug, Serialize)]
pub struct BilldeskRefundRequest {
    pub mercid: String,
    pub refund_amount: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refund_desc: Option<String>,
    pub refund_ref_id: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        crate::connectors::billdesk::BilldeskRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for BilldeskRefundRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        wrapper: crate::connectors::billdesk::BilldeskRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &wrapper.router_data;
        let auth = BilldeskAuthType::try_from(&item.connector_config)?;

        let amount_f64 = item.request.minor_refund_amount.0 as f64 / 100.0;

        Ok(Self {
            mercid: auth.merchant_id.expose(),
            refund_amount: format!("{:.2}", amount_f64),
            refund_desc: item.request.reason.clone(),
            refund_ref_id: item.request.refund_id.clone(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BilldeskRefundResponse {
    pub refundid: Option<String>,
    pub refund_status: Option<String>,
    pub transaction_error_type: Option<String>,
    pub transaction_error_desc: Option<String>,
}

fn get_refund_status(status: Option<&str>) -> common_enums::RefundStatus {
    match status {
        Some("0700") => common_enums::RefundStatus::Success,
        Some("0799") => common_enums::RefundStatus::Failure,
        _ => common_enums::RefundStatus::Pending,
    }
}

impl TryFrom<
        ResponseRouterData<
            BilldeskRefundResponse,
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        >,
    > for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            BilldeskRefundResponse,
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let refund_status = get_refund_status(response.refund_status.as_deref());

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: response
                    .refundid
                    .clone()
                    .unwrap_or_else(|| item.router_data.request.refund_id.clone()),
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

#[derive(Debug, Serialize)]
pub struct BilldeskRSyncRequest {
    pub mercid: String,
    pub refundid: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        crate::connectors::billdesk::BilldeskRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for BilldeskRSyncRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        wrapper: crate::connectors::billdesk::BilldeskRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &wrapper.router_data;
        let auth = BilldeskAuthType::try_from(&item.connector_config)?;
        Ok(Self {
            mercid: auth.merchant_id.expose(),
            refundid: item.request.connector_refund_id.clone(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BilldeskRSyncResponse {
    pub refundid: Option<String>,
    pub refund_status: Option<String>,
    pub transaction_error_type: Option<String>,
    pub transaction_error_desc: Option<String>,
}

impl TryFrom<
        ResponseRouterData<
            BilldeskRSyncResponse,
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        >,
    > for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            BilldeskRSyncResponse,
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let refund_status = get_refund_status(response.refund_status.as_deref());

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: response
                    .refundid
                    .clone()
                    .unwrap_or_else(|| item.router_data.request.connector_refund_id.clone()),
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}
