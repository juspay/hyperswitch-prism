use crate::types::ResponseRouterData;
use common_enums::{AttemptStatus, RefundStatus};
use common_utils::types::MinorUnit;
use domain_types::errors::{ConnectorResponseTransformationError, IntegrationError};
use domain_types::{
    connector_flow::{Authorize, PSync, RSync, RepeatPayment, SetupMandate},
    connector_types::{
        self as connector_types, PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        RepeatPaymentData, ResponseId, SetupMandateRequestData,
    },
    payment_method_data::{PaymentMethodDataTypes, RawCardNumber},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
};
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};

// ===== ENUMS =====

/// MultiSafepay order type
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Type {
    Direct,
    Redirect,
}

/// MultiSafepay gateway identifiers
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Gateway {
    Visa,
    Mastercard,
    Amex,
    Maestro,
    #[serde(rename = "DINER")]
    DinersClub,
    Discover,
    #[serde(rename = "CREDITCARD")]
    CreditCard,
    #[serde(rename = "IDEAL")]
    Ideal,
    #[serde(rename = "PAYPAL")]
    PayPal,
    #[serde(rename = "GOOGLEPAY")]
    GooglePay,
    #[serde(rename = "ALIPAY")]
    AliPay,
    #[serde(rename = "WECHATPAY")]
    WeChatPay,
    #[serde(rename = "TRUSTLY")]
    Trustly,
    #[serde(rename = "GIROPAY")]
    Giropay,
    #[serde(rename = "SOFORT")]
    Sofort,
    #[serde(rename = "EPS")]
    Eps,
    #[serde(rename = "DIRECTBANK")]
    DirectBank,
    #[serde(rename = "MBWAY")]
    MbWay,
    #[serde(rename = "DIRDEB")]
    DirectDebit,
}

// ===== HELPER FUNCTIONS =====

/// Determines the order type based on payment method
/// Cards and certain payment methods use direct flow, others use redirect flow
fn get_order_type_from_payment_method<T: PaymentMethodDataTypes>(
    payment_method_data: &domain_types::payment_method_data::PaymentMethodData<T>,
) -> Result<Type, error_stack::Report<IntegrationError>> {
    use domain_types::payment_method_data::{BankRedirectData, PaymentMethodData, WalletData};
    use error_stack::ResultExt;

    let payment_type = match payment_method_data {
        PaymentMethodData::Card(_) => Type::Direct,
        PaymentMethodData::CardRedirect(_) => Type::Redirect,
        PaymentMethodData::MandatePayment => Type::Direct,
        PaymentMethodData::Wallet(ref wallet_data) => match wallet_data {
            WalletData::GooglePay(_) => Type::Direct,
            WalletData::PaypalRedirect(_) => Type::Redirect,
            WalletData::AliPayRedirect(_) => Type::Redirect,
            WalletData::WeChatPayRedirect(_) => Type::Redirect,
            WalletData::MbWayRedirect(_) => Type::Redirect,
            WalletData::AliPayQr(_)
            | WalletData::AliPayHkRedirect(_)
            | WalletData::AmazonPayRedirect(_)
            | WalletData::BluecodeRedirect {}
            | WalletData::MomoRedirect(_)
            | WalletData::KakaoPayRedirect(_)
            | WalletData::GoPayRedirect(_)
            | WalletData::GcashRedirect(_)
            | WalletData::ApplePay(_)
            | WalletData::ApplePayRedirect(_)
            | WalletData::ApplePayThirdPartySdk(_)
            | WalletData::DanaRedirect {}
            | WalletData::GooglePayRedirect(_)
            | WalletData::GooglePayThirdPartySdk(_)
            | WalletData::MobilePayRedirect(_)
            | WalletData::PaypalSdk(_)
            | WalletData::Paze(_)
            | WalletData::SamsungPay(_)
            | WalletData::TwintRedirect {}
            | WalletData::VippsRedirect {}
            | WalletData::TouchNGoRedirect(_)
            | WalletData::WeChatPayQr(_)
            | WalletData::CashappQr(_)
            | WalletData::SwishQr(_)
            | WalletData::Mifinity(_)
            | WalletData::RevolutPay(_)
            | WalletData::MbWay(_)
            | WalletData::Satispay(_)
            | WalletData::Wero(_) => Err(IntegrationError::not_implemented(
                crate::utils::get_unimplemented_payment_method_error_message("multisafepay"),
            ))
            .attach_printable("Wallet payment method not supported")?,
        },
        PaymentMethodData::BankRedirect(ref bank_data) => match bank_data {
            BankRedirectData::Giropay { .. } => Type::Redirect,
            BankRedirectData::Ideal { .. } => Type::Direct,
            BankRedirectData::Trustly { .. } => Type::Redirect,
            BankRedirectData::Eps { .. } => Type::Redirect,
            BankRedirectData::Sofort { .. } => Type::Redirect,
            BankRedirectData::BancontactCard { .. }
            | BankRedirectData::Bizum { .. }
            | BankRedirectData::Blik { .. }
            | BankRedirectData::Eft { .. }
            | BankRedirectData::Interac { .. }
            | BankRedirectData::OnlineBankingCzechRepublic { .. }
            | BankRedirectData::OnlineBankingFinland { .. }
            | BankRedirectData::OnlineBankingPoland { .. }
            | BankRedirectData::OnlineBankingSlovakia { .. }
            | BankRedirectData::OpenBankingUk { .. }
            | BankRedirectData::Przelewy24 { .. }
            | BankRedirectData::OnlineBankingFpx { .. }
            | BankRedirectData::OnlineBankingThailand { .. }
            | BankRedirectData::LocalBankRedirect {}
            | BankRedirectData::OpenBanking {} => Err(IntegrationError::not_implemented(
                crate::utils::get_unimplemented_payment_method_error_message("multisafepay"),
            ))
            .attach_printable("Bank redirect payment method not supported")?,
        },
        PaymentMethodData::PayLater(_) => Type::Redirect,
        PaymentMethodData::BankDebit(_) => Type::Direct,
        PaymentMethodData::BankTransfer(_)
        | PaymentMethodData::Crypto(_)
        | PaymentMethodData::Reward
        | PaymentMethodData::RealTimePayment(_)
        | PaymentMethodData::MobilePayment(_)
        | PaymentMethodData::Upi(_)
        | PaymentMethodData::Voucher(_)
        | PaymentMethodData::GiftCard(_)
        | PaymentMethodData::OpenBanking(_)
        | PaymentMethodData::CardToken(_)
        | PaymentMethodData::NetworkToken(_)
        | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
        | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
            Err(IntegrationError::not_implemented(
                crate::utils::get_unimplemented_payment_method_error_message("multisafepay"),
            ))
            .attach_printable("Payment method not supported")?
        }
    };

    Ok(payment_type)
}

