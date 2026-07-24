pub mod signing;
pub mod transformers;

#[cfg(test)]
mod test;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{
    consts::NO_ERROR_CODE,
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    request::{Request, RequestBuilder},
    Method,
};
use domain_types::{
    connector_flow::{PayoutEligibility, PayoutGet, PayoutTransfer, ServerAuthenticationToken},
    connector_types::{
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::PaymentMethodDataTypes,
    payouts::payouts_types::{
        PayoutEligibilityRequest, PayoutEligibilityResponse, PayoutFlowData, PayoutGetRequest,
        PayoutGetResponse, PayoutTransferRequest, PayoutTransferResponse,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use hyperswitch_masking::{ExposeInterface, Mask, Maskable, Secret};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{
        PayoutEligibilityV2, PayoutGetV2, PayoutServiceTrait, PayoutTransferV2,
        ServerAuthentication,
    },
};
use serde::Serialize;

use super::super::connectors::macros;
use crate::{types::ResponseRouterData, with_error_response_body};
use signing::{build_cseal_headers, CsealHeaders};
use transformers::{
    current_iso_utc_seconds, derive_vop_id, DeutschebankAuthType, DeutschebankErrorResponse,
    DeutschebankSepaPaymentRequest, DeutschebankSepaPaymentResponse, DeutschebankStatusRequest,
    DeutschebankStatusResponse, DeutschebankVopRequest, DeutschebankVopResponse,
};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const DATE: &str = "Date";
    pub(crate) const DIGEST: &str = "Digest";
    pub(crate) const SIGNATURE: &str = "Signature";
    pub(crate) const X_CORRELATION_IDENTIFIER: &str = "x-correlation-identifier";
    pub(crate) const X_APICONSUMER_REQUEST_TIMESTAMP: &str = "x-apiConsumer-request-timestamp";
    pub(crate) const X_CUSTOMER_IDENTIFIER: &str = "x-customer-identifier";
    pub(crate) const X_VERIFICATIONOFPAYEE_IDENTIFIER: &str = "x-verificationofpayee-identifier";
}

const VOP_PATH: &str = "/v1/cseal/payments/sepa/vop-check/vop";
const PAYMENT_PATH: &str = "/v2/cseal/payments/credit-transfer/sepa/payment";
const STATUS_PATH: &str = "/v2/cseal/payments/credit-transfer/sepa/status";

const CORRELATION_PREFIX_VOP: &str = "ACID";
const CORRELATION_PREFIX_PAYMENT: &str = "PYMT";

/// Connector id; also the `tracing` connector-field value.
const CONNECTOR_NAME: &str = "deutschebank";
/// The sole content type accepted by every Deutsche Bank CSEAL endpoint.
const APPLICATION_JSON: &str = "application/json";

macros::create_all_prerequisites!(
    connector_name: DeutschebankPayouts,
    generic_type: T,
    api: [
        (
            flow: PayoutEligibility,
            request_body: DeutschebankVopRequest,
            response_body: DeutschebankVopResponse,
            router_data: RouterDataV2<PayoutEligibility, PayoutFlowData, PayoutEligibilityRequest, PayoutEligibilityResponse>,
        ),
        (
            flow: PayoutTransfer,
            request_body: DeutschebankSepaPaymentRequest,
            response_body: DeutschebankSepaPaymentResponse,
            router_data: RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>,
        ),
        (
            flow: PayoutGet,
            request_body: DeutschebankStatusRequest,
            response_body: DeutschebankStatusResponse,
            router_data: RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
        )
    ],
    amount_converters: [],
    member_functions: {
        fn build_identity_headers(
            &self,
            auth: &DeutschebankAuthType,
            correlation_prefix: &str,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let correlation_id = format!(
                "{correlation_prefix}{}",
                uuid::Uuid::new_v4().simple().to_string().to_uppercase()
            );
            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    APPLICATION_JSON.to_string().into(),
                ),
                (
                    headers::X_CUSTOMER_IDENTIFIER.to_string(),
                    auth.customer_identifier.clone().expose().into_masked(),
                ),
                (
                    headers::X_CORRELATION_IDENTIFIER.to_string(),
                    correlation_id.into(),
                ),
                (
                    headers::X_APICONSUMER_REQUEST_TIMESTAMP.to_string(),
                    current_iso_utc_seconds()?.into(),
                ),
            ])
        }

        fn append_cseal_headers(
            &self,
            headers_out: &mut Vec<(String, Maskable<String>)>,
            method: Method,
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


        fn cseal_headers_for_bytes<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
            correlation_prefix: &str,
            path: &str,
            vop_id: Option<String>,
            body_bytes: &[u8],
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let auth = DeutschebankAuthType::try_from(&req.connector_config)?;
            let mut headers = self.build_identity_headers(&auth, correlation_prefix)?;
            if let Some(vop_id) = vop_id {
                headers.push((
                    headers::X_VERIFICATIONOFPAYEE_IDENTIFIER.to_string(),
                    vop_id.into(),
                ));
            }
            self.append_cseal_headers(&mut headers, Method::Post, path, body_bytes, &auth)?;
            Ok(headers)
        }


        fn build_cseal_request<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
            correlation_prefix: &str,
            path: &str,
            vop_id: Option<String>,
        ) -> CustomResult<Option<Request>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            let body = self.get_request_body(req)?;
            let body_bytes = body
                .as_ref()
                .map(|content| content.get_inner_value().expose().into_bytes())
                .unwrap_or_default();
            let headers =
                self.cseal_headers_for_bytes(req, correlation_prefix, path, vop_id, &body_bytes)?;
            Ok(Some(
                RequestBuilder::new()
                    .method(self.get_http_method())
                    .url(self.get_url(req)?.as_str())
                    .attach_default_headers()
                    .headers(headers)
                    .set_optional_body(body)
                    .add_certificate(self.get_certificate(req)?)
                    .add_certificate_key(self.get_certificate_key(req)?)
                    .add_ca_certificate_pem(self.get_ca_certificate(req)?)
                    .build(),
            ))
        }

        fn db_certificate<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Option<Secret<String>>, IntegrationError> {
            cert_pem(&DeutschebankAuthType::try_from(&req.connector_config)?)
        }

        fn db_certificate_key<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Option<Secret<String>>, IntegrationError> {
            cert_key_pem(&DeutschebankAuthType::try_from(&req.connector_config)?)
        }

        fn db_ca_certificate<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, PayoutFlowData, Req, Res>,
        ) -> CustomResult<Option<Secret<String>>, IntegrationError> {
            server_ca_pem(
                req.resource_common_data
                    .connectors
                    .deutschebank
                    .server_ca_bundle
                    .as_deref(),
            )
        }
    }
);

