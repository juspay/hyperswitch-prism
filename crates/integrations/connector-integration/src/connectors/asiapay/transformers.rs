use std::{fmt::Debug, marker::PhantomData};

use common_enums::{AttemptStatus, AuthenticationType, CardNetwork, Currency, RefundStatus};
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    pii::{Email, IpAddress},
    types::{MinorUnit, Money, StringMajorUnit},
};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, Void},
    connector_types::{
        EventType, PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RawConnectorStatus, RefundFlowData, RefundSyncData,
        RefundsData, RefundsResponseData, ResponseId,
    },
    errors::{
        ConnectorError, IntegrationError, IntegrationErrorContext,
        ResponseTransformationErrorContext, WebhookError,
    },
    payment_address::AddressDetails,
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_request_types::{
        AuthoriseIntegrityObject, CaptureIntegrityObject, PaymentSynIntegrityObject,
        PaymentVoidIntegrityObject, RefundIntegrityObject, RefundSyncIntegrityObject,
    },
    router_response_types::RedirectForm,
    utils::CardIssuer,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use super::{Asiapay, AsiapayAmountConvertor, AsiapayRouterData};
use crate::types::ResponseRouterData;

// ============================================================================
// AUTHENTICATION
// ============================================================================

/// AsiaPay credentials, read out of the flat `ConnectorSpecificConfig::Asiapay`
/// variant. `merchant_id` signs/identifies every payment call; `login_id` +
/// `password` are for the Merchant API (Capture/Void/Refund/Query);
/// `secure_hash_secret` never leaves the server.
#[derive(Debug, Clone)]
pub struct AsiapayAuthType {
    pub merchant_id: Secret<String>,
    pub login_id: Secret<String>,
    pub password: Secret<String>,
    pub secure_hash_secret: Secret<String>,
    pub secure_hash_algorithm: AsiapaySecureHashAlgorithm,
}

/// Secure Hash algorithm for the merchant account (AsiaPay panel setting).
/// AsiaPay's gateway default is SHA-1; SHA-256 is a supported alternative.
/// The same construction is hashed in both cases (`a|b|c|...|secret`) — only
/// the digest function changes, per the vendor's Transaction Security doc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsiapaySecureHashAlgorithm {
    Sha1,
    Sha256,
}

impl AsiapaySecureHashAlgorithm {
    /// `None`/empty → the gateway default (`Sha1`). Accepts `sha1`/`sha-1`
    /// and `sha256`/`sha-256` case-insensitively; anything else is rejected
    /// (fail-closed: signing with the wrong algorithm silently breaks auth).
    pub fn from_config(value: Option<&str>) -> Result<Self, error_stack::Report<IntegrationError>> {
        match value.map(str::trim).filter(|value| !value.is_empty()) {
            None => Ok(Self::Sha1),
            Some(value)
                if value.eq_ignore_ascii_case("sha1") || value.eq_ignore_ascii_case("sha-1") =>
            {
                Ok(Self::Sha1)
            }
            Some(value)
                if value.eq_ignore_ascii_case("sha256")
                    || value.eq_ignore_ascii_case("sha-256") =>
            {
                Ok(Self::Sha256)
            }
            Some(other) => Err(IntegrationError::InvalidDataFormat {
                field_name: "secure_hash_algorithm",
                context: IntegrationErrorContext {
                    additional_context: Some(format!(
                        "asiapay: unsupported secure hash algorithm `{other}` \
                         (expected `sha1` or `sha256`)"
                    )),
                    ..Default::default()
                },
            }
            .into()),
        }
    }

    /// Lowercase hex digest of the signing string, per the vendor docs.
    pub fn digest(self, input: &str) -> String {
        use sha2::Digest;
        match self {
            Self::Sha1 => hex::encode(sha1::Sha1::digest(input.as_bytes())),
            Self::Sha256 => hex::encode(sha2::Sha256::digest(input.as_bytes())),
        }
    }
}

// Auth is read from `ConnectorSpecificConfig`, NOT from a `connector_auth_type`
// field on RouterDataV2 - that field was deleted from RouterDataV2.
impl TryFrom<&ConnectorSpecificConfig> for AsiapayAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Asiapay {
                merchant_id,
                login_id,
                password,
                secure_hash_secret,
                secure_hash_algorithm,
                ..
            } => Ok(Self {
                merchant_id: merchant_id.to_owned(),
                login_id: login_id.to_owned(),
                password: password.to_owned(),
                secure_hash_secret: secure_hash_secret.to_owned(),
                secure_hash_algorithm: AsiapaySecureHashAlgorithm::from_config(
                    secure_hash_algorithm.as_deref(),
                )?,
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "asiapay: expected ConnectorSpecificConfig::Asiapay".to_string(),
                        ),
                        suggested_action: Some(
                            "Provide merchant_id, login_id, password, and secure_hash_secret."
                                .to_string(),
                        ),
                        ..Default::default()
                    },
                }
            )),
        }
    }
}

// ===== ERROR RESPONSE =====
// AsiaPay's in-band error/result payload. Bodies are URL-encoded
// (`application/x-www-form-urlencoded`) with OPTIONAL keys: `errMsg`, `prc`,
// `src`, `successcode` / `resultCode` / `orderStatus`. Every field is `Option`
// on purpose: an error body that omits a key must still deserialize. The
// NO_ERROR_CODE / NO_ERROR_MESSAGE sentinels are applied at the call site in
// the connector's `build_error_response` - never `.unwrap_or_default()`,
// which turns a missing code into an empty string indistinguishable from a
// real empty code.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AsiapayErrorResponse {
    #[serde(rename = "errMsg")]
    pub err_msg: Option<String>,
    pub prc: Option<String>,
    pub src: Option<String>,
    pub successcode: Option<String>,
    #[serde(rename = "resultCode")]
    pub result_code: Option<String>,
    #[serde(rename = "orderStatus")]
    pub order_status: Option<String>,
}

// ============================================================================
// SECURE HASH (SHA-256)
// ============================================================================

/// Sign an AsiaPay DPC payment request.
///
/// Signing string (docs, "Transaction Security by Secure Hash"):
/// `merchantId|orderRef|currCode|amount|payType|<Secure Hash Secret>`, where
/// `amount` is the raw major-unit string exactly as sent on the wire.
///
/// The digest function comes from the `secure_hash_algorithm` credential
/// (`AsiapaySecureHashAlgorithm`): the merchant's Secure Hash setting in the
/// AsiaPay admin panel must match it. AsiaPay's docs default to **SHA-1** —
/// their worked example is `56100908|1280204670187|344|10|N|<secret>`
/// → `13068c0ef09139ea711d36bde16785a2d30b9a30` — and support SHA-256 as the
/// alternative. Output is lowercase hex in both cases.
pub fn sign_payment_request(
    merchant_id: &str,
    order_ref: &str,
    curr_code: &str,
    amount: &str,
    pay_type: &str,
    secure_hash_secret: &str,
    secure_hash_algorithm: AsiapaySecureHashAlgorithm,
) -> String {
    let signing_string =
        format!("{merchant_id}|{order_ref}|{curr_code}|{amount}|{pay_type}|{secure_hash_secret}");
    secure_hash_algorithm.digest(&signing_string)
}

// ============================================================================
// AUTHORIZE REQUEST (DPC, `payComp.jsp`, application/x-www-form-urlencoded)
// ============================================================================

/// AsiaPay `payType`: `N` = normal sale (automatic capture),
/// `H` = authorize-hold (manual capture, settled later via Capture).
#[derive(Debug, Clone, Copy, Serialize)]
pub enum AsiapayPayType {
    N,
    H,
}

impl AsiapayPayType {
    /// `capture_method == Automatic` → `N`; anything else → `H` (per the tech
    /// spec's variant matrix; unspecified capture methods map to hold).
    pub fn from_capture_method(method: Option<common_enums::CaptureMethod>) -> Self {
        match method {
            Some(common_enums::CaptureMethod::Automatic) => Self::N,
            _ => Self::H,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::N => "N",
            Self::H => "H",
        }
    }
}

/// AsiaPay `lang`: language of the hosted payment page. The single-letter
/// codes are the gateway's wire values; request locales are mapped to them.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum AsiapayLanguage {
    #[serde(rename = "E")]
    English,
    #[serde(rename = "C")]
    TraditionalChinese,
    #[serde(rename = "X")]
    SimplifiedChinese,
    #[serde(rename = "J")]
    Japanese,
    #[serde(rename = "T")]
    Thai,
    #[serde(rename = "F")]
    French,
    #[serde(rename = "G")]
    German,
    #[serde(rename = "R")]
    Russian,
    #[serde(rename = "S")]
    Spanish,
    #[serde(rename = "V")]
    Vietnamese,
}

impl AsiapayLanguage {
    /// Prefer the explicit request locale, then the browser language. AsiaPay
    /// affects presentation only, so missing or unrecognised locales fall back
    /// to English rather than rejecting the payment.
    fn from_request<T: PaymentMethodDataTypes>(request: &PaymentsAuthorizeData<T>) -> Self {
        let locale = request.locale.as_deref().or_else(|| {
            request
                .browser_info
                .as_ref()
                .and_then(|browser| browser.language.as_deref())
        });
        Self::from_locale(locale)
    }

    /// Accept AsiaPay's codes and common case-insensitive BCP-47-style tags.
    /// An unspecified Chinese script defaults to Simplified Chinese.
    fn from_locale(locale: Option<&str>) -> Self {
        let normalized = locale
            .map(str::trim)
            .unwrap_or_default()
            .replace('_', "-")
            .to_ascii_lowercase();
        let mut parts = normalized.split('-');
        let language = parts.next().unwrap_or_default();
        let region_or_script = parts.next();

        match (language, region_or_script) {
            ("c", _) => Self::TraditionalChinese,
            ("x", _) => Self::SimplifiedChinese,
            ("j" | "ja", _) => Self::Japanese,
            ("t" | "th", _) => Self::Thai,
            ("f" | "fr", _) => Self::French,
            ("g" | "de", _) => Self::German,
            ("r" | "ru", _) => Self::Russian,
            ("s" | "es", _) => Self::Spanish,
            ("v" | "vi", _) => Self::Vietnamese,
            ("zh", Some("hant" | "tw" | "hk" | "mo")) => Self::TraditionalChinese,
            ("zh", _) => Self::SimplifiedChinese,
            _ => Self::English,
        }
    }
}

/// AsiaPay `pMethod`: the card scheme the payment runs on, derived from the
/// UCS `CardNetwork` (not from raw BIN ranges).
#[derive(Debug, Clone, Copy, Serialize)]
pub enum AsiapayPaymentMethod {
    #[serde(rename = "VISA")]
    Visa,
    #[serde(rename = "Master")]
    Master,
    #[serde(rename = "Diners")]
    Diners,
    #[serde(rename = "JCB")]
    Jcb,
    #[serde(rename = "AMEX")]
    Amex,
}

/// AsiaPay `threeDSTransType` (3DS 2.0 transaction type). Documented codes:
/// `01` = Goods/Service Purchase, `03` = Check Acceptance, `10` = Account
/// Funding, `11` = Quasi-Cash Transaction, `28` = Prepaid Activation and Load.
/// Only Goods/Service Purchase is used today.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum AsiapayThreeDsTransType {
    #[serde(rename = "01")]
    GoodsServicePurchase,
}

