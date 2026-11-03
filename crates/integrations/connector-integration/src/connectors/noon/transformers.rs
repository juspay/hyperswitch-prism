use common_enums::enums::{self, AttemptStatus, CountryAlpha2};
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    ext_traits::Encode,
    pii,
    request::Method,
    types::{FloatMajorUnit, FloatMajorUnitForConnector, MinorUnit, Money, StringMajorUnit},
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, MandateRevoke, Refund, RepeatPayment, SetupMandate, Void,
    },
    connector_types::{
        ConnectorEnum, MandateReference, MandateReferenceId, MandateRevokeRequestData,
        MandateRevokeResponseData, PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsResponseData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, RepeatPaymentData, ResponseId, SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    mandates::MandateDataType,
    payment_address::AddressDetails,
    payment_method_data::{
        ApplePayPaymentData, GooglePayPaymentMethodInfo, GpayEncryptedTokenizationData,
        GpayTokenizationData, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber, WalletData,
    },
    router_data::{
        AdditionalPaymentMethodConnectorResponse, ConnectorResponseData, ConnectorSpecificConfig,
        ErrorResponse, FlowStatus,
    },
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
    types::PaymentMethodDataType,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::{types::ResponseRouterData, utils};

use super::NoonRouterData;

// These needs to be accepted from SDK, need to be done after 1.0.0 stability as API contract will change
const GOOGLEPAY_API_VERSION_MINOR: u8 = 0;
const GOOGLEPAY_API_VERSION: u8 = 2;

#[derive(Debug, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum NoonChannels {
    Web,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum NoonSubscriptionType {
    Unscheduled,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonSubscriptionData {
    #[serde(rename = "type")]
    subscription_type: NoonSubscriptionType,
    //Short description about the subscription.
    name: String,
    /// Noon rejects a subscription without it (5105 "subscription.maxAmount is missing");
    /// Authorize always sets it from the mandate amount.
    #[serde(skip_serializing_if = "Option::is_none")]
    max_amount: Option<StringMajorUnit>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonBillingAddress {
    street: Option<Secret<String>>,
    street2: Option<Secret<String>>,
    city: Option<Secret<String>>,
    state_province: Option<Secret<String>>,
    country: Option<CountryAlpha2>,
    postal_code: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonBilling {
    address: NoonBillingAddress,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonOrder {
    amount: StringMajorUnit,
    currency: Option<enums::Currency>,
    channel: NoonChannels,
    category: Option<String>,
    reference: String,
    //Short description of the order.
    name: String,
    nvp: Option<NoonOrderNvp>,
    ip_address: Option<Secret<String, pii::IpAddress>>,
}

#[derive(Debug, Serialize)]
pub struct NoonOrderNvp {
    #[serde(flatten)]
    inner: std::collections::BTreeMap<String, Secret<String>>,
}

fn get_value_as_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(string) => string.to_owned(),
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::Array(_)
        | serde_json::Value::Object(_) => value.to_string(),
    }
}

impl NoonOrderNvp {
    pub fn new(metadata: &serde_json::Value) -> Self {
        let metadata_as_string = metadata.to_string();
        let hash_map: std::collections::BTreeMap<String, serde_json::Value> =
            serde_json::from_str(&metadata_as_string).unwrap_or(std::collections::BTreeMap::new());
        let inner = hash_map
            .into_iter()
            .enumerate()
            .map(|(index, (hs_key, hs_value))| {
                let noon_key = format!("{}", index + 1);
                // to_string() function on serde_json::Value returns a string with "" quotes. Noon doesn't allow this. Hence get_value_as_string function
                let noon_value = format!("{hs_key}={}", get_value_as_string(&hs_value));
                (noon_key, Secret::new(noon_value))
            })
            .collect();
        Self { inner }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum NoonPaymentActions {
    Authorize,
    Sale,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonConfiguration {
    tokenize_c_c: Option<bool>,
    payment_action: NoonPaymentActions,
    return_url: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonSubscription {
    subscription_identifier: Secret<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonCard<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
{
    name_on_card: Option<Secret<String>>,
    number_plain: RawCardNumber<T>,
    expiry_month: Secret<String>,
    expiry_year: Secret<String>,
    cvv: Secret<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonApplePayPaymentMethod {
    pub display_name: String,
    pub network: String,
    #[serde(rename = "type")]
    pub pm_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonApplePayHeader {
    ephemeral_public_key: Secret<String>,
    public_key_hash: Secret<String>,
    transaction_id: Secret<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NoonApplePaymentData {
    version: Secret<String>,
    data: Secret<String>,
    signature: Secret<String>,
    header: NoonApplePayHeader,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonApplePayData {
    payment_data: NoonApplePaymentData,
    payment_method: NoonApplePayPaymentMethod,
    transaction_identifier: Secret<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonApplePayTokenData {
    token: NoonApplePayData,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonApplePay {
    payment_info: Secret<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonGooglePay {
    api_version_minor: u8,
    api_version: u8,
    payment_method_data: NoonGooglePayPaymentMethodData,
}

/// Google Pay `paymentMethodData` as Noon reads it: Google's camelCase keys
/// (`tokenizationData`), not the snake_case of the shared domain type.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonGooglePayPaymentMethodData {
    #[serde(rename = "type")]
    pm_type: String,
    description: String,
    info: GooglePayPaymentMethodInfo,
    tokenization_data: GpayEncryptedTokenizationData,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonPayPal {
    return_url: String,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", content = "data", rename_all = "UPPERCASE")]
pub enum NoonPaymentData<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    Card(NoonCard<T>),
    Subscription(NoonSubscription),
    ApplePay(NoonApplePay),
    GooglePay(NoonGooglePay),
    PayPal(NoonPayPal),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NoonApiOperations {
    Initiate,
    Capture,
    Reverse,
    Refund,
    CancelSubscription,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonPaymentsRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    api_operation: NoonApiOperations,
    order: NoonOrder,
    configuration: NoonConfiguration,
    payment_data: NoonPaymentData<T>,
    subscription: Option<NoonSubscriptionData>,
    billing: Option<NoonBilling>,
}

/// The billing address Noon should receive, or `None` when it carries no address
/// fields at all.
///
/// Direct (hyperswitch) reaches `None` naturally: an Apple Pay token's billing
/// contact holds only email/phone, so the unified address is
/// `Address { address: None, .. }` and `.and_then(|b| b.address.as_ref())`
/// short-circuits. Prism's grpc -> domain conversion always yields
/// `Some(AddressDetails)`, so that chain can never short-circuit and Noon was
/// sent an all-null `billing` object where Direct sends `null`.
///
/// Only an entirely-empty address is treated as absent; present-but-empty
/// strings are preserved.
fn get_optional_billing_address(flow_data: &PaymentFlowData) -> Option<&AddressDetails> {
    flow_data
        .get_optional_billing()
        .and_then(|billing_address| billing_address.address.as_ref())
        .filter(|address| **address != AddressDetails::default())
}

const NOON_DOCS_URL: &str = "https://docs.noonpayments.com/";

/// `IntegrationError::NotSupported` for a payment method Noon's order API does not accept.
fn noon_payment_method_not_supported(
    payment_method: impl std::fmt::Display,
) -> error_stack::Report<IntegrationError> {
    error_stack::report!(IntegrationError::NotSupported {
        message: utils::get_unimplemented_payment_method_error_message("Noon"),
        connector: "Noon",
        context: IntegrationErrorContext {
            suggested_action: Some(
                "Pay with a card, an encrypted Apple Pay or Google Pay token, or PayPal; \
                 Noon's order API accepts no other payment data"
                    .to_string(),
            ),
            doc_url: Some(NOON_DOCS_URL.to_string()),
            additional_context: Some(format!("{payment_method} is not supported by Noon")),
        },
    })
}

/// Noon `configuration.paymentAction` for a capture method.
///
/// Noon only knows AUTHORIZE and SALE, so multiple / scheduled capture is refused before
/// any request is built. A zero amount is never sent as a SALE.
pub(crate) fn get_noon_payment_action(
    capture_method: Option<enums::CaptureMethod>,
    is_zero_amount: bool,
) -> Result<NoonPaymentActions, error_stack::Report<IntegrationError>> {
    let payment_action = match capture_method {
        Some(enums::CaptureMethod::Automatic)
        | Some(enums::CaptureMethod::SequentialAutomatic)
        | None => NoonPaymentActions::Sale,
        Some(enums::CaptureMethod::Manual) => NoonPaymentActions::Authorize,
        Some(capture_method @ enums::CaptureMethod::ManualMultiple)
        | Some(capture_method @ enums::CaptureMethod::Scheduled) => {
            return Err(error_stack::report!(IntegrationError::NotSupported {
                message: format!("capture_method {capture_method}"),
                connector: "Noon",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Use capture_method AUTOMATIC (Noon SALE) or MANUAL (Noon AUTHORIZE \
                         followed by a capture)"
                            .to_string(),
                    ),
                    doc_url: Some(NOON_DOCS_URL.to_string()),
                    additional_context: Some(
                        "Noon's paymentAction supports only AUTHORIZE and SALE".to_string(),
                    ),
                },
            }));
        }
    };
    Ok(if is_zero_amount {
        NoonPaymentActions::Authorize
    } else {
        payment_action
    })
}

/// Noon `paymentData` for the payment methods its order API accepts: CARD, APPLEPAY and
/// GOOGLEPAY (encrypted tokens only; Noon decrypts them itself) and PAYPAL.
pub(crate) fn build_noon_payment_data<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    payment_method_data: &PaymentMethodData<T>,
    flow_data: &PaymentFlowData,
    router_return_url: Option<String>,
) -> Result<NoonPaymentData<T>, error_stack::Report<IntegrationError>> {
    match payment_method_data {
        PaymentMethodData::Card(card) => Ok(NoonPaymentData::Card(NoonCard {
            name_on_card: flow_data.get_optional_billing_full_name(),
            number_plain: card.card_number.clone(),
            expiry_month: card.card_exp_month.clone(),
            expiry_year: card.get_expiry_year_4_digit(),
            cvv: card.card_cvc.clone(),
        })),
        PaymentMethodData::Wallet(wallet_data) => match wallet_data {
            WalletData::GooglePay(google_pay_data) => match &google_pay_data.tokenization_data {
                GpayTokenizationData::Encrypted(encrypted_data) => {
                    Ok(NoonPaymentData::GooglePay(NoonGooglePay {
                        api_version_minor: GOOGLEPAY_API_VERSION_MINOR,
                        api_version: GOOGLEPAY_API_VERSION,
                        payment_method_data: NoonGooglePayPaymentMethodData {
                            pm_type: google_pay_data.pm_type.clone(),
                            description: google_pay_data.description.clone(),
                            info: google_pay_data.info.clone(),
                            tokenization_data: encrypted_data.clone(),
                        },
                    }))
                }
                GpayTokenizationData::Decrypted(_) => Err(noon_payment_method_not_supported(
                    "Google Pay with a pre-decrypted token",
                )),
            },
            WalletData::ApplePay(apple_pay_data) => match &apple_pay_data.payment_data {
                ApplePayPaymentData::Encrypted(_) => {
                    let payment_token_data = NoonApplePayTokenData {
                        token: NoonApplePayData {
                            payment_data: wallet_data
                                .get_wallet_token_as_json("Apple Pay".to_string())?,
                            payment_method: NoonApplePayPaymentMethod {
                                display_name: apple_pay_data.payment_method.display_name.clone(),
                                network: apple_pay_data.payment_method.network.clone(),
                                pm_type: apple_pay_data.payment_method.pm_type.clone(),
                            },
                            transaction_identifier: Secret::new(
                                apple_pay_data.transaction_identifier.clone(),
                            ),
                        },
                    };
                    let payment_token = payment_token_data
                        .encode_to_string_of_json()
                        .change_context(IntegrationError::RequestEncodingFailed {
                            context: IntegrationErrorContext {
                                suggested_action: Some(
                                    "Check that the Apple Pay payment token is complete"
                                        .to_string(),
                                ),
                                doc_url: Some(NOON_DOCS_URL.to_string()),
                                additional_context: Some(
                                    "failed to encode Noon APPLEPAY paymentInfo".to_string(),
                                ),
                            },
                        })?;
                    Ok(NoonPaymentData::ApplePay(NoonApplePay {
                        payment_info: Secret::new(payment_token),
                    }))
                }
                ApplePayPaymentData::Decrypted(_) => Err(noon_payment_method_not_supported(
                    "Apple Pay with a pre-decrypted token",
                )),
            },
            WalletData::PaypalRedirect(_) => Ok(NoonPaymentData::PayPal(NoonPayPal {
                return_url: router_return_url.ok_or_else(|| {
                    error_stack::report!(IntegrationError::MissingRequiredField {
                        field_name: "router_return_url",
                        context: IntegrationErrorContext {
                            suggested_action: Some(
                                "Pass return_url: Noon redirects the payer back to it after PayPal"
                                    .to_string(),
                            ),
                            doc_url: Some(NOON_DOCS_URL.to_string()),
                            additional_context: None,
                        },
                    })
                })?,
            })),
            WalletData::AliPayQr(_)
            | WalletData::AliPayRedirect(_)
            | WalletData::AliPayHkRedirect(_)
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
            | WalletData::BluecodeRedirect { .. }
            | WalletData::RevolutPay(_)
            | WalletData::MbWay(_)
            | WalletData::Satispay(_)
            | WalletData::SatispayIntent(_)
            | WalletData::SatispayQr(_)
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
            | WalletData::Neteller(_) => Err(noon_payment_method_not_supported(
                PaymentMethodDataType::from(payment_method_data.clone()),
            )),
        },
        PaymentMethodData::CardRedirect(_)
        | PaymentMethodData::PayLater(_)
        | PaymentMethodData::BankRedirect(_)
        | PaymentMethodData::BankDebit(_)
        | PaymentMethodData::BankTransfer(_)
        | PaymentMethodData::Crypto(_)
        | PaymentMethodData::MandatePayment
        | PaymentMethodData::Reward
        | PaymentMethodData::RealTimePayment(_)
        | PaymentMethodData::CardWithNoCvc(_)
        | PaymentMethodData::MobilePayment(_)
        | PaymentMethodData::Upi(_)
        | PaymentMethodData::Voucher(_)
        | PaymentMethodData::GiftCard(_)
        | PaymentMethodData::OpenBanking(_)
        | PaymentMethodData::PaymentMethodToken(_)
        | PaymentMethodData::NetworkToken(_)
        | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
        | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
            Err(noon_payment_method_not_supported(
                PaymentMethodDataType::from(payment_method_data.clone()),
            ))
        }
    }
}

/// A text Noon accepts as `order.name` / `subscription.name`: whitespace runs collapsed
/// to a single space, no leading/trailing whitespace, at most 50 characters (Noon's
/// limit). `None` when nothing is left, so a blank description counts as absent.
pub(crate) fn normalize_noon_order_name(raw: &str) -> Option<String> {
    let collapsed = raw.split_whitespace().fold(String::new(), |mut acc, word| {
        if !acc.is_empty() {
            acc.push(' ');
        }
        acc.push_str(word);
        acc
    });
    let name: String = collapsed.chars().take(50).collect();
    let name = name.trim_end().to_string();
    (!name.is_empty()).then_some(name)
}

/// Noon `order.name` from the payment description; absent or blank is refused.
fn get_noon_order_name(
    flow_data: &PaymentFlowData,
) -> Result<String, error_stack::Report<IntegrationError>> {
    flow_data
        .description
        .as_deref()
        .and_then(normalize_noon_order_name)
        .ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "description",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Pass a description: Noon requires order.name on every order".to_string(),
                    ),
                    doc_url: Some(NOON_DOCS_URL.to_string()),
                    additional_context: None,
                },
            })
        })
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NoonRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NoonPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        data: NoonRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &data.router_data;

        // Refusals come first so that no request is built for an input Noon cannot take:
        // capture method, then payment method, and only then the missing-field checks, so a
        // payment method Noon does not accept is reported as such rather than as a missing
        // order_category / description.
        let payment_action = get_noon_payment_action(
            item.request.capture_method,
            item.request.amount.amount == MinorUnit::zero(),
        )?;

        // 3DS: Noon runs its own 3DS and decides enrolment itself, so there is no
        // `is_three_ds` branch here; an enrolled card comes back 3DS_ENROLL_INITIATED with a
        // `checkoutData.postUrl` redirect. Externally authenticated 3DS data
        // (`authentication_data`: CAVV/ECI) is deliberately not forwarded: the INITIATE
        // order API documents no input field for it (Hyperswitch ignores it for Noon too).
        let payment_data = build_noon_payment_data(
            &item.request.payment_method_data,
            &item.resource_common_data,
            item.request.router_return_url.clone(),
        )?;

        // A blank order_category is treated as absent rather than sent as "".
        let category = item
            .request
            .order_category
            .clone()
            .filter(|category| !category.trim().is_empty())
            .ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                    field_name: "order_category",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Pass order_category with an order category configured on the Noon \
                         business (for example \"pay\")"
                                .to_string(),
                        ),
                        doc_url: Some(NOON_DOCS_URL.to_string()),
                        additional_context: Some(
                            "Noon requires order.category on every INITIATE order".to_string(),
                        ),
                    },
                })
            })?;
        let name = get_noon_order_name(&item.resource_common_data)?;

        let amount = data
            .connector
            .amount_converter
            .convert(&item.request.amount)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Check that the amount is valid for the currency".to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "failed to convert the amount to Noon's major-unit string".to_string(),
                    ),
                },
            })?;

        let billing =
            get_optional_billing_address(&item.resource_common_data).map(|address| NoonBilling {
                address: NoonBillingAddress {
                    street: address.line1.clone(),
                    street2: address.line2.clone(),
                    city: address.city.clone(),
                    // If state is passed in request, country becomes mandatory, keep a check while debugging failed payments
                    state_province: address.state.clone(),
                    country: address.country,
                    postal_code: address.zip.clone(),
                },
            });

        // Customer-initiated transaction that stores the card for later merchant-initiated
        // charges: only when the request carries mandate details with a mandate amount, because
        // Noon rejects a subscription without subscription.maxAmount (5105). setup_future_usage
        // OFF_SESSION alone is a plain INITIATE (Hyperswitch parity). Mandate details without an
        // amount are refused before any request is sent.
        let subscription = match item.request.setup_mandate_details.as_ref() {
            None => None,
            Some(mandate_data) => {
                let mandate_amount = mandate_data
                    .mandate_type
                    .as_ref()
                    .and_then(|mandate_type| match mandate_type {
                        MandateDataType::SingleUse(amount_data) => Some(amount_data),
                        MandateDataType::MultiUse(amount_data) => amount_data.as_ref(),
                    })
                    .ok_or_else(|| {
                        error_stack::report!(IntegrationError::MissingRequiredField {
                            field_name: "setup_mandate_details.mandate_type",
                            context: IntegrationErrorContext {
                                suggested_action: Some(
                                    "Pass setup_mandate_details.mandate_type with a mandate \
                                     amount (single_use, or multi_use with an amount); Noon \
                                     requires it as subscription.maxAmount"
                                        .to_string(),
                                ),
                                doc_url: Some(NOON_DOCS_URL.to_string()),
                                additional_context: Some(
                                    "Noon rejects a subscription order without \
                                     subscription.maxAmount"
                                        .to_string(),
                                ),
                            },
                        })
                    })?;
                let max_amount = data
                    .connector
                    .amount_converter
                    .convert(&mandate_amount.amount)
                    .change_context(IntegrationError::RequestEncodingFailed {
                        context: IntegrationErrorContext {
                            suggested_action: Some(
                                "Check that the mandate amount is valid for its currency"
                                    .to_string(),
                            ),
                            doc_url: None,
                            additional_context: Some(
                                "failed to convert the mandate amount to Noon \
                                 subscription.maxAmount"
                                    .to_string(),
                            ),
                        },
                    })?;
                Some(NoonSubscriptionData {
                    subscription_type: NoonSubscriptionType::Unscheduled,
                    name: name.clone(),
                    max_amount: Some(max_amount),
                })
            }
        };
        let tokenize_c_c = subscription.is_some().then_some(true);

        let order = NoonOrder {
            amount,
            currency: Some(item.request.amount.currency),
            channel: NoonChannels::Web,
            category: Some(category),
            reference: item
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            name,
            nvp: item
                .request
                .metadata
                .as_ref()
                .map(|m| NoonOrderNvp::new(m.peek())),
            ip_address: item.request.get_ip_address_as_optional(),
        };

        Ok(Self {
            api_operation: NoonApiOperations::Initiate,
            order,
            billing,
            configuration: NoonConfiguration {
                payment_action,
                return_url: item.request.router_return_url.clone(),
                tokenize_c_c,
            },
            payment_data,
            subscription,
        })
    }
}

