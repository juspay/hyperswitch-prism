use crate::{connectors::absa_sanlam::AbsaSanlamRouterData, types::ResponseRouterData};
use common_enums::{AttemptStatus, BankNames, BankType, Currency};
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    ext_traits::ValueExt,
    pii::SecretSerdeValue,
    types::MinorUnit,
};
use domain_types::{
    connector_flow::Authorize,
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, ResponseId,
        WebhookDetailsResponse,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext, WebhookError},
    payment_method_data::{BankDebitData, PaymentMethodData, PaymentMethodDataTypes},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    utils::{get_unimplemented_payment_method_error_message, is_payment_failure},
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};

pub struct AbsaSanlamAuthType {
    pub api_key: Secret<String>,
    pub merchant_id: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for AbsaSanlamAuthType {
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
                        "Ensure the connector is configured with a AbsaSanlam-specific config containing a valid api_key.".to_string(),
                    ),
                    additional_context: Some(
                        "ConnectorSpecificConfig did not match the AbsaSanlam variant; received an unexpected config variant.".to_string(),
                    ),
                    doc_url: None,
                },
            }
            .into()),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AbsaSanlamMetaData {
    pub batch_user_reference: Option<String>,
}

impl TryFrom<SecretSerdeValue> for AbsaSanlamMetaData {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(metadata: SecretSerdeValue) -> Result<Self, Self::Error> {
        let metadata = metadata
            .expose()
            .parse_value::<Self>("AbsaSanlamMetaData")
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "metadata",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to deserialize connector metadata into AbsaSanlamMetaData; ensure 'batch_user_reference' is a valid optional string.".to_string(),
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
pub struct AbsaSanlamPaymentsRequest {
    pub user_reference: String,
    pub amount: MinorUnit,
    pub currency: Currency,
    #[serde(rename = "payment_method")]
    pub payment_method: AbsaSanlamPaymentMethod,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statement_descriptor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<SecretSerdeValue>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AbsaSanlamPaymentMethod {
    EftDebitOrder(EftDebitOrder),
}

#[derive(Debug, Serialize)]
pub struct EftDebitOrder {
    pub homing_account: Secret<String>,
    pub homing_branch: Option<Secret<String>>,
    pub homing_account_name: Secret<String>,
    pub bank_name: AbsaSanlamBankNames,
    pub bank_type: AbsaSanlamBankType,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AbsaSanlamBankNames {
    Absa,
    AccessBank,
    Albaraka,
    ChinaConstructionBank,
    Discovery,
    EnlBank,
    FirstNationalBank,
    GotymeBank,
    HabibOverseas,
    HbzBank,
    Investec,
    JpMorganChase,
    MtnBanking,
    Olympus,
    OldMutual,
    PermanentBank,
    SocieteGenerale,
    StandardBank,
    StateBankOfIndia,
    Ubank,
    VbsMutualBank,
    BankZero,
    BidvestBank,
    BidvestBankAlliances,
    FbcFidelityBank,
    FinbondEpe,
    FinbondMutualBank,
    Ithala,
    PeoplesBankPepBank,
    PeoplesBank,
    PostBank,
    Nedbank,
    Capitec,
    CapitecBusiness,
    AfricanBank,
    AfricanBankBusiness,
    IciciBank,
    StandardCharteredBank,
    BankOfChina,
    BnpParibas,
    Citi,
    RoyalBankOfScotland,
    HsbcBank,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AbsaSanlamBankType {
    Savings,
    Cheque,
    Transmission,
    Bond,
    Current,
    SubscriptionShare,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AbsaSanlamRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AbsaSanlamPaymentsRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: AbsaSanlamRouterData<
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
                                    "EFT debit order requires 'bank_account_holder_name' to populate the homing_account_name field in the AbsaSanlam payments request.".to_string(),
                                ),
                                suggested_action: Some(
                                    "Provide the bank account holder name in the EFT bank debit payment method data.".to_string(),
                                ),
                                doc_url: None,
                            },
                        },
                    )?;

                    let bank_name = bank_name
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

