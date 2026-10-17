use common_enums::{AttemptStatus, CardNetwork, Currency, FrmDecision};
use common_utils::types::FloatMajorUnit;
use domain_types::{
    connector_flow::{FrmPaymentOutcome, PostRiskCheck, PreRiskCheck, ServerAuthenticationToken},
    connector_types::{
        DomainData, HotelData, ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData, TravelData, Traveller,
    },
    errors,
    frm::frm_types::{
        FrmFlowData, FrmMetadata, FrmPaymentOutcomeRequest, FrmPaymentOutcomeResponse,
        PostRiskCheckRequest, PostRiskCheckResponse, PreRiskCheckRequest, PreRiskCheckResponse,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_address::OrderDetailsWithAmount,
    payment_method_data::{
        Card, CustomerDocumentDetails, DefaultPCIHolder, DocumentKind, PaymentMethodData,
        PaymentMethodDataTypes,
    },
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
};
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::types::ResponseRouterData;

pub(crate) type Error = error_stack::Report<errors::IntegrationError>;
type ResponseError = error_stack::Report<errors::ConnectorError>;

/// ClearSale Total / Total Garantido API reference.
pub const CLEARSALE_DOC_URL: &str =
    "https://api.clearsale.com.br/docs/total-totalGarantido-application";

/// Default `product` value for ClearSale **Total Garantido** (spec §1).
///
/// Assumption: `4` is the Total Garantido product id from the public API
/// reference ([`CLEARSALE_DOC_URL`]) — not yet confirmed against the merchant
/// pack (spec §11 open question 3). Merchants on a different ClearSale product
/// override it per request (`connector_feature_data.product`) or account-wide
/// on the connector config.
pub const DEFAULT_CLEARSALE_PRODUCT_TOTAL_GARANTIDO: i32 = 4;

// ──────────────────────────────────────────────────────────────────────────
// Auth
// ──────────────────────────────────────────────────────────────────────────

/// ClearSale login credentials. `username` is the account login name and
/// `password` the account password, matching the proto `ClearsaleBrConfig`
/// creds shape (both sent on POST /v1/authenticate as `name`/`password`).
/// `b2b_b2c` is the order's customer type ("b2b"/"b2c") — gateway/merchant
/// specific, so it rides on the RiskManagementAccount-level connector config
/// rather than per-transaction data. `product` is the ClearSale product id
/// (see [`DEFAULT_CLEARSALE_PRODUCT_TOTAL_GARANTIDO`]).
#[derive(Debug, Clone)]
pub struct ClearSaleAuthType {
    pub username: Secret<String>,
    pub password: Secret<String>,
    pub b2b_b2c: Option<String>,
    pub product: Option<i32>,
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
                b2b_b2c: b2b_b2c.clone(),
                product: *product,
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

/// Customer type (`b2bB2c` order field) from the connector config. It is
/// gateway/merchant-specific, so it is stored on the RiskManagementAccount
/// account details rather than derived per transaction.
fn b2b_b2c_from_config(config: &ConnectorSpecificConfig) -> Option<String> {
    match config {
        ConnectorSpecificConfig::ClearSaleBr { b2b_b2c, .. } => b2b_b2c.clone(),
        _ => None,
    }
}

/// Account-level ClearSale product id override from the connector config,
/// when present.
fn product_from_config(config: &ConnectorSpecificConfig) -> Option<i32> {
    match config {
        ConnectorSpecificConfig::ClearSaleBr { product, .. } => *product,
        _ => None,
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Error response
// ──────────────────────────────────────────────────────────────────────────

/// ClearSale error bodies are not formally documented; they commonly carry a
/// `Message`/`message` field. Both casings are accepted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClearSaleErrorResponse {
    #[serde(default, alias = "message")]
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
        Ok(Self {
            response: Ok(ServerAuthenticationTokenResponseData {
                access_token: item.response.token,
                token_type: Some("Bearer".to_string()),
                // ClearSale returns an absolute ExpirationDate, not a TTL; the
                // composite token cache is responsible for interpreting it.
                expires_in: None,
            }),
            ..item.router_data
        })
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Order payload — POST /v1/orders/ (spec §5.1)
// ──────────────────────────────────────────────────────────────────────────

/// billing / shipping person block. `person_type`: 1 = Pessoa Física,
/// 2 = Pessoa Jurídica. The delivery-only fields (`delivery_type`,
/// `delivery_time`, `price`, `pick_up_store_document`) are meaningful only on
/// `shipping`; they stay `None` (and off the wire) for `billing`.
#[derive(Debug, Clone, Serialize)]
pub struct ClearSalePerson {
    #[serde(rename = "type")]
    pub person_type: i32,
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
    pub email: Option<common_utils::pii::Email>,
    /// `M` / `F`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gender: Option<String>,
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
#[allow(dead_code)]
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
    pub phone_type: i32,
    /// Area code (2 digits). ClearSale expects integer, not string.
    pub ddd: i64,
    /// Phone number digits. ClearSale expects integer, not string.
    pub number: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ddi: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extension: Option<String>,
}

/// `payment_type`: 1 = Cartão de Crédito, 2 = Boleto, 19 = Débito, 27 = PIX.
#[derive(Debug, Clone, Serialize)]
pub struct ClearSalePayment {
    #[serde(rename = "type")]
    pub payment_type: i32,
    pub value: FloatMajorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    /// ISO-4217 numeric code (e.g. `986` for BRL).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<i32>,
    /// Payment sequence within the order (1-based).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sequential: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installments: Option<i32>,
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
    pub card_type: Option<i32>,
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
    pub category_id: Option<i64>,
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

/// The ClearSale order (spec §5.1). Amounts are float majors (spec §7).
/// Optional blocks UCS has no first-class carrier for (`list`,
/// `purchase_information`, `social_network`, `passengers`, `connections`,
/// `hotels`) ride along only when the merchant supplies them via
/// `connector_feature_data` — otherwise they are omitted.
#[derive(Debug, Clone, Serialize)]
pub struct ClearSaleOrderRequest {
    pub code: String,
    /// Mandatory — from ClearSale's Behavior Analytics SDK (spec §10).
    #[serde(rename = "sessionID")]
    pub session_id: String,
    pub date: String,
    pub email: common_utils::pii::Email,
    #[serde(rename = "b2bB2c", skip_serializing_if = "Option::is_none")]
    pub b2b_b2c: Option<String>,
    #[serde(rename = "itemValue", skip_serializing_if = "Option::is_none")]
    pub item_value: Option<FloatMajorUnit>,
    #[serde(rename = "totalValue")]
    pub total_value: FloatMajorUnit,
    #[serde(
        rename = "numberOfInstallments",
        skip_serializing_if = "Option::is_none"
    )]
    pub number_of_installments: Option<i32>,
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
    pub status: Option<i32>,
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
    pub product: i32,
    /// Custom SLA in seconds for the analysis decision.
    #[serde(rename = "customSla", skip_serializing_if = "Option::is_none")]
    pub custom_sla: Option<i32>,
    /// 3DS authentication result text (e.g. "Aprovado 3DS").
    #[serde(rename = "bankAuthentication", skip_serializing_if = "Option::is_none")]
    pub bank_authentication: Option<String>,
    pub billing: ClearSalePerson,
    pub shipping: ClearSalePerson,
    pub payments: Vec<ClearSalePayment>,
    pub items: Vec<ClearSaleItem>,
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
    pub type_id: i32,
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
    pub email: Option<common_utils::pii::Email>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login: Option<String>,
}

/// Order `socialNetwork` block (spec §5.1).
#[derive(Debug, Clone, Serialize)]
pub struct ClearSaleSocialNetwork {
    #[serde(rename = "optInCompreConfie", skip_serializing_if = "Option::is_none")]
    pub opt_in_compre_confie: Option<i32>,
    #[serde(rename = "typeSocialNetwork", skip_serializing_if = "Option::is_none")]
    pub type_social_network: Option<i32>,
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
    pub mile_card: Option<String>,
    /// ID type per spec: 1=CPF, 2=CNPJ, 3=RG, 4=IE, 5=Passport, 6=CTPS, 7=Voter ID.
    /// Required only when identificationNumber is present.
    #[serde(rename = "identificationType", skip_serializing_if = "Option::is_none")]
    pub identification_type: Option<i32>,
    #[serde(
        rename = "identificationNumber",
        skip_serializing_if = "Option::is_none"
    )]
    pub identification_number: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gender: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub birthdate: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpf: Option<Secret<String>>,
}

/// Order `connections[]` block (spec §5.1) — travel legs. Relevant for Buser's
/// bus-ticketing orders.
#[derive(Debug, Clone, Serialize)]
pub struct ClearSaleConnection {
    pub company: String,
    #[serde(
        rename = "identificationNumber",
        skip_serializing_if = "Option::is_none"
    )]
    pub identification_number: Option<i64>,
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
/// `PUT /v1/orders/` (PostRiskCheck re-analysis — same shape, spec §4).
/// ClearSale expects the order fields FLAT at top level (no "order" wrapper).
#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct ClearSaleOrderCreateRequest {
    pub order: ClearSaleOrderRequest,
}

/// PostRiskCheck (PUT /v1/orders/ re-analysis) wire shape is identical to the
/// create — a newtype so the connector macros can template it separately
/// (same treatment as nSure's per-flow newtypes).
#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct ClearSaleOrderReanalysisRequest(pub ClearSaleOrderCreateRequest);

// ──────────────────────────────────────────────────────────────────────────
// Order response + status codes (spec §5.2, §6)
// ──────────────────────────────────────────────────────────────────────────

/// ClearSale 3-letter order status codes (spec §6).
///
/// ClearSale sends these UPPERCASE on the wire (`"NVO"`, `"AMA"`, ...).
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

/// `GET /v1/orders/{code}/status` response (spec §5.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClearSaleStatusGetResponse {
    pub code: String,
    pub status: ClearSaleStatusEnum,
    #[serde(default)]
    pub score: Option<f64>,
}

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

/// ClearSale's webhook carries no status — only the order code; the
/// integrator must follow up with `GET /v1/orders/{code}/status`.
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

