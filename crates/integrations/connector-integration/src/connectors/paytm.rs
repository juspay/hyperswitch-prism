pub mod request;
pub mod response;
pub mod transformers;

use std::fmt::Debug;

use common_enums::AttemptStatus;
use common_utils::{errors::CustomResult, events, ext_traits::BytesExt, types::StringMajorUnit};
use domain_types::{
    connector_flow::{Authorize, PSync, ServerSessionAuthenticationToken},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData,
        ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData,
    },
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use hyperswitch_masking::{Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use paytm::constants;
use serde::Serialize;
use transformers as paytm;

use self::{
    request::{PaytmAuthorizeRequest, PaytmInitiateTxnRequest, PaytmTransactionStatusRequest},
    response::{PaytmInitiateTxnResponse, PaytmProcessTxnResponse, PaytmTransactionStatusResponse},
};
use crate::{connectors::macros, types::ResponseRouterData};
use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;

// Define connector prerequisites using macros - following the exact pattern from other connectors
macros::create_all_prerequisites!(
    connector_name: Paytm,
    generic_type: T,
    api: [
        (
            flow: ServerSessionAuthenticationToken,
            request_body: PaytmInitiateTxnRequest,
            response_body: PaytmInitiateTxnResponse,
            router_data: RouterDataV2<ServerSessionAuthenticationToken, PaymentFlowData, ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData>,
        ),
        (
            flow: Authorize,
            request_body: PaytmAuthorizeRequest,
            response_body: PaytmProcessTxnResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: PaytmTransactionStatusRequest,
            response_body: PaytmTransactionStatusResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        )
    ],
    amount_converters: [amount_converter: StringMajorUnit],
    member_functions: {
        pub fn connector_base_url<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> String {
            req.resource_common_data.connectors.paytm.base_url.to_string()
        }


        fn get_attempt_status_from_http_code(status_code: u16) -> AttemptStatus {
            match status_code {
                500..=599 => AttemptStatus::Pending, // 5xx errors should be pending for retry
                _ => AttemptStatus::Failure,          // All other errors are final failures
            }
        }

        fn build_custom_error_response(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            // First try to parse as session token error response format
            if let Ok(session_error_response) = res
                .response
                .parse_struct::<paytm::PaytmSessionTokenErrorResponse>("PaytmSessionTokenErrorResponse")
            {
                if let Some(event) = event_builder {
                    event.set_connector_response(&session_error_response);
                }

                return Ok(ErrorResponse {
                    code: session_error_response.body.result_info.result_code,
                    message: session_error_response.body.result_info.result_msg,
                    reason: None,
                    status_code: res.status_code,
                    attempt_status: Some(Self::get_attempt_status_from_http_code(res.status_code)),
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None
});
            }

            // Try to parse as callback error response format
            if let Ok(callback_response) = res
                .response
                .parse_struct::<paytm::PaytmCallbackErrorResponse>("PaytmCallbackErrorResponse")
            {
                if let Some(event) = event_builder {
                    event.set_connector_response(&callback_response);
                }

                return Ok(ErrorResponse {
                    code: callback_response
                        .body
                        .txn_info
                        .resp_code
                        .unwrap_or(callback_response.body.result_info.result_code),
                    message: callback_response
                        .body
                        .txn_info
                        .resp_msg
                        .unwrap_or(callback_response.body.result_info.result_msg),
                    reason: None,
                    status_code: res.status_code,
                    attempt_status: Some(Self::get_attempt_status_from_http_code(res.status_code)),
                    connector_transaction_id: callback_response.body.txn_info.order_id,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None
});
            }

            // Try to parse as original JSON error response format
            if let Ok(response) = res
                .response
                .parse_struct::<paytm::PaytmErrorResponse>("PaytmErrorResponse")
            {
                if let Some(event) = event_builder {
                    event.set_connector_response(&response);
                }

                return Ok(ErrorResponse {
                    code: response.error_code.unwrap_or_default(),
                    message: response.error_message.unwrap_or_default(),
                    reason: response.error_description,
                    status_code: res.status_code,
                    attempt_status: Some(Self::get_attempt_status_from_http_code(res.status_code)),
                    connector_transaction_id: response.transaction_id,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None
});
            }

            // Final fallback for non-JSON responses (HTML errors, etc.)
            let raw_response = String::from_utf8_lossy(&res.response);
            let error_message = match res.status_code {
                503 => "Service temporarily unavailable".to_string(),
                502 => "Bad gateway".to_string(),
                500 => "Internal server error".to_string(),
                404 => "Not found".to_string(),
                400 => "Bad request".to_string(),
                _ => format!("HTTP {} error", res.status_code)
};

            Ok(ErrorResponse {
                code: res.status_code.to_string(),
                message: error_message,
                reason: Some(format!(
                    "Raw response: {}",
                    raw_response.chars().take(200).collect::<String>()
                )),
                status_code: res.status_code,
                attempt_status: Some(Self::get_attempt_status_from_http_code(res.status_code)),
                connector_transaction_id: None,
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None
})
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Paytm<T>
{
    fn should_do_session_token(&self) -> bool {
        true // Enable ServerSessionAuthenticationToken flow for Paytm's initiate step
    }

    fn should_do_order_create(&self) -> bool {
        false // Paytm doesn't require separate order creation
    }
}

// Service trait implementations with generic type parameters
macros::macro_connector_payout_implementation!(
    connector: Paytm,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Paytm<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerSessionAuthentication for Paytm<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Paytm<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Paytm<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Paytm<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Paytm<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Paytm<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Paytm<T>
{
}
// Authentication trait implementations
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Paytm<T>
{
    fn id(&self) -> &'static str {
        "paytm"
    }

    fn get_currency_unit(&self) -> common_enums::CurrencyUnit {
        common_enums::CurrencyUnit::Minor
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.paytm.base_url
    }

    fn get_auth_header(
        &self,
        _auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        Ok(vec![(
            constants::CONTENT_TYPE_HEADER.to_string(),
            constants::CONTENT_TYPE_JSON.into(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_custom_error_response(res, event_builder)
    }
}

// ServerSessionAuthenticationToken flow implementation using macros
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Paytm,
    curl_request: Json(PaytmInitiateTxnRequest),
    curl_response: PaytmInitiateTxnResponse,
    flow_name: ServerSessionAuthenticationToken,
    resource_common_data: PaymentFlowData,
    flow_request: ServerSessionAuthenticationTokenRequestData,
    flow_response: ServerSessionAuthenticationTokenResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<ServerSessionAuthenticationToken, PaymentFlowData, ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let headers = self.get_auth_header(&req.connector_config)?;
            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<ServerSessionAuthenticationToken, PaymentFlowData, ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url(req);
            let auth = paytm::PaytmAuthType::try_from(&req.connector_config)?;
            let merchant_id = auth.merchant_id.peek();
            let order_id = &req.resource_common_data.connector_request_reference_id;

            Ok(format!(
                "{base_url}theia/api/v1/initiateTransaction?mid={merchant_id}&orderId={order_id}"
            ))
        }

        fn get_5xx_error_response(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
            self.build_custom_error_response(res, event_builder)
        }
    }
);

// ServerAuthenticationToken implementation

// CreateConnectorCustomer implementation

// Authorize flow implementation using macros
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Paytm,
    curl_request: Json(PaytmAuthorizeRequest),
    curl_response: PaytmProcessTxnResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {

            let headers = self.get_auth_header(&req.connector_config)?;
            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url(req);
            let auth = paytm::PaytmAuthType::try_from(&req.connector_config)?;
            let merchant_id = auth.merchant_id.peek();
            let order_id = &req.resource_common_data.connector_request_reference_id;

            Ok(format!(
                "{base_url}theia/api/v1/processTransaction?mid={merchant_id}&orderId={order_id}"
            ))
        }

        fn get_5xx_error_response(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
            self.build_custom_error_response(res, event_builder)
        }
    }
);

// PSync flow implementation using macros
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Paytm,
    curl_request: Json(PaytmTransactionStatusRequest),
    curl_response: PaytmTransactionStatusResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let headers = self.get_auth_header(&req.connector_config)?;
            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url(req);
            Ok(format!("{base_url}/v3/order/status"))
        }

        fn get_5xx_error_response(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
            self.build_custom_error_response(res, event_builder)
        }
    }
);

macros::macro_connector_flow_status_impls!(
    connector: Paytm,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        VoidPC,
        CreateConnectorCustomer,
        Capture,
        Void,
        Refund,
        RSync,
        SetupMandate,
        Accept,
        SubmitEvidence,
        DefendDispute,
        RepeatPayment,
        PaymentMethodToken,
        MandateRevoke,
    ],
    not_supported: [
        IncrementalAuthorization,
        ServerAuthenticationToken,
        CreateOrder,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        ClientAuthenticationToken,
    ],
);
