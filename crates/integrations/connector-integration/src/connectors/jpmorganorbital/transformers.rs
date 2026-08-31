use common_enums::{AttemptStatus, CaptureMethod, CardNetwork, Currency};
use common_utils::types::MinorUnit;
use domain_types::{
    connector_flow::{Authorize, PSync},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData, ResponseId,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::{Card, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_request_types::AuthenticationData,
    utils::{get_card_issuer, CardIssuer},
};
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::connectors::jpmorganorbital::{
    JpmorganOrbitalAmountConvertor, JpmorganOrbitalRouterData,
};
use crate::types::ResponseRouterData;

/// Feature release version. Required to receive v4 / Feature Version 5.2 response
/// elements; hard-coded because the connector only speaks that dialect.
pub const ORBITAL_VERSION: &str = "5.2";
/// `order.industryType`. Only e-commerce is in scope.
const INDUSTRY_TYPE_ECOMMERCE: &str = "EC";
/// `transType` for an authorization without capture (UCS `CaptureMethod::Manual`).
const TRANS_TYPE_AUTH_ONLY: &str = "A";
/// `transType` for authorization + capture (UCS `CaptureMethod::Automatic`).
const TRANS_TYPE_AUTH_CAPTURE: &str = "AC";
/// `cardholderVerification.ccCardVerifyPresenceInd` — "value is present".
const CARD_VERIFY_PRESENCE_PRESENT: &str = "1";
/// `order.status.procStatus` value meaning "passed all Gateway edit checks".
const PROC_STATUS_SUCCESS: &str = "0";
/// `order.status.approvalStatus` value meaning "approved by the issuer".
const APPROVAL_STATUS_APPROVED: &str = "1";
/// `merchant.bin` selecting the Stratus host. Only Stratus supports
/// `mcProgramProtocol` / `mcDirectoryTransID`.
const BIN_STRATUS: &str = "000001";
/// `additionalAuthInfo.pymtBrandProgramCode` for Discover ProtectBuy.
const BRAND_PROGRAM_DISCOVER_PROTECTBUY: &str = "DPB";
/// `additionalAuthInfo.pymtBrandProgramCode` for American Express SafeKey.
const BRAND_PROGRAM_AMEX_SAFEKEY: &str = "ASK";

/// Max length of `order.orderID`.
const MAX_ORDER_ID_LEN: usize = 22;
/// Max length of `order.amount`.
const MAX_AMOUNT_LEN: usize = 12;

// =============================================================================
// AUTH
// =============================================================================

#[derive(Debug, Clone)]
pub struct JpmorganOrbitalAuthType {
    pub username: Secret<String>,
    pub password: Secret<String>,
    pub merchant_id: Secret<String>,
    pub bin: String,
    pub terminal_id: String,
}

impl TryFrom<&ConnectorSpecificConfig> for JpmorganOrbitalAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(config: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match config {
            ConnectorSpecificConfig::JpmorganOrbital {
                username,
                password,
                merchant_id,
                bin,
                terminal_id,
                ..
            } => {
                let bin = bin
                    .clone()
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| {
                        error_stack::report!(IntegrationError::InvalidConnectorConfig {
                        config: "jpmorganorbital.bin",
                        context: IntegrationErrorContext {
                            suggested_action: Some(
                                "Set `bin` on the JpmorganOrbital connector config: \"000001\" \
                                 for the Stratus host (US) or \"000002\" for Tandem (Canada)."
                                    .to_string(),
                            ),
                            ..Default::default()
                        },
                    })
                    })?;
                let terminal_id = terminal_id
                    .clone()
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| {
                        error_stack::report!(IntegrationError::InvalidConnectorConfig {
                            config: "jpmorganorbital.terminal_id",
                            context: IntegrationErrorContext {
                                suggested_action: Some(
                                    "Set `terminal_id` on the JpmorganOrbital connector config \
                                     (3 digits; \"001\" on Stratus, \"001\"-\"999\" on Tandem)."
                                        .to_string(),
                                ),
                                ..Default::default()
                            },
                        })
                    })?;

                Ok(Self {
                    username: username.clone(),
                    password: password.clone(),
                    merchant_id: merchant_id.clone(),
                    bin: bin.trim().to_string(),
                    terminal_id: terminal_id.trim().to_string(),
                })
            }
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Send the JP Morgan Orbital credentials as \
                             x-connector-config: {\"config\":{\"JpmorganOrbital\":{\"username\":…,\
                             \"password\":…,\"merchant_id\":…,\"bin\":…,\"terminal_id\":…}}}."
                                .to_string(),
                        ),
                        ..Default::default()
                    },
                }
            )),
        }
    }
}

// =============================================================================
// AMOUNT — two implied decimals for EVERY currency
// =============================================================================

