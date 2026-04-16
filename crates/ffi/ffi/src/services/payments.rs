use crate::macros::{req_transformer, res_transformer};
use external_services;
use grpc_api_types::payments::ConnectorError;
use grpc_api_types::payments::{
    CustomerServiceCreateRequest, CustomerServiceCreateResponse, DisputeServiceAcceptRequest,
    DisputeServiceAcceptResponse, DisputeServiceDefendRequest, DisputeServiceDefendResponse,
    DisputeServiceSubmitEvidenceRequest, DisputeServiceSubmitEvidenceResponse,
    EventServiceHandleRequest, EventServiceHandleResponse,
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
    PaymentServiceVoidRequest, PaymentServiceVoidResponse, RecurringPaymentServiceChargeRequest,
    RecurringPaymentServiceChargeResponse, RecurringPaymentServiceRevokeRequest,
    RecurringPaymentServiceRevokeResponse, RefundResponse, RefundServiceGetRequest,
};

use domain_types::{
    connector_flow::{
        Accept, Authenticate, Authorize, Capture, ClientAuthenticationToken,
        CreateConnectorCustomer, CreateOrder, DefendDispute, IncrementalAuthorization,
        MandateRevoke, PSync, PaymentMethodToken, PostAuthenticate, PreAuthenticate, RSync, Refund,
        RepeatPayment, ServerAuthenticationToken, ServerSessionAuthenticationToken, SetupMandate,
        SubmitEvidence, Void, VoidPC,
    },
    connector_types::{
        AcceptDisputeData, ClientAuthenticationTokenRequestData, ConnectorCustomerData,
        ConnectorCustomerResponse, ConnectorWebhookSecrets, DisputeDefendData, DisputeFlowData,
        DisputeResponseData, MandateRevokeRequestData, MandateRevokeResponseData,
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData,
        PaymentMethodTokenResponse, PaymentMethodTokenizationData, PaymentVoidData,
        PaymentsAuthenticateData, PaymentsAuthorizeData, PaymentsCancelPostCaptureData,
        PaymentsCaptureData, PaymentsIncrementalAuthorizationData, PaymentsPostAuthenticateData,
        PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData, RequestDetails,
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
        ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData,
        SetupMandateRequestData, SubmitEvidenceData,
    },
};

// authorize request transformer
req_transformer!(
    fn_name: authorize_req_transformer,
    request_type: PaymentServiceAuthorizeRequest,
    flow_marker: Authorize,
    resource_common_data_type: PaymentFlowData,
    request_data_type: PaymentsAuthorizeData<T>,
    response_data_type: PaymentsResponseData,
);

// authorize response transformer
res_transformer!(
    fn_name: authorize_res_transformer,
    request_type: PaymentServiceAuthorizeRequest,
    response_type: PaymentServiceAuthorizeResponse,
    flow_marker: Authorize,
    resource_common_data_type: PaymentFlowData,
    request_data_type: PaymentsAuthorizeData<T>,
    response_data_type: PaymentsResponseData,
    generate_response_fn: generate_payment_authorize_response,
);

// capture request transformer
req_transformer!(
    fn_name: capture_req_transformer,
    request_type: PaymentServiceCaptureRequest,
    flow_marker: Capture,
    resource_common_data_type: PaymentFlowData,
    request_data_type: PaymentsCaptureData,
    response_data_type: PaymentsResponseData,
);

// capture response transformer
res_transformer!(
    fn_name: capture_res_transformer,
    request_type: PaymentServiceCaptureRequest,
    response_type: PaymentServiceCaptureResponse,
    flow_marker: Capture,
    resource_common_data_type: PaymentFlowData,
    request_data_type: PaymentsCaptureData,
    response_data_type: PaymentsResponseData,
    generate_response_fn: generate_payment_capture_response,
);

// void request transformer
req_transformer!(
    fn_name: void_req_transformer,
    request_type: PaymentServiceVoidRequest,
    flow_marker: Void,
    resource_common_data_type: PaymentFlowData,
    request_data_type: PaymentVoidData,
    response_data_type: PaymentsResponseData,
);

// void response transformer
res_transformer!(
    fn_name: void_res_transformer,
    request_type: PaymentServiceVoidRequest,
    response_type: PaymentServiceVoidResponse,
    flow_marker: Void,
    resource_common_data_type: PaymentFlowData,
    request_data_type: PaymentVoidData,
    response_data_type: PaymentsResponseData,
    generate_response_fn: generate_payment_void_response,
);

