//! definition of the new connector integration trait

use common_utils::{
    events,
    request::{KafkaRecord, Method, Request, RequestBuilder, RequestContent, TransportType},
    CustomResult,
};
use domain_types::{
    connector_flow,
    errors::{ConnectorError, IntegrationError},
    router_data::ErrorResponse,
    router_data_v2::RouterDataV2,
};
use hyperswitch_masking::Maskable;
use serde_json::json;

use crate::api;

pub trait FlowDescriptor {
    const NAME: &'static str;
}

macro_rules! impl_flow_descriptor {
    ($($flow:ty => $name:literal),+ $(,)?) => {
        $(
            impl FlowDescriptor for $flow {
                const NAME: &'static str = $name;
            }
        )+
    };
}

impl_flow_descriptor!(
    connector_flow::CreateOrder => "create_order",
    connector_flow::Authorize => "authorize",
    connector_flow::PSync => "payment_sync",
    connector_flow::Void => "void",
    connector_flow::RSync => "refund_sync",
    connector_flow::Refund => "refund",
    connector_flow::Capture => "capture",
    connector_flow::SetupMandate => "setup_mandate",
    connector_flow::RepeatPayment => "repeat_payment",
    connector_flow::Accept => "accept_dispute",
    connector_flow::SubmitEvidence => "submit_evidence",
    connector_flow::DefendDispute => "defend_dispute",
    connector_flow::ServerSessionAuthenticationToken => "server_session_authentication_token",
    connector_flow::ServerAuthenticationToken => "server_authentication_token",
    connector_flow::CreateConnectorCustomer => "create_connector_customer",
    connector_flow::PaymentMethodToken => "payment_method_token",
    connector_flow::PreAuthenticate => "pre_authenticate",
    connector_flow::Authenticate => "authenticate",
    connector_flow::PostAuthenticate => "post_authenticate",
    connector_flow::VoidPC => "void_post_capture",
    connector_flow::ClientAuthenticationToken => "client_authentication_token",
    connector_flow::IncrementalAuthorization => "incremental_authorization",
    connector_flow::MandateRevoke => "mandate_revoke",
    connector_flow::VerifyWebhookSource => "verify_webhook_source",
    connector_flow::PayoutCreate => "payout_create",
    connector_flow::PayoutTransfer => "payout_transfer",
    connector_flow::PayoutGet => "payout_get",
    connector_flow::PayoutVoid => "payout_void",
    connector_flow::PayoutStage => "payout_stage",
    connector_flow::PayoutCreateLink => "payout_create_link",
    connector_flow::PayoutCreateRecipient => "payout_create_recipient",
    connector_flow::PayoutEnrollDisburseAccount => "payout_enroll_disburse_account",
);

/// alias for Box of a type that implements trait ConnectorIntegrationV2
pub type BoxedConnectorIntegrationV2<'a, Flow, ResourceCommonData, Req, Resp> =
    Box<&'a (dyn ConnectorIntegrationV2<Flow, ResourceCommonData, Req, Resp> + Send + Sync)>;

/// trait with a function that returns BoxedConnectorIntegrationV2
pub trait ConnectorIntegrationAnyV2<Flow, ResourceCommonData, Req, Resp>:
    Send + Sync + 'static
{
    /// function what returns BoxedConnectorIntegrationV2
    fn get_connector_integration_v2(
        &self,
    ) -> BoxedConnectorIntegrationV2<'_, Flow, ResourceCommonData, Req, Resp>;
}

impl<S, Flow: FlowDescriptor, ResourceCommonData, Req, Resp>
    ConnectorIntegrationAnyV2<Flow, ResourceCommonData, Req, Resp> for S
where
    S: ConnectorIntegrationV2<Flow, ResourceCommonData, Req, Resp> + Send + Sync,
{
    fn get_connector_integration_v2(
        &self,
    ) -> BoxedConnectorIntegrationV2<'_, Flow, ResourceCommonData, Req, Resp> {
        Box::new(self)
    }
}

