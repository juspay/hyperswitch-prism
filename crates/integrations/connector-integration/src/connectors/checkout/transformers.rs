use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::{CustomResult, ParsingError},
    request::Method,
    types::MinorUnit,
};
use domain_types::{
    connector_flow::{Authorize, Capture, PaymentMethodToken, RepeatPayment, SetupMandate, Void},
    connector_types::{
        MandateReference, MandateReferenceId, PartnerMerchantIdentifierDetails, PaymentFlowData,
        PaymentMethodTokenResponse, PaymentMethodTokenizationData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        RecipientAccount, RecipientBankAccount, RecipientDetails, RefundFlowData, RefundSyncData,
        RefundsData, RefundsResponseData, RepeatPaymentData, ResponseId, SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::{
        BankDebitData, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber, WalletData,
    },
    router_data::{
        AdditionalPaymentMethodConnectorResponse, ConnectorResponseData, ConnectorSpecificConfig,
        ErrorResponse,
    },
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
    utils,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_with::skip_serializing_none;
use url::Url;

use crate::{
    connectors::checkout::CheckoutRouterData,
    types::ResponseRouterData,
    utils::{
        construct_captures_response_hashmap, ErrorCodeAndMessage, MultipleCaptureSyncResponse,
    },
};

#[skip_serializing_none]
#[derive(Debug, Serialize)]
pub struct CheckoutAddress {
    pub address_line1: Option<Secret<String>>,
    pub address_line2: Option<Secret<String>>,
    pub city: Option<Secret<String>>,
    pub state: Option<Secret<String>>,
    pub zip: Option<Secret<String>>,
    pub country: Option<common_enums::CountryAlpha2>,
}

#[skip_serializing_none]
#[derive(Debug, Serialize)]
pub struct CheckoutAccountHolderDetails {
    pub first_name: Option<Secret<String>>,
    pub last_name: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
pub struct CardSource<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(rename = "type")]
    pub source_type: CheckoutSourceTypes,
    pub number: RawCardNumber<T>,
    pub expiry_month: Secret<String>,
    pub expiry_year: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cvv: Option<Secret<String>>,
    pub billing_address: Option<CheckoutAddress>,
    pub account_holder: Option<CheckoutAccountHolderDetails>,
}

#[derive(Debug, Serialize)]
pub struct WalletSource {
    #[serde(rename = "type")]
    pub source_type: CheckoutSourceTypes,
    pub token: Secret<String>,
    pub billing_address: Option<CheckoutAddress>,
}

/// Constants for ACH payment type
const ACH_PAYMENT_TYPE: &str = "ach";
const ACH_COUNTRY_US: &str = "US";
/// Source `type` Checkout expects for a wallet token that arrives already decrypted into a
/// network token.
const NETWORK_TOKEN_TYPE: &str = "network_token";
/// Documentation for the connector-decryption path, surfaced on the errors that ask for a token.
const CHECKOUT_TOKENS_DOC_URL: &str =
    "https://api-reference.checkout.com/tag/Tokens/#operation/requestAToken";
const APPLE_PAY_TOKEN_TYPE: &str = "applepay";
const GOOGLE_PAY_TOKEN_TYPE: &str = "googlepay";
/// Checkout.com ACH account holder type (mapped from common_enums::BankHolderType)
#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckoutAchHolderType {
    Individual,
    Corporate,
}

impl From<common_enums::BankHolderType> for CheckoutAchHolderType {
    fn from(holder_type: common_enums::BankHolderType) -> Self {
        match holder_type {
            common_enums::BankHolderType::Business => Self::Corporate,
            common_enums::BankHolderType::Personal => Self::Individual,
        }
    }
}

#[derive(Debug)]
pub struct CheckoutBankType(common_enums::BankType);

