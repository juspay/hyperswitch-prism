use common_enums::{
    AttemptStatus, CardNetwork, CountryAlpha2, Currency, CustomerType, FrmDecision, Gender,
};
use common_utils::{pii::Email, types::FloatMajorUnit};
use domain_types::{
    connector_flow::{FrmPaymentOutcome, PostRiskCheck, PreRiskCheck, ServerAuthenticationToken},
    connector_types::{
        DomainData, HotelSegment, ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData, TravelData, TravelSegment, Traveller,
    },
    errors,
    frm::frm_types::{
        FrmFlowData, FrmPaymentOutcomeRequest, FrmPaymentOutcomeResponse, PostRiskCheckRequest,
        PostRiskCheckResponse, PreRiskCheckRequest, PreRiskCheckResponse,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_address::OrderDetailsWithAmount,
    payment_method_data::{
        BankTransferData, Card, CardWithNoCvc, CustomerDocumentDetails, DefaultPCIHolder,
        DocumentKind, PaymentMethodData, PaymentMethodDataTypes, VoucherData, WalletData,
    },
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    state_codes::convert_state_to_code,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::types::ResponseRouterData;

pub(crate) type Error = error_stack::Report<errors::IntegrationError>;
type ResponseError = error_stack::Report<errors::ConnectorError>;

// ──────────────────────────────────────────────────────────────────────────
// ClearSale FRM request metadata (camelCase JSON on `metadata`)

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearSaleFrmMetadata {
    #[serde(default, rename = "sessionId", alias = "session_id")]
    pub session_id: Option<String>,
}

impl ClearSaleFrmMetadata {
    pub fn parse(raw: Option<&Secret<String>>) -> Result<Self, Error> {
        match raw {
            None => Ok(Self::default()),
            Some(data) => serde_json::from_str(data.peek()).map_err(|err| {
                error_stack::report!(errors::IntegrationError::InvalidDataFormat {
                    field_name: "metadata",
                    context: errors::IntegrationErrorContext {
                        additional_context: Some(format!(
                            "FRM request `metadata` must be a JSON object: {err}"
                        )),
                        suggested_action: Some(
                            "Send a JSON object carrying `session_id`".to_owned(),
                        ),
                        doc_url: Some(CLEARSALE_DOC_URL.to_owned()),
                    },
                })
            }),
        }
    }
}

pub(crate) fn get_session_id_from_metadata(
    metadata: Option<&Secret<String>>,
) -> Result<String, Error> {
    ClearSaleFrmMetadata::parse(metadata)?
        .session_id
        .ok_or_else(|| missing_field_err("sessionID"))
}

/// ClearSale Total / Total Garantido API reference.
pub const CLEARSALE_DOC_URL: &str =
    "https://api.clearsale.com.br/docs/total-totalGarantido-application";

/// Default `product` value for ClearSale **Total Garantido** (spec §1).
///
/// Merchants on a different ClearSale product override it per request
/// (`connector_feature_data.product`) or account-wide on the connector config.
pub const DEFAULT_CLEARSALE_PRODUCT_TOTAL_GARANTIDO: u32 = 4;

// ──────────────────────────────────────────────────────────────────────────
// Auth
// ──────────────────────────────────────────────────────────────────────────
pub(crate) fn customer_type_from_config(raw: Option<&str>) -> Result<Option<CustomerType>, Error> {
    match raw.map(str::trim) {
        None | Some("") => Ok(None),
        Some(value)
            if value.eq_ignore_ascii_case("b2b") || value.eq_ignore_ascii_case("business") =>
        {
            Ok(Some(CustomerType::Business))
        }
        Some(value)
            if value.eq_ignore_ascii_case("b2c") || value.eq_ignore_ascii_case("individual") =>
        {
            Ok(Some(CustomerType::Individual))
        }
        Some(other) => Err(error_stack::report!(
            errors::IntegrationError::InvalidDataFormat {
                field_name: "connector_config.clearsale_br.b2b_b2c",
                context: errors::IntegrationErrorContext {
                    additional_context: Some(format!(
                        "`b2b_b2c` must be `b2b`/`business` or `b2c`/`individual`; got {other:?}"
                    )),
                    suggested_action: Some(
                        "Set connector_config.clearsale_br.b2b_b2c to `b2b` or `b2c`".to_owned(),
                    ),
                    doc_url: Some(CLEARSALE_DOC_URL.to_owned()),
                },
            }
        )),
    }
}

/// ClearSale order `b2bB2c` wire codes (spec §5.1): size-3 `b2b` / `b2c`.
fn customer_type_wire_code(customer_type: CustomerType) -> &'static str {
    match customer_type {
        CustomerType::Business => "b2b",
        CustomerType::Individual => "b2c",
    }
}

fn serialize_customer_type<S>(
    value: &Option<CustomerType>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match value {
        Some(customer_type) => serializer.serialize_str(customer_type_wire_code(*customer_type)),
        None => serializer.serialize_none(),
    }
}

#[derive(Debug, Clone)]
pub struct ClearSaleAuthType {
    pub username: Secret<String>,
    pub password: Secret<String>,
    pub b2b_b2c: Option<CustomerType>,
    pub product: Option<u32>,
}

impl TryFrom<&ConnectorSpecificConfig> for ClearSaleAuthType {
    type Error = Error;

    fn try_from(config: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match config {
            ConnectorSpecificConfig::ClearSaleBr {
                username,
                password,
                b2b_b2c,
                product,
                ..
            } => Ok(Self {
                username: username.clone(),
                password: password.clone(),
                b2b_b2c: customer_type_from_config(b2b_b2c.as_deref())?,
                product: product
                    .map(|code| u32::try_from(code).map_err(|_| {
                        error_stack::report!(
                            errors::IntegrationError::FailedToObtainAuthType {
                                context: errors::IntegrationErrorContext {
                                    additional_context: Some(format!(
                                        "ClearSale product id must be a non-negative code, got {code}"
                                    )),
                                    suggested_action: Some(
                                        "Set clearsale_br.product to a valid ClearSale product id \
                                         (e.g. 4 = Total Garantido)"
                                            .to_owned(),
                                    ),
                                    doc_url: Some(CLEARSALE_DOC_URL.to_owned()),
                                },
                            }
                        )
                    }))
                    .transpose()?,
            }),
            _ => Err(error_stack::report!(
                errors::IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext {
                        additional_context: Some(
                            "expected a ClearSaleBr connector config (username = login name, \
                             password = login password for POST /v1/authenticate)"
                                .to_owned(),
                        ),
                        suggested_action: Some(
                            "Send the ClearSale credentials as connector_config.clearsale_br \
                             with username + password"
                                .to_owned(),
                        ),
                        doc_url: Some(CLEARSALE_DOC_URL.to_owned()),
                    },
                }
            )),
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Error response
// ──────────────────────────────────────────────────────────────────────────

/// ClearSale error bodies are not formally documented; they commonly carry a
/// `Message`/`message` field. Both casings are accepted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClearSaleErrorResponse {
    #[serde(default, alias = "Message")]
    pub message: Option<String>,
}

impl ClearSaleErrorResponse {
    pub fn message(&self) -> String {
        self.message
            .clone()
            .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string())
    }
}

// ──────────────────────────────────────────────────────────────────────────
// ServerAuthenticationToken — POST /v1/authenticate (spec §3)
// ──────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ClearSaleAuthenticateRequest {
    pub name: Secret<String>,
    pub password: Secret<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::ClearSaleBrRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for ClearSaleAuthenticateRequest
{
    type Error = Error;

    fn try_from(
        item: super::ClearSaleBrRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // The framework request data carries only `grant_type`; the actual
        // login credentials come from the connector config.
        let auth = ClearSaleAuthType::try_from(&item.router_data.connector_config)?;
        Ok(Self {
            name: auth.username,
            password: auth.password,
        })
    }
}

/// Login response (spec §3). Note the PascalCase keys — verbatim from
/// ClearSale's API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClearSaleAuthenticateResponse {
    #[serde(rename = "Token")]
    pub token: Secret<String>,
    #[serde(rename = "ExpirationDate")]
    pub expiration_date: String,
}

const CLEARSALE_NAIVE_OFFSET: &str = "-03:00";

pub(crate) fn parse_clearsale_expiration_date(raw: &str) -> Result<time::OffsetDateTime, Error> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(missing_field_err("expiration_date"));
    }
    if let Ok(instant) =
        time::OffsetDateTime::parse(raw, &time::format_description::well_known::Rfc3339)
    {
        return Ok(instant);
    }
    let normalized = format!("{}{CLEARSALE_NAIVE_OFFSET}", to_clearsale_datetime(raw));
    time::OffsetDateTime::parse(&normalized, &time::format_description::well_known::Rfc3339)
        .map_err(|error| {
            error_stack::report!(errors::IntegrationError::InvalidDataFormat {
                field_name: "expiration_date",
                context: errors::IntegrationErrorContext {
                    additional_context: Some(format!(
                    "ClearSale ExpirationDate {raw:?} is not RFC 3339 or a .NET timestamp: {error}"
                )),
                    suggested_action: Some(
                        "Return ExpirationDate as an ISO-8601 timestamp (optionally with offset)"
                            .to_owned(),
                    ),
                    doc_url: Some(CLEARSALE_DOC_URL.to_owned()),
                },
            })
        })
}

/// Remaining TTL seconds for the login token. `Ok(None)` means the stamp is
/// already past `now` — a real, already-expired token, not a parse failure.
pub(crate) fn expires_in_from_expiration_date(
    expiration_date: &str,
    now: time::OffsetDateTime,
) -> Result<Option<i64>, Error> {
    let expiry = parse_clearsale_expiration_date(expiration_date)?;
    let secs = (expiry - now).whole_seconds();
    Ok((secs > 0).then_some(secs))
}

