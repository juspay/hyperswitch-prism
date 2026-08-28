use base64::Engine;
use common_enums::{AttemptStatus, AuthenticationType, Currency, RefundStatus};
use common_utils::{
    crypto::{self, GenerateDigest},
    pii::Email,
    types::StringMinorUnit,
};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, PreAuthenticate, RSync, Refund, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSyncData, RawConnectorStatus,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, ResponseId,
    },
    errors,
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_request_types::BrowserInformation,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use time::{format_description::well_known::Rfc3339, Date, Duration, OffsetDateTime, UtcOffset};

use super::IlixiumRouterData;

pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

/// Message-level API version. The platform's `version` field is `2` for every
/// in-scope Direct API message (`/direct/auth`, `/direct/threedcomplete`).
const ILIXIUM_MESSAGE_VERSION: u8 = 2;

/// `transaction.merchantRef` is constrained to `^(?!.*£)[\w]{4,20}$`.
const MERCHANT_REF_MIN_LEN: usize = 4;
const MERCHANT_REF_MAX_LEN: usize = 20;

const CHALLENGE_WINDOW_SIZE_FULL_SCREEN: &str = "05";

/// The colour depths Ilixium accepts (`^1$|^4$|^8$|^15$|^16$|^24$|^32$|^48$`).
const ACCEPTED_COLOR_DEPTHS: [u8; 8] = [1, 4, 8, 15, 16, 24, 32, 48];

#[derive(Debug, Clone)]
pub struct IlixiumAuthType {
    pub merchant_password: Secret<String>,
    pub merchant_id: Secret<String>,
    pub account_id: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for IlixiumAuthType {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Ilixium {
                api_key,
                key1,
                api_secret,
                ..
            } => Ok(Self {
                merchant_password: api_key.to_owned(),
                merchant_id: key1.to_owned(),
                account_id: api_secret.to_owned(),
            }),
            _ => Err(error_stack::report!(
                errors::IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext {
                        suggested_action: Some(
                            "Configure this merchant account's Ilixium connector with a \
                             SignatureKey auth type: api_key = Digest Calculation Password, \
                             key1 = MerchantId, api_secret = AccountId."
                                .to_string(),
                        ),
                        doc_url: Some("https://docs.ilixium.com/docs/direct/digest".to_string()),
                        additional_context: Some(
                            "The connector_config passed to IlixiumAuthType::try_from was not \
                             the ConnectorSpecificConfig::Ilixium variant — either a different \
                             connector's config was routed to Ilixium, or Ilixium's three \
                             credentials were never configured for this merchant account."
                                .to_string(),
                        ),
                    }
                }
            )),
        }
    }
}

impl IlixiumAuthType {
    pub fn compute_merchant_digest(
        &self,
        body: &str,
    ) -> Result<String, error_stack::Report<errors::IntegrationError>> {
        let digest_error = |round: &str| errors::IntegrationError::RequestEncodingFailed {
            context: errors::IntegrationErrorContext {
                suggested_action: None,
                doc_url: Some("https://docs.ilixium.com/docs/direct/digest".to_string()),
                additional_context: Some(format!(
                    "SHA-512 hashing failed while computing {round} of Ilixium's \
                     X-MERCHANT-DIGEST (two rounds of Base64(SHA-512(...)) over the request \
                     body, with the Digest Calculation Password appended in round two)."
                )),
            },
        };

        let round_one = BASE64_ENGINE.encode(
            crypto::Sha512
                .generate_digest(body.as_bytes())
                .change_context(digest_error("round one"))?,
        );
        let salted = format!("{}{}", round_one, self.merchant_password.peek());
        let round_two = BASE64_ENGINE.encode(
            crypto::Sha512
                .generate_digest(salted.as_bytes())
                .change_context(digest_error("round two"))?,
        );
        Ok(round_two)
    }
}

