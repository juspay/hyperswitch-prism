use std::ops::Deref;

use common_enums::{AttemptStatus, CountryAlpha2, Currency};
use common_utils::{pii::Email, request::Method, types::FloatMajorUnit};
use domain_types::{
    connector_flow::{Authorize, PSync, RSync, Refund},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData,
        RawConnectorStatus, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        ResponseId,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::{
        BankTransferData, CardRedirectData, CustomerDocumentDetails, DocumentKind,
        PaymentMethodData, PaymentMethodDataTypes,
    },
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
    utils,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::{connectors::d24::D24RouterData, types::ResponseRouterData};

/// The Directa24 `payment_method` codes this integration can emit.
///
/// Directa24's own catalogue of method codes is an open, per-country set
/// published on its coverage page — but that is not what this enum models. It
/// models only the codes *this connector emits*, which is a closed set:
///
/// * WebPay (Transbank, Chile) — `PaymentMethodData::CardRedirect(Webpay {})`.
/// * The Mexican and Brazilian methods whose Directa24 payment type is
///   `BANK_TRANSFER` **and** that need no `reported_info` —
///   `PaymentMethodData::BankTransfer(LocalBankTransfer { .. })`. Tech spec
///   §"Per-country bank payment-method codes": only `BANK_TRANSFER`-typed
///   methods are candidates for `LocalBankTransfer`; `BANK_DEPOSIT` methods and
///   the bank methods that require `reported_info` (BR `BB`, `B`) are excluded
///   because no UCS field carries the customer's bank account (UNDECIDED #3,
///   option a). Chile has no `BANK_TRANSFER`-typed method at all.
///
/// The wire code is the single source of truth in [`D24PaymentMethod::code`];
/// `Serialize` writes exactly that string, so WebPay still serializes as `"WP"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum D24PaymentMethod {
    /// `WP` — Chile, card redirect.
    Webpay,
    /// `SE` — SPEI, Mexico, Bank Transfer.
    Spei,
    /// `COD` — CoDi, Mexico, Bank Transfer.
    Codi,
    /// `BM` — Banamex, Mexico, Bank Transfer.
    Banamex,
    /// `STS` — Santander SuperNet, Mexico, Bank Transfer.
    SantanderSupernet,
    /// `AF` — Afirme online banking, Mexico, Bank Transfer.
    Afirme,
    /// `BQL` — Banorte online banking, Mexico, Bank Transfer.
    Banorte,
    /// `IX` — Pix, Brazil, BANK_TRANSFER.
    Pix,
    /// `I` — Itaú, Brazil, BANK_TRANSFER. Requires the payer address.
    Itau,
    /// `NU` — Nubank, Brazil, BANK_TRANSFER.
    Nubank,
    /// `ME` — MercadoPago, Brazil, BANK_TRANSFER. (`ME` is a *wallet* in
    /// Mexico, which is why codes are only ever resolved together with the
    /// billing country.)
    MercadoPago,
}

impl Serialize for D24PaymentMethod {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.code())
    }
}

