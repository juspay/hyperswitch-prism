//! Ilixium Direct API transformers.
//!
//! Scope: **Card, one-time (non-recurring) payments, Authorize flow only** — both the
//! no-3DS and the 3DS variants. The 3DS variant is a two-leg flow that UCS drives through
//! the *same* `Authorize` flow:
//!
//! 1. `POST /direct/auth` — returns `status.code = PENDING` plus the ACS redirect data.
//!    The connector answers with a `RedirectForm::Form` that auto-POSTs `MD` / `PaReq` /
//!    `TermUrl` to `cardResponse.threeDSecureAcsUrl`.
//! 2. The ACS posts `md` / `paRes` back to `TermUrl`; UCS re-invokes `Authorize` with
//!    `request.redirect_response` populated, and the connector then calls
//!    `POST /direct/threedcomplete` to finalise the payment.
//!
//! Wallets, APMs, bank transfers, mandates/MIT/recurring, tokenization, and
//! Capture/Refund/Void/PSync/RSync are deliberately out of scope.
//!
//! Reference: `grace/rulesbook/codegen/references/ilixium/technical_specification.md`

use base64::Engine;
use common_enums::{AttemptStatus, AuthenticationType, Currency};
use common_utils::{
    crypto::{self, GenerateDigest},
    pii::Email,
    types::StringMinorUnit,
};
use domain_types::{
    connector_flow::Authorize,
    connector_types::{PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, ResponseId},
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

use super::IlixiumRouterData;

pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

/// Message-level API version. The platform's `version` field is `2` for every
/// in-scope Direct API message (`/direct/auth`, `/direct/threedcomplete`).
const ILIXIUM_MESSAGE_VERSION: u8 = 2;

/// `transaction.merchantRef` is constrained to `^(?!.*£)[\w]{4,20}$`.
const MERCHANT_REF_MIN_LEN: usize = 4;
const MERCHANT_REF_MAX_LEN: usize = 20;

/// `emvco3ds.browserDetails.challengeWindowSize`. `05` = full screen.
///
/// UCS renders the ACS challenge as a full-page browser redirect (the connector emits a
/// `RedirectForm::Form` that the browser navigates to), not inside a sized iframe, so the
/// full-screen preset is the only one that matches how the challenge is actually displayed.
/// The vendor's own example uses `01` (250x400), which would under-size a full-page challenge.
const CHALLENGE_WINDOW_SIZE_FULL_SCREEN: &str = "05";

/// The colour depths Ilixium accepts (`^1$|^4$|^8$|^15$|^16$|^24$|^32$|^48$`).
const ACCEPTED_COLOR_DEPTHS: [u8; 8] = [1, 4, 8, 15, 16, 24, 32, 48];

// =============================================================================
// AUTH
// =============================================================================

/// Ilixium needs three secrets, so this maps onto the repo's `SignatureKey`-shaped
/// auth type:
///
/// | field       | Ilixium name                | used as                            |
/// |-------------|-----------------------------|------------------------------------|
/// | `api_key`   | Digest Calculation Password | secret input to `X-MERCHANT-DIGEST`, **never transmitted** |
/// | `key1`      | MerchantId                  | request body `merchant.merchantId` |
/// | `api_secret`| AccountId                   | request body `merchant.accountId`  |
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
    /// Ilixium's `X-MERCHANT-DIGEST` (https://docs.ilixium.com/docs/direct/digest).
    ///
    /// This is **not** an HMAC. It is two rounds of SHA-512 + standard (non-chunked) Base64,
    /// where the password participates only in round two and is concatenated to the
    /// **Base64 text** of round one:
    ///
    /// ```text
    /// d1 = Base64(SHA-512(body_bytes))
    /// d2 = Base64(SHA-512(UTF8(d1 || password)))
    /// X-MERCHANT-DIGEST: d2
    /// ```
    ///
    /// `body` must be the exact byte string that will be transmitted — the caller passes the
    /// serialised request body straight from `get_request_body`, which is the same value the
    /// framework puts on the wire (`RequestContent::get_body_bytes` re-uses
    /// `get_inner_value`).
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

// =============================================================================
// merchantRef derivation
// =============================================================================

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
    let is_verbatim_safe = (MERCHANT_REF_MIN_LEN..=MERCHANT_REF_MAX_LEN).contains(&reference.len())
        && reference
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_');
    if is_verbatim_safe {
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

// =============================================================================
// SHARED REQUEST TYPES
// =============================================================================

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
    pub country: common_enums::CountryAlpha2,
}

#[derive(Debug, Clone, Serialize)]
pub struct IlixiumCustomer {
    #[serde(rename = "customerId")]
    pub customer_id: String,
    pub email: Email,
    #[serde(rename = "firstName")]
    pub first_name: Secret<String>,
    pub surname: Secret<String>,
    /// `ddmmyyyy`. Schema-mandatory but absent from the UCS payment model — see
    /// [`extract_date_of_birth`] for how it is sourced and why it may be omitted.
    #[serde(rename = "dateOfBirth", skip_serializing_if = "Option::is_none")]
    pub date_of_birth: Option<Secret<String>>,
    pub address: IlixiumAddress,
    #[serde(rename = "mobileNumber", skip_serializing_if = "Option::is_none")]
    pub mobile_number: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IlixiumPaymentInfo {
    /// ISO 3166 country of origin of the transaction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<common_enums::CountryAlpha2>,
    /// Cardholder browser IP — recommended by Ilixium whenever 3-D Secure is in play.
    #[serde(rename = "ipAddress", skip_serializing_if = "Option::is_none")]
    pub ip_address: Option<Secret<String>>,
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

// =============================================================================
// AUTHORIZE REQUEST (both legs)
// =============================================================================

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

// =============================================================================
// REQUEST BUILDERS
// =============================================================================

/// `customer.dateOfBirth` is schema-mandatory (`ddmmyyyy`) but has no home in the UCS payment
/// model (tech spec UNDECIDED #1). Rather than fabricate a placeholder date — which would be
/// sent to the issuer and could distort Ilixium's own fraud checks — it is read from the
/// merchant-supplied `metadata` object under `ilixium_date_of_birth` (or `date_of_birth`) and
/// simply omitted when absent. Accounts that enforce the field will answer `VA8`, which
/// surfaces as a normal `REJECTED` error rather than a silently wrong value.
fn extract_date_of_birth(
    metadata: Option<&common_utils::pii::SecretSerdeValue>,
) -> Option<Secret<String>> {
    let value = metadata?.clone().expose();
    ["ilixium_date_of_birth", "date_of_birth"]
        .iter()
        .find_map(|key| value.get(key).and_then(|v| v.as_str()).map(String::from))
        .map(Secret::new)
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

        let card = match &request.payment_method_data {
            PaymentMethodData::Card(card_data) => IlixiumCard {
                card_number: card_data.card_number.clone(),
                // Ilixium wants MMyyyy with no separator, e.g. `012030`.
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
                        "Ilixium supports only raw card payments for Authorize; wallets, APMs, \
                         bank transfers and stored-card tokens are out of scope for this \
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
                                 Authorize: {other:?}"
                            )),
                        },
                    )
                ));
            }
        };

        let auth = IlixiumAuthType::try_from(&router_data.connector_config)?;

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

        let country = common.get_optional_billing_country().ok_or_else(|| {
            error_stack::report!(errors::IntegrationError::MissingRequiredField {
                field_name: "payment_method_data.billing.address.country",
                context: errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Ilixium always requires customer.address.country, even on accounts \
                         configured for Optional Address."
                            .to_string(),
                    ),
                    doc_url: Some(
                        "https://docs.ilixium.com/docs/direct/optional-address".to_string()
                    ),
                    additional_context: None,
                },
            })
        })?;

        let email = request
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

        let (first_name, surname) = split_customer_name(common, request.customer_name.as_deref())?;

        // `customer.customerId` is mandatory (pattern allows `-`, up to 255 chars), so the UCS
        // customer id maps across verbatim when present. Ilixium ties a card to a single
        // customer id (fraud code 121, "Duplicate Card"), which is why the real customer id is
        // preferred over a per-payment synthetic; the payment reference is only a fallback for
        // guest checkouts that carry no customer at all (tech spec UNDECIDED #2, options a+b).
        let customer_id = common
            .customer_id
            .as_ref()
            .map(|id| id.get_string_repr().to_string())
            .unwrap_or_else(|| common.connector_request_reference_id.clone());

        let ip_address = request
            .browser_info
            .as_ref()
            .and_then(|info| info.ip_address)
            .map(|ip| Secret::new(ip.to_string()));

        // `emvco3ds` is only meaningful on a 3-D Secure authorisation. Ilixium ignores it on
        // non-3DS accounts, but sending it there would be noise, so it is gated strictly on the
        // request's authentication type.
        let is_three_ds = common.auth_type == AuthenticationType::ThreeDs;
        let emvco3ds = if is_three_ds {
            let browser_info = request.browser_info.as_ref().ok_or_else(|| {
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
            })?;
            Some(IlixiumEmvco3ds {
                browser_details: build_browser_details(browser_info)?,
            })
        } else {
            None
        };

        // `paymentInfo.country` is the country of origin of the transaction and
        // `paymentInfo.ipAddress` is the cardholder's browser IP, which Ilixium recommends
        // whenever 3-D Secure is in play. Both are optional, so the block is emitted whenever
        // either value is known.
        let payment_info = Some(IlixiumPaymentInfo {
            country: Some(country),
            ip_address,
        });

        Ok(Self {
            version: ILIXIUM_MESSAGE_VERSION,
            deferred_capture: !request.is_auto_capture(),
            transaction: IlixiumTransaction {
                transaction_type: IlixiumTransactionType::Ecommerce,
                merchant_ref: derive_merchant_ref(&common.connector_request_reference_id)?,
                amount,
                currency: request.currency,
            },
            payment_method_type: IlixiumPaymentMethodType::Card,
            merchant: IlixiumMerchant::from(&auth),
            card,
            customer: IlixiumCustomer {
                customer_id,
                email,
                first_name,
                surname,
                date_of_birth: extract_date_of_birth(request.metadata.as_ref()),
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

        // The ACS form field names are case-inconsistent across issuers: the spec documents
        // lowercase `md`/`paRes` on the TermUrl post-back, while the outbound form fields are
        // `MD`/`PaReq`. Accept the documented casing plus the common variants.
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
            // Recomputed from the same reference the original /direct/auth used, so the two
            // legs always quote an identical merchantRef without needing a persisted mapping.
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

// =============================================================================
// RESPONSE TYPES
// =============================================================================

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
/// `deferredCapture` was false and `AUTH` when it was true.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IlixiumOperationType {
    Auth,
    AuthCap,
    Capture,
    Reversal,
    Refund,
    Credit,
    ThreedSecureComplete,
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
/// unparseable response.
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

/// The shared `paymentResponse` envelope. `/direct/auth` and `/direct/threedcomplete` return
/// byte-for-byte the same schema, so both Authorize legs deserialise into this one type.
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
        // RESUBMISSION appears in the OpenAPI enum but is undocumented; treat it, and any code
        // this integration does not recognise, as still-in-flight rather than guessing a
        // terminal outcome.
        IlixiumStatusCode::Resubmission | IlixiumStatusCode::Unknown => AttemptStatus::Pending,
    }
}

/// Builds the ACS auto-POST form. Ilixium never receives the return URL — the merchant owns
/// the `TermUrl` — so the connector assembles the form itself (tech spec UNDECIDED #5,
/// option (a)). The 1.x MPI field names `MD`/`PaReq` are retained by Ilixium even for 3DS2.
fn build_three_ds_redirect_form<T: PaymentMethodDataTypes>(
    response: &IlixiumPaymentResponse,
    acs_url: &str,
    request: &PaymentsAuthorizeData<T>,
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

    // The ACS posts `md`/`paRes` back to TermUrl, which must be the UCS endpoint that
    // re-invokes Authorize; `complete_authorize_url` is exactly that. `router_return_url` is
    // the merchant-facing landing page and is only a fallback.
    let term_url = request
        .complete_authorize_url
        .clone()
        .or_else(|| request.router_return_url.clone())
        .ok_or_else(|| {
            error_stack::report!(errors::ConnectorError::ResponseHandlingFailed {
                context: errors::ResponseTransformationErrorContext {
                    http_status_code: Some(http_code),
                    additional_context: Some(
                        "Ilixium 3DS requires a TermUrl for the ACS form, but neither \
                         complete_authorize_url nor router_return_url was supplied"
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
        // Ilixium's machine-readable failure detail lives in status.reasons.reason; the
        // status.code itself is the coarse bucket and is the only thing available when the
        // platform sends no reasons at all.
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
            Some(acs_url) => Some(Box::new(build_three_ds_redirect_form(
                &response,
                acs_url,
                &item.router_data.request,
                item.http_code,
            )?)),
            None => None,
        };

        let payments_response = if status == AttemptStatus::Failure {
            Err(build_error_response(&response, status, item.http_code))
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                // `transaction.gatewayRef` is Ilixium's own unique reference and the only id
                // that identifies the transaction for follow-up operations. Validation
                // rejections never reach here (they map to Failure above), but a PENDING
                // response before the gateway has minted one still needs a sensible fallback.
                resource_id: response
                    .gateway_ref()
                    .map(ResponseId::ConnectorTransactionId)
                    .unwrap_or(ResponseId::NoResponseId),
                redirection_data,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                // The echoed `transaction.merchantRef`. When the UCS reference had to be
                // hashed down to fit Ilixium's 20-character `[\w]` limit, this is the only
                // place the caller can learn the reference Ilixium's back office actually
                // knows the payment by, so surfacing it here is what makes reconciliation
                // possible.
                connector_response_reference_id: response.merchant_ref(),
                incremental_authorization_allowed: None,
                splits: None,
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
