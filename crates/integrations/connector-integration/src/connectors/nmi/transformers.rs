use crate::types::ResponseRouterData;
use common_enums::{AttemptStatus, RefundStatus};
use common_utils::types::{AmountConvertor, FloatMajorUnit, FloatMajorUnitForConnector};
use domain_types::{
    connector_flow::{
        Authorize, Capture, PSync, PreAuthenticate, RSync, Refund, RepeatPayment, SetupMandate,
        Void,
    },
    connector_types::{
        MandateReference, MandateReferenceId, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsPreAuthenticateData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, RepeatPaymentData, ResponseId, SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::{
        ApplePayPaymentData, BankDebitData, GpayTokenizationData, PaymentMethodData,
        PaymentMethodDataTypes, RawCardNumber, WalletData,
    },
    router_data::{
        AdditionalPaymentMethodConnectorResponse, ConnectorResponseData, ConnectorSpecificConfig,
        ErrorResponse, FlowStatus,
    },
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
    utils::{get_unimplemented_payment_method_error_message, ForeignTryFrom},
};
use grpc_api_types::payments::{Currency, Money};

// Note: Refund and RefundsData are used for the Refund flow implementation
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use serde_json;
use std::str::FromStr;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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

/// Decides `type=sale` vs `type=auth` for every charging request from the capture method.
/// NMI documents a single capture of at most the authorised amount
/// (https://docs.nmi.com/reference/getting-started, "Transaction Types": sale / auth / capture),
/// so multiple partial captures and scheduled captures are refused before any call.
pub(crate) fn get_nmi_transaction_type(
    capture_method: Option<common_enums::CaptureMethod>,
) -> Result<TransactionType, error_stack::Report<IntegrationError>> {
    match capture_method {
        None
        | Some(common_enums::CaptureMethod::Automatic)
        | Some(common_enums::CaptureMethod::SequentialAutomatic) => Ok(TransactionType::Sale),
        Some(common_enums::CaptureMethod::Manual) => Ok(TransactionType::Auth),
        Some(
            capture_method @ (common_enums::CaptureMethod::ManualMultiple
            | common_enums::CaptureMethod::Scheduled),
        ) => Err(error_stack::report!(IntegrationError::NotSupported {
            message: format!("capture_method {capture_method}"),
            connector: "NMI",
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Use capture_method AUTOMATIC or MANUAL: NMI supports one capture of at most the authorised amount"
                        .to_string()
                ),
                doc_url: Some("https://docs.nmi.com/reference/getting-started".to_string()),
                additional_context: None,
            },
        })),
    }
}

// ===== NMI STATUS ENUM =====

/// `condition` of a Query API transaction and of a webhook `event_body`
/// (https://docs.nmi.com/reference/query-api, "condition").
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub enum NmiCondition {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "pendingsettlement")]
    Pendingsettlement,
    #[serde(rename = "complete")]
    Complete,
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(rename = "abandoned")]
    Abandoned,
    #[serde(rename = "canceled")]
    Canceled,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "unknown")]
    Unknown,
    /// A condition NMI has not documented: never assumed terminal.
    #[serde(other)]
    Unrecognized,
}

impl From<NmiCondition> for AttemptStatus {
    fn from(item: NmiCondition) -> Self {
        match item {
            // Query API condition "abandoned": the cardholder did not complete authentication.
            NmiCondition::Abandoned => Self::AuthenticationFailed,
            // Query API condition "canceled": voided.
            NmiCondition::Canceled => Self::Voided,
            // Query API condition "pending": authorised, awaiting capture.
            NmiCondition::Pending => Self::Authorized,
            // Query API conditions "pendingsettlement" / "complete": captured.
            NmiCondition::Pendingsettlement | NmiCondition::Complete => Self::Charged,
            // Query API condition "in_progress": still being processed.
            NmiCondition::InProgress => Self::AuthenticationPending,
            // Query API condition "failed": the transaction failed.
            NmiCondition::Failed => Self::Failure,
            // Query API condition "unknown": NMI cannot report the state, so it is non-terminal
            // (Hyperswitch maps it to Failure; deliberate divergence).
            NmiCondition::Unknown => Self::Pending,
            // Undocumented value: non-terminal.
            NmiCondition::Unrecognized => Self::Pending,
        }
    }
}

impl From<NmiCondition> for RefundStatus {
    fn from(item: NmiCondition) -> Self {
        match item {
            // Query API conditions "abandoned" / "canceled" / "failed".
            NmiCondition::Abandoned | NmiCondition::Canceled | NmiCondition::Failed => {
                Self::Failure
            }
            // Query API condition "unknown": NMI cannot report the state, so it is non-terminal
            // (Hyperswitch maps it to Failure; deliberate divergence).
            NmiCondition::Unknown => Self::Pending,
            // Query API conditions "pending" / "in_progress": not yet settled.
            NmiCondition::Pending | NmiCondition::InProgress => Self::Pending,
            // Query API conditions "pendingsettlement" / "complete": refund accepted.
            NmiCondition::Pendingsettlement | NmiCondition::Complete => Self::Success,
            // Undocumented value: non-terminal.
            NmiCondition::Unrecognized => Self::Pending,
        }
    }
}

// ===== PAYMENT METHOD DATA =====

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum NmiPaymentMethod<T: PaymentMethodDataTypes> {
    Card(Box<CardData<T>>),
    Ach(Box<AchData>),
    GooglePay(Box<GooglePayData>),
    GooglePayDecrypt(Box<GooglePayDecryptedData>),
    ApplePay(Box<NmiApplePayData>),
    ApplePayDecrypt(Box<NmiApplePayDecryptedData>),
}

// ===== GOOGLE PAY DATA =====

/// Google Pay, NMI decryption: the raw `tokenizationData.token` JSON string.
#[derive(Debug, Serialize)]
pub struct GooglePayData {
    googlepay_payment_data: Secret<String>,
}

// ===== APPLE PAY DATA =====

/// Apple Pay, NMI decryption: `applepay_payment_data`.
#[derive(Debug, Serialize)]
pub struct NmiApplePayData {
    applepay_payment_data: Secret<String>,
}

/// Apple Pay, merchant/Hyperswitch decryption: DPAN + cryptogram in the card fields.
#[derive(Debug, Serialize)]
pub struct NmiApplePayDecryptedData {
    decrypted_applepay_data: DecryptedDataIndicator,
    ccnumber: Secret<String>,
    ccexp: Secret<String>,
    cavv: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    eci: Option<String>,
}

