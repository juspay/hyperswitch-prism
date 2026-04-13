use common_enums::{self, AttemptStatus};
use common_utils::{consts, request::Method, types::MinorUnit, CustomerId};
use domain_types::{
    connector_flow::{Authorize, PSync, ServerAuthenticationToken},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundsData, RefundsResponseData, ResponseId,
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
    },
    payment_method_data::{BankRedirectData, PaymentMethodData, PaymentMethodDataTypes},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
    utils,
};
use hyperswitch_masking::{ExposeInterface, Secret};
use interfaces::webhooks::IncomingWebhookEvent;
use serde::{Deserialize, Serialize};

use crate::{connectors::volt::VoltRouterData, types::ResponseRouterData};
use domain_types::errors::{ConnectorError, IntegrationError};

// Type alias for refunds router data following existing patterns
pub type RefundsResponseRouterData<F, T> =
    ResponseRouterData<T, RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>>;

// Empty request type for PSync GET requests
#[derive(Debug, Serialize, Default)]
pub struct VoltPsyncRequest;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        VoltRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for VoltPsyncRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        _item: VoltRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self)
    }
}

fn get_attempt_status(item: VoltPaymentStatus) -> AttemptStatus {
    match item {
        VoltPaymentStatus::Received | VoltPaymentStatus::Settled => AttemptStatus::Charged,
        VoltPaymentStatus::Completed
        | VoltPaymentStatus::DelayedAtBank
        | VoltPaymentStatus::AuthorisedByUser
        | VoltPaymentStatus::ApprovedByRisk => AttemptStatus::Pending,
        VoltPaymentStatus::NewPayment
        | VoltPaymentStatus::BankRedirect
        | VoltPaymentStatus::AwaitingCheckoutAuthorisation
        | VoltPaymentStatus::AdditionalAuthorizationRequired => {
            AttemptStatus::AuthenticationPending
        }
        VoltPaymentStatus::RefusedByBank
        | VoltPaymentStatus::RefusedByRisk
        | VoltPaymentStatus::NotReceived
        | VoltPaymentStatus::ErrorAtBank
        | VoltPaymentStatus::CancelledByUser
        | VoltPaymentStatus::AbandonedByUser
        | VoltPaymentStatus::Failed
        | VoltPaymentStatus::ProviderCommunicationError => AttemptStatus::Failure,
        VoltPaymentStatus::Unknown => AttemptStatus::Unspecified,
    }
}

const PASSWORD: &str = "password";