impl Serialize for CheckoutBankType {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl TryFrom<common_enums::BankType> for CheckoutBankType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(bank: common_enums::BankType) -> Result<Self, Self::Error> {
        match bank {
            common_enums::BankType::Salary | common_enums::BankType::Payment => {
                Err(error_stack::report!(IntegrationError::NotSupported {
                    message: format!("Bank type {bank:?} is not supported by Checkout"),
                    connector: "checkout",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Use a supported bank account type such as Checking or Savings"
                                .to_owned(),
                        ),
                        additional_context: None,
                        doc_url: None,
                    },
                }))
            }
            other => Ok(Self(other)),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct AchBankDebitSource {
    #[serde(rename = "type")]
    pub source_type: String,
    #[serde(rename = "account_type")]
    pub account_type: CheckoutBankType,
    pub country: String,
    pub account_number: Secret<String>,
    #[serde(rename = "bank_code")]
    pub routing_number: Secret<String>,
    pub account_holder: Option<AchAccountHolder>,
}

#[derive(Debug, Serialize)]
pub struct AchAccountHolder {
    #[serde(rename = "type")]
    pub holder_type: CheckoutAchHolderType,
    pub first_name: Option<Secret<String>>,
    pub last_name: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
pub struct MandateSource {
    #[serde(rename = "type")]
    pub source_type: CheckoutSourceTypes,
    #[serde(rename = "id")]
    pub source_id: Option<String>,
    pub billing_address: Option<CheckoutAddress>,
}

#[derive(Debug, Serialize)]
pub struct CheckoutRawCardDetails {
    #[serde(rename = "type")]
    pub source_type: CheckoutSourceTypes,
    pub number: cards::CardNumber,
    pub expiry_month: Secret<String>,
    pub expiry_year: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cvv: Option<Secret<String>>,
    pub billing_address: Option<CheckoutAddress>,
    pub account_holder: Option<CheckoutAccountHolderDetails>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum PaymentSource<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    Card(CardSource<T>),
    RawCardForNTI(CheckoutRawCardDetails),
    Wallets(WalletSource),
    ApplePayPredecrypt(Box<ApplePayPredecrypt>),
    MandatePayment(MandateSource),
    GooglePayPredecrypt(Box<GooglePayPredecrypt>),
    AchBankDebit(AchBankDebitSource),
    DecryptedWalletToken(DecryptedWalletToken),
}

#[derive(Debug, Serialize)]
pub struct DecryptedWalletToken {
    #[serde(rename = "type")]
    decrypt_type: String,
    token: cards::NetworkToken,
    token_type: String,
    expiry_month: Secret<String>,
    expiry_year: Secret<String>,
    pub billing_address: Option<CheckoutAddress>,
}

#[derive(Debug, Serialize)]
pub struct GooglePayPredecrypt {
    #[serde(rename = "type")]
    _type: String,
    token: cards::CardNumber,
    token_type: String,
    expiry_month: Secret<String>,
    expiry_year: Secret<String>,
    eci: String,
    cryptogram: Secret<String>,
    pub billing_address: Option<CheckoutAddress>,
}

#[derive(Debug, Serialize)]
pub struct ApplePayPredecrypt {
    token: cards::CardNumber,
    #[serde(rename = "type")]
    decrypt_type: String,
    token_type: String,
    expiry_month: Secret<String>,
    expiry_year: Secret<String>,
    eci: Option<String>,
    cryptogram: Secret<String>,
    pub billing_address: Option<CheckoutAddress>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckoutSourceTypes {
    Card,
    Token,
    NetworkToken,
    #[serde(rename = "id")]
    SourceId,
}

#[derive(Debug, Serialize)]
pub enum CheckoutPaymentType {
    Regular,
    Unscheduled,
    #[serde(rename = "MOTO")]
    Moto,
    Installment,
    Recurring,
}

/// Checkout credentials.
///
/// NOTE — the two key fields are the opposite way round from what the names suggest, and this
/// is deliberate. Checkout issues two keys per account, mapped onto the auth fields as:
///
/// * `api_key`    = the **public** key (`pk_...`). Used *only* by `POST /tokens`, the wallet
///   tokenization exchange. That endpoint rejects the secret key with `403`.
/// * `api_secret` = the **secret** key (`sk_...`). Used by every other endpoint (`/payments`,
///   captures, voids, refunds, syncs). `/payments` rejects the public key with `401`.
///
/// Do not "fix" this by swapping them or by introducing a separate public-key field: the
/// asymmetry is Checkout's, the field names are the connector auth type's, and the two are matched here on
/// purpose so a merchant's existing Checkout credentials work unchanged.
pub struct CheckoutAuthType {
    /// Public key (`pk_...`) — see the note on [`CheckoutAuthType`]. Tokenization only.
    pub api_key: Secret<String>,
    pub processing_channel_id: Secret<String>,
    /// Secret key (`sk_...`) — see the note on [`CheckoutAuthType`]. Everything except tokenization.
    pub api_secret: Secret<String>,
}

#[derive(Debug, Serialize)]
pub struct ReturnUrl {
    pub success_url: Option<String>,
    pub failure_url: Option<String>,
}

#[skip_serializing_none]
#[derive(Debug, Default, Serialize)]
pub struct CheckoutCustomer {
    pub name: Option<Secret<String>>,
    pub email: Option<common_utils::pii::Email>,
    pub phone: Option<CheckoutPhoneDetails>,
    pub tax_number: Option<Secret<String>>,
}

#[skip_serializing_none]
#[derive(Debug, Default, Serialize)]
pub struct CheckoutPhoneDetails {
    pub country_code: Option<String>,
    pub number: Option<Secret<String>>,
}

#[skip_serializing_none]
#[derive(Debug, Default, Serialize)]
pub struct CheckoutProcessing {
    /// Marks the payment as an Account Funding Transaction.
    pub aft: Option<bool>,
    pub order_id: Option<String>,
    pub tax_amount: Option<MinorUnit>,
    pub discount_amount: Option<MinorUnit>,
    pub duty_amount: Option<MinorUnit>,
    pub shipping_amount: Option<MinorUnit>,
    pub shipping_tax_amount: Option<MinorUnit>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckoutSenderType {
    Individual,
}

#[skip_serializing_none]
#[derive(Debug, Serialize)]
pub struct CheckoutSender {
    #[serde(rename = "type")]
    pub sender_type: CheckoutSenderType,
    pub first_name: Secret<String>,
    pub last_name: Secret<String>,
    pub address: CheckoutAddress,
    pub date_of_birth: Secret<time::Date>,
}

#[skip_serializing_none]
#[derive(Debug, Serialize)]
pub struct CheckoutInstruction {
    pub purpose: String,
}

#[skip_serializing_none]
#[derive(Debug, Serialize)]
pub struct CheckoutRecipient {
    pub first_name: Secret<String>,
    pub last_name: Secret<String>,
    pub account_number: Secret<String>,
    pub address: CheckoutAddress,
}

fn get_checkout_recipient_account_number(
    account: RecipientAccount,
) -> Result<Secret<String>, error_stack::Report<IntegrationError>> {
    let unsupported = |identifier: &str| {
        error_stack::report!(IntegrationError::NotSupported {
            message: format!("{identifier} as a recipient account identifier"),
            connector: "checkout",
            context: IntegrationErrorContext::default(),
        })
    };

    match account {
        RecipientAccount::BankAccount(bank_account) => {
            match bank_account {
                RecipientBankAccount::Iban { iban } => Ok(iban.clone()),
                RecipientBankAccount::RoutingNumber { .. } => {
                    Err(unsupported("a bank account number with a routing number"))
                }
                RecipientBankAccount::Bic { .. } => {
                    Err(unsupported("a bank account number with a BIC"))
                }
                RecipientBankAccount::AccountNumber { .. } => {
                    Err(unsupported("a bare bank account number"))
                }
                // Checkout documents the first six and last four digits of the PAN as one of the
                // accepted account number forms.
                RecipientBankAccount::TruncatedPan { card_isin, last4 } => Ok(Secret::new(
                    format!("{}{}", card_isin.expose(), last4.expose()),
                )),
            }
        }
        RecipientAccount::Card { card_number } => Ok(Secret::new(card_number.get_card_no())),
        RecipientAccount::Phone { phone_number } => Ok(phone_number.clone()),
        RecipientAccount::Wallet { .. } => Err(unsupported("wallet_id")),
        RecipientAccount::Email { .. } => Err(unsupported("email")),
        RecipientAccount::SocialNetwork { .. } => Err(unsupported("social_network_id")),
    }
}

fn build_checkout_recipient(
    recipient_details: Option<&RecipientDetails>,
) -> Result<CheckoutRecipient, error_stack::Report<IntegrationError>> {
    let recipient_details =
        recipient_details.ok_or_else(utils::missing_field_err("recipient_details"))?;

    let address = recipient_details
        .address
        .as_ref()
        .ok_or_else(utils::missing_field_err("recipient_details.address"))?;

    let account_number = recipient_details
        .account
        .as_ref()
        .ok_or_else(utils::missing_field_err("recipient_details.account"))
        .and_then(|account| get_checkout_recipient_account_number(account.clone()))?;

    Ok(CheckoutRecipient {
        first_name: address
            .first_name
            .clone()
            .ok_or_else(utils::missing_field_err(
                "recipient_details.address.first_name",
            ))?,
        last_name: address
            .last_name
            .clone()
            .ok_or_else(utils::missing_field_err(
                "recipient_details.address.last_name",
            ))?,
        account_number,
        address: CheckoutAddress {
            address_line1: Some(
                address
                    .line1
                    .clone()
                    .ok_or_else(utils::missing_field_err("recipient_details.address.line1"))?,
            ),
            address_line2: address.line2.clone(),
            city: Some(
                address
                    .city
                    .clone()
                    .ok_or_else(utils::missing_field_err("recipient_details.address.city"))?,
            ),
            state: Some(
                address
                    .state
                    .clone()
                    .ok_or_else(utils::missing_field_err("recipient_details.address.state"))?,
            ),
            zip: Some(
                address
                    .zip
                    .clone()
                    .ok_or_else(utils::missing_field_err("recipient_details.address.zip"))?,
            ),
            country: Some(address.country.ok_or_else(utils::missing_field_err(
                "recipient_details.address.country",
            ))?),
        },
    })
}

fn build_checkout_sender(
    resource_common_data: &PaymentFlowData,
    date_of_birth: Secret<time::Date>,
) -> Result<CheckoutSender, error_stack::Report<IntegrationError>> {
    Ok(CheckoutSender {
        sender_type: CheckoutSenderType::Individual,
        first_name: resource_common_data.get_billing_first_name()?,
        last_name: resource_common_data.get_billing_last_name()?,
        date_of_birth,
        address: CheckoutAddress {
            address_line1: Some(resource_common_data.get_billing_line1()?),
            address_line2: resource_common_data.get_optional_billing_line2(),
            city: Some(resource_common_data.get_billing_city()?),
            state: Some(resource_common_data.get_billing_state()?),
            zip: Some(resource_common_data.get_billing_zip()?),
            country: Some(resource_common_data.get_billing_country()?),
        },
    })
}

#[skip_serializing_none]
#[derive(Debug, Default, Serialize)]
pub struct CheckoutShipping {
    pub address: Option<CheckoutAddress>,
    pub from_address_zip: Option<String>,
}

#[skip_serializing_none]
#[derive(Debug, Default, Serialize)]
pub struct CheckoutLineItem {
    pub commodity_code: Option<String>,
    pub discount_amount: Option<MinorUnit>,
    pub name: Option<String>,
    pub quantity: Option<u16>,
    pub reference: Option<String>,
    pub tax_exempt: Option<bool>,
    pub tax_amount: Option<MinorUnit>,
    pub total_amount: Option<MinorUnit>,
    pub unit_of_measure: Option<String>,
    pub unit_price: Option<MinorUnit>,
}

#[skip_serializing_none]
#[derive(Debug, Default, Serialize)]
pub struct CheckoutBillingDescriptor {
    pub name: Option<Secret<String>>,
    pub city: Option<Secret<String>>,
    pub reference: Option<String>,
}

#[skip_serializing_none]
#[derive(Debug, Serialize)]
pub struct PaymentsRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub source: PaymentSource<T>,
    pub amount: MinorUnit,
    pub currency: String,
    pub processing_channel_id: Secret<String>,
    #[serde(rename = "3ds")]
    pub three_ds: CheckoutThreeDS,
    #[serde(flatten)]
    pub return_url: ReturnUrl,
    pub capture: bool,
    pub reference: String,
    #[serde(skip_serializing_if = "is_metadata_empty")]
    pub metadata: Option<Secret<serde_json::Value>>,
    pub payment_type: CheckoutPaymentType,
    pub merchant_initiated: Option<bool>,
    pub previous_payment_id: Option<String>,
    pub store_for_future_use: Option<bool>,
    pub billing_descriptor: Option<CheckoutBillingDescriptor>,
    // Level 2/3 data fields
    pub customer: Option<CheckoutCustomer>,
    pub processing: Option<CheckoutProcessing>,
    pub shipping: Option<CheckoutShipping>,
    pub items: Option<Vec<CheckoutLineItem>>,
    pub partial_authorization: Option<CheckoutPartialAuthorization>,
    pub payment_ip: Option<Secret<String, common_utils::pii::IpAddress>>,
    pub recipient: Option<CheckoutRecipient>,
    pub sender: Option<CheckoutSender>,
    pub instruction: Option<CheckoutInstruction>,
}

#[skip_serializing_none]
#[derive(Debug, Default, Serialize)]
pub struct CheckoutPartialAuthorization {
    pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CheckoutMeta {
    pub psync_flow: CheckoutPaymentIntent,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum CheckoutPaymentIntent {
    Capture,
    Authorize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CheckoutChallengeIndicator {
    NoPreference,
    ChallengeRequestedMandate,
    ChallengeRequested,
    NoChallengeRequested,
}

#[derive(Debug, Serialize)]
pub struct CheckoutThreeDS {
    enabled: bool,
    force_3ds: bool,
    eci: Option<String>,
    cryptogram: Option<Secret<String>>,
    xid: Option<String>,
    version: Option<String>,
    challenge_indicator: CheckoutChallengeIndicator,
}

impl TryFrom<&ConnectorSpecificConfig> for CheckoutAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        if let ConnectorSpecificConfig::Checkout {
            api_key,
            api_secret,
            processing_channel_id,
            ..
        } = auth_type
        {
            Ok(Self {
                api_key: api_key.to_owned(),
                api_secret: api_secret.to_owned(),
                processing_channel_id: processing_channel_id.to_owned(),
            })
        } else {
            Err(IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Configure this merchant account's Checkout connector with api_key, \
                         api_secret and processing_channel_id."
                            .to_owned(),
                    ),
                    additional_context: Some(
                        "Failed to obtain CheckoutAuthType from ConnectorSpecificConfig".to_owned(),
                    ),
                    doc_url: None,
                },
            }
            .into())
        }
    }
}

fn split_account_holder_name(
    card_holder_name: Option<Secret<String>>,
) -> (Option<Secret<String>>, Option<Secret<String>>) {
    let account_holder_name = card_holder_name
        .as_ref()
        .map(|name| name.clone().expose().trim().to_string());
    match account_holder_name {
        Some(name) if !name.is_empty() => match name.rsplit_once(' ') {
            Some((first, last)) => (
                Some(Secret::new(first.to_string())),
                Some(Secret::new(last.to_string())),
            ),
            None => (Some(Secret::new(name)), None),
        },
        _ => (None, None),
    }
}

/// Error for a wallet payload that reaches Authorize while still encrypted.
///
/// Checkout decrypts wallet payloads at its end, but only behind a separate `POST /tokens` call
/// that is authenticated with the account's *public* key and yields a single-use `tok_...`. A
/// `POST /payments` request cannot carry the raw wallet payload, so the exchange has to happen
/// before Authorize is invoked; the resulting token is then handed back on
/// `payment_method.token` and consumed as `source.type = "token"`.
///
/// UCS now performs that exchange itself — see the `PaymentMethodToken` flow
/// ([`CheckoutTokenRequest`]) exposed as `PaymentMethodService/Tokenize`. So this is no longer
/// "Checkout cannot do this"; it is "this payload is at the wrong step of a two-call sequence".
/// The arm is deliberately kept rather than tokenizing inline from Authorize: Checkout's tokens
/// are single-use and expire 15 minutes after issue, so minting one inside Authorize would hide a
/// second network call (with different credentials, and its own failure modes) behind a flow the
/// caller believes is one request, and would silently double-charge nothing but double-spend the
/// token on any Authorize retry. Keeping the two calls explicit also lets the caller reuse a token
/// across Authorize and SetupMandate, which is what the tail of this path already supports.
fn encrypted_wallet_needs_token(wallet_name: &str) -> error_stack::Report<IntegrationError> {
    error_stack::report!(IntegrationError::NotSupported {
        message: format!("{wallet_name} payload that is still encrypted"),
        connector: "checkout",
        context: IntegrationErrorContext {
            suggested_action: Some(
                "Call PaymentMethodService/Tokenize on this connector first — it performs \
                 Checkout's POST /tokens exchange (authenticated with the account public key) — \
                 then send the returned `tok_...` on `payment_method.token`, or supply the wallet \
                 already decrypted as a network token"
                    .to_owned(),
            ),
            doc_url: Some(CHECKOUT_TOKENS_DOC_URL.to_owned()),
            additional_context: None,
        },
    })
}

/// Builds the Checkout wallet source for a Checkout-issued reference token (`tok_...`).
///
/// This is the tail of the connector-decryption path: the wallet payload was handed to Checkout's
/// `POST /tokens` endpoint, Checkout decrypted it and returned a single-use token, and the payment
/// itself just references that token, i.e. `PaymentMethodToken::Token` ->
/// `PaymentSource::Wallets { source_type: Token }` mapping.
impl From<(Secret<String>, Option<CheckoutAddress>)> for WalletSource {
    fn from((token, billing_address): (Secret<String>, Option<CheckoutAddress>)) -> Self {
        Self {
            source_type: CheckoutSourceTypes::Token,
            token,
            billing_address,
        }
    }
}

/// Builds the Checkout payment source for a wallet whose token has already been decrypted.
///
/// Checkout accepts a decrypted wallet in the network-token shape (PAN + cryptogram). A wallet that
/// is still encrypted has to go through Checkout's `POST /tokens` exchange first — see
/// [`encrypted_wallet_needs_token`] and [`WalletSource::from`]. Shared by the Authorize and
/// SetupMandate flows so a zero-amount mandate setup accepts the same wallets as a regular payment.
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(&WalletData, Option<CheckoutAddress>)> for PaymentSource<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        (wallet_data, billing_address): (&WalletData, Option<CheckoutAddress>),
    ) -> Result<Self, Self::Error> {
        match wallet_data {
            WalletData::GooglePay(google_pay_data) => match &google_pay_data.tokenization_data {
                domain_types::payment_method_data::GpayTokenizationData::Decrypted(
                    google_pay_decrypted_data,
                ) => {
                    let expiry_month = google_pay_decrypted_data
                        .get_expiry_month()
                        .change_context(IntegrationError::InvalidDataFormat {
                        field_name: "google_pay_decrypted_data.card_exp_month",
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "Checkout's network-token source needs a two-digit expiry month \
                                     from the decrypted Google Pay token."
                                    .to_owned(),
                            ),
                            ..Default::default()
                        },
                    })?;

                    let expiry_year = google_pay_decrypted_data
                        .get_four_digit_expiry_year()
                        .change_context(IntegrationError::InvalidDataFormat {
                        field_name: "google_pay_decrypted_data.card_exp_year",
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "Checkout's network-token source needs a four-digit expiry year \
                                     from the decrypted Google Pay token."
                                    .to_owned(),
                            ),
                            ..Default::default()
                        },
                    })?;

                    // A PAN_ONLY token decrypts to a plain FPAN with neither(eci and cryptogram), so it has to be
                    // sent as an ordinary card source instead — Checkout rejects a network token
                    // that arrives without its cryptogram.
                    match (
                        google_pay_decrypted_data.cryptogram.clone(),
                        google_pay_decrypted_data.eci_indicator.clone(),
                    ) {
                        (Some(cryptogram), Some(eci)) => {
                            Ok(Self::GooglePayPredecrypt(Box::new(GooglePayPredecrypt {
                                _type: NETWORK_TOKEN_TYPE.to_string(),
                                token: google_pay_decrypted_data
                                    .application_primary_account_number
                                    .clone(),
                                token_type: GOOGLE_PAY_TOKEN_TYPE.to_string(),
                                expiry_month,
                                expiry_year,
                                eci,
                                cryptogram,
                                billing_address,
                            })))
                        }
                        _ => Ok(Self::RawCardForNTI(CheckoutRawCardDetails {
                            source_type: CheckoutSourceTypes::Card,
                            number: google_pay_decrypted_data
                                .application_primary_account_number
                                .clone(),
                            expiry_month,
                            expiry_year,
                            cvv: None,
                            billing_address,
                            account_holder: None,
                        })),
                    }
                }
                domain_types::payment_method_data::GpayTokenizationData::Encrypted(_) => {
                    Err(encrypted_wallet_needs_token("Google Pay"))
                }
            },
            WalletData::ApplePay(apple_pay_data) => match apple_pay_data
                .payment_data
                .get_decrypted_apple_pay_payment_data_optional()
            {
                Some(apple_pay_decrypt_data) => {
                    Ok(Self::ApplePayPredecrypt(Box::new(ApplePayPredecrypt {
                        token: apple_pay_decrypt_data
                            .application_primary_account_number
                            .clone(),
                        decrypt_type: NETWORK_TOKEN_TYPE.to_string(),
                        token_type: APPLE_PAY_TOKEN_TYPE.to_string(),
                        expiry_month: apple_pay_decrypt_data.get_expiry_month(),
                        expiry_year: apple_pay_decrypt_data.get_four_digit_expiry_year(),
                        eci: apple_pay_decrypt_data.payment_data.eci_indicator.clone(),
                        cryptogram: apple_pay_decrypt_data
                            .payment_data
                            .online_payment_cryptogram
                            .clone(),
                        billing_address,
                    })))
                }
                None => Err(encrypted_wallet_needs_token("Apple Pay")),
            },
            _ => Err(IntegrationError::NotImplemented(
                utils::get_unimplemented_payment_method_error_message("checkout"),
                IntegrationErrorContext {
                    additional_context: Some(
                        "Checkout only accepts Google Pay and Apple Pay wallets as a decrypted \
                     network token"
                            .to_owned(),
                    ),
                    ..Default::default()
                },
            )
            .into()),
        }
    }
}

