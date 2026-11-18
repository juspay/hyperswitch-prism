pub mod transformers;

#[cfg(test)]
mod test;

use base64::Engine;
use common_utils::{
    crypto::VerifySignature, errors::CustomResult, events, ext_traits::ByteSliceExt,
    StringMajorUnit,
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
        RequestDetails, SetupMandateRequestData, WebhookDetailsResponse, WebhookResourceReference,
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
    RapydCreateOrderRequest, RapydCreateOrderResponse, RapydErrorResponse, RapydPSyncResponse,
    RapydPaymentsRequest, RapydPaymentsResponse as RapydCaptureResponse,
    RapydPaymentsResponse as RapydVoidResponse, RapydPaymentsResponse as RapydAuthorizeResponse,
    RapydRefundRequest, RapydRepeatPaymentRequest, RapydRepeatPaymentResponse,
    RapydSetupMandateRequest, RapydSetupMandateResponse, RefundResponse,
    RefundResponse as RapydRSyncResponse,
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
    fn validate_psync_reference_id(
        &self,
        data: &PaymentsSyncData,
        _payment_flow_data: &PaymentFlowData,
    ) -> CustomResult<(), IntegrationError> {
        connector_types::validate_psync_connector_transaction_id(data)
    }
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
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::webhooks::IncomingWebhook for Rapyd<T>
{
    fn get_event_type(&self, request: RequestDetails) -> Result<EventType, Report<WebhookError>> {
        let event: transformers::RapydIncomingWebhook = request
            .body
            .parse_struct("RapydIncomingWebhook")
            .change_context(WebhookError::WebhookEventTypeNotFound)?;
        event.try_into()
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, Report<WebhookError>> {
        let event: transformers::RapydIncomingWebhook = request
            .body
            .parse_struct("RapydIncomingWebhook")
            .change_context(WebhookError::WebhookReferenceIdNotFound)?;
        use transformers::RapydWebhookEventType;
        Ok(match event.webhook_type {
            RapydWebhookEventType::PaymentSucceeded
            | RapydWebhookEventType::PaymentCompleted
            | RapydWebhookEventType::PaymentCaptured
            | RapydWebhookEventType::PaymentCanceled
            | RapydWebhookEventType::PaymentExpired
            | RapydWebhookEventType::PaymentFailed => {
                let data = event.payment_data()?;
                Some(WebhookResourceReference::Payment(PaymentWebhookReference {
                    connector_transaction_id: Some(data.id),
                    merchant_transaction_id: None,
                    connector_preprocessing_id: None,
                }))
            }
            RapydWebhookEventType::RefundCompleted
            | RapydWebhookEventType::PaymentRefundRejected
            | RapydWebhookEventType::PaymentRefundFailed => {
                let data = event.refund_data()?;
                Some(WebhookResourceReference::Refund(RefundWebhookReference {
                    connector_refund_id: Some(data.id),
                    connector_transaction_id: Some(data.payment),
                    merchant_refund_id: None,
                    merchant_transaction_id: None,
                }))
            }
            RapydWebhookEventType::PaymentDisputeCreated
            | RapydWebhookEventType::PaymentDisputeUpdated => {
                let data = event.dispute_data()?;
                Some(WebhookResourceReference::Dispute(DisputeWebhookReference {
                    connector_dispute_id: Some(data.token),
                    connector_transaction_id: Some(data.original_transaction_id),
                }))
            }
            RapydWebhookEventType::Unknown => None,
        })
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, Report<WebhookError>> {
        let event: transformers::RapydIncomingWebhook = request
            .body
            .parse_struct("RapydIncomingWebhook")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        event.payment_data()?.into_response(&request.body)
    }

    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, Report<WebhookError>> {
        let event: transformers::RapydIncomingWebhook = request
            .body
            .parse_struct("RapydIncomingWebhook")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        use transformers::{RapydWebhookEventType, RefundStatus};
        let status = match event.webhook_type {
            RapydWebhookEventType::RefundCompleted => RefundStatus::Completed,
            RapydWebhookEventType::PaymentRefundRejected => RefundStatus::Rejected,
            RapydWebhookEventType::PaymentRefundFailed => RefundStatus::Error,
            _ => return Err(WebhookError::WebhookEventTypeNotFound.into()),
        };
        let data = event.refund_data()?;
        let error_message = if matches!(status, RefundStatus::Error | RefundStatus::Rejected) {
            data.failure_reason
        } else {
            None
        };
        Ok(RefundWebhookDetailsResponse {
            raw_connector_status: None,
            connector_refund_id: Some(data.id),
            merchant_transaction_id: None,
            status: status.into(),
            connector_response_reference_id: None,
            error_code: None,
            error_message,
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).into_owned()),
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
        let event: transformers::RapydIncomingWebhook = request
            .body
            .parse_struct("RapydIncomingWebhook")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        let data = event.dispute_data()?;
        // Rapyd sends dispute amounts in major units, including decimal amounts.
        let money = domain_types::utils::convert_back_amount_to_money_for_webhook(
            &common_utils::types::FloatMajorUnitForConnector,
            data.amount,
            data.currency,
        )?;
        let amount = domain_types::utils::convert_amount_for_webhook(
            &common_utils::types::StringMinorUnitForConnector,
            &money,
        )?;
        Ok(DisputeWebhookDetailsResponse {
            amount,
            currency: data.currency,
            dispute_id: data.token,
            status: data.status.try_into()?,
            stage: common_enums::DisputeStage::Dispute,
            connector_response_reference_id: None,
            dispute_message: data.dispute_reason_description,
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).into_owned()),
            status_code: 200,
            response_headers: None,
            connector_reason_code: None,
            additional_details: None,
        })
    }

    fn get_webhook_source_verification_signature(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, Report<WebhookError>> {
        let signature = request
            .headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("signature"))
            .map(|(_, value)| value)
            .ok_or(WebhookError::WebhookSignatureNotFound)?;
        let encoded_hex = BASE64_ENGINE_URL_SAFE
            .decode(signature)
            .change_context(WebhookError::WebhookSourceVerificationFailed)?;
        hex::decode(encoded_hex).change_context(WebhookError::WebhookSourceVerificationFailed)
    }

    fn get_webhook_source_verification_message(
        &self,
        request: &RequestDetails,
        connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, Report<WebhookError>> {
        let auth: RapydAuthType = serde_json::from_slice(&connector_webhook_secret.secret)
            .change_context(WebhookError::WebhookVerificationSecretInvalid)?;
        build_webhook_verification_message(request, &auth)
    }

    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, Report<WebhookError>> {
        let secrets =
            connector_webhook_secret.ok_or(WebhookError::WebhookVerificationSecretNotFound)?;
        let auth: RapydAuthType = serde_json::from_slice(&secrets.secret)
            .change_context(WebhookError::WebhookVerificationSecretInvalid)?;
        let message = build_webhook_verification_message(&request, &auth)?;
        let signature = self.get_webhook_source_verification_signature(&request, &secrets)?;
        common_utils::crypto::HmacSha256
            .verify_signature(auth.secret_key.peek().as_bytes(), &signature, &message)
            .change_context(WebhookError::WebhookSourceVerificationFailed)
    }
}

fn build_webhook_verification_message(
    request: &RequestDetails,
    auth: &RapydAuthType,
) -> Result<Vec<u8>, Report<WebhookError>> {
    if auth.access_key.peek().is_empty() || auth.secret_key.peek().is_empty() {
        return Err(WebhookError::WebhookVerificationSecretInvalid.into());
    }
    let header = |key: &'static str| {
        request
            .headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(key))
            .map(|(_, value)| value.as_str())
            .ok_or(WebhookError::WebhookMissingRequiredField { field: key })
    };
    let uri = request
        .uri
        .as_deref()
        .filter(|uri| !uri.is_empty())
        .ok_or(WebhookError::WebhookMissingRequiredField { field: "uri" })?;
    let mut url = if uri.starts_with("https://") || uri.starts_with("http://") {
        uri.to_string()
    } else if uri.starts_with('/') {
        // HS forwards a relative URI and uses HTTPS for the public callback.
        format!("https://{}{uri}", header("host")?)
    } else {
        return Err(WebhookError::WebhookSourceVerificationFailed.into());
    };
    if !uri.contains('?') {
        if let Some(query) = request
            .query_params
            .as_deref()
            .filter(|query| !query.is_empty())
        {
            url.push('?');
            url.push_str(query);
        }
    }
    let mut message = format!(
        "{url}{}{}{}{}",
        header("salt")?,
        header("timestamp")?,
        auth.access_key.peek(),
        auth.secret_key.peek(),
    )
    .into_bytes();
    message.extend_from_slice(&request.body);
    Ok(message)
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
        let response: Result<RapydErrorResponse, Report<common_utils::errors::ParsingError>> =
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
        amount_converter: StringMajorUnit
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
// card-on-file verification. The saved payment method ID is surfaced as the
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
// `payment_method` token (the card_* id returned by SetupMandate) with
// `initiation_type: recurring`.
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
