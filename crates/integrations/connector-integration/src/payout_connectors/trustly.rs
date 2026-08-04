pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt};
use domain_types::{
    connector_flow::{
        PayoutCreateRecipient, PayoutEligibility, PayoutGet, PayoutTransfer,
        ServerAuthenticationToken,
    },
    connector_types::{
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
    },
    errors::{
        ConnectorError, IntegrationError, IntegrationErrorContext,
        ResponseTransformationErrorContext,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::PaymentMethodDataTypes,
    payouts::payouts_types::{
        PayoutCreateRecipientRequest, PayoutCreateRecipientResponse, PayoutEligibilityRequest,
        PayoutEligibilityResponse, PayoutFlowData, PayoutGetRequest, PayoutGetResponse,
        PayoutTransferRequest, PayoutTransferResponse,
    },
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
    connector_types::{
        PayoutCreateRecipientV2, PayoutEligibilityV2, PayoutGetV2, PayoutServiceTrait,
        PayoutTransferV2, ServerAuthentication,
    },
};
use serde::Serialize;

use super::super::connectors::macros;
// Reuse the shared Trustly error-response type for parsing connector errors.
use crate::connectors::trustly::transformers::TrustlyErrorResponse;
use crate::types::ResponseRouterData;
use transformers::{
    AccountPayoutRequest, AccountPayoutResponse, RegisterAccountRequest, RegisterAccountResponse,
    TrustlyPayoutSyncRequest, TrustlyPayoutSyncResponse,
};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

const CONTENT_TYPE_JSON: &str = "application/json; charset=UTF-8";

macros::create_all_prerequisites!(
    connector_name: TrustlyPayouts,
    generic_type: T,
    api: [
        (
            flow: PayoutCreateRecipient,
            request_body: RegisterAccountRequest,
            response_body: RegisterAccountResponse,
            router_data: RouterDataV2<PayoutCreateRecipient, PayoutFlowData, PayoutCreateRecipientRequest, PayoutCreateRecipientResponse>,
        ),
        (
            flow: PayoutTransfer,
            request_body: AccountPayoutRequest,
            response_body: AccountPayoutResponse,
            router_data: RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>,
        ),
        (
            flow: PayoutGet,
            request_body: TrustlyPayoutSyncRequest,
            response_body: TrustlyPayoutSyncResponse,
            router_data: RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
        )
    ],
    amount_converters: [],
    member_functions: {
        // Trustly authenticates each request in the JSON-RPC body (username / password
        // / signature), so only the content-type header is required.
        fn payout_headers(&self) -> Vec<(String, Maskable<String>)> {
            vec![(
                headers::CONTENT_TYPE.to_string(),
                CONTENT_TYPE_JSON.to_string().into(),
            )]
        }
    }
);

// ===== CONNECTOR COMMON =====

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for TrustlyPayouts<T>
{
    fn id(&self) -> &'static str {
        "trustly"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        CONTENT_TYPE_JSON
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.trustly.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        _auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        Ok(Vec::new())
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: TrustlyErrorResponse = res
            .response
            .parse_struct("TrustlyErrorResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: ResponseTransformationErrorContext {
                    http_status_code: Some(res.status_code),
                    additional_context: Some(
                        "Trustly payout - failed to deserialize error response".to_string(),
                    ),
                },
            })?;

        event_builder.map(|i| i.set_connector_response(&response));

        Ok(ErrorResponse {
            code: response.error.code.to_string(),
            message: response.error.message.clone(),
            reason: Some(response.error.message),
            status_code: res.status_code,
            attempt_status: None,
            connector_transaction_id: Some(response.error.error.uuid),
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        })
    }
}

// ===== SERVER AUTHENTICATION (not implemented) =====

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ServerAuthentication
    for TrustlyPayouts<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerAuthenticationToken,
        MerchantAuthenticationFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    > for TrustlyPayouts<T>
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
    for TrustlyPayouts<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutCreateRecipientV2
    for TrustlyPayouts<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutTransferV2
    for TrustlyPayouts<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutGetV2
    for TrustlyPayouts<T>
{
}

// ===== PAYOUT CREATE RECIPIENT (RegisterAccount) =====

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: TrustlyPayouts,
    curl_request: Json(RegisterAccountRequest),
    curl_response: RegisterAccountResponse,
    flow_name: PayoutCreateRecipient,
    resource_common_data: PayoutFlowData,
    flow_request: PayoutCreateRecipientRequest,
    flow_response: PayoutCreateRecipientResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<
                PayoutCreateRecipient,
                PayoutFlowData,
                PayoutCreateRecipientRequest,
                PayoutCreateRecipientResponse,
            >,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self
                .base_url(&req.resource_common_data.connectors)
                .to_string())
        }

        fn get_headers(
            &self,
            _req: &RouterDataV2<
                PayoutCreateRecipient,
                PayoutFlowData,
                PayoutCreateRecipientRequest,
                PayoutCreateRecipientResponse,
            >,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(self.payout_headers())
        }
    }
);

// ===== PAYOUT TRANSFER (AccountPayout) =====

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: TrustlyPayouts,
    curl_request: Json(AccountPayoutRequest),
    curl_response: AccountPayoutResponse,
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
            Ok(self
                .base_url(&req.resource_common_data.connectors)
                .to_string())
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
            Ok(self.payout_headers())
        }
    }
);

// ===== PAYOUT GET (GetWithdrawals) =====

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: TrustlyPayouts,
    curl_request: Json(TrustlyPayoutSyncRequest),
    curl_response: TrustlyPayoutSyncResponse,
    flow_name: PayoutGet,
    resource_common_data: PayoutFlowData,
    flow_request: PayoutGetRequest,
    flow_response: PayoutGetResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self
                .base_url(&req.resource_common_data.connectors)
                .to_string())
        }

        fn get_headers(
            &self,
            _req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(self.payout_headers())
        }
    }
);

// ===== PAYOUT STUB FLOWS (not supported by Trustly) =====

macros::macro_connector_payout_implementation!(
    connector: TrustlyPayouts,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    payout_flows: [
        PayoutCreate,
        PayoutVoid,
        PayoutStage,
        PayoutCreateLink,
        PayoutEnrollDisburseAccount
    ]
);

// `PayoutEligibility` has no arm in `macro_connector_payout_implementation!`,
// so its stub is still written out by hand.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutEligibilityV2
    for TrustlyPayouts<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PayoutEligibility,
        PayoutFlowData,
        PayoutEligibilityRequest,
        PayoutEligibilityResponse,
    > for TrustlyPayouts<T>
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
