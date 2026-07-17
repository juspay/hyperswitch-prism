use crate::{types::ResponseRouterData, utils};
use common_enums::{CountryAlpha2, CountryAlpha3};
use common_utils::types::FloatMajorUnit;
use domain_types::{
    connector_flow::{SurchargeCalculate, SurchargePaymentSucceeded, SurchargeRefundSucceeded},
    errors::{ConnectorError, IntegrationError},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    surcharge::surcharge_types::{
        SurchargeCalculateRequest, SurchargeCalculateResponse, SurchargeFlowData,
        SurchargePaymentSucceededRequest, SurchargePaymentSucceededResponse,
        SurchargeRefundSucceededRequest, SurchargeRefundSucceededResponse, SurchargeStrategy,
    },
};
use error_stack::ResultExt;
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

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
#[serde(rename_all = "camelCase")]
pub struct InterpaymentsErrorResponse {
    pub reason_code: String,
    pub message: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InterPaymentsSurchargeRequest {
    pub amount: FloatMajorUnit,
    pub region: Secret<String>,
    pub country: CountryAlpha3,
    // Card Bin
    pub nicn: String,
    pub m_tx_id: Option<String>,
    pub data: Option<Vec<InterpaymentsSurchargeStrategy>>,
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
            Some(SurchargeStrategy::WaiveSurcharge::Waive) => {
                Some(vec![InterpaymentsSurchargeStrategy::WaiveSurcharge])
            }
            Some(SurchargeStrategy::WaiveSurcharge::Apply) | None => None,
        };

        Ok(Self {
            amount,
            region,
            country,
            nicn: req.request.card_bin.clone(),
            m_tx_id: Some(
                req.resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            data,
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