pub fn orbital_amount(
    minor_amount: MinorUnit,
    currency: Currency,
) -> Result<String, error_stack::Report<IntegrationError>> {
    let major = JpmorganOrbitalAmountConvertor::convert(minor_amount, currency)?;
    shift_two_decimal_places(&major.get_amount_as_string())
}

/// Multiply a decimal string by 100 without floating point, and without a decimal
/// point, sign, separator or currency symbol in the result.
fn shift_two_decimal_places(major: &str) -> Result<String, error_stack::Report<IntegrationError>> {
    let invalid = |reason: &str| {
        error_stack::report!(IntegrationError::InvalidDataFormat {
            field_name: "order.amount",
            context: IntegrationErrorContext {
                additional_context: Some(format!(
                    "JP Morgan Orbital amount `{major}` could not be rendered with two implied \
                     decimals: {reason}"
                )),
                ..Default::default()
            },
        })
    };

    let value = major.trim();
    if value.starts_with('-') {
        return Err(invalid("Orbital's order.amount is unsigned"));
    }

    let (integral, fractional) = value.split_once('.').unwrap_or((value, ""));
    if integral.is_empty() && fractional.is_empty() {
        return Err(invalid("empty amount"));
    }
    if !integral.bytes().all(|b| b.is_ascii_digit())
        || !fractional.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(invalid("non-numeric characters"));
    }

    let mut cents = fractional.to_string();
    if cents.len() > 2 {
        if cents[2..].bytes().any(|b| b != b'0') {
            return Err(invalid(
                "the currency has more than two decimal places and the sub-cent digits are \
                 non-zero; Orbital cannot represent this amount",
            ));
        }
        cents.truncate(2);
    }
    while cents.len() < 2 {
        cents.push('0');
    }

    let combined = format!("{integral}{cents}");
    let trimmed = combined.trim_start_matches('0');
    let rendered = if trimmed.is_empty() {
        "0".to_string()
    } else {
        trimmed.to_string()
    };

    if rendered.len() > MAX_AMOUNT_LEN {
        return Err(invalid("exceeds the 12-character maximum"));
    }
    Ok(rendered)
}

// =============================================================================
// ORDER ID + RETRY TRACE
// =============================================================================

fn build_order_id(reference: &str) -> Result<String, error_stack::Report<IntegrationError>> {
    let sanitized: String = reference
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | ',' | '$' | '@' | '&' | ' ') {
                c
            } else {
                '-'
            }
        })
        .collect();

    let truncated = if sanitized.len() > MAX_ORDER_ID_LEN {
        sanitized
            .get(sanitized.len() - MAX_ORDER_ID_LEN..)
            .unwrap_or(&sanitized)
            .to_string()
    } else {
        sanitized
    };

    // A leading space is explicitly illegal.
    let order_id = truncated.trim_start().to_string();
    if order_id.is_empty() {
        return Err(error_stack::report!(
            IntegrationError::MissingRequiredField {
                field_name: "connector_request_reference_id",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "JP Morgan Orbital requires a non-empty order.orderID".to_string(),
                    ),
                    ..Default::default()
                },
            }
        ));
    }
    Ok(order_id)
}

