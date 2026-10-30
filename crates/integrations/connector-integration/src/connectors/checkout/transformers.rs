use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::{CustomResult, ParsingError},
    request::Method,
    types::MinorUnit,
};
use domain_types::{
    connector_flow::{
        Accept, Authorize, Capture, DefendDispute, PaymentMethodToken, RepeatPayment, SetupMandate,
        SubmitEvidence, Void,
    },
    connector_types::{
        AcceptDisputeData, DisputeDefendData, DisputeFlowData, DisputeResponseData,
        MandateReference, MandateReferenceId, PartnerMerchantIdentifierDetails, PaymentFlowData,
        PaymentMethodTokenResponse, PaymentMethodTokenizationData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        RecipientAccount, RecipientBankAccount, RecipientDetails, RefundFlowData, RefundSyncData,
        RefundsData, RefundsResponseData, RepeatPaymentData, ResponseId, SetupMandateRequestData,
        SubmitEvidenceData,
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
    router_request_types::AuthenticationData,
    router_response_types::RedirectForm,
    utils,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<CheckoutPhoneDetails>,
    #[serde(skip_serializing_if = "Option::is_none")]
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
/// Checkout rejects references longer than 30 characters for American Express cards.
const AMEX_PAYMENT_REFERENCE_LENGTH: usize = 30;
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
    #[serde(skip_serializing_if = "Option::is_none")]
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
pub struct CheckoutRawCardDetails<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(rename = "type")]
    pub source_type: CheckoutSourceTypes,
    /// A PAN or a vault alias, depending on the holder — the latter carries the injector's
    /// substitution placeholder, so this cannot narrow to `cards::CardNumber`.
    pub number: RawCardNumber<T>,
    pub expiry_month: Secret<String>,
    pub expiry_year: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cvv: Option<Secret<String>>,
    pub billing_address: Option<CheckoutAddress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_holder: Option<CheckoutAccountHolderDetails>,
    /// `true` when the card details are stored outside Checkout.com and replayed against a
    /// network transaction id (NTID MIT); omitted on a customer-initiated payment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stored: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum PaymentSource<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    Card(CardSource<T>),
    RawCardForNTI(CheckoutRawCardDetails<T>),
    Wallets(WalletSource),
    ApplePayPredecrypt(Box<ApplePayPredecrypt>),
    MandatePayment(MandateSource),
    GooglePayPredecrypt(Box<GooglePayPredecrypt>),
    AchBankDebit(AchBankDebitSource),
    DecryptedWalletToken(DecryptedWalletToken),
    NetworkToken(Box<NetworkTokenSource>),
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
    /// `true` when the network token is stored outside Checkout.com and replayed against a
    /// network transaction id (NTID MIT).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stored: Option<bool>,
}

/// Payment source for Visa Token Service (vts) and Mastercard Digital Enablement Service (mdes)
/// network tokens — both CIT (stored: None) and MIT NTID replay (stored: Some(true)).
#[skip_serializing_none]
#[derive(Debug, Serialize)]
pub struct NetworkTokenSource {
    #[serde(rename = "type")]
    pub source_type: String,
    pub token: cards::NetworkToken,
    pub expiry_month: Secret<String>,
    pub expiry_year: Secret<String>,
    pub token_type: String,
    pub cryptogram: Option<Secret<String>>,
    pub eci: Option<String>,
    pub stored: Option<bool>,
    pub store_for_future_use: Option<bool>,
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
    pub scheme_transaction_link_id: Option<String>,
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
        RecipientAccount::Wallet { wallet_id } => Ok(wallet_id.clone()),
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

    let country = address.country.ok_or_else(utils::missing_field_err(
        "recipient_details.address.country",
    ))?;

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
            state: CheckoutAftStateParams {
                state: address.state.clone(),
                country,
                field_name: "recipient_details.address.state",
            }
            .try_into()?,
            zip: Some(
                address
                    .zip
                    .clone()
                    .ok_or_else(utils::missing_field_err("recipient_details.address.zip"))?,
            ),
            country: Some(country),
        },
    })
}

fn build_checkout_sender(
    resource_common_data: &PaymentFlowData,
    date_of_birth: Secret<time::Date>,
) -> Result<CheckoutSender, error_stack::Report<IntegrationError>> {
    let country = resource_common_data.get_billing_country()?;
    Ok(CheckoutSender {
        sender_type: CheckoutSenderType::Individual,
        first_name: resource_common_data.get_billing_first_name()?,
        last_name: resource_common_data.get_billing_last_name()?,
        date_of_birth,
        address: CheckoutAddress {
            address_line1: Some(resource_common_data.get_billing_line1()?),
            address_line2: resource_common_data.get_optional_billing_line2(),
            city: Some(resource_common_data.get_billing_city()?),
            state: CheckoutAftStateParams {
                state: resource_common_data.get_optional_billing_state(),
                country,
                field_name: "payment_method_data.billing.address.state",
            }
            .try_into()?,
            zip: Some(resource_common_data.get_billing_zip()?),
            country: Some(country),
        },
    })
}

#[skip_serializing_none]
#[derive(Debug, Default, Serialize)]
pub struct CheckoutShipping {
    pub first_name: Option<Secret<String>>,
    pub last_name: Option<Secret<String>>,
    pub email: Option<common_utils::pii::Email>,
    pub address: Option<CheckoutAddress>,
    pub phone: Option<CheckoutPhoneDetails>,
    pub from_address_zip: Option<String>,
}

impl CheckoutAddress {
    fn is_empty(&self) -> bool {
        self.address_line1.is_none()
            && self.address_line2.is_none()
            && self.city.is_none()
            && self.state.is_none()
            && self.zip.is_none()
            && self.country.is_none()
    }
}

impl From<&domain_types::payment_address::AddressDetails> for CheckoutAddress {
    fn from(details: &domain_types::payment_address::AddressDetails) -> Self {
        Self {
            address_line1: details.line1.clone(),
            address_line2: details.line2.clone(),
            city: details.city.clone(),
            state: details.state.clone(),
            zip: details.zip.clone(),
            country: details.country,
        }
    }
}

/// `{country_code, number}` from a phone, or `None` when it carries no number.
fn build_phone(
    phone: Option<&domain_types::payment_address::PhoneDetails>,
) -> Option<CheckoutPhoneDetails> {
    phone.and_then(|phone| {
        phone.number.clone().map(|number| CheckoutPhoneDetails {
            country_code: phone.country_code.clone(),
            number: Some(number),
        })
    })
}

/// `source.billing_address` from the payment-method billing address. Shared by Authorize,
/// RepeatPayment and SetupMandate.
fn build_billing_address(common: &PaymentFlowData) -> Option<CheckoutAddress> {
    Some(CheckoutAddress {
        city: common.get_optional_billing_city(),
        address_line1: common.get_optional_billing_line1(),
        address_line2: common.get_optional_billing_line2(),
        state: common.get_optional_billing_state(),
        zip: common.get_optional_billing_zip(),
        country: common.get_optional_billing_country(),
    })
}

/// `source.phone` from the payment-method billing phone.
fn build_source_phone(common: &PaymentFlowData) -> Option<CheckoutPhoneDetails> {
    build_phone(
        common
            .get_optional_billing()
            .and_then(|billing| billing.phone.as_ref()),
    )
}

/// The request `shipping` object.
///
/// Address details come from the Level 2/3 shipping details when present, else from the
/// payment's shipping address; name, email and phone come from the shipping address. Returns
/// `None` when no field is set, so an empty `"shipping": {"address": {}}` is never sent.
fn build_shipping(common: &PaymentFlowData) -> Option<CheckoutShipping> {
    let shipping = common.get_optional_shipping();
    let details = common
        .l2_l3_data
        .as_ref()
        .and_then(|l2l3| l2l3.shipping_details.as_ref())
        .or_else(|| shipping.and_then(|address| address.address.as_ref()));
    let address = details
        .map(CheckoutAddress::from)
        .filter(|address| !address.is_empty());
    let first_name = details.and_then(|details| details.first_name.clone());
    let last_name = details.and_then(|details| details.last_name.clone());
    let from_address_zip = details
        .and_then(|details| details.origin_zip.clone())
        .map(ExposeInterface::expose);
    let email = shipping.and_then(|address| address.email.clone());
    let phone = build_phone(shipping.and_then(|address| address.phone.as_ref()));

    let is_empty = address.is_none()
        && first_name.is_none()
        && last_name.is_none()
        && from_address_zip.is_none()
        && email.is_none()
        && phone.is_none();
    (!is_empty).then_some(CheckoutShipping {
        first_name,
        last_name,
        email,
        address,
        phone,
        from_address_zip,
    })
}

