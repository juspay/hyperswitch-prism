use common_utils::{types::FloatMajorUnit, Method};
use domain_types::{
    connector_flow::{Authorize, PSync, RSync, Refund, Void},
    connector_types::{
        EventType, PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsResponseData,
        PaymentsSyncData, RawConnectorStatus, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId, SettlementStatus,
    },
    errors::{self, ConnectorError, IntegrationError},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, WalletData},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{connectors::maya::MayaRouterData, types::ResponseRouterData};

#[derive(Debug, Clone)]
pub struct MayaAuthType {
    pub public_key: Secret<String>,
    pub secret_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for MayaAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Maya {
                public_key,
                secret_key,
                ..
            } => Ok(Self {
                public_key: public_key.to_owned(),
                secret_key: secret_key.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext {
                        additional_context: Some(
                            "Maya requires public_key and secret_key authentication".to_string()
                        ),
                        suggested_action: Some(
                            "Check that the connector account is configured with Maya's public_key and secret_key credentials".to_string(),
                        ),
                        doc_url:Some("https://developers.maya.ph/reference/api-authentication-methods".to_string()),
                    }
                }
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MayaErrorParameter {
    pub description: String,
    pub field: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MayaErrorResponse {
    pub code: String,
    /// Maya uses both `message` (payment errors) and `error` (auth/endpoint errors)
    /// for the human-readable description.
    #[serde(alias = "error")]
    pub message: String,
    #[serde(default)]
    pub parameters: Option<Vec<MayaErrorParameter>>,
    /// Optional correlation/reference id returned by some Maya error responses.
    #[serde(default)]
    pub reference: Option<String>,
}

// =============================================================================
// AUTHORIZE FLOW TYPES AND TRANSFORMERS
// =============================================================================

/// Maya "Create Single Payment" request body.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MayaPaymentsRequest {
    pub total_amount: MayaTotalAmount,
    pub redirect_url: MayaRedirectUrl,
    pub request_reference_number: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}

/// `totalAmount` object carried by most Maya request bodies (`value` + `currency`,
/// value as a JSON number).
#[derive(Debug, Serialize)]
pub struct MayaTotalAmount {
    pub value: FloatMajorUnit,
    pub currency: common_enums::Currency,
}

/// Maya's "Refund Payment via ID" endpoint expects the refund amount under the
/// `totalAmount.amount` key (rather than the `value` key used elsewhere), also as a
/// JSON number.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MayaRefundTotalAmount {
    pub amount: FloatMajorUnit,
    pub currency: common_enums::Currency,
}

#[derive(Debug, Serialize)]
pub struct MayaRedirectUrl {
    pub success: String,
    pub failure: String,
    pub cancel: String,
}

/// Maya "Create Single Payment" response body.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MayaPaymentsResponse {
    pub payment_id: String,
    pub redirect_url: String,
}

/// Maya payment / webhook status values (used by PSync / webhooks).
///
/// Includes both the documented payment-object statuses and the webhook event
/// names returned by Maya (e.g. `CHECKOUT_DROPOUT`, `3DS_PAYMENT_SUCCESS`).
#[derive(Debug, Serialize, Deserialize, Clone, strum::Display)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum MayaPaymentStatus {
    PendingToken,
    PendingPayment,
    ForAuthentication,
    Authenticating,
    AuthSuccess,
    AuthFailed,
    PaymentProcessing,
    PaymentSuccess,
    PaymentFailed,
    PaymentExpired,
    PaymentCancelled,
    Voided,
    Refunded,
    // Webhook-specific event names. Explicit renames are required for values
    // that contain numbers (3DS) or do not round-trip cleanly through serde's
    // SCREAMING_SNAKE_CASE conversion.
    #[serde(rename = "CHECKOUT_DROPOUT")]
    CheckOutDropout,
    #[serde(rename = "3DS_PAYMENT_FAILURE")]
    ThreeDsPaymentFailure,
    #[serde(rename = "3DS_PAYMENT_DROPOUT")]
    ThreeDsPaymentDropout,
    #[serde(rename = "CHECKOUT_SUCCESS")]
    CheckOutSuccess,
    #[serde(rename = "CHECKOUT_FAILURE")]
    CheckOutFailure,
    #[serde(rename = "3DS_PAYMENT_SUCCESS")]
    ThreeDsPaymentSuccess,
}

