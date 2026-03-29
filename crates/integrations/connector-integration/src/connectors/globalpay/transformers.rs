use crate::{
    connectors::globalpay::{GlobalpayAmountConvertor, GlobalpayRouterData},
    types::ResponseRouterData,
};
use common_enums::{AttemptStatus, RefundStatus};
use common_utils::request::Method;
use common_utils::types::StringMinorUnit;
use domain_types::{
    connector_flow::{Authorize, Capture, CreateAccessToken, PSync, RSync, Refund, Void},
    connector_types::{
        AccessTokenRequestData, AccessTokenResponseData, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, ResponseId,
    },
    errors,
    payment_method_data::{
        BankRedirectData, GpayTokenizationData, PaymentMethodData, PaymentMethodDataTypes,
        RawCardNumber, WalletData,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use rand::distributions::DistString;
use serde::{Deserialize, Serialize};
use url::Url;

// ===== TYPE ALIASES FOR MACRO =====
// These type aliases are needed because the create_all_prerequisites! macro
// creates unique "Templating" structs for each response type, but GlobalPay
// reuses the same response types across multiple flows. To avoid duplication errors,
// we create flow-specific aliases that reference the same underlying types.

/// Response type for Authorize flow - reuses GlobalpayPaymentsResponse
pub type GlobalpayAuthorizeResponse = GlobalpayPaymentsResponse;
/// Response type for PSync flow - reuses GlobalpayPaymentsResponse
pub type GlobalpayPSyncResponse = GlobalpayPaymentsResponse;
/// Response type for Void flow - reuses GlobalpayPaymentsResponse
pub type GlobalpayVoidResponse = GlobalpayPaymentsResponse;
/// Response type for Capture flow - reuses GlobalpayPaymentsResponse
pub type GlobalpayCaptureResponse = GlobalpayPaymentsResponse;
/// Response type for RSync flow - reuses GlobalpayRefundResponse
pub type GlobalpayRSyncResponse = GlobalpayRefundResponse;

// ===== CONSTANTS =====

mod constants {

    /// Entry mode for e-commerce transactions
    pub(super) const ENTRY_MODE_ECOM: &str = "ECOM";

    /// Account name for transaction processing
    pub(super) const ACCOUNT_NAME: &str = "transaction_processing";

    /// Channel for card-not-present transactions
    pub(super) const CHANNEL_CNP: &str = "CNP";
}

#[derive(Debug, Clone)]
pub struct GlobalpayAuthType {
    pub app_id: Secret<String>,
    pub app_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for GlobalpayAuthType {
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Globalpay {
                app_id, app_key, ..
            } => Ok(Self {
                app_id: app_id.to_owned(),
                app_key: app_key.to_owned(),
            }),
            _ => Err(error_stack::report!(
                errors::ConnectorError::FailedToObtainAuthType
            )),
        }
    }
}

#[derive(Debug, Clone, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct GlobalpayErrorResponse {
    pub error_code: String,
    pub detailed_error_code: String,
    pub detailed_error_description: String,
}

// ===== STATUS ENUMS =====

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GlobalpayPaymentStatus {
    Captured,
    Preauthorized,
    Declined,
    Failed,
    Rejected,
    Pending,
    Initiated,
    ForReview,
    Funded,
    Reversed,
}

