use crate::payments::CompositeAccessTokenRequest;
use crate::transformers::ForeignFrom;
use crate::utils::frm_connector_from_composite_frm_metadata;
use common_utils::consts::{X_CONNECTOR_NAME, X_FRM_CONNECTOR_NAME};
use connector_integration::types::FrmConnectorData;
use domain_types::connector_types::ConnectorVariant;
use grpc_api_types::frm::{
    composite_fraud_and_risk_management_service_server::CompositeFraudAndRiskManagementService,
    fraud_and_risk_management_service_server::FraudAndRiskManagementService,
    CompositeFrmDeviceDataCollectionRequest, CompositeFrmDeviceDataCollectionResponse,
    CompositeFrmPostRiskCheckRequest, CompositeFrmPostRiskCheckResponse,
    CompositeFrmPreRiskCheckRequest, CompositeFrmPreRiskCheckResponse,
    FrmServicePostRiskCheckRequest, FrmServicePostRiskCheckResponse, FrmServicePreRiskCheckRequest,
    FrmServicePreRiskCheckResponse,
};
use grpc_api_types::payments::{
    MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest,
    MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse,
    PaymentMethodAuthenticationServicePreAuthenticateRequest,
    PaymentMethodAuthenticationServicePreAuthenticateResponse,
};

impl CompositeAccessTokenRequest for CompositeFrmPreRiskCheckRequest {
    fn payment_method(&self) -> Option<grpc_api_types::payments::PaymentMethod> {
        None
    }

    fn state(&self) -> Option<&grpc_api_types::payments::ConnectorState> {
        self.state.as_ref()
    }

    fn build_access_token_request(
        &self,
        connector: &ConnectorVariant,
    ) -> MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest {
        MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest::foreign_from((
            self, connector,
        ))
    }
}

impl CompositeAccessTokenRequest for CompositeFrmPostRiskCheckRequest {
    fn payment_method(&self) -> Option<grpc_api_types::payments::PaymentMethod> {
        None
    }

    fn state(&self) -> Option<&grpc_api_types::payments::ConnectorState> {
        self.state.as_ref()
    }

    fn build_access_token_request(
        &self,
        connector: &ConnectorVariant,
    ) -> MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest {
        MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest::foreign_from((
            self, connector,
        ))
    }
}

impl CompositeAccessTokenRequest for CompositeFrmDeviceDataCollectionRequest {
    fn payment_method(&self) -> Option<grpc_api_types::payments::PaymentMethod> {
        self.payment_method.clone()
    }

    fn state(&self) -> Option<&grpc_api_types::payments::ConnectorState> {
        self.state.as_ref()
    }

    fn build_access_token_request(
        &self,
        connector: &ConnectorVariant,
    ) -> MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest {
        MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest::foreign_from((
            self, connector,
        ))
    }
}

/// Composite Fraud and Risk Management Service that wraps the underlying FRM service
/// with access token bootstrapping.
#[derive(Clone)]
pub struct Frm<F, MA, PMA>
where
    F: FraudAndRiskManagementService + Clone + Send + Sync + 'static,
    MA: grpc_api_types::payments::merchant_authentication_service_server::MerchantAuthenticationService
        + Clone
        + Send
        + Sync
        + 'static,
    PMA: grpc_api_types::payments::payment_method_authentication_service_server::PaymentMethodAuthenticationService
        + Clone
        + Send
        + Sync
        + 'static,
{
    frm_service: F,
    merchant_authentication_service: MA,
    payment_method_authentication_service: PMA,
}

