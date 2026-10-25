use crate::types::ResponseRouterData;
use common_enums::{AttemptStatus, RefundStatus};
use common_utils::types::{AmountConvertor, FloatMajorUnit, FloatMajorUnitForConnector, MinorUnit};
use domain_types::{
    connector_flow::{
        Authorize, Capture, PSync, PreAuthenticate, RSync, Refund, RepeatPayment, SetupMandate,
        Void,
    },
    connector_types::{
        BillingDescriptor, L2L3Data, MandateReference, MandateReferenceId, PaymentFlowData,
        PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData, PaymentsPreAuthenticateData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, RepeatPaymentData, ResponseId, SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_address::OrderDetailsWithAmount,
    payment_method_data::{
        ApplePayPaymentData, BankDebitData, GpayTokenizationData, PaymentMethodData,
        PaymentMethodDataTypes, RawCardNumber, WalletData,
    },
    router_data::{
        AdditionalPaymentMethodConnectorResponse, ConnectorResponseData, ConnectorSpecificConfig,
        FlowStatus,
    },
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
    utils::{get_unimplemented_payment_method_error_message, ForeignTryFrom},
};
use grpc_api_types::payments::{Currency, Money};

// Note: Refund and RefundsData are used for the Refund flow implementation
use base64::Engine;
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use serde_json;

// ===== AUTHENTICATION =====

#[derive(Debug, Clone)]
pub struct NmiAuthType {
    pub api_key: Secret<String>,
    pub public_key: Option<Secret<String>>,
}

impl TryFrom<&ConnectorSpecificConfig> for NmiAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Nmi {
                api_key,
                public_key,
                ..
            } => Ok(Self {
                api_key: api_key.to_owned(),
                public_key: public_key.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
            )),
        }
    }
}

// ===== TRANSACTION TYPES =====

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TransactionType {
    Auth,
    Sale,
    Capture,
    Refund,
    Void,
    Validate,
    Credit,
}

// ===== NMI STATUS ENUM =====

/// `condition` as NMI documents it: a closed set, deserialized once at the serde layer.
/// `Unrecognised` catches a value outside the documented set rather than letting the whole
/// query/webhook body fail to deserialize, and neither it nor the documented `unknown`
/// ("gateway could not determine the state") is resolved to a terminal status.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NmiStatus {
    Abandoned,
    // NMI spells this with one `l` on the wire.
    #[serde(rename = "canceled")]
    Cancelled,
    Pendingsettlement,
    Pending,
    Failed,
    Complete,
    InProgress,
    Unknown,
    #[serde(other)]
    Unrecognised,
}

impl From<NmiStatus> for AttemptStatus {
    fn from(item: NmiStatus) -> Self {
        match item {
            NmiStatus::Abandoned => Self::AuthenticationFailed,
            NmiStatus::Cancelled => Self::Voided,
            NmiStatus::Pending => Self::Authorized,
            NmiStatus::Pendingsettlement | NmiStatus::Complete => Self::Charged,
            NmiStatus::InProgress => Self::AuthenticationPending,
            NmiStatus::Failed => Self::Failure,
            // Neither state says the payment ended. Resolving them terminally is what
            // review_themes TH-07 forbids.
            NmiStatus::Unknown | NmiStatus::Unrecognised => Self::Pending,
        }
    }
}

impl From<NmiStatus> for RefundStatus {
    fn from(item: NmiStatus) -> Self {
        match item {
            NmiStatus::Abandoned | NmiStatus::Cancelled | NmiStatus::Failed => Self::Failure,
            NmiStatus::Pending | NmiStatus::InProgress => Self::Pending,
            NmiStatus::Pendingsettlement | NmiStatus::Complete => Self::Success,
            NmiStatus::Unknown | NmiStatus::Unrecognised => Self::Pending,
        }
    }
}

// ===== PAYMENT METHOD DATA =====

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum NmiPaymentMethod<T: PaymentMethodDataTypes> {
    Card(Box<CardData<T>>),
    Ach(Box<AchData>),
    Wallet(NmiWalletData),
}

// ===== GOOGLE PAY DATA =====

/// NMI's wire name for an encrypted (PSP-decryption) Google Pay payload. `payment_token` is a
/// *different* NMI field -- a Collect.js single-use token -- so the encrypted wallet blob must
/// not be sent under it.
#[derive(Debug, Serialize)]
pub struct GooglePayData {
    googlepay_payment_data: Secret<String>,
}

#[derive(Debug, Serialize)]
pub struct GooglePayDecryptedData {
    decrypted_googlepay_data: DecryptedDataIndicator,
    ccnumber: Secret<String>,
    ccexp: Secret<String>,
    cavv: Option<Secret<String>>,
    eci: Option<String>,
}

#[derive(Debug, Serialize)]
pub enum DecryptedDataIndicator {
    #[serde(rename = "1")]
    Decrypted,
}

// ===== APPLE PAY DATA =====

/// PSP-decryption Apple Pay: NMI documents `applepay_payment_data` as the **hex** form of the
/// decoded token, so the base64 string the wallet hands over is re-encoded, never passed through.
#[derive(Debug, Serialize)]
pub struct ApplePayData {
    applepay_payment_data: Secret<String>,
}

/// Hyperswitch-decryption Apple Pay: the cleartext PAN plus the network cryptogram.
#[derive(Debug, Serialize)]
pub struct ApplePayDecryptedFormData {
    decrypted_applepay_data: DecryptedDataIndicator,
    ccnumber: Secret<String>,
    ccexp: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cavv: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    eci: Option<String>,
}

/// G-ThreeDS-04. An encrypted wallet token is the one field whose emptiness is invisible on the
/// wire: `applepay_payment_data=` / `googlepay_payment_data=` is a syntactically valid form body
/// that NMI reads as a sale with no instrument. `is_none()` is not a validity check here.
fn non_blank_wallet_token(
    token: &str,
    wallet_name: &'static str,
) -> Result<String, error_stack::Report<IntegrationError>> {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return Err(error_stack::report!(IntegrationError::InvalidWalletToken {
            wallet_name: wallet_name.to_string(),
            context: IntegrationErrorContext {
                suggested_action: Some(format!(
                    "Send a non-empty encrypted {wallet_name} token; NMI reads an empty \
                     payment-data field as a sale with no payment instrument"
                )),
                doc_url: Some(NMI_DIRECT_POST_DOC_URL.to_string()),
                additional_context: Some(format!(
                    "{wallet_name} encrypted payment token was absent, empty or whitespace-only"
                )),
            },
        }));
    }
    Ok(trimmed.to_string())
}

/// The wallet payloads NMI accepts on both a sale and the `type=validate` vault add. Shared by
/// Authorize and SetupMandate so the two bodies cannot drift: NMI documents
/// `applepay_payment_data` / `googlepay_payment_data` as request fields of
/// `customer_vault=add_customer` as well. `#[serde(untagged)]` nests, so the flattened form body
/// is the payload struct's own fields either way.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum NmiWalletData {
    GooglePay(Box<GooglePayData>),
    GooglePayDecrypt(Box<GooglePayDecryptedData>),
    ApplePay(Box<ApplePayData>),
    ApplePayDecrypt(Box<ApplePayDecryptedFormData>),
}

/// Only the two wallets NMI has wire fields for are built here; every other `WalletData` variant
/// is refused by the caller's own fallback arm. The transaction type is the caller's to choose --
/// Authorize derives it from the capture method, SetupMandate fixes it to `validate`.
fn nmi_wallet_data(
    wallet_data: &WalletData,
) -> Result<NmiWalletData, error_stack::Report<IntegrationError>> {
    match wallet_data {
        WalletData::GooglePay(google_pay_data) => match &google_pay_data.tokenization_data {
            GpayTokenizationData::Decrypted(decrypted_data) => {
                let ccexp =
                    decrypted_data
                        .get_expiry_date_as_mmyy()
                        .change_context(IntegrationError::RequestEncodingFailed {
                        context: IntegrationErrorContext {
                            suggested_action: Some(
                                "Check the decrypted Google Pay token carries a two-digit expiry \
                                 month and a four-digit expiry year"
                                    .to_string(),
                            ),
                            doc_url: Some(NMI_DIRECT_POST_DOC_URL.to_string()),
                            additional_context: Some(
                                "the decrypted Google Pay expiry could not be formatted as the \
                                 MMYY `ccexp` NMI expects"
                                    .to_string(),
                            ),
                        },
                    })?;
                Ok(NmiWalletData::GooglePayDecrypt(Box::new(
                    GooglePayDecryptedData {
                        decrypted_googlepay_data: DecryptedDataIndicator::Decrypted,
                        ccnumber: Secret::new(
                            decrypted_data
                                .application_primary_account_number
                                .get_card_no(),
                        ),
                        ccexp,
                        cavv: decrypted_data.cryptogram.clone(),
                        eci: decrypted_data.eci_indicator.clone(),
                    },
                )))
            }
            GpayTokenizationData::Encrypted(encrypted_data) => {
                Ok(NmiWalletData::GooglePay(Box::new(GooglePayData {
                    googlepay_payment_data: Secret::new(non_blank_wallet_token(
                        &encrypted_data.token,
                        "Google Pay",
                    )?),
                })))
            }
        },
        WalletData::ApplePay(apple_pay_data) => match &apple_pay_data.payment_data {
            ApplePayPaymentData::Decrypted(decrypted_data) => {
                let ccexp =
                    decrypted_data
                        .get_expiry_date_as_mmyy()
                        .change_context(IntegrationError::RequestEncodingFailed {
                        context: IntegrationErrorContext {
                            suggested_action: Some(
                                "Check the decrypted Apple Pay token carries a two-digit expiry \
                                 month and a four-digit expiry year"
                                    .to_string(),
                            ),
                            doc_url: Some(NMI_DIRECT_POST_DOC_URL.to_string()),
                            additional_context: Some(
                                "the decrypted Apple Pay expiry could not be formatted as the \
                                 MMYY `ccexp` NMI expects"
                                    .to_string(),
                            ),
                        },
                    })?;
                Ok(NmiWalletData::ApplePayDecrypt(Box::new(
                    ApplePayDecryptedFormData {
                        decrypted_applepay_data: DecryptedDataIndicator::Decrypted,
                        ccnumber: Secret::new(
                            decrypted_data
                                .application_primary_account_number
                                .get_card_no(),
                        ),
                        ccexp,
                        cavv: Some(
                            decrypted_data
                                .payment_data
                                .online_payment_cryptogram
                                .clone(),
                        ),
                        eci: decrypted_data.payment_data.eci_indicator.clone(),
                    },
                )))
            }
            ApplePayPaymentData::Encrypted(encrypted_token) => {
                // NMI documents `applepay_payment_data` as the hex form of the decoded token,
                // so the wallet's base64 string is re-encoded. The guard runs first: an
                // undecodable token would otherwise reach `hex::encode` as an empty buffer and
                // ship `applepay_payment_data=`.
                let token = non_blank_wallet_token(encrypted_token, "Apple Pay")?;
                let decoded = BASE64_ENGINE.decode(token).map_err(|error| {
                    error_stack::report!(IntegrationError::InvalidWalletToken {
                        wallet_name: "Apple Pay".to_string(),
                        context: IntegrationErrorContext {
                            suggested_action: Some(
                                "Send the Apple Pay payment token as standard base64; NMI \
                                 expects its hex re-encoding"
                                    .to_string(),
                            ),
                            doc_url: Some(NMI_DIRECT_POST_DOC_URL.to_string()),
                            additional_context: Some(format!(
                                "base64 decode of the Apple Pay token failed: {error}"
                            )),
                        },
                    })
                })?;
                Ok(NmiWalletData::ApplePay(Box::new(ApplePayData {
                    applepay_payment_data: Secret::new(hex::encode(decoded)),
                })))
            }
        },
        // Unreachable from either caller -- both match the two wallets NMI has wire fields for
        // before calling -- but a refusal still has to say what it refused (INV-20).
        _ => Err(error_stack::report!(IntegrationError::NotImplemented(
            get_unimplemented_payment_method_error_message("NMI"),
            IntegrationErrorContext {
                suggested_action: Some(
                    "Send Google Pay or Apple Pay; NMI's Direct Post API has wire fields for \
                     no other wallet"
                        .to_string(),
                ),
                doc_url: Some(NMI_DIRECT_POST_DOC_URL.to_string()),
                additional_context: Some(
                    "wallet variant outside GooglePay / ApplePay reached nmi_wallet_data"
                        .to_string(),
                ),
            },
        ))),
    }
}

#[derive(Debug, Serialize)]
pub struct CardData<T: PaymentMethodDataTypes> {
    ccnumber: RawCardNumber<T>,
    ccexp: Secret<String>, // MMYY format
    cvv: Secret<String>,
}

// ACH Payment Type Constant
const ACH_PAYMENT_TYPE: &str = "check";

// ACH Bank Debit Data Structure
#[derive(Debug, Serialize)]
pub struct AchData {
    /// Payment type - must be "check" for ACH transactions
    #[serde(rename = "payment")]
    payment_type: &'static str,
    /// Name on the customer's ACH account
    checkname: Secret<String>,
    /// Customer's bank routing number (exactly 9 digits)
    checkaba: Secret<String>,
    /// Customer's bank account number
    checkaccount: Secret<String>,
    /// Type of ACH account holder (business, personal)
    #[serde(skip_serializing_if = "Option::is_none")]
    account_holder_type: Option<common_enums::BankHolderType>,
    /// Type of ACH account (checking, savings)
    #[serde(skip_serializing_if = "Option::is_none")]
    account_type: Option<common_enums::BankType>,
    /// Standard Entry Class code of the ACH transaction (PPD, WEB, TEL, CCD)
    #[serde(skip_serializing_if = "Option::is_none")]
    sec_code: Option<String>,
}