impl From<GlobalpayPaymentStatus> for AttemptStatus {
    fn from(status: GlobalpayPaymentStatus) -> Self {
        match status {
            GlobalpayPaymentStatus::Captured => Self::Charged,
            GlobalpayPaymentStatus::Preauthorized => Self::Authorized,
            GlobalpayPaymentStatus::Declined => Self::Failure,
            GlobalpayPaymentStatus::Failed => Self::Failure,
            GlobalpayPaymentStatus::Rejected => Self::Failure,
            GlobalpayPaymentStatus::Pending => Self::Pending,
            GlobalpayPaymentStatus::Initiated => Self::AuthenticationPending,
            GlobalpayPaymentStatus::ForReview => Self::Pending,
            GlobalpayPaymentStatus::Funded => Self::Charged,
            GlobalpayPaymentStatus::Reversed => Self::Voided,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GlobalpayRefundStatus {
    Captured,
    Funded,
    Pending,
    Initiated,
    ForReview,
    Declined,
    Failed,
    Rejected,
}

impl From<GlobalpayRefundStatus> for RefundStatus {
    fn from(status: GlobalpayRefundStatus) -> Self {
        match status {
            GlobalpayRefundStatus::Captured | GlobalpayRefundStatus::Funded => Self::Success,
            GlobalpayRefundStatus::Pending
            | GlobalpayRefundStatus::Initiated
            | GlobalpayRefundStatus::ForReview => Self::Pending,
            GlobalpayRefundStatus::Declined
            | GlobalpayRefundStatus::Failed
            | GlobalpayRefundStatus::Rejected => Self::Failure,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Sequence {
    First,
    Last,
    Subsequent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GlobalpayCaptureMode {
    Auto,
    Later,
}

// ===== OAUTH / ACCESS TOKEN FLOW STRUCTURES =====

#[derive(Debug, Serialize)]
pub struct GlobalpayAccessTokenRequest {
    pub app_id: Secret<String>,
    pub nonce: Secret<String>,
    pub secret: Secret<String>,
    pub grant_type: String,
}

impl
    TryFrom<
        &RouterDataV2<
            CreateAccessToken,
            PaymentFlowData,
            AccessTokenRequestData,
            AccessTokenResponseData,
        >,
    > for GlobalpayAccessTokenRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: &RouterDataV2<
            CreateAccessToken,
            PaymentFlowData,
            AccessTokenRequestData,
            AccessTokenResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        if let ConnectorSpecificConfig::Globalpay {
            app_id, app_key, ..
        } = &item.connector_config
        {
            use sha2::{Digest, Sha512};

            // Generate random alphanumeric nonce (matching Hyperswitch implementation)
            let nonce =
                rand::distributions::Alphanumeric.sample_string(&mut rand::thread_rng(), 12);

            // Create secret: SHA512(nonce + app_key)
            let secret_input = format!("{}{}", nonce, app_key.peek());

            // Generate SHA-512 hash
            let mut hasher = Sha512::new();
            hasher.update(secret_input.as_bytes());
            let result = hasher.finalize();
            let secret_hex = hex::encode(result);

            Ok(Self {
                app_id: app_id.clone(),
                nonce: Secret::new(nonce),
                secret: Secret::new(secret_hex),
                grant_type: item.request.grant_type.clone(),
            })
        } else {
            Err(error_stack::report!(
                errors::ConnectorError::FailedToObtainAuthType
            ))
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GlobalpayAccessTokenResponse {
    pub token: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub seconds_to_expire: i64,
}

impl<F, T> TryFrom<ResponseRouterData<GlobalpayAccessTokenResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, AccessTokenResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlobalpayAccessTokenResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(AccessTokenResponseData {
                access_token: item.response.token.into(),
                token_type: Some(item.response.type_),
                expires_in: Some(item.response.seconds_to_expire),
            }),
            ..item.router_data
        })
    }
}

// ===== PAYMENT FLOW STRUCTURES =====

#[derive(Debug, Serialize)]
pub struct GlobalpayNotifications {
    pub cancel_url: String,
    pub return_url: String,
    pub status_url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InitiatorType {
    Merchant,
    Payer,
}

#[derive(Debug, Serialize)]
pub struct Initiator {
    #[serde(rename = "type")]
    pub initiator_type: Option<InitiatorType>,
    pub id: Option<String>,
    pub stored_credential: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StoredCredentialType {
    Installment,
    Recurring,
    Unscheduled,
    Subscription,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StoredCredentialSequence {
    First,
    Subsequent,
}

#[derive(Debug, Serialize)]
pub struct StoredCredential {
    #[serde(rename = "type")]
    pub credential_type: Option<StoredCredentialType>,
    pub sequence: Option<StoredCredentialSequence>,
    pub initiator: Option<InitiatorType>,
}

// ===== APM / BANK REDIRECT STRUCTURES =====

/// APM (Alternative Payment Method) provider for bank redirect payments
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApmProvider {
    Giropay,
    Ideal,
    Paypal,
    Sofort,
    Eps,
    Testpay,
}

/// APM payment method data for bank redirect flows
#[derive(Debug, Serialize)]
pub struct GlobalpayApm {
    /// A string used to identify the payment method provider being used to execute this transaction.
    pub provider: Option<ApmProvider>,
}

#[derive(Debug, Serialize)]
pub struct GlobalpayPaymentsRequest<T: PaymentMethodDataTypes> {
    pub account_name: String,
    pub channel: String,
    pub amount: StringMinorUnit,
    pub currency: common_enums::Currency,
    pub reference: String,
    pub country: common_enums::CountryAlpha2,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_mode: Option<GlobalpayCaptureMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initiator: Option<Initiator>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notifications: Option<GlobalpayNotifications>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stored_credential: Option<StoredCredential>,
    pub payment_method: GlobalpayPaymentMethod<T>,
}

#[derive(Debug, Serialize)]
pub struct GlobalpayPaymentMethod<T: PaymentMethodDataTypes> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Secret<String>>,
    pub entry_mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<GlobalpayCard<T>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apm: Option<GlobalpayApm>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digital_wallet: Option<GlobalpayDigitalWallet>,
}

#[derive(Debug, Serialize)]
pub struct GlobalpayDigitalWallet {
    pub provider: GlobalpayDigitalWalletProvider,
    pub payment_token: GlobalpayPaymentToken,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GlobalpayDigitalWalletProvider {
    PayByGoogle,
}

#[derive(Debug, Serialize)]
pub struct GlobalpayPaymentToken {
    pub signature: String,
    pub protocol_version: String,
    pub signed_message: String,
}

#[derive(Debug, Serialize)]
pub struct GlobalpayCard<T: PaymentMethodDataTypes> {
    pub number: RawCardNumber<T>,
    pub expiry_month: Secret<String>,
    pub expiry_year: Secret<String>,
    pub cvv: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cvv_indicator: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GlobalpayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for GlobalpayPaymentsRequest<T>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        wrapper: GlobalpayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &wrapper.router_data;
        let payment_method = match &item.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                // Convert to 2-digit year using built-in helper method
                let expiry_year_2digit = card_data
                    .get_card_expiry_year_2_digit()
                    .change_context(errors::ConnectorError::RequestEncodingFailed)?;

                // Determine cvv_indicator based on whether CVV is provided
                let cvv_indicator = if card_data.card_cvc.peek().is_empty() {
                    Some("NOT_PRESENT".to_string())
                } else {
                    Some("PRESENT".to_string())
                };

                GlobalpayPaymentMethod {
                    name: item.request.customer_name.clone().map(Secret::new),
                    entry_mode: constants::ENTRY_MODE_ECOM.to_string(),
                    card: Some(GlobalpayCard {
                        number: card_data.card_number.clone(),
                        expiry_month: card_data.card_exp_month.clone(),
                        expiry_year: expiry_year_2digit,
                        cvv: card_data.card_cvc.clone(),
                        cvv_indicator,
                    }),
                    apm: None,
                    digital_wallet: None,
                }
            }
            PaymentMethodData::BankRedirect(bank_redirect) => {
                let apm_provider = match bank_redirect {
                    BankRedirectData::Eps { .. } => Some(ApmProvider::Eps),
                    BankRedirectData::Ideal { .. } => Some(ApmProvider::Ideal),
                    _ => {
                        return Err(error_stack::report!(
                            errors::ConnectorError::NotImplemented(
                                "Bank redirect payment method not supported".to_string()
                            )
                        ))
                    }
                };

                GlobalpayPaymentMethod {
                    name: item.request.customer_name.clone().map(Secret::new),
                    entry_mode: constants::ENTRY_MODE_ECOM.to_string(),
                    card: None,
                    apm: Some(GlobalpayApm {
                        provider: apm_provider,
                    }),
                    digital_wallet: None,
                }
            }
            PaymentMethodData::Wallet(WalletData::GooglePay(google_pay_data)) => {
                // Get the encrypted token data from Google Pay
                let encrypted_token = match &google_pay_data.tokenization_data {
                    GpayTokenizationData::Encrypted(encrypted_data) => encrypted_data,
                    GpayTokenizationData::Decrypted(_) => {
                        return Err(error_stack::report!(
                            errors::ConnectorError::InvalidWalletToken {
                                wallet_name: "Google Pay".to_string(),
                            }
                        ))
                    }
                };

                GlobalpayPaymentMethod {
                    name: item.request.customer_name.clone().map(Secret::new),
                    entry_mode: constants::ENTRY_MODE_ECOM.to_string(),
                    card: None,
                    apm: None,
                    digital_wallet: Some(GlobalpayDigitalWallet {
                        provider: GlobalpayDigitalWalletProvider::PayByGoogle,
                        payment_token: GlobalpayPaymentToken {
                            signature: "".to_string(), // GlobalPay uses encrypted token as signed_message
                            protocol_version: encrypted_token.token_type.clone(),
                            signed_message: encrypted_token.token.clone(),
                        },
                    }),
                }
            }
            _ => {
                return Err(error_stack::report!(
                    errors::ConnectorError::NotImplemented(
                        "Payment method not supported".to_string()
                    )
                ))
            }
        };

        // Determine capture_mode based on capture_method
        let capture_mode = match item.request.capture_method {
            Some(common_enums::CaptureMethod::Manual) => Some(GlobalpayCaptureMode::Later),
            _ => Some(GlobalpayCaptureMode::Auto),
        };

        // Get country from billing address or use default
        let country = item
            .resource_common_data
            .get_billing_country()
            .unwrap_or(common_enums::CountryAlpha2::US);

        // Build notifications object from router data
        let notifications = if let (Some(return_url), Some(webhook_url)) = (
            item.request.router_return_url.as_ref(),
            item.request.webhook_url.as_ref(),
        ) {
            Some(GlobalpayNotifications {
                cancel_url: return_url.clone(),
                return_url: return_url.clone(),
                status_url: webhook_url.clone(),
            })
        } else {
            None
        };

        Ok(Self {
            account_name: constants::ACCOUNT_NAME.to_string(),
            channel: constants::CHANNEL_CNP.to_string(),
            amount: GlobalpayAmountConvertor::convert(
                item.request.minor_amount,
                item.request.currency,
            )
            .change_context(errors::ConnectorError::RequestEncodingFailed)?,
            currency: item.request.currency,
            reference: item
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            country,
            capture_mode,
            initiator: None,
            notifications,
            stored_credential: None,
            payment_method,
        })
    }
}

// Capture Request Structure
#[derive(Debug, Serialize)]
pub struct GlobalpayCaptureRequest {
    pub amount: StringMinorUnit,
    pub capture_sequence: Option<Sequence>,
    pub reference: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GlobalpayRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for GlobalpayCaptureRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        wrapper: GlobalpayRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &wrapper.router_data;

        Ok(Self {
            amount: GlobalpayAmountConvertor::convert(
                item.request.minor_amount_to_capture,
                item.request.currency,
            )
            .change_context(errors::ConnectorError::RequestEncodingFailed)?,
            capture_sequence: item.request.multiple_capture_data.as_ref().map(|mcd| {
                if mcd.capture_sequence == 1 {
                    Sequence::First
                } else {
                    Sequence::Subsequent
                }
            }),
            reference: item
                .request
                .multiple_capture_data
                .as_ref()
                .map(|mcd| mcd.capture_reference.clone()),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GlobalpayPaymentsResponse {
    pub id: String,
    pub status: GlobalpayPaymentStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<StringMinorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<common_enums::Currency>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method: Option<GlobalpayPaymentMethodResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GlobalpayPaymentMethodResponse {
    pub card: Option<GlobalpayCardResponse>,
    pub apm: Option<GlobalpayApmResponse>,
    pub id: Option<Secret<String>>,
    pub message: Option<String>,
    pub result: Option<String>,
}

/// Data associated with the response of an APM transaction
#[derive(Debug, Deserialize, Serialize)]
pub struct GlobalpayApmResponse {
    #[serde(alias = "provider_redirect_url")]
    pub redirect_url: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GlobalpayCardResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brand_reference: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub masked_number_last4: Option<String>,
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<GlobalpayPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlobalpayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Extract redirect URL from APM response for bank redirect flows
        let redirect_url = item
            .response
            .payment_method
            .as_ref()
            .and_then(|payment_method| {
                payment_method
                    .apm
                    .as_ref()
                    .and_then(|apm| apm.redirect_url.as_ref())
            })
            .filter(|redirect_str| !redirect_str.is_empty())
            .map(|url| {
                Url::parse(url).change_context(errors::ConnectorError::FailedToObtainIntegrationUrl)
            })
            .transpose()?;

        let redirection_data = redirect_url
            .as_ref()
            .map(|url| Box::new(RedirectForm::from((url.clone(), Method::Get))));

        // Determine status based on connector status and presence of redirect
        let status = AttemptStatus::from(item.response.status.clone());

        // Extract network transaction ID from card response
        let network_txn_id = item
            .response
            .payment_method
            .as_ref()
            .and_then(|pm| pm.card.as_ref())
            .and_then(|card| card.brand_reference.as_ref())
            .map(|s| s.peek().to_string());

        // Handle failure responses separately
        let response = match status {
            AttemptStatus::Failure => Err(ErrorResponse {
                status_code: item.http_code,
                code: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.result.clone())
                    .unwrap_or_else(|| "UNKNOWN_ERROR".to_string()),
                message: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.message.clone())
                    .unwrap_or_else(|| "Transaction failed".to_string()),
                reason: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.message.clone()),
                attempt_status: Some(status),
                connector_transaction_id: Some(item.response.id.clone()),
                network_decline_code: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.result.clone()),
                network_advice_code: None,
                network_error_message: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.message.clone()),
            }),
            _ => Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id,
                connector_response_reference_id: item.response.reference.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
        };

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// PSync flow - reuses the same GlobalpayPaymentsResponse structure
impl TryFrom<ResponseRouterData<GlobalpayPaymentsResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlobalpayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::from(item.response.status.clone());

        // Extract network transaction ID from card response
        let network_txn_id = item
            .response
            .payment_method
            .as_ref()
            .and_then(|pm| pm.card.as_ref())
            .and_then(|card| card.brand_reference.as_ref())
            .map(|s| s.peek().to_string());

        // Handle failure responses separately
        let response = match status {
            AttemptStatus::Failure => Err(ErrorResponse {
                status_code: item.http_code,
                code: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.result.clone())
                    .unwrap_or_else(|| "UNKNOWN_ERROR".to_string()),
                message: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.message.clone())
                    .unwrap_or_else(|| "Transaction failed".to_string()),
                reason: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.message.clone()),
                attempt_status: Some(status),
                connector_transaction_id: Some(item.response.id.clone()),
                network_decline_code: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.result.clone()),
                network_advice_code: None,
                network_error_message: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.message.clone()),
            }),
            _ => Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id,
                connector_response_reference_id: item.response.reference.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
        };

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// Capture flow - reuses the same GlobalpayPaymentsResponse structure
impl TryFrom<ResponseRouterData<GlobalpayPaymentsResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlobalpayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::from(item.response.status.clone());

        // Extract network transaction ID from card response
        let network_txn_id = item
            .response
            .payment_method
            .as_ref()
            .and_then(|pm| pm.card.as_ref())
            .and_then(|card| card.brand_reference.as_ref())
            .map(|s| s.peek().to_string());

        // Handle failure responses separately
        let response = match status {
            AttemptStatus::Failure => Err(ErrorResponse {
                status_code: item.http_code,
                code: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.result.clone())
                    .unwrap_or_else(|| "UNKNOWN_ERROR".to_string()),
                message: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.message.clone())
                    .unwrap_or_else(|| "Capture failed".to_string()),
                reason: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.message.clone()),
                attempt_status: Some(status),
                connector_transaction_id: Some(item.response.id.clone()),
                network_decline_code: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.result.clone()),
                network_advice_code: None,
                network_error_message: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.message.clone()),
            }),
            _ => Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id,
                connector_response_reference_id: item.response.reference.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
        };

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== REFUND FLOW STRUCTURES =====

