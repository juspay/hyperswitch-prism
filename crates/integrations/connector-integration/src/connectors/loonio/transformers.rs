use super::LoonioRouterData;
use crate::types::ResponseRouterData;
use common_enums::AttemptStatus;
use common_utils::{id_type::CustomerId, pii::Email, types::FloatMajorUnit, Method};
use domain_types::errors::{ConnectorResponseTransformationError, IntegrationError};
use domain_types::{
    connector_flow::{Authorize, PSync},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData, ResponseId,
    },
    payment_method_data::{
        BankRedirectData, CustomerInfoDetails, PaymentMethodData, PaymentMethodDataTypes,
    },
    router_data::{
        AdditionalPaymentMethodConnectorResponse, ConnectorResponseData, ConnectorSpecificConfig,
        InteracCustomerInfo,
    },
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
    utils,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
// ===== AUTH TYPE =====

#[derive(Debug, Clone)]
pub struct LoonioAuthType {
    pub merchant_id: Secret<String>,
    pub merchant_token: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for LoonioAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Loonio {
                merchant_id,
                merchant_token,
                ..
            } => Ok(Self {
                merchant_id: merchant_id.to_owned(),
                merchant_token: merchant_token.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
            )),
        }
    }
}

// ===== ERROR RESPONSE =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoonioErrorResponse {
    pub status: Option<u16>,
    pub error_code: Option<String>,
    pub message: String,
}

// ===== AUTHORIZE FLOW =====

