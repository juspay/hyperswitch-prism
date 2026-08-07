use crate::{
    connectors::sanlam_common::transformers::AbsaSanlamBankNames, types::ResponseRouterData,
};
use common_enums::PayoutStatus;
use common_utils::types::StringMajorUnit;
use domain_types::{
    connector_flow::{PayoutGet, PayoutTransfer},
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payouts::{
        payout_method_data::{Bank, PayoutMethodData},
        payouts_types::{
            PayoutFlowData, PayoutGetRequest, PayoutGetResponse, PayoutTransferRequest,
            PayoutTransferResponse,
        },
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    utils::get_unimplemented_payment_method_error_message,
};
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct GotymeSanlamAuthType {
    pub api_key: Secret<String>,
    pub profile_id: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for GotymeSanlamAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::GotymeSanlam {
                api_key,
                profile_id,
                ..
            } => Ok(Self {
                api_key: api_key.clone(),
                profile_id: profile_id.clone(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Ensure the connector is configured with a GotymeSanlam-specific config containing a valid api_key and profile_id.".to_string(),
                    ),
                    additional_context: Some(
                        "ConnectorSpecificConfig did not match the GotymeSanlam variant; received an unexpected config variant.".to_string(),
                    ),
                    doc_url: None,
                },
            }
            .into()),
        }
    }
}

pub struct GotymeSanlamPayoutRouterData<T> {
    pub amount: StringMajorUnit,
    pub router_data: T,
}