impl<F> TryFrom<ResponseRouterData<ClearSaleAuthenticateResponse, Self>>
    for RouterDataV2<
        F,
        MerchantAuthenticationFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    >
{
    type Error = ResponseError;

    fn try_from(
        item: ResponseRouterData<ClearSaleAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let expires_in = expires_in_from_expiration_date(
            &item.response.expiration_date,
            common_utils::date_time::now().assume_utc(),
        )
        .change_context(errors::ConnectorError::ResponseHandlingFailed {
            context: errors::ResponseTransformationErrorContext {
                http_status_code: None,
                additional_context: Some(
                    "ClearSale login ExpirationDate could not be parsed into a token TTL"
                        .to_owned(),
                ),
            },
        })?;
        Ok(Self {
            response: Ok(ServerAuthenticationTokenResponseData {
                access_token: item.response.token,
                token_type: Some("Bearer".to_string()),
                expires_in,
            }),
            ..item.router_data
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ClearSalePerson {
    #[serde(rename = "type")]
    pub person_type: u32,
    /// CPF (type 1) or CNPJ (type 2).
    #[serde(rename = "primaryDocument")]
    pub primary_document: Secret<String>,
    #[serde(rename = "secondaryDocument", skip_serializing_if = "Option::is_none")]
    pub secondary_document: Option<Secret<String>>,
    #[serde(rename = "clientID", skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    pub name: Secret<String>,
    #[serde(rename = "birthDate", skip_serializing_if = "Option::is_none")]
    pub birth_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<Email>,
    /// ClearSale person gender wire code (`M` / `F` / `X`) — see
    /// [`gender_wire_code`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gender: Option<&'static str>,
    pub address: ClearSaleAddress,
    pub phones: Vec<ClearSalePhone>,
    /// Shipping only: delivery type code.
    #[serde(rename = "deliveryType", skip_serializing_if = "Option::is_none")]
    pub delivery_type: Option<String>,
    /// Shipping only: delivery time description (e.g. "2 dias úteis").
    #[serde(rename = "deliveryTime", skip_serializing_if = "Option::is_none")]
    pub delivery_time: Option<String>,
    /// Shipping only: delivery price, float majors.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<FloatMajorUnit>,
    /// Shipping only: document of the pickup store.
    #[serde(
        rename = "pickUpStoreDocument",
        skip_serializing_if = "Option::is_none"
    )]
    pub pick_up_store_document: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClearSaleAddress {
    pub street: Secret<String>,
    pub number: String,
    /// Bairro.
    pub county: Secret<String>,
    pub city: Secret<String>,
    /// UF, 2 chars.
    pub state: Secret<String>,
    pub zipcode: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(
        rename = "additionalInformation",
        skip_serializing_if = "Option::is_none"
    )]
    pub additional_information: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<Secret<String>>,
}

/// `phone_type`: 1 = Home, 2 = Work, 6 = Mobile.
#[derive(Debug, Clone, Serialize)]
pub struct ClearSalePhone {
    #[serde(rename = "type")]
    pub phone_type: u32,
    /// Area code (2 digits). ClearSale expects integer, not string.
    pub ddd: u64,
    /// Phone number digits. ClearSale expects integer, not string.
    pub number: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ddi: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extension: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClearSalePayment {
    #[serde(rename = "type")]
    pub payment_type: u32,
    pub value: FloatMajorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    /// ISO-4217 numeric code (e.g. `986` for BRL).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<u32>,
    /// Payment sequence within the order (1-based).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sequential: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installments: Option<u32>,
    /// Installment interest rate (percent).
    #[serde(rename = "interestRate", skip_serializing_if = "Option::is_none")]
    pub interest_rate: Option<FloatMajorUnit>,
    #[serde(rename = "interestValue", skip_serializing_if = "Option::is_none")]
    pub interest_value: Option<FloatMajorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<ClearSaleCard>,
    #[serde(rename = "voucherOrderOrigin", skip_serializing_if = "Option::is_none")]
    pub voucher_order_origin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<ClearSaleAddress>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClearSaleCard {
    /// First 6 digits.
    pub bin: Secret<String>,
    /// Last 4 digits.
    pub end: Secret<String>,
    #[serde(rename = "ownerName")]
    pub owner_name: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<Secret<String>>,
    /// Bandeira: 1 Diners, 2 Master, 3 Visa, 5 Amex, 6 HiperCard, 10 Elo.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub card_type: Option<u32>,
    #[serde(rename = "validityDate", skip_serializing_if = "Option::is_none")]
    pub validity_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nsu: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClearSaleItem {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(rename = "barCode", skip_serializing_if = "Option::is_none")]
    pub bar_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<FloatMajorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<u16>,
    #[serde(rename = "categoryID", skip_serializing_if = "Option::is_none")]
    pub category_id: Option<u64>,
    #[serde(rename = "categoryName", skip_serializing_if = "Option::is_none")]
    pub category_name: Option<String>,
    #[serde(rename = "isGift", skip_serializing_if = "Option::is_none")]
    pub is_gift: Option<bool>,
    #[serde(rename = "sellerName", skip_serializing_if = "Option::is_none")]
    pub seller_name: Option<String>,
    #[serde(rename = "sellerDocument", skip_serializing_if = "Option::is_none")]
    pub seller_document: Option<Secret<String>>,
    #[serde(rename = "isMarketPlace", skip_serializing_if = "Option::is_none")]
    pub is_market_place: Option<String>,
    #[serde(rename = "sellerSegment", skip_serializing_if = "Option::is_none")]
    pub seller_segment: Option<String>,
    #[serde(rename = "shippingCompany", skip_serializing_if = "Option::is_none")]
    pub shipping_company: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClearSaleOrderRequest {
    pub code: String,
    /// Mandatory — from ClearSale's Behavior Analytics SDK (spec §10).
    #[serde(rename = "sessionID")]
    pub session_id: String,
    pub date: String,
    pub email: Email,
    /// ClearSale order `b2bB2c`: `b2b` / `b2c` wire codes, mapped from the
    /// shared [`CustomerType`] enum.
    #[serde(
        rename = "b2bB2c",
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_customer_type"
    )]
    pub b2b_b2c: Option<CustomerType>,
    #[serde(rename = "itemValue", skip_serializing_if = "Option::is_none")]
    pub item_value: Option<FloatMajorUnit>,
    #[serde(rename = "totalValue")]
    pub total_value: FloatMajorUnit,
    #[serde(
        rename = "numberOfInstallments",
        skip_serializing_if = "Option::is_none"
    )]
    pub number_of_installments: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    #[serde(rename = "isGift", skip_serializing_if = "Option::is_none")]
    pub is_gift: Option<bool>,
    #[serde(rename = "giftMessage", skip_serializing_if = "Option::is_none")]
    pub gift_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observation: Option<String>,
    /// 0 / 9 / 41 / 45.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    #[serde(rename = "channelID", skip_serializing_if = "Option::is_none")]
    pub channel_id: Option<String>,
    #[serde(rename = "reservationDate", skip_serializing_if = "Option::is_none")]
    pub reservation_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nationality: Option<String>,
    /// `4` = Total Garantido.
    pub product: u32,
    /// Custom SLA in seconds for the analysis decision.
    #[serde(rename = "customSla", skip_serializing_if = "Option::is_none")]
    pub custom_sla: Option<u32>,
    /// 3DS authentication result text (e.g. "Aprovado 3DS").
    #[serde(rename = "bankAuthentication", skip_serializing_if = "Option::is_none")]
    pub bank_authentication: Option<String>,
    pub billing: ClearSalePerson,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipping: Option<ClearSalePerson>,
    pub payments: Vec<ClearSalePayment>,
    pub items: Vec<ClearSaleItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list: Option<ClearSaleList>,
    #[serde(
        rename = "purchaseInformation",
        skip_serializing_if = "Option::is_none"
    )]
    pub purchase_information: Option<ClearSalePurchaseInformation>,
    #[serde(rename = "socialNetwork", skip_serializing_if = "Option::is_none")]
    pub social_network: Option<ClearSaleSocialNetwork>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passengers: Option<Vec<ClearSalePassenger>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connections: Option<Vec<ClearSaleConnection>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hotels: Option<Vec<ClearSaleHotel>>,
}

/// Order `list` block (spec §5.1): customer list membership.
#[derive(Debug, Clone, Serialize)]
pub struct ClearSaleList {
    #[serde(rename = "typeID")]
    pub type_id: u32,
    pub id: String,
}

/// Order `purchaseInformation` block (spec §5.1): account-history signals.
#[derive(Debug, Clone, Serialize)]
pub struct ClearSalePurchaseInformation {
    #[serde(
        rename = "lastDateInsertedMail",
        skip_serializing_if = "Option::is_none"
    )]
    pub last_date_inserted_mail: Option<String>,
    #[serde(
        rename = "lastDateChangePassword",
        skip_serializing_if = "Option::is_none"
    )]
    pub last_date_change_password: Option<String>,
    #[serde(
        rename = "lastDateChangePhone",
        skip_serializing_if = "Option::is_none"
    )]
    pub last_date_change_phone: Option<String>,
    #[serde(
        rename = "lastDateChangeMobilePhone",
        skip_serializing_if = "Option::is_none"
    )]
    pub last_date_change_mobile_phone: Option<String>,
    #[serde(
        rename = "lastDateInsertedAddress",
        skip_serializing_if = "Option::is_none"
    )]
    pub last_date_inserted_address: Option<String>,
    #[serde(rename = "purchaseLogged", skip_serializing_if = "Option::is_none")]
    pub purchase_logged: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<Email>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login: Option<String>,
}

/// Order `socialNetwork` block (spec §5.1).
#[derive(Debug, Clone, Serialize)]
pub struct ClearSaleSocialNetwork {
    #[serde(rename = "optInCompreConfie", skip_serializing_if = "Option::is_none")]
    pub opt_in_compre_confie: Option<u32>,
    #[serde(rename = "typeSocialNetwork", skip_serializing_if = "Option::is_none")]
    pub type_social_network: Option<u32>,
    #[serde(
        rename = "authenticationToken",
        skip_serializing_if = "Option::is_none"
    )]
    pub authentication_token: Option<Secret<String>>,
}

