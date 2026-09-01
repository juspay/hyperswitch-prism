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
    connector_flow::{Authorize, PSync},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData,
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
use transformers::{self as d24, D24PaymentsRequest, D24PaymentsResponse, D24SyncResponse};

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
            request_body: D24PaymentsRequest,
            response_body: D24PaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: D24SyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
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
        ///
        /// `read_only_login` selects which credential feeds `X-Login` **and**
        /// the signed message. Directa24 issues two API Keys: the deposit
        /// (write) key used on `POST /v3/deposits`, and a read-only key used on
        /// the read-only `GET` endpoints. Both are signed with the single API
        /// Signature (`api_secret`), which is the only HMAC key D24 issues.
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
            read_only_login: bool,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            let auth = d24::D24AuthType::try_from(&req.connector_config)?;
            let x_date = d24_x_date();

            // `key1` is the read-only API Key. Merchants that were provisioned
            // with a single credential leave it empty — fall back to `api_key`
            // rather than signing with an empty login, which D24 rejects with
            // `Invalid Signature`.
            let x_login = if read_only_login && !auth.key1.peek().is_empty() {
                auth.key1.clone()
            } else {
                auth.api_key.clone()
            };

            // GET endpoints sign the empty string, not `null`. A flow declared
            // without `request_body` yields `Ok(None)` here.
            let json_payload: String = match self.get_request_body(req)? {
                Some(body) => body.content.get_inner_value().peek().to_owned(),
                None => String::new(),
            };

            let message = format!("{}{}{}", x_date, x_login.peek(), json_payload);

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
                (headers::X_LOGIN.to_string(), x_login.into_masked()),
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

    /// Directa24 `amount` is a JSON number in MAJOR units. There is no
    /// minor-unit field anywhere in the API. `FloatMajorUnit` divides by the
    /// currency's exponent, and CLP is already registered as a zero-decimal
    /// currency in `common_enums`, so CLP 10000 minor -> `10000.0` and
    /// USD 1050 minor -> `10.5`. Both are correct.
    fn get_currency_unit(&self) -> common_enums::CurrencyUnit {
        common_enums::CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.d24.base_url.as_ref()
    }

    /// Shared by both flows, and the two flows do NOT share an error shape:
    /// `POST /v3/deposits` answers with the flat
    /// `{code: integer, description, details: [string]}` envelope while
    /// `GET /v3/deposits/{id}` answers with the nested `ApiError`
    /// `{"error": {code: string, message, details: string|null}}`.
    /// `D24ErrorResponse` is an untagged enum over both.
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
                "d24: response body matched neither the flat `{code, description, details}` \
                 deposit-creation error envelope nor the nested `{error: {code, message, \
                 details}}` deposit-status error envelope.",
            ))?;

        with_error_response_body!(event_builder, response);

        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");
        let body = response.body();

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: body
                .code_string()
                .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
            message: body
                .message_string()
                .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
            reason: body.reason(),
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
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for D24<T>
{
}

// =============================================================================
// AUTHORIZE — POST /v3/deposits (non-PCI deposit; WebPay "WP")
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
            // Deposit creation is a write call — the deposit (write) API Key.
            self.build_headers(req, false)
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
// PSYNC — GET /v3/deposits/{deposit_id}
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for D24<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: D24,
    curl_response: D24SyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        // `get_headers` must NOT be defaulted: D24 authenticates every call,
        // GET included, with the HMAC signature over `X-Date || X-Login || ""`.
        fn get_headers(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            // Deposit status is a read-only endpoint — the read-only API Key.
            self.build_headers(req, true)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let deposit_id = req
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID {
                    context: Default::default(),
                })?;
            Ok(format!(
                "{}/v3/deposits/{}",
                self.connector_base_url_payments(req),
                deposit_id
            ))
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

// Webhooks are out of scope for this integration — see the comment on
// `notification_url` in transformers.rs.
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
// Only `Authorize` (WebPay redirect deposit) and `PSync` are in scope.
// Directa24 documents no capture and no void endpoint at all; Refund / RSync
// are a separate API surface and are not implemented yet.
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
