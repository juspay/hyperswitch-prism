pub mod transformers;

use std::fmt::Debug;

use common_enums::{AttemptStatus, CurrencyUnit};
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
};
use domain_types::{
    connector_flow::{
        Authorize, PSync, RSync, Refund, ServerAuthenticationToken, VerifyWebhookSource,
    },
    connector_types::*,
    errors,
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, FlowStatus},
    router_data_v2::RouterDataV2,
    router_request_types::VerifyWebhookSourceRequestData,
    router_response_types::{Response, VerifyWebhookSourceResponseData},
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding,
};
use serde::Serialize;
use transformers as fire;
use transformers::{
    FireAuthorizeRequest, FireCreatePaymentRequestResponse, FirePaymentAttemptsResponse,
    FireRefundRequest, FireRefundResponse, FireServerAuthenticationTokenRequest,
    FireServerAuthenticationTokenResponse, FireTransactionDetails,
};

use crate::{
    connectors::macros, finalize_connector_response, types::ResponseRouterData,
    with_error_response_body,
};

const FIRE_AUTH_ENDPOINT: &str = "/business/v1/apps/accesstokens";
const FIRE_CREATE_PAYMENT_REQUEST_ENDPOINT: &str = "/business/v1/paymentrequests";
const FIRE_PAYMENT_REQUEST_PAYMENTS_ENDPOINT: &str = "/business/v2/paymentrequests";
const FIRE_REFUND_ENDPOINT_PREFIX: &str = "/business/v1/payments/";
const FIRE_REFUND_ENDPOINT_SUFFIX: &str = "/bankpayrefund";
// Get Payment Details v2 — the same transaction API serves payments and
// refunds (transactionType "PAYMENT" vs "REFUND" / "REFUND_REQUEST").
const FIRE_PAYMENT_DETAILS_ENDPOINT_PREFIX: &str = "/business/v2/payments/";
const CONTENT_TYPE_HEADER: &str = "Content-Type";
const AUTHORIZATION_HEADER: &str = "Authorization";
const FIRE_CONTENT_TYPE: &str = "application/json";
const FIRE_BEARER_AUTH_SCHEME: &str = "Bearer";

