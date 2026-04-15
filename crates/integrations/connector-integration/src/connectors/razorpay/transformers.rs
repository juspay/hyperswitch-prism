use std::collections::HashMap;

use base64::{engine::general_purpose::STANDARD, Engine};
use common_enums::{self, AttemptStatus, CardNetwork};
use common_utils::{ext_traits::ByteSliceExt, pii::Email, request::Method, types::MinorUnit};
use domain_types::errors::{
    ConnectorError, IntegrationError, IntegrationErrorContext, WebhookError,
};
use domain_types::{
    connector_flow::{Authorize, Capture, CreateOrder, RSync, Refund},
    connector_types::{
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsResponseData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId,
    },
    payment_method_data::{
        BankRedirectData, Card, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber,
        WalletData,
    },
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use tracing::info;

pub const NEXT_ACTION_DATA: &str = "nextActionData";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NextActionData {
    WaitScreenInstructions,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub enum Currency {
    #[default]
    USD,
    EUR,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Amount {
    pub currency: common_enums::Currency,
    pub value: MinorUnit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CardBrand {
    Visa,
}

#[derive(Debug, PartialEq)]
pub enum RazorpayConnectorError {
    ParsingFailed,
    NotImplemented,
    FailedToObtainAuthType,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RazorpayCard<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    number: RawCardNumber<T>,
    expiry_month: Secret<String>,
    expiry_year: Secret<String>,
    cvc: Option<Secret<String>>,
    holder_name: Option<Secret<String>>,
    brand: Option<CardNetwork>,
    network_payment_reference: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "lowercase")]
pub enum RazorpayPaymentMethod<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(rename = "scheme")]
    RazorpayCard(Box<RazorpayCard<T>>),
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub enum AuthType {
    #[default]
    PreAuth,
}

#[serde_with::skip_serializing_none]
#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Address {
    city: String,
    country: common_enums::CountryAlpha2,
    house_number_or_name: Secret<String>,
    postal_code: Secret<String>,
    state_or_province: Option<Secret<String>>,
    street: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum PaymentMethod<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    RazorpayPaymentMethod(Box<RazorpayPaymentMethod<T>>),
}

#[serde_with::skip_serializing_none]
#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CardDetails<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub number: RawCardNumber<T>,
    pub name: Option<Secret<String>>,
    pub expiry_month: Option<Secret<String>>,
    pub expiry_year: Secret<String>,
    pub cvv: Option<Secret<String>>,
}

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum AuthenticationChannel {
    #[default]
    Browser,
    App,
}

#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AuthenticationDetails {
    pub authentication_channel: AuthenticationChannel,
}

#[serde_with::skip_serializing_none]
#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct BrowserInfo {
    pub java_enabled: Option<bool>,
    pub javascript_enabled: Option<bool>,
    pub timezone_offset: Option<i32>,
    pub color_depth: Option<i32>,
    pub screen_width: Option<i32>,
    pub screen_height: Option<i32>,
    pub language: Option<String>,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct RazorpayPaymentRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub amount: MinorUnit,
    pub currency: String,
    pub contact: Secret<String>,
    pub email: Email,
    pub order_id: String,
    pub method: PaymentMethodType,
    pub card: Option<RazorpayCardSpecificData<T>>,
    pub wallet: Option<RazorpayWalletType>,
    pub bank: Option<String>,
    pub authentication: Option<AuthenticationDetails>,
    pub browser: Option<BrowserInfo>,
    pub ip: Secret<String>,
    pub referer: String,
    pub user_agent: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged, rename_all = "snake_case")]
