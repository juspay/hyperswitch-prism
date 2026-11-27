pub mod transformers;

use crate::{connectors::macros, types::ResponseRouterData, with_error_response_body};
use common_enums::CurrencyUnit;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::CustomResult,
    events,
};
use domain_types::{
    connector_flow::{Authorize, PSync},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData,
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
    PaynamicsAuthorizeRequest, PaynamicsPaymentResponse, PaynamicsSyncRequest,
    PaynamicsSyncResponse,
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
        VoidPostRefund,
        RepeatPayment,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        SetupMandate,
        SubmitEvidence
    ],
    not_supported: [Capture, Refund, RSync, VoidPC],
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
    for (method, subtype) in [(
        common_enums::PaymentMethod::BankTransfer,
        common_enums::PaymentMethodType::LocalBankTransfer,
    )] {
        methods.add(
            method,
            subtype,
            PaymentMethodDetails {
                mandates: FeatureStatus::NotSupported,
                refunds: FeatureStatus::NotSupported,
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