// ===== MERCHANT DEFINED FIELDS =====

#[derive(Debug, Serialize)]
pub struct NmiMerchantDefinedField {
    #[serde(flatten)]
    inner: std::collections::BTreeMap<String, Secret<String>>,
}

impl NmiMerchantDefinedField {
    pub fn new(metadata: &serde_json::Value) -> Self {
        // Match Hyperswitch: deserialize into a BTreeMap so the merchant defined
        // fields are emitted in key-sorted order (e.g. login_date, new_customer,
        // udf1), independent of the original metadata insertion order.
        let metadata_as_string = metadata.to_string();
        let sorted: std::collections::BTreeMap<String, serde_json::Value> =
            serde_json::from_str(&metadata_as_string).unwrap_or_default();
        let inner = sorted
            .into_iter()
            .enumerate()
            .map(|(index, (hs_key, hs_value))| {
                // Extract string value properly to avoid JSON encoding
                let value_str = match hs_value {
                    serde_json::Value::Bool(boolean) => boolean.to_string(),
                    serde_json::Value::Number(number) => number.to_string(),
                    serde_json::Value::String(string) => string,
                    other => other.to_string(),
                };
                let nmi_key = format!("merchant_defined_field_{}", index + 1);
                let nmi_value = format!("{hs_key}={value_str}");
                (nmi_key, Secret::new(nmi_value))
            })
            .collect();
        Self { inner }
    }
}

/// NMI documents `tax=-1` as "tax exempt", which is a different statement from `tax=0`.
const NMI_TAX_EXEMPT: FloatMajorUnit = FloatMajorUnit(-1.0);
/// "Line items, up to 99 sets" -- spec, Credit Card Sale request parameters.
const NMI_MAX_LINE_ITEMS: usize = 99;
/// Apple Pay hands the token over as standard (padded) base64.
const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
const NMI_DIRECT_POST_DOC_URL: &str = "https://docs.nmi.com/reference/credit-card-sale";
const NMI_THREE_DS_DOC_URL: &str = "https://docs.nmi.com/docs/three-ds-with-gatewayjs";
const NMI_QUERY_DOC_URL: &str = "https://docs.nmi.com/reference/query";
const NMI_REFUND_DOC_URL: &str = "https://docs.nmi.com/recipes/refund-credit-card-transaction";
const NMI_CREDENTIAL_ON_FILE_DOC_URL: &str = "https://docs.nmi.com/docs/credential-on-file";
/// `cardholder_auth` is documented as these two values only. Every other `trans_status` has no
/// NMI spelling, so the field is omitted rather than mapped to the nearest one -- G-ThreeDS-02
/// has already refused those requests, and an unrecognised status must never read as `verified`.
fn nmi_cardholder_auth(trans_status: &common_enums::TransactionStatus) -> Option<&'static str> {
    match trans_status {
        common_enums::TransactionStatus::Success => Some("verified"),
        common_enums::TransactionStatus::NotVerified => Some("attempted"),
        common_enums::TransactionStatus::Failure
        | common_enums::TransactionStatus::VerificationNotPerformed
        | common_enums::TransactionStatus::Rejected
        | common_enums::TransactionStatus::ChallengeRequired
        | common_enums::TransactionStatus::ChallengeRequiredDecoupledAuthentication
        | common_enums::TransactionStatus::InformationOnly
        | common_enums::TransactionStatus::SecurePaymentConfirmationRequired => None,
    }
}

/// The same rendering serde gives the request's own `amount: FloatMajorUnit` field (`10.0`, not
/// `10`). `FloatMajorUnit` carries no `Display`, and the line items are form *strings*, so the
/// float has to be spelled out here rather than serialized as a number.
fn major_unit_string(amount: FloatMajorUnit) -> String {
    format!("{:?}", amount.0)
}

// ===== LEVEL II / LEVEL III DATA =====

/// Order-level Level II/III fields. Sourced from `PaymentFlowData.l2_l3_data` and **not** from
/// `request.order_tax_amount` / `request.shipping_cost`: those are `0` on every request in the
/// global corpus, so reading them would put `tax=0.00&shipping=0.00` on every single payment.
#[derive(Debug, Serialize)]
pub struct NmiL2L3Data {
    #[serde(skip_serializing_if = "Option::is_none")]
    tax: Option<FloatMajorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping: Option<FloatMajorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ponumber: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duty_amount: Option<FloatMajorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    discount_amount: Option<FloatMajorUnit>,
}

impl NmiL2L3Data {
    /// `None` when the merchant sent no L2/L3 data at all, so an unconfigured merchant's body is
    /// byte-identical to what it was before this group existed.
    fn new(l2_l3_data: Option<&L2L3Data>, currency: common_enums::Currency) -> Option<Self> {
        let data = l2_l3_data?;
        let convert = |amount: Option<MinorUnit>| {
            amount.and_then(|a| FloatMajorUnitForConnector.convert(a, currency).ok())
        };
        // NMI documents `tax=-1` as "tax exempt", which is a different statement from `tax=0`.
        let tax = match data.get_tax_status() {
            Some(common_enums::TaxStatus::Exempt) => Some(NMI_TAX_EXEMPT),
            Some(common_enums::TaxStatus::Taxable) | None => convert(data.get_order_tax_amount()),
        };
        let out = Self {
            tax,
            shipping: convert(data.get_shipping_cost()),
            ponumber: data.get_merchant_order_reference_id(),
            duty_amount: convert(data.get_duty_amount()),
            discount_amount: convert(data.get_discount_amount()),
        };
        let empty = out.tax.is_none()
            && out.shipping.is_none()
            && out.ponumber.is_none()
            && out.duty_amount.is_none()
            && out.discount_amount.is_none();
        (!empty).then_some(out)
    }
}

/// `item_<field>_#` line items, indexed from 1 in `order_details` order. The flattened
/// `BTreeMap` is the same shape `NmiMerchantDefinedField` uses for `merchant_defined_field_#`,
/// and gives a stable emission order.
#[derive(Debug, Serialize)]
pub struct NmiLineItems {
    #[serde(flatten)]
    inner: std::collections::BTreeMap<String, Secret<String>>,
}

impl NmiLineItems {
    fn new(
        order_details: Option<&[OrderDetailsWithAmount]>,
        currency: common_enums::Currency,
    ) -> Option<Self> {
        let order_details = order_details?;
        let mut inner = std::collections::BTreeMap::new();
        for (index, item) in order_details.iter().take(NMI_MAX_LINE_ITEMS).enumerate() {
            let n = index + 1;
            // `item_unit_cost_#` and `item_quantity_#` are the two mandatory members. A unit
            // cost that will not convert means the whole item cannot be described, so it is
            // dropped -- loudly, never silently.
            let Ok(unit_cost) = FloatMajorUnitForConnector.convert(item.amount, currency) else {
                tracing::warn!(
                    line_item = n,
                    "nmi: dropping line item -- unit amount could not be converted to the \
                     connector's major-unit format"
                );
                continue;
            };
            let mut put = |key: String, value: String| {
                inner.insert(key, Secret::new(value));
            };
            put(format!("item_unit_cost_{n}"), major_unit_string(unit_cost));
            put(format!("item_quantity_{n}"), item.quantity.to_string());
            if let Some(product_id) = item.product_id.as_ref() {
                put(format!("item_product_code_{n}"), product_id.clone());
            }
            // The spec's line-item description; `product_name` is the fallback because it is the
            // only non-optional human-readable member.
            let description = item
                .description
                .clone()
                .unwrap_or_else(|| item.product_name.clone());
            put(format!("item_description_{n}"), description);
            if let Some(commodity_code) = item.commodity_code.as_ref() {
                put(format!("item_commodity_code_{n}"), commodity_code.clone());
            }
            if let Some(unit_of_measure) = item.unit_of_measure.as_ref() {
                put(format!("item_unit_of_measure_{n}"), unit_of_measure.clone());
            }
            // Read, never derived from amount x quantity (INV-18).
            if let Some(total) = item
                .total_amount
                .and_then(|a| FloatMajorUnitForConnector.convert(a, currency).ok())
            {
                put(format!("item_total_amount_{n}"), major_unit_string(total));
            }
            if let Some(tax) = item
                .total_tax_amount
                .and_then(|a| FloatMajorUnitForConnector.convert(a, currency).ok())
            {
                put(format!("item_tax_amount_{n}"), major_unit_string(tax));
            }
            if let Some(discount) = item
                .unit_discount_amount
                .and_then(|a| FloatMajorUnitForConnector.convert(a, currency).ok())
            {
                put(
                    format!("item_discount_amount_{n}"),
                    major_unit_string(discount),
                );
            }
            if let Some(rate) = item.discount_percentage {
                put(format!("item_discount_rate_{n}"), rate.to_string());
            }
        }
        (!inner.is_empty()).then_some(Self { inner })
    }
}

// ===== STATEMENT DESCRIPTOR =====

/// `descriptor` / `descriptor_phone` statement-descriptor overrides. `descriptor_address` is not
/// emitted: no proto request field carries a descriptor address, so it cannot be user-provided.
#[derive(Debug, Serialize)]
pub struct NmiDescriptor {
    #[serde(skip_serializing_if = "Option::is_none")]
    descriptor: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    descriptor_phone: Option<Secret<String>>,
}

impl NmiDescriptor {
    fn new(billing_descriptor: Option<&BillingDescriptor>) -> Option<Self> {
        let descriptor_data = billing_descriptor?;
        let descriptor = non_blank_secret(descriptor_data.statement_descriptor.as_deref())
            .or_else(|| non_blank_secret(Some(descriptor_data.name.as_ref()?.peek())));
        let descriptor_phone =
            non_blank_secret(descriptor_data.phone.as_ref().map(|p| p.peek().as_str()));
        (descriptor.is_some() || descriptor_phone.is_some()).then_some(Self {
            descriptor,
            descriptor_phone,
        })
    }
}

/// Absent, empty and whitespace-only all mean "not present" (P01); a trimmed value never reaches
/// the wire as `descriptor=`.
fn non_blank_secret(value: Option<&str>) -> Option<Secret<String>> {
    let trimmed = value?.trim();
    (!trimmed.is_empty()).then(|| Secret::new(trimmed.to_string()))
}

/// Same rule for the address getters, which hand back `Option<Secret<String>>` already and do
/// not normalise (P01): `skip_serializing_if = "Option::is_none"` drops `None` but not
/// `Some("")`, so without this `first_name=&last_name=&phone=` reach transact.php.
fn non_blank(value: Option<Secret<String>>) -> Option<Secret<String>> {
    non_blank_secret(value.as_ref().map(|secret| secret.peek().as_str()))
}

// ===== STORED CREDENTIAL MARKERS =====

/// Who initiated the transaction. NMI requires it on a stored-credential transaction.
#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InitiatedBy {
    Customer,
    Merchant,
}

/// `stored` on the credential-establishing transaction, `used` on every subsequent one.
#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StoredCredentialIndicator {
    Stored,
    Used,
}

#[derive(Debug, Serialize)]
pub struct NmiBillingDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    first_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    address1: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    address2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    zip: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    country: Option<common_enums::CountryAlpha2>,
    #[serde(skip_serializing_if = "Option::is_none")]
    phone: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<common_utils::pii::Email>,
}

