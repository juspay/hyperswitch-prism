use crate::connectors::revolut::RevolutRouterData;
use domain_types::{
    connector_flow::{Authorize, Capture, IncrementalAuthorization, PSync, Refund},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsIncrementalAuthorizationData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, ResponseId,
        WebhookDetailsResponse,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::PaymentMethodDataTypes,
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};

use crate::types::ResponseRouterData;
use common_enums::AttemptStatus;
use common_utils::{
    custom_serde,
    types::{MinorUnit, Money},
};
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use time::PrimitiveDateTime;

pub struct RevolutAuthType {
    pub secret_api_key: Secret<String>,
    pub signing_secret: Option<Secret<String>>,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
pub struct RevolutOrderCreateRequest {
    pub amount: MinorUnit,
    pub currency: common_enums::Currency,
    pub settlement_currency: Option<String>,
    pub description: Option<String>,
    pub customer: Option<RevolutCustomer>,
    pub enforce_challenge: Option<RevolutEnforceChallengeMode>,
    pub line_items: Option<Vec<RevolutLineItem>>,
    pub shipping: Option<RevolutShipping>,
    pub capture_mode: Option<RevolutCaptureMode>,
    pub cancel_authorised_after: Option<String>,
    pub location_id: Option<String>,
    pub metadata: Option<Secret<serde_json::Value>>,
    pub industry_data: Option<Secret<serde_json::Value>>,
    pub merchant_order_data: Option<RevolutMerchantOrderData>,
    pub upcoming_payment_data: Option<serde_json::Value>,
    pub redirect_url: Option<String>,
    pub statement_descriptor_suffix: Option<String>,
    /// Authorisation type for the order. Use `pre_authorisation` to enable
    /// extended clearing windows and incremental authorisation on the order.
    pub authorisation_type: Option<RevolutAuthorisationType>,
}

/// Revolut order authorisation type. Use `PreAuthorisation` to enable
/// incremental authorisation on the order.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevolutAuthorisationType {
    FinalAuthorisation,
    PreAuthorisation,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutCustomer {
    pub id: Option<String>,
    pub full_name: Option<Secret<String>>,
    pub phone: Option<Secret<String>>,
    pub email: Option<common_utils::pii::Email>,
    pub date_of_birth: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RevolutEnforceChallengeMode {
    #[default]
    Automatic,
    Forced,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutLineItem {
    pub name: String,
    pub r#type: RevolutLineItemType,
    pub quantity: RevolutLineItemQuantity,
    pub unit_price_amount: MinorUnit, //integer(int64)
    pub total_amount: MinorUnit,      //integer(int64)
    pub external_id: Option<String>,
    pub discounts: Option<Vec<RevolutLineItemDiscount>>,
    pub taxes: Option<Vec<RevolutLineItemTax>>,
    pub image_urls: Option<Vec<String>>,
    pub description: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RevolutLineItemType {
    Physical,
    Service,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutLineItemQuantity {
    pub value: f64, // number(double)
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutLineItemDiscount {
    pub name: String,
    pub amount: MinorUnit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutLineItemTax {
    pub name: String,
    pub amount: MinorUnit,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutShipping {
    pub address: Option<RevolutAddress>,
    pub contact: Option<RevolutContact>,
    pub shipments: Option<Vec<RevolutShipment>>,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutAddress {
    pub street_line_1: Option<String>,
    pub street_line_2: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
    pub country_code: Option<String>,
    pub postcode: Option<String>,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutContact {
    pub full_name: Option<Secret<String>>,
    pub phone: Option<Secret<String>>,
    pub email: Option<common_utils::pii::Email>,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutShipment {
    pub shipping_company_name: String,
    pub tracking_number: String,
    pub estimated_delivery_date: Option<String>,
    pub tracking_url: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum RevolutErrorResponse {
    StandardError {
        code: String,
        message: String,
        timestamp: i64,
    },
    ErrorIdResponse {
        #[serde(rename = "errorId")]
        error_id: String,
        timestamp: i64,
        code: Option<i64>,
    },
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RevolutOrderCreateResponse {
    pub id: String,
    pub token: Secret<String>,
    pub r#type: RevolutOrderType,
    pub state: RevolutOrderState,
    #[serde(with = "custom_serde::iso8601")]
    pub created_at: PrimitiveDateTime,
    #[serde(with = "custom_serde::iso8601")]
    pub updated_at: PrimitiveDateTime,
    pub description: Option<String>,
    pub capture_mode: Option<RevolutCaptureMode>,
    pub cancel_authorised_after: Option<String>,
    pub amount: MinorUnit,
    pub outstanding_amount: Option<MinorUnit>,
    pub refunded_amount: Option<MinorUnit>,
    pub currency: common_enums::Currency,
    pub settlement_currency: Option<String>,
    pub customer: Option<RevolutCustomer>,
    pub payments: Option<Vec<RevolutPayment>>,
    pub location_id: Option<String>,
    pub metadata: Option<Secret<serde_json::Value>>,
    pub industry_data: Option<Secret<serde_json::Value>>,
    pub merchant_order_data: Option<RevolutMerchantOrderData>,
    pub upcoming_payment_data: Option<RevolutUpcomingPaymentData>,
    pub checkout_url: Option<String>,
    pub redirect_url: Option<String>,
    pub shipping: Option<RevolutShipping>,
    pub enforce_challenge: Option<RevolutEnforceChallengeMode>,
    pub line_items: Option<Vec<RevolutLineItem>>,
    pub statement_descriptor_suffix: Option<String>,
    pub related_order_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevolutOrderType {
    Payment,
    PaymentRequest,
    Refund,
    Chargeback,
    ChargebackReversal,
    CreditReimbursement,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevolutOrderState {
    Pending,
    Processing,
    Authorised,
    Completed,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RevolutCaptureMode {
    Automatic,
    Manual,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutPayment {
    pub id: String,
    pub state: RevolutPaymentState,
    pub decline_reason: Option<RevolutDeclineReason>,
    pub bank_message: Option<String>,
    #[serde(with = "custom_serde::iso8601")]
    pub created_at: PrimitiveDateTime,
    #[serde(with = "custom_serde::iso8601")]
    pub updated_at: PrimitiveDateTime,
    pub token: Option<String>,
    pub amount: MinorUnit,
    pub currency: common_enums::Currency,
    pub settled_amount: Option<MinorUnit>,
    pub settled_currency: Option<String>,
    pub payment_method: Option<RevolutPaymentMethod>,
    pub authentication_challenge: Option<RevolutAuthenticationChallenge>,
    pub billing_address: Option<RevolutAddress>,
    pub risk_level: Option<RevolutRiskLevel>,
    pub fees: Option<Vec<RevolutFee>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevolutPaymentState {
    Pending,
    AuthenticationChallenge,
    AuthenticationVerified,
    AuthorisationStarted,
    AuthorisationPassed,
    Authorised,
    CaptureStarted,
    Captured,
    RefundValidated,
    RefundStarted,
    CancellationStarted,
    Declining,
    Completing,
    Cancelling,
    Failing,
    Completed,
    Declined,
    SoftDeclined,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevolutDeclineReason {
    #[serde(rename = "3ds_challenge_abandoned")]
    ThreeDsChallengeAbandoned,
    #[serde(rename = "3ds_challenge_failed_manually")]
    ThreeDsChallengeFailedManually,
    CardholderNameMissing,
    CustomerChallengeAbandoned,
    CustomerChallengeFailed,
    CustomerNameMismatch,
    DoNotHonour,
    ExpiredCard,
    HighRisk,
    InsufficientFunds,
    InvalidAddress,
    InvalidAmount,
    InvalidCard,
    InvalidCountry,
    InvalidCvv,
    InvalidEmail,
    InvalidExpiry,
    InvalidMerchant,
    InvalidPhone,
    InvalidPin,
    IssuerNotAvailable,
    PickUpCard,
    RejectedByCustomer,
    RestrictedCard,
    SuspectedFraud,
    TechnicalError,
    TransactionNotAllowedForCardholder,
    UnknownCard,
    WithdrawalLimitExceeded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutMerchantOrderData {
    pub url: Option<String>,
    pub reference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutUpcomingPaymentData {
    #[serde(with = "custom_serde::iso8601")]
    pub date: PrimitiveDateTime,
    pub payment_method_id: Secret<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevolutRiskLevel {
    Low,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutFee {
    pub r#type: RevolutFeeType,
    pub amount: MinorUnit,
    pub currency: common_enums::Currency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevolutFeeType {
    Fx,
    Acquiring,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RevolutPaymentMethod {
    ApplePay(RevolutCardDetails),
    Card(RevolutCardDetails),
    GooglePay(RevolutCardDetails),
    RevolutPayCard(RevolutCardDetails),
    RevolutPayAccount(RevolutAccountDetails),
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutCardDetails {
    pub id: Option<String>,
    pub card_brand: Option<RevolutCardBrand>,
    pub funding: Option<RevolutCardFunding>,
    pub card_country_code: Option<String>,
    pub card_bin: Option<String>,
    pub card_last_four: Option<String>,
    pub card_expiry: Option<String>,
    pub cardholder_name: Option<Secret<String>>,
    pub checks: Option<RevolutPaymentChecks>,
    pub authorisation_code: Option<String>,
    pub arn: Option<String>,
    pub fingerprint: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutAccountDetails {
    pub id: Option<String>,
    pub fingerprint: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevolutCardBrand {
    Visa,
    Mastercard,
    AmericanExpress,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevolutCardFunding {
    Credit,
    Debit,
    Prepaid,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutPaymentChecks {
    pub three_ds: Option<RevolutThreeDsCheck>,
    pub cvv_verification: Option<RevolutVerificationResult>,
    pub address: Option<RevolutVerificationResult>,
    pub postcode: Option<RevolutVerificationResult>,
    pub cardholder: Option<RevolutVerificationResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutThreeDsCheck {
    pub eci: Option<String>,
    pub state: Option<RevolutThreeDsState>,
    pub version: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevolutThreeDsState {
    Verified,
    Failed,
    Challenge,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevolutVerificationResult {
    Match,
    NotMatch,
    #[serde(rename = "n_a")]
    NA,
    Invalid,
    #[serde(rename = "incorrect")]
    Incorrect,
    #[serde(rename = "not_processed")]
    NotProcessed,
}
#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RevolutAuthenticationChallenge {
    ThreeDs(RevolutThreeDsChallenge),
    ThreeDsFingerprint(RevolutThreeDsFingerprintChallenge),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutThreeDsChallenge {
    pub acs_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutThreeDsFingerprintChallenge {
    pub fingerprint_url: String,
    pub fingerprint_data: String,
}

impl TryFrom<&ConnectorSpecificConfig> for RevolutAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Revolut {
                secret_api_key,
                signing_secret,
                ..
            } => Ok(Self {
                secret_api_key: secret_api_key.to_owned(),
                signing_secret: signing_secret.to_owned(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        RevolutRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for RevolutOrderCreateRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: RevolutRouterData<
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

        let customer = router_data
            .request
            .email
            .as_ref()
            .map(|email| RevolutCustomer {
                id: None,
                full_name: router_data
                    .request
                    .customer_name
                    .as_ref()
                    .map(|name| Secret::new(name.clone())),
                phone: None,
                email: Some(email.clone()),
                date_of_birth: None,
            });

        // Map shipping data from resource_common_data
        let shipping = router_data
            .resource_common_data
            .get_optional_shipping()
            .map(|_shipping_address| {
                // Create contact with at least email or phone
                // Use shipping email if available, otherwise use customer email
                let contact_email = router_data
                    .resource_common_data
                    .get_optional_shipping_email()
                    .or_else(|| router_data.request.email.clone());

                // Only include contact if we have email or phone
                let contact = if contact_email.is_some()
                    || router_data
                        .resource_common_data
                        .get_optional_shipping_phone_number()
                        .is_some()
                {
                    Some(RevolutContact {
                        full_name: router_data
                            .resource_common_data
                            .get_optional_shipping_full_name(),
                        phone: router_data
                            .resource_common_data
                            .get_optional_shipping_phone_number(),
                        email: contact_email,
                    })
                } else {
                    None
                };

                RevolutShipping {
                    address: Some(RevolutAddress {
                        street_line_1: router_data
                            .resource_common_data
                            .get_optional_shipping_line1()
                            .map(|line1| line1.expose()),
                        street_line_2: router_data
                            .resource_common_data
                            .get_optional_shipping_line2()
                            .map(|line2| line2.expose()),
                        region: router_data
                            .resource_common_data
                            .get_optional_shipping_state()
                            .map(|state| state.expose()),
                        city: router_data
                            .resource_common_data
                            .get_optional_shipping_city()
                            .map(|city| city.expose()),
                        country_code: router_data
                            .resource_common_data
                            .get_optional_shipping_country()
                            .map(|country| country.to_string()),
                        postcode: router_data
                            .resource_common_data
                            .get_optional_shipping_zip()
                            .map(|zip| zip.expose()),
                    }),
                    contact,
                    shipments: None,
                }
            });

        // Map merchant_order_data from connector_request_reference_id
        let merchant_order_data = Some(RevolutMerchantOrderData {
            url: router_data.request.router_return_url.clone(),
            reference: Some(
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
        });

        let capture_mode = router_data.request.capture_method.map(|c| match c {
            common_enums::CaptureMethod::Manual => RevolutCaptureMode::Manual,
            _ => RevolutCaptureMode::Automatic,
        });

        // Manual capture requires pre_authorisation so subsequent capture and
        // incremental authorisation flows succeed against the same order.
        let authorisation_type = match capture_mode {
            Some(RevolutCaptureMode::Manual) => Some(RevolutAuthorisationType::PreAuthorisation),
            _ => None,
        };

        Ok(Self {
            amount: router_data.request.amount,
            currency: router_data.request.currency,
            settlement_currency: None,
            description: router_data.resource_common_data.description.clone(),
            customer,
            enforce_challenge: None,
            line_items: None,
            shipping,
            capture_mode,
            cancel_authorised_after: None,
            location_id: None,
            metadata: router_data.request.metadata.clone(),
            industry_data: None,
            merchant_order_data,
            upcoming_payment_data: None,
            redirect_url: router_data.request.router_return_url.clone(),
            statement_descriptor_suffix: router_data
                .request
                .billing_descriptor
                .as_ref()
                .and_then(|bd| bd.statement_descriptor_suffix.clone()),
            authorisation_type,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<RevolutOrderCreateResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<RevolutOrderCreateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        let status = match response.state {
            RevolutOrderState::Authorised => AttemptStatus::Authorized,
            RevolutOrderState::Completed => AttemptStatus::Charged,
            RevolutOrderState::Failed => AttemptStatus::Failure,
            RevolutOrderState::Cancelled => AttemptStatus::Voided,
            RevolutOrderState::Pending => AttemptStatus::AuthenticationPending,
            RevolutOrderState::Processing => AttemptStatus::Pending,
        };

        let redirection_data = response
            .checkout_url
            .as_ref()
            .map(|url| Box::new(RedirectForm::Uri { uri: url.clone() }));

        let merchant_reference = response
            .merchant_order_data
            .as_ref()
            .and_then(|m| m.reference.clone());

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
                redirection_data,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: merchant_reference,
                incremental_authorization_allowed: None,
                status_code: 200,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl TryFrom<ResponseRouterData<RevolutOrderCreateResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<RevolutOrderCreateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        let status = match &response.payments {
            Some(payments) => match payments.first() {
                Some(first_payment) => match first_payment.state {
                    RevolutPaymentState::Authorised => AttemptStatus::Authorized,
                    RevolutPaymentState::Captured | RevolutPaymentState::Completed => {
                        AttemptStatus::Charged
                    }
                    RevolutPaymentState::Failed | RevolutPaymentState::Declined => {
                        AttemptStatus::Failure
                    }
                    RevolutPaymentState::Cancelled => AttemptStatus::Voided,
                    RevolutPaymentState::Pending => AttemptStatus::Pending,
                    RevolutPaymentState::AuthenticationChallenge => {
                        AttemptStatus::AuthenticationPending
                    }
                    _ => AttemptStatus::Pending,
                },
                None => map_order_state(response.state),
            },
            None => map_order_state(response.state),
        };

        let amount = Some(Money {
            amount: response.amount,
            currency: response.currency,
        });

        let merchant_reference = response
            .merchant_order_data
            .as_ref()
            .and_then(|m| m.reference.clone());

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: merchant_reference,
                incremental_authorization_allowed: None,
                status_code: 200,
            }),
            resource_common_data: PaymentFlowData {
                status,
                amount,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

fn map_order_state(state: RevolutOrderState) -> AttemptStatus {
    match state {
        RevolutOrderState::Authorised => AttemptStatus::Authorized,
        RevolutOrderState::Completed => AttemptStatus::Charged,
        RevolutOrderState::Failed => AttemptStatus::Failure,
        RevolutOrderState::Cancelled => AttemptStatus::Voided,
        RevolutOrderState::Pending => AttemptStatus::AuthenticationPending,
        RevolutOrderState::Processing => AttemptStatus::Pending,
    }
}

#[derive(Debug, Serialize)]
pub struct RevolutCaptureRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<MinorUnit>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RevolutRefundResponse {
    pub id: String,
    pub r#type: RevolutOrderType,
    pub state: RevolutOrderState,
    #[serde(with = "custom_serde::iso8601")]
    pub created_at: PrimitiveDateTime,
    #[serde(with = "custom_serde::iso8601")]
    pub updated_at: PrimitiveDateTime,
    pub description: Option<String>,
    pub capture_mode: Option<RevolutCaptureMode>,
    pub cancel_authorised_after: Option<String>,
    pub amount: MinorUnit,
    pub outstanding_amount: Option<MinorUnit>,
    pub refunded_amount: Option<MinorUnit>,
    pub currency: common_enums::Currency,
    pub settlement_currency: Option<String>,
    pub customer: Option<RevolutCustomer>,
    pub payments: Option<Vec<RevolutPayment>>,
    pub location_id: Option<String>,
    pub metadata: Option<Secret<serde_json::Value>>,
    pub industry_data: Option<Secret<serde_json::Value>>,
    pub merchant_order_data: Option<RevolutMerchantOrderData>,
    pub upcoming_payment_data: Option<RevolutUpcomingPaymentData>,
    pub checkout_url: Option<String>,
    pub redirect_url: Option<String>,
    pub shipping: Option<RevolutShipping>,
    pub enforce_challenge: Option<RevolutEnforceChallengeMode>,
    pub line_items: Option<Vec<RevolutLineItem>>,
    pub statement_descriptor_suffix: Option<String>,
    pub related_order_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RevolutRefundRequest {
    pub amount: MinorUnit,
    pub currency: common_enums::Currency,
    pub merchant_order_data: Option<RevolutMerchantOrderData>,
    pub metadata: Option<Secret<serde_json::Value>>,
    pub description: Option<String>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        RevolutRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for RevolutRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: RevolutRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        Ok(Self {
            amount: router_data.request.minor_refund_amount,
            currency: router_data.request.currency,
            merchant_order_data: Some(RevolutMerchantOrderData {
                reference: Some(
                    router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                ),
                url: None,
            }),
            metadata: router_data.request.connector_feature_data.clone(),
            description: router_data.request.reason.clone(),
        })
    }
}

impl<F> TryFrom<ResponseRouterData<RevolutRefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<RevolutRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let status = match response.state {
            RevolutOrderState::Completed => common_enums::RefundStatus::Success,
            RevolutOrderState::Processing => common_enums::RefundStatus::Pending,
            RevolutOrderState::Failed => common_enums::RefundStatus::Failure,
            RevolutOrderState::Cancelled => common_enums::RefundStatus::Failure,
            _ => common_enums::RefundStatus::Pending,
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: response.id.clone(),
                refund_status: status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<RevolutRefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<RevolutRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let status = match response.state {
            RevolutOrderState::Completed => common_enums::RefundStatus::Success,
            RevolutOrderState::Processing => common_enums::RefundStatus::Pending,
            RevolutOrderState::Failed => common_enums::RefundStatus::Failure,
            RevolutOrderState::Cancelled => common_enums::RefundStatus::Failure,
            _ => common_enums::RefundStatus::Pending,
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: response.id.clone(),
                refund_status: status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        RevolutRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for RevolutCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: RevolutRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Amount is optional - if not provided, Revolut captures full authorized amount
        let amount = Some(item.router_data.request.minor_amount_to_capture);

        Ok(Self { amount })
    }
}

impl<F> TryFrom<ResponseRouterData<RevolutOrderCreateResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        value: ResponseRouterData<RevolutOrderCreateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        let status = match response.state {
            RevolutOrderState::Completed => AttemptStatus::Charged,
            RevolutOrderState::Authorised => AttemptStatus::Authorized,
            RevolutOrderState::Processing => AttemptStatus::Pending,
            RevolutOrderState::Pending => AttemptStatus::Pending,
            RevolutOrderState::Failed => AttemptStatus::Failure,
            RevolutOrderState::Cancelled => AttemptStatus::Voided,
        };

        let connector_transaction_id = response
            .payments
            .and_then(|payments| payments.first().map(|p| p.id.clone()))
            .unwrap_or_else(|| response.id.clone());

        let merchant_reference = response
            .merchant_order_data
            .as_ref()
            .and_then(|m| m.reference.clone());

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
                redirection_data: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: merchant_reference,
                incremental_authorization_allowed: None,
                mandate_reference: None,
                status_code: http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

// Webhook related structures and implementations

#[derive(Debug, Serialize, Deserialize)]
pub struct RevolutWebhookBody {
    pub event: RevolutWebhookEvent,
    pub order_id: String,
    pub merchant_order_ext_ref: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RevolutWebhookEvent {
    OrderCompleted,
    OrderAuthorised,
    OrderCancelled,
    OrderFailed,
    OrderPaymentAuthenticated,
    OrderPaymentDeclined,
    OrderPaymentFailed,
    PayoutInitiated,
    PayoutCompleted,
    PayoutFailed,
    DisputeActionRequired,
    DisputeUnderReview,
    DisputeWon,
    DisputeLost,
}

/// Maps Revolut webhook event to AttemptStatus for webhook processing
fn map_webhook_event_to_attempt_status(
    event: RevolutWebhookEvent,
) -> Result<AttemptStatus, IntegrationError> {
    match event {
        RevolutWebhookEvent::OrderCompleted => Ok(AttemptStatus::Charged),
        RevolutWebhookEvent::OrderAuthorised => Ok(AttemptStatus::Authorized),
        RevolutWebhookEvent::OrderCancelled => Ok(AttemptStatus::Voided),
        RevolutWebhookEvent::OrderFailed => Ok(AttemptStatus::Failure),
        _ => Err(IntegrationError::not_implemented(
            "webhook event type not found".to_string(),
        )),
    }
}

impl TryFrom<RevolutWebhookBody> for WebhookDetailsResponse {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(webhook_body: RevolutWebhookBody) -> Result<Self, Self::Error> {
        let status = map_webhook_event_to_attempt_status(webhook_body.event)?;

        Ok(Self {
            resource_id: Some(ResponseId::ConnectorTransactionId(
                webhook_body.order_id.clone(),
            )),
            status,
            error_code: None,
            error_message: None,
            error_reason: None,
            status_code: 200,
            connector_response_reference_id: webhook_body.merchant_order_ext_ref,
            mandate_reference: None,
            raw_connector_response: None,
            response_headers: None,
            transformation_status: common_enums::WebhookTransformationStatus::Complete,
            minor_amount_captured: None,
            amount_captured: None,
            network_txn_id: None,
            payment_method_update: None,
        })
    }
}

// ============================================================================
// Incremental Authorization
// ============================================================================
//
// Revolut Merchant API supports incremental authorisation on orders that were
// originally created with `authorisation_type: pre_authorisation`. The increment
// endpoint accepts the incremental delta `amount` (in minor units) and an optional
// merchant `reference`. The endpoint URL is:
//
//   POST {base_url}/api/orders/{id}/increment-authorisation
//
// Successful (2xx) responses return the updated order object. The actual
// outcome (authorised/declined/failed) is also delivered asynchronously via the
// ORDER_INCREMENTAL_AUTHORISATION_AUTHORISED / DECLINED / FAILED webhooks.
// Refs:
//   https://developer.revolut.com/docs/guides/accept-payments/tutorials/advanced-authorisation/incremental-authorisation
//   https://developer.revolut.com/docs/merchant/2024-09-01/orders

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
pub struct RevolutIncrementalAuthRequest {
    /// Incremental delta amount to add to the existing authorization (in minor currency units).
    pub amount: MinorUnit,
    /// External reference for this incremental authorisation. Returned in
    /// webhooks for easy matching to the merchant's system.
    pub reference: Option<String>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        RevolutRouterData<
            RouterDataV2<
                IncrementalAuthorization,
                PaymentFlowData,
                PaymentsIncrementalAuthorizationData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for RevolutIncrementalAuthRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: RevolutRouterData<
            RouterDataV2<
                IncrementalAuthorization,
                PaymentFlowData,
                PaymentsIncrementalAuthorizationData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let incremental_amount = router_data.request.minor_amount.get_amount_as_i64()
            - router_data.request.parent_amount.get_amount_as_i64();

        Ok(Self {
            amount: MinorUnit::new(incremental_amount),
            reference: Some(
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
        })
    }
}

impl TryFrom<ResponseRouterData<RevolutOrderCreateResponse, Self>>
    for RouterDataV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<RevolutOrderCreateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Revolut returns HTTP 2xx with the updated order object once the
        // increment request is accepted. The final outcome
        // (authorised / declined / failed) is delivered asynchronously via
        // ORDER_INCREMENTAL_AUTHORISATION_* webhooks. Map the synchronous
        // body's order state to the closest AuthorizationStatus.
        let authorization_status = if item.http_code >= 200 && item.http_code < 300 {
            match item.response.state {
                RevolutOrderState::Authorised | RevolutOrderState::Completed => {
                    common_enums::AuthorizationStatus::Success
                }
                RevolutOrderState::Failed | RevolutOrderState::Cancelled => {
                    common_enums::AuthorizationStatus::Failure
                }
                RevolutOrderState::Pending | RevolutOrderState::Processing => {
                    common_enums::AuthorizationStatus::Processing
                }
            }
        } else {
            common_enums::AuthorizationStatus::Failure
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::IncrementalAuthorizationResponse {
                status: authorization_status,
                connector_authorization_id: Some(item.response.id.clone()),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