/// Maps CardNetwork enum to MultiSafepay gateway identifier
/// Returns None for unrecognized networks to trigger fallback to card number detection
fn card_network_to_gateway(network: &common_enums::CardNetwork) -> Option<Gateway> {
    match network {
        common_enums::CardNetwork::Visa => Some(Gateway::Visa),
        common_enums::CardNetwork::Mastercard => Some(Gateway::Mastercard),
        common_enums::CardNetwork::AmericanExpress => Some(Gateway::Amex),
        common_enums::CardNetwork::Maestro => Some(Gateway::Maestro),
        common_enums::CardNetwork::DinersClub => Some(Gateway::DinersClub),
        common_enums::CardNetwork::Discover => Some(Gateway::Discover),
        // Unknown card networks will fall through to card number detection
        common_enums::CardNetwork::Interac
        | common_enums::CardNetwork::JCB
        | common_enums::CardNetwork::UnionPay
        | common_enums::CardNetwork::RuPay
        | common_enums::CardNetwork::CartesBancaires
        | common_enums::CardNetwork::Star
        | common_enums::CardNetwork::Pulse
        | common_enums::CardNetwork::Accel
        | common_enums::CardNetwork::Nyce => None,
    }
}

/// Maps CardIssuer (from card number analysis) to MultiSafepay gateway identifier
/// Returns None for unsupported card types to trigger CREDITCARD fallback
fn card_issuer_to_gateway(issuer: domain_types::utils::CardIssuer) -> Option<Gateway> {
    match issuer {
        domain_types::utils::CardIssuer::Visa => Some(Gateway::Visa),
        domain_types::utils::CardIssuer::Master => Some(Gateway::Mastercard),
        domain_types::utils::CardIssuer::AmericanExpress => Some(Gateway::Amex),
        domain_types::utils::CardIssuer::Maestro => Some(Gateway::Maestro),
        domain_types::utils::CardIssuer::DinersClub => Some(Gateway::DinersClub),
        domain_types::utils::CardIssuer::Discover => Some(Gateway::Discover),
        // Unsupported card types will use CREDITCARD fallback
        domain_types::utils::CardIssuer::JCB
        | domain_types::utils::CardIssuer::CarteBlanche
        | domain_types::utils::CardIssuer::UnionPay
        | domain_types::utils::CardIssuer::CartesBancaires => None,
    }
}

