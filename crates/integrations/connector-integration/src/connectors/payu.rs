pub mod transformers;

use std::fmt::Debug;

use base64::Engine;
use common_enums::{enums, CurrencyUnit};
use common_utils::{
    errors::CustomResult, events, ext_traits::ByteSliceExt, types::StringMajorUnit,
};
use domain_types::{
    connector_flow::{
        Accept, Authenticate, Authorize, Capture, CreateAccessToken, CreateConnectorCustomer,
        CreateOrder, CreateSessionToken, DefendDispute, IncrementalAuthorization, MandateRevoke,
        PSync, PaymentMethodToken, PostAuthenticate, PreAuthenticate, RSync, Refund, RepeatPayment,
        SdkSessionToken, SetupMandate, SubmitEvidence, Void, VoidPC,
    },
    connector_types::{
        AcceptDisputeData, AccessTokenRequestData, AccessTokenResponseData, ConnectorCustomerData,
        ConnectorCustomerResponse, DisputeDefendData, DisputeFlowData, DisputeResponseData,
        MandateRevokeRequestData, MandateRevokeResponseData, PaymentCreateOrderData,
        PaymentCreateOrderResponse, PaymentFlowData, PaymentMethodTokenResponse,
        PaymentMethodTokenizationData, PaymentVoidData, PaymentsAuthenticateData,
        PaymentsAuthorizeData, PaymentsCancelPostCaptureData, PaymentsCaptureData,
        PaymentsIncrementalAuthorizationData, PaymentsPostAuthenticateData,
        PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSdkSessionTokenData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        RepeatPaymentData, SessionTokenRequestData, SessionTokenResponseData,
        SetupMandateRequestData, SubmitEvidenceData,
    },
    errors::{self, ConnectorError},
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::Maskable;
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;

pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

use transformers::{
    is_upi_collect_flow, PayuAuthType, PayuPaymentRequest, PayuPaymentResponse, PayuSyncRequest,
    PayuSyncResponse,
};

use super::macros;
use crate::types::ResponseRouterData;

// Trait implementations with generic type parameters

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    > for Payu<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Payu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Payu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Payu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSessionToken for Payu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAccessToken for Payu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for Payu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Payu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Payu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Payu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Payu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Payu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::AcceptDispute for Payu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SubmitEvidenceV2 for Payu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::DisputeDefend for Payu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Payu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Payu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Payu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Payu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Payu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentIncrementalAuthorization for Payu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Payu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Payu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Payu<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::MandateRevokeV2 for Payu<T>
{
}

// Authentication trait implementations
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Payu<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Payu<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Payu<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SdkSessionTokenV2 for Payu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for Payu<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for Payu<T>
{
}

// Set up connector using macros with all framework integrations
macros::create_all_prerequisites!(
    connector_name: Payu,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: PayuPaymentRequest,
            response_body: PayuPaymentResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: PayuSyncRequest,
            response_body: PayuSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: StringMajorUnit
    ],
    member_functions: {

        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            _req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, ConnectorError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            Ok(vec![
                ("Content-Type".to_string(), "application/x-www-form-urlencoded".into()),
                ("Accept".to_string(), "application/json".into()),
            ])
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.payu.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.payu.base_url
        }

        pub fn preprocess_response_bytes<F, FCD, Res>(
            &self,
            req: &RouterDataV2<F, FCD, PaymentsAuthorizeData<T>, Res>,
            bytes: bytes::Bytes,
        ) -> CustomResult<bytes::Bytes, ConnectorError> {
            if is_upi_collect_flow(&req.request) {
                // For UPI collect flows, we need to return base64 decoded response
                let decoded_value = BASE64_ENGINE.decode(bytes.clone());
                match decoded_value {
                    Ok(decoded_bytes) => Ok(decoded_bytes.into()),
                    Err(_) => Ok(bytes.clone())
                }
            } else {
                // For other flows, we can use the response itself
                Ok(bytes)
            }
        }
    }
);

// Implement PSync flow using macro framework
macros::macro_connector_implementation!(
    connector_default_implementations: [],
    connector: Payu,
    curl_request: FormUrlEncoded(PayuSyncRequest),
    curl_response: PayuSyncResponse,
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
            _req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, ConnectorError> {
            Ok(vec![
                ("Content-Type".to_string(), "application/x-www-form-urlencoded".into()),
                ("Accept".to_string(), "application/json".into()),
            ])
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, ConnectorError> {
            // Based on Haskell implementation: uses /merchant/postservice.php?form=2 for verification
            // Test: https://test.payu.in/merchant/postservice.php?form=2
            let base_url = self.base_url(&req.resource_common_data.connectors);
            Ok(format!("{base_url}/merchant/postservice.php?form=2"))
        }

        fn get_content_type(&self) -> &'static str {
            "application/x-www-form-urlencoded"
        }

        fn get_error_response_v2(
            &self,
            res: Response,
            _event_builder: Option<&mut events::Event>,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            // PayU sync may return error responses in different formats
            let response: PayuSyncResponse = res
                .response
                .parse_struct("PayU Sync ErrorResponse")
                .change_context(ConnectorError::ResponseDeserializationFailed)?;

            // Check if PayU returned error status (0 = error)
            if response.status == Some(0) {
                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: "PAYU_SYNC_ERROR".to_string(),
                    message: response.msg.unwrap_or_default(),
                    reason: None,
                    attempt_status: Some(enums::AttemptStatus::Failure),
                    connector_transaction_id: None,
                    network_error_message: None,
                    network_advice_code: None,
                    network_decline_code: None,
                })
            } else {
                // Generic error response
                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: "SYNC_UNKNOWN_ERROR".to_string(),
                    message: "Unknown PayU sync error".to_string(),
                    reason: None,
                    attempt_status: Some(enums::AttemptStatus::Failure),
                    connector_transaction_id: None,
                    network_error_message: None,
                    network_advice_code: None,
                    network_decline_code: None,
                })
            }
        }
    }
);

