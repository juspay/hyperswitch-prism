pub mod signing;
pub mod transformers;

use common_enums::CurrencyUnit;
use common_utils::{
    consts::NO_ERROR_CODE, errors::CustomResult, events, ext_traits::ByteSliceExt,
    request::RequestContent,
};
use domain_types::{
    connector_flow::{
        PayoutCreate, PayoutCreateLink, PayoutCreateRecipient, PayoutEligibility,
        PayoutEnrollDisburseAccount, PayoutGet, PayoutStage, PayoutTransfer, PayoutVoid,
    },
    errors::{ConnectorError, IntegrationError},
    payouts::payouts_types::{
        PayoutCreateLinkRequest, PayoutCreateLinkResponse, PayoutCreateRecipientRequest,
        PayoutCreateRecipientResponse, PayoutCreateRequest, PayoutCreateResponse,
        PayoutEligibilityRequest, PayoutEligibilityResponse, PayoutEnrollDisburseAccountRequest,
        PayoutEnrollDisburseAccountResponse, PayoutFlowData, PayoutGetRequest, PayoutGetResponse,
        PayoutStageRequest, PayoutStageResponse, PayoutTransferRequest, PayoutTransferResponse,
        PayoutVoidRequest, PayoutVoidResponse,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Mask, Maskable, Secret};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{
        PayoutCreateLinkV2, PayoutCreateRecipientV2, PayoutCreateV2, PayoutEligibilityV2,
        PayoutEnrollDisburseAccountV2, PayoutGetV2, PayoutServiceTrait, PayoutStageV2,
        PayoutTransferV2, PayoutVoidV2,
    },
};

use crate::types::ResponseRouterData;
use crate::with_error_response_body;
use signing::{build_cseal_headers, CsealHeaders};
use transformers::{
    build_eligibility_response, derive_vop_id, encode_connector_payout_id, DeutschebankAuthType,
    DeutschebankErrorResponse, DeutschebankSepaPaymentBuilt, DeutschebankSepaPaymentResponse,
    DeutschebankStatusRequest, DeutschebankStatusResponse, DeutschebankVopRequest,
    DeutschebankVopResponse,
};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const APIKEY: &str = "apikey";
    pub(crate) const DATE: &str = "Date";
    pub(crate) const DIGEST: &str = "Digest";
    pub(crate) const SIGNATURE: &str = "Signature";
    pub(crate) const X_CORRELATION_IDENTIFIER: &str = "x-correlation-identifier";
    pub(crate) const X_APICONSUMER_REQUEST_TIMESTAMP: &str = "x-apiConsumer-request-timestamp";
    pub(crate) const X_CUSTOMER_IDENTIFIER: &str = "x-customer-identifier";
    pub(crate) const I_APICONSUMER_IDENTIFIER: &str = "i-apiconsumer-identifier";
    pub(crate) const X_VERIFICATIONOFPAYEE_IDENTIFIER: &str = "x-verificationofpayee-identifier";
}

const VOP_PATH: &str = "/v1/cseal/payments/sepa/vop-check/vop";
const PAYMENT_PATH: &str = "/v2/cseal/payments/credit-transfer/sepa/payment";
const STATUS_PATH: &str = "/v2/cseal/payments/credit-transfer/sepa/status";

const CORRELATION_PREFIX_VOP: &str = "ACID";
const CORRELATION_PREFIX_PAYMENT: &str = "PYMT";

pub struct DeutschebankPayouts;