/// Order `passengers[]` block (spec §5.1) — bus/air travel orders.
#[derive(Debug, Clone, Serialize)]
pub struct ClearSalePassenger {
    pub name: Secret<String>,
    #[serde(rename = "companyMileCard", skip_serializing_if = "Option::is_none")]
    pub company_mile_card: Option<String>,
    #[serde(rename = "mileCard", skip_serializing_if = "Option::is_none")]
    pub mile_card: Option<Secret<String>>,
    /// ID type per spec: 1=CPF, 2=CNPJ, 3=RG, 4=IE, 5=Passport, 6=CTPS, 7=Voter ID.
    /// Required only when identificationNumber is present.
    #[serde(rename = "identificationType", skip_serializing_if = "Option::is_none")]
    pub identification_type: Option<u32>,
    #[serde(
        rename = "identificationNumber",
        skip_serializing_if = "Option::is_none"
    )]
    pub identification_number: Option<Secret<String>>,
    /// ClearSale passenger gender wire code (`M` / `F` / `X`) — see
    /// [`gender_wire_code`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gender: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub birthdate: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpf: Option<Secret<String>>,
}

/// Order `connections[]` block (spec §5.1) — travel legs.
#[derive(Debug, Clone, Serialize)]
pub struct ClearSaleConnection {
    pub company: String,
    #[serde(
        rename = "identificationNumber",
        skip_serializing_if = "Option::is_none"
    )]
    pub identification_number: Option<u64>,
    /// Departure date (required per spec); fall back from boarding if needed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(rename = "seatClass", skip_serializing_if = "Option::is_none")]
    pub seat_class: Option<String>,
    pub origin: String,
    pub destination: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boarding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arriving: Option<String>,
    #[serde(rename = "fareClass", skip_serializing_if = "Option::is_none")]
    pub fare_class: Option<String>,
}

/// Order `hotels[]` block (spec §5.1).
#[derive(Debug, Clone, Serialize)]
pub struct ClearSaleHotel {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(rename = "reservationDate", skip_serializing_if = "Option::is_none")]
    pub reservation_date: Option<String>,
    #[serde(
        rename = "reserveExpirationDate",
        skip_serializing_if = "Option::is_none"
    )]
    pub reserve_expiration_date: Option<String>,
    #[serde(rename = "checkInDate", skip_serializing_if = "Option::is_none")]
    pub check_in_date: Option<String>,
    #[serde(rename = "checkOutDate", skip_serializing_if = "Option::is_none")]
    pub check_out_date: Option<String>,
}

/// Request body for both `POST /v1/orders/` (PreRiskCheck) and
#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct ClearSaleOrderCreateRequest {
    pub order: ClearSaleOrderRequest,
}

/// PostRiskCheck (PUT /v1/orders/ re-analysis) wire shape is identical to the

#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct ClearSaleOrderReanalysisRequest(pub ClearSaleOrderCreateRequest);

// ──────────────────────────────────────────────────────────────────────────
// Order response + status codes (spec §5.2, §6)
// ──────────────────────────────────────────────────────────────────────────

/// ClearSale 3-letter order status codes (spec §6).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ClearSaleStatusEnum {
    /// Novo.
    #[serde(rename = "NVO", alias = "Nvo")]
    Nvo,
    /// Análise manual.
    #[serde(rename = "AMA", alias = "Ama")]
    Ama,
    /// Suspensão manual.
    #[serde(rename = "SUS", alias = "Sus")]
    Sus,
    /// Aprovação automática.
    #[serde(rename = "APA", alias = "Apa")]
    Apa,
    /// Aprovação manual.
    #[serde(rename = "APM", alias = "Apm")]
    Apm,
    /// Aprovação por política.
    #[serde(rename = "APP", alias = "App")]
    App,
    /// Aguardando pagamento.
    #[serde(rename = "APG", alias = "Apg")]
    Apg,
    /// Reprovação automática.
    #[serde(rename = "RPA", alias = "Rpa")]
    Rpa,
    /// Reprovado sem suspeita.
    #[serde(rename = "RPM", alias = "Rpm")]
    Rpm,
    /// Reprovação por política.
    #[serde(rename = "RPP", alias = "Rpp")]
    Rpp,
    /// Cancelado pelo cliente.
    #[serde(rename = "CAN", alias = "Can")]
    Can,
    /// Fraude confirmada.
    #[serde(rename = "FRD", alias = "Frd")]
    Frd,
    /// Legacy rule-engine statuses, kept so deserialization does not crash.
    #[serde(rename = "AMM", alias = "Amm")]
    Amm,
    #[serde(rename = "APR", alias = "Apr")]
    Apr,
    #[serde(rename = "REA", alias = "Rea")]
    Rea,
    #[serde(rename = "REP", alias = "Rep")]
    Rep,
    /// Processing error.
    #[serde(rename = "ERR", alias = "Err")]
    Err,
    #[serde(other)]
    Unknown,
}

impl ClearSaleStatusEnum {
    /// Map a ClearSale status onto the UCS FRM decision (spec §6).
    pub fn to_frm_decision(&self) -> FrmDecision {
        match self {
            Self::Apa | Self::Apm | Self::App | Self::Apg => FrmDecision::Approve,
            Self::Nvo
            | Self::Ama
            | Self::Sus
            | Self::Err
            | Self::Amm
            | Self::Apr
            | Self::Rea
            | Self::Rep
            | Self::Unknown => FrmDecision::Review,
            Self::Rpa | Self::Rpm | Self::Rpp | Self::Can | Self::Frd => FrmDecision::Reject,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClearSaleOrderStatusResponse {
    pub code: String,
    pub status: ClearSaleStatusEnum,
    #[serde(default)]
    pub score: Option<f64>,
}

/// `POST /v1/orders/` response (spec §5.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearSaleOrderCreateResponse {
    #[serde(rename = "packageID")]
    pub package_id: String,
    #[serde(default)]
    pub orders: Vec<ClearSaleOrderStatusResponse>,
}

/// Re-analysis response — same wire shape as the create response, distinct
/// type so the macros can template it separately.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ClearSaleOrderReanalysisResponse(pub ClearSaleOrderCreateResponse);

// ──────────────────────────────────────────────────────────────────────────
// Status push — PUT /v1/orders/{code}/status (spec §5.4)
// ──────────────────────────────────────────────────────────────────────────

/// Body `{"status": "PGA"}` (payment approved) or `{"status": "PGR"}`
/// (payment denied).
#[derive(Debug, Clone, Serialize)]
pub struct ClearSaleStatusUpdateRequest {
    pub status: String,
}

/// `PUT /v1/orders/{code}/status` response `{"status": "OK", "message": ...}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClearSaleStatusUpdateResponse {
    pub status: String,
    #[serde(default)]
    pub message: Option<String>,
}

// ──────────────────────────────────────────────────────────────────────────
// Webhook (spec §5.5)
// ──────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClearSaleWebhookPayload {
    pub code: String,
    pub date: String,
    #[serde(rename = "type")]
    pub event_type: String,
}

// ──────────────────────────────────────────────────────────────────────────
// Request construction — PreRiskCheck / PostRiskCheck
// ──────────────────────────────────────────────────────────────────────────

pub(crate) const CLEARSALE_PERSON_TYPE_PF: u32 = 1;
/// `type` for Pessoa Jurídica (legal entity) — spec §5.1. Mapped from
/// `DocumentKind::Cnpj`.
pub(crate) const CLEARSALE_PERSON_TYPE_PJ: u32 = 2;
pub(crate) const CLEARSALE_PAYMENT_TYPE_CREDIT_CARD: u32 = 1;
pub(crate) const CLEARSALE_PAYMENT_TYPE_BANK_SLIP: u32 = 2;
pub(crate) const CLEARSALE_PAYMENT_TYPE_BANK_DEBIT: u32 = 3;
pub(crate) const CLEARSALE_PAYMENT_TYPE_BANK_TRANSFER: u32 = 6;
pub(crate) const CLEARSALE_PAYMENT_TYPE_VOUCHER: u32 = 16;
pub(crate) const CLEARSALE_PAYMENT_TYPE_DEBIT_CARD: u32 = 19;
pub(crate) const CLEARSALE_PAYMENT_TYPE_PIX: u32 = 27;
pub(crate) const CLEARSALE_PAYMENT_TYPE_CASHBACK: u32 = 28;
pub(crate) const CLEARSALE_PAYMENT_TYPE_VIRTUAL_GIFT_CARD: u32 = 1041;
pub(crate) const CLEARSALE_PHONE_TYPE_HOME: u32 = 1;
pub(crate) fn missing_field_err(field_name: &'static str) -> Error {
    error_stack::report!(errors::IntegrationError::MissingRequiredField {
        field_name,
        context: errors::IntegrationErrorContext {
            additional_context: Some(format!(
                "ClearSale requires `{field_name}` on {CLEARSALE_DOC_URL} order bodies; the \
                 UCS FRM request did not carry a value for it"
            )),
            suggested_action: Some(
                "Send the field via the FRM request or connector_feature_data".to_owned(),
            ),
            doc_url: Some(CLEARSALE_DOC_URL.to_owned()),
        },
    })
}

pub(crate) fn clearsale_currency_code(currency: Currency) -> Result<u32, Error> {
    match currency {
        Currency::BRL => Ok(986),
        other => Err(error_stack::report!(
            errors::IntegrationError::CurrencyNotSupported {
                message: format!(
                    "ClearSale FRM BR screens BRL (ISO 4217 986) only; got {other}. \
                     Sending another currency would misreport the amount as reais"
                ),
                connector: "clearsale_br",
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Convert the order to BRL before the ClearSale risk check".to_owned(),
                    ),
                    suggested_action: Some(
                        "Send amount.currency = BRL on the FRM risk-check request".to_owned(),
                    ),
                    doc_url: Some(CLEARSALE_DOC_URL.to_owned()),
                },
            }
        )),
    }
}

