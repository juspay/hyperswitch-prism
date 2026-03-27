use base64::{engine::general_purpose::STANDARD, Engine};
use common_enums::AttemptStatus;
use common_utils::errors::CustomResult;
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        ResponseId,
    },
    errors,
    payment_method_data::{BankDebitData, PaymentMethodData, PaymentMethodDataTypes},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::Serialize;

use super::{requests, responses};
use crate::types::ResponseRouterData;

// Wallet type constants
const WALLET_TYPE_APPLE_PAY: &str = "APPLE_PAY";
const WALLET_TYPE_GOOGLE_PAY: &str = "GOOGLE_PAY";

// Re-export request types
pub use requests::{
    BluesnapAchAuthorizeRequest, BluesnapAchData, BluesnapAuthorizeRequest, BluesnapCaptureRequest,
    BluesnapCardHolderInfo, BluesnapCompletePaymentsRequest, BluesnapCreditCard,
    BluesnapEcpTransaction, BluesnapMetadata, BluesnapPayerInfo, BluesnapPaymentMethodDetails,
    BluesnapPaymentsRequest, BluesnapPaymentsTokenRequest, BluesnapRefundRequest,
    BluesnapThreeDSecureInfo, BluesnapTxnType, BluesnapVoidRequest, BluesnapWallet,
    RequestMetadata, TransactionFraudInfo,
};

// Re-export response types
pub use responses::{
    BluesnapAuthorizeResponse, BluesnapCaptureResponse, BluesnapChargebackStatus,
    BluesnapCreditCardResponse, BluesnapDisputeWebhookBody, BluesnapErrorResponse,
    BluesnapPSyncResponse, BluesnapPaymentsResponse, BluesnapProcessingInfo,
    BluesnapProcessingStatus, BluesnapRedirectionResponse, BluesnapRefundResponse,
    BluesnapRefundStatus, BluesnapRefundSyncResponse, BluesnapThreeDsReference,
    BluesnapThreeDsResult, BluesnapVoidResponse, BluesnapWebhookBody, BluesnapWebhookEvent,
    BluesnapWebhookObjectResource, RedirectErrorMessage,
};

const DISPLAY_METADATA: &str = "Y";

fn convert_metadata_to_request_metadata(metadata: serde_json::Value) -> Vec<RequestMetadata> {
    let hashmap: std::collections::HashMap<Option<String>, Option<serde_json::Value>> =
        serde_json::from_str(&metadata.to_string()).unwrap_or_default();

    hashmap
        .into_iter()
        .map(|(key, value)| RequestMetadata {
            meta_key: key,
            meta_value: value.map(|v| v.to_string()),
            is_visible: Some(DISPLAY_METADATA.to_string()),
        })
        .collect()
}

fn get_card_holder_info(
    address: &domain_types::payment_address::AddressDetails,
    email: common_utils::pii::Email,
) -> CustomResult<Option<BluesnapCardHolderInfo>, errors::ConnectorError> {
    let first_name = address.get_first_name()?.clone();
    let last_name = address.get_last_name().unwrap_or(&first_name).clone();

    Ok(Some(BluesnapCardHolderInfo {
        first_name,
        last_name,
        email,
    }))
}

// Helper function to extract payer info from billing address (for ACH transactions)
fn get_payer_info(
    address: &domain_types::payment_address::AddressDetails,
) -> CustomResult<BluesnapPayerInfo, errors::ConnectorError> {
    let first_name = address.get_first_name()?.clone();
    let last_name = address.get_last_name().unwrap_or(&first_name).clone();
    let zip = address.get_zip()?.clone();

    Ok(BluesnapPayerInfo {
        first_name,
        last_name,
        zip,
    })
}

