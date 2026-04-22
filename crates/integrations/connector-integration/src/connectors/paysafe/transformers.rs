use common_enums::enums;
use common_utils::{ext_traits::ValueExt, request::Method};
use domain_types::{
    connector_flow::{Authorize, Capture, PaymentMethodToken, RSync, Refund, RepeatPayment, Void},
    connector_types::{
        MandateReference, MandateReferenceId, PaymentFlowData, PaymentMethodTokenResponse,
        PaymentMethodTokenizationData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        RepeatPaymentData, ResponseId,
    },
    payment_method_data::{
        BankDebitData, GpayTokenizationData, PaymentMethodData, PaymentMethodDataTypes, WalletData,
    },
    router_data::{ConnectorSpecificConfig, PaysafePaymentMethodDetails},
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::connectors::paysafe::PaysafeRouterData;
use crate::types::ResponseRouterData;
use domain_types::errors::ConnectorError;
use domain_types::errors::{IntegrationError, IntegrationErrorContext};

pub use super::requests::*;
pub use super::responses::*;

// Auth Type

#[derive(Debug, Clone)]
pub struct PaysafeAuthType {
    pub username: Secret<String>,
    pub password: Secret<String>,
    pub account_id: Option<PaysafePaymentMethodDetails>,
}

impl TryFrom<&ConnectorSpecificConfig> for PaysafeAuthType {
    type Error = IntegrationError;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Paysafe {
                username,
                password,
                account_id,
                ..
            } => Ok(Self {
                username: username.clone(),
                password: password.clone(),
                account_id: account_id.clone(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }),
        }
    }
}

