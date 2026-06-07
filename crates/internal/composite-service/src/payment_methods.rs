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
    PaymentMethodServiceRechargeRequest,
};

use crate::transformers::ForeignFrom;
use crate::utils::{connector_from_composite_authorize_metadata, grpc_connector_from_connector_enum};

/// Composite Payment Method Service that combines payment-method operations
/// with the access-token bootstrap.
///
/// Recharge auto-bootstraps the connector's session token (when the connector
/// requires one and the caller didn't already provide one), then forwards
/// the recharge request to the underlying `PaymentMethodService` with the
/// minted token spliced into `state.access_token`.
///
/// Create/Get remain stubs — those flows have no core impl yet.
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
    async fn bootstrap_access_token(
        &self,
        connector: &domain_types::connector_types::ConnectorEnum,
        payload: &CompositePaymentMethodRechargeRequest,
        metadata: &tonic::metadata::MetadataMap,
        extensions: &tonic::Extensions,
    ) -> Result<
        Option<MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
        tonic::Status,
    > {
        // If the caller already supplied an access token, reuse it — no
        // bootstrap RPC needed.
        if payload
            .state
            .as_ref()
            .and_then(|s| s.access_token.as_ref())
            .is_some()
        {
            return Ok(None);
        }

        // Recharge isn't gated on a specific payment_method — bootstrap
        // unconditionally when the connector asks for it.
        let connector_data = ConnectorData::<
            domain_types::payment_method_data::DefaultPCIHolder,
        >::get_connector_by_name(connector);
        if !connector_data.connector.should_do_access_token(None) {
            return Ok(None);
        }

        let token_payload = MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest {
            merchant_access_token_id: payload.merchant_access_token_id.clone(),
            connector: grpc_connector_from_connector_enum(connector),
            metadata: payload.metadata.clone(),
            connector_feature_data: payload.connector_feature_data.clone(),
            test_mode: payload.test_mode,
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

        let recharge_payload = PaymentMethodServiceRechargeRequest::foreign_from((
            &payload,
            access_token_response.as_ref(),
        ));
        let mut recharge_request = tonic::Request::new(recharge_payload);
        *recharge_request.metadata_mut() = metadata;
        *recharge_request.extensions_mut() = extensions;

        let recharge_response = self
            .payment_method_service
            .recharge(recharge_request)
            .await?
            .into_inner();

        Ok(tonic::Response::new(
            CompositePaymentMethodRechargeResponse {
                access_token_response,
                recharge_response: Some(recharge_response),
            },
        ))
    }
}

#[tonic::async_trait]
impl<PM, MA> CompositePaymentMethodService for PaymentMethods<PM, MA>
where
    PM: PaymentMethodService + Clone + Send + Sync + 'static,
    MA: MerchantAuthenticationService + Clone + Send + Sync + 'static,
{
    /// Create payment method
    /// TODO: Returns empty response until core flows are implemented
    async fn create(
        &self,
        _request: tonic::Request<CompositePaymentMethodCreateRequest>,
    ) -> Result<tonic::Response<CompositePaymentMethodCreateResponse>, tonic::Status> {
        Ok(tonic::Response::new(CompositePaymentMethodCreateResponse {
            access_token_response: None,
            create_response: None,
        }))
    }

    /// Get payment method
    /// TODO: Returns empty response until core flows are implemented
    async fn get(
        &self,
        _request: tonic::Request<CompositePaymentMethodGetRequest>,
    ) -> Result<tonic::Response<CompositePaymentMethodGetResponse>, tonic::Status> {
        Ok(tonic::Response::new(CompositePaymentMethodGetResponse {
            access_token_response: None,
            get_response: None,
        }))
    }

    /// Recharge a payment method (e.g. credit value to a wallet). Bootstraps
    /// the connector session token when needed, then forwards to the
    /// underlying `PaymentMethodService.Recharge`.
    async fn recharge(
        &self,
        request: tonic::Request<CompositePaymentMethodRechargeRequest>,
    ) -> Result<tonic::Response<CompositePaymentMethodRechargeResponse>, tonic::Status> {
        self.process_recharge(request).await
    }
}

