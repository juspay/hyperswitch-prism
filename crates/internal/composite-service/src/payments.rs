use connector_integration::types::ConnectorData;
use domain_types::{
    connector_types::{ConnectorEnum, ServerAuthenticationTokenResponseData},
    utils::ForeignTryFrom as _,
};
use grpc_api_types::payments::{
    composite_payment_service_server::CompositePaymentService,
    composite_refund_service_server::CompositeRefundService,
    customer_service_server::CustomerService,
    merchant_authentication_service_server::MerchantAuthenticationService,
    payment_method_authentication_service_server::PaymentMethodAuthenticationService,
    payment_service_server::PaymentService, refund_service_server::RefundService,
    CompositeAuthorizeRequest, CompositeAuthorizeResponse, CompositeCaptureRequest,
    CompositeCaptureResponse, CompositeGetRequest, CompositeGetResponse, CompositeRefundGetRequest,
    CompositeRefundGetResponse, CompositeRefundRequest, CompositeRefundResponse, CompositeStatus,
    CompositeVoidRequest, CompositeVoidResponse, ConnectorState, CustomerServiceCreateResponse,
    MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest,
    MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse, PaymentMethod,
    PaymentMethodAuthenticationServiceAuthenticateRequest,
    PaymentMethodAuthenticationServiceAuthenticateResponse,
    PaymentMethodAuthenticationServicePostAuthenticateRequest,
    PaymentMethodAuthenticationServicePostAuthenticateResponse,
    PaymentMethodAuthenticationServicePreAuthenticateRequest,
    PaymentMethodAuthenticationServicePreAuthenticateResponse, PaymentServiceAuthorizeRequest,
    PaymentServiceCaptureRequest, PaymentServiceCaptureResponse, PaymentServiceCreateOrderRequest,
    PaymentServiceCreateOrderResponse, PaymentServiceGetResponse, PaymentServiceRefundRequest,
    PaymentServiceVoidRequest, PaymentServiceVoidResponse, RefundResponse, RefundServiceGetRequest,
};

use crate::transformers::ForeignFrom;
use crate::utils::connector_from_composite_authorize_metadata;

/// Trait for abstracting access to common fields needed for access token creation.
pub trait CompositeAccessTokenRequest {
    fn payment_method(&self) -> Option<PaymentMethod>;
    fn state(&self) -> Option<&ConnectorState>;
    fn build_access_token_request(
        &self,
        connector: &ConnectorEnum,
    ) -> MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest;
}

