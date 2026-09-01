use common_enums::AttemptStatus;
use common_utils::{pii::Email, request::Method, types::FloatMajorUnit};
use domain_types::{
    connector_flow::{Authorize, PSync},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData,
        RawConnectorStatus, ResponseId,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::{
        CardRedirectData, DocumentKind, PaymentMethodData, PaymentMethodDataTypes,
    },
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
    utils,
};
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::{connectors::d24::D24RouterData, types::ResponseRouterData};

/// Directa24 payment method code for **WebPay** (Transbank, Chile). D24 method
/// codes are an open, per-country set published on the coverage page, not a
/// closed enum, so this stays a plain constant rather than a Rust enum.
const D24_PAYMENT_METHOD_WEBPAY: &str = "WP";

/// Chilean national identifier as Directa24 names it. `DocumentKind` has no
/// `Rut` variant, so `DocumentKind::Other` + a Chilean billing country is the
/// only representable spelling.
const D24_DOCUMENT_TYPE_RUT: &str = "RUT";

// =============================================================================
// AUTH
// =============================================================================

/// Directa24 Deposits v3 credentials.
///
/// * `api_key`    — the deposit (write) **API Key**. Sent verbatim as `X-Login`
///   on `POST /v3/deposits` and is the second component of the signed string.
/// * `key1`       — the **read-only API Key**. `X-Login` on the read-only `GET`
///   endpoints (PSync).
/// * `api_secret` — the **API Signature**. HMAC-SHA256 key; never transmitted.
///   D24 issues a single signature that covers both API Keys.
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

/// `details` is `["..."]` on deposit creation and a single nullable string on
/// deposit status. Untagged so one type covers both.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum D24ErrorDetails {
    Many(Vec<String>),
    One(String),
}

impl D24ErrorDetails {
    fn joined(&self) -> Option<String> {
        match self {
            Self::Many(details) if details.is_empty() => None,
            Self::Many(details) => Some(details.join(", ")),
            Self::One(detail) if detail.is_empty() => None,
            Self::One(detail) => Some(detail.clone()),
        }
    }
}

/// The error payload itself, in the shape shared by both envelopes once the
/// nesting is stripped.
///
/// `code` is an **integer** on `POST /v3/deposits`
/// (`{code: 201, description, details: [..]}`) and a **string** on
/// `GET /v3/deposits/{id}` (`ApiError.error.code`), so it is kept as an
/// untyped `Value` and stringified only when the ErrorResponse is built.
/// `description` / `message` are the two spellings of the same field.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct D24ErrorBody {
    pub code: Option<serde_json::Value>,
    pub description: Option<String>,
    pub message: Option<String>,
    pub details: Option<D24ErrorDetails>,
    #[serde(rename = "type")]
    pub error_type: Option<String>,
}

impl D24ErrorBody {
    pub fn code_string(&self) -> Option<String> {
        self.code.as_ref().map(|code| match code {
            serde_json::Value::String(code) => code.clone(),
            other => other.to_string(),
        })
    }

    pub fn message_string(&self) -> Option<String> {
        self.description.clone().or_else(|| self.message.clone())
    }

    pub fn reason(&self) -> Option<String> {
        self.details
            .as_ref()
            .and_then(|details| details.joined())
            .or_else(|| self.error_type.clone())
            .or_else(|| self.message_string())
    }
}

/// Directa24 answers with two structurally different error envelopes on the
/// same connector:
///
/// * `POST /v3/deposits` → flat `{code: integer, description, details: [string]}`
/// * `GET /v3/deposits/{id}` → nested `ApiError`
///   `{"error": {code: string, message, details: string|null}}`
///
/// `build_error_response` is a single shared `ConnectorCommon` method, so both
/// must parse. `Nested` is listed first because every field of `D24ErrorBody`
/// is optional — `Flat` would otherwise swallow the nested envelope and produce
/// an all-`None` error.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum D24ErrorResponse {
    Nested { error: D24ErrorBody },
    Flat(D24ErrorBody),
}

