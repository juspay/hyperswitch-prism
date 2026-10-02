pub mod transformers;

use std::fmt::Debug;

use common_enums::{CurrencyUnit, DisputeStatus};
use common_utils::{
    consts,
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    types::{FloatMajorUnit, StringMajorUnit, StringMinorUnit, StringMinorUnitForConnector},
};
use domain_types::{
    connector_flow::{Authorize, Capture, Refund, RepeatPayment, SetupMandate, Void},
    connector_types::{
        ConnectorWebhookSecrets, DisputeWebhookDetailsResponse, DisputeWebhookReference,
        EventContext, EventType, PaymentFlowData, PaymentVoidData, PaymentWebhookReference,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, RefundFlowData,
        RefundsData, RefundsResponseData, RepeatPaymentData, RequestDetails, ResponseId,
        SetupMandateRequestData, WebhookDetailsResponse, WebhookResourceReference,
    },
    errors::{self, WebhookError},
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Mask, Maskable};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers as worldpayraft;
use transformers::{
    WorldpayraftCaptureRequest, WorldpayraftCaptureResponse, WorldpayraftPaymentRequest,
    WorldpayraftPaymentResponse, WorldpayraftRefundRequest, WorldpayraftRefundResponse,
    WorldpayraftRepeatPaymentRequest, WorldpayraftRepeatPaymentResponse,
    WorldpayraftSetupMandateRequest, WorldpayraftSetupMandateResponse, WorldpayraftVoidRequest,
    WorldpayraftVoidResponse,
};

use crate::{connectors::macros, types::ResponseRouterData, utils, with_error_response_body};

pub(crate) mod headers {
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

// =============================================================================
// CREATE ALL PREREQUISITES
// =============================================================================
macros::create_all_prerequisites!(
    connector_name: Worldpayraft,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: WorldpayraftPaymentRequest<T>,
            response_body: WorldpayraftPaymentResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: WorldpayraftCaptureRequest,
            response_body: WorldpayraftCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: WorldpayraftVoidRequest,
            response_body: WorldpayraftVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: WorldpayraftRefundRequest,
            response_body: WorldpayraftRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: WorldpayraftSetupMandateRequest<T>,
            response_body: WorldpayraftSetupMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: WorldpayraftRepeatPaymentRequest,
            response_body: WorldpayraftRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        )
    ],
    // amount_converter: MiscAmountsBalances ddddddddd.cc major-unit strings (every request amount
    //   and OriginalAuthAmount on the response).
    // line_item_amount_converter: Level3Data.UnitPrice only — implied decimals (minor units),
    //   with UnitPriceDecimal = the currency exponent.
    // webhook_amount_converter: Event Notifications JSON-number amounts only (dispute payloads).
    amount_converters: [
        amount_converter: StringMajorUnit,
        line_item_amount_converter: StringMinorUnit,
        webhook_amount_converter: FloatMajorUnit
    ],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            )];
            let mut auth_header = self.get_auth_header(&req.connector_config)?;
            header.append(&mut auth_header);
            Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.worldpayraft.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.worldpayraft.base_url
        }
    }
);

// =============================================================================
// CONNECTOR COMMON IMPLEMENTATION
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Worldpayraft<T>
{
    fn id(&self) -> &'static str {
        "worldpayraft"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.worldpayraft.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
        let auth = worldpayraft::WorldpayraftAuthType::try_from(auth_type).change_context(
            errors::IntegrationError::FailedToObtainAuthType {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Worldpay RAFT auth requires license and merchant_id from ConnectorSpecificConfig::Worldpayraft".to_string(),
                    ),
                    ..Default::default()
                },
            },
        )?;
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            format!("VANTIV license=\"{}\"", auth.license.expose()).into_masked(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let response: worldpayraft::WorldpayraftErrorBody = res
            .response
            .parse_struct("WorldpayraftErrorBody")
            .change_context(utils::response_handling_fail_for_connector(
                res.status_code,
                "worldpayraft",
            ))?;

        with_error_response_body!(event_builder, response);

        match response {
            // A wrapped RAFT body on a non-2xx status: same fields as the 2xx failure arm.
            worldpayraft::WorldpayraftErrorBody::Wrapped(wrapped) => {
                match wrapped.values().next() {
                    Some(inner) => Ok(worldpayraft::build_business_error(
                        inner,
                        res.status_code,
                        None,
                    )),
                    None => Ok(ErrorResponse {
                        status_code: res.status_code,
                        code: consts::NO_ERROR_CODE.to_string(),
                        message: consts::NO_ERROR_MESSAGE.to_string(),
                        reason: None,
                        attempt_status: None,
                        connector_transaction_id: None,
                        network_decline_code: None,
                        network_advice_code: None,
                        network_error_message: None,
                        raw_connector_response: None,
                        raw_connector_request: None,
                        typed_connector_response: None,
                        typed_connector_request: None,
                    }),
                }
            }
            // 401/403/404/500 licence or transport faults: `{"fault": {...}}` only.
            worldpayraft::WorldpayraftErrorBody::Fault { fault } => {
                let message = fault
                    .as_str()
                    .map(str::to_string)
                    .or_else(|| {
                        fault
                            .get("faultstring")
                            .or_else(|| fault.get("message"))
                            .and_then(|value| value.as_str())
                            .map(str::to_string)
                    })
                    .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string());
                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: consts::NO_ERROR_CODE.to_string(),
                    message,
                    reason: Some(fault.to_string()),
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
    }
}