// Mandate Metadata

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PaysafeMandateMetadata {
    pub initial_transaction_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaysafeMeta {
    pub payment_handle_token: Secret<String>,
}

// Helper Functions

fn create_paysafe_billing_details(
    resource_common_data: &PaymentFlowData,
) -> Result<Option<PaysafeBillingDetails>, error_stack::Report<IntegrationError>> {
    let billing_address = resource_common_data.get_billing_address()?;
    // Only send billing details if billing mandatory fields are available
    if let (Some(zip), Some(country), Some(state)) = (
        resource_common_data.get_optional_billing_zip(),
        resource_common_data.get_optional_billing_country(),
        billing_address.to_state_code_as_optional()?,
    ) {
        Ok(Some(PaysafeBillingDetails {
            nick_name: resource_common_data.get_optional_billing_first_name(),
            street: resource_common_data.get_optional_billing_line1(),
            street2: resource_common_data.get_optional_billing_line2(),
            city: resource_common_data.get_optional_billing_city(),
            zip,
            country,
            state,
        }))
    } else {
        Ok(None)
    }
}

// Status Mapping Functions

pub fn get_paysafe_payment_status(
    status: PaysafePaymentStatus,
    capture_method: Option<enums::CaptureMethod>,
) -> enums::AttemptStatus {
    match status {
        PaysafePaymentStatus::Completed => match capture_method {
            Some(enums::CaptureMethod::Manual) => enums::AttemptStatus::Authorized,
            Some(enums::CaptureMethod::Automatic) | None => enums::AttemptStatus::Charged,
            Some(enums::CaptureMethod::SequentialAutomatic)
            | Some(enums::CaptureMethod::ManualMultiple)
            | Some(enums::CaptureMethod::Scheduled) => enums::AttemptStatus::Unresolved,
        },
        PaysafePaymentStatus::Failed => enums::AttemptStatus::Failure,
        PaysafePaymentStatus::Pending | PaysafePaymentStatus::Processing => {
            enums::AttemptStatus::Pending
        }
        PaysafePaymentStatus::Cancelled => enums::AttemptStatus::Voided,
    }
}

impl TryFrom<PaysafePaymentHandleStatus> for enums::AttemptStatus {
    type Error = ConnectorError;
    fn try_from(item: PaysafePaymentHandleStatus) -> Result<Self, Self::Error> {
        match item {
            PaysafePaymentHandleStatus::Completed => Ok(Self::Authorized),
            PaysafePaymentHandleStatus::Failed
            | PaysafePaymentHandleStatus::Expired
            | PaysafePaymentHandleStatus::Error => Ok(Self::Failure),
            PaysafePaymentHandleStatus::Initiated => Ok(Self::AuthenticationPending),
            PaysafePaymentHandleStatus::Payable | PaysafePaymentHandleStatus::Processing => {
                Ok(Self::Pending)
            }
        }
    }
}

impl From<PaysafeSettlementStatus> for enums::AttemptStatus {
    fn from(item: PaysafeSettlementStatus) -> Self {
        match item {
            PaysafeSettlementStatus::Completed
            | PaysafeSettlementStatus::Pending
            | PaysafeSettlementStatus::Processing => Self::Charged,
            PaysafeSettlementStatus::Failed => Self::Failure,
            PaysafeSettlementStatus::Cancelled => Self::Voided,
        }
    }
}

impl From<PaysafeVoidStatus> for enums::AttemptStatus {
    fn from(item: PaysafeVoidStatus) -> Self {
        match item {
            PaysafeVoidStatus::Completed
            | PaysafeVoidStatus::Pending
            | PaysafeVoidStatus::Processing => Self::Voided,
            PaysafeVoidStatus::Failed => Self::Failure,
            PaysafeVoidStatus::Cancelled => Self::Voided,
        }
    }
}

impl From<PaysafeRefundStatus> for enums::RefundStatus {
    fn from(item: PaysafeRefundStatus) -> Self {
        match item {
            PaysafeRefundStatus::Completed => Self::Success,
            PaysafeRefundStatus::Failed | PaysafeRefundStatus::Cancelled => Self::Failure,
            PaysafeRefundStatus::Pending | PaysafeRefundStatus::Processing => Self::Pending,
        }
    }
}

impl TryFrom<&enums::BankType> for PaysafeAchAccountType {
    type Error = IntegrationError;
    fn try_from(bank_type: &enums::BankType) -> Result<Self, Self::Error> {
        match bank_type {
            enums::BankType::Checking => Ok(Self::Checking),
            enums::BankType::Savings => Ok(Self::Savings),
            _ => Err(IntegrationError::NotImplemented(
                format!(
                    "Bank type {:?} is not supported for ACH bank debit",
                    bank_type
                ),
                Default::default(),
            )),
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaysafeRouterData<
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
            T,
        >,
    > for PaysafePaymentMethodTokenRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PaysafeRouterData<
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        let auth = PaysafeAuthType::try_from(&item.router_data.connector_config)?;
        let account_id = auth
            .account_id
            .ok_or(IntegrationError::InvalidConnectorConfig {
                config: "account_id",
                context: Default::default(),
            })?;

        let currency = router_data.request.currency;
        let amount = router_data.request.amount;

        let (payment_method, payment_type, account_id) =
            match &router_data.request.payment_method_data {
                PaymentMethodData::Card(req_card) => {
                    let card = PaysafeCard {
                        card_num: req_card.card_number.clone(),
                        card_expiry: PaysafeCardExpiry {
                            month: req_card.card_exp_month.clone(),
                            year: req_card.get_expiry_year_4_digit(),
                        },
                        cvv: if req_card.card_cvc.peek().is_empty() {
                            None
                        } else {
                            Some(req_card.card_cvc.clone())
                        },
                        holder_name: req_card.card_holder_name.clone().or_else(|| {
                            router_data
                                .resource_common_data
                                .get_optional_billing_full_name()
                        }),
                    };
                    let account_id = account_id.get_no_three_ds_account_id(currency)?;
                    (
                        PaysafePaymentMethod::Card { card },
                        PaysafePaymentType::Card,
                        account_id,
                    )
                }
                PaymentMethodData::BankDebit(BankDebitData::AchBankDebit {
                    account_number,
                    routing_number,
                    bank_account_holder_name,
                    bank_type,
                    ..
                }) => {
                    let account_holder_name = bank_account_holder_name
                        .clone()
                        .or_else(|| {
                            router_data
                                .resource_common_data
                                .get_optional_billing_full_name()
                        })
                        .ok_or(IntegrationError::MissingRequiredField {
                            field_name: "bank_account_holder_name",
                            context: Default::default(),
                        })?;
                    let account_type = bank_type
                        .as_ref()
                        .map(PaysafeAchAccountType::try_from)
                        .transpose()?
                        .ok_or(IntegrationError::MissingRequiredField {
                            field_name: "bank_type (ach.accountType)",
                            context: Default::default(),
                        })?;
                    let ach = PaysafeAch {
                        account_holder_name,
                        account_number: account_number.clone(),
                        routing_number: routing_number.clone(),
                        account_type,
                    };
                    let account_id = account_id.get_ach_account_id(currency)?;
                    (
                        PaysafePaymentMethod::Ach { ach },
                        PaysafePaymentType::Ach,
                        account_id,
                    )
                }
                PaymentMethodData::Wallet(WalletData::GooglePay(google_pay_data)) => {
                    let decrypted_data = match &google_pay_data.tokenization_data {
                        GpayTokenizationData::Decrypted(d) => d,
                        GpayTokenizationData::Encrypted(_) => {
                            return Err(IntegrationError::MissingRequiredField {
                                field_name: "google_pay.tokenization_data (decrypted)",
                                context: Default::default(),
                            }
                            .into())
                        }
                    };

                    let expiration_month = decrypted_data
                        .get_expiry_month()
                        .change_context(IntegrationError::MissingRequiredField {
                            field_name: "google_pay_decrypted_data.card_exp_month",
                            context: Default::default(),
                        })?
                        .peek()
                        .parse::<u8>()
                        .map_err(|_| {
                            IntegrationError::InvalidDataFormat {
                                field_name: "google_pay_decrypted_data.card_exp_month",
                                context: Default::default(),
                            }
                        })?;

                    let expiration_year = decrypted_data
                        .get_four_digit_expiry_year()
                        .change_context(IntegrationError::MissingRequiredField {
                            field_name: "google_pay_decrypted_data.card_exp_year",
                            context: Default::default(),
                        })?
                        .peek()
                        .parse::<u16>()
                        .map_err(|_| {
                            IntegrationError::InvalidDataFormat {
                                field_name: "google_pay_decrypted_data.card_exp_year",
                                context: Default::default(),
                            }
                        })?;

                    let pan = Secret::new(
                        decrypted_data
                            .application_primary_account_number
                            .get_card_no(),
                    );

                    let auth_method = if decrypted_data.cryptogram.is_some() {
                        PaysafeGooglePayAuthMethod::Cryptogram3Ds
                    } else {
                        PaysafeGooglePayAuthMethod::PanOnly
                    };

                    let payment_method_details = PaysafeGooglePayPaymentMethodDetails {
                        auth_method,
                        pan,
                        expiration_month,
                        expiration_year,
                        cryptogram: decrypted_data.cryptogram.clone(),
                    };

                    // TODO(https://github.com/juspay/hyperswitch/issues/11684): HS parses
                    // message_id and message_expiration from the decrypted GPay payload
                    // internally but drops them before forwarding to UCS via GPayPredecryptData.
                    // Until HS propagates these fields, we fall back to a random UUID for
                    // message_id (losing Paysafe's replay-detection guarantee) and a far-future
                    // placeholder for message_expiration.
                    let decrypted_token = PaysafeGooglePayDecryptedToken {
                        message_id: uuid::Uuid::new_v4().to_string(),
                        message_expiration: "9999999999999".to_string(),
                        payment_method_details,
                    };

                    let google_pay_payment_token = PaysafeGooglePayPaymentToken {
                        api_version: 2,
                        api_version_minor: 0,
                        payment_method_data: PaysafeGooglePayPaymentMethodData {
                            pm_type: "CARD".to_string(),
                            description: google_pay_data.description.clone(),
                            info: PaysafeGooglePayCardInfo {
                                card_network: google_pay_data.info.card_network.clone(),
                                card_details: google_pay_data.info.card_details.clone(),
                            },
                            tokenization_data: PaysafeGooglePayTokenizationData {
                                token_type: "PAYMENT_GATEWAY".to_string(),
                                decrypted_token,
                            },
                        },
                    };

                    let account_id = account_id.get_no_three_ds_account_id(currency)?;
                    (
                        PaysafePaymentMethod::GooglePay {
                            google_pay: PaysafeGooglePay {
                                google_pay_payment_token,
                            },
                        },
                        PaysafePaymentType::Card,
                        account_id,
                    )
                }
                _ => {
                    return Err(IntegrationError::NotImplemented("Only card, ACH, and GooglePay payment methods are supported for PaymentMethodToken"
                            .to_string() , Default::default())
                    .into())
                }
            };

        // For ACH payments, Paysafe requires settleWithAuth to be true
        // For Card (including GooglePay which maps to Card), settle based on capture_method
        let settle_with_auth = match payment_type {
            PaysafePaymentType::Ach => true,
            PaysafePaymentType::Card => matches!(
                router_data.request.capture_method,
                Some(enums::CaptureMethod::Automatic) | None
            ),
        };

        let billing_details = create_paysafe_billing_details(&router_data.resource_common_data)?;

        // Paysafe requires return_links even for no-3DS flows
        let redirect_url = router_data.resource_common_data.get_return_url().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "return_url",
                context: Default::default(),
            },
        )?;

        let return_links = Some(vec![
            ReturnLink {
                rel: LinkType::Default,
                href: redirect_url.clone(),
                method: Method::Get.to_string(),
            },
            ReturnLink {
                rel: LinkType::OnCompleted,
                href: redirect_url.clone(),
                method: Method::Get.to_string(),
            },
            ReturnLink {
                rel: LinkType::OnFailed,
                href: redirect_url.clone(),
                method: Method::Get.to_string(),
            },
            ReturnLink {
                rel: LinkType::OnCancelled,
                href: redirect_url,
                method: Method::Get.to_string(),
            },
        ]);

        Ok(Self {
            merchant_ref_num: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            amount,
            settle_with_auth,
            payment_method,
            currency_code: currency,
            payment_type,
            transaction_type: TransactionType::Payment,
            return_links,
            account_id,
            three_ds: None, // No 3DS for PaymentMethodToken
            profile: None,
            billing_details,
        })
    }
}

