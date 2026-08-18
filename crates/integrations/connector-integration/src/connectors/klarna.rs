pub mod transformers;

use std::fmt::Debug;

use base64::Engine;
use common_enums::CurrencyUnit;
use common_utils::{
    consts::{BASE64_ENGINE, NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
};
use domain_types::{
    connector_flow::{Authorize, Capture, ClientAuthenticationToken, PSync, RSync, Refund, Void},
    connector_types::{
        ClientAuthenticationTokenRequestData, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
    },
    errors,
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::{PayLaterData, PaymentMethodData, PaymentMethodDataTypes},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Maskable};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding,
};
use serde::Serialize;
use transformers::{
    self as klarna, KlarnaAuthResponse, KlarnaCaptureRequest, KlarnaCaptureResponse,
    KlarnaPaymentsRequest, KlarnaPsyncResponse, KlarnaRSyncResponse, KlarnaRefundRequest,
    KlarnaRefundResponse, KlarnaSessionRequest, KlarnaSessionResponse, KlarnaVoidResponse,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body, with_response_body};

pub(crate) mod headers {
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

// Klarna amounts are integers in minor units (e.g. cents).
macros::create_amount_converter_wrapper!(connector_name: Klarna, amount_type: MinorUnit);

// Generates the `Klarna<T>` connector struct, its `new()` constructor, the
// `KlarnaRouterData<RD, T>` input wrapper, and the per-flow request/response
// bridges. Every flow implemented via `macro_connector_implementation!` must
// appear in this `api` array.
macros::create_all_prerequisites!(
    connector_name: Klarna,
    generic_type: T,
    api: [
        (
            flow: ClientAuthenticationToken,
            request_body: KlarnaSessionRequest,
            response_body: KlarnaSessionResponse,
            router_data: RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ),
        (
            flow: Authorize,
            request_body: KlarnaPaymentsRequest,
            response_body: KlarnaAuthResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: KlarnaPsyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: KlarnaCaptureRequest,
            response_body: KlarnaCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: KlarnaRefundRequest,
            response_body: KlarnaRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: KlarnaRSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        )
    ],
    amount_converters: [],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);
            Ok(header)
        }
    }
);

// Klarna uses a region-templated base URL. The stored base URL contains the
// literal `{{klarna_region}}` placeholder, resolved at request time from the
// merchant connector account metadata field `metadata.klarna_region`
// (Europe -> "", NorthAmerica -> "-na", Oceania -> "-oc"). This helper is a
// faithful port of Hyperswitch's `build_region_specific_endpoint`; flow
// implementations call it from `get_url` with `connector_feature_data`.
fn build_region_specific_endpoint(
    base_url: &str,
    connector_metadata: &Option<common_utils::pii::SecretSerdeValue>,
) -> CustomResult<String, errors::IntegrationError> {
    let klarna_metadata_object =
        klarna::KlarnaConnectorMetadataObject::try_from(connector_metadata)?;
    let klarna_region = klarna_metadata_object
        .klarna_region
        .ok_or(errors::IntegrationError::InvalidConnectorConfig {
            config: "merchant_connector_account.metadata.klarna_region",
            context: Default::default(),
        })
        .map(String::from)?;

    Ok(base_url.replace("{{klarna_region}}", &klarna_region))
}

// =============================================================================
// MAIN CONNECTOR INTEGRATION IMPLEMENTATIONS
// =============================================================================
// Primary authorize implementation - customize as needed
// =============================================================================
// MAIN CONNECTOR INTEGRATION IMPLEMENTATIONS
// =============================================================================
// Primary authorize implementation - customize as needed
// ... Authorize implementation is now valid generated code ...