                    let bank_type = bank_type.map(AbsaSanlamBankType::from).ok_or(
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

                    Ok(AbsaSanlamPaymentMethod::EftDebitOrder(EftDebitOrder {
                        homing_account: account_number.clone(),
                        homing_branch: branch_code.clone(),
                        homing_account_name: homing_account_name.clone(),
                        bank_name,
                        bank_type,
                    }))
                }
                _ => Err(error_stack::report!(IntegrationError::NotSupported {
                    message: get_unimplemented_payment_method_error_message("AbsaSanlam"),
                    connector: "AbsaSanlam",
                    context: Default::default(),
                }))?,
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
            | PaymentMethodData::CardWithNoCvc(_)
            | PaymentMethodData::MobilePayment(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::PaymentMethodToken(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
                Err(error_stack::report!(IntegrationError::NotSupported {
                    message: get_unimplemented_payment_method_error_message("AbsaSanlam"),
                    connector: "AbsaSanlam",
                    context: Default::default(),
                }))
            }
        }?;

        Ok(Self {
            amount: item.router_data.request.minor_amount,
            currency: item.router_data.request.currency,
            payment_method,
            user_reference: item
                .router_data
                .resource_common_data
                .connector_request_reference_id,
            metadata: item.router_data.request.metadata,
            statement_descriptor: item
                .router_data
                .request
                .billing_descriptor
                .as_ref()
                .and_then(|descriptor| descriptor.statement_descriptor.clone()),
        })
    }
}

impl TryFrom<BankNames> for AbsaSanlamBankNames {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(bank: BankNames) -> Result<Self, Self::Error> {
        match bank {
            BankNames::Absa => Ok(Self::Absa),
            BankNames::AccessBank => Ok(Self::AccessBank),
            BankNames::Albaraka => Ok(Self::Albaraka),
            BankNames::ChinaConstructionBank => Ok(Self::ChinaConstructionBank),
            BankNames::Discovery => Ok(Self::Discovery),
            BankNames::EnlBank => Ok(Self::EnlBank),
            BankNames::FirstNationalBank => Ok(Self::FirstNationalBank),
            BankNames::GotymeBank => Ok(Self::GotymeBank),
            BankNames::HabibOverseas => Ok(Self::HabibOverseas),
            BankNames::HbzBank => Ok(Self::HbzBank),
            BankNames::Investec => Ok(Self::Investec),
            BankNames::JpMorganChase => Ok(Self::JpMorganChase),
            BankNames::MtnBanking => Ok(Self::MtnBanking),
            BankNames::Olympus => Ok(Self::Olympus),
            BankNames::OldMutual => Ok(Self::OldMutual),
            BankNames::PermanentBank => Ok(Self::PermanentBank),
            BankNames::SocieteGenerale => Ok(Self::SocieteGenerale),
            BankNames::StandardBank => Ok(Self::StandardBank),
            BankNames::StateBankOfIndia => Ok(Self::StateBankOfIndia),
            BankNames::Ubank => Ok(Self::Ubank),
            BankNames::VbsMutualBank => Ok(Self::VbsMutualBank),
            BankNames::BankZero => Ok(Self::BankZero),
            BankNames::BidvestBank => Ok(Self::BidvestBank),
            BankNames::BidvestBankAlliances => Ok(Self::BidvestBankAlliances),
            BankNames::FbcFidelityBank => Ok(Self::FbcFidelityBank),
            BankNames::FinbondEpe => Ok(Self::FinbondEpe),
            BankNames::FinbondMutualBank => Ok(Self::FinbondMutualBank),
            BankNames::Ithala => Ok(Self::Ithala),
            BankNames::PeoplesBankPepBank => Ok(Self::PeoplesBankPepBank),
            BankNames::PeoplesBank => Ok(Self::PeoplesBank),
            BankNames::PostBank => Ok(Self::PostBank),
            BankNames::Nedbank => Ok(Self::Nedbank),
            BankNames::Capitec => Ok(Self::Capitec),
            BankNames::CapitecBusiness => Ok(Self::CapitecBusiness),
            BankNames::AfricanBank => Ok(Self::AfricanBank),
            BankNames::AfricanBankBusiness => Ok(Self::AfricanBankBusiness),
            BankNames::IciciBank => Ok(Self::IciciBank),
            BankNames::StandardCharteredBank => Ok(Self::StandardCharteredBank),
            BankNames::BankOfChina => Ok(Self::BankOfChina),
            BankNames::BnpParibas => Ok(Self::BnpParibas),
            BankNames::Citi => Ok(Self::Citi),
            BankNames::RoyalBankOfScotland => Ok(Self::RoyalBankOfScotland),
            BankNames::HsbcBank => Ok(Self::HsbcBank),
            bank => Err(IntegrationError::NotSupported {
                message: format!("Invalid BankName for EFT Debit order payment: {bank:?}"),
                connector: "AbsaSanlam",
                context: Default::default(),
            })?,
        }
    }
}

