use crate::{
    connectors::{hipay::HipayRouterData, macros::GetFormData},
    types::ResponseRouterData,
    utils::{build_form_from_struct, is_manual_capture},
};
use common_enums::{AttemptStatus, RefundStatus};
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    ext_traits::ValueExt,
    pii::Email,
    request::MultipartData,
    types::{MinorUnit, StringMajorUnit},
};
use domain_types::errors::{ConnectorError, IntegrationError, IntegrationErrorContext};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, PaymentMethodToken, RSync, Refund, Void},
    connector_types::{
        MandateReference, PaymentFlowData, PaymentMethodTokenResponse,
        PaymentMethodTokenizationData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId,
    },
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes},
    router_data::{
        AdditionalPaymentMethodConnectorResponse, ConnectorResponseData, ConnectorSpecificConfig,
    },
    router_data_v2::RouterDataV2,
    router_request_types::BrowserInformation,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct HipayAuthType {
    pub api_key: Secret<String>,
    pub api_secret: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for HipayAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Hipay {
                api_key,
                api_secret,
                ..
            } => Ok(Self {
                api_key: api_key.to_owned(),
                api_secret: api_secret.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
            )),
        }
    }
}

/// HiPay's HTTP-level error envelope: `{"code": …, "message": …, "description": …}`.
///
/// `code` is declared as an **integer** in the gateway OpenAPI document but several routes
/// emit the 7-digit HiPay code as a JSON string, so it is read through a tolerant
/// deserializer rather than being pinned to one JSON type. `description` carries the
/// actionable detail and is surfaced as the `ErrorResponse.reason`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HipayErrorResponse {
    #[serde(deserialize_with = "deserialize_code_as_string")]
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// HiPay writes its numeric codes as either `4010103` or `"4010103"` depending on the
/// route; both are accepted and normalised to the string form the `ErrorResponse.code`
/// field expects.
fn deserialize_code_as_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrNumber {
        String(String),
        Number(i64),
    }

    Ok(match StringOrNumber::deserialize(deserializer)? {
        StringOrNumber::String(value) => value,
        StringOrNumber::Number(value) => value.to_string(),
    })
}

/// HiPay renders an absent object or an absent code as an **empty string** rather than
/// omitting the key or sending `null` — a successful order comes back with `"reason": ""`,
/// `"avsResult": ""` and `"threeDSecure": ""`. Deserializing those directly into the typed
/// field fails the whole response, so an empty string is read as "absent".
fn deserialize_optional_ignoring_empty<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    match Option::<serde_json::Value>::deserialize(deserializer)? {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(value)) if value.trim().is_empty() => Ok(None),
        Some(value) => T::deserialize(value)
            .map(Some)
            .map_err(serde::de::Error::custom),
    }
}

/// Transaction-level decline detail. A declined transaction arrives as HTTP 200 with a
/// failure `status` and this nested object rather than the HTTP error envelope above.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HipayReason {
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

/// HiPay error codes are 7 digits and the leading two digits identify the origin:
/// `10xxxxx` account/validation/routing and `30xxxxx` order-lifecycle codes are raised by
/// HiPay itself, while only the `40xxxxx` range (`400000x` acquirer, `401xxxx` issuer) is
/// sourced from the card networks. Only that range may populate `network_decline_code` —
/// reporting a HiPay-side validation error as a network decline corrupts GSM retry rules.
pub(crate) fn is_network_sourced_code(code: &str) -> bool {
    code.len() == 7 && code.starts_with("40") && code.chars().all(|c| c.is_ascii_digit())
}

/// Build the `ErrorResponse` for a transaction that HiPay accepted (HTTP 2xx) but
/// declined. `reason.code` / `reason.message` are the real connector codes; the
/// transaction `message` is the fallback when no `reason` block is present.
///
/// `network_advice_code` is always `None`: HiPay publishes no advice/retry code — it has
/// no equivalent of the Visa VAU / Mastercard MIT advice codes.
fn build_transaction_error_response(
    reason: Option<&HipayReason>,
    message: &str,
    status_code: u16,
    connector_transaction_id: Option<String>,
    attempt_status: Option<domain_types::router_data::FlowStatus>,
) -> domain_types::router_data::ErrorResponse {
    let code = reason
        .and_then(|reason| reason.code.clone())
        .filter(|code| !code.is_empty())
        .unwrap_or_else(|| NO_ERROR_CODE.to_string());
    let error_message = reason
        .and_then(|reason| reason.message.clone())
        .filter(|reason_message| !reason_message.is_empty())
        .unwrap_or_else(|| {
            if message.is_empty() {
                NO_ERROR_MESSAGE.to_string()
            } else {
                message.to_string()
            }
        });
    let is_network_decline = is_network_sourced_code(&code);

    domain_types::router_data::ErrorResponse {
        code: code.clone(),
        message: error_message.clone(),
        reason: Some(error_message.clone()),
        status_code,
        attempt_status,
        connector_transaction_id,
        network_decline_code: is_network_decline.then_some(code),
        network_advice_code: None,
        network_error_message: is_network_decline.then_some(error_message),
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
}

/// HiPay transaction status codes.
///
/// The full published catalogue is modelled so that a documented code never fails
/// deserialization; `Unknown` catches codes HiPay adds later (HiPay reserves 127, 128,
/// 130, 133, 135-139, 145-149, 152-159, 162, 164, 167, 170-171, 176, 179 and 184+).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HipayPaymentStatus {
    #[serde(rename = "101")]
    Created,
    #[serde(rename = "103")]
    CardholderEnrolled,
    #[serde(rename = "104")]
    CardholderNotEnrolled,
    #[serde(rename = "105")]
    UnableToAuthenticate,
    #[serde(rename = "106")]
    CardholderAuthenticated,
    #[serde(rename = "107")]
    AuthenticationAttempted,
    #[serde(rename = "108")]
    CouldNotAuthenticate,
    #[serde(rename = "109")]
    AuthenticationFailed,
    #[serde(rename = "110")]
    Blocked,
    #[serde(rename = "111")]
    Denied,
    #[serde(rename = "112")]
    AuthorizedAndPending,
    #[serde(rename = "113")]
    Refused,
    #[serde(rename = "114")]
    Expired,
    #[serde(rename = "115")]
    Cancelled,
    #[serde(rename = "116")]
    Authorized,
    #[serde(rename = "117")]
    CaptureRequested,
    #[serde(rename = "118")]
    Captured,
    #[serde(rename = "119")]
    PartiallyCaptured,
    #[serde(rename = "120")]
    Collected,
    #[serde(rename = "121")]
    PartiallyCollected,
    #[serde(rename = "122")]
    Settled,
    #[serde(rename = "123")]
    PartiallySettled,
    /// Deprecated by HiPay in favour of 180 / 181, still emitted by older accounts.
    #[serde(rename = "129")]
    ChargedBack,
    #[serde(rename = "131")]
    Debited,
    #[serde(rename = "132")]
    PartiallyDebited,
    #[serde(rename = "134")]
    DisputeLost,
    #[serde(rename = "140")]
    AuthenticationRequested,
    #[serde(rename = "141")]
    Authenticated,
    #[serde(rename = "142")]
    AuthorizationRequested,
    #[serde(rename = "143")]
    AuthorizationCancelled,
    #[serde(rename = "144")]
    ReferenceRendered,
    #[serde(rename = "150")]
    AcquirerFound,
    #[serde(rename = "151")]
    AcquirerNotFound,
    #[serde(rename = "160")]
    CardholderEnrollmentUnknown,
    #[serde(rename = "161")]
    RiskAccepted,
    #[serde(rename = "163")]
    AuthorizationRefused,
    /// HiPay publishes 166 and 168 under the same label, "Debited (cardholder credit)",
    /// without distinguishing them; both are modelled and mapped identically.
    #[serde(rename = "166")]
    CardholderCreditDebited,
    #[serde(rename = "168")]
    CardholderCreditDebitedAlternate,
    #[serde(rename = "169")]
    CreditRequested,
    #[serde(rename = "172")]
    InProgress,
    #[serde(rename = "173")]
    CaptureRefused,
    #[serde(rename = "174")]
    AwaitingTerminal,
    #[serde(rename = "175")]
    AuthorizationCancellationRequested,
    #[serde(rename = "177")]
    ChallengeRequested,
    #[serde(rename = "178")]
    SoftDeclined,
    #[serde(rename = "180")]
    PartiallyChargedBack,
    #[serde(rename = "181")]
    Chargeback,
    #[serde(rename = "200")]
    PendingPayment,
    /// Any status code HiPay adds that this integration does not yet model. Kept so a new
    /// upstream code degrades to "not determined" instead of failing deserialization.
    #[serde(other)]
    Unknown,
}

