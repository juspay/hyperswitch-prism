use std::{collections::HashMap, fmt::Debug, ops::Deref, str::FromStr};

use cards::CardNumber;
use common_utils::{
    collect_missing_value_keys, consts, custom_serde,
    errors::CustomResult,
    ext_traits::{ByteSliceExt, Encode, OptionExt},
    pii::{self, Email},
    request::Method,
    types::MinorUnit,
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, CreateConnectorCustomer,
        IncrementalAuthorization, PaymentMethodToken, RepeatPayment, SetupMandate, Void,
    },
    connector_types::{
        ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData, ConnectorCustomerData,
        ConnectorCustomerResponse, ConnectorSpecificClientAuthenticationResponse, MandateReference,
        MandateReferenceId, PaymentFlowData, PaymentMethodTokenResponse,
        PaymentMethodTokenizationData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsIncrementalAuthorizationData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        ResponseId, SetupMandateRequestData,
        StripeClientAuthenticationResponse as StripeClientAuthenticationResponseDomain,
    },
    errors::{ConnectorError, IntegrationError},
    mandates::AcceptanceType,
    payment_method_data::{
        self, AchTransfer, BankRedirectData, BankTransferInstructions, BankTransferNextStepsData,
        Card, CardRedirectData, GiftCardData, GooglePayWalletData, MultibancoTransferInstructions,
        PayLaterData, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber, VoucherData,
        WalletData,
    },
    router_data::{
        AdditionalPaymentMethodConnectorResponse, ConnectorResponseData, ConnectorSpecificConfig,
        ExtendedAuthorizationResponseData,
    },
    router_data_v2::RouterDataV2,
    router_request_types::{
        AuthoriseIntegrityObject, BrowserInformation, CaptureIntegrityObject,
        PaymentSynIntegrityObject, RefundIntegrityObject,
    },
    router_response_types::RedirectForm,
    utils::{get_unimplemented_payment_method_error_message, is_payment_failure},
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Mask, Maskable, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::PrimitiveDateTime;
use url::Url;

use crate::{
    connectors::stripe::{
        headers::STRIPE_COMPATIBLE_CONNECT_ACCOUNT, StripeAmountConvertor, StripeRouterData,
    },
    types::ResponseRouterData,
    utils::{
        convert_uppercase, deserialize_zero_minor_amount_as_none, is_refund_failure,
        SplitPaymentData,
    },
};

pub mod auth_headers {
    pub const STRIPE_API_VERSION: &str = "stripe-version";
    pub const STRIPE_VERSION: &str = "2022-11-15";
}

trait GetRequestIncrementalAuthorization {
    fn get_request_incremental_authorization(&self) -> Option<bool>;
}

impl<T: PaymentMethodDataTypes> GetRequestIncrementalAuthorization for PaymentsAuthorizeData<T> {
    fn get_request_incremental_authorization(&self) -> Option<bool> {
        self.request_incremental_authorization
    }
}

impl GetRequestIncrementalAuthorization for PaymentsCaptureData {
    fn get_request_incremental_authorization(&self) -> Option<bool> {
        None
    }
}

impl GetRequestIncrementalAuthorization for PaymentVoidData {
    fn get_request_incremental_authorization(&self) -> Option<bool> {
        None
    }
}

impl<T: PaymentMethodDataTypes> GetRequestIncrementalAuthorization for RepeatPaymentData<T> {
    fn get_request_incremental_authorization(&self) -> Option<bool> {
        Some(false)
    }
}