/// Whether a value already satisfies Ilixium's `transaction.merchantRef` pattern
/// (`^(?!.*£)[\w]{4,20}$`) and can therefore be sent verbatim.
///
/// This is what makes [`derive_merchant_ref`] **idempotent**: a value it has already produced —
/// either an untouched caller reference or the 20-character hex digest — passes this test, so
/// re-deriving from it returns it unchanged. The Refund flow relies on that, because the caller
/// may legitimately supply *either* the original UCS payment reference *or* the `merchantRef`
/// Ilixium echoed back for it.
fn is_merchant_ref_verbatim_safe(reference: &str) -> bool {
    (MERCHANT_REF_MIN_LEN..=MERCHANT_REF_MAX_LEN).contains(&reference.len())
        && reference
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Ilixium's `transaction.merchantRef` is 4–20 characters of `[\w]` (`[A-Za-z0-9_]`) and must
/// be unique per payment request (a repeat yields response code 102, Duplicate Merchant Ref).
/// A UCS `connector_request_reference_id` is routinely longer than 20 characters and usually
/// contains `-`, which is not in `[\w]`.
///
/// Resolution (tech spec UNDECIDED #3, option (a) made collision-safe):
/// * If the reference already satisfies the pattern verbatim, send it unchanged so the value
///   stays human-traceable in Ilixium's back office.
/// * Otherwise derive `hex(SHA-512(reference))[..20]`. Hex is a subset of `[\w]`, the length
///   is exactly 20, and the derivation is **deterministic**, which is what makes the 3DS
///   second leg work: `/direct/threedcomplete` must quote the *same* `merchantRef` as the
///   original `/direct/auth`, and both legs recompute it from the same reference rather than
///   depending on any persisted mapping.
pub fn derive_merchant_ref(
    reference: &str,
) -> Result<String, error_stack::Report<errors::IntegrationError>> {
    if is_merchant_ref_verbatim_safe(reference) {
        return Ok(reference.to_string());
    }

    if reference.is_empty() {
        return Err(error_stack::report!(
            errors::IntegrationError::MissingRequiredField {
                field_name: "connector_request_reference_id",
                context: errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Ilixium requires a non-empty transaction.merchantRef on every \
                         /direct/auth request."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: None,
                },
            }
        ));
    }

    let digest = crypto::Sha512
        .generate_digest(reference.as_bytes())
        .change_context(errors::IntegrationError::RequestEncodingFailed {
            context: errors::IntegrationErrorContext {
                suggested_action: None,
                doc_url: None,
                additional_context: Some(
                    "SHA-512 hashing failed while deriving Ilixium's 20-character \
                     transaction.merchantRef from the UCS payment reference."
                        .to_string(),
                ),
            },
        })?;
    Ok(hex::encode(digest)
        .chars()
        .take(MERCHANT_REF_MAX_LEN)
        .collect())
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IlixiumPaymentMethodType {
    Card,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IlixiumTransactionType {
    Ecommerce,
}

#[derive(Debug, Clone, Serialize)]
pub struct IlixiumMerchant {
    #[serde(rename = "merchantId")]
    pub merchant_id: Secret<String>,
    #[serde(rename = "accountId")]
    pub account_id: Secret<String>,
}

impl From<&IlixiumAuthType> for IlixiumMerchant {
    fn from(auth: &IlixiumAuthType) -> Self {
        Self {
            merchant_id: auth.merchant_id.clone(),
            account_id: auth.account_id.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct IlixiumTransaction {
    #[serde(rename = "transactionType")]
    pub transaction_type: IlixiumTransactionType,
    #[serde(rename = "merchantRef")]
    pub merchant_ref: String,
    /// Minor units, digits only, serialised as a JSON **string** to match the schema's
    /// `^[\d]{1,12}$` (the published examples emit a bare number, but the schema types it
    /// as a string and both are accepted).
    pub amount: StringMinorUnit,
    /// ISO 4217 three-letter code (`GBP`); the schema also accepts the three-digit form.
    pub currency: Currency,
}

#[derive(Debug, Clone, Serialize)]
pub struct IlixiumCard<T: PaymentMethodDataTypes> {
    #[serde(rename = "cardNumber")]
    pub card_number: RawCardNumber<T>,
    /// `MMyyyy`, no separator (e.g. `012030`).
    #[serde(rename = "expiryDate")]
    pub expiry_date: Secret<String>,
    #[serde(rename = "securityCode")]
    pub security_code: Secret<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IlixiumAddress {
    #[serde(rename = "addressLine1", skip_serializing_if = "Option::is_none")]
    pub address_line1: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub province: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postcode: Option<Secret<String>>,
    /// Always mandatory, even when the account is configured for Optional Address.
    pub country: common_enums::CountryAlpha3,
}

#[derive(Debug, Clone, Serialize)]
pub struct IlixiumCustomer {
    #[serde(rename = "customerId")]
    pub customer_id: String,
    pub email: Email,
    #[serde(rename = "firstName")]
    pub first_name: Secret<String>,
    pub surname: Secret<String>,
    /// `ddmmyyyy`. Schema-mandatory — see [`resolve_date_of_birth`] for how it is sourced
    /// and why it may still be omitted.
    #[serde(rename = "dateOfBirth", skip_serializing_if = "Option::is_none")]
    pub date_of_birth: Option<Secret<String>>,
    pub address: IlixiumAddress,
    #[serde(rename = "mobileNumber", skip_serializing_if = "Option::is_none")]
    pub mobile_number: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IlixiumPaymentInfo {
    /// Cardholder browser IP — recommended by Ilixium whenever 3-D Secure is in play.
    #[serde(rename = "ipAddress")]
    pub ip_address: Secret<String>,
}

/// Mandatory when the merchant's account performs 3-D Secure; ignored (but harmless) when the
/// account is not configured for 3DS. `browserDetails` is the platform-performed-3DS branch of
/// the schema's `oneOf`; the `externalReferences` branch (merchant/third-party
/// pre-authentication) is out of scope here.
#[derive(Debug, Clone, Serialize)]
pub struct IlixiumEmvco3ds {
    #[serde(rename = "browserDetails")]
    pub browser_details: IlixiumBrowserDetails,
}

/// All ten fields are mandatory. The schema types the numeric ones as strings.
#[derive(Debug, Clone, Serialize)]
pub struct IlixiumBrowserDetails {
    #[serde(rename = "acceptHeader")]
    pub accept_header: String,
    #[serde(rename = "javaScriptEnabled")]
    pub java_script_enabled: bool,
    #[serde(rename = "javaEnabled")]
    pub java_enabled: bool,
    pub language: String,
    #[serde(rename = "screenHeight")]
    pub screen_height: String,
    #[serde(rename = "screenWidth")]
    pub screen_width: String,
    /// `getTimezoneOffset()` in minutes.
    #[serde(rename = "timeDifference")]
    pub time_difference: String,
    #[serde(rename = "userAgent")]
    pub user_agent: String,
    #[serde(rename = "colorDepth")]
    pub color_depth: String,
    #[serde(rename = "challengeWindowSize")]
    pub challenge_window_size: String,
}

/// Untagged request body for the Authorize flow.
///
/// Leg 1 (`Auth`) is `POST /direct/auth`; leg 2 (`ThreeDsComplete`) is
/// `POST /direct/threedcomplete`. The leg is selected by [`is_three_ds_completion`], which is
/// also what `get_url` in `ilixium.rs` keys off, so the URL and the body can never disagree.
/// `untagged` so each variant serialises as its own bare body.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum IlixiumAuthorizeRequest<T: PaymentMethodDataTypes> {
    Auth(Box<IlixiumPaymentsRequest<T>>),
    ThreeDsComplete(Box<IlixiumThreeDsCompleteRequest>),
}

#[derive(Debug, Clone, Serialize)]
pub struct IlixiumPaymentsRequest<T: PaymentMethodDataTypes> {
    pub version: u8,
    /// `false` (the default) captures the authorisation immediately — the response `type`
    /// is then `AUTH_CAP`. `true` authorises only and requires a later Capture or Reversal.
    #[serde(rename = "deferredCapture")]
    pub deferred_capture: bool,
    pub transaction: IlixiumTransaction,
    #[serde(rename = "paymentMethodType")]
    pub payment_method_type: IlixiumPaymentMethodType,
    pub merchant: IlixiumMerchant,
    pub card: IlixiumCard<T>,
    pub customer: IlixiumCustomer,
    #[serde(rename = "paymentInfo", skip_serializing_if = "Option::is_none")]
    pub payment_info: Option<IlixiumPaymentInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emvco3ds: Option<IlixiumEmvco3ds>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IlixiumThreeDsTransactionRef {
    /// Must be byte-identical to the `merchantRef` sent on the original `/direct/auth`.
    #[serde(rename = "merchantRef")]
    pub merchant_ref: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct IlixiumThreeDsData {
    pub md: Secret<String>,
    #[serde(rename = "paRes")]
    pub pa_res: Secret<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IlixiumThreeDsCompleteRequest {
    pub version: u8,
    pub merchant: IlixiumMerchant,
    pub transaction: IlixiumThreeDsTransactionRef,
    #[serde(rename = "threeDSecure")]
    pub three_d_secure: IlixiumThreeDsData,
}

/// Whether this Authorize invocation is the 3DS return leg.
///
/// UCS re-invokes `Authorize` after the ACS posts back to `TermUrl`, and it is the presence of
/// `redirect_response` that distinguishes that call from the initial authorisation. Used by
/// both the request builder below and `get_url` in `ilixium.rs`.
pub fn is_three_ds_completion<T: PaymentMethodDataTypes>(
    request: &PaymentsAuthorizeData<T>,
) -> bool {
    request.redirect_response.is_some()
}

/// Ilixium wants `customer.dateOfBirth` as `ddmmyyyy` — eight digits, no separators, so
/// 3 April 1970 is `03041970`. The rest of the stack carries a real `Date`; this is the only
/// place that knows Ilixium's shape.
fn format_date_of_birth(date_of_birth: &Secret<Date>) -> Secret<String> {
    let date = date_of_birth.peek();
    Secret::new(format!(
        "{:02}{:02}{:04}",
        date.day(),
        u8::from(date.month()),
        date.year(),
    ))
}

/// Deprecated path: before `customer.date_of_birth` existed in the UCS payment model, the only
/// way to reach Ilixium's schema-mandatory `dateOfBirth` was the merchant-supplied `metadata`
/// object. Merchants integrated against that, so it stays as a fallback for one release; the
/// value is passed through verbatim because it was already in Ilixium's `ddmmyyyy` shape.
///
/// Remove once merchants have moved to `customer.date_of_birth`.
fn extract_date_of_birth_from_metadata(
    metadata: Option<&common_utils::pii::SecretSerdeValue>,
) -> Option<Secret<String>> {
    let value = metadata?.clone().expose();
    ["ilixium_date_of_birth", "date_of_birth"]
        .iter()
        .find_map(|key| value.get(key).and_then(|v| v.as_str()).map(String::from))
        .map(Secret::new)
}

/// Resolves `customer.dateOfBirth`, preferring the structured field over the deprecated metadata
/// key. Absent from both, it is omitted rather than filled with a placeholder: a fabricated date
/// would reach the issuer and could distort Ilixium's own fraud checks. Accounts that enforce the
/// field answer `VA8`, which surfaces as a normal `REJECTED` error rather than a silently wrong
/// value.
fn resolve_date_of_birth(
    customer_date_of_birth: Option<&Secret<Date>>,
    metadata: Option<&common_utils::pii::SecretSerdeValue>,
) -> Option<Secret<String>> {
    if let Some(date_of_birth) = customer_date_of_birth {
        return Some(format_date_of_birth(date_of_birth));
    }
    extract_date_of_birth_from_metadata(metadata).inspect(|_| {
        tracing::warn!(
            connector = "ilixium",
            "Reading customer.dateOfBirth from connector metadata is deprecated and will be \
             removed; send customer.date_of_birth on the payment or customer instead."
        );
    })
}

/// Ilixium accepts only the colour depths in [`ACCEPTED_COLOR_DEPTHS`], while browsers report
/// whatever `screen.colorDepth` returns (30 on some X11 configurations, for instance). Snap
/// down to the nearest accepted value so a legitimate browser reading never turns into a
/// `VB88` rejection.
fn normalize_color_depth(reported: u8) -> u8 {
    ACCEPTED_COLOR_DEPTHS
        .iter()
        .rev()
        .find(|accepted| **accepted <= reported)
        .copied()
        .unwrap_or(ACCEPTED_COLOR_DEPTHS[0])
}

fn build_browser_details(
    browser_info: &BrowserInformation,
) -> Result<IlixiumBrowserDetails, error_stack::Report<errors::IntegrationError>> {
    let missing = |field: &'static str| {
        error_stack::report!(errors::IntegrationError::MissingRequiredField {
            field_name: field,
            context: errors::IntegrationErrorContext {
                suggested_action: Some(
                    "Ilixium requires all ten emvco3ds.browserDetails fields on a 3-D Secure \
                     authorisation. Collect the full browser profile client-side before \
                     confirming a three_ds card payment."
                        .to_string(),
                ),
                doc_url: Some("https://docs.ilixium.com/docs/direct/3dsecure".to_string()),
                additional_context: None,
            },
        })
    };

    Ok(IlixiumBrowserDetails {
        accept_header: browser_info
            .accept_header
            .clone()
            .ok_or_else(|| missing("browser_info.accept_header"))?,
        java_script_enabled: browser_info
            .java_script_enabled
            .ok_or_else(|| missing("browser_info.java_script_enabled"))?,
        java_enabled: browser_info
            .java_enabled
            .ok_or_else(|| missing("browser_info.java_enabled"))?,
        language: browser_info
            .language
            .clone()
            .ok_or_else(|| missing("browser_info.language"))?,
        screen_height: browser_info
            .screen_height
            .ok_or_else(|| missing("browser_info.screen_height"))?
            .to_string(),
        screen_width: browser_info
            .screen_width
            .ok_or_else(|| missing("browser_info.screen_width"))?
            .to_string(),
        time_difference: browser_info
            .time_zone
            .ok_or_else(|| missing("browser_info.time_zone"))?
            .to_string(),
        user_agent: browser_info
            .user_agent
            .clone()
            .ok_or_else(|| missing("browser_info.user_agent"))?,
        color_depth: normalize_color_depth(
            browser_info
                .color_depth
                .ok_or_else(|| missing("browser_info.color_depth"))?,
        )
        .to_string(),
        challenge_window_size: CHALLENGE_WINDOW_SIZE_FULL_SCREEN.to_string(),
    })
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        IlixiumRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for IlixiumAuthorizeRequest<T>
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: IlixiumRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        if is_three_ds_completion(&item.router_data.request) {
            Ok(Self::ThreeDsComplete(Box::new(
                IlixiumThreeDsCompleteRequest::try_from(&item.router_data)?,
            )))
        } else {
            Ok(Self::Auth(Box::new(IlixiumPaymentsRequest::try_from(
                &item,
            )?)))
        }
    }
}

/// Everything the `/direct/auth` body needs, normalised out of whichever flow request is driving
/// it.
///
/// `/direct/auth` is reachable from two flows: `Authorize` (non-3DS payments, and the 3DS first
/// leg when the connector is not routed through PreAuthenticate) and `PreAuthenticate` (the 3DS
/// first leg). The two carry the same information under different types — and
/// [`PaymentsPreAuthenticateData`] carries strictly less of it — so both funnel through here
/// rather than duplicating a 130-line body builder.
///
/// Fields the PreAuthenticate request cannot supply are `Option` and documented at the call site.
pub(super) struct IlixiumAuthBodyInputs<'a, T: PaymentMethodDataTypes> {
    pub payment_method_data: &'a PaymentMethodData<T>,
    /// Already converted to Ilixium's minor-unit digit string by the caller, which owns the
    /// flow-specific amount field (`minor_amount` vs `amount`).
    pub amount: StringMinorUnit,
    pub currency: Currency,
    pub email: Option<Email>,
    /// `None` on the PreAuthenticate leg — `PaymentsPreAuthenticateData` has no `customer_name`,
    /// so the billing address is the only name source there.
    pub customer_name: Option<&'a str>,
    /// Available on both legs — `PaymentsPreAuthenticateData` carries it too.
    pub customer_date_of_birth: Option<&'a Secret<Date>>,
    /// Only still read for the deprecated `ilixium_date_of_birth` fallback — see
    /// [`resolve_date_of_birth`].
    pub metadata: Option<&'a common_utils::pii::SecretSerdeValue>,
    pub browser_info: Option<&'a BrowserInformation>,
    pub is_auto_capture: bool,
    pub is_three_ds: bool,
}

/// Builds the `POST /direct/auth` body shared by the Authorize and PreAuthenticate flows.
pub(super) fn build_ilixium_payments_request<T: PaymentMethodDataTypes + std::fmt::Debug>(
    inputs: IlixiumAuthBodyInputs<'_, T>,
    common: &PaymentFlowData,
    connector_config: &ConnectorSpecificConfig,
) -> Result<IlixiumPaymentsRequest<T>, error_stack::Report<errors::IntegrationError>> {
    let card = match inputs.payment_method_data {
        PaymentMethodData::Card(card_data) => IlixiumCard {
            card_number: card_data.card_number.clone(),
            expiry_date: Secret::new(format!(
                "{}{}",
                card_data.get_card_expiry_month_2_digit()?.peek(),
                card_data.get_expiry_year_4_digit().peek()
            )),
            security_code: card_data.card_cvc.clone(),
        },
        other => {
            return Err(error_stack::report!(
                errors::IntegrationError::NotImplemented(
                    "Ilixium supports only raw card payments on POST /direct/auth; wallets, \
                     APMs, bank transfers and stored-card tokens are out of scope for this \
                     connector implementation"
                        .to_string(),
                    errors::IntegrationErrorContext {
                        suggested_action: Some(
                            "Route this payment with PaymentMethodData::Card.".to_string(),
                        ),
                        doc_url: Some(
                            "https://docs.ilixium.com/docs/api/authorisation".to_string()
                        ),
                        additional_context: Some(format!(
                            "Unsupported payment_method_data variant for Ilixium \
                             /direct/auth: {other:?}"
                        )),
                    },
                )
            ));
        }
    };

    let auth = IlixiumAuthType::try_from(connector_config)?;

    let amount = inputs.amount;

    let country = common.get_optional_billing_country().ok_or_else(|| {
        error_stack::report!(errors::IntegrationError::MissingRequiredField {
            field_name: "payment_method_data.billing.address.country",
            context: errors::IntegrationErrorContext {
                suggested_action: Some(
                    "Ilixium always requires customer.address.country, even on accounts \
                     configured for Optional Address."
                        .to_string(),
                ),
                doc_url: Some("https://docs.ilixium.com/docs/direct/optional-address".to_string()),
                additional_context: None,
            },
        })
    })?;
    let country = common_enums::CountryAlpha2::from_alpha2_to_alpha3(country);

    let email = inputs
        .email
        .clone()
        .or_else(|| common.get_optional_billing_email())
        .ok_or_else(|| {
            error_stack::report!(errors::IntegrationError::MissingRequiredField {
                field_name: "email",
                context: errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Ilixium requires customer.email on every authorisation.".to_string(),
                    ),
                    doc_url: None,
                    additional_context: None,
                },
            })
        })?;

    let (first_name, surname) = split_customer_name(common, inputs.customer_name)?;

    let customer_id = common
        .customer_id
        .as_ref()
        .map(|id| id.get_string_repr().to_string())
        .unwrap_or_else(|| common.connector_request_reference_id.clone());

    let ip_address = inputs
        .browser_info
        .and_then(|info| info.ip_address)
        .map(|ip| Secret::new(ip.to_string()));

    // Ilixium keys emvco3ds off the *account*, not the transaction: an account configured for 3-D
    // Secure rejects every auth that omits a full browserDetails block (VA80-VA89, one code per
    // missing field), while an account not so configured ignores the element when it is present.
    // So send it whenever the browser profile is complete; only a 3DS authorisation treats an
    // absent or partial profile as an error.
    let missing_browser_info = || {
        error_stack::report!(errors::IntegrationError::MissingRequiredField {
            field_name: "browser_info",
            context: errors::IntegrationErrorContext {
                suggested_action: Some(
                    "A 3-D Secure card authorisation must carry the browser profile \
                     that populates emvco3ds.browserDetails."
                        .to_string(),
                ),
                doc_url: Some("https://docs.ilixium.com/docs/direct/3dsecure".to_string()),
                additional_context: None,
            },
        })
    };

    let emvco3ds = match inputs.browser_info {
        Some(browser_info) if inputs.is_three_ds => Some(IlixiumEmvco3ds {
            browser_details: build_browser_details(browser_info)?,
        }),
        // Best-effort outside 3DS: a partial profile is omitted rather than fatal, so merchants on
        // accounts that do not require 3-D Secure behave exactly as they did before.
        Some(browser_info) => build_browser_details(browser_info)
            .ok()
            .map(|browser_details| IlixiumEmvco3ds { browser_details }),
        None if inputs.is_three_ds => return Err(missing_browser_info()),
        None => None,
    };

    let payment_info = ip_address.map(|ip_address| IlixiumPaymentInfo { ip_address });

    Ok(IlixiumPaymentsRequest {
        version: ILIXIUM_MESSAGE_VERSION,
        deferred_capture: !inputs.is_auto_capture,
        transaction: IlixiumTransaction {
            transaction_type: IlixiumTransactionType::Ecommerce,
            merchant_ref: derive_merchant_ref(&common.connector_request_reference_id)?,
            amount,
            currency: inputs.currency,
        },
        payment_method_type: IlixiumPaymentMethodType::Card,
        merchant: IlixiumMerchant::from(&auth),
        card,
        customer: IlixiumCustomer {
            customer_id,
            email,
            first_name,
            surname,
            date_of_birth: resolve_date_of_birth(inputs.customer_date_of_birth, inputs.metadata),
            address: IlixiumAddress {
                address_line1: common.get_optional_billing_line1(),
                city: common.get_optional_billing_city(),
                province: common.get_optional_billing_state(),
                postcode: common.get_optional_billing_zip(),
                country,
            },
            mobile_number: common.get_billing_phone_number().ok(),
        },
        payment_info,
        emvco3ds,
    })
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        &IlixiumRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for IlixiumPaymentsRequest<T>
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: &IlixiumRouterData<
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

        let amount = item
            .connector
            .amount_converter
            .convert(request.minor_amount, request.currency)
            .change_context(errors::IntegrationError::AmountConversionFailed {
                context: errors::IntegrationErrorContext {
                    suggested_action: None,
                    doc_url: None,
                    additional_context: Some(format!(
                        "Failed to convert minor_amount {} {} into Ilixium's \
                         transaction.amount (minor units, digits only, sent as a JSON string).",
                        request.minor_amount.get_amount_as_i64(),
                        request.currency
                    )),
                },
            })?;

        build_ilixium_payments_request(
            IlixiumAuthBodyInputs {
                payment_method_data: &request.payment_method_data,
                amount,
                currency: request.currency,
                email: request.email.clone(),
                customer_name: request.customer_name.as_deref(),
                customer_date_of_birth: request.customer_date_of_birth.as_ref(),
                metadata: request.metadata.as_ref(),
                browser_info: request.browser_info.as_ref(),
                is_auto_capture: request.is_auto_capture(),
                is_three_ds: common.auth_type == AuthenticationType::ThreeDs,
            },
            common,
            &router_data.connector_config,
        )
    }
}

/// `POST /direct/auth` built from the **PreAuthenticate** leg.
///
/// Same body as the Authorize path — Ilixium has one authorisation endpoint — but
/// [`PaymentsPreAuthenticateData`] is a narrower struct, so three things differ:
///
/// * `payment_method_data` and `currency` are `Option` here and must be unwrapped;
/// * `amount` is a single `MinorUnit` (there is no separate `minor_amount`);
/// * `is_auto_capture()` returns a `Result` (it rejects `ManualMultiple`/`Scheduled`) rather than
///   a bare `bool`.
///
/// One input is unavailable on this leg: `customer_name`, so the billing address is the only
/// name source (which [`split_customer_name`] already handles). `customer_date_of_birth` and
/// `metadata` are both carried by `PaymentsPreAuthenticateData`, so `customer.dateOfBirth`
/// resolves here exactly as it does on Authorize.
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        IlixiumRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for IlixiumPreAuthenticateRequest<T>
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: IlixiumRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;
        let common = &router_data.resource_common_data;

        let missing = |field: &'static str| {
            error_stack::report!(errors::IntegrationError::MissingRequiredField {
                field_name: field,
                context: errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Ilixium's PreAuthenticate leg sends the full POST /direct/auth \
                         authorisation, so it needs the same card, amount and currency an \
                         Authorize would carry."
                            .to_string(),
                    ),
                    doc_url: Some("https://docs.ilixium.com/docs/api/authorisation".to_string()),
                    additional_context: None,
                },
            })
        };

        let payment_method_data = request
            .payment_method_data
            .as_ref()
            .ok_or_else(|| missing("payment_method_data"))?;
        let currency = request.currency.ok_or_else(|| missing("currency"))?;

        let amount = item
            .connector
            .amount_converter
            .convert(request.amount, currency)
            .change_context(errors::IntegrationError::AmountConversionFailed {
                context: errors::IntegrationErrorContext {
                    suggested_action: None,
                    doc_url: None,
                    additional_context: Some(format!(
                        "Failed to convert amount {} {} into Ilixium's transaction.amount \
                         (minor units, digits only, sent as a JSON string).",
                        request.amount.get_amount_as_i64(),
                        currency
                    )),
                },
            })?;

        build_ilixium_payments_request(
            IlixiumAuthBodyInputs {
                payment_method_data,
                amount,
                currency,
                email: request.email.clone(),
                customer_name: None,
                customer_date_of_birth: request.customer_date_of_birth.as_ref(),
                metadata: request.metadata.as_ref(),
                browser_info: request.browser_info.as_ref(),
                is_auto_capture: request.is_auto_capture()?,
                is_three_ds: true,
            },
            common,
            &router_data.connector_config,
        )
    }
}

/// Ilixium requires `firstName` and `surname` separately. Prefer the structured billing name,
/// then fall back to splitting the single `customer_name` on the first space.
fn split_customer_name(
    common: &PaymentFlowData,
    customer_name: Option<&str>,
) -> Result<(Secret<String>, Secret<String>), error_stack::Report<errors::IntegrationError>> {
    let missing = |field: &'static str| {
        error_stack::report!(errors::IntegrationError::MissingRequiredField {
            field_name: field,
            context: errors::IntegrationErrorContext {
                suggested_action: Some(
                    "Ilixium requires customer.firstName and customer.surname. Supply a \
                     billing address with first_name/last_name, or a customer name containing \
                     both parts."
                        .to_string(),
                ),
                doc_url: None,
                additional_context: None,
            },
        })
    };

    let billing_first = common.get_optional_billing_first_name();
    let billing_last = common.get_optional_billing_last_name();
    if let (Some(first), Some(last)) = (billing_first.clone(), billing_last.clone()) {
        return Ok((first, last));
    }

    let (split_first, split_last) = customer_name
        .and_then(|name| {
            name.trim()
                .split_once(char::is_whitespace)
                .map(|(first, last)| (first.to_string(), last.trim().to_string()))
        })
        .map_or((None, None), |(first, last)| {
            (Some(Secret::new(first)), Some(Secret::new(last)))
        });

    let first_name = billing_first
        .or(split_first)
        .ok_or_else(|| missing("payment_method_data.billing.address.first_name"))?;
    let surname = billing_last
        .or(split_last)
        .ok_or_else(|| missing("payment_method_data.billing.address.last_name"))?;
    Ok((first_name, surname))
}

impl<T: PaymentMethodDataTypes>
    TryFrom<
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    > for IlixiumThreeDsCompleteRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        router_data: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = IlixiumAuthType::try_from(&router_data.connector_config)?;

        let payload = router_data
            .request
            .redirect_response
            .as_ref()
            .and_then(|response| response.payload.clone())
            .ok_or_else(|| {
                error_stack::report!(errors::IntegrationError::MissingRequiredField {
                    field_name: "request.redirect_response.payload",
                    context: errors::IntegrationErrorContext {
                        suggested_action: Some(
                            "The ACS posts `md` and `paRes` back to TermUrl; both must be \
                             forwarded into the Authorize completion call."
                                .to_string(),
                        ),
                        doc_url: Some(
                            "https://docs.ilixium.com/docs/api/threedcomplete".to_string()
                        ),
                        additional_context: None,
                    },
                })
            })?
            .expose();

        let pick = |candidates: &[&str], field_name: &'static str| {
            candidates
                .iter()
                .find_map(|key| payload.get(*key).and_then(|value| value.as_str()))
                .map(|value| Secret::new(value.to_string()))
                .ok_or_else(|| {
                    error_stack::report!(errors::IntegrationError::MissingRequiredField {
                        field_name,
                        context: errors::IntegrationErrorContext {
                            suggested_action: Some(
                                "POST /direct/threedcomplete requires both threeDSecure.md and \
                                 threeDSecure.paRes exactly as returned by the ACS."
                                    .to_string(),
                            ),
                            doc_url: Some(
                                "https://docs.ilixium.com/docs/api/threedcomplete".to_string()
                            ),
                            additional_context: None,
                        },
                    })
                })
        };

        Ok(Self {
            version: ILIXIUM_MESSAGE_VERSION,
            merchant: IlixiumMerchant::from(&auth),
            transaction: IlixiumThreeDsTransactionRef {
                merchant_ref: derive_merchant_ref(
                    &router_data
                        .resource_common_data
                        .connector_request_reference_id,
                )?,
            },
            three_d_secure: IlixiumThreeDsData {
                md: pick(&["md", "MD", "Md"], "request.redirect_response.payload.md")?,
                pa_res: pick(
                    &["paRes", "PaRes", "PARes", "pares"],
                    "request.redirect_response.payload.paRes",
                )?,
            },
        })
    }
}