/// Builds the wallet part of a transact.php request for Apple Pay and Google Pay, in both the
/// NMI-decryption (encrypted token) and merchant-decryption (DPAN + cryptogram) modes.
pub(crate) fn nmi_wallet_payment_method<T: PaymentMethodDataTypes>(
    wallet_data: &WalletData,
) -> Result<NmiPaymentMethod<T>, error_stack::Report<IntegrationError>> {
    match wallet_data {
        WalletData::GooglePay(google_pay_data) => match &google_pay_data.tokenization_data {
            GpayTokenizationData::Decrypted(decrypted_data) => {
                let ccexp = decrypted_data.get_expiry_date_as_mmyy().change_context(
                    IntegrationError::RequestEncodingFailed {
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "Google Pay decrypted expiry could not be formatted as MMYY"
                                    .to_string(),
                            ),
                            ..Default::default()
                        },
                    },
                )?;
                Ok(NmiPaymentMethod::GooglePayDecrypt(Box::new(
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
                Ok(NmiPaymentMethod::GooglePay(Box::new(GooglePayData {
                    googlepay_payment_data: Secret::new(encrypted_data.token.clone()),
                })))
            }
        },
        WalletData::ApplePay(apple_pay_data) => match &apple_pay_data.payment_data {
            ApplePayPaymentData::Encrypted(encrypted_data) => {
                // NMI takes the Apple Pay `paymentData` as hex of its base64-decoded bytes
                // (Hyperswitch reference nmi/transformers.rs, live-proven; NMI's docs do not
                // state the encoding).
                let decoded = base64::Engine::decode(
                    &base64::engine::general_purpose::STANDARD,
                    encrypted_data,
                )
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "apple_pay.payment_data",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Send the Apple Pay paymentData exactly as received from the device, base64-encoded"
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "Apple Pay payment_data is not valid base64".to_string(),
                        ),
                    },
                })?;
                Ok(NmiPaymentMethod::ApplePay(Box::new(NmiApplePayData {
                    applepay_payment_data: Secret::new(hex::encode(decoded)),
                })))
            }
            ApplePayPaymentData::Decrypted(decrypted_data) => {
                let ccexp = decrypted_data.get_expiry_date_as_mmyy().change_context(
                    IntegrationError::RequestEncodingFailed {
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "Apple Pay decrypted expiry could not be formatted as MMYY"
                                    .to_string(),
                            ),
                            ..Default::default()
                        },
                    },
                )?;
                Ok(NmiPaymentMethod::ApplePayDecrypt(Box::new(
                    NmiApplePayDecryptedData {
                        decrypted_applepay_data: DecryptedDataIndicator::Decrypted,
                        ccnumber: Secret::new(
                            decrypted_data
                                .application_primary_account_number
                                .get_card_no(),
                        ),
                        ccexp,
                        cavv: decrypted_data
                            .payment_data
                            .online_payment_cryptogram
                            .clone(),
                        eci: decrypted_data.payment_data.eci_indicator.clone(),
                    },
                )))
            }
        },
        _ => Err(error_stack::report!(IntegrationError::NotImplemented(
            get_unimplemented_payment_method_error_message("NMI"),
            IntegrationErrorContext {
                suggested_action: Some(
                    "NMI supports Apple Pay and Google Pay wallets only".to_string()
                ),
                ..Default::default()
            }
        ))),
    }
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
    xid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    eci: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cvv: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    three_ds_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    directory_server_id: Option<Secret<String>>,
    // Level 2 / Level 3 data (card payments only)
    #[serde(flatten)]
    #[serde(skip_serializing_if = "Option::is_none")]
    l2_l3: Option<NmiLevel2Level3Data>,
    // Dynamic descriptor (card and Apple Pay payments)
    #[serde(flatten)]
    #[serde(skip_serializing_if = "Option::is_none")]
    descriptor: Option<NmiDescriptor>,
}

// ===== LEVEL 2 / LEVEL 3 DATA =====

/// NMI line items are numbered `item_*_1` .. `item_*_99`.
const NMI_MAX_LINE_ITEMS: usize = 99;

/// Level 2 / Level 3 fields of transact.php (tax, shipping, ponumber, L3 order fields and
/// `item_*_#` line items).
#[derive(Debug, Serialize)]
pub struct NmiLevel2Level3Data {
    #[serde(skip_serializing_if = "Option::is_none")]
    tax: Option<FloatMajorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping: Option<FloatMajorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duty_amount: Option<FloatMajorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    discount_amount: Option<FloatMajorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ponumber: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    customer_vat_registration: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    merchant_vat_registration: Option<Secret<String>>,
    /// YYMMDD
    #[serde(skip_serializing_if = "Option::is_none")]
    order_date: Option<String>,
    #[serde(flatten)]
    line_items: std::collections::BTreeMap<String, NmiLineItemValue>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum NmiLineItemValue {
    Amount(FloatMajorUnit),
    Quantity(u16),
    Text(Secret<String>),
}

fn convert_nmi_money(
    money: &common_utils::types::Money,
    field: &'static str,
) -> Result<FloatMajorUnit, error_stack::Report<IntegrationError>> {
    FloatMajorUnitForConnector
        .convert(money)
        .change_context(IntegrationError::InvalidDataFormat {
            field_name: field,
            context: IntegrationErrorContext {
                additional_context: Some(
                    "amount could not be converted to NMI's major-unit format".to_string(),
                ),
                ..Default::default()
            },
        })
}

fn get_nmi_level2_level3_data(
    l2_l3_data: Option<&domain_types::connector_types::L2L3Data>,
    currency: common_enums::Currency,
) -> Result<Option<NmiLevel2Level3Data>, error_stack::Report<IntegrationError>> {
    let Some(l2_l3_data) = l2_l3_data else {
        return Ok(None);
    };
    let order_info = l2_l3_data.order_info.as_ref();
    let tax_info = l2_l3_data.tax_info.as_ref();

    let convert_opt = |money: Option<&common_utils::types::Money>, field: &'static str| {
        money
            .map(|money| convert_nmi_money(money, field))
            .transpose()
    };
    let convert_minor = |amount: Option<common_utils::types::MinorUnit>, field: &'static str| {
        amount
            .map(|amount| {
                convert_nmi_money(&common_utils::types::Money { amount, currency }, field)
            })
            .transpose()
    };

    let mut line_items = std::collections::BTreeMap::new();
    if let Some(order_details) = order_info.and_then(|info| info.order_details.as_ref()) {
        if order_details.len() > NMI_MAX_LINE_ITEMS {
            return Err(error_stack::report!(IntegrationError::InvalidDataFormat {
                field_name: "l2_l3_data.order_info.order_details",
                context: IntegrationErrorContext {
                    suggested_action: Some(format!(
                        "Send at most {NMI_MAX_LINE_ITEMS} line items: NMI numbers them item_*_1..item_*_{NMI_MAX_LINE_ITEMS}"
                    )),
                    doc_url: Some("https://docs.nmi.com/reference/getting-started".to_string()),
                    additional_context: Some(format!(
                        "{} line items supplied",
                        order_details.len()
                    )),
                },
            }));
        }
        for (index, item) in order_details.iter().enumerate() {
            let n = index + 1;
            let mut put_text = |key: &str, value: Option<&String>| {
                if let Some(value) = value {
                    line_items.insert(
                        format!("{key}_{n}"),
                        NmiLineItemValue::Text(Secret::new(value.clone())),
                    );
                }
            };
            put_text("item_product_code", item.product_id.as_ref());
            put_text("item_description", Some(&item.product_name));
            put_text("item_commodity_code", item.commodity_code.as_ref());
            put_text("item_unit_of_measure", item.unit_of_measure.as_ref());
            line_items.insert(
                format!("item_quantity_{n}"),
                NmiLineItemValue::Quantity(item.quantity),
            );
            let amounts = [
                ("item_unit_cost", Some(item.amount)),
                ("item_total_amount", item.total_amount),
                ("item_tax_amount", item.total_tax_amount),
                ("item_discount_amount", item.unit_discount_amount),
            ];
            for (key, amount) in amounts {
                if let Some(amount) =
                    convert_minor(amount, "l2_l3_data.order_info.order_details.amount")?
                {
                    line_items.insert(format!("{key}_{n}"), NmiLineItemValue::Amount(amount));
                }
            }
        }
    }

    let order_date = order_info.and_then(|info| info.order_date).map(|date| {
        format!(
            "{:02}{:02}{:02}",
            date.year().rem_euclid(100),
            u8::from(date.month()),
            date.day()
        )
    });

    let data = NmiLevel2Level3Data {
        tax: convert_opt(
            tax_info.and_then(|info| info.order_tax_amount.as_ref()),
            "l2_l3_data.tax_info.order_tax_amount",
        )?,
        shipping: convert_opt(
            order_info.and_then(|info| info.shipping_cost.as_ref()),
            "l2_l3_data.order_info.shipping_cost",
        )?,
        duty_amount: convert_opt(
            order_info.and_then(|info| info.duty_amount.as_ref()),
            "l2_l3_data.order_info.duty_amount",
        )?,
        discount_amount: convert_opt(
            order_info.and_then(|info| info.discount_amount.as_ref()),
            "l2_l3_data.order_info.discount_amount",
        )?,
        ponumber: order_info.and_then(|info| info.merchant_order_reference_id.clone()),
        customer_vat_registration: tax_info
            .and_then(|info| info.customer_tax_registration_id.clone()),
        merchant_vat_registration: tax_info
            .and_then(|info| info.merchant_tax_registration_id.clone()),
        order_date,
        line_items,
    };

    let is_empty = data.tax.is_none()
        && data.shipping.is_none()
        && data.duty_amount.is_none()
        && data.discount_amount.is_none()
        && data.ponumber.is_none()
        && data.customer_vat_registration.is_none()
        && data.merchant_vat_registration.is_none()
        && data.order_date.is_none()
        && data.line_items.is_empty();
    Ok((!is_empty).then_some(data))
}

