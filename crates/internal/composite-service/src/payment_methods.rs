use connector_integration::types::ConnectorData;
use grpc_api_types::payments::{
    composite_payment_method_service_server::CompositePaymentMethodService,
    merchant_authentication_service_server::MerchantAuthenticationService,
    payment_method_service_server::PaymentMethodService,
    CompositePaymentMethodCreateRequest, CompositePaymentMethodCreateResponse,
    CompositePaymentMethodGetRequest, CompositePaymentMethodGetResponse,
    CompositePaymentMethodRechargeRequest, CompositePaymentMethodRechargeResponse,
    MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest,
    MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse,
    PaymentMethodServiceCreateRequest, PaymentMethodServiceGetRequest,
    PaymentMethodServiceRechargeRequest,
};
use hyperswitch_masking::Secret;

use crate::transformers::ForeignFrom;
use crate::utils::{connector_from_composite_authorize_metadata, grpc_connector_from_connector_enum};

/// Common shape every composite-PaymentMethod request exposes for the
/// access-token bootstrap step. Keeps `bootstrap_access_token` generic over
/// Create / Get / Recharge.
trait CompositePaymentMethodRequest {
    fn already_has_access_token(&self) -> bool;
    fn merchant_access_token_id(&self) -> Option<String>;
    fn metadata(&self) -> Option<Secret<String>>;
    fn connector_feature_data(&self) -> Option<Secret<String>>;
    fn test_mode(&self) -> Option<bool>;
}

impl CompositePaymentMethodRequest for CompositePaymentMethodRechargeRequest {
    fn already_has_access_token(&self) -> bool {
        self.state
            .as_ref()
            .and_then(|s| s.access_token.as_ref())
            .is_some()
    }
    fn merchant_access_token_id(&self) -> Option<String> {
        self.merchant_access_token_id.clone()
    }
    fn metadata(&self) -> Option<Secret<String>> {
        self.metadata.clone()
    }
    fn connector_feature_data(&self) -> Option<Secret<String>> {
        self.connector_feature_data.clone()
    }
    fn test_mode(&self) -> Option<bool> {
        self.test_mode
    }
}

impl CompositePaymentMethodRequest for CompositePaymentMethodCreateRequest {
    fn already_has_access_token(&self) -> bool {
        self.state
            .as_ref()
            .and_then(|s| s.access_token.as_ref())
            .is_some()
    }
    fn merchant_access_token_id(&self) -> Option<String> {
        self.merchant_access_token_id.clone()
    }
    fn metadata(&self) -> Option<Secret<String>> {
        self.metadata.clone()
    }
    fn connector_feature_data(&self) -> Option<Secret<String>> {
        self.connector_feature_data.clone()
    }
    fn test_mode(&self) -> Option<bool> {
        self.test_mode
    }
}

impl CompositePaymentMethodRequest for CompositePaymentMethodGetRequest {
    fn already_has_access_token(&self) -> bool {
        self.state
            .as_ref()
            .and_then(|s| s.access_token.as_ref())
            .is_some()
    }
    fn merchant_access_token_id(&self) -> Option<String> {
        self.merchant_access_token_id.clone()
    }
    fn metadata(&self) -> Option<Secret<String>> {
        self.metadata.clone()
    }
    fn connector_feature_data(&self) -> Option<Secret<String>> {
        self.connector_feature_data.clone()
    }
    fn test_mode(&self) -> Option<bool> {
        self.test_mode
    }
}

/// Composite Payment Method Service that combines payment-method operations
/// with the access-token bootstrap.
///
/// Each composite RPC (Create, Get, Recharge) auto-bootstraps the connector's
/// session token (only when the connector requires one AND the caller didn't
/// already supply one), splices the minted token into the inner request, and
/// forwards to the underlying `PaymentMethodService`.
#[derive(Clone)]
pub struct PaymentMethods<PM, MA>
where
    PM: PaymentMethodService + Clone + Send + Sync + 'static,
    MA: MerchantAuthenticationService + Clone + Send + Sync + 'static,
{
    payment_method_service: PM,
    merchant_authentication_service: MA,
}