/// `captureRequest.transaction`.
///
/// A deliberate second transaction struct rather than a reuse of [`IlixiumTransaction`]:
/// `captureRequest` requires exactly `amount`, `currency` and `merchantRef`, and Ilixium
/// ignores `transactionType` here (it is inherited from `transactionDetails` but is not part
/// of a capture), so sending it would be noise on a message that identifies the payment by
/// nothing but its reference.
#[derive(Debug, Clone, Serialize)]
pub struct IlixiumCaptureTransaction {
    /// **The original authorisation's** `merchantRef` — this is the only thing binding the
    /// capture to a payment; `captureRequest` carries no gateway id. A reference that matches
    /// nothing yields response code 104.
    #[serde(rename = "merchantRef")]
    pub merchant_ref: String,
    /// Minor units, digits only, serialised as a JSON string (`^[\d]{1,12}$`).
    pub amount: StringMinorUnit,
    /// Must equal the original transaction's currency; a mismatch yields response code 103.
    pub currency: Currency,
}

/// `POST /direct/capture` body. The schema's top-level properties are exactly `version`,
/// `transaction` and `merchant` — there is no card, token, customer or paymentMethodType on a
/// capture.
#[derive(Debug, Clone, Serialize)]
pub struct IlixiumCaptureRequest {
    pub version: u8,
    pub transaction: IlixiumCaptureTransaction,
    pub merchant: IlixiumMerchant,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        IlixiumRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for IlixiumCaptureRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: IlixiumRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;
        let common = &router_data.resource_common_data;

        let auth = IlixiumAuthType::try_from(&router_data.connector_config)?;

        let amount = item
            .connector
            .amount_converter
            .convert(request.minor_amount_to_capture, request.currency)
            .change_context(errors::IntegrationError::AmountConversionFailed {
                context: errors::IntegrationErrorContext {
                    suggested_action: None,
                    doc_url: None,
                    additional_context: Some(format!(
                        "Failed to convert minor_amount_to_capture {} {} into Ilixium's \
                         transaction.amount (minor units, digits only, sent as a JSON string).",
                        request.minor_amount_to_capture.get_amount_as_i64(),
                        request.currency
                    )),
                },
            })?;

        Ok(Self {
            version: ILIXIUM_MESSAGE_VERSION,
            transaction: IlixiumCaptureTransaction {
                merchant_ref: derive_merchant_ref(&common.connector_request_reference_id)?,
                amount,
                currency: request.currency,
            },
            merchant: IlixiumMerchant::from(&auth),
        })
    }
}

/// `reversalRequest.transaction`.
///
/// `reversalRequest` is field-for-field identical to `captureRequest` — same
/// `required: [merchant, transaction, version]`, same regexes, same length limits; only the
/// schema `title`/`description` differ. It is kept as a sibling of
/// [`IlixiumCaptureTransaction`] rather than reusing it because `amount` means something
/// materially different here: on a capture it may be *less* than the original (partial capture
/// is allowed), on a reversal it **must equal it exactly**.
#[derive(Debug, Clone, Serialize)]
pub struct IlixiumReversalTransaction {
    /// **The original authorisation's** `merchantRef` — the only thing binding the reversal to
    /// a payment; `reversalRequest` carries no gateway id. A reference that matches nothing
    /// yields response code 104.
    #[serde(rename = "merchantRef")]
    pub merchant_ref: String,
    /// Minor units, digits only, serialised as a JSON string (`^[\d]{1,12}$`).
    ///
    /// **Partial reversals are not supported.** This must equal the original transaction's
    /// amount exactly; anything else — including `0` or a partial value — is rejected with
    /// response code 142, "Reversal amount does not match original transaction". Nothing in
    /// this connector ever synthesises the value: it is whatever the caller supplied, and the
    /// request is refused locally when the caller supplied nothing.
    pub amount: StringMinorUnit,
    /// Must equal the original transaction's currency; a mismatch yields response code 103.
    pub currency: Currency,
}

/// `POST /direct/reversal` body. Top-level properties are exactly `version`, `transaction` and
/// `merchant` — the same three the capture body carries.
#[derive(Debug, Clone, Serialize)]
pub struct IlixiumVoidRequest {
    pub version: u8,
    pub transaction: IlixiumReversalTransaction,
    pub merchant: IlixiumMerchant,
}

/// Builds the reversal body from a UCS Void request.
///
/// **The missing-amount problem.** `PaymentVoidData::amount` and `::currency` are both
/// `Option`, mirroring `PaymentServiceVoidRequest.amount`, which is an `optional Money` on the
/// wire — a caller may legitimately omit it. Ilixium, however, makes `transaction.amount`
/// mandatory *and* requires it to equal the original transaction's amount exactly. There is no
/// second source to fall back on: `PaymentFlowData` is built for the Void flow with `amount`,
/// `minor_amount_captured`, `minor_amount_capturable` and `minor_amount_authorized` all set to
/// `None`, so `resource_common_data` knows nothing about the payment's value either.
///
/// Every way of papering over that would be worse than failing:
/// * sending `0` or a partial amount earns response code 142 — a remote rejection dressed up as
///   a local success path, and one that consumes an operation slot on the transaction (code
///   117 serialises operations per transaction);
/// * fabricating an amount risks reversing the wrong figure if Ilixium ever relaxes the
///   equality check.
///
/// So a Void with no amount is refused here with a `MissingRequiredField` naming `amount` (or
/// `currency`) and spelling out that the *original* transaction's values are required.
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        IlixiumRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for IlixiumVoidRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: IlixiumRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;
        let common = &router_data.resource_common_data;

        let auth = IlixiumAuthType::try_from(&router_data.connector_config)?;

        let missing = |field: &'static str, what: &str| {
            error_stack::report!(errors::IntegrationError::MissingRequiredField {
                field_name: field,
                context: errors::IntegrationErrorContext {
                    suggested_action: Some(format!(
                        "Ilixium does not support partial reversals: POST /direct/reversal \
                         requires transaction.{what}, and it must match the original \
                         transaction exactly. Send the void request with the original \
                         authorisation's amount and currency (PaymentServiceVoidRequest.amount \
                         carries both)."
                    )),
                    doc_url: Some("https://docs.ilixium.com/docs/api/reversal".to_string()),
                    additional_context: Some(
                        "PaymentVoidData carries amount/currency as Option because the gRPC \
                         field is optional, and the Void flow's PaymentFlowData holds no \
                         amount to fall back on. Rather than send 0 or a partial value — both \
                         of which Ilixium rejects with response code 142, 'Reversal amount \
                         does not match original transaction' — the request is refused here."
                            .to_string(),
                    ),
                },
            })
        };

        let minor_amount = request.amount.ok_or_else(|| missing("amount", "amount"))?;
        let currency = request
            .currency
            .ok_or_else(|| missing("currency", "currency"))?;

        let amount = item
            .connector
            .amount_converter
            .convert(minor_amount, currency)
            .change_context(errors::IntegrationError::AmountConversionFailed {
                context: errors::IntegrationErrorContext {
                    suggested_action: None,
                    doc_url: None,
                    additional_context: Some(format!(
                        "Failed to convert void amount {} {} into Ilixium's \
                         transaction.amount (minor units, digits only, sent as a JSON string).",
                        minor_amount.get_amount_as_i64(),
                        currency
                    )),
                },
            })?;

        Ok(Self {
            version: ILIXIUM_MESSAGE_VERSION,
            transaction: IlixiumReversalTransaction {
                merchant_ref: derive_merchant_ref(&common.connector_request_reference_id)?,
                amount,
                currency,
            },
            merchant: IlixiumMerchant::from(&auth),
        })
    }
}

/// `refundRequest.transaction`, **variant A** — a refund that references a previous
/// transaction.
///
/// `refundRequest` is a single schema covering two variants. Variant A carries exactly the same
/// six fields as `captureRequest`/`reversalRequest`; variant B is a *standalone* refund that
/// additionally needs `paymentMethodType`, `token`-or-`card`, `customer` and `paymentInfo`.
/// Only variant A is implemented: a connector Refund flow always has a previous payment, and
/// the vendor states standalone refunds are account-gated ("during testing both types will be
/// accepted, although this may not be the case within the production environment").
///
/// A sibling of [`IlixiumCaptureTransaction`] / [`IlixiumReversalTransaction`] rather than a
/// reuse of either, because `amount` means a third thing again: on a capture it may be less than
/// the original (partial capture), on a reversal it **must** equal it exactly, and on a refund it
/// may be less than the original *and* accumulates across successive refunds — Ilixium rejects
/// the request with code 125 only once the **total** would exceed the original authorised amount.
#[derive(Debug, Clone, Serialize)]
pub struct IlixiumRefundTransaction {
    /// **The original payment's** `merchantRef` — the only thing binding the refund to a
    /// transaction; `refundRequest` carries no gateway id and mints no refund id. A reference
    /// that matches nothing yields response code 104.
    #[serde(rename = "merchantRef")]
    pub merchant_ref: String,
    /// Minor units, digits only, serialised as a JSON string (`^[\d]{1,12}$`).
    ///
    /// **Partial refunds are supported and cumulative.** This is
    /// `RefundsData::minor_refund_amount` (the amount to refund now), never
    /// `minor_payment_amount` (the original payment's total).
    pub amount: StringMinorUnit,
    /// Must equal the original transaction's currency; a mismatch yields response code 103.
    pub currency: Currency,
}

/// `POST /direct/refund` body, variant A. The three top-level properties are the same ones the
/// capture and reversal bodies carry — no card, token, customer, paymentInfo or
/// paymentMethodType, all of which belong to variant B only.
#[derive(Debug, Clone, Serialize)]
pub struct IlixiumRefundRequest {
    pub version: u8,
    pub transaction: IlixiumRefundTransaction,
    pub merchant: IlixiumMerchant,
}

/// Resolves the **original payment's** `merchantRef` for a refund-side flow.
///
/// This is the one place where Refund and RSync cannot copy Capture and Void. Those flows read
/// `PaymentFlowData::connector_request_reference_id`, which UCS populates from
/// `merchant_capture_id` / `merchant_void_id` — i.e. from a caller-supplied reference that
/// *identifies the original payment*. On the refund-side flows the analogous field,
/// `RefundFlowData::connector_request_reference_id`, is populated from **`merchant_refund_id`**:
/// it identifies the *refund*, not the payment. Deriving a `merchantRef` from it would address a
/// transaction that does not exist — response code 104 on `/direct/refund`, and on RSync a
/// client-side filter over `/history/operations` that can never match anything.
///
/// So the payment reference is taken from the flow's *request* data, in this order:
///
/// 1. **`connector_order_id`** (`PaymentServiceRefundRequest.connector_order_id`, and its
///    `RefundSyncData` counterpart) — the field whose documented purpose is exactly this:
///    "connector-side identifier for the original payment that this refund targets". It is passed
///    through [`derive_merchant_ref`], which is idempotent (see [`is_merchant_ref_verbatim_safe`]),
///    so it works whether the caller supplies the original UCS payment reference or the
///    `merchantRef` Ilixium echoed back in `connector_reference_id`.
/// 2. **`connector_transaction_id`**, but only when it is already `merchantRef`-shaped. For
///    Ilixium this field normally holds `transaction.gatewayRef`, a 36-character dashed UUID that
///    is *not* a `merchantRef` and cannot be converted into one — hashing it would produce a
///    well-formed reference pointing at nothing. The shape test is what separates the two: a
///    caller that put the reference itself here is honoured, a gatewayRef is not.
/// 3. Otherwise the request is **refused locally**. Sending a derived-from-the-wrong-input
///    reference would be a guaranteed 104 dressed up as a local success path, and it consumes an
///    operation slot on the transaction (Ilixium serialises operations per transaction, code
///    117).
///
/// Taken as two plain values rather than as a `&RefundsData` so that RSync — which is handed a
/// [`RefundSyncData`] carrying the very same two fields — resolves the reference through exactly
/// this ladder rather than through a copy of it.
fn resolve_original_merchant_ref(
    connector_order_id: Option<&str>,
    connector_transaction_id: &str,
) -> Result<String, error_stack::Report<errors::IntegrationError>> {
    if let Some(order_id) = connector_order_id.filter(|value| !value.is_empty()) {
        return derive_merchant_ref(order_id);
    }

    if is_merchant_ref_verbatim_safe(connector_transaction_id) {
        return Ok(connector_transaction_id.to_owned());
    }

    Err(error_stack::report!(
        errors::IntegrationError::MissingRequiredField {
            field_name: "connector_order_id",
            context: errors::IntegrationErrorContext {
                suggested_action: Some(
                    "Set connector_order_id on the refund (or refund-sync) request to the \
                     original payment's reference — either the merchant_transaction_id the \
                     payment was authorised with, or the connector_reference_id Ilixium returned \
                     for it (both resolve to the same transaction.merchantRef). POST \
                     /direct/refund binds a refund to a payment by merchantRef alone, and POST \
                     /history/operations can only be filtered on it client-side."
                        .to_string(),
                ),
                doc_url: Some("https://docs.ilixium.com/docs/api/refund".to_string()),
                additional_context: Some(
                    "RefundFlowData::connector_request_reference_id is derived from \
                     merchant_refund_id, so it identifies the refund rather than the payment, and \
                     connector_transaction_id holds Ilixium's gatewayRef (a dashed UUID) which is \
                     not a merchantRef and cannot be converted into one. Rather than send a \
                     reference that matches no transaction — response code 104, 'No Matching \
                     Transaction', or a history filter that selects nothing — the request is \
                     refused here."
                        .to_string(),
                ),
            },
        }
    ))
}

/// The refund's own reference, as supplied by UCS.
///
/// Ilixium returns **no refund identifier** (see [`refund_identifier`]), so this value is the
/// connector's only stable handle on an individual refund. It is validated when the request is
/// built — before any money moves — rather than at response time, so that a refund Ilixium has
/// already accepted can never fail for want of an id.
///
/// `RefundsData::refund_id` and `RefundFlowData::connector_request_reference_id` are both
/// `merchant_refund_id`; the second is checked only so that a caller who populated one and not
/// the other still works.
fn refund_reference(request: &RefundsData, common: &RefundFlowData) -> Option<String> {
    [
        request.refund_id.as_str(),
        common.connector_request_reference_id.as_str(),
    ]
    .into_iter()
    .find(|value| !value.is_empty())
    .map(str::to_string)
}

/// [`refund_reference`] as a hard requirement, used when the request body is built.
fn resolve_refund_reference(
    request: &RefundsData,
    common: &RefundFlowData,
) -> Result<String, error_stack::Report<errors::IntegrationError>> {
    refund_reference(request, common).ok_or_else(|| {
        error_stack::report!(errors::IntegrationError::MissingRequiredField {
            field_name: "merchant_refund_id",
            context: errors::IntegrationErrorContext {
                suggested_action: Some(
                    "Send merchant_refund_id on every Ilixium refund request.".to_string(),
                ),
                doc_url: Some("https://docs.ilixium.com/docs/direct/reconciliation".to_string()),
                additional_context: Some(
                    "POST /direct/refund mints no refund identifier: its response echoes the \
                     original payment's merchantRef and gatewayRef, and status.operationRef — the \
                     only refund-unique value — is documented as 'available soon'. When Ilixium \
                     omits operationRef, merchant_refund_id is the only value that can identify \
                     this refund, so the request is refused rather than performed with no way to \
                     report a connector_refund_id."
                        .to_string(),
                ),
            },
        })
    })
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        IlixiumRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for IlixiumRefundRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: IlixiumRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;
        let common = &router_data.resource_common_data;

        let auth = IlixiumAuthType::try_from(&router_data.connector_config)?;

        resolve_refund_reference(request, common)?;

        let amount = item
            .connector
            .amount_converter
            .convert(request.minor_refund_amount, request.currency)
            .change_context(errors::IntegrationError::AmountConversionFailed {
                context: errors::IntegrationErrorContext {
                    suggested_action: None,
                    doc_url: None,
                    additional_context: Some(format!(
                        "Failed to convert minor_refund_amount {} {} into Ilixium's \
                         transaction.amount (minor units, digits only, sent as a JSON string).",
                        request.minor_refund_amount.get_amount_as_i64(),
                        request.currency
                    )),
                },
            })?;

        Ok(Self {
            version: ILIXIUM_MESSAGE_VERSION,
            transaction: IlixiumRefundTransaction {
                merchant_ref: resolve_original_merchant_ref(
                    request.connector_order_id.as_deref(),
                    &request.connector_transaction_id,
                )?,
                amount,
                currency: request.currency,
            },
            merchant: IlixiumMerchant::from(&auth),
        })
    }
}

