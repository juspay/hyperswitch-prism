pub mod transformers;

use base64::Engine;
use common_enums::{self as enums, CurrencyUnit};
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::CustomResult,
    events,
    ext_traits::{ByteSliceExt, ValueExt},
    FloatMajorUnit,
};
use domain_types::{
    connector_flow::{
        Accept, Authenticate, Authorize, Capture, CreateAccessToken, CreateConnectorCustomer,
        CreateOrder, CreateSessionToken, DefendDispute, IncrementalAuthorization, MandateRevoke,
        PSync, PaymentMethodToken, PostAuthenticate, PreAuthenticate, RSync, Refund, RepeatPayment,
        SdkSessionToken, SetupMandate, SubmitEvidence, Void, VoidPC,
    },
    connector_types::{
        AcceptDisputeData, AccessTokenRequestData, AccessTokenResponseData, ConnectorCustomerData,
        ConnectorCustomerResponse, ConnectorSpecifications, DisputeDefendData, DisputeFlowData,
        DisputeResponseData, MandateRevokeRequestData, MandateRevokeResponseData,
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData,
        PaymentMethodTokenResponse, PaymentMethodTokenizationData, PaymentVoidData,
        PaymentsAuthenticateData, PaymentsAuthorizeData, PaymentsCancelPostCaptureData,
        PaymentsCaptureData, PaymentsIncrementalAuthorizationData, PaymentsPostAuthenticateData,
        PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSdkSessionTokenData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        RepeatPaymentData, SessionTokenRequestData, SessionTokenResponseData,
        SetupMandateRequestData, SubmitEvidenceData, SupportedPaymentMethodsExt,
    },
    errors::{self},
    payment_method_data::PaymentMethodDataTypes,
    router_data::ErrorResponse,
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::{
        CardSpecificFeatures, ConnectorInfo, Connectors, FeatureStatus, PaymentConnectorCategory,
        PaymentMethodDetails, PaymentMethodSpecificFeatures, SupportedPaymentMethods,
    },
};
use error_stack::ResultExt;
use hyperswitch_masking::{Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding,
};
use serde::Serialize;
use std::fmt::Debug;
use std::sync::LazyLock;
use transformers::{
    self as hyperpg, HyperpgAuthorizeRequest, HyperpgAuthorizeResponse, HyperpgRefundRequest,
    HyperpgRefundResponse, HyperpgRefundSyncResponse, HyperpgSyncResponse,
};

use super::macros;
use crate::types::ResponseRouterData;
use crate::with_error_response_body;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const X_MERCHANT_ID: &str = "x-merchantid";
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSessionToken for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAccessToken for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentIncrementalAuthorization for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SdkSessionTokenV2 for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::MandateRevokeV2 for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::AcceptDispute for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::DisputeDefend for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SubmitEvidenceV2 for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Hyperpg<T>
{
}

