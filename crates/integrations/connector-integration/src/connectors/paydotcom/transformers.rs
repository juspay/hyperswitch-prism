//! Pay.com (`paydotcom`) request/response transformers.
//!
//! Scope: **cards only, one-time payments**. Flows implemented here are Authorize,
//! PSync, Capture, Void, Refund and RSync. Wallets, mandates/MIT, recurring, payouts
//! and stored-token reuse are deliberately absent.
//!
//! Wire-format notes that are easy to get wrong (all verified against the Pay.com
//! OpenAPI documents embedded in <https://apiref.pay.com/reference/>):
//!
//! * The card security code field is spelled **`cvc`** inside `source_data.card`.
//!   (`payment_method_options.card.security_checks.cvv_check` legitimately uses `cvv`
//!   — that is a different object and is not normalised here.)
//! * `amount` is a JSON **integer** in minor units on create, but `amount_to_capture`
//!   and `amount_to_refund` are JSON **strings** in minor units.
//! * Currency is lower-case ISO-4217 on the wire.
//! * A Hold's status enum in the OpenAPI omits `canceled`, yet `POST /v1/holds/{id}/cancel`
//!   returns exactly that — so `Canceled` is included below or deserialization panics.
//! * `POST /v1/holds/{id}/capture` answers with a **Charge carrying a brand-new id**;
//!   the Capture transformer rewrites `connector_transaction_id` to it, otherwise a
//!   later refund (which requires a `chrg_` id) is rejected by Pay.com.
//! * Gateway-driven 3DS spans three flow executions (PreAuthenticate -> Authenticate ->
//!   Authorize); the `chrg_`/`hld_` id travels between them on `authentication_data`.
//!   See `PaydotcomAuthorizeLeg` and the connector module docs.

use common_enums::{AttemptStatus, RefundStatus};
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    request::Method,
    types::{MinorUnit, StringMinorUnit},
};
use domain_types::{
    connector_flow::{
        Authenticate, Authorize, Capture, PSync, PreAuthenticate, RSync, Refund, Void,
    },
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthenticateData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, ResponseId,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::{Card, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_request_types::{AuthenticationData, BrowserInformation},
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::{connectors::paydotcom::PaydotcomRouterData, types::ResponseRouterData};

// ===== FLOW RESPONSE TYPE ALIASES =====
// `create_all_prerequisites!` derives a unique `…Templating` struct from each
// `response_body` identifier, so a response type reused by several flows needs one
// alias per flow.

/// Authorize answers with a Charge (auto capture) or a Hold (manual capture).
pub type PaydotcomAuthorizeResponse = PaydotcomPaymentsResponse;
/// PSync reads back whichever of the two the id prefix pointed at.
pub type PaydotcomPSyncResponse = PaydotcomPaymentsResponse;
/// Capture answers with a Charge; Void answers with a canceled Hold.
pub type PaydotcomCaptureResponse = PaydotcomPaymentsResponse;
pub type PaydotcomVoidResponse = PaydotcomPaymentsResponse;
/// PreAuthenticate answers with the Charge/Hold it parked on `requires_authentication`.
pub type PaydotcomPreAuthenticateResponse = PaydotcomPaymentsResponse;
/// Authenticate answers with the authentication session carrying the challenge URL.
pub type PaydotcomAuthenticateResponse = PaydotcomPaymentsResponse;
/// RSync reuses the Refund object returned by `POST /v1/refunds`.
pub type PaydotcomRefundSyncResponse = PaydotcomRefundResponse;

// ===== RESOURCE ID PREFIXES =====

/// `^chrg_\d{18,20}` — a Charge.
pub const CHARGE_ID_PREFIX: &str = "chrg_";
/// `^hld_\d{18,20}` — a Hold.
pub const HOLD_ID_PREFIX: &str = "hld_";

// ===== AUTH =====

#[derive(Debug, Clone)]
pub struct PaydotcomAuthType {
    /// Sent as the `x-paycom-api-key` header. `test_…` on sandbox, `live_…` in production.
    pub api_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for PaydotcomAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Paydotcom { api_key, .. } => Ok(Self {
                api_key: api_key.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Expected ConnectorSpecificConfig::Paydotcom { api_key }; received a \
                             different connector variant"
                                .to_string(),
                        ),
                        suggested_action: Some(
                            "Configure the connector as Paydotcom with a HeaderKey api_key"
                                .to_string(),
                        ),
                        doc_url: None,
                    },
                }
            )),
        }
    }
}

// ===== SHARED HELPERS =====

fn amount_conversion_error(context: &str) -> IntegrationError {
    IntegrationError::AmountConversionFailed {
        context: IntegrationErrorContext {
            additional_context: Some(context.to_string()),
            suggested_action: None,
            doc_url: None,
        },
    }
}

/// Pay.com spells `currency` as `^[a-z]{3}$` on the wire, while `common_enums::Currency`
/// serialises UPPERCASE. These adapters keep the currency typed as the enum everywhere in
/// this module and normalise the case at the serde boundary, in both directions.
mod paydotcom_currency {
    use std::str::FromStr;

    use common_enums::Currency;
    use serde::{de::Error as _, Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S: Serializer>(
        currency: &Currency,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&currency.to_string().to_lowercase())
    }

    /// Optional counterpart, for the response bodies where Pay.com may omit the field.
    pub(super) mod option {
        use super::{Currency, Deserialize, Deserializer, FromStr, Serializer};

        pub(in super::super) fn serialize<S: Serializer>(
            currency: &Option<Currency>,
            serializer: S,
        ) -> Result<S::Ok, S::Error> {
            match currency {
                Some(currency) => super::serialize(currency, serializer),
                None => serializer.serialize_none(),
            }
        }

        pub(in super::super) fn deserialize<'de, D: Deserializer<'de>>(
            deserializer: D,
        ) -> Result<Option<Currency>, D::Error> {
            let raw = <Option<String>>::deserialize(deserializer)?;
            raw.map(|raw| Currency::from_str(&raw.to_uppercase()).map_err(serde::de::Error::custom))
                .transpose()
        }
    }
}

// ===== REQUEST: CREATE CHARGE / CREATE HOLD =====

/// Body of `POST /v1/charges` **and** `POST /v1/holds` — the two endpoints take an
/// identical payload in this scope, only the URL differs (see `paydotcom.rs`).
#[derive(Debug, Serialize)]
pub struct PaydotcomCreateResourceRequest<T: PaymentMethodDataTypes> {
    /// Minor units, JSON integer.
    pub amount: MinorUnit,
    /// Serialised as lower-case ISO-4217; see `paydotcom_currency`.
    #[serde(serialize_with = "paydotcom_currency::serialize")]
    pub currency: common_enums::Currency,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_reference_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method_options: Option<PaydotcomPaymentMethodOptions>,
    pub source_data: PaydotcomSourceData<T>,
}