// =============================================================================
// BODY DECODING IMPLEMENTATION
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Worldpayraft<T>
{
}

// ===== CONNECTOR SERVICE TRAIT IMPLEMENTATION =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Worldpayraft<T>
{
}

// ===== BASE (NON-FLOW) TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Worldpayraft<T>
{
}

// Worldpay Event Notifications (https://docs.worldpay.com/apis/event-notifications). The RAFT API
// itself pushes nothing; a transaction opts in with ProcFlagsIndicators.EventNotificationIndicator.
// Refund events do not exist in the Event Notifications catalogue, so process_refund_webhook keeps
// the trait default.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Worldpayraft<T>
{
    /// Event Notifications authenticates itself with an OAuth 2.0 bearer JWT signed RS256, not an
    /// HMAC over the body (https://docs.worldpay.com/apis/event-notifications/authentication):
    /// 1. read the token from `Authorization` (Standard) or `AuthorizationToken` (Salesforce);
    /// 2. the header must name `alg: RS256` and a `kid`;
    /// 3. resolve the `kid` in Worldpay's JWKS ({issuer}/rest/1.0/idpsettings/discovery/key),
    ///    which the merchant stores as the webhook secret (the trait is synchronous, so the JWKS
    ///    is configuration rather than a runtime fetch);
    /// 4. verify the RS256 signature, `iss` (cert or prod issuer) and `exp`.
    ///
    /// Any failure is `Ok(false)`: an unverifiable delivery is reported, never trusted.
    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        let Some(secrets) = connector_webhook_secret else {
            tracing::warn!(
                target: "worldpayraft_webhook",
                "no JWKS webhook secret configured for Worldpay Event Notifications verification"
            );
            return Ok(false);
        };
        Ok(verify_event_notification_jwt(&request, &secrets.secret))
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let envelope = worldpayraft::parse_webhook_envelope(&request.body)?;
        worldpayraft::get_webhook_event_type(&envelope)
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        let envelope = worldpayraft::parse_webhook_envelope(&request.body)?;
        // APITransactionID is not a field of any Event Notifications payload, but every payment
        // request sends its own APITransactionID as UserData1, which Worldpay echoes as
        // customerFields.field1: payments correlate on that (the same id Authorize reports as
        // connector_transaction_id), disputes on the Worldpay case id (dispute payloads carry no
        // payment key).
        if let Some(item) = envelope.payment_event_item() {
            let connector_transaction_id = item.api_transaction_id()?;
            return Ok(Some(WebhookResourceReference::Payment(
                PaymentWebhookReference {
                    connector_transaction_id: Some(connector_transaction_id),
                    merchant_transaction_id: None,
                },
            )));
        }
        Ok(envelope.dispute_case().map(|case| {
            WebhookResourceReference::Dispute(DisputeWebhookReference {
                connector_dispute_id: Some(case.id.clone()),
                connector_transaction_id: None,
            })
        }))
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let envelope = worldpayraft::parse_webhook_envelope(&request.body)?;
        let item = envelope.payment_event_item().ok_or_else(|| {
            error_stack::report!(WebhookError::WebhookBodyDecodingFailed).attach_printable(
                "Worldpay notification carries no authorizations/settlements item to process",
            )
        })?;
        let connector_transaction_id = item.api_transaction_id()?;
        let (error_code, error_message) = item.denial_details().unzip();
        Ok(WebhookDetailsResponse {
            resource_id: Some(ResponseId::ConnectorTransactionId(
                connector_transaction_id.clone(),
            )),
            // authorizations.created is emitted at authorization time and can be delivered after
            // a Capture: a preAuthIndicator=true event still reports Authorized, and the consumer
            // must not downgrade a Charged payment on it (settlement events are the terminal truth).
            status: item.outcome().attempt_status(),
            connector_response_reference_id: Some(connector_transaction_id),
            connector_request_reference_id: None,
            mandate_reference: None,
            error_code,
            error_message,
            error_reason: None,
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
            amount_captured: None,
            minor_amount_captured: None,
            network_txn_id: item.network_transaction_id(),
            payment_method_update: None,
            sender_payment_instrument_id: None,
            connector_returned_payment_method_details: None,
        })
    }

    fn process_dispute_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<DisputeWebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let envelope = worldpayraft::parse_webhook_envelope(&request.body)?;
        let case = envelope.dispute_case().ok_or_else(|| {
            error_stack::report!(WebhookError::WebhookBodyDecodingFailed)
                .attach_printable("Worldpay dispute notification carries no caseDetails")
        })?;

        let (status, stage) = match case.action.code.outcome() {
            worldpayraft::WorldpayraftDisputeActionOutcome::Opened(stage) => {
                (DisputeStatus::DisputeOpened, Some(stage))
            }
            worldpayraft::WorldpayraftDisputeActionOutcome::Challenged => (
                DisputeStatus::DisputeChallenged,
                case.stage.code.dispute_stage(),
            ),
            worldpayraft::WorldpayraftDisputeActionOutcome::Won => {
                (DisputeStatus::DisputeWon, case.stage.code.dispute_stage())
            }
            worldpayraft::WorldpayraftDisputeActionOutcome::Lost => {
                (DisputeStatus::DisputeLost, case.stage.code.dispute_stage())
            }
            worldpayraft::WorldpayraftDisputeActionOutcome::Unmapped => {
                return Err(error_stack::report!(WebhookError::WebhookProcessingFailed)
                    .attach_printable(format!(
                        "Worldpay dispute action {:?} has no published dispute-status mapping",
                        case.action.code
                    )));
            }
        };
        let stage = stage.ok_or_else(|| {
            error_stack::report!(WebhookError::WebhookProcessingFailed).attach_printable(format!(
                "Worldpay dispute stage {:?} has no dispute-stage mapping",
                case.stage.code
            ))
        })?;

        // Only `transaction.disputecases.created` carries a currency; a status update without one
        // is refused rather than assuming USD.
        let currency = case
            .original_transaction_amount_currency_type
            .ok_or_else(|| {
                error_stack::report!(WebhookError::WebhookMissingRequiredField {
                    field: "caseDetails.originalTransactionAmountCurrencyType",
                })
            })?;
        let amount = case
            .action
            .amount
            .or(case.original_transaction_amount)
            .ok_or_else(|| {
                error_stack::report!(WebhookError::WebhookMissingRequiredField {
                    field: "caseDetails.action.amount",
                })
            })?;
        let minor_amount = domain_types::utils::convert_back_amount_to_minor_units_for_webhook(
            self.webhook_amount_converter,
            amount,
            currency,
        )?;
        let amount = domain_types::utils::convert_amount_for_webhook(
            &StringMinorUnitForConnector,
            minor_amount,
            currency,
        )?;

        Ok(DisputeWebhookDetailsResponse {
            amount,
            currency,
            dispute_id: case.id.clone(),
            status,
            stage,
            connector_response_reference_id: case.source_system_case_id.clone(),
            dispute_message: case
                .reason
                .as_ref()
                .and_then(|reason| reason.description.clone()),
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
            connector_reason_code: case.reason.as_ref().and_then(|reason| reason.code.clone()),
        })
    }

    /// A minimal `authorizations.created` notification (field-probe input), trimmed from the
    /// documented Approved Credit Transaction example.
    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"{"eventType":"authorizations.created","notificationId":"64197a5d-d3e1-7y3g-5432-6c1074e270rf","eventCount":1,"version":"2.0","createdAt":"2023-02-01 10:05:16.113000+00:00","data":{"authorizations":[{"transactionStatus":{"code":"AA","shortDescription":"Approval"},"preAuthIndicator":false,"customerFields":{"field1":"4006642557"},"authCode":"075898","traceNumber":"833753"}]}}"#
    }
}

