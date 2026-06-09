use connector_integration::types::ConnectorData;
use domain_types::utils::ForeignTryFrom as _;
use grpc_api_types::payments::{
    composite_payment_method_service_server::CompositePaymentMethodService,
    merchant_authentication_service_server::MerchantAuthenticationService,
    payment_method_service_server::PaymentMethodService, CompositePaymentMethodCreateRequest,
    CompositePaymentMethodCreateResponse, CompositePaymentMethodGetRequest,
    CompositePaymentMethodGetResponse, CompositePaymentMethodRechargeRequest,
    CompositePaymentMethodRechargeResponse,
    MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse,
    PaymentMethodServiceCreateRequest, PaymentMethodServiceGetRequest,
    PaymentMethodServiceRechargeRequest,
};

use crate::payments::CompositeAccessTokenRequest;
use crate::transformers::ForeignFrom;
use crate::utils::connector_from_composite_authorize_metadata;

/// Implementation of CompositeAccessTokenRequest for payment method requests.
/// These requests don't have a specific payment_method field since payment-method-management
/// flows aren't gated on a specific payment method.
impl CompositeAccessTokenRequest for CompositePaymentMethodRechargeRequest {
    fn payment_method(&self) -> Option<grpc_api_types::payments::PaymentMethod> {
        None
    }

    fn state(&self) -> Option<&grpc_api_types::payments::ConnectorState> {
        self.state.as_ref()
    }

    fn build_access_token_request(
        &self,
        connector: &domain_types::connector_types::ConnectorEnum,
    ) -> grpc_api_types::payments::MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest
    {
        grpc_api_types::payments::MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest::foreign_from((
            self, connector,
        ))
    }
}

impl CompositeAccessTokenRequest for CompositePaymentMethodCreateRequest {
    fn payment_method(&self) -> Option<grpc_api_types::payments::PaymentMethod> {
        None
    }

    fn state(&self) -> Option<&grpc_api_types::payments::ConnectorState> {
        self.state.as_ref()
    }

    fn build_access_token_request(
        &self,
        connector: &domain_types::connector_types::ConnectorEnum,
    ) -> grpc_api_types::payments::MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest
    {
        grpc_api_types::payments::MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest::foreign_from((
            self, connector,
        ))
    }
}

impl CompositeAccessTokenRequest for CompositePaymentMethodGetRequest {
    fn payment_method(&self) -> Option<grpc_api_types::payments::PaymentMethod> {
        None
    }

    fn state(&self) -> Option<&grpc_api_types::payments::ConnectorState> {
        self.state.as_ref()
    }

    fn build_access_token_request(
        &self,
        connector: &domain_types::connector_types::ConnectorEnum,
    ) -> grpc_api_types::payments::MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest
    {
        grpc_api_types::payments::MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest::foreign_from((
            self, connector,
        ))
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
    async fn create_server_authentication_token<R: CompositeAccessTokenRequest>(
        &self,
        connector: &domain_types::connector_types::ConnectorEnum,
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
                .map_err(|err| {
                    tonic::Status::invalid_argument(format!(
                        "invalid payment_method in request payload: {err}"
                    ))
                })?;
            let connector_data = ConnectorData::<
                domain_types::payment_method_data::DefaultPCIHolder,
            >::get_connector_by_name(connector);
            connector_data
                .connector
                .should_do_access_token(payment_method)
        };
        let payload_access_token = payload
            .state()
            .and_then(|state| state.access_token.as_ref())
            .and_then(|token| {
                domain_types::connector_types::ServerAuthenticationTokenResponseData::foreign_try_from(
                    token,
                )
                .ok()
            });
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

    async fn process_recharge(
        &self,
        request: tonic::Request<CompositePaymentMethodRechargeRequest>,
    ) -> Result<tonic::Response<CompositePaymentMethodRechargeResponse>, tonic::Status> {
        let (metadata, extensions, payload) = request.into_parts();
        let connector =
            connector_from_composite_authorize_metadata(&metadata).map_err(|err| *err)?;
        let access_token_response = self
            .create_server_authentication_token(&connector, &payload, &metadata, &extensions)
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
            .create_server_authentication_token(&connector, &payload, &metadata, &extensions)
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
            .create_server_authentication_token(&connector, &payload, &metadata, &extensions)
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