// Auth Struct
pub struct NoonAuthType {
    pub(super) api_key: Secret<String>,
    pub(super) application_identifier: Secret<String>,
    pub(super) business_identifier: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for NoonAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Noon {
                api_key,
                application_identifier,
                business_identifier,
                ..
            } => Ok(Self {
                api_key: api_key.to_owned(),
                application_identifier: application_identifier.to_owned(),
                business_identifier: business_identifier.to_owned(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}
#[derive(Default, Debug, Deserialize, Serialize, strum::Display)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "UPPERCASE")]
pub enum NoonPaymentStatus {
    Initiated,
    Authorized,
    Captured,
    PartiallyCaptured,
    PartiallyRefunded,
    PaymentInfoAdded,
    #[serde(rename = "3DS_ENROLL_INITIATED")]
    ThreeDsEnrollInitiated,
    #[serde(rename = "3DS_ENROLL_CHECKED")]
    ThreeDsEnrollChecked,
    #[serde(rename = "3DS_RESULT_VERIFIED")]
    ThreeDsResultVerified,
    MarkedForReview,
    Authenticated,
    PartiallyReversed,
    #[default]
    Pending,
    Cancelled,
    Failed,
    Refunded,
    Expired,
    Reversed,
    Rejected,
    Locked,
    /// Any order status Noon adds later (or `TOKENIZED`, which only the tokenization API
    /// returns). Never terminal: the attempt keeps the status it already had.
    #[serde(other)]
    Unknown,
}

/// Maps Noon's `order.status` to an `AttemptStatus` (spec ## Status Mappings).
///
/// `LOCKED` and any unrecognised status leave the attempt's `current_status` unchanged,
/// so neither can move a payment into a terminal state.
pub(crate) fn get_payment_status(
    item: NoonPaymentStatus,
    current_status: AttemptStatus,
) -> AttemptStatus {
    match item {
        NoonPaymentStatus::Authorized => AttemptStatus::Authorized,
        // A partial capture is `PartialCharged`, not `Charged`, so neither PSync nor the
        // webhook implies the full authorised amount was taken; PSync additionally reports the
        // captured total. Refunds are tracked on the refund, not the attempt, so a refunded
        // order stays `Charged`.
        NoonPaymentStatus::PartiallyCaptured => AttemptStatus::PartialCharged,
        NoonPaymentStatus::Captured
        | NoonPaymentStatus::PartiallyRefunded
        | NoonPaymentStatus::Refunded => AttemptStatus::Charged,
        NoonPaymentStatus::Reversed | NoonPaymentStatus::PartiallyReversed => AttemptStatus::Voided,
        NoonPaymentStatus::Cancelled | NoonPaymentStatus::Expired => {
            AttemptStatus::AuthenticationFailed
        }
        NoonPaymentStatus::ThreeDsEnrollInitiated | NoonPaymentStatus::ThreeDsEnrollChecked => {
            AttemptStatus::AuthenticationPending
        }
        NoonPaymentStatus::ThreeDsResultVerified => AttemptStatus::AuthenticationSuccessful,
        NoonPaymentStatus::Failed | NoonPaymentStatus::Rejected => AttemptStatus::Failure,
        NoonPaymentStatus::Pending | NoonPaymentStatus::MarkedForReview => AttemptStatus::Pending,
        NoonPaymentStatus::Initiated
        | NoonPaymentStatus::PaymentInfoAdded
        | NoonPaymentStatus::Authenticated => AttemptStatus::Started,
        NoonPaymentStatus::Locked | NoonPaymentStatus::Unknown => current_status,
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NoonSubscriptionObject {
    identifier: Secret<String>,
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonPaymentsOrderResponse {
    status: NoonPaymentStatus,
    id: u64,
    error_code: u64,
    error_message: Option<String>,
    reference: Option<String>,
    /// Major-unit total Noon has captured on the order (absent on some responses).
    total_captured_amount: Option<f64>,
    currency: Option<enums::Currency>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonCheckoutData {
    post_url: url::Url,
}

/// Noon transaction type (`result.transaction.type`).
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NoonTransactionType {
    Authorize,
    Sale,
    Capture,
    Reverse,
    Refund,
    #[serde(other)]
    Unknown,
}

/// The transaction Noon created for this operation (`result.transaction`). Every field is
/// optional: an absent value is simply not reported.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonTransactionResponse {
    id: Option<String>,
    #[serde(rename = "type")]
    transaction_type: Option<NoonTransactionType>,
    /// Informational only: the attempt status is always derived from `order.status`.
    status: Option<String>,
    authorization_code: Option<String>,
    advice_code: Option<String>,
    raw_external_message: Option<String>,
}

/// Address verification result (`result.avs`). Noon documents no closed set of values,
/// so the status is passed through as received.
#[derive(Debug, Serialize, Deserialize)]
pub struct NoonAvsResult {
    status: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonPaymentsResponseResult {
    order: NoonPaymentsOrderResponse,
    checkout_data: Option<NoonCheckoutData>,
    subscription: Option<NoonSubscriptionObject>,
    transaction: Option<NoonTransactionResponse>,
    avs: Option<NoonAvsResult>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NoonPaymentsResponse {
    result: NoonPaymentsResponseResult,
}

/// Issuer / AVS data for `PaymentFlowData.connector_response`.
///
/// Apple Pay and Google Pay report the authorization code only; cards also report the
/// AVS result as `payment_checks.avs_result`. Nothing is reported when Noon sent neither.
fn build_noon_connector_response(
    result: &NoonPaymentsResponseResult,
    flow_data: &PaymentFlowData,
) -> Option<ConnectorResponseData> {
    let auth_code = result
        .transaction
        .as_ref()
        .and_then(|transaction| transaction.authorization_code.clone());

    if flow_data.payment_method == enums::PaymentMethod::Wallet {
        return match flow_data.payment_method_type {
            Some(wallet_type @ enums::PaymentMethodType::ApplePay)
            | Some(wallet_type @ enums::PaymentMethodType::GooglePay) => auth_code
                .map(|auth_code| ConnectorResponseData::with_auth_code(auth_code, wallet_type)),
            Some(_) | None => None,
        };
    }
    if flow_data.payment_method != enums::PaymentMethod::Card {
        return None;
    }

    let avs_result = result.avs.as_ref().and_then(|avs| avs.status.clone());
    if avs_result.is_none() && auth_code.is_none() {
        return None;
    }
    Some(ConnectorResponseData::with_additional_payment_method_data(
        AdditionalPaymentMethodConnectorResponse::Card {
            authentication_data: None,
            payment_checks: avs_result
                .map(|avs_result| serde_json::json!({ "avs_result": avs_result })),
            card_network: None,
            domestic_network: None,
            auth_code,
            // Noon's order response carries only the AVS result and the auth code; it
            // returns no card metadata.
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

/// The attempt status, flow response and connector response for one Noon order response.
pub(crate) struct NoonPaymentsResponseOutcome {
    pub(crate) status: AttemptStatus,
    pub(crate) response: Result<PaymentsResponseData, ErrorResponse>,
    pub(crate) connector_response: Option<ConnectorResponseData>,
    /// What Noon reports as captured so far, when it reports a positive total.
    pub(crate) amount_captured: Option<Money>,
}

/// Shared mapper for every Noon response that carries `result.order`.
///
/// A 2xx is a failure only when the mapped order status is `Failure` or
/// `AuthenticationFailed` (FAILED, REJECTED, CANCELLED, EXPIRED). Noon keeps the
/// `errorMessage` of a later failed operation on the order (e.g. a CAPTURED order after a
/// rejected refund), so an error message on any other status is not a payment failure.
pub(crate) fn map_noon_payments_response(
    result: NoonPaymentsResponseResult,
    flow_data: &PaymentFlowData,
    http_code: u16,
) -> NoonPaymentsResponseOutcome {
    let connector_response = build_noon_connector_response(&result, flow_data);
    let amount_captured = noon_amount_captured(&result.order, flow_data, http_code);
    let NoonPaymentsResponseResult {
        order,
        checkout_data,
        subscription,
        transaction,
        avs: _,
    } = result;
    let status = get_payment_status(order.status, flow_data.status);
    let connector_transaction_id = order.id.to_string();

    let response = match status {
        AttemptStatus::Failure | AttemptStatus::AuthenticationFailed => {
            let (network_advice_code, network_error_message) = transaction
                .map(|transaction| (transaction.advice_code, transaction.raw_external_message))
                .unwrap_or((None, None));
            Err(ErrorResponse {
                code: if order.error_code == 0 {
                    NO_ERROR_CODE.to_string()
                } else {
                    order.error_code.to_string()
                },
                message: order
                    .error_message
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: order.error_message,
                status_code: http_code,
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: Some(connector_transaction_id),
                network_advice_code,
                network_decline_code: None,
                network_error_message,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            })
        }
        _ => {
            let redirection_data = checkout_data.map(|checkout_data| {
                Box::new(RedirectForm::Form {
                    endpoint: checkout_data.post_url.to_string(),
                    method: Method::Post,
                    form_fields: std::collections::HashMap::new(),
                })
            });
            let mandate_reference = subscription.map(|subscription_data| {
                Box::new(MandateReference {
                    connector_mandate_id: Some(subscription_data.identifier.expose()),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                    mandate_metadata: None,
                })
            });
            // The merchant reference is what RSync's getbyreference lookup needs; callers
            // carry it back through connector_feature_data.
            let connector_metadata = order.reference.as_ref().and_then(|order_reference| {
                serde_json::to_value(NoonConnectorMetadata {
                    order_reference: order_reference.clone(),
                })
                .ok()
            });
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id.clone()),
                redirection_data,
                mandate_reference,
                connector_metadata,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: order.reference.or(Some(connector_transaction_id)),
                incremental_authorization_allowed: None,
                status_code: http_code,
                splits: None,
                payment_account_reference: None,
            })
        }
    };

    NoonPaymentsResponseOutcome {
        status,
        response,
        connector_response,
        amount_captured,
    }
}

/// The captured total as `Money`, so a partial capture is not recorded as the full authorized
/// amount. Currency is the order's own, else the payment's. `None` when Noon reports no
/// positive total or the amount cannot be converted (the amount is then simply not reported).
fn noon_amount_captured(
    order: &NoonPaymentsOrderResponse,
    flow_data: &PaymentFlowData,
    http_code: u16,
) -> Option<Money> {
    let total = order.total_captured_amount.filter(|total| *total > 0.0)?;
    let currency = order
        .currency
        .or_else(|| flow_data.amount.as_ref().map(|money| money.currency))?;
    utils::response_amount_to_money(
        &FloatMajorUnitForConnector,
        FloatMajorUnit(total),
        currency,
        http_code,
        ConnectorEnum::Noon,
    )
    .ok()
}

impl<F, T> TryFrom<ResponseRouterData<NoonPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<NoonPaymentsResponse, Self>) -> Result<Self, Self::Error> {
        let NoonPaymentsResponseOutcome {
            status,
            response,
            connector_response,
            amount_captured,
        } = map_noon_payments_response(
            item.response.result,
            &item.router_data.resource_common_data,
            item.http_code,
        );
        let resource_common_data = item.router_data.resource_common_data;
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_response: connector_response
                    .or(resource_common_data.connector_response.clone()),
                amount_captured: amount_captured.or(resource_common_data.amount_captured.clone()),
                ..resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonActionTransaction {
    amount: StringMajorUnit,
    currency: enums::Currency,
    transaction_reference: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonActionOrder {
    id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonPaymentsActionRequest {
    api_operation: NoonApiOperations,
    order: NoonActionOrder,
    transaction: NoonActionTransaction,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NoonRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for NoonPaymentsActionRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        data: NoonRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &data.router_data;
        // The order id from Authorize (result.order.id) is required; refuse before building
        // the request when it is absent.
        let order = NoonActionOrder {
            id: item
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingRequiredField {
                    field_name: "connector_transaction_id",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Pass the connector_transaction_id (Noon result.order.id) returned \
                             by Authorize"
                                .to_string(),
                        ),
                        doc_url: Some(NOON_DOCS_URL.to_string()),
                        additional_context: Some(
                            "Noon CAPTURE requires order.id of an AUTHORIZED order".to_string(),
                        ),
                    },
                })?,
        };
        // Partial capture is allowed: Noon reports PARTIALLY_CAPTURED for an amount below
        // the authorized total.
        let amount = data
            .connector
            .amount_converter
            .convert(&item.request.amount_to_capture)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Check that the capture amount is valid for the currency".to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "failed to convert the capture amount to Noon's major-unit string"
                            .to_string(),
                    ),
                },
            })?;
        let transaction = NoonActionTransaction {
            amount,
            currency: item.request.amount_to_capture.currency,
            transaction_reference: None,
        };
        Ok(Self {
            api_operation: NoonApiOperations::Capture,
            order,
            transaction,
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonPaymentsCancelRequest {
    api_operation: NoonApiOperations,
    order: NoonActionOrder,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NoonRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for NoonPaymentsCancelRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: NoonRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let order = NoonActionOrder {
            id: item.router_data.request.connector_transaction_id.clone(),
        };
        Ok(Self {
            api_operation: NoonApiOperations::Reverse,
            order,
        })
    }
}

/// Response to a `REVERSE` (Void). Same envelope as every order response; only the
/// status interpretation differs.
#[derive(Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NoonVoidResponse(pub NoonPaymentsResponse);

impl TryFrom<ResponseRouterData<NoonVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<NoonVoidResponse, Self>) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response: NoonVoidResponse(NoonPaymentsResponse { result }),
            router_data,
            http_code,
        } = item;

        // Noon answers a REVERSE that did not release the hold with the order still
        // AUTHORIZED: the reverse failed (spec ## Webhook Events, Reverse: AUTHORIZED =
        // reverse failed). The shared mapper would report that as a successful Authorized
        // response, so it is handled here.
        let (status, response, connector_response, amount_captured) = match result.order.status {
            NoonPaymentStatus::Authorized => {
                let NoonPaymentsResponseResult {
                    order, transaction, ..
                } = result;
                // Forward the gateway advice code and raw message, as the shared mapper does.
                let (network_advice_code, network_error_message) = transaction
                    .map(|transaction| (transaction.advice_code, transaction.raw_external_message))
                    .unwrap_or((None, None));
                (
                    AttemptStatus::VoidFailed,
                    Err(ErrorResponse {
                        code: if order.error_code == 0 {
                            NO_ERROR_CODE.to_string()
                        } else {
                            order.error_code.to_string()
                        },
                        message: order
                            .error_message
                            .clone()
                            .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                        reason: order.error_message.clone(),
                        status_code: http_code,
                        attempt_status: Some(FlowStatus::Payment(AttemptStatus::VoidFailed)),
                        connector_transaction_id: Some(order.id.to_string()),
                        network_advice_code,
                        network_decline_code: None,
                        network_error_message,
                        typed_connector_response: None,
                        raw_connector_response: None,
                        raw_connector_request: None,
                        typed_connector_request: None,
                    }),
                    None,
                    None,
                )
            }
            // REVERSED / PARTIALLY_REVERSED map to Voided; every other status follows the
            // shared order-status mapping.
            NoonPaymentStatus::Initiated
            | NoonPaymentStatus::PaymentInfoAdded
            | NoonPaymentStatus::Authenticated
            | NoonPaymentStatus::ThreeDsEnrollInitiated
            | NoonPaymentStatus::ThreeDsEnrollChecked
            | NoonPaymentStatus::ThreeDsResultVerified
            | NoonPaymentStatus::Captured
            | NoonPaymentStatus::PartiallyCaptured
            | NoonPaymentStatus::PartiallyRefunded
            | NoonPaymentStatus::Refunded
            | NoonPaymentStatus::Reversed
            | NoonPaymentStatus::PartiallyReversed
            | NoonPaymentStatus::Cancelled
            | NoonPaymentStatus::Expired
            | NoonPaymentStatus::Failed
            | NoonPaymentStatus::Rejected
            | NoonPaymentStatus::Pending
            | NoonPaymentStatus::MarkedForReview
            | NoonPaymentStatus::Locked
            | NoonPaymentStatus::Unknown => {
                let NoonPaymentsResponseOutcome {
                    status,
                    response,
                    connector_response,
                    amount_captured,
                } = map_noon_payments_response(
                    result,
                    &router_data.resource_common_data,
                    http_code,
                );
                (status, response, connector_response, amount_captured)
            }
        };

        let resource_common_data = router_data.resource_common_data;
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_response: connector_response
                    .or(resource_common_data.connector_response.clone()),
                amount_captured: amount_captured.or(resource_common_data.amount_captured.clone()),
                ..resource_common_data
            },
            response,
            ..router_data
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonRevokeMandateRequest {
    api_operation: NoonApiOperations,
    subscription: NoonSubscriptionObject,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NoonRouterData<
            RouterDataV2<
                MandateRevoke,
                PaymentFlowData,
                MandateRevokeRequestData,
                MandateRevokeResponseData,
            >,
            T,
        >,
    > for NoonRevokeMandateRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: NoonRouterData<
            RouterDataV2<
                MandateRevoke,
                PaymentFlowData,
                MandateRevokeRequestData,
                MandateRevokeResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            api_operation: NoonApiOperations::CancelSubscription,
            subscription: NoonSubscriptionObject {
                identifier: item.router_data.request.mandate_id,
            },
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NoonRouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for NoonPaymentsActionRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        data: NoonRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &data.router_data;
        let refund_amount = data
            .connector
            .amount_converter
            .convert(&data.router_data.request.refund_amount);
        let order = NoonActionOrder {
            id: item.request.connector_transaction_id.clone(),
        };
        let transaction = NoonActionTransaction {
            amount: refund_amount.change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?,
            currency: item.request.refund_amount.currency,
            transaction_reference: Some(item.request.refund_id.clone()),
        };
        Ok(Self {
            api_operation: NoonApiOperations::Refund,
            order,
            transaction,
        })
    }
}
#[derive(Debug, Deserialize, Serialize)]
pub enum NoonRevokeStatus {
    Cancelled,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NoonCancelSubscriptionObject {
    status: NoonRevokeStatus,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NoonRevokeMandateResult {
    subscription: NoonCancelSubscriptionObject,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NoonRevokeMandateResponse {
    result: NoonRevokeMandateResult,
}

impl TryFrom<ResponseRouterData<NoonRevokeMandateResponse, Self>>
    for RouterDataV2<
        MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        MandateRevokeResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NoonRevokeMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response.result.subscription.status {
            NoonRevokeStatus::Cancelled => Ok(Self {
                response: Ok(MandateRevokeResponseData {
                    mandate_status: common_enums::MandateStatus::Revoked,
                    status_code: item.http_code,
                }),
                ..item.router_data
            }),
        }
    }
}

#[derive(Debug, Default, Deserialize, Clone, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum RefundStatus {
    Success,
    Failed,
    /// Spelling the live sandbox uses for a failed REFUND transaction.
    Failure,
    #[default]
    Pending,
    /// Any status Noon adds later; kept non-terminal.
    #[serde(other)]
    Unknown,
}

impl From<RefundStatus> for enums::RefundStatus {
    fn from(item: RefundStatus) -> Self {
        match item {
            RefundStatus::Success => Self::Success,
            RefundStatus::Failed | RefundStatus::Failure => Self::Failure,
            RefundStatus::Pending | RefundStatus::Unknown => Self::Pending,
        }
    }
}

#[derive(Default, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonPaymentsTransactionResponse {
    id: String,
    status: RefundStatus,
}

#[derive(Default, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonRefundResponseResult {
    transaction: NoonPaymentsTransactionResponse,
}

