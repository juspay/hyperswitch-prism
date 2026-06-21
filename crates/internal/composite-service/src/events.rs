use connector_integration::types::ConnectorData;
use domain_types::{
    connector_types::{ConnectorEnum, ServerAuthenticationTokenResponseData},
    payment_method_data::DefaultPCIHolder,
    utils::ForeignTryFrom as _,
};
use grpc_api_types::payments::{
    composite_event_service_server::CompositeEventService, event_service_server::EventService,
    merchant_authentication_service_server::MerchantAuthenticationService,
    CompositeEventHandleRequest, CompositeEventHandleResponse, CompositeNotifyRequest,
    CompositeNotifyResponse, ConnectorState, EventServiceHandleRequest, EventServiceParseRequest,
    MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest,
    MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse, NotifyConnectorRequest,
};
use ucs_env::error::ResultExtGrpc;

use crate::payments::CompositeAccessTokenRequest;
use crate::transformers::ForeignFrom;
use crate::utils::connector_from_composite_authorize_metadata;

/// Composite implementation of [`CompositeEventService`].
///
/// Orchestrates the two-phase webhook flow by calling the granular [`EventService`] RPCs.
/// 1. `ParseEvent`  — stateless reference + event-type extraction.
/// 2. `HandleEvent` — source verification + unified event content.
///
/// Also provides a composite Notify endpoint for FRM notifications with access token bootstrapping.
///
/// Metadata and extensions are forwarded to each sub-call so that connector routing,
/// config injection, and tracing all work transparently through the granular handlers.
#[derive(Debug, Clone)]
pub struct CompositeEvents<E, MA>
where
    E: EventService + Clone + Send + Sync + 'static,
    MA: MerchantAuthenticationService + Clone + Send + Sync + 'static,
{
    event_service: E,
    merchant_authentication_service: MA,
}

impl CompositeAccessTokenRequest for CompositeNotifyRequest {
    fn payment_method(&self) -> Option<grpc_api_types::payments::PaymentMethod> {
        None
    }

    fn state(&self) -> Option<&ConnectorState> {
        self.state.as_ref()
    }

    fn build_access_token_request(
        &self,
        connector: &ConnectorEnum,
    ) -> MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest {
        MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest::foreign_from((
            self, connector,
        ))
    }
}

impl<E, MA> CompositeEvents<E, MA>
where
    E: EventService + Clone + Send + Sync + 'static,
    MA: MerchantAuthenticationService + Clone + Send + Sync + 'static,
{
    pub fn new(event_service: E, merchant_authentication_service: MA) -> Self {
        Self {
            event_service,
            merchant_authentication_service,
        }
    }

    async fn create_server_authentication_token<R: CompositeAccessTokenRequest>(
        &self,
        connector: &ConnectorEnum,
        payload: &R,
        metadata: &tonic::metadata::MetadataMap,
        extensions: &tonic::Extensions,
    ) -> Result<
        Option<MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
        tonic::Status,
    > {
        let should_do_access_token = {
            let payment_method = payload
                .payment_method()
                .map(common_enums::PaymentMethod::foreign_try_from)
                .transpose()
                .into_grpc_status()?;
            let connector_data =
                ConnectorData::<DefaultPCIHolder>::get_connector_by_name(connector);
            connector_data
                .connector
                .should_do_access_token(payment_method)
        };

        let payload_access_token = payload
            .state()
            .and_then(|state| state.access_token.as_ref())
            .and_then(|token| ServerAuthenticationTokenResponseData::foreign_try_from(token).ok());
        let should_create_access_token = should_do_access_token && payload_access_token.is_none();

        let access_token_response = match should_create_access_token {
            true => {
                let access_token_payload = payload.build_access_token_request(connector);
                let mut access_token_request = tonic::Request::new(access_token_payload);
                *access_token_request.metadata_mut() = metadata.clone();
                *access_token_request.extensions_mut() = extensions.clone();

                let access_token_response = self
                    .merchant_authentication_service
                    .create_server_authentication_token(access_token_request)
                    .await?
                    .into_inner();

                Some(access_token_response)
            }
            false => None,
        };

        Ok(access_token_response)
    }
}

#[tonic::async_trait]
impl<E, MA> CompositeEventService for CompositeEvents<E, MA>
where
    E: EventService + Clone + Send + Sync + 'static,
    MA: MerchantAuthenticationService + Clone + Send + Sync + 'static,
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

    /// FRM Notify endpoint with access token bootstrapping.
    /// Delegates to EventService.NotifyConnector for actual event handling.
    async fn notify(
        &self,
        request: tonic::Request<CompositeNotifyRequest>,
    ) -> Result<tonic::Response<CompositeNotifyResponse>, tonic::Status> {
        let (metadata, extensions, payload) = request.into_parts();

        let connector =
            connector_from_composite_authorize_metadata(&metadata).map_err(|err| *err)?;

        let access_token_response = self
            .create_server_authentication_token(&connector, &payload, &metadata, &extensions)
            .await?;

        // Build the underlying NotifyConnectorRequest using ForeignFrom
        let inner =
            NotifyConnectorRequest::foreign_from((&payload, access_token_response.as_ref()));
        let mut inner_request = tonic::Request::new(inner);
        *inner_request.metadata_mut() = metadata;
        *inner_request.extensions_mut() = extensions;

        let notify_response = self
            .event_service
            .notify_connector(inner_request)
            .await?
            .into_inner();

        Ok(tonic::Response::new(CompositeNotifyResponse {
            notify_response: Some(notify_response),
            access_token_response,
        }))
    }
}
