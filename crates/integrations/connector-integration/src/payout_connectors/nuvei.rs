pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::BytesExt};
use domain_types::{
    connector_flow::PayoutTransfer,
    errors::{ConnectorError, IntegrationError},
    payment_method_data::PaymentMethodDataTypes,
    payouts::payouts_types::{PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::Maskable;
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{PayoutServiceTrait, PayoutTransferV2},
};
use serde::Serialize;

use crate::{connectors::macros, types::ResponseRouterData, with_error_response_body};
use transformers::{NuveiPayoutRequest, NuveiPayoutResponse};

macros::create_all_prerequisites!(
    connector_name: NuveiPayouts,
    generic_type: T,
    api: [
        (
            flow: PayoutTransfer,
            request_body: NuveiPayoutRequest,
            response_body: NuveiPayoutResponse,
            router_data: RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>,
        )
    ],
    amount_converters: [],
    member_functions: {}
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for NuveiPayouts<T>
{
    fn id(&self) -> &'static str {
        "nuvei"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.nuvei.base_url
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: NuveiPayoutResponse = res
            .response
            .parse_struct("NuveiPayoutResponse")
            .change_context(crate::utils::response_deserialization_fail(
                res.status_code,
                "nuvei",
            ))?;
        with_error_response_body!(event_builder, response);
        Ok(response.error_response(res.status_code))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutServiceTrait
    for NuveiPayouts<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutTransferV2
    for NuveiPayouts<T>
{
}
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: NuveiPayouts,
    curl_request: Json(NuveiPayoutRequest),
    curl_response: NuveiPayoutResponse,
    flow_name: PayoutTransfer,
    resource_common_data: PayoutFlowData,
    flow_request: PayoutTransferRequest,
    flow_response: PayoutTransferResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<
                PayoutTransfer,
                PayoutFlowData,
                PayoutTransferRequest,
                PayoutTransferResponse,
            >,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}/payout.do",
                self.base_url(&req.resource_common_data.connectors)
                    .trim_end_matches('/')
            ))
        }

        fn get_headers(
            &self,
            _req: &RouterDataV2<
                PayoutTransfer,
                PayoutFlowData,
                PayoutTransferRequest,
                PayoutTransferResponse,
            >,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![(
                "Content-Type".to_owned(),
                self.common_get_content_type().to_owned().into(),
            )])
        }
    }
);

macros::macro_connector_payout_implementation!(
    connector: NuveiPayouts,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    payout_flows: [
        PayoutCreate,
        PayoutGet,
        PayoutVoid,
        PayoutStage,
        PayoutCreateLink,
        PayoutCreateRecipient,
        PayoutEnrollDisburseAccount,
        PayoutEligibility
    ]
);

macros::macro_connector_flow_status_impls!(
    connector: NuveiPayouts,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [ServerAuthenticationToken]
);
