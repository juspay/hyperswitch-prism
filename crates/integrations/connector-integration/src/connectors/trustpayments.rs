pub mod transformers;

use std::fmt::Debug;

use common_enums::{AttemptStatus, CurrencyUnit, RefundStatus};
use common_utils::{
    errors::CustomResult, events, ext_traits::ByteSliceExt, types::StringMinorUnit,
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, IncrementalAuthorization, PSync, RSync, Refund, RepeatPayment,
        SetupMandate, Void,
    },
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsIncrementalAuthorizationData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        SetupMandateRequestData,
    },
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    self as trustpayments, TrustpaymentsAuthorizeRequest, TrustpaymentsAuthorizeResponse,
    TrustpaymentsCaptureRequest, TrustpaymentsCaptureResponse, TrustpaymentsIncrementalAuthRequest,
    TrustpaymentsIncrementalAuthResponse, TrustpaymentsPSyncRequest, TrustpaymentsPSyncResponse,
    TrustpaymentsRSyncRequest, TrustpaymentsRSyncResponse, TrustpaymentsRefundRequest,
    TrustpaymentsRefundResponse, TrustpaymentsRepeatPaymentRequest,
    TrustpaymentsRepeatPaymentResponse, TrustpaymentsSetupMandateRequest,
    TrustpaymentsSetupMandateResponse, TrustpaymentsVoidRequest, TrustpaymentsVoidResponse,
};

use super::macros;
use crate::{types::ResponseRouterData, utils, with_error_response_body};
use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

// ===== CONNECTOR SERVICE TRAIT IMPLEMENTATIONS =====

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Trustpayments<T>
{
}

// ===== PAYMENT FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Trustpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Trustpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Trustpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Trustpayments<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Trustpayments,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

// ===== REFUND FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Trustpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Trustpayments<T>
{
}

// ===== ADVANCED FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentIncrementalAuthorization for Trustpayments<T>
{
}

// ===== AUTHENTICATION FLOW TRAIT IMPLEMENTATIONS =====
// ===== DISPUTE FLOW TRAIT IMPLEMENTATIONS =====
// ===== WEBHOOK TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Trustpayments<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Trustpayments<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Trustpayments<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Trustpayments<T>
{
}

// ===== VALIDATION TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Trustpayments<T>
{
}

// ===== CONNECTOR CUSTOMER TRAIT IMPLEMENTATIONS =====
// ===== MACRO-BASED CONNECTOR SETUP =====
macros::create_all_prerequisites!(
    connector_name: Trustpayments,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: TrustpaymentsAuthorizeRequest,
            response_body: TrustpaymentsAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: TrustpaymentsPSyncRequest,
            response_body: TrustpaymentsPSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: TrustpaymentsCaptureRequest,
            response_body: TrustpaymentsCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: TrustpaymentsRefundRequest,
            response_body: TrustpaymentsRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            request_body: TrustpaymentsRSyncRequest,
            response_body: TrustpaymentsRSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: Void,
            request_body: TrustpaymentsVoidRequest,
            response_body: TrustpaymentsVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: IncrementalAuthorization,
            request_body: TrustpaymentsIncrementalAuthRequest,
            response_body: TrustpaymentsIncrementalAuthResponse,
            router_data: RouterDataV2<IncrementalAuthorization, PaymentFlowData, PaymentsIncrementalAuthorizationData, PaymentsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: TrustpaymentsSetupMandateRequest,
            response_body: TrustpaymentsSetupMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: TrustpaymentsRepeatPaymentRequest,
            response_body: TrustpaymentsRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: StringMinorUnit
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
            &req.resource_common_data.connectors.trustpayments.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.trustpayments.base_url
        }
    }
);

