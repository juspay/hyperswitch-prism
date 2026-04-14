use common_enums::enums::{self, AttemptStatus, CountryAlpha2};
use common_utils::{ext_traits::Encode, pii, request::Method, types::StringMajorUnit};
use domain_types::{
    connector_flow::{
        Authorize, Capture, CreateOrder, MandateRevoke, Refund, RepeatPayment, SetupMandate, Void,
    },
    connector_types::{
        MandateReference, MandateReferenceId, MandateRevokeRequestData, MandateRevokeResponseData,
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, RefundFlowData,
        RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData, ResponseId,
        SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    mandates::MandateDataType,
    payment_method_data::{
        GooglePayWalletData, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber, WalletData,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
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
    max_amount: StringMajorUnit,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    currency: Option<enums::Currency>,
    channel: NoonChannels,
    #[serde(skip_serializing_if = "Option::is_none")]
    category: Option<String>,
    reference: String,
    //Short description of the order.
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    nvp: Option<NoonOrderNvp>,
    #[serde(skip_serializing_if = "Option::is_none")]
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
    payment_method_data: GooglePayWalletData,
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
        let amount = data
            .connector
            .amount_converter
            .convert(
                data.router_data.request.minor_amount,
                data.router_data.request.currency,
            )
            .change_context(IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    doc_url: Some("https://docs.noonpayments.com/payment-api/reference/initiate-payment".to_string()),
                    suggested_action: Some("Ensure the payment amount is valid and within Noon's acceptable range for the specified currency".to_string()),
                    additional_context: Some(format!(
                        "Failed to convert amount {} {} to Noon format for authorize request",
                        data.router_data.request.minor_amount, data.router_data.request.currency
                    )),
                },
            })?;

        let payment_data = match item.request.payment_method_data.clone() {
            PaymentMethodData::Card(req_card) => Ok(NoonPaymentData::Card(NoonCard {
                name_on_card: item.resource_common_data.get_optional_billing_full_name(),
                number_plain: req_card.card_number.clone(),
                expiry_month: req_card.card_exp_month.clone(),
                expiry_year: req_card.get_expiry_year_4_digit(),
                cvv: req_card.card_cvc,
            })),
            PaymentMethodData::Wallet(wallet_data) => match wallet_data.clone() {
                WalletData::GooglePay(google_pay_data) => {
                    Ok(NoonPaymentData::GooglePay(NoonGooglePay {
                        api_version_minor: GOOGLEPAY_API_VERSION_MINOR,
                        api_version: GOOGLEPAY_API_VERSION,
                        payment_method_data: google_pay_data,
                    }))
                }
                WalletData::ApplePay(apple_pay_data) => {
                    let payment_token_data = NoonApplePayTokenData {
                        token: NoonApplePayData {
                            payment_data: wallet_data
                                .get_wallet_token_as_json("Apple Pay".to_string())?,
                            payment_method: NoonApplePayPaymentMethod {
                                display_name: apple_pay_data.payment_method.display_name,
                                network: apple_pay_data.payment_method.network,
                                pm_type: apple_pay_data.payment_method.pm_type,
                            },
                            transaction_identifier: Secret::new(
                                apple_pay_data.transaction_identifier,
                            ),
                        },
                    };
                    let payment_token = payment_token_data
                        .encode_to_string_of_json()
                        .change_context(IntegrationError::RequestEncodingFailed {
                            context: IntegrationErrorContext {
                                doc_url: Some("https://docs.noonpayments.com/payment-method/apple-pay".to_string()),
                                suggested_action: Some("Verify the Apple Pay payment token is properly formatted and contains all required fields".to_string()),
                                additional_context: Some("Failed to encode Apple Pay payment token data to JSON string for authorize request".to_string()),
                            },
                        })?;

                    Ok(NoonPaymentData::ApplePay(NoonApplePay {
                        payment_info: Secret::new(payment_token),
                    }))
                }
                WalletData::PaypalRedirect(_) => Ok(NoonPaymentData::PayPal(NoonPayPal {
                    return_url: item.request.get_router_return_url()?,
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
                | WalletData::Wero(_)
                | WalletData::LazyPayRedirect(_)
                | WalletData::PhonePeRedirect(_)
                | WalletData::BillDeskRedirect(_)
                | WalletData::CashfreeRedirect(_)
                | WalletData::PayURedirect(_)
                | WalletData::EaseBuzzRedirect(_) => Err(IntegrationError::not_implemented(
                    utils::get_unimplemented_payment_method_error_message("Noon"),
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
            | PaymentMethodData::MobilePayment(_)
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::PaymentMethodToken(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
                Err(IntegrationError::not_implemented(
                    utils::get_unimplemented_payment_method_error_message("Noon"),
                ))
            }
        }?;

        let currency = Some(item.request.currency);
        let category = Some(item.request.order_category.clone().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "order_category",
                context: IntegrationErrorContext {
                    doc_url: Some("https://docs.noonpayments.com/payment-api/reference/initiate-payment".to_string()),
                    suggested_action: Some("Provide the order_category field in the payment request to classify the type of goods or services".to_string()),
                    additional_context: Some("order_category is required for Noon authorize payments to specify the nature of the transaction".to_string()),
                },
            },
        )?);

        let ip_address = item.request.get_ip_address_as_optional();

        let channel = NoonChannels::Web;

        let billing = item
            .resource_common_data
            .get_optional_billing()
            .and_then(|billing_address| billing_address.address.as_ref())
            .map(|address| NoonBilling {
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

        // The description should not have leading or trailing whitespaces, also it should not have double whitespaces and a max 50 chars according to Noon's Docs
        let name: String = item
            .resource_common_data
            .get_description()?
            .trim()
            .replace("  ", " ")
            .chars()
            .take(50)
            .collect();

        let order = NoonOrder {
            amount,
            currency,
            channel,
            category,
            reference: item
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            name: name.clone(),
            nvp: item
                .request
                .metadata
                .as_ref()
                .map(|m| NoonOrderNvp::new(m.peek())),
            ip_address,
        };
        let payment_action = if item.request.is_auto_capture() {
            NoonPaymentActions::Sale
        } else {
            NoonPaymentActions::Authorize
        };

        // Handle subscription/mandate data for mandate creation
        let subscription = item
            .request
            .setup_mandate_details
            .as_ref()
            .and_then(|mandate_data| {
                mandate_data.mandate_type.as_ref().and_then(|mandate_type| {
                    let mandate_amount_data = match mandate_type {
                        MandateDataType::SingleUse(amount_data) => Some(amount_data),
                        MandateDataType::MultiUse(amount_data_opt) => amount_data_opt.as_ref(),
                    };
                    mandate_amount_data.map(|amount_data| {
                        data.connector
                            .amount_converter
                            .convert(amount_data.amount, amount_data.currency)
                            .map(|max_amount| NoonSubscriptionData {
                                subscription_type: NoonSubscriptionType::Unscheduled,
                                name: name.clone(),
                                max_amount,
                            })
                    })
                })
            })
            .transpose()
            .change_context(IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    doc_url: Some("https://docs.noonpayments.com/subscriptions".to_string()),
                    suggested_action: Some("Verify the subscription details contain valid mandate name and max_amount fields".to_string()),
                    additional_context: Some("Failed to process subscription/mandate details for authorize request".to_string()),
                },
            })?;

        let tokenize_c_c = subscription.is_some().then_some(true);

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
                context: IntegrationErrorContext {
                    doc_url: Some("https://docs.noonpayments.com/payment-api/authentication".to_string()),
                    suggested_action: Some("Provide valid Noon API credentials (api_key, application_identifier, business_identifier) in the connector configuration".to_string()),
                    additional_context: Some("Failed to obtain Noon authentication credentials from connector configuration".to_string()),
                },
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
}

fn get_payment_status(item: NoonPaymentStatus) -> AttemptStatus {
    match item {
        NoonPaymentStatus::Authorized => AttemptStatus::Authorized,
        NoonPaymentStatus::Captured
        | NoonPaymentStatus::PartiallyCaptured
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
        NoonPaymentStatus::Locked => AttemptStatus::Unspecified,
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
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonCheckoutData {
    post_url: url::Url,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonPaymentsResponseResult {
    order: NoonPaymentsOrderResponse,
    checkout_data: Option<NoonCheckoutData>,
    subscription: Option<NoonSubscriptionObject>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NoonPaymentsResponse {
    result: NoonPaymentsResponseResult,
}

impl<F, T> TryFrom<ResponseRouterData<NoonPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<NoonPaymentsResponse, Self>) -> Result<Self, Self::Error> {
        let order = item.response.result.order;
        let status = get_payment_status(order.status);
        let redirection_data = item.response.result.checkout_data.map(|redirection_data| {
            Box::new(RedirectForm::Form {
                endpoint: redirection_data.post_url.to_string(),
                method: Method::Post,
                form_fields: std::collections::HashMap::new(),
            })
        });
        let mandate_reference = item.response.result.subscription.map(|subscription_data| {
            Box::new(MandateReference {
                connector_mandate_id: Some(subscription_data.identifier.expose()),
                payment_method_id: None,
                connector_mandate_request_reference_id: None,
            })
        });
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response: match order.error_message {
                Some(error_message) => Err(ErrorResponse {
                    code: order.error_code.to_string(),
                    message: error_message.clone(),
                    reason: Some(error_message),
                    status_code: item.http_code,
                    attempt_status: Some(status),
                    connector_transaction_id: Some(order.id.to_string()),
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                }),
                _ => {
                    let connector_response_reference_id =
                        order.reference.or(Some(order.id.to_string()));
                    Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(order.id.to_string()),
                        redirection_data,
                        mandate_reference,
                        connector_metadata: None,
                        network_txn_id: None,
                        connector_response_reference_id,
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                    })
                }
            },
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
        let amount = data.connector.amount_converter.convert(
            data.router_data.request.minor_amount_to_capture,
            data.router_data.request.currency,
        );
        let order = NoonActionOrder {
            id: item
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingRequiredField {
                    field_name: "connector_transaction_id",
                    context: IntegrationErrorContext {
                        doc_url: Some("https://docs.noonpayments.com/payment-api/reference/capture-payment".to_string()),
                        suggested_action: Some("Ensure the payment has been authorized and a connector_transaction_id is available from Noon".to_string()),
                        additional_context: Some("connector_transaction_id is required to identify the transaction for capture".to_string()),
                    },
                })?,
        };
        let transaction = NoonActionTransaction {
            amount: amount.change_context(IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    doc_url: Some("https://docs.noonpayments.com/payment-api/reference/capture-payment".to_string()),
                    suggested_action: Some("Ensure the capture amount is valid and within the authorized amount for the specified currency".to_string()),
                    additional_context: Some(format!(
                        "Failed to convert capture amount {} {} to Noon format",
                        data.router_data.request.minor_amount_to_capture, data.router_data.request.currency
                    )),
                },
            })?,
            currency: item.request.currency,
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
        let refund_amount = data.connector.amount_converter.convert(
            data.router_data.request.minor_refund_amount,
            data.router_data.request.currency,
        );
        let order = NoonActionOrder {
            id: item.request.connector_transaction_id.clone(),
        };
        let transaction = NoonActionTransaction {
            amount: refund_amount.change_context(IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    doc_url: Some("https://docs.noonpayments.com/payment-api/reference/refund-payment".to_string()),
                    suggested_action: Some("Ensure the refund amount is valid and within the captured amount for the specified currency".to_string()),
                    additional_context: Some(format!(
                        "Failed to convert refund amount {} {} to Noon format",
                        data.router_data.request.minor_refund_amount, data.router_data.request.currency
                    )),
                },
            })?,
            currency: item.request.currency,
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
    #[default]
    Pending,
}

impl From<RefundStatus> for enums::RefundStatus {
    fn from(item: RefundStatus) -> Self {
        match item {
            RefundStatus::Success => Self::Success,
            RefundStatus::Failed => Self::Failure,
            RefundStatus::Pending => Self::Pending,
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
    class_description: String,
    message: String,
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
                network_error_message: None,
            })
        } else {
            Ok(RefundsResponseData {
                connector_refund_id: item.response.result.transaction.id,
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

#[derive(Default, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonRefundResponseTransactions {
    id: String,
    status: RefundStatus,
    transaction_reference: Option<String>,
}

#[derive(Default, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonRefundSyncResponseResult {
    transactions: Vec<NoonRefundResponseTransactions>,
}

#[derive(Default, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefundSyncResponse {
    result: NoonRefundSyncResponseResult,
    result_code: u32,
    class_description: String,
    message: String,
}

impl<F> TryFrom<ResponseRouterData<RefundSyncResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<RefundSyncResponse, Self>) -> Result<Self, Self::Error> {
        let noon_transaction: &NoonRefundResponseTransactions = item
            .response
            .result
            .transactions
            .iter()
            .find(|transaction| transaction.transaction_reference.is_some())
            .ok_or(utils::response_handling_fail_for_connector(
                item.http_code,
                "noon",
            ))?;

        let refund_status = enums::RefundStatus::from(noon_transaction.status.to_owned());
        let response = if utils::is_refund_failure(refund_status) {
            let response = &item.response;
            Err(ErrorResponse {
                status_code: item.http_code,
                code: response.result_code.to_string(),
                message: response.message.clone(),
                reason: Some(response.message.clone()),
                attempt_status: None,
                connector_transaction_id: Some(noon_transaction.id.clone()),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            })
        } else {
            Ok(RefundsResponseData {
                connector_refund_id: noon_transaction.id.to_owned(),
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

#[derive(Debug, Deserialize, strum::Display)]
pub enum NoonWebhookEventTypes {
    Authenticate,
    Authorize,
    Capture,
    Fail,
    Refund,
    Sale,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonWebhookBody {
    pub order_id: u64,
    pub order_status: NoonPaymentStatus,
    pub event_type: NoonWebhookEventTypes,
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
                },
                checkout_data: None,
                subscription: None,
            },
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonErrorResponse {
    pub result_code: u32,
    pub message: String,
    pub class_description: String,
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
        let amount = data.connector.amount_converter.convert(
            common_utils::types::MinorUnit::new(1),
            data.router_data.request.currency,
        );
        let mandate_amount = &data.router_data.request.setup_mandate_details;

        let (payment_data, currency, category) = match &item.request.mandate_id {
            Some(mandate_ids) => match &mandate_ids.mandate_reference_id {
                Some(MandateReferenceId::ConnectorMandateId(connector_mandate_ids)) => {
                    if let Some(mandate_id) = connector_mandate_ids.get_connector_mandate_id() {
                        (
                            NoonPaymentData::Subscription(NoonSubscription {
                                subscription_identifier: Secret::new(mandate_id),
                            }),
                            None,
                            None,
                        )
                    } else {
                        return Err(IntegrationError::MissingRequiredField {
                            field_name: "connector_mandate_id",
                            context: IntegrationErrorContext {
                                doc_url: Some("https://docs.noonpayments.com/subscriptions".to_string()),
                                suggested_action: Some("Ensure a valid connector_mandate_id is available from previous mandate setup".to_string()),
                                additional_context: Some("connector_mandate_id is required to verify an existing mandate in setup mandate flow".to_string()),
                            },
                        }
                        .into());
                    }
                }
                _ => {
                    return Err(IntegrationError::MissingRequiredField {
                        field_name: "connector_mandate_id",
                        context: IntegrationErrorContext {
                            doc_url: Some("https://docs.noonpayments.com/subscriptions".to_string()),
                            suggested_action: Some("Provide a connector_mandate_id in the mandate_reference_id field".to_string()),
                            additional_context: Some("connector_mandate_id is required in setup mandate flow when mandate_reference_id is present".to_string()),
                        },
                    }
                    .into());
                }
            },
            None => (
                match item.request.payment_method_data.clone() {
                    PaymentMethodData::Card(req_card) => Ok(NoonPaymentData::Card(NoonCard {
                        name_on_card: item.resource_common_data.get_optional_billing_full_name(),
                        number_plain: req_card.card_number.clone(),
                        expiry_month: req_card.card_exp_month.clone(),
                        expiry_year: req_card.get_expiry_year_4_digit(),
                        cvv: req_card.card_cvc,
                    })),
                    PaymentMethodData::Wallet(wallet_data) => match wallet_data.clone() {
                        WalletData::GooglePay(google_pay_data) => {
                            Ok(NoonPaymentData::GooglePay(NoonGooglePay {
                                api_version_minor: GOOGLEPAY_API_VERSION_MINOR,
                                api_version: GOOGLEPAY_API_VERSION,
                                payment_method_data: google_pay_data,
                            }))
                        }
                        WalletData::ApplePay(apple_pay_data) => {
                            let payment_token_data = NoonApplePayTokenData {
                                token: NoonApplePayData {
                                    payment_data: wallet_data
                                        .get_wallet_token_as_json("Apple Pay".to_string())?,
                                    payment_method: NoonApplePayPaymentMethod {
                                        display_name: apple_pay_data.payment_method.display_name,
                                        network: apple_pay_data.payment_method.network,
                                        pm_type: apple_pay_data.payment_method.pm_type,
                                    },
                                    transaction_identifier: Secret::new(
                                        apple_pay_data.transaction_identifier,
                                    ),
                                },
                            };
                            let payment_token = payment_token_data
                                .encode_to_string_of_json()
                                .change_context(IntegrationError::RequestEncodingFailed {
                                    context: IntegrationErrorContext {
                                        doc_url: Some("https://docs.noonpayments.com/payment-method/apple-pay".to_string()),
                                        suggested_action: Some("Verify the Apple Pay payment token is properly formatted for mandate setup".to_string()),
                                        additional_context: Some("Failed to encode Apple Pay payment token data to JSON string for setup mandate request".to_string()),
                                    },
                                })?;

                            Ok(NoonPaymentData::ApplePay(NoonApplePay {
                                payment_info: Secret::new(payment_token),
                            }))
                        }
                        WalletData::PaypalRedirect(_) => Ok(NoonPaymentData::PayPal(NoonPayPal {
                            return_url: item.request.get_router_return_url()?,
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
                        | WalletData::BluecodeRedirect { .. }
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
                        | WalletData::EaseBuzzRedirect(_) => {
                            Err(IntegrationError::not_implemented(
                                utils::get_unimplemented_payment_method_error_message("Noon"),
                            ))
                        }
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
                    | PaymentMethodData::MobilePayment(_)
                    | PaymentMethodData::Upi(_)
                    | PaymentMethodData::Voucher(_)
                    | PaymentMethodData::GiftCard(_)
                    | PaymentMethodData::OpenBanking(_)
                    | PaymentMethodData::PaymentMethodToken(_)
                    | PaymentMethodData::NetworkToken(_)
                    | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
                    | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
                        Err(IntegrationError::not_implemented(
                            utils::get_unimplemented_payment_method_error_message("Noon"),
                        ))
                    }
                }?,
                Some(item.request.currency),
                // Get order_category from metadata field, return error if not provided
                Some(
                    item.request
                        .metadata
                        .as_ref()
                        .and_then(|metadata| metadata.peek().get("order_category"))
                        .and_then(|value| value.as_str())
                        .map(|s| s.to_string())
                        .ok_or(IntegrationError::MissingRequiredField {
                            field_name: "order_category in metadata",
                            context: IntegrationErrorContext {
                                doc_url: Some("https://docs.noonpayments.com/subscriptions".to_string()),
                                suggested_action: Some("Include order_category in the metadata field for setup mandate requests".to_string()),
                                additional_context: Some("order_category must be provided in metadata for Noon setup mandate flow to classify the transaction type".to_string()),
                            },
                        })?,
                ),
            ),
        };

        let ip_address = item.request.browser_info.as_ref().and_then(|browser_info| {
            browser_info
                .ip_address
                .map(|ip| Secret::new(ip.to_string()))
        });

        let channel = NoonChannels::Web;

        let billing = item
            .resource_common_data
            .get_optional_billing()
            .and_then(|billing_address| billing_address.address.as_ref())
            .map(|address| NoonBilling {
                address: NoonBillingAddress {
                    street: address.line1.clone(),
                    street2: address.line2.clone(),
                    city: address.city.clone(),
                    state_province: address.state.clone(),
                    country: address.country,
                    postal_code: address.zip.clone(),
                },
            });

        // The description should not have leading or trailing whitespaces, also it should not have double whitespaces and a max 50 chars according to Noon's Docs
        let name: String = item
            .resource_common_data
            .get_description()?
            .trim()
            .replace("  ", " ")
            .chars()
            .take(50)
            .collect();

        let subscription = mandate_amount
            .as_ref()
            .and_then(|mandate_data| {
                mandate_data.mandate_type.as_ref().and_then(|mandate_type| {
                    let mandate_amount_data = match mandate_type {
                        MandateDataType::SingleUse(amount_data) => Some(amount_data),
                        MandateDataType::MultiUse(amount_data_opt) => amount_data_opt.as_ref(),
                    };
                    mandate_amount_data.map(|amount_data| {
                        data.connector
                            .amount_converter
                            .convert(amount_data.amount, amount_data.currency)
                            .map(|max_amount| NoonSubscriptionData {
                                subscription_type: NoonSubscriptionType::Unscheduled,
                                name: name.clone(),
                                max_amount,
                            })
                    })
                })
            })
            .transpose()
            .change_context(IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    doc_url: Some("https://docs.noonpayments.com/subscriptions".to_string()),
                    suggested_action: Some("Verify the mandate details contain valid amount and currency for subscription setup".to_string()),
                    additional_context: Some("Failed to process subscription/mandate details for setup mandate request".to_string()),
                },
            })?;

        let tokenize_c_c = subscription.is_some().then_some(true);

        let order = NoonOrder {
            amount: amount.change_context(IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    doc_url: Some("https://docs.noonpayments.com/subscriptions".to_string()),
                    suggested_action: Some("Ensure a valid amount (even a minimal one like 1 unit) is provided for setup mandate".to_string()),
                    additional_context: Some("Failed to convert amount to Noon format for setup mandate request".to_string()),
                },
            })?,
            currency,
            channel,
            category,
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
        let payment_action = match item.request.capture_method {
            Some(common_enums::CaptureMethod::Automatic)
            | None
            | Some(common_enums::CaptureMethod::SequentialAutomatic) => NoonPaymentActions::Sale,
            Some(common_enums::CaptureMethod::Manual) => NoonPaymentActions::Authorize,
            Some(_) => NoonPaymentActions::Authorize,
        };
        Ok(Self(NoonPaymentsRequest {
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
        }))
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupMandateResponse {
    pub result_code: u32,
    pub message: String,
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
        let order = item.response.result.order;
        let status = get_payment_status(order.status);
        let redirection_data = item.response.result.checkout_data.map(|redirection_data| {
            Box::new(RedirectForm::Form {
                endpoint: redirection_data.post_url.to_string(),
                method: Method::Post,
                form_fields: std::collections::HashMap::new(),
            })
        });
        let mandate_reference = item.response.result.subscription.map(|subscription_data| {
            Box::new(MandateReference {
                connector_mandate_id: Some(subscription_data.identifier.expose()),
                payment_method_id: None,
                connector_mandate_request_reference_id: None,
            })
        });
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response: match order.error_message {
                Some(error_message) => Err(ErrorResponse {
                    code: order.error_code.to_string(),
                    message: error_message.clone(),
                    reason: Some(error_message),
                    status_code: item.http_code,
                    attempt_status: Some(status),
                    connector_transaction_id: Some(order.id.to_string()),
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                }),
                _ => {
                    let connector_response_reference_id =
                        order.reference.or(Some(order.id.to_string()));
                    Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(order.id.to_string()),
                        redirection_data,
                        mandate_reference,
                        connector_metadata: None,
                        network_txn_id: None,
                        connector_response_reference_id,
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                    })
                }
            },
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
        let amount = item
            .connector
            .amount_converter
            .convert(
                router_data.request.minor_amount,
                router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: IntegrationErrorContext {
                    doc_url: Some("https://docs.noonpayments.com/subscriptions".to_string()),
                    suggested_action: Some("Ensure the payment amount is valid and within acceptable limits for the specified currency".to_string()),
                    additional_context: Some(format!(
                        "Failed to convert amount {} {} to Noon format for repeat payment request",
                        router_data.request.minor_amount, router_data.request.currency
                    )),
                },
            })?;

        // For repeat payments, use the subscription payment method with the mandate ID
        let payment_data = match &router_data.request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(mandate_ids) => {
                let connector_mandate_id = mandate_ids.get_connector_mandate_id().ok_or(
                    IntegrationError::MissingRequiredField {
                        field_name: "connector_mandate_id",
                        context: IntegrationErrorContext {
                            doc_url: Some("https://docs.noonpayments.com/subscriptions".to_string()),
                            suggested_action: Some("Ensure the mandate has been set up and a valid connector_mandate_id is available".to_string()),
                            additional_context: Some("connector_mandate_id is required for repeat payment to identify the saved payment method".to_string()),
                        },
                    },
                )?;
                NoonPaymentData::Subscription(NoonSubscription {
                    subscription_identifier: Secret::new(connector_mandate_id.to_string()),
                })
            }
            MandateReferenceId::NetworkMandateId(_)
            | MandateReferenceId::NetworkTokenWithNTI(_) => {
                return Err(IntegrationError::not_implemented(
                    "Only connector mandate ID is supported for Noon repeat payments".to_string(),
                )
                .into())
            }
        };

        // Get IP address
        let ip_address = router_data.request.get_ip_address_as_optional();

        let channel = NoonChannels::Web;

        let billing = router_data
            .resource_common_data
            .get_optional_billing()
            .and_then(|billing_address| billing_address.address.as_ref())
            .map(|address| NoonBilling {
                address: NoonBillingAddress {
                    street: address.line1.clone(),
                    street2: address.line2.clone(),
                    city: address.city.clone(),
                    state_province: address.state.clone(),
                    country: address.country,
                    postal_code: address.zip.clone(),
                },
            });

        // Clean description
        let name: String = router_data
            .resource_common_data
            .get_description()?
            .trim()
            .replace("  ", " ")
            .chars()
            .take(50)
            .collect();

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

        // Determine payment action based on capture method
        let payment_action = if router_data.request.is_auto_capture() {
            NoonPaymentActions::Sale
        } else {
            NoonPaymentActions::Authorize
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

// TryFrom for NoonRepeatPaymentResponse - delegates to existing NoonPaymentsResponse
// Since NoonPaymentsResponse has a generic TryFrom for any F, we need to create a wrapper type
// that allows us to use the existing implementation for RepeatPayment flow
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<NoonRepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NoonRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Unwrap the response and process it directly
        let ResponseRouterData {
            response: NoonRepeatPaymentResponse(payments_response),
            router_data,
            http_code,
        } = item;

        let order = payments_response.result.order;
        let status = get_payment_status(order.status);
        let redirection_data = payments_response
            .result
            .checkout_data
            .map(|redirection_data| {
                Box::new(RedirectForm::Form {
                    endpoint: redirection_data.post_url.to_string(),
                    method: Method::Post,
                    form_fields: std::collections::HashMap::new(),
                })
            });
        let mandate_reference = payments_response
            .result
            .subscription
            .map(|subscription_data| {
                Box::new(MandateReference {
                    connector_mandate_id: Some(subscription_data.identifier.expose()),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                })
            });
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            response: match order.error_message {
                Some(error_message) => Err(ErrorResponse {
                    code: order.error_code.to_string(),
                    message: error_message.clone(),
                    reason: Some(error_message),
                    status_code: http_code,
                    attempt_status: Some(status),
                    connector_transaction_id: Some(order.id.to_string()),
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                }),
                _ => {
                    let connector_response_reference_id =
                        order.reference.or(Some(order.id.to_string()));
                    Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(order.id.to_string()),
                        redirection_data,
                        mandate_reference,
                        connector_metadata: None,
                        network_txn_id: None,
                        connector_response_reference_id,
                        incremental_authorization_allowed: None,
                        status_code: http_code,
                    })
                }
            },
            ..router_data
        })
    }
}

