use domain_types::connector_types::{ConnectorEnum, ConnectorVariant};
use grpc_api_types::payments::{
    CompositeAuthorizeRequest, CompositeCaptureRequest, CompositeGetRequest,
    CompositePaymentMethodCreateRequest, CompositePaymentMethodGetRequest,
    CompositePaymentMethodRechargeRequest, CompositePreAuthenticateRequest,
    CompositeRefundGetRequest, CompositeRefundRequest, CompositeVerifyRedirectResponseRequest,
    CompositeVoidRequest, ConnectorState, CustomerServiceCreateRequest,
    CustomerServiceCreateResponse, CustomerServiceGetRequest, CustomerServiceGetResponse,
    MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest,
    MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse,
    MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest,
    MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse,
    PaymentMethodAuthenticationServiceAuthenticateRequest,
    PaymentMethodAuthenticationServiceAuthenticateResponse,
    PaymentMethodAuthenticationServicePostAuthenticateRequest,
    PaymentMethodAuthenticationServicePostAuthenticateResponse,
    PaymentMethodAuthenticationServicePreAuthenticateRequest,
    PaymentMethodAuthenticationServicePreAuthenticateResponse, PaymentMethodServiceCreateRequest,
    PaymentMethodServiceGetRequest, PaymentMethodServiceRechargeRequest,
    PaymentMethodServiceTokenizeRequest, PaymentMethodServiceTokenizeResponse,
    PaymentServiceAuthorizeRequest, PaymentServiceCaptureRequest, PaymentServiceCreateOrderRequest,
    PaymentServiceCreateOrderResponse, PaymentServiceGetRequest, PaymentServiceRefundRequest,
    PaymentServiceVerifyRedirectResponseResponse, PaymentServiceVoidRequest,
    RefundServiceGetRequest,
};

use crate::utils::{
    get_access_token, get_connector_customer_id, get_payment_method_token, get_session_token,
    grpc_connector_from_connector_variant,
};

pub trait ForeignFrom<F>: Sized {
    fn foreign_from(item: F) -> Self;
}

pub trait ForeignTryFrom<F>: Sized {
    type Error;
    fn foreign_try_from(item: F) -> Result<Self, Self::Error>;
}

/// Convert a `CustomerServiceGetResponse` into a create-shaped response for
/// downstream callers that expect `CustomerServiceCreateResponse`. Returns
/// `Err(())` when the get response carries no customer with a
/// `connector_customer_id` — caller should treat that as "not found on
/// connector" and fall through to CREATE.
impl ForeignTryFrom<CustomerServiceGetResponse> for CustomerServiceCreateResponse {
    type Error = ();

    fn foreign_try_from(get_response: CustomerServiceGetResponse) -> Result<Self, Self::Error> {
        let connector_customer_id = get_response
            .customer
            .as_ref()
            .and_then(|c| c.connector_customer_id.clone())
            .ok_or(())?;

        Ok(Self {
            merchant_customer_id: get_response.merchant_customer_id,
            connector_customer_id,
            error: None,
            status_code: get_response.status_code,
            response_headers: get_response.response_headers,
        })
    }
}

impl ForeignFrom<(&CompositeAuthorizeRequest, &ConnectorVariant)>
    for MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest
{
    fn foreign_from((item, connector): (&CompositeAuthorizeRequest, &ConnectorVariant)) -> Self {
        Self {
            merchant_access_token_id: item.merchant_access_token_id.clone(),
            connector: grpc_connector_from_connector_variant(connector),
            metadata: item.metadata.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            test_mode: item.test_mode,
            merchant_request_id: item.merchant_request_id.clone(),
        }
    }
}

impl ForeignFrom<(&CompositePreAuthenticateRequest, &ConnectorVariant)>
    for MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest
{
    fn foreign_from(
        (item, connector): (&CompositePreAuthenticateRequest, &ConnectorVariant),
    ) -> Self {
        Self {
            merchant_access_token_id: item.merchant_access_token_id.clone(),
            connector: grpc_connector_from_connector_variant(connector),
            metadata: item.metadata.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            test_mode: item.test_mode,
            merchant_request_id: item.merchant_request_id.clone(),
        }
    }
}

impl ForeignFrom<(&CompositeAuthorizeRequest, &ConnectorEnum)>
    for MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest
{
    fn foreign_from((item, _connector): (&CompositeAuthorizeRequest, &ConnectorEnum)) -> Self {
        use grpc_api_types::payments::{
            merchant_authentication_service_create_server_session_authentication_token_request::DomainContext,
            PaymentSessionContext,
        };

        Self {
            merchant_server_session_id: item.merchant_server_session_id.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            state: item.state.clone(),
            test_mode: item.test_mode,
            domain_context: Some(DomainContext::Payment(PaymentSessionContext {
                amount: item.amount,
                metadata: item.metadata.clone(),
                browser_info: item.browser_info.clone(),
                customer: item.customer.clone(),
                address: item.address.clone(),
            })),
        }
    }
}

