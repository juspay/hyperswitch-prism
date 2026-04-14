use std::{fmt::Debug, sync::Arc};

use crate::{
    implement_connector_operation,
    request::RequestData,
    utils::{self, get_config_from_request, grpc_logging_wrapper},
};
use common_enums;
use common_utils::{events::FlowName, lineage, metadata::MaskedMetadata, SecretSerdeValue};
use connector_integration::types::ConnectorData;
use domain_types::connector_types::ConnectorEnum;
use domain_types::{
    connector_flow::{
        Authenticate, Authorize, Capture, ClientAuthenticationToken, CreateConnectorCustomer,
        CreateOrder, IncrementalAuthorization, MandateRevoke, PSync, PaymentMethodToken,
        PostAuthenticate, PreAuthenticate, Refund, RepeatPayment, ServerAuthenticationToken,
        ServerSessionAuthenticationToken, SetupMandate, Void, VoidPC,
    },
    connector_types::{
        ClientAuthenticationTokenRequestData, ConnectorCustomerData, ConnectorCustomerResponse,
        ConnectorResponseHeaders, MandateRevokeRequestData, MandateRevokeResponseData,
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData,
        PaymentMethodTokenResponse, PaymentMethodTokenizationData, PaymentVoidData,
        PaymentsAuthenticateData, PaymentsAuthorizeData, PaymentsCancelPostCaptureData,
        PaymentsCaptureData, PaymentsIncrementalAuthorizationData, PaymentsPostAuthenticateData,
        PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSyncData,
        RawConnectorRequestResponse, RefundFlowData, RefundsData, RefundsResponseData,
        RepeatPaymentData, ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData, ServerSessionAuthenticationTokenRequestData,
        ServerSessionAuthenticationTokenResponseData, SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::{DefaultPCIHolder, PaymentMethodDataTypes, VaultTokenHolder},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    types::{
        generate_create_order_response, generate_payment_authenticate_response,
        generate_payment_capture_response, generate_payment_incremental_authorization_response,
        generate_payment_post_authenticate_response, generate_payment_pre_authenticate_response,
        generate_payment_sdk_session_token_response, generate_payment_sync_response,
        generate_payment_void_post_capture_response, generate_payment_void_response,
        generate_refund_response, generate_repeat_payment_response,
        generate_setup_mandate_response, proxied_authorize_to_base,
        proxied_setup_recurring_to_base, tokenized_authorize_to_base,
        tokenized_setup_recurring_to_base,
    },
    utils::ForeignTryFrom,
};
use external_services::service::EventProcessingParams;
use grpc_api_types::payments::{
    customer_service_server::CustomerService,
    merchant_authentication_service_server::MerchantAuthenticationService, payment_method,
    payment_method_authentication_service_server::PaymentMethodAuthenticationService,
    payment_method_service_server::PaymentMethodService, payment_service_server::PaymentService,
    recurring_payment_service_server::RecurringPaymentService,
    MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest,
    MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse,
    MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest,
    MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse,
    MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest,
    MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse,
    PaymentMethodAuthenticationServiceAuthenticateRequest,
    PaymentMethodAuthenticationServiceAuthenticateResponse,
    PaymentMethodAuthenticationServicePostAuthenticateRequest,
    PaymentMethodAuthenticationServicePostAuthenticateResponse,
    PaymentMethodAuthenticationServicePreAuthenticateRequest,
    PaymentMethodAuthenticationServicePreAuthenticateResponse, PaymentMethodServiceTokenizeRequest,
    PaymentMethodServiceTokenizeResponse, PaymentServiceAuthorizeRequest,
    PaymentServiceAuthorizeResponse, PaymentServiceCaptureRequest, PaymentServiceCaptureResponse,
    PaymentServiceCreateOrderRequest, PaymentServiceCreateOrderResponse, PaymentServiceGetRequest,
    PaymentServiceGetResponse, PaymentServiceIncrementalAuthorizationRequest,
    PaymentServiceIncrementalAuthorizationResponse, PaymentServiceProxyAuthorizeRequest,
    PaymentServiceProxySetupRecurringRequest, PaymentServiceRefundRequest,
    PaymentServiceReverseRequest, PaymentServiceReverseResponse,
    PaymentServiceSetupRecurringRequest, PaymentServiceSetupRecurringResponse,
    PaymentServiceTokenAuthorizeRequest, PaymentServiceTokenSetupRecurringRequest,
    PaymentServiceVerifyRedirectResponseRequest, PaymentServiceVerifyRedirectResponseResponse,
    PaymentServiceVoidRequest, PaymentServiceVoidResponse, PayoutMethodEligibilityRequest,
    PayoutMethodEligibilityResponse, RecurringPaymentServiceChargeRequest,
    RecurringPaymentServiceChargeResponse, RecurringPaymentServiceRevokeRequest,
    RecurringPaymentServiceRevokeResponse, RefundResponse,
};
use hyperswitch_masking::ExposeInterface;
use injector::{TokenData, VaultConnectors};
use interfaces::{
    connector_integration_v2::BoxedConnectorIntegrationV2,
    verification::ConnectorSourceVerificationSecrets,
};
use tracing::info;
use ucs_env::{
    configs::Config,
    error::{IntoGrpcStatus, ResultExtGrpc},
};

#[derive(Debug, Clone)]
struct EventParams<'a> {
    _connector_name: &'a str,
    _service_name: &'a str,
    service_type: &'a str,
    request_id: &'a str,
    lineage_ids: &'a lineage::LineageIds<'a>,
    reference_id: &'a Option<String>,
    resource_id: &'a Option<String>,
    shadow_mode: bool,
}

/// Helper function for converting CardDetails to TokenData with structured types
#[derive(Debug, serde::Serialize)]
struct CardTokenData {
    card_number: String,
    cvv: String,
    exp_month: String,
    exp_year: String,
}

trait ToTokenData {
    fn to_token_data(&self) -> TokenData;
    fn to_token_data_with_vault(&self, vault_connector: VaultConnectors) -> TokenData;
}

impl ToTokenData for grpc_api_types::payments::CardDetails {
    fn to_token_data(&self) -> TokenData {
        self.to_token_data_with_vault(VaultConnectors::VGS)
    }

    fn to_token_data_with_vault(&self, vault_connector: VaultConnectors) -> TokenData {
        let card_data = CardTokenData {
            card_number: self
                .card_number
                .as_ref()
                .map(|cn| cn.get_card_no())
                .unwrap_or_default(),
            cvv: self
                .card_cvc
                .as_ref()
                .map(|cvc| cvc.clone().expose().to_string())
                .unwrap_or_default(),
            exp_month: self
                .card_exp_month
                .as_ref()
                .map(|em| em.clone().expose().to_string())
                .unwrap_or_default(),
            exp_year: self
                .card_exp_year
                .as_ref()
                .map(|ey| ey.clone().expose().to_string())
                .unwrap_or_default(),
        };

        let card_json = serde_json::to_value(card_data).unwrap_or(serde_json::Value::Null);

        TokenData {
            specific_token_data: SecretSerdeValue::new(card_json),
            vault_connector,
        }
    }
}
// Helper trait for payment operations
trait PaymentOperationsInternal {
    async fn internal_void_payment(
        &self,
        request: RequestData<PaymentServiceVoidRequest>,
    ) -> Result<tonic::Response<PaymentServiceVoidResponse>, tonic::Status>;

    async fn internal_void_post_capture(
        &self,
        request: RequestData<PaymentServiceReverseRequest>,
    ) -> Result<tonic::Response<PaymentServiceReverseResponse>, tonic::Status>;

    async fn internal_refund(
        &self,
        request: RequestData<PaymentServiceRefundRequest>,
    ) -> Result<tonic::Response<RefundResponse>, tonic::Status>;

    async fn internal_payment_capture(
        &self,
        request: RequestData<PaymentServiceCaptureRequest>,
    ) -> Result<tonic::Response<PaymentServiceCaptureResponse>, tonic::Status>;

    async fn internal_incremental_authorization(
        &self,
        request: RequestData<PaymentServiceIncrementalAuthorizationRequest>,
    ) -> Result<tonic::Response<PaymentServiceIncrementalAuthorizationResponse>, tonic::Status>;

    async fn internal_create_order(
        &self,
        request: RequestData<PaymentServiceCreateOrderRequest>,
    ) -> Result<tonic::Response<PaymentServiceCreateOrderResponse>, tonic::Status>;
}

trait PaymentMethodAuthOperational {
    async fn internal_pre_authenticate(
        &self,
        request: RequestData<PaymentMethodAuthenticationServicePreAuthenticateRequest>,
    ) -> Result<
        tonic::Response<PaymentMethodAuthenticationServicePreAuthenticateResponse>,
        tonic::Status,
    >;

    async fn internal_authenticate(
        &self,
        request: RequestData<PaymentMethodAuthenticationServiceAuthenticateRequest>,
    ) -> Result<
        tonic::Response<PaymentMethodAuthenticationServiceAuthenticateResponse>,
        tonic::Status,
    >;

    async fn internal_post_authenticate(
        &self,
        request: RequestData<PaymentMethodAuthenticationServicePostAuthenticateRequest>,
    ) -> Result<
        tonic::Response<PaymentMethodAuthenticationServicePostAuthenticateResponse>,
        tonic::Status,
    >;
}

trait RecurringPaymentOperational {
    async fn internal_mandate_revoke(
        &self,
        request: RequestData<RecurringPaymentServiceRevokeRequest>,
    ) -> Result<tonic::Response<RecurringPaymentServiceRevokeResponse>, tonic::Status>;
}

trait MerchantAuthenticationOperational {
    async fn internal_sdk_session_token(
        &self,
        request: RequestData<MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest>,
    ) -> Result<
        tonic::Response<MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse>,
        tonic::Status,
    >;
}

#[derive(Clone)]
pub struct RecurringPayments;

#[derive(Clone)]
pub struct MerchantAuthentication;

#[derive(Clone)]
pub struct PaymentMethodAuthentication;

#[derive(Clone)]
pub struct PaymentMethod;

#[derive(Clone)]
pub struct Events;

#[derive(Clone)]
pub struct Payments {
    pub merchant_authentication_service: MerchantAuthentication,
    pub customer_service: Customer,
}

#[derive(Clone)]
pub struct Customer;