// =============================================================================
// CONNECTOR COMMON IMPLEMENTATION
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Klarna<T>
{
    fn id(&self) -> &'static str {
        "klarna"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        // Region-templated URL (contains `{{klarna_region}}`); resolved per-flow
        // via `build_region_specific_endpoint`.
        connectors.klarna.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
        let auth = klarna::KlarnaAuthType::try_from(auth_type).change_context(
            errors::IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        // HTTP Basic auth: base64(key1:api_key) i.e. base64(username:password).
        let credentials = format!("{}:{}", auth.username.expose(), auth.password.expose());
        let encoded = BASE64_ENGINE.encode(credentials);
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            format!("Basic {encoded}").into(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let response: klarna::KlarnaErrorResponse = res
            .response
            .parse_struct("KlarnaErrorResponse")
            .change_context(errors::ConnectorError::ResponseDeserializationFailed {
                context: Default::default(),
            })?;

        with_error_response_body!(event_builder, response);

        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");

        // Klarna has two error shapes (see `KlarnaErrorResponse`): the API
        // validation error carries `error_code` + `error_messages`/`error_message`;
        // the auth/gateway error (e.g. 401) carries `http_status_code` +
        // `http_status_message` + `internal_message` and no `error_code`. Fall back
        // across both so a real 401 surfaces instead of a deserialization failure.
        let code = response
            .error_code
            .clone()
            .or_else(|| response.http_status_code.map(|code| code.to_string()))
            .unwrap_or_else(|| NO_ERROR_CODE.to_string());

        let reason = response
            .error_messages
            .map(|messages| messages.join(" & "))
            .or_else(|| response.error_message.clone())
            .or_else(|| response.internal_message.clone())
            .or_else(|| response.http_status_message.clone());

        let message = response
            .error_message
            .clone()
            .or_else(|| response.http_status_message.clone())
            .or_else(|| response.internal_message.clone())
            .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string());

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
            typed_connector_response: typed,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        })
    }
}

// =============================================================================
// BODY DECODING IMPLEMENTATION
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Klarna<T>
{
}

// =============================================================================
// SESSION (CreateSessionToken pre-auth) FLOW
// =============================================================================
// SDK / embedded Klarna variant: a pre-authorization session is created via
// `POST {base}payments/v1/sessions`. The `client_token` in the response is
// handed to the client-side Klarna SDK. Faithful port of Hyperswitch's
// `ConnectorIntegration<Session, PaymentsSessionData, PaymentsResponseData>`;
// in UCS this maps to the `ClientAuthenticationToken` flow.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Klarna<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Klarna,
    curl_request: Json(KlarnaSessionRequest),
    curl_response: KlarnaSessionResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let endpoint = build_region_specific_endpoint(
                &req.resource_common_data.connectors.klarna.base_url,
                &req.resource_common_data.connector_feature_data,
            )?;
            Ok(format!("{endpoint}payments/v1/sessions"))
        }
    }
);

// =============================================================================
// AUTHORIZE FLOW
// =============================================================================
// Two variants, branched by the payment method data:
//   * `PayLaterData::KlarnaSdk { token }` -> Klarna Payments API
//     `POST {base}payments/v1/authorizations/{token}/order`
//   * `PayLaterData::KlarnaRedirect {}`   -> Klarna Checkout v3
//     `POST {base}checkout/v3/orders`
// The request/response body selection (and fraud/checkout status mapping) lives
// in the transformers. Faithful port of Hyperswitch's
// `ConnectorIntegration<Authorize, PaymentsAuthorizeData, PaymentsResponseData>`.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Klarna<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Klarna,
    curl_request: Json(KlarnaPaymentsRequest),
    curl_response: KlarnaAuthResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let endpoint = build_region_specific_endpoint(
                &req.resource_common_data.connectors.klarna.base_url,
                &req.resource_common_data.connector_feature_data,
            )?;
            match &req.request.payment_method_data {
                PaymentMethodData::PayLater(PayLaterData::KlarnaSdk { token }) => {
                    Ok(format!("{endpoint}payments/v1/authorizations/{token}/order"))
                }
                PaymentMethodData::PayLater(PayLaterData::KlarnaRedirect {}) => {
                    Ok(format!("{endpoint}checkout/v3/orders"))
                }
                _ => Err(errors::IntegrationError::NotImplemented(
                    "Payment method".to_string(),
                    Default::default(),
                )
                .into()),
            }
        }
    }
);

