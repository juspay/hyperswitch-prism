pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt, types::FloatMajorUnit};
use domain_types::{
    connector_flow::{
        FrmChargebackReceived, FrmPaymentOutcome, FrmRefundProcessed, PostRiskCheck, PreRiskCheck,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    frm::frm_types::{
        FrmChargebackReceivedRequest, FrmChargebackReceivedResponse, FrmFlowData,
        FrmPaymentOutcomeRequest, FrmPaymentOutcomeResponse, FrmRefundProcessedRequest,
        FrmRefundProcessedResponse, PostRiskCheckRequest, PostRiskCheckResponse,
        PreRiskCheckRequest, PreRiskCheckResponse,
    },
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Mask, Maskable};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers as nsure;
use transformers::{
    NsureChargebackResponse, NsureDisputeRequest, NsurePaymentOutcomeResponse,
    NsurePreRiskCheckRequest, NsurePreRiskCheckResponse, NsureRefundProcessedResponse,
    NsureRefundStatusRequest, NsureStatusUpdateRequest,
};

use super::super::connectors::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const ACCEPT: &str = "Accept";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    /// nSure's custom version header — `apiVersion.major.minor`.
    pub(crate) const X_NSURE_API_VERSION: &str = "x-nsure-api-version";
    /// nSure Application ID from the management portal.
    pub(crate) const X_NSURE_APP_ID: &str = "x-nsure-app-id";
}

/// Transaction submission endpoint. The `{transactionId}` segment is the
/// merchant's own transaction reference, echoed in `metadata.uniqueRequestId`.
const NSURE_TRANSACTIONS_PATH: &str = "/transactions";

/// Resolve the `{transactionId}` path segment for the lifecycle callbacks.
///
/// It must be the same id used on the original `POST /transactions/{id}`, which
/// was the merchant transaction id. Falls back to the ids nSure/the PSP echoed
/// back so a late notification can still be addressed.
fn nsure_transaction_id(
    merchant_transaction_id: Option<&str>,
    frm_transaction_id: Option<&str>,
    connector_transaction_id: Option<&str>,
) -> CustomResult<String, IntegrationError> {
    merchant_transaction_id
        .or(frm_transaction_id)
        .or(connector_transaction_id)
        .map(|id| id.to_string())
        .ok_or_else(|| {
            IntegrationError::MissingRequiredField {
                field_name: "merchant_transaction_id",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "nSure addresses lifecycle callbacks by the same transaction id used \
                         for the risk evaluation"
                            .to_owned(),
                    ),
                    suggested_action: Some(
                        "Send merchant_transaction_id on the FRM notification".to_owned(),
                    ),
                    doc_url: Some(nsure::NSURE_DOC_URL.to_owned()),
                },
            }
            .into()
        })
}

/// nSure amounts are JSON numbers in the major currency unit
/// (`{"valueInCurrency": 90, "currency": "USD"}` = 90 USD, not 90 cents).
macros::create_amount_converter_wrapper!(connector_name: Nsure, amount_type: FloatMajorUnit);

macros::create_all_prerequisites!(
    connector_name: Nsure,
    generic_type: T,
    api: [
        (
            flow: PreRiskCheck,
            request_body: NsurePreRiskCheckRequest,
            response_body: NsurePreRiskCheckResponse,
            router_data: RouterDataV2<PreRiskCheck, FrmFlowData, PreRiskCheckRequest, PreRiskCheckResponse>,
        ),
        (
            flow: FrmPaymentOutcome,
            request_body: NsureStatusUpdateRequest,
            response_body: NsurePaymentOutcomeResponse,
            router_data: RouterDataV2<FrmPaymentOutcome, FrmFlowData, FrmPaymentOutcomeRequest, FrmPaymentOutcomeResponse>,
        ),
        (
            flow: FrmRefundProcessed,
            request_body: NsureRefundStatusRequest,
            response_body: NsureRefundProcessedResponse,
            router_data: RouterDataV2<FrmRefundProcessed, FrmFlowData, FrmRefundProcessedRequest, FrmRefundProcessedResponse>,
        ),
        (
            flow: FrmChargebackReceived,
            request_body: NsureDisputeRequest,
            response_body: NsureChargebackResponse,
            router_data: RouterDataV2<FrmChargebackReceived, FrmFlowData, FrmChargebackReceivedRequest, FrmChargebackReceivedResponse>,
        )
    ],
    amount_converters: [
        amount_converter: FloatMajorUnit
    ],
    member_functions: {
        /// Headers common to every nSure call. Unlike Kount there is no token
        /// exchange: the authorization key is sent verbatim, with no scheme
        /// prefix, exactly as the nSure integration guide specifies.
        fn nsure_headers(
            &self,
            connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let auth = nsure::NsureAuthType::try_from(connector_config)?;
            let api_version = auth
                .api_version
                .clone()
                .unwrap_or_else(|| nsure::NSURE_DEFAULT_API_VERSION.to_string());
            let mut headers = vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.common_get_content_type().to_string().into(),
                ),
                (
                    headers::ACCEPT.to_string(),
                    "application/json".to_string().into(),
                ),
                (
                    headers::X_NSURE_API_VERSION.to_string(),
                    api_version.into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    auth.api_key.expose().into_masked(),
                ),
            ];
            if let Some(app_id) = auth.app_id {
                headers.push((headers::X_NSURE_APP_ID.to_string(), app_id.into()));
            }
            Ok(headers)
        }
    }
);