impl From<HipayPaymentStatus> for AttemptStatus {
    fn from(status: HipayPaymentStatus) -> Self {
        match status {
            HipayPaymentStatus::AuthenticationFailed => Self::AuthenticationFailed,
            HipayPaymentStatus::Blocked
            | HipayPaymentStatus::Refused
            | HipayPaymentStatus::Expired
            | HipayPaymentStatus::Denied => Self::Failure,
            HipayPaymentStatus::AuthorizedAndPending => Self::Pending,
            HipayPaymentStatus::Cancelled => Self::Voided,
            HipayPaymentStatus::Authorized => Self::Authorized,
            HipayPaymentStatus::CaptureRequested => Self::CaptureInitiated,
            HipayPaymentStatus::Captured => Self::Charged,
            HipayPaymentStatus::PartiallyCaptured => Self::PartialCharged,
            HipayPaymentStatus::CaptureRefused => Self::CaptureFailed,
            HipayPaymentStatus::AwaitingTerminal => Self::Pending,
            HipayPaymentStatus::AuthorizationCancellationRequested => Self::VoidInitiated,
            HipayPaymentStatus::ChallengeRequested => Self::AuthenticationPending,
            HipayPaymentStatus::SoftDeclined => Self::Failure,
            HipayPaymentStatus::PendingPayment => Self::Pending,
            // Chargebacks and a lost dispute are dispute outcomes rather than payment
            // outcomes. UCS has no dispute-bearing AttemptStatus on the payment path, so
            // they collapse to Failure here; the dispute signal is carried by the webhook
            // flow, which is not part of this integration yet.
            HipayPaymentStatus::ChargedBack
            | HipayPaymentStatus::PartiallyChargedBack
            | HipayPaymentStatus::Chargeback
            | HipayPaymentStatus::DisputeLost => Self::Failure,
            HipayPaymentStatus::Created => Self::Started,
            HipayPaymentStatus::UnableToAuthenticate | HipayPaymentStatus::CouldNotAuthenticate => {
                Self::AuthenticationFailed
            }
            HipayPaymentStatus::CardholderAuthenticated => Self::Pending,
            HipayPaymentStatus::AuthenticationAttempted => Self::AuthenticationPending,
            HipayPaymentStatus::CardholderEnrolled
            | HipayPaymentStatus::CardholderNotEnrolled
            | HipayPaymentStatus::CardholderEnrollmentUnknown => Self::AuthenticationPending,
            HipayPaymentStatus::Collected
            | HipayPaymentStatus::PartiallySettled
            | HipayPaymentStatus::PartiallyCollected
            | HipayPaymentStatus::Settled => Self::Charged,
            HipayPaymentStatus::CardholderCreditDebited
            | HipayPaymentStatus::CardholderCreditDebitedAlternate => Self::Charged,
            HipayPaymentStatus::Debited
            | HipayPaymentStatus::PartiallyDebited
            | HipayPaymentStatus::CreditRequested
            | HipayPaymentStatus::InProgress
            | HipayPaymentStatus::ReferenceRendered
            | HipayPaymentStatus::AcquirerFound
            | HipayPaymentStatus::AuthorizationRequested => Self::Pending,
            HipayPaymentStatus::AuthenticationRequested => Self::AuthenticationPending,
            HipayPaymentStatus::Authenticated => Self::AuthenticationSuccessful,
            HipayPaymentStatus::AcquirerNotFound => Self::Failure,
            HipayPaymentStatus::RiskAccepted => Self::Pending,
            HipayPaymentStatus::AuthorizationRefused => Self::Failure,
            // 143 terminates the 175 -> 115 cancellation sequence. HiPay buckets it under
            // its coarse "Failure" outcome column, but a successfully cancelled
            // authorization is a completed void: reporting it as Failure would show a
            // voided payment as a failed one.
            HipayPaymentStatus::AuthorizationCancelled => Self::Voided,
            // Never guess on an unmodelled code: Unspecified leaves the caller's stored
            // status intact instead of stamping a payment as failed or charged.
            HipayPaymentStatus::Unknown => Self::Unspecified,
        }
    }
}

/// HiPay refund status codes, including the Visa Rapid Dispute Resolution refunds
/// (182 / 183) that a dispute can settle a payment with.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum HipayRefundStatus {
    #[serde(rename = "124")]
    RefundRequested,
    #[serde(rename = "125")]
    Refunded,
    #[serde(rename = "126")]
    PartiallyRefunded,
    #[serde(rename = "165")]
    RefundRefused,
    #[serde(rename = "182")]
    PartiallyRefundedByRdr,
    #[serde(rename = "183")]
    RefundedByRdr,
    /// Any refund status code HiPay adds later.
    #[serde(other)]
    Unknown,
}

impl From<HipayRefundStatus> for RefundStatus {
    fn from(item: HipayRefundStatus) -> Self {
        match item {
            HipayRefundStatus::RefundRequested => Self::Pending,
            HipayRefundStatus::Refunded
            | HipayRefundStatus::PartiallyRefunded
            | HipayRefundStatus::PartiallyRefundedByRdr
            | HipayRefundStatus::RefundedByRdr => Self::Success,
            HipayRefundStatus::RefundRefused => Self::Failure,
            // Unknown, never Pending: a terminal refund decline mapped to Pending polls
            // for ever.
            HipayRefundStatus::Unknown => Self::Unknown,
        }
    }
}

// Sync Response Types
// Reason struct for PSync response - matches v3 API format
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Reason {
    pub reason: Option<String>,
    pub code: Option<u64>,
}

// HiPay v3 PSync Response - flat structure matching v3 transaction API
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HipaySyncResponse {
    Response {
        id: i64,
        status: i32,
        #[serde(default)]
        reason: Reason,
        #[serde(flatten)]
        extra: std::collections::HashMap<String, serde_json::Value>,
    },
    Error {
        message: String,
        code: u32,
    },
}

// HiPay v3 Refund Sync Response - JSON structure matching v3 transaction API
// Same endpoint as PSync but for refund transactions
#[derive(Debug, Serialize, Deserialize)]
pub struct HipayRefundSyncJsonResponse {
    pub id: i64,
    pub status: i32,
}

// Type alias for backward compatibility
pub type HipayRefundSyncResponse = HipayRefundSyncJsonResponse;

// HiPay Operation Enum - Type-safe operation codes for maintenance requests
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HipayOperation {
    Capture,
    Refund,
    Cancel,
}

