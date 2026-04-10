use crate::{connectors::sanlammultidata::SanlammultidataRouterData, types::ResponseRouterData};
use common_enums::{AttemptStatus, BankNames, BankType, Currency};
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    ext_traits::ValueExt,
    pii::SecretSerdeValue,
    types::MinorUnit,
};
use domain_types::{
    connector_flow::Authorize,
    connector_types::{PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, ResponseId},
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::{BankDebitData, PaymentMethodData, PaymentMethodDataTypes},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    utils::{get_unimplemented_payment_method_error_message, is_payment_failure},
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};

pub struct SanlammultidataAuthType {
    pub(super) api_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for SanlammultidataAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(item: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match item {
            ConnectorSpecificConfig::Sanlammultidata { api_key, .. } => Ok(Self {
                api_key: api_key.to_owned(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Ensure the connector is configured with a Sanlammultidata-specific config containing a valid api_key.".to_string(),
                    ),
                    additional_context: Some(
                        "ConnectorSpecificConfig did not match the Sanlammultidata variant; received an unexpected config variant.".to_string(),
                    ),
                    doc_url: None,
                },
            }
            .into()),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SanlammultidataMetaData {
    pub batch_user_reference: Option<String>,
}

impl TryFrom<SecretSerdeValue> for SanlammultidataMetaData {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(metadata: SecretSerdeValue) -> Result<Self, Self::Error> {
        let metadata = metadata
            .expose()
            .parse_value::<Self>("SanlammultidataMetaData")
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "metadata",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to deserialize connector metadata into SanlammultidataMetaData; ensure 'batch_user_reference' is a valid optional string.".to_string(),
                    ),
                    suggested_action: Some(
                        "Verify the connector metadata is valid JSON with an optional 'batch_user_reference' string field.".to_string(),
                    ),
                    doc_url: None,
                },
            })?;
        Ok(metadata)
    }
}

