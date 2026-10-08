pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt};
use domain_types::{
    connector_flow::{PayoutGet, PayoutTransfer},
    errors::{
        ConnectorError, IntegrationError, IntegrationErrorContext,
        ResponseTransformationErrorContext,
    },
    payment_method_data::PaymentMethodDataTypes,
    payouts::{
        payout_method_data::{Bank, PayoutMethodData, Wallet},
        payouts_types::{
            PayoutFlowData, PayoutGetRequest, PayoutGetResponse, PayoutTransferRequest,
            PayoutTransferResponse,
        },
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Mask, Maskable};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{PayoutGetV2, PayoutServiceTrait, PayoutTransferV2},
};

use crate::{connectors::macros, types::ResponseRouterData, with_error_response_body};
use serde::Serialize;
use transformers::{
    MifinityAuthType, MifinityErrorResponse, MifinityPayoutRequest, MifinityPayoutResponse,
    MifinityStatusResponse,
};

const API_VERSION: &str = "1";

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const ACCEPT: &str = "Accept";
    pub(crate) const KEY: &str = "key";
    pub(crate) const API_VERSION: &str = "api-version";
}

macros::create_all_prerequisites!(
    connector_name: MifinityPayouts,
    generic_type: T,
    api: [
        (
            flow: PayoutTransfer,
            request_body: MifinityPayoutRequest,
            response_body: MifinityPayoutResponse,
            router_data: RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest<T>, PayoutTransferResponse>,
        ),
        (
            flow: PayoutGet,
            response_body: MifinityStatusResponse,
            router_data: RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
        )
    ],
    amount_converters: [],
    member_functions: {
        fn build_headers(
            &self,
            connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let auth = MifinityAuthType::try_from(connector_config)?;
            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.common_get_content_type().to_string().into(),
                ),
                (
                    headers::ACCEPT.to_string(),
                    self.common_get_content_type().to_string().into(),
                ),
                (headers::KEY.to_string(), auth.key.expose().into_masked()),
                (
                    headers::API_VERSION.to_string(),
                    API_VERSION.to_string().into(),
                ),
            ])
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for MifinityPayouts<T>
{
    fn id(&self) -> &'static str {
        "mifinity"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.mifinity.base_url
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: MifinityErrorResponse = res
            .response
            .parse_struct("MifinityErrorResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: ResponseTransformationErrorContext {
                    http_status_code: Some(res.status_code),
                    additional_context: Some(
                        "MiFinity payouts - failed to deserialize error response".to_string(),
                    ),
                },
            })?;

        with_error_response_body!(event_builder, response);

        let typed_connector_response =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");

        let first_error = response.errors.first();
        let code = first_error
            .and_then(|e| e.error_code.clone())
            .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string());
        let message = first_error
            .and_then(|e| e.message.clone())
            .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string());

        Ok(ErrorResponse {
            status_code: res.status_code,
            code,
            message: message.clone(),
            reason: Some(message),
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
            typed_connector_response,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutServiceTrait<T>
    for MifinityPayouts<T>
{
}

macros::macro_connector_flow_status_impls!(
    connector: MifinityPayouts,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_supported: [ServerAuthenticationToken],
);

// ===== PAYOUT TRANSFER (dispatched by payout method) =====
// MiFinity wallet -> POST /api/payments/acct2acct
// SEPA bank transfer -> POST /api/payments/pab

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutTransferV2<T>
    for MifinityPayouts<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: MifinityPayouts,
    curl_request: Json(MifinityPayoutRequest),
    curl_response: MifinityPayoutResponse,
    flow_name: PayoutTransfer,
    resource_common_data: PayoutFlowData,
    flow_request: PayoutTransferRequest<T>,
    flow_response: PayoutTransferResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest<T>, PayoutTransferResponse>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self
                .base_url(&req.resource_common_data.connectors)
                .trim_end_matches('/');
            let endpoint = match req.request.payout_method_data.as_ref() {
                Some(PayoutMethodData::Wallet(Wallet::Mifinity(_))) => "api/payments/acct2acct",
                Some(PayoutMethodData::Bank(Bank::Sepa(_))) => "api/payments/pab",
                Some(_) | None => {
                    return Err(IntegrationError::connector_feature_not_supported(
                        self.id(),
                        "the selected payout method (MiFinity supports the MiFinity wallet and SEPA bank transfer only)",
                        Default::default(),
                    )
                    .into());
                }
            };
            Ok(format!("{base_url}/{endpoint}"))
        }

        fn get_headers(
            &self,
            req: &RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest<T>, PayoutTransferResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(&req.connector_config)
        }
    }
);

// ===== PAYOUT GET / STATUS SYNC (GET /api/transactions/{traceId}/status) =====

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutGetV2
    for MifinityPayouts<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: MifinityPayouts,
    curl_response: MifinityStatusResponse,
    flow_name: PayoutGet,
    resource_common_data: PayoutFlowData,
    flow_request: PayoutGetRequest,
    flow_response: PayoutGetResponse,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
        ) -> CustomResult<String, IntegrationError> {
            // MiFinity's status endpoint is keyed by transactionReference,
            // persisted as connector_payout_id from the transfer response.
            let trace_id = if let Some(reference) = req.request.connector_payout_id.clone() {
                reference
            } else {
                let reference = req.resource_common_data.connector_request_reference_id.clone();
                if reference.is_empty() {
                    return Err(IntegrationError::MissingRequiredField {
                        field_name: "connector_payout_id",
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "MiFinity payout sync requires the traceId (merchant_payout_id) used on the original transfer, or a connector_payout_id."
                                    .to_string(),
                            ),
                            ..Default::default()
                        },
                    }
                    .into());
                }
                reference
            };

            let base_url = self
                .base_url(&req.resource_common_data.connectors)
                .trim_end_matches('/');
            Ok(format!("{base_url}/api/transactions/{trace_id}/status"))
        }

        fn get_headers(
            &self,
            req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(&req.connector_config)
        }
    }
);

// ===== PAYOUT STUB FLOWS =====

macros::macro_connector_payout_implementation!(
    connector: MifinityPayouts,
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