impl std::fmt::Display for HipayOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Capture => write!(f, "capture"),
            Self::Refund => write!(f, "refund"),
            Self::Cancel => write!(f, "cancel"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Operation {
    Authorization,
    Sale,
}

/// HiPay `payment_product` codes for the card and card-adjacent products.
///
/// The wire values are a closed set published by HiPay; an unroutable or unknown value is
/// rejected with `1020003 Unsupported Payment Product`, so the field is an enum rather
/// than a free string. Non-card products (wallets, bank transfers, APMs) are deliberately
/// absent — this connector only serves the Card payment method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HipayPaymentProduct {
    Visa,
    Mastercard,
    Maestro,
    AmericanExpress,
    Cb,
    Bcmc,
    Bancontactqrcode,
    Diners,
    Jcb,
    Discover,
    Unionpay,
    Cup,
    Dankort,
    Edankort,
    Postepay,
    EloCard,
    Hipercard,
    Aura,
    CarteAccord,
    EcarteBleue,
    PostfinanceCard,
    CarteTitreRestaurant,
}

impl TryFrom<common_enums::CardNetwork> for HipayPaymentProduct {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(network: common_enums::CardNetwork) -> Result<Self, Self::Error> {
        match network {
            common_enums::CardNetwork::Visa => Ok(Self::Visa),
            common_enums::CardNetwork::Mastercard => Ok(Self::Mastercard),
            common_enums::CardNetwork::AmericanExpress => Ok(Self::AmericanExpress),
            common_enums::CardNetwork::JCB => Ok(Self::Jcb),
            common_enums::CardNetwork::DinersClub => Ok(Self::Diners),
            common_enums::CardNetwork::Discover => Ok(Self::Discover),
            common_enums::CardNetwork::CartesBancaires => Ok(Self::Cb),
            common_enums::CardNetwork::UnionPay => Ok(Self::Unionpay),
            common_enums::CardNetwork::Maestro => Ok(Self::Maestro),
            // HiPay's product enum has no code for these networks. Sending the network
            // name anyway (the previous behaviour for Interac and RuPay) is rejected with
            // 1020003, and an empty string fails validation, so the request is refused
            // locally with an actionable error instead.
            common_enums::CardNetwork::Interac
            | common_enums::CardNetwork::RuPay
            | common_enums::CardNetwork::Star
            | common_enums::CardNetwork::Pulse
            | common_enums::CardNetwork::Accel
            | common_enums::CardNetwork::Nyce
            | common_enums::CardNetwork::Prop
            | common_enums::CardNetwork::PrivateLabel
            | common_enums::CardNetwork::Dinacard => {
                Err(error_stack::report!(IntegrationError::NotSupported {
                    message: format!("card network {network:?}"),
                    connector: "hipay",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Route this card network to a connector that supports it; HiPay's \
                             payment_product enum has no code for it."
                                .to_string(),
                        ),
                        doc_url: Some(
                            "https://developer.hipay.com/doc-api/enterprise/gateway/".to_string(),
                        ),
                        additional_context: None,
                    },
                }))
            }
        }
    }
}

/// HiPay `eci` — the request-side Electronic Commerce Indicator, which declares the
/// *nature* of the transaction before authentication. It is not an authentication result:
/// HiPay returns the resulting ECI on `threeDSecure.eci`. Only 1, 2, 7, 9 and 10 are
/// accepted by the order request; anything else is rejected with `1020002 Unsupported ECI`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HipayEci {
    /// Mail order / telephone order.
    MailOrTelephoneOrder,
    /// Recurring MO/TO.
    RecurringMailOrTelephoneOrder,
    /// Secure e-commerce with SSL/TLS encryption — the initial customer-initiated
    /// transaction and subsequent one-click payments.
    SecureEcommerce,
    /// Recurring e-commerce — merchant-initiated transactions on a stored alias.
    RecurringEcommerce,
    /// Point-of-sale payment.
    PointOfSale,
}

impl HipayEci {
    fn code(self) -> u8 {
        match self {
            Self::MailOrTelephoneOrder => 1,
            Self::RecurringMailOrTelephoneOrder => 2,
            Self::SecureEcommerce => 7,
            Self::RecurringEcommerce => 9,
            Self::PointOfSale => 10,
        }
    }
}

impl Serialize for HipayEci {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(self.code())
    }
}

/// HiPay `order_category_code` — a closed set of category codes (3x/4x CB products).
/// Values outside the published list are rejected, so the caller-supplied
/// `order_category` is validated against it rather than forwarded blindly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HipayOrderCategoryCode(u16);

impl HipayOrderCategoryCode {
    const ALLOWED: [u16; 16] = [
        0, 4722, 4812, 5193, 5200, 5261, 5499, 5571, 5651, 5734, 5941, 5946, 7278, 7298, 7361, 7929,
    ];

    /// `None` when the value is not one of HiPay's published category codes.
    fn from_code(code: u16) -> Option<Self> {
        Self::ALLOWED.contains(&code).then_some(Self(code))
    }
}

impl Serialize for HipayOrderCategoryCode {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u16(self.0)
    }
}

/// One line of HiPay's `basket`. HiPay carries the basket as a **single form field holding
/// a JSON-encoded array**, not as repeated indexed form fields.
///
/// HiPay's OpenAPI document declares no component schema for a basket item; the field
/// names come from the example attached to the `basket` parameter. Money is carried as a
/// major-unit decimal string, consistent with `amount` on the order itself.
#[derive(Debug, Clone, Serialize)]
pub struct HipayBasketItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub european_article_numbering: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_reference: Option<String>,
    pub name: String,
    #[serde(rename = "type")]
    pub item_type: HipayBasketItemType,
    pub quantity: u16,
    pub unit_price: StringMajorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tax_rate: Option<String>,
    pub total_amount: StringMajorUnit,
}

/// HiPay's basket line `type`. The published example only ever shows `good`.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HipayBasketItemType {
    Good,
}

/// Billing / customer details. HiPay carries these as **flat, top-level, snake_case form
/// fields** on `POST /v1/order` — there is no nested `billing` or `customer` object.
///
/// They are ordinary order parameters, not PSD2 parameters (HiPay marks the genuinely
/// 3DS-scoped fields with "This parameter is specific to the PSD2."), so they are sent on
/// every authorization. They are also the only AVS input HiPay has: `streetaddress` and
/// `zipcode` drive the address check, and without them `avsResult` comes back blank.
#[derive(Debug, Default, Clone, Serialize)]
pub struct HipayBillingData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firstname: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lastname: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub streetaddress: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub streetaddress2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<Secret<String>>,
    /// Required by HiPay when `country` is US or CA.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zipcode: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<common_enums::CountryAlpha2>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<Email>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<Secret<String>>,
}

/// Delivery details — the flat `shipto_*` family.
///
/// The family is not a mirror of the billing family: HiPay publishes no `shipto_email`,
/// `shipto_house_extension` or `shipto_streetaddress3`. `shipto_recipientinfo`,
/// `shipto_house_number`, `shipto_gender` and `shipto_msisdn` exist on HiPay's side but
/// have no source in the UCS shipping address, so they are not modelled here.
#[derive(Debug, Default, Clone, Serialize)]
pub struct HipayShippingData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipto_firstname: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipto_lastname: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipto_streetaddress: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipto_streetaddress2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipto_city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipto_state: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipto_zipcode: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipto_country: Option<common_enums::CountryAlpha2>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipto_phone: Option<Secret<String>>,
}

/// PSD2 `browser_info` block, which HiPay feeds into the 3DS2 authentication it performs
/// itself.
///
/// HiPay takes this as a **single form field holding a JSON object**, the same convention it
/// uses for `basket` — not as bracket-notation sub-fields. Verified against the sandbox:
/// `browser_info[java_enabled]` is rejected with `1010502` ("The format of java_enabled
/// field is invalid. Please respect the format boolean.") for every scalar spelling
/// (`true`, `1`, `TRUE`), while the same data inside a JSON object with real JSON booleans
/// is accepted. Only sent when the request asks for 3-D Secure — HiPay marks the whole
/// block "specific to the PSD2".
#[derive(Debug, Clone, Serialize)]
pub struct HipayBrowserInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub java_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub javascript_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipaddr: Option<std::net::IpAddr>,
    /// HiPay requires the `Accept` header the cardholder's browser sent. `*/*` is the
    /// value used when the caller supplies none, matching the reference integration.
    pub http_accept: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_user_agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_depth: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screen_height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screen_width: Option<u32>,
    /// UTC offset in minutes. HiPay declares it as a string, e.g. `"-120"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
}