// CreateOrder types and implementations
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonCreateOrderRequest {
    api_operation: NoonApiOperations,
    order: NoonOrder,
    configuration: NoonConfiguration,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NoonCreateOrderResponse {
    result: NoonCreateOrderResponseResult,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonCreateOrderResponseResult {
    order: NoonCreateOrderOrderResponse,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonCreateOrderOrderResponse {
    id: u64,
    status: NoonPaymentStatus,
    reference: Option<String>,
}

// TryFrom for CreateOrder Request
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NoonRouterData<
            RouterDataV2<
                CreateOrder,
                PaymentFlowData,
                PaymentCreateOrderData,
                PaymentCreateOrderResponse,
            >,
            T,
        >,
    > for NoonCreateOrderRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        data: NoonRouterData<
            RouterDataV2<
                CreateOrder,
                PaymentFlowData,
                PaymentCreateOrderData,
                PaymentCreateOrderResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &data.router_data;

        let amount = data
            .connector
            .amount_converter
            .convert(item.request.amount, item.request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    doc_url: Some("https://docs.noonpayments.com/payment-api/reference/get-order".to_string()),
                    suggested_action: Some("Ensure the payment amount is valid and within Noon's acceptable range for the specified currency".to_string()),
                    additional_context: Some(format!(
                        "Failed to convert amount {} {} to Noon format for create order request",
                        item.request.amount, item.request.currency
                    )),
                },
            })?;

        let currency = Some(item.request.currency);

        let channel = NoonChannels::Web;

        let category = item
            .request
            .order_details
            .as_ref()
            .and_then(|details| details.first())
            .and_then(|detail| detail.category.clone());

        // The description should not have leading or trailing whitespaces, also it should not have double whitespaces and a max 50 chars according to Noon's Docs
        let name: String = item
            .resource_common_data
            .get_description()?
            .trim()
            .replace("  ", " ")
            .chars()
            .take(50)
            .collect();

        let order = NoonOrder {
            amount,
            currency,
            channel,
            category,
            reference: item.request.merchant_order_id.clone().unwrap_or_else(|| {
                item.resource_common_data
                    .connector_request_reference_id
                    .clone()
            }),
            name,
            nvp: item
                .request
                .metadata
                .as_ref()
                .map(|m| NoonOrderNvp::new(m.peek())),
            ip_address: None,
        };

        // For orderCreate, use Authorize as the payment action (payment will be completed later)
        let payment_action = NoonPaymentActions::Authorize;

        Ok(Self {
            api_operation: NoonApiOperations::Initiate,
            order,
            configuration: NoonConfiguration {
                payment_action,
                return_url: None,
                tokenize_c_c: None,
            },
        })
    }
}

// TryFrom for CreateOrder Response
impl TryFrom<ResponseRouterData<NoonCreateOrderResponse, Self>>
    for RouterDataV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NoonCreateOrderResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let order = item.response.result.order;
        let status = get_payment_status(order.status);

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_order_id: Some(order.id.to_string()),
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentCreateOrderResponse {
                merchant_order_id: item.router_data.request.merchant_order_id.clone(),
                connector_order_id: order.id.to_string(),
                session_data: None,
            }),
            ..item.router_data
        })
    }
}