// PaymentMethodToken (No-3DS) Flow - Response

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<PaysafePaymentMethodTokenResponse, Self>>
    for RouterDataV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaysafePaymentMethodTokenResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = enums::AttemptStatus::try_from(item.response.status)?;

        let mut router_data = item.router_data;
        router_data.resource_common_data.status = status;

        // Return the payment_handle_token as the payment method token
        Ok(Self {
            response: Ok(PaymentMethodTokenResponse {
                token: item.response.payment_handle_token.peek().to_string(),
            }),
            ..router_data
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaysafeRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PaysafePaymentsRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PaysafeRouterData<
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
        let amount = router_data.request.minor_amount;

        let auth = PaysafeAuthType::try_from(&item.router_data.connector_config)?;
        let account_id = auth
            .account_id
            .ok_or(IntegrationError::InvalidConnectorConfig {
                config: "account_id",
                context: Default::default(),
            })?;

        let payment_handle_token: Secret<String> = match &router_data.request.payment_method_data {
            PaymentMethodData::PaymentMethodToken(t) => t.token.clone(),
            _ => router_data
                .resource_common_data
                .connector_feature_data
                .as_ref()
                .and_then(|metadata_value| {
                    metadata_value
                        .clone()
                        .parse_value::<PaysafeMeta>("PaysafeMeta")
                        .ok()
                        .map(|meta| meta.payment_handle_token)
                })
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "payment_method_token",
                    context: IntegrationErrorContext {
                        suggested_action: Some("Obtain a Paysafe payment_handle_token via PaymentMethodService.Tokenize before authorizing.".to_string()),
                        doc_url: Some("https://developer.paysafe.com/en/payments/payment-handles/create-payment-handle/".to_string()),
                        additional_context: Some("Paysafe requires a payment handle token. Pass it via PaymentMethodData::PaymentMethodToken or connector_feature_data metadata.".to_string()),
                    },
                })?,
        };

        let customer_ip = router_data
            .request
            .get_browser_info()
            .ok()
            .and_then(|browser_info| browser_info.ip_address)
            .map(|ip| Secret::new(ip.to_string()));

        // Determine if this is an ACH payment based on payment_method
        let is_ach = matches!(
            router_data.resource_common_data.payment_method,
            enums::PaymentMethod::BankDebit
        );

        // For ACH payments, Paysafe requires settleWithAuth to be true
        let settle_with_auth = if is_ach {
            true
        } else {
            matches!(
                router_data.request.capture_method,
                Some(enums::CaptureMethod::Automatic) | None
            )
        };

        // For ACH, use the ach account_id; for wallets (GooglePay), always use no_three_ds
        // because the PaymentMethodToken (payment handle) was created under the no_three_ds
        // account. For cards, branch on is_three_ds().
        let is_wallet = matches!(
            router_data.resource_common_data.payment_method,
            enums::PaymentMethod::Wallet
        );

        let account_id = Some(if is_ach {
            account_id.get_ach_account_id(router_data.request.currency)?
        } else if is_wallet || !router_data.resource_common_data.is_three_ds() {
            // Wallets (GooglePay) always use no_three_ds account because the payment handle
            // (created in PaymentMethodToken flow) uses no_three_ds account.
            // Non-3DS card payments also use no_three_ds.
            account_id.get_no_three_ds_account_id(router_data.request.currency)?
        } else {
            account_id.get_three_ds_account_id(router_data.request.currency)?
        });

        Ok(Self {
            merchant_ref_num: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            payment_handle_token,
            amount,
            settle_with_auth,
            currency_code: router_data.request.currency,
            customer_ip,
            stored_credential: None,
            account_id,
        })
    }
}