impl From<MayaPaymentStatus> for common_enums::AttemptStatus {
    fn from(status: MayaPaymentStatus) -> Self {
        match status {
            MayaPaymentStatus::PendingToken | MayaPaymentStatus::PendingPayment => {
                Self::PaymentMethodAwaited
            }
            MayaPaymentStatus::ForAuthentication | MayaPaymentStatus::Authenticating => {
                Self::AuthenticationPending
            }
            MayaPaymentStatus::AuthSuccess | MayaPaymentStatus::ThreeDsPaymentSuccess => {
                Self::AuthenticationSuccessful
            }
            MayaPaymentStatus::AuthFailed
            | MayaPaymentStatus::ThreeDsPaymentFailure
            | MayaPaymentStatus::ThreeDsPaymentDropout
            | MayaPaymentStatus::CheckOutDropout => Self::AuthenticationFailed,
            MayaPaymentStatus::PaymentProcessing | MayaPaymentStatus::CheckOutSuccess => {
                Self::Authorizing
            }
            MayaPaymentStatus::PaymentSuccess => Self::Charged,
            MayaPaymentStatus::PaymentFailed | MayaPaymentStatus::CheckOutFailure => {
                Self::AuthorizationFailed
            }
            MayaPaymentStatus::PaymentCancelled | MayaPaymentStatus::Voided => Self::Voided,
            MayaPaymentStatus::PaymentExpired => Self::Expired,
            MayaPaymentStatus::Refunded => Self::AutoRefunded,
        }
    }
}

impl From<MayaPaymentStatus> for EventType {
    fn from(status: MayaPaymentStatus) -> Self {
        match status {
            MayaPaymentStatus::PaymentSuccess => Self::PaymentIntentSuccess,
            MayaPaymentStatus::AuthSuccess | MayaPaymentStatus::ThreeDsPaymentSuccess => {
                Self::PaymentIntentAuthorizationSuccess
            }
            MayaPaymentStatus::PaymentCancelled | MayaPaymentStatus::Voided => {
                Self::PaymentIntentCancelled
            }
            MayaPaymentStatus::PaymentExpired => Self::PaymentIntentExpired,
            MayaPaymentStatus::PaymentProcessing
            | MayaPaymentStatus::CheckOutSuccess
            | MayaPaymentStatus::PendingToken
            | MayaPaymentStatus::PendingPayment
            | MayaPaymentStatus::ForAuthentication
            | MayaPaymentStatus::Authenticating => Self::PaymentIntentProcessing,
            MayaPaymentStatus::PaymentFailed
            | MayaPaymentStatus::CheckOutFailure
            | MayaPaymentStatus::AuthFailed
            | MayaPaymentStatus::ThreeDsPaymentFailure
            | MayaPaymentStatus::CheckOutDropout
            | MayaPaymentStatus::ThreeDsPaymentDropout => Self::PaymentIntentFailure,
            MayaPaymentStatus::Refunded => Self::PaymentIntentSuccess,
        }
    }
}

// =============================================================================
// WEBHOOK TYPES AND HELPERS
// =============================================================================

/// Pay-with-Maya payment object.
///
/// Used for both incoming webhooks and the response from the payment-by-ID
/// endpoint (`GET /payments/v1/payments/{paymentId}`).
///
/// Only the fields relevant to the Pay-with-Maya redirect flow are modeled;
/// Checkout / Payment-Facilitator / Vault fields are intentionally omitted.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MayaWebhookBody {
    pub id: String,
    pub status: MayaPaymentStatus,
    #[serde(default)]
    pub request_reference_number: Option<String>,
    #[serde(default)]
    pub transaction_reference_number: Option<String>,
    #[serde(default)]
    pub receipt_number: Option<String>,
    #[serde(default)]
    pub approval_code: Option<String>,
    #[serde(default)]
    pub error_code: Option<String>,
    #[serde(default)]
    pub error_message: Option<String>,
    #[serde(default)]
    pub can_void: Option<bool>,
    #[serde(default)]
    pub can_refund: Option<bool>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

impl MayaWebhookBody {
    /// Merchant reference echoed by Maya, distinct from Maya's payment `id`.
    pub fn connector_response_reference_id(&self) -> Option<String> {
        self.request_reference_number.clone()
    }
}

fn maya_raw_connector_status(payment: &MayaWebhookBody) -> RawConnectorStatus {
    RawConnectorStatus {
        code: payment.error_code.clone(),
        message: Some(payment.status.to_string()),
        reason: None,
    }
}

/// Derive Maya's settlement phase from the operation flags returned by payment inquiry.
///
/// `canVoid` takes precedence. A payment is considered settled only when Maya explicitly disables
/// void and enables refund.
fn maya_settlement_status(
    can_void: Option<bool>,
    can_refund: Option<bool>,
) -> Option<SettlementStatus> {
    match (can_void, can_refund) {
        (Some(true), _) => Some(SettlementStatus::NotSettled),
        (Some(false), Some(true)) => Some(SettlementStatus::Settled),
        _ => None,
    }
}