pub enum RazorpayCardSpecificData<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    Card(CardDetails<T>),
    Wallet(RazorpayWalletType),
    Netbanking(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RazorpayWalletType {
    LazyPay,
    PhonePe,
    BillDesk,
    Cashfree,
    PayU,
    EaseBuzz,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PaymentMethodType {
    Card,
    Wallet,
    Upi,
    Emi,
    Netbanking,
}

pub struct RazorpayRouterData<T> {
    pub amount: MinorUnit,
    pub router_data: T,
}

impl<T> TryFrom<(MinorUnit, T)> for RazorpayRouterData<T> {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from((amount, item): (MinorUnit, T)) -> Result<Self, Self::Error> {
        Ok(Self {
            amount,
            router_data: item,
        })
    }
}

pub enum RazorpayAuthType {
    AuthToken(Secret<String>),
    ApiKeySecret {
        api_key: Secret<String>,
        api_secret: Secret<String>,
    },
}

impl RazorpayAuthType {
    pub fn generate_authorization_header(&self) -> String {
        let auth_type_name = match self {
            Self::AuthToken(_) => "AuthToken",
            Self::ApiKeySecret { .. } => "ApiKeySecret",
        };
        info!("Type of auth Token is {}", auth_type_name);
        match self {
            Self::AuthToken(token) => format!("Bearer {}", token.peek()),
            Self::ApiKeySecret {
                api_key,
                api_secret,
            } => {
                let credentials = format!("{}:{}", api_key.peek(), api_secret.peek());
                let encoded = STANDARD.encode(credentials);
                format!("Basic {encoded}")
            }
        }
    }
}

impl TryFrom<&ConnectorSpecificConfig> for RazorpayAuthType {
    type Error = IntegrationError;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Razorpay {
                api_key,
                api_secret,
                ..
            } => match api_secret {
                None => Ok(Self::AuthToken(api_key.to_owned())),
                Some(secret) => Ok(Self::ApiKeySecret {
                    api_key: api_key.to_owned(),
                    api_secret: secret.to_owned(),
                }),
            },
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext {
                    suggested_action: Some("Pass Razorpay credentials via x-connector-config with api_key and api_secret".to_owned()),
                    doc_url: Some("https://razorpay.com/docs/api/#authentication".to_owned()),
                    additional_context: Some("Expected ConnectorSpecificConfig::Razorpay with api_key (key_id) and api_secret (key_secret)".to_owned()),
                },
            }),
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(&Card<T>, Option<Secret<String>>)> for RazorpayPaymentMethod<T>
{
    type Error = IntegrationError;
    fn try_from(
        (card, card_holder_name): (&Card<T>, Option<Secret<String>>),
    ) -> Result<Self, Self::Error> {
        let razorpay_card = RazorpayCard {
            number: card.card_number.clone(),
            expiry_month: card.card_exp_month.clone(),
            expiry_year: card.card_exp_year.clone(),
            cvc: Some(card.card_cvc.clone()),
            holder_name: card_holder_name,
            brand: card.card_network.clone(),
            network_payment_reference: None,
        };
        Ok(Self::RazorpayCard(Box::new(razorpay_card)))
    }
}

impl TryFrom<&WalletData> for RazorpayWalletType {
    type Error = IntegrationError;

    fn try_from(wallet_data: &WalletData) -> Result<Self, Self::Error> {
        match wallet_data {
            WalletData::LazyPayRedirect(_) => Ok(Self::LazyPay),
            WalletData::PhonePeRedirect(_) => Ok(Self::PhonePe),
            WalletData::BillDeskRedirect(_) => Ok(Self::BillDesk),
            WalletData::CashfreeRedirect(_) => Ok(Self::Cashfree),
            WalletData::PayURedirect(_) => Ok(Self::PayU),
            WalletData::EaseBuzzRedirect(_) => Ok(Self::EaseBuzz),
            WalletData::AliPayQr(_)
            | WalletData::AliPayRedirect(_)
            | WalletData::AliPayHkRedirect(_)
            | WalletData::BluecodeRedirect {}
            | WalletData::AmazonPayRedirect(_)
            | WalletData::MomoRedirect(_)
            | WalletData::KakaoPayRedirect(_)
            | WalletData::GoPayRedirect(_)
            | WalletData::GcashRedirect(_)
            | WalletData::ApplePay(_)
            | WalletData::ApplePayRedirect(_)
            | WalletData::ApplePayThirdPartySdk(_)
            | WalletData::DanaRedirect {}
            | WalletData::GooglePay(_)
            | WalletData::GooglePayRedirect(_)
            | WalletData::GooglePayThirdPartySdk(_)
            | WalletData::MbWayRedirect(_)
            | WalletData::MobilePayRedirect(_)
            | WalletData::PaypalRedirect(_)
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
            | WalletData::Wero(_) => Err(IntegrationError::not_implemented(format!(
                "Payment Method {wallet_data:?} not supported for Razorpay"
            ))),
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(&Card<T>, Option<Secret<String>>)> for CardDetails<T>
{
    type Error = IntegrationError;

    fn try_from(
        (card_data, card_holder_name): (&Card<T>, Option<Secret<String>>),
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            number: card_data.card_number.clone(),
            name: card_holder_name,
            expiry_month: Some(card_data.card_exp_month.clone()),
            expiry_year: card_data.card_exp_year.clone(),
            cvv: Some(card_data.card_cvc.clone()),
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        &RazorpayRouterData<
            &RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        >,
        &Card<T>,
    )> for RazorpayPaymentRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        value: (
            &RazorpayRouterData<
                &RouterDataV2<
                    Authorize,
                    PaymentFlowData,
                    PaymentsAuthorizeData<T>,
                    PaymentsResponseData,
                >,
            >,
            &Card<T>,
        ),
    ) -> Result<Self, Self::Error> {
        let (item, _card_data) = value;
        Self::try_from(item)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        &RazorpayRouterData<
            &RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        >,
    > for RazorpayPaymentRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RazorpayRouterData<
            &RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        >,
    ) -> Result<Self, Self::Error> {
        let amount = item.amount;
        let currency = item.router_data.request.currency.to_string();

        let billing = item
            .router_data
            .resource_common_data
            .address
            .get_payment_billing();

        let contact = billing
            .and_then(|billing| billing.phone.as_ref())
            .and_then(|phone| phone.number.clone())
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "contact",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Provide billing phone number in the address".to_owned(),
                    ),
                    doc_url: Some(
                        "https://razorpay.com/docs/api/payments/#create-a-payment".to_owned(),
                    ),
                    additional_context: Some(
                        "Razorpay requires a contact phone number for payment creation".to_owned(),
                    ),
                },
            })?;

        let billing_email = item
            .router_data
            .resource_common_data
            .get_billing_email()
            .ok();

        let email = billing_email
            .or(item.router_data.request.email.clone())
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "email",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Provide customer email in billing address or request".to_owned(),
                    ),
                    doc_url: Some(
                        "https://razorpay.com/docs/api/payments/#create-a-payment".to_owned(),
                    ),
                    additional_context: Some(
                        "Razorpay requires a customer email for payment creation".to_owned(),
                    ),
                },
            })?;

        let order_id = item
            .router_data
            .resource_common_data
            .reference_id
            .as_ref()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "order_id (reference_id)",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Call `PaymentService.CreateOrder` first and pass the returned order id \
                         as `merchant_order_id` (which becomes `reference_id` internally) on the \
                         Authorize request."
                            .to_owned(),
                    ),
                    doc_url: Some(
                        "https://razorpay.com/docs/api/orders/#create-an-order".to_owned(),
                    ),
                    additional_context: Some(
                        "Razorpay requires a pre-created `order_id` in the payment create \
                         request; it cannot be omitted."
                            .to_owned(),
                    ),
                },
            })?
            .clone();

        let customer_name = item.router_data.request.customer_name.clone();
        let browser_info_opt = item.router_data.request.browser_info.as_ref();

        let (method, card, wallet, bank, authentication, browser) =
            match &item.router_data.request.payment_method_data {
                PaymentMethodData::Card(card_data) => {
                    let card_details =
                        CardDetails::try_from((card_data, customer_name.map(Secret::new)))?;
                    let authentication_channel = match browser_info_opt {
                        Some(_) => AuthenticationChannel::Browser,
                        None => AuthenticationChannel::App,
                    };
                    let auth = Some(AuthenticationDetails {
                        authentication_channel,
                    });
                    let browser = browser_info_opt.map(|info| BrowserInfo {
                        java_enabled: info.java_enabled,
                        javascript_enabled: info.java_script_enabled,
                        timezone_offset: info.time_zone,
                        color_depth: info.color_depth.map(i32::from),
                        #[allow(clippy::as_conversions)]
                        screen_width: info.screen_width.map(|v| v as i32),
                        #[allow(clippy::as_conversions)]
                        screen_height: info.screen_height.map(|v| v as i32),
                        language: info.language.clone(),
                    });
                    (
                        PaymentMethodType::Card,
                        Some(RazorpayCardSpecificData::Card(card_details)),
                        None,
                        None,
                        auth,
                        browser,
                    )
                }
                PaymentMethodData::Wallet(wallet_data) => {
                    let wallet_type = RazorpayWalletType::try_from(wallet_data)?;
                    (
                        PaymentMethodType::Wallet,
                        None,
                        Some(wallet_type),
                        None,
                        None,
                        None,
                    )
                }
                PaymentMethodData::BankRedirect(BankRedirectData::Netbanking { issuer }) => (
                    PaymentMethodType::Netbanking,
                    None,
                    None,
                    Some(issuer.to_string()),
                    None,
                    None,
                ),
                pm @ (PaymentMethodData::CardRedirect(_)
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
                | PaymentMethodData::CardDetailsForNetworkTransactionId(_)
                | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
                | PaymentMethodData::NetworkToken(_)
                | PaymentMethodData::MobilePayment(_)
                | PaymentMethodData::OpenBanking(_)) => {
                    return Err(IntegrationError::not_implemented(format!(
                        "Payment Method {pm:?} not supported for Razorpay"
                    ))
                    .into())
                }
            };

        let ip = item
            .router_data
            .request
            .get_ip_address_as_optional()
            .map(|ip| Secret::new(ip.expose()))
            .unwrap_or_else(|| Secret::new("127.0.0.1".to_string()));

        let user_agent = item
            .router_data
            .request
            .browser_info
            .as_ref()
            .and_then(|info| info.get_user_agent().ok())
            .unwrap_or_else(|| "Mozilla/5.0".to_string());

        let referer = item
            .router_data
            .request
            .browser_info
            .as_ref()
            .and_then(|info| info.get_referer().ok())
            .unwrap_or_else(|| "https://example.com".to_string());

        Ok(Self {
            amount,
            currency,
            contact,
            email,
            order_id,
            method,
            card,
            wallet,
            bank,
            authentication,
            browser,
            ip,
            referer,
            user_agent,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RazorpayPaymentResponse {
    pub razorpay_payment_id: String,
    pub next: Option<Vec<NextAction>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct NextAction {
    pub action: String,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged, rename_all = "snake_case")]
pub enum RazorpayResponse {
    PaymentResponse(Box<RazorpayPaymentResponse>),
    PsyncResponse(Box<RazorpayPsyncResponse>),
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RazorpayPsyncResponse {
    pub id: String,
    pub entity: String,
    pub amount: MinorUnit,
    pub base_amount: i64,
    pub currency: String,
    pub base_currency: String,
    pub status: RazorpayStatus,
    pub method: PaymentMethodType,
    pub order_id: Option<String>,
    pub invoice_id: Option<String>,
    pub description: Option<String>,
    pub international: bool,
    pub refund_status: Option<String>,
    pub amount_refunded: i64,
    pub captured: bool,
    pub email: Email,
    pub contact: Secret<String>,
    pub fee: Option<i64>,
    pub tax: Option<i64>,
    pub error_code: Option<String>,
    pub error_description: Option<String>,
    pub error_source: Option<String>,
    pub error_step: Option<String>,
    pub error_reason: Option<String>,
    pub notes: Option<HashMap<String, String>>,
    pub created_at: i64,
    pub card_id: Option<String>,
    pub card: Option<SyncCardDetails>,
    pub upi: Option<SyncUPIDetails>,
    pub acquirer_data: Option<AcquirerData>,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RazorpayRefundResponse {
    pub id: String,
    pub status: RazorpayRefundStatus,
    pub receipt: Option<String>,
    pub amount: MinorUnit,
    pub currency: String,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RazorpayRefundRequest {
    pub amount: MinorUnit,
}

impl ForeignTryFrom<RazorpayRefundStatus> for common_enums::RefundStatus {
    type Error = IntegrationError;
    fn foreign_try_from(item: RazorpayRefundStatus) -> Result<Self, Self::Error> {
        match item {
            RazorpayRefundStatus::Failed => Ok(Self::Failure),
            RazorpayRefundStatus::Pending | RazorpayRefundStatus::Created => Ok(Self::Pending),
            RazorpayRefundStatus::Processed => Ok(Self::Success),
        }
    }
}

impl
    TryFrom<
        &RazorpayRouterData<
            &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        >,
    > for RazorpayRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: &RazorpayRouterData<
            &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            amount: item.amount,
        })
    }
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SyncCardDetails {
    pub id: String,
    pub entity: String,
    pub name: String,
    pub last4: String,
    pub network: String,
    pub r#type: String,
    pub issuer: Option<String>,
    pub emi: bool,
    pub sub_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SyncUPIDetails {
    pub payer_account_type: String,
    pub vpa: Secret<String>,
    pub flow: String,
    pub bank: String,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AcquirerData {
    pub auth_code: Option<String>,
    pub rrn: Option<Secret<String>>,
    pub authentication_reference_number: Option<Secret<String>>,
    pub bank_transaction_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RazorpayStatus {
    Created,
    Authorized,
    Captured,
    Refunded,
    Failed,
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureMethod {
    #[default]
    Automatic,
    Manual,
    ManualMultiple,
    Scheduled,
    SequentialAutomatic,
}

pub trait ForeignTryFrom<F>: Sized {
    type Error;

    fn foreign_try_from(from: F) -> Result<Self, Self::Error>;
}

fn get_authorization_razorpay_payment_status_from_action(
    is_manual_capture: bool,
    has_next_action: bool,
) -> AttemptStatus {
    if has_next_action {
        AttemptStatus::AuthenticationPending
    } else if is_manual_capture {
        AttemptStatus::Authorized
    } else {
        AttemptStatus::Charged
    }
}

fn get_psync_razorpay_payment_status(
    is_manual_capture: bool,
    razorpay_status: RazorpayStatus,
) -> AttemptStatus {
    match razorpay_status {
        RazorpayStatus::Created => AttemptStatus::Pending,
        RazorpayStatus::Authorized => {
            if is_manual_capture {
                AttemptStatus::Authorized
            } else {
                AttemptStatus::Charged
            }
        }
        RazorpayStatus::Captured => AttemptStatus::Charged,
        RazorpayStatus::Refunded => AttemptStatus::AutoRefunded,
        RazorpayStatus::Failed => AttemptStatus::Failure,
    }
}

impl ForeignTryFrom<(RazorpayRefundResponse, Self, u16)>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (response, data, http_code): (RazorpayRefundResponse, Self, u16),
    ) -> Result<Self, Self::Error> {
        let status = common_enums::RefundStatus::foreign_try_from(response.status)?;

        let refunds_response_data = RefundsResponseData {
            connector_refund_id: response.id,
            refund_status: status,
            status_code: http_code,
        };

        Ok(Self {
            resource_common_data: RefundFlowData {
                status,
                ..data.resource_common_data
            },
            response: Ok(refunds_response_data),
            ..data
        })
    }
}

impl ForeignTryFrom<(RazorpayRefundResponse, Self, u16)>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (response, data, http_code): (RazorpayRefundResponse, Self, u16),
    ) -> Result<Self, Self::Error> {
        let status = common_enums::RefundStatus::foreign_try_from(response.status)?;

        let refunds_response_data = RefundsResponseData {
            connector_refund_id: response.id,
            refund_status: status,
            status_code: http_code,
        };

        Ok(Self {
            resource_common_data: RefundFlowData {
                status,
                ..data.resource_common_data
            },
            response: Ok(refunds_response_data),
            ..data
        })
    }
}

impl<F, Req>
    ForeignTryFrom<(
        RazorpayResponse,
        Self,
        u16,
        Option<common_enums::CaptureMethod>,
        bool,
        Option<common_enums::PaymentMethodType>,
    )> for RouterDataV2<F, PaymentFlowData, Req, PaymentsResponseData>
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (response, data, _http_code, _capture_method, _is_multiple_capture_psync_flow, _pmt): (
            RazorpayResponse,
            Self,
            u16,
            Option<common_enums::CaptureMethod>,
            bool,
            Option<common_enums::PaymentMethodType>,
        ),
    ) -> Result<Self, Self::Error> {
        let is_manual_capture = false;

        match response {
            RazorpayResponse::PaymentResponse(payment_response) => {
                let status =
                    get_authorization_razorpay_payment_status_from_action(is_manual_capture, true);
                let redirect_url = payment_response
                    .next
                    .as_ref()
                    .and_then(|next_actions| next_actions.first())
                    .map(|action| action.url.clone())
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "next.url",
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "3DS redirect URL missing in Razorpay authorize response"
                                    .to_owned(),
                            ),
                            ..Default::default()
                        },
                    })?;

                let form_fields = HashMap::new();

                let payment_response_data = PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        payment_response.razorpay_payment_id.clone(),
                    ),
                    redirection_data: Some(Box::new(RedirectForm::Form {
                        endpoint: redirect_url,
                        method: Method::Get,
                        form_fields,
                    })),
                    connector_metadata: None,
                    network_txn_id: None,
                    connector_response_reference_id: data.resource_common_data.reference_id.clone(),
                    incremental_authorization_allowed: None,
                    mandate_reference: None,
                    status_code: _http_code,
                };
                let error = None;

                Ok(Self {
                    response: error.map_or_else(|| Ok(payment_response_data), Err),
                    resource_common_data: PaymentFlowData {
                        status,
                        ..data.resource_common_data
                    },
                    ..data
                })
            }
            RazorpayResponse::PsyncResponse(psync_response) => {
                let status =
                    get_psync_razorpay_payment_status(is_manual_capture, psync_response.status);

                // Extract UPI mode and set in connector_response
                let connector_response = psync_response
                    .upi
                    .as_ref()
                    .filter(|upi| upi.payer_account_type == "credit_card")
                    .map(|_| {
                        domain_types::router_data::ConnectorResponseData::
                            with_additional_payment_method_data(
                                domain_types::router_data::AdditionalPaymentMethodConnectorResponse::Upi {
                                    upi_mode: Some(domain_types::payment_method_data::UpiSource::UpiCc)
},
                            )
                    });

                let psync_response_data = PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(psync_response.id),
                    redirection_data: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    connector_response_reference_id: data.resource_common_data.reference_id.clone(),
                    incremental_authorization_allowed: None,
                    mandate_reference: None,
                    status_code: _http_code,
                };
                let error = None;

                Ok(Self {
                    response: error.map_or_else(|| Ok(psync_response_data), Err),
                    resource_common_data: PaymentFlowData {
                        status,
                        connector_response,
                        ..data.resource_common_data
                    },
                    ..data
                })
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RazorpayErrorResponse {
    StandardError { error: RazorpayError },
    SimpleError { message: String },
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RazorpayError {
    pub code: String,
    pub description: String,
    pub source: Option<String>,
    pub step: Option<String>,
    pub reason: Option<String>,
    pub metadata: Option<Metadata>,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Metadata {
    pub order_id: Option<String>,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct RazorpayOrderRequest {
    pub amount: MinorUnit,
    pub currency: String,
    pub receipt: String,
    pub partial_payment: Option<bool>,
    pub first_payment_min_amount: Option<MinorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_capture: Option<i8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discount: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offer_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_id: Option<String>,
    #[serde(rename = "token[expire_at]", skip_serializing_if = "Option::is_none")]
    pub __token_91_expire_at_93_: Option<i64>,
    #[serde(rename = "token[max_amount]", skip_serializing_if = "Option::is_none")]
    pub __token_91_max_amount_93_: Option<i64>,
    #[serde(rename = "token[auth_type]", skip_serializing_if = "Option::is_none")]
    pub __token_91_auth_type_93_: Option<String>,
    #[serde(rename = "token[frequency]", skip_serializing_if = "Option::is_none")]
    pub __token_91_frequency_93_: Option<String>,
    #[serde(rename = "bank_account[name]", skip_serializing_if = "Option::is_none")]
    pub __bank_account_91_name_93_: Option<String>,
    #[serde(
        rename = "bank_account[account_number]",
        skip_serializing_if = "Option::is_none"
    )]
    pub __bank_account_91_account_number_93_: Option<String>,
    #[serde(rename = "bank_account[ifsc]", skip_serializing_if = "Option::is_none")]
    pub __bank_account_91_ifsc_93_: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phonepe_switch_context: Option<String>,
    #[serde(rename = "notes[crm1]", skip_serializing_if = "Option::is_none")]
    pub __notes_91_crm1_93_: Option<String>,
    #[serde(rename = "notes[crm2]", skip_serializing_if = "Option::is_none")]
    pub __notes_91_crm2_93_: Option<String>,
}

impl
    TryFrom<
        &RazorpayRouterData<
            &RouterDataV2<
                CreateOrder,
                PaymentFlowData,
                PaymentCreateOrderData,
                PaymentCreateOrderResponse,
            >,
        >,
    > for RazorpayOrderRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RazorpayRouterData<
            &RouterDataV2<
                CreateOrder,
                PaymentFlowData,
                PaymentCreateOrderData,
                PaymentCreateOrderResponse,
            >,
        >,
    ) -> Result<Self, Self::Error> {
        let request_data = &item.router_data.request;

        let converted_amount = item.amount;
        // Extract metadata as a HashMap
        let metadata_map = item
            .router_data
            .request
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.peek().as_object())
            .map(|obj| {
                obj.iter()
                    .map(|(k, v)| (k.clone(), json_value_to_string(v)))
                    .collect::<HashMap<String, String>>()
            })
            .unwrap_or_default();

        Ok(Self {
            amount: converted_amount,
            currency: request_data.currency.to_string(),
            receipt: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            partial_payment: None,
            first_payment_min_amount: None,
            payment_capture: Some(1),
            method: metadata_map.get("method").cloned(),
            discount: metadata_map
                .get("discount")
                .and_then(|v| v.parse::<i64>().ok()),
            offer_id: metadata_map.get("offer_id").cloned(),
            customer_id: metadata_map.get("customer_id").cloned(),
            __token_91_expire_at_93_: metadata_map
                .get("__token_91_expire_at_93_")
                .and_then(|v| v.parse::<i64>().ok()),
            __token_91_max_amount_93_: metadata_map
                .get("__token_91_max_amount_93_")
                .and_then(|v| v.parse::<i64>().ok()),
            __token_91_auth_type_93_: metadata_map.get("__token_91_auth_type_93_").cloned(),
            __token_91_frequency_93_: metadata_map.get("__token_91_frequency_93_").cloned(),
            __bank_account_91_name_93_: metadata_map.get("__bank_account_91_name_93_").cloned(),
            __bank_account_91_account_number_93_: metadata_map
                .get("__bank_account_91_account_number_93_")
                .cloned(),
            __bank_account_91_ifsc_93_: metadata_map
                .get("__bank_account_91_ifsc_93_")
                .cloned()
                .map(Secret::new),
            account_id: metadata_map.get("account_id").cloned(),
            phonepe_switch_context: metadata_map.get("phonepe_switch_context").cloned(),
            __notes_91_crm1_93_: metadata_map.get("__notes_91_crm1_93_").cloned(),
            __notes_91_crm2_93_: metadata_map.get("__notes_91_crm2_93_").cloned(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum RazorpayNotes {
    Map(HashMap<String, String>),
    EmptyVec(Vec<()>),
}
#[serde_with::skip_serializing_none]
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct RazorpayOrderResponse {
    pub id: String,
    pub entity: String,
    pub amount: MinorUnit,
    pub amount_paid: MinorUnit,
    pub amount_due: MinorUnit,
    pub currency: String,
    pub receipt: String,
    pub status: String,
    pub attempts: u32,
    pub notes: Option<RazorpayNotes>,
    pub offer_id: Option<String>,
    pub created_at: u64,
}

impl ForeignTryFrom<(RazorpayOrderResponse, Self, u16, bool)>
    for RouterDataV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    >
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (response, data, _status_code, _): (RazorpayOrderResponse, Self, u16, bool),
    ) -> Result<Self, Self::Error> {
        let order_response = PaymentCreateOrderResponse {
            connector_order_id: response.id.clone(),
            session_data: None,
        };

        Ok(Self {
            response: Ok(order_response),
            resource_common_data: PaymentFlowData {
                connector_order_id: Some(response.id),
                ..data.resource_common_data
            },
            ..data
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct RazorpayWebhook {
    pub account_id: String,
    pub contains: Vec<String>,
    pub entity: String,
    pub event: String,
    pub payload: Payload,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct Payload {
    pub payment: Option<PaymentWrapper>,
    pub refund: Option<RefundWrapper>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct PaymentWrapper {
    pub entity: PaymentEntity,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct RefundWrapper {
    pub entity: RefundEntity,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct PaymentEntity {
    pub id: String,
    pub entity: RazorpayEntity,
    pub amount: i64,
    pub currency: String,
    pub status: RazorpayPaymentStatus,
    pub order_id: String,
    pub invoice_id: Option<String>,
    pub international: bool,
    pub method: String,
    pub amount_refunded: i64,
    pub refund_status: Option<String>,
    pub captured: bool,
    pub description: Option<String>,
    pub card_id: Option<String>,
    pub bank: Option<String>,
    pub wallet: Option<String>,
    pub vpa: Option<Secret<String>>,
    pub email: Option<Email>,
    pub contact: Option<Secret<String>>,
    pub notes: Vec<String>,
    pub fee: Option<i64>,
    pub tax: Option<i64>,
    pub error_code: Option<String>,
    pub error_description: Option<String>,
    pub error_reason: Option<String>,
    pub error_source: Option<String>,
    pub error_step: Option<String>,
    pub acquirer_data: Option<AcquirerData>,
    pub card: Option<RazorpayWebhookCard>,
    pub token_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct RefundEntity {
    pub id: String,
    pub entity: RazorpayEntity,
    pub amount: i64,
    pub currency: String,
    pub payment_id: String,
    pub status: RazorpayRefundStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RazorpayEntity {
    Payment,
    Refund,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RazorpayPaymentStatus {
    Authorized,
    Captured,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RazorpayRefundStatus {
    Created,
    Processed,
    Failed,
    Pending,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct RazorpayWebhookCard {
    pub id: String,
    pub entity: String,
    pub name: String,
    pub last4: String,
    pub network: String,
    #[serde(rename = "type")]
    pub card_type: String,
    pub sub_type: String,
    pub issuer: Option<String>,
    pub international: bool,
    pub iin: String,
    pub emi: bool,
}

pub fn get_webhook_object_from_body(
    body: Vec<u8>,
) -> Result<Payload, error_stack::Report<WebhookError>> {
    let webhook: RazorpayWebhook = body
        .parse_struct("RazorpayWebhook")
        .change_context(WebhookError::WebhookBodyDecodingFailed)?;
    Ok(webhook.payload)
}

pub(crate) fn get_razorpay_payment_webhook_status(
    entity: RazorpayEntity,
    status: RazorpayPaymentStatus,
) -> Result<AttemptStatus, WebhookError> {
    match entity {
        RazorpayEntity::Payment => match status {
            RazorpayPaymentStatus::Authorized => Ok(AttemptStatus::Authorized),
            RazorpayPaymentStatus::Captured => Ok(AttemptStatus::Charged),
            RazorpayPaymentStatus::Failed => Ok(AttemptStatus::AuthorizationFailed),
        },
        RazorpayEntity::Refund => Err(WebhookError::WebhookProcessingFailed),
    }
}

pub(crate) fn get_razorpay_refund_webhook_status(
    entity: RazorpayEntity,
    status: RazorpayRefundStatus,
) -> Result<common_enums::RefundStatus, WebhookError> {
    match entity {
        RazorpayEntity::Refund => match status {
            RazorpayRefundStatus::Processed => Ok(common_enums::RefundStatus::Success),
            RazorpayRefundStatus::Created | RazorpayRefundStatus::Pending => {
                Ok(common_enums::RefundStatus::Pending)
            }
            RazorpayRefundStatus::Failed => Ok(common_enums::RefundStatus::Failure),
        },
        RazorpayEntity::Payment => Err(WebhookError::WebhookProcessingFailed),
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RazorpayCaptureRequest {
    pub amount: MinorUnit,
    pub currency: String,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RazorpayCaptureResponse {
    pub id: String,
    pub entity: RazorpayEntity,
    pub amount: i64,
    pub currency: String,
    pub status: RazorpayPaymentStatus,
    pub order_id: String,
    pub invoice_id: Option<String>,
    pub international: bool,
    pub method: String,
    pub amount_refunded: i64,
    pub refund_status: Option<String>,
    pub captured: bool,
    pub description: Option<String>,
    pub card_id: Option<String>,
    pub bank: Option<String>,
    pub wallet: Option<String>,
    pub vpa: Option<Secret<String>>,
    pub email: Option<Email>,
    pub contact: Option<Secret<String>>,
    pub customer_id: Option<String>,
    pub token_id: Option<String>,
    pub notes: Vec<String>,
    pub fee: Option<i64>,
    pub tax: Option<i64>,
    pub error_code: Option<String>,
    pub error_description: Option<String>,
    pub error_reason: Option<String>,
    pub error_source: Option<String>,
    pub error_step: Option<String>,
    pub acquirer_data: Option<AcquirerData>,
}

impl
    TryFrom<
        &RazorpayRouterData<
            &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        >,
    > for RazorpayCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RazorpayRouterData<
            &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        let request_data = &item.router_data.request;

        Ok(Self {
            amount: item.amount,
            currency: request_data.currency.to_string(),
        })
    }
}

impl<F, Req> ForeignTryFrom<(RazorpayCaptureResponse, Self, u16)>
    for RouterDataV2<F, PaymentFlowData, Req, PaymentsResponseData>
{
    type Error = IntegrationError;
    fn foreign_try_from(
        (response, data, http_code): (RazorpayCaptureResponse, Self, u16),
    ) -> Result<Self, Self::Error> {
        let status = match response.status {
            RazorpayPaymentStatus::Captured => AttemptStatus::Charged,
            RazorpayPaymentStatus::Authorized => AttemptStatus::Authorized,
            RazorpayPaymentStatus::Failed => AttemptStatus::Failure,
        };
        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.id),
                redirection_data: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(response.order_id),
                incremental_authorization_allowed: None,
                mandate_reference: None,
                status_code: http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..data.resource_common_data
            },
            ..data
        })
    }
}

// ============ UPI Web Collect Request ============

#[derive(Debug, Serialize)]
pub struct RazorpayWebCollectRequest {
    pub currency: String,
    pub amount: MinorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<Email>,
    pub order_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact: Option<Secret<String>>,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vpa: Option<Secret<String>>,
    #[serde(rename = "notes[txn_uuid]", skip_serializing_if = "Option::is_none")]
    pub __notes_91_txn_uuid_93_: Option<String>,
    #[serde(
        rename = "notes[transaction_id]",
        skip_serializing_if = "Option::is_none"
    )]
    pub __notes_91_transaction_id_93_: Option<String>,
    pub callback_url: String,
    pub ip: Secret<String>,
    pub referer: String,
    pub user_agent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow: Option<String>,
    #[serde(rename = "notes[cust_id]", skip_serializing_if = "Option::is_none")]
    pub __notes_91_cust_id_93_: Option<String>,
    #[serde(rename = "notes[cust_name]", skip_serializing_if = "Option::is_none")]
    pub __notes_91_cust_name_93_: Option<String>,
    #[serde(rename = "upi[flow]", skip_serializing_if = "Option::is_none")]
    pub __upi_91_flow_93_: Option<String>,
    #[serde(rename = "upi[type]", skip_serializing_if = "Option::is_none")]
    pub __upi_91_type_93_: Option<String>,
    #[serde(rename = "upi[end_date]", skip_serializing_if = "Option::is_none")]
    pub __upi_91_end_date_93_: Option<i64>,
    #[serde(rename = "upi[vpa]", skip_serializing_if = "Option::is_none")]
    pub __upi_91_vpa_93_: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurring: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_id: Option<String>,
    #[serde(rename = "upi[expiry_time]", skip_serializing_if = "Option::is_none")]
    pub __upi_91_expiry_time_93_: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee: Option<i64>,
    #[serde(rename = "notes[BookingID]", skip_serializing_if = "Option::is_none")]
    pub __notes_91_booking_id_93: Option<String>,
    #[serde(rename = "notes[PNR]", skip_serializing_if = "Option::is_none")]
    pub __notes_91_pnr_93: Option<String>,
    #[serde(rename = "notes[PaymentID]", skip_serializing_if = "Option::is_none")]
    pub __notes_91_payment_id_93: Option<String>,
    #[serde(rename = "notes[lob]", skip_serializing_if = "Option::is_none")]
    pub __notes_91_lob_93_: Option<String>,
    #[serde(
        rename = "notes[credit_line_id]",
        skip_serializing_if = "Option::is_none"
    )]
    pub __notes_91_credit_line_id_93_: Option<String>,
    #[serde(rename = "notes[loan_id]", skip_serializing_if = "Option::is_none")]
    pub __notes_91_loan_id_93_: Option<String>,
    #[serde(
        rename = "notes[transaction_type]",
        skip_serializing_if = "Option::is_none"
    )]
    pub __notes_91_transaction_type_93_: Option<String>,
    #[serde(
        rename = "notes[loan_product_code]",
        skip_serializing_if = "Option::is_none"
    )]
    pub __notes_91_loan_product_code_93_: Option<String>,
    #[serde(rename = "notes[pg_flow]", skip_serializing_if = "Option::is_none")]
    pub __notes_91_pg_flow_93_: Option<String>,
    #[serde(rename = "notes[TID]", skip_serializing_if = "Option::is_none")]
    pub __notes_91_tid_93: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        &RazorpayRouterData<
            &RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        >,
    > for RazorpayWebCollectRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RazorpayRouterData<
            &RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        >,
    ) -> Result<Self, Self::Error> {
        use domain_types::payment_method_data::{PaymentMethodData, UpiData};
        use hyperswitch_masking::PeekInterface;

        // Determine flow type and extract VPA based on UPI payment method
        let (flow_type, vpa) = match &item.router_data.request.payment_method_data {
            PaymentMethodData::Upi(UpiData::UpiCollect(collect_data)) => {
                let vpa = collect_data
                    .vpa_id
                    .as_ref()
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "vpa_id",
                        context: IntegrationErrorContext {
                            suggested_action: Some("Provide a valid VPA in upi_collect.vpa_id (e.g. user@upi)".to_owned()),
                            doc_url: Some("https://razorpay.com/docs/api/payments/upi/#create-a-upi-payment".to_owned()),
                            additional_context: Some("UPI collect requires a non-empty VPA to initiate a collect request".to_owned()),
                        },
                    })?
                    .peek()
                    .to_string();
                (None, Some(vpa))
            }
            PaymentMethodData::Upi(UpiData::UpiIntent(_))
            | PaymentMethodData::Upi(UpiData::UpiQr(_)) => (Some("intent"), None),
            _ => (None, None), // Default fallback
        };

        // Get order_id from the CreateOrder response (stored in reference_id)
        let order_id = item
            .router_data
            .resource_common_data
            .reference_id
            .as_ref()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "order_id (reference_id)",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Call `PaymentService.CreateOrder` first and pass the returned order id \
                         as `merchant_order_id` (which becomes `reference_id` internally) on the \
                         Authorize request."
                            .to_owned(),
                    ),
                    doc_url: Some(
                        "https://razorpay.com/docs/api/orders/#create-an-order".to_owned(),
                    ),
                    additional_context: Some(
                        "Razorpay requires a pre-created `order_id` in the payment create \
                         request; it cannot be omitted."
                            .to_owned(),
                    ),
                },
            })?
            .clone();

        // Extract metadata as a HashMap
        let metadata_map = item
            .router_data
            .request
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.peek().as_object())
            .map(|obj| {
                obj.iter()
                    .map(|(k, v)| (k.clone(), json_value_to_string(v)))
                    .collect::<HashMap<String, String>>()
            })
            .unwrap_or_default();

        Ok(Self {
            currency: item.router_data.request.currency.to_string(),
            amount: item.amount,
            email: item
                .router_data
                .resource_common_data
                .get_billing_email()
                .ok(),
            order_id: order_id.to_string(),
            contact: item
                .router_data
                .resource_common_data
                .get_billing_phone_number()
                .ok(),
            method: match &item.router_data.request.payment_method_data {
                PaymentMethodData::Upi(_) => "upi".to_string(),
                PaymentMethodData::Card(_) => "card".to_string(),
                _ => "card".to_string(), // Default to card
            },
            vpa: vpa.clone().map(Secret::new),
            __notes_91_txn_uuid_93_: metadata_map.get("__notes_91_txn_uuid_93_").cloned(),
            __notes_91_transaction_id_93_: metadata_map
                .get("__notes_91_transaction_id_93_")
                .cloned(),
            callback_url: item.router_data.request.get_router_return_url()?,

            ip: item
                .router_data
                .request
                .get_ip_address_as_optional()
                .map(|ip| Secret::new(ip.expose()))
                .unwrap_or_else(|| Secret::new("127.0.0.1".to_string())),
            referer: item
                .router_data
                .request
                .browser_info
                .as_ref()
                .and_then(|info| info.get_referer().ok())
                .unwrap_or_else(|| "https://example.com".to_string()),
            user_agent: item
                .router_data
                .request
                .browser_info
                .as_ref()
                .and_then(|info| info.get_user_agent().ok())
                .unwrap_or_else(|| "Mozilla/5.0".to_string()),
            description: Some("".to_string()),
            flow: flow_type.map(|s| s.to_string()),
            __notes_91_cust_id_93_: metadata_map.get("__notes_91_cust_id_93_").cloned(),
            __notes_91_cust_name_93_: metadata_map.get("__notes_91_cust_name_93_").cloned(),
            __upi_91_flow_93_: metadata_map.get("__upi_91_flow_93_").cloned(),
            __upi_91_type_93_: metadata_map.get("__upi_91_type_93_").cloned(),
            __upi_91_end_date_93_: metadata_map
                .get("__upi_91_end_date_93_")
                .and_then(|v| v.parse::<i64>().ok()),
            __upi_91_vpa_93_: metadata_map.get("__upi_91_vpa_93_").cloned(),
            recurring: None,
            customer_id: None,
            __upi_91_expiry_time_93_: metadata_map
                .get("__upi_91_expiry_time_93_")
                .and_then(|v| v.parse::<i64>().ok()),
            fee: None,
            __notes_91_booking_id_93: metadata_map.get("__notes_91_booking_id_93").cloned(),
            __notes_91_pnr_93: metadata_map.get("__notes_91_pnr_93").cloned(),
            __notes_91_payment_id_93: metadata_map.get("__notes_91_payment_id_93").cloned(),
            __notes_91_lob_93_: metadata_map.get("__notes_91_lob_93_").cloned(),
            __notes_91_credit_line_id_93_: metadata_map
                .get("__notes_91_credit_line_id_93_")
                .cloned(),
            __notes_91_loan_id_93_: metadata_map.get("__notes_91_loan_id_93_").cloned(),
            __notes_91_transaction_type_93_: metadata_map
                .get("__notes_91_transaction_type_93_")
                .cloned(),
            __notes_91_loan_product_code_93_: metadata_map
                .get("__notes_91_loan_product_code_93_")
                .cloned(),
            __notes_91_pg_flow_93_: metadata_map.get("__notes_91_pg_flow_93_").cloned(),
            __notes_91_tid_93: metadata_map.get("__notes_91_tid_93").cloned(),
            account_id: None,
        })
    }
}

