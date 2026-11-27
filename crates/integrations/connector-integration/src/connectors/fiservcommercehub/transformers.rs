use std::collections::HashMap;

use crate::{types::ResponseRouterData, utils::response_amount_to_money};
use base64::{engine::general_purpose, Engine};
use cards::CardNumber;
use common_enums::{AttemptStatus, AuthenticationType, RefundStatus};
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    crypto::{self, RsaOaepSha256, SignMessage},
    pii::SecretSerdeValue,
    request::Method,
    types::FloatMajorUnitForConnector,
    Email, FloatMajorUnit,
};
use domain_types::{
    connector_flow::{
        Authenticate, Authorize, Capture, IncrementalAuthorization, PSync, PostAuthenticate,
        PreAuthenticate, RSync, Refund, RepeatPayment, ServerAuthenticationToken, SetupMandate,
        Void, VoidPC,
    },
    connector_types::{
        ContinueRedirectionResponse, DeviceChannel, MandateIds, MandateReference,
        MandateReferenceId, PaymentFlowData, PaymentVoidData, PaymentsAuthenticateData,
        PaymentsAuthorizeData, PaymentsCancelPostCaptureData, PaymentsCaptureData,
        PaymentsIncrementalAuthorizationData, PaymentsPostAuthenticateData,
        PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData, ResponseId,
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
        SetupMandateRequestData,
    },
    errors,
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::{
        ApplePayPaymentData, CardWithNoCvc, GpayTokenizationData, PaymentMethodData,
        PaymentMethodDataTypes, RawCardNumber, WalletData,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_request_types::{AuthenticationData, BrowserInformation, CaptureIntegrityObject},
    router_response_types::RedirectForm,
    utils,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Mask, Maskable, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

pub(crate) const ACCESS_TOKEN_SEPARATOR: &str = "|||";
pub(crate) const TOKEN_SOURCE_TRANSARMOR: &str = "TRANSARMOR";
pub(crate) const TOKEN_SOURCE_NETWORK_TOKEN: &str = "NETWORK_TOKEN";
const MERCHANT_INVOICE_NUMBER_MAX_LEN: usize = 12;
const FISERV_PAYMENT_METHOD_ENCRYPTION_URL: &str =
    "https://developer.fiserv.com/product/CommerceHub/docs/Payment-Methods/Payment-Methods.mdx";
pub(crate) const FISERV_PAYMENT_AUTHENTICATION_URL: &str =
    "https://developer.fiserv.com/product/CommerceHub/docs/Developer-Resources/Authentication/Authentication.mdx";
pub(crate) const FISERV_CHARGES_API_VERSION_URL: &str = "https://developer.fiserv.com/product/CommerceHub/api/post/payments/v1/charges?branch=active&version=1.26.0602";
pub(crate) const FISERV_CANCEL_API_URL: &str =
    "https://developer.fiserv.com/product/CommerceHub/api/post/payments/v1/cancels?branch=active";
pub(crate) const FISERV_INCREMENTAL_AUTH_DOC_URL: &str =
    "https://developer.fiserv.com/product/CommerceHub/docs/Payment-Operations/Accept-Payments/Incremental-Auth.mdx?branch=active";
const FISERV_TOKEN_API_VERSION_URL: &str = "https://developer.fiserv.com/product/CommerceHub/api/post/payments-vas/v1/tokens?branch=active&version=1.26.0602";
const FISERV_TRANSACTION_DETAILS_DOC_URL: &str =
    "https://developer.fiserv.com/product/CommerceHub/docs/Reference/Master-Data/Transaction-Details.mdx";
const FISERV_TRANSACTION_INTERACTION_DOC_URL: &str =
    "https://developer.fiserv.com/product/CommerceHub/docs/Reference/Master-Data/Transaction-Interaction.mdx?branch=active#electronic-commerce-indicator-eci";
const FISERV_DECRYPTED_WALLET_DOC_URL: &str = "https://developer.fiserv.com/product/CommerceHub/docs/Payment-Methods/Digital-Wallets/Decrypted-Wallet/Decrypted-Wallet.mdx";
const FISERV_STORED_CREDENTIALS_DOC_URL: &str = "https://developer.fiserv.com/product/CommerceHub/docs/Payment-Methods/Stored-Credentials/Stored-Credentials.mdx?branch=active";
pub(crate) const FISERV_THREE_DS_DOC_URL: &str = "https://developer.fiserv.com/product/CommerceHub/docs/Online-Commerce/3D-Secure/3DS-API-Direct.mdx?branch=active";

#[derive(Debug, Clone, Copy, Serialize)]
pub enum FiservcommercehubEncryptionType {
    #[serde(rename = "RSA")]
    Rsa,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum FiservcommercehubEncryptionTarget {
    #[serde(rename = "MANUAL")]
    Manual,
}
#[derive(Debug)]
pub struct EncryptedCardData {
    pub key_id: String,
    pub encryption_block: Secret<String>,
    pub encryption_block_fields: String,
}

fn encrypt_fiserv_block(
    public_key_der: &[u8],
    plaintext: &[u8],
    field_name: &'static str,
    doc_url: &'static str,
    additional_context: &'static str,
) -> Result<Vec<u8>, error_stack::Report<errors::IntegrationError>> {
    RsaOaepSha256::encrypt(public_key_der, plaintext).map_err(|error| {
        let integration_error = match error.current_context() {
            common_utils::errors::CryptoError::PlaintextTooLong {
                max_length,
                actual_length,
            } => errors::IntegrationError::MaxFieldLengthViolated {
                connector: "fiservcommercehub".to_string(),
                field_name: field_name.to_string(),
                max_length: *max_length,
                received_length: *actual_length,
                context: errors::IntegrationErrorContext {
                    doc_url: Some(doc_url.to_string()),
                    suggested_action: Some(
                        "Reduce the UTF-8 byte length of the payment data included in the Fiserv encryption block"
                            .to_string(),
                    ),
                    additional_context: Some(
                        "The combined payment data exceeds the RSA-OAEP-SHA256 capacity of the active Fiserv public key"
                            .to_string(),
                    ),
                },
            },
            _ => errors::IntegrationError::RequestEncodingFailed {
                context: errors::IntegrationErrorContext {
                    doc_url: Some(doc_url.to_string()),
                    suggested_action: Some(
                        "Ensure the RSA public key is correctly configured and valid".to_string(),
                    ),
                    additional_context: Some(additional_context.to_string()),
                },
            },
        };

        error.change_context(integration_error)
    })
}

fn encrypt_card_data<T: PaymentMethodDataTypes>(
    card: &domain_types::payment_method_data::Card<T>,
    key_id: String,
    public_key_der: &[u8],
) -> Result<EncryptedCardData, error_stack::Report<errors::IntegrationError>> {
    let card_data = card.card_number.peek().to_string();
    let name_on_card = card
        .card_holder_name
        .as_ref()
        .map(|n| n.peek().clone())
        .ok_or(errors::IntegrationError::MissingRequiredField {
            field_name: "card_holder_name",
            context: errors::IntegrationErrorContext {
                additional_context: Some(
                    "card_holder_name is required for card encryption".to_string(),
                ),
                ..Default::default()
            },
        })?;
    let expiration_month = card.card_exp_month.peek().to_string();
    let expiration_year = card.get_expiry_year_4_digit().peek().to_string();
    let security_code = card.card_cvc.peek().to_string();

    let plain_block =
        format!("{card_data}{name_on_card}{expiration_month}{expiration_year}{security_code}");

    let card_data_len = card_data.len();
    let name_on_card_len = name_on_card.len();
    let expiration_month_len = expiration_month.len();
    let expiration_year_len = expiration_year.len();
    let security_code_len = security_code.len();
    let encryption_block_fields = format!(
        "card.cardData:{card_data_len},card.nameOnCard:{name_on_card_len},card.expirationMonth:{expiration_month_len},card.expirationYear:{expiration_year_len},card.securityCode:{security_code_len}"
    );

    let encrypted_bytes = encrypt_fiserv_block(
        public_key_der,
        plain_block.as_bytes(),
        "payment_method_data.card",
        FISERV_PAYMENT_METHOD_ENCRYPTION_URL,
        "RSA OAEP-SHA256 encryption for card data failed",
    )
    .attach_printable("RSA OAEP-SHA256 encryption of card data failed")?;

    let encryption_block = Secret::new(general_purpose::STANDARD.encode(&encrypted_bytes));

    Ok(EncryptedCardData {
        key_id,
        encryption_block,
        encryption_block_fields,
    })
}

// =============================================================================
// DECRYPTED WALLET ENCRYPTION
// =============================================================================

/// Identifies which wallet provider produced the decrypted payment data sent to Fiserv.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FiservcommercehubWalletType {
    ApplePay,
    GooglePay,
}

/// Whether the account number in the encrypted block is a Device PAN (DPAN, wallet token)
/// or a Funding PAN (FPAN, physical card number). Production wallet tokens are DPAN;
/// sandbox/test cards that carry no cryptogram are FPAN.
#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FiservcommercehubWalletAccountType {
    Dpan,
    Fpan,
}

/// Encryption envelope for a DecryptedWallet source.
/// `encryptionTarget: MANUAL` is always required for pre-decrypted wallet submissions.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubDecryptedWalletEncryptionData {
    pub key_id: String,
    pub encryption_type: FiservcommercehubEncryptionType,
    pub encryption_block: Secret<String>,
    pub encryption_block_fields: String,
    pub encryption_target: FiservcommercehubEncryptionTarget,
}

/// Source payload shape for `sourceType: DecryptedWallet` — used when the merchant
/// decrypts the Apple Pay / Google Pay token themselves and re-encrypts it with MUPK.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubDecryptedWalletSource {
    pub wallet_type: FiservcommercehubWalletType,
    pub encryption_data: FiservcommercehubDecryptedWalletEncryptionData,
    pub account_type: FiservcommercehubWalletAccountType,
}

/// Intermediate result from encrypting the decrypted-wallet payment block.
struct DecryptedWalletEncryptedBlock {
    key_id: String,
    encrypted_block: Secret<String>,
    /// Comma-separated field descriptors, e.g. `cavv:44,card.cardData:16,...`.
    encryption_block_fields: String,
}

/// Card-derived fields for a DecryptedWallet encryption block.
///
/// Grouping these three adjacent args prevents silent positional swaps at call sites.
struct DecryptedWalletCardFields {
    dpan: CardNumber,
    exp_month: Secret<String>,
    exp_year: Secret<String>,
}

/// Build and RSA-OAEP-SHA256 encrypt the plaintext block for a DecryptedWallet charge.
///
/// Fiserv's block format is a bare concatenation of field values in the order listed by
/// `encryptionBlockFields`. Each descriptor records the exact character count of that
/// field so Fiserv can parse the block back apart.
///
/// Field order: cavv (optional, omitted when absent) → xid (optional, omitted when absent)
///              → card.cardData → card.expirationMonth → card.expirationYear
///
/// `cavv` is omitted when `None` — Google Pay test cards do not carry a cryptogram;
/// production tokens always provide one.
fn encrypt_decrypted_wallet_data(
    cavv: Option<&Secret<String>>,
    xid: Option<&str>,
    card_fields: DecryptedWalletCardFields,
    key_id: String,
    public_key_der: &[u8],
) -> Result<DecryptedWalletEncryptedBlock, error_stack::Report<errors::IntegrationError>> {
    let DecryptedWalletCardFields {
        dpan,
        exp_month,
        exp_year,
    } = card_fields;

    let mut plain_block = String::new();
    let mut field_descriptors: Vec<String> = Vec::new();

    if let Some(cavv_secret) = cavv {
        let cavv_str = cavv_secret.peek();
        plain_block.push_str(cavv_str);
        field_descriptors.push(format!("cavv:{}", cavv_str.len()));
    }

    if let Some(xid_str) = xid.filter(|s| !s.is_empty()) {
        plain_block.push_str(xid_str);
        field_descriptors.push(format!("xid:{}", xid_str.len()));
    }

    plain_block.push_str(dpan.peek());
    field_descriptors.push(format!("card.cardData:{}", dpan.peek().len()));

    plain_block.push_str(exp_month.peek());
    field_descriptors.push(format!("card.expirationMonth:{}", exp_month.peek().len()));

    plain_block.push_str(exp_year.peek());
    field_descriptors.push(format!("card.expirationYear:{}", exp_year.peek().len()));

    let encryption_block_fields = field_descriptors.join(",");

    let encrypted_bytes = encrypt_fiserv_block(
        public_key_der,
        plain_block.as_bytes(),
        "payment_method_data.decrypted_wallet",
        FISERV_DECRYPTED_WALLET_DOC_URL,
        "RSA OAEP-SHA256 encryption of decrypted wallet payment data failed",
    )
    .attach_printable("RSA OAEP-SHA256 encryption of decrypted wallet data failed")?;

    let encrypted_block = Secret::new(general_purpose::STANDARD.encode(&encrypted_bytes));

    Ok(DecryptedWalletEncryptedBlock {
        key_id,
        encrypted_block,
        encryption_block_fields,
    })
}

fn encrypt_card_data_no_cvc(
    card: &CardWithNoCvc,
    key_id: String,
    public_key_der: &[u8],
) -> Result<EncryptedCardData, error_stack::Report<errors::IntegrationError>> {
    let card_data = card.card_number.get_card_no();
    let name_on_card = card
        .get_cardholder_name()
        .change_context(errors::IntegrationError::MissingRequiredField {
            field_name: "card_holder_name",
            context: errors::IntegrationErrorContext {
                doc_url: Some(FISERV_PAYMENT_METHOD_ENCRYPTION_URL.to_string()),
                suggested_action: Some(
                    "Provide card_holder_name as it is required for card encryption".to_string(),
                ),
                additional_context: Some(
                    "card_holder_name is required for card encryption".to_string(),
                ),
            },
        })?
        .peek()
        .clone();
    let expiration_month = card
        .get_card_expiry_month_2_digit()
        .change_context(errors::IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        })?
        .peek()
        .to_string();
    let expiration_year = card.get_expiry_year_4_digit().peek().to_string();

    let plain_block = format!("{card_data}{name_on_card}{expiration_month}{expiration_year}");

    let card_data_len = card_data.len();
    let name_on_card_len = name_on_card.len();
    let expiration_month_len = expiration_month.len();
    let expiration_year_len = expiration_year.len();
    let encryption_block_fields = format!(
        "card.cardData:{card_data_len},card.nameOnCard:{name_on_card_len},card.expirationMonth:{expiration_month_len},card.expirationYear:{expiration_year_len}"
    );

    let encrypted_bytes = encrypt_fiserv_block(
        public_key_der,
        plain_block.as_bytes(),
        "payment_method_data.card",
        FISERV_PAYMENT_METHOD_ENCRYPTION_URL,
        "RSA OAEP-SHA256 encryption for card data failed",
    )
    .attach_printable("RSA OAEP-SHA256 encryption of card data failed")?;

    let encryption_block = Secret::new(general_purpose::STANDARD.encode(&encrypted_bytes));

    Ok(EncryptedCardData {
        key_id,
        encryption_block,
        encryption_block_fields,
    })
}

fn encrypt_network_mit_card<T: PaymentMethodDataTypes>(
    card_number: &RawCardNumber<T>,
    card_exp_month: &Secret<String>,
    card_exp_year: &Secret<String>,
    card_holder_name: Option<&Secret<String>>,
    key_id: String,
    public_key_der: &[u8],
) -> Result<EncryptedCardData, error_stack::Report<errors::IntegrationError>> {
    // A network-MIT card has no CVC. Reuse the existing PaymentCard encryption
    // format, while rejecting vault aliases that cannot be substituted inside
    // the encrypted block.
    let card_without_cvc = CardWithNoCvc {
        card_number: card_number.try_card_number("fiservcommercehub")?,
        card_exp_month: card_exp_month.clone(),
        card_exp_year: card_exp_year.clone(),
        card_holder_name: card_holder_name.cloned(),
        ..Default::default()
    };
    encrypt_card_data_no_cvc(&card_without_cvc, key_id, public_key_der)
}

