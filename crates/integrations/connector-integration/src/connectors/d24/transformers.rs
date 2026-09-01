use common_enums::AttemptStatus;
use common_utils::{pii::Email, request::Method, types::FloatMajorUnit};
use domain_types::{
    connector_flow::Authorize,
    connector_types::{PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, ResponseId},
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::{DocumentKind, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
    utils,
};
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

use crate::{connectors::d24::D24RouterData, types::ResponseRouterData};

// =============================================================================
// AUTH
// =============================================================================

/// Directa24 Deposits v3 credentials.
///
/// * `api_key`    — the POST-credential **API Key**. Sent verbatim as `X-Login`
///   and is the second component of the signed string.
/// * `key1`       — the **read-only API Key**. Only used as `X-Login` on the
///   read-only `GET` endpoints (PSync / RSync), which are out of scope here.
/// * `api_secret` — the **API Signature**. HMAC-SHA256 key; never transmitted.
#[derive(Debug, Clone)]
pub struct D24AuthType {
    pub api_key: Secret<String>,
    pub key1: Secret<String>,
    pub api_secret: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for D24AuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::D24 {
                api_key,
                key1,
                api_secret,
                ..
            } => Ok(Self {
                api_key: api_key.to_owned(),
                key1: key1.to_owned(),
                api_secret: api_secret.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: IntegrationErrorContext::default()
                }
            )),
        }
    }
}

// =============================================================================
// ERROR
// =============================================================================

/// Flat error envelope returned by `POST /v3/deposits` on the PCI (card) host:
/// `{ "code": 201, "description": "...", "details": ["..."], "type": "BEAN_VALIDATION_ERROR" }`.
///
/// Every field is optional so that the deserializer tolerates the partially
/// documented variants of the envelope (`details`/`type` are frequently absent).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct D24ErrorResponse {
    pub code: Option<i64>,
    pub description: Option<String>,
    pub details: Option<Vec<String>>,
    #[serde(rename = "type")]
    pub error_type: Option<String>,
}

// =============================================================================
// REQUEST
// =============================================================================

/// `POST {cc-api host}/v3/deposits` — Directa24 Server2Server ("webpaycard")
/// card deposit. Field names are snake_case exactly as documented; `amount` is a
/// JSON **number in major units** (1000 == 1000.00 BRL).
#[derive(Debug, Serialize)]
pub struct D24PaymentsRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub country: common_enums::CountryAlpha2,
    pub amount: FloatMajorUnit,
    pub currency: common_enums::Currency,
    pub invoice_id: String,
    pub payer: D24Payer,
    pub credit_card: D24CreditCard<T>,
    pub client_ip: Secret<String, common_utils::pii::IpAddress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// External-MPI (merchant-side 3DS) pass-through. Omitted entirely when UCS
    /// supplies no authentication artefacts, which lets D24 run its own 3DS.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_domain_secure: Option<D24ThreeDomainSecure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub back_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notification_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct D24Payer {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub document: Secret<String>,
    pub document_type: String,
    pub email: Email,
    pub first_name: Secret<String>,
    pub last_name: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<D24PayerAddress>,
}

#[derive(Debug, Serialize)]
pub struct D24PayerAddress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub street: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zip_code: Option<Secret<String>>,
}

/// Raw PAN block. The OpenAPI schema names the PAN field `card_number`; the one
/// hand-written curl sample that spells it `number` is a documentation typo.
#[derive(Debug, Serialize)]
pub struct D24CreditCard<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub card_number: RawCardNumber<T>,
    /// Zero-padded `MM`.
    pub expiration_month: Secret<String>,
    /// Last two digits only (`YY`) — D24 rejects a 4-digit year.
    pub expiration_year: Secret<String>,
    pub cvv: Secret<String>,
    pub holder_name: Secret<String>,
}

#[derive(Debug, Serialize)]
pub struct D24ThreeDomainSecure {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cavv: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eci: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub specification_version: Option<String>,
}

/// Directa24 document types are the uppercase form of the UCS `DocumentKind`.
fn d24_document_type(kind: DocumentKind) -> String {
    match kind {
        DocumentKind::Cpf => "CPF",
        DocumentKind::Cnpj => "CNPJ",
        DocumentKind::Psn => "PSN",
        DocumentKind::Other => "OTHER",
    }
    .to_string()
}

