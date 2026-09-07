pub mod transformers;

use std::fmt::Debug;
use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt, crypto::GenerateDigest};

use domain_types::{merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    connector_flow::{ServerAuthenticationToken, Authorize, PSync, Refund},
    router_data_v2::RouterDataV2,

    
    connector_types::*,
    errors,
    payment_method_data::PaymentMethodDataTypes,
    router_data::ConnectorSpecificConfig,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{Maskable, ExposeInterface};
use interfaces::{connector_integration_v2::ConnectorIntegrationV2, 
    api::ConnectorCommon,
    decode::BodyDecoding,
};
use serde::Serialize;

use transformers::*;
use crate::types::ResponseRouterData;
use super::macros;
use common_utils::types::StringMajorUnit;
use base64::Engine;
pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

use domain_types::router_data::ErrorResponse;
use crate::with_error_response_body;

pub(crate) mod headers {
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

macros::create_all_prerequisites!(
    connector_name: Payhere,
    generic_type: T,
    api: [
        (
            flow: ServerAuthenticationToken,
            request_body: PayhereServerAuthenticationTokenRequest,
            response_body: PayhereServerAuthenticationTokenResponse,
            router_data: RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ),
        (
            flow: PSync,
            response_body: PayhereSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: PayhereRefundRequest,
            response_body: PayhereRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: StringMajorUnit
    ],
    member_functions: {
        fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            )];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);
            Ok(header)
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon for Payhere<T> {
    fn id(&self) -> &'static str {
        "payhere"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.payhere.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
        let auth = PayhereAuthType::try_from(auth_type)
            .change_context(errors::IntegrationError::FailedToObtainAuthType { context: Default::default() })?;
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            format!("Bearer {}", auth.access_token.expose()).into(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let response: PayhereErrorResponse = res
            .response
            .parse_struct("PayhereErrorResponse")
            .change_context(errors::ConnectorError::ResponseDeserializationFailed { context: Default::default() })?;

        with_error_response_body!(event_builder, response);
        let typed = macros::serialize_typed_connector_payload(&response, "typed_connector_response");

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.status.to_string(),
            message: response.msg.clone(),
            reason: Some(response.msg),
            attempt_status: None,
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
            typed_connector_response: typed,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        })
    }
}



impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> interfaces::connector_types::ValidationTrait for Payhere<T> {
    fn should_do_access_token(&self, _payment_method: Option<common_enums::PaymentMethod>) -> bool {
        true
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> interfaces::connector_types::IncomingWebhook for Payhere<T> {
    
    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<errors::WebhookError>> {
        let secret = connector_webhook_secret.ok_or(errors::WebhookError::WebhookSourceVerificationFailed)?;
        let merchant_secret = std::str::from_utf8(&secret.secret)
            .map_err(|_| error_stack::report!(errors::WebhookError::WebhookSourceVerificationFailed))?;
        let payload: PayhereWebhookPayload = serde_urlencoded::from_bytes::<PayhereWebhookPayload>(&request.body)
            .map_err(|_| error_stack::report!(errors::WebhookError::WebhookSourceVerificationFailed))?;
        
        let hash_secret = common_utils::crypto::Md5.generate_digest(merchant_secret.as_bytes())
            .map_err(|_| error_stack::report!(errors::WebhookError::WebhookSourceVerificationFailed))?;
        let hash_secret_upper = hex::encode(hash_secret).to_uppercase();
        
        let message = format!("{}{}{}{}{}{}", payload.merchant_id, payload.order_id, payload.payhere_amount, payload.payhere_currency, payload.status_code, hash_secret_upper);
        
        let final_hash = common_utils::crypto::Md5.generate_digest(message.as_bytes())
            .map_err(|_| error_stack::report!(errors::WebhookError::WebhookSourceVerificationFailed))?;
        
        Ok(hex::encode(final_hash).to_uppercase() == payload.md5sig)
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<errors::WebhookError>> {
        let payload: PayhereWebhookPayload = serde_urlencoded::from_bytes::<PayhereWebhookPayload>(&request.body)
            .map_err(|_| error_stack::report!(errors::WebhookError::WebhookEventTypeNotFound))?;
        
        match payload.status_code {
            2 => Ok(EventType::PaymentIntentSuccess),
            0 => Ok(EventType::PaymentIntentProcessing),
            -1 | -2 => Ok(EventType::PaymentIntentFailure),
            _ => Ok(EventType::IncomingWebhookEventUnspecified),
        }
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<errors::WebhookError>> {
        let payload: PayhereWebhookPayload = serde_urlencoded::from_bytes::<PayhereWebhookPayload>(&request.body)
            .map_err(|_| error_stack::report!(errors::WebhookError::WebhookEventTypeNotFound))?;
        
        Ok(WebhookDetailsResponse {
            resource_id: payload.payment_id.map(|id| ResponseId::ConnectorTransactionId(id.to_string())),
            status: get_status_from_code(payload.status_code),
            connector_response_reference_id: None,
            connector_request_reference_id: Some(payload.order_id),
            mandate_reference: None,
            error_code: None,
            error_message: None,
            error_reason: None,
            raw_connector_response: None,
            status_code: 200,
            response_headers: None,
            amount_captured: None,
            minor_amount_captured: None,
            network_txn_id: None,
            payment_method_update: None,
            sender_payment_instrument_id: None,
            connector_returned_payment_method_details: None,
        })
    }
}


macros::macro_connector_payout_implementation!(
    connector: Payhere,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Payhere,
    curl_request: FormUrlEncoded(PayhereServerAuthenticationTokenRequest),
    curl_response: PayhereServerAuthenticationTokenResponse,
    flow_name: ServerAuthenticationToken,
    resource_common_data: MerchantAuthenticationFlowData,
    flow_request: ServerAuthenticationTokenRequestData,
    flow_response: ServerAuthenticationTokenResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            let auth = PayhereAuthType::try_from(&req.connector_config)
                .change_context(errors::IntegrationError::FailedToObtainAuthType { context: Default::default() })?;
            let basic_auth = format!("{}:{}", auth.app_id.expose(), auth.app_secret.expose());
            let encoded = BASE64_ENGINE.encode(basic_auth.as_bytes());
            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    "application/x-www-form-urlencoded".to_string().into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    format!("Basic {}", encoded).into(),
                ),
            ])
        }
        fn get_url(
            &self,
            req: &RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            Ok(format!("{}/merchant/v1/oauth/token", self.base_url(&req.resource_common_data.connectors)))
        }
    }
);