// ===== CONNECTOR COMMON IMPLEMENTATION =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Trustpayments<T>
{
    fn id(&self) -> &'static str {
        "trustpayments"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.trustpayments.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = trustpayments::TrustpaymentsAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            auth.generate_basic_auth().peek().to_string().into(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: trustpayments::TrustpaymentsErrorResponse = res
            .response
            .parse_struct("TrustpaymentsErrorResponse")
            .change_context(
                utils::response_deserialization_fail(
                    res.status_code,
                "trustpayments: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        with_error_response_body!(event_builder, response);

        // Trust Payments can return errors in multiple formats:
        // 1. In "responses" array (AUTH endpoint)
        // 2. In "response" array (REFUND, QUERY endpoints)
        // 3. At top level (simple errors)
        let (error_code, error_message, connector_transaction_id) = response
            .responses
            .as_ref()
            .or(response.response.as_ref())
            .and_then(|responses| responses.first())
            .map(|error_item| {
                (
                    error_item.errorcode.clone(),
                    error_item.errormessage.clone(),
                    error_item.transactionreference.clone(),
                )
            })
            .or_else(|| {
                // Fallback to top-level error fields
                response.errorcode.clone().map(|code| {
                    (
                        code,
                        response.errormessage.clone().unwrap_or_default(),
                        None,
                    )
                })
            })
            .unwrap_or_else(|| {
                (
                    "UNKNOWN_ERROR".to_string(),
                    "Unknown error occurred".to_string(),
                    None,
                )
            });

        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");
        Ok(ErrorResponse {
            status_code: res.status_code,
            code: error_code,
            message: error_message.clone(),
            reason: Some(error_message),
            attempt_status: None,
            connector_transaction_id,
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

// ===== AUTHORIZE FLOW IMPLEMENTATION =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Trustpayments,
    curl_request: Json(TrustpaymentsAuthorizeRequest),
    curl_response: TrustpaymentsAuthorizeResponse,
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
            Ok(format!("{}/json/", self.connector_base_url_payments(req)))
        }
    }
);

// ===== PSYNC FLOW IMPLEMENTATION =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Trustpayments,
    curl_request: Json(TrustpaymentsPSyncRequest),
    curl_response: TrustpaymentsPSyncResponse,
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
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/json/", self.connector_base_url_payments(req)))
        }
    }
);

// ===== EMPTY IMPLEMENTATIONS FOR OTHER FLOWS =====

// ===== VOID FLOW IMPLEMENTATION =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Trustpayments,
    curl_request: Json(TrustpaymentsVoidRequest),
    curl_response: TrustpaymentsVoidResponse,
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
            Ok(format!("{}/json/", self.connector_base_url_payments(req)))
        }
    }
);

// ===== INCREMENTAL AUTHORIZATION FLOW IMPLEMENTATION =====
// Trust Payments treats incremental authorisations as an additional AUTH
// request with `authmethod = "INCREMENTAL"` and a `parenttransactionreference`.
// The endpoint is identical to a standard AUTH (POST /json/).
// Reference: https://docs.trustpayments.com/document/tru-connect/knowledge-base/authorisations/auth-method/incremental-authorisations-api/
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Trustpayments,
    curl_request: Json(TrustpaymentsIncrementalAuthRequest),
    curl_response: TrustpaymentsIncrementalAuthResponse,
    flow_name: IncrementalAuthorization,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsIncrementalAuthorizationData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<IncrementalAuthorization, PaymentFlowData, PaymentsIncrementalAuthorizationData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<IncrementalAuthorization, PaymentFlowData, PaymentsIncrementalAuthorizationData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/json/", self.connector_base_url_payments(req)))
        }
    }
);

// Payment Void Post Capture

// ===== CAPTURE FLOW IMPLEMENTATION =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Trustpayments,
    curl_request: Json(TrustpaymentsCaptureRequest),
    curl_response: TrustpaymentsCaptureResponse,
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
            Ok(format!("{}/json/", self.connector_base_url_payments(req)))
        }
    }
);

// ===== REFUND FLOW IMPLEMENTATION =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Trustpayments,
    curl_request: Json(TrustpaymentsRefundRequest),
    curl_response: TrustpaymentsRefundResponse,
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
            Ok(format!("{}/json/", self.connector_base_url_refunds(req)))
        }
    }
);

// ===== RSYNC FLOW IMPLEMENTATION =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Trustpayments,
    curl_request: Json(TrustpaymentsRSyncRequest),
    curl_response: TrustpaymentsRSyncResponse,
    flow_name: RSync,
    resource_common_data: RefundFlowData,
    flow_request: RefundSyncData,
    flow_response: RefundsResponseData,
    http_method: Post,
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
            Ok(format!("{}/json/", self.connector_base_url_refunds(req)))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Trustpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Trustpayments<T>
{
}

// ===== SETUP MANDATE FLOW IMPLEMENTATION =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Trustpayments,
    curl_request: Json(TrustpaymentsSetupMandateRequest),
    curl_response: TrustpaymentsSetupMandateResponse,
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
            Ok(format!("{}/json/", self.connector_base_url_payments(req)))
        }
    }
);