// Map bank type and holder type to BlueSnap's ECP account type format
fn map_ecp_account_type(
    bank_type: Option<common_enums::BankType>,
    bank_holder_type: Option<common_enums::BankHolderType>,
) -> String {
    match (bank_holder_type, bank_type) {
        (Some(common_enums::BankHolderType::Business), Some(common_enums::BankType::Checking)) => {
            "BUSINESS_CHECKING"
        }
        (Some(common_enums::BankHolderType::Business), Some(common_enums::BankType::Savings)) => {
            "BUSINESS_SAVINGS"
        }
        (Some(common_enums::BankHolderType::Personal), Some(common_enums::BankType::Savings))
        | (None, Some(common_enums::BankType::Savings)) => "CONSUMER_SAVINGS",
        (Some(common_enums::BankHolderType::Personal), Some(common_enums::BankType::Checking))
        | (None, Some(common_enums::BankType::Checking))
        | (_, None) => "CONSUMER_CHECKING",
    }
    .to_string()
}

// Auth Type
#[derive(Debug, Clone)]
pub struct BluesnapAuthType {
    pub username: Secret<String>,
    pub password: Secret<String>,
}

impl BluesnapAuthType {
    pub fn generate_basic_auth(&self) -> String {
        let credentials = format!("{}:{}", self.username.peek(), self.password.peek());
        let encoded = STANDARD.encode(credentials);
        format!("Basic {encoded}")
    }
}

impl TryFrom<&ConnectorSpecificConfig> for BluesnapAuthType {
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Bluesnap {
                username, password, ..
            } => Ok(Self {
                username: username.to_owned(),
                password: password.to_owned(),
            }),
            _ => Err(error_stack::report!(
                errors::ConnectorError::FailedToObtainAuthType
            )),
        }
    }
}

// Status mapping function - mimics Hyperswitch's ForeignTryFrom pattern
// Note: txn_type is optional because ACH/ECP responses don't include cardTransactionType
fn get_attempt_status_from_bluesnap_status(
    txn_type: Option<BluesnapTxnType>,
    processing_status: BluesnapProcessingStatus,
) -> AttemptStatus {
    match processing_status {
        BluesnapProcessingStatus::Success => match txn_type {
            Some(BluesnapTxnType::AuthOnly) => AttemptStatus::Authorized,
            Some(BluesnapTxnType::AuthReversal) => AttemptStatus::Voided,
            Some(BluesnapTxnType::AuthCapture) | Some(BluesnapTxnType::Capture) => {
                AttemptStatus::Charged
            }
            Some(BluesnapTxnType::Refund) => AttemptStatus::Charged,
            // Default for ACH/ECP (no transaction type) - treat as charged on success
            None => AttemptStatus::Charged,
        },
        BluesnapProcessingStatus::Pending | BluesnapProcessingStatus::PendingMerchantReview => {
            AttemptStatus::Pending
        }
        BluesnapProcessingStatus::Fail => AttemptStatus::Failure,
    }
}

// Status mapping for refunds
impl From<BluesnapRefundStatus> for common_enums::RefundStatus {
    fn from(status: BluesnapRefundStatus) -> Self {
        match status {
            BluesnapRefundStatus::Success => Self::Success,
            BluesnapRefundStatus::Pending => Self::Pending,
        }
    }
}

