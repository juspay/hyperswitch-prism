pub mod transformers;

use base64::Engine;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    crypto::VerifySignature,
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    FloatMajorUnitForConnector, StringMajorUnit, StringMinorUnit,
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, CreateOrder, PSync, RSync, Refund,
        RepeatPayment, SetupMandate, Void,
    },
    connector_types::{
        ClientAuthenticationTokenRequestData, ConnectorWebhookSecrets,
        DisputeWebhookDetailsResponse, DisputeWebhookReference, EventContext, EventType,
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData, PaymentVoidData,
        PaymentWebhookReference, PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundWebhookDetailsResponse,
        RefundWebhookReference, RefundsData, RefundsResponseData, RepeatPaymentData,
        RequestDetails, ResponseId, SetupMandateRequestData, WebhookDetailsResponse,
        WebhookResourceReference,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::{report, Report, ResultExt};
use hyperswitch_masking::{ExposeInterface, Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use ring::hmac;
use serde::Serialize;
use std::fmt::Debug;
use transformers::{
    non_empty_owned, rapyd_network_error_fields, CaptureRequest, RapydAuthType,
    RapydClientAuthRequest, RapydClientAuthResponse, RapydCreateOrderRequest,
    RapydCreateOrderResponse, RapydErrorClass, RapydErrorDataView, RapydErrorResponse,
    RapydIncomingWebhook, RapydPSyncResponse, RapydPaymentsRequest, RapydPaymentsResponse,
    RapydPaymentsResponse as RapydCaptureResponse, RapydPaymentsResponse as RapydVoidResponse,
    RapydPaymentsResponse as RapydAuthorizeResponse, RapydRefundRequest, RapydRepeatPaymentRequest,
    RapydRepeatPaymentResponse, RapydSetupMandateRequest, RapydSetupMandateResponse,
    RapydWebhookData, RefundResponse, RefundResponse as RapydRSyncResponse,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};
use domain_types::errors::{ConnectorError, WebhookError};
use domain_types::errors::{IntegrationError, IntegrationErrorContext};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

pub const BASE64_ENGINE_URL_SAFE: base64::engine::GeneralPurpose =
    base64::engine::general_purpose::URL_SAFE;

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Rapyd<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Rapyd,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Rapyd<T>
{
}
/// Reads a webhook header by its lowercase name (spec: Common Headers).
fn rapyd_webhook_header<'a>(
    request: &'a RequestDetails,
    name: &'static str,
) -> Result<&'a str, Report<WebhookError>> {
    request
        .headers
        .get(name)
        .map(String::as_str)
        .ok_or_else(|| report!(WebhookError::WebhookMissingRequiredField { field: name }))
}

/// Rebuilds the URL Rapyd signed: `https://{host}` + the path of the inbound
/// request (UD-01). An absolute `uri` contributes only its path; a relative one
/// drops its query string, as the Hyperswitch preimage carries none.
fn rapyd_webhook_url_path(request: &RequestDetails) -> Result<String, Report<WebhookError>> {
    let host = rapyd_webhook_header(request, "host")?;
    let uri = request
        .uri
        .as_deref()
        .ok_or_else(|| report!(WebhookError::WebhookMissingRequiredField { field: "uri" }))?;
    let path = match url::Url::parse(uri) {
        Ok(url) => url.path().to_owned(),
        Err(_) => uri.split('?').next().unwrap_or(uri).to_owned(),
    };
    Ok(format!("https://{host}{path}"))
}