// Tuple variant: threads the freshly-created connector_customer_id from
// `create_customer_response` into the outgoing state. Required for connectors
// that do not cache the connector-side customer id externally (e.g. Glomopay),
// where a first-time-customer flow would otherwise send an order-create request
// with an empty state.connector_customer_id.
impl
    ForeignFrom<(
        &CompositeAuthorizeRequest,
        Option<&CustomerServiceCreateResponse>,
        interfaces::connector_types::MerchantOrderIdSource,
    )> for PaymentServiceCreateOrderRequest
{
    fn foreign_from(
        (item, create_customer_response, merchant_order_id_source): (
            &CompositeAuthorizeRequest,
            Option<&CustomerServiceCreateResponse>,
            interfaces::connector_types::MerchantOrderIdSource,
        ),
    ) -> Self {
        let connector_customer_id_from_req = item
            .state
            .as_ref()
            .and_then(|state| state.connector_customer_id.clone());
        let connector_customer_id =
            get_connector_customer_id(connector_customer_id_from_req, create_customer_response);

        let state = Some(ConnectorState {
            access_token: item.state.as_ref().and_then(|s| s.access_token.clone()),
            connector_customer_id,
        });

        let merchant_order_id = match merchant_order_id_source {
            interfaces::connector_types::MerchantOrderIdSource::OrderId => item.merchant_order_id.clone(),
            interfaces::connector_types::MerchantOrderIdSource::TransactionId => item.merchant_transaction_id.clone(),
        };

        Self {
            merchant_order_id,
            amount: item.amount,
            webhook_url: item.webhook_url.clone(),
            metadata: item.metadata.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            state,
            test_mode: item.test_mode,
            payment_method_type: None,
            order_details: item.order_details.clone(),
        }
    }
}

impl ForeignFrom<&CompositeAuthorizeRequest> for CustomerServiceCreateRequest {
    fn foreign_from(item: &CompositeAuthorizeRequest) -> Self {
        let customer = item.customer.as_ref();
        Self {
            merchant_customer_id: item
                .merchant_customer_id
                .clone()
                .or_else(|| customer.and_then(|c| c.id.clone())),
            customer_name: item
                .customer_name
                .clone()
                .or_else(|| customer.and_then(|c| c.name.clone())),
            email: item
                .email
                .clone()
                .or_else(|| customer.and_then(|c| c.email.clone())),
            phone_number: item
                .phone_number
                .clone()
                .or_else(|| customer.and_then(|c| c.phone_number.clone())),
            address: item.address.clone(),
            metadata: item.metadata.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            test_mode: item.test_mode,
            split_payments: item.split_payments.clone(),
        }
    }
}

/// Build a `CustomerServiceGetRequest` for a lookup-before-create flow.
/// Only carries identity fields — the connector's `GetConnectorCustomer`
/// implementation picks which one to look up by (Glomopay uses email).
impl ForeignFrom<&CompositeAuthorizeRequest> for CustomerServiceGetRequest {
    fn foreign_from(item: &CompositeAuthorizeRequest) -> Self {
        let customer = item.customer.as_ref();
        Self {
            merchant_customer_id: item
                .merchant_customer_id
                .clone()
                .or_else(|| customer.and_then(|customer_data| customer_data.id.clone())),
            connector_customer_id: item
                .state
                .as_ref()
                .and_then(|state| state.connector_customer_id.clone())
                .or_else(|| {
                    customer.and_then(|customer_data| customer_data.connector_customer_id.clone())
                }),
            email: item
                .email
                .clone()
                .or_else(|| customer.and_then(|customer_data| customer_data.email.clone())),
            phone_number: item
                .phone_number
                .clone()
                .or_else(|| customer.and_then(|customer_data| customer_data.phone_number.clone())),
            connector_feature_data: item.connector_feature_data.clone(),
        }
    }
}

impl
    ForeignFrom<(
        &CompositeAuthorizeRequest,
        Option<&MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
        Option<&MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse>,
        Option<&CustomerServiceCreateResponse>,
        Option<&PaymentServiceCreateOrderResponse>,
        Option<&PaymentMethodAuthenticationServiceAuthenticateResponse>,
        Option<&PaymentMethodAuthenticationServicePostAuthenticateResponse>,
    )> for PaymentServiceAuthorizeRequest
{
    fn foreign_from(
        (
            item,
            access_token_response,
            session_token_response,
            create_customer_response,
            create_order_response,
            authenticate_response,
            post_authenticate_response,
        ): (
            &CompositeAuthorizeRequest,
            Option<&MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
            Option<&MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse>,
            Option<&CustomerServiceCreateResponse>,
            Option<&PaymentServiceCreateOrderResponse>,
            Option<&PaymentMethodAuthenticationServiceAuthenticateResponse>,
            Option<&PaymentMethodAuthenticationServicePostAuthenticateResponse>,
        ),
    ) -> Self {
        let connector_customer_id_from_req = item
            .state
            .as_ref()
            .and_then(|state| state.connector_customer_id.clone());

        let connector_customer_id =
            get_connector_customer_id(connector_customer_id_from_req, create_customer_response);

        let access_token_from_req = item
            .state
            .as_ref()
            .and_then(|state| state.access_token.clone());

        let access_token = get_access_token(access_token_from_req, access_token_response);

        let resolved_state = Some(ConnectorState {
            access_token,
            connector_customer_id,
        });

        // Prefer authentication_data from post-auth, then from authenticate, then from request
        let authentication_data = post_authenticate_response
            .and_then(|r| r.authentication_data.clone())
            .or_else(|| authenticate_response.and_then(|r| r.authentication_data.clone()))
            .or_else(|| item.authentication_data.clone());

        // Prefer connector_feature_data from post-auth, then authenticate, then request
        let connector_feature_data = post_authenticate_response
            .and_then(|r| r.connector_feature_data.clone())
            .or_else(|| authenticate_response.and_then(|r| r.connector_feature_data.clone()))
            .or_else(|| item.connector_feature_data.clone());

        // Prefer connector_order_id from create_order_response, then from request
        let connector_order_id = create_order_response
            .and_then(|r| r.connector_order_id.clone())
            .or_else(|| item.connector_order_id.clone());

        Self {
            merchant_transaction_id: item.merchant_transaction_id.clone(),
            amount: item.amount,
            order_tax_amount: item.order_tax_amount,
            surcharge_amount: None,
            shipping_cost: item.shipping_cost,
            payment_method: item.payment_method.clone(),
            capture_method: item.capture_method,
            customer: item.customer.clone(),
            address: item.address.clone(),
            auth_type: item.auth_type,
            enrolled_for_3ds: item.enrolled_for_3ds,
            authentication_data,
            metadata: item.metadata.clone(),
            connector_feature_data,
            return_url: item.return_url.clone(),
            webhook_url: item.webhook_url.clone(),
            complete_authorize_url: item.complete_authorize_url.clone(),
            session_token: get_session_token(item.session_token.clone(), session_token_response),
            order_category: item.order_category.clone(),
            merchant_order_id: item.merchant_order_id.clone(),
            setup_future_usage: item.setup_future_usage,
            off_session: item.off_session,
            request_incremental_authorization: item.request_incremental_authorization,
            request_extended_authorization: item.request_extended_authorization,
            enable_partial_authorization: item.enable_partial_authorization,
            customer_acceptance: item.customer_acceptance.clone(),
            browser_info: item.browser_info.clone(),
            payment_experience: item.payment_experience,
            description: item.description.clone(),
            payment_channel: item.payment_channel,
            test_mode: item.test_mode,
            setup_mandate_details: item.setup_mandate_details.clone(),
            statement_descriptor_name: item.statement_descriptor_name.clone(),
            statement_descriptor_suffix: item.statement_descriptor_suffix.clone(),
            billing_descriptor: item.billing_descriptor.clone(),
            state: resolved_state,
            order_details: item.order_details.clone(),
            locale: item.locale.clone(),
            tokenization_strategy: item.tokenization_strategy,
            threeds_completion_indicator: item.threeds_completion_indicator,
            redirection_response: item.redirection_response.clone(),
            continue_redirection_url: item.continue_redirection_url.clone(),
            l2_l3_data: item.l2_l3_data.clone(),
            connector_order_id,
            mit_category: item.mit_category,
            merchant_request_id: item.merchant_request_id.clone(),
            domain_data: item.domain_data.clone(),
            split_payments: item.split_payments.clone(),
            partner_merchant_identifier_details: item.partner_merchant_identifier_details.clone(),
            currency_conversion_data: item.currency_conversion_data.clone(),
        }
    }
}

