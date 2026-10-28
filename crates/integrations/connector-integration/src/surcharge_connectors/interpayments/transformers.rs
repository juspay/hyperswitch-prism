use crate::{types::ResponseRouterData, utils};
use common_enums::{CountryAlpha2, CountryAlpha3};
use common_utils::types::FloatMajorUnit;
use domain_types::{
    connector_flow::{
        SurchargeCalculate, SurchargePaymentSucceeded, SurchargeRefundComplete,
        SurchargeRefundInit, SurchargeRefundSucceeded,
    },
    errors::{ConnectorError, IntegrationError},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    surcharge::surcharge_types::{
        SurchargeCalculateRequest, SurchargeCalculateResponse, SurchargeFlowData,
        SurchargePaymentSucceededRequest, SurchargePaymentSucceededResponse,
        SurchargeRefundCompleteRequest, SurchargeRefundCompleteResponse,
        SurchargeRefundInitRequest, SurchargeRefundInitResponse, SurchargeRefundSucceededRequest,
        SurchargeRefundSucceededResponse, SurchargeStrategy,
    },
};
use error_stack::ResultExt;
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const INTERPAYMENTS_OK_MESSAGE: &str = "ok";
pub struct InterpaymentsAuthType {
    pub api_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for InterpaymentsAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(item: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        if let ConnectorSpecificConfig::Interpayments { api_key, .. } = item {
            Ok(Self {
                api_key: api_key.to_owned(),
            })
        } else {
            Err(IntegrationError::FailedToObtainAuthType {
                context: domain_types::errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to obtain InterPayments authentication credentials".to_string(),
                    ),
                    suggested_action: None,
                    doc_url: None,
                },
            })?
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum InterpaymentsErrorResponse {
    Api(InterpaymentsApiErrorResponse),
    Validation(InterpaymentsValidationErrorResponse),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InterpaymentsApiErrorResponse {
    pub reason_code: String,
    pub reason_codes: Option<Vec<String>>,
    pub reason_code_ids: Option<Vec<i64>>,
    pub code: Option<String>,
    pub message: Option<String>,
    pub reason: Option<String>,
    pub see_also: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct InterpaymentsValidationErrorResponse {
    pub error: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InterPaymentsSurchargeRequest {
    pub amount: FloatMajorUnit,
    pub region: Secret<String>,
    pub country: CountryAlpha3,
    pub data: Option<Vec<InterpaymentsSurchargeStrategy>>,
    // Card Bin
    pub nicn: String,
    pub m_tx_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub enum InterpaymentsSurchargeStrategy {
    #[serde(rename = "waive-surcharge")]
    WaiveSurcharge,
}

impl
    TryFrom<
        &RouterDataV2<
            SurchargeCalculate,
            SurchargeFlowData,
            SurchargeCalculateRequest,
            SurchargeCalculateResponse,
        >,
    > for InterPaymentsSurchargeRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        req: &RouterDataV2<
            SurchargeCalculate,
            SurchargeFlowData,
            SurchargeCalculateRequest,
            SurchargeCalculateResponse,
        >,
    ) -> Result<Self, Self::Error> {
        let amount =
            super::InterPaymentsAmountConvertor::convert(req.request.amount, req.request.currency)?;

        let region = req.request.postal_code.clone();

        let country = req
            .request
            .country
            .map(CountryAlpha2::from_alpha2_to_alpha3)
            .ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                    field_name: "country",
                    context: domain_types::errors::IntegrationErrorContext {
                        additional_context: Some(
                            "Country is required for surcharge calculation with InterPayments"
                                .to_string()
                        ),
                        suggested_action: Some("Provide a valid country code".to_string()),
                        doc_url: None,
                    },
                })
            })?;

        let data = match req.request.surcharge_strategy {
            Some(SurchargeStrategy::Waive) => {
                Some(vec![InterpaymentsSurchargeStrategy::WaiveSurcharge])
            }
            Some(SurchargeStrategy::Apply) | None => None,
        };

