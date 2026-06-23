use crate::connectors::flywire::FlywireRouterData;
use crate::types::ResponseRouterData;
use common_enums::{AttemptStatus, RefundStatus};
use common_utils::types::{MinorUnit, Money};
use domain_types::{
    connector_flow::{Authenticate, Authorize, PSync, Refund},
    connector_types::{
        EventType, PaymentFlowData, PaymentsAuthenticateData, PaymentsAuthorizeData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData,
        RefundWebhookDetailsResponse, RefundsData, RefundsResponseData, ResponseId,
        WebhookDetailsResponse,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::PaymentMethodDataTypes,
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

// Iframe wrapper around the Flywire hosted form URL.
// `__FLYWIRE_IFRAME_SRC__` is replaced at runtime with the live session URL;
// `__FLYWIRE_RETURN_URL__` is replaced with the merchant's return_url.
// Flywire postMessage event spec: https://developers.flywire.com/docs/hosted-checkout-js-sdk
const FLYWIRE_HOSTED_TEMPLATE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>Flywire Payment</title>
  <style>
    body, html { margin: 0; height: 100%; }
    iframe { width: 100%; height: 100%; border: none; }
  </style>
</head>
<body>
  <iframe src="__FLYWIRE_IFRAME_SRC__" allow="payment"></iframe>
  <script>
    // Flywire's iframe emits many non-completion messages (focus, validation,
    // resize); a false positive here interrupts customer input mid-entry.
    function isCompletion(d) {
      if (!d || typeof d !== 'object') return false;
      if (d.success === true || d.success === false) return true;
      var t = ((d.type || d.event) || '').toString().toUpperCase();
      if (t === 'PAYMENT_COMPLETED' || t === 'PAYMENT_SUCCESS' ||
          t === 'PAYMENT_FAILED'    || t === 'PAYMENT_CANCELLED') return true;
      var s = (d.status || '').toString().toLowerCase();
      return s === 'completed' || s === 'succeeded' ||
             s === 'failed'    || s === 'cancelled';
    }
    window.addEventListener('message', function (e) {
      if (!e.origin || !e.origin.endsWith('.flywire.com')) return;
      if (!isCompletion(e.data)) return;
      window.location.href = '__FLYWIRE_RETURN_URL__';
    });
  </script>
</body>
</html>"#;

// =============================================================================
// Auth
// =============================================================================

pub struct FlywireAuthType {
    pub api_key: Secret<String>,
    pub shared_secret: Option<Secret<String>>,
    pub recipient_id: String,
}

impl TryFrom<&ConnectorSpecificConfig> for FlywireAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(value: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match value {
            ConnectorSpecificConfig::Flywire {
                api_key,
                shared_secret,
                recipient_id,
                ..
            } => Ok(Self {
                api_key: api_key.clone(),
                shared_secret: shared_secret.clone(),
                recipient_id: recipient_id.clone(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

// =============================================================================
// CreateOrder — Create Hosted Checkout Session
// =============================================================================

#[derive(Debug, Serialize)]
pub struct FlywireCheckoutSessionRequest {
    #[serde(rename = "type")]
    pub type_: &'static str,
    pub schema: &'static str,
    pub charge_intent: FlywireChargeIntent,
    pub recipient_id: String,
    pub recipient: FlywireRecipient,
    pub items: Vec<FlywireItem>,
    pub payor_id: String,
    pub external_reference: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notifications_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payor: Option<FlywirePayor>,
    pub enable_email_notifications: bool,
}

#[derive(Debug, Serialize)]
pub struct FlywireChargeIntent {
    pub mode: &'static str,
}

#[derive(Debug, Serialize)]
pub struct FlywireRecipient {
    pub fields: Vec<FlywireRecipientField>,
}

#[derive(Debug, Serialize)]
pub struct FlywireRecipientField {
    pub id: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct FlywireItem {
    pub id: &'static str,
    pub amount: MinorUnit,
}

#[derive(Debug, Serialize)]
pub struct FlywirePayor {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zip: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FlywireCheckoutSessionResponse {
    pub id: String,
    pub expires_in_seconds: Option<u64>,
    pub hosted_form: FlywireHostedForm,
    #[serde(default)]
    pub warnings: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FlywireHostedForm {
    pub url: String,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        FlywireRouterData<
            RouterDataV2<
                Authenticate,
                PaymentFlowData,
                PaymentsAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for FlywireCheckoutSessionRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: FlywireRouterData<
            RouterDataV2<
                Authenticate,
                PaymentFlowData,
                PaymentsAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let auth = FlywireAuthType::try_from(&router_data.connector_config)?;

        let ucs_payment_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let payor_id = format!("ucs_{}", ucs_payment_id);

        // Recipient fields: pulled from resource_common_data.connector_feature_data
        // if the caller provided `flywire_recipient_fields`, otherwise placeholders.
        let recipient_fields =
            build_recipient_fields(&router_data.resource_common_data.connector_feature_data)
                .unwrap_or_else(default_recipient_fields);

        // Payor (name, address, email): from billing address on resource_common_data.
        let payor = build_payor_from_billing(&router_data.resource_common_data);

        Ok(Self {
            type_: "one_off",
            schema: "cards",
            charge_intent: FlywireChargeIntent { mode: "one_off" },
            recipient_id: auth.recipient_id,
            recipient: FlywireRecipient {
                fields: recipient_fields,
            },
            items: vec![FlywireItem {
                id: "default",
                amount: router_data.request.amount,
            }],
            payor_id,
            external_reference: ucs_payment_id,
            // No notifications_url on the CheckoutSession request — the
            // FLYWIRE recipient is pre-configured with the webhook destination.
            // (PaymentsAuthenticateData has no webhook_url field to forward.)
            notifications_url: None,
            payor,
            enable_email_notifications: false,
        })
    }
}

fn build_recipient_fields(
    metadata: &Option<Secret<serde_json::Value>>,
) -> Option<Vec<FlywireRecipientField>> {
    use hyperswitch_masking::PeekInterface;
    let value = metadata.as_ref().map(|m| m.peek())?;
    let fields_value = value.get("flywire_recipient_fields")?;
    let arr = fields_value.as_array()?;
    let mut out = Vec::with_capacity(arr.len());
    for entry in arr {
        let id = entry.get("id").and_then(|v| v.as_str())?.to_string();
        let val = entry.get("value").and_then(|v| v.as_str())?.to_string();
        out.push(FlywireRecipientField { id, value: val });
    }
    Some(out)
}

fn default_recipient_fields() -> Vec<FlywireRecipientField> {
    vec![
        FlywireRecipientField {
            id: "student_id".to_string(),
            value: "UCS_TEST".to_string(),
        },
        FlywireRecipientField {
            id: "student_first_name".to_string(),
            value: "UCS".to_string(),
        },
        FlywireRecipientField {
            id: "student_last_name".to_string(),
            value: "Test".to_string(),
        },
        FlywireRecipientField {
            id: "student_email".to_string(),
            value: "ucs-test@example.com".to_string(),
        },
    ]
}

fn build_payor_from_billing(common: &PaymentFlowData) -> Option<FlywirePayor> {
    use hyperswitch_masking::ExposeInterface;
    let billing = common.get_optional_billing()?;
    let address = billing.address.as_ref()?;
    Some(FlywirePayor {
        first_name: address.first_name.as_ref().map(|n| n.clone().expose()),
        last_name: address.last_name.as_ref().map(|n| n.clone().expose()),
        address: address.line1.as_ref().map(|s| s.clone().expose()),
        city: address.city.as_ref().map(|s| s.clone().expose()),
        country: address.country.map(|c| c.to_string()),
        state: address.state.as_ref().map(|s| s.clone().expose()),
        phone: None,
        email: billing.email.as_ref().map(|e| e.clone().expose().expose()),
        zip: address.zip.as_ref().map(|s| s.clone().expose()),
    })
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<FlywireCheckoutSessionResponse, Self>>
    for RouterDataV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FlywireCheckoutSessionResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let raw_response = serde_json::to_string(&response).ok().map(Secret::new);
        let session_id = response.id.clone();

        // Wrap the hosted form URL in the iframe + postMessage template.
        // The session UUID flows as connector_transaction_id so the caller can
        // pass it back as connector_order_id in the subsequent CompositeAuthorize
        // call (which triggers the Authorize/confirm step).
        let return_url = item
            .router_data
            .resource_common_data
            .return_url
            .clone()
            .unwrap_or_default();
        let html_data = FLYWIRE_HOSTED_TEMPLATE
            .replace("__FLYWIRE_IFRAME_SRC__", &response.hosted_form.url)
            .replace("__FLYWIRE_RETURN_URL__", &return_url);
        let redirection_data = Some(Box::new(RedirectForm::Html { html_data }));

        Ok(Self {
            response: Ok(PaymentsResponseData::AuthenticateResponse {
                resource_id: Some(ResponseId::ConnectorTransactionId(session_id.clone())),
                redirection_data,
                connector_response_reference_id: None,
                authentication_data: None,
                connector_feature_data: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::Pending,
                connector_order_id: Some(session_id),
                raw_connector_response: raw_response,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// PSync — payment retrieve (also used for RSync of parent payment)
// =============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FlywirePayment {
    pub payment_id: String,
    pub status: FlywirePaymentStatus,
    #[serde(default)]
    pub status_detail: Option<String>,
    pub amount_from: Option<MinorUnit>,
    pub currency_from: Option<String>,
    pub amount_to: Option<MinorUnit>,
    pub currency_to: Option<String>,
    #[serde(default)]
    pub external_reference: Option<String>,
    #[serde(default)]
    pub payor_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FlywirePaymentStatus {
    Initiated,
    Authorized,
    Adjusted,
    Processed,
    Guaranteed,
    Delivered,
    Cancelled,
    Reversed,
    Failed,
    Expired,
    Pending,
    #[serde(other)]
    Unknown,
}

impl FlywirePaymentStatus {
    pub fn to_attempt_status(&self) -> AttemptStatus {
        match self {
            Self::Guaranteed | Self::Delivered => AttemptStatus::Charged,
            Self::Authorized | Self::Adjusted => AttemptStatus::Authorized,
            Self::Cancelled | Self::Failed | Self::Expired => AttemptStatus::Failure,
            Self::Reversed => AttemptStatus::AutoRefunded,
            Self::Initiated | Self::Processed | Self::Pending => AttemptStatus::Pending,
            Self::Unknown => {
                tracing::warn!("Flywire returned unrecognized payment status; treating as Pending");
                AttemptStatus::Pending
            }
        }
    }

    pub fn to_refund_status(&self) -> RefundStatus {
        match self {
            Self::Guaranteed | Self::Delivered | Self::Reversed => RefundStatus::Pending,
            Self::Cancelled => RefundStatus::Success,
            Self::Failed | Self::Expired => RefundStatus::Failure,
            _ => RefundStatus::Pending,
        }
    }
}

impl TryFrom<ResponseRouterData<FlywirePayment, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<FlywirePayment, Self>) -> Result<Self, Self::Error> {
        let response = item.response;
        let status = response.status.to_attempt_status();
        let raw_response = serde_json::to_string(&response).ok().map(Secret::new);

        // Override the request-side amount with what FLYWIRE actually recorded.
        // amount_to is the recipient-side billing amount — the same value we
        // sent in items[].amount at session creation. Surfacing it here
        // (instead of echoing the PSync request amount) gives the caller a
        // real integrity-check basis. We pair it with the original currency
        // from the request because FLYWIRE's currency_to is the recipient
        // billing currency (constant per recipient config) and using it
        // would conflate connector-side state with merchant-side intent.
        // If either side is absent, fall through to the request amount
        // (existing behaviour) so we don't regress non-FLYWIRE-shaped txns.
        let amount_override = match (
            response.amount_to,
            item.router_data
                .resource_common_data
                .amount
                .as_ref()
                .map(|m| m.currency),
        ) {
            (Some(amt), Some(curr)) => Some(Money { amount: amt, currency: curr }),
            _ => item.router_data.resource_common_data.amount.clone(),
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.payment_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: response.external_reference,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                network_txn_link_id: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                raw_connector_response: raw_response,
                amount: amount_override,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// Authorize — POST /checkout/sessions/{session_id}/confirm
// =============================================================================
//
// Flywire's checkout flow uses three sequential UCS gRPC calls:
//   1. PaymentService/CreateOrder  → POST /checkout/sessions
//   2. PaymentService/Authorize    → POST /checkout/sessions/{id}/confirm  ← THIS
//   3. PaymentService/Get          → GET  /payments/{payment_id}
//
// The session_id created in step 1 is stored as `connector_order_id` on
// PaymentFlowData and read here to build the URL.
//
// /confirm is one-shot — subsequent calls 404. It returns the Flywire
// payment_id (`payment_reference`) which UCS surfaces as
// `connectorTransactionId` for step 3.

/// Flywire's /confirm endpoint takes no body, but the macro framework requires
/// a serializable request type when `flow_request` has generics. An empty struct
/// serializes to `{}` which Flywire accepts.
#[derive(Debug, Serialize)]
pub struct FlywireConfirmRequest {}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        FlywireRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for FlywireConfirmRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: FlywireRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FlywireConfirmResponse {
    pub payment_reference: String,
    #[serde(default)]
    pub charge_info: Option<FlywireConfirmChargeInfo>,
    #[serde(default)]
    pub payor: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FlywireConfirmChargeInfo {
    #[serde(default)]
    pub amount: Option<MinorUnit>,
    #[serde(default)]
    pub currency: Option<String>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<FlywireConfirmResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FlywireConfirmResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let raw_response = serde_json::to_string(&response).ok().map(Secret::new);
        let merchant_ref = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        // HTTP 200 from /confirm means Flywire accepted the form and finalized
        // the payment. Mark as Charged; the subsequent PSync (GET /payments/{id})
        // will return the live status (delivered, etc.).
        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.payment_reference.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(merchant_ref),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                network_txn_link_id: None,
            }),
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::Charged,
                raw_connector_response: raw_response,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// Refund
// =============================================================================

#[derive(Debug, Serialize)]
pub struct FlywireRefundRequest {
    pub amount: MinorUnit,
    pub external_reference: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notifications_url: Option<String>,
    pub enable_payer_email_notifications: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FlywireRefundResponse {
    pub refund_id: String,
    #[serde(default)]
    pub payment_id: Option<String>,
    #[serde(default)]
    pub bundle_id: Option<String>,
    pub status: FlywireRefundStatus,
    #[serde(default)]
    pub amount: Option<MinorUnit>,
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(default)]
    pub external_reference: Option<String>,
    #[serde(default)]
    pub notifications_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FlywireRefundStatus {
    Initiated,
    Pending,
    Received,
    Approved,
    Finished,
    Completed,
    Cancelled,
    Failed,
    Returned,
    Rejected,
    #[serde(other)]
    Unknown,
}

impl FlywireRefundStatus {
    pub fn to_refund_status(&self) -> RefundStatus {
        match self {
            Self::Finished | Self::Completed | Self::Approved => RefundStatus::Success,
            Self::Rejected | Self::Cancelled | Self::Failed   => RefundStatus::Failure,
            Self::Initiated
            | Self::Pending
            | Self::Received
            | Self::Returned
            | Self::Unknown                                    => RefundStatus::Pending,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        FlywireRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for FlywireRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: FlywireRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        Ok(Self {
            amount: router_data.request.minor_refund_amount,
            external_reference: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            notifications_url: router_data.request.webhook_url.clone(),
            enable_payer_email_notifications: false,
        })
    }
}

impl<F> TryFrom<ResponseRouterData<FlywireRefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FlywireRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let raw_response = serde_json::to_string(&response).ok().map(Secret::new);
        let router_data = item.router_data;
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: response.refund_id.clone(),
                refund_status: response.status.to_refund_status(),
                status_code: item.http_code,
            }),
            resource_common_data: RefundFlowData {
                raw_connector_response: raw_response,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<FlywireRefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FlywireRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let raw_response = serde_json::to_string(&response).ok().map(Secret::new);
        let router_data = item.router_data;
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: response.refund_id.clone(),
                refund_status: response.status.to_refund_status(),
                status_code: item.http_code,
            }),
            resource_common_data: RefundFlowData {
                raw_connector_response: raw_response,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

// RSync re-queries the parent payment (GET /payments/{id}); Flywire has no per-refund GET.
// We map the parent payment status into a refund status.
impl<F> TryFrom<ResponseRouterData<FlywirePayment, Self>>
    for RouterDataV2<F, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<FlywirePayment, Self>) -> Result<Self, Self::Error> {
        let response = item.response;
        let raw_response = serde_json::to_string(&response).ok().map(Secret::new);
        let router_data = item.router_data;
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: response.payment_id.clone(),
                refund_status: response.status.to_refund_status(),
                status_code: item.http_code,
            }),
            resource_common_data: RefundFlowData {
                raw_connector_response: raw_response,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

// =============================================================================
// Errors (RFC 7807 problem+json)
// =============================================================================

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FlywireErrorResponse {
    #[serde(rename = "type")]
    pub error_type: Option<String>,
    pub title: Option<String>,
    pub status: Option<i32>,
    pub detail: Option<String>,
    #[serde(default)]
    pub errors: Vec<FlywireErrorDetail>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FlywireErrorDetail {
    #[serde(default, rename = "type")]
    pub detail_type: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub param: Option<String>,
    pub message: Option<String>,
}

// =============================================================================
// Webhook
//
// Spec: https://developers.flywire.com/education/Content/notifications-from-flywire.htm
//
// Top-level shape (both payments and refunds):
//   {
//     "event_type":     "<status name>",
//     "event_date":     "<RFC3339 timestamp>",
//     "event_resource": "payments" | "charges" | "refunds",
//     "data": { ... resource-specific fields ... }
//   }
// =============================================================================

/// Tag used by Flywire to route a webhook between payment-status and refund-status
/// payloads. `charges` is the legacy partial-capture variant of `payments` and
/// uses the same body schema; we treat them identically.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FlywireWebhookResource {
    Payments,
    Charges,
    Refunds,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FlywireWebhookBody {
    pub event_type: String,
    #[serde(default)]
    pub event_date: Option<String>,
    pub event_resource: FlywireWebhookResource,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FlywirePaymentWebhookData {
    pub payment_id: String,
    pub status: FlywirePaymentStatus,
    #[serde(default)]
    pub external_reference: Option<String>,
    #[serde(default)]
    pub amount_from: Option<String>,
    #[serde(default)]
    pub currency_from: Option<String>,
    #[serde(default)]
    pub amount_to: Option<String>,
    #[serde(default)]
    pub currency_to: Option<String>,
}

/// Refund-resource webhook payload. Flywire uses a separate set of statuses
/// for refund events (the documented values are `initiated`, `received`,
/// `finished`, `cancelled`, `returned`, `failed`). We model just success vs.
/// failure here; everything else maps to Pending.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FlywireRefundWebhookStatus {
    Initiated,
    Received,
    Finished,
    Cancelled,
    Returned,
    Failed,
    #[serde(other)]
    Unknown,
}

impl FlywireRefundWebhookStatus {
    pub fn to_refund_status(&self) -> RefundStatus {
        match self {
            Self::Finished => RefundStatus::Success,
            Self::Cancelled | Self::Returned | Self::Failed => RefundStatus::Failure,
            Self::Initiated | Self::Received | Self::Unknown => RefundStatus::Pending,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FlywireRefundWebhookData {
    pub refund_id: String,
    #[serde(default)]
    pub payment_id: Option<String>,
    pub status: FlywireRefundWebhookStatus,
    #[serde(default)]
    pub external_reference: Option<String>,
    #[serde(default)]
    pub amount: Option<String>,
    #[serde(default)]
    pub currency: Option<String>,
}

impl FlywireWebhookBody {
    pub fn parse_payment_data(
        &self,
    ) -> Result<FlywirePaymentWebhookData, error_stack::Report<IntegrationError>> {
        serde_json::from_value(self.data.clone())
            .map_err(|_| IntegrationError::InvalidDataFormat {
                field_name: "flywire webhook data",
                context: Default::default(),
            })
            .map_err(error_stack::Report::from)
    }

    pub fn parse_refund_data(
        &self,
    ) -> Result<FlywireRefundWebhookData, error_stack::Report<IntegrationError>> {
        serde_json::from_value(self.data.clone())
            .map_err(|_| IntegrationError::InvalidDataFormat {
                field_name: "flywire webhook data",
                context: Default::default(),
            })
            .map_err(error_stack::Report::from)
    }
}

impl TryFrom<&FlywireWebhookBody> for WebhookDetailsResponse {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(body: &FlywireWebhookBody) -> Result<Self, Self::Error> {
        let data = body.parse_payment_data()?;
        Ok(Self {
            resource_id: Some(ResponseId::ConnectorTransactionId(data.payment_id)),
            status: data.status.to_attempt_status(),
            error_code: None,
            error_message: None,
            error_reason: None,
            status_code: 200,
            connector_response_reference_id: data.external_reference,
            mandate_reference: None,
            raw_connector_response: None,
            response_headers: None,
            minor_amount_captured: None,
            amount_captured: None,
            network_txn_id: None,
            payment_method_update: None,
            sender_payment_instrument_id: None,
        })
    }
}

impl TryFrom<&FlywireWebhookBody> for RefundWebhookDetailsResponse {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(body: &FlywireWebhookBody) -> Result<Self, Self::Error> {
        let data = body.parse_refund_data()?;
        Ok(Self {
            connector_refund_id: Some(data.refund_id),
            status: data.status.to_refund_status(),
            connector_response_reference_id: data.external_reference,
            error_code: None,
            error_message: None,
            raw_connector_response: None,
            status_code: 200,
            response_headers: None,
        })
    }
}

pub fn webhook_event_type(body: &FlywireWebhookBody) -> EventType {
    match body.event_resource {
        FlywireWebhookResource::Refunds => {
            let status = body
                .parse_refund_data()
                .ok()
                .map(|d| d.status)
                .unwrap_or(FlywireRefundWebhookStatus::Unknown);
            match status {
                FlywireRefundWebhookStatus::Finished => EventType::RefundSuccess,
                FlywireRefundWebhookStatus::Cancelled
                | FlywireRefundWebhookStatus::Returned
                | FlywireRefundWebhookStatus::Failed => EventType::RefundFailure,
                FlywireRefundWebhookStatus::Initiated
                | FlywireRefundWebhookStatus::Received
                | FlywireRefundWebhookStatus::Unknown => EventType::PaymentIntentProcessing,
            }
        }
        // `charges` events are per-card-capture-attempt notifications. A single
        // payment can emit multiple charges events (e.g., first attempt declined,
        // second attempt succeeds) before the payment-level state is decided.
        // We MUST NOT promote a charges:failed event to PaymentIntentFailure -
        // that would terminate the txn in Euler before the payer's retry chance
        // is exhausted. The canonical terminal signal is the `payments:*` event
        // family; charges events are informational and always map to Processing.
        FlywireWebhookResource::Charges => EventType::PaymentIntentProcessing,
        FlywireWebhookResource::Payments | FlywireWebhookResource::Unknown => {
            let status = body
                .parse_payment_data()
                .ok()
                .map(|d| d.status)
                .unwrap_or(FlywirePaymentStatus::Unknown);
            match status {
                FlywirePaymentStatus::Guaranteed | FlywirePaymentStatus::Delivered => {
                    EventType::PaymentIntentSuccess
                }
                FlywirePaymentStatus::Cancelled
                | FlywirePaymentStatus::Failed
                | FlywirePaymentStatus::Expired => EventType::PaymentIntentFailure,
                FlywirePaymentStatus::Authorized => EventType::PaymentIntentAuthorizationSuccess,
                FlywirePaymentStatus::Reversed => EventType::RefundSuccess,
                FlywirePaymentStatus::Initiated
                | FlywirePaymentStatus::Adjusted
                | FlywirePaymentStatus::Processed
                | FlywirePaymentStatus::Pending
                | FlywirePaymentStatus::Unknown => EventType::PaymentIntentProcessing,
            }
        }
    }
}
