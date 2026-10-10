pub mod transformers;

use std::{fmt::Debug, sync::LazyLock};

use base64::Engine;
use common_enums::{AttemptStatus, CurrencyUnit};
use common_utils::{
    consts::{BASE64_ENGINE, NO_ERROR_CODE, NO_ERROR_MESSAGE},
    crypto::{self, SignMessage},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    types::FloatMajorUnit,
};
use domain_types::{
    connector_flow::{Authorize, PSync, RSync, Refund},
    connector_types::{
        ConnectorWebhookSecrets, EventContext, EventType, PaymentFlowData, PaymentWebhookReference,
        PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundWebhookDetailsResponse, RefundsData, RefundsResponseData,
        RequestDetails, ResponseId, SupportedPaymentMethodsExt, WebhookDetailsResponse,
        WebhookResourceReference,
    },
    errors,
    payment_method_data::{DefaultPCIHolder, PaymentMethodDataTypes},
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::{
        ConnectorInfo, Connectors, FeatureStatus, PaymentConnectorCategory, PaymentMethodDetails,
        SupportedPaymentMethods,
    },
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Mask, Maskable};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding,
};
use serde::Serialize;
use transformers::{
    self as betterpayment, BetterpaymentAuthorizeRequest, BetterpaymentAuthorizeResponse,
    BetterpaymentPSyncResponse, BetterpaymentRSyncResponse, BetterpaymentRefundRequest,
    BetterpaymentRefundResponse, BetterpaymentWebhookBody,
};

use crate::{connectors::macros, types::ResponseRouterData, with_error_response_body};