impl From<&BrowserInformation> for HipayBrowserInfo {
    fn from(browser_info: &BrowserInformation) -> Self {
        Self {
            java_enabled: browser_info.java_enabled,
            javascript_enabled: browser_info.java_script_enabled,
            ipaddr: browser_info.ip_address,
            http_accept: browser_info
                .accept_header
                .clone()
                .unwrap_or_else(|| "*/*".to_string()),
            http_user_agent: browser_info.user_agent.clone(),
            language: browser_info.language.clone(),
            color_depth: browser_info.color_depth,
            screen_height: browser_info.screen_height,
            screen_width: browser_info.screen_width,
            timezone: browser_info.time_zone.map(|offset| offset.to_string()),
        }
    }
}

/// PSD2 `device_channel`: the environment the authentication runs in. HiPay documents `2`
/// (browser) as the default and it is the only channel this connector serves — the
/// `browser_info` block it accompanies is browser data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HipayDeviceChannel {
    AppBased,
    Browser,
    RequestorInitiated,
}

impl HipayDeviceChannel {
    fn code(self) -> u8 {
        match self {
            Self::AppBased => 1,
            Self::Browser => 2,
            Self::RequestorInitiated => 3,
        }
    }
}

impl Serialize for HipayDeviceChannel {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(self.code())
    }
}

/// Connector-specific extras the caller echoes back on Authorize through
/// `connector_feature_data`.
///
/// HiPay's order endpoint only accepts a Secure Vault `cardtoken` — raw PANs are rejected
/// — and a token-only authorize payload (`PaymentMethod::Token`) carries no card network,
/// while `payment_product` is mandatory. Both values are produced by
/// `PaymentMethodService/Tokenize`, which returns the token alongside the card `brand` and
/// the co-branded `domestic_network`, so the caller passes back whichever it needs here.
/// Same channel and shape as the Paysafe payment-handle token
/// (`paysafe/transformers.rs::paysafe_feature_data_handle_token`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HipayMeta {
    #[serde(default)]
    pub cardtoken: Option<Secret<String>>,
    #[serde(default)]
    pub payment_product: Option<HipayPaymentProduct>,
}

fn hipay_meta(resource_common_data: &PaymentFlowData) -> Option<HipayMeta> {
    resource_common_data
        .connector_feature_data
        .clone()
        .and_then(|feature_data| feature_data.parse_value::<HipayMeta>("HipayMeta").ok())
}

#[derive(Debug, Serialize)]
pub struct HipayPaymentsRequest {
    // --- Core order ---
    pub payment_product: HipayPaymentProduct,
    pub orderid: String,
    pub operation: Operation,
    pub description: String,
    pub currency: common_enums::Currency,
    pub amount: StringMajorUnit,
    pub cardtoken: Secret<String>,

    // --- 3-D Secure intent ---
    /// `0` bypass, `1` if available, `2` mandatory. HiPay performs the authentication
    /// itself; this only declares whether it should.
    pub authentication_indicator: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eci: Option<HipayEci>,
    /// `1` asks HiPay to open a multi-use debit agreement on this customer-initiated
    /// transaction, which is how HiPay chains later merchant-initiated payments. Omitted
    /// unless the payment is set up for off-session reuse.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurring_payment: Option<u8>,
    /// `1` marks the agreement as usable for later one-click (customer-present) payments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub one_click: Option<u8>,

    // --- Redirect / notification URLs ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accept_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decline_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancel_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exception_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notify_url: Option<String>,

    // --- Dynamic descriptor ---
    /// HiPay's single descriptor parameter. There is no `merchant_name` / `dba` and no
    /// split city/phone descriptor components.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub soft_descriptor: Option<String>,

    // --- Level 2 data ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipping: Option<StringMajorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tax: Option<StringMajorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_category_code: Option<HipayOrderCategoryCode>,
    /// JSON-encoded array of `HipayBasketItem`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub basket: Option<String>,

    // --- Flat field families ---
    #[serde(flatten)]
    pub billing: HipayBillingData,
    #[serde(flatten)]
    pub shipping_details: HipayShippingData,

    // --- PSD2 3DS2 data ---
    /// JSON-encoded `HipayBrowserInfo`, carried as one form field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_info: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_channel: Option<HipayDeviceChannel>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        HipayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for HipayPaymentsRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: HipayRouterData<
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
        let common = &router_data.resource_common_data;
        let currency = request.currency;
        let meta = hipay_meta(common);

        let convert = |amount: MinorUnit| -> Result<StringMajorUnit, Self::Error> {
            item.connector
                .amount_converter
                .convert(amount, currency)
                .change_context(IntegrationError::AmountConversionFailed {
                    context: Default::default(),
                })
        };

        // `payment_product` is mandatory and drawn from a closed HiPay enum.
        //
        // 1. `connector_feature_data.payment_product` — the only channel available on a
        //    token-only payload, and where the co-branded `domestic_network` returned by
        //    Tokenize belongs (a domestic network wins over the global one on co-branded
        //    cards, which is why HiPay wants it).
        // 2. the card network on the request, when the raw card is still present.
        //
        // There is deliberately no empty-string fallback: HiPay rejects an unknown or
        // blank product with 1020003 / 1020001 and the merchant sees an opaque decline.
        let payment_product = match meta.as_ref().and_then(|meta| meta.payment_product) {
            Some(product) => product,
            None => match &request.payment_method_data {
                PaymentMethodData::Card(card_data) => card_data
                    .card_network
                    .clone()
                    .ok_or_else(|| {
                        error_stack::report!(IntegrationError::MissingRequiredField {
                            field_name: "payment_method.card.card_network",
                            context: IntegrationErrorContext {
                                suggested_action: Some(
                                    "Send the card network on the card, or the HiPay \
                                     payment_product in connector_feature_data."
                                        .to_string(),
                                ),
                                doc_url: Some(
                                    "https://developer.hipay.com/doc-api/enterprise/gateway/"
                                        .to_string(),
                                ),
                                additional_context: Some(
                                    "HiPay requires payment_product on POST /v1/order.".to_string(),
                                ),
                            },
                        })
                    })
                    .and_then(HipayPaymentProduct::try_from)?,
                _ => Err(error_stack::report!(
                    IntegrationError::MissingRequiredField {
                        field_name: "connector_feature_data.payment_product",
                        context: IntegrationErrorContext {
                            suggested_action: Some(
                                "Pass the HiPay payment_product (the brand or domestic_network \
                                 returned by PaymentMethodService/Tokenize) in \
                                 connector_feature_data."
                                    .to_string(),
                            ),
                            doc_url: Some(
                                "https://developer.hipay.com/doc-api/enterprise/gateway/"
                                    .to_string(),
                            ),
                            additional_context: Some(
                                "A token-only authorize payload carries no card network, but \
                                 HiPay requires payment_product."
                                    .to_string(),
                            ),
                        },
                    }
                ))?,
            },
        };