impl D24PaymentMethod {
    /// The Directa24 `payment_method` code sent on `POST /v3/deposits`.
    pub fn code(self) -> &'static str {
        match self {
            Self::Webpay => "WP",
            Self::Spei => "SE",
            Self::Codi => "COD",
            Self::Banamex => "BM",
            Self::SantanderSupernet => "STS",
            Self::Afirme => "AF",
            Self::Banorte => "BQL",
            Self::Pix => "IX",
            Self::Itau => "I",
            Self::Nubank => "NU",
            Self::MercadoPago => "ME",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Webpay => "WebPay",
            Self::Spei => "SPEI",
            Self::Codi => "CoDi",
            Self::Banamex => "Banamex",
            Self::SantanderSupernet => "Santander SuperNet",
            Self::Afirme => "Afirme",
            Self::Banorte => "Banorte",
            Self::Pix => "Pix",
            Self::Itau => "Itaú",
            Self::Nubank => "Nubank",
            Self::MercadoPago => "MercadoPago",
        }
    }

    /// The single country each emitted method is offered in.
    fn country(self) -> CountryAlpha2 {
        match self {
            Self::Webpay => CountryAlpha2::CL,
            Self::Spei
            | Self::Codi
            | Self::Banamex
            | Self::SantanderSupernet
            | Self::Afirme
            | Self::Banorte => CountryAlpha2::MX,
            Self::Pix | Self::Itau | Self::Nubank | Self::MercadoPago => CountryAlpha2::BR,
        }
    }

    /// Tech spec §"Country specifications": every documented country accepts
    /// its local currency or USD (CL: CLP/USD, MX: MXN/USD, BR: BRL/USD).
    fn local_currency(self) -> Currency {
        match self.country() {
            CountryAlpha2::MX => Currency::MXN,
            CountryAlpha2::BR => Currency::BRL,
            _ => Currency::CLP,
        }
    }

    /// Tech spec §"Per-country bank payment-method codes", Brazil table:
    /// Itaú (`I`) requires street, city, state and zip_code.
    fn requires_payer_address(self) -> bool {
        matches!(self, Self::Itau)
    }

    /// WebPay is a mandatory redirect, so its three return URLs are required.
    /// Bank transfers can be completed from the instructions alone (Embedded
    /// Checkout), so the URLs are sent only when present (UNDECIDED #6, option b).
    fn requires_return_url(self) -> bool {
        match self {
            Self::Webpay => true,
            Self::Spei
            | Self::Codi
            | Self::Banamex
            | Self::SantanderSupernet
            | Self::Afirme
            | Self::Banorte
            | Self::Pix
            | Self::Itau
            | Self::Nubank
            | Self::MercadoPago => false,
        }
    }

    /// Resolves the Directa24 bank-transfer code for a `LocalBankTransfer`
    /// (UNDECIDED #1, options a + b).
    ///
    /// `LocalBankTransfer.bank_code`, when present, is read as the Directa24
    /// method code and validated against the closed per-country allow-list.
    /// When absent, the country's default is used: SPEI (`SE`) for Mexico and
    /// Pix (`IX`) for Brazil — the methods of the spec's own bank-transfer
    /// request/response examples. Anything else fails locally rather than as an
    /// opaque `401 PAYMENT_METHOD_NOT_FOUND` from Directa24.
    fn from_local_bank_transfer(
        bank_code: Option<&str>,
        country: CountryAlpha2,
    ) -> Result<Self, error_stack::Report<IntegrationError>> {
        let requested = bank_code.map(str::trim).filter(|code| !code.is_empty());

        let Some(code) = requested else {
            return match country {
                CountryAlpha2::MX => Ok(Self::Spei),
                CountryAlpha2::BR => Ok(Self::Pix),
                _ => Err(error_stack::report!(IntegrationError::NotSupported {
                    message: format!(
                        "LocalBankTransfer in billing country {country}; Directa24 bank \
                         transfer is available in Mexico (MX) and Brazil (BR) only"
                    ),
                    connector: "d24",
                    context: IntegrationErrorContext::default(),
                })),
            };
        };

        match (country, code.to_ascii_uppercase().as_str()) {
            (CountryAlpha2::MX, "SE") => Ok(Self::Spei),
            (CountryAlpha2::MX, "COD") => Ok(Self::Codi),
            (CountryAlpha2::MX, "BM") => Ok(Self::Banamex),
            (CountryAlpha2::MX, "STS") => Ok(Self::SantanderSupernet),
            (CountryAlpha2::MX, "AF") => Ok(Self::Afirme),
            (CountryAlpha2::MX, "BQL") => Ok(Self::Banorte),
            (CountryAlpha2::BR, "IX") => Ok(Self::Pix),
            (CountryAlpha2::BR, "I") => Ok(Self::Itau),
            (CountryAlpha2::BR, "NU") => Ok(Self::Nubank),
            (CountryAlpha2::BR, "ME") => Ok(Self::MercadoPago),
            _ => Err(error_stack::report!(IntegrationError::NotSupported {
                message: format!(
                    "LocalBankTransfer bank_code {code:?} in billing country {country}; \
                     supported Directa24 bank-transfer codes are MX: SE, COD, BM, STS, AF, BQL \
                     and BR: IX, I, NU, ME (BANK_DEPOSIT methods and methods that require \
                     reported_info, such as BB or B, are not supported)"
                ),
                connector: "d24",
                context: IntegrationErrorContext::default(),
            })),
        }
    }
}

impl TryFrom<&CardRedirectData> for D24PaymentMethod {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(card_redirect_data: &CardRedirectData) -> Result<Self, Self::Error> {
        match card_redirect_data {
            CardRedirectData::Webpay {} => Ok(Self::Webpay),
            CardRedirectData::Knet {}
            | CardRedirectData::Benefit {}
            | CardRedirectData::MomoAtm {}
            | CardRedirectData::CardRedirect {} => {
                Err(error_stack::report!(IntegrationError::NotImplemented(
                    "Directa24 only supports PaymentMethodData::CardRedirect(Webpay {})"
                        .to_string(),
                    IntegrationErrorContext::default(),
                )))
            }
        }
    }
}

/// The `payer.document_type` values this integration can emit.
///
/// * `Rut` — Chile (WebPay). `DocumentKind` has no `Rut` variant, so
///   `DocumentKind::Other` + a Chilean billing country is its only spelling.
/// * `Cpf` / `Cnpj` — Brazil (bank transfer). Tech spec §"Country
///   specifications": BR document type `CPF`, with `CNPJ` referenced by KYC
///   reason code 114 and by Pix `payer_document_type`.
///
/// Mexico has four document types (`CURP`, `RFC`, `IFE`, `PASS`) and
/// `DocumentKind` distinguishes none of them, so a Mexican payer is sent
/// **without** `document_type` — exactly as the spec's verbatim SPEI request
/// does (UNDECIDED #2, option b for MX / option a for CL and BR).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum D24DocumentType {
    Rut,
    Cpf,
    Cnpj,
}

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

