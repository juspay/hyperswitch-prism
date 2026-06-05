pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    types::FloatMajorUnit,
};
use domain_types::{
    connector_flow::{Authorize, Refund, ServerAuthenticationToken},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, RefundFlowData, RefundsData,
        RefundsResponseData, ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    self as qwikcilver, QwikcilverAuthType, QwikcilverAuthorizeFeatureData,
    QwikcilverAuthorizeRequest, QwikcilverAuthorizeResponse, QwikcilverErrorResponse,
    QwikcilverRedeemRequest, QwikcilverRedeemResponse, QwikcilverRefundMetadata,
    QwikcilverRefundOp, QwikcilverRefundRequest, QwikcilverRefundResponse,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const DATE_AT_CLIENT: &str = "DateAtClient";
    pub(crate) const TRANSACTION_ID: &str = "TransactionId";
}

// ============================================================================
// TRAIT WIRING — Qwikcilver implements only ServerAuthenticationToken +
// Authorize (Redeem) + Refund (Add Card | Cancel Redeem). Everything else
// falls through to the `not_supported`/`not_implemented` stubs emitted by
// `macro_connector_flow_status_impls!` below.
// ============================================================================

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Qwikcilver<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Qwikcilver<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Qwikcilver<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerAuthentication for Qwikcilver<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Qwikcilver<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Qwikcilver<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Qwikcilver<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Qwikcilver<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Qwikcilver<T>
{
    /// The `/authorize` endpoint runs as the access-token flow before any
    /// other call. The framework caches the resulting JWT on RouterDataV2
    /// and reuses it across subsequent flows.
    fn should_do_access_token(&self, _payment_method: Option<common_enums::PaymentMethod>) -> bool {
        true
    }
}

// NOTE: despite the `_payout_` in its name, this macro is the framework's
// universal "emit default stubs for every flow we don't explicitly handle"
// fan-out — it's used by all ~87 payment connectors in this crate. The name
// is a historical artifact; do not be misled into thinking Qwikcilver is a
// payout connector.
macros::macro_connector_payout_implementation!(
    connector: Qwikcilver,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

// ============================================================================
// AMOUNT CONVERTER — Qwikcilver uses decimal major units (e.g. `10.0` = 10 AED)
// ============================================================================

macros::create_amount_converter_wrapper!(connector_name: Qwikcilver, amount_type: FloatMajorUnit);

// ============================================================================
// PREREQUISITES — registers the 3 active flows + amount converter + helpers
// ============================================================================

macros::create_all_prerequisites!(
    connector_name: Qwikcilver,
    generic_type: T,
    api: [
        (
            flow: ServerAuthenticationToken,
            request_body: QwikcilverAuthorizeRequest,
            response_body: QwikcilverAuthorizeResponse,
            router_data: RouterDataV2<ServerAuthenticationToken, PaymentFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ),
        (
            flow: Authorize,
            request_body: QwikcilverRedeemRequest,
            response_body: QwikcilverRedeemResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: QwikcilverRefundRequest,
            response_body: QwikcilverRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: FloatMajorUnit
    ],
    member_functions: {
        /// Headers required by every authenticated Qwikcilver call. The
        /// `TransactionId` value MUST be numeric — non-numeric values produce
        /// HTTP 500 ("Input string was not in a correct format") from the API.
        /// The session JWT stays wrapped in `Secret<String>` until the very
        /// last moment (`.peek()` inside `format!`) and is then immediately
        /// masked before leaving the function.
        pub fn build_authenticated_headers<F, FCD, Req, Res>(
            &self,
            _req: &RouterDataV2<F, FCD, Req, Res>,
            access_token: &hyperswitch_masking::Secret<String>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.common_get_content_type().to_string().into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    format!("Bearer {}", access_token.peek()).into_masked(),
                ),
                (
                    headers::DATE_AT_CLIENT.to_string(),
                    qwikcilver::current_datetime_qwikcilver().into(),
                ),
                (
                    headers::TRANSACTION_ID.to_string(),
                    qwikcilver::numeric_transaction_id().to_string().into(),
                ),
            ])
        }

        pub fn extract_access_token<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<hyperswitch_masking::Secret<String>, IntegrationError>
        where
            FCD: AccessTokenHolder,
        {
            req.resource_common_data
                .access_token_secret()
                .ok_or_else(|| {
                    IntegrationError::FailedToObtainAuthType {
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "Qwikcilver requires a session token from `/authorize`".to_string(),
                            ),
                            ..Default::default()
                        },
                    }
                    .into()
                })
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.qwikcilver.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.qwikcilver.base_url
        }
    }
);

// ============================================================================
// ACCESS-TOKEN PLUMBING — small trait so member functions in the
// `create_all_prerequisites!` block can extract the cached JWT regardless of
// whether the flow's common data is `PaymentFlowData` or `RefundFlowData`.
//
// `RouterDataV2` is generic over the flow-common-data type; the framework
// gives us no shared accessor for "the access_token field" because not every
// FCD has one. We bridge that gap here with a tiny trait. Token stays
// wrapped in `Secret<String>` until the very last moment (the `format!` in
// `build_authenticated_headers`), keeping it out of logs and event_builder
// snapshots.
//
// Adding a new flow whose common-data type is neither PaymentFlowData nor
// RefundFlowData? Implement `AccessTokenHolder` for it here.
// ============================================================================