pub(crate) mod headers {
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

macros::create_amount_converter_wrapper!(connector_name: Betterpayment, amount_type: FloatMajorUnit);

static BETTERPAYMENT_CONNECTOR_INFO: ConnectorInfo = ConnectorInfo {
    display_name: "Betterpayment",
    description: "Deutsche Bank Group payment gateway, processing Wero digital wallet payments",
    connector_type: PaymentConnectorCategory::PaymentGateway,
};

static BETTERPAYMENT_SUPPORTED_PAYMENT_METHODS: LazyLock<SupportedPaymentMethods> =
    LazyLock::new(|| {
        let mut methods = SupportedPaymentMethods::new();
        methods.add(
            common_enums::PaymentMethod::Wallet,
            common_enums::PaymentMethodType::Wero,
            PaymentMethodDetails {
                mandates: FeatureStatus::NotSupported,
                refunds: FeatureStatus::Supported,
                supported_capture_methods: vec![common_enums::CaptureMethod::Automatic],
                specific_features: None,
            },
        );
        methods
    });

macros::create_all_prerequisites!(
    connector_name: Betterpayment,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: BetterpaymentAuthorizeRequest<T>,
            response_body: BetterpaymentAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: BetterpaymentPSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: BetterpaymentRefundRequest,
            response_body: BetterpaymentRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: BetterpaymentRSyncResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            let mut header = vec![(
                "Content-Type".to_string(),
                self.get_content_type().to_string().into(),
            )];
            let mut auth_header = self.get_auth_header(&req.connector_config)?;
            header.append(&mut auth_header);
            Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            req.resource_common_data
                .connectors
                .betterpayment
                .base_url
                .trim_end_matches('/')
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            req.resource_common_data
                .connectors
                .betterpayment
                .base_url
                .trim_end_matches('/')
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Betterpayment<T>
{
    fn id(&self) -> &'static str {
        "betterpayment"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        // Betterpayment uses FloatMajorUnit (e.g. 10.00), which is a major/base unit.
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.betterpayment.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
        let auth = betterpayment::BetterpaymentAuthType::try_from(auth_type).change_context(
            errors::IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        // Betterpayment uses HTTP Basic Auth: `Authorization: Basic base64(api_key:key1)`.
        // `.into_masked()` (the `Mask` trait), NOT `.into()`: `impl From<T> for
        // Maskable<T>` builds `Maskable::Normal`, which logs the credentials in clear.
        let credentials = format!("{}:{}", auth.api_key.expose(), auth.key1.expose());
        let encoded = BASE64_ENGINE.encode(credentials);
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            format!("Basic {encoded}").into_masked(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let response: betterpayment::BetterpaymentErrorResponse = res
            .response
            .parse_struct("BetterpaymentErrorResponse")
            .change_context(crate::utils::response_deserialization_fail(
                res.status_code,
                "betterpayment: error body did not match the documented error shape",
            ))?;

        with_error_response_body!(event_builder, response);

        // HTTP errors are shared across flows; do not overwrite the payment/refund status.
        let attempt_status: Option<FlowStatus> = None;

        Ok(ErrorResponse {
            status_code: res.status_code,
            // Never `.unwrap_or_default()` here: a missing code/message must use
            // the NO_ERROR_CODE / NO_ERROR_MESSAGE sentinels, not an empty string.
            code: response
                .error_code
                .map(|c| c.to_string())
                .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            message: response
                .message
                .clone()
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            reason: response.message.clone(),
            attempt_status,
            typed_connector_response: macros::serialize_typed_connector_payload(
                &response,
                "typed_connector_response",
            ),
            ..Default::default()
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Betterpayment<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Betterpayment<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Betterpayment<T>
{
}

impl domain_types::connector_types::ConnectorSpecifications for Betterpayment<DefaultPCIHolder> {
    fn get_connector_about(&self) -> Option<&'static ConnectorInfo> {
        Some(&BETTERPAYMENT_CONNECTOR_INFO)
    }

    fn get_supported_payment_methods(&self) -> Option<&'static SupportedPaymentMethods> {
        Some(&BETTERPAYMENT_SUPPORTED_PAYMENT_METHODS)
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Betterpayment<T>
{
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<errors::WebhookError>> {
        let body: BetterpaymentWebhookBody = request
            .body
            .parse_struct("BetterpaymentWebhookBody")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;
        Ok(betterpayment::get_webhook_event_type(&body))
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<errors::WebhookError>> {
        let body: BetterpaymentWebhookBody = request
            .body
            .parse_struct("BetterpaymentWebhookBody")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        if let Some(refund_id) = body.refund_id {
            return Ok(Some(WebhookResourceReference::Refund(
                domain_types::connector_types::RefundWebhookReference {
                    connector_refund_id: Some(refund_id),
                    merchant_refund_id: None,
                    connector_transaction_id: Some(body.transaction_id),
                    merchant_transaction_id: Some(body.order_id),
                },
            )));
        }

        Ok(Some(WebhookResourceReference::Payment(
            PaymentWebhookReference {
                connector_transaction_id: Some(body.transaction_id),
                merchant_transaction_id: Some(body.order_id),
            },
        )))
    }

    fn get_webhook_source_verification_signature(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<errors::WebhookError>> {
        request
            .headers
            .get("gateway-webhook-signature")
            .ok_or_else(|| error_stack::report!(errors::WebhookError::WebhookSignatureNotFound))
            .map(|s| s.as_bytes().to_vec())
    }

    fn get_webhook_source_verification_message(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<errors::WebhookError>> {
        Ok(request.body.clone())
    }

    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<errors::WebhookError>> {
        let connector_config = connector_account_details.ok_or_else(|| {
            error_stack::report!(errors::WebhookError::WebhookVerificationSecretNotFound)
        })?;

        let auth = transformers::BetterpaymentAuthType::try_from(&connector_config)
            .change_context(errors::WebhookError::WebhookSourceVerificationFailed)?;

        let webhook_secret = ConnectorWebhookSecrets {
            secret: auth.api_secret.expose().into_bytes(),
            additional_secret: None,
        };

        let signature =
            self.get_webhook_source_verification_signature(&request, &webhook_secret)?;
        let message = self.get_webhook_source_verification_message(&request, &webhook_secret)?;

        let computed = crypto::HmacSha256
            .sign_message(&webhook_secret.secret, &message)
            .change_context(errors::WebhookError::WebhookSourceVerificationFailed)?;

        let computed_b64 = BASE64_ENGINE.encode(&computed);
        let incoming = std::str::from_utf8(&signature)
            .change_context(errors::WebhookError::WebhookSourceVerificationFailed)?;

        Ok(computed_b64 == incoming)
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<errors::WebhookError>> {
        let raw = request.body.clone();
        let body: BetterpaymentWebhookBody = request
            .body
            .parse_struct("BetterpaymentWebhookBody")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        let status = AttemptStatus::from(body.status.clone());

        let (error_code, error_message) = if status == AttemptStatus::Failure {
            (body.reason_code.clone(), body.message.clone())
        } else {
            (None, None)
        };

        Ok(WebhookDetailsResponse {
            connector_returned_payment_method_details: None,
            resource_id: Some(ResponseId::ConnectorTransactionId(
                body.transaction_id.clone(),
            )),
            status,
            connector_response_reference_id: Some(body.transaction_id),
            connector_request_reference_id: Some(body.order_id),
            mandate_reference: None,
            error_code,
            error_message,
            error_reason: None,
            raw_connector_response: Some(String::from_utf8_lossy(&raw).to_string()),
            status_code: 200,
            response_headers: None,
            amount_captured: None,
            minor_amount_captured: None,
            network_txn_id: None,
            payment_method_update: None,
            sender_payment_instrument_id: None,
        })
    }

    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<errors::WebhookError>> {
        let raw = request.body.clone();
        let body: BetterpaymentWebhookBody = request
            .body
            .parse_struct("BetterpaymentWebhookBody")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        let status = betterpayment::get_refund_webhook_status(body.refund_status_code);

        let (error_code, error_message) = if status == common_enums::RefundStatus::Failure {
            (body.refund_reason_code.clone(), body.message.clone())
        } else {
            (None, None)
        };

        Ok(RefundWebhookDetailsResponse {
            connector_refund_id: body.refund_id.clone(),
            merchant_transaction_id: Some(body.order_id),
            status,
            connector_response_reference_id: body.refund_id,
            error_code,
            error_message,
            raw_connector_response: Some(String::from_utf8_lossy(&raw).to_string()),
            status_code: 200,
            response_headers: None,
        })
    }

    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"{"transaction_id":"bp_test123","payment_type":"wero","status_code":0,"status":"completed","order_id":"order_abc","amount":10.00,"currency":"EUR","checksum":"dummy_checksum"}"#
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Betterpayment<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::verification::SourceVerification for Betterpayment<T>
{
}

crate::connectors::macros::macro_connector_payout_implementation!(
    connector: Betterpayment,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Betterpayment<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Betterpayment,
    curl_request: Json(BetterpaymentAuthorizeRequest),
    curl_response: BetterpaymentAuthorizeResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}/rest/payment"))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Betterpayment<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Betterpayment,
    curl_response: BetterpaymentPSyncResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.get_auth_header(&req.connector_config)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let transaction_id = req.request.get_connector_transaction_id()?;
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}/rest/transactions/{transaction_id}"))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Betterpayment<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Betterpayment,
    curl_request: Json(BetterpaymentRefundRequest),
    curl_response: BetterpaymentRefundResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = self.connector_base_url_refunds(req);
            Ok(format!("{base_url}/rest/refund"))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Betterpayment<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Betterpayment,
    curl_response: BetterpaymentRSyncResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.get_auth_header(&req.connector_config)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let transaction_id = &req.request.connector_transaction_id;
            let base_url = self.connector_base_url_refunds(req);
            Ok(format!("{base_url}/rest/transactions/{transaction_id}/refunds"))
        }
    }
);

crate::connectors::macros::macro_connector_flow_status_impls!(
    connector: Betterpayment,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        Accept,
        ClientAuthenticationToken,
        CreateConnectorCustomer,
        DefendDispute,
        GetConnectorCustomer,
        MandateRevoke,
        Authenticate,
        Capture,
        IncrementalAuthorization,
        CreateOrder,
        PostAuthenticate,
        PreAuthenticate,
        PaymentMethodToken,
        VoidPC,
        Void,
        VoidPostRefund,
        RepeatPayment,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        SetupMandate,
        SubmitEvidence
    ],
);