// ===== DYNAMIC DESCRIPTOR =====

/// Dynamic descriptor fields of transact.php ("supported processors only").
#[derive(Debug, Serialize)]
pub struct NmiDescriptor {
    #[serde(skip_serializing_if = "Option::is_none")]
    descriptor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    descriptor_phone: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    descriptor_city: Option<Secret<String>>,
}

fn get_nmi_descriptor(
    billing_descriptor: Option<&domain_types::connector_types::BillingDescriptor>,
) -> Option<NmiDescriptor> {
    billing_descriptor.and_then(|billing_descriptor| {
        let descriptor = NmiDescriptor {
            descriptor: billing_descriptor.statement_descriptor.clone(),
            descriptor_phone: billing_descriptor.phone.clone(),
            descriptor_city: billing_descriptor.city.clone(),
        };
        (descriptor.descriptor.is_some()
            || descriptor.descriptor_phone.is_some()
            || descriptor.descriptor_city.is_some())
        .then_some(descriptor)
    })
}

// ===== EXTERNAL 3DS =====

/// Merchant-supplied 3DS result carried on a card sale/auth.
struct NmiExternalThreeDs {
    cavv: Secret<String>,
    eci: Option<String>,
    three_ds_version: Option<String>,
    directory_server_id: Option<Secret<String>>,
}