/// Request-level outcome. **Every** business failure is returned as HTTP 200 with one of these
/// codes, so this — never the HTTP status — is what the connector branches on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IlixiumStatusCode {
    Success,
    Declined,
    Rejected,
    Error,
    Cancelled,
    Pending,
    Resubmission,
    /// Any code Ilixium adds that this integration does not yet know about.
    #[serde(other)]
    Unknown,
}

/// Operation type echoed in the response. `/direct/auth` answers `AUTH_CAP` when
/// `deferredCapture` was false and `AUTH` when it was true; `/direct/capture` answers
/// `CAPTURE` and `/direct/reversal` answers `REVERSAL`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IlixiumOperationType {
    Auth,
    AuthCap,
    Capture,
    Reversal,
    Refund,
    Credit,
    /// The wire value is pinned explicitly rather than derived from the identifier: the
    /// enum's `rename_all = "SCREAMING_SNAKE_CASE"` would turn any respelling of "Threed"
    /// into a *different* string (`THREE_D_SECURE_COMPLETE`), which Ilixium never sends, and
    /// the variant would silently fall through to `Unknown` instead of failing loudly.
    #[serde(rename = "THREED_SECURE_COMPLETE")]
    ThreeDsSecureComplete,
    Payment,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IlixiumAttemptCode {
    Success,
    Pending,
    Rejected,
    Error,
    Cancelled,
    Declined,
    Cvv2CheckFailed,
    AvsAddressCheckFailed,
    AvsPostcodeCheckFailed,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IlixiumThreeDsStatus {
    Pending,
    AuthenticationSuccessful,
    AuthenticationFailed,
    NotEnrolled,
    AuthenticationUnavailable,
    AuthenticationAttempted,
    AcsError,
    InternalError,
    #[serde(other)]
    Unknown,
}

/// A single entry of `status.reasons.reason`. The OpenAPI schema types these as strings
/// (`VA22`, `121`), but the legacy 3DS example emits a bare number (`502`), so accept both.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum IlixiumReasonCode {
    Text(String),
    Numeric(i64),
}

impl std::fmt::Display for IlixiumReasonCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text(value) => write!(f, "{value}"),
            Self::Numeric(value) => write!(f, "{value}"),
        }
    }
}

/// `status.reasons` shape. Tech spec CONFLICT: the OpenAPI schema and the `/docs/api/*`
/// examples wrap the codes in an object (`{"reason": ["VA22"]}`), while `/docs/response-codes/`
/// shows a bare array and the legacy 3DS page shows a scalar. The schema is authoritative, but
/// all three shapes are accepted here so a stale wire format can never turn a decline into an
/// unparsable response.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum IlixiumReasons {
    Wrapped { reason: IlixiumReasonList },
    Bare(IlixiumReasonList),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum IlixiumReasonList {
    Many(Vec<IlixiumReasonCode>),
    One(IlixiumReasonCode),
}

impl IlixiumReasonList {
    fn codes(&self) -> Vec<String> {
        match self {
            Self::Many(codes) => codes.iter().map(ToString::to_string).collect(),
            Self::One(code) => vec![code.to_string()],
        }
    }
}