/// Body of `POST /v1/sessions/authentication/linked` — mints the challenge URL for a
/// charge/hold already parked on `requires_authentication`. This is the Authenticate
/// flow's request body (gateway 3DS leg 2).
#[derive(Debug, Serialize)]
pub struct PaydotcomAuthenticateRequest {
    /// The `chrg_…` / `hld_…` id the session authenticates.
    pub resource: String,
    pub return_url: String,
    /// `false` keeps the authorization on our side: Pay.com redirects straight back and
    /// only authorizes once `/confirm` is sent, so the final status is read synchronously
    /// instead of arriving on a webhook (webhooks are out of scope here).
    pub confirm: bool,
}

/// `POST /v1/{charges|holds}/{id}/confirm` documents no request body. The macro-generated
/// `get_request_body` always emits one, so an empty object is sent; Pay.com ignores it.
#[derive(Debug, Serialize)]
pub struct PaydotcomConfirmRequest {}

/// The Authorize flow covers two different HTTP calls (see `PaydotcomAuthorizeLeg`), so
/// its request body is a union. `untagged` keeps each variant's own JSON shape.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum PaydotcomAuthorizeRequest<T: PaymentMethodDataTypes> {
    /// No-3DS or external-MPI 3DS: create the Charge/Hold outright.
    Create(Box<PaydotcomCreateResourceRequest<T>>),
    /// Gateway 3DS leg 3: settle after the shopper came back from the challenge.
    Confirm(PaydotcomConfirmRequest),
}

/// PreAuthenticate always creates the Charge/Hold that the challenge will authenticate.
pub type PaydotcomPreAuthenticateRequest<T> = PaydotcomCreateResourceRequest<T>;

#[derive(Debug, Serialize)]
pub struct PaydotcomPaymentMethodOptions {
    pub card: PaydotcomCardOptions,
}

#[derive(Debug, Serialize)]
pub struct PaydotcomCardOptions {
    pub request_threed_secure: PaydotcomThreeDsRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PaydotcomThreeDsRequest {
    None,
    Automatic,
    Challenge,
    Exemption,
}

#[derive(Debug, Serialize)]
pub struct PaydotcomSourceData<T: PaymentMethodDataTypes> {
    #[serde(rename = "type")]
    pub source_type: PaydotcomSourceType,
    pub card: PaydotcomCardSourceDetails<T>,
    /// The 3DS guide places this beside `card` (not inside it) and requires `email`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_details: Option<PaydotcomBillingDetails>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PaydotcomSourceType {
    Card,
}

#[derive(Debug, Serialize)]
pub struct PaydotcomCardSourceDetails<T: PaymentMethodDataTypes> {
    pub number: RawCardNumber<T>,
    /// Two digits, zero padded.
    pub expiry_month: Secret<String>,
    /// Four digits.
    pub expiry_year: Secret<String>,
    /// `cvc`, **not** `cvv` — see the module docs.
    pub cvc: Secret<String>,
    /// Cardholder name. Mandatory: Pay.com expects the name as it appears on the card,
    /// so the billing name is deliberately NOT used as a fallback — the two can differ.
    pub name: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_address: Option<PaydotcomAddress>,
    /// External-MPI pass-through. Mutually exclusive with `authentication_context`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_ds: Option<PaydotcomThreeDsRaw>,
    /// Gateway-driven 3DS input. Mutually exclusive with `three_ds`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication_context: Option<PaydotcomAuthenticationContext>,
}

#[derive(Debug, Serialize)]
pub struct PaydotcomThreeDsRaw {
    pub eci: String,
    pub cavv: Secret<String>,
    /// Masked alongside `cavv`: both are authentication artefacts that should never
    /// surface in a log line, even though the domain model keeps this one unmasked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds_trans_id: Option<Secret<String>>,
    /// Only meaningful for 3DS 1.0.0; UCS carries no xid, so this is always `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xid: Option<String>,
    /// Always sent explicitly so a 2.x authentication is not downgraded to the
    /// documented `1.0.0` default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PaydotcomAuthenticationContext {
    pub browser_details: PaydotcomBrowserDetails,
    pub customer_ip_address: Secret<String, common_utils::pii::IpAddress>,
}

#[derive(Debug, Serialize)]
pub struct PaydotcomBrowserDetails {
    pub java_enabled: bool,
    /// UCS spells this `java_script_enabled`; Pay.com spells it `javascript_enabled`.
    pub javascript_enabled: bool,
    pub user_agent: String,
    pub accept_header: String,
    pub language: String,
    pub color_depth: u8,
    pub screen_height: u32,
    pub screen_width: u32,
    /// UCS spells this `time_zone`.
    pub timezone_offset: i32,
}

#[derive(Debug, Serialize)]
pub struct PaydotcomBillingDetails {
    pub email: common_utils::pii::Email,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<PaydotcomAddress>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaydotcomAddress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line1: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postal_code: Option<Secret<String>>,
    /// ISO 3166-1 alpha-2.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<common_enums::CountryAlpha2>,
}

impl PaydotcomAddress {
    fn is_empty(&self) -> bool {
        self.line1.is_none()
            && self.line2.is_none()
            && self.city.is_none()
            && self.state.is_none()
            && self.postal_code.is_none()
            && self.country.is_none()
    }
}

/// Which 3DS shape the Authorize body carries.
///
/// * `None` — plain card.
/// * `ExternalMpi` — merchant-supplied `eci`/`cavv` replayed in `card.three_ds`.
/// * `GatewayAuthenticationContext` — Pay.com runs the authentication itself from the
///   browser data in `card.authentication_context`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaydotcomThreeDsMode {
    None,
    ExternalMpi,
    GatewayAuthenticationContext,
}

/// Decides the 3DS shape from the request alone, so `get_url` and the body can never
/// disagree.
pub fn three_ds_mode<T: PaymentMethodDataTypes>(
    request: &PaymentsAuthorizeData<T>,
    auth_type: common_enums::AuthenticationType,
) -> PaydotcomThreeDsMode {
    if request.authentication_data.is_some() {
        PaydotcomThreeDsMode::ExternalMpi
    } else if auth_type == common_enums::AuthenticationType::ThreeDs {
        PaydotcomThreeDsMode::GatewayAuthenticationContext
    } else {
        PaydotcomThreeDsMode::None
    }
}

/// `Manual` reserves funds on a Hold; everything else charges immediately.
/// `ManualMultiple` and `Scheduled` are rejected up front rather than being silently
/// folded into manual capture by `is_auto_capture()`.
pub fn is_manual_capture(
    capture_method: Option<common_enums::CaptureMethod>,
) -> Result<bool, error_stack::Report<IntegrationError>> {
    match capture_method {
        Some(common_enums::CaptureMethod::Manual) => Ok(true),
        Some(common_enums::CaptureMethod::Automatic)
        | Some(common_enums::CaptureMethod::SequentialAutomatic)
        | None => Ok(false),
        Some(other) => Err(error_stack::report!(IntegrationError::NotImplemented(
            format!("capture_method {other:?} for paydotcom"),
            IntegrationErrorContext {
                additional_context: Some(
                    "Pay.com exposes exactly one Charge (auto capture) and one Hold (manual \
                     capture) endpoint; ManualMultiple and Scheduled have no counterpart"
                        .to_string(),
                ),
                suggested_action: Some("Use capture_method AUTOMATIC or MANUAL".to_string(),),
                doc_url: None,
            },
        ))),
    }
}