/// A card payment requested with 3DS must carry the authentication result: NMI's transact.php
/// has no server-side 3DS start, so charging it without one would silently downgrade it to
/// non-3DS.
fn get_nmi_external_three_ds<T: PaymentMethodDataTypes>(
    router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
) -> Result<Option<NmiExternalThreeDs>, error_stack::Report<IntegrationError>> {
    match router_data.resource_common_data.auth_type {
        common_enums::AuthenticationType::NoThreeDs => Ok(None),
        common_enums::AuthenticationType::ThreeDs => {
            let authentication_data =
                router_data
                    .request
                    .authentication_data
                    .as_ref()
                    .ok_or_else(|| {
                        error_stack::report!(IntegrationError::MissingRequiredField {
                            field_name: "authentication_data",
                            context: IntegrationErrorContext {
                                suggested_action: Some(
                                    "Supply the 3DS authentication result (cavv, eci, message_version) or send auth_type NO_THREE_DS"
                                        .to_string()
                                ),
                                doc_url: None,
                                additional_context: Some(
                                    "NMI has no server-side 3DS start on transact.php".to_string()
                                ),
                            },
                        })
                    })?;
            let cavv = authentication_data.cavv.clone().ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                    field_name: "authentication_data.cavv",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Supply authentication_data.cavv: NMI requires it for a 3DS card payment"
                                .to_string()
                        ),
                        doc_url: None,
                        additional_context: None,
                    },
                })
            })?;
            Ok(Some(NmiExternalThreeDs {
                cavv,
                eci: authentication_data.eci.clone(),
                three_ds_version: authentication_data
                    .message_version
                    .as_ref()
                    .map(|version| version.to_string()),
                // NMI `directory_server_id` is the Directory Server transaction id
                // (dsTransID); the 3DS-server transaction id is a different identifier,
                // so it is never substituted when `ds_trans_id` is absent.
                directory_server_id: authentication_data.ds_trans_id.clone().map(Secret::new),
            }))
        }
    }
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

            // Gateway.js posts either the 3DS result or a `{code, message}` failure back as the
            // redirect payload (https://docs.nmi.com/docs/payer-authentication-3ds).
            let three_ds_data = match serde_json::from_value::<NmiRedirectResponse>(
                payload_data.expose(),
            )
            .change_context(IntegrationError::MissingRequiredField {
                field_name: "redirect_response.payload",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Pass the Gateway.js 3DS result (customerVaultId, orderId, cavv, eci, ...) as the redirect payload"
                            .to_string(),
                    ),
                    doc_url: Some("https://docs.nmi.com/docs/payer-authentication-3ds".to_string()),
                    additional_context: None,
                },
            })? {
                NmiRedirectResponse::NmiRedirectResponseData(data) => data,
                // A failed client-side authentication must not reach transact.php.
                NmiRedirectResponse::NmiErrorResponseData(error) => {
                    return Err(error_stack::report!(IntegrationError::NotSupported {
                        message: "Authorize after a failed Gateway.js 3DS authentication".to_string(),
                        connector: "NMI",
                        context: IntegrationErrorContext {
                            suggested_action: Some(
                                "Retry the payment; the cardholder did not complete 3DS authentication"
                                    .to_string(),
                            ),
                            doc_url: Some(
                                "https://docs.nmi.com/docs/payer-authentication-3ds".to_string(),
                            ),
                            additional_context: Some(format!(
                                "Gateway.js 3DS error code={} message={}",
                                error.code, error.message
                            )),
                        },
                    }));
                }
            };

            let cvv = match &router_data.request.payment_method_data {
                PaymentMethodData::Card(card_data) => Some(card_data.card_cvc.clone()),
                _ => None,
            };

            let converter = FloatMajorUnitForConnector;
            let amount = converter
                .convert(&router_data.request.amount)
                .change_context(IntegrationError::RequestEncodingFailed {
                    context: Default::default(),
                })?;

            let transaction_type = get_nmi_transaction_type(router_data.request.capture_method)?;

            Ok(Self {
                security_key: auth.api_key.clone(),
                transaction_type,
                amount,
                currency: None,
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
                cavv: three_ds_data.cavv.map(Secret::new),
                xid: three_ds_data.xid,
                eci: three_ds_data.eci,
                cvv,
                three_ds_version: three_ds_data.three_ds_version,
                directory_server_id: three_ds_data.directory_server_id,
                l2_l3: None,
                descriptor: None,
            })
        } else {
            // Regular authorization flow
            let mut external_three_ds = None;
            // NMI documents Level 2/3 fields (tax, shipping, ponumber, item_*_#) for card
            // transactions only, so l2_l3_data is not sent on wallet or ACH payments.
            let mut l2_l3 = None;
            let mut descriptor = None;
            let (payment_method, transaction_type) = match &router_data.request.payment_method_data
            {
                PaymentMethodData::Card(_) => {
                    let transaction_type =
                        get_nmi_transaction_type(router_data.request.capture_method)?;
                    external_three_ds = get_nmi_external_three_ds(router_data)?;
                    l2_l3 = get_nmi_level2_level3_data(
                        router_data.resource_common_data.l2_l3_data.as_deref(),
                        router_data.request.currency,
                    )?;
                    descriptor =
                        get_nmi_descriptor(router_data.request.billing_descriptor.as_ref());
                    (
                        NmiPaymentMethod::try_from(&router_data.request.payment_method_data)?,
                        transaction_type,
                    )
                }
                PaymentMethodData::BankDebit(bank_debit_data) => {
                    // NMI ACH is sale-only (no ACH auth form is documented): sending type=sale
                    // for a non-automatic capture would move money while reporting Authorized,
                    // so refuse it before any request is built.
                    // Same set as `is_auto_capture() == false`, bound so the message names it.
                    if let Some(
                        capture_method @ (common_enums::CaptureMethod::Manual
                        | common_enums::CaptureMethod::ManualMultiple
                        | common_enums::CaptureMethod::Scheduled),
                    ) = router_data.request.capture_method
                    {
                        return Err(error_stack::report!(IntegrationError::NotSupported {
                            message: format!("capture_method {capture_method} for ACH bank debit"),
                            connector: "NMI",
                            context: IntegrationErrorContext {
                                suggested_action: Some(
                                    "Use capture_method AUTOMATIC for ACH bank debit: NMI ACH is sale-only"
                                        .to_string(),
                                ),
                                doc_url: Some(
                                    "https://docs.nmi.com/reference/getting-started".to_string(),
                                ),
                                additional_context: None,
                            },
                        }));
                    }
                    let ach_data = create_ach_data(bank_debit_data, router_data)?;
                    (
                        NmiPaymentMethod::Ach(Box::new(ach_data)),
                        TransactionType::Sale,
                    )
                }
                // Google Pay, both modes: NMI decryption (`googlepay_payment_data`) and
                // merchant/Hyperswitch decryption (`decrypted_googlepay_data=1` + DPAN/cryptogram).
                // Sale vs auth follows the capture method in both modes.
                PaymentMethodData::Wallet(wallet_data @ WalletData::GooglePay(_)) => {
                    let transaction_type =
                        get_nmi_transaction_type(router_data.request.capture_method)?;
                    descriptor =
                        get_nmi_descriptor(router_data.request.billing_descriptor.as_ref());
                    (nmi_wallet_payment_method(wallet_data)?, transaction_type)
                }
                // Apple Pay, both modes: NMI decryption (`applepay_payment_data`) and
                // merchant/Hyperswitch decryption (`decrypted_applepay_data=1` + DPAN/cryptogram).
                PaymentMethodData::Wallet(wallet_data @ WalletData::ApplePay(_)) => {
                    let transaction_type =
                        get_nmi_transaction_type(router_data.request.capture_method)?;
                    descriptor =
                        get_nmi_descriptor(router_data.request.billing_descriptor.as_ref());
                    (nmi_wallet_payment_method(wallet_data)?, transaction_type)
                }
                _ => {
                    let txn_type = if router_data.request.is_auto_capture() {
                        TransactionType::Sale
                    } else {
                        TransactionType::Auth
                    };
                    (
                        NmiPaymentMethod::try_from(&router_data.request.payment_method_data)?,
                        txn_type,
                    )
                }
            };

            let converter = FloatMajorUnitForConnector;
            let amount = converter
                .convert(&router_data.request.amount)
                .change_context(IntegrationError::RequestEncodingFailed {
                    context: Default::default(),
                })?;

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
                customer_vault: router_data
                    .request
                    .is_mandate_payment()
                    .then_some(CustomerAction::AddCustomer),
                billing_details: Some(NmiBillingDetails {
                    first_name: router_data
                        .resource_common_data
                        .get_optional_billing_first_name(),
                    last_name: router_data
                        .resource_common_data
                        .get_optional_billing_last_name(),
                    address1: router_data
                        .resource_common_data
                        .get_optional_billing_line1(),
                    address2: router_data
                        .resource_common_data
                        .get_optional_billing_line2(),
                    city: router_data.resource_common_data.get_optional_billing_city(),
                    state: router_data
                        .resource_common_data
                        .get_optional_billing_state(),
                    zip: router_data.resource_common_data.get_optional_billing_zip(),
                    country: router_data
                        .resource_common_data
                        .get_optional_billing_country(),
                    phone: router_data
                        .resource_common_data
                        .get_optional_billing_phone_number(),
                    email: router_data
                        .resource_common_data
                        .get_optional_billing_email(),
                }),
                shipping_details: Some(NmiShippingDetails {
                    shipping_firstname: router_data
                        .resource_common_data
                        .get_optional_shipping_first_name(),
                    shipping_lastname: router_data
                        .resource_common_data
                        .get_optional_shipping_last_name(),
                    shipping_address1: router_data
                        .resource_common_data
                        .get_optional_shipping_line1(),
                    shipping_address2: router_data
                        .resource_common_data
                        .get_optional_shipping_line2(),
                    shipping_city: router_data
                        .resource_common_data
                        .get_optional_shipping_city(),
                    shipping_state: router_data
                        .resource_common_data
                        .get_optional_shipping_state(),
                    shipping_zip: router_data.resource_common_data.get_optional_shipping_zip(),
                    shipping_country: router_data
                        .resource_common_data
                        .get_optional_shipping_country(),
                    shipping_email: router_data
                        .resource_common_data
                        .get_optional_shipping_email(),
                }),
                customer_vault_id: None,
                // HS parity: the customer email travels only with an external-3DS card payment
                email: external_three_ds
                    .as_ref()
                    .and_then(|_| router_data.request.email.clone()),
                // cardholder_auth and xid are not sent on an external-3DS payment (HS parity)
                cardholder_auth: None,
                cavv: external_three_ds
                    .as_ref()
                    .map(|three_ds| three_ds.cavv.clone()),
                xid: None,
                eci: external_three_ds
                    .as_ref()
                    .and_then(|three_ds| three_ds.eci.clone()),
                cvv: None,
                three_ds_version: external_three_ds
                    .as_ref()
                    .and_then(|three_ds| three_ds.three_ds_version.clone()),
                directory_server_id: external_three_ds
                    .as_ref()
                    .and_then(|three_ds| three_ds.directory_server_id.clone()),
                l2_l3,
                descriptor,
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

/// NMI sends absent optional values as empty strings (`avsresponse=&cvvresponse=`).
fn non_empty(value: Option<&String>) -> Option<String> {
    value
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// AVS and CVV results of a card transaction, as `payment_checks`.
#[derive(Debug, Serialize)]
struct NmiPaymentChecks {
    #[serde(skip_serializing_if = "Option::is_none")]
    avs_response: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cvv_response: Option<String>,
}

/// AVS and CVV results (`avsresponse`, `cvvresponse`) and the `authcode` of a card
/// transaction, as `connector_response.additional_payment_method_data.card`.
pub(crate) fn get_nmi_card_connector_response(
    avsresponse: Option<&String>,
    cvvresponse: Option<&String>,
    authcode: Option<&String>,
) -> Option<ConnectorResponseData> {
    let avs_response = non_empty(avsresponse);
    let cvv_response = non_empty(cvvresponse);
    let auth_code = non_empty(authcode);
    if avs_response.is_none() && cvv_response.is_none() && auth_code.is_none() {
        return None;
    }
    let payment_checks = if avs_response.is_some() || cvv_response.is_some() {
        serde_json::to_value(NmiPaymentChecks {
            avs_response,
            cvv_response,
        })
        .inspect_err(|error| {
            tracing::warn!(?error, "failed to serialize NMI AVS/CVV payment checks");
        })
        .ok()
    } else {
        None
    };
    Some(ConnectorResponseData::with_additional_payment_method_data(
        AdditionalPaymentMethodConnectorResponse::Card {
            authentication_data: None,
            payment_checks,
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

/// The error-bearing fields every transact.php response carries.
#[derive(Debug, Clone, Copy)]
pub(crate) struct NmiTransactResult<'a> {
    pub response_code: &'a String,
    pub responsetext: &'a String,
    pub transactionid: &'a String,
}

impl<'a> From<&'a StandardResponse> for NmiTransactResult<'a> {
    fn from(response: &'a StandardResponse) -> Self {
        Self {
            response_code: &response.response_code,
            responsetext: &response.responsetext,
            transactionid: &response.transactionid,
        }
    }
}

impl<'a> From<&'a NmiVaultResponseStruct> for NmiTransactResult<'a> {
    fn from(response: &'a NmiVaultResponseStruct) -> Self {
        Self {
            response_code: &response.response_code,
            responsetext: &response.responsetext,
            transactionid: &response.transactionid,
        }
    }
}

impl<'a> From<&'a NmiSetupMandateResponse> for NmiTransactResult<'a> {
    fn from(response: &'a NmiSetupMandateResponse) -> Self {
        Self {
            response_code: &response.response_code,
            responsetext: &response.responsetext,
            transactionid: &response.transactionid,
        }
    }
}

/// Error for a transact.php body that did not approve (`response=2|3`, or an undocumented
/// `response` value). NMI returns declines with HTTP 200, so this is the only place the
/// decline code and text reach the caller.
pub(crate) fn build_nmi_error_response(
    response: NmiTransactResult<'_>,
    http_code: u16,
    attempt_status: Option<FlowStatus>,
) -> ErrorResponse {
    let message = non_empty(Some(response.responsetext));
    ErrorResponse {
        code: non_empty(Some(response.response_code))
            .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
        message: message
            .clone()
            .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
        reason: message,
        status_code: http_code,
        attempt_status,
        connector_transaction_id: non_empty(Some(response.transactionid)),
        network_decline_code: None,
        network_advice_code: None,
        network_error_message: None,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<StandardResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<StandardResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;

        let mut connector_response = item
            .router_data
            .resource_common_data
            .connector_response
            .clone();
        let (status, payment_response) = match response.response {
            Response::Approved => {
                // type=sale -> Charged, type=auth -> Authorized; the request derives the type
                // from the same capture method (get_nmi_transaction_type).
                let status = if item.router_data.request.is_auto_capture() {
                    AttemptStatus::Charged
                } else {
                    AttemptStatus::Authorized
                };
                if let PaymentMethodData::Card(_) = item.router_data.request.payment_method_data {
                    connector_response = get_nmi_card_connector_response(
                        response.avsresponse.as_ref(),
                        response.cvvresponse.as_ref(),
                        response.authcode.as_ref(),
                    )
                    .or(connector_response);
                }
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
                                mandate_metadata: None,
                            })
                        }),
                        connector_metadata: None,
                        network_txn_id: None,
                        network_txn_link_id: None,
                        connector_response_reference_id: Some(response.orderid.clone()),
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                        splits: None,
                        payment_account_reference: None,
                    }),
                )
            }
            // response=2 (declined) / response=3 (error in data or system error)
            Response::Declined | Response::Error => (
                AttemptStatus::Failure,
                Err(build_nmi_error_response(
                    response.into(),
                    item.http_code,
                    Some(FlowStatus::Payment(AttemptStatus::Failure)),
                )),
            ),
            // Undocumented `response` value: inconclusive, the attempt status is left as it was.
            Response::Unknown => (
                item.router_data.resource_common_data.status,
                Err(build_nmi_error_response(
                    response.into(),
                    item.http_code,
                    None,
                )),
            ),
        };

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
    #[serde(flatten)]
    query: NmiSyncQuery,
}

