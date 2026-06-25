use crate::payments::CompositeAccessTokenRequest;
use crate::transformers::ForeignFrom;
use crate::utils::frm_connector_from_composite_frm_metadata;
use connector_integration::types::FrmConnectorData;
use common_utils::consts::{X_CONNECTOR_NAME, X_FRM_CONNECTOR_NAME};
use domain_types::connector_types::{ConnectorEnum, ConnectorVariant, FrmConnectorEnum};
use grpc_api_types::frm::{
    composite_fraud_and_risk_management_service_server::CompositeFraudAndRiskManagementService,
    fraud_and_risk_management_service_server::FraudAndRiskManagementService,
    CompositeFrmPostRiskCheckRequest, CompositeFrmPostRiskCheckResponse,
    CompositeFrmPreRiskCheckRequest, CompositeFrmPreRiskCheckResponse,
    FrmServicePostRiskCheckRequest, FrmServicePostRiskCheckResponse, FrmServicePreRiskCheckRequest,
    FrmServicePreRiskCheckResponse,
};
use grpc_api_types::payments::{
    MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest,
    MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse,
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

/// Composite Fraud and Risk Management Service that wraps the underlying FRM service
/// with access token bootstrapping.
#[derive(Clone)]
pub struct Frm<F, MA>
where
    F: FraudAndRiskManagementService + Clone + Send + Sync + 'static,
    MA: grpc_api_types::payments::merchant_authentication_service_server::MerchantAuthenticationService
        + Clone
        + Send
        + Sync
        + 'static,
{
    frm_service: F,
    merchant_authentication_service: MA,
}

impl<F, MA> Frm<F, MA>
where
    F: FraudAndRiskManagementService + Clone + Send + Sync + 'static,
    MA: grpc_api_types::payments::merchant_authentication_service_server::MerchantAuthenticationService
        + Clone
        + Send
        + Sync
        + 'static,
{
    pub fn new(frm_service: F, merchant_authentication_service: MA) -> Self {
        Self {
            frm_service,
            merchant_authentication_service,
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

        let access_token_connector = match frm_connector {
            FrmConnectorEnum::Kount => ConnectorVariant::Payment(ConnectorEnum::Kount),
        };
        let access_token_payload = payload.build_access_token_request(&access_token_connector);

        let mut access_token_metadata = metadata.clone();
        access_token_metadata.remove(X_FRM_CONNECTOR_NAME);
        access_token_metadata.insert(
            X_CONNECTOR_NAME,
            tonic::metadata::MetadataValue::try_from("kount")
                .map_err(|_| tonic::Status::invalid_argument("invalid x-connector value"))?,
        );

        let mut access_token_request = tonic::Request::new(access_token_payload);
        *access_token_request.metadata_mut() = access_token_metadata;
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
}

#[tonic::async_trait]
impl<F, MA> CompositeFraudAndRiskManagementService for Frm<F, MA>
where
    F: FraudAndRiskManagementService + Clone + Send + Sync + 'static,
    MA: grpc_api_types::payments::merchant_authentication_service_server::MerchantAuthenticationService
        + Clone
        + Send
        + Sync
        + 'static,
{
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