/// Maps payment method data to MultiSafepay gateway identifier
/// Uses three-tier detection for cards: card_network metadata -> card number analysis -> CREDITCARD fallback
fn get_gateway_from_payment_method<T: PaymentMethodDataTypes>(
    payment_method_data: &domain_types::payment_method_data::PaymentMethodData<T>,
) -> Result<Gateway, error_stack::Report<IntegrationError>> {
    use domain_types::payment_method_data::{BankRedirectData, PaymentMethodData, WalletData};
    use error_stack::ResultExt;

    let gateway = match payment_method_data {
        PaymentMethodData::Card(card_data) => {
            // Tier 1: Try to use card_network metadata (fast path)
            if let Some(ref network) = card_data.card_network {
                if let Some(gateway) = card_network_to_gateway(network) {
                    return Ok(gateway);
                }
            }

            // Tier 2: Try to detect from card number (more reliable)
            if let Ok(card_number_str) = get_card_number_string(&card_data.card_number) {
                if let Ok(issuer) = domain_types::utils::get_card_issuer(&card_number_str) {
                    if let Some(gateway) = card_issuer_to_gateway(issuer) {
                        return Ok(gateway);
                    }
                }
            }

            // Tier 3: Final fallback for unsupported or undetectable card types
            Gateway::CreditCard
        }
        PaymentMethodData::CardRedirect(_) => {
            // Card redirect payments use generic credit card gateway
            Gateway::CreditCard
        }
        PaymentMethodData::BankRedirect(ref bank_data) => match bank_data {
            BankRedirectData::Ideal { .. } => Gateway::Ideal,
            BankRedirectData::Giropay { .. } => Gateway::Giropay,
            BankRedirectData::Trustly { .. } => Gateway::Trustly,
            BankRedirectData::Eps { .. } => Gateway::Eps,
            BankRedirectData::Sofort { .. } => Gateway::Sofort,
            BankRedirectData::BancontactCard { .. }
            | BankRedirectData::Bizum { .. }
            | BankRedirectData::Blik { .. }
            | BankRedirectData::Eft { .. }
            | BankRedirectData::Interac { .. }
            | BankRedirectData::OnlineBankingCzechRepublic { .. }
            | BankRedirectData::OnlineBankingFinland { .. }
            | BankRedirectData::OnlineBankingPoland { .. }
            | BankRedirectData::OnlineBankingSlovakia { .. }
            | BankRedirectData::OpenBankingUk { .. }
            | BankRedirectData::Przelewy24 { .. }
            | BankRedirectData::OnlineBankingFpx { .. }
            | BankRedirectData::OnlineBankingThailand { .. }
            | BankRedirectData::LocalBankRedirect {}
            | BankRedirectData::OpenBanking {} => Err(IntegrationError::not_implemented(
                crate::utils::get_unimplemented_payment_method_error_message("multisafepay"),
            ))
            .attach_printable("Bank redirect payment method not supported")?,
        },
        PaymentMethodData::Wallet(ref wallet_data) => match wallet_data {
            WalletData::GooglePay(_) => Gateway::GooglePay,
            WalletData::PaypalRedirect(_) => Gateway::PayPal,
            WalletData::AliPayRedirect(_) => Gateway::AliPay,
            WalletData::WeChatPayRedirect(_) => Gateway::WeChatPay,
            WalletData::MbWayRedirect(_) => Gateway::MbWay,
            WalletData::AliPayQr(_)
            | WalletData::AliPayHkRedirect(_)
            | WalletData::AmazonPayRedirect(_)
            | WalletData::BluecodeRedirect {}
            | WalletData::MomoRedirect(_)
            | WalletData::KakaoPayRedirect(_)
            | WalletData::GoPayRedirect(_)
            | WalletData::GcashRedirect(_)
            | WalletData::ApplePay(_)
            | WalletData::ApplePayRedirect(_)
            | WalletData::ApplePayThirdPartySdk(_)
            | WalletData::DanaRedirect {}
            | WalletData::GooglePayRedirect(_)
            | WalletData::GooglePayThirdPartySdk(_)
            | WalletData::MobilePayRedirect(_)
            | WalletData::PaypalSdk(_)
            | WalletData::Paze(_)
            | WalletData::SamsungPay(_)
            | WalletData::TwintRedirect {}
            | WalletData::VippsRedirect {}
            | WalletData::TouchNGoRedirect(_)
            | WalletData::WeChatPayQr(_)
            | WalletData::CashappQr(_)
            | WalletData::SwishQr(_)
            | WalletData::Mifinity(_)
            | WalletData::RevolutPay(_)
            | WalletData::MbWay(_)
            | WalletData::Satispay(_)
            | WalletData::Wero(_) => Err(IntegrationError::not_implemented(
                crate::utils::get_unimplemented_payment_method_error_message("multisafepay"),
            ))
            .attach_printable("Wallet payment method not supported")?,
        },
        PaymentMethodData::BankDebit(ref bank_debit_data) => {
            use domain_types::payment_method_data::BankDebitData;
            match bank_debit_data {
                BankDebitData::SepaBankDebit { .. } => Gateway::DirectDebit,
                BankDebitData::AchBankDebit { .. }
                | BankDebitData::BecsBankDebit { .. }
                | BankDebitData::BacsBankDebit { .. }
                | BankDebitData::SepaGuaranteedBankDebit { .. } => {
                    Err(IntegrationError::not_implemented(
                        crate::utils::get_unimplemented_payment_method_error_message(
                            "multisafepay",
                        ),
                    ))
                    .attach_printable("Only SEPA bank debit is supported by MultiSafepay")?
                }
            }
        }
        PaymentMethodData::MandatePayment
        | PaymentMethodData::PayLater(_)
        | PaymentMethodData::BankTransfer(_)
        | PaymentMethodData::Crypto(_)
        | PaymentMethodData::Reward
        | PaymentMethodData::RealTimePayment(_)
        | PaymentMethodData::MobilePayment(_)
        | PaymentMethodData::Upi(_)
        | PaymentMethodData::Voucher(_)
        | PaymentMethodData::GiftCard(_)
        | PaymentMethodData::OpenBanking(_)
        | PaymentMethodData::CardToken(_)
        | PaymentMethodData::NetworkToken(_)
        | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
        | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
            Err(IntegrationError::not_implemented(
                crate::utils::get_unimplemented_payment_method_error_message("multisafepay"),
            ))
            .attach_printable("Payment method not supported")?
        }
    };

    Ok(gateway)
}

