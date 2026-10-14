use common_enums::{self, enums, AttemptStatus, RefundStatus};
use common_utils::{
    consts,
    pii::Email,
    types::{FloatMajorUnit, MinorUnit},
};
use domain_types::{
    connector_flow::{
        Authorize, CreateConnectorCustomer, PSync, RSync, Refund, RepeatPayment,
        ServerSessionAuthenticationToken, SetupMandate, VoidPC,
    },
    connector_types::{
        ConnectorCustomerData, ConnectorCustomerResponse, MandateReference, MandateReferenceId,
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCancelPostCaptureData,
        PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData, ResponseId,
        ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData,
        SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext, WebhookError},
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_address::Address,
    payment_method_data::{
        ApplePayPaymentData, BankDebitData, DefaultPCIHolder, GpayTokenizationData,
        PaymentMethodData, PaymentMethodDataTypes, RawCardNumber, VaultTokenHolder, WalletData,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
};

use crate::{types::ResponseRouterData, utils::get_unimplemented_payment_method_error_message};
// Alias to make the transition easier
type HsInterfacesConnectorRequestError = IntegrationError;
use std::str::FromStr;

use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, ExposeOptionInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;

use super::AuthorizedotnetRouterData;

type Error = error_stack::Report<IntegrationError>;
type ResponseError = error_stack::Report<ConnectorError>;

// Constants
const MAX_ID_LENGTH: usize = 20;
const ADDRESS_MAX_LENGTH: usize = 60; // Authorize.Net address field max length
const PHONE_MAX_LENGTH: usize = 25; // billTo.phoneNumber max length (API reference field table)
const PO_NUMBER_MAX_LENGTH: usize = 25; // poNumber max length (API reference field table)
const MAX_LINE_ITEMS: usize = 30; // lineItems.lineItem[]: "up to 30" (API reference field table)

// Authorize.Net transient server-side error codes: returned (HTTP 200) with no
// transaction record while the underlying payment is unchanged. On a psync poll
// these are not a payment failure, so we keep the existing state (mirroring HS).
const TRANSIENT_ERROR_SERVER_BUSY: &str = "E00053"; // "server too busy"
const TRANSIENT_ERROR_MAINTENANCE: &str = "E00104"; // "server in maintenance"

// Helper function for concatenating address lines with length constraints
/// Authorize.Net fails the whole request on an over-long field, so a value past the
/// documented limit is omitted rather than sent. Logged, because a field dropped on
/// the way out is otherwise invisible.
fn within_limit<T>(field: &'static str, max: usize, len: usize, value: T) -> Option<T> {
    if len <= max {
        return Some(value);
    }
    tracing::warn!(
        field,
        len,
        max,
        "Field exceeds the Authorize.Net length limit and is omitted from the request"
    );
    None
}

fn get_address_line(
    address_line1: &Option<Secret<String>>,
    address_line2: &Option<Secret<String>>,
    address_line3: &Option<Secret<String>>,
) -> Option<Secret<String>> {
    for lines in [
        vec![address_line1, address_line2, address_line3],
        vec![address_line1, address_line2],
    ] {
        let combined: String = lines
            .into_iter()
            .flatten()
            .map(|s| s.clone().expose())
            .collect::<Vec<_>>()
            .join(" ");

        if !combined.is_empty() && combined.len() <= ADDRESS_MAX_LENGTH {
            return Some(Secret::new(combined));
        }
    }
    address_line1.clone()
}

// Extract credit card payment details from refund metadata
fn get_refund_credit_card_payment(
    connector_metadata: &Option<Secret<serde_json::Value>>,
) -> Result<RefundPaymentDetails, Error> {
    let metadata = connector_metadata
        .as_ref()
        .ok_or_else(|| {
            error_stack::report!(HsInterfacesConnectorRequestError::MissingRequiredField {
                field_name: "connector_metadata",
                context: Default::default()
            })
        })?
        .peek();

    // Extract creditCard field (stringified JSON or JSON object)
    let credit_card_value = metadata.get("creditCard").ok_or_else(|| {
        error_stack::report!(HsInterfacesConnectorRequestError::MissingRequiredField {
            field_name: "creditCard",
            context: Default::default()
        })
    })?;

    let credit_card: serde_json::Value = match credit_card_value {
        serde_json::Value::String(credit_card_str) => serde_json::from_str(credit_card_str)
            .inspect_err(|e| {
                tracing::error!(
                    error = %e,
                    credit_card_str = %credit_card_str,
                    "Failed to parse credit card JSON"
                );
            })
            .change_context(HsInterfacesConnectorRequestError::RequestEncodingFailed {
                context: Default::default(),
            })?,
        serde_json::Value::Object(_) => credit_card_value.clone(),
        _ => {
            return Err(error_stack::report!(
                HsInterfacesConnectorRequestError::MissingRequiredField {
                    field_name: "creditCard",
                    context: Default::default()
                }
            ))
        }
    };

    let card_number = credit_card
        .get("cardNumber")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            error_stack::report!(HsInterfacesConnectorRequestError::MissingRequiredField {
                field_name: "cardNumber",
                context: Default::default()
            })
        })?
        .to_string();

    let expiration_date = credit_card
        .get("expirationDate")
        .and_then(|v| v.as_str())
        .unwrap_or("XXXX")
        .to_string();

    Ok(RefundPaymentDetails {
        credit_card: CreditCardInfo {
            card_number,
            expiration_date,
        },
    })
}

fn get_random_string() -> String {
    common_utils::crypto::generate_cryptographically_secure_random_string(MAX_ID_LENGTH)
}

/// Returns invoice number if length <= MAX_ID_LENGTH, otherwise random string
fn get_invoice_number_or_random(merchant_order_id: Option<String>) -> String {
    match merchant_order_id {
        Some(num) if num.len() <= MAX_ID_LENGTH => num,
        None | Some(_) => get_random_string(),
    }
}

/// Returns customer ID only if length <= MAX_ID_LENGTH
fn validate_customer_id_length(customer_id: Option<String>) -> Option<String> {
    customer_id.filter(|id| id.len() <= MAX_ID_LENGTH)
}

/// Convert metadata to UserFields with optional serialization
fn metadata_to_user_fields(
    metadata: Option<serde_json::Value>,
    needs_serialization: bool,
) -> Result<Option<UserFields>, Error> {
    let meta = match metadata {
        Some(serde_json::Value::Null) | None => return Ok(None),
        Some(m) => m,
    };

    let value = if needs_serialization {
        serde_json::to_value(meta).change_context(IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        })?
    } else {
        meta
    };

    // userFields are name/value pairs, so only a JSON object can be converted. Any
    // other value is rejected (as hyperswitch does) instead of going out as an empty
    // `userFields` element.
    if !value.is_object() {
        return Err(error_stack::report!(IntegrationError::InvalidDataFormat {
            field_name: "metadata",
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Send `metadata` as a JSON object of name/value pairs".to_string(),
                ),
                doc_url: None,
                additional_context: Some(
                    "Authorize.Net `userFields` are name/value pairs, so `metadata` must be a JSON object, not an array or a scalar".to_string(),
                ),
            },
        }));
    }

    Ok(Some(UserFields {
        user_field: Vec::<UserField>::foreign_try_from(value)?,
    }))
}

// // Helper traits for working with generic types
// trait RawCardNumberExt<T: PaymentMethodDataTypes> {
//     fn peek(&self) -> &str;
// }

// trait CardExt<T: PaymentMethodDataTypes> {
//     fn get_expiry_date_as_yyyymm(&self, separator: &str) -> Secret<String>;
// }

// // Implementations for DefaultPCIHolder
// impl RawCardNumberExt<DefaultPCIHolder> for RawCardNumber<DefaultPCIHolder> {
//     fn peek(&self) -> &str {
//         self.0.peek()
//     }
// }

// impl CardExt<DefaultPCIHolder> for domain_types::payment_method_data::Card<DefaultPCIHolder> {
//     fn get_expiry_date_as_yyyymm(&self, separator: &str) -> Secret<String> {
//         Secret::new(format!("{}{}{}",
//             self.card_exp_year.peek(),
//             separator,
//             self.card_exp_month.peek()
//         ))
//     }
// }

// // Implementations for VaultTokenHolder
// impl RawCardNumberExt<VaultTokenHolder> for RawCardNumber<VaultTokenHolder> {
//     fn peek(&self) -> &str {
//         &self.0
//     }
// }

// impl CardExt<VaultTokenHolder> for domain_types::payment_method_data::Card<VaultTokenHolder> {
//     fn get_expiry_date_as_yyyymm(&self, separator: &str) -> Secret<String> {
//         Secret::new(format!("{}{}{}",
//             self.card_exp_year.peek(),
//             separator,
//             self.card_exp_month.peek()
//         ))
//     }
// }

// Wrapper for RawCardNumber to provide construction methods
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AuthorizedotnetRawCardNumber<T: PaymentMethodDataTypes>(pub RawCardNumber<T>);

impl AuthorizedotnetRawCardNumber<DefaultPCIHolder> {
    pub fn from_card_number_string(card_number: String) -> Result<Self, Error> {
        let card_number = cards::CardNumber::from_str(&card_number).change_context(
            IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            },
        )?;
        Ok(Self(RawCardNumber(card_number)))
    }
}

impl AuthorizedotnetRawCardNumber<VaultTokenHolder> {
    pub fn from_token_string(token: Secret<String>) -> Self {
        Self(RawCardNumber(token))
    }
}

// Implement From to convert back to RawCardNumber
impl<T: PaymentMethodDataTypes> From<AuthorizedotnetRawCardNumber<T>> for RawCardNumber<T> {
    fn from(wrapper: AuthorizedotnetRawCardNumber<T>) -> Self {
        wrapper.0
    }
}

// Re-export common enums for use in this file
pub mod api_enums {
    pub use common_enums::Currency;
}

pub trait ForeignTryFrom<F>: Sized {
    type Error;

    fn foreign_try_from(from: F) -> Result<Self, Self::Error>;
}

#[skip_serializing_none]
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MerchantAuthentication {
    name: Secret<String>,
    transaction_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for MerchantAuthentication {
    type Error = Error;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Authorizedotnet {
                name,
                transaction_key,
                ..
            } => Ok(Self {
                name: name.clone(),
                transaction_key: transaction_key.clone(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
            )),
        }
    }
}
impl ForeignTryFrom<serde_json::Value> for Vec<UserField> {
    type Error = Error;
    fn foreign_try_from(metadata: serde_json::Value) -> Result<Self, Self::Error> {
        let mut vector = Self::new();

        if let serde_json::Value::Object(obj) = metadata {
            // Sort keys alphabetically so the outbound userField order is deterministic
            // and matches hyperswitch's native authorizedotnet request (which parses
            // metadata into a BTreeMap<String, Value> for the same purpose), regardless
            // of the insertion/storage order the metadata JSON arrived in.
            let sorted: std::collections::BTreeMap<String, serde_json::Value> =
                obj.into_iter().collect();
            for (key, value) in sorted {
                vector.push(UserField {
                    name: key,
                    value: match value {
                        serde_json::Value::String(s) => s,
                        _ => value.to_string(),
                    },
                });
            }
        }

        Ok(vector)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthorizationType {
    Final,
    Pre,
}

impl TryFrom<enums::CaptureMethod> for AuthorizationType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(capture_method: enums::CaptureMethod) -> Result<Self, Self::Error> {
        match capture_method {
            enums::CaptureMethod::Manual => Ok(Self::Pre),
            enums::CaptureMethod::SequentialAutomatic | enums::CaptureMethod::Automatic => {
                Ok(Self::Final)
            }
            enums::CaptureMethod::ManualMultiple | enums::CaptureMethod::Scheduled => {
                Err(error_stack::report!(IntegrationError::NotSupported {
                    message: "Capture method not supported".to_string(),
                    connector: "authorizedotnet",
                    context: Default::default()
                }))?
            }
        }
    }
}

#[skip_serializing_none]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CreditCardDetails<T: PaymentMethodDataTypes> {
    card_number: RawCardNumber<T>,
    expiration_date: Secret<String>, // YYYY-MM
    card_code: Option<Secret<String>>,
}

/// `payment.creditCard` for a wallet token the merchant side already decrypted
/// (Apple Pay / Google Pay "Hyperswitch decryption"): the device PAN travels as
/// `cardNumber` with `isPaymentToken` and the network `cryptogram`; no `cardCode`.
/// Shape from the XSD `creditCardType`, whose element order is cardNumber,
/// expirationDate, cardCode, isPaymentToken, cryptogram - keep the fields in
/// that order. No ECI is sent: the in-app guide forbids 3-D Secure data next to
/// wallet payment data and `tokenRequestorEci` has no documented meaning.
#[skip_serializing_none]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DecryptedWalletCardDetails {
    card_number: cards::CardNumber,
    expiration_date: Secret<String>, // YYYY-MM
    is_payment_token: Option<bool>,
    cryptogram: Option<Secret<String>>,
}

#[skip_serializing_none]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BankAccountDetails {
    account_type: AccountType,
    routing_number: Secret<String>,
    account_number: Secret<String>,
    name_on_account: Secret<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum PaymentDetails<T: PaymentMethodDataTypes> {
    CreditCard(CreditCardDetails<T>),
    BankAccount(BankAccountDetails),
    OpaqueData(WalletDetails),
    /// Serialised under the same `creditCard` key as `CreditCard`; the type only
    /// differs because a device PAN is not the generic `RawCardNumber<T>`.
    #[serde(rename(serialize = "creditCard", deserialize = "decryptedWalletCreditCard"))]
    DecryptedWalletCard(DecryptedWalletCardDetails),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WalletDetails {
    pub data_descriptor: WalletMethod,
    pub data_value: Secret<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum WalletMethod {
    #[serde(rename = "COMMON.GOOGLE.INAPP.PAYMENT")]
    Googlepay,
    #[serde(rename = "COMMON.APPLE.INAPP.PAYMENT")]
    Applepay,
}

/// The only wallets Authorize.Net accepts are Google Pay and Apple Pay, both as
/// PSP-decryption `opaqueData` passthrough: Authorize.Net decrypts the token
/// itself, so the encrypted blob is forwarded untouched. PayPal is a redirect
/// wallet and stays unimplemented -- the response side cannot surface the
/// approval URL yet (`SecureAcceptance` is an empty stub and `redirection_data`
/// is always `None`), so accepting the request would strand the payment.
impl<T: PaymentMethodDataTypes> TryFrom<&WalletData> for PaymentDetails<T> {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(wallet_data: &WalletData) -> Result<Self, Self::Error> {
        match wallet_data {
            WalletData::GooglePay(_) => Ok(Self::OpaqueData(WalletDetails {
                data_descriptor: WalletMethod::Googlepay,
                data_value: Secret::new(wallet_data.get_encoded_wallet_token()?),
            })),
            WalletData::ApplePay(applepay_token) => {
                let apple_pay_encrypted_data = applepay_token
                    .payment_data
                    .get_encrypted_apple_pay_payment_data_mandatory()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "Apple pay encrypted data",
                        context: Default::default(),
                    })?;
                Ok(Self::OpaqueData(WalletDetails {
                    data_descriptor: WalletMethod::Applepay,
                    data_value: Secret::new(apple_pay_encrypted_data.clone()),
                }))
            }
            WalletData::PaypalRedirect(_)
            | WalletData::AliPayQr(_)
            | WalletData::AliPayRedirect(_)
            | WalletData::AliPayHkRedirect(_)
            | WalletData::BluecodeRedirect {}
            | WalletData::AmazonPayRedirect(_)
            | WalletData::MomoRedirect(_)
            | WalletData::KakaoPayRedirect(_)
            | WalletData::GoPayRedirect(_)
            | WalletData::GcashRedirect(_)
            | WalletData::ApplePayRedirect(_)
            | WalletData::ApplePayThirdPartySdk(_)
            | WalletData::DanaRedirect {}
            | WalletData::GrabpayRedirect {}
            | WalletData::GooglePayRedirect(_)
            | WalletData::GooglePayThirdPartySdk(_)
            | WalletData::MbWayRedirect(_)
            | WalletData::MobilePayRedirect(_)
            | WalletData::PaypalSdk(_)
            | WalletData::Paze(_)
            | WalletData::SamsungPay(_)
            | WalletData::TwintRedirect {}
            | WalletData::VippsRedirect {}
            | WalletData::TouchNGoRedirect(_)
            | WalletData::WeChatPayRedirect(_)
            | WalletData::WeChatPayQr(_)
            | WalletData::CashappQr(_)
            | WalletData::SwishQr(_)
            | WalletData::Mifinity(_)
            | WalletData::RevolutPay(_)
            | WalletData::MbWay(_)
            | WalletData::Satispay(_)
            | WalletData::Wero(_)
            | WalletData::LazyPayRedirect(_)
            | WalletData::PhonePeRedirect(_)
            | WalletData::BillDeskRedirect(_)
            | WalletData::CashfreeRedirect(_)
            | WalletData::PayURedirect(_)
            | WalletData::EaseBuzzRedirect(_)
            | WalletData::PaymayaRedirect(_)
            | WalletData::PayhereRedirect {}
            | WalletData::QwikcilverWalletDirect(_)
            | WalletData::Skrill(_)
            | WalletData::Neteller(_) => {
                Err(error_stack::report!(IntegrationError::NotImplemented(
                    get_unimplemented_payment_method_error_message("authorizedotnet"),
                    Default::default(),
                )))
            }
        }
    }
}