pub(crate) struct OrderInputs<'a> {
    code: String,
    session_id: String,
    feature_data: &'a ClearSaleFeatureData,
    b2b_b2c: Option<CustomerType>,
    config_product: Option<u32>,
    email: Email,
    money: common_utils::types::Money,
    ip: Option<String>,
    customer_info: Option<&'a domain_types::connector_types::CustomerInfo>,
    address: Option<&'a domain_types::payment_address::PaymentAddress>,
    payment_method: Option<&'a PaymentMethodData<DefaultPCIHolder>>,
    order_details: Option<&'a Vec<OrderDetailsWithAmount>>,
    /// Typed billing document — CPF/CNPJ number and kind (`billing.person_type`).
    billing_document_data: Option<&'a CustomerDocumentDetails>,
    /// Typed shipping document. Falls back to the billing document when absent.
    shipping_document_data: Option<&'a CustomerDocumentDetails>,
    travel_data: ClearSaleTravelData<'a>,
}

impl<'a> OrderInputs<'a> {
    pub(crate) fn from_pre_risk_check(
        req: &'a PreRiskCheckRequest,
        feature_data: &'a ClearSaleFeatureData,
        b2b_b2c: Option<CustomerType>,
        config_product: Option<u32>,
    ) -> Result<Self, Error> {
        let travel_data = ClearSaleTravelData::from_domain_data(req.domain_data.as_ref());
        Ok(Self {
            code: req
                .merchant_transaction_id
                .clone()
                .ok_or_else(|| missing_field_err("merchant_transaction_id"))?,
            session_id: get_session_id_from_metadata(req.metadata.as_ref())?,
            feature_data,
            b2b_b2c,
            config_product,
            travel_data,
            email: req
                .customer_info
                .as_ref()
                .ok_or_else(|| missing_field_err("email"))?
                .get_email()?,
            money: req.amount.clone(),
            ip: req
                .browser_info
                .as_ref()
                .and_then(|info| info.ip_address.map(|ip_addr| ip_addr.to_string())),
            customer_info: req.customer_info.as_ref(),
            address: req.address.as_ref(),
            payment_method: req.payment_method.as_ref(),
            order_details: req.order_details.as_ref(),
            billing_document_data: req.billing_document_data.as_ref(),
            shipping_document_data: req.shipping_document_data.as_ref(),
        })
    }

    pub(crate) fn from_post_risk_check(
        req: &'a PostRiskCheckRequest,
        feature_data: &'a ClearSaleFeatureData,
        b2b_b2c: Option<CustomerType>,
        config_product: Option<u32>,
    ) -> Result<Self, Error> {
        let travel_data = ClearSaleTravelData::from_domain_data(req.domain_data.as_ref());
        Ok(Self {
            code: req
                .merchant_transaction_id
                .clone()
                .ok_or_else(|| missing_field_err("merchant_transaction_id"))?,
            session_id: get_session_id_from_metadata(req.metadata.as_ref())?,
            feature_data,
            b2b_b2c,
            config_product,
            travel_data,
            email: req
                .customer_info
                .as_ref()
                .ok_or_else(|| missing_field_err("email"))?
                .get_email()?,
            money: req.amount.clone(),
            ip: req
                .browser_info
                .as_ref()
                .and_then(|info| info.ip_address.map(|ip_addr| ip_addr.to_string())),
            customer_info: req.customer_info.as_ref(),
            address: req.address.as_ref(),
            payment_method: req.payment_method.as_ref(),
            order_details: req.order_details.as_ref(),
            billing_document_data: req.billing_document_data.as_ref(),
            shipping_document_data: req.shipping_document_data.as_ref(),
        })
    }
}

fn is_house_number_token(token: &str) -> bool {
    let token = token.trim();
    if token.is_empty() || token.chars().count() > 14 {
        return false;
    }
    let lower = token.to_ascii_lowercase();
    if matches!(lower.as_str(), "s/n" | "sn" | "s.n" | "s.n.") {
        return true;
    }
    let Some(first) = token.chars().next() else {
        return false;
    };
    first.is_ascii_digit()
        && token.chars().all(|c| {
            c.is_ascii_digit()
                || c.is_ascii_alphabetic()
                || matches!(c, '-' | '/' | '.' | 'º' | 'ª' | '°')
        })
}

pub(crate) const CLEARSALE_ADDRESS_NUMBER_SEM_NUMERO: &str = "s/n";

pub(crate) fn split_street_and_number(
    label: &str,
    line1: Option<&Secret<String>>,
    line3: Option<&Secret<String>>,
) -> Result<(Secret<String>, String, Option<String>), Error> {
    let (err_number, err_street) = match label {
        "billing" => ("billing.address.number", "billing.address.street"),
        _ => ("shipping.address.number", "shipping.address.street"),
    };
    let line1 = line1
        .map(|value| value.peek().trim().to_owned())
        .filter(|value| !value.is_empty());
    let line3_trimmed = line3
        .map(|value| value.peek().trim().to_owned())
        .filter(|value| !value.is_empty());
    // No street at all: a bare `line3` number cannot carry the address.
    let Some(line1) = line1 else {
        return Err(missing_field_err(err_street));
    };

    let parts: Vec<&str> = line1
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    if let Some((street_part, rest_parts)) = parts.split_first() {
        if let Some(number) = rest_parts.iter().find_map(|segment| {
            let first = segment.split([' ', '/']).next().unwrap_or("");
            is_house_number_token(first).then_some(first)
        }) {
            return Ok((
                Secret::new((*street_part).to_owned()),
                number.to_owned(),
                line3_trimmed,
            ));
        }
    }

    if let Some((first, rest)) = line1.split_once(' ') {
        let first = first.trim();
        let rest = rest.trim();
        if is_house_number_token(first) && !rest.is_empty() {
            return Ok((
                Secret::new(rest.to_owned()),
                first.to_owned(),
                line3_trimmed,
            ));
        }
    }

    let tokens: Vec<&str> = line1
        .split([',', ' '])
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .collect();
    if let Some(pos) = tokens
        .iter()
        .rposition(|token| is_house_number_token(token))
    {
        let Some(number) = tokens.get(pos).map(|token| (*token).to_owned()) else {
            return Err(missing_field_err(err_number));
        };
        let street = tokens.get(..pos).unwrap_or_default().join(" ");
        if street.is_empty() {
            // line1 was nothing but a number — send it as `number` only when
            // `line3` can supply the street name.
            return match line3_trimmed {
                Some(street) => Ok((Secret::new(street), number.to_owned(), None)),
                None => Err(missing_field_err(err_street)),
            };
        }
        return Ok((Secret::new(street), number.to_owned(), line3_trimmed));
    }

    // 4. Bare number in line3.
    if let Some(number) = line3_trimmed
        .as_deref()
        .filter(|value| is_house_number_token(value))
        .map(ToOwned::to_owned)
    {
        return Ok((Secret::new(line1), number, None));
    }

    // 5. Street present, no house number anywhere — *sem número*.
    Ok((
        Secret::new(line1),
        CLEARSALE_ADDRESS_NUMBER_SEM_NUMERO.to_owned(),
        line3_trimmed,
    ))
}

pub(crate) fn build_clear_sale_address(
    label: &str,
    address_details: Option<&domain_types::payment_address::AddressDetails>,
) -> Result<ClearSaleAddress, Error> {
    let (street, number, additional_line) = split_street_and_number(
        label,
        address_details.and_then(|details| details.line1.as_ref()),
        address_details.and_then(|details| details.line3.as_ref()),
    )?;
    let additional_information = additional_line.map(Secret::new);
    let (n_county, n_city, n_state, n_zipcode) = match label {
        "billing" => (
            "billing.address.county",
            "billing.address.city",
            "billing.address.state",
            "billing.address.zipcode",
        ),
        _ => (
            "shipping.address.county",
            "shipping.address.city",
            "shipping.address.state",
            "shipping.address.zipcode",
        ),
    };
    let required_value = |field_name: &'static str,
                          value: Option<&Secret<String>>|
     -> Result<Secret<String>, Error> {
        value
            .filter(|value| !value.peek().trim().is_empty())
            .cloned()
            .ok_or_else(|| missing_field_err(field_name))
    };
    let county = required_value(n_county, address_details.and_then(|d| d.line2.as_ref()))?;
    let city = required_value(n_city, address_details.and_then(|d| d.city.as_ref()))?;
    let state_raw = required_value(n_state, address_details.and_then(|d| d.state.as_ref()))?;
    let zipcode = required_value(n_zipcode, address_details.and_then(|d| d.zip.as_ref()))?;
    // Spec §5.1: `state` is the 2-char UF. `"Acre"` must go on the wire as `AC`.
    let state = clearsale_state_uf(label, state_raw.peek())?;
    Ok(ClearSaleAddress {
        street,
        number,
        county,
        city,
        state,
        zipcode,
        country: address_details.and_then(|details| details.country.map(clearsale_country_name)),
        additional_information,
        reference: None,
    })
}