impl ForeignFrom<(&CompositeGetRequest, &ConnectorVariant)>
    for MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest
{
    fn foreign_from((item, connector): (&CompositeGetRequest, &ConnectorVariant)) -> Self {
        Self {
            merchant_access_token_id: item.merchant_access_token_id.clone(),
            connector: grpc_connector_from_connector_variant(connector),
            metadata: item.metadata.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            test_mode: item.test_mode,
            merchant_request_id: item.merchant_request_id.clone(),
        }
    }
}

impl
    ForeignFrom<(
        &CompositeGetRequest,
        Option<&MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
    )> for PaymentServiceGetRequest
{
    fn foreign_from(
        (item, access_token_response): (
            &CompositeGetRequest,
            Option<&MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
        ),
    ) -> Self {
        let access_token_from_req = item
            .state
            .as_ref()
            .and_then(|state| state.access_token.clone());

        let access_token = get_access_token(access_token_from_req, access_token_response);

        let connector_customer_id = item
            .state
            .as_ref()
            .and_then(|state| state.connector_customer_id.clone());

        let resolved_state = Some(ConnectorState {
            access_token,
            connector_customer_id,
        });

        Self {
            connector_transaction_id: item.connector_transaction_id.clone(),
            merchant_transaction_id: item.merchant_transaction_id.clone(),
            encoded_data: item.encoded_data.clone(),
            capture_method: item.capture_method,
            // handle_response: item.handle_response.clone(), // field removed from proto (field 5 reserved)
            amount: item.amount,
            setup_future_usage: item.setup_future_usage,
            state: resolved_state,
            metadata: item.metadata.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            sync_type: item.sync_type,
            connector_order_reference_id: item.connector_order_reference_id.clone(),
            test_mode: item.test_mode,
            payment_experience: item.payment_experience,
            merchant_request_id: item.merchant_request_id.clone(),
            payment_method_type: item.payment_method_type,
            split_payments: item.split_payments.clone(),
            mandate_reference: item.mandate_reference.clone(),
        }
    }
}

impl ForeignFrom<(&CompositeRefundRequest, &ConnectorVariant)>
    for MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest
{
    fn foreign_from((item, connector): (&CompositeRefundRequest, &ConnectorVariant)) -> Self {
        Self {
            merchant_access_token_id: item.merchant_access_token_id.clone(),
            connector: grpc_connector_from_connector_variant(connector),
            metadata: item.metadata.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            test_mode: item.test_mode,
            merchant_request_id: item.merchant_request_id.clone(),
        }
    }
}

impl
    ForeignFrom<(
        &CompositeRefundRequest,
        Option<&MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
    )> for PaymentServiceRefundRequest
{
    fn foreign_from(
        (item, access_token_response): (
            &CompositeRefundRequest,
            Option<&MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
        ),
    ) -> Self {
        let access_token_from_req = item
            .state
            .as_ref()
            .and_then(|state| state.access_token.clone());

        let access_token = get_access_token(access_token_from_req, access_token_response);

        let connector_customer_id = item
            .state
            .as_ref()
            .and_then(|state| state.connector_customer_id.clone());

        let resolved_state = Some(ConnectorState {
            access_token,
            connector_customer_id,
        });

        Self {
            merchant_refund_id: item.merchant_refund_id.clone(),
            connector_transaction_id: item.connector_transaction_id.clone(),
            payment_amount: item.payment_amount,
            refund_amount: item.refund_amount,
            reason: item.reason.clone(),
            webhook_url: item.webhook_url.clone(),
            merchant_account_id: item.merchant_account_id.clone(),
            capture_method: item.capture_method,
            metadata: item.metadata.clone(),
            refund_metadata: item.refund_metadata.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            browser_info: item.browser_info.clone(),
            state: resolved_state,
            test_mode: item.test_mode,
            payment_method_type: item.payment_method_type,
            customer_id: item.customer_id.clone(),
            merchant_request_id: item.merchant_request_id.clone(),
            connector_order_id: item.connector_order_id.clone(),
            payment_method: item.payment_method.clone(),
            split_refunds: item.split_refunds.clone(),
        }
    }
}