// psync request transformer
req_transformer!(
    fn_name: get_req_transformer,
    request_type: PaymentServiceGetRequest,
    flow_marker: PSync,
    resource_common_data_type: PaymentFlowData,
    request_data_type: PaymentsSyncData,
    response_data_type: PaymentsResponseData,
);

// psync response transformer
res_transformer!(
    fn_name: get_res_transformer,
    request_type: PaymentServiceGetRequest,
    response_type: PaymentServiceGetResponse,
    flow_marker: PSync,
    resource_common_data_type: PaymentFlowData,
    request_data_type: PaymentsSyncData,
    response_data_type: PaymentsResponseData,
    generate_response_fn: generate_payment_sync_response,
);

// create order request transformer
req_transformer!(
    fn_name: create_order_req_transformer,
    request_type: PaymentServiceCreateOrderRequest,
    flow_marker: CreateOrder,
    resource_common_data_type: PaymentFlowData,
    request_data_type: PaymentCreateOrderData,
    response_data_type: PaymentCreateOrderResponse,
);

// create order response transformer
res_transformer!(
    fn_name: create_order_res_transformer,
    request_type: PaymentServiceCreateOrderRequest,
    response_type: PaymentServiceCreateOrderResponse,
    flow_marker: CreateOrder,
    resource_common_data_type: PaymentFlowData,
    request_data_type: PaymentCreateOrderData,
    response_data_type: PaymentCreateOrderResponse,
    generate_response_fn: generate_create_order_response,
);

// create access token request transformer
req_transformer!(
    fn_name: create_server_authentication_token_req_transformer,
    request_type: MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest,
    flow_marker: ServerAuthenticationToken,
    resource_common_data_type: PaymentFlowData,
    request_data_type: ServerAuthenticationTokenRequestData,
    response_data_type: ServerAuthenticationTokenResponseData,
);

// create access token response transformer
res_transformer!(
    fn_name: create_server_authentication_token_res_transformer,
    request_type: MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest,
    response_type: MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse,
    flow_marker: ServerAuthenticationToken,
    resource_common_data_type: PaymentFlowData,
    request_data_type: ServerAuthenticationTokenRequestData,
    response_data_type: ServerAuthenticationTokenResponseData,
    generate_response_fn: generate_access_token_response,
);

// refund request transformer
req_transformer!(
    fn_name: refund_req_transformer,
    request_type: PaymentServiceRefundRequest,
    flow_marker: Refund,
    resource_common_data_type: RefundFlowData,
    request_data_type: RefundsData,
    response_data_type: RefundsResponseData,
);

// refund response transformer
res_transformer!(
    fn_name: refund_res_transformer,
    request_type: PaymentServiceRefundRequest,
    response_type: RefundResponse,
    flow_marker: Refund,
    resource_common_data_type: RefundFlowData,
    request_data_type: RefundsData,
    response_data_type: RefundsResponseData,
    generate_response_fn: generate_refund_response,
);

// reverse (void post-capture) request transformer
req_transformer!(
    fn_name: reverse_req_transformer,
    request_type: PaymentServiceReverseRequest,
    flow_marker: VoidPC,
    resource_common_data_type: PaymentFlowData,
    request_data_type: PaymentsCancelPostCaptureData,
    response_data_type: PaymentsResponseData,
);

// reverse (void post-capture) response transformer
res_transformer!(
    fn_name: reverse_res_transformer,
    request_type: PaymentServiceReverseRequest,
    response_type: PaymentServiceReverseResponse,
    flow_marker: VoidPC,
    resource_common_data_type: PaymentFlowData,
    request_data_type: PaymentsCancelPostCaptureData,
    response_data_type: PaymentsResponseData,
    generate_response_fn: generate_payment_void_post_capture_response,
);

// create connector customer request transformer
req_transformer!(
    fn_name: create_req_transformer,
    request_type: CustomerServiceCreateRequest,
    flow_marker: CreateConnectorCustomer,
    resource_common_data_type: PaymentFlowData,
    request_data_type: ConnectorCustomerData,
    response_data_type: ConnectorCustomerResponse,
);