macros::create_all_prerequisites!(
    connector_name: Hyperpg,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: HyperpgAuthorizeRequest,
            response_body: HyperpgAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: HyperpgSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: HyperpgRefundRequest,
            response_body: HyperpgRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: HyperpgRefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: FloatMajorUnit
    ],
    member_functions: {
        pub fn build_headers(
            &self,
            auth: &hyperpg::HyperpgAuthType,
        ) -> Vec<(String, Maskable<String>)> {
            // Generate Basic Auth: Base64(api_key:)
            let auth_value = format!("{}:", auth.username.peek());
            let encoded_auth = base64::engine::general_purpose::STANDARD.encode(auth_value.as_bytes());

            vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    "application/json".to_string().into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    format!("Basic {encoded_auth}").into_masked(),
                ),
                (
                    headers::X_MERCHANT_ID.to_string(),
                    auth.merchant_id.peek().to_string().into(),
                ),
            ]
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.hyperpg.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.hyperpg.base_url
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Hyperpg<T>
{
    fn id(&self) -> &'static str {
        "hyperpg"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/x-www-form-urlencoded"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.hyperpg.base_url
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let response: hyperpg::HyperpgErrorResponse = res
            .response
            .parse_struct("HyperpgErrorResponse")
            .change_context(errors::ConnectorError::ResponseDeserializationFailed)
            .attach_printable("Failed to deserialize Hyperpg error response")?;

        with_error_response_body!(event_builder, response);

        let message = response
            .error_info
            .as_ref()
            .and_then(|info| info.developer_message.clone());

        let fields = response
            .error_info
            .as_ref()
            .and_then(|info| info.fields.as_ref())
            .and_then(|error| error.first());

        // Build reason without moving strings out of the struct
        let reason = fields.map(|fields| {
            format!(
                "{}: {}",
                fields.field_name.as_deref().unwrap_or_default(),
                fields.reason.as_deref().unwrap_or_default()
            )
        });

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.error_code.unwrap_or(NO_ERROR_CODE.to_string()),
            message: message.unwrap_or(NO_ERROR_MESSAGE.to_string()),
            reason,
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
    connector: Hyperpg,
    curl_request: Json(HyperpgAuthorizeRequest),
    curl_response: HyperpgAuthorizeResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            let auth = hyperpg::HyperpgAuthType::try_from(&req.connector_config)
                .change_context(errors::ConnectorError::FailedToObtainAuthType)?;
            Ok(self.build_headers(&auth))
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            Ok(format!("{}/txns", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Hyperpg,
    curl_response: HyperpgSyncResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            let auth = hyperpg::HyperpgAuthType::try_from(&req.connector_config)
                .change_context(errors::ConnectorError::FailedToObtainAuthType)?;
            Ok(self.build_headers(&auth))
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            let order_id = req.resource_common_data.get_reference_id()?;
            Ok(format!("{}/orders/{}", self.connector_base_url_payments(req), order_id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Hyperpg,
    curl_request: Json(HyperpgRefundRequest),
    curl_response: HyperpgRefundResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            let auth = hyperpg::HyperpgAuthType::try_from(&req.connector_config)
                .change_context(errors::ConnectorError::FailedToObtainAuthType)?;
            Ok(self.build_headers(&auth))
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            let connector_meta = req.request.get_connector_feature_data()?;


    let hyperpg_meta: hyperpg::HyperpgMeta = connector_meta
        .parse_value("HyperpgMeta")
        .change_context(errors::ConnectorError::ParsingFailed)?;


    let order_id = hyperpg_meta.order_id.ok_or(
        errors::ConnectorError::MissingRequiredField { field_name: "order_id"},
    )?;


            Ok(format!("{}/orders/{}/refunds", self.connector_base_url_refunds(req), order_id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Hyperpg,
    curl_response: HyperpgRefundSyncResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            let auth = hyperpg::HyperpgAuthType::try_from(&req.connector_config)
                .change_context(errors::ConnectorError::FailedToObtainAuthType)?;
            Ok(self.build_headers(&auth))
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            let order_id = req.request.connector_refund_id.clone();
            Ok(format!("{}/orders/{}", self.connector_base_url_refunds(req), order_id))
        }
    }
);

static HYPERPG_SUPPORTED_PAYMENT_METHODS: LazyLock<SupportedPaymentMethods> = {
    LazyLock::new(|| {
        let supported_capture_methods = vec![enums::CaptureMethod::Automatic];

        let supported_card_network = vec![
            common_enums::CardNetwork::Mastercard,
            common_enums::CardNetwork::Visa,
            common_enums::CardNetwork::Interac,
            common_enums::CardNetwork::AmericanExpress,
            common_enums::CardNetwork::JCB,
            common_enums::CardNetwork::DinersClub,
            common_enums::CardNetwork::Discover,
            common_enums::CardNetwork::CartesBancaires,
            common_enums::CardNetwork::UnionPay,
            common_enums::CardNetwork::Maestro,
        ];

        let mut hyperpg_supported_payment_methods = SupportedPaymentMethods::new();
        hyperpg_supported_payment_methods.add(
            enums::PaymentMethod::Card,
            enums::PaymentMethodType::Card,
            PaymentMethodDetails {
                mandates: FeatureStatus::Supported,
                refunds: FeatureStatus::Supported,
                supported_capture_methods: supported_capture_methods.clone(),
                specific_features: Some(PaymentMethodSpecificFeatures::Card({
                    CardSpecificFeatures {
                        three_ds: FeatureStatus::Supported,
                        no_three_ds: FeatureStatus::Supported,
                        supported_card_networks: supported_card_network.clone(),
                    }
                })),
            },
        );

        hyperpg_supported_payment_methods
    })
};

static HYPERPG_CONNECTOR_INFO: ConnectorInfo = ConnectorInfo {
        display_name: "Hyperpg",
        description: "Hyperpg is your trusted payment gateway solution. Seamlessly manage transactions, enhance security, and empower your business to thrive in the digital age.",
        connector_type: PaymentConnectorCategory::PaymentGateway,
    };

static HYPERPG_SUPPORTED_WEBHOOK_FLOWS: Vec<enums::EventClass> = Vec::new();

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorSpecifications
    for Hyperpg<T>
{
    fn get_connector_about(&self) -> Option<&'static ConnectorInfo> {
        Some(&HYPERPG_CONNECTOR_INFO)
    }

    fn get_supported_payment_methods(&self) -> Option<&'static SupportedPaymentMethods> {
        Some(&HYPERPG_SUPPORTED_PAYMENT_METHODS)
    }

    fn get_supported_webhook_flows(&self) -> Option<&'static [enums::EventClass]> {
        Some(&HYPERPG_SUPPORTED_WEBHOOK_FLOWS)
    }
}

// Empty implementations for other traits
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    > for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    > for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        RepeatPayment,
        PaymentFlowData,
        RepeatPaymentData<T>,
        PaymentsResponseData,
    > for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        MandateRevokeResponseData,
    > for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateSessionToken,
        PaymentFlowData,
        SessionTokenRequestData,
        SessionTokenResponseData,
    > for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>
    for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>
    for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>
    for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    > for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    > for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    > for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    > for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        SdkSessionToken,
        PaymentFlowData,
        PaymentsSdkSessionTokenData,
        PaymentsResponseData,
    > for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateAccessToken,
        PaymentFlowData,
        AccessTokenRequestData,
        AccessTokenResponseData,
    > for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    > for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
    for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
    for Hyperpg<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    > for Hyperpg<T>
{
}

// Verification traits
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::verification::SourceVerification for Hyperpg<T>
{
}
