pub mod transformers;

use crate::{connectors::macros, types::ResponseRouterData, with_error_response_body};
use common_enums::CurrencyUnit;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::CustomResult,
    events,
};
use domain_types::{
    connector_flow::{Authorize, PSync, Refund, VoidPC},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsCancelPostCaptureData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundsData, RefundsResponseData,
    },
    errors,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{Mask, Maskable};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding,
};
use serde::Serialize;
use std::fmt::Debug;
use transformers as paynamics;
use transformers::{
    PaynamicsAuthorizeRequest, PaynamicsPaymentResponse, PaynamicsRefundRequest,
    PaynamicsRefundResponse, PaynamicsSyncRequest, PaynamicsSyncResponse, PaynamicsVoidPCRequest, PaynamicsVoidPCResponse,
};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const ACCEPT: &str = "Accept";
    pub(crate) const IDEMPOTENCY_KEY: &str = "Idempotency-Key";
}
const JSON_CONTENT_TYPE: &str = "application/json";
const SALE_PATH: &str = "transactions/";
const QUERY_PATH: &str = "transactions/query";
const REFUND_PATH: &str = "transactions/refund";
const REVERSAL_PATH: &str = "transactions/reversal";
macros::create_amount_converter_wrapper!(connector_name: Paynamics, amount_type: StringMajorUnit);
macros::create_all_prerequisites!(
 connector_name: Paynamics,
 generic_type: T,
 api: [
    (
        flow: Authorize,
        request_body: PaynamicsAuthorizeRequest,
        response_body: PaynamicsPaymentResponse,
        router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    ),
    (
        flow: Refund,
        request_body: PaynamicsRefundRequest,
        response_body: PaynamicsRefundResponse,
        router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
    ),
    (
        flow: VoidPC,
        request_body: PaynamicsVoidPCRequest,
        response_body: PaynamicsVoidPCResponse,
        router_data: RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>,
    ),
    (
        flow: PSync,
        request_body: PaynamicsSyncRequest,
        response_body: PaynamicsSyncResponse,
        router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
    )
 ],
 amount_converters: [],
 member_functions: {
    pub fn build_headers<F, FCD, Req, Res>(&self, req: &RouterDataV2<F, FCD, Req, Res>) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
        let mut headers = vec![(headers::CONTENT_TYPE.to_string(), JSON_CONTENT_TYPE.to_string().into()), (headers::ACCEPT.to_string(), JSON_CONTENT_TYPE.to_string().into())];
        headers.extend(self.get_auth_header(&req.connector_config)?);
        Ok(headers)
    }
    pub fn connector_base_url_payments<'a, F, Req, Res>(&self, req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>) -> CustomResult<&'a str, errors::IntegrationError> {
        let base_url = req.resource_common_data.connectors.paynamics.base_url.as_ref();
        paynamics::validate_base_url(base_url)?;
        Ok(base_url)
    }
    pub fn connector_base_url_refunds<'a, F, Req, Res>(&self, req: &'a RouterDataV2<F, RefundFlowData, Req, Res>) -> CustomResult<&'a str, errors::IntegrationError> {
        let base_url = req.resource_common_data.connectors.paynamics.base_url.as_ref();
        paynamics::validate_base_url(base_url)?;
        Ok(base_url)
    }
 }
);
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Paynamics<T>
{
    fn id(&self) -> &'static str {
        "paynamics"
    }
    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }
    fn common_get_content_type(&self) -> &'static str {
        JSON_CONTENT_TYPE
    }
    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.paynamics.base_url.as_ref()
    }
    fn get_auth_header(
        &self,
        config: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
        let auth = paynamics::PaynamicsAuthType::try_from(config)?;
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            auth.generate_authorization_header().into_masked(),
        )])
    }
    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let response: paynamics::PaynamicsErrorResponse = serde_json::from_slice(&res.response)
            .change_context(crate::utils::response_deserialization_fail(
                res.status_code,
                "paynamics: invalid JSON error response",
            ))?;
        with_error_response_body!(event_builder, response);
        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response
                .response_code
                .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            message: response
                .response_message
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            reason: response.response_advise,
            ..Default::default()
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Paynamics<T>
{
}