/// DPC payment request posted to `{base}/eng/dPayment/payComp.jsp`.
/// serde renames match the wire parameter names exactly (the body is
/// URL-encoded from this struct).
#[derive(Debug, Serialize)]
pub struct AsiapayCardAuthorizeRequest<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(rename = "merchantId")]
    pub merchant_id: Secret<String>,
    #[serde(rename = "orderRef")]
    pub order_ref: String,
    pub amount: StringMajorUnit,
    #[serde(rename = "currCode")]
    pub curr_code: String,
    pub lang: AsiapayLanguage,
    #[serde(rename = "payType")]
    pub pay_type: AsiapayPayType,
    #[serde(rename = "pMethod", skip_serializing_if = "Option::is_none")]
    pub p_method: Option<AsiapayPaymentMethod>,
    #[serde(rename = "cardNo")]
    pub card_no: RawCardNumber<T>,
    #[serde(rename = "epMonth")]
    pub ep_month: Secret<String>,
    #[serde(rename = "epYear")]
    pub ep_year: Secret<String>,
    #[serde(rename = "securityCode")]
    pub security_code: Secret<String>,
    #[serde(rename = "cardHolder", skip_serializing_if = "Option::is_none")]
    pub card_holder: Option<Secret<String>>,
    #[serde(rename = "successUrl")]
    pub success_url: String,
    #[serde(rename = "failUrl")]
    pub fail_url: String,
    #[serde(rename = "errorUrl")]
    pub error_url: String,
    #[serde(rename = "secureHash")]
    pub secure_hash: String,
    // ----- optional block: sent only when the value exists -----
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remark: Option<String>,
    #[serde(rename = "payMode", skip_serializing_if = "Option::is_none")]
    pub pay_mode: Option<String>,
    #[serde(rename = "cancelUrl", skip_serializing_if = "Option::is_none")]
    pub cancel_url: Option<String>,
    #[serde(rename = "billingFirstName", skip_serializing_if = "Option::is_none")]
    pub billing_first_name: Option<Secret<String>>,
    #[serde(rename = "billingLastName", skip_serializing_if = "Option::is_none")]
    pub billing_last_name: Option<Secret<String>>,
    #[serde(rename = "billingStreet1", skip_serializing_if = "Option::is_none")]
    pub billing_street1: Option<Secret<String>>,
    #[serde(rename = "billingStreet2", skip_serializing_if = "Option::is_none")]
    pub billing_street2: Option<Secret<String>>,
    #[serde(rename = "billingCity", skip_serializing_if = "Option::is_none")]
    pub billing_city: Option<Secret<String>>,
    #[serde(rename = "billingState", skip_serializing_if = "Option::is_none")]
    pub billing_state: Option<Secret<String>>,
    #[serde(rename = "billingPostalCode", skip_serializing_if = "Option::is_none")]
    pub billing_postal_code: Option<Secret<String>>,
    #[serde(rename = "billingCountry", skip_serializing_if = "Option::is_none")]
    pub billing_country: Option<String>,
    #[serde(rename = "billingEmail", skip_serializing_if = "Option::is_none")]
    pub billing_email: Option<Email>,
    #[serde(rename = "custIPAddress", skip_serializing_if = "Option::is_none")]
    pub cust_ip_address: Option<Secret<String, IpAddress>>,
    // ----- 3DS 2.0 block (sent only when resource_common_data.auth_type == ThreeDs) -----
    #[serde(rename = "threeDSTransType", skip_serializing_if = "Option::is_none")]
    pub three_ds_trans_type: Option<AsiapayThreeDsTransType>,
    #[serde(
        rename = "threeDSCustomerEmail",
        skip_serializing_if = "Option::is_none"
    )]
    pub three_ds_customer_email: Option<Email>,
    #[serde(
        rename = "threeDSMobilePhoneCountryCode",
        skip_serializing_if = "Option::is_none"
    )]
    pub three_ds_mobile_phone_country_code: Option<String>,
    #[serde(
        rename = "threeDSMobilePhoneNumber",
        skip_serializing_if = "Option::is_none"
    )]
    pub three_ds_mobile_phone_number: Option<Secret<String>>,
    #[serde(
        rename = "threeDSBillingCountryCode",
        skip_serializing_if = "Option::is_none"
    )]
    pub three_ds_billing_country_code: Option<String>,
    #[serde(
        rename = "threeDSBillingState",
        skip_serializing_if = "Option::is_none"
    )]
    pub three_ds_billing_state: Option<Secret<String>>,
    #[serde(rename = "threeDSBillingCity", skip_serializing_if = "Option::is_none")]
    pub three_ds_billing_city: Option<Secret<String>>,
    #[serde(
        rename = "threeDSBillingLine1",
        skip_serializing_if = "Option::is_none"
    )]
    pub three_ds_billing_line1: Option<Secret<String>>,
    #[serde(
        rename = "threeDSBillingLine2",
        skip_serializing_if = "Option::is_none"
    )]
    pub three_ds_billing_line2: Option<Secret<String>>,
    #[serde(
        rename = "threeDSBillingLine3",
        skip_serializing_if = "Option::is_none"
    )]
    pub three_ds_billing_line3: Option<Secret<String>>,
    #[serde(
        rename = "threeDSBillingPostalCode",
        skip_serializing_if = "Option::is_none"
    )]
    pub three_ds_billing_postal_code: Option<Secret<String>>,
    #[serde(
        rename = "threeDSShippingCountryCode",
        skip_serializing_if = "Option::is_none"
    )]
    pub three_ds_shipping_country_code: Option<String>,
    #[serde(
        rename = "threeDSShippingState",
        skip_serializing_if = "Option::is_none"
    )]
    pub three_ds_shipping_state: Option<Secret<String>>,
    #[serde(
        rename = "threeDSShippingCity",
        skip_serializing_if = "Option::is_none"
    )]
    pub three_ds_shipping_city: Option<Secret<String>>,
    #[serde(
        rename = "threeDSShippingLine1",
        skip_serializing_if = "Option::is_none"
    )]
    pub three_ds_shipping_line1: Option<Secret<String>>,
    #[serde(
        rename = "threeDSShippingLine2",
        skip_serializing_if = "Option::is_none"
    )]
    pub three_ds_shipping_line2: Option<Secret<String>>,
    #[serde(
        rename = "threeDSShippingLine3",
        skip_serializing_if = "Option::is_none"
    )]
    pub three_ds_shipping_line3: Option<Secret<String>>,
    #[serde(
        rename = "threeDSShippingPostalCode",
        skip_serializing_if = "Option::is_none"
    )]
    pub three_ds_shipping_postal_code: Option<Secret<String>>,
    #[serde(skip)]
    pub payment_method_type: PhantomData<T>,
}

/// AsiaPay expects the 3DS state/city/lines/postal-code buckets split out of
/// an `AddressDetails`; `country` goes as the ISO 3166-1 numeric code (3-digit,
/// left-padded), NOT the alpha-2 code used by the plain billing block.
struct ThreeDsAddressParams {
    country_code: Option<String>,
    state: Option<Secret<String>>,
    city: Option<Secret<String>>,
    line1: Option<Secret<String>>,
    line2: Option<Secret<String>>,
    line3: Option<Secret<String>>,
    postal_code: Option<Secret<String>>,
}

impl ThreeDsAddressParams {
    fn from_address_details(details: &AddressDetails) -> Self {
        Self {
            country_code: details
                .country
                .map(|country| format!("{:03}", common_enums::CountryAlpha2::to_numeric(country))),
            state: details
                .state
                .as_ref()
                .and_then(|state| state.code_or_name().cloned()),
            city: details.city.clone(),
            line1: details.line1.clone(),
            line2: details.line2.clone(),
            line3: details.line3.clone(),
            postal_code: details.zip.clone(),
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AsiapayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AsiapayCardAuthorizeRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: AsiapayRouterData<
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

        let auth = AsiapayAuthType::try_from(&router_data.connector_config)?;

        let card = match &request.payment_method_data {
            PaymentMethodData::Card(card) => card,
            _ => {
                return Err(IntegrationError::NotSupported {
                    message: "Only card payments are supported by asiapay".to_string(),
                    connector: "asiapay",
                    context: Default::default(),
                }
                .into())
            }
        };

        // pMethod: prefer the caller-provided card network. Callers that do
        // not send a network (Euler's raw-card path) fall back to PAN
        // inference via the shared `get_card_issuer` BIN table — stayed
        // consistent with the other connectors' use of that util. When the
        // issuer cannot be resolved (or isn't an AsiaPay-supported brand),
        // pMethod is OMITTED (it is optional per the DPC contract).
        let p_method = match &card.card_network {
            Some(CardNetwork::Visa) => Some(AsiapayPaymentMethod::Visa),
            Some(CardNetwork::Mastercard) => Some(AsiapayPaymentMethod::Master),
            Some(CardNetwork::DinersClub) => Some(AsiapayPaymentMethod::Diners),
            Some(CardNetwork::JCB) => Some(AsiapayPaymentMethod::Jcb),
            Some(CardNetwork::AmericanExpress) => Some(AsiapayPaymentMethod::Amex),
            Some(other) => {
                return Err(IntegrationError::NotSupported {
                    message: format!(
                        "Card network {other:?} cannot be mapped to an asiapay pMethod \
                         (supported: VISA, Master, Diners, JCB, AMEX)"
                    ),
                    connector: "asiapay",
                    context: Default::default(),
                }
                .into())
            }
            None => match domain_types::utils::get_card_issuer(card.card_number.peek()) {
                Ok(CardIssuer::Visa) => Some(AsiapayPaymentMethod::Visa),
                Ok(CardIssuer::Master) => Some(AsiapayPaymentMethod::Master),
                Ok(CardIssuer::DinersClub) => Some(AsiapayPaymentMethod::Diners),
                Ok(CardIssuer::JCB) => Some(AsiapayPaymentMethod::Jcb),
                Ok(CardIssuer::AmericanExpress) => Some(AsiapayPaymentMethod::Amex),
                Ok(_) | Err(_) => None,
            },
        };

        // successUrl / failUrl / errorUrl (and cancelUrl) are all the UCS
        // connector return URL: AsiaPay drives the outcome through
        // successcode/prc/src on the redirect, not through different URLs.
        // The params are required by the DPC API, so the return URL is
        // required here.
        let return_url = request.get_router_return_url()?;

        let amount = AsiapayAmountConvertor::convert(&request.money)?;
        let curr_code = request.currency.iso_4217();
        let pay_type = AsiapayPayType::from_capture_method(request.capture_method);
        let order_ref = common.connector_request_reference_id.clone();

        // The amount signed into secureHash must be the exact wire string.
        let secure_hash = sign_payment_request(
            auth.merchant_id.peek(),
            &order_ref,
            curr_code,
            &amount.get_amount_as_string(),
            pay_type.as_str(),
            auth.secure_hash_secret.peek(),
            auth.secure_hash_algorithm,
        );

        let billing = common.get_optional_billing();
        let billing_details = billing.and_then(|address| address.address.as_ref());
        // Billing email: the billing address' own email wins, then the
        // payment-level customer email.
        let billing_email = billing
            .and_then(|address| address.email.clone())
            .or_else(|| request.email.clone());

        let is_three_ds = common.auth_type == AuthenticationType::ThreeDs;

        let (
            three_ds_trans_type,
            three_ds_customer_email,
            three_ds_mobile_phone_country_code,
            three_ds_mobile_phone_number,
            three_ds_billing,
            three_ds_shipping,
        ) = if is_three_ds {
            // Paired phone fields: either both keys or neither.
            let (mobile_cc, mobile_no) = request
                .customer
                .as_ref()
                .and_then(|customer| {
                    customer
                        .customer_phone_country_code
                        .clone()
                        .zip(customer.customer_phone_number.clone())
                })
                .map_or((None, None), |(cc, no)| {
                    (Some(cc.trim_start_matches('+').to_string()), Some(no))
                });

            let three_ds_billing = billing_details.map(ThreeDsAddressParams::from_address_details);
            let shipping_details = common
                .get_optional_shipping()
                .and_then(|address| address.address.as_ref());
            let three_ds_shipping =
                shipping_details.map(ThreeDsAddressParams::from_address_details);

            (
                Some(AsiapayThreeDsTransType::GoodsServicePurchase),
                request.get_optional_email(),
                mobile_cc,
                mobile_no,
                three_ds_billing,
                three_ds_shipping,
            )
        } else {
            (None, None, None, None, None, None)
        };

        Ok(Self {
            merchant_id: auth.merchant_id,
            order_ref,
            amount,
            curr_code: curr_code.to_string(),
            lang: AsiapayLanguage::from_request(request),
            pay_type,
            p_method,
            card_no: card.card_number.clone(),
            ep_month: card.get_card_expiry_month_2_digit()?,
            ep_year: card.get_expiry_year_4_digit(),
            security_code: card.card_cvc.clone(),
            card_holder: card
                .card_holder_name
                .clone()
                .or_else(|| request.get_optional_customer_name())
                .or_else(|| billing.and_then(|address| address.get_optional_full_name())),
            success_url: return_url.clone(),
            fail_url: return_url.clone(),
            error_url: return_url.clone(),
            secure_hash,
            remark: common.description.clone(),
            // Constant per the DPC optional-parameters page: EC = e-commerce.
            pay_mode: Some("EC".to_string()),
            cancel_url: Some(return_url),
            billing_first_name: billing_details.and_then(|details| details.first_name.clone()),
            billing_last_name: billing_details.and_then(|details| details.last_name.clone()),
            billing_street1: billing_details.and_then(|details| details.line1.clone()),
            billing_street2: billing_details.and_then(|details| details.line2.clone()),
            billing_city: billing_details.and_then(|details| details.city.clone()),
            billing_state: billing_details.and_then(|details| {
                details
                    .state
                    .as_ref()
                    .and_then(|state| state.code_or_name().cloned())
            }),
            billing_postal_code: billing_details.and_then(|details| details.zip.clone()),
            billing_country: billing_details
                .and_then(|details| details.country)
                .map(|country| country.to_string()),
            billing_email,
            cust_ip_address: request.get_ip_address_as_optional(),
            three_ds_trans_type,
            three_ds_customer_email,
            three_ds_mobile_phone_country_code,
            three_ds_mobile_phone_number,
            three_ds_billing_country_code: three_ds_billing
                .as_ref()
                .and_then(|params| params.country_code.clone()),
            three_ds_billing_state: three_ds_billing
                .as_ref()
                .and_then(|params| params.state.clone()),
            three_ds_billing_city: three_ds_billing
                .as_ref()
                .and_then(|params| params.city.clone()),
            three_ds_billing_line1: three_ds_billing
                .as_ref()
                .and_then(|params| params.line1.clone()),
            three_ds_billing_line2: three_ds_billing
                .as_ref()
                .and_then(|params| params.line2.clone()),
            three_ds_billing_line3: three_ds_billing
                .as_ref()
                .and_then(|params| params.line3.clone()),
            three_ds_billing_postal_code: three_ds_billing
                .as_ref()
                .and_then(|params| params.postal_code.clone()),
            three_ds_shipping_country_code: three_ds_shipping
                .as_ref()
                .and_then(|params| params.country_code.clone()),
            three_ds_shipping_state: three_ds_shipping
                .as_ref()
                .and_then(|params| params.state.clone()),
            three_ds_shipping_city: three_ds_shipping
                .as_ref()
                .and_then(|params| params.city.clone()),
            three_ds_shipping_line1: three_ds_shipping
                .as_ref()
                .and_then(|params| params.line1.clone()),
            three_ds_shipping_line2: three_ds_shipping
                .as_ref()
                .and_then(|params| params.line2.clone()),
            three_ds_shipping_line3: three_ds_shipping
                .as_ref()
                .and_then(|params| params.line3.clone()),
            three_ds_shipping_postal_code: three_ds_shipping
                .as_ref()
                .and_then(|params| params.postal_code.clone()),
            payment_method_type: PhantomData,
        })
    }
}

/// AsiaPay `payMethod` value for LINE Pay payments.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum AsiapayLinePayMethod {
    #[serde(rename = "LINEPAY")]
    LinePay,
}