impl IlixiumReasons {
    pub fn codes(&self) -> Vec<String> {
        match self {
            Self::Wrapped { reason } => reason.codes(),
            Self::Bare(list) => list.codes(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IlixiumStatus {
    pub code: IlixiumStatusCode,
    pub message: Option<String>,
    pub reasons: Option<IlixiumReasons>,
    pub timestamp: Option<String>,
    #[serde(rename = "operationRef")]
    pub operation_ref: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IlixiumTransactionResponse {
    #[serde(rename = "merchantRef")]
    pub merchant_ref: Option<String>,
    /// Gateway-assigned unique reference — this is the connector transaction id.
    #[serde(rename = "gatewayRef")]
    pub gateway_ref: Option<String>,
    pub currency: Option<String>,
    #[serde(rename = "transactionType")]
    pub transaction_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IlixiumCardResponse {
    #[serde(rename = "acquirerRef")]
    pub acquirer_ref: Option<String>,
    #[serde(rename = "cardBin")]
    pub card_bin: Option<String>,
    #[serde(rename = "cardLastFour")]
    pub card_last_four: Option<String>,
    #[serde(rename = "cardType")]
    pub card_type: Option<String>,
    /// Present only when the transaction was successfully authorised.
    #[serde(rename = "authCode")]
    pub auth_code: Option<String>,
    pub cvv: Option<String>,
    #[serde(rename = "avsAddress")]
    pub avs_address: Option<String>,
    #[serde(rename = "avsPostcode")]
    pub avs_postcode: Option<String>,
    #[serde(rename = "threeDSecureStatus")]
    pub three_d_secure_status: Option<IlixiumThreeDsStatus>,
    /// The ACS form `action` for the browser redirect.
    #[serde(rename = "threeDSecureAcsUrl")]
    pub three_d_secure_acs_url: Option<String>,
    /// POSTed to the ACS as `MD`.
    #[serde(rename = "threeDSecureMd")]
    pub three_d_secure_md: Option<Secret<String>>,
    /// POSTed to the ACS as `PaReq`.
    #[serde(rename = "threeDSecurePaReq")]
    pub three_d_secure_pa_req: Option<Secret<String>>,
    #[serde(rename = "threeDSecureVersion")]
    pub three_d_secure_version: Option<String>,
    pub iso8583code: Option<String>,
}

/// Deliberately omits `amount`/`currency`: they are unused by the Authorize mapping and the
/// vendor's examples type them inconsistently (quoted string in `transaction`, bare number
/// here), so parsing them would add a failure mode for no benefit.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IlixiumPaymentAttempt {
    pub order: Option<i32>,
    pub timestamp: Option<String>,
    pub code: Option<IlixiumAttemptCode>,
    pub message: Option<String>,
    #[serde(rename = "operationRef")]
    pub operation_ref: Option<String>,
    #[serde(rename = "paymentMethodType")]
    pub payment_method_type: Option<String>,
    #[serde(rename = "cardResponse")]
    pub card_response: Option<IlixiumCardResponse>,
}

/// Tech spec CONFLICT: the OpenAPI schema types `paymentAttempt` as an array, but the legacy
/// 3DS documentation page renders it as a single object. Both are accepted.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum IlixiumPaymentAttempts {
    Many(Vec<IlixiumPaymentAttempt>),
    One(Box<IlixiumPaymentAttempt>),
}

impl IlixiumPaymentAttempts {
    /// The most recent attempt: highest `order`, falling back to the last element when the
    /// platform omits `order`.
    pub fn latest(&self) -> Option<&IlixiumPaymentAttempt> {
        match self {
            Self::One(attempt) => Some(attempt.as_ref()),
            Self::Many(attempts) => attempts
                .iter()
                .max_by_key(|attempt| attempt.order.unwrap_or_default())
                .or_else(|| attempts.last()),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IlixiumPaymentHistory {
    #[serde(rename = "paymentAttempt")]
    pub payment_attempt: Option<IlixiumPaymentAttempts>,
}

/// The shared `paymentResponse` envelope. `/direct/auth`, `/direct/threedcomplete` and
/// `/direct/capture` return byte-for-byte the same schema, so both Authorize legs and Capture
/// deserialise into this one type.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IlixiumPaymentResponse {
    pub version: Option<i32>,
    #[serde(rename = "type")]
    pub operation_type: Option<IlixiumOperationType>,
    pub transaction: Option<IlixiumTransactionResponse>,
    pub status: IlixiumStatus,
    #[serde(rename = "paymentHistory")]
    pub payment_history: Option<IlixiumPaymentHistory>,
}

/// `/direct/capture` returns the same `paymentResponse` envelope as `/direct/auth`. The alias
/// exists because the connector macros mint one `…Templating` marker type per named response
/// body, so two flows cannot both name `IlixiumPaymentResponse` directly.
pub type IlixiumCaptureResponse = IlixiumPaymentResponse;

/// `/direct/reversal` returns the same `paymentResponse` envelope, with `type` = `REVERSAL`.
/// A distinct alias for the same reason [`IlixiumCaptureResponse`] is one.
pub type IlixiumVoidResponse = IlixiumPaymentResponse;

/// The **PreAuthenticate** leg posts the very same `/direct/auth` body as Authorize does.
///
/// An alias rather than a reuse of the ident: `create_all_prerequisites!` mints one
/// `<Ident>Templating` marker struct per named request ident, so a second flow cannot name
/// `IlixiumPaymentsRequest` directly without defining that marker twice.
pub type IlixiumPreAuthenticateRequest<T> = IlixiumPaymentsRequest<T>;

/// `/direct/auth` answers the same `paymentResponse` envelope on the PreAuthenticate leg as it
/// does on Authorize. A distinct alias for the same macro reason as
/// [`IlixiumPreAuthenticateRequest`].
pub type IlixiumPreAuthenticateResponse = IlixiumPaymentResponse;

/// `/direct/refund` returns the same `paymentResponse` envelope, with `type` = `REFUND`.
/// A distinct alias for the same reason [`IlixiumCaptureResponse`] is one.
///
/// Note what this envelope does **not** contain: any refund-specific identifier.
/// `transaction.merchantRef` and `transaction.gatewayRef` both echo the *original payment* —
/// the vendor's own published history example shows an `AUTH_CAP` and a `REFUND` sharing both
/// values — and the schema has no `refundId` field. See [`refund_identifier`].
pub type IlixiumRefundResponse = IlixiumPaymentResponse;

impl IlixiumPaymentResponse {
    pub fn latest_attempt(&self) -> Option<&IlixiumPaymentAttempt> {
        self.payment_history
            .as_ref()
            .and_then(|history| history.payment_attempt.as_ref())
            .and_then(IlixiumPaymentAttempts::latest)
    }

    pub fn card_response(&self) -> Option<&IlixiumCardResponse> {
        self.latest_attempt()
            .and_then(|attempt| attempt.card_response.as_ref())
    }

    pub fn gateway_ref(&self) -> Option<String> {
        self.transaction
            .as_ref()
            .and_then(|transaction| transaction.gateway_ref.clone())
    }

    pub fn merchant_ref(&self) -> Option<String> {
        self.transaction
            .as_ref()
            .and_then(|transaction| transaction.merchant_ref.clone())
    }

    pub fn reason_codes(&self) -> Vec<String> {
        self.status
            .reasons
            .as_ref()
            .map(IlixiumReasons::codes)
            .unwrap_or_default()
    }

    /// The 3DS branch is gated on the *structure* of the response, never on a reason code.
    ///
    /// Tech spec CONFLICT #7: `/docs/response-codes/` documents `9` as "3D Secure Required"
    /// while the 3DS guide's own PENDING example carries `502`. Both are unreliable, so the
    /// decision here is `status.code == PENDING` **and** `threeDSecureStatus == PENDING`
    /// **and** an ACS URL is actually present — the three things that must all hold for a
    /// redirect to be possible at all.
    pub fn three_ds_acs_url(&self) -> Option<&str> {
        if self.status.code != IlixiumStatusCode::Pending {
            return None;
        }
        let card_response = self.card_response()?;
        if card_response.three_d_secure_status != Some(IlixiumThreeDsStatus::Pending) {
            return None;
        }
        card_response
            .three_d_secure_acs_url
            .as_deref()
            .filter(|url| !url.is_empty())
    }
}

/// Maps the request-level `status.code` onto a UCS attempt status.
///
/// `SUCCESS` splits on the operation type Ilixium echoes back: `AUTH_CAP` means the funds were
/// captured in the same call (`Charged`), `AUTH` means authorised-only (`Authorized`). When the
/// platform omits `type`, fall back to what the request asked for.
fn map_attempt_status(
    response: &IlixiumPaymentResponse,
    requested_auto_capture: bool,
) -> AttemptStatus {
    match response.status.code {
        IlixiumStatusCode::Success => match response.operation_type {
            Some(IlixiumOperationType::AuthCap) => AttemptStatus::Charged,
            Some(IlixiumOperationType::Auth) => AttemptStatus::Authorized,
            _ => {
                if requested_auto_capture {
                    AttemptStatus::Charged
                } else {
                    AttemptStatus::Authorized
                }
            }
        },
        IlixiumStatusCode::Pending => {
            if response.three_ds_acs_url().is_some() {
                AttemptStatus::AuthenticationPending
            } else {
                AttemptStatus::Pending
            }
        }
        IlixiumStatusCode::Cancelled => AttemptStatus::Voided,
        IlixiumStatusCode::Declined | IlixiumStatusCode::Rejected | IlixiumStatusCode::Error => {
            AttemptStatus::Failure
        }
        IlixiumStatusCode::Resubmission | IlixiumStatusCode::Unknown => AttemptStatus::Pending,
    }
}

/// Maps a `/direct/capture` response onto a UCS attempt status.
///
/// Kept separate from [`map_attempt_status`] because the same `status.code` means something
/// different here: `SUCCESS` on a capture is unambiguously `Charged` (there is no
/// authorised-only outcome to distinguish), and a failure is a *capture* failure — the
/// authorisation itself is untouched and can still be captured again or reversed, which
/// `CaptureFailed` conveys and the generic `Failure` does not.
fn map_capture_status(response: &IlixiumPaymentResponse) -> AttemptStatus {
    match response.status.code {
        IlixiumStatusCode::Success => AttemptStatus::Charged,
        IlixiumStatusCode::Pending => AttemptStatus::CaptureInitiated,
        IlixiumStatusCode::Cancelled => AttemptStatus::Voided,
        IlixiumStatusCode::Declined | IlixiumStatusCode::Rejected | IlixiumStatusCode::Error => {
            AttemptStatus::CaptureFailed
        }
        IlixiumStatusCode::Resubmission | IlixiumStatusCode::Unknown => {
            AttemptStatus::CaptureInitiated
        }
    }
}

/// Maps a `/direct/reversal` response onto a UCS attempt status.
///
/// A sibling of [`map_capture_status`] rather than a reuse of it: the same `status.code` carries
/// a different meaning on a reversal. `SUCCESS` is `Voided` (the ring-fenced funds are released
/// back to the cardholder), and a failure is a *void* failure — the authorisation survives and
/// can still be captured, or reversed again once whatever blocked it clears — which is what
/// `VoidFailed` conveys and `CaptureFailed`/`Failure` do not.
fn map_void_status(response: &IlixiumPaymentResponse) -> AttemptStatus {
    match response.status.code {
        IlixiumStatusCode::Success => AttemptStatus::Voided,
        IlixiumStatusCode::Cancelled => AttemptStatus::Voided,
        IlixiumStatusCode::Pending => AttemptStatus::VoidInitiated,
        IlixiumStatusCode::Declined | IlixiumStatusCode::Rejected | IlixiumStatusCode::Error => {
            AttemptStatus::VoidFailed
        }
        IlixiumStatusCode::Resubmission | IlixiumStatusCode::Unknown => {
            AttemptStatus::VoidInitiated
        }
    }
}

/// Plain-language expansion of the failure codes `/direct/capture` can return.
///
/// Ilixium's `status.message` is generic ("Validation failed", "Operation rejected"); the
/// actionable detail is the numeric code in `status.reasons.reason`, and these six are the ones
/// the capture endpoint documents. Note that 112 ("already captured") is still reported as a
/// failed capture rather than as `Charged`: it says a capture happened, not that *this* request
/// captured anything, and treating it as success would mask a duplicate-capture bug.
fn capture_failure_hint(code: &str) -> Option<&'static str> {
    match code {
        "103" => Some("the capture currency does not match the original transaction's currency"),
        "104" => Some(
            "no transaction matching transaction.merchantRef was found on this merchant account",
        ),
        "105" => {
            Some("the original transaction was not successful, so there is nothing to capture")
        }
        "108" => Some(
            "the original transaction was not of type AUTH — only an authorisation sent with \
             deferredCapture can be captured",
        ),
        "112" => Some("the transaction has already been captured"),
        "117" => Some(
            "another operation is already in progress on this transaction — Ilixium permits \
             exactly one at a time",
        ),
        _ => None,
    }
}

/// [`build_error_response`] with the documented capture codes spelled out in `reason`. Every
/// hint is prefixed with its own code, so nothing the generic builder would have reported is
/// lost.
fn build_capture_error_response(
    response: &IlixiumPaymentResponse,
    status: AttemptStatus,
    http_code: u16,
) -> ErrorResponse {
    let mut error = build_error_response(response, status, http_code);
    let hints: Vec<String> = response
        .reason_codes()
        .iter()
        .filter_map(|code| capture_failure_hint(code).map(|hint| format!("{code}: {hint}")))
        .collect();
    if !hints.is_empty() {
        error.reason = Some(hints.join("; "));
    }
    error
}

/// Plain-language expansion of the failure codes `/direct/reversal` can return.
///
/// A sibling of [`capture_failure_hint`], not an extension of it: the two endpoints share only
/// 103/104/105/117, and the rest of each set is meaningless on the other flow. In particular
/// **142** — the partial-reversal rejection — has no capture counterpart at all (capture has no
/// "amount does not match original" code, because partial capture is permitted), and 108
/// ("cannot CAPTURE") is capture-only.
fn reversal_failure_hint(code: &str) -> Option<&'static str> {
    match code {
        "103" => Some("the reversal currency does not match the original transaction's currency"),
        "104" => Some(
            "no transaction matching transaction.merchantRef was found on this merchant account",
        ),
        "105" => {
            Some("the original transaction was not authorised, so there is nothing to reverse")
        }
        "106" => Some("the transaction has already been reversed"),
        "109" => Some("the original transaction was not of a reversible type"),
        "110" => Some(
            "the transaction has already been settled — a refund is required instead of a \
             reversal",
        ),
        "112" => Some(
            "the transaction has already been captured, and the reversal falls outside the \
             reversal-after-capture window configured for this merchant — a refund is required \
             instead",
        ),
        "117" => Some(
            "another operation is already in progress on this transaction — Ilixium permits \
             exactly one at a time",
        ),
        "142" => Some(
            "the reversal amount does not match the original transaction — Ilixium does not \
             support partial voids/reversals, so transaction.amount must equal the original \
             amount exactly",
        ),
        _ => None,
    }
}

/// [`build_error_response`] with the documented reversal codes spelled out in `reason`. Every
/// hint is prefixed with its own code, so nothing the generic builder would have reported is
/// lost.
fn build_void_error_response(
    response: &IlixiumPaymentResponse,
    status: AttemptStatus,
    http_code: u16,
) -> ErrorResponse {
    let mut error = build_error_response(response, status, http_code);
    let hints: Vec<String> = response
        .reason_codes()
        .iter()
        .filter_map(|code| reversal_failure_hint(code).map(|hint| format!("{code}: {hint}")))
        .collect();
    if !hints.is_empty() {
        error.reason = Some(hints.join("; "));
    }
    error
}

/// Maps a `/direct/refund` response onto a UCS **refund** status.
///
/// A *fourth sibling* of [`map_attempt_status`] / [`map_capture_status`] / [`map_void_status`],
/// not a reuse of any of them, for two reasons. First, the target enum is different:
/// [`RefundStatus`], not `AttemptStatus` — a refund outcome says nothing about the payment
/// attempt, which stays `Charged` throughout. Second, the same `status.code` means a third thing
/// again: `SUCCESS` is a completed refund, and a failure here leaves the payment intact and
/// still refundable for a *different* amount (a 125 "exceeds original" rejection is retryable
/// with a smaller value), which `RefundStatus::Failure` conveys without implying anything about
/// the payment.
fn map_refund_status(response: &IlixiumPaymentResponse) -> RefundStatus {
    match response.status.code {
        IlixiumStatusCode::Success => RefundStatus::Success,
        IlixiumStatusCode::Pending => RefundStatus::Pending,
        IlixiumStatusCode::Cancelled
        | IlixiumStatusCode::Declined
        | IlixiumStatusCode::Rejected
        | IlixiumStatusCode::Error => RefundStatus::Failure,
        IlixiumStatusCode::Resubmission | IlixiumStatusCode::Unknown => RefundStatus::Pending,
    }
}

/// Plain-language expansion of the failure codes `/direct/refund` can return.
///
/// A sibling of [`capture_failure_hint`] / [`reversal_failure_hint`], not an extension of
/// either: the refund set shares only 103/104/105/117 with them and diverges everywhere else.
/// 107/120/124/125 and the 204 account limit have no capture or reversal counterpart, while 108
/// (capture-only) and 106/109/110/142 (reversal-only) are meaningless here.
///
/// 111 is the one that most often surprises an integrator: an *uncaptured* authorisation cannot
/// be refunded at all — it must be reversed (Void) instead.
fn refund_failure_hint(code: &str) -> Option<&'static str> {
    match code {
        "103" => Some("the refund currency does not match the original transaction's currency"),
        "104" => Some(
            "no transaction matching transaction.merchantRef was found on this merchant account",
        ),
        "105" => Some("the original transaction was not successful, so there is nothing to refund"),
        "107" => Some("the original transaction has already been refunded"),
        "111" => Some(
            "the original transaction has not been captured — capture it first, or reverse the \
             authorisation (Void) instead of refunding it",
        ),
        "117" => Some(
            "another operation is already in progress on this transaction — Ilixium permits \
             exactly one at a time",
        ),
        "120" => Some(
            "the original payment is not refundable through the Direct API — it used a payment \
             method the Direct API cannot refund",
        ),
        "121" => Some("the supplied payment method details are invalid"),
        "124" => Some(
            "the transaction has already been fully refunded — no more refunds can be made \
             against it",
        ),
        "125" => Some(
            "the refund amount would take the total refunded above the original authorised \
             amount — refunds accumulate, so only the unrefunded remainder can still be refunded",
        ),
        "204" => Some("the account's transaction amount limit has been reached"),
        _ => None,
    }
}

/// The refund counterpart of [`build_capture_error_response`] / [`build_void_error_response`].
///
/// Written out rather than layered on [`build_error_response`] because that builder stamps
/// `attempt_status: FlowStatus::Payment(..)`, and a refund failure must report
/// `FlowStatus::Refund(..)` — the payment attempt itself is untouched by a failed refund.
fn build_refund_error_response(
    response: &IlixiumPaymentResponse,
    status: RefundStatus,
    http_code: u16,
) -> ErrorResponse {
    let reason_codes = response.reason_codes();
    let attempt_message = response
        .latest_attempt()
        .and_then(|attempt| attempt.message.clone());
    let message = response
        .status
        .message
        .clone()
        .or(attempt_message)
        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string());

    let hints: Vec<String> = reason_codes
        .iter()
        .filter_map(|code| refund_failure_hint(code).map(|hint| format!("{code}: {hint}")))
        .collect();

    ErrorResponse {
        status_code: http_code,
        code: reason_codes
            .first()
            .cloned()
            .unwrap_or_else(|| format!("{:?}", response.status.code).to_uppercase()),
        message,
        reason: if !hints.is_empty() {
            Some(hints.join("; "))
        } else if reason_codes.is_empty() {
            response.status.message.clone()
        } else {
            Some(reason_codes.join(", "))
        },
        attempt_status: Some(domain_types::router_data::FlowStatus::Refund(status)),
        connector_transaction_id: response.gateway_ref(),
        network_decline_code: None,
        network_advice_code: None,
        network_error_message: None,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
}

/// Resolves `RefundsResponseData::connector_refund_id`.
///
/// **Ilixium mints no refund identifier.** `transaction.merchantRef` and
/// `transaction.gatewayRef` in a refund response both echo the *original payment* — the vendor's
/// published `/history/operations` example shows an `AUTH_CAP` and a `REFUND` sharing both values
/// — and `paymentResponse` has no `refundId` field. `/docs/direct/reconciliation` says so
/// outright: "The `merchantRef` field is designed to only identify the transaction, not the
/// operations within it."
///
/// The resolution order is therefore:
///
/// 1. **`status.operationRef`** — a ULID that "will uniquely identify each operation" and the
///    only refund-unique value the API defines. The same page warns it "will be available soon",
///    so it may simply be absent.
/// 2. **`paymentHistory.paymentAttempt[latest].operationRef`** — the same value, echoed
///    per attempt; checked in case the platform populates one place before the other.
/// 3. **The refund's own UCS reference** (`merchant_refund_id`, validated in
///    [`resolve_refund_reference`] when the request was built). Deterministic, unique per refund,
///    and honest about its origin.
///
/// What is deliberately *not* used is the payment's `merchantRef` or `gatewayRef`: reusing either
/// would give every partial refund on one payment the same `connector_refund_id`.
///
/// **Known limitation:** until `operationRef` goes live, N partial refunds against one payment are
/// distinguished only by the UCS-side reference. Ilixium itself cannot tell them apart —
/// `/history/operations` returns N `type: REFUND` entries differing only by `entryDate`,
/// `processedDate` and `amount` — so an id from step 3 cannot be resolved back through any Ilixium
/// endpoint.
fn refund_identifier(
    response: &IlixiumPaymentResponse,
    refund_reference: Option<String>,
    http_code: u16,
) -> Result<String, error_stack::Report<errors::ConnectorError>> {
    let operation_ref = response
        .status
        .operation_ref
        .clone()
        .or_else(|| {
            response
                .latest_attempt()
                .and_then(|attempt| attempt.operation_ref.clone())
        })
        .filter(|value| !value.is_empty());

    if let Some(operation_ref) = operation_ref {
        return Ok(operation_ref);
    }

    tracing::debug!(
        "Ilixium returned no status.operationRef on a refund response; falling back to the UCS \
         refund reference as connector_refund_id. Multiple partial refunds on one payment are \
         not individually distinguishable at Ilixium until operationRef goes live."
    );

    refund_reference.ok_or_else(|| {
        error_stack::report!(errors::ConnectorError::ResponseDeserializationFailed {
            context: errors::ResponseTransformationErrorContext {
                http_status_code: Some(http_code),
                additional_context: Some(
                    "Ilixium's refund response carries no status.operationRef and UCS supplied \
                     no merchant_refund_id, so this refund has no identifier: the response's \
                     merchantRef and gatewayRef both echo the original payment and would collide \
                     across partial refunds."
                        .to_string(),
                ),
            },
        })
    })
}

/// Builds the ACS auto-POST form. Ilixium never receives the return URL — the merchant owns
/// the `TermUrl` — so the connector assembles the form itself (tech spec UNDECIDED #5,
/// option (a)). The 1.x MPI field names `MD`/`PaReq` are retained by Ilixium even for 3DS2.
///
/// `term_url` is supplied by the caller rather than read off the request, because the two flows
/// that can receive a 3DS challenge hold it under different names and types: `Authorize` has
/// `complete_authorize_url`/`router_return_url` (`Option<String>`), `PreAuthenticate` has
/// `continue_redirection_url`/`router_return_url` (`Option<Url>`).
fn build_three_ds_redirect_form(
    response: &IlixiumPaymentResponse,
    acs_url: &str,
    term_url: Option<String>,
    http_code: u16,
) -> Result<RedirectForm, error_stack::Report<errors::ConnectorError>> {
    let deserialization_error = |detail: &str| {
        error_stack::report!(errors::ConnectorError::ResponseDeserializationFailed {
            context: errors::ResponseTransformationErrorContext {
                http_status_code: Some(http_code),
                additional_context: Some(detail.to_string()),
            },
        })
    };

    let card_response = response.card_response().ok_or_else(|| {
        deserialization_error(
            "Ilixium returned PENDING with an ACS URL but no cardResponse to read \
             threeDSecureMd / threeDSecurePaReq from",
        )
    })?;

    let md = card_response.three_d_secure_md.as_ref().ok_or_else(|| {
        deserialization_error("Ilixium 3DS PENDING response is missing threeDSecureMd")
    })?;
    let pa_req = card_response
        .three_d_secure_pa_req
        .as_ref()
        .ok_or_else(|| {
            deserialization_error("Ilixium 3DS PENDING response is missing threeDSecurePaReq")
        })?;

    let term_url = term_url.ok_or_else(|| {
        error_stack::report!(errors::ConnectorError::ResponseHandlingFailed {
            context: errors::ResponseTransformationErrorContext {
                http_status_code: Some(http_code),
                additional_context: Some(
                    "Ilixium 3DS requires a TermUrl for the ACS form, but neither the \
                     completion URL nor router_return_url was supplied"
                        .to_string(),
                ),
            },
        })
    })?;

    let mut form_fields = std::collections::HashMap::with_capacity(3);
    form_fields.insert("MD".to_string(), md.peek().to_owned());
    form_fields.insert("PaReq".to_string(), pa_req.peek().to_owned());
    form_fields.insert("TermUrl".to_string(), term_url);

    Ok(RedirectForm::Form {
        endpoint: acs_url.to_string(),
        method: common_utils::request::Method::Post,
        form_fields,
    })
}

fn build_error_response(
    response: &IlixiumPaymentResponse,
    status: AttemptStatus,
    http_code: u16,
) -> ErrorResponse {
    let reason_codes = response.reason_codes();
    let attempt_message = response
        .latest_attempt()
        .and_then(|attempt| attempt.message.clone());
    let message = response
        .status
        .message
        .clone()
        .or(attempt_message)
        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string());

    ErrorResponse {
        status_code: http_code,
        code: reason_codes
            .first()
            .cloned()
            .unwrap_or_else(|| format!("{:?}", response.status.code).to_uppercase()),
        message,
        reason: if reason_codes.is_empty() {
            response.status.message.clone()
        } else {
            Some(reason_codes.join(", "))
        },
        attempt_status: Some(domain_types::router_data::FlowStatus::Payment(status)),
        connector_transaction_id: response.gateway_ref(),
        network_decline_code: None,
        network_advice_code: None,
        network_error_message: None,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
}

impl<T: PaymentMethodDataTypes>
    TryFrom<crate::types::ResponseRouterData<IlixiumPaymentResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: crate::types::ResponseRouterData<IlixiumPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let status = map_attempt_status(&response, item.router_data.request.is_auto_capture());

        let redirection_data = match response.three_ds_acs_url() {
            Some(acs_url) => {
                let request = &item.router_data.request;
                Some(Box::new(build_three_ds_redirect_form(
                    &response,
                    acs_url,
                    request
                        .complete_authorize_url
                        .clone()
                        .or_else(|| request.router_return_url.clone()),
                    item.http_code,
                )?))
            }
            None => None,
        };

        let payments_response = if status == AttemptStatus::Failure {
            Err(build_error_response(&response, status, item.http_code))
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: response
                    .gateway_ref()
                    .map(ResponseId::ConnectorTransactionId)
                    .unwrap_or(ResponseId::NoResponseId),
                redirection_data,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: response.merchant_ref(),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            })
        };

