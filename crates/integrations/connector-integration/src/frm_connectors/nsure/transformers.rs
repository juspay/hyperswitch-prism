use common_enums::{AttemptStatus, Currency, FrmDecision};
use common_utils::types::FloatMajorUnit;
use domain_types::connector_flow::PreRiskCheck;
use domain_types::{
    connector_types::CustomerInfo,
    errors,
    frm::frm_types::{
        FrmChargebackReceivedRequest, FrmChargebackReceivedResponse, FrmFlowData,
        FrmPaymentOutcomeRequest, FrmPaymentOutcomeResponse, FrmRefundProcessedRequest,
        FrmRefundProcessedResponse, PreRiskCheckRequest, PreRiskCheckResponse,
    },
    payment_address::{Address, OrderDetailsWithAmount},
    payment_method_data::{DefaultPCIHolder, PaymentMethodData},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
};
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::types::ResponseRouterData;

pub(crate) type Error = error_stack::Report<errors::IntegrationError>;
type ResponseError = error_stack::Report<errors::ConnectorError>;

/// nSure.ai Server-to-Server API reference.
pub const NSURE_DOC_URL: &str =
    "https://docs.nsure.ai/docs/nsureai-open-api/tm3emswyhns77-server-to-server-api";

/// Default `x-nsure-api-version` when the merchant config does not pin one.
/// nSure documents the format as `apiVersion.major.minor`.
pub const NSURE_DEFAULT_API_VERSION: &str = "2.0.0";

// ──────────────────────────────────────────────────────────────────────────
// Auth
// ──────────────────────────────────────────────────────────────────────────

