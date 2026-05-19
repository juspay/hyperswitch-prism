pub mod transformers;

use std::{self, fmt::Debug};

use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt, StringMajorUnit};
use domain_types::{
    connector_flow::{Authorize, Refund},
    connector_types::{
        ConnectorWebhookSecrets, EventContext, PaymentFlowData, PaymentsAuthorizeData,
        PaymentsResponseData, RefundFlowData, RefundsData, RefundsResponseData, RequestDetails,
    },
    errors,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};

use hyperswitch_masking::Maskable;
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    self as trustly, TrustlyErrorResponse, TrustlyPaymentRequest, TrustlyPaymentsResponse,
    TrustlyRefundRequest, TrustlyRefundResponse, TrustlyWebhookBody,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};
use error_stack::ResultExt;

// Trait for types that can provide access tokens
pub trait AccessTokenProvider {
    fn get_access_token(&self) -> CustomResult<String, errors::IntegrationError>;
}

impl AccessTokenProvider for PaymentFlowData {
    fn get_access_token(&self) -> CustomResult<String, errors::IntegrationError> {
        self.get_access_token().change_context(
            errors::IntegrationError::MissingConnectorTransactionID {
                context: Default::default(),
            },
        )
    }
}

impl AccessTokenProvider for RefundFlowData {
    fn get_access_token(&self) -> CustomResult<String, errors::IntegrationError> {
        self.get_access_token().change_context(
            errors::IntegrationError::MissingConnectorTransactionID {
                context: Default::default(),
            },
        )
    }
}

pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Trustly<T>
{
}
macros::macro_connector_payout_implementation!(
    connector: Trustly,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Trustly<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Trustly<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Trustly<T>
{
    fn should_do_access_token(&self, _payment_method: Option<common_enums::PaymentMethod>) -> bool {
        false
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Trustly<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Trustly<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Trustly<T>
{
}

macros::create_all_prerequisites!(
    connector_name: Trustly,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: TrustlyPaymentRequest,
            response_body: TrustlyPaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: TrustlyRefundRequest,
            response_body: TrustlyRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: StringMajorUnit
    ],
    member_functions: {
        pub fn build_headers<F, FlowData, Req, Res>(
            &self,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError>
        where
            FlowData: AccessTokenProvider,
            Self: ConnectorIntegrationV2<F, FlowData, Req, Res>,
        {
            Ok(Vec::new())
        }

        pub fn connector_base_url<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> String {
            req.resource_common_data.connectors.trustly.base_url.to_string()
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Trustly<T>
{
    fn id(&self) -> &'static str {
        "trustly"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json; charset=UTF-8"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.trustly.base_url
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let response: TrustlyErrorResponse = res
            .response
            .parse_struct("TrustlyErrorResponse")
            .change_context(crate::utils::response_deserialization_fail(
                res.status_code,
                "trustly: response body did not match the expected format",
            ))?;

        with_error_response_body!(event_builder, response);

        Ok(ErrorResponse {
            code: response.error.code.to_string(),
            message: response.error.message.clone(),
            reason: Some(response.error.message),
            status_code: res.status_code,
            attempt_status: None,
            connector_transaction_id: Some(response.error.error.uuid),
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        })
    }
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Trustly,
    curl_request: Json(TrustlyPaymentRequest),
    curl_response: TrustlyPaymentsResponse,
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
            _req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            Ok(Vec::new())
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = self.connector_base_url(req);
            Ok(base_url)
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Trustly,
    curl_request: Json(TrustlyRefundRequest),
    curl_response: TrustlyRefundResponse,
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
            _req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            Ok(Vec::new())
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            Ok(req.resource_common_data.connectors.trustly.base_url.clone())
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Trustly<T>
{
    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<errors::WebhookError>> {
        let webhook_body: TrustlyWebhookBody = request
            .body
            .parse_struct("TrustlyWebhookBody")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        let webhook_secret = match connector_webhook_secret {
            Some(secrets) => secrets.secret,
            None => return Ok(false),
        };

        trustly::verify_webhook_signature(webhook_body, webhook_secret)
    }

    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"{"method":"charge","params":{"data":{"orderid":"probe_order_001","amount":"10.00","currency":"EUR","enduserid":"probe_user"}}}"#
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<domain_types::connector_types::EventType, error_stack::Report<errors::WebhookError>>
    {
        let webhook_body: TrustlyWebhookBody = request
            .body
            .parse_struct("TrustlyWebhookBody")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        Ok(trustly::get_webhook_event(webhook_body.method))
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<
        Option<domain_types::connector_types::WebhookResourceReference>,
        error_stack::Report<errors::WebhookError>,
    > {
        let webhook_body: TrustlyWebhookBody = request
            .body
            .parse_struct("TrustlyWebhookBody")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        let webhook_resource_reference = match webhook_body.method {
            trustly::TrustlyWebhookMethod::Credit
            | trustly::TrustlyWebhookMethod::Debit
            | trustly::TrustlyWebhookMethod::Cancel
            | trustly::TrustlyWebhookMethod::Account
            | trustly::TrustlyWebhookMethod::Pending => {
                domain_types::connector_types::WebhookResourceReference::Payment(
                    domain_types::connector_types::PaymentWebhookReference {
                        connector_transaction_id: Some(webhook_body.params.data.orderid.clone()),
                        merchant_transaction_id: None,
                    },
                )
            }
            trustly::TrustlyWebhookMethod::PayoutConfirmation
            | trustly::TrustlyWebhookMethod::PayoutFailed => {
                domain_types::connector_types::WebhookResourceReference::Refund(
                    domain_types::connector_types::RefundWebhookReference {
                        connector_refund_id: Some(webhook_body.params.data.orderid.clone()),
                        merchant_refund_id: None,
                        connector_transaction_id: Some(webhook_body.params.data.orderid.clone()),
                    },
                )
            }
        };

        Ok(Some(webhook_resource_reference))
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<
        domain_types::connector_types::WebhookDetailsResponse,
        error_stack::Report<errors::WebhookError>,
    > {
        let request_body_copy = request.body.clone();
        let details: TrustlyWebhookBody = request
            .body
            .parse_struct("TrustlyWebhookBody")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        let status = trustly::get_trustly_payment_webhook_status(&details.method);

        Ok(domain_types::connector_types::WebhookDetailsResponse {
            resource_id: Some(
                domain_types::connector_types::ResponseId::ConnectorTransactionId(
                    details.params.data.orderid.clone(),
                ),
            ),
            status,
            connector_response_reference_id: None,
            mandate_reference: None,
            error_code: None,
            error_message: None,
            error_reason: None,
            raw_connector_response: Some(String::from_utf8_lossy(&request_body_copy).to_string()),
            status_code: 200,
            response_headers: None,
            amount_captured: None,
            minor_amount_captured: None,
            network_txn_id: None,
            payment_method_update: None,
            sender_payment_instrument_id: details.params.data.accountid.clone(),
        })
    }

    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<
        domain_types::connector_types::RefundWebhookDetailsResponse,
        error_stack::Report<errors::WebhookError>,
    > {
        let request_body_copy = request.body.clone();
        let details: TrustlyWebhookBody = request
            .body
            .parse_struct("TrustlyWebhookBody")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        let status = trustly::get_trustly_refund_webhook_status(&details.method);

        Ok(
            domain_types::connector_types::RefundWebhookDetailsResponse {
                connector_refund_id: Some(details.params.data.orderid.clone()),
                status,
                connector_response_reference_id: Some(details.params.uuid.clone()),
                error_code: details.params.data.errorcode,
                error_message: details.params.data.errormessage,
                raw_connector_response: Some(
                    String::from_utf8_lossy(&request_body_copy).to_string(),
                ),
                status_code: 200,
                response_headers: None,
            },
        )
    }

    fn get_webhook_resource_object(
        &self,
        request: RequestDetails,
    ) -> Result<
        Box<dyn hyperswitch_masking::ErasedMaskSerialize>,
        error_stack::Report<errors::WebhookError>,
    > {
        let details: TrustlyWebhookBody = request
            .body
            .parse_struct("TrustlyWebhookBody")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;
        Ok(Box::new(details))
    }
}

macros::macro_connector_flow_status_impls!(
    connector: Trustly,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        ClientAuthenticationToken,
        Void,
        Capture,
        SetupMandate,
        RepeatPayment,
        ServerSessionAuthenticationToken,
        PaymentMethodToken,
        PreAuthenticate,
        Authenticate,
        PSync,
        RSync,
    ],
    not_supported: [
        IncrementalAuthorization,
        CreateOrder,
        SubmitEvidence,
        DefendDispute,
        Accept,
        CreateConnectorCustomer,
        PostAuthenticate,
        MandateRevoke,
        ServerAuthenticationToken,
        VoidPC,
    ],
);