/// The request `customer` object: the Level 2/3 customer when present, else `{email, name}` from
/// the payment when either is set.
fn build_customer(
    l2l3_data: Option<&domain_types::connector_types::L2L3Data>,
    email: Option<common_utils::pii::Email>,
    name: Option<Secret<String>>,
) -> Option<CheckoutCustomer> {
    match l2l3_data.filter(|l2l3| l2l3.customer_info.is_some()) {
        Some(l2l3) => {
            let phone_number = l2l3.get_customer_phone_number();
            let phone_country_code = l2l3.get_customer_phone_country_code();
            Some(CheckoutCustomer {
                name: l2l3.get_customer_name(),
                email: l2l3.get_customer_email(),
                phone: (phone_number.is_some() || phone_country_code.is_some()).then_some(
                    CheckoutPhoneDetails {
                        country_code: phone_country_code,
                        number: phone_number,
                    },
                ),
                tax_number: l2l3.get_customer_tax_registration_id(),
            })
        }
        None => (email.is_some() || name.is_some()).then_some(CheckoutCustomer {
            name,
            email,
            phone: None,
            tax_number: None,
        }),
    }
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
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CheckoutChallengeIndicator {
    NoPreference,
    ChallengeRequestedMandate,
    ChallengeRequested,
    NoChallengeRequested,
}

/// The `3ds` object of `POST /payments`.
///
/// One struct covers the three shapes Checkout documents, and `skip_serializing_none` keeps each
/// shape free of the keys it does not use (no `null`s on the wire):
/// * integrated 3DS (`3dsRequestEnabled`): `enabled`, `force_3ds`, `challenge_indicator`;
/// * external 3DS from a third-party MPI (`3dsRequestThirdParty`): `enabled` (must be `true`),
///   `eci`, `cryptogram`, `xid`, `version`, `status`, `authentication_date`;
/// * no 3DS: `{"enabled": false}` only.
///
/// Built only by [`build_checkout_three_ds`].
#[skip_serializing_none]
#[derive(Debug, Serialize)]
pub struct CheckoutThreeDS {
    enabled: bool,
    force_3ds: Option<bool>,
    eci: Option<String>,
    cryptogram: Option<Secret<String>>,
    xid: Option<String>,
    version: Option<String>,
    status: Option<CheckoutThreeDsStatus>,
    authentication_date: Option<String>,
    challenge_indicator: Option<CheckoutChallengeIndicator>,
}

/// `3ds.status` of `3dsRequestThirdParty`: the ARes/RReq `transStatus` of an external
/// authentication. Only the two outcomes that may be imported as authenticated exist here — every
/// other `trans_status` is refused before the request is built (see [`build_checkout_three_ds`]).
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum CheckoutThreeDsStatus {
    /// Authenticated.
    Y,
    /// Attempted: not authenticated, but proof of the attempt is provided.
    A,
}

/// ECIs a fully authenticated external 3DS result may carry (Mastercard 02, Visa/Amex/others 05).
const ECI_AUTHENTICATED: [&str; 2] = ["02", "05"];
/// ECIs an attempted external 3DS result may carry (Mastercard 01, Visa/Amex/others 06).
const ECI_ATTEMPTED: [&str; 2] = ["01", "06"];
const CHECKOUT_PAYMENTS_DOC_URL: &str =
    "https://api-reference.checkout.com/#operation/requestAPaymentOrPayout";

/// Left-pads a one-digit ECI ("5" -> "05") so it compares against the two-digit tables.
fn normalize_eci(eci: &str) -> String {
    let eci = eci.trim();
    if eci.len() == 1 {
        format!("0{eci}")
    } else {
        eci.to_string()
    }
}

/// Decides whether an external 3DS result may be imported as `3dsRequestThirdParty`.
///
/// Checkout applies the supplied CAVV/ECI as proof of authentication (and the liability shift that
/// comes with it), so the outcome must be corroborated by the ECI for every `trans_status` that is
/// accepted:
///
/// | `trans_status` | accepted ECI | `3ds.status` |
/// |---|---|---|
/// | `Y` (Success)     | 02, 05         | `Y` |
/// | `A` (NotVerified) | 01, 06         | `A` |
/// | absent            | 01, 02, 05, 06 | omitted |
///
/// Both `cavv` and `eci` must be present. Every other `trans_status` (`N`, `U`, `R`, `C`, `D`,
/// `I`) and every non-corroborating ECI is refused before any HTTP call.
fn validate_external_three_ds(
    authentication_data: &AuthenticationData,
) -> Result<
    (String, Secret<String>, Option<CheckoutThreeDsStatus>),
    error_stack::Report<IntegrationError>,
> {
    let eci = authentication_data.eci.as_deref().map(normalize_eci);
    let refuse = || {
        error_stack::report!(IntegrationError::NotSupported {
            message: "external 3DS result that is not an authenticated or attempted outcome"
                .to_owned(),
            connector: "checkout",
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Send only an authenticated (trans_status Y with ECI 02/05) or attempted \
                     (trans_status A with ECI 01/06) external 3DS result, with both cavv and eci \
                     set; otherwise authorize without authentication_data"
                        .to_owned(),
                ),
                additional_context: Some(format!(
                    "trans_status={:?}, eci={:?}, cavv_present={}",
                    authentication_data.trans_status,
                    eci,
                    authentication_data.cavv.is_some()
                )),
                doc_url: Some(CHECKOUT_PAYMENTS_DOC_URL.to_owned()),
            },
        })
    };
    let (Some(eci), Some(cavv)) = (eci.clone(), authentication_data.cavv.clone()) else {
        return Err(refuse());
    };
    let eci_ref = eci.as_str();
    let status = match authentication_data.trans_status {
        Some(common_enums::TransactionStatus::Success) if ECI_AUTHENTICATED.contains(&eci_ref) => {
            Some(CheckoutThreeDsStatus::Y)
        }
        Some(common_enums::TransactionStatus::NotVerified) if ECI_ATTEMPTED.contains(&eci_ref) => {
            Some(CheckoutThreeDsStatus::A)
        }
        None if ECI_AUTHENTICATED.contains(&eci_ref) || ECI_ATTEMPTED.contains(&eci_ref) => None,
        Some(common_enums::TransactionStatus::Success)
        | Some(common_enums::TransactionStatus::NotVerified)
        | Some(common_enums::TransactionStatus::Failure)
        | Some(common_enums::TransactionStatus::VerificationNotPerformed)
        | Some(common_enums::TransactionStatus::Rejected)
        | Some(common_enums::TransactionStatus::ChallengeRequired)
        | Some(common_enums::TransactionStatus::ChallengeRequiredDecoupledAuthentication)
        | Some(common_enums::TransactionStatus::InformationOnly)
        | Some(common_enums::TransactionStatus::SecurePaymentConfirmationRequired)
        | None => return Err(refuse()),
    };
    Ok((eci, cavv, status))
}

/// Builds the `3ds` object for Authorize and SetupMandate.
///
/// * `authentication_data` present -> external 3DS (`3dsRequestThirdParty`): `enabled: true` (the
///   schema allows only `true`), `eci`, `cryptogram` = CAVV, `xid` = DS transaction id, `version`,
///   `status`, `authentication_date`. Refused unless [`validate_external_three_ds`] accepts it.
/// * else `ThreeDs` -> integrated 3DS: `enabled: true, force_3ds: true` and a challenge
///   indicator (`challenge_requested_mandate` when the payment sets up a mandate).
/// * else `NoThreeDs` -> `{"enabled": false}`.
pub fn build_checkout_three_ds(
    auth_type: common_enums::AuthenticationType,
    authentication_data: Option<&AuthenticationData>,
    is_mandate: bool,
) -> Result<CheckoutThreeDS, error_stack::Report<IntegrationError>> {
    if let Some(auth) = authentication_data {
        let (eci, cavv, status) = validate_external_three_ds(auth)?;
        let authentication_date = auth
            .created_at
            .map(|created_at| {
                created_at
                    .assume_utc()
                    .format(&time::format_description::well_known::Rfc3339)
            })
            .transpose()
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "authentication_data.created_at",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Send a valid authentication timestamp or omit it".to_owned(),
                    ),
                    additional_context: Some(
                        "Checkout 3ds.authentication_date is an ISO 8601 timestamp".to_owned(),
                    ),
                    doc_url: Some(CHECKOUT_PAYMENTS_DOC_URL.to_owned()),
                },
            })?;
        return Ok(CheckoutThreeDS {
            enabled: true,
            force_3ds: None,
            eci: Some(eci),
            cryptogram: Some(cavv),
            xid: auth.ds_trans_id.clone(),
            version: auth.message_version.as_ref().map(ToString::to_string),
            status,
            authentication_date,
            challenge_indicator: None,
        });
    }
    Ok(match auth_type {
        common_enums::AuthenticationType::ThreeDs => CheckoutThreeDS {
            enabled: true,
            force_3ds: Some(true),
            eci: None,
            cryptogram: None,
            xid: None,
            version: None,
            status: None,
            authentication_date: None,
            challenge_indicator: Some(if is_mandate {
                CheckoutChallengeIndicator::ChallengeRequestedMandate
            } else {
                CheckoutChallengeIndicator::ChallengeRequested
            }),
        },
        common_enums::AuthenticationType::NoThreeDs => CheckoutThreeDS {
            enabled: false,
            force_3ds: None,
            eci: None,
            cryptogram: None,
            xid: None,
            version: None,
            status: None,
            authentication_date: None,
            challenge_indicator: None,
        },
    })
}

/// Scope of a `Cko-Idempotency-Key`.
///
/// Checkout caches a `2xx` per key for 72 hours and replays it for any later request carrying the
/// same key. The key is derived from `connector_request_reference_id`, which a caller may reuse
/// across the actions of one payment, so each action gets its own prefix: without it a capture
/// sent with the authorization's key would be answered with the authorization's cached response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckoutIdempotencyScope {
    Payment,
    Capture,
    Void,
    Refund,
}

impl CheckoutIdempotencyScope {
    fn as_str(self) -> &'static str {
        match self {
            Self::Payment => "payment",
            Self::Capture => "capture",
            Self::Void => "void",
            Self::Refund => "refund",
        }
    }

    /// `<scope>_<reference>`, or `None` when the reference is empty — the header is then omitted
    /// rather than sent empty or with a fresh UUID (a fresh UUID would defeat retry safety).
    pub fn idempotency_key(self, connector_request_reference_id: &str) -> Option<String> {
        let reference = connector_request_reference_id.trim();
        (!reference.is_empty()).then(|| format!("{}_{reference}", self.as_str()))
    }
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

/// Returns a Checkout-safe `reference` for the payment.
///
/// American Express rejects references longer than 30 characters. When the connector_request_reference_id
/// exceeds that limit for an Amex card, a UUID v4 (hex, no dashes) truncated to 30 chars is used
/// instead — preserving uniqueness without hitting the scheme limit.
fn get_checkout_reference(connector_request_reference_id: &str, is_amex: bool) -> String {
    if is_amex && connector_request_reference_id.len() > AMEX_PAYMENT_REFERENCE_LENGTH {
        common_utils::fp_utils::generate_uuid_v4()
            .replace('-', "")
            .chars()
            .take(AMEX_PAYMENT_REFERENCE_LENGTH)
            .collect()
    } else {
        connector_request_reference_id.to_string()
    }
}

fn is_amex_card(card_number: &str) -> bool {
    utils::get_card_issuer(card_number)
        .map(|issuer| issuer == utils::CardIssuer::AmericanExpress)
        .unwrap_or(false)
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
                            number: RawCardNumber(T::inner_from_card_number(
                                google_pay_decrypted_data
                                    .application_primary_account_number
                                    .clone(),
                            )),
                            expiry_month,
                            expiry_year,
                            cvv: None,
                            billing_address,
                            account_holder: None,
                            stored: None,
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
pub enum CheckoutWalletTokenRequest {
    Googlepay(CheckoutGooglePayData),
    Applepay(Box<CheckoutApplePayData>),
}

/// `01_CardTokenRequest` of `POST /tokens`, authenticated with the public key. Card fields sit
/// at the top level (no `token_data`). The vendor documents the `card` type as intended for
/// testing; production card capture goes through Frames.
#[derive(Debug, Serialize)]
pub struct CheckoutCardTokenRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(rename = "type")]
    pub token_type: CheckoutSourceTypes,
    pub number: RawCardNumber<T>,
    pub expiry_month: Secret<String>,
    pub expiry_year: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cvv: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_address: Option<CheckoutAddress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<CheckoutPhoneDetails>,
}