#[tonic::async_trait]
impl CustomerService for Customer {
    #[tracing::instrument(
        name = "create",
        fields(
            name = common_utils::consts::NAME,
            service_name = common_utils::consts::PAYMENT_SERVICE_NAME,
            service_method = FlowName::CreateConnectorCustomer.as_str(),
            request_body = tracing::field::Empty,
            response_body = tracing::field::Empty,
            error_message = tracing::field::Empty,
            merchant_id = tracing::field::Empty,
            gateway = tracing::field::Empty,
            request_id = tracing::field::Empty,
            status_code = tracing::field::Empty,
            message_ = "Golden Log Line (incoming)",
            response_time = tracing::field::Empty,
            tenant_id = tracing::field::Empty,
            flow = FlowName::CreateConnectorCustomer.as_str(),
            flow_specific_fields.status = tracing::field::Empty,
        ),
        skip(self, request)
    )]
    async fn create(
        &self,
        request: tonic::Request<grpc_api_types::payments::CustomerServiceCreateRequest>,
    ) -> Result<
        tonic::Response<grpc_api_types::payments::CustomerServiceCreateResponse>,
        tonic::Status,
    > {
        info!("CREATE_CONNECTOR_CUSTOMER_FLOW: initiated");
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "PaymentService".to_string());
        let config = get_config_from_request(&request)?;
        grpc_logging_wrapper(
            request,
            &service_name,
            config.clone(),
            FlowName::CreateConnectorCustomer,
            |request_data| {
                let service_name = service_name.clone();
                let config = config.clone();
                Box::pin(async move {
                    let payload = request_data.payload;
                    let metadata_payload = request_data.extracted_metadata;
                    let (connector, request_id, lineage_ids) = (
                        metadata_payload.connector,
                        metadata_payload.request_id,
                        metadata_payload.lineage_ids,
                    );
                    let connector_config = &metadata_payload.connector_config;

                    //get connector data
                    let connector_data: ConnectorData<DefaultPCIHolder> =
                        ConnectorData::get_connector_by_name(&connector);

                    // Get connector integration
                    let connector_integration: BoxedConnectorIntegrationV2<
                        '_,
                        CreateConnectorCustomer,
                        PaymentFlowData,
                        ConnectorCustomerData,
                        ConnectorCustomerResponse,
                    > = connector_data.connector.get_connector_integration_v2();

                    let connectors = utils::connectors_with_connector_config_overrides(
                        connector_config,
                        &config,
                    )
                    .into_grpc_status()?;

                    // Create common request data
                    let payment_flow_data = PaymentFlowData::foreign_try_from((
                        payload.clone(),
                        connectors,
                        &request_data.masked_metadata,
                    ))
                    .map_err(|e| e.into_grpc_status())?;

                    // Create connector customer request data directly
                    let connector_customer_request_data =
                        ConnectorCustomerData::foreign_try_from(payload.clone())
                            .into_grpc_status()?;

                    // Create router data for connector customer flow
                    let connector_customer_router_data = RouterDataV2::<
                        CreateConnectorCustomer,
                        PaymentFlowData,
                        ConnectorCustomerData,
                        ConnectorCustomerResponse,
                    > {
                        flow: std::marker::PhantomData,
                        resource_common_data: payment_flow_data.clone(),
                        connector_config: connector_config.clone(),
                        request: connector_customer_request_data.clone(),
                        response: Err(ErrorResponse::default()),
                    };

                    // Get API tag for CreateConnectorCustomer flow
                    let api_tag = config
                        .api_tags
                        .get_tag(FlowName::CreateConnectorCustomer, None);

                    // Create test context if test mode is enabled
                    let test_context =
                        config.test.create_test_context(&request_id).map_err(|e| {
                            tonic::Status::internal(format!("Test mode configuration error: {e}"))
                        })?;

                    // Execute connector processing
                    let external_event_params = EventProcessingParams {
                        connector_name: &connector.to_string(),
                        service_name: &service_name,
                        service_type: utils::service_type_str(&config.server.type_),
                        flow_name: FlowName::CreateConnectorCustomer,
                        event_config: &config.events,
                        request_id: &request_id,
                        lineage_ids: &lineage_ids,
                        reference_id: &metadata_payload.reference_id,
                        resource_id: &metadata_payload.resource_id,
                        shadow_mode: metadata_payload.shadow_mode,
                    };

                    let response = Box::pin(
                        external_services::service::execute_connector_processing_step(
                            &config.proxy,
                            connector_integration,
                            connector_customer_router_data,
                            None,
                            external_event_params,
                            None,
                            common_enums::CallConnectorAction::Trigger,
                            test_context,
                            api_tag,
                        ),
                    )
                    .await
                    .into_grpc_status()?;

                    // Generate response using the new function
                    let connector_customer_response =
                        domain_types::types::generate_create_connector_customer_response(response)
                            .map_err(|e| e.into_grpc_status())?;

                    Ok(tonic::Response::new(connector_customer_response))
                })
            },
        )
        .await
    }
}
impl Payments {
    #[allow(clippy::too_many_arguments)]
    async fn process_authorization_internal<
        T: PaymentMethodDataTypes
            + Default
            + Eq
            + Debug
            + Send
            + serde::Serialize
            + serde::de::DeserializeOwned
            + Clone
            + Sync
            + domain_types::types::CardConversionHelper<T>
            + 'static,
    >(
        &self,
        config: &Arc<Config>,
        payload: PaymentServiceAuthorizeRequest,
        connector: ConnectorEnum,
        connector_config: ConnectorSpecificConfig,
        metadata: &MaskedMetadata,
        metadata_payload: &utils::MetadataPayload,
        service_name: &str,
        request_id: &str,
        token_data: Option<TokenData>,
    ) -> Result<PaymentServiceAuthorizeResponse, tonic::Status> {
        //get connector data
        let connector_data = ConnectorData::get_connector_by_name(&connector);

        // Get connector integration
        let connector_integration: BoxedConnectorIntegrationV2<
            '_,
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        > = connector_data.connector.get_connector_integration_v2();

        let connectors = utils::get_resolved_connectors(
            config,
            &connector,
            &connector_config,
            metadata_payload.environment.as_deref(),
        )
        .map_err(|e| {
            tracing::error!("Failed to resolve connector overrides: {:?}", e);
            e.into_grpc_status()
        })?;

        // Create common request data
        let payment_flow_data =
            PaymentFlowData::foreign_try_from((payload.clone(), connectors, metadata))
                .into_grpc_status()?;

        // Create connector request data
        let payment_authorize_data =
            PaymentsAuthorizeData::foreign_try_from(payload.clone()).into_grpc_status()?;

        // Construct router data
        let router_data = RouterDataV2::<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        > {
            flow: std::marker::PhantomData,
            resource_common_data: payment_flow_data.clone(),
            connector_config: connector_config.clone(),
            request: payment_authorize_data,
            response: Err(ErrorResponse::default()),
        };

        // Get API tag for the current flow with payment method type from domain layer
        let api_tag = config
            .api_tags
            .get_tag(FlowName::Authorize, router_data.request.payment_method_type);

        // Create test context if test mode is enabled
        let test_context = config
            .test
            .create_test_context(request_id)
            .map_err(|e| tonic::Status::internal(format!("Test mode configuration error: {e}")))?;

        // Execute connector processing
        let event_params = EventProcessingParams {
            connector_name: &connector.to_string(),
            service_name,
            service_type: utils::service_type_str(&config.server.type_),
            flow_name: FlowName::Authorize,
            event_config: &config.events,
            request_id,
            lineage_ids: &metadata_payload.lineage_ids,
            reference_id: &metadata_payload.reference_id,
            resource_id: &metadata_payload.resource_id,
            shadow_mode: metadata_payload.shadow_mode,
        };

        // Execute connector processing - ONLY the authorize call
        let response = external_services::service::execute_connector_processing_step(
            &config.proxy,
            connector_integration,
            router_data,
            None,
            event_params,
            token_data,
            common_enums::CallConnectorAction::Trigger,
            test_context,
            api_tag,
        )
        .await;

        // Generate response - connector flow errors propagate as Err(tonic::Status)
        let success_response = response.into_grpc_status()?;

        let authorize_response =
            domain_types::types::generate_payment_authorize_response(success_response)
                .into_grpc_status()?;

        Ok(authorize_response)
    }
}

impl PaymentOperationsInternal for Payments {
    implement_connector_operation!(
        fn_name: internal_void_payment,
        log_prefix: "PAYMENT_VOID",
        request_type: PaymentServiceVoidRequest,
        response_type: PaymentServiceVoidResponse,
        flow_marker: Void,
        resource_common_data_type: PaymentFlowData,
        request_data_type: PaymentVoidData,
        response_data_type: PaymentsResponseData,
        request_data_constructor: PaymentVoidData::foreign_try_from,
        common_flow_data_constructor: PaymentFlowData::foreign_try_from,
        generate_response_fn: generate_payment_void_response,
        all_keys_required: None
    );

    implement_connector_operation!(
        fn_name: internal_refund,
        log_prefix: "REFUND",
        request_type: PaymentServiceRefundRequest,
        response_type: RefundResponse,
        flow_marker: Refund,
        resource_common_data_type: RefundFlowData,
        request_data_type: RefundsData,
        response_data_type: RefundsResponseData,
        request_data_constructor: RefundsData::foreign_try_from,
        common_flow_data_constructor: RefundFlowData::foreign_try_from,
        generate_response_fn: generate_refund_response,
        all_keys_required: None
    );

    implement_connector_operation!(
        fn_name: internal_payment_capture,
        log_prefix: "PAYMENT_CAPTURE",
        request_type: PaymentServiceCaptureRequest,
        response_type: PaymentServiceCaptureResponse,
        flow_marker: Capture,
        resource_common_data_type: PaymentFlowData,
        request_data_type: PaymentsCaptureData,
        response_data_type: PaymentsResponseData,
        request_data_constructor: PaymentsCaptureData::foreign_try_from,
        common_flow_data_constructor: PaymentFlowData::foreign_try_from,
        generate_response_fn: generate_payment_capture_response,
        all_keys_required: None
    );

    implement_connector_operation!(
        fn_name: internal_incremental_authorization,
        log_prefix: "INCREMENTAL_AUTHORIZATION",
        request_type: PaymentServiceIncrementalAuthorizationRequest,
        response_type: PaymentServiceIncrementalAuthorizationResponse,
        flow_marker: IncrementalAuthorization,
        resource_common_data_type: PaymentFlowData,
        request_data_type: PaymentsIncrementalAuthorizationData,
        response_data_type: PaymentsResponseData,
        request_data_constructor: PaymentsIncrementalAuthorizationData::foreign_try_from,
        common_flow_data_constructor: PaymentFlowData::foreign_try_from,
        generate_response_fn: generate_payment_incremental_authorization_response,
        all_keys_required: None
    );

    implement_connector_operation!(
        fn_name: internal_void_post_capture,
        log_prefix: "PAYMENT_VOID_POST_CAPTURE",
        request_type: PaymentServiceReverseRequest,
        response_type: PaymentServiceReverseResponse,
        flow_marker: VoidPC,
        resource_common_data_type: PaymentFlowData,
        request_data_type: PaymentsCancelPostCaptureData,
        response_data_type: PaymentsResponseData,
        request_data_constructor: PaymentsCancelPostCaptureData::foreign_try_from,
        common_flow_data_constructor: PaymentFlowData::foreign_try_from,
        generate_response_fn: generate_payment_void_post_capture_response,
        all_keys_required: None
    );

    implement_connector_operation!(
        fn_name: internal_create_order,
        log_prefix: "CREATE_ORDER",
        request_type: PaymentServiceCreateOrderRequest,
        response_type: PaymentServiceCreateOrderResponse,
        flow_marker: CreateOrder,
        resource_common_data_type: PaymentFlowData,
        request_data_type: PaymentCreateOrderData,
        response_data_type: PaymentCreateOrderResponse,
        request_data_constructor: PaymentCreateOrderData::foreign_try_from,
        common_flow_data_constructor: PaymentFlowData::foreign_try_from,
        generate_response_fn: generate_create_order_response,
        all_keys_required: None
    );
}