// =============================================================================
// VOID FLOW TYPES AND TRANSFORMERS
// =============================================================================

/// Maya "Void Payment" request body.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MayaVoidRequest {
    pub reason: String,
}

/// Maya void status values.
#[derive(Debug, Clone, Serialize, Deserialize, strum::Display)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum MayaVoidStatus {
    Success,
    Failed,
    Pending,
}

impl From<MayaVoidStatus> for common_enums::AttemptStatus {
    fn from(status: MayaVoidStatus) -> Self {
        match status {
            MayaVoidStatus::Success => Self::Voided,
            MayaVoidStatus::Failed => Self::VoidFailed,
            MayaVoidStatus::Pending => Self::VoidInitiated,
        }
    }
}

/// Maya "Void Payment" response body.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MayaVoidResponse {
    pub id: String,
    /// `status` is a required field per Maya's void-payment API docs; a
    /// response without it fails deserialization instead of being
    /// optimistically treated as a successful void.
    pub status: MayaVoidStatus,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub request_reference_number: Option<String>,
    #[serde(default)]
    pub payment: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MayaRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for MayaVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: MayaRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // `reason` is mandatory per Maya's void-payment API; hard-coded. The payment to void
        // is identified solely by the `{payment_id}` path segment (see `get_url` in maya.rs).
        Ok(Self {
            reason: "Customer requested void".to_string(),
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MayaRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for MayaPaymentsRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: MayaRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;

        // Maya only supports the PayMaya wallet redirect flow; any other payment
        // method would be silently turned into a wallet checkout, so fail fast.
        match &router_data.request.payment_method_data {
            PaymentMethodData::Wallet(WalletData::PaymayaRedirect(_)) => Ok(()),
            _ => Err(error_stack::report!(IntegrationError::NotImplemented(
                "This payment method is not supported for Maya".to_string(),
                errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Use the PayMaya wallet payment method with Maya".to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "Maya supports only the PayMaya wallet redirect payment method".to_string(),
                    ),
                },
            ))),
        }?;

        let value = item
            .connector
            .amount_converter
            .convert(
                router_data.request.minor_amount,
                router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to convert the Maya payment amount from minor units to major units"
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Verify that the amount and currency are valid for major-unit conversion"
                            .to_string(),
                    ),
                    doc_url: None,
                },
            })
            .attach_printable("Failed to convert amount to Maya major unit")?;

        let return_url = router_data.resource_common_data.return_url.clone().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "return_url",
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Maya is a redirect-only connector; it needs a return_url to send the customer back to after payment"
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Set the `return_url` field in the Authorize request".to_string(),
                    ),
                    doc_url: Some(
                        "https://developers.maya.ph/reference/createv2singlepayment".to_string(),
                    ),
                },
            },
        )?;

        let user_id = router_data
            .request
            .email
            .clone()
            .map(|email| email.peek().to_string());

        Ok(Self {
            total_amount: MayaTotalAmount {
                value,
                currency: router_data.request.currency,
            },
            redirect_url: MayaRedirectUrl {
                success: return_url.clone(),
                failure: return_url.clone(),
                cancel: return_url,
            },
            request_reference_number: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            user_id,
        })
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<MayaPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<MayaPaymentsResponse, Self>) -> Result<Self, Self::Error> {
        let redirect_url = Url::parse(&item.response.redirect_url).change_context(
            ConnectorError::response_deserialization_failed_with_context(
                item.http_code,
                Some("Maya returned an invalid redirectUrl in the authorize response".to_string()),
            ),
        )?;

        let status = common_enums::AttemptStatus::AuthenticationPending;

        // Maya returns only `paymentId` and `redirectUrl` in the create-payment response.
        // The merchant-facing reference is the `requestReferenceNumber` we sent in the
        // request (`connector_request_reference_id`). Surface it as the connector
        // response reference id so it stays consistent across Authorize / PSync / Webhook.
        let connector_response_reference_id = Some(
            item.router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        );

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.payment_id.clone()),
                redirection_data: Some(Box::new(RedirectForm::from((redirect_url, Method::Get)))),
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id,
                incremental_authorization_allowed: None,
                network_txn_link_id: None,
                splits: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                raw_connector_status: None,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl TryFrom<ResponseRouterData<MayaWebhookBody, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<MayaWebhookBody, Self>) -> Result<Self, Self::Error> {
        let connector_request_reference_id = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        let payment = item.response;

        let settlement_status = maya_settlement_status(payment.can_void, payment.can_refund);

        let status = common_enums::AttemptStatus::from(payment.status.clone());

        // Use Maya's payment ID when it is present in the response.
        let resource_id = if !payment.id.is_empty() {
            ResponseId::ConnectorTransactionId(payment.id.clone())
        } else {
            ResponseId::NoResponseId
        };

        let raw_connector_status = maya_raw_connector_status(&payment);

        // `connector_response_reference_id`: the merchant reference number echoed back
        // by Maya (`requestReferenceNumber`). This keeps the reference consistent with
        // what Euler/Juspay uses as `txn_id`.
        let connector_response_reference_id = payment
            .connector_response_reference_id()
            .unwrap_or(connector_request_reference_id.clone());

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id,
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(connector_response_reference_id),
                incremental_authorization_allowed: None,
                network_txn_link_id: None,
                splits: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                settlement_status,
                raw_connector_status: Some(raw_connector_status),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl TryFrom<ResponseRouterData<MayaVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<MayaVoidResponse, Self>) -> Result<Self, Self::Error> {
        let status = common_enums::AttemptStatus::from(item.response.status.clone());

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(void_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(void_id),
                incremental_authorization_allowed: None,
                network_txn_link_id: None,
                splits: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                raw_connector_status: Some(RawConnectorStatus {
                    code: Some(item.response.status.to_string()),
                    message: None,
                    reason: None,
                }),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// REFUND FLOW TYPES AND TRANSFORMERS
// =============================================================================

/// Maya refund status values.
#[derive(Debug, Clone, Serialize, Deserialize, strum::Display)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum MayaRefundStatus {
    Success,
    Failed,
    Pending,
}

impl From<MayaRefundStatus> for common_enums::RefundStatus {
    fn from(status: MayaRefundStatus) -> Self {
        match status {
            MayaRefundStatus::Success => Self::Success,
            MayaRefundStatus::Failed => Self::Failure,
            MayaRefundStatus::Pending => Self::Pending,
        }
    }
}

/// Maya "Refund Payment" request body.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MayaRefundRequest {
    pub total_amount: MayaRefundTotalAmount,
    pub reason: String,
}