impl From<BankType> for AbsaSanlamBankType {
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
pub struct AbsaSanlamPaymentsResponse {
    pub status: AbsaSanlamPaymentEnqueueStatus,
    pub topic: String,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AbsaSanlamPaymentEnqueueStatus {
    Queued,
    Rejected,
    Unknown,
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<AbsaSanlamPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<AbsaSanlamPaymentsResponse, Self>,
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
                network_txn_link_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
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

impl From<AbsaSanlamPaymentEnqueueStatus> for AttemptStatus {
    fn from(status: AbsaSanlamPaymentEnqueueStatus) -> Self {
        match status {
            AbsaSanlamPaymentEnqueueStatus::Queued | AbsaSanlamPaymentEnqueueStatus::Unknown => {
                Self::Pending
            }
            AbsaSanlamPaymentEnqueueStatus::Rejected => Self::Failure,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum AbsaSanlamWebhookEvent {
    Payment(AbsaSanlamPaymentWebhookEvent),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AbsaSanlamPaymentWebhookEvent {
    pub event_type: AbsaSanlamWebhookEventType,
    pub payment: AbsaSanlamWebhookPayment,
    pub error: Option<AbsaSanlamWebhookError>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AbsaSanlamWebhookError {
    pub code: Option<String>,
    pub message: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum AbsaSanlamWebhookEventType {
    #[serde(rename = "payment.succeeded")]
    PaymentSucceeded,
    #[serde(rename = "payment.failed")]
    PaymentFailed,
    #[serde(rename = "dispute.opened")]
    DisputeOpened,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AbsaSanlamWebhookPayment {
    pub user_reference: String,
    pub status: AbsaSanlamPaymentStatus,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AbsaSanlamPaymentStatus {
    Success,
    Failure,
    Dispute,
}

impl TryFrom<AbsaSanlamWebhookEvent> for WebhookDetailsResponse {
    type Error = error_stack::Report<WebhookError>;
    fn try_from(item: AbsaSanlamWebhookEvent) -> Result<Self, Self::Error> {
        match item {
            AbsaSanlamWebhookEvent::Payment(payment_event) => {
                let status = AttemptStatus::try_from(&payment_event.payment.status)?;
                if is_payment_failure(status) {
                    Ok(Self {
                        status,
                        resource_id: Some(ResponseId::ConnectorTransactionId(
                            payment_event.payment.user_reference.clone(),
                        )),
                        error_code: payment_event.error.as_ref().and_then(|e| e.code.clone()),
                        error_message: payment_event.error.as_ref().and_then(|e| e.message.clone()),
                        error_reason: payment_event.error.as_ref().and_then(|e| e.reason.clone()),
                        connector_response_reference_id: Some(payment_event.payment.user_reference),
                        mandate_reference: None,
                        network_txn_id: None,
                        raw_connector_response: None,
                        response_headers: None,
                        amount_captured: None,
                        minor_amount_captured: None,
                        payment_method_update: None,
                        status_code: 200,
                        sender_payment_instrument_id: None,
                    })
                } else {
                    Ok(Self {
                        status,
                        resource_id: Some(ResponseId::ConnectorTransactionId(
                            payment_event.payment.user_reference.clone(),
                        )),
                        mandate_reference: None,
                        network_txn_id: None,
                        connector_response_reference_id: Some(payment_event.payment.user_reference),
                        raw_connector_response: None,
                        response_headers: None,
                        amount_captured: None,
                        minor_amount_captured: None,
                        payment_method_update: None,
                        error_code: None,
                        error_message: None,
                        error_reason: None,
                        status_code: 200,
                        sender_payment_instrument_id: None,
                    })
                }
            }
        }
    }
}

impl TryFrom<&AbsaSanlamPaymentStatus> for AttemptStatus {
    type Error = error_stack::Report<WebhookError>;
    fn try_from(item: &AbsaSanlamPaymentStatus) -> Result<Self, Self::Error> {
        match item {
            AbsaSanlamPaymentStatus::Success => Ok(Self::Charged),
            AbsaSanlamPaymentStatus::Failure => Ok(Self::Failure),
            AbsaSanlamPaymentStatus::Dispute => Err(WebhookError::WebhookResponseEncodingFailed)?,
        }
    }
}