// ============ Netbanking Request ============

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum RazorpayBankCode {
    Sbin,
    Hdfc,
    Icic,
    Utib,
    Kkbk,
    Punb,
    Barb,
    Ubin,
    Cnrb,
    Indb,
    Yesb,
    Ibkl,
    Fdrl,
    Ioba,
    Cbin,
}

fn map_bank_name_to_razorpay_code(
    bank: &common_enums::BankNames,
) -> Result<RazorpayBankCode, error_stack::Report<IntegrationError>> {
    match bank {
        common_enums::BankNames::StateBank => Ok(RazorpayBankCode::Sbin),
        common_enums::BankNames::HdfcBank => Ok(RazorpayBankCode::Hdfc),
        common_enums::BankNames::IciciBank => Ok(RazorpayBankCode::Icic),
        common_enums::BankNames::AxisBank => Ok(RazorpayBankCode::Utib),
        common_enums::BankNames::KotakMahindraBank => Ok(RazorpayBankCode::Kkbk),
        common_enums::BankNames::PunjabNationalBank => Ok(RazorpayBankCode::Punb),
        common_enums::BankNames::BankOfBaroda => Ok(RazorpayBankCode::Barb),
        common_enums::BankNames::UnionBankOfIndia => Ok(RazorpayBankCode::Ubin),
        common_enums::BankNames::CanaraBank => Ok(RazorpayBankCode::Cnrb),
        common_enums::BankNames::IndusIndBank => Ok(RazorpayBankCode::Indb),
        common_enums::BankNames::YesBank => Ok(RazorpayBankCode::Yesb),
        common_enums::BankNames::IdbiBank => Ok(RazorpayBankCode::Ibkl),
        common_enums::BankNames::FederalBank => Ok(RazorpayBankCode::Fdrl),
        common_enums::BankNames::IndianOverseasBank => Ok(RazorpayBankCode::Ioba),
        common_enums::BankNames::CentralBankOfIndia => Ok(RazorpayBankCode::Cbin),
        _ => Err(IntegrationError::NotSupported {
            message: format!("Bank {:?} is not supported for Razorpay netbanking", bank),
            connector: "razorpay",
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Use one of the supported Indian banks listed in Razorpay's netbanking \
                     documentation. See `RazorpayBankCode` enum for the full supported list."
                        .to_owned(),
                ),
                doc_url: Some(
                    "https://razorpay.com/docs/payments/payment-methods/netbanking/#supported-banks"
                        .to_owned(),
                ),
                additional_context: Some(format!(
                    "Razorpay netbanking accepts a fixed set of Indian bank codes; '{:?}' is \
                     outside that set.",
                    bank
                )),
            },
        }
        .into()),
    }
}