#[derive(Debug, Serialize)]
pub struct SanlammultidataPaymentsRequest {
    pub user_reference: String,
    pub amount: MinorUnit,
    pub currency: Currency,
    #[serde(rename = "payment_method")]
    pub payment_method: SanlammultidataPaymentMethod,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statement_descriptor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_user_reference: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SanlammultidataPaymentMethod {
    EftDebitOrder(EftDebitOrder),
}

#[derive(Debug, Serialize)]
pub struct EftDebitOrder {
    pub homing_account: Secret<String>,
    pub homing_branch: Secret<String>,
    pub homing_account_name: Secret<String>,
    pub bank_name: SanlammultidataBankNames,
    pub bank_type: SanlammultidataBankType,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SanlammultidataBankNames {
    Absa,
    Capitec,
    Fnb,
    Nedbank,
    StandardBank,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SanlammultidataBankType {
    Savings,
    Cheque,
    Transmission,
    Bond,
    Current,
    SubscriptionShare,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        SanlammultidataRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for SanlammultidataPaymentsRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: SanlammultidataRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let payment_method = match item.router_data.request.payment_method_data {
            PaymentMethodData::BankDebit(ref bank_debit_data) => match bank_debit_data {
                BankDebitData::EftBankDebit {
                    account_number,
                    branch_code,
                    bank_account_holder_name,
                    bank_name,
                    bank_type,
                } => {
                    let homing_account_name = bank_account_holder_name.as_ref().ok_or(
                        IntegrationError::MissingRequiredField {
                            field_name: "bank_account_holder_name",
                            context: IntegrationErrorContext {
                                additional_context: Some(
                                    "EFT debit order requires 'bank_account_holder_name' to populate the homing_account_name field in the Sanlammultidata payments request.".to_string(),
                                ),
                                suggested_action: Some(
                                    "Provide the bank account holder name in the EFT bank debit payment method data.".to_string(),
                                ),
                                doc_url: None,
                            },
                        },
                    )?;

                    let bank_name = bank_name
                        .map(SanlammultidataBankNames::try_from)
                        .transpose()?
                        .ok_or(IntegrationError::MissingRequiredField {
                            field_name: "bank_name",
                            context: IntegrationErrorContext {
                                additional_context: Some(
                                    "EFT debit order requires 'bank_name' to be provided and mapped to a supported Sanlammultidata bank (e.g., Absa).".to_string(),
                                ),
                                suggested_action: Some(
                                    "Provide a supported bank name in the EFT bank debit payment method data.".to_string(),
                                ),
                                doc_url: None,
                            },
                        })?;

                    let bank_type = bank_type.map(SanlammultidataBankType::from).ok_or(
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

                    Ok(SanlammultidataPaymentMethod::EftDebitOrder(EftDebitOrder {
                        homing_account: account_number.clone(),
                        homing_branch: branch_code.clone(),
                        homing_account_name: homing_account_name.clone(),
                        bank_name,
                        bank_type,
                    }))
                }
                _ => Err(IntegrationError::not_implemented(
                    get_unimplemented_payment_method_error_message("Sanlammultidata"),
                ))?,
            },
            PaymentMethodData::Card(_)
            | PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::Crypto(_)
            | PaymentMethodData::Wallet(_)
            | PaymentMethodData::PayLater(_)
            | PaymentMethodData::BankRedirect(_)
            | PaymentMethodData::BankTransfer(_)
            | PaymentMethodData::MandatePayment
            | PaymentMethodData::Reward
            | PaymentMethodData::RealTimePayment(_)
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::MobilePayment(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::PaymentMethodToken(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
                Err(IntegrationError::not_implemented(
                    get_unimplemented_payment_method_error_message("Sanlammultidata"),
                ))
            }
        }?;

        let batch_user_reference = item
            .router_data
            .request
            .metadata
            .map(SanlammultidataMetaData::try_from)
            .transpose()?
            .and_then(|m| m.batch_user_reference);

        Ok(Self {
            amount: item.router_data.request.minor_amount,
            currency: item.router_data.request.currency,
            payment_method,
            user_reference: item
                .router_data
                .resource_common_data
                .connector_request_reference_id,
            batch_user_reference,
            statement_descriptor: item
                .router_data
                .request
                .billing_descriptor
                .as_ref()
                .and_then(|descriptor| descriptor.statement_descriptor.clone()),
        })
    }
}

impl TryFrom<BankNames> for SanlammultidataBankNames {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(bank: BankNames) -> Result<Self, Self::Error> {
        match bank {
            BankNames::Absa => Ok(Self::Absa),
            bank => Err(IntegrationError::NotSupported {
                message: format!("Invalid BankName for EFT Debit order payment: {bank:?}"),
                connector: "Sanlammultidata",
                context: Default::default(),
            })?,
        }
    }
}

impl From<BankType> for SanlammultidataBankType {
    fn from(value: BankType) -> Self {
        match value {
            BankType::Checking => Self::Cheque,
            BankType::Savings => Self::Savings,
            BankType::Current => Self::Current,
            BankType::Bond => Self::Bond,
            BankType::Transmission => Self::Transmission,
            BankType::SubscriptionShare => Self::SubscriptionShare,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SanlammultidataPaymentsResponse {
    pub status: SanlammultidataPaymentStatus,
    pub topic: String,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SanlammultidataPaymentStatus {
    Queued,
    Rejected,
    Unknown,
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<SanlammultidataPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<SanlammultidataPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::from(item.response.status);
        let response = if is_payment_failure(status) {
            Err(ErrorResponse {
                code: item
                    .response
                    .error_code
                    .clone()
                    .unwrap_or(NO_ERROR_CODE.to_string()),
                message: item
                    .response
                    .error_message
                    .clone()
                    .unwrap_or(NO_ERROR_MESSAGE.to_string()),
                reason: None,
                status_code: item.http_code,
                attempt_status: None,
                connector_transaction_id: None,
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            })
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::NoResponseId,
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            })
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}

impl From<SanlammultidataPaymentStatus> for AttemptStatus {
    fn from(status: SanlammultidataPaymentStatus) -> Self {
        match status {
            SanlammultidataPaymentStatus::Queued | SanlammultidataPaymentStatus::Unknown => {
                Self::Pending
            }
            SanlammultidataPaymentStatus::Rejected => Self::Failure,
        }
    }
}
