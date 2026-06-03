pub mod transformers;

use std::fmt::Debug;

use base64::Engine;
use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt};
use domain_types::{
    connector_flow::{Authorize, Capture, CreateOrder, PSync, RSync, Refund, Void},
    connector_types::{
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
    },
    errors::{self, IntegrationError},
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
    self as juspay, JuspayAuthorizeRequest, JuspayAuthorizeResponse, JuspayCaptureRequest,
    JuspayCaptureResponse, JuspayCreateOrderRequest, JuspayCreateOrderResponse,
    JuspayOrderStatusResponse, JuspayRefundRequest, JuspayRefundResponse, JuspayRefundSyncResponse,
    JuspayVoidRequest, JuspayVoidResponse,
};

use crate::{types::ResponseRouterData, with_error_response_body};

pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

pub(crate) mod headers {
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const X_MERCHANT_ID: &str = "x-merchantid";
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const VERSION: &str = "version";
}

const JUSPAY_API_VERSION: &str = "2023-06-30";

use super::macros;

macros::create_amount_converter_wrapper!(connector_name: Juspay, amount_type: StringMajorUnit);

macros::create_all_prerequisites!(
    connector_name: Juspay,
    generic_type: T,
    api: [
        (
            flow: CreateOrder,
            request_body: JuspayCreateOrderRequest,
            response_body: JuspayCreateOrderResponse,
            router_data: RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>,
        ),
        (
            flow: Authorize,
            request_body: JuspayAuthorizeRequest,
            response_body: JuspayAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: JuspayOrderStatusResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: JuspayCaptureRequest,
            response_body: JuspayCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: JuspayRefundRequest,
            response_body: JuspayRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: JuspayRefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: Void,
            request_body: JuspayVoidRequest,
            response_body: JuspayVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
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
            let mut headers = vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.common_get_content_type().to_string().into(),
                ),
                (
                    headers::VERSION.to_string(),
                    JUSPAY_API_VERSION.to_string().into(),
                ),
            ];
            let mut auth_headers = self.get_auth_header(&req.connector_config)?;
            headers.append(&mut auth_headers);
            Ok(headers)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.juspay.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.juspay.base_url
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Juspay<T>
{
    fn id(&self) -> &'static str {
        "juspay"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/x-www-form-urlencoded"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.juspay.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = juspay::JuspayAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        let encoded_api_key = BASE64_ENGINE.encode(format!("{}:", auth.api_key.peek()));
        Ok(vec![
            (
                headers::AUTHORIZATION.to_string(),
                format!("Basic {encoded_api_key}").into_masked(),
            ),
            (
                headers::X_MERCHANT_ID.to_string(),
                auth.merchant_id.peek().to_string().into_masked(),
            ),
        ])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let response: juspay::JuspayErrorResponse = res
            .response
            .parse_struct("JuspayErrorResponse")
            .change_context(crate::utils::response_deserialization_fail(
                res.status_code,
                "juspay: response body did not match the expected error format; \
                 confirm API version and connector documentation.",
            ))?;

        with_error_response_body!(event_builder, response);

        let code = response
            .error_code
            .clone()
            .or_else(|| response.status.clone())
            .unwrap_or_else(|| res.status_code.to_string());

        let message = response
            .error_message
            .clone()
            .or_else(|| {
                response
                    .error_info
                    .as_ref()
                    .and_then(|info| info.user_message.clone())
            })
            .or_else(|| response.status.clone())
            .unwrap_or_else(|| format!("juspay: HTTP {}", res.status_code));

        let reason = response
            .error_info
            .as_ref()
            .and_then(|info| {
                info.user_message
                    .clone()
                    .or_else(|| info.developer_message.clone())
            })
            .or_else(|| response.error_message.clone());

        Ok(ErrorResponse {
            status_code: res.status_code,
            code,
            message,
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
    connector: Juspay,
    curl_request: FormUrlEncoded(JuspayCreateOrderRequest),
    curl_response: JuspayCreateOrderResponse,
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
    connector: Juspay,
    curl_request: FormUrlEncoded(JuspayAuthorizeRequest),
    curl_response: JuspayAuthorizeResponse,
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
            Ok(format!("{base_url}txns"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Juspay,
    curl_response: JuspayOrderStatusResponse,
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
            let order_id = req
                .resource_common_data
                .connector_order_id
                .clone()
                .unwrap_or_else(|| {
                    req.resource_common_data
                        .connector_request_reference_id
                        .clone()
                });
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}orders/{order_id}"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Juspay,
    curl_request: FormUrlEncoded(JuspayCaptureRequest),
    curl_response: JuspayCaptureResponse,
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
            let txn_uuid = req
                .request
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID {
                    context: Default::default(),
                })?;
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}v2/txns/{txn_uuid}/capture"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Juspay,
    curl_request: FormUrlEncoded(JuspayRefundRequest),
    curl_response: JuspayRefundResponse,
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
            let order_id = &req.request.connector_transaction_id;
            let base_url = self.connector_base_url_refunds(req);
            Ok(format!("{base_url}orders/{order_id}/refunds"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Juspay,
    curl_response: JuspayRefundSyncResponse,
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
            let order_id = &req.request.connector_transaction_id;
            let base_url = self.connector_base_url_refunds(req);
            Ok(format!("{base_url}orders/{order_id}"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Juspay,
    curl_request: FormUrlEncoded(JuspayVoidRequest),
    curl_response: JuspayVoidResponse,
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
            let txn_uuid = req.request.connector_transaction_id.as_str();
            if txn_uuid.is_empty() {
                return Err(error_stack::report!(
                    IntegrationError::MissingConnectorTransactionID {
                        context: Default::default(),
                    }
                ));
            }
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}v2/txns/{txn_uuid}/void"))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Juspay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Juspay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Juspay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Juspay<T>
{
    fn should_do_order_create(&self) -> bool {
        true
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Juspay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Juspay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Juspay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Juspay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Juspay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Juspay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Juspay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Juspay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Juspay<T>
{
}

crate::connectors::macros::macro_connector_payout_implementation!(
    connector: Juspay,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

crate::connectors::macros::macro_connector_flow_status_impls!(
    connector: Juspay,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        ClientAuthenticationToken,
        CreateConnectorCustomer,
        IncrementalAuthorization,
        MandateRevoke,
        PaymentMethodToken,
        RepeatPayment,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        SetupMandate
    ],
    not_supported: [
        Accept,
        DefendDispute,
        SubmitEvidence,
        Authenticate,
        PreAuthenticate,
        PostAuthenticate,
        VoidPC
    ],
);

static JUSPAY_SUPPORTED_PAYMENT_METHODS: std::sync::LazyLock<
    domain_types::types::SupportedPaymentMethods,
> = std::sync::LazyLock::new(|| {
    domain_types::build_supported_pms! {
        Supported => [
            // PaymentMethodData::Card explicit Ok arm at transformers.rs:421
            (Card, Card),
            // PaymentMethodData::Upi explicit Ok arms (UpiCollect / UpiIntent / UpiQr)
            (Upi, UpiCollect),
            (Upi, UpiIntent),
            (Upi, UpiQr),
            // PaymentMethodData::BankRedirect — only Netbanking has an Ok arm
            (BankRedirect, Netbanking),
            // PaymentMethodData::PayLater — only Atome has an Ok arm
            (PayLater, Atome),
            // PaymentMethodData::Wallet — explicit Ok arms per transformers.rs:637-657
            (Wallet, Paypal),
            (Wallet, AliPay),
            (Wallet, AliPayHk),
            (Wallet, ApplePay),
            (Wallet, GooglePay),
            (Wallet, AmazonPay),
            (Wallet, Momo),
            (Wallet, KakaoPay),
            (Wallet, WeChatPay),
            (Wallet, GoPay),
            (Wallet, Gcash),
            (Wallet, TouchNGo),
            (Wallet, SamsungPay),
            (Wallet, PhonePe),
            (Wallet, LazyPay),
        ],
        NotImplemented => [
            // PayLater — explicit Err(NotImplemented) arms at transformers.rs:757-765
            (PayLater, Klarna),
            (PayLater, Affirm),
            (PayLater, AfterpayClearpay),
            (PayLater, PayBright),
            (PayLater, Walley),
            (PayLater, Alma),
            // Wallet aggregator variants — explicit Err(NotImplemented) at 658-666
            (Wallet, BillDesk),
            (Wallet, Cashfree),
            (Wallet, PayU),
            (Wallet, EaseBuzz),
            // Wallet other variants — explicit Err(NotImplemented) at 667-682
            (Wallet, Bluecode),
            (Wallet, Dana),
            (Wallet, MbWay),
            (Wallet, MobilePay),
            (Wallet, Twint),
            (Wallet, Vipps),
            (Wallet, Cashapp),
            (Wallet, Swish),
            (Wallet, Mifinity),
            (Wallet, RevolutPay),
            (Wallet, Satispay),
            (Wallet, Wero),
            (Wallet, Paze),
        ],
    }
});

impl<
        T: domain_types::payment_method_data::PaymentMethodDataTypes
            + std::fmt::Debug
            + Sync
            + Send
            + 'static
            + serde::Serialize,
    > domain_types::connector_types::ConnectorSpecifications for Juspay<T>
{
    fn get_supported_payment_methods(
        &self,
    ) -> &'static domain_types::types::SupportedPaymentMethods {
        &JUSPAY_SUPPORTED_PAYMENT_METHODS
    }
}