macros::create_all_prerequisites!(
    connector_name: Fire,
    generic_type: T,
    api: [
        (
            flow: ServerAuthenticationToken,
            request_body: FireServerAuthenticationTokenRequest,
            response_body: FireServerAuthenticationTokenResponse,
            router_data: RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
        ),
        (
            flow: Authorize,
            request_body: FireAuthorizeRequest,
            response_body: FireCreatePaymentRequestResponse,
            router_data: RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        ),
        (
            flow: PSync,
            response_body: FirePaymentAttemptsResponse,
            router_data: RouterDataV2<
                PSync,
                PaymentFlowData,
                PaymentsSyncData,
                PaymentsResponseData,
            >,
        ),
        (
            flow: Refund,
            request_body: FireRefundRequest,
            response_body: FireRefundResponse,
            router_data: RouterDataV2<
                Refund,
                RefundFlowData,
                RefundsData,
                RefundsResponseData,
            >,
        ),
        (
            flow: RSync,
            response_body: FireTransactionDetails,
            router_data: RouterDataV2<
                RSync,
                RefundFlowData,
                RefundSyncData,
                RefundsResponseData,
            >,
        )
    ],
    amount_converters: [],
    member_functions: {}
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Fire<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Fire<T>
{
    fn validate_psync_reference_id(
        &self,
        data: &PaymentsSyncData,
        _payment_flow_data: &PaymentFlowData,
    ) -> CustomResult<(), errors::IntegrationError> {
        fire::payment_request_code_from_feature_data(data.connector_feature_data.as_ref())?;
        Ok(())
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Fire<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Fire<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerAuthentication for Fire<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Fire,
    curl_request: Json(FireServerAuthenticationTokenRequest),
    curl_response: FireServerAuthenticationTokenResponse,
    flow_name: ServerAuthenticationToken,
    resource_common_data: MerchantAuthenticationFlowData,
    flow_request: ServerAuthenticationTokenRequestData,
    flow_response: ServerAuthenticationTokenResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = self.base_url(&req.resource_common_data.connectors);
            Ok(format!("{base_url}{FIRE_AUTH_ENDPOINT}"))
        }

        fn get_headers(
            &self,
            _req: &RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            Ok(vec![(
                CONTENT_TYPE_HEADER.to_string(),
                self.common_get_content_type().to_string().into(),
            )])
        }

        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<domain_types::router_data::ErrorResponse, errors::ConnectorError> {
            self.build_error_response(res, event_builder, _connector_config)
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Fire,
    curl_response: FirePaymentAttemptsResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<
                PSync,
                PaymentFlowData,
                PaymentsSyncData,
                PaymentsResponseData,
            >,
        ) -> CustomResult<String, errors::IntegrationError> {
            let code = fire::payment_request_code_from_feature_data(
                req.request.connector_feature_data.as_ref(),
            )?;
            let base_url = self.base_url(&req.resource_common_data.connectors);
            Ok(format!(
                "{base_url}{FIRE_PAYMENT_REQUEST_PAYMENTS_ENDPOINT}/{code}/payments"
            ))
        }

        fn get_headers(
            &self,
            req: &RouterDataV2<
                PSync,
                PaymentFlowData,
                PaymentsSyncData,
                PaymentsResponseData,
            >,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            let access_token = req.resource_common_data.get_access_token()?;
            Ok(vec![
                (CONTENT_TYPE_HEADER.to_string(), self.common_get_content_type().to_string().into()),
                (
                    AUTHORIZATION_HEADER.to_string(),
                    format!("{FIRE_BEARER_AUTH_SCHEME} {access_token}").into_masked(),
                ),
            ])
        }

        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<domain_types::router_data::ErrorResponse, errors::ConnectorError> {
            self.build_error_response(res, event_builder, _connector_config)
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Fire,
    curl_request: Json(FireAuthorizeRequest),
    curl_response: FireCreatePaymentRequestResponse,
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
            req: &RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = self.base_url(&req.resource_common_data.connectors);
            Ok(format!("{base_url}{FIRE_CREATE_PAYMENT_REQUEST_ENDPOINT}"))
        }

        fn get_headers(
            &self,
            req: &RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            let access_token = req.resource_common_data.get_access_token()?;
            Ok(vec![
                (CONTENT_TYPE_HEADER.to_string(), self.common_get_content_type().to_string().into()),
                (
                    AUTHORIZATION_HEADER.to_string(),
                    format!("{FIRE_BEARER_AUTH_SCHEME} {access_token}").into_masked(),
                ),
            ])
        }

        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<domain_types::router_data::ErrorResponse, errors::ConnectorError> {
            let mut error = self.build_error_response(res, event_builder, _connector_config)?;
            // A 4xx at payment initiation is terminal: Fire rejected the
            // payment request itself (malformed payload, invalid URL, auth or
            // permission failure), so the attempt is failed. 5xx stays unset -
            // the request may still have reached Fire, and PSync must resolve
            // the final state. PSync/ServerAuthenticationToken errors keep
            // attempt_status unset for the same reason.
            if (400..500).contains(&error.status_code) {
                error.attempt_status = Some(FlowStatus::Payment(AttemptStatus::Failure));
            }
            Ok(error)
        }
    }
);

// Bank Pay refunds: POST /business/v1/payments/{paymentUuid}/bankpayrefund.
// The paymentUuid is the connector transaction ID PSync stored; Fire only
// accepts refunds against SETTLED payments and answers RECEIVED / REJECTED.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Fire,
    curl_request: Json(FireRefundRequest),
    curl_response: FireRefundResponse,
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
            req: &RouterDataV2<
                Refund,
                RefundFlowData,
                RefundsData,
                RefundsResponseData,
            >,
        ) -> CustomResult<String, errors::IntegrationError> {
            let payment_uuid = &req.request.connector_transaction_id;
            if payment_uuid.is_empty() {
                return Err(errors::IntegrationError::MissingRequiredField {
                    field_name: "connector_transaction_id",
                    context: Default::default(),
                }
                .into());
            }
            let base_url = self.base_url(&req.resource_common_data.connectors);
            Ok(format!(
                "{base_url}{FIRE_REFUND_ENDPOINT_PREFIX}{payment_uuid}{FIRE_REFUND_ENDPOINT_SUFFIX}"
            ))
        }

        fn get_headers(
            &self,
            req: &RouterDataV2<
                Refund,
                RefundFlowData,
                RefundsData,
                RefundsResponseData,
            >,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            let access_token = req.resource_common_data.get_access_token()?;
            Ok(vec![
                (CONTENT_TYPE_HEADER.to_string(), self.common_get_content_type().to_string().into()),
                (
                    AUTHORIZATION_HEADER.to_string(),
                    format!("{FIRE_BEARER_AUTH_SCHEME} {access_token}").into_masked(),
                ),
            ])
        }

        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<domain_types::router_data::ErrorResponse, errors::ConnectorError> {
            self.build_error_response(res, event_builder, _connector_config)
        }
    }
);

