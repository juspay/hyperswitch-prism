pub mod transformers;
use std::fmt::Debug;

use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt};
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, PSync, RSync, Refund, RepeatPayment,
        SetupMandate, Void,
    },
    connector_types::{
        ClientAuthenticationTokenRequestData, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        SetupMandateRequestData,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use hyperswitch_masking::{ExposeInterface, ExposeOptionInterface, Mask, Maskable};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    self as nexinets, NexinetsCaptureOrVoidRequest,
    NexinetsCaptureOrVoidRequest as NexinetsVoidRequest, NexinetsClientAuthRequest,
    NexinetsClientAuthResponse, NexinetsErrorResponse,
    NexinetsPaymentResponse as NexinetsCaptureResponse, NexinetsPaymentResponse,
    NexinetsPaymentResponse as NexinetsVoidResponse, NexinetsPaymentsRequest,
    NexinetsPreAuthOrDebitResponse, NexinetsRefundRequest, NexinetsRefundResponse,
    NexinetsRefundResponse as RefundSyncResponse, NexinetsRepeatPaymentRequest,
    NexinetsRepeatPaymentResponse, NexinetsSetupMandateRequest, NexinetsSetupMandateResponse,
    NexinetsTransactionType,
};

use super::macros;
use crate::{types::ResponseRouterData, utils, with_error_response_body};
pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;
use error_stack::ResultExt;
pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Nexinets<T>
{
    fn id(&self) -> &'static str {
        "nexinets"
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = nexinets::NexinetsAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;

        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            auth.api_key.into_masked(),
        )])
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.nexinets.base_url.as_ref()
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: NexinetsErrorResponse = res
            .response
            .parse_struct("NexinetsErrorResponse")
            .change_context(
                utils::response_deserialization_fail(
                    res.status_code,
                "nexinets: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        with_error_response_body!(event_builder, response);

        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");
        let errors = response.errors;
        let mut message = String::new();
        let mut static_message = String::new();
        for error in errors.iter() {
            let field = error.field.to_owned().unwrap_or_default();
            let mut msg = String::new();
            if !field.is_empty() {
                msg.push_str(format!("{} : {}", field, error.message).as_str());
            } else {
                error.message.clone_into(&mut msg)
            }
            if message.is_empty() {
                message.push_str(&msg);
                static_message.push_str(&msg);
            } else {
                message.push_str(format!(", {msg}").as_str());
            }
        }
        let connector_reason = format!("reason : {} , message : {}", response.message, message);

        Ok(ErrorResponse {
            status_code: response.status,
            code: response.code.to_string(),
            message: static_message,
            reason: Some(connector_reason),
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
            typed_connector_response: typed,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        })
    }
}

//marker traits
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Nexinets<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Nexinets,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Nexinets<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Nexinets<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Nexinets<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Nexinets<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Nexinets<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Nexinets<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Nexinets<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Nexinets<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Nexinets<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Nexinets<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Nexinets<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Nexinets<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Nexinets<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Nexinets<T>
{
}
macros::create_all_prerequisites!(
    connector_name: Nexinets,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: NexinetsPaymentsRequest<T>,
            response_body: NexinetsPreAuthOrDebitResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: NexinetsPaymentResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: NexinetsCaptureOrVoidRequest,
            response_body: NexinetsCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: NexinetsRefundRequest,
            response_body: NexinetsRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: RefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: Void,
            request_body: NexinetsVoidRequest,
            response_body: NexinetsVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: ClientAuthenticationToken,
            request_body: NexinetsClientAuthRequest,
            response_body: NexinetsClientAuthResponse,
            router_data: RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: NexinetsSetupMandateRequest<T>,
            response_body: NexinetsSetupMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: NexinetsRepeatPaymentRequest,
            response_body: NexinetsRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.get_content_type().to_string().into(),
            )];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);
            Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.nexinets.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.nexinets.base_url
        }

        pub fn connector_base_url_merchant_auth<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, MerchantAuthenticationFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.nexinets.base_url
        }

    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nexinets,
    curl_request: Json(NexinetsPaymentsRequest),
    curl_response: NexinetsPreAuthOrDebitResponse,
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
                    let url = if matches!(
            req.request.capture_method,
            Some(common_enums::CaptureMethod::Automatic) | Some(common_enums::CaptureMethod::SequentialAutomatic)
        ) {
            format!("{}/orders/debit", self.connector_base_url_payments(req))
        } else {
            format!("{}/orders/preauth", self.connector_base_url_payments(req))
        };
        Ok(url)
        }
    }
);