/// Split a Brazilian phone number into `(ddi, ddd, number)`.
///
/// When `country_code` is present (e.g. `+55`) it becomes `ddi` and the first
/// two digits of the remaining national number are the DDD area code. When
/// `country_code` is **absent** and the number is E.164-shaped (`55…` with
/// 12–13 digits), a leading `55` is stripped first so
/// `5511987654321` does not become DDD 55.
///
/// Unparsable `ddd`/`number` produce a missing-field error — never `0`.
pub(crate) fn split_phone(
    number: &str,
    country_code: Option<&str>,
    phone_type: u32,
) -> Result<ClearSalePhone, Error> {
    let digits: String = number.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return Err(missing_field_err("phone.number"));
    }
    let code = country_code
        .map(|code| code.trim_start_matches('+').trim().to_owned())
        .filter(|code| !code.is_empty());
    let (ddi, national) = match &code {
        Some(code) => {
            // A full E.164 national payload may still carry the country-code
            // prefix (`+55 11 98765-4321` → digits `5511987654321`). Strip it
            // so `ddd` is the area code, not the DDI.
            let national = if digits.len() >= 12 && digits.starts_with(code.as_str()) {
                digits[code.len()..].to_owned()
            } else {
                digits.clone()
            };
            (code.parse::<u64>().ok(), national)
        }
        None => {
            // E.164 without an explicit country_code: `55` + DDD + number.
            let national = if digits.len() >= 12 && digits.starts_with("55") {
                digits[2..].to_owned()
            } else {
                digits.clone()
            };
            (None, national)
        }
    };
    if national.len() < 2 {
        return Err(missing_field_err("phone.ddd"));
    }
    let (ddd_str, local_str) = national.split_at(2);
    let ddd = ddd_str
        .parse::<u64>()
        .map_err(|_| missing_field_err("phone.ddd"))?;
    let number = local_str
        .parse::<u64>()
        .map_err(|_| missing_field_err("phone.number"))?;
    if ddd == 0 || number == 0 {
        return Err(missing_field_err("phone.number"));
    }
    Ok(ClearSalePhone {
        phone_type,
        ddd,
        number,
        ddi,
        extension: None,
    })
}

/// Collect the contact phone for one person block, preferring the address
/// phone and falling back to the customer-level phone so the required
/// `phones` array never goes out empty when any number exists anywhere.
pub(crate) fn collect_phones(
    address: Option<&domain_types::payment_address::Address>,
    customer_info: Option<&domain_types::connector_types::CustomerInfo>,
    phone_type: u32,
) -> Result<Vec<ClearSalePhone>, Error> {
    let from_address = address
        .and_then(|addr| addr.phone.as_ref())
        .and_then(|phone| phone.number.as_ref().map(|number| (number, phone)))
        .map(|(number, phone)| {
            split_phone(number.peek(), phone.country_code.as_deref(), phone_type)
        })
        .transpose()?;
    let from_customer = customer_info
        .and_then(|customer| {
            customer
                .customer_phone_number
                .as_ref()
                .map(|number| (number, customer))
        })
        .map(|(number, customer)| {
            split_phone(
                number.peek(),
                customer.customer_phone_country_code.as_deref(),
                phone_type,
            )
        })
        .transpose()?;
    match (from_address, from_customer) {
        (Some(phone), _) | (None, Some(phone)) => Ok(vec![phone]),
        (None, None) => Ok(vec![]),
    }
}

pub(crate) fn person_name(
    address_details: Option<&domain_types::payment_address::AddressDetails>,
    customer_info: Option<&domain_types::connector_types::CustomerInfo>,
) -> Option<Secret<String>> {
    customer_info
        .and_then(|customer| customer.get_full_name())
        .or_else(|| address_details.and_then(|details| details.get_optional_full_name()))
}

pub(crate) fn primary_document(document: Option<&Secret<String>>) -> Option<Secret<String>> {
    document
        .filter(|doc| !doc.peek().trim().is_empty())
        .cloned()
}

/// Build one `billing`/`shipping` person block. Required fields fail at the
/// access site via [`missing_field_err`] — nothing is batched or deferred.
pub(crate) fn build_person(
    label: &str,
    address: Option<&domain_types::payment_address::Address>,
    customer_info: Option<&domain_types::connector_types::CustomerInfo>,
    document: Option<&Secret<String>>,
    document_kind: Option<&DocumentKind>,
    feature_data: &ClearSaleFeatureData,
) -> Result<ClearSalePerson, Error> {
    let address_details = address.and_then(|addr| addr.address.as_ref());
    let primary_document = primary_document(document).ok_or_else(|| {
        missing_field_err(match label {
            "billing" => "billing.primaryDocument",
            _ => "shipping.primaryDocument",
        })
    })?;
    // person_type maps from the typed DocumentKind: CPF → 1 (Pessoa Física),
    // CNPJ → 2 (Pessoa Jurídica). Psn/Other have no ClearSale person type and
    // are rejected rather than sent as CPF.
    let person_type = match document_kind {
        Some(DocumentKind::Cpf) | None => CLEARSALE_PERSON_TYPE_PF,
        Some(DocumentKind::Cnpj) => CLEARSALE_PERSON_TYPE_PJ,
        Some(DocumentKind::Psn) | Some(DocumentKind::Other) => {
            return Err(missing_field_err(match label {
                "billing" => "billing.document_type",
                _ => "shipping.document_type",
            }));
        }
    };
    let name = person_name(address_details, customer_info).ok_or_else(|| {
        missing_field_err(match label {
            "billing" => "billing.name",
            _ => "shipping.name",
        })
    })?;
    let phone_type = match label {
        "billing" => feature_data
            .billing_phone_type
            .unwrap_or(CLEARSALE_PHONE_TYPE_HOME),
        _ => feature_data
            .shipping_phone_type
            .unwrap_or(CLEARSALE_PHONE_TYPE_HOME),
    };
    let phones = collect_phones(address, customer_info, phone_type)?;
    if phones.is_empty() {
        return Err(missing_field_err(match label {
            "billing" => "billing.phones[0]",
            _ => "shipping.phones[0]",
        }));
    }
    let address = build_clear_sale_address(label, address_details)?;
    Ok(ClearSalePerson {
        person_type,
        primary_document,
        // UCS carries no secondary document; merchants who track one can pass
        // it back once a first-class carrier exists.
        secondary_document: None,
        client_id: customer_info.and_then(|customer| {
            customer
                .customer_id
                .as_ref()
                .map(|id| id.get_string_repr().to_owned())
        }),
        name,
        birth_date: customer_info.and_then(|customer| {
            customer
                .date_of_birth
                .as_ref()
                .map(|date| to_clearsale_datetime(&date.peek().to_string()))
        }),
        email: customer_info.and_then(|customer| customer.customer_email.clone()),
        gender: None,
        address,
        phones,
        delivery_type: None,
        delivery_time: None,
        price: None,
        pick_up_store_document: None,
    })
}

/// ClearSale bandeira int for a card network (spec §5.1). Networks with no
/// ClearSale code return `None` and the `card.type` field is simply omitted.
pub(crate) fn clearsale_card_type(network: Option<&CardNetwork>) -> Option<u32> {
    match network? {
        CardNetwork::DinersClub => Some(1),
        CardNetwork::Mastercard => Some(2),
        CardNetwork::Visa => Some(3),
        CardNetwork::AmericanExpress => Some(5),
        _ => None,
    }
}

/// ClearSale `validityDate` is `MM/yyyy`.
pub(crate) fn card_validity_date(card: &Card<DefaultPCIHolder>) -> String {
    validity_date_from(&card.card_exp_month, &card.get_expiry_year_4_digit())
}

/// `validityDate` from an expiry month + 4-digit year (shared by `Card` and
/// `CardWithNoCvc`, which cannot share a match-arm binding).
fn validity_date_from(month: &Secret<String>, year4: &Secret<String>) -> String {
    format!("{}/{}", month.peek(), year4.peek())
}

pub(crate) fn to_clearsale_datetime(raw: &str) -> String {
    let raw = raw.trim();
    // Date-only → midnight (matches the `birthDate` example in the field docs).
    if raw.len() == 10
        && raw.as_bytes().get(4) == Some(&b'-')
        && raw.as_bytes().get(7) == Some(&b'-')
        && !raw.contains('T')
    {
        return format!("{raw}T00:00:00.000");
    }
    let body = match raw.rfind(['Z', 'z']) {
        Some(i) if i >= 19 => raw.get(..i).unwrap_or(raw),
        _ => {
            let after = raw.get(19..).unwrap_or("");
            match after.find(['+', '-']) {
                Some(rel) => raw.get(..19 + rel).unwrap_or(raw),
                None => raw,
            }
        }
    };
    match body.find('.') {
        Some(dot) => {
            let (head, frac) = body.split_at(dot);
            let frac_digits: String = frac[1..]
                .chars()
                .filter(|c| c.is_ascii_digit())
                .take(7)
                .collect();
            if frac_digits.is_empty() {
                head.to_owned()
            } else {
                format!("{head}.{frac_digits}")
            }
        }
        None => body.to_owned(),
    }
}

/// ClearSale `country` / `address.country` use the documented **name**
/// vocabulary (`"Brasil"`), not ISO alpha-2 (`"BR"`). Unknown codes fall
/// back to the alpha-2 so a non-BR merchant is still represented.
pub(crate) fn clearsale_country_name(country: CountryAlpha2) -> String {
    match country {
        CountryAlpha2::BR => "Brasil".to_owned(),
        other => other.to_string(),
    }
}

pub(crate) fn clearsale_state_uf(label: &str, state: &str) -> Result<Secret<String>, Error> {
    let trimmed = state.trim();
    if trimmed.len() == 2 && trimmed.chars().all(|c| c.is_ascii_alphabetic()) {
        return Ok(Secret::new(trimmed.to_uppercase()));
    }
    convert_state_to_code(CountryAlpha2::BR, trimmed)
        .map(|code| Secret::new(code.to_owned()))
        .ok_or_else(|| {
            let field_name = match label {
                "billing" => "billing.address.state",
                _ => "shipping.address.state",
            };
            error_stack::report!(errors::IntegrationError::InvalidDataFormat {
                field_name,
                context: errors::IntegrationErrorContext {
                    additional_context: Some(format!(
                        "ClearSale address.state must be a 2-char UF (e.g. SP); got {trimmed:?}"
                    )),
                    suggested_action: Some(
                        "Send a UF code (SP, RJ, AC, …) or a Brazilian state name".to_owned(),
                    ),
                    doc_url: Some(CLEARSALE_DOC_URL.to_owned()),
                },
            })
        })
}

/// Same for `CardWithNoCvc` (which has no CVC but the same PAN/expiry fields).
pub(crate) fn card_with_no_cvc_validity_date(card: &CardWithNoCvc) -> String {
    validity_date_from(&card.card_exp_month, &card.get_expiry_year_4_digit())
}