/// RS256 JWT check for an Event Notifications delivery against the JWKS held in the webhook
/// secret. Returns `false` on any missing header, malformed token, unknown `kid`, bad signature,
/// wrong issuer or expired token.
fn verify_event_notification_jwt(request: &RequestDetails, jwks_secret: &[u8]) -> bool {
    let header_value = |name: &str| {
        request
            .headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.trim())
    };
    let Some(token) = header_value(worldpayraft::WEBHOOK_AUTH_HEADER)
        .or_else(|| header_value(worldpayraft::WEBHOOK_AUTH_HEADER_SALESFORCE))
        .and_then(|value| {
            value
                .strip_prefix("Bearer ")
                .or_else(|| value.strip_prefix("bearer "))
        })
        .map(str::trim)
    else {
        tracing::warn!(target: "worldpayraft_webhook", "no bearer JWT in the notification headers");
        return false;
    };

    let Ok(header) = jsonwebtoken::decode_header(token) else {
        tracing::warn!(target: "worldpayraft_webhook", "notification JWT header is malformed");
        return false;
    };
    if header.alg != jsonwebtoken::Algorithm::RS256 {
        tracing::warn!(target: "worldpayraft_webhook", "notification JWT is not RS256");
        return false;
    }
    let Some(kid) = header.kid else {
        tracing::warn!(target: "worldpayraft_webhook", "notification JWT names no kid");
        return false;
    };

    let Ok(jwks) = serde_json::from_slice::<jsonwebtoken::jwk::JwkSet>(jwks_secret) else {
        tracing::warn!(target: "worldpayraft_webhook", "webhook secret is not a JWKS document");
        return false;
    };
    let Some(jwk) = jwks.find(&kid) else {
        tracing::warn!(target: "worldpayraft_webhook", "notification JWT kid is not in the JWKS");
        return false;
    };
    let Ok(decoding_key) = jsonwebtoken::DecodingKey::from_jwk(jwk) else {
        tracing::warn!(target: "worldpayraft_webhook", "JWKS entry is not a usable RSA key");
        return false;
    };

    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256);
    validation.set_issuer(&worldpayraft::WEBHOOK_JWT_ISSUERS);
    validation.validate_exp = true;
    // Worldpay documents `aud: "EnterpriseAuth"` but no audience the receiver must enforce.
    validation.validate_aud = false;

    match jsonwebtoken::decode::<serde_json::Value>(token, &decoding_key, &validation) {
        Ok(_) => true,
        Err(error) => {
            tracing::warn!(
                target: "worldpayraft_webhook",
                error = %error,
                "notification JWT failed verification"
            );
            false
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Worldpayraft<T>
{
}

// ===== SOURCE VERIFICATION IMPLEMENTATION =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Worldpayraft<T>
{
}

// ===== PAYOUT TRAIT IMPLEMENTATIONS =====
crate::connectors::macros::macro_connector_payout_implementation!(
    connector: Worldpayraft,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

// ===== AUTHORIZE TRAIT MARKER =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Worldpayraft<T>
{
}

// =============================================================================
// AUTHORIZE FLOW IMPLEMENTATION
// =============================================================================
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Worldpayraft,
    curl_request: Json(WorldpayraftPaymentRequest),
    curl_response: WorldpayraftPaymentResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            let (card_kind, operation) = worldpayraft::select_operation(&req.request);
            let path = worldpayraft::original_message_path(card_kind, operation);
            Ok(format!("{base_url}/{path}"))
        }
    }
);

