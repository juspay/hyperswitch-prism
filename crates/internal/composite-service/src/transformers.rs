use domain_types::connector_types::ConnectorEnum;
use grpc_api_types::payments::{
    CompositeAuthorizeRequest, CompositeCaptureRequest, CompositeGetRequest,
    CompositeRefundGetRequest, CompositeRefundRequest, CompositeVoidRequest, ConnectorState,
    CustomerServiceCreateRequest, CustomerServiceCreateResponse,
    MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest,
    MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse,
    PaymentServiceAuthorizeRequest, PaymentServiceCaptureRequest, PaymentServiceGetRequest,
    PaymentServiceRefundRequest, PaymentServiceVoidRequest, RefundServiceGetRequest,
};

use crate::utils::{
    get_access_token, get_connector_customer_id, grpc_connector_from_connector_enum,
};

pub trait ForeignFrom<F>: Sized {
    fn foreign_from(item: F) -> Self;
}

impl ForeignFrom<(&CompositeAuthorizeRequest, &ConnectorEnum)>
    for MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest
{
    fn foreign_from((item, connector): (&CompositeAuthorizeRequest, &ConnectorEnum)) -> Self {
        Self {
            merchant_access_token_id: item.merchant_access_token_id.clone(),
            connector: grpc_connector_from_connector_enum(connector),
            metadata: item.metadata.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            test_mode: item.test_mode,
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
        }
    }
}

impl
    ForeignFrom<(
        &CompositeAuthorizeRequest,
        Option<&MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
        Option<&CustomerServiceCreateResponse>,
    )> for PaymentServiceAuthorizeRequest
{
    fn foreign_from(
        (item, access_token_response, create_customer_response): (
            &CompositeAuthorizeRequest,
            Option<&MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
            Option<&CustomerServiceCreateResponse>,
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

        Self {
            merchant_transaction_id: item.merchant_transaction_id.clone(),
            amount: item.amount,
            order_tax_amount: item.order_tax_amount,
            shipping_cost: item.shipping_cost,
            payment_method: item.payment_method.clone(),
            capture_method: item.capture_method,
            customer: item.customer.clone(),
            address: item.address.clone(),
            auth_type: item.auth_type,
            enrolled_for_3ds: item.enrolled_for_3ds,
            authentication_data: item.authentication_data.clone(),
            metadata: item.metadata.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            return_url: item.return_url.clone(),
            webhook_url: item.webhook_url.clone(),
            complete_authorize_url: item.complete_authorize_url.clone(),
            session_token: item.session_token.clone(),
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
            connector_order_id: item.connector_order_id.clone(),
        }
    }
}

impl ForeignFrom<(&CompositeGetRequest, &ConnectorEnum)>
    for MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest
{
    fn foreign_from((item, connector): (&CompositeGetRequest, &ConnectorEnum)) -> Self {
        Self {
            merchant_access_token_id: item.merchant_access_token_id.clone(),
            connector: grpc_connector_from_connector_enum(connector),
            metadata: item.metadata.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            test_mode: item.test_mode,
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
            handle_response: item.handle_response.clone(),
            amount: item.amount,
            setup_future_usage: item.setup_future_usage,
            state: resolved_state,
            metadata: item.metadata.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            sync_type: item.sync_type,
            connector_order_reference_id: item.connector_order_reference_id.clone(),
            test_mode: item.test_mode,
            payment_experience: item.payment_experience,
        }
    }
}

impl ForeignFrom<(&CompositeRefundRequest, &ConnectorEnum)>
    for MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest
{
    fn foreign_from((item, connector): (&CompositeRefundRequest, &ConnectorEnum)) -> Self {
        Self {
            merchant_access_token_id: item.merchant_access_token_id.clone(),
            connector: grpc_connector_from_connector_enum(connector),
            metadata: item.metadata.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            test_mode: item.test_mode,
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
        }
    }
}

impl ForeignFrom<(&CompositeRefundGetRequest, &ConnectorEnum)>
    for MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest
{
    fn foreign_from((item, connector): (&CompositeRefundGetRequest, &ConnectorEnum)) -> Self {
        Self {
            merchant_access_token_id: item.merchant_access_token_id.clone(),
            connector: grpc_connector_from_connector_enum(connector),
            metadata: item.metadata.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            test_mode: item.test_mode,
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
            refund_reason: item.refund_reason.clone(),
            browser_info: item.browser_info.clone(),
            refund_metadata: item.refund_metadata.clone(),
            state: resolved_state,
            test_mode: item.test_mode,
            payment_method_type: item.payment_method_type,
            connector_feature_data: item.connector_feature_data.clone(),
        }
    }
}

impl ForeignFrom<(&CompositeVoidRequest, &ConnectorEnum)>
    for MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest
{
    fn foreign_from((item, connector): (&CompositeVoidRequest, &ConnectorEnum)) -> Self {
        Self {
            merchant_access_token_id: item.merchant_access_token_id.clone(),
            connector: grpc_connector_from_connector_enum(connector),
            metadata: item.metadata.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            test_mode: item.test_mode,
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
        }
    }
}

impl ForeignFrom<(&CompositeCaptureRequest, &ConnectorEnum)>
    for MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest
{
    fn foreign_from((item, connector): (&CompositeCaptureRequest, &ConnectorEnum)) -> Self {
        Self {
            merchant_access_token_id: item.merchant_access_token_id.clone(),
            connector: grpc_connector_from_connector_enum(connector),
            metadata: item.metadata.clone(),
            connector_feature_data: item.connector_feature_data.clone(),
            test_mode: item.test_mode,
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
        }
    }
}