impl ForeignFrom<(&CompositeRefundGetRequest, &ConnectorVariant)>
    for MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest
{
    fn foreign_from((item, connector): (&CompositeRefundGetRequest, &ConnectorVariant)) -> Self {
        Self {
            merchant_access_token_id: item.merchant_access_token_id.clone(),
            connector: grpc_connector_from_connector_variant(connector),
            metadata: item.metadata.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            test_mode: item.test_mode,
            merchant_request_id: item.merchant_request_id.clone(),
        }
    }
}

impl
    ForeignFrom<(
        &CompositeRefundGetRequest,
        Option<&MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
    )> for RefundServiceGetRequest
{
    fn foreign_from(
        (item, access_token_response): (
            &CompositeRefundGetRequest,
            Option<&MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
        ),
    ) -> Self {
        let access_token_from_req = item
            .state
            .as_ref()
            .and_then(|state| state.access_token.clone());

        let access_token = get_access_token(access_token_from_req, access_token_response);

        let connector_customer_id = item
            .state
            .as_ref()
            .and_then(|state| state.connector_customer_id.clone());

        let resolved_state = Some(ConnectorState {
            access_token,
            connector_customer_id,
        });

        Self {
            merchant_refund_id: item.merchant_refund_id.clone(),
            connector_transaction_id: item.connector_transaction_id.clone(),
            refund_id: item.refund_id.clone(),
            connector_refund_id: item.connector_refund_id.clone(),
            refund_reason: item.refund_reason.clone(),
            browser_info: item.browser_info.clone(),
            refund_metadata: item.refund_metadata.clone(),
            state: resolved_state,
            test_mode: item.test_mode,
            payment_method_type: item.payment_method_type,
            connector_feature_data: item.connector_feature_data.clone(),
            refund_amount: item.refund_amount,
            merchant_request_id: item.merchant_request_id.clone(),
            connector_order_id: item.connector_order_id.clone(),
            split_refunds: item.split_refunds.clone(),
        }
    }
}

impl ForeignFrom<(&CompositeVoidRequest, &ConnectorVariant)>
    for MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest
{
    fn foreign_from((item, connector): (&CompositeVoidRequest, &ConnectorVariant)) -> Self {
        Self {
            merchant_access_token_id: item.merchant_access_token_id.clone(),
            connector: grpc_connector_from_connector_variant(connector),
            metadata: item.metadata.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            test_mode: item.test_mode,
            merchant_request_id: item.merchant_request_id.clone(),
        }
    }
}

impl
    ForeignFrom<(
        &CompositeVoidRequest,
        Option<&MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
    )> for PaymentServiceVoidRequest
{
    fn foreign_from(
        (item, access_token_response): (
            &CompositeVoidRequest,
            Option<&MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
        ),
    ) -> Self {
        let access_token_from_req = item
            .state
            .as_ref()
            .and_then(|state| state.access_token.clone());

        let access_token = get_access_token(access_token_from_req, access_token_response);

        let connector_customer_id = item
            .state
            .as_ref()
            .and_then(|state| state.connector_customer_id.clone());

        let resolved_state = Some(ConnectorState {
            access_token,
            connector_customer_id,
        });

        Self {
            merchant_void_id: item.merchant_void_id.clone(),
            connector_transaction_id: item.connector_transaction_id.clone(),
            cancellation_reason: item.cancellation_reason.clone(),
            all_keys_required: item.all_keys_required,
            browser_info: item.browser_info.clone(),
            amount: item.amount,
            metadata: item.metadata.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            state: resolved_state,
            test_mode: item.test_mode,
            merchant_order_id: item.merchant_order_id.clone(),
            merchant_request_id: item.merchant_request_id.clone(),
            split_payments: item.split_payments.clone(),
        }
    }
}

// ── AuthN transformers ────────────────────────────────────────────────────────

impl
    ForeignFrom<(
        &CompositeAuthorizeRequest,
        Option<&MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
    )> for PaymentMethodAuthenticationServicePreAuthenticateRequest
{
    fn foreign_from(
        (item, access_token_response): (
            &CompositeAuthorizeRequest,
            Option<&MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
        ),
    ) -> Self {
        // Resolve the access token the same way the Authorize/Capture/Refund
        // sub-requests do: prefer a caller-supplied token, otherwise fall back to
        // the parent flow's freshly-created server-authentication token. OAuth-gated
        // connectors (should_do_access_token) need this both to avoid
        // FAILED_TO_OBTAIN_AUTH_TYPE and because the resolved token is the source
        // of connector-side values derived from it during PreAuthenticate (e.g. the
        // Kount DDC clientID, read from the token's JWT claims).
        let access_token_from_req = item
            .state
            .as_ref()
            .and_then(|state| state.access_token.clone());
        let access_token = get_access_token(access_token_from_req, access_token_response);
        let connector_customer_id = item
            .state
            .as_ref()
            .and_then(|state| state.connector_customer_id.clone());
        let resolved_state = Some(ConnectorState {
            access_token,
            connector_customer_id,
        });
        Self {
            merchant_order_id: item.merchant_transaction_id.clone(),
            amount: item.amount,
            payment_method: item.payment_method.clone(),
            customer: item.customer.clone(),
            address: item.address.clone(),
            enrolled_for_3ds: item.enrolled_for_3ds.unwrap_or(false),
            metadata: item.metadata.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            return_url: item.return_url.clone(),
            continue_redirection_url: item.continue_redirection_url.clone(),
            browser_info: item.browser_info.clone(),
            state: resolved_state,
            capture_method: item.capture_method,
            description: item.description.clone(),
            merchant_transaction_id: item.merchant_transaction_id.clone(),
        }
    }
}

