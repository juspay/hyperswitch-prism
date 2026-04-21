use crate::utils::{self, ErrorCodeAndMessage};
use crate::{
    connectors,
    connectors::trustpay::{TrustpayAmountConvertor, TrustpayRouterData},
    types::ResponseRouterData,
};
use common_enums::enums;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::CustomResult,
    pii,
    request::Method,
    types::{FloatMajorUnit, MinorUnit, StringMajorUnit},
    Email,
};
use domain_types::{
    connector_flow::{
        Authorize, CreateOrder, Refund, RepeatPayment, ServerAuthenticationToken, SetupMandate,
    },
    connector_types::{
        AmountInfo, ApplePayPaymentRequest, ApplePaySessionResponse,
        ApplepayClientAuthenticationResponse, ClientAuthenticationTokenData,
        GooglePaySessionResponse, GpayAllowedPaymentMethods, GpayClientAuthenticationResponse,
        GpayMerchantInfo, GpayShippingAddressParameters, MandateReference, MandateReferenceId,
        NextActionCall, PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData,
        PaymentsAuthorizeData, PaymentsResponseData, RefundFlowData, RefundsData,
        RefundsResponseData, RepeatPaymentData, ResponseId, SdkNextAction, SecretInfoToInitiateSdk,
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
        SetupMandateRequestData, ThirdPartySdkSessionResponse,
    },
    errors::{ConnectorError, IntegrationError, WebhookError},
    payment_method_data::{
        BankRedirectData, BankTransferData, Card, PaymentMethodData, PaymentMethodDataTypes,
        RawCardNumber,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_request_types::BrowserInformation,
    router_response_types::RedirectForm,
};
use error_stack::{report, ResultExt};
use hyperswitch_masking::{PeekInterface, Secret};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

type Error = error_stack::Report<IntegrationError>;

#[allow(dead_code)]
pub struct TrustpayAuthType {
    pub(super) api_key: Secret<String>,
    pub(super) project_id: Secret<String>,
    pub(super) secret_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for TrustpayAuthType {
    type Error = Error;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        if let ConnectorSpecificConfig::Trustpay {
            api_key,
            project_id,
            secret_key,
            ..
        } = auth_type
        {
            Ok(Self {
                api_key: api_key.to_owned(),
                project_id: project_id.to_owned(),
                secret_key: secret_key.to_owned(),
            })
        } else {
            Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into())
        }
    }
}

const CLIENT_CREDENTIAL: &str = "client_credentials";
const CHALLENGE_WINDOW: &str = "1";
const PAYMENT_TYPE: &str = "Plain";
const PAYMENT_TYPE_RECURRING_INITIAL: &str = "RecurringInitial";
const PAYMENT_TYPE_RECURRING: &str = "RecurringSubsequent";
const STATUS: char = 'Y';

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum TrustpayPaymentMethod {
    #[serde(rename = "EPS")]
    Eps,
    Giropay,
    IDeal,
    Sofort,
    Blik,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum TrustpayBankTransferPaymentMethod {
    SepaCreditTransfer,
    #[serde(rename = "Wire")]
    InstantBankTransfer,
    InstantBankTransferFI,
    InstantBankTransferPL,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct MerchantIdentification {
    pub project_id: Secret<String>,
}

#[derive(Default, Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct References {
    pub merchant_reference: String,
}

#[derive(Default, Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct Amount {
    pub amount: StringMajorUnit,
    pub currency: String,
}

#[derive(Default, Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct Reason {
    pub code: Option<String>,
    pub reject_reason: Option<String>,
}

#[derive(Default, Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct StatusReasonInformation {
    pub reason: Reason,
}

#[derive(Default, Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct DebtorInformation {
    pub name: Secret<String>,
    pub email: Email,
}

#[derive(Default, Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct BankPaymentInformation {
    pub amount: Amount,
    pub references: References,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debtor: Option<DebtorInformation>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct BankPaymentInformationResponse {
    pub status: TrustpayBankRedirectPaymentStatus,
    pub status_reason_information: Option<StatusReasonInformation>,
    pub references: ReferencesResponse,
    pub amount: WebhookAmount,
}

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub struct CallbackURLs {
    pub success: String,
    pub cancel: String,
    pub error: String,
}

impl TryFrom<&BankRedirectData> for TrustpayPaymentMethod {
    type Error = Error;
    fn try_from(value: &BankRedirectData) -> Result<Self, Self::Error> {
        match value {
            BankRedirectData::Giropay { .. } => Ok(Self::Giropay),
            BankRedirectData::Eps { .. } => Ok(Self::Eps),
            BankRedirectData::Ideal { .. } => Ok(Self::IDeal),
            BankRedirectData::Sofort { .. } => Ok(Self::Sofort),
            BankRedirectData::Blik { .. } => Ok(Self::Blik),
            BankRedirectData::BancontactCard { .. }
            | BankRedirectData::Bizum {}
            | BankRedirectData::Eft { .. }
            | BankRedirectData::Interac { .. }
            | BankRedirectData::OnlineBankingCzechRepublic { .. }
            | BankRedirectData::OnlineBankingFinland { .. }
            | BankRedirectData::OnlineBankingPoland { .. }
            | BankRedirectData::OnlineBankingSlovakia { .. }
            | BankRedirectData::OpenBankingUk { .. }
            | BankRedirectData::Przelewy24 { .. }
            | BankRedirectData::Trustly { .. }
            | BankRedirectData::OnlineBankingFpx { .. }
            | BankRedirectData::OnlineBankingThailand { .. }
            | BankRedirectData::LocalBankRedirect {}
            | BankRedirectData::OpenBanking {}
            | BankRedirectData::Netbanking { .. } => Err(IntegrationError::NotImplemented(
                (utils::get_unimplemented_payment_method_error_message("trustpay")).into(),
                Default::default(),
            )
            .into()),
        }
    }
}

