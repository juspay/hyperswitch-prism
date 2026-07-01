pub mod transformers;
use std::{
    fmt::Debug,
    marker::{Send, Sync},
};

use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    crypto::VerifySignature,
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    types::StringMinorUnitForConnector,
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, CreateConnectorCustomer,
        IncrementalAuthorization, PSync, PaymentMethodToken, RSync, Refund, RepeatPayment,
        SetupMandate, Void,
    },
    connector_types::{
        ClientAuthenticationTokenRequestData, ConnectorCustomerData, ConnectorCustomerResponse,
        ConnectorWebhookSecrets, DisputeWebhookDetailsResponse, DisputeWebhookReference,
        EventContext, EventType, PaymentFlowData, PaymentMethodTokenResponse,
        PaymentMethodTokenizationData, PaymentVoidData, PaymentWebhookReference,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsIncrementalAuthorizationData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData,
        RefundWebhookDetailsResponse, RefundWebhookReference, RefundsData, RefundsResponseData,
        RepeatPaymentData, RequestDetails, ResponseId, SetupMandateRequestData,
        WebhookDetailsResponse, WebhookResourceReference,
    },
    errors::{ConnectorError, IntegrationError, WebhookError},
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};

use error_stack::{report, Report, ResultExt};
use hyperswitch_masking::{ExposeInterface, Mask, Maskable, PeekInterface, Secret};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    self as stripe, CancelRequest, CaptureRequest, CreateConnectorCustomerRequest,
    CreateConnectorCustomerResponse, PaymentIncrementalAuthRequest, PaymentIntentRequest,
    PaymentIntentRequest as RepeatPaymentRequest,
    PaymentIntentResponse as PaymentIncrementalAuthResponse, PaymentSyncResponse,
    PaymentsAuthorizeResponse, PaymentsAuthorizeResponse as RepeatPaymentResponse,
    PaymentsCaptureResponse, PaymentsVoidResponse, RefundResponse,
    RefundResponse as RefundSyncResponse, SetupMandateRequest, SetupMandateResponse,
    StripeClientAuthRequest, StripeClientAuthResponse, StripeRefundRequest, StripeTokenResponse,
    TokenRequest,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const STRIPE_COMPATIBLE_CONNECT_ACCOUNT: &str = "Stripe-Account";
}
use stripe::auth_headers;

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Stripe<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Stripe,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Stripe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Stripe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for Stripe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Stripe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Stripe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Stripe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Stripe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Stripe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Stripe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentIncrementalAuthorization for Stripe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Stripe<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Stripe<T>
{
}

/// Parse the `Stripe-Signature` header into its key/value elements.
///
/// Stripe sends a header of the form `t=1700000000,v1=<hex_hmac>[,v0=...]`. We split on `,`
/// and then on the first `=` of each element, mirroring HS `get_signature_elements_from_header`.
/// The header lookup is case-insensitive because the gateway may normalise header casing.
fn get_signature_elements_from_header(
    headers: &std::collections::HashMap<String, String>,
) -> Result<std::collections::HashMap<String, Vec<u8>>, Report<WebhookError>> {
    let security_header = headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("Stripe-Signature"))
        .map(|(_, value)| value.clone())
        .ok_or_else(|| report!(WebhookError::WebhookSignatureNotFound))?;

    let props = security_header.split(',').collect::<Vec<&str>>();
    let mut security_header_kvs: std::collections::HashMap<String, Vec<u8>> =
        std::collections::HashMap::with_capacity(props.len());

    for prop_str in &props {
        let (prop_key, prop_value) = prop_str
            .split_once('=')
            .ok_or_else(|| report!(WebhookError::WebhookSourceVerificationFailed))?;

        security_header_kvs.insert(prop_key.to_string(), prop_value.bytes().collect());
    }

    Ok(security_header_kvs)
}

/// Map a Stripe webhook object `status` to a prism dispute `EventType`.
///
/// Ports HS `From<WebhookEventStatus> for IncomingWebhookEvent` (dispute arms). Non-dispute
/// statuses map to `IncomingWebhookEventUnspecified` (HS `EventNotSupported`).
fn dispute_status_to_event_type(status: &stripe::WebhookEventStatus) -> EventType {
    use stripe::WebhookEventStatus as S;
    match status {
        S::WarningNeedsResponse | S::NeedsResponse => EventType::DisputeOpened,
        S::WarningClosed => EventType::DisputeCancelled,
        S::WarningUnderReview | S::UnderReview => EventType::DisputeChallenged,
        S::Won => EventType::DisputeWon,
        S::Lost => EventType::DisputeLost,
        S::ChargeRefunded
        | S::Succeeded
        | S::RequiresPaymentMethod
        | S::RequiresConfirmation
        | S::RequiresAction
        | S::Processing
        | S::RequiresCapture
        | S::Canceled
        | S::Chargeable
        | S::Failed
        | S::Unknown => EventType::IncomingWebhookEventUnspecified,
    }
}

