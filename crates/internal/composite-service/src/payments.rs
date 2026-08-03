use connector_integration::types::ConnectorData;
use domain_types::{
    connector_types::{ConnectorEnum, ConnectorVariant, ServerAuthenticationTokenResponseData},
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
    CompositeCaptureResponse, CompositeGetRequest, CompositeGetResponse,
    CompositePreAuthenticateRequest, CompositePreAuthenticateResponse, CompositeRefundGetRequest,
    CompositeRefundGetResponse, CompositeRefundRequest, CompositeRefundResponse, CompositeStatus,
    CompositeVoidRequest, CompositeVoidResponse, ConnectorState, CustomerServiceCreateResponse,
    CustomerServiceGetRequest, MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest,
    MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse,
    MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest,
    MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse, PaymentMethod,
    PaymentMethodAuthenticationServiceAuthenticateRequest,
    PaymentMethodAuthenticationServiceAuthenticateResponse,
    PaymentMethodAuthenticationServicePostAuthenticateRequest,
    PaymentMethodAuthenticationServicePostAuthenticateResponse,
    PaymentMethodAuthenticationServicePreAuthenticateRequest,
    PaymentMethodAuthenticationServicePreAuthenticateResponse, PaymentServiceAuthorizeRequest,
    PaymentServiceAuthorizeResponse, PaymentServiceCaptureRequest, PaymentServiceCaptureResponse,
    PaymentServiceCreateOrderRequest, PaymentServiceCreateOrderResponse, PaymentServiceGetResponse,
    PaymentServiceRefundRequest, PaymentServiceVoidRequest, PaymentServiceVoidResponse,
    RefundResponse, RefundServiceGetRequest,
};
use interfaces::connector_types::AuthenticationStep;

use crate::transformers::{ForeignFrom, ForeignTryFrom};
use crate::utils::{
    connector_from_composite_authorize_metadata, is_failure_payment_status,
    is_terminal_payment_status,
};

/// Decoded CRes (Challenge Response) from 3DS challenge completion.
/// Trait for abstracting access to common fields needed for access token creation.
pub trait CompositeAccessTokenRequest {
    fn payment_method(&self) -> Option<PaymentMethod>;
    fn state(&self) -> Option<&ConnectorState>;
    fn build_access_token_request(
        &self,
        connector: &ConnectorVariant,
    ) -> MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest;
}

/// Trait for abstracting access to common fields needed for session token creation.
pub trait CompositeSessionTokenRequest {
    fn build_session_token_request(
        &self,
        connector: &ConnectorEnum,
    ) -> MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest;
    fn has_session_token(&self) -> bool;
}

/// Trait for abstracting request construction for composite pre-authenticate flows.
pub trait CompositePreAuthenticatePayload {
    fn build_pre_authenticate_request(
        &self,
        access_token_response: Option<
            &MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse,
        >,
    ) -> PaymentMethodAuthenticationServicePreAuthenticateRequest;
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
        connector: &ConnectorVariant,
    ) -> MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest {
        MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest::foreign_from((
            self, connector,
        ))
    }
}

impl CompositeAccessTokenRequest for CompositePreAuthenticateRequest {
    fn payment_method(&self) -> Option<PaymentMethod> {
        self.payment_method.clone()
    }

