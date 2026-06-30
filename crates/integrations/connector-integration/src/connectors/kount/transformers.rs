use common_enums::{AttemptStatus, FrmDecision};
use common_utils::types::StringMinorUnit;
use domain_types::{
    connector_flow::{
        FrmPaymentOutcome, FrmRefundProcessed, PreRiskCheck, ServerAuthenticationToken,
    },
    connector_types::{
        CustomerInfo, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
    },
    errors,
    frm::frm_types::{
        FrmFlowData, FrmPaymentOutcomeRequest, FrmPaymentOutcomeResponse,
        FrmRefundProcessedRequest, FrmRefundProcessedResponse, MandateInfo, PreRiskCheckRequest,
        PreRiskCheckResponse,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_address::Address,
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
};
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::{connectors::kount::KountRouterData, types::ResponseRouterData};

type Error = error_stack::Report<errors::IntegrationError>;
type ResponseError = error_stack::Report<errors::ConnectorError>;

/// OAuth scope required by the Kount Orders API.
const KOUNT_API_SCOPE: &str = "k1_integration_api";
/// Fallback values for an unparsable Kount error body.
const KOUNT_DEFAULT_ERROR_CODE: &str = "KOUNT_ERROR";
const KOUNT_DEFAULT_ERROR_MESSAGE: &str = "Kount request failed";
/// Kount developer documentation, surfaced in error contexts.
pub const KOUNT_DOC_URL: &str = "https://developer.kount.com/";

// ──────────────────────────────────────────────────────────────────────────
// Auth + error types
// ──────────────────────────────────────────────────────────────────────────

/// Kount auth. `api_key` is the base64 of `CLIENT_ID:CLIENT_SECRET` (Kount's
/// "API Key"); it is used directly as the `Authorization: Basic {api_key}`
/// value on the token request. `auth_server_id` is the account/environment
/// specific OAuth authorization-server id (sandbox vs production differ).
#[derive(Debug, Clone)]
pub struct KountAuthType {
    pub api_key: Secret<String>,
    pub auth_server_id: Option<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for KountAuthType {
    type Error = Error;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Kount {
                api_key,
                auth_server_id,
                ..
            } => Ok(Self {
                api_key: api_key.to_owned(),
                auth_server_id: auth_server_id.to_owned(),
            }),
            _ => Err(error_stack::report!(
                errors::IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext {
                        additional_context: Some(
                            "Kount expects ConnectorSpecificConfig::Kount with a base64 \
                             `CLIENT_ID:CLIENT_SECRET` api_key, but a different connector \
                             config variant was supplied"
                                .to_owned(),
                        ),
                        suggested_action: Some(
                            "Send the Kount connector config (api_key, optional auth_server_id) \
                             for Kount FRM flows"
                                .to_owned(),
                        ),
                        doc_url: Some(KOUNT_DOC_URL.to_owned()),
                    }
                }
            )),
        }
    }
}

/// Kount error body. Both fields are optional — a malformed/empty body falls back
/// to [`KOUNT_DEFAULT_ERROR_CODE`] / [`KOUNT_DEFAULT_ERROR_MESSAGE`] when the
/// response is mapped (see `Kount::build_error_response`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KountErrorResponse {
    pub code: Option<String>,
    pub message: Option<String>,
}

impl KountErrorResponse {
    /// Error code, falling back to the default when Kount omits it.
    pub fn code(&self) -> String {
        self.code
            .clone()
            .unwrap_or_else(|| KOUNT_DEFAULT_ERROR_CODE.to_string())
    }

    /// Error message, falling back to the default when Kount omits it.
    pub fn message(&self) -> String {
        self.message
            .clone()
            .unwrap_or_else(|| KOUNT_DEFAULT_ERROR_MESSAGE.to_string())
    }
}