impl From<&PaymentFlowData> for NmiBillingDetails {
    /// One source for the ten billing names, so the Authorize body and the SetupMandate
    /// `type=validate` body cannot send a different field set for the same card.
    fn from(common_data: &PaymentFlowData) -> Self {
        Self {
            first_name: non_blank(common_data.get_optional_billing_first_name()),
            last_name: non_blank(common_data.get_optional_billing_last_name()),
            address1: non_blank(common_data.get_optional_billing_line1()),
            address2: non_blank(common_data.get_optional_billing_line2()),
            city: non_blank(common_data.get_optional_billing_city()),
            state: non_blank(common_data.get_optional_billing_state()),
            zip: non_blank(common_data.get_optional_billing_zip()),
            country: common_data.get_optional_billing_country(),
            phone: non_blank(common_data.get_optional_billing_phone_number()),
            email: common_data.get_optional_billing_email(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct NmiShippingDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_firstname: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_lastname: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_address1: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_address2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_state: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_zip: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_country: Option<common_enums::CountryAlpha2>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_email: Option<common_utils::pii::Email>,
}

impl From<&PaymentFlowData> for NmiShippingDetails {
    /// The shipping twin of `NmiBillingDetails::from`: one source for the nine shipping names,
    /// so Authorize, SetupMandate and RepeatPayment cannot send a different field set or a
    /// different blank-handling rule for the same address (P01).
    fn from(common_data: &PaymentFlowData) -> Self {
        Self {
            shipping_firstname: non_blank(common_data.get_optional_shipping_first_name()),
            shipping_lastname: non_blank(common_data.get_optional_shipping_last_name()),
            shipping_address1: non_blank(common_data.get_optional_shipping_line1()),
            shipping_address2: non_blank(common_data.get_optional_shipping_line2()),
            shipping_city: non_blank(common_data.get_optional_shipping_city()),
            shipping_state: non_blank(common_data.get_optional_shipping_state()),
            shipping_zip: non_blank(common_data.get_optional_shipping_zip()),
            shipping_country: common_data.get_optional_shipping_country(),
            shipping_email: common_data.get_optional_shipping_email(),
        }
    }
}

// ===== PAYMENT REQUEST =====

#[derive(Debug, Serialize)]
pub struct NmiPaymentsRequest<T: PaymentMethodDataTypes> {
    security_key: Secret<String>,
    #[serde(rename = "type")]
    transaction_type: TransactionType,
    amount: FloatMajorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    currency: Option<common_enums::Currency>,
    orderid: String,
    #[serde(flatten)]
    #[serde(skip_serializing_if = "Option::is_none")]
    payment_method: Option<NmiPaymentMethod<T>>,
    #[serde(flatten)]
    #[serde(skip_serializing_if = "Option::is_none")]
    merchant_defined_field: Option<NmiMerchantDefinedField>,
    #[serde(skip_serializing_if = "Option::is_none")]
    customer_vault: Option<CustomerAction>,
    #[serde(flatten)]
    #[serde(skip_serializing_if = "Option::is_none")]
    billing_details: Option<NmiBillingDetails>,
    #[serde(flatten)]
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_details: Option<NmiShippingDetails>,
    // Fields for 3DS completion (when redirect_response is present)
    #[serde(skip_serializing_if = "Option::is_none")]
    customer_vault_id: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<common_utils::pii::Email>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cardholder_auth: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cavv: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    xid: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    eci: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cvv: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    three_ds_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    directory_server_id: Option<Secret<String>>,
    // Level II/III order and line-item data
    #[serde(flatten)]
    #[serde(skip_serializing_if = "Option::is_none")]
    l2_l3_data: Option<NmiL2L3Data>,
    #[serde(flatten)]
    #[serde(skip_serializing_if = "Option::is_none")]
    line_items: Option<NmiLineItems>,
    // Statement descriptor overrides
    #[serde(flatten)]
    #[serde(skip_serializing_if = "Option::is_none")]
    descriptor: Option<NmiDescriptor>,
    // Stored-credential markers. These and `customer_vault` are two wire signals of one fact,
    // so they read the same predicate.
    #[serde(skip_serializing_if = "Option::is_none")]
    initiated_by: Option<InitiatedBy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stored_credential_indicator: Option<StoredCredentialIndicator>,
}

// Implementation for NmiRouterData wrapper (needed by macros)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::NmiRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NmiPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::NmiRouterData<
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
        let auth = NmiAuthType::try_from(&router_data.connector_config)?;

        Self::try_from(&NmiAuthorizeRouterData {
            router_data: router_data.clone(),
            auth,
        })
    }
}

/// Wrapper struct to distinguish 3DS completion from regular authorize
struct NmiAuthorizeRouterData<T: PaymentMethodDataTypes> {
    router_data:
        RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    auth: NmiAuthType,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<&NmiAuthorizeRouterData<T>> for NmiPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(data: &NmiAuthorizeRouterData<T>) -> Result<Self, Self::Error> {
        let router_data = &data.router_data;
        let auth = &data.auth;

        // G-ThreeDS-01 / G-ThreeDS-02. Both run before any field is read, so they apply to the
        // 3DS-completion branch and the regular branch alike.
        //
        // NMI publishes no server-to-server 3DS endpoint: steps 1-3 of its ThreeDS sequence are
        // browser Gateway.js interactions and only the charging call is on the wire. A THREE_DS
        // authorize with no authentication result therefore cannot be honoured, and must not
        // quietly become an unauthenticated `type=sale` that drops the liability shift.
        if router_data.resource_common_data.auth_type == common_enums::AuthenticationType::ThreeDs
            && router_data.request.authentication_data.is_none()
            && router_data.request.redirect_response.is_none()
        {
            return Err(error_stack::report!(IntegrationError::NotSupported {
                message: "nmi: 3DS authentication must be performed by Gateway.js before \
                          Authorize; pass authentication_data, or drive \
                          PaymentMethodAuthenticationService/PreAuthenticate first"
                    .to_string(),
                connector: "NMI",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Call PaymentMethodAuthenticationService/PreAuthenticate and complete the \
                         Gateway.js ThreeDSecure challenge, then send the result as \
                         authentication_data on Authorize"
                            .to_string(),
                    ),
                    doc_url: Some(NMI_THREE_DS_DOC_URL.to_string()),
                    additional_context: Some(
                        "auth_type = THREE_DS with neither authentication_data nor \
                         redirect_response present"
                            .to_string(),
                    ),
                },
            }));
        }

        // `cardholder_auth` is documented as `verified | attempted` only. N/U/R are the
        // Gateway.js `failure` codes, not transact.php inputs, so an N/U/R authentication must
        // not be submitted as an authenticated transaction.
        if let Some(trans_status) = router_data
            .request
            .authentication_data
            .as_ref()
            .and_then(|authentication| authentication.trans_status.as_ref())
        {
            if !matches!(
                trans_status,
                common_enums::TransactionStatus::Success
                    | common_enums::TransactionStatus::NotVerified
            ) {
                return Err(error_stack::report!(IntegrationError::NotSupported {
                    message: format!(
                        "nmi: cardholder_auth accepts only verified|attempted; trans_status \
                         {trans_status:?} has no documented NMI spelling"
                    ),
                    connector: "NMI",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Only submit an authentication whose trans_status is Y (verified) or \
                             A (attempted); anything else must not be charged as authenticated"
                                .to_string(),
                        ),
                        doc_url: Some(NMI_THREE_DS_DOC_URL.to_string()),
                        additional_context: Some(format!(
                            "authentication_data.trans_status = {trans_status:?}"
                        )),
                    },
                }));
            }
        }

        if router_data.request.redirect_response.is_some() {
            // 3DS completion flow
            let redirect_response =
                router_data
                    .request
                    .redirect_response
                    .as_ref()
                    .ok_or_else(|| {
                        error_stack::report!(IntegrationError::MissingRequiredField {
                            field_name: "redirect_response",
                            context: Default::default(),
                        })
                    })?;

            let payload_data = redirect_response.payload.clone().ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                    field_name: "redirect_response.payload",
                    context: Default::default(),
                })
            })?;

            let three_ds_data: NmiRedirectResponseData = serde_json::from_value(
                payload_data.expose(),
            )
            .change_context(IntegrationError::MissingRequiredField {
                field_name: "three_ds_data",
                context: Default::default(),
            })?;

            let cvv = match &router_data.request.payment_method_data {
                PaymentMethodData::Card(card_data) => Some(card_data.card_cvc.clone()),
                _ => None,
            };

            let converter = FloatMajorUnitForConnector;
            let amount = converter
                .convert(
                    router_data.request.amount.amount,
                    router_data.request.currency,
                )
                .change_context(IntegrationError::RequestEncodingFailed {
                    context: Default::default(),
                })?;

            let transaction_type = if router_data.request.is_auto_capture() {
                TransactionType::Sale
            } else {
                TransactionType::Auth
            };

            Ok(Self {
                security_key: auth.api_key.clone(),
                transaction_type,
                amount,
                // The amount three lines up was scaled with this currency; omitting the field
                // lets NMI settle it in the merchant account default instead (TH-03).
                currency: Some(router_data.request.currency),
                orderid: three_ds_data.order_id.ok_or_else(|| {
                    error_stack::report!(IntegrationError::MissingRequiredField {
                        field_name: "order_id",
                        context: Default::default(),
                    })
                })?,
                payment_method: None,
                merchant_defined_field: None,
                customer_vault: None,
                billing_details: None,
                shipping_details: None,
                customer_vault_id: Some(three_ds_data.customer_vault_id),
                email: router_data.request.email.clone(),
                cardholder_auth: three_ds_data.card_holder_auth,
                cavv: three_ds_data.cavv,
                xid: three_ds_data.xid,
                eci: three_ds_data.eci,
                cvv,
                three_ds_version: three_ds_data.three_ds_version,
                directory_server_id: three_ds_data.directory_server_id,
                // The Gateway.js return re-enters Authorize with only the 3DS payload; the
                // order-level groups belong to the original request and are not re-sent.
                l2_l3_data: None,
                line_items: None,
                descriptor: None,
                initiated_by: None,
                stored_credential_indicator: None,
            })
        } else {
            // Regular authorization flow
            // NMI's `type` *is* the capture decision, so it is derived once from the request's
            // capture intent and every payment method arm reads the same value: a manual-capture
            // intent must never be auto-captured because one arm omitted the flag (TH-15).
            let intent_type = if router_data.request.is_auto_capture() {
                TransactionType::Sale
            } else {
                TransactionType::Auth
            };
            let (payment_method, transaction_type) = match &router_data.request.payment_method_data
            {
                PaymentMethodData::BankDebit(bank_debit_data) => {
                    let ach_data = create_ach_data(bank_debit_data, router_data)?;
                    // ACH is the one exception, and it is a wire fact rather than a choice:
                    // Direct Post has no authorize/capture split for `checkaba`, so `sale` is
                    // the only `type` NMI accepts for a bank debit.
                    (
                        NmiPaymentMethod::Ach(Box::new(ach_data)),
                        TransactionType::Sale,
                    )
                }
                PaymentMethodData::Wallet(
                    wallet_data @ (WalletData::GooglePay(_) | WalletData::ApplePay(_)),
                ) => (
                    NmiPaymentMethod::Wallet(nmi_wallet_data(wallet_data)?),
                    intent_type,
                ),
                _ => (
                    NmiPaymentMethod::try_from(&router_data.request.payment_method_data)?,
                    intent_type,
                ),
            };

            let converter = FloatMajorUnitForConnector;
            let amount = converter
                .convert(
                    router_data.request.amount.amount,
                    router_data.request.currency,
                )
                .change_context(IntegrationError::RequestEncodingFailed {
                    context: Default::default(),
                })?;

            // 3DS passthrough: a caller that authenticated out of band (its own 3DS server, or
            // the Gateway.js challenge driven by its own front end) sends the result as proto
            // AuthenticationData rather than as a redirect payload.
            let authentication_data = router_data.request.authentication_data.as_ref();
            let l2_l3_data = router_data.resource_common_data.l2_l3_data.as_deref();
            // Line items prefer the L2/L3 carrier and fall back to the flow's own order details.
            let line_items = l2_l3_data
                .and_then(|data| data.get_order_details())
                .or_else(|| router_data.resource_common_data.order_details.clone());
            // `customer_vault`, `initiated_by` and `stored_credential_indicator` are three wire
            // signals of one fact, so they read one predicate (P11).
            let is_credential_on_file = router_data.request.is_mandate_payment();

            Ok(Self {
                security_key: auth.api_key.clone(),
                transaction_type,
                amount,
                currency: Some(router_data.request.currency),
                orderid: router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
                payment_method: Some(payment_method),
                merchant_defined_field: router_data
                    .request
                    .metadata
                    .as_ref()
                    .map(|m| NmiMerchantDefinedField::new(m.peek())),
                customer_vault: is_credential_on_file.then_some(CustomerAction::AddCustomer),
                billing_details: Some(NmiBillingDetails::from(&router_data.resource_common_data)),
                shipping_details: Some(NmiShippingDetails::from(&router_data.resource_common_data)),
                customer_vault_id: None,
                email: None,
                cardholder_auth: authentication_data
                    .and_then(|authentication| authentication.trans_status.as_ref())
                    .and_then(nmi_cardholder_auth)
                    .map(str::to_string),
                cavv: authentication_data.and_then(|authentication| authentication.cavv.clone()),
                xid: authentication_data
                    .and_then(|authentication| authentication.transaction_id.clone())
                    .map(Secret::new),
                eci: authentication_data.and_then(|authentication| authentication.eci.clone()),
                cvv: None,
                three_ds_version: authentication_data.and_then(|authentication| {
                    authentication
                        .message_version
                        .as_ref()
                        .map(|version| version.to_string())
                }),
                directory_server_id: authentication_data
                    .and_then(|authentication| authentication.ds_trans_id.clone())
                    .map(Secret::new),
                l2_l3_data: NmiL2L3Data::new(l2_l3_data, router_data.request.currency),
                line_items: NmiLineItems::new(line_items.as_deref(), router_data.request.currency),
                descriptor: NmiDescriptor::new(router_data.request.billing_descriptor.as_ref()),
                initiated_by: is_credential_on_file.then_some(InitiatedBy::Customer),
                stored_credential_indicator: is_credential_on_file
                    .then_some(StoredCredentialIndicator::Stored),
            })
        }
    }
}

// ===== PAYMENT METHOD TRANSFORMATION =====

impl<T: PaymentMethodDataTypes> TryFrom<&PaymentMethodData<T>> for NmiPaymentMethod<T> {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(pm_data: &PaymentMethodData<T>) -> Result<Self, Self::Error> {
        match pm_data {
            PaymentMethodData::Card(card_data) => {
                // Extract expiry date in MMYY format using framework utility
                let ccexp =
                    card_data.get_card_expiry_month_year_2_digit_with_delimiter("".to_string())?;

                let card = CardData {
                    ccnumber: card_data.card_number.clone(),
                    ccexp,
                    cvv: card_data.card_cvc.clone(),
                };
                Ok(Self::Card(Box::new(card)))
            }
            PaymentMethodData::BankDebit(
                BankDebitData::SepaBankDebit { .. }
                | BankDebitData::BecsBankDebit { .. }
                | BankDebitData::BacsBankDebit { .. },
            ) => Err(error_stack::report!(IntegrationError::NotSupported {
                message: "Bank Debit type not supported for NMI".to_string(),
                connector: "NMI",
                context: Default::default(),
            })),
            _ => Err(error_stack::report!(IntegrationError::NotImplemented(
                "Payment method not supported".to_string(),
                Default::default()
            ))),
        }
    }
}