        Ok(Self {
            response: payments_response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

/// `POST /direct/auth` response on the **PreAuthenticate** leg.
///
/// The same `paymentResponse` envelope and the same status mapping the Authorize leg uses — this
/// *is* the authorisation, so `map_attempt_status` applies unchanged. Only the response variant
/// differs.
///
/// Note this leg is terminal in two of its three outcomes. Ilixium decides at response time
/// whether to challenge, so a `SUCCESS` here means the payment is already charged and there is no
/// second leg. HS suppresses the follow-up Authorize (its `should_continue` default is `false`)
/// and finalises the attempt from the status set below, so nothing further is required here to
/// avoid a duplicate `/direct/auth` — see response code 102, "Duplicate Merchant Ref".
impl<T: PaymentMethodDataTypes>
    TryFrom<crate::types::ResponseRouterData<IlixiumPreAuthenticateResponse, Self>>
    for RouterDataV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: crate::types::ResponseRouterData<IlixiumPreAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let requested_auto_capture = item.router_data.request.is_auto_capture().unwrap_or(true);
        let status = map_attempt_status(&response, requested_auto_capture);

        let redirection_data = match response.three_ds_acs_url() {
            Some(acs_url) => {
                let request = &item.router_data.request;
                Some(Box::new(build_three_ds_redirect_form(
                    &response,
                    acs_url,
                    request
                        .continue_redirection_url
                        .as_ref()
                        .or(request.router_return_url.as_ref())
                        .map(ToString::to_string),
                    item.http_code,
                )?))
            }
            None => None,
        };

        let payments_response = if status == AttemptStatus::Failure {
            Err(build_error_response(&response, status, item.http_code))
        } else {
            Ok(PaymentsResponseData::PreAuthenticateResponse {
                resource_id: Some(
                    response
                        .gateway_ref()
                        .map(ResponseId::ConnectorTransactionId)
                        .unwrap_or(ResponseId::NoResponseId),
                ),
                authentication_data: None,
                redirection_data,
                connector_response_reference_id: response.merchant_ref(),
                status_code: item.http_code,
            })
        };

        Ok(Self {
            response: payments_response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl TryFrom<crate::types::ResponseRouterData<IlixiumVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: crate::types::ResponseRouterData<IlixiumVoidResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let status = map_void_status(&response);

        let payments_response = if status == AttemptStatus::VoidFailed {
            Err(build_void_error_response(&response, status, item.http_code))
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    response.gateway_ref().unwrap_or_else(|| {
                        item.router_data.request.connector_transaction_id.clone()
                    }),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: response.merchant_ref(),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            })
        };

        Ok(Self {
            response: payments_response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl TryFrom<crate::types::ResponseRouterData<IlixiumCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: crate::types::ResponseRouterData<IlixiumCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let status = map_capture_status(&response);

        let payments_response = if status == AttemptStatus::CaptureFailed {
            Err(build_capture_error_response(
                &response,
                status,
                item.http_code,
            ))
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: response
                    .gateway_ref()
                    .map(ResponseId::ConnectorTransactionId)
                    .unwrap_or_else(|| item.router_data.request.connector_transaction_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: response.merchant_ref(),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            })
        };

        Ok(Self {
            response: payments_response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl TryFrom<crate::types::ResponseRouterData<IlixiumRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: crate::types::ResponseRouterData<IlixiumRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let status = map_refund_status(&response);

        let refunds_response = if status == RefundStatus::Failure {
            Err(build_refund_error_response(
                &response,
                status,
                item.http_code,
            ))
        } else {
            Ok(RefundsResponseData {
                connector_refund_id: refund_identifier(
                    &response,
                    refund_reference(
                        &item.router_data.request,
                        &item.router_data.resource_common_data,
                    ),
                    item.http_code,
                )?,
                refund_status: status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            })
        };

        Ok(Self {
            response: refunds_response,
            resource_common_data: RefundFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

/// The maximum period `/history/operations` accepts, in hours. Exceeding it is a validation
/// rejection (`VA73` family), not a truncation.
const HISTORY_MAX_WINDOW_HOURS: i64 = 24;

/// `connector_feature_data` keys through which a caller can anchor the history window on the
/// payment's own creation time instead of on "now". See [`resolve_history_window`].
const HISTORY_PERIOD_START_KEYS: [&str; 2] =
    ["ilixium_history_period_start", "history_period_start"];

/// Renders one period bound in the *only* shape `historyOperationsRequest` accepts:
/// `yyyy-MM-ddTHH:mm:ssZ`, with a literal `Z` and **no fractional seconds**.
///
/// The instant is first shifted to UTC, because the literal `Z` in the pattern is an assertion
/// about the offset, not a formatting directive — rendering a `+05:30` wall clock under a `Z`
/// suffix would silently shift the window by the offset.
fn format_history_period(
    instant: OffsetDateTime,
) -> Result<String, error_stack::Report<errors::IntegrationError>> {
    let format =
        time::macros::format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]Z");
    instant
        .to_offset(UtcOffset::UTC)
        .format(&format)
        .change_context(errors::IntegrationError::RequestEncodingFailed {
            context: errors::IntegrationErrorContext {
                suggested_action: None,
                doc_url: Some("https://docs.ilixium.com/docs/direct/history".to_string()),
                additional_context: Some(
                    "Failed to render a POST /history/operations period bound as \
                     yyyy-MM-ddTHH:mm:ssZ."
                        .to_string(),
                ),
            },
        })
}

/// Reads the caller-supplied window anchor out of `PaymentsSyncData::connector_feature_data`.
///
/// Accepted as an RFC 3339 instant (`2025-12-09T11:56:11Z`, or with an offset / fractional
/// seconds — this is *input* to the connector, not the wire format, so it is parsed leniently and
/// re-rendered by [`format_history_period`]). A value that is present but unparsable is a hard
/// error: silently falling back to "now" would answer a question the caller did not ask.
fn extract_history_period_start(
    feature_data: Option<&common_utils::pii::SecretSerdeValue>,
) -> Result<Option<OffsetDateTime>, error_stack::Report<errors::IntegrationError>> {
    let Some(feature_data) = feature_data else {
        return Ok(None);
    };
    let value = feature_data.clone().expose();
    let Some(raw) = HISTORY_PERIOD_START_KEYS
        .iter()
        .find_map(|key| value.get(*key).and_then(|found| found.as_str()))
    else {
        return Ok(None);
    };

    OffsetDateTime::parse(raw, &Rfc3339)
        .map(Some)
        .change_context(errors::IntegrationError::InvalidDataFormat {
            field_name: "connector_feature_data.ilixium_history_period_start",
            context: errors::IntegrationErrorContext {
                suggested_action: Some(
                    "Supply the start of the /history/operations window as an RFC 3339 instant, \
                     e.g. \"2025-12-09T11:56:11Z\". It should be at or just before the payment's \
                     creation time; Ilixium reports at most 24 hours from that point."
                        .to_string(),
                ),
                doc_url: Some("https://docs.ilixium.com/docs/direct/history".to_string()),
                additional_context: None,
            },
        })
}

/// Picks the `[periodStartDate, periodEndDate]` pair for this sync.
///
/// **Why it is not anchored on the payment automatically.** Neither `PaymentsSyncData` nor
/// `PaymentFlowData` carries a creation timestamp — there is no `created_at`, and the only
/// time-shaped value anywhere on the PSync `RouterDataV2` is whatever the caller puts in
/// `connector_feature_data`. So:
///
/// * **Caller-anchored** — when `connector_feature_data.ilixium_history_period_start` is present,
///   the window is `[start, min(start + 24h, now)]`. This is the accurate mode: a caller that
///   knows when the payment was created can sync it at any age.
/// * **Default** — otherwise `[now - 24h, now]`, the widest window the API allows. A payment
///   whose operations all fall outside it simply will not appear in the report, which surfaces as
///   the explicit "not found in window" error built by [`build_history_not_found_error`] rather
///   than as a guessed status.
///
/// The 24-hour cap is satisfied by construction — the window is *clamped*, never widened — so the
/// platform can never answer `VA73` because of a period this function chose.
fn resolve_history_window(
    feature_data: Option<&common_utils::pii::SecretSerdeValue>,
) -> Result<(OffsetDateTime, OffsetDateTime), error_stack::Report<errors::IntegrationError>> {
    let now = OffsetDateTime::now_utc();
    let max_window = Duration::hours(HISTORY_MAX_WINDOW_HOURS);

    let overflow = |bound: &'static str| {
        error_stack::report!(errors::IntegrationError::InvalidDataFormat {
            field_name: "connector_feature_data.ilixium_history_period_start",
            context: errors::IntegrationErrorContext {
                suggested_action: Some(format!(
                    "The supplied history window start is so far from the representable range \
                     that the {bound} could not be computed. Supply a real payment creation \
                     time."
                )),
                doc_url: Some("https://docs.ilixium.com/docs/direct/history".to_string()),
                additional_context: None,
            },
        })
    };

    match extract_history_period_start(feature_data)? {
        Some(start) => {
            if start >= now {
                return Err(error_stack::report!(
                    errors::IntegrationError::InvalidDataFormat {
                        field_name: "connector_feature_data.ilixium_history_period_start",
                        context: errors::IntegrationErrorContext {
                            suggested_action: Some(
                                "The history window start must be in the past — Ilixium reports \
                                 operations that have already happened. Supply the payment's \
                                 creation time."
                                    .to_string(),
                            ),
                            doc_url: Some(
                                "https://docs.ilixium.com/docs/direct/history".to_string()
                            ),
                            additional_context: None,
                        },
                    }
                ));
            }
            let end = start
                .checked_add(max_window)
                .ok_or_else(|| overflow("period end"))?
                .min(now);
            Ok((start, end))
        }
        None => {
            let start = now
                .checked_sub(max_window)
                .ok_or_else(|| overflow("period start"))?;
            Ok((start, now))
        }
    }
}

/// `historyOperationsRequest.reportFormat`. Sent explicitly because the Direct API defaults to
/// XML, and this integration parses JSON.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IlixiumReportFormat {
    Json,
}

/// `POST /history/operations` body.
///
/// **There is deliberately no `version` field.** `historyOperationsRequest` is the one Ilixium
/// request schema that does not declare one, so sending [`ILIXIUM_MESSAGE_VERSION`] here would be
/// an unknown property on the message *and* would change the bytes the `X-MERCHANT-DIGEST` is
/// computed over.
///
/// There is likewise no place to put the payment's identity: the schema's four properties are
/// exactly the two period bounds, `merchant` and `reportFormat`. Selecting one payment out of the
/// report is [`IlixiumHistoryResponse::latest_payment_operation`]'s job.
#[derive(Debug, Clone, Serialize)]
pub struct IlixiumHistoryRequest {
    /// `yyyy-MM-ddTHH:mm:ssZ` — see [`format_history_period`].
    #[serde(rename = "periodStartDate")]
    pub period_start_date: String,
    /// `yyyy-MM-ddTHH:mm:ssZ`, at most 24 hours after `periodStartDate`.
    #[serde(rename = "periodEndDate")]
    pub period_end_date: String,
    pub merchant: IlixiumMerchant,
    #[serde(rename = "reportFormat")]
    pub report_format: IlixiumReportFormat,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        IlixiumRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for IlixiumHistoryRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: IlixiumRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = IlixiumAuthType::try_from(&router_data.connector_config)?;

        derive_merchant_ref(
            &router_data
                .resource_common_data
                .connector_request_reference_id,
        )?;

        let (period_start, period_end) =
            resolve_history_window(router_data.request.connector_feature_data.as_ref())?;

        Ok(Self {
            period_start_date: format_history_period(period_start)?,
            period_end_date: format_history_period(period_end)?,
            merchant: IlixiumMerchant::from(&auth),
            report_format: IlixiumReportFormat::Json,
        })
    }
}

/// `historyStatus.code` — a **different enum** from [`IlixiumStatusCode`], which is why this is a
/// sibling type rather than a reuse.
///
/// It adds `EXCEPTION` and `VALIDATION_ERRORS` and drops `RESUBMISSION`; deserialising a history
/// envelope through `IlixiumStatusCode` would collapse both new values onto its `#[serde(other)]`
/// arm and lose the distinction between "the query was malformed" and "the operation failed".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IlixiumHistoryStatusCode {
    Success,
    Declined,
    Rejected,
    Error,
    Cancelled,
    Pending,
    /// "An exception has occurred during the processing of the request … please contact support."
    Exception,
    /// "The request was not accepted because it failed validation checks." The `reasons` element
    /// then carries the individual codes (`VA72` = `periodStartDate`, `VA73` = `periodEndDate`).
    ValidationErrors,
    /// Any code Ilixium adds that this integration does not yet know about.
    #[serde(other)]
    Unknown,
}

/// `HistoryTransaction.transactionType` — again a **different enum** from the one every
/// `/direct/*` request and `paymentResponse` uses (`ECOMMERCE` / `MAIL_ORDER` /
/// `TELEPHONE_ORDER`). Reusing [`IlixiumTransactionType`] here would fail to match every value
/// the history report can return.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IlixiumHistoryTransactionType {
    CnpEcommerce,
    CnpMailOrder,
    CnpTelephoneOrder,
    #[serde(other)]
    Unknown,
}

/// `historyStatus`, used at two levels with two different meanings: on the envelope it is the
/// outcome of the *query*, and on an `operation[]` entry it is the outcome of that *operation*.
///
/// Kept separate from [`IlixiumStatus`] purely because of the `code` enum; `reasons` reuses
/// [`IlixiumReasons`], whose shape is identical.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IlixiumHistoryStatus {
    pub code: IlixiumHistoryStatusCode,
    /// The History API does **not** apply ISO-8583 formatting to this field, so the wording
    /// differs from the corresponding `paymentResponse` message. Never string-match across the
    /// two.
    pub message: Option<String>,
    pub reasons: Option<IlixiumReasons>,
    /// `required` in `statusDetails`, yet omitted from the vendor's own success example.
    pub timestamp: Option<String>,
    #[serde(rename = "operationRef")]
    pub operation_ref: Option<String>,
}

impl IlixiumHistoryStatus {
    pub fn reason_codes(&self) -> Vec<String> {
        self.reasons
            .as_ref()
            .map(IlixiumReasons::codes)
            .unwrap_or_default()
    }
}

/// `HistoryTransaction` — a different schema from the `transactionDetails` echoed by
/// `paymentResponse`, and much thinner: there is no `paymentHistory` and no `cardResponse`
/// anywhere in a history entry, so PSync can recover **no** `authCode`, `iso8583code`, 3-D Secure
/// field or card token.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IlixiumHistoryTransaction {
    pub amount: Option<String>,
    pub currency: Option<String>,
    /// The only value that ties a report entry back to a UCS payment.
    #[serde(rename = "merchantRef")]
    pub merchant_ref: Option<String>,
    #[serde(rename = "parentRef")]
    pub parent_ref: Option<String>,
    #[serde(rename = "gatewayRef")]
    pub gateway_ref: Option<String>,
    #[serde(rename = "transactionType")]
    pub transaction_type: Option<IlixiumHistoryTransactionType>,
    #[serde(rename = "recurringMode")]
    pub recurring_mode: Option<String>,
}

/// One `operation[]` entry.
///
/// Every field is optional even where the schema marks it required: the vendor's own published
/// example omits `processedDate` on an entry that never completed processing, and one
/// unexpectedly-absent field on one unrelated entry must not make the whole report — and
/// therefore every payment's sync — undeserialisable.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IlixiumHistoryOperation {
    #[serde(rename = "entryDate")]
    pub entry_date: Option<String>,
    #[serde(rename = "processedDate")]
    pub processed_date: Option<String>,
    /// Same Operation Type enum as `paymentResponse.type` (this one Ilixium really does share).
    #[serde(rename = "type")]
    pub operation_type: Option<IlixiumOperationType>,
    #[serde(rename = "customerId")]
    pub customer_id: Option<String>,
    pub transaction: Option<IlixiumHistoryTransaction>,
    pub status: Option<IlixiumHistoryStatus>,
}

impl IlixiumHistoryOperation {
    /// `entryDate` as an instant. The API returns millisecond-bearing RFC 3339
    /// (`2025-12-09T15:05:01.000Z`), which is *not* the shape the request accepts — see
    /// [`format_history_period`].
    fn entry_instant(&self) -> Option<OffsetDateTime> {
        self.entry_date
            .as_deref()
            .and_then(|raw| OffsetDateTime::parse(raw, &Rfc3339).ok())
    }

    fn merchant_ref(&self) -> Option<&str> {
        self.transaction
            .as_ref()
            .and_then(|transaction| transaction.merchant_ref.as_deref())
    }

    fn gateway_ref(&self) -> Option<String> {
        self.transaction
            .as_ref()
            .and_then(|transaction| transaction.gateway_ref.clone())
    }

    /// `status.operationRef` — the ULID that "will uniquely identify each operation". The only
    /// value that can tell two REFUND entries on one payment apart, and absent from both entries
    /// of the vendor's own published example. See [`IlixiumHistoryResponse::match_refund_operation`].
    fn operation_ref(&self) -> Option<&str> {
        self.status
            .as_ref()
            .and_then(|status| status.operation_ref.as_deref())
            .filter(|value| !value.is_empty())
    }

    /// `transaction.amount` parsed from its minor-unit digit string (`^[\d]{1,12}$`).
    fn minor_amount(&self) -> Option<i64> {
        self.transaction
            .as_ref()
            .and_then(|transaction| transaction.amount.as_deref())
            .and_then(|amount| amount.parse::<i64>().ok())
    }
}