impl D24ErrorResponse {
    pub fn body(&self) -> &D24ErrorBody {
        match self {
            Self::Nested { error } => error,
            Self::Flat(error) => error,
        }
    }
}

// =============================================================================
// REQUEST
// =============================================================================

/// `POST {base}/v3/deposits` — Directa24 deposit creation for the WebPay (`WP`)
/// redirect method. Field names are snake_case exactly as documented; `amount`
/// is a JSON **number in major units**.
///
/// There are **no card fields on this endpoint**: WebPay is a hosted redirect,
/// Transbank collects the card details and runs its own authentication.
#[derive(Debug, Serialize)]
pub struct D24PaymentsRequest {
    pub country: common_enums::CountryAlpha2,
    pub amount: FloatMajorUnit,
    pub currency: common_enums::Currency,
    pub invoice_id: String,
    pub payer: D24Payer,
    /// Always `"WP"` for this integration.
    pub payment_method: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub back_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_ip: Option<Secret<String, common_utils::pii::IpAddress>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    // `notification_url` is deliberately NOT sent. Directa24 webhooks are out
    // of scope for this integration and `IncomingWebhook` is an empty impl, so
    // supplying the URL would make D24 POST deposit-status notifications at an
    // endpoint that cannot parse them — pure noise in the merchant's webhook
    // log. Status is resolved by PSync instead. Add this field together with a
    // real `IncomingWebhook` implementation, not before.
}

#[derive(Debug, Serialize)]
pub struct D24Payer {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub document: Secret<String>,
    pub document_type: &'static str,
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

/// Maps the UCS document kind onto a Directa24 `payer.document_type`.
///
/// `DocumentKind` carries no `Rut` variant, so a Chilean RUT can only arrive as
/// `DocumentKind::Other`; combined with a Chilean billing country that is
/// unambiguously a RUT, which is the only document type WebPay accepts.
fn d24_document_type(
    kind: DocumentKind,
    country: common_enums::CountryAlpha2,
) -> Result<&'static str, error_stack::Report<IntegrationError>> {
    match (kind, country) {
        (DocumentKind::Other, common_enums::CountryAlpha2::CL) => Ok(D24_DOCUMENT_TYPE_RUT),
        (DocumentKind::Cpf, _) => Ok("CPF"),
        (DocumentKind::Cnpj, _) => Ok("CNPJ"),
        (DocumentKind::Psn, _) => Ok("PSN"),
        _ => Err(error_stack::report!(IntegrationError::NotSupported {
            message: format!("customer document type {kind:?} for billing country {country}"),
            connector: "d24",
            context: IntegrationErrorContext::default(),
        })),
    }
}

