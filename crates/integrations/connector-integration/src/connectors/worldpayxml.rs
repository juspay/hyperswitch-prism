mod requests;
mod responses;
pub mod transformers;

use std::fmt::Debug;

use base64::Engine;
use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt, StringMinorUnit};
use domain_types::{
    connector_flow::{Authorize, Capture, IncrementalAuthorization, PSync, RSync, Refund, Void},
    connector_types::{
        MandateRevokeResponseData, PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsIncrementalAuthorizationData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
    },
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
use transformers::{self as worldpayxml};

use requests::{
    WorldpayxmlCaptureRequest, WorldpayxmlPSyncRequest, WorldpayxmlPaymentsRequest,
    WorldpayxmlRSyncRequest, WorldpayxmlRefundRequest, WorldpayxmlVoidRequest,
};
use responses::{
    WorldpayxmlAuthorizeResponse, WorldpayxmlCaptureResponse, WorldpayxmlRefundResponse,
    WorldpayxmlRsyncResponse, WorldpayxmlTransactionResponse, WorldpayxmlVoidResponse,
};

use super::macros::{self, GetSoapXml};
use crate::{types::ResponseRouterData, utils, with_error_response_body};
use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;

pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

const CONTENT_TYPE_XML: &str = "text/xml";

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

macros::create_amount_converter_wrapper!(connector_name: Worldpayxml, amount_type: StringMinorUnit);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    > for Worldpayxml<T>
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
        Err(utils::ConnectorFlowStatusExt::flow_not_implemented(
            self,
            "incremental_authorization",
        ))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentIncrementalAuthorization for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerSessionAuthentication for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Worldpayxml<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Worldpayxml,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::MandateRevokeV2 for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerAuthentication for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::AcceptDispute for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::DisputeDefend for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SubmitEvidenceV2 for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::PostAuthenticate,
        PaymentFlowData,
        domain_types::connector_types::PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    > for Worldpayxml<T>
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
        Err(utils::ConnectorFlowStatusExt::flow_not_implemented(
            self,
            "post_authenticate",
        ))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::Authenticate,
        PaymentFlowData,
        domain_types::connector_types::PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    > for Worldpayxml<T>
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
        Err(utils::ConnectorFlowStatusExt::flow_not_implemented(
            self,
            "authenticate",
        ))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::PreAuthenticate,
        PaymentFlowData,
        domain_types::connector_types::PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    > for Worldpayxml<T>
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
        Err(utils::ConnectorFlowStatusExt::flow_not_implemented(
            self,
            "pre_authenticate",
        ))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::SubmitEvidence,
        domain_types::connector_types::DisputeFlowData,
        domain_types::connector_types::SubmitEvidenceData,
        domain_types::connector_types::DisputeResponseData,
    > for Worldpayxml<T>
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
        Err(utils::ConnectorFlowStatusExt::flow_not_implemented(
            self,
            "submit_evidence",
        ))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::DefendDispute,
        domain_types::connector_types::DisputeFlowData,
        domain_types::connector_types::DisputeDefendData,
        domain_types::connector_types::DisputeResponseData,
    > for Worldpayxml<T>
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
        Err(utils::ConnectorFlowStatusExt::flow_not_implemented(
            self,
            "defend_dispute",
        ))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::Accept,
        domain_types::connector_types::DisputeFlowData,
        domain_types::connector_types::AcceptDisputeData,
        domain_types::connector_types::DisputeResponseData,
    > for Worldpayxml<T>
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
        Err(error_stack::report!(IntegrationError::FlowNotSupported {
            flow: "accept_dispute".to_string(),
            connector: "Worldpayxml".to_string(),
            context: Default::default(),
        }))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::RepeatPayment,
        PaymentFlowData,
        domain_types::connector_types::RepeatPaymentData<T>,
        PaymentsResponseData,
    > for Worldpayxml<T>
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
        Err(utils::ConnectorFlowStatusExt::flow_not_implemented(
            self,
            "repeat_payment",
        ))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::SetupMandate,
        PaymentFlowData,
        domain_types::connector_types::SetupMandateRequestData<T>,
        PaymentsResponseData,
    > for Worldpayxml<T>
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
        Err(utils::ConnectorFlowStatusExt::flow_not_implemented(
            self,
            "setup_mandate",
        ))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::VoidPC,
        PaymentFlowData,
        domain_types::connector_types::PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for Worldpayxml<T>
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
        Err(utils::ConnectorFlowStatusExt::flow_not_implemented(
            self,
            "void_post_capture",
        ))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::PaymentMethodToken,
        PaymentFlowData,
        domain_types::connector_types::PaymentMethodTokenizationData<T>,
        domain_types::connector_types::PaymentMethodTokenResponse,
    > for Worldpayxml<T>
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
        Err(utils::ConnectorFlowStatusExt::flow_not_implemented(
            self,
            "payment_method_token",
        ))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::CreateConnectorCustomer,
        PaymentFlowData,
        domain_types::connector_types::ConnectorCustomerData,
        domain_types::connector_types::ConnectorCustomerResponse,
    > for Worldpayxml<T>
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
        Err(utils::ConnectorFlowStatusExt::flow_not_implemented(
            self,
            "create_connector_customer",
        ))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::ServerAuthenticationToken,
        PaymentFlowData,
        domain_types::connector_types::ServerAuthenticationTokenRequestData,
        domain_types::connector_types::ServerAuthenticationTokenResponseData,
    > for Worldpayxml<T>
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
        Err(utils::ConnectorFlowStatusExt::flow_not_implemented(
            self,
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
    > for Worldpayxml<T>
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
        Err(utils::ConnectorFlowStatusExt::flow_not_implemented(
            self,
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
    > for Worldpayxml<T>
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
        Err(utils::ConnectorFlowStatusExt::flow_not_implemented(
            self,
            "create_client_authentication_token",
        ))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::MandateRevoke,
        PaymentFlowData,
        domain_types::connector_types::MandateRevokeRequestData,
        MandateRevokeResponseData,
    > for Worldpayxml<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            domain_types::connector_flow::MandateRevoke,
            PaymentFlowData,
            domain_types::connector_types::MandateRevokeRequestData,
            MandateRevokeResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(utils::ConnectorFlowStatusExt::flow_not_implemented(
            self,
            "mandate_revoke",
        ))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::CreateOrder,
        PaymentFlowData,
        domain_types::connector_types::PaymentCreateOrderData,
        domain_types::connector_types::PaymentCreateOrderResponse,
    > for Worldpayxml<T>
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
        Err(utils::ConnectorFlowStatusExt::flow_not_implemented(
            self,
            "create_order",
        ))
    }
}

macros::create_all_prerequisites!(
    connector_name: Worldpayxml,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: WorldpayxmlPaymentsRequest,
            response_body: WorldpayxmlAuthorizeResponse,
            response_format: xml,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: WorldpayxmlCaptureRequest,
            response_body: WorldpayxmlCaptureResponse,
            response_format: xml,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: WorldpayxmlVoidRequest,
            response_body: WorldpayxmlVoidResponse,
            response_format: xml,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: WorldpayxmlPSyncRequest,
            response_body: WorldpayxmlTransactionResponse,
            response_format: xml,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: WorldpayxmlRefundRequest,
            response_body: WorldpayxmlRefundResponse,
            response_format: xml,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            request_body: WorldpayxmlRSyncRequest,
            response_body: WorldpayxmlRsyncResponse,
            response_format: xml,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: StringMinorUnit
    ],
    member_functions: {
        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.worldpayxml.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.worldpayxml.base_url
        }

        pub fn build_auth_header(
            &self,
            auth: worldpayxml::WorldpayxmlAuthType,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let credentials = format!("{}:{}",
                auth.api_username.expose(),
                auth.api_password.expose()
            );
            let encoded = BASE64_ENGINE.encode(credentials.as_bytes());
            Ok(vec![
                (headers::AUTHORIZATION.to_string(), format!("Basic {}", encoded).into_masked()),
            ])
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Worldpayxml,
    curl_request: SoapXml(WorldpayxmlPaymentsRequest),
    curl_response: WorldpayxmlAuthorizeResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let auth = worldpayxml::WorldpayxmlAuthType::try_from(&req.connector_config)?;
            let mut headers = vec![
                (headers::CONTENT_TYPE.to_string(), CONTENT_TYPE_XML.to_string().into()),
            ];
            headers.extend(self.build_auth_header(auth)?);
            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Worldpayxml,
    curl_request: SoapXml(WorldpayxmlCaptureRequest),
    curl_response: WorldpayxmlCaptureResponse,
    flow_name: Capture,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let auth = worldpayxml::WorldpayxmlAuthType::try_from(&req.connector_config)?;
            let mut headers = vec![
                (headers::CONTENT_TYPE.to_string(), CONTENT_TYPE_XML.to_string().into()),
            ];
            headers.extend(self.build_auth_header(auth)?);
            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Worldpayxml,
    curl_request: SoapXml(WorldpayxmlVoidRequest),
    curl_response: WorldpayxmlVoidResponse,
    flow_name: Void,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentVoidData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let auth = worldpayxml::WorldpayxmlAuthType::try_from(&req.connector_config)?;
            let mut headers = vec![
                (headers::CONTENT_TYPE.to_string(), CONTENT_TYPE_XML.to_string().into()),
            ];
            headers.extend(self.build_auth_header(auth)?);
            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Worldpayxml,
    curl_request: SoapXml(WorldpayxmlPSyncRequest),
    curl_response: WorldpayxmlTransactionResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let auth = worldpayxml::WorldpayxmlAuthType::try_from(&req.connector_config)?;
            let mut headers = vec![
                (headers::CONTENT_TYPE.to_string(), CONTENT_TYPE_XML.to_string().into()),
            ];
            headers.extend(self.build_auth_header(auth)?);
            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Worldpayxml,
    curl_request: SoapXml(WorldpayxmlRefundRequest),
    curl_response: WorldpayxmlRefundResponse,
    flow_name: Refund,
    resource_common_data: RefundFlowData,
    flow_request: RefundsData,
    flow_response: RefundsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let auth = worldpayxml::WorldpayxmlAuthType::try_from(&req.connector_config)?;
            let mut headers = vec![
                (headers::CONTENT_TYPE.to_string(), CONTENT_TYPE_XML.to_string().into()),
            ];
            headers.extend(self.build_auth_header(auth)?);
            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.connector_base_url_refunds(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Worldpayxml,
    curl_request: SoapXml(WorldpayxmlRSyncRequest),
    curl_response: WorldpayxmlRsyncResponse,
    flow_name: RSync,
    resource_common_data: RefundFlowData,
    flow_request: RefundSyncData,
    flow_response: RefundsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let auth = worldpayxml::WorldpayxmlAuthType::try_from(&req.connector_config)?;
            let mut headers = vec![
                (headers::CONTENT_TYPE.to_string(), CONTENT_TYPE_XML.to_string().into()),
            ];
            headers.extend(self.build_auth_header(auth)?);
            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.connector_base_url_refunds(req).to_string())
        }
    }
);

// Source verification implementations

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> Worldpayxml<T> {
    pub fn preprocess_response_bytes<F, FCD, Req, Res>(
        &self,
        _req: &RouterDataV2<F, FCD, Req, Res>,
        bytes: bytes::Bytes,
        _status_code: u16,
    ) -> CustomResult<bytes::Bytes, IntegrationError> {
        // WorldPay XML responses are kept as-is
        // The macros will handle XML deserialization using parse_xml()
        Ok(bytes)
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Worldpayxml<T>
{
    fn id(&self) -> &'static str {
        "worldpayxml"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        CONTENT_TYPE_XML
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.worldpayxml.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = worldpayxml::WorldpayxmlAuthType::try_from(auth_type)?;
        self.build_auth_header(auth)
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: responses::WorldpayxmlErrorResponse = res
            .response
            .parse_struct("WorldpayxmlErrorResponse")
            .change_context(
                utils::response_deserialization_fail(
                    res.status_code,
                "worldpayxml: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        match response {
            responses::WorldpayxmlErrorResponse::Standard(error_response) => {
                with_error_response_body!(event_builder, error_response);

                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: error_response
                        .code
                        .unwrap_or(common_utils::consts::NO_ERROR_CODE.to_string()),
                    message: error_response
                        .message
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
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