/// Map a Stripe webhook object `status` to a prism `DisputeStatus`.
///
/// Mirrors the dispute arms of HS `From<WebhookEventStatus> for IncomingWebhookEvent`; statuses
/// that do not denote a dispute state default to `DisputeOpened`.
fn dispute_status_to_dispute_status(
    status: &stripe::WebhookEventStatus,
) -> common_enums::DisputeStatus {
    use stripe::WebhookEventStatus as S;
    match status {
        S::WarningNeedsResponse | S::NeedsResponse => common_enums::DisputeStatus::DisputeOpened,
        S::WarningClosed => common_enums::DisputeStatus::DisputeCancelled,
        S::WarningUnderReview | S::UnderReview => common_enums::DisputeStatus::DisputeChallenged,
        S::Won => common_enums::DisputeStatus::DisputeWon,
        S::Lost => common_enums::DisputeStatus::DisputeLost,
        _ => common_enums::DisputeStatus::DisputeOpened,
    }
}

/// Map a Stripe payment-intent `status` to a prism `AttemptStatus`.
///
/// Ports HS `From<StripePaymentStatus> for AttemptStatus`. The webhook object `status` field is
/// the live payment-intent status, so this is the authoritative attempt-status source.
fn payment_status_to_attempt_status(
    status: &stripe::WebhookEventStatus,
) -> common_enums::AttemptStatus {
    use stripe::WebhookEventStatus as S;
    match status {
        S::Succeeded => common_enums::AttemptStatus::Charged,
        S::Failed => common_enums::AttemptStatus::Failure,
        // Stripe sets `requires_payment_method` after a declined attempt -> treat as failure.
        S::RequiresPaymentMethod => common_enums::AttemptStatus::Failure,
        S::RequiresConfirmation => common_enums::AttemptStatus::ConfirmationAwaited,
        S::RequiresAction => common_enums::AttemptStatus::AuthenticationPending,
        S::Processing => common_enums::AttemptStatus::Authorizing,
        S::RequiresCapture => common_enums::AttemptStatus::Authorized,
        S::Chargeable => common_enums::AttemptStatus::Authorizing,
        S::Canceled => common_enums::AttemptStatus::Voided,
        S::ChargeRefunded
        | S::WarningNeedsResponse
        | S::WarningClosed
        | S::WarningUnderReview
        | S::Won
        | S::Lost
        | S::NeedsResponse
        | S::UnderReview
        | S::Unknown => common_enums::AttemptStatus::Pending,
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Stripe<T>
{
    fn get_webhook_source_verification_signature(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, Report<WebhookError>> {
        let mut security_header_kvs = get_signature_elements_from_header(&request.headers)?;

        let signature = security_header_kvs
            .remove("v1")
            .ok_or_else(|| report!(WebhookError::WebhookSignatureNotFound))?;

        hex::decode(signature).change_context(WebhookError::WebhookSignatureNotFound)
    }

    fn get_webhook_source_verification_message(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, Report<WebhookError>> {
        let mut security_header_kvs = get_signature_elements_from_header(&request.headers)?;

        let timestamp = security_header_kvs
            .remove("t")
            .ok_or_else(|| report!(WebhookError::WebhookSignatureNotFound))?;

        // Byte-exact reproduction of HS: "{timestamp}.{raw_body}". The raw request body is used
        // verbatim (never re-serialized) so the HMAC matches Stripe's.
        Ok(format!(
            "{}.{}",
            String::from_utf8_lossy(&timestamp),
            String::from_utf8_lossy(&request.body)
        )
        .into_bytes())
    }

    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, Report<WebhookError>> {
        let connector_webhook_secrets = match connector_webhook_secret {
            Some(secrets) => secrets,
            None => return Ok(false),
        };

        let algorithm = common_utils::crypto::HmacSha256;

        let signature =
            self.get_webhook_source_verification_signature(&request, &connector_webhook_secrets)?;
        let message =
            self.get_webhook_source_verification_message(&request, &connector_webhook_secrets)?;

        algorithm
            .verify_signature(&connector_webhook_secrets.secret, &signature, &message)
            .change_context(WebhookError::WebhookSourceVerificationFailed)
    }

    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"{"id":"evt_probe_001","type":"payment_intent.succeeded","data":{"object":{"id":"pi_probe_001","object":"payment_intent","amount":1000,"currency":"usd","created":1700000000,"status":"succeeded"}}}"#
    }

    fn get_event_type(&self, request: RequestDetails) -> Result<EventType, Report<WebhookError>> {
        let details: stripe::WebhookEventTypeBody = request
            .body
            .parse_struct("WebhookEventTypeBody")
            .change_context(WebhookError::WebhookEventTypeNotFound)?;

        let stripe::WebhookStatusObjectData {
            status,
            payment_method_details,
        } = details.event_data.event_object;

        Ok(match details.event_type {
            stripe::WebhookEventType::PaymentIntentFailed => EventType::PaymentIntentFailure,
            stripe::WebhookEventType::PaymentIntentSucceed => EventType::PaymentIntentSuccess,
            stripe::WebhookEventType::PaymentIntentCanceled => EventType::PaymentIntentCancelled,
            stripe::WebhookEventType::PaymentIntentAmountCapturableUpdated => {
                EventType::PaymentIntentAuthorizationSuccess
            }
            stripe::WebhookEventType::ChargeSucceeded => match payment_method_details {
                Some(stripe::WebhookPaymentMethodDetails {
                    payment_method:
                        stripe::WebhookPaymentMethodType::AchCreditTransfer
                        | stripe::WebhookPaymentMethodType::MultibancoBankTransfers,
                }) => EventType::PaymentIntentSuccess,
                _ => EventType::IncomingWebhookEventUnspecified,
            },
            stripe::WebhookEventType::ChargeRefundUpdated => match status.as_ref() {
                Some(stripe::WebhookEventStatus::Succeeded) => EventType::RefundSuccess,
                Some(stripe::WebhookEventStatus::Failed) => EventType::RefundFailure,
                _ => EventType::IncomingWebhookEventUnspecified,
            },
            stripe::WebhookEventType::SourceChargeable => EventType::SourceChargeable,
            // Dispute events: prefer object.status, fall back to event type.
            stripe::WebhookEventType::DisputeCreated => status
                .as_ref()
                .map(dispute_status_to_event_type)
                .unwrap_or(EventType::DisputeOpened),
            stripe::WebhookEventType::DisputeUpdated => status
                .as_ref()
                .map(dispute_status_to_event_type)
                .unwrap_or(EventType::IncomingWebhookEventUnspecified),
            stripe::WebhookEventType::DisputeClosed => status
                .as_ref()
                .map(dispute_status_to_event_type)
                .unwrap_or(EventType::DisputeCancelled),
            stripe::WebhookEventType::ChargeDisputeFundsWithdrawn => status
                .as_ref()
                .map(dispute_status_to_event_type)
                .unwrap_or(EventType::DisputeLost),
            stripe::WebhookEventType::ChargeDisputeFundsReinstated => status
                .as_ref()
                .map(dispute_status_to_event_type)
                .unwrap_or(EventType::DisputeWon),
            stripe::WebhookEventType::PaymentIntentPartiallyFunded => {
                EventType::PaymentIntentPartiallyFunded
            }
            stripe::WebhookEventType::PaymentIntentRequiresAction => {
                EventType::PaymentActionRequired
            }
            stripe::WebhookEventType::Unknown
            | stripe::WebhookEventType::ChargeCaptured
            | stripe::WebhookEventType::ChargeExpired
            | stripe::WebhookEventType::ChargeFailed
            | stripe::WebhookEventType::ChargePending
            | stripe::WebhookEventType::ChargeUpdated
            | stripe::WebhookEventType::ChargeRefunded
            | stripe::WebhookEventType::PaymentIntentCreated
            | stripe::WebhookEventType::PaymentIntentProcessing
            | stripe::WebhookEventType::SourceTransactionCreated => {
                EventType::IncomingWebhookEventUnspecified
            }
        })
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, Report<WebhookError>> {
        let details: stripe::WebhookEvent = request
            .body
            .parse_struct("WebhookEvent")
            .change_context(WebhookError::WebhookReferenceIdNotFound)?;

        let event_object = details.event_data.event_object;
        let order_id = event_object
            .metadata
            .as_ref()
            .and_then(|meta_data| meta_data.order_id.clone());

        let reference = match event_object.object {
            stripe::WebhookEventObjectType::PaymentIntent => {
                // Mirror HS get_webhook_object_reference_id exactly: when metadata.order_id is
                // present the reference is the merchant order id (PaymentAttemptId), otherwise the
                // PaymentIntent object id (ConnectorTransactionId). Either/or — never both, because
                // the shadow snapshot normaliser prefers connector_transaction_id whenever it is set.
                match order_id {
                    Some(order_id) => WebhookResourceReference::Payment(PaymentWebhookReference {
                        connector_transaction_id: None,
                        merchant_transaction_id: Some(order_id),
                    }),
                    None => WebhookResourceReference::Payment(PaymentWebhookReference {
                        connector_transaction_id: Some(event_object.id.clone()),
                        merchant_transaction_id: None,
                    }),
                }
            }
            stripe::WebhookEventObjectType::Charge => {
                // HS: order_id -> PaymentAttemptId, else the linked payment_intent as the
                // ConnectorTransactionId. Either/or, as for PaymentIntent.
                match order_id {
                    Some(order_id) => WebhookResourceReference::Payment(PaymentWebhookReference {
                        connector_transaction_id: None,
                        merchant_transaction_id: Some(order_id),
                    }),
                    None => WebhookResourceReference::Payment(PaymentWebhookReference {
                        connector_transaction_id: event_object.payment_intent.clone(),
                        merchant_transaction_id: None,
                    }),
                }
            }
            stripe::WebhookEventObjectType::Dispute => {
                // HS maps a dispute to its PARENT payment:
                // PaymentId(ConnectorTransactionId(payment_intent)). The shadow normaliser prefers
                // connector_dispute_id, so leave it None and surface the parent payment_intent as
                // connector_transaction_id to match HS byte-for-byte.
                WebhookResourceReference::Dispute(DisputeWebhookReference {
                    connector_dispute_id: None,
                    connector_transaction_id: event_object.payment_intent.clone(),
                })
            }
            stripe::WebhookEventObjectType::Source => {
                // HS uses a PreprocessingId here; prism has no source/preprocessing reference,
                // so surface the source id as the payment connector transaction id.
                WebhookResourceReference::Payment(PaymentWebhookReference {
                    connector_transaction_id: Some(event_object.id.clone()),
                    merchant_transaction_id: None,
                })
            }
            stripe::WebhookEventObjectType::Refund => {
                let is_refund_id_as_reference = event_object
                    .metadata
                    .as_ref()
                    .and_then(|meta_data| meta_data.is_refund_id_as_reference.clone());
                // Ports HS issue-2076 logic: a refund-id reference becomes the merchant refund
                // id, otherwise the object id is the connector refund id.
                let (merchant_refund_id, connector_refund_id) =
                    match (order_id, is_refund_id_as_reference) {
                        (Some(order_id), Some(_)) => (Some(order_id), None),
                        _ => (None, Some(event_object.id.clone())),
                    };
                WebhookResourceReference::Refund(RefundWebhookReference {
                    connector_refund_id,
                    merchant_refund_id,
                    connector_transaction_id: event_object.payment_intent.clone(),
                })
            }
        };

        Ok(Some(reference))
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, Report<WebhookError>> {
        let details: stripe::WebhookEvent = request
            .body
            .parse_struct("WebhookEvent")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        let event_object = details.event_data.event_object;

        let status = match details.event_type {
            stripe::WebhookEventType::PaymentIntentPartiallyFunded => {
                common_enums::AttemptStatus::PartialCharged
            }
            stripe::WebhookEventType::PaymentIntentSucceed
            | stripe::WebhookEventType::ChargeSucceeded => common_enums::AttemptStatus::Charged,
            stripe::WebhookEventType::PaymentIntentFailed => common_enums::AttemptStatus::Failure,
            stripe::WebhookEventType::PaymentIntentCanceled => common_enums::AttemptStatus::Voided,
            stripe::WebhookEventType::PaymentIntentAmountCapturableUpdated => {
                common_enums::AttemptStatus::Authorized
            }
            stripe::WebhookEventType::PaymentIntentRequiresAction => {
                common_enums::AttemptStatus::AuthenticationPending
            }
            stripe::WebhookEventType::PaymentIntentProcessing => {
                common_enums::AttemptStatus::Pending
            }
            // Fall back to the live payment-intent status carried on the object.
            _ => event_object
                .status
                .as_ref()
                .map(payment_status_to_attempt_status)
                .unwrap_or(common_enums::AttemptStatus::Pending),
        };

        let (error_code, error_message, error_reason) =
            if status == common_enums::AttemptStatus::Failure {
                let error = event_object.last_payment_error.as_ref();
                (
                    error.and_then(|error| error.code.clone()),
                    error.and_then(|error| error.message.clone()),
                    error.and_then(|error| error.message.clone()),
                )
            } else {
                (None, None, None)
            };

        // For a Charge object the connector transaction id is the linked payment_intent.
        let connector_transaction_id = match event_object.object {
            stripe::WebhookEventObjectType::Charge => event_object.payment_intent.clone(),
            _ => Some(event_object.id.clone()),
        };

        let connector_response_reference_id = event_object
            .metadata
            .as_ref()
            .and_then(|meta_data| meta_data.order_id.clone())
            .or_else(|| connector_transaction_id.clone());

        Ok(WebhookDetailsResponse {
            resource_id: connector_transaction_id.map(ResponseId::ConnectorTransactionId),
            status,
            connector_response_reference_id,
            mandate_reference: None,
            error_code,
            error_message,
            error_reason,
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
            amount_captured: None,
            minor_amount_captured: None,
            network_txn_id: None,
            payment_method_update: None,
            sender_payment_instrument_id: None,
        })
    }

    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, Report<WebhookError>> {
        let details: stripe::WebhookEvent = request
            .body
            .parse_struct("WebhookEvent")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        let event_object = details.event_data.event_object;

        let status = match event_object.status.as_ref() {
            Some(stripe::WebhookEventStatus::Succeeded) => common_enums::RefundStatus::Success,
            Some(stripe::WebhookEventStatus::Failed) => common_enums::RefundStatus::Failure,
            _ => common_enums::RefundStatus::Pending,
        };

        let (error_code, error_message) = if status == common_enums::RefundStatus::Failure {
            let error = event_object.last_payment_error.as_ref();
            (
                error.and_then(|error| error.code.clone()),
                error.and_then(|error| error.message.clone()),
            )
        } else {
            (None, None)
        };

        Ok(RefundWebhookDetailsResponse {
            connector_refund_id: Some(event_object.id.clone()),
            status,
            connector_response_reference_id: Some(event_object.id),
            error_code,
            error_message,
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
        })
    }

    fn process_dispute_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<DisputeWebhookDetailsResponse, Report<WebhookError>> {
        let details: stripe::WebhookEvent = request
            .body
            .parse_struct("WebhookEvent")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        let event_object = details.event_data.event_object;

        let amount = event_object.amount.ok_or_else(|| {
            report!(WebhookError::WebhookMissingRequiredField { field: "amount" })
        })?;

        let amount = domain_types::utils::convert_amount_for_webhook(
            &StringMinorUnitForConnector,
            amount,
            event_object.currency,
        )?;

        let status = event_object
            .status
            .as_ref()
            .map(dispute_status_to_dispute_status)
            .unwrap_or(common_enums::DisputeStatus::DisputeOpened);

        Ok(DisputeWebhookDetailsResponse {
            amount,
            currency: event_object.currency,
            dispute_id: event_object.id.clone(),
            status,
            stage: common_enums::DisputeStage::Dispute,
            connector_response_reference_id: Some(event_object.id),
            dispute_message: event_object.reason,
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
            connector_reason_code: None,
        })
    }

    fn get_webhook_resource_object(
        &self,
        request: RequestDetails,
    ) -> Result<Box<dyn hyperswitch_masking::ErasedMaskSerialize>, Report<WebhookError>> {
        let details: stripe::WebhookEvent = request
            .body
            .parse_struct("WebhookEvent")
            .change_context(WebhookError::WebhookResourceObjectNotFound)?;

        Ok(Box::new(details.event_data.event_object))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Stripe<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Stripe<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Stripe<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Stripe<T>
{
    fn should_create_connector_customer(&self) -> bool {
        true
    }
    fn should_do_payment_method_token(
        &self,
        payment_method: common_enums::PaymentMethod,
        payment_method_type: Option<common_enums::PaymentMethodType>,
    ) -> bool {
        matches!(payment_method, common_enums::PaymentMethod::Wallet)
            && !matches!(
                payment_method_type,
                Some(common_enums::PaymentMethodType::GooglePay)
            )
    }
}

macros::create_amount_converter_wrapper!(connector_name: Stripe, amount_type: MinorUnit);
macros::create_all_prerequisites!(
    connector_name: Stripe,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: PaymentIntentRequest<T>,
            response_body: PaymentsAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: RepeatPaymentRequest<T>,
            response_body: RepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: PaymentSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: CaptureRequest,
            response_body: PaymentsCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: CancelRequest,
            response_body: PaymentsVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: StripeRefundRequest,
            response_body: RefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: RefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: PaymentMethodToken,
            request_body: TokenRequest<T>,
            response_body: StripeTokenResponse,
            router_data: RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>,
        ),
        (
            flow: SetupMandate,
            request_body: SetupMandateRequest<T>,
            response_body: SetupMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: CreateConnectorCustomer,
            request_body: CreateConnectorCustomerRequest,
            response_body: CreateConnectorCustomerResponse,
            router_data: RouterDataV2<CreateConnectorCustomer, PaymentFlowData, ConnectorCustomerData, ConnectorCustomerResponse>,
        ),
        (
            flow: IncrementalAuthorization,
            request_body: PaymentIncrementalAuthRequest,
            response_body: PaymentIncrementalAuthResponse,
            router_data: RouterDataV2<IncrementalAuthorization, PaymentFlowData, PaymentsIncrementalAuthorizationData, PaymentsResponseData>,
        ),
        (
            flow: ClientAuthenticationToken,
            request_body: StripeClientAuthRequest,
            response_body: StripeClientAuthResponse,
            router_data: RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        )
    ],
    amount_converters: [],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                Self::common_get_content_type(self).to_string().into(),
            )];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);
            Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.stripe.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.stripe.base_url
        }

        pub fn connector_base_url_merchant_auth<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, MerchantAuthenticationFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.stripe.base_url
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Stripe<T>
{
    fn id(&self) -> &'static str {
        "stripe"
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/x-www-form-urlencoded"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        // &self.base_url
        connectors.stripe.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = stripe::StripeAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        Ok(vec![
            (
                headers::AUTHORIZATION.to_string(),
                format!("Bearer {}", auth.api_key.peek()).into_masked(),
            ),
            (
                auth_headers::STRIPE_API_VERSION.to_string(),
                auth_headers::STRIPE_VERSION.to_string().into_masked(),
            ),
        ])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: stripe::ErrorResponse =
            res.response.parse_struct("ErrorResponse").change_context(
                crate::utils::response_handling_fail_for_connector(res.status_code, "stripe"),
            )?;

        with_error_response_body!(event_builder, response);

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response
                .error
                .code
                .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            message: response
                .error
                .message
                .clone()
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            reason: response.error.message.map(|message| {
                response
                    .error
                    .decline_code
                    .clone()
                    .map(|decline_code| {
                        format!("message - {message}, decline_code - {decline_code}")
                    })
                    .unwrap_or(message)
            }),
            attempt_status: None,
            connector_transaction_id: response.error.payment_intent.map(|pi| pi.id),
            network_advice_code: response.error.network_advice_code,
            network_decline_code: response.error.network_decline_code,
            network_error_message: response.error.decline_code.or(response.error.advice_code),
        })
    }
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Stripe,
    curl_request: FormUrlEncoded(PaymentIntentRequest),
    curl_response: PaymentsAuthorizeResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type()
                    .to_string()
                    .into(),
            )];

            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);

            let stripe_split_payment_metadata = stripe::StripeSplitPaymentRequest::try_from(req)?;

            // if the request has split payment object, then append the transfer account id in headers in charge_type is Direct
            if let Some(domain_types::connector_types::SplitPaymentsDetails::StripeSplitPayment(
                stripe_split_payment,
            )) = &req.request.split_payments
            {
                if stripe_split_payment.charge_type
                    ==common_enums::PaymentChargeType::Stripe(common_enums::StripeChargeType::Direct)
                {
                    let mut customer_account_header = vec![(
                        headers::STRIPE_COMPATIBLE_CONNECT_ACCOUNT.to_string(),
                        stripe_split_payment
                            .transfer_account_id
                            .clone()
                            .into_masked(),
                    )];
                    header.append(&mut customer_account_header);
                }
            }
            // if request doesn't have transfer_account_id, but stripe_split_payment_metadata has it, append it
            else if let Some(transfer_account_id) =
                stripe_split_payment_metadata.transfer_account_id.clone()
            {
                let mut customer_account_header = vec![(
                    headers::STRIPE_COMPATIBLE_CONNECT_ACCOUNT.to_string(),
                    transfer_account_id.into_masked(),
                )];
                header.append(&mut customer_account_header);
            }
            Ok(header)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}{}",
                self.connector_base_url_payments(req),
                "v1/payment_intents"
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Stripe,
    curl_request: FormUrlEncoded(RepeatPaymentRequest),
    curl_response: RepeatPaymentResponse,
    flow_name: RepeatPayment,
    resource_common_data: PaymentFlowData,
    flow_request: RepeatPaymentData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type()
                    .to_string()
                    .into(),
            )];

            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);

            let stripe_split_payment_metadata = stripe::StripeSplitPaymentRequest::try_from(req)?;

            let transfer_account_id = req
                .request
                .split_payments
                .as_ref()
                .and_then(|split_payments| {
                    if let domain_types::connector_types::SplitPaymentsDetails::StripeSplitPayment(stripe_split_payment) =
                        split_payments
                    {
                        Some(stripe_split_payment)
                    } else {
                        None
                    }
                })
                .filter(|stripe_split_payment| {
                    matches!(stripe_split_payment.charge_type, common_enums::PaymentChargeType::Stripe(common_enums::StripeChargeType::Direct))
                })
                .map(|stripe_split_payment| stripe_split_payment.transfer_account_id.clone())
                .or_else(|| stripe_split_payment_metadata.transfer_account_id.clone().map(|s| s.expose()));

            if let Some(transfer_account_id) = transfer_account_id {
                let mut customer_account_header = vec![(
                    headers::STRIPE_COMPATIBLE_CONNECT_ACCOUNT.to_string(),
                    transfer_account_id.clone().into_masked(),
                )];
                header.append(&mut customer_account_header);
            };
            Ok(header)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}{}",
                self.connector_base_url_payments(req),
                "v1/payment_intents"
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Stripe,
    curl_request: FormUrlEncoded(TokenRequest),
    curl_response: StripeTokenResponse,
    flow_name: PaymentMethodToken,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentMethodTokenizationData<T>,
    flow_response: PaymentMethodTokenResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )];
            let transfer_account_id = req
                .request
                .split_payments
                .as_ref()
                .and_then(|split_payments| {
                    if let domain_types::connector_types::SplitPaymentsDetails::StripeSplitPayment(stripe_split_payment) =
                        split_payments
                    {
                        Some(stripe_split_payment)
                    } else {
                        None
                    }
                })
                .filter(|stripe_split_payment| {
                    matches!(stripe_split_payment.charge_type, common_enums::PaymentChargeType::Stripe(common_enums::StripeChargeType::Direct))
                })
                .map(|stripe_split_payment| stripe_split_payment.transfer_account_id.clone());

            if let Some(transfer_account_id) = transfer_account_id {
                let mut customer_account_header = vec![(
                    headers::STRIPE_COMPATIBLE_CONNECT_ACCOUNT.to_string(),
                    transfer_account_id.clone().into_masked(),
                )];
                header.append(&mut customer_account_header);
            };

            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);
            Ok(header)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}{}",
                self.connector_base_url_payments(req),
                "v1/payment_methods"
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Stripe,
    curl_request: FormUrlEncoded(SetupMandateRequest),
    curl_response: SetupMandateResponse,
    flow_name: SetupMandate,
    resource_common_data: PaymentFlowData,
    flow_request: SetupMandateRequestData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + std::marker::Sync + std::marker::Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);
            Ok(header)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}{}",
                self.connector_base_url_payments(req),
                "v1/setup_intents"
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Stripe,
    curl_request: FormUrlEncoded(CreateConnectorCustomerRequest),
    curl_response: CreateConnectorCustomerResponse,
    flow_name: CreateConnectorCustomer,
    resource_common_data: PaymentFlowData,
    flow_request: ConnectorCustomerData,
    flow_response: ConnectorCustomerResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + std::marker::Sync + std::marker::Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<CreateConnectorCustomer, PaymentFlowData, ConnectorCustomerData, ConnectorCustomerResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type()
                    .to_string()
                    .into(),
            )];
            let transfer_account_id = req
                .request
                .split_payments
                .as_ref()
                .and_then(|split_payments| {
                    if let domain_types::connector_types::SplitPaymentsDetails::StripeSplitPayment(stripe_split_payment) =
                        split_payments
                    {
                        Some(stripe_split_payment)
                    } else {
                        None
                    }
                })
                .filter(|stripe_split_payment| {
                    matches!(stripe_split_payment.charge_type, common_enums::PaymentChargeType::Stripe(common_enums::StripeChargeType::Direct))
                })
                .map(|stripe_split_payment| stripe_split_payment.transfer_account_id.clone());

            if let Some(transfer_account_id) = transfer_account_id {
                let mut customer_account_header = vec![(
                    headers::STRIPE_COMPATIBLE_CONNECT_ACCOUNT.to_string(),
                    transfer_account_id.clone().into_masked(),
                )];
                header.append(&mut customer_account_header);
            };

            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);
            Ok(header)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<CreateConnectorCustomer, PaymentFlowData, ConnectorCustomerData, ConnectorCustomerResponse>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}{}", self.connector_base_url_payments(req), "v1/customers"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Stripe,
    curl_response: PaymentSyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + std::marker::Sync + std::marker::Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);

            if let Some(domain_types::connector_types::SplitPaymentsDetails::StripeSplitPayment(
                stripe_split_payment,
            )) = &req.request.split_payments
            {
                transformers::transform_headers_for_connect_platform(
                    stripe_split_payment.charge_type.clone(),
                    Secret::new(stripe_split_payment.transfer_account_id.clone()),
                    &mut header,
                );
            }
            Ok(header)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let id = req.request.connector_transaction_id.clone();

            match id.get_connector_transaction_id() {
                Ok(x) if x.starts_with("set") => Ok(format!(
                    "{}{}/{}?expand[0]=latest_attempt", // expand latest attempt to extract payment checks and three_d_secure data
                    self.connector_base_url_payments(req),
                    "v1/setup_intents",
                    x,
                )),
                Ok(x) => Ok(format!(
                    "{}{}/{}{}",
                    self.connector_base_url_payments(req),
                    "v1/payment_intents",
                    x,
                    "?expand[0]=latest_charge" //updated payment_id(if present) reside inside latest_charge field
                )),
                x => x.change_context(IntegrationError::MissingConnectorTransactionID { context: Default::default() })
}
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Stripe,
    curl_request: FormUrlEncoded(CaptureRequest),
    curl_response: PaymentsCaptureResponse,
    flow_name: Capture,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + std::marker::Sync + std::marker::Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                Self::common_get_content_type(self).to_string().into(),
            )];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);
            Ok(header)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let id = req.request.connector_transaction_id.get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID { context: Default::default() })?;
            Ok(format!(
                "{}{}/{}/capture",
                self.connector_base_url_payments(req),
                "v1/payment_intents",
                id
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Stripe,
    curl_request: FormUrlEncoded(CancelRequest),
    curl_response: PaymentsVoidResponse,
    flow_name: Void,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentVoidData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + std::marker::Sync + std::marker::Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);
            Ok(header)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let payment_id = &req.request.connector_transaction_id;
            Ok(format!(
                "{}v1/payment_intents/{}/cancel",
                self.connector_base_url_payments(req),
                payment_id
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Stripe,
    curl_request: FormUrlEncoded(PaymentIncrementalAuthRequest),
    curl_response: PaymentIncrementalAuthResponse,
    flow_name: IncrementalAuthorization,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsIncrementalAuthorizationData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + std::marker::Sync + std::marker::Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<IncrementalAuthorization, PaymentFlowData, PaymentsIncrementalAuthorizationData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<IncrementalAuthorization, PaymentFlowData, PaymentsIncrementalAuthorizationData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let payment_id = &req.request.connector_transaction_id.get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID { context: Default::default() })?;
            Ok(format!(
                "{}v1/payment_intents/{}/increment_authorization",
                self.connector_base_url_payments(req),
                payment_id
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Stripe,
    curl_request: FormUrlEncoded(StripeRefundRequest),
    curl_response: RefundResponse,
    flow_name: Refund,
    resource_common_data: RefundFlowData,
    flow_request: RefundsData,
    flow_response: RefundsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + std::marker::Sync + std::marker::Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);

            if let Some(domain_types::connector_types::SplitRefundsDetails::StripeSplitRefund(ref stripe_split_refund)) =
                req.request.split_refunds.as_ref()
            {
                match &stripe_split_refund.charge_type {
                    common_enums::PaymentChargeType::Stripe(stripe_charge) => {
                        if stripe_charge == &common_enums::StripeChargeType::Direct {
                            let mut customer_account_header = vec![(
                                headers::STRIPE_COMPATIBLE_CONNECT_ACCOUNT.to_string(),
                                stripe_split_refund
                                    .transfer_account_id
                                    .clone()
                                    .into_masked(),
                            )];
                            header.append(&mut customer_account_header);
                        }
                    }
                }
            }
            Ok(header)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}{}", self.connector_base_url_refunds(req), "v1/refunds"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Stripe,
    curl_response: RefundSyncResponse,
    flow_name: RSync,
    resource_common_data: RefundFlowData,
    flow_request: RefundSyncData,
    flow_response: RefundsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + std::marker::Sync + std::marker::Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);

            if let Some(domain_types::connector_types::SplitRefundsDetails::StripeSplitRefund(ref stripe_refund)) =
                req.request.split_refunds.as_ref()
            {
                transformers::transform_headers_for_connect_platform(
                    stripe_refund.charge_type.clone(),
                    Secret::new(stripe_refund.transfer_account_id.clone()),
                    &mut header,
                );
            }
            Ok(header)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let id = req.request.connector_refund_id.clone();
            Ok(format!("{}v1/refunds/{}", self.connector_base_url_refunds(req), id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Stripe,
    curl_request: FormUrlEncoded(StripeClientAuthRequest),
    curl_response: StripeClientAuthResponse,
    flow_name: ClientAuthenticationToken,
    resource_common_data: MerchantAuthenticationFlowData,
    flow_request: ClientAuthenticationTokenRequestData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}{}",
                self.connector_base_url_merchant_auth(req),
                "v1/payment_intents"
            ))
        }
    }
);

macros::macro_connector_flow_status_impls!(
    connector: Stripe,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        Accept,
        SubmitEvidence,
        DefendDispute,
        ServerSessionAuthenticationToken,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
    ],
    not_supported: [
        VoidPostRefund,
        VoidPC,
        MandateRevoke,
        CreateOrder,
        ServerAuthenticationToken,
    ],
);