/// The new connector integration trait with an additional ResourceCommonData generic parameter
pub trait ConnectorIntegrationV2<Flow: FlowDescriptor, ResourceCommonData, Req, Resp>:
    ConnectorIntegrationAnyV2<Flow, ResourceCommonData, Req, Resp> + Sync + api::ConnectorCommon
{
    /// returns a vec of tuple of header key and value
    fn get_headers(
        &self,
        _req: &RouterDataV2<Flow, ResourceCommonData, Req, Resp>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        Ok(vec![])
    }

    /// returns content type
    fn get_content_type(&self) -> &'static str {
        mime::APPLICATION_JSON.essence_str()
    }

    /// primarily used when creating signature based on request method of payment flow
    fn get_http_method(&self) -> Method {
        Method::Post
    }

    /// returns url
    fn get_url(
        &self,
        _req: &RouterDataV2<Flow, ResourceCommonData, Req, Resp>,
    ) -> CustomResult<String, IntegrationError>;

    /// returns request body
    fn get_request_body(
        &self,
        _req: &RouterDataV2<Flow, ResourceCommonData, Req, Resp>,
    ) -> CustomResult<Option<RequestContent>, IntegrationError> {
        Ok(None)
    }

    /// returns form data
    fn get_request_form_data(
        &self,
        _req: &RouterDataV2<Flow, ResourceCommonData, Req, Resp>,
    ) -> CustomResult<Option<reqwest::multipart::Form>, IntegrationError> {
        Ok(None)
    }

    fn get_transport_type(&self) -> TransportType {
        TransportType::Http
    }

    /// returns kafka topic
    fn get_kafka_topic(
        &self,
        _req: &RouterDataV2<Flow, ResourceCommonData, Req, Resp>,
    ) -> CustomResult<String, IntegrationError> {
        Ok(String::new())
    }

    /// returns kafka key
    fn get_kafka_key(
        &self,
        _req: &RouterDataV2<Flow, ResourceCommonData, Req, Resp>,
    ) -> CustomResult<Option<String>, IntegrationError> {
        Ok(None)
    }

    fn build_kafka_record(
        &self,
        _req: &RouterDataV2<Flow, ResourceCommonData, Req, Resp>,
    ) -> CustomResult<Option<KafkaRecord>, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(self.id(), Flow::NAME).into())
    }

    /// builds the request and returns it
    fn build_request_v2(
        &self,
        req: &RouterDataV2<Flow, ResourceCommonData, Req, Resp>,
    ) -> CustomResult<Option<Request>, IntegrationError> {
        Ok(Some(
            RequestBuilder::new()
                .method(self.get_http_method())
                .url(self.get_url(req)?.as_str())
                .attach_default_headers()
                .headers(self.get_headers(req)?)
                .set_optional_body(self.get_request_body(req)?)
                .add_certificate(self.get_certificate(req)?)
                .add_certificate_key(self.get_certificate_key(req)?)
                .build(),
        ))
    }

    /// accepts the raw api response and decodes it
    fn handle_response_v2(
        &self,
        data: &RouterDataV2<Flow, ResourceCommonData, Req, Resp>,
        event_builder: Option<&mut events::Event>,
        _res: domain_types::router_response_types::Response,
    ) -> CustomResult<RouterDataV2<Flow, ResourceCommonData, Req, Resp>, ConnectorError>
    where
        Flow: Clone,
        ResourceCommonData: Clone,
        Req: Clone,
        Resp: Clone,
    {
        if let Some(e) = event_builder {
            e.set_connector_response(&json!({"error": "Not Implemented"}))
        }
        Ok(data.clone())
    }

    /// accepts the raw api error response and decodes it
    fn get_error_response_v2(
        &self,
        res: domain_types::router_response_types::Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        if let Some(event) = event_builder {
            event.set_connector_response(&json!({"error": "Error response parsing not implemented", "status_code": res.status_code}))
        }
        Ok(ErrorResponse::get_not_implemented())
    }

    /// accepts the raw 5xx error response and decodes it
    fn get_5xx_error_response(
        &self,
        res: domain_types::router_response_types::Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let error_message = match res.status_code {
            500 => "internal_server_error",
            501 => "not_implemented",
            502 => "bad_gateway",
            503 => "service_unavailable",
            504 => "gateway_timeout",
            505 => "http_version_not_supported",
            506 => "variant_also_negotiates",
            507 => "insufficient_storage",
            508 => "loop_detected",
            510 => "not_extended",
            511 => "network_authentication_required",
            _ => "unknown_error",
        };

        if let Some(event) = event_builder {
            event.set_connector_response(
                &json!({"error": error_message, "status_code": res.status_code}),
            )
        }

        Ok(ErrorResponse {
            code: res.status_code.to_string(),
            message: error_message.to_string(),
            reason: String::from_utf8(res.response.to_vec()).ok(),
            status_code: res.status_code,
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        })
    }

    // whenever capture sync is implemented at the connector side, this method should be overridden
    /// retunes the capture sync method
    // fn get_multiple_capture_sync_method(
    //     &self,
    // ) -> CustomResult<api::CaptureSyncMethod, domain_types::errors::ConnectorError> {
    //     Err(domain_types::errors::ConnectorError::NotImplemented("multiple capture sync".into()).into())
    // }
    /// returns certificate string
    fn get_certificate(
        &self,
        _req: &RouterDataV2<Flow, ResourceCommonData, Req, Resp>,
    ) -> CustomResult<Option<hyperswitch_masking::Secret<String>>, IntegrationError> {
        Ok(None)
    }

    /// returns private key string
    fn get_certificate_key(
        &self,
        _req: &RouterDataV2<Flow, ResourceCommonData, Req, Resp>,
    ) -> CustomResult<Option<hyperswitch_masking::Secret<String>>, IntegrationError> {
        Ok(None)
    }
}