        Ok(Self {
            amount,
            region,
            country,
            data,
            nicn: req.request.card_bin.clone(),
            m_tx_id: Some(
                req.resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
        })
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InterPaymentsSurchargeResponse {
    pub transaction_fee: FloatMajorUnit,
    pub s_tx_id: String,
    pub message: String,
    pub reason_code: Option<String>,
    pub transaction_fee_percent: f64,
}

impl TryFrom<ResponseRouterData<InterPaymentsSurchargeResponse, Self>>
    for RouterDataV2<
        SurchargeCalculate,
        SurchargeFlowData,
        SurchargeCalculateRequest,
        SurchargeCalculateResponse,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<InterPaymentsSurchargeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = if item.response.message.to_lowercase() == INTERPAYMENTS_OK_MESSAGE {
            Ok(SurchargeCalculateResponse {
                connector_response_reference_id: None,
                connector_surcharge_id: item.response.s_tx_id.clone(),
                surcharge_amount: super::InterPaymentsAmountConvertor::convert_back(
                    item.response.transaction_fee,
                    item.router_data.request.currency,
                )
                .change_context(utils::response_handling_fail_for_connector(
                    item.http_code,
                    "interpayments",
                ))?,
                surcharge_rate_percent: item.response.transaction_fee_percent,
                currency: item.router_data.request.currency,
            })
        } else {
            Err(ErrorResponse {
                status_code: item.http_code,
                code: item
                    .response
                    .reason_code
                    .clone()
                    .unwrap_or(common_utils::consts::NO_ERROR_CODE.to_string()),
                message: item.response.message.clone(),
                reason: Some(item.response.message.clone()),
                attempt_status: None,
                connector_transaction_id: item.response.s_tx_id.clone().into(),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            })
        };

        Ok(Self {
            response,
            ..item.router_data
        })
    }
}

// Payment Succeeded Flow Types
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InterPaymentsPaymentSucceededRequest {
    pub s_tx_id: String,
}