// =============================================================================
// PSYNC FLOW
// =============================================================================
// Two variants, branched by `payment_experience` (the same signal Hyperswitch
// uses in its PSync `get_url`):
//   * `InvokeSdkClient` (SDK / Klarna Payments) -> Order Management API
//     `GET {base}ordermanagement/v1/orders/{order_id}`. Response status is a
//     `KlarnaOrderStatus` (AUTHORIZED / PART_CAPTURED / CAPTURED / CANCELLED /
//     EXPIRED / CLOSED).
//   * `RedirectToUrl` (Klarna Checkout v3)       ->
//     `GET {base}checkout/v3/orders/{order_id}`. Response status is a
//     `KlarnaCheckoutStatus` (+ `options.auto_capture`), reusing the mapping
//     added for Authorize.
// `order_id` is the connector_transaction_id from the request. GET, no body.
// Faithful port of Hyperswitch's
// `ConnectorIntegration<PSync, PaymentsSyncData, PaymentsResponseData>`.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Klarna<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Klarna,
    curl_response: KlarnaPsyncResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let order_id = req.request.get_connector_transaction_id()?;
            let endpoint = build_region_specific_endpoint(
                &req.resource_common_data.connectors.klarna.base_url,
                &req.resource_common_data.connector_feature_data,
            )?;
            match req.request.payment_experience {
                Some(common_enums::PaymentExperience::InvokeSdkClient) => {
                    Ok(format!("{endpoint}ordermanagement/v1/orders/{order_id}"))
                }
                Some(common_enums::PaymentExperience::RedirectToUrl) => {
                    Ok(format!("{endpoint}checkout/v3/orders/{order_id}"))
                }
                _ => Err(errors::IntegrationError::NotImplemented(
                    "payment_experience".to_string(),
                    Default::default(),
                )
                .into()),
            }
        }
    }
);

// =============================================================================
// CAPTURE FLOW
// =============================================================================
// POST {base}ordermanagement/v1/orders/{order_id}/captures  (order_id =
// connector_transaction_id). CRITICAL: Klarna returns the capture id in the
// `Capture-Id` *response header* (a 201 with an often-empty body), so this flow
// cannot use `macro_connector_implementation!` — the macro response pipeline
// never exposes response headers. Instead we hand-roll `ConnectorIntegrationV2`
// (mirroring the `fiuu` PSync idiom) and read the header via
// `domain_types::utils::get_http_header`. Faithful port of Hyperswitch's
// `ConnectorIntegration<Capture, PaymentsCaptureData, PaymentsResponseData>`.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Klarna<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
    for Klarna<T>
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
        self.build_headers(req)
    }

    fn get_content_type(&self) -> &'static str {
        self.common_get_content_type()
    }

    fn get_url(
        &self,
        req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
    ) -> CustomResult<String, errors::IntegrationError> {
        let order_id = req.request.get_connector_transaction_id()?;
        let endpoint = build_region_specific_endpoint(
            &req.resource_common_data.connectors.klarna.base_url,
            &req.resource_common_data.connector_feature_data,
        )?;
        Ok(format!(
            "{endpoint}ordermanagement/v1/orders/{order_id}/captures"
        ))
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
    ) -> CustomResult<Option<common_utils::request::ConnectorRequestData>, errors::IntegrationError> {
        let bridge = self.capture;
        let input_data = KlarnaRouterData {
            connector: self.to_owned(),
            router_data: req.clone(),
        };
        let request = bridge.request_body(input_data)?;
        let typed = events::MaskedSerdeValue::from_masked_optional(
            &request,
            "typed_connector_request",
        );
        Ok(Some(common_utils::request::ConnectorRequestData::new(
            common_utils::request::RequestContent::Json(Box::new(request)),
            typed,
        )))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        errors::ConnectorError,
    > {
        // Klarna returns the capture id in the `Capture-Id` response header, not
        // the body (typically empty on a 201). Absent headers ->
        // ResponseDeserializationFailed, matching Hyperswitch.
        let headers = res.headers.ok_or_else(|| {
            error_stack::report!(errors::ConnectorError::ResponseDeserializationFailed {
                context: Default::default(),
            })
            .attach_printable("Expected headers, but received no headers in response")
        })?;
        let capture_id = domain_types::utils::get_http_header("Capture-Id", &headers)
            .attach_printable("Missing capture id in headers")
            .change_context(errors::ConnectorError::ResponseHandlingFailed {
                context: Default::default(),
            })?;
        let response = KlarnaCaptureResponse {
            capture_id: Some(capture_id.to_owned()),
        };
        with_response_body!(event_builder, response);

        RouterDataV2::try_from(ResponseRouterData {
            response,
            router_data: data.clone(),
            http_code: res.status_code,
        })
        .change_context(errors::ConnectorError::ResponseHandlingFailed {
            context: Default::default(),
        })
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        self.build_error_response(res, event_builder, connector_config)
    }
}

