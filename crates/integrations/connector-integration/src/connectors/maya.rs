pub mod transformers;

use std::fmt::Debug;

use base64::Engine;
use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt, types::FloatMajorUnit};
use domain_types::{
    connector_flow::{self, Authorize, PSync, RSync, Refund, Void},
    connector_types::*,
    errors,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_request_types::VerifyWebhookSourceRequestData,
    router_response_types::{Response, VerifyWebhookSourceResponseData},
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Maskable};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding,
};
use serde::Serialize;
use transformers::{
    self as maya, MayaPaymentsRequest, MayaPaymentsResponse, MayaRefundRequest, MayaRefundResponse,
    MayaRefundSyncResponse, MayaVoidRequest, MayaVoidResponse, MayaWebhookBody,
};

use super::macros;
use crate::types::ResponseRouterData;
use crate::with_error_response_body;

pub(crate) mod headers {
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

macros::create_all_prerequisites!(
    connector_name: Maya,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: MayaPaymentsRequest,
            response_body: MayaPaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: MayaWebhookBody,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: MayaVoidRequest,
            response_body: MayaVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: MayaRefundRequest,
            response_body: MayaRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: MayaRefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: FloatMajorUnit
    ],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            )];
            let mut auth_header = self.get_auth_header(&req.connector_config)?;
            header.append(&mut auth_header);
            Ok(header)
        }

        pub fn get_secret_auth_header(
            &self,
            auth_type: &ConnectorSpecificConfig,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            let auth = maya::MayaAuthType::try_from(auth_type).change_context(
                errors::IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext {
                        additional_context: Some(
                            "Maya requires public_key and secret_key authentication".to_string(),
                        ),
                        ..Default::default()
                    },
                },
            )?;

            let credentials = format!("{}:", auth.secret_key.expose());
            let encoded = BASE64_ENGINE.encode(credentials);

            Ok(vec![(
                headers::AUTHORIZATION.to_string(),
                format!("Basic {encoded}").into(),
            )])
        }
    }
);