/// CPTB (Client Post Through Browser) payment request posted to
/// `{base}/eng/payment/payForm.jsp` for LINE Pay payments.
#[derive(Debug, Serialize)]
pub struct AsiapayLinePayAuthorizeRequest {
    #[serde(rename = "merchantId")]
    pub merchant_id: Secret<String>,
    #[serde(rename = "orderRef")]
    pub order_ref: String,
    pub amount: StringMajorUnit,
    #[serde(rename = "currCode")]
    pub curr_code: String,
    pub lang: AsiapayLanguage,
    #[serde(rename = "payType")]
    pub pay_type: AsiapayPayType,
    #[serde(rename = "payMethod")]
    pub pay_method: AsiapayLinePayMethod,
    #[serde(rename = "successUrl")]
    pub success_url: String,
    #[serde(rename = "failUrl")]
    pub fail_url: String,
    #[serde(rename = "errorUrl")]
    pub error_url: String,
    #[serde(rename = "cancelUrl")]
    pub cancel_url: String,
    #[serde(rename = "secureHash")]
    pub secure_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remark: Option<String>,
    #[serde(rename = "payMode", skip_serializing_if = "Option::is_none")]
    pub pay_mode: Option<String>,
}

/// Authorize request body — dispatches to the card DPC request or the LINE Pay
/// CPTB request based on the payment method data.
// Both variants are flat URL-encoded form bodies consumed immediately by
// serde_urlencoded; boxing the smaller variant would add allocation for no
// practical benefit since neither is stored long-term.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum AsiapayAuthorizeRequest<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
> {
    Card(AsiapayCardAuthorizeRequest<T>),
    LinePay(AsiapayLinePayAuthorizeRequest),
}

fn validate_line_pay_capture_method(
    capture_method: Option<common_enums::CaptureMethod>,
) -> Result<(), error_stack::Report<IntegrationError>> {
    // LINE Pay CPTB only supports normal sale (payType=N), never hold.
    if let Some(common_enums::CaptureMethod::Manual) = capture_method {
        return Err(IntegrationError::NotImplemented(
            "Manual capture is not supported for LINE Pay on asiapay".to_string(),
            Default::default(),
        )
        .into());
    }
    Ok(())
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AsiapayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AsiapayLinePayAuthorizeRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: AsiapayRouterData<
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
        let auth = AsiapayAuthType::try_from(&router_data.connector_config)?;

        validate_line_pay_capture_method(request.capture_method)?;

        let amount = AsiapayAmountConvertor::convert(&request.money)?;
        let curr_code = request.currency.iso_4217();
        let pay_type = AsiapayPayType::N;
        let order_ref = common.connector_request_reference_id.clone();
        let return_url = request.get_router_return_url()?;

        let secure_hash = sign_payment_request(
            auth.merchant_id.peek(),
            &order_ref,
            curr_code,
            &amount.get_amount_as_string(),
            pay_type.as_str(),
            auth.secure_hash_secret.peek(),
            auth.secure_hash_algorithm,
        );

        Ok(Self {
            merchant_id: auth.merchant_id,
            order_ref,
            amount,
            curr_code: curr_code.to_string(),
            lang: AsiapayLanguage::from_request(request),
            pay_type,
            pay_method: AsiapayLinePayMethod::LinePay,
            success_url: return_url.clone(),
            fail_url: return_url.clone(),
            error_url: return_url.clone(),
            cancel_url: return_url,
            secure_hash,
            remark: common.description.clone(),
            pay_mode: Some("EC".to_string()),
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AsiapayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AsiapayAuthorizeRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: AsiapayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        match &item.router_data.request.payment_method_data {
            PaymentMethodData::Card(_) => {
                let card_req = AsiapayCardAuthorizeRequest::try_from(item)?;
                Ok(Self::Card(card_req))
            }
            PaymentMethodData::Wallet(wallet_data) => match wallet_data {
                domain_types::payment_method_data::WalletData::LinePayRedirect {} => Ok(
                    Self::LinePay(AsiapayLinePayAuthorizeRequest::try_from(item)?),
                ),
                _ => Err(IntegrationError::NotSupported {
                    message: "Wallet is not supported by asiapay".to_string(),
                    connector: "asiapay",
                    context: Default::default(),
                }
                .into()),
            },
            _ => Err(IntegrationError::NotSupported {
                message: "Only card and LINE Pay payments are supported by asiapay".to_string(),
                connector: "asiapay",
                context: Default::default(),
            }
            .into()),
        }
    }
}

// ============================================================================
// NATIVE AUTHORIZE RESPONSE (local auto-submit POST form)
// ============================================================================

type AsiapayAuthorizeRouterData<T> =
    RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>;

/// Build the existing typed and signed request through its established TryFrom
/// conversion path. The local flow invokes this once for request validation
/// and once for rendering; no outbound body is constructed.
fn build_authorize_request<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
>(
    router_data: &AsiapayAuthorizeRouterData<T>,
) -> Result<AsiapayAuthorizeRequest<T>, error_stack::Report<IntegrationError>> {
    AsiapayAuthorizeRequest::try_from(AsiapayRouterData {
        connector: (*Asiapay::<T>::new()).clone(),
        router_data: router_data.clone(),
    })
}

/// The local macro calls this from build_request_v2. Keeping all payment
/// method, capture-method, credential, amount and return-url validation here
/// preserves IntegrationError as the request-side error category.
pub(crate) fn validate_authorize_request<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
>(
    router_data: &AsiapayAuthorizeRouterData<T>,
) -> common_utils::errors::CustomResult<(), IntegrationError> {
    build_authorize_request(router_data).map(|_| ())
}

fn native_form_error(message: &'static str) -> error_stack::Report<ConnectorError> {
    error_stack::report!(ConnectorError::ResponseHandlingFailed {
        context: ResponseTransformationErrorContext {
            http_status_code: None,
            // Deliberately static: serialization/request errors and form values
            // can carry card, billing or credential material and must not be
            // copied into diagnostics.
            additional_context: Some(message.to_string()),
        },
    })
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    AsiapayAuthorizeRequest<T>
{
    fn order_ref(&self) -> &str {
        match self {
            Self::Card(request) => &request.order_ref,
            Self::LinePay(request) => &request.order_ref,
        }
    }

    fn endpoint_path(&self) -> &'static str {
        match self {
            Self::Card(_) => "/eng/dPayment/payComp.jsp",
            Self::LinePay(_) => "/eng/payment/payForm.jsp",
        }
    }

    /// Serialize exactly the fields the former URL-encoded outbound request
    /// contained, then turn those ordered pairs into hidden form inputs. This
    /// preserves Secret and StringMajorUnit wire representations without a
    /// second secret-specific serializer. No intermediate/request HTML is
    /// logged or attached to connector events.
    fn to_native_html(
        &self,
        configured_base_url: &str,
    ) -> Result<String, error_stack::Report<ConnectorError>> {
        let base_url = configured_base_url.trim().trim_end_matches('/');
        if base_url.is_empty() {
            return Err(native_form_error(
                "asiapay: connector base_url is empty; cannot render the native Authorize form",
            ));
        }

        let encoded_fields = serde_urlencoded::to_string(self).map_err(|_| {
            native_form_error("asiapay: failed to serialize the typed native Authorize form fields")
        })?;
        let form_fields = url::form_urlencoded::parse(encoded_fields.as_bytes())
            .into_owned()
            .collect::<Vec<_>>();
        let action = format!("{base_url}{}", self.endpoint_path());
        Ok(render_native_post_form(&action, &form_fields))
    }
}

/// Render a complete, compact POST document. Dynamic attributes all pass
/// through html_escape's double-quoted-attribute encoder, including the
/// action and field names; values are never inserted as raw markup.
fn render_native_post_form(action: &str, fields: &[(String, String)]) -> String {
    let mut inputs = String::new();
    for (name, value) in fields {
        inputs.push_str("<input type=\"hidden\" name=\"");
        inputs.push_str(html_escape::encode_double_quoted_attribute(name).as_ref());
        inputs.push_str("\" value=\"");
        inputs.push_str(html_escape::encode_double_quoted_attribute(value).as_ref());
        inputs.push_str("\">");
    }

    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>AsiaPay payment</title></head><body><form id=\"asiapay-payment-form\" action=\"{}\" method=\"post\">{}<noscript><button type=\"submit\">Continue to AsiaPay</button></noscript></form><script>document.getElementById(\"asiapay-payment-form\").submit();</script></body></html>",
        html_escape::encode_double_quoted_attribute(action),
        inputs
    )
}

/// All locally produced state in one value makes it difficult to update the
/// response while forgetting AuthenticationPending or request integrity.
struct NativeAuthorizeResponse {
    status: AttemptStatus,
    integrity_object: AuthoriseIntegrityObject,
    response: PaymentsResponseData,
}

fn build_native_authorize_response(
    order_ref: String,
    html_data: String,
    amount: MinorUnit,
    currency: Currency,
) -> NativeAuthorizeResponse {
    NativeAuthorizeResponse {
        // UCS only created browser instructions. AsiaPay has not authoritatively
        // accepted or rejected anything yet; PSync/Data Feed reconciles it.
        status: AttemptStatus::AuthenticationPending,
        integrity_object: AuthoriseIntegrityObject { amount, currency },
        response: PaymentsResponseData::TransactionResponse {
            // PayRef does not exist until the browser completes payment. The
            // orderRef stand-in keeps the id chain alive; PSync still queries
            // connector_request_reference_id and later supplies PayRef.
            resource_id: ResponseId::ConnectorTransactionId(order_ref.clone()),
            redirection_data: Some(Box::new(RedirectForm::Html { html_data })),
            connector_response_reference_id: Some(order_ref),
            connector_metadata: None,
            mandate_reference: None,
            network_txn_id: None,
            network_txn_link_id: None,
            incremental_authorization_allowed: None,
            splits: None,
            status_code: 200,
            payment_account_reference: None,
        },
    }
}

pub(crate) fn handle_authorize_response<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
>(
    data: &AsiapayAuthorizeRouterData<T>,
    // Never attach the generated HTML (which includes card/billing data for
    // DPC) to raw request/response diagnostics or event values.
    _event_builder: Option<&mut common_utils::events::Event>,
    _res: domain_types::router_response_types::Response,
) -> common_utils::errors::CustomResult<AsiapayAuthorizeRouterData<T>, ConnectorError> {
    let request = build_authorize_request(data).map_err(|_| {
        native_form_error(
            "asiapay: the validated Authorize request could not be reconstructed for native rendering",
        )
    })?;
    let order_ref = request.order_ref().to_string();
    let html_data =
        request.to_native_html(&data.resource_common_data.connectors.asiapay.base_url)?;
    let native = build_native_authorize_response(
        order_ref,
        html_data,
        data.request.money.amount(),
        data.request.currency,
    );

    let mut router_data = data.clone();
    router_data.resource_common_data.status = native.status;
    router_data.request.integrity_object = Some(native.integrity_object);
    router_data.response = Ok(native.response);
    Ok(router_data)
}

// ============================================================================
// SHARED WIRE RESULT CODE (Data Feed/webhook)
// ============================================================================

/// AsiaPay `successcode` (payment status classification, vendor docs):
/// `0` = succeeded, `1` = failure, `2` = pending.
/// `#[serde(other)]` keeps an unrecognised value parseable — it lands in
/// `Other` instead of failing the whole response/payload. Mapping sites keep
/// unknown and missing codes non-terminal rather than inferring failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AsiapaySuccessCode {
    #[serde(rename = "0")]
    Succeeded,
    #[serde(rename = "1")]
    Failed,
    #[serde(rename = "2")]
    Pending,
    #[serde(other)]
    Other,
}

impl AsiapaySuccessCode {
    /// Deserialize a wire `successcode` string exactly as serde would
    /// (`#[serde(other)]` bundles unrecognised values into `Other`), for the
    /// surfaces that carry the code inside a raw string container, notably
    /// the webhook body (kept raw because the exact bytes are re-signed for
    /// `secureHash` verification). Infallible: `#[serde(other)]` makes any
    /// string parse.
    pub fn from_wire(code: &str) -> Self {
        serde_json::from_value(serde_json::Value::String(code.to_string()))
            // `#[serde(other)]` makes this infallible for any string input.
            .unwrap_or(Self::Other)
    }
}