/// `POST {base}/v3/deposits` — Directa24 "OneShot" deposit creation for the
/// WebPay (`WP`) redirect method and for local bank transfers (`SE`, `IX`, ...).
/// Field names are snake_case exactly as documented; `amount` is a JSON
/// **number in major units**.
///
/// There are **no card fields on this endpoint**: WebPay is a hosted redirect,
/// Transbank collects the card details and runs its own authentication; a bank
/// transfer is pushed by the customer from their own bank.
#[derive(Debug, Serialize)]
pub struct D24PaymentsRequest {
    pub country: CountryAlpha2,
    pub amount: FloatMajorUnit,
    pub currency: Currency,
    pub invoice_id: String,
    pub payer: D24Payer,
    /// Serializes as the Directa24 method code (`"WP"`, `"SE"`, `"IX"`, ...).
    pub payment_method: D24PaymentMethod,
    /// Always sent for WebPay: it is a mandatory redirect, and
    /// `get_router_return_url()` fails the request locally rather than letting
    /// a deposit be created that the customer cannot return from. For bank
    /// transfers they are sent only when the caller supplied a return URL.
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
    /// Always present for WebPay (`RUT`) and Brazil (`CPF`/`CNPJ`); omitted for
    /// Mexico — see [`D24DocumentType`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_type: Option<D24DocumentType>,
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

fn invalid_payer_document() -> error_stack::Report<IntegrationError> {
    error_stack::report!(IntegrationError::InvalidDataFormat {
        field_name: "payer.document",
        context: IntegrationErrorContext::default(),
    })
}

/// Maps the UCS customer document onto Directa24's `payer.document` /
/// `payer.document_type` pair for the billing country, normalising and
/// validating the number locally (tech spec §"Country specifications").
///
/// * `Other` + CL → `RUT`, mod-11 validated. `DocumentKind` carries no `Rut`
///   variant, so this is the only spelling a RUT has. WebPay-only.
/// * `Cpf` + BR → `CPF`, `Cnpj` + BR → `CNPJ`: separators stripped, check digits
///   validated ("CPF: numeric, length 11, validate verifier digits").
/// * `Other` + MX → no `document_type` (CURP/RFC/IFE/PASS cannot be told
///   apart from `DocumentKind`); the number must be alphanumeric, 7–18
///   characters.
///
/// Every other combination is rejected locally: sending it would only come back
/// as `411 INVALID_USER_DOCUMENT` or a `201` bean-validation error.
fn d24_payer_document(
    document: &CustomerDocumentDetails,
    country: CountryAlpha2,
) -> Result<(Option<D24DocumentType>, Secret<String>), error_stack::Report<IntegrationError>> {
    match (document.document_type, country) {
        (DocumentKind::Other, CountryAlpha2::CL) => Ok((
            Some(D24DocumentType::Rut),
            crate::utils::validate_and_normalize_chilean_rut(&document.document_number)?,
        )),
        (kind @ (DocumentKind::Cpf | DocumentKind::Cnpj), CountryAlpha2::BR) => {
            let digits: String = document
                .document_number
                .peek()
                .chars()
                .filter(char::is_ascii_digit)
                .collect();
            kind.validate(&digits)
                .map_err(|_| invalid_payer_document())?;
            let document_type = match kind {
                DocumentKind::Cnpj => D24DocumentType::Cnpj,
                _ => D24DocumentType::Cpf,
            };
            Ok((Some(document_type), Secret::new(digits)))
        }
        (DocumentKind::Other, CountryAlpha2::MX) => {
            let normalized = document.document_number.peek().trim().to_ascii_uppercase();
            if !(7..=18).contains(&normalized.len())
                || !normalized.chars().all(|c| c.is_ascii_alphanumeric())
            {
                return Err(invalid_payer_document());
            }
            Ok((None, Secret::new(normalized)))
        }
        (kind, country) => Err(error_stack::report!(IntegrationError::NotSupported {
            message: format!("customer document type {kind:?} for billing country {country}"),
            connector: "d24",
            context: IntegrationErrorContext::default(),
        })),
    }
}

/// `payer.id` is constrained to `^[A-Za-z0-9]*$` (max 128). UCS customer ids may
/// contain separators, so anything that does not fit is dropped — D24 then
/// autogenerates the payer id.
fn sanitize_payer_id(customer_id: Option<String>) -> Option<String> {
    customer_id.filter(|id| {
        !id.is_empty() && id.len() <= 128 && id.chars().all(|c| c.is_ascii_alphanumeric())
    })
}

/// `language` is ISO 639-1, length 2, one of `es`, `en`, `pt`, `ja` (tech spec
/// §"Request Parameters — top level"). `browser_info.language` carries a BCP 47
/// tag such as `es-MX`, so only its primary subtag is sent, and only when
/// Directa24 supports it — the field is optional, so anything else is omitted
/// rather than rejected.
fn d24_language(language: Option<String>) -> Option<String> {
    language.and_then(|tag| {
        let primary = tag.split(['-', '_']).next()?.trim().to_ascii_lowercase();
        matches!(primary.as_str(), "es" | "en" | "pt" | "ja").then_some(primary)
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
        // `Webpay` marker. No card data may reach this endpoint. The generic
        // `CardRedirect {}` marker is deliberately *not* accepted: the wire code
        // is chosen from the domain variant, so an unnamed redirect has no code.
        //
        // A local bank transfer carries no country and at most a `bank_code`,
        // so its Directa24 code is resolved together with the billing country.
        let payment_method = match &request.payment_method_data {
            PaymentMethodData::CardRedirect(card_redirect_data) => {
                D24PaymentMethod::try_from(card_redirect_data)?
            }
            PaymentMethodData::BankTransfer(bank_transfer_data) => {
                match bank_transfer_data.deref() {
                    BankTransferData::LocalBankTransfer { bank_code } => {
                        let country = *router_data
                            .resource_common_data
                            .get_billing_address()?
                            .get_country()?;
                        D24PaymentMethod::from_local_bank_transfer(bank_code.as_deref(), country)?
                    }
                    BankTransferData::AchBankTransfer {}
                    | BankTransferData::SepaBankTransfer {}
                    | BankTransferData::BacsBankTransfer {}
                    | BankTransferData::MultibancoBankTransfer {}
                    | BankTransferData::PermataBankTransfer {}
                    | BankTransferData::BcaBankTransfer {}
                    | BankTransferData::BniVaBankTransfer {}
                    | BankTransferData::BriVaBankTransfer {}
                    | BankTransferData::CimbVaBankTransfer {}
                    | BankTransferData::DanamonVaBankTransfer {}
                    | BankTransferData::MandiriVaBankTransfer {}
                    | BankTransferData::Pix { .. }
                    | BankTransferData::Pse {}
                    | BankTransferData::InstantBankTransfer {}
                    | BankTransferData::InstantBankTransferFinland {}
                    | BankTransferData::InstantBankTransferPoland {}
                    | BankTransferData::IndonesianBankTransfer { .. } => {
                        return Err(error_stack::report!(IntegrationError::NotImplemented(
                            "Directa24 only supports \
                             PaymentMethodData::BankTransfer(LocalBankTransfer { .. }) among \
                             bank transfers"
                                .to_string(),
                            IntegrationErrorContext::default(),
                        )))
                    }
                }
            }
            _ => {
                return Err(error_stack::report!(IntegrationError::NotImplemented(
                    "Directa24 only supports PaymentMethodData::CardRedirect(Webpay {}) and \
                     PaymentMethodData::BankTransfer(LocalBankTransfer { .. }) — no card data \
                     is sent to /v3/deposits"
                        .to_string(),
                    IntegrationErrorContext::default(),
                )))
            }
        };

        // --- Guard 2: billing country ----------------------------------------
        // Every emitted method is offered in exactly one country: WebPay in
        // Chile, the bank-transfer codes in Mexico or Brazil.
        let billing = router_data.resource_common_data.get_billing_address()?;
        let country = *billing.get_country()?;
        let method_country = payment_method.country();
        if country != method_country {
            return Err(error_stack::report!(IntegrationError::NotSupported {
                message: format!(
                    "{name} ({code}) in billing country {country}; {name} is available in \
                     {method_country} only",
                    name = payment_method.name(),
                    code = payment_method.code(),
                ),
                connector: "d24",
                context: IntegrationErrorContext::default(),
            }));
        }

        // --- Guard 3: currency -----------------------------------------------
        // Local currency or USD (CL: CLP/USD, MX: MXN/USD, BR: BRL/USD).
        let local_currency = payment_method.local_currency();
        if request.currency != local_currency && request.currency != Currency::USD {
            return Err(error_stack::report!(IntegrationError::NotSupported {
                message: format!(
                    "currency {} for {} ({}); only {local_currency} and USD are accepted",
                    request.currency,
                    payment_method.name(),
                    payment_method.code(),
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
        let (document_type, document_number) = d24_payer_document(&document, country)?;

        let payer_address = {
            let street = billing.get_optional_line1();
            let city = billing.get_optional_city();
            let state = billing.state.clone();
            let zip_code = billing.get_optional_zip();
            if payment_method.requires_payer_address() {
                for (present, field_name) in [
                    (street.is_some(), "billing_address.line1"),
                    (city.is_some(), "billing_address.city"),
                    (state.is_some(), "billing_address.state"),
                    (zip_code.is_some(), "billing_address.zip"),
                ] {
                    if !present {
                        return Err(error_stack::report!(
                            IntegrationError::MissingRequiredField {
                                field_name,
                                context: IntegrationErrorContext::default(),
                            }
                        ));
                    }
                }
            }
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
        //
        // WebPay: required, not optional. It is a mandatory redirect; dropping
        // the three URLs would strand the customer on Transbank with nowhere to
        // return, and HS would never receive the redirect-sync that starts
        // PSync. Failing locally is the only honest outcome.
        //
        // Bank transfer: sent when present. The customer can pay from the
        // `payment_info` instructions alone (Embedded Checkout), so a missing
        // return URL is not a reason to refuse the deposit.
        let return_url = if payment_method.requires_return_url() {
            Some(request.get_router_return_url()?)
        } else {
            request.router_return_url.clone()
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
            payment_method,
            success_url: return_url.clone(),
            back_url: return_url.clone(),
            error_url: return_url,
            client_ip: request.get_ip_address_as_optional(),
            description: router_data.resource_common_data.description.clone(),
            language: d24_language(request.get_optional_language_from_browser_info()),
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
/// documented `oneOf` differ only by the extra `payment_info` (and `iframe`),
/// so one struct with everything optional covers both.
///
/// There is no result/decline field on this response — a rejected deposit
/// arrives as an HTTP 400 (handled by `build_error_response`) or later as
/// `DECLINED` on PSync. `AttemptStatus::Charged` is therefore unreachable here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct D24PaymentsResponse {
    pub deposit_id: i64,
    pub checkout_type: Option<D24CheckoutType>,
    pub redirect_url: Option<url::Url>,
    /// Whether the checkout page may be embedded in an iframe.
    pub iframe: Option<bool>,
    pub user_id: Option<String>,
    pub merchant_invoice_id: Option<String>,
    /// ONE_SHOT only: the payment instructions for the chosen method.
    pub payment_info: Option<D24PaymentInfo>,
}

/// `payment_info` of a ONE_SHOT `201` (tech spec §"Response 201 — Created
/// (ONE_SHOT)").
///
/// Deliberately permissive — the deposit already exists by the time this is
/// parsed, and a failure here would discard its `deposit_id`:
///
/// * `type` is kept as an opaque string: the spec documents SPEI and Pix with
///   both `VOUCHER` and `BANK_TRANSFER`, so nothing may branch on it.
/// * `expiration_date` / `created_at` arrive both as `2025-07-27 19:57:40` and
///   as `2020-06-17T07:04:16Z`, so they are kept as strings.
/// * `metadata` / `secondary_metadata` are method-specific (CLABE, agency,
///   account, CNPJ, Pix QR code, digitable line, ...) and `reference` is typed
///   string but emitted as an integer, so both are kept as raw JSON.
/// * `currency` is a string so an unexpected code cannot fail the response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct D24PaymentInfo {
    #[serde(rename = "type")]
    pub payment_type: Option<String>,
    pub payment_method: Option<String>,
    pub payment_method_name: Option<String>,
    /// The exact amount the customer must pay — may differ from the requested
    /// amount when fees or surcharges are on the payer.
    pub amount: Option<FloatMajorUnit>,
    pub currency: Option<String>,
    pub expiration_date: Option<String>,
    pub created_at: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub secondary_metadata: Option<serde_json::Value>,
}

/// Serialized into `connector_metadata` (gRPC `connector_feature_data`) when a
/// ONE_SHOT deposit carries `payment_info`, so the caller can render the
/// bank-transfer instructions itself — Directa24's recommended "Embedded
/// Checkout" — instead of, or as well as, redirecting to `redirect_url`.
#[derive(Debug, Serialize)]
pub struct D24DepositInstructionsMetadata<'a> {
    pub checkout_type: Option<D24CheckoutType>,
    pub iframe: Option<bool>,
    pub payment_info: &'a D24PaymentInfo,
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
        // still has to act — authenticate on Transbank's page (WebPay), or open
        // Directa24's instructions page and push the transfer from their bank
        // (bank transfer). No URL means the deposit exists but nothing is
        // actionable yet — PSync resolves it. Bank transfers are never final
        // here: completion arrives later as `COMPLETED` on PSync.
        let (status, redirection_data) = match response.redirect_url.clone() {
            Some(url) => (
                AttemptStatus::AuthenticationPending,
                Some(RedirectForm::from((url, Method::Get))),
            ),
            None => (AttemptStatus::Pending, None),
        };

        // ONE_SHOT bank-transfer deposits carry the instructions (CLABE,
        // beneficiary, Pix QR code, exact amount, expiry) in `payment_info`.
        // They are surfaced as connector metadata; responses without
        // `payment_info` (HOSTED, WebPay) keep `connector_metadata: None`.
        let connector_metadata = response
            .payment_info
            .as_ref()
            .map(|payment_info| {
                serde_json::to_value(D24DepositInstructionsMetadata {
                    checkout_type: response.checkout_type,
                    iframe: response.iframe,
                    payment_info,
                })
                .change_context(crate::utils::response_handling_fail(
                    item.http_code,
                    "d24: failed to serialize the deposit payment_info into connector_metadata",
                ))
            })
            .transpose()?;

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(deposit_id),
                redirection_data: redirection_data.map(Box::new),
                mandate_reference: None,
                connector_metadata,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::Display)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum D24DepositStatus {
    Completed,
    Pending,
    Created,
    Declined,
    Cancelled,
    Expired,
    EarlyReleased,
    ForReview,
    /// Directa24 may add deposit statuses without notice. Without this arm an
    /// unannounced value fails the whole PSync deserialization and strands the
    /// payment at its previous status. Degrading to `Pending` keeps the poll
    /// alive and never invents a terminal state — the same rule the refund
    /// enums below follow.
    #[serde(other)]
    Unknown,
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
            // An undocumented status. Keep polling rather than guessing a
            // terminal outcome in either direction.
            D24DepositStatus::Unknown => Self::Pending,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct D24SyncResponse {
    pub deposit_id: i64,
    pub status: D24DepositStatus,
    pub invoice_id: Option<String>,
    pub user_id: Option<Secret<String>>,
    pub country: Option<String>,
    pub currency: Option<Currency>,
    pub local_amount: Option<FloatMajorUnit>,
    pub usd_amount: Option<FloatMajorUnit>,
    pub amount: Option<FloatMajorUnit>,
    pub payment_method: Option<String>,
    pub payment_type: Option<String>,
}

impl TryFrom<ResponseRouterData<D24SyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<D24SyncResponse, Self>) -> Result<Self, Self::Error> {
        let response = item.response;
        let raw_status = response.status.to_string();
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

// =============================================================================
// REFUND — POST /v3/refunds
// =============================================================================

/// `POST {base}/v3/refunds`.
///
/// `deposit_id` **and** `invoice_id` are both listed in the OpenAPI
/// `RefundRequest.required` — Directa24 binds a refund to a deposit by the pair,
/// not by the deposit id alone.
///
/// Two documented optional fields are deliberately absent:
///
/// * `notification_url` — not sent, for the same reason it is not sent on
///   `POST /v3/deposits`: `IncomingWebhook` is an empty impl, so supplying it
///   would make D24 POST refund notifications at an endpoint that cannot parse
///   them. Refund state is resolved by RSync instead.
/// * `bank_account` — documented as *"Required for refunds to bank accounts
///   (**not** for credit card refunds)"*. WebPay's D24 `payment_type` is
///   `CREDIT_CARD`, so the credit-card branch applies and no bank account
///   exists to send. This is the one assumption in this flow that could not be
///   confirmed from the documentation; if it is wrong D24 answers HTTP 400
///   `804 MISSING_BANK_ACCOUNT`, which is cheap and unambiguous. Guessing a
///   bank account instead would send money to the wrong place.
#[derive(Debug, Serialize)]
pub struct D24RefundRequest {
    /// Typed `integer` by the OpenAPI, so it is serialised unquoted.
    pub deposit_id: i64,
    /// The `invoice_id` that was sent at deposit creation.
    pub invoice_id: String,
    /// ALWAYS sent, never omitted. Directa24 reads an absent `amount` as
    /// "refund the full deposit", and it supports multiple cumulative partial
    /// refunds — so omitting it after a first partial refund would attempt the
    /// full amount again and fail with `802 INVALID_AMOUNT_TO_REFUND`. No field
    /// on the refund request carries the *remaining* refundable amount
    /// (`minor_payment_amount` is the original deposit amount, not the
    /// residual), so the requested amount is the only correct value to send.
    pub amount: FloatMajorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comments: Option<String>,
}

/// Directa24 caps `comments` at 200 characters. It is free text carrying no
/// identity and no routing meaning, so an over-long reason is truncated rather
/// than rejected — truncation cannot misdirect money, while failing the refund
/// locally over a merchant's prose would.
const D24_REFUND_COMMENTS_MAX_LEN: usize = 200;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        D24RouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for D24RefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: D24RouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;

        // `connector_transaction_id` holds exactly what Authorize wrote into
        // `ResponseId::ConnectorTransactionId` — `deposit_id.to_string()`. The
        // OpenAPI types `deposit_id` as an integer, so it is parsed back rather
        // than forwarded as a string: a quoted id would be rejected by D24's
        // bean validation as an opaque field error.
        let deposit_id = request
            .connector_transaction_id
            .parse::<i64>()
            .map_err(|_| {
                error_stack::report!(IntegrationError::InvalidDataFormat {
                    field_name: "connector_transaction_id",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Directa24's deposit_id is an integer. Set connector_transaction_id \
                             on the refund request to the deposit_id returned by \
                             POST /v3/deposits."
                                .to_string(),
                        ),
                        ..Default::default()
                    },
                })
            })?;

        // `invoice_id` is the *payment's* reference, not the refund's.
        // `RefundsData::connector_order_id` is documented as "connector-side
        // identifier for the original payment that this refund targets" and is
        // populated from the same `payment_attempt.connector_request_reference_id`
        // that Authorize turned into `invoice_id`. Running it back through
        // `sanitize_invoice_id` — which is idempotent on its own output —
        // reproduces the stored value byte for byte.
        //
        // NOT `RefundFlowData::connector_request_reference_id` and NOT
        // `RefundsData::refund_id`: both are the merchant refund id, which
        // identifies the refund rather than the payment.
        let invoice_id = sanitize_invoice_id(&request.get_connector_order_id().map_err(|_| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "connector_order_id",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Set connector_order_id on the refund request to the original payment's \
                         connector_request_reference_id — the value Directa24 stored as the \
                         deposit's invoice_id. POST /v3/refunds requires invoice_id alongside \
                         deposit_id and there is no other field on the refund request that \
                         carries the payment's reference."
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })
        })?);

        // An empty result means the original `invoice_id` was empty too, in
        // which case Directa24 autogenerated one (`merchant_invoice_id`) that is
        // not reachable from the refund context. Refuse locally rather than send
        // `invoice_id: ""` and have D24 refund against the wrong deposit or
        // reject the call opaquely.
        if invoice_id.is_empty() {
            return Err(error_stack::report!(
                IntegrationError::MissingRequiredField {
                    field_name: "connector_order_id",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "connector_order_id contains no character Directa24 accepts in an \
                             invoice_id (^[A-Za-z0-9-_]*$), so the value the deposit was created \
                             with cannot be reconstructed."
                                .to_string(),
                        ),
                        ..Default::default()
                    },
                }
            ));
        }

        let amount = utils::convert_amount(
            item.connector.amount_converter,
            request.minor_refund_amount,
            request.currency,
        )?;

        let comments = request.reason.as_ref().and_then(|reason| {
            let trimmed: String = reason.chars().take(D24_REFUND_COMMENTS_MAX_LEN).collect();
            (!trimmed.is_empty()).then_some(trimmed)
        });

        Ok(Self {
            deposit_id,
            invoice_id,
            amount,
            comments,
        })
    }
}