/// Resolves a wallet token that arrives already decrypted into `payment.creditCard`.
/// Returns `None` for an encrypted token (and for every other wallet), which
/// stays on the `opaqueData` path above.
///
/// - Apple Pay decrypted data always carries an online payment cryptogram.
/// - Google Pay decrypted data carries one for `CRYPTOGRAM_3DS` tokens only; a
///   `PAN_ONLY` token has none and is a plain card, so `isPaymentToken` is omitted.
fn get_decrypted_wallet_card(
    wallet_data: &WalletData,
) -> Result<Option<DecryptedWalletCardDetails>, Error> {
    match wallet_data {
        WalletData::ApplePay(apple_pay) => match &apple_pay.payment_data {
            ApplePayPaymentData::Decrypted(decrypted) => Ok(Some(DecryptedWalletCardDetails {
                card_number: decrypted.application_primary_account_number.clone(),
                expiration_date: decrypted.get_expiry_date_as_yyyymm("-"),
                is_payment_token: Some(true),
                cryptogram: Some(decrypted.payment_data.online_payment_cryptogram.clone()),
            })),
            ApplePayPaymentData::Encrypted(_) => Ok(None),
        },
        WalletData::GooglePay(google_pay) => match &google_pay.tokenization_data {
            GpayTokenizationData::Decrypted(decrypted) => {
                let expiration_date = decrypted.get_expiry_date_as_yyyymm("-").change_context(
                    IntegrationError::InvalidDataFormat {
                        field_name: "google_pay.tokenization_data.card_exp_month/card_exp_year",
                        context: IntegrationErrorContext {
                            suggested_action: Some(
                                "Send the decrypted Google Pay expiry as a 1-12 month and a 2 or 4 digit year"
                                    .to_owned(),
                            ),
                            doc_url: None,
                            additional_context: Some(
                                "Authorize.Net expects creditCard.expirationDate as YYYY-MM"
                                    .to_owned(),
                            ),
                        },
                    },
                )?;
                Ok(Some(DecryptedWalletCardDetails {
                    card_number: decrypted.application_primary_account_number.clone(),
                    expiration_date,
                    is_payment_token: decrypted.cryptogram.as_ref().map(|_| true),
                    cryptogram: decrypted.cryptogram.clone(),
                }))
            }
            GpayTokenizationData::Encrypted(_) => Ok(None),
        },
        WalletData::PaypalRedirect(_)
        | WalletData::AliPayQr(_)
        | WalletData::AliPayRedirect(_)
        | WalletData::AliPayHkRedirect(_)
        | WalletData::BluecodeRedirect {}
        | WalletData::AmazonPayRedirect(_)
        | WalletData::MomoRedirect(_)
        | WalletData::KakaoPayRedirect(_)
        | WalletData::GoPayRedirect(_)
        | WalletData::GcashRedirect(_)
        | WalletData::ApplePayRedirect(_)
        | WalletData::ApplePayThirdPartySdk(_)
        | WalletData::DanaRedirect {}
        | WalletData::GrabpayRedirect {}
        | WalletData::GooglePayRedirect(_)
        | WalletData::GooglePayThirdPartySdk(_)
        | WalletData::MbWayRedirect(_)
        | WalletData::MobilePayRedirect(_)
        | WalletData::PaypalSdk(_)
        | WalletData::Paze(_)
        | WalletData::SamsungPay(_)
        | WalletData::TwintRedirect {}
        | WalletData::VippsRedirect {}
        | WalletData::TouchNGoRedirect(_)
        | WalletData::WeChatPayRedirect(_)
        | WalletData::WeChatPayQr(_)
        | WalletData::CashappQr(_)
        | WalletData::SwishQr(_)
        | WalletData::Mifinity(_)
        | WalletData::RevolutPay(_)
        | WalletData::MbWay(_)
        | WalletData::Satispay(_)
        | WalletData::Wero(_)
        | WalletData::LazyPayRedirect(_)
        | WalletData::PhonePeRedirect(_)
        | WalletData::BillDeskRedirect(_)
        | WalletData::CashfreeRedirect(_)
        | WalletData::PayURedirect(_)
        | WalletData::EaseBuzzRedirect(_)
        | WalletData::PaymayaRedirect(_)
        | WalletData::PayhereRedirect {}
        | WalletData::QwikcilverWalletDirect(_)
        | WalletData::Skrill(_)
        | WalletData::Neteller(_) => Ok(None),
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum AccountType {
    Checking,
    Savings,
    BusinessChecking,
}

impl
    TryFrom<(
        Option<common_enums::BankType>,
        Option<common_enums::BankHolderType>,
    )> for AccountType
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        (bank_type, bank_holder_type): (
            Option<common_enums::BankType>,
            Option<common_enums::BankHolderType>,
        ),
    ) -> Result<Self, Self::Error> {
        match bank_type {
            Some(common_enums::BankType::Savings) => Ok(Self::Savings),
            Some(common_enums::BankType::Checking) | None => {
                if let Some(common_enums::BankHolderType::Business) = bank_holder_type {
                    Ok(Self::BusinessChecking)
                } else {
                    Ok(Self::Checking)
                }
            }
            Some(bank) => Err(error_stack::report!(IntegrationError::NotSupported {
                message: format!("Bank type {bank:?} is not supported by authorizedotnet"),
                connector: "authorizedotnet",
                context: IntegrationErrorContext {
                    suggested_action: Some("Provide a valid bank account type".to_owned()),
                    additional_context: None,
                    doc_url: None,
                },
            })),
        }
    }
}

#[skip_serializing_none]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TransactionType {
    AuthOnlyTransaction,
    AuthCaptureTransaction,
    PriorAuthCaptureTransaction,
    VoidTransaction,
    RefundTransaction,
}

#[skip_serializing_none]
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    invoice_number: String,
    description: String,
}

#[skip_serializing_none]
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BillTo {
    first_name: Option<Secret<String>>,
    last_name: Option<Secret<String>>,
    address: Option<Secret<String>>,
    city: Option<Secret<String>>,
    state: Option<Secret<String>>,
    zip: Option<Secret<String>>,
    country: Option<enums::CountryAlpha2>,
    // XSD `customerAddressType` order: ..., country, phoneNumber, faxNumber, email.
    phone_number: Option<Secret<String>>,
    email: Option<Email>,
}

/// `billTo` from a billing address. Shared by every createTransactionRequest that
/// sends card data with a billing address, so the shape cannot drift per flow.
impl From<&Address> for BillTo {
    fn from(billing: &Address) -> Self {
        let details = billing.address.as_ref();
        Self {
            first_name: details.and_then(|a| a.first_name.clone()),
            last_name: details.and_then(|a| a.last_name.clone()),
            // line1, line2 and line3 joined under the 60-character limit
            address: details.and_then(|a| get_address_line(&a.line1, &a.line2, &a.line3)),
            city: details.and_then(|a| a.city.clone()),
            state: details.and_then(|a| a.state.clone()),
            zip: details.and_then(|a| a.zip.clone()),
            country: details.and_then(|a| a.country),
            // Country code + number. `phoneNumber` is limited to 25 characters and an
            // over-long value fails the whole request on schema validation, so it is
            // omitted instead - the same rule `refId` and `customer.id` follow.
            phone_number: billing.get_e123_phone_number().and_then(|phone| {
                within_limit(
                    "billTo.phoneNumber",
                    PHONE_MAX_LENGTH,
                    phone.peek().len(),
                    phone,
                )
            }),
            email: billing.email.clone(),
        }
    }
}

#[skip_serializing_none]
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShipTo {
    first_name: Option<Secret<String>>,
    last_name: Option<Secret<String>>,
    address: Option<Secret<String>>,
    city: Option<Secret<String>>,
    state: Option<Secret<String>>,
    zip: Option<Secret<String>>,
    country: Option<enums::CountryAlpha2>,
}

/// `shipTo` from the shipping address. Prefer `ShipTo::from_address`, which keeps an
/// all-`None` `shipTo` off the wire.
impl From<&Address> for ShipTo {
    fn from(shipping: &Address) -> Self {
        let details = shipping.address.as_ref();
        Self {
            first_name: details.and_then(|a| a.first_name.clone()),
            last_name: details.and_then(|a| a.last_name.clone()),
            // Same 60-character joining rule as billTo.address
            address: details.and_then(|a| get_address_line(&a.line1, &a.line2, &a.line3)),
            city: details.and_then(|a| a.city.clone()),
            state: details.and_then(|a| a.state.clone()),
            zip: details.and_then(|a| a.zip.clone()),
            country: details.and_then(|a| a.country),
        }
    }
}

impl ShipTo {
    /// `shipTo` for an address that carries something to send, `None` otherwise - an
    /// empty `shipTo {}` must never reach the wire. Call sites use this rather than
    /// `ShipTo::from` for exactly that reason.
    fn from_address(shipping: &Address) -> Option<Self> {
        let ship_to = Self::from(shipping);
        ship_to.has_content().then_some(ship_to)
    }

    /// Whether the address carried anything to send.
    fn has_content(&self) -> bool {
        self.first_name.is_some()
            || self.last_name.is_some()
            || self.address.is_some()
            || self.city.is_some()
            || self.state.is_some()
            || self.zip.is_some()
            || self.country.is_some()
    }
}

/// Level 3 line items: `lineItems.lineItem[]`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LineItems {
    line_item: Vec<LineItem>,
}

/// One Level 3 line item. Field order follows the XSD `lineItemType`: itemId, name,
/// description, quantity, unitPrice, ..., unitOfMeasure, ..., taxAmount, ...,
/// totalAmount, commodityCode.
#[skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LineItem {
    item_id: String,
    name: String,
    description: Option<String>,
    quantity: u16,
    unit_price: FloatMajorUnit,
    unit_of_measure: Option<String>,
    tax_amount: Option<FloatMajorUnit>,
    total_amount: Option<FloatMajorUnit>,
    commodity_code: Option<String>,
}

/// Level 2 `tax` / `duty` / `shipping`: only `amount` is sent.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtendedAmount {
    amount: FloatMajorUnit,
}

#[skip_serializing_none]
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomerDetails {
    id: String,
    email: Option<Email>,
}

#[skip_serializing_none]
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserField {
    name: String,
    value: String,
}

#[skip_serializing_none]
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserFields {
    user_field: Vec<UserField>,
}

#[skip_serializing_none]
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessingOptions {
    is_subsequent_auth: bool,
}

