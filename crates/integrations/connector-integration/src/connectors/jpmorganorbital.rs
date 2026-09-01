//! JP Morgan Orbital connector — Chase Paymentech "Orbital Gateway" JSON API v4.
//!
//! **Not the same product as the `jpmorgan` connector in this repository.**
//! `jpmorgan` speaks the JPMorgan Payments API v2 (`api-ms.payments.jpmorgan.com`,
//! OAuth2 client-credentials bearer tokens). Orbital is the legacy Chase Paymentech
//! gateway at `*.chasepaymentech.com`, authenticated with three plain HTTP headers
//! (`orbitalConnectionUsername`, `orbitalConnectionPassword`, `merchantID`) and
//! speaking a flat `merchant` / `order` / `paymentInstrument` JSON dialect with an
//! implied-decimal amount. The two share nothing but the brand name.
//!
//! Implemented scope — **card one-time payments only**:
//!
//! * **Authorize** — `POST /payments`, a single synchronous call with a terminal
//!   outcome, identically for non-3DS and 3DS. Orbital 3-D Secure is *external
//!   passthrough*: the challenge happens in the merchant's own MPI before UCS is
//!   called, and the connector only forwards the resulting CAVV/ECI in two extra
//!   body objects. There is no ACS redirect, no `redirection_data`, no
//!   `CompleteAuthorize` leg and no `AuthenticationPending` state.
//! * **PSync** — `POST /inquiry`, **recovery only**: it answers "did my request
//!   land?" for an Authorize whose HTTP response was never received, keyed on
//!   `order.inquiryRetryNumber` (the original `order.retryTrace`), within 48 hours.
//!   Note this is a `POST` **with a JSON body**, unlike the usual GET-sync pattern.
//!
//! Everything else — mandates/MIT/recurring, wallets, bank transfers/debits, BNPL,
//! Capture, Void, Refund, RSync, fraud analysis, Level 2/3 — is out of scope and
//! stubbed as not-implemented / not-supported. Orbital has **no webhooks** of any
//! kind, so `IncomingWebhook` is the default not-supported stub.

pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt};
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
use hyperswitch_masking::{Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding,
};
use serde::Serialize;
use transformers::{
    self as jpmorganorbital, JpmorganOrbitalInquiryRequest, JpmorganOrbitalInquiryResponse,
    JpmorganOrbitalPaymentsRequest, JpmorganOrbitalPaymentsResponse,
};

use super::macros;
use crate::types::ResponseRouterData;
use crate::with_error_response_body;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    /// Orbital's three credentials travel as ordinary headers. There is no
    /// `Authorization` header, no bearer token, no HMAC and no request signing.
    pub(crate) const ORBITAL_CONNECTION_USERNAME: &str = "orbitalConnectionUsername";
    pub(crate) const ORBITAL_CONNECTION_PASSWORD: &str = "orbitalConnectionPassword";
    pub(crate) const MERCHANT_ID: &str = "merchantID";
}

/// Authorize endpoint, appended to the configured base URL (which already carries
/// the `/gwapi/v4/gateway` prefix). v4 paths have **no** trailing slash, unlike v2/v3.
const ORBITAL_PAYMENTS_PATH: &str = "/payments";
/// Transaction Inquiry endpoint used by PSync.
const ORBITAL_INQUIRY_PATH: &str = "/inquiry";

// =============================================================================
// PREREQUISITES: struct, flow bridges, shared helpers
// =============================================================================
macros::create_all_prerequisites!(
    connector_name: JpmorganOrbital,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: JpmorganOrbitalPaymentsRequest<T>,
            response_body: JpmorganOrbitalPaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            // PSync is a POST with a JSON body on this gateway, so it needs a
            // request_body — the body-less GET-sync pattern does not apply.
            flow: PSync,
            request_body: JpmorganOrbitalInquiryRequest,
            response_body: JpmorganOrbitalInquiryResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        )
    ],
    amount_converters: [],
    member_functions: {
        /// `Content-Type` plus the three credential headers, identical on every
        /// Orbital request. The username and password are masked; `merchantID` is a
        /// merchant identifier rather than a secret but is masked for symmetry.
        pub fn build_headers<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, PaymentFlowData, Req, Res>,
        {
            let mut headers = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )];
            headers.extend(self.get_auth_header(&req.connector_config)?);
            Ok(headers)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.jpmorganorbital.base_url
        }
    }
);