// Implement authorize flow using macro framework
macros::macro_connector_implementation!(
    connector_default_implementations: [],
    connector: Payu,
    curl_request: FormUrlEncoded(PayuPaymentRequest),
    curl_response: PayuPaymentResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, ConnectorError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, ConnectorError> {
            // Based on Haskell Endpoints.hs: uses /_payment endpoint for UPI transactions
            // Test: https://test.payu.in/_payment
            // Prod: https://secure.payu.in/_payment
            let base_url = self.base_url(&req.resource_common_data.connectors);
            Ok(format!("{base_url}/_payment"))
        }
        fn get_content_type(&self) -> &'static str {
            "application/x-www-form-urlencoded"
        }
        fn get_error_response_v2(
            &self,
            res: Response,
            _event_builder: Option<&mut events::Event>,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            // PayU returns error responses in the same JSON format as success responses
            // We need to parse the response and check for error fields
            let response: PayuPaymentResponse = res
                .response
                .parse_struct("PayU ErrorResponse")
                        .change_context(ConnectorError::ResponseDeserializationFailed)?;

            // Check if this is an error response
            if response.error.is_some() {
                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: response.error.unwrap_or_default(),
                    message: response.message.unwrap_or_default(),
                    reason: None,
                    attempt_status: Some(enums::AttemptStatus::Failure),
                    connector_transaction_id: response.reference_id,
                    network_error_message: None,
                    network_advice_code: None,
                    network_decline_code: None,
                })
            } else {
                // This shouldn't happen as successful responses go through normal flow
                // But fallback to generic error
                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: "UNKNOWN_ERROR".to_string(),
                    message: "Unknown PayU error".to_string(),
                    reason: None,
                    attempt_status: Some(enums::AttemptStatus::Failure),
                    connector_transaction_id: None,
                    network_error_message: None,
                    network_advice_code: None,
                    network_decline_code: None,
                })
            }
        }
    }
);

// Implement ConnectorCommon trait
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Payu<T>
{
    fn id(&self) -> &'static str {
        "payu"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.payu.base_url
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, ConnectorError> {
        let _auth = PayuAuthType::try_from(auth_type)?;
        // Payu uses form-based authentication, not headers
        Ok(vec![])
    }
}

// Connector integration implementations for unsupported flows (stubs)
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize + Serialize>
    ConnectorIntegrationV2<Refund, RefundFlowData, RefundsData, RefundsResponseData> for Payu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize + Serialize>
    ConnectorIntegrationV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData> for Payu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize + Serialize>
    ConnectorIntegrationV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
    for Payu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize + Serialize>
    ConnectorIntegrationV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
    for Payu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize + Serialize>
    ConnectorIntegrationV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    > for Payu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize + Serialize>
    ConnectorIntegrationV2<
        RepeatPayment,
        PaymentFlowData,
        RepeatPaymentData<T>,
        PaymentsResponseData,
    > for Payu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize + Serialize>
    ConnectorIntegrationV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>
    for Payu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize + Serialize>
    ConnectorIntegrationV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>
    for Payu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize + Serialize>
    ConnectorIntegrationV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>
    for Payu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize + Serialize>
    ConnectorIntegrationV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    > for Payu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    > for Payu<T>
{
}

// Add stub implementation for CreateSessionToken
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize + Serialize>
    ConnectorIntegrationV2<
        CreateSessionToken,
        PaymentFlowData,
        SessionTokenRequestData,
        SessionTokenResponseData,
    > for Payu<T>
{
}

// Add stub implementation for CreateAccessToken
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize + Serialize>
    ConnectorIntegrationV2<
        CreateAccessToken,
        PaymentFlowData,
        AccessTokenRequestData,
        AccessTokenResponseData,
    > for Payu<T>
{
}

// Add stub implementation for CreateConnectorCustomer
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    > for Payu<T>
{
}

// Authentication flow implementations
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    > for Payu<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    > for Payu<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    > for Payu<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        SdkSessionToken,
        PaymentFlowData,
        PaymentsSdkSessionTokenData,
        PaymentsResponseData,
    > for Payu<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        MandateRevokeResponseData,
    > for Payu<T>
{
}