/// What query.php is keyed by. `merchant_transaction_id` is optional on the Get RPC while
/// `connector_transaction_id` is the required field, so the order reference can be absent.
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum NmiSyncQuery {
    OrderId(String),
    TransactionId(String),
}

impl NmiSyncQuery {
    fn new(
        order_reference: &str,
        connector_transaction_id: &ResponseId,
    ) -> Result<Self, error_stack::Report<IntegrationError>> {
        match connector_transaction_id {
            _ if !order_reference.is_empty() => Ok(Self::OrderId(order_reference.to_string())),
            ResponseId::ConnectorTransactionId(transaction_id) => {
                Ok(Self::TransactionId(transaction_id.clone()))
            }
            ResponseId::EncodedData(_) | ResponseId::NoResponseId => Err(error_stack::report!(
                IntegrationError::MissingRequiredField {
                    field_name: "merchant_transaction_id or connector_transaction_id",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Pass the id returned by NMI or the orderid sent on the payment"
                                .to_string(),
                        ),
                        doc_url: Some("https://docs.nmi.com/reference/query".to_string()),
                        additional_context: None,
                    },
                }
            )),
        }
    }
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

        // PSync is keyed by attempt_id as order_id when the caller has it, otherwise by NMI's
        // transaction id. An empty order_id would match nothing and read as "unknown attempt".
        let query = NmiSyncQuery::new(
            &router_data
                .resource_common_data
                .connector_request_reference_id,
            &router_data.request.connector_transaction_id,
        )?;

        Ok(Self {
            security_key: auth.api_key,
            query,
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
    pub condition: NmiCondition, // Maps to status
}

impl TryFrom<ResponseRouterData<SyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<SyncResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;

        // Select the transaction this attempt refers to (never fall back to an arbitrary
        // element). NMI's query.php is keyed by `order_id` = connector_request_reference_id
        // (docs.nmi.com "Query API"), and one order can carry several transactions. When the
        // attempt already holds NMI's `transactionid` match on it; otherwise (e.g. a 3DS attempt
        // synced before NMI returned a transactionid) match on the echoed `order_id`.
        let transaction = match &item.router_data.request.connector_transaction_id {
            ResponseId::ConnectorTransactionId(requested_transaction_id) => response
                .transaction
                .iter()
                .find(|txn| &txn.transaction_id == requested_transaction_id),
            ResponseId::EncodedData(_) | ResponseId::NoResponseId => {
                let queried_order_id = &item
                    .router_data
                    .resource_common_data
                    .connector_request_reference_id;
                response
                    .transaction
                    .iter()
                    .find(|txn| &txn.order_id == queried_order_id)
            }
        };

        // No matching transaction (empty nm_response, or none of the returned rows is this attempt)
        let (status, transaction_id) = if let Some(transaction) = transaction {
            // Map condition field from XML to AttemptStatus via NmiCondition
            let status = AttemptStatus::from(transaction.condition.clone());
            (status, Some(transaction.transaction_id.clone()))
        } else {
            // NMI has no record of this attempt (empty nm_response or no matching row), so it is telling us nothing about
            // the attempt. Report Unspecified rather than inventing a status -- it reaches the router
            // as PaymentStatus::UNSPECIFIED, which resolves to the attempt's existing status. Claiming
            // AuthenticationPending here happened to suit the 3DS flow (where the attempt already is
            // authentication-pending), but it is wrong for every other way an order goes unknown to
            // NMI -- e.g. an attempt that never reached the connector at all.
            (AttemptStatus::Unspecified, None)
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: transaction_id
                    .map(ResponseId::ConnectorTransactionId)
                    .unwrap_or(ResponseId::NoResponseId),
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
            .convert(&router_data.request.amount_to_capture)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        Ok(Self {
            security_key: auth.api_key,
            transaction_type: TransactionType::Capture,
            transactionid,
            amount,
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

        let (status, capture_response) = match response.response {
            // response=1: capture approved
            Response::Approved => (
                AttemptStatus::Charged,
                Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(response.transactionid.clone()),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: Some(response.orderid.clone()),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                    splits: None,
                    payment_account_reference: None,
                }),
            ),
            // response=2 (declined) / response=3 (error in data, e.g. capture above the
            // authorised amount, or system error): the capture failed, the decline code and
            // text are carried in the error.
            Response::Declined | Response::Error => (
                AttemptStatus::CaptureFailed,
                Err(build_nmi_error_response(
                    response.into(),
                    item.http_code,
                    Some(FlowStatus::Payment(AttemptStatus::CaptureFailed)),
                )),
            ),
            // Undocumented `response` value: inconclusive, the attempt status is left as it was.
            Response::Unknown => (
                item.router_data.resource_common_data.status,
                Err(build_nmi_error_response(
                    response.into(),
                    item.http_code,
                    None,
                )),
            ),
        };

        Ok(Self {
            response: capture_response,
            resource_common_data: PaymentFlowData {
                status,
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

        // Get the refund ID (refund_id) as orderid
        // If refund_id is not present, use connector_request_reference_id as fallback
        let orderid = router_data
            .resource_common_data
            .refund_id
            .clone()
            .unwrap_or_else(|| {
                tracing::debug!("Refund: refund_id not present, using connector_request_reference_id as orderid");
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone()
            });

        // Convert amount from minor to major units using framework converter
        let converter = FloatMajorUnitForConnector;
        let amount = converter
            .convert(&router_data.request.refund_amount)
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

        let status = match response.response {
            // response=1: refund approved
            Response::Approved => RefundStatus::Success,
            // response=2 (declined) / response=3 (error in data, e.g. refund amount above the
            // settled amount, or system error): the refund failed, the decline code and text
            // are carried in the error.
            Response::Declined | Response::Error => {
                return Ok(Self {
                    response: Err(build_nmi_error_response(
                        response.into(),
                        item.http_code,
                        Some(FlowStatus::Refund(RefundStatus::Failure)),
                    )),
                    resource_common_data: RefundFlowData {
                        status: RefundStatus::Failure,
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                })
            }
            // Undocumented `response` value: inconclusive, the status is left as it was.
            Response::Unknown => {
                return Ok(Self {
                    response: Err(build_nmi_error_response(
                        response.into(),
                        item.http_code,
                        None,
                    )),
                    ..item.router_data
                })
            }
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: response.orderid.clone(),
                refund_status: status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
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
    order_id: String, // Uses connector_refund_id
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

        // RSync uses connector_refund_id as order_id (per tech spec section 3.6)
        let order_id = router_data.request.connector_refund_id.clone();

        Ok(Self {
            security_key: auth.api_key,
            order_id,
        })
    }
}

// ===== REFUND SYNC (RSYNC) RESPONSE =====
// Reusing SyncResponse structure as XML format is same (per tech spec section 3.9)

impl TryFrom<ResponseRouterData<SyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<SyncResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;

        let queried_order_id = &item.router_data.request.connector_refund_id;

        // The query is keyed by order_id (= connector_refund_id, the refund's echoed `orderid`;
        // docs.nmi.com "Query API"). Select only the row whose order_id is this refund:
        // an unrelated record must never decide the refund's status.
        let Some(transaction) = response
            .transaction
            .iter()
            .find(|txn| &txn.order_id == queried_order_id)
        else {
            // NMI returned no record of this refund (empty nm_response or no matching row): the
            // query tells us nothing, so this is inconclusive -- attempt_status None and the
            // refund's existing status is left untouched, never a terminal state.
            return Ok(Self {
                response: Err(ErrorResponse {
                    code: common_utils::consts::NO_ERROR_CODE.to_string(),
                    message: format!(
                        "NMI query returned no transaction with order_id {queried_order_id}"
                    ),
                    reason: None,
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..item.router_data
            });
        };

        // Map condition field from XML to RefundStatus via NmiCondition
        let status = RefundStatus::from(transaction.condition.clone());
        let connector_refund_id = transaction.order_id.clone();

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status: status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
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
    void_reason: VoidReason,
    #[serde(skip_serializing_if = "Option::is_none")]
    payment: Option<PaymentType>,
}

#[derive(Debug, Serialize, strum::EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum VoidReason {
    Fraud,
    #[strum(serialize = "user_cancel", serialize = "requested_by_customer")]
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

        // Map cancellation reason to NMI's void reason. An absent reason defaults to
        // user_cancel; an unrecognised one is logged and also defaults to user_cancel.
        let void_reason = match router_data.request.cancellation_reason.as_deref() {
            None => VoidReason::UserCancel,
            Some(reason) => VoidReason::from_str(reason).unwrap_or_else(|_| {
                tracing::warn!(
                    cancellation_reason = reason,
                    "unrecognised NMI void reason, defaulting to user_cancel"
                );
                VoidReason::UserCancel
            }),
        };

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

        let (status, void_response) = match response.response {
            // response=1: void approved
            Response::Approved => (
                AttemptStatus::Voided,
                Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(response.transactionid.clone()),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: Some(response.orderid.clone()),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                    splits: None,
                    payment_account_reference: None,
                }),
            ),
            // response=2 (declined) / response=3 (error in data, e.g. unknown or already
            // settled transaction, or system error): the void failed, the decline code and
            // text are carried in the error.
            Response::Declined | Response::Error => (
                AttemptStatus::VoidFailed,
                Err(build_nmi_error_response(
                    response.into(),
                    item.http_code,
                    Some(FlowStatus::Payment(AttemptStatus::VoidFailed)),
                )),
            ),
            // Undocumented `response` value: inconclusive, the attempt status is left as it was.
            Response::Unknown => (
                item.router_data.resource_common_data.status,
                Err(build_nmi_error_response(
                    response.into(),
                    item.http_code,
                    None,
                )),
            ),
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

/// transact.php `response`: 1 approved, 2 declined, 3 error in transaction data or system error.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Response {
    #[serde(alias = "1")]
    Approved,
    #[serde(alias = "2")]
    Declined,
    #[serde(alias = "3")]
    Error,
    /// A `response` value NMI does not document: inconclusive, never a terminal status.
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
    cavv: Option<String>,
    xid: Option<String>,
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

/// Inputs the Gateway.js 3DS page (`RedirectForm::Nmi`) is built from: the merchant's
/// public tokenization key, the URL Gateway.js posts the 3DS result back to, and the currency
/// of the amount it authenticates (https://docs.nmi.com/docs/payer-authentication-3ds).
fn validate_nmi_three_ds_redirect_inputs<T: PaymentMethodDataTypes>(
    auth: &NmiAuthType,
    request: &PaymentsPreAuthenticateData<T>,
) -> Result<(), error_stack::Report<IntegrationError>> {
    if auth.public_key.is_none() {
        return Err(error_stack::report!(
            IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Configure the NMI public_key (tokenization key) in the connector config"
                            .to_string()
                    ),
                    doc_url: Some("https://docs.nmi.com/docs/payer-authentication-3ds".to_string()),
                    additional_context: Some("public_key required for Gateway.js 3DS".to_string()),
                },
            }
        ));
    }
    if request.continue_redirection_url.is_none() {
        return Err(error_stack::report!(
            IntegrationError::MissingRequiredField {
                field_name: "continue_redirection_url",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Send continue_redirection_url: Gateway.js posts the 3DS result to it"
                            .to_string()
                    ),
                    doc_url: Some("https://docs.nmi.com/docs/payer-authentication-3ds".to_string()),
                    additional_context: None,
                },
            }
        ));
    }
    if request.currency.is_none() {
        return Err(error_stack::report!(
            IntegrationError::MissingRequiredField {
                field_name: "currency",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Send the payment currency: Gateway.js authenticates amount and currency"
                            .to_string()
                    ),
                    doc_url: Some("https://docs.nmi.com/docs/payer-authentication-3ds".to_string()),
                    additional_context: None,
                },
            }
        ));
    }
    Ok(())
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
        // Everything RedirectForm::Nmi needs is checked before the card is vaulted, so a
        // request that cannot produce the Gateway.js page never reaches NMI.
        validate_nmi_three_ds_redirect_inputs(&auth, &router_data.request)?;
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
            state: billing_address.get_optional_state(),
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
            // response=2 (declined) / response=3 (error in data or system error) on HTTP 200
            // (https://docs.nmi.com/reference/getting-started, "Transaction Response Variables").
            Response::Declined | Response::Error => (
                AttemptStatus::Failure,
                Err(build_nmi_error_response(
                    response.into(),
                    item.http_code,
                    Some(FlowStatus::Payment(AttemptStatus::Failure)),
                )),
            ),
            // Undocumented `response` value: inconclusive, the status is left as it was.
            Response::Unknown => (
                item.router_data.resource_common_data.status,
                Err(build_nmi_error_response(
                    response.into(),
                    item.http_code,
                    None,
                )),
            ),
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
    #[serde(skip_serializing_if = "Option::is_none")]
    first_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<common_utils::pii::Email>,
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
    #[serde(flatten)]
    shipping_details: NmiShippingDetails,
}

