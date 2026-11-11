use domain_types::{
    connector_types::EventContext, errors::WebhookError,
    payment_method_data::PaymentMethodDataTypes, router_data::ConnectorSpecificConfig,
    utils::ForeignTryFrom,
};
use error_stack::ResultExt;
use grpc_api_types::payments::{
    DisputeResponse, EventContent, EventServiceHandleResponse, EventServiceParseResponse,
    PaymentServiceGetResponse, PayoutServiceGetResponse, RefundResponse, WebhookEventType,
};

use crate::types::{
    ConnectorData, PayoutConnectorData, WebhookConnectorData, WebhookResourceConnectorData,
};

/// Inputs shared by webhook normalization and ACK generation.
pub struct WebhookHandlingContext {
    pub webhook_secrets: Option<domain_types::connector_types::ConnectorWebhookSecrets>,
    pub connector_config: Option<ConnectorSpecificConfig>,
    pub source_verified: bool,
    pub merchant_event_id: Option<String>,
    pub event_context: Option<EventContext>,
}

/// Core webhook event processing shared by both the gRPC server and the FFI layer.
///
/// Caller is responsible for:
/// 1. Parsing `request_details` and `webhook_secrets` from the raw proto payload.
/// 2. Computing `source_verified` (the gRPC server may use an async external call;
///    the FFI layer uses only synchronous local verification).
///
/// This function then: determines the event type, dispatches to the appropriate
/// content helper, and assembles the final `EventServiceHandleResponse`.
pub fn process_webhook_event<
    T: PaymentMethodDataTypes
        + Default
        + Eq
        + std::fmt::Debug
        + Send
        + serde::Serialize
        + serde::de::DeserializeOwned
        + Clone
        + Sync
        + 'static,
>(
    connector_data: ConnectorData<T>,
    request_details: domain_types::connector_types::RequestDetails,
    webhook_secrets: Option<domain_types::connector_types::ConnectorWebhookSecrets>,
    connector_config: Option<ConnectorSpecificConfig>,
    source_verified: bool,
    merchant_event_id: Option<String>,
    event_context: Option<EventContext>,
) -> error_stack::Result<EventServiceHandleResponse, WebhookError> {
    let event_type = connector_data
        .connector
        .get_event_type(request_details.clone())?;

    let api_event_type = WebhookEventType::foreign_try_from(event_type.clone())
        .change_context(WebhookError::WebhookProcessingFailed)?;

    let event_content = if event_type.is_payment_event() {
        get_payments_webhook_content(
            connector_data.clone(),
            request_details.clone(),
            webhook_secrets,
            connector_config.clone(),
            event_context.clone(),
        )?
    } else if event_type.is_refund_event() {
        get_refunds_webhook_content(
            connector_data.clone(),
            request_details.clone(),
            webhook_secrets,
            connector_config.clone(),
        )?
    } else if event_type.is_dispute_event() {
        get_disputes_webhook_content(
            connector_data.clone(),
            request_details.clone(),
            webhook_secrets,
            connector_config.clone(),
        )?
    } else if event_type.is_payout_event() {
        get_payouts_webhook_content(
            connector_data.clone(),
            request_details.clone(),
            webhook_secrets,
            connector_config.clone(),
        )?
    } else {
        // Default: treat as payment event (mandate, recovery, misc).
        get_payments_webhook_content(
            connector_data.clone(),
            request_details.clone(),
            webhook_secrets,
            connector_config.clone(),
            event_context,
        )?
    };

    let ack = connector_data.connector.get_webhook_api_response(
        request_details.clone(),
        None,
        connector_config,
    )?;
    let event_ack_response = Some(grpc_api_types::payments::EventAckResponse {
        status_code: ack.status_code,
        headers: ack.headers.into_iter().collect(),
        body: ack.body.unwrap_or_default(),
    });

    Ok(EventServiceHandleResponse {
        event_type: api_event_type.into(),
        event_content: Some(event_content),
        source_verified,
        merchant_event_id,
        event_ack_response,
        supported_integrity_checks: vec![],
    })
}