        // HiPay never accepts a raw PAN on the order endpoint: the card is exchanged for a
        // Secure Vault token first (PaymentMethodService/Tokenize), and the order
        // references that token. The token is a credential, so it stays `Secret`.
        let cardtoken = match &request.payment_method_data {
            PaymentMethodData::PaymentMethodToken(token_data) => token_data.token.clone(),
            _ => meta
                .as_ref()
                .and_then(|meta| meta.cardtoken.clone())
                .ok_or_else(|| {
                    error_stack::report!(IntegrationError::MissingRequiredField {
                        field_name: "payment_method_token",
                        context: IntegrationErrorContext {
                            suggested_action: Some(
                                "Mint a HiPay Secure Vault token with \
                             PaymentMethodService/Tokenize and send it as the payment method \
                             token, or echo it back in connector_feature_data.cardtoken."
                                    .to_string(),
                            ),
                            doc_url: Some(
                                "https://developer.hipay.com/doc-api/enterprise/secure-vault/"
                                    .to_string(),
                            ),
                            additional_context: Some(
                                "HiPay rejects raw card numbers on POST /v1/order.".to_string(),
                            ),
                        },
                    })
                })?,
        };

        let amount = convert(request.minor_amount)?;

        // Manual and manual-multiple capture both authorize only; everything else asks
        // HiPay to capture straight after the authorization.
        let operation = if is_manual_capture(request.capture_method) {
            Operation::Authorization
        } else {
            Operation::Sale
        };

        let is_three_ds = common.auth_type == common_enums::AuthenticationType::ThreeDs;
        // 0 bypasses 3-D Secure, 2 makes it mandatory. HiPay is itself the MPI, so this is
        // an instruction to authenticate rather than a passthrough of someone else's
        // authentication.
        let authentication_indicator = u8::from(is_three_ds) * 2;

        // A merchant-initiated payment replays a stored agreement; a customer-initiated one
        // that is set up for off-session reuse is the transaction that opens it.
        let is_merchant_initiated =
            request.mandate_id.is_some() || request.off_session == Some(true);
        let sets_up_future_mit =
            request.setup_future_usage == Some(common_enums::FutureUsage::OffSession);

        // HiPay accepts no CAVV, DS transaction id, XID, 3DS version or ACS transaction id
        // on the order request — it performs the authentication itself. The one piece of
        // externally-authenticated context it does take is the request-side ECI, which
        // declares the nature of the transaction: `7` for the initial customer-initiated
        // transaction and for later one-click payments, `9` for merchant-initiated ones.
        let eci = if is_merchant_initiated {
            Some(HipayEci::RecurringEcommerce)
        } else if sets_up_future_mit || request.authentication_data.is_some() {
            Some(HipayEci::SecureEcommerce)
        } else {
            None
        };

        // Ask HiPay to open the debit agreement only on the customer-initiated leg that
        // actually intends a later merchant-initiated payment.
        let recurring_payment = (sets_up_future_mit && !is_merchant_initiated).then_some(1u8);
        let one_click = recurring_payment;

        let return_url = request
            .router_return_url
            .clone()
            .or_else(|| common.return_url.clone());
        // HiPay's notify_url overrides the account-level server-to-server notification URL
        // for this order, so it takes the webhook URL when the caller supplies one.
        let notify_url = request.webhook_url.clone().or_else(|| return_url.clone());

        let description = common
            .description
            .clone()
            .unwrap_or_else(|| "Short Description".to_string());

        // HiPay has exactly one descriptor parameter, `soft_descriptor`.
        let soft_descriptor = request.billing_descriptor.as_ref().and_then(|descriptor| {
            descriptor
                .statement_descriptor
                .clone()
                .or_else(|| descriptor.name.as_ref().map(|name| name.peek().clone()))
        });

        // Level 2: HiPay carries order-level shipping and tax plus a descriptive basket.
        // There is no duty amount, no per-item shipping and no commodity code, so a full
        // Level 3 payload cannot be expressed.
        let l2_l3_data = common.l2_l3_data.as_deref();
        let shipping_amount = l2_l3_data
            .and_then(|data| data.order_info.as_ref())
            .and_then(|order_info| order_info.shipping_cost)
            .or(request.shipping_cost)
            .filter(|amount| amount.get_amount_as_i64() > 0)
            .map(convert)
            .transpose()?;
        let tax_amount = l2_l3_data
            .and_then(|data| data.tax_info.as_ref())
            .and_then(|tax_info| tax_info.order_tax_amount)
            .or(request.order_tax_amount)
            .filter(|amount| amount.get_amount_as_i64() > 0)
            .map(convert)
            .transpose()?;
        let order_category_code = request
            .order_category
            .as_ref()
            .and_then(|category| category.parse::<u16>().ok())
            .and_then(HipayOrderCategoryCode::from_code);

        let order_details = l2_l3_data
            .and_then(|data| data.order_info.as_ref())
            .and_then(|order_info| order_info.order_details.as_ref())
            .or(common.order_details.as_ref());
        let basket = match order_details.filter(|details| !details.is_empty()) {
            Some(details) => {
                let items = details
                    .iter()
                    .map(|detail| {
                        let unit_price = convert(detail.amount)?;
                        let total_amount = match detail.total_amount {
                            Some(total) => convert(total)?,
                            None => convert(MinorUnit::new(
                                detail.amount.get_amount_as_i64() * i64::from(detail.quantity),
                            ))?,
                        };
                        Ok(HipayBasketItem {
                            european_article_numbering: detail.upc.clone(),
                            product_reference: detail
                                .sku
                                .clone()
                                .or_else(|| detail.product_id.clone()),
                            name: detail.product_name.clone(),
                            item_type: HipayBasketItemType::Good,
                            quantity: detail.quantity,
                            unit_price,
                            tax_rate: detail.tax_rate.map(|rate| format!("{rate:.2}")),
                            total_amount,
                        })
                    })
                    .collect::<Result<Vec<_>, Self::Error>>()?;
                Some(serde_json::to_string(&items).change_context(
                    IntegrationError::RequestEncodingFailed {
                        context: Default::default(),
                    },
                )?)
            }
            None => None,
        };

        let billing = HipayBillingData {
            firstname: common.get_optional_billing_first_name(),
            lastname: common.get_optional_billing_last_name(),
            streetaddress: common.get_optional_billing_line1(),
            streetaddress2: common.get_optional_billing_line2(),
            city: common.get_optional_billing_city(),
            state: common.get_optional_billing_state(),
            zipcode: common.get_optional_billing_zip(),
            country: common.get_optional_billing_country(),
            email: common
                .get_optional_billing_email()
                .or(request.email.clone()),
            phone: common.get_optional_billing_phone_number(),
        };

        let shipping_details = HipayShippingData {
            shipto_firstname: common.get_optional_shipping_first_name(),
            shipto_lastname: common.get_optional_shipping_last_name(),
            shipto_streetaddress: common.get_optional_shipping_line1(),
            shipto_streetaddress2: common.get_optional_shipping_line2(),
            shipto_city: common.get_optional_shipping_city(),
            shipto_state: common.get_optional_shipping_state(),
            shipto_zipcode: common.get_optional_shipping_zip(),
            shipto_country: common.get_optional_shipping_country(),
            shipto_phone: common.get_optional_shipping_phone_number(),
        };

        // `browser_info` rides as one JSON form field, alongside the device channel it
        // describes.
        let browser_info = match is_three_ds
            .then(|| request.browser_info.as_ref().map(HipayBrowserInfo::from))
            .flatten()
        {
            Some(browser_info) => Some(serde_json::to_string(&browser_info).change_context(
                IntegrationError::RequestEncodingFailed {
                    context: Default::default(),
                },
            )?),
            None => None,
        };
        let device_channel = browser_info.as_ref().map(|_| HipayDeviceChannel::Browser);