// create connector customer response transformer
res_transformer!(
    fn_name: create_res_transformer,
    request_type: CustomerServiceCreateRequest,
    response_type: CustomerServiceCreateResponse,
    flow_marker: CreateConnectorCustomer,
    resource_common_data_type: PaymentFlowData,
    request_data_type: ConnectorCustomerData,
    response_data_type: ConnectorCustomerResponse,
    generate_response_fn: generate_create_connector_customer_response,
);

// repeat payment (charge) request transformer
req_transformer!(
    fn_name: charge_req_transformer,
    request_type: RecurringPaymentServiceChargeRequest,
    flow_marker: RepeatPayment,
    resource_common_data_type: PaymentFlowData,
    request_data_type: RepeatPaymentData<T>,
    response_data_type: PaymentsResponseData,
);

// repeat payment (charge) response transformer
res_transformer!(
    fn_name: charge_res_transformer,
    request_type: RecurringPaymentServiceChargeRequest,
    response_type: RecurringPaymentServiceChargeResponse,
    flow_marker: RepeatPayment,
    resource_common_data_type: PaymentFlowData,
    request_data_type: RepeatPaymentData<T>,
    response_data_type: PaymentsResponseData,
    generate_response_fn: generate_repeat_payment_response,
);

// create session token request transformer
req_transformer!(
    fn_name: create_server_session_authentication_token_req_transformer,
    request_type: MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest,
    flow_marker: ServerSessionAuthenticationToken,
    resource_common_data_type: PaymentFlowData,
    request_data_type: ServerSessionAuthenticationTokenRequestData,
    response_data_type: ServerSessionAuthenticationTokenResponseData,
);

// create session token response transformer
res_transformer!(
    fn_name: create_server_session_authentication_token_res_transformer,
    request_type: MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest,
    response_type: MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse,
    flow_marker: ServerSessionAuthenticationToken,
    resource_common_data_type: PaymentFlowData,
    request_data_type: ServerSessionAuthenticationTokenRequestData,
    response_data_type: ServerSessionAuthenticationTokenResponseData,
    generate_response_fn: generate_session_token_response,
);

// client authentication token request transformer
req_transformer!(
    fn_name: create_client_authentication_token_req_transformer,
    request_type: MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest,
    flow_marker: ClientAuthenticationToken,
    resource_common_data_type: PaymentFlowData,
    request_data_type: ClientAuthenticationTokenRequestData,
    response_data_type: PaymentsResponseData,
);

// client authentication token response transformer
res_transformer!(
    fn_name: create_client_authentication_token_res_transformer,
    request_type: MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest,
    response_type: MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse,
    flow_marker: ClientAuthenticationToken,
    resource_common_data_type: PaymentFlowData,
    request_data_type: ClientAuthenticationTokenRequestData,
    response_data_type: PaymentsResponseData,
    generate_response_fn: generate_payment_sdk_session_token_response,
);

// setup recurring (setup mandate) request transformer
req_transformer!(
    fn_name: setup_recurring_req_transformer,
    request_type: PaymentServiceSetupRecurringRequest,
    flow_marker: SetupMandate,
    resource_common_data_type: PaymentFlowData,
    request_data_type: SetupMandateRequestData<T>,
    response_data_type: PaymentsResponseData,
);

// setup recurring (setup mandate) response transformer
res_transformer!(
    fn_name: setup_recurring_res_transformer,
    request_type: PaymentServiceSetupRecurringRequest,
    response_type: PaymentServiceSetupRecurringResponse,
    flow_marker: SetupMandate,
    resource_common_data_type: PaymentFlowData,
    request_data_type: SetupMandateRequestData<T>,
    response_data_type: PaymentsResponseData,
    generate_response_fn: generate_setup_mandate_response,
);

// tokenize (payment method token) request transformer
req_transformer!(
    fn_name: tokenize_req_transformer,
    request_type: PaymentMethodServiceTokenizeRequest,
    flow_marker: PaymentMethodToken,
    resource_common_data_type: PaymentFlowData,
    request_data_type: PaymentMethodTokenizationData<T>,
    response_data_type: PaymentMethodTokenResponse,
);

// tokenize (payment method token) response transformer
res_transformer!(
    fn_name: tokenize_res_transformer,
    request_type: PaymentMethodServiceTokenizeRequest,
    response_type: PaymentMethodServiceTokenizeResponse,
    flow_marker: PaymentMethodToken,
    resource_common_data_type: PaymentFlowData,
    request_data_type: PaymentMethodTokenizationData<T>,
    response_data_type: PaymentMethodTokenResponse,
    generate_response_fn: generate_create_payment_method_token_response,
);