/// Helper function to create ACH data from BankDebitData with access to router data for billing name fallback
fn create_ach_data<T: PaymentMethodDataTypes>(
    bank_debit_data: &BankDebitData,
    router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
) -> Result<AchData, error_stack::Report<IntegrationError>> {
    match bank_debit_data {
        BankDebitData::AchBankDebit {
            account_number,
            routing_number,
            bank_account_holder_name,
            bank_holder_type,
            bank_type,
            ..
        } => {
            // Get account holder name: use bank_account_holder_name or fall back to billing name
            let checkname = bank_account_holder_name
                .clone()
                .or_else(|| {
                    router_data
                        .resource_common_data
                        .get_billing_full_name()
                        .ok()
                })
                .ok_or_else(|| {
                    error_stack::report!(IntegrationError::MissingRequiredField {
                        field_name: "bank_account_holder_name",
                        context: Default::default(),
                    })
                })?;

            let ach_data = AchData {
                payment_type: ACH_PAYMENT_TYPE,
                checkname,
                checkaba: routing_number.clone(),
                checkaccount: account_number.clone(),
                account_holder_type: *bank_holder_type,
                account_type: *bank_type,
                sec_code: None, // Can be set if needed: PPD, WEB, TEL, CCD
            };
            Ok(ach_data)
        }
        _ => Err(error_stack::report!(IntegrationError::NotSupported {
            message: "Only ACH Bank Debit is supported for NMI".to_string(),
            connector: "NMI",
            context: Default::default(),
        })),
    }
}

// ===== PAYMENT RESPONSE =====

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct StandardResponse {
    pub response: Response,
    pub responsetext: String,
    pub authcode: Option<String>,
    pub transactionid: String,
    pub avsresponse: Option<String>,
    pub cvvresponse: Option<String>,
    pub orderid: String,
    pub response_code: String,
    pub customer_vault_id: Option<Secret<String>>,
}

// Type alias for consistency with nmi.rs
pub type NmiPaymentsResponse = StandardResponse;

impl StandardResponse {
    /// Capture and Void send no `orderid`, so NMI echoes `orderid=`. A missing identifier is
    /// `None`, not `Some("")` -- otherwise two flows of the same payment surface different
    /// reference forms (TH-10/TH-16).
    pub fn connector_reference_id(&self) -> Option<String> {
        let trimmed = self.orderid.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    }

    /// AVS / CVV / auth-code results, which are otherwise deserialized and dropped. The
    /// emptiness guard lives here rather than at each call site, and `None` comes back when the
    /// gateway reported none of the three (the spec's own Capture and Void bodies carry
    /// `avsresponse=&cvvresponse=`).
    pub fn payment_checks(&self) -> Option<ConnectorResponseData> {
        let avs_result_code = non_blank_str(self.avsresponse.as_deref());
        let cvv_result_code = non_blank_str(self.cvvresponse.as_deref());
        let auth_code = non_blank_str(self.authcode.as_deref());

        let mut payment_checks = serde_json::Map::new();
        if let Some(code) = avs_result_code {
            payment_checks.insert("avs_result_code".to_string(), serde_json::json!(code));
        }
        if let Some(code) = cvv_result_code {
            payment_checks.insert("cvv_result_code".to_string(), serde_json::json!(code));
        }
        if payment_checks.is_empty() && auth_code.is_none() {
            return None;
        }
        Some(ConnectorResponseData::with_additional_payment_method_data(
            AdditionalPaymentMethodConnectorResponse::Card {
                authentication_data: None,
                payment_checks: (!payment_checks.is_empty())
                    .then(|| serde_json::Value::Object(payment_checks)),
                card_network: None,
                domestic_network: None,
                auth_code,
                processor_card_network: None,
                card_subtype: None,
                card_segment_type: None,
                funding_source: None,
                card_type: None,
                issuer_name: None,
                issuer_country: None,
            },
        ))
    }
}

pub(crate) fn non_blank_str(value: Option<&str>) -> Option<String> {
    let trimmed = value?.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// NMI-specific network codes for the error path, derived from the spec's own `response_code`
/// table. Only the families the spec attributes to the **processor or issuer** are reported: the
/// gateway families (300, 4xx) are not evidence of a network decline, and copying `response_code`
/// across unconditionally would report a gateway reject as an issuer decline.
pub fn nmi_network_codes(
    response_code: &str,
    responsetext: &str,
) -> (Option<String>, Option<String>, Option<String>) {
    // A code that is not a number carries no family, so it is reported as nothing rather than
    // defaulted into one.
    let Ok(code) = response_code.trim().parse::<u16>() else {
        return (None, None, None);
    };
    // 200 "declined by processor" through 264 are the processor/issuer decline families.
    if !(200..=264).contains(&code) {
        return (None, None, None);
    }
    // 260-264 are the "declined with further instructions" subfamily: stop all recurring, stop
    // this program, update cardholder data, retry in a few days.
    let advice = (260..=264)
        .contains(&code)
        .then(|| response_code.trim().to_string());
    (
        Some(response_code.trim().to_string()),
        advice,
        non_blank_str(Some(responsetext)),
    )
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<StandardResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<StandardResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;

        // The success payload is the same whichever non-error outcome the gateway reported, so
        // it is built once rather than duplicated across two arms.
        let transaction_response = || PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(response.transactionid.clone()),
            redirection_data: None,
            mandate_reference: response.customer_vault_id.as_ref().map(|vault_id| {
                Box::new(MandateReference {
                    connector_mandate_id: Some(vault_id.clone().expose()),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                    mandate_metadata: None,
                })
            }),
            connector_metadata: None,
            network_txn_id: None,
            network_txn_link_id: None,
            connector_response_reference_id: response.connector_reference_id(),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
            splits: None,
            payment_account_reference: None,
        };

        let (status, payment_response) = match response.response {
            Response::Approved => {
                let status = if item.router_data.request.is_auto_capture() {
                    AttemptStatus::Charged
                } else {
                    AttemptStatus::Authorized
                };
                (status, Ok(transaction_response()))
            }
            // The gateway returned a `response=` value outside the documented {1,2,3}. The
            // attempt's state is unknown, so the previous status is carried through rather than
            // resolved terminally in either direction.
            Response::Unknown => {
                tracing::warn!(
                    response = %response.response_code,
                    "nmi: unrecognised `response` value on Authorize; keeping the previous \
                     attempt status"
                );
                (
                    item.router_data.resource_common_data.status,
                    Ok(transaction_response()),
                )
            }
            Response::Declined | Response::Error => {
                let (network_decline_code, network_advice_code, network_error_message) =
                    nmi_network_codes(&response.response_code, &response.responsetext);
                (
                    AttemptStatus::Failure,
                    Err(domain_types::router_data::ErrorResponse {
                        code: response.response_code.clone(),
                        message: response.responsetext.clone(),
                        reason: Some(response.responsetext.clone()),
                        status_code: item.http_code,
                        attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
                        connector_transaction_id: Some(response.transactionid.clone()),
                        network_decline_code,
                        network_advice_code,
                        network_error_message,
                        typed_connector_response: None,
                        raw_connector_response: None,
                        raw_connector_request: None,
                        typed_connector_request: None,
                    }),
                )
            }
        };

        let connector_response = response.payment_checks();

        Ok(Self {
            response: payment_response,
            resource_common_data: PaymentFlowData {
                status,
                connector_response,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== PAYMENT SYNC (PSYNC) REQUEST =====

#[derive(Debug, Serialize)]
pub struct NmiSyncRequest {
    security_key: Secret<String>,
    /// The NMI **gateway** transaction id, not the merchant reference. `query.php` treats both
    /// `transaction_id` and `order_id` as optional filters, so sending neither (or an empty one)
    /// asks for an unfiltered report over the whole merchant account.
    transaction_id: String,
}

// Implementation for NmiRouterData wrapper (needed by macros)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::NmiRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for NmiSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::NmiRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = NmiAuthType::try_from(&router_data.connector_config)?;

        // G-PSync-01: the query keys on the gateway transaction id. Refuse by name when it does
        // not resolve rather than falling back to the merchant reference: `query.php` with no
        // usable filter answers with a report over the whole merchant account ("Report requested
        // is too large"), which is a timed-out sync instead of a named refusal.
        let transaction_id = match &router_data.request.connector_transaction_id {
            ResponseId::ConnectorTransactionId(transaction_id) => transaction_id.clone(),
            ResponseId::EncodedData(_) | ResponseId::NoResponseId => {
                return Err(error_stack::report!(
                    IntegrationError::MissingRequiredField {
                        field_name: "connector_transaction_id",
                        context: IntegrationErrorContext {
                            suggested_action: Some(
                                "Send the NMI gateway transaction id as connector_transaction_id \
                                 — the `transactionid` returned by the transact.php call that \
                                 created the payment being synced"
                                    .to_string(),
                            ),
                            doc_url: Some(NMI_QUERY_DOC_URL.to_string()),
                            additional_context: Some(
                                "connector_transaction_id did not resolve to a \
                                 ConnectorTransactionId; NMI's query.php needs transaction_id or \
                                 order_id and returns an unfiltered merchant-wide report with \
                                 neither"
                                    .to_string(),
                            ),
                        },
                    }
                ));
            }
        };

        Ok(Self {
            security_key: auth.api_key,
            transaction_id,
        })
    }
}

// ===== PAYMENT SYNC (PSYNC) RESPONSE =====

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename = "nm_response")]
pub struct SyncResponse {
    #[serde(default)]
    pub transaction: Vec<SyncTransactionData>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SyncTransactionData {
    pub transaction_id: String,
    pub order_id: String,
    pub condition: NmiStatus,
    /// The operations recorded against this transaction. A refund is one of these, not a
    /// transaction of its own, which is why RSync reads this list rather than a refund record.
    /// Absent on a transaction nothing has been done to, so it defaults to empty.
    #[serde(default)]
    pub action: Vec<SyncActionData>,
}

/// One `<action>` element of a `query.php` transaction record.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SyncActionData {
    pub action_type: NmiActionType,
    pub amount: FloatMajorUnit,
    /// `1` when the action succeeded. NMI sends this as a character, not a boolean.
    pub success: String,
    pub response_code: String,
    pub response_text: Option<String>,
    /// `YYYYMMDDhhmmss`, UTC. Fixed width, so lexicographic order is chronological order.
    pub date: String,
}

impl TryFrom<ResponseRouterData<SyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<SyncResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;

        // Get the requested transaction_id to find the correct transaction
        let requested_transaction_id = item
            .router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: Default::default(),
            })?;

        // Select the record by the id we asked for, and only that id. For an existing id NMI
        // answers with "exactly one `<transaction>` whose `<transaction_id>` equals the queried
        // id", so a body carrying some other transaction is a report about a different payment,
        // not a later view of this one -- adopting its `condition` would copy a stranger's status
        // onto this attempt.
        let transaction = response
            .transaction
            .iter()
            .find(|txn| txn.transaction_id == requested_transaction_id);

        // Nothing matched: an empty `<nm_response/>`, or a record for another id. The query told
        // us nothing about this attempt, so report it as a failed read rather than a successful
        // sync -- and with `attempt_status: None`, so the attempt keeps the status it already has.
        let Some(transaction) = transaction else {
            return Ok(Self {
                response: Err(domain_types::router_data::ErrorResponse {
                    code: common_utils::consts::NO_ERROR_CODE.to_string(),
                    message: format!(
                        "nmi: query.php returned no transaction with transaction_id \
                         {requested_transaction_id}"
                    ),
                    reason: Some(format!(
                        "NMI has no record of transaction_id {requested_transaction_id} under this \
                         merchant account; the payment status could not be read"
                    )),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: Some(requested_transaction_id),
                    ..Default::default()
                }),
                ..item.router_data
            });
        };

        // `condition` already deserialized into NmiStatus at the serde layer.
        let status = AttemptStatus::from(transaction.condition.clone());
        let transaction_id = transaction.transaction_id.clone();

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(transaction_id),
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
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== CAPTURE REQUEST =====

#[derive(Debug, Serialize)]
pub struct NmiCaptureRequest {
    security_key: Secret<String>,
    #[serde(rename = "type")]
    transaction_type: TransactionType,
    transactionid: String,
    amount: FloatMajorUnit,
    /// `orderid` is optional for a capture, and `connector_request_reference_id` is filled from
    /// `merchant_capture_id.unwrap_or_default()`, so a caller that supplied none leaves it empty.
    /// Sending `orderid=` would make NMI echo an empty reference back (TH-10/TH-16), so the
    /// field is omitted instead.
    #[serde(skip_serializing_if = "Option::is_none")]
    orderid: Option<String>,
}

// Implementation for NmiRouterData wrapper (needed by macros)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::NmiRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for NmiCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::NmiRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = NmiAuthType::try_from(&router_data.connector_config)?;

        // Get the original transaction ID from connector_transaction_id
        let transactionid = router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(IntegrationError::MissingRequiredField {
                field_name: "connector_transaction_id",
                context: Default::default(),
            })?;

        // Convert amount from minor to major units using framework converter
        let converter = FloatMajorUnitForConnector;
        let amount = converter
            .convert(
                router_data.request.amount_to_capture.amount,
                router_data.request.currency,
            )
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        Ok(Self {
            security_key: auth.api_key,
            transaction_type: TransactionType::Capture,
            transactionid,
            amount,
            orderid: non_blank_str(Some(
                &router_data
                    .resource_common_data
                    .connector_request_reference_id,
            )),
        })
    }
}

// ===== CAPTURE RESPONSE =====