/// Helper function to extract card number as string from RawCardNumber
/// For direct transactions, we need actual PCI data (DefaultPCIHolder), not vault tokens
fn get_card_number_string<T: PaymentMethodDataTypes>(
    card_number: &RawCardNumber<T>,
) -> Result<String, error_stack::Report<IntegrationError>> {
    use error_stack::ResultExt;

    // Serialize the card number and extract the string value
    // This works for both DefaultPCIHolder (cards::CardNumber) and VaultTokenHolder (String)
    let serialized = serde_json::to_value(card_number)
        .change_context(IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        })
        .attach_printable("Failed to serialize card number")?;

    // Extract the string from the JSON value
    serialized
        .as_str()
        .map(|s| s.to_string())
        .ok_or(IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        })
        .attach_printable("Card number is not a valid string")
}

/// Builds the gateway_info field based on the payment method type
/// For card payments: populates card details (card_number, expiry, cvc)
/// For SEPA direct debit: populates IBAN and account holder name
fn build_gateway_info<T: PaymentMethodDataTypes>(
    order_type: &Type,
    payment_method_data: &domain_types::payment_method_data::PaymentMethodData<T>,
) -> Result<Option<MultisafepayGatewayInfo<T>>, error_stack::Report<IntegrationError>> {
    use domain_types::payment_method_data::{BankDebitData, PaymentMethodData};
    use error_stack::ResultExt;

    match (order_type, payment_method_data) {
        (Type::Direct, PaymentMethodData::Card(card_data)) => {
            // Build gateway_info with card details
            // Format card expiry as YYMM (2-digit year + 2-digit month) as integer
            let card_expiry_str = card_data
                .get_card_expiry_year_month_2_digit_with_delimiter(String::new())
                .change_context(IntegrationError::RequestEncodingFailed {
                    context: Default::default(),
                })?
                .expose();

            let card_expiry_date: i64 = card_expiry_str
                .parse::<i64>()
                .change_context(IntegrationError::RequestEncodingFailed {
                    context: Default::default(),
                })
                .attach_printable("Failed to parse card expiry date as integer")?;

            Ok(Some(MultisafepayGatewayInfo::Card(GatewayInfo {
                card_number: card_data.card_number.clone(),
                card_expiry_date,
                card_cvc: card_data.card_cvc.clone(),
                card_holder_name: None,
                flexible_3d: None,
                moto: None,
                term_url: None,
            })))
        }
        (Type::Direct, PaymentMethodData::BankDebit(bank_debit_data)) => match bank_debit_data {
            BankDebitData::SepaBankDebit {
                iban,
                bank_account_holder_name,
            } => {
                let account_holder = bank_account_holder_name
                    .clone()
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "bank_account_holder_name",
                        context: Default::default(),
                    })
                    .attach_printable("Account holder name is required for SEPA direct debit")?;

                Ok(Some(MultisafepayGatewayInfo::DirectDebit(
                    DirectDebitGatewayInfo {
                        account_id: iban.clone(),
                        account_holder_name: account_holder,
                        account_holder_iban: Some(iban.clone()),
                        emandate: None,
                    },
                )))
            }
            _ => Err(IntegrationError::not_implemented(
                crate::utils::get_unimplemented_payment_method_error_message("multisafepay"),
            ))
            .attach_printable("Payment method not supported")?,
        },
        _ => Ok(None),
    }
}