// Authorize Flow - Response

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<PaysafeAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaysafeAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = get_paysafe_payment_status(
            item.response.status,
            item.router_data.request.capture_method,
        );

        // Store payment_handle_token for mandate if present
        let mandate_reference =
            item.response
                .payment_handle_token
                .as_ref()
                .map(|token| MandateReference {
                    connector_mandate_id: Some(token.peek().to_string()),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                });

        let mut router_data = item.router_data;
        router_data.resource_common_data.status = status;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: mandate_reference.map(Box::new),
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.merchant_ref_num),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..router_data
        })
    }
}

// RepeatPayment Flow - Request

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaysafeRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PaysafeRepeatPaymentRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PaysafeRouterData<
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
        let amount = router_data.request.minor_amount;

        // Get mandate ID and metadata
        let (payment_handle_token, mandate_data) = match &router_data.request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(mandate_data) => {
                let token = mandate_data
                    .get_connector_mandate_id()
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "connector_mandate_id",
                        context: Default::default(),
                    })?
                    .into();
                (token, mandate_data)
            }
            MandateReferenceId::NetworkMandateId(_)
            | MandateReferenceId::NetworkTokenWithNTI(_) => {
                return Err(IntegrationError::MissingRequiredField {
                    field_name: "connector_mandate_id",
                    context: Default::default(),
                }
                .into());
            }
        };

        let mandate_metadata: PaysafeMandateMetadata = mandate_data
            .get_mandate_metadata()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "mandate_metadata",
                context: Default::default(),
            })?
            .parse_value("PaysafeMandateMetadata")
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        let customer_ip = router_data
            .request
            .browser_info
            .as_ref()
            .and_then(|browser_info| browser_info.ip_address.as_ref())
            .map(|ip| Secret::new(ip.to_string()));

        let settle_with_auth = matches!(
            router_data.request.capture_method,
            Some(enums::CaptureMethod::Automatic) | None
        );

        // MIT (Merchant Initiated Transaction) stored credential
        let stored_credential = Some(PaysafeStoredCredential {
            stored_credential_type: PaysafeStoredCredentialType::Topup,
            occurrence: MandateOccurrence::Subsequent,
            initial_transaction_id: Some(mandate_metadata.initial_transaction_id),
        });

        Ok(Self {
            merchant_ref_num: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            payment_handle_token,
            amount,
            settle_with_auth,
            currency_code: router_data.request.currency,
            customer_ip,
            stored_credential,
            account_id: None,
        })
    }
}