// Macro implementations for PSync, Capture, Refund, and RSync flows
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nexinets,
    curl_response: NexinetsPreAuthOrDebitResponse,
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
        let meta: nexinets::NexinetsPaymentsMetadata =
            utils::to_connector_meta(req.request.connector_feature_data.clone().map(|secret| secret.expose()))?;
        let order_id = nexinets::get_order_id(&meta)?;
        let transaction_id = match meta.psync_flow {
            NexinetsTransactionType::Debit
            | NexinetsTransactionType::Capture => {
                req.request.get_connector_transaction_id()?
            }
            _ => nexinets::get_transaction_id(&meta)?
};
            Ok(format!(
                "{}/orders/{order_id}/transactions/{transaction_id}",
                self.connector_base_url_payments(req),
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nexinets,
    curl_request: Json(NexinetsCaptureOrVoidRequest),
    curl_response: NexinetsCaptureResponse,
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
        let meta: nexinets::NexinetsPaymentsMetadata =
            utils::to_connector_meta(req.request.metadata.clone().map(|secret| secret.expose()))?;
        let order_id = nexinets::get_order_id(&meta)?;
        let transaction_id = nexinets::get_transaction_id(&meta)?;
        Ok(format!(
            "{}/orders/{order_id}/transactions/{transaction_id}/capture",
            self.connector_base_url_payments(req)
        ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nexinets,
    curl_request: Json(NexinetsRefundRequest),
    curl_response: NexinetsRefundResponse,
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
        let meta: nexinets::NexinetsPaymentsMetadata =
            utils::to_connector_meta(req.request.connector_feature_data.clone().expose_option())?;
        let order_id = nexinets::get_order_id(&meta)?;

            Ok(format!(
                "{}/orders/{order_id}/transactions/{}/refund",
                self.connector_base_url_refunds(req),
                req.request.connector_transaction_id
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nexinets,
    curl_response: RefundSyncResponse,
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
            let transaction_id = req
                .request
                .connector_refund_id
                .clone();

            let meta: nexinets::NexinetsPaymentsMetadata =
            utils::to_connector_meta(req.request.refund_connector_metadata.clone().map(|secret| secret.expose()))?;
        let order_id = nexinets::get_order_id(&meta)?;

            Ok(format!(
                "{}/orders/{order_id}/transactions/{transaction_id}",
                self.connector_base_url_refunds(req),
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nexinets,
    curl_request: Json(NexinetsVoidRequest),
    curl_response: NexinetsVoidResponse,
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
        let meta: nexinets::NexinetsPaymentsMetadata =
            utils::to_connector_meta(req.request.metadata.clone().map(|secret| secret.expose()))?;
        let order_id = nexinets::get_order_id(&meta)?;
        let transaction_id = nexinets::get_transaction_id(&meta)?;
        Ok(format!(
            "{}/orders/{order_id}/transactions/{transaction_id}/cancel",
            self.connector_base_url_payments(req),
        ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nexinets,
    curl_request: Json(NexinetsClientAuthRequest),
    curl_response: NexinetsClientAuthResponse,
    flow_name: ClientAuthenticationToken,
    resource_common_data: MerchantAuthenticationFlowData,
    flow_request: ClientAuthenticationTokenRequestData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/orders", self.connector_base_url_merchant_auth(req)))
        }
    }
);

// SetupMandate flow implementation using macro. Nexinets has no dedicated
// mandate-setup endpoint — the canonical approach is to issue a PREAUTH
// against `/orders/preauth` with a `cofContract` of type UNSCHEDULED. The
// returned `paymentInstrument.paymentInstrumentId` is surfaced as the
// connector_mandate_id for subsequent RepeatPayment (MIT) calls.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nexinets,
    curl_request: Json(NexinetsSetupMandateRequest<T>),
    curl_response: NexinetsSetupMandateResponse,
    flow_name: SetupMandate,
    resource_common_data: PaymentFlowData,
    flow_request: SetupMandateRequestData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/orders/preauth", self.connector_base_url_payments(req)))
        }
    }
);

// RepeatPayment (MIT / Merchant-Initiated Transaction) uses the same Nexinets
// payments endpoint as Authorize but substitutes the card panel with a stored
// `paymentInstrument.paymentInstrumentId` reference. Auto-capture flows hit
// `/orders/debit`; manual-capture flows hit `/orders/preauth`. The stored
// credential lifecycle is signalled via `cofContract` type UNSCHEDULED.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nexinets,
    curl_request: Json(NexinetsRepeatPaymentRequest),
    curl_response: NexinetsRepeatPaymentResponse,
    flow_name: RepeatPayment,
    resource_common_data: PaymentFlowData,
    flow_request: RepeatPaymentData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let url = if matches!(
                req.request.capture_method,
                Some(common_enums::CaptureMethod::Manual) | Some(common_enums::CaptureMethod::ManualMultiple)
            ) {
                format!("{}/orders/preauth", self.connector_base_url_payments(req))
            } else {
                format!("{}/orders/debit", self.connector_base_url_payments(req))
            };
            Ok(url)
        }
    }
);

// ConnectorIntegrationV2 implementations for authentication flows

// SourceVerification implementations for authentication flows

// ── Terminal status mapping for the Authorize flow ────────────────────────────
//
// Nexinets uses the same `NexinetsPaymentStatus::Success` for both paths of
// Authorize, with `NexinetsTransactionType` (returned in the response) telling
// which path was taken:
//   Preauth (manual-capture) → Authorized
//   Debit   (auto-capture)   → Charged
//
// `MappingContext::default()` = Preauth, so `assert_terminal_mapping!` tests the
// manual-capture (Authorized) path.

domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Nexinets<T>,
    flow: Authorize,
    source: nexinets::NexinetsPaymentStatus,
    context: NexinetsTransactionType,
    mapping: |status, ctx| {
        match (status, ctx) {
            (nexinets::NexinetsPaymentStatus::Success, NexinetsTransactionType::Preauth) => {
                success!(Authorized)
            }
            (nexinets::NexinetsPaymentStatus::Success, NexinetsTransactionType::Debit) => {
                success!(Charged)
            }
            (nexinets::NexinetsPaymentStatus::Success, _) => non_terminal!(Pending),
            (nexinets::NexinetsPaymentStatus::Declined, _)
            | (nexinets::NexinetsPaymentStatus::Failure, _)
            | (nexinets::NexinetsPaymentStatus::Expired, _)
            | (nexinets::NexinetsPaymentStatus::Aborted, _) => failure!(AuthorizationFailed),

            // Ok with Preauth = still authorized; otherwise still processing
            (nexinets::NexinetsPaymentStatus::Ok, NexinetsTransactionType::Preauth) => {
                success!(Authorized)
            }
            (nexinets::NexinetsPaymentStatus::Ok, _) => non_terminal!(Pending),

            (nexinets::NexinetsPaymentStatus::Pending, _) => non_terminal!(AuthenticationPending),
            (nexinets::NexinetsPaymentStatus::InProgress, _) => non_terminal!(Pending),
        } },
    runtime: {
        request: PaymentsAuthorizeData<T>,
        response: NexinetsPreAuthOrDebitResponse,
        // The TryFrom takes the first `transactions` entry; the extractor must mirror
        // that. An empty list cannot be represented here (the TryFrom errors out) —
        // `Pending` only acts as an inert placeholder in that impossible case.,
        source: |_resource_common_data, _request, response, _http_status_code| { Ok({
            response
                .transactions
                .first()
                .map(|t| t.status.clone())
                .unwrap_or(nexinets::NexinetsPaymentStatus::Pending)
        }) },
        context: |_resource_common_data, _request, response, _http_status_code| { Ok(response.transaction_type.clone()) },
    },
}