macros::macro_connector_local_flow_implementation!(
    connector: Payhere,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    handle_response: crate::connectors::payhere::transformers::handle_authorize_response,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
);
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Payhere,
    curl_response: PayhereSyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            Ok(format!(
                "{}/merchant/v1/payment/search?order_id={}",
                self.base_url(&req.resource_common_data.connectors),
                req.resource_common_data.connector_request_reference_id.clone()
            ))
        }
        fn get_headers(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            let mut headers = self.build_headers(req)?;
            let access_token = req.resource_common_data.get_access_token().map_err(|err| {
                errors::IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext {
                        additional_context: Some(err.to_string()),
                        suggested_action: None,
                        doc_url: None,
                    },
                }
            })?;
            headers.push((
                headers::AUTHORIZATION.to_string(),
                format!("Bearer {}", access_token).into(),
            ));
            Ok(headers)
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Payhere,
    curl_request: Json(PayhereRefundRequest),
    curl_response: PayhereRefundResponse,
    flow_name: Refund,
    resource_common_data: RefundFlowData,
    flow_request: RefundsData,
    flow_response: RefundsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            Ok(format!("{}/merchant/v1/payment/refund", self.base_url(&req.resource_common_data.connectors)))
        }
        fn get_headers(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            let mut headers = self.build_headers(req)?;
            let access_token = req.resource_common_data.get_access_token().map_err(|err| {
                errors::IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext {
                        additional_context: Some(err.to_string()),
                        suggested_action: None,
                        doc_url: None,
                    },
                }
            })?;
            headers.push((
                headers::AUTHORIZATION.to_string(),
                format!("Bearer {}", access_token).into(),
            ));
            Ok(headers)
        }
    }
);

macros::macro_connector_flow_status_impls!(
    connector: Payhere,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        Accept,
        ClientAuthenticationToken,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        DefendDispute,
        MandateRevoke,
        Authenticate,
        Capture,
        IncrementalAuthorization,
        CreateOrder,
        VoidPostRefund,
        PaymentMethodEligibility,
        PostAuthenticate,
        PreAuthenticate,
        PaymentMethodToken,
        VoidPC,
        RSync,
        Void,
                RepeatPayment,
        ServerSessionAuthenticationToken,
        SetupMandate,
        SubmitEvidence
    ],
);



impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding for Payhere<T> {}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> interfaces::connector_types::ConnectorServiceTrait<T> for Payhere<T> {}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> interfaces::connector_types::VerifyRedirectResponse for Payhere<T> {}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> interfaces::verification::SourceVerification for Payhere<T> {}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> interfaces::connector_types::PaymentAuthorizeV2<T> for Payhere<T> {}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> interfaces::connector_types::PaymentSyncV2 for Payhere<T> {}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> interfaces::connector_types::RefundV2 for Payhere<T> {}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> interfaces::connector_types::ServerAuthentication for Payhere<T> {}