impl<F, MA, PMA> Frm<F, MA, PMA>
where
    F: FraudAndRiskManagementService + Clone + Send + Sync + 'static,
    MA: grpc_api_types::payments::merchant_authentication_service_server::MerchantAuthenticationService
        + Clone
        + Send
        + Sync
        + 'static,
    PMA: grpc_api_types::payments::payment_method_authentication_service_server::PaymentMethodAuthenticationService
        + Clone
        + Send
        + Sync
        + 'static,
{
    pub fn new(
        frm_service: F,
        merchant_authentication_service: MA,
        payment_method_authentication_service: PMA,
    ) -> Self {
        Self {
            frm_service,
            merchant_authentication_service,
            payment_method_authentication_service,
        }
    }

    // FrmConnectorEnum is currently empty, so the body after the early-return is unreachable
    // at runtime. The variables and expressions are kept for when a real FRM connector is added.
    #[allow(unreachable_code, unused_variables)]
    async fn create_server_authentication_token<R: CompositeAccessTokenRequest>(
        &self,
        payload: &R,
        metadata: &tonic::metadata::MetadataMap,
        extensions: &tonic::Extensions,
    ) -> Result<
        Option<MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
        tonic::Status,
    > {
        // Resolve the FRM connector from x-frm-connector. Returns Ok(None) when
        // the header is absent (no dedicated FRM connector → no access token).
        let Some(frm_connector) =
            frm_connector_from_composite_frm_metadata(metadata).map_err(|err| *err)?
        else {
            return Ok(None);
        };

        let has_existing_access_token = payload
            .state()
            .and_then(|state| state.access_token.as_ref())
            .is_some();
        let should_create_access_token =
            FrmConnectorData::get_connector_by_name(&frm_connector)
                .connector
                .should_do_access_token(None)
                && !has_existing_access_token;

        if !should_create_access_token {
            return Ok(None);
        }

        let access_token_payload =
            payload.build_access_token_request(&ConnectorVariant::Frm(frm_connector));
        let mut access_token_request = tonic::Request::new(access_token_payload);
        *access_token_request.metadata_mut() = metadata.clone();
        *access_token_request.extensions_mut() = extensions.clone();

        let access_token_response = self
            .merchant_authentication_service
            .create_server_authentication_token(access_token_request)
            .await?
            .into_inner();

        Ok(Some(access_token_response))
    }

    async fn pre_risk_check(
        &self,
        payload: &CompositeFrmPreRiskCheckRequest,
        access_token_response: Option<
            &MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse,
        >,
        metadata: &tonic::metadata::MetadataMap,
        extensions: &tonic::Extensions,
    ) -> Result<FrmServicePreRiskCheckResponse, tonic::Status> {
        let inner = FrmServicePreRiskCheckRequest::foreign_from((payload, access_token_response));
        let mut inner_request = tonic::Request::new(inner);
        *inner_request.metadata_mut() = metadata.clone();
        *inner_request.extensions_mut() = extensions.clone();

        let response = self
            .frm_service
            .pre_risk_check(inner_request)
            .await?
            .into_inner();

        Ok(response)
    }

    async fn process_pre_risk_check(
        &self,
        request: tonic::Request<CompositeFrmPreRiskCheckRequest>,
    ) -> Result<tonic::Response<CompositeFrmPreRiskCheckResponse>, tonic::Status> {
        let (metadata, extensions, payload) = request.into_parts();

        let access_token_response = self
            .create_server_authentication_token(&payload, &metadata, &extensions)
            .await?;

        let pre_risk_check_response = self
            .pre_risk_check(
                &payload,
                access_token_response.as_ref(),
                &metadata,
                &extensions,
            )
            .await?;

        Ok(tonic::Response::new(CompositeFrmPreRiskCheckResponse {
            pre_risk_check_response: Some(pre_risk_check_response),
            access_token_response,
        }))
    }

    async fn post_risk_check(
        &self,
        payload: &CompositeFrmPostRiskCheckRequest,
        access_token_response: Option<
            &MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse,
        >,
        metadata: &tonic::metadata::MetadataMap,
        extensions: &tonic::Extensions,
    ) -> Result<FrmServicePostRiskCheckResponse, tonic::Status> {
        let inner = FrmServicePostRiskCheckRequest::foreign_from((payload, access_token_response));
        let mut inner_request = tonic::Request::new(inner);
        *inner_request.metadata_mut() = metadata.clone();
        *inner_request.extensions_mut() = extensions.clone();

        let response = self
            .frm_service
            .post_risk_check(inner_request)
            .await?
            .into_inner();

        Ok(response)
    }

    async fn process_post_risk_check(
        &self,
        request: tonic::Request<CompositeFrmPostRiskCheckRequest>,
    ) -> Result<tonic::Response<CompositeFrmPostRiskCheckResponse>, tonic::Status> {
        let (metadata, extensions, payload) = request.into_parts();

        let access_token_response = self
            .create_server_authentication_token(&payload, &metadata, &extensions)
            .await?;

        let post_risk_check_response = self
            .post_risk_check(
                &payload,
                access_token_response.as_ref(),
                &metadata,
                &extensions,
            )
            .await?;

        Ok(tonic::Response::new(CompositeFrmPostRiskCheckResponse {
            post_risk_check_response: Some(post_risk_check_response),
            access_token_response,
        }))
    }

    fn payment_authentication_metadata(
        metadata: &tonic::metadata::MetadataMap,
    ) -> tonic::metadata::MetadataMap {
        let mut inner_metadata = metadata.clone();
        if let Some(frm_connector) = inner_metadata.remove(X_FRM_CONNECTOR_NAME) {
            if !inner_metadata.contains_key(X_CONNECTOR_NAME) {
                inner_metadata.insert(X_CONNECTOR_NAME, frm_connector);
            }
        }
        inner_metadata
    }

    async fn device_data_collection(
        &self,
        payload: &CompositeFrmDeviceDataCollectionRequest,
        access_token_response: Option<
            &MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse,
        >,
        metadata: &tonic::metadata::MetadataMap,
        extensions: &tonic::Extensions,
    ) -> Result<PaymentMethodAuthenticationServicePreAuthenticateResponse, tonic::Status> {
        let inner = PaymentMethodAuthenticationServicePreAuthenticateRequest::foreign_from((
            payload,
            access_token_response,
        ));
        let mut inner_request = tonic::Request::new(inner);
        *inner_request.metadata_mut() = Self::payment_authentication_metadata(metadata);
        *inner_request.extensions_mut() = extensions.clone();

        let response = self
            .payment_method_authentication_service
            .pre_authenticate(inner_request)
            .await?
            .into_inner();

        Ok(response)
    }

    async fn process_device_data_collection(
        &self,
        request: tonic::Request<CompositeFrmDeviceDataCollectionRequest>,
    ) -> Result<tonic::Response<CompositeFrmDeviceDataCollectionResponse>, tonic::Status> {
        let (metadata, extensions, payload) = request.into_parts();

        let access_token_response = self
            .create_server_authentication_token(&payload, &metadata, &extensions)
            .await?;

        let device_data_collection_response = self
            .device_data_collection(
                &payload,
                access_token_response.as_ref(),
                &metadata,
                &extensions,
            )
            .await?;

        Ok(tonic::Response::new(
            CompositeFrmDeviceDataCollectionResponse {
                device_data_collection_response: Some(device_data_collection_response),
                access_token_response,
            },
        ))
    }
}

