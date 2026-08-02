use crate::types::ResponseRouterData;
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
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
        ApplePayPaymentData, ApplePayWalletData, BankDebitData, GpayTokenizationData,
        PaymentMethodData, PaymentMethodDataTypes, RawCardNumber, WalletData,
    },
    router_data::{ConnectorSpecificConfig, FlowStatus},
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

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NmiStatus {
    Abandoned,
    Cancelled,
    Pendingsettlement,
    Pending,
    Failed,
    Complete,
    InProgress,
    Unknown,
}

impl From<String> for NmiStatus {
    fn from(value: String) -> Self {
        match value.as_str() {
            "abandoned" => Self::Abandoned,
            "canceled" => Self::Cancelled,
            "in_progress" => Self::InProgress,
            "pendingsettlement" => Self::Pendingsettlement,
            "complete" => Self::Complete,
            "failed" => Self::Failed,
            "unknown" => Self::Unknown,
            // Other than above values only pending is possible, since value is a string handling this as default
            _ => Self::Pending,
        }
    }
}

impl From<NmiStatus> for AttemptStatus {
    fn from(item: NmiStatus) -> Self {
        match item {
            NmiStatus::Abandoned => Self::AuthenticationFailed,
            NmiStatus::Cancelled => Self::Voided,
            NmiStatus::Pending => Self::Authorized,
            NmiStatus::Pendingsettlement | NmiStatus::Complete => Self::Charged,
            NmiStatus::InProgress => Self::AuthenticationPending,
            NmiStatus::Failed | NmiStatus::Unknown => Self::Failure,
        }
    }
}

impl From<NmiStatus> for RefundStatus {
    fn from(item: NmiStatus) -> Self {
        match item {
            NmiStatus::Abandoned
            | NmiStatus::Cancelled
            | NmiStatus::Failed
            | NmiStatus::Unknown => Self::Failure,
            NmiStatus::Pending | NmiStatus::InProgress => Self::Pending,
            NmiStatus::Pendingsettlement | NmiStatus::Complete => Self::Success,
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
    /// Apple Pay, in whichever of the two Direct Post shapes
    /// [`build_apple_pay_payment_data`] produced. Mirrors the Hyperswitch Direct
    /// `PaymentMethod::ApplePayPayment(ApplePayPaymentData)` single variant
    /// (`crates/hyperswitch_connectors/src/connectors/nmi/transformers.rs:654`) so the
    /// encrypted/decrypted split lives in exactly one place and both Authorize and
    /// SetupMandate consume the same value.
    ApplePay(Box<NmiApplePayPaymentData>),
}

// ===== APPLE PAY DATA =====

/// Apple Pay, gateway-decrypted variant (NMI Direct Post "Variant A").
///
/// The PassKit `payment.token.paymentData` blob is forwarded to NMI untouched in
/// `applepay_payment_data` — NMI holds the Apple Pay payment-processing certificate and
/// decrypts it itself. NMI's Direct Post documentation requires the value hex-encoded, and
/// explicitly forbids sending `ccnumber`/`ccexp`/`cvv` alongside it (they are extracted from
/// the token), which is why this struct carries the token and nothing else.
#[derive(Debug, Serialize)]
pub struct ApplePayData {
    applepay_payment_data: Secret<String>,
}

/// Apple Pay, merchant-decrypted variant (NMI Direct Post "Variant B").
///
/// The PassKit token was decrypted upstream, so the device PAN and the network cryptogram
/// travel as discrete form fields, flagged to NMI by `decrypted_applepay_data`. Mirrors the
/// existing [`GooglePayDecryptedData`] twin field-for-field; `cavv` is non-optional here
/// because Apple Pay always yields an `onlinePaymentCryptogram`.
#[derive(Debug, Serialize)]
pub struct ApplePayDecryptedData {
    decrypted_applepay_data: DecryptedDataIndicator,
    ccnumber: Secret<String>,
    ccexp: Secret<String>,
    cavv: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    eci: Option<String>,
}

/// The two mutually exclusive Apple Pay payloads NMI's `transact.php` accepts.
///
/// Deliberately flow-agnostic and serialized untagged, so the flow request enums
/// (`NmiPaymentMethod` for Authorize, `NmiSetupMandatePaymentMethod` for SetupMandate) each
/// hold it behind a single variant and neither re-implements the encrypted/decrypted split.
/// Only [`build_apple_pay_payment_data`] knows the encoding rules. Mirrors the Hyperswitch
/// Direct `ApplePayPaymentData` untagged enum
/// (`crates/hyperswitch_connectors/src/connectors/nmi/transformers.rs:719-724`), which is
/// likewise shared by that connector's Authorize and Validate/SetupMandate requests.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum NmiApplePayPaymentData {
    /// Gateway-decrypted: the hex-encoded PassKit token.
    Encrypted(ApplePayData),
    /// Merchant-decrypted: DPAN + expiry + cryptogram + ECI.
    Decrypted(ApplePayDecryptedData),
}

/// Builds the NMI Apple Pay form fields from the caller's Apple Pay wallet data.
///
/// The single source of truth for the Apple Pay wire shape: every flow that accepts Apple Pay
/// (Authorize `type=sale|auth`, SetupMandate `type=validate`) calls this and wraps the result,
/// so the base64→hex conversion and the expiry/cryptogram extraction cannot drift between them.
///
/// Parity with the Hyperswitch Direct NMI integration
/// (`crates/hyperswitch_connectors/src/connectors/nmi/transformers.rs:1015-1076`), which is
/// likewise shared by its Authorize (`:834-838`) and Validate/SetupMandate (`:1115-1122`) paths:
/// the decrypted branch emits `decrypted_applepay_data=1` plus `ccnumber`/`ccexp`/`cavv`/`eci`,
/// and the encrypted branch base64-decodes the PassKit token and re-encodes it as hex, which
/// is the encoding Direct Post requires (the v5 REST API takes base64 — this connector talks
/// Direct Post).
fn build_apple_pay_payment_data(
    apple_pay_data: &ApplePayWalletData,
) -> Result<NmiApplePayPaymentData, error_stack::Report<IntegrationError>> {
    match &apple_pay_data.payment_data {
        ApplePayPaymentData::Decrypted(decrypted_data) => {
            let ccexp = decrypted_data
                .get_expiry_date_as_mmyy()
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "payment_method.apple_pay.payment_data.decrypted_data.application_expiration_year",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "NMI needs the decrypted Apple Pay expiry as MMYY; the supplied application_expiration_month/application_expiration_year could not be reduced to that form."
                                .to_string(),
                        ),
                        ..Default::default()
                    },
                })
                .attach_printable(
                    "NMI Apple Pay (merchant-decrypted): failed to derive ccexp from the decrypted Apple Pay expiry",
                )?;

            Ok(NmiApplePayPaymentData::Decrypted(ApplePayDecryptedData {
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
            }))
        }
        ApplePayPaymentData::Encrypted(encrypted_data) => {
            if encrypted_data.is_empty() {
                return Err(error_stack::report!(
                    IntegrationError::MissingRequiredField {
                        field_name: "payment_method.apple_pay.payment_data.encrypted_data",
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "NMI requires the base64-encoded Apple Pay PKPaymentToken paymentData for the gateway-decrypted flow; an empty token would reach NMI as an empty applepay_payment_data."
                                    .to_string(),
                            ),
                            ..Default::default()
                        },
                    }
                ));
            }

            // NMI Direct Post: "The value in payment.token.paymentData is a binary (NSData)
            // object, so you must encode it as a hexadecimal string before it can be passed
            // to the Gateway." The wallet SDK hands it to us base64-encoded, so decode first.
            let decoded_apple_pay_data = BASE64_STANDARD
                .decode(encrypted_data)
                .change_context(IntegrationError::InvalidWalletToken {
                    wallet_name: "Apple Pay".to_string(),
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "NMI expects payment_method.apple_pay.payment_data.encrypted_data to be a base64-encoded Apple Pay PKPaymentToken paymentData blob, which NMI receives hex-encoded."
                                .to_string(),
                        ),
                        ..Default::default()
                    },
                })
                .attach_printable(
                    "NMI Apple Pay (gateway-decrypted): encrypted Apple Pay token is not valid base64",
                )?;

            Ok(NmiApplePayPaymentData::Encrypted(ApplePayData {
                applepay_payment_data: Secret::new(hex::encode(decoded_apple_pay_data)),
            }))
        }
    }
}