// Refund Request - Based on tech spec, refunds can be with amount or empty body
// Following Pattern 2 from pattern_refund.md - Amount-Required Refunds
#[derive(Debug, Clone, Serialize)]
pub struct GlobalpayRefundRequest {
    pub amount: StringMinorUnit,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GlobalpayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for GlobalpayRefundRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        wrapper: GlobalpayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &wrapper.router_data;
        Ok(Self {
            amount: GlobalpayAmountConvertor::convert(
                item.request.minor_refund_amount,
                item.request.currency,
            )
            .change_context(errors::ConnectorError::RequestEncodingFailed)?,
        })
    }
}

// Refund Response - Based on tech spec, refund response is similar to transaction response
// The refund endpoint returns a transaction object with status
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlobalpayRefundResponse {
    pub id: String,
    pub status: GlobalpayRefundStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<StringMinorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<common_enums::Currency>,
}

impl TryFrom<ResponseRouterData<GlobalpayRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlobalpayRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let refund_status = RefundStatus::from(item.response.status.clone());

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id.clone(),
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// RSync Response - Reuses the same GlobalpayRefundResponse structure
impl TryFrom<ResponseRouterData<GlobalpayRefundResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlobalpayRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let refund_status = RefundStatus::from(item.response.status.clone());

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id.clone(),
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// ===== VOID FLOW STRUCTURES =====