/// A Chilean RUT is 7-8 body digits plus one check character (`0-9` or `K`),
/// i.e. 8-9 characters once dots and the hyphen are stripped.
///
/// `DocumentKind::Other` deliberately skips the checksum validation that
/// `Cpf`/`Cnpj` get, so this is the only place a malformed RUT is caught before
/// it reaches Directa24 as an opaque `BEAN_VALIDATION_ERROR`.
fn validate_and_normalize_rut(
    document: &Secret<String>,
) -> Result<Secret<String>, error_stack::Report<IntegrationError>> {
    let normalized: String = document
        .peek()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect();

    let invalid = || {
        error_stack::report!(IntegrationError::InvalidDataFormat {
            field_name: "payer.document",
            context: IntegrationErrorContext::default(),
        })
    };

    if !(8..=9).contains(&normalized.len()) {
        return Err(invalid());
    }

    let (body, check) = normalized.split_at(normalized.len() - 1);
    if !body.chars().all(|c| c.is_ascii_digit()) {
        return Err(invalid());
    }
    if !check.chars().all(|c| c.is_ascii_digit() || c == 'K') {
        return Err(invalid());
    }

    Ok(Secret::new(normalized))
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
    > for D24PaymentsRequest
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

        // --- Guard 1: payment method -----------------------------------------
        // WebPay is a hosted redirect — Transbank collects the card details on
        // its own page, so the only representable domain shape is the empty
        // CardRedirect marker. No card data may reach this endpoint.
        match &request.payment_method_data {
            PaymentMethodData::CardRedirect(CardRedirectData::CardRedirect {}) => {}
            _ => {
                return Err(error_stack::report!(IntegrationError::NotImplemented(
                    "Directa24 WebPay only supports \
                     PaymentMethodData::CardRedirect(CardRedirect {}) — the customer enters \
                     card details on Transbank's own page, so no card data is sent to \
                     /v3/deposits"
                        .to_string(),
                    IntegrationErrorContext::default(),
                )))
            }
        }

        // --- Guard 2: billing country ----------------------------------------
        // WebPay is Chile-only.
        let billing = router_data.resource_common_data.get_billing_address()?;
        let country = *billing.get_country()?;
        if country != common_enums::CountryAlpha2::CL {
            return Err(error_stack::report!(IntegrationError::NotSupported {
                message: format!(
                    "WebPay (WP) in billing country {country}; WebPay is available in Chile (CL) only"
                ),
                connector: "d24",
                context: IntegrationErrorContext::default(),
            }));
        }

        // --- Guard 3: currency -----------------------------------------------
        if !matches!(
            request.currency,
            common_enums::Currency::CLP | common_enums::Currency::USD
        ) {
            return Err(error_stack::report!(IntegrationError::NotSupported {
                message: format!(
                    "currency {} for WebPay (WP); only CLP and USD are accepted",
                    request.currency
                ),
                connector: "d24",
                context: IntegrationErrorContext::default(),
            }));
        }

        // --- Guard 4: capture method -----------------------------------------
        // Directa24 documents no capture endpoint at all — deposits are
        // sale-only. Reject a manual-capture request before it reaches D24.
        if !request.is_auto_capture() {
            return Err(error_stack::report!(IntegrationError::NotImplemented(
                "manual capture is not supported by Directa24 (no capture endpoint)".to_string(),
                IntegrationErrorContext::default(),
            )));
        }

        let amount = utils::convert_amount(
            item.connector.amount_converter,
            request.minor_amount,
            request.currency,
        )?;

        let document = request.get_customer_document_details()?;
        let document_type = d24_document_type(document.document_type, country)?;
        let document_number = validate_and_normalize_rut(&document.document_number)?;

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

        // UCS carries a single return URL; D24 wants three. All three land back
        // on the same HS redirect-sync endpoint, which is what triggers PSync.
        let return_url = request.router_return_url.clone();

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
                document: document_number,
                document_type,
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
            payment_method: D24_PAYMENT_METHOD_WEBPAY,
            success_url: return_url.clone(),
            back_url: return_url.clone(),
            error_url: return_url,
            client_ip: request.get_ip_address_as_optional(),
            description: router_data.resource_common_data.description.clone(),
            language: request.get_optional_language_from_browser_info(),
        })
    }
}

// =============================================================================
// AUTHORIZE RESPONSE
// =============================================================================

/// The 201 `oneOf` declares `checkout_type` with **no enum**, so an unannounced
/// value must not break deserialization. It is informational only — the flow
/// decision is driven off `redirect_url`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum D24CheckoutType {
    Hosted,
    OneShot,
    #[serde(other)]
    Unknown,
}

