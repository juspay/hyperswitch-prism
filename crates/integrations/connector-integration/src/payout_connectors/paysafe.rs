pub mod transformers;

use std::fmt::Debug;

use base64::Engine;
use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt};
use domain_types::{
    connector_flow::{
        PayoutCreateRecipient, PayoutEligibility, PayoutTransfer, ServerAuthenticationToken,
    },
    connector_types::{
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::PaymentMethodDataTypes,
    payouts::payouts_types::{
        PayoutCreateRecipientRequest, PayoutCreateRecipientResponse, PayoutEligibilityRequest,
        PayoutEligibilityResponse, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse,
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
    connector_types::{
        PayoutCreateRecipientV2, PayoutEligibilityV2, PayoutServiceTrait, PayoutTransferV2,
        ServerAuthentication,
    },
};
use serde::Serialize;

use super::super::connectors::macros;
// Reuse the payment connector's auth and error types: paysafecard payouts share
// the PaymentHUB context (Basic auth, same error body).
use crate::connectors::paysafe::responses::PaysafeErrorResponse;
use crate::connectors::paysafe::transformers::PaysafeAuthType;
use crate::connectors::paysafe::BASE64_ENGINE;
use crate::types::ResponseRouterData;
use transformers::{
    PaysafePaymentHandleRequest, PaysafePaymentHandleResponse, PaysafeStandaloneCreditRequest,
    PaysafeStandaloneCreditResponse,
};

pub(crate) mod headers {
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

macros::create_all_prerequisites!(
    connector_name: PaysafePayouts,
    generic_type: T,
    api: [
        (
            flow: PayoutCreateRecipient,
            request_body: PaysafePaymentHandleRequest,
            response_body: PaysafePaymentHandleResponse,
            router_data: RouterDataV2<PayoutCreateRecipient, PayoutFlowData, PayoutCreateRecipientRequest, PayoutCreateRecipientResponse>,
        ),
        (
            flow: PayoutTransfer,
            request_body: PaysafeStandaloneCreditRequest,
            response_body: PaysafeStandaloneCreditResponse,
            router_data: RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>,
        )
    ],
    amount_converters: [],
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

// ===== CONNECTOR COMMON =====

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for PaysafePayouts<T>
{
    fn id(&self) -> &'static str {
        "paysafe"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.paysafe.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = PaysafeAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Paysafe Basic auth needs username/password from ConnectorSpecificConfig::Paysafe."
                            .to_string(),
                    ),
                    ..Default::default()
                },
            },
        )?;
        let auth_key = format!("{}:{}", auth.username.peek(), auth.password.peek());
        let auth_header = format!("Basic {}", BASE64_ENGINE.encode(auth_key));
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            auth_header.into_masked(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        _event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: PaysafeErrorResponse = res
            .response
            .parse_struct("PaysafeErrorResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: domain_types::errors::ResponseTransformationErrorContext {
                    http_status_code: Some(res.status_code),
                    additional_context: Some(
                        "Paysafe payout - failed to deserialize error response".to_string(),
                    ),
                },
            })?;

        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");
        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.error.code,
            message: response.error.message,
            reason: None,
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
            typed_connector_response: typed,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        })
    }
}

// ===== SERVER AUTHENTICATION (not used) =====

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ServerAuthentication
    for PaysafePayouts<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerAuthenticationToken,
        MerchantAuthenticationFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    > for PaysafePayouts<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            ServerAuthenticationToken,
            MerchantAuthenticationFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            ConnectorCommon::id(self),
            "server_authentication_token",
            IntegrationErrorContext::default(),
        )
        .into())
    }
}

// ===== PAYOUT SERVICE TRAIT =====

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutServiceTrait
    for PaysafePayouts<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutCreateRecipientV2
    for PaysafePayouts<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutTransferV2
    for PaysafePayouts<T>
{
}

// ===== PAYOUT CREATE RECIPIENT (mint the payment handle) =====

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: PaysafePayouts,
    curl_request: Json(PaysafePaymentHandleRequest),
    curl_response: PaysafePaymentHandleResponse,
    flow_name: PayoutCreateRecipient,
    resource_common_data: PayoutFlowData,
    flow_request: PayoutCreateRecipientRequest,
    flow_response: PayoutCreateRecipientResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<
                PayoutCreateRecipient,
                PayoutFlowData,
                PayoutCreateRecipientRequest,
                PayoutCreateRecipientResponse,
            >,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<
                PayoutCreateRecipient,
                PayoutFlowData,
                PayoutCreateRecipientRequest,
                PayoutCreateRecipientResponse,
            >,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}v1/paymenthandles",
                self.base_url(&req.resource_common_data.connectors)
            ))
        }
    }
);

// ===== PAYOUT TRANSFER (fund the standalone credit) =====

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: PaysafePayouts,
    curl_request: Json(PaysafeStandaloneCreditRequest),
    curl_response: PaysafeStandaloneCreditResponse,
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
            req: &RouterDataV2<
                PayoutTransfer,
                PayoutFlowData,
                PayoutTransferRequest,
                PayoutTransferResponse,
            >,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

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
                "{}v1/standalonecredits",
                self.base_url(&req.resource_common_data.connectors)
            ))
        }
    }
);

// ===== PAYOUT STUB FLOWS (not supported by Paysafe) =====

macros::macro_connector_payout_implementation!(
    connector: PaysafePayouts,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    payout_flows: [
        PayoutCreate,
        PayoutVoid,
        PayoutStage,
        PayoutCreateLink,
        PayoutEnrollDisburseAccount,
        PayoutGet
    ]
);

// `PayoutEligibility` has no arm in `macro_connector_payout_implementation!`,
// so its stub is still written out by hand.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutEligibilityV2
    for PaysafePayouts<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PayoutEligibility,
        PayoutFlowData,
        PayoutEligibilityRequest,
        PayoutEligibilityResponse,
    > for PaysafePayouts<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            PayoutEligibility,
            PayoutFlowData,
            PayoutEligibilityRequest,
            PayoutEligibilityResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            ConnectorCommon::id(self),
            "payout_eligibility",
            IntegrationErrorContext::default(),
        )
        .into())
    }
}