// ===== STATUS ENUMS =====

/// MultiSafepay payment status enum
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MultisafepayPaymentStatus {
    Completed,
    Declined,
    #[default]
    Initialized,
    Void,
    Uncleared,
}

impl From<MultisafepayPaymentStatus> for AttemptStatus {
    fn from(status: MultisafepayPaymentStatus) -> Self {
        match status {
            MultisafepayPaymentStatus::Completed => Self::Charged,
            MultisafepayPaymentStatus::Declined => Self::Failure,
            MultisafepayPaymentStatus::Initialized => Self::AuthenticationPending,
            MultisafepayPaymentStatus::Uncleared => Self::Pending,
            MultisafepayPaymentStatus::Void => Self::Voided,
        }
    }
}

/// MultiSafepay refund status enum
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum MultisafepayRefundStatus {
    Succeeded,
    Failed,
    #[default]
    Processing,
}

impl From<MultisafepayRefundStatus> for RefundStatus {
    fn from(status: MultisafepayRefundStatus) -> Self {
        match status {
            MultisafepayRefundStatus::Succeeded => Self::Success,
            MultisafepayRefundStatus::Failed => Self::Failure,
            MultisafepayRefundStatus::Processing => Self::Pending,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MultisafepayAuthType {
    pub api_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for MultisafepayAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Multisafepay { api_key, .. } => Ok(Self {
                api_key: api_key.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultisafepayErrorResponse {
    pub success: bool,
    pub data: Option<MultisafepayErrorData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultisafepayErrorData {
    pub error_code: Option<i32>,
    pub error_info: Option<String>,
}

// ===== DIRECT TRANSACTION STRUCTURES =====

#[derive(Debug, Serialize)]
pub struct PaymentOptions {
    pub redirect_url: String,
    pub cancel_url: String,
}

#[derive(Debug, Serialize)]
pub struct CustomerInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct GatewayInfo<T: PaymentMethodDataTypes> {
    pub card_number: RawCardNumber<T>,
    pub card_expiry_date: i64, // Format: YYMM as integer
    pub card_cvc: Secret<String>,
    pub card_holder_name: Option<Secret<String>>,
    pub flexible_3d: Option<bool>,
    pub moto: Option<bool>,
    pub term_url: Option<String>,
}

/// Gateway info for SEPA Direct Debit transactions
#[derive(Debug, Serialize)]
pub struct DirectDebitGatewayInfo {
    pub account_id: Secret<String>,
    pub account_holder_name: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_holder_iban: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emandate: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DeliveryObject {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address1: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub house_number: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zip_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<common_enums::CountryAlpha2>,
}

// ===== PAYMENT REQUEST STRUCTURES =====

/// Unified gateway info enum that can hold either card or direct debit data
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum MultisafepayGatewayInfo<T: PaymentMethodDataTypes> {
    Card(GatewayInfo<T>),
    DirectDebit(DirectDebitGatewayInfo),
}

#[derive(Debug, Serialize)]
pub struct MultisafepayPaymentsRequest<T: PaymentMethodDataTypes> {
    #[serde(rename = "type")]
    pub order_type: Type,
    pub order_id: String,
    pub gateway: Gateway,
    pub currency: common_enums::Currency,
    pub amount: MinorUnit,
    pub description: String,
    // Required fields for direct transactions
    pub payment_options: PaymentOptions,
    pub customer: CustomerInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway_info: Option<MultisafepayGatewayInfo<T>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivery: Option<DeliveryObject>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub days_active: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seconds_active: Option<i32>,
}

// Implementation for macro-generated wrapper type
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        crate::connectors::multisafepay::MultisafepayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for MultisafepayPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: crate::connectors::multisafepay::MultisafepayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        use error_stack::ResultExt;

        let item = &wrapper.router_data;
        let order_type = get_order_type_from_payment_method(&item.request.payment_method_data)?;
        let gateway = get_gateway_from_payment_method(&item.request.payment_method_data)?;

        // Build gateway_info based on payment method type
        let gateway_info = build_gateway_info(&order_type, &item.request.payment_method_data)?;

        // Build customer info
        let customer = CustomerInfo {
            locale: None,
            ip_address: None,
            reference: Some(
                item.resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            email: item
                .request
                .email
                .clone()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "email",
                    context: Default::default(),
                })
                .attach_printable("Missing email for transaction")?
                .expose()
                .expose(),
        };

        // Build payment_options
        let payment_options = PaymentOptions {
            redirect_url: item
                .request
                .router_return_url
                .clone()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "router_return_url",
                    context: Default::default(),
                })
                .attach_printable("Missing return URL for transaction")?,
            cancel_url: item
                .request
                .router_return_url
                .clone()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "router_return_url",
                    context: Default::default(),
                })
                .attach_printable("Missing cancel URL for transaction")?,
        };

        // Build delivery object from billing address if available
        let delivery = item
            .resource_common_data
            .get_billing()
            .ok()
            .and_then(|billing| billing.address.as_ref())
            .map(|address| DeliveryObject {
                first_name: address.get_optional_first_name(),
                last_name: address.get_optional_last_name(),
                address1: address.line1.clone(),
                house_number: address.get_optional_line2(),
                zip_code: address.zip.clone(),
                city: address.city.clone().map(|c| c.expose()),
                country: address.get_optional_country(),
            });

        Ok(Self {
            order_type,
            order_id: item
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            gateway,
            currency: item.request.currency,
            amount: item.request.minor_amount,
            description: item.resource_common_data.get_description()?,
            payment_options,
            customer,
            gateway_info,
            delivery,
            days_active: Some(30),
            seconds_active: Some(259200),
        })
    }
}

// Keep the original implementation for backwards compatibility
impl<T: PaymentMethodDataTypes>
    TryFrom<
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    > for MultisafepayPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        use error_stack::ResultExt;

