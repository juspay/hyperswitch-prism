pub mod transformers;

use std::{fmt::Debug, sync::LazyLock};

use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt, types::StringMajorUnit};
use domain_types::{
    connector_flow::{Authorize, PSync, RSync, Refund},
        connector_types::{
        ConnectorSpecifications, ConnectorWebhookSecrets, EventType, PaymentFlowData, PaymentsAuthorizeData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, RequestDetails, ResponseId, SupportedPaymentMethodsExt,
    },
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::{
        self, Connectors, ConnectorInfo, FeatureStatus, PaymentMethodDetails,
        SupportedPaymentMethods,
    },
};
use error_stack::ResultExt;
use hyperswitch_masking::Maskable;
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{self, ValidationTrait},
    decode::BodyDecoding,
    verification::SourceVerification,
};
use serde::Serialize;

use transformers::{
    AsiapayDirectPayRequest, AsiapayDirectPayResponse, AsiapayPSyncRequest,
    AsiapayPSyncResponse, AsiapayRefundRequest, AsiapayRefundResponse, AsiapayRSyncRequest,
    AsiapayRSyncResponse,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};
use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

// ===== CONNECTOR SERVICE TRAIT IMPLEMENTATIONS =====

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Asiapay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Asiapay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Asiapay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Asiapay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Asiapay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Asiapay<T>
{
    fn get_webhook_source_verification_signature(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<domain_types::errors::WebhookError>> {
        let body: transformers::AsiapayWebhookBody = request.body
            .parse_struct("AsiapayWebhookBody")
            .change_context(domain_types::errors::WebhookError::WebhookBodyDecodingFailed)?;

        let signature = body
            .secure_hash
            .unwrap_or_default()
            .to_lowercase()
            .into_bytes();
        Ok(signature)
    }

    fn get_webhook_source_verification_message(
        &self,
        request: &RequestDetails,
        connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<domain_types::errors::WebhookError>> {
        let body: transformers::AsiapayWebhookBody = request.body
            .parse_struct("AsiapayWebhookBody")
            .change_context(domain_types::errors::WebhookError::WebhookBodyDecodingFailed)?;

        let secret = hyperswitch_masking::Secret::new(
            String::from_utf8_lossy(&connector_webhook_secret.secret).to_string()
        );
        let computed = transformers::compute_asiapay_webhook_hash(&body, &secret)
            .change_context(domain_types::errors::WebhookError::WebhookSourceVerificationFailed)?;

        Ok(computed.into_bytes())
    }

    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<domain_types::errors::WebhookError>> {
        let connector_webhook_secret = match connector_webhook_secret {
            Some(secret) => secret,
            None => {
                return Err(error_stack::report!(
                    domain_types::errors::WebhookError::WebhookVerificationSecretNotFound
                ));
            }
        };

        let incoming_signature = self
            .get_webhook_source_verification_signature(&request, &connector_webhook_secret)?;

        let computed_message = self
            .get_webhook_source_verification_message(&request, &connector_webhook_secret)?;

        // Constant-time comparison to prevent timing attacks on webhook signature.
        #[allow(deprecated)]
        Ok(ring::constant_time::verify_slices_are_equal(
            &incoming_signature,
            &computed_message,
        )
        .is_ok())
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<domain_types::errors::WebhookError>> {
        let body: transformers::AsiapayWebhookBody = request.body
            .parse_struct("AsiapayWebhookBody")
            .change_context(domain_types::errors::WebhookError::WebhookBodyDecodingFailed)?;

        transformers::map_asiapay_webhook_event_type(&body)
            .change_context(domain_types::errors::WebhookError::WebhookBodyDecodingFailed)
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<domain_types::connector_types::EventContext>,
    ) -> Result<domain_types::connector_types::WebhookDetailsResponse, error_stack::Report<domain_types::errors::WebhookError>> {
        let body_str = String::from_utf8_lossy(&request.body);
        let body: transformers::AsiapayWebhookBody = request.body
            .parse_struct("AsiapayWebhookBody")
            .change_context(domain_types::errors::WebhookError::WebhookBodyDecodingFailed)?;

        let status = transformers::map_order_status(
            body.order_status.as_deref().unwrap_or("Pending")
        );

        let (error_code, error_message) = if status == common_enums::AttemptStatus::Failure {
            (body.prc.clone(), body.src.clone())
        } else {
            (None, None)
        };

        // Convert base-unit amount to minor units using the numeric currency code.
        let (amount_captured, minor_amount_captured) =
            match (&body.amt, &body.cur) {
                (Some(amt_str), Some(cur_code)) => {
                    match transformers::get_currency_from_asiapay_code(cur_code) {
                        Some(currency) => match currency.to_currency_lower_unit(amt_str.clone()) {
                            Ok(lower) => match lower.parse::<i64>() {
                                Ok(minor) => (Some(minor), Some(common_utils::types::MinorUnit::new(minor))),
                                Err(e) => {
                                    tracing::warn!("AsiaPay webhook: failed to parse amount '{}' to i64: {}", amt_str, e);
                                    (None, None)
                                }
                            },
                            Err(e) => {
                                tracing::warn!("AsiaPay webhook: failed to convert amount '{}' to lower unit for currency {:?}: {:?}", amt_str, currency, e);
                                (None, None)
                            }
                        },
                        None => {
                            tracing::warn!("AsiaPay webhook: unknown currency code '{}'", cur_code);
                            (None, None)
                        }
                    }
                }
                _ => (None, None),
            };

        Ok(domain_types::connector_types::WebhookDetailsResponse {
            resource_id: body.pay_ref.map(ResponseId::ConnectorTransactionId),
            status,
            connector_response_reference_id: body.order_ref.clone(),
            mandate_reference: None,
            error_code,
            error_message,
            error_reason: None,
            raw_connector_response: Some(body_str.to_string()),
            status_code: 200,
            response_headers: None,
            amount_captured,
            minor_amount_captured,
            network_txn_id: None,
            payment_method_update: None,
            sender_payment_instrument_id: None,
        })
    }

    fn process_refund_webhook(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<domain_types::connector_types::RefundWebhookDetailsResponse, error_stack::Report<domain_types::errors::WebhookError>> {
        // AsiaPay does not send refund-specific webhooks.
        // All refund status updates are retrieved via polling (RSync flow).
        Err(error_stack::report!(domain_types::errors::WebhookError::WebhooksNotImplemented {
            operation: "process_refund_webhook",
        }))
    }

    fn process_dispute_webhook(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<
        domain_types::connector_types::DisputeWebhookDetailsResponse,
        error_stack::Report<domain_types::errors::WebhookError>,
    > {
        // AsiaPay does not send dispute/chargeback webhooks.
        // Disputes are managed via the merchant portal, not webhooks.
        Err(error_stack::report!(domain_types::errors::WebhookError::WebhooksNotImplemented {
            operation: "process_dispute_webhook",
        }))
    }

    fn get_webhook_resource_object(
        &self,
        request: RequestDetails,
    ) -> Result<Box<dyn hyperswitch_masking::ErasedMaskSerialize>, error_stack::Report<domain_types::errors::WebhookError>> {
        let body: transformers::AsiapayWebhookBody = request.body
            .parse_struct("AsiapayWebhookBody")
            .change_context(domain_types::errors::WebhookError::WebhookBodyDecodingFailed)?;
        Ok(Box::new(body))
    }

    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"prc=0&src=0&Ord=12345678&Ref=ORD_TEST_001&PayRef=13907042&successcode=0&Amt=1.00&Cur=702&Holder=SURAJ+KUMAR&AuthId=907042&AlertCode=&remark=&eci=07&payerAuth=U&sourceIp=65.1.52.128&ipCountry=IN&payMethod=VISA&TxTime=2026-06-09+22%3A46%3A06.0&panFirst4=4333&panLast4=0011&cardIssuingCountry=HK&channelType=DPS&MerchantId=12109740&secureHash=abc123"#
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Asiapay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Asiapay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Asiapay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ValidationTrait
    for Asiapay<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Asiapay,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

// ===== CONNECTOR COMMON IMPLEMENTATION =====

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Asiapay<T>
{
    fn id(&self) -> &'static str {
        "asiapay"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/x-www-form-urlencoded"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.asiapay.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        _auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        // AsiaPay authenticates via request body parameters (merchantId, loginId, password)
        // rather than HTTP headers. No Authorization/Basic Auth headers are sent.
        Ok(vec![])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: transformers::AsiapayErrorResponse = if res.response.is_empty() {
            transformers::AsiapayErrorResponse::default()
        } else {
            res.response
                .parse_struct("AsiapayErrorResponse")
                .change_context(
                    crate::utils::response_deserialization_fail(
                        res.status_code,
                        "asiapay: response body did not match the expected format; confirm API version and connector documentation.",
                    ),
                )?
        };

        with_error_response_body!(event_builder, response);

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response
                .success_code
                .clone()
                .unwrap_or_else(|| res.status_code.to_string()),
            message: response.get_error_message(),
            reason: Some(
                std::str::from_utf8(&res.response)
                    .change_context(
                        crate::utils::response_deserialization_fail(
                            res.status_code,
                            "asiapay: response body did not match the expected format; confirm API version and connector documentation.",
                        ),
                    )?
                    .to_owned(),
            ),
            attempt_status: None,
            connector_transaction_id: None,
            network_decline_code: response.prc,
            network_advice_code: None,
            network_error_message: response.src,
        })
    }
}

// ===== NORMALIZE ASIAPAY FIELD NAMES =====
/// AsiaPay returns response fields in inconsistent casing (lowercase, camelCase, PascalCase).
/// This function normalizes all known field names to camelCase so serde aliases match.
fn normalize_asiapay_field_names(
    map: std::collections::HashMap<String, String>,
) -> std::collections::HashMap<String, String> {
    let mut normalized = std::collections::HashMap::new();
    for (key, value) in map {
        let normalized_key = match key.as_str() {
            "successcode" => {
                normalized.insert("resultCode".to_string(), value.clone());
                "successCode".to_string()
            }
            "payref" => "payRef".to_string(),
            "orderref" => "orderRef".to_string(),
            "errmsg" => "errMsg".to_string(),
            "orderstatus" => "orderStatus".to_string(),
            "authid" => "authId".to_string(),
            "authdate" => "authDate".to_string(),
            "capturedate" => "captureDate".to_string(),
            "batchid" => "batchId".to_string(),
            "settledate" => "settleDate".to_string(),
            "merref" => "merRef".to_string(),
            "merrequestamt" => "merRequestAmt".to_string(),
            "bankmid" => "bankMid".to_string(),
            "settleflag" => "settleFlag".to_string(),
            "bankref" => "bankRef".to_string(),
            "traceno" => "traceNo".to_string(),
            "accountno" => "accountNo".to_string(),
            "originalamt" => "originalAmt".to_string(),
            "txtime" => "txTime".to_string(),
            "resultcode" => "resultCode".to_string(),
            _ => key,
        };
        normalized.insert(normalized_key, value);
    }
    normalized
}

// ===== PREPROCESS RESPONSE BYTES =====
fn preprocess_xml_response(response_str: &str) -> CustomResult<bytes::Bytes, ConnectorError> {
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct XmlRecords {
        record: Vec<std::collections::HashMap<String, String>>,
    }

    let records: XmlRecords = quick_xml::de::from_str(response_str).change_context(
        ConnectorError::ResponseDeserializationFailed {
            context: Default::default(),
        },
    )?;

    let first_record = records.record.into_iter().next()
        .ok_or_else(|| ConnectorError::ResponseDeserializationFailed {
            context: domain_types::errors::ResponseTransformationErrorContext {
                additional_context: Some("AsiaPay XML response contained no records".to_string()),
                ..Default::default()
            },
        })?;

    let normalized = normalize_asiapay_field_names(first_record);

    let json = serde_json::to_vec(&normalized).change_context(
        ConnectorError::ResponseDeserializationFailed {
            context: Default::default(),
        },
    )?;

    Ok(bytes::Bytes::from(json))
}

// ===== CREATE ALL PREREQUISITES =====

macros::create_all_prerequisites!(
    connector_name: Asiapay,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: AsiapayDirectPayRequest,
            response_body: AsiapayDirectPayResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: AsiapayRefundRequest,
            response_body: AsiapayRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: PSync,
            request_body: AsiapayPSyncRequest,
            response_body: AsiapayPSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: RSync,
            request_body: AsiapayRSyncRequest,
            response_body: AsiapayRSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: StringMajorUnit
    ],
    member_functions: {
        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.asiapay.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.asiapay.base_url
        }

        pub fn preprocess_response_bytes<F, FCD, Req, Res>(
            &self,
            _req: &RouterDataV2<F, FCD, Req, Res>,
            bytes: bytes::Bytes,
            _status_code: u16,
        ) -> CustomResult<bytes::Bytes, ConnectorError> {
            let response_str = String::from_utf8_lossy(&bytes);
            let trimmed = response_str.trim();

            if trimmed.starts_with("<?xml") || trimmed.starts_with("<records") {
                return preprocess_xml_response(trimmed);
            }

            let parsed: std::collections::HashMap<String, String> =
                serde_qs::from_str(&response_str).change_context(
                    ConnectorError::ResponseDeserializationFailed {
                        context: Default::default(),
                    },
                )?;

            let normalized = normalize_asiapay_field_names(parsed);

            let json = serde_json::to_vec(&normalized).change_context(
                ConnectorError::ResponseDeserializationFailed {
                    context: Default::default(),
                },
            )?;

            Ok(bytes::Bytes::from(json))
        }
    }
);

// ===== AUTHORIZE FLOW =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Asiapay,
    curl_request: FormUrlEncoded(AsiapayDirectPayRequest),
    curl_response: AsiapayDirectPayResponse,
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
            _req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )])
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{}/directPay/payComp.jsp", base_url))
        }
    }
);