// pre_authenticate request transformer
req_transformer!(
    fn_name: pre_authenticate_req_transformer,
    request_type: PaymentMethodAuthenticationServicePreAuthenticateRequest,
    flow_marker: PreAuthenticate,
    resource_common_data_type: PaymentFlowData,
    request_data_type: PaymentsPreAuthenticateData<T>,
    response_data_type: PaymentsResponseData,
);

// pre_authenticate response transformer
res_transformer!(
    fn_name: pre_authenticate_res_transformer,
    request_type: PaymentMethodAuthenticationServicePreAuthenticateRequest,
    response_type: PaymentMethodAuthenticationServicePreAuthenticateResponse,
    flow_marker: PreAuthenticate,
    resource_common_data_type: PaymentFlowData,
    request_data_type: PaymentsPreAuthenticateData<T>,
    response_data_type: PaymentsResponseData,
    generate_response_fn: generate_payment_pre_authenticate_response,
);

// authenticate request transformer
req_transformer!(
    fn_name: authenticate_req_transformer,
    request_type: PaymentMethodAuthenticationServiceAuthenticateRequest,
    flow_marker: Authenticate,
    resource_common_data_type: PaymentFlowData,
    request_data_type: PaymentsAuthenticateData<T>,
    response_data_type: PaymentsResponseData,
);

// authenticate response transformer
res_transformer!(
    fn_name: authenticate_res_transformer,
    request_type: PaymentMethodAuthenticationServiceAuthenticateRequest,
    response_type: PaymentMethodAuthenticationServiceAuthenticateResponse,
    flow_marker: Authenticate,
    resource_common_data_type: PaymentFlowData,
    request_data_type: PaymentsAuthenticateData<T>,
    response_data_type: PaymentsResponseData,
    generate_response_fn: generate_payment_authenticate_response,
);

// post_authenticate request transformer
req_transformer!(
    fn_name: post_authenticate_req_transformer,
    request_type: PaymentMethodAuthenticationServicePostAuthenticateRequest,
    flow_marker: PostAuthenticate,
    resource_common_data_type: PaymentFlowData,
    request_data_type: PaymentsPostAuthenticateData<T>,
    response_data_type: PaymentsResponseData,
);

// post_authenticate response transformer
res_transformer!(
    fn_name: post_authenticate_res_transformer,
    request_type: PaymentMethodAuthenticationServicePostAuthenticateRequest,
    response_type: PaymentMethodAuthenticationServicePostAuthenticateResponse,
    flow_marker: PostAuthenticate,
    resource_common_data_type: PaymentFlowData,
    request_data_type: PaymentsPostAuthenticateData<T>,
    response_data_type: PaymentsResponseData,
    generate_response_fn: generate_payment_post_authenticate_response,
);

// accept request transformer
req_transformer!(
    fn_name: accept_req_transformer,
    request_type: DisputeServiceAcceptRequest,
    flow_marker: Accept,
    resource_common_data_type: DisputeFlowData,
    request_data_type: AcceptDisputeData,
    response_data_type: DisputeResponseData,
);

// submit_evidence request transformer
req_transformer!(
    fn_name: submit_evidence_req_transformer,
    request_type: DisputeServiceSubmitEvidenceRequest,
    flow_marker: SubmitEvidence,
    resource_common_data_type: DisputeFlowData,
    request_data_type: SubmitEvidenceData,
    response_data_type: DisputeResponseData,
);

// defend request transformer
req_transformer!(
    fn_name: defend_req_transformer,
    request_type: DisputeServiceDefendRequest,
    flow_marker: DefendDispute,
    resource_common_data_type: DisputeFlowData,
    request_data_type: DisputeDefendData,
    response_data_type: DisputeResponseData,
);

// accept response transformer
res_transformer!(
    fn_name: accept_res_transformer,
    request_type: DisputeServiceAcceptRequest,
    response_type: DisputeServiceAcceptResponse,
    flow_marker: Accept,
    resource_common_data_type: DisputeFlowData,
    request_data_type: AcceptDisputeData,
    response_data_type: DisputeResponseData,
    generate_response_fn: generate_accept_dispute_response,
);