pub mod webhook_headers {
    pub const X_VOLT_SIGNED: &str = "X-Volt-Signed";
    pub const X_VOLT_TIMED: &str = "X-Volt-Timed";
    pub const USER_AGENT: &str = "User-Agent";
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoltPaymentsRequest {
    amount: MinorUnit,
    currency: common_enums::Currency,
    #[serde(skip_serializing_if = "Option::is_none")]
    open_banking_u_k: Option<OpenBankingUk>,
    #[serde(skip_serializing_if = "Option::is_none")]
    open_banking_e_u: Option<OpenBankingEu>,
    internal_reference: String,
    payer: PayerDetails,
    payment_system: PaymentSystem,
    communication: CommunicationDetails,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransactionType {
    Bills,
    Goods,
    PersonToPerson,
    Other,
    Services,
}

#[derive(Debug, Serialize)]
pub struct OpenBankingUk {
    #[serde(rename = "type")]
    transaction_type: TransactionType,
}

#[derive(Debug, Serialize)]
pub struct OpenBankingEu {
    #[serde(rename = "type")]
    transaction_type: TransactionType,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PayerDetails {
    reference: CustomerId,
    email: Option<common_utils::pii::Email>,
    first_name: Secret<String>,
    last_name: Secret<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaymentSystem {
    OpenBankingEu,
    OpenBankingUk,
    NppPayToAu,
}

#[derive(Debug, Serialize)]
pub struct CommunicationDetails {
    #[serde[rename = "return"]]
    return_urls: ReturnUrls,
}

#[derive(Debug, Serialize)]
pub struct ReturnUrls {
    success: Link,
    failure: Link,
    pending: Link,
    cancel: Link,
}

#[derive(Debug, Serialize)]
pub struct Link {
    link: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        VoltRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for VoltPaymentsRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: VoltRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        match &item.router_data.request.payment_method_data {
            PaymentMethodData::BankRedirect(ref bank_redirect) => {
                let transaction_type = TransactionType::Services; //transaction_type is a form of enum, it is pre defined and value for this can not be taken from user so we are keeping it as Services as this transaction is type of service.
                let currency = item.router_data.request.currency;

                let (payment_system, open_banking_u_k, open_banking_e_u) = match bank_redirect {
                    BankRedirectData::OpenBankingUk { .. } => Ok((
                        PaymentSystem::OpenBankingUk,
                        Some(OpenBankingUk { transaction_type }),
                        None,
                    )),
                    BankRedirectData::OpenBanking {} => {
                        if matches!(currency, common_enums::Currency::GBP) {
                            Ok((
                                PaymentSystem::OpenBankingUk,
                                Some(OpenBankingUk { transaction_type }),
                                None,
                            ))
                        } else {
                            Ok((
                                PaymentSystem::OpenBankingEu,
                                None,
                                Some(OpenBankingEu { transaction_type }),
                            ))
                        }
                    }
                    BankRedirectData::BancontactCard { .. }
                    | BankRedirectData::Bizum {}
                    | BankRedirectData::Blik { .. }
                    | BankRedirectData::Eft { .. }
                    | BankRedirectData::Eps { .. }
                    | BankRedirectData::Giropay { .. }
                    | BankRedirectData::Ideal { .. }
                    | BankRedirectData::Interac { .. }
                    | BankRedirectData::OnlineBankingCzechRepublic { .. }
                    | BankRedirectData::OnlineBankingFinland { .. }
                    | BankRedirectData::OnlineBankingPoland { .. }
                    | BankRedirectData::OnlineBankingSlovakia { .. }
                    | BankRedirectData::Przelewy24 { .. }
                    | BankRedirectData::Sofort { .. }
                    | BankRedirectData::Trustly { .. }
                    | BankRedirectData::OnlineBankingFpx { .. }
                    | BankRedirectData::OnlineBankingThailand { .. }
                    | BankRedirectData::LocalBankRedirect {}
                    | BankRedirectData::Netbanking { .. } => {
                        Err(IntegrationError::not_implemented(
                            utils::get_unimplemented_payment_method_error_message("Volt"),
                        ))
                    }
                }?;

                let amount = item.router_data.request.amount;
                let internal_reference = item
                    .router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone();
                let communication = CommunicationDetails {
                    return_urls: ReturnUrls {
                        success: Link {
                            link: item.router_data.request.router_return_url.clone(),
                        },
                        failure: Link {
                            link: item.router_data.request.router_return_url.clone(),
                        },
                        pending: Link {
                            link: item.router_data.request.router_return_url.clone(),
                        },
                        cancel: Link {
                            link: item.router_data.request.router_return_url.clone(),
                        },
                    },
                };
                let address = item
                    .router_data
                    .resource_common_data
                    .get_billing_address()?;
                let first_name = address.get_first_name()?;
                let payer = PayerDetails {
                    email: item.router_data.request.get_optional_email(),
                    first_name: first_name.to_owned(),
                    last_name: address.get_last_name().unwrap_or(first_name).to_owned(),
                    reference: item
                        .router_data
                        .resource_common_data
                        .get_customer_id()?
                        .to_owned(),
                };

                Ok(Self {
                    amount,
                    currency,
                    internal_reference,
                    communication,
                    payer,
                    payment_system,
                    open_banking_u_k,
                    open_banking_e_u,
                })
            }
            PaymentMethodData::Card(_)
            | PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::Wallet(_)
            | PaymentMethodData::PayLater(_)
            | PaymentMethodData::BankDebit(_)
            | PaymentMethodData::BankTransfer(_)
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
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
                Err(IntegrationError::not_implemented(
                    utils::get_unimplemented_payment_method_error_message("Volt"),
                )
                .into())
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct VoltAuthUpdateRequest {
    grant_type: String,
    client_id: Secret<String>,
    client_secret: Secret<String>,
    username: Secret<String>,
    password: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for VoltAuthUpdateRequest {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        let auth = VoltAuthType::try_from(auth_type)?;
        Ok(Self {
            grant_type: PASSWORD.to_string(),
            username: auth.username,
            password: auth.password,
            client_id: auth.client_id,
            client_secret: auth.client_secret,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        VoltRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                PaymentFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for VoltAuthUpdateRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: VoltRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                PaymentFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&item.router_data.connector_config)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VoltAuthUpdateResponse {
    pub access_token: Secret<String>,
    pub token_type: String,
    pub expires_in: i64,
}

impl<F, T> TryFrom<ResponseRouterData<VoltAuthUpdateResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, ServerAuthenticationTokenResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<VoltAuthUpdateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(ServerAuthenticationTokenResponseData {
                access_token: item.response.access_token,
                expires_in: Some(item.response.expires_in),
                token_type: Some(item.response.token_type),
            }),
            ..item.router_data
        })
    }
}

pub struct VoltAuthType {
    pub(super) username: Secret<String>,
    pub(super) password: Secret<String>,
    pub(super) client_id: Secret<String>,
    pub(super) client_secret: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for VoltAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Volt {
                username,
                password,
                client_id,
                client_secret,
                ..
            } => Ok(Self {
                username: username.to_owned(),
                password: password.to_owned(),
                client_id: client_id.to_owned(),
                client_secret: client_secret.to_owned(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoltPaymentsResponse {
    id: String,
    amount: MinorUnit,
    currency: common_enums::Currency,
    status: VoltPaymentStatus,
    payment_initiation_flow: VoltPaymentInitiationFlow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoltPaymentInitiationFlow {
    status: VoltPaymentInitiationFlowStatus,
    details: VoltPaymentInitiationFlowDetails,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VoltPaymentInitiationFlowStatus {
    Processing,
    Finished,
    Aborted,
    Exception,
    WaitingForInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoltPaymentInitiationFlowDetails {
    reason: String,
    redirect: VoltRedirect,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoltRedirect {
    url: Secret<url::Url>,
    direct_url: Secret<url::Url>,
}

impl<F, T> TryFrom<ResponseRouterData<VoltPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<VoltPaymentsResponse, Self>) -> Result<Self, Self::Error> {
        let url = item
            .response
            .payment_initiation_flow
            .details
            .redirect
            .url
            .clone()
            .expose();
        let redirection_data = Some(RedirectForm::Form {
            endpoint: url.to_string(),
            method: Method::Get,
            form_fields: Default::default(),
        });
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::AuthenticationPending,
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: redirection_data.map(Box::new),
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.id),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

#[derive(Debug, Serialize, Clone, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[derive(strum::Display)]
pub enum VoltPaymentStatus {
    NewPayment,
    ApprovedByRisk,
    AdditionalAuthorizationRequired,
    AuthorisedByUser,
    ProviderCommunicationError,
    Completed,
    Received,
    NotReceived,
    BankRedirect,
    DelayedAtBank,
    AwaitingCheckoutAuthorisation,
    RefusedByBank,
    RefusedByRisk,
    ErrorAtBank,
    CancelledByUser,
    AbandonedByUser,
    Failed,
    Settled,
    Unknown,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum VoltPaymentsResponseData {
    PsyncResponse(VoltPsyncResponse),
    WebhookResponse(VoltPaymentWebhookObjectResource),
}

#[derive(Debug, Serialize, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoltPsyncResponse {
    status: VoltPaymentStatus,
    id: String,
    merchant_internal_reference: Option<String>,
    amount: MinorUnit,
    currency: common_enums::Currency,
}

impl<F, T> TryFrom<ResponseRouterData<VoltPaymentsResponseData, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<VoltPaymentsResponseData, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            VoltPaymentsResponseData::PsyncResponse(payment_response) => {
                let status = get_attempt_status(payment_response.status.clone());
                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        ..item.router_data.resource_common_data
                    },
                    response: if utils::is_payment_failure(status) {
                        Err(ErrorResponse {
                            code: payment_response.status.clone().to_string(),
                            message: payment_response.status.clone().to_string(),
                            reason: Some(payment_response.status.to_string()),
                            status_code: item.http_code,
                            attempt_status: Some(status),
                            connector_transaction_id: Some(payment_response.id),
                            network_advice_code: None,
                            network_decline_code: None,
                            network_error_message: None,
                        })
                    } else {
                        Ok(PaymentsResponseData::TransactionResponse {
                            resource_id: ResponseId::ConnectorTransactionId(
                                payment_response.id.clone(),
                            ),
                            redirection_data: None,
                            mandate_reference: None,
                            connector_metadata: None,
                            network_txn_id: None,
                            connector_response_reference_id: payment_response
                                .merchant_internal_reference
                                .or(Some(payment_response.id)),
                            incremental_authorization_allowed: None,
                            status_code: item.http_code,
                        })
                    },
                    ..item.router_data
                })
            }
            VoltPaymentsResponseData::WebhookResponse(webhook_response) => {
                let detailed_status = webhook_response.detailed_status.clone();
                let status = AttemptStatus::from(webhook_response.status);
                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        ..item.router_data.resource_common_data
                    },
                    response: if utils::is_payment_failure(status) {
                        Err(ErrorResponse {
                            code: detailed_status
                                .clone()
                                .map(|volt_status| volt_status.to_string())
                                .unwrap_or_else(|| consts::NO_ERROR_CODE.to_owned()),
                            message: detailed_status
                                .clone()
                                .map(|volt_status| volt_status.to_string())
                                .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_owned()),
                            reason: detailed_status
                                .clone()
                                .map(|volt_status| volt_status.to_string()),
                            status_code: item.http_code,
                            attempt_status: Some(status),
                            connector_transaction_id: Some(webhook_response.payment.clone()),
                            network_advice_code: None,
                            network_decline_code: None,
                            network_error_message: None,
                        })
                    } else {
                        Ok(PaymentsResponseData::TransactionResponse {
                            resource_id: ResponseId::ConnectorTransactionId(
                                webhook_response.payment.clone(),
                            ),
                            redirection_data: None,
                            mandate_reference: None,
                            connector_metadata: None,
                            network_txn_id: None,
                            connector_response_reference_id: webhook_response
                                .merchant_internal_reference
                                .or(Some(webhook_response.payment)),
                            incremental_authorization_allowed: None,
                            status_code: item.http_code,
                        })
                    },
                    ..item.router_data
                })
            }
        }
    }
}
impl From<VoltWebhookPaymentStatus> for AttemptStatus {
    fn from(status: VoltWebhookPaymentStatus) -> Self {
        match status {
            VoltWebhookPaymentStatus::Received => Self::Charged,
            VoltWebhookPaymentStatus::Failed | VoltWebhookPaymentStatus::NotReceived => {
                Self::Failure
            }
            VoltWebhookPaymentStatus::Completed | VoltWebhookPaymentStatus::Pending => {
                Self::Pending
            }
        }
    }
}

// REFUND :
// Type definition for RefundRequest
#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoltRefundRequest {
    pub amount: MinorUnit,
    pub external_reference: String,
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<VoltRouterData<RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>, T>>
    for VoltRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: VoltRouterData<RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            amount: item.router_data.request.minor_refund_amount,
            external_reference: item.router_data.request.refund_id.clone(),
        })
    }
}