#[derive(Default, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefundResponse {
    result: NoonRefundResponseResult,
    result_code: u32,
    class_description: Option<String>,
    message: String,
    raw_external_message: Option<String>,
}

impl<F> TryFrom<ResponseRouterData<RefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<RefundResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let refund_status =
            enums::RefundStatus::from(response.result.transaction.status.to_owned());
        let response = if utils::is_refund_failure(refund_status) {
            Err(ErrorResponse {
                status_code: item.http_code,
                code: response.result_code.to_string(),
                message: response.message.clone(),
                reason: Some(response.message.clone()),
                attempt_status: None,
                connector_transaction_id: Some(response.result.transaction.id.clone()),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: response.raw_external_message.clone(),
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            })
        } else {
            Ok(RefundsResponseData {
                connector_refund_id: item.response.result.transaction.id,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            })
        };
        Ok(Self {
            response,
            ..item.router_data
        })
    }
}

/// `connector_metadata` Authorize hands back and RSync reads from `connector_feature_data`.
#[derive(Debug, Serialize, Deserialize)]
pub struct NoonConnectorMetadata {
    pub order_reference: String,
}

/// Path key for RSync's `GET payment/v1/order/getbyreference/{reference}`: the payment's
/// merchant reference (`order.reference`). Hyperswitch sends it as `connector_order_id`;
/// gRPC/SDK callers carry it back from Authorize via `connector_feature_data.order_reference`.
///
/// `request_reference_id` is the last fallback, and the reason refunds of payments created
/// before `connector_order_id` existed can still be synced: Authorize sets `order.reference`
/// from `connector_request_reference_id`, so it is the same value by construction.
/// The refund's own id is never a valid key (Noon answers 19001 for it).
pub fn get_refund_sync_order_reference(
    request: &RefundSyncData,
    request_reference_id: &str,
) -> Result<String, error_stack::Report<IntegrationError>> {
    request
        .connector_order_id
        .as_ref()
        .filter(|reference| !reference.is_empty())
        .cloned()
        .or_else(|| {
            request.connector_feature_data.as_ref().and_then(|data| {
                serde_json::from_value::<NoonConnectorMetadata>(data.peek().clone())
                    .ok()
                    .map(|metadata| metadata.order_reference)
                    .filter(|reference| !reference.is_empty())
            })
        })
        .or_else(|| {
            Some(request_reference_id)
                .filter(|reference| !reference.is_empty())
                .map(str::to_owned)
        })
        .ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "connector_order_id",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Pass the payment's merchant reference (Noon order.reference) as \
                         connector_order_id, or pass back the connector_feature_data returned \
                         by Authorize (it carries order_reference)"
                            .to_string(),
                    ),
                    doc_url: Some(NOON_DOCS_URL.to_string()),
                    additional_context: Some(
                        "Noon looks refunds up through the payment order, not the refund id"
                            .to_string(),
                    ),
                },
            })
        })
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonRefundResponseTransactions {
    id: String,
    #[serde(rename = "type")]
    transaction_type: Option<NoonTransactionType>,
    status: RefundStatus,
    transaction_reference: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonRefundSyncResponseResult {
    transactions: Vec<NoonRefundResponseTransactions>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefundSyncResponse {
    result: NoonRefundSyncResponseResult,
    result_code: Option<u32>,
    class_description: Option<String>,
    message: Option<String>,
}

impl<F> TryFrom<ResponseRouterData<RefundSyncResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<RefundSyncResponse, Self>) -> Result<Self, Self::Error> {
        // The order holds every transaction (SALE, CAPTURE, each REFUND). Select this refund:
        // by Noon's transaction id when known, else the REFUND whose transactionReference is
        // our merchant refund id. No match is an error -- never fall back to another element.
        let transactions = &item.response.result.transactions;
        let connector_refund_id = item.router_data.request.connector_refund_id.as_str();
        let merchant_refund_id = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .as_str();
        let noon_transaction: &NoonRefundResponseTransactions = transactions
            .iter()
            .find(|transaction| {
                !connector_refund_id.is_empty() && transaction.id == connector_refund_id
            })
            .or_else(|| {
                transactions.iter().find(|transaction| {
                    matches!(
                        transaction.transaction_type,
                        Some(NoonTransactionType::Refund)
                    ) && transaction.transaction_reference.as_deref() == Some(merchant_refund_id)
                })
            })
            .ok_or(utils::response_handling_fail_for_connector(
                item.http_code,
                "noon",
            ))?;

        let refund_status = enums::RefundStatus::from(noon_transaction.status.to_owned());
        let response = if utils::is_refund_failure(refund_status) {
            let response = &item.response;
            let message = response
                .message
                .clone()
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string());
            Err(ErrorResponse {
                status_code: item.http_code,
                code: response
                    .result_code
                    .map(|code| code.to_string())
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: message.clone(),
                reason: Some(message),
                attempt_status: None,
                connector_transaction_id: Some(noon_transaction.id.clone()),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            })
        } else {
            Ok(RefundsResponseData {
                connector_refund_id: noon_transaction.id.to_owned(),
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            })
        };
        Ok(Self {
            response,
            ..item.router_data
        })
    }
}