// ===== CAPTURE TRAIT MARKER =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Worldpayraft<T>
{
}

// =============================================================================
// CAPTURE FLOW IMPLEMENTATION
// =============================================================================
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Worldpayraft,
    curl_request: Json(WorldpayraftCaptureRequest),
    curl_response: WorldpayraftCaptureResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            let feature_data = worldpayraft::capture_feature_data(&req.request)?;
            let path = worldpayraft::completion_path(feature_data.card_kind);
            Ok(format!("{base_url}/{path}"))
        }
    }
);

// ===== VOID TRAIT MARKER =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Worldpayraft<T>
{
}

// =============================================================================
// VOID FLOW IMPLEMENTATION
// =============================================================================
// No void endpoint exists: a reversal re-POSTs the original message's path with
// AuthorizationType RV (tech spec: 5. Void / Reversal).
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Worldpayraft,
    curl_request: Json(WorldpayraftVoidRequest),
    curl_response: WorldpayraftVoidResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            let feature_data = worldpayraft::void_feature_data(&req.request)?;
            let path = worldpayraft::original_message_path(feature_data.card_kind, feature_data.operation);
            Ok(format!("{base_url}/{path}"))
        }
    }
);

// ===== REFUND TRAIT MARKER =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Worldpayraft<T>
{
}

