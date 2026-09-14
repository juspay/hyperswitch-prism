use crate::types::ResponseRouterData;
use base64::Engine;
use common_utils::types::{StringMajorUnit, StringMajorUnitForConnector};
use domain_types::{
    connector_flow::{PayoutGet, PayoutTransfer},
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payouts::{
        payout_method_data::{PayoutMethodData, Wallet as PayoutWallet},
        payouts_types::{
            PayoutFlowData, PayoutGetRequest, PayoutGetResponse, PayoutTransferRequest,
            PayoutTransferResponse,
        },
    },
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    utils as domain_utils,
};
use error_stack::Report;
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};

const DEFAULT_NOTIFICATION_LANGUAGE: &str = "en-US";

// ===== AUTH TYPE =====

#[derive(Debug)]
pub enum PaypalAuthType {
    TemporaryAuth,
    AuthWithDetails(PaypalConnectorCredentials),
}

#[derive(Debug)]
pub enum PaypalConnectorCredentials {
    StandardIntegration(StandardFlowCredentials),
    PartnerIntegration(PartnerFlowCredentials),
}

impl PaypalConnectorCredentials {
    pub fn get_client_id(&self) -> Secret<String> {
        match self {
            Self::StandardIntegration(item) => item.client_id.clone(),
            Self::PartnerIntegration(item) => item.client_id.clone(),
        }
    }

    pub fn get_client_secret(&self) -> Secret<String> {
        match self {
            Self::StandardIntegration(item) => item.client_secret.clone(),
            Self::PartnerIntegration(item) => item.client_secret.clone(),
        }
    }

    pub fn get_payer_id(&self) -> Option<Secret<String>> {
        match self {
            Self::StandardIntegration(_) => None,
            Self::PartnerIntegration(item) => Some(item.payer_id.clone()),
        }
    }

    pub fn generate_authorization_value(&self) -> String {
        let auth_id = format!(
            "{}:{}",
            self.get_client_id().expose(),
            self.get_client_secret().expose(),
        );
        format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(auth_id)
        )
    }
}

#[derive(Debug)]
pub struct StandardFlowCredentials {
    pub client_id: Secret<String>,
    pub client_secret: Secret<String>,
}

#[derive(Debug)]
pub struct PartnerFlowCredentials {
    pub client_id: Secret<String>,
    pub client_secret: Secret<String>,
    pub payer_id: Secret<String>,
}

impl PaypalAuthType {
    pub fn get_credentials(
        &self,
    ) -> common_utils::errors::CustomResult<&PaypalConnectorCredentials, IntegrationError> {
        match self {
            Self::TemporaryAuth => Err(IntegrationError::InvalidConnectorConfig {
                config: "TemporaryAuth found in connector_account_details",
                context: Default::default(),
            }
            .into()),
            Self::AuthWithDetails(credentials) => Ok(credentials),
        }
    }
}

impl TryFrom<&ConnectorSpecificConfig> for PaypalAuthType {
    type Error = Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Paypal {
                client_id,
                client_secret,
                payer_id,
                ..
            } => match payer_id {
                None => Ok(Self::AuthWithDetails(
                    PaypalConnectorCredentials::StandardIntegration(StandardFlowCredentials {
                        client_id: client_id.to_owned(),
                        client_secret: client_secret.to_owned(),
                    }),
                )),
                Some(payer_id) => Ok(Self::AuthWithDetails(
                    PaypalConnectorCredentials::PartnerIntegration(PartnerFlowCredentials {
                        client_id: client_id.to_owned(),
                        client_secret: client_secret.to_owned(),
                        payer_id: payer_id.to_owned(),
                    }),
                )),
            },
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            })?,
        }
    }
}

// ===== ERROR RESPONSE =====

#[derive(Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct PaypalPaymentErrorResponse {
    pub name: Option<String>,
    pub message: String,
    pub debug_id: Option<String>,
    pub details: Option<Vec<ErrorDetails>>,
}

#[derive(Default, Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ErrorDetails {
    pub issue: String,
    pub description: Option<String>,
}