/// Request body for `POST /tokens`: a wallet payload (`type` + `token_data`) or a card.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum CheckoutTokenRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    Wallet(CheckoutWalletTokenRequest),
    Card(Box<CheckoutCardTokenRequest<T>>),
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
    > for CheckoutTokenRequest<T>
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
                WalletData::GooglePay(_) => {
                    Ok(Self::Wallet(CheckoutWalletTokenRequest::Googlepay(
                        wallet_data.get_wallet_token_as_json("Google Pay".to_string())?,
                    )))
                }
                WalletData::ApplePay(_) => Ok(Self::Wallet(CheckoutWalletTokenRequest::Applepay(
                    Box::new(wallet_data.get_wallet_token_as_json("Apple Pay".to_string())?),
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
            PaymentMethodData::Card(card) => Ok(Self::Card(Box::new(CheckoutCardTokenRequest {
                token_type: CheckoutSourceTypes::Card,
                number: card.card_number.clone(),
                expiry_month: card.card_exp_month.clone(),
                expiry_year: card.get_expiry_year_4_digit(),
                name: card.card_holder_name.clone(),
                cvv: Some(card.card_cvc.clone()).filter(|cvc| !cvc.peek().is_empty()),
                billing_address: build_billing_address(&item.router_data.resource_common_data),
                phone: build_source_phone(&item.router_data.resource_common_data),
            }))),
            _ => Err(IntegrationError::NotImplemented(
                utils::get_unimplemented_payment_method_error_message("checkout"),
                IntegrationErrorContext {
                    additional_context: Some(
                        "Checkout's Tokenize flow accepts card, Apple Pay and Google Pay"
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

/// Checkout's boolean `capture` for a capture method, shared by every `POST /payments` builder
/// (Authorize, SetupMandate, RepeatPayment).
///
/// An absent capture method is the enum default (Automatic), which is also Checkout's default
/// (`capture: true`). SequentialAutomatic is automatic capture (domain_types `is_auto_capture`),
/// and `get_attempt_status_cap` reads the response the same way, so the wire flag and the
/// reported status cannot disagree. Checkout has no scheduled capture: Scheduled is refused
/// before any HTTP call, on every payment method, instead of being discovered after
/// `POST /payments` may already have moved money.
fn checkout_capture_flag(
    capture_method: Option<common_enums::CaptureMethod>,
) -> Result<bool, error_stack::Report<IntegrationError>> {
    match capture_method.unwrap_or_default() {
        common_enums::CaptureMethod::Automatic
        | common_enums::CaptureMethod::SequentialAutomatic => Ok(true),
        common_enums::CaptureMethod::Manual | common_enums::CaptureMethod::ManualMultiple => {
            Ok(false)
        }
        common_enums::CaptureMethod::Scheduled => {
            Err(error_stack::report!(IntegrationError::NotSupported {
                message: "capture_method Scheduled".to_owned(),
                connector: "checkout",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Use capture_method Automatic or Manual; Checkout has no scheduled capture"
                            .to_owned(),
                    ),
                    additional_context: Some(
                        "Checkout POST /payments takes a boolean `capture` flag only".to_owned(),
                    ),
                    doc_url: Some(CHECKOUT_PAYMENTS_DOC_URL.to_owned()),
                },
            }))
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
        let capture = checkout_capture_flag(item.router_data.request.capture_method)?;

        let payment_type = if matches!(
            item.router_data.request.payment_channel,
            Some(
                common_enums::PaymentChannel::MailOrder
                    | common_enums::PaymentChannel::TelephoneOrder
            )
        ) {
            CheckoutPaymentType::Moto
        } else if item.router_data.request.is_mandate_payment() {
            match item.router_data.request.mit_category {
                Some(common_enums::MitCategory::Installment) => CheckoutPaymentType::Installment,
                Some(
                    common_enums::MitCategory::Recurring | common_enums::MitCategory::Subscription,
                ) => CheckoutPaymentType::Recurring,
                Some(
                    common_enums::MitCategory::Unscheduled
                    | common_enums::MitCategory::Resubmission,
                )
                | None => CheckoutPaymentType::Unscheduled,
            }
        } else {
            CheckoutPaymentType::Regular
        };

        let store_for_future_use = item
            .router_data
            .request
            .is_mandate_payment()
            .then_some(true);

        let billing_details = build_billing_address(&item.router_data.resource_common_data);

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
                        phone: build_source_phone(&item.router_data.resource_common_data),
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
                PaymentMethodData::NetworkToken(ref token_data) => {
                    let token_type = match token_data.card_network {
                        Some(common_enums::CardNetwork::Visa) => Ok("vts".to_string()),
                        Some(common_enums::CardNetwork::Mastercard) => Ok("mdes".to_string()),
                        _ => Err(IntegrationError::NotImplemented(
                            "Network token for this card network".into(),
                            IntegrationErrorContext {
                                ..Default::default()
                            },
                        )),
                    }?;

                    let payment_source =
                        PaymentSource::NetworkToken(Box::new(NetworkTokenSource {
                            source_type: "network_token".to_string(),
                            token: token_data.token_number.clone(),
                            expiry_month: token_data.token_exp_month.clone(),
                            expiry_year: token_data.get_expiry_year_4_digit(),
                            token_type,
                            cryptogram: token_data.token_cryptogram.clone(),
                            eci: token_data.eci.clone(),
                            stored: None,
                            store_for_future_use,
                            billing_address: billing_details,
                        }));

                    Ok((payment_source, None, None, store_for_future_use))
                }
                _ => Err(IntegrationError::NotImplemented(
                    utils::get_unimplemented_payment_method_error_message("checkout"),
                    IntegrationErrorContext {
                        additional_context: Some(
                            "Checkout Authorize supports cards, Google Pay / Apple Pay, Checkout \
                         tokens, ACH bank debit and network tokens (Visa/Mastercard)"
                                .to_owned(),
                        ),
                        ..Default::default()
                    },
                )),
            }?;

        let three_ds = build_checkout_three_ds(
            item.router_data.resource_common_data.auth_type,
            item.router_data.request.authentication_data.as_ref(),
            item.router_data.request.is_mandate_payment(),
        )?;

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

        let l2l3_data = item.router_data.resource_common_data.l2_l3_data.as_deref();
        let customer = build_customer(
            l2l3_data,
            item.router_data.request.email.clone(),
            item.router_data.request.get_optional_customer_name(),
        );
        let shipping = build_shipping(&item.router_data.resource_common_data);
        let (mut processing, items) = if let Some(l2l3_data) = l2l3_data {
            (
                l2l3_data.order_info.as_ref().map(|_| CheckoutProcessing {
                    order_id: l2l3_data.get_merchant_order_reference_id(),
                    scheme_transaction_link_id: None,
                    tax_amount: l2l3_data.get_order_tax_amount(),
                    discount_amount: l2l3_data.get_discount_amount(),
                    duty_amount: l2l3_data.get_duty_amount(),
                    shipping_amount: l2l3_data.get_shipping_cost(),
                    shipping_tax_amount: l2l3_data.get_shipping_amount_tax(),
                    aft: None,
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
            (None, None)
        };

        if let Some(link_id) = item.router_data.request.mandate_id.as_ref().and_then(|m| {
            match m.mandate_reference_id.as_ref() {
                Some(MandateReferenceId::NetworkMandateId(ref_data)) => {
                    ref_data.transaction_link_id.clone()
                }
                Some(MandateReferenceId::NetworkTokenWithNTI(ref_data)) => {
                    ref_data.transaction_link_id.clone()
                }
                _ => None,
            }
        }) {
            processing
                .get_or_insert_with(CheckoutProcessing::default)
                .scheme_transaction_link_id = Some(link_id);
        }

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

        let billing_descriptor = item
            .router_data
            .request
            .billing_descriptor
            .as_ref()
            .and_then(|descriptor| {
                (descriptor.name.is_some()
                    || descriptor.city.is_some()
                    || descriptor.reference.is_some())
                .then(|| CheckoutBillingDescriptor {
                    name: descriptor.name.clone(),
                    city: descriptor.city.clone(),
                    reference: descriptor.reference.clone(),
                })
            });

        let is_amex = matches!(
            &item.router_data.request.payment_method_data,
            PaymentMethodData::Card(ccard) if is_amex_card(ccard.card_number.peek())
        );
        let reference = get_checkout_reference(
            &item
                .router_data
                .resource_common_data
                .connector_request_reference_id,
            is_amex,
        );

        let request = Self {
            source: source_var,
            amount: item.router_data.request.amount.amount,
            currency: item.router_data.request.currency.to_string(),
            processing_channel_id,
            three_ds,
            return_url,
            capture,
            reference,
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
        // Shared with Authorize/SetupMandate: Scheduled is refused here, before any arm builds a
        // request or POST /payments is made; SequentialAutomatic sends `capture: true`.
        let capture = checkout_capture_flag(item.router_data.request.capture_method)?;

        let billing_details = build_billing_address(&item.router_data.resource_common_data);

        let (
            source_var,
            previous_payment_id,
            merchant_initiated,
            payment_type,
            store_for_future_use,
        ) = match &item.router_data.request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(mandate_data) => {
                // G-RepeatPayment-01: `source {type: id}` cannot be built without the `src_` id
                // the SetupMandate response returned, so refuse before any request exists rather
                // than sending `"id": null`.
                let source_id = mandate_data.get_connector_mandate_id().ok_or_else(|| {
                    IntegrationError::MissingRequiredField {
                        field_name: "connector_mandate_id",
                        context: IntegrationErrorContext {
                            suggested_action: Some(
                                "Send the Checkout source id (src_...) returned by the mandate \
                                 setup payment as connector_mandate_id"
                                    .to_owned(),
                            ),
                            doc_url: Some(CHECKOUT_PAYMENTS_DOC_URL.to_owned()),
                            additional_context: Some(
                                "A Checkout merchant-initiated payment against a stored \
                                 instrument needs source.id"
                                    .to_owned(),
                            ),
                        },
                    }
                })?;
                let mandate_source = PaymentSource::MandatePayment(MandateSource {
                    source_type: CheckoutSourceTypes::SourceId,
                    source_id: Some(source_id),
                    billing_address: billing_details,
                });
                let previous_id = mandate_data.get_connector_mandate_request_reference_id();
                let p_type = match item.router_data.request.mit_category {
                    Some(common_enums::MitCategory::Installment) => {
                        CheckoutPaymentType::Installment
                    }
                    Some(
                        common_enums::MitCategory::Recurring
                        | common_enums::MitCategory::Subscription,
                    ) => CheckoutPaymentType::Recurring,
                    Some(
                        common_enums::MitCategory::Unscheduled
                        | common_enums::MitCategory::Resubmission,
                    )
                    | None => CheckoutPaymentType::Unscheduled,
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
                            stored: Some(true),
                        });
                        let p_type = match item.router_data.request.mit_category {
                            Some(common_enums::MitCategory::Installment) => {
                                CheckoutPaymentType::Installment
                            }
                            Some(common_enums::MitCategory::Recurring | common_enums::MitCategory::Subscription) => {
                                CheckoutPaymentType::Recurring
                            }
                            Some(common_enums::MitCategory::Unscheduled | common_enums::MitCategory::Resubmission) | None => CheckoutPaymentType::Unscheduled,
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
                            Some(common_enums::MitCategory::Recurring | common_enums::MitCategory::Subscription) => {
                                CheckoutPaymentType::Recurring
                            }
                            Some(common_enums::MitCategory::Unscheduled | common_enums::MitCategory::Resubmission) | None => CheckoutPaymentType::Unscheduled,
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
                                stored: Some(true),
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
            MandateReferenceId::NetworkTokenWithNTI(ref nti_ref) => {
                match item.router_data.request.payment_method_data {
                    PaymentMethodData::NetworkToken(ref token_data) => {
                        let token_type = match token_data.card_network {
                            Some(common_enums::CardNetwork::Visa) => Ok("vts".to_string()),
                            Some(common_enums::CardNetwork::Mastercard) => Ok("mdes".to_string()),
                            _ => Err(IntegrationError::NotImplemented(
                                "Network token for this card network".into(),
                                IntegrationErrorContext {
                                    ..Default::default()
                                },
                            )),
                        }?;

                        let p_type = match item.router_data.request.mit_category {
                            Some(common_enums::MitCategory::Installment) => {
                                CheckoutPaymentType::Installment
                            }
                            Some(
                                common_enums::MitCategory::Recurring
                                | common_enums::MitCategory::Subscription,
                            ) => CheckoutPaymentType::Recurring,
                            Some(
                                common_enums::MitCategory::Unscheduled
                                | common_enums::MitCategory::Resubmission,
                            )
                            | None => CheckoutPaymentType::Unscheduled,
                        };

                        let payment_source =
                            PaymentSource::NetworkToken(Box::new(NetworkTokenSource {
                                source_type: "network_token".to_string(),
                                token: token_data.token_number.clone(),
                                expiry_month: token_data.token_exp_month.clone(),
                                expiry_year: token_data.get_expiry_year_4_digit(),
                                token_type,
                                cryptogram: None,
                                eci: token_data.eci.clone(),
                                stored: Some(true),
                                store_for_future_use: None,
                                billing_address: billing_details,
                            }));

                        Ok((
                            payment_source,
                            Some(nti_ref.network_transaction_id.clone()),
                            Some(true),
                            p_type,
                            None,
                        ))
                    }
                    _ => Err(IntegrationError::NotImplemented(
                        utils::get_unimplemented_payment_method_error_message("checkout"),
                        IntegrationErrorContext {
                            additional_context: Some(
                                "Checkout NetworkTokenWithNTI replay requires NetworkToken \
                                 payment method data"
                                    .to_owned(),
                            ),
                            ..Default::default()
                        },
                    )),
                }
            }
        }?;

        // A merchant-initiated payment is exempt from SCA: always `{"enabled": false}`.
        let three_ds =
            build_checkout_three_ds(common_enums::AuthenticationType::NoThreeDs, None, true)?;

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

        let l2l3_data = item.router_data.resource_common_data.l2_l3_data.as_deref();
        let customer = build_customer(
            l2l3_data,
            item.router_data.request.email.clone(),
            item.router_data
                .request
                .customer
                .as_ref()
                .and_then(|customer| customer.customer_name.clone()),
        );
        let shipping = build_shipping(&item.router_data.resource_common_data);
        let (mut processing, items) = if let Some(l2l3_data) = l2l3_data {
            (
                l2l3_data.order_info.as_ref().map(|_| CheckoutProcessing {
                    order_id: l2l3_data.get_merchant_order_reference_id(),
                    scheme_transaction_link_id: None,
                    tax_amount: l2l3_data.get_order_tax_amount(),
                    discount_amount: l2l3_data.get_discount_amount(),
                    duty_amount: l2l3_data.get_duty_amount(),
                    shipping_amount: l2l3_data.get_shipping_cost(),
                    shipping_tax_amount: l2l3_data.get_shipping_amount_tax(),
                    aft: None,
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
            (None, None)
        };

        if let Some(link_id) = match &item.router_data.request.mandate_reference {
            MandateReferenceId::NetworkMandateId(ref_data) => ref_data.transaction_link_id.clone(),
            MandateReferenceId::NetworkTokenWithNTI(ref_data) => {
                ref_data.transaction_link_id.clone()
            }
            _ => None,
        } {
            processing
                .get_or_insert_with(CheckoutProcessing::default)
                .scheme_transaction_link_id = Some(link_id);
        }

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

        let billing_descriptor = item
            .router_data
            .request
            .billing_descriptor
            .as_ref()
            .and_then(|descriptor| {
                (descriptor.name.is_some()
                    || descriptor.city.is_some()
                    || descriptor.reference.is_some())
                .then(|| CheckoutBillingDescriptor {
                    name: descriptor.name.clone(),
                    city: descriptor.city.clone(),
                    reference: descriptor.reference.clone(),
                })
            });

        let is_amex = matches!(
            &item.router_data.request.payment_method_data,
            PaymentMethodData::Card(ccard) if is_amex_card(ccard.card_number.peek())
        );
        let reference = get_checkout_reference(
            &item
                .router_data
                .resource_common_data
                .connector_request_reference_id,
            is_amex,
        );

        let request = Self {
            source: source_var,
            amount: item.router_data.request.amount.amount,
            currency: item.router_data.request.currency.to_string(),
            processing_channel_id,
            three_ds,
            return_url,
            capture,
            reference,
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
        // SetupMandate always sends capture:true for its $0 verification, but its response
        // handler cannot represent Scheduled: refuse it before the call, on every payment-method
        // arm, so Checkout never stores a src_ the response path would then lose (G-SetupMandate-02).
        checkout_capture_flag(item.router_data.request.capture_method)?;
        let payment_type = if matches!(
            item.router_data.request.payment_channel,
            Some(
                common_enums::PaymentChannel::MailOrder
                    | common_enums::PaymentChannel::TelephoneOrder
            )
        ) {
            CheckoutPaymentType::Moto
        } else {
            match item.router_data.request.mit_category {
                Some(common_enums::MitCategory::Installment) => CheckoutPaymentType::Installment,
                Some(
                    common_enums::MitCategory::Recurring | common_enums::MitCategory::Subscription,
                ) => CheckoutPaymentType::Recurring,
                Some(
                    common_enums::MitCategory::Unscheduled
                    | common_enums::MitCategory::Resubmission,
                )
                | None => CheckoutPaymentType::Unscheduled,
            }
        };

        let billing_details = build_billing_address(&item.router_data.resource_common_data);

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
                    phone: build_source_phone(&item.router_data.resource_common_data),
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

        // Runs after the source match, so the external-3DS outcome guard (trans_status x ECI,
        // see `validate_external_three_ds`) refuses on every accepted arm — card, decrypted
        // Google Pay / Apple Pay, Checkout token and ACH — before any request is built.
        let three_ds = build_checkout_three_ds(
            item.router_data.resource_common_data.auth_type,
            item.router_data.request.authentication_data.as_ref(),
            true,
        )?;

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

        let l2l3_data = item.router_data.resource_common_data.l2_l3_data.as_deref();
        let customer = build_customer(
            l2l3_data,
            item.router_data.request.email.clone(),
            item.router_data
                .request
                .customer_name
                .clone()
                .map(Secret::new),
        );
        let shipping = build_shipping(&item.router_data.resource_common_data);
        let (mut processing, items) = if let Some(l2l3_data) = l2l3_data {
            (
                l2l3_data.order_info.as_ref().map(|_| CheckoutProcessing {
                    order_id: l2l3_data.get_merchant_order_reference_id(),
                    scheme_transaction_link_id: None,
                    tax_amount: l2l3_data.get_order_tax_amount(),
                    discount_amount: l2l3_data.get_discount_amount(),
                    duty_amount: l2l3_data.get_duty_amount(),
                    shipping_amount: l2l3_data.get_shipping_cost(),
                    shipping_tax_amount: l2l3_data.get_shipping_amount_tax(),
                    aft: None,
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
            (None, None)
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

        let billing_descriptor = item
            .router_data
            .request
            .billing_descriptor
            .as_ref()
            .and_then(|descriptor| {
                (descriptor.name.is_some()
                    || descriptor.city.is_some()
                    || descriptor.reference.is_some())
                .then(|| CheckoutBillingDescriptor {
                    name: descriptor.name.clone(),
                    city: descriptor.city.clone(),
                    reference: descriptor.reference.clone(),
                })
            });

        let is_amex = matches!(
            &item.router_data.request.payment_method_data,
            PaymentMethodData::Card(ccard) if is_amex_card(ccard.card_number.peek())
        );
        let reference = get_checkout_reference(
            &item
                .router_data
                .resource_common_data
                .connector_request_reference_id,
            is_amex,
        );

        let request = Self {
            source: source_var,
            amount: MinorUnit::new(0),
            currency: item.router_data.request.currency.to_string(),
            processing_channel_id,
            three_ds,
            return_url,
            capture: true,
            reference,
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
    Paid,
    #[serde(other)]
    Unknown,
}

/// The `ErrorResponse` for a `2xx` payment body whose status maps to `Failure` (`Declined`,
/// `Expired`, `Canceled`). Shared by Authorize, PSync, SetupMandate and RepeatPayment.
///
/// Carries both scheme codes a retry engine needs: the Mastercard Merchant Advice Code
/// (`processing.partner_merchant_advice_code`) as `network_advice_code`, and the raw scheme
/// response code (`processing.partner_response_code`) as `network_decline_code`.
fn build_declined_payment_error(response: &PaymentsResponse, http_code: u16) -> ErrorResponse {
    let processing = response.processing.as_ref();
    ErrorResponse {
        status_code: http_code,
        code: response
            .response_code
            .clone()
            .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
        message: response
            .response_summary
            .clone()
            .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
        reason: response.response_summary.clone(),
        attempt_status: Some(domain_types::router_data::FlowStatus::Payment(
            common_enums::AttemptStatus::Failure,
        )),
        connector_transaction_id: Some(response.id.clone()),
        network_advice_code: processing
            .and_then(|processing| processing.partner_merchant_advice_code.clone()),
        network_decline_code: processing
            .and_then(|processing| processing.partner_response_code.clone()),
        network_error_message: None,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
}

fn get_attempt_status_cap(
    item: (CheckoutPaymentStatus, Option<common_enums::CaptureMethod>),
) -> common_enums::AttemptStatus {
    let (status, capture_method) = item;
    match status {
        CheckoutPaymentStatus::Authorized => {
            if matches!(
                capture_method,
                None | Some(
                    common_enums::CaptureMethod::Automatic
                        | common_enums::CaptureMethod::SequentialAutomatic
                )
            ) {
                common_enums::AttemptStatus::Charged
            } else {
                common_enums::AttemptStatus::Authorized
            }
        }
        CheckoutPaymentStatus::Captured
        | CheckoutPaymentStatus::Paid
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
        CheckoutPaymentStatus::Unknown => common_enums::AttemptStatus::Unspecified,
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
        | CheckoutPaymentStatus::Paid
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
        CheckoutPaymentStatus::Unknown => common_enums::AttemptStatus::Unspecified,
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
        | CheckoutPaymentStatus::Paid
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
        CheckoutPaymentStatus::Unknown => common_enums::AttemptStatus::Unspecified,
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
    /// The card's funding type
    card_type: Option<CheckoutCardType>,
    /// The card's category
    card_category: Option<CheckoutCardCategory>,
    /// The name of the card issuer
    issuer: Option<String>,
    /// The country of the card issuer
    issuer_country: Option<common_enums::CountryAlpha2>,
    /// The card's product/subtype, e.g. "Visa Classic"
    product_type: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum CheckoutCardType {
    Credit,
    Debit,
    Prepaid,
    Charge,
    #[serde(rename = "DEFERRED DEBIT")]
    DeferredDebit,
}

impl From<CheckoutCardType> for common_enums::FundingSource {
    fn from(card_type: CheckoutCardType) -> Self {
        match card_type {
            CheckoutCardType::Credit => Self::Credit,
            CheckoutCardType::Debit => Self::Debit,
            CheckoutCardType::Prepaid => Self::Prepaid,
            CheckoutCardType::Charge => Self::ChargeCard,
            CheckoutCardType::DeferredDebit => Self::DeferredDebit,
        }
    }
}

impl From<CheckoutCardType> for common_enums::CardType {
    fn from(card_type: CheckoutCardType) -> Self {
        match card_type {
            CheckoutCardType::Credit => Self::Credit,
            CheckoutCardType::Debit | CheckoutCardType::DeferredDebit => Self::Debit,
            CheckoutCardType::Prepaid => Self::Prepaid,
            CheckoutCardType::Charge => Self::ChargeCard,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum CheckoutCardCategory {
    Consumer,
    Commercial,
}

impl From<CheckoutCardCategory> for common_enums::CardSegmentType {
    fn from(card_category: CheckoutCardCategory) -> Self {
        match card_category {
            CheckoutCardCategory::Consumer => Self::Consumer,
            CheckoutCardCategory::Commercial => Self::Commercial,
        }
    }
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
    auth_code: Option<String>,
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
    TryFrom<ResponseRouterData<PaymentsResponse, CheckoutRouterData<Self, T>>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PaymentsResponse, CheckoutRouterData<Self, T>>,
    ) -> Result<Self, Self::Error> {
        let connector = item.router_data.connector;
        let item = ResponseRouterData {
            response: item.response,
            router_data: item.router_data.router_data,
            http_code: item.http_code,
        };
        // Same reading of capture_method as the request's `capture` flag: absent = Automatic.
        let status = get_attempt_status_cap((
            item.response.status.clone(),
            Some(item.router_data.request.capture_method.unwrap_or_default()),
        ));

        if status == common_enums::AttemptStatus::Failure {
            let error_response = build_declined_payment_error(&item.response, item.http_code);

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

        let additional_information = convert_to_additional_payment_method_connector_response(
            item.response.source.as_ref(),
            item.response.auth_code.clone(),
            item.router_data.request.payment_method_type,
        )
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

        let currency = item.router_data.request.currency;
        let to_money = |amount| {
            crate::utils::response_amount_to_money(
                &connector,
                amount,
                currency,
                item.http_code,
                domain_types::connector_types::ConnectorEnum::Checkout,
            )
        };
        let (amount_captured, amount_capturable) = match item.router_data.request.capture_method {
            Some(common_enums::CaptureMethod::Manual)
            | Some(common_enums::CaptureMethod::ManualMultiple) => {
                (None, item.response.amount.map(to_money).transpose()?)
            }
            _ => (item.response.amount.map(to_money).transpose()?, None),
        };

        let amount_authorized = item
            .router_data
            .request
            .enable_partial_authorization
            .filter(|flag| *flag)
            .and(item.response.amount)
            .map(to_money)
            .transpose()?;

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_response: additional_information,
                amount_authorized,
                amount_captured,
                amount_capturable,
                ..item.router_data.resource_common_data
            },
            response: Ok(payments_response_data),
            ..item.router_data
        })
    }
}

impl<
        T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize + Serialize,
    > TryFrom<ResponseRouterData<PaymentsResponse, CheckoutRouterData<Self, T>>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PaymentsResponse, CheckoutRouterData<Self, T>>,
    ) -> Result<Self, Self::Error> {
        let connector = item.router_data.connector;
        let item = ResponseRouterData {
            response: item.response,
            router_data: item.router_data.router_data,
            http_code: item.http_code,
        };
        let status = get_attempt_status_cap((
            item.response.status.clone(),
            item.router_data.request.capture_method,
        ));

        match status {
            common_enums::AttemptStatus::Failure => {
                let error_response = build_declined_payment_error(&item.response, item.http_code);

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
                        item.response.auth_code.clone(),
                        item.router_data.request.payment_method_type,
                    )
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
                        .and_then(|processing| processing.scheme_transaction_link_id),
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

                let currency = item.router_data.request.currency;
                let to_money = |amount| {
                    crate::utils::response_amount_to_money(
                        &connector,
                        amount,
                        currency,
                        item.http_code,
                        domain_types::connector_types::ConnectorEnum::Checkout,
                    )
                };
                let (amount_captured, amount_capturable) =
                    match item.router_data.request.capture_method {
                        Some(common_enums::CaptureMethod::Manual)
                        | Some(common_enums::CaptureMethod::ManualMultiple) => {
                            (None, item.response.amount.map(to_money).transpose()?)
                        }
                        _ => (item.response.amount.map(to_money).transpose()?, None),
                    };

                let amount_authorized = item
                    .router_data
                    .request
                    .enable_partial_authorization
                    .filter(|flag| *flag)
                    .and(item.response.amount)
                    .map(to_money)
                    .transpose()?;

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        connector_response: additional_information,
                        amount_authorized,
                        amount_captured,
                        amount_capturable,
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
        let status = get_attempt_status_cap((
            item.response.status.clone(),
            item.router_data.request.capture_method,
        ));
        // A 2xx `Declined` $0 verification is a hard failure: same error shape as Authorize, so the
        // Merchant Advice Code is the only advice code and the scheme code is the decline code.
        let error_response = (status == common_enums::AttemptStatus::Failure)
            .then(|| build_declined_payment_error(&item.response, item.http_code));
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
        let additional_information = convert_to_additional_payment_method_connector_response(
            item.response.source.as_ref(),
            item.response.auth_code.clone(),
            item.router_data.request.payment_method_type,
        )
        .map(ConnectorResponseData::with_additional_payment_method_data);
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

impl<F> TryFrom<ResponseRouterData<PaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<PaymentsResponse, Self>) -> Result<Self, Self::Error> {
        let redirection_data = item
            .response
            .links
            .redirect
            .as_ref()
            .map(|href| RedirectForm::from((href.redirection_url.clone(), Method::Get)));

        // Authorize writes {psync_flow} from the capture method and an accepted Capture
        // overwrites it with Capture. Checkout keeps reporting `Authorized` until the
        // asynchronous capture settles, so the flow the connector recorded is the authority
        // (HS parity, HP-30); the capture method is read only when nothing usable was stored.
        let stored_meta = item
            .router_data
            .request
            .connector_feature_data
            .as_ref()
            .and_then(|data| serde_json::from_value::<CheckoutMeta>(data.peek().clone()).ok());
        let checkout_meta = match stored_meta {
            Some(meta) => meta,
            None => match item.router_data.request.capture_method {
                // The same capture rule every `POST /payments` builder sends, so the synced
                // status reads the capture flag the payment was created with.
                Some(capture_method) => match checkout_capture_flag(Some(capture_method)) {
                    Ok(true) => CheckoutMeta {
                        psync_flow: CheckoutPaymentIntent::Capture,
                    },
                    Ok(false) => CheckoutMeta {
                        psync_flow: CheckoutPaymentIntent::Authorize,
                    },
                    // Scheduled: refused before any call, so no Checkout payment can carry it.
                    Err(_) => {
                        return Err(crate::utils::unexpected_response_fail(
                            item.http_code,
                            "checkout: unexpected response for this operation; retry with idempotency keys and check connector status.",
                        )
                        .into());
                    }
                },
                None => {
                    return Err(ConnectorError::response_handling_failed_with_context(
                        item.http_code,
                        Some("Checkout PSync: capture_method absent on payment intent".to_string()),
                    )
                    .into());
                }
            },
        };

        let status =
            get_attempt_status_intent((item.response.status.clone(), checkout_meta.psync_flow));
        let error_response = if status == common_enums::AttemptStatus::Failure {
            Some(build_declined_payment_error(&item.response, item.http_code))
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

        let additional_information = convert_to_additional_payment_method_connector_response(
            item.response.source.as_ref(),
            item.response.auth_code.clone(),
            item.router_data.request.payment_method_type,
        )
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
            // GET /payments/{id}: the single-payment sync mapping (status, mandate, errors).
            PaymentsResponseEnum::PaymentResponse(payments_response) => {
                return Self::try_from(ResponseRouterData {
                    response: *payments_response,
                    router_data: item.router_data,
                    http_code: item.http_code,
                });
            }
            // GET /payments/{id}/actions (MultipleCaptureSync): one entry per action.
            PaymentsResponseEnum::ActionResponse(action_list) => {
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
    reference: Option<String>,
    scheme_id: Option<String>,
}

/// Status for a `2xx` from `POST /payments/{id}/voids`.
///
/// 202 = void accepted; 200 = "Payment is already voided" (Checkout's idempotent success for a
/// repeated void). Both mean the authorization is voided. Any other 2xx is undocumented for
/// `/voids` and stays `Pending` until PSync confirms — never `VoidFailed`, which would report a
/// failure the connector did not state. Non-2xx responses go through `build_error_response`.
fn http_code_to_attempt_status_for_void_flow(http_code: u16) -> common_enums::AttemptStatus {
    if matches!(http_code, 200 | 202) {
        common_enums::AttemptStatus::Voided
    } else {
        common_enums::AttemptStatus::Pending
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
                // Keep the payment id (pay_...) as the transaction id so later PSync/Refund calls
                // still address the payment; the void's own action id (act_...) is reported as
                // the connector response reference.
                resource_id: ResponseId::ConnectorTransactionId(
                    item.router_data.request.connector_transaction_id.clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: response.scheme_id.clone(),
                network_txn_link_id: None,
                connector_response_reference_id: Some(response.action_id.clone()),
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

/// Body of `POST /disputes/{dispute_id}/accept`: the endpoint takes no fields, so this
/// serialises to `{}`.
#[derive(Debug, Default, Serialize)]
pub struct CheckoutDisputeEmptyRequest {}

/// Body of a dispute action answered by `204 No Content`. The macro JSON bridge parses the empty
/// body as `{}`, which this empty struct accepts.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct CheckoutDisputeActionResponse {}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        CheckoutRouterData<
            RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
            T,
        >,
    > for CheckoutDisputeEmptyRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        _item: CheckoutRouterData<
            RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

/// `204` from `POST /disputes/{id}/accept` is Checkout's only documented success and means the
/// dispute is accepted. Any other `2xx` is undocumented for this endpoint, so it is reported as
/// an error rather than claiming an acceptance the connector did not state. Non-`2xx` responses
/// (404 dispute not found, 422 dispute already actioned) go through `build_error_response`.
impl TryFrom<ResponseRouterData<CheckoutDisputeActionResponse, Self>>
    for RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<CheckoutDisputeActionResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response: _,
            router_data,
            http_code,
        } = item;
        let response = if http_code == 204 {
            Ok(DisputeResponseData {
                connector_dispute_id: router_data.request.connector_dispute_id.clone(),
                dispute_status: common_enums::DisputeStatus::DisputeAccepted,
                connector_dispute_status: None,
                status_code: http_code,
            })
        } else {
            Err(ErrorResponse {
                status_code: http_code,
                code: NO_ERROR_CODE.to_string(),
                message: NO_ERROR_MESSAGE.to_string(),
                reason: Some(format!(
                    "Checkout answered dispute accept with undocumented HTTP {http_code}; only 204 confirms acceptance"
                )),
                attempt_status: None,
                connector_transaction_id: None,
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            })
        };
        Ok(Self {
            response,
            ..router_data
        })
    }
}

const CHECKOUT_DISPUTE_EVIDENCE_DOC_URL: &str =
    "https://www.checkout.com/docs/payments/process-disputes/disputes-api";

/// Body of `PUT /disputes/{dispute_id}/evidence` (Checkout `ProvideEvidenceRequest`).
///
/// Every `*_file` field is a Checkout file id (`file_...`) obtained from `POST /files` with
/// `purpose: dispute_evidence`; UCS has no file-upload flow, so the caller uploads the file and
/// passes the id as the matching `*_provider_file_id`. `*_text` fields are free text (max 500
/// characters per the vendor schema).
#[skip_serializing_none]
#[derive(Debug, Serialize)]
pub struct CheckoutEvidenceRequest {
    pub proof_of_delivery_or_service_file: Option<String>,
    pub invoice_or_receipt_file: Option<String>,
    pub invoice_showing_distinct_transactions_file: Option<String>,
    pub customer_communication_file: Option<String>,
    pub refund_or_cancellation_policy_file: Option<String>,
    pub recurring_transaction_agreement_file: Option<String>,
    pub additional_evidence_file: Option<String>,
    pub refund_or_cancellation_policy_text: Option<String>,
    pub additional_evidence_text: Option<String>,
}

/// `Some` only for a non-empty string, so an empty id or text is never put on the wire.
fn non_empty_evidence(value: &Option<String>) -> Option<String> {
    value.as_ref().filter(|value| !value.is_empty()).cloned()
}

/// Checkout has one evidence field per category. An input with no Checkout field of its own
/// (`fallback`) fills the closest category when the `primary` input is absent; when both are
/// supplied the request is refused rather than silently dropping one of them.
fn pick_evidence(
    primary: (&'static str, &Option<String>),
    fallback: (&'static str, &Option<String>),
    checkout_field: &'static str,
) -> Result<Option<String>, error_stack::Report<IntegrationError>> {
    let (primary_name, primary_value) = primary;
    let (fallback_name, fallback_value) = fallback;
    match (
        non_empty_evidence(primary_value),
        non_empty_evidence(fallback_value),
    ) {
        (Some(_), Some(_)) => Err(error_stack::report!(IntegrationError::NotSupported {
            message: format!(
                "Both {primary_name} and {fallback_name} map to Checkout evidence field \
                 {checkout_field}; {fallback_name} would be dropped"
            ),
            connector: "checkout",
            context: IntegrationErrorContext {
                suggested_action: Some(format!(
                    "Send only one of {primary_name} / {fallback_name}, or merge them into a \
                     single document before upload"
                )),
                doc_url: Some(CHECKOUT_DISPUTE_EVIDENCE_DOC_URL.to_owned()),
                additional_context: Some(format!(
                    "Checkout accepts a single {checkout_field} per evidence submission"
                )),
            },
        })),
        (primary_value, fallback_value) => Ok(primary_value.or(fallback_value)),
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        CheckoutRouterData<
            RouterDataV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>,
            T,
        >,
    > for CheckoutEvidenceRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: CheckoutRouterData<
            RouterDataV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request = &item.router_data.request;

        // Raw evidence bytes cannot be sent: Checkout only accepts `file_...` ids from
        // `POST /files`, which UCS does not call. Refuse rather than silently submit evidence
        // that is missing a document the caller provided.
        let file_inputs = [
            (
                "cancellation_policy",
                &request.cancellation_policy,
                &request.cancellation_policy_provider_file_id,
            ),
            (
                "customer_communication",
                &request.customer_communication,
                &request.customer_communication_provider_file_id,
            ),
            (
                "customer_signature",
                &request.customer_signature,
                &request.customer_signature_provider_file_id,
            ),
            (
                "receipt",
                &request.receipt,
                &request.receipt_provider_file_id,
            ),
            (
                "refund_policy",
                &request.refund_policy,
                &request.refund_policy_provider_file_id,
            ),
            (
                "service_documentation",
                &request.service_documentation,
                &request.service_documentation_provider_file_id,
            ),
            (
                "shipping_documentation",
                &request.shipping_documentation,
                &request.shipping_documentation_provider_file_id,
            ),
            (
                "invoice_showing_distinct_transactions",
                &request.invoice_showing_distinct_transactions,
                &request.invoice_showing_distinct_transactions_provider_file_id,
            ),
            (
                "recurring_transaction_agreement",
                &request.recurring_transaction_agreement,
                &request.recurring_transaction_agreement_provider_file_id,
            ),
            (
                "uncategorized_file",
                &request.uncategorized_file,
                &request.uncategorized_file_provider_file_id,
            ),
        ];
        let bytes_without_file_id: Vec<&str> = file_inputs
            .iter()
            .filter(|(_, bytes, file_id)| {
                bytes.as_ref().is_some_and(|bytes| !bytes.is_empty())
                    && non_empty_evidence(file_id).is_none()
            })
            .map(|(name, _, _)| *name)
            .collect();
        if !bytes_without_file_id.is_empty() {
            return Err(error_stack::report!(IntegrationError::NotSupported {
                message: format!(
                    "Raw evidence file bytes ({}) without a Checkout file id",
                    bytes_without_file_id.join(", ")
                ),
                connector: "checkout",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Upload the file via Checkout POST /files (purpose dispute_evidence) and \
                         pass the returned file_ id as the matching *_provider_file_id"
                            .to_owned(),
                    ),
                    doc_url: Some(CHECKOUT_DISPUTE_EVIDENCE_DOC_URL.to_owned()),
                    additional_context: Some(
                        "Checkout dispute evidence accepts only file ids, never file contents"
                            .to_owned(),
                    ),
                },
            }));
        }

        // Every supplied evidence id reaches the body or is refused: service_documentation,
        // cancellation_policy and customer_signature have no Checkout field of their own and
        // fill the closest category when it is free (pick_evidence refuses a collision).
        let evidence = Self {
            proof_of_delivery_or_service_file: pick_evidence(
                (
                    "shipping_documentation_provider_file_id",
                    &request.shipping_documentation_provider_file_id,
                ),
                (
                    "service_documentation_provider_file_id",
                    &request.service_documentation_provider_file_id,
                ),
                "proof_of_delivery_or_service_file",
            )?,
            invoice_or_receipt_file: non_empty_evidence(&request.receipt_provider_file_id),
            invoice_showing_distinct_transactions_file: non_empty_evidence(
                &request.invoice_showing_distinct_transactions_provider_file_id,
            ),
            customer_communication_file: non_empty_evidence(
                &request.customer_communication_provider_file_id,
            ),
            refund_or_cancellation_policy_file: pick_evidence(
                (
                    "refund_policy_provider_file_id",
                    &request.refund_policy_provider_file_id,
                ),
                (
                    "cancellation_policy_provider_file_id",
                    &request.cancellation_policy_provider_file_id,
                ),
                "refund_or_cancellation_policy_file",
            )?,
            recurring_transaction_agreement_file: non_empty_evidence(
                &request.recurring_transaction_agreement_provider_file_id,
            ),
            additional_evidence_file: pick_evidence(
                (
                    "uncategorized_file_provider_file_id",
                    &request.uncategorized_file_provider_file_id,
                ),
                (
                    "customer_signature_provider_file_id",
                    &request.customer_signature_provider_file_id,
                ),
                "additional_evidence_file",
            )?,
            refund_or_cancellation_policy_text: pick_evidence(
                (
                    "refund_policy_disclosure",
                    &request.refund_policy_disclosure,
                ),
                (
                    "cancellation_policy_disclosure",
                    &request.cancellation_policy_disclosure,
                ),
                "refund_or_cancellation_policy_text",
            )?,
            additional_evidence_text: non_empty_evidence(&request.uncategorized_text),
        };

        // Checkout answers `422 no_evidence_provided` to an empty body; refuse before the call.
        let has_evidence = [
            &evidence.proof_of_delivery_or_service_file,
            &evidence.invoice_or_receipt_file,
            &evidence.invoice_showing_distinct_transactions_file,
            &evidence.customer_communication_file,
            &evidence.refund_or_cancellation_policy_file,
            &evidence.recurring_transaction_agreement_file,
            &evidence.additional_evidence_file,
            &evidence.refund_or_cancellation_policy_text,
            &evidence.additional_evidence_text,
        ]
        .iter()
        .any(|field| field.is_some());
        if !has_evidence {
            return Err(error_stack::report!(
                IntegrationError::MissingRequiredField {
                    field_name: "evidence",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Provide at least one of shipping_documentation, \
                         service_documentation, receipt, invoice_showing_distinct_transactions, \
                         customer_communication, refund_policy, cancellation_policy, \
                         recurring_transaction_agreement, uncategorized_file or \
                         customer_signature provider file ids, or refund_policy_disclosure / \
                         cancellation_policy_disclosure / uncategorized_text"
                                .to_owned(),
                        ),
                        doc_url: Some(CHECKOUT_DISPUTE_EVIDENCE_DOC_URL.to_owned()),
                        additional_context: Some(
                            "Checkout rejects an evidence request with no evidence \
                         (422 no_evidence_provided)"
                                .to_owned(),
                        ),
                    },
                }
            ));
        }

        Ok(evidence)
    }
}

/// `204` from `PUT /disputes/{id}/evidence` is Checkout's only documented success and means the
/// evidence is attached, so the dispute is being challenged. Any other `2xx` is undocumented for
/// this endpoint and is reported as an error. Non-`2xx` responses (404 dispute not found,
/// 422 `no_evidence_provided` / `dispute_already_*`) go through `build_error_response`.
impl TryFrom<ResponseRouterData<CheckoutDisputeActionResponse, Self>>
    for RouterDataV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<CheckoutDisputeActionResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response: _,
            router_data,
            http_code,
        } = item;
        let response = if http_code == 204 {
            Ok(DisputeResponseData {
                connector_dispute_id: router_data.request.connector_dispute_id.clone(),
                dispute_status: common_enums::DisputeStatus::DisputeChallenged,
                connector_dispute_status: None,
                status_code: http_code,
            })
        } else {
            Err(ErrorResponse {
                status_code: http_code,
                code: NO_ERROR_CODE.to_string(),
                message: NO_ERROR_MESSAGE.to_string(),
                reason: Some(format!(
                    "Checkout answered dispute evidence with undocumented HTTP {http_code}; only 204 confirms the evidence was attached"
                )),
                attempt_status: None,
                connector_transaction_id: None,
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            })
        };
        Ok(Self {
            response,
            ..router_data
        })
    }
}

/// Body of `POST /disputes/{dispute_id}/evidence` (submit the attached evidence to the scheme):
/// the endpoint takes no fields, so `defense_reason_code` has nowhere to go and the body is `{}`.
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        CheckoutRouterData<
            RouterDataV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>,
            T,
        >,
    > for CheckoutDisputeEmptyRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        _item: CheckoutRouterData<
            RouterDataV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

/// `204` from `POST /disputes/{id}/evidence` is Checkout's only documented success and means the
/// attached evidence was submitted to the scheme, so the dispute is challenged. Any other `2xx`
/// is undocumented for this endpoint and is reported as an error. Non-`2xx` responses
/// (404 dispute not found, 422 `dispute_already_*` / no evidence attached) go through
/// `build_error_response`.
impl TryFrom<ResponseRouterData<CheckoutDisputeActionResponse, Self>>
    for RouterDataV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<CheckoutDisputeActionResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response: _,
            router_data,
            http_code,
        } = item;
        let response = if http_code == 204 {
            Ok(DisputeResponseData {
                connector_dispute_id: router_data.request.connector_dispute_id.clone(),
                dispute_status: common_enums::DisputeStatus::DisputeChallenged,
                connector_dispute_status: None,
                status_code: http_code,
            })
        } else {
            Err(ErrorResponse {
                status_code: http_code,
                code: NO_ERROR_CODE.to_string(),
                message: NO_ERROR_MESSAGE.to_string(),
                reason: Some(format!(
                    "Checkout answered dispute evidence submission with undocumented HTTP {http_code}; only 204 confirms the evidence was submitted"
                )),
                attempt_status: None,
                connector_transaction_id: None,
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            })
        };
        Ok(Self {
            response,
            ..router_data
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
            amount: Some(item.router_data.request.amount_to_capture.amount.to_owned()),
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
        // 202 = capture accepted; 200 = "Payment is already captured" (Checkout's idempotent
        // success for a repeated capture). Both mean the funds are captured. Any other 2xx is
        // undocumented for /captures and stays Pending until PSync confirms.
        let (status, amount_captured) = if matches!(item.http_code, 200 | 202) {
            (
                common_enums::AttemptStatus::Charged,
                Some(common_utils::types::Money {
                    amount: item.router_data.request.amount_to_capture.amount,
                    currency: item.router_data.request.currency,
                }),
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
            amount: Some(item.router_data.request.refund_amount.amount.to_owned()),
            reference,
        })
    }
}

#[derive(Deserialize, Debug, Serialize)]
pub struct RefundResponse {
    action_id: String,
    reference: Option<String>,
}

impl<F> TryFrom<ResponseRouterData<RefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<RefundResponse, Self>) -> Result<Self, Self::Error> {
        // Checkout refunds are asynchronous: "The response does not confirm success." Both the
        // 202 (refund accepted) and the 200 (payment already refunded) answers carry only an
        // `action_id`, so the refund stays Pending here; the final outcome comes from RSync
        // (`GET /payments/{id}/actions`, Refund action `approved`) or the `payment_refunded` /
        // `payment_refund_declined` webhooks. Mapping 202 to Success would report a refund the
        // issuer can still decline (sandbox amount 3738). Non-2xx answers take the error path.
        // This intentionally diverges from Hyperswitch, which maps 202 to Success.
        let refund_status = common_enums::RefundStatus::Pending;
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
    #[serde(other)]
    Unknown,
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
    #[serde(other)]
    Unknown,
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
        // Refunds are asynchronous: until Checkout lists the refund action (only
        // `payment_refund_pending` seen so far), the refund is still in flight, so
        // an absent action is Pending rather than a response-handling failure.
        let refund_status = item
            .response
            .iter()
            .find(|action| action.action_id == refund_action_id)
            .map(common_enums::RefundStatus::from)
            .unwrap_or(common_enums::RefundStatus::Pending);
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: refund_action_id,
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
            CheckoutRedirectResponseStatus::Unknown => Self::Unspecified,
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
    auth_code: Option<String>,
    payment_method_type: Option<common_enums::PaymentMethodType>,
) -> Option<AdditionalPaymentMethodConnectorResponse> {
    match payment_method_type {
        Some(common_enums::PaymentMethodType::GooglePay) => {
            Some(AdditionalPaymentMethodConnectorResponse::GooglePay {
                auth_code,
                device_pan_bin: None,
                card_bin: None,
                card_subtype: source.and_then(|source| source.product_type.clone()),
                card_segment_type: source
                    .and_then(|source| source.card_category.clone())
                    .map(common_enums::CardSegmentType::from),
                funding_source: source
                    .and_then(|source| source.card_type.clone())
                    .map(common_enums::FundingSource::from),
                card_type: source
                    .and_then(|source| source.card_type.as_ref())
                    .map(|card_type| common_enums::CardType::from(card_type.clone())),
                issuer_name: source.and_then(|source| source.issuer.clone()),
                issuer_country: source.and_then(|source| source.issuer_country),
            })
        }
        Some(common_enums::PaymentMethodType::ApplePay) => {
            Some(AdditionalPaymentMethodConnectorResponse::ApplePay {
                auth_code,
                device_pan_bin: None,
                card_bin: None,
                card_subtype: source.and_then(|source| source.product_type.clone()),
                card_segment_type: source
                    .and_then(|source| source.card_category.clone())
                    .map(common_enums::CardSegmentType::from),
                funding_source: source
                    .and_then(|source| source.card_type.clone())
                    .map(common_enums::FundingSource::from),
                issuer_name: source.and_then(|source| source.issuer.clone()),
                issuer_country: source.and_then(|source| source.issuer_country),
            })
        }
        _ => {
            let payment_checks = source.map(|code| {
                serde_json::json!({
                    "avs_result": code.avs_check,
                    "card_validation_result": code.cvv_check,
                })
            });

            Some(AdditionalPaymentMethodConnectorResponse::Card {
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
            })
        }
    }
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

// ─── Incoming webhook types ────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum CheckoutWebhookEventType {
    AuthenticationStarted,
    AuthenticationApproved,
    AuthenticationAttempted,
    AuthenticationExpired,
    AuthenticationFailed,
    PaymentApproved,
    PaymentCaptured,
    PaymentDeclined,
    PaymentRefunded,
    PaymentRefundDeclined,
    PaymentAuthenticationFailed,
    PaymentCanceled,
    PaymentCaptureDeclined,
    PaymentVoided,
    PaymentPending,
    PaymentCapturePending,
    PaymentRetryScheduled,
    PaymentExpired,
    PaymentVoidDeclined,
    PaymentRefundPending,
    CardVerified,
    CardVerificationDeclined,
    DisputeReceived,
    DisputeExpired,
    DisputeAccepted,
    DisputeCanceled,
    DisputeEvidenceSubmitted,
    DisputeEvidenceAcknowledgedByScheme,
    DisputeEvidenceRequired,
    DisputeArbitrationEvidenceSubmitted,
    DisputeArbitrationSentToScheme,
    DisputeArbitrationLost,
    DisputeArbitrationWon,
    DisputeWon,
    DisputeLost,
    #[serde(other)]
    Unknown,
}

/// Thin body used by `get_event_type` — deserialises only the `type` field so we
/// avoid pulling in the full data payload during the stateless ParseEvent phase.
#[derive(Debug, Deserialize)]
pub struct CheckoutWebhookEventTypeBody {
    #[serde(rename = "type")]
    pub transaction_type: CheckoutWebhookEventType,
}

#[derive(Debug, Deserialize)]
pub struct CheckoutWebhookData {
    pub id: String,
    pub payment_id: Option<String>,
    pub action_id: Option<String>,
    pub reference: Option<String>,
    pub amount: MinorUnit,
    pub balances: Option<Balances>,
    pub response_code: Option<String>,
    pub response_summary: Option<String>,
    pub auth_code: Option<String>,
    /// Scheme transaction identifier (network transaction id) carried by
    /// `payment_approved` / `payment_declined` / `card_verified` events.
    pub scheme_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CheckoutWebhookBody {
    #[serde(rename = "type")]
    pub transaction_type: CheckoutWebhookEventType,
    pub data: CheckoutWebhookData,
    /// Not read; optional so a body without `_links` still decodes.
    #[serde(rename = "_links", default)]
    pub links: Option<Links>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum CheckoutDisputeTransactionType {
    DisputeReceived,
    DisputeExpired,
    DisputeAccepted,
    DisputeCanceled,
    DisputeEvidenceSubmitted,
    DisputeEvidenceAcknowledgedByScheme,
    DisputeEvidenceRequired,
    DisputeArbitrationEvidenceSubmitted,
    DisputeArbitrationSentToScheme,
    DisputeArbitrationLost,
    DisputeArbitrationWon,
    DisputeWon,
    DisputeLost,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
pub struct CheckoutDisputeWebhookData {
    pub id: String,
    pub payment_id: Option<String>,
    pub amount: Option<MinorUnit>,
    pub currency: Option<common_enums::Currency>,
    pub reason_code: Option<String>,
    /// Dispute category (e.g. `fraudulent`), surfaced as the dispute message.
    pub category: Option<String>,
    pub evidence_required_by: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CheckoutDisputeWebhookBody {
    #[serde(rename = "type")]
    pub transaction_type: CheckoutDisputeTransactionType,
    pub data: CheckoutDisputeWebhookData,
}

pub fn is_refund_event(event_code: &CheckoutWebhookEventType) -> bool {
    matches!(
        event_code,
        CheckoutWebhookEventType::PaymentRefunded
            | CheckoutWebhookEventType::PaymentRefundDeclined
            | CheckoutWebhookEventType::PaymentRefundPending
    )
}

pub fn is_chargeback_event(event_code: &CheckoutWebhookEventType) -> bool {
    matches!(
        event_code,
        CheckoutWebhookEventType::DisputeReceived
            | CheckoutWebhookEventType::DisputeExpired
            | CheckoutWebhookEventType::DisputeAccepted
            | CheckoutWebhookEventType::DisputeCanceled
            | CheckoutWebhookEventType::DisputeEvidenceSubmitted
            | CheckoutWebhookEventType::DisputeEvidenceAcknowledgedByScheme
            | CheckoutWebhookEventType::DisputeEvidenceRequired
            | CheckoutWebhookEventType::DisputeArbitrationEvidenceSubmitted
            | CheckoutWebhookEventType::DisputeArbitrationSentToScheme
            | CheckoutWebhookEventType::DisputeArbitrationLost
            | CheckoutWebhookEventType::DisputeArbitrationWon
            | CheckoutWebhookEventType::DisputeWon
            | CheckoutWebhookEventType::DisputeLost
    )
}

impl From<CheckoutWebhookEventType> for domain_types::connector_types::EventType {
    fn from(event: CheckoutWebhookEventType) -> Self {
        match event {
            CheckoutWebhookEventType::AuthenticationStarted
            | CheckoutWebhookEventType::AuthenticationApproved
            | CheckoutWebhookEventType::AuthenticationAttempted => Self::PaymentActionRequired,
            CheckoutWebhookEventType::AuthenticationExpired
            | CheckoutWebhookEventType::AuthenticationFailed
            | CheckoutWebhookEventType::PaymentAuthenticationFailed => {
                Self::PaymentIntentAuthorizationFailure
            }
            CheckoutWebhookEventType::PaymentApproved => Self::PaymentIntentAuthorizationSuccess,
            CheckoutWebhookEventType::PaymentCaptured => Self::PaymentIntentSuccess,
            CheckoutWebhookEventType::PaymentDeclined => Self::PaymentIntentFailure,
            CheckoutWebhookEventType::PaymentCanceled => Self::PaymentIntentCancelFailure,
            CheckoutWebhookEventType::PaymentCaptureDeclined => Self::PaymentIntentCaptureFailure,
            CheckoutWebhookEventType::PaymentVoided => Self::PaymentIntentCancelled,
            CheckoutWebhookEventType::PaymentVoidDeclined => Self::PaymentIntentCancelFailure,
            CheckoutWebhookEventType::PaymentPending => Self::PaymentActionRequired,
            CheckoutWebhookEventType::PaymentCapturePending
            | CheckoutWebhookEventType::PaymentRetryScheduled => Self::PaymentIntentProcessing,
            CheckoutWebhookEventType::PaymentExpired => Self::PaymentIntentExpired,
            CheckoutWebhookEventType::CardVerified => Self::PaymentIntentSuccess,
            CheckoutWebhookEventType::CardVerificationDeclined => Self::PaymentIntentFailure,
            CheckoutWebhookEventType::PaymentRefunded => Self::RefundSuccess,
            CheckoutWebhookEventType::PaymentRefundDeclined => Self::RefundFailure,
            CheckoutWebhookEventType::PaymentRefundPending => Self::RefundProcessing,
            CheckoutWebhookEventType::DisputeReceived
            | CheckoutWebhookEventType::DisputeEvidenceRequired => Self::DisputeOpened,
            CheckoutWebhookEventType::DisputeExpired => Self::DisputeExpired,
            CheckoutWebhookEventType::DisputeAccepted => Self::DisputeAccepted,
            CheckoutWebhookEventType::DisputeCanceled => Self::DisputeCancelled,
            CheckoutWebhookEventType::DisputeEvidenceSubmitted
            | CheckoutWebhookEventType::DisputeEvidenceAcknowledgedByScheme
            | CheckoutWebhookEventType::DisputeArbitrationEvidenceSubmitted
            | CheckoutWebhookEventType::DisputeArbitrationSentToScheme => Self::DisputeChallenged,
            CheckoutWebhookEventType::DisputeWon
            | CheckoutWebhookEventType::DisputeArbitrationWon => Self::DisputeWon,
            CheckoutWebhookEventType::DisputeLost
            | CheckoutWebhookEventType::DisputeArbitrationLost => Self::DisputeLost,
            CheckoutWebhookEventType::Unknown => Self::IncomingWebhookEventUnspecified,
        }
    }
}

impl TryFrom<CheckoutWebhookEventType> for common_enums::AttemptStatus {
    type Error = error_stack::Report<domain_types::errors::WebhookError>;
    fn try_from(event: CheckoutWebhookEventType) -> Result<Self, Self::Error> {
        match event {
            CheckoutWebhookEventType::PaymentApproved => Ok(Self::Authorized),
            CheckoutWebhookEventType::PaymentCaptured | CheckoutWebhookEventType::CardVerified => {
                Ok(Self::Charged)
            }
            CheckoutWebhookEventType::PaymentDeclined
            | CheckoutWebhookEventType::PaymentCanceled
            | CheckoutWebhookEventType::PaymentCaptureDeclined
            | CheckoutWebhookEventType::PaymentExpired
            | CheckoutWebhookEventType::CardVerificationDeclined => Ok(Self::Failure),
            CheckoutWebhookEventType::AuthenticationExpired
            | CheckoutWebhookEventType::AuthenticationFailed
            | CheckoutWebhookEventType::PaymentAuthenticationFailed => {
                Ok(Self::AuthenticationFailed)
            }
            CheckoutWebhookEventType::AuthenticationStarted
            | CheckoutWebhookEventType::AuthenticationApproved
            | CheckoutWebhookEventType::AuthenticationAttempted
            | CheckoutWebhookEventType::PaymentPending => Ok(Self::AuthenticationPending),
            CheckoutWebhookEventType::PaymentCapturePending
            | CheckoutWebhookEventType::PaymentRetryScheduled => Ok(Self::Pending),
            CheckoutWebhookEventType::PaymentVoided => Ok(Self::Voided),
            CheckoutWebhookEventType::PaymentVoidDeclined => Ok(Self::VoidFailed),
            CheckoutWebhookEventType::PaymentRefunded
            | CheckoutWebhookEventType::PaymentRefundDeclined
            | CheckoutWebhookEventType::PaymentRefundPending
            | CheckoutWebhookEventType::DisputeReceived
            | CheckoutWebhookEventType::DisputeExpired
            | CheckoutWebhookEventType::DisputeAccepted
            | CheckoutWebhookEventType::DisputeCanceled
            | CheckoutWebhookEventType::DisputeEvidenceSubmitted
            | CheckoutWebhookEventType::DisputeEvidenceAcknowledgedByScheme
            | CheckoutWebhookEventType::DisputeEvidenceRequired
            | CheckoutWebhookEventType::DisputeArbitrationEvidenceSubmitted
            | CheckoutWebhookEventType::DisputeArbitrationSentToScheme
            | CheckoutWebhookEventType::DisputeArbitrationLost
            | CheckoutWebhookEventType::DisputeArbitrationWon
            | CheckoutWebhookEventType::DisputeWon
            | CheckoutWebhookEventType::DisputeLost
            | CheckoutWebhookEventType::Unknown => Err(error_stack::report!(
                domain_types::errors::WebhookError::WebhookEventTypeNotFound
            )),
        }
    }
}

impl TryFrom<CheckoutWebhookEventType> for common_enums::RefundStatus {
    type Error = error_stack::Report<domain_types::errors::WebhookError>;
    fn try_from(event: CheckoutWebhookEventType) -> Result<Self, Self::Error> {
        match event {
            CheckoutWebhookEventType::PaymentRefunded => Ok(Self::Success),
            CheckoutWebhookEventType::PaymentRefundDeclined => Ok(Self::Failure),
            CheckoutWebhookEventType::PaymentRefundPending => Ok(Self::Pending),
            CheckoutWebhookEventType::AuthenticationStarted
            | CheckoutWebhookEventType::AuthenticationApproved
            | CheckoutWebhookEventType::AuthenticationAttempted
            | CheckoutWebhookEventType::AuthenticationExpired
            | CheckoutWebhookEventType::AuthenticationFailed
            | CheckoutWebhookEventType::PaymentApproved
            | CheckoutWebhookEventType::PaymentCaptured
            | CheckoutWebhookEventType::PaymentDeclined
            | CheckoutWebhookEventType::PaymentAuthenticationFailed
            | CheckoutWebhookEventType::PaymentCanceled
            | CheckoutWebhookEventType::PaymentCaptureDeclined
            | CheckoutWebhookEventType::PaymentVoided
            | CheckoutWebhookEventType::PaymentPending
            | CheckoutWebhookEventType::PaymentCapturePending
            | CheckoutWebhookEventType::PaymentRetryScheduled
            | CheckoutWebhookEventType::PaymentExpired
            | CheckoutWebhookEventType::PaymentVoidDeclined
            | CheckoutWebhookEventType::CardVerified
            | CheckoutWebhookEventType::CardVerificationDeclined
            | CheckoutWebhookEventType::DisputeReceived
            | CheckoutWebhookEventType::DisputeExpired
            | CheckoutWebhookEventType::DisputeAccepted
            | CheckoutWebhookEventType::DisputeCanceled
            | CheckoutWebhookEventType::DisputeEvidenceSubmitted
            | CheckoutWebhookEventType::DisputeEvidenceAcknowledgedByScheme
            | CheckoutWebhookEventType::DisputeEvidenceRequired
            | CheckoutWebhookEventType::DisputeArbitrationEvidenceSubmitted
            | CheckoutWebhookEventType::DisputeArbitrationSentToScheme
            | CheckoutWebhookEventType::DisputeArbitrationLost
            | CheckoutWebhookEventType::DisputeArbitrationWon
            | CheckoutWebhookEventType::DisputeWon
            | CheckoutWebhookEventType::DisputeLost
            | CheckoutWebhookEventType::Unknown => Err(error_stack::report!(
                domain_types::errors::WebhookError::WebhookEventTypeNotFound
            )),
        }
    }
}

impl TryFrom<CheckoutDisputeTransactionType>
    for (common_enums::DisputeStatus, common_enums::DisputeStage)
{
    type Error = error_stack::Report<domain_types::errors::WebhookError>;
    fn try_from(dispute_type: CheckoutDisputeTransactionType) -> Result<Self, Self::Error> {
        match dispute_type {
            CheckoutDisputeTransactionType::DisputeReceived
            | CheckoutDisputeTransactionType::DisputeEvidenceRequired => Ok((
                common_enums::DisputeStatus::DisputeOpened,
                common_enums::DisputeStage::Dispute,
            )),
            CheckoutDisputeTransactionType::DisputeExpired => Ok((
                common_enums::DisputeStatus::DisputeExpired,
                common_enums::DisputeStage::Dispute,
            )),
            CheckoutDisputeTransactionType::DisputeAccepted => Ok((
                common_enums::DisputeStatus::DisputeAccepted,
                common_enums::DisputeStage::Dispute,
            )),
            CheckoutDisputeTransactionType::DisputeCanceled => Ok((
                common_enums::DisputeStatus::DisputeCancelled,
                common_enums::DisputeStage::Dispute,
            )),
            CheckoutDisputeTransactionType::DisputeEvidenceSubmitted
            | CheckoutDisputeTransactionType::DisputeEvidenceAcknowledgedByScheme => Ok((
                common_enums::DisputeStatus::DisputeChallenged,
                common_enums::DisputeStage::Dispute,
            )),
            CheckoutDisputeTransactionType::DisputeWon => Ok((
                common_enums::DisputeStatus::DisputeWon,
                common_enums::DisputeStage::Dispute,
            )),
            CheckoutDisputeTransactionType::DisputeLost => Ok((
                common_enums::DisputeStatus::DisputeLost,
                common_enums::DisputeStage::Dispute,
            )),
            CheckoutDisputeTransactionType::DisputeArbitrationEvidenceSubmitted
            | CheckoutDisputeTransactionType::DisputeArbitrationSentToScheme => Ok((
                common_enums::DisputeStatus::DisputeChallenged,
                common_enums::DisputeStage::PreArbitration,
            )),
            CheckoutDisputeTransactionType::DisputeArbitrationWon => Ok((
                common_enums::DisputeStatus::DisputeWon,
                common_enums::DisputeStage::PreArbitration,
            )),
            CheckoutDisputeTransactionType::DisputeArbitrationLost => Ok((
                common_enums::DisputeStatus::DisputeLost,
                common_enums::DisputeStage::PreArbitration,
            )),
            CheckoutDisputeTransactionType::Unknown => Err(error_stack::report!(
                domain_types::errors::WebhookError::WebhookBodyDecodingFailed
            )
            .attach_printable(
                "Unrecognized Checkout dispute event type; cannot determine dispute outcome",
            )),
        }
    }
}

struct CheckoutAftStateParams {
    state: Option<Secret<String>>,
    country: common_enums::CountryAlpha2,
    field_name: &'static str,
}

impl TryFrom<CheckoutAftStateParams> for Option<Secret<String>> {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(params: CheckoutAftStateParams) -> Result<Self, Self::Error> {
        match params.country {
            common_enums::CountryAlpha2::US | common_enums::CountryAlpha2::CA => Ok(Some(
                params
                    .state
                    .ok_or_else(utils::missing_field_err(params.field_name))?,
            )),
            _ => Ok(params.state),
        }
    }
}