pub trait AccessTokenHolder {
    fn access_token_secret(&self) -> Option<hyperswitch_masking::Secret<String>>;
}

impl AccessTokenHolder for PaymentFlowData {
    fn access_token_secret(&self) -> Option<hyperswitch_masking::Secret<String>> {
        self.access_token.as_ref().map(|t| t.access_token.clone())
    }
}

impl AccessTokenHolder for RefundFlowData {
    fn access_token_secret(&self) -> Option<hyperswitch_masking::Secret<String>> {
        self.access_token.as_ref().map(|t| t.access_token.clone())
    }
}

// ============================================================================
// ConnectorCommon — shared identity, base URL, error parser.
// ============================================================================

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Qwikcilver<T>
{
    fn id(&self) -> &'static str {
        "qwikcilver"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.qwikcilver.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        // For the access-token bootstrap call we send the long-lived bearer
        // from the connector config. All other calls use the session JWT
        // returned by `/authorize`, attached via `build_authenticated_headers`.
        let auth = QwikcilverAuthType::try_from(auth_type)?;
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            format!("Bearer {}", auth.bootstrap_bearer_token.peek()).into_masked(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: QwikcilverErrorResponse =
            res.response.parse_struct("QwikcilverErrorResponse").change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                    "qwikcilver: response body did not match the expected format; confirm API version and connector documentation.",
                ),
            )?;

        with_error_response_body!(event_builder, response);

        let code = response
            .error_code
            .clone()
            .or_else(|| response.response_code.map(|c| c.to_string()))
            .unwrap_or_else(|| NO_ERROR_CODE.to_string());
        let message = response
            .response_message
            .clone()
            .or_else(|| response.error_description.clone())
            .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string());

        Ok(ErrorResponse {
            status_code: res.status_code,
            code,
            message,
            reason: response.error_description.or(response.response_message),
            attempt_status: None,
            connector_transaction_id: response.wallet_number.map(|v| v.expose()),
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        })
    }
}

// ============================================================================
// SERVER AUTHENTICATION TOKEN — `/authorize` bootstrap call
// ============================================================================

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Qwikcilver,
    curl_request: Json(QwikcilverAuthorizeRequest),
    curl_response: QwikcilverAuthorizeResponse,
    flow_name: ServerAuthenticationToken,
    resource_common_data: PaymentFlowData,
    flow_request: ServerAuthenticationTokenRequestData,
    flow_response: ServerAuthenticationTokenResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<ServerAuthenticationToken, PaymentFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}{}",
                self.connector_base_url_payments(req),
                "Qwikcilver/eGMS.RestApi/api/v2/authorize"
            ))
        }

        fn get_headers(
            &self,
            req: &RouterDataV2<ServerAuthenticationToken, PaymentFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut headers = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )];
            let mut auth = self.get_auth_header(&req.connector_config)?;
            headers.append(&mut auth);
            Ok(headers)
        }
    }
);

// ============================================================================
// AUTHORIZE (Redeem)
// ============================================================================

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Qwikcilver,
    curl_request: Json(QwikcilverRedeemRequest),
    curl_response: QwikcilverRedeemResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let feature_data = QwikcilverAuthorizeFeatureData::from_request(&req.request)?;
            Ok(format!(
                "{}QwikCilver/egms.restapi/api/v2/wallet/{}/REDEEM",
                self.connector_base_url_payments(req),
                urlencoding::encode(feature_data.wallet_number.peek()),
            ))
        }

        fn get_headers(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let token = self.extract_access_token(req)?;
            self.build_authenticated_headers(req, &token)
        }
    }
);

// ============================================================================
// REFUND — Add Card or Cancel Redeem (chosen by `refund_metadata.refund_type`)
// ============================================================================

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Qwikcilver,
    curl_request: Json(QwikcilverRefundRequest),
    curl_response: QwikcilverRefundResponse,
    flow_name: Refund,
    resource_common_data: RefundFlowData,
    flow_request: RefundsData,
    flow_response: RefundsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let metadata = QwikcilverRefundMetadata::from_request(&req.request)?;
            let endpoint = match metadata.refund_type {
                QwikcilverRefundOp::CancelRedeem => "CANCELREDEEM",
                QwikcilverRefundOp::AddCard => "card",
            };
            Ok(format!(
                "{}QwikCilver/egms.restapi/api/v2/wallet/{}/{}",
                self.connector_base_url_refunds(req),
                urlencoding::encode(metadata.wallet_number.peek()),
                endpoint,
            ))
        }

        fn get_headers(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let token = self.extract_access_token(req)?;
            self.build_authenticated_headers(req, &token)
        }
    }
);

// ============================================================================
// Opt-out list — every other framework flow is `not_supported`/`not_implemented`
// ============================================================================

macros::macro_connector_flow_status_impls!(
    connector: Qwikcilver,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        PSync,
        RSync,
    ],
    not_supported: [
        Void,
        Capture,
        VoidPC,
        IncrementalAuthorization,
        SetupMandate,
        RepeatPayment,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        ClientAuthenticationToken,
        ServerSessionAuthenticationToken,
        MandateRevoke,
        CreateOrder,
        CreateConnectorCustomer,
        PaymentMethodToken,
        Accept,
        SubmitEvidence,
        DefendDispute,
    ],
);