// ===== PAYOUT TRANSFER REQUEST/RESPONSE =====

#[derive(Debug, Serialize)]
pub struct PaypalFulfillRequest {
    sender_batch_header: PaypalPayoutBatchHeader,
    items: Vec<PaypalPayoutItem>,
}

#[derive(Debug, Serialize)]
pub struct PaypalPayoutBatchHeader {
    sender_batch_id: String,
}

#[derive(Debug, Serialize)]
pub struct PaypalPayoutItem {
    amount: PayoutAmount,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
    notification_language: String,
    #[serde(flatten)]
    payout_method_data: PaypalPayoutMethodData,
}

#[derive(Debug, Serialize)]
pub struct PaypalPayoutMethodData {
    recipient_type: PayoutRecipientType,
    recipient_wallet: PayoutWalletType,
    receiver: PaypalPayoutDataType,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PayoutRecipientType {
    Email,
    PaypalId,
    Phone,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PayoutWalletType {
    Paypal,
    Venmo,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum PaypalPayoutDataType {
    EmailType(common_utils::pii::Email),
    OtherType(Secret<String>),
}

#[derive(Debug, Serialize)]
pub struct PayoutAmount {
    value: StringMajorUnit,
    currency: common_enums::Currency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaypalFulfillResponse {
    batch_header: PaypalBatchResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaypalBatchResponse {
    pub payout_batch_id: String,
    pub batch_status: PaypalPayoutStatus,
}

impl
    TryFrom<
        &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    > for PaypalFulfillRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> Result<Self, Self::Error> {
        let item_data = PaypalPayoutItemDirect::try_from(item)?;
        Ok(Self {
            sender_batch_header: PaypalPayoutBatchHeader {
                sender_batch_id: item
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            },
            items: vec![item_data.0],
        })
    }
}

/// Newtype wrapper used by the direct TryFrom for PaypalPayoutItem.
struct PaypalPayoutItemDirect(PaypalPayoutItem);

impl
    TryFrom<
        &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    > for PaypalPayoutItemDirect
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> Result<Self, Self::Error> {
        let minor_amount = item.request.amount;
        if minor_amount <= common_utils::types::MinorUnit::zero() {
            return Err(IntegrationError::InvalidDataFormat {
                field_name: "amount",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "PayPal Payout Transfer - Payout amount must be greater than zero"
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Provide a valid payout amount greater than zero".to_string(),
                    ),
                    doc_url: None,
                },
            }
            .into());
        }

        let amount = PayoutAmount {
            value: domain_utils::convert_amount(
                &StringMajorUnitForConnector,
                minor_amount,
                item.request.destination_currency,
            )?,
            currency: item.request.destination_currency,
        };

        let payout_method_data = match item.request.payout_method_data.as_ref() {
            Some(PayoutMethodData::Wallet(wallet_data)) => match wallet_data {
                PayoutWallet::Paypal(data) => {
                    let (recipient_type, receiver) =
                        match (&data.email, &data.telephone_number, &data.paypal_id) {
                            (Some(email), _, _) => (
                                PayoutRecipientType::Email,
                                PaypalPayoutDataType::EmailType(email.clone()),
                            ),
                            (_, Some(phone), _) => (
                                PayoutRecipientType::Phone,
                                PaypalPayoutDataType::OtherType(phone.clone()),
                            ),
                            (_, _, Some(paypal_id)) => (
                                PayoutRecipientType::PaypalId,
                                PaypalPayoutDataType::OtherType(paypal_id.clone()),
                            ),
                            _ => Err(IntegrationError::MissingRequiredField {
                                field_name: "receiver_data",
                                context: IntegrationErrorContext {
                                    additional_context: Some("PayPal Payout Transfer - Missing recipient data (email, phone, or PayPal ID)".to_string()),
                                    suggested_action: Some(
                                        "Provide one of: email, telephone_number, or paypal_id in the payout method data".to_string(),
                                    ),
                                    doc_url: None,
                                },
                            })?,
                        };

                    PaypalPayoutMethodData {
                        recipient_type,
                        recipient_wallet: PayoutWalletType::Paypal,
                        receiver,
                    }
                }
                PayoutWallet::Venmo(data) => {
                    let receiver =
                        PaypalPayoutDataType::OtherType(data.telephone_number.clone().ok_or(
                            IntegrationError::MissingRequiredField {
                                field_name: "telephone_number",
                                context: IntegrationErrorContext {
                                    additional_context: Some(
                                        "PayPal Payout Transfer - Venmo requires telephone number"
                                            .to_string(),
                                    ),
                                    suggested_action: Some(
                                        "Provide a valid telephone_number for Venmo payout"
                                            .to_string(),
                                    ),
                                    doc_url: None,
                                },
                            },
                        )?);
                    PaypalPayoutMethodData {
                        recipient_type: PayoutRecipientType::Phone,
                        recipient_wallet: PayoutWalletType::Venmo,
                        receiver,
                    }
                }
                PayoutWallet::ApplePayDecrypt(_) => Err(IntegrationError::NotSupported {
                    message: "ApplePayDecrypt PayoutMethodType is not supported".to_string(),
                    connector: "Paypal",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "PayPal Payout Transfer - Apple Pay Decrypt is not supported for payouts"
                                .to_string(),
                        ),
                        suggested_action: Some(
                            "Use PayPal or Venmo wallet for payouts".to_string(),
                        ),
                        doc_url: None,
                    },
                })?,
            },
            _ => Err(IntegrationError::NotSupported {
                message: "PayoutMethodType is not supported".to_string(),
                connector: "Paypal",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "PayPal Payout Transfer - Only PayPal and Venmo wallets are supported"
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Use PayPal or Venmo wallet for payouts".to_string(),
                    ),
                    doc_url: None,
                },
            })?,
        };

        Ok(Self(PaypalPayoutItem {
            amount,
            payout_method_data,
            note: item.resource_common_data.description.clone(),
            notification_language: DEFAULT_NOTIFICATION_LANGUAGE.to_string(),
        }))
    }
}