#[skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubsequentAuthInformation {
    original_network_trans_id: Secret<String>,
    reason: Reason,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Reason {
    Resubmission,
    #[serde(rename = "delayedCharge")]
    DelayedCharge,
    Reauthorization,
    #[serde(rename = "noShow")]
    NoShow,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ProfileDetails {
    CreateProfileDetails(CreateProfileDetails),
    CustomerProfileDetails(CustomerProfileDetails),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProfileDetails {
    create_profile: bool,
    customer_profile_id: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomerProfileDetails {
    customer_profile_id: Secret<String>,
    payment_profile: PaymentProfileDetails,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentProfileDetails {
    payment_profile_id: Secret<String>,
}

#[skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetTransactionRequest<T: PaymentMethodDataTypes> {
    // General structure for transaction details in Authorize
    transaction_type: TransactionType,
    amount: Option<FloatMajorUnit>,
    currency_code: Option<api_enums::Currency>,
    payment: Option<PaymentDetails<T>>,
    profile: Option<ProfileDetails>,
    order: Option<Order>,
    // Authorize.Net validates the element order of `transactionRequest` against its
    // XSD (`transactionRequestType`); a field in the wrong position is rejected with
    // E00003. Keep: order, lineItems, tax, duty, shipping, taxExempt, poNumber,
    // customer, billTo, shipTo, cardholderAuthentication, userFields,
    // merchantDescriptor, processingOptions, subsequentAuthInformation.
    line_items: Option<LineItems>,
    tax: Option<ExtendedAmount>,
    duty: Option<ExtendedAmount>,
    shipping: Option<ExtendedAmount>,
    tax_exempt: Option<bool>,
    po_number: Option<String>,
    customer: Option<CustomerDetails>,
    bill_to: Option<BillTo>,
    ship_to: Option<ShipTo>,
    cardholder_authentication: Option<CardholderAuthentication>,
    user_fields: Option<UserFields>,
    merchant_descriptor: Option<String>,
    processing_options: Option<ProcessingOptions>,
    subsequent_auth_information: Option<SubsequentAuthInformation>,
    ref_trans_id: Option<String>,
}

/// `transactionRequest.cardholderAuthentication`: the result of a 3D Secure
/// authentication performed outside Authorize.Net (the gateway has no enrolment,
/// challenge or validation call of its own). Both elements are required when the
/// element is sent.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardholderAuthentication {
    /// ECI (Visa) or UCAF collection indicator (Mastercard)
    authentication_indicator: String,
    /// CAVV (Visa) or AAV / UCAF (Mastercard)
    cardholder_authentication_value: Secret<String>,
}

/// Builds `cardholderAuthentication` for a card payment from externally obtained
/// 3DS data, refusing before any connector call when the data cannot be honoured:
/// - a 3DS payment with no authentication data: Authorize.Net cannot authenticate
///   the cardholder, and charging it anyway would silently downgrade it to non-3DS;
/// - authentication data carrying only one of indicator / CAVV: the gateway
///   requires both elements together.
fn build_cardholder_authentication(
    authentication_data: Option<&domain_types::router_request_types::AuthenticationData>,
    is_three_ds: bool,
) -> Result<Option<CardholderAuthentication>, Error> {
    // authenticationIndicator is "ECI (Visa) or UCAF collection indicator
    // (Mastercard)": the ECI is used when present, the UCAF indicator otherwise.
    let authentication_indicator = authentication_data.and_then(|data| {
        data.eci
            .as_ref()
            .filter(|eci| !eci.is_empty())
            .or(data
                .ucaf_collection_indicator
                .as_ref()
                .filter(|indicator| !indicator.is_empty()))
            .cloned()
    });
    let cardholder_authentication_value = authentication_data
        .and_then(|data| data.cavv.clone())
        .filter(|cavv| !cavv.peek().is_empty());

    match (authentication_indicator, cardholder_authentication_value) {
        (Some(authentication_indicator), Some(cardholder_authentication_value)) => {
            Ok(Some(CardholderAuthentication {
                authentication_indicator,
                cardholder_authentication_value,
            }))
        }
        (Some(_), None) => Err(error_stack::report!(IntegrationError::MissingRequiredField {
            field_name: "authentication_data.cavv",
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Send authentication_data.cavv together with the ECI, or omit authentication_data"
                        .to_owned(),
                ),
                doc_url: None,
                additional_context: Some(
                    "Authorize.Net requires both authenticationIndicator and cardholderAuthenticationValue when cardholderAuthentication is sent"
                        .to_owned(),
                ),
            },
        })),
        (None, Some(_)) => Err(error_stack::report!(IntegrationError::MissingRequiredField {
            field_name: "authentication_data.eci",
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Send authentication_data.eci (or ucaf_collection_indicator for Mastercard) together with the CAVV, or omit authentication_data"
                        .to_owned(),
                ),
                doc_url: None,
                additional_context: Some(
                    "Authorize.Net requires both authenticationIndicator and cardholderAuthenticationValue when cardholderAuthentication is sent"
                        .to_owned(),
                ),
            },
        })),
        (None, None) if is_three_ds => Err(error_stack::report!(IntegrationError::NotSupported {
            message: "3DS flow".to_string(),
            connector: "authorizedotnet",
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Authenticate the cardholder with an external 3DS provider and send the result in authentication_data (eci and cavv), or use auth_type no_three_ds"
                        .to_owned(),
                ),
                doc_url: None,
                additional_context: Some(
                    "Authorize.Net performs no 3D Secure authentication of its own; it only accepts externally obtained authentication data on the charge"
                        .to_owned(),
                ),
            },
        })),
        (None, None) => Ok(None),
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionSettings {
    setting: Vec<TransactionSetting>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionSetting {
    setting_name: String,
    setting_value: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTransactionRequest<T: PaymentMethodDataTypes> {
    // Used by Authorize Flow, wraps the general transaction request
    merchant_authentication: AuthorizedotnetAuthType,
    ref_id: Option<String>,
    transaction_request: AuthorizedotnetTransactionRequest<T>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetPaymentsRequest<T: PaymentMethodDataTypes> {
    // Top-level wrapper for Authorize Flow
    create_transaction_request: CreateTransactionRequest<T>,
}

// Implementation for owned RouterData that doesn't depend on reference version
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AuthorizedotnetRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AuthorizedotnetPaymentsRequest<T>
{
    type Error = Error;
    fn try_from(
        item: AuthorizedotnetRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let merchant_authentication =
            AuthorizedotnetAuthType::try_from(&item.router_data.connector_config)?;

        let currency_str = item.router_data.request.currency.to_string();
        let currency = api_enums::Currency::from_str(&currency_str).map_err(|_| {
            error_stack::report!(IntegrationError::RequestEncodingFailed {
                context: Default::default()
            })
        })?;

        // Always create regular transaction request (mandate logic moved to RepeatPayment flow)
        let transaction_request = create_regular_transaction_request(&item, currency)?;

        let ref_id = Some(
            item.router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        )
        .filter(|id| id.len() <= MAX_ID_LENGTH);
        let create_transaction_request = CreateTransactionRequest {
            merchant_authentication,
            ref_id,
            transaction_request,
        };

        Ok(Self {
            create_transaction_request,
        })
    }
}

// Helper function to create regular transaction request (non-mandate)
fn create_regular_transaction_request<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: &AuthorizedotnetRouterData<
        RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        T,
    >,
    currency: api_enums::Currency,
) -> Result<AuthorizedotnetTransactionRequest<T>, Error> {
    // External 3DS data is carried on card payments only (never on the wallet or
    // bank arms: Apple Pay opaque data may not include 3D Secure data).
    let mut cardholder_authentication = None;
    let payment_details = match &item.router_data.request.payment_method_data {
        PaymentMethodData::Card(card) => {
            // Refusals come first, before any field of the request is built.
            cardholder_authentication = build_cardholder_authentication(
                item.router_data.request.authentication_data.as_ref(),
                item.router_data.resource_common_data.is_three_ds(),
            )?;

            let expiry_month = card.card_exp_month.peek().clone();
            let expiry_year =
                domain_types::utils::expand_expiry_year_to_four_digits(&card.card_exp_year);
            let expiration_date = format!("{}-{expiry_month}", expiry_year.peek());

            let credit_card_details = CreditCardDetails {
                card_number: card.card_number.clone(),
                expiration_date: Secret::new(expiration_date),
                card_code: Some(card.card_cvc.clone()),
            };

            Ok(PaymentDetails::CreditCard(credit_card_details))
        }
        PaymentMethodData::BankDebit(bank_debit_data) => {
            match bank_debit_data {
                BankDebitData::AchBankDebit {
                    account_number,
                    routing_number,
                    bank_account_holder_name,
                    card_holder_name,
                    bank_type,
                    bank_holder_type,
                    ..
                } => {
                    // Get account holder name from bank_account_holder_name, card_holder_name,
                    // or billing address
                    let name_on_account = bank_account_holder_name
                        .clone()
                        .or_else(|| card_holder_name.clone())
                        .or_else(|| {
                            item.router_data
                                .resource_common_data
                                .get_optional_billing_full_name()
                        })
                        .ok_or_else(|| {
                            error_stack::report!(IntegrationError::MissingRequiredField {
                                field_name: "bank_account_holder_name",
                context: Default::default()
                            })
                        })?;

                    // Map bank_type and bank_holder_type to AccountType
                    // Business accounts with checking should use BusinessChecking
                    let account_type =
                        AccountType::try_from((*bank_type, *bank_holder_type))?;

                    let bank_account_details = BankAccountDetails {
                        account_type,
                        routing_number: routing_number.clone(),
                        account_number: account_number.clone(),
                        name_on_account
};

                    Ok(PaymentDetails::BankAccount(bank_account_details))
                }
                BankDebitData::SepaBankDebit { .. }
                | BankDebitData::SepaGuaranteedBankDebit { .. }
                | BankDebitData::BecsBankDebit { .. }
                | BankDebitData::EftBankDebit { .. }
                | BankDebitData::BacsBankDebit { .. } => {
                    Err(error_stack::report!(IntegrationError::NotSupported {
                        message: "SEPA, SEPA Guaranteed, BECS, EFT, and BACS bank debits are not supported for authorizedotnet"
                            .to_string(),
                        connector: "authorizedotnet",
                        context: Default::default(),
                    }))
                }
            }
        }
        PaymentMethodData::Wallet(wallet_data) => match get_decrypted_wallet_card(wallet_data)? {
            Some(decrypted_wallet_card) => {
                Ok(PaymentDetails::DecryptedWalletCard(decrypted_wallet_card))
            }
            None => PaymentDetails::try_from(wallet_data),
        },
        pm => Err(error_stack::report!(IntegrationError::NotSupported {
            message: format!("Payment method {:?}", pm),
            connector: "authorizedotnet",
            context: Default::default(),
        })),
    }?;

    let transaction_type = match item.router_data.request.capture_method {
        Some(enums::CaptureMethod::Manual) => TransactionType::AuthOnlyTransaction,
        Some(enums::CaptureMethod::Automatic)
        | None
        | Some(enums::CaptureMethod::SequentialAutomatic) => {
            TransactionType::AuthCaptureTransaction
        }
        // Single capture only: authOnlyTransaction or authCaptureTransaction. Refused
        // for every payment method arm before the connector is called.
        Some(enums::CaptureMethod::ManualMultiple) | Some(enums::CaptureMethod::Scheduled) => {
            return Err(error_stack::report!(IntegrationError::NotSupported {
                message: "Capture method not supported".to_string(),
                connector: "authorizedotnet",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Use capture_method automatic or manual (a single capture)".to_owned(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "Authorize.Net supports authCaptureTransaction and authOnlyTransaction with one priorAuthCaptureTransaction; multiple and scheduled captures are not available"
                            .to_owned(),
                    ),
                },
            }))
        }
    };

    let order_description = item
        .router_data
        .resource_common_data
        .connector_request_reference_id
        .clone();

    // Get invoice number (random string if > MAX_ID_LENGTH or None)
    let invoice_number =
        get_invoice_number_or_random(item.router_data.request.merchant_order_id.clone());

    let order = Order {
        invoice_number,
        description: order_description,
    };

    // Extract user fields from metadata
    let user_fields = metadata_to_user_fields(
        item.router_data.request.metadata.clone().expose_option(),
        false,
    )?;

    // billTo (with phoneNumber and email) from the payment's billing address
    let bill_to = item
        .router_data
        .resource_common_data
        .address
        .get_payment_billing()
        .map(BillTo::from);

    // shipTo from the shipping address, omitted when there is none
    let ship_to = item
        .router_data
        .resource_common_data
        .address
        .get_shipping()
        .and_then(ShipTo::from_address);

    // Level 2 / Level 3 data. Every amount goes through the connector's
    // FloatMajorUnit converter; nothing is computed here.
    let to_major_unit =
        |amount: MinorUnit, currency: enums::Currency| -> Result<FloatMajorUnit, Error> {
            item.connector
                .amount_converter
                .convert(amount, currency)
                .change_context(IntegrationError::AmountConversionFailed {
                    context: IntegrationErrorContext {
                        suggested_action: None,
                        doc_url: None,
                        additional_context: Some(
                            "Failed to convert a Level 2/3 amount for the authorize transaction"
                                .to_owned(),
                        ),
                    },
                })
        };
    let request_currency = item.router_data.request.currency;
    let l2_l3_data = item.router_data.resource_common_data.l2_l3_data.as_deref();
    let order_info = l2_l3_data.and_then(|data| data.order_info.as_ref());
    let tax_info = l2_l3_data.and_then(|data| data.tax_info.as_ref());

    // lineItems.lineItem[]: every order detail is forwarded. The API accepts at most
    // 30 line items; a longer list would fail the whole payment on schema
    // validation, so lineItems is omitted then instead of sending a partial order.
    let order_details = order_info.and_then(|info| info.order_details.as_ref());
    if let Some(dropped) = order_details.filter(|details| details.len() > MAX_LINE_ITEMS) {
        tracing::warn!(
            line_items = dropped.len(),
            max_line_items = MAX_LINE_ITEMS,
            "Order carries more line items than Authorize.Net accepts: lineItems omitted \
             while tax, duty and shipping are still sent"
        );
    }
    let line_items = order_details
        .filter(|details| !details.is_empty() && details.len() <= MAX_LINE_ITEMS)
        .map(|details| {
            details
                .iter()
                .enumerate()
                .map(|(index, detail)| {
                    Ok(LineItem {
                        // product_id when given, else the 1-based position
                        item_id: detail
                            .product_id
                            .clone()
                            .unwrap_or_else(|| (index + 1).to_string()),
                        name: detail.product_name.clone(),
                        description: detail.description.clone(),
                        quantity: detail.quantity,
                        unit_price: to_major_unit(detail.amount, request_currency)?,
                        unit_of_measure: detail.unit_of_measure.clone(),
                        tax_amount: detail
                            .total_tax_amount
                            .map(|amount| to_major_unit(amount, request_currency))
                            .transpose()?,
                        total_amount: detail
                            .total_amount
                            .map(|amount| to_major_unit(amount, request_currency))
                            .transpose()?,
                        commodity_code: detail.commodity_code.clone(),
                    })
                })
                .collect::<Result<Vec<_>, Error>>()
                .map(|line_item| LineItems { line_item })
        })
        .transpose()?;

    // tax: the request's order_tax_amount, else the L2/L3 tax info
    let tax = item
        .router_data
        .request
        .order_tax_amount
        .as_ref()
        .or_else(|| tax_info.and_then(|info| info.order_tax_amount.as_ref()))
        .map(|money| to_major_unit(money.amount, money.currency))
        .transpose()?
        .map(|amount| ExtendedAmount { amount });

    let duty = order_info
        .and_then(|info| info.duty_amount.as_ref())
        .map(|money| to_major_unit(money.amount, money.currency))
        .transpose()?
        .map(|amount| ExtendedAmount { amount });

    // shipping: the request's shipping_cost, else the L2/L3 order info
    let shipping = item
        .router_data
        .request
        .shipping_cost
        .as_ref()
        .or_else(|| order_info.and_then(|info| info.shipping_cost.as_ref()))
        .map(|money| to_major_unit(money.amount, money.currency))
        .transpose()?
        .map(|amount| ExtendedAmount { amount });

    let tax_exempt = tax_info
        .and_then(|info| info.tax_status)
        .map(|tax_status| match tax_status {
            common_enums::TaxStatus::Exempt => true,
            common_enums::TaxStatus::Taxable => false,
        });

    // poNumber: the L2/L3 merchant order reference (merchant_order_id itself stays
    // order.invoiceNumber). Limited to 25 characters; omitted when longer.
    let po_number = order_info
        .and_then(|info| info.merchant_order_reference_id.clone())
        .filter(|po_number| !po_number.is_empty())
        .and_then(|po_number| {
            within_limit(
                "order.poNumber",
                PO_NUMBER_MAX_LENGTH,
                po_number.len(),
                po_number,
            )
        });

    // merchantDescriptor (dynamic descriptor): the statement descriptor, passed
    // unchanged - Authorize.Net documents no length or format for it.
    let merchant_descriptor = item
        .router_data
        .request
        .billing_descriptor
        .as_ref()
        .and_then(|descriptor| descriptor.statement_descriptor.clone())
        .filter(|descriptor| !descriptor.is_empty());

    let customer_details = item
        .router_data
        .request
        .customer_id
        .as_ref()
        .filter(|_| {
            !item
                .router_data
                .request
                .is_customer_initiated_mandate_payment()
        })
        .and_then(|customer| {
            let customer_id = customer.get_string_repr();
            (customer_id.len() <= MAX_ID_LENGTH).then_some(CustomerDetails {
                id: customer_id.to_string(),
                email: item.router_data.request.get_optional_email(),
            })
        });

    // Check if we should create a profile for future mandate usage
    let profile = item
        .router_data
        .request
        .is_customer_initiated_mandate_payment()
        .then(|| {
            ProfileDetails::CreateProfileDetails(CreateProfileDetails {
                create_profile: true,
                customer_profile_id: item
                    .router_data
                    .resource_common_data
                    .connector_customer
                    .as_ref()
                    .map(|cid| Secret::new(cid.to_string())),
            })
        });

    Ok(AuthorizedotnetTransactionRequest {
        transaction_type,
        amount: Some(
            item.connector
                .amount_converter
                .convert(
                    item.router_data.request.amount.amount,
                    item.router_data.request.currency,
                )
                .change_context(IntegrationError::AmountConversionFailed {
                    context: Default::default(),
                })
                .attach_printable("Failed to convert payment amount for authorize transaction")?,
        ),
        currency_code: Some(currency),
        payment: Some(payment_details),
        profile,
        order: Some(order),
        line_items,
        tax,
        duty,
        shipping,
        tax_exempt,
        po_number,
        customer: customer_details,
        bill_to,
        ship_to,
        cardholder_authentication,
        user_fields,
        merchant_descriptor,
        processing_options: None,
        subsequent_auth_information: None,
        ref_trans_id: None,
    })
}

// RepeatPayment request structures
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetRepeatPaymentRequest<T: PaymentMethodDataTypes> {
    create_transaction_request: CreateRepeatPaymentRequest<T>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRepeatPaymentRequest<T: PaymentMethodDataTypes> {
    merchant_authentication: AuthorizedotnetAuthType,
    ref_id: Option<String>,
    transaction_request: AuthorizedotnetRepeatPaymentTransactionRequest<T>,
}

#[skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetRepeatPaymentTransactionRequest<T: PaymentMethodDataTypes> {
    // Element order is validated against the XSD `transactionRequestType` (a field
    // in the wrong position is rejected with E00003). Keep: transactionType, amount,
    // currencyCode, payment, profile, order, customer, billTo, userFields,
    // merchantDescriptor, processingOptions, subsequentAuthInformation.
    transaction_type: TransactionType,
    amount: FloatMajorUnit,
    currency_code: api_enums::Currency,
    // The card of a network-transaction-id MIT (no cardCode); absent when a stored
    // payment profile is charged.
    payment: Option<PaymentDetails<T>>,
    profile: Option<ProfileDetails>,
    order: Option<Order>,
    customer: Option<CustomerDetails>,
    bill_to: Option<BillTo>,
    user_fields: Option<UserFields>,
    merchant_descriptor: Option<String>,
    processing_options: Option<ProcessingOptions>,
    subsequent_auth_information: Option<SubsequentAuthInformation>,
}

