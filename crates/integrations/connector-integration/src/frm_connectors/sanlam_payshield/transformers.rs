use common_enums::{Currency, FrmDecision};
use common_utils::{ext_traits::ValueExt, pii::SecretSerdeValue, types::StringMinorUnit};
use domain_types::{
    connector_flow::{PrePayoutRiskCheck, PreRiskCheck},
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    frm::frm_types::{
        FrmFlowData, PrePayoutRiskCheckRequest, PrePayoutRiskCheckResponse, PreRiskCheckRequest,
        PreRiskCheckResponse,
    },
    payment_method_data::{BankDebitData, PaymentMethodData, PaymentMethodDataTypes},
    payouts::payout_method_data::{Bank, PayoutMethodData},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

use super::{SanlamPayshieldAmountConvertor, SanlamPayshieldRouterData};
use crate::{types::ResponseRouterData, utils::get_unimplemented_payment_method_error_message};

type RequestError = error_stack::Report<IntegrationError>;
type ResponseError = error_stack::Report<ConnectorError>;

pub struct SanlamPayshieldAuthType {
    pub api_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for SanlamPayshieldAuthType {
    type Error = RequestError;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::SanlamPayshield { api_key, .. } => Ok(Self {
                api_key: api_key.clone(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Ensure the connector is configured with a SanlamPayshield-specific config containing a valid api_key."
                            .to_string(),
                    ),
                    additional_context: Some(
                        "ConnectorSpecificConfig did not match the SanlamPayshield variant; received an unexpected config variant."
                            .to_string(),
                    ),
                    doc_url: None,
                },
            }
            .into()),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SanlamPayshieldFrmMetadata {
    pub profile_id: String,
    pub connector_id: Option<String>,
    pub created_at: time::PrimitiveDateTime,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SanlamPayshieldCheckRequest {
    request_id: String,
    profile_id: String,
    connector_id: String,
    connector_type: ConnectorType,
    transaction: Transaction,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<SecretSerdeValue>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConnectorType {
    Payin,
    Payout,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    payment_id: String,
    amount_in_cents: StringMinorUnit,
    currency: Currency,
    payment_method_type: PaymentMethodType,
    created_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaymentMethodType {
    EftDebitOrder,
    Payshap,
    PayshapProxy,
}

impl<T: PaymentMethodDataTypes> TryFrom<&PaymentMethodData<T>> for PaymentMethodType {
    type Error = RequestError;

    fn try_from(payment_method: &PaymentMethodData<T>) -> Result<Self, Self::Error> {
        match payment_method {
            PaymentMethodData::BankDebit(BankDebitData::EftBankDebit { .. }) => {
                Ok(Self::EftDebitOrder)
            }
            _ => Err(IntegrationError::NotSupported {
                message: get_unimplemented_payment_method_error_message("SanlamPayshield"),
                connector: "SanlamPayshield",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "SanlamPayshield payin checks only support EftDebitOrder".to_string(),
                    ),
                    suggested_action: Some(
                        "Provide EftBankDebit in payment_method for the pre-risk check."
                            .to_string(),
                    ),
                    doc_url: None,
                },
            }
            .into()),
        }
    }
}

impl TryFrom<&PayoutMethodData> for PaymentMethodType {
    type Error = RequestError;

    fn try_from(payout_method: &PayoutMethodData) -> Result<Self, Self::Error> {
        match payout_method {
            PayoutMethodData::Bank(Bank::Payshap(_)) => Ok(Self::Payshap),
            PayoutMethodData::Bank(Bank::PayshapProxy(_)) => Ok(Self::PayshapProxy),
            _ => Err(IntegrationError::NotSupported {
                message: get_unimplemented_payment_method_error_message("SanlamPayshield"),
                connector: "SanlamPayshield",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "SanlamPayshield payout checks only support Payshap or PayshapProxy"
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Provide either Payshap or PayshapProxy in payout_method for the pre-payout-risk check."
                            .to_string(),
                    ),
                    doc_url: None,
                },
            }
            .into()),
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        SanlamPayshieldRouterData<
            RouterDataV2<PreRiskCheck, FrmFlowData, PreRiskCheckRequest, PreRiskCheckResponse>,
            T,
        >,
    > for SanlamPayshieldCheckRequest
{
    type Error = RequestError;

    fn try_from(
        item: SanlamPayshieldRouterData<
            RouterDataV2<PreRiskCheck, FrmFlowData, PreRiskCheckRequest, PreRiskCheckResponse>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let SanlamPayshieldFrmMetadata {
            profile_id,
            connector_id,
            created_at,
        } = item
            .router_data
            .request
            .gateway_metadata
            .clone()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "gateway_metadata".into(),
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "SanlamPayshield pre-risk check requires gateway_metadata to identify the profile, gateway connector, and transaction creation time."
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Provide gateway_metadata containing profile_id, connector_id, and created_at.".to_string(),
                    ),
                    doc_url: None,
                },
            })?
            .parse_value("SanlamPayshieldFrmMetadata")
            .change_context(IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "SanlamPayshield pre-risk check could not deserialize gateway_metadata as SanlamPayshieldFrmMetadata."
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Provide gateway_metadata with string profile_id and connector_id fields and a created_at value that can be deserialized as a PrimitiveDateTime."
                            .to_string(),
                    ),
                    doc_url: None,
                },
            })
            .attach_printable("Failed to parse SanlamPayshieldFrmMetadata")?;

        let connector_id = connector_id.ok_or(IntegrationError::MissingRequiredField {
            field_name: "connector_id".into(),
            context: IntegrationErrorContext {
                additional_context: Some(
                    "SanlamPayshield pre-risk check requires a connector_id in gateway_metadata to identify the gateway being evaluated."
                        .to_string(),
                ),
                suggested_action: Some(
                    "Provide the gateway connector identifier as gateway_metadata.connector_id.".to_string(),
                ),
                doc_url: None,
            },
        })?;

        let payment_method_type = item
            .router_data
            .request
            .payment_method
            .as_ref()
            .map(PaymentMethodType::try_from)
            .transpose()?
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "payment_method".into(),
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "SanlamPayshield pre-risk check requires payment_method to determine transaction.paymentMethodType."
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Provide payment_method.".to_string(),
                    ),
                    doc_url: None,
                },
            })?;

        let request_id = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "connector_request_reference_id".into(),
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "SanlamPayshield pre-risk check requires a merchant FRM identifier to populate requestId."
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Provide merchant_frm_id in the risk-check request"
                            .to_string(),
                    ),
                    doc_url: None,
                },
            })?;

        let payment_id = item
            .router_data
            .request
            .merchant_transaction_id
            .clone()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "merchant_transaction_id".into(),
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "SanlamPayshield pre-risk check requires the merchant transaction identifier as transaction.paymentId."
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Provide merchant_transaction_id in the risk-check request.".to_string(),
                    ),
                    doc_url: None,
                },
            })?;

        let created_at = created_at
            .assume_utc()
            .to_offset(time::macros::offset!(+2))
            .format(time::macros::format_description!(
                "[year]-[month]-[day]T[hour]:[minute]:[second][offset_hour sign:mandatory]:[offset_minute]"
            ))
            .change_context(IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "SanlamPayshield pre-risk check could not format gateway_metadata.created_at as transaction.createdAt with the +02:00 offset."
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Provide a valid UTC creation time in gateway_metadata.created_at that can be formatted with the +02:00 offset."
                            .to_string(),
                    ),
                    doc_url: None,
                },
            })?;

        let amount = SanlamPayshieldAmountConvertor::convert(
            item.router_data.request.amount.amount,
            item.router_data.request.amount.currency,
        )?;

        Ok(Self {
            request_id,
            profile_id,
            connector_id,
            connector_type: ConnectorType::Payin,
            transaction: Transaction {
                payment_id,
                amount_in_cents: amount,
                currency: item.router_data.request.amount.currency,
                payment_method_type,
                created_at,
            },
            metadata: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        SanlamPayshieldRouterData<
            RouterDataV2<
                PrePayoutRiskCheck,
                FrmFlowData,
                PrePayoutRiskCheckRequest,
                PrePayoutRiskCheckResponse,
            >,
            T,
        >,
    > for SanlamPayshieldCheckRequest
{
    type Error = RequestError;

    fn try_from(
        item: SanlamPayshieldRouterData<
            RouterDataV2<
                PrePayoutRiskCheck,
                FrmFlowData,
                PrePayoutRiskCheckRequest,
                PrePayoutRiskCheckResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let SanlamPayshieldFrmMetadata {
            profile_id,
            connector_id,
            created_at,
        } = item
            .router_data
            .request
            .gateway_metadata
            .clone()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "gateway_metadata".into(),
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "SanlamPayshield pre-payout-risk check requires gateway_metadata to identify the profile, gateway connector, and transaction creation time."
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Provide gateway_metadata containing profile_id, connector_id, and created_at.".to_string(),
                    ),
                    doc_url: None,
                },
            })?
            .parse_value("SanlamPayshieldFrmMetadata")
            .change_context(IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "SanlamPayshield pre-payout-risk check could not deserialize gateway_metadata as SanlamPayshieldFrmMetadata."
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Provide gateway_metadata with string profile_id and connector_id fields and a created_at value that can be deserialized as a PrimitiveDateTime."
                            .to_string(),
                    ),
                    doc_url: None,
                },
            })
            .attach_printable("Failed to parse SanlamPayshieldFrmMetadata")?;

        let connector_id = connector_id.ok_or(IntegrationError::MissingRequiredField {
            field_name: "connector_id".into(),
            context: IntegrationErrorContext {
                additional_context: Some(
                    "SanlamPayshield pre-payout-risk check requires a connector_id in gateway_metadata to identify the gateway being evaluated."
                        .to_string(),
                ),
                suggested_action: Some(
                    "Provide the gateway connector identifier as gateway_metadata.connector_id.".to_string(),
                ),
                doc_url: None,
            },
        })?;

        let payment_method_type = item
            .router_data
            .request
            .payout_method
            .as_ref()
            .map(PaymentMethodType::try_from)
            .transpose()?
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "payout_method".into(),
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "SanlamPayshield pre-payout-risk check requires payout_method to determine transaction.paymentMethodType."
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Provide payout_method.".to_string(),
                    ),
                    doc_url: None,
                },
            })?;

        let request_id = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "connector_request_reference_id".into(),
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "SanlamPayshield pre-payout-risk check requires a merchant FRM identifier to populate requestId."
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Provide merchant_frm_id in the risk-check request"
                            .to_string(),
                    ),
                    doc_url: None,
                },
            })?;

        let payout_id = item.router_data.request.merchant_payout_id.clone().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "merchant_payout_id".into(),
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "SanlamPayshield pre-payout-risk check requires the merchant payout identifier as transaction.paymentId."
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Provide merchant_payout_id in the risk-check request.".to_string(),
                    ),
                    doc_url: None,
                },
            },
        )?;

        let created_at = created_at
            .assume_utc()
            .to_offset(time::macros::offset!(+2))
            .format(time::macros::format_description!(
                "[year]-[month]-[day]T[hour]:[minute]:[second][offset_hour sign:mandatory]:[offset_minute]"
            ))
            .change_context(IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "SanlamPayshield pre-payout-risk check could not format gateway_metadata.created_at as transaction.createdAt with the +02:00 offset."
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Provide a valid UTC creation time in gateway_metadata.created_at that can be formatted with the +02:00 offset."
                            .to_string(),
                    ),
                    doc_url: None,
                },
            })?;

        let amount = SanlamPayshieldAmountConvertor::convert(
            item.router_data.request.amount.amount,
            item.router_data.request.amount.currency,
        )?;

        Ok(Self {
            request_id,
            profile_id,
            connector_id,
            connector_type: ConnectorType::Payout,
            transaction: Transaction {
                payment_id: payout_id,
                amount_in_cents: amount,
                currency: item.router_data.request.amount.currency,
                payment_method_type,
                created_at,
            },
            metadata: None,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SanlamPayshieldCheckResponse {
    request_id: String,
    decision: Decision,
    severity: i32,
    reason_codes: Option<Vec<String>>,
    reason: Option<String>,
    rule_config_version: Option<String>,
    evaluated_checks: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum Decision {
    Accept,
    Reject,
}

impl From<Decision> for FrmDecision {
    fn from(decision: Decision) -> Self {
        match decision {
            Decision::Accept => Self::Approve,
            Decision::Reject => Self::Reject,
        }
    }
}

impl
    TryFrom<
        ResponseRouterData<
            SanlamPayshieldCheckResponse,
            RouterDataV2<PreRiskCheck, FrmFlowData, PreRiskCheckRequest, PreRiskCheckResponse>,
        >,
    > for RouterDataV2<PreRiskCheck, FrmFlowData, PreRiskCheckRequest, PreRiskCheckResponse>
{
    type Error = ResponseError;

    fn try_from(
        item: ResponseRouterData<
            SanlamPayshieldCheckResponse,
            RouterDataV2<PreRiskCheck, FrmFlowData, PreRiskCheckRequest, PreRiskCheckResponse>,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(PreRiskCheckResponse {
                frm_decision: Some(item.response.decision.into()),
                risk_score: Some(item.response.severity),
                reason: item.response.reason,
                frm_transaction_id: Some(item.response.request_id),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

impl
    TryFrom<
        ResponseRouterData<
            SanlamPayshieldCheckResponse,
            RouterDataV2<
                PrePayoutRiskCheck,
                FrmFlowData,
                PrePayoutRiskCheckRequest,
                PrePayoutRiskCheckResponse,
            >,
        >,
    >
    for RouterDataV2<
        PrePayoutRiskCheck,
        FrmFlowData,
        PrePayoutRiskCheckRequest,
        PrePayoutRiskCheckResponse,
    >
{
    type Error = ResponseError;

    fn try_from(
        item: ResponseRouterData<
            SanlamPayshieldCheckResponse,
            RouterDataV2<
                PrePayoutRiskCheck,
                FrmFlowData,
                PrePayoutRiskCheckRequest,
                PrePayoutRiskCheckResponse,
            >,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(PrePayoutRiskCheckResponse {
                frm_decision: Some(item.response.decision.into()),
                risk_score: Some(item.response.severity),
                reason: item.response.reason,
                frm_transaction_id: Some(item.response.request_id),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SanlamPayshieldErrorResponse {
    pub error_code: Option<i64>,
    pub error_message: Option<String>,
    #[serde(default)]
    pub inner_errors: Vec<Self>,
    pub message: Option<String>,
}

impl SanlamPayshieldErrorResponse {
    pub fn reason(&self) -> Option<String> {
        (!self.inner_errors.is_empty())
            .then(|| serde_json::to_string(&self.inner_errors).ok())
            .flatten()
    }
}