pub fn build_retry_trace(reference: &str) -> String {
    const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

    let mut hash = FNV_OFFSET_BASIS;
    for byte in reference.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    let trace = 1_000_000_000_000_000_u64 + (hash % 9_000_000_000_000_000_u64);
    trace.to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrbitalBrand {
    Visa,
    Mastercard,
    Discover,
    Amex,
    Other,
}

fn resolve_brand<T: PaymentMethodDataTypes>(card: &Card<T>) -> OrbitalBrand {
    if let Some(network) = card.card_network.as_ref() {
        return match network {
            CardNetwork::Visa => OrbitalBrand::Visa,
            // International Maestro shares Mastercard's AAV field and ECI scale.
            CardNetwork::Mastercard | CardNetwork::Maestro => OrbitalBrand::Mastercard,
            CardNetwork::Discover => OrbitalBrand::Discover,
            CardNetwork::AmericanExpress => OrbitalBrand::Amex,
            _ => OrbitalBrand::Other,
        };
    }

    match get_card_issuer(card.card_number.peek()) {
        Ok(CardIssuer::Visa) => OrbitalBrand::Visa,
        Ok(CardIssuer::Master) | Ok(CardIssuer::Maestro) => OrbitalBrand::Mastercard,
        Ok(CardIssuer::Discover) => OrbitalBrand::Discover,
        Ok(CardIssuer::AmericanExpress) => OrbitalBrand::Amex,
        _ => OrbitalBrand::Other,
    }
}

// =============================================================================
// REQUEST — POST /payments
// =============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct JpmorganOrbitalMerchant {
    /// `000001` = Stratus (US), `000002` = Tandem (Canada).
    pub bin: String,
    /// Documented mandatory in the Payments Request guide but absent from every
    /// OpenAPI example; the header is authoritative. Sent here too because it cannot
    /// conflict (there is only one credential) and it satisfies the stricter reading.
    #[serde(rename = "merchantID")]
    pub merchant_id: Secret<String>,
    #[serde(rename = "terminalID")]
    pub terminal_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct JpmorganOrbitalCard<T: PaymentMethodDataTypes> {
    /// The PAN.
    #[serde(rename = "ccAccountNum")]
    pub cc_account_num: RawCardNumber<T>,
    /// `YYYYMM` — four-digit year first.
    #[serde(rename = "ccExp")]
    pub cc_exp: Secret<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JpmorganOrbitalPaymentInstrument<T: PaymentMethodDataTypes> {
    pub card: JpmorganOrbitalCard<T>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JpmorganOrbitalOrder {
    #[serde(rename = "orderID")]
    pub order_id: String,
    /// Two implied decimals for every currency. See [`orbital_amount`].
    pub amount: String,
    #[serde(rename = "industryType")]
    pub industry_type: String,
    /// Idempotency key **and** the `/inquiry` lookup key. Always sent.
    #[serde(rename = "retryTrace")]
    pub retry_trace: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct JpmorganOrbitalCardholderVerification {
    #[serde(rename = "ccCardVerifyNum")]
    pub cc_card_verify_num: Secret<String>,
    /// `1` = the value is present. Mandatory alongside the CVV for Visa, Discover,
    /// JCB and UnionPay, optional for Mastercard — always sending `1` when a CVV is
    /// present is correct for every brand.
    #[serde(rename = "ccCardVerifyPresenceInd")]
    pub cc_card_verify_presence_ind: String,
}

/// 3DS only. Emitted solely when the external authentication produced a usable
/// CAVV/ECI pair; never emitted empty.
#[derive(Debug, Clone, Default, Serialize)]
pub struct JpmorganOrbitalCryptogram {
    /// Visa (and ChaseNet) CAVV. Already Base64 — passed through verbatim.
    #[serde(rename = "verifyByVisaCAVV", skip_serializing_if = "Option::is_none")]
    pub verify_by_visa_cavv: Option<Secret<String>>,
    /// Visa XID. Base64-only; sent only when the upstream value already is Base64.
    #[serde(rename = "verifyByVisaXID", skip_serializing_if = "Option::is_none")]
    pub verify_by_visa_xid: Option<Secret<String>>,
    /// Mastercard / International Maestro AAV.
    #[serde(rename = "mcSecureCodeAAV", skip_serializing_if = "Option::is_none")]
    pub mc_secure_code_aav: Option<Secret<String>>,
    /// Discover ProtectBuy and Amex SafeKey both carry the CAVV here.
    #[serde(
        rename = "digitalTokenCryptogram",
        skip_serializing_if = "Option::is_none"
    )]
    pub digital_token_cryptogram: Option<Secret<String>>,
}

/// 3DS only. Companion of [`JpmorganOrbitalCryptogram`].
#[derive(Debug, Clone, Default, Serialize)]
pub struct JpmorganOrbitalAdditionalAuthInfo {
    /// `5` authenticated, `6` attempted, `7` not authenticated. Note this is *not*
    /// the raw UCS ECI — see [`map_eci`].
    #[serde(
        rename = "authenticationECIInd",
        skip_serializing_if = "Option::is_none"
    )]
    pub authentication_eci_ind: Option<String>,
    /// `DPB` (Discover ProtectBuy) or `ASK` (Amex SafeKey); omitted for Visa/MC.
    #[serde(
        rename = "pymtBrandProgramCode",
        skip_serializing_if = "Option::is_none"
    )]
    pub pymt_brand_program_code: Option<String>,
    /// Mastercard UCAF Collection Indicator. Passed through only when UCS supplies
    /// one; never derived, because Merchant Services derives every value but `4`.
    #[serde(rename = "ucafInd", skip_serializing_if = "Option::is_none")]
    pub ucaf_ind: Option<String>,
    /// `1` = 3DS 1.0, `2` = 3DS 2.x. Mastercard, Stratus (BIN `000001`) only.
    #[serde(rename = "mcProgramProtocol", skip_serializing_if = "Option::is_none")]
    pub mc_program_protocol: Option<String>,
    /// Mastercard Directory Server Transaction ID. Stratus only.
    #[serde(rename = "mcDirectoryTransID", skip_serializing_if = "Option::is_none")]
    pub mc_directory_trans_id: Option<String>,
}