/// `payer.id` is constrained to `^[A-Za-z0-9]*$` (max 128). UCS customer ids may
/// contain separators, so anything that does not fit is dropped — D24 then
/// autogenerates the payer id.
fn sanitize_payer_id(customer_id: Option<String>) -> Option<String> {
    customer_id.filter(|id| {
        !id.is_empty() && id.len() <= 128 && id.chars().all(|c| c.is_ascii_alphanumeric())
    })
}

/// `invoice_id` is constrained to `^[A-Za-z0-9-_]*$` (max 128).
fn sanitize_invoice_id(reference: &str) -> String {
    reference
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(128)
        .collect()
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        D24RouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for D24PaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: D24RouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;

        // Directa24 has no capture endpoint at all — the card deposit is
        // sale-only. Reject a manual-capture request before it reaches D24.
        if !request.is_auto_capture() {
            return Err(error_stack::report!(IntegrationError::NotImplemented(
                "manual capture is not supported by Directa24 (no capture endpoint)".to_string(),
                IntegrationErrorContext::default(),
            )));
        }

        let card = match &request.payment_method_data {
            PaymentMethodData::Card(card) => card,
            _ => {
                return Err(error_stack::report!(IntegrationError::NotImplemented(
                    "Directa24 supports card payments only".to_string(),
                    IntegrationErrorContext::default(),
                )))
            }
        };

        let billing = router_data.resource_common_data.get_billing_address()?;
        let country = *billing.get_country()?;

        let amount = utils::convert_amount(
            item.connector.amount_converter,
            request.minor_amount,
            request.currency,
        )?;

        let document = request.get_customer_document_details()?;

        let holder_name = card
            .card_holder_name
            .clone()
            .or_else(|| billing.get_optional_full_name())
            .ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                    field_name: "credit_card.holder_name",
                    context: IntegrationErrorContext::default(),
                })
            })?;

        let payer_address = {
            let street = billing.get_optional_line1();
            let city = billing.get_optional_city();
            let state = billing.state.clone();
            let zip_code = billing.get_optional_zip();
            if street.is_some() || city.is_some() || state.is_some() || zip_code.is_some() {
                Some(D24PayerAddress {
                    street,
                    city,
                    state,
                    zip_code,
                })
            } else {
                None
            }
        };

        // External / third-party 3DS pass-through. Only emitted when UCS actually
        // carries MPI artefacts; otherwise the object is omitted so that D24 runs
        // its own challenge and answers with `authentication_url`.
        let three_domain_secure = request.authentication_data.as_ref().and_then(|auth| {
            if auth.cavv.is_none() && auth.eci.is_none() {
                return None;
            }
            Some(D24ThreeDomainSecure {
                cavv: auth.cavv.clone(),
                eci: auth.eci.clone(),
                transaction_id: auth
                    .threeds_server_transaction_id
                    .clone()
                    .or_else(|| auth.transaction_id.clone()),
                specification_version: auth
                    .message_version
                    .as_ref()
                    .map(|version| version.to_string()),
            })
        });

        // The D24-managed 3DS challenge is opened in a new tab, so the deposit
        // request carries the merchant redirect targets. UCS has a single return
        // URL — it is mapped onto all three.
        let return_url = request.router_return_url.clone();
        let (success_url, back_url, error_url) = match (&three_domain_secure, &return_url) {
            (None, Some(url)) => (Some(url.clone()), Some(url.clone()), Some(url.clone())),
            _ => (None, None, None),
        };

        Ok(Self {
            country,
            amount,
            currency: request.currency,
            invoice_id: sanitize_invoice_id(
                &router_data
                    .resource_common_data
                    .connector_request_reference_id,
            ),
            payer: D24Payer {
                id: sanitize_payer_id(
                    request
                        .customer_id
                        .as_ref()
                        .map(|customer_id| customer_id.get_string_repr().to_string()),
                ),
                document: document.document_number,
                document_type: d24_document_type(document.document_type),
                email: request.get_email()?,
                first_name: billing.get_first_name()?.clone(),
                last_name: billing.get_last_name()?.clone(),
                phone: router_data
                    .resource_common_data
                    .get_optional_billing()
                    .and_then(|address| address.phone.as_ref())
                    .and_then(|phone| phone.get_number_with_country_code().ok()),
                address: payer_address,
            },
            credit_card: D24CreditCard {
                card_number: card.card_number.clone(),
                expiration_month: card.get_card_expiry_month_2_digit()?,
                expiration_year: card.get_card_expiry_year_2_digit()?,
                cvv: card.card_cvc.clone(),
                holder_name,
            },
            client_ip: request.get_ip_address()?,
            description: router_data.resource_common_data.description.clone(),
            three_domain_secure,
            success_url,
            back_url,
            error_url,
            notification_url: request.webhook_url.clone(),
        })
    }
}