fn build_three_ds_raw(
    authentication_data: &AuthenticationData,
) -> Result<PaydotcomThreeDsRaw, error_stack::Report<IntegrationError>> {
    let eci = authentication_data.eci.clone().ok_or_else(|| {
        error_stack::report!(IntegrationError::MissingRequiredField {
            field_name: "authentication_data.eci",
            context: Default::default(),
        })
    })?;
    let cavv = authentication_data.cavv.clone().ok_or_else(|| {
        error_stack::report!(IntegrationError::MissingRequiredField {
            field_name: "authentication_data.cavv",
            context: Default::default(),
        })
    })?;

    Ok(PaydotcomThreeDsRaw {
        eci,
        cavv,
        ds_trans_id: authentication_data.ds_trans_id.clone().map(Secret::new),
        // UCS has no xid; `transaction_id` is not one and must not be mapped here.
        xid: None,
        version: Some(
            authentication_data
                .message_version
                .as_ref()
                .map(|version| version.to_string())
                .unwrap_or_else(|| "2.2.0".to_string()),
        ),
    })
}

/// Every field under `authentication_context` is documented as mandatory, so a missing
/// one fails fast instead of producing a request Pay.com will reject.
fn build_authentication_context(
    browser_info: Option<&BrowserInformation>,
) -> Result<PaydotcomAuthenticationContext, error_stack::Report<IntegrationError>> {
    let browser_info = browser_info.ok_or_else(|| {
        error_stack::report!(IntegrationError::MissingRequiredField {
            field_name: "browser_info",
            context: Default::default(),
        })
    })?;

    let missing = |field: &'static str| {
        error_stack::report!(IntegrationError::MissingRequiredField {
            field_name: field,
            context: Default::default(),
        })
    };

    Ok(PaydotcomAuthenticationContext {
        browser_details: PaydotcomBrowserDetails {
            java_enabled: browser_info
                .java_enabled
                .ok_or_else(|| missing("browser_info.java_enabled"))?,
            javascript_enabled: browser_info
                .java_script_enabled
                .ok_or_else(|| missing("browser_info.java_script_enabled"))?,
            user_agent: browser_info
                .user_agent
                .clone()
                .ok_or_else(|| missing("browser_info.user_agent"))?,
            accept_header: browser_info
                .accept_header
                .clone()
                .ok_or_else(|| missing("browser_info.accept_header"))?,
            language: browser_info
                .language
                .clone()
                .ok_or_else(|| missing("browser_info.language"))?,
            color_depth: browser_info
                .color_depth
                .ok_or_else(|| missing("browser_info.color_depth"))?,
            screen_height: browser_info
                .screen_height
                .ok_or_else(|| missing("browser_info.screen_height"))?,
            screen_width: browser_info
                .screen_width
                .ok_or_else(|| missing("browser_info.screen_width"))?,
            timezone_offset: browser_info
                .time_zone
                .ok_or_else(|| missing("browser_info.time_zone"))?,
        },
        customer_ip_address: Secret::new(
            browser_info
                .ip_address
                .ok_or_else(|| missing("browser_info.ip_address"))?
                .to_string(),
        ),
    })
}

/// Metadata key under which the `chrg_…` / `hld_…` id is mirrored on
/// `connector_feature_data`, for callers that drive the gRPC flows directly instead of
/// letting an orchestrator carry `authentication_data` between them.
pub const PAYDOTCOM_RESOURCE_METADATA_KEY: &str = "paydotcom_resource";

/// Reads a mirrored resource id back out of `connector_feature_data`.
///
/// This is a *fallback* channel. The primary one is `authentication_data.transaction_id`
/// (see `resource_id_authentication_data`), which is what an orchestrator such as
/// Hyperswitch already carries from PreAuthenticate into Authenticate, and from
/// Authenticate into the settling Authorize, with no connector-specific plumbing.
pub fn pending_resource_id(
    connector_feature_data: Option<&common_utils::pii::SecretSerdeValue>,
) -> Option<String> {
    connector_feature_data.and_then(|metadata| {
        metadata
            .peek()
            .get(PAYDOTCOM_RESOURCE_METADATA_KEY)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_string)
    })
}

/// Reads the pending `chrg_`/`hld_` id out of `authentication_data.transaction_id`.
///
/// Field 8 of the wire `AuthenticationData` message is documented as "transaction
/// identifier generated by the 3DS system", which is exactly what this id is on Pay.com:
/// the resource the linked authentication session authenticates. Every other member of
/// that message stays empty — Pay.com performs the authentication itself and hands the
/// merchant no CAVV/ECI.
pub fn pending_resource_id_from_authentication_data(
    authentication_data: Option<&AuthenticationData>,
) -> Option<String> {
    authentication_data
        .and_then(|data| data.transaction_id.as_deref())
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
}

/// Builds the `AuthenticationData` that carries the pending resource id to the next leg.
///
/// Only `transaction_id` is populated, deliberately: an `eci`/`cavv` here would be
/// indistinguishable from a merchant-supplied external-MPI authentication (see
/// `three_ds_mode`), and Pay.com does not return either on this journey anyway.
pub fn resource_id_authentication_data(resource_id: &str) -> AuthenticationData {
    AuthenticationData {
        trans_status: None,
        eci: None,
        cavv: None,
        ucaf_collection_indicator: None,
        threeds_server_transaction_id: None,
        message_version: None,
        ds_trans_id: None,
        acs_transaction_id: None,
        transaction_id: Some(resource_id.to_string()),
        network_params: None,
        exemption_indicator: None,
        created_at: None,
        challenge_code: None,
        challenge_cancel: None,
        challenge_code_reason: None,
        message_extension: None,
        authentication_type: None,
    }
}

/// The pending resource id for an Authorize execution, from either channel.
pub fn authorize_pending_resource_id<T: PaymentMethodDataTypes>(
    request: &PaymentsAuthorizeData<T>,
) -> Option<String> {
    pending_resource_id_from_authentication_data(request.authentication_data.as_ref())
        .or_else(|| pending_resource_id(request.connector_feature_data.as_ref()))
}

/// Which of Pay.com's calls this Authorize execution is.
///
/// Gateway-driven 3DS needs three HTTP calls and `ConnectorIntegrationV2` issues exactly
/// one per flow execution, so the journey is split across three flows —
/// PreAuthenticate → Authenticate → Authorize:
///
/// | Leg | Flow | Call | Trigger |
/// |---|---|---|---|
/// | 1 | `PreAuthenticate` | `POST /v1/charges\|/v1/holds` with `authentication_context` | 3DS card |
/// | 2 | `Authenticate` | `POST /v1/sessions/authentication/linked` | always, for this connector |
/// | 3 | `Authorize` (`Confirm`) | `POST /v1/{charges\|holds}/{id}/confirm` | a pending resource id is present |
/// | — | `Authorize` (`Create`) | `POST /v1/charges\|/v1/holds` | everything else (no-3DS, external-MPI) |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaydotcomAuthorizeLeg {
    Create,
    Confirm,
}

