pub mod transformers;

#[cfg(test)]
mod test;

use base64::Engine;
use common_utils::{
    crypto::VerifySignature,
    errors::CustomResult,
    events,
    ext_traits::{ByteSliceExt, Encode},
    FloatMajorUnitForConnector, StringMajorUnit, StringMinorUnit,
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, CreateOrder, PSync, RSync, Refund,
        RepeatPayment, SetupMandate, Void,
    },
    connector_types::{
        ClientAuthenticationTokenRequestData, ConnectorWebhookSecrets,
        DisputeWebhookDetailsResponse, DisputeWebhookReference, EventType, PaymentCreateOrderData,
        PaymentCreateOrderResponse, PaymentFlowData, PaymentVoidData, PaymentWebhookReference,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundWebhookDetailsResponse, RefundWebhookReference,
        RefundsData, RefundsResponseData, RepeatPaymentData, RequestDetails, ResponseId,
        SetupMandateRequestData, WebhookDetailsResponse, WebhookResourceReference,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{ExposeInterface, Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use ring::hmac;
use serde::Serialize;
use std::fmt::Debug;
use transformers::{
    CaptureRequest, RapydAuthType, RapydClientAuthRequest, RapydClientAuthResponse,
    RapydCreateOrderRequest, RapydCreateOrderResponse, RapydPaymentsRequest,
    RapydPaymentsResponse as RapydCaptureResponse, RapydPaymentsResponse as RapydPSyncResponse,
    RapydPaymentsResponse, RapydPaymentsResponse as RapydVoidResponse,
    RapydPaymentsResponse as RapydAuthorizeResponse, RapydRefundRequest, RapydRepeatPaymentRequest,
    RapydRepeatPaymentResponse, RapydSetupMandateRequest, RapydSetupMandateResponse,
    RefundResponse, RefundResponse as RapydRSyncResponse,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};
use domain_types::errors::ConnectorError;
use domain_types::errors::{IntegrationError, IntegrationErrorContext, WebhookError};

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
/// Reads a webhook header case-insensitively; Rapyd sends them lowercase but the
/// caller's header map is not normalised.
fn get_webhook_header<'a>(
    headers: &'a std::collections::HashMap<String, String>,
    header_name: &'static str,
) -> Result<&'a str, Report<WebhookError>> {
    headers
        .iter()
        .find_map(|(key, value)| {
            key.eq_ignore_ascii_case(header_name)
                .then_some(value.as_str())
        })
        .ok_or_else(|| {
            error_stack::report!(WebhookError::WebhookMissingRequiredField { field: header_name })
        })
}

/// Rebuilds the `url_path` component of the Rapyd webhook preimage.
///
/// Rapyd signs the **entire configured webhook URL**. Hyperswitch built it as
/// `https://{host}/webhooks/{merchant_id}/rapyd`, but UCS's `verify_webhook_source`
/// receives no merchant id, so the URL is reconstructed from what the caller
/// actually forwards: `RequestDetails.uri` plus the `host` header.
///
/// * If `uri` is already absolute, it is used verbatim (query string stripped).
/// * If `uri` is a bare path, it is composed onto `https://{host}`.
///
/// Precedent: `grabpay_webhook_path` in `connectors/grabpay.rs`.
fn rapyd_webhook_url_path(request: &RequestDetails) -> Result<String, Report<WebhookError>> {
    let uri = request.uri.as_deref().ok_or_else(|| {
        error_stack::report!(WebhookError::WebhookMissingRequiredField { field: "uri" })
    })?;

    if let Ok(url) = url::Url::parse(uri) {
        let mut absolute = url.clone();
        absolute.set_query(None);
        absolute.set_fragment(None);
        return Ok(absolute.as_str().trim_end_matches('/').to_string());
    }

    let host = get_webhook_header(&request.headers, "host")?;
    let path = uri.split('?').next().unwrap_or(uri);
    Ok(format!("https://{host}{path}"))
}

/// Assembles the Rapyd webhook HMAC preimage.
///
/// Exact component order, no separators:
/// `url_path + salt + timestamp + access_key + secret_key + body_string`.
/// `body_string` must be the RAW received bytes — re-serialising the JSON
/// changes the whitespace and breaks verification.
fn rapyd_webhook_preimage(
    url_path: &str,
    salt: &str,
    timestamp: &str,
    access_key: &str,
    secret_key: &str,
    body_string: &str,
) -> String {
    format!("{url_path}{salt}{timestamp}{access_key}{secret_key}{body_string}")
}