/// `type` value for a Pessoa Física (the buyer is a natural person — spec
/// §5.1). This integration always sends PF: no typed CNPJ carrier exists on
/// the request, so B2B shoppers are out of scope until a usecase lands.
const CLEARSALE_PERSON_TYPE_PF: i32 = 1;
/// `payments[].type` for Cartão de Crédito (spec §5.1).
const CLEARSALE_PAYMENT_TYPE_CREDIT_CARD: i32 = 1;
/// `payments[].type` for Débito (spec §5.1).
const CLEARSALE_PAYMENT_TYPE_DEBIT: i32 = 19;
/// `phones[].type` default when the merchant supplies no phone-kind hint:
/// 1 = Home. A merchant on a mobile-first flow overrides it via
/// `connector_feature_data.billing.phoneType` / `.shipping.phoneType`.
const CLEARSALE_PHONE_TYPE_HOME: i32 = 1;
/// IOError detail used when UCS can't produce a mandatory ClearSale field.
fn missing_field_err(field_name: &'static str) -> Error {
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

/// ISO-4217 numeric currency code for ClearSale `payments[].currency` (spec
/// §7). Today UCS carries `common_enums::Currency` with no numeric accessor;
/// BRL (986) is the only code ClearSale Total Garantido operates on.
fn clearsale_currency_code(currency: Currency) -> i32 {
    match currency {
        Currency::BRL => 986,
        _ => {
            // ClearSale BR accounts are BRL-denominated; a foreign currency
            // still needs a numeric wire value, so we fall back to the BRL
            // account code rather than dropping the field. A future
            // common_enums::Currency::to_numeric() should replace this.
            986
        }
    }
}

/// Every risk-check input `order_from_common` needs, pre-extracted from either
/// `PreRiskCheckRequest` or `PostRiskCheckRequest` (they carry the same
/// purchase fields, so the construction is shared; spec §4 prescribes the same
/// order body for POST and PUT).
struct OrderInputs<'a> {
    code: String,
    feature_data: &'a ClearSaleFeatureData,
    /// Customer type from the connector config (`b2bB2c`); merchant-specific,
    /// stored on the RiskManagementAccount.
    b2b_b2c: Option<String>,
    /// Account-level ClearSale product id override from the connector config
    /// (the per-request `connector_feature_data.product` wins over this).
    config_product: Option<i32>,
    email: Option<common_utils::pii::Email>,
    money: common_utils::types::Money,
    ip: Option<String>,
    customer_info: Option<&'a domain_types::connector_types::CustomerInfo>,
    address: Option<&'a domain_types::payment_address::PaymentAddress>,
    payment_method: Option<&'a PaymentMethodData<DefaultPCIHolder>>,
    order_details: Option<&'a Vec<OrderDetailsWithAmount>>,
    /// Travel/hospitality blocks projected from the typed
    /// `PreRiskCheckRequest::domain_data` (responds with all-`None` lists for
    /// non-travel orders).
    travel_data: ClearSaleTravelData,
}

impl<'a> OrderInputs<'a> {
    fn from_pre_risk_check(
        req: &'a PreRiskCheckRequest,
        feature_data: &'a ClearSaleFeatureData,
        b2b_b2c: Option<String>,
        config_product: Option<i32>,
        code: String,
    ) -> Self {
        let travel_data = ClearSaleTravelData::from_domain_data(req.domain_data.as_ref());
        Self {
            code,
            feature_data,
            b2b_b2c,
            config_product,
            travel_data,
            email: req
                .customer_info
                .as_ref()
                .and_then(|customer| customer.customer_email.clone()),
            money: req.amount.clone(),
            ip: req
                .browser_info
                .as_ref()
                .and_then(|info| info.ip_address.map(|ip_addr| ip_addr.to_string())),
            customer_info: req.customer_info.as_ref(),
            address: req.address.as_ref(),
            payment_method: req.payment_method.as_ref(),
            order_details: req.order_details.as_ref(),
        }
    }

    fn from_post_risk_check(
        req: &'a PostRiskCheckRequest,
        feature_data: &'a ClearSaleFeatureData,
        b2b_b2c: Option<String>,
        config_product: Option<i32>,
        code: String,
    ) -> Self {
        Self {
            code,
            feature_data,
            b2b_b2c,
            config_product,
            // PostRiskCheckRequest carries no `domain_data` — re-analysis
            // replays the order body; travel blocks stay empty.
            travel_data: ClearSaleTravelData::default(),
            email: req
                .customer_info
                .as_ref()
                .and_then(|customer| customer.customer_email.clone()),
            money: req.amount.clone(),
            // PostRiskCheckRequest carries no browser_info — the risk signals
            // were captured on the pre check.
            ip: None,
            customer_info: req.customer_info.as_ref(),
            address: req.address.as_ref(),
            payment_method: req.payment_method.as_ref(),
            order_details: req.order_details.as_ref(),
        }
    }
}

/// Split `line1` ("Rua Augusta, 1234" / "Rua Augusta 1234") into ClearSale's
/// `(street, number)` pair. A trailing all-digit (or digit-plus-letter suffix
/// such as "12B") token after whitespace or a comma is treated as the number;
/// `line3` can also carry the number ("Rua Augusta" / "Bela Vista" / "1234").
///
/// When neither yields a number, the **whole `line1` is used as `number`**
/// (street left empty) — ClearSale requires both fields, so rather than
/// hard-failing the order we send the address line as the number. Only a
/// missing `line1` with no usable `line3` still fails.
fn split_street_and_number(
    line1: Option<&Secret<String>>,
    line3: Option<&Secret<String>>,
) -> Result<(Secret<String>, String), Error> {
    let line1 = line1.map(|value| value.peek().trim().to_owned());
    let line3_number = line3
        .map(|value| value.peek().trim().to_owned())
        .filter(|value| value.chars().all(|c| c.is_ascii_alphanumeric()) && !value.is_empty());
    match line1 {
        Some(line1) => {
            // Try the tail of line1 first: the last comma/space-separated token
            // wins when it looks like a house number.
            let tail = line1
                .rsplit([',', ' '])
                .map(str::trim)
                .find(|token| !token.is_empty());
            match tail {
                Some(token)
                    if token.len() <= 14
                        && token
                            .chars()
                            .all(|c| c.is_ascii_digit() || c.is_ascii_alphabetic())
                        && token.chars().next().is_some_and(|c| c.is_ascii_digit()) =>
                {
                    let street = line1[..line1.len() - token.len()]
                        .trim_end_matches([',', ' '])
                        .to_owned();
                    if street.is_empty() {
                        // line1 was nothing but a number — send it as `number`.
                        Ok((Secret::new(String::new()), token.to_owned()))
                    } else {
                        Ok((Secret::new(street), token.to_owned()))
                    }
                }
                // line3 carries a bare number token ("Rua Augusta" / "Bela Vista" / "1234").
                _ => match line3_number {
                    Some(number) => Ok((Secret::new(line1), number)),
                    // No parseable number: fall back to the whole `line1` as `number`.
                    // ClearSale requires both fields; the street stays empty rather
                    // than hard-failing the order.
                    None => Ok((Secret::new(String::new()), line1)),
                },
            }
        }
        None => line3_number
            .map(|number| (Secret::new(String::new()), number))
            .ok_or_else(|| missing_field_err("billing.address.number")),
    }
}

/// Build the ClearSale `address` block from UCS `AddressDetails`. The field
/// mapping (spec §5.1): `street`/`number` come from the `line1` split,
/// `county` (bairro) from `line2`, `city`, `state` (UF), `zipcode`, and
/// `additionalInformation` from `line3` when it was not consumed as the
/// number.
fn build_clear_sale_address(
    address_details: Option<&domain_types::payment_address::AddressDetails>,
) -> Result<ClearSaleAddress, Error> {
    let raw_line3 = address_details
        .and_then(|details| details.line3.as_ref())
        .map(|line| line.peek().trim().to_owned());
    let (street, number) = split_street_and_number(
        address_details.and_then(|details| details.line1.as_ref()),
        address_details.and_then(|details| details.line3.as_ref()),
    )?;
    // `line3` doubles as the number only when it is a bare alphanumeric token;
    // otherwise it belongs in `additionalInformation` — never both.
    let additional_information = raw_line3
        .filter(|line| !(line.chars().all(|c| c.is_ascii_alphanumeric()) && !line.is_empty()))
        .filter(|line| !line.is_empty())
        .map(Secret::new);
    Ok(ClearSaleAddress {
        street,
        number,
        county: address_details
            .and_then(|details| details.line2.clone())
            .unwrap_or_else(|| Secret::new(String::new())),
        city: address_details
            .and_then(|details| details.city.clone())
            .unwrap_or_else(|| Secret::new(String::new())),
        state: address_details
            .and_then(|details| details.state.clone())
            .unwrap_or_else(|| Secret::new(String::new())),
        zipcode: address_details
            .and_then(|details| details.zip.clone())
            .unwrap_or_else(|| Secret::new(String::new())),
        country: address_details.and_then(|details| details.country.map(|c| c.to_string())),
        additional_information,
        reference: None,
    })
}

/// Split a Brazilian phone number into `(ddi, ddd, number)`. When a country
/// code like `+55` is present it becomes `ddi`; the first two digits of what
/// remains are the DDD area code. A non-BR number still must produce a `ddd`
/// because ClearSale requires one, so the first two digits are used as-is —
/// imperfect, but better than failing the whole order for a phone shape.
fn split_phone(number: &str, country_code: Option<&str>, phone_type: i32) -> ClearSalePhone {
    let digits: String = number.chars().filter(|c| c.is_ascii_digit()).collect();
    let (ddi, national) = match country_code
        .map(|code| code.trim_start_matches('+'))
        .filter(|code| !code.is_empty())
    {
        Some(code) => (code.parse::<i64>().ok(), digits.clone()),
        None => (None, digits.clone()),
    };
    let (ddd_str, local_str) = national.split_at(national.len().min(2));
    let ddd = ddd_str.parse::<i64>().unwrap_or(0);
    let number = local_str.parse::<i64>().unwrap_or(0);
    ClearSalePhone {
        phone_type,
        ddd,
        number,
        ddi,
        extension: None,
    }
}