impl TryFrom<ResponseRouterData<StandardResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<StandardResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;

        let transaction_response = || PaymentsResponseData::TransactionResponse {
            // A capture does not mint a new id: NMI echoes the captured authorization's own
            // `transactionid` back (spec "Expected result (oracle)").
            resource_id: ResponseId::ConnectorTransactionId(response.transactionid.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            network_txn_link_id: None,
            connector_response_reference_id: response.connector_reference_id(),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
            splits: None,
            payment_account_reference: None,
        };

        let (status, capture_response) = match response.response {
            Response::Approved => (AttemptStatus::Charged, Ok(transaction_response())),
            // The gateway returned a `response=` value outside the documented {1,2,3}. None of
            // the Capture flow's allowed states describes "unrecognised", so the previous status
            // is carried through and PSync resolves it, rather than being resolved terminally.
            Response::Unknown => {
                tracing::warn!(
                    response = %response.response_code,
                    "nmi: unrecognised `response` value on Capture; keeping the previous \
                     attempt status"
                );
                (
                    item.router_data.resource_common_data.status,
                    Ok(transaction_response()),
                )
            }
            // HTTP is always 200, so a decline is only visible in the body: surface it as
            // `Err(ErrorResponse)` carrying the capture flow's own status (INV-11 / TH-06).
            Response::Declined | Response::Error => {
                let (network_decline_code, network_advice_code, network_error_message) =
                    nmi_network_codes(&response.response_code, &response.responsetext);
                (
                    AttemptStatus::CaptureFailed,
                    Err(domain_types::router_data::ErrorResponse {
                        code: response.response_code.clone(),
                        message: response.responsetext.clone(),
                        reason: Some(response.responsetext.clone()),
                        status_code: item.http_code,
                        attempt_status: Some(FlowStatus::Payment(AttemptStatus::CaptureFailed)),
                        connector_transaction_id: Some(response.transactionid.clone()),
                        network_decline_code,
                        network_advice_code,
                        network_error_message,
                        typed_connector_response: None,
                        raw_connector_response: None,
                        raw_connector_request: None,
                        typed_connector_request: None,
                    }),
                )
            }
        };

        let connector_response = response.payment_checks();

        Ok(Self {
            response: capture_response,
            resource_common_data: PaymentFlowData {
                status,
                connector_response,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== REFUND REQUEST =====

#[derive(Debug, Serialize)]
pub struct NmiRefundRequest {
    security_key: Secret<String>,
    #[serde(rename = "type")]
    transaction_type: TransactionType,
    transactionid: String,
    orderid: String,
    amount: FloatMajorUnit, // 0.00 for full refund
    #[serde(skip_serializing_if = "Option::is_none")]
    payment: Option<PaymentType>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PaymentType {
    Creditcard,
    Check,
}

// Implementation for NmiRouterData wrapper (needed by macros)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::NmiRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for NmiRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::NmiRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = NmiAuthType::try_from(&router_data.connector_config)?;

        // Get the original payment transaction ID
        let transactionid = router_data.request.connector_transaction_id.clone();

        // G-Refund-01 (request side): `orderid` is the refund's only identifier on NMI -- the
        // refund echoes the payment's `transactionid` back, so the merchant `orderid` is what
        // distinguishes the refund from its payment and what RSync looks it up by. Falling back
        // to `connector_request_reference_id` would send the *payment's* reference as the
        // refund's, which makes the two indistinguishable on the gateway; refuse by name instead.
        let orderid = router_data
            .resource_common_data
            .refund_id
            .clone()
            .ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                    field_name: "refund_id",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Send the refund's own id as refund_id — NMI carries it as the \
                             `orderid` of the `type=refund` call, and RefundService/Get looks \
                             the refund up by that order id"
                                .to_string(),
                        ),
                        doc_url: Some(NMI_REFUND_DOC_URL.to_string()),
                        additional_context: Some(
                            "refund_id was absent; connector_request_reference_id is the \
                             payment's reference, not the refund's, so it is not substituted"
                                .to_string(),
                        ),
                    },
                })
            })?;

        // Convert amount from minor to major units using framework converter
        let converter = FloatMajorUnitForConnector;
        let amount = converter
            .convert(
                router_data.request.refund_amount.amount,
                router_data.request.currency,
            )
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        Ok(Self {
            security_key: auth.api_key,
            transaction_type: TransactionType::Refund,
            transactionid,
            orderid,
            amount,
            payment: None, // NMI infers payment type from the referenced transaction
        })
    }
}

// ===== REFUND RESPONSE =====

impl TryFrom<ResponseRouterData<StandardResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<StandardResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;

        // G-Refund-01 (response side): every arm that publishes a refund id publishes it the
        // same way and refuses rather than publishing an empty one. `connector_refund_id` is a
        // plain `String`, so an unechoed `orderid` would surface as `""` -- a key
        // RefundService/Get then queries with and no report resolves (TH-10/TH-16).
        let published = |refund_status: RefundStatus| match response.connector_reference_id() {
            Some(connector_refund_id) => (
                refund_status,
                Ok(RefundsResponseData {
                    connector_refund_id,
                    refund_status,
                    status_code: item.http_code,
                    acquirer_reference_number: None,
                }),
            ),
            // The refund may well have been accepted, so this is reported non-terminally and
            // RefundService/Get resolves it -- never as a failed refund.
            None => (
                RefundStatus::Pending,
                Err(domain_types::router_data::ErrorResponse {
                    code: common_utils::consts::NO_ERROR_CODE.to_string(),
                    message: "nmi: the refund was accepted but the response echoed no `orderid`, \
                              so no connector_refund_id could be published"
                        .to_string(),
                    reason: Some(
                        "NMI identifies a refund by the `orderid` sent on the `type=refund` \
                         call; with an empty echo the refund cannot be reconciled or synced"
                            .to_string(),
                    ),
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Refund(RefundStatus::Pending)),
                    connector_transaction_id: Some(response.transactionid.clone()),
                    ..Default::default()
                }),
            ),
        };

        let (status, refund_response) = match response.response {
            Response::Approved => published(RefundStatus::Success),
            // The gateway returned a `response=` value outside the documented {1,2,3}. None of
            // the refund states describes "unrecognised", so the refund keeps the status it
            // already carries and RefundService/Get resolves it, rather than being failed here.
            Response::Unknown => {
                tracing::warn!(
                    response = %response.response_code,
                    "nmi: unrecognised `response` value on Refund; keeping the previous refund \
                     status"
                );
                published(item.router_data.resource_common_data.status)
            }
            // HTTP is always 200, so a rejected refund is only visible in the body: surface it as
            // `Err(ErrorResponse)` carrying the refund flow's own status (INV-11 / TH-06).
            Response::Declined | Response::Error => {
                let (network_decline_code, network_advice_code, network_error_message) =
                    nmi_network_codes(&response.response_code, &response.responsetext);
                (
                    RefundStatus::Failure,
                    Err(domain_types::router_data::ErrorResponse {
                        code: response.response_code.clone(),
                        message: response.responsetext.clone(),
                        reason: Some(response.responsetext.clone()),
                        status_code: item.http_code,
                        attempt_status: Some(FlowStatus::Refund(RefundStatus::Failure)),
                        connector_transaction_id: Some(response.transactionid.clone()),
                        network_decline_code,
                        network_advice_code,
                        network_error_message,
                        typed_connector_response: None,
                        raw_connector_response: None,
                        raw_connector_request: None,
                        typed_connector_request: None,
                    }),
                )
            }
        };

        Ok(Self {
            response: refund_response,
            resource_common_data: RefundFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== REFUND SYNC (RSYNC) REQUEST =====

#[derive(Debug, Serialize)]
pub struct NmiRefundSyncRequest {
    security_key: Secret<String>,
    /// The NMI **gateway** transaction id of the *payment* the refund was issued against. NMI
    /// records a refund as an `<action>` inside that transaction and publishes no refund-side
    /// key `query.php` accepts, so this is the lookup. As on PSync, `transaction_id` and
    /// `order_id` are both optional filters: sending neither asks for an unfiltered report over
    /// the whole merchant account.
    transaction_id: String,
    /// Narrows the returned `<action>` list to refund actions. A documented constant the flow
    /// fixes, never a caller input, so it is always sent.
    action_type: NmiActionType,
}

// Implementation for NmiRouterData wrapper (needed by macros)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::NmiRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for NmiRefundSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::NmiRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = NmiAuthType::try_from(&router_data.connector_config)?;

        // G-RSync-01: the query keys on the gateway transaction id of the refunded payment --
        // NMI carries the refund as an `<action>` on that transaction and publishes no refund
        // id `query.php` accepts. Refuse by name when it is absent rather than falling back to
        // the refund's `order_id`: that lookup matches nothing (it is the *refund's* merchant
        // reference, not a transaction the report is indexed by) and an unfiltered `query.php`
        // answers with a merchant-wide report, which is a timed-out sync instead of a named
        // refusal.
        let transaction_id = router_data.request.connector_transaction_id.clone();
        if transaction_id.is_empty() {
            return Err(error_stack::report!(
                IntegrationError::MissingRequiredField {
                    field_name: "connector_transaction_id",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Send the NMI gateway transaction id of the refunded payment as \
                             connector_transaction_id — the `transactionid` the `type=refund` \
                             call echoed back. NMI records the refund as an `<action>` on that \
                             transaction, so there is no refund-side lookup key"
                                .to_string(),
                        ),
                        doc_url: Some(NMI_QUERY_DOC_URL.to_string()),
                        additional_context: Some(
                            "connector_transaction_id was empty; NMI's query.php needs \
                             transaction_id or order_id and returns an unfiltered merchant-wide \
                             report with neither"
                                .to_string(),
                        ),
                    },
                }
            ));
        }

        Ok(Self {
            security_key: auth.api_key,
            transaction_id,
            action_type: NmiActionType::Refund,
        })
    }
}

// ===== REFUND SYNC (RSYNC) RESPONSE =====
// The same `<nm_response><transaction>` document PSync reads: NMI has no refund-status
// endpoint, so RSync is `query.php` narrowed to refund actions.

impl TryFrom<ResponseRouterData<SyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<SyncResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let request = &item.router_data.request;

        // The refund has no record of its own at NMI: it is an `<action>` on the *payment's*
        // transaction, and an `<action>` carries no refund identifier -- only `action_type`,
        // `amount`, `success`, `response_code` and `date`. So the amount being reconciled is the
        // only discriminator the record offers (UD-07), and among several matches the newest
        // wins: `date` is fixed-width `YYYYMMDDhhmmss`, so string order is chronological order.
        // The transaction is pinned to the id we queried for the same reason PSync pins it -- a
        // record for another id is a report about a different payment, and its `condition` is
        // not this refund's.
        let refund_money = request.refund_money.as_ref();
        let matched = response
            .transaction
            .iter()
            .filter(|txn| txn.transaction_id == request.connector_transaction_id)
            .flat_map(|txn| txn.action.iter().map(move |action| (txn, action)))
            .filter(|(_, action)| {
                matches!(
                    action.action_type,
                    NmiActionType::Refund | NmiActionType::Return
                )
            })
            .filter(|(_, action)| {
                // Compared in minor units: `amount` arrives as a float major unit and two floats
                // that print the same need not be bit-equal.
                refund_money.is_some_and(|money| {
                    FloatMajorUnitForConnector
                        .convert_back(action.amount, money.currency)
                        .is_ok_and(|minor| minor == money.amount)
                })
            })
            .max_by(|(_, a), (_, b)| a.date.cmp(&b.date));

        // Republished from the request: the response carries no refund identifier, and
        // `order_id` on the transaction record is the *payment's* merchant reference.
        let connector_refund_id = request.connector_refund_id.clone();

        let (status, refund_response) = match matched {
            // The refund action is on the record and succeeded, so the enclosing transaction's
            // `condition` is this refund's lifecycle state. It reads `pendingsettlement`
            // immediately after the refund and only `complete` once NMI's batch settles.
            Some((transaction, action)) if action.success == "1" => {
                let status = RefundStatus::from(transaction.condition.clone());
                (
                    status,
                    Ok(RefundsResponseData {
                        connector_refund_id,
                        refund_status: status,
                        status_code: item.http_code,
                        acquirer_reference_number: None,
                    }),
                )
            }
            // NMI marked the refund action unsuccessful. HTTP is always 200, so this is only
            // visible in the body (INV-11), and it is reported with the refund flow's own status
            // and non-terminally: the action can be retried and a later read resolves it.
            Some((_, action)) => (
                RefundStatus::Pending,
                Err(domain_types::router_data::ErrorResponse {
                    code: action.response_code.clone(),
                    message: action
                        .response_text
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                    reason: action.response_text.clone(),
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Refund(RefundStatus::Pending)),
                    connector_transaction_id: Some(request.connector_transaction_id.clone()),
                    ..Default::default()
                }),
            ),
            // No refund action matched: an empty `<nm_response/>`, a record for another id, or a
            // transaction carrying no refund of the amount being reconciled. The read told us
            // nothing about this refund, so it keeps the status it already has and a later poll
            // resolves it. Reporting this as a deserialization failure is the defect B002 names
            // -- a well-formed 200 turned into a transport-level error.
            None => (
                RefundStatus::Pending,
                Ok(RefundsResponseData {
                    connector_refund_id,
                    refund_status: RefundStatus::Pending,
                    status_code: item.http_code,
                    acquirer_reference_number: None,
                }),
            ),
        };

        Ok(Self {
            response: refund_response,
            resource_common_data: RefundFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== VOID REQUEST =====

#[derive(Debug, Serialize)]
pub struct NmiVoidRequest {
    security_key: Secret<String>,
    #[serde(rename = "type")]
    transaction_type: TransactionType,
    transactionid: String,
    /// Documented Conditional and EMV-scoped, over a closed set of six values. A reason the
    /// caller did not give -- or one NMI does not publish -- is omitted rather than substituted,
    /// because the field changes what the gateway records against the transaction (TH-09/TH-10).
    /// The spec's own v5 wording: "When not sending a void_reason, send an empty JSON object."
    #[serde(skip_serializing_if = "Option::is_none")]
    void_reason: Option<VoidReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    payment: Option<PaymentType>,
}

/// The six EMV void reasons `transact.php` documents for `void_reason`. `rename_all` already
/// spells each variant as its wire value, so no per-variant rename is needed.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VoidReason {
    Fraud,
    UserCancel,
    IccRejected,
    IccCardRemoved,
    IccNoConfirmation,
    PosTimeout,
}

// Implementation for NmiRouterData wrapper (needed by macros)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::NmiRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for NmiVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::NmiRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = NmiAuthType::try_from(&router_data.connector_config)?;

        // Get the original payment transaction ID
        let transactionid = router_data.request.connector_transaction_id.clone();

        // G-Void-01: only the six values NMI documents are sent. A cancellation reason outside
        // that set has no citable NMI spelling, so the optional field is omitted -- never
        // defaulted to a different reason, which is what the previous `.unwrap_or(UserCancel)`
        // did on every void, including voids that carried no cancellation reason at all.
        let void_reason = router_data
            .request
            .cancellation_reason
            .as_deref()
            .and_then(|reason| match reason {
                "fraud" => Some(VoidReason::Fraud),
                "user_cancel" => Some(VoidReason::UserCancel),
                "icc_rejected" => Some(VoidReason::IccRejected),
                "icc_card_removed" => Some(VoidReason::IccCardRemoved),
                "icc_no_confirmation" => Some(VoidReason::IccNoConfirmation),
                "pos_timeout" => Some(VoidReason::PosTimeout),
                _ => None,
            });

        Ok(Self {
            security_key: auth.api_key,
            transaction_type: TransactionType::Void,
            transactionid,
            void_reason,
            payment: None, // NMI infers payment type from the referenced transaction
        })
    }
}

