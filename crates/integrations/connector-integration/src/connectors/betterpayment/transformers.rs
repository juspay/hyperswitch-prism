use common_enums::AttemptStatus;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    request::Method,
    types::FloatMajorUnit,
};
use domain_types::{
    connector_flow::{Authorize, PSync, RSync, Refund},
    connector_types::{
        EventType, PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, ResponseId,
    },
    errors,
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, WalletData},
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
    utils,
};
use error_stack::ResultExt;
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

use crate::{connectors::betterpayment::BetterpaymentRouterData, types::ResponseRouterData};

/// Betterpayment order_id only allows SEPA characters (a-zA-Z0-9/-?():.,'+ and space)
/// and is capped at 35 characters. Underscores are replaced with hyphens; any remaining
/// non-SEPA characters are dropped rather than forwarded.
fn sanitize_order_id(reference: &str) -> String {
    reference
        .chars()
        .map(|c| if c == '_' { '-' } else { c })
        .filter(|c| {
            c.is_ascii_alphanumeric()
                || matches!(
                    c,
                    '/' | '-' | '?' | '(' | ')' | ':' | '.' | ',' | '\'' | '+' | ' '
                )
        })
        .take(35)
        .collect()
}

#[derive(Debug, Clone)]
pub struct BetterpaymentAuthType {
    /// API key — used as the HTTP Basic Auth username.
    pub api_key: Secret<String>,
    /// API password — used as the HTTP Basic Auth password.
    pub key1: Secret<String>,
    /// Outgoing key — used only for webhook signature verification.
    pub api_secret: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for BetterpaymentAuthType {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Betterpayment {
                api_key,
                key1,
                api_secret,
                ..
            } => Ok(Self {
                api_key: api_key.to_owned(),
                key1: key1.to_owned(),
                api_secret: api_secret.to_owned(),
            }),
            _ => Err(error_stack::report!(
                errors::IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext::default()
                }
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BetterpaymentErrorResponse {
    pub error_code: Option<u64>,
    pub message: Option<String>,
}

/// Betterpayment's `payment_type` wire values.
/// To add a new payment method: add a variant here and a matching arm in the
/// `TryFrom` authorize block.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BetterpaymentPaymentType {
    Wero,
}

#[derive(Debug, Serialize)]
pub struct BetterpaymentAuthorizeRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub payment_type: BetterpaymentPaymentType,
    pub order_id: String,
    pub amount: FloatMajorUnit,
    pub currency: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postback_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wero_checkout_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<common_utils::pii::Email>,
    pub address: Secret<String>,
    pub city: Secret<String>,
    pub postal_code: Secret<String>,
    pub country: common_enums::CountryAlpha2,
    pub first_name: Secret<String>,
    pub last_name: Secret<String>,
    #[serde(skip)]
    pub _phantom: std::marker::PhantomData<T>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BetterpaymentRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for BetterpaymentAuthorizeRequest<T>
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: BetterpaymentRouterData<
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
        let request = &router_data.request;

        let amount = utils::convert_amount(
            item.connector.amount_converter,
            request.minor_amount,
            request.currency,
        )
        .change_context(errors::IntegrationError::AmountConversionFailed {
            context: errors::IntegrationErrorContext::default(),
        })?;

        let order_id = sanitize_order_id(
            &router_data
                .resource_common_data
                .connector_request_reference_id,
        );

        match &request.payment_method_data {
            PaymentMethodData::Wallet(WalletData::Wero(_)) => {
                let billing = router_data.resource_common_data.get_billing_address()?;
                let postback_url = request
                    .webhook_url
                    .clone()
                    .ok_or_else(utils::missing_field_err("webhook_url"))?;
                let return_url = request
                    .router_return_url
                    .clone()
                    .ok_or_else(utils::missing_field_err("router_return_url"))?;

                Ok(Self {
                    payment_type: BetterpaymentPaymentType::Wero,
                    order_id: order_id.clone(),
                    amount,
                    currency: request.currency.to_string(),
                    merchant_reference: Some(order_id),
                    postback_url: Some(postback_url),
                    success_url: Some(return_url.clone()),
                    error_url: Some(return_url.clone()),
                    wero_checkout_url: Some(return_url),
                    email: request.email.clone(),
                    address: billing.get_line1()?.clone(),
                    city: billing.get_city()?.clone(),
                    postal_code: billing.get_zip()?.clone(),
                    country: *billing.get_country()?,
                    first_name: billing.get_first_name()?.clone(),
                    last_name: billing.get_last_name()?.clone(),
                    _phantom: std::marker::PhantomData,
                })
            }
            _ => Err(error_stack::report!(
                errors::IntegrationError::NotImplemented(
                    utils::get_unimplemented_payment_method_error_message("Betterpayment"),
                    errors::IntegrationErrorContext::default(),
                )
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BetterpaymentPaymentStatus {
    Started,
    Pending,
    Completed,
    Declined,
    Canceled,
    Error,
    #[default]
    #[serde(other)]
    Unknown,
}

impl From<BetterpaymentPaymentStatus> for AttemptStatus {
    fn from(status: BetterpaymentPaymentStatus) -> Self {
        match status {
            BetterpaymentPaymentStatus::Started => Self::AuthenticationPending,
            BetterpaymentPaymentStatus::Pending => Self::Pending,
            BetterpaymentPaymentStatus::Completed => Self::Charged,
            BetterpaymentPaymentStatus::Declined => Self::Failure,
            BetterpaymentPaymentStatus::Canceled => Self::Voided,
            BetterpaymentPaymentStatus::Error => Self::Failure,
            BetterpaymentPaymentStatus::Unknown => Self::Pending,
        }
    }
}

/// Typed wire value for the `client_action` field.
/// Extend with new variants as Betterpayment adds more response flows
/// (e.g. `QrCode`, `PresentToShopper`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BetterpaymentClientAction {
    Redirect,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BetterpaymentActionData {
    pub url: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BetterpaymentAuthorizeResponse {
    pub transaction_id: Option<String>,
    #[serde(default)]
    pub status: BetterpaymentPaymentStatus,
    pub order_id: Option<String>,
    pub error_code: Option<i64>,
    pub message: Option<String>,
    pub error_message: Option<String>,
    pub client_action: Option<BetterpaymentClientAction>,
    pub action_data: Option<BetterpaymentActionData>,
}

impl<T: PaymentMethodDataTypes>
    TryFrom<
        ResponseRouterData<
            BetterpaymentAuthorizeResponse,
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        >,
    > for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            BetterpaymentAuthorizeResponse,
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        >,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response: connector_response,
            router_data,
            http_code,
        } = item;
        let status = AttemptStatus::from(connector_response.status);

        // IN-BAND 2xx FAILURE: Betterpayment returns HTTP 200 with `declined`
        // or `error` status. Turn those into Err(ErrorResponse) here — the
        // shared `build_error_response` never sees 2xx bodies.
        let response = if utils::is_payment_failure(status) {
            Err(ErrorResponse {
                code: connector_response
                    .error_code
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: connector_response
                    .message
                    .clone()
                    .or_else(|| connector_response.error_message.clone())
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: connector_response
                    .message
                    .or(connector_response.error_message),
                status_code: http_code,
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: connector_response.transaction_id,
                ..Default::default()
            })
        } else {
            let redirection_data = match connector_response.client_action {
                Some(BetterpaymentClientAction::Redirect) => connector_response
                    .action_data
                    .and_then(|ad| ad.url)
                    .map(|url| {
                        Box::new(RedirectForm::Form {
                            endpoint: url,
                            method: Method::Get,
                            form_fields: Default::default(),
                        })
                    }),
                None | Some(BetterpaymentClientAction::Unknown) => None,
            };
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    connector_response
                        .transaction_id
                        .ok_or_else(utils::missing_field_err("transaction_id"))
                        .change_context(errors::ConnectorError::ResponseDeserializationFailed {
                            context: Default::default(),
                        })?,
                ),
                redirection_data,
                connector_metadata: None,
                mandate_reference: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: connector_response.order_id,
                incremental_authorization_allowed: None,
                splits: None,
                status_code: http_code,
                payment_account_reference: None,
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
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BetterpaymentPSyncResponse {
    pub transaction_id: Option<String>,
    pub status: BetterpaymentPaymentStatus,
    pub order_id: Option<String>,
    pub amount: Option<f64>,
    pub error_code: Option<u64>,
    pub message: Option<String>,
}

impl
    TryFrom<
        ResponseRouterData<
            BetterpaymentPSyncResponse,
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        >,
    > for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            BetterpaymentPSyncResponse,
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response: connector_response,
            router_data,
            http_code,
        } = item;
        let status = AttemptStatus::from(connector_response.status);

        let response = if utils::is_payment_failure(status) {
            Err(ErrorResponse {
                code: connector_response
                    .error_code
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: connector_response
                    .message
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: connector_response.message,
                status_code: http_code,
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: connector_response.transaction_id,
                ..Default::default()
            })
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    connector_response
                        .transaction_id
                        .ok_or_else(utils::missing_field_err("transaction_id"))
                        .change_context(errors::ConnectorError::ResponseDeserializationFailed {
                            context: Default::default(),
                        })?,
                ),
                redirection_data: None,
                connector_metadata: None,
                mandate_reference: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: connector_response.order_id,
                incremental_authorization_allowed: None,
                splits: None,
                status_code: http_code,
                payment_account_reference: None,
            })
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            response,
            ..router_data
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BetterpaymentRefundStatus {
    Started,
    Successful,
    Error,
    #[serde(other)]
    Unknown,
}

impl From<BetterpaymentRefundStatus> for common_enums::RefundStatus {
    fn from(status: BetterpaymentRefundStatus) -> Self {
        match status {
            BetterpaymentRefundStatus::Started => Self::Pending,
            BetterpaymentRefundStatus::Successful => Self::Success,
            BetterpaymentRefundStatus::Error => Self::Failure,
            BetterpaymentRefundStatus::Unknown => Self::Pending,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct BetterpaymentRefundRequest {
    pub transaction_id: String,
    pub amount: FloatMajorUnit,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BetterpaymentRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for BetterpaymentRefundRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: BetterpaymentRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount = utils::convert_amount(
            item.connector.amount_converter,
            item.router_data.request.minor_refund_amount,
            item.router_data.request.currency,
        )
        .change_context(errors::IntegrationError::AmountConversionFailed {
            context: errors::IntegrationErrorContext::default(),
        })?;

        Ok(Self {
            transaction_id: item.router_data.request.connector_transaction_id.clone(),
            amount,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BetterpaymentRefundResponse {
    pub transaction_id: Option<String>,
    pub refund_id: Option<String>,
    pub status: BetterpaymentRefundStatus,
    pub status_code: Option<i64>,
    pub error_code: Option<u64>,
    pub message: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(transparent)]
pub struct BetterpaymentRSyncResponse(pub Vec<BetterpaymentRefundResponse>);

impl BetterpaymentRSyncResponse {
    fn into_refund(
        self,
        refund_id: &str,
    ) -> Result<BetterpaymentRefundResponse, error_stack::Report<errors::ConnectorError>> {
        self.0
            .into_iter()
            .find(|refund| refund.refund_id.as_deref() == Some(refund_id) && !refund_id.is_empty())
            .ok_or_else(|| {
                error_stack::report!(errors::ConnectorError::UnexpectedResponseError {
                    context: Default::default(),
                })
                .attach_printable("Requested refund was absent from Betterpayment's refund list")
            })
    }
}

impl
    TryFrom<
        ResponseRouterData<
            BetterpaymentRSyncResponse,
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        >,
    > for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BetterpaymentRSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        let refund = response.into_refund(&router_data.request.connector_refund_id)?;
        Self::try_from(ResponseRouterData {
            response: refund,
            router_data,
            http_code,
        })
    }
}

impl<F, Req>
    TryFrom<
        ResponseRouterData<
            BetterpaymentRefundResponse,
            RouterDataV2<F, RefundFlowData, Req, RefundsResponseData>,
        >,
    > for RouterDataV2<F, RefundFlowData, Req, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            BetterpaymentRefundResponse,
            RouterDataV2<F, RefundFlowData, Req, RefundsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response: refund,
            router_data,
            http_code,
        } = item;
        let refund_status = common_enums::RefundStatus::from(refund.status);

        let response = if matches!(refund_status, common_enums::RefundStatus::Failure) {
            Err(ErrorResponse {
                code: refund
                    .error_code
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: refund
                    .message
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: refund.message,
                status_code: http_code,
                attempt_status: Some(FlowStatus::Refund(refund_status)),
                connector_transaction_id: refund.transaction_id,
                ..Default::default()
            })
        } else {
            let connector_refund_id =
                refund
                    .refund_id
                    .filter(|id| !id.is_empty())
                    .ok_or_else(|| {
                        error_stack::report!(errors::ConnectorError::UnexpectedResponseError {
                            context: Default::default(),
                        })
                        .attach_printable("Missing Betterpayment refund_id")
                    })?;

            Ok(RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: http_code,
                acquirer_reference_number: None,
            })
        };

        Ok(Self {
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..router_data.resource_common_data
            },
            response,
            ..router_data
        })
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct BetterpaymentWebhookBody {
    pub transaction_id: String,
    pub payment_type: Option<String>,
    pub status_code: Option<i64>,
    pub status: BetterpaymentWebhookStatus,
    pub order_id: String,
    pub amount: Option<f64>,
    pub currency: Option<String>,
    pub message: Option<String>,
    pub reason_code: Option<String>,
    pub additional_transaction_data: Option<String>,
    pub remittance_info: Option<String>,
    pub wallet_activity_reference: Option<String>,
    pub wallet_account_reference: Option<String>,
    pub payer_location: Option<String>,
    // Refund-only fields; absent on payment postbacks.
    pub refund_id: Option<String>,
    pub refund_status_code: Option<i64>,
    pub refund_reason_code: Option<String>,
    pub refund_wallet_activity_reference: Option<String>,
    pub refund_remittance_info: Option<String>,
    pub checksum: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BetterpaymentWebhookStatus {
    Started,
    Pending,
    Completed,
    Declined,
    Canceled,
    Error,
    #[serde(other)]
    Unknown,
}

impl From<BetterpaymentWebhookStatus> for EventType {
    fn from(status: BetterpaymentWebhookStatus) -> Self {
        match status {
            BetterpaymentWebhookStatus::Completed => Self::PaymentIntentSuccess,
            BetterpaymentWebhookStatus::Declined | BetterpaymentWebhookStatus::Error => {
                Self::PaymentIntentFailure
            }
            BetterpaymentWebhookStatus::Started | BetterpaymentWebhookStatus::Pending => {
                Self::PaymentIntentProcessing
            }
            BetterpaymentWebhookStatus::Canceled => Self::PaymentIntentCancelled,
            BetterpaymentWebhookStatus::Unknown => Self::IncomingWebhookEventUnspecified,
        }
    }
}

impl From<BetterpaymentWebhookStatus> for AttemptStatus {
    fn from(status: BetterpaymentWebhookStatus) -> Self {
        match status {
            BetterpaymentWebhookStatus::Completed => Self::Charged,
            BetterpaymentWebhookStatus::Declined | BetterpaymentWebhookStatus::Error => {
                Self::Failure
            }
            BetterpaymentWebhookStatus::Started | BetterpaymentWebhookStatus::Pending => {
                Self::Pending
            }
            BetterpaymentWebhookStatus::Canceled => Self::Voided,
            BetterpaymentWebhookStatus::Unknown => Self::Pending,
        }
    }
}

/// Derives the UCS EventType from a parsed webhook body.
/// Refund postbacks are identified by the presence of `refund_id`.
pub fn get_webhook_event_type(body: &BetterpaymentWebhookBody) -> EventType {
    if body.refund_id.is_some() {
        return get_refund_webhook_event_type(body.refund_status_code);
    }
    EventType::from(body.status.clone())
}

/// Maps Betterpayment's `refund_status_code` to a UCS EventType.
/// 0 = Started (processing), 1 = Successful, anything else = failure.
pub fn get_refund_webhook_event_type(refund_status_code: Option<i64>) -> EventType {
    match refund_status_code {
        Some(0) => EventType::RefundProcessing,
        Some(1) => EventType::RefundSuccess,
        _ => EventType::RefundFailure,
    }
}

/// Maps Betterpayment's `refund_status_code` to a UCS RefundStatus.
pub fn get_refund_webhook_status(refund_status_code: Option<i64>) -> common_enums::RefundStatus {
    match refund_status_code {
        Some(0) => common_enums::RefundStatus::Pending,
        Some(1) => common_enums::RefundStatus::Success,
        _ => common_enums::RefundStatus::Failure,
    }
}