/// Decides the leg from the **request alone**, never from a previous response, so
/// `get_url` and `get_request_body` can never disagree about which call is being made.
///
/// An external-MPI Authorize also carries `authentication_data`, but with `eci`/`cavv`
/// and no `transaction_id` — so it falls through to `Create`, as it must.
pub fn authorize_leg<T: PaymentMethodDataTypes>(
    request: &PaymentsAuthorizeData<T>,
) -> PaydotcomAuthorizeLeg {
    if authorize_pending_resource_id(request).is_some() {
        PaydotcomAuthorizeLeg::Confirm
    } else {
        PaydotcomAuthorizeLeg::Create
    }
}

/// Builds the shared Charge/Hold creation body used by both Authorize's `Create` leg and
/// PreAuthenticate.
#[allow(clippy::too_many_arguments)]
fn build_create_resource_request<T: PaymentMethodDataTypes>(
    card: &Card<T>,
    amount: MinorUnit,
    currency: common_enums::Currency,
    common: &PaymentFlowData,
    request_email: Option<common_utils::pii::Email>,
    customer_reference_id: Option<String>,
    three_ds: Option<PaydotcomThreeDsRaw>,
    authentication_context: Option<PaydotcomAuthenticationContext>,
    request_threed_secure: PaydotcomThreeDsRequest,
) -> Result<PaydotcomCreateResourceRequest<T>, error_stack::Report<IntegrationError>> {
    let billing_address = PaydotcomAddress {
        line1: common.get_optional_billing_line1(),
        line2: common.get_optional_billing_line2(),
        city: common.get_optional_billing_city(),
        state: common.get_optional_billing_state(),
        postal_code: common.get_optional_billing_zip(),
        country: common.get_optional_billing_country(),
    };
    let billing_address = (!billing_address.is_empty()).then_some(billing_address);

    // `email` is the one mandatory member of `billing_details`, so the object is only
    // emitted when an email is available.
    let billing_details = common
        .get_optional_billing_email()
        .or(request_email)
        .map(|email| PaydotcomBillingDetails {
            email,
            name: common.get_optional_billing_full_name(),
            phone: common.get_optional_billing_phone_number(),
            address: billing_address.clone(),
        });

    Ok(PaydotcomCreateResourceRequest {
        amount,
        currency,
        reference: Some(common.connector_request_reference_id.clone()),
        customer_reference_id,
        payment_method_options: Some(PaydotcomPaymentMethodOptions {
            card: PaydotcomCardOptions {
                request_threed_secure,
            },
        }),
        source_data: PaydotcomSourceData {
            source_type: PaydotcomSourceType::Card,
            card: PaydotcomCardSourceDetails {
                number: card.card_number.clone(),
                expiry_month: card.get_card_expiry_month_2_digit()?,
                expiry_year: card.get_expiry_year_4_digit(),
                cvc: card.card_cvc.clone(),
                name: card.card_holder_name.clone().ok_or_else(|| {
                    error_stack::report!(IntegrationError::MissingRequiredField {
                        field_name: "payment_method_data.card.card_holder_name",
                        context: Default::default(),
                    })
                })?,
                billing_address,
                three_ds,
                authentication_context,
            },
            billing_details,
        },
    })
}

fn only_cards_error() -> error_stack::Report<IntegrationError> {
    error_stack::report!(IntegrationError::NotImplemented(
        "Only card payments are supported by paydotcom".to_string(),
        IntegrationErrorContext {
            additional_context: Some(
                "Pay.com is integrated for cards only in this scope; wallets, bank debits and \
                 BNPL are out of scope"
                    .to_string(),
            ),
            suggested_action: None,
            doc_url: None,
        },
    ))
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaydotcomRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PaydotcomAuthorizeRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: PaydotcomRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &wrapper.router_data;

        match authorize_leg(&item.request) {
            // Leg 3 — the shopper is back from the challenge; settle synchronously.
            PaydotcomAuthorizeLeg::Confirm => Ok(Self::Confirm(PaydotcomConfirmRequest {})),

            // No 3DS, or external-MPI 3DS: a single create call settles it.
            PaydotcomAuthorizeLeg::Create => {
                // Rejects ManualMultiple / Scheduled before any wire work happens.
                is_manual_capture(item.request.capture_method)?;

                let card = match &item.request.payment_method_data {
                    PaymentMethodData::Card(card) => card,
                    _ => return Err(only_cards_error()),
                };

                let amount = wrapper
                    .connector
                    .amount_converter
                    .convert(item.request.minor_amount, item.request.currency)
                    .change_context(amount_conversion_error(
                        "Failed to convert authorize amount to MinorUnit for the Pay.com \
                         POST /v1/charges|/v1/holds request",
                    ))?;

                let (three_ds, authentication_context, request_threed_secure) =
                    match three_ds_mode(&item.request, item.resource_common_data.auth_type) {
                        PaydotcomThreeDsMode::None => (None, None, PaydotcomThreeDsRequest::None),
                        PaydotcomThreeDsMode::ExternalMpi => {
                            let authentication_data =
                                item.request.authentication_data.as_ref().ok_or_else(|| {
                                    error_stack::report!(IntegrationError::MissingRequiredField {
                                        field_name: "authentication_data",
                                        context: Default::default(),
                                    })
                                })?;
                            (
                                Some(build_three_ds_raw(authentication_data)?),
                                None,
                                // The authentication already happened off-platform; asking
                                // Pay.com to run another would double-authenticate the shopper.
                                PaydotcomThreeDsRequest::None,
                            )
                        }
                        PaydotcomThreeDsMode::GatewayAuthenticationContext => (
                            None,
                            Some(build_authentication_context(
                                item.request.browser_info.as_ref(),
                            )?),
                            // A 3DS Authorize that was not opened by PreAuthenticate can only
                            // finish here if the authentication turns out frictionless, so ask
                            // for `automatic` rather than forcing a challenge nobody can answer.
                            // The challenge journey runs PreAuthenticate first (see paydotcom.rs).
                            PaydotcomThreeDsRequest::Automatic,
                        ),
                    };

                Ok(Self::Create(Box::new(build_create_resource_request(
                    card,
                    amount,
                    item.request.currency,
                    &item.resource_common_data,
                    item.request.email.clone(),
                    item.request
                        .customer_id
                        .as_ref()
                        .map(|customer_id| customer_id.get_string_repr().to_string()),
                    three_ds,
                    authentication_context,
                    request_threed_secure,
                )?)))
            }
        }
    }
}