// ===== CONNECTOR COMMON =====

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for DeutschebankPayouts<T>
{
    fn id(&self) -> &'static str {
        CONNECTOR_NAME
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        APPLICATION_JSON
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
                    raw_body = %String::from_utf8_lossy(res.response.as_ref()),
                    "Failed to parse Deutsche Bank error response",
                );
                DeutschebankErrorResponse::default()
            });

        with_error_response_body!(event_builder, response);

        let first_error = response.errors.as_ref().and_then(|errs| errs.first());

        let code = response
            .code
            .clone()
            .or_else(|| response.error_code.clone())
            .or_else(|| first_error.and_then(|e| e.code.clone()))
            .unwrap_or_else(|| NO_ERROR_CODE.to_string());
        let message = response
            .message
            .clone()
            .or_else(|| response.error_message.clone())
            .or_else(|| first_error.and_then(|e| e.message.clone()))
            .unwrap_or_else(|| match code.as_str() {
                NO_ERROR_CODE => {
                    format!("Deutsche Bank request failed (HTTP {})", res.status_code)
                }
                _ => format!(
                    "Deutsche Bank request failed: {code} (HTTP {})",
                    res.status_code
                ),
            });

        let mut details: Vec<String> = Vec::new();
        if let Some(reason) = response.reason.as_deref() {
            details.push(reason.to_string());
        }
        for entry in response.errors.iter().flatten() {
            let parts: Vec<String> = [
                entry.code.as_deref(),
                entry.message.as_deref(),
                entry.reason.as_deref(),
            ]
            .into_iter()
            .flatten()
            .map(str::to_string)
            .collect();
            if !parts.is_empty() {
                details.push(parts.join(": "));
            }
        }

        if !response.additional.is_empty() {
            tracing::warn!(
                connector = CONNECTOR_NAME,
                status_code = res.status_code,
                unmapped_error_fields = ?response.additional,
                "Deutsche Bank error response carried fields outside our model",
            );
        }

        let reason = match details.as_slice() {
            [] => Some(message.clone()),
            _ => Some(details.join(" | ")),
        };

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

// ===== SERVER AUTHENTICATION (not implemented) =====

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ServerAuthentication
    for DeutschebankPayouts<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerAuthenticationToken,
        MerchantAuthenticationFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    > for DeutschebankPayouts<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            ServerAuthenticationToken,
            MerchantAuthenticationFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            ConnectorCommon::id(self),
            "server_authentication_token",
            IntegrationErrorContext::default(),
        )
        .into())
    }
}

// ===== PAYOUT SERVICE TRAIT + REAL-FLOW MARKERS =====

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutServiceTrait
    for DeutschebankPayouts<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutEligibilityV2
    for DeutschebankPayouts<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutTransferV2
    for DeutschebankPayouts<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutGetV2
    for DeutschebankPayouts<T>
{
}

// ===== mTLS material =====

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

