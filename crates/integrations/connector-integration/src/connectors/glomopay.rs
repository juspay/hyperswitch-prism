pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt};
use domain_types::{
    connector_flow::{
        Authorize, CreateConnectorCustomer, CreateOrder, GetConnectorCustomer, PSync, RSync, Refund,
    },
    connector_types::{
        ConnectorCustomerData, ConnectorCustomerResponse, PaymentCreateOrderData,
        PaymentCreateOrderResponse, PaymentFlowData, PaymentWebhookReference,
        PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundWebhookReference, RefundsData, RefundsResponseData, RequestDetails,
        WebhookResourceReference,
    },
    errors::{self, IntegrationError, WebhookError},
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
use transformers::{
    self as glomopay, GlomopayAuthorizeRequest, GlomopayAuthorizeResponse,
    GlomopayCreateCustomerRequest, GlomopayCreateCustomerResponse, GlomopayCreateOrderRequest,
    GlomopayCreateOrderResponse, GlomopayGetCustomerResponse, GlomopayPaymentSyncResponse,
    GlomopayRefundRequest, GlomopayRefundResponse, GlomopayRefundSyncResponse,
    GlomopayRefundWebhookPayload, GlomopayWebhookEntityProbe, GlomopayWebhookPayload,
};

use crate::{types::ResponseRouterData, with_error_response_body};

use super::macros;

macros::create_amount_converter_wrapper!(connector_name: Glomopay, amount_type: MinorUnit);

macros::create_all_prerequisites!(
    connector_name: Glomopay,
    generic_type: T,
    api: [
        (
            flow: GetConnectorCustomer,
            response_body: GlomopayGetCustomerResponse,
            router_data: RouterDataV2<GetConnectorCustomer, PaymentFlowData, ConnectorCustomerData, ConnectorCustomerResponse>,
        ),
        (
            flow: CreateConnectorCustomer,
            request_body: GlomopayCreateCustomerRequest,
            response_body: GlomopayCreateCustomerResponse,
            router_data: RouterDataV2<CreateConnectorCustomer, PaymentFlowData, ConnectorCustomerData, ConnectorCustomerResponse>,
        ),
        (
            flow: CreateOrder,
            request_body: GlomopayCreateOrderRequest,
            response_body: GlomopayCreateOrderResponse,
            router_data: RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>,
        ),
        (
            flow: Authorize,
            request_body: GlomopayAuthorizeRequest,
            response_body: GlomopayAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: GlomopayPaymentSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: GlomopayRefundRequest,
            response_body: GlomopayRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: GlomopayRefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
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
            let mut headers = vec![(
                "Content-Type".to_string(),
                self.common_get_content_type().to_string().into(),
            )];
            let mut auth = self.get_auth_header(&req.connector_config)?;
            headers.append(&mut auth);
            Ok(headers)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.glomopay.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.glomopay.base_url
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Glomopay<T>
{
    fn id(&self) -> &'static str {
        "glomopay"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.glomopay.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = glomopay::GlomopayAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        Ok(vec![(
            "Authorization".to_string(),
            format!("Bearer {}", auth.api_key.peek()).into_masked(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let response: glomopay::GlomopayErrorResponse = res
            .response
            .parse_struct("GlomopayErrorResponse")
            .change_context(crate::utils::response_deserialization_fail(
                res.status_code,
                "glomopay: response body did not match the expected error format",
            ))?;

        with_error_response_body!(event_builder, response);

        let code = response
            .error
            .clone()
            .unwrap_or_else(|| res.status_code.to_string());

        let message = response
            .message
            .clone()
            .unwrap_or_else(|| format!("glomopay: HTTP {}", res.status_code));

        Ok(ErrorResponse {
            status_code: res.status_code,
            code,
            message,
            reason: response.message,
            attempt_status: None,
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Glomopay,
    curl_response: GlomopayGetCustomerResponse,
    flow_name: GetConnectorCustomer,
    resource_common_data: PaymentFlowData,
    flow_request: ConnectorCustomerData,
    flow_response: ConnectorCustomerResponse,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<GetConnectorCustomer, PaymentFlowData, ConnectorCustomerData, ConnectorCustomerResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<GetConnectorCustomer, PaymentFlowData, ConnectorCustomerData, ConnectorCustomerResponse>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            let email = req.request.get_email()?;
            // Percent-encode the email so query-special characters (notably `+`,
            // which is interpreted as space in application/x-www-form-urlencoded)
            // round-trip intact. Without this, emails like `alice+tag@x.com` are
            // decoded server-side as `alice tag@x.com`, the lookup misses, and
            // the fall-through create-customer path duplicates the record.
            Ok(format!(
                "{base_url}customer?email_address={}",
                urlencoding::encode(email.peek())
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Glomopay,
    curl_request: Json(GlomopayCreateCustomerRequest),
    curl_response: GlomopayCreateCustomerResponse,
    flow_name: CreateConnectorCustomer,
    resource_common_data: PaymentFlowData,
    flow_request: ConnectorCustomerData,
    flow_response: ConnectorCustomerResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<CreateConnectorCustomer, PaymentFlowData, ConnectorCustomerData, ConnectorCustomerResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<CreateConnectorCustomer, PaymentFlowData, ConnectorCustomerData, ConnectorCustomerResponse>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}customer"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Glomopay,
    curl_request: Json(GlomopayCreateOrderRequest),
    curl_response: GlomopayCreateOrderResponse,
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
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}orders"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Glomopay,
    curl_request: Json(GlomopayAuthorizeRequest),
    curl_response: GlomopayAuthorizeResponse,
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
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}payment"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Glomopay,
    curl_response: GlomopayPaymentSyncResponse,
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
            let request_id = &req.resource_common_data.connector_request_reference_id;
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}payment?request_id={request_id}"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Glomopay,
    curl_request: Json(GlomopayRefundRequest),
    curl_response: GlomopayRefundResponse,
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
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_refunds(req);
            Ok(format!("{base_url}refunds"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Glomopay,
    curl_response: GlomopayRefundSyncResponse,
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
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_refunds(req);
            let request_id = &req.resource_common_data.connector_request_reference_id;
            // Fail loudly on an empty reference id. Dropping the ?request_id
            // filter would return every refund on the merchant's account and
            // the response transformer would silently pick the first one —
            // effectively reporting an unrelated refund's status as if it
            // belonged to the current one.
            if request_id.is_empty() {
                Err(error_stack::report!(
                    IntegrationError::MissingRequiredField {
                        field_name: "connector_request_reference_id",
                        context: Default::default(),
                    }
                ))
            } else {
                Ok(format!("{base_url}refunds?request_id={request_id}"))
            }
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Glomopay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Glomopay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Glomopay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Glomopay<T>
{
    fn should_do_order_create(&self) -> bool {
        true
    }

    fn should_create_connector_customer(&self) -> bool {
        true
    }

    fn should_lookup_connector_customer(&self) -> bool {
        true
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::GetConnectorCustomer for Glomopay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Glomopay<T>
{
    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"{"entity_type":"payment","event_type":"in_progress","data":{"id":"payt_686f7cc3pe69T","payin_id":"order_686f7c8c7txqj","status":"active","payment_amount":374580,"payment_currency":"AED","converted_amount":102000,"converted_currency":"USD","requested_amount":100000,"requested_currency":"USD","fees":{"txn_fee":{"currency":"USD","amount":1000},"fx_fee":{"currency":"USD","amount":1000},"referral_fee":{"currency":"USD","amount":0}},"error_code":null,"error_description":null,"error_message":null,"funds_available":false}}"#
    }

    fn get_webhook_integrity_checks(&self) -> Vec<connector_types::WebhookIntegrityCheck> {
        vec![connector_types::WebhookIntegrityCheck::ConnectorTransactionId]
    }

    fn verify_webhook_source(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<domain_types::connector_types::ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        // Glomopay does not include a signature/HMAC header on its webhooks.
        // Hardcode to false so callers see the webhook as unverified and
        // trigger a follow-up authenticated PSync before acting on any
        // terminal state change.
        Ok(false)
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<domain_types::connector_types::EventType, error_stack::Report<WebhookError>> {
        let probe: GlomopayWebhookEntityProbe = request
            .body
            .parse_struct("GlomopayWebhookEntityProbe")
            .change_context(WebhookError::WebhookBodyDecodingFailed)
            .attach_printable("Failed to read entity_type from Glomopay webhook")?;

        match probe.entity_type.as_str() {
            "payment" => {
                let payload: GlomopayWebhookPayload = request
                    .body
                    .parse_struct("GlomopayWebhookPayload")
                    .change_context(WebhookError::WebhookBodyDecodingFailed)
                    .attach_printable("Failed to parse Glomopay payment webhook event type")?;
                Ok(payload.get_event_type())
            }
            "refund" => {
                let payload: GlomopayRefundWebhookPayload = request
                    .body
                    .parse_struct("GlomopayRefundWebhookPayload")
                    .change_context(WebhookError::WebhookBodyDecodingFailed)
                    .attach_printable("Failed to parse Glomopay refund webhook event type")?;
                Ok(payload.get_event_type())
            }
            other => Err(error_stack::report!(WebhookError::WebhookEventTypeNotFound)
                .attach_printable(format!("Unknown Glomopay webhook entity_type: {other}"))),
        }
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        let probe: GlomopayWebhookEntityProbe = request
            .body
            .parse_struct("GlomopayWebhookEntityProbe")
            .change_context(WebhookError::WebhookBodyDecodingFailed)
            .attach_printable("Failed to read entity_type from Glomopay webhook")?;

        match probe.entity_type.as_str() {
            "payment" => {
                let payload: GlomopayWebhookPayload = request
                    .body
                    .parse_struct("GlomopayWebhookPayload")
                    .change_context(WebhookError::WebhookBodyDecodingFailed)
                    .attach_printable("Failed to parse Glomopay payment webhook for reference")?;
                Ok(Some(WebhookResourceReference::Payment(
                    PaymentWebhookReference {
                        connector_transaction_id: Some(payload.data.id),
                        merchant_transaction_id: None,
                    },
                )))
            }
            "refund" => {
                let payload: GlomopayRefundWebhookPayload = request
                    .body
                    .parse_struct("GlomopayRefundWebhookPayload")
                    .change_context(WebhookError::WebhookBodyDecodingFailed)
                    .attach_printable("Failed to parse Glomopay refund webhook for reference")?;
                Ok(Some(WebhookResourceReference::Refund(
                    RefundWebhookReference {
                        connector_refund_id: Some(payload.data.id),
                        merchant_refund_id: None,
                        connector_transaction_id: payload.data.payment_id,
                    },
                )))
            }
            _ => Ok(None),
        }
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<domain_types::connector_types::ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<domain_types::connector_types::EventContext>,
    ) -> Result<
        domain_types::connector_types::WebhookDetailsResponse,
        error_stack::Report<WebhookError>,
    > {
        let payload: GlomopayWebhookPayload = request
            .body
            .parse_struct("GlomopayWebhookPayload")
            .change_context(WebhookError::WebhookBodyDecodingFailed)
            .attach_printable("Failed to parse Glomopay payment webhook body")?;
        Ok(payload.into_webhook_details_response(200, &request.body))
    }

    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<domain_types::connector_types::ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<
        domain_types::connector_types::RefundWebhookDetailsResponse,
        error_stack::Report<WebhookError>,
    > {
        let payload: GlomopayRefundWebhookPayload = request
            .body
            .parse_struct("GlomopayRefundWebhookPayload")
            .change_context(WebhookError::WebhookBodyDecodingFailed)
            .attach_printable("Failed to parse Glomopay refund webhook body")?;
        Ok(payload.into_refund_webhook_details_response(200, &request.body))
    }

    fn get_webhook_resource_object(
        &self,
        request: RequestDetails,
    ) -> Result<Box<dyn hyperswitch_masking::ErasedMaskSerialize>, error_stack::Report<WebhookError>>
    {
        let probe: GlomopayWebhookEntityProbe = request
            .body
            .parse_struct("GlomopayWebhookEntityProbe")
            .change_context(WebhookError::WebhookResourceObjectNotFound)
            .attach_printable("Failed to read entity_type from Glomopay webhook")?;

        match probe.entity_type.as_str() {
            "refund" => {
                let payload: GlomopayRefundWebhookPayload = request
                    .body
                    .parse_struct("GlomopayRefundWebhookPayload")
                    .change_context(WebhookError::WebhookResourceObjectNotFound)
                    .attach_printable("Failed to parse Glomopay refund webhook resource object")?;
                Ok(Box::new(payload))
            }
            _ => {
                let payload: GlomopayWebhookPayload = request
                    .body
                    .parse_struct("GlomopayWebhookPayload")
                    .change_context(WebhookError::WebhookResourceObjectNotFound)
                    .attach_printable("Failed to parse Glomopay payment webhook resource object")?;
                Ok(Box::new(payload))
            }
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Glomopay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for Glomopay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Glomopay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Glomopay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Glomopay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Glomopay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Glomopay<T>
{
}

crate::connectors::macros::macro_connector_payout_implementation!(
    connector: Glomopay,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

crate::connectors::macros::macro_connector_flow_status_impls!(
    connector: Glomopay,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        Capture,
        ClientAuthenticationToken,
        IncrementalAuthorization,
        MandateRevoke,
        PaymentMethodToken,
        RepeatPayment,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        SetupMandate
    ],
    not_supported: [
        Void,
        VoidPostRefund,
        Accept,
        DefendDispute,
        SubmitEvidence,
        Authenticate,
        PreAuthenticate,
        PostAuthenticate,
        VoidPC
    ],
);