/// Collect the contact phone for one person block, preferring the address
/// phone and falling back to the customer-level phone so the required
/// `phones` array never goes out empty when any number exists anywhere.
fn collect_phones(
    address: Option<&domain_types::payment_address::Address>,
    customer_info: Option<&domain_types::connector_types::CustomerInfo>,
    phone_type: i32,
) -> Vec<ClearSalePhone> {
    let from_address = address
        .and_then(|addr| addr.phone.as_ref())
        .and_then(|phone| {
            phone
                .number
                .as_ref()
                .map(|number| split_phone(number.peek(), phone.country_code.as_deref(), phone_type))
        });
    let from_customer = customer_info.and_then(|customer| {
        customer.customer_phone_number.as_ref().map(|number| {
            split_phone(
                number.peek(),
                customer.customer_phone_country_code.as_deref(),
                phone_type,
            )
        })
    });
    match (from_address, from_customer) {
        (Some(phone), _) | (None, Some(phone)) => vec![phone],
        (None, None) => vec![],
    }
}

/// Resolve a person's display name: the customer's explicit name wins, then
/// the address first+last names.
fn person_name(
    address_details: Option<&domain_types::payment_address::AddressDetails>,
    customer_info: Option<&domain_types::connector_types::CustomerInfo>,
) -> Option<Secret<String>> {
    customer_info
        .and_then(|customer| customer.get_full_name())
        .or_else(|| address_details.and_then(|details| details.get_optional_full_name()))
}

/// Build one `billing`/`shipping` person block. `missing` accumulates the
/// ClearSale-field names this connector cannot synthesize so the caller can
/// fail once with the full list.
/// Resolve the per-person CPF/CNPJ. The only document source is the request's
/// typed `document_details` (merged into `ClearSaleFeatureData` by
/// `merged_with_euler_data`, shipping falling back to the billing document
/// when absent).
fn primary_document(feature_document: Option<&String>) -> Option<Secret<String>> {
    feature_document
        .map(|doc| doc.trim())
        .filter(|doc| !doc.is_empty())
        .map(|doc| Secret::new(doc.to_owned()))
}

