pub mod transformers;
pub use transformers as axisbank;

use self::transformers::{
    extract_merchant_identifiers_from_metadata, AxisbankAuthConfig, AxisbankPaymentsRequest,
    AxisbankPaymentsResponse, AxisbankRefundRequest, AxisbankRefundResponse,
    AxisbankRefundSyncRequest, AxisbankRefundSyncResponse, AxisbankSyncRequest,
    AxisbankSyncResponse,
};
use super::macros;
use crate::types::ResponseRouterData;
use common_enums as enums;
use common_utils::{errors::CustomResult, events, ext_traits::BytesExt, types::StringMajorUnit};
use domain_types::errors::{ConnectorError, IntegrationError, IntegrationErrorContext};
use domain_types::{
    connector_flow::{Authorize, PSync, RSync, Refund},
    connector_types::{
        ConnectorSpecifications, PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
    },
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
use tracing::error;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Axisbank<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    SourceVerification for Axisbank<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Axisbank<T>
{
}

// Authorize (Register Intent): the response is a UPI deeplink, so any non-failure
// outer code with a payload lands on AuthenticationPending — `handle_authorize_response`
// never yields Charged/Authorized here. Failure outer codes short-circuit to Failure.
// Success (deeplink built) is represented by AuthenticationPending; RequestPending
// (no payload yet) is the canonical "still starting" path.
// Authorize macro intentionally omitted: axisbank is UPI-collect. The Authorize
// TryFrom only ever emits AuthenticationPending (redirect) or Pending — neither
// is in Authorize::TERMINAL_SUCCESS_SET, so no `success:` is declareable. The
// authorized-or-charged outcome arrives via PSync / webhook, same as absa_sanlam.
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Axisbank<T>
{
}

// PSync (Status 360): mirror of `map_transaction_status` — `Success` is
// disambiguated by the gateway_response_code in the payload (ctx).
domain_types::impl_flow_status_mapping_ctx! {
    generics:        [T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
    connector:       Axisbank<T>,
    flow:            PSync,
    source:          crate::connectors::juspay_upi_stack::types::OuterResponseCode,
    context:         transformers::AxisbankSyncCtx,
    params:          [status, ctx],
    success_status:  Success,
    success_targets: [Charged],
    failure_status:  Failure,
    failure_target:  Failure,
    {
        use common_enums::AttemptStatus;
        use crate::connectors::juspay_upi_stack::types::{
            GatewayResponseCode, OuterResponseCode as Outer,
        };
        match status {
            // handle_psync_response short-circuits is_failure() codes to Failure —
            // that set includes DuplicateRequest (unlike map_transaction_status).
            Outer::Failure
            | Outer::RequestExpired
            | Outer::Dropout
            | Outer::InvalidData
            | Outer::Unauthorized
            | Outer::InvalidMerchant
            | Outer::DeviceFingerprintMismatch
            | Outer::InternalServerError
            | Outer::InvalidTransactionId
            | Outer::UninitiatedRequest
            | Outer::InvalidRefundAmount
            | Outer::DuplicateRequest
            | Outer::BadRequest => AttemptStatus::Failure,
            Outer::RequestNotFound
            | Outer::RequestPending
            | Outer::ServiceUnavailable
            | Outer::GatewayTimeout => AttemptStatus::Pending,
            Outer::Success => match ctx.gateway_response_code.as_deref() {
                Some(code) => match GatewayResponseCode::parse(code) {
                    GatewayResponseCode::Success => AttemptStatus::Charged,
                    GatewayResponseCode::Pending
                    | GatewayResponseCode::Deemed
                    | GatewayResponseCode::MandatePaused
                    | GatewayResponseCode::MandateCompleted => AttemptStatus::Pending,
                    GatewayResponseCode::Declined
                    | GatewayResponseCode::Expired
                    | GatewayResponseCode::BeneAddrIncorrect
                    | GatewayResponseCode::IntentExpired
                    | GatewayResponseCode::ValidationError
                    | GatewayResponseCode::MandateRevoked
                    | GatewayResponseCode::MandateDeclined
                    | GatewayResponseCode::MandateExpired
                    | GatewayResponseCode::Unknown(_) => AttemptStatus::Failure,
                },
                None => AttemptStatus::Pending,
            },
        }
    }
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Axisbank<T>
{
}

// Refund / RSync route through the shared juspay_upi_stack handlers
// (`handle_refund_response` / `handle_rsync_response`), which map the gateway
// codes to the shared `juspay_upi_stack::types::RefundStatus` and then to
// `enums::RefundStatus` (`Deemed` → Pending). The contextual refund_type
// (UDIR vs ONLINE/OFFLINE) split lives upstream of that enum inside
// `map_refund_status`, so the declared mapping here covers the enum →
// `RefundStatus` leg that both flows share.
domain_types::impl_refund_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
    connector: Axisbank<T>,
    flow:      Refund,
    source:    crate::connectors::juspay_upi_stack::types::RefundStatus,
    success:   Success => Success,
    failure:   Failed  => Failure,
    {
        Pending => Pending,
        Deemed  => Pending,
    }
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Axisbank<T>
{
}

domain_types::impl_refund_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
    connector: Axisbank<T>,
    flow:      RSync,
    source:    crate::connectors::juspay_upi_stack::types::RefundStatus,
    success:   Success => Success,
    failure:   Failed  => Failure,
    {
        Pending => Pending,
        Deemed  => Pending,
    }
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Axisbank<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Axisbank,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize]
);

macros::create_amount_converter_wrapper!(
    connector_name: Axisbank,
    amount_type: StringMajorUnit
);

macros::create_all_prerequisites!(
    connector_name: Axisbank,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: AxisbankPaymentsRequest,
            response_body: AxisbankPaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: AxisbankSyncRequest,
            response_body: AxisbankSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: AxisbankRefundRequest,
            response_body: AxisbankRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            request_body: AxisbankRefundSyncRequest,
            response_body: AxisbankRefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: StringMajorUnit
    ],
    member_functions: {
        /// Preprocess JWE-encrypted responses from Axis Bank.
        ///
        /// Delegates to the shared Juspay UPI Stack preprocessing function.
        /// All banks in the Juspay UPI Merchant Stack family (Axis, YES, Kotak, RBL, AU)
        /// share the same JWE/JWS handling pipeline.
        pub fn preprocess_response_bytes<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
            response_bytes: bytes::Bytes,
            _status_code: u16,
        ) -> Result<bytes::Bytes, ConnectorError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            use domain_types::errors::ResponseTransformationErrorContext;

            let auth_config = AxisbankAuthConfig::try_from(&req.connector_config)
                .map_err(|e| {
                    error!(error = %e, "Could not extract Axisbank auth config");
                    ConnectorError::ResponseDeserializationFailed {
                        context: ResponseTransformationErrorContext {
                            http_status_code: None,
                            additional_context: Some(format!("Failed to extract AxisbankAuthConfig from connector_config. Verify all required fields are present: merchant_id, merchant_channel_id, merchant_kid, juspay_kid, merchant_private_key, juspay_public_key. See documentation: {}/docs/transactions", crate::connectors::juspay_upi_stack::constants::DOC_URL_BASE)),
                        },
                    }
                })?;

            crate::connectors::juspay_upi_stack::crypto::preprocess_jwe_response(
                response_bytes,
                &auth_config.merchant_private_key,
            )
        }

        pub fn connector_base_url<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.axisbank.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.axisbank.base_url
        }

        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            _req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            Ok(vec![
                ("content-type".to_string(), "application/json".to_string().into()),
            ])
        }
    }
);

