mod requests;
mod responses;
pub mod transformers;

use std::fmt::Debug;

use base64::Engine;
use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt, Method};
use domain_types::{
    connector_flow::{Authorize, Capture, IncrementalAuthorization, PSync, RSync, Refund, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsIncrementalAuthorizationData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, ResponseId,
    },
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::{Connectors, HasConnectors},
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use ring::{digest, hmac};
use serde::Serialize;
use time::OffsetDateTime;
use transformers::{self as barclaycard};

use requests::{
    BarclaycardCaptureRequest, BarclaycardPaymentsRequest, BarclaycardRefundRequest,
    BarclaycardVoidRequest,
};
use responses::{
    BarclaycardAuthorizeResponse, BarclaycardCaptureResponse, BarclaycardRefundResponse,
    BarclaycardRsyncResponse, BarclaycardTransactionResponse, BarclaycardVoidResponse,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};
use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;

pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
pub const V_C_MERCHANT_ID: &str = "v-c-merchant-id";

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const ACCEPT: &str = "Accept";
}

fn barclaycard_flow_not_supported(flow: &str) -> error_stack::Report<IntegrationError> {
    error_stack::report!(IntegrationError::FlowNotSupported {
        flow: flow.to_string(),
        connector: "Barclaycard".to_string(),
        context: Default::default(),
    })
}
fn barclaycard_not_implemented(flow: &str) -> error_stack::Report<IntegrationError> {
    error_stack::report!(IntegrationError::not_implemented(format!(
        "{flow} flow for barclaycard"
    )))
}