// Void Request - Based on tech spec, /transactions/{transaction_id}/reverse endpoint
#[derive(Debug, Clone, Serialize)]
pub struct GlobalpayVoidRequest {
    pub amount: Option<StringMinorUnit>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GlobalpayRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for GlobalpayVoidRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        wrapper: GlobalpayRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &wrapper.router_data;
        // Validate that we have a connector transaction ID (required for URL construction)
        if item.request.connector_transaction_id.is_empty() {
            return Err(error_stack::report!(
                errors::ConnectorError::MissingConnectorTransactionID
            ));
        }

        // Convert amount from MinorUnit to StringMinorUnit if present
        let amount = item
            .request
            .amount
            .zip(item.request.currency)
            .map(|(amount_value, currency)| {
                GlobalpayAmountConvertor::convert(amount_value, currency)
                    .change_context(errors::ConnectorError::RequestEncodingFailed)
            })
            .transpose()?;

        Ok(Self { amount })
    }
}

// Void Response - Reuses GlobalpayPaymentsResponse structure
// The response is similar to transaction response with REVERSED status
impl TryFrom<ResponseRouterData<GlobalpayPaymentsResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlobalpayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Map GlobalPay void statuses to UCS AttemptStatus
        // Void flow uses VoidFailed instead of generic Failure for failed void attempts
        let status = match item.response.status.clone() {
            // Success case - void completed
            GlobalpayPaymentStatus::Reversed => AttemptStatus::Voided,
            // Pending cases - void in progress
            GlobalpayPaymentStatus::Pending
            | GlobalpayPaymentStatus::Initiated
            | GlobalpayPaymentStatus::ForReview => AttemptStatus::Pending,
            // Failure cases - void attempt failed or invalid states
            GlobalpayPaymentStatus::Declined
            | GlobalpayPaymentStatus::Failed
            | GlobalpayPaymentStatus::Rejected
            | GlobalpayPaymentStatus::Captured
            | GlobalpayPaymentStatus::Preauthorized
            | GlobalpayPaymentStatus::Funded => AttemptStatus::VoidFailed,
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: item.response.reference.clone(),
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