pub struct NsureAuthType {
    /// Sent verbatim in the `Authorization` header — nSure uses a bare key,
    /// with no `Bearer`/`Basic` scheme prefix.
    pub api_key: Secret<String>,
    pub app_id: Option<String>,
    pub api_version: Option<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for NsureAuthType {
    type Error = Error;

    fn try_from(config: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match config {
            ConnectorSpecificConfig::Nsure {
                api_key,
                app_id,
                api_version,
                ..
            } => Ok(Self {
                api_key: api_key.clone(),
                app_id: app_id.clone(),
                api_version: api_version.clone(),
            }),
            _ => Err(error_stack::report!(
                errors::IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext {
                        additional_context: Some(
                            "expected an Nsure connector config (api_key = the nSure.ai \
                             authorization key from the management portal)"
                                .to_owned(),
                        ),
                        suggested_action: Some(
                            "Send the nSure.ai credentials as connector_config.nsure".to_owned(),
                        ),
                        doc_url: Some(NSURE_DOC_URL.to_owned()),
                    },
                }
            )),
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Error response
// ──────────────────────────────────────────────────────────────────────────

/// nSure returns `{"error": {"message": "..."}}` for 400/401/500.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NsureErrorResponse {
    #[serde(default)]
    pub error: Option<NsureErrorDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NsureErrorDetail {
    #[serde(default)]
    pub message: Option<String>,
}

impl NsureErrorResponse {
    pub fn message(&self) -> String {
        self.error
            .as_ref()
            .and_then(|detail| detail.message.clone())
            .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string())
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Request — POST /transactions/{transactionId}
// ──────────────────────────────────────────────────────────────────────────

/// nSure evaluation mode. Only `preAuthorization` is implemented; the
/// `postAuthorization` variant is intentionally absent so it cannot be
/// selected by accident.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum NsureMode {
    #[serde(rename = "preAuthorization")]
    PreAuthorization,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NsurePreRiskCheckRequest {
    pub metadata: NsureMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_info: Option<NsureSessionInfo>,
    pub end_user_info: NsureEndUserInfo,
    pub mode: NsureMode,
    pub transaction_details: NsureTransactionDetails,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NsureMetadata {
    pub unique_request_id: String,
    /// Epoch milliseconds.
    pub timestamp: i128,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_type: Option<NsureAccountType>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NsureSessionInfo {
    /// Minted by the nSure browser/mobile SDK; optional within sessionInfo.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    /// Required by nSure whenever `sessionInfo` is present.
    pub user_agent: String,
    /// Required by nSure whenever `sessionInfo` is present.
    pub end_user_ip: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NsureEndUserInfo {
    /// Stable merchant-side user key. nSure's `endUserInfo` oneOf requires it and
    /// it is what lets them build cross-transaction history for the buyer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone_info: Option<NsurePhoneInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_first_seen_timestamp: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_first_successful_tx_timestamp: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NsurePhoneInfo {
    pub phone: Secret<String>,
    pub country_code: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NsureTransactionDetails {
    pub paid_amount: NsureAmount,
    pub payment_method: NsurePaymentMethod,
    /// Required, and nSure enforces `minItems: 1`. Items are vertical-specific
    /// `oneOf` variants keyed by `itemClass`; prism supplies the fields common to
    /// every variant and takes the vertical from `connector_feature_data`.
    pub cart: Vec<NsureCartItem>,
}

/// Fields required by every nSure cart-item variant (`brand`, `quantity`,
/// `itemFulfillment`, `sellingPrice`), plus the optional discriminator and the
/// vertical-specific extras that some variants demand.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NsureCartItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_class: Option<String>,
    pub brand: String,
    pub quantity: u16,
    pub item_fulfillment: NsureItemFulfillment,
    pub selling_price: NsureAmount,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sku: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categories: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_type: Option<String>,
}

/// nSure `itemFulfillment`. `digital` means instant, unrecoverable delivery —
/// a materially higher-risk shape than a shipped good.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NsureItemFulfillment {
    Digital,
    Physical,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NsureAmount {
    pub value_in_currency: FloatMajorUnit,
    pub currency: Currency,
}

/// nSure's `paymentMethod` is a `oneOf` discriminated by `type`, with `type`,
/// `paymentProcessor` and `billingInfo` shared across every variant. Only the
/// variants prism can fully populate are emitted; anything else uses the
/// documented `other` variant (which requires only `name`) so the body stays
/// valid rather than failing the variant's required fields.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NsurePaymentMethod {
    #[serde(rename = "type")]
    pub payment_method_type: NsurePaymentMethodType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_processor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method_first_successful_tx_timestamp: Option<i64>,
    // ── card-variant fields ──
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bin: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last4: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration_month: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration_year: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_holder_name: Option<Secret<String>>,
    // ── shared ──
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_info: Option<NsureBillingInfo>,
    /// Required by the `other` variant.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum NsurePaymentMethodType {
    Card,
    Paypal,
    BankTransfer,
    Other,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NsureBillingInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<NsureAddress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone_info: Option<NsurePhoneInfo>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NsureAddress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub street: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postal_code: Option<Secret<String>>,
}

// ──────────────────────────────────────────────────────────────────────────
// Connector feature data
// ──────────────────────────────────────────────────────────────────────────

/// Signals nSure wants that the FRM request has no first-class field for.
///
/// `sessionInfo.deviceId` is minted by the nSure browser/mobile SDK and
/// `paymentMethod.paymentProcessor` names the downstream PSP — neither is
/// present on `PreRiskCheckRequest`, so both are read from the request's
/// `connector_feature_data` JSON. Absent or unparseable data degrades to
/// `None` rather than failing the risk check: nSure treats both as optional.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NsureFeatureData {
    /// nSure SDK device fingerprint — their single highest-value signal.
    #[serde(default)]
    pub device_id: Option<String>,
    /// Downstream PSP name (nSure enum, e.g. `adyen`, `stripe`).
    #[serde(default)]
    pub payment_processor: Option<String>,
    /// Buyer registration state: `guest` | `private` | `business`.
    #[serde(default)]
    pub account_type: Option<NsureAccountType>,
    /// Epoch ms of the buyer's first interaction with the merchant (account age).
    #[serde(default)]
    pub user_first_seen_timestamp: Option<i64>,
    /// Epoch ms of the buyer's first successful transaction.
    #[serde(default)]
    pub user_first_successful_tx_timestamp: Option<i64>,
    /// Epoch ms the instrument was first used successfully (card tenure).
    #[serde(default)]
    pub payment_method_first_successful_tx_timestamp: Option<i64>,
    /// nSure basket vertical. Selects the `cart[]` oneOf variant; when unset the
    /// item is sent without `itemClass`, which nSure matches structurally.
    #[serde(default)]
    pub item_class: Option<String>,
    /// Vertical-specific fields some `itemClass` variants require (e.g. `gaming`
    /// needs `productType`). Passed straight through onto every cart item.
    #[serde(default)]
    pub product_type: Option<String>,
    /// Fallback `brand` when order_details carries none — `brand` is required.
    #[serde(default)]
    pub default_brand: Option<String>,
}

/// nSure `metadata.accountType`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NsureAccountType {
    Guest,
    Private,
    Business,
}

impl NsureFeatureData {
    fn parse(raw: Option<&Secret<String>>) -> Self {
        raw.and_then(|data| serde_json::from_str(data.peek()).ok())
            .unwrap_or_default()
    }

    /// nSure asks for the processor's raw response on the capture/failure
    /// status transitions. It has no first-class field on the FRM notification,
    /// so it rides in `connector_feature_data` alongside the other
    /// provider-specific signals.
    fn raw_processor_response(raw: Option<&Secret<String>>) -> Option<serde_json::Value> {
        let parsed: serde_json::Value = serde_json::from_str(raw?.peek()).ok()?;
        parsed.get("raw_payment_processor_response").cloned()
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Request construction
// ──────────────────────────────────────────────────────────────────────────

fn nsure_phone_info(
    number: Option<&Secret<String>>,
    country_code: Option<&String>,
) -> Option<NsurePhoneInfo> {
    // nSure requires both `phone` and `countryCode` on a phoneInfo object, so
    // a half-populated phone is dropped rather than sent as an invalid object.
    let phone = number?;
    let country_code = country_code?;
    Some(NsurePhoneInfo {
        phone: phone.clone(),
        // nSure expects the bare dialling digits, not a `+`-prefixed code.
        country_code: country_code.trim_start_matches('+').to_string(),
    })
}

fn nsure_end_user_info(
    customer: Option<&CustomerInfo>,
    feature_data: &NsureFeatureData,
) -> NsureEndUserInfo {
    let tenure = |info: NsureEndUserInfo| NsureEndUserInfo {
        user_first_seen_timestamp: feature_data.user_first_seen_timestamp,
        user_first_successful_tx_timestamp: feature_data.user_first_successful_tx_timestamp,
        ..info
    };
    let Some(customer) = customer else {
        return tenure(NsureEndUserInfo::default());
    };
    // `first_name`/`last_name` are preferred; fall back to splitting the
    // single `customer_name` field on the first space when they are absent.
    let (first_name, last_name) = match (&customer.first_name, &customer.last_name) {
        (None, None) => customer
            .customer_name
            .as_ref()
            .map(|name| {
                let full = name.peek().trim().to_string();
                match full.split_once(' ') {
                    Some((first, last)) => (
                        Some(Secret::new(first.to_string())),
                        Some(Secret::new(last.trim().to_string())),
                    ),
                    None => (Some(Secret::new(full)), None),
                }
            })
            .unwrap_or((None, None)),
        (first, last) => (first.clone(), last.clone()),
    };
    tenure(NsureEndUserInfo {
        id: customer
            .customer_id
            .as_ref()
            .map(|id| id.get_string_repr().to_string()),
        email: customer
            .customer_email
            .as_ref()
            .map(|email| Secret::new(email.peek().to_string())),
        first_name,
        last_name,
        phone_info: nsure_phone_info(
            customer.customer_phone_number.as_ref(),
            customer.customer_phone_country_code.as_ref(),
        ),
        ..Default::default()
    })
}

fn nsure_billing_info(address: Option<&Address>) -> Option<NsureBillingInfo> {
    let address = address?;
    let details = address.address.as_ref();
    let nsure_address = details.map(|details| NsureAddress {
        country: details.country.map(|country| country.to_string()),
        state: details.state.clone(),
        city: details.city.clone(),
        // nSure models the street as one line; join line1/line2 when both exist.
        street: match (details.line1.as_ref(), details.line2.as_ref()) {
            (Some(line1), Some(line2)) => {
                Some(Secret::new(format!("{} {}", line1.peek(), line2.peek())))
            }
            (Some(line1), None) => Some(line1.clone()),
            (None, Some(line2)) => Some(line2.clone()),
            (None, None) => None,
        },
        postal_code: details.zip.clone(),
    });
    let info = NsureBillingInfo {
        first_name: details.and_then(|details| details.first_name.clone()),
        last_name: details.and_then(|details| details.last_name.clone()),
        address: nsure_address,
        phone_info: address
            .phone
            .as_ref()
            .and_then(|phone| nsure_phone_info(phone.number.as_ref(), phone.country_code.as_ref())),
    };
    // Don't send an object where every field is empty.
    let is_empty = info.first_name.is_none()
        && info.last_name.is_none()
        && info.address.is_none()
        && info.phone_info.is_none();
    (!is_empty).then_some(info)
}

/// Split a PAN into (bin, last4). nSure's card variant requires `last4`; `bin`
/// is the first six digits, which it uses for issuer lookup.
fn card_bin_last4(pan: &str) -> (Option<Secret<String>>, Option<Secret<String>>) {
    let digits: String = pan.chars().filter(|c| c.is_ascii_digit()).collect();
    let bin = (digits.len() >= 6).then(|| Secret::new(digits[..6].to_string()));
    let last4 = (digits.len() >= 4).then(|| Secret::new(digits[digits.len() - 4..].to_string()));
    (bin, last4)
}

/// Normalise a 2- or 4-digit expiry year to the 4-digit form nSure documents.
fn four_digit_year(year: &Secret<String>) -> Secret<String> {
    let raw = year.peek().trim();
    if raw.len() == 2 {
        Secret::new(format!("20{raw}"))
    } else {
        Secret::new(raw.to_string())
    }
}

fn nsure_payment_method(
    payment_method: Option<&PaymentMethodData<DefaultPCIHolder>>,
    billing_info: Option<NsureBillingInfo>,
    payment_processor: Option<String>,
    merchant_id: Option<String>,
    payment_method_first_successful_tx_timestamp: Option<i64>,
) -> NsurePaymentMethod {
    let base = NsurePaymentMethod {
        payment_method_type: NsurePaymentMethodType::Other,
        payment_processor,
        merchant_id,
        payment_method_first_successful_tx_timestamp,
        bin: None,
        last4: None,
        expiration_month: None,
        expiration_year: None,
        card_holder_name: None,
        billing_info,
        name: None,
    };
    match payment_method {
        Some(PaymentMethodData::Card(card)) => {
            let (bin, last4) = card_bin_last4(card.card_number.peek());
            NsurePaymentMethod {
                payment_method_type: NsurePaymentMethodType::Card,
                bin,
                last4,
                expiration_month: Some(card.card_exp_month.clone()),
                expiration_year: Some(four_digit_year(&card.card_exp_year)),
                card_holder_name: card.card_holder_name.clone(),
                ..base
            }
        }
        Some(PaymentMethodData::Wallet(wallet)) => {
            // nSure's `digitalWallet` variant requires `last4`, which prism's
            // wallet payloads do not carry, so only PayPal gets a first-class
            // variant. Every other wallet uses `other` with the wallet name,
            // which keeps the signal without producing an invalid body.
            match wallet {
                domain_types::payment_method_data::WalletData::PaypalRedirect(_)
                | domain_types::payment_method_data::WalletData::PaypalSdk(_) => {
                    NsurePaymentMethod {
                        payment_method_type: NsurePaymentMethodType::Paypal,
                        ..base
                    }
                }
                domain_types::payment_method_data::WalletData::GooglePay(_)
                | domain_types::payment_method_data::WalletData::GooglePayRedirect(_) => {
                    NsurePaymentMethod {
                        name: Some("googlePay".to_string()),
                        ..base
                    }
                }
                domain_types::payment_method_data::WalletData::ApplePay(_)
                | domain_types::payment_method_data::WalletData::ApplePayRedirect(_) => {
                    NsurePaymentMethod {
                        name: Some("applePay".to_string()),
                        ..base
                    }
                }
                domain_types::payment_method_data::WalletData::AliPayQr(_)
                | domain_types::payment_method_data::WalletData::AliPayRedirect(_) => {
                    NsurePaymentMethod {
                        name: Some("aliPay".to_string()),
                        ..base
                    }
                }
                _ => NsurePaymentMethod {
                    name: Some("wallet".to_string()),
                    ..base
                },
            }
        }
        Some(PaymentMethodData::BankTransfer(_)) | Some(PaymentMethodData::BankDebit(_)) => {
            NsurePaymentMethod {
                payment_method_type: NsurePaymentMethodType::BankTransfer,
                ..base
            }
        }
        // Everything prism can carry but nSure has no first-class variant for.
        Some(_) => NsurePaymentMethod {
            name: Some("other".to_string()),
            ..base
        },
        None => NsurePaymentMethod {
            name: Some("unknown".to_string()),
            ..base
        },
    }
}

/// Build nSure's `cart`. The array is required and must be non-empty
/// (`minItems: 1`), so an order with no line items still sends one item
/// representing the whole purchase. `brand`, `quantity`, `itemFulfillment` and
/// `sellingPrice` are required on every variant; `itemClass`/`productType` come
/// from `connector_feature_data` because prism has no notion of nSure's
/// vertical taxonomy.
fn nsure_cart(
    order_details: Option<&Vec<OrderDetailsWithAmount>>,
    total: &NsureAmount,
    feature_data: &NsureFeatureData,
) -> Result<Vec<NsureCartItem>, Error> {
    let items = match order_details.filter(|details| !details.is_empty()) {
        Some(details) => details
            .iter()
            .map(|detail| {
                Ok::<_, Error>(NsureCartItem {
                    item_class: feature_data.item_class.clone(),
                    // `brand` is required; fall back to the product name.
                    brand: detail
                        .brand
                        .clone()
                        .unwrap_or_else(|| detail.product_name.clone()),
                    quantity: detail.quantity,
                    // Unknown shipping requirement is treated as physical:
                    // claiming `digital` for a shipped good would overstate risk.
                    item_fulfillment: if detail.requires_shipping == Some(false) {
                        NsureItemFulfillment::Digital
                    } else {
                        NsureItemFulfillment::Physical
                    },
                    selling_price: NsureAmount {
                        value_in_currency: super::NsureAmountConvertor::convert(
                            detail.amount,
                            total.currency,
                        )?,
                        currency: total.currency,
                    },
                    sku: detail.sku.clone().or_else(|| detail.product_id.clone()),
                    categories: detail.category.as_ref().map(|c| vec![c.clone()]),
                    product_name: Some(detail.product_name.clone()),
                    product_type: feature_data.product_type.clone(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?,
        // No line items: one synthetic item carrying the order total, so the
        // required non-empty cart is satisfied without fabricating detail.
        None => vec![NsureCartItem {
            item_class: feature_data.item_class.clone(),
            brand: feature_data
                .default_brand
                .clone()
                .unwrap_or_else(|| "unspecified".to_string()),
            quantity: 1,
            item_fulfillment: NsureItemFulfillment::Physical,
            selling_price: total.clone(),
            sku: None,
            categories: None,
            product_name: None,
            product_type: feature_data.product_type.clone(),
        }],
    };
    Ok(items)
}

impl<
        T: domain_types::payment_method_data::PaymentMethodDataTypes
            + std::fmt::Debug
            + Sync
            + Send
            + 'static
            + Serialize,
    >
    TryFrom<
        super::NsureRouterData<
            RouterDataV2<PreRiskCheck, FrmFlowData, PreRiskCheckRequest, PreRiskCheckResponse>,
            T,
        >,
    > for NsurePreRiskCheckRequest
{
    type Error = Error;

    fn try_from(
        item: super::NsureRouterData<
            RouterDataV2<PreRiskCheck, FrmFlowData, PreRiskCheckRequest, PreRiskCheckResponse>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let req = &item.router_data.request;

        // `uniqueRequestId` is nSure's idempotency/correlation key. It must be
        // the same value used as the `{transactionId}` path segment so the
        // evaluation and any later status update refer to one transaction.
        let unique_request_id = req.merchant_transaction_id.clone().ok_or_else(|| {
            error_stack::report!(errors::IntegrationError::MissingRequiredField {
                field_name: "merchant_transaction_id",
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "nSure.ai needs merchant_transaction_id: it is both the \
                         POST /transactions/{transactionId} path segment and the \
                         metadata.uniqueRequestId in the body"
                            .to_owned(),
                    ),
                    suggested_action: Some(
                        "Set merchant_transaction_id on the FRM Pre Risk Check request".to_owned(),
                    ),
                    doc_url: Some(NSURE_DOC_URL.to_owned()),
                },
            })
        })?;

        let currency = req.amount.currency;
        let feature_data = NsureFeatureData::parse(req.connector_feature_data.as_ref());

        // sessionInfo carries the SDK device id plus the browser signals prism
        // already has. Omitted entirely when none of the three are available.
        let browser = req.browser_info.as_ref();
        let end_user_ip = browser.and_then(|info| info.ip_address.map(|ip| ip.to_string()));
        let user_agent = browser.and_then(|info| info.user_agent.clone());
        // nSure requires userAgent and endUserIp on any sessionInfo it is given,
        // so a partial session block is omitted rather than sent incomplete. The
        // SDK deviceId rides along when present.
        let session_info = match (user_agent, end_user_ip) {
            (Some(user_agent), Some(end_user_ip)) => Some(NsureSessionInfo {
                device_id: feature_data.device_id.clone(),
                user_agent,
                end_user_ip,
                language: browser.and_then(|info| info.language.clone()),
                country: req
                    .address
                    .as_ref()
                    .and_then(|address| address.get_payment_billing())
                    .and_then(|billing| billing.address.as_ref())
                    .and_then(|details| details.country.map(|c| c.to_string())),
            }),
            _ => None,
        };

        let paid_amount = NsureAmount {
            value_in_currency: super::NsureAmountConvertor::convert(req.amount.amount, currency)?,
            currency,
        };
        let cart = nsure_cart(req.order_details.as_ref(), &paid_amount, &feature_data)?;

        let billing_info = nsure_billing_info(
            req.address
                .as_ref()
                .and_then(|address| address.get_payment_billing()),
        );

        let payment_method = nsure_payment_method(
            req.payment_method.as_ref(),
            billing_info,
            feature_data.payment_processor.clone(),
            req.merchant_details
                .as_ref()
                .and_then(|details| details.merchant_id.clone()),
            feature_data.payment_method_first_successful_tx_timestamp,
        );

        Ok(Self {
            metadata: NsureMetadata {
                unique_request_id,
                timestamp: common_utils::date_time::now_unix_timestamp() as i128 * 1000,
                account_type: feature_data.account_type,
            },
            session_info,
            end_user_info: nsure_end_user_info(req.customer_info.as_ref(), &feature_data),
            mode: NsureMode::PreAuthorization,
            transaction_details: NsureTransactionDetails {
                paid_amount,
                payment_method,
                cart,
            },
        })
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Response
// ──────────────────────────────────────────────────────────────────────────

/// nSure decision values, per the `POST /transactions/{transactionId}` 200 schema.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum NsureDecision {
    Approved,
    Rejected,
    SoftApproved,
    Review,
    #[serde(rename = "Not Reviewed")]
    NotReviewed,
    #[serde(other)]
    Unknown,
}

impl From<&NsureDecision> for FrmDecision {
    fn from(value: &NsureDecision) -> Self {
        match value {
            NsureDecision::Approved => Self::Approve,
            NsureDecision::Rejected => Self::Reject,
            // A soft approval is still an approval — nSure accepts liability on
            // it, so it must not be downgraded to a manual-review hold.
            NsureDecision::SoftApproved => Self::Approve,
            NsureDecision::Review => Self::Review,
            // "Not Reviewed" means nSure returned no opinion (e.g. the segment is
            // not enabled). Route to review rather than silently approving.
            NsureDecision::NotReviewed => Self::Review,
            // Any value nSure adds later: fail safe to review, never to approve.
            NsureDecision::Unknown => Self::Review,
        }
    }
}

/// Pairs the parsed decision with the verbatim body it was parsed from, so the
/// audit trail keeps fields this connector does not model.
#[derive(Debug, Clone)]
pub struct NsureResponseWithRaw<T> {
    pub parsed_response: T,
    pub raw_response: serde_json::Value,
}

impl<'de, T: serde::de::DeserializeOwned> Deserialize<'de> for NsureResponseWithRaw<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw_response = serde_json::Value::deserialize(deserializer)?;
        let parsed_response = T::deserialize(&raw_response).map_err(serde::de::Error::custom)?;
        Ok(Self {
            parsed_response,
            raw_response,
        })
    }
}

impl<T: Serialize> Serialize for NsureResponseWithRaw<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.parsed_response.serialize(serializer)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NsureOrderDecision {
    #[serde(default)]
    pub decision: Option<NsureDecision>,
    #[serde(default)]
    pub segment_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NsurePreRiskCheckResponse(pub NsureResponseWithRaw<NsureOrderDecision>);

impl TryFrom<ResponseRouterData<NsurePreRiskCheckResponse, Self>>
    for RouterDataV2<PreRiskCheck, FrmFlowData, PreRiskCheckRequest, PreRiskCheckResponse>
{
    type Error = ResponseError;

    fn try_from(
        item: ResponseRouterData<NsurePreRiskCheckResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Serialise the captured raw JSON, not the typed struct, so nothing
        // nSure sent is dropped from the audit trail. `None` on failure
        // degrades the trail rather than failing the flow.
        let raw_connector_response = serde_json::to_string(&item.response.0.raw_response)
            .ok()
            .map(Secret::new);
        let parsed = &item.response.0.parsed_response;

        Ok(Self {
            response: Ok(PreRiskCheckResponse {
                frm_decision: parsed.decision.as_ref().map(FrmDecision::from),
                // nSure's pre-auth decision response carries no numeric score.
                risk_score: None,
                // No reason field either; the segment that produced the decision
                // is the only explanatory value returned.
                reason: parsed
                    .segment_id
                    .as_ref()
                    .map(|segment| format!("segmentId: {segment}")),
                // nSure does not mint its own id — the transaction is keyed by
                // the merchant's transactionId, which is what we sent.
                frm_transaction_id: item.router_data.request.merchant_transaction_id.clone(),
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

// ──────────────────────────────────────────────────────────────────────────
// Lifecycle notifications — PUT /transactions/{transactionId}/status
//
// nSure's pre-auth flow is two calls, not one: the risk evaluation above, and
// this status callback afterwards. Their docs are explicit that the transition
// to `fundsCaptured` is "the formal handshake for the nSure.ai liability
// shift" — without it nSure never learns the payment's outcome and no
// chargeback liability transfers, which is the commercial point of the
// integration.
// ──────────────────────────────────────────────────────────────────────────

/// `status` values accepted by `PUT /transactions/{id}/status`.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum NsureTransactionStatus {
    /// Merchant followed a `Rejected` recommendation and stopped the payment.
    Rejected,
    /// Processor declined the authorization.
    ProcessorAuthorizationFailure,
    /// Capture attempt failed.
    FundsCaptureFailure,
    /// Capture succeeded — this is the value that shifts liability.
    FundsCaptured,
    /// Funds returned to the buyer.
    Refunded,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NsureStatusUpdateRequest {
    pub status: NsureTransactionStatus,
    /// nSure asks for the processor's raw response on the failure and capture
    /// transitions; it is the payload they use to reconcile the authorization.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_payment_processor_response: Option<serde_json::Value>,
}

impl NsureTransactionStatus {
    /// Map the payment outcome onto nSure's vocabulary.
    ///
    /// An FRM `Reject` that the merchant honoured is `rejected`; anything that
    /// reached a successful capture is `fundsCaptured` regardless of the
    /// original recommendation — sending `fundsCaptured` after a `Rejected`
    /// verdict is nSure's documented "merchant override", which moves liability
    /// back to the merchant but must still be reported.
    fn from_attempt_status(
        status: Option<AttemptStatus>,
        frm_decision: Option<FrmDecision>,
    ) -> Option<Self> {
        match status {
            Some(
                AttemptStatus::Charged
                | AttemptStatus::PartialCharged
                | AttemptStatus::PartialChargedAndChargeable,
            ) => Some(Self::FundsCaptured),
            Some(AttemptStatus::AuthorizationFailed) => Some(Self::ProcessorAuthorizationFailure),
            Some(AttemptStatus::CaptureFailed) => Some(Self::FundsCaptureFailure),
            Some(AttemptStatus::Failure) => Some(Self::ProcessorAuthorizationFailure),
            Some(AttemptStatus::Voided | AttemptStatus::VoidedPostCapture) => Some(Self::Rejected),
            Some(AttemptStatus::AutoRefunded) => Some(Self::Refunded),
            // No payment outcome to report: fall back to the FRM decision, so a
            // transaction the merchant declined on nSure's advice is still
            // closed out on their side.
            _ => match frm_decision {
                Some(FrmDecision::Reject) => Some(Self::Rejected),
                _ => None,
            },
        }
    }
}

impl<
        T: domain_types::payment_method_data::PaymentMethodDataTypes
            + std::fmt::Debug
            + Sync
            + Send
            + 'static
            + Serialize,
    >
    TryFrom<
        super::NsureRouterData<
            RouterDataV2<
                domain_types::connector_flow::FrmPaymentOutcome,
                FrmFlowData,
                FrmPaymentOutcomeRequest,
                FrmPaymentOutcomeResponse,
            >,
            T,
        >,
    > for NsureStatusUpdateRequest
{
    type Error = Error;

    fn try_from(
        item: super::NsureRouterData<
            RouterDataV2<
                domain_types::connector_flow::FrmPaymentOutcome,
                FrmFlowData,
                FrmPaymentOutcomeRequest,
                FrmPaymentOutcomeResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let req = &item.router_data.request;
        let status =
            NsureTransactionStatus::from_attempt_status(req.payment_status, req.frm_decision)
                .ok_or_else(|| {
                    error_stack::report!(errors::IntegrationError::MissingRequiredField {
                        field_name: "payment_status",
                        context: errors::IntegrationErrorContext {
                            additional_context: Some(
                                "nSure needs a payment outcome to close out the transaction; \
                         no nSure status maps to the supplied payment_status"
                                    .to_owned(),
                            ),
                            suggested_action: Some(
                                "Send the payment status (charged / authorization_failed / \
                         capture_failed / voided) on the FRM notification"
                                    .to_owned(),
                            ),
                            doc_url: Some(NSURE_DOC_URL.to_owned()),
                        },
                    })
                })?;

        Ok(Self {
            status,
            // The connector-agnostic notification carries the PSP payload (when
            // the caller supplies one) in connector_feature_data.
            raw_payment_processor_response: NsureFeatureData::raw_processor_response(
                req.connector_feature_data.as_ref(),
            ),
        })
    }
}

impl<
        T: domain_types::payment_method_data::PaymentMethodDataTypes
            + std::fmt::Debug
            + Sync
            + Send
            + 'static
            + Serialize,
    >
    TryFrom<
        super::NsureRouterData<
            RouterDataV2<
                domain_types::connector_flow::FrmRefundProcessed,
                FrmFlowData,
                FrmRefundProcessedRequest,
                FrmRefundProcessedResponse,
            >,
            T,
        >,
    > for NsureRefundStatusRequest
{
    type Error = Error;

    fn try_from(
        _item: super::NsureRouterData<
            RouterDataV2<
                domain_types::connector_flow::FrmRefundProcessed,
                FrmFlowData,
                FrmRefundProcessedRequest,
                FrmRefundProcessedResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // A processed refund is unambiguous — nSure has a single value for it.
        Ok(Self {
            status: NsureTransactionStatus::Refunded,
        })
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Disputes — POST /transactions/{transactionId}/disputes
// ──────────────────────────────────────────────────────────────────────────

/// nSure tracks a dispute as opening or closing; there is no richer state.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum NsureDisputeStatus {
    Open,
    Close,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NsureDisputeRequest {
    pub status: NsureDisputeStatus,
}

impl<
        T: domain_types::payment_method_data::PaymentMethodDataTypes
            + std::fmt::Debug
            + Sync
            + Send
            + 'static
            + Serialize,
    >
    TryFrom<
        super::NsureRouterData<
            RouterDataV2<
                domain_types::connector_flow::FrmChargebackReceived,
                FrmFlowData,
                FrmChargebackReceivedRequest,
                FrmChargebackReceivedResponse,
            >,
            T,
        >,
    > for NsureDisputeRequest
{
    type Error = Error;

    fn try_from(
        _item: super::NsureRouterData<
            RouterDataV2<
                domain_types::connector_flow::FrmChargebackReceived,
                FrmFlowData,
                FrmChargebackReceivedRequest,
                FrmChargebackReceivedResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Hyperswitch notifies on receipt of a chargeback, which is the opening
        // of a dispute. Closing is reported separately when the dispute resolves.
        Ok(Self {
            status: NsureDisputeStatus::Open,
        })
    }
}

/// nSure answers the notification endpoints with `200` and an empty or minimal
/// body; there is nothing to map beyond the status code.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NsureNotificationBody {
    #[serde(default)]
    pub ok: Option<bool>,
}

// The connector macros generate one templating type per (request, response)
// pair, so each flow needs its own named types even where the wire shape is
// identical. Mirrors Kount's per-flow newtypes.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct NsurePaymentOutcomeResponse(pub NsureNotificationBody);

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct NsureRefundProcessedResponse(pub NsureNotificationBody);

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct NsureChargebackResponse(pub NsureNotificationBody);

/// Refund status update. Same wire shape as [`NsureStatusUpdateRequest`] but a
/// distinct type so the macros can template it separately.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NsureRefundStatusRequest {
    pub status: NsureTransactionStatus,
}

impl TryFrom<ResponseRouterData<NsurePaymentOutcomeResponse, Self>>
    for RouterDataV2<
        domain_types::connector_flow::FrmPaymentOutcome,
        FrmFlowData,
        FrmPaymentOutcomeRequest,
        FrmPaymentOutcomeResponse,
    >
{
    type Error = ResponseError;

    fn try_from(
        item: ResponseRouterData<NsurePaymentOutcomeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(FrmPaymentOutcomeResponse {
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

impl TryFrom<ResponseRouterData<NsureRefundProcessedResponse, Self>>
    for RouterDataV2<
        domain_types::connector_flow::FrmRefundProcessed,
        FrmFlowData,
        FrmRefundProcessedRequest,
        FrmRefundProcessedResponse,
    >
{
    type Error = ResponseError;

    fn try_from(
        item: ResponseRouterData<NsureRefundProcessedResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(FrmRefundProcessedResponse {
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

impl TryFrom<ResponseRouterData<NsureChargebackResponse, Self>>
    for RouterDataV2<
        domain_types::connector_flow::FrmChargebackReceived,
        FrmFlowData,
        FrmChargebackReceivedRequest,
        FrmChargebackReceivedResponse,
    >
{
    type Error = ResponseError;

    fn try_from(
        item: ResponseRouterData<NsureChargebackResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(FrmChargebackReceivedResponse {
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}