/// Noon webhook `eventType` (spec ## Webhook Events). Noon documents the PascalCase spelling;
/// the UPPERCASE aliases accept the casing seen in older samples.
#[derive(Debug, Deserialize, strum::Display)]
pub enum NoonWebhookEventTypes {
    #[serde(rename = "3DS")]
    #[strum(serialize = "3DS")]
    ThreeDs,
    #[serde(alias = "AUTHENTICATE")]
    Authenticate,
    #[serde(alias = "AUTHORIZE")]
    Authorize,
    #[serde(alias = "CANCEL")]
    Cancel,
    #[serde(alias = "CAPTURE")]
    Capture,
    #[serde(alias = "FAIL")]
    Fail,
    #[serde(alias = "LOCK")]
    Lock,
    #[serde(alias = "REFUND")]
    Refund,
    #[serde(alias = "REJECT")]
    Reject,
    #[serde(alias = "REVERSE")]
    Reverse,
    #[serde(alias = "REVIEW")]
    Review,
    #[serde(alias = "SALE")]
    Sale,
    #[serde(other)]
    Unknown,
}

/// The v1 signature preimage fields. `orderStatus` and `eventType` stay the raw wire strings:
/// Noon signs the values exactly as sent (case-sensitive), so they must not be re-rendered
/// from a parsed enum.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonWebhookBody {
    pub order_id: u64,
    pub order_status: String,
    pub event_type: String,
    pub event_id: String,
    pub time_stamp: String,
}