impl TryFrom<&BankTransferData> for TrustpayBankTransferPaymentMethod {
    type Error = Error;
    fn try_from(value: &BankTransferData) -> Result<Self, Self::Error> {
        match value {
            BankTransferData::SepaBankTransfer { .. } => Ok(Self::SepaCreditTransfer),
            BankTransferData::InstantBankTransfer {} => Ok(Self::InstantBankTransfer),
            BankTransferData::InstantBankTransferFinland {} => Ok(Self::InstantBankTransferFI),
            BankTransferData::InstantBankTransferPoland {} => Ok(Self::InstantBankTransferPL),
            _ => Err(error_stack::report!(IntegrationError::NotSupported {
                message: utils::get_unimplemented_payment_method_error_message("trustpay"),
                connector: "trustpay",
                context: Default::default(),
            })),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustpayPaymentStatusCode {
    // CVV and card validation errors
    EmptyCvvNotAllowed,

    // Authentication and session errors
    SessionRejected,
    UserAuthenticationFailed,
    RiskManagementTimeout,
    PaResValidationFailed,
    ThreeDSecureSystemError,
    DirectoryServerError,
    ThreeDSystemError,
    AuthenticationInvalidFormat,
    AuthenticationSuspectedFraud,

    // Input and parameter errors
    InvalidInputData,
    AmountOutsideBoundaries,
    InvalidOrMissingParameter,

    // Transaction decline reasons
    AdditionalAuthRequired,
    CardNotEnrolledIn3DS,
    AuthenticationError,
    TransactionDeclinedAuth,
    InvalidTransaction1,
    InvalidTransaction2,
    NoDescription,

    // Refund errors
    CannotRefund,
    TooManyTransactions,
    TestAccountsNotAllowed,

    // General decline reasons
    DeclinedUnknownReason,
    DeclinedInvalidCard,
    DeclinedByAuthSystem,
    DeclinedInvalidCvv,
    DeclinedExceedsCredit,
    DeclinedWrongExpiry,
    DeclinedSuspectingManipulation,
    DeclinedCardBlocked,
    DeclinedLimitExceeded,
    DeclinedFrequencyExceeded,
    DeclinedCardLost,
    DeclinedRestrictedCard,
    DeclinedNotPermitted,
    DeclinedPickUpCard,
    DeclinedAccountBlocked,
    DeclinedInvalidConfig,
    AccountClosed,
    InsufficientFunds,
    RejectedByThrottling,
    CountryBlacklisted,
    BinBlacklisted,
    SessionBeingProcessed,

    // Communication errors
    CommunicationError,
    TimeoutUncertainResult,

    // Success or other status
    Unknown,
}

impl TrustpayPaymentStatusCode {
    pub fn error_message(&self) -> &'static str {
        match self {
            Self::EmptyCvvNotAllowed => "Empty CVV for VISA, MASTER not allowed",
            Self::SessionRejected => "Referenced session is rejected (no action possible)",
            Self::UserAuthenticationFailed => "User authentication failed",
            Self::RiskManagementTimeout => "Risk management transaction timeout",
            Self::PaResValidationFailed => "PARes validation failed - problem with signature",
            Self::ThreeDSecureSystemError => "Transaction rejected because of technical error in 3DSecure system",
            Self::DirectoryServerError => "Communication error to VISA/Mastercard Directory Server",
            Self::ThreeDSystemError => "Technical error in 3D system",
            Self::AuthenticationInvalidFormat => "Authentication failed due to invalid message format",
            Self::AuthenticationSuspectedFraud => "Authentication failed due to suspected fraud",
            Self::InvalidInputData => "Invalid input data",
            Self::AmountOutsideBoundaries => "Amount is outside allowed ticket size boundaries",
            Self::InvalidOrMissingParameter => "Invalid or missing parameter",
            Self::AdditionalAuthRequired => "Transaction declined (additional customer authentication required)",
            Self::CardNotEnrolledIn3DS => "Card not enrolled in 3DS",
            Self::AuthenticationError => "Authentication error",
            Self::TransactionDeclinedAuth => "Transaction declined (auth. declined)",
            Self::InvalidTransaction1 => "Invalid transaction",
            Self::InvalidTransaction2 => "Invalid transaction",
            Self::NoDescription => "No description available.",
            Self::CannotRefund => "Cannot refund (refund volume exceeded or tx reversed or invalid workflow)",
            Self::TooManyTransactions => "Referenced session contains too many transactions",
            Self::TestAccountsNotAllowed => "Test accounts not allowed in production",
            Self::DeclinedUnknownReason => "Transaction declined for unknown reason",
            Self::DeclinedInvalidCard => "Transaction declined (invalid card)",
            Self::DeclinedByAuthSystem => "Transaction declined by authorization system",
            Self::DeclinedInvalidCvv => "Transaction declined (invalid CVV)",
            Self::DeclinedExceedsCredit => "Transaction declined (amount exceeds credit)",
            Self::DeclinedWrongExpiry => "Transaction declined (wrong expiry date)",
            Self::DeclinedSuspectingManipulation => "transaction declined (suspecting manipulation)",
            Self::DeclinedCardBlocked => "transaction declined (card blocked)",
            Self::DeclinedLimitExceeded => "Transaction declined (limit exceeded)",
            Self::DeclinedFrequencyExceeded => "Transaction declined (maximum transaction frequency exceeded)",
            Self::DeclinedCardLost => "Transaction declined (card lost)",
            Self::DeclinedRestrictedCard => "Transaction declined (restricted card)",
            Self::DeclinedNotPermitted => "Transaction declined (transaction not permitted)",
            Self::DeclinedPickUpCard => "transaction declined (pick up card)",
            Self::DeclinedAccountBlocked => "Transaction declined (account blocked)",
            Self::DeclinedInvalidConfig => "Transaction declined (invalid configuration data)",
            Self::AccountClosed => "Account Closed",
            Self::InsufficientFunds => "Insufficient Funds",
            Self::RejectedByThrottling => "Rejected by throttling",
            Self::CountryBlacklisted => "Country blacklisted",
            Self::BinBlacklisted => "Bin blacklisted",
            Self::SessionBeingProcessed => "Transaction for the same session is currently being processed, please try again later",
            Self::CommunicationError => "Unexpected communication error with connector/acquirer",
            Self::TimeoutUncertainResult => "Timeout, uncertain result",
            Self::Unknown => ""
}
    }

    pub fn is_failure(&self) -> bool {
        !matches!(self, Self::Unknown)
    }
}

impl From<&str> for TrustpayPaymentStatusCode {
    fn from(status_code: &str) -> Self {
        match status_code {
            "100.100.600" => Self::EmptyCvvNotAllowed,
            "100.350.100" => Self::SessionRejected,
            "100.380.401" => Self::UserAuthenticationFailed,
            "100.380.501" => Self::RiskManagementTimeout,
            "100.390.103" => Self::PaResValidationFailed,
            "100.390.105" => Self::ThreeDSecureSystemError,
            "100.390.111" => Self::DirectoryServerError,
            "100.390.112" => Self::ThreeDSystemError,
            "100.390.115" => Self::AuthenticationInvalidFormat,
            "100.390.118" => Self::AuthenticationSuspectedFraud,
            "100.400.304" => Self::InvalidInputData,
            "100.550.312" => Self::AmountOutsideBoundaries,
            "200.300.404" => Self::InvalidOrMissingParameter,
            "300.100.100" => Self::AdditionalAuthRequired,
            "400.001.301" => Self::CardNotEnrolledIn3DS,
            "400.001.600" => Self::AuthenticationError,
            "400.001.601" => Self::TransactionDeclinedAuth,
            "400.001.602" => Self::InvalidTransaction1,
            "400.001.603" => Self::InvalidTransaction2,
            "400.003.600" => Self::NoDescription,
            "700.400.200" => Self::CannotRefund,
            "700.500.001" => Self::TooManyTransactions,
            "700.500.003" => Self::TestAccountsNotAllowed,
            "800.100.100" => Self::DeclinedUnknownReason,
            "800.100.151" => Self::DeclinedInvalidCard,
            "800.100.152" => Self::DeclinedByAuthSystem,
            "800.100.153" => Self::DeclinedInvalidCvv,
            "800.100.155" => Self::DeclinedExceedsCredit,
            "800.100.157" => Self::DeclinedWrongExpiry,
            "800.100.158" => Self::DeclinedSuspectingManipulation,
            "800.100.160" => Self::DeclinedCardBlocked,
            "800.100.162" => Self::DeclinedLimitExceeded,
            "800.100.163" => Self::DeclinedFrequencyExceeded,
            "800.100.165" => Self::DeclinedCardLost,
            "800.100.168" => Self::DeclinedRestrictedCard,
            "800.100.170" => Self::DeclinedNotPermitted,
            "800.100.171" => Self::DeclinedPickUpCard,
            "800.100.172" => Self::DeclinedAccountBlocked,
            "800.100.190" => Self::DeclinedInvalidConfig,
            "800.100.202" => Self::AccountClosed,
            "800.100.203" => Self::InsufficientFunds,
            "800.120.100" => Self::RejectedByThrottling,
            "800.300.102" => Self::CountryBlacklisted,
            "800.300.401" => Self::BinBlacklisted,
            "800.700.100" => Self::SessionBeingProcessed,
            "900.100.100" => Self::CommunicationError,
            "900.100.300" => Self::TimeoutUncertainResult,
            _ => Self::Unknown,
        }
    }
}

fn is_payment_failed(payment_status: &str) -> (bool, &'static str) {
    let status_code = TrustpayPaymentStatusCode::from(payment_status);
    (status_code.is_failure(), status_code.error_message())
}

/// Returns whether the connector status string indicates a successful charge.
/// (Always succeeds; status classification is pure logic.)
fn is_payment_successful(payment_status: &str) -> bool {
    match payment_status {
        "000.400.100" => true,
        _ => {
            let allowed_prefixes = [
                "000.000.",
                "000.100.1",
                "000.3",
                "000.6",
                "000.400.01",
                "000.400.02",
                "000.400.04",
                "000.400.05",
                "000.400.06",
                "000.400.07",
                "000.400.08",
                "000.400.09",
            ];
            allowed_prefixes
                .iter()
                .any(|&prefix| payment_status.starts_with(prefix))
        }
    }
}

fn get_pending_status_based_on_redirect_url(redirect_url: Option<Url>) -> enums::AttemptStatus {
    match redirect_url {
        Some(_url) => enums::AttemptStatus::AuthenticationPending,
        None => enums::AttemptStatus::Pending,
    }
}

fn get_transaction_status(
    payment_status: Option<String>,
    redirect_url: Option<Url>,
) -> CustomResult<(enums::AttemptStatus, Option<String>), ConnectorError> {
    // We don't get payment_status only in case, when the user doesn't complete the authentication step.
    // If we receive status, then return the proper status based on the connector response
    if let Some(payment_status) = payment_status {
        let (is_failed, failure_message) = is_payment_failed(&payment_status);
        if is_failed {
            Ok((
                enums::AttemptStatus::Failure,
                Some(failure_message.to_string()),
            ))
        } else if is_payment_successful(&payment_status) {
            Ok((enums::AttemptStatus::Charged, None))
        } else {
            let pending_status = get_pending_status_based_on_redirect_url(redirect_url);
            Ok((pending_status, None))
        }
    } else {
        Ok((enums::AttemptStatus::AuthenticationPending, None))
    }
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub enum TrustpayBankRedirectPaymentStatus {
    Paid,
    Authorized,
    Rejected,
    Authorizing,
    Pending,
}

impl From<TrustpayBankRedirectPaymentStatus> for enums::AttemptStatus {
    fn from(item: TrustpayBankRedirectPaymentStatus) -> Self {
        match item {
            TrustpayBankRedirectPaymentStatus::Paid => Self::Charged,
            TrustpayBankRedirectPaymentStatus::Rejected => Self::AuthorizationFailed,
            TrustpayBankRedirectPaymentStatus::Authorized => Self::Authorized,
            TrustpayBankRedirectPaymentStatus::Authorizing => Self::Authorizing,
            TrustpayBankRedirectPaymentStatus::Pending => Self::Authorizing,
        }
    }
}

impl From<TrustpayBankRedirectPaymentStatus> for enums::RefundStatus {
    fn from(item: TrustpayBankRedirectPaymentStatus) -> Self {
        match item {
            TrustpayBankRedirectPaymentStatus::Paid => Self::Success,
            TrustpayBankRedirectPaymentStatus::Rejected => Self::Failure,
            _ => Self::Pending,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaymentsResponseCards {
    pub status: i64,
    pub description: Option<String>,
    pub instance_id: String,
    pub payment_status: Option<String>,
    pub payment_description: Option<String>,
    pub redirect_url: Option<Url>,
    pub redirect_params: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct PaymentsResponseBankRedirect {
    pub payment_request_id: i64,
    pub gateway_url: Url,
    pub payment_result_info: Option<ResultInfo>,
    pub payment_method_response: Option<TrustpayPaymentMethod>,
    pub merchant_identification_response: Option<MerchantIdentification>,
    pub payment_information_response: Option<BankPaymentInformationResponse>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct ErrorResponseBankRedirect {
    #[serde(rename = "ResultInfo")]
    pub payment_result_info: ResultInfo,
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct ReferencesResponse {
    pub payment_request_id: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct SyncResponseBankRedirect {
    pub payment_information: BankPaymentInformationResponse,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum TrustpayPaymentsResponse {
    CardsPayments(Box<PaymentsResponseCards>),
    BankRedirectPayments(Box<PaymentsResponseBankRedirect>),
    BankRedirectSync(Box<SyncResponseBankRedirect>),
    BankRedirectError(Box<ErrorResponseBankRedirect>),
    WebhookResponse(Box<WebhookPaymentInformation>),
}

impl<F, T> TryFrom<ResponseRouterData<TrustpayPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<TrustpayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let (status, error, payment_response_data) = get_trustpay_response(
            item.response,
            item.http_code,
            item.router_data.resource_common_data.status,
        )?;
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response: error.map_or_else(|| Ok(payment_response_data), Err),
            ..item.router_data
        })
    }
}

fn handle_cards_response(
    response: PaymentsResponseCards,
    status_code: u16,
) -> CustomResult<
    (
        enums::AttemptStatus,
        Option<ErrorResponse>,
        PaymentsResponseData,
    ),
    ConnectorError,
> {
    let (status, message) = get_transaction_status(
        response.payment_status.to_owned(),
        response.redirect_url.to_owned(),
    )?;

    let form_fields = response.redirect_params.unwrap_or_default();
    let redirection_data = response.redirect_url.map(|url| RedirectForm::Form {
        endpoint: url.to_string(),
        method: Method::Post,
        form_fields,
    });
    let error = if message.is_some() {
        Some(ErrorResponse {
            code: response
                .payment_status
                .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            message: message
                .clone()
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            reason: message,
            status_code,
            attempt_status: None,
            connector_transaction_id: Some(response.instance_id.clone()),
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        })
    } else {
        None
    };
    let payment_response_data = PaymentsResponseData::TransactionResponse {
        resource_id: ResponseId::ConnectorTransactionId(response.instance_id.clone()),
        redirection_data: redirection_data.map(Box::new),
        mandate_reference: None,
        connector_metadata: None,
        network_txn_id: None,
        connector_response_reference_id: None,
        incremental_authorization_allowed: None,
        status_code,
    };
    Ok((status, error, payment_response_data))
}

fn handle_bank_redirects_response(
    response: PaymentsResponseBankRedirect,
    status_code: u16,
) -> CustomResult<
    (
        enums::AttemptStatus,
        Option<ErrorResponse>,
        PaymentsResponseData,
    ),
    ConnectorError,
> {
    let status = enums::AttemptStatus::AuthenticationPending;
    let error = None;
    let payment_response_data = PaymentsResponseData::TransactionResponse {
        resource_id: ResponseId::ConnectorTransactionId(response.payment_request_id.to_string()),
        redirection_data: Some(Box::new(RedirectForm::from((
            response.gateway_url,
            Method::Get,
        )))),
        mandate_reference: None,
        connector_metadata: None,
        network_txn_id: None,
        connector_response_reference_id: None,
        incremental_authorization_allowed: None,
        status_code,
    };
    Ok((status, error, payment_response_data))
}

fn handle_bank_redirects_error_response(
    response: ErrorResponseBankRedirect,
    status_code: u16,
    previous_attempt_status: enums::AttemptStatus,
) -> CustomResult<
    (
        enums::AttemptStatus,
        Option<ErrorResponse>,
        PaymentsResponseData,
    ),
    ConnectorError,
> {
    let status = if matches!(response.payment_result_info.result_code, 1132014 | 1132005) {
        previous_attempt_status
    } else {
        enums::AttemptStatus::AuthorizationFailed
    };
    let error = Some(ErrorResponse {
        code: response.payment_result_info.result_code.to_string(),
        message: response
            .payment_result_info
            .additional_info
            .clone()
            .unwrap_or(NO_ERROR_MESSAGE.to_string()),
        reason: response.payment_result_info.additional_info,
        status_code,
        attempt_status: Some(status),
        connector_transaction_id: None,
        network_advice_code: None,
        network_decline_code: None,
        network_error_message: None,
    });
    let payment_response_data = PaymentsResponseData::TransactionResponse {
        resource_id: ResponseId::NoResponseId,
        redirection_data: None,
        mandate_reference: None,
        connector_metadata: None,
        network_txn_id: None,
        connector_response_reference_id: None,
        incremental_authorization_allowed: None,
        status_code,
    };
    Ok((status, error, payment_response_data))
}

fn handle_bank_redirects_sync_response(
    response: SyncResponseBankRedirect,
    status_code: u16,
) -> CustomResult<
    (
        enums::AttemptStatus,
        Option<ErrorResponse>,
        PaymentsResponseData,
    ),
    ConnectorError,
> {
    let status = enums::AttemptStatus::from(response.payment_information.status);
    let error = if domain_types::utils::is_payment_failure(status) {
        let reason_info = response
            .payment_information
            .status_reason_information
            .unwrap_or_default();
        Some(ErrorResponse {
            code: reason_info
                .reason
                .code
                .clone()
                .unwrap_or(NO_ERROR_CODE.to_string()),
            message: reason_info
                .reason
                .reject_reason
                .clone()
                .unwrap_or(NO_ERROR_MESSAGE.to_string()),
            reason: reason_info.reason.reject_reason,
            status_code,
            attempt_status: None,
            connector_transaction_id: Some(
                response
                    .payment_information
                    .references
                    .payment_request_id
                    .clone(),
            ),
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        })
    } else {
        None
    };
    let payment_response_data = PaymentsResponseData::TransactionResponse {
        resource_id: ResponseId::ConnectorTransactionId(
            response
                .payment_information
                .references
                .payment_request_id
                .clone(),
        ),
        redirection_data: None,
        mandate_reference: None,
        connector_metadata: None,
        network_txn_id: None,
        connector_response_reference_id: None,
        incremental_authorization_allowed: None,
        status_code,
    };
    Ok((status, error, payment_response_data))
}

pub fn handle_webhook_response(
    payment_information: WebhookPaymentInformation,
    status_code: u16,
) -> CustomResult<
    (
        enums::AttemptStatus,
        Option<ErrorResponse>,
        PaymentsResponseData,
    ),
    ConnectorError,
> {
    let status = enums::AttemptStatus::try_from(payment_information.status)?;
    let error = if domain_types::utils::is_payment_failure(status) {
        let reason_info = payment_information
            .status_reason_information
            .unwrap_or_default();
        Some(ErrorResponse {
            code: reason_info
                .reason
                .code
                .clone()
                .unwrap_or(NO_ERROR_CODE.to_string()),
            message: reason_info
                .reason
                .reject_reason
                .clone()
                .unwrap_or(NO_ERROR_MESSAGE.to_string()),
            reason: reason_info.reason.reject_reason,
            status_code,
            attempt_status: None,
            connector_transaction_id: payment_information.references.payment_request_id.clone(),
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        })
    } else {
        None
    };
    let payment_response_data = PaymentsResponseData::TransactionResponse {
        resource_id: ResponseId::NoResponseId,
        redirection_data: None,
        mandate_reference: None,
        connector_metadata: None,
        network_txn_id: None,
        connector_response_reference_id: None,
        incremental_authorization_allowed: None,
        status_code,
    };
    Ok((status, error, payment_response_data))
}

/// Same as [`handle_webhook_response`], but for incoming webhook handling that reports
/// [`IntegrationError`].
pub fn handle_webhook_response_incoming_webhook(
    payment_information: WebhookPaymentInformation,
    status_code: u16,
) -> CustomResult<
    (
        enums::AttemptStatus,
        Option<ErrorResponse>,
        PaymentsResponseData,
    ),
    WebhookError,
> {
    let status = match payment_information.status {
        WebhookStatus::Paid => enums::AttemptStatus::Charged,
        WebhookStatus::Rejected => enums::AttemptStatus::AuthorizationFailed,
        WebhookStatus::Refunded | WebhookStatus::Chargebacked | WebhookStatus::Unknown => {
            return Err(report!(WebhookError::WebhookProcessingFailed));
        }
    };
    let error = if domain_types::utils::is_payment_failure(status) {
        let reason_info = payment_information
            .status_reason_information
            .unwrap_or_default();
        Some(ErrorResponse {
            code: reason_info
                .reason
                .code
                .clone()
                .unwrap_or(NO_ERROR_CODE.to_string()),
            message: reason_info
                .reason
                .reject_reason
                .clone()
                .unwrap_or(NO_ERROR_MESSAGE.to_string()),
            reason: reason_info.reason.reject_reason,
            status_code,
            attempt_status: None,
            connector_transaction_id: payment_information.references.payment_request_id.clone(),
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        })
    } else {
        None
    };
    let payment_response_data = PaymentsResponseData::TransactionResponse {
        resource_id: ResponseId::NoResponseId,
        redirection_data: None,
        mandate_reference: None,
        connector_metadata: None,
        network_txn_id: None,
        connector_response_reference_id: None,
        incremental_authorization_allowed: None,
        status_code,
    };
    Ok((status, error, payment_response_data))
}

pub fn get_trustpay_response(
    response: TrustpayPaymentsResponse,
    status_code: u16,
    previous_attempt_status: enums::AttemptStatus,
) -> CustomResult<
    (
        enums::AttemptStatus,
        Option<ErrorResponse>,
        PaymentsResponseData,
    ),
    ConnectorError,
> {
    match response {
        TrustpayPaymentsResponse::CardsPayments(response) => {
            handle_cards_response(*response, status_code)
        }
        TrustpayPaymentsResponse::BankRedirectPayments(response) => {
            handle_bank_redirects_response(*response, status_code)
        }
        TrustpayPaymentsResponse::BankRedirectSync(response) => {
            handle_bank_redirects_sync_response(*response, status_code)
        }
        TrustpayPaymentsResponse::BankRedirectError(response) => {
            handle_bank_redirects_error_response(*response, status_code, previous_attempt_status)
        }
        TrustpayPaymentsResponse::WebhookResponse(response) => {
            handle_webhook_response(*response, status_code)
        }
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct ResultInfo {
    pub result_code: i64,
    pub additional_info: Option<String>,
    pub correlation_id: Option<String>,
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Errors {
    pub code: i64,
    pub description: String,
}

impl From<Errors> for ErrorCodeAndMessage {
    fn from(error: Errors) -> Self {
        Self {
            error_code: error.code.to_string(),
            error_message: error.description,
        }
    }
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TrustpayErrorResponse {
    pub status: i64,
    pub description: Option<String>,
    pub errors: Option<Vec<Errors>>,
    pub instance_id: Option<String>,
    pub payment_description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum CreditDebitIndicator {
    Crdt,
    Dbit,
}

#[derive(strum::Display, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WebhookStatus {
    Paid,
    Rejected,
    Refunded,
    Chargebacked,
    #[serde(other)]
    Unknown,
}

impl TryFrom<WebhookStatus> for enums::AttemptStatus {
    type Error = ConnectorError;
    fn try_from(item: WebhookStatus) -> Result<Self, Self::Error> {
        match item {
            WebhookStatus::Paid => Ok(Self::Charged),
            WebhookStatus::Rejected => Ok(Self::AuthorizationFailed),
            _ => Err(ConnectorError::unexpected_response_error_http_status_unknown()),
        }
    }
}

impl TryFrom<WebhookStatus> for enums::RefundStatus {
    type Error = ConnectorError;
    fn try_from(item: WebhookStatus) -> Result<Self, Self::Error> {
        match item {
            WebhookStatus::Paid => Ok(Self::Success),
            WebhookStatus::Refunded => Ok(Self::Success),
            WebhookStatus::Rejected => Ok(Self::Failure),
            _ => Err(ConnectorError::unexpected_response_error_http_status_unknown()),
        }
    }
}

#[derive(Default, Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct WebhookReferences {
    pub merchant_reference: Option<String>,
    pub payment_id: Option<String>,
    pub payment_request_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct WebhookAmount {
    pub amount: FloatMajorUnit,
    pub currency: enums::Currency,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct WebhookPaymentInformation {
    pub credit_debit_indicator: CreditDebitIndicator,
    pub references: WebhookReferences,
    pub status: WebhookStatus,
    pub amount: WebhookAmount,
    pub status_reason_information: Option<StatusReasonInformation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct TrustpayWebhookResponse {
    pub payment_information: WebhookPaymentInformation,
    pub signature: String,
}

pub fn get_event_type_from_webhook(
    indicator: &CreditDebitIndicator,
    status: &WebhookStatus,
) -> domain_types::connector_types::EventType {
    match (indicator, status) {
        // Credit (Crdt) = Payment events
        (CreditDebitIndicator::Crdt, WebhookStatus::Paid) => {
            domain_types::connector_types::EventType::PaymentIntentSuccess
        }
        (CreditDebitIndicator::Crdt, WebhookStatus::Rejected) => {
            domain_types::connector_types::EventType::PaymentIntentFailure
        }
        // Debit (Dbit) = Refund events
        (CreditDebitIndicator::Dbit, WebhookStatus::Paid)
        | (CreditDebitIndicator::Dbit, WebhookStatus::Refunded) => {
            domain_types::connector_types::EventType::RefundSuccess
        }
        (CreditDebitIndicator::Dbit, WebhookStatus::Rejected) => {
            domain_types::connector_types::EventType::RefundFailure
        }
        // Chargeback = Dispute event
        (CreditDebitIndicator::Dbit, WebhookStatus::Chargebacked) => {
            domain_types::connector_types::EventType::DisputeLost
        }
        _ => domain_types::connector_types::EventType::IncomingWebhookEventUnspecified,
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TrustpayAuthUpdateRequest {
    pub grant_type: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TrustpayRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                PaymentFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for TrustpayAuthUpdateRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: TrustpayRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                PaymentFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            grant_type: CLIENT_CREDENTIAL.to_string(),
        })
    }
}

#[derive(Default, Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct TrustpayAuthUpdateResponse {
    pub access_token: Option<Secret<String>>,
    pub token_type: Option<String>,
    pub expires_in: Option<i64>,
    #[serde(rename = "ResultInfo")]
    pub result_info: ResultInfo,
}

impl TryFrom<ResponseRouterData<TrustpayAuthUpdateResponse, Self>>
    for RouterDataV2<
        ServerAuthenticationToken,
        PaymentFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TrustpayAuthUpdateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match (item.response.access_token, item.response.expires_in) {
            (Some(access_token), Some(expires_in)) => Ok(Self {
                response: Ok(ServerAuthenticationTokenResponseData {
                    access_token,
                    expires_in: Some(expires_in),
                    token_type: Some(item.router_data.request.grant_type.clone()),
                }),
                ..item.router_data
            }),
            _ => Ok(Self {
                response: Err(ErrorResponse {
                    code: item.response.result_info.result_code.to_string(),
                    message: item
                        .response
                        .result_info
                        .additional_info
                        .clone()
                        .unwrap_or(NO_ERROR_MESSAGE.to_string()),
                    reason: item.response.result_info.additional_info,
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                }),
                ..item.router_data
            }),
        }
    }
}

#[derive(Debug, Serialize, PartialEq)]
pub struct PaymentRequestCards<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub amount: StringMajorUnit,
    pub currency: String,
    pub pan: RawCardNumber<T>,
    pub cvv: Secret<String>,
    #[serde(rename = "exp")]
    pub expiry_date: Secret<String>,
    pub cardholder: Secret<String>,
    pub reference: String,
    #[serde(rename = "redirectUrl")]
    pub redirect_url: String,
    #[serde(rename = "billing[city]")]
    pub billing_city: String,
    #[serde(rename = "billing[country]")]
    pub billing_country: common_enums::CountryAlpha2,
    #[serde(rename = "billing[street1]")]
    pub billing_street1: Secret<String>,
    #[serde(rename = "billing[postcode]")]
    pub billing_postcode: Secret<String>,
    #[serde(rename = "customer[email]")]
    pub customer_email: Email,
    #[serde(rename = "customer[ipAddress]")]
    pub customer_ip_address: Secret<String, pii::IpAddress>,
    #[serde(rename = "browser[acceptHeader]")]
    pub browser_accept_header: String,
    #[serde(rename = "browser[language]")]
    pub browser_language: String,
    #[serde(rename = "browser[screenHeight]")]
    pub browser_screen_height: String,
    #[serde(rename = "browser[screenWidth]")]
    pub browser_screen_width: String,
    #[serde(rename = "browser[timezone]")]
    pub browser_timezone: String,
    #[serde(rename = "browser[userAgent]")]
    pub browser_user_agent: String,
    #[serde(rename = "browser[javaEnabled]")]
    pub browser_java_enabled: String,
    #[serde(rename = "browser[javaScriptEnabled]")]
    pub browser_java_script_enabled: String,
    #[serde(rename = "browser[screenColorDepth]")]
    pub browser_screen_color_depth: String,
    #[serde(rename = "browser[challengeWindow]")]
    pub browser_challenge_window: String,
    #[serde(rename = "browser[paymentAction]")]
    pub payment_action: Option<String>,
    #[serde(rename = "browser[paymentType]")]
    pub payment_type: String,
    pub descriptor: Option<String>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaymentRequestBankRedirect {
    pub payment_method: TrustpayPaymentMethod,
    pub merchant_identification: MerchantIdentification,
    pub payment_information: BankPaymentInformation,
    pub callback_urls: CallbackURLs,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaymentRequestBankTransfer {
    pub payment_method: TrustpayBankTransferPaymentMethod,
    pub merchant_identification: MerchantIdentification,
    pub payment_information: BankPaymentInformation,
    pub callback_urls: CallbackURLs,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct PaymentRequestNetworkToken {
    pub amount: StringMajorUnit,
    pub currency: enums::Currency,
    pub pan: cards::NetworkToken,
    #[serde(rename = "exp")]
    pub expiry_date: Secret<String>,
    #[serde(rename = "RedirectUrl")]
    pub redirect_url: String,
    #[serde(rename = "threeDSecureEnrollmentStatus")]
    pub enrollment_status: char,
    #[serde(rename = "threeDSecureEci")]
    pub eci: String,
    #[serde(rename = "threeDSecureAuthenticationStatus")]
    pub authentication_status: char,
    #[serde(rename = "threeDSecureVerificationId")]
    pub verification_id: Secret<String>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(untagged)]
pub enum TrustpayPaymentsRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    CardsPaymentRequest(Box<PaymentRequestCards<T>>),
    BankRedirectPaymentRequest(Box<PaymentRequestBankRedirect>),
    BankTransferPaymentRequest(Box<PaymentRequestBankTransfer>),
    NetworkTokenPaymentRequest(Box<PaymentRequestNetworkToken>),
}

// ===== SetupMandate (SetupRecurring) flow structs =====

/// TrustPay SetupMandate request - stores card credentials for future recurring payments
/// Uses zero-amount verification to validate and store card without charging
#[derive(Debug, Serialize, PartialEq)]
pub struct TrustpaySetupMandateRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    /// Amount for verification (typically 0 or minimal amount)
    pub amount: StringMajorUnit,
    /// Currency code
    pub currency: String,
    /// Card number
    pub pan: RawCardNumber<T>,
    /// Card CVV
    pub cvv: Secret<String>,
    /// Card expiry date in MM/YY format
    #[serde(rename = "exp")]
    pub expiry_date: Secret<String>,
    /// Cardholder name
    pub cardholder: Secret<String>,
    /// Merchant reference for the mandate setup
    pub reference: String,
    /// Return URL for 3DS redirect
    #[serde(rename = "redirectUrl")]
    pub redirect_url: String,
    /// Billing city
    #[serde(rename = "billing[city]")]
    pub billing_city: String,
    /// Billing country
    #[serde(rename = "billing[country]")]
    pub billing_country: common_enums::CountryAlpha2,
    /// Billing street
    #[serde(rename = "billing[street1]")]
    pub billing_street1: Secret<String>,
    /// Billing postal code
    #[serde(rename = "billing[postcode]")]
    pub billing_postcode: Secret<String>,
    /// Customer email
    #[serde(rename = "customer[email]")]
    pub customer_email: Email,
    /// Customer IP address
    #[serde(rename = "customer[ipAddress]")]
    pub customer_ip_address: Secret<String, pii::IpAddress>,
    /// Browser accept header
    #[serde(rename = "browser[acceptHeader]")]
    pub browser_accept_header: String,
    /// Browser language
    #[serde(rename = "browser[language]")]
    pub browser_language: String,
    /// Browser screen height
    #[serde(rename = "browser[screenHeight]")]
    pub browser_screen_height: String,
    /// Browser screen width
    #[serde(rename = "browser[screenWidth]")]
    pub browser_screen_width: String,
    /// Browser timezone
    #[serde(rename = "browser[timezone]")]
    pub browser_timezone: String,
    /// Browser user agent
    #[serde(rename = "browser[userAgent]")]
    pub browser_user_agent: String,
    /// Browser Java enabled
    #[serde(rename = "browser[javaEnabled]")]
    pub browser_java_enabled: String,
    /// Browser JavaScript enabled
    #[serde(rename = "browser[javaScriptEnabled]")]
    pub browser_java_script_enabled: String,
    /// Browser screen color depth
    #[serde(rename = "browser[screenColorDepth]")]
    pub browser_screen_color_depth: String,
    /// Challenge window size
    #[serde(rename = "browser[challengeWindow]")]
    pub browser_challenge_window: String,
    /// Payment action - set to "preauth" for mandate setup
    #[serde(rename = "browser[paymentAction]")]
    pub payment_action: Option<String>,
    /// Browser-level payment type (legacy compatibility)
    #[serde(rename = "browser[paymentType]")]
    pub payment_type: String,
    /// Top-level PaymentType — set to "RecurringInitial" so this preauth can be referenced
    /// by later RecurringSubsequent charges via its InstanceId.
    #[serde(rename = "PaymentType")]
    pub recurring_payment_type: String,
}

/// TrustPay SetupMandate response - reuses the card payment response structure
/// The instance_id serves as the connector_mandate_id for future recurring transactions
pub type TrustpaySetupMandateResponse = PaymentsResponseCards;

// CreateOrder flow structs for wallet initialization
#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustpayCreateIntentRequest {
    pub amount: StringMajorUnit,
    pub currency: String,
    pub init_apple_pay: Option<bool>,
    pub init_google_pay: Option<bool>,
    pub reference: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustpayCreateIntentResponse {
    // TrustPay's authorization secrets used by client
    pub secrets: SdkSecretInfo,
    // 	Data object to be used for Apple Pay or Google Pay
    #[serde(flatten)]
    pub init_result_data: InitResultData,
    // Unique operation/transaction identifier
    pub instance_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InitResultData {
    AppleInitResultData(TrustpayApplePayResponse),
    GoogleInitResultData(TrustpayGooglePayResponse),
}

#[derive(Clone, Default, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SdkSecretInfo {
    pub display: Secret<String>,
    pub payment: Secret<String>,
}

#[derive(Clone, Default, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustpayApplePayResponse {
    pub country_code: common_enums::CountryAlpha2,
    pub currency_code: common_enums::Currency,
    pub supported_networks: Vec<String>,
    pub merchant_capabilities: Vec<String>,
    pub total: ApplePayTotalInfo,
}

#[derive(Clone, Default, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplePayTotalInfo {
    pub label: String,
    pub amount: StringMajorUnit,
}

#[derive(Clone, Default, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustpayGooglePayResponse {
    pub merchant_info: GooglePayMerchantInfo,
    pub allowed_payment_methods: Vec<GooglePayAllowedPaymentMethods>,
    pub transaction_info: GooglePayTransactionInfo,
}

#[derive(Clone, Default, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GooglePayMerchantInfo {
    pub merchant_name: String,
    pub merchant_id: String,
}

#[derive(Clone, Default, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GooglePayTransactionInfo {
    pub country_code: common_enums::CountryAlpha2,
    pub currency_code: common_enums::Currency,
    pub total_price_status: String,
    pub total_price: StringMajorUnit,
}

#[derive(Clone, Default, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GooglePayAllowedPaymentMethods {
    #[serde(rename = "type")]
    pub payment_method_type: String,
    pub parameters: GpayAllowedMethodsParameters,
    pub tokenization_specification: GpayTokenizationSpecification,
}

#[derive(Clone, Default, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpayAllowedMethodsParameters {
    pub allowed_auth_methods: Vec<String>,
    pub allowed_card_networks: Vec<String>,
    pub assurance_details_required: Option<bool>,
}

#[derive(Clone, Default, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpayTokenizationSpecification {
    #[serde(rename = "type")]
    pub token_specification_type: String,
    pub parameters: GpayTokenParameters,
}

#[derive(Clone, Default, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpayTokenParameters {
    pub gateway: String,
    pub gateway_merchant_id: String,
}

