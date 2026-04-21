use axum::{
    extract::{Extension, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use grpc_api_types::payments::{
    customer_service_server::CustomerService, event_service_server::EventService,
    merchant_authentication_service_server::MerchantAuthenticationService,
    payment_method_authentication_service_server::PaymentMethodAuthenticationService,
    payment_method_service_server::PaymentMethodService, payment_service_server::PaymentService,
    recurring_payment_service_server::RecurringPaymentService, CustomerServiceCreateRequest,
    CustomerServiceCreateResponse, EventServiceHandleRequest, EventServiceHandleResponse,
    EventServiceParseRequest, EventServiceParseResponse,
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
    PaymentServiceGetResponse, PaymentServiceRefundRequest, PaymentServiceReverseRequest,
    PaymentServiceReverseResponse, PaymentServiceSetupRecurringRequest,
    PaymentServiceSetupRecurringResponse, PaymentServiceVerifyRedirectResponseRequest,
    PaymentServiceVerifyRedirectResponseResponse, PaymentServiceVoidRequest,
    PaymentServiceVoidResponse, RecurringPaymentServiceChargeRequest,
    RecurringPaymentServiceChargeResponse, RefundResponse,
};
use std::sync::Arc;

use crate::http::handlers::macros::http_handler;
use crate::http::{
    error::HttpError, http_headers_to_grpc_metadata, state::AppState,
    transfer_config_to_grpc_request, utils::ValidatedJson,
};
use ucs_env::configs::Config;

http_handler!(
    authorize,
    PaymentServiceAuthorizeRequest,
    PaymentServiceAuthorizeResponse,
    authorize,
    payments_service
);
// http_handler!(
//     authorize_only,
//     PaymentServiceAuthorizeOnlyRequest,
//     PaymentServiceAuthorizeResponse,
//     authorize_only,
//     payments_service
// );
http_handler!(
    capture,
    PaymentServiceCaptureRequest,
    PaymentServiceCaptureResponse,
    capture,
    payments_service
);
http_handler!(
    void,
    PaymentServiceVoidRequest,
    PaymentServiceVoidResponse,
    void,
    payments_service
);
http_handler!(
    void_post_capture,
    PaymentServiceReverseRequest,
    PaymentServiceReverseResponse,
    reverse,
    payments_service
);
http_handler!(
    get_payment,
    PaymentServiceGetRequest,
    PaymentServiceGetResponse,
    get,
    payments_service
);
http_handler!(
    create_order,
    PaymentServiceCreateOrderRequest,
    PaymentServiceCreateOrderResponse,
    create_order,
    payments_service
);
http_handler!(
    server_session_authentication_token,
    MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest,
    MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse,
    create_server_session_authentication_token,
    merchant_authentication_service
);
http_handler!(
    create_connector_customer,
    CustomerServiceCreateRequest,
    CustomerServiceCreateResponse,
    create,
    customer_service
);
http_handler!(
    create_payment_method_token,
    PaymentMethodServiceTokenizeRequest,
    PaymentMethodServiceTokenizeResponse,
    tokenize,
    payment_method_service
);
http_handler!(
    setup_recurring,
    PaymentServiceSetupRecurringRequest,
    PaymentServiceSetupRecurringResponse,
    setup_recurring,
    payments_service
);
// http_handler!(
//     register_only,
//     PaymentServiceSetupRecurringRequest,
//     PaymentServiceSetupRecurringResponse,
//     register_only,
//     payments_service
// );
http_handler!(
    repeat_everything,
    RecurringPaymentServiceChargeRequest,
    RecurringPaymentServiceChargeResponse,
    charge,
    recurring_payment_service
);
http_handler!(
    refund,
    PaymentServiceRefundRequest,
    RefundResponse,
    refund,
    payments_service
);
http_handler!(
    pre_authenticate,
    PaymentMethodAuthenticationServicePreAuthenticateRequest,
    PaymentMethodAuthenticationServicePreAuthenticateResponse,
    pre_authenticate,
    payment_method_authentication_service
);
http_handler!(
    authenticate,
    PaymentMethodAuthenticationServiceAuthenticateRequest,
    PaymentMethodAuthenticationServiceAuthenticateResponse,
    authenticate,
    payment_method_authentication_service
);
http_handler!(
    post_authenticate,
    PaymentMethodAuthenticationServicePostAuthenticateRequest,
    PaymentMethodAuthenticationServicePostAuthenticateResponse,
    post_authenticate,
    payment_method_authentication_service
);
http_handler!(
    server_authentication_token,
    MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest,
    MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse,
    create_server_authentication_token,
    merchant_authentication_service
);
http_handler!(
    transform,
    EventServiceHandleRequest,
    EventServiceHandleResponse,
    handle_event,
    event_service
);
http_handler!(
    parse_event,
    EventServiceParseRequest,
    EventServiceParseResponse,
    parse_event,
    event_service
);
http_handler!(
    handle_event,
    EventServiceHandleRequest,
    EventServiceHandleResponse,
    handle_event,
    event_service
);
http_handler!(
    verify_redirect_response,
    PaymentServiceVerifyRedirectResponseRequest,
    PaymentServiceVerifyRedirectResponseResponse,
    verify_redirect_response,
    payments_service
);