/// Parse AsiaPay's echoed `Amt` (major-unit string, e.g. `"80.00"`) back into
/// `Money` via the connector's own StringMajorUnit converter (inverse
/// direction). `StringMajorUnit` has no public constructor, so the echoed
/// string is round-tripped through its serde representation.
fn parse_echoed_money(
    amt: &str,
    currency: Currency,
    http_code: u16,
) -> Result<Money, error_stack::Report<ConnectorError>> {
    let echoed: StringMajorUnit =
        serde_json::from_value(serde_json::Value::String(amt.to_string())).map_err(|_| {
            error_stack::report!(ConnectorError::response_handling_failed_with_context(
                http_code,
                Some(format!(
                    "asiapay: echoed Amt `{amt}` is not a valid {currency:?} amount string"
                )),
            ))
        })?;
    AsiapayAmountConvertor::convert_back(echoed, currency).map_err(|conversion_error| {
        error_stack::report!(ConnectorError::response_handling_failed_with_context(
            http_code,
            Some(format!(
                "asiapay: could not convert echoed Amt `{amt}` ({conversion_error:?})"
            )),
        ))
    })
}

fn parse_echoed_amount(
    amt: &str,
    currency: Currency,
    http_code: u16,
) -> Result<MinorUnit, error_stack::Report<ConnectorError>> {
    parse_echoed_money(amt, currency, http_code).map(|money| money.amount())
}

// ============================================================================
// PSYNC (Merchant API Query, `orderApi.jsp`, application/x-www-form-urlencoded
// request, XML response)
// ============================================================================

/// AsiaPay `actionType`: the operation a Merchant API (`orderApi.jsp`) call
/// performs. serde renames match the wire values exactly.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum AsiapayActionType {
    #[serde(rename = "Query")]
    Query,
    #[serde(rename = "Capture")]
    Capture,
    #[serde(rename = "OnlineReversal")]
    OnlineReversal,
    #[serde(rename = "RequestRefund")]
    RequestRefund,
}

/// `errMsg` value AsiaPay attaches to a SUCCESSFUL Query response. Any other
/// message — and any message on a response whose `orderStatus` says Rejected —
/// is an in-band failure (the endpoint always answers HTTP 200).
/// The Merchant API Query answers HTTP 200 even for found orders; its
/// success sentinel text varies by merchant block/brand: DPC sandbox
/// (observed live) replies `Query Successfully`, the DPC/EOC block uses
/// `EOC Query Successfully`. Both are success — anything else is an in-band
/// error (order not found, bad credentials, ...).
const ASIAPAY_QUERY_SUCCESS_MESSAGES: &[&str] = &["Query Successfully", "EOC Query Successfully"];

fn is_query_err_msg(err_msg: Option<&str>) -> bool {
    err_msg.is_some_and(|err_msg| {
        let err_msg = err_msg.trim();
        !ASIAPAY_QUERY_SUCCESS_MESSAGES.contains(&err_msg)
    })
}

/// Merchant API Query request posted to `{base}/eng/merchant/api/orderApi.jsp`.
/// Keyed by `orderRef` — the merchant reference sent on the original Authorize
/// (`merchantTransactionId`), carried into PSync on
/// `PaymentFlowData.connector_request_reference_id`.
#[derive(Debug, Serialize)]
pub struct AsiapayPSyncRequest {
    #[serde(rename = "merchantId")]
    pub merchant_id: Secret<String>,
    #[serde(rename = "loginId")]
    pub login_id: Secret<String>,
    #[serde(rename = "password")]
    pub password: Secret<String>,
    #[serde(rename = "actionType")]
    pub action_type: AsiapayActionType,
    #[serde(rename = "orderRef")]
    pub order_ref: String,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AsiapayRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for AsiapayPSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: AsiapayRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = AsiapayAuthType::try_from(&router_data.connector_config)?;

        // orderRef SOURCE RULE: the Query is keyed ONLY by the resource-carried
        // merchant reference. Never re-derive or synthesize a lookup key and
        // never fall back to a re-computed id — an empty carrier is a hard
        // MissingRequiredField, not a silent `orderRef=`.
        let order_ref = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        if order_ref.trim().is_empty() {
            return Err(error_stack::report!(
                IntegrationError::MissingRequiredField {
                    field_name: "connector_request_reference_id",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "asiapay PSync (Query) requires the orderRef sent on the original \
                             payment, sourced from connector_request_reference_id, which is \
                             empty for this sync request."
                                .to_string(),
                        ),
                        suggested_action: Some(
                            "Issue the payment with a non-empty connector_request_reference_id \
                             so PSync can query it by orderRef."
                                .to_string(),
                        ),
                        doc_url: None,
                    },
                }
            ));
        }

        Ok(Self {
            merchant_id: auth.merchant_id,
            login_id: auth.login_id,
            password: auth.password,
            action_type: AsiapayActionType::Query,
            order_ref,
        })
    }
}

/// AsiaPay `orderStatus` wire values (payment status classification, Appendix A
/// of the integration guide). `#[serde(other)]` keeps an unrecognised value
/// parseable — it lands in `Unknown` instead of failing the whole response,
/// and the mapping below funnels it to the non-terminal `Pending`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AsiapayOrderStatus {
    Accepted,
    Captured,
    #[serde(rename = "Accepted_Adj")]
    AcceptedAdj,
    Authorized,
    #[serde(rename = "Authorized_Adj")]
    AuthorizedAdj,
    Pending,
    #[serde(rename = "Pending_feedback")]
    PendingFeedback,
    Capturing,
    Voiding,
    RequestRefund,
    RequestPartialRefund,
    #[serde(rename = "Pending_3D")]
    Pending3d,
    Voided,
    #[serde(rename = "Reversal-Auth")]
    ReversalAuth,
    Refunded,
    PartialRefunded,
    Rejected,
    #[serde(other)]
    Unknown,
}

impl From<AsiapayOrderStatus> for AttemptStatus {
    fn from(status: AsiapayOrderStatus) -> Self {
        // Exhaustive on purpose — no `_ =>` arm. A new variant on
        // AsiapayOrderStatus must break compilation here, not be silently
        // funnelled into a catch-all.
        match status {
            // `Refunded` / `PartialRefunded` are still completed charges — the
            // refund itself is tracked via RSync, not PSync.
            AsiapayOrderStatus::Accepted
            | AsiapayOrderStatus::Captured
            | AsiapayOrderStatus::AcceptedAdj
            | AsiapayOrderStatus::Refunded
            | AsiapayOrderStatus::PartialRefunded => Self::Charged,
            AsiapayOrderStatus::Authorized | AsiapayOrderStatus::AuthorizedAdj => Self::Authorized,
            AsiapayOrderStatus::Pending
            | AsiapayOrderStatus::PendingFeedback
            | AsiapayOrderStatus::Capturing
            | AsiapayOrderStatus::Voiding
            | AsiapayOrderStatus::RequestRefund
            | AsiapayOrderStatus::RequestPartialRefund
            // Pending-safe: an unrecognised status is not proof of failure.
            | AsiapayOrderStatus::Unknown => Self::Pending,
            AsiapayOrderStatus::Pending3d => Self::AuthenticationPending,
            AsiapayOrderStatus::Voided | AsiapayOrderStatus::ReversalAuth => Self::Voided,
            AsiapayOrderStatus::Rejected => Self::Failure,
        }
    }
}

/// Wire response of the Merchant API Query, deserialized directly by
/// quick-xml via the `response_format: xml` prerequisite in asiapay.rs.
/// AsiaPay answers an ENVELOPE — `<records><record>…flat result tags…</record></records>`
/// — so the response type models the envelope itself; the flow's result tags
/// live one level down on `record`. A query without a result returns an empty
/// `<records></records>` envelope, which carries no authoritative payment
/// state and must not fail XML deserialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsiapayPSyncResponse {
    pub record: Option<AsiapayPSyncRecord>,
}

/// The result tags carried inside `<record>`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AsiapayPSyncRecord {
    #[serde(rename = "merchantId")]
    pub merchant_id: Option<String>,
    #[serde(rename = "errMsg")]
    pub err_msg: Option<String>,
    #[serde(rename = "orderStatus")]
    pub order_status: Option<AsiapayOrderStatus>,
    /// Echo of the `orderRef` we queried by (AsiaPay's `ref`).
    #[serde(rename = "ref")]
    pub order_ref: Option<String>,
    /// AsiaPay's payment-gateway reference — the `connector_transaction_id`.
    #[serde(rename = "payRef")]
    pub pay_ref: Option<String>,
    #[serde(rename = "mpsMode")]
    pub mps_mode: Option<String>,
    /// Major-unit amount string (e.g. "80.00").
    pub amt: Option<String>,
    /// Numeric ISO-4217 currency code (e.g. "344").
    pub cur: Option<String>,
    pub prc: Option<String>,
    pub src: Option<String>,
    pub ord: Option<String>,
    pub holder: Option<String>,
    #[serde(rename = "authId")]
    pub auth_id: Option<String>,
    #[serde(rename = "txTime")]
    pub tx_time: Option<String>,
    pub eci: Option<String>,
    #[serde(rename = "channelType")]
    pub channel_type: Option<String>,
    #[serde(rename = "alertCode")]
    pub alert_code: Option<String>,
}

impl AsiapayPSyncRecord {
    fn status_or_pending(&self) -> AttemptStatus {
        // An empty Query result, a record without orderStatus, and an
        // unrecognised status are all non-authoritative. None proves failure;
        // a later Query or verified Data Feed can resolve the payment.
        self.order_status
            .map(AttemptStatus::from)
            .unwrap_or(AttemptStatus::Pending)
    }
}

impl TryFrom<ResponseRouterData<AsiapayPSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<AsiapayPSyncResponse, Self>) -> Result<Self, Self::Error> {
        // `<records></records>` means the Query returned no payment result.
        // Keep the normal in-band error routing for populated records; use
        // non-terminal Pending and no fabricated identifiers for an empty one.
        let response = item.response.record.unwrap_or_default();
        let router_data = item.router_data;
        let http_code = item.http_code;

        let status = response.status_or_pending();

        // Keep the raw connector outcome for observability.
        let raw_connector_status = RawConnectorStatus {
            code: response.prc.clone(),
            message: response.err_msg.clone(),
            reason: response.src.clone(),
        };
        let pay_ref = response.pay_ref.clone();

        // IN-BAND FAILURE: the Query endpoint answers HTTP 200 for "order not
        // found", bad credentials, and declined payments alike. Failure =
        // explicit errMsg other than the success sentinel, or a status the
        // connector itself reports as Rejected (mapped above to Failure).
        let query_errored = is_query_err_msg(response.err_msg.as_deref());
        if query_errored || domain_types::utils::is_payment_failure(status) {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    raw_connector_status: Some(raw_connector_status),
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    // prc identifies the bank/payment outcome; src the system
                    // component. Never `.unwrap_or_default()`.
                    code: response
                        .prc
                        .clone()
                        .or_else(|| response.src.clone())
                        .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                    message: response
                        .err_msg
                        .clone()
                        .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                    reason: response.err_msg.clone(),
                    status_code: http_code,
                    // Flow-aware: PSync reports a payment status, so the tag is
                    // FlowStatus::Payment; the value comes from the orderStatus
                    // mapping (terminal ONLY for Rejected → Failure; an
                    // errMsg-only failure stays on the mapped status, e.g.
                    // Pending, leaving a later sync able to resolve it).
                    attempt_status: Some(FlowStatus::Payment(status)),
                    connector_transaction_id: pay_ref,
                    ..Default::default()
                }),
                ..router_data
            });
        }

        // ---- success / pending ----
        // Integrity: prefer the connector-echoed Amt/Cur (major-unit string →
        // MinorUnit via the connector's own converter, same as Authorize). If
        // `cur` is a code outside the supported table, fall back to the
        // request currency so a new market does not hard-fail bookkeeping; if
        // `amt` is absent (contract violation), mirror the request amount.
        let currency = response
            .cur
            .as_deref()
            .and_then(crate::utils::currency_from_iso_4217_numeric)
            .unwrap_or(router_data.request.money.currency());
        let amount = match response.amt.as_deref() {
            Some(amt) => parse_echoed_amount(amt, currency, http_code)?,
            None => router_data.request.money.amount(),
        };

        // Identifier carrier chain: re-publish the connector feature data the
        // Authorize response handed back (None on this connector today, but do
        // not sever the chain if that changes).
        let connector_metadata = router_data
            .request
            .connector_feature_data
            .clone()
            .map(|meta| meta.expose());

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                raw_connector_status: Some(raw_connector_status),
                ..router_data.resource_common_data.clone()
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                // payRef is AsiaPay's payment-gateway reference.
                resource_id: pay_ref
                    .map(ResponseId::ConnectorTransactionId)
                    // By contract payRef is present on a found order; if a
                    // sandbox quirk drops it, still surface the outcome.
                    .unwrap_or(ResponseId::NoResponseId),
                redirection_data: None,
                connector_metadata,
                mandate_reference: None,
                network_txn_id: response.auth_id.clone(),
                network_txn_link_id: None,
                connector_response_reference_id: response.order_ref.clone(),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: http_code,
                payment_account_reference: None,
            }),
            request: PaymentsSyncData {
                integrity_object: Some(PaymentSynIntegrityObject { amount, currency }),
                ..router_data.request.clone()
            },
            ..router_data
        })
    }
}

// ============================================================================
// CAPTURE (Merchant API, `orderApi.jsp`, `actionType=Capture`,
// application/x-www-form-urlencoded request AND response)
// ============================================================================

