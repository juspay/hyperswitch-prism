use connector_integration::types::ConnectorData;
use domain_types::{
    connector_types::{ConnectorEnum, ServerAuthenticationTokenResponseData},
    payment_method_data::DefaultPCIHolder,
    utils::ForeignTryFrom as _,
};
use grpc_api_types::frm::{
    fraud_and_risk_management_service_server::FraudAndRiskManagementService,
    CompositeFrmPostRiskCheckRequest, CompositeFrmPostRiskCheckResponse,
    CompositeFrmPreRiskCheckRequest, CompositeFrmPreRiskCheckResponse,
    FrmServicePostRiskCheckRequest, FrmServicePostRiskCheckResponse, FrmServicePreRiskCheckRequest,
    FrmServicePreRiskCheckResponse,
};
use grpc_api_types::payments::{
    ConnectorState as PaymentsConnectorState,
    MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest,
    MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse, PaymentMethod,
};
use prost::Message as ProstMessage;
use ucs_env::error::ResultExtGrpc;

use crate::payments::CompositeAccessTokenRequest;
use crate::transformers::ForeignFrom;
use crate::utils::connector_from_composite_authorize_metadata;

impl CompositeAccessTokenRequest for CompositeFrmPreRiskCheckRequest {
    fn payment_method(&self) -> Option<PaymentMethod> {
        None
    }

    fn state(&self) -> Option<&PaymentsConnectorState> {
        None
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
    fn payment_method(&self) -> Option<PaymentMethod> {
        None
    }

    fn state(&self) -> Option<&PaymentsConnectorState> {
        None
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

    /// Bootstrap the connector's session token for FRM requests.
    ///
    /// Works directly with FRM types since `frm::ConnectorState` and
    /// `payments::ConnectorState` are structurally identical proto types but
    /// different Rust types, so we can't use the generic
    /// `CompositeAccessTokenRequest::state()` path here.
    async fn create_server_authentication_token(
        &self,
        connector: &ConnectorEnum,
        payment_method: Option<grpc_api_types::frm::PaymentMethod>,
        frm_state: Option<&grpc_api_types::frm::ConnectorState>,
        access_token_request: MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest,
        metadata: &tonic::metadata::MetadataMap,
        extensions: &tonic::Extensions,
    ) -> Result<
        Option<MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
        tonic::Status,
    > {
        let should_do_access_token = {
            let payments_pm = payment_method.and_then(|pm| {
                let mut buf = Vec::new();
                pm.encode(&mut buf).ok()?;
                PaymentMethod::decode(buf.as_slice()).ok()
            });
            let pm = payments_pm
                .map(common_enums::PaymentMethod::foreign_try_from)
                .transpose()
                .into_grpc_status()?;
            let connector_data = ConnectorData::<DefaultPCIHolder>::get_connector_by_name(connector);
            connector_data.connector.should_do_access_token(pm)
        };

        let payload_access_token = frm_state
            .and_then(|s| s.access_token.as_ref())
            .and_then(|token| {
                let mut buf = Vec::new();
                token.encode(&mut buf).ok()?;
                let payments_token: grpc_api_types::payments::AccessToken =
                    ProstMessage::decode(buf.as_slice()).ok()?;
                ServerAuthenticationTokenResponseData::foreign_try_from(&payments_token).ok()
            });
        let should_create_access_token = should_do_access_token && payload_access_token.is_none();

        let access_token_response = match should_create_access_token {
            true => {
                let mut req = tonic::Request::new(access_token_request);
                *req.metadata_mut() = metadata.clone();
                *req.extensions_mut() = extensions.clone();

                let response = self
                    .merchant_authentication_service
                    .create_server_authentication_token(req)
                    .await?
                    .into_inner();

                Some(response)
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
                payload.payment_method.clone(),
                payload.state.as_ref(),
                payload.build_access_token_request(&connector),
                &metadata,
                &extensions,
            )
            .await?;
        let pre_risk_check_response = self
            .pre_risk_check(&payload, access_token_response.as_ref(), &metadata, &extensions)
            .await?;

        let frm_access_token_response = access_token_response.and_then(|r| {
            let mut buf = Vec::new();
            r.encode(&mut buf).ok()?;
            grpc_api_types::frm::MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse::decode(buf.as_slice()).ok()
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
                payload.payment_method.clone(),
                payload.state.as_ref(),
                payload.build_access_token_request(&connector),
                &metadata,
                &extensions,
            )
            .await?;
        let post_risk_check_response = self
            .post_risk_check(&payload, access_token_response.as_ref(), &metadata, &extensions)
            .await?;

        let frm_access_token_response = access_token_response.and_then(|r| {
            let mut buf = Vec::new();
            r.encode(&mut buf).ok()?;
            grpc_api_types::frm::MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse::decode(buf.as_slice()).ok()
        });

        Ok(tonic::Response::new(CompositeFrmPostRiskCheckResponse {
            post_risk_check_response: Some(post_risk_check_response),
            access_token_response: frm_access_token_response,
        }))
    }
}

#[tonic::async_trait]
impl<F, MA> grpc_api_types::frm::composite_fraud_and_risk_management_service_server::CompositeFraudAndRiskManagementService for Frm<F, MA>
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