// ===== REQUEST TRANSFORMERS =====

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::BluesnapRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for BluesnapAuthorizeRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: super::BluesnapRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        let billing_address = router_data
            .resource_common_data
            .address
            .get_payment_method_billing();

        // Build request based on payment method type
        match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                // Determine card_transaction_type based on capture_method
                let card_transaction_type = match router_data.request.capture_method {
                    Some(common_enums::CaptureMethod::Manual) => BluesnapTxnType::AuthOnly,
                    _ => BluesnapTxnType::AuthCapture,
                };

                let card_holder_info =
                    billing_address
                        .and_then(|addr| addr.address.as_ref())
                        .and_then(|details| {
                            router_data.request.email.clone().and_then(|email| {
                                get_card_holder_info(details, email).ok().flatten()
                            })
                        });

                // Convert card number to Secret<String>
                let card_number = Secret::new(
                    serde_json::to_string(&card_data.card_number.clone().0)
                        .change_context(errors::ConnectorError::RequestEncodingFailed)?
                        .trim_matches('"')
                        .to_string(),
                );

                let payment_method_details = BluesnapPaymentMethodDetails::Card {
                    credit_card: BluesnapCreditCard {
                        card_number,
                        security_code: card_data.card_cvc.clone(),
                        expiration_month: card_data.card_exp_month.clone(),
                        expiration_year: card_data.get_expiry_year_4_digit(),
                    },
                };

                let transaction_meta_data =
                    router_data
                        .request
                        .metadata
                        .as_ref()
                        .map(|metadata| BluesnapMetadata {
                            meta_data: convert_metadata_to_request_metadata(
                                metadata.clone().expose(),
                            ),
                        });

                let amount = super::BluesnapAmountConvertor::convert(
                    router_data.request.minor_amount,
                    router_data.request.currency,
                )?;

                Ok(Self::Card(BluesnapPaymentsRequest {
                    amount,
                    currency: router_data.request.currency.to_string(),
                    card_transaction_type,
                    payment_method_details,
                    card_holder_info,
                    transaction_fraud_info: Some(TransactionFraudInfo {
                        fraud_session_id: router_data
                            .resource_common_data
                            .connector_request_reference_id
                            .clone(),
                    }),
                    merchant_transaction_id: Some(
                        router_data
                            .resource_common_data
                            .connector_request_reference_id
                            .clone(),
                    ),
                    transaction_meta_data,
                }))
            }
            PaymentMethodData::Wallet(wallet_data) => {
                // Determine card_transaction_type based on capture_method
                let card_transaction_type = match router_data.request.capture_method {
                    Some(common_enums::CaptureMethod::Manual) => BluesnapTxnType::AuthOnly,
                    _ => BluesnapTxnType::AuthCapture,
                };

                let card_holder_info =
                    billing_address
                        .and_then(|addr| addr.address.as_ref())
                        .and_then(|details| {
                            router_data.request.email.clone().and_then(|email| {
                                get_card_holder_info(details, email).ok().flatten()
                            })
                        });

                let payment_method_details = match wallet_data {
                    domain_types::payment_method_data::WalletData::ApplePay(apple_pay_data) => {
                        let encoded_payment_token = Secret::new(
                            serde_json::to_string(&apple_pay_data.payment_data)
                                .change_context(errors::ConnectorError::RequestEncodingFailed)?,
                        );
                        BluesnapPaymentMethodDetails::Wallet {
                            wallet: BluesnapWallet {
                                apple_pay: Some(requests::BluesnapApplePayWallet {
                                    encoded_payment_token,
                                }),
                                google_pay: None,
                                wallet_type: WALLET_TYPE_APPLE_PAY.to_string(),
                            },
                        }
                    }
                    domain_types::payment_method_data::WalletData::GooglePay(google_pay_data) => {
                        let encoded_payment_token = Secret::new(
                            serde_json::to_string(&google_pay_data.tokenization_data)
                                .change_context(errors::ConnectorError::RequestEncodingFailed)?,
                        );
                        BluesnapPaymentMethodDetails::Wallet {
                            wallet: BluesnapWallet {
                                apple_pay: None,
                                google_pay: Some(requests::BluesnapGooglePayWallet {
                                    encoded_payment_token,
                                }),
                                wallet_type: WALLET_TYPE_GOOGLE_PAY.to_string(),
                            },
                        }
                    }
                    _ => Err(errors::ConnectorError::NotImplemented(
                        "Selected wallet type is not supported".to_string(),
                    ))?,
                };

                let transaction_meta_data =
                    router_data
                        .request
                        .metadata
                        .as_ref()
                        .map(|metadata| BluesnapMetadata {
                            meta_data: convert_metadata_to_request_metadata(
                                metadata.clone().expose(),
                            ),
                        });

                let amount = super::BluesnapAmountConvertor::convert(
                    router_data.request.minor_amount,
                    router_data.request.currency,
                )?;

                Ok(Self::Card(BluesnapPaymentsRequest {
                    amount,
                    currency: router_data.request.currency.to_string(),
                    card_transaction_type,
                    payment_method_details,
                    card_holder_info,
                    transaction_fraud_info: Some(TransactionFraudInfo {
                        fraud_session_id: router_data
                            .resource_common_data
                            .connector_request_reference_id
                            .clone(),
                    }),
                    merchant_transaction_id: Some(
                        router_data
                            .resource_common_data
                            .connector_request_reference_id
                            .clone(),
                    ),
                    transaction_meta_data,
                }))
            }
            PaymentMethodData::BankDebit(bank_debit_data) => match bank_debit_data {
                BankDebitData::AchBankDebit {
                    account_number,
                    routing_number,
                    bank_type,
                    bank_holder_type,
                    ..
                } => {
                    // Get payer info from billing address (required for ACH)
                    let address_details = billing_address
                        .and_then(|addr| addr.address.as_ref())
                        .ok_or_else(|| {
                            error_stack::report!(errors::ConnectorError::MissingRequiredField {
                                field_name: "billing_address"
                            })
                        })?;

                    let payer_info = get_payer_info(address_details)?;

                    // Map to BlueSnap ECP account type format
                    let account_type = map_ecp_account_type(*bank_type, *bank_holder_type);

                    let amount = super::BluesnapAmountConvertor::convert(
                        router_data.request.minor_amount,
                        router_data.request.currency,
                    )?;

                    let transaction_fraud_info = Some(TransactionFraudInfo {
                        fraud_session_id: router_data
                            .resource_common_data
                            .connector_request_reference_id
                            .clone(),
                    });

                    Ok(Self::Ach(BluesnapAchAuthorizeRequest {
                        ecp_transaction: BluesnapEcpTransaction {
                            routing_number: routing_number.clone(),
                            account_number: account_number.clone(),
                            account_type,
                        },
                        amount,
                        currency: router_data.request.currency.to_string(),
                        authorized_by_shopper: true,
                        payer_info,
                        merchant_transaction_id: router_data
                            .resource_common_data
                            .connector_request_reference_id
                            .clone(),
                        soft_descriptor: None,
                        transaction_fraud_info,
                    }))
                }
                _ => Err(errors::ConnectorError::NotImplemented(
                    "Only ACH Bank Debit is supported".to_string(),
                ))?,
            },
            _ => Err(errors::ConnectorError::NotImplemented(
                "Selected payment method is not supported".to_string(),
            ))?,
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::BluesnapRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for BluesnapCaptureRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: super::BluesnapRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        let connector_transaction_id = match router_data.request.connector_transaction_id {
            ResponseId::ConnectorTransactionId(ref id) => id.clone(),
            _ => return Err(errors::ConnectorError::MissingConnectorTransactionID.into()),
        };

        let amount = super::BluesnapAmountConvertor::convert(
            router_data.request.minor_amount_to_capture,
            router_data.request.currency,
        )?;

        Ok(Self {
            card_transaction_type: BluesnapTxnType::Capture,
            transaction_id: connector_transaction_id,
            amount: Some(amount),
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::BluesnapRouterData<
            RouterDataV2<
                domain_types::connector_flow::Void,
                PaymentFlowData,
                domain_types::connector_types::PaymentVoidData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for BluesnapVoidRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: super::BluesnapRouterData<
            RouterDataV2<
                domain_types::connector_flow::Void,
                PaymentFlowData,
                domain_types::connector_types::PaymentVoidData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        Ok(Self {
            card_transaction_type: BluesnapTxnType::AuthReversal,
            transaction_id: router_data.request.connector_transaction_id.clone(),
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::BluesnapRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for BluesnapRefundRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: super::BluesnapRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        let amount = super::BluesnapAmountConvertor::convert(
            router_data.request.minor_refund_amount,
            router_data.request.currency,
        )?;

        Ok(Self {
            amount: Some(amount),
            reason: router_data.request.reason.clone(),
        })
    }
}

// ===== RESPONSE TRANSFORMERS =====

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<BluesnapAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BluesnapAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = get_attempt_status_from_bluesnap_status(
            item.response.card_transaction_type.clone(),
            item.response.processing_info.processing_status.clone(),
        );

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.response.transaction_id.clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.transaction_id.clone()),
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

impl TryFrom<ResponseRouterData<BluesnapCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BluesnapCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = get_attempt_status_from_bluesnap_status(
            item.response.card_transaction_type.clone(),
            item.response.processing_info.processing_status.clone(),
        );

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.response.transaction_id.clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.transaction_id.clone()),
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

