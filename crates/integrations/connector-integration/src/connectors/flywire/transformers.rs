use crate::connectors::flywire::FlywireRouterData;
use crate::types::ResponseRouterData;
use common_enums::{AttemptStatus, RefundStatus};
use common_utils::{pii::Email, types::MinorUnit};
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
    router_request_types::AuthoriseIntegrityObject,
    router_response_types::RedirectForm,
};
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

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
                context: domain_types::errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Configure the merchant connector account with the Flywire variant \
                         (api_key, shared_secret, recipient_id)."
                            .to_string(),
                    ),
                    doc_url: Some("https://developers.flywire.com/docs/api-basics".to_string()),
                    additional_context: Some(
                        "ConnectorSpecificConfig did not match the expected `Flywire` variant."
                            .to_string(),
                    ),
                },
            }
            .into()),
        }
    }
}

// =============================================================================
// CreateOrder — Create Hosted Checkout Session
// =============================================================================

const CHECKOUT_SESSION_TYPE: &str = "one_off";
const CHECKOUT_SESSION_SCHEMA: &str = "cards";
const CHECKOUT_SESSION_ITEM_ID: &str = "default";
const FLYWIRE_EVENT_LISTENER_TEMPLATE: &str = r#"window.addEventListener("message", function (e) {
  if (!e.origin || !e.origin.endsWith(".flywire.com")) return;
  var d = e.data;
  if (!d || typeof d !== 'object') return;
  if (d.source !== 'checkout_session') return;
  if (d.success === true && d.confirm_url) {
    window.location.href = __RETURN_URL__;
  }
});"#;

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
    pub first_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<Secret<String>>,
    // ISO country code — not PII, kept as a plain string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<Email>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zip: Option<Secret<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FlywireCheckoutSessionResponse {
    pub id: String,
    pub expires_in_seconds: Option<u64>,
    pub hosted_form: FlywireHostedForm,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warnings: Option<Vec<serde_json::Value>>,
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

        let payment_ref = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let payor_id = payment_ref.clone();

        // Recipient fields: mapped from typed `domain_data.education_data.student_details`.
        // Institution-specific and required — cannot be defaulted.
        let student_details = router_data
            .request
            .domain_data
            .as_ref()
            .and_then(|d| d.education_data.as_ref())
            .and_then(|e| e.student_details.as_ref())
            .ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                    field_name: "domain_data.education_data.student_details",
                    context: domain_types::errors::IntegrationErrorContext {
                        suggested_action: Some(
                            "Pass student details in `domain_data.education_data.student_details` \
                             (student_id, student_first_name, student_last_name, student_email)."
                                .to_string(),
                        ),
                        doc_url: Some(
                            "https://developers.flywire.com/docs/checkout-session".to_string(),
                        ),
                        additional_context: None,
                    },
                })
            })?;
        let recipient_fields = student_details_to_recipient_fields(student_details);

        // Payor (name, address, email): from billing address on resource_common_data.
        let payor = build_payor_from_billing(&router_data.resource_common_data);

        Ok(Self {
            type_: CHECKOUT_SESSION_TYPE,
            schema: CHECKOUT_SESSION_SCHEMA,
            charge_intent: FlywireChargeIntent {
                mode: CHECKOUT_SESSION_TYPE,
            },
            recipient_id: auth.recipient_id,
            recipient: FlywireRecipient {
                fields: recipient_fields,
            },
            items: vec![FlywireItem {
                id: CHECKOUT_SESSION_ITEM_ID,
                amount: router_data.request.amount,
            }],
            payor_id,
            external_reference: payment_ref,
            notifications_url: router_data.request.webhook_url.clone(),
            payor,
            enable_email_notifications: false,
        })
    }
}

// Flywire recipient field ids for the student_details (student) schema.
const FIELD_STUDENT_ID: &str = "student_id";
const FIELD_STUDENT_FIRST_NAME: &str = "student_first_name";
const FIELD_STUDENT_LAST_NAME: &str = "student_last_name";
const FIELD_STUDENT_EMAIL: &str = "student_email";

/// Maps typed student_details data to Flywire's `recipient.fields` id/value pairs.
/// Only fields the merchant actually supplied are emitted; absent fields are
/// skipped so we never send empty recipient values.
fn student_details_to_recipient_fields(
    student_details: &domain_types::connector_types::StudentDetails,
) -> Vec<FlywireRecipientField> {
    [
        (FIELD_STUDENT_ID, student_details.student_id.as_ref()),
        (
            FIELD_STUDENT_FIRST_NAME,
            student_details.student_first_name.as_ref(),
        ),
        (
            FIELD_STUDENT_LAST_NAME,
            student_details.student_last_name.as_ref(),
        ),
        (FIELD_STUDENT_EMAIL, student_details.student_email.as_ref()),
    ]
    .into_iter()
    .filter_map(|(id, value)| {
        value.map(|value| FlywireRecipientField {
            id: id.to_string(),
            value: value.clone(),
        })
    })
    .collect()
}