#[derive(Debug, Deserialize)]
pub struct NoonWebhookSignature {
    pub signature: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonWebhookOrderId {
    pub order_id: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonWebhookEvent {
    pub order_status: NoonPaymentStatus,
    pub event_type: NoonWebhookEventTypes,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonWebhookObject {
    pub order_status: NoonPaymentStatus,
    pub order_id: u64,
    pub event_type: NoonWebhookEventTypes,
    /// Merchant reference sent as `transaction.transactionReference` on REFUND / REVERSE;
    /// Noon echoes it on the matching event.
    pub merchant_txn_ref: Option<String>,
    /// The merchant's order reference (`order.reference` sent on INITIATE), echoed by Noon.
    pub merchant_order_ref: Option<String>,
}

/// This from will ensure that webhook body would be properly parsed into PSync response
impl From<NoonWebhookObject> for NoonPaymentsResponse {
    fn from(value: NoonWebhookObject) -> Self {
        Self {
            result: NoonPaymentsResponseResult {
                order: NoonPaymentsOrderResponse {
                    status: value.order_status,
                    id: value.order_id,
                    //For successful payments Noon Always populates error_code as 0.
                    error_code: 0,
                    error_message: None,
                    reference: None,
                    total_captured_amount: None,
                    currency: None,
                },
                checkout_data: None,
                subscription: None,
                transaction: None,
                avs: None,
            },
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonErrorResponse {
    pub result_code: u32,
    pub message: String,
    pub result_class: Option<u32>,
    pub class_description: Option<String>,
    pub action_hint: Option<String>,
    pub request_reference: Option<String>,
    /// Issuer / acquirer text, when Noon relays one.
    pub raw_external_message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SetupMandateRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(NoonPaymentsRequest<T>);

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NoonRouterData<
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
        data: NoonRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &data.router_data;

        // Refusals come first so that no request is built for an input Noon cannot take:
        // capture method, then payment method, and only then the missing-field checks, so a
        // payment method Noon does not accept is reported as such rather than as a missing
        // order_category / description.
        let payment_action = get_noon_payment_action(
            item.request.capture_method,
            item.request
                .amount
                .as_ref()
                .is_some_and(|money| money.amount == MinorUnit::zero()),
        )?;
        let payment_data = match &item.request.payment_method_data {
            PaymentMethodData::Wallet(WalletData::PaypalRedirect(_)) => Err(
                noon_payment_method_not_supported("PayPal for a mandate setup"),
            ),
            payment_method_data => build_noon_payment_data(
                payment_method_data,
                &item.resource_common_data,
                item.request.router_return_url.clone(),
            ),
        }?;

        // A blank order_category (request field or metadata) is treated as absent.
        let category = item
            .request
            .order_category
            .clone()
            .filter(|category| !category.trim().is_empty())
            .or_else(|| {
                item.request
                    .metadata
                    .as_ref()
                    .and_then(|metadata| metadata.peek().get("order_category"))
                    .and_then(|value| value.as_str())
                    .filter(|category| !category.trim().is_empty())
                    .map(|category| category.to_string())
            })
            .ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                    field_name: "order_category",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Pass order_category (or metadata.order_category) with an order \
                             category configured on the Noon business (for example \"pay\")"
                                .to_string(),
                        ),
                        doc_url: Some(NOON_DOCS_URL.to_string()),
                        additional_context: Some(
                            "Noon requires order.category on every INITIATE order".to_string(),
                        ),
                    },
                })
            })?;
        // PaymentServiceSetupRecurringRequest has no description field and
        // PaymentFlowData.description is always None on this path, so Noon's mandatory
        // order.name is read from the request `metadata` JSON key `description`. The error
        // below names that key so a caller finds the contract the first time it misses it.
        let name = get_noon_order_name(&item.resource_common_data).or_else(|_| {
            item.request
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.peek().get("description"))
                .and_then(|value| value.as_str())
                .and_then(normalize_noon_order_name)
                .ok_or_else(|| {
                    error_stack::report!(IntegrationError::MissingRequiredField {
                        field_name: "metadata.description",
                        context: IntegrationErrorContext {
                            suggested_action: Some(
                                "Noon requires order.name on every order and SetupRecurring has \
                                 no description field: pass it as the `description` key of the \
                                 request `metadata` JSON object, e.g. {\"description\": \"Card on file\"}"
                                    .to_string(),
                            ),
                            doc_url: Some(NOON_DOCS_URL.to_string()),
                            additional_context: None,
                        },
                    })
                })
        })?;
        let order_money = item.request.amount.as_ref().ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "amount",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Pass the mandate-setup amount (zero is allowed for a card-on-file setup)"
                            .to_string(),
                    ),
                    doc_url: Some(NOON_DOCS_URL.to_string()),
                    additional_context: Some(
                        "Noon requires order.amount on the subscription CIT order".to_string(),
                    ),
                },
            })
        })?;
        let mandate_amount = item
            .request
            .setup_mandate_details
            .as_ref()
            .and_then(|mandate_data| mandate_data.mandate_type.as_ref())
            .and_then(|mandate_type| match mandate_type {
                MandateDataType::SingleUse(amount_data) => Some(amount_data),
                MandateDataType::MultiUse(amount_data) => amount_data.as_ref(),
            })
            .ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                    field_name: "setup_mandate_details.mandate_type",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Pass setup_mandate_details.mandate_type with a mandate amount \
                             (single_use, or multi_use with an amount); Noon requires it as \
                             subscription.maxAmount"
                                .to_string(),
                        ),
                        doc_url: Some(NOON_DOCS_URL.to_string()),
                        additional_context: Some(
                            "Noon rejects a subscription order without subscription.maxAmount"
                                .to_string(),
                        ),
                    },
                })
            })?;

        let amount = data
            .connector
            .amount_converter
            .convert(order_money)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Check that the amount is valid for the currency".to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "failed to convert the amount to Noon's major-unit string".to_string(),
                    ),
                },
            })?;

        let ip_address = item.request.browser_info.as_ref().and_then(|browser_info| {
            browser_info
                .ip_address
                .map(|ip| Secret::new(ip.to_string()))
        });

        let billing =
            get_optional_billing_address(&item.resource_common_data).map(|address| NoonBilling {
                address: NoonBillingAddress {
                    street: address.line1.clone(),
                    street2: address.line2.clone(),
                    city: address.city.clone(),
                    state_province: address.state.clone(),
                    country: address.country,
                    postal_code: address.zip.clone(),
                },
            });

        // A mandate setup is always a subscription CIT, and Noon rejects a subscription
        // without subscription.maxAmount (5105); the mandate amount has no default.
        let max_amount = data
            .connector
            .amount_converter
            .convert(&mandate_amount.amount)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Check that the mandate amount is valid for its currency".to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "failed to convert the mandate amount to Noon subscription.maxAmount"
                            .to_string(),
                    ),
                },
            })?;
        let subscription = NoonSubscriptionData {
            subscription_type: NoonSubscriptionType::Unscheduled,
            name: name.clone(),
            max_amount: Some(max_amount),
        };

        let order = NoonOrder {
            amount,
            currency: Some(order_money.currency),
            channel: NoonChannels::Web,
            category: Some(category),
            reference: item
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            name,
            nvp: item
                .request
                .metadata
                .as_ref()
                .map(|m| NoonOrderNvp::new(m.peek())),
            ip_address,
        };

        Ok(Self(NoonPaymentsRequest {
            api_operation: NoonApiOperations::Initiate,
            order,
            billing,
            configuration: NoonConfiguration {
                payment_action,
                return_url: item.request.router_return_url.clone(),
                tokenize_c_c: Some(true),
            },
            payment_data,
            subscription: Some(subscription),
        }))
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupMandateResponse {
    pub result_code: Option<u32>,
    pub message: Option<String>,
    pub result_class: Option<u32>,
    pub class_description: Option<String>,
    pub action_hint: Option<String>,
    pub request_reference: Option<String>,
    pub result: NoonPaymentsResponseResult,
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<SetupMandateResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<SetupMandateResponse, Self>) -> Result<Self, Self::Error> {
        // Same mapping as every other Noon order response, including the mandate reference
        // from result.subscription.identifier and the 3DS redirect from checkoutData.
        let NoonPaymentsResponseOutcome {
            status,
            response,
            connector_response,
            amount_captured,
        } = map_noon_payments_response(
            item.response.result,
            &item.router_data.resource_common_data,
            item.http_code,
        );
        let resource_common_data = item.router_data.resource_common_data;
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_response: connector_response
                    .or(resource_common_data.connector_response.clone()),
                amount_captured: amount_captured.or(resource_common_data.amount_captured.clone()),
                ..resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}

