pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    types::StringMajorUnit,
};
use domain_types::{
    connector_flow::{PayoutGet, PayoutTransfer},
    errors::{ConnectorError, IntegrationError},
    payment_method_data::PaymentMethodDataTypes,
    payouts::payouts_types::{
        PayoutFlowData, PayoutGetRequest, PayoutGetResponse, PayoutTransferRequest,
        PayoutTransferResponse,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{PayoutGetV2, PayoutServiceTrait, PayoutTransferV2},
};
use serde::Serialize;

use crate::{connectors::macros, types::ResponseRouterData, utils::response_deserialization_fail};
use transformers::{
    GotymeSanlamAuthType, GotymeSanlamErrorResponse, GotymeSanlamPayoutGetRequest,
    GotymeSanlamPayoutResponse as GotymeSanlamPayoutGetResponse, GotymeSanlamPayoutResponse,
    GotymeSanlamPayoutTransferRequest,
};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const X_API_KEY: &str = "X-Api-Key";
    pub(crate) const PROFILE_ID: &str = "Profile-Id";
}

macros::create_all_prerequisites!(
    connector_name: GotymeSanlamPayouts,
    generic_type: T,
    api: [
        (
            flow: PayoutTransfer,
            request_body: GotymeSanlamPayoutTransferRequest,
            response_body: GotymeSanlamPayoutResponse,
            router_data: RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>,
        ),
        (
            flow: PayoutGet,
            request_body: GotymeSanlamPayoutGetRequest,
            response_body: GotymeSanlamPayoutGetResponse,
            router_data: RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
        )
    ],
    amount_converters: [
        amount_converter: StringMajorUnit
    ],
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
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for GotymeSanlamPayouts<T>
{
    fn id(&self) -> &'static str {
        "gotyme_sanlam"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.gotyme_sanlam.base_url
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = GotymeSanlamAuthType::try_from(auth_type)?;

        Ok(vec![
            (
                headers::X_API_KEY.to_string(),
                auth.api_key.peek().to_owned().into_masked(),
            ),
            (
                headers::PROFILE_ID.to_string(),
                auth.profile_id.peek().to_owned().into(),
            ),
        ])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: GotymeSanlamErrorResponse = res
            .response
            .parse_struct("GotymeSanlamErrorResponse")
            .change_context(response_deserialization_fail(
                res.status_code,
                "gotyme_sanlam: response body did not match the expected error format",
            ))?;

        event_builder.map(|i| i.set_connector_response(&response));
        tracing::info!(response=?response, "response from connector");

        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");
        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response
                .error_code
                .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            message: response
                .error_message
                .or(response.message)
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            reason: None,
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

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutServiceTrait
    for GotymeSanlamPayouts<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutTransferV2
    for GotymeSanlamPayouts<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutGetV2
    for GotymeSanlamPayouts<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: GotymeSanlamPayouts,
    curl_request: Json(GotymeSanlamPayoutTransferRequest),
    curl_response: GotymeSanlamPayoutResponse,
    flow_name: PayoutTransfer,
    resource_common_data: PayoutFlowData,
    flow_request: PayoutTransferRequest,
    flow_response: PayoutTransferResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}/invoke",
                self.base_url(&req.resource_common_data.connectors)
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: GotymeSanlamPayouts,
    curl_request: Json(GotymeSanlamPayoutGetRequest),
    curl_response: GotymeSanlamPayoutGetResponse,
    flow_name: PayoutGet,
    resource_common_data: PayoutFlowData,
    flow_request: PayoutGetRequest,
    flow_response: PayoutGetResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}/invoke",
                self.base_url(&req.resource_common_data.connectors)
            ))
        }
    }
);

macros::macro_connector_payout_implementation!(
    connector: GotymeSanlamPayouts,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    payout_flows: [
        PayoutCreate,
        PayoutVoid,
        PayoutStage,
        PayoutCreateLink,
        PayoutCreateRecipient,
        PayoutEnrollDisburseAccount,
        PayoutEligibility
    ]
);

macros::macro_connector_flow_status_impls!(
    connector: GotymeSanlamPayouts,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [ServerAuthenticationToken],
);