/// Merchant API Capture request posted to `{base}/eng/merchant/api/orderApi.jsp`.
/// Captures (fully or partially) a previous authorize-hold (`payType=H`),
/// keyed by AsiaPay's `payRef`. serde renames match the wire parameter names
/// exactly (the body is URL-encoded from this struct).
#[derive(Debug, Serialize)]
pub struct AsiapayCaptureRequest {
    #[serde(rename = "merchantId")]
    pub merchant_id: Secret<String>,
    #[serde(rename = "loginId")]
    pub login_id: Secret<String>,
    #[serde(rename = "password")]
    pub password: Secret<String>,
    #[serde(rename = "actionType")]
    pub action_type: AsiapayActionType,
    #[serde(rename = "payRef")]
    pub pay_ref: String,
    /// Major-unit string ("80.00"), ≤ the authorized amount (partial capture
    /// is allowed by the gateway).
    pub amount: StringMajorUnit,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AsiapayRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for AsiapayCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: AsiapayRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = AsiapayAuthType::try_from(&router_data.connector_config)?;

        // payRef SOURCE RULE: the Capture is keyed ONLY by the payment-gateway
        // reference AsiaPay issued at authorize time, carried into Capture on
        // `PaymentsCaptureData::connector_transaction_id`
        // (ResponseId::ConnectorTransactionId). Never re-derive or synthesize a
        // reference — an absent carrier is a hard MissingRequiredField.
        let pay_ref = match &router_data.request.connector_transaction_id {
            ResponseId::ConnectorTransactionId(pay_ref) if !pay_ref.trim().is_empty() => {
                pay_ref.clone()
            }
            _ => {
                return Err(error_stack::report!(
                    IntegrationError::MissingRequiredField {
                        field_name: "connector_transaction_id",
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "asiapay Capture requires the payRef returned by the original \
                                 authorize, carried on connector_transaction_id \
                                 (ResponseId::ConnectorTransactionId), which is absent for this \
                                 capture request."
                                    .to_string(),
                            ),
                            suggested_action: Some(
                                "Authorize the payment first and pass the authorize response's \
                                 connector_transaction_id (AsiaPay payRef) into the capture \
                                 request."
                                    .to_string(),
                            ),
                            doc_url: None,
                        },
                    }
                ));
            }
        };

        // The amount being captured NOW (`capture_money`), which may be
        // below the originally authorized amount — partial capture is a
        // first-class AsiaPay feature.
        let amount = AsiapayAmountConvertor::convert(&router_data.request.capture_money)?;

        Ok(Self {
            merchant_id: auth.merchant_id,
            login_id: auth.login_id,
            password: auth.password,
            action_type: AsiapayActionType::Capture,
            pay_ref,
            amount,
        })
    }
}

/// Capture-context mapping of AsiaPay's `orderStatus`. Reuses the
/// `AsiapayOrderStatus` wire enum but deliberately differs from the shared
/// `From<AsiapayOrderStatus> for AttemptStatus` (PSync semantics) in one
/// place: in a CAPTURE response, `Authorized_Adj` means the partial capture
/// SUCCEEDED for the echoed `amt` and the remainder was left as an adjusted
/// authorization — so the capture is Charged. Exhaustive on purpose — no `_`
/// arm — so a new `AsiapayOrderStatus` variant breaks compilation here rather
/// than being silently misclassified.
fn map_capture_attempt_status(status: AsiapayOrderStatus) -> AttemptStatus {
    match status {
        // Full capture completed.
        AsiapayOrderStatus::Captured => AttemptStatus::Charged,
        // Partial capture completed for the echoed `amt` (the response
        // handler feeds that echoed amount into the integrity object).
        AsiapayOrderStatus::AuthorizedAdj => AttemptStatus::Charged,
        // The payment's funds already moved (accepted for settlement, or even
        // refunded before this capture) — the charge stands as-is.
        AsiapayOrderStatus::Accepted
        | AsiapayOrderStatus::AcceptedAdj
        | AsiapayOrderStatus::Refunded
        | AsiapayOrderStatus::PartialRefunded => AttemptStatus::Charged,
        // Capture accepted by the gateway but not yet settled.
        AsiapayOrderStatus::Capturing => AttemptStatus::Pending,
        // The order is still merely authorized, in a non-capture in-flight
        // state, or unrecognised: the capture did not take effect but is not
        // proven failed — a later PSync reconciles.
        AsiapayOrderStatus::Authorized
        | AsiapayOrderStatus::Pending
        | AsiapayOrderStatus::PendingFeedback
        | AsiapayOrderStatus::Voiding
        | AsiapayOrderStatus::RequestRefund
        | AsiapayOrderStatus::RequestPartialRefund
        // Pending-safe: an unrecognised status is not proof of failure.
        | AsiapayOrderStatus::Unknown => AttemptStatus::Pending,
        // A 3DS challenge is still outstanding on the order.
        AsiapayOrderStatus::Pending3d => AttemptStatus::AuthenticationPending,
        // The authorization was voided/reversed instead of captured.
        AsiapayOrderStatus::Voided | AsiapayOrderStatus::ReversalAuth => AttemptStatus::Voided,
        // The connector itself reports the order rejected: a terminal,
        // capture-specific failure (never the generic `Failure`).
        AsiapayOrderStatus::Rejected => AttemptStatus::CaptureFailed,
    }
}

/// Wire response of the Merchant API Capture. The body is URL-encoded
/// (`resultCode=0&orderStatus=Captured&ref=...&payRef=...&amt=...&cur=...`),
/// parseable by `serde_urlencoded`; it reaches this struct as JSON via the
/// connector's `preprocess_response_bytes`. Every field is `Option` on
/// purpose: a failure body (`resultCode=-1&errMsg=...`) omits the success
/// keys and must still deserialize.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsiapayCaptureResponse {
    /// `0` = capture succeeded; any other value (e.g. `-1`) = failed.
    #[serde(rename = "resultCode")]
    pub result_code: Option<String>,
    #[serde(rename = "orderStatus")]
    pub order_status: Option<AsiapayOrderStatus>,
    /// Echo of the merchant order reference (`orderRef`).
    #[serde(rename = "ref")]
    pub order_ref: Option<String>,
    /// Echo of AsiaPay's payment-gateway reference (the captured `payRef`).
    #[serde(rename = "payRef")]
    pub pay_ref: Option<String>,
    /// Major-unit amount string actually captured (e.g. "80.00"); on a
    /// partial capture (`Authorized_Adj`) it is below the authorized amount.
    pub amt: Option<String>,
    /// Numeric ISO-4217 currency code (e.g. "344").
    pub cur: Option<String>,
    #[serde(rename = "errMsg")]
    pub err_msg: Option<String>,
}

impl TryFrom<ResponseRouterData<AsiapayCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AsiapayCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let router_data = item.router_data;
        let http_code = item.http_code;

        // `resultCode = "0"` is the authoritative success signal of the
        // merchant API; the endpoint answers HTTP 200 for failures too.
        // `orderStatus` absent → the operation outcome is unknown → Pending.
        let capture_succeeded = response.result_code.as_deref() == Some("0");
        let status = response
            .order_status
            .map(map_capture_attempt_status)
            .unwrap_or(AttemptStatus::Pending);

        // Keep the raw connector outcome for observability.
        let raw_connector_status = RawConnectorStatus {
            code: response.result_code.clone(),
            message: response.err_msg.clone(),
            reason: None,
        };
        let pay_ref = response.pay_ref.clone();

        // IN-BAND FAILURE: resultCode != "0", or an order status the connector
        // itself reports as terminally rejected (Rejected → CaptureFailed,
        // caught by is_payment_failure). A merchant-API capture rejection is
        // NOT terminal for the payment: the authorization still stands and the
        // capture can be retried or reconciled, so `attempt_status` carries
        // the MAPPED status (Pending for a plain resultCode failure), not a
        // forced failure.
        if !capture_succeeded || domain_types::utils::is_payment_failure(status) {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    raw_connector_status: Some(raw_connector_status),
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    // Never `.unwrap_or_default()`: the NO_ERROR_* sentinels
                    // keep a missing code/message distinguishable from "".
                    code: response
                        .result_code
                        .clone()
                        .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                    message: response
                        .err_msg
                        .clone()
                        .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                    reason: response.err_msg.clone(),
                    status_code: http_code,
                    // Flow-aware: the status derived from THIS response.
                    attempt_status: Some(FlowStatus::Payment(status)),
                    connector_transaction_id: pay_ref,
                    ..Default::default()
                }),
                ..router_data
            });
        }

        // ---- success / pending ----
        // Integrity: prefer the connector-echoed amt/cur. On a partial capture
        // (`Authorized_Adj`) the echoed `amt` is the amount ACTUALLY captured,
        // which is exactly what the integrity object must record. Fallbacks on
        // contract violation mirror Authorize/PSync: unknown cur-code → request
        // currency; absent amt → the requested capture amount.
        let currency = response
            .cur
            .as_deref()
            .and_then(crate::utils::currency_from_iso_4217_numeric)
            .unwrap_or(router_data.request.currency);
        let amount_to_capture = match response.amt.as_deref() {
            Some(amt) => parse_echoed_amount(amt, currency, http_code)?,
            None => router_data.request.capture_money.amount(),
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                raw_connector_status: Some(raw_connector_status),
                ..router_data.resource_common_data.clone()
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                // payRef echo = the reference we captured against; by contract
                // present on success. Fall back to the carrier we sent (the
                // request transformer proved it is a ConnectorTransactionId)
                // rather than severing the id chain.
                resource_id: pay_ref
                    .map(ResponseId::ConnectorTransactionId)
                    .unwrap_or_else(|| router_data.request.connector_transaction_id.clone()),
                redirection_data: None,
                connector_metadata: None,
                mandate_reference: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: response.order_ref.clone(),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: http_code,
                payment_account_reference: None,
            }),
            request: PaymentsCaptureData {
                integrity_object: Some(CaptureIntegrityObject {
                    amount_to_capture,
                    currency,
                }),
                ..router_data.request.clone()
            },
            ..router_data
        })
    }
}

// ============================================================================
// VOID (Merchant API, `orderApi.jsp`, `actionType=OnlineReversal`,
// application/x-www-form-urlencoded request AND response)
// ============================================================================

/// Merchant API Void request posted to `{base}/eng/merchant/api/orderApi.jsp`.
/// Reverses a not-yet-settled payment (an authorize-hold, `payType=H`, or a
/// same-day sale), keyed by AsiaPay's `payRef`. Only valid BEFORE AsiaPay's
/// daily settlement cut-off — after settlement the gateway rejects the void
/// and the remedy is a Refund. serde renames match the wire parameter names
/// exactly (the body is URL-encoded from this struct).
#[derive(Debug, Serialize)]
pub struct AsiapayVoidRequest {
    #[serde(rename = "merchantId")]
    pub merchant_id: Secret<String>,
    #[serde(rename = "loginId")]
    pub login_id: Secret<String>,
    #[serde(rename = "password")]
    pub password: Secret<String>,
    #[serde(rename = "actionType")]
    pub action_type: AsiapayActionType,
    #[serde(rename = "payRef")]
    pub pay_ref: String,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AsiapayRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for AsiapayVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: AsiapayRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = AsiapayAuthType::try_from(&router_data.connector_config)?;

        // payRef SOURCE RULE: the Void is keyed ONLY by the payment-gateway
        // reference AsiaPay issued at authorize time, carried into Void on
        // `PaymentVoidData::connector_transaction_id`. Never re-derive or
        // synthesize a reference — an empty carrier is a hard
        // MissingRequiredField, not a silent `payRef=`.
        let pay_ref = router_data.request.connector_transaction_id.clone();
        if pay_ref.trim().is_empty() {
            return Err(error_stack::report!(
                IntegrationError::MissingRequiredField {
                    field_name: "connector_transaction_id",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "asiapay Void requires the payRef returned by the original \
                             payment, carried on connector_transaction_id, which is empty \
                             for this void request."
                                .to_string(),
                        ),
                        suggested_action: Some(
                            "Pass the original payment's connector_transaction_id (AsiaPay \
                             payRef) into the void request. Note AsiaPay only accepts a Void \
                             before its settlement cut-off; after settlement, issue a Refund."
                                .to_string(),
                        ),
                        doc_url: None,
                    },
                }
            ));
        }

        Ok(Self {
            merchant_id: auth.merchant_id,
            login_id: auth.login_id,
            password: auth.password,
            action_type: AsiapayActionType::OnlineReversal,
            pay_ref,
        })
    }
}

/// Void-context mapping of AsiaPay's `orderStatus`. Reuses the shared
/// `AsiapayOrderStatus` wire enum; per the tech spec, a successful Void
/// echoes `Voided` — or `Reversal-Auth` when the voided payment was an
/// authorize-hold (`payType=H`): `Reversal-Auth` is AsiaPay's name for an
/// authorization reversal, which for UCS purposes IS a completed void.
/// Exhaustive on purpose — no `_` arm — so a new `AsiapayOrderStatus`
/// variant breaks compilation here rather than being silently misclassified.
fn map_void_attempt_status(status: AsiapayOrderStatus) -> AttemptStatus {
    match status {
        // Void completed (see the enum-level comment for `Reversal-Auth`).
        AsiapayOrderStatus::Voided | AsiapayOrderStatus::ReversalAuth => AttemptStatus::Voided,
        // Void accepted by the gateway but not yet applied.
        AsiapayOrderStatus::Voiding => AttemptStatus::Pending,
        // The connector itself reports the order rejected: a terminal,
        // void-specific failure (never the generic `Failure`).
        AsiapayOrderStatus::Rejected => AttemptStatus::VoidFailed,
        // Every payment/capture/refund/in-flight state a Void response is not
        // expected to echo: the void is not proven done, but not proven failed
        // either — stay non-terminal; PSync reconciles the true state.
        // Unrecognised values land here too: unknown is not proof of failure.
        AsiapayOrderStatus::Accepted
        | AsiapayOrderStatus::Captured
        | AsiapayOrderStatus::AcceptedAdj
        | AsiapayOrderStatus::Authorized
        | AsiapayOrderStatus::AuthorizedAdj
        | AsiapayOrderStatus::Pending
        | AsiapayOrderStatus::PendingFeedback
        | AsiapayOrderStatus::Capturing
        | AsiapayOrderStatus::RequestRefund
        | AsiapayOrderStatus::RequestPartialRefund
        | AsiapayOrderStatus::Refunded
        | AsiapayOrderStatus::PartialRefunded
        | AsiapayOrderStatus::Unknown => AttemptStatus::Pending,
        // A 3DS challenge is still outstanding on the order.
        AsiapayOrderStatus::Pending3d => AttemptStatus::AuthenticationPending,
    }
}