#[derive(Debug, Serialize, Eq, PartialEq)]
pub struct TrustpayMandatoryParams {
    pub billing_city: String,
    pub billing_country: common_enums::CountryAlpha2,
    pub billing_street1: Secret<String>,
    pub billing_postcode: Secret<String>,
    pub billing_first_name: Secret<String>,
}

fn get_card_request_data<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    browser_info: &BrowserInformation,
    params: TrustpayMandatoryParams,
    amount: StringMajorUnit,
    ccard: &Card<T>,
    return_url: String,
) -> Result<TrustpayPaymentsRequest<T>, Error> {
    let email = item.request.get_email()?;
    let customer_ip_address = browser_info.get_ip_address()?;
    let billing_last_name = item
        .resource_common_data
        .get_billing()?
        .address
        .as_ref()
        .and_then(|address| address.last_name.clone());
    Ok(TrustpayPaymentsRequest::CardsPaymentRequest(Box::new(
        PaymentRequestCards {
            amount,
            currency: item.request.currency.to_string(),
            pan: ccard.card_number.clone(),
            cvv: ccard.card_cvc.clone(),
            expiry_date: {
                let year = ccard.card_exp_year.peek();
                let year_2_digit = if year.len() == 4 { &year[2..] } else { year };
                Secret::new(format!("{}/{}", ccard.card_exp_month.peek(), year_2_digit))
            },
            cardholder: get_full_name(params.billing_first_name, billing_last_name),
            reference: item
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            redirect_url: return_url,
            billing_city: params.billing_city,
            billing_country: params.billing_country,
            billing_street1: params.billing_street1,
            billing_postcode: params.billing_postcode,
            customer_email: email,
            customer_ip_address,
            browser_accept_header: browser_info.get_accept_header()?,
            browser_language: browser_info.get_language()?,
            browser_screen_height: browser_info.get_screen_height()?.to_string(),
            browser_screen_width: browser_info.get_screen_width()?.to_string(),
            browser_timezone: browser_info.get_time_zone()?.to_string(),
            browser_user_agent: browser_info.get_user_agent()?,
            browser_java_enabled: browser_info.get_java_enabled()?.to_string(),
            browser_java_script_enabled: browser_info.get_java_script_enabled()?.to_string(),
            browser_screen_color_depth: browser_info.get_color_depth()?.to_string(),
            browser_challenge_window: CHALLENGE_WINDOW.to_string(),
            payment_action: None,
            payment_type: PAYMENT_TYPE.to_string(),
            descriptor: item
                .request
                .billing_descriptor
                .as_ref()
                .and_then(|descriptor| descriptor.statement_descriptor.clone()),
        },
    )))
}