/// Operation types that describe the **payment's own** lifecycle, and are therefore the ones
/// PSync selects between.
///
/// `REFUND` is excluded because it is RSync's subject, not PSync's — the vendor's own published
/// example has an `AUTH_CAP` and a `REFUND` sharing one `merchantRef`, so a filter that ignored
/// `type` would report a payment's status from its refund. `CREDIT` is a `/direct/credit` payout
/// and is not part of any payment. `PAYMENT` is the APM/hosted-UI surface this connector never
/// touches.
///
/// `REVERSAL` **is** included even though it is not produced by Authorize or Capture: it is what
/// this connector's own Void flow records, and the tech spec's sync-mapping table maps
/// `REVERSAL` + `SUCCESS` to `Voided`. Omitting it would make PSync report a successfully voided
/// payment as `Authorized`.
fn is_payment_lifecycle_operation(operation_type: IlixiumOperationType) -> bool {
    matches!(
        operation_type,
        IlixiumOperationType::Auth
            | IlixiumOperationType::AuthCap
            | IlixiumOperationType::Capture
            | IlixiumOperationType::Reversal
            | IlixiumOperationType::ThreeDsSecureComplete
    )
}

/// `historyResponse`.
///
/// A genuinely different envelope from [`IlixiumPaymentResponse`] — different `status` enum,
/// different transaction schema, an `operation[]` array instead of `paymentHistory`, and no
/// `version` or `type` at the top level — so this is a new type rather than an alias.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IlixiumHistoryResponse {
    /// The outcome of the **query**, not of any payment. A `SUCCESS` here says nothing at all
    /// about the operations in `operation[]`.
    pub status: IlixiumHistoryStatus,
    /// Absent on some failures and explicitly `[]` on a validation rejection.
    #[serde(default)]
    pub operation: Vec<IlixiumHistoryOperation>,
}

impl IlixiumHistoryResponse {
    /// The latest payment-lifecycle operation recorded for `merchant_ref`, or `None` when the
    /// payment does not appear in the queried window at all.
    ///
    /// **Latest, never first.** `/docs/direct/history` is explicit that a late outcome is
    /// appended as a *new* entry and that "historical information is never modified", so the most
    /// recent `entryDate` is the current truth. Entries whose `entryDate` is missing or
    /// unparsable sort below every dated entry (`None < Some` for `Option<OffsetDateTime>`), and
    /// ties are broken by document order so that a later duplicate still wins.
    pub fn latest_payment_operation(&self, merchant_ref: &str) -> Option<&IlixiumHistoryOperation> {
        self.operation
            .iter()
            .enumerate()
            .filter(|(_, operation)| {
                operation.merchant_ref() == Some(merchant_ref)
                    && operation
                        .operation_type
                        .is_some_and(is_payment_lifecycle_operation)
            })
            .max_by(|(left_index, left), (right_index, right)| {
                left.entry_instant()
                    .cmp(&right.entry_instant())
                    .then_with(|| left_index.cmp(right_index))
            })
            .map(|(_, operation)| operation)
    }
}

/// Maps one history `operation[]` entry onto a UCS attempt status.
///
/// A **sibling** of [`map_attempt_status`] / [`map_capture_status`] / [`map_void_status`], not an
/// extension of any of them, for three reasons: the `status.code` enum is
/// [`IlixiumHistoryStatusCode`] rather than [`IlixiumStatusCode`]; the operation type is read off
/// the entry rather than off the envelope, so one mapper has to cover authorisation, capture and
/// reversal outcomes at once; and there is no `cardResponse`, so the 3-D Secure branch that
/// [`map_attempt_status`] relies on cannot exist here.
fn map_history_sync_status(
    operation: &IlixiumHistoryOperation,
    status: &IlixiumHistoryStatus,
    requested_auto_capture: bool,
) -> AttemptStatus {
    let operation_type = operation.operation_type;
    match status.code {
        IlixiumHistoryStatusCode::Success => match operation_type {
            Some(IlixiumOperationType::AuthCap) | Some(IlixiumOperationType::Capture) => {
                AttemptStatus::Charged
            }
            Some(IlixiumOperationType::Auth) => AttemptStatus::Authorized,
            Some(IlixiumOperationType::Reversal) => AttemptStatus::Voided,
            _ => {
                if requested_auto_capture {
                    AttemptStatus::Charged
                } else {
                    AttemptStatus::Authorized
                }
            }
        },
        IlixiumHistoryStatusCode::Pending => match operation_type {
            Some(IlixiumOperationType::Capture) => AttemptStatus::CaptureInitiated,
            Some(IlixiumOperationType::Reversal) => AttemptStatus::VoidInitiated,
            _ => AttemptStatus::Pending,
        },
        IlixiumHistoryStatusCode::Cancelled => AttemptStatus::Voided,
        IlixiumHistoryStatusCode::Declined
        | IlixiumHistoryStatusCode::Rejected
        | IlixiumHistoryStatusCode::Error
        | IlixiumHistoryStatusCode::Exception
        | IlixiumHistoryStatusCode::ValidationErrors => match operation_type {
            Some(IlixiumOperationType::Capture) => AttemptStatus::CaptureFailed,
            Some(IlixiumOperationType::Reversal) => AttemptStatus::VoidFailed,
            _ => AttemptStatus::Failure,
        },
        IlixiumHistoryStatusCode::Unknown => AttemptStatus::Pending,
    }
}

/// Plain-language expansion of the validation codes `/history/operations` can return **about the
/// query itself**.
///
/// A sibling of [`capture_failure_hint`] / [`reversal_failure_hint`]: this endpoint's failure
/// vocabulary is entirely disjoint from theirs, because nothing here is a payment outcome.
fn history_query_failure_hint(code: &str) -> Option<&'static str> {
    match code {
        "VA72" | "VB72" | "VC72" => Some(
            "periodStartDate was missing or malformed — it must match yyyy-MM-ddTHH:mm:ssZ \
             exactly, with no fractional seconds and a literal Z",
        ),
        "VA73" | "VB73" | "VC73" => Some(
            "periodEndDate was missing or malformed — it must match yyyy-MM-ddTHH:mm:ssZ \
             exactly, and the period it closes may span at most 24 hours",
        ),
        "VA74" | "VB74" | "VC74" => Some("reportFormat was missing or was not one of JSON / XML"),
        _ => None,
    }
}