        Ok(Self {
            payment_product,
            // HiPay treats `orderid` as unique (`x-uniqId`) and rejects a replay with
            // 3010004 Duplicate Order rather than replaying it idempotently.
            orderid: common.connector_request_reference_id.clone(),
            operation,
            description,
            currency,
            amount,
            cardtoken,
            authentication_indicator,
            eci,
            recurring_payment,
            one_click,
            accept_url: return_url.clone(),
            decline_url: return_url.clone(),
            pending_url: return_url.clone(),
            cancel_url: return_url.clone(),
            exception_url: return_url,
            notify_url,
            soft_descriptor,
            shipping: shipping_amount,
            tax: tax_amount,
            order_category_code,
            basket,
            billing,
            shipping_details,
            browser_info,
            device_channel,
        })
    }
}

// Response Structures aligned with Hyperswitch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentOrder {
    id: String,
}

/// HiPay AVS result codes. AVS has no request parameter of its own — it is driven by the
/// `streetaddress` and `zipcode` billing fields — and this is where the outcome comes back
/// (`avsResult` on the JSON API, `avs_result` in notifications).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HipayAvsResult {
    /// Street addresses and postal codes match.
    #[serde(rename = "Y")]
    ExactMatch,
    /// Street addresses match, postal codes do not.
    #[serde(rename = "A")]
    StreetMatch,
    /// Postal codes match, street addresses do not.
    #[serde(rename = "P")]
    PostalMatch,
    /// Neither matches.
    #[serde(rename = "N")]
    NoMatch,
    /// Not verified — incompatible address formats.
    #[serde(rename = "C")]
    NotCompatible,
    /// AVS data invalid, or AVS not allowed for this card type.
    #[serde(rename = "E")]
    NotAllowed,
    /// Address information unavailable, or the issuer does not support AVS.
    #[serde(rename = "U")]
    Unavailable,
    /// The issuer's authorization system is unavailable; retry later.
    #[serde(rename = "R")]
    Retry,
    /// The card issuer does not support AVS.
    #[serde(rename = "S")]
    NotSupported,
    /// Any code HiPay adds later. A blank `avsResult` (no AVS response obtained) is the
    /// documented default and is skipped before this enum is reached.
    #[serde(other)]
    Unknown,
}

/// HiPay CVC result codes. The CVC itself is submitted to the Secure Vault, not to the
/// order endpoint, so this is the only CVC signal on the authorization.
///
/// Only a few acquirers return a specific CVC result; for most, a successful authorization
/// implies the CVC was correct. A blank `cvcResult` on an approved transaction is normal
/// and must not be read as a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HipayCvcResult {
    #[serde(rename = "M")]
    Match,
    #[serde(rename = "N")]
    NoMatch,
    #[serde(rename = "P")]
    NotProcessed,
    /// The CVC should be on the card, but the cardholder reported that it is not.
    #[serde(rename = "S")]
    Missing,
    #[serde(rename = "U")]
    NotSupported,
    #[serde(other)]
    Unknown,
}

/// The card HiPay charged, echoed back on the transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HipayResponsePaymentMethod {
    #[serde(default)]
    pub brand: Option<String>,
    #[serde(default)]
    pub token: Option<Secret<String>>,
}

/// HiPay's debit agreement (mandate) for the card, returned once a customer-initiated
/// transaction is sent with `recurring_payment=1` and a multi-use `cardtoken`.
///
/// This is the identifier a later merchant-initiated payment replays — HiPay's model is
/// agreement-based, not scheme-NTID-based: the merchant sends `debit_agreement_id` with
/// `eci=9` rather than a stored network transaction id. `id` and `status` come back as
/// empty strings on a transaction that opened no agreement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HipayDebitAgreement {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

/// AVS and CVC outcomes, surfaced to the caller as the card `payment_checks` of the
/// connector response.
#[derive(Debug, Clone, Serialize)]
struct HipayPaymentChecks {
    #[serde(skip_serializing_if = "Option::is_none")]
    avs_result: Option<HipayAvsResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cvc_result: Option<HipayCvcResult>,
}

// Authorize Response - matches HiPay's order API response (camelCase from HiPay API)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HipayPaymentsResponse {
    status: HipayPaymentStatus,
    message: String,
    order: PaymentOrder,
    /// Present (and non-null) only when the shopper must be redirected — a 3-D Secure
    /// challenge or an APM hand-off. HiPay sends an explicit `null` otherwise.
    #[serde(default, rename = "forwardUrl")]
    forward_url: Option<String>,
    #[serde(rename = "transactionReference")]
    transaction_reference: String,
    /// Decline detail on a HTTP 200 that carries a failure status. `""` when the order
    /// succeeded.
    #[serde(default, deserialize_with = "deserialize_optional_ignoring_empty")]
    reason: Option<HipayReason>,
    /// The acquirer's authorization code. This is NOT a scheme transaction identifier.
    #[serde(default, rename = "authorizationCode")]
    authorization_code: Option<String>,
    /// `""` when no AVS response was obtained, which is the documented default.
    #[serde(
        default,
        rename = "avsResult",
        deserialize_with = "deserialize_optional_ignoring_empty"
    )]
    avs_result: Option<HipayAvsResult>,
    /// `""` or absent for the many acquirers that return no explicit CVC result.
    #[serde(
        default,
        rename = "cvcResult",
        deserialize_with = "deserialize_optional_ignoring_empty"
    )]
    cvc_result: Option<HipayCvcResult>,
    #[serde(
        default,
        rename = "paymentMethod",
        deserialize_with = "deserialize_optional_ignoring_empty"
    )]
    payment_method: Option<HipayResponsePaymentMethod>,
    /// The debit agreement HiPay opened for this card. This is HiPay's chaining
    /// identifier for later merchant-initiated payments — see `HipayDebitAgreement`.
    #[serde(
        default,
        rename = "debitAgreement",
        deserialize_with = "deserialize_optional_ignoring_empty"
    )]
    debit_agreement: Option<HipayDebitAgreement>,
}

// Generic Maintenance Response for Capture/Void/Refund operations (camelCase from HiPay API)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HipayMaintenanceResponse<S> {
    status: S,
    message: String,
    #[serde(rename = "transactionReference")]
    transaction_reference: String,
    /// Decline detail. A maintenance operation HiPay accepted but refused comes back as
    /// HTTP 200 with a failure status and this nested object; `""` when it succeeded.
    #[serde(default, deserialize_with = "deserialize_optional_ignoring_empty")]
    reason: Option<HipayReason>,
}