impl DeutschebankPayouts {
    pub const fn new() -> &'static Self {
        &Self
    }

    fn build_identity_headers(
        &self,
        auth: &DeutschebankAuthType,
        correlation_prefix: &str,
    ) -> Vec<(String, Maskable<String>)> {
        vec![
            (
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            ),
            (
                headers::APIKEY.to_string(),
                auth.apikey.clone().expose().into_masked(),
            ),
            (
                headers::X_CUSTOMER_IDENTIFIER.to_string(),
                auth.customer_identifier.clone().expose().into_masked(),
            ),
            (
                headers::I_APICONSUMER_IDENTIFIER.to_string(),
                auth.consumer_identifier.clone().expose().into_masked(),
            ),
            (
                headers::X_CORRELATION_IDENTIFIER.to_string(),
                format!(
                    "{correlation_prefix}{}",
                    uuid::Uuid::new_v4().simple().to_string().to_uppercase()
                )
                .into(),
            ),
            (
                headers::X_APICONSUMER_REQUEST_TIMESTAMP.to_string(),
                current_iso_utc_seconds().into(),
            ),
        ]
    }

    fn append_cseal_headers(
        &self,
        headers_out: &mut Vec<(String, Maskable<String>)>,
        method: common_utils::request::Method,
        path: &str,
        body: &[u8],
        auth: &DeutschebankAuthType,
    ) -> CustomResult<(), IntegrationError> {
        let CsealHeaders {
            date,
            digest,
            signature,
        } = build_cseal_headers(method, path, body, &auth.key_id, &auth.signing_private_key)?;
        headers_out.push((headers::DATE.to_string(), date.into()));
        if let Some(digest_value) = digest {
            headers_out.push((headers::DIGEST.to_string(), digest_value.into()));
        }
        headers_out.push((headers::SIGNATURE.to_string(), signature.into_masked()));
        Ok(())
    }
}

fn current_iso_utc_seconds() -> String {
    use time::macros::format_description;
    let fmt = format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]Z");
    time::OffsetDateTime::now_utc()
        .format(&fmt)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

fn serialize_json<T: serde::Serialize>(value: &T) -> CustomResult<Vec<u8>, IntegrationError> {
    serde_json::to_vec(value).change_context(IntegrationError::RequestEncodingFailed {
        context: Default::default(),
    })
}

// ===== CONNECTOR COMMON =====