// =============================================================================
// CONNECTOR COMMON
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Nsure<T>
{
    fn id(&self) -> &'static str {
        "nsure"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        // nSure takes `valueInCurrency` in the major unit.
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json;charset=UTF-8"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.nsure.base_url
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = nsure::NsureAuthType::try_from(auth_type)?;
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            auth.api_key.expose().into_masked(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: nsure::NsureErrorResponse =
            res.response
                .parse_struct("NsureErrorResponse")
                .change_context(ConnectorError::ResponseDeserializationFailed {
                    context: domain_types::errors::ResponseTransformationErrorContext {
                        http_status_code: Some(res.status_code),
                        additional_context: Some(
                            "failed to parse the nSure error body as NsureErrorResponse".to_owned(),
                        ),
                    },
                })?;

        with_error_response_body!(event_builder, response);

        let message = response.message();
        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");
        Ok(ErrorResponse {
            // nSure error bodies carry only a message; the HTTP status is the
            // only machine-readable code, so it doubles as the error code.
            status_code: res.status_code,
            code: res.status_code.to_string(),
            reason: Some(message.clone()),
            message,
            attempt_status: None,
            connector_transaction_id: None,
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

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Nsure<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Nsure<T>
{
}

// =============================================================================
// VALIDATION
// =============================================================================
// nSure authenticates with a static key, so the access-token bootstrap that
// Kount needs is deliberately left at its `false` default — no
// ServerAuthenticationToken round trip happens for this connector.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Nsure<T>
{
}

// =============================================================================
// FRM SERVICE TRAIT
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::FrmServiceTrait for Nsure<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PreRiskCheckV2 for Nsure<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PostRiskCheckV2 for Nsure<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::FrmPaymentOutcomeV2 for Nsure<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::FrmRefundProcessedV2 for Nsure<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::FrmChargebackReceivedV2 for Nsure<T>
{
}

// `FrmServiceTrait` requires `ServerAuthentication`, but nSure has no token
// endpoint. The stub macro below emits both the `ServerAuthentication` marker
// and the not-implemented flow, so the requirement is satisfied without
// implying a capability the connector does not have.
macros::macro_connector_flow_status_impls!(
    connector: Nsure,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [ServerAuthenticationToken],
);

// =============================================================================
// REAL FLOW: PreRiskCheck — POST /transactions/{transactionId}
//            with `mode: "preAuthorization"`
// =============================================================================
macros::macro_connector_implementation!(
    connector_default_implementations: [get_error_response_v2],
    connector: Nsure,
    curl_request: Json(NsurePreRiskCheckRequest),
    curl_response: NsurePreRiskCheckResponse,
    flow_name: PreRiskCheck,
    resource_common_data: FrmFlowData,
    flow_request: PreRiskCheckRequest,
    flow_response: PreRiskCheckResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_content_type(&self) -> &'static str {
            ConnectorCommon::common_get_content_type(self)
        }
        fn get_headers(
            &self,
            req: &RouterDataV2<PreRiskCheck, FrmFlowData, PreRiskCheckRequest, PreRiskCheckResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.nsure_headers(&req.connector_config)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PreRiskCheck, FrmFlowData, PreRiskCheckRequest, PreRiskCheckResponse>,
        ) -> CustomResult<String, IntegrationError> {
            // The path segment is the merchant's transaction id; it must match
            // metadata.uniqueRequestId in the body so a later status update
            // addresses the same transaction.
            let transaction_id = req.request.merchant_transaction_id.as_deref().ok_or_else(|| {
                IntegrationError::MissingRequiredField {
                    field_name: "merchant_transaction_id",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "nSure.ai addresses the transaction by id in the URL: \
                             POST /transactions/{transactionId}"
                                .to_owned(),
                        ),
                        suggested_action: Some(
                            "Set merchant_transaction_id on the FRM Pre Risk Check request"
                                .to_owned(),
                        ),
                        doc_url: Some(nsure::NSURE_DOC_URL.to_owned()),
                    },
                }
            })?;
            Ok(format!(
                "{}{}/{}",
                req.resource_common_data.connectors.nsure.base_url,
                NSURE_TRANSACTIONS_PATH,
                transaction_id
            ))
        }
    }
);