// Bank Pay refund sync: Get Payment Details v2 — the same transaction API
// as the payment sync, keyed by the refund's `refundRequestUuid` (stored
// as connector_refund_id). The transaction comes back with
// `transactionType: "REFUND_REQUEST"` and a refund status
// (RECEIVED / REJECTED).
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Fire,
    curl_response: FireTransactionDetails,
    flow_name: RSync,
    resource_common_data: RefundFlowData,
    flow_request: RefundSyncData,
    flow_response: RefundsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<
                RSync,
                RefundFlowData,
                RefundSyncData,
                RefundsResponseData,
            >,
        ) -> CustomResult<String, errors::IntegrationError> {
            let refund_uuid = &req.request.connector_refund_id;
            if refund_uuid.is_empty() {
                return Err(errors::IntegrationError::MissingRequiredField {
                    field_name: "connector_refund_id",
                    context: Default::default(),
                }
                .into());
            }
            let base_url = self.base_url(&req.resource_common_data.connectors);
            Ok(format!("{base_url}{FIRE_PAYMENT_DETAILS_ENDPOINT_PREFIX}{refund_uuid}"))
        }

        fn get_headers(
            &self,
            req: &RouterDataV2<
                RSync,
                RefundFlowData,
                RefundSyncData,
                RefundsResponseData,
            >,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            let access_token = req.resource_common_data.get_access_token()?;
            Ok(vec![
                (CONTENT_TYPE_HEADER.to_string(), self.common_get_content_type().to_string().into()),
                (
                    AUTHORIZATION_HEADER.to_string(),
                    format!("{FIRE_BEARER_AUTH_SCHEME} {access_token}").into_masked(),
                ),
            ])
        }

        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<domain_types::router_data::ErrorResponse, errors::ConnectorError> {
            self.build_error_response(res, event_builder, _connector_config)
        }
    }
);

// =============================================================================
// CONNECTOR COMMON IMPLEMENTATION
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Fire<T>
{
    fn id(&self) -> &'static str {
        "fire"
    }

    // FIRE amounts are integer minor units (e.g. 1000 = 10.00 GBP).
    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        FIRE_CONTENT_TYPE
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.fire.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        _auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
        // FIRE bearer tokens are short-lived and are acquired by the
        // ServerAuthenticationToken flow. Payment flows add the token from
        // PaymentFlowData::access_token to their request headers.
        Ok(Vec::new())
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<domain_types::router_data::ErrorResponse, errors::ConnectorError> {
        let response: fire::FireErrorResponse = res
            .response
            .parse_struct("FireErrorResponse")
            .change_context(crate::utils::response_deserialization_fail(
                res.status_code,
                "fire: error body did not match the documented error shape",
            ))?;

        with_error_response_body!(event_builder, response);

        // FIRE's public API pages document only generic 400/401/403 errors and
        // do not identify terminal payment or refund error codes. Leave the
        // shared error status unset so a transport/authentication error cannot
        // overwrite an existing payment state. The explicit empty-attempt PSync
        // case is handled separately with PAYMENT_NOT_ATTEMPTED.
        let attempt_status: Option<FlowStatus> = None;
        let error = response.primary_error();

        Ok(domain_types::router_data::ErrorResponse {
            status_code: res.status_code,
            // Never `.unwrap_or_default()` here: an empty `code`/`message` is
            // indistinguishable from a connector that really returned "". The
            // NO_ERROR_CODE / NO_ERROR_MESSAGE sentinels are what the rest of
            // the platform greps for.
            code: error
                .and_then(|error| error.code)
                .map(|code| code.to_string())
                .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            message: error
                .and_then(|error| error.message.clone())
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            reason: error.and_then(|error| error.message.clone()),
            attempt_status,
            typed_connector_response: macros::serialize_typed_connector_payload(
                &response,
                "typed_connector_response",
            ),
            // `ErrorResponse` has 13 fields and `impl Default`
            // (crates/types-traits/domain_types/src/router_data.rs). Use the
            // functional-update tail so a new field upstream does not break this.
            ..Default::default()
        })
    }
}

