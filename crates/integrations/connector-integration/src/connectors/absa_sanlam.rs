use super::{
    macros,
    sanlam_common::transformers::{
        self as absa_sanlam, AbsaSanlamPaymentsRequest, AbsaSanlamPaymentsResponse,
    },
};
use crate::types::ResponseRouterData;
use common_enums::CurrencyUnit;
use common_utils::{
    crypto::{self, VerifySignature},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    request::{KafkaRecord, KafkaRecordBuilder, TransportType},
};
use domain_types::{
    connector_flow::Authorize,
    connector_types::{
        ConnectorWebhookSecrets, EventContext, EventType, PaymentFlowData, PaymentWebhookReference,
        PaymentsAuthorizeData, PaymentsResponseData, RequestDetails, WebhookDetailsResponse,
        WebhookResourceReference,
    },
    errors::{IntegrationError, IntegrationErrorContext, WebhookError},
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
use std::fmt::Debug;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const MERCHANT_ID: &str = "Merchant-Id";
}

macros::macro_connector_payout_implementation!(
    connector: AbsaSanlam,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for AbsaSanlam<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for AbsaSanlam<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for AbsaSanlam<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for AbsaSanlam<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for AbsaSanlam<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for AbsaSanlam<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for AbsaSanlam<T>
{
    fn get_webhook_source_verification_signature(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        let signature = request
            .headers
            .get("x-signature")
            .ok_or(WebhookError::WebhookSignatureNotFound)
            .attach_printable("Missing incoming webhook signature for AbsaSanlam")?;

        hex::decode(signature).change_context(WebhookError::WebhookSourceVerificationFailed)
    }

    fn get_webhook_source_verification_message(
        &self,
        request: &RequestDetails,
        _connector_webhook_secrets: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        let message = std::str::from_utf8(&request.body)
            .change_context(WebhookError::WebhookSourceVerificationFailed)?;

        Ok(message.to_string().into_bytes())
    }

    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        let algorithm = crypto::HmacSha256;

        let connector_webhook_secrets = match connector_webhook_secret {
            Some(secrets) => secrets,
            None => Err(WebhookError::WebhookVerificationSecretNotFound)?,
        };

        let signature =
            self.get_webhook_source_verification_signature(&request, &connector_webhook_secrets)?;

        let message =
            self.get_webhook_source_verification_message(&request, &connector_webhook_secrets)?;

        algorithm
            .verify_signature(&connector_webhook_secrets.secret, &signature, &message)
            .change_context(WebhookError::WebhookSourceVerificationFailed)
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let details: absa_sanlam::AbsaSanlamWebhookEvent = request
            .body
            .parse_struct("AbsaSanlamWebhookEvent")
            .change_context(WebhookError::WebhookEventTypeNotFound)?;

        let event_type = match details {
            absa_sanlam::AbsaSanlamWebhookEvent::Payment(ref event) => match event.event_type {
                absa_sanlam::AbsaSanlamWebhookEventType::PaymentSucceeded => {
                    EventType::PaymentIntentSuccess
                }
                absa_sanlam::AbsaSanlamWebhookEventType::PaymentFailed => {
                    EventType::PaymentIntentFailure
                }
                absa_sanlam::AbsaSanlamWebhookEventType::DisputeOpened => EventType::DisputeOpened,
            },
        };

        Ok(event_type)
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        let details: absa_sanlam::AbsaSanlamWebhookEvent = request
            .body
            .parse_struct("AbsaSanlamWebhookEvent")
            .change_context(WebhookError::WebhookReferenceIdNotFound)?;

        let id = match details {
            absa_sanlam::AbsaSanlamWebhookEvent::Payment(ref event) => {
                WebhookResourceReference::Payment(PaymentWebhookReference {
                    connector_transaction_id: None,
                    merchant_transaction_id: Some(event.payment.user_reference.clone()),
                })
            }
        };

        Ok(Some(id))
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let details: absa_sanlam::AbsaSanlamWebhookEvent = request
            .body
            .parse_struct("AbsaSanlamWebhookEvent")
            .change_context(WebhookError::WebhookResponseEncodingFailed)?;

        let response = WebhookDetailsResponse::try_from(details)
            .change_context(WebhookError::WebhookResponseEncodingFailed);

        response.map(|mut response| {
            response.raw_connector_response =
                Some(String::from_utf8_lossy(&request.body).to_string());
            response
        })
    }
}

macros::create_all_prerequisites!(
    connector_name: AbsaSanlam,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: AbsaSanlamPaymentsRequest,
            response_body: AbsaSanlamPaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                Self::common_get_content_type(self).to_string().into(),
            )];

            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);

            Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.absa_sanlam.base_url
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: AbsaSanlam,
    curl_request: Json(AbsaSanlamPaymentsRequest),
    curl_response: AbsaSanlamPaymentsResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_transport_type(&self) -> TransportType {
            TransportType::Kafka
        }

        fn get_headers(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            _req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Err(IntegrationError::connector_flow_not_supported(
                self.id(),
                "authorize",
                IntegrationErrorContext::default(),
            )
            .into())
        }

        fn get_kafka_topic(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}_payments_queue", self.connector_base_url_payments(req)))
        }

        fn get_kafka_key(
            &self,
            _req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Option<String>, IntegrationError> {
            Ok(None)
        }

        fn build_kafka_record(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Option<KafkaRecord>, IntegrationError> {
            Ok(Some(
                KafkaRecordBuilder::new()
                    .topic(self.get_kafka_topic(req)?.as_str())
                    .attach_default_headers()
                    .headers(self.get_headers(req)?)
                    .set_optional_payload(self.get_request_body(req)?)
                    .build(),
            ))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for AbsaSanlam<T>
{
    fn id(&self) -> &'static str {
        "absa_sanlam"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.absa_sanlam.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = absa_sanlam::AbsaSanlamAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        Ok(vec![
            (
                headers::AUTHORIZATION.to_string(),
                auth.api_key.peek().to_owned().into_masked(),
            ),
            (
                headers::MERCHANT_ID.to_string(),
                auth.merchant_id.peek().to_owned().into(),
            ),
        ])
    }
}

macros::macro_connector_flow_status_impls!(
    connector: AbsaSanlam,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        PSync,
        Capture,
        Refund,
        RSync,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        MandateRevoke,
        SetupMandate,
        RepeatPayment,
        PaymentMethodToken,
    ],
    not_supported: [
        Void,
        CreateOrder,
        SubmitEvidence,
        DefendDispute,
        Accept,
        ServerSessionAuthenticationToken,
        ServerAuthenticationToken,
        VoidPC,
        ClientAuthenticationToken,
        CreateConnectorCustomer,
        IncrementalAuthorization,
    ],
);


static ABSA_SANLAM_SUPPORTED_PAYMENT_METHODS: std::sync::LazyLock<domain_types::types::SupportedPaymentMethods> =
    std::sync::LazyLock::new(|| {

        let mut m = domain_types::types::SupportedPaymentMethods::new();
        let mut m = domain_types::build_supported_pms! {
            Supported => [
                (BankDebit, Eft),
            ],
        };
        m
    });

impl<T: domain_types::payment_method_data::PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + serde::Serialize>
    domain_types::connector_types::ConnectorSpecifications for AbsaSanlam<T>
{
    fn get_supported_payment_methods(&self) -> &'static domain_types::types::SupportedPaymentMethods {
        &ABSA_SANLAM_SUPPORTED_PAYMENT_METHODS
    }
}