// RepeatPayment types - wrapper around NoonPaymentsRequest
#[derive(Debug, Serialize)]
pub struct NoonRepeatPaymentRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(pub NoonPaymentsRequest<T>);

#[derive(Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NoonRepeatPaymentResponse(pub NoonPaymentsResponse);

// TryFrom for NoonRepeatPaymentRequest - creates a payment using the saved mandate
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NoonRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NoonRepeatPaymentRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: NoonRouterData<
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

        // Refusals come first so that no request is built for an input Noon cannot take.
        let payment_action = get_noon_payment_action(
            router_data.request.capture_method,
            router_data.request.amount.amount == MinorUnit::zero(),
        )?;

        // A Noon MIT charges the subscription created by the mandate setup; Noon has no
        // network-transaction-id based MIT, so only the connector mandate id is usable.
        let payment_data = match &router_data.request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(mandate_ids) => {
                let connector_mandate_id =
                    mandate_ids.get_connector_mandate_id().ok_or_else(|| {
                        error_stack::report!(IntegrationError::MissingRequiredField {
                            field_name: "connector_mandate_id",
                            context: IntegrationErrorContext {
                                suggested_action: Some(
                                    "Pass the connector_mandate_id returned by the Noon mandate \
                                 setup (result.subscription.identifier)"
                                        .to_string(),
                                ),
                                doc_url: Some(NOON_DOCS_URL.to_string()),
                                additional_context: Some(
                                    "Noon requires paymentData.data.subscriptionIdentifier on a \
                                 recurring charge"
                                        .to_string(),
                                ),
                            },
                        })
                    })?;
                NoonPaymentData::Subscription(NoonSubscription {
                    subscription_identifier: Secret::new(connector_mandate_id),
                })
            }
            MandateReferenceId::NetworkMandateId(_)
            | MandateReferenceId::NetworkTokenWithNTI(_) => {
                return Err(error_stack::report!(IntegrationError::NotSupported {
                    message: "network transaction id based recurring charge".to_string(),
                    connector: "Noon",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Set up the mandate with Noon and charge it with the returned \
                             connector_mandate_id"
                                .to_string(),
                        ),
                        doc_url: Some(NOON_DOCS_URL.to_string()),
                        additional_context: Some(
                            "Noon recurring charges reference a Noon subscription identifier; \
                             Noon accepts no network transaction id"
                                .to_string(),
                        ),
                    },
                }));
            }
        };

        let name = get_noon_order_name(&router_data.resource_common_data)?;

        let amount = item
            .connector
            .amount_converter
            .convert(&router_data.request.amount)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Check that the amount is valid for the currency".to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "failed to convert the amount to Noon's major-unit string".to_string(),
                    ),
                },
            })?;

        // Get IP address
        let ip_address = router_data.request.get_ip_address_as_optional();

        let channel = NoonChannels::Web;

        let billing =
            get_optional_billing_address(&router_data.resource_common_data).map(|address| {
                NoonBilling {
                    address: NoonBillingAddress {
                        street: address.line1.clone(),
                        street2: address.line2.clone(),
                        city: address.city.clone(),
                        state_province: address.state.clone(),
                        country: address.country,
                        postal_code: address.zip.clone(),
                    },
                }
            });

        // Noon doesn't accept currency and category in order for repeat payments using mandate
        let order = NoonOrder {
            amount,
            currency: None,
            channel,
            category: None,
            reference: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            name,
            nvp: router_data
                .request
                .metadata
                .as_ref()
                .map(|m| NoonOrderNvp::new(m.peek())),
            ip_address,
        };

        Ok(Self(NoonPaymentsRequest {
            api_operation: NoonApiOperations::Initiate,
            order,
            billing,
            configuration: NoonConfiguration {
                payment_action,
                return_url: router_data.request.router_return_url.clone(),
                tokenize_c_c: None, // Already tokenized via mandate
            },
            payment_data,
            subscription: None, // Not needed for repeat payment using existing mandate
        }))
    }
}

// TryFrom for NoonRepeatPaymentResponse - the MIT order response is mapped exactly like every
// other Noon order response (status, failure predicate, mandate reference, connector response).
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<NoonRepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NoonRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let NoonPaymentsResponseOutcome {
            status,
            response,
            connector_response,
            amount_captured,
        } = map_noon_payments_response(
            item.response.0.result,
            &item.router_data.resource_common_data,
            item.http_code,
        );
        let resource_common_data = item.router_data.resource_common_data;
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_response: connector_response
                    .or(resource_common_data.connector_response.clone()),
                amount_captured: amount_captured.or(resource_common_data.amount_captured.clone()),
                ..resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}