// Type aliases for different flows - operation-specific types
pub type HipayAuthorizeResponse = HipayPaymentsResponse;
pub type HipayCaptureResponse = HipayMaintenanceResponse<HipayPaymentStatus>;
pub type HipayVoidResponse = HipayMaintenanceResponse<HipayPaymentStatus>;
pub type HipayRefundResponse = HipayMaintenanceResponse<HipayRefundStatus>;
pub type HipayPSyncResponse = HipaySyncResponse;
pub type HipayRSyncResponse = HipayRefundSyncResponse;

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<HipayAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<HipayAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Convert HipayPaymentStatus enum directly to AttemptStatus using From trait
        let status = AttemptStatus::from(item.response.status.clone());

        // AVS and CVC outcomes, the acquirer authorization code and the brand HiPay
        // charged are all carried on the connector response so the caller keeps them.
        let payment_checks = match (item.response.avs_result, item.response.cvc_result) {
            (None, None) => None,
            (avs_result, cvc_result) => serde_json::to_value(HipayPaymentChecks {
                avs_result,
                cvc_result,
            })
            .ok(),
        };
        let card_network = item
            .response
            .payment_method
            .as_ref()
            .and_then(|payment_method| payment_method.brand.clone());
        let connector_response = (payment_checks.is_some()
            || card_network.is_some()
            || item.response.authorization_code.is_some())
        .then(|| {
            ConnectorResponseData::with_additional_payment_method_data(
                AdditionalPaymentMethodConnectorResponse::Card {
                    authentication_data: None,
                    payment_checks,
                    card_network,
                    domestic_network: None,
                    auth_code: item.response.authorization_code.clone(),
                },
            )
        });

        // HiPay writes `""` into both members when no agreement was opened.
        let debit_agreement_id = item
            .response
            .debit_agreement
            .as_ref()
            .and_then(|agreement| agreement.id.clone())
            .filter(|agreement_id| !agreement_id.trim().is_empty());

        // Check if status is failure to return error response
        let response = if status == AttemptStatus::Failure {
            Err(build_transaction_error_response(
                item.response.reason.as_ref(),
                &item.response.message,
                item.http_code,
                Some(item.response.transaction_reference.clone()),
                None,
            ))
        } else {
            // A populated forwardUrl means HiPay needs the shopper at its page — a 3-D
            // Secure challenge or an APM hand-off.
            let redirection_data = item
                .response
                .forward_url
                .as_ref()
                .filter(|url| !url.is_empty())
                .map(|url| {
                    Box::new(domain_types::router_response_types::RedirectForm::Uri {
                        uri: url.clone(),
                    })
                });

            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.response.transaction_reference.clone(),
                ),
                redirection_data,
                // HiPay chains merchant-initiated payments on the debit agreement it opens
                // for the card, so the agreement id is the mandate reference a later MIT
                // replays as `debit_agreement_id`.
                mandate_reference: debit_agreement_id.map(|agreement_id| {
                    Box::new(MandateReference {
                        connector_mandate_id: Some(agreement_id),
                        payment_method_id: None,
                        connector_mandate_request_reference_id: None,
                        mandate_metadata: None,
                    })
                }),
                connector_metadata: None,
                // HiPay's Gateway API returns no scheme/network transaction identifier.
                // The published Transaction schema and a live sandbox authorization (a
                // recurring CIT with `recurring_payment=1`, which is the case that would
                // carry one) both contain no `ntid`, `scheme_transaction_id`,
                // `transaction_id_life_cycle`, `provider_agreement_ref` or
                // `transaction_link_identifier`; `acquirerTransactionReference` appears
                // only in the notification documentation, never in a gateway response.
                // `transactionReference` is HiPay's own reference and `authorizationCode`
                // is the acquirer auth code — neither is a scheme NTID, and mapping either
                // here would corrupt merchant-initiated replay. HiPay's chaining key is
                // `debitAgreement.id`, carried above as the mandate reference.
                network_txn_id: None,
                // `network_txn_link_id` is the scheme TLID. HiPay only ever accepts a TLID
                // *inbound*, when migrating an agreement from another provider; it never
                // returns one.
                network_txn_link_id: None,
                connector_response_reference_id: Some(item.response.order.id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            })
        };

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                connector_response,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// Tokenization Structures
#[derive(Debug, Serialize, Deserialize)]
pub struct HipayTokenRequest<T: PaymentMethodDataTypes> {
    pub card_number: domain_types::payment_method_data::RawCardNumber<T>,
    pub card_expiry_month: Secret<String>,
    pub card_expiry_year: Secret<String>,
    pub card_holder: Secret<String>,
    pub cvc: Secret<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        HipayRouterData<
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
            T,
        >,
    > for HipayTokenRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: HipayRouterData<
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        match &item.router_data.request.payment_method_data {
            PaymentMethodData::Card(card_data) => Ok(Self {
                card_number: card_data.card_number.clone(),
                card_expiry_month: card_data.card_exp_month.clone(),
                // The vault expects a four-digit year; a two-digit year reaches it
                // verbatim otherwise and the token is minted with the wrong expiry.
                card_expiry_year: card_data.get_expiry_year_4_digit(),
                // The cardholder name is a required vault field. Defaulting it to an empty
                // string mints a token with no holder name and defers the failure to the
                // authorization, so the absence is reported here instead.
                card_holder: item
                    .router_data
                    .resource_common_data
                    .get_optional_billing_full_name()
                    .ok_or_else(|| {
                        error_stack::report!(IntegrationError::MissingRequiredField {
                            field_name: "billing.address.first_name",
                            context: IntegrationErrorContext {
                                suggested_action: Some(
                                    "Send the cardholder name in the billing address.".to_string(),
                                ),
                                doc_url: Some(
                                    "https://developer.hipay.com/doc-api/enterprise/secure-vault/"
                                        .to_string(),
                                ),
                                additional_context: Some(
                                    "HiPay's Secure Vault requires card_holder.".to_string(),
                                ),
                            },
                        })
                    })?,
                cvc: card_data.card_cvc.clone(),
            }),
            PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::Wallet(_)
            | PaymentMethodData::PayLater(_)
            | PaymentMethodData::BankRedirect(_)
            | PaymentMethodData::BankDebit(_)
            | PaymentMethodData::BankTransfer(_)
            | PaymentMethodData::Crypto(_)
            | PaymentMethodData::MandatePayment
            | PaymentMethodData::Reward
            | PaymentMethodData::RealTimePayment(_)
            | PaymentMethodData::CardWithNoCvc(_)
            | PaymentMethodData::MobilePayment(_)
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::PaymentMethodToken(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
                Err(IntegrationError::NotImplemented(
                    "Payment method not supported for tokenization".to_string(),
                    Default::default(),
                ))
                .change_context(IntegrationError::NotImplemented(
                    "Payment method".to_string(),
                    Default::default(),
                ))
            }
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct HipayTokenResponse {
    pub token: Secret<String>,
    pub request_id: String,
    pub brand: String,
    /// The co-branded (domestic) network of the card, when HiPay detects one. This is the
    /// value HiPay wants back as `payment_product` on the order: on a co-branded card the
    /// domestic network takes precedence over the global one.
    #[serde(default)]
    pub domestic_network: Option<String>,
    pub pan: Secret<String>,
    pub card_holder: Secret<String>,
    pub card_expiry_month: Secret<String>,
    pub card_expiry_year: Secret<String>,
    pub issuer: Option<String>,
    pub country: Option<common_enums::CountryAlpha2>,
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<HipayTokenResponse, Self>>
    for RouterDataV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<HipayTokenResponse, Self>) -> Result<Self, Self::Error> {
        use hyperswitch_masking::ExposeInterface;
        // The brand and the co-branded domestic network travel back on the connector
        // response: they are what the Authorize leg needs as `payment_product`, and the
        // token-only authorize payload carries no card network of its own.
        let connector_response = ConnectorResponseData::with_additional_payment_method_data(
            AdditionalPaymentMethodConnectorResponse::Card {
                authentication_data: None,
                payment_checks: None,
                card_network: Some(item.response.brand.clone()),
                domestic_network: item.response.domestic_network.clone(),
                auth_code: None,
            },
        );
        Ok(Self {
            response: Ok(PaymentMethodTokenResponse {
                token: item.response.token.expose(),
                connector_payment_method_id: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                connector_response: Some(connector_response),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// Helper function to map v3 API integer status codes to AttemptStatus
// Matches Hyperswitch's get_sync_status function
fn get_sync_status(state: i32) -> AttemptStatus {
    match state {
        9 => AttemptStatus::AuthenticationFailed,
        10 => AttemptStatus::Failure,
        11 => AttemptStatus::Failure,
        12 => AttemptStatus::Pending,
        13 => AttemptStatus::Failure,
        14 => AttemptStatus::Failure,
        15 => AttemptStatus::Voided,
        16 => AttemptStatus::Authorized,
        17 => AttemptStatus::CaptureInitiated,
        18 => AttemptStatus::Charged,
        19 => AttemptStatus::PartialCharged,
        29 => AttemptStatus::Failure,
        73 => AttemptStatus::CaptureFailed,
        74 => AttemptStatus::Pending,
        75 => AttemptStatus::VoidInitiated,
        77 => AttemptStatus::AuthenticationPending,
        78 => AttemptStatus::Failure,
        200 => AttemptStatus::Pending,
        1 => AttemptStatus::Started,
        5 => AttemptStatus::AuthenticationFailed,
        6 => AttemptStatus::Pending,
        7 => AttemptStatus::AuthenticationPending,
        8 => AttemptStatus::AuthenticationFailed,
        20 => AttemptStatus::Charged,
        21 => AttemptStatus::Charged,
        22 => AttemptStatus::Charged,
        23 => AttemptStatus::Charged,
        40 => AttemptStatus::AuthenticationPending,
        41 => AttemptStatus::AuthenticationSuccessful,
        51 => AttemptStatus::Failure,
        61 => AttemptStatus::Pending,
        63 => AttemptStatus::Failure,
        _ => AttemptStatus::Failure,
    }
}

// Payment Sync Response Implementation
// Uses HipaySyncResponse enum with v3 API flat structure
impl TryFrom<ResponseRouterData<HipayPSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<HipayPSyncResponse, Self>) -> Result<Self, Self::Error> {
        // Handle sync response - could be Response or Error variant
        match item.response {
            HipaySyncResponse::Response { id, status, .. } => {
                // Convert i32 status code to AttemptStatus using mapping function
                let attempt_status = get_sync_status(status);

                Ok(Self {
                    response: Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(id.to_string()),
                        redirection_data: None,
                        mandate_reference: None,
                        connector_metadata: None,
                        network_txn_id: None,
                        network_txn_link_id: None,
                        connector_response_reference_id: None,
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                        splits: None,
                        payment_account_reference: None,
                    }),
                    resource_common_data: PaymentFlowData {
                        status: attempt_status,
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                })
            }
            HipaySyncResponse::Error { message, code } => Ok(Self {
                response: Err(domain_types::router_data::ErrorResponse {
                    code: code.to_string(),
                    message: message.clone(),
                    reason: Some(message),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: item
                        .router_data
                        .request
                        .connector_transaction_id
                        .get_connector_transaction_id()
                        .ok(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            }),
        }
    }
}

// Capture Request Structure
#[derive(Debug, Serialize, Deserialize)]
pub struct HipayCaptureRequest {
    pub operation: HipayOperation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<common_enums::Currency>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<StringMajorUnit>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        HipayRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for HipayCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: HipayRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Convert amount to StringMajorUnit (HiPay expects decimal format)
        let amount = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.minor_amount_to_capture,
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        Ok(Self {
            operation: HipayOperation::Capture,
            currency: Some(item.router_data.request.currency),
            amount: Some(amount),
        })
    }
}

// Capture Response Implementation
// Uses HipayMaintenanceResponse<HipayPaymentStatus> with direct enum conversion
impl TryFrom<ResponseRouterData<HipayCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<HipayCaptureResponse, Self>) -> Result<Self, Self::Error> {
        // Convert HipayPaymentStatus enum directly to AttemptStatus using From trait
        let status = AttemptStatus::from(item.response.status.clone());

        // Check if status indicates failure
        let response = if status == AttemptStatus::Failure || status == AttemptStatus::CaptureFailed
        {
            Err(build_transaction_error_response(
                item.response.reason.as_ref(),
                &item.response.message,
                item.http_code,
                Some(item.response.transaction_reference.clone()),
                None,
            ))
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.response.transaction_reference.clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            })
        };

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// Refund Request Structure
#[derive(Debug, Serialize, Deserialize)]
pub struct HipayRefundRequest {
    pub operation: HipayOperation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<common_enums::Currency>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<StringMajorUnit>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        HipayRouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for HipayRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: HipayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Convert minor unit amount to StringMajorUnit (HiPay expects decimal format)
        let amount = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.minor_refund_amount,
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        Ok(Self {
            operation: HipayOperation::Refund,
            currency: Some(item.router_data.request.currency),
            amount: Some(amount),
        })
    }
}

// Refund Response Implementation
// Uses HipayMaintenanceResponse<HipayRefundStatus> with From trait conversion
impl TryFrom<ResponseRouterData<HipayRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<HipayRefundResponse, Self>) -> Result<Self, Self::Error> {
        // Convert HipayRefundStatus enum directly to RefundStatus using From trait
        let refund_status = RefundStatus::from(item.response.status.clone());

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.transaction_reference.clone(),
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            ..item.router_data
        })
    }
}

