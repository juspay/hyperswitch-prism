use crate::{connectors::globalpay::GlobalpayRouterData, types::ResponseRouterData};
use common_enums::{AttemptStatus, RefundStatus};
use common_utils::consts::NO_ERROR_CODE;
use common_utils::request::Method;
use common_utils::types::StringMinorUnit;
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, PSync, RSync, Refund, RepeatPayment,
        ServerAuthenticationToken, SetupMandate, Void,
    },
    connector_types::{
        ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData,
        ConnectorSpecificClientAuthenticationResponse,
        GlobalpayClientAuthenticationResponse as GlobalpayClientAuthenticationResponseDomain,
        MandateReference, MandateReferenceId, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        ResponseId, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
        SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::{
        BankRedirectData, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber,
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
/// Response type for SetupMandate flow. GlobalPay's mandate setup tokenizes the
/// card via the `/payment-methods` endpoint; the `PMT_` id returned here is
/// what we surface as the connector_mandate_id for later MIT charges.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlobalpaySetupMandateResponse {
    pub id: String,
    pub reference: Option<String>,
    pub usage_mode: Option<GlobalpayUsageMode>,
    pub card: Option<GlobalpayTokenizedCard>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlobalpayTokenizedCard {
    pub masked_number_last4: Option<Secret<String>>,
    pub brand: Option<Secret<String>>,
    pub expiry_month: Option<Secret<String>>,
    pub expiry_year: Option<Secret<String>>,
}
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
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Globalpay {
                app_id, app_key, ..
            } => Ok(Self {
                app_id: app_id.to_owned(),
                app_key: app_key.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
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

/// GlobalPay `usage_mode` on /payment-methods. `MULTIPLE` allows the returned
/// PMT_ id to be reused for subsequent MIT charges; `SINGLE` restricts it to
/// a single subsequent transaction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GlobalpayUsageMode {
    Single,
    Multiple,
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
            ServerAuthenticationToken,
            PaymentFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
    > for GlobalpayAccessTokenRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<
            ServerAuthenticationToken,
            PaymentFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
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
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
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
    for RouterDataV2<F, PaymentFlowData, T, ServerAuthenticationTokenResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlobalpayAccessTokenResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(ServerAuthenticationTokenResponseData {
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
    pub entry_mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<GlobalpayCard<T>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apm: Option<GlobalpayApm>,
    /// Connector-issued token reference (e.g. from GlobalPayments.js hosted fields).
    /// When set, GlobalPay looks up the tokenized card by this ID instead of
    /// requiring raw card data in the request body.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
pub struct GlobalpayCard<T: PaymentMethodDataTypes> {
    pub number: RawCardNumber<T>,
    pub expiry_month: Secret<String>,
    pub expiry_year: Secret<String>,
    pub cvv: Secret<String>,
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
    type Error = error_stack::Report<IntegrationError>;

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
                let expiry_year_2digit = card_data.get_card_expiry_year_2_digit().change_context(
                    IntegrationError::RequestEncodingFailed {
                        context: Default::default(),
                    },
                )?;

                GlobalpayPaymentMethod {
                    entry_mode: constants::ENTRY_MODE_ECOM.to_string(),
                    card: Some(GlobalpayCard {
                        number: card_data.card_number.clone(),
                        expiry_month: card_data.card_exp_month.clone(),
                        expiry_year: expiry_year_2digit,
                        cvv: card_data.card_cvc.clone(),
                    }),
                    apm: None,
                    id: None,
                }
            }
            PaymentMethodData::BankRedirect(bank_redirect) => {
                let apm_provider = match bank_redirect {
                    BankRedirectData::Eps { .. } => Some(ApmProvider::Eps),
                    BankRedirectData::Ideal { .. } => Some(ApmProvider::Ideal),
                    _ => {
                        return Err(error_stack::report!(IntegrationError::NotImplemented(
                            "Bank redirect payment method not supported".to_string(),
                            Default::default()
                        )))
                    }
                };

                GlobalpayPaymentMethod {
                    entry_mode: constants::ENTRY_MODE_ECOM.to_string(),
                    card: None,
                    apm: Some(GlobalpayApm {
                        provider: apm_provider,
                    }),
                    id: None,
                }
            }

            PaymentMethodData::PaymentMethodToken(t) => {
                let token = t.token.clone();

                GlobalpayPaymentMethod {
                    entry_mode: constants::ENTRY_MODE_ECOM.to_string(),
                    card: None,
                    apm: None,
                    id: Some(token),
                }
            }
            _ => {
                return Err(error_stack::report!(IntegrationError::NotImplemented(
                    "Payment method not supported".to_string(),
                    Default::default()
                )))
            }
        };

        // Determine capture_mode based on capture_method
        let capture_mode = match item.request.capture_method {
            Some(common_enums::CaptureMethod::Manual) => Some(GlobalpayCaptureMode::Later),
            _ => Some(GlobalpayCaptureMode::Auto),
        };

        // Country is required by GlobalPay - missing billing country is a user error
        let country = item.resource_common_data.get_billing_country()?;

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

        let amount = wrapper
            .connector
            .amount_converter
            .convert(item.request.minor_amount, item.request.currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        Ok(Self {
            account_name: constants::ACCOUNT_NAME.to_string(),
            channel: constants::CHANNEL_CNP.to_string(),
            amount,
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
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: GlobalpayRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &wrapper.router_data;
        let amount = wrapper
            .connector
            .amount_converter
            .convert(item.request.minor_amount_to_capture, item.request.currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        Ok(Self {
            amount,
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
    type Error = error_stack::Report<ConnectorError>;

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
                Url::parse(url).change_context(crate::utils::response_handling_fail_for_connector(
                    item.http_code,
                    "globalpay",
                ))
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
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
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
    type Error = error_stack::Report<ConnectorError>;

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
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
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
    type Error = error_stack::Report<ConnectorError>;

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
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
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
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: GlobalpayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &wrapper.router_data;
        let amount = wrapper
            .connector
            .amount_converter
            .convert(item.request.minor_refund_amount, item.request.currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;
        Ok(Self { amount })
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
    type Error = error_stack::Report<ConnectorError>;

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
    type Error = error_stack::Report<ConnectorError>;

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
    type Error = error_stack::Report<IntegrationError>;

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
                IntegrationError::MissingConnectorTransactionID {
                    context: Default::default()
                }
            ));
        }

        // Convert amount from MinorUnit to StringMinorUnit if present
        let amount = item
            .request
            .amount
            .zip(item.request.currency)
            .map(|(amount_value, currency)| {
                wrapper
                    .connector
                    .amount_converter
                    .convert(amount_value, currency)
                    .change_context(IntegrationError::AmountConversionFailed {
                        context: Default::default(),
                    })
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
    type Error = error_stack::Report<ConnectorError>;

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

// ===== CLIENT AUTHENTICATION TOKEN FLOW STRUCTURES =====

/// Request to obtain an access token for client-side SDK initialization.
/// Uses the same /accesstoken endpoint as the ServerAuthenticationToken flow,
/// but returns the token in a client-auth-specific format.
#[derive(Debug, Serialize)]
pub struct GlobalpayClientAuthRequest {
    pub app_id: Secret<String>,
    pub nonce: Secret<String>,
    pub secret: Secret<String>,
    pub grant_type: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GlobalpayRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                PaymentFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for GlobalpayClientAuthRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: GlobalpayRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                PaymentFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &wrapper.router_data;
        if let ConnectorSpecificConfig::Globalpay {
            app_id, app_key, ..
        } = &item.connector_config
        {
            use sha2::{Digest, Sha512};

            // Generate random alphanumeric nonce
            let nonce =
                rand::distributions::Alphanumeric.sample_string(&mut rand::thread_rng(), 12);

            // Create secret: SHA512(nonce + app_key)
            let secret_input = format!("{}{}", nonce, app_key.peek());

            let mut hasher = Sha512::new();
            hasher.update(secret_input.as_bytes());
            let result = hasher.finalize();
            let secret_hex = hex::encode(result);

            Ok(Self {
                app_id: app_id.clone(),
                nonce: Secret::new(nonce),
                secret: Secret::new(secret_hex),
                grant_type: "client_credentials".to_string(),
            })
        } else {
            Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
            ))
        }
    }
}

/// Response from the /accesstoken endpoint for client-side SDK use.
#[derive(Debug, Deserialize, Serialize)]
pub struct GlobalpayClientAuthResponse {
    pub token: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub seconds_to_expire: i64,
}

impl TryFrom<ResponseRouterData<GlobalpayClientAuthResponse, Self>>
    for RouterDataV2<
        ClientAuthenticationToken,
        PaymentFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlobalpayClientAuthResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        let session_data = ClientAuthenticationTokenData::ConnectorSpecific(Box::new(
            ConnectorSpecificClientAuthenticationResponse::Globalpay(
                GlobalpayClientAuthenticationResponseDomain {
                    access_token: Secret::new(response.token),
                    token_type: Some(response.type_),
                    expires_in: Some(response.seconds_to_expire),
                },
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

// ===== SETUP MANDATE FLOW STRUCTURES =====
//
// GlobalPay requires a tokenized payment method (PMT_...) to drive MIT charges
// against the /transactions endpoint (the `payment_method.id` field only
// accepts tokens, never transaction ids). SetupRecurring therefore calls the
// /payment-methods endpoint to tokenize the card with `usage_mode: MULTIPLE`
// and surfaces the returned `PMT_...` id as the connector_mandate_id that
// RepeatPayment later plugs into `payment_method.id`.

/// Initiator enum for SetupMandate - GlobalPay expects a simple string
/// value here, not a nested object (which is what the existing `Initiator`
/// struct serializes to). See HyperSwitch's globalpay reference impl.
#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GlobalpayMandateInitiator {
    Merchant,
    Payer,
}

/// Stored credential model for setup mandate. GlobalPay expects `model`
/// (not `type`) as the field name and only the `model`/`sequence` pair.
#[derive(Debug, Serialize)]
pub struct GlobalpayMandateStoredCredential {
    pub model: GlobalpayStoredCredentialModel,
    pub sequence: StoredCredentialSequence,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GlobalpayStoredCredentialModel {
    Recurring,
    Subscription,
    Unscheduled,
    Installment,
}

#[derive(Debug, Serialize)]
pub struct GlobalpaySetupMandateCard<T: PaymentMethodDataTypes> {
    pub number: RawCardNumber<T>,
    pub expiry_month: Secret<String>,
    pub expiry_year: Secret<String>,
    pub cvv: Secret<String>,
}

/// Tokenization request sent to `/payment-methods`. `usage_mode: MULTIPLE`
/// allows the returned PMT_ id to be reused by subsequent MIT charges.
#[derive(Debug, Serialize)]
pub struct GlobalpaySetupMandateRequest<T: PaymentMethodDataTypes> {
    pub reference: String,
    pub usage_mode: GlobalpayUsageMode,
    pub card: GlobalpaySetupMandateCard<T>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GlobalpayRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for GlobalpaySetupMandateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: GlobalpayRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &wrapper.router_data;

        let card = match &item.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                let expiry_year_2digit = card_data.get_card_expiry_year_2_digit().change_context(
                    IntegrationError::RequestEncodingFailed {
                        context: Default::default(),
                    },
                )?;
                // GlobalPay's /payment-methods endpoint requires CVV for card
                // tokenization; unlike the Authorize flow there is no
                // `cvv_indicator` fallback, so an empty CVV would surface as an
                // opaque connector-side rejection. Fail fast instead.
                if card_data.card_cvc.peek().is_empty() {
                    return Err(error_stack::report!(
                        IntegrationError::MissingRequiredField {
                            field_name: "card_cvc",
                            context: Default::default(),
                        }
                    ));
                }
                GlobalpaySetupMandateCard {
                    number: card_data.card_number.clone(),
                    expiry_month: card_data.card_exp_month.clone(),
                    expiry_year: expiry_year_2digit,
                    cvv: card_data.card_cvc.clone(),
                }
            }
            _ => {
                return Err(error_stack::report!(IntegrationError::NotImplemented(
                    "Payment method not supported for SetupMandate".to_string(),
                    Default::default()
                )))
            }
        };

        Ok(Self {
            reference: item
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            usage_mode: GlobalpayUsageMode::Multiple,
            card,
        })
    }
}

// SetupMandate response: a 2xx from /payment-methods returns a PMT_ id which
// becomes the connector_mandate_id used later by RepeatPayment (as
// `payment_method.id` on /transactions). GlobalPay's /payment-methods response
// has no status field - a successful parse implies tokenization succeeded.
impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<GlobalpaySetupMandateResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlobalpaySetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // The PMT_ id is a payment-method token, not a transaction id, so PSync
        // (which hits /transactions/{id}) cannot be performed against it. We
        // surface the PMT_ id through MandateReference.connector_mandate_id for
        // later RepeatPayment use and leave resource_id as NoResponseId.
        let mandate_reference = Some(Box::new(MandateReference {
            connector_mandate_id: Some(item.response.id.clone()),
            payment_method_id: None,
            connector_mandate_request_reference_id: None,
        }));

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::NoResponseId,
                redirection_data: None,
                mandate_reference,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: item.response.reference.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::Charged,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== REPEAT PAYMENT (MIT) FLOW STRUCTURES =====
//
// GlobalPay MIT charges reuse the `/transactions` endpoint. The stored credential
// is referenced by putting the connector_mandate_id (transaction id from the prior
// SetupMandate) into `payment_method.id`, with initiator=MERCHANT and
// stored_credential={ model: RECURRING, sequence: SUBSEQUENT }.

/// Response type for RepeatPayment flow - reuses GlobalpayPaymentsResponse
pub type GlobalpayRepeatPaymentResponse = GlobalpayPaymentsResponse;

/// Payment method body for MIT - references the stored transaction by id.
#[derive(Debug, Serialize)]
pub struct GlobalpayRepeatPaymentMethod {
    pub entry_mode: String,
    pub id: String,
}

#[derive(Debug, Serialize)]
pub struct GlobalpayRepeatPaymentRequest {
    pub account_name: String,
    pub channel: String,
    pub amount: StringMinorUnit,
    pub currency: common_enums::Currency,
    pub reference: String,
    pub country: common_enums::CountryAlpha2,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_mode: Option<GlobalpayCaptureMode>,
    pub initiator: GlobalpayMandateInitiator,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notifications: Option<GlobalpayNotifications>,
    pub stored_credential: GlobalpayMandateStoredCredential,
    pub payment_method: GlobalpayRepeatPaymentMethod,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GlobalpayRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for GlobalpayRepeatPaymentRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: GlobalpayRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &wrapper.router_data;

        let mandate_id = match &item.request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(connector_mandate_ref) => connector_mandate_ref
                .get_connector_mandate_id()
                .ok_or_else(|| {
                    error_stack::report!(IntegrationError::MissingRequiredField {
                        field_name: "connector_mandate_id",
                        context: Default::default()
                    })
                })?,
            MandateReferenceId::NetworkMandateId(_)
            | MandateReferenceId::NetworkTokenWithNTI(_) => {
                return Err(error_stack::report!(IntegrationError::NotImplemented(
                    "Network mandate id not supported for GlobalPay RepeatPayment".to_string(),
                    Default::default()
                )));
            }
        };

        let country = item.resource_common_data.get_billing_country()?;

        let notifications = if let Some(webhook_url) = item.request.webhook_url.as_ref() {
            let return_url = item
                .request
                .router_return_url
                .clone()
                .unwrap_or_else(|| webhook_url.clone());
            Some(GlobalpayNotifications {
                cancel_url: return_url.clone(),
                return_url,
                status_url: webhook_url.clone(),
            })
        } else {
            None
        };

        let capture_mode = match item.request.capture_method {
            Some(common_enums::CaptureMethod::Manual) => Some(GlobalpayCaptureMode::Later),
            _ => Some(GlobalpayCaptureMode::Auto),
        };

        let amount = wrapper
            .connector
            .amount_converter
            .convert(item.request.minor_amount, item.request.currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        Ok(Self {
            account_name: constants::ACCOUNT_NAME.to_string(),
            channel: constants::CHANNEL_CNP.to_string(),
            amount,
            currency: item.request.currency,
            reference: item
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            country,
            capture_mode,
            initiator: GlobalpayMandateInitiator::Merchant,
            notifications,
            stored_credential: GlobalpayMandateStoredCredential {
                model: GlobalpayStoredCredentialModel::Recurring,
                sequence: StoredCredentialSequence::Subsequent,
            },
            payment_method: GlobalpayRepeatPaymentMethod {
                entry_mode: constants::ENTRY_MODE_ECOM.to_string(),
                id: mandate_id,
            },
        })
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<GlobalpayRepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlobalpayRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::from(item.response.status.clone());

        let network_txn_id = item
            .response
            .payment_method
            .as_ref()
            .and_then(|pm| pm.card.as_ref())
            .and_then(|card| card.brand_reference.as_ref())
            .map(|s| s.peek().to_string());

        let response = match status {
            AttemptStatus::Failure => Err(ErrorResponse {
                status_code: item.http_code,
                code: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.result.clone())
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.message.clone())
                    .unwrap_or_else(|| "Repeat payment failed".to_string()),
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