// RepeatPayment Flow - Response

impl<
        T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize + Serialize,
    > TryFrom<ResponseRouterData<PaysafeRepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaysafeRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = get_paysafe_payment_status(
            item.response.status,
            item.router_data.request.capture_method,
        );

        let mut router_data = item.router_data;
        router_data.resource_common_data.status = status;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.merchant_ref_num),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..router_data
        })
    }
}

// PSync Flow - Response

impl TryFrom<ResponseRouterData<PaysafeSyncResponse, Self>>
    for RouterDataV2<
        domain_types::connector_flow::PSync,
        PaymentFlowData,
        domain_types::connector_types::PaymentsSyncData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<PaysafeSyncResponse, Self>) -> Result<Self, Self::Error> {
        let (status, connector_transaction_id) = match &item.response {
            PaysafeSyncResponse::SinglePayment(payment_response) => {
                let status = get_paysafe_payment_status(
                    payment_response.status,
                    item.router_data.request.capture_method,
                );
                (status, Some(payment_response.id.clone()))
            }
            PaysafeSyncResponse::Payments(sync_response) => {
                let payment_response = sync_response.payments.first().ok_or_else(|| {
                    error_stack::Report::from(
                        crate::utils::response_deserialization_fail(
                            item.http_code,
                        "paysafe: response body did not match the expected format; confirm API version and connector documentation."),
                    )
                })?;
                let status = get_paysafe_payment_status(
                    payment_response.status,
                    item.router_data.request.capture_method,
                );
                (status, Some(payment_response.id.clone()))
            }
            PaysafeSyncResponse::SinglePaymentHandle(payment_handle_response) => {
                let status = enums::AttemptStatus::try_from(payment_handle_response.status)?;
                (status, Some(payment_handle_response.id.clone()))
            }
            PaysafeSyncResponse::PaymentHandle(sync_response) => {
                let payment_handle_response =
                    sync_response.payment_handles.first().ok_or_else(|| {
                        error_stack::Report::from(
                            crate::utils::response_deserialization_fail(
                                item.http_code,
                            "paysafe: response body did not match the expected format; confirm API version and connector documentation."),
                        )
                    })?;
                let status = enums::AttemptStatus::try_from(payment_handle_response.status)?;
                (status, Some(payment_handle_response.id.clone()))
            }
        };

        let mut router_data = item.router_data;
        router_data.resource_common_data.status = status;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: connector_transaction_id
                    .map(ResponseId::ConnectorTransactionId)
                    .unwrap_or(ResponseId::NoResponseId),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..router_data
        })
    }
}