impl
    ForeignFrom<(
        &CompositeAuthorizeRequest,
        Option<&PaymentMethodAuthenticationServicePreAuthenticateResponse>,
    )> for PaymentMethodAuthenticationServiceAuthenticateRequest
{
    fn foreign_from(
        (item, pre_auth_response): (
            &CompositeAuthorizeRequest,
            Option<&PaymentMethodAuthenticationServicePreAuthenticateResponse>,
        ),
    ) -> Self {
        Self {
            merchant_order_id: item.merchant_transaction_id.clone(),
            amount: item.amount,
            payment_method: item.payment_method.clone(),
            customer: item.customer.clone(),
            address: item.address.clone(),
            // Pass authentication_data from pre-auth response if available, else from payload
            authentication_data: pre_auth_response
                .and_then(|r| r.authentication_data.clone())
                .or_else(|| item.authentication_data.clone()),
            metadata: item.metadata.clone(),
            // Carry connector_feature_data from pre-auth response if available, else from payload
            connector_feature_data: pre_auth_response
                .and_then(|r| r.connector_feature_data.clone())
                .or_else(|| item.connector_feature_data.clone()),
            return_url: item.return_url.clone(),
            continue_redirection_url: item.continue_redirection_url.clone(),
            browser_info: item.browser_info.clone(),
            // Thread the caller-supplied ConnectorState (access token) into the
            // sub-request so OAuth-gated connectors don't fail FAILED_TO_OBTAIN_AUTH_TYPE.
            state: item.state.clone(),
            redirection_response: item.redirection_response.clone(),
            capture_method: item.capture_method,
            webhook_url: item.webhook_url.clone(),
            domain_data: item.domain_data.clone(),
        }
    }
}

