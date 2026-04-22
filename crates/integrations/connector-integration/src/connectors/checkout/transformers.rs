use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::{CustomResult, ParsingError},
    request::Method,
    types::MinorUnit,
};
use domain_types::{
    connector_flow::{Authorize, Capture, RepeatPayment, SetupMandate, Void},
    connector_types::{
        MandateReference, MandateReferenceId, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        ResponseId, SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError},
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
use hyperswitch_masking::{ExposeInterface, ExposeOptionInterface, Secret};
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

#[derive(Debug, Serialize)]
pub struct AchBankDebitSource {
    #[serde(rename = "type")]
    pub source_type: String,
    #[serde(rename = "account_type")]
    pub account_type: common_enums::BankType,
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
    cryptogram: Option<Secret<String>>,
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

pub struct CheckoutAuthType {
    pub api_key: Secret<String>,
    pub processing_channel_id: Secret<String>,
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
    pub order_id: Option<String>,
    pub tax_amount: Option<MinorUnit>,
    pub discount_amount: Option<MinorUnit>,
    pub duty_amount: Option<MinorUnit>,
    pub shipping_amount: Option<MinorUnit>,
    pub shipping_tax_amount: Option<MinorUnit>,
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
                context: Default::default(),
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

fn build_metadata<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: &CheckoutRouterData<
        RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        T,
    >,
) -> Option<Secret<serde_json::Value>> {
    // get metadata or create empty json object
    let metadata_json = item
        .router_data
        .request
        .metadata
        .clone()
        .expose_option()
        .unwrap_or_else(|| json!({}));

    Some(Secret::new(metadata_json))
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

        let (source_var, previous_payment_id, merchant_initiated, store_for_future_use) = match item
            .router_data
            .request
            .payment_method_data
            .clone()
        {
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
            PaymentMethodData::Wallet(wallet_data) => match wallet_data {
                WalletData::GooglePay(google_pay_data) => {
                    match &google_pay_data.tokenization_data {
                        domain_types::payment_method_data::GpayTokenizationData::Decrypted(
                            google_pay_decrypted_data,
                        ) => {
                            let token = google_pay_decrypted_data
                                .application_primary_account_number
                                .clone();

                            let expiry_month = google_pay_decrypted_data
                                .get_expiry_month()
                                .change_context(IntegrationError::InvalidDataFormat {
                                    field_name: "google_pay_decrypted_data.card_exp_month",
                                    context: Default::default(),
                                })?;

                            let expiry_year = google_pay_decrypted_data
                                .get_four_digit_expiry_year()
                                .change_context(IntegrationError::InvalidDataFormat {
                                    field_name: "google_pay_decrypted_data.card_exp_year",
                                    context: Default::default(),
                                })?;

                            let cryptogram = google_pay_decrypted_data.cryptogram.clone();

                            let p_source =
                                PaymentSource::GooglePayPredecrypt(Box::new(GooglePayPredecrypt {
                                    _type: "network_token".to_string(),
                                    token,
                                    token_type: "googlepay".to_string(),
                                    expiry_month,
                                    expiry_year,
                                    eci: "06".to_string(),
                                    cryptogram,
                                    billing_address: billing_details,
                                }));

                            Ok((p_source, None, Some(false), store_for_future_use))
                        }
                        domain_types::payment_method_data::GpayTokenizationData::Encrypted(_) => {
                            Err(IntegrationError::MissingRequiredField {
                                field_name: "google_pay_decrypted_data",
                                context: Default::default(),
                            })
                        }
                    }
                }
                WalletData::ApplePay(apple_pay_data) => {
                    match apple_pay_data
                        .payment_data
                        .get_decrypted_apple_pay_payment_data_optional()
                    {
                        Some(apple_pay_decrypt_data) => {
                            let exp_month = apple_pay_decrypt_data.get_expiry_month();
                            let expiry_year_4_digit =
                                apple_pay_decrypt_data.get_four_digit_expiry_year();
                            let p_source =
                                PaymentSource::ApplePayPredecrypt(Box::new(ApplePayPredecrypt {
                                    token: apple_pay_decrypt_data
                                        .application_primary_account_number
                                        .clone(),
                                    decrypt_type: "network_token".to_string(),
                                    token_type: "applepay".to_string(),
                                    expiry_month: exp_month,
                                    expiry_year: expiry_year_4_digit,
                                    eci: apple_pay_decrypt_data.payment_data.eci_indicator.clone(),
                                    cryptogram: apple_pay_decrypt_data
                                        .payment_data
                                        .online_payment_cryptogram
                                        .clone(),
                                    billing_address: billing_details,
                                }));
                            Ok((p_source, None, Some(false), store_for_future_use))
                        }
                        None => Err(IntegrationError::NotImplemented(
                            utils::get_unimplemented_payment_method_error_message("checkout"),
                            Default::default(),
                        )),
                    }
                }
                _ => Err(IntegrationError::NotImplemented(
                    utils::get_unimplemented_payment_method_error_message("checkout"),
                    Default::default(),
                )),
            },
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

                // Use bank_type from input or default to Savings
                let account_type = bank_type.unwrap_or(common_enums::BankType::Savings);

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
                Default::default(),
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
        let metadata = build_metadata(&item);

        let (customer, processing, shipping, items) = if let Some(l2l3_data) =
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
                            Some(network_transaction_id.clone()),
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
                                context: Default::default(),
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
                            Some(network_transaction_id.clone()),
                            Some(true),
                            p_type,
                            None,
                        ))
                    }
                    _ => Err(IntegrationError::NotImplemented(
                        utils::get_unimplemented_payment_method_error_message("checkout"),
                        Default::default(),
                    )),
                }
            }
            _ => Err(IntegrationError::NotImplemented(
                utils::get_unimplemented_payment_method_error_message("checkout"),
                Default::default(),
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

        let metadata = item.router_data.request.metadata.clone();

        let (customer, processing, shipping, items) = if let Some(l2l3_data) =
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

                // Use bank_type from input or default to Savings
                let account_type = bank_type.unwrap_or(common_enums::BankType::Savings);

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
            _ => Err(IntegrationError::NotImplemented(
                utils::get_unimplemented_payment_method_error_message("checkout"),
                Default::default(),
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

        let (customer, processing, shipping, items) = if let Some(l2l3_data) =
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
            metadata: None,
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
            connector_response_reference_id: Some(
                item.response.reference.unwrap_or(item.response.id),
            ),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
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
                    connector_response_reference_id: Some(
                        item.response.reference.unwrap_or(item.response.id),
                    ),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
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
            });

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
            redirection_data: redirection_data.map(Box::new),
            mandate_reference: mandate_reference.map(Box::new),
            connector_metadata: Some(connector_meta),
            network_txn_id: item.response.scheme_id.clone(),
            connector_response_reference_id: Some(
                item.response.reference.unwrap_or(item.response.id),
            ),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
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
            connector_response_reference_id: Some(
                item.response.reference.unwrap_or(item.response.id),
            ),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
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
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
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
                connector_response_reference_id: item.response.reference,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
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