#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct RefundResponse {
    id: String,
}

impl<F> TryFrom<RefundsResponseRouterData<F, RefundResponse>>
    for RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: RefundsResponseRouterData<F, RefundResponse>) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id.to_string(),
                refund_status: common_enums::RefundStatus::Pending, //We get Refund Status only by Webhooks
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

#[derive(Debug, Deserialize, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoltPaymentWebhookBodyReference {
    pub payment: String,
    pub merchant_internal_reference: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoltRefundWebhookBodyReference {
    pub refund: String,
    pub external_reference: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum WebhookResponse {
    // the enum order shouldn't be changed as this is being used during serialization and deserialization
    Refund(VoltRefundWebhookBodyReference),
    Payment(VoltPaymentWebhookBodyReference),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum VoltWebhookBodyEventType {
    Payment(VoltPaymentsWebhookBodyEventType),
    Refund(VoltRefundsWebhookBodyEventType),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoltPaymentsWebhookBodyEventType {
    pub status: VoltWebhookPaymentStatus,
    pub detailed_status: Option<VoltDetailedStatus>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoltRefundsWebhookBodyEventType {
    pub status: VoltWebhookRefundsStatus,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum VoltWebhookObjectResource {
    Payment(VoltPaymentWebhookObjectResource),
    Refund(VoltRefundWebhookObjectResource),
}

#[derive(Debug, Deserialize, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoltPaymentWebhookObjectResource {
    #[serde(alias = "id")]
    pub payment: String,
    pub merchant_internal_reference: Option<String>,
    pub status: VoltWebhookPaymentStatus,
    pub detailed_status: Option<VoltDetailedStatus>,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoltRefundWebhookObjectResource {
    pub refund: String,
    pub external_reference: Option<String>,
    pub status: VoltWebhookRefundsStatus,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VoltWebhookPaymentStatus {
    Completed,
    Failed,
    Pending,
    Received,
    NotReceived,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VoltWebhookRefundsStatus {
    RefundConfirmed,
    RefundFailed,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[derive(strum::Display)]
pub enum VoltDetailedStatus {
    RefusedByRisk,
    RefusedByBank,
    ErrorAtBank,
    CancelledByUser,
    AbandonedByUser,
    Failed,
    Completed,
    BankRedirect,
    DelayedAtBank,
    AwaitingCheckoutAuthorisation,
}

impl From<VoltWebhookBodyEventType> for IncomingWebhookEvent {
    fn from(status: VoltWebhookBodyEventType) -> Self {
        match status {
            VoltWebhookBodyEventType::Payment(payment_data) => match payment_data.status {
                VoltWebhookPaymentStatus::Received => Self::PaymentIntentSuccess,
                VoltWebhookPaymentStatus::Failed | VoltWebhookPaymentStatus::NotReceived => {
                    Self::PaymentIntentFailure
                }
                VoltWebhookPaymentStatus::Completed | VoltWebhookPaymentStatus::Pending => {
                    Self::PaymentIntentProcessing
                }
            },
            VoltWebhookBodyEventType::Refund(refund_data) => match refund_data.status {
                VoltWebhookRefundsStatus::RefundConfirmed => Self::RefundSuccess,
                VoltWebhookRefundsStatus::RefundFailed => Self::RefundFailure,
            },
        }
    }
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct VoltErrorResponse {
    pub code: Option<String>,
    pub message: String,
    pub errors: Option<Vec<Errors>>,
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Errors {
    #[serde(rename = "type")]
    pub error_type: String,
    pub property_path: String,
    pub message: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct VoltAuthErrorResponse {
    pub code: u64,
    pub message: String,
}
