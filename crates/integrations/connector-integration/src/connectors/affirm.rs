pub mod transformers;

use std::fmt::Debug;

use base64::Engine;
use common_enums::CurrencyUnit;
use common_utils::{
    consts::{BASE64_ENGINE, NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    types::MinorUnit,
};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData,
    },
    errors::{
        ConnectorError, IntegrationError, IntegrationErrorContext,
        ResponseTransformationErrorContext,
    },
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Maskable};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    affirm_checkout_token, AffirmAuthType, AffirmCaptureRequest, AffirmCaptureResponse,
    AffirmErrorResponse, AffirmPaymentsRequest, AffirmPaymentsResponse, AffirmRSyncResponse,
    AffirmRefundRequest, AffirmRefundResponse, AffirmSyncResponse, AffirmVoidRequest,
    AffirmVoidResponse,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

pub(crate) mod headers {
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

const TRANSACTIONS_PATH: &str = "/api/v1/transactions";
// Affirm's checkout-create endpoint — used on the INITIATE leg of Authorize to
// mint the hosted-checkout redirect URL (before any checkout_token exists).
const CHECKOUT_DIRECT_PATH: &str = "/api/v2/checkout/direct";

macros::macro_connector_payout_implementation!(
    connector: Affirm,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Affirm<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Affirm<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Affirm<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Affirm<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Affirm<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Affirm<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Affirm<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Affirm<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Affirm<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Affirm<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Affirm<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Affirm<T>
{
}

macros::create_amount_converter_wrapper!(connector_name: Affirm, amount_type: MinorUnit);

macros::create_all_prerequisites!(
    connector_name: Affirm,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: AffirmPaymentsRequest,
            response_body: AffirmPaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: AffirmSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: AffirmCaptureRequest,
            response_body: AffirmCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: AffirmVoidRequest,
            response_body: AffirmVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: AffirmRefundRequest,
            response_body: AffirmRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: AffirmRSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: MinorUnit
    ],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            )];
            let mut auth_header = self.get_auth_header(&req.connector_config)?;
            header.append(&mut auth_header);
            Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.affirm.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.affirm.base_url
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Affirm<T>
{
    fn id(&self) -> &'static str {
        "affirm"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.affirm.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = AffirmAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Affirm uses HTTP Basic auth: provide public_key and private_key via x-connector-config."
                            .to_string(),
                    ),
                    ..Default::default()
                },
            },
        )?;
        // HTTP Basic auth: base64(public_key:private_key)
        let credentials = format!("{}:{}", auth.public_key.expose(), auth.private_key.expose());
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
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: AffirmErrorResponse = res
            .response
            .parse_struct("AffirmErrorResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: ResponseTransformationErrorContext {
                    http_status_code: Some(res.status_code),
                    additional_context: Some(
                        "Failed to parse the Affirm error response body.".to_string(),
                    ),
                },
            })?;

        with_error_response_body!(event_builder, response);
        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response
                .code
                .clone()
                .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            message: response
                .message
                .clone()
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            reason: response.message,
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
            typed_connector_response: typed,
        })
    }
}

// ===== AUTHORIZE: POST /api/v1/transactions =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Affirm,
    curl_request: Json(AffirmPaymentsRequest),
    curl_response: AffirmPaymentsResponse,
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
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // INITIATE (no checkout_token yet) hits the checkout-create endpoint to
            // mint the redirect URL; COMPLETE (token present) charges the transaction.
            let path = match affirm_checkout_token(req) {
                Some(_) => TRANSACTIONS_PATH,
                None => CHECKOUT_DIRECT_PATH,
            };
            Ok(format!("{}{path}", self.connector_base_url_payments(req)))
        }
    }
);

// ===== PSYNC: GET /api/v1/transactions/{id} =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Affirm,
    curl_response: AffirmSyncResponse,
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
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let transaction_id = req
                .request
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Affirm PSync requires the connector_transaction_id returned by Authorize COMPLETE."
                                .to_string(),
                        ),
                        ..Default::default()
                    },
                })?;
            Ok(format!(
                "{}{TRANSACTIONS_PATH}/{transaction_id}?expand=events",
                self.connector_base_url_payments(req)
            ))
        }
    }
);

// ===== CAPTURE: POST /api/v1/transactions/{id}/capture =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Affirm,
    curl_request: Json(AffirmCaptureRequest),
    curl_response: AffirmCaptureResponse,
    flow_name: Capture,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let transaction_id = req
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Affirm Capture requires the connector_transaction_id returned by Authorize COMPLETE."
                                .to_string(),
                        ),
                        ..Default::default()
                    },
                })?;
            Ok(format!(
                "{}{TRANSACTIONS_PATH}/{transaction_id}/capture",
                self.connector_base_url_payments(req)
            ))
        }
    }
);

// ===== VOID: POST /api/v1/transactions/{id}/void =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Affirm,
    curl_request: Json(AffirmVoidRequest),
    curl_response: AffirmVoidResponse,
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
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let transaction_id = &req.request.connector_transaction_id;
            Ok(format!(
                "{}{TRANSACTIONS_PATH}/{transaction_id}/void",
                self.connector_base_url_payments(req)
            ))
        }
    }
);

// ===== REFUND: POST /api/v1/transactions/{id}/refund =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Affirm,
    curl_request: Json(AffirmRefundRequest),
    curl_response: AffirmRefundResponse,
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
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let transaction_id = &req.request.connector_transaction_id;
            Ok(format!(
                "{}{TRANSACTIONS_PATH}/{transaction_id}/refund",
                self.connector_base_url_refunds(req)
            ))
        }
    }
);

// ===== RSYNC: GET /api/v1/transactions/{id}?expand=events =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Affirm,
    curl_response: AffirmRSyncResponse,
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
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let transaction_id = &req.request.connector_transaction_id;
            Ok(format!(
                "{}{TRANSACTIONS_PATH}/{transaction_id}?expand=events",
                self.connector_base_url_refunds(req)
            ))
        }
    }
);

macros::macro_connector_flow_status_impls!(
    connector: Affirm,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        CreateConnectorCustomer,
        GetConnectorCustomer,
        CreateOrder,
        MandateRevoke,
        PaymentMethodToken,
        RepeatPayment,
        SetupMandate
    ],
    not_supported: [
        VoidPostRefund,
        Accept,
        Authenticate,
        ClientAuthenticationToken,
        DefendDispute,
        IncrementalAuthorization,
        PostAuthenticate,
        PreAuthenticate,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        SubmitEvidence,
        VoidPC
    ],
);