// =============================================================================
// RESPONSE
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum D24PaymentResult {
    Success,
    Rejected,
    PendingAuthentication,
    InProgress,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct D24PaymentInfo {
    #[serde(rename = "type")]
    pub payment_type: Option<String>,
    pub result: D24PaymentResult,
    pub reason: Option<String>,
    pub reason_code: Option<String>,
    pub payment_method: Option<String>,
    pub payment_method_name: Option<String>,
    pub amount: Option<f64>,
    pub currency: Option<String>,
    pub created_at: Option<String>,
    /// The 3DS documentation nests `authentication_url` here, while the OpenAPI
    /// schema puts it at the response root. Both positions are parsed.
    pub authentication_url: Option<url::Url>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct D24PaymentsResponse {
    pub deposit_id: i64,
    pub user_id: Option<String>,
    pub merchant_invoice_id: Option<String>,
    /// Root-level `authentication_url` (OpenAPI `PCIDepositResponse`). Preferred
    /// over the copy nested in `payment_info`.
    pub authentication_url: Option<url::Url>,
    pub payment_info: D24PaymentInfo,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ResponseRouterData<
            D24PaymentsResponse,
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        >,
    > for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            D24PaymentsResponse,
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        >,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let deposit_id = response.deposit_id.to_string();

        // Root first, then the nested copy documented on the 3DS page.
        let authentication_url = response
            .authentication_url
            .clone()
            .or_else(|| response.payment_info.authentication_url.clone());
        let redirection_data = authentication_url
            .clone()
            .map(|url| RedirectForm::from((url, Method::Get)));

        // Directa24 card deposits are sale-only: nothing maps to `Authorized`.
        let status = match response.payment_info.result {
            D24PaymentResult::Success => AttemptStatus::Charged,
            D24PaymentResult::Rejected => AttemptStatus::Failure,
            D24PaymentResult::PendingAuthentication => AttemptStatus::AuthenticationPending,
            D24PaymentResult::InProgress => {
                if authentication_url.is_some() {
                    AttemptStatus::AuthenticationPending
                } else {
                    AttemptStatus::Pending
                }
            }
        };

        if matches!(response.payment_info.result, D24PaymentResult::Rejected) {
            let code = response
                .payment_info
                .reason_code
                .clone()
                .unwrap_or_else(|| "REJECTED".to_string());
            let message = response
                .payment_info
                .reason
                .clone()
                .unwrap_or_else(|| "The transaction was rejected by Directa24".to_string());
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                response: Err(ErrorResponse {
                    status_code: item.http_code,
                    code,
                    message: message.clone(),
                    reason: Some(message),
                    attempt_status: Some(FlowStatus::Payment(status)),
                    connector_transaction_id: Some(deposit_id),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..item.router_data
            });
        }

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(deposit_id),
                redirection_data: redirection_data.map(Box::new),
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: response.merchant_invoice_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            }),
            ..item.router_data
        })
    }
}

/// Helper used by `build_error_response` to flatten `details` into a reason.
impl D24ErrorResponse {
    pub fn reason(&self) -> Option<String> {
        match (&self.details, &self.error_type) {
            (Some(details), _) if !details.is_empty() => Some(details.join(", ")),
            (_, Some(error_type)) => Some(error_type.clone()),
            _ => self.description.clone(),
        }
    }
}