impl
    From<
        &RouterDataV2<
            SurchargePaymentSucceeded,
            SurchargeFlowData,
            SurchargePaymentSucceededRequest,
            SurchargePaymentSucceededResponse,
        >,
    > for InterPaymentsPaymentSucceededRequest
{
    fn from(
        req: &RouterDataV2<
            SurchargePaymentSucceeded,
            SurchargeFlowData,
            SurchargePaymentSucceededRequest,
            SurchargePaymentSucceededResponse,
        >,
    ) -> Self {
        Self {
            s_tx_id: req.request.connector_surcharge_id.clone(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InterPaymentsPaymentSucceededResponse {
    pub s_tx_id: String,
}

// Refund Succeeded Flow Types
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InterPaymentsRefundSucceededRequest {
    pub s_tx_id: String,
}

impl
    From<
        &RouterDataV2<
            SurchargeRefundSucceeded,
            SurchargeFlowData,
            SurchargeRefundSucceededRequest,
            SurchargeRefundSucceededResponse,
        >,
    > for InterPaymentsRefundSucceededRequest
{
    fn from(
        req: &RouterDataV2<
            SurchargeRefundSucceeded,
            SurchargeFlowData,
            SurchargeRefundSucceededRequest,
            SurchargeRefundSucceededResponse,
        >,
    ) -> Self {
        Self {
            s_tx_id: req.request.connector_surcharge_id.clone(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InterPaymentsRefundSucceededResponse {
    pub s_tx_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InterPaymentsSurchargeRefundInitRequest {
    pub s_tx_id: String,
    pub amount: FloatMajorUnit,
}

impl
    TryFrom<
        &RouterDataV2<
            SurchargeRefundInit,
            SurchargeFlowData,
            SurchargeRefundInitRequest,
            SurchargeRefundInitResponse,
        >,
    > for InterPaymentsSurchargeRefundInitRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        data: &RouterDataV2<
            SurchargeRefundInit,
            SurchargeFlowData,
            SurchargeRefundInitRequest,
            SurchargeRefundInitResponse,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            s_tx_id: data.request.connector_surcharge_id.clone(),
            amount: super::InterPaymentsAmountConvertor::convert(
                data.request.amount,
                data.request.currency,
            )?,
        })
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InterPaymentsSurchargeRefundInitResponse {
    pub refund: FloatMajorUnit,
    pub original_transaction_fee: FloatMajorUnit,
    pub previously_refunded_transaction_fees: FloatMajorUnit,
    pub s_tx_id: String,
}

impl TryFrom<ResponseRouterData<InterPaymentsSurchargeRefundInitResponse, Self>>
    for RouterDataV2<
        SurchargeRefundInit,
        SurchargeFlowData,
        SurchargeRefundInitRequest,
        SurchargeRefundInitResponse,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<InterPaymentsSurchargeRefundInitResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        if response.s_tx_id.trim().is_empty() {
            return Err(error_stack::report!(
                ConnectorError::UnexpectedResponseError {
                    context: domain_types::errors::ResponseTransformationErrorContext {
                        http_status_code: Some(item.http_code),
                        additional_context: Some(
                            "InterPayments refund init returned an empty sTxId".to_string()
                        ),
                    },
                }
            ));
        }
        let currency = item.router_data.request.currency;
        let convert = |amount| {
            super::InterPaymentsAmountConvertor::convert_back(amount, currency).change_context(
                utils::response_handling_fail_for_connector(item.http_code, "interpayments"),
            )
        };
        let refund_amount = convert(response.refund)?;
        let original_transaction_fee = convert(response.original_transaction_fee)?;
        let previously_refunded_transaction_fees =
            convert(response.previously_refunded_transaction_fees)?;
        Ok(Self {
            response: Ok(SurchargeRefundInitResponse {
                connector_surcharge_refund_id: response.s_tx_id,
                refund_amount,
                original_transaction_fee: Some(original_transaction_fee),
                previously_refunded_transaction_fees: Some(previously_refunded_transaction_fees),
                currency: Some(currency),
            }),
            ..item.router_data
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InterPaymentsSurchargeRefundCompleteRequest {
    pub s_tx_id: String,
}

impl
    TryFrom<
        &RouterDataV2<
            SurchargeRefundComplete,
            SurchargeFlowData,
            SurchargeRefundCompleteRequest,
            SurchargeRefundCompleteResponse,
        >,
    > for InterPaymentsSurchargeRefundCompleteRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        data: &RouterDataV2<
            SurchargeRefundComplete,
            SurchargeFlowData,
            SurchargeRefundCompleteRequest,
            SurchargeRefundCompleteResponse,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            s_tx_id: data.request.connector_surcharge_refund_id.clone(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InterPaymentsSurchargeRefundCompleteResponse {
    pub s_tx_id: String,
}

impl TryFrom<ResponseRouterData<InterPaymentsSurchargeRefundCompleteResponse, Self>>
    for RouterDataV2<
        SurchargeRefundComplete,
        SurchargeFlowData,
        SurchargeRefundCompleteRequest,
        SurchargeRefundCompleteResponse,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<InterPaymentsSurchargeRefundCompleteResponse, Self>,
    ) -> Result<Self, Self::Error> {
        if item.response.s_tx_id != item.router_data.request.connector_surcharge_refund_id {
            return Err(error_stack::report!(ConnectorError::IntegrityCheckFailed {
                context: domain_types::errors::ResponseTransformationErrorContext {
                    http_status_code: Some(item.http_code),
                    additional_context: Some(
                        "InterPayments refund completion returned a different sTxId".to_string()
                    ),
                },
                field_names: "sTxId".to_string(),
                connector_transaction_id: Some(item.response.s_tx_id),
            }));
        }
        Ok(Self {
            response: Ok(SurchargeRefundCompleteResponse {
                connector_surcharge_refund_id: item.response.s_tx_id,
            }),
            ..item.router_data
        })
    }
}