/// Payment method for SetupMandate - supports Card, ACH, Apple Pay and Google Pay
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum NmiSetupMandatePaymentMethod<T: PaymentMethodDataTypes> {
    Card(NmiSetupMandateCard<T>),
    Ach(NmiSetupMandateAch),
    /// Apple Pay / Google Pay, NMI- or merchant-decrypted; built by `nmi_wallet_payment_method`.
    Wallet(Box<NmiPaymentMethod<T>>),
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

/// NMI SetupMandate response - typed `response` via the shared `Response` enum so
/// the raw "1"/"2"/"3" codes deserialize into `Approved`/`Declined`/`Error` variants.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NmiSetupMandateResponse {
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
            .is_some_and(|amount| amount > common_utils::types::MinorUnit::zero())
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
                wallet_data @ (WalletData::ApplePay(_) | WalletData::GooglePay(_)),
            ) => NmiSetupMandatePaymentMethod::Wallet(Box::new(nmi_wallet_payment_method(
                wallet_data,
            )?)),
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
            first_name: common_data.get_optional_billing_first_name(),
            last_name: common_data.get_optional_billing_last_name(),
            // hyperswitch `get_billing_details` sends the billing-address email and phone,
            // never the customer email; absent -> omitted, no fallback.
            email: common_data.get_optional_billing_email(),
            address1: common_data.get_optional_billing_line1(),
            address2: common_data.get_optional_billing_line2(),
            city: common_data.get_optional_billing_city(),
            state: common_data.get_optional_billing_state(),
            zip: common_data.get_optional_billing_zip(),
            country: common_data.get_optional_billing_country(),
            phone: common_data.get_optional_billing_phone_number(),
            shipping_details: NmiShippingDetails {
                shipping_firstname: common_data.get_optional_shipping_first_name(),
                shipping_lastname: common_data.get_optional_shipping_last_name(),
                shipping_address1: common_data.get_optional_shipping_line1(),
                shipping_address2: common_data.get_optional_shipping_line2(),
                shipping_city: common_data.get_optional_shipping_city(),
                shipping_state: common_data.get_optional_shipping_state(),
                shipping_zip: common_data.get_optional_shipping_zip(),
                shipping_country: common_data.get_optional_shipping_country(),
                shipping_email: common_data.get_optional_shipping_email(),
            },
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

        let mut connector_response = item
            .router_data
            .resource_common_data
            .connector_response
            .clone();
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
                        mandate_metadata: None,
                    })
                });

                // $0 validate on a card: surface the AVS / CVV / authcode results.
                if let PaymentMethodData::Card(_) = item.router_data.request.payment_method_data {
                    connector_response = get_nmi_card_connector_response(
                        response.avsresponse.as_ref(),
                        response.cvvresponse.as_ref(),
                        response.authcode.as_ref(),
                    )
                    .or(connector_response);
                }

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
                        connector_response_reference_id: Some(response.orderid.clone()),
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                        splits: None,
                        payment_account_reference: None,
                    }),
                )
            }
            // response=2 (declined) / response=3 (error in data or system error)
            Response::Declined | Response::Error => (
                AttemptStatus::Failure,
                Err(build_nmi_error_response(
                    response.into(),
                    item.http_code,
                    Some(FlowStatus::Payment(AttemptStatus::Failure)),
                )),
            ),
            // Undocumented `response` value: inconclusive, the status is left as it was.
            Response::Unknown => (
                item.router_data.resource_common_data.status,
                Err(build_nmi_error_response(
                    response.into(),
                    item.http_code,
                    None,
                )),
            ),
        };

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

        // A recurring charge runs against the stored Customer Vault record created by
        // SetupMandate (https://docs.nmi.com/docs/credential-on-file).
        let customer_vault_id = match &router_data.request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(connector_mandate_ids) => connector_mandate_ids
                .get_connector_mandate_id()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "connector_mandate_id",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Pass the customer_vault_id returned by the NMI SetupRecurring call as connector_mandate_id"
                                .to_string(),
                        ),
                        doc_url: Some("https://docs.nmi.com/docs/credential-on-file".to_string()),
                        additional_context: None,
                    },
                })?,
            // NMI documents no network transaction id field: `initial_transaction_id` only
            // accepts NMI's own gateway transaction id, so an NTID-based MIT is refused
            // before any call (https://docs.nmi.com/docs/credential-on-file).
            MandateReferenceId::NetworkMandateId(_)
            | MandateReferenceId::NetworkTokenWithNTI(_) => {
                return Err(error_stack::report!(IntegrationError::NotSupported {
                    message: "Network transaction id based recurring payments".to_string(),
                    connector: "NMI",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Use a connector_mandate_id (NMI Customer Vault id) for NMI recurring payments"
                                .to_string(),
                        ),
                        doc_url: Some("https://docs.nmi.com/docs/credential-on-file".to_string()),
                        additional_context: Some(
                            "NMI documents no network transaction id field; use a Customer Vault mandate"
                                .to_string(),
                        ),
                    },
                }))
            }
        };

        // type=sale vs type=auth from the capture method; ManualMultiple / Scheduled are refused.
        let transaction_type = get_nmi_transaction_type(router_data.request.capture_method)?;

        let amount = convert_nmi_money(&router_data.request.amount, "amount")?;

        let common_data = &router_data.resource_common_data;

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
            first_name: common_data.get_optional_billing_first_name(),
            last_name: common_data.get_optional_billing_last_name(),
            address1: common_data.get_optional_billing_line1(),
            address2: common_data.get_optional_billing_line2(),
            city: common_data.get_optional_billing_city(),
            state: common_data.get_optional_billing_state(),
            zip: common_data.get_optional_billing_zip(),
            country: common_data.get_optional_billing_country(),
            phone: common_data.get_optional_billing_phone_number(),
            // hyperswitch sends only the address emails on every NMI flow, MIT included
            // (`NmiRouterData::get_billing_details` / `get_shipping_details`). A
            // customer-email fallback here makes UCS emit fields the direct path omits.
            email: common_data.get_optional_billing_email(),
            shipping_details: Some(NmiShippingDetails {
                shipping_firstname: common_data.get_optional_shipping_first_name(),
                shipping_lastname: common_data.get_optional_shipping_last_name(),
                shipping_address1: common_data.get_optional_shipping_line1(),
                shipping_address2: common_data.get_optional_shipping_line2(),
                shipping_city: common_data.get_optional_shipping_city(),
                shipping_state: common_data.get_optional_shipping_state(),
                shipping_zip: common_data.get_optional_shipping_zip(),
                shipping_country: common_data.get_optional_shipping_country(),
                shipping_email: common_data.get_optional_shipping_email(),
            }),
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

        let mut connector_response = item
            .router_data
            .resource_common_data
            .connector_response
            .clone();
        let (status, payment_response) = match response.response {
            Response::Approved => {
                // type=sale -> Charged, type=auth -> Authorized; the request derives the type
                // from the same capture method (get_nmi_transaction_type).
                let status = if item.router_data.request.is_auto_capture() {
                    AttemptStatus::Charged
                } else {
                    AttemptStatus::Authorized
                };
                // Vault MIT charges the stored card: surface the AVS / CVV / authcode results.
                connector_response = get_nmi_card_connector_response(
                    response.avsresponse.as_ref(),
                    response.cvvresponse.as_ref(),
                    response.authcode.as_ref(),
                )
                .or(connector_response);
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
                                mandate_metadata: None,
                            })
                        }),
                        connector_metadata: None,
                        network_txn_id: None,
                        network_txn_link_id: None,
                        // Hyperswitch parity: NMI maps connector_response_reference_id to the
                        // merchant `orderid` (echoed back), not the connector `transactionid`
                        // (which is already the resource_id / ConnectorTransactionId above).
                        connector_response_reference_id: Some(response.orderid.clone()),
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                        splits: None,
                        payment_account_reference: None,
                    }),
                )
            }
            // response=2 (declined) / response=3 (error in data or system error)
            Response::Declined | Response::Error => (
                AttemptStatus::Failure,
                Err(build_nmi_error_response(
                    response.into(),
                    item.http_code,
                    Some(FlowStatus::Payment(AttemptStatus::Failure)),
                )),
            ),
            // Undocumented `response` value: inconclusive, the status is left as it was.
            Response::Unknown => (
                item.router_data.resource_common_data.status,
                Err(build_nmi_error_response(
                    response.into(),
                    item.http_code,
                    None,
                )),
            ),
        };

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