// submit_evidence response transformer
res_transformer!(
    fn_name: submit_evidence_res_transformer,
    request_type: DisputeServiceSubmitEvidenceRequest,
    response_type: DisputeServiceSubmitEvidenceResponse,
    flow_marker: SubmitEvidence,
    resource_common_data_type: DisputeFlowData,
    request_data_type: SubmitEvidenceData,
    response_data_type: DisputeResponseData,
    generate_response_fn: generate_submit_evidence_response,
);

// defend response transformer
res_transformer!(
    fn_name: defend_res_transformer,
    request_type: DisputeServiceDefendRequest,
    response_type: DisputeServiceDefendResponse,
    flow_marker: DefendDispute,
    resource_common_data_type: DisputeFlowData,
    request_data_type: DisputeDefendData,
    response_data_type: DisputeResponseData,
    generate_response_fn: generate_defend_dispute_response,
);

/// handle_event — synchronous webhook processing (single-step, no outgoing HTTP).
///
/// The caller supplies the raw webhook body + headers received from the connector
/// and gets back a fully-structured `EventServiceHandleResponse`.
///
/// External source verification (async HTTP used by PayPal / Stripe) is **not**
/// performed here; only local synchronous signature verification is done.
/// The gRPC server performs external verification before calling its equivalent path.
pub fn handle_event_transformer(
    payload: EventServiceHandleRequest,
    _config: &std::sync::Arc<ucs_env::configs::Config>,
    connector: domain_types::connector_types::ConnectorEnum,
    connector_config: domain_types::router_data::ConnectorSpecificConfig,
    _metadata: &common_utils::metadata::MaskedMetadata,
) -> Result<EventServiceHandleResponse, ConnectorError> {
    use domain_types::utils::ForeignTryFrom as _;

    let request_details = payload.request_details.ok_or_else(|| ConnectorError {
        error_message: "Missing required field: request_details".to_string(),
        error_code: "MISSING_REQUIRED_FIELD".to_string(),
        http_status_code: None,
    })?;
    let request_details =
        RequestDetails::foreign_try_from(request_details).map_err(|e| ConnectorError {
            error_message: format!("ForeignTryFrom failed: {e}"),
            error_code: "CONVERSION_FAILED".to_string(),
            http_status_code: None,
        })?;

    let webhook_secrets = payload
        .webhook_secrets
        .map(|ws| {
            ConnectorWebhookSecrets::foreign_try_from(ws).map_err(|e| ConnectorError {
                error_message: format!("ForeignTryFrom failed: {e}"),
                error_code: "CONVERSION_FAILED".to_string(),
                http_status_code: None,
            })
        })
        .transpose()?;

    let connector_data: connector_integration::types::ConnectorData<
        domain_types::payment_method_data::DefaultPCIHolder,
    > = connector_integration::types::ConnectorData::get_connector_by_name(&connector);

    // Local synchronous source verification only (no external HTTP call in FFI).
    let source_verified = connector_data
        .connector
        .verify_webhook_source(
            request_details.clone(),
            webhook_secrets.clone(),
            Some(connector_config.clone()),
        )
        .unwrap_or(false);

    connector_integration::webhook_utils::process_webhook_event(
        connector_data,
        request_details,
        webhook_secrets,
        Some(connector_config),
        source_verified,
    )
    .map_err(
        |e: error_stack::Report<domain_types::errors::WebhookError>| {
            let ctx = e.current_context();
            ConnectorError {
                error_message: ctx.to_string(),
                error_code: ctx.as_ref().to_string(),
                http_status_code: None,
            }
        },
    )
}

// incremental_authorization
req_transformer!(
    fn_name: incremental_authorization_req_transformer,
    request_type: PaymentServiceIncrementalAuthorizationRequest,
    flow_marker: IncrementalAuthorization,
    resource_common_data_type: PaymentFlowData,
    request_data_type: PaymentsIncrementalAuthorizationData,
    response_data_type: PaymentsResponseData,
);

// incremental_authorization response transformer
res_transformer!(
    fn_name: incremental_authorization_res_transformer,
    request_type: PaymentServiceIncrementalAuthorizationRequest,
    response_type: PaymentServiceIncrementalAuthorizationResponse,
    flow_marker: IncrementalAuthorization,
    resource_common_data_type: PaymentFlowData,
    request_data_type: PaymentsIncrementalAuthorizationData,
    response_data_type: PaymentsResponseData,
    generate_response_fn: generate_payment_incremental_authorization_response,
);

