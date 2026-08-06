use crate::types::ResponseRouterData;
use common_enums::{AttemptStatus, BankNames, CardNetwork, Currency, RefundStatus};
use common_utils::{
    request::Method,
    types::{FloatMajorUnit, StringMajorUnit},
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, CreateOrder, PSync, RSync, RefreshPaymentMethod, Refund, Void,
    },
    connector_types::{
        CardRefreshOutcome, CardRefreshResult, PaymentCreateOrderData, PaymentCreateOrderResponse,
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefreshPaymentMethodData,
        RefreshPaymentMethodFlowData, RefreshPaymentMethodResponseData, RefreshPaymentMethodResult,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, ResponseId,
    },
    errors,
    payment_method_data::{
        BankRedirectData, CardWithNoCvc, PayLaterData, PaymentMethodData, PaymentMethodDataTypes,
        RealTimePaymentData, UpiData, WalletData,
    },
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, str::FromStr};

use crate::connectors::juspay::JuspayAmountConvertor;

#[derive(Debug, Clone)]
pub struct JuspayAuthType {
    pub api_key: Secret<String>,
    pub merchant_id: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for JuspayAuthType {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Juspay {
                api_key,
                merchant_id,
                ..
            } => Ok(Self {
                api_key: api_key.to_owned(),
                merchant_id: merchant_id.to_owned(),
            }),
            _ => Err(error_stack::report!(
                errors::IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext::default()
                }
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JuspayErrorResponse {
    pub status: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub user_message: Option<String>,
    pub error_info: Option<JuspayErrorInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JuspayErrorInfo {
    pub code: Option<String>,
    pub user_message: Option<String>,
    pub developer_message: Option<String>,
    pub fields: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JuspayOrderStatus {
    Created,
    New,
    Started,
    JuspayDeclined,
    PendingVbv,
    VbvSuccessful,
    Authorized,
    AuthenticationFailed,
    AuthorizationFailed,
    Authorizing,
    Charged,
    CodInitiated,
    Voided,
    VoidInitiated,
    VoidFailed,
    CaptureInitiated,
    CaptureFailed,
    AutoRefunded,
    NotFound,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum JuspayPaymentMethodType {
    #[serde(rename = "CARD")]
    Card,
    #[serde(rename = "UPI")]
    Upi,
    #[serde(rename = "WALLET")]
    Wallet,
    #[serde(rename = "NB")]
    Nb,
    #[serde(rename = "RTP")]
    Rtp,
    #[serde(rename = "CONSUMER_FINANCE")]
    ConsumerFinance,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JuspayPaymentMethod {
    Visa,
    Mastercard,
    Amex,
    Jcb,
    Diners,
    Discover,
    Unionpay,
    Rupay,
    Maestro,
    UpiCollect,
    UpiPay,
    PhonepeWallet,
    Lazypay,
    Amazonpay,
    Paypal,
    Alipay,
    AlipayHk,
    Applepay,
    Googlepay,
    Momo,
    Kakaopay,
    Wechatpay,
    Gopay,
    Gcash,
    Touchngo,
    Samsungpay,
    JpHdfc,
    JpIcici,
    JpAxis,
    JpSbi,
    JpKotak,
    JpIndb,
    JpIdbi,
    JpUbi,
    JpPnb,
    JpCanr,
    JpFed,
    JpYesb,
    JpIob,
    JpCbi,
    JpScb,
    DuitnowQr,
    FpsQr,
    PromptpayQr,
    Vietqr,
    AtomePaylater,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Juspay3dsAuthType {
    ThreeDs,
    NoThreeDs,
}

impl From<JuspayOrderStatus> for AttemptStatus {
    fn from(status: JuspayOrderStatus) -> Self {
        match status {
            JuspayOrderStatus::New | JuspayOrderStatus::Created => Self::Started,
            JuspayOrderStatus::Started
            | JuspayOrderStatus::Authorizing
            | JuspayOrderStatus::VbvSuccessful
            | JuspayOrderStatus::CodInitiated => Self::Pending,
            JuspayOrderStatus::PendingVbv => Self::AuthenticationPending,
            JuspayOrderStatus::Authorized => Self::Authorized,
            JuspayOrderStatus::Charged => Self::Charged,
            JuspayOrderStatus::Voided => Self::Voided,
            JuspayOrderStatus::VoidInitiated => Self::VoidInitiated,
            JuspayOrderStatus::CaptureInitiated => Self::CaptureInitiated,
            JuspayOrderStatus::CaptureFailed => Self::CaptureFailed,
            JuspayOrderStatus::VoidFailed => Self::VoidFailed,
            JuspayOrderStatus::AutoRefunded => Self::AutoRefunded,
            JuspayOrderStatus::AuthenticationFailed
            | JuspayOrderStatus::AuthorizationFailed
            | JuspayOrderStatus::JuspayDeclined
            | JuspayOrderStatus::NotFound => Self::Failure,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct JuspayCreateOrderRequest {
    pub order_id: String,

    pub amount: StringMajorUnit,

    pub currency: Currency,

    pub customer_id: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_url: Option<String>,

    #[serde(
        rename = "metadata.webhook_url",
        skip_serializing_if = "Option::is_none"
    )]
    pub metadata_webhook_url: Option<String>,

    #[serde(rename = "metadata.txns.auto_capture")]
    pub metadata_txns_auto_capture: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JuspayCreateOrderResponse {
    pub id: String,
    pub order_id: String,
    pub status: JuspayOrderStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment_links: Option<JuspayPaymentLinks>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JuspayPaymentLinks {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mobile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iframe: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        crate::connectors::juspay::JuspayRouterData<
            RouterDataV2<
                CreateOrder,
                PaymentFlowData,
                PaymentCreateOrderData,
                PaymentCreateOrderResponse,
            >,
            T,
        >,
    > for JuspayCreateOrderRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        wrapper: crate::connectors::juspay::JuspayRouterData<
            RouterDataV2<
                CreateOrder,
                PaymentFlowData,
                PaymentCreateOrderData,
                PaymentCreateOrderResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = wrapper.router_data;

        let amount = JuspayAmountConvertor::convert(
            router_data.request.amount,
            router_data.request.currency,
        )?;

        let customer_id = router_data
            .resource_common_data
            .customer_id
            .as_ref()
            .map(|cid| cid.get_string_repr().to_string())
            .unwrap_or_else(|| router_data.resource_common_data.payment_id.clone());

        Ok(Self {
            order_id: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            amount,
            currency: router_data.request.currency,
            customer_id,
            return_url: router_data.resource_common_data.return_url.clone(),
            metadata_webhook_url: router_data.request.webhook_url.clone(),
            metadata_txns_auto_capture: false,
        })
    }
}

impl TryFrom<ResponseRouterData<JuspayCreateOrderResponse, Self>>
    for RouterDataV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    >
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<JuspayCreateOrderResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let attempt_status = AttemptStatus::from(response.status);
        let order_id = response.order_id;

        Ok(Self {
            response: Ok(PaymentCreateOrderResponse {
                connector_order_id: order_id.clone(),
                session_data: None,
            }),
            resource_common_data: PaymentFlowData {
                status: attempt_status,
                reference_id: Some(order_id.clone()),
                connector_order_id: Some(order_id),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct JuspayAuthorizeRequest {
    pub order_id: String,
    pub merchant_id: String,
    pub payment_method_type: JuspayPaymentMethodType,
    pub payment_method: JuspayPaymentMethod,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_number: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_exp_month: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_exp_year: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_on_card: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_security_code: Option<Secret<String>>,
    pub format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_type: Option<Juspay3dsAuthType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_after_payment: Option<bool>,
    #[serde(rename = "upi_vpa", skip_serializing_if = "Option::is_none")]
    pub upi_vpa: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub txn_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_channel: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JuspayAuthorizeResponse {
    pub order_id: String,
    pub txn_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub txn_uuid: Option<String>,
    pub status: JuspayOrderStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment: Option<JuspayAuthorizePayment>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JuspayAuthorizePayment {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authentication: Option<JuspayAuthentication>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JuspayAuthentication {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    pub url: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        crate::connectors::juspay::JuspayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for JuspayAuthorizeRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        wrapper: crate::connectors::juspay::JuspayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = wrapper.router_data;

        let auth = JuspayAuthType::try_from(&router_data.connector_config)?;

        let order_id = router_data
            .resource_common_data
            .connector_order_id
            .clone()
            .unwrap_or_else(|| {
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone()
            });

        let parts = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card) => {
                build_card_parts(card, &router_data.resource_common_data)?
            }
            PaymentMethodData::Upi(upi) => {
                build_upi_parts(upi, router_data.request.browser_info.as_ref())?
            }
            PaymentMethodData::Wallet(wallet) => build_wallet_parts(wallet)?,
            PaymentMethodData::RealTimePayment(rtp) => build_rtp_parts(rtp)?,
            PaymentMethodData::PayLater(pl) => build_paylater_parts(pl)?,
            PaymentMethodData::BankRedirect(bank_redirect) => match bank_redirect {
                BankRedirectData::Netbanking { issuer } => build_netbanking_parts(issuer)?,
                other => {
                    return Err(error_stack::report!(
                        errors::IntegrationError::NotImplemented(
                            format!("Juspay does not support bank redirect variant: {other:?}"),
                            Default::default(),
                        )
                    ));
                }
            },
            other => {
                return Err(error_stack::report!(
                    errors::IntegrationError::NotImplemented(
                        format!(
                            "Juspay Authorize does not support payment method variant {other:?}; \
                         supported categories are Card, Upi, Wallet, BankRedirect::Netbanking, \
                         RealTimePayment and PayLater (CONSUMER_FINANCE). See \
                         grace/rulesbook/codegen/references/juspay/technical_specification.md"
                        ),
                        Default::default(),
                    )
                ));
            }
        };

        Ok(Self {
            order_id,
            merchant_id: auth.merchant_id.expose(),
            payment_method_type: parts.payment_method_type,
            payment_method: parts.payment_method,
            card_number: parts.card_number,
            card_exp_month: parts.card_exp_month,
            card_exp_year: parts.card_exp_year,
            name_on_card: parts.name_on_card,
            card_security_code: parts.card_security_code,
            format: "json".to_string(),
            auth_type: parts.auth_type,
            redirect_after_payment: parts.redirect_after_payment,
            upi_vpa: parts.upi_vpa,
            txn_type: parts.txn_type,
            payment_channel: parts.payment_channel,
        })
    }
}

struct PmParts {
    payment_method_type: JuspayPaymentMethodType,
    payment_method: JuspayPaymentMethod,
    card_number: Option<Secret<String>>,
    card_exp_month: Option<Secret<String>>,
    card_exp_year: Option<Secret<String>>,
    card_security_code: Option<Secret<String>>,
    name_on_card: Option<Secret<String>>,
    auth_type: Option<Juspay3dsAuthType>,
    redirect_after_payment: Option<bool>,
    upi_vpa: Option<Secret<String>>,
    txn_type: Option<String>,
    payment_channel: Option<String>,
}

fn build_card_parts<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static>(
    card: &domain_types::payment_method_data::Card<T>,
    flow: &PaymentFlowData,
) -> Result<PmParts, error_stack::Report<errors::IntegrationError>> {
    let card_network = card.card_network.as_ref().ok_or_else(|| {
        error_stack::report!(errors::IntegrationError::MissingRequiredField {
            field_name: "payment_method_data.card.card_network",
            context: Default::default(),
        })
    })?;
    let payment_method = card_network_to_juspay(card_network)?;

    let card_exp_year = card.get_card_expiry_year_2_digit()?;

    let is_three_ds = matches!(flow.auth_type, common_enums::AuthenticationType::ThreeDs);
    let auth_type = if is_three_ds {
        Juspay3dsAuthType::ThreeDs
    } else {
        Juspay3dsAuthType::NoThreeDs
    };
    let redirect_after_payment = if is_three_ds { Some(true) } else { None };

    Ok(PmParts {
        payment_method_type: JuspayPaymentMethodType::Card,
        payment_method,
        card_number: Some(Secret::new(card.card_number.peek().to_string())),
        card_exp_month: Some(card.card_exp_month.clone()),
        card_exp_year: Some(card_exp_year),
        card_security_code: Some(card.card_cvc.clone()),
        name_on_card: card.card_holder_name.clone(),
        auth_type: Some(auth_type),
        redirect_after_payment,
        upi_vpa: None,
        txn_type: None,
        payment_channel: None,
    })
}

fn build_upi_parts(
    upi: &UpiData,
    browser_info: Option<&domain_types::router_request_types::BrowserInformation>,
) -> Result<PmParts, error_stack::Report<errors::IntegrationError>> {
    let (payment_method, txn_type, upi_vpa) = upi_variant_to_juspay(upi);
    let payment_channel = derive_payment_channel(browser_info);

    Ok(PmParts {
        payment_method_type: JuspayPaymentMethodType::Upi,
        payment_method,
        card_number: None,
        card_exp_month: None,
        card_exp_year: None,
        card_security_code: None,
        name_on_card: None,
        auth_type: None,
        redirect_after_payment: None,
        upi_vpa,
        txn_type: Some(txn_type.to_string()),
        payment_channel,
    })
}

fn upi_variant_to_juspay(
    upi: &UpiData,
) -> (JuspayPaymentMethod, &'static str, Option<Secret<String>>) {
    match upi {
        UpiData::UpiCollect(collect) => {
            let vpa = collect
                .vpa_id
                .as_ref()
                .map(|v| Secret::new(v.clone().expose()));
            (JuspayPaymentMethod::UpiCollect, "UPI_COLLECT", vpa)
        }
        UpiData::UpiIntent(_) => (JuspayPaymentMethod::UpiPay, "UPI_PAY", None),
        UpiData::UpiQr(_) => (JuspayPaymentMethod::UpiPay, "UPI_PAY", None),
    }
}

fn derive_payment_channel(
    browser_info: Option<&domain_types::router_request_types::BrowserInformation>,
) -> Option<String> {
    let ua = browser_info.and_then(|b| b.user_agent.as_deref())?;
    let lower = ua.to_ascii_lowercase();
    if lower.contains("android") || lower.contains("iphone") || lower.contains("ipad") {
        return None;
    }
    if lower.contains("windows")
        || lower.contains("macintosh")
        || lower.contains("mac os x")
        || lower.contains("linux")
        || lower.contains("x11")
    {
        Some("DESKTOP".to_string())
    } else {
        None
    }
}

fn card_network_to_juspay(
    network: &CardNetwork,
) -> Result<JuspayPaymentMethod, error_stack::Report<errors::IntegrationError>> {
    Ok(match network {
        CardNetwork::Visa => JuspayPaymentMethod::Visa,
        CardNetwork::Mastercard => JuspayPaymentMethod::Mastercard,
        CardNetwork::AmericanExpress => JuspayPaymentMethod::Amex,
        CardNetwork::JCB => JuspayPaymentMethod::Jcb,
        CardNetwork::DinersClub => JuspayPaymentMethod::Diners,
        CardNetwork::Discover => JuspayPaymentMethod::Discover,
        CardNetwork::UnionPay => JuspayPaymentMethod::Unionpay,
        CardNetwork::RuPay => JuspayPaymentMethod::Rupay,
        CardNetwork::Maestro => JuspayPaymentMethod::Maestro,
        other => {
            return Err(error_stack::report!(
                errors::IntegrationError::NotImplemented(
                    format!("juspay card network: {other:?}"),
                    Default::default(),
                )
            ));
        }
    })
}

fn build_wallet_parts(
    wallet: &WalletData,
) -> Result<PmParts, error_stack::Report<errors::IntegrationError>> {
    let payment_method = wallet_to_juspay(wallet)?;

    Ok(PmParts {
        payment_method_type: JuspayPaymentMethodType::Wallet,
        payment_method,
        card_number: None,
        card_exp_month: None,
        card_exp_year: None,
        card_security_code: None,
        name_on_card: None,
        auth_type: None,
        redirect_after_payment: Some(true),
        upi_vpa: None,
        txn_type: None,
        payment_channel: None,
    })
}

fn wallet_to_juspay(
    wallet: &WalletData,
) -> Result<JuspayPaymentMethod, error_stack::Report<errors::IntegrationError>> {
    match wallet {
        WalletData::PhonePeRedirect(_) => Ok(JuspayPaymentMethod::PhonepeWallet),
        WalletData::LazyPayRedirect(_) => Ok(JuspayPaymentMethod::Lazypay),
        WalletData::AmazonPayRedirect(_) => Ok(JuspayPaymentMethod::Amazonpay),
        WalletData::PaypalRedirect(_) | WalletData::PaypalSdk(_) => Ok(JuspayPaymentMethod::Paypal),
        WalletData::AliPayRedirect(_) | WalletData::AliPayQr(_) => Ok(JuspayPaymentMethod::Alipay),
        WalletData::AliPayHkRedirect(_) => Ok(JuspayPaymentMethod::AlipayHk),
        WalletData::ApplePay(_)
        | WalletData::ApplePayRedirect(_)
        | WalletData::ApplePayThirdPartySdk(_) => Ok(JuspayPaymentMethod::Applepay),
        WalletData::GooglePay(_)
        | WalletData::GooglePayRedirect(_)
        | WalletData::GooglePayThirdPartySdk(_) => Ok(JuspayPaymentMethod::Googlepay),
        WalletData::MomoRedirect(_) => Ok(JuspayPaymentMethod::Momo),
        WalletData::KakaoPayRedirect(_) => Ok(JuspayPaymentMethod::Kakaopay),
        WalletData::WeChatPayRedirect(_) | WalletData::WeChatPayQr(_) => {
            Ok(JuspayPaymentMethod::Wechatpay)
        }
        WalletData::GoPayRedirect(_) => Ok(JuspayPaymentMethod::Gopay),
        WalletData::GcashRedirect(_) => Ok(JuspayPaymentMethod::Gcash),
        WalletData::TouchNGoRedirect(_) => Ok(JuspayPaymentMethod::Touchngo),
        WalletData::SamsungPay(_) => Ok(JuspayPaymentMethod::Samsungpay),
        WalletData::BillDeskRedirect(_)
        | WalletData::CashfreeRedirect(_)
        | WalletData::PayURedirect(_)
        | WalletData::EaseBuzzRedirect(_) => Err(error_stack::report!(
            errors::IntegrationError::NotImplemented(
                format!("Juspay does not support aggregator wallet variant: {wallet:?}"),
                Default::default(),
            )
        )),
        WalletData::BluecodeRedirect {}
        | WalletData::DanaRedirect {}
        | WalletData::MbWayRedirect(_)
        | WalletData::MobilePayRedirect(_)
        | WalletData::TwintRedirect {}
        | WalletData::VippsRedirect {}
        | WalletData::CashappQr(_)
        | WalletData::SwishQr(_)
        | WalletData::Mifinity(_)
        | WalletData::RevolutPay(_)
        | WalletData::MbWay(_)
        | WalletData::Satispay(_)
        | WalletData::Wero(_)
        | WalletData::Paze(_)
        | WalletData::QwikcilverWalletDirect(_)
        | WalletData::Skrill(_)
        | WalletData::PaymayaRedirect(_) => Err(error_stack::report!(
            errors::IntegrationError::NotImplemented(
                format!("Juspay wallet variant not supported: {wallet:?}"),
                Default::default(),
            )
        )),
    }
}

fn build_netbanking_parts(
    issuer: &BankNames,
) -> Result<PmParts, error_stack::Report<errors::IntegrationError>> {
    let pm_code = bank_names_to_juspay_nb_code(issuer)?;

    Ok(PmParts {
        payment_method_type: JuspayPaymentMethodType::Nb,
        payment_method: pm_code,
        card_number: None,
        card_exp_month: None,
        card_exp_year: None,
        card_security_code: None,
        name_on_card: None,
        auth_type: None,
        redirect_after_payment: Some(true),
        upi_vpa: None,
        txn_type: None,
        payment_channel: None,
    })
}

fn build_rtp_parts(
    rtp: &RealTimePaymentData,
) -> Result<PmParts, error_stack::Report<errors::IntegrationError>> {
    let pm_code = rtp_to_juspay(rtp)?;

    Ok(PmParts {
        payment_method_type: JuspayPaymentMethodType::Rtp,
        payment_method: pm_code,
        card_number: None,
        card_exp_month: None,
        card_exp_year: None,
        card_security_code: None,
        name_on_card: None,
        auth_type: None,
        redirect_after_payment: Some(true),
        upi_vpa: None,
        txn_type: None,
        payment_channel: None,
    })
}

fn build_paylater_parts(
    pl: &PayLaterData,
) -> Result<PmParts, error_stack::Report<errors::IntegrationError>> {
    let pm_code = paylater_to_juspay(pl)?;

    Ok(PmParts {
        payment_method_type: JuspayPaymentMethodType::ConsumerFinance,
        payment_method: pm_code,
        card_number: None,
        card_exp_month: None,
        card_exp_year: None,
        card_security_code: None,
        name_on_card: None,
        auth_type: None,
        redirect_after_payment: Some(true),
        upi_vpa: None,
        txn_type: None,
        payment_channel: None,
    })
}

fn paylater_to_juspay(
    pl: &PayLaterData,
) -> Result<JuspayPaymentMethod, error_stack::Report<errors::IntegrationError>> {
    match pl {
        PayLaterData::AtomeRedirect {} => Ok(JuspayPaymentMethod::AtomePaylater),
        other @ (PayLaterData::KlarnaRedirect {}
        | PayLaterData::KlarnaSdk { .. }
        | PayLaterData::AffirmRedirect {}
        | PayLaterData::AfterpayClearpayRedirect {}
        | PayLaterData::PayBrightRedirect {}
        | PayLaterData::WalleyRedirect {}
        | PayLaterData::AlmaRedirect {}
        | PayLaterData::TamaraRedirect {}) => Err(error_stack::report!(
            errors::IntegrationError::NotImplemented(
                format!(
                    "Juspay CONSUMER_FINANCE does not map cleanly from {other:?} \
                     — use the native connector for this BNPL"
                ),
                Default::default(),
            )
        )),
    }
}

fn rtp_to_juspay(
    rtp: &RealTimePaymentData,
) -> Result<JuspayPaymentMethod, error_stack::Report<errors::IntegrationError>> {
    match rtp {
        RealTimePaymentData::DuitNow {} => Ok(JuspayPaymentMethod::DuitnowQr),
        RealTimePaymentData::Fps {} => Ok(JuspayPaymentMethod::FpsQr),
        RealTimePaymentData::PromptPay {} => Ok(JuspayPaymentMethod::PromptpayQr),
        RealTimePaymentData::VietQr {} => Ok(JuspayPaymentMethod::Vietqr),
    }
}

fn bank_names_to_juspay_nb_code(
    bank: &BankNames,
) -> Result<JuspayPaymentMethod, error_stack::Report<errors::IntegrationError>> {
    match bank {
        BankNames::HdfcBank => Ok(JuspayPaymentMethod::JpHdfc),
        BankNames::IciciBank => Ok(JuspayPaymentMethod::JpIcici),
        BankNames::AxisBank => Ok(JuspayPaymentMethod::JpAxis),
        BankNames::StateBank => Ok(JuspayPaymentMethod::JpSbi),
        BankNames::KotakMahindraBank => Ok(JuspayPaymentMethod::JpKotak),
        BankNames::IndusIndBank => Ok(JuspayPaymentMethod::JpIndb),
        BankNames::IdbiBank => Ok(JuspayPaymentMethod::JpIdbi),
        BankNames::UnionBankOfIndia => Ok(JuspayPaymentMethod::JpUbi),
        BankNames::PunjabNationalBank => Ok(JuspayPaymentMethod::JpPnb),
        BankNames::CanaraBank => Ok(JuspayPaymentMethod::JpCanr),
        BankNames::FederalBank => Ok(JuspayPaymentMethod::JpFed),
        BankNames::YesBank => Ok(JuspayPaymentMethod::JpYesb),
        BankNames::IndianOverseasBank => Ok(JuspayPaymentMethod::JpIob),
        BankNames::CentralBankOfIndia => Ok(JuspayPaymentMethod::JpCbi),
        BankNames::StandardCharteredBank => Ok(JuspayPaymentMethod::JpScb),
        BankNames::BankOfBaroda => Err(error_stack::report!(
            errors::IntegrationError::NotImplemented(
                "Juspay Net Banking does not document a code for BankOfBaroda".to_string(),
                Default::default(),
            )
        )),
        other => Err(error_stack::report!(
            errors::IntegrationError::NotImplemented(
                format!("Juspay Net Banking does not support bank: {other:?}"),
                Default::default(),
            )
        )),
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<JuspayAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<JuspayAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let status = AttemptStatus::from(response.status);

        let redirection_data = response
            .payment
            .as_ref()
            .and_then(|p| p.authentication.as_ref())
            .map(|auth| {
                let method = auth
                    .method
                    .as_deref()
                    .and_then(|m| match m.to_ascii_uppercase().as_str() {
                        "GET" => Some(Method::Get),
                        "POST" => Some(Method::Post),
                        _ => None,
                    })
                    .unwrap_or(Method::Get);
                RedirectForm::Form {
                    endpoint: auth.url.clone(),
                    method,
                    form_fields: HashMap::new(),
                }
            });

        let connector_txn_id = response
            .txn_uuid
            .clone()
            .unwrap_or_else(|| response.txn_id.clone());

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(connector_txn_id),
                redirection_data: redirection_data.map(Box::new),
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(response.txn_id),
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JuspayOrderStatusResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub order_id: String,
    pub status: JuspayOrderStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_id: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub txn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub txn_uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<FloatMajorUnit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount_refunded: Option<FloatMajorUnit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refunded: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gateway_reference_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment_gateway_response: Option<JuspayPaymentGatewayResponse>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refunds: Option<Vec<JuspayRefundEntry>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JuspayPaymentGatewayResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rrn: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epg_txn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_id_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resp_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resp_message: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JuspayRefundEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unique_request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<JuspayRefundStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<FloatMajorUnit>,
    #[serde(default, rename = "ref", skip_serializing_if = "Option::is_none")]
    pub ref_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refund_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refund_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sent_to_gateway: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initiated_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pg_processed_at: Option<String>,
}

impl TryFrom<ResponseRouterData<JuspayOrderStatusResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<JuspayOrderStatusResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let status = AttemptStatus::from(response.status);

        let connector_txn_id = response
            .txn_uuid
            .clone()
            .or_else(|| response.txn_id.clone())
            .unwrap_or_else(|| response.order_id.clone());

        let network_txn_id = response
            .payment_gateway_response
            .as_ref()
            .and_then(|pg| pg.rrn.clone());

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(connector_txn_id),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id,
                network_txn_link_id: None,
                connector_response_reference_id: response
                    .txn_id
                    .clone()
                    .or_else(|| response.gateway_reference_id.clone()),
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

#[derive(Debug, Clone, Serialize)]
pub struct JuspayCaptureRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<StringMajorUnit>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JuspayCaptureResponse {
    pub txn_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub txn_uuid: Option<String>,
    pub order_id: String,
    pub status: JuspayOrderStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<FloatMajorUnit>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        crate::connectors::juspay::JuspayRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for JuspayCaptureRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        wrapper: crate::connectors::juspay::JuspayRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = wrapper.router_data;

        let amount_to_capture = router_data.request.minor_amount_to_capture;
        let original_amount = router_data
            .resource_common_data
            .amount
            .as_ref()
            .map(|money| money.amount);

        let amount = match original_amount {
            Some(total) if total == amount_to_capture => None,
            _ => Some(JuspayAmountConvertor::convert(
                amount_to_capture,
                router_data.request.currency,
            )?),
        };

        Ok(Self { amount })
    }
}

impl TryFrom<ResponseRouterData<JuspayCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<JuspayCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let status = AttemptStatus::from(response.status);

        let connector_txn_id = response
            .txn_uuid
            .clone()
            .unwrap_or_else(|| response.txn_id.clone());

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(connector_txn_id),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(response.txn_id),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JuspayRefundStatus {
    Pending,
    Success,
    Failure,
    ManualReview,
    TransferScheduled,
}

impl From<JuspayRefundStatus> for RefundStatus {
    fn from(status: JuspayRefundStatus) -> Self {
        match status {
            JuspayRefundStatus::Success => Self::Success,
            JuspayRefundStatus::Failure => Self::Failure,
            JuspayRefundStatus::ManualReview => Self::ManualReview,
            JuspayRefundStatus::Pending | JuspayRefundStatus::TransferScheduled => Self::Pending,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct JuspayRefundRequest {
    pub unique_request_id: String,
    pub amount: StringMajorUnit,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JuspayRefundResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<JuspayOrderStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<FloatMajorUnit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount_refunded: Option<FloatMajorUnit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refunded: Option<bool>,
    pub refunds: Vec<JuspayRefundEntry>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        crate::connectors::juspay::JuspayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for JuspayRefundRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        wrapper: crate::connectors::juspay::JuspayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = wrapper.router_data;

        let amount = JuspayAmountConvertor::convert(
            router_data.request.minor_refund_amount,
            router_data.request.currency,
        )?;

        Ok(Self {
            unique_request_id: router_data.request.refund_id.clone(),
            amount,
        })
    }
}

impl TryFrom<ResponseRouterData<JuspayRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(item: ResponseRouterData<JuspayRefundResponse, Self>) -> Result<Self, Self::Error> {
        let response = item.response;
        let router_data = item.router_data;
        let unique_request_id = router_data.request.refund_id.clone();

        let entry = response
            .refunds
            .iter()
            .find(|r| {
                r.unique_request_id
                    .as_deref()
                    .map(|id| id == unique_request_id)
                    .unwrap_or(false)
            })
            .or_else(|| response.refunds.last())
            .ok_or_else(|| {
                error_stack::report!(crate::utils::response_deserialization_fail(
                    item.http_code,
                    "juspay refund response did not contain any refund entries",
                ))
            })?;

        let refund_status = entry
            .status
            .map(RefundStatus::from)
            .unwrap_or(RefundStatus::Pending);

        let connector_refund_id = entry.unique_request_id.clone().unwrap_or(unique_request_id);

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(transparent)]
pub struct JuspayRefundSyncResponse(pub JuspayOrderStatusResponse);

impl TryFrom<ResponseRouterData<JuspayRefundSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<JuspayRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response.0;
        let router_data = item.router_data;
        let connector_refund_id = router_data.request.connector_refund_id.clone();

        let matching_entry = response.refunds.as_ref().and_then(|entries| {
            entries.iter().find(|r| {
                r.unique_request_id
                    .as_deref()
                    .map(|id| id == connector_refund_id)
                    .unwrap_or(false)
            })
        });

        let (refund_status, resolved_refund_id) = match matching_entry {
            Some(entry) => {
                let status = entry
                    .status
                    .map(RefundStatus::from)
                    .unwrap_or(RefundStatus::Pending);
                let id = entry
                    .unique_request_id
                    .clone()
                    .unwrap_or_else(|| connector_refund_id.clone());
                (status, id)
            }
            None => (RefundStatus::Pending, connector_refund_id.clone()),
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: resolved_refund_id,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct JuspayVoidRequest {}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JuspayVoidResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub txn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub txn_uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
    pub status: JuspayOrderStatus,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        crate::connectors::juspay::JuspayRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for JuspayVoidRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        _wrapper: crate::connectors::juspay::JuspayRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

impl TryFrom<ResponseRouterData<JuspayVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(item: ResponseRouterData<JuspayVoidResponse, Self>) -> Result<Self, Self::Error> {
        let response = item.response;
        let router_data = item.router_data;
        let status = AttemptStatus::from(response.status);

        let connector_txn_id = response
            .txn_uuid
            .clone()
            .or_else(|| response.txn_id.clone())
            .unwrap_or_else(|| router_data.request.connector_transaction_id.clone());

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(connector_txn_id),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: response.txn_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

#[derive(Debug, Clone)]
pub struct JuspayCardSyncAuthType {
    pub api_key: Secret<String>,
    pub juspay_encryption_public_key: Secret<String>,
    pub response_decryption_private_key: Secret<String>,
    pub card_sync_key_id: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for JuspayCardSyncAuthType {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(config: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        let missing = |field_name: &'static str| {
            error_stack::report!(errors::IntegrationError::MissingRequiredField {
                field_name,
                context: Default::default(),
            })
        };

        match config {
            ConnectorSpecificConfig::Juspay {
                api_key,
                juspay_encryption_public_key,
                response_decryption_private_key,
                card_sync_key_id,
                ..
            } => Ok(Self {
                api_key: api_key.to_owned(),
                juspay_encryption_public_key: juspay_encryption_public_key
                    .to_owned()
                    .ok_or_else(|| missing("juspay_encryption_public_key"))?,
                response_decryption_private_key: response_decryption_private_key
                    .to_owned()
                    .ok_or_else(|| missing("response_decryption_private_key"))?,
                card_sync_key_id: card_sync_key_id
                    .to_owned()
                    .ok_or_else(|| missing("card_sync_key_id"))?,
            }),
            _ => Err(error_stack::report!(
                errors::IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext {
                        additional_context: Some(
                            "Juspay account updater received a connector config for a different \
                             connector; only ConnectorSpecificConfig::Juspay carries the card-sync \
                             keys"
                                .to_owned()
                        ),
                        suggested_action: Some(
                            "Send the Juspay connector config as x-connector-config metadata on \
                             the Refresh request"
                                .to_owned()
                        ),
                        doc_url: None,
                    }
                }
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JuspayCardNetwork {
    Visa,
    Mastercard,
}

impl TryFrom<&CardNetwork> for JuspayCardNetwork {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(network: &CardNetwork) -> Result<Self, Self::Error> {
        match network {
            CardNetwork::Visa => Ok(Self::Visa),
            CardNetwork::Mastercard => Ok(Self::Mastercard),
            unsupported => Err(error_stack::report!(
                errors::IntegrationError::NotSupported {
                    message: format!(
                        "card network {unsupported} is not supported for account updater; supported: Visa, Mastercard"
                    ),
                    connector: "juspay",
                    context: Default::default(),
                }
            )),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct JuspayCardSyncPlaintext {
    account_number: Secret<String>,
    expiry_month: Secret<String>,
    expiry_year: Secret<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JuspayCardSyncRequest {
    pub network: JuspayCardNetwork,
    pub card_data: Secret<String>,
    pub key_id: Secret<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JuspayCardSyncResponseCode {
    AccountUpdated,
    ExpiryUpdated,
    NoChange,
    CardClosed,
    CardNotFound,
    ContactIssuer,
    #[serde(untagged)]
    Unknown(String),
}

impl JuspayCardSyncResponseCode {
    pub fn as_provider_code(&self) -> String {
        match self {
            Self::AccountUpdated => "ACCOUNT_UPDATED".to_string(),
            Self::ExpiryUpdated => "EXPIRY_UPDATED".to_string(),
            Self::NoChange => "NO_CHANGE".to_string(),
            Self::CardClosed => "CARD_CLOSED".to_string(),
            Self::CardNotFound => "CARD_NOT_FOUND".to_string(),
            Self::ContactIssuer => "CONTACT_ISSUER".to_string(),
            Self::Unknown(code) => code.clone(),
        }
    }
}

impl From<&JuspayCardSyncResponseCode> for CardRefreshOutcome {
    fn from(code: &JuspayCardSyncResponseCode) -> Self {
        match code {
            JuspayCardSyncResponseCode::AccountUpdated => Self::AccountUpdated,
            JuspayCardSyncResponseCode::ExpiryUpdated => Self::ExpiryUpdated,
            JuspayCardSyncResponseCode::NoChange => Self::NoChange,
            JuspayCardSyncResponseCode::CardClosed => Self::Closed,
            JuspayCardSyncResponseCode::CardNotFound => Self::NotFound,
            JuspayCardSyncResponseCode::ContactIssuer => Self::ContactIssuer,
            JuspayCardSyncResponseCode::Unknown(_) => Self::Unrecognized,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JuspayCardSyncStatus {
    Success,
    Failure,
    #[serde(untagged)]
    Unknown(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JuspayCardSyncResponse {
    pub status: Option<JuspayCardSyncStatus>,
    pub response_code: Option<JuspayCardSyncResponseCode>,
    pub response_message: Option<String>,
    pub payload: Option<Secret<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JuspayCardSyncPayload {
    pub updated_account_number: Option<Secret<String>>,
    pub is_account_updated: Option<bool>,
    pub updated_expiry_date: Option<Secret<String>>,
}

/// A malformed value in Juspay's *response*. Request-side validation uses the
/// error variants directly, so the two failure origins stay distinct.
pub(crate) fn invalid_gateway_response(field_name: &'static str) -> errors::IntegrationError {
    errors::IntegrationError::InvalidDataFormat {
        field_name,
        context: Default::default(),
    }
}

fn normalize_expiry_month(
    month: &Secret<String>,
) -> Result<Secret<String>, error_stack::Report<errors::IntegrationError>> {
    let parsed = month
        .peek()
        .trim()
        .parse::<u8>()
        .map_err(|_| error_stack::report!(invalid_gateway_response("card_exp_month")))
        .attach_printable("card expiry month is not a number")?;

    let month = cards::validate::CardExpirationMonth::try_from(parsed)
        .change_context(invalid_gateway_response("card_exp_month"))
        .attach_printable("card expiry month is out of range")?;

    Ok(Secret::new(month.two_digits()))
}

pub fn build_card_sync_request(
    card: &CardWithNoCvc,
    auth: &JuspayCardSyncAuthType,
) -> Result<JuspayCardSyncRequest, error_stack::Report<errors::IntegrationError>> {
    let card_network = card
        .card_network
        .as_ref()
        .ok_or_else(|| {
            error_stack::report!(errors::IntegrationError::MissingRequiredField {
                field_name: "card_network",
                context: Default::default(),
            })
        })
        .attach_printable("card sync requires an explicit card network")?;
    let network = JuspayCardNetwork::try_from(card_network)?;

    let account_number = card.card_number.get_card_no();
    let min_len = super::JUSPAY_MIN_PAN_LENGTH;
    if account_number.len() < min_len {
        return Err(error_stack::report!(
            errors::IntegrationError::InvalidDataFormat {
                field_name: "card_number",
                context: Default::default(),
            }
        ))
        .attach_printable(format!(
            "card number is shorter than the {min_len} digits Juspay accepts"
        ));
    }

    // Field formatting lives on the card type; reuse it rather than re-derive.
    let plaintext = JuspayCardSyncPlaintext {
        account_number: Secret::new(account_number),
        expiry_month: card
            .get_card_expiry_month_2_digit()
            .map_err(error_stack::Report::new)?,
        expiry_year: card.get_expiry_year_4_digit(),
    };

    let serialized = serde_json::to_string(&plaintext)
        .change_context(errors::IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        })
        .attach_printable("failed to serialize the card-sync plaintext")?;

    let serialized = serde_json::to_string(&serialized)
        .change_context(errors::IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        })
        .attach_printable("failed to re-encode the card-sync plaintext as a JSON string")?;

    let card_data = super::crypto::encrypt_card_data(
        &Secret::new(serialized),
        &auth.juspay_encryption_public_key,
    )?;

    Ok(JuspayCardSyncRequest {
        network,
        card_data,
        key_id: auth.card_sync_key_id.clone(),
    })
}

fn parse_updated_expiry(
    expiry: &Secret<String>,
) -> Result<(Secret<String>, Secret<String>), error_stack::Report<errors::IntegrationError>> {
    let raw = expiry.peek().trim();
    if raw.len() != 4 || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return Err(error_stack::report!(invalid_gateway_response(
            "updated_expiry_date"
        )))
        .attach_printable("updatedExpiryDate is not four ASCII digits in MMYY form");
    }

    let (month, year) = raw.split_at(2);

    let month = normalize_expiry_month(&Secret::new(month.to_string()))
        .change_context(invalid_gateway_response("updated_expiry_date"))?;
    let year =
        domain_types::utils::expand_expiry_year_to_four_digits(&Secret::new(year.to_string()));

    Ok((month, year))
}

fn parse_updated_card_number(
    number: &Secret<String>,
) -> Result<cards::CardNumber, error_stack::Report<errors::IntegrationError>> {
    cards::CardNumber::from_str(number.peek())
        .map_err(|_| error_stack::report!(invalid_gateway_response("updated_account_number")))
        .attach_printable("the card number returned by Juspay failed validation")
}

fn decrypt_card_sync_payload(
    payload: &Secret<String>,
    auth: &JuspayCardSyncAuthType,
) -> Result<JuspayCardSyncPayload, error_stack::Report<errors::IntegrationError>> {
    let plaintext =
        super::crypto::decrypt_payload(payload.peek(), &auth.response_decryption_private_key)?;

    serde_json::from_str(plaintext.peek())
        .change_context(invalid_gateway_response("payload"))
        .attach_printable("decrypted Juspay payload did not match the expected shape")
}

pub fn parse_card_sync_response(
    response: &JuspayCardSyncResponse,
    auth: &JuspayCardSyncAuthType,
    status_code: u16,
    submitted_card: &CardWithNoCvc,
) -> Result<RefreshPaymentMethodResponseData, error_stack::Report<errors::IntegrationError>> {
    let response_code = response
        .response_code
        .as_ref()
        .ok_or_else(|| error_stack::report!(invalid_gateway_response("response_code")))
        .attach_printable("a SUCCESS envelope arrived without a responseCode")?;

    let outcome = CardRefreshOutcome::from(response_code);

    let unchanged = |outcome| RefreshPaymentMethodResponseData {
        result: Some(RefreshPaymentMethodResult::Card(CardRefreshResult {
            outcome,
            card: submitted_card.clone(),
        })),
        status_code,
    };

    // Undefined what a payload means under an unmapped code, so don't decrypt it.
    if outcome == CardRefreshOutcome::Unrecognized {
        tracing::warn!(
            target: "juspay_card_sync",
            response_code = %response_code.as_provider_code(),
            "unmapped card sync response code; returning the submitted card unchanged"
        );
        return Ok(unchanged(outcome));
    }

    let payload = response
        .payload
        .as_ref()
        .map(|payload| decrypt_card_sync_payload(payload, auth))
        .transpose()?;

    if !outcome.is_update_outcome() {
        if let Some(payload) = &payload {
            if payload.updated_account_number.is_some() {
                return Err(error_stack::report!(invalid_gateway_response(
                    "updated_account_number"
                )))
                .attach_printable("a terminal outcome returned a card number, which it must not");
            }
            if payload.updated_expiry_date.is_some() {
                return Err(error_stack::report!(invalid_gateway_response(
                    "updated_expiry_date"
                )))
                .attach_printable("a terminal outcome returned an expiry, which it must not");
            }
        }

        return Ok(unchanged(outcome));
    }

    let payload = payload
        .ok_or_else(|| error_stack::report!(invalid_gateway_response("payload")))
        .attach_printable("an update outcome arrived without a payload")?;

    let card_number = payload
        .updated_account_number
        .as_ref()
        .map(parse_updated_card_number)
        .transpose()?;

    let expiry = payload
        .updated_expiry_date
        .as_ref()
        .map(parse_updated_expiry)
        .transpose()?;

    match outcome {
        CardRefreshOutcome::AccountUpdated if card_number.is_none() => {
            return Err(error_stack::report!(invalid_gateway_response(
                "updated_account_number"
            )))
            .attach_printable("ACCOUNT_UPDATED arrived without a replacement card number");
        }
        CardRefreshOutcome::ExpiryUpdated if expiry.is_none() => {
            return Err(error_stack::report!(invalid_gateway_response(
                "updated_expiry_date"
            )))
            .attach_printable("EXPIRY_UPDATED arrived without a replacement expiry");
        }
        _ => {}
    }

    let (card_exp_month, card_exp_year) = match expiry {
        Some((month, year)) => (month, year),
        None => (
            submitted_card.card_exp_month.clone(),
            submitted_card.card_exp_year.clone(),
        ),
    };

    Ok(RefreshPaymentMethodResponseData {
        result: Some(RefreshPaymentMethodResult::Card(CardRefreshResult {
            outcome,
            card: CardWithNoCvc {
                card_number: card_number.unwrap_or_else(|| submitted_card.card_number.clone()),
                card_exp_month,
                card_exp_year,
                ..submitted_card.clone()
            },
        })),
        status_code,
    })
}

pub fn build_card_sync_failure(
    response: &JuspayCardSyncResponse,
    status_code: u16,
) -> domain_types::router_data::ErrorResponse {
    let code = response
        .response_code
        .as_ref()
        .map(JuspayCardSyncResponseCode::as_provider_code)
        .unwrap_or_else(|| "UNKNOWN".to_string());

    domain_types::router_data::ErrorResponse {
        status_code,
        message: response
            .response_message
            .clone()
            .unwrap_or_else(|| format!("juspay: card sync inquiry failed ({code})")),
        code,
        reason: response.response_message.clone(),
        attempt_status: None,
        connector_transaction_id: None,
        network_decline_code: None,
        network_advice_code: None,
        network_error_message: None,
    }
}

type RefreshRouterData<T> = RouterDataV2<
    RefreshPaymentMethod,
    RefreshPaymentMethodFlowData,
    RefreshPaymentMethodData<T>,
    RefreshPaymentMethodResponseData,
>;

pub(crate) fn refreshable_card<T: PaymentMethodDataTypes + std::fmt::Debug>(
    request: &RefreshPaymentMethodData<T>,
) -> Result<&CardWithNoCvc, error_stack::Report<errors::IntegrationError>> {
    match &request.payment_method_data {
        PaymentMethodData::CardWithNoCvc(card) => Ok(card),
        // Do not Debug-format the instrument — some variants have unmasked fields.
        _ => Err(error_stack::report!(
            errors::IntegrationError::NotSupported {
                message: "account updater accepts card_with_no_cvc only".to_string(),
                connector: "juspay",
                context: Default::default(),
            }
        )),
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<crate::connectors::juspay::JuspayRouterData<RefreshRouterData<T>, T>>
    for JuspayCardSyncRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        wrapper: crate::connectors::juspay::JuspayRouterData<RefreshRouterData<T>, T>,
    ) -> Result<Self, Self::Error> {
        let router_data = wrapper.router_data;
        let auth = JuspayCardSyncAuthType::try_from(&router_data.connector_config)?;
        let card = refreshable_card(&router_data.request)?;
        build_card_sync_request(card, &auth)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<JuspayCardSyncResponse, Self>> for RefreshRouterData<T>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<JuspayCardSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let handling_failed = || errors::ConnectorError::ResponseHandlingFailed {
            context: errors::ResponseTransformationErrorContext {
                http_status_code: Some(item.http_code),
                ..Default::default()
            },
        };

        let response = match item.response.status.as_ref() {
            Some(JuspayCardSyncStatus::Success) => {
                let auth = JuspayCardSyncAuthType::try_from(&item.router_data.connector_config)
                    .change_context(handling_failed())?;
                let submitted_card =
                    refreshable_card(&item.router_data.request).change_context(handling_failed())?;

                Ok(
                    parse_card_sync_response(&item.response, &auth, item.http_code, submitted_card)
                        .change_context(handling_failed())?,
                )
            }
            Some(JuspayCardSyncStatus::Failure) => {
                Err(build_card_sync_failure(&item.response, item.http_code))
            }
            unexpected => {
                return Err(error_stack::report!(handling_failed())).attach_printable(format!(
                    "unexpected card sync envelope status: {unexpected:?}"
                ));
            }
        };

        Ok(Self {
            response,
            ..item.router_data
        })
    }
}