impl TryFrom<ResponseRouterData<BluesnapVoidResponse, Self>>
    for RouterDataV2<
        domain_types::connector_flow::Void,
        PaymentFlowData,
        domain_types::connector_types::PaymentVoidData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(item: ResponseRouterData<BluesnapVoidResponse, Self>) -> Result<Self, Self::Error> {
        let status = get_attempt_status_from_bluesnap_status(
            item.response.card_transaction_type.clone(),
            item.response.processing_info.processing_status.clone(),
        );

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.response.transaction_id.clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.transaction_id.clone()),
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

impl TryFrom<ResponseRouterData<BluesnapPSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BluesnapPSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = get_attempt_status_from_bluesnap_status(
            item.response.card_transaction_type.clone(),
            item.response.processing_info.processing_status.clone(),
        );

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.response.transaction_id.clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.transaction_id.clone()),
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

impl TryFrom<ResponseRouterData<BluesnapRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BluesnapRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let refund_status = item.response.refund_status.clone().into();

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.refund_transaction_id.to_string(),
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

pub fn map_chargeback_status_to_event_type(
    cb_status: &str,
) -> CustomResult<domain_types::connector_types::EventType, errors::ConnectorError> {
    use domain_types::connector_types::EventType;

    let status: BluesnapChargebackStatus =
        serde_json::from_value(serde_json::Value::String(cb_status.to_string()))
            .change_context(errors::ConnectorError::WebhookEventTypeNotFound)?;

    Ok(match status {
        BluesnapChargebackStatus::New | BluesnapChargebackStatus::Working => {
            EventType::DisputeOpened
        }
        BluesnapChargebackStatus::Closed => EventType::DisputeExpired,
        BluesnapChargebackStatus::CompletedLost => EventType::DisputeLost,
        BluesnapChargebackStatus::CompletedPending => EventType::DisputeChallenged,
        BluesnapChargebackStatus::CompletedWon => EventType::DisputeWon,
    })
}