macros::create_amount_converter_wrapper!(connector_name: Barclaycard, amount_type: StringMajorUnit);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    > for Barclaycard<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            IncrementalAuthorization,
            PaymentFlowData,
            PaymentsIncrementalAuthorizationData,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(barclaycard_not_implemented("incremental_authorization"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Barclaycard<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Barclaycard<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Barclaycard<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Barclaycard<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Barclaycard<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentIncrementalAuthorization for Barclaycard<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Barclaycard<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Barclaycard<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Barclaycard<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerSessionAuthentication for Barclaycard<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Barclaycard<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Barclaycard,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerAuthentication for Barclaycard<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Barclaycard<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Barclaycard<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Barclaycard<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Barclaycard<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::AcceptDispute for Barclaycard<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::DisputeDefend for Barclaycard<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SubmitEvidenceV2 for Barclaycard<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Barclaycard<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Barclaycard<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Barclaycard<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Barclaycard<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for Barclaycard<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for Barclaycard<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Barclaycard<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Barclaycard<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Barclaycard<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::MandateRevokeV2 for Barclaycard<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::PostAuthenticate,
        PaymentFlowData,
        domain_types::connector_types::PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    > for Barclaycard<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            domain_types::connector_flow::PostAuthenticate,
            PaymentFlowData,
            domain_types::connector_types::PaymentsPostAuthenticateData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(barclaycard_not_implemented("post_authenticate"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::Authenticate,
        PaymentFlowData,
        domain_types::connector_types::PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    > for Barclaycard<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            domain_types::connector_flow::Authenticate,
            PaymentFlowData,
            domain_types::connector_types::PaymentsAuthenticateData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(barclaycard_not_implemented("authenticate"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::PreAuthenticate,
        PaymentFlowData,
        domain_types::connector_types::PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    > for Barclaycard<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            domain_types::connector_flow::PreAuthenticate,
            PaymentFlowData,
            domain_types::connector_types::PaymentsPreAuthenticateData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(barclaycard_not_implemented("pre_authenticate"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::SubmitEvidence,
        domain_types::connector_types::DisputeFlowData,
        domain_types::connector_types::SubmitEvidenceData,
        domain_types::connector_types::DisputeResponseData,
    > for Barclaycard<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            domain_types::connector_flow::SubmitEvidence,
            domain_types::connector_types::DisputeFlowData,
            domain_types::connector_types::SubmitEvidenceData,
            domain_types::connector_types::DisputeResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(barclaycard_flow_not_supported("submit_evidence"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::DefendDispute,
        domain_types::connector_types::DisputeFlowData,
        domain_types::connector_types::DisputeDefendData,
        domain_types::connector_types::DisputeResponseData,
    > for Barclaycard<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            domain_types::connector_flow::DefendDispute,
            domain_types::connector_types::DisputeFlowData,
            domain_types::connector_types::DisputeDefendData,
            domain_types::connector_types::DisputeResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(barclaycard_flow_not_supported("defend_dispute"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::Accept,
        domain_types::connector_types::DisputeFlowData,
        domain_types::connector_types::AcceptDisputeData,
        domain_types::connector_types::DisputeResponseData,
    > for Barclaycard<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            domain_types::connector_flow::Accept,
            domain_types::connector_types::DisputeFlowData,
            domain_types::connector_types::AcceptDisputeData,
            domain_types::connector_types::DisputeResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(barclaycard_flow_not_supported("accept_dispute"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::RepeatPayment,
        PaymentFlowData,
        domain_types::connector_types::RepeatPaymentData<T>,
        PaymentsResponseData,
    > for Barclaycard<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            domain_types::connector_flow::RepeatPayment,
            PaymentFlowData,
            domain_types::connector_types::RepeatPaymentData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(barclaycard_not_implemented("repeat_payment"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::SetupMandate,
        PaymentFlowData,
        domain_types::connector_types::SetupMandateRequestData<T>,
        PaymentsResponseData,
    > for Barclaycard<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            domain_types::connector_flow::SetupMandate,
            PaymentFlowData,
            domain_types::connector_types::SetupMandateRequestData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(barclaycard_not_implemented("setup_mandate"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::VoidPC,
        PaymentFlowData,
        domain_types::connector_types::PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for Barclaycard<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            domain_types::connector_flow::VoidPC,
            PaymentFlowData,
            domain_types::connector_types::PaymentsCancelPostCaptureData,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(barclaycard_not_implemented("void_post_capture"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::PaymentMethodToken,
        PaymentFlowData,
        domain_types::connector_types::PaymentMethodTokenizationData<T>,
        domain_types::connector_types::PaymentMethodTokenResponse,
    > for Barclaycard<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            domain_types::connector_flow::PaymentMethodToken,
            PaymentFlowData,
            domain_types::connector_types::PaymentMethodTokenizationData<T>,
            domain_types::connector_types::PaymentMethodTokenResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(barclaycard_not_implemented("payment_method_token"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::CreateConnectorCustomer,
        PaymentFlowData,
        domain_types::connector_types::ConnectorCustomerData,
        domain_types::connector_types::ConnectorCustomerResponse,
    > for Barclaycard<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            domain_types::connector_flow::CreateConnectorCustomer,
            PaymentFlowData,
            domain_types::connector_types::ConnectorCustomerData,
            domain_types::connector_types::ConnectorCustomerResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(barclaycard_not_implemented("create_connector_customer"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::ServerAuthenticationToken,
        PaymentFlowData,
        domain_types::connector_types::ServerAuthenticationTokenRequestData,
        domain_types::connector_types::ServerAuthenticationTokenResponseData,
    > for Barclaycard<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            domain_types::connector_flow::ServerAuthenticationToken,
            PaymentFlowData,
            domain_types::connector_types::ServerAuthenticationTokenRequestData,
            domain_types::connector_types::ServerAuthenticationTokenResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(barclaycard_not_implemented(
            "create_server_authentication_token",
        ))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::ServerSessionAuthenticationToken,
        PaymentFlowData,
        domain_types::connector_types::ServerSessionAuthenticationTokenRequestData,
        domain_types::connector_types::ServerSessionAuthenticationTokenResponseData,
    > for Barclaycard<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            domain_types::connector_flow::ServerSessionAuthenticationToken,
            PaymentFlowData,
            domain_types::connector_types::ServerSessionAuthenticationTokenRequestData,
            domain_types::connector_types::ServerSessionAuthenticationTokenResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(barclaycard_not_implemented(
            "create_server_session_authentication_token",
        ))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::ClientAuthenticationToken,
        PaymentFlowData,
        domain_types::connector_types::ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    > for Barclaycard<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            domain_types::connector_flow::ClientAuthenticationToken,
            PaymentFlowData,
            domain_types::connector_types::ClientAuthenticationTokenRequestData,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(barclaycard_not_implemented(
            "create_client_authentication_token",
        ))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::CreateOrder,
        PaymentFlowData,
        domain_types::connector_types::PaymentCreateOrderData,
        domain_types::connector_types::PaymentCreateOrderResponse,
    > for Barclaycard<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            domain_types::connector_flow::CreateOrder,
            PaymentFlowData,
            domain_types::connector_types::PaymentCreateOrderData,
            domain_types::connector_types::PaymentCreateOrderResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(barclaycard_flow_not_supported("create_order"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::MandateRevoke,
        PaymentFlowData,
        domain_types::connector_types::MandateRevokeRequestData,
        domain_types::connector_types::MandateRevokeResponseData,
    > for Barclaycard<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            domain_types::connector_flow::MandateRevoke,
            PaymentFlowData,
            domain_types::connector_types::MandateRevokeRequestData,
            domain_types::connector_types::MandateRevokeResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(barclaycard_flow_not_supported("mandate_revoke"))
    }
}

macros::create_all_prerequisites!(
    connector_name: Barclaycard,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: BarclaycardPaymentsRequest<T>,
            response_body: BarclaycardAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: BarclaycardCaptureRequest,
            response_body: BarclaycardCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: BarclaycardVoidRequest,
            response_body: BarclaycardVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: BarclaycardTransactionResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: BarclaycardRefundRequest,
            response_body: BarclaycardRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: BarclaycardRsyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        )
    ],
    amount_converters: [],
    member_functions: {
        pub fn generate_digest(&self, payload: &[u8]) -> String {
            let payload_digest = digest::digest(&digest::SHA256, payload);
            BASE64_ENGINE.encode(payload_digest)
        }

        pub fn generate_signature(
            &self,
            auth: barclaycard::BarclaycardAuthType,
            host: String,
            resource: &str,
            payload: &String,
            date: OffsetDateTime,
            http_method: Method,
        ) -> CustomResult<String, IntegrationError> {
            let barclaycard::BarclaycardAuthType {
                api_key,
                merchant_account,
                api_secret
} = auth;
            let is_post_or_put_method = matches!(http_method, Method::Post | Method::Put);
            let digest_str = if is_post_or_put_method { "digest " } else { "" };
            let headers = format!("host date (request-target) {digest_str}{V_C_MERCHANT_ID}");
            let request_target = if is_post_or_put_method {
                format!("(request-target): {} {resource}\ndigest: SHA-256={payload}\n", http_method.to_string().to_lowercase())
            } else {
                format!("(request-target): get {resource}\n")
            };
            let signature_string = format!(
                "host: {host}\ndate: {date}\n{request_target}{V_C_MERCHANT_ID}: {}",
                merchant_account.peek()
            );
            let key_value = BASE64_ENGINE
                .decode(api_secret.expose())
                .change_context(IntegrationError::InvalidConnectorConfig {
                    config: "connector_account_details.api_secret",
                context: Default::default()
                })?;
            let key = hmac::Key::new(hmac::HMAC_SHA256, &key_value);
            let signature_value =
                BASE64_ENGINE.encode(hmac::sign(&key, signature_string.as_bytes()).as_ref());
            let signature_header = format!(
                r#"keyid="{}", algorithm="HmacSHA256", headers="{headers}", signature="{signature_value}""#,
                api_key.peek()
            );

            Ok(signature_header)
        }

        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
            FCD: HasConnectors,
        {
            let date = OffsetDateTime::now_utc();
            let barclaycard_req = self.get_request_body(req)?;
            let http_method = self.get_http_method();
            let auth = barclaycard::BarclaycardAuthType::try_from(&req.connector_config)?;
            let merchant_account = auth.merchant_account.clone();
            let base_url = self.base_url(req.resource_common_data.connectors());
            let barclaycard_host =
                url::Url::parse(base_url).change_context(IntegrationError::RequestEncodingFailed { context: Default::default() })?;
            let host = barclaycard_host
                .host_str()
                .ok_or(IntegrationError::RequestEncodingFailed { context: Default::default() })?;
            let path: String = self
                .get_url(req)?
                .chars()
                .skip(base_url.len())
                .collect();
            let sha256 = self.generate_digest(
                barclaycard_req
                    .map(|req| req.get_inner_value().expose())
                    .unwrap_or_default()
                    .as_bytes()
            );
            let signature = self.generate_signature(
                auth,
                host.to_string(),
                path.as_str(),
                &sha256,
                date,
                http_method,
            )?;

            let mut headers = vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.common_get_content_type().to_string().into(),
                ),
                (
                    headers::ACCEPT.to_string(),
                    "application/hal+json;charset=utf-8".to_string().into(),
                ),
                (V_C_MERCHANT_ID.to_string(), merchant_account.into_masked()),
                ("Date".to_string(), date.to_string().into()),
                ("Host".to_string(), host.to_string().into()),
                ("Signature".to_string(), signature.into_masked()),
            ];
            if matches!(http_method, Method::Post | Method::Put) {
                headers.push((
                    "Digest".to_string(),
                    format!("SHA-256={sha256}").into_masked(),
                ));
            }
            Ok(headers)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.barclaycard.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.barclaycard.base_url
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Barclaycard,
    curl_request: Json(BarclaycardPaymentsRequest<T>),
    curl_response: BarclaycardAuthorizeResponse,
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
            Ok(format!("{}/pts/v2/payments/", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Barclaycard,
    curl_request: Json(BarclaycardCaptureRequest),
    curl_response: BarclaycardCaptureResponse,
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

        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let connector_payment_id = match &req.request.connector_transaction_id {
                ResponseId::ConnectorTransactionId(id) => Ok(id),
                _ => Err(IntegrationError::MissingConnectorTransactionID { context: Default::default() })
}?;
            Ok(format!(
                "{}/pts/v2/payments/{}/captures",
                self.connector_base_url_payments(req),
                connector_payment_id
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Barclaycard,
    curl_request: Json(BarclaycardVoidRequest),
    curl_response: BarclaycardVoidResponse,
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

        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}/pts/v2/payments/{}/voids",
                self.connector_base_url_payments(req),
                req.request.connector_transaction_id
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Barclaycard,
    curl_response: BarclaycardTransactionResponse,
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
            let connector_transaction_id = match &req.request.connector_transaction_id {
                ResponseId::ConnectorTransactionId(id) => Ok(id),
                _ => Err(IntegrationError::MissingConnectorTransactionID { context: Default::default() })
}?;
            Ok(format!(
                "{}/tss/v2/transactions/{}",
                self.connector_base_url_payments(req),
                connector_transaction_id
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Barclaycard,
    curl_request: Json(BarclaycardRefundRequest),
    curl_response: BarclaycardRefundResponse,
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
            let connector_transaction_id = &req.request.connector_transaction_id;
            Ok(format!(
                "{}/pts/v2/payments/{}/refunds",
                self.connector_base_url_refunds(req),
                connector_transaction_id
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Barclaycard,
    curl_response: BarclaycardRsyncResponse,
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
            let refund_id = &req.request.connector_refund_id;
            Ok(format!(
                "{}/tss/v2/transactions/{}",
                self.connector_base_url_refunds(req),
                refund_id
            ))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Barclaycard<T>
{
    fn id(&self) -> &'static str {
        "barclaycard"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.barclaycard.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        _auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        // Auth is handled via signature in build_headers
        Ok(vec![])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: responses::BarclaycardErrorResponse = res
            .response
            .parse_struct("BarclaycardErrorResponse")
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "barclaycard: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        match response {
            responses::BarclaycardErrorResponse::Standard(error_response) => {
                with_error_response_body!(event_builder, error_response);

                let detailed_error_info =
                    error_response
                        .error_information
                        .as_ref()
                        .and_then(|error_info| {
                            error_info.details.as_ref().map(|details| {
                                details
                                    .iter()
                                    .map(|d| format!("{} : {}", d.field, d.reason))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            })
                        });

                let reason = match (
                    error_response
                        .error_information
                        .as_ref()
                        .map(|e| e.message.clone()),
                    detailed_error_info,
                ) {
                    (Some(message), Some(details)) => {
                        Some(format!("{message}, detailed_error_information: {details}"))
                    }
                    (Some(message), None) => Some(message),
                    (None, Some(details)) => Some(details),
                    (None, None) => error_response.message.clone(),
                };

                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: error_response
                        .error_information
                        .map(|e| e.reason.clone())
                        .or_else(|| error_response.reason.clone())
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                    message: error_response
                        .message
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                    reason,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                })
            }
            responses::BarclaycardErrorResponse::Server(server_error) => {
                with_error_response_body!(event_builder, server_error);

                let attempt_status = match server_error.reason {
                    Some(ref reason) => match reason {
                        responses::Reason::SystemError => {
                            Some(common_enums::AttemptStatus::Failure)
                        }
                        responses::Reason::ServerTimeout | responses::Reason::ServiceTimeout => {
                            None
                        }
                    },
                    None => None,
                };

                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: server_error
                        .status
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                    message: server_error
                        .message
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                    reason: server_error.status.clone(),
                    attempt_status,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                })
            }
            responses::BarclaycardErrorResponse::Authentication(auth_error) => {
                with_error_response_body!(event_builder, auth_error);

                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: "AUTHENTICATION_ERROR".to_string(),
                    message: auth_error.response.rmsg,
                    reason: None,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                })
            }
        }
    }
}