// ──────────────────────────────────────────────────────────────────────────
// ServerAuthenticationToken (OAuth client-credentials)
// ──────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct KountTokenRequest {
    pub grant_type: String,
    pub scope: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        KountRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for KountTokenRequest
{
    type Error = Error;

    fn try_from(
        item: KountRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            grant_type: item.router_data.request.grant_type.clone(),
            scope: KOUNT_API_SCOPE.to_string(),
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KountTokenResponse {
    pub access_token: Secret<String>,
    pub token_type: String,
    pub expires_in: i64,
}

impl<F> TryFrom<ResponseRouterData<KountTokenResponse, Self>>
    for RouterDataV2<
        F,
        MerchantAuthenticationFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    >
{
    type Error = ResponseError;

    fn try_from(item: ResponseRouterData<KountTokenResponse, Self>) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(ServerAuthenticationTokenResponseData {
                access_token: item.response.access_token,
                token_type: Some(item.response.token_type),
                expires_in: Some(item.response.expires_in),
            }),
            ..item.router_data
        })
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Kount Orders API — shared response shape + decision mapping
// ──────────────────────────────────────────────────────────────────────────

/// Kount Orders API decision values.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum KountDecision {
    Approve,
    Review,
    Decline,
    #[serde(other)]
    Unknown,
}

impl From<&KountDecision> for FrmDecision {
    fn from(value: &KountDecision) -> Self {
        match value {
            KountDecision::Approve => Self::Approve,
            KountDecision::Review => Self::Review,
            KountDecision::Decline => Self::Reject,
            // Kount documents only APPROVE/DECLINE/REVIEW; treat anything else as REVIEW.
            KountDecision::Unknown => Self::Review,
        }
    }
}

/// Response from Evaluate Order (`POST /commerce/v2/orders`). Kount nests the
/// order under an `order` object, with the risk decision under `riskInquiry`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KountOrderResponse {
    pub order: Option<KountOrder>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KountOrder {
    /// Kount-assigned order id.
    #[serde(rename = "orderId")]
    pub order_id: Option<String>,
    #[serde(rename = "riskInquiry")]
    pub risk_inquiry: Option<KountRiskInquiry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KountRiskInquiry {
    pub decision: Option<KountDecision>,
    #[serde(alias = "omniscore", alias = "score")]
    pub omniscore: Option<f64>,
    pub reason: Option<String>,
}

/// Response from the Kount Orders API update (`PATCH /commerce/v2/orders/{id}`).
/// Distinct type from [`KountOrderResponse`] so the connector macros generate a
/// unique templating type per flow.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KountUpdateOrderResponse {
    #[serde(alias = "orderId")]
    pub order_id: Option<String>,
    pub decision: Option<KountDecision>,
    #[serde(alias = "omniscore", alias = "riskScore")]
    pub score: Option<f64>,
    pub reason: Option<String>,
}

/// Truncate a merchant-supplied id into a valid Kount `sessionId` (≤32 chars,
/// alphanumeric / `-` / `_`). Shared by the DDC HTML and the Evaluate Order
/// `deviceSessionId` so the two correlate.
pub fn to_session_id(raw: &str) -> String {
    raw.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(32)
        .collect()
}

/// Round a Kount omniscore (a 0–99 float) to the integer FRM risk score.
/// `f64 -> i32` has no safe `TryFrom`, and the value is bounded, so the cast is
/// scoped here behind an explicit allow.
#[allow(clippy::as_conversions)]
fn omniscore_to_risk_score(omniscore: f64) -> i32 {
    omniscore.round() as i32
}

// ──────────────────────────────────────────────────────────────────────────
// PreRiskCheck = Evaluate Order (POST /commerce/v2/orders)
// ──────────────────────────────────────────────────────────────────────────

/// Sales channel reported on the Evaluate Order.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum KountChannel {
    Web,
}

/// Kount account type, derived from whether the customer is registered.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum KountAccountType {
    Registered,
    Guest,
}

/// Kount fulfillment method for an order line.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum KountFulfillmentType {
    /// Physically shipped to the recipient.
    Shipped,
    /// Digitally delivered (no shipment).
    Digital,
}

/// Kount payment instrument type.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum KountPaymentType {
    CreditCard,
}

