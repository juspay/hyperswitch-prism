use base64::{engine::general_purpose::STANDARD, Engine};
use common_enums::AttemptStatus;
use common_utils::errors::CustomResult;
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, PSync, RSync, Refund, RepeatPayment,
        SetupMandate,
    },
    connector_types::{
        BluesnapClientAuthenticationResponse as BluesnapClientAuthenticationResponseDomain,
        ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData,
        ConnectorSpecificClientAuthenticationResponse, MandateReference, MandateReferenceId,
        PaymentFlowData, PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        RepeatPaymentData, ResponseId, SetupMandateRequestData,
    },
    payment_method_data::{BankDebitData, PaymentMethodData, PaymentMethodDataTypes},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    utils,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use super::{requests, responses, BluesnapRouterData};
use crate::types::ResponseRouterData;
use domain_types::errors::{
    ConnectorError, IntegrationError, IntegrationErrorContext, ResponseTransformationErrorContext,
    WebhookError,
};

// Wallet type constants
const WALLET_TYPE_APPLE_PAY: &str = "APPLE_PAY";
const WALLET_TYPE_GOOGLE_PAY: &str = "GOOGLE_PAY";

// Re-export request types
pub use requests::{
    BluesnapAchAuthorizeRequest, BluesnapAchData, BluesnapAuthorizeRequest,
    BluesnapBillingContactInfo, BluesnapCaptureRequest, BluesnapCardHolderInfo,
    BluesnapCompletePaymentsRequest, BluesnapCreditCard, BluesnapEcpTransaction, BluesnapMetadata,
    BluesnapPayerInfo, BluesnapPaymentMethodDetails, BluesnapPaymentSources,
    BluesnapPaymentsRequest, BluesnapPaymentsTokenRequest, BluesnapRefundRequest,
    BluesnapRepeatCreditCard, BluesnapRepeatPaymentRequest, BluesnapSepaAuthorizeRequest,
    BluesnapSepaDirectDebitTransaction, BluesnapSepaPayerInfo, BluesnapSetupMandateRequest,
    BluesnapThreeDSecureInfo, BluesnapTxnType, BluesnapVaultedCreditCard,
    BluesnapVaultedCreditCardInfo, BluesnapVoidRequest, BluesnapWallet, RequestMetadata,
    TransactionFraudInfo,
};