// Implementation for RepeatPayment request conversion
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AuthorizedotnetRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AuthorizedotnetRepeatPaymentRequest<T>
{
    type Error = Error;
    fn try_from(
        item: AuthorizedotnetRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let merchant_authentication =
            AuthorizedotnetAuthType::try_from(&item.router_data.connector_config)?;

        let currency = item.router_data.request.currency;

        // Single capture only, decided before the mandate reference is read so the
        // refusal holds for every MIT shape (stored profile and network transaction
        // id) and no connector call is made.
        let transaction_type = match item.router_data.request.capture_method {
            Some(enums::CaptureMethod::Manual) => TransactionType::AuthOnlyTransaction,
            Some(enums::CaptureMethod::Automatic)
            | None
            | Some(enums::CaptureMethod::SequentialAutomatic) => {
                TransactionType::AuthCaptureTransaction
            }
            Some(enums::CaptureMethod::ManualMultiple) | Some(enums::CaptureMethod::Scheduled) => {
                return Err(error_stack::report!(IntegrationError::NotSupported {
                    message: "Capture method not supported".to_string(),
                    connector: "authorizedotnet",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Use capture_method automatic or manual (a single capture)".to_owned(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "Authorize.Net supports authCaptureTransaction and authOnlyTransaction with one priorAuthCaptureTransaction; multiple and scheduled captures are not available"
                                .to_owned(),
                        ),
                    },
                }))
            }
        };

        // Handle different mandate reference types with appropriate MIT structures
        let (payment, profile, bill_to, processing_options, subsequent_auth_information) = match &item
            .router_data
            .request
            .mandate_reference
        {
            // Case 1: Mandate-based MIT (using stored customer profile)
            MandateReferenceId::ConnectorMandateId(connector_mandate_ref) => {
                let mandate_id = connector_mandate_ref
                    .get_connector_mandate_id()
                    .ok_or_else(|| {
                        error_stack::report!(IntegrationError::MissingRequiredField {
                            field_name: "connector_mandate_id",
                            context: IntegrationErrorContext {
                                suggested_action: Some(
                                    "Send the connector_mandate_id returned by SetupRecurring (or by the mandate-creating Authorize)"
                                        .to_owned(),
                                ),
                                doc_url: None,
                                additional_context: Some(
                                    "A stored-profile charge needs the Authorize.Net customer profile and payment profile ids"
                                        .to_owned(),
                                ),
                            },
                        })
                    })?;

                // Parse mandate_id to extract customer_profile_id and payment_profile_id
                let profile = mandate_id
                    .split_once('-')
                    .map(|(customer_profile_id, payment_profile_id)| {
                        ProfileDetails::CustomerProfileDetails(CustomerProfileDetails {
                            customer_profile_id: Secret::from(customer_profile_id.to_string()),
                            payment_profile: PaymentProfileDetails {
                                payment_profile_id: Secret::from(payment_profile_id.to_string()),
                            },
                        })
                    })
                    .ok_or_else(|| {
                        error_stack::report!(IntegrationError::MissingRequiredField {
                            field_name: "valid mandate_id format (should contain '-')",
                            context: IntegrationErrorContext {
                                suggested_action: Some(
                                    "Send connector_mandate_id as <customerProfileId>-<customerPaymentProfileId>"
                                        .to_owned(),
                                ),
                                doc_url: None,
                                additional_context: Some(
                                    "The customer profile id and the payment profile id are read from either side of the first '-'"
                                        .to_owned(),
                                ),
                            },
                        })
                    })?;

                (
                    None, // The stored payment profile carries the card
                    Some(profile),
                    None, // No billTo: the payment profile holds the billing address
                    Some(ProcessingOptions {
                        is_subsequent_auth: true,
                    }),
                    None, // No network transaction ID for mandate-based flow
                )
            }

            // Case 2: Network mandate ID flow (PG agnostic with network trans ID).
            // The gateway has no stored profile to charge here, so the card itself is
            // sent - cardNumber and expirationDate only, never cardCode (a merchant
            // may not store it) - next to the network transaction id of the original
            // customer-initiated charge. Without the card the gateway answers reason
            // code 33. Same shape as the Hyperswitch connector.
            MandateReferenceId::NetworkMandateId(network_trans_id) => {
                let credit_card = match &item.router_data.request.payment_method_data {
                    PaymentMethodData::Card(card) => CreditCardDetails {
                        card_number: card.card_number.clone(),
                        expiration_date: card.get_expiry_date_as_yyyymm("-"),
                        card_code: None,
                    },
                    PaymentMethodData::CardDetailsForNetworkTransactionId(card_details) => {
                        CreditCardDetails {
                            card_number: card_details.card_number.clone(),
                            expiration_date: card_details.get_expiry_date_as_yyyymm("-"),
                            card_code: None,
                        }
                    }
                    PaymentMethodData::CardWithNoCvc(_)
                    | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
                    | PaymentMethodData::CardRedirect(_)
                    | PaymentMethodData::Wallet(_)
                    | PaymentMethodData::PayLater(_)
                    | PaymentMethodData::BankRedirect(_)
                    | PaymentMethodData::BankDebit(_)
                    | PaymentMethodData::BankTransfer(_)
                    | PaymentMethodData::Crypto(_)
                    | PaymentMethodData::MandatePayment
                    | PaymentMethodData::Reward
                    | PaymentMethodData::RealTimePayment(_)
                    | PaymentMethodData::Upi(_)
                    | PaymentMethodData::Voucher(_)
                    | PaymentMethodData::GiftCard(_)
                    | PaymentMethodData::PaymentMethodToken(_)
                    | PaymentMethodData::OpenBanking(_)
                    | PaymentMethodData::NetworkToken(_)
                    | PaymentMethodData::MobilePayment(_) => {
                        return Err(error_stack::report!(IntegrationError::NotImplemented(
                            get_unimplemented_payment_method_error_message("authorizedotnet"),
                            IntegrationErrorContext {
                                suggested_action: Some(
                                    "Send the card (card or card_details_for_network_transaction_id) with the network transaction id, or charge a stored profile through connector_mandate_id"
                                        .to_owned(),
                                ),
                                doc_url: None,
                                additional_context: Some(
                                    "A network-transaction-id charge on Authorize.Net needs payment.creditCard (cardNumber, expirationDate)"
                                        .to_owned(),
                                ),
                            },
                        )))
                    }
                };

                (
                    Some(PaymentDetails::CreditCard(credit_card)),
                    None, // No customer profile for network transaction flow
                    // billTo from the payment's billing address, built by the same
                    // helper as Authorize
                    item.router_data
                        .resource_common_data
                        .address
                        .get_payment_billing()
                        .map(BillTo::from),
                    Some(ProcessingOptions {
                        is_subsequent_auth: true,
                    }),
                    Some(SubsequentAuthInformation {
                        original_network_trans_id: Secret::new(
                            network_trans_id.network_transaction_id.clone(),
                        ),
                        // Always `resubmission`, as the Hyperswitch connector sends
                        reason: Reason::Resubmission,
                    }),
                )
            }

            // Case 3: Network token with NTI - NOT SUPPORTED (same as Hyperswitch)
            MandateReferenceId::NetworkTokenWithNTI(_) => {
                return Err(error_stack::report!(IntegrationError::NotSupported {
                    message: "Network token with NTI not supported for authorizedotnet".to_string(),
                    connector: "authorizedotnet",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Charge the stored profile (connector_mandate_id) or the card with its network transaction id"
                                .to_owned(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "Authorize.Net merchant-initiated charges take a customer payment profile or a card with subsequentAuthInformation; a network token is not accepted"
                                .to_owned(),
                        ),
                    },
                }))
            }
        };

        // Order description should be connector_request_reference_id (same as Hyperswitch)
        let order_description = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        // Get invoice number (random string if > MAX_ID_LENGTH or None)
        let invoice_number =
            get_invoice_number_or_random(item.router_data.request.merchant_order_id.clone());

        let order = Order {
            invoice_number,
            description: order_description,
        };

        // Extract user fields from metadata (RepeatPayment metadata is HashMap, needs conversion to Value)
        let user_fields = metadata_to_user_fields(
            item.router_data
                .request
                .metadata
                .clone()
                .map(serde_json::to_value)
                .transpose()
                .change_context(IntegrationError::RequestEncodingFailed {
                    context: Default::default(),
                })?,
            false, // Already serialized above
        )?;

        // ref_id should be connector_request_reference_id with MAX_ID_LENGTH check (same as Authorize flow)
        let ref_id = Some(
            item.router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        )
        .filter(|id| id.len() <= MAX_ID_LENGTH);

        let customer_id_string = validate_customer_id_length(
            item.router_data
                .resource_common_data
                .customer_id
                .as_ref()
                .map(|cid| cid.get_string_repr().to_owned()),
        );

        let customer_details = customer_id_string.map(|cid| CustomerDetails {
            id: cid,
            email: item.router_data.request.email.clone(),
        });

        // merchantDescriptor (dynamic descriptor) on both MIT shapes: the statement
        // descriptor, passed unchanged - Authorize.Net documents no length or format.
        let merchant_descriptor = item
            .router_data
            .request
            .billing_descriptor
            .as_ref()
            .and_then(|descriptor| descriptor.statement_descriptor.clone())
            .filter(|descriptor| !descriptor.is_empty());

        let transaction_request = AuthorizedotnetRepeatPaymentTransactionRequest {
            transaction_type,
            amount: item
                .connector
                .amount_converter
                .convert(
                    item.router_data.request.amount.amount,
                    item.router_data.request.currency,
                )
                .change_context(IntegrationError::AmountConversionFailed {
                    context: Default::default(),
                })
                .attach_printable(
                    "Failed to convert payment amount for repeat payment transaction",
                )?,
            currency_code: currency,
            payment,
            profile,
            order: Some(order),
            customer: customer_details,
            bill_to,
            user_fields,
            merchant_descriptor,
            processing_options,
            subsequent_auth_information,
        };

        Ok(Self {
            create_transaction_request: CreateRepeatPaymentRequest {
                merchant_authentication,
                ref_id,
                transaction_request,
            },
        })
    }
}

#[skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetCaptureTransactionInternal {
    // Specific transaction details for Capture
    transaction_type: TransactionType,
    amount: FloatMajorUnit,
    ref_trans_id: String,
}

#[skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCaptureTransactionRequest {
    // Used by Capture Flow, wraps specific capture transaction details
    merchant_authentication: AuthorizedotnetAuthType,
    transaction_request: AuthorizedotnetCaptureTransactionInternal,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetCaptureRequest {
    // Top-level wrapper for Capture Flow
    create_transaction_request: CreateCaptureTransactionRequest,
}

// New direct implementation for capture without relying on the reference version
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AuthorizedotnetRouterData<
            RouterDataV2<
                domain_types::connector_flow::Capture,
                PaymentFlowData,
                PaymentsCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AuthorizedotnetCaptureRequest
{
    type Error = Error;
    fn try_from(
        item: AuthorizedotnetRouterData<
            RouterDataV2<
                domain_types::connector_flow::Capture,
                PaymentFlowData,
                PaymentsCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        let original_connector_txn_id = match &router_data.request.connector_transaction_id {
            ResponseId::ConnectorTransactionId(id) => id.clone(),
            _ => {
                return Err(error_stack::report!(
                    HsInterfacesConnectorRequestError::MissingRequiredField {
                        field_name: "connector_transaction_id",
                        context: Default::default()
                    }
                ));
            }
        };

        let transaction_request_payload = AuthorizedotnetCaptureTransactionInternal {
            transaction_type: TransactionType::PriorAuthCaptureTransaction,
            amount: item
                .connector
                .amount_converter
                .convert(
                    item.router_data.request.amount_to_capture.amount,
                    item.router_data.request.currency,
                )
                .change_context(IntegrationError::AmountConversionFailed {
                    context: Default::default(),
                })
                .attach_printable("Failed to convert capture amount for capture transaction")?,
            ref_trans_id: original_connector_txn_id,
        };

        let merchant_authentication =
            AuthorizedotnetAuthType::try_from(&item.router_data.connector_config)?;

        let create_transaction_request_payload = CreateCaptureTransactionRequest {
            merchant_authentication,
            transaction_request: transaction_request_payload,
        };

        Ok(Self {
            create_transaction_request: create_transaction_request_payload,
        })
    }
}

#[skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetTransactionVoidDetails {
    // Specific transaction details for Void
    transaction_type: TransactionType,
    ref_trans_id: String,
    amount: Option<f64>,
}

#[skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTransactionVoidRequest {
    // Used by Void Flow, wraps specific void transaction details
    merchant_authentication: AuthorizedotnetAuthType,
    ref_id: Option<String>,
    transaction_request: AuthorizedotnetTransactionVoidDetails,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetVoidRequest {
    // Top-level wrapper for Void Flow
    create_transaction_request: CreateTransactionVoidRequest,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetAuthType {
    name: Secret<String>,
    transaction_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for AuthorizedotnetAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        if let ConnectorSpecificConfig::Authorizedotnet {
            name,
            transaction_key,
            ..
        } = auth_type
        {
            Ok(Self {
                name: name.to_owned(),
                transaction_key: transaction_key.to_owned(),
            })
        } else {
            Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            })?
        }
    }
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AuthorizedotnetRouterData<
            RouterDataV2<
                domain_types::connector_flow::Void,
                PaymentFlowData,
                PaymentVoidData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AuthorizedotnetVoidRequest
{
    type Error = Error;

    fn try_from(
        item: AuthorizedotnetRouterData<
            RouterDataV2<
                domain_types::connector_flow::Void,
                PaymentFlowData,
                PaymentVoidData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract transaction ID from the connector_transaction_id string
        // This transaction ID comes from the authorization response
        let transaction_id = match router_data.request.connector_transaction_id.as_str() {
            "" => {
                return Err(error_stack::report!(
                    HsInterfacesConnectorRequestError::MissingRequiredField {
                        field_name: "connector_transaction_id",
                        context: Default::default()
                    }
                ));
            }
            id => id.to_string(),
        };

        let ref_id = Some(
            item.router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        )
        .filter(|id| id.len() <= MAX_ID_LENGTH);

        let transaction_void_details = AuthorizedotnetTransactionVoidDetails {
            transaction_type: TransactionType::VoidTransaction,
            ref_trans_id: transaction_id,
            amount: None,
        };

        let merchant_authentication =
            AuthorizedotnetAuthType::try_from(&router_data.connector_config)?;

        let create_transaction_void_request = CreateTransactionVoidRequest {
            merchant_authentication,
            ref_id,
            transaction_request: transaction_void_details,
        };

        Ok(Self {
            create_transaction_request: create_transaction_void_request,
        })
    }
}

// VoidPostCapture (Reverse) Flow — reuses the same API structure as Void
// Authorize.net's voidTransaction works on captured-but-not-yet-settled transactions

#[skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetTransactionVoidPCDetails {
    // Transaction details for VoidPostCapture (same structure as Void)
    transaction_type: TransactionType,
    ref_trans_id: String,
    amount: Option<f64>,
}