// ===== REPEAT PAYMENT FLOW IMPLEMENTATION =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Trustpayments,
    curl_request: Json(TrustpaymentsRepeatPaymentRequest),
    curl_response: TrustpaymentsRepeatPaymentResponse,
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
            Ok(format!("{}/json/", self.connector_base_url_payments(req)))
        }
    }
);

// Mandate Revoke

// Order Create

// Session Token

// Dispute Accept

// Dispute Defend

// Submit Evidence

// Payment Token (required by PaymentTokenV2 trait)

// Access Token (required by ServerAuthentication trait)

// ===== AUTHENTICATION FLOW CONNECTOR INTEGRATIONS =====
// Pre Authentication

// Authentication

// Post Authentication

// ===== CONNECTOR CUSTOMER CONNECTOR INTEGRATIONS =====
// Create Connector Customer

// ===== SOURCE VERIFICATION IMPLEMENTATIONS =====

// ===== AUTHENTICATION FLOW SOURCE VERIFICATION =====

// ===== CONNECTOR CUSTOMER SOURCE VERIFICATION =====

domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Trustpayments<T>,
    flow: Authorize,
    // `(errorcode, settlestatus, authcode)` distilled from the first response entry;
    // `None` when the response array is empty.
    source: Option<(String, Option<trustpayments::TrustpaymentsSettleStatus>, Option<String>)>,
    context: (),
    params: [source, _ctx],
    success: _ => [Authorized, Charged],
    failure: none,
    extractors: {
        request: PaymentsAuthorizeData<T>,
        response: TrustpaymentsAuthorizeResponse,
        source: |response| {
            response.responses.first().map(|auth_response| {
                (
                    auth_response.errorcode.clone(),
                    auth_response.settlestatus.clone(),
                    auth_response.authcode.clone(),
                )
            })
        },
        context: |_request, _response| (),
    },
    {
        let Some((errorcode, settlestatus, authcode)) = source else {
            return AttemptStatus::Failure;
        };
        if errorcode != "0" {
            return AttemptStatus::Failure;
        }
        match settlestatus {
            Some(trustpayments::TrustpaymentsSettleStatus::AutomaticCapture) => {
                if authcode.is_some() {
                    AttemptStatus::Charged
                } else {
                    AttemptStatus::Pending
                }
            }
            Some(trustpayments::TrustpaymentsSettleStatus::SettledPending)
            | Some(trustpayments::TrustpaymentsSettleStatus::SettledComplete) => {
                AttemptStatus::Charged
            }
            Some(trustpayments::TrustpaymentsSettleStatus::ManualCapture) => {
                if authcode.is_some() {
                    AttemptStatus::Authorized
                } else {
                    AttemptStatus::Pending
                }
            }
            Some(trustpayments::TrustpaymentsSettleStatus::Cancelled) => AttemptStatus::Voided,
            None => AttemptStatus::Pending,
        }
    }
}

domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Trustpayments<T>,
    flow: PSync,
    // `(response-level errorcode, first record)` where the record is
    // `(record errorcode, settlestatus, authcode)`; `None` on an empty response array.
    source: Option<(
        String,
        Option<(
            String,
            Option<trustpayments::TrustpaymentsSettleStatus>,
            Option<String>,
        )>,
    )>,
    context: (),
    params: [source, _ctx],
    success: _ => [Authorized, Charged, Voided],
    failure: none,
    extractors: {
        request: PaymentsSyncData,
        response: TrustpaymentsPSyncResponse,
        source: |response| {
            response.response.first().map(|response_item| {
                let record = response_item.records.as_ref().and_then(|records| {
                    records.first().map(|record| {
                        (
                            record.errorcode.clone(),
                            record.settlestatus.clone(),
                            record.authcode.clone(),
                        )
                    })
                });
                (response_item.errorcode.clone(), record)
            })
        },
        context: |_request, _response| (),
    },
    {
        let Some((response_errorcode, record)) = source else {
            return AttemptStatus::Failure;
        };
        if response_errorcode != "0" {
            return AttemptStatus::Failure;
        }
        let Some((record_errorcode, settlestatus, authcode)) = record else {
            // No transaction records found for the query.
            return AttemptStatus::Pending;
        };
        if record_errorcode != "0" {
            return AttemptStatus::Failure;
        }
        match settlestatus {
            Some(trustpayments::TrustpaymentsSettleStatus::AutomaticCapture) => {
                if authcode.is_some() {
                    AttemptStatus::Charged
                } else {
                    AttemptStatus::Pending
                }
            }
            Some(trustpayments::TrustpaymentsSettleStatus::SettledPending)
            | Some(trustpayments::TrustpaymentsSettleStatus::SettledComplete) => {
                AttemptStatus::Charged
            }
            Some(trustpayments::TrustpaymentsSettleStatus::ManualCapture) => {
                if authcode.is_some() {
                    AttemptStatus::Authorized
                } else {
                    AttemptStatus::Pending
                }
            }
            Some(trustpayments::TrustpaymentsSettleStatus::Cancelled) => AttemptStatus::Voided,
            None => AttemptStatus::Pending,
        }
    }
}

domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Trustpayments<T>,
    flow: Capture,
    source: Option<String>,
    context: (),
    params: [errorcode, _ctx],
    success: _ => [Charged],
    failure: none,
    extractors: {
        request: PaymentsCaptureData,
        response: TrustpaymentsCaptureResponse,
        source: |response| {
            response
                .response
                .first()
                .map(|response_item| response_item.errorcode.clone())
        },
        context: |_request, _response| (),
    },
    {
        match errorcode.as_deref() {
            // A successful TRANSACTIONUPDATE means the capture was accepted.
            Some("0") => AttemptStatus::Charged,
            _ => AttemptStatus::Failure,
        }
    }
}

domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Trustpayments<T>,
    flow: Void,
    source: Option<String>,
    context: (),
    params: [errorcode, _ctx],
    success: _ => [Voided],
    failure: none,
    extractors: {
        request: PaymentVoidData,
        response: TrustpaymentsVoidResponse,
        source: |response| {
            response
                .response
                .first()
                .map(|response_item| response_item.errorcode.clone())
        },
        context: |_request, _response| (),
    },
    {
        match errorcode.as_deref() {
            // A successful TRANSACTIONUPDATE means the void was accepted.
            Some("0") => AttemptStatus::Voided,
            _ => AttemptStatus::VoidFailed,
        }
    }
}

domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Trustpayments<T>,
    flow: SetupMandate,
    source: Option<(String, Option<trustpayments::TrustpaymentsSettleStatus>, Option<String>)>,
    context: (),
    params: [source, _ctx],
    success: _ => [Charged],
    failure: none,
    extractors: {
        request: SetupMandateRequestData<T>,
        response: TrustpaymentsSetupMandateResponse,
        source: |response| {
            response.responses.first().map(|auth_response| {
                (
                    auth_response.errorcode.clone(),
                    auth_response.settlestatus.clone(),
                    auth_response.authcode.clone(),
                )
            })
        },
        context: |_request, _response| (),
    },
    {
        let Some((errorcode, settlestatus, authcode)) = source else {
            return AttemptStatus::Failure;
        };
        if errorcode != "0" {
            return AttemptStatus::Failure;
        }
        match settlestatus {
            Some(trustpayments::TrustpaymentsSettleStatus::AutomaticCapture) => {
                if authcode.is_some() {
                    AttemptStatus::Charged
                } else {
                    AttemptStatus::Pending
                }
            }
            Some(trustpayments::TrustpaymentsSettleStatus::SettledPending)
            | Some(trustpayments::TrustpaymentsSettleStatus::SettledComplete) => {
                AttemptStatus::Charged
            }
            Some(trustpayments::TrustpaymentsSettleStatus::ManualCapture) => {
                if authcode.is_some() {
                    AttemptStatus::Authorized
                } else {
                    AttemptStatus::Pending
                }
            }
            Some(trustpayments::TrustpaymentsSettleStatus::Cancelled) => AttemptStatus::Voided,
            None => AttemptStatus::Pending,
        }
    }
}

domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Trustpayments<T>,
    flow: RepeatPayment,
    source: Option<(String, Option<trustpayments::TrustpaymentsSettleStatus>, Option<String>)>,
    context: (),
    params: [source, _ctx],
    success: _ => [Charged],
    failure: none,
    extractors: {
        request: RepeatPaymentData<T>,
        response: TrustpaymentsRepeatPaymentResponse,
        source: |response| {
            response.responses.first().map(|auth_response| {
                (
                    auth_response.errorcode.clone(),
                    auth_response.settlestatus.clone(),
                    auth_response.authcode.clone(),
                )
            })
        },
        context: |_request, _response| (),
    },
    {
        let Some((errorcode, settlestatus, authcode)) = source else {
            return AttemptStatus::Failure;
        };
        if errorcode != "0" {
            return AttemptStatus::Failure;
        }
        match settlestatus {
            Some(trustpayments::TrustpaymentsSettleStatus::AutomaticCapture) => {
                if authcode.is_some() {
                    AttemptStatus::Charged
                } else {
                    AttemptStatus::Pending
                }
            }
            Some(trustpayments::TrustpaymentsSettleStatus::SettledPending)
            | Some(trustpayments::TrustpaymentsSettleStatus::SettledComplete) => {
                AttemptStatus::Charged
            }
            Some(trustpayments::TrustpaymentsSettleStatus::ManualCapture) => {
                if authcode.is_some() {
                    AttemptStatus::Authorized
                } else {
                    AttemptStatus::Pending
                }
            }
            Some(trustpayments::TrustpaymentsSettleStatus::Cancelled) => AttemptStatus::Voided,
            None => AttemptStatus::Pending,
        }
    }
}