/// Request body for Checkout's `POST /tokens` — the connector-decryption head.
///
/// Checkout wants the *raw* wallet payload exactly as the wallet SDK produced it, wrapped in a
/// discriminator: `{"type": "applepay" | "googlepay", "token_data": { .. }}`. The response is a
/// single-use `tok_...` that `POST /payments` then consumes as `source.type = "token"` (see
/// [`WalletSource::from`]).
#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
#[serde(tag = "type", content = "token_data")]
pub enum CheckoutTokenRequest {
    Googlepay(CheckoutGooglePayData),
    Applepay(Box<CheckoutApplePayData>),
}

/// Google Pay `PaymentData.paymentMethodData.tokenizationData.token`, parsed out of the opaque
/// JSON string the Google Pay SDK hands over.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutGooglePayData {
    protocol_version: Secret<String>,
    signature: Secret<String>,
    signed_message: Secret<String>,
}

/// Apple Pay `PKPaymentToken.paymentData`, parsed out of the base64 blob the Apple Pay SDK
/// hands over.
#[derive(Debug, Serialize, Deserialize)]
pub struct CheckoutApplePayData {
    version: Secret<String>,
    data: Secret<String>,
    signature: Secret<String>,
    header: CheckoutApplePayHeader,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutApplePayHeader {
    ephemeral_public_key: Secret<String>,
    public_key_hash: Secret<String>,
    transaction_id: Secret<String>,
}

/// Response of `POST /tokens`. Checkout echoes the request `type` and expiry alongside the token;
/// only the token is load-bearing for the payment that follows.
#[derive(Debug, Deserialize, Serialize)]
pub struct CheckoutTokenResponse {
    token: Secret<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        CheckoutRouterData<
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
            T,
        >,
    > for CheckoutTokenRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: CheckoutRouterData<
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        match &item.router_data.request.payment_method_data {
            PaymentMethodData::Wallet(wallet_data) => match wallet_data {
                WalletData::GooglePay(_) => Ok(Self::Googlepay(
                    wallet_data.get_wallet_token_as_json("Google Pay".to_string())?,
                )),
                WalletData::ApplePay(_) => Ok(Self::Applepay(Box::new(
                    wallet_data.get_wallet_token_as_json("Apple Pay".to_string())?,
                ))),
                _ => Err(IntegrationError::NotImplemented(
                    utils::get_unimplemented_payment_method_error_message("checkout"),
                    IntegrationErrorContext {
                        additional_context: Some(
                            "Checkout's POST /tokens exchange only accepts Google Pay and Apple \
                         Pay payloads"
                                .to_owned(),
                        ),
                        ..Default::default()
                    },
                )
                .into()),
            },
            _ => Err(IntegrationError::NotImplemented(
                utils::get_unimplemented_payment_method_error_message("checkout"),
                IntegrationErrorContext {
                    additional_context: Some(
                        "Checkout's Tokenize flow only accepts wallet payment method data"
                            .to_owned(),
                    ),
                    ..Default::default()
                },
            )
            .into()),
        }
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<CheckoutTokenResponse, Self>>
    for RouterDataV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<CheckoutTokenResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(PaymentMethodTokenResponse {
                token: item.response.token.expose(),
                // Checkout's `tok_...` is single-use and expires after 15 minutes, so it is
                // not a durable payment method identifier.
                connector_payment_method_id: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

fn build_metadata(
    metadata: Option<Secret<serde_json::Value>>,
    partner_merchant_identifier_details: Option<&PartnerMerchantIdentifierDetails>,
) -> Option<Secret<serde_json::Value>> {
    let udf5 = partner_merchant_identifier_details
        .and_then(|details| details.partner_details.as_ref())
        .and_then(|details| details.name.clone().or_else(|| details.integrator.clone()));

    match (metadata, udf5) {
        (None, None) => None,
        (metadata, udf5) => {
            let mut metadata_json = metadata
                .map(ExposeInterface::expose)
                .unwrap_or_else(|| json!({}));

            if let Some(value) = udf5 {
                if let Some(obj) = metadata_json.as_object_mut() {
                    obj.insert("udf5".to_string(), json!(value));
                } else {
                    metadata_json = json!({ "udf5": value });
                }
            }

            Some(Secret::new(metadata_json))
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        CheckoutRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: CheckoutRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let capture = matches!(
            item.router_data.request.capture_method,
            Some(common_enums::CaptureMethod::Automatic)
        );

        let payment_type = if matches!(
            item.router_data.request.payment_channel,
            Some(
                common_enums::PaymentChannel::MailOrder
                    | common_enums::PaymentChannel::TelephoneOrder
            )
        ) {
            CheckoutPaymentType::Moto
        } else if item.router_data.request.is_mandate_payment() {
            CheckoutPaymentType::Unscheduled
        } else {
            CheckoutPaymentType::Regular
        };

        let (challenge_indicator, store_for_future_use) =
            if item.router_data.request.is_mandate_payment() {
                (
                    CheckoutChallengeIndicator::ChallengeRequestedMandate,
                    Some(true),
                )
            } else {
                (CheckoutChallengeIndicator::ChallengeRequested, None)
            };

        let billing_details = Some(CheckoutAddress {
            city: item
                .router_data
                .resource_common_data
                .get_optional_billing_city(),
            address_line1: item
                .router_data
                .resource_common_data
                .get_optional_billing_line1(),
            address_line2: item
                .router_data
                .resource_common_data
                .get_optional_billing_line2(),
            state: item
                .router_data
                .resource_common_data
                .get_optional_billing_state(),
            zip: item
                .router_data
                .resource_common_data
                .get_optional_billing_zip(),
            country: item
                .router_data
                .resource_common_data
                .get_optional_billing_country(),
        });

        let (source_var, previous_payment_id, merchant_initiated, store_for_future_use) =
            match item.router_data.request.payment_method_data.clone() {
                PaymentMethodData::Card(ccard) => {
                    let (first_name, last_name) = split_account_holder_name(ccard.card_holder_name);

                    let payment_source = PaymentSource::Card(CardSource {
                        source_type: CheckoutSourceTypes::Card,
                        number: ccard.card_number.clone(),
                        expiry_month: ccard.card_exp_month.clone(),
                        expiry_year: ccard.card_exp_year.clone(),
                        cvv: Some(ccard.card_cvc),
                        billing_address: billing_details,
                        account_holder: Some(CheckoutAccountHolderDetails {
                            first_name,
                            last_name,
                        }),
                    });
                    Ok((payment_source, None, Some(false), store_for_future_use))
                }
                PaymentMethodData::Wallet(wallet_data) => {
                    let p_source = PaymentSource::try_from((&wallet_data, billing_details))?;
                    Ok((p_source, None, Some(false), store_for_future_use))
                }
                // Connector-decryption path: the wallet payload was already exchanged for a
                // Checkout token (`tok_...`) via `POST /tokens`, so the payment only references
                // it. The token is single-use and expires 15 minutes after it was issued.
                PaymentMethodData::PaymentMethodToken(token_data) => {
                    let payment_source =
                        PaymentSource::Wallets((token_data.token, billing_details).into());
                    Ok((payment_source, None, Some(false), store_for_future_use))
                }
                PaymentMethodData::BankDebit(BankDebitData::AchBankDebit {
                    account_number,
                    routing_number,
                    bank_account_holder_name,
                    card_holder_name,
                    bank_holder_type,
                    bank_type,
                    ..
                }) => {
                    // Get account holder name from bank_account_holder_name, card_holder_name, or billing details
                    let holder_name = bank_account_holder_name.or(card_holder_name).or_else(|| {
                        item.router_data
                            .resource_common_data
                            .get_billing_full_name()
                            .ok()
                    });

                    // Map bank_holder_type to Checkout's expected format
                    let holder_type: CheckoutAchHolderType = bank_holder_type
                        .map(Into::into)
                        .unwrap_or(CheckoutAchHolderType::Individual);

                    // Only include account_holder when a name is available to avoid
                    // sending null first_name/last_name which causes ACH validation errors
                    let account_holder = match holder_name {
                        Some(name) => {
                            let (first_name, last_name) = split_account_holder_name(Some(name));
                            Some(AchAccountHolder {
                                holder_type,
                                first_name,
                                last_name,
                            })
                        }
                        None => None,
                    };

                    let account_type = CheckoutBankType::try_from(
                        bank_type.unwrap_or(common_enums::BankType::Savings),
                    )?;

                    let payment_source = PaymentSource::AchBankDebit(AchBankDebitSource {
                        source_type: ACH_PAYMENT_TYPE.to_string(),
                        account_type,
                        country: ACH_COUNTRY_US.to_string(),
                        account_number: account_number.clone(),
                        routing_number: routing_number.clone(),
                        account_holder,
                    });
                    // For ACH bank debit, we typically want to store for future use if it's a mandate payment
                    let store_for_future = if item.router_data.request.is_mandate_payment() {
                        Some(true)
                    } else {
                        store_for_future_use
                    };
                    Ok((payment_source, None, Some(false), store_for_future))
                }
                _ => Err(IntegrationError::NotImplemented(
                    utils::get_unimplemented_payment_method_error_message("checkout"),
                    IntegrationErrorContext {
                        additional_context: Some(
                            "Checkout Authorize supports cards, Google Pay / Apple Pay, Checkout \
                         tokens and ACH bank debit"
                                .to_owned(),
                        ),
                        ..Default::default()
                    },
                )),
            }?;

        let authentication_data = item.router_data.request.authentication_data.as_ref();

        let three_ds = match item.router_data.resource_common_data.auth_type {
            common_enums::AuthenticationType::ThreeDs => CheckoutThreeDS {
                enabled: true,
                force_3ds: true,
                eci: authentication_data.and_then(|auth| auth.eci.clone()),
                cryptogram: authentication_data.and_then(|auth| auth.cavv.clone()),
                xid: authentication_data
                    .and_then(|auth| auth.threeds_server_transaction_id.clone()),
                version: authentication_data.and_then(|auth| {
                    auth.message_version
                        .clone()
                        .map(|version| version.to_string())
                }),
                challenge_indicator,
            },
            common_enums::AuthenticationType::NoThreeDs => CheckoutThreeDS {
                enabled: false,
                force_3ds: false,
                eci: None,
                cryptogram: None,
                xid: None,
                version: None,
                challenge_indicator: CheckoutChallengeIndicator::NoPreference,
            },
        };

        let return_url = ReturnUrl {
            success_url: item
                .router_data
                .request
                .router_return_url
                .as_ref()
                .map(|return_url| format!("{return_url}?status=success")),
            failure_url: item
                .router_data
                .request
                .router_return_url
                .as_ref()
                .map(|return_url| format!("{return_url}?status=failure")),
        };

        let connector_auth = &item.router_data.connector_config;
        let auth_type: CheckoutAuthType = connector_auth.try_into()?;
        let processing_channel_id = auth_type.processing_channel_id;
        let metadata = build_metadata(
            item.router_data.request.metadata.clone(),
            item.router_data
                .request
                .partner_merchant_identifier_details
                .as_ref(),
        );

        let (customer, mut processing, shipping, items) = if let Some(l2l3_data) =
            &item.router_data.resource_common_data.l2_l3_data
        {
            (
                l2l3_data.customer_info.as_ref().map(|_| CheckoutCustomer {
                    name: l2l3_data.get_customer_name(),
                    email: l2l3_data.get_customer_email(),
                    phone: Some(CheckoutPhoneDetails {
                        country_code: l2l3_data.get_customer_phone_country_code(),
                        number: l2l3_data.get_customer_phone_number(),
                    }),
                    tax_number: l2l3_data.get_customer_tax_registration_id(),
                }),
                l2l3_data.order_info.as_ref().map(|_| CheckoutProcessing {
                    order_id: l2l3_data.get_merchant_order_reference_id(),
                    tax_amount: l2l3_data.get_order_tax_amount(),
                    discount_amount: l2l3_data.get_discount_amount(),
                    duty_amount: l2l3_data.get_duty_amount(),
                    shipping_amount: l2l3_data.get_shipping_cost(),
                    shipping_tax_amount: l2l3_data.get_shipping_amount_tax(),
                    aft: None,
                }),
                Some(CheckoutShipping {
                    address: Some(CheckoutAddress {
                        country: l2l3_data.get_shipping_country(),
                        address_line1: l2l3_data.get_shipping_address_line1(),
                        address_line2: l2l3_data.get_shipping_address_line2(),
                        city: l2l3_data.get_shipping_city(),
                        state: l2l3_data.get_shipping_state(),
                        zip: l2l3_data.get_shipping_zip(),
                    }),
                    from_address_zip: l2l3_data.get_shipping_origin_zip().map(|zip| zip.expose()),
                }),
                l2l3_data.get_order_details().map(|details| {
                    details
                        .iter()
                        .map(|item| CheckoutLineItem {
                            commodity_code: item.commodity_code.clone(),
                            discount_amount: item.unit_discount_amount,
                            name: Some(item.product_name.clone()),
                            quantity: Some(item.quantity),
                            reference: item.product_id.clone(),
                            tax_exempt: None,
                            tax_amount: item.total_tax_amount,
                            total_amount: item.total_amount,
                            unit_of_measure: item.unit_of_measure.clone(),
                            unit_price: Some(item.amount),
                        })
                        .collect::<Vec<_>>()
                }),
            )
        } else {
            (None, None, None, None)
        };

        let is_account_funding_transaction = item
            .router_data
            .request
            .is_account_funding_transaction
            .unwrap_or(false);

        let (recipient, sender, instruction) = if is_account_funding_transaction {
            processing
                .get_or_insert_with(CheckoutProcessing::default)
                .aft = Some(true);

            let purpose = item
                .router_data
                .request
                .additional_connector_details
                .as_ref()
                .and_then(|details| details.checkout.as_ref())
                .and_then(|checkout| checkout.purpose_of_payment.clone())
                .ok_or_else(utils::missing_field_err(
                    "additional_connector_details.checkout.purpose_of_payment",
                ))?;

            let sender_date_of_birth = item
                .router_data
                .request
                .customer
                .as_ref()
                .and_then(|customer| customer.date_of_birth.clone())
                .ok_or_else(utils::missing_field_err("customer.date_of_birth"))?;

            (
                Some(build_checkout_recipient(
                    item.router_data.request.recipient_details.as_ref(),
                )?),
                Some(build_checkout_sender(
                    &item.router_data.resource_common_data,
                    sender_date_of_birth,
                )?),
                Some(CheckoutInstruction { purpose }),
            )
        } else {
            (None, None, None)
        };

        let partial_authorization = item.router_data.request.enable_partial_authorization.map(
            |enable_partial_authorization| CheckoutPartialAuthorization {
                enabled: enable_partial_authorization,
            },
        );

        let payment_ip = item.router_data.request.get_ip_address_as_optional();

        let billing_descriptor =
            item.router_data
                .request
                .billing_descriptor
                .as_ref()
                .map(|descriptor| CheckoutBillingDescriptor {
                    name: descriptor.name.clone(),
                    city: descriptor.city.clone(),
                    reference: descriptor.reference.clone(),
                });

        let request = Self {
            source: source_var,
            amount: item.router_data.request.minor_amount,
            currency: item.router_data.request.currency.to_string(),
            processing_channel_id,
            three_ds,
            return_url,
            capture,
            reference: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            metadata,
            payment_type,
            merchant_initiated,
            previous_payment_id,
            store_for_future_use,
            partial_authorization,
            customer,
            processing,
            shipping,
            items,
            payment_ip,
            billing_descriptor,
            recipient,
            sender,
            instruction,
        };

        Ok(request)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        CheckoutRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: CheckoutRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let capture = matches!(
            item.router_data.request.capture_method,
            Some(common_enums::CaptureMethod::Automatic)
        );

        let billing_details = Some(CheckoutAddress {
            city: item
                .router_data
                .resource_common_data
                .get_optional_billing_city(),
            address_line1: item
                .router_data
                .resource_common_data
                .get_optional_billing_line1(),
            address_line2: item
                .router_data
                .resource_common_data
                .get_optional_billing_line2(),
            state: item
                .router_data
                .resource_common_data
                .get_optional_billing_state(),
            zip: item
                .router_data
                .resource_common_data
                .get_optional_billing_zip(),
            country: item
                .router_data
                .resource_common_data
                .get_optional_billing_country(),
        });

        let (
            source_var,
            previous_payment_id,
            merchant_initiated,
            payment_type,
            store_for_future_use,
        ) = match &item.router_data.request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(mandate_data) => {
                let mandate_source = PaymentSource::MandatePayment(MandateSource {
                    source_type: CheckoutSourceTypes::SourceId,
                    source_id: mandate_data.get_connector_mandate_id(),
                    billing_address: billing_details,
                });
                let previous_id = mandate_data.get_connector_mandate_request_reference_id();
                let p_type = match item.router_data.request.mit_category {
                    Some(common_enums::MitCategory::Installment) => {
                        CheckoutPaymentType::Installment
                    }
                    Some(common_enums::MitCategory::Recurring) => CheckoutPaymentType::Recurring,
                    Some(common_enums::MitCategory::Unscheduled) | None => {
                        CheckoutPaymentType::Unscheduled
                    }
                    _ => CheckoutPaymentType::Unscheduled,
                };
                Ok((mandate_source, previous_id, Some(true), p_type, None))
            }
            MandateReferenceId::NetworkMandateId(network_transaction_id) => {
                match item.router_data.request.payment_method_data {
                    PaymentMethodData::CardDetailsForNetworkTransactionId(ref card_details) => {
                        let (first_name, last_name) =
                            split_account_holder_name(card_details.card_holder_name.clone());

                        let payment_source = PaymentSource::RawCardForNTI(CheckoutRawCardDetails {
                            source_type: CheckoutSourceTypes::Card,
                            number: card_details.card_number.clone(),
                            expiry_month: card_details.card_exp_month.clone(),
                            expiry_year: card_details.card_exp_year.clone(),
                            cvv: None,
                            billing_address: billing_details,
                            account_holder: Some(CheckoutAccountHolderDetails {
                                first_name,
                                last_name,
                            }),
                        });
                        let p_type = match item.router_data.request.mit_category {
                            Some(common_enums::MitCategory::Installment) => {
                                CheckoutPaymentType::Installment
                            }
                            Some(common_enums::MitCategory::Recurring) => {
                                CheckoutPaymentType::Recurring
                            }
                            Some(common_enums::MitCategory::Unscheduled) | None => {
                                CheckoutPaymentType::Unscheduled
                            }
                            _ => CheckoutPaymentType::Unscheduled,
                        };
                        Ok((
                            payment_source,
                            Some(network_transaction_id.network_transaction_id.clone()),
                            Some(true),
                            p_type,
                            None,
                        ))
                    }
                    PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(
                        ref network_token_data,
                    ) => {
                        let p_type = match item.router_data.request.mit_category {
                            Some(common_enums::MitCategory::Installment) => {
                                CheckoutPaymentType::Installment
                            }
                            Some(common_enums::MitCategory::Recurring) => {
                                CheckoutPaymentType::Recurring
                            }
                            Some(common_enums::MitCategory::Unscheduled) | None => {
                                CheckoutPaymentType::Unscheduled
                            }
                            _ => CheckoutPaymentType::Unscheduled,
                        };

                        let token_type = match network_token_data.token_source {
                            Some(domain_types::payment_method_data::TokenSource::ApplePay) => {
                                "applepay".to_string()
                            }
                            Some(domain_types::payment_method_data::TokenSource::GooglePay) => {
                                "googlepay".to_string()
                            }
                            None => Err(IntegrationError::MissingRequiredField {
                                field_name: "token_source",
                                context: IntegrationErrorContext {
                                    additional_context: Some("Checkout needs the wallet a network token was issued for \
                                 (applepay or googlepay) to replay it against a network \
                                 transaction id".to_owned()),
                                    ..Default::default()
                                },
                            })?,
                        };

                        let exp_month = network_token_data.token_exp_month.clone();
                        let expiry_year_4_digit = network_token_data.get_expiry_year_4_digit();

                        let payment_source =
                            PaymentSource::DecryptedWalletToken(DecryptedWalletToken {
                                token: network_token_data.decrypted_token.clone(),
                                decrypt_type: "network_token".to_string(),
                                token_type,
                                expiry_month: exp_month,
                                expiry_year: expiry_year_4_digit,
                                billing_address: billing_details,
                            });

                        Ok((
                            payment_source,
                            Some(network_transaction_id.network_transaction_id.clone()),
                            Some(true),
                            p_type,
                            None,
                        ))
                    }
                    _ => Err(IntegrationError::NotImplemented(
                        utils::get_unimplemented_payment_method_error_message("checkout"),
                        IntegrationErrorContext {
                            additional_context: Some("A network-transaction-id replay on Checkout needs either raw card \
                             details or a decrypted wallet network token".to_owned()),
                            ..Default::default()
                        },
                    )),
                }
            }
            _ => Err(IntegrationError::NotImplemented(
                utils::get_unimplemented_payment_method_error_message("checkout"),
                IntegrationErrorContext {
                    additional_context: Some(
                        "Checkout RepeatPayment supports a connector mandate id (source_id) or \
                     a network transaction id"
                            .to_owned(),
                    ),
                    ..Default::default()
                },
            )),
        }?;

        let three_ds = CheckoutThreeDS {
            enabled: false,
            force_3ds: false,
            eci: None,
            cryptogram: None,
            xid: None,
            version: None,
            challenge_indicator: CheckoutChallengeIndicator::NoPreference,
        };

        let return_url = ReturnUrl {
            success_url: item
                .router_data
                .request
                .router_return_url
                .as_ref()
                .map(|return_url| format!("{return_url}?status=success")),
            failure_url: item
                .router_data
                .request
                .router_return_url
                .as_ref()
                .map(|return_url| format!("{return_url}?status=failure")),
        };

        let connector_auth = &item.router_data.connector_config;
        let auth_type: CheckoutAuthType = connector_auth.try_into()?;
        let processing_channel_id = auth_type.processing_channel_id;

        let metadata = build_metadata(
            item.router_data.request.metadata.clone(),
            item.router_data
                .request
                .partner_merchant_identifier_details
                .as_ref(),
        );

        let (customer, mut processing, shipping, items) = if let Some(l2l3_data) =
            &item.router_data.resource_common_data.l2_l3_data
        {
            (
                l2l3_data.customer_info.as_ref().map(|_| CheckoutCustomer {
                    name: l2l3_data.get_customer_name(),
                    email: l2l3_data.get_customer_email(),
                    phone: Some(CheckoutPhoneDetails {
                        country_code: l2l3_data.get_customer_phone_country_code(),
                        number: l2l3_data.get_customer_phone_number(),
                    }),
                    tax_number: l2l3_data.get_customer_tax_registration_id(),
                }),
                l2l3_data.order_info.as_ref().map(|_| CheckoutProcessing {
                    order_id: l2l3_data.get_merchant_order_reference_id(),
                    tax_amount: l2l3_data.get_order_tax_amount(),
                    discount_amount: l2l3_data.get_discount_amount(),
                    duty_amount: l2l3_data.get_duty_amount(),
                    shipping_amount: l2l3_data.get_shipping_cost(),
                    shipping_tax_amount: l2l3_data.get_shipping_amount_tax(),
                    aft: None,
                }),
                Some(CheckoutShipping {
                    address: Some(CheckoutAddress {
                        country: l2l3_data.get_shipping_country(),
                        address_line1: l2l3_data.get_shipping_address_line1(),
                        address_line2: l2l3_data.get_shipping_address_line2(),
                        city: l2l3_data.get_shipping_city(),
                        state: l2l3_data.get_shipping_state(),
                        zip: l2l3_data.get_shipping_zip(),
                    }),
                    from_address_zip: l2l3_data.get_shipping_origin_zip().map(|zip| zip.expose()),
                }),
                l2l3_data.get_order_details().map(|details| {
                    details
                        .iter()
                        .map(|item| CheckoutLineItem {
                            commodity_code: item.commodity_code.clone(),
                            discount_amount: item.unit_discount_amount,
                            name: Some(item.product_name.clone()),
                            quantity: Some(item.quantity),
                            reference: item.product_id.clone(),
                            tax_exempt: None,
                            tax_amount: item.total_tax_amount,
                            total_amount: item.total_amount,
                            unit_of_measure: item.unit_of_measure.clone(),
                            unit_price: Some(item.amount),
                        })
                        .collect::<Vec<_>>()
                }),
            )
        } else {
            (None, None, None, None)
        };

        let is_account_funding_transaction = item
            .router_data
            .request
            .is_account_funding_transaction
            .unwrap_or(false);

        let (recipient, sender, instruction) = if is_account_funding_transaction {
            processing
                .get_or_insert_with(CheckoutProcessing::default)
                .aft = Some(true);

            let purpose = item
                .router_data
                .request
                .additional_connector_details
                .as_ref()
                .and_then(|details| details.checkout.as_ref())
                .and_then(|checkout| checkout.purpose_of_payment.clone())
                .ok_or_else(utils::missing_field_err(
                    "additional_connector_details.checkout.purpose_of_payment",
                ))?;

            let sender_date_of_birth = item
                .router_data
                .request
                .customer
                .as_ref()
                .and_then(|customer| customer.date_of_birth.clone())
                .ok_or_else(utils::missing_field_err("customer.date_of_birth"))?;

            (
                Some(build_checkout_recipient(
                    item.router_data.request.recipient_details.as_ref(),
                )?),
                Some(build_checkout_sender(
                    &item.router_data.resource_common_data,
                    sender_date_of_birth,
                )?),
                Some(CheckoutInstruction { purpose }),
            )
        } else {
            (None, None, None)
        };

        let partial_authorization = item.router_data.request.enable_partial_authorization.map(
            |enable_partial_authorization| CheckoutPartialAuthorization {
                enabled: enable_partial_authorization,
            },
        );

        let payment_ip = item.router_data.request.get_ip_address_as_optional();

        let billing_descriptor =
            item.router_data
                .request
                .billing_descriptor
                .as_ref()
                .map(|descriptor| CheckoutBillingDescriptor {
                    name: descriptor.name.clone(),
                    city: descriptor.city.clone(),
                    reference: descriptor.reference.clone(),
                });

        let request = Self {
            source: source_var,
            amount: item.router_data.request.minor_amount,
            currency: item.router_data.request.currency.to_string(),
            processing_channel_id,
            three_ds,
            return_url,
            capture,
            reference: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            metadata,
            payment_type,
            merchant_initiated,
            previous_payment_id,
            store_for_future_use,
            partial_authorization,
            customer,
            processing,
            shipping,
            items,
            payment_ip,
            billing_descriptor,
            recipient,
            sender,
            instruction,
        };

        Ok(request)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        CheckoutRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: CheckoutRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let payment_type = if matches!(
            item.router_data.request.payment_channel,
            Some(
                common_enums::PaymentChannel::MailOrder
                    | common_enums::PaymentChannel::TelephoneOrder
            )
        ) {
            CheckoutPaymentType::Moto
        } else {
            CheckoutPaymentType::Unscheduled
        };

        let billing_details = Some(CheckoutAddress {
            city: item
                .router_data
                .resource_common_data
                .get_optional_billing_city(),
            address_line1: item
                .router_data
                .resource_common_data
                .get_optional_billing_line1(),
            address_line2: item
                .router_data
                .resource_common_data
                .get_optional_billing_line2(),
            state: item
                .router_data
                .resource_common_data
                .get_optional_billing_state(),
            zip: item
                .router_data
                .resource_common_data
                .get_optional_billing_zip(),
            country: item
                .router_data
                .resource_common_data
                .get_optional_billing_country(),
        });

        let (
            source_var,
            previous_payment_id,
            merchant_initiated,
            payment_type,
            store_for_future_use,
        ) = match item.router_data.request.payment_method_data.clone() {
            PaymentMethodData::Card(ccard) => {
                let (first_name, last_name) = split_account_holder_name(ccard.card_holder_name);

                let payment_source = PaymentSource::Card(CardSource {
                    source_type: CheckoutSourceTypes::Card,
                    number: ccard.card_number.clone(),
                    expiry_month: ccard.card_exp_month.clone(),
                    expiry_year: ccard.card_exp_year.clone(),
                    cvv: Some(ccard.card_cvc),
                    billing_address: billing_details,
                    account_holder: Some(CheckoutAccountHolderDetails {
                        first_name,
                        last_name,
                    }),
                });
                Ok((payment_source, None, Some(false), payment_type, Some(true)))
            }
            PaymentMethodData::BankDebit(BankDebitData::AchBankDebit {
                account_number,
                routing_number,
                bank_account_holder_name,
                card_holder_name,
                bank_holder_type,
                bank_type,
                ..
            }) => {
                // Get account holder name from bank_account_holder_name, card_holder_name, or billing details
                let holder_name = bank_account_holder_name.or(card_holder_name).or_else(|| {
                    item.router_data
                        .resource_common_data
                        .get_billing_full_name()
                        .ok()
                });

                // Map bank_holder_type to Checkout's expected format
                let holder_type: CheckoutAchHolderType = bank_holder_type
                    .map(Into::into)
                    .unwrap_or(CheckoutAchHolderType::Individual);

                // Only include account_holder when a name is available to avoid
                // sending null first_name/last_name which causes ACH validation errors
                let account_holder = match holder_name {
                    Some(name) => {
                        let (first_name, last_name) = split_account_holder_name(Some(name));
                        Some(AchAccountHolder {
                            holder_type,
                            first_name,
                            last_name,
                        })
                    }
                    None => None,
                };

                let account_type = CheckoutBankType::try_from(
                    bank_type.unwrap_or(common_enums::BankType::Savings),
                )?;

                let payment_source = PaymentSource::AchBankDebit(AchBankDebitSource {
                    source_type: ACH_PAYMENT_TYPE.to_string(),
                    account_type,
                    country: ACH_COUNTRY_US.to_string(),
                    account_number: account_number.clone(),
                    routing_number: routing_number.clone(),
                    account_holder,
                });
                Ok((payment_source, None, Some(false), payment_type, Some(true)))
            }
            // Apple Pay / Google Pay tokens that arrive decrypted can seed a mandate the same
            // way a raw card can, so a zero-amount setup must accept them too.
            PaymentMethodData::Wallet(wallet_data) => {
                let payment_source = PaymentSource::try_from((&wallet_data, billing_details))?;
                Ok((payment_source, None, Some(false), payment_type, Some(true)))
            }
            // Connector-decryption path: same as Authorize, the wallet payload was already
            // exchanged for a Checkout token, so the mandate setup only references it.
            PaymentMethodData::PaymentMethodToken(token_data) => {
                let payment_source =
                    PaymentSource::Wallets((token_data.token, billing_details).into());
                Ok((payment_source, None, Some(false), payment_type, Some(true)))
            }
            _ => Err(IntegrationError::NotImplemented(
                utils::get_unimplemented_payment_method_error_message("checkout"),
                IntegrationErrorContext {
                    additional_context: Some(
                        "Checkout SetupMandate supports cards, decrypted Google Pay / Apple Pay \
                     wallets and Checkout tokens"
                            .to_owned(),
                    ),
                    ..Default::default()
                },
            )),
        }?;

        let three_ds = match item.router_data.resource_common_data.auth_type {
            common_enums::AuthenticationType::ThreeDs => CheckoutThreeDS {
                enabled: true,
                force_3ds: true,
                eci: None,
                cryptogram: None,
                xid: None,
                version: None,
                challenge_indicator: CheckoutChallengeIndicator::ChallengeRequestedMandate,
            },
            common_enums::AuthenticationType::NoThreeDs => CheckoutThreeDS {
                enabled: false,
                force_3ds: false,
                eci: None,
                cryptogram: None,
                xid: None,
                version: None,
                challenge_indicator: CheckoutChallengeIndicator::NoPreference,
            },
        };

        let return_url = ReturnUrl {
            success_url: item
                .router_data
                .request
                .router_return_url
                .as_ref()
                .map(|return_url| format!("{return_url}?status=success")),
            failure_url: item
                .router_data
                .request
                .router_return_url
                .as_ref()
                .map(|return_url| format!("{return_url}?status=failure")),
        };

        let connector_auth = &item.router_data.connector_config;
        let auth_type: CheckoutAuthType = connector_auth.try_into()?;
        let processing_channel_id = auth_type.processing_channel_id;

        let metadata = build_metadata(
            item.router_data.request.metadata.clone(),
            item.router_data
                .request
                .partner_merchant_identifier_details
                .as_ref(),
        );

        let (customer, mut processing, shipping, items) = if let Some(l2l3_data) =
            &item.router_data.resource_common_data.l2_l3_data
        {
            (
                l2l3_data.customer_info.as_ref().map(|_| CheckoutCustomer {
                    name: l2l3_data.get_customer_name(),
                    email: l2l3_data.get_customer_email(),
                    phone: Some(CheckoutPhoneDetails {
                        country_code: l2l3_data.get_customer_phone_country_code(),
                        number: l2l3_data.get_customer_phone_number(),
                    }),
                    tax_number: l2l3_data.get_customer_tax_registration_id(),
                }),
                l2l3_data.order_info.as_ref().map(|_| CheckoutProcessing {
                    order_id: l2l3_data.get_merchant_order_reference_id(),
                    tax_amount: l2l3_data.get_order_tax_amount(),
                    discount_amount: l2l3_data.get_discount_amount(),
                    duty_amount: l2l3_data.get_duty_amount(),
                    shipping_amount: l2l3_data.get_shipping_cost(),
                    shipping_tax_amount: l2l3_data.get_shipping_amount_tax(),
                    aft: None,
                }),
                Some(CheckoutShipping {
                    address: Some(CheckoutAddress {
                        country: l2l3_data.get_shipping_country(),
                        address_line1: l2l3_data.get_shipping_address_line1(),
                        address_line2: l2l3_data.get_shipping_address_line2(),
                        city: l2l3_data.get_shipping_city(),
                        state: l2l3_data.get_shipping_state(),
                        zip: l2l3_data.get_shipping_zip(),
                    }),
                    from_address_zip: l2l3_data.get_shipping_origin_zip().map(|zip| zip.expose()),
                }),
                l2l3_data.get_order_details().map(|details| {
                    details
                        .iter()
                        .map(|item| CheckoutLineItem {
                            commodity_code: item.commodity_code.clone(),
                            discount_amount: item.unit_discount_amount,
                            name: Some(item.product_name.clone()),
                            quantity: Some(item.quantity),
                            reference: item.product_id.clone(),
                            tax_exempt: None,
                            tax_amount: item.total_tax_amount,
                            total_amount: item.total_amount,
                            unit_of_measure: item.unit_of_measure.clone(),
                            unit_price: Some(item.amount),
                        })
                        .collect::<Vec<_>>()
                }),
            )
        } else {
            (None, None, None, None)
        };

        let is_account_funding_transaction = item
            .router_data
            .request
            .is_account_funding_transaction
            .unwrap_or(false);

        let (recipient, sender, instruction) = if is_account_funding_transaction {
            processing
                .get_or_insert_with(CheckoutProcessing::default)
                .aft = Some(true);

            let purpose = item
                .router_data
                .request
                .additional_connector_details
                .as_ref()
                .and_then(|details| details.checkout.as_ref())
                .and_then(|checkout| checkout.purpose_of_payment.clone())
                .ok_or_else(utils::missing_field_err(
                    "additional_connector_details.checkout.purpose_of_payment",
                ))?;

            let sender_date_of_birth = item
                .router_data
                .request
                .customer
                .as_ref()
                .and_then(|customer| customer.date_of_birth.clone())
                .ok_or_else(utils::missing_field_err("customer.date_of_birth"))?;

            (
                Some(build_checkout_recipient(
                    item.router_data.request.recipient_details.as_ref(),
                )?),
                Some(build_checkout_sender(
                    &item.router_data.resource_common_data,
                    sender_date_of_birth,
                )?),
                Some(CheckoutInstruction { purpose }),
            )
        } else {
            (None, None, None)
        };

        let partial_authorization = item.router_data.request.enable_partial_authorization.map(
            |enable_partial_authorization| CheckoutPartialAuthorization {
                enabled: enable_partial_authorization,
            },
        );

        let payment_ip = item.router_data.request.get_ip_address_as_optional();

        let billing_descriptor =
            item.router_data
                .request
                .billing_descriptor
                .as_ref()
                .map(|descriptor| CheckoutBillingDescriptor {
                    name: descriptor.name.clone(),
                    city: descriptor.city.clone(),
                    reference: descriptor.reference.clone(),
                });

        let request = Self {
            source: source_var,
            amount: MinorUnit::new(0),
            currency: item.router_data.request.currency.to_string(),
            processing_channel_id,
            three_ds,
            return_url,
            capture: true,
            reference: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            metadata,
            payment_type,
            merchant_initiated,
            previous_payment_id,
            store_for_future_use,
            partial_authorization,
            customer,
            processing,
            shipping,
            items,
            payment_ip,
            billing_descriptor,
            recipient,
            sender,
            instruction,
        };

        Ok(request)
    }
}

#[derive(Default, Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum CheckoutPaymentStatus {
    Authorized,
    #[default]
    Pending,
    #[serde(rename = "Card Verified")]
    CardVerified,
    Declined,
    Captured,
    #[serde(rename = "Retry Scheduled")]
    RetryScheduled,
    Voided,
    #[serde(rename = "Partially Captured")]
    PartiallyCaptured,
    #[serde(rename = "Partially Refunded")]
    PartiallyRefunded,
    Refunded,
    Canceled,
    Expired,
}

fn get_attempt_status_cap(
    item: (CheckoutPaymentStatus, Option<common_enums::CaptureMethod>),
) -> common_enums::AttemptStatus {
    let (status, capture_method) = item;
    match status {
        CheckoutPaymentStatus::Authorized => {
            if capture_method == Some(common_enums::CaptureMethod::Automatic)
                || capture_method.is_none()
            {
                common_enums::AttemptStatus::Charged
            } else {
                common_enums::AttemptStatus::Authorized
            }
        }
        CheckoutPaymentStatus::Captured
        | CheckoutPaymentStatus::PartiallyRefunded
        | CheckoutPaymentStatus::Refunded
        | CheckoutPaymentStatus::CardVerified => common_enums::AttemptStatus::Charged,
        CheckoutPaymentStatus::PartiallyCaptured => common_enums::AttemptStatus::PartialCharged,
        CheckoutPaymentStatus::Declined
        | CheckoutPaymentStatus::Expired
        | CheckoutPaymentStatus::Canceled => common_enums::AttemptStatus::Failure,
        CheckoutPaymentStatus::Pending => common_enums::AttemptStatus::AuthenticationPending,
        CheckoutPaymentStatus::RetryScheduled => common_enums::AttemptStatus::Pending,
        CheckoutPaymentStatus::Voided => common_enums::AttemptStatus::Voided,
    }
}

fn get_attempt_status_intent(
    item: (CheckoutPaymentStatus, CheckoutPaymentIntent),
) -> common_enums::AttemptStatus {
    let (status, psync_flow) = item;

    match status {
        CheckoutPaymentStatus::Authorized => {
            if psync_flow == CheckoutPaymentIntent::Capture {
                common_enums::AttemptStatus::Charged
            } else {
                common_enums::AttemptStatus::Authorized
            }
        }
        CheckoutPaymentStatus::Captured
        | CheckoutPaymentStatus::PartiallyRefunded
        | CheckoutPaymentStatus::Refunded
        | CheckoutPaymentStatus::CardVerified => common_enums::AttemptStatus::Charged,
        CheckoutPaymentStatus::PartiallyCaptured => common_enums::AttemptStatus::PartialCharged,
        CheckoutPaymentStatus::Declined
        | CheckoutPaymentStatus::Expired
        | CheckoutPaymentStatus::Canceled => common_enums::AttemptStatus::Failure,
        CheckoutPaymentStatus::Pending => common_enums::AttemptStatus::AuthenticationPending,
        CheckoutPaymentStatus::RetryScheduled => common_enums::AttemptStatus::Pending,
        CheckoutPaymentStatus::Voided => common_enums::AttemptStatus::Voided,
    }
}

fn get_attempt_status_bal(
    item: (CheckoutPaymentStatus, Option<Balances>),
) -> common_enums::AttemptStatus {
    let (status, balances) = item;

    match status {
        CheckoutPaymentStatus::Authorized => {
            if let Some(Balances {
                available_to_capture: 0,
            }) = balances
            {
                common_enums::AttemptStatus::Charged
            } else {
                common_enums::AttemptStatus::Authorized
            }
        }
        CheckoutPaymentStatus::Captured
        | CheckoutPaymentStatus::PartiallyRefunded
        | CheckoutPaymentStatus::Refunded => common_enums::AttemptStatus::Charged,
        CheckoutPaymentStatus::PartiallyCaptured => common_enums::AttemptStatus::PartialCharged,
        CheckoutPaymentStatus::Declined
        | CheckoutPaymentStatus::Expired
        | CheckoutPaymentStatus::Canceled => common_enums::AttemptStatus::Failure,
        CheckoutPaymentStatus::Pending => common_enums::AttemptStatus::AuthenticationPending,
        CheckoutPaymentStatus::CardVerified | CheckoutPaymentStatus::RetryScheduled => {
            common_enums::AttemptStatus::Pending
        }
        CheckoutPaymentStatus::Voided => common_enums::AttemptStatus::Voided,
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct Href {
    #[serde(rename = "href")]
    redirection_url: Url,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct Links {
    redirect: Option<Href>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct Source {
    id: Option<String>,
    avs_check: Option<String>,
    cvv_check: Option<String>,
    payment_account_reference: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct PaymentsResponse {
    id: String,
    amount: Option<MinorUnit>,
    currency: Option<String>,
    scheme_id: Option<String>,
    processing: Option<PaymentProcessingDetails>,
    action_id: Option<String>,
    status: CheckoutPaymentStatus,
    #[serde(rename = "_links")]
    links: Links,
    balances: Option<Balances>,
    reference: Option<String>,
    response_code: Option<String>,
    response_summary: Option<String>,
    approved: Option<bool>,
    processed_on: Option<String>,
    source: Option<Source>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct PaymentProcessingDetails {
    /// A scheme-generated reference that Mastercard intends to use for tracking and linking transactions across the ecosystem.
    pub scheme_transaction_link_id: Option<String>,
    /// The Merchant Advice Code (MAC) provided by Mastercard, which contains additional information about the transaction.
    pub partner_merchant_advice_code: Option<String>,
    /// The original authorization response code sent by the scheme.
    pub partner_response_code: Option<String>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum PaymentsResponseEnum {
    ActionResponse(Vec<ActionResponse>),
    PaymentResponse(Box<PaymentsResponse>),
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct Balances {
    available_to_capture: i32,
}

fn get_connector_meta(
    capture_method: common_enums::CaptureMethod,
    http_status: u16,
) -> CustomResult<serde_json::Value, ConnectorError> {
    match capture_method {
        common_enums::CaptureMethod::Automatic
        | common_enums::CaptureMethod::SequentialAutomatic => Ok(serde_json::json!(CheckoutMeta {
            psync_flow: CheckoutPaymentIntent::Capture
        })),
        common_enums::CaptureMethod::Manual | common_enums::CaptureMethod::ManualMultiple => {
            Ok(serde_json::json!(CheckoutMeta {
                psync_flow: CheckoutPaymentIntent::Authorize
            }))
        }
        common_enums::CaptureMethod::Scheduled => {
            Err(crate::utils::unexpected_response_fail(http_status, "checkout: unexpected response for this operation; retry with idempotency keys and check connector status.").into())
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<PaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<PaymentsResponse, Self>) -> Result<Self, Self::Error> {
        let status = get_attempt_status_cap((
            item.response.status,
            item.router_data.request.capture_method,
        ));

        if status == common_enums::AttemptStatus::Failure {
            let error_response = ErrorResponse {
                status_code: item.http_code,
                code: item
                    .response
                    .response_code
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: item
                    .response
                    .response_summary
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: item.response.response_summary,
                attempt_status: None,
                connector_transaction_id: Some(item.response.id.clone()),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            };

            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                response: Err(error_response),
                ..item.router_data
            });
        }

        let connector_meta = get_connector_meta(
            item.router_data.request.capture_method.unwrap_or_default(),
            item.http_code,
        )?;

        let redirection_data = item
            .response
            .links
            .redirect
            .map(|href| RedirectForm::from((href.redirection_url, Method::Get)));

        let mandate_reference = if item.router_data.request.is_mandate_payment() {
            item.response
                .source
                .as_ref()
                .and_then(|src| src.id.clone())
                .map(|id| MandateReference {
                    connector_mandate_id: Some(id),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: Some(item.response.id.clone()),
                    mandate_metadata: None,
                })
        } else {
            None
        };

        let additional_information =
            convert_to_additional_payment_method_connector_response(item.response.source.as_ref())
                .map(ConnectorResponseData::with_additional_payment_method_data);

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
            redirection_data: redirection_data.map(Box::new),
            mandate_reference: mandate_reference.map(Box::new),
            connector_metadata: Some(connector_meta),
            network_txn_id: item.response.scheme_id.clone(),
            network_txn_link_id: item
                .response
                .processing
                .clone()
                .and_then(|processing| processing.scheme_transaction_link_id.clone()),
            connector_response_reference_id: Some(
                item.response.reference.unwrap_or(item.response.id),
            ),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
            splits: None,
            payment_account_reference: item
                .response
                .source
                .as_ref()
                .and_then(|source| source.payment_account_reference.clone()),
        };

        let (amount_captured, minor_amount_capturable) =
            match item.router_data.request.capture_method {
                Some(common_enums::CaptureMethod::Manual)
                | Some(common_enums::CaptureMethod::ManualMultiple) => (None, item.response.amount),
                _ => (item.response.amount.map(MinorUnit::get_amount_as_i64), None),
            };

        let minor_amount_authorized = item
            .router_data
            .request
            .enable_partial_authorization
            .filter(|flag| *flag)
            .and(item.response.amount);

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_response: additional_information,
                minor_amount_authorized,
                amount_captured,
                minor_amount_capturable,
                ..item.router_data.resource_common_data
            },
            response: Ok(payments_response_data),
            ..item.router_data
        })
    }
}

impl<
        T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize + Serialize,
    > TryFrom<ResponseRouterData<PaymentsResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<PaymentsResponse, Self>) -> Result<Self, Self::Error> {
        let status = get_attempt_status_cap((
            item.response.status,
            item.router_data.request.capture_method,
        ));

        match status {
            common_enums::AttemptStatus::Failure => {
                let error_response = ErrorResponse {
                    status_code: item.http_code,
                    code: item
                        .response
                        .response_code
                        .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                    message: item
                        .response
                        .response_summary
                        .clone()
                        .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                    reason: item.response.response_summary,
                    attempt_status: None,
                    connector_transaction_id: Some(item.response.id.clone()),
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                };

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        ..item.router_data.resource_common_data
                    },
                    response: Err(error_response),
                    ..item.router_data
                })
            }
            _ => {
                let connector_meta = get_connector_meta(
                    item.router_data.request.capture_method.unwrap_or_default(),
                    item.http_code,
                )?;

                let redirection_data = item
                    .response
                    .links
                    .redirect
                    .map(|href| RedirectForm::from((href.redirection_url, Method::Get)));

                let mandate_reference = item
                    .response
                    .source
                    .as_ref()
                    .and_then(|src| src.id.clone())
                    .map(|id| MandateReference {
                        connector_mandate_id: Some(id),
                        payment_method_id: None,
                        connector_mandate_request_reference_id: Some(item.response.id.clone()),
                        mandate_metadata: None,
                    });

                let additional_information =
                    convert_to_additional_payment_method_connector_response(
                        item.response.source.as_ref(),
                    )
                    .map(ConnectorResponseData::with_additional_payment_method_data);

                let payments_response_data = PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                    redirection_data: redirection_data.map(Box::new),
                    mandate_reference: mandate_reference.map(Box::new),
                    connector_metadata: Some(connector_meta),
                    network_txn_id: item.response.scheme_id.clone(),
                    network_txn_link_id: None,
                    connector_response_reference_id: Some(
                        item.response.reference.unwrap_or(item.response.id),
                    ),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                    splits: None,
                    payment_account_reference: item
                        .response
                        .source
                        .as_ref()
                        .and_then(|source| source.payment_account_reference.clone()),
                };

                let (amount_captured, minor_amount_capturable) =
                    match item.router_data.request.capture_method {
                        Some(common_enums::CaptureMethod::Manual)
                        | Some(common_enums::CaptureMethod::ManualMultiple) => {
                            (None, item.response.amount)
                        }
                        _ => (item.response.amount.map(MinorUnit::get_amount_as_i64), None),
                    };

                let minor_amount_authorized = item
                    .router_data
                    .request
                    .enable_partial_authorization
                    .filter(|flag| *flag)
                    .and(item.response.amount);

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        connector_response: additional_information,
                        minor_amount_authorized,
                        amount_captured,
                        minor_amount_capturable,
                        ..item.router_data.resource_common_data
                    },
                    response: Ok(payments_response_data),
                    ..item.router_data
                })
            }
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<PaymentsResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<PaymentsResponse, Self>) -> Result<Self, Self::Error> {
        let connector_meta = get_connector_meta(
            item.router_data.request.capture_method.unwrap_or_default(),
            item.http_code,
        )?;
        let redirection_data = item
            .response
            .links
            .redirect
            .map(|href| RedirectForm::from((href.redirection_url, Method::Get)));
        let status = get_attempt_status_cap((
            item.response.status,
            item.router_data.request.capture_method,
        ));
        let network_advice_code = item
            .response
            .processing
            .as_ref()
            .and_then(|processing| {
                processing
                    .partner_merchant_advice_code
                    .as_ref()
                    .or(processing.partner_response_code.as_ref())
            })
            .cloned();
        let error_response = if status == common_enums::AttemptStatus::Failure {
            Some(ErrorResponse {
                status_code: item.http_code,
                code: item
                    .response
                    .response_code
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: item
                    .response
                    .response_summary
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: item.response.response_summary,
                attempt_status: None,
                connector_transaction_id: Some(item.response.id.clone()),
                network_advice_code,
                network_decline_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            })
        } else {
            None
        };

        let mandate_reference = item
            .response
            .source
            .as_ref()
            .and_then(|src| src.id.clone())
            .map(|id| MandateReference {
                connector_mandate_id: Some(id),
                payment_method_id: None,
                connector_mandate_request_reference_id: Some(item.response.id.clone()),
                mandate_metadata: None,
            });

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
            redirection_data: redirection_data.map(Box::new),
            mandate_reference: mandate_reference.map(Box::new),
            connector_metadata: Some(connector_meta),
            network_txn_id: item.response.scheme_id.clone(),
            network_txn_link_id: item
                .response
                .processing
                .and_then(|processing| processing.scheme_transaction_link_id.clone()),
            connector_response_reference_id: Some(
                item.response.reference.unwrap_or(item.response.id),
            ),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
            splits: None,
            payment_account_reference: item
                .response
                .source
                .as_ref()
                .and_then(|source| source.payment_account_reference.clone()),
        };
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response: error_response.map_or_else(|| Ok(payments_response_data), Err),
            ..item.router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<PaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<PaymentsResponse, Self>) -> Result<Self, Self::Error> {
        let redirection_data = item
            .response
            .links
            .redirect
            .map(|href| RedirectForm::from((href.redirection_url, Method::Get)));

        let checkout_meta = match item.router_data.request.capture_method {
            Some(common_enums::CaptureMethod::Automatic)
            | Some(common_enums::CaptureMethod::SequentialAutomatic) => CheckoutMeta {
                psync_flow: CheckoutPaymentIntent::Capture,
            },
            Some(common_enums::CaptureMethod::Manual)
            | Some(common_enums::CaptureMethod::ManualMultiple) => CheckoutMeta {
                psync_flow: CheckoutPaymentIntent::Authorize,
            },
            Some(common_enums::CaptureMethod::Scheduled) => {
                return Err(
                    crate::utils::unexpected_response_fail(item.http_code, "checkout: unexpected response for this operation; retry with idempotency keys and check connector status.")
                        .into(),
                );
            }
            None => {
                return Err(ConnectorError::response_handling_failed_with_context(
                    item.http_code,
                    Some("Checkout PSync: capture_method absent on payment intent".to_string()),
                )
                .into());
            }
        };

        let status = get_attempt_status_intent((item.response.status, checkout_meta.psync_flow));
        let error_response = if status == common_enums::AttemptStatus::Failure {
            Some(ErrorResponse {
                status_code: item.http_code,
                code: item
                    .response
                    .response_code
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: item
                    .response
                    .response_summary
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: item.response.response_summary,
                attempt_status: None,
                connector_transaction_id: Some(item.response.id.clone()),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            })
        } else {
            None
        };

        let mandate_reference = if item.router_data.request.is_mandate_payment() {
            item.response
                .source
                .as_ref()
                .and_then(|src| src.id.clone())
                .map(|id| MandateReference {
                    connector_mandate_id: Some(id),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: Some(item.response.id.clone()),
                    mandate_metadata: None,
                })
        } else {
            None
        };

        let additional_information =
            convert_to_additional_payment_method_connector_response(item.response.source.as_ref())
                .map(ConnectorResponseData::with_additional_payment_method_data);

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
            redirection_data: redirection_data.map(Box::new),
            mandate_reference: mandate_reference.map(Box::new),
            connector_metadata: None,
            network_txn_id: item.response.scheme_id.clone(),
            network_txn_link_id: item
                .response
                .processing
                .and_then(|processing| processing.scheme_transaction_link_id.clone()),
            connector_response_reference_id: Some(
                item.response.reference.unwrap_or(item.response.id),
            ),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
            splits: None,
            payment_account_reference: item
                .response
                .source
                .as_ref()
                .and_then(|source| source.payment_account_reference.clone()),
        };
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_response: additional_information,
                ..item.router_data.resource_common_data
            },
            response: error_response.map_or_else(|| Ok(payments_response_data), Err),
            ..item.router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<PaymentsResponseEnum, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<PaymentsResponseEnum, Self>) -> Result<Self, Self::Error> {
        let capture_sync_response_list = match item.response {
            PaymentsResponseEnum::PaymentResponse(payments_response) => {
                // for webhook consumption flow
                construct_captures_response_hashmap(vec![payments_response])?
            }
            PaymentsResponseEnum::ActionResponse(action_list) => {
                // for captures sync
                construct_captures_response_hashmap(action_list)?
            }
        };
        Ok(Self {
            response: Ok(PaymentsResponseData::MultipleCaptureResponse {
                capture_sync_response_list,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

#[derive(Clone, Default, Debug, Eq, PartialEq, Serialize)]
pub struct PaymentVoidRequest {
    reference: String,
}
#[derive(Clone, Default, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct PaymentVoidResponse {
    action_id: String,
    reference: String,
    scheme_id: Option<String>,
}

fn http_code_to_attempt_status_for_void_flow(http_code: u16) -> common_enums::AttemptStatus {
    if http_code == 202 {
        common_enums::AttemptStatus::Voided
    } else {
        common_enums::AttemptStatus::VoidFailed
    }
}

impl<F> TryFrom<ResponseRouterData<PaymentVoidResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<PaymentVoidResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;
        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.action_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: item.response.scheme_id.clone(),
                network_txn_link_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status: http_code_to_attempt_status_for_void_flow(item.http_code),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        CheckoutRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for PaymentVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: CheckoutRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            reference: item.router_data.request.connector_transaction_id.clone(),
        })
    }
}