fn get_full_name(
    billing_first_name: Secret<String>,
    billing_last_name: Option<Secret<String>>,
) -> Secret<String> {
    match billing_last_name {
        Some(last_name) => format!("{} {}", billing_first_name.peek(), last_name.peek()).into(),
        None => billing_first_name,
    }
}

fn get_debtor_info<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    pm: TrustpayPaymentMethod,
    params: TrustpayMandatoryParams,
) -> CustomResult<Option<DebtorInformation>, IntegrationError> {
    let billing_last_name = item
        .resource_common_data
        .get_billing()?
        .address
        .as_ref()
        .and_then(|address| address.last_name.clone());
    Ok(match pm {
        TrustpayPaymentMethod::Blik => Some(DebtorInformation {
            name: get_full_name(params.billing_first_name, billing_last_name),
            email: item.request.get_email()?,
        }),
        TrustpayPaymentMethod::Eps
        | TrustpayPaymentMethod::Giropay
        | TrustpayPaymentMethod::IDeal
        | TrustpayPaymentMethod::Sofort => None,
    })
}

fn get_bank_transfer_debtor_info<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    pm: TrustpayBankTransferPaymentMethod,
    params: TrustpayMandatoryParams,
) -> CustomResult<Option<DebtorInformation>, IntegrationError> {
    let billing_last_name = item
        .resource_common_data
        .get_billing()?
        .address
        .as_ref()
        .and_then(|address| address.last_name.clone());
    Ok(match pm {
        TrustpayBankTransferPaymentMethod::SepaCreditTransfer
        | TrustpayBankTransferPaymentMethod::InstantBankTransfer
        | TrustpayBankTransferPaymentMethod::InstantBankTransferFI
        | TrustpayBankTransferPaymentMethod::InstantBankTransferPL => Some(DebtorInformation {
            name: get_full_name(params.billing_first_name, billing_last_name),
            email: item.request.get_email()?,
        }),
    })
}