#[tonic::async_trait]
impl PaymentService for Payments {
    #[tracing::instrument(
        name = "payment_authorize",
        fields(
            name = common_utils::consts::NAME,
            service_name = tracing::field::Empty,
            service_method = FlowName::Authorize.as_str(),
            request_body = tracing::field::Empty,
            response_body = tracing::field::Empty,
            error_message = tracing::field::Empty,
            merchant_id = tracing::field::Empty,
            gateway = tracing::field::Empty,
            request_id = tracing::field::Empty,
            status_code = tracing::field::Empty,
            message_ = "Golden Log Line (incoming)",
            response_time = tracing::field::Empty,
            tenant_id = tracing::field::Empty,
            flow = FlowName::Authorize.as_str(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn authorize(
        &self,
        request: tonic::Request<PaymentServiceAuthorizeRequest>,
    ) -> Result<tonic::Response<PaymentServiceAuthorizeResponse>, tonic::Status> {
        info!("PAYMENT_AUTHORIZE_FLOW: initiated");
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "PaymentService".to_string());
        let config = get_config_from_request(&request)?;
        grpc_logging_wrapper(request, &service_name, config.clone(), FlowName::Authorize, |request_data| {
            let service_name = service_name.clone();
            Box::pin(async move {
                let metadata_payload = request_data.extracted_metadata;
                let metadata = &request_data.masked_metadata;
                let payload = request_data.payload;

                let authorize_response = match payload.payment_method.as_ref() {
                    Some(pm) => {
                        match pm.payment_method.as_ref() {
                            Some(payment_method::PaymentMethod::CardProxy(proxy_card_details)) => {
                                let token_data = proxy_card_details.to_token_data();
                                tracing::info!("INJECTOR: Authorization completed successfully with injector");
                                Box::pin(self.process_authorization_internal::<VaultTokenHolder>(
                                    &config,
                                    payload.clone(),
                                    metadata_payload.connector,
                                    metadata_payload.connector_config.clone(),
                                    metadata,
                                    &metadata_payload,
                                    &service_name,
                                    &metadata_payload.request_id,
                                    Some(token_data),
                                ))
                                .await?
                            }
                            _ => {
                                tracing::info!("REGULAR: Processing regular payment authorization (no injector)");
                                Box::pin(self.process_authorization_internal::<DefaultPCIHolder>(
                                    &config,
                                    payload.clone(),
                                    metadata_payload.connector,
                                    metadata_payload.connector_config.clone(),
                                    metadata,
                                    &metadata_payload,
                                    &service_name,
                                    &metadata_payload.request_id,
                                    None,
                                ))
                                .await?
                            }
                        }
                    }
                    _ => {
                        Box::pin(self.process_authorization_internal::<DefaultPCIHolder>(
                            &config,
                            payload.clone(),
                            metadata_payload.connector,
                            metadata_payload.connector_config.clone(),
                            metadata,
                            &metadata_payload,
                            &service_name,
                            &metadata_payload.request_id,
                            None,
                        ))
                        .await?
                    }
                };

                Ok(tonic::Response::new(authorize_response))
            })
        })
        .await
    }

    #[tracing::instrument(
        name = "payment_sync",
        fields(
            name = common_utils::consts::NAME,
            service_name = common_utils::consts::PAYMENT_SERVICE_NAME,
            service_method = FlowName::Psync.as_str(),
            request_body = tracing::field::Empty,
            response_body = tracing::field::Empty,
            error_message = tracing::field::Empty,
            merchant_id = tracing::field::Empty,
            gateway = tracing::field::Empty,
            request_id = tracing::field::Empty,
            status_code = tracing::field::Empty,
            message = "Golden Log Line (incoming)",
            response_time = tracing::field::Empty,
            tenant_id = tracing::field::Empty,
            flow = FlowName::Psync.as_str(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn get(
        &self,
        request: tonic::Request<PaymentServiceGetRequest>,
    ) -> Result<tonic::Response<PaymentServiceGetResponse>, tonic::Status> {
        info!("PAYMENT_SYNC_FLOW: initiated");
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "PaymentService".to_string());
        let config = get_config_from_request(&request)?;

        grpc_logging_wrapper(
            request,
            &service_name,
            config.clone(),
            FlowName::Psync,
            |request_data| {
                let service_name = service_name.clone();
                Box::pin(async move {
                    let metadata_payload = request_data.extracted_metadata;
                    let utils::MetadataPayload {
                        connector,
                        ..
                    } = metadata_payload;
                    let payload = request_data.payload;

                    // Get connector data
                    let connector_data: ConnectorData<DefaultPCIHolder> =
                        ConnectorData::get_connector_by_name(&connector);

                    // Get connector integration
                    let connector_integration: BoxedConnectorIntegrationV2<
                        '_,
                        PSync,
                        PaymentFlowData,
                        PaymentsSyncData,
                        PaymentsResponseData,
                    > = connector_data.connector.get_connector_integration_v2();

                    // Create connector request data
                    let payments_sync_data =
                        PaymentsSyncData::foreign_try_from(payload.clone()).into_grpc_status()?;

                    let connectors = utils::get_resolved_connectors(
                        &config,
                        &connector,
                        &metadata_payload.connector_config,
                        metadata_payload.environment.as_deref(),
                    )
                    .into_grpc_status()?;

                    // Create common request data
                    let payment_flow_data = PaymentFlowData::foreign_try_from((
                        payload.clone(),
                        connectors,
                        &request_data.masked_metadata,
                    ))
                    .into_grpc_status()?;

                    let should_do_access_token = connector_data
                        .connector
                        .should_do_access_token(Some(payment_flow_data.payment_method));

                    let payment_flow_data = if should_do_access_token {
                        let access_token = payload
                            .state
                            .as_ref()
                            .and_then(|state| state.access_token.as_ref())
                            .ok_or_else(|| tonic::Status::unauthenticated(
                                "Connector requires an access token; provide it via state.access_token",
                            ))?;
                        let access_token_data =
                            ServerAuthenticationTokenResponseData::foreign_try_from(access_token)
                                .map_err(|e| tonic::Status::unauthenticated(format!("Invalid access token: {e}")))?;
                        payment_flow_data.set_access_token(Some(access_token_data))
                    } else {
                        payment_flow_data
                    };

                    // Create router data
                    let router_data = RouterDataV2::<
                        PSync,
                        PaymentFlowData,
                        PaymentsSyncData,
                        PaymentsResponseData,
                    > {
                        flow: std::marker::PhantomData,
                        resource_common_data: payment_flow_data,
                        connector_config: metadata_payload.connector_config.clone(),
                        request: payments_sync_data.clone(),
                        response: Err(ErrorResponse::default()),
                    };

                    // Execute connector processing
                    let flow_name = utils::flow_marker_to_flow_name::<PSync>();

                    // Get API tag for the current flow with payment method type
                    let api_tag = config
                        .api_tags
                        .get_tag(flow_name, payments_sync_data.payment_method_type);

                    // Create test context if test mode is enabled
                    let test_context = config
                        .test
                        .create_test_context(&metadata_payload.request_id)
                        .map_err(|e| {
                            tonic::Status::internal(format!("Test mode configuration error: {e}"))
                        })?;

                    let event_params = EventProcessingParams {
                        connector_name: &metadata_payload.connector.to_string(),
                        service_name: &service_name,
                        service_type: utils::service_type_str(&config.server.type_),
                        flow_name,
                        event_config: &config.events,
                        request_id: &metadata_payload.request_id,
                        lineage_ids: &metadata_payload.lineage_ids,
                        reference_id: &metadata_payload.reference_id,
                        resource_id: &metadata_payload.resource_id,
                        shadow_mode: metadata_payload.shadow_mode,
                    };

                    let consume_or_trigger_flow = match payload.handle_response {
                        Some(resource_object) => {
                            common_enums::CallConnectorAction::HandleResponse(resource_object)
                        }
                        None => common_enums::CallConnectorAction::Trigger,
                    };

                    let response_result = Box::pin(
                        external_services::service::execute_connector_processing_step(
                            &config.proxy,
                            connector_integration,
                            router_data,
                            None,
                            event_params,
                            None,
                            consume_or_trigger_flow,
                            test_context,
                            api_tag,
                        ),
                    )
                    .await
                    .into_grpc_status()?;

                    // Generate response
                    let final_response =
                        generate_payment_sync_response(response_result).into_grpc_status()?;
                    Ok(tonic::Response::new(final_response))
                })
            },
        )
        .await
    }

    #[tracing::instrument(
        name = "payment_create_order",
        fields(
            name = common_utils::consts::NAME,
            service_name = common_utils::consts::PAYMENT_SERVICE_NAME,
            service_method = FlowName::CreateOrder.as_str(),
            request_body = tracing::field::Empty,
            response_body = tracing::field::Empty,
            error_message = tracing::field::Empty,
            merchant_id = tracing::field::Empty,
            gateway = tracing::field::Empty,
            request_id = tracing::field::Empty,
            status_code = tracing::field::Empty,
            message_ = "Golden Log Line (incoming)",
            response_time = tracing::field::Empty,
            tenant_id = tracing::field::Empty,
            flow = FlowName::CreateOrder.as_str(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn create_order(
        &self,
        request: tonic::Request<PaymentServiceCreateOrderRequest>,
    ) -> Result<tonic::Response<PaymentServiceCreateOrderResponse>, tonic::Status> {
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "PaymentService".to_string());
        let config = get_config_from_request(&request)?;
        grpc_logging_wrapper(
            request,
            &service_name,
            config.clone(),
            FlowName::CreateOrder,
            |request_data| async move { self.internal_create_order(request_data).await },
        )
        .await
    }

    #[tracing::instrument(
        name = "payment_void",
        fields(
            name = common_utils::consts::NAME,
            service_name = common_utils::consts::PAYMENT_SERVICE_NAME,
            service_method = FlowName::Void.as_str(),
            request_body = tracing::field::Empty,
            response_body = tracing::field::Empty,
            error_message = tracing::field::Empty,
            merchant_id = tracing::field::Empty,
            gateway = tracing::field::Empty,
            request_id = tracing::field::Empty,
            status_code = tracing::field::Empty,
            message_ = "Golden Log Line (incoming)",
            response_time = tracing::field::Empty,
            tenant_id = tracing::field::Empty,
            flow = FlowName::Void.as_str(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn void(
        &self,
        request: tonic::Request<PaymentServiceVoidRequest>,
    ) -> Result<tonic::Response<PaymentServiceVoidResponse>, tonic::Status> {
        info!("PAYMENT_VOID_FLOW: initiated");
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "PaymentService".to_string());
        let config = get_config_from_request(&request)?;
        grpc_logging_wrapper(
            request,
            &service_name,
            config.clone(),
            FlowName::Void,
            |request_data| {
                Box::pin(async move {
                    let metadata_payload = &request_data.extracted_metadata;
                    let connector = metadata_payload.connector;

                    // Get connector data to check if access token is needed
                    let connector_data: ConnectorData<DefaultPCIHolder> =
                        ConnectorData::get_connector_by_name(&connector);

                    // Check if connector supports access tokens
                    let connectors = utils::connectors_with_connector_config_overrides(
                        &metadata_payload.connector_config,
                        &config,
                    )
                    .into_grpc_status()?;

                    let temp_payment_flow_data = PaymentFlowData::foreign_try_from((
                        request_data.payload.clone(),
                        connectors,
                        &request_data.masked_metadata,
                    ))
                    .map_err(|e| e.into_grpc_status())?;
                    let should_do_access_token = connector_data
                        .connector
                        .should_do_access_token(Some(temp_payment_flow_data.payment_method));

                    if should_do_access_token {
                        let access_token = request_data
                            .payload
                            .state
                            .as_ref()
                            .and_then(|state| state.access_token.as_ref())
                            .ok_or_else(|| tonic::Status::unauthenticated(
                                "Connector requires an access token; provide it via state.access_token",
                            ))?;
                        // Validate the token is well-formed
                        ServerAuthenticationTokenResponseData::foreign_try_from(access_token)
                            .map_err(|e| tonic::Status::unauthenticated(format!("Invalid access token: {e}")))?;
                    }

                    self.internal_void_payment(request_data).await
                })
            },
        )
        .await
    }

    #[tracing::instrument(
        name = "payment_void_post_capture",
        fields(
            name = common_utils::consts::NAME,
            service_name = common_utils::consts::PAYMENT_SERVICE_NAME,
            service_method = FlowName::VoidPostCapture.as_str(),
            request_body = tracing::field::Empty,
            response_body = tracing::field::Empty,
            error_message = tracing::field::Empty,
            merchant_id = tracing::field::Empty,
            gateway = tracing::field::Empty,
            request_id = tracing::field::Empty,
            status_code = tracing::field::Empty,
            message_ = "Golden Log Line (incoming)",
            response_time = tracing::field::Empty,
            tenant_id = tracing::field::Empty,
            flow = FlowName::VoidPostCapture.as_str(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn reverse(
        &self,
        request: tonic::Request<PaymentServiceReverseRequest>,
    ) -> Result<tonic::Response<PaymentServiceReverseResponse>, tonic::Status> {
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "PaymentService".to_string());
        let config = get_config_from_request(&request)?;
        grpc_logging_wrapper(
            request,
            &service_name,
            config.clone(),
            FlowName::VoidPostCapture,
            |request_data| async move { self.internal_void_post_capture(request_data).await },
        )
        .await
    }

    #[tracing::instrument(
        name = "verify_redirect_response",
        fields(
            name = common_utils::consts::NAME,
            service_name = common_utils::consts::PAYMENT_SERVICE_NAME,
            service_method = FlowName::VerifyRedirectResponse.as_str(),
            request_body = tracing::field::Empty,
            response_body = tracing::field::Empty,
            error_message = tracing::field::Empty,
            merchant_id = tracing::field::Empty,
            gateway = tracing::field::Empty,
            request_id = tracing::field::Empty,
            status_code = tracing::field::Empty,
            message_ = "Golden Log Line (incoming)",
            response_time = tracing::field::Empty,
            tenant_id = tracing::field::Empty,
            flow = FlowName::VerifyRedirectResponse.as_str(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn verify_redirect_response(
        &self,
        request: tonic::Request<PaymentServiceVerifyRedirectResponseRequest>,
    ) -> Result<tonic::Response<PaymentServiceVerifyRedirectResponseResponse>, tonic::Status> {
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "PaymentService".to_string());
        let config = get_config_from_request(&request)?;
        grpc_logging_wrapper(
            request,
            &service_name,
            config.clone(),
            FlowName::VerifyRedirectResponse,
            |request_data| {
                async move {
                    let payload = request_data.payload;
                    let metadata_payload = request_data.extracted_metadata;
                    let connector = metadata_payload.connector;

                    let request_details = payload
                        .request_details
                        .map(domain_types::connector_types::RequestDetails::foreign_try_from)
                        .transpose()
                        .map_err(|e| e.into_grpc_status())?
                        .ok_or(tonic::Status::invalid_argument("missing request_details in the payload"))?;

                    let secrets = payload
                        .redirect_response_secrets
                        .map(domain_types::connector_types::ConnectorRedirectResponseSecrets::foreign_try_from)
                        .transpose()
                        .map_err(|e| e.into_grpc_status())?
                        .map(ConnectorSourceVerificationSecrets::RedirectResponseSecret);

                    // Get connector data
                    let connector_data: ConnectorData<DefaultPCIHolder> =
                        ConnectorData::get_connector_by_name(&connector);

                    let decoded_body = match connector_data
                        .connector
                        .decode_redirect_response_body(
                            &request_details,
                            secrets.clone(),
                        ) {
                            Ok(result) => result,
                            Err(err) => {
                                tracing::warn!(
                                    target: "decode_redirect_response_body",
                                    "{:?}",
                                    err
                                );
                                request_details.body
                            }
                        };

                    // Create request_details with decoded body for connector processing
                    let updated_request_details = domain_types::connector_types::RequestDetails {
                        method: request_details.method.clone(),
                        uri: request_details.uri.clone(),
                        url: request_details.url.clone(),
                        headers: request_details.headers,
                        query_params: request_details.query_params.clone(),
                        body: decoded_body,
                    };

                    let source_verified = match connector_data
                        .connector
                        .verify_redirect_response_source(
                            &updated_request_details,
                            secrets,
                        ) {
                            Ok(result) => result,
                            Err(err) => {
                                tracing::warn!(
                                    target: "verify_redirect_response",
                                    "{:?}",
                                    err
                                );
                                false
                            }
                        };

                    let redirect_details_response = connector_data
                        .connector
                        .process_redirect_response(
                            &updated_request_details,
                        )
                        .into_grpc_status()?;

                    let response = PaymentServiceVerifyRedirectResponseResponse::foreign_try_from((source_verified, redirect_details_response))
                        .map_err(|e| e.into_grpc_status())?;

                    Ok(tonic::Response::new(response))
                }
            }
        ).await
    }

    #[tracing::instrument(
        name = "refund",
        fields(
            name = common_utils::consts::NAME,
            service_name = common_utils::consts::PAYMENT_SERVICE_NAME,
            service_method = FlowName::Refund.as_str(),
            request_body = tracing::field::Empty,
            response_body = tracing::field::Empty,
            error_message = tracing::field::Empty,
            merchant_id = tracing::field::Empty,
            gateway = tracing::field::Empty,
            request_id = tracing::field::Empty,
            status_code = tracing::field::Empty,
            message_ = "Golden Log Line (incoming)",
            response_time = tracing::field::Empty,
            tenant_id = tracing::field::Empty,
            flow = FlowName::Refund.as_str(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn refund(
        &self,
        request: tonic::Request<PaymentServiceRefundRequest>,
    ) -> Result<tonic::Response<RefundResponse>, tonic::Status> {
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "PaymentService".to_string());
        let config = get_config_from_request(&request)?;
        grpc_logging_wrapper(
            request,
            &service_name,
            config.clone(),
            FlowName::Refund,
            |request_data| async move { self.internal_refund(request_data).await },
        )
        .await
    }

    #[tracing::instrument(
        name = "payment_capture",
        fields(
            name = common_utils::consts::NAME,
            service_name = common_utils::consts::PAYMENT_SERVICE_NAME,
            service_method = FlowName::Capture.as_str(),
            request_body = tracing::field::Empty,
            response_body = tracing::field::Empty,
            error_message = tracing::field::Empty,
            merchant_id = tracing::field::Empty,
            gateway = tracing::field::Empty,
            request_id = tracing::field::Empty,
            status_code = tracing::field::Empty,
            message_ = "Golden Log Line (incoming)",
            response_time = tracing::field::Empty,
            tenant_id = tracing::field::Empty,
            flow = FlowName::Capture.as_str(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn capture(
        &self,
        request: tonic::Request<PaymentServiceCaptureRequest>,
    ) -> Result<tonic::Response<PaymentServiceCaptureResponse>, tonic::Status> {
        info!("PAYMENT_CAPTURE_FLOW: initiated");
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "PaymentService".to_string());
        let config = get_config_from_request(&request)?;
        grpc_logging_wrapper(
            request,
            &service_name,
            config.clone(),
            FlowName::Capture,
            |request_data| {
                Box::pin(async move {
                    let metadata_payload = &request_data.extracted_metadata;
                    let connector = metadata_payload.connector;

                    // Get connector data to check if access token is needed
                    let connector_data: ConnectorData<DefaultPCIHolder> =
                        ConnectorData::get_connector_by_name(&connector);

                    // Check if connector supports access tokens
                    let connectors = utils::connectors_with_connector_config_overrides(
                        &metadata_payload.connector_config,
                        &config,
                    )
                    .into_grpc_status()?;

                    let temp_payment_flow_data = PaymentFlowData::foreign_try_from((
                        request_data.payload.clone(),
                        connectors,
                        &request_data.masked_metadata,
                    ))
                    .map_err(|e| e.into_grpc_status())?;
                    let should_do_access_token = connector_data
                        .connector
                        .should_do_access_token(Some(temp_payment_flow_data.payment_method));

                    if should_do_access_token {
                        let access_token = request_data
                            .payload
                            .state
                            .as_ref()
                            .and_then(|state| state.access_token.as_ref())
                            .ok_or_else(|| tonic::Status::unauthenticated(
                                "Connector requires an access token; provide it via state.access_token",
                            ))?;
                        // Validate the token is well-formed
                        ServerAuthenticationTokenResponseData::foreign_try_from(access_token)
                            .map_err(|e| tonic::Status::unauthenticated(format!("Invalid access token: {e}")))?;
                    }

                    self.internal_payment_capture(request_data).await
                })
            },
        )
        .await
    }

    #[tracing::instrument(
        name = "setup_recurring",
        fields(
            name = common_utils::consts::NAME,
            service_name = common_utils::consts::PAYMENT_SERVICE_NAME,
            service_method = FlowName::SetupMandate.as_str(),
            request_body = tracing::field::Empty,
            response_body = tracing::field::Empty,
            error_message = tracing::field::Empty,
            merchant_id = tracing::field::Empty,
            gateway = tracing::field::Empty,
            request_id = tracing::field::Empty,
            status_code = tracing::field::Empty,
            message_ = "Golden Log Line (incoming)",
            response_time = tracing::field::Empty,
            tenant_id = tracing::field::Empty,
            flow = FlowName::SetupMandate.as_str(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn setup_recurring(
        &self,
        request: tonic::Request<PaymentServiceSetupRecurringRequest>,
    ) -> Result<tonic::Response<PaymentServiceSetupRecurringResponse>, tonic::Status> {
        info!("SETUP_RECURRING_FLOW: initiated");
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "PaymentService".to_string());
        let config = get_config_from_request(&request)?;
        grpc_logging_wrapper(
            request,
            &service_name,
            config.clone(),
            FlowName::SetupMandate,
            |request_data| {
                let service_name = service_name.clone();
                let config = config.clone();
                Box::pin(async move {
                    let payload = request_data.payload;
                    let metadata_payload = request_data.extracted_metadata;
                    let (connector, request_id, lineage_ids) = (
                        metadata_payload.connector,
                        metadata_payload.request_id,
                        metadata_payload.lineage_ids,
                    );
                    let connector_config = &metadata_payload.connector_config;

                    //get connector data
                    let connector_data = ConnectorData::get_connector_by_name(&connector);

                    // Get connector integration
                    let connector_integration: BoxedConnectorIntegrationV2<
                        '_,
                        SetupMandate,
                        PaymentFlowData,
                        SetupMandateRequestData<DefaultPCIHolder>,
                        PaymentsResponseData,
                    > = connector_data.connector.get_connector_integration_v2();

                    let connectors = utils::connectors_with_connector_config_overrides(
                        &metadata_payload.connector_config,
                        &config,
                    )
                    .into_grpc_status()?;

                    // Create common request data
                    let payment_flow_data = PaymentFlowData::foreign_try_from((
                        payload.clone(),
                        connectors,
                        config.common.environment,
                        &request_data.masked_metadata,
                    ))
                    .map_err(|e| e.into_grpc_status())?;

                    let setup_mandate_request_data =
                        SetupMandateRequestData::foreign_try_from(payload.clone())
                            .map_err(|e| e.into_grpc_status())?;

                    // Create router data
                    let router_data: RouterDataV2<
                        SetupMandate,
                        PaymentFlowData,
                        SetupMandateRequestData<DefaultPCIHolder>,
                        PaymentsResponseData,
                    > = RouterDataV2 {
                        flow: std::marker::PhantomData,
                        resource_common_data: payment_flow_data,
                        connector_config: connector_config.clone(),
                        request: setup_mandate_request_data.clone(),
                        response: Err(ErrorResponse::default()),
                    };

                    // Get API tag for SetupMandate flow
                    let api_tag = config.api_tags.get_tag(
                        FlowName::SetupMandate,
                        setup_mandate_request_data.payment_method_type,
                    );

                    // Create test context if test mode is enabled
                    let test_context =
                        config.test.create_test_context(&request_id).map_err(|e| {
                            tonic::Status::internal(format!("Test mode configuration error: {e}"))
                        })?;

                    let event_params = EventProcessingParams {
                        connector_name: &connector.to_string(),
                        service_name: &service_name,
                        service_type: utils::service_type_str(&config.server.type_),
                        flow_name: FlowName::SetupMandate,
                        event_config: &config.events,
                        request_id: &request_id,
                        lineage_ids: &lineage_ids,
                        reference_id: &metadata_payload.reference_id,
                        resource_id: &metadata_payload.resource_id,
                        shadow_mode: metadata_payload.shadow_mode,
                    };

                    let response = Box::pin(
                        external_services::service::execute_connector_processing_step(
                            &config.proxy,
                            connector_integration,
                            router_data,
                            None,
                            event_params,
                            None, // token_data - None for non-proxy payments
                            common_enums::CallConnectorAction::Trigger,
                            test_context,
                            api_tag,
                        ),
                    )
                    .await
                    .map_err(|e| e.into_grpc_status())?;

                    // Generate response
                    let setup_mandate_response = generate_setup_mandate_response(response)
                        .map_err(|e| e.into_grpc_status())?;

                    Ok(tonic::Response::new(setup_mandate_response))
                })
            },
        )
        .await
    }

    #[tracing::instrument(
        name = "incremental_authorization",
        fields(
            name = common_utils::consts::NAME,
            service_name = common_utils::consts::PAYMENT_SERVICE_NAME,
            service_method = FlowName::IncrementalAuthorization.as_str(),
            request_body = tracing::field::Empty,
            response_body = tracing::field::Empty,
            error_message = tracing::field::Empty,
            merchant_id = tracing::field::Empty,
            gateway = tracing::field::Empty,
            request_id = tracing::field::Empty,
            status_code = tracing::field::Empty,
            message_ = "Golden Log Line (incoming)",
            response_time = tracing::field::Empty,
            tenant_id = tracing::field::Empty,
            flow = FlowName::IncrementalAuthorization.as_str(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn incremental_authorization(
        &self,
        request: tonic::Request<PaymentServiceIncrementalAuthorizationRequest>,
    ) -> Result<tonic::Response<PaymentServiceIncrementalAuthorizationResponse>, tonic::Status>
    {
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "PaymentService".to_string());
        let config = get_config_from_request(&request)?;
        grpc_logging_wrapper(
            request,
            &service_name,
            config.clone(),
            FlowName::IncrementalAuthorization,
            |request_data: RequestData<PaymentServiceIncrementalAuthorizationRequest>| async move {
                self.internal_incremental_authorization(request_data).await
            },
        )
        .await
    }

    #[tracing::instrument(
        name = "token_authorize",
        fields(
            name = common_utils::consts::NAME,
            service_name = common_utils::consts::PAYMENT_SERVICE_NAME,
            service_method = "token_authorize",
            request_body = tracing::field::Empty,
            response_body = tracing::field::Empty,
            error_message = tracing::field::Empty,
            merchant_id = tracing::field::Empty,
            gateway = tracing::field::Empty,
            request_id = tracing::field::Empty,
            status_code = tracing::field::Empty,
            message_ = "Golden Log Line (incoming)",
            response_time = tracing::field::Empty,
            tenant_id = tracing::field::Empty,
            flow = FlowName::Authorize.as_str(),
            flow_specific_fields.status = tracing::field::Empty,
        ),
        skip(self, request)
    )]
    async fn token_authorize(
        &self,
        request: tonic::Request<PaymentServiceTokenAuthorizeRequest>,
    ) -> Result<tonic::Response<PaymentServiceAuthorizeResponse>, tonic::Status> {
        info!("TOKEN_AUTHORIZE_FLOW: initiated");
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "PaymentService".to_string());
        let config = get_config_from_request(&request)?;

        let extensions = request.extensions().clone();
        let metadata = request.metadata().clone();

        grpc_logging_wrapper(
            request,
            &service_name,
            config.clone(),
            FlowName::Authorize,
            |request_data| {
                let service_name = service_name.clone();
                let extensions = extensions.clone();
                let metadata = metadata.clone();
                Box::pin(async move {
                    let authorize_request = tokenized_authorize_to_base(request_data.payload);

                    let mut inner_request = tonic::Request::new(authorize_request);
                    *inner_request.extensions_mut() = extensions;
                    *inner_request.metadata_mut() = metadata;
                    inner_request.extensions_mut().insert(service_name.clone());

                    <Self as PaymentService>::authorize(self, inner_request).await
                })
            },
        )
        .await
    }

    #[tracing::instrument(
        name = "token_setup_recurring",
        fields(
            name = common_utils::consts::NAME,
            service_name = common_utils::consts::PAYMENT_SERVICE_NAME,
            service_method = "token_setup_recurring",
            request_body = tracing::field::Empty,
            response_body = tracing::field::Empty,
            error_message = tracing::field::Empty,
            merchant_id = tracing::field::Empty,
            gateway = tracing::field::Empty,
            request_id = tracing::field::Empty,
            status_code = tracing::field::Empty,
            message_ = "Golden Log Line (incoming)",
            response_time = tracing::field::Empty,
            tenant_id = tracing::field::Empty,
            flow = FlowName::SetupMandate.as_str(),
            flow_specific_fields.status = tracing::field::Empty,
        ),
        skip(self, request)
    )]
    async fn token_setup_recurring(
        &self,
        request: tonic::Request<PaymentServiceTokenSetupRecurringRequest>,
    ) -> Result<tonic::Response<PaymentServiceSetupRecurringResponse>, tonic::Status> {
        info!("TOKEN_SETUP_RECURRING_FLOW: initiated");
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "PaymentService".to_string());
        let config = get_config_from_request(&request)?;

        let extensions = request.extensions().clone();
        let metadata = request.metadata().clone();

        grpc_logging_wrapper(
            request,
            &service_name,
            config.clone(),
            FlowName::SetupMandate,
            |request_data| {
                let service_name = service_name.clone();
                let extensions = extensions.clone();
                let metadata = metadata.clone();
                Box::pin(async move {
                    let setup_recurring_request =
                        tokenized_setup_recurring_to_base(request_data.payload);

                    let mut inner_request = tonic::Request::new(setup_recurring_request);
                    *inner_request.extensions_mut() = extensions;
                    *inner_request.metadata_mut() = metadata;
                    inner_request.extensions_mut().insert(service_name.clone());

                    <Self as PaymentService>::setup_recurring(self, inner_request).await
                })
            },
        )
        .await
    }
    #[tracing::instrument(
        name = "proxy_authorize",
        fields(
            name = common_utils::consts::NAME,
            service_name = common_utils::consts::PAYMENT_SERVICE_NAME,
            service_method = "proxy_authorize",
            request_body = tracing::field::Empty,
            response_body = tracing::field::Empty,
            error_message = tracing::field::Empty,
            merchant_id = tracing::field::Empty,
            gateway = tracing::field::Empty,
            request_id = tracing::field::Empty,
            status_code = tracing::field::Empty,
            message_ = "Golden Log Line (incoming)",
            response_time = tracing::field::Empty,
            tenant_id = tracing::field::Empty,
            flow = FlowName::Authorize.as_str(),
            flow_specific_fields.status = tracing::field::Empty,
        ),
        skip(self, request)
    )]
    async fn proxy_authorize(
        &self,
        request: tonic::Request<PaymentServiceProxyAuthorizeRequest>,
    ) -> Result<tonic::Response<PaymentServiceAuthorizeResponse>, tonic::Status> {
        info!("PROXY_AUTHORIZE_FLOW: initiated");
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "PaymentService".to_string());
        let config = get_config_from_request(&request)?;

        let extensions = request.extensions().clone();
        let metadata = request.metadata().clone();

        grpc_logging_wrapper(
            request,
            &service_name,
            config.clone(),
            FlowName::Authorize,
            |request_data| {
                let service_name = service_name.clone();
                let extensions = extensions.clone();
                let metadata = metadata.clone();
                Box::pin(async move {
                    let authorize_request =
                        proxied_authorize_to_base(request_data.payload).into_grpc_status()?;

                    let mut inner_request = tonic::Request::new(authorize_request);
                    *inner_request.extensions_mut() = extensions;
                    *inner_request.metadata_mut() = metadata;
                    inner_request.extensions_mut().insert(service_name.clone());

                    <Self as PaymentService>::authorize(self, inner_request).await
                })
            },
        )
        .await
    }

    #[tracing::instrument(
        name = "proxy_setup_recurring",
        fields(
            name = common_utils::consts::NAME,
            service_name = common_utils::consts::PAYMENT_SERVICE_NAME,
            service_method = "proxy_setup_recurring",
            request_body = tracing::field::Empty,
            response_body = tracing::field::Empty,
            error_message = tracing::field::Empty,
            merchant_id = tracing::field::Empty,
            gateway = tracing::field::Empty,
            request_id = tracing::field::Empty,
            status_code = tracing::field::Empty,
            message_ = "Golden Log Line (incoming)",
            response_time = tracing::field::Empty,
            tenant_id = tracing::field::Empty,
            flow = FlowName::SetupMandate.as_str(),
            flow_specific_fields.status = tracing::field::Empty,
        ),
        skip(self, request)
    )]
    async fn proxy_setup_recurring(
        &self,
        request: tonic::Request<PaymentServiceProxySetupRecurringRequest>,
    ) -> Result<tonic::Response<PaymentServiceSetupRecurringResponse>, tonic::Status> {
        info!("PROXY_SETUP_RECURRING_FLOW: initiated");
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "PaymentService".to_string());
        let config = get_config_from_request(&request)?;

        let extensions = request.extensions().clone();
        let metadata = request.metadata().clone();

        grpc_logging_wrapper(
            request,
            &service_name,
            config.clone(),
            FlowName::SetupMandate,
            |request_data| {
                let service_name = service_name.clone();
                let extensions = extensions.clone();
                let metadata = metadata.clone();
                Box::pin(async move {
                    let setup_recurring_request =
                        proxied_setup_recurring_to_base(request_data.payload).into_grpc_status()?;

                    let mut inner_request = tonic::Request::new(setup_recurring_request);
                    *inner_request.extensions_mut() = extensions;
                    *inner_request.metadata_mut() = metadata;
                    inner_request.extensions_mut().insert(service_name.clone());

                    <Self as PaymentService>::setup_recurring(self, inner_request).await
                })
            },
        )
        .await
    }
}