#[derive(Debug, Serialize)]
pub enum CaptureType {
    Final,
    NonFinal,
}

#[derive(Debug, Serialize)]
pub struct PaymentCaptureRequest {
    pub amount: Option<MinorUnit>,
    pub capture_type: Option<CaptureType>,
    pub processing_channel_id: Secret<String>,
    pub reference: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        CheckoutRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for PaymentCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: CheckoutRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let connector_auth = &item.router_data.connector_config;
        let auth_type: CheckoutAuthType = connector_auth.try_into()?;
        let processing_channel_id = auth_type.processing_channel_id;
        let capture_type = if item.router_data.request.is_multiple_capture() {
            CaptureType::NonFinal
        } else {
            CaptureType::Final
        };
        let reference = item
            .router_data
            .request
            .multiple_capture_data
            .as_ref()
            .map(|multiple_capture_data| multiple_capture_data.capture_reference.clone());
        Ok(Self {
            amount: Some(item.router_data.request.minor_amount_to_capture.to_owned()),
            capture_type: Some(capture_type),
            processing_channel_id,
            reference, // hyperswitch's reference for this capture
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PaymentCaptureResponse {
    pub action_id: String,
    pub reference: Option<String>,
    pub scheme_id: Option<String>,
}

impl<F> TryFrom<ResponseRouterData<PaymentCaptureResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PaymentCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let connector_meta = serde_json::json!(CheckoutMeta {
            psync_flow: CheckoutPaymentIntent::Capture
        });
        let (status, amount_captured) = if item.http_code == 202 {
            (
                common_enums::AttemptStatus::Charged,
                Some(item.router_data.request.amount_to_capture),
            )
        } else {
            (common_enums::AttemptStatus::Pending, None)
        };

        // if multiple capture request, return capture action_id so that it will be updated in the captures table.
        // else return previous connector_transaction_id.
        let resource_id = if item.router_data.request.is_multiple_capture() {
            item.response.action_id
        } else {
            match item.router_data.request.get_connector_transaction_id() {
                Ok(id) => id.to_owned(),
                Err(_) => {
                    return Err(crate::utils::response_handling_fail_for_connector(
                        item.http_code,
                        "checkout",
                    )
                    .into());
                }
            }
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(resource_id),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: Some(connector_meta),
                network_txn_id: item.response.scheme_id.clone(),
                network_txn_link_id: None,
                connector_response_reference_id: item.response.reference,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                amount_captured,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RefundRequest {
    amount: Option<MinorUnit>,
    reference: String,
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        CheckoutRouterData<RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for RefundRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: CheckoutRouterData<
            RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let reference = item.router_data.request.refund_id.clone();
        Ok(Self {
            amount: Some(item.router_data.request.minor_refund_amount.to_owned()),
            reference,
        })
    }
}

#[derive(Deserialize, Debug, Serialize)]
pub struct RefundResponse {
    action_id: String,
    reference: String,
}

fn http_code_to_refund_status(http_code: u16) -> common_enums::RefundStatus {
    if http_code == 202 {
        common_enums::RefundStatus::Success
    } else {
        common_enums::RefundStatus::Failure
    }
}

impl<F> TryFrom<ResponseRouterData<RefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<RefundResponse, Self>) -> Result<Self, Self::Error> {
        let refund_status = http_code_to_refund_status(item.http_code);
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.action_id.clone(),
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            ..item.router_data
        })
    }
}