/// `POST /payments` body. Identical for non-3DS and 3DS apart from the two trailing
/// objects — Orbital never redirects, so there is no return/callback URL to carry.
#[derive(Debug, Clone, Serialize)]
pub struct JpmorganOrbitalPaymentsRequest<T: PaymentMethodDataTypes> {
    pub version: String,
    #[serde(rename = "transType")]
    pub trans_type: String,
    pub merchant: JpmorganOrbitalMerchant,
    #[serde(rename = "paymentInstrument")]
    pub payment_instrument: JpmorganOrbitalPaymentInstrument<T>,
    pub order: JpmorganOrbitalOrder,
    #[serde(
        rename = "cardholderVerification",
        skip_serializing_if = "Option::is_none"
    )]
    pub cardholder_verification: Option<JpmorganOrbitalCardholderVerification>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cryptogram: Option<JpmorganOrbitalCryptogram>,
    #[serde(rename = "additionalAuthInfo", skip_serializing_if = "Option::is_none")]
    pub additional_auth_info: Option<JpmorganOrbitalAdditionalAuthInfo>,
}

/// Map UCS `capture_method` onto Orbital's `transType`.
fn trans_type_for(
    capture_method: Option<CaptureMethod>,
) -> Result<&'static str, error_stack::Report<IntegrationError>> {
    match capture_method {
        None | Some(CaptureMethod::Automatic) => Ok(TRANS_TYPE_AUTH_CAPTURE),
        Some(CaptureMethod::Manual) => Ok(TRANS_TYPE_AUTH_ONLY),
        Some(other) => Err(error_stack::report!(IntegrationError::NotSupported {
            message: format!("Capture method {other:?}"),
            connector: "jpmorganorbital",
            context: IntegrationErrorContext::default(),
        })),
    }
}

fn looks_base64(value: &str) -> bool {
    !value.is_empty()
        && value.len().is_multiple_of(4)
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'=')
}


fn map_eci(eci: &str, brand: OrbitalBrand) -> Option<&'static str> {
    let trimmed = eci.trim();
    let unpadded = trimmed.trim_start_matches('0');
    let normalized = if unpadded.is_empty() { "0" } else { unpadded };

    match brand {
        OrbitalBrand::Mastercard => match normalized {
            "2" => Some("5"), // full authentication
            "1" => Some("6"), // attempted
            "0" => Some("7"), // not authenticated
            _ => None,
        },
        OrbitalBrand::Visa | OrbitalBrand::Discover | OrbitalBrand::Amex => match normalized {
            "5" => Some("5"),
            "6" => Some("6"),
            "7" => Some("7"),
            _ => None,
        },
        // No documented ECI mapping for other brands; treat as non-3DS rather than
        // guessing a value the gateway would interpret as something else.
        OrbitalBrand::Other => None,
    }
}

/// Build the two 3DS objects from externally produced authentication data.
///
/// Returns `None` when the payment is effectively non-3DS: no CAVV, or an ECI that
/// does not map onto Orbital's scale. Orbital's own rule is that the CAVV and the
/// ECI must always travel together, so a half-populated pair is never emitted.
fn build_three_ds_objects<T: PaymentMethodDataTypes>(
    card: &Card<T>,
    authentication_data: &AuthenticationData,
    bin: &str,
) -> Option<(JpmorganOrbitalCryptogram, JpmorganOrbitalAdditionalAuthInfo)> {
    let cavv = authentication_data.cavv.as_ref()?;
    let brand = resolve_brand(card);
    let eci = map_eci(authentication_data.eci.as_deref()?, brand)?;

    let mut cryptogram = JpmorganOrbitalCryptogram::default();
    let mut additional = JpmorganOrbitalAdditionalAuthInfo {
        authentication_eci_ind: Some(eci.to_string()),
        ..Default::default()
    };

    match brand {
        OrbitalBrand::Visa => {
            // Pass the CAVV through verbatim — it is already Base64 as produced by
            // the 3DS server. Re-encoding it is the single most likely bug here and
            // surfaces as a respCode 37/245 decline, not as an error.
            cryptogram.verify_by_visa_cavv = Some(cavv.clone());
            cryptogram.verify_by_visa_xid = authentication_data
                .threeds_server_transaction_id
                .as_deref()
                .filter(|xid| looks_base64(xid))
                .map(|xid| Secret::new(xid.to_string()));
        }
        OrbitalBrand::Mastercard => {
            cryptogram.mc_secure_code_aav = Some(cavv.clone());
            // UCS carries a first-class UCAF collection indicator; pass it through
            // when present, never derive one.
            additional
                .ucaf_ind
                .clone_from(&authentication_data.ucaf_collection_indicator);
            if bin == BIN_STRATUS {
                additional.mc_program_protocol = authentication_data
                    .message_version
                    .as_ref()
                    .and_then(|version| version.to_string().chars().next())
                    .filter(|major| matches!(major, '1' | '2'))
                    .map(|major| major.to_string());
                additional
                    .mc_directory_trans_id
                    .clone_from(&authentication_data.ds_trans_id);
            }
        }
        OrbitalBrand::Discover => {
            cryptogram.digital_token_cryptogram = Some(cavv.clone());
            additional.pymt_brand_program_code =
                Some(BRAND_PROGRAM_DISCOVER_PROTECTBUY.to_string());
        }
        OrbitalBrand::Amex => {
            cryptogram.digital_token_cryptogram = Some(cavv.clone());
            additional.pymt_brand_program_code = Some(BRAND_PROGRAM_AMEX_SAFEKEY.to_string());
        }
        OrbitalBrand::Other => return None,
    }

    Some((cryptogram, additional))
}