// =============================================================================
// BODY DECODING IMPLEMENTATION
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Fire<T>
{
}

// =============================================================================
// WEBHOOK SOURCE VERIFICATION
// =============================================================================
// FIRE Bank Pay webhooks arrive as `application/jwt` bodies signed with RS256;
// the public key lives at the fixed JWKS URL in the transformers module. The
// out-of-band VerifyWebhookSource flow fetches that JWKS and the transformer
// verifies the JWT signature (truelayer-style two-call architecture).
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyWebhookSourceV2 for Fire<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VerifyWebhookSource,
        VerifyWebhookSourceFlowData,
        VerifyWebhookSourceRequestData,
        VerifyWebhookSourceResponseData,
    > for Fire<T>
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Get
    }

    fn get_url(
        &self,
        _req: &RouterDataV2<
            VerifyWebhookSource,
            VerifyWebhookSourceFlowData,
            VerifyWebhookSourceRequestData,
            VerifyWebhookSourceResponseData,
        >,
    ) -> CustomResult<String, errors::IntegrationError> {
        Ok(fire::FIRE_WEBHOOK_JWKS_URL.to_string())
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            VerifyWebhookSource,
            VerifyWebhookSourceFlowData,
            VerifyWebhookSourceRequestData,
            VerifyWebhookSourceResponseData,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<
            VerifyWebhookSource,
            VerifyWebhookSourceFlowData,
            VerifyWebhookSourceRequestData,
            VerifyWebhookSourceResponseData,
        >,
        errors::ConnectorError,
    > {
        let response: fire::FireJwks = res.response.parse_struct("fire Jwks").change_context(
            crate::utils::response_deserialization_fail(
                res.status_code,
                "fire: JWKS body did not match the expected format",
            ),
        )?;
        finalize_connector_response!(event_builder, response, data, res.status_code)
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<domain_types::router_data::ErrorResponse, errors::ConnectorError> {
        self.build_error_response(res, event_builder, _connector_config)
    }
}

// =============================================================================
// INCOMING WEBHOOK IMPLEMENTATION
// =============================================================================
// Two RPCs, two phases: ParseEvent uses get_event_type /
// get_webhook_event_reference on the unverified JWT payload; HandleEvent runs
// VerifyWebhookSource (RS256 via the vendor JWKS in production, HS256 with
// the merchant webhook secret in preprod) and then process_*_webhook.
// Observed payload shape (preprod): no event-name string — `type` is a
// category (e.g. "OTHER"), classification uses `status` + `transactionType`.
// See the transformers module for details.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::webhooks::IncomingWebhook for Fire<T>
{
    fn get_webhook_integrity_checks(&self) -> Vec<WebhookIntegrityCheck> {
        // The payment webhook carries `paymentUuid` (surfaced as
        // `connector_transaction_id`), plus `amountBeforeCharges` and
        // `currency.code` (surfaced as `amount`) for the caller to compare.
        // The merchant transaction id (`orderDetails.orderId`) is returned as
        // `merchant_transaction_id`, but the integrity enum has no variant
        // for it.
        vec![
            WebhookIntegrityCheck::ConnectorTransactionId,
            WebhookIntegrityCheck::Amount,
            WebhookIntegrityCheck::Currency,
        ]
    }

    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<errors::WebhookError>> {
        // In-process path for FIRE's preprod HS256 webhooks (see the
        // `requires_external_webhook_verification_for_body` override). The
        // HS256 secret comes from the connector config
        // (`webhook_secret`), with the webhook-secrets channel as a
        // fallback. An empty secret is treated as not configured. Production RS256 bodies never reach here — they route
        // to the external JWKS flow, and if that flow is not configured
        // they fail closed in the HS256 check below (a non-HS256 token
        // cannot verify as HS256).
        let config_secret = connector_account_details
            .as_ref()
            .and_then(|config| match config {
                ConnectorSpecificConfig::Fire { webhook_secret, .. } => webhook_secret
                    .as_ref()
                    .filter(|secret| !secret.peek().is_empty())
                    .map(|secret| ConnectorWebhookSecrets {
                        secret: secret.peek().as_bytes().to_vec(),
                        additional_secret: None,
                    }),
                _ => None,
            });
        let secrets = config_secret.or(connector_webhook_secret);
        fire::verify_fire_hs256_webhook(&request.body, secrets)
    }

    fn sample_webhook_body(&self) -> &'static [u8] {
        // A structurally valid HS256 JWT mirroring the observed preprod
        // payload shape: {"type": "OTHER", "status": "AUTHORISED",
        // "transactionType": "PAYMENT", "paymentUuid": "..."}. The
        // signature is a placeholder — good for ParseEvent, fails
        // HandleEvent signature verification as expected.
        br#"eyJraWQiOiI2NjAzZWQzYi1iYzMwLTQ3ZmItYjVlMy1iMjFjOWFiYmNlYzkiLCJhbGciOiJIUzI1NiJ9.eyJ0eXBlIjoiT1RIRVIiLCJzdGF0dXMiOiJBVVRIT1JJU0VEIiwidHJhbnNhY3Rpb25UeXBlIjoiUEFZTUVOVCIsInBheW1lbnRVdWlkIjoiM2YyZDBhMjAtZTg0Yi00MWMxLTlkM2QtYTFiMmMzZDRlNWY2In0.c2lnbmF0dXJl"#
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<errors::WebhookError>> {
        let claims = fire::parse_fire_webhook_claims(&request.body)?;
        fire::get_fire_webhook_event_type(&claims)
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<errors::WebhookError>> {
        let claims = fire::parse_fire_webhook_claims(&request.body)?;
        let event_type = fire::get_fire_webhook_event_type(&claims)?;
        match event_type {
            EventType::PaymentIntentAuthorizationSuccess
            | EventType::PaymentIntentSuccess
            | EventType::PaymentIntentFailure
            | EventType::PaymentIntentProcessing => Ok(Some(WebhookResourceReference::Payment(
                PaymentWebhookReference {
                    connector_transaction_id: claims.payment_uuid.clone(),
                    merchant_transaction_id: claims.merchant_transaction_id(),
                    connector_preprocessing_id: None,
                },
            ))),
            EventType::RefundProcessing | EventType::RefundFailure | EventType::RefundSuccess => {
                Ok(Some(WebhookResourceReference::Refund(
                    RefundWebhookReference {
                        connector_refund_id: claims.refund_request_uuid.clone(),
                        merchant_refund_id: None,
                        connector_transaction_id: claims.payment_uuid.clone(),
                        merchant_transaction_id: claims.merchant_transaction_id(),
                    },
                )))
            }
            _ => Ok(None),
        }
    }

    fn get_typed_webhook_response(&self, request: RequestDetails) -> Option<String> {
        // The webhook body is a JWT; the typed response is its decoded claims.
        let claims = fire::parse_fire_webhook_claims(&request.body).ok()?;
        events::MaskedSerdeValue::from_masked_optional(&claims, "fire_webhook_response")
            .map(|masked| masked.inner().to_string())
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<errors::WebhookError>> {
        let claims = fire::parse_fire_webhook_claims(&request.body)?;
        let event_type = fire::get_fire_webhook_event_type(&claims)?;
        let status = fire::get_fire_payment_webhook_status(&event_type)?;
        let payment_uuid = claims.payment_uuid.clone().ok_or_else(|| {
            error_stack::report!(errors::WebhookError::WebhookReferenceIdNotFound)
        })?;

        Ok(WebhookDetailsResponse {
            // Surface the exact status string FIRE sent so callers can see
            // what the vendor reported, as on the PSync path.
            raw_connector_status: claims.status.clone().map(|status| RawConnectorStatus {
                code: Some(status),
                message: None,
                reason: None,
            }),
            amount: claims.money(),
            resource_id: Some(ResponseId::ConnectorTransactionId(payment_uuid)),
            status,
            connector_response_reference_id: None,
            // Surfaced as `merchant_transaction_id` in the gRPC response.
            connector_request_reference_id: claims.merchant_transaction_id(),
            mandate_reference: None,
            error_code: None,
            error_message: None,
            error_reason: None,
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
            captured_money: None,
            network_txn_id: None,
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
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<errors::WebhookError>> {
        let claims = fire::parse_fire_webhook_claims(&request.body)?;
        let event_type = fire::get_fire_webhook_event_type(&claims)?;
        let status = fire::get_fire_refund_webhook_status(&event_type)?;
        let refund_request_uuid = claims.refund_request_uuid.clone().ok_or_else(|| {
            error_stack::report!(errors::WebhookError::WebhookReferenceIdNotFound)
        })?;

        Ok(RefundWebhookDetailsResponse {
            // Surface the exact status string FIRE sent, as on the PSync
            // and payment-webhook paths.
            raw_connector_status: claims.status.clone().map(|status| RawConnectorStatus {
                code: Some(status),
                message: None,
                reason: None,
            }),
            connector_refund_id: Some(refund_request_uuid.clone()),
            merchant_transaction_id: None,
            status,
            connector_response_reference_id: Some(refund_request_uuid),
            error_code: None,
            error_message: None,
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
        })
    }

    fn get_webhook_resource_object(
        &self,
        request: RequestDetails,
    ) -> Result<
        Box<dyn hyperswitch_masking::ErasedMaskSerialize>,
        error_stack::Report<errors::WebhookError>,
    > {
        let claims = fire::parse_fire_webhook_claims(&request.body)?;
        Ok(Box::new(claims))
    }
}

// ===== CONNECTOR SERVICE TRAIT IMPLEMENTATION =====
// Aggregate trait - composes all other connector traits.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Fire<T>
{
}

// ===== BASE (NON-FLOW) TRAIT IMPLEMENTATIONS =====
// These are simple marker traits that are NOT flows and therefore have no arm
// in expand_flow_status_impl!. They must be impl'd manually.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Fire<T>
{
    fn should_do_access_token(&self, _payment_method: Option<common_enums::PaymentMethod>) -> bool {
        true
    }

    // FIRE ships two webhook signing algorithms: RS256 in production
    // (verified by the external JWKS flow) and HS256 in preprod (verified
    // in-process with the merchant webhook secret). Only HS256 bodies skip
    // the external flow; anything else — including unparsable bodies —
    // still goes through it, so verification stays fail-closed.
    fn requires_external_webhook_verification_for_body(&self, webhook_body: &[u8]) -> bool {
        !fire::fire_webhook_uses_hs256(webhook_body)
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Fire<T>
{
    // FIRE's return-URL redirect carries no signature, so it cannot be
    // source-verified. Report it as unverified and leave the outcome to PSync.
    fn verify_redirect_response_source(
        &self,
        _request: &RequestDetails,
        _secrets: Option<interfaces::verification::ConnectorSourceVerificationSecrets>,
    ) -> CustomResult<bool, errors::IntegrationError> {
        Ok(false)
    }

    // The redirect is not trusted for status, amount or IDs: return an empty
    // response so the caller confirms the payment via PSync.
    fn process_redirect_response(
        &self,
        _request: &RequestDetails,
        _connector_feature_data: Option<&hyperswitch_masking::Secret<String>>,
    ) -> CustomResult<RedirectDetailsResponse, errors::IntegrationError> {
        Ok(RedirectDetailsResponse {
            resource_id: None,
            status: None,
            connector_response_reference_id: None,
            error_code: None,
            error_message: None,
            error_reason: None,
            response_money: None,
            raw_connector_response: None,
            connector_feature_data: None,
        })
    }
}

// ===== SOURCE VERIFICATION IMPLEMENTATION =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::verification::SourceVerification for Fire<T>
{
}

// ===== PAYOUT TRAIT IMPLEMENTATIONS =====
// Emits payout marker-trait impls and default no-op ConnectorIntegrationV2
// impls for all PayoutXxxV2 flows.
crate::connectors::macros::macro_connector_payout_implementation!(
    connector: Fire,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

// ===== FLOW STATUS IMPLEMENTATIONS =====
// Emits marker-trait impls AND stub ConnectorIntegrationV2 impls for every
// flow listed. Each stub's get_url returns
// IntegrationError::connector_flow_not_implemented(...).
crate::connectors::macros::macro_connector_flow_status_impls!(
    connector: Fire,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        Accept,
        ClientAuthenticationToken,
        CreateConnectorCustomer,
        DefendDispute,
        GetConnectorCustomer,
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
        VoidPostRefund,
        RepeatPayment,
        ServerSessionAuthenticationToken,
        SetupMandate,
        SubmitEvidence
    ],
);