        let order_type = get_order_type_from_payment_method(&item.request.payment_method_data)?;
        let gateway = get_gateway_from_payment_method(&item.request.payment_method_data)?;

        // Build gateway_info based on payment method type
        let gateway_info = build_gateway_info(&order_type, &item.request.payment_method_data)?;

        // Build customer info
        let customer = CustomerInfo {
            locale: None,
            ip_address: None,
            reference: Some(
                item.resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            email: item
                .request
                .email
                .clone()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "email",
                    context: Default::default(),
                })
                .attach_printable("Missing email for transaction")?
                .expose()
                .expose(),
        };

        // Build payment_options
        let payment_options = PaymentOptions {
            redirect_url: item
                .request
                .router_return_url
                .clone()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "router_return_url",
                    context: Default::default(),
                })
                .attach_printable("Missing return URL for transaction")?,
            cancel_url: item
                .request
                .router_return_url
                .clone()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "router_return_url",
                    context: Default::default(),
                })
                .attach_printable("Missing cancel URL for transaction")?,
        };

        // Build delivery object from billing address if available
        let delivery = item
            .resource_common_data
            .get_billing()
            .ok()
            .and_then(|billing| billing.address.as_ref())
            .map(|address| DeliveryObject {
                first_name: address.get_optional_first_name(),
                last_name: address.get_optional_last_name(),
                address1: address.line1.clone(),
                house_number: address.get_optional_line2(),
                zip_code: address.zip.clone(),
                city: address.city.clone().map(|c| c.expose()),
                country: address.get_optional_country(),
            });

        Ok(Self {
            order_type,
            order_id: item
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            gateway,
            currency: item.request.currency,
            amount: item.request.minor_amount,
            description: item.resource_common_data.get_description()?,
            payment_options,
            customer,
            gateway_info,
            delivery,
            days_active: Some(30),
            seconds_active: Some(259200),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MultisafepayPaymentsResponse {
    pub success: bool,
    pub data: MultisafepayResponseData,
}

// Type aliases for different flows to avoid duplicate templating structs in macros
pub type MultisafepayPaymentsSyncResponse = MultisafepayPaymentsResponse;
pub type MultisafepayRefundSyncResponse = MultisafepayRefundResponse;

#[derive(Debug, Deserialize, Serialize)]
pub struct MultisafepayResponseData {
    #[serde(default)]
    pub order_id: Option<String>,
    pub payment_url: Option<String>,
    // transaction_id can be either a string or integer in different responses
    #[serde(deserialize_with = "deserialize_transaction_id", default)]
    pub transaction_id: Option<String>,
    #[serde(default)]
    pub status: MultisafepayPaymentStatus,
    pub amount: Option<MinorUnit>,
    pub currency: Option<common_enums::Currency>,
    // Additional fields that may appear in GET response - using flatten to ignore unknown fields
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

// Custom deserializer to handle transaction_id as either string or integer
fn deserialize_transaction_id<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let value: Option<serde_json::Value> = Option::deserialize(deserializer)?;
    Ok(value.and_then(|v| match v {
        serde_json::Value::String(s) => Some(s),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }))
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<MultisafepayPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<MultisafepayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response_data = &item.response.data;

        let status = response_data.status.clone().into();

        let redirection_data = response_data.payment_url.as_ref().map(|url| {
            Box::new(domain_types::router_response_types::RedirectForm::Uri { uri: url.clone() })
        });

        let transaction_id = response_data
            .transaction_id
            .clone()
            .or_else(|| response_data.order_id.clone())
            .unwrap_or_else(|| "unknown".to_string());

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(transaction_id),
                redirection_data,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: response_data.order_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// PSync Response Transformer - Reuses MultisafepayPaymentsResponse structure
impl TryFrom<ResponseRouterData<MultisafepayPaymentsResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<MultisafepayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response_data = &item.response.data;

        let status = response_data.status.clone().into();

        let transaction_id = response_data
            .transaction_id
            .clone()
            .or_else(|| response_data.order_id.clone())
            .unwrap_or_else(|| "unknown".to_string());

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(transaction_id),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: response_data.order_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== CAPTURE FLOW STRUCTURES =====
// Capture flow not implemented - MultiSafepay doesn't support capture
// (requires manual capture support which MultiSafepay doesn't provide)

// ===== REFUND FLOW STRUCTURES =====

#[derive(Debug, Serialize)]
pub struct MultisafepayRefundRequest {
    pub currency: common_enums::Currency,
    pub amount: MinorUnit,
}

// Implementation for macro-generated wrapper type
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        crate::connectors::multisafepay::MultisafepayRouterData<
            RouterDataV2<
                domain_types::connector_flow::Refund,
                RefundFlowData,
                RefundsData,
                RefundsResponseData,
            >,
            T,
        >,
    > for MultisafepayRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: crate::connectors::multisafepay::MultisafepayRouterData<
            RouterDataV2<
                domain_types::connector_flow::Refund,
                RefundFlowData,
                RefundsData,
                RefundsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &wrapper.router_data;
        Ok(Self {
            currency: item.request.currency,
            amount: item.request.minor_refund_amount,
        })
    }
}

// Keep the original implementation for backwards compatibility
impl<F> TryFrom<&RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>>
    for MultisafepayRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            currency: item.request.currency,
            amount: item.request.minor_refund_amount,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MultisafepayRefundResponse {
    pub success: bool,
    pub data: MultisafepayRefundData,
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct MultisafepayRefundData {
    pub transaction_id: i64,
    pub refund_id: i64,
    pub order_id: Option<String>,
    pub error_code: Option<i32>,
    pub error_info: Option<String>,
}

impl<F> TryFrom<ResponseRouterData<MultisafepayRefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<MultisafepayRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let refund_status = if item.response.success {
            MultisafepayRefundStatus::Succeeded
        } else {
            MultisafepayRefundStatus::Failed
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.data.refund_id.to_string(),
                refund_status: refund_status.into(),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// Refund Sync Response - Uses MultisafepayRefundResponse
impl TryFrom<ResponseRouterData<MultisafepayRefundResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<MultisafepayRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let refund_status = if item.response.success {
            MultisafepayRefundStatus::Succeeded
        } else {
            MultisafepayRefundStatus::Failed
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.data.refund_id.to_string(),
                refund_status: refund_status.into(),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// ===== RECURRING MODEL ENUM =====

/// MultiSafepay recurring model types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RecurringModel {
    CardOnFile,
    Subscription,
    Unscheduled,
}

// ===== SETUP MANDATE (CIT INITIAL) =====

/// SetupMandate request - initial CIT transaction that tokenizes card for recurring
#[derive(Debug, Serialize)]
pub struct MultisafepaySetupMandateRequest<T: PaymentMethodDataTypes> {
    #[serde(rename = "type")]
    pub order_type: Type,
    pub order_id: String,
    pub gateway: Gateway,
    pub currency: common_enums::Currency,
    pub amount: MinorUnit,
    pub description: String,
    pub payment_options: PaymentOptions,
    pub customer: CustomerInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway_info: Option<MultisafepayGatewayInfo<T>>,
    pub recurring_model: RecurringModel,
    pub recurring_id: String,
}

/// SetupMandate response reuses the standard payments response
pub type MultisafepaySetupMandateResponse = MultisafepayPaymentsResponse;

// TryFrom for SetupMandate request - wrapper type from macro
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        crate::connectors::multisafepay::MultisafepayRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for MultisafepaySetupMandateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: crate::connectors::multisafepay::MultisafepayRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        use error_stack::ResultExt;

        let item = &wrapper.router_data;
        let order_type = get_order_type_from_payment_method(&item.request.payment_method_data)?;
        let gateway = get_gateway_from_payment_method(&item.request.payment_method_data)?;
        let gateway_info = build_gateway_info(&order_type, &item.request.payment_method_data)?;

        let recurring_id = item
            .resource_common_data
            .connector_request_reference_id
            .clone();

        let customer = CustomerInfo {
            locale: None,
            ip_address: None,
            reference: Some(recurring_id.clone()),
            email: item
                .request
                .email
                .clone()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "email",
                    context: Default::default(),
                })
                .attach_printable("Missing email for setup mandate transaction")?
                .expose()
                .expose(),
        };

        let payment_options = PaymentOptions {
            redirect_url: item
                .request
                .router_return_url
                .clone()
                .unwrap_or_else(|| "https://example.com/return".to_string()),
            cancel_url: item
                .request
                .router_return_url
                .clone()
                .unwrap_or_else(|| "https://example.com/cancel".to_string()),
        };

        // Use zero amount for mandate setup if no amount specified
        let amount = item.request.minor_amount.unwrap_or(MinorUnit::new(0));

        let description = item
            .resource_common_data
            .get_description()
            .unwrap_or_else(|_| "Setup recurring payment".to_string());

        Ok(Self {
            order_type,
            order_id: item
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            gateway,
            currency: item.request.currency,
            amount,
            description,
            payment_options,
            customer,
            gateway_info,
            recurring_model: RecurringModel::Unscheduled,
            recurring_id,
        })
    }
}