#[derive(Debug, Serialize)]
pub struct RazorpayNetbankingRequest {
    pub amount: MinorUnit,
    pub currency: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<Email>,
    pub order_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact: Option<Secret<String>>,
    pub method: PaymentMethodType,
    pub bank: RazorpayBankCode,
    pub callback_url: String,
    pub ip: Secret<String>,
    pub referer: String,
    pub user_agent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<HashMap<String, String>>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        &RazorpayRouterData<
            &RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        >,
    > for RazorpayNetbankingRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RazorpayRouterData<
            &RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        >,
    ) -> Result<Self, Self::Error> {
        let bank_code = match &item.router_data.request.payment_method_data {
            PaymentMethodData::BankRedirect(
                BankRedirectData::Netbanking { issuer },
            ) => map_bank_name_to_razorpay_code(issuer)?,
            _ => {
                return Err(IntegrationError::MissingRequiredField {
                    field_name: "netbanking payment_method_data",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Populate `PaymentMethodData::BankRedirect(Netbanking { issuer })` \
                             with a supported Razorpay bank."
                                .to_owned(),
                        ),
                        doc_url: Some(
                            "https://razorpay.com/docs/api/payments/netbanking/#create-a-netbanking-payment"
                                .to_owned(),
                        ),
                        additional_context: Some(
                            "Razorpay netbanking requires a bank issuer to route the customer to \
                             the correct bank login page."
                                .to_owned(),
                        ),
                    },
                }
                .into())
            }
        };

        let order_id = item
            .router_data
            .resource_common_data
            .reference_id
            .as_ref()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "order_id (reference_id)",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Call `PaymentService.CreateOrder` first and pass the returned order id \
                         as `merchant_order_id` (which becomes `reference_id` internally) on the \
                         Authorize request."
                            .to_owned(),
                    ),
                    doc_url: Some(
                        "https://razorpay.com/docs/api/orders/#create-an-order".to_owned(),
                    ),
                    additional_context: Some(
                        "Razorpay requires a pre-created `order_id` in the payment create \
                         request; it cannot be omitted."
                            .to_owned(),
                    ),
                },
            })?
            .clone();

        let metadata_map = item
            .router_data
            .request
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.peek().as_object())
            .map(|obj| {
                obj.iter()
                    .map(|(k, v)| (k.clone(), json_value_to_string(v)))
                    .collect::<HashMap<String, String>>()
            });

        Ok(Self {
            currency: item.router_data.request.currency.to_string(),
            amount: item.amount,
            email: item
                .router_data
                .resource_common_data
                .get_billing_email()
                .ok(),
            order_id: order_id.to_string(),
            contact: item
                .router_data
                .resource_common_data
                .get_billing_phone_number()
                .ok(),
            method: PaymentMethodType::Netbanking,
            bank: bank_code,
            callback_url: item.router_data.request.get_router_return_url()?,
            ip: item
                .router_data
                .request
                .get_ip_address_as_optional()
                .map(|ip| Secret::new(ip.expose()))
                .unwrap_or_else(|| Secret::new("127.0.0.1".to_string())),
            referer: item
                .router_data
                .request
                .browser_info
                .as_ref()
                .and_then(|info| info.get_referer().ok())
                .unwrap_or_else(|| "https://example.com".to_string()),
            user_agent: item
                .router_data
                .request
                .browser_info
                .as_ref()
                .and_then(|info| info.get_user_agent().ok())
                .unwrap_or_else(|| "Mozilla/5.0".to_string()),
            description: None,
            notes: metadata_map,
        })
    }
}