// =============================================================================
// CONNECTOR COMMON IMPLEMENTATION
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Maya<T>
{
    fn id(&self) -> &'static str {
        "maya"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.maya.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
        let auth = maya::MayaAuthType::try_from(auth_type).change_context(
            errors::IntegrationError::FailedToObtainAuthType {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Maya requires public_key and secret_key authentication".to_string(),
                    ),
                    ..Default::default()
                },
            },
        )?;

        let credentials = format!("{}:", auth.public_key.expose());
        let encoded = BASE64_ENGINE.encode(credentials);

        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            format!("Basic {encoded}").into(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let response: maya::MayaErrorResponse = res
            .response
            .parse_struct("MayaErrorResponse")
            .change_context(crate::utils::response_deserialization_fail(
                res.status_code,
                "maya: response body did not match the expected error format",
            ))?;

        with_error_response_body!(event_builder, response);

        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");

        let mut reason_parts = Vec::new();

        if let Some(params) = response.parameters {
            reason_parts.push(
                params
                    .iter()
                    .map(|param| format!("{}: {}", param.field, param.description))
                    .collect::<Vec<_>>()
                    .join("; "),
            );
        }

        if let Some(reference) = response.reference {
            reason_parts.push(format!("reference: {reference}"));
        }

        let reason = if reason_parts.is_empty() {
            None
        } else {
            Some(reason_parts.join("; "))
        };

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.code,
            message: response.message,
            reason,
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

// =============================================================================
// BODY DECODING IMPLEMENTATION
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Maya<T>
{
}

// =============================================================================
// DYNAMICALLY GENERATED IMPLEMENTATIONS
// =============================================================================
// The following implementations were auto-generated by add_connector.sh
// based on the flows detected in ConnectorServiceTrait.
//
// To customize a flow implementation:
// 1. Move the empty impl block above (before this comment section)
// 2. Add your custom logic inside the impl block
// 3. The script will not regenerate moved implementations
// =============================================================================

// ===== CONNECTOR SERVICE TRAIT IMPLEMENTATIONS =====
// Main service trait - aggregates all other traits
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Maya<T>
{
}

// ===== FLOW TRAIT IMPLEMENTATIONS =====

crate::connectors::macros::macro_connector_payout_implementation!(
    connector: Maya,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Maya<T>
{
    fn verify_webhook_source(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<errors::WebhookError>> {
        // Maya webhooks are unsigned (no HMAC signature / encryption per Maya docs), so the
        // source cannot be cryptographically verified. Returning false makes Euler fall back
        // to a mandatory PSync (EC_TXN_SYNC) to fetch authoritative status from the gateway.
        // Source-IP whitelisting is enforced at the infrastructure level:
        //   Sandbox:    13.229.160.234, 3.1.199.75
        //   Production: 18.138.50.235, 3.1.207.200
        Ok(false)
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<errors::WebhookError>> {
        let body: MayaWebhookBody = request
            .body
            .parse_struct("MayaWebhookBody")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        Ok(EventType::from(body.status))
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<errors::WebhookError>> {
        let body: MayaWebhookBody = request
            .body
            .parse_struct("MayaWebhookBody")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        let status = common_enums::AttemptStatus::from(body.status.clone());

        let connector_request_reference_id = body.request_reference_number.clone();

        Ok(WebhookDetailsResponse {
            resource_id: Some(ResponseId::ConnectorTransactionId(body.id)),
            status,
            connector_response_reference_id: None,
            connector_request_reference_id,
            mandate_reference: None,
            error_code: body.error_code,
            error_message: body.error_message.clone(),
            error_reason: body.error_message,
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
            amount_captured: None,
            minor_amount_captured: None,
            network_txn_id: None,
            payment_method_update: None,
            sender_payment_instrument_id: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Maya<T>
{
}

// ===== AUTHORIZE CONNECTOR INTEGRATION =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_headers, get_content_type, get_error_response_v2],
    connector: Maya,
    curl_request: Json(MayaPaymentsRequest),
    curl_response: MayaPaymentsResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        ) -> CustomResult<String, errors::IntegrationError> {
            Ok(format!(
                "{}/payby/v2/paymaya/payments",
                self.base_url(&req.resource_common_data.connectors)
            ))
        }
    }
);

// ===== PAYMENT SYNC CONNECTOR INTEGRATION =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Maya,
    curl_response: MayaWebhookBody,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<
                PSync,
                PaymentFlowData,
                PaymentsSyncData,
                PaymentsResponseData,
            >,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            // The payment-by-id endpoint is authenticated with the SECRET key.
            self.get_secret_auth_header(&req.connector_config)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<
                PSync,
                PaymentFlowData,
                PaymentsSyncData,
                PaymentsResponseData,
            >,
        ) -> CustomResult<String, errors::IntegrationError> {
            // Retrieve the payment using the Maya `paymentId` returned during Authorize.
            let payment_id = req
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(errors::IntegrationError::MissingRequiredField {
                    field_name: "connector_transaction_id",
                    context: errors::IntegrationErrorContext {
                        additional_context: Some(
                            "Maya PSync requires the paymentId returned by the Authorize flow"
                                .to_string(),
                        ),
                        suggested_action: Some(
                            "Pass the Maya paymentId as connector_transaction_id when calling PSync"
                                .to_string(),
                        ),
                        doc_url: None,
                    },
                })?;
            Ok(format!(
                "{}/payments/v1/payments/{payment_id}",
                self.base_url(&req.resource_common_data.connectors)
            ))
        }
    }
);

// ===== VOID CONNECTOR INTEGRATION =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Maya,
    curl_request: Json(MayaVoidRequest),
    curl_response: MayaVoidResponse,
    flow_name: Void,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentVoidData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<
                Void,
                PaymentFlowData,
                PaymentVoidData,
                PaymentsResponseData,
            >,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            )];
            let mut auth_header = self.get_secret_auth_header(&req.connector_config)?;
            header.append(&mut auth_header);
            Ok(header)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<
                Void,
                PaymentFlowData,
                PaymentVoidData,
                PaymentsResponseData,
            >,
        ) -> CustomResult<String, errors::IntegrationError> {
            let payment_id = req.request.connector_transaction_id.clone();
            Ok(format!(
                "{}/payments/v1/payments/{payment_id}/voids",
                self.base_url(&req.resource_common_data.connectors)
            ))
        }
    }
);

// ===== REFUND CONNECTOR INTEGRATION =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Maya,
    curl_request: Json(MayaRefundRequest),
    curl_response: MayaRefundResponse,
    flow_name: Refund,
    resource_common_data: RefundFlowData,
    flow_request: RefundsData,
    flow_response: RefundsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<
                Refund,
                RefundFlowData,
                RefundsData,
                RefundsResponseData,
            >,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            )];
            let mut auth_header = self.get_secret_auth_header(&req.connector_config)?;
            header.append(&mut auth_header);
            Ok(header)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<
                Refund,
                RefundFlowData,
                RefundsData,
                RefundsResponseData,
            >,
        ) -> CustomResult<String, errors::IntegrationError> {
            let payment_id = req.request.connector_transaction_id.clone();
            Ok(format!(
                "{}/payments/v1/payments/{payment_id}/refunds",
                self.base_url(&req.resource_common_data.connectors)
            ))
        }
    }
);