impl ConnectorCommon for DeutschebankPayouts {
    fn id(&self) -> &'static str {
        "deutschebank"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.deutschebank.base_url
    }

    fn get_auth_header(
        &self,
        _auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        Ok(vec![])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: DeutschebankErrorResponse = res
            .response
            .parse_struct("DeutschebankErrorResponse")
            .unwrap_or_else(|err| {
                tracing::warn!(
                    deserialization_error = ?err,
                    "Failed to parse Deutsche Bank error response",
                );
                DeutschebankErrorResponse::default()
            });

        with_error_response_body!(event_builder, response);

        let first_error = response.errors.as_ref().and_then(|errs| errs.first());

        let code = response
            .code
            .or(response.error_code)
            .or_else(|| first_error.and_then(|e| e.code.clone()))
            .unwrap_or_else(|| NO_ERROR_CODE.to_string());
        let message = response
            .message
            .clone()
            .or(response.error_message.clone())
            .or_else(|| first_error.and_then(|e| e.message.clone()))
            .unwrap_or_else(|| {
                if code == NO_ERROR_CODE {
                    format!("Deutsche Bank request failed (HTTP {})", res.status_code)
                } else {
                    format!(
                        "Deutsche Bank request failed: {code} (HTTP {})",
                        res.status_code
                    )
                }
            });
        let reason = response
            .reason
            .clone()
            .or_else(|| first_error.and_then(|e| e.reason.clone()));

        Ok(ErrorResponse {
            status_code: res.status_code,
            code,
            message: message.clone(),
            reason: reason.or(Some(message)),
            attempt_status: None,
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}

// ===== PAYOUT SERVICE TRAIT =====

impl PayoutServiceTrait for DeutschebankPayouts {}

fn cert_pem(auth: &DeutschebankAuthType) -> CustomResult<Option<Secret<String>>, IntegrationError> {
    Ok(Some(Secret::new(b64_pem(
        auth.client_certificate.clone().expose(),
    ))))
}

fn cert_key_pem(
    auth: &DeutschebankAuthType,
) -> CustomResult<Option<Secret<String>>, IntegrationError> {
    Ok(Some(Secret::new(b64_pem(
        auth.client_certificate_key.clone().expose(),
    ))))
}

fn b64_pem(mut pem: String) -> String {
    use base64::Engine as _;
    if !pem.ends_with('\n') {
        pem.push('\n');
    }
    common_utils::consts::BASE64_ENGINE.encode(pem)
}

fn server_ca_pem(
    auth: &DeutschebankAuthType,
) -> CustomResult<Option<Secret<String>>, IntegrationError> {
    Ok(auth
        .server_ca_bundle
        .clone()
        .map(|pem| Secret::new(b64_pem(pem.expose()))))
}

// ===== PAYOUT ELIGIBILITY — VoP Check =====

impl PayoutEligibilityV2 for DeutschebankPayouts {}

impl
    ConnectorIntegrationV2<
        PayoutEligibility,
        PayoutFlowData,
        PayoutEligibilityRequest,
        PayoutEligibilityResponse,
    > for DeutschebankPayouts
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
    }

    fn get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn get_certificate(
        &self,
        req: &RouterDataV2<
            PayoutEligibility,
            PayoutFlowData,
            PayoutEligibilityRequest,
            PayoutEligibilityResponse,
        >,
    ) -> CustomResult<Option<Secret<String>>, IntegrationError> {
        cert_pem(&DeutschebankAuthType::try_from(&req.connector_config)?)
    }

    fn get_certificate_key(
        &self,
        req: &RouterDataV2<
            PayoutEligibility,
            PayoutFlowData,
            PayoutEligibilityRequest,
            PayoutEligibilityResponse,
        >,
    ) -> CustomResult<Option<Secret<String>>, IntegrationError> {
        cert_key_pem(&DeutschebankAuthType::try_from(&req.connector_config)?)
    }

    fn get_ca_certificate(
        &self,
        req: &RouterDataV2<
            PayoutEligibility,
            PayoutFlowData,
            PayoutEligibilityRequest,
            PayoutEligibilityResponse,
        >,
    ) -> CustomResult<Option<Secret<String>>, IntegrationError> {
        server_ca_pem(&DeutschebankAuthType::try_from(&req.connector_config)?)
    }

    fn get_url(
        &self,
        req: &RouterDataV2<
            PayoutEligibility,
            PayoutFlowData,
            PayoutEligibilityRequest,
            PayoutEligibilityResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Ok(format!(
            "{}{VOP_PATH}",
            self.base_url(&req.resource_common_data.connectors)
        ))
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<
            PayoutEligibility,
            PayoutFlowData,
            PayoutEligibilityRequest,
            PayoutEligibilityResponse,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = DeutschebankAuthType::try_from(&req.connector_config)?;
        let body_struct = DeutschebankVopRequest::try_from(req)?;
        let body_bytes = serialize_json(&body_struct)?;

        let mut headers = self.build_identity_headers(&auth, CORRELATION_PREFIX_VOP);
        let vop_id = derive_vop_id(
            req.resource_common_data.merchant_id.get_string_repr(),
            &req.resource_common_data.connector_request_reference_id,
        );
        headers.push((
            headers::X_VERIFICATIONOFPAYEE_IDENTIFIER.to_string(),
            vop_id.into(),
        ));
        self.append_cseal_headers(
            &mut headers,
            common_utils::request::Method::Post,
            VOP_PATH,
            &body_bytes,
            &auth,
        )?;
        Ok(headers)
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<
            PayoutEligibility,
            PayoutFlowData,
            PayoutEligibilityRequest,
            PayoutEligibilityResponse,
        >,
    ) -> CustomResult<Option<RequestContent>, IntegrationError> {
        let connector_req = DeutschebankVopRequest::try_from(req)?;
        Ok(Some(RequestContent::Json(Box::new(connector_req))))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            PayoutEligibility,
            PayoutFlowData,
            PayoutEligibilityRequest,
            PayoutEligibilityResponse,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<
            PayoutEligibility,
            PayoutFlowData,
            PayoutEligibilityRequest,
            PayoutEligibilityResponse,
        >,
        ConnectorError,
    > {
        let response: DeutschebankVopResponse = res
            .response
            .parse_struct("DeutschebankVopResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: Default::default(),
            })?;
        event_builder.map(|i| i.set_connector_response(&response));

        let vop_id = derive_vop_id(
            data.resource_common_data.merchant_id.get_string_repr(),
            &data.resource_common_data.connector_request_reference_id,
        );
        let resp = build_eligibility_response(response, vop_id, res.status_code)?;

        RouterDataV2::try_from(ResponseRouterData {
            response: resp,
            router_data: data.clone(),
            http_code: res.status_code,
        })
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, connector_config)
    }
}

