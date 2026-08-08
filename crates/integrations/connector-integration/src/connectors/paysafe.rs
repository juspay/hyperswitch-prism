pub mod requests;
pub mod responses;
pub mod transformers;

use base64::Engine;
use common_enums::{CurrencyUnit, PaymentMethod, PaymentMethodType};
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt};
use domain_types::{
    connector_flow::{
        Authenticate, Authorize, Capture, CreateConnectorCustomer, PSync, PaymentMethodToken,
        PreAuthenticate, RSync, Refund, RepeatPayment, Void,
    },
    connector_types::{
        ConnectorCustomerData, ConnectorCustomerResponse, PaymentFlowData,
        PaymentMethodTokenResponse, PaymentMethodTokenizationData, PaymentVoidData,
        PaymentsAuthenticateData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
    },
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use std::fmt::Debug;
use transformers::{
    self as paysafe, PaysafeAuthenticateRequest, PaysafeAuthenticateResponse,
    PaysafeAuthorizeRequest, PaysafeAuthorizeResponse, PaysafeCaptureRequest,
    PaysafeCaptureResponse, PaysafeCustomerRequest, PaysafeCustomerResponse, PaysafeErrorResponse,
    PaysafePaymentMethodTokenRequest, PaysafePaymentMethodTokenResponse,
    PaysafePreAuthenticateRequest, PaysafePreAuthenticateResponse, PaysafeRSyncResponse,
    PaysafeRefundRequest, PaysafeRefundResponse, PaysafeRepeatPaymentRequest,
    PaysafeRepeatPaymentResponse, PaysafeSyncResponse, PaysafeVoidRequest, PaysafeVoidResponse,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};
use domain_types::errors::ConnectorError;
use domain_types::errors::{IntegrationError, IntegrationErrorContext};

pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Paysafe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Paysafe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Paysafe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Paysafe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Paysafe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Paysafe<T>
{
}
macros::macro_connector_payout_implementation!(
    connector: Paysafe,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Paysafe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Paysafe<T>
{
    /// No auth_type here; the no-3DS restriction is enforced in the Authorize dispatch.
    fn should_do_payment_method_token(
        &self,
        _payment_method: PaymentMethod,
        _payment_method_type: Option<PaymentMethodType>,
    ) -> bool {
        true
    }

    /// Card + 3DS: PreAuthenticate mints the `threeDs` handle (ACS challenge); Authenticate re-fetches
    /// the PAYABLE handle for its `paymentHandleToken`; the main Authorize settles it.
    /// Any other payment method / auth type goes straight to Authorize.
    fn next_authentication_step(
        &self,
        auth_type: common_enums::AuthenticationType,
        payment_method: PaymentMethod,
        redirect_state: connector_types::RedirectState,
        completed_step: Option<connector_types::AuthenticationStep>,
    ) -> connector_types::AuthenticationStep {
        use connector_types::{AuthenticationStep, RedirectState};

        if auth_type == common_enums::AuthenticationType::ThreeDs
            && payment_method == PaymentMethod::Card
        {
            match (redirect_state, completed_step) {
                (RedirectState::InitialRequest, _) => AuthenticationStep::PreAuthenticate,
                // Shopper returned from the ACS: re-fetch the handle token, then settle.
                (
                    RedirectState::RedirectWithParams | RedirectState::RedirectWithoutParams,
                    None,
                ) => AuthenticationStep::Authenticate,
                (
                    RedirectState::RedirectWithParams | RedirectState::RedirectWithoutParams,
                    Some(AuthenticationStep::Authenticate),
                ) => AuthenticationStep::Authorize,
                _ => AuthenticationStep::Authorize,
            }
        } else {
            AuthenticationStep::Authorize
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Paysafe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Paysafe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Paysafe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for Paysafe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Paysafe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Paysafe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Paysafe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Paysafe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Paysafe<T>
{
}

pub(crate) mod headers {
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

macros::create_all_prerequisites!(
    connector_name: Paysafe,
    generic_type: T,
    api: [
        (
            flow: CreateConnectorCustomer,
            request_body: PaysafeCustomerRequest,
            response_body: PaysafeCustomerResponse,
            router_data: RouterDataV2<CreateConnectorCustomer, PaymentFlowData, ConnectorCustomerData, ConnectorCustomerResponse>,
        ),
        (
            flow: PaymentMethodToken,
            request_body: PaysafePaymentMethodTokenRequest<T>,
            response_body: PaysafePaymentMethodTokenResponse,
            router_data: RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>,
        ),
        (
            flow: Authorize,
            request_body: PaysafeAuthorizeRequest<T>,
            response_body: PaysafeAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PreAuthenticate,
            request_body: PaysafePreAuthenticateRequest<T>,
            response_body: PaysafePreAuthenticateResponse,
            router_data: RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: Authenticate,
            request_body: PaysafeAuthenticateRequest,
            response_body: PaysafeAuthenticateResponse,
            router_data: RouterDataV2<Authenticate, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: PaysafeSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: PaysafeCaptureRequest,
            response_body: PaysafeCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: PaysafeVoidRequest,
            response_body: PaysafeVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: PaysafeRefundRequest,
            response_body: PaysafeRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: PaysafeRSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: PaysafeRepeatPaymentRequest,
            response_body: PaysafeRepeatPaymentResponse,
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
                Self::common_get_content_type(self).to_string().into(),
            )];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);
            Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.paysafe.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.paysafe.base_url
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Paysafe<T>
{
    fn id(&self) -> &'static str {
        "paysafe"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.paysafe.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = paysafe::PaysafeAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Paysafe Basic auth needs username/password from ConnectorSpecificConfig::Paysafe."
                            .to_string(),
                    ),
                    ..Default::default()
                },
            },
        )?;
        let auth_key = format!("{}:{}", auth.username.peek(), auth.password.peek());
        let auth_header = format!("Basic {}", BASE64_ENGINE.encode(auth_key));
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            auth_header.into_masked(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: PaysafeErrorResponse = res
            .response
            .parse_struct("PaysafeErrorResponse")
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "paysafe: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        with_error_response_body!(event_builder, response);

        let detail_message = response
            .error
            .details
            .as_ref()
            .and_then(|d| d.first().cloned());
        let field_error_message = response
            .error
            .field_errors
            .as_ref()
            .and_then(|f| f.first().map(|fe| fe.error.clone()));

        let reason = match (detail_message, field_error_message) {
            (Some(detail), Some(field)) => Some(format!("{detail}, {field}")),
            (Some(detail), None) => Some(detail),
            (None, Some(field)) => Some(field),
            (None, None) => Some(response.error.message.clone()),
        };

        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");
        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.error.code,
            message: response.error.message,
            reason,
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

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Paysafe,
    curl_request: Json(PaysafeCustomerRequest),
    curl_response: PaysafeCustomerResponse,
    flow_name: CreateConnectorCustomer,
    resource_common_data: PaymentFlowData,
    flow_request: ConnectorCustomerData,
    flow_response: ConnectorCustomerResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<CreateConnectorCustomer, PaymentFlowData, ConnectorCustomerData, ConnectorCustomerResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<CreateConnectorCustomer, PaymentFlowData, ConnectorCustomerData, ConnectorCustomerResponse>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}v1/customers", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Paysafe,
    curl_request: Json(PaysafePaymentMethodTokenRequest<T>),
    curl_response: PaysafePaymentMethodTokenResponse,
    flow_name: PaymentMethodToken,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentMethodTokenizationData<T>,
    flow_response: PaymentMethodTokenResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>,
        ) -> CustomResult<String, IntegrationError> {
            use domain_types::payment_method_data::{PaymentMethodData, WalletData};
            let base = self.connector_base_url_payments(req);
            match &req.request.payment_method_data {
                // Card-on-file recurring (CIT): when a Paysafe customer was created
                // upstream (its id is carried on the flow data), mint a reusable
                // MULTI_USE payment handle under the customer vault so the later MIT
                // (RepeatPayment) can replay it — mirrors hyperswitch's
                // v1/customers/{customerId}/paymenthandles. The Tokenize proto does not
                // carry setup_future_usage, so the presence of a connector_customer_id
                // is the CIT signal; one-off card payments (no customer) fall through to
                // the single-use v1/paymenthandles endpoint.
                PaymentMethodData::Card(_) => {
                    Ok(match req.resource_common_data.connector_customer.as_ref() {
                        Some(customer_id) => {
                            format!("{base}v1/customers/{customer_id}/paymenthandles")
                        }
                        None => format!("{base}v1/paymenthandles"),
                    })
                }
                // Apple Pay / Google Pay: the vault endpoint rejects raw wallet
                // payloads (5068 "CARD object must be present"), so wallet recurring
                // is two Tokenize legs — leg 1 mints a SINGLE_USE handle on the
                // standard endpoint; leg 2 (single-use handle echoed back via
                // connector_feature_data + a connector customer present) converts it
                // into a customer-vaulted MULTI_USE handle via paymentHandleTokenFrom.
                PaymentMethodData::Wallet(WalletData::ApplePay(_) | WalletData::GooglePay(_)) => {
                    Ok(
                        match (
                            req.resource_common_data.connector_customer.as_ref(),
                            transformers::paysafe_parse_feature_data_handle_token(
                                req.request.connector_feature_data.as_ref(),
                            )
                            .is_some(),
                        ) {
                            (Some(customer_id), true) => {
                                format!("{base}v1/customers/{customer_id}/paymenthandles")
                            }
                            _ => format!("{base}v1/paymenthandles"),
                        },
                    )
                }
                // Redirect wallets stay on the standard paymenthandles endpoint
                // (singleusepaymenthandles 5270s; no MIT replay for redirect rails).
                PaymentMethodData::Wallet(WalletData::Skrill(_)) => {
                    Ok(format!("{base}v1/paymenthandles"))
                }
                _ => Ok(format!("{base}v1/paymenthandles")),
            }
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Paysafe,
    curl_request: Json(PaysafeAuthorizeRequest<T>),
    curl_response: PaysafeAuthorizeResponse,
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
            // Leg 1 (redirect APMs, card + 3DS) creates a handle for the redirect link;
            // leg 2 / other methods settle the token via v1/payments.
            let endpoint = if paysafe::is_paysafe_handle_creation_leg(req) {
                "v1/paymenthandles"
            } else {
                "v1/payments"
            };
            Ok(format!("{}{}", self.connector_base_url_payments(req), endpoint))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Paysafe,
    curl_request: Json(PaysafePreAuthenticateRequest),
    curl_response: PaysafePreAuthenticateResponse,
    flow_name: PreAuthenticate,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsPreAuthenticateData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // Card + 3DS mints a `threeDs` handle (ACS challenge); settled on the follow-up Authorize.
            Ok(format!("{}v1/paymenthandles", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Paysafe,
    curl_request: Json(PaysafeAuthenticateRequest),
    curl_response: PaysafeAuthenticateResponse,
    flow_name: Authenticate,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthenticateData<T>,
    flow_response: PaymentsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Authenticate, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authenticate, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // Post-ACS re-fetch: read the PAYABLE handle's paymentHandleToken for the settle Authorize.
            // Query by connector_request_reference_id (the merchantRefNum PreAuthenticate used).
            let merchant_ref_num = &req.resource_common_data.connector_request_reference_id;
            Ok(format!(
                "{}v1/paymenthandles?merchantRefNum={merchant_ref_num}",
                self.connector_base_url_payments(req)
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Paysafe,
    curl_response: PaysafeSyncResponse,
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
            let base_url = self.connector_base_url_payments(req);

            // Always sync by merchantRefNum, never by connector id path. For redirect
            // APMs (Skrill/Interac/paysafecard) the `connector_transaction_id` recorded at
            // authorize is a payment-HANDLE id, and `GET /v1/payments/{handleId}` returns
            // 404 (error 5269) because no settled Payment exists under that id yet. Querying
            // `?merchantRefNum=` resolves the stable reference and returns 200 (empty list
            // until the handle is settled via CompleteAuthorize). Mirrors the hyperswitch
            // Paysafe connector's PSync.
            let connector_payment_id = req.resource_common_data.get_reference_id()?;
            let url = match req.request.connector_transaction_id.get_connector_transaction_id() {
                Ok(txn_id) if !txn_id.is_empty() => {
                    // Payment progressed past the handle: query the settled payment.
                    format!("{base_url}v1/payments?merchantRefNum={connector_payment_id}")
                }
                _ => {
                    // Before authorization completes there is no payment yet: sync the handle.
                    format!("{base_url}v1/paymenthandles?merchantRefNum={connector_payment_id}")
                }
            };

            Ok(url)
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Paysafe,
    curl_request: Json(PaysafeCaptureRequest),
    curl_response: PaysafeCaptureResponse,
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
            let connector_payment_id = req.request.connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Paysafe Capture targets v1/payments/{id}/settlements and needs the payment id returned by Authorize."
                                .to_string(),
                        ),
                        ..Default::default()
                    },
                })?;
            Ok(format!(
                "{}v1/payments/{}/settlements",
                self.connector_base_url_payments(req),
                connector_payment_id
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Paysafe,
    curl_request: Json(PaysafeVoidRequest),
    curl_response: PaysafeVoidResponse,
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
            let connector_payment_id = &req.request.connector_transaction_id;
            Ok(format!(
                "{}v1/payments/{}/voidauths",
                self.connector_base_url_payments(req),
                connector_payment_id
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Paysafe,
    curl_request: Json(PaysafeRefundRequest),
    curl_response: PaysafeRefundResponse,
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
            let connector_payment_id = req.request.connector_transaction_id.clone();
            Ok(format!(
                "{}v1/settlements/{}/refunds",
                self.connector_base_url_refunds(req),
                connector_payment_id
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Paysafe,
    curl_response: PaysafeRefundResponse,
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
            let connector_refund_id = &req.request.connector_refund_id;
            Ok(format!(
                "{}v1/refunds/{}",
                self.connector_base_url_refunds(req),
                connector_refund_id
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Paysafe,
    curl_request: Json(PaysafeRepeatPaymentRequest),
    curl_response: PaysafeRepeatPaymentResponse,
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
            Ok(format!("{}v1/payments", self.connector_base_url_payments(req)))
        }
    }
);

// SourceVerification implementations for PaymentMethodToken and PreAuthenticate

// SourceVerification implementations for unsupported flows

macros::macro_connector_flow_status_impls!(
    connector: Paysafe,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        IncrementalAuthorization,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        ClientAuthenticationToken,
        MandateRevoke,
        CreateOrder,
        SetupMandate,
        VoidPC,
        PostAuthenticate,
        GetConnectorCustomer,
    ],
    not_supported: [
        VoidPostRefund,
        Accept,
        DefendDispute,
        SubmitEvidence,
    ],
);