fn parse_rapyd_webhook(
    request: &RequestDetails,
) -> Result<RapydIncomingWebhook, Report<WebhookError>> {
    request
        .body
        .parse_struct::<RapydIncomingWebhook>("RapydIncomingWebhook")
        .change_context(WebhookError::WebhookBodyDecodingFailed)
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Rapyd<T>
{
    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, Report<WebhookError>> {
        // Rapyd signs webhooks with the account's own access/secret key pair;
        // there is no separate webhook secret (spec: Authentication > Method).
        let config = connector_account_details
            .ok_or_else(|| report!(WebhookError::WebhookVerificationSecretNotFound))?;
        let auth = RapydAuthType::try_from(&config)
            .change_context(WebhookError::WebhookVerificationSecretInvalid)?;

        let signature_header = request
            .headers
            .get("signature")
            .ok_or_else(|| report!(WebhookError::WebhookSignatureNotFound))?;
        let salt = rapyd_webhook_header(&request, "salt")?;
        let timestamp = rapyd_webhook_header(&request, "timestamp")?;
        let url_path = rapyd_webhook_url_path(&request)?;

        // The header is base64(hex(HMAC-SHA256)): the digest is hex-encoded
        // BEFORE it is base64-encoded (spec: Creating / verifying the
        // authentication header). Undo both layers to get the 32 raw digest
        // bytes, then compare in constant time via the shared crypto helper.
        let signature_hex = BASE64_ENGINE_URL_SAFE
            .decode(signature_header.as_bytes())
            .change_context(WebhookError::WebhookSourceVerificationFailed)?;
        let signature = hex::decode(&signature_hex)
            .change_context(WebhookError::WebhookSourceVerificationFailed)?;

        // The raw received bytes, never re-serialised (spec: Preimage).
        let body = String::from_utf8(request.body.clone())
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        let message = format!(
            "{url_path}{salt}{timestamp}{}{}{body}",
            auth.access_key.peek(),
            auth.secret_key.peek()
        );

        common_utils::crypto::HmacSha256
            .verify_signature(
                auth.secret_key.peek().as_bytes(),
                &signature,
                message.as_bytes(),
            )
            .change_context(WebhookError::WebhookSourceVerificationFailed)
    }

    fn get_event_type(&self, request: RequestDetails) -> Result<EventType, Report<WebhookError>> {
        let webhook = request
            .body
            .parse_struct::<RapydIncomingWebhook>("RapydIncomingWebhook")
            .change_context(WebhookError::WebhookEventTypeNotFound)?;
        transformers::get_webhook_event_type(&webhook)
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, Report<WebhookError>> {
        let webhook = parse_rapyd_webhook(&request)?;
        let reference = match transformers::parse_webhook_data(&webhook)? {
            // Payment: the `payment_*` id, as PSync reads it.
            Some(RapydWebhookData::Payment(data)) => {
                Some(WebhookResourceReference::Payment(PaymentWebhookReference {
                    connector_transaction_id: Some(data.id),
                    merchant_transaction_id: non_empty_owned(data.merchant_reference_id.as_deref()),
                }))
            }
            // Refund: the `refund_*` id, as RSync reads it, plus its parent payment.
            Some(RapydWebhookData::Refund(data)) => {
                Some(WebhookResourceReference::Refund(RefundWebhookReference {
                    connector_refund_id: Some(data.id),
                    merchant_refund_id: None,
                    connector_transaction_id: Some(data.payment),
                    merchant_transaction_id: None,
                }))
            }
            // Dispute: the `dispute_*` token and the parent payment.
            Some(RapydWebhookData::Dispute(data)) => {
                Some(WebhookResourceReference::Dispute(DisputeWebhookReference {
                    connector_dispute_id: Some(data.token),
                    connector_transaction_id: Some(data.original_transaction_id),
                }))
            }
            None => None,
        };
        Ok(reference)
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, Report<WebhookError>> {
        let webhook = parse_rapyd_webhook(&request)?;
        let data = match transformers::parse_webhook_data(&webhook)? {
            Some(RapydWebhookData::Payment(data)) => data,
            Some(RapydWebhookData::Refund(_) | RapydWebhookData::Dispute(_)) | None => {
                return Err(report!(WebhookError::WebhookResourceObjectNotFound));
            }
        };
        // The same status map as Authorize/PSync/Capture/Void (review rule T2).
        let status = transformers::get_status(data.status.clone(), data.next_action.clone());
        let error_code = non_empty_owned(data.failure_code.as_deref())
            .or_else(|| non_empty_owned(data.error_code.as_deref()));
        let error_message = non_empty_owned(data.failure_message.as_deref());
        let network_txn_id = data
            .payment_method_data
            .as_ref()
            .and_then(|pmd| pmd.network_reference_id.as_ref())
            .map(|id| id.peek().to_owned());

        Ok(WebhookDetailsResponse {
            resource_id: Some(ResponseId::ConnectorTransactionId(data.id.clone())),
            status,
            connector_response_reference_id: non_empty_owned(data.merchant_reference_id.as_deref()),
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
            network_txn_id,
            payment_method_update: None,
            sender_payment_instrument_id: None,
            connector_returned_payment_method_details: None,
        })
    }

    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, Report<WebhookError>> {
        let webhook = parse_rapyd_webhook(&request)?;
        let data = match transformers::parse_webhook_data(&webhook)? {
            Some(RapydWebhookData::Refund(data)) => data,
            Some(RapydWebhookData::Payment(_) | RapydWebhookData::Dispute(_)) | None => {
                return Err(report!(WebhookError::WebhookResourceObjectNotFound));
            }
        };

        Ok(RefundWebhookDetailsResponse {
            connector_refund_id: Some(data.id.clone()),
            merchant_transaction_id: None,
            // The same refund status map as Refund/RSync (review rule T2).
            status: common_enums::RefundStatus::from(data.status.clone()),
            connector_response_reference_id: Some(data.payment.clone()),
            // UD-04: card-network code -> error_code, reason -> error_message.
            error_code: non_empty_owned(data.failure_code.as_deref()),
            error_message: non_empty_owned(data.failure_reason.as_deref()),
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
        })
    }

    fn process_dispute_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<DisputeWebhookDetailsResponse, Report<WebhookError>> {
        let webhook = parse_rapyd_webhook(&request)?;
        let data = match transformers::parse_webhook_data(&webhook)? {
            Some(RapydWebhookData::Dispute(data)) => data,
            Some(RapydWebhookData::Payment(_) | RapydWebhookData::Refund(_)) | None => {
                return Err(report!(WebhookError::WebhookResourceObjectNotFound));
            }
        };
        let status = transformers::dispute_status_to_common(&data.status)?;
        // Rapyd sends the dispute amount in MAJOR units (UD-03): convert
        // FloatMajorUnit -> MinorUnit -> StringMinorUnit through the shared
        // converters, never by float arithmetic.
        let minor_amount = domain_types::utils::convert_back_amount_to_minor_units_for_webhook(
            &FloatMajorUnitForConnector,
            data.amount,
            data.currency,
        )?;
        let amount = domain_types::utils::convert_amount_for_webhook(
            self.amount_converter_webhooks,
            minor_amount,
            data.currency,
        )?;
        let stage = if data.pre_dispute.unwrap_or(false) {
            common_enums::DisputeStage::PreDispute
        } else {
            common_enums::DisputeStage::Dispute
        };

        Ok(DisputeWebhookDetailsResponse {
            amount,
            currency: data.currency,
            dispute_id: data.token.clone(),
            status,
            stage,
            connector_response_reference_id: Some(data.original_transaction_id.clone()),
            dispute_message: Some(data.dispute_reason_description.clone()),
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
            connector_reason_code: None,
            additional_details: None,
        })
    }

    fn get_webhook_resource_object(
        &self,
        request: RequestDetails,
    ) -> Result<Box<dyn hyperswitch_masking::ErasedMaskSerialize>, Report<WebhookError>> {
        let webhook = parse_rapyd_webhook(&request)?;
        let resource: Box<dyn hyperswitch_masking::ErasedMaskSerialize> =
            match transformers::parse_webhook_data(&webhook)? {
                // The PSync envelope, so the flow handler can read it back.
                Some(RapydWebhookData::Payment(data)) => {
                    Box::new(RapydPaymentsResponse::from(*data))
                }
                // HS parity (PL-01): the bare refund object.
                Some(RapydWebhookData::Refund(data)) => Box::new(data),
                Some(RapydWebhookData::Dispute(data)) => Box::new(data),
                None => return Err(report!(WebhookError::WebhookResourceObjectNotFound)),
            };
        Ok(resource)
    }

    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"{"id":"wh_sample000000000000000000000000","type":"PAYMENT_COMPLETED","data":{"id":"payment_sample0000000000000000000000","amount":10.0,"status":"CLO","next_action":"not_applicable","currency_code":"USD","captured":true,"paid":true,"transaction_id":"","merchant_reference_id":""},"trigger_operation_id":"00000000-0000-0000-0000-000000000000","status":"NEW","created_at":1711008868}"#
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Rapyd<T>
{
    fn id(&self) -> &'static str {
        "rapyd"
    }

    fn get_currency_unit(&self) -> common_enums::CurrencyUnit {
        common_enums::CurrencyUnit::Base
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = RapydAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;

        // Return basic auth headers - signature will be added in get_headers method
        Ok(vec![(
            "access_key".to_string(),
            auth.access_key.into_masked(),
        )])
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.rapyd.base_url.as_ref()
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        // Error bodies come as `status` alone (errors §1.B) or as `status` plus
        // a reduced payment `data` (Create Payment, errors §1.C); the latter
        // carries the GSM fields, so both are parsed with RapydErrorResponse.
        let response: Result<RapydErrorResponse, Report<common_utils::errors::ParsingError>> =
            res.response.parse_struct("rapyd ErrorResponse");

        match response {
            Ok(response_data) => {
                with_error_response_body!(event_builder, response_data);
                let typed = macros::serialize_typed_connector_payload(
                    &response_data,
                    "typed_connector_response",
                );
                let data = response_data.data.as_ref();
                let network = rapyd_network_error_fields(
                    &response_data.status,
                    RapydErrorDataView {
                        failure_code: data.and_then(|d| d.failure_code.as_deref()),
                        error_code: data.and_then(|d| d.error_code.as_deref()),
                        failure_message: data.and_then(|d| d.failure_message.as_deref()),
                        merchant_advice_code: data.and_then(|d| d.merchant_advice_code.as_deref()),
                    },
                );
                let code = Some(response_data.status.error_code.as_str())
                    .filter(|code| !code.trim().is_empty())
                    .map(str::to_owned)
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string());
                // Refund-terminal codes fail the refund; every other flow sets
                // its own attempt status (Authorize: get_error_response_v2), and
                // sync/capture/void stay None so a lookup miss is never terminal.
                let attempt_status = RAPYD_REFUND_TERMINAL_ERROR_CODES
                    .contains(&code.as_str())
                    .then_some(FlowStatus::Refund(common_enums::RefundStatus::Failure));
                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code,
                    // HS parity (PL-03 / UD-16): message = status.status, so
                    // existing GSM rules keep matching; reason = status.message.
                    message: response_data
                        .status
                        .status
                        .clone()
                        .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                    reason: response_data.status.message.clone(),
                    attempt_status,
                    connector_transaction_id: data.and_then(|d| d.id.clone()),
                    network_advice_code: network.advice_code,
                    network_decline_code: network.decline_code,
                    network_error_message: network.error_message,
                    typed_connector_response: typed,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                })
            }
            Err(error_msg) => {
                if let Some(event) = event_builder {
                    event.set_connector_response(&serde_json::json!({"error": "Error response parsing failed", "status_code": res.status_code}))
                };
                tracing::error!(deserialization_error =? error_msg);
                domain_types::utils::handle_json_response_deserialization_failure(res, "rapyd")
            }
        }
    }
}