// PSync — mirrors the shared payment Sync TryFrom (transformers.rs:545) via `get_status`
// (transformers.rs:288), context = `psync_flow` from stored metadata. `PSync::ALLOWED` is
// a superset of `get_status`'s outputs (`Voided`, `VoidFailed`, `CaptureFailed` all
// reachable from the Cancel/Capture contexts), so the mirror is exact.
domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Nexinets<T>,
    flow: PSync,
    source: nexinets::NexinetsPaymentStatus,
    context: NexinetsTransactionType,
    mapping: |status, ctx| {
        let mapped_status = (|| { use common_enums::AttemptStatus;
        match (status, ctx) {
            (nexinets::NexinetsPaymentStatus::Success, NexinetsTransactionType::Preauth) => {
                AttemptStatus::Authorized
            }
            (
                nexinets::NexinetsPaymentStatus::Success,
                NexinetsTransactionType::Debit | NexinetsTransactionType::Capture,
            ) => AttemptStatus::Charged,
            (nexinets::NexinetsPaymentStatus::Success, NexinetsTransactionType::Cancel) => {
                AttemptStatus::Voided
            }
            (
                nexinets::NexinetsPaymentStatus::Declined
                | nexinets::NexinetsPaymentStatus::Failure
                | nexinets::NexinetsPaymentStatus::Expired
                | nexinets::NexinetsPaymentStatus::Aborted,
                NexinetsTransactionType::Preauth,
            ) => AttemptStatus::AuthorizationFailed,
            (
                nexinets::NexinetsPaymentStatus::Declined
                | nexinets::NexinetsPaymentStatus::Failure
                | nexinets::NexinetsPaymentStatus::Expired
                | nexinets::NexinetsPaymentStatus::Aborted,
                NexinetsTransactionType::Debit | NexinetsTransactionType::Capture,
            ) => AttemptStatus::CaptureFailed,
            (
                nexinets::NexinetsPaymentStatus::Declined
                | nexinets::NexinetsPaymentStatus::Failure
                | nexinets::NexinetsPaymentStatus::Expired
                | nexinets::NexinetsPaymentStatus::Aborted,
                NexinetsTransactionType::Cancel,
            ) => AttemptStatus::VoidFailed,
            (nexinets::NexinetsPaymentStatus::Ok, NexinetsTransactionType::Preauth) => {
                AttemptStatus::Authorized
            }
            (nexinets::NexinetsPaymentStatus::Ok, _) => AttemptStatus::Pending,
            (nexinets::NexinetsPaymentStatus::Pending, _) => AttemptStatus::AuthenticationPending,
            (nexinets::NexinetsPaymentStatus::InProgress, _) => AttemptStatus::Pending,
        } })();
        match mapped_status {
            common_enums::AttemptStatus::Authorized => success!(Authorized),
            common_enums::AttemptStatus::AutoRefunded => success!(AutoRefunded),
            common_enums::AttemptStatus::Charged => success!(Charged),
            common_enums::AttemptStatus::PartialCharged => success!(PartialCharged),
            common_enums::AttemptStatus::PartiallyAuthorized => success!(PartiallyAuthorized),
            common_enums::AttemptStatus::Voided => success!(Voided),
            common_enums::AttemptStatus::VoidedPostCapture => success!(VoidedPostCapture),
            common_enums::AttemptStatus::AuthenticationFailed => failure!(AuthenticationFailed),
            common_enums::AttemptStatus::AuthorizationFailed => failure!(AuthorizationFailed),
            common_enums::AttemptStatus::CaptureFailed => failure!(CaptureFailed),
            common_enums::AttemptStatus::Failure => failure!(Failure),
            common_enums::AttemptStatus::IntegrityFailure => failure!(IntegrityFailure),
            common_enums::AttemptStatus::VoidFailed => failure!(VoidFailed),
            common_enums::AttemptStatus::AuthenticationPending => non_terminal!(AuthenticationPending),
            common_enums::AttemptStatus::AuthenticationSuccessful => non_terminal!(AuthenticationSuccessful),
            common_enums::AttemptStatus::Authorizing => non_terminal!(Authorizing),
            common_enums::AttemptStatus::CaptureInitiated => non_terminal!(CaptureInitiated),
            common_enums::AttemptStatus::CodInitiated => non_terminal!(CodInitiated),
            common_enums::AttemptStatus::ConfirmationAwaited => non_terminal!(ConfirmationAwaited),
            common_enums::AttemptStatus::DeviceDataCollectionPending => non_terminal!(DeviceDataCollectionPending),
            common_enums::AttemptStatus::Expired => non_terminal!(Expired),
            common_enums::AttemptStatus::PartialChargedAndChargeable => non_terminal!(PartialChargedAndChargeable),
            common_enums::AttemptStatus::PaymentMethodAwaited => non_terminal!(PaymentMethodAwaited),
            common_enums::AttemptStatus::Pending => non_terminal!(Pending),
            common_enums::AttemptStatus::Started => non_terminal!(Started),
            common_enums::AttemptStatus::Unknown => non_terminal!(Unknown),
            common_enums::AttemptStatus::Unresolved => non_terminal!(Unresolved),
            common_enums::AttemptStatus::Unspecified => non_terminal!(Unspecified),
            common_enums::AttemptStatus::VoidInitiated => non_terminal!(VoidInitiated),
            common_enums::AttemptStatus::VoidPostCaptureInitiated => non_terminal!(VoidPostCaptureInitiated),
            _ => failure!(Failure),
        }
    },
    runtime: {
        request: PaymentsSyncData,
        response: NexinetsPaymentResponse,
        source: |_resource_common_data, _request, response, _http_status_code| { Ok(response.status.clone()) },
        context: |_resource_common_data, _request, response, _http_status_code| { Ok(response.transaction_type.clone()) },
    },
}