// =============================================================================
// REFUND RESPONSE
// =============================================================================

/// Directa24 types `refund_id` as a `string` in the OpenAPI while noting it
/// "might be an integer or a string depending on the refund method"; the create
/// example quotes it and the webhook example emits it bare. Both must parse.
///
/// `Text` is listed first because `#[serde(untagged)]` tries the variants in
/// order, and `serde_json::Number` is used rather than `i64` so that an
/// unexpectedly large or fractional id still deserialises. Its `Display` renders
/// `80000001` without a `.0` suffix, so the id round-trips into the RSync URL
/// unchanged.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum D24RefundId {
    Text(String),
    Number(serde_json::Number),
}

impl std::fmt::Display for D24RefundId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text(value) => f.write_str(value),
            Self::Number(value) => write!(f, "{value}"),
        }
    }
}

/// `refund_info.result` on the **create** response.
///
/// Unlike [`D24DepositStatus`] this enum carries a catch-all. The difference is
/// deliberate: by the time this field is read Directa24 has already accepted the
/// refund and the body carries the `refund_id`. Refusing to deserialise over an
/// unannounced result would throw away the only handle on a refund that is now
/// being processed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::Display)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum D24RefundResult {
    Success,
    InProgress,
    Rejected,
    #[serde(other)]
    Unknown,
}