/// Trust anchor for verifying DB's *server* certificate.
///
/// Read from per-connector env config (`Connectors::deutschebank.server_ca_bundle`)
/// rather than the merchant's MCA, since the CA is environment-level
/// infrastructure shared by every merchant pointing at the same DB endpoint.
fn server_ca_pem(bundle: Option<&str>) -> CustomResult<Option<Secret<String>>, IntegrationError> {
    Ok(bundle
        .map(str::trim)
        .filter(|pem| !pem.is_empty())
        .map(|pem| Secret::new(b64_pem(pem.to_string()))))
}

// ===== PAYOUT ELIGIBILITY — VoP Check =====

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: DeutschebankPayouts,
    curl_request: Json(DeutschebankVopRequest),
    curl_response: DeutschebankVopResponse,
    flow_name: PayoutEligibility,
    resource_common_data: PayoutFlowData,
    flow_request: PayoutEligibilityRequest,
    flow_response: PayoutEligibilityResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
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

        fn build_request_v2(
            &self,
            req: &RouterDataV2<
                PayoutEligibility,
                PayoutFlowData,
                PayoutEligibilityRequest,
                PayoutEligibilityResponse,
            >,
        ) -> CustomResult<Option<Request>, IntegrationError> {
            let vop_id = derive_vop_id(
                req.resource_common_data.merchant_id.get_string_repr(),
                &req.resource_common_data.connector_request_reference_id,
            );
            self.build_cseal_request(req, CORRELATION_PREFIX_VOP, VOP_PATH, Some(vop_id))
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
            self.db_certificate(req)
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
            self.db_certificate_key(req)
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
            self.db_ca_certificate(req)
        }
    }
);

// ===== PAYOUT TRANSFER — SEPA Credit Transfer =====

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: DeutschebankPayouts,
    curl_request: Json(DeutschebankSepaPaymentRequest),
    curl_response: DeutschebankSepaPaymentResponse,
    flow_name: PayoutTransfer,
    resource_common_data: PayoutFlowData,
    flow_request: PayoutTransferRequest,
    flow_response: PayoutTransferResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
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

        fn build_request_v2(
            &self,
            req: &RouterDataV2<
                PayoutTransfer,
                PayoutFlowData,
                PayoutTransferRequest,
                PayoutTransferResponse,
            >,
        ) -> CustomResult<Option<Request>, IntegrationError> {
            let vop_id = req.request.connector_payout_id.clone().ok_or_else(|| {
                IntegrationError::MissingRequiredField {
                    field_name: "connector_payout_id",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Deutsche Bank SEPA payment requires the VoP-ID from a prior \
                             PayoutEligibility call in `connector_payout_id`"
                                .to_string(),
                        ),
                        suggested_action: Some(
                            "Call PayoutService/Eligibility first and pass the returned \
                             `connectorPayoutId` on Transfer."
                                .to_string(),
                        ),
                        doc_url: None,
                    },
                }
            })?;
            self.build_cseal_request(req, CORRELATION_PREFIX_PAYMENT, PAYMENT_PATH, Some(vop_id))
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
            self.db_certificate(req)
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
            self.db_certificate_key(req)
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
            self.db_ca_certificate(req)
        }
    }
);

// ===== PAYOUT GET — SEPA Status Enquiry =====

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: DeutschebankPayouts,
    curl_request: Json(DeutschebankStatusRequest),
    curl_response: DeutschebankStatusResponse,
    flow_name: PayoutGet,
    resource_common_data: PayoutFlowData,
    flow_request: PayoutGetRequest,
    flow_response: PayoutGetResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}{STATUS_PATH}",
                self.base_url(&req.resource_common_data.connectors)
            ))
        }

        fn build_request_v2(
            &self,
            req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
        ) -> CustomResult<Option<Request>, IntegrationError> {
            self.build_cseal_request(req, CORRELATION_PREFIX_PAYMENT, STATUS_PATH, None)
        }

        fn get_certificate(
            &self,
            req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
        ) -> CustomResult<Option<Secret<String>>, IntegrationError> {
            self.db_certificate(req)
        }

        fn get_certificate_key(
            &self,
            req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
        ) -> CustomResult<Option<Secret<String>>, IntegrationError> {
            self.db_certificate_key(req)
        }

        fn get_ca_certificate(
            &self,
            req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
        ) -> CustomResult<Option<Secret<String>>, IntegrationError> {
            self.db_ca_certificate(req)
        }
    }
);

// ===== PAYOUT STUB FLOWS =====

macros::macro_connector_payout_implementation!(
    connector: DeutschebankPayouts,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    payout_flows: [
        PayoutCreate,
        PayoutVoid,
        PayoutStage,
        PayoutCreateLink,
        PayoutCreateRecipient,
        PayoutEnrollDisburseAccount
    ]
);