pub(crate) fn clearsale_payment_type(
    payment_method: &PaymentMethodData<DefaultPCIHolder>,
) -> Result<u32, Error> {
    match payment_method {
        PaymentMethodData::Card(card) => {
            let is_debit = card
                .card_type
                .as_deref()
                .is_some_and(|kind| kind.eq_ignore_ascii_case("debit"));
            Ok(if is_debit {
                CLEARSALE_PAYMENT_TYPE_DEBIT_CARD
            } else {
                CLEARSALE_PAYMENT_TYPE_CREDIT_CARD
            })
        }
        PaymentMethodData::CardWithNoCvc(card) => {
            let is_debit = card
                .card_type
                .as_deref()
                .is_some_and(|kind| kind.eq_ignore_ascii_case("debit"));
            Ok(if is_debit {
                CLEARSALE_PAYMENT_TYPE_DEBIT_CARD
            } else {
                CLEARSALE_PAYMENT_TYPE_CREDIT_CARD
            })
        }
        // Wallet PAN/tokenised card rails share the card code, but the wire
        // `payments[].card` block is only built for `Card`/`CardWithNoCvc`
        // (ClearSale documents it credit-card-only). `build_clear_sale_payment`
        // therefore requires that block only for those two arms.
        PaymentMethodData::CardDetailsForNetworkTransactionId(_)
        | PaymentMethodData::Wallet(WalletData::GooglePay(_))
        | PaymentMethodData::Wallet(WalletData::ApplePay(_))
        | PaymentMethodData::Wallet(WalletData::SamsungPay(_)) => {
            Ok(CLEARSALE_PAYMENT_TYPE_CREDIT_CARD)
        }
        PaymentMethodData::Voucher(VoucherData::Boleto(_)) => Ok(CLEARSALE_PAYMENT_TYPE_BANK_SLIP),
        PaymentMethodData::Voucher(_) => Ok(CLEARSALE_PAYMENT_TYPE_VOUCHER),
        PaymentMethodData::BankTransfer(bank) => match bank.as_ref() {
            BankTransferData::Pix { .. } => Ok(CLEARSALE_PAYMENT_TYPE_PIX),
            _ => Ok(CLEARSALE_PAYMENT_TYPE_BANK_TRANSFER),
        },
        PaymentMethodData::BankDebit(_) => Ok(CLEARSALE_PAYMENT_TYPE_BANK_DEBIT),
        PaymentMethodData::GiftCard(_) => Ok(CLEARSALE_PAYMENT_TYPE_VIRTUAL_GIFT_CARD),
        PaymentMethodData::Reward => Ok(CLEARSALE_PAYMENT_TYPE_CASHBACK),
        _ => Err(error_stack::report!(
            errors::IntegrationError::NotSupported {
                message: format!(
                    "payment method `{payment_method:?}` has no ClearSale payment-type code; \
                     map it explicitly instead of defaulting to credit card"
                ),
                connector: "clearsale_br",
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Send a ClearSale-mapped instrument (card, boleto, pix, bank debit, \
                         voucher, gift card, …)"
                            .to_owned(),
                    ),
                    suggested_action: Some(
                        "Use one of the ClearSale payment-type codes (1/2/3/6/16/19/27/…)"
                            .to_owned(),
                    ),
                    doc_url: Some(CLEARSALE_DOC_URL.to_owned()),
                },
            }
        )),
    }
}

fn build_clear_sale_card(
    number: &str,
    owner_name: Secret<String>,
    network: Option<&CardNetwork>,
    validity_date: String,
    document: Option<Secret<String>>,
) -> Result<ClearSaleCard, Error> {
    if owner_name.peek().trim().is_empty() {
        return Err(missing_field_err("card.card_holder_name"));
    }
    Ok(ClearSaleCard {
        bin: Secret::new(number.chars().take(6).collect()),
        end: Secret::new(
            number
                .chars()
                .rev()
                .take(4)
                .collect::<String>()
                .chars()
                .rev()
                .collect(),
        ),
        owner_name,
        hash: None,
        card_type: clearsale_card_type(network),
        validity_date: Some(validity_date),
        document,
        nsu: None,
    })
}

pub(crate) fn build_clear_sale_payment(
    payment_method: Option<&PaymentMethodData<DefaultPCIHolder>>,
    value: FloatMajorUnit,
    date: &str,
    installments: Option<u32>,
    card_document: Option<Secret<String>>,
    currency: Currency,
) -> Result<ClearSalePayment, Error> {
    let currency_code = clearsale_currency_code(currency)?;
    let payment_type = match payment_method {
        Some(pm) => clearsale_payment_type(pm)?,
        None => {
            return Err(missing_field_err("payment_method"));
        }
    };
    let is_credit_card = payment_type == CLEARSALE_PAYMENT_TYPE_CREDIT_CARD;
    let card = match payment_method {
        Some(PaymentMethodData::Card(card)) if is_credit_card => Some(build_clear_sale_card(
            card.card_number.peek(),
            card.get_cardholder_name()?,
            card.card_network.as_ref(),
            card_validity_date(card),
            card_document,
        )?),
        Some(PaymentMethodData::CardWithNoCvc(card)) if is_credit_card => {
            Some(build_clear_sale_card(
                card.card_number.peek(),
                card.get_cardholder_name()?,
                card.card_network.as_ref(),
                card_with_no_cvc_validity_date(card),
                card_document,
            )?)
        }
        _ => None,
    };
    // Full-PAN credit card instruments require the card block (spec §5.1).
    if is_credit_card
        && matches!(
            payment_method,
            Some(PaymentMethodData::Card(_) | PaymentMethodData::CardWithNoCvc(_))
        )
        && card.is_none()
    {
        return Err(missing_field_err("payment.card"));
    }
    Ok(ClearSalePayment {
        payment_type,
        value,
        date: Some(date.to_owned()),
        currency: Some(currency_code),
        sequential: Some(1),
        installments,
        interest_rate: None,
        interest_value: None,
        card,
        voucher_order_origin: None,
        address: None,
    })
}

pub(crate) fn build_clear_sale_items(
    order_details: Option<&Vec<OrderDetailsWithAmount>>,
    order_is_gift: Option<bool>,
) -> Result<Vec<ClearSaleItem>, Error> {
    order_details
        .filter(|details| !details.is_empty())
        .ok_or_else(|| missing_field_err("order_details"))?
        .iter()
        .map(|detail| {
            Ok(ClearSaleItem {
                name: detail.product_name.clone(),
                code: detail.sku.clone().or(detail.product_id.clone()),
                bar_code: detail.upc.clone(),
                value: Some(super::ClearSaleBrAmountConvertor::convert(&detail.amount)?),
                // Line quantity, not money — the wire field is ClearSale's `amount`.
                amount: Some(detail.quantity),
                category_id: None,
                category_name: detail.category.clone(),
                is_gift: order_is_gift,
                seller_name: None,
                seller_document: None,
                is_market_place: None,
                seller_segment: None,
                shipping_company: None,
            })
        })
        .collect()
}

/// Structured inputs for [`build_order_request`] — keeps the builder free of
/// a 16-positional-arg signature.
pub(crate) struct OrderRequestParts {
    code: String,
    session_id: String,
    email: Email,
    total_value: FloatMajorUnit,
    billing: ClearSalePerson,
    shipping: Option<ClearSalePerson>,
    payments: Vec<ClearSalePayment>,
    items: Vec<ClearSaleItem>,
    ip: Option<String>,
    origin: Option<String>,
    channel_id: Option<String>,
    b2b_b2c: Option<CustomerType>,
    product: u32,
    country: Option<String>,
}

fn items_subtotal(items: &[ClearSaleItem]) -> Option<FloatMajorUnit> {
    let mut subtotal = 0.0;
    for item in items {
        let unit_value = item.value?.0;
        subtotal += unit_value * f64::from(item.amount.unwrap_or(1));
    }
    Some(FloatMajorUnit(subtotal))
}

