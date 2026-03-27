use std::{collections::HashMap, fmt::Debug, sync::Arc};

use common_utils::{
    errors::CustomResult, events::FlowName, lineage, metadata::MaskedMetadata, SecretSerdeValue,
};
use connector_integration::types::ConnectorData;
use domain_types::connector_types::ConnectorEnum;
use domain_types::{
    connector_flow::{
        Authenticate, Authorize, Capture, CreateAccessToken, CreateConnectorCustomer, CreateOrder,
        CreateSessionToken, IncrementalAuthorization, MandateRevoke, PSync, PaymentMethodToken,
        PostAuthenticate, PreAuthenticate, Refund, RepeatPayment, SdkSessionToken, SetupMandate,
        VerifyWebhookSource, Void, VoidPC,
    },
    connector_types::{
        AccessTokenRequestData, AccessTokenResponseData, ConnectorCustomerData,
        ConnectorCustomerResponse, ConnectorResponseHeaders, MandateRevokeRequestData,
        MandateRevokeResponseData, PaymentCreateOrderData, PaymentCreateOrderResponse,
        PaymentFlowData, PaymentMethodTokenResponse, PaymentMethodTokenizationData,
        PaymentVoidData, PaymentsAuthenticateData, PaymentsAuthorizeData,
        PaymentsCancelPostCaptureData, PaymentsCaptureData, PaymentsIncrementalAuthorizationData,
        PaymentsPostAuthenticateData, PaymentsPreAuthenticateData, PaymentsResponseData,
        PaymentsSdkSessionTokenData, PaymentsSyncData, RawConnectorRequestResponse, RefundFlowData,
        RefundsData, RefundsResponseData, RepeatPaymentData, SessionTokenRequestData,
        SessionTokenResponseData, SetupMandateRequestData, VerifyWebhookSourceFlowData,
    },
    errors::{ApiError, ApplicationErrorResponse},
    payment_method_data::{DefaultPCIHolder, PaymentMethodDataTypes, VaultTokenHolder},
    router_data::{ConnectorSpecificAuth, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_request_types::VerifyWebhookSourceRequestData,
    router_response_types,
    router_response_types::{VerifyWebhookSourceResponseData, VerifyWebhookStatus},
    types::{
        generate_payment_capture_response, generate_payment_incremental_authorization_response,
        generate_payment_sdk_session_token_response, generate_payment_sync_response,
        generate_payment_void_post_capture_response, generate_payment_void_response,
        generate_refund_response, generate_repeat_payment_response,
        generate_setup_mandate_response,
    },
    utils::{ForeignFrom, ForeignTryFrom},
};
use error_stack::ResultExt;
use external_services::service::EventProcessingParams;
use grpc_api_types::payments::{
    payment_method, payment_service_server::PaymentService, DisputeResponse,
    PaymentServiceAuthenticateRequest, PaymentServiceAuthenticateResponse,
    PaymentServiceAuthorizeOnlyRequest, PaymentServiceAuthorizeRequest,
    PaymentServiceAuthorizeResponse, PaymentServiceCaptureRequest, PaymentServiceCaptureResponse,
    PaymentServiceCreateAccessTokenRequest, PaymentServiceCreateAccessTokenResponse,
    PaymentServiceCreateOrderRequest, PaymentServiceCreateOrderResponse,
    PaymentServiceCreatePaymentMethodTokenRequest, PaymentServiceCreatePaymentMethodTokenResponse,
    PaymentServiceCreateSessionTokenRequest, PaymentServiceCreateSessionTokenResponse,
    PaymentServiceDisputeRequest, PaymentServiceGetRequest, PaymentServiceGetResponse,
    PaymentServiceIncrementalAuthorizationRequest, PaymentServiceIncrementalAuthorizationResponse,
    PaymentServicePostAuthenticateRequest, PaymentServicePostAuthenticateResponse,
    PaymentServicePreAuthenticateRequest, PaymentServicePreAuthenticateResponse,
    PaymentServiceRefundRequest, PaymentServiceRegisterRequest, PaymentServiceRegisterResponse,
    PaymentServiceRepeatEverythingRequest, PaymentServiceRepeatEverythingResponse,
    PaymentServiceRevokeMandateRequest, PaymentServiceRevokeMandateResponse,
    PaymentServiceSdkSessionTokenRequest, PaymentServiceSdkSessionTokenResponse,
    PaymentServiceTransformRequest, PaymentServiceTransformResponse,
    PaymentServiceVerifyRedirectResponseRequest, PaymentServiceVerifyRedirectResponseResponse,
    PaymentServiceVoidPostCaptureRequest, PaymentServiceVoidPostCaptureResponse,
    PaymentServiceVoidRequest, PaymentServiceVoidResponse, RefundResponse,
    WebhookTransformationStatus,
};
use hyperswitch_masking::ExposeInterface;
use hyperswitch_masking::Secret;
use injector::{TokenData, VaultConnectors};
use interfaces::{
    connector_integration_v2::BoxedConnectorIntegrationV2,
    verification::ConnectorSourceVerificationSecrets,
};
use tracing::info;

use crate::{
    configs::Config,
    error::{
        ErrorSwitch, IntoGrpcStatus, PaymentAuthorizationError, ReportSwitchExt, ResultExtGrpc,
    },
    implement_connector_operation,
    request::RequestData,
    utils::{self, get_config_from_request, grpc_logging_wrapper},
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
        request: RequestData<PaymentServiceVoidPostCaptureRequest>,
    ) -> Result<tonic::Response<PaymentServiceVoidPostCaptureResponse>, tonic::Status>;

    async fn internal_refund(
        &self,
        request: RequestData<PaymentServiceRefundRequest>,
    ) -> Result<tonic::Response<RefundResponse>, tonic::Status>;

    async fn internal_payment_capture(
        &self,
        request: RequestData<PaymentServiceCaptureRequest>,
    ) -> Result<tonic::Response<PaymentServiceCaptureResponse>, tonic::Status>;

    async fn internal_sdk_session_token(
        &self,
        request: RequestData<PaymentServiceSdkSessionTokenRequest>,
    ) -> Result<tonic::Response<PaymentServiceSdkSessionTokenResponse>, tonic::Status>;

    async fn internal_mandate_revoke(
        &self,
        request: RequestData<PaymentServiceRevokeMandateRequest>,
    ) -> Result<tonic::Response<PaymentServiceRevokeMandateResponse>, tonic::Status>;

    async fn internal_pre_authenticate(
        &self,
        request: RequestData<PaymentServicePreAuthenticateRequest>,
    ) -> Result<tonic::Response<PaymentServicePreAuthenticateResponse>, tonic::Status>;

    async fn internal_authenticate(
        &self,
        request: RequestData<PaymentServiceAuthenticateRequest>,
    ) -> Result<tonic::Response<PaymentServiceAuthenticateResponse>, tonic::Status>;

    async fn internal_post_authenticate(
        &self,
        request: RequestData<PaymentServicePostAuthenticateRequest>,
    ) -> Result<tonic::Response<PaymentServicePostAuthenticateResponse>, tonic::Status>;

    async fn internal_incremental_authorization(
        &self,
        request: RequestData<PaymentServiceIncrementalAuthorizationRequest>,
    ) -> Result<tonic::Response<PaymentServiceIncrementalAuthorizationResponse>, tonic::Status>;

    async fn internal_create_order(
        &self,
        request: RequestData<PaymentServiceCreateOrderRequest>,
    ) -> Result<tonic::Response<PaymentServiceCreateOrderResponse>, tonic::Status>;
}

#[derive(Clone)]
pub struct Payments;

