use common_enums;
use common_utils::consts;
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

use super::PproRouterData;
use crate::types::ResponseRouterData;
use domain_types::errors::{ConnectorError, IntegrationError, WebhookError};
use domain_types::{
    connector_flow::{Capture, RSync, Refund, RepeatPayment, SetupMandate, Void},
    connector_types::{
        EventType, MandateReference, PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsResponseData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, RepeatPaymentData, ResponseId, SetupMandateRequestData,
    },
    mandates::MandateDataType,
    payment_method_data::PaymentMethodDataTypes,
    router_data::ErrorResponse,
    router_data_v2::RouterDataV2,
};
use interfaces::webhooks::IncomingWebhookEvent;

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PproPaymentMedium {
    #[default]
    Ecommerce,
    Moto,
    Pos,
}

impl From<Option<common_enums::PaymentChannel>> for PproPaymentMedium {
    fn from(channel: Option<common_enums::PaymentChannel>) -> Self {
        match channel {
            Some(common_enums::PaymentChannel::Ecommerce) => Self::Ecommerce,
            Some(common_enums::PaymentChannel::MailOrder)
            | Some(common_enums::PaymentChannel::TelephoneOrder) => Self::Moto,
            // Fallback to Ecommerce if unspecified
            None => Self::Ecommerce,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PproPaymentsRequest {
    pub payment_method: String,
    pub payment_medium: PproPaymentMedium,
    pub merchant_payment_charge_reference: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_descriptor: Option<String>,
    pub amount: Amount,
    pub consumer: Option<PproConsumer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication_settings: Option<Vec<PproAuthenticationSettings>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhooks_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PproAuthenticationType {
    Redirect,
    // TODO: Uncomment when adding support for other authentication flows
    // ScanCode,
    // MultiFactor,
    // AppNotification,
    // AppIntent,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PproAuthenticationSettings {
    pub r#type: PproAuthenticationType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<PproAuthSettingsDetails>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PproAuthSettingsDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_url: Option<String>,
    // TODO: Uncomment when adding support for other authentication flows
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub scan_by: Option<String>,
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub mobile_intent_uri: Option<String>
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PproConsumer {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<common_utils::pii::Email>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    /// Unique consumer identifier required by PPRO for payment methods like Trustly
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_consumer_reference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Amount {
    pub currency: String,
    pub value: common_utils::MinorUnit,
}

impl<F, T>
    TryFrom<
        PproRouterData<
            RouterDataV2<F, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
            T,
        >,
    > for PproPaymentsRequest
where
    T: Clone + PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: PproRouterData<
            RouterDataV2<F, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let payment_method = match router_data.request.payment_method_type {
            Some(common_enums::PaymentMethodType::BancontactCard) => "BANCONTACT".to_string(),
            Some(common_enums::PaymentMethodType::UpiIntent) => "UPI".to_string(),
            Some(common_enums::PaymentMethodType::AliPay) => "ALIPAY".to_string(),
            Some(common_enums::PaymentMethodType::WeChatPay) => "WECHATPAY".to_string(),
            Some(common_enums::PaymentMethodType::MbWay) => "MBWAY".to_string(),
            Some(common_enums::PaymentMethodType::Satispay) => "SATISPAY".to_string(),
            Some(common_enums::PaymentMethodType::Wero) => "WERO".to_string(),
            Some(common_enums::PaymentMethodType::Ideal) => "IDEAL".to_string(),
            Some(common_enums::PaymentMethodType::Trustly) => "TRUSTLY".to_string(),
            Some(common_enums::PaymentMethodType::Blik) => "BLIK".to_string(),
            Some(ref pm) => {
                return Err(IntegrationError::NotSupported {
                    message: format!("payment method {pm} is not supported by PPRO"),
                    connector: "ppro",
                    context: Default::default(),
                }
                .into())
            }
            None => {
                return Err(IntegrationError::MissingRequiredField {
                    field_name: "payment_method_type",
                    context: Default::default(),
                }
                .into())
            }
        };

        let amount = Amount {
            currency: router_data.request.currency.to_string(),
            value: common_utils::MinorUnit::new(router_data.request.amount.get_amount_as_i64()),
        };

        // Currently only Redirect authentication is requested.
        // TODO: When adding other authentication flows, extend this list based on payment method:
        //
        // authentication_settings.push(PproAuthenticationSettings {
        //     r#type: PproAuthenticationType::ScanCode,
        //     settings: None,
        // });
        // authentication_settings.push(PproAuthenticationSettings {
        //     r#type: PproAuthenticationType::MultiFactor,
        //     settings: None,
        // });
        // authentication_settings.push(PproAuthenticationSettings {
        //     r#type: PproAuthenticationType::AppNotification,
        //     settings: None,
        // });
        // authentication_settings.push(PproAuthenticationSettings {
        //     r#type: PproAuthenticationType::AppIntent,
        //     settings: Some(PproAuthSettingsDetails {
        //         return_url: None,
        //         scan_by: None,
        //         mobile_intent_uri: router_data.request.router_return_url.clone(),
        //     }),
        // });
        let authentication_settings =
            router_data
                .request
                .router_return_url
                .as_ref()
                .map(|return_url| {
                    vec![PproAuthenticationSettings {
                        r#type: PproAuthenticationType::Redirect,
                        settings: Some(PproAuthSettingsDetails {
                            return_url: Some(return_url.to_string()),
                        }),
                    }]
                });

        let email = router_data
            .resource_common_data
            .get_optional_billing_email()
            .or_else(|| router_data.request.get_optional_email());

        let merchant_consumer_reference = if matches!(
            router_data.request.payment_method_type,
            Some(common_enums::PaymentMethodType::Trustly)
        ) {
            let id = router_data
                .resource_common_data
                .get_connector_customer_id()?;
            Some(sanitize_merchant_consumer_reference(&id))
        } else {
            None
        };

        let consumer = router_data
            .resource_common_data
            .get_billing_address()
            .ok()
            .map(|billing| PproConsumer {
                name: billing.get_full_name().ok(),
                email,
                country: billing.country.map(|c| c.to_string()),
                merchant_consumer_reference,
            });

        Ok(Self {
            payment_method,
            payment_medium: router_data.request.payment_channel.clone().into(),
            merchant_payment_charge_reference: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            payment_descriptor: router_data.resource_common_data.description.clone(),
            amount,
            consumer,
            authentication_settings,
            webhooks_url: Some(router_data.request.get_webhook_url()?),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PproPaymentStatus {
    AuthorizationProcessing,
    CaptureProcessing,
    AuthenticationPending,
    AuthorizationAsync,
    CapturePending,
    Captured,
    Failed,
    Discarded,
    Voided,
    /// Returned on the charge object when a refund has settled.
    RefundSettled,
    Success,
    /// Returned on the charge object when the charge has been fully refunded.
    Refunded,
    Rejected,
    Declined,
}

impl From<PproPaymentStatus> for common_enums::AttemptStatus {
    fn from(status: PproPaymentStatus) -> Self {
        match status {
            PproPaymentStatus::AuthorizationProcessing => Self::Pending,
            PproPaymentStatus::AuthenticationPending => Self::AuthenticationPending,
            PproPaymentStatus::AuthorizationAsync
            | PproPaymentStatus::CapturePending
            | PproPaymentStatus::CaptureProcessing => Self::Authorized,
            PproPaymentStatus::Captured | PproPaymentStatus::Success => Self::Charged,
            PproPaymentStatus::Failed
            | PproPaymentStatus::Discarded
            | PproPaymentStatus::Rejected
            | PproPaymentStatus::Declined => Self::Failure,
            PproPaymentStatus::Voided => Self::Voided,
            // When a charge is refunded, treat it as Charged (terminal success state).
            PproPaymentStatus::RefundSettled | PproPaymentStatus::Refunded => Self::Charged,
        }
    }
}

impl From<PproPaymentStatus> for common_enums::RefundStatus {
    fn from(status: PproPaymentStatus) -> Self {
        match status {
            PproPaymentStatus::RefundSettled | PproPaymentStatus::Refunded => Self::Success,
            PproPaymentStatus::Failed
            | PproPaymentStatus::Rejected
            | PproPaymentStatus::Declined => Self::Failure,
            PproPaymentStatus::AuthorizationProcessing
            | PproPaymentStatus::CaptureProcessing
            | PproPaymentStatus::AuthenticationPending
            | PproPaymentStatus::AuthorizationAsync
            | PproPaymentStatus::CapturePending
            | PproPaymentStatus::Captured
            | PproPaymentStatus::Success
            | PproPaymentStatus::Discarded
            | PproPaymentStatus::Voided => Self::Pending,
        }
    }
}

/// Statuses returned by the PPRO refund API endpoints (POST/GET /v1/payment-charges/{id}/refunds).
/// These are distinct from payment charge statuses.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PproRefundStatus {
    /// Refund has been settled / funds returned to the consumer.
    RefundSettled,
    /// The parent charge has been fully refunded.
    Refunded,
    /// Refund is pending processing by PPRO or the provider.
    Pending,
    Failed,
    Rejected,
    Declined,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PproAgreementStatus {
    Active,
    AuthenticationPending,
    Initializing,
    Failed,
    Revoked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PproPaymentsResponse {
    pub id: String,
    pub status: PproPaymentStatus,
    pub amount: Option<common_utils::MinorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    /// The instrument ID returned by PPRO after a successful authorization.
    /// This is stored as the mandate reference for recurring payments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instrument_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication_methods: Option<Vec<PproAuthenticationResponse>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<PproFailure>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authorizations: Option<Vec<PproAuthorizationEntry>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captures: Option<Vec<PproCaptureEntry>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PproAuthorizationEntry {
    pub id: String,
    pub amount: common_utils::MinorUnit,
    pub status: PproAuthorizationStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_payment_charge_reference: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PproAuthorizationStatus {
    AuthenticationPending,
    Authorized,
    Failed,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PproCaptureEntry {
    pub id: String,
    pub amount: common_utils::MinorUnit,
    pub status: PproCaptureStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_capture_reference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PproCaptureStatus {
    Captured,
    Pending,
    Failed,
    #[serde(other)]
    Unknown,
}

/// PPRO Agreement response — returned from POST /v1/payment-agreements
/// and POST /v1/payment-agreements/{id}/charges
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PproAgreementResponse {
    pub id: String,
    pub status: PproAgreementStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instrument_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<PproFailure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication_methods: Option<Vec<PproAuthenticationResponse>>,
}

pub type PproPSyncResponse = PproPaymentsResponse;
pub type PproAuthorizeResponse = PproPaymentsResponse;
pub type PproCaptureResponse = PproPaymentsResponse;
pub type PproVoidResponse = PproPaymentsResponse;

/// Response body returned by PPRO refund endpoints.
/// Uses `PproRefundStatus` rather than the payment-charge `PproPaymentStatus`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PproRefundResponse {
    pub id: String,
    pub status: PproRefundStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<PproFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PproRefundEntry {
    pub id: String,
    pub amount: common_utils::MinorUnit,
    pub status: PproRefundStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PproRSyncResponse {
    pub id: String,
    pub status: PproPaymentStatus,
    #[serde(default)]
    pub refunds: Vec<PproRefundEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PproAuthenticationResponse {
    pub r#type: PproAuthenticationType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<PproAuthDetailsResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum PproHttpMethod {
    Get,
    Post,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PproAuthDetailsResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_method: Option<PproHttpMethod>,
    // TODO: Uncomment when adding support for other authentication flows
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub code_type: Option<String>,
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub code_image: Option<String>,
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub code_payload: Option<String>,
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub code_document: Option<String>,
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub scan_by: Option<String>,
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub mobile_intent_uri: Option<String>
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PproCaptureRequest {
    pub amount: common_utils::MinorUnit,
}

impl<T>
    TryFrom<
        PproRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for PproCaptureRequest
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: PproRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            amount: item.router_data.request.minor_amount_to_capture,
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PproVoidRequest {
    pub amount: common_utils::MinorUnit,
}

impl<T>
    TryFrom<
        PproRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for PproVoidRequest
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: PproRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount = item
            .router_data
            .request
            .amount
            .or(item
                .router_data
                .resource_common_data
                .minor_amount_authorized)
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "amount or minor_amount_authorized",
                context: Default::default(),
            })?;

        Ok(Self { amount })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PproRefundRequest {
    pub amount: common_utils::MinorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refund_reason: Option<PproRefundReason>,
}

impl<T>
    TryFrom<
        PproRouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for PproRefundRequest
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: PproRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            amount: item.router_data.request.minor_refund_amount,
            refund_reason: item
                .router_data
                .request
                .reason
                .as_ref()
                .map(|r| r.as_str().parse::<PproRefundReason>().unwrap_or_default()),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PproFailure {
    pub failure_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_code: Option<String>,
    pub failure_message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PproErrorResponse {
    pub status: u16,
    pub failure_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PproWebhookType {
    PaymentChargeAuthorizationSucceeded,
    PaymentChargeSuccess,
    PaymentChargeAuthorizationFailed,
    PaymentChargeFailed,
    PaymentChargeDiscarded,
    PaymentChargeCaptureSucceeded,
    PaymentChargeCaptureFailed,
    PaymentChargeVoidSucceeded,
    PaymentChargeVoidFailed,
    PaymentChargeRefundSucceeded,
    PaymentChargeRefundFailed,
    PaymentAgreementActive,
    PaymentAgreementFailed,
    PaymentAgreementRevokedByConsumer,
    PaymentAgreementRevokedByMerchant,
    PaymentAgreementRevokedByProvider,
}

impl TryFrom<PproWebhookType> for EventType {
    type Error = error_stack::Report<WebhookError>;

    fn try_from(event_type: PproWebhookType) -> Result<Self, Self::Error> {
        match event_type {
            PproWebhookType::PaymentChargeCaptureSucceeded => Ok(Self::PaymentIntentCaptureSuccess),
            PproWebhookType::PaymentChargeFailed
            | PproWebhookType::PaymentChargeAuthorizationFailed
            | PproWebhookType::PaymentChargeDiscarded => Ok(Self::PaymentIntentFailure),
            PproWebhookType::PaymentChargeAuthorizationSucceeded
            | PproWebhookType::PaymentChargeSuccess => Ok(Self::PaymentIntentAuthorizationSuccess),
            PproWebhookType::PaymentChargeRefundSucceeded => Ok(Self::RefundSuccess),
            PproWebhookType::PaymentChargeRefundFailed => Ok(Self::RefundFailure),
            PproWebhookType::PaymentChargeVoidSucceeded => Ok(Self::PaymentIntentCancelled),
            PproWebhookType::PaymentChargeVoidFailed => Ok(Self::PaymentIntentCancelFailure),
            PproWebhookType::PaymentChargeCaptureFailed => Ok(Self::PaymentIntentCaptureFailure),
            PproWebhookType::PaymentAgreementActive => Ok(Self::MandateActive),
            PproWebhookType::PaymentAgreementFailed => Ok(Self::MandateFailed),
            PproWebhookType::PaymentAgreementRevokedByConsumer
            | PproWebhookType::PaymentAgreementRevokedByMerchant
            | PproWebhookType::PaymentAgreementRevokedByProvider => Ok(Self::MandateRevoked),
        }
    }
}

impl TryFrom<PproWebhookType> for IncomingWebhookEvent {
    type Error = error_stack::Report<WebhookError>;

    fn try_from(event_type: PproWebhookType) -> Result<Self, Self::Error> {
        match event_type {
            PproWebhookType::PaymentChargeAuthorizationSucceeded
            | PproWebhookType::PaymentChargeSuccess => Ok(Self::PaymentIntentSuccess),
            PproWebhookType::PaymentChargeAuthorizationFailed
            | PproWebhookType::PaymentChargeFailed
            | PproWebhookType::PaymentChargeDiscarded => Ok(Self::PaymentIntentFailure),
            PproWebhookType::PaymentChargeCaptureSucceeded => Ok(Self::PaymentIntentCaptureSuccess),
            PproWebhookType::PaymentChargeCaptureFailed => Ok(Self::PaymentIntentCaptureFailure),
            PproWebhookType::PaymentChargeVoidSucceeded => Ok(Self::PaymentIntentCancelled),
            PproWebhookType::PaymentChargeVoidFailed => Ok(Self::PaymentIntentCancelFailure),
            PproWebhookType::PaymentChargeRefundSucceeded => Ok(Self::RefundSuccess),
            PproWebhookType::PaymentChargeRefundFailed => Ok(Self::RefundFailure),
            PproWebhookType::PaymentAgreementActive => Ok(Self::MandateActive),
            PproWebhookType::PaymentAgreementFailed
            | PproWebhookType::PaymentAgreementRevokedByConsumer
            | PproWebhookType::PaymentAgreementRevokedByMerchant
            | PproWebhookType::PaymentAgreementRevokedByProvider => Ok(Self::MandateRevoked),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PproWebhookEvent {
    pub specversion: String,
    pub r#type: PproWebhookType,
    pub source: String,
    pub id: String,
    pub time: String,
    pub data: PproWebhookData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PproWebhookData {
    Charge { charge: PproPaymentsResponse },
    Agreement { agreement: PproAgreementResponse },
}

impl<F, Req> TryFrom<ResponseRouterData<PproPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, Req, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<PproPaymentsResponse, Self>) -> Result<Self, Self::Error> {
        let status = common_enums::AttemptStatus::from(item.response.status);

        let mut error_response = None;
        if status == common_enums::AttemptStatus::Failure {
            if let Some(failure) = &item.response.failure {
                let fallback_msg = failure
                    .failure_code
                    .clone()
                    .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string());
                let message = if failure.failure_message.is_empty() {
                    fallback_msg.clone()
                } else {
                    failure.failure_message.clone()
                };

                error_response = Some(ErrorResponse {
                    status_code: item.http_code,
                    code: failure
                        .failure_code
                        .clone()
                        .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
                    message,
                    reason: Some(format!("{}: {}", failure.failure_type, fallback_msg)),
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                });
            }
        }

        let mut redirection_data: Option<domain_types::router_response_types::RedirectForm> = None;
        if status == common_enums::AttemptStatus::AuthenticationPending {
            if let Some(auth_methods) = item.response.authentication_methods.as_ref() {
                // Currently only Redirect flow is supported.
                // TODO: When adding other authentication flows, use priority-based selection:
                //
                // let priorities: Vec<PproAuthenticationType> = match &item.router_data.request.payment_method_data {
                //     PaymentMethodData::Wallet(WalletData::SatispaySdk(_)) => {
                //         vec![PproAuthenticationType::AppIntent, PproAuthenticationType::ScanCode, PproAuthenticationType::Redirect]
                //     }
                //     PaymentMethodData::Wallet(WalletData::MbWaySdk(_)) => {
                //         vec![PproAuthenticationType::AppNotification, PproAuthenticationType::Redirect]
                //     }
                //     PaymentMethodData::Upi(UpiData::UpiIntent(_)) => {
                //         vec![PproAuthenticationType::Redirect, PproAuthenticationType::ScanCode]
                //     }
                //     _ => vec![PproAuthenticationType::Redirect],
                // };
                //
                // Then iterate priorities and match:
                //   ScanCode   -> details.code_payload  -> RedirectForm::Uri
                //   AppIntent  -> details.mobile_intent_uri -> RedirectForm::Uri
                //   Redirect   -> details.request_url   -> RedirectForm::Form
                //   AppNotification / MultiFactor -> no redirect needed
                //
                // Find the Redirect authentication method from PPRO's response
                for method in auth_methods {
                    if method.r#type == PproAuthenticationType::Redirect {
                        if let Some(details) = &method.details {
                            if let Some(url) = &details.request_url {
                                redirection_data =
                                    Some(domain_types::router_response_types::RedirectForm::Uri {
                                        uri: url.to_string(),
                                    });
                                break;
                            }
                        }
                    }
                }
            }
        }

        // If PPRO returned an instrumentId, store it as the mandate reference
        // so callers can use it for subsequent RepeatPayment charges.
        let mandate_reference = item.response.instrument_id.as_ref().map(|instr_id| {
            Box::new(MandateReference {
                connector_mandate_id: Some(instr_id.clone()),
                payment_method_id: None,
                connector_mandate_request_reference_id: None,
            })
        });

        let resolved_minor_amount = item
            .response
            .captures
            .as_ref()
            .and_then(|c| c.first())
            .map(|c| c.amount)
            .or_else(|| {
                item.response
                    .authorizations
                    .as_ref()
                    .and_then(|auths| auths.last())
                    .map(|a| a.amount)
            })
            .or(item.response.amount);

        let resolved_currency = item
            .response
            .currency
            .as_deref()
            .and_then(|c| c.parse::<common_enums::Currency>().ok())
            .or_else(|| {
                item.router_data
                    .resource_common_data
                    .amount
                    .as_ref()
                    .map(|m| m.currency)
            });

        let response_amount = resolved_minor_amount.map(|minor| common_utils::types::Money {
            amount: minor,
            currency: resolved_currency.unwrap_or_default(),
        });

        let connector_response_reference_id = item
            .response
            .captures
            .as_ref()
            .and_then(|c| c.first())
            .and_then(|c| c.merchant_capture_reference.clone())
            .or_else(|| {
                item.response
                    .authorizations
                    .as_ref()
                    .and_then(|a| a.last())
                    .and_then(|a| a.merchant_payment_charge_reference.clone())
            });

        let captured_amount = item
            .response
            .captures
            .as_ref()
            .and_then(|c| c.first())
            .map(|c| c.amount.get_amount_as_i64());

        let response = if let Some(err) = error_response {
            Err(err)
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id),
                redirection_data: redirection_data.map(Box::new),
                mandate_reference,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            })
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                amount: response_amount.or(item.router_data.resource_common_data.amount),
                amount_captured: captured_amount
                    .or(item.router_data.resource_common_data.amount_captured),
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}

impl<F, Req, T> TryFrom<ResponseRouterData<PproRefundResponse, Self>>
    for RouterDataV2<F, Req, T, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<PproRefundResponse, Self>) -> Result<Self, Self::Error> {
        let refund_status = match item.response.status {
            PproRefundStatus::RefundSettled | PproRefundStatus::Refunded => {
                common_enums::RefundStatus::Success
            }
            PproRefundStatus::Failed | PproRefundStatus::Rejected | PproRefundStatus::Declined => {
                common_enums::RefundStatus::Failure
            }
            PproRefundStatus::Pending | PproRefundStatus::Unknown => {
                common_enums::RefundStatus::Pending
            }
        };

        let response = if refund_status == common_enums::RefundStatus::Failure {
            let failure_code = item
                .response
                .failure
                .as_ref()
                .and_then(|f| f.failure_code.clone());
            let failure_message = item
                .response
                .failure
                .as_ref()
                .map(|f| f.failure_message.clone())
                .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string());
            let failure_type = item
                .response
                .failure
                .as_ref()
                .map(|f| f.failure_type.clone())
                .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string());

            let failure_code_str =
                failure_code.unwrap_or_else(|| consts::NO_ERROR_CODE.to_string());
            Err(ErrorResponse {
                code: failure_code_str.clone(),
                message: failure_message,
                reason: Some(format!("{}: {}", failure_type, failure_code_str)),
                status_code: item.http_code,
                attempt_status: None,
                connector_transaction_id: Some(item.response.id.clone()),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            })
        } else {
            Ok(RefundsResponseData {
                connector_refund_id: item.response.id.clone(),
                refund_status,
                status_code: item.http_code,
            })
        };

        Ok(Self {
            response,
            ..item.router_data
        })
    }
}

impl TryFrom<ResponseRouterData<PproRSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<PproRSyncResponse, Self>) -> Result<Self, Self::Error> {
        let connector_refund_id = &item.router_data.request.connector_refund_id;
        let refunds = &item.response.refunds;
        let refund_status =
            if let Some(entry) = refunds.iter().find(|r| &r.id == connector_refund_id) {
                match entry.status {
                    PproRefundStatus::RefundSettled | PproRefundStatus::Refunded => {
                        common_enums::RefundStatus::Success
                    }
                    PproRefundStatus::Failed
                    | PproRefundStatus::Rejected
                    | PproRefundStatus::Declined => common_enums::RefundStatus::Failure,
                    PproRefundStatus::Pending | PproRefundStatus::Unknown => {
                        common_enums::RefundStatus::Pending
                    }
                }
            } else if refunds.iter().any(|r| {
                matches!(
                    r.status,
                    PproRefundStatus::Refunded | PproRefundStatus::RefundSettled
                )
            }) {
                common_enums::RefundStatus::Success
            } else if !refunds.is_empty()
                && refunds.iter().all(|r| {
                    matches!(
                        r.status,
                        PproRefundStatus::Failed
                            | PproRefundStatus::Rejected
                            | PproRefundStatus::Declined
                    )
                })
            {
                common_enums::RefundStatus::Failure
            } else {
                common_enums::RefundStatus::Pending
            };

        let response = Ok(RefundsResponseData {
            connector_refund_id: connector_refund_id.clone(),
            refund_status,
            status_code: item.http_code,
        });

        Ok(Self {
            response,
            ..item.router_data
        })
    }
}

// ========== SetupMandate (POST /v1/payment-agreements) ==========

/// Request body for creating a Payment Agreement (mandate setup with no initial charge).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PproAgreementRequest {
    pub payment_method: String,
    pub payment_medium: PproPaymentMedium,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_payment_agreement_reference: Option<String>,
    pub amount: Amount,
    pub amount_type: PproAmountType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency: Option<PproFrequency>,
    pub consumer: Option<PproConsumer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_payment_charge: Option<PproInitialPaymentCharge>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication_settings: Option<Vec<PproAuthenticationSettings>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instrument: Option<PproInstrument>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PproInstrument {
    pub r#type: PproInstrumentType,
    pub details: PproInstrumentDetails,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PproInstrumentType {
    BankAccount,
    PassthroughWallet,
    BancontactAccount,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PproInstrumentDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bank_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debit_mandate_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PproFrequency {
    pub r#type: PproFrequencyType,
    pub interval: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PproFrequencyType {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PproInitialPaymentCharge {
    pub initiator: PproChargeInitiator,
    pub amount: Amount,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_descriptor: Option<String>,
}

impl<T>
    TryFrom<
        PproRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PproAgreementRequest
where
    T: Clone + PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: PproRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let payment_method = match router_data.request.payment_method_type {
            Some(common_enums::PaymentMethodType::BancontactCard) => "BANCONTACT".to_string(),
            Some(common_enums::PaymentMethodType::Ideal) => "IDEAL".to_string(),
            Some(common_enums::PaymentMethodType::Trustly) => "TRUSTLY".to_string(),
            Some(common_enums::PaymentMethodType::Blik) => "BLIK".to_string(),
            Some(common_enums::PaymentMethodType::UpiCollect) => "UPI".to_string(),
            Some(ref pm) => {
                return Err(IntegrationError::NotSupported {
                    message: format!("payment method {pm} is not supported for PPRO mandates"),
                    connector: "ppro",
                    context: Default::default(),
                }
                .into())
            }
            None => {
                return Err(IntegrationError::MissingRequiredField {
                    field_name: "payment_method_type",
                    context: Default::default(),
                }
                .into())
            }
        };

        let amount = Amount {
            currency: router_data.request.currency.to_string(),
            value: common_utils::MinorUnit::new(
                router_data
                    .request
                    .minor_amount
                    .map(|a| a.get_amount_as_i64())
                    .unwrap_or(0),
            ),
        };

        let authentication_settings =
            router_data
                .request
                .router_return_url
                .as_ref()
                .map(|return_url| {
                    vec![PproAuthenticationSettings {
                        r#type: PproAuthenticationType::Redirect,
                        settings: Some(PproAuthSettingsDetails {
                            return_url: Some(return_url.clone()),
                        }),
                    }]
                });

        let email = router_data.request.email.clone();

        let merchant_consumer_reference = if matches!(
            router_data.request.payment_method_type,
            Some(common_enums::PaymentMethodType::Trustly)
        ) {
            let id = router_data
                .resource_common_data
                .get_connector_customer_id()?;
            Some(sanitize_merchant_consumer_reference(&id))
        } else {
            None
        };

        let consumer = router_data
            .resource_common_data
            .get_billing_address()
            .ok()
            .map(|billing| PproConsumer {
                name: billing.get_optional_full_name().or_else(|| {
                    router_data
                        .request
                        .customer_name
                        .as_ref()
                        .map(|n| Secret::new(n.clone()))
                }),
                email,
                country: billing.country.map(|c| c.to_string()),
                merchant_consumer_reference,
            });

        let start_date = router_data
            .request
            .setup_mandate_details
            .as_ref()
            .and_then(|m| m.mandate_type.as_ref())
            .and_then(|t| match t {
                MandateDataType::SingleUse(a) => a.start_date,
                MandateDataType::MultiUse(Some(a)) => a.start_date,
                MandateDataType::MultiUse(None) => None,
            })
            .map(|dt| format!("{}Z", dt.to_string().replace(" ", "T")));

        let end_date = router_data
            .request
            .setup_mandate_details
            .as_ref()
            .and_then(|m| m.mandate_type.as_ref())
            .and_then(|t| match t {
                MandateDataType::SingleUse(a) => a.end_date,
                MandateDataType::MultiUse(Some(a)) => a.end_date,
                MandateDataType::MultiUse(None) => None,
            })
            .map(|dt| format!("{}Z", dt.to_string().replace(" ", "T")));

        // Read amount_type and frequency from setup_mandate_details
        let (amount_type, frequency) = router_data
            .request
            .setup_mandate_details
            .as_ref()
            .and_then(|mandate_data| match &mandate_data.mandate_type {
                Some(MandateDataType::MultiUse(Some(amount_data)))
                | Some(MandateDataType::SingleUse(amount_data)) => {
                    let amt_type = amount_data
                        .amount_type
                        .as_ref()
                        .map(|s: &String| match s.to_uppercase().as_str() {
                            "MAX" => PproAmountType::Max,
                            "VARIABLE" => PproAmountType::Variable,
                            _ => PproAmountType::Exact,
                        })
                        .unwrap_or_default();

                    let freq = amount_data.frequency.as_ref().and_then(|f: &String| {
                        let parts: Vec<&str> = f.split(':').collect();
                        let f_type = parts.first()?;
                        let interval = parts
                            .get(1)
                            .and_then(|s: &&str| s.parse::<u32>().ok())
                            .unwrap_or(1);
                        let r_type = match f_type.to_uppercase().as_str() {
                            "DAILY" => PproFrequencyType::Daily,
                            "WEEKLY" => PproFrequencyType::Weekly,
                            "YEARLY" => PproFrequencyType::Yearly,
                            _ => PproFrequencyType::Monthly,
                        };
                        Some(PproFrequency {
                            r#type: r_type,
                            interval,
                        })
                    });

                    Some((amt_type, freq))
                }
                _ => None,
            })
            .unwrap_or((PproAmountType::Exact, None));

        // Build instrument details based on payment method type.
        // Most payment methods don't require instrument details upfront - PPRO creates it during authentication.
        // Some payment methods (like iDEAL) have specific requirements for recurring agreements.
        let instrument = build_agreement_instrument(&router_data);

        Ok(Self {
            payment_method,
            payment_medium: router_data.request.payment_channel.into(),
            merchant_payment_agreement_reference: Some(
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            amount,
            amount_type,
            description: router_data.resource_common_data.description.clone(),
            start_date,
            end_date,
            frequency,
            consumer,
            initial_payment_charge: None, // Can be extended if needed for Link & Pay
            authentication_settings,
            instrument,
        })
    }
}

fn build_agreement_instrument<T>(
    router_data: &RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >,
) -> Option<PproInstrument>
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    match router_data.request.payment_method_type {
        // iDEAL requires debitMandateId for recurring agreements
        Some(common_enums::PaymentMethodType::Ideal) => {
            let bank_name = match &router_data.request.payment_method_data {
                domain_types::payment_method_data::PaymentMethodData::BankRedirect(
                    domain_types::payment_method_data::BankRedirectData::Ideal { bank_name },
                ) => *bank_name,
                _ => None,
            };
            let bank_code = bank_name.and_then(get_ppro_bank_code);
            // For iDEAL agreements, debitMandateId is mandatory.
            // We use the mandate_id from request if available, otherwise fallback to payment_id
            let debit_mandate_id = router_data
                .request
                .mandate_id
                .as_ref()
                .and_then(|m| m.mandate_id.clone())
                .unwrap_or_else(|| router_data.resource_common_data.payment_id.clone());

            Some(PproInstrument {
                r#type: PproInstrumentType::BankAccount,
                details: PproInstrumentDetails {
                    bank_code,
                    debit_mandate_id: Some(debit_mandate_id),
                },
            })
        }
        // BLIK - Add specific handling if PPRO requires instrument details for agreements
        // Some(common_enums::PaymentMethodType::Blik) => { ... }
        //
        // Bancontact - Add specific handling if PPRO requires instrument details for agreements
        // Some(common_enums::PaymentMethodType::Bancontact) => { ... }
        //
        // For most other payment methods, PPRO creates the instrument during authentication
        // so we don't need to send instrument details upfront
        _ => None,
    }
}

pub fn get_ppro_bank_code(bank_name: common_enums::BankNames) -> Option<String> {
    match bank_name {
        common_enums::BankNames::AbnAmro => Some("ABNANL2A".to_string()),
        common_enums::BankNames::AsnBank => Some("ASNBLL21".to_string()),
        common_enums::BankNames::Bunq => Some("BUNQNL2A".to_string()),
        common_enums::BankNames::Ing => Some("INGBNL2A".to_string()),
        common_enums::BankNames::Knab => Some("KNABNL2H".to_string()),
        common_enums::BankNames::Rabobank => Some("RABONL2U".to_string()),
        common_enums::BankNames::Regiobank => Some("ASNBLL21".to_string()),
        common_enums::BankNames::Revolut => Some("REVO".to_string()),
        common_enums::BankNames::SnsBank => Some("ASNBLL21".to_string()),
        common_enums::BankNames::TriodosBank => Some("TRIO".to_string()),
        common_enums::BankNames::VanLanschot => Some("FVLB".to_string()),
        _ => None,
    }
}

/// Sanitize the merchantConsumerReference
fn sanitize_merchant_consumer_reference(connector_customer_id: &str) -> String {
    connector_customer_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || "@$%&*-+/.,".contains(*c))
        .take(50)
        .collect()
}

/// TryFrom to convert a PPRO Agreement response into the RouterDataV2 for SetupMandate.
impl<F, Req> TryFrom<ResponseRouterData<PproAgreementResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, Req, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PproAgreementResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = match item.response.status {
            PproAgreementStatus::Active => common_enums::AttemptStatus::Charged,
            PproAgreementStatus::AuthenticationPending | PproAgreementStatus::Initializing => {
                common_enums::AttemptStatus::AuthenticationPending
            }
            PproAgreementStatus::Failed | PproAgreementStatus::Revoked => {
                common_enums::AttemptStatus::Failure
            }
        };

        let mut error_response = None;
        if status == common_enums::AttemptStatus::Failure {
            if let Some(failure) = &item.response.failure {
                error_response = Some(ErrorResponse {
                    status_code: item.http_code,
                    code: failure
                        .failure_code
                        .clone()
                        .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
                    message: failure.failure_message.clone(),
                    reason: Some(format!(
                        "{}: {}",
                        failure.failure_type, failure.failure_message
                    )),
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                });
            }
        }

        // Build redirect if authentication_pending
        let mut redirection_data = None;
        if status == common_enums::AttemptStatus::AuthenticationPending {
            if let Some(auth_methods) = item.response.authentication_methods.as_ref() {
                for method in auth_methods {
                    if method.r#type == PproAuthenticationType::Redirect {
                        if let Some(details) = &method.details {
                            if let Some(url) = &details.request_url {
                                redirection_data =
                                    Some(domain_types::router_response_types::RedirectForm::Uri {
                                        uri: url.clone(),
                                    });
                                break;
                            }
                        }
                    }
                }
            }
        }

        // The agreement ID is stored as the mandate reference (connector_mandate_id)
        let mandate_reference = Some(Box::new(MandateReference {
            connector_mandate_id: Some(item.response.id.clone()),
            payment_method_id: None,
            connector_mandate_request_reference_id: None,
        }));

        let response = if let Some(err) = error_response {
            Err(err)
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id),
                redirection_data: redirection_data.map(Box::new),
                mandate_reference,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            })
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}

#[derive(Debug, Serialize, Default, Clone, Copy)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PproChargeInitiator {
    #[default]
    Merchant,
    Consumer,
}

#[derive(Debug, Serialize, Default, Clone, Copy)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PproAmountType {
    Max,
    #[default]
    Exact,
    Variable,
}

#[derive(Debug, Serialize, Default, Clone, Copy)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PproScheduleType {
    Scheduled,
    #[default]
    Unscheduled,
    ScheduledRetry,
    Recurring,
}

#[derive(Debug, Serialize, Default, Clone, Copy, strum::EnumString)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE", ascii_case_insensitive)]
pub enum PproRefundReason {
    Return,
    Duplicate,
    Fraud,
    CustomerRequest,
    PreDispute,
    #[default]
    Other,
}

/// Request body for creating a charge against an existing Payment Agreement.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PproAgreementChargeRequest {
    pub amount: Amount,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule_type: Option<PproScheduleType>,
    pub auto_capture: bool,
    pub initiator: PproChargeInitiator,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_descriptor: Option<String>,
}

impl<T>
    TryFrom<
        PproRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PproAgreementChargeRequest
where
    T: Clone + PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: PproRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let amount = Amount {
            currency: router_data.request.currency.to_string(),
            value: common_utils::MinorUnit::new(
                router_data.request.minor_amount.get_amount_as_i64(),
            ),
        };

        let initiator = if router_data.request.off_session.unwrap_or(true) {
            PproChargeInitiator::Merchant
        } else {
            PproChargeInitiator::Consumer
        };

        Ok(Self {
            amount,
            schedule_type: Some(match router_data.request.mit_category {
                Some(common_enums::MitCategory::Recurring) => PproScheduleType::Recurring,
                _ => PproScheduleType::Unscheduled,
            }),
            auto_capture: matches!(
                router_data.request.capture_method,
                Some(common_enums::CaptureMethod::Automatic)
            ),
            initiator,
            payment_descriptor: router_data.resource_common_data.description.clone(),
        })
    }
}