fn get_mandatory_fields<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
) -> Result<TrustpayMandatoryParams, Error> {
    let billing_address = item
        .resource_common_data
        .get_billing()?
        .address
        .as_ref()
        .ok_or_else(utils::missing_field_err("billing.address"))?;
    Ok(TrustpayMandatoryParams {
        billing_city: billing_address.get_city()?.peek().to_owned(),
        billing_country: billing_address.get_country()?.to_owned(),
        billing_street1: billing_address.get_line1()?.to_owned(),
        billing_postcode: billing_address.get_zip()?.to_owned(),
        billing_first_name: billing_address.get_first_name()?.to_owned(),
    })
}

fn get_bank_redirection_request_data<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    bank_redirection_data: &BankRedirectData,
    params: TrustpayMandatoryParams,
    amount: StringMajorUnit,
    auth: TrustpayAuthType,
) -> Result<TrustpayPaymentsRequest<T>, error_stack::Report<IntegrationError>> {
    let pm = TrustpayPaymentMethod::try_from(bank_redirection_data)?;
    let return_url = item.request.get_router_return_url()?;
    let payment_request =
        TrustpayPaymentsRequest::BankRedirectPaymentRequest(Box::new(PaymentRequestBankRedirect {
            payment_method: pm.clone(),
            merchant_identification: MerchantIdentification {
                project_id: auth.project_id,
            },
            payment_information: BankPaymentInformation {
                amount: Amount {
                    amount,
                    currency: item.request.currency.to_string(),
                },
                references: References {
                    merchant_reference: item
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                },
                debtor: get_debtor_info(item, pm, params)?,
            },
            callback_urls: CallbackURLs {
                success: format!("{return_url}?status=SuccessOk"),
                cancel: return_url.clone(),
                error: return_url,
            },
        }));
    Ok(payment_request)
}

fn get_bank_transfer_request_data<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    bank_transfer_data: &BankTransferData,
    params: TrustpayMandatoryParams,
    amount: StringMajorUnit,
    auth: TrustpayAuthType,
) -> Result<TrustpayPaymentsRequest<T>, error_stack::Report<IntegrationError>> {
    let pm = TrustpayBankTransferPaymentMethod::try_from(bank_transfer_data)?;
    let return_url = item.request.get_router_return_url()?;
    let payment_request =
        TrustpayPaymentsRequest::BankTransferPaymentRequest(Box::new(PaymentRequestBankTransfer {
            payment_method: pm.clone(),
            merchant_identification: MerchantIdentification {
                project_id: auth.project_id,
            },
            payment_information: BankPaymentInformation {
                amount: Amount {
                    amount,
                    currency: item.request.currency.to_string(),
                },
                references: References {
                    merchant_reference: item
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                },
                debtor: get_bank_transfer_debtor_info(item, pm, params)?,
            },
            callback_urls: CallbackURLs {
                success: format!("{return_url}?status=SuccessOk"),
                cancel: return_url.clone(),
                error: return_url,
            },
        }));
    Ok(payment_request)
}

