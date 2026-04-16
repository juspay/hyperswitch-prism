pub mod transformers;

use super::macros;
use transformers::*;

use common_utils::{
    crypto::{self, GenerateDigest, VerifySignature},
    errors::{CryptoError, CustomResult},
    events,
    ext_traits::ByteSliceExt,
};
use domain_types::{
    connector_flow::{
        Accept, Authenticate, Authorize, Capture, ClientAuthenticationToken,
        CreateConnectorCustomer, CreateOrder, DefendDispute, IncrementalAuthorization,
        MandateRevoke, PSync, PaymentMethodToken, PostAuthenticate, PreAuthenticate, RSync, Refund,
        RepeatPayment, ServerAuthenticationToken, ServerSessionAuthenticationToken, SetupMandate,
        SubmitEvidence, Void, VoidPC,
    },
    connector_types::{
        AcceptDisputeData, ClientAuthenticationTokenRequestData, ConnectorCustomerData,
        ConnectorCustomerResponse, ConnectorSpecifications, ConnectorWebhookSecrets,
        DisputeDefendData, DisputeFlowData, DisputeResponseData, EventType,
        MandateRevokeRequestData, MandateRevokeResponseData, PaymentCreateOrderData,
        PaymentCreateOrderResponse, PaymentFlowData, PaymentMethodTokenResponse,
        PaymentMethodTokenizationData, PaymentVoidData, PaymentsAuthenticateData,
        PaymentsAuthorizeData, PaymentsCancelPostCaptureData, PaymentsCaptureData,
        PaymentsIncrementalAuthorizationData, PaymentsPostAuthenticateData,
        PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSyncData,
        RedirectDetailsResponse, RefundFlowData,
        RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData, RequestDetails,
        ResponseId, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
        ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData,
        SetupMandateRequestData, SubmitEvidenceData, SupportedPaymentMethodsExt,
        WebhookDetailsResponse,
    },
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::{
        ConnectorInfo, FeatureStatus, PaymentConnectorCategory, PaymentMethodDetails,
        SupportedPaymentMethods,
    },
};
use error_stack::ResultExt;
use hyperswitch_masking::{Mask, PeekInterface};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types,
    webhooks::{IncomingWebhook, IncomingWebhookEvent, IncomingWebhookRequestDetails},
};