#[skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTransactionVoidPCRequest {
    // Wraps VoidPostCapture transaction details
    merchant_authentication: AuthorizedotnetAuthType,
    ref_id: Option<String>,
    transaction_request: AuthorizedotnetTransactionVoidPCDetails,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetVoidPCRequest {
    // Top-level wrapper for VoidPostCapture Flow
    create_transaction_request: CreateTransactionVoidPCRequest,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthorizedotnetVoidPCResponse(pub AuthorizedotnetPaymentsResponse);

impl From<AuthorizedotnetPaymentsResponse> for AuthorizedotnetVoidPCResponse {
    fn from(response: AuthorizedotnetPaymentsResponse) -> Self {
        Self(response)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AuthorizedotnetRouterData<
            RouterDataV2<
                VoidPC,
                PaymentFlowData,
                PaymentsCancelPostCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AuthorizedotnetVoidPCRequest
{
    type Error = Error;

    fn try_from(
        item: AuthorizedotnetRouterData<
            RouterDataV2<
                VoidPC,
                PaymentFlowData,
                PaymentsCancelPostCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract transaction ID from the connector_transaction_id string
        // This is the captured transaction ID to be reversed
        let transaction_id = match router_data.request.connector_transaction_id.as_str() {
            "" => {
                return Err(error_stack::report!(
                    HsInterfacesConnectorRequestError::MissingRequiredField {
                        field_name: "connector_transaction_id",
                        context: Default::default()
                    }
                ));
            }
            id => id.to_string(),
        };

        let ref_id = Some(
            item.router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        )
        .filter(|id| id.len() <= MAX_ID_LENGTH);

        let transaction_void_pc_details = AuthorizedotnetTransactionVoidPCDetails {
            transaction_type: TransactionType::VoidTransaction,
            ref_trans_id: transaction_id,
            amount: None,
        };

        let merchant_authentication =
            AuthorizedotnetAuthType::try_from(&router_data.connector_config)?;

        let create_transaction_void_pc_request = CreateTransactionVoidPCRequest {
            merchant_authentication,
            ref_id,
            transaction_request: transaction_void_pc_details,
        };

        Ok(Self {
            create_transaction_request: create_transaction_void_pc_request,
        })
    }
}

impl<F> TryFrom<ResponseRouterData<AuthorizedotnetVoidPCResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        value: ResponseRouterData<AuthorizedotnetVoidPCResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;
        let inner = &response.0;

        // VoidPC carries its outcome through PostCaptureVoidStatus, so map the
        // Authorize.net response directly instead of going through the generic
        // AttemptStatus-deriving helper used by the other payment flows. Failures
        // are surfaced as PostCaptureVoidStatus::Failed with the error text in
        // `description`, rather than as ErrorResponse.
        let connector_response_data = match &inner.transaction_response {
            Some(TransactionResponse::AuthorizedotnetTransactionResponse(trans_res)) => {
                convert_to_additional_payment_method_connector_response(trans_res).map(
                    domain_types::router_data::ConnectorResponseData::with_additional_payment_method_data,
                )
            }
            _ => None,
        };

        // Carry the connector's top-level message text through as the
        // PostCaptureVoidResponse description (e.g. "This transaction has been approved.").
        let response_description = inner.messages.message.first().map(|m| m.text.clone());

        let (post_capture_void_status, connector_reference_id, description) =
            if inner.messages.result_code == ResultCode::Error {
                let (_error_code, error_message) = extract_error_details(inner, None);
                (
                    common_enums::PostCaptureVoidStatus::Failed,
                    None,
                    Some(error_message),
                )
            } else {
                match &inner.transaction_response {
                    Some(TransactionResponse::AuthorizedotnetTransactionResponse(trans_res)) => {
                        match trans_res.response_code {
                            AuthorizedotnetPaymentStatus::Approved => (
                                common_enums::PostCaptureVoidStatus::Succeeded,
                                Some(trans_res.transaction_id.clone()),
                                response_description.clone(),
                            ),
                            // An undocumented responseCode is not a verdict either way.
                            AuthorizedotnetPaymentStatus::HeldForReview
                            | AuthorizedotnetPaymentStatus::RequiresAction
                            | AuthorizedotnetPaymentStatus::Unknown => (
                                common_enums::PostCaptureVoidStatus::Pending,
                                Some(trans_res.transaction_id.clone()),
                                response_description.clone(),
                            ),
                            AuthorizedotnetPaymentStatus::Declined
                            | AuthorizedotnetPaymentStatus::Error => {
                                let (_error_code, error_message) =
                                    extract_error_details(inner, Some(trans_res));
                                (
                                    common_enums::PostCaptureVoidStatus::Failed,
                                    Some(trans_res.transaction_id.clone()),
                                    Some(error_message),
                                )
                            }
                        }
                    }
                    Some(TransactionResponse::AuthorizedotnetTransactionResponseError(_)) => {
                        let (_error_code, error_message) = extract_error_details(inner, None);
                        (
                            common_enums::PostCaptureVoidStatus::Failed,
                            None,
                            Some(error_message),
                        )
                    }
                    None => (
                        common_enums::PostCaptureVoidStatus::Succeeded,
                        None,
                        response_description.clone(),
                    ),
                }
            };

        let response_result = Ok(PaymentsResponseData::PostCaptureVoidResponse {
            post_capture_void_status,
            connector_reference_id,
            description,
            status_code: http_code,
        });

        let mut new_router_data = router_data;
        let mut resource_common_data = new_router_data.resource_common_data.clone();
        resource_common_data.connector_response = connector_response_data;
        new_router_data.resource_common_data = resource_common_data;
        new_router_data.response = response_result;

        Ok(new_router_data)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionDetails {
    pub merchant_authentication: MerchantAuthentication,
    #[serde(rename = "transId")]
    pub transaction_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetCreateSyncRequest {
    pub get_transaction_details_request: TransactionDetails,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetRSyncRequest {
    pub get_transaction_details_request: TransactionDetails,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AuthorizedotnetRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for AuthorizedotnetCreateSyncRequest
{
    type Error = Error;

    fn try_from(
        item: AuthorizedotnetRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Extract connector_transaction_id from the request
        let connector_transaction_id = match &item.router_data.request.connector_transaction_id {
            ResponseId::ConnectorTransactionId(id) => id.clone(),
            _ => {
                return Err(error_stack::report!(
                    HsInterfacesConnectorRequestError::MissingRequiredField {
                        field_name: "connector_transaction_id",
                        context: Default::default()
                    }
                ))
            }
        };

        let merchant_authentication =
            MerchantAuthentication::try_from(&item.router_data.connector_config)?;

        let payload = Self {
            get_transaction_details_request: TransactionDetails {
                merchant_authentication,
                transaction_id: Some(connector_transaction_id),
            },
        };
        Ok(payload)
    }
}

// Implementation for the RSync flow to support refund synchronization
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AuthorizedotnetRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for AuthorizedotnetRSyncRequest
{
    type Error = Error;

    fn try_from(
        item: AuthorizedotnetRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Extract connector_refund_id from the request
        let connector_refund_id = if !item.router_data.request.connector_refund_id.is_empty() {
            item.router_data.request.connector_refund_id.clone()
        } else {
            return Err(error_stack::report!(
                HsInterfacesConnectorRequestError::MissingRequiredField {
                    field_name: "connector_refund_id",
                    context: Default::default()
                }
            ));
        };

        let merchant_authentication =
            MerchantAuthentication::try_from(&item.router_data.connector_config)?;

        let payload = Self {
            get_transaction_details_request: TransactionDetails {
                merchant_authentication,
                transaction_id: Some(connector_refund_id),
            },
        };
        Ok(payload)
    }
}

// Refund-related structs and implementations
#[skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetRefundCardDetails {
    card_number: Secret<String>,
    expiration_date: Secret<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
enum AuthorizedotnetRefundPaymentDetails<T: PaymentMethodDataTypes> {
    CreditCard(CreditCardDetails<T>),
}

#[skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetRefundTransactionDetails {
    transaction_type: TransactionType,
    amount: FloatMajorUnit,
    currency_code: api_enums::Currency,
    payment: RefundPaymentDetails,
    ref_trans_id: String,
}

#[skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetRefundRequest {
    create_transaction_request: CreateTransactionRefundRequest,
}

#[skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTransactionRefundRequest {
    merchant_authentication: AuthorizedotnetAuthType,
    transaction_request: AuthorizedotnetRefundTransactionDetails,
}

#[skip_serializing_none]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefundPaymentDetails {
    credit_card: CreditCardInfo,
}

#[skip_serializing_none]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreditCardInfo {
    card_number: String,
    expiration_date: String,
}

// Unified generic implementation for all payment method types
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AuthorizedotnetRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for AuthorizedotnetRefundRequest
{
    type Error = Error;

    fn try_from(
        item: AuthorizedotnetRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let merchant_authentication =
            AuthorizedotnetAuthType::try_from(&item.router_data.connector_config)?;

        // Extract payment details from metadata using unified helper
        let connector_metadata_secret = item
            .router_data
            .request
            .connector_feature_data
            .clone()
            .ok_or_else(|| {
                error_stack::report!(HsInterfacesConnectorRequestError::MissingRequiredField {
                    field_name: "connector_feature_data",
                    context: Default::default()
                })
            })?;
        let payment = get_refund_credit_card_payment(&Some(connector_metadata_secret))?;

        // Build the refund transaction request
        let transaction_request = AuthorizedotnetRefundTransactionDetails {
            transaction_type: TransactionType::RefundTransaction,
            amount: item
                .connector
                .amount_converter
                .convert(
                    item.router_data.request.refund_amount.amount,
                    item.router_data.request.currency,
                )
                .change_context(IntegrationError::AmountConversionFailed {
                    context: Default::default(),
                })
                .attach_printable("Failed to convert refund amount for refund transaction")?,
            currency_code: item.router_data.request.currency,
            payment,
            ref_trans_id: item.router_data.request.connector_transaction_id.clone(),
        };

        Ok(Self {
            create_transaction_request: CreateTransactionRefundRequest {
                merchant_authentication,
                transaction_request,
            },
        })
    }
}

// Refund request struct is fully implemented above

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum TransactionResponse {
    AuthorizedotnetTransactionResponse(Box<AuthorizedotnetTransactionResponse>),
    AuthorizedotnetTransactionResponseError(Box<AuthorizedotnetTransactionResponseError>),
}

// Base transaction response - used internally
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionProfileInfo {
    customer_profile_id: String,
    customer_payment_profile_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetTransactionResponse {
    response_code: AuthorizedotnetPaymentStatus,
    #[serde(rename = "transId")]
    transaction_id: String,
    #[serde(default)]
    transaction_status: Option<String>,
    #[serde(default)]
    network_trans_id: Option<Secret<String>>,
    #[serde(default)]
    pub(super) account_number: Option<Secret<String>>,
    #[serde(default)]
    pub(super) account_type: Option<Secret<String>>,
    #[serde(default)]
    pub(super) errors: Option<Vec<ErrorMessage>>,
    #[serde(default)]
    secure_acceptance: Option<SecureAcceptance>,
    #[serde(default)]
    profile: Option<TransactionProfileInfo>,
    #[serde(rename = "avsResultCode")]
    avs_result_code: Option<AvsResultCode>,
    /// CVV check result.
    cvv_result_code: Option<CvvResultCode>,
    /// CAVV validation result of an external 3-D Secure authentication.
    cavv_result_code: Option<String>,
    /// Merchant advice code and text (returned for TSYS only).
    merchant_advice: Option<MerchantAdvice>,
    /// Processor response code. Present in the XSD without a documented meaning;
    /// surfaced only when the gateway returns it.
    raw_response_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MerchantAdvice {
    code: Option<String>,
    description: Option<String>,
}

/// Authorize.Net returns an empty string for a result code it has nothing to say
/// about; treat that the same as an absent field.
fn non_empty_code(code: Option<&String>) -> Option<&str> {
    code.map(|code| code.trim()).filter(|code| !code.is_empty())
}

// Create flow-specific response types
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthorizedotnetAuthorizeResponse(pub AuthorizedotnetPaymentsResponse);

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthorizedotnetCaptureResponse(pub AuthorizedotnetPaymentsResponse);

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthorizedotnetVoidResponse(pub AuthorizedotnetPaymentsResponse);

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthorizedotnetRepeatPaymentResponse(pub AuthorizedotnetPaymentsResponse);

/// `transactionResponse.avsResultCode` (API reference). A code the reference does not
/// list is kept verbatim in `Other`, so the raw gateway value still reaches
/// `payment_checks` and an unlisted code never fails the response.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum AvsResultCode {
    A,
    B,
    E,
    G,
    N,
    P,
    R,
    S,
    U,
    W,
    X,
    Y,
    Z,
    #[serde(untagged)]
    Other(String),
}

impl AvsResultCode {
    fn description(&self) -> Option<&'static str> {
        match self {
            Self::A => Some("The street address matched, but the postal code did not."),
            Self::B => Some("No address information was provided."),
            Self::E => Some(
                "AVS data provided is invalid or AVS is not allowed for the card type that was used.",
            ),
            Self::G => {
                Some("The card was issued by a bank outside the U.S. and does not support AVS.")
            }
            Self::N => Some("Neither the street address nor postal code matched."),
            Self::P => Some("AVS is not applicable for this transaction."),
            Self::R => Some("Retry — AVS was unavailable or timed out."),
            Self::S => Some("AVS is not supported by card issuer."),
            Self::U => Some("Address information is unavailable."),
            Self::W => Some("The US ZIP+4 code matches, but the street address does not."),
            Self::X => Some("Both the street address and the US ZIP+4 code matched."),
            Self::Y => Some("The street address and postal code matched."),
            Self::Z => Some("The postal code matched, but the street address did not."),
            Self::Other(_) => None,
        }
    }

    /// `P` is "not applicable for this transaction" and an empty string is the gateway
    /// saying it has nothing to report: neither earns a `payment_checks` row.
    fn is_reportable(&self) -> bool {
        !matches!(self, Self::P) && !matches!(self, Self::Other(code) if code.trim().is_empty())
    }
}

/// `transactionResponse.cvvResultCode` (API reference), same `Other` rule as
/// `AvsResultCode`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum CvvResultCode {
    M,
    N,
    P,
    S,
    U,
    #[serde(untagged)]
    Other(String),
}

impl CvvResultCode {
    fn description(&self) -> Option<&'static str> {
        match self {
            Self::M => Some("CVV matched."),
            Self::N => Some("CVV did not match."),
            Self::P => Some("CVV was not processed."),
            Self::S => Some("CVV should have been present but was not indicated."),
            Self::U => Some("The issuer was unable to process the CVV check."),
            Self::Other(_) => None,
        }
    }

    fn is_reportable(&self) -> bool {
        !matches!(self, Self::Other(code) if code.trim().is_empty())
    }
}

// Convert transaction response to additional payment method connector response.
// `payment_checks` carries the AVS, CVV and CAVV results the gateway returned; the
// Card connector response is emitted when at least one of the three is present.
fn convert_to_additional_payment_method_connector_response(
    transaction_response: &AuthorizedotnetTransactionResponse,
) -> Option<domain_types::router_data::AdditionalPaymentMethodConnectorResponse> {
    let avs_result_code = transaction_response
        .avs_result_code
        .as_ref()
        .filter(|code| code.is_reportable());
    let cvv_result_code = transaction_response
        .cvv_result_code
        .as_ref()
        .filter(|code| code.is_reportable());
    let cavv_result_code = non_empty_code(transaction_response.cavv_result_code.as_ref());

    let mut payment_checks = serde_json::Map::new();
    if let Some(code) = avs_result_code {
        payment_checks.insert("avs_result_code".to_string(), serde_json::json!(code));
        payment_checks.insert(
            "description".to_string(),
            serde_json::json!(code.description()),
        );
    }
    if let Some(code) = cvv_result_code {
        payment_checks.insert("cvv_result_code".to_string(), serde_json::json!(code));
        payment_checks.insert(
            "cvv_description".to_string(),
            serde_json::json!(code.description()),
        );
    }
    if let Some(code) = cavv_result_code {
        payment_checks.insert("cavv_result_code".to_string(), serde_json::json!(code));
    }

    (!payment_checks.is_empty()).then(|| {
        domain_types::router_data::AdditionalPaymentMethodConnectorResponse::Card {
            authentication_data: None,
            payment_checks: Some(serde_json::Value::Object(payment_checks)),
            card_network: None,
            domestic_network: None,
            auth_code: None,
        }
    })
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefundResponse {
    response_code: AuthorizedotnetRefundStatus,
    #[serde(rename = "transId")]
    transaction_id: String,
    network_trans_id: Option<Secret<String>>,
    pub account_number: Option<Secret<String>>,
    pub errors: Option<Vec<ErrorMessage>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetRefundResponse {
    pub transaction_response: RefundResponse,
    pub messages: ResponseMessages,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetCreateConnectorCustomerRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    create_customer_profile_request: AuthorizedotnetZeroMandateRequest<T>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetZeroMandateRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    merchant_authentication: AuthorizedotnetAuthType,
    profile: Profile<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    validation_mode: Option<ValidationMode>,
}

// ShipToList for customer shipping address
#[skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ShipToList {
    first_name: Option<Secret<String>>,
    last_name: Option<Secret<String>>,
    address: Option<Secret<String>>,
    city: Option<Secret<String>>,
    state: Option<Secret<String>>,
    zip: Option<Secret<String>>,
    country: Option<common_enums::CountryAlpha2>,
    phone_number: Option<Secret<String>>,
}

#[skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Profile<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> {
    merchant_customer_id: Option<String>,
    description: Option<String>,
    email: Option<Email>,
    payment_profiles: Option<Vec<PaymentProfiles<T>>>,
    ship_to_list: Option<Vec<ShipToList>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PaymentProfiles<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    customer_type: CustomerType,
    payment: PaymentDetails<T>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CustomerType {
    Individual,
    Business,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ValidationMode {
    // testMode performs a Luhn mod-10 check on the card number, without further validation at connector.
    TestMode,
    // liveMode submits a zero-dollar or one-cent transaction (depending on card type and processor support) to confirm that the card number belongs to an active credit or debit account.
    LiveMode,
}

// SetupMandate request structures - adds payment profile to existing customer
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetSetupMandateRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    create_customer_payment_profile_request: AuthorizedotnetPaymentProfileRequest<T>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetPaymentProfileRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    merchant_authentication: AuthorizedotnetAuthType,
    customer_profile_id: Secret<String>,
    payment_profile: PaymentProfile<T>,
    validation_mode: ValidationMode,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentProfile<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(skip_serializing_if = "Option::is_none")]
    bill_to: Option<BillTo>,
    payment: PaymentDetails<T>,
}

// SetupMandate response structure
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetSetupMandateResponse {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub customer_payment_profile_id_list: Vec<String>,
    pub customer_profile_id: Option<String>,
    #[serde(rename = "customerPaymentProfileId")]
    pub customer_payment_profile_id: Option<String>,
    pub validation_direct_response_list: Option<Vec<Secret<String>>>,
    pub messages: ResponseMessages,
}

// PSync response wrapper - Using direct structure instead of wrapping AuthorizedotnetPaymentsResponse
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthorizedotnetPSyncResponse {
    pub transaction: Option<SyncTransactionResponse>,
    pub messages: ResponseMessages,
}

// Implement From/TryFrom for the response types
impl From<AuthorizedotnetPaymentsResponse> for AuthorizedotnetAuthorizeResponse {
    fn from(response: AuthorizedotnetPaymentsResponse) -> Self {
        Self(response)
    }
}

impl From<AuthorizedotnetPaymentsResponse> for AuthorizedotnetCaptureResponse {
    fn from(response: AuthorizedotnetPaymentsResponse) -> Self {
        Self(response)
    }
}

impl From<AuthorizedotnetPaymentsResponse> for AuthorizedotnetVoidResponse {
    fn from(response: AuthorizedotnetPaymentsResponse) -> Self {
        Self(response)
    }
}

impl From<AuthorizedotnetPaymentsResponse> for AuthorizedotnetRepeatPaymentResponse {
    fn from(response: AuthorizedotnetPaymentsResponse) -> Self {
        Self(response)
    }
}

// We no longer need the From implementation for AuthorizedotnetPSyncResponse since we're using the direct structure

// TryFrom implementations for the router data conversions

impl<
        F,
        T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize + Serialize,
    > TryFrom<ResponseRouterData<AuthorizedotnetAuthorizeResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        value: ResponseRouterData<AuthorizedotnetAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        // Use our helper function to convert the response
        let (status, response_result, connector_response_data) =
            convert_to_payments_response_data_or_error(
                &response.0,
                http_code,
                Operation::Authorize,
                router_data.request.capture_method,
                router_data
                    .resource_common_data
                    .raw_connector_response
                    .clone(),
            );

        // Create a new RouterDataV2 with updated fields
        let mut new_router_data = router_data;

        // Update the status and connector_response in resource_common_data
        let mut resource_common_data = new_router_data.resource_common_data.clone();
        resource_common_data.status = status;
        resource_common_data.connector_response = connector_response_data;
        new_router_data.resource_common_data = resource_common_data;

        // Set the response
        new_router_data.response = response_result;

        Ok(new_router_data)
    }
}

impl<F> TryFrom<ResponseRouterData<AuthorizedotnetCaptureResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        value: ResponseRouterData<AuthorizedotnetCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        // Use our helper function to convert the response
        let (status, response_result, connector_response_data) =
            convert_to_payments_response_data_or_error(
                &response.0,
                http_code,
                Operation::Capture,
                None,
                router_data
                    .resource_common_data
                    .raw_connector_response
                    .clone(),
            );

        // Create a new RouterDataV2 with updated fields
        let mut new_router_data = router_data;

        // Update the status and connector_response in resource_common_data
        let mut resource_common_data = new_router_data.resource_common_data.clone();
        resource_common_data.status = status;
        resource_common_data.connector_response = connector_response_data;
        new_router_data.resource_common_data = resource_common_data;

        // Set the response
        new_router_data.response = response_result;

        Ok(new_router_data)
    }
}

impl<F> TryFrom<ResponseRouterData<AuthorizedotnetVoidResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        value: ResponseRouterData<AuthorizedotnetVoidResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;
        // Use our helper function to convert the response
        let (status, response_result, connector_response_data) =
            convert_to_payments_response_data_or_error(
                &response.0,
                http_code,
                Operation::Void,
                None,
                router_data
                    .resource_common_data
                    .raw_connector_response
                    .clone(),
            );

        // Create a new RouterDataV2 with updated fields
        let mut new_router_data = router_data;

        // Update the status and connector_response in resource_common_data
        let mut resource_common_data = new_router_data.resource_common_data.clone();
        resource_common_data.status = status;
        resource_common_data.connector_response = connector_response_data;
        new_router_data.resource_common_data = resource_common_data;

        // Set the response
        new_router_data.response = response_result;

        Ok(new_router_data)
    }
}