impl From<D24RefundResult> for common_enums::RefundStatus {
    fn from(result: D24RefundResult) -> Self {
        match result {
            D24RefundResult::Success => Self::Success,
            D24RefundResult::InProgress => Self::Pending,
            // Reported through the status, NOT by returning an Err: the HTTP
            // response is a 200 and it carries a `refund_id`. An Err would
            // discard `connector_refund_id`, leaving a refund Directa24 knows
            // about that Hyperswitch can never poll or reconcile.
            D24RefundResult::Rejected => Self::Failure,
            // Never invent a terminal state for a value the API did not
            // announce — RSync resolves it.
            D24RefundResult::Unknown => Self::Pending,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct D24RefundInfo {
    #[serde(rename = "type")]
    pub refund_type: Option<String>,
    pub result: Option<D24RefundResult>,
    pub reason: Option<String>,
    /// Documented inconsistently (integer in one example, string in another).
    pub reason_code: Option<serde_json::Value>,
    pub payment_method: Option<String>,
    pub payment_method_name: Option<String>,
    pub amount: Option<FloatMajorUnit>,
    pub currency: Option<Currency>,
    pub created_at: Option<String>,
}

/// `200` from `POST /v3/refunds`.
///
/// Directa24 publishes a single `RefundSuccessResponse` schema, not two. It
/// carries `refund_id` plus three members the schema annotates as "Returned for
/// synchronously processed credit card refunds": `deposit_id`,
/// `merchant_invoice_id` and `refund_info`. So the two shapes people describe —
/// the synchronous card one and the bank/APM `{"refund_id": ...}` one — differ
/// only by the *presence* of those three, and `refund_id` is common to both.
/// WebPay (a card method) therefore returns `refund_id` as well; a single
/// all-optional struct covers both shapes. An `#[serde(untagged)]` enum would be
/// strictly worse: a merely missing field would silently select the wrong arm.
///
/// `refund_id` is nonetheless the one field kept required, and that is a
/// deliberate strictness beyond the schema — `RefundSuccessResponse` declares no
/// `required` array at all. `RefundsResponseData::connector_refund_id` is a
/// non-optional `String`, so an absent id could only be papered over by writing
/// the merchant refund id there, producing a refund that 404s on every
/// subsequent poll, for ever. A hard deserialisation failure is the better
/// outcome, and the raw body is still preserved on the error path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct D24RefundResponse {
    pub refund_id: D24RefundId,
    pub deposit_id: Option<i64>,
    pub merchant_invoice_id: Option<String>,
    pub refund_info: Option<D24RefundInfo>,
}

impl TryFrom<ResponseRouterData<D24RefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<D24RefundResponse, Self>) -> Result<Self, Self::Error> {
        let response = item.response;