/// Wire response of the Merchant API Void. The body is URL-encoded
/// (`resultCode=0&orderStatus=Voided&ref=...&payRef=...&amt=...&cur=...`),
/// parseable by `serde_urlencoded`; it reaches this struct as JSON via the
/// connector's `preprocess_response_bytes`. Every field is `Option` on
/// purpose: a failure body (`resultCode=-1&errMsg=...`) omits the success
/// keys and must still deserialize.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsiapayVoidResponse {
    /// `0` = void succeeded; any other value (e.g. `-1`) = failed.
    #[serde(rename = "resultCode")]
    pub result_code: Option<String>,
    #[serde(rename = "orderStatus")]
    pub order_status: Option<AsiapayOrderStatus>,
    /// Echo of the merchant order reference (`orderRef`).
    #[serde(rename = "ref")]
    pub order_ref: Option<String>,
    /// Echo of AsiaPay's payment-gateway reference (the voided `payRef`).
    #[serde(rename = "payRef")]
    pub pay_ref: Option<String>,
    /// Major-unit amount string of the voided payment (e.g. "80.00").
    pub amt: Option<String>,
    /// Numeric ISO-4217 currency code (e.g. "344").
    pub cur: Option<String>,
    #[serde(rename = "errMsg")]
    pub err_msg: Option<String>,
}

impl TryFrom<ResponseRouterData<AsiapayVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<AsiapayVoidResponse, Self>) -> Result<Self, Self::Error> {
        let response = item.response;
        let router_data = item.router_data;
        let http_code = item.http_code;

        // `resultCode = "0"` is the authoritative success signal of the
        // merchant API; the endpoint answers HTTP 200 for failures too.
        // `orderStatus` absent → the void outcome is unknown → Pending.
        let status = response
            .order_status
            .map(map_void_attempt_status)
            .unwrap_or(AttemptStatus::Pending);

        // Keep the raw connector outcome for observability.
        let raw_connector_status = RawConnectorStatus {
            code: response.result_code.clone(),
            message: response.err_msg.clone(),
            reason: None,
        };

        // The request transformer proved this carrier non-empty; bind it
        // before the struct literals move `router_data`.
        let sent_pay_ref = router_data.request.connector_transaction_id.clone();

        // IN-BAND FAILURE: the merchant API answers HTTP 200 for failures too;
        // a PRESENT, non-`0` `resultCode` is the authoritative failure signal
        // (typical cause: the payment already passed AsiaPay's settlement
        // cut-off — after which only a Refund can reverse it). A MISSING
        // `resultCode` is NOT failure on its own (sandbox quirks drop keys).
        // A connector-reported Rejected orderStatus maps to VoidFailed above
        // and is caught here by is_payment_failure.
        let void_failed = response
            .result_code
            .as_deref()
            .is_some_and(|result_code| result_code != "0")
            || domain_types::utils::is_payment_failure(status);
        if void_failed {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    // A failed void is VOID-specific terminal: the gateway
                    // explicitly reports the void did not happen, while the
                    // payment itself is untouched (the authorization survives)
                    // — so `VoidFailed`, never the generic `Failure`, and
                    // never `Pending` (AsiaPay has told us the void failed;
                    // pretending one is still in flight would be dishonest).
                    status: AttemptStatus::VoidFailed,
                    raw_connector_status: Some(raw_connector_status),
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    // Never `.unwrap_or_default()`: the NO_ERROR_* sentinels
                    // keep a missing code/message distinguishable from "".
                    code: response
                        .result_code
                        .clone()
                        .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                    message: response
                        .err_msg
                        .clone()
                        .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                    reason: response.err_msg.clone(),
                    status_code: http_code,
                    // Flow-aware: VoidFailed is the status this response
                    // proves — the void did not take effect; the payment's
                    // prior state (authorized / charged) stands.
                    attempt_status: Some(FlowStatus::Payment(AttemptStatus::VoidFailed)),
                    // Echoed payRef wins; the sent carrier is the proven
                    // non-empty fallback, so this is never an empty id.
                    connector_transaction_id: Some(
                        response.pay_ref.clone().unwrap_or(sent_pay_ref),
                    ),
                    ..Default::default()
                }),
                ..router_data
            });
        }

        // ---- success / pending ----
        // `Reversal-Auth` note: an authorize-hold reverse surfaces with that
        // orderStatus; `map_void_attempt_status` already mapped it to Voided.
        // Integrity: `PaymentVoidIntegrityObject` carries only the transaction
        // id — prefer the connector-echoed payRef, fall back to the proven
        // non-empty carrier we sent. The echoed amt/cur have no slot on the
        // void integrity object, so they complete the request's OPTIONAL
        // amount/currency instead (the voided payment's amount): echoed values
        // win; the request's own values are the fallback, matching how
        // Authorize/PSync/Capture mirror on contract violation.
        let currency = response
            .cur
            .as_deref()
            .and_then(crate::utils::currency_from_iso_4217_numeric)
            .or(router_data.request.currency);
        let voided_money = match (response.amt.as_deref(), currency) {
            (Some(amt), Some(currency)) => Some(parse_echoed_money(amt, currency, http_code)?),
            // amt absent (or no usable currency to convert it against):
            // mirror the void request's Money (may itself be None).
            _ => router_data.request.money.clone(),
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                raw_connector_status: Some(raw_connector_status),
                ..router_data.resource_common_data.clone()
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                // payRef echo = the reference we voided against; by contract
                // present on success. Fall back to the carrier we sent rather
                // than severing the id chain.
                resource_id: response
                    .pay_ref
                    .clone()
                    .map(ResponseId::ConnectorTransactionId)
                    .unwrap_or_else(|| ResponseId::ConnectorTransactionId(sent_pay_ref.clone())),
                redirection_data: None,
                connector_metadata: None,
                mandate_reference: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: response.order_ref.clone(),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: http_code,
                payment_account_reference: None,
            }),
            request: PaymentVoidData {
                money: voided_money,
                currency,
                integrity_object: Some(PaymentVoidIntegrityObject {
                    connector_transaction_id: response.pay_ref.clone().unwrap_or(sent_pay_ref),
                }),
                ..router_data.request.clone()
            },
            ..router_data
        })
    }
}

// ============================================================================
// REFUND (Merchant API, `orderApi.jsp`, `actionType=RequestRefund`,
// application/x-www-form-urlencoded request AND response)
// ============================================================================

/// Merchant API Refund request posted to `{base}/eng/merchant/api/orderApi.jsp`.
/// Refunds (fully or partially) a settled payment, keyed by AsiaPay's `payRef`.
/// serde renames match the wire parameter names exactly (the body is
/// URL-encoded from this struct).
#[derive(Debug, Serialize)]
pub struct AsiapayRefundRequest {
    #[serde(rename = "merchantId")]
    pub merchant_id: Secret<String>,
    #[serde(rename = "loginId")]
    pub login_id: Secret<String>,
    #[serde(rename = "password")]
    pub password: Secret<String>,
    #[serde(rename = "actionType")]
    pub action_type: AsiapayActionType,
    #[serde(rename = "payRef")]
    pub pay_ref: String,
    /// Major-unit string ("80.00"), ≤ the captured amount (partial refunds are
    /// a first-class AsiaPay feature).
    pub amount: StringMajorUnit,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AsiapayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for AsiapayRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: AsiapayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = AsiapayAuthType::try_from(&router_data.connector_config)?;

        // payRef SOURCE RULE: the Refund is keyed ONLY by the payment-gateway
        // reference AsiaPay issued at authorize time, carried into Refund on
        // `RefundsData::connector_transaction_id`. Never re-derive or
        // synthesize a reference — an empty carrier is a hard
        // MissingRequiredField, not a silent `payRef=`.
        let pay_ref = router_data.request.connector_transaction_id.clone();
        if pay_ref.trim().is_empty() {
            return Err(error_stack::report!(
                IntegrationError::MissingRequiredField {
                    field_name: "connector_transaction_id",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "asiapay Refund (RequestRefund) requires the payRef returned by the \
                             original payment, carried on connector_transaction_id, which is \
                             empty for this refund request."
                                .to_string(),
                        ),
                        suggested_action: Some(
                            "Pass the original payment's connector_transaction_id (AsiaPay \
                             payRef) into the refund request."
                                .to_string(),
                        ),
                        doc_url: None,
                    },
                }
            ));
        }

        // The amount being refunded NOW (`refund_money`), which may be below
        // the captured amount — partial refunds are allowed by the gateway.
        let amount = AsiapayAmountConvertor::convert(&router_data.request.refund_money)?;

        Ok(Self {
            merchant_id: auth.merchant_id,
            login_id: auth.login_id,
            password: auth.password,
            action_type: AsiapayActionType::RequestRefund,
            pay_ref,
            amount,
        })
    }
}

/// Refund-context mapping of AsiaPay's `orderStatus`. Reuses the shared
/// `AsiapayOrderStatus` wire enum; per the tech spec, a RequestRefund response
/// normally echoes `RequestRefund` / `RequestPartialRefund` — the refund is
/// ACCEPTED and settles asynchronously (RSync observes the terminal state) —
/// while a synchronously settled refund echoes `Refunded` / `PartialRefunded`.
/// Exhaustive on purpose — no `_` arm — so a new `AsiapayOrderStatus` variant
/// breaks compilation here rather than being silently misclassified.
fn map_refund_status(status: AsiapayOrderStatus) -> RefundStatus {
    match status {
        // Refund settled synchronously.
        AsiapayOrderStatus::Refunded | AsiapayOrderStatus::PartialRefunded => RefundStatus::Success,
        // Reversal-style refund, completed synchronously: a full-amount refund
        // on a still-unsettled payment reverses the transaction instead of
        // issuing a separate refund record — `resultCode=0` +
        // `orderStatus=Voided` + "Transaction was voided successfully"
        // (observed live). `Reversal-Auth` is the equivalent completed auth
        // reversal for `payType=H`. Both are terminal refund successes; no
        // later `Refunded` state follows, so Pending here would hang the
        // refund forever.
        AsiapayOrderStatus::Voided | AsiapayOrderStatus::ReversalAuth => RefundStatus::Success,
        // Refund accepted by the gateway, settling asynchronously — the normal
        // success outcome of `actionType=RequestRefund`.
        AsiapayOrderStatus::RequestRefund | AsiapayOrderStatus::RequestPartialRefund => {
            RefundStatus::Pending
        }
        // The connector itself reports the refund rejected: a terminal,
        // refund-specific failure.
        AsiapayOrderStatus::Rejected => RefundStatus::Failure,
        // Payment-side states a Refund response is not expected to echo, and
        // any unrecognised value: the refund is not proven accepted, but not
        // proven failed either — stay non-terminal; RSync reconciles.
        AsiapayOrderStatus::Accepted
        | AsiapayOrderStatus::Captured
        | AsiapayOrderStatus::AcceptedAdj
        | AsiapayOrderStatus::Authorized
        | AsiapayOrderStatus::AuthorizedAdj
        | AsiapayOrderStatus::Pending
        | AsiapayOrderStatus::PendingFeedback
        | AsiapayOrderStatus::Capturing
        | AsiapayOrderStatus::Voiding
        | AsiapayOrderStatus::Pending3d
        | AsiapayOrderStatus::Unknown => RefundStatus::Pending,
    }
}

/// Wire response of the Merchant API Refund (RequestRefund). The body is
/// URL-encoded (`resultCode=0&orderStatus=RequestRefund&ref=...&payRef=...&amt
/// =...&cur=...`), parseable by `serde_urlencoded`; it reaches this struct as
/// JSON via the connector's `preprocess_response_bytes`. Every field is
/// `Option` on purpose: a failure body (`resultCode=-1&errMsg=...`) omits the
/// success keys and must still deserialize.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsiapayRefundResponse {
    /// `0` = refund accepted; any other value (e.g. `-1`) = failed.
    #[serde(rename = "resultCode")]
    pub result_code: Option<String>,
    #[serde(rename = "orderStatus")]
    pub order_status: Option<AsiapayOrderStatus>,
    /// Echo of the merchant order reference (`orderRef`).
    #[serde(rename = "ref")]
    pub order_ref: Option<String>,
    /// Echo of AsiaPay's payment-gateway reference (the refunded `payRef`).
    #[serde(rename = "payRef")]
    pub pay_ref: Option<String>,
    /// Major-unit amount string being refunded (e.g. "80.00"); on a partial
    /// refund it is below the captured amount.
    pub amt: Option<String>,
    /// Numeric ISO-4217 currency code (e.g. "344").
    pub cur: Option<String>,
    #[serde(rename = "errMsg")]
    pub err_msg: Option<String>,
}