/// Handle a provider-classified event using only its selected resource handler.
/// The caller verifies the source before resource processing.
pub fn process_provider_webhook_event(
    webhook: &dyn interfaces::webhook::Webhook,
    resource: WebhookResourceConnectorData<domain_types::payment_method_data::DefaultPCIHolder>,
    event_type: domain_types::connector_types::EventType,
    request_details: domain_types::connector_types::RequestDetails,
    context: WebhookHandlingContext,
) -> error_stack::Result<EventServiceHandleResponse, WebhookError> {
    let api_event_type = WebhookEventType::foreign_try_from(event_type.clone())
        .change_context(WebhookError::WebhookProcessingFailed)?;
    let event_content = match resource {
        WebhookResourceConnectorData::Payout(payout) => get_payouts_webhook_content_from_payout(
            payout,
            request_details.clone(),
            context.webhook_secrets,
            context.connector_config.clone(),
        )?,
        WebhookResourceConnectorData::Payment(payment) => {
            if event_type.is_refund_event() {
                get_refunds_webhook_content(
                    payment,
                    request_details.clone(),
                    context.webhook_secrets,
                    context.connector_config.clone(),
                )?
            } else if event_type.is_dispute_event() {
                get_disputes_webhook_content(
                    payment,
                    request_details.clone(),
                    context.webhook_secrets,
                    context.connector_config.clone(),
                )?
            } else {
                get_payments_webhook_content(
                    payment,
                    request_details.clone(),
                    context.webhook_secrets,
                    context.connector_config.clone(),
                    context.event_context,
                )?
            }
        }
    };
    let ack = webhook.get_webhook_api_response(request_details, None, context.connector_config)?;
    Ok(EventServiceHandleResponse {
        event_type: api_event_type.into(),
        event_content: Some(event_content),
        source_verified: context.source_verified,
        merchant_event_id: context.merchant_event_id,
        event_ack_response: Some(grpc_api_types::payments::EventAckResponse {
            status_code: ack.status_code,
            headers: ack.headers.into_iter().collect(),
            body: ack.body.unwrap_or_default(),
        }),
        supported_integrity_checks: vec![],
    })
}

/// Parses through the selected standalone webhook provider.
pub fn parse_webhook_event(
    connector_data: WebhookConnectorData,
    request_details: domain_types::connector_types::RequestDetails,
) -> error_stack::Result<EventServiceParseResponse, WebhookError> {
    let parsed = connector_data.connector.parse_event(&request_details)?;
    let api_event_type = WebhookEventType::foreign_try_from(parsed.event_type)
        .change_context(WebhookError::WebhookProcessingFailed)?;

    Ok(EventServiceParseResponse {
        reference: parsed.reference.map(Into::into),
        event_type: Some(api_event_type.into()),
    })
}

/// Preserves the legacy payment connector's event classification and reference extraction.
pub fn parse_payment_webhook_event(
    connector_data: ConnectorData<domain_types::payment_method_data::DefaultPCIHolder>,
    request_details: domain_types::connector_types::RequestDetails,
) -> error_stack::Result<EventServiceParseResponse, WebhookError> {
    let event_type = connector_data
        .connector
        .get_event_type(request_details.clone())?;
    let api_event_type = WebhookEventType::foreign_try_from(event_type)
        .change_context(WebhookError::WebhookProcessingFailed)?;
    let reference = connector_data
        .connector
        .get_webhook_event_reference(request_details)
        .attach_printable("Failed to extract event reference from webhook payload")?;

    Ok(EventServiceParseResponse {
        reference: reference.map(Into::into),
        event_type: Some(api_event_type.into()),
    })
}

pub fn get_payments_webhook_content<
    T: PaymentMethodDataTypes + std::fmt::Debug + Default + Send + Sync + 'static,