#[derive(Debug, Serialize)]
pub struct LoonioCustomerProfile {
    pub first_name: Secret<String>,
    pub last_name: Secret<String>,
    pub email: Email,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address_a: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub province: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postal_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LoonioRedirectUrls {
    pub success_url: String,
    pub failed_url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InteracPaymentMethodType {
    InteracEtransfer,
}

#[derive(Debug, Serialize)]
pub struct LoonioAuthorizeRequest {
    pub currency_code: common_enums::Currency,
    pub customer_profile: LoonioCustomerProfile,
    pub amount: FloatMajorUnit,
    pub customer_id: CustomerId,
    pub transaction_id: String,
    pub payment_method_type: InteracPaymentMethodType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    pub redirect_url: Option<LoonioRedirectUrls>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhook_url: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        LoonioRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for LoonioAuthorizeRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: LoonioRouterData<
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
            PaymentMethodData::BankRedirect(BankRedirectData::Interac { .. }) => {
                let transaction_id = item
                    .router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone();

                // Get billing details
                let billing = item
                    .router_data
                    .resource_common_data
                    .get_billing()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "billing",
                        context: Default::default(),
                    })
                    .attach_printable("Failed to get billing details")?;

                let billing_address = item
                    .router_data
                    .resource_common_data
                    .get_billing_address()?;

                // Extract optional address fields with proper Secret wrapping
                let phone = billing
                    .phone
                    .as_ref()
                    .and_then(|p| p.number.as_ref())
                    .map(|n| Secret::new(n.peek().clone()));
                let address_a = billing_address
                    .line1
                    .as_ref()
                    .map(|l| Secret::new(l.peek().clone()));
                let city = billing_address.city.as_ref().map(|c| c.peek().clone());
                let province = billing_address
                    .state
                    .as_ref()
                    .map(|s| Secret::new(s.peek().clone()));
                let postal_code = billing_address
                    .zip
                    .as_ref()
                    .map(|z| Secret::new(z.peek().clone()));
                let country = billing_address.country.as_ref().map(|c| c.to_string());

                let customer_profile = LoonioCustomerProfile {
                    first_name: item
                        .router_data
                        .resource_common_data
                        .get_billing_first_name()?,
                    last_name: item
                        .router_data
                        .resource_common_data
                        .get_billing_last_name()?,
                    email: item.router_data.resource_common_data.get_billing_email()?,
                    phone,
                    address_a,
                    city,
                    province,
                    postal_code,
                    country,
                };

                let redirect_url = LoonioRedirectUrls {
                    success_url: item.router_data.request.get_router_return_url()?,
                    failed_url: item.router_data.request.get_router_return_url()?,
                };
                let amount = item
                    .connector
                    .amount_converter
                    .convert(
                        item.router_data.request.minor_amount,
                        item.router_data.request.currency,
                    )
                    .change_context(IntegrationError::AmountConversionFailed {
                        context: Default::default(),
                    })?;
                Ok(Self {
                    currency_code: item.router_data.request.currency,
                    customer_profile,
                    amount,
                    customer_id: item.router_data.resource_common_data.get_customer_id()?,
                    transaction_id,
                    payment_method_type: InteracPaymentMethodType::InteracEtransfer,
                    locale: Some("EN".to_string()),
                    redirect_url: Some(redirect_url),
                    webhook_url: Some(item.router_data.request.get_webhook_url()?),
                })
            }
            PaymentMethodData::BankRedirect(_) => Err(IntegrationError::not_implemented(
                utils::get_unimplemented_payment_method_error_message("Loonio"),
            ))?,
            PaymentMethodData::Card(_)
            | PaymentMethodData::Wallet(_)
            | PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::PayLater(_)
            | PaymentMethodData::BankDebit(_)
            | PaymentMethodData::BankTransfer(_)
            | PaymentMethodData::Crypto(_)
            | PaymentMethodData::MandatePayment
            | PaymentMethodData::Reward
            | PaymentMethodData::RealTimePayment(_)
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::CardToken(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::MobilePayment(_)
            | PaymentMethodData::Netbanking(_) => Err(IntegrationError::not_implemented(
                utils::get_unimplemented_payment_method_error_message("Loonio"),
            ))?,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LoonioAuthorizeResponse {
    pub payment_form: String,
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<LoonioAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<LoonioAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // For redirect-based flows, status should be AuthenticationPending
        let status = AttemptStatus::AuthenticationPending;

        // Build redirect form - use Form variant like Hyperswitch does
        let redirection_data = Some(Box::new(RedirectForm::Form {
            endpoint: item.response.payment_form.clone(),
            method: Method::Get,
            form_fields: HashMap::new(),
        }));

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
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

// ===== PSYNC FLOW =====

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LoonioTransactionStatus {
    Created,
    Prepared,
    Pending,
    Settled,
    Available,
    Abandoned,
    Rejected,
    Failed,
    Rollback,
    Returned,
    Nsf,
}

impl From<LoonioTransactionStatus> for AttemptStatus {
    fn from(item: LoonioTransactionStatus) -> Self {
        match item {
            LoonioTransactionStatus::Created => Self::AuthenticationPending,
            LoonioTransactionStatus::Prepared | LoonioTransactionStatus::Pending => Self::Pending,
            LoonioTransactionStatus::Settled | LoonioTransactionStatus::Available => Self::Charged,
            LoonioTransactionStatus::Abandoned
            | LoonioTransactionStatus::Rejected
            | LoonioTransactionStatus::Failed
            | LoonioTransactionStatus::Returned
            | LoonioTransactionStatus::Nsf => Self::Failure,
            LoonioTransactionStatus::Rollback => Self::Voided,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct LoonioCustomerInfo {
    pub customer_name: Option<Secret<String>>,
    pub customer_email: Option<Email>,
    pub customer_phone_number: Option<Secret<String>>,
    pub customer_bank_id: Option<Secret<String>>,
    pub customer_bank_name: Option<Secret<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LoonioPaymentResponseData {
    Sync(LoonioTransactionSyncResponse),
    Webhook(LoonioWebhookBody),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoonioTransactionSyncResponse {
    pub transaction_id: String,
    pub state: LoonioTransactionStatus,
    pub customer_bank_info: Option<LoonioCustomerInfo>,
}

impl TryFrom<ResponseRouterData<LoonioPaymentResponseData, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<LoonioPaymentResponseData, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            LoonioPaymentResponseData::Sync(sync_response) => {
                let connector_response =
                    sync_response
                        .customer_bank_info
                        .as_ref()
                        .map(|customer_info| {
                            ConnectorResponseData::with_additional_payment_method_data(
                                AdditionalPaymentMethodConnectorResponse::BankRedirect {
                                    interac: Some(InteracCustomerInfo {
                                        customer_info: Some(CustomerInfoDetails::from(
                                            customer_info,
                                        )),
                                    }),
                                },
                            )
                        });
                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status: AttemptStatus::from(sync_response.state),
                        connector_response,
                        ..item.router_data.resource_common_data
                    },
                    response: Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(
                            sync_response.transaction_id,
                        ),
                        redirection_data: None,
                        mandate_reference: None,
                        connector_metadata: None,
                        network_txn_id: None,
                        connector_response_reference_id: None,
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                    }),
                    ..item.router_data
                })
            }
            LoonioPaymentResponseData::Webhook(webhook_body) => {
                let payment_status = AttemptStatus::from(&webhook_body.event_code);
                let connector_response = webhook_body.customer_info.as_ref().map(|customer_info| {
                    ConnectorResponseData::with_additional_payment_method_data(
                        AdditionalPaymentMethodConnectorResponse::BankRedirect {
                            interac: Some(InteracCustomerInfo {
                                customer_info: Some(CustomerInfoDetails::from(customer_info)),
                            }),
                        },
                    )
                });
                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status: payment_status,
                        connector_response,
                        ..item.router_data.resource_common_data
                    },
                    response: Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(
                            webhook_body.api_transaction_id,
                        ),
                        redirection_data: None,
                        mandate_reference: None,
                        connector_metadata: None,
                        network_txn_id: None,
                        connector_response_reference_id: None,
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                    }),
                    ..item.router_data
                })
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoonioWebhookBody {
    pub amount: FloatMajorUnit,
    pub api_transaction_id: String,
    pub signature: Option<Secret<String>>,
    pub event_code: LoonioWebhookEventCode,
    #[serde(rename = "type")]
    pub transaction_type: LoonioWebhookTransactionType,
    pub customer_info: Option<LoonioCustomerInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LoonioWebhookEventCode {
    TransactionPrepared,
    TransactionPending,
    TransactionAvailable,
    TransactionSettled,
    TransactionFailed,
    TransactionRejected,
    #[serde(rename = "TRANSACTION_WAITING_STATUS_FILE")]
    TransactionWaitingStatusFile,
    #[serde(rename = "TRANSACTION_STATUS_FILE_RECEIVED")]
    TransactionStatusFileReceived,
    #[serde(rename = "TRANSACTION_STATUS_FILE_FAILED")]
    TransactionStatusFileFailed,
    #[serde(rename = "TRANSACTION_RETURNED")]
    TransactionReturned,
    #[serde(rename = "TRANSACTION_WRONG_DESTINATION")]
    TransactionWrongDestination,
    #[serde(rename = "TRANSACTION_NSF")]
    TransactionNsf,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LoonioWebhookTransactionType {
    Incoming,
    OutgoingVerified,
    OutgoingNotVerified,
    OutgoingCustomerDefined,
}

impl From<&LoonioWebhookEventCode> for AttemptStatus {
    fn from(event_code: &LoonioWebhookEventCode) -> Self {
        match event_code {
            LoonioWebhookEventCode::TransactionSettled
            | LoonioWebhookEventCode::TransactionAvailable => Self::Charged,

            LoonioWebhookEventCode::TransactionPending
            | LoonioWebhookEventCode::TransactionPrepared => Self::Pending,

            LoonioWebhookEventCode::TransactionFailed
            | LoonioWebhookEventCode::TransactionRejected
            | LoonioWebhookEventCode::TransactionStatusFileFailed
            | LoonioWebhookEventCode::TransactionReturned
            | LoonioWebhookEventCode::TransactionWrongDestination
            | LoonioWebhookEventCode::TransactionNsf => Self::Failure,

            _ => Self::Pending,
        }
    }
}

impl From<&LoonioCustomerInfo> for CustomerInfoDetails {
    fn from(value: &LoonioCustomerInfo) -> Self {
        Self {
            customer_name: value.customer_name.clone(),
            customer_email: value.customer_email.clone(),
            customer_phone_number: value.customer_phone_number.clone(),
            customer_bank_id: value.customer_bank_id.clone(),
            customer_bank_name: value.customer_bank_name.clone(),
        }
    }
}