/// Maya "Refund Payment" response body.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MayaRefundResponse {
    pub id: String,
    /// `status` is a required field per Maya's refund API docs; a response
    /// without it fails deserialization instead of being optimistically
    /// treated as a successful refund.
    pub status: MayaRefundStatus,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub request_reference_number: Option<String>,
    #[serde(default)]
    pub payment: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// Type alias for the refund-sync flow.
///
/// `create_all_prerequisites!` generates a unique templating struct per
/// response type, so reusing the same type for both `Refund` and `RSync`
/// causes a duplicate-definition error. This alias points to the same
/// underlying deserialization logic while satisfying the macro.
pub type MayaRefundSyncResponse = MayaRefundResponse;

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<MayaRouterData<RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>, T>>
    for MayaRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: MayaRouterData<RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;

        let value = item
            .connector
            .amount_converter
            .convert(
                router_data.request.minor_refund_amount,
                router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to convert the Maya refund amount from minor units to major units"
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Verify that the refund amount and currency are valid for major-unit conversion"
                            .to_string(),
                    ),
                    doc_url: None,
                },
            })
            .attach_printable("Failed to convert refund amount to Maya major unit")?;

        Ok(Self {
            total_amount: MayaRefundTotalAmount {
                amount: value,
                currency: router_data.request.currency,
            },
            // `reason` is mandatory per Maya's refund-via-payment-id API; hard-coded.
            reason: "Customer requested refund".to_string(),
        })
    }
}

impl TryFrom<ResponseRouterData<MayaRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<MayaRefundResponse, Self>) -> Result<Self, Self::Error> {
        let refund_status = common_enums::RefundStatus::from(item.response.status.clone());

        let raw_connector_status = RawConnectorStatus {
            code: Some(item.response.status.to_string()),
            message: None,
            reason: None,
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            resource_common_data: RefundFlowData {
                raw_connector_status: Some(raw_connector_status),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl TryFrom<ResponseRouterData<MayaRefundSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<MayaRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let refund_status = common_enums::RefundStatus::from(item.response.status.clone());

        let raw_connector_status = RawConnectorStatus {
            code: Some(item.response.status.to_string()),
            message: None,
            reason: None,
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            resource_common_data: RefundFlowData {
                raw_connector_status: Some(raw_connector_status),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}