pub struct StripeAuthType {
    pub(super) api_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for StripeAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(item: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match item {
            ConnectorSpecificConfig::Stripe { api_key, .. } => Ok(Self {
                api_key: api_key.to_owned(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

#[derive(Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StripeCaptureMethod {
    Manual,
    #[default]
    Automatic,
}

impl From<Option<common_enums::CaptureMethod>> for StripeCaptureMethod {
    fn from(item: Option<common_enums::CaptureMethod>) -> Self {
        match item {
            Some(p) => match p {
                common_enums::CaptureMethod::ManualMultiple => Self::Manual,
                common_enums::CaptureMethod::Manual => Self::Manual,
                common_enums::CaptureMethod::Automatic
                | common_enums::CaptureMethod::SequentialAutomatic => Self::Automatic,
                common_enums::CaptureMethod::Scheduled => Self::Manual,
            },
            None => Self::Automatic,
        }
    }
}

#[derive(Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Auth3ds {
    #[default]
    Automatic,
    Any,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StripeCardNetwork {
    CartesBancaires,
    Mastercard,
    Visa,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(
    rename_all = "snake_case",
    tag = "mandate_data[customer_acceptance][type]"
)]
pub enum StripeMandateType {
    Online {
        #[serde(rename = "mandate_data[customer_acceptance][online][ip_address]")]
        ip_address: Secret<String, pii::IpAddress>,
        #[serde(rename = "mandate_data[customer_acceptance][online][user_agent]")]
        user_agent: String,
    },
    Offline,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct StripeMandateRequest {
    #[serde(flatten)]
    mandate_type: StripeMandateType,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpandableObjects {
    LatestCharge,
    Customer,
    LatestAttempt,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct StripeBrowserInformation {
    #[serde(rename = "payment_method_data[ip]")]
    pub ip_address: Option<Secret<String, pii::IpAddress>>,
    #[serde(rename = "payment_method_data[user_agent]")]
    pub user_agent: Option<String>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct PaymentIntentRequest<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
> {
    pub amount: MinorUnit, //amount in cents, hence passed as integer
    pub currency: String,
    pub statement_descriptor_suffix: Option<String>,
    pub statement_descriptor: Option<String>,
    #[serde(flatten)]
    pub meta_data: HashMap<String, String>,
    pub return_url: String,
    pub confirm: bool,
    pub payment_method: Option<Secret<String>>,
    pub customer: Option<Secret<String>>,
    #[serde(flatten)]
    pub setup_mandate_details: Option<StripeMandateRequest>,
    pub description: Option<String>,
    #[serde(flatten)]
    pub shipping: Option<StripeShippingAddress>,
    #[serde(flatten)]
    pub billing: StripeBillingAddress,
    #[serde(flatten)]
    pub payment_data: Option<StripePaymentMethodData<T>>,
    pub capture_method: StripeCaptureMethod,
    #[serde(flatten)]
    pub payment_method_options: Option<StripePaymentMethodOptions>, // For mandate txns using network_txns_id, needs to be validated
    pub setup_future_usage: Option<common_enums::FutureUsage>,
    pub off_session: Option<bool>,
    #[serde(rename = "payment_method_types[0]")]
    pub payment_method_types: Option<StripePaymentMethodType>,
    #[serde(rename = "expand[0]")]
    pub expand: Option<ExpandableObjects>,
    #[serde(flatten)]
    pub browser_info: Option<StripeBrowserInformation>,
    #[serde(flatten)]
    pub charges: Option<IntentCharges>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct IntentCharges {
    pub application_fee_amount: Option<MinorUnit>,
    #[serde(
        rename = "transfer_data[destination]",
        skip_serializing_if = "Option::is_none"
    )]
    pub destination_account_id: Option<Secret<String>>,
}

// Field rename is required only in case of serialization as it is passed in the request to the connector.
// Deserialization is happening only in case of webhooks, where fields name should be used as defined in the struct.
// Whenever adding new fields, Please ensure it doesn't break the webhook flow
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct StripeMetadata {
    // merchant_reference_id
    #[serde(rename(serialize = "metadata[order_id]"))]
    pub order_id: Option<String>,
    // to check whether the order_id is refund_id or payment_id
    // before deployment, order id is set to payment_id in refunds but now it is set as refund_id
    // it is set as string instead of bool because stripe pass it as string even if we set it as bool
    #[serde(rename(serialize = "metadata[is_refund_id_as_reference]"))]
    pub is_refund_id_as_reference: Option<String>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct SetupMandateRequest<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
> {
    pub confirm: bool,
    pub usage: Option<common_enums::FutureUsage>,
    pub customer: Option<Secret<String>>,
    pub off_session: Option<bool>,
    pub return_url: Option<String>,
    #[serde(flatten)]
    pub payment_data: StripePaymentMethodData<T>,
    pub payment_method_options: Option<StripePaymentMethodOptions>, // For mandate txns using network_txns_id, needs to be validated
    #[serde(flatten)]
    pub meta_data: Option<HashMap<String, String>>,
    #[serde(rename = "payment_method_types[0]")]
    pub payment_method_types: Option<StripePaymentMethodType>,
    #[serde(rename = "expand[0]")]
    pub expand: Option<ExpandableObjects>,
    #[serde(flatten)]
    pub browser_info: Option<StripeBrowserInformation>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct StripeCardData<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> {
    #[serde(rename = "payment_method_data[type]")]
    pub payment_method_data_type: StripePaymentMethodType,
    #[serde(rename = "payment_method_data[card][number]")]
    pub payment_method_data_card_number: RawCardNumber<T>,
    #[serde(rename = "payment_method_data[card][exp_month]")]
    pub payment_method_data_card_exp_month: Secret<String>,
    #[serde(rename = "payment_method_data[card][exp_year]")]
    pub payment_method_data_card_exp_year: Secret<String>,
    #[serde(rename = "payment_method_data[card][cvc]")]
    pub payment_method_data_card_cvc: Option<Secret<String>>,
    #[serde(rename = "payment_method_options[card][request_three_d_secure]")]
    pub payment_method_auth_type: Option<Auth3ds>,
    #[serde(rename = "payment_method_options[card][network]")]
    pub payment_method_data_card_preferred_network: Option<StripeCardNetwork>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "payment_method_options[card][request_incremental_authorization]")]
    pub request_incremental_authorization: Option<StripeRequestIncrementalAuthorization>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "payment_method_options[card][request_extended_authorization]")]
    request_extended_authorization: Option<StripeRequestExtendedAuthorization>,
    #[serde(rename = "payment_method_options[card][request_overcapture]")]
    pub request_overcapture: Option<StripeRequestOvercaptureBool>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct StripeCardNetworkTransactionIdData {
    #[serde(rename = "payment_method_data[type]")]
    pub payment_method_data_type: StripePaymentMethodType,
    #[serde(rename = "payment_method_data[card][number]")]
    pub payment_method_data_card_number: CardNumber,
    #[serde(rename = "payment_method_data[card][exp_month]")]
    pub payment_method_data_card_exp_month: Secret<String>,
    #[serde(rename = "payment_method_data[card][exp_year]")]
    pub payment_method_data_card_exp_year: Secret<String>,
    #[serde(rename = "payment_method_data[card][cvc]")]
    pub payment_method_data_card_cvc: Option<Secret<String>>,
    #[serde(rename = "payment_method_options[card][request_three_d_secure]")]
    pub payment_method_auth_type: Option<Auth3ds>,
    #[serde(rename = "payment_method_options[card][network]")]
    pub payment_method_data_card_preferred_network: Option<StripeCardNetwork>,
    #[serde(rename = "payment_method_options[card][request_overcapture]")]
    pub request_overcapture: Option<StripeRequestOvercaptureBool>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StripeRequestIncrementalAuthorization {
    IfAvailable,
    Never,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum StripeRequestExtendedAuthorization {
    IfAvailable,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StripeRequestOvercaptureBool {
    IfAvailable,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct StripePayLaterData {
    #[serde(rename = "payment_method_data[type]")]
    pub payment_method_data_type: StripePaymentMethodType,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct TokenRequest<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> {
    #[serde(flatten)]
    pub token_data: StripePaymentMethodData<T>,
}

#[derive(Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct StripeTokenResponse {
    pub id: Secret<String>,
    pub object: String,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct CreateConnectorCustomerRequest {
    pub description: Option<String>,
    pub email: Option<Email>,
    pub phone: Option<Secret<String>>,
    pub name: Option<Secret<String>>,
    pub source: Option<Secret<String>>,
}

#[derive(Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct CreateConnectorCustomerResponse {
    pub id: String,
    pub description: Option<String>,
    pub email: Option<Email>,
    pub phone: Option<Secret<String>>,
    pub name: Option<Secret<String>>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct ChargesRequest {
    pub amount: MinorUnit,
    pub currency: String,
    pub customer: Secret<String>,
    pub source: Secret<String>,
    #[serde(flatten)]
    pub meta_data: Option<HashMap<String, String>>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct ChargesResponse {
    pub id: String,
    pub amount: MinorUnit,
    pub amount_captured: MinorUnit,
    pub currency: String,
    pub status: StripePaymentStatus,
    pub source: StripeSourceResponse,
    pub failure_code: Option<String>,
    pub failure_message: Option<String>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum StripeBankName {
    Eps {
        #[serde(rename = "payment_method_data[eps][bank]")]
        bank_name: Option<StripeBankNames>,
    },
    Ideal {
        #[serde(rename = "payment_method_data[ideal][bank]")]
        ideal_bank_name: Option<StripeBankNames>,
    },
    Przelewy24 {
        #[serde(rename = "payment_method_data[p24][bank]")]
        bank_name: Option<StripeBankNames>,
    },
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum StripeBankRedirectData {
    StripeGiropay(Box<StripeGiropay>),
    StripeIdeal(Box<StripeIdeal>),
    StripeBancontactCard(Box<StripeBancontactCard>),
    StripePrezelewy24(Box<StripePrezelewy24>),
    StripeEps(Box<StripeEps>),
    StripeBlik(Box<StripeBlik>),
    StripeOnlineBankingFpx(Box<StripeOnlineBankingFpx>),
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct StripeGiropay {
    #[serde(rename = "payment_method_data[type]")]
    pub payment_method_data_type: StripePaymentMethodType,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct StripeIdeal {
    #[serde(rename = "payment_method_data[type]")]
    pub payment_method_data_type: StripePaymentMethodType,
    #[serde(rename = "payment_method_data[ideal][bank]")]
    ideal_bank_name: Option<StripeBankNames>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct StripeBancontactCard {
    #[serde(rename = "payment_method_data[type]")]
    pub payment_method_data_type: StripePaymentMethodType,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct StripePrezelewy24 {
    #[serde(rename = "payment_method_data[type]")]
    pub payment_method_data_type: StripePaymentMethodType,
    #[serde(rename = "payment_method_data[p24][bank]")]
    bank_name: Option<StripeBankNames>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct StripeEps {
    #[serde(rename = "payment_method_data[type]")]
    pub payment_method_data_type: StripePaymentMethodType,
    #[serde(rename = "payment_method_data[eps][bank]")]
    bank_name: Option<StripeBankNames>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct StripeBlik {
    #[serde(rename = "payment_method_data[type]")]
    pub payment_method_data_type: StripePaymentMethodType,
    #[serde(rename = "payment_method_options[blik][code]")]
    pub code: Secret<String>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct StripeOnlineBankingFpx {
    #[serde(rename = "payment_method_data[type]")]
    pub payment_method_data_type: StripePaymentMethodType,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct AchTransferData {
    #[serde(rename = "payment_method_data[type]")]
    pub payment_method_data_type: StripePaymentMethodType,
    #[serde(rename = "payment_method_options[customer_balance][bank_transfer][type]")]
    pub bank_transfer_type: StripeCreditTransferTypes,
    #[serde(rename = "payment_method_types[0]")]
    pub payment_method_type: StripePaymentMethodType,
    #[serde(rename = "payment_method_options[customer_balance][funding_type]")]
    pub balance_funding_type: BankTransferType,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct MultibancoTransferData {
    #[serde(rename = "payment_method_data[type]")]
    pub payment_method_data_type: StripeCreditTransferTypes,
    #[serde(rename = "payment_method_types[0]")]
    pub payment_method_type: StripeCreditTransferTypes,
    #[serde(rename = "payment_method_data[billing_details][email]")]
    pub email: Email,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct BacsBankTransferData {
    #[serde(rename = "payment_method_data[type]")]
    pub payment_method_data_type: StripePaymentMethodType,
    #[serde(rename = "payment_method_options[customer_balance][bank_transfer][type]")]
    pub bank_transfer_type: BankTransferType,
    #[serde(rename = "payment_method_options[customer_balance][funding_type]")]
    pub balance_funding_type: BankTransferType,
    #[serde(rename = "payment_method_types[0]")]
    pub payment_method_type: StripePaymentMethodType,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct SepaBankTransferData {
    #[serde(rename = "payment_method_data[type]")]
    pub payment_method_data_type: StripePaymentMethodType,
    #[serde(rename = "payment_method_options[customer_balance][bank_transfer][type]")]
    pub bank_transfer_type: BankTransferType,
    #[serde(rename = "payment_method_options[customer_balance][funding_type]")]
    pub balance_funding_type: BankTransferType,
    #[serde(rename = "payment_method_types[0]")]
    pub payment_method_type: StripePaymentMethodType,
    #[serde(
        rename = "payment_method_options[customer_balance][bank_transfer][eu_bank_transfer][country]"
    )]
    pub country: common_enums::CountryAlpha2,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum StripeCreditTransferSourceRequest {
    AchBankTransfer(AchCreditTransferSourceRequest),
    MultibancoBankTransfer(MultibancoCreditTransferSourceRequest),
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct AchCreditTransferSourceRequest {
    #[serde(rename = "type")]
    pub transfer_type: StripeCreditTransferTypes,
    #[serde(flatten)]
    pub payment_method_data: AchTransferData,
    pub currency: common_enums::Currency,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct MultibancoCreditTransferSourceRequest {
    #[serde(rename = "type")]
    pub transfer_type: StripeCreditTransferTypes,
    #[serde(flatten)]
    pub payment_method_data: MultibancoTransferData,
    pub currency: common_enums::Currency,
    pub amount: Option<MinorUnit>,
    #[serde(rename = "redirect[return_url]")]
    pub return_url: Option<String>,
}

// Remove untagged when Deserialize is added
#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum StripePaymentMethodData<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
> {
    CardToken(StripeCardToken<T>),
    Card(StripeCardData<T>),
    CardNetworkTransactionId(StripeCardNetworkTransactionIdData),
    PayLater(StripePayLaterData),
    Wallet(StripeWallet),
    BankRedirect(StripeBankRedirectData),
    BankDebit(StripeBankDebitData),
    BankTransfer(StripeBankTransferData),
}

#[derive(Debug, Clone, Default, Eq, PartialEq, Serialize)]
pub struct StripeBillingAddressCardToken {
    #[serde(rename = "billing_details[name]")]
    pub name: Option<Secret<String>>,
    #[serde(rename = "billing_details[email]")]
    pub email: Option<Email>,
    #[serde(rename = "billing_details[phone]")]
    pub phone: Option<Secret<String>>,
    #[serde(rename = "billing_details[address][line1]")]
    pub address_line1: Option<Secret<String>>,
    #[serde(rename = "billing_details[address][line2]")]
    pub address_line2: Option<Secret<String>>,
    #[serde(rename = "billing_details[address][state]")]
    pub state: Option<Secret<String>>,
    #[serde(rename = "billing_details[address][city]")]
    pub city: Option<Secret<String>>,
}

// Struct to call the Stripe tokens API to create a PSP token for the card details provided.
#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct StripeCardToken<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> {
    #[serde(rename = "type")]
    pub payment_method_type: Option<StripePaymentMethodType>,
    #[serde(rename = "card[number]")]
    pub token_card_number: RawCardNumber<T>,
    #[serde(rename = "card[exp_month]")]
    pub token_card_exp_month: Secret<String>,
    #[serde(rename = "card[exp_year]")]
    pub token_card_exp_year: Secret<String>,
    #[serde(rename = "card[cvc]")]
    pub token_card_cvc: Secret<String>,
    #[serde(flatten)]
    pub billing: StripeBillingAddressCardToken,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "payment_method_data[type]")]
pub enum BankDebitData {
    #[serde(rename = "us_bank_account")]
    Ach {
        #[serde(rename = "payment_method_data[us_bank_account][account_holder_type]")]
        account_holder_type: String,
        #[serde(rename = "payment_method_data[us_bank_account][account_number]")]
        account_number: Secret<String>,
        #[serde(rename = "payment_method_data[us_bank_account][routing_number]")]
        routing_number: Secret<String>,
    },
    #[serde(rename = "sepa_debit")]
    Sepa {
        #[serde(rename = "payment_method_data[sepa_debit][iban]")]
        iban: Secret<String>,
    },
    #[serde(rename = "au_becs_debit")]
    Becs {
        #[serde(rename = "payment_method_data[au_becs_debit][account_number]")]
        account_number: Secret<String>,
        #[serde(rename = "payment_method_data[au_becs_debit][bsb_number]")]
        bsb_number: Secret<String>,
    },
    #[serde(rename = "bacs_debit")]
    Bacs {
        #[serde(rename = "payment_method_data[bacs_debit][account_number]")]
        account_number: Secret<String>,
        #[serde(rename = "payment_method_data[bacs_debit][sort_code]")]
        sort_code: Secret<String>,
    },
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct StripeBankDebitData {
    #[serde(flatten)]
    pub bank_specific_data: Option<BankDebitData>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct BankTransferData {
    pub email: Email,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum StripeBankTransferData {
    AchBankTransfer(Box<AchTransferData>),
    SepaBankTransfer(Box<SepaBankTransferData>),
    BacsBankTransfers(Box<BacsBankTransferData>),
    MultibancoBankTransfers(Box<MultibancoTransferData>),
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum StripeWallet {
    ApplepayToken(StripeApplePay),
    GooglepayToken(GooglePayToken),
    ApplepayPayment(ApplepayPayment),
    AmazonpayPayment(AmazonpayPayment),
    WechatpayPayment(WechatpayPayment),
    AlipayPayment(AlipayPayment),
    Cashapp(CashappPayment),
    RevolutPay(RevolutpayPayment),
    ApplePayPredecryptToken(Box<StripeApplePayPredecrypt>),
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct StripeApplePayPredecrypt {
    #[serde(rename = "card[number]")]
    number: CardNumber,
    #[serde(rename = "card[exp_year]")]
    exp_year: Secret<String>,
    #[serde(rename = "card[exp_month]")]
    exp_month: Secret<String>,
    #[serde(rename = "card[cryptogram]")]
    cryptogram: Secret<String>,
    #[serde(rename = "card[eci]")]
    eci: Option<String>,
    #[serde(rename = "card[tokenization_method]")]
    tokenization_method: String,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct StripeApplePay {
    pub pk_token: Secret<String>,
    pub pk_token_instrument_name: String,
    pub pk_token_payment_network: String,
    pub pk_token_transaction_id: Secret<String>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct GooglePayToken {
    #[serde(rename = "payment_method_data[type]")]
    pub payment_type: StripePaymentMethodType,
    #[serde(rename = "payment_method_data[card][token]")]
    pub token: Secret<String>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct ApplepayPayment {
    #[serde(rename = "payment_method_data[card][token]")]
    pub token: Secret<String>,
    #[serde(rename = "payment_method_data[type]")]
    pub payment_method_types: StripePaymentMethodType,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct AmazonpayPayment {
    #[serde(rename = "payment_method_data[type]")]
    pub payment_method_types: StripePaymentMethodType,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct RevolutpayPayment {
    #[serde(rename = "payment_method_data[type]")]
    pub payment_method_types: StripePaymentMethodType,
}
#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct AlipayPayment {
    #[serde(rename = "payment_method_data[type]")]
    pub payment_method_data_type: StripePaymentMethodType,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct CashappPayment {
    #[serde(rename = "payment_method_data[type]")]
    pub payment_method_data_type: StripePaymentMethodType,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct WechatpayPayment {
    #[serde(rename = "payment_method_data[type]")]
    pub payment_method_data_type: StripePaymentMethodType,
    #[serde(rename = "payment_method_options[wechat_pay][client]")]
    pub client: WechatClient,
}

#[derive(Debug, Eq, PartialEq, Serialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum WechatClient {
    Web,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct GooglepayPayment {
    #[serde(rename = "payment_method_data[card][token]")]
    pub token: Secret<String>,
    #[serde(rename = "payment_method_data[type]")]
    pub payment_method_types: StripePaymentMethodType,
}

// All supported payment_method_types in stripe
// This enum goes in payment_method_types[] field in stripe request body
// https://stripe.com/docs/api/payment_intents/create#create_payment_intent-payment_method_types
#[derive(Eq, PartialEq, Serialize, Clone, Debug, Copy)]
#[serde(rename_all = "snake_case")]
pub enum StripePaymentMethodType {
    Affirm,
    AfterpayClearpay,
    Alipay,
    #[serde(rename = "amazon_pay")]
    AmazonPay,
    #[serde(rename = "au_becs_debit")]
    Becs,
    #[serde(rename = "bacs_debit")]
    Bacs,
    Bancontact,
    Blik,
    Card,
    CustomerBalance,
    Eps,
    Giropay,
    Ideal,
    Klarna,
    #[serde(rename = "p24")]
    Przelewy24,
    #[serde(rename = "sepa_debit")]
    Sepa,
    Sofort,
    #[serde(rename = "us_bank_account")]
    Ach,
    #[serde(rename = "wechat_pay")]
    Wechatpay,
    #[serde(rename = "cashapp")]
    Cashapp,
    RevolutPay,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum StripeCreditTransferTypes {
    #[serde(rename = "us_bank_transfer")]
    AchCreditTransfer,
    Multibanco,
    Blik,
}

impl TryFrom<common_enums::PaymentMethodType> for StripePaymentMethodType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(value: common_enums::PaymentMethodType) -> Result<Self, Self::Error> {
        match value {
            common_enums::PaymentMethodType::Card => Ok(Self::Card),
            common_enums::PaymentMethodType::Klarna => Ok(Self::Klarna),
            common_enums::PaymentMethodType::Affirm => Ok(Self::Affirm),
            common_enums::PaymentMethodType::AfterpayClearpay => Ok(Self::AfterpayClearpay),
            common_enums::PaymentMethodType::Eps => Ok(Self::Eps),
            common_enums::PaymentMethodType::Giropay => Ok(Self::Giropay),
            common_enums::PaymentMethodType::Ideal => Ok(Self::Ideal),
            common_enums::PaymentMethodType::Sofort => Ok(Self::Sofort),
            common_enums::PaymentMethodType::AmazonPay => Ok(Self::AmazonPay),
            common_enums::PaymentMethodType::ApplePay => Ok(Self::Card),
            common_enums::PaymentMethodType::Ach => Ok(Self::Ach),
            common_enums::PaymentMethodType::Sepa => Ok(Self::Sepa),
            common_enums::PaymentMethodType::Becs => Ok(Self::Becs),
            common_enums::PaymentMethodType::Bacs => Ok(Self::Bacs),
            common_enums::PaymentMethodType::BancontactCard => Ok(Self::Bancontact),
            common_enums::PaymentMethodType::WeChatPay => Ok(Self::Wechatpay),
            common_enums::PaymentMethodType::Blik => Ok(Self::Blik),
            common_enums::PaymentMethodType::AliPay => Ok(Self::Alipay),
            common_enums::PaymentMethodType::Przelewy24 => Ok(Self::Przelewy24),
            common_enums::PaymentMethodType::RevolutPay => Ok(Self::RevolutPay),
            // Stripe expects PMT as Card for Recurring Mandates Payments
            common_enums::PaymentMethodType::GooglePay => Ok(Self::Card),
            common_enums::PaymentMethodType::Boleto
            | common_enums::PaymentMethodType::CardRedirect
            | common_enums::PaymentMethodType::CryptoCurrency
            | common_enums::PaymentMethodType::Multibanco
            | common_enums::PaymentMethodType::OnlineBankingFpx
            | common_enums::PaymentMethodType::Paypal
            | common_enums::PaymentMethodType::Pix
            | common_enums::PaymentMethodType::UpiCollect
            | common_enums::PaymentMethodType::UpiIntent
            | common_enums::PaymentMethodType::UpiQr
            | common_enums::PaymentMethodType::Cashapp
            | common_enums::PaymentMethodType::Bluecode
            | common_enums::PaymentMethodType::SepaGuaranteedDebit
            | common_enums::PaymentMethodType::Oxxo => Err(IntegrationError::NotImplemented(
                (get_unimplemented_payment_method_error_message("stripe")).into(),
                Default::default(),
            )
            .into()),
            common_enums::PaymentMethodType::AliPayHk
            | common_enums::PaymentMethodType::Atome
            | common_enums::PaymentMethodType::Bizum
            | common_enums::PaymentMethodType::Alma
            | common_enums::PaymentMethodType::ClassicReward
            | common_enums::PaymentMethodType::Dana
            | common_enums::PaymentMethodType::DirectCarrierBilling
            | common_enums::PaymentMethodType::Efecty
            | common_enums::PaymentMethodType::Eft
            | common_enums::PaymentMethodType::Evoucher
            | common_enums::PaymentMethodType::GoPay
            | common_enums::PaymentMethodType::Gcash
            | common_enums::PaymentMethodType::Interac
            | common_enums::PaymentMethodType::KakaoPay
            | common_enums::PaymentMethodType::LocalBankRedirect
            | common_enums::PaymentMethodType::MbWay
            | common_enums::PaymentMethodType::MobilePay
            | common_enums::PaymentMethodType::Momo
            | common_enums::PaymentMethodType::MomoAtm
            | common_enums::PaymentMethodType::OnlineBankingThailand
            | common_enums::PaymentMethodType::OnlineBankingCzechRepublic
            | common_enums::PaymentMethodType::OnlineBankingFinland
            | common_enums::PaymentMethodType::OnlineBankingPoland
            | common_enums::PaymentMethodType::OnlineBankingSlovakia
            | common_enums::PaymentMethodType::OpenBankingUk
            | common_enums::PaymentMethodType::OpenBanking
            | common_enums::PaymentMethodType::OpenBankingPIS
            | common_enums::PaymentMethodType::PagoEfectivo
            | common_enums::PaymentMethodType::PayBright
            | common_enums::PaymentMethodType::Pse
            | common_enums::PaymentMethodType::RedCompra
            | common_enums::PaymentMethodType::RedPagos
            | common_enums::PaymentMethodType::SamsungPay
            | common_enums::PaymentMethodType::Swish
            | common_enums::PaymentMethodType::TouchNGo
            | common_enums::PaymentMethodType::Trustly
            | common_enums::PaymentMethodType::Twint
            | common_enums::PaymentMethodType::Vipps
            | common_enums::PaymentMethodType::Venmo
            | common_enums::PaymentMethodType::Alfamart
            | common_enums::PaymentMethodType::BcaBankTransfer
            | common_enums::PaymentMethodType::BniVa
            | common_enums::PaymentMethodType::CimbVa
            | common_enums::PaymentMethodType::BriVa
            | common_enums::PaymentMethodType::DanamonVa
            | common_enums::PaymentMethodType::Indomaret
            | common_enums::PaymentMethodType::MandiriVa
            | common_enums::PaymentMethodType::PermataBankTransfer
            | common_enums::PaymentMethodType::PaySafeCard
            | common_enums::PaymentMethodType::Paze
            | common_enums::PaymentMethodType::Givex
            | common_enums::PaymentMethodType::Benefit
            | common_enums::PaymentMethodType::Knet
            | common_enums::PaymentMethodType::SevenEleven
            | common_enums::PaymentMethodType::Lawson
            | common_enums::PaymentMethodType::MiniStop
            | common_enums::PaymentMethodType::FamilyMart
            | common_enums::PaymentMethodType::Seicomart
            | common_enums::PaymentMethodType::PayEasy
            | common_enums::PaymentMethodType::LocalBankTransfer
            | common_enums::PaymentMethodType::InstantBankTransfer
            | common_enums::PaymentMethodType::InstantBankTransferFinland
            | common_enums::PaymentMethodType::InstantBankTransferPoland
            | common_enums::PaymentMethodType::SepaBankTransfer
            | common_enums::PaymentMethodType::IndonesianBankTransfer
            | common_enums::PaymentMethodType::Walley
            | common_enums::PaymentMethodType::Fps
            | common_enums::PaymentMethodType::DuitNow
            | common_enums::PaymentMethodType::PromptPay
            | common_enums::PaymentMethodType::VietQr
            | common_enums::PaymentMethodType::NetworkToken
            | common_enums::PaymentMethodType::Mifinity
            | common_enums::PaymentMethodType::Satispay
            | common_enums::PaymentMethodType::Wero
            | common_enums::PaymentMethodType::LazyPay
            | common_enums::PaymentMethodType::PhonePe
            | common_enums::PaymentMethodType::BillDesk
            | common_enums::PaymentMethodType::Cashfree
            | common_enums::PaymentMethodType::PayU
            | common_enums::PaymentMethodType::EaseBuzz
            | common_enums::PaymentMethodType::Netbanking => Err(IntegrationError::NotImplemented(
                (get_unimplemented_payment_method_error_message("stripe")).into(),
                Default::default(),
            )
            .into()),
        }
    }
}

#[derive(Debug, Eq, PartialEq, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum BankTransferType {
    GbBankTransfer,
    EuBankTransfer,
    #[serde(rename = "bank_transfer")]
    BankTransfers,
}

#[derive(Debug, Eq, PartialEq, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum StripeBankNames {
    AbnAmro,
    ArzteUndApothekerBank,
    AsnBank,
    AustrianAnadiBankAg,
    BankAustria,
    BankhausCarlSpangler,
    BankhausSchelhammerUndSchatteraAg,
    BawagPskAg,
    BksBankAg,
    BrullKallmusBankAg,
    BtvVierLanderBank,
    Bunq,
    CapitalBankGraweGruppeAg,
    CitiHandlowy,
    Dolomitenbank,
    EasybankAg,
    ErsteBankUndSparkassen,
    Handelsbanken,
    HypoAlpeadriabankInternationalAg,
    HypoNoeLbFurNiederosterreichUWien,
    HypoOberosterreichSalzburgSteiermark,
    HypoTirolBankAg,
    HypoVorarlbergBankAg,
    HypoBankBurgenlandAktiengesellschaft,
    Ing,
    Knab,
    MarchfelderBank,
    OberbankAg,
    RaiffeisenBankengruppeOsterreich,
    SchoellerbankAg,
    SpardaBankWien,
    VolksbankGruppe,
    VolkskreditbankAg,
    VrBankBraunau,
    Moneyou,
    Rabobank,
    Regiobank,
    Revolut,
    SnsBank,
    TriodosBank,
    VanLanschot,
    PlusBank,
    EtransferPocztowy24,
    BankiSpbdzielcze,
    BankNowyBfgSa,
    GetinBank,
    Blik,
    NoblePay,
    #[serde(rename = "ideabank")]
    IdeaBank,
    #[serde(rename = "envelobank")]
    EnveloBank,
    NestPrzelew,
    MbankMtransfer,
    Inteligo,
    PbacZIpko,
    BnpParibas,
    BankPekaoSa,
    VolkswagenBank,
    AliorBank,
    Boz,
}

impl TryFrom<&common_enums::BankNames> for StripeBankNames {
    type Error = IntegrationError;
    fn try_from(bank: &common_enums::BankNames) -> Result<Self, Self::Error> {
        Ok(match bank {
            common_enums::BankNames::AbnAmro => Self::AbnAmro,
            common_enums::BankNames::ArzteUndApothekerBank => Self::ArzteUndApothekerBank,
            common_enums::BankNames::AsnBank => Self::AsnBank,
            common_enums::BankNames::AustrianAnadiBankAg => Self::AustrianAnadiBankAg,
            common_enums::BankNames::BankAustria => Self::BankAustria,
            common_enums::BankNames::BankhausCarlSpangler => Self::BankhausCarlSpangler,
            common_enums::BankNames::BankhausSchelhammerUndSchatteraAg => {
                Self::BankhausSchelhammerUndSchatteraAg
            }
            common_enums::BankNames::BawagPskAg => Self::BawagPskAg,
            common_enums::BankNames::BksBankAg => Self::BksBankAg,
            common_enums::BankNames::BrullKallmusBankAg => Self::BrullKallmusBankAg,
            common_enums::BankNames::BtvVierLanderBank => Self::BtvVierLanderBank,
            common_enums::BankNames::Bunq => Self::Bunq,
            common_enums::BankNames::CapitalBankGraweGruppeAg => Self::CapitalBankGraweGruppeAg,
            common_enums::BankNames::Citi => Self::CitiHandlowy,
            common_enums::BankNames::Dolomitenbank => Self::Dolomitenbank,
            common_enums::BankNames::EasybankAg => Self::EasybankAg,
            common_enums::BankNames::ErsteBankUndSparkassen => Self::ErsteBankUndSparkassen,
            common_enums::BankNames::Handelsbanken => Self::Handelsbanken,
            common_enums::BankNames::HypoAlpeadriabankInternationalAg => {
                Self::HypoAlpeadriabankInternationalAg
            }

            common_enums::BankNames::HypoNoeLbFurNiederosterreichUWien => {
                Self::HypoNoeLbFurNiederosterreichUWien
            }
            common_enums::BankNames::HypoOberosterreichSalzburgSteiermark => {
                Self::HypoOberosterreichSalzburgSteiermark
            }
            common_enums::BankNames::HypoTirolBankAg => Self::HypoTirolBankAg,
            common_enums::BankNames::HypoVorarlbergBankAg => Self::HypoVorarlbergBankAg,
            common_enums::BankNames::HypoBankBurgenlandAktiengesellschaft => {
                Self::HypoBankBurgenlandAktiengesellschaft
            }
            common_enums::BankNames::Ing => Self::Ing,
            common_enums::BankNames::Knab => Self::Knab,
            common_enums::BankNames::MarchfelderBank => Self::MarchfelderBank,
            common_enums::BankNames::OberbankAg => Self::OberbankAg,
            common_enums::BankNames::RaiffeisenBankengruppeOsterreich => {
                Self::RaiffeisenBankengruppeOsterreich
            }
            common_enums::BankNames::Rabobank => Self::Rabobank,
            common_enums::BankNames::Regiobank => Self::Regiobank,
            common_enums::BankNames::Revolut => Self::Revolut,
            common_enums::BankNames::SnsBank => Self::SnsBank,
            common_enums::BankNames::TriodosBank => Self::TriodosBank,
            common_enums::BankNames::VanLanschot => Self::VanLanschot,
            common_enums::BankNames::Moneyou => Self::Moneyou,
            common_enums::BankNames::SchoellerbankAg => Self::SchoellerbankAg,
            common_enums::BankNames::SpardaBankWien => Self::SpardaBankWien,
            common_enums::BankNames::VolksbankGruppe => Self::VolksbankGruppe,
            common_enums::BankNames::VolkskreditbankAg => Self::VolkskreditbankAg,
            common_enums::BankNames::VrBankBraunau => Self::VrBankBraunau,
            common_enums::BankNames::PlusBank => Self::PlusBank,
            common_enums::BankNames::EtransferPocztowy24 => Self::EtransferPocztowy24,
            common_enums::BankNames::BankiSpbdzielcze => Self::BankiSpbdzielcze,
            common_enums::BankNames::BankNowyBfgSa => Self::BankNowyBfgSa,
            common_enums::BankNames::GetinBank => Self::GetinBank,
            common_enums::BankNames::Blik => Self::Blik,
            common_enums::BankNames::NoblePay => Self::NoblePay,
            common_enums::BankNames::IdeaBank => Self::IdeaBank,
            common_enums::BankNames::EnveloBank => Self::EnveloBank,
            common_enums::BankNames::NestPrzelew => Self::NestPrzelew,
            common_enums::BankNames::MbankMtransfer => Self::MbankMtransfer,
            common_enums::BankNames::Inteligo => Self::Inteligo,
            common_enums::BankNames::PbacZIpko => Self::PbacZIpko,
            common_enums::BankNames::BnpParibas => Self::BnpParibas,
            common_enums::BankNames::BankPekaoSa => Self::BankPekaoSa,
            common_enums::BankNames::VolkswagenBank => Self::VolkswagenBank,
            common_enums::BankNames::AliorBank => Self::AliorBank,
            common_enums::BankNames::Boz => Self::Boz,

            _ => Err(IntegrationError::NotImplemented(
                (get_unimplemented_payment_method_error_message("stripe")).into(),
                Default::default(),
            ))?,
        })
    }
}

fn validate_shipping_address_against_payment_method(
    shipping_address: &Option<StripeShippingAddress>,
    payment_method: Option<&StripePaymentMethodType>,
) -> Result<(), error_stack::Report<IntegrationError>> {
    match payment_method {
        Some(StripePaymentMethodType::AfterpayClearpay) => match shipping_address {
            Some(address) => {
                let missing_fields = collect_missing_value_keys!(
                    ("shipping.address.line1", address.line1),
                    ("shipping.address.country", address.country),
                    ("shipping.address.zip", address.zip)
                );

                if !missing_fields.is_empty() {
                    return Err(IntegrationError::MissingRequiredFields {
                        field_names: missing_fields,
                        context: Default::default(),
                    }
                    .into());
                }
                Ok(())
            }
            None => Err(IntegrationError::MissingRequiredField {
                field_name: "shipping.address",
                context: Default::default(),
            }
            .into()),
        },
        _ => Ok(()),
    }
}

impl TryFrom<&PayLaterData> for StripePaymentMethodType {
    type Error = IntegrationError;
    fn try_from(pay_later_data: &PayLaterData) -> Result<Self, Self::Error> {
        match pay_later_data {
            PayLaterData::KlarnaRedirect { .. } => Ok(Self::Klarna),
            PayLaterData::AffirmRedirect {} => Ok(Self::Affirm),
            PayLaterData::AfterpayClearpayRedirect { .. } => Ok(Self::AfterpayClearpay),

            PayLaterData::KlarnaSdk { .. }
            | PayLaterData::PayBrightRedirect {}
            | PayLaterData::WalleyRedirect {}
            | PayLaterData::AlmaRedirect {}
            | PayLaterData::AtomeRedirect {} => Err(IntegrationError::NotImplemented(
                (get_unimplemented_payment_method_error_message("stripe")).into(),
                Default::default(),
            )),
        }
    }
}

impl TryFrom<&BankRedirectData> for StripePaymentMethodType {
    type Error = IntegrationError;
    fn try_from(bank_redirect_data: &BankRedirectData) -> Result<Self, Self::Error> {
        match bank_redirect_data {
            BankRedirectData::Giropay { .. } => Ok(Self::Giropay),
            BankRedirectData::Ideal { .. } => Ok(Self::Ideal),
            BankRedirectData::Sofort { .. } => Ok(Self::Sofort),
            BankRedirectData::BancontactCard { .. } => Ok(Self::Bancontact),
            BankRedirectData::Przelewy24 { .. } => Ok(Self::Przelewy24),
            BankRedirectData::Eps { .. } => Ok(Self::Eps),
            BankRedirectData::Blik { .. } => Ok(Self::Blik),
            BankRedirectData::OnlineBankingFpx { .. } => Err(IntegrationError::NotImplemented(
                (get_unimplemented_payment_method_error_message("stripe")).into(),
                Default::default(),
            )),
            BankRedirectData::Bizum {}
            | BankRedirectData::Interac { .. }
            | BankRedirectData::Eft { .. }
            | BankRedirectData::OnlineBankingCzechRepublic { .. }
            | BankRedirectData::OnlineBankingFinland { .. }
            | BankRedirectData::OnlineBankingPoland { .. }
            | BankRedirectData::OnlineBankingSlovakia { .. }
            | BankRedirectData::OnlineBankingThailand { .. }
            | BankRedirectData::OpenBankingUk { .. }
            | BankRedirectData::Trustly { .. }
            | BankRedirectData::LocalBankRedirect {}
            | BankRedirectData::OpenBanking {}
            | BankRedirectData::Netbanking { .. } => Err(IntegrationError::NotImplemented(
                (get_unimplemented_payment_method_error_message("stripe")).into(),
                Default::default(),
            )),
        }
    }
}

fn get_stripe_payment_method_type_from_wallet_data(
    wallet_data: &WalletData,
) -> Result<Option<StripePaymentMethodType>, IntegrationError> {
    match wallet_data {
        WalletData::AliPayRedirect(_) => Ok(Some(StripePaymentMethodType::Alipay)),
        WalletData::ApplePay(_) => Ok(None),
        WalletData::GooglePay(_) => Ok(Some(StripePaymentMethodType::Card)),
        WalletData::WeChatPayQr(_) => Ok(Some(StripePaymentMethodType::Wechatpay)),
        WalletData::CashappQr(_) => Ok(Some(StripePaymentMethodType::Cashapp)),
        WalletData::AmazonPayRedirect(_) => Ok(Some(StripePaymentMethodType::AmazonPay)),
        WalletData::RevolutPay(_) => Ok(Some(StripePaymentMethodType::RevolutPay)),
        WalletData::MobilePayRedirect(_) => Err(IntegrationError::NotImplemented(
            (get_unimplemented_payment_method_error_message("stripe")).into(),
            Default::default(),
        )),
        WalletData::PaypalRedirect(_)
        | WalletData::AliPayQr(_)
        | WalletData::BluecodeRedirect {}
        | WalletData::AliPayHkRedirect(_)
        | WalletData::MomoRedirect(_)
        | WalletData::KakaoPayRedirect(_)
        | WalletData::GoPayRedirect(_)
        | WalletData::GcashRedirect(_)
        | WalletData::ApplePayRedirect(_)
        | WalletData::ApplePayThirdPartySdk(_)
        | WalletData::DanaRedirect {}
        | WalletData::GooglePayRedirect(_)
        | WalletData::GooglePayThirdPartySdk(_)
        | WalletData::MbWayRedirect(_)
        | WalletData::PaypalSdk(_)
        | WalletData::Paze(_)
        | WalletData::SamsungPay(_)
        | WalletData::TwintRedirect {}
        | WalletData::VippsRedirect {}
        | WalletData::TouchNGoRedirect(_)
        | WalletData::SwishQr(_)
        | WalletData::WeChatPayRedirect(_)
        | WalletData::Mifinity(_)
        | WalletData::MbWay(_)
        | WalletData::Satispay(_)
        | WalletData::Wero(_)
        | WalletData::LazyPayRedirect(_)
        | WalletData::PhonePeRedirect(_)
        | WalletData::BillDeskRedirect(_)
        | WalletData::CashfreeRedirect(_)
        | WalletData::PayURedirect(_)
        | WalletData::EaseBuzzRedirect(_) => Err(IntegrationError::NotImplemented(
            (get_unimplemented_payment_method_error_message("stripe")).into(),
            Default::default(),
        )),
    }
}

impl TryFrom<&payment_method_data::BankDebitData> for StripePaymentMethodType {
    type Error = IntegrationError;
    fn try_from(bank_debit_data: &payment_method_data::BankDebitData) -> Result<Self, Self::Error> {
        match bank_debit_data {
            payment_method_data::BankDebitData::AchBankDebit { .. } => Ok(Self::Ach),
            payment_method_data::BankDebitData::SepaBankDebit { .. } => Ok(Self::Sepa),
            payment_method_data::BankDebitData::BecsBankDebit { .. } => Ok(Self::Becs),
            payment_method_data::BankDebitData::BacsBankDebit { .. } => Ok(Self::Bacs),
            payment_method_data::BankDebitData::SepaGuaranteedBankDebit { .. }
            | payment_method_data::BankDebitData::EftBankDebit { .. } => {
                Err(IntegrationError::NotImplemented(
                    (get_unimplemented_payment_method_error_message("stripe")).into(),
                    Default::default(),
                ))
            }
        }
    }
}

fn get_bank_debit_data(
    bank_debit_data: &payment_method_data::BankDebitData,
) -> Result<(Option<StripePaymentMethodType>, Option<BankDebitData>), IntegrationError> {
    match bank_debit_data {
        payment_method_data::BankDebitData::AchBankDebit {
            account_number,
            routing_number,
            ..
        } => {
            let ach_data = BankDebitData::Ach {
                account_holder_type: "individual".to_string(),
                account_number: account_number.to_owned(),
                routing_number: routing_number.to_owned(),
            };
            Ok((Some(StripePaymentMethodType::Ach), Some(ach_data)))
        }
        payment_method_data::BankDebitData::SepaBankDebit { iban, .. } => {
            let sepa_data: BankDebitData = BankDebitData::Sepa {
                iban: iban.to_owned(),
            };
            Ok((Some(StripePaymentMethodType::Sepa), Some(sepa_data)))
        }
        payment_method_data::BankDebitData::BecsBankDebit {
            account_number,
            bsb_number,
            ..
        } => {
            let becs_data = BankDebitData::Becs {
                account_number: account_number.to_owned(),
                bsb_number: bsb_number.to_owned(),
            };
            Ok((Some(StripePaymentMethodType::Becs), Some(becs_data)))
        }
        payment_method_data::BankDebitData::BacsBankDebit {
            account_number,
            sort_code,
            ..
        } => {
            let bacs_data = BankDebitData::Bacs {
                account_number: account_number.to_owned(),
                sort_code: Secret::new(sort_code.clone().expose().replace('-', "")),
            };
            Ok((Some(StripePaymentMethodType::Bacs), Some(bacs_data)))
        }
        payment_method_data::BankDebitData::SepaGuaranteedBankDebit { .. }
        | payment_method_data::BankDebitData::EftBankDebit { .. } => {
            Err(IntegrationError::NotImplemented(
                (get_unimplemented_payment_method_error_message("stripe")).into(),
                Default::default(),
            ))
        }
    }
}

pub struct PaymentRequestDetails {
    pub auth_type: common_enums::AuthenticationType,
    pub is_customer_initiated_mandate_payment: Option<bool>,
    pub billing_address: StripeBillingAddress,
    pub request_incremental_authorization: bool,
    pub request_extended_authorization: Option<bool>,
    pub request_overcapture: Option<StripeRequestOvercaptureBool>,
}

fn create_stripe_payment_method<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
>(
    payment_method_data: &PaymentMethodData<T>,
    payment_request_details: PaymentRequestDetails,
) -> Result<
    (
        StripePaymentMethodData<T>,
        Option<StripePaymentMethodType>,
        StripeBillingAddress,
    ),
    error_stack::Report<IntegrationError>,
> {
    match payment_method_data {
        PaymentMethodData::Card(card_details) => {
            let payment_method_auth_type = match payment_request_details.auth_type {
                common_enums::AuthenticationType::ThreeDs => Auth3ds::Any,
                common_enums::AuthenticationType::NoThreeDs => Auth3ds::Automatic,
            };
            Ok((
                StripePaymentMethodData::try_from((
                    card_details,
                    payment_method_auth_type,
                    payment_request_details.request_incremental_authorization,
                    payment_request_details.request_extended_authorization,
                    payment_request_details.request_overcapture,
                ))?,
                Some(StripePaymentMethodType::Card),
                payment_request_details.billing_address,
            ))
        }
        PaymentMethodData::PayLater(pay_later_data) => {
            let stripe_pm_type = StripePaymentMethodType::try_from(pay_later_data)?;

            Ok((
                StripePaymentMethodData::PayLater(StripePayLaterData {
                    payment_method_data_type: stripe_pm_type,
                }),
                Some(stripe_pm_type),
                payment_request_details.billing_address,
            ))
        }
        PaymentMethodData::BankRedirect(bank_redirect_data) => {
            let billing_address =
                if payment_request_details.is_customer_initiated_mandate_payment == Some(true) {
                    mandatory_parameters_for_sepa_bank_debit_mandates(
                        &Some(payment_request_details.billing_address.to_owned()),
                        payment_request_details.is_customer_initiated_mandate_payment,
                    )?
                } else {
                    payment_request_details.billing_address
                };
            let pm_type = StripePaymentMethodType::try_from(bank_redirect_data)?;
            let bank_redirect_data = StripePaymentMethodData::try_from(bank_redirect_data)?;

            Ok((bank_redirect_data, Some(pm_type), billing_address))
        }
        PaymentMethodData::Wallet(wallet_data) => {
            let pm_type = get_stripe_payment_method_type_from_wallet_data(wallet_data)?;
            let wallet_specific_data = StripePaymentMethodData::try_from(wallet_data)?;
            Ok((
                wallet_specific_data,
                pm_type,
                StripeBillingAddress::default(),
            ))
        }
        PaymentMethodData::BankDebit(bank_debit_data) => {
            let (pm_type, bank_debit_data) = get_bank_debit_data(bank_debit_data)?;

            let pm_data = StripePaymentMethodData::BankDebit(StripeBankDebitData {
                bank_specific_data: bank_debit_data,
            });

            Ok((pm_data, pm_type, payment_request_details.billing_address))
        }
        PaymentMethodData::BankTransfer(bank_transfer_data) => match bank_transfer_data.deref() {
            payment_method_data::BankTransferData::AchBankTransfer {} => Ok((
                StripePaymentMethodData::BankTransfer(StripeBankTransferData::AchBankTransfer(
                    Box::new(AchTransferData {
                        payment_method_data_type: StripePaymentMethodType::CustomerBalance,
                        bank_transfer_type: StripeCreditTransferTypes::AchCreditTransfer,
                        payment_method_type: StripePaymentMethodType::CustomerBalance,
                        balance_funding_type: BankTransferType::BankTransfers,
                    }),
                )),
                None,
                StripeBillingAddress::default(),
            )),
            payment_method_data::BankTransferData::MultibancoBankTransfer {} => Ok((
                StripePaymentMethodData::BankTransfer(
                    StripeBankTransferData::MultibancoBankTransfers(Box::new(
                        MultibancoTransferData {
                            payment_method_data_type: StripeCreditTransferTypes::Multibanco,
                            payment_method_type: StripeCreditTransferTypes::Multibanco,
                            email: payment_request_details.billing_address.email.ok_or(
                                IntegrationError::MissingRequiredField {
                                    field_name: "billing_address.email",
                                    context: Default::default(),
                                },
                            )?,
                        },
                    )),
                ),
                None,
                StripeBillingAddress::default(),
            )),
            payment_method_data::BankTransferData::SepaBankTransfer {} => Ok((
                StripePaymentMethodData::BankTransfer(StripeBankTransferData::SepaBankTransfer(
                    Box::new(SepaBankTransferData {
                        payment_method_data_type: StripePaymentMethodType::CustomerBalance,
                        bank_transfer_type: BankTransferType::EuBankTransfer,
                        balance_funding_type: BankTransferType::BankTransfers,
                        payment_method_type: StripePaymentMethodType::CustomerBalance,
                        country: payment_request_details.billing_address.country.ok_or(
                            IntegrationError::MissingRequiredField {
                                field_name: "billing_address.country",
                                context: Default::default(),
                            },
                        )?,
                    }),
                )),
                Some(StripePaymentMethodType::CustomerBalance),
                payment_request_details.billing_address,
            )),
            payment_method_data::BankTransferData::BacsBankTransfer {} => Ok((
                StripePaymentMethodData::BankTransfer(StripeBankTransferData::BacsBankTransfers(
                    Box::new(BacsBankTransferData {
                        payment_method_data_type: StripePaymentMethodType::CustomerBalance,
                        bank_transfer_type: BankTransferType::GbBankTransfer,
                        balance_funding_type: BankTransferType::BankTransfers,
                        payment_method_type: StripePaymentMethodType::CustomerBalance,
                    }),
                )),
                Some(StripePaymentMethodType::CustomerBalance),
                payment_request_details.billing_address,
            )),
            payment_method_data::BankTransferData::Pix { .. } => {
                Err(IntegrationError::NotImplemented(
                    (get_unimplemented_payment_method_error_message("stripe")).into(),
                    Default::default(),
                )
                .into())
            }
            payment_method_data::BankTransferData::Pse {}
            | payment_method_data::BankTransferData::LocalBankTransfer { .. }
            | payment_method_data::BankTransferData::InstantBankTransfer {}
            | payment_method_data::BankTransferData::InstantBankTransferFinland { .. }
            | payment_method_data::BankTransferData::InstantBankTransferPoland { .. }
            | payment_method_data::BankTransferData::IndonesianBankTransfer { .. }
            | payment_method_data::BankTransferData::PermataBankTransfer { .. }
            | payment_method_data::BankTransferData::BcaBankTransfer { .. }
            | payment_method_data::BankTransferData::BniVaBankTransfer { .. }
            | payment_method_data::BankTransferData::BriVaBankTransfer { .. }
            | payment_method_data::BankTransferData::CimbVaBankTransfer { .. }
            | payment_method_data::BankTransferData::DanamonVaBankTransfer { .. }
            | payment_method_data::BankTransferData::MandiriVaBankTransfer { .. } => {
                Err(IntegrationError::NotImplemented(
                    (get_unimplemented_payment_method_error_message("stripe")).into(),
                    Default::default(),
                )
                .into())
            }
        },
        PaymentMethodData::Crypto(_) => Err(IntegrationError::NotImplemented(
            (get_unimplemented_payment_method_error_message("stripe")).into(),
            Default::default(),
        )
        .into()),

        PaymentMethodData::GiftCard(giftcard_data) => match giftcard_data.deref() {
            GiftCardData::Givex(_) | GiftCardData::PaySafeCard {} => {
                Err(IntegrationError::NotImplemented(
                    (get_unimplemented_payment_method_error_message("stripe")).into(),
                    Default::default(),
                )
                .into())
            }
        },
        PaymentMethodData::CardRedirect(cardredirect_data) => match cardredirect_data {
            CardRedirectData::Knet {}
            | CardRedirectData::Benefit {}
            | CardRedirectData::MomoAtm {}
            | CardRedirectData::CardRedirect {} => Err(IntegrationError::NotImplemented(
                (get_unimplemented_payment_method_error_message("stripe")).into(),
                Default::default(),
            )
            .into()),
        },
        PaymentMethodData::Reward => Err(IntegrationError::NotImplemented(
            (get_unimplemented_payment_method_error_message("stripe")).into(),
            Default::default(),
        )
        .into()),

        PaymentMethodData::Voucher(voucher_data) => match voucher_data {
            VoucherData::Boleto(_) | VoucherData::Oxxo => Err(IntegrationError::NotImplemented(
                (get_unimplemented_payment_method_error_message("stripe")).into(),
                Default::default(),
            )
            .into()),
            VoucherData::Alfamart(_)
            | VoucherData::Efecty
            | VoucherData::PagoEfectivo
            | VoucherData::RedCompra
            | VoucherData::RedPagos
            | VoucherData::Indomaret(_)
            | VoucherData::SevenEleven(_)
            | VoucherData::Lawson(_)
            | VoucherData::MiniStop(_)
            | VoucherData::FamilyMart(_)
            | VoucherData::Seicomart(_)
            | VoucherData::PayEasy(_) => Err(IntegrationError::NotImplemented(
                (get_unimplemented_payment_method_error_message("stripe")).into(),
                Default::default(),
            )
            .into()),
        },

        PaymentMethodData::Upi(_)
        | PaymentMethodData::RealTimePayment(_)
        | PaymentMethodData::MobilePayment(_)
        | PaymentMethodData::MandatePayment
        | PaymentMethodData::OpenBanking(_)
        | PaymentMethodData::PaymentMethodToken(_)
        | PaymentMethodData::NetworkToken(_)
        | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
        | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
            Err(IntegrationError::NotImplemented(
                (get_unimplemented_payment_method_error_message("stripe")).into(),
                Default::default(),
            )
            .into())
        }
    }
}

fn get_stripe_card_network(card_network: common_enums::CardNetwork) -> Option<StripeCardNetwork> {
    match card_network {
        common_enums::CardNetwork::Visa => Some(StripeCardNetwork::Visa),
        common_enums::CardNetwork::Mastercard => Some(StripeCardNetwork::Mastercard),
        common_enums::CardNetwork::CartesBancaires => Some(StripeCardNetwork::CartesBancaires),
        common_enums::CardNetwork::AmericanExpress
        | common_enums::CardNetwork::JCB
        | common_enums::CardNetwork::DinersClub
        | common_enums::CardNetwork::Discover
        | common_enums::CardNetwork::UnionPay
        | common_enums::CardNetwork::Interac
        | common_enums::CardNetwork::RuPay
        | common_enums::CardNetwork::Maestro
        | common_enums::CardNetwork::Star
        | common_enums::CardNetwork::Accel
        | common_enums::CardNetwork::Pulse
        | common_enums::CardNetwork::Nyce => None,
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        &Card<T>,
        Auth3ds,
        bool,
        Option<bool>,
        Option<StripeRequestOvercaptureBool>,
    )> for StripePaymentMethodData<T>
{
    type Error = IntegrationError;
    fn try_from(
        (
            card,
            payment_method_auth_type,
            request_incremental_authorization,
            request_extended_authorization,
            request_overcapture,
        ): (
            &Card<T>,
            Auth3ds,
            bool,
            Option<bool>,
            Option<StripeRequestOvercaptureBool>,
        ),
    ) -> Result<Self, Self::Error> {
        Ok(Self::Card(StripeCardData {
            payment_method_data_type: StripePaymentMethodType::Card,
            payment_method_data_card_number: card.card_number.clone(),
            payment_method_data_card_exp_month: card.card_exp_month.clone(),
            payment_method_data_card_exp_year: card.card_exp_year.clone(),
            payment_method_data_card_cvc: Some(card.card_cvc.clone()),
            payment_method_auth_type: Some(payment_method_auth_type),
            payment_method_data_card_preferred_network: card
                .card_network
                .clone()
                .and_then(get_stripe_card_network),
            request_incremental_authorization: if request_incremental_authorization {
                Some(StripeRequestIncrementalAuthorization::IfAvailable)
            } else {
                None
            },
            request_extended_authorization: if request_extended_authorization.unwrap_or(false) {
                Some(StripeRequestExtendedAuthorization::IfAvailable)
            } else {
                None
            },
            request_overcapture,
        }))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> TryFrom<&WalletData>
    for StripePaymentMethodData<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(wallet_data: &WalletData) -> Result<Self, Self::Error> {
        match wallet_data {
            WalletData::ApplePay(applepay_data) => match applepay_data
                .payment_data
                .get_decrypted_apple_pay_payment_data_optional()
            {
                Some(decrypt_data) => Ok(Self::Wallet(StripeWallet::ApplePayPredecryptToken(
                    Box::new(StripeApplePayPredecrypt {
                        number: decrypt_data.clone().application_primary_account_number,
                        exp_year: decrypt_data.get_four_digit_expiry_year(),
                        exp_month: decrypt_data.get_expiry_month(),
                        eci: decrypt_data.payment_data.eci_indicator.clone(),
                        cryptogram: decrypt_data.payment_data.online_payment_cryptogram.clone(),
                        tokenization_method: "apple_pay".to_string(),
                    }),
                ))),
                None => Ok(Self::Wallet(StripeWallet::ApplepayToken(StripeApplePay {
                    pk_token: applepay_data.get_applepay_decoded_payment_data()?,
                    pk_token_instrument_name: applepay_data.payment_method.pm_type.to_owned(),
                    pk_token_payment_network: applepay_data.payment_method.network.to_owned(),
                    pk_token_transaction_id: Secret::new(
                        applepay_data.transaction_identifier.to_owned(),
                    ),
                }))),
            },
            WalletData::WeChatPayQr(_) => Ok(Self::Wallet(StripeWallet::WechatpayPayment(
                WechatpayPayment {
                    client: WechatClient::Web,
                    payment_method_data_type: StripePaymentMethodType::Wechatpay,
                },
            ))),
            WalletData::AliPayRedirect(_) => {
                Ok(Self::Wallet(StripeWallet::AlipayPayment(AlipayPayment {
                    payment_method_data_type: StripePaymentMethodType::Alipay,
                })))
            }
            WalletData::CashappQr(_) => Ok(Self::Wallet(StripeWallet::Cashapp(CashappPayment {
                payment_method_data_type: StripePaymentMethodType::Cashapp,
            }))),
            WalletData::AmazonPayRedirect(_) => Ok(Self::Wallet(StripeWallet::AmazonpayPayment(
                AmazonpayPayment {
                    payment_method_types: StripePaymentMethodType::AmazonPay,
                },
            ))),
            WalletData::RevolutPay(_) => {
                Ok(Self::Wallet(StripeWallet::RevolutPay(RevolutpayPayment {
                    payment_method_types: StripePaymentMethodType::RevolutPay,
                })))
            }
            WalletData::GooglePay(gpay_data) => Ok(Self::try_from(gpay_data)?),
            WalletData::PaypalRedirect(_) | WalletData::MobilePayRedirect(_) => {
                Err(IntegrationError::NotImplemented(
                    (get_unimplemented_payment_method_error_message("stripe")).into(),
                    Default::default(),
                )
                .into())
            }
            WalletData::AliPayQr(_)
            | WalletData::BluecodeRedirect {}
            | WalletData::AliPayHkRedirect(_)
            | WalletData::MomoRedirect(_)
            | WalletData::KakaoPayRedirect(_)
            | WalletData::GoPayRedirect(_)
            | WalletData::GcashRedirect(_)
            | WalletData::ApplePayRedirect(_)
            | WalletData::ApplePayThirdPartySdk(_)
            | WalletData::DanaRedirect {}
            | WalletData::GooglePayRedirect(_)
            | WalletData::GooglePayThirdPartySdk(_)
            | WalletData::MbWayRedirect(_)
            | WalletData::PaypalSdk(_)
            | WalletData::Paze(_)
            | WalletData::SamsungPay(_)
            | WalletData::TwintRedirect {}
            | WalletData::VippsRedirect {}
            | WalletData::TouchNGoRedirect(_)
            | WalletData::SwishQr(_)
            | WalletData::WeChatPayRedirect(_)
            | WalletData::Mifinity(_)
            | WalletData::MbWay(_)
            | WalletData::Satispay(_)
            | WalletData::Wero(_)
            | WalletData::LazyPayRedirect(_)
            | WalletData::PhonePeRedirect(_)
            | WalletData::BillDeskRedirect(_)
            | WalletData::CashfreeRedirect(_)
            | WalletData::PayURedirect(_)
            | WalletData::EaseBuzzRedirect(_) => Err(IntegrationError::NotImplemented(
                (get_unimplemented_payment_method_error_message("stripe")).into(),
                Default::default(),
            )
            .into()),
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<&BankRedirectData> for StripePaymentMethodData<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(bank_redirect_data: &BankRedirectData) -> Result<Self, Self::Error> {
        let payment_method_data_type = StripePaymentMethodType::try_from(bank_redirect_data)?;
        match bank_redirect_data {
            BankRedirectData::BancontactCard { .. } => Ok(Self::BankRedirect(
                StripeBankRedirectData::StripeBancontactCard(Box::new(StripeBancontactCard {
                    payment_method_data_type,
                })),
            )),
            BankRedirectData::Blik { blik_code } => Ok(Self::BankRedirect(
                StripeBankRedirectData::StripeBlik(Box::new(StripeBlik {
                    payment_method_data_type,
                    code: Secret::new(blik_code.clone().ok_or(
                        IntegrationError::MissingRequiredField {
                            field_name: "blik_code",
                            context: Default::default(),
                        },
                    )?),
                })),
            )),
            BankRedirectData::Eps { bank_name, .. } => Ok(Self::BankRedirect(
                StripeBankRedirectData::StripeEps(Box::new(StripeEps {
                    payment_method_data_type,
                    bank_name: bank_name
                        .map(|bank_name| StripeBankNames::try_from(&bank_name))
                        .transpose()?,
                })),
            )),
            BankRedirectData::Giropay { .. } => Ok(Self::BankRedirect(
                StripeBankRedirectData::StripeGiropay(Box::new(StripeGiropay {
                    payment_method_data_type,
                })),
            )),
            BankRedirectData::Ideal { bank_name, .. } => {
                let bank_name = bank_name
                    .map(|bank_name| StripeBankNames::try_from(&bank_name))
                    .transpose()?;
                Ok(Self::BankRedirect(StripeBankRedirectData::StripeIdeal(
                    Box::new(StripeIdeal {
                        payment_method_data_type,
                        ideal_bank_name: bank_name,
                    }),
                )))
            }
            BankRedirectData::Przelewy24 { bank_name, .. } => {
                let bank_name = bank_name
                    .map(|bank_name| StripeBankNames::try_from(&bank_name))
                    .transpose()?;
                Ok(Self::BankRedirect(
                    StripeBankRedirectData::StripePrezelewy24(Box::new(StripePrezelewy24 {
                        payment_method_data_type,
                        bank_name,
                    })),
                ))
            }
            BankRedirectData::OnlineBankingFpx { .. } => Err(IntegrationError::NotImplemented(
                (get_unimplemented_payment_method_error_message("stripe")).into(),
                Default::default(),
            )
            .into()),
            BankRedirectData::Bizum {}
            | BankRedirectData::Eft { .. }
            | BankRedirectData::Interac { .. }
            | BankRedirectData::OnlineBankingCzechRepublic { .. }
            | BankRedirectData::OnlineBankingFinland { .. }
            | BankRedirectData::OnlineBankingPoland { .. }
            | BankRedirectData::OnlineBankingSlovakia { .. }
            | BankRedirectData::OnlineBankingThailand { .. }
            | BankRedirectData::OpenBankingUk { .. }
            | BankRedirectData::Sofort { .. }
            | BankRedirectData::Trustly { .. }
            | BankRedirectData::LocalBankRedirect {}
            | BankRedirectData::OpenBanking {}
            | BankRedirectData::Netbanking { .. } => Err(IntegrationError::NotImplemented(
                (get_unimplemented_payment_method_error_message("stripe")).into(),
                Default::default(),
            )
            .into()),
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<&GooglePayWalletData> for StripePaymentMethodData<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(gpay_data: &GooglePayWalletData) -> Result<Self, Self::Error> {
        Ok(Self::Wallet(StripeWallet::GooglepayToken(GooglePayToken {
            token: Secret::new(
                gpay_data
                    .tokenization_data
                    .get_encrypted_google_pay_token()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "gpay wallet_token",
                        context: Default::default(),
                    })?
                    .as_bytes()
                    .parse_struct::<StripeGpayToken>("StripeGpayToken")
                    .change_context(IntegrationError::InvalidWalletToken {
                        wallet_name: "Google Pay".to_string(),
                        context: Default::default(),
                    })?
                    .id,
            ),
            payment_type: StripePaymentMethodType::Card,
        })))
    }
}

fn is_setup_future_usage_supported(
    payment_method_type: Option<common_enums::PaymentMethodType>,
) -> bool {
    !matches!(
        payment_method_type,
        Some(common_enums::PaymentMethodType::Affirm)
            | Some(common_enums::PaymentMethodType::Klarna)
    )
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        StripeRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PaymentIntentRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        value: StripeRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = value.router_data;

        let (transfer_account_id, charge_type, application_fees) = (None, None, None);

        let payment_method_token = match &item.request.payment_method_data {
            PaymentMethodData::PaymentMethodToken(t) => Some(t.token.clone()),
            _ => None,
        };

        let amount =
            StripeAmountConvertor::convert(item.request.minor_amount, item.request.currency)?;
        let order_id = item
            .resource_common_data
            .connector_request_reference_id
            .clone();

        let shipping_address = if payment_method_token.is_some() {
            None
        } else {
            Some(StripeShippingAddress {
                city: item.resource_common_data.get_optional_shipping_city(),
                country: item.resource_common_data.get_optional_shipping_country(),
                line1: item.resource_common_data.get_optional_shipping_line1(),
                line2: item.resource_common_data.get_optional_shipping_line2(),
                zip: item.resource_common_data.get_optional_shipping_zip(),
                state: item.resource_common_data.get_optional_shipping_state(),
                name: item.resource_common_data.get_optional_shipping_full_name(),
                phone: item
                    .resource_common_data
                    .get_optional_shipping_phone_number(),
            })
        };

        let billing_address = if payment_method_token.is_some() {
            None
        } else {
            Some(StripeBillingAddress {
                city: item.resource_common_data.get_optional_billing_city(),
                country: item.resource_common_data.get_optional_billing_country(),
                address_line1: item.resource_common_data.get_optional_billing_line1(),
                address_line2: item.resource_common_data.get_optional_billing_line2(),
                zip_code: item.resource_common_data.get_optional_billing_zip(),
                state: item.resource_common_data.get_optional_billing_state(),
                name: item.resource_common_data.get_optional_billing_full_name(),
                email: item.resource_common_data.get_optional_billing_email(),
                phone: item
                    .resource_common_data
                    .get_optional_billing_phone_number(),
            })
        };

        let payment_method_options = None;

        let (
            payment_data,
            payment_method,
            billing_address,
            payment_method_types,
            setup_future_usage,
        ) = if payment_method_token.is_some() {
            let setup_future_usage =
                if is_setup_future_usage_supported(item.request.payment_method_type) {
                    item.request.setup_future_usage
                } else {
                    None
                };

            (
                None,
                None,
                StripeBillingAddress::default(),
                None,
                setup_future_usage,
            )
        } else {
            let (payment_method_data, payment_method_type, billing_address) =
                create_stripe_payment_method(
                    &item.request.payment_method_data,
                    PaymentRequestDetails {
                        auth_type: item.resource_common_data.auth_type,
                        is_customer_initiated_mandate_payment: Some(
                            PaymentsAuthorizeData::is_customer_initiated_mandate_payment(
                                &item.request,
                            ),
                        ),
                        billing_address: billing_address.ok_or(
                            IntegrationError::MissingRequiredField {
                                field_name: "billing_address",
                                context: Default::default(),
                            },
                        )?,
                        request_incremental_authorization: item
                            .request
                            .request_incremental_authorization
                            .unwrap_or(false),
                        request_extended_authorization: item.request.request_extended_authorization,
                        request_overcapture: item
                            .request
                            .enable_overcapture
                            .and_then(get_stripe_overcapture_request),
                    },
                )?;

            validate_shipping_address_against_payment_method(
                &shipping_address,
                payment_method_type.as_ref(),
            )?;

            let setup_future_usage =
                if is_setup_future_usage_supported(item.request.payment_method_type) {
                    item.request.setup_future_usage
                } else {
                    None
                };

            (
                Some(payment_method_data),
                None,
                billing_address,
                payment_method_type,
                setup_future_usage,
            )
        };

        let setup_mandate_details = item
            .request
            .setup_mandate_details
            .as_ref()
            .and_then(|mandate_details| {
                mandate_details
                    .customer_acceptance
                    .as_ref()
                    .map(|customer_acceptance| {
                        Ok::<_, error_stack::Report<IntegrationError>>(
                            match customer_acceptance.acceptance_type {
                                AcceptanceType::Online => {
                                    let online_mandate = customer_acceptance
                                        .online
                                        .clone()
                                        .get_required_value("online")
                                        .change_context(IntegrationError::MissingRequiredField {
                                            field_name: "online",
                                            context: Default::default(),
                                        })?;
                                    StripeMandateRequest {
                                        mandate_type: StripeMandateType::Online {
                                            ip_address: online_mandate
                                                .ip_address
                                                .get_required_value("ip_address")
                                                .change_context(
                                                    IntegrationError::MissingRequiredField {
                                                        field_name: "ip_address",
                                                        context: Default::default(),
                                                    },
                                                )?,
                                            user_agent: online_mandate.user_agent,
                                        },
                                    }
                                }
                                AcceptanceType::Offline => StripeMandateRequest {
                                    mandate_type: StripeMandateType::Offline,
                                },
                            },
                        )
                    })
            })
            .transpose()?
            .or_else(|| {
                //stripe requires us to send mandate_data while making recurring payment through saved bank debit
                if payment_method.is_some() {
                    //check if payment is done through saved payment method
                    match &payment_method_types {
                        //check if payment method is bank debit
                        Some(
                            StripePaymentMethodType::Ach
                            | StripePaymentMethodType::Sepa
                            | StripePaymentMethodType::Becs
                            | StripePaymentMethodType::Bacs,
                        ) => Some(StripeMandateRequest {
                            mandate_type: StripeMandateType::Offline,
                        }),
                        _ => None,
                    }
                } else {
                    None
                }
            });

        let meta_data = get_transaction_metadata(item.request.metadata.clone(), order_id);

        // We pass browser_info only when payment_data exists.
        // Hence, we're pass Null during recurring payments as payment_method_data[type] is not passed
        let browser_info = if payment_data.is_some() && payment_method_token.is_none() {
            item.request
                .browser_info
                .clone()
                .map(StripeBrowserInformation::from)
        } else {
            None
        };

        let charges = match &item.request.split_payments {
            Some(domain_types::connector_types::SplitPaymentsRequest::StripeSplitPayment(
                stripe_split_payment,
            )) => match &stripe_split_payment.charge_type {
                common_enums::PaymentChargeType::Stripe(charge_type) => match charge_type {
                    common_enums::StripeChargeType::Direct => Some(IntentCharges {
                        application_fee_amount: stripe_split_payment.application_fees,
                        destination_account_id: None,
                    }),
                    common_enums::StripeChargeType::Destination => Some(IntentCharges {
                        application_fee_amount: stripe_split_payment.application_fees,
                        destination_account_id: Some(Secret::new(
                            stripe_split_payment.transfer_account_id.clone(),
                        )),
                    }),
                },
            },
            None => None,
        };

        let charges_in = if charges.is_none() {
            match charge_type {
                Some(common_enums::PaymentChargeType::Stripe(
                    common_enums::StripeChargeType::Direct,
                )) => Some(IntentCharges {
                    application_fee_amount: application_fees, // default to 0 if None
                    destination_account_id: None,
                }),
                Some(common_enums::PaymentChargeType::Stripe(
                    common_enums::StripeChargeType::Destination,
                )) => Some(IntentCharges {
                    application_fee_amount: application_fees,
                    destination_account_id: transfer_account_id,
                }),
                _ => None,
            }
        } else {
            charges
        };

        let pm = match (payment_method, payment_method_token.clone()) {
            (Some(method), _) => Some(Secret::new(method)),
            (None, Some(token)) => Some(token),
            (None, None) => None,
        };

        Ok(Self {
            amount,                                      //hopefully we don't loose some cents here
            currency: item.request.currency.to_string(), //we need to copy the value and not transfer ownership
            statement_descriptor_suffix: item
                .request
                .billing_descriptor
                .as_ref()
                .and_then(|descriptor| descriptor.statement_descriptor_suffix.clone()),
            statement_descriptor: item
                .request
                .billing_descriptor
                .as_ref()
                .and_then(|descriptor| descriptor.statement_descriptor.clone()),
            meta_data,
            return_url: item
                .request
                .router_return_url
                .clone()
                .unwrap_or_else(|| "https://juspay.in/".to_string()),
            confirm: true, // Stripe requires confirm to be true if return URL is present
            description: item.resource_common_data.description.clone(),
            shipping: shipping_address,
            billing: billing_address,
            capture_method: StripeCaptureMethod::from(item.request.capture_method),
            payment_data,
            payment_method_options,
            payment_method: pm,
            customer: item
                .resource_common_data
                .connector_customer
                .clone()
                .map(Secret::new),
            setup_mandate_details,
            off_session: item.request.off_session,
            setup_future_usage: match (
                item.request.split_payments.as_ref(),
                item.request.setup_future_usage,
                item.request.customer_acceptance.as_ref(),
            ) {
                (Some(_), Some(usage), Some(_)) => Some(usage),
                _ => setup_future_usage,
            },

            payment_method_types,
            expand: Some(ExpandableObjects::LatestCharge),
            browser_info,
            charges: charges_in,
        })
    }
}

fn get_stripe_overcapture_request(
    enable_overcapture: bool,
) -> Option<StripeRequestOvercaptureBool> {
    match enable_overcapture {
        true => Some(StripeRequestOvercaptureBool::IfAvailable),
        false => None,
    }
}

impl From<BrowserInformation> for StripeBrowserInformation {
    fn from(item: BrowserInformation) -> Self {
        Self {
            ip_address: item.ip_address.map(|ip| Secret::new(ip.to_string())),
            user_agent: item.user_agent,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StripeSplitPaymentRequest {
    pub charge_type: Option<common_enums::PaymentChargeType>,
    pub application_fees: Option<MinorUnit>,
    pub transfer_account_id: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
pub struct PaymentIncrementalAuthRequest {
    pub amount: MinorUnit,
}

#[derive(Clone, Default, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StripePaymentStatus {
    Succeeded,
    Failed,
    #[default]
    Processing,
    #[serde(rename = "requires_action")]
    RequiresCustomerAction,
    #[serde(rename = "requires_payment_method")]
    RequiresPaymentMethod,
    RequiresConfirmation,
    Canceled,
    RequiresCapture,
    Chargeable,
    Consumed,
    Pending,
}

impl From<StripePaymentStatus> for common_enums::AttemptStatus {
    fn from(item: StripePaymentStatus) -> Self {
        match item {
            StripePaymentStatus::Succeeded => Self::Charged,
            StripePaymentStatus::Failed => Self::Failure,
            StripePaymentStatus::Processing => Self::Authorizing,
            StripePaymentStatus::RequiresCustomerAction => Self::AuthenticationPending,
            // Make the payment attempt status as failed
            StripePaymentStatus::RequiresPaymentMethod => Self::Failure,
            StripePaymentStatus::RequiresConfirmation => Self::ConfirmationAwaited,
            StripePaymentStatus::Canceled => Self::Voided,
            StripePaymentStatus::RequiresCapture => Self::Authorized,
            StripePaymentStatus::Chargeable => Self::Authorizing,
            StripePaymentStatus::Consumed => Self::Authorizing,
            StripePaymentStatus::Pending => Self::Pending,
        }
    }
}

#[derive(Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct PaymentIntentResponse {
    pub id: String,
    pub object: String,
    pub amount: MinorUnit,
    #[serde(default, deserialize_with = "deserialize_zero_minor_amount_as_none")]
    // stripe gives amount_captured as 0 for payment intents instead of null
    pub amount_received: Option<MinorUnit>,
    pub amount_capturable: Option<MinorUnit>,
    pub currency: String,
    pub status: StripePaymentStatus,
    pub client_secret: Option<Secret<String>>,
    #[serde(default, with = "common_utils::custom_serde::timestamp::option")]
    pub created: Option<PrimitiveDateTime>,
    pub customer: Option<Secret<String>>,
    pub payment_method: Option<Secret<String>>,
    pub description: Option<String>,
    pub statement_descriptor: Option<String>,
    pub statement_descriptor_suffix: Option<String>,
    pub metadata: StripeMetadata,
    pub next_action: Option<StripeNextActionResponse>,
    pub payment_method_options: Option<StripePaymentMethodOptions>,
    pub last_payment_error: Option<ErrorDetails>,
    pub latest_attempt: Option<LatestAttempt>, //need a merchant to test this
    pub latest_charge: Option<StripeChargeEnum>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct StripeSourceResponse {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ach_credit_transfer: Option<AchCreditTransferResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multibanco: Option<MultibancoCreditTransferResponse>,
    pub receiver: AchReceiverDetails,
    pub status: StripePaymentStatus,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct AchCreditTransferResponse {
    pub account_number: Secret<String>,
    pub bank_name: Secret<String>,
    pub routing_number: Secret<String>,
    pub swift_code: Secret<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct MultibancoCreditTransferResponse {
    pub reference: Secret<String>,
    pub entity: Secret<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct AchReceiverDetails {
    pub amount_received: MinorUnit,
    pub amount_charged: MinorUnit,
}

#[serde_with::skip_serializing_none]
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct SepaAndBacsBankTransferInstructions {
    pub bacs_bank_instructions: Option<BacsFinancialDetails>,
    pub sepa_bank_instructions: Option<SepaFinancialDetails>,
    pub receiver: SepaAndBacsReceiver,
}

#[serde_with::skip_serializing_none]
#[derive(Clone, Debug, Serialize)]
pub struct QrCodeNextInstructions {
    pub image_data_url: Url,
    pub display_to_timestamp: Option<i64>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct SepaAndBacsReceiver {
    pub amount_received: MinorUnit,
    pub amount_remaining: MinorUnit,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct PaymentIntentSyncResponse {
    #[serde(flatten)]
    payment_intent_fields: PaymentIntentResponse,
    pub latest_charge: Option<StripeChargeEnum>,
}

#[derive(Debug, Eq, PartialEq, Deserialize, Clone, Serialize)]
#[serde(untagged)]
pub enum StripeChargeEnum {
    ChargeId(String),
    ChargeObject(Box<StripeCharge>),
}

impl StripeChargeEnum {
    pub fn get_overcapture_status(&self) -> Option<bool> {
        match self {
            Self::ChargeObject(charge_object) => charge_object
                .payment_method_details
                .as_ref()
                .and_then(|payment_method_details| match payment_method_details {
                    StripePaymentMethodDetailsResponse::Card { card } => card
                        .overcapture
                        .as_ref()
                        .and_then(|overcapture| match overcapture.status {
                            Some(StripeOvercaptureStatus::Available) => Some(true),
                            Some(StripeOvercaptureStatus::Unavailable) => Some(false),
                            None => None,
                        }),
                    _ => None,
                }),
            _ => None,
        }
    }

    pub fn get_maximum_capturable_amount(&self) -> Option<MinorUnit> {
        match self {
            Self::ChargeObject(charge_object) => {
                if let Some(payment_method_details) = charge_object.payment_method_details.as_ref()
                {
                    match payment_method_details {
                        StripePaymentMethodDetailsResponse::Card { card } => card
                            .overcapture
                            .clone()
                            .filter(|overcapture| {
                                matches!(
                                    overcapture.status,
                                    Some(StripeOvercaptureStatus::Available)
                                )
                            })
                            .and_then(|overcapture_data| {
                                overcapture_data.maximum_amount_capturable
                            }),
                        _ => None,
                    }
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StripeCharge {
    pub id: String,
    pub payment_method_details: Option<StripePaymentMethodDetailsResponse>,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StripeBankRedirectDetails {
    #[serde(rename = "generated_sepa_debit")]
    attached_payment_method: Option<Secret<String>>,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StripeCashappDetails {
    buyer_id: Option<String>,
    cashtag: Option<String>,
}

impl Deref for PaymentIntentSyncResponse {
    type Target = PaymentIntentResponse;

    fn deref(&self) -> &Self::Target {
        &self.payment_intent_fields
    }
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StripeAdditionalCardDetails {
    checks: Option<Value>,
    three_d_secure: Option<Value>,
    network_transaction_id: Option<String>,
    extended_authorization: Option<StripeExtendedAuthorizationResponse>,
    #[serde(default, with = "custom_serde::timestamp::option")]
    capture_before: Option<PrimitiveDateTime>,
    overcapture: Option<StripeOvercaptureResponse>,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StripeExtendedAuthorizationResponse {
    status: Option<StripeExtendedAuthorizationStatus>,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StripeExtendedAuthorizationStatus {
    Disabled,
    Enabled,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StripeOvercaptureResponse {
    status: Option<StripeOvercaptureStatus>,
    #[serde(default, deserialize_with = "deserialize_zero_minor_amount_as_none")]
    // stripe gives amount_captured as 0 for payment intents instead of null
    maximum_amount_capturable: Option<MinorUnit>,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StripeOvercaptureStatus {
    Available,
    Unavailable,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum StripePaymentMethodDetailsResponse {
    //only ideal and bancontact is supported by stripe for recurring payment in bank redirect
    Ideal {
        ideal: StripeBankRedirectDetails,
    },
    Bancontact {
        bancontact: StripeBankRedirectDetails,
    },

    //other payment method types supported by stripe. To avoid deserialization error.
    Blik,
    Eps,
    Fpx,
    Giropay,
    #[serde(rename = "p24")]
    Przelewy24,
    Card {
        card: StripeAdditionalCardDetails,
    },
    Cashapp {
        cashapp: StripeCashappDetails,
    },
    Klarna,
    Affirm,
    AfterpayClearpay,
    AmazonPay,
    ApplePay,
    #[serde(rename = "us_bank_account")]
    Ach,
    #[serde(rename = "sepa_debit")]
    Sepa,
    #[serde(rename = "au_becs_debit")]
    Becs,
    #[serde(rename = "bacs_debit")]
    Bacs,
    #[serde(rename = "wechat_pay")]
    Wechatpay,
    Alipay,
    CustomerBalance,
    RevolutPay,
}

pub struct AdditionalPaymentMethodDetails {
    pub payment_checks: Option<Value>,
    pub authentication_details: Option<Value>,
    pub extended_authorization: Option<StripeExtendedAuthorizationResponse>,
    pub capture_before: Option<PrimitiveDateTime>,
}

impl From<&AdditionalPaymentMethodDetails> for AdditionalPaymentMethodConnectorResponse {
    fn from(item: &AdditionalPaymentMethodDetails) -> Self {
        Self::Card {
            authentication_data: item.authentication_details.clone(),
            payment_checks: item.payment_checks.clone(),
            card_network: None,
            domestic_network: None,
            auth_code: None,
        }
    }
}

impl StripePaymentMethodDetailsResponse {
    pub fn get_additional_payment_method_data(&self) -> Option<AdditionalPaymentMethodDetails> {
        match self {
            Self::Card { card } => Some(AdditionalPaymentMethodDetails {
                payment_checks: card.checks.clone(),
                authentication_details: card.three_d_secure.clone(),
                extended_authorization: card.extended_authorization.clone(),
                capture_before: card.capture_before,
            }),
            Self::Ideal { .. }
            | Self::Bancontact { .. }
            | Self::Blik
            | Self::Eps
            | Self::Fpx
            | Self::Giropay
            | Self::Przelewy24
            | Self::Klarna
            | Self::Affirm
            | Self::AfterpayClearpay
            | Self::AmazonPay
            | Self::ApplePay
            | Self::Ach
            | Self::Sepa
            | Self::Becs
            | Self::Bacs
            | Self::Wechatpay
            | Self::Alipay
            | Self::CustomerBalance
            | Self::RevolutPay
            | Self::Cashapp { .. } => None,
        }
    }
}

#[derive(Deserialize)]
pub struct SetupIntentSyncResponse {
    #[serde(flatten)]
    setup_intent_fields: SetupMandateResponse,
}

impl Deref for SetupIntentSyncResponse {
    type Target = SetupMandateResponse;

    fn deref(&self) -> &Self::Target {
        &self.setup_intent_fields
    }
}

impl From<SetupIntentSyncResponse> for PaymentIntentSyncResponse {
    fn from(value: SetupIntentSyncResponse) -> Self {
        Self {
            payment_intent_fields: value.setup_intent_fields.into(),
            latest_charge: None,
        }
    }
}

impl From<SetupMandateResponse> for PaymentIntentResponse {
    fn from(value: SetupMandateResponse) -> Self {
        Self {
            id: value.id,
            object: value.object,
            status: value.status,
            client_secret: Some(value.client_secret),
            customer: value.customer,
            description: None,
            statement_descriptor: value.statement_descriptor,
            statement_descriptor_suffix: value.statement_descriptor_suffix,
            metadata: value.metadata,
            next_action: value.next_action,
            payment_method_options: value.payment_method_options,
            last_payment_error: value.last_setup_error,
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct SetupMandateResponse {
    pub id: String,
    pub object: String,
    pub status: StripePaymentStatus, // Change to SetupStatus
    pub client_secret: Secret<String>,
    pub customer: Option<Secret<String>>,
    pub payment_method: Option<String>,
    pub statement_descriptor: Option<String>,
    pub statement_descriptor_suffix: Option<String>,
    pub metadata: StripeMetadata,
    pub next_action: Option<StripeNextActionResponse>,
    pub payment_method_options: Option<StripePaymentMethodOptions>,
    pub latest_attempt: Option<LatestAttempt>,
    pub last_setup_error: Option<ErrorDetails>,
}

fn extract_payment_method_connector_response_from_latest_charge(
    stripe_charge_enum: &StripeChargeEnum,
    created_at: Option<PrimitiveDateTime>,
) -> Option<ConnectorResponseData> {
    let is_overcapture_enabled = stripe_charge_enum.get_overcapture_status();
    let additional_payment_method_details =
        if let StripeChargeEnum::ChargeObject(charge_object) = stripe_charge_enum {
            charge_object
                .payment_method_details
                .as_ref()
                .and_then(StripePaymentMethodDetailsResponse::get_additional_payment_method_data)
        } else {
            None
        };

    let additional_payment_method_data = additional_payment_method_details
        .as_ref()
        .map(AdditionalPaymentMethodConnectorResponse::from);
    let extended_authorization_data =
        additional_payment_method_details
            .as_ref()
            .and_then(|additional_payment_methods_details| {
                get_extended_authorization_data(additional_payment_methods_details, created_at)
            });

    if additional_payment_method_data.is_some()
        || extended_authorization_data.is_some()
        || is_overcapture_enabled.is_some()
    {
        Some(ConnectorResponseData::new(
            additional_payment_method_data,
            is_overcapture_enabled,
            extended_authorization_data,
        ))
    } else {
        None
    }
}

fn get_extended_authorization_data(
    item: &AdditionalPaymentMethodDetails,
    created_at: Option<PrimitiveDateTime>,
) -> Option<ExtendedAuthorizationResponseData> {
    match item
        .extended_authorization
        .as_ref()
        .map(|extended_authorization| {
            matches!(
                extended_authorization.status,
                Some(StripeExtendedAuthorizationStatus::Enabled)
            )
        }) {
        Some(true) => Some(ExtendedAuthorizationResponseData {
            extended_authentication_applied: Some(true),
            capture_before: item.capture_before,
            extended_authorization_last_applied_at: created_at,
        }),
        Some(false) => Some(ExtendedAuthorizationResponseData {
            extended_authentication_applied: Some(false),
            capture_before: None,
            extended_authorization_last_applied_at: None,
        }),
        None => None,
    }
}

impl<F, T> TryFrom<ResponseRouterData<PaymentIntentResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
where
    T: SplitPaymentData + GetRequestIncrementalAuthorization,
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PaymentIntentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let redirect_data = item.response.next_action.clone();
        let redirection_data = redirect_data
            .and_then(|redirection_data| redirection_data.get_url())
            .map(|redirection_url| RedirectForm::from((redirection_url, Method::Get)));

        let mandate_reference = item.response.payment_method.map(|payment_method_id| {
            // Implemented Save and re-use payment information for recurring charges
            // For more info: https://docs.stripe.com/recurring-payments#accept-recurring-payments
            // For backward compatibility payment_method_id & connector_mandate_id is being populated with the same value
            let connector_mandate_id = Some(payment_method_id.clone().expose());
            let payment_method_id = Some(payment_method_id.expose());

            let _mandate_metadata: Option<Secret<Value>> =
                match item.router_data.request.get_split_payment_data() {
                    Some(
                        domain_types::connector_types::SplitPaymentsRequest::StripeSplitPayment(
                            stripe_split_data,
                        ),
                    ) => Some(Secret::new(serde_json::json!({
                        "transfer_account_id": stripe_split_data.transfer_account_id,
                        "charge_type": stripe_split_data.charge_type,
                        "application_fees": stripe_split_data.application_fees
}))),
                    _ => None
};

            MandateReference {
                connector_mandate_id,
                payment_method_id,
                connector_mandate_request_reference_id: None
}
        });

        //Note: we might have to call retrieve_setup_intent to get the network_transaction_id in case its not sent in PaymentIntentResponse
        // Or we identify the mandate txns before hand and always call SetupIntent in case of mandate payment call
        let network_txn_id = match item.response.latest_charge.as_ref() {
            Some(StripeChargeEnum::ChargeObject(charge_object)) => charge_object
                .payment_method_details
                .as_ref()
                .and_then(|payment_method_details| match payment_method_details {
                    StripePaymentMethodDetailsResponse::Card { card } => {
                        card.network_transaction_id.clone()
                    }
                    _ => None,
                }),
            _ => None,
        };

        let connector_metadata = get_connector_metadata(
            item.response.next_action.as_ref(),
            item.response.amount,
            item.http_code,
        )?;

        let status = common_enums::AttemptStatus::from(item.response.status);

        let response = if is_payment_failure(status) {
            *get_stripe_payments_response_data(
                &item.response.last_payment_error,
                item.http_code,
                item.response.id.clone(),
            )
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: redirection_data.map(Box::new),
                mandate_reference: mandate_reference.map(Box::new),
                connector_metadata,
                network_txn_id,
                connector_response_reference_id: Some(item.response.id),
                incremental_authorization_allowed: item
                    .router_data
                    .request
                    .get_request_incremental_authorization(),
                status_code: item.http_code,
            })
        };

        let connector_response_data =
            item.response
                .latest_charge
                .as_ref()
                .and_then(|latest_charge| {
                    extract_payment_method_connector_response_from_latest_charge(
                        latest_charge,
                        item.response.created,
                    )
                });

        let minor_amount_capturable = item
            .response
            .latest_charge
            .as_ref()
            .and_then(StripeChargeEnum::get_maximum_capturable_amount);

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                amount_captured: item
                    .response
                    .amount_received
                    .map(|amount| amount.get_amount_as_i64()),
                minor_amount_captured: item.response.amount_received,
                connector_response: connector_response_data,
                minor_amount_capturable,
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}

impl From<StripePaymentStatus> for common_enums::AuthorizationStatus {
    fn from(item: StripePaymentStatus) -> Self {
        match item {
            StripePaymentStatus::Succeeded
            | StripePaymentStatus::RequiresCapture
            | StripePaymentStatus::Chargeable
            | StripePaymentStatus::RequiresCustomerAction
            | StripePaymentStatus::RequiresConfirmation
            | StripePaymentStatus::Consumed => Self::Success,
            StripePaymentStatus::Processing | StripePaymentStatus::Pending => Self::Processing,
            StripePaymentStatus::Failed
            | StripePaymentStatus::Canceled
            | StripePaymentStatus::RequiresPaymentMethod => Self::Failure,
        }
    }
}

pub fn get_connector_metadata(
    next_action: Option<&StripeNextActionResponse>,
    amount: MinorUnit,
    http_status: u16,
) -> CustomResult<Option<Value>, ConnectorError> {
    let next_action_response = next_action
        .and_then(|next_action_response| match next_action_response {
            StripeNextActionResponse::DisplayBankTransferInstructions(response) => {
                match response.financial_addresses.clone() {
                    FinancialInformation::StripeFinancialInformation(financial_addresses) => {
                        let bank_instructions = financial_addresses.first();
                        let (sepa_bank_instructions, bacs_bank_instructions) = bank_instructions
                            .map_or((None, None), |financial_address| {
                                (
                                    financial_address.iban.to_owned().map(
                                        |sepa_financial_details| SepaFinancialDetails {
                                            account_holder_name: sepa_financial_details
                                                .account_holder_name,
                                            bic: sepa_financial_details.bic,
                                            country: sepa_financial_details.country,
                                            iban: sepa_financial_details.iban,
                                            reference: response.reference.to_owned(),
                                        },
                                    ),
                                    financial_address.sort_code.to_owned(),
                                )
                            });
                        let bank_transfer_instructions = SepaAndBacsBankTransferInstructions {
                            sepa_bank_instructions,
                            bacs_bank_instructions,
                            receiver: SepaAndBacsReceiver {
                                amount_received: amount - response.amount_remaining,
                                amount_remaining: response.amount_remaining,
                            },
                        };

                        Some(bank_transfer_instructions.encode_to_value())
                    }
                    FinancialInformation::AchFinancialInformation(financial_addresses) => {
                        let mut ach_financial_information = HashMap::new();
                        for address in financial_addresses {
                            match address.financial_details {
                                AchFinancialDetails::Aba(aba_details) => {
                                    ach_financial_information
                                        .insert("account_number", aba_details.account_number);
                                    ach_financial_information
                                        .insert("bank_name", aba_details.bank_name);
                                    ach_financial_information
                                        .insert("routing_number", aba_details.routing_number);
                                }
                                AchFinancialDetails::Swift(swift_details) => {
                                    ach_financial_information
                                        .insert("swift_code", swift_details.swift_code);
                                }
                            }
                        }

                        let ach_financial_information_value =
                            serde_json::to_value(ach_financial_information).ok()?;

                        let ach_transfer_instruction =
                            serde_json::from_value::<AchTransfer>(ach_financial_information_value)
                                .ok()?;

                        let bank_transfer_instructions = BankTransferNextStepsData {
                            bank_transfer_instructions: BankTransferInstructions::AchCreditTransfer(
                                Box::new(ach_transfer_instruction),
                            ),
                            receiver: None,
                        };

                        Some(bank_transfer_instructions.encode_to_value())
                    }
                }
            }
            StripeNextActionResponse::WechatPayDisplayQrCode(response) => {
                let wechat_pay_instructions = QrCodeNextInstructions {
                    image_data_url: response.image_data_url.to_owned(),
                    display_to_timestamp: None,
                };

                Some(wechat_pay_instructions.encode_to_value())
            }
            StripeNextActionResponse::CashappHandleRedirectOrDisplayQrCode(response) => {
                let cashapp_qr_instructions: QrCodeNextInstructions = QrCodeNextInstructions {
                    image_data_url: response.qr_code.image_url_png.to_owned(),
                    display_to_timestamp: response.qr_code.expires_at.to_owned(),
                };
                Some(cashapp_qr_instructions.encode_to_value())
            }
            StripeNextActionResponse::MultibancoDisplayDetails(response) => {
                let multibanco_bank_transfer_instructions = BankTransferNextStepsData {
                    bank_transfer_instructions: BankTransferInstructions::Multibanco(Box::new(
                        MultibancoTransferInstructions {
                            reference: response.clone().reference,
                            entity: response.clone().entity.expose(),
                        },
                    )),
                    receiver: None,
                };
                Some(multibanco_bank_transfer_instructions.encode_to_value())
            }
            _ => None,
        })
        .transpose()
        .change_context(crate::utils::response_handling_fail_for_connector(
            http_status,
            "stripe",
        ))?;
    Ok(next_action_response)
}

pub fn get_payment_method_id(
    latest_charge: Option<StripeChargeEnum>,
    payment_method_id_from_intent_root: Secret<String>,
) -> String {
    match latest_charge {
        Some(StripeChargeEnum::ChargeObject(charge)) => match charge.payment_method_details {
            Some(StripePaymentMethodDetailsResponse::Bancontact { bancontact }) => bancontact
                .attached_payment_method
                .map(|attached_payment_method| attached_payment_method.expose())
                .unwrap_or(payment_method_id_from_intent_root.expose()),
            Some(StripePaymentMethodDetailsResponse::Ideal { ideal }) => ideal
                .attached_payment_method
                .map(|attached_payment_method| attached_payment_method.expose())
                .unwrap_or(payment_method_id_from_intent_root.expose()),
            Some(StripePaymentMethodDetailsResponse::Blik)
            | Some(StripePaymentMethodDetailsResponse::Eps)
            | Some(StripePaymentMethodDetailsResponse::Fpx)
            | Some(StripePaymentMethodDetailsResponse::Giropay)
            | Some(StripePaymentMethodDetailsResponse::Przelewy24)
            | Some(StripePaymentMethodDetailsResponse::Card { .. })
            | Some(StripePaymentMethodDetailsResponse::Klarna)
            | Some(StripePaymentMethodDetailsResponse::Affirm)
            | Some(StripePaymentMethodDetailsResponse::AfterpayClearpay)
            | Some(StripePaymentMethodDetailsResponse::AmazonPay)
            | Some(StripePaymentMethodDetailsResponse::ApplePay)
            | Some(StripePaymentMethodDetailsResponse::Ach)
            | Some(StripePaymentMethodDetailsResponse::Sepa)
            | Some(StripePaymentMethodDetailsResponse::Becs)
            | Some(StripePaymentMethodDetailsResponse::Bacs)
            | Some(StripePaymentMethodDetailsResponse::Wechatpay)
            | Some(StripePaymentMethodDetailsResponse::Alipay)
            | Some(StripePaymentMethodDetailsResponse::CustomerBalance)
            | Some(StripePaymentMethodDetailsResponse::Cashapp { .. })
            | Some(StripePaymentMethodDetailsResponse::RevolutPay)
            | None => payment_method_id_from_intent_root.expose(),
        },
        Some(StripeChargeEnum::ChargeId(_)) | None => payment_method_id_from_intent_root.expose(),
    }
}

impl<F> TryFrom<ResponseRouterData<PaymentIntentSyncResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PaymentIntentSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let redirect_data = item.response.next_action.clone();
        let redirection_data = redirect_data
            .and_then(|redirection_data| redirection_data.get_url())
            .map(|redirection_url| RedirectForm::from((redirection_url, Method::Get)));

        let mandate_reference = item
            .response
            .payment_method
            .clone()
            .map(|payment_method_id| {
                // Implemented Save and re-use payment information for recurring charges
                // For more info: https://docs.stripe.com/recurring-payments#accept-recurring-payments
                // For backward compatibility payment_method_id & connector_mandate_id is being populated with the same value
                let payment_method_id =
                    get_payment_method_id(item.response.latest_charge.clone(), payment_method_id);

                MandateReference {
                    connector_mandate_id: Some(payment_method_id.clone()),
                    payment_method_id: Some(payment_method_id),
                    connector_mandate_request_reference_id: None,
                }
            });

        let connector_metadata = get_connector_metadata(
            item.response.next_action.as_ref(),
            item.response.amount,
            item.http_code,
        )?;

        let status = common_enums::AttemptStatus::from(item.response.status.to_owned());

        let connector_response_data =
            item.response
                .latest_charge
                .as_ref()
                .and_then(|latest_charge| {
                    extract_payment_method_connector_response_from_latest_charge(
                        latest_charge,
                        item.response.created,
                    )
                });

        let response = if is_payment_failure(status) {
            *get_stripe_payments_response_data(
                &item.response.payment_intent_fields.last_payment_error,
                item.http_code,
                item.response.id.clone(),
            )
        } else {
            let network_transaction_id = match item.response.latest_charge.clone() {
                Some(StripeChargeEnum::ChargeObject(charge_object)) => charge_object
                    .payment_method_details
                    .and_then(|payment_method_details| match payment_method_details {
                        StripePaymentMethodDetailsResponse::Card { card } => {
                            card.network_transaction_id
                        }
                        _ => None,
                    }),
                _ => None,
            };

            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: redirection_data.map(Box::new),
                mandate_reference: mandate_reference.map(Box::new),
                connector_metadata,
                network_txn_id: network_transaction_id,
                connector_response_reference_id: Some(item.response.id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            })
        };

        let currency_enum =
            common_enums::Currency::from_str(item.response.currency.to_uppercase().as_str())
                .change_context(
                    crate::utils::response_deserialization_fail(
                        item.http_code,
                    "stripe: response body did not match the expected format; confirm API version and connector documentation."),
                )?;
        let amount_in_minor_unit =
            StripeAmountConvertor::convert_back(item.response.amount, currency_enum)
                .change_context(crate::utils::response_handling_fail_for_connector(
                    item.http_code,
                    "stripe",
                ))?;

        let response_integrity_object = PaymentSynIntegrityObject {
            amount: amount_in_minor_unit,
            currency: currency_enum,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: common_enums::AttemptStatus::from(item.response.status.to_owned()),
                amount_captured: item
                    .response
                    .amount_received
                    .map(|amount| amount.get_amount_as_i64()),
                minor_amount_captured: item.response.amount_received,
                connector_response: connector_response_data,
                ..item.router_data.resource_common_data
            },
            request: PaymentsSyncData {
                integrity_object: Some(response_integrity_object),
                ..item.router_data.request
            },
            response,
            ..item.router_data
        })
    }
}

fn extract_payment_method_connector_response_from_latest_attempt(
    stripe_latest_attempt: &LatestAttempt,
) -> Option<ConnectorResponseData> {
    if let LatestAttempt::PaymentIntentAttempt(intent_attempt) = stripe_latest_attempt {
        intent_attempt
            .payment_method_details
            .as_ref()
            .and_then(StripePaymentMethodDetailsResponse::get_additional_payment_method_data)
    } else {
        None
    }
    .as_ref()
    .map(AdditionalPaymentMethodConnectorResponse::from)
    .map(ConnectorResponseData::with_additional_payment_method_data)
}

impl<F, T> TryFrom<ResponseRouterData<SetupMandateResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<SetupMandateResponse, Self>) -> Result<Self, Self::Error> {
        let redirect_data = item.response.next_action.clone();
        let redirection_data = redirect_data
            .and_then(|redirection_data| redirection_data.get_url())
            .map(|redirection_url| RedirectForm::from((redirection_url, Method::Get)));

        let mandate_reference = item.response.payment_method.map(|payment_method_id| {
            // Implemented Save and re-use payment information for recurring charges
            // For more info: https://docs.stripe.com/recurring-payments#accept-recurring-payments
            // For backward compatibility payment_method_id & connector_mandate_id is being populated with the same value
            let connector_mandate_id = Some(payment_method_id.clone());
            let payment_method_id = Some(payment_method_id);
            MandateReference {
                connector_mandate_id,
                payment_method_id,
                connector_mandate_request_reference_id: None,
            }
        });
        let status = common_enums::AttemptStatus::from(item.response.status);
        let connector_response_data = item
            .response
            .latest_attempt
            .as_ref()
            .and_then(extract_payment_method_connector_response_from_latest_attempt);

        let response = if is_payment_failure(status) {
            *get_stripe_payments_response_data(
                &item.response.last_setup_error,
                item.http_code,
                item.response.id.clone(),
            )
        } else {
            let network_transaction_id = match item.response.latest_attempt {
                Some(LatestAttempt::PaymentIntentAttempt(attempt)) => attempt
                    .payment_method_details
                    .and_then(|payment_method_details| match payment_method_details {
                        StripePaymentMethodDetailsResponse::Card { card } => {
                            card.network_transaction_id
                        }
                        _ => None,
                    }),
                _ => None,
            };

            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: redirection_data.map(Box::new),
                mandate_reference: mandate_reference.map(Box::new),
                connector_metadata: None,
                network_txn_id: network_transaction_id,
                connector_response_reference_id: Some(item.response.id),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            })
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_response: connector_response_data,
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}

pub fn stripe_opt_latest_attempt_to_opt_string(
    latest_attempt: Option<LatestAttempt>,
) -> Option<String> {
    match latest_attempt {
        Some(LatestAttempt::PaymentIntentAttempt(attempt)) => attempt
            .payment_method_options
            .and_then(|payment_method_options| match payment_method_options {
                StripePaymentMethodOptions::Card {
                    network_transaction_id,
                    ..
                } => network_transaction_id.map(|network_id| network_id.expose()),
                _ => None,
            }),
        _ => None,
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", remote = "Self")]
pub enum StripeNextActionResponse {
    CashappHandleRedirectOrDisplayQrCode(StripeCashappQrResponse),
    RedirectToUrl(StripeRedirectToUrlResponse),
    AlipayHandleRedirect(StripeRedirectToUrlResponse),
    VerifyWithMicrodeposits(StripeVerifyWithMicroDepositsResponse),
    WechatPayDisplayQrCode(WechatPayRedirectToQr),
    DisplayBankTransferInstructions(StripeBankTransferDetails),
    MultibancoDisplayDetails(MultibancoCreditTransferResponse),
    NoNextActionBody,
}

impl StripeNextActionResponse {
    fn get_url(&self) -> Option<Url> {
        match self {
            Self::RedirectToUrl(redirect_to_url) | Self::AlipayHandleRedirect(redirect_to_url) => {
                Some(redirect_to_url.url.to_owned())
            }
            Self::WechatPayDisplayQrCode(_) => None,
            Self::VerifyWithMicrodeposits(verify_with_microdeposits) => {
                Some(verify_with_microdeposits.hosted_verification_url.to_owned())
            }
            Self::CashappHandleRedirectOrDisplayQrCode(_) => None,
            Self::DisplayBankTransferInstructions(_) => None,
            Self::MultibancoDisplayDetails(_) => None,
            Self::NoNextActionBody => None,
        }
    }
}

// This impl is required because Stripe's response is of the below format, which is externally
// tagged, but also with an extra 'type' field specifying the enum variant name:
// "next_action": {
//   "redirect_to_url": { "return_url": "...", "url": "..." },
//   "type": "redirect_to_url"
// },
// Reference: https://github.com/serde-rs/serde/issues/1343#issuecomment-409698470
impl<'de> Deserialize<'de> for StripeNextActionResponse {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Wrapper {
            #[serde(rename = "type")]
            _ignore: String,
            #[serde(flatten, with = "StripeNextActionResponse")]
            inner: StripeNextActionResponse,
        }

        // There is some exception in the stripe next action, it usually sends :
        // "next_action": {
        //   "redirect_to_url": { "return_url": "...", "url": "..." },
        //   "type": "redirect_to_url"
        // },
        // But there is a case where it only sends the type and not other field named as it's type
        let stripe_next_action_response =
            Wrapper::deserialize(deserializer).map_or(Self::NoNextActionBody, |w| w.inner);

        Ok(stripe_next_action_response)
    }
}

impl Serialize for StripeNextActionResponse {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match *self {
            Self::CashappHandleRedirectOrDisplayQrCode(ref i) => {
                Serialize::serialize(i, serializer)
            }
            Self::RedirectToUrl(ref i) => Serialize::serialize(i, serializer),
            Self::AlipayHandleRedirect(ref i) => Serialize::serialize(i, serializer),
            Self::VerifyWithMicrodeposits(ref i) => Serialize::serialize(i, serializer),
            Self::WechatPayDisplayQrCode(ref i) => Serialize::serialize(i, serializer),
            Self::DisplayBankTransferInstructions(ref i) => Serialize::serialize(i, serializer),
            Self::MultibancoDisplayDetails(ref i) => Serialize::serialize(i, serializer),
            Self::NoNextActionBody => Serialize::serialize("NoNextActionBody", serializer),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct StripeRedirectToUrlResponse {
    return_url: String,
    url: Url,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct WechatPayRedirectToQr {
    // This data contains url, it should be converted to QR code.
    // Note: The url in this data is not redirection url
    data: Url,
    // This is the image source, this image_data_url can directly be used by sdk to show the QR code
    image_data_url: Url,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct StripeVerifyWithMicroDepositsResponse {
    hosted_verification_url: Url,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum FinancialInformation {
    AchFinancialInformation(Vec<AchFinancialInformation>),
    StripeFinancialInformation(Vec<StripeFinancialInformation>),
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct StripeBankTransferDetails {
    pub amount_remaining: MinorUnit,
    pub currency: String,
    pub financial_addresses: FinancialInformation,
    pub hosted_instructions_url: Option<String>,
    pub reference: Option<String>,
    #[serde(rename = "type")]
    pub bank_transfer_type: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct StripeCashappQrResponse {
    pub mobile_auth_url: Url,
    pub qr_code: QrCodeResponse,
    pub hosted_instructions_url: Url,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct QrCodeResponse {
    pub expires_at: Option<i64>,
    pub image_url_png: Url,
    pub image_url_svg: Url,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct AbaDetails {
    pub account_number: Secret<String>,
    pub bank_name: Secret<String>,
    pub routing_number: Secret<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct SwiftDetails {
    pub account_number: Secret<String>,
    pub bank_name: Secret<String>,
    pub swift_code: Secret<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AchFinancialDetails {
    Aba(AbaDetails),
    Swift(SwiftDetails),
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct StripeFinancialInformation {
    pub iban: Option<SepaFinancialDetails>,
    pub sort_code: Option<BacsFinancialDetails>,
    pub supported_networks: Vec<String>,
    #[serde(rename = "type")]
    pub financial_info_type: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct AchFinancialInformation {
    #[serde(flatten)]
    pub financial_details: AchFinancialDetails,
    pub supported_networks: Vec<String>,
    #[serde(rename = "type")]
    pub financial_info_type: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct SepaFinancialDetails {
    pub account_holder_name: Secret<String>,
    pub bic: Secret<String>,
    pub country: Secret<String>,
    pub iban: Secret<String>,
    pub reference: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct BacsFinancialDetails {
    pub account_holder_name: Secret<String>,
    pub account_number: Secret<String>,
    pub sort_code: Secret<String>,
}

// REFUND :
// Type definition for Stripe RefundRequest

#[derive(Debug, Serialize)]
pub struct RefundRequest {
    pub amount: Option<MinorUnit>, //amount in cents, hence passed as integer
    pub payment_intent: String,
    #[serde(flatten)]
    pub meta_data: StripeMetadata,
}

#[derive(Debug, Serialize)]
pub struct ChargeRefundRequest {
    pub charge: String,
    pub refund_application_fee: Option<bool>,
    pub reverse_transfer: Option<bool>,
    pub amount: Option<MinorUnit>, //amount in cents, hence passed as integer
    #[serde(flatten)]
    pub meta_data: StripeMetadata,
}
// Type definition for Stripe Refund Response

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum RefundStatus {
    Succeeded,
    Failed,
    #[default]
    Pending,
    RequiresAction,
}

impl From<RefundStatus> for common_enums::RefundStatus {
    fn from(item: RefundStatus) -> Self {
        match item {
            RefundStatus::Succeeded => Self::Success,
            RefundStatus::Failed => Self::Failure,
            RefundStatus::Pending => Self::Pending,
            RefundStatus::RequiresAction => Self::ManualReview,
        }
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct RefundResponse {
    pub id: String,
    pub object: String,
    pub amount: MinorUnit,
    pub currency: String,
    pub metadata: StripeMetadata,
    pub payment_intent: String,
    pub status: RefundStatus,
    pub failure_reason: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct ErrorDetails {
    pub code: Option<String>,
    #[serde(rename = "type")]
    pub error_type: Option<String>,
    pub message: Option<String>,
    pub param: Option<String>,
    pub decline_code: Option<String>,
    pub payment_intent: Option<PaymentIntentErrorResponse>,
    pub network_advice_code: Option<String>,
    pub network_decline_code: Option<String>,
    pub advice_code: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct PaymentIntentErrorResponse {
    pub id: String,
}

#[derive(Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct ErrorResponse {
    pub error: ErrorDetails,
}

#[derive(Debug, Default, Eq, PartialEq, Serialize)]
pub struct StripeShippingAddress {
    #[serde(rename = "shipping[address][city]")]
    pub city: Option<Secret<String>>,
    #[serde(rename = "shipping[address][country]")]
    pub country: Option<common_enums::CountryAlpha2>,
    #[serde(rename = "shipping[address][line1]")]
    pub line1: Option<Secret<String>>,
    #[serde(rename = "shipping[address][line2]")]
    pub line2: Option<Secret<String>>,
    #[serde(rename = "shipping[address][postal_code]")]
    pub zip: Option<Secret<String>>,
    #[serde(rename = "shipping[address][state]")]
    pub state: Option<Secret<String>>,
    #[serde(rename = "shipping[name]")]
    pub name: Option<Secret<String>>,
    #[serde(rename = "shipping[phone]")]
    pub phone: Option<Secret<String>>,
}

#[derive(Debug, Clone, Default, Eq, PartialEq, Serialize)]
pub struct StripeBillingAddress {
    #[serde(rename = "payment_method_data[billing_details][email]")]
    pub email: Option<Email>,
    #[serde(rename = "payment_method_data[billing_details][address][country]")]
    pub country: Option<common_enums::CountryAlpha2>,
    #[serde(rename = "payment_method_data[billing_details][name]")]
    pub name: Option<Secret<String>>,
    #[serde(rename = "payment_method_data[billing_details][address][city]")]
    pub city: Option<Secret<String>>,
    #[serde(rename = "payment_method_data[billing_details][address][line1]")]
    pub address_line1: Option<Secret<String>>,
    #[serde(rename = "payment_method_data[billing_details][address][line2]")]
    pub address_line2: Option<Secret<String>>,
    #[serde(rename = "payment_method_data[billing_details][address][postal_code]")]
    pub zip_code: Option<Secret<String>>,
    #[serde(rename = "payment_method_data[billing_details][address][state]")]
    pub state: Option<Secret<String>>,
    #[serde(rename = "payment_method_data[billing_details][phone]")]
    pub phone: Option<Secret<String>>,
}

#[derive(Debug, Clone, serde::Deserialize, Eq, PartialEq)]
pub struct StripeRedirectResponse {
    pub payment_intent: Option<String>,
    pub payment_intent_client_secret: Option<Secret<String>>,
    pub source_redirect_slug: Option<String>,
    pub redirect_status: Option<StripePaymentStatus>,
    pub source_type: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
pub struct CancelRequest {
    cancellation_reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UpdateMetadataRequest {
    #[serde(flatten)]
    pub metadata: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
#[serde(untagged)]
pub enum StripePaymentMethodOptions {
    Card {
        mandate_options: Option<StripeMandateOptions>,
        #[serde(rename = "payment_method_options[card][network_transaction_id]")]
        network_transaction_id: Option<Secret<String>>,
        #[serde(flatten)]
        mit_exemption: Option<MitExemption>, // To be used for MIT mandate txns
    },
    Klarna {},
    Affirm {},
    AfterpayClearpay {},
    AmazonPay {},
    Eps {},
    Giropay {},
    Ideal {},
    Sofort {},
    #[serde(rename = "us_bank_account")]
    Ach {},
    #[serde(rename = "sepa_debit")]
    Sepa {},
    #[serde(rename = "au_becs_debit")]
    Becs {},
    #[serde(rename = "bacs_debit")]
    Bacs {},
    Bancontact {},
    WechatPay {},
    Alipay {},
    #[serde(rename = "p24")]
    Przelewy24 {},
    CustomerBalance {},
    Multibanco {},
    Blik {},
    Cashapp {},
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct MitExemption {
    #[serde(rename = "payment_method_options[card][mit_exemption][network_transaction_id]")]
    pub network_transaction_id: Secret<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum LatestAttempt {
    PaymentIntentAttempt(Box<LatestPaymentAttempt>),
    SetupAttempt(String),
}
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct LatestPaymentAttempt {
    pub payment_method_options: Option<StripePaymentMethodOptions>,
    pub payment_method_details: Option<StripePaymentMethodDetailsResponse>,
}

// #[derive(Deserialize, Debug, Clone, Eq, PartialEq)]
// pub struct Card
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct StripeMandateOptions {
    reference: Secret<String>, // Extendable, But only important field to be captured
}
/// Represents the capture request body for stripe connector.
#[derive(Debug, Serialize, Clone, Copy)]
pub struct CaptureRequest {
    /// If amount_to_capture is None stripe captures the amount in the payment intent.
    amount_to_capture: Option<MinorUnit>,
}

impl TryFrom<MinorUnit> for CaptureRequest {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(capture_amount: MinorUnit) -> Result<Self, Self::Error> {
        Ok(Self {
            amount_to_capture: Some(capture_amount),
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct WebhookEventDataResource {
    pub object: Value,
}

#[derive(Debug, Deserialize)]
pub struct WebhookEventObjectResource {
    pub data: WebhookEventDataResource,
}

#[derive(Debug, Deserialize)]
pub struct WebhookEvent {
    #[serde(rename = "type")]
    pub event_type: WebhookEventType,
    #[serde(rename = "data")]
    pub event_data: WebhookEventData,
}

#[derive(Debug, Deserialize)]
pub struct WebhookEventTypeBody {
    #[serde(rename = "type")]
    pub event_type: WebhookEventType,
    #[serde(rename = "data")]
    pub event_data: WebhookStatusData,
}

#[derive(Debug, Deserialize)]
pub struct WebhookEventData {
    #[serde(rename = "object")]
    pub event_object: WebhookEventObjectData,
}

#[derive(Debug, Deserialize)]
pub struct WebhookStatusData {
    #[serde(rename = "object")]
    pub event_object: WebhookStatusObjectData,
}

#[derive(Debug, Deserialize)]
pub struct WebhookStatusObjectData {
    pub status: Option<WebhookEventStatus>,
    pub payment_method_details: Option<WebhookPaymentMethodDetails>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebhookPaymentMethodType {
    AchCreditTransfer,
    MultibancoBankTransfers,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
pub struct WebhookPaymentMethodDetails {
    #[serde(rename = "type")]
    pub payment_method: WebhookPaymentMethodType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEventObjectData {
    pub id: String,
    pub object: WebhookEventObjectType,
    pub amount: Option<MinorUnit>,
    #[serde(default, deserialize_with = "convert_uppercase")]
    pub currency: common_enums::Currency,
    pub payment_intent: Option<String>,
    pub client_secret: Option<Secret<String>>,
    pub reason: Option<String>,
    #[serde(with = "common_utils::custom_serde::timestamp")]
    pub created: PrimitiveDateTime,
    pub evidence_details: Option<EvidenceDetails>,
    pub status: Option<WebhookEventStatus>,
    pub metadata: Option<StripeMetadata>,
    pub last_payment_error: Option<ErrorDetails>,
}

#[derive(Debug, Clone, Serialize, Deserialize, strum::Display)]
#[serde(rename_all = "snake_case")]
pub enum WebhookEventObjectType {
    PaymentIntent,
    Dispute,
    Charge,
    Source,
    Refund,
}

#[derive(Debug, Deserialize)]
pub enum WebhookEventType {
    #[serde(rename = "payment_intent.payment_failed")]
    PaymentIntentFailed,
    #[serde(rename = "payment_intent.succeeded")]
    PaymentIntentSucceed,
    #[serde(rename = "charge.dispute.created")]
    DisputeCreated,
    #[serde(rename = "charge.dispute.closed")]
    DisputeClosed,
    #[serde(rename = "charge.dispute.updated")]
    DisputeUpdated,
    #[serde(rename = "charge.dispute.funds_reinstated")]
    ChargeDisputeFundsReinstated,
    #[serde(rename = "charge.dispute.funds_withdrawn")]
    ChargeDisputeFundsWithdrawn,
    #[serde(rename = "charge.expired")]
    ChargeExpired,
    #[serde(rename = "charge.failed")]
    ChargeFailed,
    #[serde(rename = "charge.pending")]
    ChargePending,
    #[serde(rename = "charge.captured")]
    ChargeCaptured,
    #[serde(rename = "charge.refund.updated")]
    ChargeRefundUpdated,
    #[serde(rename = "charge.succeeded")]
    ChargeSucceeded,
    #[serde(rename = "charge.updated")]
    ChargeUpdated,
    #[serde(rename = "charge.refunded")]
    ChargeRefunded,
    #[serde(rename = "payment_intent.canceled")]
    PaymentIntentCanceled,
    #[serde(rename = "payment_intent.created")]
    PaymentIntentCreated,
    #[serde(rename = "payment_intent.processing")]
    PaymentIntentProcessing,
    #[serde(rename = "payment_intent.requires_action")]
    PaymentIntentRequiresAction,
    #[serde(rename = "payment_intent.amount_capturable_updated")]
    PaymentIntentAmountCapturableUpdated,
    #[serde(rename = "source.chargeable")]
    SourceChargeable,
    #[serde(rename = "source.transaction.created")]
    SourceTransactionCreated,
    #[serde(rename = "payment_intent.partially_funded")]
    PaymentIntentPartiallyFunded,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, strum::Display, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WebhookEventStatus {
    WarningNeedsResponse,
    WarningClosed,
    WarningUnderReview,
    Won,
    Lost,
    NeedsResponse,
    UnderReview,
    ChargeRefunded,
    Succeeded,
    RequiresPaymentMethod,
    RequiresConfirmation,
    RequiresAction,
    Processing,
    RequiresCapture,
    Canceled,
    Chargeable,
    Failed,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvidenceDetails {
    #[serde(with = "common_utils::custom_serde::timestamp")]
    pub due_by: PrimitiveDateTime,
}

#[derive(Debug, Deserialize)]
pub struct StripeGpayToken {
    pub id: String,
}

#[derive(Debug, Serialize)]
pub struct StripeFileRequest {
    pub purpose: &'static str,
    #[serde(skip)]
    pub file: Vec<u8>,
    #[serde(skip)]
    pub file_key: String,
    #[serde(skip)]
    pub file_type: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FileUploadResponse {
    #[serde(rename = "id")]
    pub file_id: String,
}

#[derive(Debug, Serialize)]
pub struct Evidence {
    #[serde(rename = "evidence[access_activity_log]")]
    pub access_activity_log: Option<String>,
    #[serde(rename = "evidence[billing_address]")]
    pub billing_address: Option<Secret<String>>,
    #[serde(rename = "evidence[cancellation_policy]")]
    pub cancellation_policy: Option<String>,
    #[serde(rename = "evidence[cancellation_policy_disclosure]")]
    pub cancellation_policy_disclosure: Option<String>,
    #[serde(rename = "evidence[cancellation_rebuttal]")]
    pub cancellation_rebuttal: Option<String>,
    #[serde(rename = "evidence[customer_communication]")]
    pub customer_communication: Option<String>,
    #[serde(rename = "evidence[customer_email_address]")]
    pub customer_email_address: Option<Secret<String, pii::EmailStrategy>>,
    #[serde(rename = "evidence[customer_name]")]
    pub customer_name: Option<Secret<String>>,
    #[serde(rename = "evidence[customer_purchase_ip]")]
    pub customer_purchase_ip: Option<Secret<String, pii::IpAddress>>,
    #[serde(rename = "evidence[customer_signature]")]
    pub customer_signature: Option<Secret<String>>,
    #[serde(rename = "evidence[product_description]")]
    pub product_description: Option<String>,
    #[serde(rename = "evidence[receipt]")]
    pub receipt: Option<Secret<String>>,
    #[serde(rename = "evidence[refund_policy]")]
    pub refund_policy: Option<String>,
    #[serde(rename = "evidence[refund_policy_disclosure]")]
    pub refund_policy_disclosure: Option<String>,
    #[serde(rename = "evidence[refund_refusal_explanation]")]
    pub refund_refusal_explanation: Option<String>,
    #[serde(rename = "evidence[service_date]")]
    pub service_date: Option<String>,
    #[serde(rename = "evidence[service_documentation]")]
    pub service_documentation: Option<String>,
    #[serde(rename = "evidence[shipping_address]")]
    pub shipping_address: Option<Secret<String>>,
    #[serde(rename = "evidence[shipping_carrier]")]
    pub shipping_carrier: Option<String>,
    #[serde(rename = "evidence[shipping_date]")]
    pub shipping_date: Option<String>,
    #[serde(rename = "evidence[shipping_documentation]")]
    pub shipping_documentation: Option<Secret<String>>,
    #[serde(rename = "evidence[shipping_tracking_number]")]
    pub shipping_tracking_number: Option<Secret<String>>,
    #[serde(rename = "evidence[uncategorized_file]")]
    pub uncategorized_file: Option<String>,
    #[serde(rename = "evidence[uncategorized_text]")]
    pub uncategorized_text: Option<String>,
    pub submit: bool,
}

// Mandates for bank redirects - ideal happens through sepa direct debit in stripe
fn mandatory_parameters_for_sepa_bank_debit_mandates(
    billing_details: &Option<StripeBillingAddress>,
    is_customer_initiated_mandate_payment: Option<bool>,
) -> Result<StripeBillingAddress, IntegrationError> {
    let billing_name = billing_details
        .clone()
        .and_then(|billing_data| billing_data.name.clone());

    let billing_email = billing_details
        .clone()
        .and_then(|billing_data| billing_data.email.clone());
    match is_customer_initiated_mandate_payment {
        Some(true) => Ok(StripeBillingAddress {
            name: Some(billing_name.ok_or(IntegrationError::MissingRequiredField {
                field_name: "billing_name",
                context: Default::default(),
            })?),

            email: Some(billing_email.ok_or(IntegrationError::MissingRequiredField {
                field_name: "billing_email",
                context: Default::default(),
            })?),
            ..StripeBillingAddress::default()
        }),
        Some(false) | None => Ok(StripeBillingAddress {
            name: billing_name,
            email: billing_email,
            ..StripeBillingAddress::default()
        }),
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DisputeObj {
    #[serde(rename = "id")]
    pub dispute_id: String,
    pub status: String,
}

fn get_transaction_metadata(
    merchant_metadata: Option<Secret<Value>>,
    order_id: String,
) -> HashMap<String, String> {
    let mut meta_data = HashMap::from([("metadata[order_id]".to_string(), order_id)]);
    if let Some(metadata) = merchant_metadata {
        let hashmap: HashMap<String, Value> =
            serde_json::from_str(&metadata.peek().to_string()).unwrap_or(HashMap::new());

        for (key, value) in hashmap {
            let metadata_value = match value {
                Value::String(string_value) => string_value,
                value_data => value_data.to_string(),
            };
            meta_data.insert(format!("metadata[{key}]"), metadata_value);
        }
    };
    meta_data
}

fn get_stripe_payments_response_data(
    response: &Option<ErrorDetails>,
    http_code: u16,
    response_id: String,
) -> Box<Result<PaymentsResponseData, domain_types::router_data::ErrorResponse>> {
    let (code, error_message) = match response {
        Some(error_details) => (
            error_details
                .code
                .to_owned()
                .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
            error_details
                .message
                .to_owned()
                .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
        ),
        None => (
            consts::NO_ERROR_CODE.to_string(),
            consts::NO_ERROR_MESSAGE.to_string(),
        ),
    };

    Box::new(Err(domain_types::router_data::ErrorResponse {
        code,
        message: error_message.clone(),
        reason: response.clone().and_then(|res| {
            res.decline_code
                .clone()
                .map(|decline_code| {
                    format!("message - {error_message}, decline_code - {decline_code}")
                })
                .or(Some(error_message.clone()))
        }),
        status_code: http_code,
        attempt_status: None,
        connector_transaction_id: Some(response_id),
        network_advice_code: response
            .as_ref()
            .and_then(|res| res.network_advice_code.clone()),
        network_decline_code: response
            .as_ref()
            .and_then(|res| res.network_decline_code.clone()),
        network_error_message: response
            .as_ref()
            .and_then(|res| res.decline_code.clone().or(res.advice_code.clone())),
    }))
}

pub fn construct_charge_response<T>(
    charge_id: String,
    request: &T,
) -> Option<domain_types::connector_types::ConnectorChargeResponseData>
where
    T: SplitPaymentData,
{
    let charge_request = request.get_split_payment_data();
    if let Some(domain_types::connector_types::SplitPaymentsRequest::StripeSplitPayment(
        stripe_split_payment,
    )) = charge_request
    {
        let stripe_charge_response = domain_types::connector_types::StripeChargeResponseData {
            charge_id: Some(charge_id),
            charge_type: stripe_split_payment.charge_type,
            application_fees: stripe_split_payment.application_fees,
            transfer_account_id: stripe_split_payment.transfer_account_id,
        };
        Some(
            domain_types::connector_types::ConnectorChargeResponseData::StripeSplitPayment(
                stripe_charge_response,
            ),
        )
    } else {
        None
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    > for StripeSplitPaymentRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            charge_type: None,
            transfer_account_id: None,
            application_fees: None,
        })
    }
}

pub(super) fn transform_headers_for_connect_platform(
    charge_type: common_enums::PaymentChargeType,
    transfer_account_id: Secret<String>,
    header: &mut Vec<(String, Maskable<String>)>,
) {
    if let common_enums::PaymentChargeType::Stripe(common_enums::StripeChargeType::Direct) =
        charge_type
    {
        let mut customer_account_header = vec![(
            STRIPE_COMPATIBLE_CONNECT_ACCOUNT.to_string(),
            transfer_account_id.into_masked(),
        )];
        header.append(&mut customer_account_header);
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum PaymentSyncResponse {
    PaymentIntentSyncResponse(PaymentIntentSyncResponse),
    SetupMandateResponse(SetupMandateResponse),
}

impl<F> TryFrom<ResponseRouterData<PaymentSyncResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<PaymentSyncResponse, Self>) -> Result<Self, Self::Error> {
        // Untagged serde already disambiguates PI vs setup intent; prev code of routing on connector_transaction_id could fail sync when the txn id is missing or not a ConnectorTransactionId.
        match item.response {
            PaymentSyncResponse::SetupMandateResponse(setup_intent_response) => {
                Self::try_from(ResponseRouterData {
                    response: setup_intent_response,
                    router_data: item.router_data,
                    http_code: item.http_code,
                })
            }
            PaymentSyncResponse::PaymentIntentSyncResponse(payment_intent_sync_response) => {
                Self::try_from(ResponseRouterData {
                    response: payment_intent_sync_response,
                    router_data: item.router_data,
                    http_code: item.http_code,
                })
            }
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PaymentsAuthorizeResponse(PaymentIntentResponse);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<PaymentsAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PaymentsAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let currency_enum =
            common_enums::Currency::from_str(item.response.0.currency.to_uppercase().as_str())
                .change_context(
                    crate::utils::response_deserialization_fail(
                        item.http_code,
                    "stripe: response body did not match the expected format; confirm API version and connector documentation."),
                )?;

        let amount_in_minor_unit =
            StripeAmountConvertor::convert_back(item.response.0.amount, currency_enum)
                .change_context(crate::utils::response_handling_fail_for_connector(
                    item.http_code,
                    "stripe",
                ))?;

        let response_integrity_object = AuthoriseIntegrityObject {
            amount: amount_in_minor_unit,
            currency: currency_enum,
        };

        let new_router_data = Self::try_from(ResponseRouterData {
            response: item.response.0,
            router_data: item.router_data,
            http_code: item.http_code,
        })
        .change_context(crate::utils::response_handling_fail_for_connector(
            item.http_code,
            "stripe",
        ));

        new_router_data.map(|mut router_data| {
            router_data.request.integrity_object = Some(response_integrity_object);
            router_data
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PaymentsCaptureResponse(PaymentIntentResponse);

impl TryFrom<ResponseRouterData<PaymentsCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PaymentsCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let currency_enum =
            common_enums::Currency::from_str(item.response.0.currency.to_uppercase().as_str())
                .change_context(
                    crate::utils::response_deserialization_fail(
                        item.http_code,
                    "stripe: response body did not match the expected format; confirm API version and connector documentation."),
                )?;

        let capture_amount_in_minor_unit = item
            .response
            .0
            .amount_received
            .map(|amount| StripeAmountConvertor::convert_back(amount, currency_enum))
            .transpose()
            .change_context(crate::utils::response_handling_fail_for_connector(
                item.http_code,
                "stripe",
            ))?;

        let response_integrity_object =
            capture_amount_in_minor_unit.map(|amount_to_capture| CaptureIntegrityObject {
                amount_to_capture,
                currency: currency_enum,
            });

        let new_router_data = Self::try_from(ResponseRouterData {
            response: item.response.0,
            router_data: item.router_data,
            http_code: item.http_code,
        })
        .change_context(crate::utils::response_handling_fail_for_connector(
            item.http_code,
            "stripe",
        ));

        new_router_data.map(|mut router_data| {
            router_data.request.integrity_object = response_integrity_object;
            router_data
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PaymentsVoidResponse(PaymentIntentResponse);

impl TryFrom<ResponseRouterData<PaymentsVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<PaymentsVoidResponse, Self>) -> Result<Self, Self::Error> {
        Self::try_from(ResponseRouterData {
            response: item.response.0,
            router_data: item.router_data,
            http_code: item.http_code,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        StripeRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for CancelRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: StripeRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            cancellation_reason: item.router_data.request.cancellation_reason.clone(),
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        StripeRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for CaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: StripeRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount_to_capture = StripeAmountConvertor::convert(
            item.router_data.request.minor_amount_to_capture,
            item.router_data.request.currency,
        )?;
        Ok(Self {
            amount_to_capture: Some(amount_to_capture),
        })
    }
}

impl<F> TryFrom<ResponseRouterData<PaymentIntentResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsIncrementalAuthorizationData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PaymentIntentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = common_enums::AuthorizationStatus::from(item.response.status);
        Ok(Self {
            response: Ok(PaymentsResponseData::IncrementalAuthorizationResponse {
                status,
                connector_authorization_id: Some(item.response.id),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum StripeRefundRequest {
    RefundRequest(RefundRequest),
    ChargeRefundRequest(ChargeRefundRequest),
}

impl<F, T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<StripeRouterData<RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>, T>>
    for StripeRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: StripeRouterData<
            RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let refund_amount = StripeAmountConvertor::convert(
            item.router_data.request.minor_refund_amount,
            item.router_data.request.currency,
        )?;
        match item.router_data.request.split_refunds.as_ref() {
            Some(domain_types::connector_types::SplitRefundsRequest::StripeSplitRefund(_)) => Ok(
                Self::ChargeRefundRequest(ChargeRefundRequest::try_from(&item.router_data)?),
            ),
            _ => Ok(Self::RefundRequest(RefundRequest::try_from((
                &item.router_data,
                refund_amount,
            ))?)),
        }
    }
}

impl<F>
    TryFrom<(
        &RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>,
        MinorUnit,
    )> for RefundRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        (item, refund_amount): (
            &RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>,
            MinorUnit,
        ),
    ) -> Result<Self, Self::Error> {
        let payment_intent = item.request.connector_transaction_id.clone();
        Ok(Self {
            amount: Some(refund_amount),
            payment_intent,
            meta_data: StripeMetadata {
                order_id: Some(item.request.refund_id.clone()),
                is_refund_id_as_reference: Some("true".to_string()),
            },
        })
    }
}

impl<F> TryFrom<&RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>>
    for ChargeRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: &RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>,
    ) -> Result<Self, Self::Error> {
        let amount = item.request.minor_refund_amount;
        match item.request.split_refunds.as_ref() {
            None => Err(IntegrationError::MissingRequiredField {
                field_name: "split_refunds",
                context: Default::default(),
            }
            .into()),

            Some(split_refunds) => match split_refunds {
                domain_types::connector_types::SplitRefundsRequest::StripeSplitRefund(
                    stripe_refund,
                ) => {
                    let (refund_application_fee, reverse_transfer) = match &stripe_refund.options {
                        domain_types::connector_types::ChargeRefundsOptions::Direct(
                            domain_types::connector_types::DirectChargeRefund {
                                revert_platform_fee,
                            },
                        ) => (Some(*revert_platform_fee), None),
                        domain_types::connector_types::ChargeRefundsOptions::Destination(
                            domain_types::connector_types::DestinationChargeRefund {
                                revert_platform_fee,
                                revert_transfer,
                            },
                        ) => (Some(*revert_platform_fee), Some(*revert_transfer)),
                    };

                    Ok(Self {
                        charge: stripe_refund.charge_id.clone(),
                        refund_application_fee,
                        reverse_transfer,
                        amount: Some(amount),
                        meta_data: StripeMetadata {
                            order_id: Some(item.request.refund_id.clone()),
                            is_refund_id_as_reference: Some("true".to_string()),
                        },
                    })
                }
            },
        }
    }
}

impl<F> TryFrom<ResponseRouterData<RefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<RefundResponse, Self>) -> Result<Self, Self::Error> {
        let refund_status = common_enums::RefundStatus::from(item.response.status);
        let response = if is_refund_failure(refund_status) {
            Err(domain_types::router_data::ErrorResponse {
                code: consts::NO_ERROR_CODE.to_string(),
                message: item
                    .response
                    .failure_reason
                    .clone()
                    .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
                reason: item.response.failure_reason,
                status_code: item.http_code,
                attempt_status: None,
                connector_transaction_id: Some(item.response.id),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            })
        } else {
            Ok(RefundsResponseData {
                connector_refund_id: item.response.id,
                refund_status,
                status_code: item.http_code,
            })
        };

        let currency_enum =
            common_enums::Currency::from_str(item.response.currency.to_uppercase().as_str())
                .change_context(
                    crate::utils::response_deserialization_fail(
                        item.http_code,
                    "stripe: response body did not match the expected format; confirm API version and connector documentation."),
                )?;

        let refund_amount_in_minor_unit =
            StripeAmountConvertor::convert_back(item.response.amount, currency_enum)
                .change_context(crate::utils::response_handling_fail_for_connector(
                    item.http_code,
                    "stripe",
                ))?;

        let response_integrity_object = RefundIntegrityObject {
            currency: currency_enum,
            refund_amount: refund_amount_in_minor_unit,
        };

        Ok(Self {
            response,
            request: RefundsData {
                integrity_object: Some(response_integrity_object),
                ..item.router_data.request
            },
            ..item.router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<RefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<RefundResponse, Self>) -> Result<Self, Self::Error> {
        let refund_status = common_enums::RefundStatus::from(item.response.status);
        let response = if is_refund_failure(refund_status) {
            Err(domain_types::router_data::ErrorResponse {
                code: consts::NO_ERROR_CODE.to_string(),
                message: item
                    .response
                    .failure_reason
                    .clone()
                    .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
                reason: item.response.failure_reason,
                status_code: item.http_code,
                attempt_status: None,
                connector_transaction_id: Some(item.response.id),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            })
        } else {
            Ok(RefundsResponseData {
                connector_refund_id: item.response.id,
                refund_status,
                status_code: item.http_code,
            })
        };

        Ok(Self {
            response,
            ..item.router_data
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        StripeRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for SetupMandateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: StripeRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        //Only cards supported for mandates
        let pm_type = StripePaymentMethodType::Card;
        let payment_data = StripePaymentMethodData::try_from((
            &item,
            item.router_data.resource_common_data.auth_type,
            pm_type,
        ))?;

        let meta_data = Some(get_transaction_metadata(
            item.router_data.request.metadata.clone(),
            item.router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        ));

        let browser_info = item
            .router_data
            .request
            .browser_info
            .clone()
            .map(StripeBrowserInformation::from);

        Ok(Self {
            confirm: true,
            payment_data,
            return_url: item.router_data.request.router_return_url.clone(),
            off_session: item.router_data.request.off_session,
            usage: item.router_data.request.setup_future_usage,
            payment_method_options: None,
            customer: item
                .router_data
                .resource_common_data
                .connector_customer
                .to_owned()
                .map(Secret::new),
            meta_data,
            payment_method_types: Some(pm_type),
            expand: Some(ExpandableObjects::LatestAttempt),
            browser_info,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        &StripeRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
        common_enums::AuthenticationType,
        StripePaymentMethodType,
    )> for StripePaymentMethodData<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        (item, auth_type, pm_type): (
            &StripeRouterData<
                RouterDataV2<
                    SetupMandate,
                    PaymentFlowData,
                    SetupMandateRequestData<T>,
                    PaymentsResponseData,
                >,
                T,
            >,
            common_enums::AuthenticationType,
            StripePaymentMethodType,
        ),
    ) -> Result<Self, Self::Error> {
        let pm_data = &item.router_data.request.payment_method_data;
        match pm_data {
            PaymentMethodData::Card(ref ccard) => {
                let payment_method_auth_type = match auth_type {
                    common_enums::AuthenticationType::ThreeDs => Auth3ds::Any,
                    common_enums::AuthenticationType::NoThreeDs => Auth3ds::Automatic,
                };
                Ok(Self::try_from((
                    ccard,
                    payment_method_auth_type,
                    item.router_data.request.request_incremental_authorization,
                    None,
                    None,
                ))?)
            }
            PaymentMethodData::PayLater(_) => Ok(Self::PayLater(StripePayLaterData {
                payment_method_data_type: pm_type,
            })),
            PaymentMethodData::BankRedirect(ref bank_redirect_data) => {
                Ok(Self::try_from(bank_redirect_data)?)
            }
            PaymentMethodData::Wallet(ref wallet_data) => Ok(Self::try_from(wallet_data)?),
            PaymentMethodData::BankDebit(bank_debit_data) => {
                let (_pm_type, bank_data) = get_bank_debit_data(bank_debit_data)?;

                Ok(Self::BankDebit(StripeBankDebitData {
                    bank_specific_data: bank_data,
                }))
            }
            PaymentMethodData::BankTransfer(bank_transfer_data) => match bank_transfer_data.deref()
            {
                payment_method_data::BankTransferData::AchBankTransfer {} => {
                    Ok(Self::BankTransfer(StripeBankTransferData::AchBankTransfer(
                        Box::new(AchTransferData {
                            payment_method_data_type: StripePaymentMethodType::CustomerBalance,
                            bank_transfer_type: StripeCreditTransferTypes::AchCreditTransfer,
                            payment_method_type: StripePaymentMethodType::CustomerBalance,
                            balance_funding_type: BankTransferType::BankTransfers,
                        }),
                    )))
                }
                payment_method_data::BankTransferData::MultibancoBankTransfer {} => Ok(
                    Self::BankTransfer(StripeBankTransferData::MultibancoBankTransfers(Box::new(
                        MultibancoTransferData {
                            payment_method_data_type: StripeCreditTransferTypes::Multibanco,
                            payment_method_type: StripeCreditTransferTypes::Multibanco,
                            email: item.router_data.resource_common_data.get_billing_email()?,
                        },
                    ))),
                ),
                payment_method_data::BankTransferData::SepaBankTransfer {} => {
                    Ok(Self::BankTransfer(
                        StripeBankTransferData::SepaBankTransfer(Box::new(SepaBankTransferData {
                            payment_method_data_type: StripePaymentMethodType::CustomerBalance,
                            bank_transfer_type: BankTransferType::EuBankTransfer,
                            balance_funding_type: BankTransferType::BankTransfers,
                            payment_method_type: StripePaymentMethodType::CustomerBalance,
                            country: item
                                .router_data
                                .resource_common_data
                                .get_billing_country()?,
                        })),
                    ))
                }
                payment_method_data::BankTransferData::BacsBankTransfer { .. } => {
                    Ok(Self::BankTransfer(
                        StripeBankTransferData::BacsBankTransfers(Box::new(BacsBankTransferData {
                            payment_method_data_type: StripePaymentMethodType::CustomerBalance,
                            bank_transfer_type: BankTransferType::GbBankTransfer,
                            balance_funding_type: BankTransferType::BankTransfers,
                            payment_method_type: StripePaymentMethodType::CustomerBalance,
                        })),
                    ))
                }
                payment_method_data::BankTransferData::Pix { .. }
                | payment_method_data::BankTransferData::Pse {}
                | payment_method_data::BankTransferData::PermataBankTransfer { .. }
                | payment_method_data::BankTransferData::BcaBankTransfer { .. }
                | payment_method_data::BankTransferData::BniVaBankTransfer { .. }
                | payment_method_data::BankTransferData::BriVaBankTransfer { .. }
                | payment_method_data::BankTransferData::CimbVaBankTransfer { .. }
                | payment_method_data::BankTransferData::DanamonVaBankTransfer { .. }
                | payment_method_data::BankTransferData::LocalBankTransfer { .. }
                | payment_method_data::BankTransferData::InstantBankTransfer {}
                | payment_method_data::BankTransferData::InstantBankTransferFinland {}
                | payment_method_data::BankTransferData::InstantBankTransferPoland {}
                | payment_method_data::BankTransferData::IndonesianBankTransfer { .. }
                | payment_method_data::BankTransferData::MandiriVaBankTransfer { .. } => {
                    Err(IntegrationError::NotImplemented(
                        (get_unimplemented_payment_method_error_message("stripe")).into(),
                        Default::default(),
                    )
                    .into())
                }
            },
            PaymentMethodData::MandatePayment
            | PaymentMethodData::Crypto(_)
            | PaymentMethodData::Reward
            | PaymentMethodData::RealTimePayment(_)
            | PaymentMethodData::MobilePayment(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::PaymentMethodToken(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
                Err(IntegrationError::NotImplemented(
                    (get_unimplemented_payment_method_error_message("stripe")).into(),
                    Default::default(),
                ))?
            }
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        StripeRouterData<
            RouterDataV2<
                CreateConnectorCustomer,
                PaymentFlowData,
                ConnectorCustomerData,
                ConnectorCustomerResponse,
            >,
            T,
        >,
    > for CreateConnectorCustomerRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: StripeRouterData<
            RouterDataV2<
                CreateConnectorCustomer,
                PaymentFlowData,
                ConnectorCustomerData,
                ConnectorCustomerResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            description: item.router_data.request.description.to_owned(),
            email: item
                .router_data
                .request
                .email
                .map(|email| email.peek().to_owned()),
            phone: item.router_data.request.phone.to_owned(),
            name: item.router_data.request.name.to_owned(),
            source: item
                .router_data
                .request
                .preprocessing_id
                .to_owned()
                .map(Secret::new),
        })
    }
}

impl<F, T> TryFrom<ResponseRouterData<CreateConnectorCustomerResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, ConnectorCustomerResponse>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<CreateConnectorCustomerResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(ConnectorCustomerResponse {
                connector_customer_id: item.response.id,
            }),
            ..item.router_data
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        StripeRouterData<
            RouterDataV2<
                IncrementalAuthorization,
                PaymentFlowData,
                PaymentsIncrementalAuthorizationData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PaymentIncrementalAuthRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: StripeRouterData<
            RouterDataV2<
                IncrementalAuthorization,
                PaymentFlowData,
                PaymentsIncrementalAuthorizationData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount = StripeAmountConvertor::convert(
            item.router_data.request.minor_amount,
            item.router_data.request.currency,
        )?;

        Ok(Self { amount })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize + Serialize>
    TryFrom<
        &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
    > for StripeSplitPaymentRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<
            RepeatPayment,
            PaymentFlowData,
            RepeatPaymentData<T>,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        //extracting mandate metadata from CIT call if CIT call was a Split Payment
        let from_metadata = match &item
            .request
            .mandate_reference {
                MandateReferenceId::ConnectorMandateId(mandate_data) => {
                    mandate_data.get_mandate_metadata()
                }
                _ => None
}
            .and_then(|secret_value| {
                let json_value = secret_value.clone().expose();
                match serde_json::from_value::<Self>(json_value.clone()) {
                    Ok(val) => Some(val),
                    Err(err) => {
                        tracing::info!(
                            "STRIPE: Picking merchant_account_id and merchant_config_currency from payments request: {:?}", err
                        );
                        None
                    }
                }
            });

        // If the Split Payment Request in MIT mismatches with the metadata from CIT, throw an error
        if from_metadata.is_some() && item.request.split_payments.is_some() {
            let mut mit_charge_type = None;
            let mut mit_application_fees = None;
            let mut mit_transfer_account_id = None;
            if let Some(domain_types::connector_types::SplitPaymentsRequest::StripeSplitPayment(
                stripe_split_payment,
            )) = item.request.split_payments.as_ref()
            {
                mit_charge_type = Some(stripe_split_payment.charge_type.clone());
                mit_application_fees = stripe_split_payment.application_fees;
                mit_transfer_account_id = Some(stripe_split_payment.transfer_account_id.clone());
            }

            if mit_charge_type != from_metadata.as_ref().and_then(|m| m.charge_type.clone())
                || mit_application_fees != from_metadata.as_ref().and_then(|m| m.application_fees)
                || mit_transfer_account_id
                    != from_metadata
                        .as_ref()
                        .and_then(|m| m.transfer_account_id.clone().map(|s| s.expose()))
            {
                let mismatched_fields = ["transfer_account_id", "application_fees", "charge_type"];

                let field_str = mismatched_fields.join(", ");
                Err(IntegrationError::MandatePaymentDataMismatch {
                    fields: field_str,
                    context: Default::default(),
                })?
            }
        }

        // If Mandate Metadata from CIT call has something, populate it
        let (charge_type, mut transfer_account_id, application_fees) =
            if let Some(ref metadata) = from_metadata {
                (
                    metadata.charge_type.clone(),
                    metadata.transfer_account_id.clone(),
                    metadata.application_fees,
                )
            } else {
                (None, None, None)
            };

        // If Charge Type is Destination, transfer_account_id need not be appended in headers
        if charge_type
            == Some(common_enums::PaymentChargeType::Stripe(
                common_enums::StripeChargeType::Destination,
            ))
        {
            transfer_account_id = None;
        }
        Ok(Self {
            charge_type,
            transfer_account_id,
            application_fees,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize + Serialize>
    TryFrom<ResponseRouterData<PaymentsAuthorizeResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PaymentsAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Self::try_from(ResponseRouterData {
            response: item.response.0,
            router_data: item.router_data,
            http_code: item.http_code,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        StripeRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PaymentIntentRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        value: StripeRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = value.router_data;

        let mandate_metadata = match &item.request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(mandate_data) => {
                Some(mandate_data.get_mandate_metadata())
            }
            _ => None,
        };

        let (transfer_account_id, charge_type, application_fees) =
            match mandate_metadata.as_ref().and_then(|s| s.as_ref()) {
                Some(secret_value) => {
                    let json_value = secret_value.clone().expose();

                    let parsed: Result<StripeSplitPaymentRequest, _> =
                        serde_json::from_value(json_value);

                    match parsed {
                        Ok(data) => (
                            data.transfer_account_id,
                            data.charge_type,
                            data.application_fees,
                        ),
                        Err(_) => (None, None, None),
                    }
                }
                None => (None, None, None),
            };

        let payment_method_token = match &item.request.payment_method_data {
            PaymentMethodData::PaymentMethodToken(t) => Some(t.token.clone()),
            _ => None,
        };

        let amount =
            StripeAmountConvertor::convert(item.request.minor_amount, item.request.currency)?;
        let order_id = item
            .resource_common_data
            .connector_request_reference_id
            .clone();

        let shipping_address = match payment_method_token {
            Some(_) => None,
            None => Some(StripeShippingAddress {
                city: item.resource_common_data.get_optional_shipping_city(),
                country: item.resource_common_data.get_optional_shipping_country(),
                line1: item.resource_common_data.get_optional_shipping_line1(),
                line2: item.resource_common_data.get_optional_shipping_line2(),
                zip: item.resource_common_data.get_optional_shipping_zip(),
                state: item.resource_common_data.get_optional_shipping_state(),
                name: item.resource_common_data.get_optional_shipping_full_name(),
                phone: item
                    .resource_common_data
                    .get_optional_shipping_phone_number(),
            }),
        };

        let billing_address = match payment_method_token {
            Some(_) => None,
            None => Some(StripeBillingAddress {
                city: item.resource_common_data.get_optional_billing_city(),
                country: item.resource_common_data.get_optional_billing_country(),
                address_line1: item.resource_common_data.get_optional_billing_line1(),
                address_line2: item.resource_common_data.get_optional_billing_line2(),
                zip_code: item.resource_common_data.get_optional_billing_zip(),
                state: item.resource_common_data.get_optional_billing_state(),
                name: item.resource_common_data.get_optional_billing_full_name(),
                email: item.resource_common_data.get_optional_billing_email(),
                phone: item
                    .resource_common_data
                    .get_optional_billing_phone_number(),
            }),
        };

        let mut payment_method_options = None;

        let (
            payment_data,
            payment_method,
            billing_address,
            payment_method_types,
            setup_future_usage,
        ) = if payment_method_token.is_some() {
            (None, None, StripeBillingAddress::default(), None, None)
        } else {
            match &item.request.mandate_reference {
                MandateReferenceId::ConnectorMandateId(connector_mandate_ids) => (
                    None,
                    connector_mandate_ids.get_connector_mandate_id(),
                    StripeBillingAddress::default(),
                    get_payment_method_type_for_saved_payment_method_payment(&item)?,
                    None,
                ),
                MandateReferenceId::NetworkMandateId(network_transaction_id) => {
                    payment_method_options = Some(StripePaymentMethodOptions::Card {
                        mandate_options: None,
                        network_transaction_id: None,
                        mit_exemption: Some(MitExemption {
                            network_transaction_id: Secret::new(network_transaction_id.clone()),
                        }),
                    });

                    let payment_data = match item.request.payment_method_data {
                        PaymentMethodData::CardDetailsForNetworkTransactionId(
                            ref card_details_for_network_transaction_id,
                        ) => StripePaymentMethodData::CardNetworkTransactionId(
                            StripeCardNetworkTransactionIdData {
                                payment_method_data_type: StripePaymentMethodType::Card,
                                payment_method_data_card_number:
                                    card_details_for_network_transaction_id.card_number.clone(),
                                payment_method_data_card_exp_month:
                                    card_details_for_network_transaction_id
                                        .card_exp_month
                                        .clone(),
                                payment_method_data_card_exp_year:
                                    card_details_for_network_transaction_id
                                        .card_exp_year
                                        .clone(),
                                payment_method_data_card_cvc: None,
                                payment_method_auth_type: None,
                                payment_method_data_card_preferred_network:
                                    card_details_for_network_transaction_id
                                        .card_network
                                        .clone()
                                        .and_then(get_stripe_card_network),
                                request_overcapture: None,
                            },
                        ),
                        PaymentMethodData::CardRedirect(_)
                        | PaymentMethodData::Wallet(_)
                        | PaymentMethodData::PayLater(_)
                        | PaymentMethodData::BankRedirect(_)
                        | PaymentMethodData::BankDebit(_)
                        | PaymentMethodData::BankTransfer(_)
                        | PaymentMethodData::Crypto(_)
                        | PaymentMethodData::MandatePayment
                        | PaymentMethodData::Reward
                        | PaymentMethodData::RealTimePayment(_)
                        | PaymentMethodData::MobilePayment(_)
                        | PaymentMethodData::Upi(_)
                        | PaymentMethodData::Voucher(_)
                        | PaymentMethodData::GiftCard(_)
                        | PaymentMethodData::OpenBanking(_)
                        | PaymentMethodData::PaymentMethodToken(_)
                        | PaymentMethodData::NetworkToken(_)
                        | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(
                            _,
                        )
                        | PaymentMethodData::Card(_) => Err(IntegrationError::NotImplemented(
                            ("Network tokenization for payment method".to_string()).into(),
                            Default::default(),
                        ))?,
                    };

                    (
                        Some(payment_data),
                        None,
                        StripeBillingAddress::default(),
                        None,
                        None,
                    )
                }
                MandateReferenceId::NetworkTokenWithNTI(_) => {
                    let (payment_method_data, payment_method_type, billing_address) =
                        create_stripe_payment_method(
                            &item.request.payment_method_data,
                            PaymentRequestDetails {
                                auth_type: item.resource_common_data.auth_type,
                                is_customer_initiated_mandate_payment: Some(false),
                                billing_address: billing_address.ok_or(
                                    IntegrationError::MissingRequiredField {
                                        field_name: "billing_address",
                                        context: Default::default(),
                                    },
                                )?,
                                request_incremental_authorization: false,
                                request_extended_authorization: None,
                                request_overcapture: None,
                            },
                        )?;

                    validate_shipping_address_against_payment_method(
                        &shipping_address,
                        payment_method_type.as_ref(),
                    )?;

                    (
                        Some(payment_method_data),
                        None,
                        billing_address,
                        payment_method_type,
                        None,
                    )
                }
            }
        };

        let meta_data = get_transaction_metadata(item.request.metadata.clone(), order_id);

        // We pass browser_info only when payment_data exists.
        // Hence, we're pass Null during recurring payments as payment_method_data[type] is not passed
        let browser_info = if payment_data.is_some() && payment_method_token.is_none() {
            item.request
                .browser_info
                .clone()
                .map(StripeBrowserInformation::from)
        } else {
            None
        };

        let charges = match &item.request.split_payments {
            Some(domain_types::connector_types::SplitPaymentsRequest::StripeSplitPayment(
                stripe_split_payment,
            )) => match &stripe_split_payment.charge_type {
                common_enums::PaymentChargeType::Stripe(charge_type) => match charge_type {
                    common_enums::StripeChargeType::Direct => Some(IntentCharges {
                        application_fee_amount: stripe_split_payment.application_fees,
                        destination_account_id: None,
                    }),
                    common_enums::StripeChargeType::Destination => Some(IntentCharges {
                        application_fee_amount: stripe_split_payment.application_fees,
                        destination_account_id: Some(Secret::new(
                            stripe_split_payment.transfer_account_id.clone(),
                        )),
                    }),
                },
            },
            None => None,
        };

        let charges_in = if charges.is_none() {
            match charge_type {
                Some(common_enums::PaymentChargeType::Stripe(
                    common_enums::StripeChargeType::Direct,
                )) => Some(IntentCharges {
                    application_fee_amount: application_fees, // default to 0 if None
                    destination_account_id: None,
                }),
                Some(common_enums::PaymentChargeType::Stripe(
                    common_enums::StripeChargeType::Destination,
                )) => Some(IntentCharges {
                    application_fee_amount: application_fees,
                    destination_account_id: transfer_account_id,
                }),
                _ => None,
            }
        } else {
            charges
        };

        let pm = match (payment_method, payment_method_token.clone()) {
            (Some(method), _) => Some(Secret::new(method)),
            (None, Some(token)) => Some(token),
            (None, None) => None,
        };

        Ok(Self {
            amount,                                      //hopefully we don't loose some cents here
            currency: item.request.currency.to_string(), //we need to copy the value and not transfer ownership
            statement_descriptor_suffix: None,
            statement_descriptor: None,
            meta_data,
            return_url: item
                .request
                .router_return_url
                .clone()
                .unwrap_or_else(|| "https://juspay.in/".to_string()),
            confirm: true, // Stripe requires confirm to be true if return URL is present
            description: item.resource_common_data.description.clone(),
            shipping: shipping_address,
            billing: billing_address,
            capture_method: StripeCaptureMethod::from(item.request.capture_method),
            payment_data,
            payment_method_options,
            payment_method: pm,
            customer: item
                .resource_common_data
                .connector_customer
                .clone()
                .map(Secret::new),
            setup_mandate_details: None,
            off_session: item.request.off_session,
            setup_future_usage,
            payment_method_types,
            expand: Some(ExpandableObjects::LatestCharge),
            browser_info,
            charges: charges_in,
        })
    }
}

fn get_payment_method_type_for_saved_payment_method_payment<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize + Serialize,
>(
    item: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
) -> Result<Option<StripePaymentMethodType>, error_stack::Report<IntegrationError>> {
    if item.resource_common_data.payment_method == common_enums::PaymentMethod::Card {
        Ok(Some(StripePaymentMethodType::Card)) //stripe takes ["Card"] as default
    } else {
        let stripe_payment_method_type = match item
            .resource_common_data
            .recurring_mandate_payment_data
            .clone()
        {
            Some(recurring_payment_method_data) => {
                match recurring_payment_method_data.payment_method_type {
                    Some(payment_method_type) => {
                        StripePaymentMethodType::try_from(payment_method_type)
                    }
                    None => Err(IntegrationError::MissingRequiredField {
                        field_name: "payment_method_type",
                        context: Default::default(),
                    }
                    .into()),
                }
            }
            None => Err(IntegrationError::MissingRequiredField {
                field_name: "recurring_mandate_payment_data",
                context: Default::default(),
            }
            .into()),
        }?;
        match stripe_payment_method_type {
            //Stripe converts Ideal, Bancontact & Sofort Bank redirect methods to Sepa direct debit and attaches to the customer for future usage
            StripePaymentMethodType::Ideal
            | StripePaymentMethodType::Bancontact
            | StripePaymentMethodType::Sofort => Ok(Some(StripePaymentMethodType::Sepa)),
            _ => Ok(Some(stripe_payment_method_type)),
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        StripeRouterData<
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
            T,
        >,
    > for TokenRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: StripeRouterData<
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let billing_address = StripeBillingAddressCardToken {
            name: item
                .router_data
                .resource_common_data
                .get_optional_billing_full_name(),
            email: item
                .router_data
                .resource_common_data
                .get_optional_billing_email(),
            phone: item
                .router_data
                .resource_common_data
                .get_optional_billing_phone_number(),
            address_line1: item
                .router_data
                .resource_common_data
                .get_optional_billing_line1(),
            address_line2: item
                .router_data
                .resource_common_data
                .get_optional_billing_line2(),
            city: item
                .router_data
                .resource_common_data
                .get_optional_billing_city(),
            state: item
                .router_data
                .resource_common_data
                .get_optional_billing_state(),
        };

        // Card flow for tokenization is handled separately because of API contact difference.
        // /v1/tokens only accepts card[*] fields — do NOT include `type` or `billing_details[*]`.
        let request_payment_data = match &item.router_data.request.payment_method_data {
            PaymentMethodData::Card(card_details) => {
                StripePaymentMethodData::CardToken(StripeCardToken {
                    payment_method_type: Some(StripePaymentMethodType::Card),
                    token_card_number: card_details.card_number.clone(),
                    token_card_exp_month: card_details.card_exp_month.clone(),
                    token_card_exp_year: card_details.card_exp_year.clone(),
                    token_card_cvc: card_details.card_cvc.clone(),
                    billing: billing_address,
                })
            }
            _ => {
                create_stripe_payment_method(
                    &item.router_data.request.payment_method_data,
                    PaymentRequestDetails {
                        auth_type: item.router_data.resource_common_data.auth_type,
                        is_customer_initiated_mandate_payment: None,
                        billing_address: StripeBillingAddress::default(),
                        request_incremental_authorization: false,
                        request_extended_authorization: None,
                        request_overcapture: None,
                    },
                )?
                .0
            }
        };

        Ok(Self {
            token_data: request_payment_data,
        })
    }
}

impl<F, T> TryFrom<ResponseRouterData<StripeTokenResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentMethodTokenResponse>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<StripeTokenResponse, Self>) -> Result<Self, Self::Error> {
        let token = item.response.id.clone().expose();
        Ok(Self {
            response: Ok(PaymentMethodTokenResponse { token }),
            ..item.router_data
        })
    }
}

// ---- ClientAuthenticationToken flow types ----

/// Creates an unconfirmed PaymentIntent. `confirm` is intentionally omitted —
/// confirmation happens browser-side via `stripe.confirmPayment()` using the
/// returned `client_secret`.
#[serde_with::skip_serializing_none]
#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct StripeClientAuthRequest {
    pub amount: MinorUnit,
    pub currency: String,
    #[serde(rename = "automatic_payment_methods[enabled]")]
    pub automatic_payment_methods_enabled: Option<bool>,
    #[serde(flatten)]
    pub meta_data: HashMap<String, String>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        StripeRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                PaymentFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for StripeClientAuthRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: StripeRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                PaymentFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;

        let amount = StripeAmountConvertor::convert(
            router_data.request.amount,
            router_data.request.currency,
        )?;

        let currency = router_data.request.currency.to_string().to_lowercase();

        let order_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        let meta_data = get_transaction_metadata(None, order_id);

        Ok(Self {
            amount,
            currency,
            automatic_payment_methods_enabled: Some(true),
            meta_data,
        })
    }
}

/// Wraps PaymentIntentResponse for the ClientAuthenticationToken flow.
#[derive(Debug, Deserialize, Serialize)]
pub struct StripeClientAuthResponse(PaymentIntentResponse);

impl TryFrom<ResponseRouterData<StripeClientAuthResponse, Self>>
    for RouterDataV2<
        ClientAuthenticationToken,
        PaymentFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<StripeClientAuthResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response.0;

        let client_secret =
            response
                .client_secret
                .ok_or(ConnectorError::ResponseDeserializationFailed {
                    context: Default::default(),
                })?;

        let session_data = ClientAuthenticationTokenData::ConnectorSpecific(Box::new(
            ConnectorSpecificClientAuthenticationResponse::Stripe(
                StripeClientAuthenticationResponseDomain { client_secret },
            ),
        ));

        Ok(Self {
            response: Ok(PaymentsResponseData::ClientAuthenticationTokenResponse {
                session_data,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}
