//! Elavon Payment Gateway (EPG) — JSON REST gateway.
//!
//! Not to be confused with the `elavon` connector in this repo, which integrates
//! Elavon Converge: a different product with an XML/form-encoded API. EPG shares no
//! request or response shape with it.
//!
//! Scope: raw card payments (no-3DS and external/pass-through 3DS), one-time only,
//! plus the companion PSync / Capture / Void / Refund / RSync flows so that a
//! manual-capture authorization always has a settle and a release path.

pub mod transformers;

mod test;

use std::fmt::Debug;

use base64::Engine;
use common_enums::CurrencyUnit;
use common_utils::{
    consts::BASE64_ENGINE, errors::CustomResult, events, ext_traits::ByteSliceExt,
    types::StringMajorUnit,
};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
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
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers as elavon_pg;
use transformers::{
    ElavonPgAuthorizeRequest, ElavonPgCaptureRequest, ElavonPgCaptureResponse,
    ElavonPgPsyncResponse, ElavonPgRefundRequest, ElavonPgRefundResponse, ElavonPgRsyncResponse,
    ElavonPgTransactionResponse, ElavonPgVoidRequest, ElavonPgVoidResponse,
    ELAVON_PG_ACCEPT_VERSION, ELAVON_PG_CONNECTOR_ID, ELAVON_PG_MEDIA_TYPE,
};

use super::macros;
use crate::{types::ResponseRouterData, utils, with_error_response_body};

pub(crate) mod headers {
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const ACCEPT: &str = "Accept";
    /// EPG pins the API major version through this header; see
    /// [`transformers::ELAVON_PG_ACCEPT_VERSION`].
    pub(crate) const ACCEPT_VERSION: &str = "Accept-Version";
}

/// EPG resource paths (spec §15).
pub(crate) mod paths {
    /// Collection that backs `sale`, `void` and `refund` — the `type` discriminator
    /// in the body selects which.
    pub(crate) const TRANSACTIONS: &str = "transactions";
    /// Partial-capture resource. It is the only capture endpoint this connector
    /// uses, because it is the only one that states the captured amount explicitly.
    pub(crate) const PARTIAL_CAPTURES: &str = "partial-captures";
}

// ===== MACRO-BASED STRUCT AND BRIDGE SETUP =====
macros::create_all_prerequisites!(
    connector_name: ElavonPg,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: ElavonPgAuthorizeRequest<T>,
            response_body: ElavonPgTransactionResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: ElavonPgPsyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: ElavonPgCaptureRequest,
            response_body: ElavonPgCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: ElavonPgVoidRequest,
            response_body: ElavonPgVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: ElavonPgRefundRequest,
            response_body: ElavonPgRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: ElavonPgRsyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        )
    ],
    amount_converters: [amount_converter: StringMajorUnit],
    member_functions: {
        /// Headers common to every EPG request (spec §2.4). `Accept-Version` is
        /// pinned so a future EPG major cannot silently reshape our traffic.
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    ELAVON_PG_MEDIA_TYPE.to_string().into(),
                ),
                (
                    headers::ACCEPT.to_string(),
                    ELAVON_PG_MEDIA_TYPE.to_string().into(),
                ),
                (
                    headers::ACCEPT_VERSION.to_string(),
                    ELAVON_PG_ACCEPT_VERSION.to_string().into(),
                ),
            ];
            let mut auth_header = self.get_auth_header(&req.connector_config)?;
            header.append(&mut auth_header);
            Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.elavon_pg.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.elavon_pg.base_url
        }
    }
);

// ===== CONNECTOR COMMON IMPLEMENTATION =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for ElavonPg<T>
{
    fn id(&self) -> &'static str {
        ELAVON_PG_CONNECTOR_ID
    }

    /// EPG carries amounts as decimal strings in the currency's major units.
    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        ELAVON_PG_MEDIA_TYPE
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.elavon_pg.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = elavon_pg::ElavonPgAuthType::try_from(auth_type)?;
        let credentials = format!("{}:{}", auth.merchant_alias.peek(), auth.secret_key.peek());
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            format!("Basic {}", BASE64_ENGINE.encode(credentials)).into_masked(),
        )])
    }

    /// Only ever sees non-2xx bodies. EPG answers `201` even when the transaction
    /// itself was declined; that in-band decline carries a full `Transaction` body
    /// and is mapped inside the response transformers instead (spec §4.5).
    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        // 5xx responses may carry no body at all, in which case the HTTP status is
        // the only signal available.
        let response: elavon_pg::ElavonPgErrorResponse = if res.response.is_empty() {
            elavon_pg::ElavonPgErrorResponse::default()
        } else {
            res.response
                .parse_struct("ElavonPgErrorResponse")
                .change_context(utils::response_handling_fail_for_connector(
                    res.status_code,
                    ELAVON_PG_CONNECTOR_ID,
                ))?
        };

        with_error_response_body!(event_builder, response);

        let (code, message, reason) = elavon_pg::summarize_failures(&response.failures);

        Ok(ErrorResponse {
            status_code: res.status_code,
            code,
            message,
            reason,
            // A transport / validation / authorization failure says nothing about
            // the attempt's own status, so it is left for the caller to decide.
            attempt_status: None,
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_response: None,
            typed_connector_request: None,
        })
    }
}