// Capture — mirrors the Capture TryFrom via `get_status` (the capture endpoint reuses
// `NexinetsPaymentResponse`; context is always `NexinetsTransactionType::Capture`).
// Deviations: `Voided` is not in `Capture::ALLOWED`, so unreachable Cancel-context rows
// map to `CaptureFailed`; `Ok` → `CaptureInitiated` (still in flight).
domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Nexinets<T>,
    flow: Capture,
    source: nexinets::NexinetsPaymentStatus,
    context: NexinetsTransactionType,
    mapping: |status, _ctx| {
        let mapped_status = (|| { use common_enums::AttemptStatus;
        match status {
            nexinets::NexinetsPaymentStatus::Success => AttemptStatus::Charged,
            // Ok: capture accepted, settlement still in flight.
            nexinets::NexinetsPaymentStatus::Ok
            | nexinets::NexinetsPaymentStatus::InProgress => AttemptStatus::CaptureInitiated,
            nexinets::NexinetsPaymentStatus::Pending => AttemptStatus::Pending,
            nexinets::NexinetsPaymentStatus::Declined
            | nexinets::NexinetsPaymentStatus::Failure
            | nexinets::NexinetsPaymentStatus::Expired
            | nexinets::NexinetsPaymentStatus::Aborted => AttemptStatus::CaptureFailed,
        } })();
        match mapped_status {
            common_enums::AttemptStatus::Charged => success!(Charged),
            common_enums::AttemptStatus::PartialCharged => success!(PartialCharged),
            common_enums::AttemptStatus::CaptureFailed => failure!(CaptureFailed),
            common_enums::AttemptStatus::Failure => failure!(Failure),
            common_enums::AttemptStatus::IntegrityFailure => failure!(IntegrityFailure),
            common_enums::AttemptStatus::CaptureInitiated => non_terminal!(CaptureInitiated),
            common_enums::AttemptStatus::PartialChargedAndChargeable => non_terminal!(PartialChargedAndChargeable),
            common_enums::AttemptStatus::Pending => non_terminal!(Pending),
            _ => failure!(Failure),
        }
    },
    runtime: {
        request: PaymentsCaptureData,
        response: NexinetsCaptureResponse,
        source: |_resource_common_data, _request, response, _http_status_code| { Ok(response.status.clone()) },
        context: |_resource_common_data, _request, response, _http_status_code| { Ok(response.transaction_type.clone()) },
    },
}

