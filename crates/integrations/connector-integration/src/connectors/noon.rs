use std::fmt::Debug;

use base64::Engine;
use common_enums::{AttemptStatus, RefundStatus};
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    crypto::{self, VerifySignature},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    types::StringMajorUnit,
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, MandateRevoke, PSync, RSync, Refund, RepeatPayment, SetupMandate, Void,
    },
    connector_types::{
        ConnectorSpecifications, ConnectorWebhookSecrets, EventContext, EventType,
        MandateRevokeRequestData, MandateRevokeResponseData, PaymentFlowData, PaymentVoidData,
        PaymentWebhookReference, PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundWebhookDetailsResponse,
        RefundWebhookReference, RefundsData, RefundsResponseData, RepeatPaymentData,
        RequestDetails, ResponseId, SetupMandateRequestData, WebhookDetailsResponse,
        WebhookResourceReference,
    },
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use hyperswitch_masking::{Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
pub mod transformers;
use error_stack::{report, ResultExt};
pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
/// Noon `resultCode` for "Order referenced in the request does not exist".
const ORDER_NOT_FOUND_RESULT_CODE: &str = "19001";

use transformers::{
    self as noon, NoonAuthType, NoonErrorResponse, NoonPaymentsActionRequest,
    NoonPaymentsActionRequest as NoonPaymentsRefundActionRequest, NoonPaymentsCancelRequest,
    NoonPaymentsRequest, NoonPaymentsResponse, NoonPaymentsResponse as NoonPaymentsSyncResponse,
    NoonPaymentsResponse as NoonPaymentsCaptureResponse, NoonRepeatPaymentRequest,
    NoonRepeatPaymentResponse, NoonRevokeMandateRequest, NoonRevokeMandateResponse,
    NoonVoidResponse, RefundResponse, RefundSyncResponse, SetupMandateRequest,
    SetupMandateResponse,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};
use domain_types::errors::ConnectorError;
use domain_types::errors::{IntegrationError, WebhookError};

// Local headers module
mod headers {
    pub const CONTENT_TYPE: &str = "Content-Type";
    pub const AUTHORIZATION: &str = "Authorization";
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Noon<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Noon<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Noon<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Noon<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Noon<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Noon<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Noon<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Noon<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Noon<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Noon<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Noon<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Noon<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Noon<T>
{
}
/// Maps a signed (`eventType`, `orderStatus`) pair to an event (spec ## Webhook Events).
///
/// Only the pairs the spec documents are mapped; every other pair (3DS, Authenticate, Cancel,
/// Review, Lock, an unknown event, or an undocumented status) is
/// `IncomingWebhookEventUnspecified`, so it can never move a payment or refund to a terminal state.
fn get_noon_webhook_event_type(
    event_type: &noon::NoonWebhookEventTypes,
    order_status: &noon::NoonPaymentStatus,
) -> EventType {
    use noon::{NoonPaymentStatus as Status, NoonWebhookEventTypes as Event};

    match event_type {
        Event::Sale => match order_status {
            Status::Captured => EventType::PaymentIntentSuccess,
            _ => EventType::IncomingWebhookEventUnspecified,
        },
        Event::Capture => match order_status {
            Status::Captured => EventType::PaymentIntentSuccess,
            Status::PartiallyCaptured => EventType::PaymentIntentCaptureSuccess,
            _ => EventType::IncomingWebhookEventUnspecified,
        },
        Event::Authorize => match order_status {
            Status::Authorized => EventType::PaymentIntentAuthorizationSuccess,
            Status::Failed => EventType::PaymentIntentAuthorizationFailure,
            Status::Pending | Status::MarkedForReview => EventType::PaymentIntentProcessing,
            _ => EventType::IncomingWebhookEventUnspecified,
        },
        // Fail carries the order's resulting status: only a failed order is a payment failure.
        // A failed later operation (e.g. a refund on a CAPTURED order) must not be terminal.
        Event::Fail => match order_status {
            Status::Failed | Status::Rejected | Status::Cancelled | Status::Expired => {
                EventType::PaymentIntentFailure
            }
            Status::Initiated
            | Status::Authorized
            | Status::Captured
            | Status::PartiallyCaptured
            | Status::PartiallyRefunded
            | Status::PaymentInfoAdded
            | Status::ThreeDsEnrollInitiated
            | Status::ThreeDsEnrollChecked
            | Status::ThreeDsResultVerified
            | Status::MarkedForReview
            | Status::Authenticated
            | Status::PartiallyReversed
            | Status::Pending
            | Status::Refunded
            | Status::Reversed
            | Status::Locked
            | Status::Unknown => EventType::IncomingWebhookEventUnspecified,
        },
        Event::Reject => match order_status {
            Status::Rejected => EventType::PaymentIntentFailure,
            _ => EventType::IncomingWebhookEventUnspecified,
        },
        Event::Reverse => match order_status {
            Status::Reversed => EventType::PaymentIntentCancelled,
            // AUTHORIZED on a Reverse event means the reversal failed (spec ## Webhook Events).
            Status::Authorized => EventType::PaymentIntentCancelFailure,
            _ => EventType::IncomingWebhookEventUnspecified,
        },
        Event::Refund => match order_status {
            Status::Refunded | Status::PartiallyRefunded => EventType::RefundSuccess,
            Status::Pending => EventType::RefundProcessing,
            _ => EventType::IncomingWebhookEventUnspecified,
        },
        Event::ThreeDs
        | Event::Authenticate
        | Event::Cancel
        | Event::Review
        | Event::Lock
        | Event::Unknown => EventType::IncomingWebhookEventUnspecified,
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Noon<T>
{
    fn get_webhook_source_verification_signature(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        let webhook_body: noon::NoonWebhookSignature = request
            .body
            .parse_struct("NoonWebhookSignature")
            .change_context(WebhookError::WebhookSignatureNotFound)
            .attach_printable("Missing incoming webhook signature for noon")?;
        let signature = webhook_body.signature;
        BASE64_ENGINE
            .decode(signature)
            .change_context(WebhookError::WebhookSignatureNotFound)
            .attach_printable("Missing incoming webhook signature for noon")
    }

    fn get_webhook_source_verification_message(
        &self,
        request: &RequestDetails,
        _connector_webhook_secrets: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        let webhook_body: noon::NoonWebhookBody = request
            .body
            .parse_struct("NoonWebhookBody")
            .change_context(WebhookError::WebhookSignatureNotFound)
            .attach_printable("Missing incoming webhook signature for noon")?;
        // v1 preimage (spec ## Webhook Authentication & Signature Verification): the raw,
        // case-sensitive values joined by commas in exactly this order.
        let message = format!(
            "{},{},{},{},{}",
            webhook_body.order_id,
            webhook_body.order_status,
            webhook_body.event_id,
            webhook_body.event_type,
            webhook_body.time_stamp,
        );
        Ok(message.into_bytes())
    }

    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"{"orderId":12345,"orderStatus":"CAPTURED","eventType":"Sale","eventId":"probe-event-001","timeStamp":"2024-01-01T00:00:00Z"}"#
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        // Noon checks the endpoint with an empty body.
        if request.body.is_empty() {
            return Ok(EventType::EndpointVerification);
        }
        let details: noon::NoonWebhookEvent = request
            .body
            .parse_struct("NoonWebhookEvent")
            .change_context(WebhookError::WebhookBodyDecodingFailed)
            .attach_printable("Failed to parse webhook event type from Noon webhook body")?;

        Ok(get_noon_webhook_event_type(
            &details.event_type,
            &details.order_status,
        ))
    }

    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        let algorithm = crypto::HmacSha512;

        let connector_webhook_secrets = match connector_webhook_secret {
            Some(secrets) => secrets,
            None => {
                return Err(report!(WebhookError::WebhookVerificationSecretNotFound));
            }
        };

        let signature =
            self.get_webhook_source_verification_signature(&request, &connector_webhook_secrets)?;

        let message =
            self.get_webhook_source_verification_message(&request, &connector_webhook_secrets)?;

        algorithm
            .verify_signature(&connector_webhook_secrets.secret, &signature, &message)
            .change_context(WebhookError::WebhookSourceVerificationFailed)
            .attach_printable("Noon webhook signature verification failed")
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        let webhook_object: noon::NoonWebhookObject = request
            .body
            .parse_struct("NoonWebhookObject")
            .change_context(WebhookError::WebhookBodyDecodingFailed)
            .attach_printable("Failed to parse NoonWebhookObject for reference extraction")?;

        // Noon's order_id is the connector transaction ID and the only identifier covered by
        // the v1 signature. A refund event also echoes the refund's merchant reference
        // (merchantTxnRef), but that field is NOT in the signed preimage, so it can be edited
        // on a validly signed body and is never used to pick a refund. Noon documents no
        // chargeback / dispute event, so no dispute reference is produced.
        let connector_transaction_id = Some(webhook_object.order_id.to_string());
        match webhook_object.event_type {
            noon::NoonWebhookEventTypes::Refund => Ok(Some(WebhookResourceReference::Refund(
                RefundWebhookReference {
                    connector_refund_id: None,
                    merchant_refund_id: None,
                    connector_transaction_id,
                    merchant_transaction_id: None,
                },
            ))),
            noon::NoonWebhookEventTypes::ThreeDs
            | noon::NoonWebhookEventTypes::Authenticate
            | noon::NoonWebhookEventTypes::Authorize
            | noon::NoonWebhookEventTypes::Cancel
            | noon::NoonWebhookEventTypes::Capture
            | noon::NoonWebhookEventTypes::Fail
            | noon::NoonWebhookEventTypes::Lock
            | noon::NoonWebhookEventTypes::Reject
            | noon::NoonWebhookEventTypes::Reverse
            | noon::NoonWebhookEventTypes::Review
            | noon::NoonWebhookEventTypes::Sale
            | noon::NoonWebhookEventTypes::Unknown => Ok(Some(WebhookResourceReference::Payment(
                PaymentWebhookReference {
                    connector_transaction_id,
                    merchant_transaction_id: None,
                    connector_preprocessing_id: None,
                },
            ))),
        }
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let webhook_object: noon::NoonWebhookObject = request
            .body
            .parse_struct("NoonWebhookObject")
            .change_context(WebhookError::WebhookBodyDecodingFailed)
            .attach_printable("Failed to parse payment webhook details from Noon webhook body")?;

        // Same mapping as PSync, which reports PARTIALLY_CAPTURED as PartialCharged: the
        // webhook carries no currency and no order total, so it must not imply the full
        // authorised amount was captured. AUTHORIZED is Authorized whatever the capture
        // method; LOCKED / unknown statuses are never terminal.
        let status =
            noon::get_payment_status(webhook_object.order_status, AttemptStatus::Unspecified);

        let connector_order_id = webhook_object.order_id.to_string();
        Ok(WebhookDetailsResponse {
            connector_returned_payment_method_details: None,
            resource_id: Some(ResponseId::ConnectorTransactionId(
                connector_order_id.clone(),
            )),
            status,
            connector_response_reference_id: Some(connector_order_id.clone()),
            // The signed v1 preimage is orderId, orderStatus, eventId, eventType, timeStamp.
            // merchantOrderRef is NOT in it, so it can be edited on a validly signed body and
            // must not be what a payment is correlated by -- the signed orderId is used for
            // both references, as it was before this flow was reworked.
            connector_request_reference_id: Some(connector_order_id),
            error_code: None,
            error_message: None,
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
            mandate_reference: None,
            amount_captured: None,
            error_reason: None,
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
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let webhook_object: noon::NoonWebhookObject = request
            .body
            .parse_struct("NoonWebhookObject")
            .change_context(WebhookError::WebhookBodyDecodingFailed)
            .attach_printable("Failed to parse refund webhook details from Noon webhook body")?;

        // The signed fields (eventType, orderStatus, orderId) say that some refund on the order
        // changed, never which one: merchantTxnRef is unsigned. The webhook is therefore a
        // trigger only. It reports Pending with no refund identity, so no refund can be moved
        // to a terminal state from it; the outcome is confirmed by RSync (getbyreference).
        // Anything other than a Refund event on a refund status is not a refund outcome.
        let status = match (&webhook_object.event_type, &webhook_object.order_status) {
            (
                noon::NoonWebhookEventTypes::Refund,
                noon::NoonPaymentStatus::Refunded
                | noon::NoonPaymentStatus::PartiallyRefunded
                | noon::NoonPaymentStatus::Pending,
            ) => RefundStatus::Pending,
            (event_type, order_status) => {
                return Err(
                    report!(WebhookError::WebhookEventTypeNotFound).attach_printable(format!(
                        "Noon webhook {event_type}/{order_status} is not a refund outcome"
                    )),
                );
            }
        };

        Ok(RefundWebhookDetailsResponse {
            connector_refund_id: None,
            merchant_transaction_id: None,
            status,
            connector_response_reference_id: None,
            error_code: None,
            error_message: None,
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
        })
    }

    fn get_webhook_resource_object(
        &self,
        request: RequestDetails,
    ) -> Result<Box<dyn hyperswitch_masking::ErasedMaskSerialize>, error_stack::Report<WebhookError>>
    {
        let webhook_object: noon::NoonWebhookObject = request
            .body
            .parse_struct("NoonWebhookObject")
            .change_context(WebhookError::WebhookResourceObjectNotFound)
            .attach_printable("Failed to parse webhook resource object from Noon webhook body")?;

        let resource: Box<dyn hyperswitch_masking::ErasedMaskSerialize> =
            Box::new(NoonPaymentsResponse::from(webhook_object));
        Ok(resource)
    }
}
macros::macro_connector_payout_implementation!(
    connector: Noon,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::MandateRevokeV2 for Noon<T>
{
}

macros::create_all_prerequisites!(
    connector_name: Noon,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: NoonPaymentsRequest<T>,
            response_body: NoonPaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: NoonPaymentsSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: NoonPaymentsActionRequest,
            response_body: NoonPaymentsCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: NoonPaymentsCancelRequest,
            response_body: NoonVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: NoonPaymentsRefundActionRequest,
            response_body: RefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: SetupMandateRequest<T>,
            response_body: SetupMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: RSync,
            response_body: RefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: MandateRevoke,
            request_body: NoonRevokeMandateRequest,
            response_body: NoonRevokeMandateResponse,
            router_data: RouterDataV2<MandateRevoke, PaymentFlowData, MandateRevokeRequestData, MandateRevokeResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: NoonRepeatPaymentRequest<T>,
            response_body: NoonRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: StringMajorUnit
    ],
    member_functions: {

        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
        let mut header = vec![(
            headers::CONTENT_TYPE.to_string(),
            self.get_content_type().to_string().into(),
        )];
        let mut auth_header = self.get_auth_header(&req.connector_config)?;
        header.append(&mut auth_header);
        Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.noon.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.noon.base_url
        }

        pub fn get_auth_header(
            &self,
            auth_type: &ConnectorSpecificConfig,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let auth = NoonAuthType::try_from(auth_type)?;

            let encoded_api_key = auth
                .business_identifier
                .zip(auth.application_identifier)
                .zip(auth.api_key)
                .map(|((business_identifier, application_identifier), api_key)| {
                    BASE64_ENGINE.encode(format!(
                        "{business_identifier}.{application_identifier}:{api_key}",
                    ))
                });

            Ok(vec![(
                headers::AUTHORIZATION.to_string(),
                format!("Key {}", encoded_api_key.peek()).into_masked(),
            )])
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Noon<T>
{
    fn id(&self) -> &'static str {
        "noon"
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.noon.base_url.as_ref()
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        // Noon sometimes answers an error status with no body at all.
        if res.response.is_empty() {
            return Ok(ErrorResponse {
                status_code: res.status_code,
                code: NO_ERROR_CODE.to_string(),
                message: NO_ERROR_MESSAGE.to_string(),
                reason: None,
                attempt_status: None,
                connector_transaction_id: None,
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            });
        }

        // A body that is not Noon's error envelope: an HTML / plain-text gateway page (WAF,
        // 5xx) becomes an ErrorResponse carrying the HTTP status and the raw body, while JSON
        // of an unexpected shape stays a deserialization failure (Hyperswitch parity).
        let response: NoonErrorResponse = match res.response.parse_struct("NoonErrorResponse") {
            Ok(response) => response,
            Err(_) => {
                return crate::utils::handle_json_response_deserialization_failure(res, "noon");
            }
        };

        with_error_response_body!(event_builder, response);

        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");
        // attempt_status stays None here: what an error envelope means for the attempt is
        // flow-specific, so a flow that needs a status sets it in its own error handler.
        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.result_code.to_string(),
            message: response.message.clone(),
            reason: Some(response.message),
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: response.raw_external_message,
            typed_connector_response: typed,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        })
    }
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Noon,
    curl_request: Json(NoonPaymentsRequest),
    curl_response: NoonPaymentsResponse,
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
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}payment/v1/order", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Noon,
    curl_request: Json(SetupMandateRequest<T>),
    curl_response: SetupMandateResponse,
    flow_name: SetupMandate,
    resource_common_data: PaymentFlowData,
    flow_request: SetupMandateRequestData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}payment/v1/order", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Noon,
    curl_response: NoonPaymentsSyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
        Ok(format!(
            "{}payment/v1/order/getbyreference/{}",
            self.connector_base_url_payments(req),
            req.resource_common_data.connector_request_reference_id,
        ))
        }

        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            let mut error = self.build_error_response(res, event_builder, connector_config)?;
            // resultCode 19001 = "order referenced in the request does not exist": the order was
            // never created (e.g. Authorize timed out before Noon recorded it), so a sync that
            // gets it is a definitive answer and the attempt is failed. Every other error leaves
            // the attempt status untouched.
            if error.code == ORDER_NOT_FOUND_RESULT_CODE {
                error.attempt_status = Some(FlowStatus::Payment(AttemptStatus::Failure));
            }
            Ok(error)
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Noon,
    curl_request: Json(NoonPaymentsActionRequest),
    curl_response: NoonPaymentsCaptureResponse,
    flow_name: Capture,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}payment/v1/order", self.connector_base_url_payments(req)))
        }
    }
);