// ===== PAYOUT TRANSFER =====

impl PayoutTransferV2 for DeutschebankPayouts {}

impl
    ConnectorIntegrationV2<
        PayoutTransfer,
        PayoutFlowData,
        PayoutTransferRequest,
        PayoutTransferResponse,
    > for DeutschebankPayouts
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
    }

    fn get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn get_certificate(
        &self,
        req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> CustomResult<Option<Secret<String>>, IntegrationError> {
        cert_pem(&DeutschebankAuthType::try_from(&req.connector_config)?)
    }

    fn get_certificate_key(
        &self,
        req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> CustomResult<Option<Secret<String>>, IntegrationError> {
        cert_key_pem(&DeutschebankAuthType::try_from(&req.connector_config)?)
    }

    fn get_ca_certificate(
        &self,
        req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> CustomResult<Option<Secret<String>>, IntegrationError> {
        server_ca_pem(&DeutschebankAuthType::try_from(&req.connector_config)?)
    }

    fn get_url(
        &self,
        req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Ok(format!(
            "{}{PAYMENT_PATH}",
            self.base_url(&req.resource_common_data.connectors)
        ))
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = DeutschebankAuthType::try_from(&req.connector_config)?;
        let built = DeutschebankSepaPaymentBuilt::try_from(req)?;
        let body_bytes = serialize_json(&built.request)?;

        let vop_id = req.request.connector_payout_id.clone().ok_or_else(|| {
            IntegrationError::MissingRequiredField {
                field_name: "connector_payout_id",
                context: domain_types::errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Deutsche Bank SEPA payment requires the VoP-ID from a prior \
                         PayoutEligibility call in `connector_payout_id`"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            }
        })?;

        let mut headers = self.build_identity_headers(&auth, CORRELATION_PREFIX_PAYMENT);
        headers.push((
            headers::X_VERIFICATIONOFPAYEE_IDENTIFIER.to_string(),
            vop_id.into(),
        ));
        self.append_cseal_headers(
            &mut headers,
            common_utils::request::Method::Post,
            PAYMENT_PATH,
            &body_bytes,
            &auth,
        )?;
        Ok(headers)
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> CustomResult<Option<RequestContent>, IntegrationError> {
        let built = DeutschebankSepaPaymentBuilt::try_from(req)?;
        Ok(Some(RequestContent::Json(Box::new(built.request))))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>,
        ConnectorError,
    > {
        let response: DeutschebankSepaPaymentResponse = res
            .response
            .parse_struct("DeutschebankSepaPaymentResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: Default::default(),
            })?;
        event_builder.map(|i| i.set_connector_response(&response));

        let built = DeutschebankSepaPaymentBuilt::try_from(data).change_context(
            ConnectorError::ResponseDeserializationFailed {
                context: Default::default(),
            },
        )?;
        let compound = encode_connector_payout_id(&built.end_to_end_id, &built.debtor_iban);

        let payout_status = response
            .extract_status()
            .map(common_enums::PayoutStatus::from)
            .unwrap_or(common_enums::PayoutStatus::Pending);

        Ok(RouterDataV2 {
            response: Ok(PayoutTransferResponse {
                merchant_payout_id: None,
                payout_status,
                connector_payout_id: Some(compound),
                status_code: res.status_code,
            }),
            ..data.clone()
        })
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, connector_config)
    }
}

// ===== PAYOUT GET =====

impl PayoutGetV2 for DeutschebankPayouts {}

impl ConnectorIntegrationV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>
    for DeutschebankPayouts
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
    }

    fn get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn get_certificate(
        &self,
        req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
    ) -> CustomResult<Option<Secret<String>>, IntegrationError> {
        cert_pem(&DeutschebankAuthType::try_from(&req.connector_config)?)
    }

    fn get_certificate_key(
        &self,
        req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
    ) -> CustomResult<Option<Secret<String>>, IntegrationError> {
        cert_key_pem(&DeutschebankAuthType::try_from(&req.connector_config)?)
    }

    fn get_ca_certificate(
        &self,
        req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
    ) -> CustomResult<Option<Secret<String>>, IntegrationError> {
        server_ca_pem(&DeutschebankAuthType::try_from(&req.connector_config)?)
    }

    fn get_url(
        &self,
        req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
    ) -> CustomResult<String, IntegrationError> {
        Ok(format!(
            "{}{STATUS_PATH}",
            self.base_url(&req.resource_common_data.connectors)
        ))
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = DeutschebankAuthType::try_from(&req.connector_config)?;
        let body_struct = DeutschebankStatusRequest::try_from(req)?;
        let body_bytes = serialize_json(&body_struct)?;

        let mut headers = self.build_identity_headers(&auth, CORRELATION_PREFIX_PAYMENT);
        self.append_cseal_headers(
            &mut headers,
            common_utils::request::Method::Post,
            STATUS_PATH,
            &body_bytes,
            &auth,
        )?;
        Ok(headers)
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
    ) -> CustomResult<Option<RequestContent>, IntegrationError> {
        let connector_req = DeutschebankStatusRequest::try_from(req)?;
        Ok(Some(RequestContent::Json(Box::new(connector_req))))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
        ConnectorError,
    > {
        let response: DeutschebankStatusResponse = res
            .response
            .parse_struct("DeutschebankStatusResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: Default::default(),
            })?;
        event_builder.map(|i| i.set_connector_response(&response));

        RouterDataV2::try_from(ResponseRouterData {
            response,
            router_data: data.clone(),
            http_code: res.status_code,
        })
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, connector_config)
    }
}

