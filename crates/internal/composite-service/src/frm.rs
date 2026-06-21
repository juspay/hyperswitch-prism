use connector_integration::types::ConnectorData;
use domain_types::{
    connector_types::ConnectorEnum, payment_method_data::DefaultPCIHolder,
    utils::ForeignTryFrom as _,
};
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
use ucs_env::error::ResultExtGrpc;

use crate::payments::CompositeAccessTokenRequest;
use crate::transformers::ForeignFrom;
use crate::utils::connector_from_composite_authorize_metadata;

impl CompositeAccessTokenRequest for CompositeFrmPreRiskCheckRequest {
    fn payment_method(&self) -> Option<grpc_api_types::payments::PaymentMethod> {
        None
    }

    fn state(&self) -> Option<&grpc_api_types::payments::ConnectorState> {
        None
    }

    fn has_existing_access_token(&self) -> bool {
        self.state
            .as_ref()
            .and_then(|s| s.access_token.as_ref())
            .and_then(|at| at.token.as_ref())
            .is_some()
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

impl CompositeAccessTokenRequest for CompositeFrmPostRiskCheckRequest {
    fn payment_method(&self) -> Option<grpc_api_types::payments::PaymentMethod> {
        None
    }

    fn state(&self) -> Option<&grpc_api_types::payments::ConnectorState> {
        None
    }

    fn has_existing_access_token(&self) -> bool {
        self.state
            .as_ref()
            .and_then(|s| s.access_token.as_ref())
            .and_then(|at| at.token.as_ref())
            .is_some()
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
            let connector_data = ConnectorData::<DefaultPCIHolder>::get_connector_by_name(connector);
            connector_data
                .connector
                .should_do_access_token(payment_method)
        };
        let should_create_access_token = should_do_access_token && !payload.has_existing_access_token();

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

        let connector =
            connector_from_composite_authorize_metadata(&metadata).map_err(|err| *err)?;

        let access_token_response = self
            .create_server_authentication_token(
                &connector,
                &payload,
                &metadata,
                &extensions,
            )
            .await?;

        let pre_risk_check_response = self
            .pre_risk_check(
                &payload,
                access_token_response.as_ref(),
                &metadata,
                &extensions,
            )
            .await?;

        // Field-by-field mapping required: create_server_authentication_token returns payments::MASATR
        // but CompositeFrmPreRiskCheckResponse expects frm::MASATR — same proto, different Rust modules.
        let frm_access_token_response = access_token_response.map(|r| {
            grpc_api_types::frm::MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse {
                access_token: r.access_token,
                token_type: r.token_type,
                expires_in_seconds: r.expires_in_seconds,
                status: r.status,
                error: None,
                status_code: r.status_code,
                merchant_access_token_id: r.merchant_access_token_id,
            }
        });

        Ok(tonic::Response::new(CompositeFrmPreRiskCheckResponse {
            pre_risk_check_response: Some(pre_risk_check_response),
            access_token_response: frm_access_token_response,
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

        let connector =
            connector_from_composite_authorize_metadata(&metadata).map_err(|err| *err)?;

        let access_token_response = self
            .create_server_authentication_token(
                &connector,
                &payload,
                &metadata,
                &extensions,
            )
            .await?;

        let post_risk_check_response = self
            .post_risk_check(
                &payload,
                access_token_response.as_ref(),
                &metadata,
                &extensions,
            )
            .await?;

        // Field-by-field mapping required: create_server_authentication_token returns payments::MASATR
        // but CompositeFrmPostRiskCheckResponse expects frm::MASATR — same proto, different Rust modules.
        let frm_access_token_response = access_token_response.map(|r| {
            grpc_api_types::frm::MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse {
                access_token: r.access_token,
                token_type: r.token_type,
                expires_in_seconds: r.expires_in_seconds,
                status: r.status,
                error: None,
                status_code: r.status_code,
                merchant_access_token_id: r.merchant_access_token_id,
            }
        });

        Ok(tonic::Response::new(CompositeFrmPostRiskCheckResponse {
            post_risk_check_response: Some(post_risk_check_response),
            access_token_response: frm_access_token_response,
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