// Void — mirrors the Void TryFrom via `get_status` (context always
// `NexinetsTransactionType::Cancel`); `Void::ALLOWED` covers `Voided`, `VoidFailed` and
// `Pending`, and `Ok` → `VoidInitiated` (cancel accepted, still settling).
domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Nexinets<T>,
    flow: Void,
    source: nexinets::NexinetsPaymentStatus,
    context: NexinetsTransactionType,
    mapping: |status, _ctx| {
        let mapped_status = (|| { use common_enums::AttemptStatus;
        match status {
            nexinets::NexinetsPaymentStatus::Success => AttemptStatus::Voided,
            nexinets::NexinetsPaymentStatus::Ok
            | nexinets::NexinetsPaymentStatus::InProgress
            | nexinets::NexinetsPaymentStatus::Pending => AttemptStatus::VoidInitiated,
            nexinets::NexinetsPaymentStatus::Declined
            | nexinets::NexinetsPaymentStatus::Failure
            | nexinets::NexinetsPaymentStatus::Expired
            | nexinets::NexinetsPaymentStatus::Aborted => AttemptStatus::VoidFailed,
        } })();
        match mapped_status {
            common_enums::AttemptStatus::Voided => success!(Voided),
            common_enums::AttemptStatus::Failure => failure!(Failure),
            common_enums::AttemptStatus::VoidFailed => failure!(VoidFailed),
            common_enums::AttemptStatus::Pending => non_terminal!(Pending),
            common_enums::AttemptStatus::VoidInitiated => non_terminal!(VoidInitiated),
            _ => failure!(Failure),
        }
    },
    runtime: {
        request: PaymentVoidData,
        response: NexinetsVoidResponse,
        source: |_resource_common_data, _request, response, _http_status_code| { Ok(response.status.clone()) },
        context: |_resource_common_data, _request, response, _http_status_code| { Ok(response.transaction_type.clone()) },
    },
}