#[tonic::async_trait]
impl<F, MA, PMA> CompositeFraudAndRiskManagementService for Frm<F, MA, PMA>
where
    F: FraudAndRiskManagementService + Clone + Send + Sync + 'static,
    MA: grpc_api_types::payments::merchant_authentication_service_server::MerchantAuthenticationService
        + Clone
        + Send
        + Sync
        + 'static,
    PMA: grpc_api_types::payments::payment_method_authentication_service_server::PaymentMethodAuthenticationService
        + Clone
        + Send
        + Sync
        + 'static,
{
    async fn device_data_collection(
        &self,
        request: tonic::Request<CompositeFrmDeviceDataCollectionRequest>,
    ) -> Result<tonic::Response<CompositeFrmDeviceDataCollectionResponse>, tonic::Status> {
        self.process_device_data_collection(request).await
    }

    async fn pre_risk_check(
        &self,
        request: tonic::Request<CompositeFrmPreRiskCheckRequest>,
    ) -> Result<tonic::Response<CompositeFrmPreRiskCheckResponse>, tonic::Status> {
        self.process_pre_risk_check(request).await
    }

    async fn post_risk_check(
        &self,
        request: tonic::Request<CompositeFrmPostRiskCheckRequest>,
    ) -> Result<tonic::Response<CompositeFrmPostRiskCheckResponse>, tonic::Status> {
        self.process_post_risk_check(request).await
    }
}
