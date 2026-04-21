use grpc_api_types::payments::{
    composite_event_service_server::CompositeEventService, event_service_server::EventService,
    CompositeEventHandleRequest, CompositeEventHandleResponse, EventServiceHandleRequest,
    EventServiceParseRequest,
};

/// Composite implementation of [`CompositeEventService`].
///
/// Orchestrates the two-phase webhook flow by calling the granular [`EventService`] RPCs.
/// 1. `ParseEvent`  — stateless reference + event-type extraction.
/// 2. `HandleEvent` — source verification + unified event content.
///
/// Metadata and extensions are forwarded to each sub-call so that connector routing,
/// config injection, and tracing all work transparently through the granular handlers.
#[derive(Debug, Clone)]
pub struct CompositeEvents<E> {
    event_service: E,
}

impl<E> CompositeEvents<E> {
    pub fn new(event_service: E) -> Self {
        Self { event_service }
    }
}

#[tonic::async_trait]
impl<E> CompositeEventService for CompositeEvents<E>
where
    E: EventService + Clone + Send + Sync + 'static,
{
    async fn handle_event(
        &self,
        request: tonic::Request<CompositeEventHandleRequest>,
    ) -> Result<tonic::Response<CompositeEventHandleResponse>, tonic::Status> {
        let (metadata, extensions, payload) = request.into_parts();

        // Phase 1: ParseEvent — extract reference and event type from the raw payload.
        let mut parse_req = tonic::Request::new(EventServiceParseRequest {
            request_details: payload.request_details.clone(),
        });
        *parse_req.metadata_mut() = metadata.clone();
        *parse_req.extensions_mut() = extensions.clone();
        let parse_resp = self
            .event_service
            .parse_event(parse_req)
            .await?
            .into_inner();

        // Phase 2: HandleEvent — source verification + unified event content.
        let mut handle_req = tonic::Request::new(EventServiceHandleRequest {
            merchant_event_id: payload.merchant_event_id,
            request_details: payload.request_details,
            webhook_secrets: payload.webhook_secrets,
            access_token: payload.access_token,
            event_context: payload.event_context,
        });
        *handle_req.metadata_mut() = metadata;
        *handle_req.extensions_mut() = extensions;
        let handle_resp = self
            .event_service
            .handle_event(handle_req)
            .await?
            .into_inner();

        Ok(tonic::Response::new(CompositeEventHandleResponse {
            reference: parse_resp.reference,
            event_type: handle_resp.event_type,
            event_content: handle_resp.event_content,
            source_verified: handle_resp.source_verified,
            merchant_event_id: handle_resp.merchant_event_id,
            event_ack_response: handle_resp.event_ack_response,
        }))
    }
}