/// Rapyd error codes that end a refund (spec errors Appendix A; plan P-foundation-08).
const RAPYD_REFUND_TERMINAL_ERROR_CODES: [&str; 4] = [
    "ERROR_REFUND_AMOUNT_EXCEEDS_PAYMENT_AMOUNT",
    "ERROR_CREATE_REFUND_CURRENCY_NOT_VALID",
    "ERROR_CREATE_REFUND",
    "ERROR_ADDING_REFUND",
];

macros::create_all_prerequisites!(
    connector_name: Rapyd,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: RapydPaymentsRequest<T>,
            response_body: RapydAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: RapydPSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: CaptureRequest,
            response_body: RapydCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            response_body: RapydVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: RapydRefundRequest,
            response_body: RefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: RapydRSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: ClientAuthenticationToken,
            request_body: RapydClientAuthRequest,
            response_body: RapydClientAuthResponse,
            router_data: RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ),
        (
            flow: CreateOrder,
            request_body: RapydCreateOrderRequest,
            response_body: RapydCreateOrderResponse,
            router_data: RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>,
        ),
        (
            flow: SetupMandate,
            request_body: RapydSetupMandateRequest<T>,
            response_body: RapydSetupMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: RapydRepeatPaymentRequest<T>,
            response_body: RapydRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: StringMajorUnit,
        amount_converter_webhooks: StringMinorUnit
    ],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
            http_method: &str,
            url_path: &str,
            body: &str,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            let auth = RapydAuthType::try_from(&req.connector_config)?;
            let timestamp = common_utils::date_time::now_unix_timestamp();
            let salt = common_utils::crypto::generate_cryptographically_secure_random_string(12);

            let signature = self.generate_signature(
                &auth,
                http_method,
                url_path,
                body,
                timestamp,
                &salt,
            )?;

            let headers = vec![
                (headers::CONTENT_TYPE.to_string(), "application/json".to_string().into()),
                ("access_key".to_string(), auth.access_key.into_masked()),
                ("salt".to_string(), salt.into()),
                ("timestamp".to_string(), timestamp.to_string().into()),
                ("signature".to_string(), signature.into()),
            ];
            Ok(headers)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.rapyd.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.rapyd.base_url
        }

        pub fn connector_base_url_merchant_auth<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, MerchantAuthenticationFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.rapyd.base_url
        }

        pub fn generate_signature(
            &self,
            auth: &RapydAuthType,
            http_method: &str,
            url_path: &str,
            body: &str,
            timestamp: i64,
            salt: &str,
        ) -> CustomResult<String, IntegrationError> {
            let RapydAuthType {
            access_key,
            secret_key
} = auth;
        let to_sign = format!(
            "{http_method}{url_path}{salt}{timestamp}{}{}{body}",
            access_key.peek(),
            secret_key.peek()
        );
        let key = hmac::Key::new(hmac::HMAC_SHA256, secret_key.peek().as_bytes());
        let tag = hmac::sign(&key, to_sign.as_bytes());
        let hmac_sign = hex::encode(tag);
        let signature_value = BASE64_ENGINE_URL_SAFE.encode(hmac_sign);
        Ok(signature_value)
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Rapyd,
    curl_request: Json(RapydPaymentsRequest),
    curl_response: RapydAuthorizeResponse,
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
            let url = self.get_url(req)?;
            // HMAC SHA-256 signs only the path component (everything after
            // the base URL). Falling back to the full URL here would produce
            // a signature Rapyd cannot verify, so treat a missing prefix as
            // a hard error instead of silently signing the wrong input.
            let url_path = url
                .strip_prefix(self.connector_base_url_payments(req))
                .ok_or(IntegrationError::RequestEncodingFailed {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "rapyd Authorize: computed URL did not start with the configured base URL; HMAC signature requires the exact path component"
                                .to_owned(),
                        ),
                        suggested_action: Some(
                            "Check the rapyd base_url in the connector configuration.".to_owned(),
                        ),
                        doc_url: Some("https://docs.rapyd.net/en/request-signatures.html".to_owned()),
                    },
                })?;
            // The signed body must be the exact body that is sent.
            let body = self
                .get_request_body(req)?
                .ok_or(IntegrationError::RequestEncodingFailed {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "rapyd Authorize: request body is required for HMAC signing"
                                .to_owned(),
                        ),
                        suggested_action: Some(
                            "Ensure the Authorize request carries a payment method.".to_owned(),
                        ),
                        doc_url: Some("https://docs.rapyd.net/en/request-signatures.html".to_owned()),
                    },
                })?
                .content
                .get_inner_value()
                .expose();
            self.build_headers(req, "post", url_path, &body)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/v1/payments", self.connector_base_url_payments(req)))
        }
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            let mut error = self.build_error_response(res, event_builder, connector_config)?;
            // A 4xx issuer decline or request rejection ends the attempt
            // (TH-06, B002). Transport errors (auth, signature, idempotency,
            // general, rate limit), 5xx and unparsable bodies leave it None:
            // the request may never have become a payment attempt.
            let is_transport =
                RapydErrorClass::classify(&error.code, None) == RapydErrorClass::Transport;
            let has_rapyd_error = error.code != NO_ERROR_CODE || error.network_decline_code.is_some();
            let ends_attempt =
                (400..500).contains(&error.status_code) && has_rapyd_error && !is_transport;
            if ends_attempt {
                error.attempt_status =
                    Some(FlowStatus::Payment(common_enums::AttemptStatus::Failure));
            }
            Ok(error)
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Rapyd,
    curl_response: RapydPSyncResponse,
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
            let url = self.get_url(req)?;
            // HMAC SHA-256 signs only the path component (everything after
            // the base URL). Falling back to the full URL here would produce
            // a signature Rapyd cannot verify, so treat a missing prefix as
            // a hard error instead of silently signing the wrong input.
            let url_path = url
                .strip_prefix(self.connector_base_url_payments(req))
                .ok_or(IntegrationError::RequestEncodingFailed {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "rapyd PSync: computed URL did not start with the configured base URL; HMAC signature requires the exact path component"
                                .to_owned(),
                        ),
                        suggested_action: Some(
                            "Check the rapyd base_url in the connector configuration.".to_owned(),
                        ),
                        doc_url: Some("https://docs.rapyd.net/en/request-signatures.html".to_owned()),
                    },
                })?;
            // A GET carries no body; Rapyd signs the empty string (never "{}").
            let body = "";
            self.build_headers(req, "get", url_path, body)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let id = req.request.get_connector_transaction_id()?;
            Ok(format!("{}/v1/payments/{}", self.connector_base_url_payments(req), id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Rapyd,
    curl_request: Json(CaptureRequest),
    curl_response: RapydCaptureResponse,
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
            let url = self.get_url(req)?;
            // HMAC SHA-256 signs only the path component (everything after
            // the base URL). Falling back to the full URL here would produce
            // a signature Rapyd cannot verify, so treat a missing prefix as
            // a hard error instead of silently signing the wrong input.
            let url_path = url
                .strip_prefix(self.connector_base_url_payments(req))
                .ok_or(IntegrationError::RequestEncodingFailed {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "rapyd Capture: computed URL did not start with the configured base URL; HMAC signature requires the exact path component"
                                .to_owned(),
                        ),
                        suggested_action: Some(
                            "Check the rapyd base_url in the connector configuration.".to_owned(),
                        ),
                        doc_url: Some("https://docs.rapyd.net/en/request-signatures.html".to_owned()),
                    },
                })?;
            // The signed body must be the exact body that is sent.
            let body = self
                .get_request_body(req)?
                .ok_or(IntegrationError::RequestEncodingFailed {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "rapyd Capture: request body is required for HMAC signing"
                                .to_owned(),
                        ),
                        suggested_action: Some(
                            "Ensure the Capture request carries an amount to capture.".to_owned(),
                        ),
                        doc_url: Some("https://docs.rapyd.net/en/request-signatures.html".to_owned()),
                    },
                })?
                .content
                .get_inner_value()
                .expose();
            self.build_headers(req, "post", url_path, &body)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let id = req.request.get_connector_transaction_id()?;
            Ok(format!("{}/v1/payments/{}/capture", self.connector_base_url_payments(req), id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Rapyd,
    curl_response: RapydVoidResponse,
    flow_name: Void,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentVoidData,
    flow_response: PaymentsResponseData,
    http_method: Delete,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let url = self.get_url(req)?;
            // HMAC SHA-256 signs only the path component (everything after
            // the base URL). Falling back to the full URL here would produce
            // a signature Rapyd cannot verify, so treat a missing prefix as
            // a hard error instead of silently signing the wrong input.
            let url_path = url
                .strip_prefix(self.connector_base_url_payments(req))
                .ok_or(IntegrationError::RequestEncodingFailed {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "rapyd Void: computed URL did not start with the configured base URL; HMAC signature requires the exact path component"
                                .to_owned(),
                        ),
                        suggested_action: Some(
                            "Check the rapyd base_url in the connector configuration.".to_owned(),
                        ),
                        doc_url: Some("https://docs.rapyd.net/en/request-signatures.html".to_owned()),
                    },
                })?;
            // A DELETE carries no body; Rapyd signs the empty string (never "{}").
            let body = "";
            self.build_headers(req, "delete", url_path, body)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/v1/payments/{}", self.connector_base_url_payments(req), req.request.connector_transaction_id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Rapyd,
    curl_request: Json(RapydRefundRequest),
    curl_response: RefundResponse,
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
            let url = self.get_url(req)?;
            // HMAC SHA-256 signs only the path component (everything after
            // the base URL). Falling back to the full URL here would produce
            // a signature Rapyd cannot verify, so treat a missing prefix as
            // a hard error instead of silently signing the wrong input.
            let url_path = url
                .strip_prefix(self.connector_base_url_refunds(req))
                .ok_or(IntegrationError::RequestEncodingFailed {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "rapyd Refund: computed URL did not start with the configured base URL; HMAC signature requires the exact path component"
                                .to_owned(),
                        ),
                        suggested_action: Some(
                            "Check the rapyd base_url in the connector configuration.".to_owned(),
                        ),
                        doc_url: Some("https://docs.rapyd.net/en/request-signatures.html".to_owned()),
                    },
                })?;
            // The signed body must be the exact body that is sent.
            let body = self
                .get_request_body(req)?
                .ok_or(IntegrationError::RequestEncodingFailed {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "rapyd Refund: request body is required for HMAC signing"
                                .to_owned(),
                        ),
                        suggested_action: Some(
                            "Ensure the Refund request carries the payment id, amount and currency.".to_owned(),
                        ),
                        doc_url: Some("https://docs.rapyd.net/en/request-signatures.html".to_owned()),
                    },
                })?
                .content
                .get_inner_value()
                .expose();
            self.build_headers(req, "post", url_path, &body)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/v1/refunds", self.connector_base_url_refunds(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Rapyd,
    curl_response: RapydRSyncResponse,
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
            let url = self.get_url(req)?;
            // HMAC SHA-256 signs only the path component (everything after
            // the base URL). Falling back to the full URL here would produce
            // a signature Rapyd cannot verify, so treat a missing prefix as
            // a hard error instead of silently signing the wrong input.
            let url_path = url
                .strip_prefix(self.connector_base_url_refunds(req))
                .ok_or(IntegrationError::RequestEncodingFailed {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "rapyd RSync: computed URL did not start with the configured base URL; HMAC signature requires the exact path component"
                                .to_owned(),
                        ),
                        suggested_action: Some(
                            "Check the rapyd base_url in the connector configuration.".to_owned(),
                        ),
                        doc_url: Some("https://docs.rapyd.net/en/request-signatures.html".to_owned()),
                    },
                })?;
            // A GET carries no body; Rapyd signs the empty string (never "{}").
            let body = "";
            self.build_headers(req, "get", url_path, body)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/v1/refunds/{}", self.connector_base_url_refunds(req), req.request.connector_refund_id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Rapyd,
    curl_request: Json(RapydCreateOrderRequest),
    curl_response: RapydCreateOrderResponse,
    flow_name: CreateOrder,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentCreateOrderData,
    flow_response: PaymentCreateOrderResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let url = self.get_url(req)?;
            let url_path = url.strip_prefix(self.connector_base_url_payments(req))
                .unwrap_or(&url);
            let body = self.get_request_body(req)?
                .map(|content| content.content.get_inner_value().expose())
                .unwrap_or_default();
            self.build_headers(req, "post", url_path, &body)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/v1/checkout", self.connector_base_url_payments(req)))
        }
    }
);