#[derive(Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct CheckoutErrorResponse {
    pub request_id: Option<String>,
    pub error_type: Option<String>,
    pub error_codes: Option<Vec<String>>,
}

#[derive(Deserialize, Debug, PartialEq, Serialize)]
pub enum ActionType {
    Authorization,
    Void,
    Capture,
    Refund,
    Payout,
    Return,
    #[serde(rename = "Card Verification")]
    CardVerification,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct ActionResponse {
    #[serde(rename = "id")]
    pub action_id: String,
    pub amount: MinorUnit,
    #[serde(rename = "type")]
    pub action_type: ActionType,
    pub approved: Option<bool>,
    pub reference: Option<String>,
}

impl MultipleCaptureSyncResponse for ActionResponse {
    fn get_connector_capture_id(&self) -> String {
        self.action_id.clone()
    }

    fn get_capture_attempt_status(&self) -> common_enums::AttemptStatus {
        match self.approved {
            Some(true) => common_enums::AttemptStatus::Charged,
            Some(false) => common_enums::AttemptStatus::Failure,
            None => common_enums::AttemptStatus::Pending,
        }
    }

    fn get_connector_reference_id(&self) -> Option<String> {
        self.reference.clone()
    }

    fn is_capture_response(&self) -> bool {
        self.action_type == ActionType::Capture
    }