fn build_payor_from_billing(common: &PaymentFlowData) -> Option<FlywirePayor> {
    let billing = common.get_optional_billing()?;
    let address = billing.address.as_ref()?;
    // Keep PII inside `Secret`/`Email` — never `expose()` it into the request struct.
    Some(FlywirePayor {
        first_name: address.first_name.clone(),
        last_name: address.last_name.clone(),
        address: address.line1.clone(),
        city: address.city.clone(),
        country: address.country.map(|c| c.to_string()),
        state: address.state.clone(),
        phone: billing.phone.as_ref().and_then(|p| p.number.clone()),
        email: billing.email.clone(),
        zip: address.zip.clone(),
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

        // `return_url` is where the payer is navigated once the checkout form is
        // submitted — without it there is no way back to the merchant, so a
        // missing value is a hard error rather than an empty redirect target.
        let return_url = item
            .router_data
            .resource_common_data
            .return_url
            .clone()
            .ok_or_else(|| {
                error_stack::report!(ConnectorError::ResponseHandlingFailed {
                    context: domain_types::errors::ResponseTransformationErrorContext {
                        http_status_code: Some(item.http_code),
                        additional_context: Some(
                            "Flywire hosted-checkout requires return_url to redirect the payer \
                             back after payment completion; none was provided on the request."
                                .to_string(),
                        ),
                    },
                })
            })?;
        let endpoint = response.hosted_form.url.clone();
        // `return_url` is caller-supplied and gets spliced into a script the client
        // inlines verbatim in a <script> tag. Require an http(s) scheme (blocks
        // `javascript:`/`data:`) and emit it as a JSON-encoded, HTML-safe JS string
        // literal so it cannot break out of the string or the <script> element.
        if !return_url.starts_with("https://") && !return_url.starts_with("http://") {
            return Err(error_stack::report!(
                ConnectorError::ResponseHandlingFailed {
                    context: domain_types::errors::ResponseTransformationErrorContext {
                        http_status_code: Some(item.http_code),
                        additional_context: Some(
                            "Flywire return_url must be an http(s) URL".to_string(),
                        ),
                    },
                }
            ));
        }
        let return_url_literal = serde_json::to_string(&return_url)
            .map_err(|_| {
                error_stack::report!(ConnectorError::ResponseHandlingFailed {
                    context: domain_types::errors::ResponseTransformationErrorContext {
                        http_status_code: Some(item.http_code),
                        additional_context: Some("Failed to encode return_url".to_string()),
                    },
                })
            })?
            .replace('<', "\\u003c")
            .replace('>', "\\u003e")
            .replace('&', "\\u0026");
        // postMessage listener the client attaches; it navigates to `return_url`
        // once Flywire signals a successful checkout-session submission.
        let events = FLYWIRE_EVENT_LISTENER_TEMPLATE.replace("__RETURN_URL__", &return_url_literal);
        let redirection_data = Some(Box::new(RedirectForm::HostedIframe {
            endpoint,
            method: common_utils::Method::Get,
            events,
        }));

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
    pub status_detail: Option<String>,
    pub amount_from: Option<MinorUnit>,
    pub currency_from: Option<String>,
    pub amount_to: Option<MinorUnit>,
    pub currency_to: Option<String>,
    pub external_reference: Option<String>,
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
                splits: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                raw_connector_response: raw_response,
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
    pub charge_info: Option<FlywireConfirmChargeInfo>,
    pub payor: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FlywireConfirmChargeInfo {
    pub amount: Option<MinorUnit>,
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

        // Flywire's /confirm response body carries only payor-side amount
        // (charge_info.amount = amount_from after FX) and does not echo
        // external_reference. Neither is a usable integrity counterpart. So
        // we echo the request amount + currency into the AuthoriseIntegrityObject
        // to enable round-trip integrity (catches prism/UCS-side tampering,
        // NOT Flywire-side settlement drift — that is caught by PSync).
        let response_integrity_object = Some(AuthoriseIntegrityObject {
            amount: item.router_data.request.amount,
            currency: item.router_data.request.currency,
        });

        // HTTP 200 from /confirm means Flywire accepted the form. The payment is
        // at FLYWIRE status "initiated"/"processed" at this moment; funds are
        // NOT yet guaranteed (typically 20-30s later via webhook) or delivered
        // (daily batch). The Charged mapping here is intentionally optimistic;
        // subsequent PSync or webhook refines to the actual FLYWIRE state.
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
                splits: None,
            }),
            resource_common_data: PaymentFlowData {
                // HTTP 200 from /confirm only means Flywire accepted the form.
                // Actual state is initiated/processed; guaranteed/delivered
                // come async via webhook/PSync. Marking Charged here would
                // report success on a payment that could still fail.
                status: AttemptStatus::Pending,
                raw_connector_response: raw_response,
                ..item.router_data.resource_common_data
            },
            request: PaymentsAuthorizeData {
                integrity_object: response_integrity_object,
                ..item.router_data.request
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
    pub payment_id: Option<String>,
    pub bundle_id: Option<String>,
    pub status: FlywireRefundStatus,
    pub amount: Option<MinorUnit>,
    pub currency: Option<String>,
    pub external_reference: Option<String>,
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
            Self::Rejected | Self::Cancelled | Self::Failed | Self::Returned => {
                RefundStatus::Failure
            }
            Self::Initiated | Self::Pending | Self::Received | Self::Unknown => {
                RefundStatus::Pending
            }
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<FlywireErrorDetail>>,
}

/// Inner `errors[].type` reason codes on Flywire refund errors. Every listed
/// value is a terminal failure (the refund can never succeed); unknown codes
/// deserialize to `Other`.
/// https://developers.flywire.com/education/Content/resource_refunds.htm
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FlywireRefundErrorType {
    InvalidStatus,
    NotFound,
    AlreadyRefund,
    Status,
    AmountOverRemainingBalance,
    MinimumThreshold,
    Currency,
    Multiple,
    #[serde(other)]
    Other,
}

impl FlywireRefundErrorType {
    /// A known reason code means the refund is permanently failed, so the refund
    /// row should be marked FAILED rather than left transient/PENDING.
    pub fn is_terminal_refund_failure(&self) -> bool {
        !matches!(self, Self::Other)
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FlywireErrorDetail {
    #[serde(rename = "type")]
    pub detail_type: Option<FlywireRefundErrorType>,
    pub source: Option<String>,
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
    pub event_date: Option<String>,
    pub event_resource: FlywireWebhookResource,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FlywirePaymentWebhookData {
    pub payment_id: String,
    pub status: FlywirePaymentStatus,
    pub external_reference: Option<String>,
    pub amount_from: Option<String>,
    pub currency_from: Option<String>,
    pub amount_to: Option<String>,
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
    pub payment_id: Option<String>,
    pub status: FlywireRefundWebhookStatus,
    pub external_reference: Option<String>,
    pub amount: Option<String>,
    pub currency: Option<String>,
}

impl FlywireWebhookBody {
    pub fn parse_payment_data(
        &self,
    ) -> Result<FlywirePaymentWebhookData, error_stack::Report<IntegrationError>> {
        serde_json::from_value(self.data.clone())
            .map_err(|err| IntegrationError::InvalidDataFormat {
                field_name: "flywire webhook data",
                context: domain_types::errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Verify the webhook `data` object matches Flywire's payment-status \
                         notification schema (payment_id, status, ...)."
                            .to_string(),
                    ),
                    doc_url: Some(
                        "https://developers.flywire.com/education/Content/notifications-payment-status.htm"
                            .to_string(),
                    ),
                    additional_context: Some(format!(
                        "Failed to deserialize payments webhook `data`: {err}"
                    )),
                },
            })
            .map_err(error_stack::Report::from)
    }

    pub fn parse_refund_data(
        &self,
    ) -> Result<FlywireRefundWebhookData, error_stack::Report<IntegrationError>> {
        serde_json::from_value(self.data.clone())
            .map_err(|err| IntegrationError::InvalidDataFormat {
                field_name: "flywire webhook data",
                context: domain_types::errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Verify the webhook `data` object matches Flywire's refund-status \
                         notification schema (refund_id, status, ...)."
                            .to_string(),
                    ),
                    doc_url: Some(
                        "https://developers.flywire.com/education/Content/resource_refunds.htm"
                            .to_string(),
                    ),
                    additional_context: Some(format!(
                        "Failed to deserialize refunds webhook `data`: {err}"
                    )),
                },
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
            merchant_transaction_id: None,
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