>(
    connector_data: ConnectorData<T>,
    request_details: domain_types::connector_types::RequestDetails,
    webhook_secrets: Option<domain_types::connector_types::ConnectorWebhookSecrets>,
    connector_config: Option<ConnectorSpecificConfig>,
    event_context: Option<EventContext>,
) -> error_stack::Result<EventContent, WebhookError> {
    let webhook_details = connector_data
        .connector
        .process_payment_webhook(
            request_details,
            webhook_secrets,
            connector_config,
            event_context,
        )
        .attach_printable("Failed to process payment webhook from connector")?;

    let response = PaymentServiceGetResponse::foreign_try_from(webhook_details)
        .change_context(WebhookError::WebhookProcessingFailed)?;

    Ok(EventContent {
        content: Some(grpc_api_types::payments::event_content::Content::PaymentsResponse(response)),
    })
}

pub fn get_refunds_webhook_content<
    T: PaymentMethodDataTypes
        + Default
        + Eq
        + std::fmt::Debug
        + Send
        + serde::Serialize
        + serde::de::DeserializeOwned
        + Clone
        + Sync
        + 'static,
>(
    connector_data: ConnectorData<T>,
    request_details: domain_types::connector_types::RequestDetails,
    webhook_secrets: Option<domain_types::connector_types::ConnectorWebhookSecrets>,
    connector_config: Option<ConnectorSpecificConfig>,
) -> error_stack::Result<EventContent, WebhookError> {
    let webhook_details = connector_data
        .connector
        .process_refund_webhook(request_details, webhook_secrets, connector_config)
        .attach_printable("Failed to process refund webhook from connector")?;

    let response = RefundResponse::foreign_try_from(webhook_details)
        .change_context(WebhookError::WebhookProcessingFailed)?;

    Ok(EventContent {
        content: Some(grpc_api_types::payments::event_content::Content::RefundsResponse(response)),
    })
}

pub fn get_payouts_webhook_content<
    T: PaymentMethodDataTypes
        + Default
        + Eq
        + std::fmt::Debug
        + Send
        + serde::Serialize
        + serde::de::DeserializeOwned
        + Clone
        + Sync
        + 'static,
>(
    connector_data: ConnectorData<T>,
    request_details: domain_types::connector_types::RequestDetails,
    webhook_secrets: Option<domain_types::connector_types::ConnectorWebhookSecrets>,
    connector_config: Option<ConnectorSpecificConfig>,
) -> error_stack::Result<EventContent, WebhookError> {
    let webhook_details = connector_data
        .connector
        .process_payout_webhook(request_details, webhook_secrets, connector_config)
        .attach_printable("Failed to process payout webhook from connector")?;

    let response = PayoutServiceGetResponse::foreign_try_from(webhook_details)
        .change_context(WebhookError::WebhookProcessingFailed)?;

    Ok(EventContent {
        content: Some(grpc_api_types::payments::event_content::Content::PayoutsResponse(response)),
    })
}

/// Normalize using the registered payout connector; errors never retry the payment path.
pub fn get_payouts_webhook_content_from_payout<T>(
    connector_data: PayoutConnectorData<T>,
    request_details: domain_types::connector_types::RequestDetails,
    webhook_secrets: Option<domain_types::connector_types::ConnectorWebhookSecrets>,
    connector_config: Option<ConnectorSpecificConfig>,
) -> error_stack::Result<EventContent, WebhookError>
where
    T: PaymentMethodDataTypes + Default + std::fmt::Debug + Send + Sync + 'static,
{
    let webhook_details = connector_data
        .connector
        .process_payout_webhook(request_details, webhook_secrets, connector_config)
        .attach_printable("Failed to process payout webhook from connector")?;
    let response = PayoutServiceGetResponse::foreign_try_from(webhook_details)
        .change_context(WebhookError::WebhookProcessingFailed)?;
    Ok(EventContent {
        content: Some(grpc_api_types::payments::event_content::Content::PayoutsResponse(response)),
    })
}

pub fn get_disputes_webhook_content<
    T: PaymentMethodDataTypes
        + Default
        + Eq
        + std::fmt::Debug
        + Send
        + serde::Serialize
        + serde::de::DeserializeOwned
        + Clone
        + Sync
        + 'static,