// Add implementation for Void
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Noon,
    curl_request: Json(NoonPaymentsCancelRequest),
    curl_response: NoonVoidResponse,
    flow_name: Void,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentVoidData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
             Ok(format!("{}payment/v1/order", self.connector_base_url_payments(req),))
        }
    }
);

// Add implementation for Refund
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Noon,
    curl_request: Json(NoonPaymentsRefundActionRequest),
    curl_response: RefundResponse,
    flow_name: Refund,
    resource_common_data: RefundFlowData,
    flow_request: RefundsData,
    flow_response: RefundsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}payment/v1/order", self.connector_base_url_refunds(req)))
        }
    }
);

// Implement RSync to fix the RefundSyncV2 trait requirement
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Noon,
    curl_response: RefundSyncResponse,
    flow_name: RSync,
    resource_common_data: RefundFlowData,
    flow_request: RefundSyncData,
    flow_response: RefundsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // getbyreference is keyed by the payment's merchant reference, never the refund id.
            let order_reference = noon::get_refund_sync_order_reference(
                &req.request,
                &req.resource_common_data.connector_request_reference_id,
            )?;
            Ok(format!(
                "{}payment/v1/order/getbyreference/{}",
                self.connector_base_url_refunds(req),
                order_reference,
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Noon,
    curl_request: Json(NoonRevokeMandateRequest),
    curl_response: NoonRevokeMandateResponse,
    flow_name: MandateRevoke,
    resource_common_data: PaymentFlowData,
    flow_request: MandateRevokeRequestData,
    flow_response: MandateRevokeResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<MandateRevoke, PaymentFlowData, MandateRevokeRequestData, MandateRevokeResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<MandateRevoke, PaymentFlowData, MandateRevokeRequestData, MandateRevokeResponseData>,
        ) -> CustomResult<String, IntegrationError> {
             Ok(format!("{}payment/v1/order", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Noon,
    curl_request: Json(NoonRepeatPaymentRequest<T>),
    curl_response: NoonRepeatPaymentResponse,
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
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}payment/v1/order", self.connector_base_url_payments(req)))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorSpecifications
    for Noon<T>
{
}

// We already have an implementation for ValidationTrait above

// ConnectorIntegrationV2 implementations for authentication flows

// SourceVerification implementations for authentication flows

macros::macro_connector_flow_status_impls!(
    connector: Noon,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        VoidPC,
        CreateOrder,
        ServerSessionAuthenticationToken,
        PaymentMethodToken,
        // 3DS needs no separate authentication leg on Noon (zero-leg shape): the charging
        // INITIATE call (Authorize) itself returns `checkoutData.postUrl` for the cardholder
        // challenge, the browser returns to `returnUrl`, and PSync reads the final status.
        // Noon has no pre-authenticate, authenticate or post-authenticate endpoint.
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        ClientAuthenticationToken,
    ],
    not_supported: [
        VoidPostRefund,
        IncrementalAuthorization,
        Accept,
        SubmitEvidence,
        DefendDispute,
        ServerAuthenticationToken,
        CreateConnectorCustomer,
        GetConnectorCustomer,
    ],
);