// ===== REFUND SYNC CONNECTOR INTEGRATION =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Maya,
    curl_response: MayaRefundSyncResponse,
    flow_name: RSync,
    resource_common_data: RefundFlowData,
    flow_request: RefundSyncData,
    flow_response: RefundsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<
                RSync,
                RefundFlowData,
                RefundSyncData,
                RefundsResponseData,
            >,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.get_secret_auth_header(&req.connector_config)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<
                RSync,
                RefundFlowData,
                RefundSyncData,
                RefundsResponseData,
            >,
        ) -> CustomResult<String, errors::IntegrationError> {
            let payment_id = req.request.connector_transaction_id.clone();
            let refund_id = req.request.connector_refund_id.clone();
            Ok(format!(
                "{}/payments/v1/payments/{payment_id}/refunds/{refund_id}",
                self.base_url(&req.resource_common_data.connectors)
            ))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Maya<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Maya<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Maya<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Maya<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Maya<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Maya<T>
{
    fn decode_redirect_response_body(
        &self,
        request: &RequestDetails,
        _secrets: Option<interfaces::verification::ConnectorSourceVerificationSecrets>,
    ) -> CustomResult<Vec<u8>, errors::IntegrationError> {
        // Maya does not encode/sign redirect bodies; pass through unchanged.
        Ok(request.body.clone())
    }

    fn verify_redirect_response_source(
        &self,
        _request: &RequestDetails,
        _secrets: Option<interfaces::verification::ConnectorSourceVerificationSecrets>,
    ) -> CustomResult<bool, errors::IntegrationError> {
        // Maya does not sign redirect responses; source verification is
        // performed via webhook signature validation instead.
        Ok(false)
    }

    fn process_redirect_response(
        &self,
        _request: &RequestDetails,
    ) -> CustomResult<RedirectDetailsResponse, errors::IntegrationError> {
        // Maya is a redirect-only connector. The redirect body carries no
        // meaningful payment state; final status is confirmed via PSync or
        // webhook. Return an empty success so the redirect flow completes
        // and the caller proceeds to PSync.
        Ok(RedirectDetailsResponse {
            resource_id: None,
            status: None,
            connector_response_reference_id: None,
            error_code: None,
            error_message: None,
            error_reason: None,
            response_amount: None,
            raw_connector_response: None,
        })
    }
}

// ===== CONNECTOR INTEGRATION V2 IMPLEMENTATIONS =====

// Stub implementations for service-trait bounds that are not exercised by Maya.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyWebhookSourceV2 for Maya<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundVoidPostRefundV2 for Maya<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::GetPaymentMethodV2 for Maya<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::CreatePaymentMethodV2 for Maya<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefreshPaymentMethodV2<T> for Maya<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RechargeV2 for Maya<T>
{
}

macro_rules! impl_unsupported_connector_flow {
    ($flow:ty, $rcd:ty, $req:ty, $resp:ty) => {
        impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
            ConnectorIntegrationV2<$flow, $rcd, $req, $resp> for Maya<T>
        {
            fn get_url(
                &self,
                _req: &RouterDataV2<$flow, $rcd, $req, $resp>,
            ) -> CustomResult<String, errors::IntegrationError> {
                Err(errors::IntegrationError::connector_flow_not_implemented(
                    self.id(),
                    std::any::type_name::<$flow>(),
                    errors::IntegrationErrorContext::default(),
                )
                .into())
            }
        }
    };
}

macros::macro_connector_flow_status_impls!(
    connector: Maya,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        CreateOrder,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        SetupMandate,
        PaymentMethodToken,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        RepeatPayment,
        ClientAuthenticationToken,
        MandateRevoke,
        Capture,
        IncrementalAuthorization,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
    ],
    not_supported: [
        Accept,
        DefendDispute,
        SubmitEvidence,
        VoidPC,
    ],
);

// Additional flows required by ConnectorServiceTrait bounds that are not covered
// by `macro_connector_flow_status_impls!` above.
impl_unsupported_connector_flow!(
    connector_flow::VerifyWebhookSource,
    VerifyWebhookSourceFlowData,
    VerifyWebhookSourceRequestData,
    VerifyWebhookSourceResponseData
);
impl_unsupported_connector_flow!(
    connector_flow::VoidPostRefund,
    RefundFlowData,
    RefundVoidPostRefundData,
    RefundsResponseData
);
impl_unsupported_connector_flow!(
    connector_flow::GetPaymentMethod,
    PaymentFlowData,
    GetPaymentMethodData,
    GetPaymentMethodResponseData
);
impl_unsupported_connector_flow!(
    connector_flow::CreatePaymentMethod,
    PaymentFlowData,
    CreatePaymentMethodData,
    CreatePaymentMethodResponseData
);
impl_unsupported_connector_flow!(
    connector_flow::RefreshPaymentMethod,
    RefreshPaymentMethodFlowData,
    RefreshPaymentMethodData<T>,
    RefreshPaymentMethodResponseData
);
impl_unsupported_connector_flow!(
    connector_flow::Recharge,
    PaymentFlowData,
    RechargeRequestData,
    RechargeResponseData
);

// ===== SOURCE VERIFICATION IMPLEMENTATION =====
// Simple non-generic trait for webhook signature verification
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::verification::SourceVerification for Maya<T>
{
}