use crate::{types::ResponseRouterData, with_error_response_body};
use domain_types::errors::ConnectorError;
use domain_types::errors::{IntegrationError, WebhookError};
use serde::Serialize;
use std::fmt::Debug;
use std::sync::LazyLock;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const MERCHANT_ID: &str = "Merchant-Id";
    pub(crate) const REQUEST_IDEMPOTENCY_KEY: &str = "Request-Idempotency-Key";
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Ppro<T>
{
    fn id(&self) -> &'static str {
        "ppro"
    }

    fn get_currency_unit(&self) -> common_enums::CurrencyUnit {
        common_enums::CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a domain_types::types::Connectors) -> &'a str {
        connectors.ppro.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, hyperswitch_masking::Maskable<String>)>, IntegrationError> {
        match auth_type {
            ConnectorSpecificConfig::Ppro {
                api_key,
                merchant_id,
                ..
            } => Ok(vec![
                (
                    headers::AUTHORIZATION.to_string(),
                    format!("Bearer {}", api_key.peek()).into_masked(),
                ),
                (
                    headers::MERCHANT_ID.to_string(),
                    merchant_id.clone().into_masked(),
                ),
            ]),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: PproErrorResponse = res
            .response
            .parse_struct("Ppro ErrorResponse")
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "ppro: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        with_error_response_body!(event_builder, response);

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.status.to_string(),
            message: response.failure_message,
            reason: None,
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Ppro<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Ppro<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerSessionAuthentication for Ppro<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerAuthentication for Ppro<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for Ppro<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Ppro<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Ppro<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Ppro<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Ppro<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Ppro<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for Ppro<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for Ppro<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Ppro<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::AcceptDispute for Ppro<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentIncrementalAuthorization for Ppro<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SubmitEvidenceV2 for Ppro<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::DisputeDefend for Ppro<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Ppro<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Ppro<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Ppro<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Ppro<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Ppro<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Ppro<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::MandateRevokeV2 for Ppro<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    > for Ppro<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerSessionAuthenticationToken,
        PaymentFlowData,
        ServerSessionAuthenticationTokenRequestData,
        ServerSessionAuthenticationTokenResponseData,
    > for Ppro<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ClientAuthenticationToken,
        PaymentFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    > for Ppro<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerAuthenticationToken,
        PaymentFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    > for Ppro<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    > for Ppro<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    > for Ppro<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Ppro,
    curl_request: Json(PproAgreementChargeRequest),
    curl_response: PproPaymentsResponse,
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
        ) -> CustomResult<Vec<(String, hyperswitch_masking::Maskable<String>)>, IntegrationError> {
            let mut header = self.get_auth_header(&req.connector_config)?;
            header.push((
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            ));
            Ok(header)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let agr_id = req.request.connector_mandate_id().ok_or(
                IntegrationError::MissingRequiredField {
                    field_name: "mandate_reference.connector_mandate_id",
                context: Default::default()
                },
            )?;
            Ok(format!(
                "{}/v1/payment-agreements/{}/payment-charges",
                self.base_url(&req.resource_common_data.connectors),
                agr_id
            ))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        MandateRevokeResponseData,
    > for Ppro<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>
    for Ppro<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>
    for Ppro<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>
    for Ppro<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    > for Ppro<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    > for Ppro<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    > for Ppro<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    > for Ppro<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Ppro<T>
{
    fn get_event_type(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let event: PproWebhookEvent = request
            .body
            .parse_struct("PproWebhookEvent")
            .change_context(WebhookError::WebhookResourceObjectNotFound)?;

        EventType::try_from(event.r#type)
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let event: PproWebhookEvent = request
            .body
            .parse_struct("PproWebhookEvent")
            .change_context(WebhookError::WebhookResourceObjectNotFound)?;

        let charge = match event.data {
            PproWebhookData::Charge { charge } => charge,
            PproWebhookData::Agreement { .. } => {
                return Err(error_stack::report!(WebhookError::WebhooksNotImplemented {
                    operation: "process_payment_webhook",
                }));
            }
        };

        let status = common_enums::AttemptStatus::from(charge.status);

        let (error_code, error_message, error_reason) = match charge.failure.as_ref() {
            Some(failure) => (
                failure.failure_code.clone(),
                Some(failure.failure_message.clone()),
                Some(format!(
                    "{}: {}",
                    failure.failure_type,
                    failure.failure_code.as_deref().unwrap_or("UNKNOWN")
                )),
            ),
            None => (None, None, None),
        };

        Ok(WebhookDetailsResponse {
            resource_id: Some(ResponseId::ConnectorTransactionId(charge.id.clone())),
            status,
            connector_response_reference_id: Some(charge.id),
            error_code,
            error_message,
            error_reason,
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            mandate_reference: None,
            response_headers: None,
            transformation_status: common_enums::WebhookTransformationStatus::Complete,
            amount_captured: None,
            minor_amount_captured: None,
            network_txn_id: None,
            payment_method_update: None,
        })
    }

    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<
        domain_types::connector_types::RefundWebhookDetailsResponse,
        error_stack::Report<WebhookError>,
    > {
        let event: PproWebhookEvent = request
            .body
            .parse_struct("PproWebhookEvent")
            .change_context(WebhookError::WebhookResourceObjectNotFound)?;

        let charge = match event.data {
            PproWebhookData::Charge { charge } => charge,
            PproWebhookData::Agreement { .. } => {
                return Err(error_stack::report!(WebhookError::WebhooksNotImplemented {
                    operation: "process_refund_webhook",
                }));
            }
        };

        let status = common_enums::RefundStatus::from(charge.status);

        let (error_code, error_message) = match charge.failure.as_ref() {
            Some(failure) => (
                failure.failure_code.clone(),
                Some(failure.failure_message.clone()),
            ),
            None => (None, None),
        };

        Ok(
            domain_types::connector_types::RefundWebhookDetailsResponse {
                connector_refund_id: Some(charge.id.clone()),
                status,
                connector_response_reference_id: Some(charge.id),
                error_code,
                error_message,
                raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
                status_code: 200,
                response_headers: None,
            },
        )
    }

    fn process_dispute_webhook(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<
        domain_types::connector_types::DisputeWebhookDetailsResponse,
        error_stack::Report<WebhookError>,
    > {
        Err(error_stack::report!(WebhookError::WebhooksNotImplemented {
            operation: "process_dispute_webhook",
        }))
    }

    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        let connector_webhook_secrets = connector_webhook_secret
            .ok_or_else(|| error_stack::report!(WebhookError::WebhookVerificationSecretNotFound))
            .attach_printable("Connector webhook secret not configured")?;

        let signature = request
            .headers
            .get("Webhook-Signature")
            .ok_or_else(|| error_stack::report!(WebhookError::WebhookSignatureNotFound))?;

        let algorithm = crypto::HmacSha256;
        let expected_signature =
            hex::decode(signature).change_context(WebhookError::WebhookBodyDecodingFailed)?;

        algorithm
            .verify_signature(
                &connector_webhook_secrets.secret,
                &expected_signature,
                &request.body,
            )
            .change_context(WebhookError::WebhookSourceVerificationFailed)
    }

    fn get_webhook_resource_object(
        &self,
        request: RequestDetails,
    ) -> Result<Box<dyn hyperswitch_masking::ErasedMaskSerialize>, error_stack::Report<WebhookError>>
    {
        let event: PproWebhookEvent = request
            .body
            .parse_struct("PproWebhookEvent")
            .change_context(WebhookError::WebhookResourceObjectNotFound)?;

        match event.data {
            PproWebhookData::Charge { charge } => Ok(Box::new(charge)),
            PproWebhookData::Agreement { agreement } => Ok(Box::new(agreement)),
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Ppro<T>
{
    fn verify_redirect_response_source(
        &self,
        _request: &RequestDetails,
        _secrets: Option<interfaces::verification::ConnectorSourceVerificationSecrets>,
    ) -> CustomResult<bool, IntegrationError> {
        Ok(false)
    }

    fn process_redirect_response(
        &self,
        request: &RequestDetails,
    ) -> CustomResult<RedirectDetailsResponse, IntegrationError> {
        let charge_id = request
            .query_params
            .as_deref()
            .and_then(|qs| {
                url::form_urlencoded::parse(qs.as_bytes())
                    .find(|(k, _)| k == "payment-charge-id")
                    .map(|(_, v)| v.into_owned())
            });

        Ok(RedirectDetailsResponse {
            resource_id: charge_id.map(ResponseId::ConnectorTransactionId),
            status: None,
            connector_response_reference_id: None,
            error_code: None,
            error_message: None,
            error_reason: None,
            response_amount: None,
            raw_connector_response: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::verification::SourceVerification for Ppro<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::decode::BodyDecoding for Ppro<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Ppro<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Ppro<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Ppro,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

static PPRO_CONNECTOR_INFO: ConnectorInfo = ConnectorInfo {
    display_name: "Ppro",
    description: "Ppro is a global provider of local payment infrastructure.",
    connector_type: PaymentConnectorCategory::PaymentGateway,
};

static PPRO_SUPPORTED_PAYMENT_METHODS: LazyLock<SupportedPaymentMethods> = LazyLock::new(|| {
    let mut ppro_supported_payment_methods = SupportedPaymentMethods::new();

    let ppro_bridge_supported_capture_methods = vec![common_enums::CaptureMethod::Automatic];

    ppro_supported_payment_methods.add(
        common_enums::PaymentMethod::Wallet,
        common_enums::PaymentMethodType::AliPay,
        PaymentMethodDetails {
            mandates: FeatureStatus::NotSupported,
            refunds: FeatureStatus::Supported,
            supported_capture_methods: ppro_bridge_supported_capture_methods.clone(),
            specific_features: None,
        },
    );
    ppro_supported_payment_methods.add(
        common_enums::PaymentMethod::Wallet,
        common_enums::PaymentMethodType::WeChatPay,
        PaymentMethodDetails {
            mandates: FeatureStatus::NotSupported,
            refunds: FeatureStatus::Supported,
            supported_capture_methods: ppro_bridge_supported_capture_methods.clone(),
            specific_features: None,
        },
    );

    ppro_supported_payment_methods.add(
        common_enums::PaymentMethod::Wallet,
        common_enums::PaymentMethodType::MbWay,
        PaymentMethodDetails {
            mandates: FeatureStatus::NotSupported,
            refunds: FeatureStatus::Supported,
            supported_capture_methods: ppro_bridge_supported_capture_methods.clone(),
            specific_features: None,
        },
    );

    ppro_supported_payment_methods.add(
        common_enums::PaymentMethod::Wallet,
        common_enums::PaymentMethodType::Satispay,
        PaymentMethodDetails {
            mandates: FeatureStatus::NotSupported,
            refunds: FeatureStatus::Supported,
            supported_capture_methods: ppro_bridge_supported_capture_methods.clone(),
            specific_features: None,
        },
    );

    ppro_supported_payment_methods.add(
        common_enums::PaymentMethod::Wallet,
        common_enums::PaymentMethodType::Wero,
        PaymentMethodDetails {
            mandates: FeatureStatus::NotSupported,
            refunds: FeatureStatus::Supported,
            supported_capture_methods: ppro_bridge_supported_capture_methods.clone(),
            specific_features: None,
        },
    );

    ppro_supported_payment_methods.add(
        common_enums::PaymentMethod::Upi,
        common_enums::PaymentMethodType::UpiIntent,
        PaymentMethodDetails {
            mandates: FeatureStatus::NotSupported,
            refunds: FeatureStatus::Supported,
            supported_capture_methods: ppro_bridge_supported_capture_methods.clone(),
            specific_features: None,
        },
    );

    let bank_redirect_methods = vec![
        (
            common_enums::PaymentMethodType::Ideal,
            FeatureStatus::Supported,
        ),
        (
            common_enums::PaymentMethodType::BancontactCard,
            FeatureStatus::Supported,
        ),
        (
            common_enums::PaymentMethodType::Trustly,
            FeatureStatus::Supported,
        ),
        (
            common_enums::PaymentMethodType::Blik,
            FeatureStatus::Supported,
        ),
    ];

    for (pm_type, mandate_support) in bank_redirect_methods {
        ppro_supported_payment_methods.add(
            common_enums::PaymentMethod::BankRedirect,
            pm_type,
            PaymentMethodDetails {
                mandates: mandate_support,
                refunds: FeatureStatus::Supported,
                supported_capture_methods: ppro_bridge_supported_capture_methods.clone(),
                specific_features: None,
            },
        );
    }

    ppro_supported_payment_methods
});

static PPRO_SUPPORTED_WEBHOOK_FLOWS: &[common_enums::EventClass] = &[
    common_enums::EventClass::Payments,
    common_enums::EventClass::Refunds,
];

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorSpecifications
    for Ppro<T>
{
    fn get_connector_about(&self) -> Option<&'static ConnectorInfo> {
        Some(&PPRO_CONNECTOR_INFO)
    }

    fn get_supported_payment_methods(&self) -> Option<&'static SupportedPaymentMethods> {
        Some(&PPRO_SUPPORTED_PAYMENT_METHODS)
    }

    fn get_supported_webhook_flows(&self) -> Option<&'static [common_enums::EventClass]> {
        Some(PPRO_SUPPORTED_WEBHOOK_FLOWS)
    }
}

macros::create_all_prerequisites!(
    connector_name: Ppro,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: PproPaymentsRequest,
            response_body: PproAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: PproPSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: PproCaptureRequest,
            response_body: PproCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: PproVoidRequest,
            response_body: PproVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: PproRefundRequest,
            response_body: PproRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: PproRSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: PproAgreementRequest,
            response_body: PproAgreementResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: PproAgreementChargeRequest,
            response_body: PproPaymentsResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [],
    member_functions: {}
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Ppro,
    curl_request: Json(PproPaymentsRequest),
    curl_response: PproAuthorizeResponse,
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
        ) -> CustomResult<Vec<(String, hyperswitch_masking::Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )];
            header.push((
                headers::REQUEST_IDEMPOTENCY_KEY.to_string(),
                req.resource_common_data.connector_request_reference_id.clone().into(),
            ));
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);
            Ok(header)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/v1/payment-charges", self.base_url(&req.resource_common_data.connectors)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Ppro,
    curl_response: PproPSyncResponse,
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
        ) -> CustomResult<Vec<(String, hyperswitch_masking::Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);
            Ok(header)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let id = req.request.get_connector_transaction_id().change_context(IntegrationError::MissingConnectorTransactionID { context: Default::default() })?;
            Ok(format!("{}/v1/payment-charges/{}", self.base_url(&req.resource_common_data.connectors), id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Ppro,
    curl_request: Json(PproCaptureRequest),
    curl_response: PproCaptureResponse,
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
        ) -> CustomResult<Vec<(String, hyperswitch_masking::Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )];
            header.push((
                headers::REQUEST_IDEMPOTENCY_KEY.to_string(),
                req.resource_common_data.connector_request_reference_id.clone().into(),
            ));
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);
            Ok(header)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let id = req.request.get_connector_transaction_id().change_context(IntegrationError::MissingConnectorTransactionID { context: Default::default() })?;
            Ok(format!("{}/v1/payment-charges/{}/captures", self.base_url(&req.resource_common_data.connectors), id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Ppro,
    curl_request: Json(PproVoidRequest),
    curl_response: PproVoidResponse,
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
        ) -> CustomResult<Vec<(String, hyperswitch_masking::Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )];
            header.push((
                headers::REQUEST_IDEMPOTENCY_KEY.to_string(),
                req.resource_common_data.connector_request_reference_id.clone().into(),
            ));
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);
            Ok(header)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let id = req.request.connector_transaction_id.clone();
            Ok(format!("{}/v1/payment-charges/{}/voids", self.base_url(&req.resource_common_data.connectors), id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Ppro,
    curl_request: Json(PproRefundRequest),
    curl_response: PproRefundResponse,
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
        ) -> CustomResult<Vec<(String, hyperswitch_masking::Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )];
            header.push((
                headers::REQUEST_IDEMPOTENCY_KEY.to_string(),
                req.resource_common_data.connector_request_reference_id.clone().into(),
            ));
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);
            Ok(header)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let id = req.request.connector_transaction_id.clone();
            Ok(format!("{}/v1/payment-charges/{}/refunds", self.base_url(&req.resource_common_data.connectors), id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Ppro,
    curl_response: PproRSyncResponse,
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
        ) -> CustomResult<Vec<(String, hyperswitch_masking::Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);
            Ok(header)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let refund_id = req.request.connector_refund_id.clone();
            Ok(format!("{}/v1/payment-charges/{}", self.base_url(&req.resource_common_data.connectors), refund_id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Ppro,
    curl_request: Json(PproAgreementRequest),
    curl_response: PproAgreementResponse,
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
        ) -> CustomResult<Vec<(String, hyperswitch_masking::Maskable<String>)>, IntegrationError> {
            let mut header = self.get_auth_header(&req.connector_config)?;
            header.push((
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            ));
            Ok(header)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/v1/payment-agreements", self.base_url(&req.resource_common_data.connectors)))
        }
    }
);

#[derive(Debug, Clone)]
pub struct PproWebhookSignature;

impl VerifySignature for PproWebhookSignature {
    fn verify_signature(
        &self,
        secret: &[u8],
        signature: &[u8],
        msg: &[u8],
    ) -> CustomResult<bool, CryptoError> {
        let mut buf = Vec::with_capacity(msg.len() + 1 + secret.len());
        buf.extend_from_slice(msg);
        buf.push(b'.');
        buf.extend_from_slice(secret);

        let digest = crypto::Sha256
            .generate_digest(&buf)
            .change_context(CryptoError::SignatureVerificationFailed)?;

        let expected_signature = hex::encode(digest);
        Ok(expected_signature.as_bytes() == signature)
    }
}

#[async_trait::async_trait]
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + Serialize + 'static> IncomingWebhook
    for Ppro<T>
{
    fn get_webhook_source_verification_algorithm(
        &self,
        _request: &IncomingWebhookRequestDetails<'_>,
    ) -> CustomResult<Box<dyn VerifySignature + Send>, WebhookError> {
        Ok(Box::new(PproWebhookSignature))
    }

    fn get_webhook_source_verification_signature(
        &self,
        request: &IncomingWebhookRequestDetails<'_>,
        _connector_webhook_secrets: &ConnectorWebhookSecrets,
    ) -> CustomResult<Vec<u8>, WebhookError> {
        let header_value = request
            .headers
            .get("Webhook-Signature")
            .ok_or(WebhookError::WebhookSignatureNotFound)?
            .to_str()
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        Ok(header_value.as_bytes().to_vec())
    }

    fn get_webhook_source_verification_message(
        &self,
        request: &IncomingWebhookRequestDetails<'_>,
        _merchant_id: &common_utils::id_type::MerchantId,
        _connector_webhook_secrets: &ConnectorWebhookSecrets,
    ) -> CustomResult<Vec<u8>, WebhookError> {
        Ok(request.body.to_vec())
    }

    fn get_webhook_event_type(
        &self,
        request: &IncomingWebhookRequestDetails<'_>,
    ) -> CustomResult<IncomingWebhookEvent, WebhookError> {
        let event: PproWebhookEvent = request
            .body
            .parse_struct("PproWebhookEvent")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        IncomingWebhookEvent::try_from(event.r#type)
    }

    fn get_webhook_resource_object(
        &self,
        request: &IncomingWebhookRequestDetails<'_>,
    ) -> CustomResult<Box<dyn hyperswitch_masking::ErasedMaskSerialize>, WebhookError> {
        let event: PproWebhookEvent = request
            .body
            .parse_struct("PproWebhookEvent")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        match event.data {
            PproWebhookData::Charge { charge } => Ok(Box::new(charge)),
            PproWebhookData::Agreement { agreement } => Ok(Box::new(agreement)),
        }
    }
}

#[cfg(test)]
mod test;