type AuthorizeRouterData<T> =
    RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<JpmorganOrbitalRouterData<AuthorizeRouterData<T>, T>>
    for JpmorganOrbitalPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: JpmorganOrbitalRouterData<AuthorizeRouterData<T>, T>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;

        let card = match &request.payment_method_data {
            PaymentMethodData::Card(card) => card,
            other => {
                return Err(error_stack::report!(IntegrationError::NotSupported {
                    message: format!("Payment method {}", payment_method_label(other)),
                    connector: "jpmorganorbital",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "JP Morgan Orbital is integrated for card payments only.".to_string(),
                        ),
                        ..Default::default()
                    },
                }))
            }
        };

        // Mandates / MIT / stored credentials are explicitly out of scope: the
        // gateway models them with `merchantInitiatedTransaction` / `profile` fields
        // this connector does not send, so accepting the request would quietly drop
        // the credential-on-file signalling the networks require.
        if request.mandate_id.is_some() || request.setup_mandate_details.is_some() {
            return Err(error_stack::report!(IntegrationError::NotSupported {
                message: "Mandates / merchant-initiated transactions".to_string(),
                connector: "jpmorganorbital",
                context: IntegrationErrorContext::default(),
            }));
        }

        let auth = JpmorganOrbitalAuthType::try_from(&router_data.connector_config)?;
        let common = &router_data.resource_common_data;

        let cc_exp = card.get_expiry_date_as_yyyymm("");

        let cvc = card.card_cvc.peek().trim().to_string();
        let cardholder_verification =
            (!cvc.is_empty()).then(|| JpmorganOrbitalCardholderVerification {
                cc_card_verify_num: Secret::new(cvc),
                cc_card_verify_presence_ind: CARD_VERIFY_PRESENCE_PRESENT.to_string(),
            });

        // Orbital 3DS is a passthrough: the challenge (if any) already happened in the
        // merchant's own MPI / the UCS external-authentication flow. There is no
        // second call and no redirect to build here.
        let (cryptogram, additional_auth_info) = match request.authentication_data.as_ref() {
            Some(authentication_data) => {
                match build_three_ds_objects(card, authentication_data, &auth.bin) {
                    Some((cryptogram, additional)) => (Some(cryptogram), Some(additional)),
                    None => (None, None),
                }
            }
            None => (None, None),
        };

        Ok(Self {
            version: ORBITAL_VERSION.to_string(),
            trans_type: trans_type_for(request.capture_method)?.to_string(),
            merchant: JpmorganOrbitalMerchant {
                bin: auth.bin.clone(),
                merchant_id: auth.merchant_id.clone(),
                terminal_id: auth.terminal_id.clone(),
            },
            payment_instrument: JpmorganOrbitalPaymentInstrument {
                card: JpmorganOrbitalCard {
                    cc_account_num: card.card_number.clone(),
                    cc_exp,
                },
            },
            order: JpmorganOrbitalOrder {
                order_id: build_order_id(&common.connector_request_reference_id)?,
                // Currency is deliberately absent: Orbital derives it from the
                // Merchant ID setup and has no field to receive it.
                amount: orbital_amount(request.minor_amount, request.currency)?,
                industry_type: INDUSTRY_TYPE_ECOMMERCE.to_string(),
                retry_trace: build_retry_trace(&common.connector_request_reference_id),
            },
            cardholder_verification,
            cryptogram,
            additional_auth_info,
        })
    }
}

/// Human-readable label for the unsupported-payment-method error.
fn payment_method_label<T: PaymentMethodDataTypes>(data: &PaymentMethodData<T>) -> &'static str {
    match data {
        PaymentMethodData::Card(_) => "card",
        PaymentMethodData::CardWithNoCvc(_) => "card without CVC",
        PaymentMethodData::CardDetailsForNetworkTransactionId(_) => "network transaction id card",
        PaymentMethodData::CardRedirect(_) => "card redirect",
        PaymentMethodData::Wallet(_) => "wallet",
        PaymentMethodData::PayLater(_) => "pay later",
        PaymentMethodData::BankRedirect(_) => "bank redirect",
        PaymentMethodData::BankDebit(_) => "bank debit",
        PaymentMethodData::BankTransfer(_) => "bank transfer",
        PaymentMethodData::Crypto(_) => "crypto",
        PaymentMethodData::MandatePayment => "mandate payment",
        PaymentMethodData::Reward => "reward",
        PaymentMethodData::RealTimePayment(_) => "real time payment",
        PaymentMethodData::Upi(_) => "UPI",
        PaymentMethodData::Voucher(_) => "voucher",
        PaymentMethodData::GiftCard(_) => "gift card",
        PaymentMethodData::PaymentMethodToken(_) => "payment method token",
        PaymentMethodData::OpenBanking(_) => "open banking",
        PaymentMethodData::NetworkToken(_) => "network token",
        PaymentMethodData::MobilePayment(_) => "mobile payment",
        PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_) => {
            "decrypted wallet token"
        }
    }
}