// Refund Sync Response Implementation
// Uses HipayRefundSyncResponse JSON structure from v3 API
impl TryFrom<ResponseRouterData<HipayRSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<HipayRSyncResponse, Self>) -> Result<Self, Self::Error> {
        // Map numeric status codes to RefundStatus (matching Hyperswitch)
        // Status codes from HiPay API documentation:
        // 24 = Refund Requested (Pending)
        // 25 = Refunded (Success)
        // 26 = Partially Refunded (Success)
        // 65 = Refund Refused (Failure)
        let refund_status = match item.response.status {
            25 | 26 => RefundStatus::Success,
            65 => RefundStatus::Failure,
            24 => RefundStatus::Pending,
            _ => RefundStatus::Pending, // Default to Pending for unknown statuses
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id.to_string(),
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            ..item.router_data
        })
    }
}

// Void Request Structure
#[derive(Debug, Serialize, Deserialize)]
pub struct HipayVoidRequest {
    pub operation: HipayOperation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<common_enums::Currency>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<StringMajorUnit>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        HipayRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for HipayVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: HipayRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            operation: HipayOperation::Cancel,
            currency: item.router_data.request.currency,
            amount: None, // None for void requests
        })
    }
}

// Void Response Implementation
// Uses HipayMaintenanceResponse<HipayPaymentStatus> with direct enum conversion
impl TryFrom<ResponseRouterData<HipayVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<HipayVoidResponse, Self>) -> Result<Self, Self::Error> {
        // Convert HipayPaymentStatus enum directly to AttemptStatus using From trait
        let status = AttemptStatus::from(item.response.status.clone());

        // Check if status indicates void failure
        let response = if status == AttemptStatus::Failure || status == AttemptStatus::VoidFailed {
            Err(build_transaction_error_response(
                item.response.reason.as_ref(),
                &item.response.message,
                item.http_code,
                Some(item.response.transaction_reference.clone()),
                None,
            ))
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.response.transaction_reference.clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            })
        };

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ========================================================================================
// GetFormData TRAIT IMPLEMENTATIONS
// ========================================================================================
// These implementations enable multipart/form-data request format for HiPay API

// GetFormData implementation for HipayTokenRequest
impl<T: PaymentMethodDataTypes + Serialize> GetFormData for HipayTokenRequest<T> {
    fn get_form_data(&self) -> MultipartData {
        build_form_from_struct(self).unwrap_or_else(|_| MultipartData::new())
    }
}

// GetFormData implementation for HipayPaymentsRequest
impl GetFormData for HipayPaymentsRequest {
    fn get_form_data(&self) -> MultipartData {
        build_form_from_struct(self).unwrap_or_else(|_| MultipartData::new())
    }
}

// GetFormData implementation for HipayCaptureRequest
impl GetFormData for HipayCaptureRequest {
    fn get_form_data(&self) -> MultipartData {
        build_form_from_struct(self).unwrap_or_else(|_| MultipartData::new())
    }
}

// GetFormData implementation for HipayVoidRequest
impl GetFormData for HipayVoidRequest {
    fn get_form_data(&self) -> MultipartData {
        build_form_from_struct(self).unwrap_or_else(|_| MultipartData::new())
    }
}

// GetFormData implementation for HipayRefundRequest
impl GetFormData for HipayRefundRequest {
    fn get_form_data(&self) -> MultipartData {
        build_form_from_struct(self).unwrap_or_else(|_| MultipartData::new())
    }
}