// =============================================================================
// CONNECTOR COMMON
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for JpmorganOrbital<T>
{
    fn id(&self) -> &'static str {
        "jpmorganorbital"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        // Orbital's wire amount is the major-unit value with two implied decimals
        // for every currency; see `transformers::JpmorganOrbitalAmountForConnector`.
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.jpmorganorbital.base_url
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = jpmorganorbital::JpmorganOrbitalAuthType::try_from(auth_type)?;
        Ok(vec![
            (
                headers::ORBITAL_CONNECTION_USERNAME.to_string(),
                auth.username.peek().to_owned().into_masked(),
            ),
            (
                headers::ORBITAL_CONNECTION_PASSWORD.to_string(),
                auth.password.peek().to_owned().into_masked(),
            ),
            (
                headers::MERCHANT_ID.to_string(),
                auth.merchant_id.peek().to_owned().into_masked(),
            ),
        ])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        // Orbital has no separate error envelope. A non-2xx returns the flat
        // `{procStatus, procStatusMessage}` body, while a 200 carrying a decline
        // returns a full `paymentsResponse` with the same two fields nested under
        // `order.status`. `JpmorganOrbitalPaymentsResponse` models both shapes, so
        // the error path parses exactly the same struct as the success path.
        match res
            .response
            .parse_struct::<JpmorganOrbitalPaymentsResponse>("JpmorganOrbitalPaymentsResponse")
        {
            Ok(response) => {
                with_error_response_body!(event_builder, response);
                Ok(response.to_error_response(res.status_code))
            }
            // Not every failure comes from Orbital itself. 403 "SSL Connection Required"
            // and 412 "Security Information is missing" are documented, and an
            // intermediary proxy or WAF can answer with HTML or nothing at all. Failing
            // to deserialize would discard the HTTP status, which is the only diagnostic
            // such a response carries, so surface it instead.
            Err(_) => {
                let raw_body = String::from_utf8_lossy(&res.response).to_string();
                tracing::warn!(
                    status_code = res.status_code,
                    "jpmorganorbital returned a body matching neither the paymentsResponse \
                     envelope nor the flat {{procStatus, procStatusMessage}} error body — most \
                     likely an intermediary proxy, WAF or gateway response"
                );
                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: common_utils::consts::NO_ERROR_CODE.to_string(),
                    message: common_utils::consts::NO_ERROR_MESSAGE.to_string(),
                    reason: Some(raw_body.chars().take(500).collect()),
                    // Nothing here says whether the authorization landed, so the attempt
                    // is left for PSync to settle rather than marked terminal.
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                })
            }
        }
    }
}

// =============================================================================
// BODY DECODING
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for JpmorganOrbital<T>
{
}

// =============================================================================
// AUTHORIZE — POST /payments (non-3DS and external-3DS passthrough)
// =============================================================================
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: JpmorganOrbital,
    curl_request: Json(JpmorganOrbitalPaymentsRequest),
    curl_response: JpmorganOrbitalPaymentsResponse,
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
            Ok(format!(
                "{}{}",
                self.connector_base_url_payments(req),
                ORBITAL_PAYMENTS_PATH
            ))
        }
    }
);

// =============================================================================
// PSYNC — POST /inquiry (recovery only)
// =============================================================================
// Only useful when the Authorize response was never received: `/payments` is
// synchronous and terminal, so a payment whose response *did* arrive never needs a
// sync. The lookup key is the original `retryTrace`, not `txRefNum`, and the
// gateway forgets the pair after 48 hours.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: JpmorganOrbital,
    curl_request: Json(JpmorganOrbitalInquiryRequest),
    curl_response: JpmorganOrbitalInquiryResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}{}",
                self.connector_base_url_payments(req),
                ORBITAL_INQUIRY_PATH
            ))
        }
    }
);

// =============================================================================
// TRAIT IMPLEMENTATIONS
// =============================================================================

// Aggregate trait — composes all other connector traits.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for JpmorganOrbital<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for JpmorganOrbital<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for JpmorganOrbital<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for JpmorganOrbital<T>
{
}

// The Orbital Gateway JSON API has no webhooks, callbacks or server-to-server
// notifications of any kind, so this is the default not-supported stub.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for JpmorganOrbital<T>
{
}

// Orbital never redirects — 3DS is external passthrough and `paymentsResponse`
// contains no URL field — so there is no redirect response to verify.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for JpmorganOrbital<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::verification::SourceVerification for JpmorganOrbital<T>
{
}

// ===== PAYOUT TRAIT IMPLEMENTATIONS =====
macros::macro_connector_payout_implementation!(
    connector: JpmorganOrbital,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

// ===== FLOW STATUS IMPLEMENTATIONS =====
// Authorize and PSync are implemented above and therefore appear in neither list.
// `not_implemented` = Orbital documents an endpoint for it but this pass is scoped
// to card one-time payments; `not_supported` = the gateway has no equivalent concept
// at all, or it is a product this connector deliberately refuses.
macros::macro_connector_flow_status_impls!(
    connector: JpmorganOrbital,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        // Real Orbital endpoints (/capture, /reversal, /refund), out of scope here.
        Capture,
        Void,
        VoidPC,
        VoidPostRefund,
        Refund,
        RSync,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        PaymentMethodToken
    ],
    not_supported: [
        // Mandates / MIT / stored credentials are modelled by Orbital fields this
        // connector deliberately does not send.
        SetupMandate,
        RepeatPayment,
        MandateRevoke,
        // Orbital performs no authentication itself; 3DS is passed through on
        // Authorize, so there is no authentication leg to drive.
        Authenticate,
        PreAuthenticate,
        PostAuthenticate,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        ClientAuthenticationToken,
        IncrementalAuthorization,
        CreateOrder,
        // Disputes are handled outside the gateway API.
        Accept,
        SubmitEvidence,
        DefendDispute
    ],
);