// refund_get (RefundService.Get) - rsync
req_transformer!(
    fn_name: refund_get_req_transformer,
    request_type: RefundServiceGetRequest,
    flow_marker: RSync,
    resource_common_data_type: RefundFlowData,
    request_data_type: RefundSyncData,
    response_data_type: RefundsResponseData,
);

// refund_get response transformer
res_transformer!(
    fn_name: refund_get_res_transformer,
    request_type: RefundServiceGetRequest,
    response_type: RefundResponse,
    flow_marker: RSync,
    resource_common_data_type: RefundFlowData,
    request_data_type: RefundSyncData,
    response_data_type: RefundsResponseData,
    generate_response_fn: generate_refund_sync_response,
);

// create_sdk_session_token (MerchantAuthenticationService.CreateSdkSessionToken)
req_transformer!(
    fn_name: create_client_authentication_token_req_handler,
    request_type: MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest,
    flow_marker: ClientAuthenticationToken,
    resource_common_data_type: PaymentFlowData,
    request_data_type: ClientAuthenticationTokenRequestData,
    response_data_type: PaymentsResponseData,
);

// create_sdk_session_token response transformer
res_transformer!(
    fn_name: create_client_authentication_token_res_handler,
    request_type: MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest,
    response_type: MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse,
    flow_marker: ClientAuthenticationToken,
    resource_common_data_type: PaymentFlowData,
    request_data_type: ClientAuthenticationTokenRequestData,
    response_data_type: PaymentsResponseData,
    generate_response_fn: generate_payment_sdk_session_token_response,
);

// recurring_revoke (RecurringPaymentService.Revoke)
req_transformer!(
    fn_name: recurring_revoke_req_transformer,
    request_type: RecurringPaymentServiceRevokeRequest,
    flow_marker: MandateRevoke,
    resource_common_data_type: PaymentFlowData,
    request_data_type: MandateRevokeRequestData,
    response_data_type: MandateRevokeResponseData,
);

// recurring_revoke response transformer
res_transformer!(
    fn_name: recurring_revoke_res_transformer,
    request_type: RecurringPaymentServiceRevokeRequest,
    response_type: RecurringPaymentServiceRevokeResponse,
    flow_marker: MandateRevoke,
    resource_common_data_type: PaymentFlowData,
    request_data_type: MandateRevokeRequestData,
    response_data_type: MandateRevokeResponseData,
    generate_response_fn: generate_mandate_revoke_response,
);

/// verify_redirect_response — synchronous verification of redirect response (no outgoing HTTP call).
///
/// Calls `decode_redirect_response_body`, `verify_redirect_response_source`, and
/// `process_redirect_response` on the connector, mirroring what the gRPC server does.
pub fn verify_redirect_response_transformer(
    payload: PaymentServiceVerifyRedirectResponseRequest,
    _config: &std::sync::Arc<ucs_env::configs::Config>,
    connector: domain_types::connector_types::ConnectorEnum,
    _connector_config: domain_types::router_data::ConnectorSpecificConfig,
    _metadata: &common_utils::metadata::MaskedMetadata,
) -> Result<PaymentServiceVerifyRedirectResponseResponse, ConnectorError> {
    use domain_types::utils::ForeignTryFrom as _;
    use interfaces::verification::ConnectorSourceVerificationSecrets;

    let request_details_proto = payload.request_details.ok_or_else(|| ConnectorError {
        error_message: "Missing required field: request_details".to_string(),
        error_code: "MISSING_REQUIRED_FIELD".to_string(),
        http_status_code: None,
    })?;

    let request_details =
        RequestDetails::foreign_try_from(request_details_proto).map_err(|e| ConnectorError {
            error_message: format!("ForeignTryFrom failed: {e}"),
            error_code: "CONVERSION_FAILED".to_string(),
            http_status_code: None,
        })?;

    let secrets = payload
        .redirect_response_secrets
        .map(|s| {
            domain_types::connector_types::ConnectorRedirectResponseSecrets::foreign_try_from(s)
                .map_err(|e| ConnectorError {
                    error_message: format!("ForeignTryFrom failed: {e}"),
                    error_code: "CONVERSION_FAILED".to_string(),
                    http_status_code: None,
                })
        })
        .transpose()?
        .map(ConnectorSourceVerificationSecrets::RedirectResponseSecret);

    let connector_data: connector_integration::types::ConnectorData<
        domain_types::payment_method_data::DefaultPCIHolder,
    > = connector_integration::types::ConnectorData::get_connector_by_name(&connector);

    let decoded_body = connector_data
        .connector
        .decode_redirect_response_body(&request_details, secrets.clone())
        .unwrap_or_else(|_| request_details.body.clone());

    let updated_request_details = RequestDetails {
        method: request_details.method,
        uri: request_details.uri,
        headers: request_details.headers,
        query_params: request_details.query_params,
        body: decoded_body,
    };

    let source_verified = connector_data
        .connector
        .verify_redirect_response_source(&updated_request_details, secrets)
        .unwrap_or(false);

    let redirect_details = connector_data
        .connector
        .process_redirect_response(&updated_request_details)
        .map_err(|e| ConnectorError {
            error_message: format!("{e}"),
            error_code: "PROCESS_REDIRECT_ERROR".to_string(),
            http_status_code: None,
        })?;

    PaymentServiceVerifyRedirectResponseResponse::foreign_try_from((
        source_verified,
        redirect_details,
    ))
    .map_err(|e| ConnectorError {
        error_message: format!("ForeignTryFrom failed: {e}"),
        error_code: "CONVERSION_FAILED".to_string(),
        http_status_code: None,
    })
}

