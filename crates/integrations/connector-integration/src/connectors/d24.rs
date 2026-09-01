pub mod transformers;

use std::fmt::Debug;

use common_utils::{
    crypto::{self, SignMessage},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    types::FloatMajorUnit,
};
use domain_types::{
    connector_flow::Authorize,
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, RefundFlowData,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding,
};
use serde::Serialize;
use transformers::{self as d24, D24PaymentsRequest, D24PaymentsResponse};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

pub(crate) mod headers {
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const X_DATE: &str = "X-Date";
    pub(crate) const X_LOGIN: &str = "X-Login";
}

/// Literal scheme prefix of the `Authorization` header. Case sensitive, single
/// trailing space.
const D24_AUTHORIZATION_SCHEME: &str = "D24 ";

/// `X-Date` must be an ISO-8601 UTC datetime formatted exactly as
/// `yyyy-MM-dd'T'HH:mm:ss'Z'` — **no sub-second component**. `time`'s Iso8601
/// encoder emits fractional seconds, so the value is assembled by hand.
fn d24_x_date() -> String {
    let now = common_utils::date_time::now();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    )
}

macros::create_all_prerequisites!(
    connector_name: D24,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: D24PaymentsRequest<T>,
            response_body: D24PaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: FloatMajorUnit
    ],
    member_functions: {
        /// Builds the Directa24 request headers, including the HMAC signature.
        ///
        /// `Authorization = "D24 " + lowercase_hex(HMAC_SHA256(api_secret, X-Date || X-Login || JSONPayload))`
        ///
        /// The signed `JSONPayload` MUST be byte-identical to the transmitted
        /// body. `get_request_body` yields the very same
        /// `RequestContent::Json`, and `RequestContent::get_body_bytes` derives
        /// the wire bytes from `get_inner_value()` — the identical
        /// `serde_json::to_string` call used here — so the two always agree.
        /// Request construction is deterministic (no nonces, no timestamps in
        /// the body), which is what makes this safe.
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            let auth = d24::D24AuthType::try_from(&req.connector_config)?;
            let x_date = d24_x_date();

            // GET endpoints sign the empty string, not `null`.
            let json_payload: String = match self.get_request_body(req)? {
                Some(body) => body.content.get_inner_value().peek().to_owned(),
                None => String::new(),
            };

            let message = format!("{}{}{}", x_date, auth.api_key.peek(), json_payload);

            let signature = crypto::HmacSha256::sign_message(
                &crypto::HmacSha256,
                auth.api_secret.peek().as_bytes(),
                message.as_bytes(),
            )
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })
            .attach_printable("d24: failed to sign the request payload")?;

            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.get_content_type().to_string().into(),
                ),
                (headers::X_DATE.to_string(), x_date.into()),
                (headers::X_LOGIN.to_string(), auth.api_key.into_masked()),
                (
                    headers::AUTHORIZATION.to_string(),
                    format!("{D24_AUTHORIZATION_SCHEME}{}", hex::encode(signature)).into_masked(),
                ),
            ])
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            req.resource_common_data.connectors.d24.base_url.trim_end_matches('/')
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            req.resource_common_data.connectors.d24.base_url.trim_end_matches('/')
        }
    }
);

// =============================================================================
// CONNECTOR COMMON IMPLEMENTATION
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for D24<T>
{
    fn id(&self) -> &'static str {
        "d24"
    }

    /// Directa24 `amount` is a JSON number in MAJOR units (`1000` == 1000.00 BRL).
    /// There is no minor-unit field anywhere in the API.
    fn get_currency_unit(&self) -> common_enums::CurrencyUnit {
        common_enums::CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.d24.base_url.as_ref()
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: d24::D24ErrorResponse = res
            .response
            .parse_struct("D24ErrorResponse")
            .change_context(crate::utils::response_deserialization_fail(
                res.status_code,
                "d24: response body did not match the documented \
                 `{code, description, details, type}` error envelope.",
            ))?;

        with_error_response_body!(event_builder, response);

        let typed = macros::serialize_typed_connector_payload(&response, "typed_connector_response");
        let reason = response.reason();

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response
                .code
                .map(|code| code.to_string())
                .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
            message: response
                .description
                .clone()
                .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
            reason,
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

// =============================================================================
// BODY DECODING IMPLEMENTATION
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding for D24<T> {}

// =============================================================================
// AUTHORIZE — POST /v3/deposits (PCI / Server2Server card deposit)
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for D24<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: D24,
    curl_request: Json(D24PaymentsRequest),
    curl_response: D24PaymentsResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/v3/deposits", self.connector_base_url_payments(req)))
        }
    }
);

// =============================================================================
// DYNAMICALLY GENERATED IMPLEMENTATIONS
// =============================================================================

// ===== CONNECTOR SERVICE TRAIT IMPLEMENTATION =====
// Aggregate trait - composes all other connector traits.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for D24<T>
{
}

// ===== BASE (NON-FLOW) TRAIT IMPLEMENTATIONS =====
// These are simple marker traits that are NOT flows and therefore have no arm
// in expand_flow_status_impl!. They must be impl'd manually.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for D24<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for D24<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for D24<T>
{
}

// ===== SOURCE VERIFICATION IMPLEMENTATION =====
// Non-generic marker trait required by VerifyRedirectResponse for webhook
// signature verification.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::verification::SourceVerification for D24<T>
{
}

// ===== PAYOUT TRAIT IMPLEMENTATIONS =====
// Emits payout marker-trait impls and default no-op ConnectorIntegrationV2
// impls for all PayoutXxxV2 flows.
crate::connectors::macros::macro_connector_payout_implementation!(
    connector: D24,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

// ===== FLOW STATUS IMPLEMENTATIONS =====
// Emits marker-trait impls AND stub ConnectorIntegrationV2 impls for every
// flow listed. Each stub's get_url returns
// IntegrationError::connector_flow_not_implemented(...).
//
// Only `Authorize` (card, one-time, auto-capture) is in scope for this
// connector. Directa24 documents no capture and no void endpoint at all; PSync,
// Refund and RSync live on the non-PCI `api…` host and are not implemented yet.
crate::connectors::macros::macro_connector_flow_status_impls!(
    connector: D24,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        Accept,
        GetConnectorCustomer,
        VoidPostRefund,
        ClientAuthenticationToken,
        CreateConnectorCustomer,
        DefendDispute,
        MandateRevoke,
        Authenticate,
        Capture,
        IncrementalAuthorization,
        CreateOrder,
        PostAuthenticate,
        PreAuthenticate,
        PSync,
        PaymentMethodToken,
        VoidPC,
        Void,
        RSync,
        Refund,
        RepeatPayment,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        SetupMandate,
        SubmitEvidence
    ],
);