/// Builds a `DecryptedWallet` source payload from hyperswitch's in-memory wallet data.
///
/// Only the pre-decrypted variants (`ApplePayPaymentData::Decrypted` and
/// `GpayTokenizationData::Decrypted`) are supported; encrypted blobs must be handled by
/// the normal `ApplePay` / `GooglePay` source types instead.
fn build_decrypted_wallet_source(
    wallet_data: &WalletData,
    key_id: String,
    public_key_der: &[u8],
) -> Result<FiservcommercehubSourceData, error_stack::Report<errors::IntegrationError>> {
    match wallet_data {
        WalletData::ApplePay(apple_pay_data) => {
            let decrypted_apple_pay = match &apple_pay_data.payment_data {
                ApplePayPaymentData::Decrypted(decrypted_apple_pay) => decrypted_apple_pay,
                ApplePayPaymentData::Encrypted(_) => {
                    return Err(error_stack::report!(
                        errors::IntegrationError::NotImplemented(
                            "Fiserv CommerceHub requires the pre-decrypted Apple Pay flow \
                             (ApplePayPaymentData::Decrypted); the encrypted token variant is \
                             not supported — decrypt the Apple Pay payload with your own \
                             merchant certificate before submitting"
                                .to_string(),
                            errors::IntegrationErrorContext {
                                doc_url: Some(FISERV_DECRYPTED_WALLET_DOC_URL.to_string()),
                                suggested_action: Some(
                                    "Enable hyperswitch Apple Pay decryption in the connector \
                                     settings so the token is decrypted before reaching Fiserv"
                                        .to_string(),
                                ),
                                additional_context: Some(
                                    "Received ApplePayPaymentData::Encrypted; expected Decrypted"
                                        .to_string(),
                                ),
                            },
                        )
                    ))
                }
            };

            let payment_cryptogram = decrypted_apple_pay
                .payment_data
                .online_payment_cryptogram
                .clone();
            // Fiserv expects a 2-digit month; zero-pad against bare-digit wallet payloads.
            let expiration_month = Secret::new(format!(
                "{:0>2}",
                decrypted_apple_pay.application_expiration_month.peek()
            ));
            let expiration_year = Secret::new(
                decrypted_apple_pay
                    .get_four_digit_expiry_year()
                    .peek()
                    .to_string(),
            );

            // XID is intentionally omitted for Apple Pay.
            // Fiserv's `xid` field is a 3DS authentication cryptogram, not Apple Pay's
            // `transaction_identifier` (which is a hex session-scoped unique ID, not a
            // cryptographic authentication value). Sending `transaction_identifier` as XID
            // causes Fiserv to return error code 100: "source.xid | Invalid or Missing Field Data".
            let encrypted_block = encrypt_decrypted_wallet_data(
                Some(&payment_cryptogram),
                None,
                DecryptedWalletCardFields {
                    dpan: decrypted_apple_pay
                        .application_primary_account_number
                        .clone(),
                    exp_month: expiration_month,
                    exp_year: expiration_year,
                },
                key_id,
                public_key_der,
            )?;

            let source = FiservcommercehubSourceData::DecryptedWallet(
                FiservcommercehubDecryptedWalletSource {
                    wallet_type: FiservcommercehubWalletType::ApplePay,
                    encryption_data: FiservcommercehubDecryptedWalletEncryptionData {
                        key_id: encrypted_block.key_id,
                        encryption_type: FiservcommercehubEncryptionType::Rsa,
                        encryption_block: encrypted_block.encrypted_block,
                        encryption_block_fields: encrypted_block.encryption_block_fields,
                        encryption_target: FiservcommercehubEncryptionTarget::Manual,
                    },
                    // Apple Pay always yields a DPAN (device-specific token), not the raw FPAN.
                    account_type: FiservcommercehubWalletAccountType::Dpan,
                },
            );

            Ok(source)
        }

        WalletData::GooglePay(gpay_data) => {
            let decrypted_gpay = match &gpay_data.tokenization_data {
                GpayTokenizationData::Decrypted(decrypted_gpay) => decrypted_gpay,
                GpayTokenizationData::Encrypted(_) => {
                    return Err(error_stack::report!(
                        errors::IntegrationError::NotImplemented(
                            "Fiserv CommerceHub requires the pre-decrypted Google Pay flow \
                             (GpayTokenizationData::Decrypted); the encrypted ECv2 token \
                             variant is not supported — decrypt the Google Pay payload with \
                             your own merchant certificate before submitting"
                                .to_string(),
                            errors::IntegrationErrorContext {
                                doc_url: Some(FISERV_DECRYPTED_WALLET_DOC_URL.to_string()),
                                suggested_action: Some(
                                    "Enable hyperswitch Google Pay decryption in the connector \
                                     settings so the token is decrypted before reaching Fiserv"
                                        .to_string(),
                                ),
                                additional_context: Some(
                                    "Received GpayTokenizationData::Encrypted; expected Decrypted"
                                        .to_string(),
                                ),
                            },
                        )
                    ))
                }
            };

            let expiration_month =
                Secret::new(format!("{:0>2}", decrypted_gpay.card_exp_month.peek()));
            let expiration_year = Secret::new(
                decrypted_gpay
                    .get_four_digit_expiry_year()
                    .change_context(errors::IntegrationError::InvalidDataFormat {
                        field_name: "google_pay.card_exp_year",
                        context: errors::IntegrationErrorContext {
                            doc_url: Some(FISERV_DECRYPTED_WALLET_DOC_URL.to_string()),
                            suggested_action: Some(
                                "Expiry year from Google Pay decrypted data must be 2 or 4 digits"
                                    .to_string(),
                            ),
                            additional_context: Some(
                                "Failed to expand Google Pay card_exp_year to 4-digit format"
                                    .to_string(),
                            ),
                        },
                    })?
                    .peek()
                    .to_string(),
            );

            // Google Pay decrypted data has no separate transaction identifier (XID).
            let encrypted_block = encrypt_decrypted_wallet_data(
                decrypted_gpay.cryptogram.as_ref(),
                None,
                DecryptedWalletCardFields {
                    dpan: decrypted_gpay.application_primary_account_number.clone(),
                    exp_month: expiration_month,
                    exp_year: expiration_year,
                },
                key_id,
                public_key_der,
            )?;
            let account_type = if decrypted_gpay.cryptogram.is_some() {
                FiservcommercehubWalletAccountType::Dpan
            } else {
                FiservcommercehubWalletAccountType::Fpan
            };

            let source = FiservcommercehubSourceData::DecryptedWallet(
                FiservcommercehubDecryptedWalletSource {
                    wallet_type: FiservcommercehubWalletType::GooglePay,
                    encryption_data: FiservcommercehubDecryptedWalletEncryptionData {
                        key_id: encrypted_block.key_id,
                        encryption_type: FiservcommercehubEncryptionType::Rsa,
                        encryption_block: encrypted_block.encrypted_block,
                        encryption_block_fields: encrypted_block.encryption_block_fields,
                        encryption_target: FiservcommercehubEncryptionTarget::Manual,
                    },
                    account_type,
                },
            );

            Ok(source)
        }

        _ => Err(error_stack::report!(
            errors::IntegrationError::NotImplemented(
                "Fiserv CommerceHub supports only ApplePay (pre-decrypted) and \
                 GooglePay (pre-decrypted) wallet types"
                    .to_string(),
                errors::IntegrationErrorContext {
                    doc_url: Some(FISERV_DECRYPTED_WALLET_DOC_URL.to_string()),
                    suggested_action: Some(
                        "Use a supported wallet type (ApplePay or GooglePay) with \
                         pre-decrypted payment data"
                            .to_string(),
                    ),
                    additional_context: None,
                },
            )
        )),
    }
}