impl<PM, MA> PaymentMethods<PM, MA>
where
    PM: PaymentMethodService + Clone + Send + Sync + 'static,
    MA: MerchantAuthenticationService + Clone + Send + Sync + 'static,
{
    pub fn new(payment_method_service: PM, merchant_authentication_service: MA) -> Self {
        Self {
            payment_method_service,
            merchant_authentication_service,
        }
    }

    /// Bootstrap the connector's session token if (a) the connector requires
    /// one and (b) the caller didn't already pass one via
    /// `state.access_token`. Returns `None` otherwise so the response's
    /// `access_token_response` slot stays unset.
    async fn bootstrap_access_token<R: CompositePaymentMethodRequest>(
        &self,
        connector: &domain_types::connector_types::ConnectorEnum,
        payload: &R,
        metadata: &tonic::metadata::MetadataMap,
        extensions: &tonic::Extensions,
    ) -> Result<
        Option<MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
        tonic::Status,
    > {
        if payload.already_has_access_token() {
            return Ok(None);
        }

        // Payment-method-management flows aren't gated on a specific
        // payment_method — bootstrap unconditionally when the connector asks
        // for one.
        let connector_data = ConnectorData::<
            domain_types::payment_method_data::DefaultPCIHolder,
        >::get_connector_by_name(connector);
        if !connector_data.connector.should_do_access_token(None) {
            return Ok(None);
        }

        let token_payload = MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest {
            merchant_access_token_id: payload.merchant_access_token_id(),
            connector: grpc_connector_from_connector_enum(connector),
            metadata: payload.metadata(),
            connector_feature_data: payload.connector_feature_data(),
            test_mode: payload.test_mode(),
        };
        let mut token_request = tonic::Request::new(token_payload);
        *token_request.metadata_mut() = metadata.clone();
        *token_request.extensions_mut() = extensions.clone();

        let token_response = self
            .merchant_authentication_service
            .create_server_authentication_token(token_request)
            .await?
            .into_inner();

        Ok(Some(token_response))
    }

    async fn process_recharge(
        &self,
        request: tonic::Request<CompositePaymentMethodRechargeRequest>,
    ) -> Result<tonic::Response<CompositePaymentMethodRechargeResponse>, tonic::Status> {
        let (metadata, extensions, payload) = request.into_parts();
        let connector =
            connector_from_composite_authorize_metadata(&metadata).map_err(|err| *err)?;
        let access_token_response = self
            .bootstrap_access_token(&connector, &payload, &metadata, &extensions)
            .await?;

        let inner = PaymentMethodServiceRechargeRequest::foreign_from((
            &payload,
            access_token_response.as_ref(),
        ));
        let mut inner_request = tonic::Request::new(inner);
        *inner_request.metadata_mut() = metadata;
        *inner_request.extensions_mut() = extensions;

        let recharge_response = self
            .payment_method_service
            .recharge(inner_request)
            .await?
            .into_inner();

        Ok(tonic::Response::new(
            CompositePaymentMethodRechargeResponse {
                access_token_response,
                recharge_response: Some(recharge_response),
            },
        ))
    }

    async fn process_create(
        &self,
        request: tonic::Request<CompositePaymentMethodCreateRequest>,
    ) -> Result<tonic::Response<CompositePaymentMethodCreateResponse>, tonic::Status> {
        let (metadata, extensions, payload) = request.into_parts();
        let connector =
            connector_from_composite_authorize_metadata(&metadata).map_err(|err| *err)?;
        let access_token_response = self
            .bootstrap_access_token(&connector, &payload, &metadata, &extensions)
            .await?;

        let inner = PaymentMethodServiceCreateRequest::foreign_from((
            &payload,
            access_token_response.as_ref(),
        ));
        let mut inner_request = tonic::Request::new(inner);
        *inner_request.metadata_mut() = metadata;
        *inner_request.extensions_mut() = extensions;

        let create_response = self
            .payment_method_service
            .create(inner_request)
            .await?
            .into_inner();

        Ok(tonic::Response::new(CompositePaymentMethodCreateResponse {
            access_token_response,
            create_response: Some(create_response),
        }))
    }

    async fn process_get(
        &self,
        request: tonic::Request<CompositePaymentMethodGetRequest>,
    ) -> Result<tonic::Response<CompositePaymentMethodGetResponse>, tonic::Status> {
        let (metadata, extensions, payload) = request.into_parts();
        let connector =
            connector_from_composite_authorize_metadata(&metadata).map_err(|err| *err)?;
        let access_token_response = self
            .bootstrap_access_token(&connector, &payload, &metadata, &extensions)
            .await?;

        let inner = PaymentMethodServiceGetRequest::foreign_from((
            &payload,
            access_token_response.as_ref(),
        ));
        let mut inner_request = tonic::Request::new(inner);
        *inner_request.metadata_mut() = metadata;
        *inner_request.extensions_mut() = extensions;

        let get_response = self
            .payment_method_service
            .get(inner_request)
            .await?
            .into_inner();

        Ok(tonic::Response::new(CompositePaymentMethodGetResponse {
            access_token_response,
            get_response: Some(get_response),
        }))
    }
}

#[tonic::async_trait]
impl<PM, MA> CompositePaymentMethodService for PaymentMethods<PM, MA>
where
    PM: PaymentMethodService + Clone + Send + Sync + 'static,
    MA: MerchantAuthenticationService + Clone + Send + Sync + 'static,
{
    /// Create a payment method (e.g. provision a wallet). Bootstraps the
    /// connector session token when needed, then forwards to the underlying
    /// `PaymentMethodService.Create`.
    async fn create(
        &self,
        request: tonic::Request<CompositePaymentMethodCreateRequest>,
    ) -> Result<tonic::Response<CompositePaymentMethodCreateResponse>, tonic::Status> {
        self.process_create(request).await
    }

    /// Look up a payment method (e.g. fetch wallet details). Same bootstrap +
    /// forward pattern.
    async fn get(
        &self,
        request: tonic::Request<CompositePaymentMethodGetRequest>,
    ) -> Result<tonic::Response<CompositePaymentMethodGetResponse>, tonic::Status> {
        self.process_get(request).await
    }

    /// Recharge a payment method (e.g. credit value to a wallet).
    async fn recharge(
        &self,
        request: tonic::Request<CompositePaymentMethodRechargeRequest>,
    ) -> Result<tonic::Response<CompositePaymentMethodRechargeResponse>, tonic::Status> {
        self.process_recharge(request).await
    }
}