impl<
        F,
        T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize + Serialize,
    > TryFrom<ResponseRouterData<AuthorizedotnetRepeatPaymentResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        value: ResponseRouterData<AuthorizedotnetRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        // Dedicated RepeatPayment response handling (matching Hyperswitch)
        let status = get_hs_status(
            &response.0,
            http_code,
            Operation::Authorize,
            router_data.request.capture_method,
        );

        // Extract connector response data
        let connector_response_data = match &response.0.transaction_response {
            Some(TransactionResponse::AuthorizedotnetTransactionResponse(trans_res)) => {
                convert_to_additional_payment_method_connector_response(trans_res)
                    .map(domain_types::router_data::ConnectorResponseData::with_additional_payment_method_data)
            }
            _ => None
};

        let response_result = match &response.0.transaction_response {
            Some(TransactionResponse::AuthorizedotnetTransactionResponse(transaction_response)) => {
                // Check for errors in the response. Built through `create_error_response`
                // so a declined MIT carries the merchant advice code / processor
                // response code like Authorize does.
                let error = transaction_response.errors.as_ref().and_then(|errors| {
                    errors.first().map(|error| {
                        create_error_response(
                            http_code,
                            error.error_code.clone(),
                            error.error_text.clone(),
                            status,
                            Some(transaction_response.transaction_id.clone()),
                            Some(transaction_response),
                            None,
                        )
                    })
                });

                // Extract mandate_reference from transaction_response.profile (RepeatPayment returns profile info)
                let mandate_reference =
                    transaction_response
                        .profile
                        .as_ref()
                        .map(|profile| MandateReference {
                            connector_mandate_id: Some(format!(
                                "{}-{}",
                                profile.customer_profile_id, profile.customer_payment_profile_id
                            )),
                            payment_method_id: None,
                            connector_mandate_request_reference_id: None,
                            mandate_metadata: None,
                        });

                // Build connector_metadata from account_number
                let connector_metadata = build_connector_metadata(transaction_response);

                match error {
                    Some(err) => Err(err),
                    None => Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(
                            transaction_response.transaction_id.clone(),
                        ),
                        redirection_data: None,
                        mandate_reference: mandate_reference.map(Box::new),
                        connector_metadata,
                        network_txn_id: transaction_response
                            .network_trans_id
                            .as_ref()
                            .map(|s| s.peek().clone()),
                        network_txn_link_id: None,
                        connector_response_reference_id: Some(
                            transaction_response.transaction_id.clone(),
                        ),
                        incremental_authorization_allowed: None,
                        status_code: http_code,
                        splits: None,
                        payment_account_reference: None,
                    }),
                }
            }
            Some(TransactionResponse::AuthorizedotnetTransactionResponseError(_)) | None => {
                let (error_code, error_message) = extract_error_details(&response.0, None);
                Err(create_error_response(
                    http_code,
                    error_code,
                    error_message,
                    status,
                    None,
                    None,
                    router_data
                        .resource_common_data
                        .raw_connector_response
                        .clone(),
                ))
            }
        };

        // Create a new RouterDataV2 with updated fields
        let mut new_router_data = router_data;

        // Update the status and connector_response in resource_common_data
        let mut resource_common_data = new_router_data.resource_common_data.clone();
        resource_common_data.status = status;
        resource_common_data.connector_response = connector_response_data;
        new_router_data.resource_common_data = resource_common_data;

        // Set the response
        new_router_data.response = response_result;

        Ok(new_router_data)
    }
}

impl TryFrom<ResponseRouterData<AuthorizedotnetRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        value: ResponseRouterData<AuthorizedotnetRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        let transaction_response = &response.transaction_response;
        let refund_status = RefundStatus::from(transaction_response.response_code.clone());

        let error = transaction_response.errors.clone().and_then(|errors| {
            errors.first().map(|error| ErrorResponse {
                code: error.error_code.clone(),
                message: error.error_text.clone(),
                reason: Some(error.error_text.clone()),
                status_code: http_code,
                attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
                connector_transaction_id: Some(transaction_response.transaction_id.clone()),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            })
        });

        // Create a new RouterDataV2 with updated fields
        let mut new_router_data = router_data;

        // Update the status in resource_common_data
        let mut resource_common_data = new_router_data.resource_common_data.clone();
        resource_common_data.status = refund_status;
        new_router_data.resource_common_data = resource_common_data;

        // Set the response based on whether there was an error
        new_router_data.response = match error {
            Some(err) => Err(err),
            None => Ok(RefundsResponseData {
                connector_refund_id: transaction_response.transaction_id.clone(),
                refund_status,
                status_code: http_code,
                acquirer_reference_number: None,
            }),
        };

        Ok(new_router_data)
    }
}

/// Error triple for CreateConnectorCustomer and SetupMandate, where hyperswitch's direct
/// Authorize.net flow uses `resultCode` as the error *message* (`"Error"`) and joins every
/// `messages.message[].text` into the reason.
fn error_code_message_and_reason(messages: &ResponseMessages) -> (String, String, String) {
    let code = messages
        .message
        .first()
        .map(|m| m.code.clone())
        .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string());
    let joined_reason = messages
        .message
        .iter()
        .map(|m| m.text.clone())
        .collect::<Vec<String>>()
        .join(" ");
    let reason = if joined_reason.is_empty() {
        consts::NO_ERROR_MESSAGE.to_string()
    } else {
        joined_reason
    };
    (code, messages.result_code.to_string(), reason)
}

/// Build an `ErrorResponse` from Authorize.Net's `ResponseMessages` for PSync and RSync,
/// mirroring hyperswitch's `get_err_response`: the first message's code is the error code,
/// its text is both message and reason, and `attempt_status` stays `None` so the caller
/// preserves the status it already holds.
fn get_err_response(status_code: u16, messages: ResponseMessages) -> ErrorResponse {
    let first_message = messages.message.into_iter().next();
    let (code, message) = match first_message {
        Some(first_message) => (first_message.code, first_message.text),
        None => (
            consts::NO_ERROR_CODE.to_string(),
            consts::NO_ERROR_MESSAGE.to_string(),
        ),
    };
    ErrorResponse {
        code,
        message: message.clone(),
        reason: Some(message),
        status_code,
        attempt_status: None,
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

// Implementation for PSync flow
impl<F> TryFrom<ResponseRouterData<AuthorizedotnetPSyncResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        value: ResponseRouterData<AuthorizedotnetPSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        // No need to transform the response since we're using the direct structure
        // Use the clean approach with the From trait implementation
        match response.transaction {
            Some(transaction) => {
                let payment_status = AttemptStatus::from(transaction.transaction_status);

                // Create a new RouterDataV2 with updated fields
                let mut new_router_data = router_data;

                // Update the status in resource_common_data
                let mut resource_common_data = new_router_data.resource_common_data.clone();
                resource_common_data.status = payment_status;
                new_router_data.resource_common_data = resource_common_data;

                // Set the response
                new_router_data.response = Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        transaction.transaction_id.clone(),
                    ),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: Some(transaction.transaction_id.clone()),
                    incremental_authorization_allowed: None,
                    status_code: http_code,
                    splits: None,
                    payment_account_reference: None,
                });

                Ok(new_router_data)
            }
            None => {
                // A psync poll with no transaction record. `E00053` ("server too
                // busy") and `E00104` ("server in maintenance") are not payment
                // failures - Authorize.Net is asking us to retry later.
                //
                // Hyperswitch returns `Ok(item.data)` unchanged, keeping the prior
                // attempt status, because its psync `item.data.response` is a
                // `TransactionResponse` seeded from the stored attempt
                // (`construct_payment_router_data`) and `item.data.status` is the
                // stored `payment_attempt.status`. This connector is stateless:
                // `router_data` is built with `response: Err(ErrorResponse::default())`
                // and no prior status, so returning it unchanged surfaces as
                // `HE_00 / 500 / PAYMENT_STATUS_UNSPECIFIED` (a spurious error).
                //
                // Emit instead an `Ok(TransactionResponse)` with `AttemptStatus::Unknown`
                // ("polled, could not infer") and `status_code = http_code` (200). It
                // serializes to `PaymentStatus::Unspecified`, which hyperswitch maps
                // back to the previous attempt status on psync
                // (`unified_connector_service/transformers.rs`,
                // `PaymentStatus::Unspecified => Ok(prev_status)`) - matching the
                // direct connector leg. Any other message code is a real error.
                match response.messages.message.iter().find(|m| {
                    m.code == TRANSIENT_ERROR_SERVER_BUSY || m.code == TRANSIENT_ERROR_MAINTENANCE
                }) {
                    Some(_) => {
                        let connector_transaction_id = router_data
                            .request
                            .connector_transaction_id
                            .get_connector_transaction_id()
                            .ok();
                        let mut new_router_data = router_data;
                        new_router_data.resource_common_data.status = AttemptStatus::Unknown;
                        new_router_data.response = Ok(PaymentsResponseData::TransactionResponse {
                            resource_id: connector_transaction_id
                                .clone()
                                .map(ResponseId::ConnectorTransactionId)
                                .unwrap_or(ResponseId::NoResponseId),
                            redirection_data: None,
                            mandate_reference: None,
                            connector_metadata: None,
                            network_txn_id: None,
                            network_txn_link_id: None,
                            connector_response_reference_id: connector_transaction_id,
                            incremental_authorization_allowed: None,
                            status_code: http_code,
                            splits: None,
                            payment_account_reference: None,
                        });
                        Ok(new_router_data)
                    }
                    None => Ok(Self {
                        response: Err(get_err_response(http_code, response.messages)),
                        ..router_data
                    }),
                }
            }
        }
    }
}

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub enum AuthorizedotnetPaymentStatus {
    #[serde(rename = "1")]
    Approved,
    #[serde(rename = "2")]
    Declined,
    #[serde(rename = "3")]
    Error,
    #[serde(rename = "4")]
    #[default]
    HeldForReview,
    #[serde(rename = "5")]
    RequiresAction, // Maps to hyperswitch_common_enums::enums::AttemptStatus::AuthenticationPending
    /// Any responseCode the API reference does not list. Parsed instead of failing
    /// the whole body, and mapped to a non-terminal status so the caller keeps
    /// the status it already holds.
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub enum AuthorizedotnetRefundStatus {
    #[serde(rename = "1")]
    Approved,
    #[serde(rename = "2")]
    Declined,
    #[serde(rename = "3")]
    Error,
    #[serde(rename = "4")]
    #[default]
    HeldForReview,
    /// Any responseCode the API reference does not list; see
    /// `AuthorizedotnetPaymentStatus::Unknown`.
    #[serde(other)]
    Unknown,
}

/// Helper function to extract error code and message from response
fn extract_error_details(
    response: &AuthorizedotnetPaymentsResponse,
    trans_res: Option<&AuthorizedotnetTransactionResponse>,
) -> (String, String) {
    let error_code = trans_res
        .and_then(|tr| {
            tr.errors
                .as_ref()
                .and_then(|e| e.first().map(|e| e.error_code.clone()))
        })
        .or_else(|| response.messages.message.first().map(|m| m.code.clone()))
        .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string());

    let error_message = trans_res
        .and_then(|tr| {
            tr.errors
                .as_ref()
                .and_then(|e| e.first().map(|e| e.error_text.clone()))
        })
        .or_else(|| response.messages.message.first().map(|m| m.text.clone()))
        .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string());

    (error_code, error_message)
}