domain_types::impl_refund_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Trustpayments<T>,
    flow: Refund,
    source: Option<(String, Option<trustpayments::TrustpaymentsSettleStatus>)>,
    context: (),
    params: [source, _ctx],
    success: _ => [Success],
    failure: none,
    extractors: {
        request: RefundsData,
        response: TrustpaymentsRefundResponse,
        source: |response| {
            response.responses.first().map(|refund_response| {
                (
                    refund_response.errorcode.clone(),
                    refund_response.settlestatus.clone(),
                )
            })
        },
        context: |_request, _response| (),
    },
    {
        let Some((errorcode, settlestatus)) = source else {
            return RefundStatus::Pending;
        };
        if errorcode != "0" {
            return RefundStatus::Failure;
        }
        match settlestatus {
            Some(trustpayments::TrustpaymentsSettleStatus::SettledComplete)
            | Some(trustpayments::TrustpaymentsSettleStatus::SettledPending) => {
                RefundStatus::Success
            }
            Some(trustpayments::TrustpaymentsSettleStatus::AutomaticCapture) | None => {
                RefundStatus::Pending
            }
            Some(trustpayments::TrustpaymentsSettleStatus::ManualCapture) => {
                RefundStatus::ManualReview
            }
            Some(trustpayments::TrustpaymentsSettleStatus::Cancelled) => RefundStatus::Failure,
        }
    }
}

domain_types::impl_refund_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Trustpayments<T>,
    flow: RSync,
    // `(response-level errorcode, first record)`; `None` on an empty response array.
    source: Option<(
        String,
        Option<(String, Option<trustpayments::TrustpaymentsSettleStatus>)>,
    )>,
    context: (),
    params: [source, _ctx],
    success: _ => [Success],
    failure: none,
    extractors: {
        request: RefundSyncData,
        response: TrustpaymentsRSyncResponse,
        source: |response| {
            response.response.first().map(|response_item| {
                let record = response_item.records.as_ref().and_then(|records| {
                    records
                        .first()
                        .map(|record| (record.errorcode.clone(), record.settlestatus.clone()))
                });
                (response_item.errorcode.clone(), record)
            })
        },
        context: |_request, _response| (),
    },
    {
        let Some((response_errorcode, record)) = source else {
            return RefundStatus::Pending;
        };
        if response_errorcode != "0" {
            return RefundStatus::Failure;
        }
        let Some((record_errorcode, settlestatus)) = record else {
            // No transaction records found for the refund query yet.
            return RefundStatus::Pending;
        };
        if record_errorcode != "0" {
            return RefundStatus::Failure;
        }
        match settlestatus {
            Some(trustpayments::TrustpaymentsSettleStatus::SettledComplete)
            | Some(trustpayments::TrustpaymentsSettleStatus::SettledPending) => {
                RefundStatus::Success
            }
            Some(trustpayments::TrustpaymentsSettleStatus::AutomaticCapture) | None => {
                RefundStatus::Pending
            }
            Some(trustpayments::TrustpaymentsSettleStatus::ManualCapture) => {
                RefundStatus::ManualReview
            }
            Some(trustpayments::TrustpaymentsSettleStatus::Cancelled) => RefundStatus::Failure,
        }
    }
}

macros::macro_connector_flow_status_impls!(
    connector: Trustpayments,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        VoidPC,
        MandateRevoke,
        CreateOrder,
        PaymentMethodToken,
    ],
    not_supported: [
        VoidPostRefund,
        ServerSessionAuthenticationToken,
        ClientAuthenticationToken,
        Accept,
        DefendDispute,
        SubmitEvidence,
        ServerAuthenticationToken,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        CreateConnectorCustomer,
        GetConnectorCustomer,
    ],
);