// ===== GOOGLE PAY DATA =====

#[derive(Debug, Serialize)]
pub struct GooglePayData {
    #[serde(rename = "payment_token")]
    payment_token: Secret<String>,
}

#[derive(Debug, Serialize)]
pub struct GooglePayDecryptedData {
    decrypted_googlepay_data: DecryptedDataIndicator,
    ccnumber: Secret<String>,
    ccexp: Secret<String>,
    cavv: Option<Secret<String>>,
    eci: Option<String>,
}

/// NMI's flag telling `transact.php` that the wallet payload was decrypted upstream, i.e.
/// that `ccnumber`/`ccexp`/`cavv`/`eci` carry the decrypted token rather than the encrypted
/// blob. Serialised as the literal `1` — the value the Hyperswitch Direct NMI integration
/// sends for both `decrypted_googlepay_data` and `decrypted_applepay_data`.
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
    cavv: Option<String>,
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
                    router_data.request.minor_amount,
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
                cavv: three_ds_data.cavv,
                xid: three_ds_data.xid,
                eci: three_ds_data.eci,
                cvv,
                three_ds_version: three_ds_data.three_ds_version,
                directory_server_id: three_ds_data.directory_server_id,
            })
        } else {
            // Regular authorization flow
            let (payment_method, transaction_type) = match &router_data.request.payment_method_data
            {
                PaymentMethodData::BankDebit(bank_debit_data) => {
                    let ach_data = create_ach_data(bank_debit_data, router_data)?;
                    (
                        NmiPaymentMethod::Ach(Box::new(ach_data)),
                        TransactionType::Sale,
                    )
                }
                PaymentMethodData::Wallet(WalletData::GooglePay(google_pay_data)) => {
                    match &google_pay_data.tokenization_data {
                        GpayTokenizationData::Decrypted(decrypted_data) => {
                            let ccexp = decrypted_data.get_expiry_date_as_mmyy().change_context(
                                IntegrationError::RequestEncodingFailed {
                                    context: Default::default(),
                                },
                            )?;
                            (
                                NmiPaymentMethod::GooglePayDecrypt(Box::new(
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
                                )),
                                TransactionType::Sale,
                            )
                        }
                        GpayTokenizationData::Encrypted(encrypted_data) => (
                            NmiPaymentMethod::GooglePay(Box::new(GooglePayData {
                                payment_token: Secret::new(encrypted_data.token.clone()),
                            })),
                            if router_data.request.is_auto_capture() {
                                TransactionType::Sale
                            } else {
                                TransactionType::Auth
                            },
                        ),
                    }
                }
                PaymentMethodData::Wallet(WalletData::ApplePay(apple_pay_data)) => {
                    // NMI accepts `type=auth` as well as `type=sale` for Apple Pay on
                    // `transact.php`. Hyperswitch Direct derives the transaction type from the
                    // capture method once, for every payment method
                    // (`crates/hyperswitch_connectors/src/connectors/nmi/transformers.rs:736-739`),
                    // so both Apple Pay variants honour `is_auto_capture()` here rather than
                    // being pinned to `sale`.
                    let transaction_type = if router_data.request.is_auto_capture() {
                        TransactionType::Sale
                    } else {
                        TransactionType::Auth
                    };

                    (
                        NmiPaymentMethod::ApplePay(Box::new(build_apple_pay_payment_data(
                            apple_pay_data,
                        )?)),
                        transaction_type,
                    )
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
                .convert(
                    router_data.request.minor_amount,
                    router_data.request.currency,
                )
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
                email: None,
                cardholder_auth: None,
                cavv: None,
                xid: None,
                eci: None,
                cvv: None,
                three_ds_version: None,
                directory_server_id: None,
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

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<StandardResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<StandardResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;

        let (status, payment_response) = match response.response {
            Response::Approved => {
                let status = if item.router_data.request.is_auto_capture() {
                    AttemptStatus::Charged
                } else {
                    AttemptStatus::Authorized
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
                    }),
                )
            }
            Response::Declined | Response::Error => (
                AttemptStatus::Failure,
                Err(domain_types::router_data::ErrorResponse {
                    code: response.response_code.clone(),
                    message: response.responsetext.clone(),
                    reason: Some(response.responsetext.clone()),
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
                    connector_transaction_id: Some(response.transactionid.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
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

// ===== PAYMENT SYNC (PSYNC) REQUEST =====

#[derive(Debug, Serialize)]
pub struct NmiSyncRequest {
    security_key: Secret<String>,
    order_id: String, // Uses attempt_id, NOT connector_transaction_id
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

        // PSync uses attempt_id as order_id (NOT connector_transaction_id)
        // The connector_transaction_id contains the attempt_id for sync operations
        let order_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        Ok(Self {
            security_key: auth.api_key,
            order_id,
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
    pub condition: String, // Maps to status
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

        // Find the transaction matching the requested transaction_id
        // If not found, use the most recent one (last in list)
        let transaction = response
            .transaction
            .iter()
            .find(|txn| txn.transaction_id == requested_transaction_id)
            .or_else(|| {
                // Log when using fallback to most recent transaction
                if let Some(last_txn) = response.transaction.last() {
                    tracing::warn!(
                        requested_txn = %requested_transaction_id,
                        fallback_txn = %last_txn.transaction_id,
                        "PSync: Transaction not found in response, using most recent transaction instead"
                    );
                }
                response.transaction.last()
            });

        // Handle empty response (NMI has no record of the order) or transaction data
        let (status, transaction_id) = if let Some(transaction) = transaction {
            // Map condition field from XML to AttemptStatus using NmiStatus enum
            let status = AttemptStatus::from(NmiStatus::from(transaction.condition.clone()));
            (status, Some(transaction.transaction_id.clone()))
        } else {
            // Empty XML response: NMI has no record of this order, so it is telling us nothing about
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
            .convert(
                router_data.request.minor_amount_to_capture,
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

        let status = match response.response {
            Response::Approved => AttemptStatus::Charged,
            Response::Declined | Response::Error => AttemptStatus::Failure,
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
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
            }),
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
            .convert(
                router_data.request.minor_refund_amount,
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

        let status = match response.response {
            Response::Approved => RefundStatus::Success,
            Response::Declined | Response::Error => RefundStatus::Failure,
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: response.orderid.clone(),
                refund_status: status,
                status_code: item.http_code,
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

        // The query is keyed by order_id (= connector_refund_id), so match on the
        // echoed order_id, falling back to the last transaction
        let transaction = response
            .transaction
            .iter()
            .find(|txn| txn.order_id == item.router_data.request.connector_refund_id)
            .or_else(|| response.transaction.last());

        // Map condition field from XML to RefundStatus using NmiStatus enum
        let (status, connector_refund_id) = if let Some(transaction) = transaction {
            let status = RefundStatus::from(NmiStatus::from(transaction.condition.clone()));
            (status, transaction.order_id.clone())
        } else {
            // Empty response - treat as pending with proper error for connector_refund_id
            return Err(error_stack::report!(
                ConnectorError::ResponseDeserializationFailed {
                    context: Default::default(),
                }
            ));
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status: status,
                status_code: item.http_code,
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VoidReason {
    Fraud,
    UserCancel,
    IccRejected,
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

        // Map cancellation reason to NMI's void reason
        let void_reason = router_data
            .request
            .cancellation_reason
            .as_ref()
            .and_then(|reason| match reason.as_str() {
                "fraud" => Some(VoidReason::Fraud),
                "user_cancel" | "requested_by_customer" => Some(VoidReason::UserCancel),
                _ => None,
            })
            .unwrap_or(VoidReason::UserCancel); // Default to UserCancel

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

        let status = match response.response {
            Response::Approved => AttemptStatus::Voided,
            Response::Declined | Response::Error => AttemptStatus::VoidFailed,
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
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
            }),
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
            Response::Approved => {
                let auth_type = NmiAuthType::try_from(&item.router_data.connector_config)
                    .change_context(ConnectorError::ResponseHandlingFailed {
                        context: Default::default(),
                    })?;
                let amount_data = item.router_data.request.amount;
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
            Response::Declined | Response::Error => (
                AttemptStatus::Failure,
                Err(domain_types::router_data::ErrorResponse {
                    code: response.response_code.clone(),
                    message: response.responsetext.clone(),
                    reason: Some(response.responsetext.clone()),
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
                    connector_transaction_id: Some(response.transactionid.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
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
    #[serde(flatten)]
    shipping_details: NmiShippingDetails,
}

/// Payment method for SetupMandate - supports Card, ACH and Apple Pay
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum NmiSetupMandatePaymentMethod<T: PaymentMethodDataTypes> {
    Card(NmiSetupMandateCard<T>),
    Ach(NmiSetupMandateAch),
    /// Apple Pay, produced by the same [`build_apple_pay_payment_data`] the Authorize flow
    /// uses, so a vaulted Apple Pay credential is described to NMI with exactly the fields
    /// an Apple Pay sale would carry. Matches the Hyperswitch Direct
    /// `NmiValidatePaymentData::ApplePayPayment(Box<ApplePayPaymentData>)`
    /// (`crates/hyperswitch_connectors/src/connectors/nmi/transformers.rs:565`).
    ApplePay(Box<NmiApplePayPaymentData>),
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
        if router_data.request.amount.unwrap_or(0) > 0 {
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
            // Apple Pay reuses the Authorize helper verbatim: `type=validate` +
            // `customer_vault=add_customer` stores the Apple Pay credential in NMI's Customer
            // Vault and returns a `customer_vault_id`, which RepeatPayment later replays as
            // `customer_vault_id` on a `type=sale`. Hyperswitch Direct wires Apple Pay into its
            // Validate request the same way
            // (`crates/hyperswitch_connectors/src/connectors/nmi/transformers.rs:1115-1122`).
            PaymentMethodData::Wallet(WalletData::ApplePay(apple_pay_data)) => {
                NmiSetupMandatePaymentMethod::ApplePay(Box::new(build_apple_pay_payment_data(
                    apple_pay_data,
                )?))
            }
            _ => {
                return Err(error_stack::report!(IntegrationError::NotSupported {
                    message: get_unimplemented_payment_method_error_message("NMI SetupMandate"),
                    connector: "NMI",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "NMI SetupMandate (Customer Vault `type=validate`) accepts Card, ACH bank debit and Apple Pay wallet payment methods only."
                                .to_string(),
                        ),
                        ..Default::default()
                    },
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
            email: router_data.request.email.clone(),
            address1: common_data.get_optional_billing_line1(),
            address2: common_data.get_optional_billing_line2(),
            city: common_data.get_optional_billing_city(),
            state: common_data.get_optional_billing_state(),
            zip: common_data.get_optional_billing_zip(),
            country: common_data.get_optional_billing_country(),
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
                    }),
                )
            }
            Response::Declined | Response::Error => (
                AttemptStatus::Failure,
                Err(domain_types::router_data::ErrorResponse {
                    code: response.response_code.clone(),
                    message: response.responsetext.clone(),
                    reason: Some(response.responsetext.clone()),
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
                    connector_transaction_id: Some(response.transactionid.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
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

        let customer_vault_id = match &router_data.request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(connector_mandate_ids) => connector_mandate_ids
                .get_connector_mandate_id()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "connector_mandate_id",
                    context: Default::default(),
                })?,
            _ => {
                return Err(IntegrationError::NotSupported {
                    message: "Only ConnectorMandateId is supported for NMI recurring payments"
                        .to_string(),
                    connector: "NMI",
                    context: Default::default(),
                }
                .into())
            }
        };

        let amount = FloatMajorUnitForConnector
            .convert(
                router_data.request.minor_amount,
                router_data.request.currency,
            )
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        let common_data = &router_data.resource_common_data;

        Ok(Self {
            transaction_type: TransactionType::Sale,
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
            // Prefer the billing-address email (mirrors the hyperswitch reference
            // NMI MIT path), falling back to the top-level `RepeatPaymentData.email`
            // so the customer email is still sent when no billing email is present.
            email: common_data
                .get_optional_billing_email()
                .or_else(|| router_data.request.email.clone()),
            shipping_details: Some(NmiShippingDetails {
                shipping_firstname: common_data.get_optional_shipping_first_name(),
                shipping_lastname: common_data.get_optional_shipping_last_name(),
                shipping_address1: common_data.get_optional_shipping_line1(),
                shipping_address2: common_data.get_optional_shipping_line2(),
                shipping_city: common_data.get_optional_shipping_city(),
                shipping_state: common_data.get_optional_shipping_state(),
                shipping_zip: common_data.get_optional_shipping_zip(),
                shipping_country: common_data.get_optional_shipping_country(),
                // Same precedence for the shipping email: shipping-address email
                // first, then the top-level `RepeatPaymentData.email` fallback.
                shipping_email: common_data
                    .get_optional_shipping_email()
                    .or_else(|| router_data.request.email.clone()),
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

        let (status, payment_response) = match response.response {
            Response::Approved => (
                AttemptStatus::Charged,
                Ok(PaymentsResponseData::TransactionResponse {
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
                    // Hyperswitch parity: NMI maps connector_response_reference_id to the
                    // merchant `orderid` (echoed back), not the connector `transactionid`
                    // (which is already the resource_id / ConnectorTransactionId above).
                    connector_response_reference_id: Some(response.orderid.clone()),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                    splits: None,
                }),
            ),
            Response::Declined | Response::Error => (
                AttemptStatus::Failure,
                Err(domain_types::router_data::ErrorResponse {
                    code: response.response_code.clone(),
                    message: response.responsetext.clone(),
                    reason: Some(response.responsetext.clone()),
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
                    connector_transaction_id: Some(response.transactionid.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
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

/// `event_body.action.action_type` on an NMI webhook.
///
/// NMI's normative OpenAPI enum for this field is 8 values — `auth`, `capture`, `sale`,
/// `void`, `refund`, `credit`, `return`, `validate` — plus 3 further values observed only
/// on check-status events: `settle`, `check_return`, `check_late_return`. This enum models
/// only the 6 actions UCS acts on, so every other value MUST absorb into [`Self::Unknown`]
/// rather than failing deserialization and turning an unmodelled action into an opaque
/// `WebhookResourceObjectNotFound`. Mirrors the HS Direct connector, which carries the same
/// `#[serde(other)] Unknown`
/// (`hyperswitch/crates/hyperswitch_connectors/src/connectors/nmi/transformers.rs:1775-1776`).
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NmiActionType {
    Auth,
    Capture,
    Credit,
    Refund,
    Sale,
    Void,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
pub struct NmiWebhookEventBody {
    pub event_type: NmiWebhookEventType,
}

/// `event_type` on an NMI webhook.
///
/// NMI documents 36 gateway `event_type` values across 6 families. This enum models the 15
/// transaction events UCS acts on; the other 21 — `transaction.credit.*`,
/// `transaction.validate.*`, `transaction.check.status.*`, `recurring.*`,
/// `settlement.batch.*`, `chargeback.batch.complete` and `acu.summary.*` — MUST deserialize
/// to [`Self::Unknown`] and be acknowledged rather than rejected: NMI retries a non-200
/// delivery 20 times over 3 days (<https://docs.nmi.com/reference/retry-logic>), so a single
/// unmodelled event class would otherwise produce a three-day retry storm. Mirrors the HS
/// Direct connector's `#[serde(other)] Unknown`
/// (`hyperswitch/crates/hyperswitch_connectors/src/connectors/nmi/transformers.rs:1816-1817`).
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
    pub condition: String,
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
    pub condition: String,
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
        NmiWebhookEventType::Unknown => {
            tracing::warn!(
                connector = "nmi",
                flow = "Webhooks",
                "Unrecognised NMI webhook event_type received; acknowledging without processing"
            );
            EventType::IncomingWebhookEventUnspecified
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
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
#[allow(clippy::panic)]
mod tests {
    use super::*;
    use domain_types::payment_method_data::{
        ApplePayCryptogramData, ApplePayDecryptedData as DomainApplePayDecryptedData,
        ApplepayPaymentMethod,
    };

    /// The base64 form of the 4-byte payload `0xde 0xad 0xbe 0xef`, i.e. what an Apple Pay SDK
    /// hands us; NMI's Direct Post must receive it hex-encoded as `deadbeef`.
    const SAMPLE_TOKEN_BASE64: &str = "3q2+7w==";
    const SAMPLE_TOKEN_HEX: &str = "deadbeef";

    fn apple_pay_payment_method() -> ApplepayPaymentMethod {
        ApplepayPaymentMethod {
            display_name: "Visa 1111".to_string(),
            network: "Visa".to_string(),
            pm_type: "debit".to_string(),
        }
    }

    fn encrypted_wallet_data(token: &str) -> ApplePayWalletData {
        ApplePayWalletData {
            payment_data: ApplePayPaymentData::Encrypted(token.to_string()),
            payment_method: apple_pay_payment_method(),
            transaction_identifier: "txn_1".to_string(),
        }
    }

    fn decrypted_wallet_data(eci: Option<&str>) -> ApplePayWalletData {
        ApplePayWalletData {
            payment_data: ApplePayPaymentData::Decrypted(DomainApplePayDecryptedData {
                application_primary_account_number: "4111111111111111".parse().expect("card"),
                application_expiration_month: Secret::new("03".to_string()),
                application_expiration_year: Secret::new("2030".to_string()),
                payment_data: ApplePayCryptogramData {
                    online_payment_cryptogram: Secret::new("AAAA".to_string()),
                    eci_indicator: eci.map(str::to_string),
                },
            }),
            payment_method: apple_pay_payment_method(),
            transaction_identifier: "txn_1".to_string(),
        }
    }

    #[test]
    fn encrypted_apple_pay_token_is_base64_decoded_then_hex_encoded() {
        let built = build_apple_pay_payment_data(&encrypted_wallet_data(SAMPLE_TOKEN_BASE64))
            .expect("encrypted apple pay token should build");

        assert_eq!(
            serde_urlencoded::to_string(&built).expect("serialize"),
            format!("applepay_payment_data={SAMPLE_TOKEN_HEX}")
        );
    }

    #[test]
    fn decrypted_apple_pay_emits_the_flag_pan_expiry_cryptogram_and_eci() {
        let built = build_apple_pay_payment_data(&decrypted_wallet_data(Some("05")))
            .expect("decrypted apple pay data should build");

        assert_eq!(
            serde_urlencoded::to_string(&built).expect("serialize"),
            "decrypted_applepay_data=1&ccnumber=4111111111111111&ccexp=0330&cavv=AAAA&eci=05"
        );
    }

    #[test]
    fn decrypted_apple_pay_omits_eci_when_absent() {
        let built = build_apple_pay_payment_data(&decrypted_wallet_data(None))
            .expect("decrypted apple pay data should build");

        let encoded = serde_urlencoded::to_string(&built).expect("serialize");
        assert!(
            !encoded.contains("eci"),
            "an absent eci must be omitted, not sent empty: {encoded}"
        );
    }

    #[test]
    fn empty_encrypted_apple_pay_token_is_rejected() {
        let error = build_apple_pay_payment_data(&encrypted_wallet_data(""))
            .expect_err("an empty token must not reach NMI");

        assert!(
            matches!(
                error.current_context(),
                IntegrationError::MissingRequiredField {
                    field_name: "payment_method.apple_pay.payment_data.encrypted_data",
                    ..
                }
            ),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn non_base64_encrypted_apple_pay_token_is_rejected() {
        let error = build_apple_pay_payment_data(&encrypted_wallet_data("not base64 !!"))
            .expect_err("a non-base64 token must not reach NMI");

        assert!(
            matches!(
                error.current_context(),
                IntegrationError::InvalidWalletToken { wallet_name, .. } if wallet_name == "Apple Pay"
            ),
            "unexpected error: {error:?}"
        );
    }

    /// The anti-drift guarantee this change exists for: Authorize and SetupMandate wrap the very
    /// same [`NmiApplePayPaymentData`], so the Apple Pay form fields NMI receives are identical
    /// on both flows and cannot diverge without this test failing.
    #[test]
    fn authorize_and_setup_mandate_emit_identical_apple_pay_fields() {
        for wallet_data in [
            decrypted_wallet_data(Some("05")),
            encrypted_wallet_data(SAMPLE_TOKEN_BASE64),
        ] {
            let authorize =
                NmiPaymentMethod::<domain_types::payment_method_data::DefaultPCIHolder>::ApplePay(
                    Box::new(
                        build_apple_pay_payment_data(&wallet_data).expect("authorize apple pay"),
                    ),
                );
            let setup_mandate = NmiSetupMandatePaymentMethod::<
                domain_types::payment_method_data::DefaultPCIHolder,
            >::ApplePay(Box::new(
                build_apple_pay_payment_data(&wallet_data).expect("setup mandate apple pay"),
            ));

            assert_eq!(
                serde_urlencoded::to_string(&authorize).expect("serialize authorize"),
                serde_urlencoded::to_string(&setup_mandate).expect("serialize setup mandate"),
            );
        }
    }

    // ===== INCOMING WEBHOOK TESTS =====

    use domain_types::connector_types::EventType;

    /// Deserializes an NMI webhook body and runs it through the event-type mapping, i.e.
    /// exactly what `IncomingWebhook::get_event_type` does.
    fn parse_event_type(body: &str) -> EventType {
        let event_body: NmiWebhookEventBody = serde_json::from_str(body)
            .unwrap_or_else(|error| panic!("event body should deserialize: {error} — {body}"));
        get_nmi_webhook_event(event_body.event_type)
    }

    fn parse_event_type_variant(event_type: &str) -> NmiWebhookEventType {
        let body = format!(r#"{{"event_type":"{event_type}"}}"#);
        serde_json::from_str::<NmiWebhookEventBody>(&body)
            .unwrap_or_else(|error| panic!("event_type {event_type} should deserialize: {error}"))
            .event_type
    }

    fn parse_action_type(action_type: &str) -> NmiActionType {
        let body = format!(r#"{{"action_type":"{action_type}"}}"#);
        serde_json::from_str::<NmiActionBody>(&body)
            .unwrap_or_else(|error| panic!("action_type {action_type} should deserialize: {error}"))
            .action_type
    }

    /// A full NMI `transaction.sale.success` webhook in the shape NMI actually delivers,
    /// including the many `event_body` keys the connector deliberately does not model.
    /// `network_tokenised` drives the only two fields that differ between an Apple Pay-
    /// originated sale and an equivalent raw keyed-card sale.
    fn full_sale_webhook(network_tokenised: bool) -> String {
        format!(
            r#"{{
                "event_type": "transaction.sale.success",
                "event_body": {{
                    "merchant": {{"id": "pmle-1072470", "name": "Test Merchant"}},
                    "transaction_id": "10345678901",
                    "transaction_type": "cc",
                    "condition": "pendingsettlement",
                    "processor_id": "ccprocessora",
                    "order_id": "pay_nmi_wallet_001",
                    "order_description": "Test order",
                    "currency": "USD",
                    "requested_amount": "10.00",
                    "authorization_code": "123456",
                    "card": {{
                        "cc_number": "4xxxxxxxxxxx1111",
                        "cc_exp": "0330",
                        "cc_type": "Visa",
                        "cc_bin": "411111",
                        "entry_mode": "4"
                    }},
                    "action": {{
                        "action_type": "sale",
                        "success": "1",
                        "amount": "10.00",
                        "date": "20260803040000",
                        "network_token_used": {network_tokenised},
                        "network_token_cryptogram_created": {network_tokenised}
                    }}
                }}
            }}"#
        )
    }

    /// Regression test for the gap this change closes: before `#[serde(other)] Unknown`,
    /// each of these hard-failed at `serde_json::from_slice` with `unknown variant`, so
    /// `EventService/ParseEvent` returned a gRPC error and NMI retried the delivery 20 times
    /// over 3 days. `transaction.validate.*` is live traffic today — our own SetupMandate
    /// (including the Apple Pay SetupMandate) submits `type=validate`.
    #[test]
    fn unmodelled_event_types_absorb_into_unknown_and_are_acknowledged() {
        for event_type in [
            "transaction.credit.success",
            "transaction.validate.success",
            "chargeback.batch.complete",
            "settlement.batch.complete",
            "transaction.check.status.settle",
        ] {
            let parsed = parse_event_type_variant(event_type);
            assert!(
                matches!(parsed, NmiWebhookEventType::Unknown),
                "{event_type} should absorb into NmiWebhookEventType::Unknown"
            );
            assert_eq!(
                get_nmi_webhook_event(parsed),
                EventType::IncomingWebhookEventUnspecified,
                "{event_type} should be acknowledged as Unspecified"
            );
        }
    }

    /// Guards the 15 modelled `event_type` strings against the new `#[serde(other)]` arm:
    /// each must still deserialize to its own variant (never `Unknown`) and map to its exact
    /// event type, so a modelled event can never be silently swallowed.
    #[test]
    fn modelled_event_types_are_not_swallowed_by_the_serde_other_arm() {
        for (event_type, expected) in [
            ("transaction.sale.success", EventType::PaymentIntentSuccess),
            ("transaction.sale.failure", EventType::PaymentIntentFailure),
            (
                "transaction.sale.unknown",
                EventType::IncomingWebhookEventUnspecified,
            ),
            (
                "transaction.auth.success",
                EventType::PaymentIntentAuthorizationSuccess,
            ),
            (
                "transaction.auth.failure",
                EventType::PaymentIntentAuthorizationFailure,
            ),
            (
                "transaction.auth.unknown",
                EventType::IncomingWebhookEventUnspecified,
            ),
            (
                "transaction.capture.success",
                EventType::PaymentIntentCaptureSuccess,
            ),
            (
                "transaction.capture.failure",
                EventType::PaymentIntentCaptureFailure,
            ),
            (
                "transaction.capture.unknown",
                EventType::IncomingWebhookEventUnspecified,
            ),
            (
                "transaction.void.success",
                EventType::PaymentIntentCancelled,
            ),
            (
                "transaction.void.failure",
                EventType::PaymentIntentCancelFailure,
            ),
            (
                "transaction.void.unknown",
                EventType::IncomingWebhookEventUnspecified,
            ),
            ("transaction.refund.success", EventType::RefundSuccess),
            ("transaction.refund.failure", EventType::RefundFailure),
            (
                "transaction.refund.unknown",
                EventType::IncomingWebhookEventUnspecified,
            ),
        ] {
            let parsed = parse_event_type_variant(event_type);
            assert!(
                !matches!(parsed, NmiWebhookEventType::Unknown),
                "{event_type} is modelled and must not fall into the serde(other) arm"
            );
            assert_eq!(
                get_nmi_webhook_event(parsed),
                expected,
                "unexpected event type mapping for {event_type}"
            );
        }
    }

    /// `validate`, `return` and `settle` are documented NMI `action.action_type` values that
    /// UCS does not act on; they must absorb into `Unknown` instead of aborting the whole
    /// webhook parse, while the six modelled actions keep their own variants.
    #[test]
    fn unmodelled_action_types_absorb_into_unknown() {
        for action_type in ["validate", "return", "settle"] {
            assert!(
                matches!(parse_action_type(action_type), NmiActionType::Unknown),
                "{action_type} should absorb into NmiActionType::Unknown"
            );
        }

        assert!(matches!(parse_action_type("sale"), NmiActionType::Sale));
        assert!(matches!(parse_action_type("auth"), NmiActionType::Auth));
        assert!(matches!(
            parse_action_type("capture"),
            NmiActionType::Capture
        ));
        assert!(matches!(parse_action_type("void"), NmiActionType::Void));
        assert!(matches!(parse_action_type("refund"), NmiActionType::Refund));
        assert!(matches!(parse_action_type("credit"), NmiActionType::Credit));
    }

    /// THE determination behind "NMI Webhooks / Wallet-ApplePay", asserted rather than
    /// asserted-in-prose: NMI's webhook payload carries no wallet discriminator, so an Apple
    /// Pay-originated sale is indistinguishable from a raw keyed-card sale at every field the
    /// webhook path reads (`event_type`, `order_id`, `transaction_id`, `condition`,
    /// `action.action_type`). This is why **no Apple Pay-specific webhook code exists in this
    /// connector** — any such branch would be unreachable dead code.
    #[test]
    fn apple_pay_and_raw_card_webhooks_parse_identically() {
        // Apple Pay: NMI decrypts the PassKit token to a DPAN, so the transaction is reported
        // as a network-tokenised Visa keyed (`entry_mode` 4) e-commerce sale.
        let apple_pay_webhook = full_sale_webhook(true);
        // Raw keyed card: same entry mode, no network token.
        let raw_card_webhook = full_sale_webhook(false);

        assert_eq!(
            parse_event_type(&apple_pay_webhook),
            parse_event_type(&raw_card_webhook),
            "Apple Pay and raw card must yield the same webhook event type"
        );

        let apple_pay_body: NmiWebhookBody = serde_json::from_str(&apple_pay_webhook)
            .expect("apple pay webhook body should deserialize");
        let raw_card_body: NmiWebhookBody = serde_json::from_str(&raw_card_webhook)
            .expect("raw card webhook body should deserialize");

        assert_eq!(
            apple_pay_body.event_body.order_id,
            raw_card_body.event_body.order_id
        );
        assert_eq!(
            apple_pay_body.event_body.transaction_id,
            raw_card_body.event_body.transaction_id
        );
        assert_eq!(
            apple_pay_body.event_body.condition,
            raw_card_body.event_body.condition
        );
        assert_eq!(
            serde_json::to_value(&apple_pay_body.event_body.action)
                .expect("serialize apple pay action"),
            serde_json::to_value(&raw_card_body.event_body.action)
                .expect("serialize raw card action"),
            "action_type must be identical — NMI reports both as a plain `sale`"
        );
    }

    /// The payment-action resource object must be byte-identical to the PSync envelope HS
    /// Direct emits, since downstream reuses the PSync parser on it.
    #[test]
    fn webhook_sync_response_serialises_to_the_psync_transaction_envelope() {
        let webhook_body: NmiWebhookBody = serde_json::from_str(&full_sale_webhook(true))
            .expect("webhook body should deserialize");

        assert_eq!(
            serde_json::to_string(&NmiWebhookSyncResponse::from(&webhook_body))
                .expect("serialize webhook sync response"),
            r#"{"transaction":{"transaction_id":"10345678901","condition":"pendingsettlement"}}"#
        );
    }

    #[test]
    fn webhook_signature_header_is_split_on_the_last_comma_s() {
        assert_eq!(
            parse_nmi_webhook_signature_header("t=1785000000,s=0a1b2c3d"),
            Some(("1785000000", "0a1b2c3d"))
        );

        // HS Direct uses the greedy regex `r"t=(.*),s=(.*)"`, so a nonce that itself contains
        // `,s=` is split on the LAST occurrence. Reproduced deliberately — a naive
        // `split_once(",s=")` would diverge from the Direct gateway here.
        assert_eq!(
            parse_nmi_webhook_signature_header("t=1785000000,s=notthesignature,s=0a1b2c3d"),
            Some(("1785000000,s=notthesignature", "0a1b2c3d"))
        );

        assert_eq!(parse_nmi_webhook_signature_header("t=1785000000"), None);
        assert_eq!(parse_nmi_webhook_signature_header("malformed"), None);
    }
}