// Re-export response types
pub use responses::{
    BluesnapAuthorizeResponse, BluesnapCaptureResponse, BluesnapChargebackStatus,
    BluesnapCreditCardResponse, BluesnapDisputeWebhookBody, BluesnapErrorResponse,
    BluesnapPSyncResponse, BluesnapPaymentsResponse, BluesnapProcessingInfo,
    BluesnapProcessingStatus, BluesnapRedirectionResponse, BluesnapRefundResponse,
    BluesnapRefundStatus, BluesnapRefundSyncResponse, BluesnapRepeatPaymentResponse,
    BluesnapSetupMandateResponse, BluesnapThreeDsReference, BluesnapThreeDsResult,
    BluesnapVoidResponse, BluesnapWebhookBody, BluesnapWebhookEvent, BluesnapWebhookObjectResource,
    RedirectErrorMessage,
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
) -> CustomResult<Option<BluesnapCardHolderInfo>, IntegrationError> {
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
) -> CustomResult<BluesnapPayerInfo, IntegrationError> {
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
) -> CustomResult<String, IntegrationError> {
    let account_type = match (bank_holder_type, bank_type) {
        (Some(common_enums::BankHolderType::Business), Some(common_enums::BankType::Checking)) => {
            "CORPORATE_CHECKING"
        }
        (Some(common_enums::BankHolderType::Business), Some(common_enums::BankType::Savings)) => {
            "CORPORATE_SAVINGS"
        }
        (Some(common_enums::BankHolderType::Personal), Some(common_enums::BankType::Savings))
        | (None, Some(common_enums::BankType::Savings)) => "CONSUMER_SAVINGS",
        (Some(common_enums::BankHolderType::Personal), Some(common_enums::BankType::Checking))
        | (None, Some(common_enums::BankType::Checking))
        | (_, None) => "CONSUMER_CHECKING",
        (_, Some(common_enums::BankType::Transmission))
        | (_, Some(common_enums::BankType::Current))
        | (_, Some(common_enums::BankType::Bond))
        | (_, Some(common_enums::BankType::SubscriptionShare)) => {
            Err(IntegrationError::NotSupported {
                message: format!("Bank type {bank_type:?} is not supported by BlueSnap"),
                connector: "bluesnap",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Use `BankType::Checking` or `BankType::Savings`".to_owned(),
                    ),
                    doc_url: None,
                    additional_context: Some(format!(
                        "Received BankType::{bank_type:?}, which does not map to any BlueSnap ECP account type. Only `Checking` and `Savings` are accepted by the BlueSnap."
                    )),
                },
            })?
        }
    };
    Ok(account_type.to_string())
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
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Bluesnap {
                username, password, ..
            } => Ok(Self {
                username: username.to_owned(),
                password: password.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
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
        BluesnapRouterData<
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
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: BluesnapRouterData<
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
                        .change_context(IntegrationError::RequestEncodingFailed {
                            context: Default::default(),
                        })?
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

                let amount = utils::convert_amount(
                    item.connector.amount_converter,
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
                            serde_json::to_string(&apple_pay_data.payment_data).change_context(
                                IntegrationError::RequestEncodingFailed {
                                    context: Default::default(),
                                },
                            )?,
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
                                .change_context(IntegrationError::RequestEncodingFailed {
                                    context: Default::default(),
                                })?,
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
                    _ => Err(IntegrationError::NotImplemented(
                        "Selected wallet type is not supported".to_string(),
                        Default::default(),
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

                let amount = utils::convert_amount(
                    item.connector.amount_converter,
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
                            error_stack::report!(IntegrationError::MissingRequiredField {
                                field_name: "billing_address",
                                context: Default::default()
                            })
                        })?;

                    let payer_info = get_payer_info(address_details)?;

                    // Map to BlueSnap ECP account type format
                    let account_type = map_ecp_account_type(*bank_type, *bank_holder_type)?;

                    let amount = utils::convert_amount(
                        item.connector.amount_converter,
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
                BankDebitData::SepaBankDebit { iban, .. } => {
                    let first_name = router_data.resource_common_data.get_billing_first_name()?;

                    let last_name = router_data
                        .resource_common_data
                        .get_billing_last_name()
                        .unwrap_or_else(|_| first_name.clone());

                    // Extract country from billing address
                    let country = billing_address
                        .and_then(|addr| addr.address.as_ref())
                        .and_then(|details| details.country.as_ref())
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "de".to_string()); // Default to DE for SEPA

                    let amount = utils::convert_amount(
                        item.connector.amount_converter,
                        router_data.request.minor_amount,
                        router_data.request.currency,
                    )?;

                    let transaction_fraud_info = Some(TransactionFraudInfo {
                        fraud_session_id: router_data
                            .resource_common_data
                            .connector_request_reference_id
                            .clone(),
                    });

                    Ok(Self::Sepa(BluesnapSepaAuthorizeRequest {
                        amount,
                        currency: router_data.request.currency.to_string(),
                        authorized_by_shopper: true,
                        payer_info: BluesnapSepaPayerInfo {
                            first_name,
                            last_name,
                            country: country.to_lowercase(),
                        },
                        sepa_direct_debit_transaction: BluesnapSepaDirectDebitTransaction {
                            iban: iban.clone(),
                        },
                        merchant_transaction_id: router_data
                            .resource_common_data
                            .connector_request_reference_id
                            .clone(),
                        soft_descriptor: None,
                        transaction_fraud_info,
                    }))
                }
                _ => Err(IntegrationError::NotImplemented(
                    "Only ACH and SEPA Bank Debit are supported".to_string(),
                    Default::default(),
                ))?,
            },
            PaymentMethodData::PaymentMethodToken(token_data) => {
                let token = token_data.token.clone();

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

                let amount = utils::convert_amount(
                    item.connector.amount_converter,
                    router_data.request.minor_amount,
                    router_data.request.currency,
                )?;

                Ok(Self::CardToken(BluesnapCompletePaymentsRequest {
                    amount,
                    currency: router_data.request.currency.to_string(),
                    card_transaction_type,
                    pf_token: token,
                    three_d_secure: None,
                    transaction_fraud_info: Some(TransactionFraudInfo {
                        fraud_session_id: router_data
                            .resource_common_data
                            .connector_request_reference_id
                            .clone(),
                    }),
                    card_holder_info,
                    merchant_transaction_id: Some(
                        router_data
                            .resource_common_data
                            .connector_request_reference_id
                            .clone(),
                    ),
                    transaction_meta_data,
                }))
            }
            _ => Err(IntegrationError::NotImplemented(
                "Selected payment method is not supported".to_string(),
                Default::default(),
            ))?,
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BluesnapRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for BluesnapCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: BluesnapRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        let connector_transaction_id = match router_data.request.connector_transaction_id {
            ResponseId::ConnectorTransactionId(ref id) => id.clone(),
            _ => {
                return Err(IntegrationError::MissingConnectorTransactionID {
                    context: Default::default(),
                }
                .into())
            }
        };

        let amount = utils::convert_amount(
            item.connector.amount_converter,
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
        BluesnapRouterData<
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
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: BluesnapRouterData<
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
        BluesnapRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for BluesnapRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: BluesnapRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        let amount = utils::convert_amount(
            item.connector.amount_converter,
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
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BluesnapAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = get_attempt_status_from_bluesnap_status(
            item.response.card_transaction_type.clone(),
            item.response.processing_info.processing_status.clone(),
        );

        // When card_transaction_type is absent, it's a bank debit (ACH/SEPA) response.
        // Store this hint so PSync can route to the alt-transactions endpoint.
        let connector_metadata = if item.response.card_transaction_type.is_none() {
            Some(serde_json::json!({"is_alt_transaction": true}))
        } else {
            None
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.response.transaction_id.clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata,
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
    type Error = error_stack::Report<ConnectorError>;

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
    type Error = error_stack::Report<ConnectorError>;

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
    type Error = error_stack::Report<ConnectorError>;

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
    type Error = error_stack::Report<ConnectorError>;

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
) -> CustomResult<domain_types::connector_types::EventType, WebhookError> {
    use domain_types::connector_types::EventType;

    let status: BluesnapChargebackStatus =
        serde_json::from_value(serde_json::Value::String(cb_status.to_string()))
            .change_context(WebhookError::WebhookEventTypeNotFound)?;

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
    type Error = error_stack::Report<ConnectorError>;

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

// ---- ClientAuthenticationToken flow types ----

/// Bluesnap Hosted Payment Fields token request.
/// The POST /services/2/payment-fields-tokens endpoint does not require a JSON body;
/// it returns a pfToken in the Location header of the response.
/// However, to use the macro framework we define an empty request body.
#[derive(Debug, Serialize)]
pub struct BluesnapClientAuthRequest {}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BluesnapRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                PaymentFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for BluesnapClientAuthRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        _item: BluesnapRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                PaymentFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

/// Bluesnap Hosted Payment Fields token response.
/// The pfToken is extracted from the Location header (last path segment)
/// or from the JSON response body if present.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapClientAuthResponse {
    pub pf_token: Option<Secret<String>>,
}

impl TryFrom<ResponseRouterData<BluesnapClientAuthResponse, Self>>
    for RouterDataV2<
        ClientAuthenticationToken,
        PaymentFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<BluesnapClientAuthResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        // Extract the pfToken from the response
        let pf_token = response
            .pf_token
            .ok_or(ConnectorError::ResponseDeserializationFailed {
                context: ResponseTransformationErrorContext {
                    http_status_code: Some(item.http_code),
                    additional_context: Some(
                        "Bluesnap ClientAuthenticationToken response did not contain a pfToken. \
                         The token is expected in the HTTP Location header of the POST \
                         /services/2/payment-fields-tokens response."
                            .to_owned(),
                    ),
                },
            })?;

        let session_data = ClientAuthenticationTokenData::ConnectorSpecific(Box::new(
            ConnectorSpecificClientAuthenticationResponse::Bluesnap(
                BluesnapClientAuthenticationResponseDomain { pf_token },
            ),
        ));

        Ok(Self {
            response: Ok(PaymentsResponseData::ClientAuthenticationTokenResponse {
                session_data,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// ===== SETUP MANDATE (VAULTED SHOPPER) TRANSFORMERS =====

/// Transform SetupMandate request to BlueSnap vaulted shopper request
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BluesnapRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for BluesnapSetupMandateRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: BluesnapRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Get billing address for cardholder info
        let billing_address = router_data
            .resource_common_data
            .get_optional_billing()
            .and_then(|b| b.address.as_ref());

        let (first_name, last_name) = match billing_address {
            Some(address) => {
                let first = address.get_first_name()?.clone();
                let last = address.get_last_name().unwrap_or(&first).clone();
                (first, last)
            }
            None => {
                return Err(IntegrationError::MissingRequiredField {
                    field_name: "billing_address",
                    context: Default::default(),
                }
                .into())
            }
        };

        // Build payment sources based on payment method
        let payment_sources = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                let credit_card = BluesnapVaultedCreditCard {
                    card_number: Secret::new(card_data.card_number.peek().to_string()),
                    expiration_month: card_data.card_exp_month.clone(),
                    expiration_year: card_data.card_exp_year.clone(),
                    security_code: Some(card_data.card_cvc.clone()),
                };

                let billing_contact_info = billing_address.map(|addr| BluesnapBillingContactInfo {
                    first_name: addr.first_name.clone(),
                    last_name: addr.last_name.clone(),
                    address1: addr.line1.clone(),
                    address2: addr.line2.clone(),
                    city: addr.city.as_ref().map(|c| c.peek().to_string()),
                    state: addr.state.clone(),
                    zip: addr.zip.clone(),
                    country: addr.country.map(|c| c.to_string()),
                });

                BluesnapPaymentSources {
                    credit_card_info: Some(vec![BluesnapVaultedCreditCardInfo {
                        credit_card,
                        billing_contact_info,
                    }]),
                }
            }
            _ => {
                return Err(IntegrationError::NotImplemented(
                    "BlueSnap SetupMandate only supports Card payment method".to_string(),
                    Default::default(),
                )
                .into())
            }
        };

        Ok(Self {
            first_name,
            last_name,
            email: router_data.request.email.clone(),
            payment_sources,
        })
    }
}

/// Transform BlueSnap vaulted shopper response to SetupMandate response
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ResponseRouterData<
            BluesnapSetupMandateResponse,
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
        >,
    >
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            BluesnapSetupMandateResponse,
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
        >,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;

        // BlueSnap's POST /services/2/vaulted-shoppers is a customer/vault
        // resource creation — the response contains only `vaultedShopperId`
        // and no separate transaction id. We therefore use the same value
        // for both `resource_id` (the flow's only available identifier) and
        // `mandate_reference.connector_mandate_id` (the id downstream MIT
        // charges must reference via RepeatPayment). Subsequent MIT charges
        // do return a distinct `transactionId` via RepeatPayment, so this
        // dual use is confined to the SetupMandate response only.
        let connector_mandate_id = response.vaulted_shopper_id.to_string();

        let mandate_reference = Some(Box::new(MandateReference {
            connector_mandate_id: Some(connector_mandate_id.clone()),
            payment_method_id: None,
            connector_mandate_request_reference_id: None,
        }));

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(connector_mandate_id.clone()),
                redirection_data: None,
                mandate_reference,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(connector_mandate_id),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                // Vaulted-shoppers responses have no `processingInfo.processingStatus`
                // (it is a resource-creation endpoint, not a transaction); HTTP 201
                // is the success signal. `Charged` is the cross-connector convention
                // for "mandate setup succeeded" in this repo — see helcim/billwerk,
                // whose SetupMandate paths resolve to the same status.
                status: AttemptStatus::Charged,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== REPEAT PAYMENT (MIT via vaulted shopper) TRANSFORMERS =====

// Bluesnap recurring type marker for MIT-initiated subsequent charges.
// Passed as `recurringTransaction` in POST /services/2/transactions to
// identify this as a merchant-initiated charge on a stored payment source.
const BLUESNAP_RECURRING_TRANSACTION_ECOMMERCE: &str = "ECOMMERCE";

/// Transform RepeatPayment request to BlueSnap `POST /services/2/transactions`
/// body using the vaulted shopper id stored as `connector_mandate_id`.
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BluesnapRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for BluesnapRepeatPaymentRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: BluesnapRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Pull the connector mandate id (BlueSnap `vaultedShopperId`) from the
        // router request. BlueSnap only supports the ConnectorMandateId variant
        // here — network mandate ids / network-token NTIs aren't applicable to
        // its vaulted-shopper flow.
        let vaulted_shopper_id_str: String = match &router_data.request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(ids) => {
                ids.get_connector_mandate_id()
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "mandate_reference.connector_mandate_id",
                        context: Default::default(),
                    })?
            }
            MandateReferenceId::NetworkMandateId(_)
            | MandateReferenceId::NetworkTokenWithNTI(_) => {
                return Err(IntegrationError::NotImplemented(
                    "BlueSnap RepeatPayment only supports ConnectorMandateId (vaultedShopperId)"
                        .to_string(),
                    Default::default(),
                )
                .into());
            }
        };

        let vaulted_shopper_id: u64 = vaulted_shopper_id_str.parse::<u64>().map_err(|_| {
            IntegrationError::InvalidDataFormat {
                field_name: "mandate_reference.connector_mandate_id",
                context: Default::default(),
            }
        })?;

        let card_transaction_type = match router_data.request.capture_method {
            Some(common_enums::CaptureMethod::Manual) => BluesnapTxnType::AuthOnly,
            _ => BluesnapTxnType::AuthCapture,
        };

        let amount = utils::convert_amount(
            item.connector.amount_converter,
            router_data.request.minor_amount,
            router_data.request.currency,
        )?;

        // `creditCard` { cardLastFourDigits, cardType } is optional; BlueSnap
        // only needs it when a vaulted shopper has multiple stored payment
        // sources and you want to disambiguate. If the caller also passes a
        // full Card payload in this flow we expose last-four as a hint so the
        // subsequent charge is unambiguous.
        let credit_card = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                let pan = card_data.card_number.peek().to_string();
                let last_four = if pan.len() >= 4 {
                    Some(pan[pan.len() - 4..].to_string())
                } else {
                    None
                };
                Some(BluesnapRepeatCreditCard {
                    card_last_four_digits: last_four,
                    card_type: None,
                })
            }
            _ => None,
        };

        let merchant_txn_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        Ok(Self {
            amount,
            currency: router_data.request.currency,
            vaulted_shopper_id,
            card_transaction_type,
            recurring_transaction: Some(BLUESNAP_RECURRING_TRANSACTION_ECOMMERCE.to_string()),
            credit_card,
            merchant_transaction_id: Some(merchant_txn_id.clone()),
            transaction_fraud_info: Some(TransactionFraudInfo {
                fraud_session_id: merchant_txn_id,
            }),
        })
    }
}

/// Transform BlueSnap transaction response into a RepeatPayment router response.
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ResponseRouterData<
            BluesnapRepeatPaymentResponse,
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
        >,
    > for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            BluesnapRepeatPaymentResponse,
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
        >,
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