// SetupMandate flow – reuses the standard `/v1/payments` endpoint for
// card-on-file verification. The returned payment id is surfaced as the
// connector_mandate_id for subsequent RepeatPayment (MIT) calls.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Rapyd,
    curl_request: Json(RapydSetupMandateRequest),
    curl_response: RapydSetupMandateResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let url = self.get_url(req)?;
            // HMAC SHA-256 signs only the path component (everything after
            // the base URL). Falling back to the full URL here would produce
            // a signature Rapyd cannot verify, so treat a missing prefix as
            // a hard error instead of silently signing the wrong input.
            let url_path = url
                .strip_prefix(self.connector_base_url_payments(req))
                .ok_or(IntegrationError::RequestEncodingFailed {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "rapyd SetupMandate: computed URL did not start with the configured base URL; HMAC signature requires the exact path component"
                                .to_owned(),
                        ),
                        ..Default::default()
                    },
                })?;
            let body = self
                .get_request_body(req)?
                .ok_or(IntegrationError::RequestEncodingFailed {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "rapyd SetupMandate: request body is required for HMAC signing"
                                .to_owned(),
                        ),
                        ..Default::default()
                    },
                })?
                .content
                .get_inner_value()
                .expose();
            self.build_headers(req, "post", url_path, &body)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // Reuse /v1/payments — `save_payment_method: true` + inline customer
            // object in the body yields a reusable `card_*` token without
            // requiring the complete_payment_url whitelist that the
            // /v1/customers endpoint enforces on sandbox accounts.
            Ok(format!("{}/v1/payments", self.connector_base_url_payments(req)))
        }
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            let mut error = self.build_error_response(res, event_builder, connector_config)?;
            // The card-save CIT is a payment attempt: a 4xx issuer decline or
            // request rejection ends it (TH-06), exactly as on Authorize.
            // Transport errors (auth, signature, idempotency, general, rate
            // limit), 5xx and unparsable bodies leave it None.
            let is_transport =
                RapydErrorClass::classify(&error.code, None) == RapydErrorClass::Transport;
            let has_rapyd_error = error.code != NO_ERROR_CODE || error.network_decline_code.is_some();
            let ends_attempt =
                (400..500).contains(&error.status_code) && has_rapyd_error && !is_transport;
            if ends_attempt {
                error.attempt_status =
                    Some(FlowStatus::Payment(common_enums::AttemptStatus::Failure));
            }
            Ok(error)
        }
    }
);