// SetupMandate — mirrors the SetupMandate TryFrom (transformers.rs:~1190): `get_status`
// on the always-`Preauth` mandate request, with `Authorized` promoted to `Charged` (a
// registered zero/low-amount mandate setup must terminate; `Authorized` is not in
// `SetupMandate::ALLOWED`).
domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Nexinets<T>,
    flow: SetupMandate,
    source: nexinets::NexinetsPaymentStatus,
    context: NexinetsTransactionType,
    mapping: |status, _ctx| {
        let mapped_status = (|| { use common_enums::AttemptStatus;
        match status {
            // Preauth-context Success/Ok → get_status gives Authorized, promoted to Charged.
            nexinets::NexinetsPaymentStatus::Success
            | nexinets::NexinetsPaymentStatus::Ok => AttemptStatus::Charged,
            nexinets::NexinetsPaymentStatus::Pending => AttemptStatus::AuthenticationPending,
            nexinets::NexinetsPaymentStatus::InProgress => AttemptStatus::Pending,
            nexinets::NexinetsPaymentStatus::Declined
            | nexinets::NexinetsPaymentStatus::Failure
            | nexinets::NexinetsPaymentStatus::Expired
            | nexinets::NexinetsPaymentStatus::Aborted => AttemptStatus::Failure,
        } })();
        match mapped_status {
            common_enums::AttemptStatus::Charged => success!(Charged),
            common_enums::AttemptStatus::AuthenticationFailed => failure!(AuthenticationFailed),
            common_enums::AttemptStatus::AuthorizationFailed => failure!(AuthorizationFailed),
            common_enums::AttemptStatus::Failure => failure!(Failure),
            common_enums::AttemptStatus::AuthenticationPending => non_terminal!(AuthenticationPending),
            common_enums::AttemptStatus::AuthenticationSuccessful => non_terminal!(AuthenticationSuccessful),
            common_enums::AttemptStatus::Pending => non_terminal!(Pending),
            common_enums::AttemptStatus::Started => non_terminal!(Started),
            _ => failure!(Failure),
        }
    },
    runtime: {
        request: SetupMandateRequestData<T>,
        response: NexinetsSetupMandateResponse,
        // The TryFrom takes the first `transactions` entry; the extractor must mirror
        // that. An empty list cannot be represented here (the TryFrom errors out) —
        // `Pending` only acts as an inert placeholder in that impossible case.,
        source: |_resource_common_data, _request, response, _http_status_code| { Ok({
            response
                .transactions
                .first()
                .map(|t| t.status.clone())
                .unwrap_or(nexinets::NexinetsPaymentStatus::Pending)
        }) },
        context: |_resource_common_data, _request, response, _http_status_code| { Ok(response.transaction_type.clone()) },
    },
}