impl Payments {
    #[allow(clippy::too_many_arguments)]
    async fn handle_access_token_flow<
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
        connector_data: &ConnectorData<T>,
        access_token: Option<&grpc_api_types::payments::AccessToken>,
        payment_flow_data: &PaymentFlowData,
        connector_auth_type: &ConnectorSpecificAuth,
        connector_name: &str,
        service_name: &str,
        event_params: EventParams<'_>,
    ) -> Result<AccessTokenResponseData, tonic::Status> {
        let access_token_result =
            access_token.and_then(|token| AccessTokenResponseData::foreign_try_from(token).ok());

        let access_token_data = match access_token_result {
            Some(cached_access_token) => {
                // If provided cached token - use it, don't generate new one
                tracing::info!("Using cached access from request");
                cached_access_token
            }
            None => {
                // No cached token - generate fresh one
                tracing::info!("No cached access token found, generating new token");

                let access_token_data = Box::pin(self.handle_access_token(
                    config,
                    connector_data.clone(),
                    payment_flow_data,
                    connector_auth_type.clone(),
                    connector_name,
                    service_name,
                    event_params,
                ))
                .await
                .map_err(|e| {
                    let message = e
                        .error_message
                        .unwrap_or_else(|| "Access token creation failed".to_string());
                    tonic::Status::internal(message)
                })?;

                tracing::info!(
                    "Access token created successfully with expiry: {:?}",
                    access_token_data.expires_in
                );

                access_token_data
            }
        };

        Ok(access_token_data)
    }

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
        connector_auth_details: ConnectorSpecificAuth,
        metadata: &MaskedMetadata,
        metadata_payload: &utils::MetadataPayload,
        service_name: &str,
        request_id: &str,
        token_data: Option<TokenData>,
    ) -> Result<PaymentServiceAuthorizeResponse, PaymentAuthorizationError> {
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

        // Create common request data
        let payment_flow_data = PaymentFlowData::foreign_try_from((
            payload.clone(),
            config.connectors.clone(),
            metadata,
        ))
        .map_err(|err| {
            tracing::error!("Failed to process payment flow data: {:?}", err);
            PaymentAuthorizationError::new(
                grpc_api_types::payments::PaymentStatus::Pending,
                Some("Failed to process payment flow data".to_string()),
                Some("PAYMENT_FLOW_ERROR".to_string()),
                None,
            )
        })?;

        let lineage_ids = &metadata_payload.lineage_ids;
        let reference_id = &metadata_payload.reference_id;
        let resource_id = &metadata_payload.resource_id;

        // Extract access token from request
        let cached_access_token = payload
            .state
            .as_ref()
            .and_then(|state| state.access_token.as_ref());

        // Check if connector supports access tokens
        let should_do_access_token = connector_data
            .connector
            .should_do_access_token(payment_flow_data.payment_method);

        // Conditional token generation - ONLY if not provided in request
        let payment_flow_data = if should_do_access_token {
            let event_params = EventParams {
                _connector_name: &connector.to_string(),
                _service_name: service_name,
                service_type: utils::service_type_str(&config.server.type_),
                request_id,
                lineage_ids,
                reference_id,
                resource_id,
                shadow_mode: metadata_payload.shadow_mode,
            };

            let access_token_data = self
                .handle_access_token_flow(
                    config,
                    &connector_data,
                    cached_access_token,
                    &payment_flow_data,
                    &metadata_payload.connector_auth_type,
                    &connector.to_string(),
                    service_name,
                    event_params,
                )
                .await
                .map_err(|err| {
                    tracing::error!("Failed to process payment access token data: {:?}", err);
                    PaymentAuthorizationError::new(
                        grpc_api_types::payments::PaymentStatus::Failure,
                        Some("Failed to process payment access token data".to_string()),
                        Some("ACCESS_TOKEN_ERROR".to_string()),
                        Some(400),
                    )
                })?;

            // Store in flow data for connector API calls
            payment_flow_data.set_access_token(Some(access_token_data))
        } else {
            // Connector doesn't support access tokens
            payment_flow_data
        };

        let should_do_order_create = connector_data.connector.should_do_order_create();

        let payment_flow_data = if should_do_order_create {
            let event_params = EventParams {
                _connector_name: &connector.to_string(),
                _service_name: service_name,
                service_type: utils::service_type_str(&config.server.type_),
                request_id,
                lineage_ids,
                reference_id,
                resource_id,
                shadow_mode: metadata_payload.shadow_mode,
            };

            let order_create_result = Box::pin(self.handle_order_creation(
                config,
                connector_data.clone(),
                &payment_flow_data,
                connector_auth_details.clone(),
                &payload,
                &connector.to_string(),
                service_name,
                event_params,
            ))
            .await?;

            tracing::info!(
                "Order created successfully with order_id: {}",
                order_create_result.order_id
            );

            payment_flow_data.set_order_reference_id(Some(order_create_result.order_id))
        } else {
            payment_flow_data
        };

        let should_do_session_token = connector_data.connector.should_do_session_token();

        let payment_flow_data = if should_do_session_token {
            let event_params = EventParams {
                _connector_name: &connector.to_string(),
                _service_name: service_name,
                service_type: utils::service_type_str(&config.server.type_),
                request_id,
                lineage_ids,
                reference_id,
                resource_id,
                shadow_mode: metadata_payload.shadow_mode,
            };

            let payment_session_data = Box::pin(self.handle_session_token(
                config,
                connector_data.clone(),
                &payment_flow_data,
                connector_auth_details.clone(),
                &payload,
                &connector.to_string(),
                service_name,
                event_params,
            ))
            .await?;
            tracing::info!(
                "Session Token created successfully with session_id: {}",
                payment_session_data.session_token
            );
            payment_flow_data.set_session_token_id(Some(payment_session_data.session_token))
        } else {
            payment_flow_data
        };

        // Extract connector customer ID (if provided by Hyperswitch)
        let cached_connector_customer_id = payload.connector_customer_id.clone();

        // Check if connector supports customer creation
        let should_create_connector_customer =
            connector_data.connector.should_create_connector_customer();

        // Conditional customer creation - ONLY if connector needs it AND no existing customer ID
        let payment_flow_data = if should_create_connector_customer {
            match cached_connector_customer_id {
                Some(_customer_id) => payment_flow_data,
                None => {
                    let event_params = EventParams {
                        _connector_name: &connector.to_string(),
                        _service_name: service_name,
                        service_type: utils::service_type_str(&config.server.type_),
                        request_id,
                        lineage_ids,
                        reference_id,
                        resource_id,
                        shadow_mode: metadata_payload.shadow_mode,
                    };

                    let connector_customer_response = Box::pin(self.handle_connector_customer(
                        config,
                        connector_data.clone(),
                        &payment_flow_data,
                        connector_auth_details.clone(),
                        &payload,
                        &connector.to_string(),
                        service_name,
                        event_params,
                    ))
                    .await?;

                    payment_flow_data.set_connector_customer_id(Some(
                        connector_customer_response.connector_customer_id,
                    ))
                }
            }
        } else {
            // Connector doesn't support customer creation
            payment_flow_data
        };

        // Create connector request data
        let payment_authorize_data = PaymentsAuthorizeData::foreign_try_from(payload.clone())
            .map_err(|err| {
                tracing::error!("Failed to process payment authorize data: {:?}", err);
                PaymentAuthorizationError::new(
                    grpc_api_types::payments::PaymentStatus::Pending,
                    Some("Failed to process payment authorize data".to_string()),
                    Some("PAYMENT_AUTHORIZE_DATA_ERROR".to_string()),
                    None,
                )
            })?
            // Set session token from payment flow data if available
            .set_session_token(payment_flow_data.session_token.clone());

        let should_do_payment_method_token =
            connector_data.connector.should_do_payment_method_token(
                payment_flow_data.payment_method,
                payment_authorize_data.payment_method_type,
            );

        let payment_flow_data = if should_do_payment_method_token {
            let event_params = EventParams {
                _connector_name: &connector.to_string(),
                _service_name: service_name,
                service_type: utils::service_type_str(&config.server.type_),
                request_id,
                lineage_ids,
                reference_id,
                resource_id,
                shadow_mode: metadata_payload.shadow_mode,
            };
            let payment_method_token_data = self
                .handle_payment_method_token(
                    config,
                    connector_data.clone(),
                    &payment_flow_data,
                    connector_auth_details.clone(),
                    event_params,
                    &payment_authorize_data,
                    &connector.to_string(),
                    service_name,
                )
                .await?;
            tracing::info!("Payment Method Token created successfully");
            payment_flow_data.set_payment_method_token(Some(payment_method_token_data.token))
        } else {
            payment_flow_data
        };

        // Construct router data
        let router_data = RouterDataV2::<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        > {
            flow: std::marker::PhantomData,
            resource_common_data: payment_flow_data.clone(),
            connector_auth_type: connector_auth_details.clone(),
            request: payment_authorize_data,
            response: Err(ErrorResponse::default()),
        };

        // Get API tag for the current flow with payment method type from domain layer
        let api_tag = config
            .api_tags
            .get_tag(FlowName::Authorize, router_data.request.payment_method_type);

        // Create test context if test mode is enabled
        let test_context = config.test.create_test_context(request_id).map_err(|e| {
            PaymentAuthorizationError::new(
                grpc_api_types::payments::PaymentStatus::Pending,
                Some(format!("Test mode configuration error: {e}")),
                Some("TEST_CONFIG_ERROR".to_string()),
                None,
            )
        })?;

        // Execute connector processing
        let event_params = EventProcessingParams {
            connector_name: &connector.to_string(),
            service_name,
            service_type: utils::service_type_str(&config.server.type_),
            flow_name: FlowName::Authorize,
            event_config: &config.events,
            request_id,
            lineage_ids,
            reference_id,
            resource_id,
            shadow_mode: metadata_payload.shadow_mode,
        };

        // Execute connector processing
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

        // Generate response - pass both success and error cases
        let authorize_response = match response {
            Ok(success_response) => domain_types::types::generate_payment_authorize_response(
                success_response,
            )
            .map_err(|err| {
                tracing::error!("Failed to generate authorize response: {:?}", err);
                PaymentAuthorizationError::new(
                    grpc_api_types::payments::PaymentStatus::Pending,
                    Some("Failed to generate authorize response".to_string()),
                    Some("RESPONSE_GENERATION_ERROR".to_string()),
                    None,
                )
            })?,
            Err(error_report) => {
                tracing::error!("{:?}", error_report);
                // Convert ConnectorError to ApplicationErrorResponse to get proper error details
                let app_err: ApplicationErrorResponse = error_report.current_context().switch();
                let api_error = app_err.get_api_error();

                // Convert error to RouterDataV2 with error response
                let error_router_data = RouterDataV2 {
                    flow: std::marker::PhantomData,
                    resource_common_data: payment_flow_data,
                    connector_auth_type: connector_auth_details,
                    request: PaymentsAuthorizeData::foreign_try_from(payload.clone()).map_err(
                        |err| {
                            tracing::error!(
                                "Failed to process payment authorize data in error path: {:?}",
                                err
                            );
                            PaymentAuthorizationError::new(
                                grpc_api_types::payments::PaymentStatus::Pending,
                                Some(
                                    "Failed to process payment authorize data in error path"
                                        .to_string(),
                                ),
                                Some("PAYMENT_AUTHORIZE_DATA_ERROR".to_string()),
                                None,
                            )
                        },
                    )?,
                    response: Err(ErrorResponse {
                        status_code: api_error.error_identifier,
                        code: api_error.sub_code.clone(),
                        message: api_error.error_message.clone(),
                        reason: None,
                        attempt_status: Some(common_enums::AttemptStatus::Failure),
                        connector_transaction_id: None,
                        network_decline_code: None,
                        network_advice_code: None,
                        network_error_message: None,
                    }),
                };
                domain_types::types::generate_payment_authorize_response::<T>(error_router_data)
                    .map_err(|err| {
                        tracing::error!(
                            "Failed to generate authorize response for connector error: {:?}",
                            err
                        );
                        PaymentAuthorizationError::new(
                            grpc_api_types::payments::PaymentStatus::Pending,
                            Some(format!("Connector error: {error_report}")),
                            Some("CONNECTOR_ERROR".to_string()),
                            None,
                        )
                    })?
            }
        };

        Ok(authorize_response)
    }

    #[allow(clippy::too_many_arguments)]
    async fn process_authorization_only_internal<
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
        payload: PaymentServiceAuthorizeOnlyRequest,
        connector: ConnectorEnum,
        connector_auth_details: ConnectorSpecificAuth,
        metadata: &MaskedMetadata,
        metadata_payload: &utils::MetadataPayload,
        service_name: &str,
        request_id: &str,
        token_data: Option<TokenData>,
    ) -> Result<PaymentServiceAuthorizeResponse, PaymentAuthorizationError> {
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

        // Create common request data
        let payment_flow_data = PaymentFlowData::foreign_try_from((
            payload.clone(),
            config.connectors.clone(),
            metadata,
        ))
        .map_err(|err| {
            tracing::error!("Failed to process payment flow data: {:?}", err);
            PaymentAuthorizationError::new(
                grpc_api_types::payments::PaymentStatus::Pending,
                Some("Failed to process payment flow data".to_string()),
                Some("PAYMENT_FLOW_ERROR".to_string()),
                None,
            )
        })?;

        // Create connector request data
        let payment_authorize_data = PaymentsAuthorizeData::foreign_try_from(payload.clone())
            .map_err(|err| {
                tracing::error!("Failed to process payment authorize data: {:?}", err);
                PaymentAuthorizationError::new(
                    grpc_api_types::payments::PaymentStatus::Pending,
                    Some("Failed to process payment authorize data".to_string()),
                    Some("PAYMENT_AUTHORIZE_DATA_ERROR".to_string()),
                    None,
                )
            })?;

        // Construct router data
        let router_data = RouterDataV2::<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        > {
            flow: std::marker::PhantomData,
            resource_common_data: payment_flow_data.clone(),
            connector_auth_type: connector_auth_details.clone(),
            request: payment_authorize_data,
            response: Err(ErrorResponse::default()),
        };

        // Get API tag for the current flow with payment method type from domain layer
        let api_tag = config
            .api_tags
            .get_tag(FlowName::Authorize, router_data.request.payment_method_type);

        // Create test context if test mode is enabled
        let test_context = config.test.create_test_context(request_id).map_err(|e| {
            PaymentAuthorizationError::new(
                grpc_api_types::payments::PaymentStatus::Pending,
                Some(format!("Test mode configuration error: {e}")),
                Some("TEST_CONFIG_ERROR".to_string()),
                None,
            )
        })?;

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

        // Generate response - pass both success and error cases
        let authorize_response = match response {
            Ok(success_response) => domain_types::types::generate_payment_authorize_response(
                success_response,
            )
            .map_err(|err| {
                tracing::error!("Failed to generate authorize response: {:?}", err);
                PaymentAuthorizationError::new(
                    grpc_api_types::payments::PaymentStatus::Pending,
                    Some("Failed to generate authorize response".to_string()),
                    Some("RESPONSE_GENERATION_ERROR".to_string()),
                    None,
                )
            })?,
            Err(error_report) => {
                // Convert ConnectorError to ApplicationErrorResponse to get proper error details
                let app_err: ApplicationErrorResponse = error_report.current_context().switch();
                let api_error = app_err.get_api_error();

                // Convert error to RouterDataV2 with error response
                let error_router_data = RouterDataV2 {
                    flow: std::marker::PhantomData,
                    resource_common_data: payment_flow_data,
                    connector_auth_type: connector_auth_details,
                    request: PaymentsAuthorizeData::foreign_try_from(payload.clone()).map_err(
                        |err| {
                            tracing::error!(
                                "Failed to process payment authorize data in error path: {:?}",
                                err
                            );
                            PaymentAuthorizationError::new(
                                grpc_api_types::payments::PaymentStatus::Pending,
                                Some(
                                    "Failed to process payment authorize data in error path"
                                        .to_string(),
                                ),
                                Some("PAYMENT_AUTHORIZE_DATA_ERROR".to_string()),
                                None,
                            )
                        },
                    )?,
                    response: Err(ErrorResponse {
                        status_code: api_error.error_identifier,
                        code: api_error.sub_code.clone(),
                        message: api_error.error_message.clone(),
                        reason: None,
                        attempt_status: Some(common_enums::AttemptStatus::Failure),
                        connector_transaction_id: None,
                        network_decline_code: None,
                        network_advice_code: None,
                        network_error_message: None,
                    }),
                };
                domain_types::types::generate_payment_authorize_response::<T>(error_router_data)
                    .map_err(|err| {
                        tracing::error!(
                            "Failed to generate authorize response for connector error: {:?}",
                            err
                        );
                        PaymentAuthorizationError::new(
                            grpc_api_types::payments::PaymentStatus::Pending,
                            Some(format!("Connector error: {error_report}")),
                            Some("CONNECTOR_ERROR".to_string()),
                            None,
                        )
                    })?
            }
        };

        Ok(authorize_response)
    }

    #[allow(clippy::too_many_arguments)]
    async fn handle_order_creation<
        T: PaymentMethodDataTypes
            + Default
            + Eq
            + Debug
            + Send
            + serde::Serialize
            + serde::de::DeserializeOwned
            + Clone
            + Sync
            + domain_types::types::CardConversionHelper<T>,
    >(
        &self,
        config: &Arc<Config>,
        connector_data: ConnectorData<T>,
        payment_flow_data: &PaymentFlowData,
        connector_auth_details: ConnectorSpecificAuth,
        payload: &PaymentServiceAuthorizeRequest,
        connector_name: &str,
        service_name: &str,
        event_params: EventParams<'_>,
    ) -> Result<PaymentCreateOrderResponse, PaymentAuthorizationError> {
        // Get connector integration
        let connector_integration: BoxedConnectorIntegrationV2<
            '_,
            CreateOrder,
            PaymentFlowData,
            PaymentCreateOrderData,
            PaymentCreateOrderResponse,
        > = connector_data.connector.get_connector_integration_v2();

        let currency =
            common_enums::Currency::foreign_try_from(payload.currency()).map_err(|e| {
                PaymentAuthorizationError::new(
                    grpc_api_types::payments::PaymentStatus::Pending,
                    Some(format!("Currency conversion failed: {e}")),
                    Some("CURRENCY_ERROR".to_string()),
                    None,
                )
            })?;

        let payment_method: grpc_api_types::payments::PaymentMethod =
            payload.payment_method.clone().ok_or_else(|| {
                PaymentAuthorizationError::new(
                    grpc_api_types::payments::PaymentStatus::Pending,
                    Some("Payment method is required".to_string()),
                    Some("PAYMENT_METHOD_REQUIRED".to_string()),
                    None,
                )
            })?;

        let payment_method_type: Option<common_enums::PaymentMethodType> =
            <Option<common_enums::PaymentMethodType>>::foreign_try_from(payment_method).map_err(
                |e| {
                    PaymentAuthorizationError::new(
                        grpc_api_types::payments::PaymentStatus::Pending,
                        Some(format!("Payment method type conversion failed: {e}")),
                        Some("PAYMENT_METHOD_TYPE_ERROR".to_string()),
                        None,
                    )
                },
            )?;

        let order_create_data = PaymentCreateOrderData {
            amount: common_utils::types::MinorUnit::new(payload.minor_amount),
            currency,
            integrity_object: None,
            metadata: payload.metadata.clone().map(|m| {
                let metadata = m.expose();
                let value =
                    serde_json::from_str::<serde_json::Value>(&metadata).unwrap_or_default();
                Secret::new(value)
            }),
            webhook_url: payload.webhook_url.clone(),
            payment_method_type,
        };

        let order_router_data = RouterDataV2::<
            CreateOrder,
            PaymentFlowData,
            PaymentCreateOrderData,
            PaymentCreateOrderResponse,
        > {
            flow: std::marker::PhantomData,
            resource_common_data: payment_flow_data.clone(),
            connector_auth_type: connector_auth_details,
            request: order_create_data,
            response: Err(ErrorResponse::default()),
        };

        // Get API tag for CreateOrder flow
        let api_tag = config.api_tags.get_tag(FlowName::CreateOrder, None);

        // Create test context if test mode is enabled
        let test_context = config
            .test
            .create_test_context(event_params.request_id)
            .map_err(|e| {
                PaymentAuthorizationError::new(
                    grpc_api_types::payments::PaymentStatus::Pending,
                    Some(format!("Test mode configuration error: {e}")),
                    Some("TEST_CONFIG_ERROR".to_string()),
                    None,
                )
            })?;

        // Create event processing parameters
        let external_event_params = EventProcessingParams {
            connector_name,
            service_name,
            service_type: event_params.service_type,
            flow_name: FlowName::CreateOrder,
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
                order_router_data,
                None,
                external_event_params,
                None,
                common_enums::CallConnectorAction::Trigger,
                test_context,
                api_tag,
            ),
        )
        .await
        .map_err(
            |e: error_stack::Report<domain_types::errors::ConnectorError>| {
                PaymentAuthorizationError::new(
                    grpc_api_types::payments::PaymentStatus::Pending,
                    Some(format!("Order creation failed: {e}")),
                    Some("ORDER_CREATION_ERROR".to_string()),
                    None,
                )
            },
        )?;

        match response.response {
            Ok(PaymentCreateOrderResponse {
                order_id,
                session_token,
            }) => Ok(PaymentCreateOrderResponse {
                order_id,
                session_token,
            }),
            Err(e) => Err(PaymentAuthorizationError::new(
                grpc_api_types::payments::PaymentStatus::Pending,
                Some(e.message.clone()),
                Some(e.code.clone()),
                Some(e.status_code.into()),
            )),
        }
    }
    #[allow(clippy::too_many_arguments)]
    async fn handle_order_creation_for_setup_mandate<
        T: PaymentMethodDataTypes
            + Default
            + Eq
            + Debug
            + Send
            + serde::Serialize
            + serde::de::DeserializeOwned
            + Clone
            + Sync
            + domain_types::types::CardConversionHelper<T>,
    >(
        &self,
        config: &Arc<Config>,
        connector_data: ConnectorData<T>,
        payment_flow_data: &PaymentFlowData,
        connector_auth_details: ConnectorSpecificAuth,
        event_params: EventParams<'_>,
        payload: &PaymentServiceRegisterRequest,
        connector_name: &str,
        service_name: &str,
    ) -> Result<String, tonic::Status> {
        // Get connector integration
        let connector_integration: BoxedConnectorIntegrationV2<
            '_,
            CreateOrder,
            PaymentFlowData,
            PaymentCreateOrderData,
            PaymentCreateOrderResponse,
        > = connector_data.connector.get_connector_integration_v2();

        let currency = common_enums::Currency::foreign_try_from(payload.currency())
            .map_err(|e| e.into_grpc_status())?;

        let order_create_data = PaymentCreateOrderData {
            amount: common_utils::types::MinorUnit::new(0),
            currency,
            integrity_object: None,
            metadata: payload.metadata.clone().map(|m| {
                let metadata = m.expose();
                let value =
                    serde_json::from_str::<serde_json::Value>(&metadata).unwrap_or_default();
                Secret::new(value)
            }),
            webhook_url: payload.webhook_url.clone(),
            // Setup mandate flow doesn't use wallets, so payment_method_type is not applicable
            payment_method_type: None,
        };

        let order_router_data = RouterDataV2::<
            CreateOrder,
            PaymentFlowData,
            PaymentCreateOrderData,
            PaymentCreateOrderResponse,
        > {
            flow: std::marker::PhantomData,
            resource_common_data: payment_flow_data.clone(),
            connector_auth_type: connector_auth_details,
            request: order_create_data,
            response: Err(ErrorResponse::default()),
        };

        // Get API tag for CreateOrder flow
        let api_tag = config.api_tags.get_tag(FlowName::CreateOrder, None);

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
            flow_name: FlowName::CreateOrder,
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
                order_router_data,
                None,
                external_event_params,
                None,
                common_enums::CallConnectorAction::Trigger,
                test_context,
                api_tag,
            ),
        )
        .await
        .switch()
        .map_err(|e| e.into_grpc_status())?;

        match response.response {
            Ok(PaymentCreateOrderResponse { order_id, .. }) => Ok(order_id),
            Err(ErrorResponse { message, .. }) => Err(tonic::Status::internal(format!(
                "Order creation error: {message}"
            ))),
        }
    }

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
        connector_auth_details: ConnectorSpecificAuth,
        payload: &P,
        connector_name: &str,
        service_name: &str,
        event_params: EventParams<'_>,
    ) -> Result<SessionTokenResponseData, PaymentAuthorizationError>
    where
        SessionTokenRequestData: ForeignTryFrom<P, Error = ApplicationErrorResponse>,
    {
        // Get connector integration
        let connector_integration: BoxedConnectorIntegrationV2<
            '_,
            CreateSessionToken,
            PaymentFlowData,
            SessionTokenRequestData,
            SessionTokenResponseData,
        > = connector_data.connector.get_connector_integration_v2();

        // Create session token request data using try_from_foreign
        let session_token_request_data = SessionTokenRequestData::foreign_try_from(payload.clone())
            .map_err(|e| {
                PaymentAuthorizationError::new(
                    grpc_api_types::payments::PaymentStatus::Pending,
                    Some(format!("Session Token creation failed: {e}")),
                    Some("SESSION_TOKEN_CREATION_ERROR".to_string()),
                    Some(400), // Bad Request - client data issue
                )
            })?;

        let session_token_router_data = RouterDataV2::<
            CreateSessionToken,
            PaymentFlowData,
            SessionTokenRequestData,
            SessionTokenResponseData,
        > {
            flow: std::marker::PhantomData,
            resource_common_data: payment_flow_data.clone(),
            connector_auth_type: connector_auth_details,
            request: session_token_request_data,
            response: Err(ErrorResponse::default()),
        };

        // Get API tag for CreateSessionToken flow with payment method type if available
        let api_tag = config.api_tags.get_tag(FlowName::CreateSessionToken, None);

        // Create test context if test mode is enabled
        let test_context = config
            .test
            .create_test_context(event_params.request_id)
            .map_err(|e| {
                PaymentAuthorizationError::new(
                    grpc_api_types::payments::PaymentStatus::Pending,
                    Some(format!("Test mode configuration error: {e}")),
                    Some("TEST_CONFIG_ERROR".to_string()),
                    None,
                )
            })?;

        // Create event processing parameters
        let external_event_params = EventProcessingParams {
            connector_name,
            service_name,
            service_type: event_params.service_type,
            flow_name: FlowName::CreateSessionToken,
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
        .switch()
        .map_err(|e: error_stack::Report<ApplicationErrorResponse>| {
            PaymentAuthorizationError::new(
                grpc_api_types::payments::PaymentStatus::Pending,
                Some(format!("Session Token creation failed: {e}")),
                Some("SESSION_TOKEN_CREATION_ERROR".to_string()),
                Some(500), // Internal Server Error - connector processing failed
            )
        })?;

        match response.response {
            Ok(session_token_data) => {
                tracing::info!(
                    "Session token created successfully: {}",
                    session_token_data.session_token
                );
                Ok(session_token_data)
            }
            Err(ErrorResponse {
                message,
                status_code,
                ..
            }) => Err(PaymentAuthorizationError::new(
                grpc_api_types::payments::PaymentStatus::Pending,
                Some(format!("Session Token creation failed: {message}")),
                Some("SESSION_TOKEN_CREATION_ERROR".to_string()),
                Some(status_code.into()), // Use actual status code from ErrorResponse
            )),
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
        connector_auth_details: ConnectorSpecificAuth,
        connector_name: &str,
        service_name: &str,
        event_params: EventParams<'_>,
    ) -> Result<AccessTokenResponseData, PaymentAuthorizationError>
    where
        AccessTokenRequestData:
            for<'a> ForeignTryFrom<&'a ConnectorSpecificAuth, Error = ApplicationErrorResponse>,
    {
        // Get connector integration for CreateAccessToken flow
        let connector_integration: BoxedConnectorIntegrationV2<
            '_,
            CreateAccessToken,
            PaymentFlowData,
            AccessTokenRequestData,
            AccessTokenResponseData,
        > = connector_data.connector.get_connector_integration_v2();

        // Create access token request data - grant type determined by connector
        let access_token_request_data = AccessTokenRequestData::foreign_try_from(
            &connector_auth_details, // Contains connector-specific auth details
        )
        .map_err(|e| {
            PaymentAuthorizationError::new(
                grpc_api_types::payments::PaymentStatus::Pending,
                Some(format!("Failed to create access token request: {e}")),
                Some("ACCESS_TOKEN_REQUEST_ERROR".to_string()),
                Some(400),
            )
        })?;

        // Create router data for access token flow
        let access_token_router_data = RouterDataV2::<
            CreateAccessToken,
            PaymentFlowData,
            AccessTokenRequestData,
            AccessTokenResponseData,
        > {
            flow: std::marker::PhantomData,
            resource_common_data: payment_flow_data.clone(),
            connector_auth_type: connector_auth_details,
            request: access_token_request_data,
            response: Err(ErrorResponse::default()),
        };

        // Get API tag for CreateAccessToken flow with payment method type if available
        let api_tag = config.api_tags.get_tag(FlowName::CreateAccessToken, None);

        // Create test context if test mode is enabled
        let test_context = config
            .test
            .create_test_context(event_params.request_id)
            .map_err(|e| {
                PaymentAuthorizationError::new(
                    grpc_api_types::payments::PaymentStatus::Pending,
                    Some(format!("Test mode configuration error: {e}")),
                    Some("TEST_CONFIG_ERROR".to_string()),
                    None,
                )
            })?;

        // Execute connector processing
        let external_event_params = EventProcessingParams {
            connector_name,
            service_name,
            service_type: event_params.service_type,
            flow_name: FlowName::CreateAccessToken,
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
        .switch()
        .map_err(|e: error_stack::Report<ApplicationErrorResponse>| {
            PaymentAuthorizationError::new(
                grpc_api_types::payments::PaymentStatus::Pending,
                Some(format!("Access Token creation failed: {e}")),
                Some("ACCESS_TOKEN_CREATION_ERROR".to_string()),
                Some(500),
            )
        })?;

        match response.response {
            Ok(access_token_data) => {
                tracing::info!(
                    "Access token created successfully with expiry: {:?}",
                    access_token_data.expires_in
                );
                Ok(access_token_data)
            }
            Err(ErrorResponse {
                message,
                status_code,
                ..
            }) => Err(PaymentAuthorizationError::new(
                grpc_api_types::payments::PaymentStatus::Pending,
                Some(format!("Access Token creation failed: {message}")),
                Some("ACCESS_TOKEN_CREATION_ERROR".to_string()),
                Some(status_code.into()),
            )),
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn handle_connector_customer<
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
        connector_auth_details: ConnectorSpecificAuth,
        payload: &PaymentServiceAuthorizeRequest,
        connector_name: &str,
        service_name: &str,
        event_params: EventParams<'_>,
    ) -> Result<ConnectorCustomerResponse, PaymentAuthorizationError> {
        // Get connector integration for CreateConnectorCustomer flow
        let connector_integration: BoxedConnectorIntegrationV2<
            '_,
            CreateConnectorCustomer,
            PaymentFlowData,
            ConnectorCustomerData,
            ConnectorCustomerResponse,
        > = connector_data.connector.get_connector_integration_v2();

        // Create connector customer request data using ForeignTryFrom
        let connector_customer_request_data =
            ConnectorCustomerData::foreign_try_from(payload.clone()).map_err(|err| {
                tracing::error!("Failed to process connector customer data: {:?}", err);
                PaymentAuthorizationError::new(
                    grpc_api_types::payments::PaymentStatus::Pending,
                    Some("Failed to process connector customer data".to_string()),
                    Some("CONNECTOR_CUSTOMER_DATA_ERROR".to_string()),
                    None,
                )
            })?;

        // Create router data for connector customer flow
        let connector_customer_router_data = RouterDataV2::<
            CreateConnectorCustomer,
            PaymentFlowData,
            ConnectorCustomerData,
            ConnectorCustomerResponse,
        > {
            flow: std::marker::PhantomData,
            resource_common_data: payment_flow_data.clone(),
            connector_auth_type: connector_auth_details,
            request: connector_customer_request_data,
            response: Err(ErrorResponse::default()),
        };

        // Get API tag for CreateConnectorCustomer flow
        let api_tag = config
            .api_tags
            .get_tag(FlowName::CreateConnectorCustomer, None);

        // Create test context if test mode is enabled
        let test_context = config
            .test
            .create_test_context(event_params.request_id)
            .map_err(|e| {
                PaymentAuthorizationError::new(
                    grpc_api_types::payments::PaymentStatus::Pending,
                    Some(format!("Test mode configuration error: {e}")),
                    Some("TEST_CONFIG_ERROR".to_string()),
                    None,
                )
            })?;

        // Execute connector processing
        let external_event_params = EventProcessingParams {
            connector_name,
            service_name,
            service_type: event_params.service_type,
            flow_name: FlowName::CreateConnectorCustomer,
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
        .switch()
        .map_err(|e: error_stack::Report<ApplicationErrorResponse>| {
            PaymentAuthorizationError::new(
                grpc_api_types::payments::PaymentStatus::Pending,
                Some(format!("Connector customer creation failed: {e}")),
                Some("CONNECTOR_CUSTOMER_CREATION_ERROR".to_string()),
                Some(500),
            )
        })?;

        match response.response {
            Ok(connector_customer_data) => Ok(connector_customer_data),
            Err(ErrorResponse {
                message,
                status_code,
                ..
            }) => Err(PaymentAuthorizationError::new(
                grpc_api_types::payments::PaymentStatus::Pending,
                Some(format!("Connector customer creation failed: {message}")),
                Some("CONNECTOR_CUSTOMER_CREATION_ERROR".to_string()),
                Some(status_code.into()),
            )),
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn handle_connector_customer_for_setup_mandate<
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
        connector_auth_details: ConnectorSpecificAuth,
        payload: &PaymentServiceRegisterRequest,
        connector_name: &str,
        service_name: &str,
        event_params: EventParams<'_>,
    ) -> Result<ConnectorCustomerResponse, tonic::Status> {
        // Get connector integration for CreateConnectorCustomer flow
        let connector_integration: BoxedConnectorIntegrationV2<
            '_,
            CreateConnectorCustomer,
            PaymentFlowData,
            ConnectorCustomerData,
            ConnectorCustomerResponse,
        > = connector_data.connector.get_connector_integration_v2();

        // Create connector customer request data using ForeignTryFrom
        let connector_customer_request_data =
            ConnectorCustomerData::foreign_try_from(payload.clone()).map_err(|err| {
                tracing::error!("Failed to process connector customer data: {:?}", err);
                tonic::Status::internal(format!("Failed to process connector customer data: {err}"))
            })?;

        // Create router data for connector customer flow
        let connector_customer_router_data = RouterDataV2::<
            CreateConnectorCustomer,
            PaymentFlowData,
            ConnectorCustomerData,
            ConnectorCustomerResponse,
        > {
            flow: std::marker::PhantomData,
            resource_common_data: payment_flow_data.clone(),
            connector_auth_type: connector_auth_details,
            request: connector_customer_request_data,
            response: Err(ErrorResponse::default()),
        };

        // Get API tag for CreateConnectorCustomer flow
        let api_tag = config
            .api_tags
            .get_tag(FlowName::CreateConnectorCustomer, None);

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
            flow_name: FlowName::CreateConnectorCustomer,
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
        .switch()
        .map_err(|e: error_stack::Report<ApplicationErrorResponse>| {
            tonic::Status::internal(format!("Connector customer creation failed: {e}"))
        })?;

        match response.response {
            Ok(connector_customer_data) => Ok(connector_customer_data),
            Err(ErrorResponse {
                message,
                status_code,
                ..
            }) => Err(tonic::Status::internal(format!(
                "Connector customer creation failed: {message} (status: {status_code})"
            ))),
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn handle_payment_method_token<
        T: PaymentMethodDataTypes
            + Default
            + Eq
            + Debug
            + Send
            + serde::Serialize
            + serde::de::DeserializeOwned
            + Clone
            + Sync
            + domain_types::types::CardConversionHelper<T>,
    >(
        &self,
        config: &Arc<Config>,
        connector_data: ConnectorData<T>,
        payment_flow_data: &PaymentFlowData,
        connector_auth_details: ConnectorSpecificAuth,
        event_params: EventParams<'_>,
        payment_authorize_data: &PaymentsAuthorizeData<T>,
        connector_name: &str,
        service_name: &str,
    ) -> Result<PaymentMethodTokenResponse, PaymentAuthorizationError> {
        // Get connector integration
        let connector_integration: BoxedConnectorIntegrationV2<
            '_,
            PaymentMethodToken,
            PaymentFlowData,
            PaymentMethodTokenizationData<T>,
            PaymentMethodTokenResponse,
        > = connector_data.connector.get_connector_integration_v2();

        let payment_method_tokenization_data =
            PaymentMethodTokenizationData::from(payment_authorize_data);

        let payment_method_token_router_data = RouterDataV2::<
            PaymentMethodToken,
            PaymentFlowData,
            PaymentMethodTokenizationData<T>,
            PaymentMethodTokenResponse,
        > {
            flow: std::marker::PhantomData,
            resource_common_data: payment_flow_data.clone(),
            connector_auth_type: connector_auth_details,
            request: payment_method_tokenization_data,
            response: Err(ErrorResponse::default()),
        };

        // Get API tag for PaymentMethodToken flow
        let api_tag = config.api_tags.get_tag(FlowName::PaymentMethodToken, None);

        // Create test context if test mode is enabled
        let test_context = config
            .test
            .create_test_context(event_params.request_id)
            .map_err(|e| {
                PaymentAuthorizationError::new(
                    grpc_api_types::payments::PaymentStatus::Pending,
                    Some(format!("Test mode configuration error: {e}")),
                    Some("TEST_CONFIG_ERROR".to_string()),
                    None,
                )
            })?;

        // Execute connector processing
        let external_event_params = EventProcessingParams {
            connector_name,
            service_name,
            service_type: event_params.service_type,
            flow_name: FlowName::PaymentMethodToken,
            event_config: &config.events,
            request_id: event_params.request_id,
            lineage_ids: event_params.lineage_ids,
            reference_id: event_params.reference_id,
            resource_id: event_params.resource_id,
            shadow_mode: event_params.shadow_mode,
        };
        let response = external_services::service::execute_connector_processing_step(
            &config.proxy,
            connector_integration,
            payment_method_token_router_data,
            None,
            external_event_params,
            None,
            common_enums::CallConnectorAction::Trigger,
            test_context,
            api_tag,
        )
        .await
        .switch()
        .map_err(|e: error_stack::Report<ApplicationErrorResponse>| {
            PaymentAuthorizationError::new(
                grpc_api_types::payments::PaymentStatus::Pending,
                Some(format!("Payment Method Token creation failed: {e}")),
                Some("PAYMENT_METHOD_TOKEN_CREATION_ERROR".to_string()),
                Some(500),
            )
        })?;

        match response.response {
            Ok(payment_method_token_data) => {
                tracing::info!("Payment method token created successfully");
                Ok(payment_method_token_data)
            }
            Err(ErrorResponse {
                message,
                status_code,
                ..
            }) => Err(PaymentAuthorizationError::new(
                grpc_api_types::payments::PaymentStatus::Pending,
                Some(format!("Payment Method Token creation failed: {message}")),
                Some("PAYMENT_METHOD_TOKEN_CREATION_ERROR".to_string()),
                Some(status_code.into()),
            )),
        }
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
        fn_name: internal_sdk_session_token,
        log_prefix: "SDK_SESSION_TOKEN",
        request_type: PaymentServiceSdkSessionTokenRequest,
        response_type: PaymentServiceSdkSessionTokenResponse,
        flow_marker: SdkSessionToken,
        resource_common_data_type: PaymentFlowData,
        request_data_type: PaymentsSdkSessionTokenData,
        response_data_type: PaymentsResponseData,
        request_data_constructor: PaymentsSdkSessionTokenData::foreign_try_from,
        common_flow_data_constructor: PaymentFlowData::foreign_try_from,
        generate_response_fn: generate_payment_sdk_session_token_response,
        all_keys_required: None
    );

    implement_connector_operation!(
        fn_name: internal_mandate_revoke,
        log_prefix: "MANDATE_REVOKE",
        request_type: PaymentServiceRevokeMandateRequest,
        response_type: PaymentServiceRevokeMandateResponse,
        flow_marker: MandateRevoke,
        resource_common_data_type: PaymentFlowData,
        request_data_type: MandateRevokeRequestData,
        response_data_type: MandateRevokeResponseData,
        request_data_constructor: MandateRevokeRequestData::foreign_try_from,
        common_flow_data_constructor: PaymentFlowData::foreign_try_from,
        generate_response_fn: generate_mandate_revoke_response,
        all_keys_required: None
    );

    implement_connector_operation!(
        fn_name: internal_pre_authenticate,
        log_prefix: "PRE_AUTHENTICATE",
        request_type: PaymentServicePreAuthenticateRequest,
        response_type: PaymentServicePreAuthenticateResponse,
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
        request_type: PaymentServiceAuthenticateRequest,
        response_type: PaymentServiceAuthenticateResponse,
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
        request_type: PaymentServicePostAuthenticateRequest,
        response_type: PaymentServicePostAuthenticateResponse,
        flow_marker: PostAuthenticate,
        resource_common_data_type: PaymentFlowData,
        request_data_type: PaymentsPostAuthenticateData<DefaultPCIHolder>,
        response_data_type: PaymentsResponseData,
        request_data_constructor: PaymentsPostAuthenticateData::foreign_try_from,
        common_flow_data_constructor: PaymentFlowData::foreign_try_from,
        generate_response_fn: generate_payment_post_authenticate_response,
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
        request_type: PaymentServiceVoidPostCaptureRequest,
        response_type: PaymentServiceVoidPostCaptureResponse,
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
                                match Box::pin(self.process_authorization_internal::<VaultTokenHolder>(
                                    &config,
                                    payload.clone(),
                                    metadata_payload.connector,
                                    metadata_payload.connector_auth_type.clone(),
                                    metadata,
                                    &metadata_payload,
                                    &service_name,
                                    &metadata_payload.request_id,
                                    Some(token_data),
                                ))
                                .await
                                {
                                    Ok(response) => {
                                        tracing::info!("INJECTOR: Authorization completed successfully with injector");
                                        response
                                    },
                                    Err(error_response) => {
                                        tracing::error!("INJECTOR: Authorization failed with injector - error: {:?}", error_response);
                                        PaymentServiceAuthorizeResponse::from(error_response)
                                    },
                                }
                            }
                            _ => {
                                match Box::pin(self.process_authorization_internal::<DefaultPCIHolder>(
                                    &config,
                                    payload.clone(),
                                    metadata_payload.connector,
                                    metadata_payload.connector_auth_type.clone(),
                                    metadata,
                                    &metadata_payload,
                                    &service_name,
                                    &metadata_payload.request_id,
                                    None,
                                ))
                                .await
                                {
                                    Ok(response) => response,
                                    Err(error_response) => PaymentServiceAuthorizeResponse::from(error_response),
                                }
                            }
                        }
                    }
                    _ => {
                        match Box::pin(self.process_authorization_internal::<DefaultPCIHolder>(
                            &config,
                            payload.clone(),
                            metadata_payload.connector,
                            metadata_payload.connector_auth_type.clone(),
                            metadata,
                            &metadata_payload,
                            &service_name,
                            &metadata_payload.request_id,
                            None,
                        ))
                        .await
                        {
                            Ok(response) => response,
                            Err(error_response) => PaymentServiceAuthorizeResponse::from(error_response),
                        }
                    }
                };

                Ok(tonic::Response::new(authorize_response))
            })
        })
        .await
    }

    #[tracing::instrument(
        name = "payment_authorize_only",
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
    async fn authorize_only(
        &self,
        request: tonic::Request<PaymentServiceAuthorizeOnlyRequest>,
    ) -> Result<tonic::Response<PaymentServiceAuthorizeResponse>, tonic::Status> {
        info!("PAYMENT_AUTHORIZE_ONLY_FLOW: initiated");
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
                                match Box::pin(self.process_authorization_only_internal::<VaultTokenHolder>(
                                    &config,
                                    payload.clone(),
                                    metadata_payload.connector,
                                    metadata_payload.connector_auth_type.clone(),
                                    metadata,
                                    &metadata_payload,
                                    &service_name,
                                    &metadata_payload.request_id,
                                    Some(token_data),
                                ))
                                .await
                                {
                                    Ok(response) => {
                                        tracing::info!("INJECTOR: Authorization only completed successfully with injector");
                                        response
                                    },
                                    Err(error_response) => {
                                        tracing::error!("INJECTOR: Authorization only failed with injector - error: {:?}", error_response);
                                        PaymentServiceAuthorizeResponse::from(error_response)
                                    },
                                }
                            }
                            _ => {
                                tracing::info!("REGULAR: Processing regular payment authorization only (no injector)");
                                match Box::pin(self.process_authorization_only_internal::<DefaultPCIHolder>(
                                    &config,
                                    payload.clone(),
                                    metadata_payload.connector,
                                    metadata_payload.connector_auth_type.clone(),
                                    metadata,
                                    &metadata_payload,
                                    &service_name,
                                    &metadata_payload.request_id,
                                    None,
                                ))
                                .await
                                {
                                    Ok(response) => {
                                        tracing::info!("REGULAR: Authorization only completed successfully without injector");
                                        response
                                    },
                                    Err(error_response) => {
                                        tracing::error!("REGULAR: Authorization only failed without injector - error: {:?}", error_response);
                                        PaymentServiceAuthorizeResponse::from(error_response)
                                    },
                                }
                            }
                        }
                    }
                    _ => {
                        match Box::pin(self.process_authorization_only_internal::<DefaultPCIHolder>(
                            &config,
                            payload.clone(),
                            metadata_payload.connector,
                            metadata_payload.connector_auth_type.clone(),
                            metadata,
                            &metadata_payload,
                            &service_name,
                            &metadata_payload.request_id,
                            None,
                        ))
                        .await
                        {
                            Ok(response) => response,
                            Err(error_response) => PaymentServiceAuthorizeResponse::from(error_response),
                        }
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
                        ref request_id,
                        ref lineage_ids,
                        ref reference_id,
                        ref resource_id,
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

                    // Create common request data
                    let payment_flow_data = PaymentFlowData::foreign_try_from((
                        payload.clone(),
                        config.connectors.clone(),
                        &request_data.masked_metadata,
                    ))
                    .into_grpc_status()?;

                    // Extract access token from Hyperswitch request
                    let cached_access_token = payload
                        .state
                        .as_ref()
                        .and_then(|state| state.access_token.as_ref());

                    // Check if connector supports access tokens
                    let should_do_access_token = connector_data
                        .connector
                        .should_do_access_token(payment_flow_data.payment_method);

                    // Conditional token generation - ONLY if not provided in request
                    let payment_flow_data = if should_do_access_token {
                        let event_params = EventParams {
                            _connector_name: &connector.to_string(),
                            _service_name: &service_name,
                            service_type: utils::service_type_str(&config.server.type_),
                            request_id,
                            lineage_ids,
                            reference_id,
                            resource_id,
                            shadow_mode: metadata_payload.shadow_mode,
                        };

                        let access_token_data = self
                            .handle_access_token_flow(
                                &config,
                                &connector_data,
                                cached_access_token,
                                &payment_flow_data,
                                &metadata_payload.connector_auth_type,
                                &connector.to_string(),
                                &service_name,
                                event_params,
                            )
                            .await?;

                        // Store in flow data for connector API calls
                        payment_flow_data.set_access_token(Some(access_token_data))
                    } else {
                        // Connector doesn't support access tokens
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
                        connector_auth_type: metadata_payload.connector_auth_type.clone(),
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
                    .switch()
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
            |mut request_data| {
                let service_name = service_name.clone();
                Box::pin(async move {
                    let metadata_payload = &request_data.extracted_metadata;
                    let connector = metadata_payload.connector;

                    // Get connector data to check if access token is needed
                    let connector_data: ConnectorData<DefaultPCIHolder> =
                        ConnectorData::get_connector_by_name(&connector);

                    // Check if connector supports access tokens
                    let temp_payment_flow_data = PaymentFlowData::foreign_try_from((
                        request_data.payload.clone(),
                        config.connectors.clone(),
                        &request_data.masked_metadata,
                    ))
                    .map_err(|e| {
                        tonic::Status::internal(format!("Failed to create payment flow data: {e}"))
                    })?;
                    let should_do_access_token = connector_data
                        .connector
                        .should_do_access_token(temp_payment_flow_data.payment_method);

                    if should_do_access_token {
                        // Extract access token from Hyperswitch request
                        let cached_access_token = request_data
                            .payload
                            .state
                            .as_ref()
                            .and_then(|state| state.access_token.as_ref());

                        let event_params = EventParams {
                            _connector_name: &connector.to_string(),
                            _service_name: &service_name,
                            service_type: utils::service_type_str(&config.server.type_),
                            request_id: &metadata_payload.request_id,
                            lineage_ids: &metadata_payload.lineage_ids,
                            reference_id: &metadata_payload.reference_id,
                            resource_id: &metadata_payload.resource_id,
                            shadow_mode: metadata_payload.shadow_mode,
                        };

                        let access_token_data = self
                            .handle_access_token_flow(
                                &config,
                                &connector_data,
                                cached_access_token,
                                &temp_payment_flow_data,
                                &metadata_payload.connector_auth_type,
                                &connector.to_string(),
                                &service_name,
                                event_params,
                            )
                            .await?;

                        // Create access token info for the request
                        let access_token_info = grpc_api_types::payments::AccessToken {
                            token: Some(access_token_data.access_token),
                            expires_in_seconds: access_token_data.expires_in,
                            token_type: access_token_data.token_type,
                        };

                        // Set the access token in the request payload
                        if request_data.payload.state.is_none() {
                            request_data.payload.state =
                                Some(grpc_api_types::payments::ConnectorState {
                                    access_token: Some(access_token_info),
                                    connector_customer_id: None,
                                });
                        } else if let Some(ref mut state) = request_data.payload.state {
                            state.access_token = Some(access_token_info);
                        }
                    }

                    // Now call the existing internal_void_payment method
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
    async fn void_post_capture(
        &self,
        request: tonic::Request<PaymentServiceVoidPostCaptureRequest>,
    ) -> Result<tonic::Response<PaymentServiceVoidPostCaptureResponse>, tonic::Status> {
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
        name = "incoming_webhook",
        fields(
            name = common_utils::consts::NAME,
            service_name = common_utils::consts::PAYMENT_SERVICE_NAME,
            service_method = FlowName::IncomingWebhook.as_str(),
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
            flow = FlowName::IncomingWebhook.as_str(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]

    async fn transform(
        &self,
        request: tonic::Request<PaymentServiceTransformRequest>,
    ) -> Result<tonic::Response<PaymentServiceTransformResponse>, tonic::Status> {
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "PaymentService".to_string());
        let config = get_config_from_request(&request)?;
        grpc_logging_wrapper(request, &service_name,
        config.clone(),
        FlowName::IncomingWebhook,
        |request_data| {
            let service_name_clone = service_name.clone();
            async move {
                let payload = request_data.payload;
                let metadata_payload = request_data.extracted_metadata;
                let connector = metadata_payload.connector;
                let _request_id = &metadata_payload.request_id;
                let connector_auth_details = &metadata_payload.connector_auth_type;
                let request_details = payload
                    .request_details
                    .map(domain_types::connector_types::RequestDetails::foreign_try_from)
                    .ok_or_else(|| {
                        tonic::Status::invalid_argument("missing request_details in the payload")
                    })?
                    .map_err(|e| e.into_grpc_status())?;
                let webhook_secrets = payload
                    .webhook_secrets
                    .clone()
                    .map(|details| {
                        domain_types::connector_types::ConnectorWebhookSecrets::foreign_try_from(
                            details,
                        )
                        .map_err(|e| e.into_grpc_status())
                    })
                    .transpose()?;
                //get connector data
                let connector_data: ConnectorData<DefaultPCIHolder> =
                    ConnectorData::get_connector_by_name(&connector);

                let requires_external_verification = connector_data
                    .connector
                    .requires_external_webhook_verification(config
                        .webhook_source_verification_call
                        .connectors_with_webhook_source_verification_call
                        .as_ref());

                let source_verified = if requires_external_verification {
                    verify_webhook_source_external(
                        config.as_ref(),
                        &connector_data,
                        &request_details,
                        webhook_secrets.clone(),
                        connector_auth_details,
                        &metadata_payload,
                        &service_name_clone,
                    )
                    .await?
                } else {
                     match connector_data
                    .connector
                    .verify_webhook_source(
                        request_details.clone(),
                        webhook_secrets.clone(),
                        Some(connector_auth_details.clone()),
                    ) {
                    Ok(result) => result,
                    Err(err) => {
                        tracing::warn!(
                            target: "webhook",
                            "{:?}",
                            err
                        );
                        false
                    }
            }
                };

                let event_type = connector_data
                    .connector
                    .get_event_type(
                        request_details.clone(),
                        webhook_secrets.clone(),
                        Some(connector_auth_details.clone()),
                    )
                    .switch()
                    .into_grpc_status()?;
                // Get content for the webhook based on the event type using categorization
                let content = if event_type.is_payment_event() {
                    Some(get_payments_webhook_content(
                        connector_data,
                        request_details,
                        webhook_secrets,
                        Some(connector_auth_details.clone()),
                    )
                    .await
                    .into_grpc_status()?)
                } else if event_type.is_refund_event() {
                    Some(get_refunds_webhook_content(
                        connector_data,
                        request_details,
                        webhook_secrets,
                        Some(connector_auth_details.clone()),
                    )
                    .await
                    .into_grpc_status()?)
                } else if event_type.is_dispute_event() {
                    Some(get_disputes_webhook_content(
                        connector_data,
                        request_details,
                        webhook_secrets,
                        Some(connector_auth_details.clone()),
                    )
                    .await
                    .into_grpc_status()?)
                } else if event_type.is_test_event() {
                    None
                }else {
                    // For all other event types, default to payment webhook content for now
                    // This includes mandate, payout, recovery, and misc events
                    Some(get_payments_webhook_content(
                        connector_data,
                        request_details,
                        webhook_secrets,
                        Some(connector_auth_details.clone()),
                    )
                    .await
                    .into_grpc_status()?)
                };
                let api_event_type =
                    grpc_api_types::payments::WebhookEventType::foreign_try_from(event_type)
                        .map_err(|e| e.into_grpc_status())?;

                let webhook_transformation_status = match content.as_ref().and_then(|content| content.content.clone()) {
                    Some(grpc_api_types::payments::webhook_response_content::Content::IncompleteTransformation(_)) => WebhookTransformationStatus::Incomplete,
                    _ => WebhookTransformationStatus::Complete,
                };

                let response = PaymentServiceTransformResponse {
                    event_type: api_event_type.into(),
                    content: content,
                    source_verified,
                    response_ref_id: None,
                    transformation_status: webhook_transformation_status.into(),
                };

                Ok(tonic::Response::new(response))
            }
        },)
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
        grpc_logging_wrapper(request, &service_name,
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
                    .switch()
                    .into_grpc_status()?;

                let response = PaymentServiceVerifyRedirectResponseResponse::foreign_try_from((source_verified, redirect_details_response))
                    .map_err(|e| e.into_grpc_status())?;

                Ok(tonic::Response::new(response))
            }
        }).await
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
        name = "defend_dispute",
        fields(
            name = common_utils::consts::NAME,
            service_name = common_utils::consts::PAYMENT_SERVICE_NAME,
            service_method = FlowName::DefendDispute.as_str(),
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
            flow = FlowName::DefendDispute.as_str(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn dispute(
        &self,
        request: tonic::Request<PaymentServiceDisputeRequest>,
    ) -> Result<tonic::Response<DisputeResponse>, tonic::Status> {
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
            FlowName::DefendDispute,
            |_request_data| async {
                let response = DisputeResponse {
                    ..Default::default()
                };
                Ok(tonic::Response::new(response))
            },
        )
        .await
    }

    #[tracing::instrument(
        name = "sdk_session_token",
        fields(
            name = common_utils::consts::NAME,
            service_name = common_utils::consts::PAYMENT_SERVICE_NAME,
            service_method = FlowName::SdkSessionToken.as_str(),
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
            flow = FlowName::SdkSessionToken.as_str(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn sdk_session_token(
        &self,
        request: tonic::Request<PaymentServiceSdkSessionTokenRequest>,
    ) -> Result<tonic::Response<PaymentServiceSdkSessionTokenResponse>, tonic::Status> {
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
            FlowName::SdkSessionToken,
            |request_data| async move { self.internal_sdk_session_token(request_data).await },
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
            |mut request_data| {
                let service_name = service_name.clone();
                Box::pin(async move {
                    let metadata_payload = &request_data.extracted_metadata;
                    let connector = metadata_payload.connector;

                    // Get connector data to check if access token is needed
                    let connector_data: ConnectorData<DefaultPCIHolder> =
                        ConnectorData::get_connector_by_name(&connector);

                    // Check if connector supports access tokens
                    let temp_payment_flow_data = PaymentFlowData::foreign_try_from((
                        request_data.payload.clone(),
                        config.connectors.clone(),
                        &request_data.masked_metadata,
                    ))
                    .map_err(|e| {
                        tonic::Status::internal(format!("Failed to create payment flow data: {e}"))
                    })?;
                    let should_do_access_token = connector_data
                        .connector
                        .should_do_access_token(temp_payment_flow_data.payment_method);

                    if should_do_access_token {
                        // Extract access token from Hyperswitch request
                        let cached_access_token = request_data
                            .payload
                            .state
                            .as_ref()
                            .and_then(|state| state.access_token.as_ref());

                        let event_params = EventParams {
                            _connector_name: &connector.to_string(),
                            _service_name: &service_name,
                            service_type: utils::service_type_str(&config.server.type_),
                            request_id: &metadata_payload.request_id,
                            lineage_ids: &metadata_payload.lineage_ids,
                            reference_id: &metadata_payload.reference_id,
                            resource_id: &metadata_payload.resource_id,
                            shadow_mode: metadata_payload.shadow_mode,
                        };

                        let access_token_data = self
                            .handle_access_token_flow(
                                &config,
                                &connector_data,
                                cached_access_token,
                                &temp_payment_flow_data,
                                &metadata_payload.connector_auth_type,
                                &connector.to_string(),
                                &service_name,
                                event_params,
                            )
                            .await?;

                        // Create access token info for the request
                        let access_token_info = grpc_api_types::payments::AccessToken {
                            token: Some(access_token_data.access_token.clone()),
                            expires_in_seconds: access_token_data.expires_in,
                            token_type: access_token_data.token_type.clone(),
                        };

                        // Set the access token in the request payload
                        if request_data.payload.state.is_none() {
                            request_data.payload.state =
                                Some(grpc_api_types::payments::ConnectorState {
                                    access_token: Some(access_token_info),
                                    connector_customer_id: None,
                                });
                        } else if let Some(ref mut state) = request_data.payload.state {
                            state.access_token = Some(access_token_info);
                        }
                    }
                    // Now call the existing internal_payment_capture method
                    self.internal_payment_capture(request_data).await
                })
            },
        )
        .await
    }

    #[tracing::instrument(
        name = "setup_mandate",
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
    async fn register(
        &self,
        request: tonic::Request<PaymentServiceRegisterRequest>,
    ) -> Result<tonic::Response<PaymentServiceRegisterResponse>, tonic::Status> {
        info!("SETUP_MANDATE_FLOW: initiated");
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
                Box::pin(async move {
                    let payload = request_data.payload;
                    let metadata_payload = request_data.extracted_metadata;
                    let (connector, request_id, lineage_ids) = (
                        metadata_payload.connector,
                        metadata_payload.request_id,
                        metadata_payload.lineage_ids,
                    );
                    let connector_auth_details = &metadata_payload.connector_auth_type;

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

                    // Create common request data
                    let payment_flow_data = PaymentFlowData::foreign_try_from((
                        payload.clone(),
                        config.connectors.clone(),
                        config.common.environment,
                        &request_data.masked_metadata,
                    ))
                    .map_err(|e| e.into_grpc_status())?;

                    let should_do_order_create = connector_data.connector.should_do_order_create();

                    let order_id = if should_do_order_create {
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

                        Some(
                            Box::pin(self.handle_order_creation_for_setup_mandate(
                                &config,
                                connector_data.clone(),
                                &payment_flow_data,
                                connector_auth_details.clone(),
                                event_params,
                                &payload,
                                &connector.to_string(),
                                &service_name,
                            ))
                            .await?,
                        )
                    } else {
                        None
                    };
                    let payment_flow_data = payment_flow_data.set_order_reference_id(order_id);

                    // Extract connector customer ID (if provided)
                    let cached_connector_customer_id = payload.connector_customer_id.clone();

                    // Check if connector supports customer creation
                    let should_create_connector_customer =
                        connector_data.connector.should_create_connector_customer();

                    // Conditional customer creation - ONLY if connector needs it AND no existing customer ID
                    let payment_flow_data = if should_create_connector_customer {
                        match cached_connector_customer_id {
                            Some(_customer_id) => payment_flow_data,
                            None => {
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

                                let connector_customer_response =
                                    Box::pin(self.handle_connector_customer_for_setup_mandate(
                                        &config,
                                        connector_data.clone(),
                                        &payment_flow_data,
                                        connector_auth_details.clone(),
                                        &payload,
                                        &connector.to_string(),
                                        &service_name,
                                        event_params,
                                    ))
                                    .await?;

                                payment_flow_data.set_connector_customer_id(Some(
                                    connector_customer_response.connector_customer_id,
                                ))
                            }
                        }
                    } else {
                        // Connector doesn't support customer creation
                        payment_flow_data
                    };

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
                        connector_auth_type: connector_auth_details.clone(),
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
                    .switch()
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

    async fn register_only(
        &self,
        request: tonic::Request<PaymentServiceRegisterRequest>,
    ) -> Result<tonic::Response<PaymentServiceRegisterResponse>, tonic::Status> {
        info!("SETUP_MANDATE_FLOW: initiated");
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
                    let connector_auth_details = &metadata_payload.connector_auth_type;

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

                    // Create common request data
                    let payment_flow_data = PaymentFlowData::foreign_try_from((
                        payload.clone(),
                        config.connectors.clone(),
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
                        connector_auth_type: connector_auth_details.clone(),
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
                    .switch()
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
        name = "create_session_token",
        fields(
            name = common_utils::consts::NAME,
            service_name = common_utils::consts::PAYMENT_SERVICE_NAME,
            service_method = FlowName::CreateSessionToken.as_str(),
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
            flow = FlowName::CreateSessionToken.as_str(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn create_session_token(
        &self,
        request: tonic::Request<PaymentServiceCreateSessionTokenRequest>,
    ) -> Result<tonic::Response<PaymentServiceCreateSessionTokenResponse>, tonic::Status> {
        info!("CREATE_SESSION_TOKEN_FLOW: initiated");
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
            FlowName::CreateSessionToken,
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
                    let connector_auth_details = &metadata_payload.connector_auth_type;

                    //get connector data
                    let connector_data: ConnectorData<DefaultPCIHolder> =
                        ConnectorData::get_connector_by_name(&connector);

                    // Create common request data
                    let payment_flow_data = PaymentFlowData::foreign_try_from((
                        payload.clone(),
                        config.connectors.clone(),
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

                    let session_token_data = Box::pin(self.handle_session_token(
                        &config,
                        connector_data.clone(),
                        &payment_flow_data,
                        connector_auth_details.clone(),
                        &payload,
                        &connector.to_string(),
                        &service_name,
                        event_params,
                    ))
                    .await
                    .map_err(|e| {
                        let message = e
                            .error_message
                            .unwrap_or_else(|| "Session token creation failed".to_string());
                        tonic::Status::internal(message)
                    })?;

                    tracing::info!(
                        "Session token created successfully: {}",
                        session_token_data.session_token
                    );

                    // Create response
                    let session_token_response = PaymentServiceCreateSessionTokenResponse {
                        session_token: session_token_data.session_token,
                        error_message: None,
                        error_code: None,
                        status_code: 200u16.into(),
                    };

                    Ok(tonic::Response::new(session_token_response))
                })
            },
        )
        .await
    }

    #[tracing::instrument(
        name = "repeat_payment",
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
    async fn repeat_everything(
        &self,
        request: tonic::Request<PaymentServiceRepeatEverythingRequest>,
    ) -> Result<tonic::Response<PaymentServiceRepeatEverythingResponse>, tonic::Status> {
        info!("REPEAT_PAYMENT_FLOW: initiated");
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
                    let connector_auth_details = &metadata_payload.connector_auth_type;

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

                    // Create payment flow data
                    let payment_flow_data = PaymentFlowData::foreign_try_from((
                        payload.clone(),
                        config.connectors.clone(),
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
                        connector_auth_type: connector_auth_details.clone(),
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
                    .switch()
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
        name = "mandate_revoke",
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
    async fn mandate_revoke(
        &self,
        request: tonic::Request<PaymentServiceRevokeMandateRequest>,
    ) -> Result<tonic::Response<PaymentServiceRevokeMandateResponse>, tonic::Status> {
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
        request: tonic::Request<PaymentServicePreAuthenticateRequest>,
    ) -> Result<tonic::Response<PaymentServicePreAuthenticateResponse>, tonic::Status> {
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
            FlowName::PreAuthenticate,
            |request_data| async move { self.internal_pre_authenticate(request_data).await },
        )
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
        request: tonic::Request<PaymentServiceAuthenticateRequest>,
    ) -> Result<tonic::Response<PaymentServiceAuthenticateResponse>, tonic::Status> {
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
        request: tonic::Request<PaymentServicePostAuthenticateRequest>,
    ) -> Result<tonic::Response<PaymentServicePostAuthenticateResponse>, tonic::Status> {
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
        name = "create_payment_method_token",
        fields(
            name = common_utils::consts::NAME,
            service_name = common_utils::consts::PAYMENT_SERVICE_NAME,
            service_method = "CreatePaymentMethodToken",
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
    async fn create_payment_method_token(
        &self,
        request: tonic::Request<PaymentServiceCreatePaymentMethodTokenRequest>,
    ) -> Result<tonic::Response<PaymentServiceCreatePaymentMethodTokenResponse>, tonic::Status>
    {
        info!("CREATE_PAYMENT_METHOD_TOKEN_FLOW: initiated");
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
                    let connector_auth_details = &metadata_payload.connector_auth_type;

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

                    // Create payment flow data
                    let payment_flow_data = PaymentFlowData::foreign_try_from((
                        payload.clone(),
                        config.connectors.clone(),
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
                                tonic::Status::internal(format!(
                                    "Failed to process payment method token data: {err}"
                                ))
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
                        connector_auth_type: connector_auth_details.clone(),
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
                    .switch()
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

    #[tracing::instrument(
        name = "create_connector_customer",
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
    async fn create_connector_customer(
        &self,
        request: tonic::Request<
            grpc_api_types::payments::PaymentServiceCreateConnectorCustomerRequest,
        >,
    ) -> Result<
        tonic::Response<grpc_api_types::payments::PaymentServiceCreateConnectorCustomerResponse>,
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
                    let connector_auth_details = &metadata_payload.connector_auth_type;

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

                    // Create common request data
                    let payment_flow_data = PaymentFlowData::foreign_try_from((
                        payload.clone(),
                        config.connectors.clone(),
                        &request_data.masked_metadata,
                    ))
                    .map_err(|e| e.into_grpc_status())?;

                    // Create connector customer request data directly
                    let connector_customer_request_data = ConnectorCustomerData::foreign_try_from(
                        payload.clone(),
                    )
                    .map_err(|err| {
                        tracing::error!("Failed to process connector customer data: {:?}", err);
                        tonic::Status::internal(format!(
                            "Failed to process connector customer data: {err}"
                        ))
                    })?;

                    // Create router data for connector customer flow
                    let connector_customer_router_data = RouterDataV2::<
                        CreateConnectorCustomer,
                        PaymentFlowData,
                        ConnectorCustomerData,
                        ConnectorCustomerResponse,
                    > {
                        flow: std::marker::PhantomData,
                        resource_common_data: payment_flow_data.clone(),
                        connector_auth_type: connector_auth_details.clone(),
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
                    .switch()
                    .map_err(
                        |e: error_stack::Report<ApplicationErrorResponse>| {
                            tonic::Status::internal(format!(
                                "Connector customer creation failed: {e}"
                            ))
                        },
                    )?;

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

    #[tracing::instrument(
        name = "create_access_token",
        fields(
            name = common_utils::consts::NAME,
            service_name = common_utils::consts::PAYMENT_SERVICE_NAME,
            service_method = FlowName::CreateAccessToken.as_str(),
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
            flow = FlowName::CreateAccessToken.as_str(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn create_access_token(
        &self,
        request: tonic::Request<PaymentServiceCreateAccessTokenRequest>,
    ) -> Result<tonic::Response<PaymentServiceCreateAccessTokenResponse>, tonic::Status> {
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
            FlowName::CreateAccessToken,
            |request_data| {
                let service_name = service_name.clone();
                Box::pin(async move {
                    let metadata_payload = request_data.extracted_metadata;
                    let (connector, request_id, lineage_ids) = (
                        metadata_payload.connector,
                        metadata_payload.request_id,
                        metadata_payload.lineage_ids,
                    );
                    let connector_auth_details = &metadata_payload.connector_auth_type;

                    // Get connector data
                    let connector_data: ConnectorData<DefaultPCIHolder> =
                        ConnectorData::get_connector_by_name(&connector);
                    let access_token_create_request = request_data.payload;
                    // Create minimal payment flow data for access token generation
                    let payment_flow_data = PaymentFlowData::foreign_try_from((
                        access_token_create_request,
                        config.connectors.clone(),
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

                    // Reuse the existing handle_access_token function
                    let access_token_data = Box::pin(self.handle_access_token(
                        &config,
                        connector_data,
                        &payment_flow_data,
                        connector_auth_details.clone(),
                        &connector.to_string(),
                        &service_name,
                        event_params,
                    ))
                    .await
                    .map_err(|e| {
                        let message = e
                            .error_message
                            .unwrap_or_else(|| "Access token creation failed".to_string());
                        tonic::Status::internal(message)
                    })?;

                    tracing::info!(
                        "Access token created successfully with expiry: {:?}",
                        access_token_data.expires_in
                    );

                    // Create response using the access token data
                    let create_access_token_response = PaymentServiceCreateAccessTokenResponse {
                        access_token: Some(access_token_data.access_token),
                        token_type: access_token_data.token_type,
                        expires_in_seconds: access_token_data.expires_in,
                        status: i32::from(grpc_api_types::payments::OperationStatus::Success),
                        error_code: None,
                        error_message: None,
                        status_code: 200,
                        response_ref_id: None,
                    };

                    Ok(tonic::Response::new(create_access_token_response))
                })
            },
        )
        .await
    }
}

/// For connectors requiring external webhook source verification (e.g., PayPal).
/// Executes the VerifyWebhookSource flow via the connector integration.
async fn verify_webhook_source_external(
    config: &Config,
    connector_data: &ConnectorData<DefaultPCIHolder>,
    request_details: &domain_types::connector_types::RequestDetails,
    webhook_secrets: Option<domain_types::connector_types::ConnectorWebhookSecrets>,
    connector_auth_details: &ConnectorSpecificAuth,
    metadata_payload: &utils::MetadataPayload,
    service_name: &str,
) -> Result<bool, tonic::Status> {
    let verify_webhook_flow_data = VerifyWebhookSourceFlowData {
        connectors: config.connectors.clone(),
        connector_request_reference_id: format!("webhook_verify_{}", metadata_payload.request_id),
        raw_connector_response: None,
        raw_connector_request: None,
        connector_response_headers: None,
    };

    let merchant_secret =
        webhook_secrets.unwrap_or_else(|| domain_types::connector_types::ConnectorWebhookSecrets {
            secret: "default_secret".to_string().into_bytes(),
            additional_secret: None,
        });

    let verify_webhook_request = VerifyWebhookSourceRequestData {
        webhook_headers: request_details.headers.clone(),
        webhook_body: request_details.body.clone(),
        merchant_secret,
        webhook_uri: request_details.uri.clone(),
    };

    let verify_webhook_router_data = RouterDataV2::<
        VerifyWebhookSource,
        VerifyWebhookSourceFlowData,
        VerifyWebhookSourceRequestData,
        VerifyWebhookSourceResponseData,
    > {
        flow: std::marker::PhantomData,
        resource_common_data: verify_webhook_flow_data,
        connector_auth_type: connector_auth_details.clone(),
        request: verify_webhook_request,
        response: Err(ErrorResponse::default()),
    };

    let connector_integration: BoxedConnectorIntegrationV2<
        '_,
        VerifyWebhookSource,
        VerifyWebhookSourceFlowData,
        VerifyWebhookSourceRequestData,
        VerifyWebhookSourceResponseData,
    > = connector_data.connector.get_connector_integration_v2();

    let event_params = EventProcessingParams {
        connector_name: connector_data.connector.id(),
        service_name,
        service_type: utils::service_type_str(&config.server.type_),
        flow_name: FlowName::IncomingWebhook,
        event_config: &config.events,
        request_id: &metadata_payload.request_id,
        lineage_ids: &metadata_payload.lineage_ids,
        reference_id: &metadata_payload.reference_id,
        resource_id: &metadata_payload.resource_id,
        shadow_mode: metadata_payload.shadow_mode,
    };

    match Box::pin(
        external_services::service::execute_connector_processing_step(
            &config.proxy,
            connector_integration,
            verify_webhook_router_data,
            None,
            event_params,
            None,
            common_enums::CallConnectorAction::Trigger,
            None,
            None,
        ),
    )
    .await
    {
        Ok(verify_result) => Ok(match verify_result.response {
            Ok(response_data) => {
                matches!(
                    response_data.verify_webhook_status,
                    VerifyWebhookStatus::SourceVerified
                )
            }
            Err(_) => {
                tracing::warn!(
                    target: "webhook",
                    "Webhook verification returned error response for connector {}",
                    connector_data.connector.id()
                );
                false
            }
        }),
        Err(e) => {
            tracing::warn!(
                target: "webhook",
                "Webhook verification failed for connector {}: {:?}. Setting source_verified=false",
                connector_data.connector.id(),
                e
            );
            Ok(false)
        }
    }
}

async fn get_payments_webhook_content(
    connector_data: ConnectorData<DefaultPCIHolder>,
    request_details: domain_types::connector_types::RequestDetails,
    webhook_secrets: Option<domain_types::connector_types::ConnectorWebhookSecrets>,
    connector_auth_details: Option<ConnectorSpecificAuth>,
) -> CustomResult<grpc_api_types::payments::WebhookResponseContent, ApplicationErrorResponse> {
    let webhook_details = connector_data
        .connector
        .process_payment_webhook(
            request_details.clone(),
            webhook_secrets,
            connector_auth_details,
        )
        .switch()?;

    match webhook_details.transformation_status {
        common_enums::WebhookTransformationStatus::Complete => {
            // Generate response
            let response = PaymentServiceGetResponse::foreign_try_from(webhook_details)
                .change_context(ApplicationErrorResponse::InternalServerError(ApiError {
                    sub_code: "RESPONSE_CONSTRUCTION_ERROR".to_string(),
                    error_identifier: 500,
                    error_message: "Error while constructing response".to_string(),
                    error_object: None,
                }))?;

            Ok(grpc_api_types::payments::WebhookResponseContent {
                content: Some(
                    grpc_api_types::payments::webhook_response_content::Content::PaymentsResponse(
                        response,
                    ),
                ),
            })
        }
        common_enums::WebhookTransformationStatus::Incomplete => {
            let resource_object = connector_data
                .connector
                .get_webhook_resource_object(request_details)
                .switch()?;
            let resource_object_vec = serde_json::to_vec(&resource_object).change_context(
                ApplicationErrorResponse::InternalServerError(ApiError {
                    sub_code: "SERIALIZATION_ERROR".to_string(),
                    error_identifier: 500,
                    error_message: "Error while serializing resource object".to_string(),
                    error_object: None,
                }),
            )?;

            Ok(grpc_api_types::payments::WebhookResponseContent {
                content: Some(
                    grpc_api_types::payments::webhook_response_content::Content::IncompleteTransformation(
                        grpc_api_types::payments::IncompleteTransformationResponse {
                            resource_object: resource_object_vec,
                            reason: "Payment information required".to_string(),
                        }
                    ),
                ),
            })
        }
    }
}

async fn get_refunds_webhook_content<
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
    connector_data: ConnectorData<T>,
    request_details: domain_types::connector_types::RequestDetails,
    webhook_secrets: Option<domain_types::connector_types::ConnectorWebhookSecrets>,
    connector_auth_details: Option<ConnectorSpecificAuth>,
) -> CustomResult<grpc_api_types::payments::WebhookResponseContent, ApplicationErrorResponse> {
    let webhook_details = connector_data
        .connector
        .process_refund_webhook(request_details, webhook_secrets, connector_auth_details)
        .switch()?;

    // Generate response - RefundService should handle this, for now return basic response
    let response = RefundResponse::foreign_try_from(webhook_details).change_context(
        ApplicationErrorResponse::InternalServerError(ApiError {
            sub_code: "RESPONSE_CONSTRUCTION_ERROR".to_string(),
            error_identifier: 500,
            error_message: "Error while constructing response".to_string(),
            error_object: None,
        }),
    )?;

    Ok(grpc_api_types::payments::WebhookResponseContent {
        content: Some(
            grpc_api_types::payments::webhook_response_content::Content::RefundsResponse(response),
        ),
    })
}

async fn get_disputes_webhook_content<
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
    connector_data: ConnectorData<T>,
    request_details: domain_types::connector_types::RequestDetails,
    webhook_secrets: Option<domain_types::connector_types::ConnectorWebhookSecrets>,
    connector_auth_details: Option<ConnectorSpecificAuth>,
) -> CustomResult<grpc_api_types::payments::WebhookResponseContent, ApplicationErrorResponse> {
    let webhook_details = connector_data
        .connector
        .process_dispute_webhook(request_details, webhook_secrets, connector_auth_details)
        .switch()?;

    // Generate response - DisputeService should handle this, for now return basic response
    let response = DisputeResponse::foreign_try_from(webhook_details).change_context(
        ApplicationErrorResponse::InternalServerError(ApiError {
            sub_code: "RESPONSE_CONSTRUCTION_ERROR".to_string(),
            error_identifier: 500,
            error_message: "Error while constructing response".to_string(),
            error_object: None,
        }),
    )?;

    Ok(grpc_api_types::payments::WebhookResponseContent {
        content: Some(
            grpc_api_types::payments::webhook_response_content::Content::DisputesResponse(response),
        ),
    })
}

pub fn generate_payment_pre_authenticate_response<T: PaymentMethodDataTypes>(
    router_data_v2: RouterDataV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    >,
) -> Result<PaymentServicePreAuthenticateResponse, error_stack::Report<ApplicationErrorResponse>> {
    let transaction_response = router_data_v2.response;
    let status = router_data_v2.resource_common_data.status;
    let grpc_status = grpc_api_types::payments::PaymentStatus::foreign_from(status);
    let raw_connector_response = router_data_v2
        .resource_common_data
        .get_raw_connector_response();
    let response_headers = router_data_v2
        .resource_common_data
        .get_connector_response_headers_as_map();

    let response = match transaction_response {
        Ok(response) => match response {
            PaymentsResponseData::PreAuthenticateResponse {
                redirection_data,
                connector_response_reference_id,
                status_code,
                authentication_data,
            } => PaymentServicePreAuthenticateResponse {
                transaction_id: None,
                redirection_data: redirection_data
                    .map(|form| match *form {
                        router_response_types::RedirectForm::Form {
                            endpoint,
                            method,
                            form_fields,
                        } => Ok::<grpc_api_types::payments::RedirectForm, ApplicationErrorResponse>(
                            grpc_api_types::payments::RedirectForm {
                                form_type: Some(
                                    grpc_api_types::payments::redirect_form::FormType::Form(
                                        grpc_api_types::payments::FormData {
                                            endpoint,
                                            method:
                                                grpc_api_types::payments::HttpMethod::foreign_from(
                                                    method,
                                                )
                                                .into(),
                                            form_fields,
                                        },
                                    ),
                                ),
                            },
                        ),
                        router_response_types::RedirectForm::Html { html_data } => {
                            Ok(grpc_api_types::payments::RedirectForm {
                                form_type: Some(
                                    grpc_api_types::payments::redirect_form::FormType::Html(
                                        grpc_api_types::payments::HtmlData { html_data },
                                    ),
                                ),
                            })
                        }
                        router_response_types::RedirectForm::Uri { uri } => {
                            Ok(grpc_api_types::payments::RedirectForm {
                                form_type: Some(
                                    grpc_api_types::payments::redirect_form::FormType::Uri(
                                        grpc_api_types::payments::UriData { uri },
                                    ),
                                ),
                            })
                        }
                        router_response_types::RedirectForm::Mifinity {
                            initialization_token,
                        } => Ok(grpc_api_types::payments::RedirectForm {
                            form_type: Some(
                                grpc_api_types::payments::redirect_form::FormType::Uri(
                                    grpc_api_types::payments::UriData {
                                        uri: initialization_token,
                                    },
                                ),
                            ),
                        }),
                        router_response_types::RedirectForm::CybersourceAuthSetup {
                            access_token,
                            ddc_url,
                            reference_id,
                        } => {
                            let mut form_fields = HashMap::new();
                            form_fields.insert("access_token".to_string(), access_token);
                            form_fields.insert("ddc_url".to_string(), ddc_url.clone());
                            form_fields.insert("reference_id".to_string(), reference_id);

                            Ok(grpc_api_types::payments::RedirectForm {
                                form_type: Some(
                                    grpc_api_types::payments::redirect_form::FormType::Form(
                                        grpc_api_types::payments::FormData {
                                            endpoint: ddc_url,
                                            method: grpc_api_types::payments::HttpMethod::Post
                                                .into(),
                                            form_fields,
                                        },
                                    ),
                                ),
                            })
                        }
                        _ => Err(ApplicationErrorResponse::BadRequest(ApiError {
                            sub_code: "INVALID_RESPONSE".to_owned(),
                            error_identifier: 400,
                            error_message: "Invalid response from connector".to_owned(),
                            error_object: None,
                        }))?,
                    })
                    .transpose()?,
                connector_metadata: None,
                response_ref_id: connector_response_reference_id.map(|id| {
                    grpc_api_types::payments::Identifier {
                        id_type: Some(grpc_api_types::payments::identifier::IdType::Id(id)),
                    }
                }),
                status: grpc_status.into(),
                error_message: None,
                error_code: None,
                error_reason: None,
                raw_connector_response,
                status_code: status_code.into(),
                response_headers,
                network_txn_id: None,
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
                state: None,
                authentication_data: authentication_data.map(ForeignFrom::foreign_from),
            },
            _ => {
                return Err(ApplicationErrorResponse::BadRequest(ApiError {
                    sub_code: "INVALID_RESPONSE".to_owned(),
                    error_identifier: 400,
                    error_message: "Invalid response type for pre authenticate".to_owned(),
                    error_object: None,
                })
                .into())
            }
        },
        Err(err) => {
            let status = err
                .attempt_status
                .map(grpc_api_types::payments::PaymentStatus::foreign_from)
                .unwrap_or_default();
            PaymentServicePreAuthenticateResponse {
                transaction_id: Some(grpc_api_types::payments::Identifier {
                    id_type: Some(
                        grpc_api_types::payments::identifier::IdType::NoResponseIdMarker(()),
                    ),
                }),
                redirection_data: None,
                network_txn_id: None,
                response_ref_id: None,
                status: status.into(),
                error_message: Some(err.message),
                error_code: Some(err.code),
                error_reason: err.reason,
                network_decline_code: err.network_decline_code,
                network_advice_code: err.network_advice_code,
                network_error_message: err.network_error_message,
                status_code: err.status_code.into(),
                response_headers,
                raw_connector_response,
                connector_metadata: None,
                state: None,
                authentication_data: None,
            }
        }
    };
    Ok(response)
}

pub fn generate_create_order_response(
    router_data_v2: RouterDataV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    >,
) -> Result<PaymentServiceCreateOrderResponse, error_stack::Report<ApplicationErrorResponse>> {
    let transaction_response = router_data_v2.response;
    let status = router_data_v2.resource_common_data.status;
    let grpc_status = grpc_api_types::payments::PaymentStatus::foreign_from(status);
    let raw_connector_response = router_data_v2
        .resource_common_data
        .get_raw_connector_response();
    let raw_connector_request = router_data_v2
        .resource_common_data
        .get_raw_connector_request();
    let response_headers = router_data_v2
        .resource_common_data
        .get_connector_response_headers_as_map();

    let response = match transaction_response {
        Ok(PaymentCreateOrderResponse {
            order_id,
            session_token,
        }) => {
            let grpc_session_token = session_token
                .map(grpc_api_types::payments::SessionToken::foreign_try_from)
                .transpose()?;

            PaymentServiceCreateOrderResponse {
                order_id: Some(grpc_api_types::payments::Identifier {
                    id_type: Some(grpc_api_types::payments::identifier::IdType::Id(order_id)),
                }),
                status: grpc_status.into(),
                error_code: None,
                error_message: None,
                status_code: 200,
                response_headers,
                response_ref_id: None,
                raw_connector_request,
                raw_connector_response,
                session_token: grpc_session_token,
            }
        }
        Err(err) => PaymentServiceCreateOrderResponse {
            order_id: Some(grpc_api_types::payments::Identifier {
                id_type: Some(grpc_api_types::payments::identifier::IdType::NoResponseIdMarker(())),
            }),
            status: err
                .attempt_status
                .map(grpc_api_types::payments::PaymentStatus::foreign_from)
                .unwrap_or_default()
                .into(),
            error_code: Some(err.code),
            error_message: Some(err.message),
            status_code: err.status_code.into(),
            response_headers,
            response_ref_id: None,
            raw_connector_request,
            raw_connector_response,
            session_token: None,
        },
    };
    Ok(response)
}

pub fn generate_payment_authenticate_response<T: PaymentMethodDataTypes>(
    router_data_v2: RouterDataV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    >,
) -> Result<PaymentServiceAuthenticateResponse, error_stack::Report<ApplicationErrorResponse>> {
    let transaction_response = router_data_v2.response;
    let status = router_data_v2.resource_common_data.status;
    let grpc_status = grpc_api_types::payments::PaymentStatus::foreign_from(status);
    let raw_connector_response = router_data_v2
        .resource_common_data
        .get_raw_connector_response();
    let response_headers = router_data_v2
        .resource_common_data
        .get_connector_response_headers_as_map();

    let response = match transaction_response {
        Ok(response) => match response {
            PaymentsResponseData::AuthenticateResponse {
                resource_id,
                redirection_data,
                authentication_data,
                connector_response_reference_id,
                status_code,
            } => PaymentServiceAuthenticateResponse {
                response_ref_id: connector_response_reference_id.map(|id| {
                    grpc_api_types::payments::Identifier {
                        id_type: Some(grpc_api_types::payments::identifier::IdType::Id(id)),
                    }
                }),
                transaction_id: resource_id
                    .map(grpc_api_types::payments::Identifier::foreign_try_from)
                    .transpose()?,
                redirection_data: redirection_data
                    .map(|form| match *form {
                        router_response_types::RedirectForm::Form {
                            endpoint,
                            method,
                            form_fields,
                        } => Ok::<grpc_api_types::payments::RedirectForm, ApplicationErrorResponse>(
                            grpc_api_types::payments::RedirectForm {
                                form_type: Some(
                                    grpc_api_types::payments::redirect_form::FormType::Form(
                                        grpc_api_types::payments::FormData {
                                            endpoint,
                                            method:
                                                grpc_api_types::payments::HttpMethod::foreign_from(
                                                    method,
                                                )
                                                .into(),
                                            form_fields,
                                        },
                                    ),
                                ),
                            },
                        ),
                        router_response_types::RedirectForm::Html { html_data } => {
                            Ok(grpc_api_types::payments::RedirectForm {
                                form_type: Some(
                                    grpc_api_types::payments::redirect_form::FormType::Html(
                                        grpc_api_types::payments::HtmlData { html_data },
                                    ),
                                ),
                            })
                        }
                        router_response_types::RedirectForm::Uri { uri } => {
                            Ok(grpc_api_types::payments::RedirectForm {
                                form_type: Some(
                                    grpc_api_types::payments::redirect_form::FormType::Uri(
                                        grpc_api_types::payments::UriData { uri },
                                    ),
                                ),
                            })
                        }
                        router_response_types::RedirectForm::Mifinity {
                            initialization_token,
                        } => Ok(grpc_api_types::payments::RedirectForm {
                            form_type: Some(
                                grpc_api_types::payments::redirect_form::FormType::Uri(
                                    grpc_api_types::payments::UriData {
                                        uri: initialization_token,
                                    },
                                ),
                            ),
                        }),
                        router_response_types::RedirectForm::CybersourceConsumerAuth {
                            access_token,
                            step_up_url,
                        } => {
                            let mut form_fields = HashMap::new();
                            form_fields.insert("access_token".to_string(), access_token);
                            form_fields.insert("step_up_url".to_string(), step_up_url.clone());

                            Ok(grpc_api_types::payments::RedirectForm {
                                form_type: Some(
                                    grpc_api_types::payments::redirect_form::FormType::Form(
                                        grpc_api_types::payments::FormData {
                                            endpoint: step_up_url,
                                            method: grpc_api_types::payments::HttpMethod::Post
                                                .into(),
                                            form_fields,
                                        },
                                    ),
                                ),
                            })
                        }
                        _ => Err(ApplicationErrorResponse::BadRequest(ApiError {
                            sub_code: "INVALID_RESPONSE".to_owned(),
                            error_identifier: 400,
                            error_message: "Invalid response from connector".to_owned(),
                            error_object: None,
                        }))?,
                    })
                    .transpose()?,
                connector_metadata: None,
                authentication_data: authentication_data.map(ForeignFrom::foreign_from),
                status: grpc_status.into(),
                error_message: None,
                error_code: None,
                error_reason: None,
                raw_connector_response,
                status_code: status_code.into(),
                response_headers,
                network_txn_id: None,
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
                state: None,
            },
            _ => {
                return Err(ApplicationErrorResponse::BadRequest(ApiError {
                    sub_code: "INVALID_RESPONSE".to_owned(),
                    error_identifier: 400,
                    error_message: "Invalid response type for authenticate".to_owned(),
                    error_object: None,
                })
                .into())
            }
        },
        Err(err) => {
            let status = err
                .attempt_status
                .map(grpc_api_types::payments::PaymentStatus::foreign_from)
                .unwrap_or_default();
            PaymentServiceAuthenticateResponse {
                transaction_id: Some(grpc_api_types::payments::Identifier {
                    id_type: Some(grpc_api_types::payments::identifier::IdType::Id(
                        "session_created".to_string(),
                    )),
                }),
                redirection_data: None,
                network_txn_id: None,
                response_ref_id: None,
                authentication_data: None,
                status: status.into(),
                error_message: Some(err.message),
                error_code: Some(err.code),
                error_reason: err.reason,
                network_decline_code: err.network_decline_code,
                network_advice_code: err.network_advice_code,
                network_error_message: err.network_error_message,
                status_code: err.status_code.into(),
                raw_connector_response,
                response_headers,
                connector_metadata: None,
                state: None,
            }
        }
    };
    Ok(response)
}

pub fn generate_payment_post_authenticate_response<T: PaymentMethodDataTypes>(
    router_data_v2: RouterDataV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    >,
) -> Result<PaymentServicePostAuthenticateResponse, error_stack::Report<ApplicationErrorResponse>> {
    let transaction_response = router_data_v2.response;
    let status = router_data_v2.resource_common_data.status;
    let grpc_status = grpc_api_types::payments::PaymentStatus::foreign_from(status);
    let raw_connector_response = router_data_v2
        .resource_common_data
        .get_raw_connector_response();
    let response_headers = router_data_v2
        .resource_common_data
        .get_connector_response_headers_as_map();

    let response = match transaction_response {
        Ok(response) => match response {
            PaymentsResponseData::PostAuthenticateResponse {
                authentication_data,
                connector_response_reference_id,
                status_code,
            } => PaymentServicePostAuthenticateResponse {
                transaction_id: None,
                redirection_data: None,
                connector_metadata: None,
                network_txn_id: None,
                response_ref_id: connector_response_reference_id.map(|id| {
                    grpc_api_types::payments::Identifier {
                        id_type: Some(grpc_api_types::payments::identifier::IdType::Id(id)),
                    }
                }),
                authentication_data: authentication_data.map(ForeignFrom::foreign_from),
                incremental_authorization_allowed: None,
                status: grpc_status.into(),
                error_message: None,
                error_code: None,
                error_reason: None,
                raw_connector_response,
                status_code: status_code.into(),
                response_headers,
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
                state: None,
            },
            _ => {
                return Err(ApplicationErrorResponse::BadRequest(ApiError {
                    sub_code: "INVALID_RESPONSE".to_owned(),
                    error_identifier: 400,
                    error_message: "Invalid response type for post authenticate".to_owned(),
                    error_object: None,
                })
                .into())
            }
        },
        Err(err) => {
            let status = err
                .attempt_status
                .map(grpc_api_types::payments::PaymentStatus::foreign_from)
                .unwrap_or_default();
            PaymentServicePostAuthenticateResponse {
                transaction_id: Some(grpc_api_types::payments::Identifier {
                    id_type: Some(
                        grpc_api_types::payments::identifier::IdType::NoResponseIdMarker(()),
                    ),
                }),
                redirection_data: None,
                network_txn_id: None,
                response_ref_id: None,
                authentication_data: None,
                incremental_authorization_allowed: None,
                status: status.into(),
                error_message: Some(err.message),
                error_code: Some(err.code),
                error_reason: err.reason,
                network_decline_code: err.network_decline_code,
                network_advice_code: err.network_advice_code,
                network_error_message: err.network_error_message,
                status_code: err.status_code.into(),
                response_headers,
                raw_connector_response,
                connector_metadata: None,
                state: None,
            }
        }
    };
    Ok(response)
}

pub fn generate_mandate_revoke_response(
    router_data_v2: RouterDataV2<
        MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        MandateRevokeResponseData,
    >,
) -> Result<PaymentServiceRevokeMandateResponse, error_stack::Report<ApplicationErrorResponse>> {
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
        Ok(response) => Ok(PaymentServiceRevokeMandateResponse {
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
            error_code: None,
            error_message: None,
            error_reason: None,
            status_code: response.status_code.into(),
            response_headers,
            network_txn_id: None,
            response_ref_id: None,
            raw_connector_response,
            raw_connector_request,
        }),
        Err(e) => Ok(PaymentServiceRevokeMandateResponse {
            status: grpc_api_types::payments::MandateStatus::MandateRevokeFailed.into(), // Default status for failed revoke
            error_code: Some(e.code),
            error_message: Some(e.message),
            error_reason: e.reason,
            status_code: e.status_code.into(),
            response_headers,
            network_txn_id: None,
            response_ref_id: e.connector_transaction_id.map(|id| {
                grpc_api_types::payments::Identifier {
                    id_type: Some(grpc_api_types::payments::identifier::IdType::Id(id)),
                }
            }),
            raw_connector_response,
            raw_connector_request,
        }),
    }
}