// RepeatPayment (MIT) – Rapyd has no dedicated recurring endpoint. It reuses
// `/v1/payments` with either the stored `payment_method` token (the card_* id
// returned by SetupMandate) or card details plus the original network
// reference id, and an `initiation_type` derived from the MIT category.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Rapyd,
    curl_request: Json(RapydRepeatPaymentRequest),
    curl_response: RapydRepeatPaymentResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let url = self.get_url(req)?;
            // HMAC SHA-256 signs only the path component (everything after
            // the base URL). Falling back to the full URL here would produce
            // a signature Rapyd cannot verify, so treat a missing prefix as
            // a hard error instead of silently signing the wrong input.
            let url_path = url
                .strip_prefix(self.connector_base_url_payments(req))
                .ok_or(IntegrationError::RequestEncodingFailed {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "rapyd RepeatPayment: computed URL did not start with the configured base URL; HMAC signature requires the exact path component"
                                .to_owned(),
                        ),
                        ..Default::default()
                    },
                })?;
            let body = self
                .get_request_body(req)?
                .ok_or(IntegrationError::RequestEncodingFailed {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "rapyd RepeatPayment: request body is required for HMAC signing"
                                .to_owned(),
                        ),
                        ..Default::default()
                    },
                })?
                .content
                .get_inner_value()
                .expose();
            self.build_headers(req, "post", url_path, &body)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/v1/payments", self.connector_base_url_payments(req)))
        }
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            let mut error = self.build_error_response(res, event_builder, connector_config)?;
            // A MIT is a payment attempt: a 4xx issuer decline or request
            // rejection ends it (TH-06), exactly as on Authorize, and carries
            // the merchant advice code from the shared error fields. Transport
            // errors (auth, signature, idempotency, general, rate limit), 5xx
            // and unparsable bodies leave it None.
            let is_transport =
                RapydErrorClass::classify(&error.code, None) == RapydErrorClass::Transport;
            let has_rapyd_error = error.code != NO_ERROR_CODE || error.network_decline_code.is_some();
            let ends_attempt =
                (400..500).contains(&error.status_code) && has_rapyd_error && !is_transport;
            if ends_attempt {
                error.attempt_status =
                    Some(FlowStatus::Payment(common_enums::AttemptStatus::Failure));
            }
            Ok(error)
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Rapyd,
    curl_request: Json(RapydClientAuthRequest),
    curl_response: RapydClientAuthResponse,
    flow_name: ClientAuthenticationToken,
    resource_common_data: MerchantAuthenticationFlowData,
    flow_request: ClientAuthenticationTokenRequestData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let url = self.get_url(req)?;
            let url_path = url.strip_prefix(self.connector_base_url_merchant_auth(req))
                .unwrap_or(&url);
            let body = self.get_request_body(req)?
                .map(|content| content.content.get_inner_value().expose())
                .unwrap_or_default();
            self.build_headers(req, "post", url_path, &body)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/v1/checkout", self.connector_base_url_merchant_auth(req)))
        }
    }
);

macros::macro_connector_flow_status_impls!(
    connector: Rapyd,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        IncrementalAuthorization,
        PaymentMethodToken,
        SubmitEvidence,
        DefendDispute,
        Accept,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        MandateRevoke,
    ],
    not_supported: [
        VoidPostRefund,
        VoidPC,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
    ],
);