/// Evaluate Order request (`POST /commerce/v2/orders`). Modelled on the Kount
/// Orders schema and populated with as much merchant context as the FRM request
/// carries — account, line items, fulfillment, and a transaction block with the
/// payment instrument and billed person — so Kount has enough signal to return a
/// meaningful `riskInquiry` decision rather than a bare order.
#[derive(Debug, Clone, Serialize)]
pub struct KountEvaluateOrderRequest {
    /// Merchant's own order reference (Kount returns its own `order.orderId`).
    #[serde(rename = "merchantOrderId")]
    pub order_id: String,
    /// Links the DDC-collected device data — must equal the DDC SDK sessionID.
    #[serde(rename = "deviceSessionId")]
    pub session_id: String,
    /// Sales channel; web checkout by default.
    pub channel: KountChannel,
    /// Order creation timestamp (RFC 3339).
    #[serde(rename = "creationDateTime")]
    pub creation_date_time: String,
    /// End-user IP address (from browser info), when available.
    #[serde(rename = "userIp", skip_serializing_if = "Option::is_none")]
    pub user_ip: Option<String>,
    /// Customer/account context, when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<KountAccount>,
    /// Purchased line items.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<KountItem>,
    /// Fulfillment / shipping context, when a shipping address is present.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fulfillment: Vec<KountFulfillment>,
    /// Payment transaction(s) with the billed person and amount.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub transactions: Vec<KountTransaction>,
    /// Device/browser details derived from the browser info, when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<KountDevice>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KountDevice {
    #[serde(rename = "ipAddress", skip_serializing_if = "Option::is_none")]
    pub ip_address: Option<String>,
    #[serde(rename = "userAgent", skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KountAccount {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub account_type: KountAccountType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// Whether the account is active. Omitted when unknown (the FRM request
    /// carries no account-status signal).
    #[serde(rename = "accountIsActive", skip_serializing_if = "Option::is_none")]
    pub account_is_active: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KountItem {
    /// Per-unit price in the smallest currency unit (string per Kount schema).
    pub price: StringMinorUnit,
    pub name: String,
    pub quantity: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(rename = "isDigital", skip_serializing_if = "Option::is_none")]
    pub is_digital: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sku: Option<String>,
    /// Subscription / recurring billing context for this line item, when the
    /// order carries mandate info.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurring: Option<KountRecurring>,
}