// ===== REFUND FLOW =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Asiapay,
    curl_request: FormUrlEncoded(AsiapayRefundRequest),
    curl_response: AsiapayRefundResponse,
    flow_name: Refund,
    resource_common_data: RefundFlowData,
    flow_request: RefundsData,
    flow_response: RefundsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            _req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )])
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_refunds(req);
            Ok(format!("{}/merchant/api/orderApi.jsp", base_url))
        }
    }
);

// ===== PSYNC FLOW =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Asiapay,
    curl_request: FormUrlEncoded(AsiapayPSyncRequest),
    curl_response: AsiapayPSyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            _req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )])
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{}/merchant/api/orderApi.jsp", base_url))
        }
    }
);

// ===== RSYNC FLOW =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Asiapay,
    curl_request: FormUrlEncoded(AsiapayRSyncRequest),
    curl_response: AsiapayRSyncResponse,
    flow_name: RSync,
    resource_common_data: RefundFlowData,
    flow_request: RefundSyncData,
    flow_response: RefundsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            _req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )])
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_refunds(req);
            Ok(format!("{}/merchant/api/orderApi.jsp", base_url))
        }
    }
);

// ===== CONNECTOR SPECIFICATIONS =====

static ASIAPAY_CONNECTOR_INFO: ConnectorInfo = ConnectorInfo {
    display_name: "AsiaPay",
    description: "AsiaPay is a leading payment service provider offering online payment solutions across Asia, supporting credit cards and various alternative payment methods.",
    connector_type: types::PaymentConnectorCategory::PaymentGateway,
};

