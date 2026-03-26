use common_enums::{self, enums, AttemptStatus};
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    types::MinorUnit,
};
use domain_types::{
    connector_flow::Authorize,
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData, ResponseId,
    },
    errors::{self},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, WalletData},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

use crate::types::ResponseRouterData;

// Auth
pub struct AmazonpayAuthType {
    pub(super) api_key: Secret<String>,
    pub(super) api_secret: Option<Secret<String>>,
}

impl TryFrom<&ConnectorSpecificConfig> for AmazonpayAuthType {
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Amazonpay {
                api_key,
                api_secret,
                ..
            } => Ok(Self {
                api_key: api_key.to_owned(),
                api_secret: api_secret.to_owned(),
            }),
            _ => Err(errors::ConnectorError::FailedToObtainAuthType.into()),
        }
    }
}

// Requests
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AmazonpayPaymentsRequest {
    pub charge_amount: AmazonpayAmount,
    pub charge_permission_id: Secret<String>,
    pub merchant_metadata: Option<AmazonpayMerchantMetadata>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AmazonpayAmount {
    pub amount: MinorUnit,
    pub currency_code: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AmazonpayMerchantMetadata {
    pub merchant_reference_id: String,
}

// Request TryFrom implementations
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::AmazonpayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AmazonpayPaymentsRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(
        item: super::AmazonpayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        match item.router_data.request.payment_method_data.clone() {
            PaymentMethodData::Wallet(WalletData::AmazonPayDirect(data)) => Ok(Self {
                charge_amount: AmazonpayAmount {
                    amount: item.router_data.request.minor_amount,
                    currency_code: item.router_data.request.currency.to_string(),
                },
                charge_permission_id: data.wallet_token.clone(),
                merchant_metadata: Some(AmazonpayMerchantMetadata {
                    merchant_reference_id: item
                        .router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                }),
            }),
            _ => Err(
                errors::ConnectorError::NotImplemented("Payment method".to_string()).into(),
            ),
        }
    }
}

// Responses
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AmazonpayPaymentsResponse {
    pub charge_id: String,
    pub charge_amount: Option<AmazonpayResponseAmount>,
    pub status_details: AmazonpayStatusDetails,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AmazonpayResponseAmount {
    pub amount: Option<i64>,
    pub currency_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AmazonpayStatusDetails {
    pub state: AmazonpayPaymentStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AmazonpayPaymentStatus {
    Completed,
    Declined,
    Pending,
    CaptureInitiated,
}

impl From<AmazonpayPaymentStatus> for AttemptStatus {
    fn from(item: AmazonpayPaymentStatus) -> Self {
        match item {
            AmazonpayPaymentStatus::Completed => Self::Charged,
            AmazonpayPaymentStatus::CaptureInitiated => Self::CaptureInitiated,
            AmazonpayPaymentStatus::Pending => Self::Pending,
            AmazonpayPaymentStatus::Declined => Self::Failure,
        }
    }
}

// Response TryFrom implementations
impl<F, T> TryFrom<ResponseRouterData<AmazonpayPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
where
    T: Clone,
{
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(
        item: ResponseRouterData<AmazonpayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::from(item.response.status_details.state.clone());
        let response = if status == AttemptStatus::Failure {
            Err(ErrorResponse {
                code: NO_ERROR_CODE.to_string(),
                message: NO_ERROR_MESSAGE.to_string(),
                reason: Some(NO_ERROR_MESSAGE.to_string()),
                attempt_status: Some(status),
                connector_transaction_id: Some(item.response.charge_id.clone()),
                status_code: item.http_code,
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            })
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.response.charge_id.clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.charge_id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            })
        };

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// Sync response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AmazonpaySyncResponse {
    pub charge_id: String,
    pub charge_amount: Option<AmazonpayResponseAmount>,
    pub status_details: AmazonpayStatusDetails,
}

impl<F> TryFrom<ResponseRouterData<AmazonpaySyncResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(item: ResponseRouterData<AmazonpaySyncResponse, Self>) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        let status = AttemptStatus::from(response.status_details.state.clone());
        let response = if status == AttemptStatus::Failure {
            Err(ErrorResponse {
                code: NO_ERROR_CODE.to_string(),
                message: NO_ERROR_MESSAGE.to_string(),
                reason: Some(NO_ERROR_MESSAGE.to_string()),
                attempt_status: Some(status),
                connector_transaction_id: Some(response.charge_id.clone()),
                status_code: http_code,
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            })
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.charge_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: http_code,
            })
        };
        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

// Error
#[derive(Debug, Serialize, Deserialize)]
pub struct AmazonpayErrorResponse {
    pub message: String,
    #[serde(rename = "reasonCode")]
    pub reason_code: Option<String>,
}