// Implement GetFormData for TrustpayPaymentsRequest to satisfy the macro requirement
// This will never be called since TrustPay only uses Json and FormUrlEncoded
impl<T> connectors::macros::GetFormData for TrustpayPaymentsRequest<T>
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    #[allow(clippy::unimplemented)]
    fn get_form_data(&self) -> common_utils::request::MultipartData {
        // This should never be called for TrustPay since we only use Json and FormUrlEncoded
        unimplemented!("TrustPay only support Json and FormUrlEncoded content types.")
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TrustpayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for TrustpayPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: TrustpayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let browser_info = item
            .router_data
            .request
            .browser_info
            .clone()
            .unwrap_or_default();
        //     we don't want payment to fail, even if we don't get browser info from sdk, and these default values are present in Trustpay's doc,
        //     Trustpay required to pass this values, when we don't get from sdk. That's why this values are hard coded.
        let default_browser_info = BrowserInformation {
            color_depth: Some(browser_info.color_depth.unwrap_or(24)),
            java_enabled: Some(browser_info.java_enabled.unwrap_or(false)),
            java_script_enabled: Some(browser_info.java_script_enabled.unwrap_or(true)),
            language: Some(browser_info.language.unwrap_or("en-US".to_string())),
            screen_height: Some(browser_info.screen_height.unwrap_or(1080)),
            screen_width: Some(browser_info.screen_width.unwrap_or(1920)),
            time_zone: Some(browser_info.time_zone.unwrap_or(3600)),
            accept_header: Some(browser_info.accept_header.unwrap_or("*".to_string())),
            user_agent: browser_info.user_agent,
            ip_address: browser_info.ip_address,
            os_type: None,
            os_version: None,
            device_model: None,
            accept_language: Some(browser_info.accept_language.unwrap_or("en".to_string())),
            referer: None,
        };
        let params = get_mandatory_fields(item.router_data.clone())?;
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
        let auth = TrustpayAuthType::try_from(&item.router_data.connector_config).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        match item.router_data.request.payment_method_data {
            PaymentMethodData::Card(ref ccard) => Ok(get_card_request_data(
                item.router_data.clone(),
                &default_browser_info,
                params,
                amount,
                ccard,
                item.router_data.request.get_router_return_url()?,
            )?),
            PaymentMethodData::BankRedirect(ref bank_redirection_data) => {
                get_bank_redirection_request_data(
                    item.router_data.clone(),
                    bank_redirection_data,
                    params,
                    amount,
                    auth,
                )
            }
            PaymentMethodData::BankTransfer(ref bank_transfer_data) => {
                get_bank_transfer_request_data(
                    item.router_data.clone(),
                    bank_transfer_data,
                    params,
                    amount,
                    auth,
                )
            }
            PaymentMethodData::NetworkToken(ref token_data) => {
                let month = token_data.get_network_token_expiry_month();
                let year = token_data.get_network_token_expiry_year();
                let expiry_date =
                    utils::get_token_expiry_month_year_2_digit_with_delimiter(month, year);
                Ok(Self::NetworkTokenPaymentRequest(Box::new(
                    PaymentRequestNetworkToken {
                        amount,
                        currency: item.router_data.request.currency,
                        pan: token_data.get_network_token(),
                        expiry_date,
                        redirect_url: item.router_data.request.get_router_return_url()?,
                        enrollment_status: STATUS,
                        eci: token_data.eci.clone().ok_or(
                            IntegrationError::MissingRequiredField {
                                field_name: "eci",
                                context: Default::default(),
                            },
                        )?,
                        authentication_status: STATUS,
                        verification_id: token_data.get_cryptogram().ok_or(
                            IntegrationError::MissingRequiredField {
                                field_name: "verification_id",
                                context: Default::default(),
                            },
                        )?,
                    },
                )))
            }
            PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::Wallet(_)
            | PaymentMethodData::PayLater(_)
            | PaymentMethodData::BankDebit(_)
            | PaymentMethodData::Crypto(_)
            | PaymentMethodData::MandatePayment
            | PaymentMethodData::Reward
            | PaymentMethodData::RealTimePayment(_)
            | PaymentMethodData::MobilePayment(_)
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::PaymentMethodToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
                Err(error_stack::report!(IntegrationError::NotSupported {
                    message: utils::get_unimplemented_payment_method_error_message("trustpay"),
                    connector: "trustpay",
                    context: Default::default(),
                }))
            }
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum TrustpayRefundRequest {
    CardsRefund(Box<TrustpayRefundRequestCards>),
    BankRedirectRefund(Box<TrustpayRefundRequestBankRedirect>),
}

#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustpayRefundRequestCards {
    instance_id: String,
    amount: StringMajorUnit,
    currency: String,
    reference: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustpayRefundRequestBankRedirect {
    pub merchant_identification: MerchantIdentification,
    pub payment_information: BankPaymentInformation,
}

// Implement GetFormData for TrustpayRefundRequest to satisfy the macro requirement
// This will never be called since TrustPay only uses Json and FormUrlEncoded
impl connectors::macros::GetFormData for TrustpayRefundRequest {
    #[allow(clippy::unimplemented)]
    fn get_form_data(&self) -> common_utils::request::MultipartData {
        // TrustPay refunds only support Json and FormUrlEncoded content types
        unimplemented!("TrustPay only support Json and FormUrlEncoded content types.")
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TrustpayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for TrustpayRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: TrustpayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.minor_refund_amount,
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;
        match item.router_data.resource_common_data.payment_method {
            Some(enums::PaymentMethod::BankRedirect) => {
                let auth = TrustpayAuthType::try_from(&item.router_data.connector_config)
                    .change_context(IntegrationError::FailedToObtainAuthType {
                        context: Default::default(),
                    })?;
                Ok(Self::BankRedirectRefund(Box::new(
                    TrustpayRefundRequestBankRedirect {
                        merchant_identification: MerchantIdentification {
                            project_id: auth.project_id,
                        },
                        payment_information: BankPaymentInformation {
                            amount: Amount {
                                amount,
                                currency: item.router_data.request.currency.to_string(),
                            },
                            references: References {
                                merchant_reference: item.router_data.request.refund_id.clone(),
                            },
                            debtor: None,
                        },
                    },
                )))
            }
            _ => Ok(Self::CardsRefund(Box::new(TrustpayRefundRequestCards {
                instance_id: item.router_data.request.connector_transaction_id.clone(),
                amount,
                currency: item.router_data.request.currency.to_string(),
                reference: item.router_data.request.refund_id.clone(),
            }))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RefundResponse {
    CardsRefund(Box<CardsRefundResponse>),
    WebhookRefund(Box<WebhookPaymentInformation>),
    BankRedirectRefund(Box<BankRedirectRefundResponse>),
    BankRedirectRefundSyncResponse(Box<SyncResponseBankRedirect>),
    BankRedirectError(Box<ErrorResponseBankRedirect>),
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardsRefundResponse {
    pub status: i64,
    pub description: Option<String>,
    pub instance_id: String,
    pub payment_status: String,
    pub payment_description: Option<String>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct BankRedirectRefundResponse {
    pub payment_request_id: i64,
    pub result_info: ResultInfo,
}

impl<F, T> TryFrom<ResponseRouterData<RefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, T, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<RefundResponse, Self>) -> Result<Self, Self::Error> {
        let (error, response) = match item.response {
            RefundResponse::CardsRefund(response) => {
                handle_cards_refund_response(*response, item.http_code)?
            }
            RefundResponse::WebhookRefund(response) => {
                handle_webhooks_refund_response(*response, item.http_code)?
            }
            RefundResponse::BankRedirectRefund(response) => {
                handle_bank_redirects_refund_response(*response, item.http_code)
            }
            RefundResponse::BankRedirectRefundSyncResponse(response) => {
                handle_bank_redirects_refund_sync_response(*response, item.http_code)
            }
            RefundResponse::BankRedirectError(response) => {
                handle_bank_redirects_refund_sync_error_response(*response, item.http_code)
            }
        };
        Ok(Self {
            response: error.map_or_else(|| Ok(response), Err),
            ..item.router_data
        })
    }
}

fn handle_cards_refund_response(
    response: CardsRefundResponse,
    status_code: u16,
) -> CustomResult<(Option<ErrorResponse>, RefundsResponseData), ConnectorError> {
    let (refund_status, message) = get_refund_status(&response.payment_status);
    let error = match message {
        Some(message) => Some(ErrorResponse {
            code: response.payment_status,
            message: message.clone(),
            reason: Some(message),
            status_code,
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        }),
        None => None,
    };
    let refund_response_data = RefundsResponseData {
        connector_refund_id: response.instance_id,
        refund_status,
        status_code,
    };
    Ok((error, refund_response_data))
}

pub fn handle_webhooks_refund_response(
    response: WebhookPaymentInformation,
    status_code: u16,
) -> CustomResult<(Option<ErrorResponse>, RefundsResponseData), ConnectorError> {
    let refund_status = enums::RefundStatus::try_from(response.status)?;
    let error = match utils::is_refund_failure(refund_status) {
        true => {
            let reason_info = response.status_reason_information.unwrap_or_default();
            Some(ErrorResponse {
                code: reason_info
                    .reason
                    .code
                    .clone()
                    .unwrap_or(NO_ERROR_CODE.to_string()),
                // message vary for the same code, so relying on code alone as it is unique
                message: reason_info
                    .reason
                    .code
                    .unwrap_or(NO_ERROR_MESSAGE.to_string()),
                reason: reason_info.reason.reject_reason,
                status_code,
                attempt_status: None,
                connector_transaction_id: response.references.payment_id.clone(),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            })
        }
        false => None,
    };
    let refund_response_data = RefundsResponseData {
        connector_refund_id: match response.references.payment_id {
            Some(id) => id,
            None => {
                return Err(report!(
                    ConnectorError::response_handling_failed_with_context(
                        status_code,
                        Some("missing connector refund id".to_string()),
                    )
                ));
            }
        },
        refund_status,
        status_code,
    };
    Ok((error, refund_response_data))
}

/// Same as [`handle_webhooks_refund_response`], for incoming webhook processing with
/// [`IntegrationError`].
pub fn handle_webhooks_refund_response_incoming_webhook(
    response: WebhookPaymentInformation,
    status_code: u16,
) -> CustomResult<(Option<ErrorResponse>, RefundsResponseData), WebhookError> {
    let refund_status = match response.status {
        WebhookStatus::Paid | WebhookStatus::Refunded => enums::RefundStatus::Success,
        WebhookStatus::Rejected => enums::RefundStatus::Failure,
        WebhookStatus::Chargebacked | WebhookStatus::Unknown => {
            return Err(report!(WebhookError::WebhookProcessingFailed));
        }
    };
    let error = match utils::is_refund_failure(refund_status) {
        true => {
            let reason_info = response.status_reason_information.unwrap_or_default();
            Some(ErrorResponse {
                code: reason_info
                    .reason
                    .code
                    .clone()
                    .unwrap_or(NO_ERROR_CODE.to_string()),
                // message vary for the same code, so relying on code alone as it is unique
                message: reason_info
                    .reason
                    .code
                    .unwrap_or(NO_ERROR_MESSAGE.to_string()),
                reason: reason_info.reason.reject_reason,
                status_code,
                attempt_status: None,
                connector_transaction_id: response.references.payment_id.clone(),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            })
        }
        false => None,
    };
    let refund_response_data = RefundsResponseData {
        connector_refund_id: response
            .references
            .payment_id
            .ok_or_else(|| report!(WebhookError::WebhookProcessingFailed))?,
        refund_status,
        status_code,
    };
    Ok((error, refund_response_data))
}

fn handle_bank_redirects_refund_response(
    response: BankRedirectRefundResponse,
    status_code: u16,
) -> (Option<ErrorResponse>, RefundsResponseData) {
    let (refund_status, msg) = get_refund_status_from_result_info(response.result_info.result_code);
    let error = match msg.is_some() {
        true => Some(ErrorResponse {
            code: response.result_info.result_code.to_string(),
            // message vary for the same code, so relying on code alone as it is unique
            message: response.result_info.result_code.to_string(),
            reason: msg.map(|message| message.to_string()),
            status_code,
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        }),
        false => None,
    };
    let refund_response_data = RefundsResponseData {
        connector_refund_id: response.payment_request_id.to_string(),
        refund_status,
        status_code,
    };
    (error, refund_response_data)
}

fn handle_bank_redirects_refund_sync_response(
    response: SyncResponseBankRedirect,
    status_code: u16,
) -> (Option<ErrorResponse>, RefundsResponseData) {
    let refund_status = enums::RefundStatus::from(response.payment_information.status);
    let error = match utils::is_refund_failure(refund_status) {
        true => {
            let reason_info = response
                .payment_information
                .status_reason_information
                .unwrap_or_default();
            Some(ErrorResponse {
                code: reason_info
                    .reason
                    .code
                    .clone()
                    .unwrap_or(NO_ERROR_CODE.to_string()),
                // message vary for the same code, so relying on code alone as it is unique
                message: reason_info
                    .reason
                    .code
                    .unwrap_or(NO_ERROR_MESSAGE.to_string()),
                reason: reason_info.reason.reject_reason,
                status_code,
                attempt_status: None,
                connector_transaction_id: None,
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            })
        }
        false => None,
    };
    let refund_response_data = RefundsResponseData {
        connector_refund_id: response.payment_information.references.payment_request_id,
        refund_status,
        status_code,
    };
    (error, refund_response_data)
}

fn handle_bank_redirects_refund_sync_error_response(
    response: ErrorResponseBankRedirect,
    status_code: u16,
) -> (Option<ErrorResponse>, RefundsResponseData) {
    let error = Some(ErrorResponse {
        code: response.payment_result_info.result_code.to_string(),
        message: response
            .payment_result_info
            .additional_info
            .clone()
            .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
        reason: response.payment_result_info.additional_info,
        status_code,
        attempt_status: None,
        connector_transaction_id: None,
        network_advice_code: None,
        network_decline_code: None,
        network_error_message: None,
    });
    //unreachable case as we are sending error as Some()
    let refund_response_data = RefundsResponseData {
        connector_refund_id: "".to_string(),
        refund_status: enums::RefundStatus::Failure,
        status_code,
    };
    (error, refund_response_data)
}

fn get_refund_status(payment_status: &str) -> (enums::RefundStatus, Option<String>) {
    let (is_failed, failure_message) = is_payment_failed(payment_status);
    match payment_status {
        "000.200.000" => (enums::RefundStatus::Pending, None),
        _ if is_failed => (
            enums::RefundStatus::Failure,
            Some(failure_message.to_string()),
        ),
        _ if is_payment_successful(payment_status) => (enums::RefundStatus::Success, None),
        _ => (enums::RefundStatus::Pending, None),
    }
}

fn get_refund_status_from_result_info(
    result_code: i64,
) -> (enums::RefundStatus, Option<&'static str>) {
    match result_code {
        1001000 => (enums::RefundStatus::Success, None),
        1130001 => (enums::RefundStatus::Pending, Some("MapiPending")),
        1130000 => (enums::RefundStatus::Pending, Some("MapiSuccess")),
        1130004 => (enums::RefundStatus::Pending, Some("MapiProcessing")),
        1130002 => (enums::RefundStatus::Pending, Some("MapiAnnounced")),
        1130003 => (enums::RefundStatus::Pending, Some("MapiAuthorized")),
        1130005 => (enums::RefundStatus::Pending, Some("MapiAuthorizedOnly")),
        1112008 => (enums::RefundStatus::Failure, Some("InvalidPaymentState")),
        1112009 => (enums::RefundStatus::Failure, Some("RefundRejected")),
        1122006 => (
            enums::RefundStatus::Failure,
            Some("AccountCurrencyNotAllowed"),
        ),
        1132000 => (enums::RefundStatus::Failure, Some("InvalidMapiRequest")),
        1132001 => (enums::RefundStatus::Failure, Some("UnknownAccount")),
        1132002 => (
            enums::RefundStatus::Failure,
            Some("MerchantAccountDisabled"),
        ),
        1132003 => (enums::RefundStatus::Failure, Some("InvalidSign")),
        1132004 => (enums::RefundStatus::Failure, Some("DisposableBalance")),
        1132005 => (enums::RefundStatus::Failure, Some("TransactionNotFound")),
        1132006 => (enums::RefundStatus::Failure, Some("UnsupportedTransaction")),
        1132007 => (enums::RefundStatus::Failure, Some("GeneralMapiError")),
        1132008 => (
            enums::RefundStatus::Failure,
            Some("UnsupportedCurrencyConversion"),
        ),
        1132009 => (enums::RefundStatus::Failure, Some("UnknownMandate")),
        1132010 => (enums::RefundStatus::Failure, Some("CanceledMandate")),
        1132011 => (enums::RefundStatus::Failure, Some("MissingCid")),
        1132012 => (enums::RefundStatus::Failure, Some("MandateAlreadyPaid")),
        1132013 => (enums::RefundStatus::Failure, Some("AccountIsTesting")),
        1132014 => (enums::RefundStatus::Failure, Some("RequestThrottled")),
        1133000 => (enums::RefundStatus::Failure, Some("InvalidAuthentication")),
        1133001 => (enums::RefundStatus::Failure, Some("ServiceNotAllowed")),
        1133002 => (enums::RefundStatus::Failure, Some("PaymentRequestNotFound")),
        1133003 => (enums::RefundStatus::Failure, Some("UnexpectedGateway")),
        1133004 => (enums::RefundStatus::Failure, Some("MissingExternalId")),
        1152000 => (enums::RefundStatus::Failure, Some("RiskDecline")),
        _ => (enums::RefundStatus::Pending, None),
    }
}

// CreateOrder flow implementations for wallet initialization
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TrustpayRouterData<
            RouterDataV2<
                CreateOrder,
                PaymentFlowData,
                PaymentCreateOrderData,
                PaymentCreateOrderResponse,
            >,
            T,
        >,
    > for TrustpayCreateIntentRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: TrustpayRouterData<
            RouterDataV2<
                CreateOrder,
                PaymentFlowData,
                PaymentCreateOrderData,
                PaymentCreateOrderResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.amount,
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        let is_apple_pay = item
            .router_data
            .request
            .payment_method_type
            .as_ref()
            .map(|pmt| matches!(pmt, enums::PaymentMethodType::ApplePay));

        let is_google_pay = item
            .router_data
            .request
            .payment_method_type
            .as_ref()
            .map(|pmt| matches!(pmt, enums::PaymentMethodType::GooglePay));

        Ok(Self {
            amount,
            currency: item.router_data.request.currency.to_string(),
            init_apple_pay: is_apple_pay,
            init_google_pay: is_google_pay,
            reference: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        })
    }
}

impl TryFrom<ResponseRouterData<TrustpayCreateIntentResponse, Self>>
    for RouterDataV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TrustpayCreateIntentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let http_code = item.http_code;
        let instance_id = item.response.instance_id.clone();
        let create_intent_response = item.response.init_result_data.clone();
        let secrets = item.response.secrets.clone();

        // Get payment_method_type from the request
        let payment_method_type = match item.router_data.request.payment_method_type.as_ref() {
            Some(pmt) => pmt,
            None => {
                return Err(report!(
                    ConnectorError::response_handling_failed_with_context(
                        http_code,
                        Some("missing payment_method_type".to_string()),
                    )
                ));
            }
        };

        match (payment_method_type, create_intent_response) {
            (
                enums::PaymentMethodType::ApplePay,
                InitResultData::AppleInitResultData(apple_pay_response),
            ) => match get_apple_pay_session(instance_id, &secrets, apple_pay_response, item) {
                Ok(v) => Ok(v),
                Err(e) => Err(report!(utils::response_handling_fail_for_connector(
                    http_code, "trustpay"
                ))
                .attach(e)),
            },
            (
                enums::PaymentMethodType::GooglePay,
                InitResultData::GoogleInitResultData(google_pay_response),
            ) => match get_google_pay_session(instance_id, &secrets, google_pay_response, item) {
                Ok(v) => Ok(v),
                Err(e) => Err(report!(utils::response_handling_fail_for_connector(
                    http_code, "trustpay"
                ))
                .attach(e)),
            },
            _ => Err(report!(
                ConnectorError::response_handling_failed_with_context(
                    http_code,
                    Some("invalid wallet configuration for create intent response".to_string()),
                )
            )),
        }
    }
}

pub(crate) fn get_apple_pay_session(
    instance_id: String,
    secrets: &SdkSecretInfo,
    apple_pay_init_result: TrustpayApplePayResponse,
    item: ResponseRouterData<
        TrustpayCreateIntentResponse,
        RouterDataV2<
            CreateOrder,
            PaymentFlowData,
            PaymentCreateOrderData,
            PaymentCreateOrderResponse,
        >,
    >,
) -> Result<
    RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>,
    Error,
> {
    let session_token =
        ClientAuthenticationTokenData::ApplePay(Box::new(ApplepayClientAuthenticationResponse {
            session_response: Some(ApplePaySessionResponse::ThirdPartySdk(
                ThirdPartySdkSessionResponse {
                    secrets: secrets.to_owned().into(),
                },
            )),
            payment_request_data: Some(ApplePayPaymentRequest {
                country_code: apple_pay_init_result.country_code,
                currency_code: apple_pay_init_result.currency_code,
                supported_networks: Some(apple_pay_init_result.supported_networks.clone()),
                merchant_capabilities: Some(apple_pay_init_result.merchant_capabilities.clone()),
                total: AmountInfo {
                    label: apple_pay_init_result.total.label.clone(),
                    amount: TrustpayAmountConvertor::convert_back(
                        apple_pay_init_result.total.amount.clone(),
                        apple_pay_init_result.currency_code,
                    )
                    .change_context(IntegrationError::InvalidDataFormat {
                        field_name: "amount",
                        context: Default::default(),
                    })?,
                    total_type: None,
                },
                merchant_identifier: None,
                required_billing_contact_fields: None,
                required_shipping_contact_fields: None,
                recurring_payment_request: None,
            }),
            connector: "trustpay".to_string(),
            delayed_session_token: true,
            sdk_next_action: {
                SdkNextAction {
                    next_action: NextActionCall::Confirm,
                }
            },
            connector_reference_id: None,
            connector_sdk_public_key: None,
            connector_merchant_id: None,
        }));

    Ok(RouterDataV2 {
        resource_common_data: PaymentFlowData {
            status: enums::AttemptStatus::AuthenticationPending,
            ..item.router_data.resource_common_data.clone()
        },
        response: Ok(PaymentCreateOrderResponse {
            connector_order_id: instance_id,
            session_data: Some(session_token),
        }),
        ..item.router_data.clone()
    })
}

pub(crate) fn get_google_pay_session(
    instance_id: String,
    secrets: &SdkSecretInfo,
    google_pay_init_result: TrustpayGooglePayResponse,
    item: ResponseRouterData<
        TrustpayCreateIntentResponse,
        RouterDataV2<
            CreateOrder,
            PaymentFlowData,
            PaymentCreateOrderData,
            PaymentCreateOrderResponse,
        >,
    >,
) -> Result<
    RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>,
    Error,
> {
    let session_token = ClientAuthenticationTokenData::GooglePay(Box::new(
        GpayClientAuthenticationResponse::GooglePaySession(GooglePaySessionResponse {
            connector: "trustpay".to_string(),
            delayed_session_token: true,
            sdk_next_action: {
                SdkNextAction {
                    next_action: NextActionCall::Confirm,
                }
            },
            merchant_info: google_pay_init_result.merchant_info.into(),
            allowed_payment_methods: google_pay_init_result
                .allowed_payment_methods
                .into_iter()
                .map(Into::into)
                .collect(),
            transaction_info: google_pay_init_result.transaction_info.into(),
            secrets: Some((*secrets).clone().into()),
            shipping_address_required: false,
            email_required: false,
            shipping_address_parameters: GpayShippingAddressParameters {
                phone_number_required: false,
            },
        }),
    ));

    Ok(RouterDataV2 {
        resource_common_data: PaymentFlowData {
            status: enums::AttemptStatus::AuthenticationPending,
            ..item.router_data.resource_common_data.clone()
        },
        response: Ok(PaymentCreateOrderResponse {
            connector_order_id: instance_id,
            session_data: Some(session_token),
        }),
        ..item.router_data.clone()
    })
}

// Helper structs for serializing wallet session data
impl From<SdkSecretInfo> for SecretInfoToInitiateSdk {
    fn from(secrets: SdkSecretInfo) -> Self {
        Self {
            display: secrets.display,
            payment: Some(secrets.payment),
        }
    }
}

// From implementations for GooglePay types
impl From<GooglePayTransactionInfo> for domain_types::connector_types::GpayTransactionInfo {
    fn from(value: GooglePayTransactionInfo) -> Self {
        let total_price =
            TrustpayAmountConvertor::convert_back(value.total_price, value.currency_code)
                .unwrap_or_else(|_| MinorUnit::new(0));

        Self {
            country_code: value.country_code,
            currency_code: value.currency_code,
            total_price_status: value.total_price_status,
            total_price,
        }
    }
}

impl From<GooglePayMerchantInfo> for GpayMerchantInfo {
    fn from(value: GooglePayMerchantInfo) -> Self {
        Self {
            merchant_id: Some(value.merchant_id),
            merchant_name: value.merchant_name,
        }
    }
}

impl From<GooglePayAllowedPaymentMethods> for GpayAllowedPaymentMethods {
    fn from(value: GooglePayAllowedPaymentMethods) -> Self {
        Self {
            payment_method_type: value.payment_method_type,
            parameters: domain_types::connector_types::GpayAllowedMethodsParameters {
                allowed_auth_methods: value.parameters.allowed_auth_methods,
                allowed_card_networks: value.parameters.allowed_card_networks,
                billing_address_required: None,
                billing_address_parameters: None,
                assurance_details_required: value.parameters.assurance_details_required,
            },
            tokenization_specification:
                domain_types::connector_types::GpayTokenizationSpecification {
                    token_specification_type: value
                        .tokenization_specification
                        .token_specification_type,
                    parameters: domain_types::connector_types::GpayTokenParameters {
                        gateway: Some(value.tokenization_specification.parameters.gateway),
                        gateway_merchant_id: Some(
                            value
                                .tokenization_specification
                                .parameters
                                .gateway_merchant_id,
                        ),
                        public_key: None,
                        protocol_version: None,
                    },
                },
        }
    }
}

// ===== SetupMandate (SetupRecurring) TryFrom implementations =====

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TrustpayRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for TrustpaySetupMandateRequest<T>
{
    type Error = Error;

    fn try_from(
        item: TrustpayRouterData<
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

        // Extract card data
        let card_data = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card) => card,
            _ => {
                return Err(error_stack::report!(IntegrationError::NotSupported {
                    message: utils::get_unimplemented_payment_method_error_message(
                        "trustpay SetupMandate"
                    ),
                    connector: "trustpay",
                    context: Default::default(),
                }))
            }
        };

        // Get billing address
        let address = router_data.resource_common_data.get_billing_address()?;

        // Extract mandatory params
        let billing_city =
            address
                .city
                .clone()
                .ok_or_else(|| IntegrationError::MissingRequiredField {
                    field_name: "billing.address.city",
                    context: Default::default(),
                })?;
        let billing_country = router_data.resource_common_data.get_billing_country()?;
        let billing_street1 =
            address
                .line1
                .clone()
                .ok_or_else(|| IntegrationError::MissingRequiredField {
                    field_name: "billing.address.line1",
                    context: Default::default(),
                })?;
        let billing_postcode =
            address
                .zip
                .clone()
                .ok_or_else(|| IntegrationError::MissingRequiredField {
                    field_name: "billing.address.zip",
                    context: Default::default(),
                })?;
        let billing_first_name =
            address
                .first_name
                .clone()
                .ok_or_else(|| IntegrationError::MissingRequiredField {
                    field_name: "billing.address.first_name",
                    context: Default::default(),
                })?;
        let billing_last_name = address.last_name.clone();

        // Get browser info
        let browser_info = router_data.request.browser_info.as_ref().ok_or_else(|| {
            IntegrationError::MissingRequiredField {
                field_name: "browser_info",
                context: Default::default(),
            }
        })?;

        // Get email
        let customer_email = router_data.request.get_email()?;

        // Get IP address
        let customer_ip_address = browser_info.get_ip_address()?;

        // Get return URL
        let redirect_url = router_data
            .request
            .router_return_url
            .clone()
            .ok_or_else(|| IntegrationError::MissingRequiredField {
                field_name: "return_url",
                context: Default::default(),
            })?;

        let expiry_date =
            card_data.get_card_expiry_month_year_2_digit_with_delimiter("/".to_string())?;

        // Build cardholder name
        let cardholder = get_full_name(billing_first_name.clone(), billing_last_name);

        // Use zero amount for mandate setup verification
        let amount = item
            .connector
            .amount_converter
            .convert(MinorUnit::new(0), router_data.request.currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        Ok(Self {
            amount,
            currency: router_data.request.currency.to_string(),
            pan: card_data.card_number.clone(),
            cvv: card_data.card_cvc.clone(),
            expiry_date,
            cardholder,
            reference: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            redirect_url,
            billing_city: billing_city.peek().to_string(),
            billing_country,
            billing_street1,
            billing_postcode,
            customer_email,
            customer_ip_address,
            browser_accept_header: browser_info.get_accept_header()?,
            browser_language: browser_info.get_language()?,
            browser_screen_height: browser_info.get_screen_height()?.to_string(),
            browser_screen_width: browser_info.get_screen_width()?.to_string(),
            browser_timezone: browser_info.get_time_zone()?.to_string(),
            browser_user_agent: browser_info.get_user_agent()?,
            browser_java_enabled: browser_info.get_java_enabled()?.to_string(),
            browser_java_script_enabled: browser_info.get_java_script_enabled()?.to_string(),
            browser_screen_color_depth: browser_info.get_color_depth()?.to_string(),
            browser_challenge_window: CHALLENGE_WINDOW.to_string(),
            payment_action: Some("preauth".to_string()), // Use preauth for mandate setup
            payment_type: PAYMENT_TYPE.to_string(),
            // Mark as RecurringInitial so the returned InstanceId is chainable for MIT
            recurring_payment_type: PAYMENT_TYPE_RECURRING_INITIAL.to_string(),
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<TrustpaySetupMandateResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TrustpaySetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;

        // Get transaction status from payment status
        let (status, message) = get_transaction_status(
            response.payment_status.clone(),
            response.redirect_url.clone(),
        )?;

        // Build redirection data if redirect URL is present
        let form_fields = response.redirect_params.clone().unwrap_or_default();
        let redirection_data = response.redirect_url.clone().map(|url| RedirectForm::Form {
            endpoint: url.to_string(),
            method: Method::Post,
            form_fields,
        });

        // Build error response if there's a failure
        let error = if message.is_some() {
            Some(ErrorResponse {
                code: response
                    .payment_status
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: message
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: message,
                status_code: item.http_code,
                attempt_status: None,
                connector_transaction_id: Some(response.instance_id.clone()),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            })
        } else {
            None
        };

        // The instance_id serves as the connector_mandate_id for future recurring payments
        let mandate_reference = Some(Box::new(MandateReference {
            connector_mandate_id: Some(response.instance_id.clone()),
            payment_method_id: None,
            connector_mandate_request_reference_id: None,
        }));

        let payment_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(response.instance_id.clone()),
            redirection_data: redirection_data.map(Box::new),
            mandate_reference,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: None,
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response: error.map_or_else(|| Ok(payment_response_data), Err),
            ..item.router_data
        })
    }
}