// SetupMandate response transformer
impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<MultisafepaySetupMandateResponse, Self>>
    for RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<MultisafepaySetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response_data = &item.response.data;
        let status = response_data.status.clone().into();

        let redirection_data = response_data.payment_url.as_ref().map(|url| {
            Box::new(domain_types::router_response_types::RedirectForm::Uri { uri: url.clone() })
        });

        let transaction_id = response_data
            .transaction_id
            .clone()
            .or_else(|| response_data.order_id.clone())
            .unwrap_or_else(|| "unknown".to_string());

        // The recurring_id (customer reference) is the mandate reference for MultiSafepay
        // It's used to identify the stored token for subsequent MIT payments
        let mandate_reference = response_data
            .order_id
            .as_ref()
            .map(|order_id| {
                Box::new(connector_types::MandateReference {
                    connector_mandate_id: Some(order_id.clone()),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                })
            });

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(transaction_id),
                redirection_data,
                mandate_reference,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: response_data.order_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== REPEAT PAYMENT (MIT SUBSEQUENT) =====

/// RepeatPayment request - MIT subsequent transaction using stored token
#[derive(Debug, Serialize)]
pub struct MultisafepayRepeatPaymentRequest {
    #[serde(rename = "type")]
    pub order_type: Type,
    pub order_id: String,
    pub gateway: Gateway,
    pub currency: common_enums::Currency,
    pub amount: MinorUnit,
    pub description: String,
    pub recurring_model: RecurringModel,
    pub recurring_id: String,
    pub customer: CustomerInfo,
}