impl<T> From<(StringMajorUnit, T)> for GotymeSanlamPayoutRouterData<T> {
    fn from((amount, item): (StringMajorUnit, T)) -> Self {
        Self {
            amount,
            router_data: item,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GotymeSanlamErrorResponse {
    pub error_code: Option<String>,
    pub error_title: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GotymeSanlamPayoutTransferPayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sa_id: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_number: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bank_name: Option<AbsaSanlamBankNames>,
    pub amount: StringMajorUnit,
    pub idempotency_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GotymeSanlamPayoutTransferRequest {
    pub flow: GotymeSanlamPayoutFlow,
    pub payload: GotymeSanlamPayoutTransferPayload,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GotymeSanlamPayoutGetPayload {
    pub idempotency_key: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GotymeSanlamPayoutGetRequest {
    pub flow: GotymeSanlamPayoutFlow,
    pub payload: GotymeSanlamPayoutGetPayload,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GotymeSanlamPayoutFlow {
    PayoutCreate,
    PayoutSync,
}

impl
    TryFrom<
        &GotymeSanlamPayoutRouterData<
            &RouterDataV2<
                PayoutTransfer,
                PayoutFlowData,
                PayoutTransferRequest,
                PayoutTransferResponse,
            >,
        >,
    > for GotymeSanlamPayoutTransferRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        req: &GotymeSanlamPayoutRouterData<
            &RouterDataV2<
                PayoutTransfer,
                PayoutFlowData,
                PayoutTransferRequest,
                PayoutTransferResponse,
            >,
        >,
    ) -> Result<Self, Self::Error> {
        let payload = GotymeSanlamPayoutTransferPayload::try_from(req)?;

        Ok(Self {
            flow: GotymeSanlamPayoutFlow::PayoutCreate,
            payload,
        })
    }
}

impl
    TryFrom<
        &GotymeSanlamPayoutRouterData<
            &RouterDataV2<
                PayoutTransfer,
                PayoutFlowData,
                PayoutTransferRequest,
                PayoutTransferResponse,
            >,
        >,
    > for GotymeSanlamPayoutTransferPayload
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        req: &GotymeSanlamPayoutRouterData<
            &RouterDataV2<
                PayoutTransfer,
                PayoutFlowData,
                PayoutTransferRequest,
                PayoutTransferResponse,
            >,
        >,
    ) -> Result<Self, Self::Error> {
        match req.router_data.request.payout_method_data.as_ref() {
            Some(PayoutMethodData::Bank(Bank::Payshap(payshap))) => {
                let bank_name = payshap
                    .bank_name
                    .map(AbsaSanlamBankNames::try_from)
                    .transpose()?;

                Ok(Self {
                    account_name: payshap.account_holder_name.clone(),
                    sa_id: None,
                    account_number: Some(payshap.bank_account_number.clone()),
                    bank_name,
                    amount: req.amount.clone(),
                    idempotency_key: req
                        .router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                    description: req.router_data.resource_common_data.description.clone(),
                })
            }
            Some(PayoutMethodData::Bank(Bank::PayshapProxy(payshap_proxy))) => Ok(Self {
                account_name: None,
                sa_id: payshap_proxy.shap_id.clone(),
                account_number: None,
                bank_name: None,
                amount: req.amount.clone(),
                idempotency_key: req
                    .router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
                description: req.router_data.resource_common_data.description.clone(),
            }),
            Some(
                PayoutMethodData::Card(_)
                | PayoutMethodData::Bank(_)
                | PayoutMethodData::Wallet(_)
                | PayoutMethodData::BankRedirect(_)
                | PayoutMethodData::Passthrough(_),
            ) => Err(IntegrationError::NotSupported {
                message: get_unimplemented_payment_method_error_message("GotymeSanlam"),
                connector: "GotymeSanlam",
                context: Default::default(),
            })?,
            None => Err(IntegrationError::MissingRequiredField {
                field_name: "payout_method_data",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "GotymeSanlam payout transfer requires PayShap payout method data"
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Provide either `payshap` or `payshap_proxy` as payout method data"
                            .to_string(),
                    ),
                    doc_url: None,
                },
            })?,
        }
    }
}

impl TryFrom<&RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>>
    for GotymeSanlamPayoutGetRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
    ) -> Result<Self, Self::Error> {
        let idempotency_key = req
            .request
            .merchant_payout_id
            .clone()
            .ok_or(IntegrationError::MissingRequiredField {
            field_name: "merchant_payout_id",
            context: IntegrationErrorContext {
                additional_context: Some(
                    "GotymeSanlam payout get requires the idempotency key sent in payout transfer call"
                        .to_string(),
                ),
                suggested_action: Some(
                    "Pass the transfer idempotency key as merchant_payout_id".to_string(),
                ),
                doc_url: None,
            },
        })?;

        Ok(Self {
            flow: GotymeSanlamPayoutFlow::PayoutSync,
            payload: GotymeSanlamPayoutGetPayload { idempotency_key },
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GotymeSanlamPayoutStatus {
    Pending,
    Successful,
    Failed,
    Reversed,
}

impl From<GotymeSanlamPayoutStatus> for PayoutStatus {
    fn from(status: GotymeSanlamPayoutStatus) -> Self {
        match status {
            GotymeSanlamPayoutStatus::Pending => Self::Initiated,
            GotymeSanlamPayoutStatus::Successful => Self::Success,
            GotymeSanlamPayoutStatus::Failed => Self::Failure,
            GotymeSanlamPayoutStatus::Reversed => Self::Reversed,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GotymeSanlamPayoutResponse {
    pub id: Option<String>,
    pub idempotency_key: String,
    pub status: GotymeSanlamPayoutStatus,
    pub created_at: Option<String>,
    pub payment_processor_txn_id: Option<String>,
    pub reason: Option<GotymeSanlamPayoutReason>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GotymeSanlamPayoutReason {
    pub error_code: Option<String>,
    pub error_title: Option<String>,
    pub error_message: Option<String>,
}

fn get_error_response_from_reason(
    reason: GotymeSanlamPayoutReason,
    status_code: u16,
    payout_status: PayoutStatus,
    connector_transaction_id: Option<String>,
) -> ErrorResponse {
    ErrorResponse {
        status_code,
        code: reason
            .error_code
            .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
        message: reason
            .error_message
            .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
        reason: None,
        attempt_status: Some(FlowStatus::Payout(payout_status)),
        connector_transaction_id,
        network_decline_code: None,
        network_advice_code: None,
        network_error_message: None,
    }
}

impl TryFrom<ResponseRouterData<GotymeSanlamPayoutResponse, Self>>
    for RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GotymeSanlamPayoutResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let payout_status = PayoutStatus::from(item.response.status);
        let connector_payout_id = item.response.payment_processor_txn_id.clone();
        let response = match item.response.reason {
            Some(reason) => Err(get_error_response_from_reason(
                reason,
                item.http_code,
                payout_status,
                connector_payout_id.clone(),
            )),
            None => Ok(PayoutTransferResponse {
                merchant_payout_id: item.router_data.request.merchant_payout_id.clone(),
                payout_status,
                connector_payout_id,
                status_code: item.http_code,
            }),
        };

        Ok(Self {
            response,
            ..item.router_data
        })
    }
}

impl TryFrom<ResponseRouterData<GotymeSanlamPayoutResponse, Self>>
    for RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GotymeSanlamPayoutResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let payout_status = PayoutStatus::from(item.response.status);
        let connector_payout_id = item.response.payment_processor_txn_id.clone();
        let response = match item.response.reason {
            Some(reason) => Err(get_error_response_from_reason(
                reason,
                item.http_code,
                payout_status,
                connector_payout_id.clone(),
            )),
            None => Ok(PayoutGetResponse {
                merchant_payout_id: item.router_data.request.merchant_payout_id.clone(),
                payout_status,
                connector_payout_id,
                status_code: item.http_code,
            }),
        };

        Ok(Self {
            response,
            ..item.router_data
        })
    }
}