        let result = response.refund_info.as_ref().and_then(|info| info.result);

        // No `refund_info` at all is the documented bank/APM shape, which
        // carries no result field. Directa24 has acknowledged the refund but has
        // not said it settled — defaulting to Success there would book every
        // such refund as complete on acknowledgement alone.
        let refund_status = result.map_or(common_enums::RefundStatus::Pending, Into::into);

        let raw_status = result.map(|result| result.to_string());
        let raw_reason = response
            .refund_info
            .as_ref()
            .and_then(|info| info.reason.clone());

        Ok(Self {
            resource_common_data: RefundFlowData {
                status: refund_status,
                raw_connector_status: Some(RawConnectorStatus {
                    code: raw_status.clone(),
                    message: raw_status,
                    reason: raw_reason,
                }),
                ..item.router_data.resource_common_data
            },
            response: Ok(RefundsResponseData {
                connector_refund_id: response.refund_id.to_string(),
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            ..item.router_data
        })
    }
}

// =============================================================================
// RSYNC — GET /v3/refunds/{refund_id}
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::Display)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum D24RefundSyncStatus {
    Pending,
    IncorrectDetails,
    Delivered,
    Completed,
    Rejected,
    Cancelled,
    /// The refund-status response is documented as extensible. A refund already
    /// exists at this point, so an unannounced status must not break the poll.
    #[serde(other)]
    Unknown,
}

impl From<D24RefundSyncStatus> for common_enums::RefundStatus {
    fn from(status: D24RefundSyncStatus) -> Self {
        match status {
            D24RefundSyncStatus::Pending => Self::Pending,
            // NOT Pending: "more information is needed" does not resolve on its
            // own, it waits on a human, and `ManualReview` is the status that
            // says so. Hyperswitch's refund process tracker treats Pending and
            // ManualReview identically, so nothing is lost by being precise.
            //
            // It is also a canary. This integration sends no `bank_account`
            // because WebPay is a credit-card method; if that assumption is
            // wrong, this is the status that says so.
            D24RefundSyncStatus::IncorrectDetails => Self::ManualReview,
            // TRAP: reads terminal, is not. DELIVERED means the refund has been
            // handed to the bank and can no longer be cancelled — that is a
            // statement about *our* control over it, not about the outcome. The
            // bank can still bounce it to REJECTED. Mapping it to Success would
            // tell the merchant the money is back before it is.
            D24RefundSyncStatus::Delivered => Self::Pending,
            D24RefundSyncStatus::Completed => Self::Success,
            D24RefundSyncStatus::Rejected => Self::Failure,
            // `RefundStatus` has no Cancelled variant. NOT `TransactionFailure`,
            // which asserts the underlying payment failed: the payment was fine,
            // the refund was withdrawn.
            D24RefundSyncStatus::Cancelled => Self::Failure,
            // Never invent a terminal state. NOT `RefundStatus::Unknown` — that
            // maps to the proto's Unspecified, on which Hyperswitch falls back
            // to the previously stored status, which is silently misleading.
            D24RefundSyncStatus::Unknown => Self::Pending,
        }
    }
}

/// `200` from `GET /v3/refunds/{refund_id}`.
///
/// `status` is the only field this flow needs and the only one treated as
/// required; the documentation states the response is extensible, so unknown
/// members are ignored. Note there is **no `refund_id` in the body** — the id
/// the poll was issued against is echoed back from the request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct D24RefundSyncResponse {
    pub status: D24RefundSyncStatus,
    pub deposit_id: Option<i64>,
    pub merchant_invoice_id: Option<String>,
    pub amount: Option<FloatMajorUnit>,
}

impl TryFrom<ResponseRouterData<D24RefundSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<D24RefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let raw_status = response.status.to_string();
        let refund_status = common_enums::RefundStatus::from(response.status);

        // The response body carries no refund id, so the one the request was
        // built from is the id. Cloned before the `..item.router_data` struct
        // update below moves the router data.
        let connector_refund_id = item.router_data.request.connector_refund_id.clone();

        Ok(Self {
            resource_common_data: RefundFlowData {
                status: refund_status,
                raw_connector_status: Some(RawConnectorStatus {
                    code: Some(raw_status.clone()),
                    message: Some(raw_status),
                    reason: None,
                }),
                ..item.router_data.resource_common_data
            },
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            ..item.router_data
        })
    }
}