/// The error surfaced when the *query* failed — `VALIDATION_ERRORS`, `EXCEPTION`, or any other
/// non-`SUCCESS` envelope code.
///
/// `attempt_status` deliberately echoes the payment's **existing** status: a report this
/// connector could not read says nothing whatsoever about the payment, and inventing a status
/// from a failed query is exactly the mistake this flow has to avoid.
fn build_history_query_error(
    response: &IlixiumHistoryResponse,
    current_status: AttemptStatus,
    http_code: u16,
) -> ErrorResponse {
    let reason_codes = response.status.reason_codes();
    let hints: Vec<String> = reason_codes
        .iter()
        .filter_map(|code| history_query_failure_hint(code).map(|hint| format!("{code}: {hint}")))
        .collect();

    ErrorResponse {
        status_code: http_code,
        code: reason_codes
            .first()
            .cloned()
            .unwrap_or_else(|| format!("{:?}", response.status.code).to_uppercase()),
        message: response
            .status
            .message
            .clone()
            .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
        reason: Some(if hints.is_empty() {
            format!(
                "POST /history/operations answered {:?}; the payment's status could not be read \
                 and is left unchanged.",
                response.status.code
            )
        } else {
            hints.join("; ")
        }),
        attempt_status: Some(domain_types::router_data::FlowStatus::Payment(
            current_status,
        )),
        connector_transaction_id: None,
        network_decline_code: None,
        network_advice_code: None,
        network_error_message: None,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
}

/// The error surfaced when the query succeeded but the payment is **not in the report**.
///
/// This is emphatically *not* a failed payment. Ilixium's report is windowed and capped at 24
/// hours, so an absent `merchantRef` means one of: the payment is older than the queried window,
/// it is newer than `periodEndDate`, or the platform has not recorded an operation for it yet.
/// None of those is a payment outcome, so `attempt_status` echoes the status the sync started
/// with — `PaymentFlowData::status`, which the framework seeds to `Pending` for a sync — leaving
/// the caller's view of the payment exactly as it was, and the message spells out the one thing
/// the caller can actually do about it.
fn build_history_not_found_error(
    merchant_ref: &str,
    current_status: AttemptStatus,
    http_code: u16,
) -> ErrorResponse {
    ErrorResponse {
        status_code: http_code,
        code: "ILIXIUM_NOT_IN_HISTORY_WINDOW".to_string(),
        message: format!(
            "Ilixium's operation history contains no payment operation for \
             transaction.merchantRef {merchant_ref} in the queried period. The payment's status \
             is unchanged."
        ),
        reason: Some(
            "POST /history/operations is the only query the Ilixium Direct API offers and it \
             reports a window of at most 24 hours, which this connector defaults to [now - 24h, \
             now]. A payment older than that cannot be synced against the default window: \
             re-issue the sync with connector_feature_data.ilixium_history_period_start set to \
             the payment's creation time (RFC 3339, e.g. \"2025-12-09T11:56:11Z\") and the \
             window will be anchored there instead."
                .to_string(),
        ),
        attempt_status: Some(domain_types::router_data::FlowStatus::Payment(
            current_status,
        )),
        connector_transaction_id: None,
        network_decline_code: None,
        network_advice_code: None,
        network_error_message: None,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
}

/// The error surfaced when the matched operation itself failed.
///
/// Unlike the two above, this one *is* a payment outcome, so `attempt_status` carries the mapped
/// failure status. History entries carry no ISO-8583 code and no `cardResponse`, so the reason
/// codes in `status.reasons.reason` are the only machine-readable detail there is.
fn build_history_operation_error(
    status: &IlixiumHistoryStatus,
    attempt_status: AttemptStatus,
    gateway_ref: Option<String>,
    http_code: u16,
) -> ErrorResponse {
    let reason_codes = status.reason_codes();
    ErrorResponse {
        status_code: http_code,
        code: reason_codes
            .first()
            .cloned()
            .unwrap_or_else(|| format!("{:?}", status.code).to_uppercase()),
        message: status
            .message
            .clone()
            .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
        reason: if reason_codes.is_empty() {
            status.message.clone()
        } else {
            Some(reason_codes.join(", "))
        },
        attempt_status: Some(domain_types::router_data::FlowStatus::Payment(
            attempt_status,
        )),
        connector_transaction_id: gateway_ref,
        network_decline_code: None,
        network_advice_code: None,
        network_error_message: None,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
}

impl TryFrom<crate::types::ResponseRouterData<IlixiumHistoryResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: crate::types::ResponseRouterData<IlixiumHistoryResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let current_status = item.router_data.resource_common_data.status;

        let merchant_ref = derive_merchant_ref(
            &item
                .router_data
                .resource_common_data
                .connector_request_reference_id,
        )
        .change_context(errors::ConnectorError::ResponseHandlingFailed {
            context: errors::ResponseTransformationErrorContext {
                http_status_code: Some(item.http_code),
                additional_context: Some(
                    "Could not derive the Ilixium transaction.merchantRef to match this payment \
                     against POST /history/operations."
                        .to_string(),
                ),
            },
        })?;

        if response.status.code != IlixiumHistoryStatusCode::Success {
            return Ok(Self {
                response: Err(build_history_query_error(
                    &response,
                    current_status,
                    item.http_code,
                )),
                ..item.router_data
            });
        }

        let Some(operation) = response.latest_payment_operation(&merchant_ref) else {
            return Ok(Self {
                response: Err(build_history_not_found_error(
                    &merchant_ref,
                    current_status,
                    item.http_code,
                )),
                ..item.router_data
            });
        };

        let Some(operation_status) = operation.status.as_ref() else {
            return Ok(Self {
                response: Err(build_history_not_found_error(
                    &merchant_ref,
                    current_status,
                    item.http_code,
                )),
                ..item.router_data
            });
        };

        let status = map_history_sync_status(
            operation,
            operation_status,
            item.router_data.request.is_auto_capture(),
        );
        let gateway_ref = operation.gateway_ref();
        let operation_merchant_ref = operation.merchant_ref().map(ToOwned::to_owned);

        let raw_connector_status = RawConnectorStatus {
            code: Some(format!("{:?}", operation_status.code).to_uppercase()),
            message: operation_status.message.clone(),
            reason: {
                let codes = operation_status.reason_codes();
                (!codes.is_empty()).then(|| codes.join(", "))
            },
        };

        let payments_response = if matches!(
            status,
            AttemptStatus::Failure | AttemptStatus::CaptureFailed | AttemptStatus::VoidFailed
        ) {
            Err(build_history_operation_error(
                operation_status,
                status,
                gateway_ref.clone(),
                item.http_code,
            ))
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: gateway_ref.clone().map_or_else(
                    || item.router_data.request.connector_transaction_id.clone(),
                    ResponseId::ConnectorTransactionId,
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: operation_merchant_ref,
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            })
        };

        Ok(Self {
            response: payments_response,
            resource_common_data: PaymentFlowData {
                status,
                raw_connector_status: Some(raw_connector_status),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

/// The RSync request body — **the same message PSync sends**, aliased under its own name.
///
/// The alias is not cosmetic. `create_all_prerequisites!` mints one `<Ident>Templating` marker
/// struct per named request/response ident, so naming [`IlixiumHistoryRequest`] on a second flow
/// would try to define `IlixiumHistoryRequestTemplating` twice. A distinct ident gives RSync its
/// own marker while keeping exactly one request type, one serialisation and one digest.
pub type IlixiumRefundHistoryRequest = IlixiumHistoryRequest;

/// The RSync response envelope — **the same message PSync parses**, aliased for the same
/// `<Ident>Templating` reason as [`IlixiumRefundHistoryRequest`].
pub type IlixiumRefundHistoryResponse = IlixiumHistoryResponse;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        IlixiumRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for IlixiumRefundHistoryRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: IlixiumRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;
        let auth = IlixiumAuthType::try_from(&router_data.connector_config)?;

        resolve_original_merchant_ref(
            request.connector_order_id.as_deref(),
            &request.connector_transaction_id,
        )?;

        let (period_start, period_end) =
            resolve_history_window(request.connector_feature_data.as_ref())?;

        Ok(Self {
            period_start_date: format_history_period(period_start)?,
            period_end_date: format_history_period(period_end)?,
            merchant: IlixiumMerchant::from(&auth),
            report_format: IlixiumReportFormat::Json,
        })
    }
}

/// Length of a ULID in its canonical Crockford-Base32 text form.
const OPERATION_REF_LEN: usize = 26;

/// Whether a value looks like an Ilixium `operationRef`.
///
/// `/docs/direct/reconciliation` describes `operationRef` as a ULID, which is 26 characters of
/// Crockford Base32. The test is deliberately a *shape* test rather than a strict alphabet check:
/// its only job is to separate a real `operationRef` from the fallback id
/// [`refund_identifier`] hands out when Ilixium omits one (a UCS `merchant_refund_id`, which is a
/// dashed/prefixed value of a quite different length). Being slightly permissive costs nothing —
/// a value that passes but matches no entry simply falls through to the heuristic below.
fn is_operation_ref_shaped(value: &str) -> bool {
    value.len() == OPERATION_REF_LEN && value.chars().all(|c| c.is_ascii_alphanumeric())
}

/// How a `type: REFUND` history entry was selected for the refund being synced.
///
/// The distinction is carried out of the matcher rather than collapsed inside it because the two
/// arms have genuinely different authority, and the caller logs accordingly.
enum RefundOperationMatch<'a> {
    /// Selected by an exact `status.operationRef` match. Authoritative: `operationRef` is
    /// documented to uniquely identify an operation.
    Exact(&'a IlixiumHistoryOperation),
    /// Selected by the documented heuristic because no `operationRef` was available to match on.
    /// `candidates` is how many `type: REFUND` entries remained after narrowing — anything above
    /// 1 means the choice was genuinely ambiguous.
    Heuristic {
        operation: &'a IlixiumHistoryOperation,
        candidates: usize,
    },
}

impl RefundOperationMatch<'_> {
    fn operation(&self) -> &IlixiumHistoryOperation {
        match self {
            Self::Exact(operation) => operation,
            Self::Heuristic { operation, .. } => operation,
        }
    }
}

impl IlixiumHistoryResponse {
    /// Selects the `type: REFUND` entry that corresponds to the refund being synced.
    ///
    /// **The candidate set** is every entry whose `transaction.merchantRef` is the *original
    /// payment's* and whose `type` is exactly `REFUND`. `CREDIT` is excluded by construction: it
    /// is a `/direct/credit` payout, not a refund of a payment, and treating one as a refund would
    /// report money-out that no refund record ever asked for. Every other operation type belongs
    /// to the payment's own lifecycle and is PSync's subject (see
    /// [`Self::latest_payment_operation`]).
    ///
    /// **Selecting one refund out of the candidates is the hard part, and it cannot be done
    /// reliably today.** A variant-A refund is bound to its payment by the payment's own
    /// `merchantRef`, and its history entry echoes the payment's `merchantRef` *and* `gatewayRef`
    /// — observable in the vendor's own published example, where an `AUTH_CAP` and a `REFUND`
    /// share both values, with **no `operationRef` on either**. So N partial refunds against one
    /// payment produce N entries differing only by `entryDate`, `processedDate` and `amount`.
    ///
    /// The ladder is therefore:
    ///
    /// 1. **Exact `operationRef` match** — used when `RefundSyncData::connector_refund_id` holds
    ///    an [`is_operation_ref_shaped`] value (i.e. `/direct/refund` returned a real
    ///    `operationRef` and [`refund_identifier`] passed it through). This is the only
    ///    authoritative match, and the only one that survives multiple partial refunds.
    /// 2. If that id is `operationRef`-shaped and the report *does* carry `operationRef`s on the
    ///    candidates but none of them is ours, the refund is **not in this window** — `None` — and
    ///    is emphatically *not* silently swapped for some other refund of the same payment.
    /// 3. **Heuristic**, used only when no `operationRef` is available on either side: narrow to
    ///    the candidates whose `transaction.amount` equals the amount being synced (skipped when
    ///    that would leave nothing, so a report that omits `amount` still matches), then take the
    ///    **latest by `entryDate`** — the same "late outcomes are appended, never edited" rule
    ///    PSync relies on — with ties broken by document order.
    ///
    /// **Known limitation (tech spec UNDECIDED §12).** Step 3 cannot distinguish two refunds of
    /// the same amount on the same payment. Syncing refund #1 of three identical partial refunds
    /// returns the *latest* one's status. The caller is told: the match arm is reported back so
    /// the response mapping can log the ambiguity, and the number of candidates is carried with
    /// it. This resolves itself the moment Ilixium's `operationRef` goes live, at which point
    /// step 1 applies and steps 2–3 become unreachable.
    fn match_refund_operation(
        &self,
        merchant_ref: &str,
        connector_refund_id: &str,
        minor_refund_amount: Option<i64>,
    ) -> Option<RefundOperationMatch<'_>> {
        let candidates: Vec<(usize, &IlixiumHistoryOperation)> = self
            .operation
            .iter()
            .enumerate()
            .filter(|(_, operation)| {
                operation.merchant_ref() == Some(merchant_ref)
                    && operation.operation_type == Some(IlixiumOperationType::Refund)
            })
            .collect();

        if candidates.is_empty() {
            return None;
        }

        if is_operation_ref_shaped(connector_refund_id) {
            if let Some((_, operation)) = candidates
                .iter()
                .find(|(_, operation)| operation.operation_ref() == Some(connector_refund_id))
            {
                return Some(RefundOperationMatch::Exact(operation));
            }

            if candidates
                .iter()
                .any(|(_, operation)| operation.operation_ref().is_some())
            {
                return None;
            }
        }

        let narrowed: Vec<(usize, &IlixiumHistoryOperation)> = match minor_refund_amount {
            Some(amount) => {
                let matching: Vec<(usize, &IlixiumHistoryOperation)> = candidates
                    .iter()
                    .filter(|(_, operation)| operation.minor_amount() == Some(amount))
                    .copied()
                    .collect();
                if matching.is_empty() {
                    candidates
                } else {
                    matching
                }
            }
            None => candidates,
        };

        let candidate_count = narrowed.len();
        narrowed
            .into_iter()
            .max_by(|(left_index, left), (right_index, right)| {
                left.entry_instant()
                    .cmp(&right.entry_instant())
                    .then_with(|| left_index.cmp(right_index))
            })
            .map(|(_, operation)| RefundOperationMatch::Heuristic {
                operation,
                candidates: candidate_count,
            })
    }
}

/// Maps one `type: REFUND` history entry onto a UCS [`RefundStatus`].
///
/// A **fifth sibling** of [`map_attempt_status`] / [`map_capture_status`] / [`map_void_status`] /
/// [`map_refund_status`], not a reuse of any of them. The nearest neighbour, [`map_refund_status`],
/// is keyed on [`IlixiumStatusCode`]; this one is keyed on [`IlixiumHistoryStatusCode`], a
/// different enum that adds `EXCEPTION` and `VALIDATION_ERRORS` and drops `RESUBMISSION`. Feeding
/// a history entry through `map_refund_status` would not even type-check, and collapsing the two
/// enums to make it would lose the distinction between "the operation failed" and "the query was
/// malformed".
///
/// The nearest neighbour in the *other* direction, [`map_history_sync_status`], shares this enum
/// but targets `AttemptStatus` and branches on the operation type; here the operation type is
/// already known to be `REFUND`, so only the status code matters.
fn map_history_refund_status(status: &IlixiumHistoryStatus) -> RefundStatus {
    match status.code {
        IlixiumHistoryStatusCode::Success => RefundStatus::Success,
        IlixiumHistoryStatusCode::Pending => RefundStatus::Pending,
        IlixiumHistoryStatusCode::Cancelled
        | IlixiumHistoryStatusCode::Declined
        | IlixiumHistoryStatusCode::Rejected
        | IlixiumHistoryStatusCode::Error
        | IlixiumHistoryStatusCode::Exception
        | IlixiumHistoryStatusCode::ValidationErrors => RefundStatus::Failure,
        IlixiumHistoryStatusCode::Unknown => RefundStatus::Pending,
    }
}

/// The error surfaced when the *query* failed — the refund-side counterpart of
/// [`build_history_query_error`].
///
/// Written out rather than layered on it because that builder stamps
/// `attempt_status: FlowStatus::Payment(..)`, and a refund sync must report
/// `FlowStatus::Refund(..)`. As there, the status echoed back is the refund's **existing** one: a
/// report this connector could not read says nothing whatsoever about the refund.
fn build_refund_history_query_error(
    response: &IlixiumRefundHistoryResponse,
    current_status: RefundStatus,
    http_code: u16,
) -> ErrorResponse {
    let reason_codes = response.status.reason_codes();
    let hints: Vec<String> = reason_codes
        .iter()
        .filter_map(|code| history_query_failure_hint(code).map(|hint| format!("{code}: {hint}")))
        .collect();

    ErrorResponse {
        status_code: http_code,
        code: reason_codes
            .first()
            .cloned()
            .unwrap_or_else(|| format!("{:?}", response.status.code).to_uppercase()),
        message: response
            .status
            .message
            .clone()
            .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
        reason: Some(if hints.is_empty() {
            format!(
                "POST /history/operations answered {:?}; the refund's status could not be read \
                 and is left unchanged.",
                response.status.code
            )
        } else {
            hints.join("; ")
        }),
        attempt_status: Some(domain_types::router_data::FlowStatus::Refund(
            current_status,
        )),
        connector_transaction_id: None,
        network_decline_code: None,
        network_advice_code: None,
        network_error_message: None,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
}

/// The error surfaced when the query succeeded but no `type: REFUND` entry for this payment is in
/// the report — the refund-side counterpart of [`build_history_not_found_error`].
///
/// This is emphatically *not* a failed refund, so `attempt_status` echoes the status the sync
/// started with. Three things can put a refund here, and the message names all of them: the refund
/// is outside the queried `<= 24h` window, the platform has not recorded an operation for it yet,
/// or `operationRef` is live and none of the payment's refund entries is this one.
fn build_refund_not_found_error(
    merchant_ref: &str,
    current_status: RefundStatus,
    http_code: u16,
) -> ErrorResponse {
    ErrorResponse {
        status_code: http_code,
        code: "ILIXIUM_REFUND_NOT_IN_HISTORY_WINDOW".to_string(),
        message: format!(
            "Ilixium's operation history contains no REFUND operation for \
             transaction.merchantRef {merchant_ref} in the queried period. The refund's status is \
             unchanged."
        ),
        reason: Some(
            "POST /history/operations is the only query the Ilixium Direct API offers — there is \
             no refund-status endpoint and no lookup by refund id — and it reports a window of at \
             most 24 hours, which this connector defaults to [now - 24h, now]. A refund older \
             than that cannot be synced against the default window: re-issue the sync with \
             connector_feature_data.ilixium_history_period_start set to at or just before the \
             refund's creation time (RFC 3339, e.g. \"2025-12-09T11:56:11Z\") and the window will \
             be anchored there instead. Note also that CREDIT operations are payouts, not \
             refunds, and are never matched."
                .to_string(),
        ),
        attempt_status: Some(domain_types::router_data::FlowStatus::Refund(
            current_status,
        )),
        connector_transaction_id: None,
        network_decline_code: None,
        network_advice_code: None,
        network_error_message: None,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
}

/// The error surfaced when the matched REFUND entry itself failed — the refund-side counterpart of
/// [`build_history_operation_error`].
///
/// Unlike the two above, this one *is* an outcome, so `attempt_status` carries the mapped
/// [`RefundStatus`]. History entries carry no ISO-8583 code and no `cardResponse`, so
/// `status.reasons.reason` is the only machine-readable detail there is; it is expanded through
/// [`refund_failure_hint`], the same vocabulary `/direct/refund` uses, because a REFUND history
/// entry records the outcome of exactly that operation.
fn build_refund_history_operation_error(
    status: &IlixiumHistoryStatus,
    refund_status: RefundStatus,
    gateway_ref: Option<String>,
    http_code: u16,
) -> ErrorResponse {
    let reason_codes = status.reason_codes();
    let hints: Vec<String> = reason_codes
        .iter()
        .filter_map(|code| refund_failure_hint(code).map(|hint| format!("{code}: {hint}")))
        .collect();

    ErrorResponse {
        status_code: http_code,
        code: reason_codes
            .first()
            .cloned()
            .unwrap_or_else(|| format!("{:?}", status.code).to_uppercase()),
        message: status
            .message
            .clone()
            .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
        reason: if !hints.is_empty() {
            Some(hints.join("; "))
        } else if reason_codes.is_empty() {
            status.message.clone()
        } else {
            Some(reason_codes.join(", "))
        },
        attempt_status: Some(domain_types::router_data::FlowStatus::Refund(refund_status)),
        connector_transaction_id: gateway_ref,
        network_decline_code: None,
        network_advice_code: None,
        network_error_message: None,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
}

/// Resolves the `connector_refund_id` an RSync reports back.
///
/// Prefers the matched entry's `status.operationRef` — if the platform has started populating it,
/// echoing it upgrades a UCS-side fallback id to the real Ilixium one — and otherwise echoes the
/// id the sync was issued with, which is still correct. Erroring is reachable only if a sync was
/// somehow issued with no refund id at all.
fn resolve_sync_refund_identifier(
    operation: &IlixiumHistoryOperation,
    connector_refund_id: &str,
    http_code: u16,
) -> Result<String, error_stack::Report<errors::ConnectorError>> {
    if let Some(operation_ref) = operation.operation_ref() {
        return Ok(operation_ref.to_owned());
    }

    if !connector_refund_id.is_empty() {
        return Ok(connector_refund_id.to_owned());
    }

    Err(error_stack::report!(
        errors::ConnectorError::ResponseHandlingFailed {
            context: errors::ResponseTransformationErrorContext {
                http_status_code: Some(http_code),
                additional_context: Some(
                    "The matched Ilixium REFUND history entry carries no status.operationRef and \
                     the sync was issued with no connector_refund_id, so this refund has no \
                     identifier: the entry's merchantRef and gatewayRef both echo the original \
                     payment and would collide across partial refunds."
                        .to_string(),
                ),
            },
        }
    ))
}

impl TryFrom<crate::types::ResponseRouterData<IlixiumRefundHistoryResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: crate::types::ResponseRouterData<IlixiumRefundHistoryResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let request = &item.router_data.request;
        let current_status = item.router_data.resource_common_data.status;

        let merchant_ref = resolve_original_merchant_ref(
            request.connector_order_id.as_deref(),
            &request.connector_transaction_id,
        )
        .change_context(errors::ConnectorError::ResponseHandlingFailed {
            context: errors::ResponseTransformationErrorContext {
                http_status_code: Some(item.http_code),
                additional_context: Some(
                    "Could not resolve the original payment's Ilixium transaction.merchantRef to \
                     match this refund against POST /history/operations."
                        .to_string(),
                ),
            },
        })?;

        if response.status.code != IlixiumHistoryStatusCode::Success {
            return Ok(Self {
                response: Err(build_refund_history_query_error(
                    &response,
                    current_status,
                    item.http_code,
                )),
                ..item.router_data
            });
        }

        let refund_amount = request
            .refund_money
            .as_ref()
            .map(|money| money.amount.get_amount_as_i64());

        let Some(matched) = response.match_refund_operation(
            &merchant_ref,
            &request.connector_refund_id,
            refund_amount,
        ) else {
            return Ok(Self {
                response: Err(build_refund_not_found_error(
                    &merchant_ref,
                    current_status,
                    item.http_code,
                )),
                ..item.router_data
            });
        };

        if let RefundOperationMatch::Heuristic { candidates, .. } = &matched {
            if *candidates > 1 {
                tracing::warn!(
                    connector = "ilixium",
                    merchant_ref = %merchant_ref,
                    candidates = *candidates,
                    "Ilixium's operation history holds several REFUND entries for one payment and \
                     none carries a status.operationRef, so this refund was matched by the \
                     documented heuristic (amount, then latest entryDate) rather than \
                     identified. Partial refunds of equal amounts on one payment are not \
                     individually distinguishable at Ilixium until operationRef goes live."
                );
            }
        }

        let operation = matched.operation();

        let Some(operation_status) = operation.status.as_ref() else {
            return Ok(Self {
                response: Err(build_refund_not_found_error(
                    &merchant_ref,
                    current_status,
                    item.http_code,
                )),
                ..item.router_data
            });
        };

        let status = map_history_refund_status(operation_status);

        let raw_connector_status = RawConnectorStatus {
            code: Some(format!("{:?}", operation_status.code).to_uppercase()),
            message: operation_status.message.clone(),
            reason: {
                let codes = operation_status.reason_codes();
                (!codes.is_empty()).then(|| codes.join(", "))
            },
        };

        let refunds_response = if status == RefundStatus::Failure {
            Err(build_refund_history_operation_error(
                operation_status,
                status,
                operation.gateway_ref(),
                item.http_code,
            ))
        } else {
            Ok(RefundsResponseData {
                connector_refund_id: resolve_sync_refund_identifier(
                    operation,
                    &request.connector_refund_id,
                    item.http_code,
                )?,
                refund_status: status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            })
        };

        Ok(Self {
            response: refunds_response,
            resource_common_data: RefundFlowData {
                status,
                raw_connector_status: Some(raw_connector_status),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
#[allow(clippy::panic)]
mod date_of_birth_tests {
    use hyperswitch_masking::{ExposeInterface, Secret};
    use time::macros::date;

    use super::{format_date_of_birth, resolve_date_of_birth};

    #[test]
    fn formats_as_ddmmyyyy_with_zero_padding() {
        // Single-digit day and month are where the padding matters: Ilixium reads the string
        // positionally, so "341970" would be a VC8 rather than a short date.
        assert_eq!(
            format_date_of_birth(&Secret::new(date!(1970 - 04 - 03))).expose(),
            "03041970"
        );
        assert_eq!(
            format_date_of_birth(&Secret::new(date!(2000 - 12 - 25))).expose(),
            "25122000"
        );
    }

    #[test]
    fn structured_field_wins_over_deprecated_metadata() {
        let metadata = Secret::new(serde_json::json!({ "ilixium_date_of_birth": "01011999" }));
        let resolved =
            resolve_date_of_birth(Some(&Secret::new(date!(1970 - 04 - 03))), Some(&metadata))
                .expect("a date of birth was supplied");
        assert_eq!(resolved.expose(), "03041970");
    }

    #[test]
    fn falls_back_to_metadata_under_either_key() {
        for key in ["ilixium_date_of_birth", "date_of_birth"] {
            let metadata = Secret::new(serde_json::json!({ key: "01011999" }));
            let resolved = resolve_date_of_birth(None, Some(&metadata))
                .unwrap_or_else(|| panic!("expected the {key} fallback to resolve"));
            assert_eq!(resolved.expose(), "01011999");
        }
    }

    #[test]
    fn omitted_when_neither_source_has_one() {
        assert!(resolve_date_of_birth(None, None).is_none());
        let metadata = Secret::new(serde_json::json!({ "unrelated": "value" }));
        assert!(resolve_date_of_birth(None, Some(&metadata)).is_none());
    }
}