impl TryFrom<ResponseRouterData<AsiapayRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AsiapayRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let router_data = item.router_data;
        let http_code = item.http_code;

        // `orderStatus` absent → the refund outcome is unknown → Pending
        // (quirky-but-errorless responses are never failed by the mapper).
        let refund_status = response
            .order_status
            .map(map_refund_status)
            .unwrap_or(RefundStatus::Pending);

        // Keep the raw connector outcome for observability.
        let raw_connector_status = RawConnectorStatus {
            code: response.result_code.clone(),
            message: response.err_msg.clone(),
            reason: None,
        };

        // The request transformer proved this carrier non-empty; bind it
        // before the struct literals move `router_data`.
        let payment_pay_ref = router_data.request.connector_transaction_id.clone();

        // IN-BAND FAILURE: the merchant API answers HTTP 200 for failures
        // too; a PRESENT, non-`0` `resultCode` is the authoritative failure
        // signal. A MISSING `resultCode` is NOT failure on its own (sandbox
        // quirks drop keys) — the refund then keeps its mapped (typically
        // Pending) status and RSync reconciles. A refund the connector itself
        // reports Rejected maps to RefundStatus::Failure above and is caught
        // here by is_refund_failure.
        let refund_failed = response
            .result_code
            .as_deref()
            .is_some_and(|result_code| result_code != "0")
            || crate::utils::is_refund_failure(refund_status);
        if refund_failed {
            return Ok(Self {
                resource_common_data: RefundFlowData {
                    status: refund_status,
                    raw_connector_status: Some(raw_connector_status),
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    // Never `.unwrap_or_default()`: the NO_ERROR_* sentinels
                    // keep a missing code/message distinguishable from "".
                    code: response
                        .result_code
                        .clone()
                        .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                    message: response
                        .err_msg
                        .clone()
                        .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                    reason: response.err_msg.clone(),
                    status_code: http_code,
                    // Flow-aware, non-terminal by default: a RequestRefund
                    // rejection does not prove the refund can never be
                    // submitted, so this carries the MAPPED status — Pending
                    // for a plain resultCode failure; only a
                    // connector-reported Rejected terminals the refund.
                    attempt_status: Some(FlowStatus::Refund(refund_status)),
                    connector_transaction_id: Some(payment_pay_ref),
                    ..Default::default()
                }),
                ..router_data
            });
        }

        // ---- success / pending ----
        // REFUND ID CHOICE: AsiaPay issues no distinct refund identifier —
        // the RequestRefund response echoes only the PAYMENT's `payRef`.
        // `connector_refund_id` therefore carries that echoed payRef (falling
        // back to the request's carrier); RSync keys the refund by the same
        // payRef, so the id chain does not break. The `ref` echo is the
        // merchant's own orderRef — already known to UCS as
        // `connector_request_reference_id` — and `RefundsResponseData` has no
        // connector_response_reference_id carrier, so it is not surfaced.
        let connector_refund_id = response.pay_ref.clone().unwrap_or(payment_pay_ref);

        // Integrity: prefer the connector-echoed amt/cur (major → minor via
        // the connector's converter). On a partial refund the echoed `amt` is
        // the amount ACTUALLY refunded, which is exactly what the integrity
        // object must record. Fallbacks mirror Authorize/PSync/Capture:
        // unknown cur-code → request currency; absent amt → the requested
        // refund amount.
        let currency = response
            .cur
            .as_deref()
            .and_then(crate::utils::currency_from_iso_4217_numeric)
            .unwrap_or(router_data.request.currency);
        let refund_amount = match response.amt.as_deref() {
            Some(amt) => parse_echoed_amount(amt, currency, http_code)?,
            None => router_data.request.refund_money.amount(),
        };

        Ok(Self {
            resource_common_data: RefundFlowData {
                status: refund_status,
                raw_connector_status: Some(raw_connector_status),
                ..router_data.resource_common_data.clone()
            },
            // `RefundsResponseData` is a plain struct of four fields — every
            // one is listed (no functional-update syntax).
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: http_code,
                // AsiaPay's merchant API echoes no acquirer reference number.
                acquirer_reference_number: None,
            }),
            request: RefundsData {
                integrity_object: Some(RefundIntegrityObject {
                    refund_amount,
                    currency,
                }),
                ..router_data.request.clone()
            },
            ..router_data
        })
    }
}

// ============================================================================
// RSYNC (Merchant API Query, `orderApi.jsp`, `actionType=Query` keyed by
// `payRef`; application/x-www-form-urlencoded request, XML response)
// ============================================================================

/// Merchant API Query request posted to `{base}/eng/merchant/api/orderApi.jsp`,
/// keyed by `payRef` — AsiaPay's Query accepts orderRef XOR payRef, and for a
/// refund sync payRef keys the precise payment the refund applies to. AsiaPay
/// issues no distinct refund identifier: the Refund response stores the
/// payment's payRef on `connector_refund_id`, so that carrier is the Query
/// key here.
#[derive(Debug, Serialize)]
pub struct AsiapayRSyncRequest {
    #[serde(rename = "merchantId")]
    pub merchant_id: Secret<String>,
    #[serde(rename = "loginId")]
    pub login_id: Secret<String>,
    #[serde(rename = "password")]
    pub password: Secret<String>,
    #[serde(rename = "actionType")]
    pub action_type: AsiapayActionType,
    #[serde(rename = "payRef")]
    pub pay_ref: String,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AsiapayRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for AsiapayRSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: AsiapayRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = AsiapayAuthType::try_from(&router_data.connector_config)?;

        // payRef SOURCE RULE: the refund-sync Query is keyed ONLY by the
        // refund's own carrier — `RefundSyncData::connector_refund_id`, which
        // holds the payRef our Refund response published. Never re-derive or
        // synthesize a lookup key and never fall back to a re-computed id —
        // an empty carrier is a hard MissingRequiredField, not a silent
        // `payRef=`.
        let pay_ref = router_data.request.connector_refund_id.clone();
        if pay_ref.trim().is_empty() {
            return Err(error_stack::report!(
                IntegrationError::MissingRequiredField {
                    field_name: "connector_refund_id",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "asiapay RSync (Query by payRef) requires the connector_refund_id \
                             returned by the original Refund call (AsiaPay issues no distinct \
                             refund id, so the refund carrier holds the payment's payRef), \
                             which is empty for this refund sync request."
                                .to_string(),
                        ),
                        suggested_action: Some(
                            "Pass the connector_refund_id surfaced by the asiapay Refund \
                             response (the payment's payRef) into the refund sync request."
                                .to_string(),
                        ),
                        doc_url: None,
                    },
                }
            ));
        }

        Ok(Self {
            merchant_id: auth.merchant_id,
            login_id: auth.login_id,
            password: auth.password,
            action_type: AsiapayActionType::Query,
            pay_ref,
        })
    }
}

/// RSync-context mapping of AsiaPay's `orderStatus` (tech spec §6, Response
/// status drives refund status). Reuses the shared `AsiapayOrderStatus` wire
/// enum but deliberately differs from `map_refund_status` (Refund semantics)
/// in one place: in an RSYNC response, `Accepted_Adj` means the completed
/// transaction was partially refunded and that partial refund has SETTLED —
/// so the refund is Success, not Pending. Exhaustive on purpose — no `_` arm —
/// so a new `AsiapayOrderStatus` variant breaks compilation here rather than
/// being silently misclassified.
fn map_refund_sync_status(status: AsiapayOrderStatus) -> RefundStatus {
    match status {
        // Refund settled. `Accepted_Adj` specifically: the transaction
        // completed and was refunded partial — the adjusted (refunded)
        // portion is settled, so the refund succeeded.
        AsiapayOrderStatus::Refunded
        | AsiapayOrderStatus::PartialRefunded
        | AsiapayOrderStatus::AcceptedAdj => RefundStatus::Success,
        // Reversal-style refund: a full-amount refund on an unsettled payment
        // voids/reverses the transaction instead of producing a `Refunded`
        // state (observed on the Refund call itself:
        // `orderStatus=Voided` + "Transaction was voided successfully").
        // Query echoes `Voided`/`Reversal-Auth` forever afterwards — mapping
        // it to Pending would leave the refund unresolved permanently.
        AsiapayOrderStatus::Voided | AsiapayOrderStatus::ReversalAuth => RefundStatus::Success,
        // Refund request accepted and settling asynchronously, an adjacent
        // capture/void in flight on the same order, or a plain payment
        // pending state — the refund settlement is still pending.
        AsiapayOrderStatus::RequestRefund
        | AsiapayOrderStatus::RequestPartialRefund
        | AsiapayOrderStatus::Capturing
        | AsiapayOrderStatus::Voiding
        | AsiapayOrderStatus::Pending
        | AsiapayOrderStatus::PendingFeedback
        // A 3DS challenge on the underlying payment is still outstanding —
        // the refund cannot settle until the payment does.
        | AsiapayOrderStatus::Pending3d => RefundStatus::Pending,
        // The connector itself reports the refund rejected: a terminal,
        // refund-specific failure.
        AsiapayOrderStatus::Rejected => RefundStatus::Failure,
        // Payment statuses that OUTLIVE the refund ask (the payment sits in
        // a completed/authorized state while the refund request has not
        // surfaced as Refunded/PartialRefunded yet): the refund settlement is
        // still pending — stay non-terminal so a later RSync reconciles.
        AsiapayOrderStatus::Accepted
        | AsiapayOrderStatus::Captured
        | AsiapayOrderStatus::Authorized
        | AsiapayOrderStatus::AuthorizedAdj
        // Pending-safe: an unrecognised status is not proof of failure.
        | AsiapayOrderStatus::Unknown => RefundStatus::Pending,
    }
}

/// Wire response of the Merchant API Query when polled from RSync — the SAME
/// Same `<records><record>…</record></records>` envelope as PSync,
/// deserialized directly by quick-xml via the `response_format: xml`
/// prerequisite in asiapay.rs. A dedicated struct per the repo's connector-
/// named response-type rule (NOT a type alias of `AsiapayPSyncResponse`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsiapayRSyncResponse {
    pub record: AsiapayRSyncRecord,
}

/// The result tags carried inside `<record>`; only the tags the integration
/// needs are modelled and AsiaPay's card-info / MPS / VAS blocks are ignored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsiapayRSyncRecord {
    #[serde(rename = "merchantId")]
    pub merchant_id: Option<String>,
    #[serde(rename = "errMsg")]
    pub err_msg: Option<String>,
    #[serde(rename = "orderStatus")]
    pub order_status: Option<AsiapayOrderStatus>,
    /// Echo of the merchant order reference (`orderRef`).
    #[serde(rename = "ref")]
    pub order_ref: Option<String>,
    /// AsiaPay's payment-gateway reference — the payRef the refund applies to
    /// (and, for this connector, the refund carrier itself).
    #[serde(rename = "payRef")]
    pub pay_ref: Option<String>,
    #[serde(rename = "mpsMode")]
    pub mps_mode: Option<String>,
    /// Major-unit amount string (e.g. "80.00").
    pub amt: Option<String>,
    /// Numeric ISO-4217 currency code (e.g. "344").
    pub cur: Option<String>,
    pub prc: Option<String>,
    pub src: Option<String>,
    pub ord: Option<String>,
    pub holder: Option<String>,
    #[serde(rename = "authId")]
    pub auth_id: Option<String>,
    #[serde(rename = "txTime")]
    pub tx_time: Option<String>,
    pub eci: Option<String>,
    #[serde(rename = "channelType")]
    pub channel_type: Option<String>,
    #[serde(rename = "alertCode")]
    pub alert_code: Option<String>,
}

impl TryFrom<ResponseRouterData<AsiapayRSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<AsiapayRSyncResponse, Self>) -> Result<Self, Self::Error> {
        let response = item.response.record;
        let router_data = item.router_data;
        let http_code = item.http_code;

        // No orderStatus at all → the Query itself failed in-band (only
        // merchantId+errMsg come back); keep it non-terminal.
        let refund_status = response
            .order_status
            .map(map_refund_sync_status)
            .unwrap_or(RefundStatus::Pending);

        // Keep the raw connector outcome for observability.
        let raw_connector_status = RawConnectorStatus {
            code: response.prc.clone(),
            message: response.err_msg.clone(),
            reason: response.src.clone(),
        };

        // The request transformer proved this carrier non-empty; bind it
        // (and the payment-side id) before the struct literals move
        // `router_data`.
        let sent_pay_ref = router_data.request.connector_refund_id.clone();
        let payment_pay_ref = {
            let transaction_id = router_data.request.connector_transaction_id.clone();
            if transaction_id.trim().is_empty() {
                sent_pay_ref.clone()
            } else {
                transaction_id
            }
        };