/// Helper function to create error response
fn create_error_response(
    http_status_code: u16,
    error_code: String,
    error_message: String,
    status: AttemptStatus,
    connector_transaction_id: Option<String>,
    transaction_response: Option<&AuthorizedotnetTransactionResponse>,
    _raw_connector_response: Option<Secret<String>>,
) -> ErrorResponse {
    // Issuer-side detail of a failed transaction, each only when the gateway
    // returned it: merchantAdvice.code / .description (merchant advice code, TSYS
    // only) and rawResponseCode (processor response code).
    let merchant_advice =
        transaction_response.and_then(|response| response.merchant_advice.as_ref());
    let network_advice_code = merchant_advice
        .and_then(|advice| non_empty_code(advice.code.as_ref()))
        .map(str::to_string);
    let network_error_message = merchant_advice
        .and_then(|advice| non_empty_code(advice.description.as_ref()))
        .map(str::to_string);
    let network_decline_code = transaction_response
        .and_then(|response| non_empty_code(response.raw_response_code.as_ref()))
        .map(str::to_string);

    ErrorResponse {
        status_code: http_status_code,
        code: error_code,
        message: error_message.clone(),
        reason: Some(error_message),
        attempt_status: Some(FlowStatus::Payment(status)),
        connector_transaction_id,
        network_decline_code,
        network_advice_code,
        network_error_message,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
}

impl From<AuthorizedotnetRefundStatus> for RefundStatus {
    fn from(item: AuthorizedotnetRefundStatus) -> Self {
        match item {
            AuthorizedotnetRefundStatus::Declined | AuthorizedotnetRefundStatus::Error => {
                Self::Failure
            }
            AuthorizedotnetRefundStatus::Approved | AuthorizedotnetRefundStatus::HeldForReview => {
                Self::Pending
            }
            // Undocumented responseCode: not a verdict, so the refund is left for sync.
            AuthorizedotnetRefundStatus::Unknown => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorMessage {
    pub error_code: String,
    pub error_text: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct AuthorizedotnetTransactionResponseError {
    _supplemental_data_qualification_indicator: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SecureAcceptance {
    // Define fields for SecureAcceptance if it's actually used and its structure is known
}

#[derive(Debug, Default, Clone, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseMessage {
    pub code: String,
    pub text: String,
}

#[derive(Debug, Default, Clone, Deserialize, PartialEq, Serialize, strum::Display)]
#[serde(rename_all = "PascalCase")]
pub enum ResultCode {
    #[default]
    Ok,
    Error,
    /// Any resultCode the API reference does not list; parsed instead of failing the
    /// whole body.
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseMessages {
    result_code: ResultCode,
    pub message: Vec<ResponseMessage>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetPaymentsResponse {
    pub transaction_response: Option<TransactionResponse>,
    pub profile_response: Option<AuthorizedotnetNonZeroMandateResponse>,
    pub messages: ResponseMessages,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetNonZeroMandateResponse {
    customer_profile_id: Option<String>,
    customer_payment_profile_id_list: Option<Vec<String>>,
    // Never read. Optional so a profileResponse without it cannot fail the body of a
    // transaction the gateway has already processed.
    pub messages: Option<ResponseMessages>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Operation {
    Authorize,
    Capture,
    Void,
    VoidPostCapture,
    Refund,
}

fn get_hs_status(
    response: &AuthorizedotnetPaymentsResponse,
    _http_status_code: u16,
    operation: Operation,
    capture_method: Option<enums::CaptureMethod>,
) -> AttemptStatus {
    // A failed void is reported with the void's own status, not the payment status
    // `Failure`: the authorization it targeted is still open.
    let failure_status = match operation {
        Operation::Void => AttemptStatus::VoidFailed,
        Operation::Authorize
        | Operation::Capture
        | Operation::VoidPostCapture
        | Operation::Refund => AttemptStatus::Failure,
    };

    // Return failure immediately if result code is Error
    if response.messages.result_code == ResultCode::Error {
        return failure_status;
    }

    // An unlisted resultCode is not a verdict of its own: `transactionResponse` below
    // carries the transaction's outcome and stays authoritative (as in hyperswitch, which
    // only guards against `Error`), so an approved transaction is not thrown away. Logged
    // because an unlisted code means the API reference and this mapping have drifted.
    if response.messages.result_code == ResultCode::Unknown {
        tracing::warn!(
            ?operation,
            "Authorize.Net returned a resultCode the API reference does not list; \
             falling through to transactionResponse.responseCode"
        );
    }

    // Handle case when transaction_response is None
    if response.transaction_response.is_none() {
        return match operation {
            // A void response always carries `transactionResponse` unless the API call
            // itself failed, so a body without one is never a completed void. The
            // pre-GRACE code returned `Voided` here, reporting a void the gateway never
            // confirmed; hyperswitch reports this shape as a failure carrying the
            // gateway messages (its void arm returns `Failure`), and prism keeps the
            // more specific `VoidFailed` because the authorization is still open.
            Operation::Void => AttemptStatus::VoidFailed,
            Operation::VoidPostCapture => AttemptStatus::VoidedPostCapture,
            // No transaction was created: a failed attempt, as in hyperswitch.
            Operation::Authorize | Operation::Capture | Operation::Refund => AttemptStatus::Failure,
        };
    }

    // Now handle transaction_response cases
    // Safety: transaction_response is checked above to be Some
    match response.transaction_response.as_ref() {
        Some(trans_resp) => match trans_resp {
            TransactionResponse::AuthorizedotnetTransactionResponseError(_) => failure_status,
            TransactionResponse::AuthorizedotnetTransactionResponse(trans_res) => {
                match trans_res.response_code {
                    // transactionResponse.responseCode: 2 = Declined, 3 = Error.
                    AuthorizedotnetPaymentStatus::Declined
                    | AuthorizedotnetPaymentStatus::Error => failure_status,
                    // responseCode 4 = Held for Review: accepted, not terminal.
                    AuthorizedotnetPaymentStatus::HeldForReview => match operation {
                        Operation::Void => AttemptStatus::VoidInitiated,
                        Operation::Authorize
                        | Operation::Capture
                        | Operation::VoidPostCapture
                        | Operation::Refund => AttemptStatus::Unresolved,
                    },
                    AuthorizedotnetPaymentStatus::RequiresAction => {
                        AttemptStatus::AuthenticationPending
                    }
                    // Undocumented responseCode, for every operation: no verdict.
                    // `Unspecified` lets the caller keep its stored status.
                    AuthorizedotnetPaymentStatus::Unknown => AttemptStatus::Unspecified,
                    AuthorizedotnetPaymentStatus::Approved => {
                        // For Approved status, determine specific status based on operation and capture method
                        match operation {
                            Operation::Authorize => match capture_method {
                                Some(enums::CaptureMethod::Manual) => AttemptStatus::Authorized,
                                _ => AttemptStatus::Charged, // Automatic or None defaults to Charged
                            },
                            Operation::Capture | Operation::Refund => AttemptStatus::Charged,
                            Operation::Void => AttemptStatus::Voided,
                            Operation::VoidPostCapture => AttemptStatus::VoidedPostCapture,
                        }
                    }
                }
            }
        },
        None => failure_status,
    }
}

// Simple structs for connector_metadata (no validation, accepts masked cards like "XXXX2346")
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConnectorMetadataCreditCard {
    card_number: Secret<String>,
    expiration_date: Secret<String>,
}

#[derive(Debug, Serialize)]
struct ConnectorMetadataPayment {
    #[serde(rename = "creditCard")]
    credit_card: ConnectorMetadataCreditCard,
}

// Build connector_metadata from transaction response
// Uses simple structs without validation to handle masked card numbers like "XXXX2346"
fn build_connector_metadata(
    transaction_response: &AuthorizedotnetTransactionResponse,
) -> Option<serde_json::Value> {
    let card_number = transaction_response
        .account_number
        .as_ref()?
        .peek()
        .to_string();

    let payment = ConnectorMetadataPayment {
        credit_card: ConnectorMetadataCreditCard {
            card_number: Secret::new(card_number),
            expiration_date: Secret::new("XXXX".to_string()),
        },
    };

    serde_json::to_value(payment)
        .inspect_err(|e| {
            tracing::warn!(
                error = %e,
                "Failed to serialize connector_metadata payment"
            );
        })
        .ok()
}

type PaymentConversionResult = (
    AttemptStatus,
    Result<PaymentsResponseData, ErrorResponse>,
    Option<domain_types::router_data::ConnectorResponseData>,
);

pub fn convert_to_payments_response_data_or_error(
    response: &AuthorizedotnetPaymentsResponse,
    http_status_code: u16,
    operation: Operation,
    capture_method: Option<enums::CaptureMethod>,
    raw_connector_response: Option<Secret<String>>,
) -> PaymentConversionResult {
    let status = get_hs_status(response, http_status_code, operation, capture_method);

    // `Unresolved` (authorize.net responseCode "4" - held for review) is an accepted,
    // non-terminal state: authorize.net returns no `transaction_response.errors` for it, so
    // hyperswitch's direct path yields `response = Ok(TransactionResponse)`. Match that here
    // instead of wrapping it as an `ErrorResponse`.
    let is_successful_status = matches!(
        status,
        AttemptStatus::Authorized
            | AttemptStatus::Pending
            | AttemptStatus::AuthenticationPending
            | AttemptStatus::Charged
            | AttemptStatus::Voided
            | AttemptStatus::VoidedPostCapture
            | AttemptStatus::Unresolved
            // A void held for review (responseCode "4") is accepted and non-terminal too.
            | AttemptStatus::VoidInitiated
            // An undocumented responseCode is returned as Ok, never as a failure.
            | AttemptStatus::Unspecified
    );

    // Extract connector response data from transaction response if available
    let connector_response_data = match &response.transaction_response {
        Some(TransactionResponse::AuthorizedotnetTransactionResponse(trans_res)) => {
            convert_to_additional_payment_method_connector_response(trans_res)
                .map(domain_types::router_data::ConnectorResponseData::with_additional_payment_method_data)
        }
        _ => None
};

    let response_payload_result = match &response.transaction_response {
        Some(TransactionResponse::AuthorizedotnetTransactionResponse(trans_res))
            if is_successful_status =>
        {
            let connector_metadata = build_connector_metadata(trans_res);

            if operation == Operation::VoidPostCapture {
                Ok(PaymentsResponseData::PostCaptureVoidResponse {
                    post_capture_void_status: common_enums::PostCaptureVoidStatus::Succeeded,
                    connector_reference_id: Some(trans_res.transaction_id.clone()),
                    description: None,
                    status_code: http_status_code,
                })
            } else {
                // Extract mandate_reference from profile_response if available
                let mandate_reference =
                    response.profile_response.as_ref().map(|profile_response| {
                        let payment_profile_id = profile_response
                            .customer_payment_profile_id_list
                            .as_ref()
                            .and_then(|list| list.first().cloned());

                        MandateReference {
                            connector_mandate_id: profile_response
                                .customer_profile_id
                                .as_ref()
                                .and_then(|customer_profile_id| {
                                    payment_profile_id.map(|payment_profile_id| {
                                        format!("{customer_profile_id}-{payment_profile_id}")
                                    })
                                }),
                            payment_method_id: None,
                            connector_mandate_request_reference_id: None,
                            mandate_metadata: None,
                        }
                    });

                Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        trans_res.transaction_id.clone(),
                    ),
                    redirection_data: None,
                    connector_metadata,
                    mandate_reference: mandate_reference.map(Box::new),
                    network_txn_id: trans_res
                        .network_trans_id
                        .as_ref()
                        .map(|s| s.peek().clone()),
                    network_txn_link_id: None,
                    connector_response_reference_id: Some(trans_res.transaction_id.clone()),
                    incremental_authorization_allowed: None,
                    status_code: http_status_code,
                    splits: None,
                    payment_account_reference: None,
                })
            }
        }
        Some(TransactionResponse::AuthorizedotnetTransactionResponse(trans_res)) => {
            // Failure status or other non-successful statuses
            let (error_code, error_message) = extract_error_details(response, Some(trans_res));
            Err(create_error_response(
                http_status_code,
                error_code,
                error_message,
                status,
                Some(trans_res.transaction_id.clone()),
                Some(trans_res),
                raw_connector_response.clone(),
            ))
        }
        Some(TransactionResponse::AuthorizedotnetTransactionResponseError(_)) => {
            let (error_code, error_message) = extract_error_details(response, None);
            Err(create_error_response(
                http_status_code,
                error_code,
                error_message,
                status,
                None,
                None,
                raw_connector_response.clone(),
            ))
        }
        None if status == AttemptStatus::VoidedPostCapture
            && operation == Operation::VoidPostCapture =>
        {
            Ok(PaymentsResponseData::PostCaptureVoidResponse {
                post_capture_void_status: common_enums::PostCaptureVoidStatus::Succeeded,
                connector_reference_id: None,
                description: None,
                status_code: http_status_code,
            })
        }
        None => {
            let (error_code, error_message) = extract_error_details(response, None);
            Err(create_error_response(
                http_status_code,
                error_code,
                error_message,
                status,
                None,
                None,
                raw_connector_response.clone(),
            ))
        }
    };

    (status, response_payload_result, connector_response_data)
}

// Transaction details for sync response used in PSync implementation

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SyncStatus {
    CapturedPendingSettlement,
    SettledSuccessfully,
    AuthorizedPendingCapture,
    Declined,
    Voided,
    CouldNotVoid,
    GeneralError,
    RefundSettledSuccessfully,
    RefundPendingSettlement,
    #[serde(rename = "FDSPendingReview")]
    FDSPendingReview,
    #[serde(rename = "FDSAuthorizedPendingReview")]
    FDSAuthorizedPendingReview,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncTransactionResponse {
    #[serde(rename = "transId")]
    pub transaction_id: String,
    #[serde(rename = "transactionStatus")]
    pub transaction_status: SyncStatus,
    // Authorize.Net reason codes run past 255 (e.g. 315 invalid card number, 318
    // duplicate transaction), so `u8` overflows and fails the whole psync body.
    // Hyperswitch does not model these fields at all and so never fails on them;
    // they are carried here only for `typed_connector_response`, never read.
    pub response_code: Option<i32>,
    pub response_reason_code: Option<i32>,
    pub response_reason_description: Option<String>,
    pub network_trans_id: Option<String>,
    // Additional fields available but not needed for our implementation
}

impl From<SyncStatus> for AttemptStatus {
    fn from(transaction_status: SyncStatus) -> Self {
        match transaction_status {
            SyncStatus::SettledSuccessfully | SyncStatus::CapturedPendingSettlement => {
                Self::Charged
            }
            SyncStatus::AuthorizedPendingCapture => Self::Authorized,
            SyncStatus::Declined => Self::AuthenticationFailed,
            SyncStatus::Voided => Self::Voided,
            SyncStatus::CouldNotVoid => Self::VoidFailed,
            SyncStatus::GeneralError => Self::Failure,
            SyncStatus::RefundSettledSuccessfully | SyncStatus::RefundPendingSettlement => {
                Self::Charged
            }
            SyncStatus::FDSPendingReview | SyncStatus::FDSAuthorizedPendingReview => {
                Self::Unresolved
            }
            // This service is stateless - there is no prior attempt status to fall back
            // on, so an unrecognised `transactionStatus` must not resolve to a concrete
            // status. `Unspecified` serializes to `PaymentStatus::Unspecified`, which
            // hyperswitch maps back to the stored status
            // (`unified_connector_service/transformers.rs`,
            // `PaymentStatus::Unspecified => Ok(prev_status)`), so the retention happens
            // on the side that actually holds the attempt.
            SyncStatus::Unknown => Self::Unspecified,
        }
    }
}

// Removing duplicate implementation

// RSync related types for Refund Sync
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RSyncStatus {
    RefundSettledSuccessfully,
    RefundPendingSettlement,
    Declined,
    GeneralError,
    Voided,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RSyncTransactionResponse {
    #[serde(rename = "transId")]
    transaction_id: String,
    transaction_status: RSyncStatus,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthorizedotnetRSyncResponse {
    transaction: Option<RSyncTransactionResponse>,
    messages: ResponseMessages,
}

impl From<RSyncStatus> for RefundStatus {
    fn from(transaction_status: RSyncStatus) -> Self {
        match transaction_status {
            RSyncStatus::RefundSettledSuccessfully => Self::Success,
            RSyncStatus::RefundPendingSettlement => Self::Pending,
            RSyncStatus::Declined | RSyncStatus::GeneralError | RSyncStatus::Voided => {
                Self::Failure
            }
            // Refund analogue of the `SyncStatus::Unknown` arm above: `RefundStatus::Unknown`
            // serializes to proto `RefundStatus::Unspecified`, which hyperswitch maps back to
            // the stored refund status.
            RSyncStatus::Unknown => Self::Unknown,
        }
    }
}

impl TryFrom<ResponseRouterData<AuthorizedotnetRSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        value: ResponseRouterData<AuthorizedotnetRSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        match response.transaction {
            Some(transaction) => {
                let refund_status = RefundStatus::from(transaction.transaction_status);

                // Create a new RouterDataV2 with updated fields
                let mut new_router_data = router_data;

                // Update the status in resource_common_data
                let mut resource_common_data = new_router_data.resource_common_data.clone();
                resource_common_data.status = refund_status;
                new_router_data.resource_common_data = resource_common_data;

                // Set the response
                new_router_data.response = Ok(RefundsResponseData {
                    connector_refund_id: transaction.transaction_id,
                    refund_status,
                    status_code: http_code,
                    acquirer_reference_number: None,
                });

                Ok(new_router_data)
            }
            // A sync that returned no transaction (record not found, server busy,
            // auth error) says nothing about the refund itself: surface the error and
            // leave the refund status untouched, as hyperswitch and the PSync arm do.
            None => Ok(Self {
                response: Err(get_err_response(http_code, response.messages)),
                ..router_data
            }),
        }
    }
}

// SetupMandate (Zero Mandate) implementation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AuthorizedotnetRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AuthorizedotnetSetupMandateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: AuthorizedotnetRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, error_stack::Report<IntegrationError>> {
        let merchant_authentication =
            AuthorizedotnetAuthType::try_from(&item.router_data.connector_config)?;

        let validation_mode = match item.router_data.resource_common_data.test_mode {
            Some(true) | None => ValidationMode::TestMode,
            Some(false) => ValidationMode::LiveMode,
        };
        let customer_profile_id = item
            .router_data
            .resource_common_data
            .connector_customer
            .as_ref()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "connector_customer_id is missing",
                context: Default::default(),
            })?
            .clone();

        // Build billing address if present - use get_optional_billing() method
        let bill_to = item
            .router_data
            .resource_common_data
            .get_optional_billing()
            .and_then(|billing| billing.address.as_ref())
            .map(|address| BillTo {
                first_name: address.first_name.clone(),
                last_name: address.last_name.clone(),
                address: get_address_line(&address.line1, &address.line2, &address.line3),
                city: address.city.clone(),
                state: address.state.clone(),
                zip: address.zip.clone(),
                country: address.country,
                // The stored payment profile keeps its existing billTo shape.
                phone_number: None,
                email: None,
            });

        let payment = match &item.router_data.request.payment_method_data {
            PaymentMethodData::Card(ccard) => {
                // Create expiry date manually since we can't use the trait method generically
                let expiry_month = ccard.card_exp_month.peek().clone();
                let expiry_year =
                    domain_types::utils::expand_expiry_year_to_four_digits(&ccard.card_exp_year);
                let expiration_date = format!("{}-{expiry_month}", expiry_year.peek());

                PaymentDetails::CreditCard(CreditCardDetails {
                    card_number: ccard.card_number.clone(),
                    expiration_date: Secret::new(expiration_date),
                    card_code: Some(ccard.card_cvc.clone()),
                })
            }
            PaymentMethodData::Wallet(wallet_data) => PaymentDetails::try_from(wallet_data)?,
            pm => {
                return Err(error_stack::report!(IntegrationError::NotImplemented(
                    format!("Payment method {:?}", pm),
                    Default::default()
                )))
            }
        };

        let payment_profile = PaymentProfile { bill_to, payment };

        Ok(Self {
            create_customer_payment_profile_request: AuthorizedotnetPaymentProfileRequest {
                merchant_authentication,
                customer_profile_id: Secret::new(customer_profile_id),
                payment_profile,
                validation_mode,
            },
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetCreateConnectorCustomerResponse {
    pub customer_profile_id: Option<String>,
    pub customer_payment_profile_id_list: Option<Vec<String>>,
    pub validation_direct_response_list: Option<Vec<Secret<String>>>,
    pub messages: ResponseMessages,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<AuthorizedotnetSetupMandateResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        value: ResponseRouterData<AuthorizedotnetSetupMandateResponse, Self>,
    ) -> Result<Self, error_stack::Report<ConnectorError>> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        // Get connector customer ID from resource_common_data - we need it to build mandate reference
        let connector_customer_id = router_data
            .resource_common_data
            .connector_customer
            .as_ref()
            .ok_or_else(|| {
                error_stack::report!(ConnectorError::ResponseHandlingFailed {
                    context: domain_types::errors::ResponseTransformationErrorContext {
                        http_status_code: Some(http_code),
                        additional_context: Some("connector_customer_id required to build mandate reference for authorizedotnet".to_string()),
                    },
                })
            })?
            .clone();

        // The status follows the outcome returned below (hyperswitch's rule): a
        // customerProfileId means the profile is available for use - this covers
        // E00039 "A duplicate customer payment profile already exists", which is
        // acceptable for an idempotent SetupMandate - and its absence is a failure
        // whatever the resultCode.
        let status = if response.customer_profile_id.is_some() {
            AttemptStatus::Charged
        } else {
            AttemptStatus::Failure
        };

        let mut new_router_data = router_data;
        let mut resource_common_data = new_router_data.resource_common_data.clone();
        resource_common_data.status = status;
        new_router_data.resource_common_data = resource_common_data;

        if response.customer_profile_id.is_some() {
            // {customer_profile_id}-{payment_profile_id}, and `None` when the profile came
            // back without a payment profile to pair with it. Hyperswitch leaves the mandate
            // id empty there instead of failing the transformation, so a later MIT fails on a
            // missing mandate rather than this response failing as a deserialization error.
            let connector_mandate_id = response
                .customer_payment_profile_id_list
                .first()
                .or(response.customer_payment_profile_id.as_ref())
                .map(|payment_profile_id| format!("{connector_customer_id}-{payment_profile_id}"));

            new_router_data.response = Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::NoResponseId,
                redirection_data: None,
                connector_metadata: None,
                mandate_reference: Some(Box::new(MandateReference {
                    connector_mandate_id,
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                    mandate_metadata: None,
                })),
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: http_code,
                splits: None,
                payment_account_reference: None,
            });
        } else {
            let (code, message, reason) = error_code_message_and_reason(&response.messages);
            let error_response = ErrorResponse {
                status_code: http_code,
                code,
                message,
                reason: Some(reason),
                attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
                connector_transaction_id: None,
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            };
            new_router_data.response = Err(error_response);
        }

        Ok(new_router_data)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetErrorResponse {
    pub messages: ResponseMessages,
}

// Webhook-related structures
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetWebhookObjectId {
    pub webhook_id: Option<String>,
    pub event_type: AuthorizedotnetWebhookEvent,
    pub payload: AuthorizedotnetWebhookPayload,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetWebhookPayload {
    pub id: Option<String>,
    /// `payload.merchantReferenceId`: echo of the request `refId`; absent when no
    /// `refId` was sent (techspec "IncomingWebhook").
    pub merchant_reference_id: Option<String>,
    // Fields specific to customer creation webhooks
    pub payment_profiles: Option<Vec<PaymentProfileInfo>>,
    pub merchant_customer_id: Option<String>,
    pub description: Option<String>,
    pub entity_name: Option<String>,
    // Fields specific to customer payment profile creation webhooks
    pub customer_profile_id: Option<u64>,
    pub customer_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentProfileInfo {
    pub id: String,
    pub customer_type: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetWebhookEventType {
    pub event_type: AuthorizedotnetIncomingWebhookEventType,
}

#[derive(Debug, Clone, Deserialize)]
pub enum AuthorizedotnetWebhookEvent {
    #[serde(rename = "net.authorize.payment.authorization.created")]
    AuthorizationCreated,
    #[serde(rename = "net.authorize.payment.priorAuthCapture.created")]
    PriorAuthCapture,
    #[serde(rename = "net.authorize.payment.authcapture.created")]
    AuthCapCreated,
    #[serde(rename = "net.authorize.payment.capture.created")]
    CaptureCreated,
    #[serde(rename = "net.authorize.payment.void.created")]
    VoidCreated,
    #[serde(rename = "net.authorize.payment.refund.created")]
    RefundCreated,
    #[serde(rename = "net.authorize.customer.created")]
    CustomerCreated,
    #[serde(rename = "net.authorize.customer.paymentProfile.created")]
    CustomerPaymentProfileCreated,
    #[serde(other)]
    Unknown,
}

/// Including Unknown to map unknown webhook events
#[derive(Debug, Clone, Deserialize)]
pub enum AuthorizedotnetIncomingWebhookEventType {
    #[serde(rename = "net.authorize.payment.authorization.created")]
    AuthorizationCreated,
    #[serde(rename = "net.authorize.payment.priorAuthCapture.created")]
    PriorAuthCapture,
    #[serde(rename = "net.authorize.payment.authcapture.created")]
    AuthCapCreated,
    #[serde(rename = "net.authorize.payment.capture.created")]
    CaptureCreated,
    #[serde(rename = "net.authorize.payment.void.created")]
    VoidCreated,
    #[serde(rename = "net.authorize.payment.refund.created")]
    RefundCreated,
    #[serde(rename = "net.authorize.customer.created")]
    CustomerCreated,
    #[serde(rename = "net.authorize.customer.paymentProfile.created")]
    CustomerPaymentProfileCreated,
    #[serde(other)]
    Unknown,
}

impl From<AuthorizedotnetIncomingWebhookEventType> for interfaces::webhooks::IncomingWebhookEvent {
    fn from(event_type: AuthorizedotnetIncomingWebhookEventType) -> Self {
        match event_type {
            AuthorizedotnetIncomingWebhookEventType::AuthorizationCreated
            | AuthorizedotnetIncomingWebhookEventType::PriorAuthCapture
            | AuthorizedotnetIncomingWebhookEventType::AuthCapCreated
            | AuthorizedotnetIncomingWebhookEventType::CaptureCreated
            | AuthorizedotnetIncomingWebhookEventType::VoidCreated
            | AuthorizedotnetIncomingWebhookEventType::CustomerCreated
            | AuthorizedotnetIncomingWebhookEventType::CustomerPaymentProfileCreated => {
                Self::PaymentIntentSuccess
            }
            AuthorizedotnetIncomingWebhookEventType::RefundCreated => Self::RefundSuccess,
            AuthorizedotnetIncomingWebhookEventType::Unknown => Self::EventNotSupported,
        }
    }
}

impl From<AuthorizedotnetWebhookEvent> for AttemptStatus {
    // status mapping reference https://developer.authorize.net/api/reference/features/webhooks.html#Event_Types_and_Payloads
    fn from(event_type: AuthorizedotnetWebhookEvent) -> Self {
        match event_type {
            AuthorizedotnetWebhookEvent::AuthorizationCreated => Self::Authorized,
            AuthorizedotnetWebhookEvent::CaptureCreated
            | AuthorizedotnetWebhookEvent::AuthCapCreated
            | AuthorizedotnetWebhookEvent::PriorAuthCapture => Self::Charged,
            AuthorizedotnetWebhookEvent::VoidCreated => Self::Voided,
            AuthorizedotnetWebhookEvent::RefundCreated => Self::PartialCharged, // This will be used for refund status
            AuthorizedotnetWebhookEvent::CustomerCreated => Self::Charged, // Customer profile creation indicates successful setup mandate
            AuthorizedotnetWebhookEvent::CustomerPaymentProfileCreated => Self::Charged, // Payment profile creation indicates successful setup mandate
            // Not a payment event: no verdict, the caller keeps its stored status.
            AuthorizedotnetWebhookEvent::Unknown => Self::Unspecified,
        }
    }
}

impl From<AuthorizedotnetWebhookEvent> for SyncStatus {
    // status mapping reference https://developer.authorize.net/api/reference/features/webhooks.html#Event_Types_and_Payloads
    fn from(event_type: AuthorizedotnetWebhookEvent) -> Self {
        match event_type {
            AuthorizedotnetWebhookEvent::AuthorizationCreated => Self::AuthorizedPendingCapture,
            AuthorizedotnetWebhookEvent::CaptureCreated
            | AuthorizedotnetWebhookEvent::AuthCapCreated => Self::CapturedPendingSettlement,
            AuthorizedotnetWebhookEvent::PriorAuthCapture => Self::SettledSuccessfully,
            AuthorizedotnetWebhookEvent::VoidCreated => Self::Voided,
            AuthorizedotnetWebhookEvent::RefundCreated => Self::RefundSettledSuccessfully,
            AuthorizedotnetWebhookEvent::CustomerCreated => Self::SettledSuccessfully, // Customer profile successfully created and settled
            AuthorizedotnetWebhookEvent::CustomerPaymentProfileCreated => Self::SettledSuccessfully, // Payment profile successfully created and settled
            AuthorizedotnetWebhookEvent::Unknown => Self::Unknown,
        }
    }
}

pub fn get_trans_id(details: &AuthorizedotnetWebhookObjectId) -> Result<String, WebhookError> {
    match details.event_type {
        AuthorizedotnetWebhookEvent::CustomerPaymentProfileCreated => {
            // For payment profile creation, use the customer_profile_id as the primary identifier
            if let Some(customer_profile_id) = details.payload.customer_profile_id {
                tracing::debug!(
                    target: "authorizedotnet_webhook",
                    "Extracted customer profile ID {} for payment profile creation webhook",
                    customer_profile_id
                );
                Ok(customer_profile_id.to_string())
            } else {
                match details.payload.id.clone() {
                    Some(id) => {
                        tracing::debug!(
                            target: "authorizedotnet_webhook",
                            "Extracted transaction ID {} from payment profile webhook payload",
                            id
                        );
                        Ok(id)
                    }
                    None => {
                        tracing::error!(
                            target: "authorizedotnet_webhook",
                            "No customer_profile_id or id found in CustomerPaymentProfileCreated webhook payload"
                        );
                        Err(WebhookError::WebhookReferenceIdNotFound)
                    }
                }
            }
        }
        _ => {
            // For all other events, use the standard id field
            match details.payload.id.clone() {
                Some(id) => {
                    tracing::debug!(
                        target: "authorizedotnet_webhook",
                        "Extracted transaction ID {} for webhook event type: {:?}",
                        id,
                        details.event_type
                    );
                    Ok(id)
                }
                None => {
                    tracing::error!(
                        target: "authorizedotnet_webhook",
                        "No transaction ID found in webhook payload for event type: {:?}",
                        details.event_type
                    );
                    Err(WebhookError::WebhookReferenceIdNotFound)
                }
            }
        }
    }
}

impl TryFrom<AuthorizedotnetWebhookObjectId> for AuthorizedotnetPSyncResponse {
    type Error = error_stack::Report<WebhookError>;
    fn try_from(item: AuthorizedotnetWebhookObjectId) -> Result<Self, Self::Error> {
        Ok(Self {
            transaction: Some(SyncTransactionResponse {
                transaction_id: get_trans_id(&item)?,
                transaction_status: SyncStatus::from(item.event_type),
                response_code: Some(1),
                response_reason_code: Some(1),
                response_reason_description: Some("Approved".to_string()),
                network_trans_id: None,
            }),
            messages: ResponseMessages {
                result_code: ResultCode::Ok,
                message: vec![ResponseMessage {
                    code: "I00001".to_string(),
                    text: "Successful.".to_string(),
                }],
            },
        })
    }
}

// Helper function to extract customer profile ID from error message
// Message format: "A duplicate record with ID 933042598 already exists."
fn extract_customer_id_from_error(error_text: &str) -> Option<String> {
    // Look for pattern "ID <numbers>"
    error_text
        .split_whitespace()
        .skip_while(|&word| word != "ID")
        .nth(1) // Get the word after "ID"
        .and_then(|id_str| {
            // Remove any trailing punctuation and validate it's numeric
            let cleaned = id_str.trim_end_matches(|c: char| !c.is_numeric());
            if cleaned.chars().all(char::is_numeric) && !cleaned.is_empty() {
                Some(cleaned.to_string())
            } else {
                None
            }
        })
}

// TryFrom implementations for CreateConnectorCustomer flow

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AuthorizedotnetRouterData<
            RouterDataV2<
                CreateConnectorCustomer,
                PaymentFlowData,
                ConnectorCustomerData,
                ConnectorCustomerResponse,
            >,
            T,
        >,
    > for AuthorizedotnetCreateConnectorCustomerRequest<T>
{
    type Error = Error;
    fn try_from(
        item: AuthorizedotnetRouterData<
            RouterDataV2<
                CreateConnectorCustomer,
                PaymentFlowData,
                ConnectorCustomerData,
                ConnectorCustomerResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let merchant_authentication =
            AuthorizedotnetAuthType::try_from(&item.router_data.connector_config)?;

        // Build ship_to_list from shipping address if available
        let ship_to_list = item
            .router_data
            .resource_common_data
            .address
            .get_shipping()
            .and_then(|shipping| {
                shipping.address.as_ref().map(|address| {
                    vec![ShipToList {
                        first_name: address.first_name.clone(),
                        last_name: address.last_name.clone(),
                        // hyperswitch direct sends line1 only for shipTo (unlike billTo, which
                        // does join the lines) -- joining here emitted a trailing space whenever
                        // line2 was present-but-empty.
                        address: address.line1.clone(),
                        city: address.city.clone(),
                        state: address.state.clone(),
                        zip: address.zip.clone(),
                        country: address.country,
                        phone_number: shipping
                            .phone
                            .as_ref()
                            .and_then(|phone| phone.number.clone()),
                    }]
                })
            });

        // Conditionally send merchant_customer_id (matching Hyperswitch parity)
        // Only send if customer_id exists and length <= MAX_ID_LENGTH (20 chars)
        let merchant_customer_id = validate_customer_id_length(
            item.router_data
                .request
                .customer_id
                .as_ref()
                .map(|id| id.peek().clone()),
        );

        // Create a customer profile without payment method (zero mandate)
        Ok(Self {
            create_customer_profile_request: AuthorizedotnetZeroMandateRequest {
                merchant_authentication,
                profile: Profile {
                    merchant_customer_id,
                    description: None,
                    email: item
                        .router_data
                        .request
                        .email
                        .as_ref()
                        .map(|e| e.peek().clone()),
                    payment_profiles: None,
                    ship_to_list,
                },
                validation_mode: None,
            },
        })
    }
}

impl TryFrom<ResponseRouterData<AuthorizedotnetCreateConnectorCustomerResponse, Self>>
    for RouterDataV2<
        CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    >
{
    type Error = ResponseError;
    fn try_from(
        value: ResponseRouterData<AuthorizedotnetCreateConnectorCustomerResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        let mut new_router_data = router_data;

        if let Some(profile_id) = response.customer_profile_id {
            // Success - return the connector customer ID
            new_router_data.response = Ok(ConnectorCustomerResponse {
                connector_customer_id: profile_id,
                status_code: http_code,
            });
        } else {
            // Check if this is a "duplicate customer" error (E00039)
            let first_error = response.messages.message.first();
            let error_code = first_error.map(|m| m.code.as_str()).unwrap_or("");
            let error_text = first_error.map(|m| m.text.as_str()).unwrap_or("");
            // Same hyperswitch-parity mapping as every other error path in this file:
            // message is the resultCode, reason joins every message text.
            let (parity_code, parity_message, parity_reason) =
                error_code_message_and_reason(&response.messages);

            if error_code == "E00039" {
                // Extract customer profile ID from error message
                // Message format: "A duplicate record with ID 933042598 already exists."
                if let Some(existing_profile_id) = extract_customer_id_from_error(error_text) {
                    tracing::info!(
                        "Customer profile already exists with ID: {}, treating as success",
                        existing_profile_id
                    );
                    new_router_data.response = Ok(ConnectorCustomerResponse {
                        connector_customer_id: existing_profile_id,
                        status_code: http_code,
                    });
                } else {
                    // Couldn't extract ID, return error
                    new_router_data.response = Err(ErrorResponse {
                        status_code: http_code,
                        code: parity_code,
                        message: parity_message,
                        reason: Some(parity_reason),
                        attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)), // Marking attempt as failure since we couldn't confirm existing profile ID
                        connector_transaction_id: None,
                        network_decline_code: None,
                        network_advice_code: None,
                        network_error_message: None,
                        typed_connector_response: None,
                        raw_connector_response: None,
                        raw_connector_request: None,
                        typed_connector_request: None,
                    });
                }
            } else {
                // Other error - return error response
                new_router_data.response = Err(ErrorResponse {
                    status_code: http_code,
                    code: parity_code,
                    message: parity_message,
                    reason: Some(parity_reason),
                    attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)), // Marking attempt as failure for non-duplicate errors
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                });
            }
        }

        Ok(new_router_data)
    }
}

// ================================================================================
// SDKSessionToken (ServerSessionAuthenticationToken) flow
//
// Maps to Authorize.Net `getMerchantDetailsRequest`, which returns the merchant's
// `publicClientKey`. That key (together with the API Login ID configured client-side)
// is the session credential the front-end Accept.js / AcceptUI SDK uses to tokenize
// card data. We surface the `publicClientKey` as the `session_token`.
// ================================================================================

#[skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetMerchantDetailsRequest {
    merchant_authentication: MerchantAuthentication,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetSdkSessionTokenRequest {
    get_merchant_details_request: GetMerchantDetailsRequest,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AuthorizedotnetRouterData<
            RouterDataV2<
                ServerSessionAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerSessionAuthenticationTokenRequestData,
                ServerSessionAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for AuthorizedotnetSdkSessionTokenRequest
{
    type Error = Error;

    fn try_from(
        item: AuthorizedotnetRouterData<
            RouterDataV2<
                ServerSessionAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerSessionAuthenticationTokenRequestData,
                ServerSessionAuthenticationTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let merchant_authentication =
            MerchantAuthentication::try_from(&item.router_data.connector_config)?;
        Ok(Self {
            get_merchant_details_request: GetMerchantDetailsRequest {
                merchant_authentication,
            },
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedotnetSdkSessionTokenResponse {
    pub public_client_key: Option<Secret<String>>,
    pub merchant_name: Option<String>,
    pub gateway_id: Option<String>,
    pub messages: ResponseMessages,
}

impl TryFrom<ResponseRouterData<AuthorizedotnetSdkSessionTokenResponse, Self>>
    for RouterDataV2<
        ServerSessionAuthenticationToken,
        MerchantAuthenticationFlowData,
        ServerSessionAuthenticationTokenRequestData,
        ServerSessionAuthenticationTokenResponseData,
    >
{
    type Error = ResponseError;

    fn try_from(
        item: ResponseRouterData<AuthorizedotnetSdkSessionTokenResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Authorize.Net returns HTTP 200 even for business errors; inspect resultCode.
        if response.messages.result_code == ResultCode::Error {
            let error = response.messages.message.first();
            let code = error
                .map(|m| m.code.clone())
                .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string());
            let message = error
                .map(|m| m.text.clone())
                .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string());
            return Ok(Self {
                response: Err(ErrorResponse {
                    code,
                    message: message.clone(),
                    reason: Some(message),
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..router_data.clone()
            });
        }

        let session_token = response
            .public_client_key
            .clone()
            .expose_option()
            .ok_or_else(|| {
                ResponseError::from(ConnectorError::response_handling_failed_with_context(
                    item.http_code,
                    Some("publicClientKey missing in Authorize.Net response".to_string()),
                ))
            })?;

        // This flow only issues a session token; no payment has been authorized yet, so the
        // attempt status is left unchanged (it is not Pending).
        Ok(Self {
            response: Ok(ServerSessionAuthenticationTokenResponseData { session_token }),
            ..router_data.clone()
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{AvsResultCode, CvvResultCode};

    /// A documented code parses to its variant and a code the reference does not list
    /// survives verbatim, so `payment_checks` still carries what the gateway sent.
    #[test]
    fn result_codes_round_trip_including_unlisted_values() {
        for (raw, reportable, has_description) in [
            ("Y", true, true),
            ("P", false, true),
            ("Q", true, false),
            ("", false, false),
        ] {
            let code: AvsResultCode = serde_json::from_value(serde_json::json!(raw))
                .expect("every string is a valid avsResultCode");
            assert_eq!(code.is_reportable(), reportable, "avs {raw}");
            assert_eq!(code.description().is_some(), has_description, "avs {raw}");
            assert_eq!(
                serde_json::to_value(&code).expect("serializable"),
                serde_json::json!(raw),
                "avs {raw}"
            );
        }

        for (raw, reportable, has_description) in
            [("M", true, true), ("Q", true, false), ("", false, false)]
        {
            let code: CvvResultCode = serde_json::from_value(serde_json::json!(raw))
                .expect("every string is a valid cvvResultCode");
            assert_eq!(code.is_reportable(), reportable, "cvv {raw}");
            assert_eq!(code.description().is_some(), has_description, "cvv {raw}");
            assert_eq!(
                serde_json::to_value(&code).expect("serializable"),
                serde_json::json!(raw),
                "cvv {raw}"
            );
        }
    }
}