fn build_person(
    label: &str,
    address: Option<&domain_types::payment_address::Address>,
    customer_info: Option<&domain_types::connector_types::CustomerInfo>,
    document: Option<&String>,
    feature_data: &ClearSaleFeatureData,
    missing: &mut Vec<&'static str>,
) -> Option<ClearSalePerson> {
    let address_details = address.and_then(|addr| addr.address.as_ref());
    let primary_document = match primary_document(document) {
        Some(doc) => doc,
        None => {
            missing.push(match label {
                "billing" => "billing.primaryDocument (CPF)",
                _ => "shipping.primaryDocument (CPF)",
            });
            return None;
        }
    };
    let name = match person_name(address_details, customer_info) {
        Some(name) => name,
        None => {
            missing.push(match label {
                "billing" => "billing.name",
                _ => "shipping.name",
            });
            return None;
        }
    };
    let phone_type = match label {
        "billing" => feature_data
            .billing_phone_type
            .unwrap_or(CLEARSALE_PHONE_TYPE_HOME),
        _ => feature_data
            .shipping_phone_type
            .unwrap_or(CLEARSALE_PHONE_TYPE_HOME),
    };
    let phones = collect_phones(address, customer_info, phone_type);
    if phones.is_empty() {
        missing.push(match label {
            "billing" => "billing.phones[0]",
            _ => "shipping.phones[0]",
        });
        return None;
    }
    // The buyer is always a Pessoa Física (CPF-holder) for this integration —
    // no typed CNPJ carrier exists on the request, so B2B shoppers are out of
    // scope until a usecase lands.
    let person_type = CLEARSALE_PERSON_TYPE_PF;
    let address = match build_clear_sale_address(address_details) {
        Ok(address) => address,
        Err(err) => {
            // `split_street_and_number` rejected a mandatory address field;
            // surface that on the caller as a missing-field error rather
            // than swallowing it into the per-block `missing` list.
            return match err.current_context() {
                errors::IntegrationError::MissingRequiredField { field_name, .. } => {
                    missing.push(field_name);
                    None
                }
                _ => None,
            };
        }
    };
    Some(ClearSalePerson {
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
                .map(|date| date.peek().to_string())
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
fn clearsale_card_type(network: Option<&CardNetwork>) -> Option<i32> {
    match network? {
        CardNetwork::DinersClub => Some(1),
        CardNetwork::Mastercard => Some(2),
        CardNetwork::Visa => Some(3),
        CardNetwork::AmericanExpress => Some(5),
        _ => None,
    }
}

/// ClearSale `validityDate` is `MM/yyyy`.
fn card_validity_date(card: &Card<DefaultPCIHolder>) -> String {
    format!(
        "{}/{}",
        card.card_exp_month.peek(),
        card.get_expiry_year_4_digit().peek()
    )
}

/// Build `payments[0]` from the purchase amount and instrument (spec §5.1).
/// Cards map to type 1 (or 19 when the purchase flagged the card as debit),
/// Boleto vouchers to type 2, and any other / absent instrument to the
/// credit-card default so the required array stays non-empty. The wire has no
/// per-payment currency field — ClearSale accounts are configured
/// per-currency (the BRL deployment is ISO 986).
fn build_clear_sale_payment(
    payment_method: Option<&PaymentMethodData<DefaultPCIHolder>>,
    value: FloatMajorUnit,
    date: &str,
    installments: Option<i32>,
    card_document: Option<Secret<String>>,
    currency: Currency,
) -> ClearSalePayment {
    match payment_method {
        Some(PaymentMethodData::Card(card)) => {
            let is_debit = card
                .card_type
                .as_deref()
                .is_some_and(|kind| kind.eq_ignore_ascii_case("debit"));
            let number = card.card_number.peek();
            ClearSalePayment {
                payment_type: if is_debit {
                    CLEARSALE_PAYMENT_TYPE_DEBIT
                } else {
                    CLEARSALE_PAYMENT_TYPE_CREDIT_CARD
                },
                value,
                date: Some(date.to_owned()),
                currency: Some(clearsale_currency_code(currency)),
                sequential: Some(1),
                installments,
                interest_rate: None,
                interest_value: None,
                card: Some(ClearSaleCard {
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
                    owner_name: card
                        .card_holder_name
                        .clone()
                        .unwrap_or_else(|| Secret::new(String::new())),
                    hash: None,
                    card_type: clearsale_card_type(card.card_network.as_ref()),
                    validity_date: Some(card_validity_date(card)),
                    document: card_document,
                    nsu: None,
                }),
                voucher_order_origin: None,
                address: None,
            }
        }
        Some(PaymentMethodData::Voucher(
            domain_types::payment_method_data::VoucherData::Boleto(_),
        )) => ClearSalePayment {
            payment_type: 2, // Boleto Bancário (spec §5.1)
            value,
            date: Some(date.to_owned()),
            currency: Some(clearsale_currency_code(currency)),
            sequential: Some(1),
            installments,
            interest_rate: None,
            interest_value: None,
            card: None,
            voucher_order_origin: None,
            address: None,
        },
        Some(_) | None => ClearSalePayment {
            payment_type: CLEARSALE_PAYMENT_TYPE_CREDIT_CARD,
            value,
            date: Some(date.to_owned()),
            currency: Some(clearsale_currency_code(currency)),
            sequential: Some(1),
            installments,
            interest_rate: None,
            interest_value: None,
            card: None,
            voucher_order_origin: None,
            address: None,
        },
    }
}

/// Build the `items` array from `order_details`. Kount-style: when the FRM
/// request carries no line items the call fails — ClearSale's docs only mark
/// `items` as required (spec §5.1), and a synthetic "Order item" at the order
/// total would be a fraudulent representation of what the buyer purchased.
fn build_clear_sale_items(
    order_details: Option<&Vec<OrderDetailsWithAmount>>,
    currency: Currency,
    total_value: FloatMajorUnit,
    order_is_gift: Option<bool>,
) -> Result<Vec<ClearSaleItem>, Error> {
    let _ = total_value; // kept in the signature for the converter guard below
    order_details
        .filter(|details| !details.is_empty())
        .ok_or_else(|| missing_field_err("order_details"))?
        .iter()
        .map(|detail| {
            Ok(ClearSaleItem {
                name: detail.product_name.clone(),
                code: detail.sku.clone().or(detail.product_id.clone()),
                bar_code: detail.upc.clone(),
                value: Some(super::ClearSaleBrAmountConvertor::convert(
                    detail.amount,
                    currency,
                )?),
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

/// Build the ClearSale order from the prepared inputs (spec §4: the same
/// order body backs both the POST create and the PUT re-analysis, so the
/// construction is shared).
#[allow(clippy::too_many_arguments)]
fn build_order_request(
    code: String,
    session_id: String,
    email: common_utils::pii::Email,
    total_value: FloatMajorUnit,
    billing: ClearSalePerson,
    shipping: ClearSalePerson,
    payments: Vec<ClearSalePayment>,
    items: Vec<ClearSaleItem>,
    ip: Option<String>,
    origin: Option<String>,
    channel_id: Option<String>,
    b2b_b2c: Option<String>,
    product: i32,
    country: Option<String>,
    feature_data: &ClearSaleFeatureData,
    travel_data: &ClearSaleTravelData,
) -> ClearSaleOrderCreateRequest {
    ClearSaleOrderCreateRequest {
        order: ClearSaleOrderRequest {
            code,
            session_id,
            // ClearSale expects an ISO-8601 datetime; RFC 3339 is ISO-8601.
            // `date_time::now` is the one sanctioned clock read.
            date: common_utils::date_time::now()
                .assume_utc()
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_default(),
            email,
            // Customer type is gateway/merchant-specific: it lives on the
            // RiskManagementAccount account details (b2b_b2c config field),
            // not per-transaction data.
            b2b_b2c,
            // There is no separate "items-only" value in the UCS purchase
            // model; the order total doubles as the item value.
            item_value: Some(total_value),
            total_value,
            // Installment count rides in `connector_feature_data.installments`
            // and is relayed both here and on `payments[0].installments`.
            number_of_installments: feature_data.installments,
            ip,
            is_gift: feature_data.is_gift,
            gift_message: feature_data.gift_message.clone(),
            observation: feature_data.observation.clone(),
            status: feature_data.status,
            origin,
            channel_id,
            reservation_date: feature_data.reservation_date.clone(),
            // Brazilian-only product: ClearSale expects the *names* here (its
            // documented vocabulary), not an ISO code, and defaults the pair
            // to Brasil/Brasileiro when the billing address carries no
            // country. The per-address `country` already carries the real
            // value; this is only the order-level fallback.
            country: country.clone().or_else(|| Some("Brasil".to_owned())),
            nationality: country.or_else(|| Some("Brasileiro".to_owned())),
            product,
            custom_sla: feature_data.custom_sla,
            bank_authentication: None,
            billing,
            shipping,
            payments,
            items,
            // Travel / hospitality blocks (Buser passengers, connections,
            // hotels) come from the typed `domain_data` on the pre risk check
            // request (projected by `ClearSaleTravelData::from_domain_data`).
            // The social block comes from connector_feature_data / metadata;
            // `list` and `purchaseInformation` have no carrier and stay off
            // the wire.
            list: None,
            purchase_information: None,
            social_network: feature_data.social_network.as_ref().map(|social| {
                ClearSaleSocialNetwork {
                    opt_in_compre_confie: social.opt_in_compre_confie,
                    type_social_network: social.type_social_network,
                    authentication_token: social.authentication_token.clone().map(Secret::new),
                }
            }),
            passengers: travel_data
                .passengers
                .as_ref()
                .map(|list| list.iter().filter_map(passenger_to_clear_sale).collect()),
            connections: travel_data.travel_segments.as_ref().map(|list| {
                list.iter()
                    .filter_map(travel_data_to_clear_sale_connection)
                    .collect()
            }),
            hotels: travel_data.accommodations.as_ref().map(|list| {
                list.iter()
                    .filter_map(accommodation_to_clear_sale)
                    .collect()
            }),
        },
    }
}

/// Signals ClearSale requires that the FRM request has no first-class field
/// for, carried in `connector_feature_data` JSON:
/// - `primaryDocument` CPF/CNPJ per person (spec §9)
/// - `origin` / `channelID` when the merchant tracks them
/// - order metadata the merchant tracks per transaction: `isGift`,
///   `giftMessage`, `observation`, `status`, `reservationDate`, `customSla`
///   and the `socialNetwork` block (Compre e Confie opt-in).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearSaleFeatureData {
    /// `sessionID` (mandatory, from the Behavior Analytics SDK — spec §10).
    /// Not deserializable from `connector_feature_data` (`skip`): it is filled
    /// only from the request `metadata` (`sessionId`) by
    /// [`Self::merged_with_euler_data`].
    #[serde(skip)]
    pub session_id: Option<String>,
    /// Billing `primaryDocument` CPF/CNPJ — filled ONLY from the typed
    /// [`PreRiskCheckRequest::document_details`] field (`DocumentKind::Cpf`/`Cnpj`)
    /// by [`Self::merged_with_euler_data`] (`skip`: single source of truth).
    #[serde(skip)]
    pub billing_document: Option<String>,
    /// Shipping `primaryDocument` — filled from `DocumentKind::CustomerCpf` on
    /// [`PreRiskCheckRequest::document_details`], falling back to the billing
    /// document by [`Self::merged_with_euler_data`].
    #[serde(skip)]
    pub shipping_document: Option<String>,
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
    /// Initial order status: 0 new, 9 approved, 41 cancelled by the
    /// customer, 45 denied — orders sent with 9/41/45 are not analyzed.
    #[serde(default)]
    pub status: Option<i32>,
    #[serde(default, rename = "reservationDate")]
    pub reservation_date: Option<String>,
    /// Custom SLA for the analysis decision, in seconds.
    #[serde(default, rename = "customSla")]
    pub custom_sla: Option<i32>,
    /// Compre e Confie opt-in block, merchant-supplied.
    #[serde(default)]
    pub social_network: Option<ClearSaleSocialNetworkFeature>,
    /// Phone kind for `billing.phones[].type` (1=Home, 2=Work, 6=Mobile per
    /// spec §5.1). Defaults to 1 when absent — merchants on a mobile-first
    /// checkout pass `6` here.
    #[serde(default)]
    pub billing_phone_type: Option<i32>,
    /// Same, for `shipping.phones[].type`.
    #[serde(default)]
    pub shipping_phone_type: Option<i32>,
    /// `payments[0].installments` — the card installment count (order-level
    /// `numberOfInstallments` derives from this).
    #[serde(default)]
    pub installments: Option<i32>,
    /// Per-request override for the ClearSale `product` id — merchants on a
    /// product other than Total Garantido pass their product id here
    /// (otherwise [`DEFAULT_CLEARSALE_PRODUCT_TOTAL_GARANTIDO`] or the
    /// account-level config applies).
    #[serde(default)]
    pub product: Option<i32>,
}

/// The three travel/hospitality blocks ClearSale maps onto its `passengers[]`
/// / `connections[]` / `hotels[]`, projected from the typed
/// `PreRiskCheckRequest::domain_data` (ground-travel `TravelData` legs +
/// `HotelData`; built by [`ClearSaleTravelData::from_domain_data`]).
#[derive(Debug, Clone, Default)]
pub struct ClearSaleTravelData {
    /// Travel/identity passengers (Buser per-seat travelers) — the
    /// concatenation of every leg's passengers.
    pub passengers: Option<Vec<Traveller>>,
    /// Travel legs (Buser `connections[]`) — one entry per `travel_data` row.
    pub travel_segments: Option<Vec<TravelData>>,
    /// Lodging blocks (`hotels[]`).
    pub accommodations: Option<Vec<HotelData>>,
}

impl ClearSaleTravelData {
    /// Project the typed [`DomainData`] onto the ClearSale blocks: every
    /// `travel_data` row is one connection and its passengers are appended to
    /// the order-level passenger list. A request with no travel/hotel blocks
    /// yields the default (every list `None`), which reads as "not a travel
    /// order".
    fn from_domain_data(domain_data: Option<&DomainData>) -> Self {
        let Some(domain_data) = domain_data else {
            return Self::default();
        };
        let passengers: Vec<Traveller> = domain_data
            .travel_data
            .iter()
            .flat_map(|leg| leg.passengers.clone())
            .collect();
        Self {
            passengers: (!passengers.is_empty()).then_some(passengers),
            travel_segments: (!domain_data.travel_data.is_empty())
                .then(|| domain_data.travel_data.clone()),
            accommodations: (!domain_data.hotel_data.is_empty())
                .then(|| domain_data.hotel_data.clone()),
        }
    }
}

// ClearSale-specific projections of the generic domain travel types (defined
// in `domain_types::connector_types`) onto the ClearSale wire contracts. Kept
// as free functions rather than inherent methods because the generic types
// live outside this crate (orphan rule).

/// ClearSale `passengers[]` source; the only mandatory field is `name` —
/// entries without a usable first/last name are dropped.
fn passenger_to_clear_sale(p: &Traveller) -> Option<ClearSalePassenger> {
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
        mile_card: p.loyalty_card.as_ref().map(|card| card.peek().clone()),
        // ClearSale identification type enum (spec): 1=CPF, 2=CNPJ, 3=RG, 4=IE,
        // 5=Passport, 6=CTPS, 7=Voter ID. PSN/Other have no ClearSale code.
        identification_type: p.document.as_ref().and_then(|doc| match doc.document_type {
            DocumentKind::Cpf | DocumentKind::CustomerCpf => Some(1),
            DocumentKind::Cnpj => Some(2),
            DocumentKind::Other => Some(5),
            DocumentKind::Psn => None,
        }),
        identification_number: p.document.as_ref().and_then(|doc| {
            let number = doc.document_number.peek().trim();
            (!number.is_empty()).then(|| Secret::new(number.to_owned()))
        }),
        gender: p.gender.clone(),
        birthdate: p.date_of_birth.as_ref().map(|dob| dob.peek().clone()),
        // CPF rides in the identificationNumber; no separate cpf carrier
        // exists in the typed domain data.
        cpf: None,
    })
}

/// ClearSale `connections[]` source; `company`, `origin`, `destination` are
/// mandatory — an entry missing any is dropped.
fn travel_data_to_clear_sale_connection(s: &TravelData) -> Option<ClearSaleConnection> {
    // Parse the service number to i64; skip if it does not parse
    let identification_number = s
        .service_number
        .as_deref()
        .and_then(|f| f.parse::<i64>().ok());
    // Fall back: departure → travel date → boarding → fail
    let date = s
        .departure_time
        .clone()
        .or_else(|| s.travel_date.clone())
        .or_else(|| s.boarding_time.clone())?;
    // seat_class overrides the plain travel_class when both are present.
    Some(ClearSaleConnection {
        company: s
            .carrier_code
            .clone()
            .or_else(|| s.carrier_name.clone())?
            .trim()
            .to_owned(),
        identification_number,
        date: Some(date),
        seat_class: s.seat_class.clone().or_else(|| s.travel_class.clone()),
        origin: s.origin.as_deref()?.trim().to_owned(),
        destination: s.destination.as_deref()?.trim().to_owned(),
        boarding: s.boarding_time.clone(),
        arriving: s.arrival_time.clone(),
        fare_class: s.fare_class.clone(),
    })
}

/// ClearSale `hotels[]` source; `name` is mandatory.
fn accommodation_to_clear_sale(a: &HotelData) -> Option<ClearSaleHotel> {
    Some(ClearSaleHotel {
        name: a.hotel_name.as_deref()?.trim().to_owned(),
        city: a.hotel_city.clone(),
        state: a.hotel_state.clone(),
        country: a.hotel_country.clone(),
        reservation_date: a.reservation_date.clone(),
        reserve_expiration_date: a.reservation_expiration_date.clone(),
        check_in_date: a.checkin.clone(),
        check_out_date: a.checkout.clone(),
    })
}

/// Merchant-supplied `socialNetwork` block carried in
/// `connector_feature_data` (ClearSale's Compre e Confie program).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearSaleSocialNetworkFeature {
    #[serde(default, rename = "optInCompreConfie")]
    pub opt_in_compre_confie: Option<i32>,
    #[serde(default, rename = "typeSocialNetwork")]
    pub type_social_network: Option<i32>,
    #[serde(default, rename = "authenticationToken")]
    pub authentication_token: Option<String>,
}

impl ClearSaleFeatureData {
    pub fn parse(raw: Option<&Secret<String>>) -> Self {
        raw.and_then(|data| serde_json::from_str(data.peek()).ok())
            .unwrap_or_default()
    }

    /// Merges the request `metadata` JSON (`{"sessionId","customFields"}` — the
    /// merchant-supplied extras ClearSale rides on) and the typed
    /// `document_details` field into the feature data.
    ///
    /// **Documents come ONLY from the typed field** — single source of truth.
    /// `DocumentKind::Cpf`/`Cnpj` → billing, `DocumentKind::CustomerCpf` →
    /// shipping (falling back to the billing document when absent — a bus-ticket
    /// order has one payer). Fields that have no proper ClearSale mapping are
    /// left `None`.
    fn merged_with_euler_data(
        self,
        metadata: Option<&Secret<String>>,
        document_details: Option<&CustomerDocumentDetails>,
    ) -> Self {
        // euler encodes the metadata JSON in snake_case (`UcsFrmMetadata` uses
        // `camelTo2 '_'` for its wire keys) — `FrmMetadata` carries snake_case
        // `alias`es on every field so both casings decode into it.
        let mut billing_document = None;
        let mut shipping_document = None;
        if let Some(doc) = document_details {
            let number = doc.document_number.peek().clone();
            match doc.document_type {
                DocumentKind::Cpf | DocumentKind::Cnpj => billing_document = Some(number),
                DocumentKind::CustomerCpf => shipping_document = Some(number),
                DocumentKind::Psn | DocumentKind::Other => {}
            }
        }
        let shipping_document = shipping_document.or_else(|| billing_document.clone());

        // Decode the generic FRM metadata once — shared with every FRM
        // connector, not ClearSale-specific.
        let parsed = FrmMetadata::parse(metadata);
        if parsed.session_id.is_none() && parsed.custom_fields.is_none() {
            return Self {
                billing_document,
                shipping_document,
                ..self
            };
        }

        let FrmMetadata {
            session_id: parsed_session_id,
            custom_fields,
            ..
        } = parsed;
        // Read a string custom field; nested JSON values are ignored.
        let custom_str = |key: &str| -> Option<String> {
            custom_fields
                .as_ref()
                .and_then(|fields| fields.get(key))
                .and_then(|value| value.as_str())
                .map(str::to_owned)
        };

        Self {
            // Single source: the request `metadata` (`sessionId`).
            // `connector_feature_data` cannot carry one (`#[serde(skip)]`).
            session_id: parsed_session_id,
            billing_document,
            shipping_document,
            origin: self.origin.or_else(|| custom_str("origin")),
            channel_id: self.channel_id.or_else(|| custom_str("channelId")),
            is_gift: self
                .is_gift
                .or_else(|| custom_str("isGift").map(|v| v == "true")),
            gift_message: self.gift_message.or_else(|| custom_str("giftMessage")),
            observation: self.observation.or_else(|| custom_str("observation")),
            status: self
                .status
                .or_else(|| custom_str("status").and_then(|v| v.parse::<i32>().ok())),
            reservation_date: self
                .reservation_date
                .or_else(|| custom_str("reservationDate")),
            custom_sla: self
                .custom_sla
                .or_else(|| custom_str("customSla").and_then(|v| v.parse::<i32>().ok())),
            billing_phone_type: self
                .billing_phone_type
                .or_else(|| custom_str("billingPhoneType").and_then(|v| v.parse::<i32>().ok())),
            shipping_phone_type: self
                .shipping_phone_type
                .or_else(|| custom_str("shippingPhoneType").and_then(|v| v.parse::<i32>().ok())),
            installments: self
                .installments
                .or_else(|| custom_str("installments").and_then(|v| v.parse::<i32>().ok())),
            product: self
                .product
                .or_else(|| custom_str("product").and_then(|v| v.parse::<i32>().ok())),
            social_network: self.social_network.or_else(|| {
                let opt_in = custom_str("optInCompreConfie")
                    .and_then(|v| v.parse::<i32>().ok())
                    .or_else(|| {
                        custom_fields.as_ref().and_then(|fields| {
                            fields
                                .get("socialNetwork")
                                .and_then(|value| value.get("optInCompreConfie"))
                                .and_then(serde_json::Value::as_i64)
                                .and_then(|v| i32::try_from(v).ok())
                        })
                    });
                Some(ClearSaleSocialNetworkFeature {
                    opt_in_compre_confie: Some(opt_in?),
                    type_social_network: custom_str("typeSocialNetwork")
                        .and_then(|v| v.parse::<i32>().ok())
                        .or_else(|| {
                            custom_fields.as_ref().and_then(|fields| {
                                fields
                                    .get("socialNetwork")
                                    .and_then(|value| value.get("typeSocialNetwork"))
                                    .and_then(serde_json::Value::as_i64)
                                    .and_then(|v| i32::try_from(v).ok())
                            })
                        }),
                    authentication_token: custom_str("authenticationToken"),
                })
            }),
        }
    }
}

/// Shared order construction for both risk checks. Every ClearSale-mandatory
/// field is validated up front; all missing fields are reported in one
/// `MissingRequiredFields` error rather than one round-trip per missing
/// field. Synthesizable fields (address line splits, phone DDD, synthetic
/// line item) never fail.
fn order_from_common(inputs: OrderInputs<'_>) -> Result<ClearSaleOrderCreateRequest, Error> {
    let OrderInputs {
        code,
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
        travel_data,
    } = inputs;

    let mut missing: Vec<&'static str> = Vec::new();

    let session_id = feature_data.session_id.clone().unwrap_or_else(|| {
        missing.push("sessionID");
        String::new()
    });
    if email.is_none() {
        missing.push("email");
    }

    let total_value = super::ClearSaleBrAmountConvertor::convert(money.amount, money.currency)?;

    let billing = build_person(
        "billing",
        address.and_then(|addr| addr.get_payment_billing()),
        customer_info,
        feature_data.billing_document.as_ref(),
        feature_data,
        &mut missing,
    );
    // A bus-ticket-style digital order has no separate shipping recipient;
    // when the shipping address is absent the billing block is mirrored so
    // the required `shipping` object is still populated.
    let shipping = build_person(
        "shipping",
        address.and_then(|addr| addr.get_shipping()),
        customer_info,
        feature_data
            .shipping_document
            .as_ref()
            .or(feature_data.billing_document.as_ref()),
        feature_data,
        &mut missing,
    );

    // Format the order date once so the payment entries share the
    // transaction instant (spec §5.1 `payments[].date`). `date_time::now`
    // is the one sanctioned clock read.
    let now = common_utils::date_time::now()
        .assume_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default();
    let payments = vec![build_clear_sale_payment(
        payment_method,
        total_value,
        &now,
        feature_data.installments,
        billing
            .as_ref()
            .map(|person| person.primary_document.clone()),
        money.currency,
    )];
    let items = build_clear_sale_items(
        order_details,
        money.currency,
        total_value,
        feature_data.is_gift,
    )?;

    if !missing.is_empty() {
        return Err(error_stack::report!(
            errors::IntegrationError::MissingRequiredFields {
                field_names: missing,
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "ClearSale requires order code, sessionID, email and fully-populated \
                         billing/shipping/payments/items — the listed fields were absent or \
                         empty"
                            .to_owned(),
                    ),
                    suggested_action: Some(
                        "sessionID comes from ClearSale's Behavior Analytics SDK and must be \
                         passed as `sessionId` in the request metadata; CPF/CNPJ documents \
                         come from the typed `billing_document` / `shipping_document` fields \
                         on the risk-check request"
                            .to_owned(),
                    ),
                    doc_url: Some(CLEARSALE_DOC_URL.to_owned()),
                },
            }
        ));
    }

    // Both were guarded above; re-extracting via `if let` keeps this
    // panic-free.
    let (Some(email), Some(billing), Some(shipping)) = (email, billing, shipping) else {
        return Err(error_stack::report!(
            errors::IntegrationError::MissingRequiredFields {
                field_names: vec!["email", "billing", "shipping"],
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "unreachable: the missing-field guard above must have fired".to_owned(),
                    ),
                    suggested_action: None,
                    doc_url: Some(CLEARSALE_DOC_URL.to_owned()),
                },
            }
        ));
    };

    // BR-specific APIs: the billing address country feeds the order
    // `country`/`nationality` fields, defaulting to "BR" in
    // `build_order_request` when absent.
    let billing_country = address
        .and_then(|addr| addr.get_payment_billing())
        .and_then(|addr| addr.address.as_ref())
        .and_then(|details| details.country.map(|c| c.to_string()));

    Ok(build_order_request(
        code,
        session_id,
        email,
        total_value,
        billing,
        shipping,
        payments,
        items,
        ip,
        feature_data.origin.clone(),
        feature_data.channel_id.clone(),
        b2b_b2c,
        // Per-request `connector_feature_data.product` wins, then the
        // account-level connector config, then the Total Garantido default.
        feature_data
            .product
            .or(config_product)
            .unwrap_or(DEFAULT_CLEARSALE_PRODUCT_TOTAL_GARANTIDO),
        billing_country,
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
        let code = req.merchant_transaction_id.clone().ok_or_else(|| {
            error_stack::report!(errors::IntegrationError::MissingRequiredField {
                field_name: "merchant_transaction_id",
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "ClearSale keys the order by `code`; the merchant transaction id is \
                         used as that code and for webhook correlation"
                            .to_owned(),
                    ),
                    suggested_action: Some(
                        "Set merchant_transaction_id on the FRM Pre Risk Check request".to_owned(),
                    ),
                    doc_url: Some(CLEARSALE_DOC_URL.to_owned()),
                },
            })
        })?;
        let feature_data = ClearSaleFeatureData::parse(req.connector_feature_data.as_ref())
            .merged_with_euler_data(req.metadata.as_ref(), req.document_details.as_ref());
        order_from_common(OrderInputs::from_pre_risk_check(
            req,
            &feature_data,
            b2b_b2c_from_config(&item.router_data.connector_config),
            product_from_config(&item.router_data.connector_config),
            code,
        ))
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
        // PUT /v1/orders/ re-analysis takes the same body as the POST (spec §4).
        let req = &item.router_data.request;
        let code = req
            .merchant_transaction_id
            .clone()
            .or_else(|| req.connector_transaction_id.clone())
            .ok_or_else(|| {
                error_stack::report!(errors::IntegrationError::MissingRequiredField {
                    field_name: "merchant_transaction_id",
                    context: errors::IntegrationErrorContext {
                        additional_context: Some(
                            "ClearSale re-analysis (PUT /v1/orders/) addresses the order by \
                             the same `code` used to create it"
                                .to_owned(),
                        ),
                        suggested_action: Some(
                            "Send merchant_transaction_id or connector_transaction_id on the \
                             FRM Post Risk Check request"
                                .to_owned(),
                        ),
                        doc_url: Some(CLEARSALE_DOC_URL.to_owned()),
                    },
                })
            })?;
        // PostRiskCheck carries no typed billing/shipping documents — the
        // re-analysis replays the original order body; any document overrides
        // ride on the merchant's `connector_feature_data`, which
        // `ClearSaleFeatureData::parse` already reads.
        let feature_data = ClearSaleFeatureData::parse(req.connector_feature_data.as_ref())
            .merged_with_euler_data(req.metadata.as_ref(), None);
        order_from_common(OrderInputs::from_post_risk_check(
            req,
            &feature_data,
            b2b_b2c_from_config(&item.router_data.connector_config),
            product_from_config(&item.router_data.connector_config),
            code,
        ))
        .map(ClearSaleOrderReanalysisRequest)
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Responses — order create / re-analysis
// ──────────────────────────────────────────────────────────────────────────

/// The first (and normally only) per-order status entry, with the code echoed
/// back as the FRM transaction reference. ClearSale's score is a float
/// (e.g. `99.99`); UCS carries an integer risk score, so it is truncated
/// (explicitly allowed, same as Kount's omniscore conversion).
fn first_order_decision(
    response: &ClearSaleOrderCreateResponse,
) -> (Option<FrmDecision>, Option<i32>, Option<String>) {
    response.orders.first().map_or((None, None, None), |order| {
        (
            Some(order.status.to_frm_decision()),
            #[allow(clippy::as_conversions)]
            order.score.map(|score| score.trunc() as i32),
            Some(order.code.clone()),
        )
    })
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
                // A response with an empty `orders` array yields no decision;
                // fail safe to review rather than approve.
                frm_decision: Some(frm_decision.unwrap_or(FrmDecision::Review)),
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
                frm_decision: Some(frm_decision.unwrap_or(FrmDecision::Review)),
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

// ──────────────────────────────────────────────────────────────────────────
// FrmPaymentOutcome — PUT /v1/orders/{code}/status (spec §5.4)
// ──────────────────────────────────────────────────────────────────────────

/// Map a payment outcome onto ClearSale's merchant-push vocabulary.
/// `PGA` = payment approved, `PGR` = payment denied.
fn clearsale_payment_outcome_status(
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
        Ok(Self {
            response: Ok(FrmPaymentOutcomeResponse {
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;
    use std::str::FromStr;

    /// The verdict mapping decides whether a payment proceeds, so every
    /// documented status is pinned — in particular that approval comes only
    /// from the four "Aprovação" statuses (spec §6).
    #[test]
    fn clearsale_status_maps_fail_safe() {
        use ClearSaleStatusEnum as S;
        for status in [S::Apa, S::Apm, S::App, S::Apg] {
            assert_eq!(status.to_frm_decision(), FrmDecision::Approve);
        }
        for status in [S::Rpa, S::Rpm, S::Rpp, S::Can, S::Frd] {
            assert_eq!(status.to_frm_decision(), FrmDecision::Reject);
        }
        for status in [
            S::Nvo,
            S::Ama,
            S::Sus,
            S::Err,
            S::Amm,
            S::Apr,
            S::Rea,
            S::Rep,
            S::Unknown,
        ] {
            assert_eq!(
                status.to_frm_decision(),
                FrmDecision::Review,
                "{status:?} must hold for review, never approve nor reject"
            );
        }
    }

    /// An unrecognised status string must deserialize to `Unknown` (then
    /// review) rather than fail the whole response.
    #[test]
    fn unknown_status_deserializes() {
        let parsed =
            serde_json::from_str::<ClearSaleStatusEnum>("\"XYZ\"").expect("serde(other) fallback");
        assert_eq!(parsed, ClearSaleStatusEnum::Unknown);
        assert_eq!(parsed.to_frm_decision(), FrmDecision::Review);
    }

    /// ClearSale sends 3-letter statuses UPPERCASE on the wire (spec §5.2/§5.3).
    #[test]
    fn uppercase_wire_statuses_deserialize() {
        use ClearSaleStatusEnum as S;
        for (wire, variant) in [
            ("NVO", S::Nvo),
            ("AMA", S::Ama),
            ("SUS", S::Sus),
            ("APA", S::Apa),
            ("APM", S::Apm),
            ("APP", S::App),
            ("APG", S::Apg),
            ("RPA", S::Rpa),
            ("RPM", S::Rpm),
            ("RPP", S::Rpp),
            ("CAN", S::Can),
            ("FRD", S::Frd),
            ("ERR", S::Err),
        ] {
            let parsed = serde_json::from_str::<ClearSaleStatusEnum>(&format!("\"{wire}\""))
                .expect("uppercase wire status must deserialize");
            assert_eq!(parsed, variant, "wire status {wire}");
        }
        assert_eq!(
            serde_json::from_str::<ClearSaleStatusEnum>("\"NVO\"")
                .expect("NVO")
                .to_frm_decision(),
            FrmDecision::Review
        );
        assert_eq!(
            serde_json::from_str::<ClearSaleStatusEnum>("\"APA\"")
                .expect("APA")
                .to_frm_decision(),
            FrmDecision::Approve
        );
        assert_eq!(
            serde_json::from_str::<ClearSaleStatusEnum>("\"RPA\"")
                .expect("RPA")
                .to_frm_decision(),
            FrmDecision::Reject
        );
    }

    /// Only a captured payment pushes PGA, and only a ClearSale rejection or
    /// an explicit failure pushes PGR.
    #[test]
    fn payment_outcome_maps_to_pga_pgr() {
        assert_eq!(
            clearsale_payment_outcome_status(Some(AttemptStatus::Charged), None),
            Some("PGA")
        );
        assert_eq!(
            clearsale_payment_outcome_status(Some(AttemptStatus::AuthorizationFailed), None),
            Some("PGR")
        );
        assert_eq!(
            clearsale_payment_outcome_status(None, Some(FrmDecision::Reject)),
            Some("PGR")
        );
        assert_eq!(
            clearsale_payment_outcome_status(None, Some(FrmDecision::Approve)),
            None
        );
        assert_eq!(clearsale_payment_outcome_status(None, None), None);
    }

    /// The serialized order must carry every field name from ClearSale's
    /// documented sample (spec §5.1) — with the optional blocks populated,
    /// so a rename or a missing `#[serde(rename)]` fails here instead of
    /// silently sending a wrong wire name to ClearSale.
    #[test]
    fn order_request_serializes_documented_wire_names() {
        let order = ClearSaleOrderRequest {
            code: "ORDER_EXAMPLE_2_0_1".to_string(),
            session_id: "SessionIDValue".to_string(),
            date: "2017-03-22T13:38:59.9894222".to_string(),
            email: common_utils::pii::Email::from_str("email@email.com.br").expect("valid email"),
            b2b_b2c: Some("B2C".to_string()),
            item_value: Some(FloatMajorUnit(10.00)),
            total_value: FloatMajorUnit(15.00),
            number_of_installments: Some(1),
            ip: Some("192.168.0.1".to_string()),
            is_gift: Some(false),
            gift_message: Some("Message Example".to_string()),
            observation: Some("Observation example".to_string()),
            status: Some(0),
            origin: Some("Origin example".to_string()),
            channel_id: Some("Additional origin information".to_string()),
            reservation_date: Some("2017-03-21T22:36:36.0000000".to_string()),
            country: Some("Brasil".to_string()),
            nationality: Some("Brasileiro".to_string()),
            product: DEFAULT_CLEARSALE_PRODUCT_TOTAL_GARANTIDO,
            custom_sla: Some(60),
            bank_authentication: Some("Aprovado 3DS".to_string()),
            billing: ClearSalePerson {
                person_type: 1,
                primary_document: Secret::new("12345678910".to_string()),
                secondary_document: Some(Secret::new("12345678".to_string())),
                client_id: Some("Client123".to_string()),
                name: Secret::new("Complete Client Name".to_string()),
                birth_date: Some("1990-01-10T00:00:00.000".to_string()),
                email: Some(
                    common_utils::pii::Email::from_str("email@email.com.br").expect("valid email"),
                ),
                gender: Some("M".to_string()),
                address: ClearSaleAddress {
                    street: Secret::new("Street name example".to_string()),
                    number: "100".to_string(),
                    county: Secret::new("County Example".to_string()),
                    city: Secret::new("City Example".to_string()),
                    state: Secret::new("SP".to_string()),
                    zipcode: Secret::new("12345678".to_string()),
                    country: Some("Brasil".to_string()),
                    additional_information: Some(Secret::new(
                        "Additional information example".to_string(),
                    )),
                    reference: Some(Secret::new("Address reference example".to_string())),
                },
                phones: vec![ClearSalePhone {
                    phone_type: 1,
                    ddi: Some(55),
                    ddd: 11,
                    number: 33333333,
                    extension: Some("1111".to_string()),
                }],
                delivery_type: None,
                delivery_time: None,
                price: None,
                pick_up_store_document: None,
            },
            shipping: ClearSalePerson {
                person_type: 1,
                primary_document: Secret::new("12345678910".to_string()),
                secondary_document: None,
                client_id: None,
                name: Secret::new("Complete Client Name".to_string()),
                birth_date: None,
                email: None,
                gender: None,
                address: ClearSaleAddress {
                    street: Secret::new("Street name example".to_string()),
                    number: "100".to_string(),
                    county: Secret::new("County Example".to_string()),
                    city: Secret::new("City Example".to_string()),
                    state: Secret::new("SP".to_string()),
                    zipcode: Secret::new("12345678".to_string()),
                    country: Some("Brasil".to_string()),
                    additional_information: None,
                    reference: None,
                },
                phones: vec![ClearSalePhone {
                    phone_type: 1,
                    ddi: None,
                    ddd: 11,
                    number: 33333333,
                    extension: None,
                }],
                delivery_type: Some("1".to_string()),
                delivery_time: Some("2 dias úteis".to_string()),
                price: Some(FloatMajorUnit(5.00)),
                pick_up_store_document: Some(Secret::new("12345678910".to_string())),
            },
            payments: vec![ClearSalePayment {
                payment_type: 1,
                value: FloatMajorUnit(25.00),
                date: Some("2017-03-21T22:36:53.0000000".to_string()),
                currency: Some(986),
                sequential: Some(1),
                installments: Some(1),
                interest_rate: Some(FloatMajorUnit(10.0)),
                interest_value: Some(FloatMajorUnit(2.00)),
                card: Some(ClearSaleCard {
                    bin: Secret::new("123456".to_string()),
                    end: Secret::new("1234".to_string()),
                    owner_name: Secret::new("Owner Card Name".to_string()),
                    hash: Some(Secret::new("12345678945612301234569874563210".to_string())),
                    card_type: Some(1),
                    validity_date: Some("02/2021".to_string()),
                    document: Some(Secret::new("12345678910".to_string())),
                    nsu: Some("12345".to_string()),
                }),
                voucher_order_origin: Some("123456".to_string()),
                address: None,
            }],
            items: vec![ClearSaleItem {
                name: "Item description".to_string(),
                code: Some("Item Code".to_string()),
                bar_code: None,
                value: Some(FloatMajorUnit(10.00)),
                amount: Some(1),
                category_id: Some(1),
                category_name: Some("Item category name".to_string()),
                is_gift: Some(true),
                seller_name: Some("Seller Name".to_string()),
                seller_document: None,
                is_market_place: None,
                seller_segment: None,
                shipping_company: Some("Shipping Company Name".to_string()),
            }],
            list: Some(ClearSaleList {
                type_id: 3,
                id: "List Code Example".to_string(),
            }),
            purchase_information: Some(ClearSalePurchaseInformation {
                last_date_inserted_mail: Some("2015-03-01T02:40:00".to_string()),
                last_date_change_password: Some("2015-04-02T05:15:00".to_string()),
                last_date_change_phone: Some("2015-05-03T10:45:00".to_string()),
                last_date_change_mobile_phone: Some("2015-06-04T12:05:00".to_string()),
                last_date_inserted_address: Some("2015-07-05T15:25:00".to_string()),
                purchase_logged: Some(false),
                email: Some(
                    common_utils::pii::Email::from_str("email@pemail.com.br").expect("valid email"),
                ),
                login: Some("SocialNetworkLogin".to_string()),
            }),
            social_network: Some(ClearSaleSocialNetwork {
                opt_in_compre_confie: Some(1),
                type_social_network: Some(2),
                authentication_token: Some(Secret::new("123456abcd".to_string())),
            }),
            passengers: Some(vec![ClearSalePassenger {
                name: Secret::new("Passenger name".to_string()),
                company_mile_card: Some("Company Name".to_string()),
                mile_card: Some("123456789".to_string()),
                identification_type: Some(1),
                identification_number: Some(Secret::new("123456789".to_string())),
                gender: Some("M".to_string()),
                birthdate: Some("1990-01-10T00:00:00.000".to_string()),
                cpf: Some(Secret::new("12345678910".to_string())),
            }]),
            connections: Some(vec![ClearSaleConnection {
                company: "JJ".to_string(),
                identification_number: Some(12345),
                date: Some("2017-10-10T00:00:00.000".to_string()),
                seat_class: Some("Seat Class".to_string()),
                origin: "GRU".to_string(),
                destination: "LHR".to_string(),
                boarding: Some("2017-10-10T00:00:00.000".to_string()),
                arriving: Some("2017-10-10T00:00:00.000".to_string()),
                fare_class: Some("First Class".to_string()),
            }]),
            hotels: Some(vec![ClearSaleHotel {
                name: "Hotel Name".to_string(),
                city: Some("Hotel City".to_string()),
                state: Some("Hotel State".to_string()),
                country: Some("Hotel Country".to_string()),
                reservation_date: Some("2017-10-10T00:00:00.000".to_string()),
                reserve_expiration_date: Some("2017-10-10T00:00:00.000".to_string()),
                check_in_date: Some("2017-10-10T00:00:00.000".to_string()),
                check_out_date: Some("2017-10-10T00:00:00.000".to_string()),
            }]),
        };

        let json = serde_json::to_value(&order).expect("order serializes");
        // Every wire name from the documented sample must be present
        // (spec §5.1). Pinned as a flat list so a missing rename is a
        // one-line diff here.
        let expected_wire_names = [
            "code",
            "sessionID",
            "date",
            "email",
            "b2bB2c",
            "itemValue",
            "totalValue",
            "numberOfInstallments",
            "ip",
            "isGift",
            "giftMessage",
            "observation",
            "status",
            "origin",
            "channelID",
            "reservationDate",
            "country",
            "nationality",
            "product",
            "customSla",
            "bankAuthentication",
            "billing",
            "shipping",
            "payments",
            "items",
            "list",
            "purchaseInformation",
            "socialNetwork",
            "passengers",
            "connections",
            "hotels",
        ];
        let object = json.as_object().expect("order is a JSON object");
        for name in expected_wire_names {
            assert!(
                object.contains_key(name),
                "wire field {name} missing from serialized order"
            );
        }
        // Person-level and payment-level wire names.
        for block in ["billing", "shipping"] {
            let person = object[block].as_object().expect("person block");
            for name in ["type", "primaryDocument", "name", "address", "phones"] {
                assert!(
                    person.contains_key(name),
                    "wire field {name} missing from {block}"
                );
            }
        }
        let payment = object["payments"][0].as_object().expect("payment entry");
        for name in [
            "type",
            "value",
            "date",
            "currency",
            "sequential",
            "installments",
            "interestRate",
            "interestValue",
            "card",
            "voucherOrderOrigin",
        ] {
            assert!(
                payment.contains_key(name),
                "wire field {name} missing from payments[0]"
            );
        }
    }

    /// The `metadata` JSON (`{"sessionId","customFields"}`) fills the
    /// merchant-supplied extras; documents come only from the typed
    /// `identity_credentials` field.
    #[test]
    fn metadata_fills_custom_fields_only() {
        // A `sessionID` smuggled into `connector_feature_data` is ignored by
        // design, while the metadata `sessionId` is the authoritative one —
        // this must stay true even before the merge test below.
        let feature_data = ClearSaleFeatureData::parse(Some(&Secret::new(
            r#"{"sessionID": "feature-data-session", "origin": "fd-origin"}"#.to_owned(),
        )));
        assert_eq!(feature_data.session_id, None);

        let metadata = r#"{
            "merchantOrderId": "order-123",
            "userIp": "10.0.0.1",
            "sessionId": "sdk-session-42",
            "customFields": {
                "origin": "WebCheckout",
                "isGift": "true",
                "giftMessage": "Happy birthday!",
                "status": "0",
                "reservationDate": "2025-01-15T00:00:00",
                "customSla": "3600",
                "socialNetwork": {
                    "optInCompreConfie": 1,
                    "typeSocialNetwork": 2
                }
            }
        }"#;
        let merged = ClearSaleFeatureData::default()
            .merged_with_euler_data(Some(&Secret::new(metadata.to_owned())), None);

        assert_eq!(merged.session_id.as_deref(), Some("sdk-session-42"));
        assert_eq!(merged.origin.as_deref(), Some("WebCheckout"));
        assert_eq!(merged.is_gift, Some(true));
        assert_eq!(merged.gift_message.as_deref(), Some("Happy birthday!"));
        assert_eq!(merged.status, Some(0));
        assert_eq!(
            merged.reservation_date.as_deref(),
            Some("2025-01-15T00:00:00")
        );
        assert_eq!(merged.custom_sla, Some(3600));
        let social = merged.social_network.expect("social block parsed");
        assert_eq!(social.opt_in_compre_confie, Some(1));
        assert_eq!(social.type_social_network, Some(2));

        // Without typed billing/shipping documents, the document fields stay
        // empty (None).
        assert_eq!(merged.billing_document, None);
        assert_eq!(merged.shipping_document, None);

        // euler's real wire shape is snake_case (`camelTo2 '_'` on
        // `UcsFrmMetadata`) — the same fields must decode identically.
        let euler_metadata = r#"{
            "merchant_order_id": "order-123",
            "user_ip": "10.0.0.1",
            "session_id": "sdk-session-42",
            "custom_fields": { "origin": "WebCheckout" }
        }"#;
        let merged = ClearSaleFeatureData::default()
            .merged_with_euler_data(Some(&Secret::new(euler_metadata.to_owned())), None);
        assert_eq!(merged.session_id.as_deref(), Some("sdk-session-42"));
        assert_eq!(merged.origin.as_deref(), Some("WebCheckout"));
    }

    /// A payer document (Cpf/Cnpj) alone fills both document fields (a
    /// single-payer order: shipping falls back to billing). A `CustomerCpf`
    /// document is the buyer and fills shipping only. Absent documents leave
    /// everything untouched.
    #[test]
    fn shipping_document_falls_back_to_billing() {
        let billing = CustomerDocumentDetails {
            document_type: DocumentKind::Cpf,
            document_number: Secret::new("11122233344".to_owned()),
        };
        let buyer = CustomerDocumentDetails {
            document_type: DocumentKind::CustomerCpf,
            document_number: Secret::new("55566677788".to_owned()),
        };
        let company = CustomerDocumentDetails {
            document_type: DocumentKind::Cnpj,
            document_number: Secret::new("12345678000199".to_owned()),
        };

        // Payer CPF: billing set, shipping falls back to billing.
        let merged = ClearSaleFeatureData::default().merged_with_euler_data(None, Some(&billing));
        assert_eq!(merged.billing_document.as_deref(), Some("11122233344"));
        assert_eq!(merged.shipping_document.as_deref(), Some("11122233344"));

        // Payer CNPJ: same routing as CPF.
        let merged = ClearSaleFeatureData::default().merged_with_euler_data(None, Some(&company));
        assert_eq!(merged.billing_document.as_deref(), Some("12345678000199"));
        assert_eq!(merged.shipping_document.as_deref(), Some("12345678000199"));

        // Buyer CUSTOMER_CPF: shipping only, billing stays empty.
        let merged = ClearSaleFeatureData::default().merged_with_euler_data(None, Some(&buyer));
        assert_eq!(merged.billing_document, None);
        assert_eq!(merged.shipping_document.as_deref(), Some("55566677788"));

        // No document: fields stay at their defaults.
        let untouched = ClearSaleFeatureData::default().merged_with_euler_data(None, None);
        assert_eq!(untouched, ClearSaleFeatureData::default());
    }

    /// Typed `document_details` is the single source for documents; metadata
    /// has no effect on documents.
    #[test]
    fn typed_documents_are_the_only_document_source() {
        let billing = CustomerDocumentDetails {
            document_type: DocumentKind::Cnpj,
            document_number: Secret::new("12345678000199".to_owned()),
        };
        let metadata = r#"{"identityCredentials": {"cpf": "99900011122"}}"#;

        let merged = ClearSaleFeatureData::default()
            .merged_with_euler_data(Some(&Secret::new(metadata.to_owned())), Some(&billing));
        assert_eq!(merged.billing_document.as_deref(), Some("12345678000199"));
        // shipping falls back to the billing document
        assert_eq!(merged.shipping_document.as_deref(), Some("12345678000199"));
        // A stray identity blob in the metadata JSON does not decode into the
        // generic FRM metadata parse — it stays inert.
        assert_eq!(merged.session_id, None);
    }

    /// Street/number split: a trailing number token is used, then `line3`,
    /// and when neither works the **whole `line1`** becomes `number` rather
    /// than hard-failing. Only a missing line1 still errors.
    #[test]
    fn line1_falls_back_to_number_when_not_parseable() {
        // Trailing token.
        let (street, number) =
            split_street_and_number(Some(&Secret::new("Rua Augusta, 1234".to_owned())), None)
                .unwrap();
        assert_eq!(street.peek(), "Rua Augusta");
        assert_eq!(number, "1234");

        // line3 carries the number.
        let (street, number) = split_street_and_number(
            Some(&Secret::new("Rua Augusta".to_owned())),
            Some(&Secret::new("567".to_owned())),
        )
        .unwrap();
        assert_eq!(street.peek(), "Rua Augusta");
        assert_eq!(number, "567");

        // Not parseable ("Girija Building") + non-number line3 ("8th Block"):
        // whole line1 becomes the number.
        let (street, number) = split_street_and_number(
            Some(&Secret::new("Girija Building".to_owned())),
            Some(&Secret::new("8th Block".to_owned())),
        )
        .unwrap();
        assert_eq!(street.peek(), "");
        assert_eq!(number, "Girija Building");

        // Only line1=number and nothing else → street empty, number=token.
        let (street, number) =
            split_street_and_number(Some(&Secret::new("1234".to_owned())), None).unwrap();
        assert_eq!(street.peek(), "");
        assert_eq!(number, "1234");

        // No line1 and no usable line3 still errors.
        assert!(split_street_and_number(None, None).is_err());
        assert!(split_street_and_number(None, Some(&Secret::new("8th Block".to_owned()))).is_err());
    }

    /// The typed `domain_data` travel/hotel blocks project onto the ClearSale
    /// `passengers[]`/`connections[]`/`hotels[]` wire fields: each
    /// `travel_data` row is one connection, all rows' passengers append into
    /// `passengers[]`. Requests without travel/hotel blocks yield no travel
    /// data at all.
    #[test]
    fn domain_data_travel_blocks_pass_through() {
        let leg = TravelData {
            carrier_code: Some("BUSER".to_owned()),
            service_number: Some("101".to_owned()),
            departure_time: Some("2025-06-01T08:00:00Z".to_owned()),
            boarding_time: Some("2025-06-01T08:00:00Z".to_owned()),
            arrival_time: Some("2025-06-01T12:00:00Z".to_owned()),
            origin: Some("SAO".to_owned()),
            destination: Some("RIO".to_owned()),
            seat_class: Some("ECON".to_owned()),
            fare_class: Some("Y".to_owned()),
            passengers: vec![Traveller {
                first_name: Some(Secret::new("Joao".to_owned())),
                last_name: Some(Secret::new("Silva".to_owned())),
                document: Some(CustomerDocumentDetails {
                    document_type: DocumentKind::Cpf,
                    document_number: Secret::new("11122233344".to_owned()),
                }),
                gender: Some("M".to_owned()),
                loyalty_card: Some(Secret::new("BC-99".to_owned())),
                loyalty_card_issuer: Some("BuserClub".to_owned()),
                ..Traveller::default()
            }],
            ..TravelData::default()
        };
        // Second leg: different origin/destination, its own passenger.
        let return_leg = TravelData {
            carrier_name: Some("Buser".to_owned()),
            travel_date: Some("2025-06-10".to_owned()),
            origin: Some("RIO".to_owned()),
            destination: Some("SAO".to_owned()),
            passengers: vec![Traveller {
                first_name: Some(Secret::new("Maria".to_owned())),
                ..Traveller::default()
            }],
            ..TravelData::default()
        };
        let domain_data = DomainData {
            travel_data: vec![leg, return_leg],
            hotel_data: vec![HotelData {
                hotel_name: Some("Hotel Rio".to_owned()),
                hotel_city: Some("Rio de Janeiro".to_owned()),
                hotel_state: Some("RJ".to_owned()),
                hotel_country: Some("BR".to_owned()),
                reservation_date: Some("2025-05-01T10:00:00Z".to_owned()),
                reservation_expiration_date: Some("2025-05-02T10:00:00Z".to_owned()),
                checkin: Some("2025-06-01".to_owned()),
                checkout: Some("2025-06-05".to_owned()),
                ..HotelData::default()
            }],
            ..DomainData::default()
        };

        let travel = ClearSaleTravelData::from_domain_data(Some(&domain_data));

        // Every leg's passengers append into the order-level list.
        let passengers = travel.passengers.expect("passengers projected");
        assert_eq!(passengers.len(), 2);
        let p = passenger_to_clear_sale(&passengers[0]).expect("passenger has a name");
        assert_eq!(p.name.peek(), "Joao Silva");
        assert_eq!(p.company_mile_card.as_deref(), Some("BuserClub"));
        assert_eq!(p.mile_card.as_deref(), Some("BC-99"));
        assert_eq!(p.identification_type, Some(1));
        assert_eq!(
            p.identification_number.as_ref().map(|s| s.peek().as_str()),
            Some("11122233344")
        );
        assert_eq!(p.gender.as_deref(), Some("M"));
        let p2 = passenger_to_clear_sale(&passengers[1]).expect("passenger has a name");
        assert_eq!(p2.name.peek(), "Maria");

        // One connection per travel_data row; carrier_name is a fallback for
        // carrier code, travel_date a fallback for the departure.
        let segments = travel.travel_segments.expect("segments projected");
        assert_eq!(segments.len(), 2);
        let c =
            travel_data_to_clear_sale_connection(&segments[0]).expect("required fields present");
        assert_eq!(c.company, "BUSER");
        assert_eq!(c.origin, "SAO");
        assert_eq!(c.destination, "RIO");
        assert_eq!(c.seat_class.as_deref(), Some("ECON"));
        let c2 =
            travel_data_to_clear_sale_connection(&segments[1]).expect("required fields present");
        assert_eq!(c2.company, "Buser");
        assert_eq!(c2.date.as_deref(), Some("2025-06-10"));
        assert_eq!(c2.origin, "RIO");
        assert_eq!(c2.destination, "SAO");

        let hotels = travel.accommodations.expect("hotel projected");
        let h = accommodation_to_clear_sale(&hotels[0]).expect("hotel name present");
        assert_eq!(h.name, "Hotel Rio");
        assert_eq!(h.check_in_date.as_deref(), Some("2025-06-01"));
        assert_eq!(h.check_out_date.as_deref(), Some("2025-06-05"));

        // No travel/hotel blocks at all → not a travel order.
        let bare = DomainData::default();
        let bare_travel = ClearSaleTravelData::from_domain_data(Some(&bare));
        assert!(bare_travel.passengers.is_none());
        assert!(bare_travel.travel_segments.is_none());
        assert!(bare_travel.accommodations.is_none());

        // Entries missing mandatory ClearSale fields are dropped, not emitted.
        let bad = Traveller::default();
        assert!(passenger_to_clear_sale(&bad).is_none());
        let bad_seg = TravelData {
            carrier_code: Some("BUSER".to_owned()),
            ..TravelData::default()
        };
        assert!(travel_data_to_clear_sale_connection(&bad_seg).is_none());
        let bad_hotel = HotelData::default();
        assert!(accommodation_to_clear_sale(&bad_hotel).is_none());

        // A leg with no passengers still projects as a connection; empty
        // lists (present but empty) yield None, not [].
        let passengerless = TravelData {
            origin: Some("SAO".to_owned()),
            destination: Some("RIO".to_owned()),
            carrier_code: Some("BUSER".to_owned()),
            departure_time: Some("2025-06-01T08:00:00Z".to_owned()),
            ..TravelData::default()
        };
        let leg_only = DomainData {
            travel_data: vec![passengerless],
            ..DomainData::default()
        };
        let leg_only_travel = ClearSaleTravelData::from_domain_data(Some(&leg_only));
        assert!(leg_only_travel.passengers.is_none());
        assert_eq!(
            leg_only_travel.travel_segments.as_ref().map(Vec::len),
            Some(1)
        );
    }
}
