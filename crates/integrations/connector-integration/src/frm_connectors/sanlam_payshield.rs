pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    types::StringMinorUnit,
};
use domain_types::{
    connector_flow::{
        FrmChargebackReceived, FrmPaymentOutcome, FrmRefundProcessed, PostRiskCheck,
        PrePayoutRiskCheck, PreRiskCheck,
    },
    errors::{ConnectorError, IntegrationError},
    frm::frm_types::{
        FrmChargebackReceivedRequest, FrmChargebackReceivedResponse, FrmFlowData,
        FrmPaymentOutcomeRequest, FrmPaymentOutcomeResponse, FrmRefundProcessedRequest,
        FrmRefundProcessedResponse, PostRiskCheckRequest, PostRiskCheckResponse,
        PrePayoutRiskCheckRequest, PrePayoutRiskCheckResponse, PreRiskCheckRequest,
        PreRiskCheckResponse,
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
use transformers as sanlam_payshield;
use transformers::{
    SanlamPayshieldCheckRequest, SanlamPayshieldCheckRequest as SanlamPayshieldPayoutCheckRequest,
    SanlamPayshieldCheckResponse,
    SanlamPayshieldCheckResponse as SanlamPayshieldPayoutCheckResponse,
};

use super::super::connectors::macros;
use crate::utils::response_deserialization_fail;
use crate::{types::ResponseRouterData, with_error_response_body};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const X_API_KEY: &str = "X-Api-Key";
}

macros::create_amount_converter_wrapper!(
    connector_name: SanlamPayshield,
    amount_type: StringMinorUnit
);

macros::create_all_prerequisites!(
    connector_name: SanlamPayshield,
    generic_type: T,
    api: [
        (
            flow: PreRiskCheck,
            request_body: SanlamPayshieldCheckRequest,
            response_body: SanlamPayshieldCheckResponse,
            router_data: RouterDataV2<PreRiskCheck, FrmFlowData, PreRiskCheckRequest, PreRiskCheckResponse>,
        ),
        (
            flow: PrePayoutRiskCheck,
            request_body: SanlamPayshieldPayoutCheckRequest,
            response_body: SanlamPayshieldPayoutCheckResponse,
            router_data: RouterDataV2<PrePayoutRiskCheck, FrmFlowData, PrePayoutRiskCheckRequest, PrePayoutRiskCheckResponse>,
        )
    ],
    amount_converters: [
        amount_convertor: StringMinorUnit
    ],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            let mut headers = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.get_content_type().to_string().into(),
            )];
            headers.append(&mut self.get_auth_header(&req.connector_config)?);
            Ok(headers)
        }
    }
);

// =============================================================================
// CONNECTOR COMMON
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for SanlamPayshield<T>
{
    fn id(&self) -> &'static str {
        "sanlam_payshield"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.sanlam_payshield.base_url
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = sanlam_payshield::SanlamPayshieldAuthType::try_from(auth_type)?;
        Ok(vec![(
            headers::X_API_KEY.to_string(),
            auth.api_key.expose().into_masked(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: sanlam_payshield::SanlamPayshieldErrorResponse = res
            .response
            .parse_struct("SanlamPayshieldErrorResponse")
            .change_context(response_deserialization_fail(
                res.status_code,
                "sanlam_payshield: failed to deserialize the risk-check error response as SanlamPayshieldErrorResponse; verify the connector returned JSON matching the expected error schema",
            ))?;

        with_error_response_body!(event_builder, response);

        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");
        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response
                .error_code
                .map_or(NO_ERROR_CODE.to_string(), |v| v.to_string()),
            message: response
                .error_message
                .clone()
                .or(response.message.clone())
                .unwrap_or(NO_ERROR_MESSAGE.to_string()),
            reason: response.reason(),
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
    for SanlamPayshield<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for SanlamPayshield<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for SanlamPayshield<T>
{
}

impl connector_types::FrmServiceTrait
    for SanlamPayshield<domain_types::payment_method_data::DefaultPCIHolder>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PreRiskCheckV2 for SanlamPayshield<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PostRiskCheckV2 for SanlamPayshield<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PrePayoutRiskCheckV2 for SanlamPayshield<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::FrmPaymentOutcomeV2 for SanlamPayshield<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::FrmRefundProcessedV2 for SanlamPayshield<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::FrmChargebackReceivedV2 for SanlamPayshield<T>
{
}

macros::macro_connector_flow_status_impls!(
    connector: SanlamPayshield,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [ServerAuthenticationToken, PreAuthenticate],
);

macros::frm_flow_not_implemented!(
    connector: SanlamPayshield,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    flow: PostRiskCheck,
    request: PostRiskCheckRequest,
    response: PostRiskCheckResponse,
    flow_name: "post_risk_check",
);
macros::frm_flow_not_implemented!(
    connector: SanlamPayshield,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    flow: FrmPaymentOutcome,
    request: FrmPaymentOutcomeRequest,
    response: FrmPaymentOutcomeResponse,
    flow_name: "frm_payment_outcome",
);
macros::frm_flow_not_implemented!(
    connector: SanlamPayshield,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    flow: FrmRefundProcessed,
    request: FrmRefundProcessedRequest,
    response: FrmRefundProcessedResponse,
    flow_name: "frm_refund_processed",
);
macros::frm_flow_not_implemented!(
    connector: SanlamPayshield,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    flow: FrmChargebackReceived,
    request: FrmChargebackReceivedRequest,
    response: FrmChargebackReceivedResponse,
    flow_name: "frm_chargeback_received",
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_error_response_v2],
    connector: SanlamPayshield,
    curl_request: Json(SanlamPayshieldCheckRequest),
    curl_response: SanlamPayshieldCheckResponse,
    flow_name: PreRiskCheck,
    resource_common_data: FrmFlowData,
    flow_request: PreRiskCheckRequest,
    flow_response: PreRiskCheckResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PreRiskCheck, FrmFlowData, PreRiskCheckRequest, PreRiskCheckResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PreRiskCheck, FrmFlowData, PreRiskCheckRequest, PreRiskCheckResponse>,
        ) -> CustomResult<String, IntegrationError> {
            let merchant_id = req.resource_common_data.merchant_id.get_string_repr();
            Ok(format!(
                "{}/payshield/v1/check/{}",
                self.base_url(&req.resource_common_data.connectors).to_owned(),
                merchant_id
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_error_response_v2],
    connector: SanlamPayshield,
    curl_request: Json(SanlamPayshieldPayoutCheckRequest),
    curl_response: SanlamPayshieldPayoutCheckResponse,
    flow_name: PrePayoutRiskCheck,
    resource_common_data: FrmFlowData,
    flow_request: PrePayoutRiskCheckRequest,
    flow_response: PrePayoutRiskCheckResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PrePayoutRiskCheck, FrmFlowData, PrePayoutRiskCheckRequest, PrePayoutRiskCheckResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PrePayoutRiskCheck, FrmFlowData, PrePayoutRiskCheckRequest, PrePayoutRiskCheckResponse>,
        ) -> CustomResult<String, IntegrationError> {
            let merchant_id = req.resource_common_data.merchant_id.get_string_repr();
            Ok(format!(
                "{}/payshield/v1/check/{}",
                self.base_url(&req.resource_common_data.connectors).to_owned(),
                merchant_id
            ))
        }
    }
);