// RepeatPayment — mirrors the RepeatPayment TryFrom (transformers.rs:1424) via
// `get_status`. Every produced value is RepeatPayment-legal (`Authorized` is
// intermediate-pending for MIT — the merchant may capture later), but only
// `Charged` is a *terminal* success in `RepeatPayment::TERMINAL_SUCCESS_SET`, so
// the declared success_targets is narrowed to `[Charged]`; `Authorized` remains
// reachable in the body as an intermediate state.
domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Nexinets<T>,
    flow: RepeatPayment,
    source: nexinets::NexinetsPaymentStatus,
    context: NexinetsTransactionType,
    mapping: |status, ctx| {
        match (status, ctx) {
            (nexinets::NexinetsPaymentStatus::Success, NexinetsTransactionType::Preauth) => {
                non_terminal!(Authorized)
            }
            (nexinets::NexinetsPaymentStatus::Success, NexinetsTransactionType::Debit | NexinetsTransactionType::Capture) => success!(Charged),
            (nexinets::NexinetsPaymentStatus::Success, NexinetsTransactionType::Cancel) => failure!(Failure),
            (nexinets::NexinetsPaymentStatus::Ok, NexinetsTransactionType::Preauth) => non_terminal!(Authorized),
            (nexinets::NexinetsPaymentStatus::Ok, _) => non_terminal!(Pending),
            (nexinets::NexinetsPaymentStatus::Pending, _) => {
                non_terminal!(AuthenticationPending)
            }
            (nexinets::NexinetsPaymentStatus::InProgress, _) => non_terminal!(Pending),
            (nexinets::NexinetsPaymentStatus::Declined | nexinets::NexinetsPaymentStatus::Failure | nexinets::NexinetsPaymentStatus::Expired | nexinets::NexinetsPaymentStatus::Aborted, _) => failure!(Failure),
        } },
    runtime: {
        request: RepeatPaymentData<T>,
        response: NexinetsRepeatPaymentResponse,
        // The TryFrom takes the first `transactions` entry; the extractor must mirror
        // that. An empty list cannot be represented here (the TryFrom errors out) —
        // `Pending` only acts as an inert placeholder in that impossible case.,
        source: |_resource_common_data, _request, response, _http_status_code| { Ok({
            response
                .transactions
                .first()
                .map(|t| t.status.clone())
                .unwrap_or(nexinets::NexinetsPaymentStatus::Pending)
        }) },
        context: |_resource_common_data, _request, response, _http_status_code| { Ok(response.transaction_type.clone()) },
    },
}

// NOTE: no impl_flow_status_mapping! for IncrementalAuthorization / ClientAuthenticationToken.
// IncrementalAuthorization is declared `not_implemented:` below — the flow never dispatches
// (no response type is parsed), so there is no wire status to map. ClientAuthenticationToken
// answers with a bare `order_id` (`NexinetsClientAuthResponse`) and has no `FlowStatusRules`.

// Refund — mirrors `From<nexinets::RefundStatus> for common_enums::RefundStatus`
// (transformers.rs:650) used by the Refund TryFrom (transformers.rs:660). The `_ctx`
// variant is used only because the enum is mirrored in full with a per-variant pair list.
domain_types::impl_refund_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Nexinets<T>,
    flow: Refund,
    source: nexinets::RefundStatus,
    mapping: |status| { match status {
            nexinets::RefundStatus::Success => success!(Success),
            nexinets::RefundStatus::Failure
            | nexinets::RefundStatus::Declined => failure!(Failure),
            nexinets::RefundStatus::InProgress
            | nexinets::RefundStatus::Ok => non_terminal!(Pending),
        } },
    runtime: {
        request: RefundsData,
        response: NexinetsRefundResponse,
        source: |_resource_common_data, _request, response, _http_status_code| { Ok(response.status.clone()) },
    },
}

// RSync — same `From<nexinets::RefundStatus>` mapping as Refund (the sync TryFrom at
// transformers.rs:679 shares it verbatim).
domain_types::impl_refund_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Nexinets<T>,
    flow: RSync,
    source: nexinets::RefundStatus,
    mapping: |status| { match status {
            nexinets::RefundStatus::Success => success!(Success),
            nexinets::RefundStatus::Failure
            | nexinets::RefundStatus::Declined => failure!(Failure),
            nexinets::RefundStatus::InProgress
            | nexinets::RefundStatus::Ok => non_terminal!(Pending),
        } },
    runtime: {
        request: RefundSyncData,
        response: RefundSyncResponse,
        source: |_resource_common_data, _request, response, _http_status_code| { Ok(response.status.clone()) },
    },
}

macros::macro_connector_flow_status_impls!(
    connector: Nexinets,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        IncrementalAuthorization,
        CreateOrder,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        MandateRevoke,
        PaymentMethodToken,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
    ],
    not_supported: [
        VoidPostRefund,
        VoidPC,
        ServerSessionAuthenticationToken,
        ServerAuthenticationToken,
        SubmitEvidence,
        DefendDispute,
        Accept,
    ],
);