#[tonic::async_trait]
impl PaymentMethodService for PaymentMethod {
    #[tracing::instrument(
        name = "tokenize",
        fields(
            name = common_utils::consts::NAME,
            service_name = common_utils::consts::PAYMENT_SERVICE_NAME,
            service_method = "Tokenize",
            request_body = tracing::field::Empty,
            response_body = tracing::field::Empty,
            error_message = tracing::field::Empty,
            merchant_id = tracing::field::Empty,
            gateway = tracing::field::Empty,
            request_id = tracing::field::Empty,
            status_code = tracing::field::Empty,
            message_ = "Golden Log Line (incoming)",
            response_time = tracing::field::Empty,
            tenant_id = tracing::field::Empty,
            flow = "CreatePaymentMethodToken",
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn tokenize(
        &self,
        request: tonic::Request<PaymentMethodServiceTokenizeRequest>,
    ) -> Result<tonic::Response<PaymentMethodServiceTokenizeResponse>, tonic::Status> {
        info!("TOKENIZE_FLOW: initiated");
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "PaymentService".to_string());
        let config = get_config_from_request(&request)?;

        grpc_logging_wrapper(
            request,
            &service_name,
            config.clone(),
            FlowName::PaymentMethodToken,
            |request_data| {
                let service_name = service_name.clone();
                let config = config.clone();
                Box::pin(async move {
                    let payload = request_data.payload;
                    let metadata_payload = request_data.extracted_metadata;
                    let (connector, request_id, lineage_ids) = (
                        metadata_payload.connector,
                        metadata_payload.request_id,
                        metadata_payload.lineage_ids,
                    );
                    let connector_config = &metadata_payload.connector_config;

                    // Get connector data
                    let connector_data: ConnectorData<DefaultPCIHolder> =
                        ConnectorData::get_connector_by_name(&connector);

                    // Get connector integration
                    let connector_integration: BoxedConnectorIntegrationV2<
                        '_,
                        PaymentMethodToken,
                        PaymentFlowData,
                        PaymentMethodTokenizationData<DefaultPCIHolder>,
                        PaymentMethodTokenResponse,
                    > = connector_data.connector.get_connector_integration_v2();

                    let connectors = utils::connectors_with_connector_config_overrides(
                        &metadata_payload.connector_config,
                        &config,
                    )
                    .into_grpc_status()?;

                    // Create payment flow data
                    let payment_flow_data = PaymentFlowData::foreign_try_from((
                        payload.clone(),
                        connectors,
                        &request_data.masked_metadata,
                    ))
                    .map_err(|e| e.into_grpc_status())?;

                    // Get payment method token request data
                    let payment_method_token_request_data =
                        PaymentMethodTokenizationData::foreign_try_from(payload.clone()).map_err(
                            |err| {
                                tracing::error!(
                                    "Failed to process payment method token data: {:?}",
                                    err
                                );
                                err.into_grpc_status()
                            },
                        )?;

                    // Create router data for payment method token flow
                    let payment_method_token_router_data = RouterDataV2::<
                        PaymentMethodToken,
                        PaymentFlowData,
                        PaymentMethodTokenizationData<DefaultPCIHolder>,
                        PaymentMethodTokenResponse,
                    > {
                        flow: std::marker::PhantomData,
                        resource_common_data: payment_flow_data.clone(),
                        connector_config: connector_config.clone(),
                        request: payment_method_token_request_data.clone(),
                        response: Err(ErrorResponse::default()),
                    };

                    // Get API tag for PaymentMethodToken flow
                    let api_tag = config.api_tags.get_tag(FlowName::PaymentMethodToken, None);

                    // Create test context if test mode is enabled
                    let test_context =
                        config.test.create_test_context(&request_id).map_err(|e| {
                            tonic::Status::internal(format!("Test mode configuration error: {e}"))
                        })?;

                    // Execute connector processing
                    let event_params = EventProcessingParams {
                        connector_name: &connector.to_string(),
                        service_name: &service_name,
                        service_type: utils::service_type_str(&config.server.type_),
                        flow_name: FlowName::PaymentMethodToken,
                        event_config: &config.events,
                        request_id: &request_id,
                        lineage_ids: &lineage_ids,
                        reference_id: &metadata_payload.reference_id,
                        resource_id: &metadata_payload.resource_id,
                        shadow_mode: metadata_payload.shadow_mode,
                    };

                    let response = Box::pin(
                        external_services::service::execute_connector_processing_step(
                            &config.proxy,
                            connector_integration,
                            payment_method_token_router_data,
                            None,
                            event_params,
                            None,
                            common_enums::CallConnectorAction::Trigger,
                            test_context,
                            api_tag,
                        ),
                    )
                    .await
                    .map_err(|e| e.into_grpc_status())?;

                    // Generate response using the existing function
                    let payment_method_token_response =
                        domain_types::types::generate_create_payment_method_token_response(
                            response,
                        )
                        .map_err(|e| e.into_grpc_status())?;

                    Ok(tonic::Response::new(payment_method_token_response))
                })
            },
        )
        .await
    }

