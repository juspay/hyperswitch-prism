use common_utils::{types::StringMajorUnit, Method};
use domain_types::{
    connector_flow::{Authorize, PSync, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsResponseData,
        PaymentsSyncData, ResponseId,
    },
    errors::{self, ConnectorError, IntegrationError},
    payment_method_data::PaymentMethodDataTypes,
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeOptionInterface, PeekInterface, Secret};
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
                    context: errors::IntegrationErrorContext::default()
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct MayaTotalAmount {
    pub value: StringMajorUnit,
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

/// Maya "Retrieve Payment via Request Reference Number (RRN)" response body.
///
/// The RRN endpoint (`GET /payments/v1/payment-rrns/{rrn}`) returns an **array** of
/// payment objects sharing the queried RRN. Each element has the same shape as a
/// Maya webhook payment body, so [`MayaWebhookBody`] is reused for the element
/// type — this gives us the [`payment_status`](MayaWebhookBody::payment_status)
/// helper for effective-status extraction (prefer `paymentStatus`, fall back to
/// `status`).
///
/// A permissive deserializer accepts both a bare JSON array (`[...]`, the shape
/// documented by Maya) and a wrapper object containing an array under a common key
/// (`{ "data": [...] }`, `{ "payments": [...] }`, `{ "results": [...] }`).
#[derive(Debug, Clone, Serialize)]
pub struct MayaRrnSyncResponse(pub Vec<MayaWebhookBody>);

impl<'de> Deserialize<'de> for MayaRrnSyncResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        let value = serde_json::Value::deserialize(deserializer)?;

        // Bare JSON array (the documented Maya shape).
        if value.is_array() {
            let items = serde_json::from_value::<Vec<MayaWebhookBody>>(value).map_err(|e| {
                D::Error::custom(format!("failed to deserialize Maya RRN array: {e}"))
            })?;
            return Ok(Self(items));
        }

        // Defensive fallback: wrapper object containing the array under a common key.
        if let serde_json::Value::Object(map) = &value {
            for key in ["data", "payments", "results", "items"] {
                if let Some(inner) = map.get(key) {
                    if inner.is_array() {
                        let items = serde_json::from_value::<Vec<MayaWebhookBody>>(inner.clone())
                            .map_err(|e| {
                            D::Error::custom(format!(
                                "failed to deserialize Maya RRN array in '{key}': {e}"
                            ))
                        })?;
                        return Ok(Self(items));
                    }
                }
            }
        }

        Err(D::Error::custom(
            "expected a JSON array or a wrapper object containing an array for the Maya RRN sync response",
        ))
    }
}

/// Maya payment status values (used by PSync / webhooks).
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
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
            MayaPaymentStatus::AuthSuccess => Self::AuthenticationSuccessful,
            MayaPaymentStatus::AuthFailed => Self::AuthenticationFailed,
            MayaPaymentStatus::PaymentProcessing => Self::Authorizing,
            MayaPaymentStatus::PaymentSuccess => Self::Charged,
            MayaPaymentStatus::PaymentFailed => Self::AuthorizationFailed,
            MayaPaymentStatus::PaymentCancelled | MayaPaymentStatus::Voided => Self::Voided,
            MayaPaymentStatus::PaymentExpired => Self::Expired,
            MayaPaymentStatus::Refunded => Self::AutoRefunded,
        }
    }
}

// =============================================================================
// WEBHOOK TYPES AND HELPERS
// =============================================================================

/// Pay-with-Maya payment object.
///
/// Used for both incoming webhooks and the elements returned by the RRN-based
/// payment-status endpoint (`GET /payments/v1/payment-rrns/{rrn}`).
///
/// Only the fields relevant to the Pay-with-Maya redirect flow are modeled;
/// Checkout / Payment-Facilitator / Vault fields are intentionally omitted.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MayaWebhookBody {
    pub id: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub payment_status: Option<String>,
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
    pub metadata: Option<serde_json::Value>,
}

impl MayaWebhookBody {
    /// Effective payment status string.
    ///
    /// Prefers the explicit `paymentStatus` field, falling back to `status`.
    pub fn payment_status(&self) -> Option<&str> {
        self.payment_status.as_deref().or(self.status.as_deref())
    }
}

// =============================================================================
// VOID FLOW TYPES AND TRANSFORMERS
// =============================================================================

/// Maya "Cancel Payment" request body.
///
/// The endpoint does not require any fields; we serialize it as an empty object.
#[derive(Debug, Serialize)]
pub struct MayaVoidRequest {}

/// Maya "Cancel Payment" response body.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MayaVoidResponse {
    pub payment_status: MayaPaymentStatus,
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
        Ok(Self {})
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

        let value = item
            .connector
            .amount_converter
            .convert(
                router_data.request.minor_amount,
                router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })
            .attach_printable("Failed to convert amount to Maya major unit")?;

        let return_url = router_data.resource_common_data.return_url.clone().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "return_url",
                context: Default::default(),
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
            metadata: router_data.request.metadata.clone().expose_option(),
        })
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<MayaPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<MayaPaymentsResponse, Self>) -> Result<Self, Self::Error> {
        let redirect_url = Url::parse(&item.response.redirect_url).change_context(
            ConnectorError::ResponseDeserializationFailed {
                context: Default::default(),
            },
        )?;

        let status = common_enums::AttemptStatus::Pending;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.payment_id.clone()),
                redirection_data: Some(Box::new(RedirectForm::from((redirect_url, Method::Get)))),
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.payment_id),
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

impl TryFrom<ResponseRouterData<MayaRrnSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<MayaRrnSyncResponse, Self>) -> Result<Self, Self::Error> {
        // The Maya paymentId from the request, used as a fallback for `resource_id`
        // when the response does not carry one.
        let request_txn_id = item
            .router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .ok();

        // The RRN endpoint returns an array of payments that share the queried RRN.
        // Pick the first element.
        let payment = item
            .response
            .0
            .into_iter()
            .next()
            .ok_or_else(|| ConnectorError::response_handling_failed(item.http_code))?;

        // Effective status: prefer `paymentStatus`, fall back to `status`
        // (same logic as the webhook body).
        let effective_status = payment.payment_status().unwrap_or("PAYMENT_FAILED");
        let maya_payment_status: MayaPaymentStatus =
            serde_json::from_str(&format!("\"{effective_status}\"")).change_context(
                ConnectorError::response_deserialization_failed(item.http_code),
            )?;
        let status = common_enums::AttemptStatus::from(maya_payment_status);

        // `resource_id`: Maya paymentId (`id`) from the response if available, falling
        // back to the request's `connector_transaction_id`.
        let resource_id_value = if !payment.id.is_empty() {
            payment.id
        } else {
            request_txn_id.unwrap_or_default()
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(resource_id_value.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(resource_id_value),
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

impl TryFrom<ResponseRouterData<MayaVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<MayaVoidResponse, Self>) -> Result<Self, Self::Error> {
        let status = common_enums::AttemptStatus::from(item.response.payment_status.clone());
        let connector_transaction_id = item.router_data.request.connector_transaction_id.clone();

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(connector_transaction_id),
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