        // IN-BAND FAILURE: the Query endpoint answers HTTP 200 for "order not
        // found", bad credentials, and rejected refunds alike. Failure = an
        // explicit errMsg other than the success sentinel, or a status the
        // connector itself reports as Rejected (mapped above to Failure).
        let query_errored = is_query_err_msg(response.err_msg.as_deref());
        if query_errored || crate::utils::is_refund_failure(refund_status) {
            return Ok(Self {
                resource_common_data: RefundFlowData {
                    status: refund_status,
                    raw_connector_status: Some(raw_connector_status),
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    // prc identifies the bank/payment outcome; src the system
                    // component. Never `.unwrap_or_default()`: the NO_ERROR_*
                    // sentinels keep a missing code/message distinguishable
                    // from "".
                    code: response
                        .prc
                        .clone()
                        .or_else(|| response.src.clone())
                        .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                    message: response
                        .err_msg
                        .clone()
                        .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                    reason: response.err_msg.clone(),
                    status_code: http_code,
                    // Flow-aware: RSync reports a refund status, so the tag is
                    // FlowStatus::Refund (never FlowStatus::Payment); the value
                    // comes from the orderStatus mapping — terminal ONLY for
                    // Rejected → Failure; an errMsg-only failure keeps the
                    // mapped status (typically Pending) so a later RSync can
                    // still resolve the refund.
                    attempt_status: Some(FlowStatus::Refund(refund_status)),
                    connector_transaction_id: Some(
                        // Echoed payRef wins; the sent carrier is the proven
                        // non-empty fallback, so this is never an empty id.
                        response.pay_ref.clone().unwrap_or(sent_pay_ref),
                    ),
                    ..Default::default()
                }),
                ..router_data
            });
        }

        // ---- success / pending ----
        // REFUND ID CHAIN: prefer the connector-echoed payRef; fall back to
        // the carrier we sent (proven non-empty by the request transformer)
        // so the id chain never breaks. AsiaPay echoes no amount/currency
        // carrier on RefundsResponseData, so the echoed amt/cur are surfaced
        // only through the raw status / integrity ids.
        let connector_refund_id = response.pay_ref.clone().unwrap_or(sent_pay_ref);

        Ok(Self {
            resource_common_data: RefundFlowData {
                status: refund_status,
                raw_connector_status: Some(raw_connector_status),
                ..router_data.resource_common_data.clone()
            },
            // `RefundsResponseData` is a plain struct of four fields — every
            // one is listed (no functional-update syntax).
            response: Ok(RefundsResponseData {
                connector_refund_id: connector_refund_id.clone(),
                refund_status,
                status_code: http_code,
                // AsiaPay's merchant API echoes no acquirer reference number.
                acquirer_reference_number: None,
            }),
            request: RefundSyncData {
                // Integrity: the Query response's identity anchors. AsiaPay
                // issues no distinct refund id — the refund carrier IS the
                // payment's payRef — so both ids prefer the echoed payRef and
                // fall back to the ids the sync request carried.
                integrity_object: Some(RefundSyncIntegrityObject {
                    connector_transaction_id: response.pay_ref.clone().unwrap_or(payment_pay_ref),
                    connector_refund_id,
                }),
                ..router_data.request.clone()
            },
            ..router_data
        })
    }
}

// ============================================================================
// WEBHOOK (AsiaPay "Data Feed" — inbound gateway → UCS notification, POST
// application/x-www-form-urlencoded; tech spec §7)
// ============================================================================

/// AsiaPay "Data Feed" notification body — the server-to-server POST AsiaPay
/// makes to the merchant's registered Data Feed URL with the transaction
/// result. The body is URL-encoded (`k=v&...`), so it is parsed with
/// `serde_urlencoded`, consistent with this connector's other URL-encoded
/// surfaces. EVERY field is `Option` on purpose: the feed also carries large
/// optional blocks the integration ignores (card info `panFirst4`/`panLast4`,
/// MPS, SchedulePay/MemberPay VAS, eWallet, promo, installment, rewards,
/// `threeDSVersion` — unknown keys are skipped by the deserializer), and a
/// degraded feed must still parse so the stateless ParseEvent phase can answer
/// "who is this about" — `verify_webhook_source` is where missing signature
/// fields become errors.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AsiapayWebhookBody {
    /// `0` = succeeded, `1` = failure, `2` = pending.
    #[serde(rename = "successcode", alias = "SuccessCode")]
    pub success_code: Option<String>,
    /// Payment result code (bank outcome) — the error-code carrier on failure.
    pub prc: Option<String>,
    /// System result code (component that produced the outcome).
    pub src: Option<String>,
    #[serde(rename = "Ord")]
    pub ord: Option<String>,
    #[serde(rename = "Holder")]
    pub holder: Option<String>,
    /// Echo of the merchant order reference (the `orderRef` sent on Authorize).
    #[serde(rename = "Ref", alias = "ref", alias = "orderRef")]
    pub order_ref: Option<String>,
    /// AsiaPay's payment-gateway reference — the `connector_transaction_id`.
    #[serde(rename = "PayRef", alias = "payRef")]
    pub pay_ref: Option<String>,
    /// Major-unit amount string (e.g. "80.00").
    #[serde(rename = "Amt", alias = "amt")]
    pub amt: Option<String>,
    /// Numeric ISO-4217 currency code (e.g. "344").
    #[serde(rename = "Cur", alias = "cur")]
    pub cur: Option<String>,
    /// Human-readable result remark — the error-message carrier on failure.
    pub remark: Option<String>,
    /// Acquirer authorization id (present when approved; surfaced as
    /// `network_txn_id`, mirroring the Authorize handler).
    #[serde(rename = "AuthId", alias = "authId")]
    pub auth_id: Option<String>,
    pub eci: Option<String>,
    /// 3-D Secure payer-authentication result (Y/N/P/A/U; with 3DS 2.0 also
    /// C/R). Part of the `secureHash` signing string.
    #[serde(rename = "payerAuth")]
    pub payer_auth: Option<String>,
    #[serde(rename = "sourceIp")]
    pub source_ip: Option<String>,
    #[serde(rename = "ipCountry")]
    pub ip_country: Option<String>,
    #[serde(rename = "payMethod")]
    pub pay_method: Option<String>,
    #[serde(rename = "cardIssuingCountry")]
    pub card_issuing_country: Option<String>,
    /// Channel the transaction came through (`DPC`).
    #[serde(rename = "channelType")]
    pub channel_type: Option<String>,
    /// The signature: hex digest of
    /// `src|prc|successcode|Ref|PayRef|Cur|Amt|payerAuth|<secure hash secret>`.
    #[serde(rename = "secureHash")]
    pub secure_hash: Option<String>,
    #[serde(rename = "AlertCode", alias = "alertCode")]
    pub alert_code: Option<String>,
    /// Merchant id the feed was issued for — cross-checked against the
    /// configured credentials during source verification.
    #[serde(rename = "MerchantId", alias = "merchantId")]
    pub merchant_id: Option<String>,
    /// AsiaPay transaction timestamp (`yyyy-MM-dd HH:mm:ss.S`). There is no
    /// transaction-time carrier on `WebhookDetailsResponse`, so it surfaces
    /// only inside `raw_connector_response`.
    #[serde(rename = "TxTime", alias = "txTime")]
    pub tx_time: Option<String>,
    /// Current order status classification (e.g. `Authorized`, `Refunded`) —
    /// drives the payment-vs-refund event-type split in
    /// `map_asiapay_webhook_event_type`.
    #[serde(rename = "orderStatus")]
    pub order_status: Option<String>,
}

/// Parse the URL-encoded data-feed body. Shared by every `IncomingWebhook`
/// member (each receives the raw bytes independently and must tolerate the
/// same degraded shapes).
pub fn parse_asiapay_webhook_body(
    bytes: &[u8],
) -> Result<AsiapayWebhookBody, error_stack::Report<WebhookError>> {
    serde_urlencoded::from_bytes(bytes)
        .map_err(|_| error_stack::report!(WebhookError::WebhookBodyDecodingFailed))
        .attach_printable(
            "asiapay: data-feed webhook body was not a valid \
         application/x-www-form-urlencoded payload",
        )
}

/// Map a parsed data-feed body to the UCS `EventType` (tech spec §7):
/// `successcode` 0 is a success whose payment-vs-refund-vs-authorization split
/// is decided by `orderStatus`; `successcode` 1 is a failure; `2` (pending), a
/// missing `successcode`, or any other value is not a UCS-modelled event.
/// Infallible on purpose — an unclassifiable feed degrades to
/// `IncomingWebhookEventUnspecified` rather than failing the stateless
/// ParseEvent phase.
pub fn map_asiapay_webhook_event_type(body: &AsiapayWebhookBody) -> EventType {
    // Exhaustive over the named AsiapaySuccessCode variants on purpose — no
    // `_ =>` arm. The body's success_code stays a raw String (the exact
    // bytes are re-signed for secureHash verification), so the typed
    // classification happens here via the enum's serde layer.
    match body
        .success_code
        .as_deref()
        .map(AsiapaySuccessCode::from_wire)
    {
        Some(AsiapaySuccessCode::Succeeded) => match body.order_status.as_deref() {
            // Note: the data feed spells the partial form with a space
            // ("Partial Refunded"), unlike the Merchant API's
            // `PartialRefunded`.
            Some("Refunded" | "Partial Refunded") => EventType::RefundSuccess,
            Some("Authorized") => EventType::PaymentIntentAuthorizationSuccess,
            // Required catch-all, enumerated as the irrefutable final pair:
            // a successcode=0 feed may carry ANY other orderStatus (Accepted,
            // Captured, Accepted_Adj, ...) or none at all — every case is a
            // completed payment. The AsiapayOrderStatus wire enum cannot be
            // reused here: its `PartialRefunded` variant does not match the
            // feed's "Partial Refunded" spelling.
            Some(_) | None => EventType::PaymentIntentSuccess,
        },
        Some(AsiapaySuccessCode::Failed) => EventType::PaymentIntentFailure,
        // `2` (pending), an unrecognised value, or a missing successcode is
        // not a UCS-modelled event.
        Some(AsiapaySuccessCode::Pending) | Some(AsiapaySuccessCode::Other) | None => {
            EventType::IncomingWebhookEventUnspecified
        }
    }
}

/// Recompute the data-feed `secureHash` for source verification.
///
/// Signing string (tech spec §7, "Data Feed Validation"):
/// `src|prc|successcode|Ref|PayRef|Cur|Amt|payerAuth|<Secure Hash Secret>`,
/// digested with the same `AsiapaySecureHashAlgorithm` the Authorize request
/// signing uses (`sign_payment_request`) — both sides come from the
/// `secure_hash_algorithm` credential and must match the merchant admin
/// panel's Secure Hash setting (SHA-1 is the gateway default). Returns
/// `WebhookError::WebhookSourceVerificationFailed` when any signing-string
/// field is absent — a feed missing a signed field cannot be verified, only
/// rejected.
pub fn compute_asiapay_webhook_secure_hash(
    body: &AsiapayWebhookBody,
    secure_hash_secret: &str,
    secure_hash_algorithm: AsiapaySecureHashAlgorithm,
) -> Result<String, error_stack::Report<WebhookError>> {
    let required = |value: &Option<String>, field_name: &'static str| {
        value
            .clone()
            .ok_or_else(|| error_stack::report!(WebhookError::WebhookSourceVerificationFailed))
            .attach_printable_lazy(|| {
                format!(
                    "asiapay: data-feed body is missing `{field_name}`, which is part of the \
                     secureHash signing string; cannot verify the webhook source"
                )
            })
    };

    let src = required(&body.src, "src")?;
    let prc = required(&body.prc, "prc")?;
    let success_code = required(&body.success_code, "successcode")?;
    let order_ref = required(&body.order_ref, "Ref")?;
    let pay_ref = required(&body.pay_ref, "PayRef")?;
    let cur = required(&body.cur, "Cur")?;
    let amt = required(&body.amt, "Amt")?;
    let payer_auth = required(&body.payer_auth, "payerAuth")?;

    let signing_string = format!(
        "{src}|{prc}|{success_code}|{order_ref}|{pay_ref}|{cur}|{amt}|{payer_auth}|{secure_hash_secret}"
    );
    Ok(secure_hash_algorithm.digest(&signing_string))
}

/// Resolve the data feed's `Amt`/`Cur` pair into `Money` (major-unit string →
/// `MinorUnit` via the connector's own StringMajorUnit converter — the same
/// inverse direction `parse_echoed_amount` uses for the flow responses).
/// `Ok(None)` when a key is absent, or when `Cur` is a code outside the
/// supported currency table: a webhook feed must never hard-fail amount
/// bookkeeping, and PSync stays the amount authority. An `Amt` that IS present
/// (with a known currency) but unparsable is genuinely corrupt →
/// `WebhookAmountConversionFailed`.
pub fn parse_asiapay_webhook_amount(
    amt: Option<&str>,
    cur: Option<&str>,
) -> Result<Option<Money>, error_stack::Report<WebhookError>> {
    let currency = match cur.and_then(crate::utils::currency_from_iso_4217_numeric) {
        Some(currency) => currency,
        None => return Ok(None),
    };
    let amt = match amt {
        Some(amt) => amt,
        None => return Ok(None),
    };
    let echoed: StringMajorUnit =
        serde_json::from_value(serde_json::Value::String(amt.to_string())).map_err(|_| {
            error_stack::report!(WebhookError::WebhookAmountConversionFailed {
                reason: format!(
                    "asiapay: data-feed Amt `{amt}` is not a valid {currency:?} major-unit \
                     amount string"
                ),
            })
        })?;
    AsiapayAmountConvertor::convert_back(echoed, currency)
        .map(Some)
        .map_err(|conversion_error| {
            error_stack::report!(WebhookError::WebhookAmountConversionFailed {
                reason: format!(
                    "asiapay: could not convert data-feed Amt `{amt}` ({conversion_error:?})"
                ),
            })
        })
}