// ===== REQUEST: PRE-AUTHENTICATE (gateway 3DS, leg 1) =====

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaydotcomRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PaydotcomPreAuthenticateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: PaydotcomRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &wrapper.router_data;

        is_manual_capture(item.request.capture_method)?;

        let card = match &item.request.payment_method_data {
            Some(PaymentMethodData::Card(card)) => card,
            _ => return Err(only_cards_error()),
        };

        let currency = item.request.currency.ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "currency",
                context: Default::default(),
            })
        })?;

        let amount = wrapper
            .connector
            .amount_converter
            .convert(item.request.amount, currency)
            .change_context(amount_conversion_error(
                "Failed to convert pre-authenticate amount to MinorUnit for the Pay.com \
                 POST /v1/charges|/v1/holds request",
            ))?;

        build_create_resource_request(
            card,
            amount,
            currency,
            &item.resource_common_data,
            item.request.email.clone(),
            None,
            None,
            Some(build_authentication_context(
                item.request.browser_info.as_ref(),
            )?),
            // `challenge`, deliberately: it is what makes the leg deterministic. With
            // `automatic` a frictionless authentication would come back `succeeded`, and the
            // following linked-session call would target a resource that no longer requires
            // authentication.
            PaydotcomThreeDsRequest::Challenge,
        )
    }
}

// ===== REQUEST: AUTHENTICATE (gateway 3DS, leg 2) =====

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaydotcomRouterData<
            RouterDataV2<
                Authenticate,
                PaymentFlowData,
                PaymentsAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PaydotcomAuthenticateRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: PaydotcomRouterData<
            RouterDataV2<
                Authenticate,
                PaymentFlowData,
                PaymentsAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &wrapper.router_data;

        let resource =
            pending_resource_id_from_authentication_data(item.request.authentication_data.as_ref())
                .or_else(|| {
                    pending_resource_id(item.resource_common_data.connector_feature_data.as_ref())
                })
                .ok_or_else(|| {
                    error_stack::report!(IntegrationError::MissingRequiredField {
                        field_name: "authentication_data.transaction_id",
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "Pay.com's linked authentication session authenticates a Charge \
                                 or Hold that PreAuthenticate must have created first; its id \
                                 arrives on the PreAuthenticate response's authentication_data."
                                    .to_string(),
                            ),
                            ..Default::default()
                        },
                    })
                })?;

        // Where the shopper lands after the challenge. This MUST be the
        // continue-redirection ("complete authorize") URL, not the plain return URL:
        // landing on the plain one makes the orchestrator treat the return as a sync, and
        // leg 3 (`/confirm`) never runs — the resource is then left parked on
        // `requires_authentication` forever. `router_return_url` is only a fallback for
        // callers that drive the flows themselves and set nothing else.
        let return_url = item
            .request
            .continue_redirection_url
            .as_ref()
            .map(ToString::to_string)
            .or_else(|| {
                item.request
                    .router_return_url
                    .as_ref()
                    .map(ToString::to_string)
            })
            .or_else(|| item.resource_common_data.return_url.clone())
            .ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                    field_name: "continue_redirection_url",
                    context: Default::default(),
                })
            })?;

        Ok(Self {
            resource,
            return_url,
            confirm: false,
        })
    }
}

// ===== REQUEST: CAPTURE =====

/// `POST /v1/holds/{id}/capture`. Every field is optional — an empty body captures the
/// full hold.
#[derive(Debug, Serialize)]
pub struct PaydotcomCaptureRequest {
    /// Minor units as a **JSON string** (see the module docs).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount_to_capture: Option<StringMinorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaydotcomRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for PaydotcomCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: PaydotcomRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &wrapper.router_data;
        let amount_to_capture = wrapper
            .connector
            .string_amount_converter
            .convert(item.request.minor_amount_to_capture, item.request.currency)
            .change_context(amount_conversion_error(
                "Failed to convert capture amount to StringMinorUnit for the Pay.com \
                 POST /v1/holds/{id}/capture request",
            ))?;

        Ok(Self {
            amount_to_capture: Some(amount_to_capture),
            reference: Some(
                item.resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
        })
    }
}

// ===== REQUEST: REFUND =====

/// `POST /v1/refunds`. `charge` is the only required member and must be a `chrg_` id.
#[derive(Debug, Serialize)]
pub struct PaydotcomRefundRequest {
    pub charge: String,
    /// Minor units as a **JSON string** (see the module docs).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount_to_refund: Option<StringMinorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaydotcomRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for PaydotcomRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: PaydotcomRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &wrapper.router_data;
        let charge = item.request.connector_transaction_id.clone();

        // A Hold holds no money yet, so Pay.com has nothing to refund; the caller must
        // capture (which mints a `chrg_` id) or cancel instead.
        if charge.starts_with(HOLD_ID_PREFIX) {
            return Err(error_stack::report!(IntegrationError::NotImplemented(
                "Refunding an uncaptured Pay.com Hold".to_string(),
                IntegrationErrorContext {
                    additional_context: Some(format!(
                        "POST /v1/refunds requires a captured Charge id (^chrg_); got `{charge}`"
                    )),
                    suggested_action: Some(
                        "Capture the hold first (the Capture response carries the new chrg_ id) \
                         or cancel it with a Void"
                            .to_string(),
                    ),
                    doc_url: None,
                },
            )));
        }

        let amount_to_refund = wrapper
            .connector
            .string_amount_converter
            .convert(item.request.minor_refund_amount, item.request.currency)
            .change_context(amount_conversion_error(
                "Failed to convert refund amount to StringMinorUnit for the Pay.com \
                 POST /v1/refunds request",
            ))?;

        Ok(Self {
            charge,
            amount_to_refund: Some(amount_to_refund),
            reference: Some(item.request.refund_id.clone()),
        })
    }
}