// Supported webhook flows. Currently only payment webhooks are implemented.
static ASIAPAY_SUPPORTED_WEBHOOK_FLOWS: [common_enums::EventClass; 1] = [
    common_enums::EventClass::Payments,
];

static ASIAPAY_SUPPORTED_PAYMENT_METHODS: LazyLock<SupportedPaymentMethods> = LazyLock::new(|| {
    let supported_capture_methods = vec![
        common_enums::CaptureMethod::Automatic,
        common_enums::CaptureMethod::Manual,
    ];

    let mut methods = SupportedPaymentMethods::new();

    // Currently only card payments are implemented.
    // Wallets (Apple Pay, Google Pay, Alipay, etc.) will be implemented later.
    methods.add(
        common_enums::PaymentMethod::Card,
        common_enums::PaymentMethodType::Card,
        PaymentMethodDetails {
            mandates: FeatureStatus::Supported,
            refunds: FeatureStatus::Supported,
            supported_capture_methods,
            specific_features: None,
        },
    );

    methods
});

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorSpecifications for Asiapay<T>
{
    fn get_supported_payment_methods(
        &self,
    ) -> Option<&'static SupportedPaymentMethods> {
        Some(&*ASIAPAY_SUPPORTED_PAYMENT_METHODS)
    }

    fn get_supported_webhook_flows(&self) -> Option<&'static [common_enums::EventClass]> {
        Some(&ASIAPAY_SUPPORTED_WEBHOOK_FLOWS)
    }

    fn get_connector_about(&self) -> Option<&'static ConnectorInfo> {
        Some(&ASIAPAY_CONNECTOR_INFO)
    }
}

// ===== FLOW STATUS IMPLS =====
macros::macro_connector_flow_status_impls!(
    connector: Asiapay,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        SetupMandate,
        RepeatPayment,
        PaymentMethodToken,
        CreateConnectorCustomer,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        ServerSessionAuthenticationToken,
        ClientAuthenticationToken,
        ServerAuthenticationToken,
        MandateRevoke,
        Capture,
        Void
    ],
    not_supported: [
        IncrementalAuthorization,
        VoidPC,
        Accept,
        DefendDispute,
        SubmitEvidence,
        CreateOrder,
    ],
);