// =============================================================================
// REFUND (Execute) FLOW
// =============================================================================
// POST {base}ordermanagement/v1/orders/{order_id}/refunds  (order_id =
// connector_transaction_id being refunded). CRITICAL: Klarna returns the refund
// id in the `Refund-Id` *response header* (a 201 with an often-empty body), not
// the body -- exactly like Capture's `Capture-Id`. This flow therefore cannot use
// `macro_connector_implementation!` (the macro response pipeline never exposes
// response headers); we hand-roll `ConnectorIntegrationV2` and read the header
// via `domain_types::utils::get_http_header`. Faithful port of Hyperswitch's
// `ConnectorIntegration<Execute, RefundsData, RefundsResponseData>`.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Klarna<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Refund, RefundFlowData, RefundsData, RefundsResponseData> for Klarna<T>
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
        self.build_headers(req)
    }

    fn get_content_type(&self) -> &'static str {
        self.common_get_content_type()
    }

    fn get_url(
        &self,
        req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
    ) -> CustomResult<String, errors::IntegrationError> {
        let order_id = &req.request.connector_transaction_id;
        let endpoint = build_region_specific_endpoint(
            &req.resource_common_data.connectors.klarna.base_url,
            &req.resource_common_data.connector_feature_data,
        )?;
        Ok(format!(
            "{endpoint}ordermanagement/v1/orders/{order_id}/refunds"
        ))
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
    ) -> CustomResult<Option<common_utils::request::ConnectorRequestData>, errors::IntegrationError> {
        let bridge = self.refund;
        let input_data = KlarnaRouterData {
            connector: self.to_owned(),
            router_data: req.clone(),
        };
        let request = bridge.request_body(input_data)?;
        let typed = events::MaskedSerdeValue::from_masked_optional(
            &request,
            "typed_connector_request",
        );
        Ok(Some(common_utils::request::ConnectorRequestData::new(
            common_utils::request::RequestContent::Json(Box::new(request)),
            typed,
        )))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        errors::ConnectorError,
    > {
        // Klarna returns the refund id in the `Refund-Id` response header, not the
        // body (typically empty on a 201). Absent headers ->
        // ResponseDeserializationFailed, matching Hyperswitch.
        let headers = res.headers.ok_or_else(|| {
            error_stack::report!(errors::ConnectorError::ResponseDeserializationFailed {
                context: Default::default(),
            })
            .attach_printable("Expected headers, but received no headers in response")
        })?;
        let refund_id = domain_types::utils::get_http_header("Refund-Id", &headers)
            .attach_printable("Missing refund id in headers")
            .change_context(errors::ConnectorError::ResponseHandlingFailed {
                context: Default::default(),
            })?;
        let response = KlarnaRefundResponse {
            refund_id: refund_id.to_owned(),
        };
        with_response_body!(event_builder, response);

        RouterDataV2::try_from(ResponseRouterData {
            response,
            router_data: data.clone(),
            http_code: res.status_code,
        })
        .change_context(errors::ConnectorError::ResponseHandlingFailed {
            context: Default::default(),
        })
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        self.build_error_response(res, event_builder, connector_config)
    }
}

// =============================================================================
// RSYNC (refund status sync) FLOW
// =============================================================================
// GET {base}ordermanagement/v1/orders/{order_id}/refunds/{refund_id}
//   * order_id  = connector_transaction_id (the original order being refunded)
//   * refund_id = connector_refund_id (the refund to sync), from RefundSyncData
// GET, no body. Unlike Refund (whose id rides a response header), RSync is a
// normal body-driven response, so it uses `macro_connector_implementation!` like
// PSync. Status: 200 -> RefundStatus::Success; anything else is routed through
// the error handler. Faithful port of Hyperswitch's
// `ConnectorIntegration<RSync, RefundsData, RefundsResponseData>`.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Klarna<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Klarna,
    curl_response: KlarnaRSyncResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let order_id = &req.request.connector_transaction_id;
            let refund_id = &req.request.connector_refund_id;
            let endpoint = build_region_specific_endpoint(
                &req.resource_common_data.connectors.klarna.base_url,
                &req.resource_common_data.connector_feature_data,
            )?;
            Ok(format!(
                "{endpoint}ordermanagement/v1/orders/{order_id}/refunds/{refund_id}"
            ))
        }
    }
);