impl
    ForeignFrom<(
        &CompositeAuthorizeRequest,
        Option<&PaymentMethodAuthenticationServiceAuthenticateResponse>,
    )> for PaymentMethodAuthenticationServicePostAuthenticateRequest
{
    fn foreign_from(
        (item, auth_response): (
            &CompositeAuthorizeRequest,
            Option<&PaymentMethodAuthenticationServiceAuthenticateResponse>,
        ),
    ) -> Self {
        Self {
            merchant_order_id: item.merchant_transaction_id.clone(),
            amount: item.amount,
            payment_method: item.payment_method.clone(),
            customer: item.customer.clone(),
            address: item.address.clone(),
            authentication_data: auth_response
                .and_then(|r| r.authentication_data.clone())
                .or_else(|| item.authentication_data.clone()),
            connector_order_reference_id: auth_response
                .and_then(|r| r.connector_transaction_id.clone()),
            metadata: item.metadata.clone(),
            connector_feature_data: auth_response
                .and_then(|r| r.connector_feature_data.clone())
                .or_else(|| item.connector_feature_data.clone()),
            return_url: item.return_url.clone(),
            continue_redirection_url: item.continue_redirection_url.clone(),
            browser_info: item.browser_info.clone(),
            // Thread the caller-supplied ConnectorState (access token) into the
            // sub-request so OAuth-gated connectors don't fail FAILED_TO_OBTAIN_AUTH_TYPE.
            state: item.state.clone(),
            redirection_response: item.redirection_response.clone(),
            capture_method: item.capture_method,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────

impl ForeignFrom<(&CompositeCaptureRequest, &ConnectorVariant)>
    for MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest
{
    fn foreign_from((item, connector): (&CompositeCaptureRequest, &ConnectorVariant)) -> Self {
        Self {
            merchant_access_token_id: item.merchant_access_token_id.clone(),
            connector: grpc_connector_from_connector_variant(connector),
            metadata: item.metadata.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            test_mode: item.test_mode,
            merchant_request_id: item.merchant_request_id.clone(),
        }
    }
}

impl
    ForeignFrom<(
        &CompositeCaptureRequest,
        Option<&MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
    )> for PaymentServiceCaptureRequest
{
    fn foreign_from(
        (item, access_token_response): (
            &CompositeCaptureRequest,
            Option<&MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
        ),
    ) -> Self {
        let access_token_from_req = item
            .state
            .as_ref()
            .and_then(|state| state.access_token.clone());

        let access_token = get_access_token(access_token_from_req, access_token_response);

        let connector_customer_id = item
            .state
            .as_ref()
            .and_then(|state| state.connector_customer_id.clone());

        let resolved_state = Some(ConnectorState {
            access_token,
            connector_customer_id,
        });

        Self {
            merchant_capture_id: item.merchant_capture_id.clone(),
            connector_transaction_id: item.connector_transaction_id.clone(),
            amount_to_capture: item.amount_to_capture,
            metadata: item.metadata.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            multiple_capture_data: item.multiple_capture_data.clone(),
            browser_info: item.browser_info.clone(),
            capture_method: item.capture_method,
            state: resolved_state,
            test_mode: item.test_mode,
            merchant_order_id: item.merchant_order_id.clone(),
            merchant_request_id: item.merchant_request_id.clone(),
            order_tax_amount: item.order_tax_amount,
            split_payments: item.split_payments.clone(),
        }
    }
}

impl ForeignFrom<(&CompositePaymentMethodRechargeRequest, &ConnectorVariant)>
    for MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest
{
    fn foreign_from(
        (item, connector): (&CompositePaymentMethodRechargeRequest, &ConnectorVariant),
    ) -> Self {
        Self {
            merchant_access_token_id: item.merchant_access_token_id.clone(),
            connector: grpc_connector_from_connector_variant(connector),
            metadata: item.metadata.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            test_mode: item.test_mode,
            merchant_request_id: item.merchant_request_id.clone(),
        }
    }
}

impl ForeignFrom<(&CompositePaymentMethodCreateRequest, &ConnectorVariant)>
    for MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest
{
    fn foreign_from(
        (item, connector): (&CompositePaymentMethodCreateRequest, &ConnectorVariant),
    ) -> Self {
        Self {
            merchant_access_token_id: item.merchant_access_token_id.clone(),
            connector: grpc_connector_from_connector_variant(connector),
            metadata: item.metadata.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            test_mode: item.test_mode,
            merchant_request_id: item.merchant_request_id.clone(),
        }
    }
}

impl ForeignFrom<(&CompositePaymentMethodGetRequest, &ConnectorVariant)>
    for MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest
{
    fn foreign_from(
        (item, connector): (&CompositePaymentMethodGetRequest, &ConnectorVariant),
    ) -> Self {
        Self {
            merchant_access_token_id: item.merchant_access_token_id.clone(),
            connector: grpc_connector_from_connector_variant(connector),
            metadata: item.metadata.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            test_mode: item.test_mode,
            merchant_request_id: item.merchant_request_id.clone(),
        }
    }
}

impl
    ForeignFrom<(
        &CompositePaymentMethodRechargeRequest,
        Option<&MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
    )> for PaymentMethodServiceRechargeRequest
{
    fn foreign_from(
        (item, access_token_response): (
            &CompositePaymentMethodRechargeRequest,
            Option<&MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
        ),
    ) -> Self {
        let access_token_from_req = item
            .state
            .as_ref()
            .and_then(|state| state.access_token.clone());
        let access_token = get_access_token(access_token_from_req, access_token_response);

        let connector_customer_id = item
            .state
            .as_ref()
            .and_then(|state| state.connector_customer_id.clone());

        let resolved_state = Some(ConnectorState {
            access_token,
            connector_customer_id,
        });

        Self {
            merchant_payment_method_id: item.merchant_payment_method_id.clone(),
            connector_payment_method_id: item.connector_payment_method_id.clone(),
            merchant_request_id: item.merchant_request_id.clone(),
            merchant_recharge_id: item.merchant_recharge_id.clone(),
            product_id: item.product_id.clone(),
            amount: item.amount,
            description: item.description.clone(),
            payment_method_type: item.payment_method_type,
            state: resolved_state,
            metadata: item.metadata.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            test_mode: item.test_mode,
        }
    }
}

impl
    ForeignFrom<(
        &CompositePaymentMethodCreateRequest,
        Option<&MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
    )> for PaymentMethodServiceCreateRequest
{
    fn foreign_from(
        (item, access_token_response): (
            &CompositePaymentMethodCreateRequest,
            Option<&MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
        ),
    ) -> Self {
        let access_token_from_req = item
            .state
            .as_ref()
            .and_then(|state| state.access_token.clone());
        let access_token = get_access_token(access_token_from_req, access_token_response);
        let connector_customer_id = item
            .state
            .as_ref()
            .and_then(|state| state.connector_customer_id.clone());
        let resolved_state = Some(ConnectorState {
            access_token,
            connector_customer_id,
        });

        Self {
            merchant_payment_method_id: item.merchant_payment_method_id.clone(),
            customer: item.customer.clone(),
            description: item.description.clone(),
            payment_method_type: item.payment_method_type,
            state: resolved_state,
            product_id: item.product_id.clone(),
            metadata: item.metadata.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            test_mode: item.test_mode,
        }
    }
}

impl ForeignFrom<&CompositePaymentMethodGetRequest> for PaymentMethodServiceTokenizeRequest {
    fn foreign_from(item: &CompositePaymentMethodGetRequest) -> Self {
        Self {
            merchant_payment_method_id: item.merchant_payment_method_id.clone(),
            amount: item.amount,
            payment_method: item.payment_method.clone(),
            customer: item.customer.clone(),
            address: item.address.clone(),
            metadata: item.metadata.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            return_url: item.return_url.clone(),
            test_mode: item.test_mode,
            state: item.state.clone(),
            split_payments: item.split_payments.clone(),
        }
    }
}

impl
    ForeignFrom<(
        &CompositePaymentMethodGetRequest,
        Option<&MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
        Option<&PaymentMethodServiceTokenizeResponse>,
    )> for PaymentMethodServiceGetRequest
{
    fn foreign_from(
        (item, access_token_response, payment_method_tokenize_response): (
            &CompositePaymentMethodGetRequest,
            Option<&MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
            Option<&PaymentMethodServiceTokenizeResponse>,
        ),
    ) -> Self {
        let access_token_from_req = item
            .state
            .as_ref()
            .and_then(|state| state.access_token.clone());
        let access_token = get_access_token(access_token_from_req, access_token_response);
        let connector_customer_id = item
            .state
            .as_ref()
            .and_then(|state| state.connector_customer_id.clone());
        let resolved_state = Some(ConnectorState {
            access_token,
            connector_customer_id,
        });

        let payment_method_token = get_payment_method_token(
            item.payment_method_token.clone(),
            payment_method_tokenize_response,
        );

        Self {
            merchant_payment_method_id: item.merchant_payment_method_id.clone(),
            connector_payment_method_id: item.connector_payment_method_id.clone(),
            customer: item.customer.clone(),
            payment_method_type: item.payment_method_type,
            state: resolved_state,
            connector_feature_data: item.connector_feature_data.clone(),
            metadata: item.metadata.clone(),
            test_mode: item.test_mode,
            payment_method_token,
        }
    }
}

// Transformers for CompositeVerifyRedirectResponse

impl ForeignFrom<(&CompositeVerifyRedirectResponseRequest, &ConnectorVariant)>
    for MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest
{
    fn foreign_from(
        (item, connector): (&CompositeVerifyRedirectResponseRequest, &ConnectorVariant),
    ) -> Self {
        Self {
            merchant_access_token_id: item.merchant_access_token_id.clone(),
            connector: grpc_connector_from_connector_variant(connector),
            metadata: item.metadata.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            test_mode: item.test_mode,
            merchant_request_id: item.merchant_request_id.clone(),
        }
    }
}

impl ForeignFrom<(&CompositeVerifyRedirectResponseRequest, &ConnectorEnum)>
    for MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest
{
    fn foreign_from(
        (item, _connector): (&CompositeVerifyRedirectResponseRequest, &ConnectorEnum),
    ) -> Self {
        let payment_context =
            item.amount
                .map(|amount| grpc_api_types::payments::PaymentSessionContext {
                    amount: Some(amount),
                    metadata: item.metadata.clone(),
                    browser_info: item.browser_info.clone(),
                    customer: item.customer.clone(),
                    address: item.address.clone(),
                });

        Self {
            merchant_server_session_id: item.merchant_server_session_id.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            state: item.state.clone(),
            test_mode: item.test_mode,
            domain_context: payment_context.map(|ctx| {
                grpc_api_types::payments::merchant_authentication_service_create_server_session_authentication_token_request::DomainContext::Payment(ctx)
            }),
        }
    }
}

impl
    ForeignFrom<(
        &CompositeVerifyRedirectResponseRequest,
        &PaymentServiceVerifyRedirectResponseResponse,
        Option<&MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
        Option<&MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse>,
    )> for PaymentServiceAuthorizeRequest
{
    fn foreign_from(
        (request, _verify_response, access_token_response, session_token_response): (
            &CompositeVerifyRedirectResponseRequest,
            &PaymentServiceVerifyRedirectResponseResponse,
            Option<&MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
            Option<&MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse>,
        ),
    ) -> Self {
        // Build access token from response or request state
        let access_token_from_req = request
            .state
            .as_ref()
            .and_then(|state| state.access_token.clone());
        let access_token = get_access_token(access_token_from_req, access_token_response);

        // Build connector customer id from state
        let connector_customer_id = request
            .state
            .as_ref()
            .and_then(|state| state.connector_customer_id.clone());

        let resolved_state = Some(ConnectorState {
            access_token,
            connector_customer_id,
        });

        Self {
            merchant_transaction_id: request.merchant_transaction_id.clone(),
            merchant_order_id: request.merchant_order_id.clone(),
            amount: request.amount,
            order_tax_amount: request.order_tax_amount,
            surcharge_amount: None,
            mit_category: None,
            shipping_cost: request.shipping_cost,
            payment_method: request.payment_method.clone(),
            capture_method: request.capture_method,
            customer: request.customer.clone(),
            address: request.address.clone(),
            auth_type: request.auth_type.unwrap_or_default(),
            enrolled_for_3ds: request.enrolled_for_3ds,
            authentication_data: request.authentication_data.clone(),
            metadata: request.metadata.clone(),
            connector_feature_data: request.connector_feature_data.clone(),
            return_url: request.return_url.clone(),
            webhook_url: request.webhook_url.clone(),
            complete_authorize_url: request.complete_authorize_url.clone(),
            session_token: get_session_token(request.session_token.clone(), session_token_response),
            order_category: request.order_category.clone(),
            setup_future_usage: request.setup_future_usage,
            off_session: request.off_session,
            request_incremental_authorization: request.request_incremental_authorization,
            request_extended_authorization: request.request_extended_authorization,
            enable_partial_authorization: request.enable_partial_authorization,
            customer_acceptance: request.customer_acceptance.clone(),
            browser_info: request.browser_info.clone(),
            payment_experience: request.payment_experience,
            description: request.description.clone(),
            payment_channel: request.payment_channel,
            test_mode: request.test_mode,
            setup_mandate_details: request.setup_mandate_details.clone(),
            statement_descriptor_name: request.statement_descriptor_name.clone(),
            statement_descriptor_suffix: request.statement_descriptor_suffix.clone(),
            billing_descriptor: request.billing_descriptor.clone(),
            state: resolved_state,
            order_details: request.order_details.clone(),
            locale: request.locale.clone(),
            tokenization_strategy: request.tokenization_strategy,
            threeds_completion_indicator: request.threeds_completion_indicator,
            redirection_response: request.redirection_response.clone(),
            continue_redirection_url: request.continue_redirection_url.clone(),
            l2_l3_data: request.l2_l3_data.clone(),
            connector_order_id: request.connector_order_id.clone(),
            merchant_request_id: request.merchant_request_id.clone(),
            domain_data: None,
            split_payments: request.split_payments.clone(),
            partner_merchant_identifier_details: request
                .partner_merchant_identifier_details
                .clone(),
            currency_conversion_data: request.currency_conversion_data.clone(),
        }
    }
}

// ============================================================================
// FRM COMPOSITE REQUESTS
// ============================================================================

impl
    ForeignFrom<(
        &grpc_api_types::frm::CompositeFrmPreRiskCheckRequest,
        &ConnectorVariant,
    )> for MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest
{
    fn foreign_from(
        (item, connector): (
            &grpc_api_types::frm::CompositeFrmPreRiskCheckRequest,
            &ConnectorVariant,
        ),
    ) -> Self {
        Self {
            merchant_access_token_id: item.merchant_access_token_id.clone(),
            connector: grpc_connector_from_connector_variant(connector),
            metadata: item.metadata.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            test_mode: item.test_mode,
            merchant_request_id: item.merchant_request_id.clone(),
        }
    }
}

impl
    ForeignFrom<(
        &grpc_api_types::frm::CompositeFrmPostRiskCheckRequest,
        &ConnectorVariant,
    )> for MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest
{
    fn foreign_from(
        (item, connector): (
            &grpc_api_types::frm::CompositeFrmPostRiskCheckRequest,
            &ConnectorVariant,
        ),
    ) -> Self {
        Self {
            merchant_access_token_id: item.merchant_access_token_id.clone(),
            connector: grpc_connector_from_connector_variant(connector),
            metadata: item.metadata.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            test_mode: item.test_mode,
            merchant_request_id: item.merchant_request_id.clone(),
        }
    }
}

impl
    ForeignFrom<(
        &grpc_api_types::frm::CompositeFrmPreRiskCheckRequest,
        Option<&MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
    )> for grpc_api_types::frm::FrmServicePreRiskCheckRequest
{
    fn foreign_from(
        (item, access_token_response): (
            &grpc_api_types::frm::CompositeFrmPreRiskCheckRequest,
            Option<&MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
        ),
    ) -> Self {
        let access_token = get_access_token(
            item.state
                .as_ref()
                .and_then(|state| state.access_token.clone()),
            access_token_response,
        );
        let connector_customer_id = item
            .state
            .as_ref()
            .and_then(|state| state.connector_customer_id.clone());

        Self {
            amount: item.amount,
            customer_info: item.customer_info.clone(),
            payment_method: item.payment_method.clone(),
            browser_info: item.browser_info.clone(),
            merchant_transaction_id: item.merchant_transaction_id.clone(),
            order_details: item.order_details.clone(),
            address: item.address.clone(),
            metadata: item.metadata.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            test_mode: item.test_mode,
            state: Some(ConnectorState {
                access_token,
                connector_customer_id,
            }),
            merchant_details: item.merchant_details.clone(),
            mandate_details: item.mandate_details.clone(),
        }
    }
}

impl
    ForeignFrom<(
        &CompositePreAuthenticateRequest,
        Option<&MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
    )> for PaymentMethodAuthenticationServicePreAuthenticateRequest
{
    fn foreign_from(
        (item, access_token_response): (
            &CompositePreAuthenticateRequest,
            Option<&MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
        ),
    ) -> Self {
        let access_token = get_access_token(
            item.state
                .as_ref()
                .and_then(|state| state.access_token.clone()),
            access_token_response,
        );
        let connector_customer_id = item
            .state
            .as_ref()
            .and_then(|state| state.connector_customer_id.clone());

        Self {
            merchant_order_id: item.merchant_order_id.clone(),
            amount: item.amount,
            payment_method: item.payment_method.clone(),
            customer: item.customer.clone(),
            address: item.address.clone(),
            enrolled_for_3ds: item.enrolled_for_3ds,
            metadata: item.metadata.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            return_url: item.return_url.clone(),
            continue_redirection_url: item.continue_redirection_url.clone(),
            browser_info: item.browser_info.clone(),
            state: Some(ConnectorState {
                access_token,
                connector_customer_id,
            }),
            capture_method: item.capture_method,
            description: item.description.clone(),
            merchant_transaction_id: item.merchant_transaction_id.clone(),
        }
    }
}

impl
    ForeignFrom<(
        &grpc_api_types::frm::CompositeFrmPostRiskCheckRequest,
        Option<&MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
    )> for grpc_api_types::frm::FrmServicePostRiskCheckRequest
{
    fn foreign_from(
        (item, access_token_response): (
            &grpc_api_types::frm::CompositeFrmPostRiskCheckRequest,
            Option<&MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
        ),
    ) -> Self {
        let access_token = get_access_token(
            item.state
                .as_ref()
                .and_then(|state| state.access_token.clone()),
            access_token_response,
        );
        let connector_customer_id = item
            .state
            .as_ref()
            .and_then(|state| state.connector_customer_id.clone());

        Self {
            amount: item.amount,
            customer_info: item.customer_info.clone(),
            payment_method: item.payment_method.clone(),
            merchant_transaction_id: item.merchant_transaction_id.clone(),
            order_details: item.order_details.clone(),
            metadata: item.metadata.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            test_mode: item.test_mode,
            payment_status: item.payment_status,
            connector_transaction_id: item.connector_transaction_id.clone(),
            payment_connector: item.payment_connector,
            state: Some(ConnectorState {
                access_token,
                connector_customer_id,
            }),
        }
    }
}

impl
    ForeignFrom<(
        &grpc_api_types::payments::CompositeNotifyRequest,
        &ConnectorVariant,
    )> for MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest
{
    fn foreign_from(
        (item, connector): (
            &grpc_api_types::payments::CompositeNotifyRequest,
            &ConnectorVariant,
        ),
    ) -> Self {
        Self {
            merchant_access_token_id: item.merchant_access_token_id.clone(),
            connector: grpc_connector_from_connector_variant(connector),
            metadata: item.metadata.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            test_mode: item.test_mode,
            merchant_request_id: item.merchant_request_id.clone(),
        }
    }
}

impl
    ForeignFrom<(
        &grpc_api_types::payments::CompositeNotifyRequest,
        Option<&MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
    )> for grpc_api_types::payments::NotifyConnectorRequest
{
    fn foreign_from(
        (item, access_token_response): (
            &grpc_api_types::payments::CompositeNotifyRequest,
            Option<&MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
        ),
    ) -> Self {
        let access_token_from_req = item
            .state
            .as_ref()
            .and_then(|state| state.access_token.clone());

        let access_token = get_access_token(access_token_from_req, access_token_response);

        let connector_customer_id = item
            .state
            .as_ref()
            .and_then(|state| state.connector_customer_id.clone());

        let resolved_state = Some(ConnectorState {
            access_token,
            connector_customer_id,
        });

        Self {
            event_id: item.event_id.clone(),
            event_type: item.event_type,
            content: item.content.clone(),
            timestamp: item.timestamp,
            state: resolved_state,
        }
    }
}