/// `201` from `POST /v3/deposits`. The HOSTED and ONE_SHOT branches of the
/// documented `oneOf` differ only by the extra `payment_info` object, so one
/// struct with everything optional covers both.
///
/// There is no result/decline field on this response — a rejected deposit
/// arrives as an HTTP 400 (handled by `build_error_response`) or later as
/// `DECLINED` on PSync. `AttemptStatus::Charged` is therefore unreachable here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct D24PaymentsResponse {
    pub deposit_id: i64,
    pub checkout_type: Option<D24CheckoutType>,
    pub redirect_url: Option<url::Url>,
    pub user_id: Option<String>,
    pub merchant_invoice_id: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<D24PaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<D24PaymentsResponse, Self>) -> Result<Self, Self::Error> {
        let response = item.response;
        let deposit_id = response.deposit_id.to_string();

        // Status is driven off `redirect_url`, NOT `checkout_type`: the OpenAPI
        // declares no enum for `checkout_type`, so an unannounced value must not
        // decide whether the customer is redirected. A URL means the customer
        // still has to authenticate on Transbank's page; no URL means the
        // deposit exists but nothing is actionable yet — PSync resolves it.
        let (status, redirection_data) = match response.redirect_url.clone() {
            Some(url) => (
                AttemptStatus::AuthenticationPending,
                Some(RedirectForm::from((url, Method::Get))),
            ),
            None => (AttemptStatus::Pending, None),
        };

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

// =============================================================================
// PSYNC — GET /v3/deposits/{deposit_id}
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum D24DepositStatus {
    Completed,
    Pending,
    Created,
    Declined,
    Cancelled,
    Expired,
    EarlyReleased,
    ForReview,
}

impl D24DepositStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "COMPLETED",
            Self::Pending => "PENDING",
            Self::Created => "CREATED",
            Self::Declined => "DECLINED",
            Self::Cancelled => "CANCELLED",
            Self::Expired => "EXPIRED",
            Self::EarlyReleased => "EARLY_RELEASED",
            Self::ForReview => "FOR_REVIEW",
        }
    }
}

impl From<D24DepositStatus> for AttemptStatus {
    fn from(status: D24DepositStatus) -> Self {
        match status {
            D24DepositStatus::Completed => Self::Charged,
            // The customer opened the checkout but has not finished paying.
            // Ambiguous, not terminal — never map this to Failure.
            D24DepositStatus::Pending => Self::Pending,
            // Deposit exists, the customer has not opened the link yet: still
            // redirect-awaited.
            D24DepositStatus::Created => Self::AuthenticationPending,
            // "No transaction will change its status from DECLINED."
            D24DepositStatus::Declined => Self::Failure,
            // NOTE: Directa24 documents that EXPIRED and CANCELLED deposits can
            // be moved back to COMPLETED by D24 support after manual
            // intervention. Hyperswitch treats both as terminal and stops
            // polling, so such a reversal is reconciled out of band rather than
            // picked up by PSync. Accepted deliberately.
            D24DepositStatus::Cancelled => Self::Voided,
            D24DepositStatus::Expired => Self::Expired,
            // TRAP: reads terminal, is not. Early release credits the MERCHANT's
            // balance ahead of settlement while, per D24, "the customer hasn't
            // paid yet and the money won't be credited". Mapping it to Charged
            // books unpaid revenue. Only reachable when `early_release: true`
            // was sent — which this integration never does — but handled anyway.
            D24DepositStatus::EarlyReleased => Self::Pending,
            // TRAP: a transient anti-fraud hold that resolves to COMPLETED or
            // DECLINED on its own. NOT Unresolved (Hyperswitch reads that as
            // needing merchant action) and NOT Failure (it may still succeed).
            D24DepositStatus::ForReview => Self::Pending,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct D24SyncResponse {
    pub deposit_id: i64,
    pub status: D24DepositStatus,
    pub invoice_id: Option<String>,
    pub user_id: Option<String>,
    pub country: Option<String>,
    pub currency: Option<String>,
    pub local_amount: Option<f64>,
    pub usd_amount: Option<f64>,
    pub amount: Option<f64>,
    pub payment_method: Option<String>,
    pub payment_type: Option<String>,
}

impl TryFrom<ResponseRouterData<D24SyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<D24SyncResponse, Self>) -> Result<Self, Self::Error> {
        let response = item.response;
        let raw_status = response.status.as_str().to_string();
        let status = AttemptStatus::from(response.status);

        // A DECLINED deposit is reported through `status`, not by returning an
        // Err: the UCS PSync convention is that the sync response carries the
        // attempt status and the flow itself succeeded.
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                raw_connector_status: Some(RawConnectorStatus {
                    code: Some(raw_status.clone()),
                    message: Some(raw_status),
                    reason: None,
                }),
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.deposit_id.to_string()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: response.invoice_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            }),
            ..item.router_data
        })
    }
}