// ===== STATUS ENUMS =====

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaydotcomChargeStatus {
    Succeeded,
    Failed,
    Pending,
    RequiresAuthentication,
    RequiresConfirmation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaydotcomHoldStatus {
    RequiresCapture,
    Succeeded,
    Failed,
    RequiresAuthentication,
    RequiresConfirmation,
    /// Absent from the OpenAPI enum but returned by `POST /v1/holds/{id}/cancel`.
    Canceled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaydotcomRefundStatus {
    Succeeded,
    Failed,
    Pending,
}

impl From<PaydotcomChargeStatus> for AttemptStatus {
    fn from(status: PaydotcomChargeStatus) -> Self {
        match status {
            // A Charge *is* a captured payment; `paid` describes settlement, not capture,
            // so it must not gate this mapping.
            PaydotcomChargeStatus::Succeeded => Self::Charged,
            PaydotcomChargeStatus::Pending => Self::Pending,
            PaydotcomChargeStatus::RequiresAuthentication
            | PaydotcomChargeStatus::RequiresConfirmation => Self::AuthenticationPending,
            PaydotcomChargeStatus::Failed => Self::Failure,
        }
    }
}

impl From<PaydotcomHoldStatus> for AttemptStatus {
    fn from(status: PaydotcomHoldStatus) -> Self {
        match status {
            PaydotcomHoldStatus::RequiresCapture => Self::Authorized,
            PaydotcomHoldStatus::Succeeded => Self::Charged,
            PaydotcomHoldStatus::Canceled => Self::Voided,
            PaydotcomHoldStatus::RequiresAuthentication
            | PaydotcomHoldStatus::RequiresConfirmation => Self::AuthenticationPending,
            PaydotcomHoldStatus::Failed => Self::Failure,
        }
    }
}

impl From<PaydotcomRefundStatus> for RefundStatus {
    fn from(status: PaydotcomRefundStatus) -> Self {
        match status {
            PaydotcomRefundStatus::Succeeded => Self::Success,
            PaydotcomRefundStatus::Pending => Self::Pending,
            PaydotcomRefundStatus::Failed => Self::Failure,
        }
    }
}

// ===== RESPONSE: CHARGE / HOLD =====

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PaydotcomChargeResponse {
    pub id: String,
    pub status: PaydotcomChargeStatus,
    #[serde(default)]
    pub amount: Option<MinorUnit>,
    #[serde(default)]
    pub amount_refunded: Option<MinorUnit>,
    #[serde(
        default,
        serialize_with = "paydotcom_currency::option::serialize",
        deserialize_with = "paydotcom_currency::option::deserialize"
    )]
    pub currency: Option<common_enums::Currency>,
    #[serde(default)]
    pub reference: Option<String>,
    /// Set when the Charge was produced by capturing a Hold.
    #[serde(default)]
    pub hold: Option<String>,
    #[serde(default)]
    pub failure_code: Option<String>,
    #[serde(default)]
    pub failure_message: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PaydotcomHoldResponse {
    pub id: String,
    pub status: PaydotcomHoldStatus,
    #[serde(default)]
    pub amount: Option<MinorUnit>,
    #[serde(default)]
    pub amount_capturable: Option<MinorUnit>,
    #[serde(
        default,
        serialize_with = "paydotcom_currency::option::serialize",
        deserialize_with = "paydotcom_currency::option::deserialize"
    )]
    pub currency: Option<common_enums::Currency>,
    #[serde(default)]
    pub reference: Option<String>,
    #[serde(default)]
    pub canceled: Option<bool>,
    #[serde(default)]
    pub failure_code: Option<String>,
    #[serde(default)]
    pub failure_message: Option<String>,
}

/// `POST /v1/sessions/authentication/linked` — carries the challenge URL the shopper is
/// redirected to.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PaydotcomAuthenticationSessionResponse {
    pub id: String,
    pub status: PaydotcomAuthenticationSessionStatus,
    /// The challenge page, e.g. `https://sca.pay.com/authenticate?client_secret=<jwt>`.
    #[serde(default)]
    pub url: Option<String>,
    /// Set when the session authenticates a Charge.
    #[serde(default)]
    pub charge: Option<String>,
    /// Set when the session authenticates a Hold.
    #[serde(default)]
    pub hold: Option<String>,
    #[serde(default)]
    pub return_url: Option<String>,
    // `client_secret` is a Pay Components (browser SDK) credential and is out of scope;
    // it is deliberately not deserialized so it can never reach a log.
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaydotcomAuthenticationSessionStatus {
    Open,
    Authenticated,
    Failed,
}

/// All three objects carry a `resource` discriminator; an internally tagged enum is used
/// rather than `untagged` because Charge and Hold overlap heavily and `untagged` would
/// silently pick the wrong arm.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "resource", rename_all = "snake_case")]
pub enum PaydotcomPaymentsResponse {
    Charge(Box<PaydotcomChargeResponse>),
    Hold(Box<PaydotcomHoldResponse>),
    AuthenticationSession(Box<PaydotcomAuthenticationSessionResponse>),
}

impl PaydotcomPaymentsResponse {
    /// The id this response is *about*. For an authentication session that is the linked
    /// Charge/Hold, never the `sca_…` id — the attempt's connector transaction id must not
    /// change across the challenge leg.
    pub fn id(&self) -> &str {
        match self {
            Self::Charge(charge) => &charge.id,
            Self::Hold(hold) => &hold.id,
            Self::AuthenticationSession(session) => session
                .charge
                .as_deref()
                .or(session.hold.as_deref())
                .unwrap_or(&session.id),
        }
    }

    /// The challenge URL, when this response is an authentication session that opened one.
    pub fn challenge_url(&self) -> Option<&str> {
        match self {
            Self::AuthenticationSession(session) => session.url.as_deref(),
            _ => None,
        }
    }

    pub fn attempt_status(&self) -> AttemptStatus {
        match self {
            Self::Charge(charge) => AttemptStatus::from(charge.status),
            Self::Hold(hold) => {
                // `cancel` answers with `canceled: true`; trust the boolean over any
                // status value the enum did not expect.
                if hold.canceled.unwrap_or(false) {
                    AttemptStatus::Voided
                } else {
                    AttemptStatus::from(hold.status)
                }
            }
            // The session exists only to hand the shopper a challenge page.
            Self::AuthenticationSession(session) => match session.status {
                PaydotcomAuthenticationSessionStatus::Failed => AttemptStatus::AuthenticationFailed,
                PaydotcomAuthenticationSessionStatus::Open
                | PaydotcomAuthenticationSessionStatus::Authenticated => {
                    AttemptStatus::AuthenticationPending
                }
            },
        }
    }

    pub fn reference(&self) -> Option<String> {
        match self {
            Self::Charge(charge) => charge.reference.clone(),
            Self::Hold(hold) => hold.reference.clone(),
            Self::AuthenticationSession(_) => None,
        }
    }

    pub fn amount(&self) -> Option<MinorUnit> {
        match self {
            Self::Charge(charge) => charge.amount,
            Self::Hold(hold) => hold.amount,
            Self::AuthenticationSession(_) => None,
        }
    }

    /// True while the resource is parked waiting for the shopper to authenticate — the
    /// point at which the caller must persist the resource id and drive the next leg.
    pub fn awaits_authentication(&self) -> bool {
        matches!(
            self,
            Self::Charge(charge)
                if matches!(charge.status, PaydotcomChargeStatus::RequiresAuthentication)
        ) || matches!(
            self,
            Self::Hold(hold)
                if matches!(hold.status, PaydotcomHoldStatus::RequiresAuthentication)
        )
    }

    /// Republished on every leg so the caller keeps handing the resource id back until the
    /// journey ends. See `PAYDOTCOM_RESOURCE_METADATA_KEY`.
    pub fn pending_metadata(&self) -> Option<serde_json::Value> {
        (self.awaits_authentication() || matches!(self, Self::AuthenticationSession(_)))
            .then(|| serde_json::json!({ PAYDOTCOM_RESOURCE_METADATA_KEY: self.id() }))
    }

    fn failure(&self) -> (Option<String>, Option<String>) {
        match self {
            Self::Charge(charge) => (charge.failure_code.clone(), charge.failure_message.clone()),
            Self::Hold(hold) => (hold.failure_code.clone(), hold.failure_message.clone()),
            Self::AuthenticationSession(_) => (None, None),
        }
    }