// ===== PAYOUT STUB FLOWS =====

impl PayoutCreateV2 for DeutschebankPayouts {}

impl ConnectorIntegrationV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>
    for DeutschebankPayouts
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            PayoutCreate,
            PayoutFlowData,
            PayoutCreateRequest,
            PayoutCreateResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            self.id(),
            "payout_create",
            Default::default(),
        )
        .into())
    }
}

impl PayoutVoidV2 for DeutschebankPayouts {}

impl ConnectorIntegrationV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>
    for DeutschebankPayouts
{
    fn get_url(
        &self,
        _req: &RouterDataV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            self.id(),
            "payout_void",
            Default::default(),
        )
        .into())
    }
}

impl PayoutStageV2 for DeutschebankPayouts {}

impl ConnectorIntegrationV2<PayoutStage, PayoutFlowData, PayoutStageRequest, PayoutStageResponse>
    for DeutschebankPayouts
{
    fn get_url(
        &self,
        _req: &RouterDataV2<PayoutStage, PayoutFlowData, PayoutStageRequest, PayoutStageResponse>,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            self.id(),
            "payout_stage",
            Default::default(),
        )
        .into())
    }
}

impl PayoutCreateLinkV2 for DeutschebankPayouts {}

impl
    ConnectorIntegrationV2<
        PayoutCreateLink,
        PayoutFlowData,
        PayoutCreateLinkRequest,
        PayoutCreateLinkResponse,
    > for DeutschebankPayouts
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            PayoutCreateLink,
            PayoutFlowData,
            PayoutCreateLinkRequest,
            PayoutCreateLinkResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            self.id(),
            "payout_create_link",
            Default::default(),
        )
        .into())
    }
}

impl PayoutCreateRecipientV2 for DeutschebankPayouts {}

impl
    ConnectorIntegrationV2<
        PayoutCreateRecipient,
        PayoutFlowData,
        PayoutCreateRecipientRequest,
        PayoutCreateRecipientResponse,
    > for DeutschebankPayouts
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            PayoutCreateRecipient,
            PayoutFlowData,
            PayoutCreateRecipientRequest,
            PayoutCreateRecipientResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            self.id(),
            "payout_create_recipient",
            Default::default(),
        )
        .into())
    }
}

impl PayoutEnrollDisburseAccountV2 for DeutschebankPayouts {}

impl
    ConnectorIntegrationV2<
        PayoutEnrollDisburseAccount,
        PayoutFlowData,
        PayoutEnrollDisburseAccountRequest,
        PayoutEnrollDisburseAccountResponse,
    > for DeutschebankPayouts
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            PayoutEnrollDisburseAccount,
            PayoutFlowData,
            PayoutEnrollDisburseAccountRequest,
            PayoutEnrollDisburseAccountResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            self.id(),
            "payout_enroll_disburse_account",
            Default::default(),
        )
        .into())
    }
}