// Authorize Flow - Register Intent
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Axisbank,
    curl_request: Json(AxisbankPaymentsRequest),
    curl_response: AxisbankPaymentsResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let (merchant_id, merchant_channel_id) =
                extract_merchant_identifiers_from_metadata(&req.request.metadata)?;
            let merchant_request_id = req.resource_common_data.connector_request_reference_id.clone();
            crate::connectors::juspay_upi_stack::transformers::build_request_headers(
                &merchant_id,
                &merchant_channel_id,
                &merchant_request_id,
            )
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url(req);
            Ok(format!("{}merchants/transactions/registerIntent", base_url))
        }
    }
);

// PSync Flow - Status 360
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Axisbank,
    curl_request: Json(AxisbankSyncRequest),
    curl_response: AxisbankSyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let (merchant_id, merchant_channel_id) =
                extract_merchant_identifiers_from_metadata(&req.resource_common_data.connector_feature_data)?;
            let merchant_request_id = req
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .map_err(|_| IntegrationError::MissingRequiredField {
                    field_name: "connector_transaction_id",
                    context: IntegrationErrorContext {
                        suggested_action: Some("connector_transaction_id must be set before calling PSync".to_string()),
                        doc_url: Some(crate::connectors::juspay_upi_stack::constants::DOC_URL_TRANSACTION_STATUS_360.to_string()),
                        additional_context: Some("PSync requires the merchantRequestId returned from Register Intent. Ensure the payment was initialized successfully before querying status.".to_string()),
                    },
                })?;

            crate::connectors::juspay_upi_stack::transformers::build_request_headers(
                &merchant_id,
                &merchant_channel_id,
                &merchant_request_id,
            )
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url(req);
            Ok(format!("{}merchants/transactions/status360", base_url))
        }
    }
);

