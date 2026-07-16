use crate::types::ResponseRouterData;
use common_enums::{Currency, PayoutStatus};
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    ext_traits::ValueExt,
    pii::SecretSerdeValue,
    types::MinorUnit,
};
use domain_types::{
    connector_flow::PayoutTransfer,
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payouts::{
        payout_method_data::{Bank, PayoutMethodData},
        payouts_types::{PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse},
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};
use crate::connectors::sanlam_common::transformers::{AbsaSanlamBankNames, AbsaSanlamBankType, KafkaEnqueueResponse, KafkaEnqueueStatus};

const CONNECTOR_NAME: &str = "AbsaSanlam";

pub struct AbsaSanlamPayoutAuthType {
    pub api_key: Secret<String>,
    pub merchant_id: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for AbsaSanlamPayoutAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match item {
            ConnectorSpecificConfig::AbsaSanlam { api_key, merchant_id, .. } => Ok(Self {
                api_key: api_key.to_owned(),
                merchant_id: merchant_id.to_owned(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Ensure the connector is configured with an AbsaSanlam-specific config containing a valid api_key."
                            .to_string(),
                    ),
                    additional_context: Some(
                        "ConnectorSpecificConfig did not match the AbsaSanlam variant; received an unexpected config variant."
                            .to_string(),
                    ),
                    doc_url: None,
                },
            }
            .into()),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct AbsaSanlamPayoutMetadata {
    pub batch_user_reference: Option<String>,
    pub eft_service_type: Option<String>,
    pub payout_reason: Option<String>,
}

impl TryFrom<SecretSerdeValue> for AbsaSanlamPayoutMetadata {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(metadata: SecretSerdeValue) -> Result<Self, Self::Error> {
        metadata
            .expose()
            .parse_value::<Self>("AbsaSanlamPayoutMetadata")
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "metadata",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to deserialize AbsaSanlam payout metadata. Expected optional string fields: batch_user_reference, eft_service_type, payout_reason."
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Verify the connector metadata is valid JSON with optional fields 'batch_user_reference', 'eft_service_type', and 'payout_reason' as string values."
                            .to_string(),
                    ),
                    doc_url: None,
                },
            })
    }
}

#[derive(Debug, Serialize)]
pub struct AbsaSanlamPayoutTransferRequest {
    pub user_reference: String,
    pub amount: MinorUnit,
    pub currency: Currency,
    pub payout_method: AbsaSanlamPayoutMethod,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statement_descriptor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<SecretSerdeValue>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AbsaSanlamPayoutMethod {
    EftBankTransfer(EftBankTransfer),
}

#[derive(Debug, Serialize)]
pub struct EftBankTransfer {
    pub homing_account: Secret<String>,
    pub homing_branch: Option<Secret<String>>,
    pub homing_account_name: Secret<String>,
    pub bank_name: AbsaSanlamBankNames,
    pub bank_type: AbsaSanlamBankType,
}

impl
    TryFrom<
        &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    > for AbsaSanlamPayoutTransferRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> Result<Self, Self::Error> {
        let payout_method = match item.request.payout_method_data.clone() {
            Some(PayoutMethodData::Bank(Bank::Eft(eft))) => {
                let homing_account_name = eft.bank_account_holder_name.as_ref().ok_or(
                    IntegrationError::MissingRequiredField {
                        field_name: "bank_account_holder_name",
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "EFT debit order requires 'bank_account_holder_name' to populate the homing_account_name field in the AbsaSanlam payments request.".to_string(),
                            ),
                            suggested_action: Some(
                                "Provide the bank account holder name in the EFT bank debit payment method data.".to_string(),
                            ),
                            doc_url: None,
                        },
                    },
                )?;

                let bank_name = eft.bank_name
                    .map(AbsaSanlamBankNames::try_from)
                    .transpose()?
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "bank_name",
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "EFT debit order requires 'bank_name' to be provided and mapped to a supported AbsaSanlam bank (e.g., Absa).".to_string(),
                            ),
                            suggested_action: Some(
                                "Provide a supported bank name in the EFT bank debit payment method data.".to_string(),
                            ),
                            doc_url: None,
                        },
                    })?;

                let bank_type = eft.bank_type.map(AbsaSanlamBankType::from).ok_or(
                    IntegrationError::MissingRequiredField {
                        field_name: "bank_type",
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "EFT debit order requires 'bank_type' to be provided (e.g., Savings, Cheque, Current, Bond, Transmission, SubscriptionShare).".to_string(),
                            ),
                            suggested_action: Some(
                                "Provide a valid bank account type in the EFT bank debit payment method data.".to_string(),
                            ),
                            doc_url: None,
                        },
                    },
                )?;

                Ok(AbsaSanlamPayoutMethod::EftBankTransfer(EftBankTransfer {
                    homing_account: eft.bank_account_number.clone(),
                    homing_branch: eft.branch_code.clone(),
                    homing_account_name: homing_account_name.clone(),
                    bank_name,
                    bank_type,
                }))
            }
            Some(PayoutMethodData::Card(_))
            | Some(PayoutMethodData::Bank(_))
            | Some(PayoutMethodData::Wallet(_))
            | Some(PayoutMethodData::BankRedirect(_))
            | Some(PayoutMethodData::Passthrough(_)) => Err(IntegrationError::NotSupported {
                message:
                    "AbsaSanlam payout transfer supports eft_bank_transfer only"
                        .to_string(),
                connector: CONNECTOR_NAME,
                context: Default::default(),
            }),
            None => Err(IntegrationError::MissingRequiredField {
                field_name: "payout_method_data",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "AbsaSanlam payout transfer requires payout_method_data".to_string(),
                    ),
                    suggested_action: Some(
                        "Provide eft_bank_transfer payout method data"
                            .to_string(),
                    ),
                    doc_url: None,
                },
            }),
        }?;

        Ok(Self {
            user_reference: item
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            amount: item.request.amount,
            currency: item.request.source_currency,
            payout_method,
            statement_descriptor: item.resource_common_data.description.clone(),
            metadata: item.request.metadata.clone(),
        })
    }
}

impl TryFrom<ResponseRouterData<KafkaEnqueueResponse, Self>>
    for RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<KafkaEnqueueResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = PayoutStatus::from(item.response.status);
        let response = if matches!(status, PayoutStatus::Failure) {
            Err(ErrorResponse {
                code: item
                    .response
                    .error_code
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: item
                    .response
                    .error_message
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: None,
                status_code: item.http_code,
                attempt_status: None,
                connector_transaction_id: None,
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            })
        } else {
            Ok(PayoutTransferResponse {
                merchant_payout_id: item.router_data.request.merchant_payout_id.clone(),
                payout_status: status,
                connector_payout_id: None,
                status_code: item.http_code,
            })
        };

        Ok(Self {
            response,
            ..item.router_data
        })
    }
}

impl From<KafkaEnqueueStatus> for PayoutStatus {
    fn from(status: KafkaEnqueueStatus) -> Self {
        match status {
            KafkaEnqueueStatus::Queued =>Self::Initiated,
            KafkaEnqueueStatus::Unknown => Self::Pending,
            KafkaEnqueueStatus::Rejected => Self::Failure,
        }
    }
}