    /// A 2xx carrying `status: "failed"` is the normal decline path once the transaction
    /// has reached the network, so it is turned into an `ErrorResponse` here rather than
    /// being left to `build_error_response`.
    fn in_band_error(&self, http_code: u16) -> ErrorResponse {
        let (failure_code, failure_message) = self.failure();
        ErrorResponse {
            status_code: http_code,
            code: failure_code
                .clone()
                .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            message: failure_message
                .clone()
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            reason: failure_message,
            attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
            connector_transaction_id: Some(self.id().to_string()),
            network_decline_code: failure_code,
            network_advice_code: None,
            network_error_message: None,
            typed_connector_response: None,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        }
    }

    fn transaction_response(&self, http_code: u16) -> PaymentsResponseData {
        PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(self.id().to_string()),
            // The shopper is sent to the challenge page with a plain GET.
            redirection_data: self.challenge_url().map(|url| {
                Box::new(RedirectForm::Form {
                    endpoint: url.to_string(),
                    method: Method::Get,
                    form_fields: std::collections::HashMap::new(),
                })
            }),
            mandate_reference: None,
            // While the gateway-3DS journey is unfinished the resource id has to be
            // republished HERE, not only on `PaymentFlowData::connector_feature_data`:
            // the Authorize response maps `PaymentsResponseData::connector_metadata` (and
            // only that) onto the gRPC `connector_feature_data`, so anything left on the
            // flow data is dropped and the caller cannot drive the `/confirm` leg.
            connector_metadata: self.pending_metadata().or_else(|| match self {
                Self::Charge(charge) => charge
                    .hold
                    .as_ref()
                    .map(|hold| serde_json::json!({ "hold": hold })),
                Self::Hold(_) | Self::AuthenticationSession(_) => None,
            }),
            network_txn_id: None,
            network_txn_link_id: None,
            connector_response_reference_id: self.reference(),
            incremental_authorization_allowed: None,
            status_code: http_code,
            splits: None,
            payment_account_reference: None,
        }
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<PaydotcomPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaydotcomPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = item.response.attempt_status();
        // Keep republishing the resource id while the journey is unfinished, so the caller
        // can hand it back on the next leg (linked session, then confirm).
        let connector_feature_data = item.response.pending_metadata().map(Secret::new);
        let response = match status {
            AttemptStatus::Failure => Err(item.response.in_band_error(item.http_code)),
            _ => Ok(item.response.transaction_response(item.http_code)),
        };

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                connector_feature_data,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== RESPONSE: PRE-AUTHENTICATE (gateway 3DS, leg 1) =====

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<PaydotcomPaymentsResponse, Self>>
    for RouterDataV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaydotcomPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = item.response.attempt_status();
        let resource_id = item.response.id().to_string();

        // `request_threed_secure: "challenge"` forces `requires_authentication`, so anything
        // else means the leg cannot be continued and the caller must not be handed a
        // resource id it would then try to authenticate.
        let pending = item.response.pending_metadata();
        // `pending` is `Some(resource_id)` only while Pay.com is waiting for the shopper to
        // authenticate — that is, when this Charge/Hold still needs the linked-authentication
        // session that the following Authenticate leg creates. When it is `None` the journey
        // is already resolved (approved or failed) and there is no id to hand forward, so
        // both carriers below stay empty and the orchestrator stops after this leg.
        //
        // The id travels on `authentication_data`, the channel an orchestrator already moves
        // from a PreAuthenticate response into the next Authenticate request, so no
        // connector-specific metadata plumbing is needed. `connector_feature_data` carries the
        // same id as a fallback for callers that drive the gRPC flows directly and therefore
        // have no orchestrator doing that for them.
        let connector_feature_data = pending.as_ref().cloned().map(Secret::new);
        let authentication_data = pending
            .as_ref()
            .map(|_| resource_id_authentication_data(&resource_id));

        let response = match status {
            AttemptStatus::Failure => Err(item.response.in_band_error(item.http_code)),
            _ => Ok(PaymentsResponseData::PreAuthenticateResponse {
                resource_id: Some(ResponseId::ConnectorTransactionId(resource_id)),
                // The challenge URL does not exist yet — it is minted by the linked-session
                // call the following Authenticate leg makes.
                redirection_data: None,
                authentication_data,
                connector_response_reference_id: item.response.reference(),
                status_code: item.http_code,
            }),
        };

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                connector_feature_data,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== RESPONSE: AUTHENTICATE (gateway 3DS, leg 2) =====

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<PaydotcomPaymentsResponse, Self>>
    for RouterDataV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaydotcomPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = item.response.attempt_status();
        let resource_id = item.response.id().to_string();

        // Republished so the settling Authorize knows what to `/confirm`. The orchestrator
        // persists this and hands it back on CompleteAuthorize; `connector_feature_data`
        // mirrors it for callers driving the gRPC flows directly.
        let pending = item.response.pending_metadata();
        let authentication_data = pending
            .clone()
            .is_some()
            .then(|| resource_id_authentication_data(&resource_id));

        let response = match status {
            AttemptStatus::Failure | AttemptStatus::AuthenticationFailed => {
                Err(item.response.in_band_error(item.http_code))
            }
            _ => Ok(PaymentsResponseData::AuthenticateResponse {
                resource_id: Some(ResponseId::ConnectorTransactionId(resource_id)),
                // The challenge page; the shopper is sent there with a plain GET.
                redirection_data: item.response.challenge_url().map(|url| {
                    Box::new(RedirectForm::Form {
                        endpoint: url.to_string(),
                        method: Method::Get,
                        form_fields: std::collections::HashMap::new(),
                    })
                }),
                authentication_data,
                connector_feature_data: pending,
                connector_response_reference_id: item.response.reference(),
                status_code: item.http_code,
            }),
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

impl TryFrom<ResponseRouterData<PaydotcomPaymentsResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaydotcomPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = item.response.attempt_status();
        let response = match status {
            AttemptStatus::Failure => Err(item.response.in_band_error(item.http_code)),
            _ => Ok(item.response.transaction_response(item.http_code)),
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

impl TryFrom<ResponseRouterData<PaydotcomPaymentsResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaydotcomPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let captured_amount = item.response.amount();
        let authorized_amount = item
            .router_data
            .resource_common_data
            .amount
            .as_ref()
            .map(|money| money.amount);

        let status = match item.response.attempt_status() {
            // A capture smaller than the amount originally held leaves the payment
            // partially charged, not fully charged.
            AttemptStatus::Charged => match (captured_amount, authorized_amount) {
                (Some(captured), Some(authorized)) if captured < authorized => {
                    AttemptStatus::PartialCharged
                }
                _ => AttemptStatus::Charged,
            },
            AttemptStatus::Pending => AttemptStatus::CaptureInitiated,
            AttemptStatus::Failure => AttemptStatus::CaptureFailed,
            other => other,
        };

        let response = match status {
            AttemptStatus::CaptureFailed => Err(item.response.in_band_error(item.http_code)),
            // `resource_id` is the **new** `chrg_` id minted by the capture. Rewriting the
            // attempt's connector_transaction_id here is what makes a later refund work,
            // because `POST /v1/refunds` only accepts `^chrg_` ids.
            _ => Ok(item.response.transaction_response(item.http_code)),
        };

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                amount_captured: captured_amount.map(|amount| amount.get_amount_as_i64()),
                minor_amount_captured: captured_amount,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl TryFrom<ResponseRouterData<PaydotcomPaymentsResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaydotcomPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = item.response.attempt_status();
        let response = match status {
            AttemptStatus::Failure => Err(item.response.in_band_error(item.http_code)),
            _ => Ok(item.response.transaction_response(item.http_code)),
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

// ===== RESPONSE: REFUND =====

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PaydotcomRefundResponse {
    pub id: String,
    pub status: PaydotcomRefundStatus,
    #[serde(default)]
    pub amount: Option<MinorUnit>,
    #[serde(
        default,
        serialize_with = "paydotcom_currency::option::serialize",
        deserialize_with = "paydotcom_currency::option::deserialize"
    )]
    pub currency: Option<common_enums::Currency>,
    #[serde(default)]
    pub charge: Option<String>,
    #[serde(default)]
    pub reference: Option<String>,
    #[serde(default)]
    pub failure_code: Option<String>,
    #[serde(default)]
    pub failure_message: Option<String>,
}

impl TryFrom<ResponseRouterData<PaydotcomRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaydotcomRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id.clone(),
                refund_status: RefundStatus::from(item.response.status),
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            ..item.router_data
        })
    }
}