// ===== BODY DECODING =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for ElavonPg<T>
{
}

// ===== CONNECTOR SERVICE TRAIT =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for ElavonPg<T>
{
}

// ===== BASE (NON-FLOW) TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for ElavonPg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for ElavonPg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for ElavonPg<T>
{
}

// EPG webhook signature verification is optional and per-merchant: the signer id
// and key are handed out by Elavon support and there is no universal signing secret
// or documented algorithm in the API reference. Fabricating an HMAC here would be
// worse than the default no-op.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for ElavonPg<T>
{
}

// ===== PAYOUT TRAIT IMPLEMENTATIONS =====
macros::macro_connector_payout_implementation!(
    connector: ElavonPg,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

// ===== AUTHORIZE =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for ElavonPg<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: ElavonPg,
    curl_request: Json(ElavonPgAuthorizeRequest),
    curl_response: ElavonPgTransactionResponse,
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
                "{}/{}",
                self.connector_base_url_payments(req),
                paths::TRANSACTIONS
            ))
        }
    }
);

// ===== PSYNC =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for ElavonPg<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: ElavonPg,
    curl_response: ElavonPgPsyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Get,
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
            let transaction_id = req
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Elavon Payment Gateway syncs a payment with GET /transactions/{id}, \
                             which needs the id returned by the sale"
                                .to_string(),
                        ),
                        ..Default::default()
                    },
                })?;
            Ok(format!(
                "{}/{}/{}",
                self.connector_base_url_payments(req),
                paths::TRANSACTIONS,
                transaction_id
            ))
        }
    }
);

// ===== CAPTURE =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for ElavonPg<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: ElavonPg,
    curl_request: Json(ElavonPgCaptureRequest),
    curl_response: ElavonPgCaptureResponse,
    flow_name: Capture,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        // EPG has no /capture endpoint. See `ElavonPgCaptureRequest` for why every
        // capture goes through the partial-capture resource rather than the
        // `POST /transactions/{id}` update (spec §8.2).
        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}/{}",
                self.connector_base_url_payments(req),
                paths::PARTIAL_CAPTURES
            ))
        }
    }
);

// ===== VOID =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for ElavonPg<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: ElavonPg,
    curl_request: Json(ElavonPgVoidRequest),
    curl_response: ElavonPgVoidResponse,
    flow_name: Void,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentVoidData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        // A void is a new Transaction on the same collection, with the payment being
        // reversed named in `parentTransaction` (spec §8.3).
        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}/{}",
                self.connector_base_url_payments(req),
                paths::TRANSACTIONS
            ))
        }
    }
);

// ===== REFUND =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for ElavonPg<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: ElavonPg,
    curl_request: Json(ElavonPgRefundRequest),
    curl_response: ElavonPgRefundResponse,
    flow_name: Refund,
    resource_common_data: RefundFlowData,
    flow_request: RefundsData,
    flow_response: RefundsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        // A refund is a new Transaction on the same collection (spec §8.4).
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}/{}",
                self.connector_base_url_refunds(req),
                paths::TRANSACTIONS
            ))
        }
    }
);

// ===== RSYNC =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for ElavonPg<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: ElavonPg,
    curl_response: ElavonPgRsyncResponse,
    flow_name: RSync,
    resource_common_data: RefundFlowData,
    flow_request: RefundSyncData,
    flow_response: RefundsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        // The refund is itself a Transaction, so it is read back from the same
        // endpoint as a payment — with the refund's own id (spec §8.5).
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}/{}/{}",
                self.connector_base_url_refunds(req),
                paths::TRANSACTIONS,
                req.request.connector_refund_id
            ))
        }
    }
);

// ===== FLOW STATUS IMPLEMENTATIONS =====
// not_implemented: EPG exposes the capability but it is out of scope for this
//                  integration (cards, one-time payments only).
// not_supported:   EPG's v1 API has no such resource at all.
macros::macro_connector_flow_status_impls!(
    connector: ElavonPg,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        SetupMandate,
        RepeatPayment,
        MandateRevoke,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        PaymentMethodToken,
        CreateOrder,
        ClientAuthenticationToken,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        IncrementalAuthorization,
        VoidPC,
        VoidPostRefund,
    ],
    not_supported: [
        Accept,
        DefendDispute,
        SubmitEvidence,
    ],
);