// =============================================================================
// REFUND FLOW IMPLEMENTATION
// =============================================================================
// A RAFT refund is an independent credit: the credit/debit route and the card token come from
// the original charge's connector_feature_data (tech spec: 4. Refund — Credit Refund).
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Worldpayraft,
    curl_request: Json(WorldpayraftRefundRequest),
    curl_response: WorldpayraftRefundResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = self.connector_base_url_refunds(req);
            let (feature_data, _) = worldpayraft::refund_feature_data(&req.request)?;
            let path = worldpayraft::refund_path(feature_data.card_kind);
            Ok(format!("{base_url}/{path}"))
        }
    }
);

// ===== SETUPMANDATE TRAIT MARKER =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Worldpayraft<T>
{
}

// =============================================================================
// SETUPMANDATE FLOW IMPLEMENTATION
// =============================================================================
// A zero-amount credit authorization (POSConditionCode 51) establishes the stored credential
// with the networks and returns the TokenizedPAN and the NTID (tech spec: SetupMandate).
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Worldpayraft,
    curl_request: Json(WorldpayraftSetupMandateRequest),
    curl_response: WorldpayraftSetupMandateResponse,
    flow_name: SetupMandate,
    resource_common_data: PaymentFlowData,
    flow_request: SetupMandateRequestData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}/credit/authorization"))
        }
    }
);

// ===== REPEAT PAYMENT TRAIT MARKER =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Worldpayraft<T>
{
}

// =============================================================================
// REPEAT PAYMENT FLOW IMPLEMENTATION
// =============================================================================
// No dedicated MIT endpoint: a merchant-initiated charge is a credit purchase (auto capture) or a
// credit authorization (manual) carrying the stored-credential fields (tech spec: RepeatPayment).
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Worldpayraft,
    curl_request: Json(WorldpayraftRepeatPaymentRequest),
    curl_response: WorldpayraftRepeatPaymentResponse,
    flow_name: RepeatPayment,
    resource_common_data: PaymentFlowData,
    flow_request: RepeatPaymentData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            let path = worldpayraft::original_message_path(
                worldpayraft::WorldpayraftCardKind::Credit,
                worldpayraft::repeat_payment_operation(&req.request),
            );
            Ok(format!("{base_url}/{path}"))
        }
    }
);

// ===== FLOW STATUS IMPLEMENTATIONS =====
// not_implemented: supportable by the Native RAFT API but out of this change.
//   IncrementalAuthorization: ProcFlagsIndicators.IncrementalAuth exists on creditauth.
// not_supported: the Native RAFT API has no call for the flow.
//   Accept / DefendDispute / SubmitEvidence: disputes are a separate Worldpay product.
//   VoidPC / VoidPostRefund: no cancel of a completion or refund exists (void is AuthorizationType RV).
crate::connectors::macros::macro_connector_flow_status_impls!(
    connector: Worldpayraft,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        CreateConnectorCustomer,
        GetConnectorCustomer,
        MandateRevoke,
        IncrementalAuthorization,
        PaymentMethodToken,
        CreateOrder,
        ClientAuthenticationToken,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
    ],
    not_supported: [
        // no Native RAFT transaction-inquiry operation exists; the synchronous response is final (tech spec: Not supported by the API §1)
        PSync,
        // no refund inquiry operation and no refund Event Notification exist; the refund response is final (tech spec: Not supported by the API §1)
        RSync,
        // LEG-COUNT = 0: Native RAFT has no 3DS call; external 3DS rides inside Authorize (tech spec: API Call Sequences → ThreeDS)
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        Accept,
        DefendDispute,
        SubmitEvidence,
        VoidPC,
        VoidPostRefund,
    ],
);
