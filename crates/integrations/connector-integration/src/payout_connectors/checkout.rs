pub mod transformers;

use crate::connectors::checkout::{resolve_checkout_base_url, OPTIONAL_CLIENT_ID};
use crate::{connectors::macros, types::ResponseRouterData};
use common_utils::{errors::CustomResult, events, types::MinorUnit};
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
use hyperswitch_masking::{Mask, Maskable, PeekInterface, Secret};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{PayoutGetV2, PayoutServiceTrait, PayoutTransferV2},
};
use serde::Serialize;
use std::fmt::Debug;
use transformers::CheckoutPayoutResponse as CheckoutPayoutGetResponse;
use transformers::{CheckoutPayoutGetRequest, CheckoutPayoutRequest, CheckoutPayoutResponse};

pub const CHECKOUT_IDEMPOTENCY_KEY_HEADER: &str = "Cko-Idempotency-Key";

/// Resolves the Checkout payout-only hostname without changing the payment connector URL.
/// An absent client ID removes the optional subdomain; a present client ID contributes its
/// eight-character account prefix followed by a dot.
/// https://www.checkout.com/docs/developer-resources/api/api-endpoints
pub fn get_checkout_payout_base_url(
    configured_base_url: &str,
    client_id: Option<&Secret<String>>,
) -> CustomResult<String, IntegrationError> {
    if !configured_base_url.contains(OPTIONAL_CLIENT_ID) {
        return Err(IntegrationError::InvalidDataFormat {
            field_name: "connectors.checkout.base_url",
            context: Default::default(),
        }
        .into());
    }

    let optional_client_id = client_id
        .map(|client_id| {
            client_id
                .peek()
                .strip_prefix("cli_")
                .and_then(|value| value.get(..8))
                .filter(|value| value.bytes().all(|byte| byte.is_ascii_alphanumeric()))
                .map(|prefix| format!("{prefix}."))
                .ok_or_else(|| {
                    domain_types::utils::missing_field_err(
                        "merchant_connector_account.metadata.client_id",
                    )()
                })
        })
        .transpose()?
        .unwrap_or_default();

    Ok(resolve_checkout_base_url(
        configured_base_url,
        &optional_client_id,
    ))
}

macros::create_all_prerequisites!(
    connector_name: CheckoutPayouts,
    generic_type: T,
    api: [
        (flow: PayoutTransfer, request_body: CheckoutPayoutRequest, response_body: CheckoutPayoutResponse,
         router_data: RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest<T>, PayoutTransferResponse>,),
        (flow: PayoutGet, request_body: CheckoutPayoutGetRequest, response_body: CheckoutPayoutGetResponse,
         router_data: RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,)
    ],
    amount_converters: [amount_converter: MinorUnit],
    member_functions: {}
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for CheckoutPayouts<T>
{
    fn id(&self) -> &'static str {
        "checkout"
    }
    fn get_currency_unit(&self) -> common_enums::CurrencyUnit {
        common_enums::CurrencyUnit::Minor
    }
    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }
    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.checkout.base_url
    }
    fn get_auth_header(
        &self,
        config: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let credentials = transformers::credentials(config)?;
        Ok(vec![
            (
                "Authorization".into(),
                format!("Bearer {}", credentials.api_secret.peek()).into_masked(),
            ),
            ("Content-Type".into(), "application/json".into()),
        ])
    }
    fn build_error_response(
        &self,
        res: Response,
        event: Option<&mut events::Event>,
        config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        crate::connectors::Checkout::<T>::new().build_error_response(res, event, config)
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutServiceTrait<T>
    for CheckoutPayouts<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutTransferV2<T>
    for CheckoutPayouts<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutGetV2
    for CheckoutPayouts<T>
{
}
macros::macro_connector_flow_status_impls!(
    connector: CheckoutPayouts, generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [ServerAuthenticationToken]
);
macros::macro_connector_payout_implementation!(connector: CheckoutPayouts, generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    payout_flows: [PayoutCreate, PayoutVoid, PayoutStage, PayoutCreateLink, PayoutCreateRecipient, PayoutEnrollDisburseAccount, PayoutEligibility]
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: CheckoutPayouts, curl_request: Json(CheckoutPayoutRequest), curl_response: CheckoutPayoutResponse,
    flow_name: PayoutTransfer, resource_common_data: PayoutFlowData, flow_request: PayoutTransferRequest<T>, flow_response: PayoutTransferResponse,
    http_method: Post, generic_type: T, [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(&self, req: &RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest<T>, PayoutTransferResponse>) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut headers = self.get_auth_header(&req.connector_config)?;
            headers.push((CHECKOUT_IDEMPOTENCY_KEY_HEADER.into(), req.resource_common_data.connector_request_reference_id.clone().into()));
            Ok(headers)
        }
        fn get_url(&self, req: &RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest<T>, PayoutTransferResponse>) -> CustomResult<String, IntegrationError> {
            let credentials = transformers::credentials(&req.connector_config)?;
            let configured_base_url = self.base_url(req.resource_common_data.connectors.as_ref());
            Ok(format!("{}/payments", get_checkout_payout_base_url(configured_base_url, credentials.client_id)?))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: CheckoutPayouts, curl_request: Json(CheckoutPayoutGetRequest), curl_response: CheckoutPayoutGetResponse,
    flow_name: PayoutGet, resource_common_data: PayoutFlowData, flow_request: PayoutGetRequest, flow_response: PayoutGetResponse,
    http_method: Get, generic_type: T, [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(&self, req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.get_auth_header(&req.connector_config)
        }
        fn get_url(&self, req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>) -> CustomResult<String, IntegrationError> {
            let id = req.request.connector_payout_id.as_deref()
                .ok_or_else(|| domain_types::utils::missing_field_err("connector_payout_id")())?;
            let credentials = transformers::credentials(&req.connector_config)?;
            let configured_base_url = self.base_url(req.resource_common_data.connectors.as_ref());
            Ok(format!("{}/payments/{id}", get_checkout_payout_base_url(configured_base_url, credentials.client_id)?))
        }
    }
);