    fn state(&self) -> Option<&ConnectorState> {
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

impl CompositeSessionTokenRequest for CompositeAuthorizeRequest {
    fn build_session_token_request(
        &self,
        connector: &ConnectorEnum,
    ) -> MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest {
        MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest::foreign_from((
            self, connector,
        ))
    }

    fn has_session_token(&self) -> bool {
        self.session_token.is_some()
    }
}

impl CompositePreAuthenticatePayload for CompositeAuthorizeRequest {
    fn build_pre_authenticate_request(
        &self,
        access_token_response: Option<
            &MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse,
        >,
    ) -> PaymentMethodAuthenticationServicePreAuthenticateRequest {
        PaymentMethodAuthenticationServicePreAuthenticateRequest::foreign_from((
            self,
            access_token_response,
        ))
    }
}

impl CompositePreAuthenticatePayload for CompositePreAuthenticateRequest {
    fn build_pre_authenticate_request(
        &self,
        access_token_response: Option<
            &MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse,
        >,
    ) -> PaymentMethodAuthenticationServicePreAuthenticateRequest {
        PaymentMethodAuthenticationServicePreAuthenticateRequest::foreign_from((
            self,
            access_token_response,
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
        connector: &ConnectorVariant,
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
        connector: &ConnectorVariant,
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
        connector: &ConnectorVariant,
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
        connector: &ConnectorVariant,
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
        connector: &ConnectorVariant,
    ) -> MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest {
        MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest::foreign_from((
            self, connector,
        ))
    }
}

impl CompositeAccessTokenRequest
    for grpc_api_types::payments::CompositeVerifyRedirectResponseRequest
{
    fn payment_method(&self) -> Option<PaymentMethod> {
        self.payment_method.clone()
    }

    fn state(&self) -> Option<&ConnectorState> {
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

impl CompositeSessionTokenRequest
    for grpc_api_types::payments::CompositeVerifyRedirectResponseRequest
{
    fn build_session_token_request(
        &self,
        connector: &ConnectorEnum,
    ) -> MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest {
        MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest::foreign_from((
            self, connector,
        ))
    }

    fn has_session_token(&self) -> bool {
        self.session_token.is_some()
    }
}

/// Holds the mutable state accumulated during composite authorize flow execution.
#[derive(Default)]
struct AuthorizeCompositeState {
    pre_auth_response_opt: Option<PaymentMethodAuthenticationServicePreAuthenticateResponse>,
    authn_response_opt: Option<PaymentMethodAuthenticationServiceAuthenticateResponse>,
    post_authn_response_opt: Option<PaymentMethodAuthenticationServicePostAuthenticateResponse>,
    authorize_response_opt: Option<PaymentServiceAuthorizeResponse>,
    completed_step: Option<AuthenticationStep>,
}

/// Outcome of a connector-side customer lookup. `Found` carries a create-shaped
/// response so the caller can reuse the ID without hitting CREATE; `NotFound`
/// covers both "connector returned no match" and "lookup call failed" — the
/// caller falls through to CREATE in either case.
enum ConnectorCustomerLookup {
    Found(Box<CustomerServiceCreateResponse>),
    NotFound,
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
                let access_token_payload =
                    payload.build_access_token_request(&ConnectorVariant::Payment(*connector));
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

    async fn create_server_session_authentication_token<Req: CompositeSessionTokenRequest>(
        &self,
        connector: &ConnectorEnum,
        payload: &Req,
        metadata: &tonic::metadata::MetadataMap,
        extensions: &tonic::Extensions,
    ) -> Result<
        Option<MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse>,
        tonic::Status,
    > {
        let connector_data = ConnectorData::<domain_types::payment_method_data::DefaultPCIHolder>::get_connector_by_name(connector);
        let should_do_session_token = connector_data.connector.should_do_session_token();

        let should_create_session_token = !payload.has_session_token() && should_do_session_token;

        let session_token_response = match should_create_session_token {
            true => {
                let session_token_payload = payload.build_session_token_request(connector);
                let mut session_token_request = tonic::Request::new(session_token_payload);
                *session_token_request.metadata_mut() = metadata.clone();
                *session_token_request.extensions_mut() = extensions.clone();

                let session_token_response = self
                    .merchant_authentication_service
                    .create_server_session_authentication_token(session_token_request)
                    .await?
                    .into_inner();

                Some(session_token_response)
            }
            false => None,
        };

        Ok(session_token_response)
    }

    /// For connectors that support lookup (e.g. Glomopay, which 4xxs on
    /// duplicate email), issue a GET to see if the customer already exists.
    /// Returns `Ok(Found(response))` when the connector confirms an existing
    /// customer with an ID, `Ok(NotFound)` when the lookup succeeded but
    /// returned no matching customer, and `Err(status)` on transient
    /// failures (network / connector / auth). Errors are propagated rather
    /// than treated as `NotFound` — swallowing them would silently fall
    /// through to CREATE and produce duplicate customers on the connector.
    async fn get_connector_customer(
        &self,
        payload: &CompositeAuthorizeRequest,
        metadata: &tonic::metadata::MetadataMap,
        extensions: &tonic::Extensions,
    ) -> Result<ConnectorCustomerLookup, tonic::Status> {
        let get_payload = CustomerServiceGetRequest::foreign_from(payload);
        let mut get_request = tonic::Request::new(get_payload);
        *get_request.metadata_mut() = metadata.clone();
        *get_request.extensions_mut() = extensions.clone();

        let get_response = self.customer_service.get(get_request).await?.into_inner();

        // Consume the explicit lookup_status enum rather than inferring
        // found-vs-not-found from field presence. A malformed or partially
        // populated response no longer silently routes to a duplicate CREATE.
        match grpc_api_types::payments::CustomerLookupStatus::try_from(get_response.lookup_status) {
            Ok(grpc_api_types::payments::CustomerLookupStatus::Found) => {
                CustomerServiceCreateResponse::foreign_try_from(get_response)
                    .map(|r| ConnectorCustomerLookup::Found(Box::new(r)))
                    .map_err(|_| {
                        tonic::Status::internal(
                            "CustomerServiceGetResponse.lookup_status was CustomerFound \
                             but the response did not include a connector_customer_id — \
                             refusing to fall through to CREATE and risk duplicates",
                        )
                    })
            }
            Ok(grpc_api_types::payments::CustomerLookupStatus::NotFound) => {
                Ok(ConnectorCustomerLookup::NotFound)
            }
            Ok(grpc_api_types::payments::CustomerLookupStatus::Unspecified) => {
                Err(tonic::Status::internal(
                    "CustomerServiceGetResponse.lookup_status was unspecified — \
                     connector did not signal found/not-found explicitly",
                ))
            }
            Err(_) => Err(tonic::Status::internal(
                "CustomerServiceGetResponse.lookup_status contained an unknown value",
            )),
        }
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

        let create_customer_response = if should_create_connector_customer {
            // Try lookup first for connectors that support get-or-create
            // semantics (e.g. Glomopay). If the customer already exists on
            // the connector, reuse that ID instead of hitting CREATE.
            let existing_customer_response = if connector_data
                .connector
                .should_get_connector_customer()
            {
                match self
                    .get_connector_customer(payload, metadata, extensions)
                    .await?
                {
                    ConnectorCustomerLookup::Found(customer_response) => Some(customer_response),
                    ConnectorCustomerLookup::NotFound => None,
                }
            } else {
                None
            };

            let customer_response = match existing_customer_response {
                Some(customer_response) => customer_response,
                None => {
                    let create_customer_payload =
                        grpc_api_types::payments::CustomerServiceCreateRequest::foreign_from(
                            payload,
                        );
                    let mut create_customer_request = tonic::Request::new(create_customer_payload);
                    *create_customer_request.metadata_mut() = metadata.clone();
                    *create_customer_request.extensions_mut() = extensions.clone();

                    Box::new(
                        self.customer_service
                            .create(create_customer_request)
                            .await?
                            .into_inner(),
                    )
                }
            };
            Some(*customer_response)
        } else {
            None
        };

        Ok(create_customer_response)
    }

    async fn create_order(
        &self,
        connector: &ConnectorEnum,
        payload: &CompositeAuthorizeRequest,
        create_customer_response: Option<&CustomerServiceCreateResponse>,
        metadata: &tonic::metadata::MetadataMap,
        extensions: &tonic::Extensions,
    ) -> Result<Option<PaymentServiceCreateOrderResponse>, tonic::Status> {
        let connector_data =
            ConnectorData::<domain_types::payment_method_data::DefaultPCIHolder>::get_connector_by_name(
                connector,
            );

        let should_execute_create_order = connector_data.connector.should_do_order_create();

        let create_order_response = match should_execute_create_order {
            true => {
                // Build PaymentServiceCreateOrderRequest from CompositeAuthorizeRequest,
                // threading in the freshly-created connector customer id (if any) so
                // connectors like Glomopay whose CreateOrder API requires customer_id
                // work correctly on first-time-customer flows.
                let create_order_payload = PaymentServiceCreateOrderRequest::foreign_from((
                    payload,
                    create_customer_response,
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

    #[allow(clippy::too_many_arguments)]
    async fn authorize(
        &self,
        payload: &CompositeAuthorizeRequest,
        access_token_response: Option<
            &MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse,
        >,
        session_token_response: Option<
            &MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse,
        >,
        create_customer_response: Option<&CustomerServiceCreateResponse>,
        create_order_response: Option<&PaymentServiceCreateOrderResponse>,
        authenticate_response: Option<&PaymentMethodAuthenticationServiceAuthenticateResponse>,
        post_authenticate_response: Option<
            &PaymentMethodAuthenticationServicePostAuthenticateResponse,
        >,
        metadata: &tonic::metadata::MetadataMap,
        extensions: &tonic::Extensions,
    ) -> Result<PaymentServiceAuthorizeResponse, tonic::Status> {
        let authorize_payload = PaymentServiceAuthorizeRequest::foreign_from((
            payload,
            access_token_response,
            session_token_response,
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

    async fn pre_authenticate<Req>(
        &self,
        payload: &Req,
        access_token_response: Option<
            &MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse,
        >,
        metadata: &tonic::metadata::MetadataMap,
        extensions: &tonic::Extensions,
    ) -> Result<PaymentMethodAuthenticationServicePreAuthenticateResponse, tonic::Status>
    where
        Req: CompositePreAuthenticatePayload,
    {
        let pre_auth_payload = payload.build_pre_authenticate_request(access_token_response);
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
        pre_auth_response: Option<&PaymentMethodAuthenticationServicePreAuthenticateResponse>,
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
        auth_response: Option<&PaymentMethodAuthenticationServiceAuthenticateResponse>,
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

    /// Extracts and validates authentication type from the request payload.
    fn get_auth_type(
        &self,
        payload: &CompositeAuthorizeRequest,
    ) -> Result<common_enums::AuthenticationType, tonic::Status> {
        common_enums::AuthenticationType::foreign_try_from(
            grpc_api_types::payments::AuthenticationType::try_from(payload.auth_type)
                .unwrap_or_default(),
        )
        .map_err(|err| tonic::Status::invalid_argument(format!("invalid auth_type: {err}")))
    }

    /// Extracts and validates payment method from the request payload.
    fn get_payment_method(
        &self,
        payload: &CompositeAuthorizeRequest,
    ) -> Result<common_enums::PaymentMethod, tonic::Status> {
        payload
            .payment_method()
            .map(common_enums::PaymentMethod::foreign_try_from)
            .transpose()
            .map_err(|err| {
                tonic::Status::invalid_argument(format!("invalid payment_method: {err}"))
            })?
            .ok_or_else(|| tonic::Status::invalid_argument("missing payment_method"))
    }

    /// Derives redirect state from the proto redirection_response field.
    fn get_redirect_state(
        &self,
        payload: &CompositeAuthorizeRequest,
    ) -> interfaces::connector_types::RedirectState {
        match payload.redirection_response.as_ref() {
            None => interfaces::connector_types::RedirectState::InitialRequest,
            Some(r) => {
                if r.params.as_ref().map(|p| !p.is_empty()).unwrap_or(false) {
                    interfaces::connector_types::RedirectState::RedirectWithParams
                } else {
                    interfaces::connector_types::RedirectState::RedirectWithoutParams
                }
            }
        }
    }

    async fn process_composite_authorize(
        &self,
        request: tonic::Request<CompositeAuthorizeRequest>,
    ) -> Result<tonic::Response<CompositeAuthorizeResponse>, tonic::Status> {
        let (metadata, extensions, payload) = request.into_parts();

        let connector =
            connector_from_composite_authorize_metadata(&metadata).map_err(|err| *err)?;

        let access_token_response = self
            .create_server_authentication_token(&connector, &payload, &metadata, &extensions)
            .await?;
        let session_token_response = self
            .create_server_session_authentication_token(
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
                create_customer_response.as_ref(),
                &metadata,
                &extensions,
            )
            .await?;

        // Extract flow parameters from payload
        let auth_type = self.get_auth_type(&payload)?;
        let payment_method = self.get_payment_method(&payload)?;
        let connector_data = ConnectorData::<domain_types::payment_method_data::DefaultPCIHolder>::get_connector_by_name(&connector);
        let redirect_state = self.get_redirect_state(&payload);

        let mut state = AuthorizeCompositeState::default();

        // Authentication loop - connector controls flow via next_authentication_step
        loop {
            let next_step = connector_data.connector.next_authentication_step(
                auth_type,
                payment_method,
                redirect_state,
                state.completed_step,
            );

            match next_step {
                AuthenticationStep::PreAuthenticate => {
                    state.pre_auth_response_opt = Some(
                        self.pre_authenticate(
                            &payload,
                            access_token_response.as_ref(),
                            &metadata,
                            &extensions,
                        )
                        .await?,
                    );
                    state.completed_step = Some(AuthenticationStep::PreAuthenticate);

                    if state
                        .pre_auth_response_opt
                        .as_ref()
                        .map(|r| {
                            r.redirection_data.is_some() || is_failure_payment_status(r.status)
                        })
                        .unwrap_or(false)
                    {
                        break;
                    }
                }

                AuthenticationStep::Authenticate => {
                    state.authn_response_opt = Some(
                        self.authenticate(
                            &payload,
                            state.pre_auth_response_opt.as_ref(),
                            &metadata,
                            &extensions,
                        )
                        .await?,
                    );
                    state.completed_step = Some(AuthenticationStep::Authenticate);

                    if state
                        .authn_response_opt
                        .as_ref()
                        .map(|r| {
                            r.redirection_data.is_some() || is_terminal_payment_status(r.status)
                        })
                        .unwrap_or(false)
                    {
                        break;
                    }
                }

                AuthenticationStep::PostAuthenticate => {
                    state.post_authn_response_opt = Some(
                        self.post_authenticate(
                            &payload,
                            state.authn_response_opt.as_ref(),
                            &metadata,
                            &extensions,
                        )
                        .await?,
                    );
                    state.completed_step = Some(AuthenticationStep::PostAuthenticate);
                }

                AuthenticationStep::Authorize => {
                    state.authorize_response_opt = Some(
                        self.authorize(
                            &payload,
                            access_token_response.as_ref(),
                            session_token_response.as_ref(),
                            create_customer_response.as_ref(),
                            create_order_response.as_ref(),
                            state.authn_response_opt.as_ref(),
                            state.post_authn_response_opt.as_ref(),
                            &metadata,
                            &extensions,
                        )
                        .await?,
                    );
                    break;
                }
            }
        }

        // Response construction - check if redirect occurred
        let has_redirection = state
            .pre_auth_response_opt
            .as_ref()
            .map(|r| r.redirection_data.is_some())
            .unwrap_or(false)
            || state
                .authn_response_opt
                .as_ref()
                .map(|r| r.redirection_data.is_some())
                .unwrap_or(false);

        let composite_status = if has_redirection {
            CompositeStatus::RedirectRequired
        } else {
            CompositeStatus::Completed
        };

        Ok(tonic::Response::new(CompositeAuthorizeResponse {
            access_token_response,
            session_token_response,
            create_customer_response,
            create_order_response,
            pre_authenticate_response: state.pre_auth_response_opt,
            authenticate_response: state.authn_response_opt,
            post_authenticate_response: state.post_authn_response_opt,
            authorize_response: state.authorize_response_opt,
            composite_status: composite_status.into(),
        }))
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

    async fn process_pre_authenticate(
        &self,
        request: tonic::Request<CompositePreAuthenticateRequest>,
    ) -> Result<tonic::Response<CompositePreAuthenticateResponse>, tonic::Status> {
        let (metadata, extensions, payload) = request.into_parts();

        let connector =
            connector_from_composite_authorize_metadata(&metadata).map_err(|err| *err)?;
        let access_token_response = self
            .create_server_authentication_token(&connector, &payload, &metadata, &extensions)
            .await?;
        let pre_authenticate_response = self
            .pre_authenticate(
                &payload,
                access_token_response.as_ref(),
                &metadata,
                &extensions,
            )
            .await?;

        Ok(tonic::Response::new(CompositePreAuthenticateResponse {
            pre_authenticate_response: Some(pre_authenticate_response),
            access_token_response,
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
    /// Orchestrates access_token + session_token + authorize for post-redirect scenarios.
    async fn authorize_post_redirect(
        &self,
        connector: &ConnectorEnum,
        payload: &grpc_api_types::payments::CompositeVerifyRedirectResponseRequest,
        verify_response: &grpc_api_types::payments::PaymentServiceVerifyRedirectResponseResponse,
        metadata: &tonic::metadata::MetadataMap,
        extensions: &tonic::Extensions,
    ) -> Result<
        (
            Option<MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
            Option<MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse>,
            PaymentServiceAuthorizeResponse,
        ),
        tonic::Status,
    > {
        let access_token_response = self
            .create_server_authentication_token(connector, payload, metadata, extensions)
            .await?;

        let session_token_response = self
            .create_server_session_authentication_token(connector, payload, metadata, extensions)
            .await?;

        let authorize_payload = PaymentServiceAuthorizeRequest::foreign_from((
            payload,
            verify_response,
            access_token_response.as_ref(),
            session_token_response.as_ref(),
        ));

        let mut authorize_request = tonic::Request::new(authorize_payload);
        *authorize_request.metadata_mut() = metadata.clone();
        *authorize_request.extensions_mut() = extensions.clone();

        let authorize_response = self
            .payment_service
            .authorize(authorize_request)
            .await?
            .into_inner();

        Ok((
            access_token_response,
            session_token_response,
            authorize_response,
        ))
    }

    /// Helper method to call VerifyRedirectResponse service
    async fn verify_redirect_response(
        &self,
        payload: &grpc_api_types::payments::CompositeVerifyRedirectResponseRequest,
        metadata: &tonic::metadata::MetadataMap,
        extensions: &tonic::Extensions,
    ) -> Result<grpc_api_types::payments::PaymentServiceVerifyRedirectResponseResponse, tonic::Status>
    {
        // Build verify request from composite request
        let verify_payload =
            grpc_api_types::payments::PaymentServiceVerifyRedirectResponseRequest {
                merchant_order_id: payload.merchant_order_id.clone(),
                request_details: payload.request_details.clone(),
                redirect_response_secrets: payload.redirect_response_secrets.clone(),
            };

        // Create tonic request with metadata
        let mut verify_request = tonic::Request::new(verify_payload);
        *verify_request.metadata_mut() = metadata.clone();
        *verify_request.extensions_mut() = extensions.clone();

        // Call service and return
        let verify_response = self
            .payment_service
            .verify_redirect_response(verify_request)
            .await?
            .into_inner();

        Ok(verify_response)
    }

    /// Main composite flow: verify redirect response, then conditionally authorize
    async fn process_composite_verify_redirect_response(
        &self,
        request: tonic::Request<grpc_api_types::payments::CompositeVerifyRedirectResponseRequest>,
    ) -> Result<
        tonic::Response<grpc_api_types::payments::CompositeVerifyRedirectResponseResponse>,
        tonic::Status,
    > {
        let (metadata, extensions, payload) = request.into_parts();
        let connector =
            connector_from_composite_authorize_metadata(&metadata).map_err(|err| *err)?;

        let verify_response = self
            .verify_redirect_response(&payload, &metadata, &extensions)
            .await?;

        let connector_data = ConnectorData::<
            domain_types::payment_method_data::DefaultPCIHolder,
        >::get_connector_by_name(&connector);

        let (access_token_response, session_token_response, authorize_response) =
            if connector_data.connector.requires_authorize_post_redirect() {
                let (access_token, session_token, authorize) = self
                    .authorize_post_redirect(
                        &connector,
                        &payload,
                        &verify_response,
                        &metadata,
                        &extensions,
                    )
                    .await?;
                (Some(access_token), Some(session_token), Some(authorize))
            } else {
                (None, None, None)
            };

        Ok(tonic::Response::new(
            grpc_api_types::payments::CompositeVerifyRedirectResponseResponse {
                verify_redirect_response: Some(verify_response),
                access_token_response: access_token_response.flatten(),
                session_token_response: session_token_response.flatten(),
                authorize_response,
            },
        ))
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
        Box::pin(self.process_composite_authorize(request)).await
    }

    async fn pre_authenticate(
        &self,
        request: tonic::Request<CompositePreAuthenticateRequest>,
    ) -> Result<tonic::Response<CompositePreAuthenticateResponse>, tonic::Status> {
        self.process_pre_authenticate(request).await
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

    async fn verify_redirect_response(
        &self,
        request: tonic::Request<grpc_api_types::payments::CompositeVerifyRedirectResponseRequest>,
    ) -> Result<
        tonic::Response<grpc_api_types::payments::CompositeVerifyRedirectResponseResponse>,
        tonic::Status,
    > {
        self.process_composite_verify_redirect_response(request)
            .await
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