impl CompositeAccessTokenRequest for CompositeAuthorizeRequest {
    fn payment_method(&self) -> Option<PaymentMethod> {
        self.payment_method.clone()
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

impl CompositeAccessTokenRequest for CompositeGetRequest {
    fn payment_method(&self) -> Option<PaymentMethod> {
        self.payment_method.clone()
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

impl CompositeAccessTokenRequest for CompositeRefundRequest {
    fn payment_method(&self) -> Option<PaymentMethod> {
        self.payment_method.clone()
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

impl CompositeAccessTokenRequest for CompositeRefundGetRequest {
    fn payment_method(&self) -> Option<PaymentMethod> {
        self.payment_method.clone()
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

impl CompositeAccessTokenRequest for CompositeVoidRequest {
    fn payment_method(&self) -> Option<PaymentMethod> {
        self.payment_method.clone()
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

impl CompositeAccessTokenRequest for CompositeCaptureRequest {
    fn payment_method(&self) -> Option<PaymentMethod> {
        self.payment_method.clone()
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

/// Stages of the authentication+authorization flow determined by the stateless decider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuthNStage {
    /// Fresh request with no authN state; connector does NOT require PreAuthenticate.
    FreshNoPreAuth,
    /// Fresh request; connector requires PreAuthenticate first.
    FreshWithPreAuth,
    /// Redirect response present WITHOUT threeds_completion_indicator — this is a 3DS challenge callback.
    PostChallengeRedirect,
    /// Authentication data already provided (from prior Authenticate call that was frictionless).
    PreAuthed,
}

/// Determines the current authN stage purely from the request (stateless decider).
fn decide_authn_stage(payload: &CompositeAuthorizeRequest) -> AuthNStage {
    let has_redirection = payload.redirection_response.is_some();
    let has_auth_data = payload.authentication_data.is_some();
    let has_ddc_indicator = payload.threeds_completion_indicator.is_some();

    // CASE: Post-challenge redirect (3DS challenge completed, browser redirected back)
    // redirection_response is present but no DDC indicator (DDC is separate step)
    if has_redirection && !has_ddc_indicator {
        return AuthNStage::PostChallengeRedirect;
    }

    // CASE: Pre-authed (authentication data already provided from frictionless flow)
    if has_auth_data && !has_redirection {
        return AuthNStage::PreAuthed;
    }

    // CASE: Fresh start — need to check connector to decide between FreshNoPreAuth vs FreshWithPreAuth
    // The actual connector check happens in process_composite_authorize after we have the connector enum
    // For now, return FreshWithPreAuth as default; the caller will adjust if connector doesn't need it
    AuthNStage::FreshWithPreAuth
}

#[derive(Clone)]
pub struct Payments<P, M, C, R, A> {
    payment_service: P,
    merchant_authentication_service: M,
    customer_service: C,
    refund_service: R,
    authentication_service: A,
}

impl<P, M, C, R, A> Payments<P, M, C, R, A> {
    pub fn new(
        payment_service: P,
        merchant_authentication_service: M,
        customer_service: C,
        refund_service: R,
        authentication_service: A,
    ) -> Self {
        Self {
            payment_service,
            merchant_authentication_service,
            customer_service,
            refund_service,
            authentication_service,
        }
    }
}

impl<P, M, C, R, A> Payments<P, M, C, R, A>
where
    P: PaymentService + Clone + Send + Sync + 'static,
    M: MerchantAuthenticationService + Clone + Send + Sync + 'static,
    C: CustomerService + Clone + Send + Sync + 'static,
    R: RefundService + Clone + Send + Sync + 'static,
    A: PaymentMethodAuthenticationService + Clone + Send + Sync + 'static,
{
    async fn create_server_authentication_token<Req: CompositeAccessTokenRequest>(
        &self,
        connector: &ConnectorEnum,
        payload: &Req,
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

    async fn create_connector_customer(
        &self,
        connector: &ConnectorEnum,
        payload: &CompositeAuthorizeRequest,
        metadata: &tonic::metadata::MetadataMap,
        extensions: &tonic::Extensions,
    ) -> Result<Option<CustomerServiceCreateResponse>, tonic::Status> {
        let connector_data = ConnectorData::<domain_types::payment_method_data::DefaultPCIHolder>::get_connector_by_name(connector);
        let connector_customer_id = payload
            .state
            .as_ref()
            .and_then(|state| state.connector_customer_id.clone())
            .or_else(|| {
                payload
                    .customer
                    .as_ref()
                    .and_then(|c| c.connector_customer_id.clone())
            });
        let should_create_connector_customer =
            connector_data.connector.should_create_connector_customer()
                && connector_customer_id.is_none();

        let create_customer_response = match should_create_connector_customer {
            true => {
                let create_customer_payload =
                    grpc_api_types::payments::CustomerServiceCreateRequest::foreign_from(payload);
                let mut create_customer_request = tonic::Request::new(create_customer_payload);
                *create_customer_request.metadata_mut() = metadata.clone();
                *create_customer_request.extensions_mut() = extensions.clone();

                let create_customer_response = self
                    .customer_service
                    .create(create_customer_request)
                    .await?
                    .into_inner();

                Some(create_customer_response)
            }
            false => None,
        };

        Ok(create_customer_response)
    }

    async fn create_order(
        &self,
        connector: &ConnectorEnum,
        payload: &CompositeAuthorizeRequest,
        access_token_response: Option<
            &MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse,
        >,
        metadata: &tonic::metadata::MetadataMap,
        extensions: &tonic::Extensions,
    ) -> Result<Option<PaymentServiceCreateOrderResponse>, tonic::Status> {
        let connector_data = ConnectorData::<domain_types::payment_method_data::DefaultPCIHolder>::get_connector_by_name(connector);
        let should_create_order = connector_data.connector.should_do_order_create();

        let create_order_response = match should_create_order {
            true => {
                let create_order_payload = PaymentServiceCreateOrderRequest::foreign_from((
                    payload,
                    access_token_response,
                ));
                let mut create_order_request = tonic::Request::new(create_order_payload);
                *create_order_request.metadata_mut() = metadata.clone();
                *create_order_request.extensions_mut() = extensions.clone();

                let create_order_response = self
                    .payment_service
                    .create_order(create_order_request)
                    .await?
                    .into_inner();

                Some(create_order_response)
            }
            false => None,
        };

        Ok(create_order_response)
    }

    async fn authorize(
        &self,
        payload: &CompositeAuthorizeRequest,
        access_token_response: Option<
            &MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse,
        >,
        create_customer_response: Option<&CustomerServiceCreateResponse>,
        create_order_response: Option<&PaymentServiceCreateOrderResponse>,
        authenticate_response: Option<&PaymentMethodAuthenticationServiceAuthenticateResponse>,
        post_authenticate_response: Option<
            &PaymentMethodAuthenticationServicePostAuthenticateResponse,
        >,
        metadata: &tonic::metadata::MetadataMap,
        extensions: &tonic::Extensions,
    ) -> Result<grpc_api_types::payments::PaymentServiceAuthorizeResponse, tonic::Status> {
        let authorize_payload = PaymentServiceAuthorizeRequest::foreign_from((
            payload,
            access_token_response,
            create_customer_response,
            create_order_response,
            authenticate_response,
            post_authenticate_response,
        ));

        let mut authorize_request = tonic::Request::new(authorize_payload);
        *authorize_request.metadata_mut() = metadata.clone();
        *authorize_request.extensions_mut() = extensions.clone();

        let authorize_response = self
            .payment_service
            .authorize(authorize_request)
            .await?
            .into_inner();

        Ok(authorize_response)
    }

    async fn pre_authenticate(
        &self,
        payload: &CompositeAuthorizeRequest,
        metadata: &tonic::metadata::MetadataMap,
        extensions: &tonic::Extensions,
    ) -> Result<PaymentMethodAuthenticationServicePreAuthenticateResponse, tonic::Status> {
        let pre_auth_payload =
            PaymentMethodAuthenticationServicePreAuthenticateRequest::foreign_from(payload);
        let mut pre_auth_request = tonic::Request::new(pre_auth_payload);
        *pre_auth_request.metadata_mut() = metadata.clone();
        *pre_auth_request.extensions_mut() = extensions.clone();

        let pre_auth_response = self
            .authentication_service
            .pre_authenticate(pre_auth_request)
            .await?
            .into_inner();

        Ok(pre_auth_response)
    }

    async fn authenticate(
        &self,
        payload: &CompositeAuthorizeRequest,
        pre_auth_response: &PaymentMethodAuthenticationServicePreAuthenticateResponse,
        metadata: &tonic::metadata::MetadataMap,
        extensions: &tonic::Extensions,
    ) -> Result<PaymentMethodAuthenticationServiceAuthenticateResponse, tonic::Status> {
        let auth_payload = PaymentMethodAuthenticationServiceAuthenticateRequest::foreign_from((
            payload,
            pre_auth_response,
        ));
        let mut auth_request = tonic::Request::new(auth_payload);
        *auth_request.metadata_mut() = metadata.clone();
        *auth_request.extensions_mut() = extensions.clone();

        let auth_response = self
            .authentication_service
            .authenticate(auth_request)
            .await?
            .into_inner();

        Ok(auth_response)
    }

    async fn post_authenticate(
        &self,
        payload: &CompositeAuthorizeRequest,
        auth_response: &PaymentMethodAuthenticationServiceAuthenticateResponse,
        metadata: &tonic::metadata::MetadataMap,
        extensions: &tonic::Extensions,
    ) -> Result<PaymentMethodAuthenticationServicePostAuthenticateResponse, tonic::Status> {
        let post_auth_payload =
            PaymentMethodAuthenticationServicePostAuthenticateRequest::foreign_from((
                payload,
                auth_response,
            ));
        let mut post_auth_request = tonic::Request::new(post_auth_payload);
        *post_auth_request.metadata_mut() = metadata.clone();
        *post_auth_request.extensions_mut() = extensions.clone();

        let post_auth_response = self
            .authentication_service
            .post_authenticate(post_auth_request)
            .await?
            .into_inner();

        Ok(post_auth_response)
    }

    /// Main composite authorize processor with stateless 3DS authN decider.
    async fn process_composite_authorize(
        &self,
        request: tonic::Request<CompositeAuthorizeRequest>,
    ) -> Result<tonic::Response<CompositeAuthorizeResponse>, tonic::Status> {
        let (metadata, extensions, payload) = request.into_parts();

        let connector =
            connector_from_composite_authorize_metadata(&metadata).map_err(|err| *err)?;

        // Get connector characteristics
        let connector_data = ConnectorData::<domain_types::payment_method_data::DefaultPCIHolder>::get_connector_by_name(&connector);
        let requires_pre_auth = connector_data.connector.requires_pre_authentication();
        let requires_post_auth = connector_data.connector.requires_post_authentication();

        // Determine authN stage from request fields (stateless decider)
        let initial_stage = decide_authn_stage(&payload);

        // Adjust stage based on connector requirements
        let stage = match initial_stage {
            AuthNStage::FreshWithPreAuth if !requires_pre_auth => AuthNStage::FreshNoPreAuth,
            other => other,
        };

        match stage {
            AuthNStage::FreshNoPreAuth => {
                // Standard non-3DS flow: AccessToken → Customer → Order → Authorize
                let access_token_response = self
                    .create_server_authentication_token(
                        &connector,
                        &payload,
                        &metadata,
                        &extensions,
                    )
                    .await?;
                let create_customer_response = self
                    .create_connector_customer(&connector, &payload, &metadata, &extensions)
                    .await?;
                let create_order_response = self
                    .create_order(
                        &connector,
                        &payload,
                        access_token_response.as_ref(),
                        &metadata,
                        &extensions,
                    )
                    .await?;
                let authorize_response = self
                    .authorize(
                        &payload,
                        access_token_response.as_ref(),
                        create_customer_response.as_ref(),
                        create_order_response.as_ref(),
                        None,
                        None,
                        &metadata,
                        &extensions,
                    )
                    .await?;

                Ok(tonic::Response::new(CompositeAuthorizeResponse {
                    access_token_response,
                    create_customer_response,
                    create_order_response,
                    authorize_response: Some(authorize_response),
                    composite_status: CompositeStatus::Completed.into(),
                }))
            }

            AuthNStage::FreshWithPreAuth => {
                // 3DS flow starts with PreAuthenticate
                let access_token_response = self
                    .create_server_authentication_token(
                        &connector,
                        &payload,
                        &metadata,
                        &extensions,
                    )
                    .await?;

                let pre_auth_response = self
                    .pre_authenticate(&payload, &metadata, &extensions)
                    .await?;

                // Check if PreAuth returned a redirect (DDC invoke path - not tested)
                if pre_auth_response.redirection_data.is_some() {
                    // Return REDIRECT_REQUIRED — caller must handle DDC form submission
                    // and call again with redirection_response
                    let authorize_response =
                        grpc_api_types::payments::PaymentServiceAuthorizeResponse {
                            status: grpc_api_types::payments::PaymentStatus::AuthenticationPending
                                .into(),
                            redirection_data: pre_auth_response.redirection_data,
                            ..Default::default()
                        };

                    return Ok(tonic::Response::new(CompositeAuthorizeResponse {
                        access_token_response,
                        create_customer_response: None,
                        create_order_response: None,
                        authorize_response: Some(authorize_response),
                        composite_status: CompositeStatus::RedirectRequired.into(),
                    }));
                }

                // PreAuth returned authentication_data (exempt path)
                // Chain into Authenticate immediately (within this same call)
                let auth_response = self
                    .authenticate(&payload, &pre_auth_response, &metadata, &extensions)
                    .await?;

                // Check if Authenticate returned a redirect (3DS challenge)
                if auth_response.redirection_data.is_some() {
                    // Return REDIRECT_REQUIRED — caller must complete 3DS challenge
                    let authorize_response =
                        grpc_api_types::payments::PaymentServiceAuthorizeResponse {
                            status: grpc_api_types::payments::PaymentStatus::AuthenticationPending
                                .into(),
                            redirection_data: auth_response.redirection_data,
                            ..Default::default()
                        };

                    return Ok(tonic::Response::new(CompositeAuthorizeResponse {
                        access_token_response,
                        create_customer_response: None,
                        create_order_response: None,
                        authorize_response: Some(authorize_response),
                        composite_status: CompositeStatus::RedirectRequired.into(),
                    }));
                }

                // Authenticate returned authentication_data (frictionless)
                // For Redsys (requires_post_auth=false): go straight to Authorize
                // For CyberSource (requires_post_auth=true): chain into PostAuthenticate
                let post_auth_response_opt = if requires_post_auth {
                    let post_auth_response = self
                        .post_authenticate(&payload, &auth_response, &metadata, &extensions)
                        .await?;
                    Some(post_auth_response)
                } else {
                    None
                };

                // Now do Customer, Order, Authorize
                let create_customer_response = self
                    .create_connector_customer(&connector, &payload, &metadata, &extensions)
                    .await?;
                let create_order_response = self
                    .create_order(
                        &connector,
                        &payload,
                        access_token_response.as_ref(),
                        &metadata,
                        &extensions,
                    )
                    .await?;

                let authorize_response = self
                    .authorize(
                        &payload,
                        access_token_response.as_ref(),
                        create_customer_response.as_ref(),
                        create_order_response.as_ref(),
                        // For Redsys: pass auth_response directly; for CyberSource: pass None
                        if requires_post_auth {
                            None
                        } else {
                            Some(&auth_response)
                        },
                        post_auth_response_opt.as_ref(),
                        &metadata,
                        &extensions,
                    )
                    .await?;

                Ok(tonic::Response::new(CompositeAuthorizeResponse {
                    access_token_response,
                    create_customer_response,
                    create_order_response,
                    authorize_response: Some(authorize_response),
                    composite_status: CompositeStatus::Completed.into(),
                }))
            }

            AuthNStage::PostChallengeRedirect => {
                // 3DS challenge completed — caller has provided redirection_response
                let access_token_response = self
                    .create_server_authentication_token(
                        &connector,
                        &payload,
                        &metadata,
                        &extensions,
                    )
                    .await?;

                if requires_post_auth {
                    // CyberSource path: PostAuthenticate → Authorize
                    // Need to reconstruct an auth_response from the payload's authentication_data
                    // (which should have been set by the caller from the Authenticate response)
                    let synthetic_auth_response =
                        PaymentMethodAuthenticationServiceAuthenticateResponse {
                            authentication_data: payload.authentication_data.clone(),
                            connector_feature_data: payload.connector_feature_data.clone(),
                            ..Default::default()
                        };

                    let post_auth_response = self
                        .post_authenticate(
                            &payload,
                            &synthetic_auth_response,
                            &metadata,
                            &extensions,
                        )
                        .await?;

                    let create_customer_response = self
                        .create_connector_customer(&connector, &payload, &metadata, &extensions)
                        .await?;
                    let create_order_response = self
                        .create_order(
                            &connector,
                            &payload,
                            access_token_response.as_ref(),
                            &metadata,
                            &extensions,
                        )
                        .await?;

                    let authorize_response = self
                        .authorize(
                            &payload,
                            access_token_response.as_ref(),
                            create_customer_response.as_ref(),
                            create_order_response.as_ref(),
                            None,
                            Some(&post_auth_response),
                            &metadata,
                            &extensions,
                        )
                        .await?;

                    Ok(tonic::Response::new(CompositeAuthorizeResponse {
                        access_token_response,
                        create_customer_response,
                        create_order_response,
                        authorize_response: Some(authorize_response),
                        composite_status: CompositeStatus::Completed.into(),
                    }))
                } else {
                    // Redsys path: Authorize directly with cres from redirection_response
                    let create_customer_response = self
                        .create_connector_customer(&connector, &payload, &metadata, &extensions)
                        .await?;
                    let create_order_response = self
                        .create_order(
                            &connector,
                            &payload,
                            access_token_response.as_ref(),
                            &metadata,
                            &extensions,
                        )
                        .await?;

                    let authorize_response = self
                        .authorize(
                            &payload,
                            access_token_response.as_ref(),
                            create_customer_response.as_ref(),
                            create_order_response.as_ref(),
                            None,
                            None,
                            &metadata,
                            &extensions,
                        )
                        .await?;

                    Ok(tonic::Response::new(CompositeAuthorizeResponse {
                        access_token_response,
                        create_customer_response,
                        create_order_response,
                        authorize_response: Some(authorize_response),
                        composite_status: CompositeStatus::Completed.into(),
                    }))
                }
            }

            AuthNStage::PreAuthed => {
                // Authentication data already provided — skip authN, go straight to Authorize
                let access_token_response = self
                    .create_server_authentication_token(
                        &connector,
                        &payload,
                        &metadata,
                        &extensions,
                    )
                    .await?;
                let create_customer_response = self
                    .create_connector_customer(&connector, &payload, &metadata, &extensions)
                    .await?;
                let create_order_response = self
                    .create_order(
                        &connector,
                        &payload,
                        access_token_response.as_ref(),
                        &metadata,
                        &extensions,
                    )
                    .await?;

                let authorize_response = self
                    .authorize(
                        &payload,
                        access_token_response.as_ref(),
                        create_customer_response.as_ref(),
                        create_order_response.as_ref(),
                        None,
                        None,
                        &metadata,
                        &extensions,
                    )
                    .await?;

                Ok(tonic::Response::new(CompositeAuthorizeResponse {
                    access_token_response,
                    create_customer_response,
                    create_order_response,
                    authorize_response: Some(authorize_response),
                    composite_status: CompositeStatus::Completed.into(),
                }))
            }
        }
    }

    async fn get(
        &self,
        payload: &CompositeGetRequest,
        access_token_response: Option<
            &MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse,
        >,
        metadata: &tonic::metadata::MetadataMap,
        extensions: &tonic::Extensions,
    ) -> Result<PaymentServiceGetResponse, tonic::Status> {
        let get_payload = grpc_api_types::payments::PaymentServiceGetRequest::foreign_from((
            payload,
            access_token_response,
        ));

        let mut get_request = tonic::Request::new(get_payload);
        *get_request.metadata_mut() = metadata.clone();
        *get_request.extensions_mut() = extensions.clone();

        let get_response = self.payment_service.get(get_request).await?.into_inner();

        Ok(get_response)
    }

    async fn process_composite_get(
        &self,
        request: tonic::Request<CompositeGetRequest>,
    ) -> Result<tonic::Response<CompositeGetResponse>, tonic::Status> {
        let (metadata, extensions, payload) = request.into_parts();

        let connector =
            connector_from_composite_authorize_metadata(&metadata).map_err(|err| *err)?;
        let access_token_response = self
            .create_server_authentication_token(&connector, &payload, &metadata, &extensions)
            .await?;
        let get_response = self
            .get(
                &payload,
                access_token_response.as_ref(),
                &metadata,
                &extensions,
            )
            .await?;

        Ok(tonic::Response::new(CompositeGetResponse {
            access_token_response,
            get_response: Some(get_response),
        }))
    }

    async fn refund(
        &self,
        payload: &CompositeRefundRequest,
        access_token_response: Option<
            &MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse,
        >,
        metadata: &tonic::metadata::MetadataMap,
        extensions: &tonic::Extensions,
    ) -> Result<RefundResponse, tonic::Status> {
        let refund_payload =
            PaymentServiceRefundRequest::foreign_from((payload, access_token_response));

        let mut refund_request = tonic::Request::new(refund_payload);
        *refund_request.metadata_mut() = metadata.clone();
        *refund_request.extensions_mut() = extensions.clone();

        let refund_response = self
            .payment_service
            .refund(refund_request)
            .await?
            .into_inner();

        Ok(refund_response)
    }

    async fn process_composite_refund(
        &self,
        request: tonic::Request<CompositeRefundRequest>,
    ) -> Result<tonic::Response<CompositeRefundResponse>, tonic::Status> {
        let (metadata, extensions, payload) = request.into_parts();

        let connector =
            connector_from_composite_authorize_metadata(&metadata).map_err(|err| *err)?;
        let access_token_response = self
            .create_server_authentication_token(&connector, &payload, &metadata, &extensions)
            .await?;
        let refund_response = self
            .refund(
                &payload,
                access_token_response.as_ref(),
                &metadata,
                &extensions,
            )
            .await?;

        Ok(tonic::Response::new(CompositeRefundResponse {
            access_token_response,
            refund_response: Some(refund_response),
        }))
    }

    async fn refund_get(
        &self,
        payload: &CompositeRefundGetRequest,
        access_token_response: Option<
            &MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse,
        >,
        metadata: &tonic::metadata::MetadataMap,
        extensions: &tonic::Extensions,
    ) -> Result<RefundResponse, tonic::Status> {
        let refund_get_payload =
            RefundServiceGetRequest::foreign_from((payload, access_token_response));

        let mut refund_get_request = tonic::Request::new(refund_get_payload);
        *refund_get_request.metadata_mut() = metadata.clone();
        *refund_get_request.extensions_mut() = extensions.clone();

        let refund_get_response = self
            .refund_service
            .get(refund_get_request)
            .await?
            .into_inner();

        Ok(refund_get_response)
    }

    async fn process_composite_refund_get(
        &self,
        request: tonic::Request<CompositeRefundGetRequest>,
    ) -> Result<tonic::Response<CompositeRefundGetResponse>, tonic::Status> {
        let (metadata, extensions, payload) = request.into_parts();

        let connector =
            connector_from_composite_authorize_metadata(&metadata).map_err(|err| *err)?;
        let access_token_response = self
            .create_server_authentication_token(&connector, &payload, &metadata, &extensions)
            .await?;
        let refund_get_response = self
            .refund_get(
                &payload,
                access_token_response.as_ref(),
                &metadata,
                &extensions,
            )
            .await?;

        Ok(tonic::Response::new(CompositeRefundGetResponse {
            access_token_response,
            refund_response: Some(refund_get_response),
        }))
    }

    async fn void(
        &self,
        payload: &CompositeVoidRequest,
        access_token_response: Option<
            &MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse,
        >,
        metadata: &tonic::metadata::MetadataMap,
        extensions: &tonic::Extensions,
    ) -> Result<PaymentServiceVoidResponse, tonic::Status> {
        let void_payload =
            PaymentServiceVoidRequest::foreign_from((payload, access_token_response));

        let mut void_request = tonic::Request::new(void_payload);
        *void_request.metadata_mut() = metadata.clone();
        *void_request.extensions_mut() = extensions.clone();

        let void_response = self.payment_service.void(void_request).await?.into_inner();

        Ok(void_response)
    }

    async fn process_composite_void(
        &self,
        request: tonic::Request<CompositeVoidRequest>,
    ) -> Result<tonic::Response<CompositeVoidResponse>, tonic::Status> {
        let (metadata, extensions, payload) = request.into_parts();

        let connector =
            connector_from_composite_authorize_metadata(&metadata).map_err(|err| *err)?;
        let access_token_response = self
            .create_server_authentication_token(&connector, &payload, &metadata, &extensions)
            .await?;
        let void_response = self
            .void(
                &payload,
                access_token_response.as_ref(),
                &metadata,
                &extensions,
            )
            .await?;

        Ok(tonic::Response::new(CompositeVoidResponse {
            access_token_response,
            void_response: Some(void_response),
        }))
    }

    async fn capture(
        &self,
        payload: &CompositeCaptureRequest,
        access_token_response: Option<
            &MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse,
        >,
        metadata: &tonic::metadata::MetadataMap,
        extensions: &tonic::Extensions,
    ) -> Result<PaymentServiceCaptureResponse, tonic::Status> {
        let capture_payload =
            PaymentServiceCaptureRequest::foreign_from((payload, access_token_response));

        let mut capture_request = tonic::Request::new(capture_payload);
        *capture_request.metadata_mut() = metadata.clone();
        *capture_request.extensions_mut() = extensions.clone();

        let capture_response = self
            .payment_service
            .capture(capture_request)
            .await?
            .into_inner();

        Ok(capture_response)
    }

    async fn process_composite_capture(
        &self,
        request: tonic::Request<CompositeCaptureRequest>,
    ) -> Result<tonic::Response<CompositeCaptureResponse>, tonic::Status> {
        let (metadata, extensions, payload) = request.into_parts();

        let connector =
            connector_from_composite_authorize_metadata(&metadata).map_err(|err| *err)?;
        let access_token_response = self
            .create_server_authentication_token(&connector, &payload, &metadata, &extensions)
            .await?;
        let capture_response = self
            .capture(
                &payload,
                access_token_response.as_ref(),
                &metadata,
                &extensions,
            )
            .await?;

        Ok(tonic::Response::new(CompositeCaptureResponse {
            access_token_response,
            capture_response: Some(capture_response),
        }))
    }
}

#[tonic::async_trait]
impl<P, M, C, R, A> CompositePaymentService for Payments<P, M, C, R, A>
where
    P: PaymentService + Clone + Send + Sync + 'static,
    M: MerchantAuthenticationService + Clone + Send + Sync + 'static,
    C: CustomerService + Clone + Send + Sync + 'static,
    R: RefundService + Clone + Send + Sync + 'static,
    A: PaymentMethodAuthenticationService + Clone + Send + Sync + 'static,
{
    async fn authorize(
        &self,
        request: tonic::Request<CompositeAuthorizeRequest>,
    ) -> Result<tonic::Response<CompositeAuthorizeResponse>, tonic::Status> {
        self.process_composite_authorize(request).await
    }

    async fn get(
        &self,
        request: tonic::Request<CompositeGetRequest>,
    ) -> Result<tonic::Response<CompositeGetResponse>, tonic::Status> {
        self.process_composite_get(request).await
    }

    async fn refund(
        &self,
        request: tonic::Request<CompositeRefundRequest>,
    ) -> Result<tonic::Response<CompositeRefundResponse>, tonic::Status> {
        self.process_composite_refund(request).await
    }

    async fn void(
        &self,
        request: tonic::Request<CompositeVoidRequest>,
    ) -> Result<tonic::Response<CompositeVoidResponse>, tonic::Status> {
        self.process_composite_void(request).await
    }

    async fn capture(
        &self,
        request: tonic::Request<CompositeCaptureRequest>,
    ) -> Result<tonic::Response<CompositeCaptureResponse>, tonic::Status> {
        self.process_composite_capture(request).await
    }
}

#[tonic::async_trait]
impl<P, M, C, R, A> CompositeRefundService for Payments<P, M, C, R, A>
where
    P: PaymentService + Clone + Send + Sync + 'static,
    M: MerchantAuthenticationService + Clone + Send + Sync + 'static,
    C: CustomerService + Clone + Send + Sync + 'static,
    R: RefundService + Clone + Send + Sync + 'static,
    A: PaymentMethodAuthenticationService + Clone + Send + Sync + 'static,
{
    async fn get(
        &self,
        request: tonic::Request<CompositeRefundGetRequest>,
    ) -> Result<tonic::Response<CompositeRefundGetResponse>, tonic::Status> {
        self.process_composite_refund_get(request).await
    }
}