impl TryFrom<ResponseRouterData<PaydotcomRefundResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaydotcomRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id.clone(),
                refund_status: RefundStatus::from(item.response.status),
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            ..item.router_data
        })
    }
}

// ===== ERRORS =====

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PaydotcomErrorResponse {
    pub error: PaydotcomError,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PaydotcomError {
    /// `api_error` | `payment_method_error` | `idempotency_error` | `invalid_request_error`.
    #[serde(rename = "type")]
    pub error_type: String,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub decline_code: Option<String>,
    /// Present on `invalid_request_error`.
    #[serde(default)]
    pub params: Option<Vec<PaydotcomErrorParam>>,
    /// A declined Charge still has an id — propagating it lets PSync find the attempt.
    #[serde(default)]
    pub charge: Option<String>,
    #[serde(default)]
    pub hold: Option<String>,
    #[serde(default)]
    pub payment_method: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PaydotcomErrorParam {
    pub key: String,
    pub error: String,
}

const ERROR_TYPE_PAYMENT_METHOD: &str = "payment_method_error";

impl PaydotcomErrorResponse {
    fn reason(&self) -> Option<String> {
        self.error.decline_code.clone().or_else(|| {
            self.error.params.as_ref().map(|params| {
                params
                    .iter()
                    .map(|param| format!("{}: {}", param.key, param.error))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
        })
    }

    pub fn to_error_response(&self, status_code: u16) -> ErrorResponse {
        ErrorResponse {
            status_code,
            code: self
                .error
                .code
                .clone()
                .unwrap_or_else(|| self.error.error_type.clone()),
            message: self
                .error
                .message
                .clone()
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            reason: self.reason(),
            // A validation or idempotency error is not a payment decline, so the attempt
            // status is left untouched for anything but `payment_method_error`.
            attempt_status: (self.error.error_type == ERROR_TYPE_PAYMENT_METHOD)
                .then_some(FlowStatus::Payment(AttemptStatus::Failure)),
            connector_transaction_id: self
                .error
                .charge
                .clone()
                .or_else(|| self.error.hold.clone()),
            network_decline_code: self.error.decline_code.clone(),
            network_advice_code: None,
            network_error_message: self.error.message.clone(),
            typed_connector_response: None,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        }
    }

    pub fn to_refund_error_response(&self, status_code: u16) -> ErrorResponse {
        let mut error_response = self.to_error_response(status_code);
        // `attempt_status` is a payment concept; a failed refund must not rewrite the
        // payment attempt.
        error_response.attempt_status = None;
        error_response
    }
}

/// PSync and Void both address a resource by the prefix of the stored
/// `connector_transaction_id`; this keeps the auto/manual capture split invisible to the
/// rest of UCS.
pub fn payment_resource_path(
    connector_transaction_id: &str,
) -> Result<String, error_stack::Report<IntegrationError>> {
    if connector_transaction_id.starts_with(HOLD_ID_PREFIX) {
        Ok(format!("/v1/holds/{connector_transaction_id}"))
    } else if connector_transaction_id.starts_with(CHARGE_ID_PREFIX) {
        Ok(format!("/v1/charges/{connector_transaction_id}"))
    } else {
        Err(error_stack::report!(
            IntegrationError::MissingConnectorTransactionID {
                context: IntegrationErrorContext {
                    additional_context: Some(format!(
                        "Pay.com resource ids are prefixed `chrg_` or `hld_`; got \
                         `{connector_transaction_id}`"
                    )),
                    suggested_action: Some(
                        "Sync the payment only after Authorize returned a connector \
                         transaction id"
                            .to_string(),
                    ),
                    doc_url: None,
                },
            }
        ))
    }
}

/// `cancel` exists only on Holds; Pay.com has no charge-void (that is a Refund).
pub fn hold_cancel_path(
    connector_transaction_id: &str,
) -> Result<String, error_stack::Report<IntegrationError>> {
    if connector_transaction_id.starts_with(HOLD_ID_PREFIX) {
        Ok(format!("/v1/holds/{connector_transaction_id}/cancel"))
    } else {
        Err(error_stack::report!(IntegrationError::NotImplemented(
            "Voiding a captured Pay.com Charge".to_string(),
            IntegrationErrorContext {
                additional_context: Some(format!(
                    "Only a Hold (^hld_) can be cancelled; got `{connector_transaction_id}`"
                )),
                suggested_action: Some("Refund the charge instead of voiding it".to_string()),
                doc_url: None,
            },
        )))
    }
}

/// `POST /v1/{charges|holds}/{id}/confirm` — the settle leg after a challenge.
pub fn confirm_path(
    connector_transaction_id: &str,
) -> Result<String, error_stack::Report<IntegrationError>> {
    Ok(format!(
        "{}/confirm",
        payment_resource_path(connector_transaction_id)?
    ))
}

/// `capture` likewise exists only on Holds.
pub fn hold_capture_path(
    connector_transaction_id: &str,
) -> Result<String, error_stack::Report<IntegrationError>> {
    if connector_transaction_id.starts_with(HOLD_ID_PREFIX) {
        Ok(format!("/v1/holds/{connector_transaction_id}/capture"))
    } else {
        Err(error_stack::report!(IntegrationError::NotImplemented(
            "Capturing a Pay.com resource that is not a Hold".to_string(),
            IntegrationErrorContext {
                additional_context: Some(format!(
                    "Only a Hold (^hld_) can be captured; got `{connector_transaction_id}`. An \
                     auto-capture Authorize already produced a Charge."
                )),
                suggested_action: None,
                doc_url: None,
            },
        )))
    }
}
