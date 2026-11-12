mod test;
pub mod transformers;

use crate::connectors::checkout::{resolve_checkout_base_url, OPTIONAL_CLIENT_ID};
use crate::{connectors::macros, types::ResponseRouterData};
use common_enums::PayoutStatus;
use common_utils::{errors::CustomResult, events, types::MinorUnit};
use domain_types::{
    connector_flow::{PayoutGet, PayoutTransfer},
    errors::{ConnectorError, IntegrationError},
    payment_method_data::PaymentMethodDataTypes,
    payouts::payouts_types::{
        PayoutFlowData, PayoutGetRequest, PayoutGetResponse, PayoutTransferRequest,
        PayoutTransferResponse,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
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
use transformers::{CheckoutPayoutRequest, CheckoutPayoutResponse};

pub const CHECKOUT_IDEMPOTENCY_KEY_HEADER: &str = "Cko-Idempotency-Key";

/// Resolves the Checkout payout-only hostname without changing the payment connector URL.
/// An absent client ID removes the optional subdomain; a present client ID contributes its
/// eight-character account prefix followed by a dot. A `base_url` override without the
/// `{{optional_client_id}}` placeholder is taken literally, mirroring payment URL handling.
/// https://www.checkout.com/docs/developer-resources/api/api-endpoints
pub fn get_checkout_payout_base_url(
    configured_base_url: &str,
    client_id: Option<&Secret<String>>,
) -> CustomResult<String, IntegrationError> {
    if !configured_base_url.contains(OPTIONAL_CLIENT_ID) {
        return Ok(configured_base_url.trim_end_matches('/').to_string());
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
                    error_stack::report!(IntegrationError::InvalidDataFormat {
                        field_name: "checkout.connector_config.connector_client_id",
                        context: domain_types::errors::IntegrationErrorContext {
                            additional_context: Some(
                                "Client ID must look like `cli_<id>`; its eight-character \
                                 account prefix forms the payout API subdomain"
                                    .to_owned(),
                            ),
                            ..Default::default()
                        },
                    })
                })
        })
        .transpose()?
        .unwrap_or_default();

    Ok(
        resolve_checkout_base_url(configured_base_url, &optional_client_id)
            .trim_end_matches('/')
            .to_string(),
    )
}

macros::create_all_prerequisites!(
    connector_name: CheckoutPayouts,
    generic_type: T,
    api: [
        (flow: PayoutTransfer, request_body: CheckoutPayoutRequest, response_body: CheckoutPayoutResponse,
         router_data: RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest<T>, PayoutTransferResponse>,),
        (flow: PayoutGet, response_body: CheckoutPayoutGetResponse,
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
        // Reuse the payment connector's body parsing, then remap PayoutTransfer errors: a 4xx
        // rejection terminally fails the payout (409/429 stay retryable), 5xx stays pending,
        // and the payment-side `request_id` must not leak into `connector_payout_id`.
        // PayoutGet clears this inferred status in its `get_error_response_v2` override.
        let mut error =
            crate::connectors::Checkout::<T>::new().build_error_response(res, event, config)?;
        error.attempt_status = match error.status_code {
            409 | 429 => None,
            400..=499 => Some(FlowStatus::Payout(PayoutStatus::Failure)),
            _ => Some(FlowStatus::Payout(PayoutStatus::Pending)),
        };
        error.connector_transaction_id = None;
        Ok(error)
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
            // A missing merchant_payout_id makes extract_connector_request_reference_id default
            // to "", which would send an empty Cko-Idempotency-Key and `reference` — retries on
            // this flow move money, so an empty reference must never reach the wire.
            if req.resource_common_data.connector_request_reference_id.is_empty() {
                return Err(domain_types::utils::missing_field_err("merchant_payout_id")());
            }
            let mut headers = self.get_auth_header(&req.connector_config)?;
            headers.push((CHECKOUT_IDEMPOTENCY_KEY_HEADER.into(), req.resource_common_data.connector_request_reference_id.clone().into()));
            Ok(headers)
        }
        fn get_url(&self, req: &RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest<T>, PayoutTransferResponse>) -> CustomResult<String, IntegrationError> {
            let credentials = transformers::credentials(&req.connector_config)?;
            let configured_base_url = self.base_url(req.resource_common_data.connectors.as_ref());
            Ok(format!("{}/payments", get_checkout_payout_base_url(configured_base_url, credentials.connector_client_id)?))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: CheckoutPayouts, curl_response: CheckoutPayoutGetResponse,
    flow_name: PayoutGet, resource_common_data: PayoutFlowData, flow_request: PayoutGetRequest, flow_response: PayoutGetResponse,
    http_method: Get, generic_type: T, [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            // A status-sync HTTP error says nothing about whether Checkout already paid the
            // payout, so it must not be turned into a terminal payout state.
            let mut error = self.build_error_response(res, event_builder, connector_config)?;
            error.attempt_status = None;
            Ok(error)
        }
        fn get_headers(&self, req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.get_auth_header(&req.connector_config)
        }
        fn get_url(&self, req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>) -> CustomResult<String, IntegrationError> {
            let id = req.request.connector_payout_id.as_deref()
                .ok_or_else(|| domain_types::utils::missing_field_err("connector_payout_id")())?;
            let credentials = transformers::credentials(&req.connector_config)?;
            let configured_base_url = self.base_url(req.resource_common_data.connectors.as_ref());
            Ok(format!("{}/payments/{id}", get_checkout_payout_base_url(configured_base_url, credentials.connector_client_id)?))
        }
    }
);