// ===== CONNECTOR SERVICE TRAIT IMPLEMENTATION =====
// Aggregate trait - composes all other connector traits.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Paynamics<T>
{
}

// ===== BASE (NON-FLOW) TRAIT IMPLEMENTATIONS =====
// These are simple marker traits that are NOT flows and therefore have no arm
// in expand_flow_status_impl!. They must be impl'd manually.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Paynamics<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::webhooks::IncomingWebhook for Paynamics<T>
{
    fn verify_webhook_source(
        &self,
        request: domain_types::connector_types::RequestDetails,
        _secrets: Option<domain_types::connector_types::ConnectorWebhookSecrets>,
        config: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<errors::WebhookError>> {
        paynamics::verified_callback(&request.body, config.as_ref())?;
        Ok(true)
    }
    fn get_event_type(
        &self,
        request: domain_types::connector_types::RequestDetails,
    ) -> Result<domain_types::connector_types::EventType, error_stack::Report<errors::WebhookError>>
    {
        Ok(paynamics::callback_event(&paynamics::parse_callback(
            &request.body,
        )?))
    }
    fn get_webhook_event_reference(
        &self,
        request: domain_types::connector_types::RequestDetails,
    ) -> Result<
        Option<domain_types::connector_types::WebhookResourceReference>,
        error_stack::Report<errors::WebhookError>,
    > {
        use domain_types::connector_types::{PaymentWebhookReference, WebhookResourceReference};
        let response = paynamics::parse_callback(&request.body)?;
        if !paynamics::callback_event(&response).is_payment_event() {
            return Ok(None);
        }
        if response.response_id.is_none() && response.request_id.is_none() {
            return Err(errors::WebhookError::WebhookReferenceIdNotFound.into());
        }
        Ok(Some(WebhookResourceReference::Payment(
            PaymentWebhookReference {
                connector_transaction_id: response.response_id,
                merchant_transaction_id: response.request_id,
                connector_preprocessing_id: None,
            },
        )))
    }
    fn process_payment_webhook(
        &self,
        request: domain_types::connector_types::RequestDetails,
        _secrets: Option<domain_types::connector_types::ConnectorWebhookSecrets>,
        config: Option<ConnectorSpecificConfig>,
        _context: Option<domain_types::connector_types::EventContext>,
    ) -> Result<
        domain_types::connector_types::WebhookDetailsResponse,
        error_stack::Report<errors::WebhookError>,
    > {
        use domain_types::connector_types::{
            RawConnectorStatus, ResponseId, WebhookDetailsResponse,
        };
        let response = paynamics::verified_callback(&request.body, config.as_ref())?;
        if !paynamics::callback_event(&response).is_payment_event() {
            return Err(errors::WebhookError::WebhookEventTypeNotFound.into());
        }
        let status = common_enums::AttemptStatus::from(response.code());
        let failed = domain_types::utils::is_payment_failure(status);
        // Retain wire fields including the exact timestamp as masked debug data.
        // Unsigned instructions are masked and never become actionable outputs.
        let raw = hyperswitch_masking::masked_serialize(&response)
            .and_then(|value| serde_json::to_string(&value))
            .map_err(|_| errors::WebhookError::WebhookResponseEncodingFailed)?;
        // EventContext contains no stored status or identifiers. The caller must
        // reconcile IDs and lexical timestamp, deduplicate and prevent regression.
        Ok(WebhookDetailsResponse {
            raw_connector_status: Some(RawConnectorStatus {
                code: Some(response.response_code.clone()),
                message: response.response_message.clone(),
                reason: response.response_advise.clone(),
            }),
            amount: None,
            resource_id: response
                .response_id
                .clone()
                .map(ResponseId::ConnectorTransactionId),
            status,
            connector_response_reference_id: response.response_id,
            connector_request_reference_id: response.request_id,
            mandate_reference: None,
            error_code: failed.then_some(response.response_code),
            error_message: if failed {
                response.response_message
            } else {
                None
            },
            error_reason: if failed {
                response.response_advise
            } else {
                None
            },
            raw_connector_response: Some(raw),
            status_code: 200,
            response_headers: None,
            captured_money: None,
            network_txn_id: None,
            payment_method_update: None,
            sender_payment_instrument_id: None,
            connector_returned_payment_method_details: None,
        })
    }
    fn get_webhook_resource_object(
        &self,
        request: domain_types::connector_types::RequestDetails,
    ) -> Result<
        Box<dyn hyperswitch_masking::ErasedMaskSerialize>,
        error_stack::Report<errors::WebhookError>,
    > {
        paynamics::parse_callback(&request.body)?;
        Ok(Box::new(hyperswitch_masking::Secret::<String>::new(
            String::from_utf8_lossy(&request.body).into_owned(),
        )))
    }
    fn sample_webhook_body(&self) -> &'static [u8] {
        // Fictional structural probe, not a captured or certified provider callback.
        br#"{"merchant_id":"probe-merchant","request_id":"probe-request","response_id":"probe-response","response_code":"GR033","response_message":"Pending","timestamp":"2026-01-01T00:00:00.000+08:00","signature":"00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"}"#
    }
    // Retain framework's default 200 acknowledgment; provider requirements and
    // actual callback delivery/content type have not been empirically verified.
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Paynamics<T>
{
}

// ===== SOURCE VERIFICATION IMPLEMENTATION =====
// `VerifyRedirectResponse: SourceVerification + BodyDecoding`
// (crates/types-traits/interfaces/src/connector_types.rs), so both supertraits
// must be impl'd or the VerifyRedirectResponse impl above does not compile.
// SourceVerification is emitted here; BodyDecoding already comes from
// template-generation/connector.rs.template - do NOT emit a second one here,
// two impls of the same trait for the same type do not compile.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::verification::SourceVerification for Paynamics<T>
{
}

// ===== PAYOUT TRAIT IMPLEMENTATIONS =====
// Emits payout marker-trait impls and default no-op ConnectorIntegrationV2
// impls for all PayoutXxxV2 flows.
macros::macro_connector_payout_implementation!(
    connector: Paynamics,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

// ===== FLOW STATUS IMPLEMENTATIONS =====
// Emits marker-trait impls AND stub ConnectorIntegrationV2 impls for every
// flow listed. Each stub's get_url returns
// IntegrationError::connector_flow_not_implemented(...).
macros::macro_connector_flow_status_impls!(
    connector: Paynamics,
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
        IncrementalAuthorization,
        CreateOrder,
        PostAuthenticate,
        PreAuthenticate,
        PaymentMethodToken,
        Void,
        RSync,
        VoidPostRefund,
        RepeatPayment,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        SetupMandate,
        SubmitEvidence
    ],
    not_supported: [Capture],
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Paynamics<T>
{
}
macros::macro_connector_implementation!(
 connector_default_implementations: [get_content_type, get_error_response_v2],
 connector: Paynamics, curl_request: Json(PaynamicsAuthorizeRequest),
 curl_response: PaynamicsPaymentResponse, flow_name: Authorize,
 resource_common_data: PaymentFlowData, flow_request: PaymentsAuthorizeData<T>, flow_response: PaymentsResponseData,
 http_method: Post, generic_type: T,
 [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
 other_functions: {
 fn get_headers(&self, req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> { let mut headers = self.build_headers(req)?; // Appendix B FR046/FR047; no guaranteed replay contract.
 headers.push((headers::IDEMPOTENCY_KEY.to_string(), paynamics::sale_request_id(&req.resource_common_data)?.into())); Ok(headers) }
 fn get_url(&self, req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>) -> CustomResult<String, errors::IntegrationError> { Ok(format!("{}{SALE_PATH}", self.connector_base_url_payments(req)?)) }
 }
);

static PAYNAMICS_SUPPORTED_METHODS: std::sync::LazyLock<
    domain_types::types::SupportedPaymentMethods,
> = std::sync::LazyLock::new(|| {
    use domain_types::{
        connector_types::SupportedPaymentMethodsExt,
        types::{FeatureStatus, PaymentMethodDetails, SupportedPaymentMethods},
    };
    let mut methods = SupportedPaymentMethods::new();
    for (method, subtype) in [
        (
            common_enums::PaymentMethod::BankTransfer,
            common_enums::PaymentMethodType::LocalBankTransfer,
        ),
        (
            common_enums::PaymentMethod::Voucher,
            common_enums::PaymentMethodType::Dragonpay,
        ),
    ] {
        methods.add(
            method,
            subtype,
            PaymentMethodDetails {
                mandates: FeatureStatus::NotSupported,
                refunds: FeatureStatus::Supported,
                supported_capture_methods: vec![common_enums::CaptureMethod::Automatic],
                specific_features: None,
            },
        );
    }
    methods
});
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    domain_types::connector_types::ConnectorSpecifications for Paynamics<T>
{
    fn get_supported_payment_methods(
        &self,
    ) -> Option<&'static domain_types::types::SupportedPaymentMethods> {
        Some(&PAYNAMICS_SUPPORTED_METHODS)
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Paynamics<T>
{
    fn validate_psync_reference_id(
        &self,
        data: &PaymentsSyncData,
        flow: &PaymentFlowData,
    ) -> CustomResult<(), errors::IntegrationError> {
        let metadata = paynamics::sync_metadata(data)?;
        paynamics::query_request_id(flow, &metadata.original_sale_request_id)?;
        Ok(())
    }
}
macros::macro_connector_implementation!(
 connector_default_implementations: [get_content_type, get_error_response_v2],
 connector: Paynamics, curl_request: Json(PaynamicsSyncRequest),
 curl_response: PaynamicsSyncResponse, flow_name: PSync,
 resource_common_data: PaymentFlowData, flow_request: PaymentsSyncData, flow_response: PaymentsResponseData,
 http_method: Post, generic_type: T,
 [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
 other_functions: {
 fn get_headers(&self, req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> { let mut headers = self.build_headers(req)?; let metadata = paynamics::sync_metadata(&req.request)?;
 // Same pure operation ID helper used by query body, never original target ID.
 headers.push((headers::IDEMPOTENCY_KEY.to_string(), paynamics::query_request_id(&req.resource_common_data, &metadata.original_sale_request_id)?.into())); Ok(headers) }
 fn get_url(&self, req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>) -> CustomResult<String, errors::IntegrationError> { Ok(format!("{}{QUERY_PATH}", self.connector_base_url_payments(req)?)) }
 }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Paynamics<T>
{
}
macros::macro_connector_implementation!(
 connector_default_implementations: [get_content_type, get_error_response_v2],
 connector: Paynamics, curl_request: Json(PaynamicsRefundRequest),
 curl_response: PaynamicsRefundResponse, flow_name: Refund,
 resource_common_data: RefundFlowData, flow_request: RefundsData, flow_response: RefundsResponseData,
 http_method: Post, generic_type: T,
 [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
 other_functions: {
 fn get_headers(&self, req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
 let mut headers = self.build_headers(req)?;
 paynamics::validate_request_id(&req.request.refund_id)?;
 headers.push((headers::IDEMPOTENCY_KEY.to_string(), req.request.refund_id.clone().into())); Ok(headers)
 }
 fn get_url(&self, req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>) -> CustomResult<String, errors::IntegrationError> { Ok(format!("{}{REFUND_PATH}", self.connector_base_url_refunds(req)?)) }
 }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for Paynamics<T>
{
}
macros::macro_connector_implementation!(
 connector_default_implementations: [get_content_type, get_error_response_v2],
 connector: Paynamics, curl_request: Json(PaynamicsVoidPCRequest),
 curl_response: PaynamicsVoidPCResponse, flow_name: VoidPC,
 resource_common_data: PaymentFlowData, flow_request: PaymentsCancelPostCaptureData, flow_response: PaymentsResponseData,
 http_method: Post, generic_type: T,
 [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
 other_functions: {
 fn get_headers(&self, req: &RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
 let mut headers = self.build_headers(req)?;
 headers.push((headers::IDEMPOTENCY_KEY.to_string(), paynamics::sale_request_id(&req.resource_common_data)?.into())); Ok(headers)
 }
 fn get_url(&self, req: &RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>) -> CustomResult<String, errors::IntegrationError> {
 Ok(format!("{}{REVERSAL_PATH}", self.connector_base_url_payments(req)?))
 }
 }
);