fn get_decrypted_wallet_eci_indicator(wallet_data: &WalletData) -> Option<String> {
    match wallet_data {
        WalletData::ApplePay(apple_pay_data) => match &apple_pay_data.payment_data {
            ApplePayPaymentData::Decrypted(decrypted_apple_pay) => {
                decrypted_apple_pay.payment_data.eci_indicator.clone()
            }
            ApplePayPaymentData::Encrypted(_) => None,
        },
        WalletData::GooglePay(gpay_data) => match &gpay_data.tokenization_data {
            GpayTokenizationData::Decrypted(decrypted_gpay) => decrypted_gpay.eci_indicator.clone(),
            GpayTokenizationData::Encrypted(_) => None,
        },
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct FiservcommercehubAuthType {
    pub api_key: Secret<String>,
    pub api_secret: Secret<String>,
    pub merchant_id: Secret<String>,
    pub terminal_id: Secret<String>,
}

impl FiservcommercehubAuthType {
    pub fn generate_hmac_signature(
        &self,
        api_key: &str,
        client_request_id: &str,
        timestamp: &str,
        request_body: &str,
    ) -> Result<String, error_stack::Report<errors::IntegrationError>> {
        let raw_signature = format!("{api_key}{client_request_id}{timestamp}{request_body}");
        let signature = crypto::HmacSha256
            .sign_message(self.api_secret.peek().as_bytes(), raw_signature.as_bytes())
            .change_context(errors::IntegrationError::RequestEncodingFailed {
                context: errors::IntegrationErrorContext {
                    additional_context: Some("HMAC-SHA256 signature generation failed".to_string()),
                    doc_url: Some(FISERV_PAYMENT_AUTHENTICATION_URL.to_string()),
                    suggested_action: Some(
                        "Verify the API secret is correct and properly configured".to_string(),
                    ),
                },
            })?;
        Ok(general_purpose::STANDARD.encode(signature))
    }

    pub fn generate_client_request_id() -> String {
        common_utils::fp_utils::generate_uuid_v4()
    }

    pub fn generate_timestamp() -> String {
        common_utils::date_time::now_unix_millis().to_string()
    }

    pub fn build_hmac_headers(
        &self,
        content_type: &str,
        request_body_str: &str,
    ) -> Result<Vec<(String, Maskable<String>)>, error_stack::Report<errors::IntegrationError>>
    {
        let api_key = self.api_key.peek().to_string();
        let client_request_id = Self::generate_client_request_id();
        let timestamp = Self::generate_timestamp();

        let authorization = self.generate_hmac_signature(
            &api_key,
            &client_request_id,
            &timestamp,
            request_body_str,
        )?;

        Ok(vec![
            (
                super::headers::CONTENT_TYPE.to_string(),
                Secret::new(content_type.to_string()).into_masked(),
            ),
            (
                super::headers::API_KEY.to_string(),
                Secret::new(api_key).into_masked(),
            ),
            (
                super::headers::TIMESTAMP.to_string(),
                Secret::new(timestamp).into_masked(),
            ),
            (
                super::headers::CLIENT_REQUEST_ID.to_string(),
                Secret::new(client_request_id).into_masked(),
            ),
            (
                super::headers::AUTHORIZATION.to_string(),
                Secret::new(authorization).into_masked(),
            ),
            (
                super::headers::AUTH_TOKEN_TYPE.to_string(),
                Secret::new(super::headers::AUTH_TOKEN_TYPE_HMAC.to_string()).into_masked(),
            ),
            (
                super::headers::ACCEPT_LANGUAGE.to_string(),
                Secret::new(super::headers::ACCEPT_LANGUAGE_EN.to_string()).into_masked(),
            ),
        ])
    }
}

impl TryFrom<&ConnectorSpecificConfig> for FiservcommercehubAuthType {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Fiservcommercehub {
                api_key,
                secret: api_secret,
                merchant_id,
                terminal_id,
                ..
            } => Ok(Self {
                api_key: api_key.to_owned(),
                api_secret: api_secret.to_owned(),
                merchant_id: merchant_id.to_owned(),
                terminal_id: terminal_id.to_owned(),
            }),
            _ => Err(error_stack::report!(
                errors::IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubErrorResponse {
    pub gateway_response: Option<FiservcommercehubErrorGatewayResponse>,
    pub error: Vec<FiservcommercehubErrorDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubErrorGatewayResponse {
    pub transaction_state: Option<String>,
    pub transaction_processing_details: Option<FiservcommercehubErrorTxnDetails>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubErrorTxnDetails {
    pub api_trace_id: Option<String>,
    pub transaction_id: Option<String>,
    pub order_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FiservcommercehubErrorDetail {
    #[serde(rename = "type")]
    pub error_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    pub message: String,
}

// =============================================================================
// AUTHORIZE FLOW
// =============================================================================

/// Managed 3DS charges reference the authenticated transaction and contain no card source.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum FiservcommercehubAuthorizeRequest {
    Source(Box<FiservcommercehubSourceAuthorizeRequest>),
    Authenticated(Box<FiservcommercehubAuthenticatedChargeRequest>),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubSourceAuthorizeRequest {
    pub amount: FiservcommercehubAuthorizeAmount,
    pub source: FiservcommercehubSourceData,
    pub merchant_details: FiservcommercehubMerchantDetails,
    pub transaction_details: FiservcommercehubTransactionDetailsReq,
    pub transaction_interaction: FiservcommercehubTransactionInteractionReq,
    #[serde(rename = "additionalData3DS", skip_serializing_if = "Option::is_none")]
    pub additional_data_3ds: Option<FiservcommercehubAdditionalData3DS>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stored_credentials: Option<FiservcommercehubStoredCredentials>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_details: Option<FiservcommercehubNetworkDetails>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubNetworkDetails {
    pub transaction_link_identifier: String,
}

#[derive(Debug, Serialize)]
pub struct FiservcommercehubAuthorizeAmount {
    pub currency: common_enums::Currency,
    pub total: FloatMajorUnit,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubPaymentCardSource {
    pub encryption_data: FiservcommercehubEncryptionData,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubPaymentTokenSource {
    pub token_data: Secret<String>,
    pub token_source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decline_duplicates: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<FiservcommercehubTokenCardInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cryptogram: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "sourceType")]
pub enum FiservcommercehubSourceData {
    PaymentCard(FiservcommercehubPaymentCardSource),
    PaymentToken(FiservcommercehubPaymentTokenSource),
    DecryptedWallet(FiservcommercehubDecryptedWalletSource),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubEncryptionData {
    pub key_id: String,
    pub encryption_type: FiservcommercehubEncryptionType,
    pub encryption_block: Secret<String>,
    pub encryption_block_fields: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubTokenCardInfo {
    pub expiration_month: Secret<String>,
    pub expiration_year: Secret<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubTransactionDetailsReq {
    pub capture_flag: bool,
    pub merchant_transaction_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_order_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_invoice_number: Option<String>,
    /// Instructs Commerce Hub to create a TRANSARMOR token inline with this charge.
    /// Required for CIT (Customer Initiated Transaction) mandate payments so that a
    /// reusable token is returned in `paymentTokens` for subsequent MIT charges.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_token: Option<bool>,
}

#[derive(Default, Debug, Deserialize)]
pub struct FiservcommercehubConnectorMetadata {
    pub merchant_invoice_id: Option<String>,
}

fn validate_connector_metadata(
    metadata: &FiservcommercehubConnectorMetadata,
) -> Result<(), error_stack::Report<errors::IntegrationError>> {
    metadata
        .merchant_invoice_id
        .as_ref()
        .filter(|invoice_id| invoice_id.len() > MERCHANT_INVOICE_NUMBER_MAX_LEN)
        .map(|invoice_id| {
            error_stack::report!(errors::IntegrationError::InvalidDataFormat {
                field_name: "metadata.merchant_invoice_id",
                context: errors::IntegrationErrorContext {
                    suggested_action: Some(format!(
                        "merchant_invoice_id must be at most {MERCHANT_INVOICE_NUMBER_MAX_LEN} characters"
                    )),
                    additional_context: Some(format!(
                        "Fiserv CommerceHub accepts a maximum of {MERCHANT_INVOICE_NUMBER_MAX_LEN} characters for merchantInvoiceNumber, but {} characters were provided",
                        invoice_id.len()
                    )),
                    doc_url: Some(FISERV_TRANSACTION_DETAILS_DOC_URL.to_string()),
                },
            })
        })
        .map_or(Ok(()), Err)
}

fn parse_connector_metadata(
    metadata: Option<&SecretSerdeValue>,
) -> Result<FiservcommercehubConnectorMetadata, error_stack::Report<errors::IntegrationError>> {
    let parsed = match metadata {
        Some(meta) => {
            serde_json::from_value::<FiservcommercehubConnectorMetadata>(meta.clone().expose())
                .change_context(errors::IntegrationError::InvalidDataFormat {
                field_name: "metadata",
                context: errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Ensure metadata matches the expected schema for Fiserv CommerceHub"
                            .to_string(),
                    ),
                    doc_url: Some(FISERV_TRANSACTION_DETAILS_DOC_URL.to_string()),
                    additional_context: Some(
                        "Failed to deserialize metadata into FiservcommercehubConnectorMetadata"
                            .to_string(),
                    ),
                },
            })?
        }
        None => FiservcommercehubConnectorMetadata::default(),
    };

    validate_connector_metadata(&parsed)?;
    Ok(parsed)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FiservcommercehubOrigin {
    Ecom,
    Moto,
    Pos,
}

impl From<Option<&common_enums::PaymentChannel>> for FiservcommercehubOrigin {
    fn from(channel: Option<&common_enums::PaymentChannel>) -> Self {
        match channel {
            Some(common_enums::PaymentChannel::MailOrder)
            | Some(common_enums::PaymentChannel::TelephoneOrder) => Self::Moto,
            Some(common_enums::PaymentChannel::Ecommerce) | None => Self::Ecom,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubTransactionInteractionReq {
    pub origin: FiservcommercehubOrigin,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eci_indicator: Option<FiservcommercehubEciIndicator>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FiservcommercehubEciIndicator {
    SecureEcom,
    NonAuthEcom,
    ChannelEncrypted,
    NonSecureEcom,
    AmexPaymentToken,
}

impl TryFrom<&str> for FiservcommercehubEciIndicator {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(eci: &str) -> Result<Self, Self::Error> {
        let eci = eci.trim();
        let normalized_eci = eci.to_ascii_uppercase();
        let indicator = match normalized_eci.as_str() {
            // CommerceHub's 3DS table maps Visa/Amex/Discover 5/6/7 and Mastercard 2/1/0.
            "05" | "5" | "02" | "2" | "SECURE_ECOM" => Some(Self::SecureEcom),
            "06" | "6" | "01" | "1" | "NON_AUTH_ECOM" => Some(Self::NonAuthEcom),
            "07" | "7" | "00" | "0" | "CHANNEL_ENCRYPTED" => Some(Self::ChannelEncrypted),
            "NON_SECURE_ECOM" => Some(Self::NonSecureEcom),
            "AMEX_PAYMENT_TOKEN" => Some(Self::AmexPaymentToken),
            _ => None,
        };

        indicator.ok_or_else(|| {
            error_stack::report!(errors::IntegrationError::InvalidDataFormat {
                field_name: "authentication_data.eci",
                context: errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Use network ECI 5/6/7 (Visa, Amex, Discover) or 2/1/0 (Mastercard), or a documented CommerceHub eciIndicator enum".to_string(),
                    ),
                    doc_url: Some(FISERV_TRANSACTION_INTERACTION_DOC_URL.to_string()),
                    additional_context: Some(format!(
                        "CommerceHub does not accept the provided ECI value: {eci}"
                    )),
                },
            })
        })
    }
}

// =============================================================================
// STORED CREDENTIALS STRUCTURES
// =============================================================================

#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FiservcommercehubStoredCredentialInitiator {
    Merchant,
    CardHolder,
}

#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FiservcommercehubStoredCredentialSequence {
    First,
    Subsequent,
}

/// Reference: https://developer.fiserv.com/product/CommerceHub/docs/Payment-Methods/Stored-Credentials/Stored-Credentials.mdx?branch=active
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubStoredCredentials {
    pub initiator: FiservcommercehubStoredCredentialInitiator,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheme_referenced_transaction_id: Option<String>,
    pub sequence: FiservcommercehubStoredCredentialSequence,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheme_original_amount: Option<FloatMajorUnit>,
}

impl FiservcommercehubStoredCredentials {
    /// * `scheme_original_amount` - Optional original amount (required for Discover)
    pub fn new_cit() -> Self {
        Self {
            initiator: FiservcommercehubStoredCredentialInitiator::CardHolder,
            // scheduled: false,
            scheme_referenced_transaction_id: None,
            sequence: FiservcommercehubStoredCredentialSequence::First,
            scheme_original_amount: None,
        }
    }
    /// * `scheme_original_amount` - Optional original amount (required for Discover)
    pub fn new_mit(
        scheme_referenced_transaction_id: Option<String>,
        scheme_original_amount: Option<FloatMajorUnit>,
    ) -> Self {
        Self {
            initiator: FiservcommercehubStoredCredentialInitiator::Merchant,
            // scheduled,
            scheme_referenced_transaction_id,
            sequence: FiservcommercehubStoredCredentialSequence::Subsequent,
            scheme_original_amount,
        }
    }
}

fn card_stored_credentials(
    mandate_id: Option<&MandateIds>,
    is_customer_initiated_mandate: bool,
    requires_network_reference: bool,
) -> Result<
    (
        Option<FiservcommercehubStoredCredentials>,
        Option<FiservcommercehubNetworkDetails>,
    ),
    error_stack::Report<errors::IntegrationError>,
> {
    let network_reference = mandate_id.and_then(|ids| match ids.mandate_reference_id.as_ref() {
        Some(MandateReferenceId::NetworkMandateId(reference)) => Some((
            reference.network_transaction_id.as_str(),
            reference.transaction_link_id.as_deref(),
        )),
        // Raised by the NetworkToken Authorize arm when the payment is a MIT replaying a
        // network token under `recurring_details: network_transaction_id_and_network_token`.
        Some(MandateReferenceId::NetworkTokenWithNTI(reference)) => Some((
            reference.network_transaction_id.as_str(),
            reference.transaction_link_id.as_deref(),
        )),
        _ => None,
    });

    stored_credentials_for_network_reference(
        network_reference,
        is_customer_initiated_mandate,
        requires_network_reference,
    )
}

/// Shared by both the Authorize flow (via [`card_stored_credentials`], which extracts the
/// reference out of a `MandateIds`) and the RepeatPayment flow's `NetworkMandateId` /
/// `NetworkTokenWithNTI` arms (whose reference types carry the same
/// `network_transaction_id` + `transaction_link_id` shape but aren't the same Rust type).
fn stored_credentials_for_network_reference(
    network_reference: Option<(&str, Option<&str>)>,
    is_customer_initiated_mandate: bool,
    requires_network_reference: bool,
) -> Result<
    (
        Option<FiservcommercehubStoredCredentials>,
        Option<FiservcommercehubNetworkDetails>,
    ),
    error_stack::Report<errors::IntegrationError>,
> {
    match network_reference {
        Some((network_transaction_id, transaction_link_id)) => {
            let network_transaction_id = network_transaction_id.trim();
            if network_transaction_id.is_empty() {
                return Err(error_stack::report!(
                    errors::IntegrationError::MissingRequiredField {
                        field_name: "mandate_id.network_transaction_id",
                        context: errors::IntegrationErrorContext {
                            doc_url: Some(FISERV_STORED_CREDENTIALS_DOC_URL.to_string()),
                            ..Default::default()
                        },
                    }
                ));
            }

            let network_details = transaction_link_id.map(|id| FiservcommercehubNetworkDetails {
                transaction_link_identifier: id.to_string(),
            });
            Ok((
                Some(FiservcommercehubStoredCredentials::new_mit(
                    Some(network_transaction_id.to_string()),
                    None,
                )),
                network_details,
            ))
        }
        None if requires_network_reference => Err(error_stack::report!(
            errors::IntegrationError::MissingRequiredField {
                field_name: "mandate_id.network_transaction_id",
                context: errors::IntegrationErrorContext {
                    doc_url: Some(FISERV_STORED_CREDENTIALS_DOC_URL.to_string()),
                    ..Default::default()
                },
            }
        )),
        None if is_customer_initiated_mandate => {
            Ok((Some(FiservcommercehubStoredCredentials::new_cit()), None))
        }
        None => Ok((None, None)),
    }
}

/// Wraps an RSA-encrypted card/token block into the `PaymentCard` source shape Fiserv expects.
/// Shared by the `Card`, `CardWithNoCvc`, and `CardDetailsForNetworkTransactionId` Authorize arms
/// and the `NetworkMandateId` RepeatPayment arm, which otherwise each repeat this struct literal.
fn payment_card_source(encrypted: EncryptedCardData) -> FiservcommercehubSourceData {
    FiservcommercehubSourceData::PaymentCard(FiservcommercehubPaymentCardSource {
        encryption_data: FiservcommercehubEncryptionData {
            key_id: encrypted.key_id,
            encryption_type: FiservcommercehubEncryptionType::Rsa,
            encryption_block: encrypted.encryption_block,
            encryption_block_fields: encrypted.encryption_block_fields,
        },
    })
}

// =============================================================================
// PAYMENT TOKEN STRUCTURES
// =============================================================================

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubPaymentToken {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_data: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_response_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_response_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cryptogram: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_requestor_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_assurance_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_reference_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FiservcommercehubPaymentTokens(pub Vec<FiservcommercehubPaymentToken>);

impl FiservcommercehubPaymentTokens {
    pub fn get_mandate_reference(
        &self,
        original_txn_id: Option<String>,
    ) -> Option<Box<MandateReference>> {
        self.0
            .iter()
            .find(|token| {
                token
                    .token_response_code
                    .as_ref()
                    .map(|code| {
                        (code == "000" || code.eq_ignore_ascii_case("SUCCESS"))
                            && token.token_source == Some(TOKEN_SOURCE_TRANSARMOR.to_string())
                    })
                    .unwrap_or(false)
            })
            .map(|token| {
                Box::new(MandateReference {
                    connector_mandate_id: token.token_data.as_ref().map(|t| t.peek().clone()),
                    payment_method_id: token.token_source.clone(),
                    connector_mandate_request_reference_id: original_txn_id,
                    mandate_metadata: None,
                })
            })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubMpiData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cavv: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xid: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubAdditionalData3DS {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds_transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acs_transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mpi_data: Option<FiservcommercehubMpiData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_data: Option<FiservcommercehubVersionData>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubVersionData {
    pub recommended_version: String,
}

pub fn build_additional_data_3ds(
    authentication_data: Option<&AuthenticationData>,
) -> Option<FiservcommercehubAdditionalData3DS> {
    authentication_data.and_then(|auth_data| {
        let xid = auth_data.threeds_server_transaction_id.clone();
        let mpi_data =
            (auth_data.cavv.is_some() || xid.is_some()).then(|| FiservcommercehubMpiData {
                cavv: auth_data.cavv.clone(),
                xid,
            });
        let additional_data_3ds = FiservcommercehubAdditionalData3DS {
            ds_transaction_id: auth_data.ds_trans_id.clone(),
            authentication_status: auth_data
                .trans_status
                .as_ref()
                .map(fiserv_authentication_status),
            acs_transaction_id: auth_data.acs_transaction_id.clone(),
            mpi_data,
            version_data: auth_data.message_version.as_ref().map(|version| {
                FiservcommercehubVersionData {
                    recommended_version: version.to_string(),
                }
            }),
        };

        (additional_data_3ds.ds_transaction_id.is_some()
            || additional_data_3ds.authentication_status.is_some()
            || additional_data_3ds.acs_transaction_id.is_some()
            || additional_data_3ds.mpi_data.is_some()
            || additional_data_3ds.version_data.is_some())
        .then_some(additional_data_3ds)
    })
}

// These are the standard EMV 3DS transStatus codes mapped to CommerceHub's authenticationStatus field.
fn fiserv_authentication_status(status: &common_enums::TransactionStatus) -> String {
    match status {
        common_enums::TransactionStatus::Success => "Y",
        common_enums::TransactionStatus::Failure => "N",
        common_enums::TransactionStatus::VerificationNotPerformed => "U",
        common_enums::TransactionStatus::NotVerified => "A",
        common_enums::TransactionStatus::Rejected => "R",
        common_enums::TransactionStatus::ChallengeRequired => "C",
        common_enums::TransactionStatus::ChallengeRequiredDecoupledAuthentication => "D",
        common_enums::TransactionStatus::InformationOnly => "I",
        common_enums::TransactionStatus::SecurePaymentConfirmationRequired => "S",
    }
    .to_string()
}

fn encryption_key(
    resource: &PaymentFlowData,
) -> Result<(String, Vec<u8>), error_stack::Report<errors::IntegrationError>> {
    let access_token = resource.get_access_token()?;
    let (key_id, encoded_public_key) = access_token
        .split_once(ACCESS_TOKEN_SEPARATOR)
        .filter(|(key_id, public_key)| !key_id.is_empty() && !public_key.is_empty())
        .ok_or_else(|| {
            error_stack::report!(errors::IntegrationError::InvalidDataFormat {
                field_name: "access_token",
                context: super::fiserv_integration_error_context(
                    "Refresh the CommerceHub public key using ServerAuthenticationToken.",
                    FISERV_PAYMENT_METHOD_ENCRYPTION_URL,
                    "Expected a key ID and Base64 RSA public key separated by |||.",
                ),
            })
        })?;
    let public_key = general_purpose::STANDARD
        .decode(encoded_public_key)
        .change_context(errors::IntegrationError::RequestEncodingFailed {
            context: super::fiserv_integration_error_context(
                "Refresh the CommerceHub public key using ServerAuthenticationToken.",
                FISERV_PAYMENT_METHOD_ENCRYPTION_URL,
                "The cached RSA public key is not valid Base64.",
            ),
        })?;
    Ok((key_id.to_string(), public_key))
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FiservcommercehubRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for FiservcommercehubAuthorizeRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: super::FiservcommercehubRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;

        let currency = router_data.request.currency;
        let total =
            utils::convert_amount(item.connector.amount_converter, &router_data.request.money)?;

        if let Some(request) = build_authenticated_charge_request(
            &router_data.resource_common_data,
            &router_data.connector_config,
            &router_data.request,
            FiservcommercehubAuthorizeAmount { currency, total },
        )? {
            return Ok(Self::Authenticated(Box::new(request)));
        }
        let (key_id, public_key_der) = encryption_key(&router_data.resource_common_data)?;

        let auth_type = &router_data.connector_config;
        let auth = FiservcommercehubAuthType::try_from(auth_type)?;

        let (source, stored_credentials, network_details, wallet_eci) = match &router_data
            .request
            .payment_method_data
        {
            PaymentMethodData::Card(card) => {
                let (stored_credentials, network_details) = card_stored_credentials(
                    router_data.request.mandate_id.as_ref(),
                    router_data.request.is_customer_initiated_mandate_payment(),
                    false,
                )?;
                let encrypted_card = if stored_credentials.as_ref().is_some_and(|credentials| {
                    credentials.initiator == FiservcommercehubStoredCredentialInitiator::Merchant
                }) {
                    // A stored-card MIT must not transmit a saved security code.
                    encrypt_network_mit_card(
                        &card.card_number,
                        &card.card_exp_month,
                        &card.card_exp_year,
                        card.card_holder_name.as_ref(),
                        key_id,
                        &public_key_der,
                    )?
                } else {
                    encrypt_card_data(card, key_id, &public_key_der)?
                };

                (
                    payment_card_source(encrypted_card),
                    stored_credentials,
                    network_details,
                    None,
                )
            }
            PaymentMethodData::CardWithNoCvc(card) => {
                let encrypted_card = encrypt_card_data_no_cvc(card, key_id, &public_key_der)?;
                let (stored_credentials, network_details) = card_stored_credentials(
                    router_data.request.mandate_id.as_ref(),
                    router_data.request.is_customer_initiated_mandate_payment(),
                    false,
                )?;

                (
                    payment_card_source(encrypted_card),
                    stored_credentials,
                    network_details,
                    None,
                )
            }
            PaymentMethodData::CardDetailsForNetworkTransactionId(card) => {
                let (stored_credentials, network_details) =
                    card_stored_credentials(router_data.request.mandate_id.as_ref(), false, true)?;
                let encrypted_card = encrypt_network_mit_card(
                    &card.card_number,
                    &card.card_exp_month,
                    &card.card_exp_year,
                    card.card_holder_name.as_ref(),
                    key_id,
                    &public_key_der,
                )?;

                (
                    payment_card_source(encrypted_card),
                    stored_credentials,
                    network_details,
                    None,
                )
            }
            PaymentMethodData::Wallet(wallet_data) => {
                let source = build_decrypted_wallet_source(wallet_data, key_id, &public_key_der)?;

                let wallet_eci = get_decrypted_wallet_eci_indicator(wallet_data);

                let stored_credentials =
                    if router_data.request.is_customer_initiated_mandate_payment() {
                        Some(FiservcommercehubStoredCredentials::new_cit())
                    } else {
                        None
                    };

                (source, stored_credentials, None, wallet_eci)
            }
            PaymentMethodData::NetworkToken(token_data) => {
                // A merchant-supplied third-party network token is passed through as a
                // PaymentToken source (tokenSource=NETWORK_TOKEN), not RSA-encrypted like a
                // raw PAN -- the token itself is already network-scoped and opaque.
                let (stored_credentials, network_details) = card_stored_credentials(
                    router_data.request.mandate_id.as_ref(),
                    router_data.request.is_customer_initiated_mandate_payment(),
                    false,
                )?;

                // Fiserv requires a cryptogram for customer-initiated token payments; a MIT
                // replaying the token via a network transaction ID uses the scheme reference
                // (stored_credentials) instead, so the cryptogram must be omitted there.
                let is_network_mit = stored_credentials.as_ref().is_some_and(|credentials| {
                    credentials.initiator == FiservcommercehubStoredCredentialInitiator::Merchant
                });
                let source = FiservcommercehubSourceData::PaymentToken(
                    FiservcommercehubPaymentTokenSource {
                        token_data: Secret::new(token_data.get_network_token().peek().clone()),
                        token_source: TOKEN_SOURCE_NETWORK_TOKEN.to_string(),
                        decline_duplicates: None,
                        card: Some(FiservcommercehubTokenCardInfo {
                            expiration_month: Secret::new(format!(
                                "{:02}",
                                crate::utils::parse_expiry_month(
                                    token_data.get_network_token_expiry_month().peek(),
                                    "network_token.token_exp_month",
                                )?
                            )),
                            expiration_year: token_data.get_expiry_year_4_digit(),
                        }),
                        cryptogram: if is_network_mit {
                            None
                        } else {
                            token_data.get_cryptogram()
                        },
                    },
                );

                (
                    source,
                    stored_credentials,
                    network_details,
                    token_data.eci.clone(),
                )
            }
            _ => {
                return Err(error_stack::report!(
                    errors::IntegrationError::NotImplemented(
                        "Fiserv CommerceHub supports only Card, CardWithNoCvc, \
                         CardDetailsForNetworkTransactionId, NetworkToken, \
                         ApplePay (pre-decrypted), and GooglePay (pre-decrypted)"
                            .to_string(),
                        errors::IntegrationErrorContext {
                            doc_url: Some(FISERV_CHARGES_API_VERSION_URL.to_string()),
                            additional_context: Some(
                                "Received an unsupported payment method type for the Authorize flow"
                                    .to_string(),
                            ),
                            ..Default::default()
                        },
                    )
                ))
            }
        };

        let origin = FiservcommercehubOrigin::from(router_data.request.payment_channel.as_ref());

        let eci_indicator = router_data
            .request
            .authentication_data
            .as_ref()
            .and_then(|auth_data| auth_data.eci.as_deref())
            .or(wallet_eci.as_deref())
            .map(FiservcommercehubEciIndicator::try_from)
            .transpose()?;

        let additional_data_3ds =
            build_additional_data_3ds(router_data.request.authentication_data.as_ref());

        let connector_metadata = parse_connector_metadata(router_data.request.metadata.as_ref())?;

        // Request inline TRANSARMOR tokenization for wallet CIT mandate payments so the token
        // returned in `paymentTokens` can be stored and used for subsequent MIT charges.
        // Restricted to wallets until card TRANSARMOR tokenization is explicitly tested.
        let create_token = (matches!(
            &router_data.request.payment_method_data,
            PaymentMethodData::Wallet(_)
        ) && router_data.request.is_customer_initiated_mandate_payment())
        .then_some(true);

        let request = FiservcommercehubSourceAuthorizeRequest {
            amount: FiservcommercehubAuthorizeAmount { currency, total },
            source,
            merchant_details: FiservcommercehubMerchantDetails {
                merchant_id: auth.merchant_id.clone(),
                terminal_id: auth.terminal_id.clone(),
            },
            transaction_details: FiservcommercehubTransactionDetailsReq {
                capture_flag: router_data.request.is_auto_capture(),
                merchant_transaction_id: router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
                merchant_order_id: router_data.request.merchant_order_id.clone(),
                merchant_invoice_number: connector_metadata.merchant_invoice_id,
                create_token,
            },
            stored_credentials,
            network_details,
            transaction_interaction: FiservcommercehubTransactionInteractionReq {
                origin,
                eci_indicator,
            },
            additional_data_3ds,
        };
        Ok(Self::Source(Box::new(request)))
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FiservcommercehubTransactionState {
    Approved,
    Captured,
    Authorized,
    Pending,
    Timeout,
    Declined,
    Rejected,
    Failed,
    Cancelled,
    Voided,
}

impl From<&FiservcommercehubTransactionState> for AttemptStatus {
    fn from(state: &FiservcommercehubTransactionState) -> Self {
        match state {
            FiservcommercehubTransactionState::Approved
            | FiservcommercehubTransactionState::Captured => Self::Charged,
            FiservcommercehubTransactionState::Authorized => Self::Authorized,
            FiservcommercehubTransactionState::Pending
            | FiservcommercehubTransactionState::Timeout => Self::Pending,
            FiservcommercehubTransactionState::Declined
            | FiservcommercehubTransactionState::Rejected
            | FiservcommercehubTransactionState::Failed => Self::Failure,
            FiservcommercehubTransactionState::Cancelled
            | FiservcommercehubTransactionState::Voided => Self::Voided,
        }
    }
}

impl From<&FiservcommercehubTransactionState> for common_enums::AuthorizationStatus {
    fn from(state: &FiservcommercehubTransactionState) -> Self {
        match state {
            FiservcommercehubTransactionState::Approved
            | FiservcommercehubTransactionState::Authorized => Self::Success,
            FiservcommercehubTransactionState::Pending => Self::Processing,
            FiservcommercehubTransactionState::Timeout => Self::Unresolved,
            FiservcommercehubTransactionState::Captured
            | FiservcommercehubTransactionState::Declined
            | FiservcommercehubTransactionState::Rejected
            | FiservcommercehubTransactionState::Failed
            | FiservcommercehubTransactionState::Cancelled
            | FiservcommercehubTransactionState::Voided => Self::Failure,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FiservcommercehubRefundState {
    Approved,
    Captured,
    Authorized,
    Pending,
    Declined,
    Rejected,
    Failed,
    Cancelled,
}

impl From<&FiservcommercehubRefundState> for RefundStatus {
    fn from(state: &FiservcommercehubRefundState) -> Self {
        match state {
            FiservcommercehubRefundState::Approved | FiservcommercehubRefundState::Captured => {
                Self::Success
            }
            FiservcommercehubRefundState::Authorized | FiservcommercehubRefundState::Pending => {
                Self::Pending
            }
            FiservcommercehubRefundState::Declined
            | FiservcommercehubRefundState::Rejected
            | FiservcommercehubRefundState::Failed
            | FiservcommercehubRefundState::Cancelled => Self::Failure,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubAuthorizeResponse {
    pub gateway_response: FiservcommercehubGatewayResponseBody,
    pub payment_receipt: Option<FiservcommercehubPaymentReceipt>,
    pub payment_tokens: Option<FiservcommercehubPaymentTokens>,
    pub network_details: Option<FiservcommercehubResponseNetworkDetails>,
    /// Additional 3DS data returned in the response as a generic JSON Value
    #[serde(rename = "additionalData3DS", skip_serializing_if = "Option::is_none")]
    pub additional_data_3ds: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubResponseNetworkDetails {
    pub transaction_identifier: Option<String>,
    pub transaction_link_identifier: Option<String>,
}

impl FiservcommercehubAuthorizeResponse {
    fn approval_code(&self) -> Option<String> {
        self.payment_receipt
            .as_ref()
            .and_then(FiservcommercehubPaymentReceipt::approval_code)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubPaymentReceipt {
    pub processor_response_details: Option<FiservcommercehubProcessorResponseDetails>,
    pub approved_amount: Option<FiservcommercehubApprovedAmount>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubApprovedAmount {
    pub currency: common_enums::Currency,
    pub total: FloatMajorUnit,
}

impl FiservcommercehubPaymentReceipt {
    fn approval_code(&self) -> Option<String> {
        self.processor_response_details
            .as_ref()
            .and_then(|details| details.approval_code.clone())
    }
}

fn approved_amount_to_money(
    payment_receipt: Option<&FiservcommercehubPaymentReceipt>,
    http_status: u16,
) -> Result<Option<common_utils::types::Money>, error_stack::Report<errors::ConnectorError>> {
    payment_receipt
        .and_then(|receipt| receipt.approved_amount.as_ref())
        .map(|approved_amount| {
            response_amount_to_money(
                &FloatMajorUnitForConnector,
                approved_amount.total,
                approved_amount.currency,
                http_status,
                domain_types::connector_types::ConnectorEnum::Fiservcommercehub,
            )
        })
        .transpose()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubProcessorResponseDetails {
    pub approval_code: Option<String>,
    pub response_code: Option<String>,
    pub response_message: Option<String>,
    pub host_response_code: Option<String>,
    pub host_response_message: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubGatewayResponseBody {
    pub transaction_state: FiservcommercehubTransactionState,
    pub transaction_processing_details: FiservcommercehubTxnDetails,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubTxnDetails {
    pub order_id: Option<String>,
    pub transaction_id: String,
}

fn build_connector_response(
    additional_data_3ds: Option<&serde_json::Value>,
    auth_code: Option<String>,
) -> Option<domain_types::router_data::ConnectorResponseData> {
    if additional_data_3ds.is_some() || auth_code.is_some() {
        let additional_payment_method_data =
            domain_types::router_data::AdditionalPaymentMethodConnectorResponse::Card {
                authentication_data: additional_data_3ds.cloned(),
                payment_checks: None,
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
            };
        Some(
            domain_types::router_data::ConnectorResponseData::with_additional_payment_method_data(
                additional_payment_method_data,
            ),
        )
    } else {
        None
    }
}

/// Bundles the optional response-side extras consulted by [`build_payment_response`] and
/// [`build_transaction_payment_response`] so adding one more doesn't push either function's
/// argument count past clippy's `too_many_arguments` threshold.
struct FiservcommercehubResponseExtras<'a> {
    payment_tokens: Option<&'a FiservcommercehubPaymentTokens>,
    payment_receipt: Option<&'a FiservcommercehubPaymentReceipt>,
    network_details: Option<&'a FiservcommercehubResponseNetworkDetails>,
}

fn build_payment_response(
    status: AttemptStatus,
    status_code: u16,
    resource_id: ResponseId,
    connector_transaction_id: Option<String>,
    connector_response_reference_id: Option<String>,
    extras: FiservcommercehubResponseExtras<'_>,
) -> Result<PaymentsResponseData, ErrorResponse> {
    let FiservcommercehubResponseExtras {
        payment_tokens,
        payment_receipt,
        network_details,
    } = extras;
    match status {
        AttemptStatus::Failure => {
            let processor_response_details =
                payment_receipt.and_then(|receipt| receipt.processor_response_details.as_ref());

            let response_code =
                processor_response_details.and_then(|details| details.response_code.clone());
            let response_message =
                processor_response_details.and_then(|details| details.response_message.clone());
            let host_response_code =
                processor_response_details.and_then(|details| details.host_response_code.clone());
            let host_response_message = processor_response_details
                .and_then(|details| details.host_response_message.clone());

            Err(ErrorResponse {
                code: response_code
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: response_message
                    .clone()
                    .or_else(|| host_response_message.clone())
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: host_response_message
                    .clone()
                    .or_else(|| response_message.clone()),
                status_code,
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id,
                network_decline_code: host_response_code,
                network_advice_code: None,
                network_error_message: host_response_message,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            })
        }
        _ => Ok(PaymentsResponseData::TransactionResponse {
            resource_id,
            redirection_data: None,
            mandate_reference: payment_tokens
                .and_then(|token| token.get_mandate_reference(connector_transaction_id)),
            connector_metadata: None,
            network_txn_id: network_details
                .and_then(|details| details.transaction_identifier.as_deref())
                .map(str::trim)
                .filter(|id| !id.is_empty())
                .map(str::to_owned),
            network_txn_link_id: network_details
                .and_then(|details| details.transaction_link_identifier.as_deref())
                .map(str::trim)
                .filter(|id| !id.is_empty())
                .map(str::to_owned),
            connector_response_reference_id,
            incremental_authorization_allowed: None,
            status_code,
            splits: None,
            payment_account_reference: None,
        }),
    }
}

fn build_transaction_payment_response(
    status: AttemptStatus,
    status_code: u16,
    txn: &FiservcommercehubTxnDetails,
    extras: FiservcommercehubResponseExtras<'_>,
) -> Result<PaymentsResponseData, ErrorResponse> {
    let connector_transaction_id = txn.transaction_id.clone();

    build_payment_response(
        status,
        status_code,
        ResponseId::ConnectorTransactionId(connector_transaction_id.clone()),
        Some(connector_transaction_id),
        txn.order_id.clone(),
        extras,
    )
}

impl<T: PaymentMethodDataTypes>
    TryFrom<ResponseRouterData<FiservcommercehubAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiservcommercehubAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let txn = &item
            .response
            .gateway_response
            .transaction_processing_details;
        let status = AttemptStatus::from(&item.response.gateway_response.transaction_state);

        let connector_response = build_connector_response(
            item.response.additional_data_3ds.as_ref(),
            item.response.approval_code(),
        );
        let request_incremental_authorization =
            item.router_data.request.request_incremental_authorization;
        let mut response = build_transaction_payment_response(
            status,
            item.http_code,
            txn,
            FiservcommercehubResponseExtras {
                payment_tokens: item.response.payment_tokens.as_ref(),
                payment_receipt: item.response.payment_receipt.as_ref(),
                network_details: item.response.network_details.as_ref(),
            },
        );
        if let Ok(PaymentsResponseData::TransactionResponse {
            incremental_authorization_allowed,
            ..
        }) = &mut response
        {
            *incremental_authorization_allowed = request_incremental_authorization;
        }

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                connector_response,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// PSYNC FLOW
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubPSyncMerchantDetails {
    pub merchant_id: Secret<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubReferenceTransactionDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_merchant_transaction_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubPSyncRequest {
    pub merchant_details: FiservcommercehubPSyncMerchantDetails,
    pub reference_transaction_details: FiservcommercehubReferenceTransactionDetails,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FiservcommercehubRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for FiservcommercehubPSyncRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: super::FiservcommercehubRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let auth = FiservcommercehubAuthType::try_from(&router_data.connector_config)?;

        let connector_transaction_id = router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .inspect_err(|_| {
                tracing::warn!(
                    "fiservcommercehub PSync: connector_transaction_id not present,
                     falling back to connector_request_reference_id"
                );
            })
            .ok()
            .filter(|id| !id.is_empty());

        let connector_request_reference_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        let reference_transaction_details = match connector_transaction_id {
            Some(txn_id) => FiservcommercehubReferenceTransactionDetails {
                reference_transaction_id: Some(txn_id),
                reference_merchant_transaction_id: None,
            },
            None => FiservcommercehubReferenceTransactionDetails {
                reference_transaction_id: None,
                reference_merchant_transaction_id: Some(connector_request_reference_id),
            },
        };

        Ok(Self {
            merchant_details: FiservcommercehubPSyncMerchantDetails {
                merchant_id: auth.merchant_id.clone(),
            },
            reference_transaction_details,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubPSyncGatewayResponse {
    pub transaction_state: FiservcommercehubTransactionState,
    pub transaction_processing_details: Option<FiservcommercehubTxnDetails>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubPSyncItem {
    pub gateway_response: FiservcommercehubPSyncGatewayResponse,
    pub payment_receipt: Option<FiservcommercehubPaymentReceipt>,
    pub network_details: Option<FiservcommercehubResponseNetworkDetails>,
}

impl FiservcommercehubPSyncItem {
    fn approval_code(&self) -> Option<String> {
        self.payment_receipt
            .as_ref()
            .and_then(FiservcommercehubPaymentReceipt::approval_code)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FiservcommercehubPSyncResponse(pub Vec<FiservcommercehubPSyncItem>);

impl TryFrom<ResponseRouterData<FiservcommercehubPSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiservcommercehubPSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let Some(psync_item) = item.response.0.into_iter().next() else {
            return Ok(Self {
                response: Err(ErrorResponse {
                    code: "TRANSACTION_NOT_FOUND".to_string(),
                    message: "No matching transaction found".to_string(),
                    reason: Some(
                        "Transaction not found in transaction inquiry results".to_string(),
                    ),
                    status_code: item.http_code,
                    ..Default::default()
                }),
                ..item.router_data
            });
        };
        let status = AttemptStatus::from(&psync_item.gateway_response.transaction_state);
        let connector_response = build_connector_response(None, psync_item.approval_code());
        let connector_transaction_id = psync_item
            .gateway_response
            .transaction_processing_details
            .as_ref()
            .map(|txn| txn.transaction_id.clone());
        let resource_id = connector_transaction_id
            .clone()
            .map(ResponseId::ConnectorTransactionId)
            .unwrap_or_else(|| {
                tracing::warn!(
                    "fiservcommercehub PSync: connector_transaction_id absent in response, 
                     resource_id set to NoResponseId"
                );
                ResponseId::NoResponseId
            });
        let response = build_payment_response(
            status,
            item.http_code,
            resource_id,
            connector_transaction_id,
            None,
            FiservcommercehubResponseExtras {
                payment_tokens: None,
                payment_receipt: psync_item.payment_receipt.as_ref(),
                network_details: psync_item.network_details.as_ref(),
            },
        );

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                connector_response,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// REFUND FLOW
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubRefundTransactionDetails {
    pub merchant_transaction_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubRefundRequest {
    pub amount: FiservcommercehubAuthorizeAmount,
    pub transaction_details: FiservcommercehubRefundTransactionDetails,
    pub merchant_details: FiservcommercehubMerchantDetails,
    pub reference_transaction_details: FiservcommercehubReferenceTransactionDetails,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FiservcommercehubRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for FiservcommercehubRefundRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: super::FiservcommercehubRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let total = utils::convert_amount(
            item.connector.amount_converter,
            &router_data.request.refund_money,
        )?;
        let auth = FiservcommercehubAuthType::try_from(&router_data.connector_config)?;
        Ok(Self {
            amount: FiservcommercehubAuthorizeAmount {
                currency: router_data.request.currency,
                total,
            },
            transaction_details: FiservcommercehubRefundTransactionDetails {
                merchant_transaction_id: router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            },
            merchant_details: FiservcommercehubMerchantDetails {
                merchant_id: auth.merchant_id.clone(),
                terminal_id: auth.terminal_id.clone(),
            },
            reference_transaction_details: FiservcommercehubReferenceTransactionDetails {
                reference_transaction_id: Some(
                    router_data.request.connector_transaction_id.clone(),
                ),
                reference_merchant_transaction_id: None,
            },
        })
    }
}

/// Response body from `POST /payments/v1/refunds`.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubRefundResponse {
    pub gateway_response: FiservcommercehubRefundGatewayResponseBody,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubRefundGatewayResponseBody {
    pub transaction_state: FiservcommercehubRefundState,
    pub transaction_processing_details: FiservcommercehubTxnDetails,
}

impl TryFrom<ResponseRouterData<FiservcommercehubRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiservcommercehubRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let refund_status = RefundStatus::from(&item.response.gateway_response.transaction_state);
        let txn = &item
            .response
            .gateway_response
            .transaction_processing_details;
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: txn.transaction_id.clone(),
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// RSYNC FLOW (Refund Sync)
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubRSyncRequest {
    pub merchant_details: FiservcommercehubPSyncMerchantDetails,
    pub reference_transaction_details: FiservcommercehubReferenceTransactionDetails,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FiservcommercehubRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for FiservcommercehubRSyncRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: super::FiservcommercehubRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let auth = FiservcommercehubAuthType::try_from(&router_data.connector_config)?;
        Ok(Self {
            merchant_details: FiservcommercehubPSyncMerchantDetails {
                merchant_id: auth.merchant_id.clone(),
            },
            reference_transaction_details: FiservcommercehubReferenceTransactionDetails {
                reference_transaction_id: Some(router_data.request.connector_refund_id.clone()),
                reference_merchant_transaction_id: None,
            },
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubRSyncGatewayResponse {
    pub transaction_state: FiservcommercehubRefundState,
    pub transaction_processing_details: Option<FiservcommercehubRSyncTxnDetails>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubRSyncTxnDetails {
    pub transaction_id: String,
    pub order_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubRSyncItem {
    pub gateway_response: FiservcommercehubRSyncGatewayResponse,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FiservcommercehubRSyncResponse(pub Vec<FiservcommercehubRSyncItem>);

impl TryFrom<ResponseRouterData<FiservcommercehubRSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiservcommercehubRSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let rsync_item = item.response.0.into_iter().next().ok_or_else(|| {
            error_stack::report!(
                crate::utils::response_deserialization_fail(
                    item.http_code
                , "fiservcommercehub: response body did not match the expected format; confirm API version and connector documentation.")
            )
        })?;
        let refund_status = RefundStatus::from(&rsync_item.gateway_response.transaction_state);
        let connector_refund_id = rsync_item
            .gateway_response
            .transaction_processing_details
            .map(|d| d.transaction_id)
            .unwrap_or(item.router_data.request.connector_refund_id.clone());
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// VOID FLOW
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubVoidRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<FiservcommercehubAuthorizeAmount>,
    pub transaction_details: FiservcommercehubRefundTransactionDetails,
    pub merchant_details: FiservcommercehubMerchantDetails,
    pub reference_transaction_details: FiservcommercehubReferenceTransactionDetails,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FiservcommercehubRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for FiservcommercehubVoidRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: super::FiservcommercehubRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let auth = FiservcommercehubAuthType::try_from(&router_data.connector_config)?;

        let amount = match (
            router_data.request.money.as_ref(),
            router_data.request.currency,
        ) {
            (Some(money), Some(currency)) => {
                let total = utils::convert_amount(item.connector.amount_converter, money)?;
                Some(FiservcommercehubAuthorizeAmount { currency, total })
            }
            _ => None,
        };

        Ok(Self {
            amount,
            transaction_details: FiservcommercehubRefundTransactionDetails {
                merchant_transaction_id: router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            },
            merchant_details: FiservcommercehubMerchantDetails {
                merchant_id: auth.merchant_id.clone(),
                terminal_id: auth.terminal_id.clone(),
            },
            reference_transaction_details: FiservcommercehubReferenceTransactionDetails {
                reference_transaction_id: Some(
                    router_data.request.connector_transaction_id.clone(),
                ),
                reference_merchant_transaction_id: None,
            },
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubVoidResponse {
    pub gateway_response: FiservcommercehubGatewayResponseBody,
    pub payment_receipt: Option<FiservcommercehubPaymentReceipt>,
}

impl TryFrom<ResponseRouterData<FiservcommercehubVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiservcommercehubVoidResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::from(&item.response.gateway_response.transaction_state);
        let txn = &item
            .response
            .gateway_response
            .transaction_processing_details;
        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(txn.transaction_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: txn.order_id.clone(),
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

// =============================================================================
// POST-CAPTURE VOID (CANCEL BEFORE SETTLEMENT) FLOW
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubVoidPCRequest {
    pub transaction_details: FiservcommercehubRefundTransactionDetails,
    pub merchant_details: FiservcommercehubMerchantDetails,
    pub reference_transaction_details: FiservcommercehubReferenceTransactionDetails,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FiservcommercehubRouterData<
            RouterDataV2<
                VoidPC,
                PaymentFlowData,
                PaymentsCancelPostCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for FiservcommercehubVoidPCRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: super::FiservcommercehubRouterData<
            RouterDataV2<
                VoidPC,
                PaymentFlowData,
                PaymentsCancelPostCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let auth = FiservcommercehubAuthType::try_from(&router_data.connector_config)?;

        Ok(Self {
            transaction_details: FiservcommercehubRefundTransactionDetails {
                merchant_transaction_id: router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            },
            merchant_details: FiservcommercehubMerchantDetails {
                merchant_id: auth.merchant_id.clone(),
                terminal_id: auth.terminal_id.clone(),
            },
            reference_transaction_details: FiservcommercehubReferenceTransactionDetails {
                reference_transaction_id: Some(
                    router_data.request.connector_transaction_id.clone(),
                ),
                reference_merchant_transaction_id: None,
            },
        })
    }
}

impl TryFrom<ResponseRouterData<FiservcommercehubVoidResponse, Self>>
    for RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiservcommercehubVoidResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let gateway_response = item.response.gateway_response;
        let transaction_id = gateway_response
            .transaction_processing_details
            .transaction_id
            .clone();
        let post_capture_void_status = match &gateway_response.transaction_state {
            FiservcommercehubTransactionState::Cancelled
            | FiservcommercehubTransactionState::Voided => {
                common_enums::PostCaptureVoidStatus::Succeeded
            }
            FiservcommercehubTransactionState::Pending
            | FiservcommercehubTransactionState::Timeout => {
                common_enums::PostCaptureVoidStatus::Pending
            }
            FiservcommercehubTransactionState::Approved
            | FiservcommercehubTransactionState::Captured
            | FiservcommercehubTransactionState::Authorized
            | FiservcommercehubTransactionState::Declined
            | FiservcommercehubTransactionState::Rejected
            | FiservcommercehubTransactionState::Failed => {
                common_enums::PostCaptureVoidStatus::Failed
            }
        };

        let response = if post_capture_void_status.is_post_capture_void_failure() {
            build_payment_response(
                AttemptStatus::Failure,
                item.http_code,
                ResponseId::ConnectorTransactionId(transaction_id.clone()),
                Some(transaction_id),
                None,
                FiservcommercehubResponseExtras {
                    payment_tokens: None,
                    payment_receipt: item.response.payment_receipt.as_ref(),
                    network_details: None,
                },
            )
        } else {
            Ok(PaymentsResponseData::PostCaptureVoidResponse {
                post_capture_void_status,
                connector_reference_id: Some(transaction_id),
                status_code: item.http_code,
            })
        };

        Ok(Self {
            response,
            ..item.router_data
        })
    }
}

// =============================================================================
// INCREMENTAL AUTHORIZATION FLOW
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FiservcommercehubAuthorizationTypeIndicator {
    #[serde(rename = "INCREMENTAL")]
    Incremental,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubIncrementalAuthorizationTransactionDetails {
    pub authorization_type_indicator: FiservcommercehubAuthorizationTypeIndicator,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubIncrementalAuthorizationRequest {
    pub amount: FiservcommercehubAuthorizeAmount,
    pub transaction_details: FiservcommercehubIncrementalAuthorizationTransactionDetails,
    pub merchant_details: FiservcommercehubMerchantDetails,
    pub reference_transaction_details: FiservcommercehubReferenceTransactionDetails,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FiservcommercehubRouterData<
            RouterDataV2<
                IncrementalAuthorization,
                PaymentFlowData,
                PaymentsIncrementalAuthorizationData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for FiservcommercehubIncrementalAuthorizationRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: super::FiservcommercehubRouterData<
            RouterDataV2<
                IncrementalAuthorization,
                PaymentFlowData,
                PaymentsIncrementalAuthorizationData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;
        let auth = FiservcommercehubAuthType::try_from(&router_data.connector_config)?;
        let additional_amount = request.additional_money.as_ref().ok_or(
            errors::IntegrationError::MissingRequiredField {
                field_name: "additional_amount",
                context: errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Provide the positive amount to add to the current authorization"
                            .to_string(),
                    ),
                    doc_url: Some(FISERV_INCREMENTAL_AUTH_DOC_URL.to_string()),
                    additional_context: Some(
                        "Fiserv CommerceHub charges the supplied additional_amount as the increment, not the cumulative authorized total".to_string(),
                    ),
                },
            },
        )?;

        if additional_amount.currency() != request.currency
            || request.money.currency() != request.currency
            || additional_amount.amount() <= common_utils::types::MinorUnit::zero()
            || additional_amount.amount() > request.money.amount()
        {
            return Err(errors::IntegrationError::InvalidDataFormat {
                field_name: "additional_amount",
                context: errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Use a positive additional amount in the same currency as the new authorization total".to_string(),
                    ),
                    doc_url: Some(FISERV_INCREMENTAL_AUTH_DOC_URL.to_string()),
                    additional_context: Some(
                        "additional_amount must be positive, in the request currency, and no greater than the new total authorization amount".to_string(),
                    ),
                },
            }
            .into());
        }

        let total = utils::convert_amount(item.connector.amount_converter, additional_amount)?;
        let reference_transaction_id = request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(errors::IntegrationError::MissingConnectorTransactionID {
                context: errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Pass the connector transaction ID returned by the original Fiserv authorization".to_string(),
                    ),
                    doc_url: Some(FISERV_INCREMENTAL_AUTH_DOC_URL.to_string()),
                    additional_context: Some(
                        "Fiserv CommerceHub incremental authorization requires the original authorization transaction ID".to_string(),
                    ),
                },
            })?;

        Ok(Self {
            amount: FiservcommercehubAuthorizeAmount {
                currency: request.currency,
                total,
            },
            transaction_details: FiservcommercehubIncrementalAuthorizationTransactionDetails {
                authorization_type_indicator:
                    FiservcommercehubAuthorizationTypeIndicator::Incremental,
            },
            merchant_details: FiservcommercehubMerchantDetails {
                merchant_id: auth.merchant_id.clone(),
                terminal_id: auth.terminal_id.clone(),
            },
            reference_transaction_details: FiservcommercehubReferenceTransactionDetails {
                reference_transaction_id: Some(reference_transaction_id),
                reference_merchant_transaction_id: None,
            },
        })
    }
}

impl TryFrom<ResponseRouterData<FiservcommercehubAuthorizeResponse, Self>>
    for RouterDataV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiservcommercehubAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let txn = &item
            .response
            .gateway_response
            .transaction_processing_details;
        let authorization_status = common_enums::AuthorizationStatus::from(
            &item.response.gateway_response.transaction_state,
        );

        let response = match authorization_status {
            common_enums::AuthorizationStatus::Success
            | common_enums::AuthorizationStatus::Processing
            | common_enums::AuthorizationStatus::Unresolved => {
                Ok(PaymentsResponseData::IncrementalAuthorizationResponse {
                    status: authorization_status,
                    connector_authorization_id: Some(txn.transaction_id.clone()),
                    status_code: item.http_code,
                })
            }
            common_enums::AuthorizationStatus::Failure => build_payment_response(
                AttemptStatus::Failure,
                item.http_code,
                ResponseId::ConnectorTransactionId(txn.transaction_id.clone()),
                Some(txn.transaction_id.clone()),
                txn.order_id.clone(),
                FiservcommercehubResponseExtras {
                    payment_tokens: item.response.payment_tokens.as_ref(),
                    payment_receipt: item.response.payment_receipt.as_ref(),
                    network_details: item.response.network_details.as_ref(),
                },
            ),
        };

        Ok(Self {
            response,
            ..item.router_data
        })
    }
}

// =============================================================================
// ACCESS TOKEN FLOW
// =============================================================================

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubMerchantDetails {
    pub merchant_id: Secret<String>,
    pub terminal_id: Secret<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubAccessTokenRequest {
    pub merchant_details: FiservcommercehubMerchantDetails,
}

impl TryFrom<&ConnectorSpecificConfig> for FiservcommercehubAccessTokenRequest {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        let auth = FiservcommercehubAuthType::try_from(auth_type)?;
        Ok(Self {
            merchant_details: FiservcommercehubMerchantDetails {
                merchant_id: auth.merchant_id.clone(),
                terminal_id: auth.terminal_id.clone(),
            },
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FiservcommercehubRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for FiservcommercehubAccessTokenRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: super::FiservcommercehubRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&item.router_data.connector_config)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubTransactionProcessingDetails {
    pub api_key: Secret<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubGatewayResponse {
    pub transaction_processing_details: FiservcommercehubTransactionProcessingDetails,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubAsymmetricKeyDetails {
    pub key_id: String,
    pub encoded_public_key: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubAccessTokenResponse {
    pub gateway_response: FiservcommercehubGatewayResponse,
    pub asymmetric_key_details: FiservcommercehubAsymmetricKeyDetails,
}

impl<F, T> TryFrom<ResponseRouterData<FiservcommercehubAccessTokenResponse, Self>>
    for RouterDataV2<F, MerchantAuthenticationFlowData, T, ServerAuthenticationTokenResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiservcommercehubAccessTokenResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let key_id = &item.response.asymmetric_key_details.key_id;
        let encoded_public_key = &item.response.asymmetric_key_details.encoded_public_key;
        let combined_token = Secret::new(format!(
            "{key_id}{ACCESS_TOKEN_SEPARATOR}{encoded_public_key}"
        ));
        Ok(Self {
            response: Ok(ServerAuthenticationTokenResponseData {
                access_token: combined_token,
                expires_in: Some(604_800), // 1 week in seconds
                token_type: None,
            }),
            ..item.router_data
        })
    }
}

// =============================================================================
// CAPTURE FLOW
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubCaptureRequest {
    pub amount: FiservcommercehubAuthorizeAmount,
    pub transaction_details: FiservcommercehubTransactionDetailsReq,
    pub merchant_details: FiservcommercehubMerchantDetails,
    pub reference_transaction_details: FiservcommercehubReferenceTransactionDetails,
    /// Additional 3DS data for capture requests (optional)
    #[serde(rename = "additionalData3DS", skip_serializing_if = "Option::is_none")]
    pub additional_data_3ds: Option<FiservcommercehubAdditionalData3DS>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FiservcommercehubRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for FiservcommercehubCaptureRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: super::FiservcommercehubRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let total = utils::convert_amount(
            item.connector.amount_converter,
            &router_data.request.capture_money,
        )?;
        let auth = FiservcommercehubAuthType::try_from(&router_data.connector_config)?;
        let connector_transaction_id = router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(errors::IntegrationError::MissingConnectorTransactionID {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "connector_transaction_id is required for Capture".to_string(),
                    ),
                    doc_url: Some(FISERV_CHARGES_API_VERSION_URL.to_string()),
                    ..Default::default()
                },
            })?;
        let connector_metadata = parse_connector_metadata(router_data.request.metadata.as_ref())?;
        Ok(Self {
            amount: FiservcommercehubAuthorizeAmount {
                currency: router_data.request.currency,
                total,
            },
            transaction_details: FiservcommercehubTransactionDetailsReq {
                capture_flag: true,
                merchant_transaction_id: router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
                merchant_order_id: router_data.request.merchant_order_id.clone(),
                merchant_invoice_number: connector_metadata.merchant_invoice_id,
                create_token: None,
            },
            merchant_details: FiservcommercehubMerchantDetails {
                merchant_id: auth.merchant_id.clone(),
                terminal_id: auth.terminal_id.clone(),
            },
            reference_transaction_details: FiservcommercehubReferenceTransactionDetails {
                reference_transaction_id: Some(connector_transaction_id),
                reference_merchant_transaction_id: None,
            },
            // Note: Capture flow doesn't currently receive authentication_data
            // in PaymentsCaptureData. Set to None unless Fiserv requires it.
            additional_data_3ds: None,
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(transparent)]
pub struct FiservcommercehubCaptureResponse(pub FiservcommercehubAuthorizeResponse);

impl TryFrom<ResponseRouterData<FiservcommercehubCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiservcommercehubCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let txn = &item
            .response
            .0
            .gateway_response
            .transaction_processing_details;
        let status = AttemptStatus::from(&item.response.0.gateway_response.transaction_state);

        let connector_response = build_connector_response(
            item.response.0.additional_data_3ds.as_ref(),
            item.response.0.approval_code(),
        );
        let amount_captured =
            approved_amount_to_money(item.response.0.payment_receipt.as_ref(), item.http_code)?;
        let integrity_object = amount_captured
            .as_ref()
            .map(|amount| CaptureIntegrityObject {
                amount_to_capture: amount.amount(),
                currency: amount.currency(),
            });
        let response = build_transaction_payment_response(
            status,
            item.http_code,
            txn,
            FiservcommercehubResponseExtras {
                payment_tokens: item.response.0.payment_tokens.as_ref(),
                payment_receipt: item.response.0.payment_receipt.as_ref(),
                network_details: item.response.0.network_details.as_ref(),
            },
        );

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                captured_money: amount_captured
                    .or_else(|| item.router_data.resource_common_data.captured_money.clone()),
                connector_response,
                ..item.router_data.resource_common_data
            },
            request: PaymentsCaptureData {
                integrity_object,
                ..item.router_data.request
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// REPEAT PAYMENT FLOW
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubRepeatPaymentRequest {
    pub amount: FiservcommercehubAuthorizeAmount,
    pub source: FiservcommercehubSourceData,
    pub merchant_details: FiservcommercehubMerchantDetails,
    pub transaction_details: FiservcommercehubTransactionDetailsReq,
    pub transaction_interaction: FiservcommercehubTransactionInteractionReq,
    pub stored_credentials: FiservcommercehubStoredCredentials,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_details: Option<FiservcommercehubNetworkDetails>,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(transparent)]
pub struct FiservcommercehubRepeatResponse(pub FiservcommercehubAuthorizeResponse);

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FiservcommercehubRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for FiservcommercehubRepeatPaymentRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: super::FiservcommercehubRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;

        let total =
            utils::convert_amount(item.connector.amount_converter, &router_data.request.money)?;

        let auth = FiservcommercehubAuthType::try_from(&router_data.connector_config)?;

        // Extract card expiration details from additional_payment_data if available
        let card_info = router_data
            .request
            .additional_payment_data
            .as_ref()
            .and_then(|data| match data {
                domain_types::types::AdditionalPaymentData::Card(card_info) => {
                    match (&card_info.card_exp_month, &card_info.card_exp_year) {
                        (Some(month), Some(year)) => Some(FiservcommercehubTokenCardInfo {
                            expiration_month: Secret::new(format!("{:0>2}", month.peek())),
                            expiration_year: utils::expand_expiry_year_to_four_digits(year),
                        }),
                        _ => None,
                    }
                }
                domain_types::types::AdditionalPaymentData::Wallet(wallet) => {
                    let (exp_month, exp_year) = match wallet {
                        domain_types::payment_method_data::WalletAdditionalData::ApplePay(info) => {
                            (
                                info.card_info
                                    .as_ref()
                                    .and_then(|ci| ci.card_exp_month.as_ref()),
                                info.card_info
                                    .as_ref()
                                    .and_then(|ci| ci.card_exp_year.as_ref()),
                            )
                        }
                        domain_types::payment_method_data::WalletAdditionalData::GooglePay(
                            info,
                        ) => (
                            info.card_info
                                .as_ref()
                                .and_then(|ci| ci.card_exp_month.as_ref()),
                            info.card_info
                                .as_ref()
                                .and_then(|ci| ci.card_exp_year.as_ref()),
                        ),
                    };
                    match (exp_month, exp_year) {
                        (Some(month), Some(year)) => Some(FiservcommercehubTokenCardInfo {
                            expiration_month: Secret::new(format!("{:0>2}", month.peek())),
                            expiration_year: utils::expand_expiry_year_to_four_digits(year),
                        }),
                        _ => None,
                    }
                }
            });

        let (source, stored_credentials, network_details) = match &router_data
            .request
            .mandate_reference
        {
            MandateReferenceId::ConnectorMandateId(id) => {
                let connector_mandate_id = id.get_connector_mandate_id().ok_or(
                    errors::IntegrationError::MissingRequiredField {
                        field_name: "connector_mandate_id",
                        context: errors::IntegrationErrorContext {
                            additional_context: Some(
                                "connector_mandate_id is required for repeat payments".to_string(),
                            ),
                            doc_url: Some(FISERV_CHARGES_API_VERSION_URL.to_string()),
                            ..Default::default()
                        },
                    },
                )?;
                (
                    FiservcommercehubSourceData::PaymentToken(
                        FiservcommercehubPaymentTokenSource {
                            token_data: Secret::new(connector_mandate_id),
                            token_source: TOKEN_SOURCE_TRANSARMOR.to_string(),
                            decline_duplicates: Some(false),
                            card: card_info,
                            cryptogram: None,
                        },
                    ),
                    FiservcommercehubStoredCredentials::new_mit(
                        id.get_connector_mandate_request_reference_id(),
                        None,
                    ),
                    None,
                )
            }
            MandateReferenceId::NetworkMandateId(reference) => {
                let card = match &router_data.request.payment_method_data {
                    PaymentMethodData::CardDetailsForNetworkTransactionId(card) => card,
                    _ => {
                        return Err(error_stack::report!(
                            errors::IntegrationError::MissingRequiredField {
                                field_name:
                                    "payment_method_data.card_details_for_network_transaction_id",
                                context: errors::IntegrationErrorContext {
                                    doc_url: Some(FISERV_STORED_CREDENTIALS_DOC_URL.to_string()),
                                    ..Default::default()
                                },
                            }
                        ))
                    }
                };
                let (stored_credentials, network_details) =
                    stored_credentials_for_network_reference(
                        Some((
                            reference.network_transaction_id.as_str(),
                            reference.transaction_link_id.as_deref(),
                        )),
                        false,
                        true,
                    )?;
                let stored_credentials = stored_credentials.ok_or_else(|| {
                    error_stack::report!(errors::IntegrationError::MissingRequiredField {
                        field_name: "mandate_id.network_transaction_id",
                        context: Default::default(),
                    })
                })?;

                let access_token = router_data.resource_common_data.get_access_token()?;
                let (key_id, encoded_public_key) = access_token
                    .split_once(ACCESS_TOKEN_SEPARATOR)
                    .filter(|(key_id, public_key)| !key_id.is_empty() && !public_key.is_empty())
                    .ok_or_else(|| {
                        error_stack::report!(errors::IntegrationError::MissingRequiredField {
                            field_name: "fiserv_public_key",
                            context: Default::default(),
                        })
                    })?;
                let public_key_der = general_purpose::STANDARD
                    .decode(encoded_public_key)
                    .map_err(|_| {
                        error_stack::report!(errors::IntegrationError::RequestEncodingFailed {
                            context: Default::default(),
                        })
                    })?;
                let encrypted_card = encrypt_network_mit_card(
                    &card.card_number,
                    &card.card_exp_month,
                    &card.card_exp_year,
                    card.card_holder_name.as_ref(),
                    key_id.to_string(),
                    &public_key_der,
                )?;
                (
                    payment_card_source(encrypted_card),
                    stored_credentials,
                    network_details,
                )
            }
            MandateReferenceId::NetworkTokenWithNTI(reference) => {
                let token = match &router_data.request.payment_method_data {
                    PaymentMethodData::NetworkToken(token) => token,
                    _ => {
                        return Err(error_stack::report!(
                            errors::IntegrationError::MissingRequiredField {
                                field_name: "payment_method_data.network_token",
                                context: errors::IntegrationErrorContext {
                                    doc_url: Some(FISERV_STORED_CREDENTIALS_DOC_URL.to_string()),
                                    ..Default::default()
                                },
                            }
                        ))
                    }
                };
                let (stored_credentials, network_details) =
                    stored_credentials_for_network_reference(
                        Some((
                            reference.network_transaction_id.as_str(),
                            reference.transaction_link_id.as_deref(),
                        )),
                        false,
                        true,
                    )?;
                let stored_credentials = stored_credentials.ok_or_else(|| {
                    error_stack::report!(errors::IntegrationError::MissingRequiredField {
                        field_name: "mandate_id.network_transaction_id",
                        context: Default::default(),
                    })
                })?;

                (
                    FiservcommercehubSourceData::PaymentToken(
                        FiservcommercehubPaymentTokenSource {
                            token_data: Secret::new(token.get_network_token().peek().clone()),
                            token_source: TOKEN_SOURCE_NETWORK_TOKEN.to_string(),
                            decline_duplicates: None,
                            card: Some(FiservcommercehubTokenCardInfo {
                                expiration_month: Secret::new(format!(
                                    "{:02}",
                                    crate::utils::parse_expiry_month(
                                        token.get_network_token_expiry_month().peek(),
                                        "network_token.token_exp_month",
                                    )?
                                )),
                                expiration_year: token.get_expiry_year_4_digit(),
                            }),
                            // Fiserv requires a cryptogram for customer initiated token payments;
                            // subsequent merchant initiated payments use the scheme reference.
                            cryptogram: None,
                        },
                    ),
                    stored_credentials,
                    network_details,
                )
            }
        };

        let connector_metadata = parse_connector_metadata(router_data.request.metadata.as_ref())?;

        let request = Self {
            amount: FiservcommercehubAuthorizeAmount {
                currency: router_data.request.currency,
                total,
            },
            source,
            merchant_details: FiservcommercehubMerchantDetails {
                merchant_id: auth.merchant_id.clone(),
                terminal_id: auth.terminal_id.clone(),
            },
            transaction_details: FiservcommercehubTransactionDetailsReq {
                capture_flag: router_data.request.is_auto_capture(),
                merchant_transaction_id: router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
                merchant_order_id: router_data.request.merchant_order_id.clone(),
                merchant_invoice_number: connector_metadata.merchant_invoice_id,
                create_token: None,
            },
            transaction_interaction: FiservcommercehubTransactionInteractionReq {
                origin: FiservcommercehubOrigin::Ecom,
                eci_indicator: None,
            },
            stored_credentials,
            network_details,
        };
        Ok(request)
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<FiservcommercehubRepeatResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiservcommercehubRepeatResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Delegate to the Authorize response handling since the inner structure is identical
        let txn = &item
            .response
            .0
            .gateway_response
            .transaction_processing_details;
        let status = AttemptStatus::from(&item.response.0.gateway_response.transaction_state);

        // Build connector_response with 3DS authentication data or approval code if available
        let connector_response = build_connector_response(
            item.response.0.additional_data_3ds.as_ref(),
            item.response.0.approval_code(),
        );
        let response = build_transaction_payment_response(
            status,
            item.http_code,
            txn,
            FiservcommercehubResponseExtras {
                payment_tokens: item.response.0.payment_tokens.as_ref(),
                payment_receipt: item.response.0.payment_receipt.as_ref(),
                network_details: item.response.0.network_details.as_ref(),
            },
        );

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                connector_response,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// SETUP MANDATE FLOW (Tokenize Payment Source)
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubSetupMandateRequest {
    pub source: FiservcommercehubSourceData,
    pub merchant_details: FiservcommercehubMerchantDetails,
    pub transaction_details: FiservcommercehubSetupMandateTransactionDetails,
    pub transaction_interaction: FiservcommercehubTransactionInteractionReq,
    #[serde(rename = "additionalData3DS", skip_serializing_if = "Option::is_none")]
    pub additional_data_3ds: Option<FiservcommercehubAdditionalData3DS>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stored_credentials: Option<FiservcommercehubStoredCredentials>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubSetupMandateTransactionDetails {
    pub merchant_transaction_id: String,
    /// Required with stored credentials when creating a customer-authorized reusable token.
    pub account_verification: bool,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FiservcommercehubRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for FiservcommercehubSetupMandateRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: super::FiservcommercehubRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;

        // SetupMandate (tokenization) should not have an amount - it's for storing cards without charging
        if router_data
            .request
            .money
            .as_ref()
            .map(|money| money.amount())
            .is_some_and(|amount| amount > common_utils::types::MinorUnit::zero())
        {
            return Err(error_stack::report!(
                errors::IntegrationError::NotSupported {
                    message: "SetupMandate flow does not support amounts greater than 0"
                        .to_string(),
                    connector: "fiservcommercehub",
                    context: errors::IntegrationErrorContext {
                        additional_context: Some(
                            "SetupMandate is for tokenization only; amount must be 0".to_string(),
                        ),
                        doc_url: Some(FISERV_TOKEN_API_VERSION_URL.to_string()),
                        ..Default::default()
                    },
                }
            ));
        }

        let access_token = router_data.resource_common_data.get_access_token()?;
        let parts: Vec<&str> = access_token.split(ACCESS_TOKEN_SEPARATOR).collect();

        let key_id = parts
            .first()
            .filter(|key_id| !key_id.is_empty())
            .ok_or_else(|| {
                error_stack::report!(errors::IntegrationError::MissingRequiredField {
                    field_name: "key_id",
                    context: errors::IntegrationErrorContext {
                        doc_url: Some(FISERV_DECRYPTED_WALLET_DOC_URL.to_string()),
                        suggested_action: Some(
                            "Refresh the Fiserv access token and ensure its response contains the MUPK key id"
                                .to_string(),
                        ),
                        additional_context: Some(
                            "The access token did not include a key id required to encrypt the payment source"
                                .to_string(),
                        ),
                    },
                })
            })?
            .to_string();

        let encoded_public_key = parts.get(1).filter(|key| !key.is_empty()).ok_or_else(|| {
            error_stack::report!(errors::IntegrationError::MissingRequiredField {
                field_name: "encoded_public_key",
                context: errors::IntegrationErrorContext {
                    doc_url: Some(FISERV_DECRYPTED_WALLET_DOC_URL.to_string()),
                    suggested_action: Some(
                        "Refresh the Fiserv access token and ensure its response contains the MUPK public key"
                            .to_string(),
                    ),
                    additional_context: Some(
                        "The access token did not include the public key required to encrypt the payment source"
                            .to_string(),
                    ),
                },
            })
        })?;

        let public_key_der = general_purpose::STANDARD
            .decode(encoded_public_key)
            .map_err(|_| {
                error_stack::report!(errors::IntegrationError::InvalidDataFormat {
                    field_name: "access_token.encoded_public_key",
                    context: errors::IntegrationErrorContext {
                        doc_url: Some(FISERV_DECRYPTED_WALLET_DOC_URL.to_string()),
                        suggested_action: Some(
                            "Refresh the Fiserv access token and verify its MUPK public key is base64 encoded"
                                .to_string(),
                        ),
                        additional_context: Some(
                            "The MUPK public key from the access token could not be base64 decoded"
                                .to_string(),
                        ),
                    },
                })
            })
            .attach_printable("Failed to decode Base64 RSA public key")?;

        let auth_type = &router_data.connector_config;
        let auth = FiservcommercehubAuthType::try_from(auth_type)?;

        let (source, wallet_eci) = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card) => {
                let encrypted_card = encrypt_card_data(card, key_id, &public_key_der)?;

                (
                    FiservcommercehubSourceData::PaymentCard(FiservcommercehubPaymentCardSource {
                        encryption_data: FiservcommercehubEncryptionData {
                            key_id: encrypted_card.key_id,
                            encryption_type: FiservcommercehubEncryptionType::Rsa,
                            encryption_block: encrypted_card.encryption_block,
                            encryption_block_fields: encrypted_card.encryption_block_fields,
                        },
                    }),
                    None,
                )
            }
            PaymentMethodData::Wallet(wallet_data) => {
                let wallet_eci = get_decrypted_wallet_eci_indicator(wallet_data);

                let source = build_decrypted_wallet_source(wallet_data, key_id, &public_key_der)?;
                (source, wallet_eci)
            }
            _ => {
                return Err(error_stack::report!(
                    errors::IntegrationError::NotImplemented(
                        "Fiserv Commerce Hub SetupMandate supports only Card, decrypted Apple Pay, or \
                         decrypted Google Pay"
                            .to_string(),
                        errors::IntegrationErrorContext {
                            doc_url: Some(FISERV_TOKEN_API_VERSION_URL.to_string()),
                            suggested_action: Some(
                                "Provide card data or a supported decrypted Apple Pay or Google Pay wallet"
                                    .to_string(),
                            ),
                            additional_context: Some(
                                "Received an unsupported payment method type for the SetupMandate flow"
                                    .to_string(),
                            ),
                        },
                    )
                ));
            }
        };

        let origin = FiservcommercehubOrigin::from(router_data.request.payment_channel.as_ref());

        // SetupMandate is always a CIT (Card Holder Initiated Transaction)
        // as it's the first transaction where cardholder consents to store credentials
        let stored_credentials = Some(FiservcommercehubStoredCredentials::new_cit());

        let request = Self {
            source,
            merchant_details: FiservcommercehubMerchantDetails {
                merchant_id: auth.merchant_id.clone(),
                terminal_id: auth.terminal_id.clone(),
            },
            transaction_details: FiservcommercehubSetupMandateTransactionDetails {
                merchant_transaction_id: router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
                account_verification: true,
            },
            transaction_interaction: FiservcommercehubTransactionInteractionReq {
                origin,
                eci_indicator: router_data
                    .request
                    .authentication_data
                    .as_ref()
                    .and_then(|auth_data| auth_data.eci.as_deref())
                    .or(wallet_eci.as_deref())
                    .map(FiservcommercehubEciIndicator::try_from)
                    .transpose()?,
            },
            additional_data_3ds: build_additional_data_3ds(
                router_data.request.authentication_data.as_ref(),
            ),
            stored_credentials,
        };
        Ok(request)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubSetupMandateResponse {
    pub gateway_response: FiservcommercehubGatewayResponseBody,
    pub payment_receipt: Option<FiservcommercehubPaymentReceipt>,
    pub payment_tokens: Option<FiservcommercehubPaymentTokens>,
    pub network_details: Option<FiservcommercehubResponseNetworkDetails>,
    #[serde(rename = "additionalData3DS", skip_serializing_if = "Option::is_none")]
    pub additional_data_3ds: Option<serde_json::Value>,
}

impl FiservcommercehubSetupMandateResponse {
    fn approval_code(&self) -> Option<String> {
        self.payment_receipt
            .as_ref()
            .and_then(FiservcommercehubPaymentReceipt::approval_code)
    }
}

impl<F, T> TryFrom<ResponseRouterData<FiservcommercehubSetupMandateResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
where
    F: Clone,
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiservcommercehubSetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let txn = &item
            .response
            .gateway_response
            .transaction_processing_details;
        let txn_state = &item.response.gateway_response.transaction_state;
        let status = match txn_state {
            FiservcommercehubTransactionState::Authorized => AttemptStatus::Charged,
            _ => AttemptStatus::from(txn_state),
        };

        let connector_response = build_connector_response(
            item.response.additional_data_3ds.as_ref(),
            item.response.approval_code(),
        );
        let response = build_transaction_payment_response(
            status,
            item.http_code,
            txn,
            FiservcommercehubResponseExtras {
                payment_tokens: item.response.payment_tokens.as_ref(),
                payment_receipt: item.response.payment_receipt.as_ref(),
                network_details: item.response.network_details.as_ref(),
            },
        );

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                connector_response,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// API-DIRECT BROWSER 3DS
// =============================================================================
// The caller forwards typed authentication data between the granular RPCs and
// hosts Cardinal's DDC/challenge iframe. Browser completion triggers verification;
// only the server response can advance authentication toward authorization.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubAuthenticatedChargeRequest {
    amount: FiservcommercehubAuthorizeAmount,
    merchant_details: FiservcommercehubMerchantDetails,
    transaction_details: FiservcommercehubTransactionDetailsReq,
    #[serde(skip_serializing_if = "Option::is_none")]
    stored_credentials: Option<FiservcommercehubStoredCredentials>,
    #[serde(rename = "additionalData3DS")]
    additional_data_3ds: FiservcommercehubAuthenticationReference,
}

fn build_authenticated_charge_request<T: PaymentMethodDataTypes>(
    resource: &PaymentFlowData,
    config: &ConnectorSpecificConfig,
    request: &PaymentsAuthorizeData<T>,
    amount: FiservcommercehubAuthorizeAmount,
) -> Result<
    Option<FiservcommercehubAuthenticatedChargeRequest>,
    error_stack::Report<errors::IntegrationError>,
> {
    let authentication_stage = request
        .authentication_data
        .as_ref()
        .and_then(|authentication_data| authentication_data.authentication_stage);
    match authentication_stage {
        Some(_) => {
            build_managed_three_ds_charge_request(resource, config, request, amount).map(Some)
        }
        None => {
            validate_source_charge_authentication(resource, request)?;
            Ok(None)
        }
    }
}

fn build_managed_three_ds_charge_request<T: PaymentMethodDataTypes>(
    resource: &PaymentFlowData,
    config: &ConnectorSpecificConfig,
    request: &PaymentsAuthorizeData<T>,
    amount: FiservcommercehubAuthorizeAmount,
) -> Result<
    FiservcommercehubAuthenticatedChargeRequest,
    error_stack::Report<errors::IntegrationError>,
> {
    let auth = FiservcommercehubAuthType::try_from(config)?;
    // Authorization requires this completed stage, regardless of the supplied stage.
    let authentication_reference = FiservcommercehubAuthenticationReference::try_from((
        request.authentication_data.as_ref(),
        common_enums::AuthenticationStage::ReadyToAuthorize,
    ))?;

    let currency_matches = amount.currency == request.money.currency();
    let authentication_status = request
        .authentication_data
        .as_ref()
        .and_then(|authentication_data| authentication_data.trans_status.as_ref());
    match (currency_matches, authentication_status) {
        (false, _) => Err(error_stack::Report::new(
            errors::IntegrationError::InvalidDataFormat {
                field_name: "currency",
                context: errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Use the same currency in the authorization request and its \
                        money field."
                            .to_string(),
                    ),
                    doc_url: Some(FISERV_THREE_DS_DOC_URL.to_string()),
                    additional_context: Some(
                        "The authorization currency must match request.money.currency.".to_string(),
                    ),
                },
            },
        )),
        (
            true,
            Some(
                common_enums::TransactionStatus::Success
                | common_enums::TransactionStatus::NotVerified,
            ),
        ) => Ok(FiservcommercehubAuthenticatedChargeRequest {
            amount,
            merchant_details: build_three_ds_merchant_details(&auth),
            transaction_details: FiservcommercehubTransactionDetailsReq {
                capture_flag: request.is_auto_capture(),
                merchant_transaction_id: resource.connector_request_reference_id.clone(),
                merchant_order_id: request.merchant_order_id.clone(),
                merchant_invoice_number: parse_connector_metadata(request.metadata.as_ref())?
                    .merchant_invoice_id,
                create_token: None,
            },
            stored_credentials: request
                .is_customer_initiated_mandate_payment()
                .then(FiservcommercehubStoredCredentials::new_cit),
            additional_data_3ds: authentication_reference,
        }),
        (true, _) => Err(error_stack::Report::new(
            errors::IntegrationError::InvalidDataFormat {
                field_name: "authentication_data.trans_status",
                context: errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Complete 3DS authentication successfully before authorizing the \
                        payment."
                            .to_string(),
                    ),
                    doc_url: Some(FISERV_THREE_DS_DOC_URL.to_string()),
                    additional_context: Some(
                        "Authorization requires successful or accepted attempted \
                        authentication."
                            .to_string(),
                    ),
                },
            },
        )),
    }
}

fn validate_source_charge_authentication<T: PaymentMethodDataTypes>(
    resource: &PaymentFlowData,
    request: &PaymentsAuthorizeData<T>,
) -> Result<(), error_stack::Report<errors::IntegrationError>> {
    let authentication_data = request.authentication_data.as_ref();
    let authentication_status = authentication_data.and_then(|data| data.trans_status.as_ref());
    let requires_card_authentication = resource.auth_type == AuthenticationType::ThreeDs
        && matches!(
            &request.payment_method_data,
            PaymentMethodData::Card(_) | PaymentMethodData::CardWithNoCvc(_)
        );
    let has_external_authentication =
        authentication_data.is_some_and(AuthenticationData::has_external_authentication);
    let missing_authentication = requires_card_authentication && !has_external_authentication;

    // Challenge responses also contain DS IDs. An incomplete status must
    // remain blocked even if the caller loses the managed authentication stage.
    match (authentication_status, missing_authentication) {
        (
            Some(
                common_enums::TransactionStatus::Success
                | common_enums::TransactionStatus::NotVerified,
            )
            | None,
            false,
        ) => Ok(()),
        (
            Some(
                common_enums::TransactionStatus::Success
                | common_enums::TransactionStatus::NotVerified,
            )
            | None,
            true,
        ) => Err(error_stack::Report::new(
            errors::IntegrationError::InvalidDataFormat {
                field_name: "authentication_data",
                context: errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Forward the CommerceHub authentication data for the same \
                        payment attempt."
                            .to_string(),
                    ),
                    doc_url: Some(FISERV_THREE_DS_DOC_URL.to_string()),
                    additional_context: Some(
                        "Managed 3DS authorization requires completed authentication; \
                        start with PreAuthenticate."
                            .to_string(),
                    ),
                },
            },
        )),
        (Some(_), _) => Err(error_stack::Report::new(
            errors::IntegrationError::InvalidDataFormat {
                field_name: "authentication_data.trans_status",
                context: errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Complete 3DS authentication successfully before authorizing the \
                        payment."
                            .to_string(),
                    ),
                    doc_url: Some(FISERV_THREE_DS_DOC_URL.to_string()),
                    additional_context: Some(
                        "Pending, failed or unsupported 3DS authentication cannot \
                        authorize a payment."
                            .to_string(),
                    ),
                },
            },
        )),
    }
}

fn build_three_ds_merchant_details(
    auth: &FiservcommercehubAuthType,
) -> FiservcommercehubMerchantDetails {
    FiservcommercehubMerchantDetails {
        merchant_id: auth.merchant_id.clone(),
        terminal_id: auth.terminal_id.clone(),
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FiservcommercehubAuthenticationReference {
    authentication_transaction_id: String,
}

impl
    TryFrom<(
        Option<&AuthenticationData>,
        common_enums::AuthenticationStage,
    )> for FiservcommercehubAuthenticationReference
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        (authentication_data, expected_stage): (
            Option<&AuthenticationData>,
            common_enums::AuthenticationStage,
        ),
    ) -> Result<Self, Self::Error> {
        let authentication_stage = authentication_data
            .and_then(|authentication_data| authentication_data.authentication_stage);
        match authentication_stage {
            Some(stage) if stage == expected_stage => Ok(()),
            _ => Err(errors::IntegrationError::InvalidDataFormat {
                field_name: "authentication_data.authentication_stage",
                context: errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Forward the previous CommerceHub authentication response and call the \
                        operation appropriate for its stage."
                            .to_string(),
                    ),
                    doc_url: Some(FISERV_THREE_DS_DOC_URL.to_string()),
                    additional_context: Some(format!(
                        "Expected authentication stage {expected_stage:?}; received {authentication_stage:?}."
                    )),
                },
            }),
        }?;
        let authentication_transaction_id = authentication_data
            .and_then(|authentication_data| authentication_data.transaction_id.as_ref())
            .filter(|transaction_id| !transaction_id.trim().is_empty())
            .ok_or_else(|| errors::IntegrationError::MissingRequiredField {
                field_name: "authentication_data.transaction_id",
                context: errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Forward authentication_data from the previous CommerceHub 3DS response \
                        for this payment attempt."
                            .to_string(),
                    ),
                    doc_url: Some(FISERV_THREE_DS_DOC_URL.to_string()),
                    additional_context: Some(
                        "The authentication reference must be the Fiserv authenticationTransactionId, \
                        not a charge ID or the browser's challenge TransactionId."
                            .to_string(),
                    ),
                },
            })?;

        Ok(Self {
            authentication_transaction_id: authentication_transaction_id.clone(),
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FiservcommercehubThreeDsCaptureDetails {
    capture_flag: bool,
}

#[derive(Debug, Serialize)]
#[serde(tag = "sourceType")]
enum FiservcommercehubThreeDsCardSource {
    PaymentCard {
        #[serde(rename = "encryptionData")]
        encryption_data: FiservcommercehubThreeDsEncryptionData,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FiservcommercehubThreeDsEncryptionData {
    #[serde(flatten)]
    card: FiservcommercehubEncryptionData,
    encryption_target: FiservcommercehubEncryptionTarget,
}

fn encrypt_three_ds_card_source<T: PaymentMethodDataTypes>(
    payment_method: Option<&PaymentMethodData<T>>,
    resource: &PaymentFlowData,
) -> Result<FiservcommercehubThreeDsCardSource, error_stack::Report<errors::IntegrationError>> {
    let payment_method = payment_method.ok_or_else(|| {
        error_stack::Report::new(errors::IntegrationError::MissingRequiredField {
            field_name: "payment_method_data",
            context: errors::IntegrationErrorContext {
                suggested_action: Some(
                    "Provide card payment data for CommerceHub browser 3DS.".to_string(),
                ),
                doc_url: Some(FISERV_THREE_DS_DOC_URL.to_string()),
                additional_context: Some(
                    "A required CommerceHub 3DS request field is missing.".to_string(),
                ),
            },
        })
    })?;
    let (key_id, public_key) = encryption_key(resource)?;
    let encrypted = match payment_method {
        PaymentMethodData::Card(card) => encrypt_card_data(card, key_id, &public_key)?,
        PaymentMethodData::CardWithNoCvc(card) => {
            encrypt_card_data_no_cvc(card, key_id, &public_key)?
        }
        _ => {
            return Err(error_stack::Report::new(
                errors::IntegrationError::InvalidDataFormat {
                    field_name: "payment_method_data",
                    context: errors::IntegrationErrorContext {
                        suggested_action: Some(
                            "Provide card payment data for CommerceHub browser 3DS.".to_string(),
                        ),
                        doc_url: Some(FISERV_THREE_DS_DOC_URL.to_string()),
                        additional_context: Some(
                            "CommerceHub API-direct browser 3DS supports card payments only."
                                .to_string(),
                        ),
                    },
                },
            ))
        }
    };
    Ok(FiservcommercehubThreeDsCardSource::PaymentCard {
        encryption_data: FiservcommercehubThreeDsEncryptionData {
            card: FiservcommercehubEncryptionData {
                key_id: encrypted.key_id,
                encryption_type: FiservcommercehubEncryptionType::Rsa,
                encryption_block: encrypted.encryption_block,
                encryption_block_fields: encrypted.encryption_block_fields,
            },
            encryption_target: FiservcommercehubEncryptionTarget::Manual,
        },
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubPreAuthenticateRequest {
    amount: FiservcommercehubAuthorizeAmount,
    source: FiservcommercehubThreeDsCardSource,
    merchant_details: FiservcommercehubMerchantDetails,
    transaction_details: FiservcommercehubThreeDsCaptureDetails,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FiservcommercehubRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for FiservcommercehubPreAuthenticateRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;
    fn try_from(
        item: super::FiservcommercehubRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let data = &item.router_data;
        let auth = FiservcommercehubAuthType::try_from(&data.connector_config)?;
        Ok(Self {
            amount: FiservcommercehubAuthorizeAmount {
                currency: data.request.money.currency(),
                total: utils::convert_amount(item.connector.amount_converter, &data.request.money)?,
            },
            source: encrypt_three_ds_card_source(
                data.request.payment_method_data.as_ref(),
                &data.resource_common_data,
            )?,
            merchant_details: build_three_ds_merchant_details(&auth),
            transaction_details: FiservcommercehubThreeDsCaptureDetails {
                capture_flag: data.request.is_auto_capture()?,
            },
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "MessageType")]
enum FiservcommercehubDeviceCollectionMessage {
    #[serde(rename = "profile.completed")]
    Completed {
        #[serde(rename = "Payload")]
        payload: Option<FiservcommercehubDeviceCollectionPayload>,
        #[serde(rename = "SessionId")]
        session_id: Option<String>,
        #[serde(rename = "Status")]
        status: Option<bool>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct FiservcommercehubDeviceCollectionPayload {
    action_code: String,
    session_id: Option<String>,
}

fn device_collection_reference(
    redirect: Option<&ContinueRedirectionResponse>,
) -> Result<Option<String>, error_stack::Report<errors::IntegrationError>> {
    let Some(payload) = redirect.and_then(|response| response.payload.as_ref()) else {
        return Ok(None);
    };
    let mut collection_message = payload.peek().clone();
    // RedirectionResponse.payload is a proto map<string, string>. Decode only
    // the JSON-valued DDC fields; session IDs must remain strings.
    for field_name in ["Payload", "Status"] {
        let Some(field_value) = collection_message.get_mut(field_name) else {
            continue;
        };
        if let serde_json::Value::String(encoded_value) = field_value {
            *field_value = serde_json::from_str(encoded_value).change_context(
                errors::IntegrationError::InvalidDataFormat {
                    field_name: "redirect_response.payload",
                    context: errors::IntegrationErrorContext {
                        suggested_action: Some(
                            "JSON-encode the Cardinal Payload object and Status boolean when \
                            forwarding them through the redirection response."
                                .to_string(),
                        ),
                        doc_url: Some(FISERV_THREE_DS_DOC_URL.to_string()),
                        additional_context: Some(format!(
                            "The Cardinal {field_name} field is not valid JSON."
                        )),
                    },
                },
            )?;
        }
    }
    let message: FiservcommercehubDeviceCollectionMessage = serde_json::from_value(
        collection_message,
    )
    .change_context(errors::IntegrationError::InvalidDataFormat {
        field_name: "redirect_response.payload",
        context: errors::IntegrationErrorContext {
            suggested_action: Some(
                "Forward the Cardinal profile.completed payload, including its \
                session ID."
                    .to_string(),
            ),
            doc_url: Some(FISERV_THREE_DS_DOC_URL.to_string()),
            additional_context: Some(
                "Expected Cardinal profile.completed JSON from the DDC iframe.".to_string(),
            ),
        },
    })?;
    let FiservcommercehubDeviceCollectionMessage::Completed {
        payload,
        session_id,
        status,
    } = message;
    let (completed, reference) = match payload {
        Some(payload) => (payload.action_code == "SUCCESS", payload.session_id),
        None => (status == Some(true), session_id),
    };
    if completed {
        let reference = reference
            .filter(|id| !id.trim().is_empty())
            .ok_or_else(|| {
                error_stack::Report::new(errors::IntegrationError::MissingRequiredField {
                    field_name: "redirect_response.payload.SessionId",
                    context: errors::IntegrationErrorContext {
                        suggested_action: Some(
                            "Forward the Cardinal profile.completed payload, including its \
                            session ID."
                                .to_string(),
                        ),
                        doc_url: Some(FISERV_THREE_DS_DOC_URL.to_string()),
                        additional_context: Some(
                            "A required CommerceHub 3DS request field is missing.".to_string(),
                        ),
                    },
                })
            })?;
        Ok(Some(reference))
    } else {
        Ok(None)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubAuthenticateRequest {
    amount: FiservcommercehubAuthorizeAmount,
    source: FiservcommercehubThreeDsCardSource,
    merchant_details: FiservcommercehubMerchantDetails,
    #[serde(rename = "additionalData3DS")]
    additional_data_3ds: FiservcommercehubAuthenticateData,
    #[serde(skip_serializing_if = "Option::is_none")]
    transaction_details: Option<FiservcommercehubFingerprintDetails>,
    #[serde(skip_serializing_if = "Option::is_none")]
    customer: Option<FiservcommercehubThreeDsCustomer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    billing_address: Option<FiservcommercehubThreeDsAddress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_address: Option<FiservcommercehubThreeDsAddress>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FiservcommercehubAuthenticateData {
    authentication_transaction_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    service_provider_reference_id: Option<String>,
    channel: FiservcommercehubThreeDsChannel,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum FiservcommercehubThreeDsChannel {
    Browser,
}

#[derive(Debug, Serialize)]
struct FiservcommercehubThreeDsCustomer {
    email: Email,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FiservcommercehubThreeDsAddress {
    #[serde(skip_serializing_if = "Option::is_none")]
    first_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_name: Option<Secret<String>>,
    address: FiservcommercehubThreeDsStreetAddress,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FiservcommercehubThreeDsStreetAddress {
    #[serde(skip_serializing_if = "Option::is_none")]
    street: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    house_number_or_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state_or_province: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    postal_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    country: Option<common_enums::CountryAlpha2>,
}

fn build_three_ds_address(
    value: Option<&domain_types::payment_address::Address>,
) -> Option<FiservcommercehubThreeDsAddress> {
    value
        .and_then(|value| value.address.as_ref())
        .map(|value| FiservcommercehubThreeDsAddress {
            first_name: value.first_name.clone(),
            last_name: value.last_name.clone(),
            address: FiservcommercehubThreeDsStreetAddress {
                street: value.line1.clone(),
                house_number_or_name: value.line2.clone(),
                city: value.city.clone(),
                state_or_province: value.get_state().ok().cloned(),
                postal_code: value.zip.clone(),
                country: value.country,
            },
        })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FiservcommercehubFingerprintDetails {
    device_fingerprint: [FiservcommercehubFingerprint; 2],
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
enum FiservcommercehubFingerprint {
    DataStatic(FiservcommercehubStaticFingerprint),
    DataDynamic(FiservcommercehubDynamicFingerprint),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FiservcommercehubStaticFingerprint {
    color_depth: u8,
    screen_height: u32,
    screen_width: u32,
    timezone_offset: i32,
    java_enabled: bool,
    java_script_enabled: bool,
    locale: String,
    accepts: String,
    user_agent: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FiservcommercehubDynamicFingerprint {
    ip_address: Secret<String>,
}

impl TryFrom<&BrowserInformation> for FiservcommercehubFingerprintDetails {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(browser: &BrowserInformation) -> Result<Self, Self::Error> {
        let locale = browser.get_language()?;
        let accepts = browser.get_accept_header()?;
        if locale.is_empty() || locale.len() > 8 || accepts.is_empty() || accepts.len() > 256 {
            return Err(errors::IntegrationError::InvalidDataFormat {
                field_name: "browser_info",
                context: errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Collect all required browser fields when Cardinal device data \
                        collection is unavailable."
                            .to_string(),
                    ),
                    doc_url: Some(FISERV_THREE_DS_DOC_URL.to_string()),
                    additional_context: Some(
                        "DDC fallback requires a locale of 1–8 characters and an Accept \
                        header of 1–256 characters."
                            .to_string(),
                    ),
                },
            }
            .into());
        }
        let user_agent = browser.get_user_agent()?;
        Ok(Self {
            device_fingerprint: [
                FiservcommercehubFingerprint::DataStatic(FiservcommercehubStaticFingerprint {
                    color_depth: browser.get_color_depth()?,
                    screen_height: browser.get_screen_height()?,
                    screen_width: browser.get_screen_width()?,
                    timezone_offset: browser.get_time_zone()?,
                    java_enabled: browser.get_java_enabled()?,
                    java_script_enabled: browser.get_java_script_enabled()?,
                    locale,
                    accepts,
                    user_agent: user_agent.chars().take(2048).collect(),
                }),
                FiservcommercehubFingerprint::DataDynamic(FiservcommercehubDynamicFingerprint {
                    ip_address: Secret::new(browser.get_ip_address()?.expose()),
                }),
            ],
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FiservcommercehubRouterData<
            RouterDataV2<
                Authenticate,
                PaymentFlowData,
                PaymentsAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for FiservcommercehubAuthenticateRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;
    fn try_from(
        item: super::FiservcommercehubRouterData<
            RouterDataV2<
                Authenticate,
                PaymentFlowData,
                PaymentsAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let data = &item.router_data;
        if data.request.device_channel == Some(DeviceChannel::App)
            || data.request.sdk_information.is_some()
        {
            return Err(error_stack::Report::new(
                errors::IntegrationError::InvalidDataFormat {
                    field_name: "device_channel",
                    context: errors::IntegrationErrorContext {
                        suggested_action: Some(
                            "Use the BROWSER channel for CommerceHub API-direct 3DS.".to_string(),
                        ),
                        doc_url: Some(FISERV_THREE_DS_DOC_URL.to_string()),
                        additional_context: Some(
                            "CommerceHub API-direct 3DS currently supports the BROWSER channel \
                            only."
                                .to_string(),
                        ),
                    },
                },
            ));
        }
        let auth = FiservcommercehubAuthType::try_from(&data.connector_config)?;
        let authentication_reference = FiservcommercehubAuthenticationReference::try_from((
            data.request.authentication_data.as_ref(),
            common_enums::AuthenticationStage::DeviceDataCollectionPending,
        ))?;
        let service_provider_reference_id =
            device_collection_reference(data.request.redirect_response.as_ref())?;
        let transaction_details = if service_provider_reference_id.is_none() {
            let browser = data
                .request
                .browser_info
                .as_ref()
                .ok_or_else(utils::missing_field_err("browser_info"))?;
            Some(FiservcommercehubFingerprintDetails::try_from(browser)?)
        } else {
            None
        };
        Ok(Self {
            amount: FiservcommercehubAuthorizeAmount {
                currency: data.request.money.currency(),
                total: utils::convert_amount(item.connector.amount_converter, &data.request.money)?,
            },
            source: encrypt_three_ds_card_source(
                data.request.payment_method_data.as_ref(),
                &data.resource_common_data,
            )?,
            merchant_details: build_three_ds_merchant_details(&auth),
            additional_data_3ds: FiservcommercehubAuthenticateData {
                authentication_transaction_id: authentication_reference
                    .authentication_transaction_id,
                service_provider_reference_id,
                channel: FiservcommercehubThreeDsChannel::Browser,
            },
            transaction_details,
            customer: data
                .request
                .email
                .clone()
                .or_else(|| data.resource_common_data.get_optional_billing_email())
                .map(|email| FiservcommercehubThreeDsCustomer { email }),
            billing_address: build_three_ds_address(
                data.resource_common_data.get_optional_billing(),
            ),
            shipping_address: build_three_ds_address(
                data.resource_common_data.get_optional_shipping(),
            ),
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubPostAuthenticateRequest {
    merchant_details: FiservcommercehubMerchantDetails,
    #[serde(rename = "additionalData3DS")]
    additional_data_3ds: FiservcommercehubAuthenticationReference,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FiservcommercehubRouterData<
            RouterDataV2<
                PostAuthenticate,
                PaymentFlowData,
                PaymentsPostAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for FiservcommercehubPostAuthenticateRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;
    fn try_from(
        item: super::FiservcommercehubRouterData<
            RouterDataV2<
                PostAuthenticate,
                PaymentFlowData,
                PaymentsPostAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let data = &item.router_data;
        let auth = FiservcommercehubAuthType::try_from(&data.connector_config)?;
        // The browser's stepUp.completion.TransactionId is not the Fiserv ID.
        // Use the authentication reference forwarded by the server instead.
        Ok(Self {
            merchant_details: build_three_ds_merchant_details(&auth),
            additional_data_3ds: FiservcommercehubAuthenticationReference::try_from((
                data.request.authentication_data.as_ref(),
                common_enums::AuthenticationStage::ChallengePending,
            ))?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubThreeDsResponse {
    gateway_response: FiservcommercehubThreeDsGatewayResponse,
    processor_response_details: FiservcommercehubThreeDsProcessorResponse,
    #[serde(rename = "additionalData3DS")]
    additional_data_3ds: Option<FiservcommercehubAuthenticationResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FiservcommercehubThreeDsGatewayResponse {
    transaction_type: String,
    transaction_state: FiservcommercehubThreeDsTransactionState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum FiservcommercehubThreeDsTransactionState {
    Initialized,
    Authenticated,
    Waiting,
    // Unrecognized states must never satisfy initialization or authentication success checks.
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FiservcommercehubThreeDsProcessorResponse {
    approval_status: Option<String>,
    response_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FiservcommercehubAuthenticationResult {
    authentication_transaction_id: String,
    service_provider: Option<String>,
    authentication_status: Option<String>,
    challenge_indicator: Option<bool>,
    challenge_token: Option<Secret<String>>,
    step_up_url: Option<String>,
    method_data: Option<FiservcommercehubMethodData>,
    server_transaction_id: Option<String>,
    ds_transaction_id: Option<String>,
    acs_transaction_id: Option<String>,
    mpi_data: Option<FiservcommercehubThreeDsMpiData>,
    version_data: Option<FiservcommercehubThreeDsVersionData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FiservcommercehubMethodData {
    encoded_token: Secret<String>,
    data_collection_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FiservcommercehubThreeDsMpiData {
    cavv: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FiservcommercehubThreeDsVersionData {
    recommended_version: common_utils::types::SemanticVersion,
}

fn jwt_redirect(
    endpoint: Option<&str>,
    token: Option<&Secret<String>>,
    http_status: u16,
) -> Result<Box<RedirectForm>, error_stack::Report<errors::ConnectorError>> {
    let endpoint = endpoint.ok_or_else(|| {
        error_stack::Report::new(
            errors::ConnectorError::response_handling_failed_with_context(
                http_status,
                Some("CommerceHub omitted the DDC/challenge URL.".to_string()),
            ),
        )
    })?;
    let url = url::Url::parse(endpoint).change_context(
        errors::ConnectorError::response_handling_failed_with_context(
            http_status,
            Some("Invalid CommerceHub DDC/challenge URL.".to_string()),
        ),
    )?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(error_stack::Report::new(
            errors::ConnectorError::response_handling_failed_with_context(
                http_status,
                Some(
                    "CommerceHub DDC/challenge URL must use HTTPS without credentials.".to_string(),
                ),
            ),
        ));
    }
    let token = token
        .filter(|token| !token.peek().trim().is_empty())
        .ok_or_else(|| {
            error_stack::Report::new(
                errors::ConnectorError::response_handling_failed_with_context(
                    http_status,
                    Some("CommerceHub omitted the DDC/challenge JWT.".to_string()),
                ),
            )
        })?;
    Ok(Box::new(RedirectForm::Form {
        endpoint: endpoint.to_string(),
        method: Method::Post,
        form_fields: HashMap::from([("JWT".to_string(), token.peek().clone())]),
    }))
}

impl FiservcommercehubAuthenticationResult {
    fn authentication_data(
        &self,
        authentication_stage: common_enums::AuthenticationStage,
    ) -> AuthenticationData {
        AuthenticationData {
            authentication_stage: Some(authentication_stage),
            trans_status: match self.authentication_status.as_deref() {
                Some("Y") => Some(common_enums::TransactionStatus::Success),
                Some("A") => Some(common_enums::TransactionStatus::NotVerified),
                Some("C") => Some(common_enums::TransactionStatus::ChallengeRequired),
                Some("N") => Some(common_enums::TransactionStatus::Failure),
                Some("R") => Some(common_enums::TransactionStatus::Rejected),
                Some("U") => Some(common_enums::TransactionStatus::VerificationNotPerformed),
                // Initialize may omit this field. Fiserv's Additional Data 3DS reference
                // does not enumerate all status codes; authentication_outcome rejects
                // missing or unsupported values before Authenticate/Verify can advance.
                _ => None,
            },
            transaction_id: Some(self.authentication_transaction_id.clone()),
            cavv: self.mpi_data.as_ref().and_then(|data| data.cavv.clone()),
            threeds_server_transaction_id: self.server_transaction_id.clone(),
            ds_trans_id: self.ds_transaction_id.clone(),
            acs_transaction_id: self.acs_transaction_id.clone(),
            message_version: self
                .version_data
                .as_ref()
                .map(|data| data.recommended_version.clone()),
            // SECURE_ECOM is a category, not a scheme-specific numeric ECI.
            eci: None,
            ucaf_collection_indicator: None,
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
}

struct FiservcommercehubAuthenticationOutcome {
    attempt_status: AttemptStatus,
    authentication_data: AuthenticationData,
    redirection_data: Option<Box<RedirectForm>>,
}

impl FiservcommercehubThreeDsResponse {
    fn authentication_result(
        &self,
        http_status: u16,
    ) -> Result<&FiservcommercehubAuthenticationResult, error_stack::Report<errors::ConnectorError>>
    {
        let authentication_result = self.additional_data_3ds.as_ref().ok_or_else(|| {
            error_stack::Report::new(
                errors::ConnectorError::response_handling_failed_with_context(
                    http_status,
                    Some("CommerceHub omitted additionalData3DS.".to_string()),
                ),
            )
        })?;
        if authentication_result
            .authentication_transaction_id
            .trim()
            .is_empty()
            || authentication_result
                .service_provider
                .as_deref()
                .is_some_and(|provider| provider != "CARDINAL")
        {
            return Err(error_stack::Report::new(
                errors::ConnectorError::response_handling_failed_with_context(
                    http_status,
                    Some(
                        "CommerceHub returned an empty authentication ID or unsupported \
                        3DS provider."
                            .to_string(),
                    ),
                ),
            ));
        }
        Ok(authentication_result)
    }

    fn authentication_outcome(
        &self,
        authentication_reference: &FiservcommercehubAuthenticationReference,
        expected_stage: common_enums::AuthenticationStage,
        http_status: u16,
    ) -> Result<FiservcommercehubAuthenticationOutcome, error_stack::Report<errors::ConnectorError>>
    {
        let authentication_result = self.authentication_result(http_status)?;
        if self.gateway_response.transaction_type != "AUTHENTICATION"
            || authentication_result.authentication_transaction_id
                != authentication_reference.authentication_transaction_id
        {
            return Err(error_stack::Report::new(
                errors::ConnectorError::response_handling_failed_with_context(
                    http_status,
                    Some(
                        "CommerceHub returned the wrong operation type or a different \
                        authentication transaction ID."
                            .to_string(),
                    ),
                ),
            ));
        }
        let authentication_status = authentication_result.authentication_status.as_deref();
        let (next_stage, attempt_status, redirection_data) = match authentication_status {
            Some("C")
                if expected_stage
                    == common_enums::AuthenticationStage::DeviceDataCollectionPending
                    && self.gateway_response.transaction_state
                        == FiservcommercehubThreeDsTransactionState::Waiting
                    && self.processor_response_details.response_code == "364"
                    && authentication_result.challenge_indicator == Some(true) =>
            {
                (
                    common_enums::AuthenticationStage::ChallengePending,
                    AttemptStatus::AuthenticationPending,
                    Some(jwt_redirect(
                        authentication_result.step_up_url.as_deref(),
                        authentication_result.challenge_token.as_ref(),
                        http_status,
                    )?),
                )
            }
            // Fiserv documents A (attempted) on APPROVED/AUTHENTICATED responses.
            // Permit the payment while preserving A as NotVerified for callers.
            Some("Y" | "A")
                if self.gateway_response.transaction_state
                    == FiservcommercehubThreeDsTransactionState::Authenticated
                    && self.processor_response_details.response_code == "000"
                    && self.processor_response_details.approval_status.as_deref()
                        == Some("APPROVED")
                    && authentication_result.challenge_indicator != Some(true) =>
            {
                (
                    common_enums::AuthenticationStage::ReadyToAuthorize,
                    AttemptStatus::AuthenticationSuccessful,
                    None,
                )
            }
            Some("N" | "R" | "U") => (
                common_enums::AuthenticationStage::Failed,
                AttemptStatus::AuthenticationFailed,
                None,
            ),
            _ => {
                return Err(error_stack::Report::new(
                    errors::ConnectorError::response_handling_failed_with_context(
                        http_status,
                        Some(format!(
                            "CommerceHub 3DS is incomplete, unsupported or inconsistent; \
                            authorization is blocked. authenticationStatus={authentication_status:?}, \
                            transactionState={:?}, responseCode={}, challengeIndicator={:?}.",
                            self.gateway_response.transaction_state,
                            self.processor_response_details.response_code,
                            authentication_result.challenge_indicator,
                        )),
                    ),
                ))
            }
        };
        Ok(FiservcommercehubAuthenticationOutcome {
            attempt_status,
            authentication_data: authentication_result.authentication_data(next_stage),
            redirection_data,
        })
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<FiservcommercehubThreeDsResponse, Self>>
    for RouterDataV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(
        response_router_data: ResponseRouterData<FiservcommercehubThreeDsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response: connector_response,
            mut router_data,
            http_code: http_status_code,
        } = response_router_data;
        let authentication_result = connector_response.authentication_result(http_status_code)?;
        let gateway_response = &connector_response.gateway_response;
        let processor_response = &connector_response.processor_response_details;
        match (
            gateway_response.transaction_type.as_str(),
            gateway_response.transaction_state,
            processor_response.response_code.as_str(),
        ) {
            ("INITIALIZE", FiservcommercehubThreeDsTransactionState::Initialized, "000") => Ok(()),
            _ => Err(
                errors::ConnectorError::response_handling_failed_with_context(
                    http_status_code,
                    Some("CommerceHub device-data initialization was not successful.".to_string()),
                ),
            ),
        }?;

        let device_collection_data =
            authentication_result.method_data.as_ref().ok_or_else(|| {
                error_stack::Report::new(
                    errors::ConnectorError::response_handling_failed_with_context(
                        http_status_code,
                        Some("CommerceHub initialization omitted methodData.".to_string()),
                    ),
                )
            })?;
        let device_collection_redirect = jwt_redirect(
            Some(&device_collection_data.data_collection_url),
            Some(&device_collection_data.encoded_token),
            http_status_code,
        )?;
        router_data.resource_common_data.status = AttemptStatus::AuthenticationPending;
        router_data.response = Ok(PaymentsResponseData::PreAuthenticateResponse {
            resource_id: None,
            authentication_data: Some(authentication_result.authentication_data(
                common_enums::AuthenticationStage::DeviceDataCollectionPending,
            )),
            redirection_data: Some(device_collection_redirect),
            connector_response_reference_id: Some(
                authentication_result.authentication_transaction_id.clone(),
            ),
            status_code: http_status_code,
        });
        Ok(router_data)
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<FiservcommercehubThreeDsResponse, Self>>
    for RouterDataV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(
        response_router_data: ResponseRouterData<FiservcommercehubThreeDsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response: connector_response,
            mut router_data,
            http_code: http_status_code,
        } = response_router_data;
        let authentication_reference = FiservcommercehubAuthenticationReference::try_from((
            router_data.request.authentication_data.as_ref(),
            common_enums::AuthenticationStage::DeviceDataCollectionPending,
        ))
        .change_context(
            errors::ConnectorError::response_handling_failed_with_context(
                http_status_code,
                Some(
                    "Authenticate request is missing its CommerceHub authentication reference."
                        .to_string(),
                ),
            ),
        )?;
        let authentication_outcome = connector_response.authentication_outcome(
            &authentication_reference,
            common_enums::AuthenticationStage::DeviceDataCollectionPending,
            http_status_code,
        )?;
        let connector_feature_data = router_data
            .resource_common_data
            .connector_feature_data
            .as_ref()
            .map(|feature_data| feature_data.peek().clone());

        router_data.resource_common_data.status = authentication_outcome.attempt_status;
        router_data.response = Ok(PaymentsResponseData::AuthenticateResponse {
            resource_id: None,
            redirection_data: authentication_outcome.redirection_data,
            authentication_data: Some(authentication_outcome.authentication_data),
            connector_feature_data,
            connector_response_reference_id: Some(
                authentication_reference.authentication_transaction_id,
            ),
            status_code: http_status_code,
            mandate_reference: None,
            network_txn_id: None,
            network_txn_link_id: None,
        });
        Ok(router_data)
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<FiservcommercehubThreeDsResponse, Self>>
    for RouterDataV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(
        response_router_data: ResponseRouterData<FiservcommercehubThreeDsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response: connector_response,
            mut router_data,
            http_code: http_status_code,
        } = response_router_data;
        let authentication_reference = FiservcommercehubAuthenticationReference::try_from((
            router_data.request.authentication_data.as_ref(),
            common_enums::AuthenticationStage::ChallengePending,
        ))
        .change_context(
            errors::ConnectorError::response_handling_failed_with_context(
                http_status_code,
                Some(
                    "PostAuthenticate request is missing its CommerceHub authentication reference."
                        .to_string(),
                ),
            ),
        )?;
        let authentication_outcome = connector_response.authentication_outcome(
            &authentication_reference,
            common_enums::AuthenticationStage::ChallengePending,
            http_status_code,
        )?;
        router_data.resource_common_data.status = authentication_outcome.attempt_status;
        router_data.response = Ok(PaymentsResponseData::PostAuthenticateResponse {
            authentication_data: Some(authentication_outcome.authentication_data),
            connector_response_reference_id: Some(
                authentication_reference.authentication_transaction_id,
            ),
            status_code: http_status_code,
        });
        Ok(router_data)
    }
}