// =============================================================================
// VOID (Cancel) FLOW
// =============================================================================
// POST {base}ordermanagement/v1/orders/{order_id}/cancel  (order_id =
// connector_transaction_id). Empty POST — no request body. CRITICAL: Klarna
// returns a 204 No Content with an empty body on success, so there is nothing
// for the macro response pipeline to deserialize (an empty body cannot satisfy
// a `curl_response` type). We therefore hand-roll `ConnectorIntegrationV2`
// (mirroring Capture / Refund): `get_request_body` yields no body, and
// `handle_response_v2` synthesizes a `KlarnaVoidResponse` placeholder whose
// `TryFrom` resolves the attempt status purely from the HTTP status code
// (204 -> Voided, else VoidFailed). Faithful port of Hyperswitch's
// `ConnectorIntegration<Void, PaymentsCancelData, PaymentsResponseData>`, whose
// `handle_response` likewise skips response parsing and maps status from the
// status code.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Klarna<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
    for Klarna<T>
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
        self.build_headers(req)
    }

    fn get_content_type(&self) -> &'static str {
        self.common_get_content_type()
    }

    fn get_url(
        &self,
        req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
    ) -> CustomResult<String, errors::IntegrationError> {
        let order_id = &req.request.connector_transaction_id;
        let endpoint = build_region_specific_endpoint(
            &req.resource_common_data.connectors.klarna.base_url,
            &req.resource_common_data.connector_feature_data,
        )?;
        Ok(format!(
            "{endpoint}ordermanagement/v1/orders/{order_id}/cancel"
        ))
    }

    fn get_request_body(
        &self,
        _req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
    ) -> CustomResult<Option<common_utils::request::ConnectorRequestData>, errors::IntegrationError> {
        // Klarna's cancel endpoint takes an empty POST; there is no request body.
        Ok(None)
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        errors::ConnectorError,
    > {
        // Klarna returns a 204 No Content with an empty body on success, so there
        // is nothing to parse. The attempt status is resolved from the HTTP
        // status code inside the `TryFrom` below (204 -> Voided, else
        // VoidFailed), mirroring Hyperswitch's Void `handle_response`.
        let response = KlarnaVoidResponse;
        with_response_body!(event_builder, response);

        RouterDataV2::try_from(ResponseRouterData {
            response,
            router_data: data.clone(),
            http_code: res.status_code,
        })
        .change_context(errors::ConnectorError::ResponseHandlingFailed {
            context: Default::default(),
        })
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        self.build_error_response(res, event_builder, connector_config)
    }
}

// =============================================================================
// DYNAMICALLY GENERATED IMPLEMENTATIONS
// =============================================================================
// Auto-generated by add_connector.sh using the macro-based pattern. All flow
// traits are stubbed via `macros::macro_connector_flow_status_impls!` with
// `not_implemented` status, which emits both the marker-trait impl and a stub
// `ConnectorIntegrationV2` impl per flow.
//
// To implement a flow:
//   1. Remove that flow's name from the `not_implemented` list below.
//   2. Add a manual marker-trait impl, e.g.
//        impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
//            connector_types::PaymentAuthorizeV2<T> for Klarna<T> {}
//   3. Add a `macros::macro_connector_implementation!(...)` block with the
//      flow's request/response types, HTTP method, and `get_url`/`get_headers`.
//
// See crates/integrations/connector-integration/src/connectors/xendit.rs for a
// reference implementation that follows this pattern.
// =============================================================================

// ===== CONNECTOR SERVICE TRAIT IMPLEMENTATION =====
// Aggregate trait - composes all other connector traits.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Klarna<T>
{
}

// ===== BASE (NON-FLOW) TRAIT IMPLEMENTATIONS =====
// These are simple marker traits that are NOT flows and therefore have no arm
// in expand_flow_status_impl!. They must be impl'd manually.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Klarna<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Klarna<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Klarna<T>
{
}

// ===== SOURCE VERIFICATION IMPLEMENTATION =====
// Non-generic marker trait required by VerifyRedirectResponse for webhook
// signature verification.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::verification::SourceVerification for Klarna<T>
{
}

// ===== PAYOUT TRAIT IMPLEMENTATIONS =====
// Emits payout marker-trait impls and default no-op ConnectorIntegrationV2
// impls for all PayoutXxxV2 flows.
crate::connectors::macros::macro_connector_payout_implementation!(
    connector: Klarna,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

// ===== FLOW STATUS IMPLEMENTATIONS =====
// Emits marker-trait impls AND stub ConnectorIntegrationV2 impls for every
// flow listed. Each stub's get_url returns
// IntegrationError::connector_flow_not_implemented(...).
crate::connectors::macros::macro_connector_flow_status_impls!(
    connector: Klarna,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        Accept,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        DefendDispute,
        MandateRevoke,
        Authenticate,
        IncrementalAuthorization,
        CreateOrder,
        PostAuthenticate,
        PreAuthenticate,
        PaymentMethodToken,
        VoidPC,
        VoidPostRefund,
        RepeatPayment,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        SetupMandate,
        SubmitEvidence
    ],
);