pub(crate) fn build_order_request(
    parts: OrderRequestParts,
    feature_data: &ClearSaleFeatureData,
    travel_data: &ClearSaleTravelData<'_>,
) -> ClearSaleOrderCreateRequest {
    let OrderRequestParts {
        code,
        session_id,
        email,
        total_value,
        billing,
        shipping,
        payments,
        items,
        ip,
        origin,
        channel_id,
        b2b_b2c,
        product,
        country,
    } = parts;
    ClearSaleOrderCreateRequest {
        order: ClearSaleOrderRequest {
            code,
            session_id,
            date: to_clearsale_datetime(
                &common_utils::date_time::now()
                    .assume_utc()
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap_or_default(),
            ),
            email,
            b2b_b2c,
            item_value: items_subtotal(&items),
            total_value,
            number_of_installments: feature_data.installments,
            ip,
            is_gift: feature_data.is_gift,
            gift_message: feature_data.gift_message.clone(),
            observation: feature_data.observation.clone(),
            status: feature_data.status,
            origin,
            channel_id,
            reservation_date: feature_data
                .reservation_date
                .as_deref()
                .map(to_clearsale_datetime),
            country: country.clone().or_else(|| Some("Brasil".to_owned())),
            nationality: feature_data.nationality.clone(),
            product,
            custom_sla: feature_data.custom_sla,
            bank_authentication: None,
            billing,
            shipping,
            payments,
            items,
            list: None,
            purchase_information: None,
            social_network: feature_data.social_network.as_ref().map(|social| {
                ClearSaleSocialNetwork {
                    opt_in_compre_confie: social.opt_in_compre_confie,
                    type_social_network: social.type_social_network,
                    authentication_token: social.authentication_token.clone(),
                }
            }),
            passengers: travel_data.passengers.as_ref().and_then(|list| {
                let mapped: Vec<ClearSalePassenger> =
                    list.iter().filter_map(passenger_to_clear_sale).collect();
                (!mapped.is_empty()).then_some(mapped)
            }),
            connections: travel_data.travel_segments.as_ref().and_then(|segments| {
                let mapped: Vec<ClearSaleConnection> = segments
                    .iter()
                    .filter_map(|(leg, segment)| {
                        travel_segment_to_clear_sale_connection(leg, segment)
                    })
                    .collect();
                (!mapped.is_empty()).then_some(mapped)
            }),
            hotels: travel_data.accommodations.as_ref().and_then(|list| {
                let mapped: Vec<ClearSaleHotel> = list
                    .iter()
                    .copied()
                    .filter_map(accommodation_to_clear_sale)
                    .collect();
                (!mapped.is_empty()).then_some(mapped)
            }),
        },
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearSaleFeatureData {
    #[serde(default)]
    pub origin: Option<String>,
    #[serde(default)]
    pub channel_id: Option<String>,
    #[serde(default, rename = "isGift")]
    pub is_gift: Option<bool>,
    #[serde(default, rename = "giftMessage")]
    pub gift_message: Option<String>,
    #[serde(default)]
    pub observation: Option<String>,
    #[serde(default)]
    pub status: Option<u32>,
    #[serde(default, rename = "reservationDate")]
    pub reservation_date: Option<String>,
    /// Custom SLA for the analysis decision, in seconds.
    #[serde(default, rename = "customSla")]
    pub custom_sla: Option<u32>,
    /// Compre e Confie opt-in block, merchant-supplied.
    #[serde(default)]
    pub social_network: Option<ClearSaleSocialNetworkFeature>,
    #[serde(default)]
    pub billing_phone_type: Option<u32>,
    /// Same, for `shipping.phones[].type`.
    #[serde(default)]
    pub shipping_phone_type: Option<u32>,
    #[serde(default)]
    pub installments: Option<u32>,
    #[serde(default)]
    pub product: Option<u32>,
    #[serde(default)]
    pub nationality: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ClearSaleTravelData<'a> {
    pub passengers: Option<Vec<Traveller>>,
    pub travel_segments: Option<Vec<(&'a TravelData, &'a TravelSegment)>>,
    pub accommodations: Option<Vec<&'a HotelSegment>>,
}

fn traveller_identity(p: &Traveller) -> (String, String, String) {
    let first = p
        .first_name
        .as_ref()
        .map(|s| s.peek().trim().to_owned())
        .unwrap_or_default();
    let last = p
        .last_name
        .as_ref()
        .map(|s| s.peek().trim().to_owned())
        .unwrap_or_default();
    let document = p
        .document
        .as_ref()
        .map(|doc| doc.document_number.peek().trim().to_owned())
        .unwrap_or_default();
    (first, last, document)
}

impl<'a> ClearSaleTravelData<'a> {
    pub(crate) fn from_domain_data(domain_data: Option<&'a DomainData>) -> Self {
        let Some(domain_data) = domain_data else {
            return Self::default();
        };
        let mut passengers: Vec<Traveller> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut travel_segments = Vec::new();
        if let Some(booking) = domain_data.travel_data.as_ref() {
            for segment in &booking.travel_segments {
                for passenger in &segment.passengers {
                    if seen.insert(traveller_identity(passenger)) {
                        passengers.push(passenger.clone());
                    }
                }
                travel_segments.push((booking, segment));
            }
        }
        let segments: Vec<&HotelSegment> = domain_data
            .hotel_data
            .as_ref()
            .map(|booking| booking.hotel_segments.iter().collect())
            .unwrap_or_default();
        Self {
            passengers: (!passengers.is_empty()).then_some(passengers),
            travel_segments: (!travel_segments.is_empty()).then_some(travel_segments),
            accommodations: (!segments.is_empty()).then_some(segments),
        }
    }
}

fn gender_wire_code(gender: Gender) -> &'static str {
    match gender {
        Gender::Male => "M",
        Gender::Female => "F",
        Gender::Other => "X",
    }
}

/// ClearSale `passengers[]` source; the only mandatory field is `name` —
/// entries without a usable first/last name are dropped.
pub(crate) fn passenger_to_clear_sale(p: &Traveller) -> Option<ClearSalePassenger> {
    let name = match (p.first_name.as_ref(), p.last_name.as_ref()) {
        (None, None) => None,
        (first, last) => {
            let full = [
                first.map(|s| s.peek().clone()),
                last.map(|s| s.peek().clone()),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" ");
            let full = full.trim().to_owned();
            if full.is_empty() {
                None
            } else {
                Some(Secret::new(full))
            }
        }
    }?;
    Some(ClearSalePassenger {
        name,
        company_mile_card: p.loyalty_card_issuer.clone(),
        mile_card: p.loyalty_card.clone(),
        identification_type: p.document.as_ref().and_then(|doc| match doc.document_type {
            DocumentKind::Cpf => Some(1),
            DocumentKind::Cnpj => Some(2),
            DocumentKind::Psn | DocumentKind::Other => None,
        }),
        identification_number: p.document.as_ref().and_then(|doc| {
            let number = doc.document_number.peek().trim();
            (!number.is_empty()).then(|| Secret::new(number.to_owned()))
        }),
        gender: p.gender.map(gender_wire_code),
        birthdate: p
            .date_of_birth
            .as_ref()
            .map(|dob| to_clearsale_datetime(dob.peek())),
        // CPF rides in the identificationNumber; no separate cpf carrier
        // exists in the typed domain data.
        cpf: None,
    })
}

/// ClearSale `connections[]` source; `company`, `origin`, `destination` and
pub(crate) fn travel_segment_to_clear_sale_connection(
    leg: &TravelData,
    segment: &TravelSegment,
) -> Option<ClearSaleConnection> {
    // Only a fully-numeric identifier becomes identificationNumber (e.g.
    // "101" → 101). "G3-1234" does not parse and is dropped from the wire.
    let identifier = segment
        .identification_number
        .as_deref()
        .map(str::trim)
        .unwrap_or("");
    let identification_number = if identifier.is_empty() {
        None
    } else {
        identifier.parse::<u64>().ok()
    };
    // Fall back: departure → boarding → booking travel date → fail
    let date = segment
        .departure_date_time
        .clone()
        .or_else(|| segment.boarding_date_time.clone())
        .or_else(|| leg.travel_date.clone())
        .map(|date| to_clearsale_datetime(&date))?;
    let company = leg
        .carrier_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .or_else(|| carrier_code_from_identifier(identifier))?;
    Some(ClearSaleConnection {
        company,
        identification_number,
        date: Some(date),
        seat_class: segment.travel_class.map(|class| class.to_string()),
        origin: segment.origin.as_deref()?.trim().to_owned(),
        destination: segment.destination.as_deref()?.trim().to_owned(),
        boarding: segment
            .boarding_date_time
            .as_deref()
            .map(to_clearsale_datetime),
        arriving: segment
            .arrival_date_time
            .as_deref()
            .map(to_clearsale_datetime),
        // No separate fare-basis carrier on `TravelSegment` — `travel_class`
        // already feeds `seatClass`.
        fare_class: None,
    })
}

pub(crate) fn carrier_code_from_identifier(identifier: &str) -> Option<String> {
    let token = identifier.trim();
    if token.is_empty() {
        return None;
    }
    let candidate: &str = match token.split_once(['-', ' ', '/']) {
        Some((head, _)) if !head.trim().is_empty() => head.trim(),
        _ => {
            if token.chars().all(|c| c.is_ascii_alphabetic()) {
                token
            } else {
                let mut bytes = token.bytes();
                match (bytes.next(), bytes.next()) {
                    (Some(first), Some(second))
                        if first.is_ascii_alphabetic() && second.is_ascii_alphanumeric() =>
                    {
                        token.get(..2)?
                    }
                    _ => return None,
                }
            }
        }
    };
    let company = candidate.trim().to_uppercase();
    (!company.is_empty()
        && company.chars().all(|c| c.is_ascii_alphanumeric())
        && company.chars().any(|c| c.is_ascii_alphabetic()))
    .then_some(company)
}

/// ClearSale `hotels[]` source; `name` is mandatory. One entry per
/// [`HotelSegment`] (a booking with several stays emits several hotels[]).
pub(crate) fn accommodation_to_clear_sale(a: &HotelSegment) -> Option<ClearSaleHotel> {
    Some(ClearSaleHotel {
        name: a.hotel_name.as_deref()?.trim().to_owned(),
        city: a.hotel_city.clone(),
        state: a.hotel_state.clone(),
        country: a.hotel_country.clone(),
        reservation_date: a
            .reservation_date_time
            .as_deref()
            .map(to_clearsale_datetime),
        reserve_expiration_date: a
            .reservation_expires_date_time
            .as_deref()
            .map(to_clearsale_datetime),
        check_in_date: a.checkin.as_deref().map(to_clearsale_datetime),
        check_out_date: a.checkout.as_deref().map(to_clearsale_datetime),
    })
}

/// Merchant-supplied `socialNetwork` block carried in
/// `connector_feature_data` (ClearSale's Compre e Confie program).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearSaleSocialNetworkFeature {
    #[serde(default, rename = "optInCompreConfie")]
    pub opt_in_compre_confie: Option<u32>,
    #[serde(default, rename = "typeSocialNetwork")]
    pub type_social_network: Option<u32>,
    #[serde(default, rename = "authenticationToken")]
    pub authentication_token: Option<Secret<String>>,
}

impl ClearSaleFeatureData {
    /// Parse `connector_feature_data` into ClearSale extras. A present but
    /// malformed blob is an error — never silently defaulted.
    pub(crate) fn parse(raw: Option<&Secret<String>>) -> Result<Self, Error> {
        match raw {
            None => Ok(Self::default()),
            Some(data) => serde_json::from_str(data.peek()).map_err(|err| {
                error_stack::report!(errors::IntegrationError::InvalidDataFormat {
                    field_name: "connector_feature_data",
                    context: errors::IntegrationErrorContext {
                        additional_context: Some(format!(
                            "ClearSale connector_feature_data must be a JSON object: {err}"
                        )),
                        suggested_action: Some(
                            "Send a JSON object matching ClearSaleFeatureData".to_owned(),
                        ),
                        doc_url: Some(CLEARSALE_DOC_URL.to_owned()),
                    },
                })
            }),
        }
    }
}