// =============================================================================
// LIFECYCLE NOTIFICATIONS
// =============================================================================
// nSure's pre-auth flow does not end at the risk decision. Their docs state the
// transition to `fundsCaptured` is "the formal handshake for the nSure.ai
// liability shift" — without it nSure never learns the payment outcome and no
// chargeback liability transfers.

// FrmPaymentOutcome -> PUT /transactions/{transactionId}/status
macros::macro_connector_implementation!(
    connector_default_implementations: [get_error_response_v2],
    connector: Nsure,
    curl_request: Json(NsureStatusUpdateRequest),
    curl_response: NsurePaymentOutcomeResponse,
    flow_name: FrmPaymentOutcome,
    resource_common_data: FrmFlowData,
    flow_request: FrmPaymentOutcomeRequest,
    flow_response: FrmPaymentOutcomeResponse,
    http_method: Put,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_content_type(&self) -> &'static str {
            ConnectorCommon::common_get_content_type(self)
        }
        fn get_headers(
            &self,
            req: &RouterDataV2<FrmPaymentOutcome, FrmFlowData, FrmPaymentOutcomeRequest, FrmPaymentOutcomeResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.nsure_headers(&req.connector_config)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<FrmPaymentOutcome, FrmFlowData, FrmPaymentOutcomeRequest, FrmPaymentOutcomeResponse>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}{}/{}/status",
                req.resource_common_data.connectors.nsure.base_url,
                NSURE_TRANSACTIONS_PATH,
                nsure_transaction_id(
                    req.request.merchant_transaction_id.as_deref(),
                    req.request.frm_transaction_id.as_deref(),
                    req.request.connector_transaction_id.as_deref(),
                )?
            ))
        }
    }
);

// FrmRefundProcessed -> PUT /transactions/{transactionId}/status  (status: refunded)
macros::macro_connector_implementation!(
    connector_default_implementations: [get_error_response_v2],
    connector: Nsure,
    curl_request: Json(NsureRefundStatusRequest),
    curl_response: NsureRefundProcessedResponse,
    flow_name: FrmRefundProcessed,
    resource_common_data: FrmFlowData,
    flow_request: FrmRefundProcessedRequest,
    flow_response: FrmRefundProcessedResponse,
    http_method: Put,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_content_type(&self) -> &'static str {
            ConnectorCommon::common_get_content_type(self)
        }
        fn get_headers(
            &self,
            req: &RouterDataV2<FrmRefundProcessed, FrmFlowData, FrmRefundProcessedRequest, FrmRefundProcessedResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.nsure_headers(&req.connector_config)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<FrmRefundProcessed, FrmFlowData, FrmRefundProcessedRequest, FrmRefundProcessedResponse>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}{}/{}/status",
                req.resource_common_data.connectors.nsure.base_url,
                NSURE_TRANSACTIONS_PATH,
                nsure_transaction_id(
                    None,
                    req.request.frm_transaction_id.as_deref(),
                    req.request.connector_transaction_id.as_deref(),
                )?
            ))
        }
    }
);

// FrmChargebackReceived -> POST /transactions/{transactionId}/disputes
macros::macro_connector_implementation!(
    connector_default_implementations: [get_error_response_v2],
    connector: Nsure,
    curl_request: Json(NsureDisputeRequest),
    curl_response: NsureChargebackResponse,
    flow_name: FrmChargebackReceived,
    resource_common_data: FrmFlowData,
    flow_request: FrmChargebackReceivedRequest,
    flow_response: FrmChargebackReceivedResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_content_type(&self) -> &'static str {
            ConnectorCommon::common_get_content_type(self)
        }
        fn get_headers(
            &self,
            req: &RouterDataV2<FrmChargebackReceived, FrmFlowData, FrmChargebackReceivedRequest, FrmChargebackReceivedResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.nsure_headers(&req.connector_config)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<FrmChargebackReceived, FrmFlowData, FrmChargebackReceivedRequest, FrmChargebackReceivedResponse>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}{}/{}/disputes",
                req.resource_common_data.connectors.nsure.base_url,
                NSURE_TRANSACTIONS_PATH,
                nsure_transaction_id(
                    None,
                    req.request.frm_transaction_id.as_deref(),
                    req.request.connector_transaction_id.as_deref(),
                )?
            ))
        }
    }
);

// =============================================================================
// NOT-IMPLEMENTED FRM FLOWS
// =============================================================================
// PostRiskCheck is unused: nSure's post-decision reporting is the status
// callback above, not a second risk evaluation.
macros::frm_flow_not_implemented!(
    connector: Nsure,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    flow: PostRiskCheck,
    request: PostRiskCheckRequest,
    response: PostRiskCheckResponse,
    flow_name: "post_risk_check",
);