// Capture Flow - Request

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaysafeRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for PaysafeCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PaysafeRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            merchant_ref_num: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            amount: item.router_data.request.minor_amount_to_capture,
        })
    }
}

// Capture Flow - Response

impl TryFrom<ResponseRouterData<PaysafeCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaysafeCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = enums::AttemptStatus::from(item.response.status);

        let mut router_data = item.router_data;
        router_data.resource_common_data.status = status;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.merchant_ref_num),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..router_data
        })
    }
}

// Void Flow - Request

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaysafeRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for PaysafeVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PaysafeRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount =
            item.router_data
                .request
                .amount
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "amount",
                    context: Default::default(),
                })?;
        Ok(Self {
            merchant_ref_num: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            amount,
        })
    }
}

// Void Flow - Response

impl TryFrom<ResponseRouterData<PaysafeVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<PaysafeVoidResponse, Self>) -> Result<Self, Self::Error> {
        let status = enums::AttemptStatus::from(item.response.status);

        let mut router_data = item.router_data;
        router_data.resource_common_data.status = status;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::NoResponseId,
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.merchant_ref_num),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..router_data
        })
    }
}

// Refund Flow - Request

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaysafeRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for PaysafeRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PaysafeRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            merchant_ref_num: item.router_data.request.refund_id.clone(),
            amount: item.router_data.request.minor_refund_amount,
        })
    }
}

// Refund Flow - Response

impl TryFrom<ResponseRouterData<PaysafeRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaysafeRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id.clone(),
                refund_status: enums::RefundStatus::from(item.response.status),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// RSync Flow - Response

impl TryFrom<ResponseRouterData<PaysafeRSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<PaysafeRSyncResponse, Self>) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id.clone(),
                refund_status: enums::RefundStatus::from(item.response.status),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}