// ===== VOID RESPONSE =====

impl TryFrom<ResponseRouterData<StandardResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<StandardResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;

        let transaction_response = || PaymentsResponseData::TransactionResponse {
            // A void does not mint a new id: NMI echoes the voided transaction's own
            // `transactionid` back (spec "Expected result (oracle)" -- "`transactionid`
            // unchanged").
            resource_id: ResponseId::ConnectorTransactionId(response.transactionid.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            network_txn_link_id: None,
            connector_response_reference_id: response.connector_reference_id(),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
            splits: None,
            payment_account_reference: None,
        };

        let (status, void_response) = match response.response {
            // The `type=void` acknowledgement is not settlement. The spec says the transaction's
            // terminal condition *becomes* `canceled`, and the only way to observe that is the
            // `query.php` read, so the ack is `VoidInitiated` and PSync's
            // `NmiStatus::Cancelled` is what reaches the terminal `Voided`.
            Response::Approved => (AttemptStatus::VoidInitiated, Ok(transaction_response())),
            // The gateway returned a `response=` value outside the documented {1,2,3}. None of
            // the Void flow's allowed states describes "unrecognised", so the previous status is
            // carried through and PSync resolves it, rather than stranding in `VoidFailed` a
            // void that may well have succeeded.
            Response::Unknown => {
                tracing::warn!(
                    response = %response.response_code,
                    "nmi: unrecognised `response` value on Void; keeping the previous \
                     attempt status"
                );
                (
                    item.router_data.resource_common_data.status,
                    Ok(transaction_response()),
                )
            }
            // HTTP is always 200, so a rejected void is only visible in the body: surface it as
            // `Err(ErrorResponse)` carrying the void flow's own status (INV-11 / TH-06).
            Response::Declined | Response::Error => {
                let (network_decline_code, network_advice_code, network_error_message) =
                    nmi_network_codes(&response.response_code, &response.responsetext);
                (
                    AttemptStatus::VoidFailed,
                    Err(domain_types::router_data::ErrorResponse {
                        code: response.response_code.clone(),
                        message: response.responsetext.clone(),
                        reason: Some(response.responsetext.clone()),
                        status_code: item.http_code,
                        attempt_status: Some(FlowStatus::Payment(AttemptStatus::VoidFailed)),
                        connector_transaction_id: Some(response.transactionid.clone()),
                        network_decline_code,
                        network_advice_code,
                        network_error_message,
                        typed_connector_response: None,
                        raw_connector_response: None,
                        raw_connector_request: None,
                        typed_connector_request: None,
                    }),
                )
            }
        };

        Ok(Self {
            response: void_response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

pub type NmiVaultResponse = NmiVaultResponseStruct;
pub type NmiPreAuthenticateResponse = NmiVaultResponse;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Response {
    #[serde(alias = "1")]
    Approved,
    #[serde(alias = "2")]
    Declined,
    #[serde(alias = "3")]
    Error,
    /// Any `response=` value outside the documented {1,2,3}. Without it `StandardResponse`
    /// never deserializes and the flow returns a framework deserialization failure instead of a
    /// status; and because the state is genuinely unknown it is never resolved terminally.
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CustomerAction {
    AddCustomer,
    UpdateCustomer,
}

#[derive(Debug, Serialize)]
pub struct NmiVaultRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    security_key: Secret<String>,
    ccnumber: RawCardNumber<T>,
    ccexp: Secret<String>,
    cvv: Secret<String>,
    first_name: Secret<String>,
    last_name: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    address1: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    address2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    zip: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    country: Option<common_enums::CountryAlpha2>,
    customer_vault: CustomerAction,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NmiVaultResponseStruct {
    pub response: Response,
    pub responsetext: String,
    pub customer_vault_id: Option<Secret<String>>,
    pub response_code: String,
    pub transactionid: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum NmiRedirectResponse {
    NmiRedirectResponseData(NmiRedirectResponseData),
    NmiErrorResponseData(NmiErrorResponseData),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NmiErrorResponseData {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NmiRedirectResponseData {
    cavv: Option<Secret<String>>,
    xid: Option<Secret<String>>,
    eci: Option<String>,
    card_holder_auth: Option<String>,
    three_ds_version: Option<String>,
    order_id: Option<String>,
    directory_server_id: Option<Secret<String>>,
    customer_vault_id: Secret<String>,
}

type CardDetails<T> = common_utils::CustomResult<
    (RawCardNumber<T>, Secret<String>, Secret<String>),
    IntegrationError,
>;

fn get_card_details<T: PaymentMethodDataTypes>(
    payment_method_data: Option<&PaymentMethodData<T>>,
) -> CardDetails<T> {
    match payment_method_data {
        Some(PaymentMethodData::Card(ref card_details)) => Ok((
            card_details.card_number.clone(),
            card_details.get_card_expiry_month_year_2_digit_with_delimiter("".to_string())?,
            card_details.card_cvc.clone(),
        )),
        _ => Err(IntegrationError::NotImplemented(
            get_unimplemented_payment_method_error_message("NMI"),
            Default::default(),
        )
        .into()),
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::NmiRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NmiVaultRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::NmiRouterData<
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
        let auth = NmiAuthType::try_from(&router_data.connector_config)?;
        let (ccnumber, ccexp, cvv) =
            get_card_details(router_data.request.payment_method_data.as_ref())?;

        let billing_address = router_data.resource_common_data.get_billing_address()?;

        let first_name = billing_address.get_first_name()?;
        let last_name = billing_address.get_last_name().unwrap_or(first_name);

        Ok(Self {
            security_key: auth.api_key,
            ccnumber,
            ccexp,
            cvv,
            first_name: first_name.clone(),
            last_name: last_name.clone(),
            address1: billing_address.line1.clone(),
            address2: billing_address.line2.clone(),
            city: billing_address.city.clone(),
            state: billing_address.state.clone(),
            zip: billing_address.zip.clone(),
            country: billing_address.country,
            customer_vault: CustomerAction::AddCustomer,
        })
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<NmiVaultResponse, Self>>
    for RouterDataV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<NmiVaultResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;

        let (status, payment_response) = match response.response {
            // The gateway returned a `response=` value outside the documented {1,2,3}. There is
            // no vault id to authenticate against and no evidence the attempt failed, so the
            // previous status is carried through and the caller gets no redirect.
            Response::Unknown => {
                tracing::warn!(
                    response = %response.response_code,
                    "nmi: unrecognised `response` value on the customer-vault add; keeping the \
                     previous attempt status"
                );
                (
                    item.router_data.resource_common_data.status,
                    Ok(PaymentsResponseData::PreAuthenticateResponse {
                        resource_id: None,
                        authentication_data: None,
                        redirection_data: None,
                        connector_response_reference_id: Some(response.transactionid.clone()),
                        status_code: item.http_code,
                    }),
                )
            }
            Response::Approved => {
                let auth_type = NmiAuthType::try_from(&item.router_data.connector_config)
                    .change_context(ConnectorError::ResponseHandlingFailed {
                        context: Default::default(),
                    })?;
                let amount_data = item.router_data.request.amount.amount;
                let currency_data = item.router_data.request.currency.ok_or(
                    ConnectorError::ResponseHandlingFailed {
                        context: Default::default(),
                    },
                )?;
                let customer_vault_id = response.customer_vault_id.clone().ok_or_else(|| {
                    error_stack::report!(ConnectorError::UnexpectedResponseError {
                        context: Default::default(),
                    })
                })?;

                (
                    AttemptStatus::AuthenticationPending,
                    Ok(PaymentsResponseData::PreAuthenticateResponse {
                        resource_id: None,
                        authentication_data: None,
                        redirection_data: Some(Box::new(RedirectForm::Nmi {
                            amount: Money {
                                minor_amount: amount_data.get_amount_as_i64(),
                                currency: Currency::foreign_try_from(currency_data)
                                    .map_err(|_| {
                                        error_stack::report!(
                                            ConnectorError::ResponseHandlingFailed {
                                                context: Default::default(),
                                            }
                                        )
                                    })?
                                    .into(),
                            },
                            public_key: auth_type.public_key.ok_or(
                                ConnectorError::ResponseHandlingFailed {
                                    context: Default::default(),
                                },
                            )?,
                            customer_vault_id: customer_vault_id.peek().to_string(),
                            order_id: item
                                .router_data
                                .resource_common_data
                                .connector_request_reference_id
                                .clone(),
                            continue_redirection_url: item
                                .router_data
                                .request
                                .continue_redirection_url
                                .as_ref()
                                .map(|url| url.to_string())
                                .ok_or_else(|| {
                                    error_stack::report!(ConnectorError::ResponseHandlingFailed {
                                        context: Default::default(),
                                    })
                                })?,
                        })),
                        connector_response_reference_id: Some(response.transactionid.clone()),
                        status_code: item.http_code,
                    }),
                )
            }
            Response::Declined | Response::Error => {
                let (network_decline_code, network_advice_code, network_error_message) =
                    nmi_network_codes(&response.response_code, &response.responsetext);
                (
                    AttemptStatus::Failure,
                    Err(domain_types::router_data::ErrorResponse {
                        code: response.response_code.clone(),
                        message: response.responsetext.clone(),
                        reason: Some(response.responsetext.clone()),
                        status_code: item.http_code,
                        attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
                        connector_transaction_id: Some(response.transactionid.clone()),
                        network_decline_code,
                        network_advice_code,
                        network_error_message,
                        typed_connector_response: None,
                        raw_connector_response: None,
                        raw_connector_request: None,
                        typed_connector_request: None,
                    }),
                )
            }
        };

        Ok(Self {
            response: payment_response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== SETUP MANDATE (SetupRecurring) =====

/// NMI SetupMandate request - adds payment method to Customer Vault for recurring payments
#[derive(Debug, Serialize)]
pub struct NmiSetupMandateRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(rename = "type")]
    transaction_type: TransactionType,
    security_key: Secret<String>,
    orderid: String,
    customer_vault: CustomerAction,
    #[serde(flatten)]
    payment_method: NmiSetupMandatePaymentMethod<T>,
    #[serde(flatten)]
    #[serde(skip_serializing_if = "Option::is_none")]
    billing_details: Option<NmiBillingDetails>,
    #[serde(flatten)]
    shipping_details: NmiShippingDetails,
    // A `type=validate` + `customer_vault=add_customer` call *is* the credential-establishing
    // transaction, so unlike Authorize there is no predicate to read: both are always sent.
    #[serde(skip_serializing_if = "Option::is_none")]
    initiated_by: Option<InitiatedBy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stored_credential_indicator: Option<StoredCredentialIndicator>,
}

/// Payment method for SetupMandate - supports Card and ACH
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum NmiSetupMandatePaymentMethod<T: PaymentMethodDataTypes> {
    Card(NmiSetupMandateCard<T>),
    Ach(NmiSetupMandateAch),
    /// NMI documents `applepay_payment_data` / `googlepay_payment_data` as request fields of
    /// `customer_vault=add_customer`, so a wallet instrument can establish a credential on file.
    Wallet(NmiWalletData),
}

/// Card payment method for SetupMandate
#[derive(Debug, Serialize)]
pub struct NmiSetupMandateCard<T: PaymentMethodDataTypes> {
    ccnumber: RawCardNumber<T>,
    ccexp: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cvv: Option<Secret<String>>,
}

/// ACH payment method for SetupMandate
#[derive(Debug, Serialize)]
pub struct NmiSetupMandateAch {
    payment: &'static str,
    checkname: Secret<String>,
    checkaba: Secret<String>,
    checkaccount: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    account_holder_type: Option<common_enums::BankHolderType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    account_type: Option<common_enums::BankType>,
}

/// The `type=validate` vault add answers with the same `transact.php` envelope as every other
/// flow, so it is the same type -- which is also how it gets `connector_reference_id()` (the
/// validate sample echoes `orderid=` empty) and `payment_checks()` (it returns `avsresponse`,
/// the whole point of a zero-amount verification).
pub type NmiSetupMandateResponse = StandardResponse;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::NmiRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NmiSetupMandateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::NmiRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Hyperswitch parity: NMI SetupMandate (Validate) only supports zero amount.
        if router_data
            .request
            .amount
            .as_ref()
            .map(|money| money.amount)
            .is_some_and(|amount| amount > MinorUnit::zero())
        {
            return Err(IntegrationError::NotSupported {
                message: "Setup Mandate with non zero amount".to_string(),
                connector: "NMI",
                context: Default::default(),
            }
            .into());
        }

        let auth = NmiAuthType::try_from(&router_data.connector_config)?;

        let payment_method = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                let ccexp = card_data.get_expiry_date_as_mmyy()?;
                NmiSetupMandatePaymentMethod::Card(NmiSetupMandateCard {
                    ccnumber: card_data.card_number.clone(),
                    ccexp,
                    cvv: Some(card_data.card_cvc.clone()),
                })
            }
            PaymentMethodData::BankDebit(BankDebitData::AchBankDebit {
                account_number,
                routing_number,
                bank_account_holder_name,
                bank_type,
                bank_holder_type,
                ..
            }) => {
                let checkname = bank_account_holder_name.clone().ok_or_else(|| {
                    IntegrationError::MissingRequiredField {
                        field_name: "bank_account_holder_name",
                        context: Default::default(),
                    }
                })?;
                NmiSetupMandatePaymentMethod::Ach(NmiSetupMandateAch {
                    payment: ACH_PAYMENT_TYPE,
                    checkname,
                    checkaba: routing_number.clone(),
                    checkaccount: account_number.clone(),
                    account_holder_type: *bank_holder_type,
                    account_type: *bank_type,
                })
            }
            PaymentMethodData::Wallet(
                wallet_data @ (WalletData::GooglePay(_) | WalletData::ApplePay(_)),
            ) => NmiSetupMandatePaymentMethod::Wallet(nmi_wallet_data(wallet_data)?),
            // G-SetupMandate-01: every other payment method is refused here, before the
            // request is built and before any HTTP call.
            _ => {
                return Err(error_stack::report!(IntegrationError::NotSupported {
                    message: get_unimplemented_payment_method_error_message("NMI SetupMandate"),
                    connector: "NMI",
                    context: Default::default(),
                }))
            }
        };

        let common_data = &router_data.resource_common_data;

        Ok(Self {
            transaction_type: TransactionType::Validate,
            security_key: auth.api_key,
            orderid: common_data.connector_request_reference_id.clone(),
            customer_vault: CustomerAction::AddCustomer,
            payment_method,
            billing_details: Some(NmiBillingDetails::from(common_data)),
            shipping_details: NmiShippingDetails::from(common_data),
            initiated_by: Some(InitiatedBy::Customer),
            stored_credential_indicator: Some(StoredCredentialIndicator::Stored),
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<NmiSetupMandateResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NmiSetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;

        let (status, payment_response) = match response.response {
            Response::Approved => {
                let connector_mandate_id = response
                    .customer_vault_id
                    .as_ref()
                    .map(|id| id.clone().expose());

                let mandate_reference = connector_mandate_id.clone().map(|id| {
                    Box::new(MandateReference {
                        connector_mandate_id: Some(id),
                        payment_method_id: None,
                        connector_mandate_request_reference_id: None,
                        // NMI requires `initial_transaction_id` on every later MIT, and the
                        // `type=validate` call is the transaction it must cite. The vault id
                        // alone cannot carry it, so it rides in the mandate metadata (UD-06).
                        mandate_metadata: Some(Secret::new(serde_json::json!({
                            "initial_transaction_id": response.transactionid.clone(),
                        }))),
                    })
                });

                (
                    AttemptStatus::Charged,
                    Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(
                            response.transactionid.clone(),
                        ),
                        redirection_data: None,
                        mandate_reference,
                        connector_metadata: None,
                        network_txn_id: None,
                        network_txn_link_id: None,
                        // Hyperswitch parity: NMI maps connector_response_reference_id to the
                        // merchant `orderid` (echoed back), not the connector `transactionid`
                        // (which is already the resource_id / ConnectorTransactionId above).
                        // The validate response echoes `orderid=` empty, so the non-blank guard
                        // decides between a reference and None.
                        connector_response_reference_id: response.connector_reference_id(),
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                        splits: None,
                        payment_account_reference: None,
                    }),
                )
            }
            // P-ThreeDS-09: `network_advice_code` on a declined credential setup is what tells
            // the caller not to re-attempt vaulting this card (260-264 "declined with further
            // instructions"), so the stored-credential flows carry it like every other flow.
            Response::Declined | Response::Error => {
                let (network_decline_code, network_advice_code, network_error_message) =
                    nmi_network_codes(&response.response_code, &response.responsetext);
                (
                    AttemptStatus::Failure,
                    Err(domain_types::router_data::ErrorResponse {
                        code: response.response_code.clone(),
                        message: response.responsetext.clone(),
                        reason: Some(response.responsetext.clone()),
                        status_code: item.http_code,
                        attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
                        connector_transaction_id: Some(response.transactionid.clone()),
                        network_decline_code,
                        network_advice_code,
                        network_error_message,
                        typed_connector_response: None,
                        raw_connector_response: None,
                        raw_connector_request: None,
                        typed_connector_request: None,
                    }),
                )
            }
            // An unrecognised `response` is never resolved terminally, and the mandate
            // reference is withheld: a vault id published off a response UCS could not read
            // would be a mandate handle the connector invented.
            Response::Unknown => {
                tracing::warn!(
                    response = ?response.response,
                    response_code = %response.response_code,
                    "nmi: unrecognised `response` value on SetupMandate; keeping the previous \
                     attempt status and publishing no mandate reference"
                );
                (
                    item.router_data.resource_common_data.status,
                    Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(
                            response.transactionid.clone(),
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
                    }),
                )
            }
        };

        // The zero-amount validate is the one call whose AVS result is the point: the spec's own
        // sample answers `avsresponse=N`, i.e. it is how a merchant learns the address did not
        // match before charging.
        let connector_response = response.payment_checks();

        Ok(Self {
            response: payment_response,
            resource_common_data: PaymentFlowData {
                status,
                connector_response,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== REPEAT PAYMENT (RecurringPaymentService/Charge) =====

#[derive(Debug, Serialize)]
pub struct NmiRepeatPaymentRequest {
    #[serde(rename = "type")]
    transaction_type: TransactionType,
    security_key: Secret<String>,
    amount: FloatMajorUnit,
    currency: common_enums::Currency,
    orderid: String,
    customer_vault_id: Secret<String>,
    #[serde(flatten)]
    #[serde(skip_serializing_if = "Option::is_none")]
    merchant_defined_field: Option<NmiMerchantDefinedField>,
    #[serde(skip_serializing_if = "Option::is_none")]
    first_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    address1: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    address2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    zip: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    country: Option<common_enums::CountryAlpha2>,
    #[serde(skip_serializing_if = "Option::is_none")]
    phone: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<common_utils::pii::Email>,
    #[serde(flatten)]
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_details: Option<NmiShippingDetails>,
    // Stored-credential markers. Unlike Authorize -- where they are conditional on
    // `is_credential_on_file` -- the two labels below are unconditional on an MIT: the charge
    // has to be labelled to the network as merchant-initiated against a stored credential.
    initiated_by: InitiatedBy,
    stored_credential_indicator: StoredCredentialIndicator,
    // Optional, and deliberately so: NMI documents it as required on every transaction after
    // the first, but a mandate vaulted before this connector started writing
    // `mandate_metadata.initial_transaction_id` has no value to cite. Refusing the charge would
    // break every pre-existing NMI mandate on its next renewal, so the field is omitted and the
    // body falls back to the shape that shipped before (run nmi-dd2a12 parity divergence D06).
    #[serde(skip_serializing_if = "Option::is_none")]
    initial_transaction_id: Option<String>,
}

pub type NmiRepeatPaymentResponse = NmiSetupMandateResponse;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::NmiRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NmiRepeatPaymentRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::NmiRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = NmiAuthType::try_from(&router_data.connector_config)?;

        // G-RepeatPayment-01. Both inputs fail closed here, before the request is built and
        // before any HTTP call. The builder never reads `payment_method_data` -- the vault
        // record decides the instrument -- so this one site covers every payment-method arm.
        let MandateReferenceId::ConnectorMandateId(connector_mandate_ids) =
            &router_data.request.mandate_reference
        else {
            return Err(IntegrationError::NotSupported {
                message: "Only ConnectorMandateId is supported for NMI recurring payments"
                    .to_string(),
                connector: "NMI",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Send mandate_reference as ConnectorMandateId carrying the NMI Customer \
                         Vault id -- NMI charges a stored credential by vault record, not by \
                         network transaction id or network token"
                            .to_string(),
                    ),
                    doc_url: Some(NMI_CREDENTIAL_ON_FILE_DOC_URL.to_string()),
                    additional_context: Some(
                        "a NetworkMandateId / NetworkTokenWithNTI reference cannot address an \
                         NMI Customer Vault record"
                            .to_string(),
                    ),
                },
            }
            .into());
        };

        let customer_vault_id = connector_mandate_ids
            .get_connector_mandate_id()
            .ok_or_else(|| IntegrationError::MissingRequiredField {
                field_name: "connector_mandate_id",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Send the NMI Customer Vault id returned by SetupRecurring as \
                         mandate_reference.connector_mandate_id -- it is the vault record \
                         `customer_vault_id` charges"
                            .to_string(),
                    ),
                    doc_url: Some(NMI_CREDENTIAL_ON_FILE_DOC_URL.to_string()),
                    additional_context: Some(
                        "connector_mandate_id was absent from the ConnectorMandateId reference; \
                         NMI has no other handle on a vaulted credential"
                            .to_string(),
                    ),
                },
            })?;

        // NMI documents `initial_transaction_id` as "Required for all subsequent transactions":
        // it cites the credential-establishing transaction so the network can match the MIT to
        // it. SetupMandate writes it into the mandate metadata (UD-06) because the RPC carries
        // no field for it -- but only for credentials vaulted since that change. A mandate
        // created earlier has no `mandate_metadata` at all, and refusing its renewal would
        // break every pre-existing NMI mandate rather than mislabel one charge, so an absent
        // value falls back to the pre-change body (no `initial_transaction_id`) and is logged.
        let initial_transaction_id = connector_mandate_ids.get_mandate_metadata().and_then(|m| {
            non_blank_str(
                m.peek()
                    .get("initial_transaction_id")
                    .and_then(|value| value.as_str()),
            )
        });
        if initial_transaction_id.is_none() {
            tracing::warn!(
                "nmi: mandate_metadata carries no `initial_transaction_id`; sending the \
                 merchant-initiated charge without it. NMI documents it as required on every \
                 transaction after the first, so the network may price this charge as \
                 unreferenced. Re-establish the credential through \
                 PaymentService/SetupRecurring to populate it ({NMI_CREDENTIAL_ON_FILE_DOC_URL})"
            );
        }

        let amount = FloatMajorUnitForConnector
            .convert(
                router_data.request.amount.amount,
                router_data.request.currency,
            )
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        let common_data = &router_data.resource_common_data;

        // NMI's `type` *is* the capture decision: `sale` settles immediately, `auth` leaves the
        // money uncaptured for a later Capture. A manual-capture recurring charge submitted as
        // `sale` would move money the caller did not ask to move (TH-15).
        let transaction_type = if router_data.request.is_auto_capture() {
            TransactionType::Sale
        } else {
            TransactionType::Auth
        };

        Ok(Self {
            transaction_type,
            security_key: auth.api_key,
            amount,
            currency: router_data.request.currency,
            orderid: common_data.connector_request_reference_id.clone(),
            customer_vault_id: Secret::new(customer_vault_id),
            // Mirror the Authorize/SetupMandate flows: NMI expects the billing
            // address and merchant_defined_field_* as flat top-level fields, which
            // the hyperswitch reference also sends on recurring (MIT) charges.
            merchant_defined_field: router_data
                .request
                .metadata
                .as_ref()
                .map(|m| NmiMerchantDefinedField::new(m.peek())),
            first_name: non_blank(common_data.get_optional_billing_first_name()),
            last_name: non_blank(common_data.get_optional_billing_last_name()),
            address1: non_blank(common_data.get_optional_billing_line1()),
            address2: non_blank(common_data.get_optional_billing_line2()),
            city: non_blank(common_data.get_optional_billing_city()),
            state: non_blank(common_data.get_optional_billing_state()),
            zip: non_blank(common_data.get_optional_billing_zip()),
            country: common_data.get_optional_billing_country(),
            phone: non_blank(common_data.get_optional_billing_phone_number()),
            // hyperswitch sends only the address emails on every NMI flow, MIT included
            // (`NmiRouterData::get_billing_details` / `get_shipping_details`). A
            // customer-email fallback here makes UCS emit fields the direct path omits.
            email: common_data.get_optional_billing_email(),
            shipping_details: Some(NmiShippingDetails::from(common_data)),
            initiated_by: InitiatedBy::Merchant,
            stored_credential_indicator: StoredCredentialIndicator::Used,
            initial_transaction_id,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<NmiRepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NmiRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;

        let (status, payment_response) = match response.response {
            Response::Approved => {
                // `type=auth` is reachable on this flow, so the outcome is not always a
                // settlement: reporting `Charged` for an uncaptured MIT would tell the caller
                // the money moved (TH-15).
                let status = if item.router_data.request.is_auto_capture() {
                    AttemptStatus::Charged
                } else {
                    AttemptStatus::Authorized
                };

                // Re-publish the credential-establishing `initial_transaction_id` the request
                // read, verbatim. NMI requires it on every transaction after the first, so a
                // reference republished with `mandate_metadata: None` would silently downgrade
                // the SECOND and every later charge to the unreferenced body for a caller that
                // persists what this flow returns. Never `response.transactionid`:
                // that is this MIT's own new id, not the transaction the credential was
                // established on. The other `MandateReferenceId` variants are refused
                // pre-request, so the `else` is unreachable in practice.
                let mandate_metadata =
                    if let MandateReferenceId::ConnectorMandateId(connector_mandate_ids) =
                        &item.router_data.request.mandate_reference
                    {
                        connector_mandate_ids.get_mandate_metadata()
                    } else {
                        None
                    };

                (
                    status,
                    Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(
                            response.transactionid.clone(),
                        ),
                        redirection_data: None,
                        mandate_reference: response.customer_vault_id.as_ref().map(|vault_id| {
                            Box::new(MandateReference {
                                connector_mandate_id: Some(vault_id.clone().expose()),
                                payment_method_id: None,
                                connector_mandate_request_reference_id: None,
                                mandate_metadata,
                            })
                        }),
                        connector_metadata: None,
                        network_txn_id: None,
                        network_txn_link_id: None,
                        // Hyperswitch parity: NMI maps connector_response_reference_id to the
                        // merchant `orderid` (echoed back), not the connector `transactionid`
                        // (which is already the resource_id / ConnectorTransactionId above).
                        // The MIT body echoes `orderid=` empty when no merchant charge id was
                        // sent, so the non-blank guard decides between a reference and None.
                        connector_response_reference_id: response.connector_reference_id(),
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                        splits: None,
                        payment_account_reference: None,
                    }),
                )
            }
            // P-ThreeDS-09: an MIT is the only flow that can act on NMI's 260-264 "declined
            // with further instructions" subfamily -- `network_advice_code` is what tells a
            // dunning loop to stop retrying this credential.
            Response::Declined | Response::Error => {
                let (network_decline_code, network_advice_code, network_error_message) =
                    nmi_network_codes(&response.response_code, &response.responsetext);
                (
                    AttemptStatus::Failure,
                    Err(domain_types::router_data::ErrorResponse {
                        code: response.response_code.clone(),
                        message: response.responsetext.clone(),
                        reason: Some(response.responsetext.clone()),
                        status_code: item.http_code,
                        attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
                        connector_transaction_id: Some(response.transactionid.clone()),
                        network_decline_code,
                        network_advice_code,
                        network_error_message,
                        typed_connector_response: None,
                        raw_connector_response: None,
                        raw_connector_request: None,
                        typed_connector_request: None,
                    }),
                )
            }
            // The gateway returned a `response=` value outside the documented {1,2,3}. The
            // attempt's state is unknown, so the previous status is carried through rather than
            // resolved terminally in either direction, and PSync resolves it.
            Response::Unknown => {
                tracing::warn!(
                    response = %response.response_code,
                    "nmi: unrecognised `response` value on RepeatPayment; keeping the previous \
                     attempt status"
                );
                (
                    item.router_data.resource_common_data.status,
                    Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(
                            response.transactionid.clone(),
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
                    }),
                )
            }
        };

        Ok(Self {
            response: payment_response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== INCOMING WEBHOOK TYPES =====
// Ports the hyperswitch NMI webhook types/behaviour 1:1
// (crates/hyperswitch_connectors/src/connectors/nmi/transformers.rs).

#[derive(Debug, Deserialize)]
pub struct NmiWebhookObjectReference {
    /// The discriminator has to be read here too: `chargeback.batch.complete` carries an
    /// `event_body` with none of the members below, so the reference resolver has to branch
    /// on the event before it looks at the action.
    pub event_type: NmiWebhookEventType,
    pub event_body: NmiReferenceBody,
}

/// Every member is optional because NMI's own documented deliveries disagree about which
/// are present: the verbatim `transaction.void.success` payload has `transaction_id`,
/// `condition` and `action` but no `order_id`, and the chargeback payload has none of them.
#[derive(Debug, Deserialize)]
pub struct NmiReferenceBody {
    pub order_id: Option<String>,
    pub transaction_id: Option<String>,
    pub action: Option<NmiActionBody>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NmiActionBody {
    pub action_type: NmiActionType,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum NmiActionType {
    Auth,
    Capture,
    Credit,
    Refund,
    Sale,
    Void,
    Return,
    Validate,
    /// Any `action_type` outside the documented set. Without this a `validate` action from a
    /// SetupMandate transaction, or an ACH `return`, fails the whole webhook deserialization.
    #[serde(other)]
    Unrecognised,
}

#[derive(Debug, Deserialize)]
pub struct NmiWebhookEventBody {
    pub event_type: NmiWebhookEventType,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum NmiWebhookEventType {
    #[serde(rename = "transaction.sale.success")]
    SaleSuccess,
    #[serde(rename = "transaction.sale.failure")]
    SaleFailure,
    #[serde(rename = "transaction.sale.unknown")]
    SaleUnknown,
    #[serde(rename = "transaction.auth.success")]
    AuthSuccess,
    #[serde(rename = "transaction.auth.failure")]
    AuthFailure,
    #[serde(rename = "transaction.auth.unknown")]
    AuthUnknown,
    #[serde(rename = "transaction.refund.success")]
    RefundSuccess,
    #[serde(rename = "transaction.refund.failure")]
    RefundFailure,
    #[serde(rename = "transaction.refund.unknown")]
    RefundUnknown,
    #[serde(rename = "transaction.void.success")]
    VoidSuccess,
    #[serde(rename = "transaction.void.failure")]
    VoidFailure,
    #[serde(rename = "transaction.void.unknown")]
    VoidUnknown,
    #[serde(rename = "transaction.capture.success")]
    CaptureSuccess,
    #[serde(rename = "transaction.capture.failure")]
    CaptureFailure,
    #[serde(rename = "transaction.capture.unknown")]
    CaptureUnknown,
    /// NMI's only dispute-related event.
    #[serde(rename = "chargeback.batch.complete")]
    ChargebackBatchComplete,
    /// The documented catalogue also carries `transaction.credit.*`,
    /// `transaction.validate.*`, `transaction.check.status.*`, `recurring.*` and
    /// `settlement.*`. Without this arm any of them fails the whole deserialization and
    /// ParseEvent answers NotFound instead of "event not supported".
    #[serde(other)]
    Unrecognised,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NmiWebhookBody {
    pub event_body: NmiWebhookObject,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NmiWebhookObject {
    pub transaction_id: String,
    /// Optional for the same reason as `NmiReferenceBody::order_id`: the spec's verbatim
    /// `transaction.void.success` delivery omits it, and nothing here reads it.
    pub order_id: Option<String>,
    pub condition: NmiStatus,
    pub action: NmiActionBody,
}

/// `chargeback.batch.complete` — NMI's only dispute event, and a *batch* summary rather
/// than one dispute: `{merchant, processor, count, chargeback_amount, chargebacks[]}`.
#[derive(Debug, Deserialize)]
pub struct NmiChargebackWebhookBody {
    pub event_body: NmiChargebackBody,
}

#[derive(Debug, Deserialize)]
pub struct NmiChargebackBody {
    pub count: Option<u64>,
    /// The batch total, a major-unit string on the wire. Typed so it cannot be mistaken
    /// for a minor-unit value, but never converted: without a currency there is nothing
    /// to convert it against (plan UD-09).
    pub chargeback_amount: Option<common_utils::types::StringMajorUnit>,
    #[serde(default)]
    pub chargebacks: Vec<NmiChargeback>,
}

/// No currency member on the entry or on the envelope, and the only identifier is `id`,
/// which no NMI endpoint accepts.
#[derive(Debug, Deserialize)]
pub struct NmiChargeback {
    pub id: Option<String>,
    pub reason: Option<String>,
}

/// NMI publishes no dispute record (see `get_nmi_webhook_event`), so this log line is the
/// only surviving trace of a `chargeback.batch.complete` delivery: every entry of the batch
/// plus the batch totals, so nothing is dropped and a multi-entry batch is not reduced to
/// `chargebacks[0]`. A body that does not parse as a chargeback batch is logged as such
/// rather than refused — the caller is already being told the event is unsupported.
pub(crate) fn log_nmi_chargeback_batch(request: &domain_types::connector_types::RequestDetails) {
    match serde_json::from_slice::<NmiChargebackWebhookBody>(&request.body) {
        Ok(body) => tracing::warn!(
            count = ?body.event_body.count,
            chargeback_amount = ?body.event_body.chargeback_amount,
            chargebacks = %body
                .event_body
                .chargebacks
                .iter()
                .map(|chargeback| format!(
                    "id={:?} reason={:?}",
                    chargeback.id, chargeback.reason
                ))
                .collect::<Vec<_>>()
                .join(", "),
            "nmi chargeback.batch.complete received; NMI's batch report carries no currency \
             and no transaction id, so no dispute record is published"
        ),
        Err(error) => tracing::warn!(
            error = ?error,
            "nmi chargeback.batch.complete received but its event_body did not parse; \
             no dispute record is published"
        ),
    }
}

/// Webhook resource object for payment actions. Mirrors the hyperswitch webhook
/// `SyncResponse` shape: `{"transaction":{"transaction_id":"...","condition":"..."}}`.
#[derive(Debug, Deserialize, Serialize)]
pub struct NmiWebhookSyncResponse {
    pub transaction: Option<NmiWebhookSyncTransactionResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NmiWebhookSyncTransactionResponse {
    pub transaction_id: String,
    pub condition: NmiStatus,
}

impl From<&NmiWebhookBody> for NmiWebhookSyncResponse {
    fn from(item: &NmiWebhookBody) -> Self {
        Self {
            transaction: Some(NmiWebhookSyncTransactionResponse {
                transaction_id: item.event_body.transaction_id.to_owned(),
                condition: item.event_body.condition.to_owned(),
            }),
        }
    }
}

/// Maps the NMI webhook `event_type` to the prism webhook event type.
/// Ports HS `get_nmi_webhook_event` 1:1. The `*.unknown` events map to
/// `IncomingWebhookEventUnspecified` (proto `UNSPECIFIED`), which hyperswitch
/// converts back to `IncomingWebhookEvent::EventNotSupported` — matching the
/// Direct gateway's `EventNotSupported`.
pub(crate) fn get_nmi_webhook_event(
    status: NmiWebhookEventType,
) -> domain_types::connector_types::EventType {
    use domain_types::connector_types::EventType;
    match status {
        NmiWebhookEventType::SaleSuccess => EventType::PaymentIntentSuccess,
        NmiWebhookEventType::SaleFailure => EventType::PaymentIntentFailure,
        NmiWebhookEventType::RefundSuccess => EventType::RefundSuccess,
        NmiWebhookEventType::RefundFailure => EventType::RefundFailure,
        NmiWebhookEventType::VoidSuccess => EventType::PaymentIntentCancelled,
        NmiWebhookEventType::AuthSuccess => EventType::PaymentIntentAuthorizationSuccess,
        NmiWebhookEventType::CaptureSuccess => EventType::PaymentIntentCaptureSuccess,
        NmiWebhookEventType::AuthFailure => EventType::PaymentIntentAuthorizationFailure,
        NmiWebhookEventType::CaptureFailure => EventType::PaymentIntentCaptureFailure,
        NmiWebhookEventType::VoidFailure => EventType::PaymentIntentCancelFailure,
        // `ChargebackBatchComplete` is deliberately *not* `DisputeOpened`. `DisputeOpened`
        // satisfies `EventType::is_dispute_event()`, which makes
        // `webhook_utils::process_webhook_event` dispatch unconditionally into
        // `get_disputes_webhook_content` -> `process_dispute_webhook`, and NMI's chargeback
        // event cannot be expressed as a `DisputeWebhookDetailsResponse`: it is a *batch*
        // summary (`{count, chargeback_amount, chargebacks[{id, reason, …}]}`) with no
        // per-chargeback currency and no transaction id, while the shared type requires a
        // `Currency`, a `StringMinorUnit` and one `dispute_id` — and plan C-02 rejected
        // relaxing it. Publishing a dispute event whose content path can only fail is the
        // two-halves-disagree defect; so the batch is published as "event not supported",
        // exactly like the `*.unknown` family, and `get_event_type` logs it so the
        // notification is still on the record.
        NmiWebhookEventType::ChargebackBatchComplete
        | NmiWebhookEventType::SaleUnknown
        | NmiWebhookEventType::RefundUnknown
        | NmiWebhookEventType::AuthUnknown
        | NmiWebhookEventType::VoidUnknown
        | NmiWebhookEventType::CaptureUnknown
        | NmiWebhookEventType::Unrecognised => EventType::IncomingWebhookEventUnspecified,
    }
}

/// The wire spelling of `event_body.action.action_type`, re-read from the raw body.
/// `#[serde(other)]` folds every undocumented value into `NmiActionType::Unrecognised` and
/// discards the string, so looking again is the only way to name it in a log line.
pub(crate) fn raw_webhook_action_type(
    request: &domain_types::connector_types::RequestDetails,
) -> Option<String> {
    serde_json::from_slice::<serde_json::Value>(&request.body)
        .ok()?
        .pointer("/event_body/action/action_type")
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
}

/// Extracts the `webhook-signature` header value (case-insensitive lookup, matching
/// the case-insensitive actix `HeaderMap::get` used by hyperswitch).
pub(crate) fn get_nmi_webhook_signature_header(
    request: &domain_types::connector_types::RequestDetails,
) -> Result<&str, error_stack::Report<domain_types::errors::WebhookError>> {
    request
        .headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("webhook-signature"))
        .map(|(_, value)| value.as_str())
        .ok_or_else(|| {
            error_stack::report!(domain_types::errors::WebhookError::WebhookSignatureNotFound)
        })
}

/// Splits an NMI `webhook-signature` header (`t=<nonce>,s=<hex signature>`) into
/// `(nonce, signature)`. Mimics the hyperswitch regex `r"t=(.*),s=(.*)"` exactly:
/// leftmost `t=` match with greedy captures, i.e. the nonce runs to the LAST `,s=`.
pub(crate) fn parse_nmi_webhook_signature_header(header: &str) -> Option<(&str, &str)> {
    let t_idx = header.find("t=")?;
    let after_t = header.get(t_idx + 2..)?;
    let s_idx = after_t.rfind(",s=")?;
    Some((after_t.get(..s_idx)?, after_t.get(s_idx + 3..)?))
}
