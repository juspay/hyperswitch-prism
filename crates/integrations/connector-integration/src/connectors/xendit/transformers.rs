use std::collections::HashMap;

use common_enums::Currency;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    pii,
    request::Method,
    types::FloatMajorUnit,
};
use domain_types::{
    connector_flow::{Authorize, Capture},
    connector_types::{
        MandateReference, PaymentFlowData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId,
    },
    errors::{ConnectorResponseTransformationError, IntegrationError},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_request_types::{AuthoriseIntegrityObject, RefundIntegrityObject},
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::{
    connectors::xendit::{XenditAmountConvertor, XenditRouterData},
    types::ResponseRouterData,
    utils::get_unimplemented_payment_method_error_message,
};

pub trait ForeignTryFrom<F>: Sized {
    type Error;

    fn foreign_try_from(from: F) -> Result<Self, Self::Error>;
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ChannelProperties {
    pub success_return_url: String,
    pub failure_return_url: String,
    pub skip_three_d_secure: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CardInformation<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub card_number: RawCardNumber<T>,
    pub expiry_month: Secret<String>,
    pub expiry_year: Secret<String>,
    pub cvv: Option<Secret<String>>,
    pub cardholder_name: Secret<String>,
    pub cardholder_email: pii::Email,
    pub cardholder_phone_number: Secret<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CardInfo<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
{
    pub channel_properties: ChannelProperties,
    pub card_information: CardInformation<T>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransactionType {
    OneTimeUse,
    MultipleUse,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum PaymentMethodType {
    CARD,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum PaymentMethod<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    Card(CardPaymentRequest<T>),
}
#[derive(Serialize, Deserialize, Debug)]
pub struct CardPaymentRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(rename = "type")]
    pub payment_type: PaymentMethodType,
    pub card: CardInfo<T>,
    pub reusability: TransactionType,
    pub reference_id: Secret<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaymentStatus {
    Pending,
    RequiresAction,
    Failed,
    Succeeded,
    AwaitingCapture,
    Verified,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum MethodType {
    Get,
    Post,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub method: MethodType,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentMethodInfo {
    pub id: Secret<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct XenditPaymentResponse {
    pub id: String,
    pub status: PaymentStatus,
    pub actions: Option<Vec<Action>>,
    pub payment_method: PaymentMethodInfo,
    pub failure_code: Option<String>,
    pub reference_id: Secret<String>,
    pub amount: FloatMajorUnit,
    pub currency: Currency,
}

pub struct XenditAuthType {
    pub(super) api_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for XenditAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Xendit { api_key, .. } => Ok(Self {
                api_key: api_key.to_owned(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

// Basic Request Structure from Hyperswitch Xendit
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct XenditPaymentsRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub amount: FloatMajorUnit,
    pub currency: Currency,
    pub capture_method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method: Option<PaymentMethod<T>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method_id: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel_properties: Option<ChannelProperties>,
}

#[derive(Debug, Clone, Serialize)]
pub enum XenditPaymentMethodType {
    #[serde(rename = "CARD")]
    Card,
    // ... other types like EWALLET, DIRECT_DEBIT etc.
}

#[derive(Debug, Clone, Serialize)]
pub struct XenditLineItem {
    pub name: String,
    pub quantity: i32,
    pub price: i64,
    pub category: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum XenditResponse {
    Payment(XenditPaymentResponse),
    Webhook(XenditWebhookEvent),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct XenditWebhookEvent {
    pub event: XenditEventType,
    pub data: EventDetails,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum XenditEventType {
    #[serde(rename = "payment.succeeded")]
    PaymentSucceeded,
    #[serde(rename = "payment.awaiting_capture")]
    PaymentAwaitingCapture,
    #[serde(rename = "payment.failed")]
    PaymentFailed,
    #[serde(rename = "capture.succeeded")]
    CaptureSucceeded,
    #[serde(rename = "capture.failed")]
    CaptureFailed,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EventDetails {
    pub id: String,
    pub payment_request_id: Option<String>,
    pub amount: FloatMajorUnit,
    pub currency: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct XenditPaymentActions {
    #[serde(rename = "desktop_web_checkout_url")]
    pub desktop_redirect_url: Option<String>,
    #[serde(rename = "mobile_web_checkout_url")]
    pub mobile_redirect_url: Option<String>,
    #[serde(rename = "mobile_deeplink_checkout_url")]
    pub mobile_deeplink_url: Option<String>,
    // QR code URL if applicable
    #[serde(rename = "qr_checkout_string")]
    pub qr_code_url: Option<String>,
}

// Xendit Error Response Structure (from Hyperswitch xendit.rs)
#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct XenditErrorResponse {
    pub error_code: Option<String>,
    pub message: Option<String>,
    pub reason: Option<String>, // This might not be standard, check Xendit docs
                                // Xendit might have more structured errors, e.g. a list of errors
                                // errors: Option<Vec<XenditErrorDetail>>
}

fn is_auto_capture<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    data: &PaymentsAuthorizeData<T>,
) -> Result<bool, IntegrationError> {
    match data.capture_method {
        Some(common_enums::CaptureMethod::Automatic) | None => Ok(true),
        Some(common_enums::CaptureMethod::Manual) => Ok(false),
        Some(_) => Err(IntegrationError::CaptureMethodNotSupported {
            context: Default::default(),
        }),
    }
}

fn is_auto_capture_psync(data: &PaymentsSyncData) -> Result<bool, IntegrationError> {
    match data.capture_method {
        Some(common_enums::CaptureMethod::Automatic) | None => Ok(true),
        Some(common_enums::CaptureMethod::Manual) => Ok(false),
        Some(_) => Err(IntegrationError::CaptureMethodNotSupported {
            context: Default::default(),
        }),
    }
}

fn is_auto_capture_request<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    data: &PaymentsAuthorizeData<T>,
) -> Result<bool, error_stack::Report<IntegrationError>> {
    is_auto_capture(data).change_context(IntegrationError::CaptureMethodNotSupported {
        context: Default::default(),
    })
}

fn is_auto_capture_psync_response(
    data: &PaymentsSyncData,
) -> Result<bool, error_stack::Report<ConnectorResponseTransformationError>> {
    is_auto_capture_psync(data).change_context(
        ConnectorResponseTransformationError::response_handling_failed_http_status_unknown(),
    )
}

fn map_payment_response_to_attempt_status(
    response: XenditPaymentResponse,
    is_auto_capture: bool,
) -> common_enums::AttemptStatus {
    match response.status {
        PaymentStatus::Failed => common_enums::AttemptStatus::Failure,
        PaymentStatus::Succeeded | PaymentStatus::Verified => {
            if is_auto_capture {
                common_enums::AttemptStatus::Charged
            } else {
                common_enums::AttemptStatus::Authorized
            }
        }
        PaymentStatus::Pending => common_enums::AttemptStatus::Pending,
        PaymentStatus::RequiresAction => common_enums::AttemptStatus::AuthenticationPending,
        PaymentStatus::AwaitingCapture => common_enums::AttemptStatus::Authorized,
    }
}

impl From<PaymentStatus> for common_enums::AttemptStatus {
    fn from(status: PaymentStatus) -> Self {
        match status {
            PaymentStatus::Failed => Self::Failure,
            PaymentStatus::Succeeded | PaymentStatus::Verified => Self::Charged,
            PaymentStatus::Pending => Self::Pending,
            PaymentStatus::RequiresAction => Self::AuthenticationPending,
            PaymentStatus::AwaitingCapture => Self::Authorized,
        }
    }
}

// Transformer for Request: RouterData -> XenditPaymentsRequest
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        XenditRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for XenditPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: XenditRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        match item.router_data.request.payment_method_data.clone() {
            PaymentMethodData::Card(card_data) => Ok(Self {
                capture_method: match is_auto_capture_request(&item.router_data.request)? {
                    true => "AUTOMATIC".to_string(),
                    false => "MANUAL".to_string(),
                },
                currency: item.router_data.request.currency,
                amount: item
                    .connector
                    .amount_converter
                    .convert(
                        item.router_data.request.minor_amount,
                        item.router_data.request.currency,
                    )
                    .change_context(IntegrationError::AmountConversionFailed {
                        context: Default::default(),
                    })
                    .attach_printable("Failed to convert amount to required type")?,
                payment_method: Some(PaymentMethod::Card(CardPaymentRequest {
                    payment_type: PaymentMethodType::CARD,
                    reference_id: Secret::new(
                        item.router_data
                            .resource_common_data
                            .connector_request_reference_id
                            .clone(),
                    ),
                    card: CardInfo {
                        channel_properties: ChannelProperties {
                            success_return_url: item
                                .router_data
                                .request
                                .get_router_return_url()
                                .change_context(IntegrationError::MissingRequiredField {
                                    field_name: "router_return_url",
                                    context: Default::default(),
                                })?,
                            failure_return_url: item
                                .router_data
                                .request
                                .get_router_return_url()
                                .change_context(IntegrationError::MissingRequiredField {
                                    field_name: "router_return_url",
                                    context: Default::default(),
                                })?,
                            skip_three_d_secure: !item
                                .router_data
                                .resource_common_data
                                .is_three_ds(),
                        },
                        card_information: CardInformation {
                            card_number: card_data.card_number.clone(),
                            expiry_month: card_data.card_exp_month.clone(),
                            expiry_year: card_data.get_expiry_year_4_digit(),
                            cvv: if card_data.card_cvc.clone().expose().is_empty() {
                                None
                            } else {
                                Some(card_data.card_cvc.clone())
                            },
                            cardholder_name: card_data
                                .get_cardholder_name()
                                .or(item
                                    .router_data
                                    .resource_common_data
                                    .get_payment_billing_full_name())
                                .change_context(IntegrationError::MissingRequiredField {
                                    field_name: "billing.full_name",
                                    context: Default::default(),
                                })?,
                            cardholder_email: item
                                .router_data
                                .resource_common_data
                                .get_billing_email()
                                .or(item.router_data.request.get_email())
                                .change_context(IntegrationError::MissingRequiredField {
                                    field_name: "billing.email",
                                    context: Default::default(),
                                })?,
                            cardholder_phone_number: item
                                .router_data
                                .resource_common_data
                                .get_billing_phone_number()
                                .change_context(IntegrationError::MissingRequiredField {
                                    field_name: "billing.phone_number",
                                    context: Default::default(),
                                })?,
                        },
                    },
                    reusability: match item.router_data.request.is_mandate_payment() {
                        true => TransactionType::MultipleUse,
                        false => TransactionType::OneTimeUse,
                    },
                })),
                payment_method_id: None,
                channel_properties: None,
            }),
            _ => Err(IntegrationError::not_implemented(
                get_unimplemented_payment_method_error_message("xendit"),
            )
            .into()),
        }
    }
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<XenditPaymentResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;
    fn try_from(
        item: ResponseRouterData<XenditPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        let status = map_payment_response_to_attempt_status(
            response.clone(),
            is_auto_capture(&router_data.request).change_context(
                crate::utils::response_handling_fail_for_connector(item.http_code, "xendit"),
            )?,
        );

        let payment_response = if status == common_enums::AttemptStatus::Failure {
            Err(ErrorResponse {
                code: response
                    .failure_code
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: response
                    .failure_code
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: Some(
                    response
                        .failure_code
                        .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                ),
                attempt_status: None,
                connector_transaction_id: Some(response.id.clone()),
                status_code: http_code,
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            })
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
                redirection_data: match response.actions {
                    Some(actions) if !actions.is_empty() => actions.first().map(|single_action| {
                        Box::new(RedirectForm::Form {
                            endpoint: single_action.url.clone(),
                            method: match single_action.method {
                                MethodType::Get => Method::Get,
                                MethodType::Post => Method::Post,
                            },
                            form_fields: HashMap::new(),
                        })
                    }),
                    _ => None,
                },
                mandate_reference: match is_mandate_payment(&router_data.request) {
                    true => Some(Box::new(MandateReference {
                        connector_mandate_id: Some(response.payment_method.id.expose()),
                        payment_method_id: None,
                        connector_mandate_request_reference_id: None,
                    })),
                    false => None,
                },
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(response.reference_id.peek().to_string()),
                incremental_authorization_allowed: None,
                status_code: http_code,
            })
        };

        let response_amount =
            XenditAmountConvertor::convert_back(response.amount, response.currency)
                .change_context(crate::utils::response_handling_fail_for_connector(
                    item.http_code,
                    "xendit",
                ))?;

        let response_integrity_object = Some(AuthoriseIntegrityObject {
            amount: response_amount,
            currency: response.currency,
        });

        Ok(Self {
            response: payment_response,
            request: PaymentsAuthorizeData {
                integrity_object: response_integrity_object,
                ..router_data.request
            },
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<XenditResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;
    fn try_from(item: ResponseRouterData<XenditResponse, Self>) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        match response {
            XenditResponse::Payment(payment_response) => {
                let status = map_payment_response_to_attempt_status(
                    payment_response.clone(),
                    is_auto_capture_psync_response(&router_data.request)?,
                );
                let response = if status == common_enums::AttemptStatus::Failure {
                    Err(ErrorResponse {
                        code: payment_response
                            .failure_code
                            .clone()
                            .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                        message: payment_response
                            .failure_code
                            .clone()
                            .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                        reason: Some(
                            payment_response
                                .failure_code
                                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                        ),
                        attempt_status: None,
                        connector_transaction_id: Some(payment_response.id.clone()),
                        status_code: http_code,
                        network_advice_code: None,
                        network_decline_code: None,
                        network_error_message: None,
                    })
                } else {
                    Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::NoResponseId,
                        redirection_data: None,
                        mandate_reference: None,
                        connector_metadata: None,
                        network_txn_id: None,
                        connector_response_reference_id: None,
                        incremental_authorization_allowed: None,
                        status_code: http_code,
                    })
                };

                Ok(Self {
                    response,
                    resource_common_data: PaymentFlowData {
                        status,
                        ..router_data.resource_common_data
                    },
                    ..router_data
                })
            }
            XenditResponse::Webhook(webhook_event) => {
                let status = match webhook_event.event {
                    XenditEventType::PaymentSucceeded | XenditEventType::CaptureSucceeded => {
                        common_enums::AttemptStatus::Charged
                    }
                    XenditEventType::PaymentAwaitingCapture => {
                        common_enums::AttemptStatus::Authorized
                    }
                    XenditEventType::PaymentFailed | XenditEventType::CaptureFailed => {
                        common_enums::AttemptStatus::Failure
                    }
                };
                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        ..router_data.resource_common_data
                    },
                    ..router_data
                })
            }
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        XenditRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for XenditPaymentsCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: XenditRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount = XenditAmountConvertor::convert(
            item.router_data.request.minor_amount_to_capture,
            item.router_data.request.currency,
        )
        .change_context(IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        })?;
        Ok(Self {
            capture_amount: amount,
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct XenditCaptureResponse {
    pub id: String,
    pub status: PaymentStatus,
    pub actions: Option<Vec<Action>>,
    pub payment_method: PaymentMethodInfo,
    pub failure_code: Option<String>,
    pub reference_id: Secret<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct XenditPaymentsCaptureRequest {
    pub capture_amount: FloatMajorUnit,
}

impl<F> TryFrom<ResponseRouterData<XenditCaptureResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;
    fn try_from(
        item: ResponseRouterData<XenditCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = common_enums::AttemptStatus::from(item.response.status);
        let response = if status == common_enums::AttemptStatus::Failure {
            Err(ErrorResponse {
                code: item
                    .response
                    .failure_code
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: item
                    .response
                    .failure_code
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: Some(
                    item.response
                        .failure_code
                        .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                ),
                attempt_status: None,
                connector_transaction_id: None,
                status_code: item.http_code,
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            })
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::NoResponseId,
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(
                    item.response.reference_id.peek().to_string(),
                ),
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

#[derive(Default, Debug, Serialize)]
pub struct XenditRefundRequest {
    pub amount: FloatMajorUnit,
    pub payment_request_id: String,
    pub reason: String,
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<XenditRouterData<RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>, T>>
    for XenditRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: XenditRouterData<
            RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount = XenditAmountConvertor::convert(
            item.router_data.request.minor_refund_amount,
            item.router_data.request.currency,
        )
        .change_context(IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        })?;
        Ok(Self {
            amount: amount.to_owned(),
            payment_request_id: item.router_data.request.connector_transaction_id.clone(),
            reason: "REQUESTED_BY_CUSTOMER".to_string(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefundResponse {
    pub id: String,
    pub status: RefundStatus,
    pub amount: FloatMajorUnit,
    pub currency: Currency,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RefundStatus {
    RequiresAction,
    Succeeded,
    Failed,
    Pending,
    Cancelled,
}

impl<F> TryFrom<ResponseRouterData<RefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;
    fn try_from(item: ResponseRouterData<RefundResponse, Self>) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;

        let response_amount =
            XenditAmountConvertor::convert_back(response.amount, response.currency)
                .change_context(crate::utils::response_handling_fail_for_connector(
                    item.http_code,
                    "xendit",
                ))?;

        let response_integrity_object = {
            Some(RefundIntegrityObject {
                refund_amount: response_amount,
                currency: response.currency,
            })
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: response.id,
                refund_status: common_enums::RefundStatus::from(response.status),
                status_code: http_code,
            }),
            request: RefundsData {
                integrity_object: response_integrity_object,
                ..router_data.request
            },
            ..router_data
        })
    }
}

impl From<RefundStatus> for common_enums::RefundStatus {
    fn from(item: RefundStatus) -> Self {
        match item {
            RefundStatus::Succeeded => Self::Success,
            RefundStatus::Failed | RefundStatus::Cancelled => Self::Failure,
            RefundStatus::Pending | RefundStatus::RequiresAction => Self::Pending,
        }
    }
}

impl<F> TryFrom<ResponseRouterData<RefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;
    fn try_from(item: ResponseRouterData<RefundResponse, Self>) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: response.id,
                refund_status: common_enums::RefundStatus::from(response.status),
                status_code: http_code,
            }),
            ..router_data
        })
    }
}

fn is_mandate_payment<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: &PaymentsAuthorizeData<T>,
) -> bool {
    (item.setup_future_usage == Some(common_enums::enums::FutureUsage::OffSession))
        || item
            .mandate_id
            .as_ref()
            .and_then(|mandate_ids| mandate_ids.mandate_reference_id.as_ref())
            .is_some()
}