// Refund Flow - Refund 360
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Axisbank,
    curl_request: Json(AxisbankRefundRequest),
    curl_response: AxisbankRefundResponse,
    flow_name: Refund,
    resource_common_data: RefundFlowData,
    flow_request: RefundsData,
    flow_response: RefundsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let (merchant_id, merchant_channel_id) =
                extract_merchant_identifiers_from_metadata(&req.resource_common_data.connector_feature_data)?;

            let refund_request_id = req.request.refund_id.clone();

            crate::connectors::juspay_upi_stack::transformers::build_request_headers(
                &merchant_id,
                &merchant_channel_id,
                &refund_request_id,
            )
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_refunds(req);
            Ok(format!("{}merchants/transactions/refund360", base_url))
        }
    }
);

// RSync Flow - Refund Status (uses same endpoint as Refund)
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Axisbank,
    curl_request: Json(AxisbankRefundSyncRequest),
    curl_response: AxisbankRefundSyncResponse,
    flow_name: RSync,
    resource_common_data: RefundFlowData,
    flow_request: RefundSyncData,
    flow_response: RefundsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let (merchant_id, merchant_channel_id) =
                extract_merchant_identifiers_from_metadata(&req.resource_common_data.connector_feature_data)?;

            let refund_request_id = req.request.connector_refund_id.clone();

            crate::connectors::juspay_upi_stack::transformers::build_request_headers(
                &merchant_id,
                &merchant_channel_id,
                &refund_request_id,
            )
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_refunds(req);
            Ok(format!("{}merchants/transactions/refund360", base_url))
        }
    }
);

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorCommon for Axisbank<T>
{
    fn id(&self) -> &'static str {
        "axisbank"
    }

    fn get_currency_unit(&self) -> enums::CurrencyUnit {
        enums::CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn get_auth_header(
        &self,
        _auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        Ok(vec![])
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.axisbank.base_url.as_ref()
    }

    fn build_error_response(
        &self,
        res: Response,
        _event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let error_response = if let Ok(error) = res
            .response
            .parse_struct::<axisbank::AxisbankErrorResponse>("Axisbank ErrorResponse")
        {
            let typed =
                macros::serialize_typed_connector_payload(&error, "typed_connector_response");
            let mut resp = axisbank::build_error_response(
                res.status_code,
                &error.response_code,
                &error.response_message,
            );
            resp.typed_connector_response = typed;
            resp
        } else {
            let raw_response = String::from_utf8_lossy(&res.response);
            axisbank::build_error_response(res.status_code, "UNKNOWN", &raw_response)
        };

        Ok(error_response)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorSpecifications for Axisbank<T>
{
    fn get_supported_payment_methods(
        &self,
    ) -> Option<&'static domain_types::types::SupportedPaymentMethods> {
        None
    }

    fn get_supported_webhook_flows(&self) -> Option<&'static [enums::EventClass]> {
        None
    }
}

macros::macro_connector_flow_status_impls!(
    connector: Axisbank,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        Capture,
        Void,
        SetupMandate,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        RepeatPayment,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        CreateOrder,
        PaymentMethodToken,
        Accept,
        DefendDispute,
        SubmitEvidence,
        MandateRevoke,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        ClientAuthenticationToken,
        VoidPC,
        IncrementalAuthorization,
    ],
    not_supported: [
        VoidPostRefund,
    ],
);