    fn get_amount_captured(&self) -> Result<Option<MinorUnit>, error_stack::Report<ParsingError>> {
        Ok(Some(self.amount))
    }
}

impl MultipleCaptureSyncResponse for Box<PaymentsResponse> {
    fn get_connector_capture_id(&self) -> String {
        self.action_id.clone().unwrap_or("".into())
    }

    fn get_capture_attempt_status(&self) -> common_enums::AttemptStatus {
        get_attempt_status_bal((self.status.clone(), self.balances.clone()))
    }

    fn get_connector_reference_id(&self) -> Option<String> {
        self.reference.clone()
    }

    fn is_capture_response(&self) -> bool {
        self.status == CheckoutPaymentStatus::Captured
    }
    fn get_amount_captured(&self) -> Result<Option<MinorUnit>, error_stack::Report<ParsingError>> {
        Ok(self.amount)
    }
}

#[derive(Debug, Clone, serde::Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CheckoutRedirectResponseStatus {
    Success,
    Failure,
}

#[derive(Debug, Clone, serde::Deserialize, Eq, PartialEq)]
pub struct CheckoutRedirectResponse {
    pub status: Option<CheckoutRedirectResponseStatus>,
    #[serde(rename = "cko-session-id")]
    pub cko_session_id: Option<String>,
}