fn parse_rapyd_webhook(
    body: &[u8],
) -> Result<transformers::RapydIncomingWebhook, Report<WebhookError>> {
    body.parse_struct::<transformers::RapydIncomingWebhook>("RapydIncomingWebhook")
        .change_context(WebhookError::WebhookBodyDecodingFailed)
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Rapyd<T>
{
    fn get_event_type(&self, request: RequestDetails) -> Result<EventType, Report<WebhookError>> {
        let webhook = parse_rapyd_webhook(&request.body)
            .change_context(WebhookError::WebhookEventTypeNotFound)?;
        Ok(transformers::get_webhook_event_type(&webhook))
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, Report<WebhookError>> {
        let webhook = parse_rapyd_webhook(&request.body)
            .change_context(WebhookError::WebhookReferenceIdNotFound)?;

        if matches!(
            webhook.webhook_type,
            transformers::RapydWebhookObjectEventType::Unknown
        ) {
            return Ok(None);
        }

        let reference = match webhook.data {
            transformers::WebhookData::Payment(payment_data) => {
                WebhookResourceReference::Payment(PaymentWebhookReference {
                    connector_transaction_id: Some(payment_data.id),
                    merchant_transaction_id: transformers::non_empty(
                        payment_data.merchant_reference_id,
                    ),
                })
            }
            transformers::WebhookData::Refund(refund_data) => {
                WebhookResourceReference::Refund(RefundWebhookReference {
                    connector_refund_id: Some(refund_data.id),
                    merchant_refund_id: None,
                    connector_transaction_id: Some(refund_data.payment),
                    merchant_transaction_id: None,
                })
            }
            transformers::WebhookData::Dispute(dispute_data) => {
                WebhookResourceReference::Dispute(DisputeWebhookReference {
                    connector_dispute_id: Some(dispute_data.token),
                    connector_transaction_id: Some(dispute_data.original_transaction_id),
                })
            }
        };

        Ok(Some(reference))
    }

    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, Report<WebhookError>> {
        // Rapyd has no separate webhook secret: the organization access_key /
        // secret_key pair that signs outbound requests also signs webhooks.
        let connector_config = connector_account_details
            .ok_or_else(|| error_stack::report!(WebhookError::WebhookVerificationSecretNotFound))?;
        let auth = RapydAuthType::try_from(&connector_config)
            .change_context(WebhookError::WebhookVerificationSecretInvalid)?;

        let signature_header = get_webhook_header(&request.headers, "signature")
            .change_context(WebhookError::WebhookSignatureNotFound)?;
        let salt = get_webhook_header(&request.headers, "salt")?;
        let timestamp = get_webhook_header(&request.headers, "timestamp")?;

        let url_path = rapyd_webhook_url_path(&request)?;

        // Rapyd double-encodes: the HMAC digest is hex-encoded to 64 ASCII
        // characters and only then base64-encoded (URL-safe alphabet) into the
        // header. Undo both to recover the 32 raw digest bytes.
        let hex_ascii = BASE64_ENGINE_URL_SAFE
            .decode(signature_header.as_bytes())
            .change_context(WebhookError::WebhookSourceVerificationFailed)?;
        let signature =
            hex::decode(hex_ascii).change_context(WebhookError::WebhookSourceVerificationFailed)?;

        let body_string = String::from_utf8(request.body.clone())
            .change_context(WebhookError::WebhookSourceVerificationFailed)?;

        let message = rapyd_webhook_preimage(
            &url_path,
            salt,
            timestamp,
            auth.access_key.peek(),
            auth.secret_key.peek(),
            &body_string,
        );

        // `HmacSha256::verify_signature` uses ring's constant-time comparison.
        common_utils::crypto::HmacSha256
            .verify_signature(
                auth.secret_key.peek().as_bytes(),
                &signature,
                message.as_bytes(),
            )
            .change_context(WebhookError::WebhookSourceVerificationFailed)
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<domain_types::connector_types::EventContext>,
    ) -> Result<WebhookDetailsResponse, Report<WebhookError>> {
        let webhook = parse_rapyd_webhook(&request.body)?;

        let data = match webhook.data {
            transformers::WebhookData::Payment(payment_data) => payment_data,
            transformers::WebhookData::Refund(_) | transformers::WebhookData::Dispute(_) => {
                return Err(
                    error_stack::report!(WebhookError::WebhookResourceObjectNotFound)
                        .attach_printable("Rapyd payment webhook did not carry a payment object"),
                )
            }
        };

        let status = transformers::get_status_for_webhook(&data);

        Ok(WebhookDetailsResponse {
            resource_id: Some(ResponseId::ConnectorTransactionId(data.id.clone())),
            status,
            connector_response_reference_id: transformers::non_empty(
                data.merchant_reference_id.clone(),
            ),
            connector_request_reference_id: None,
            mandate_reference: None,
            error_code: transformers::non_empty(data.failure_code.clone()),
            error_message: transformers::non_empty(data.failure_message.clone()),
            error_reason: None,
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
            amount_captured: None,
            minor_amount_captured: None,
            network_txn_id: data
                .payment_method_data
                .as_ref()
                .and_then(|pmd| pmd.network_reference_id.as_ref())
                .map(|reference| reference.peek().to_owned()),
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
        let webhook = parse_rapyd_webhook(&request.body)?;

        let data = match webhook.data {
            transformers::WebhookData::Refund(refund_data) => refund_data,
            transformers::WebhookData::Payment(_) | transformers::WebhookData::Dispute(_) => {
                return Err(
                    error_stack::report!(WebhookError::WebhookResourceObjectNotFound)
                        .attach_printable("Rapyd refund webhook did not carry a refund object"),
                )
            }
        };

        Ok(RefundWebhookDetailsResponse {
            connector_refund_id: Some(data.id.clone()),
            merchant_transaction_id: None,
            status: common_enums::RefundStatus::from(data.status.clone()),
            connector_response_reference_id: Some(data.payment.clone()),
            error_code: transformers::non_empty(data.failure_code.clone()),
            error_message: transformers::non_empty(data.failure_reason.clone()),
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
        let webhook = parse_rapyd_webhook(&request.body)?;

        let data = match webhook.data {
            transformers::WebhookData::Dispute(dispute_data) => dispute_data,
            transformers::WebhookData::Payment(_) | transformers::WebhookData::Refund(_) => {
                return Err(
                    error_stack::report!(WebhookError::WebhookResourceObjectNotFound)
                        .attach_printable("Rapyd dispute webhook did not carry a dispute object"),
                )
            }
        };

        // Rapyd sends dispute amounts in major units (`"amount": 10` with
        // `"currency": "USD"` is ten dollars), consistent with every other
        // amount it sends. Convert major -> minor -> the response's
        // StringMinorUnit.
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

        Ok(DisputeWebhookDetailsResponse {
            amount,
            currency: data.currency,
            // The `dispute_*` token, not `data.id` (an internal UUID).
            dispute_id: data.token.clone(),
            status: data.status.to_dispute_status()?,
            stage: if data.pre_dispute.unwrap_or(false) {
                common_enums::DisputeStage::PreDispute
            } else {
                common_enums::DisputeStage::Dispute
            },
            connector_response_reference_id: Some(data.original_transaction_id.clone()),
            dispute_message: Some(data.dispute_reason_description.clone()),
            connector_reason_code: None,
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
        })
    }

    fn get_webhook_resource_object(
        &self,
        request: RequestDetails,
    ) -> Result<Box<dyn hyperswitch_masking::ErasedMaskSerialize>, Report<WebhookError>> {
        let webhook = parse_rapyd_webhook(&request.body)
            .change_context(WebhookError::WebhookResourceObjectNotFound)?;

        // Re-encode the `data` object in the shape the flow handlers already
        // understand, so the caller can feed it back through the existing
        // `TryFrom<ResponseRouterData<..>>` impls.
        let resource = match webhook.data {
            transformers::WebhookData::Payment(payment_data) => {
                RapydPaymentsResponse::from(payment_data)
                    .encode_to_value()
                    .change_context(WebhookError::WebhookResourceObjectNotFound)?
            }
            transformers::WebhookData::Refund(refund_data) => RefundResponse::from(refund_data)
                .encode_to_value()
                .change_context(WebhookError::WebhookResourceObjectNotFound)?,
            transformers::WebhookData::Dispute(dispute_data) => dispute_data
                .encode_to_value()
                .change_context(WebhookError::WebhookResourceObjectNotFound)?,
        };

        Ok(Box::new(resource))
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
        let response: Result<RapydPaymentsResponse, Report<common_utils::errors::ParsingError>> =
            res.response.parse_struct("rapyd ErrorResponse");

        match response {
            Ok(response_data) => {
                with_error_response_body!(event_builder, response_data);
                let typed = macros::serialize_typed_connector_payload(
                    &response_data,
                    "typed_connector_response",
                );
                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: response_data.status.error_code,
                    message: response_data.status.status.unwrap_or_default(),
                    reason: response_data.status.message,
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
    connector_default_implementations: [get_content_type, get_error_response_v2],
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
            let url_path = url.strip_prefix(self.connector_base_url_payments(req))
                .unwrap_or(&url);
            // Get the exact request body that will be sent
            let body = self.get_request_body(req)?
                .map(|content| content.content.get_inner_value().expose())
                .unwrap_or_default();
            self.build_headers(req, "post", url_path, &body)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/v1/payments", self.connector_base_url_payments(req)))
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
            let url_path = url.strip_prefix(self.connector_base_url_payments(req))
                .unwrap_or(&url);
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
            let url_path = url.strip_prefix(self.connector_base_url_payments(req))
                .unwrap_or(&url);
            let body = self.get_request_body(req)?
                .map(|content| content.content.get_inner_value().expose())
                .unwrap_or_default();
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
            let url_path = url.strip_prefix(self.connector_base_url_payments(req))
                .unwrap_or(&url);
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
            let url_path = url.strip_prefix(self.connector_base_url_refunds(req))
                .unwrap_or(&url);
            let body = self.get_request_body(req)?
                .map(|content| content.content.get_inner_value().expose())
                .unwrap_or_default();
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
            let url_path = url.strip_prefix(self.connector_base_url_refunds(req))
                .unwrap_or(&url);
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
    connector_default_implementations: [get_content_type, get_error_response_v2],
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
    }
);

// RepeatPayment (MIT) – Rapyd has no dedicated recurring endpoint. It reuses
// `/v1/payments` but substitutes the card object with a stored
// `payment_method` token (the card_* id returned by SetupMandate) paired
// with the `customer` id and `initiation_type: recurring`.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
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