// token_authorize
req_transformer!(
    fn_name: token_authorize_req_transformer,
    request_type: PaymentServiceTokenAuthorizeRequest,
    flow_marker: Authorize,
    resource_common_data_type: PaymentFlowData,
    request_data_type: PaymentsAuthorizeData<T>,
    response_data_type: PaymentsResponseData,
);

res_transformer!(
    fn_name: token_authorize_res_transformer,
    request_type: PaymentServiceTokenAuthorizeRequest,
    response_type: PaymentServiceAuthorizeResponse,
    flow_marker: Authorize,
    resource_common_data_type: PaymentFlowData,
    request_data_type: PaymentsAuthorizeData<T>,
    response_data_type: PaymentsResponseData,
    generate_response_fn: generate_payment_authorize_response,
);

// token_setup_recurring
req_transformer!(
    fn_name: token_setup_recurring_req_transformer,
    request_type: PaymentServiceTokenSetupRecurringRequest,
    flow_marker: SetupMandate,
    resource_common_data_type: PaymentFlowData,
    request_data_type: SetupMandateRequestData<T>,
    response_data_type: PaymentsResponseData,
);

res_transformer!(
    fn_name: token_setup_recurring_res_transformer,
    request_type: PaymentServiceTokenSetupRecurringRequest,
    response_type: PaymentServiceSetupRecurringResponse,
    flow_marker: SetupMandate,
    resource_common_data_type: PaymentFlowData,
    request_data_type: SetupMandateRequestData<T>,
    response_data_type: PaymentsResponseData,
    generate_response_fn: generate_setup_mandate_response,
);

// proxy_authorize
req_transformer!(
    fn_name: proxy_authorize_req_transformer,
    request_type: PaymentServiceProxyAuthorizeRequest,
    flow_marker: Authorize,
    resource_common_data_type: PaymentFlowData,
    request_data_type: PaymentsAuthorizeData<T>,
    response_data_type: PaymentsResponseData,
);

res_transformer!(
    fn_name: proxy_authorize_res_transformer,
    request_type: PaymentServiceProxyAuthorizeRequest,
    response_type: PaymentServiceAuthorizeResponse,
    flow_marker: Authorize,
    resource_common_data_type: PaymentFlowData,
    request_data_type: PaymentsAuthorizeData<T>,
    response_data_type: PaymentsResponseData,
    generate_response_fn: generate_payment_authorize_response,
);

// proxy_setup_recurring
req_transformer!(
    fn_name: proxy_setup_recurring_req_transformer,
    request_type: PaymentServiceProxySetupRecurringRequest,
    flow_marker: SetupMandate,
    resource_common_data_type: PaymentFlowData,
    request_data_type: SetupMandateRequestData<T>,
    response_data_type: PaymentsResponseData,
);

res_transformer!(
    fn_name: proxy_setup_recurring_res_transformer,
    request_type: PaymentServiceProxySetupRecurringRequest,
    response_type: PaymentServiceSetupRecurringResponse,
    flow_marker: SetupMandate,
    resource_common_data_type: PaymentFlowData,
    request_data_type: SetupMandateRequestData<T>,
    response_data_type: PaymentsResponseData,
    generate_response_fn: generate_setup_mandate_response,
);