pub fn map_webhook_event_to_incoming_webhook_event(
    webhook_event: &BluesnapWebhookEvent,
) -> domain_types::connector_types::EventType {
    use domain_types::connector_types::EventType;

    match webhook_event {
        BluesnapWebhookEvent::Decline | BluesnapWebhookEvent::CcChargeFailed => {
            EventType::PaymentIntentFailure
        }
        BluesnapWebhookEvent::Charge => EventType::PaymentIntentSuccess,
        BluesnapWebhookEvent::Refund => EventType::RefundSuccess,
        BluesnapWebhookEvent::Chargeback | BluesnapWebhookEvent::ChargebackStatusChanged => {
            EventType::DisputeOpened
        }
        BluesnapWebhookEvent::Unknown => EventType::PaymentIntentFailure,
    }
}

impl TryFrom<ResponseRouterData<BluesnapRefundSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BluesnapRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let refund_status = match item.response.processing_info.processing_status {
            BluesnapProcessingStatus::Success => common_enums::RefundStatus::Success,
            BluesnapProcessingStatus::Pending | BluesnapProcessingStatus::PendingMerchantReview => {
                common_enums::RefundStatus::Pending
            }
            BluesnapProcessingStatus::Fail => common_enums::RefundStatus::Failure,
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.transaction_id.clone(),
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}
