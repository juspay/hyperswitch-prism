use common_enums::{self, AttemptStatus, CountryAlpha2, RefundStatus};
use common_utils::{consts, pii, types::MinorUnit};
use domain_types::{
    connector_flow::{Authorize, RSync, Refund, RepeatPayment},
    connector_types::{
        EventType, MandateReference, PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        RepeatPaymentData, ResponseId,
    },
    errors,
    payment_address::AddressDetails,
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    utils::is_payment_failure,
};
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};
use strum::Display;

use crate::{connectors::givepayments::GivepaymentsRouterData, types::ResponseRouterData, utils};

pub struct GivepaymentsAuthType {
    pub(super) api_key: Secret<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GivepaymentsErrorResponse {
    #[serde(rename = "type")]
    pub error_type: GivepaymentsErrorType,
    pub code: GivepaymentsErrorCode,
    pub message: String,
    pub input: Option<Vec<GivePaymentsInputSpecificError>>,
    pub payment: Option<GivepaymentsPaymentProcessingError>,
}

#[derive(Debug, Display, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GivepaymentsErrorType {
    ClientError,
    AuthError,
    UserError,
    RiskError,
    PaymentError,
    ServiceError,
    ServerError,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Display, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GivepaymentsErrorCode {
    BadRequest,
    IncorrectCredentials,
    EmailVerificationRequired,
    NotAuthenticated,
    NotAuthorized,
    NotAllowed,
    InvalidInput,
    InvalidResource,
    ResourceNotFound,
    ResourceConflict,
    ResourceLimit,
    ResourceLocked,
    FraudBlock,
    PaymentIssue,
    ServiceUnavailable,
    ServiceIssue,
    ServerIssue,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GivePaymentsInputSpecificError {
    #[serde(rename = "type")]
    pub input_error_type: InputErrorType,
    pub code: InputErrorCode,
    pub param: String,
    pub message: String,
}

#[derive(Debug, Display, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum InputErrorType {
    PathError,
    QueryError,
    HeaderError,
    BodyError,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Display, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum InputErrorCode {
    Missing,
    Invalid,
    Duplicate,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GivepaymentsPaymentProcessingError {
    pub code: GivepaymentsPaymentProcessingErrorCode,
    pub message: String,
}

#[derive(Debug, Display, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GivepaymentsPaymentProcessingErrorCode {
    ProcessingError,
    ServiceNotAvailable,
    GenericDecline,
    AccountNotFound,
    InvalidAccount,
    ContactIssuer,
    ContactAcquirer,
    InvalidMerchant,
    NoActionTaken,
    ActionNotAllowed,
    ActionNotSupported,
    DoNotHonor,
    InsufficientFunds,
    DuplicateTransaction,
    ReenterTransaction,
    InvalidTransaction,
    InvalidAmount,
    InvalidIssuer,
    InvalidNumber,
    InvalidCvv,
    InvalidExpiry,
    InvalidAddress,
    InvalidCard,
    PickupCard,
    LostCard,
    StolenCard,
    ExpiredCard,
    RestrictedCard,
    IncorrectPin,
    PinTryExceeded,
    PinError,
    SuspectedFraud,
    BankAuthRequired,
    SecurityViolation,
    AmountLimitExceeded,
    RateLimitExceeded,
    Timeout,
    CardNetworkIssue,
    UnknownIssue,
    #[serde(other)]
    Unknown,
}

impl TryFrom<&ConnectorSpecificConfig> for GivepaymentsAuthType {
    type Error = error_stack::Report<errors::IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Givepayments {
                api_key,
                ..
            } => {
                Ok(Self {
                    api_key: api_key.to_owned(),
                })
            }
            _ => Err(errors::IntegrationError::FailedToObtainAuthType {
                context: errors::IntegrationErrorContext {
                    suggested_action: Some("Provide AuthType as HeaderKey".to_string()),
                    doc_url: Some("https://developer.givepayments.com/merchant/api/general/authentication/api-keys".to_string()),
                    additional_context: Some(
                        "Provided AuthType is incorrect. AuthType should be HeaderKey.".to_string(),
                    ),
                },
            }
            .into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GivepaymentsPaymentsRequestData<T: PaymentMethodDataTypes> {
    amount: MinorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    paymethod: GivepaymentsPaymentDetails<T>,
    customer: GivepaymentsCustomerDetails,
    #[serde(skip_serializing_if = "Option::is_none")]
    fee_payer: Option<ServiceFeePayer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    external_reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    send_receipt: Option<bool>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GivepaymentsPaymentDetails<T: PaymentMethodDataTypes> {
    #[serde(flatten)]
    payment_method: GivepaymentsPaymentMethod<T>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(untagged)]
pub enum GivepaymentsPaymentMethod<T: PaymentMethodDataTypes> {
    Card {
        card: CardDetails<T>,
        #[serde(skip_serializing_if = "Option::is_none")]
        billing_address: Box<Option<GivepaymentsAddressDetails>>,
    },
    CardToken {
        card_token: CardTokenDetails,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CardDetails<T: PaymentMethodDataTypes> {
    name: Secret<String>,
    number: RawCardNumber<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cvv: Option<Secret<String>>,
    exp_year: Secret<i32>,
    exp_month: Secret<i8>,
    tokenize: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CardTokenDetails {
    id: Secret<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GivepaymentsAddressDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<Secret<String>>,
    line1: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line2: Option<Secret<String>>,
    city: Secret<String>,
    state: Secret<String>,
    zip: Secret<String>,
    country: CountryAlpha2,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct GivepaymentsCustomerDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Secret<String>>,
    email: pii::Email,
    #[serde(skip_serializing_if = "Option::is_none")]
    phone: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    first_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    company_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_address: Option<GivepaymentsAddressDetails>,
    #[serde(skip_serializing_if = "Option::is_none")]
    email_verification_code: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum ServiceFeePayer {
    Customer,
    Merchant,
}

trait GetEmail {
    fn get_email(&self) -> Result<pii::Email, error_stack::Report<errors::IntegrationError>>;
}

impl<T> GetEmail for PaymentsAuthorizeData<T>
where
    T: PaymentMethodDataTypes,
{
    fn get_email(&self) -> Result<pii::Email, error_stack::Report<errors::IntegrationError>> {
        self.get_email()
    }
}

impl<T> GetEmail for RepeatPaymentData<T>
where
    T: PaymentMethodDataTypes,
{
    fn get_email(&self) -> Result<pii::Email, error_stack::Report<errors::IntegrationError>> {
        self.get_email()
    }
}

impl TryFrom<&AddressDetails> for GivepaymentsAddressDetails {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(address: &AddressDetails) -> Result<Self, Self::Error> {
        Ok(Self {
            id: None,
            name: address.get_optional_full_name(),
            line1: address.get_line1()?.clone(),
            line2: address.get_optional_line2(),
            city: address.get_city()?.clone(),
            state: address.get_state()?.clone(),
            zip: address.get_zip()?.clone(),
            country: *address.get_country()?,
        })
    }
}

fn get_billing_details<F, Req>(
    item: &RouterDataV2<F, PaymentFlowData, Req, PaymentsResponseData>,
) -> Result<Option<GivepaymentsAddressDetails>, error_stack::Report<errors::IntegrationError>> {
    item.resource_common_data
        .get_optional_payment_billing()
        .and_then(|billing| billing.address.as_ref())
        .filter(|address| {
            address.line1.is_some()
                && address.city.is_some()
                && address.state.is_some()
                && address.zip.is_some()
                && address.country.is_some()
        })
        .map(GivepaymentsAddressDetails::try_from)
        .transpose()
}

fn get_shipping_details<F, Req>(
    item: &RouterDataV2<F, PaymentFlowData, Req, PaymentsResponseData>,
) -> Result<Option<GivepaymentsAddressDetails>, error_stack::Report<errors::IntegrationError>> {
    item.resource_common_data
        .get_optional_shipping()
        .and_then(|shipping| shipping.address.as_ref())
        .filter(|address| {
            address.line1.is_some()
                && address.city.is_some()
                && address.state.is_some()
                && address.zip.is_some()
                && address.country.is_some()
        })
        .map(GivepaymentsAddressDetails::try_from)
        .transpose()
}

fn get_customer_details<F, Req>(
    item: &RouterDataV2<F, PaymentFlowData, Req, PaymentsResponseData>,
) -> Result<GivepaymentsCustomerDetails, error_stack::Report<errors::IntegrationError>>
where
    Req: GetEmail,
{
    Ok(GivepaymentsCustomerDetails {
        id: None,
        email: item
            .resource_common_data
            .get_billing_email()
            .or_else(|_| item.request.get_email())?,
        phone: item
            .resource_common_data
            .get_optional_billing_phone_number(),
        first_name: item.resource_common_data.get_optional_billing_first_name(),
        last_name: item.resource_common_data.get_optional_billing_last_name(),
        company_name: None,
        shipping_address: get_shipping_details(item)?,
        email_verification_code: None,
    })
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GivepaymentsRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for GivepaymentsPaymentsRequestData<T>
{
    type Error = error_stack::Report<errors::IntegrationError>;
    fn try_from(
        item: GivepaymentsRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        if !item.router_data.request.is_auto_capture() {
            Err(errors::IntegrationError::CaptureMethodNotSupported {
                context: errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Use Automatic capture method for Givepayments connector.".to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "Givepayments connector supports Automatic capture method.".to_string(),
                    ),
                },
            })?
        }

        let paymethod = match &item.router_data.request.payment_method_data {
            PaymentMethodData::Card(ref card_data) => {
                let card = CardDetails {
                    name: card_data.get_cardholder_name()?,
                    number: card_data.card_number.clone(),
                    cvv: Some(card_data.card_cvc.clone()),
                    exp_year: card_data.get_expiry_year_as_i32()?,
                    exp_month: card_data.get_expiry_month_as_i8()?,
                    tokenize: item.router_data.request.is_customer_initiated_mandate_payment(),
                };

                let billing_address = get_billing_details(&item.router_data)?;

                GivepaymentsPaymentDetails {
                    payment_method: GivepaymentsPaymentMethod::Card {
                        card,
                        billing_address: Box::new(billing_address),
                    },
                }
            }
            PaymentMethodData::Wallet(_)
            | PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::CardWithNoCvc(_)
            | PaymentMethodData::BankRedirect(_)
            | PaymentMethodData::BankTransfer(_)
            | PaymentMethodData::BankDebit(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::PayLater(_)
            | PaymentMethodData::Crypto(_)
            | PaymentMethodData::MandatePayment
            | PaymentMethodData::Reward
            | PaymentMethodData::RealTimePayment(_)
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::MobilePayment(_)
            | PaymentMethodData::PaymentMethodToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_) => {
                Err(errors::IntegrationError::NotImplemented(
                    utils::get_unimplemented_payment_method_error_message("Givepayments"),
                    errors::IntegrationErrorContext {
                        suggested_action: Some("Use a payment method supported by the Givepayments connector or choose a different connector that supports the requested payment method.".to_string()),
                        doc_url: None,
                        additional_context: Some("The requested payment method is not currently supported by the Givepayments connector implementation.".to_string()),
                    }
                ))?
            }
        };

        let customer = get_customer_details(&item.router_data)?;

        Ok(Self {
            amount: item.router_data.request.amount,
            description: item.router_data.resource_common_data.description,
            paymethod,
            customer,
            fee_payer: None,
            external_reference: item.router_data.resource_common_data.reference_id.clone(),
            metadata: None,
            send_receipt: None,
        })
    }
}

pub type GivepaymentsPaymentResponseData =
    GivepaymentsResponseData<GivepaymentsPaymentStatus, GivepaymentsPaymentProcessingState>;

pub type GivepaymentsRefundResponseData =
    GivepaymentsResponseData<GivepaymentsRefundStatus, GivepaymentsRefundProcessingState>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GivepaymentsResponseData<S, P> {
    pub id: String,
    pub status: S,
    pub processing_state: P,
    total_amount: MinorUnit,
    net_amount: MinorUnit,
    fee_amount: MinorUnit,
    fees_paid_by: Option<ServiceFeePayer>,
    reversal_status: Option<GivepaymentsReversalStatus>,
    billing_descriptor: Option<String>,
    description: Option<String>,
    pub external_reference: Option<String>,
    metadata: Option<serde_json::Value>,
    #[serde(alias = "cancel_reason")]
    void_reason: Option<String>,
    pub paymethod_token: Option<PayMethodTokenDetails>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GivepaymentsPaymentStatus {
    Pending,
    Failed,
    Voided,
    Successful,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GivepaymentsPaymentProcessingState {
    Created,
    Failed,
    Voided,
    Declined,
    Authorized,
    Captured,
    Settled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GivepaymentsRefundStatus {
    Pending,
    Failed,
    Canceled,
    Successful,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GivepaymentsRefundProcessingState {
    Created,
    Pending,
    Failed,
    Declined,
    Canceled,
    Approved,
    Settled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PayMethodTokenDetails {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum GivepaymentsReversalStatus {
    PartiallyReversed,
    FullyReversed,
    #[serde(other)]
    Unknown,
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<GivepaymentsPaymentResponseData, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(
        item: ResponseRouterData<GivepaymentsPaymentResponseData, Self>,
    ) -> Result<Self, Self::Error> {
        let status = item.response.processing_state.clone().into();

        if is_payment_failure(status) {
            let error_response = ErrorResponse {
                code: consts::NO_ERROR_CODE.to_string(),
                message: consts::NO_ERROR_MESSAGE.to_string(),
                reason: None,
                status_code: item.http_code,
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: Some(item.response.id),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            };

            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                response: Err(error_response),
                ..item.router_data
            })
        } else {
            let connector_mandate_id = item
                .response
                .paymethod_token
                .as_ref()
                .map(|data| data.id.clone());

            let mandate_reference = connector_mandate_id.map(|mandate_id| {
                Box::new(MandateReference {
                    connector_mandate_id: Some(mandate_id),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                    mandate_metadata: None,
                })
            });

            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                    redirection_data: None,
                    mandate_reference,
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: Some(item.response.id.clone()),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                    splits: None,
                }),
                ..item.router_data
            })
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GivepaymentsRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for GivepaymentsPaymentsRequestData<T>
{
    type Error = error_stack::Report<errors::IntegrationError>;
    fn try_from(
        item: GivepaymentsRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let card_token_data = item
            .router_data
            .request
            .connector_mandate_id()
            .ok_or_else(|| errors::IntegrationError::MissingRequiredField {
                field_name: "connector_mandate_id",
                context: errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Pass a valid `connector_mandate_id` for recurring or mandate payments."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some("Expected connector mandate ID not found".to_string()),
                },
            })?;

        let paymethod = GivepaymentsPaymentDetails {
            payment_method: GivepaymentsPaymentMethod::CardToken {
                card_token: CardTokenDetails {
                    id: Secret::new(card_token_data),
                },
            },
        };

        let customer = get_customer_details(&item.router_data)?;

        Ok(Self {
            amount: item.router_data.request.minor_amount,
            description: item.router_data.resource_common_data.description,
            paymethod,
            customer,
            fee_payer: None,
            external_reference: item.router_data.resource_common_data.reference_id.clone(),
            metadata: None,
            send_receipt: None,
        })
    }
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<GivepaymentsPaymentResponseData, Self>>
    for RouterDataV2<F, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(
        item: ResponseRouterData<GivepaymentsPaymentResponseData, Self>,
    ) -> Result<Self, Self::Error> {
        let status = item.response.processing_state.clone().into();

        if is_payment_failure(status) {
            let error_response = ErrorResponse {
                code: consts::NO_ERROR_CODE.to_string(),
                message: consts::NO_ERROR_MESSAGE.to_string(),
                reason: None,
                status_code: item.http_code,
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: Some(item.response.id),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            };

            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                response: Err(error_response),
                ..item.router_data
            })
        } else {
            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: Some(item.response.id.clone()),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                    splits: None,
                }),
                ..item.router_data
            })
        }
    }
}

impl<F> TryFrom<ResponseRouterData<GivepaymentsPaymentResponseData, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(
        item: ResponseRouterData<GivepaymentsPaymentResponseData, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;

        let status = response.processing_state.clone().into();

        if is_payment_failure(status) {
            let error_response = ErrorResponse {
                code: consts::NO_ERROR_CODE.to_string(),
                message: consts::NO_ERROR_MESSAGE.to_string(),
                reason: None,
                status_code: http_code,
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: Some(response.id),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            };

            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..router_data.resource_common_data
                },
                response: Err(error_response),
                ..router_data
            })
        } else {
            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..router_data.resource_common_data
                },
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: Some(response.id.clone()),
                    incremental_authorization_allowed: None,
                    status_code: http_code,
                    splits: None,
                }),
                ..router_data
            })
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GivepaymentsRefundRequestData {
    payment: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    amount: Option<MinorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    external_reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    notify_customer: Option<bool>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GivepaymentsRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for GivepaymentsRefundRequestData
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: GivepaymentsRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            payment: item.router_data.request.connector_transaction_id.clone(),
            amount: Some(item.router_data.request.minor_refund_amount),
            reason: item.router_data.request.reason.clone(),
            description: item.router_data.request.reason,
            external_reference: Some(item.router_data.request.refund_id.clone()),
            metadata: None,
            notify_customer: None,
        })
    }
}