>(
    connector_data: ConnectorData<T>,
    request_details: domain_types::connector_types::RequestDetails,
    webhook_secrets: Option<domain_types::connector_types::ConnectorWebhookSecrets>,
    connector_config: Option<ConnectorSpecificConfig>,
) -> error_stack::Result<EventContent, WebhookError> {
    let webhook_details = connector_data
        .connector
        .process_dispute_webhook(request_details, webhook_secrets, connector_config)
        .attach_printable("Failed to process dispute webhook from connector")?;

    let response = DisputeResponse::foreign_try_from(webhook_details)
        .change_context(WebhookError::WebhookProcessingFailed)?;

    Ok(EventContent {
        content: Some(grpc_api_types::payments::event_content::Content::DisputesResponse(response)),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod provider_tests {
    use super::*;
    use domain_types::connector_types::{
        ConnectorEnum, ConnectorVariant, EventType, HttpMethod, PayoutConnectorEnum,
        RequestDetails, WebhookParseResponse,
    };
    use domain_types::payment_method_data::DefaultPCIHolder;
    use interfaces::{api::EventAckResponse, webhook::Webhook};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestWebhook {
        event: EventType,
        classifications: AtomicUsize,
        acknowledgements: AtomicUsize,
        ack_fails: bool,
    }
    impl TestWebhook {
        fn new(event: EventType) -> Self {
            Self {
                event,
                classifications: AtomicUsize::new(0),
                acknowledgements: AtomicUsize::new(0),
                ack_fails: false,
            }
        }
    }
    impl Webhook for TestWebhook {
        fn parse_event(
            &self,
            _request: &RequestDetails,
        ) -> error_stack::Result<WebhookParseResponse, WebhookError> {
            // HandleEvent classification must not require reference extraction.
            Err(WebhookError::WebhookReferenceIdNotFound.into())
        }
        fn get_event_type(
            &self,
            _request: RequestDetails,
        ) -> error_stack::Result<EventType, WebhookError> {
            self.classifications.fetch_add(1, Ordering::SeqCst);
            Ok(self.event.clone())
        }
        fn get_webhook_api_response(
            &self,
            _request: RequestDetails,
            _error: Option<interfaces::connector_types::IncomingWebhookFlowError>,
            _config: Option<ConnectorSpecificConfig>,
        ) -> error_stack::Result<EventAckResponse, WebhookError> {
            self.acknowledgements.fetch_add(1, Ordering::SeqCst);
            if self.ack_fails {
                return Err(WebhookError::WebhookResponseEncodingFailed.into());
            }
            Ok(EventAckResponse {
                status_code: 202,
                headers: vec![("x-provider-ack".to_string(), "yes".to_string())],
                body: Some(b"accepted".to_vec()),
            })
        }
    }
    fn request(event: &str) -> RequestDetails {
        RequestDetails {
            method: HttpMethod::Post, uri: None, headers: Default::default(), query_params: None,
            body: serde_json::to_vec(&serde_json::json!({"type":event,"data":{"id":"resource_id","payment_id":"pay_parent","action_id":"refund_id","amount":100,"currency":"USD","reference":"merchant_id"}})).unwrap(),
        }
    }
    fn context(verified: bool) -> WebhookHandlingContext {
        WebhookHandlingContext {
            webhook_secrets: None,
            connector_config: None,
            source_verified: verified,
            merchant_event_id: Some("merchant_event".into()),
            event_context: None,
        }
    }
    fn run(
        provider: &TestWebhook,
        body: RequestDetails,
        verified: bool,
    ) -> error_stack::Result<EventServiceHandleResponse, WebhookError> {
        let event = provider.get_event_type(body.clone())?;
        let resource = crate::types::get_webhook_resource_connector_data(
            &ConnectorVariant::Payment(ConnectorEnum::Checkout),
            &event,
        )?;
        process_provider_webhook_event(provider, resource, event, body, context(verified))
    }
    #[test]
    fn provider_dispatch_classifies_once_and_preserves_unverified_content_and_ack() {
        let provider = TestWebhook::new(EventType::PaymentIntentSuccess);
        let response = run(&provider, request("payment_captured"), false).unwrap();
        assert!(!response.source_verified);
        assert_eq!(
            response.merchant_event_id.as_deref(),
            Some("merchant_event")
        );
        assert!(matches!(
            response.event_content.unwrap().content,
            Some(grpc_api_types::payments::event_content::Content::PaymentsResponse(_))
        ));
        let ack = response.event_ack_response.unwrap();
        assert_eq!(ack.status_code, 202);
        assert_eq!(ack.body, b"accepted");
        assert_eq!(
            ack.headers.get("x-provider-ack").map(String::as_str),
            Some("yes")
        );
        assert_eq!(provider.classifications.load(Ordering::SeqCst), 1);
        assert_eq!(provider.acknowledgements.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn provider_dispatch_routes_refunds_and_disputes_to_their_resource_methods() {
        for (event, wire, refund) in [
            (EventType::RefundSuccess, "payment_refunded", true),
            (EventType::DisputeOpened, "dispute_received", false),
        ] {
            let provider = TestWebhook::new(event);
            let response = run(&provider, request(wire), true).unwrap();
            assert!(response.source_verified);
            let content = response.event_content.unwrap().content.unwrap();
            if refund {
                assert!(matches!(
                    content,
                    grpc_api_types::payments::event_content::Content::RefundsResponse(_)
                ));
            } else {
                assert!(matches!(
                    content,
                    grpc_api_types::payments::event_content::Content::DisputesResponse(_)
                ));
            }
        }
    }
    #[test]
    fn normalization_error_stops_before_ack() {
        let provider = TestWebhook::new(EventType::PaymentIntentSuccess);
        let mut body = request("payment_captured");
        body.body = b"invalid".to_vec();
        assert!(run(&provider, body, true).is_err());
        assert_eq!(provider.acknowledgements.load(Ordering::SeqCst), 0);
    }
    #[test]
    fn provider_ack_error_propagates() {
        let mut provider = TestWebhook::new(EventType::PaymentIntentSuccess);
        provider.ack_fails = true;
        let error = run(&provider, request("payment_captured"), true).unwrap_err();
        assert!(matches!(
            error.current_context(),
            WebhookError::WebhookResponseEncodingFailed
        ));
    }
    #[test]
    fn selected_payout_handler_error_never_retries_legacy_payment_handler() {
        let provider = TestWebhook::new(EventType::PayoutSuccess);
        let payout = PayoutConnectorData::<DefaultPCIHolder>::get_connector_by_name(
            &PayoutConnectorEnum::Truelayer,
        );
        let mut body = request("payout_executed");
        body.body = b"invalid".to_vec();
        let error = process_provider_webhook_event(
            &provider,
            WebhookResourceConnectorData::Payout(payout),
            EventType::PayoutSuccess,
            body,
            context(true),
        )
        .unwrap_err();
        // The unmigrated payout trait defaults to unsupported. Legacy payment handling
        // would deserialize this body and return a body-decoding error instead.
        assert!(matches!(
            error.current_context(),
            WebhookError::WebhooksNotImplemented {
                operation: "process_payout_webhook"
            }
        ));
        assert_eq!(provider.acknowledgements.load(Ordering::SeqCst), 0);
    }
    #[test]
    fn payout_resolution_requires_no_payment_identity_and_rejects_unregistered_handler() {
        let error = crate::types::get_webhook_resource_connector_data(
            &ConnectorVariant::Payout(PayoutConnectorEnum::GotymeSanlam),
            &EventType::PayoutSuccess,
        )
        .err()
        .unwrap();
        assert!(matches!(
            error.current_context(),
            WebhookError::WebhooksNotImplemented {
                operation: "process_payout_webhook"
            }
        ));
    }
    #[test]
    fn event_family_selects_resource_even_with_payout_metadata() {
        let selected = crate::types::get_webhook_resource_connector_data(
            &ConnectorVariant::Payout(PayoutConnectorEnum::Checkout),
            &EventType::RefundSuccess,
        )
        .unwrap();
        assert!(matches!(selected, WebhookResourceConnectorData::Payment(_)));
    }
}