// =============================================================================
// REQUEST — POST /inquiry (PSync)
// =============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct JpmorganOrbitalInquiryOrder {
    #[serde(rename = "orderID", skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
    /// **The `retryTrace` of the original `/payments` request.** Not a counter, not
    /// the `txRefNum` — Orbital has no "get transaction by txRefNum" endpoint.
    #[serde(rename = "inquiryRetryNumber")]
    pub inquiry_retry_number: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct JpmorganOrbitalInquiryRequest {
    pub version: String,
    pub merchant: JpmorganOrbitalMerchant,
    pub order: JpmorganOrbitalInquiryOrder,
}

type SyncRouterData = RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<JpmorganOrbitalRouterData<SyncRouterData, T>> for JpmorganOrbitalInquiryRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: JpmorganOrbitalRouterData<SyncRouterData, T>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = JpmorganOrbitalAuthType::try_from(&router_data.connector_config)?;
        let reference = &router_data
            .resource_common_data
            .connector_request_reference_id;

        Ok(Self {
            version: ORBITAL_VERSION.to_string(),
            merchant: JpmorganOrbitalMerchant {
                bin: auth.bin.clone(),
                merchant_id: auth.merchant_id.clone(),
                terminal_id: auth.terminal_id.clone(),
            },
            order: JpmorganOrbitalInquiryOrder {
                order_id: Some(build_order_id(reference)?),
                inquiry_retry_number: build_retry_trace(reference),
            },
        })
    }
}