// ============ UPI Response Types ============

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RazorpayUpiPaymentsResponse {
    SuccessIntent {
        razorpay_payment_id: String,
        link: String,
    },
    SuccessCollect {
        razorpay_payment_id: String,
    },
    NullResponse {
        razorpay_payment_id: Option<String>,
    },
    Error {
        error: RazorpayErrorResponse,
    },
}

// Wrapper type for UPI response transformations
#[derive(Debug)]
pub struct RazorpayUpiResponseData {
    pub transaction_id: ResponseId,
    pub redirection_data: Option<RedirectForm>,
}

impl<F, Req>
    ForeignTryFrom<(
        RazorpayUpiPaymentsResponse,
        Self,
        u16,
        Vec<u8>, // raw_response
    )> for RouterDataV2<F, PaymentFlowData, Req, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn foreign_try_from(
        (upi_response, data, _status_code, _raw_response): (
            RazorpayUpiPaymentsResponse,
            Self,
            u16,
            Vec<u8>,
        ),
    ) -> Result<Self, Self::Error> {
        let (transaction_id, redirection_data) = match upi_response {
            RazorpayUpiPaymentsResponse::SuccessIntent {
                razorpay_payment_id,
                link,
            } => {
                let redirect_form = RedirectForm::Uri { uri: link };
                (
                    ResponseId::ConnectorTransactionId(razorpay_payment_id),
                    Some(redirect_form),
                )
            }
            RazorpayUpiPaymentsResponse::SuccessCollect {
                razorpay_payment_id,
            } => {
                // For UPI Collect, there's no link, so no redirection data
                (
                    ResponseId::ConnectorTransactionId(razorpay_payment_id),
                    None,
                )
            }
            RazorpayUpiPaymentsResponse::NullResponse {
                razorpay_payment_id,
            } => {
                // Handle null response - likely an error condition
                match razorpay_payment_id {
                    Some(payment_id) => (ResponseId::ConnectorTransactionId(payment_id), None),
                    None => {
                        // Payment ID is null, this is likely an error
                        return Err(error_stack::report!(
                            crate::utils::response_handling_fail_for_connector(
                                _status_code,
                                "razorpay"
                            )
                        ));
                    }
                }
            }
            RazorpayUpiPaymentsResponse::Error { error: _ } => {
                // Handle error case - this should probably return an error instead
                return Err(error_stack::report!(
                    crate::utils::response_handling_fail_for_connector(_status_code, "razorpay")
                ));
            }
        };

        let connector_metadata = get_wait_screen_metadata();

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: transaction_id,
            redirection_data: redirection_data.map(Box::new),
            connector_metadata,
            mandate_reference: None,
            network_txn_id: None,
            connector_response_reference_id: data.resource_common_data.reference_id.clone(),
            incremental_authorization_allowed: None,
            status_code: _status_code,
        };

        Ok(Self {
            response: Ok(payments_response_data),
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::AuthenticationPending,
                ..data.resource_common_data
            },
            ..data
        })
    }
}

pub fn get_wait_screen_metadata() -> Option<serde_json::Value> {
    serde_json::to_value(serde_json::json!({
        NEXT_ACTION_DATA: NextActionData::WaitScreenInstructions
    }))
    .map_err(|e| {
        tracing::error!("Failed to serialize wait screen metadata: {}", e);
        e
    })
    .ok()
}

pub fn json_value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        _ => value.to_string(), // For Number, Bool, Null, Object, Array - serialize as JSON
    }
}