/// Build the ClearSale order body shared by PreRiskCheck and PostRiskCheck.
pub(crate) fn order_from_common(
    inputs: OrderInputs<'_>,
) -> Result<ClearSaleOrderCreateRequest, Error> {
    let OrderInputs {
        code,
        session_id,
        feature_data,
        b2b_b2c,
        config_product,
        email,
        money,
        ip,
        customer_info,
        address,
        payment_method,
        order_details,
        billing_document_data,
        shipping_document_data,
        travel_data,
    } = inputs;

    let billing_document = billing_document_data.map(|doc| doc.document_number.clone());
    let shipping_document = shipping_document_data
        .map(|doc| doc.document_number.clone())
        .or_else(|| billing_document.clone());
    let billing_document_kind = billing_document_data.map(|doc| doc.document_type);
    let shipping_document_kind = shipping_document_data
        .map(|doc| doc.document_type)
        .or(billing_document_kind);

    let total_value = super::ClearSaleBrAmountConvertor::convert(&money)?;

    let billing_address = address
        .and_then(|addr| addr.get_payment_billing())
        .ok_or_else(|| missing_field_err("billing"))?;

    let billing = build_person(
        "billing",
        Some(billing_address),
        customer_info,
        billing_document.as_ref(),
        billing_document_kind.as_ref(),
        feature_data,
    )?;

    let shipping = match address.and_then(|addr| addr.get_shipping()) {
        None => None,
        Some(shipping_addr) => Some(build_person(
            "shipping",
            Some(shipping_addr),
            customer_info,
            shipping_document.as_ref(),
            shipping_document_kind.as_ref(),
            feature_data,
        )?),
    };

    let now = to_clearsale_datetime(
        &common_utils::date_time::now()
            .assume_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default(),
    );
    let payments = vec![build_clear_sale_payment(
        payment_method,
        total_value,
        &now,
        feature_data.installments,
        Some(billing.primary_document.clone()),
        money.currency,
    )?];
    let items = build_clear_sale_items(order_details, feature_data.is_gift)?;
    let billing_country = billing_address
        .address
        .as_ref()
        .and_then(|details| details.country.map(clearsale_country_name));

    Ok(build_order_request(
        OrderRequestParts {
            code,
            session_id,
            email,
            total_value,
            billing,
            shipping,
            payments,
            items,
            ip,
            origin: feature_data.origin.clone(),
            channel_id: feature_data.channel_id.clone(),
            b2b_b2c,
            // Two scopes, one value: `connector_feature_data.product` is the
            // per-request override, `config_product` is the account-wide
            // default on the connector config. A merchant provisioned for one
            // product never sets the request field; one who sells under more
            // than one product picks per order. Falls back to Total Garantido.
            product: feature_data
                .product
                .or(config_product)
                .unwrap_or(DEFAULT_CLEARSALE_PRODUCT_TOTAL_GARANTIDO),
            country: billing_country,
        },
        feature_data,
        &travel_data,
    ))
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::ClearSaleBrRouterData<
            RouterDataV2<PreRiskCheck, FrmFlowData, PreRiskCheckRequest, PreRiskCheckResponse>,
            T,
        >,
    > for ClearSaleOrderCreateRequest
{
    type Error = Error;

    fn try_from(
        item: super::ClearSaleBrRouterData<
            RouterDataV2<PreRiskCheck, FrmFlowData, PreRiskCheckRequest, PreRiskCheckResponse>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let req = &item.router_data.request;
        let feature_data = ClearSaleFeatureData::parse(req.connector_feature_data.as_ref())?;
        let auth = ClearSaleAuthType::try_from(&item.router_data.connector_config)?;
        order_from_common(OrderInputs::from_pre_risk_check(
            req,
            &feature_data,
            auth.b2b_b2c,
            auth.product,
        )?)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::ClearSaleBrRouterData<
            RouterDataV2<PostRiskCheck, FrmFlowData, PostRiskCheckRequest, PostRiskCheckResponse>,
            T,
        >,
    > for ClearSaleOrderReanalysisRequest
{
    type Error = Error;

    fn try_from(
        item: super::ClearSaleBrRouterData<
            RouterDataV2<PostRiskCheck, FrmFlowData, PostRiskCheckRequest, PostRiskCheckResponse>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let req = &item.router_data.request;
        let feature_data = ClearSaleFeatureData::parse(req.connector_feature_data.as_ref())?;
        let auth = ClearSaleAuthType::try_from(&item.router_data.connector_config)?;
        order_from_common(OrderInputs::from_post_risk_check(
            req,
            &feature_data,
            auth.b2b_b2c,
            auth.product,
        )?)
        .map(ClearSaleOrderReanalysisRequest)
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Responses — order create / re-analysis
// ──────────────────────────────────────────────────────────────────────────
pub(crate) fn first_order_decision(
    response: &ClearSaleOrderCreateResponse,
) -> (FrmDecision, Option<i32>, Option<String>) {
    response.orders.first().map_or_else(
        || (FrmDecision::Review, None, None),
        |order| {
            (
                order.status.to_frm_decision(),
                #[allow(clippy::as_conversions)]
                order.score.map(|score| score.trunc() as i32),
                Some(order.code.clone()),
            )
        },
    )
}

impl TryFrom<ResponseRouterData<ClearSaleOrderCreateResponse, Self>>
    for RouterDataV2<PreRiskCheck, FrmFlowData, PreRiskCheckRequest, PreRiskCheckResponse>
{
    type Error = ResponseError;

    fn try_from(
        item: ResponseRouterData<ClearSaleOrderCreateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let (frm_decision, score, order_code) = first_order_decision(&item.response);
        Ok(Self {
            response: Ok(PreRiskCheckResponse {
                frm_decision: Some(frm_decision),
                risk_score: score,
                reason: Some(format!("packageID: {}", item.response.package_id)),
                frm_transaction_id: order_code
                    .or_else(|| item.router_data.request.merchant_transaction_id.clone()),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

impl TryFrom<ResponseRouterData<ClearSaleOrderReanalysisResponse, Self>>
    for RouterDataV2<PostRiskCheck, FrmFlowData, PostRiskCheckRequest, PostRiskCheckResponse>
{
    type Error = ResponseError;

    fn try_from(
        item: ResponseRouterData<ClearSaleOrderReanalysisResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let (frm_decision, score, order_code) = first_order_decision(&item.response.0);
        Ok(Self {
            response: Ok(PostRiskCheckResponse {
                frm_decision: Some(frm_decision),
                risk_score: score,
                reason: Some(format!("packageID: {}", item.response.0.package_id)),
                frm_transaction_id: order_code
                    .or_else(|| item.router_data.request.merchant_transaction_id.clone()),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

pub(crate) fn clearsale_payment_outcome_status(
    payment_status: Option<AttemptStatus>,
    frm_decision: Option<FrmDecision>,
) -> Option<&'static str> {
    match payment_status {
        Some(
            AttemptStatus::Charged
            | AttemptStatus::PartialCharged
            | AttemptStatus::PartialChargedAndChargeable,
        ) => Some("PGA"),
        Some(
            AttemptStatus::AuthorizationFailed
            | AttemptStatus::CaptureFailed
            | AttemptStatus::Failure,
        ) => Some("PGR"),
        _ => match frm_decision {
            Some(FrmDecision::Reject) => Some("PGR"),
            _ => None,
        },
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::ClearSaleBrRouterData<
            RouterDataV2<
                FrmPaymentOutcome,
                FrmFlowData,
                FrmPaymentOutcomeRequest,
                FrmPaymentOutcomeResponse,
            >,
            T,
        >,
    > for ClearSaleStatusUpdateRequest
{
    type Error = Error;

    fn try_from(
        item: super::ClearSaleBrRouterData<
            RouterDataV2<
                FrmPaymentOutcome,
                FrmFlowData,
                FrmPaymentOutcomeRequest,
                FrmPaymentOutcomeResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let req = &item.router_data.request;
        let status = clearsale_payment_outcome_status(req.payment_status, req.frm_decision)
            .ok_or_else(|| {
                error_stack::report!(errors::IntegrationError::MissingRequiredField {
                    field_name: "payment_status",
                    context: errors::IntegrationErrorContext {
                        additional_context: Some(
                            "ClearSale expects PGA (approved) or PGR (denied); no ClearSale \
                             status maps to the supplied payment_status"
                                .to_owned(),
                        ),
                        suggested_action: Some(
                            "Send the payment status (charged / authorization_failed / \
                             capture_failed) on the FRM notification"
                                .to_owned(),
                        ),
                        doc_url: Some(CLEARSALE_DOC_URL.to_owned()),
                    },
                })
            })?;
        Ok(Self {
            status: status.to_string(),
        })
    }
}

impl TryFrom<ResponseRouterData<ClearSaleStatusUpdateResponse, Self>>
    for RouterDataV2<
        FrmPaymentOutcome,
        FrmFlowData,
        FrmPaymentOutcomeRequest,
        FrmPaymentOutcomeResponse,
    >
{
    type Error = ResponseError;

    fn try_from(
        item: ResponseRouterData<ClearSaleStatusUpdateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        if !item.response.status.eq_ignore_ascii_case("OK") {
            return Err(error_stack::report!(
                errors::ConnectorError::UnexpectedResponseError {
                    context: errors::ResponseTransformationErrorContext {
                        http_status_code: Some(item.http_code),
                        additional_context: Some(format!(
                            "ClearSale status push was not accepted: status = {:?}, message = {:?}",
                            item.response.status,
                            item.response.message.as_deref().unwrap_or_default(),
                        )),
                    },
                }
            ));
        }
        Ok(Self {
            response: Ok(FrmPaymentOutcomeResponse {
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}