impl From<&ActionResponse> for common_enums::RefundStatus {
    fn from(item: &ActionResponse) -> Self {
        match item.approved {
            Some(true) => Self::Success,
            Some(false) => Self::Failure,
            None => Self::Pending,
        }
    }
}

pub type RSyncResponse = Vec<ActionResponse>;

impl<F> TryFrom<ResponseRouterData<RSyncResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<RSyncResponse, Self>) -> Result<Self, Self::Error> {
        let refund_action_id = item.router_data.request.connector_refund_id.clone();
        let action_response = item
            .response
            .iter()
            .find(|&x| x.action_id.clone() == refund_action_id)
            .ok_or(crate::utils::response_handling_fail_for_connector(
                item.http_code,
                "checkout",
            ))?;
        let refund_status = common_enums::RefundStatus::from(action_response);
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: action_response.action_id.clone(),
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            ..item.router_data
        })
    }
}

impl From<CheckoutRedirectResponseStatus> for common_enums::AttemptStatus {
    fn from(item: CheckoutRedirectResponseStatus) -> Self {
        match item {
            CheckoutRedirectResponseStatus::Success => Self::AuthenticationSuccessful,
            CheckoutRedirectResponseStatus::Failure => Self::Failure,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CheckoutWebhookObjectResource {
    pub data: serde_json::Value,
}

impl From<String> for ErrorCodeAndMessage {
    fn from(error: String) -> Self {
        Self {
            error_code: error.clone(),
            error_message: error,
        }
    }
}

fn convert_to_additional_payment_method_connector_response(
    source: Option<&Source>,
) -> Option<AdditionalPaymentMethodConnectorResponse> {
    source.map(|code| {
        let payment_checks = serde_json::json!({
                    "avs_result": code.avs_check,
                    "card_validation_result": code.cvv_check
        });
        AdditionalPaymentMethodConnectorResponse::Card {
            authentication_data: None,
            payment_checks: Some(payment_checks),
            card_network: None,
            domestic_network: None,
            auth_code: None,
        }
    })
}

fn is_metadata_empty(val: &Option<Secret<serde_json::Value>>) -> bool {
    match val {
        None => true,
        Some(secret) => {
            let inner = secret.clone().expose();
            match inner {
                serde_json::Value::Null => true,
                serde_json::Value::Object(map) => map.is_empty(),
                _ => false,
            }
        }
    }
}