/// RepeatPayment response reuses the standard payments response
pub type MultisafepayRepeatPaymentResponse = MultisafepayPaymentsResponse;

// TryFrom for RepeatPayment request - wrapper type from macro
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        crate::connectors::multisafepay::MultisafepayRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for MultisafepayRepeatPaymentRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: crate::connectors::multisafepay::MultisafepayRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        use error_stack::ResultExt;

        let item = &wrapper.router_data;

        // Extract the connector_mandate_id which holds the recurring_id from SetupMandate
        let recurring_id = item
            .request
            .connector_mandate_id()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "connector_mandate_id (recurring_id)",
                context: Default::default(),
            })
            .attach_printable(
                "connector_mandate_id is required for MIT subsequent payment on MultiSafepay",
            )?;

        // Determine gateway from payment method data
        let gateway = get_gateway_from_payment_method(&item.request.payment_method_data)?;

        let customer = CustomerInfo {
            locale: None,
            ip_address: None,
            reference: Some(recurring_id.clone()),
            email: item
                .request
                .get_optional_email()
                .map(|e| e.expose().expose())
                .unwrap_or_else(|| "noreply@example.com".to_string()),
        };

        let description = item
            .resource_common_data
            .get_description()
            .unwrap_or_else(|_| "Recurring payment".to_string());

        Ok(Self {
            order_type: Type::Direct,
            order_id: item
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            gateway,
            currency: item.request.currency,
            amount: item.request.minor_amount,
            description,
            recurring_model: RecurringModel::Unscheduled,
            recurring_id,
            customer,
        })
    }
}

// RepeatPayment response transformer
impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<MultisafepayRepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<MultisafepayRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response_data = &item.response.data;
        let status = response_data.status.clone().into();

        let transaction_id = response_data
            .transaction_id
            .clone()
            .or_else(|| response_data.order_id.clone())
            .unwrap_or_else(|| "unknown".to_string());

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(transaction_id),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: response_data.order_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== VOID FLOW STRUCTURES =====
// Void flow not implemented - MultiSafepay doesn't support void
// (requires manual capture support which MultiSafepay doesn't provide)