impl TryFrom<ResponseRouterData<GivepaymentsRefundResponseData, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GivepaymentsRefundResponseData, Self>,
    ) -> Result<Self, Self::Error> {
        let refund_status = item.response.processing_state.into();

        if utils::is_refund_failure(refund_status) {
            let error_response = Err(ErrorResponse {
                status_code: item.http_code,
                code: consts::NO_ERROR_CODE.to_string(),
                message: consts::NO_ERROR_MESSAGE.to_string(),
                reason: None,
                attempt_status: Some(FlowStatus::Refund(refund_status)),
                connector_transaction_id: None,
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            });

            Ok(Self {
                response: error_response,
                ..item.router_data
            })
        } else {
            Ok(Self {
                response: Ok(RefundsResponseData {
                    connector_refund_id: item.response.id,
                    refund_status,
                    status_code: item.http_code,
                    refund_arn: None,
                }),
                ..item.router_data
            })
        }
    }
}

impl TryFrom<ResponseRouterData<GivepaymentsRefundResponseData, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(
        item: ResponseRouterData<GivepaymentsRefundResponseData, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;

        let refund_status = response.processing_state.clone().into();

        if utils::is_refund_failure(refund_status) {
            let error_response = Err(ErrorResponse {
                status_code: http_code,
                code: consts::NO_ERROR_CODE.to_string(),
                message: consts::NO_ERROR_MESSAGE.to_string(),
                reason: None,
                attempt_status: Some(FlowStatus::Refund(refund_status)),
                connector_transaction_id: None,
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            });

            Ok(Self {
                response: error_response,
                ..router_data
            })
        } else {
            Ok(Self {
                response: Ok(RefundsResponseData {
                    connector_refund_id: response.id,
                    refund_status,
                    status_code: http_code,
                    refund_arn: None,
                }),
                ..router_data
            })
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GivepaymentsIncomingWebhookData {
    id: String,
    #[serde(rename = "type")]
    pub event_type: GivepaymentsWebhookEventType,
    pub data: GivepaymentsWebhookData,
    merchant: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GivepaymentsWebhookEventType {
    #[serde(rename = "payment.created")]
    PaymentCreated,
    #[serde(rename = "payment.failed")]
    PaymentFailed,
    #[serde(rename = "payment.voided")]
    PaymentVoided,
    #[serde(rename = "payment.declined")]
    PaymentDeclined,
    #[serde(rename = "payment.authorized")]
    PaymentAuthorized,
    #[serde(rename = "payment.captured")]
    PaymentCaptured,
    #[serde(rename = "payment.settled")]
    PaymentSettled,
    #[serde(rename = "refund.created")]
    RefundCreated,
    #[serde(rename = "refund.pending")]
    RefundPending,
    #[serde(rename = "refund.failed")]
    RefundFailed,
    #[serde(rename = "refund.declined")]
    RefundDeclined,
    #[serde(rename = "refund.canceled")]
    RefundCanceled,
    #[serde(rename = "refund.approved")]
    RefundApproved,
    #[serde(rename = "refund.settled")]
    RefundSettled,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "object", rename_all = "snake_case")]
pub enum GivepaymentsWebhookData {
    Payment(GivepaymentsPaymentResponseData),
    Refund(GivepaymentsRefundResponseData),
}

impl From<GivepaymentsPaymentProcessingState> for AttemptStatus {
    fn from(item: GivepaymentsPaymentProcessingState) -> Self {
        match item {
            GivepaymentsPaymentProcessingState::Created
            | GivepaymentsPaymentProcessingState::Authorized => Self::Pending,

            GivepaymentsPaymentProcessingState::Voided => Self::Voided,

            GivepaymentsPaymentProcessingState::Captured
            | GivepaymentsPaymentProcessingState::Settled => Self::Charged,

            GivepaymentsPaymentProcessingState::Failed
            | GivepaymentsPaymentProcessingState::Declined => Self::Failure,
        }
    }
}

impl From<GivepaymentsRefundProcessingState> for RefundStatus {
    fn from(status: GivepaymentsRefundProcessingState) -> Self {
        match status {
            GivepaymentsRefundProcessingState::Created
            | GivepaymentsRefundProcessingState::Pending => Self::Pending,

            GivepaymentsRefundProcessingState::Approved
            | GivepaymentsRefundProcessingState::Settled => Self::Success,

            GivepaymentsRefundProcessingState::Failed
            | GivepaymentsRefundProcessingState::Declined
            | GivepaymentsRefundProcessingState::Canceled => Self::Failure,
        }
    }
}

impl From<GivepaymentsWebhookEventType> for EventType {
    fn from(event_type: GivepaymentsWebhookEventType) -> Self {
        match event_type {
            GivepaymentsWebhookEventType::PaymentCreated
            | GivepaymentsWebhookEventType::PaymentAuthorized => Self::PaymentIntentProcessing,

            GivepaymentsWebhookEventType::PaymentFailed
            | GivepaymentsWebhookEventType::PaymentDeclined => Self::PaymentIntentCaptureFailure,

            GivepaymentsWebhookEventType::PaymentVoided => Self::PaymentIntentCancelled,

            GivepaymentsWebhookEventType::PaymentCaptured
            | GivepaymentsWebhookEventType::PaymentSettled => Self::PaymentIntentCaptureSuccess,

            GivepaymentsWebhookEventType::RefundCreated
            | GivepaymentsWebhookEventType::RefundPending => Self::RefundProcessing,

            GivepaymentsWebhookEventType::RefundFailed
            | GivepaymentsWebhookEventType::RefundDeclined
            | GivepaymentsWebhookEventType::RefundCanceled => Self::RefundFailure,

            GivepaymentsWebhookEventType::RefundApproved
            | GivepaymentsWebhookEventType::RefundSettled => Self::RefundSuccess,

            GivepaymentsWebhookEventType::Unknown => Self::IncomingWebhookEventUnspecified,
        }
    }
}