// ===== INCOMING WEBHOOK TYPES =====
// Ports the hyperswitch NMI webhook types/behaviour 1:1
// (crates/hyperswitch_connectors/src/connectors/nmi/transformers.rs).

#[derive(Debug, Deserialize)]
pub struct NmiWebhookObjectReference {
    pub event_body: NmiReferenceBody,
}

#[derive(Debug, Deserialize)]
pub struct NmiReferenceBody {
    pub order_id: String,
    pub action: NmiActionBody,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NmiActionBody {
    pub action_type: NmiActionType,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NmiActionType {
    Auth,
    Capture,
    Credit,
    Refund,
    Sale,
    Void,
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
    /// "Batch with chargebacks is completed" — https://docs.nmi.com/reference/chargeback-events
    #[serde(rename = "chargeback.batch.complete")]
    ChargebackBatchComplete,
    /// Payments Sign-Up API "Merchant: New Chargeback" event —
    /// https://docs.nmi.com/reference/sendmerchantchargebackaddedevent
    #[serde(rename = "merchant.chargeback.added")]
    MerchantChargebackAdded,
    /// Any event type not listed above (e.g. `transaction.credit.*`,
    /// `transaction.validate.*` from https://docs.nmi.com/reference/transaction-events).
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NmiWebhookBody {
    pub event_body: NmiWebhookObject,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NmiWebhookObject {
    pub transaction_id: String,
    pub order_id: String,
    pub condition: NmiCondition,
    pub action: NmiActionBody,
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
    pub condition: NmiCondition,
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
        NmiWebhookEventType::SaleUnknown
        | NmiWebhookEventType::RefundUnknown
        | NmiWebhookEventType::AuthUnknown
        | NmiWebhookEventType::VoidUnknown
        | NmiWebhookEventType::CaptureUnknown => EventType::IncomingWebhookEventUnspecified,
        // https://docs.nmi.com/reference/chargeback-events: a completed chargeback batch
        // notifies new chargebacks against the merchant's transactions.
        NmiWebhookEventType::ChargebackBatchComplete => EventType::DisputeOpened,
        // https://docs.nmi.com/reference/sendmerchantchargebackaddedevent: Sign-Up API event,
        // signed with a different scheme and carrying no transaction identity.
        NmiWebhookEventType::MerchantChargebackAdded => EventType::IncomingWebhookEventUnspecified,
        // Unlisted events (transaction.credit.*, transaction.validate.*) are not mapped,
        // matching the hyperswitch `EventNotSupported` behaviour.
        NmiWebhookEventType::Unknown => EventType::IncomingWebhookEventUnspecified,
    }
}

/// `chargeback.batch.complete` webhook body —
/// https://docs.nmi.com/reference/chargeback-events. Chargeback records carry no
/// `order_id`/`transaction_id`; `chargebacks[].id` is NMI's chargeback record id.
#[derive(Debug, Deserialize, Serialize)]
pub struct NmiChargebackBatchWebhook {
    pub event_body: NmiChargebackBatchBody,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NmiChargebackBatchBody {
    pub count: u32,
    pub chargebacks: Vec<NmiChargebackRecord>,
    /// Not in the documented example payload; read when present, never defaulted.
    pub currency: Option<common_enums::Currency>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NmiChargebackRecord {
    pub id: String,
    pub amount: common_utils::types::StringMajorUnit,
    pub reason: Option<String>,
}

impl NmiChargebackBatchBody {
    /// A batch is only mapped to a dispute when it holds exactly one chargeback;
    /// no record is ever chosen by position out of a larger batch.
    pub(crate) fn single_chargeback(
        &self,
    ) -> Result<&NmiChargebackRecord, error_stack::Report<domain_types::errors::WebhookError>> {
        match self.chargebacks.as_slice() {
            [only] => Ok(only),
            records => Err(error_stack::report!(
                domain_types::errors::WebhookError::WebhookResourceObjectNotFound
            )
            .attach_printable(format!(
                "NMI chargeback.batch.complete must carry exactly one chargeback to map to a \
                 dispute; got {} (count {})",
                records.len(),
                self.count
            ))),
        }
    }
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

#[cfg(test)]
mod sync_query_tests {
    use super::*;

    #[test]
    fn prefers_order_reference_then_transaction_id_and_rejects_neither() {
        let transaction_id = ResponseId::ConnectorTransactionId("123".to_string());
        assert_eq!(
            NmiSyncQuery::new("order_1", &transaction_id).ok(),
            Some(NmiSyncQuery::OrderId("order_1".to_string()))
        );
        assert_eq!(
            NmiSyncQuery::new("", &transaction_id).ok(),
            Some(NmiSyncQuery::TransactionId("123".to_string()))
        );
        assert!(NmiSyncQuery::new("", &ResponseId::NoResponseId).is_err());
    }
}