impl TryFrom<ResponseRouterData<PaypalFulfillResponse, Self>>
    for RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaypalFulfillResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let batch_header = item.response.batch_header;
        let payout_status = get_payout_status(batch_header.batch_status);

        Ok(Self {
            response: Ok(PayoutTransferResponse {
                merchant_payout_id: item.router_data.request.merchant_payout_id.clone(),
                payout_status,
                connector_payout_id: Some(batch_header.payout_batch_id),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// ===== PAYOUT SYNC (PayoutGet) =====

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaypalPayoutStatus {
    Success,
    Pending,
    Processing,
    Denied,
    Failed,
    Cancelled,
    Refunded,
    Returned,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaypalSyncBatchResponse {
    pub payout_batch_id: Option<String>,
    pub sender_batch_id: Option<String>,
    pub batch_status: PaypalPayoutStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaypalPayoutSyncResponse {
    pub batch_header: PaypalSyncBatchResponse,
}

pub fn get_payout_status(status: PaypalPayoutStatus) -> common_enums::PayoutStatus {
    match status {
        PaypalPayoutStatus::Success => common_enums::PayoutStatus::Success,
        PaypalPayoutStatus::Denied | PaypalPayoutStatus::Failed => {
            common_enums::PayoutStatus::Failure
        }
        PaypalPayoutStatus::Cancelled => common_enums::PayoutStatus::Cancelled,
        PaypalPayoutStatus::Pending | PaypalPayoutStatus::Processing => {
            common_enums::PayoutStatus::Pending
        }
        PaypalPayoutStatus::Refunded | PaypalPayoutStatus::Returned => {
            common_enums::PayoutStatus::Reversed
        }
    }
}

impl TryFrom<ResponseRouterData<PaypalPayoutSyncResponse, Self>>
    for RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaypalPayoutSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let batch_header = response.batch_header;
        let payout_status = get_payout_status(batch_header.batch_status);

        Ok(Self {
            response: Ok(PayoutGetResponse {
                merchant_payout_id: batch_header.sender_batch_id,
                payout_status,
                connector_payout_id: batch_header.payout_batch_id,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}