/// Kount Orders `recurring` (RecurringDetails) block carried on a line item.
/// Populated from the FRM request's `mandate_info`. Amounts are serialized as
/// strings to match Kount's `string<uint64>` schema.
#[derive(Debug, Clone, Serialize)]
pub struct KountRecurring {
    #[serde(rename = "startDate", skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    #[serde(rename = "endDate", skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
    #[serde(
        rename = "initialBillingAmount",
        skip_serializing_if = "Option::is_none"
    )]
    pub initial_billing_amount: Option<String>,
    #[serde(
        rename = "periodBillingAmount",
        skip_serializing_if = "Option::is_none"
    )]
    pub period_billing_amount: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period: Option<String>,
    #[serde(
        rename = "externalSubscriptionId",
        skip_serializing_if = "Option::is_none"
    )]
    pub external_subscription_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(rename = "nextBillingDate", skip_serializing_if = "Option::is_none")]
    pub next_billing_date: Option<String>,
    #[serde(rename = "billingCycle", skip_serializing_if = "Option::is_none")]
    pub billing_cycle: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl From<&MandateInfo> for KountRecurring {
    fn from(m: &MandateInfo) -> Self {
        Self {
            start_date: m.start_date.clone(),
            end_date: m.end_date.clone(),
            initial_billing_amount: m.initial_billing_amount.map(|amount| amount.to_string()),
            period_billing_amount: m.period_billing_amount.map(|amount| amount.to_string()),
            period: m.period.clone(),
            external_subscription_id: m.external_subscription_id.clone(),
            status: m.status.clone(),
            next_billing_date: m.next_billing_date.clone(),
            billing_cycle: m.billing_cycle,
            description: m.description.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct KountFulfillment {
    #[serde(rename = "type")]
    pub fulfillment_type: KountFulfillmentType,
    #[serde(rename = "recipientPerson", skip_serializing_if = "Option::is_none")]
    pub recipient_person: Option<KountPerson>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KountTransaction {
    /// Subtotal in the smallest currency unit (string per Kount schema).
    pub subtotal: StringMinorUnit,
    /// Order total in the smallest currency unit (string per Kount schema).
    #[serde(rename = "orderTotal")]
    pub order_total: StringMinorUnit,
    pub currency: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment: Option<KountPayment>,
    #[serde(rename = "billedPerson", skip_serializing_if = "Option::is_none")]
    pub billed_person: Option<KountPerson>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KountPayment {
    #[serde(rename = "type")]
    pub payment_type: KountPaymentType,
    /// Card BIN (first 6 digits) — no full PAN is sent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last4: Option<String>,
    /// Stable, salted hash of the payment instrument (never the raw PAN). Lets
    /// Kount link/score the instrument across orders. See [`payment_token_hash`].
    #[serde(rename = "paymentToken", skip_serializing_if = "Option::is_none")]
    pub payment_token: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KountPerson {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<KountName>,
    #[serde(rename = "emailAddress", skip_serializing_if = "Option::is_none")]
    pub email_address: Option<String>,
    #[serde(rename = "phoneNumber", skip_serializing_if = "Option::is_none")]
    pub phone_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<KountAddress>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KountName {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last: Option<String>,
}

impl KountName {
    /// Build a name only when at least one part is present, so we never emit an
    /// empty `{}` name object.
    fn from_parts(first: Option<String>, last: Option<String>) -> Option<Self> {
        (first.is_some() || last.is_some()).then_some(Self { first, last })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct KountAddress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line1: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line2: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(rename = "countryCode", skip_serializing_if = "Option::is_none")]
    pub country_code: Option<String>,
    #[serde(rename = "postalCode", skip_serializing_if = "Option::is_none")]
    pub postal_code: Option<String>,
}

/// Marker error for person conversions that yield no usable fields. Callers use
/// `.ok()` to treat "no person data" as an absent (skipped) block.
pub struct NoPersonData;

/// Build a Kount person block (name / email / phone / address) from a domain
/// address. `Err(NoPersonData)` when the address carries no usable person fields.
impl TryFrom<&Address> for KountPerson {
    type Error = NoPersonData;

    fn try_from(addr: &Address) -> Result<Self, Self::Error> {
        let details = addr.address.as_ref();
        let name = details.and_then(|d| {
            let first = d.first_name.as_ref().map(|name| name.peek().to_string());
            let last = d.last_name.as_ref().map(|name| name.peek().to_string());
            KountName::from_parts(first, last)
        });
        let kount_address = details.map(|d| KountAddress {
            line1: d.line1.as_ref().map(|line| line.peek().to_string()),
            line2: d.line2.as_ref().map(|line| line.peek().to_string()),
            city: d.city.as_ref().map(|city| city.peek().to_string()),
            region: d.state.as_ref().map(|state| state.peek().to_string()),
            country_code: d.country.map(|country| country.to_string()),
            postal_code: d.zip.as_ref().map(|zip| zip.peek().to_string()),
        });
        let email_address = addr.email.as_ref().map(|email| email.peek().to_string());
        let phone_number = addr
            .phone
            .as_ref()
            .and_then(|phone| phone.number.as_ref().map(|num| num.peek().to_string()));
        if name.is_none()
            && kount_address.is_none()
            && email_address.is_none()
            && phone_number.is_none()
        {
            return Err(NoPersonData);
        }
        Ok(Self {
            name,
            email_address,
            phone_number,
            address: kount_address,
        })
    }
}

/// Fallback person block from customer info (name / email / phone, no address).
/// `Err(NoPersonData)` when the customer carries no usable fields.
impl TryFrom<&CustomerInfo> for KountPerson {
    type Error = NoPersonData;

    fn try_from(customer: &CustomerInfo) -> Result<Self, Self::Error> {
        let first = customer
            .first_name
            .as_ref()
            .map(|name| name.peek().to_string());
        let last = customer
            .last_name
            .as_ref()
            .map(|name| name.peek().to_string());
        let email_address = customer
            .customer_email
            .as_ref()
            .map(|email| email.peek().to_string());
        let phone_number = customer
            .customer_phone_number
            .as_ref()
            .map(|phone| phone.peek().to_string());
        if first.is_none() && last.is_none() && email_address.is_none() && phone_number.is_none() {
            return Err(NoPersonData);
        }
        Ok(Self {
            name: KountName::from_parts(first, last),
            email_address,
            phone_number,
            address: None,
        })
    }
}

/// Stable payment-instrument token for Kount: `hex(HMAC-SHA256(key = api_key,
/// msg = PAN))`. The Kount `api_key` secret is reused as the salt so the token
/// is consistent for a given card under a merchant's credentials while never
/// emitting the raw PAN. Returns `None` if signing fails.
fn payment_token_hash(api_key: &Secret<String>, pan: &str) -> Option<String> {
    use common_utils::crypto::{HmacSha256, SignMessage};
    HmacSha256
        .sign_message(api_key.peek().as_bytes(), pan.as_bytes())
        .ok()
        .map(hex::encode)
}

/// Card BIN (first 6) + last4 from a raw PAN, ignoring formatting. Never emits
/// the full PAN.
fn card_bin_last4(pan: &str) -> (Option<String>, Option<String>) {
    let digits: String = pan.chars().filter(|c| c.is_ascii_digit()).collect();
    let bin = (digits.len() >= 6).then(|| digits[..6].to_string());
    let last4 = (digits.len() >= 4).then(|| digits[digits.len() - 4..].to_string());
    (bin, last4)
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        KountRouterData<
            RouterDataV2<PreRiskCheck, FrmFlowData, PreRiskCheckRequest, PreRiskCheckResponse>,
            T,
        >,
    > for KountEvaluateOrderRequest
{
    type Error = Error;

    fn try_from(
        item: KountRouterData<
            RouterDataV2<PreRiskCheck, FrmFlowData, PreRiskCheckRequest, PreRiskCheckResponse>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let req = &item.router_data.request;
        let order_id = req.merchant_transaction_id.clone().ok_or_else(|| {
            error_stack::report!(errors::IntegrationError::MissingRequiredField {
                field_name: "merchant_transaction_id",
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Kount Evaluate Order needs merchant_transaction_id; it is the merchant \
                         order reference and the basis for the deviceSessionId"
                            .to_owned(),
                    ),
                    suggested_action: Some(
                        "Set merchant_transaction_id on the FRM Pre Risk Check request".to_owned(),
                    ),
                    doc_url: Some(KOUNT_DOC_URL.to_owned()),
                },
            })
        })?;

        // Device / IP from browser info.
        let browser = req.browser_info.as_ref();
        let user_ip = browser.and_then(|info| info.ip_address.map(|ip| ip.to_string()));
        let device = browser.and_then(|info| {
            let ip_address = info.ip_address.map(|ip| ip.to_string());
            let user_agent = info.user_agent.clone();
            (ip_address.is_some() || user_agent.is_some()).then_some(KountDevice {
                ip_address,
                user_agent,
            })
        });

        // Account context from customer info.
        let account = req.customer_info.as_ref().map(|customer| KountAccount {
            account_type: if customer.customer_id.is_some() {
                KountAccountType::Registered
            } else {
                KountAccountType::Guest
            },
            id: customer
                .customer_id
                .as_ref()
                .map(|id| id.get_string_repr().to_string()),
            username: customer
                .customer_email
                .as_ref()
                .map(|email| email.peek().to_string())
                .or_else(|| customer.get_full_name().map(|name| name.peek().to_string())),
            // No account-status signal in the FRM request, so leave it unset.
            account_is_active: None,
        });

        let currency = req.amount.currency;

        // Subscription / recurring context, applied to every line item when the
        // order carries mandate info.
        let recurring = req.mandate_info.as_ref().map(KountRecurring::from);

        // Line items from order details.
        let items = match req.order_details.as_ref() {
            Some(details) => details
                .iter()
                .map(|detail| {
                    Ok::<_, Self::Error>(KountItem {
                        price: super::KountAmountConvertor::convert(detail.amount, currency)?,
                        name: detail.product_name.clone(),
                        quantity: detail.quantity,
                        description: detail.description.clone(),
                        category: detail.category.clone(),
                        is_digital: detail.requires_shipping.map(|ships| !ships),
                        sku: detail.sku.clone(),
                        recurring: recurring.clone(),
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
            None => Vec::new(),
        };

        // Fulfillment method (the order's intended fulfillment at evaluation time,
        // not a post-ship status): digital only when every line item is flagged as
        // not requiring shipping, otherwise physical shipment.
        let fulfillment_type = req
            .order_details
            .as_ref()
            .filter(|details| {
                !details.is_empty()
                    && details
                        .iter()
                        .all(|detail| detail.requires_shipping == Some(false))
            })
            .map_or(KountFulfillmentType::Shipped, |_| {
                KountFulfillmentType::Digital
            });

        // Billing / shipping persons from the payment address.
        let address = req.address.as_ref();
        let billed_person = address
            .and_then(|addr| {
                addr.get_payment_billing()
                    .or_else(|| addr.get_payment_method_billing())
            })
            .and_then(|addr| KountPerson::try_from(addr).ok())
            .or_else(|| {
                req.customer_info
                    .as_ref()
                    .and_then(|customer| KountPerson::try_from(customer).ok())
            });
        let fulfillment = address
            .and_then(|addr| addr.get_shipping())
            .and_then(|addr| KountPerson::try_from(addr).ok())
            .map(|recipient| {
                vec![KountFulfillment {
                    fulfillment_type,
                    recipient_person: Some(recipient),
                }]
            })
            .unwrap_or_default();

        // Payment instrument (BIN/last4 + salted token only) from the payment
        // method. The api_key is reused as the token salt; if auth can't be
        // resolved we still send BIN/last4 and simply omit the token.
        let api_key = KountAuthType::try_from(&item.router_data.connector_config)
            .ok()
            .map(|auth| auth.api_key);
        let payment = match req.payment_method.as_ref() {
            Some(PaymentMethodData::Card(card)) => {
                let pan = card.card_number.peek();
                let (bin, last4) = card_bin_last4(pan);
                let payment_token = api_key
                    .as_ref()
                    .and_then(|api_key| payment_token_hash(api_key, pan));
                Some(KountPayment {
                    payment_type: KountPaymentType::CreditCard,
                    bin,
                    last4,
                    payment_token,
                })
            }
            _ => None,
        };

        let amount = super::KountAmountConvertor::convert(req.amount.amount, currency)?;
        let transactions = vec![KountTransaction {
            subtotal: amount.clone(),
            order_total: amount,
            currency: currency.to_string(),
            payment,
            billed_person,
        }];

        let creation_date_time = OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default();

        Ok(Self {
            order_id: order_id.clone(),
            session_id: to_session_id(&order_id),
            channel: KountChannel::Web,
            creation_date_time,
            user_ip,
            account,
            items,
            fulfillment,
            transactions,
            device,
        })
    }
}

impl TryFrom<ResponseRouterData<KountOrderResponse, Self>>
    for RouterDataV2<PreRiskCheck, FrmFlowData, PreRiskCheckRequest, PreRiskCheckResponse>
{
    type Error = ResponseError;

    fn try_from(item: ResponseRouterData<KountOrderResponse, Self>) -> Result<Self, Self::Error> {
        // Always surface the raw Kount response (independent of the global
        // `return_raw_connector_data` flag), mirroring twoc_twop_paco / ppro.
        let raw_connector_response = serde_json::to_string(&item.response).ok().map(Secret::new);
        let order = item.response.order.as_ref();
        let risk = order.and_then(|o| o.risk_inquiry.as_ref());
        Ok(Self {
            response: Ok(PreRiskCheckResponse {
                frm_decision: risk
                    .and_then(|inquiry| inquiry.decision.as_ref())
                    .map(FrmDecision::from),
                risk_score: risk
                    .and_then(|inquiry| inquiry.omniscore)
                    .map(omniscore_to_risk_score),
                reason: risk.and_then(|inquiry| inquiry.reason.clone()),
                frm_transaction_id: order.and_then(|order| order.order_id.clone()),
                status_code: item.http_code,
            }),
            resource_common_data: FrmFlowData {
                raw_connector_response,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

/// Kount Update Order disposition tokens. Serialized in uppercase to match the
/// Kount Orders schema.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum KountDisposition {
    Approve,
    Decline,
    Review,
}

impl KountDisposition {
    /// Map the FRM decision to a Kount disposition. `Error` has no Kount
    /// disposition, so it is omitted rather than sent as a guessed value.
    fn from_decision(decision: FrmDecision) -> Option<Self> {
        match decision {
            FrmDecision::Approve => Some(Self::Approve),
            FrmDecision::Reject => Some(Self::Decline),
            FrmDecision::Review => Some(Self::Review),
            FrmDecision::Error => None,
        }
    }
}

/// Kount Update Order payment-status tokens. Serialized in uppercase to match
/// the Kount Orders schema.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum KountPaymentStatus {
    Charged,
    Authorized,
    Voided,
    Refunded,
    Declined,
}

impl KountPaymentStatus {
    /// Map the internal attempt status to a Kount payment status. Unmapped
    /// statuses are omitted rather than sent as an unrecognized value.
    fn from_attempt_status(status: AttemptStatus) -> Option<Self> {
        match status {
            AttemptStatus::Charged
            | AttemptStatus::PartialCharged
            | AttemptStatus::PartialChargedAndChargeable => Some(Self::Charged),
            AttemptStatus::Authorized | AttemptStatus::PartiallyAuthorized => {
                Some(Self::Authorized)
            }
            AttemptStatus::Voided | AttemptStatus::VoidedPostCapture => Some(Self::Voided),
            AttemptStatus::AutoRefunded => Some(Self::Refunded),
            AttemptStatus::Failure
            | AttemptStatus::AuthorizationFailed
            | AttemptStatus::CaptureFailed
            | AttemptStatus::RouterDeclined => Some(Self::Declined),
            _ => None,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────
// FrmPaymentOutcome (Notify: payment succeeded) = Update Order
// PATCH /commerce/v2/orders/{orderId}
// ──────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct KountUpdateOrderRequest {
    /// Connector (gateway) transaction id, attached after authorization.
    #[serde(
        rename = "merchantTransactionId",
        skip_serializing_if = "Option::is_none"
    )]
    pub merchant_transaction_id: Option<String>,
    /// Final payment status.
    #[serde(rename = "paymentStatus", skip_serializing_if = "Option::is_none")]
    pub payment_status: Option<KountPaymentStatus>,
    /// Order total in the smallest currency unit (string per Kount schema).
    #[serde(rename = "orderTotal")]
    pub order_total: StringMinorUnit,
    /// ISO 4217 currency code.
    pub currency: String,
    /// FRM decision being notified (APPROVE / DECLINE / REVIEW).
    #[serde(rename = "frmDisposition", skip_serializing_if = "Option::is_none")]
    pub frm_disposition: Option<KountDisposition>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        KountRouterData<
            RouterDataV2<
                FrmPaymentOutcome,
                FrmFlowData,
                FrmPaymentOutcomeRequest,
                FrmPaymentOutcomeResponse,
            >,
            T,
        >,
    > for KountUpdateOrderRequest
{
    type Error = Error;

    fn try_from(
        item: KountRouterData<
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
        Ok(Self {
            merchant_transaction_id: req
                .merchant_transaction_id
                .clone()
                .or_else(|| req.connector_transaction_id.clone()),
            payment_status: req
                .payment_status
                .and_then(KountPaymentStatus::from_attempt_status),
            order_total: super::KountAmountConvertor::convert(
                req.amount.amount,
                req.amount.currency,
            )?,
            currency: req.amount.currency.to_string(),
            frm_disposition: req.frm_decision.and_then(KountDisposition::from_decision),
        })
    }
}

impl TryFrom<ResponseRouterData<KountUpdateOrderResponse, Self>>
    for RouterDataV2<
        FrmPaymentOutcome,
        FrmFlowData,
        FrmPaymentOutcomeRequest,
        FrmPaymentOutcomeResponse,
    >
{
    type Error = ResponseError;

    fn try_from(
        item: ResponseRouterData<KountUpdateOrderResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(FrmPaymentOutcomeResponse {
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// ──────────────────────────────────────────────────────────────────────────
// FrmRefundProcessed (Notify: refund) = Update Order
// PATCH /commerce/v2/orders/{orderId}
// ──────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct KountRefundUpdateRequest {
    /// Connector (gateway) transaction id the refund belongs to.
    #[serde(
        rename = "merchantTransactionId",
        skip_serializing_if = "Option::is_none"
    )]
    pub merchant_transaction_id: Option<String>,
    /// Connector refund id.
    #[serde(rename = "refundId", skip_serializing_if = "Option::is_none")]
    pub refund_id: Option<String>,
    /// Reason supplied for the refund.
    #[serde(rename = "refundReason", skip_serializing_if = "Option::is_none")]
    pub refund_reason: Option<String>,
    /// Refund amount in the smallest currency unit (string per Kount schema).
    #[serde(rename = "refundAmount")]
    pub refund_amount: StringMinorUnit,
    /// ISO 4217 currency code.
    pub currency: String,
    /// FRM decision being notified (APPROVE / DECLINE / REVIEW).
    #[serde(rename = "frmDisposition", skip_serializing_if = "Option::is_none")]
    pub frm_disposition: Option<KountDisposition>,
}

/// Response from the refund Update Order PATCH. Distinct type from
/// [`KountUpdateOrderResponse`] so the connector macros generate a unique
/// templating type per flow.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KountRefundUpdateResponse {
    #[serde(alias = "orderId")]
    pub order_id: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        KountRouterData<
            RouterDataV2<
                FrmRefundProcessed,
                FrmFlowData,
                FrmRefundProcessedRequest,
                FrmRefundProcessedResponse,
            >,
            T,
        >,
    > for KountRefundUpdateRequest
{
    type Error = Error;

    fn try_from(
        item: KountRouterData<
            RouterDataV2<
                FrmRefundProcessed,
                FrmFlowData,
                FrmRefundProcessedRequest,
                FrmRefundProcessedResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let req = &item.router_data.request;
        Ok(Self {
            merchant_transaction_id: req.connector_transaction_id.clone(),
            refund_id: req
                .connector_refund_id
                .clone()
                .or_else(|| req.merchant_refund_id.clone()),
            refund_reason: req.refund_reason.clone(),
            refund_amount: super::KountAmountConvertor::convert(
                req.amount.amount,
                req.amount.currency,
            )?,
            currency: req.amount.currency.to_string(),
            frm_disposition: req.frm_decision.and_then(KountDisposition::from_decision),
        })
    }
}

impl TryFrom<ResponseRouterData<KountRefundUpdateResponse, Self>>
    for RouterDataV2<
        FrmRefundProcessed,
        FrmFlowData,
        FrmRefundProcessedRequest,
        FrmRefundProcessedResponse,
    >
{
    type Error = ResponseError;

    fn try_from(
        item: ResponseRouterData<KountRefundUpdateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(FrmRefundProcessedResponse {
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}