// =============================================================================
// RESPONSE
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JpmorganOrbitalStatus {
    #[serde(rename = "procStatus")]
    pub proc_status: Option<String>,
    #[serde(rename = "procStatusMessage")]
    pub proc_status_message: Option<String>,
    #[serde(rename = "hostRespCode")]
    pub host_resp_code: Option<String>,
    #[serde(rename = "actualRespCd")]
    pub actual_resp_cd: Option<String>,
    /// Normalized authorization response code. `"00"` = approved; every other value
    /// is passed through verbatim.
    #[serde(rename = "respCode")]
    pub resp_code: Option<String>,
    #[serde(rename = "respCodeMessage")]
    pub resp_code_message: Option<String>,
    /// `0` declined, `1` approved, `2` message/system error.
    #[serde(rename = "approvalStatus")]
    pub approval_status: Option<String>,
    #[serde(rename = "authorizationCode")]
    pub authorization_code: Option<String>,
    #[serde(rename = "partialAuthOccurred")]
    pub partial_auth_occurred: Option<String>,
    /// CAVV response code for Visa Secure. Informational only — never derive the
    /// attempt status from it.
    #[serde(rename = "visaVbVRespCode")]
    pub visa_vbv_resp_code: Option<String>,
    #[serde(rename = "pymtBrandAuthResponseCode")]
    pub pymt_brand_auth_response_code: Option<String>,
    #[serde(rename = "pymtBrandResponseCodeCategory")]
    pub pymt_brand_response_code_category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JpmorganOrbitalOrderResponse {
    #[serde(rename = "orderID")]
    pub order_id: Option<String>,
    #[serde(rename = "industryType")]
    pub industry_type: Option<String>,
    /// Gateway Transaction Reference Number → `connector_transaction_id`.
    #[serde(rename = "txRefNum")]
    pub tx_ref_num: Option<String>,
    /// Gateway Transaction Index. Required *alongside* `txRefNum` by any later
    /// Capture / Void / Refund, so it is persisted even though those flows are out of
    /// scope — without it a follow-up PR would need a data migration.
    #[serde(rename = "txRefIdx")]
    pub tx_ref_idx: Option<String>,
    #[serde(rename = "respDateTime")]
    pub resp_date_time: Option<String>,
    #[serde(rename = "authNetwkID")]
    pub auth_netwk_id: Option<String>,
    /// `0` = first response for this `retryTrace`; `>= 1` = an echoed duplicate.
    #[serde(rename = "retryAttempCount")]
    pub retry_attempt_count: Option<String>,
    pub status: Option<JpmorganOrbitalStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JpmorganOrbitalCardResponse {
    #[serde(rename = "cardBrand")]
    pub card_brand: Option<String>,
    /// Masked in the response (`545454XXXXXX5454`) — never a usable PAN.
    #[serde(rename = "ccAccountNum")]
    pub cc_account_num: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JpmorganOrbitalPaymentInstrumentResponse {
    pub card: Option<JpmorganOrbitalCardResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JpmorganOrbitalAvsBilling {
    #[serde(rename = "avsRespCode")]
    pub avs_resp_code: Option<String>,
    #[serde(rename = "hostAVSRespCode")]
    pub host_avs_resp_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JpmorganOrbitalCardholderVerificationResponse {
    #[serde(rename = "cvvRespCode")]
    pub cvv_resp_code: Option<String>,
    #[serde(rename = "hostCVVRespCode")]
    pub host_cvv_resp_code: Option<String>,
}

/// `paymentsResponse`, and also the flat `{procStatus, procStatusMessage}` error body
/// every non-2xx uses.
///
/// Orbital nests the status at `order.status` on a 200 but puts the very same two
/// fields at the **top level** on an error, so both shapes are modelled here and
/// [`JpmorganOrbitalPaymentsResponse::proc_status_value`] reads whichever is present.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JpmorganOrbitalPaymentsResponse {
    pub version: Option<String>,
    #[serde(rename = "transType")]
    pub trans_type: Option<String>,
    pub merchant: Option<serde_json::Value>,
    #[serde(rename = "paymentInstrument")]
    pub payment_instrument: Option<JpmorganOrbitalPaymentInstrumentResponse>,
    pub order: Option<JpmorganOrbitalOrderResponse>,
    #[serde(rename = "avsBilling")]
    pub avs_billing: Option<JpmorganOrbitalAvsBilling>,
    #[serde(rename = "cardholderVerification")]
    pub cardholder_verification: Option<JpmorganOrbitalCardholderVerificationResponse>,
    /// Top-level, error-body-only form.
    #[serde(rename = "procStatus")]
    pub proc_status: Option<String>,
    /// Top-level, error-body-only form.
    #[serde(rename = "procStatusMessage")]
    pub proc_status_message: Option<String>,
}

/// `/inquiry` answers with the same envelope; the newtype only gives the macro
/// framework a distinct response type per flow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JpmorganOrbitalInquiryResponse(pub JpmorganOrbitalPaymentsResponse);

impl JpmorganOrbitalPaymentsResponse {
    fn status(&self) -> Option<&JpmorganOrbitalStatus> {
        self.order.as_ref()?.status.as_ref()
    }

    /// `procStatus` from wherever this response carries it.
    fn proc_status_value(&self) -> Option<&str> {
        self.status()
            .and_then(|status| status.proc_status.as_deref())
            .or(self.proc_status.as_deref())
    }

    fn proc_status_message_value(&self) -> Option<&str> {
        self.status()
            .and_then(|status| status.proc_status_message.as_deref())
            .or(self.proc_status_message.as_deref())
    }

    fn approval_status_value(&self) -> Option<&str> {
        self.status()
            .and_then(|status| status.approval_status.as_deref())
    }

    fn gateway_accepted(&self) -> bool {
        self.proc_status_value()
            .map(|value| value.trim() == PROC_STATUS_SUCCESS)
            .unwrap_or(false)
    }

    fn issuer_approved(&self) -> bool {
        self.approval_status_value()
            .map(|value| value.trim() == APPROVAL_STATUS_APPROVED)
            .unwrap_or(false)
    }

    /// Orbital is a two-level protocol: the Gateway edit checks must pass **and** the
    /// issuer must approve. Both levels are checked, in that order.
    pub fn is_success(&self) -> bool {
        self.gateway_accepted() && self.issuer_approved()
    }

    pub fn connector_transaction_id(&self) -> Option<String> {
        self.order
            .as_ref()?
            .tx_ref_num
            .as_ref()
            .filter(|value| !value.trim().is_empty())
            .cloned()
    }

    fn order_id(&self) -> Option<String> {
        self.order.as_ref()?.order_id.clone()
    }

    /// Persist everything a later Capture / Void / Refund or PSync will need.
    /// `txRefIdx` in particular is useless to this flow but mandatory for those.
    fn connector_metadata(&self, retry_trace: &str) -> Option<serde_json::Value> {
        let order = self.order.as_ref()?;
        Some(serde_json::json!({
            "tx_ref_idx": order.tx_ref_idx,
            "order_id": order.order_id,
            "retry_trace": retry_trace,
            "retry_attempt_count": order.retry_attempt_count,
            "auth_netwk_id": order.auth_netwk_id,
            "authorization_code": self.status().and_then(|s| s.authorization_code.clone()),
            "visa_vbv_resp_code": self.status().and_then(|s| s.visa_vbv_resp_code.clone()),
            "avs_resp_code": self.avs_billing.as_ref().and_then(|a| a.avs_resp_code.clone()),
            "cvv_resp_code": self
                .cardholder_verification
                .as_ref()
                .and_then(|c| c.cvv_resp_code.clone()),
        }))
    }

    /// `Charged` vs `Authorized` is decided by the `transType` **sent**, not by
    /// anything in the response: an `AC` approval and an `A` approval produce byte
    /// identical `status` objects.
    fn success_status(sent_trans_type: &str) -> AttemptStatus {
        if sent_trans_type == TRANS_TYPE_AUTH_CAPTURE {
            AttemptStatus::Charged
        } else {
            AttemptStatus::Authorized
        }
    }

    /// Every failure mode of this connector is terminal. `/payments` is synchronous,
    /// there is no challenge state and no webhook, so nothing here may map to
    /// `Pending` / `AuthenticationPending` / `Started`.
    fn failure_status(&self) -> AttemptStatus {
        AttemptStatus::Failure
    }

    /// Build an `ErrorResponse` covering all three of Orbital's failure layers:
    /// a non-2xx with the flat `messages` body, a 200 whose `procStatus != "0"`
    /// (gateway edit-check failure) and a 200 whose `approvalStatus != "1"`
    /// (issuer decline or host error).
    pub fn to_error_response(&self, http_code: u16) -> ErrorResponse {
        let gateway_failure = !self.gateway_accepted();

        let (code, message, reason) = if gateway_failure {
            (
                self.proc_status_value()
                    .map(str::to_string)
                    .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                self.proc_status_message_value()
                    .map(str::to_string)
                    .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                self.proc_status_message_value().map(str::to_string),
            )
        } else {
            let status = self.status();
            (
                status
                    .and_then(|s| s.resp_code.clone())
                    .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                status
                    .and_then(|s| s.resp_code_message.clone())
                    .or_else(|| self.proc_status_message_value().map(str::to_string))
                    .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                status
                    .and_then(|s| s.host_resp_code.clone())
                    .or_else(|| status.and_then(|s| s.resp_code_message.clone())),
            )
        };

        ErrorResponse {
            status_code: http_code,
            code,
            message,
            reason,
            attempt_status: Some(FlowStatus::Payment(self.failure_status())),
            // The attempt exists at the gateway whenever a txRefNum came back, even
            // on a decline, so surface it.
            connector_transaction_id: self.connector_transaction_id(),
            // Orbital exposes no network decline/advice fields of its own.
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
            typed_connector_response: None,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        }
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<JpmorganOrbitalPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<JpmorganOrbitalPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let retry_trace = build_retry_trace(
            &item
                .router_data
                .resource_common_data
                .connector_request_reference_id,
        );

        if !response.is_success() {
            return Ok(Self {
                response: Err(response.to_error_response(item.http_code)),
                resource_common_data: PaymentFlowData {
                    status: response.failure_status(),
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        // Derived from the request, never from the response — see `success_status`.
        let sent_trans_type =
            trans_type_for(item.router_data.request.capture_method).map_err(|_| {
                error_stack::report!(crate::utils::unexpected_response_fail(
                    item.http_code,
                    "jpmorganorbital: capture_method is not supported by this connector, so the \
                     transType that produced this response cannot be reconstructed",
                ))
            })?;

        let resource_id = match response.connector_transaction_id() {
            Some(tx_ref_num) => ResponseId::ConnectorTransactionId(tx_ref_num),
            None => ResponseId::NoResponseId,
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id,
                // Orbital never redirects: 3DS is external passthrough and
                // `paymentsResponse` has no URL field of any kind.
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: response.connector_metadata(&retry_trace),
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: response.order_id(),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status: JpmorganOrbitalPaymentsResponse::success_status(sent_trans_type),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl TryFrom<ResponseRouterData<JpmorganOrbitalInquiryResponse, Self>> for SyncRouterData {
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<JpmorganOrbitalInquiryResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response.0;
        let retry_trace = build_retry_trace(
            &item
                .router_data
                .resource_common_data
                .connector_request_reference_id,
        );

        if !response.is_success() {
            return Ok(Self {
                response: Err(response.to_error_response(item.http_code)),
                resource_common_data: PaymentFlowData {
                    status: response.failure_status(),
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        // On `/inquiry` the echoed `transType` comes from the stored original
        // request, so it *is* authoritative here (unlike on `/payments`).
        let echoed_trans_type = response
            .trans_type
            .as_deref()
            .unwrap_or(TRANS_TYPE_AUTH_ONLY)
            .trim();

        let resource_id = match response.connector_transaction_id() {
            Some(tx_ref_num) => ResponseId::ConnectorTransactionId(tx_ref_num),
            None => item.router_data.request.connector_transaction_id.clone(),
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id,
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: response.connector_metadata(&retry_trace),
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: response.order_id(),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status: JpmorganOrbitalPaymentsResponse::success_status(echoed_trans_type),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}