    async fn eligibility(
        &self,
        _request: tonic::Request<PayoutMethodEligibilityRequest>,
    ) -> Result<tonic::Response<PayoutMethodEligibilityResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "Payout method eligibility is not implemented",
        ))
    }
}

impl MerchantAuthentication {
    #[allow(clippy::too_many_arguments)]
    async fn handle_session_token<
        T: PaymentMethodDataTypes
            + Default
            + Eq
            + Debug
            + Send
            + serde::Serialize
            + serde::de::DeserializeOwned
            + Clone
            + Sync
            + domain_types::types::CardConversionHelper<T>
            + 'static,
        P: serde::Serialize + Clone,
    >(
        &self,
        config: &Arc<Config>,
        connector_data: ConnectorData<T>,
        payment_flow_data: &PaymentFlowData,
        connector_config: ConnectorSpecificConfig,
        payload: &P,
        connector_name: &str,
        service_name: &str,
        event_params: EventParams<'_>,
    ) -> Result<ServerSessionAuthenticationTokenResponseData, tonic::Status>
    where
        ServerSessionAuthenticationTokenRequestData: ForeignTryFrom<P, Error = IntegrationError>,
    {
        // Get connector integration
        let connector_integration: BoxedConnectorIntegrationV2<
            '_,
            ServerSessionAuthenticationToken,
            PaymentFlowData,
            ServerSessionAuthenticationTokenRequestData,
            ServerSessionAuthenticationTokenResponseData,
        > = connector_data.connector.get_connector_integration_v2();

        // Create session token request data using try_from_foreign
        let session_token_request_data =
            ServerSessionAuthenticationTokenRequestData::foreign_try_from(payload.clone())
                .into_grpc_status()?;

        let session_token_router_data = RouterDataV2::<
            ServerSessionAuthenticationToken,
            PaymentFlowData,
            ServerSessionAuthenticationTokenRequestData,
            ServerSessionAuthenticationTokenResponseData,
        > {
            flow: std::marker::PhantomData,
            resource_common_data: payment_flow_data.clone(),
            connector_config,
            request: session_token_request_data,
            response: Err(ErrorResponse::default()),
        };

        // Get API tag for ServerSessionAuthenticationToken flow with payment method type if available
        let api_tag = config
            .api_tags
            .get_tag(FlowName::ServerSessionAuthenticationToken, None);

        // Create test context if test mode is enabled
        let test_context = config
            .test
            .create_test_context(event_params.request_id)
            .map_err(|e| tonic::Status::internal(format!("Test mode configuration error: {e}")))?;

        // Create event processing parameters
        let external_event_params = EventProcessingParams {
            connector_name,
            service_name,
            service_type: event_params.service_type,
            flow_name: FlowName::ServerSessionAuthenticationToken,
            event_config: &config.events,
            request_id: event_params.request_id,
            lineage_ids: event_params.lineage_ids,
            reference_id: event_params.reference_id,
            resource_id: event_params.resource_id,
            shadow_mode: event_params.shadow_mode,
        };

        // Execute connector processing
        let response = Box::pin(
            external_services::service::execute_connector_processing_step(
                &config.proxy,
                connector_integration,
                session_token_router_data,
                None,
                external_event_params,
                None,
                common_enums::CallConnectorAction::Trigger,
                test_context,
                api_tag,
            ),
        )
        .await
        .into_grpc_status()?;

        match response.response {
            Ok(session_response) => {
                tracing::info!(
                    "Session token created successfully: {}",
                    session_response.session_token
                );
                Ok(session_response)
            }
            Err(error_response) => Err(error_stack::report!(
                ConnectorError::ConnectorErrorResponse(error_response)
            )
            .into_grpc_status()),
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn handle_access_token<
        T: PaymentMethodDataTypes
            + Default
            + Eq
            + Debug
            + Send
            + serde::Serialize
            + serde::de::DeserializeOwned
            + Clone
            + Sync
            + domain_types::types::CardConversionHelper<T>
            + 'static,
    >(
        &self,
        config: &Arc<Config>,
        connector_data: ConnectorData<T>,
        payment_flow_data: &PaymentFlowData,
        connector_config: ConnectorSpecificConfig,
        connector_name: &str,
        service_name: &str,
        event_params: EventParams<'_>,
    ) -> Result<MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse, tonic::Status>
    where
        ServerAuthenticationTokenRequestData:
            for<'a> ForeignTryFrom<&'a ConnectorSpecificConfig, Error = IntegrationError>,
    {
        // Get connector integration for ServerAuthenticationToken flow
        let connector_integration: BoxedConnectorIntegrationV2<
            '_,
            ServerAuthenticationToken,
            PaymentFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        > = connector_data.connector.get_connector_integration_v2();

        // Create access token request data - grant type determined by connector
        let access_token_request_data = ServerAuthenticationTokenRequestData::foreign_try_from(
            &connector_config, // Contains typed connector config
        )
        .into_grpc_status()?;

        // Create router data for access token flow
        let access_token_router_data = RouterDataV2::<
            ServerAuthenticationToken,
            PaymentFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        > {
            flow: std::marker::PhantomData,
            resource_common_data: payment_flow_data.clone(),
            connector_config,
            request: access_token_request_data,
            response: Err(ErrorResponse::default()),
        };

        // Get API tag for ServerAuthenticationToken flow with payment method type if available
        let api_tag = config
            .api_tags
            .get_tag(FlowName::ServerAuthenticationToken, None);

        // Create test context if test mode is enabled
        let test_context = config
            .test
            .create_test_context(event_params.request_id)
            .map_err(|e| tonic::Status::internal(format!("Test mode configuration error: {e}")))?;

        // Execute connector processing
        let external_event_params = EventProcessingParams {
            connector_name,
            service_name,
            service_type: event_params.service_type,
            flow_name: FlowName::ServerAuthenticationToken,
            event_config: &config.events,
            request_id: event_params.request_id,
            lineage_ids: event_params.lineage_ids,
            reference_id: event_params.reference_id,
            resource_id: event_params.resource_id,
            shadow_mode: event_params.shadow_mode,
        };

        let response = Box::pin(
            external_services::service::execute_connector_processing_step(
                &config.proxy,
                connector_integration,
                access_token_router_data,
                None,
                external_event_params,
                None,
                common_enums::CallConnectorAction::Trigger,
                test_context,
                api_tag,
            ),
        )
        .await
        .into_grpc_status()?;

        // Use generate_access_token_response for consistency
        domain_types::types::generate_access_token_response(response).into_grpc_status()
    }
}

impl MerchantAuthenticationOperational for MerchantAuthentication {
    implement_connector_operation!(
        fn_name: internal_sdk_session_token,
        log_prefix: "SDK_SESSION",
        request_type: MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest,
        response_type: MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse,
        flow_marker: ClientAuthenticationToken,
        resource_common_data_type: PaymentFlowData,
        request_data_type: ClientAuthenticationTokenRequestData,
        response_data_type: PaymentsResponseData,
        request_data_constructor: ClientAuthenticationTokenRequestData::foreign_try_from,
        common_flow_data_constructor: PaymentFlowData::foreign_try_from,
        generate_response_fn: generate_payment_sdk_session_token_response,
        all_keys_required: None
    );
}

#[tonic::async_trait]
impl MerchantAuthenticationService for MerchantAuthentication {
    #[tracing::instrument(
        name = "client_authentication_token",
        fields(
            name = common_utils::consts::NAME,
            service_name = common_utils::consts::PAYMENT_SERVICE_NAME,
            service_method = FlowName::ClientAuthenticationToken.as_str(),
            request_body = tracing::field::Empty,
            response_body = tracing::field::Empty,
            error_message = tracing::field::Empty,
            merchant_id = tracing::field::Empty,
            gateway = tracing::field::Empty,
            request_id = tracing::field::Empty,
            status_code = tracing::field::Empty,
            message_ = "Golden Log Line (incoming)",
            response_time = tracing::field::Empty,
            tenant_id = tracing::field::Empty,
            flow = FlowName::ClientAuthenticationToken.as_str(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn create_client_authentication_token(
        &self,
        request: tonic::Request<
            MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest,
        >,
    ) -> Result<
        tonic::Response<MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse>,
        tonic::Status,
    > {
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "PaymentService".to_string());
        let config = get_config_from_request(&request)?;
        grpc_logging_wrapper(
            request,
            &service_name,
            config,
            FlowName::ClientAuthenticationToken,
            |request_data| async move { self.internal_sdk_session_token(request_data).await },
        )
        .await
    }

    #[tracing::instrument(
        name = "server_session_authentication_token",
        fields(
            name = common_utils::consts::NAME,
            service_name = common_utils::consts::PAYMENT_SERVICE_NAME,
            service_method = FlowName::ServerSessionAuthenticationToken.as_str(),
            request_body = tracing::field::Empty,
            response_body = tracing::field::Empty,
            error_message = tracing::field::Empty,
            merchant_id = tracing::field::Empty,
            gateway = tracing::field::Empty,
            request_id = tracing::field::Empty,
            status_code = tracing::field::Empty,
            message_ = "Golden Log Line (incoming)",
            response_time = tracing::field::Empty,
            tenant_id = tracing::field::Empty,
            flow = FlowName::ServerSessionAuthenticationToken.as_str(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn create_server_session_authentication_token(
        &self,
        request: tonic::Request<
            MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest,
        >,
    ) -> Result<
        tonic::Response<
            MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse,
        >,
        tonic::Status,
    > {
        info!("CREATE_CONNECTOR_SESSION_FLOW: initiated");
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "PaymentService".to_string());
        let config = get_config_from_request(&request)?;
        grpc_logging_wrapper(
            request,
            &service_name,
            config.clone(),
            FlowName::ServerSessionAuthenticationToken,
            |request_data| {
                let service_name = service_name.clone();
                Box::pin(async move {
                    let payload = request_data.payload;
                    let metadata_payload = request_data.extracted_metadata;
                    let (connector, request_id, lineage_ids) = (
                        metadata_payload.connector,
                        metadata_payload.request_id,
                        metadata_payload.lineage_ids,
                    );
                    let connector_config = &metadata_payload.connector_config;

                    //get connector data
                    let connector_data: ConnectorData<DefaultPCIHolder> =
                        ConnectorData::get_connector_by_name(&connector);

                    let connectors = utils::connectors_with_connector_config_overrides(
                        connector_config,
                        &config,
                    )
                    .into_grpc_status()?;

                    // Create common request data
                    let payment_flow_data = PaymentFlowData::foreign_try_from((
                        payload.clone(),
                        connectors,
                        &request_data.masked_metadata,
                    ))
                    .map_err(|e| e.into_grpc_status())?;

                    // Use the existing handle_session_token function
                    let event_params = EventParams {
                        _connector_name: &connector.to_string(),
                        _service_name: &service_name,
                        service_type: utils::service_type_str(&config.server.type_),
                        request_id: &request_id,
                        lineage_ids: &lineage_ids,
                        reference_id: &metadata_payload.reference_id,
                        resource_id: &metadata_payload.resource_id,
                        shadow_mode: metadata_payload.shadow_mode,
                    };

                    let session_response = Box::pin(self.handle_session_token(
                        &config,
                        connector_data.clone(),
                        &payment_flow_data,
                        connector_config.clone(),
                        &payload,
                        &connector.to_string(),
                        &service_name,
                        event_params,
                    ))
                    .await?;

                    tracing::info!(
                        "Session token created successfully: {}",
                        session_response.session_token
                    );

                    // Create response
                    let session_token_response =
                        MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse {
                            session_token: session_response.session_token,
                            error: None,
                            status_code: 200u16.into(),
                        };

                    Ok(tonic::Response::new(session_token_response))
                })
            },
        )
        .await
    }

    #[tracing::instrument(
        name = "server_authentication_token",
        fields(
            name = common_utils::consts::NAME,
            service_name = common_utils::consts::PAYMENT_SERVICE_NAME,
            service_method = FlowName::ServerAuthenticationToken.as_str(),
            request_body = tracing::field::Empty,
            response_body = tracing::field::Empty,
            error_message = tracing::field::Empty,
            merchant_id = tracing::field::Empty,
            gateway = tracing::field::Empty,
            request_id = tracing::field::Empty,
            status_code = tracing::field::Empty,
            message_ = "Golden Log Line (incoming)",
            response_time = tracing::field::Empty,
            tenant_id = tracing::field::Empty,
            flow = FlowName::ServerAuthenticationToken.as_str(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn create_server_authentication_token(
        &self,
        request: tonic::Request<
            MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest,
        >,
    ) -> Result<
        tonic::Response<MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
        tonic::Status,
    > {
        tracing::info!("ACCESS_TOKEN_FLOW: initiated");
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "PaymentService".to_string());
        let config = get_config_from_request(&request)?;
        grpc_logging_wrapper(
            request,
            &service_name,
            config.clone(),
            FlowName::ServerAuthenticationToken,
            |request_data| {
                let service_name = service_name.clone();
                Box::pin(async move {
                    let metadata_payload = request_data.extracted_metadata;
                    let (connector, request_id, lineage_ids) = (
                        metadata_payload.connector,
                        metadata_payload.request_id,
                        metadata_payload.lineage_ids,
                    );
                    let connector_config = &metadata_payload.connector_config;

                    // Get connector data
                    let connector_data: ConnectorData<DefaultPCIHolder> =
                        ConnectorData::get_connector_by_name(&connector);
                    let access_token_create_request = request_data.payload;
                    let connectors = utils::connectors_with_connector_config_overrides(
                        connector_config,
                        &config,
                    )
                    .into_grpc_status()?;

                    // Create minimal payment flow data for access token generation
                    let payment_flow_data = PaymentFlowData::foreign_try_from((
                        access_token_create_request,
                        connectors,
                        &request_data.masked_metadata,
                    ))
                    .map_err(|e| e.into_grpc_status())?;

                    // Create event params for the handle_access_token function
                    let event_params = EventParams {
                        _connector_name: &connector.to_string(),
                        _service_name: &service_name,
                        service_type: utils::service_type_str(&config.server.type_),
                        request_id: &request_id,
                        lineage_ids: &lineage_ids,
                        reference_id: &metadata_payload.reference_id,
                        resource_id: &metadata_payload.resource_id,
                        shadow_mode: metadata_payload.shadow_mode,
                    };

                    // Reuse the existing handle_access_token function which now uses
                    // generate_access_token_response for consistent error handling
                    let server_auth_token_response = Box::pin(self.handle_access_token(
                        &config,
                        connector_data,
                        &payment_flow_data,
                        connector_config.clone(),
                        &connector.to_string(),
                        &service_name,
                        event_params,
                    ))
                    .await?;

                    Ok(tonic::Response::new(server_auth_token_response))
                })
            },
        )
        .await
    }
}

impl RecurringPaymentOperational for RecurringPayments {
    implement_connector_operation!(
        fn_name: internal_mandate_revoke,
        log_prefix: "MANDATE_REVOKE",
        request_type: RecurringPaymentServiceRevokeRequest,
        response_type: RecurringPaymentServiceRevokeResponse,
        flow_marker: MandateRevoke,
        resource_common_data_type: PaymentFlowData,
        request_data_type: MandateRevokeRequestData,
        response_data_type: MandateRevokeResponseData,
        request_data_constructor: MandateRevokeRequestData::foreign_try_from,
        common_flow_data_constructor: PaymentFlowData::foreign_try_from,
        generate_response_fn: generate_mandate_revoke_response,
        all_keys_required: None
    );
}

#[tonic::async_trait]
impl RecurringPaymentService for RecurringPayments {
    #[tracing::instrument(
        name = "charge",
        fields(
            name = common_utils::consts::NAME,
            service_name = common_utils::consts::PAYMENT_SERVICE_NAME,
            service_method = FlowName::RepeatPayment.as_str(),
            request_body = tracing::field::Empty,
            response_body = tracing::field::Empty,
            error_message = tracing::field::Empty,
            merchant_id = tracing::field::Empty,
            gateway = tracing::field::Empty,
            request_id = tracing::field::Empty,
            status_code = tracing::field::Empty,
            message_ = "Golden Log Line (incoming)",
            response_time = tracing::field::Empty,
            tenant_id = tracing::field::Empty,
        ),
        skip(self, request)
    )]
    async fn charge(
        &self,
        request: tonic::Request<RecurringPaymentServiceChargeRequest>,
    ) -> Result<tonic::Response<RecurringPaymentServiceChargeResponse>, tonic::Status> {
        info!("CHARGE_FLOW: initiated");
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "PaymentService".to_string());
        let config = get_config_from_request(&request)?;
        grpc_logging_wrapper(
            request,
            &service_name,
            config.clone(),
            FlowName::RepeatPayment,
            |request_data| {
                let service_name = service_name.clone();
                Box::pin(async move {
                    let payload = request_data.payload;
                    let metadata_payload = request_data.extracted_metadata;
                    let (connector, request_id, lineage_ids) = (
                        metadata_payload.connector,
                        metadata_payload.request_id,
                        metadata_payload.lineage_ids,
                    );
                    let connector_config = &metadata_payload.connector_config;

                    //get connector data
                    let connector_data: ConnectorData<DefaultPCIHolder> =
                        ConnectorData::get_connector_by_name(&connector);

                    // Get connector integration
                    let connector_integration: BoxedConnectorIntegrationV2<
                        '_,
                        RepeatPayment,
                        PaymentFlowData,
                        RepeatPaymentData<DefaultPCIHolder>,
                        PaymentsResponseData,
                    > = connector_data.connector.get_connector_integration_v2();

                    let connectors = utils::connectors_with_connector_config_overrides(
                        &metadata_payload.connector_config,
                        &config,
                    )
                    .into_grpc_status()?;

                    // Create payment flow data
                    let payment_flow_data = PaymentFlowData::foreign_try_from((
                        payload.clone(),
                        connectors,
                        &request_data.masked_metadata,
                    ))
                    .map_err(|e| e.into_grpc_status())?;

                    // Create repeat payment data
                    let repeat_payment_data = RepeatPaymentData::foreign_try_from(payload.clone())
                        .map_err(|e| e.into_grpc_status())?;

                    // Create router data
                    let router_data: RouterDataV2<
                        RepeatPayment,
                        PaymentFlowData,
                        RepeatPaymentData<DefaultPCIHolder>,
                        PaymentsResponseData,
                    > = RouterDataV2 {
                        flow: std::marker::PhantomData,
                        resource_common_data: payment_flow_data,
                        connector_config: connector_config.clone(),
                        request: repeat_payment_data.clone(),
                        response: Err(ErrorResponse::default()),
                    };
                    // Get API tag for RepeatPayment flow
                    let api_tag = config.api_tags.get_tag(
                        FlowName::RepeatPayment,
                        repeat_payment_data.payment_method_type,
                    );

                    // Create test context if test mode is enabled
                    let test_context =
                        config.test.create_test_context(&request_id).map_err(|e| {
                            tonic::Status::internal(format!("Test mode configuration error: {e}"))
                        })?;

                    let event_params = EventProcessingParams {
                        connector_name: &connector.to_string(),
                        service_name: &service_name,
                        service_type: utils::service_type_str(&config.server.type_),
                        flow_name: FlowName::RepeatPayment,
                        event_config: &config.events,
                        request_id: &request_id,
                        lineage_ids: &lineage_ids,
                        reference_id: &metadata_payload.reference_id,
                        resource_id: &metadata_payload.resource_id,
                        shadow_mode: metadata_payload.shadow_mode,
                    };

                    let response = Box::pin(
                        external_services::service::execute_connector_processing_step(
                            &config.proxy,
                            connector_integration,
                            router_data,
                            None,
                            event_params,
                            None, // token_data - None for non-proxy payments
                            common_enums::CallConnectorAction::Trigger,
                            test_context,
                            api_tag,
                        ),
                    )
                    .await
                    .map_err(|e| e.into_grpc_status())?;

                    // Generate response
                    let repeat_payment_response = generate_repeat_payment_response(response)
                        .map_err(|e| e.into_grpc_status())?;

                    Ok(tonic::Response::new(repeat_payment_response))
                })
            },
        )
        .await
    }

    #[tracing::instrument(
        name = "revoke",
        fields(
            name = common_utils::consts::NAME,
            service_name = common_utils::consts::PAYMENT_SERVICE_NAME,
            service_method = FlowName::MandateRevoke.as_str(),
            request_body = tracing::field::Empty,
            response_body = tracing::field::Empty,
            error_message = tracing::field::Empty,
            merchant_id = tracing::field::Empty,
            gateway = tracing::field::Empty,
            request_id = tracing::field::Empty,
            status_code = tracing::field::Empty,
            message_ = "Golden Log Line (incoming)",
            response_time = tracing::field::Empty,
            tenant_id = tracing::field::Empty,
            flow = FlowName::MandateRevoke.as_str(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn revoke(
        &self,
        request: tonic::Request<RecurringPaymentServiceRevokeRequest>,
    ) -> Result<tonic::Response<RecurringPaymentServiceRevokeResponse>, tonic::Status> {
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "PaymentService".to_string());
        let config = get_config_from_request(&request)?;
        grpc_logging_wrapper(
            request,
            &service_name,
            config.clone(),
            FlowName::Authenticate,
            |request_data| async move { self.internal_mandate_revoke(request_data).await },
        )
        .await
    }
}

impl PaymentMethodAuthOperational for PaymentMethodAuthentication {
    implement_connector_operation!(
        fn_name: internal_pre_authenticate,
        log_prefix: "PRE_AUTHENTICATE",
        request_type: PaymentMethodAuthenticationServicePreAuthenticateRequest,
        response_type: PaymentMethodAuthenticationServicePreAuthenticateResponse,
        flow_marker: PreAuthenticate,
        resource_common_data_type: PaymentFlowData,
        request_data_type: PaymentsPreAuthenticateData<DefaultPCIHolder>,
        response_data_type: PaymentsResponseData,
        request_data_constructor: PaymentsPreAuthenticateData::foreign_try_from,
        common_flow_data_constructor: PaymentFlowData::foreign_try_from,
        generate_response_fn: generate_payment_pre_authenticate_response,
        all_keys_required: None
    );

    implement_connector_operation!(
        fn_name: internal_authenticate,
        log_prefix: "AUTHENTICATE",
        request_type: PaymentMethodAuthenticationServiceAuthenticateRequest,
        response_type: PaymentMethodAuthenticationServiceAuthenticateResponse,
        flow_marker: Authenticate,
        resource_common_data_type: PaymentFlowData,
        request_data_type: PaymentsAuthenticateData<DefaultPCIHolder>,
        response_data_type: PaymentsResponseData,
        request_data_constructor: PaymentsAuthenticateData::foreign_try_from,
        common_flow_data_constructor: PaymentFlowData::foreign_try_from,
        generate_response_fn: generate_payment_authenticate_response,
        all_keys_required: None
    );

    implement_connector_operation!(
        fn_name: internal_post_authenticate,
        log_prefix: "POST_AUTHENTICATE",
        request_type: PaymentMethodAuthenticationServicePostAuthenticateRequest,
        response_type: PaymentMethodAuthenticationServicePostAuthenticateResponse,
        flow_marker: PostAuthenticate,
        resource_common_data_type: PaymentFlowData,
        request_data_type: PaymentsPostAuthenticateData<DefaultPCIHolder>,
        response_data_type: PaymentsResponseData,
        request_data_constructor: PaymentsPostAuthenticateData::foreign_try_from,
        common_flow_data_constructor: PaymentFlowData::foreign_try_from,
        generate_response_fn: generate_payment_post_authenticate_response,
        all_keys_required: None
    );
}

#[tonic::async_trait]
impl PaymentMethodAuthenticationService for PaymentMethodAuthentication {
    #[tracing::instrument(
        name = "pre_authenticate",
        fields(
            name = common_utils::consts::NAME,
            service_name = common_utils::consts::PAYMENT_SERVICE_NAME,
            service_method = FlowName::PreAuthenticate.as_str(),
            request_body = tracing::field::Empty,
            response_body = tracing::field::Empty,
            error_message = tracing::field::Empty,
            merchant_id = tracing::field::Empty,
            gateway = tracing::field::Empty,
            request_id = tracing::field::Empty,
            status_code = tracing::field::Empty,
            message_ = "Golden Log Line (incoming)",
            response_time = tracing::field::Empty,
            tenant_id = tracing::field::Empty,
            flow = FlowName::PreAuthenticate.as_str(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn pre_authenticate(
        &self,
        request: tonic::Request<PaymentMethodAuthenticationServicePreAuthenticateRequest>,
    ) -> Result<
        tonic::Response<PaymentMethodAuthenticationServicePreAuthenticateResponse>,
        tonic::Status,
    > {
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "PaymentService".to_string());
        let config = get_config_from_request(&request)?;
        Box::pin(grpc_logging_wrapper(
            request,
            &service_name,
            config.clone(),
            FlowName::PreAuthenticate,
            |request_data| async move { self.internal_pre_authenticate(request_data).await },
        ))
        .await
    }

    #[tracing::instrument(
        name = "authenticate",
        fields(
            name = common_utils::consts::NAME,
            service_name = common_utils::consts::PAYMENT_SERVICE_NAME,
            service_method = FlowName::Authenticate.as_str(),
            request_body = tracing::field::Empty,
            response_body = tracing::field::Empty,
            error_message = tracing::field::Empty,
            merchant_id = tracing::field::Empty,
            gateway = tracing::field::Empty,
            request_id = tracing::field::Empty,
            status_code = tracing::field::Empty,
            message_ = "Golden Log Line (incoming)",
            response_time = tracing::field::Empty,
            tenant_id = tracing::field::Empty,
            flow = FlowName::Authenticate.as_str(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn authenticate(
        &self,
        request: tonic::Request<PaymentMethodAuthenticationServiceAuthenticateRequest>,
    ) -> Result<
        tonic::Response<PaymentMethodAuthenticationServiceAuthenticateResponse>,
        tonic::Status,
    > {
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "PaymentService".to_string());
        let config = get_config_from_request(&request)?;
        Box::pin(grpc_logging_wrapper(
            request,
            &service_name,
            config.clone(),
            FlowName::Authenticate,
            |request_data| async move { self.internal_authenticate(request_data).await },
        ))
        .await
    }

    #[tracing::instrument(
        name = "post_authenticate",
        fields(
            name = common_utils::consts::NAME,
            service_name = common_utils::consts::PAYMENT_SERVICE_NAME,
            service_method = FlowName::PostAuthenticate.as_str(),
            request_body = tracing::field::Empty,
            response_body = tracing::field::Empty,
            error_message = tracing::field::Empty,
            merchant_id = tracing::field::Empty,
            gateway = tracing::field::Empty,
            request_id = tracing::field::Empty,
            status_code = tracing::field::Empty,
            message_ = "Golden Log Line (incoming)",
            response_time = tracing::field::Empty,
            tenant_id = tracing::field::Empty,
            flow = FlowName::PostAuthenticate.as_str(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn post_authenticate(
        &self,
        request: tonic::Request<PaymentMethodAuthenticationServicePostAuthenticateRequest>,
    ) -> Result<
        tonic::Response<PaymentMethodAuthenticationServicePostAuthenticateResponse>,
        tonic::Status,
    > {
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "PaymentService".to_string());
        let config = get_config_from_request(&request)?;
        Box::pin(grpc_logging_wrapper(
            request,
            &service_name,
            config.clone(),
            FlowName::PostAuthenticate,
            |request_data| async move { self.internal_post_authenticate(request_data).await },
        ))
        .await
    }
}

pub fn generate_mandate_revoke_response(
    router_data_v2: RouterDataV2<
        MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        MandateRevokeResponseData,
    >,
) -> Result<RecurringPaymentServiceRevokeResponse, error_stack::Report<ConnectorError>> {
    let mandate_revoke_response = router_data_v2.response;
    let raw_connector_response = router_data_v2
        .resource_common_data
        .get_raw_connector_response();
    let raw_connector_request = router_data_v2
        .resource_common_data
        .get_raw_connector_request();
    let response_headers = router_data_v2
        .resource_common_data
        .get_connector_response_headers_as_map();
    match mandate_revoke_response {
        Ok(response) => Ok(RecurringPaymentServiceRevokeResponse {
            status: match response.mandate_status {
                common_enums::MandateStatus::Active => {
                    grpc_api_types::payments::MandateStatus::Active
                }
                common_enums::MandateStatus::Inactive => {
                    grpc_api_types::payments::MandateStatus::MandateInactive
                }
                common_enums::MandateStatus::Pending => {
                    grpc_api_types::payments::MandateStatus::MandatePending
                }
                common_enums::MandateStatus::Revoked => {
                    grpc_api_types::payments::MandateStatus::Revoked
                }
            }
            .into(),
            error: None,
            status_code: response.status_code.into(),
            response_headers,
            network_transaction_id: None,
            merchant_revoke_id: None,
            raw_connector_response,
            raw_connector_request,
        }),
        Err(e) => Ok(RecurringPaymentServiceRevokeResponse {
            status: grpc_api_types::payments::MandateStatus::MandateRevokeFailed.into(), // Default status for failed revoke
            error: Some(grpc_api_types::payments::ErrorInfo {
                unified_details: None,
                connector_details: Some(grpc_api_types::payments::ConnectorErrorDetails {
                    code: Some(e.code),
                    message: Some(e.message.clone()),
                    reason: e.reason.clone(),
                }),
                issuer_details: None,
            }),
            status_code: e.status_code.into(),
            response_headers,
            network_transaction_id: None,
            merchant_revoke_id: e.connector_transaction_id,
            raw_connector_response,
            raw_connector_request,
        }),
    }
}