// ===== RepeatPayment (recurring subsequent / MIT) structs =====

/// TrustPay RepeatPayment request — references a stored mandate (InstanceId) and
/// charges a subsequent MIT. Posted to `/api/v1/purchase` as form-url-encoded.
#[derive(Debug, Serialize, PartialEq)]
pub struct TrustpayRepeatPaymentRequest {
    pub amount: StringMajorUnit,
    pub currency: String,
    /// InstanceId of the initial preauth/setup — serves as the mandate reference.
    #[serde(rename = "InstanceId")]
    pub instance_id: String,
    pub reference: String,
    /// PaymentType marks this as a subsequent recurring charge.
    #[serde(rename = "PaymentType")]
    pub payment_type: String,
}

/// TrustPay RepeatPayment response — reuses the card payment response shape.
pub type TrustpayRepeatPaymentResponse = PaymentsResponseCards;

fn extract_trustpay_mandate_id(mandate_reference: &MandateReferenceId) -> Result<String, Error> {
    match mandate_reference {
        MandateReferenceId::ConnectorMandateId(connector_mandate_ref) => connector_mandate_ref
            .get_connector_mandate_id()
            .ok_or_else(|| {
                report!(IntegrationError::MissingRequiredField {
                    field_name: "connector_mandate_id",
                    context: Default::default(),
                })
            }),
        MandateReferenceId::NetworkMandateId(_) | MandateReferenceId::NetworkTokenWithNTI(_) => {
            Err(report!(IntegrationError::NotSupported {
                message: "Network mandate / NTI not supported for trustpay RepeatPayment"
                    .to_string(),
                connector: "trustpay",
                context: Default::default(),
            }))
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TrustpayRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for TrustpayRepeatPaymentRequest
{
    type Error = Error;

    fn try_from(
        item: TrustpayRouterData<
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
        let instance_id = extract_trustpay_mandate_id(&router_data.request.mandate_reference)?;

        let amount = item
            .connector
            .amount_converter
            .convert(
                router_data.request.minor_amount,
                router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        Ok(Self {
            amount,
            currency: router_data.request.currency.to_string(),
            instance_id,
            reference: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            payment_type: PAYMENT_TYPE_RECURRING.to_string(),
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<TrustpayRepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TrustpayRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;

        let (status, message) = get_transaction_status(
            response.payment_status.clone(),
            response.redirect_url.clone(),
        )?;

        let error = if message.is_some() {
            Some(ErrorResponse {
                code: response
                    .payment_status
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: message
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: message,
                status_code: item.http_code,
                attempt_status: None,
                connector_transaction_id: Some(response.instance_id.clone()),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            })
        } else {
            None
        };

        let payment_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(response.instance_id.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: None,
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response: error.map_or_else(|| Ok(payment_response_data), Err),
            ..item.router_data
        })
    }
}
