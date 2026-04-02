use common_utils::{request::Method, CustomerId, Email, StringMajorUnit};
use domain_types::{
    connector_flow::{Authorize, Capture, Refund, RepeatPayment, SetupMandate, Void},
    connector_types::{
        MandateIds, MandateReference, PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundsData,
        RefundsResponseData, RepeatPaymentData, ResponseId, SetupMandateRequestData,
    },
    errors::ConnectorError,
    payment_method_data::{
        BankRedirectData, Card, NetworkTokenData, PayLaterData, PaymentMethodData,
        PaymentMethodDataTypes, RawCardNumber, WalletData,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::{self, report, ResultExt};
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::str::FromStr;
use url::Url;

use super::aci_result_codes::{FAILURE_CODES, PENDING_CODES, SUCCESSFUL_CODES};
use super::AciRouterData;
use crate::{types::ResponseRouterData, utils};

type Error = error_stack::Report<ConnectorError>;

trait GetCaptureMethod {
    fn get_capture_method(&self) -> Option<common_enums::CaptureMethod>;
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> GetCaptureMethod
    for PaymentsAuthorizeData<T>
{
    fn get_capture_method(&self) -> Option<common_enums::CaptureMethod> {
        self.capture_method
    }
}

impl GetCaptureMethod for PaymentsSyncData {
    fn get_capture_method(&self) -> Option<common_enums::CaptureMethod> {
        self.capture_method
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> GetCaptureMethod
    for RepeatPaymentData<T>
{
    fn get_capture_method(&self) -> Option<common_enums::CaptureMethod> {
        self.capture_method
    }
}

impl GetCaptureMethod for PaymentVoidData {
    fn get_capture_method(&self) -> Option<common_enums::CaptureMethod> {
        None
    }
}
pub struct AciAuthType {
    pub api_key: Secret<String>,
    pub entity_id: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for AciAuthType {
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        if let ConnectorSpecificConfig::Aci {
            api_key, entity_id, ..
        } = item
        {
            Ok(Self {
                api_key: api_key.to_owned(),
                entity_id: entity_id.to_owned(),
            })
        } else {
            Err(ConnectorError::FailedToObtainAuthType)?
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum AciRecurringType {
    Initial,
    Repeated,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AciPaymentsRequest<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
{
    #[serde(flatten)]
    pub txn_details: TransactionDetails,
    #[serde(flatten)]
    pub payment_method: PaymentDetails<T>,
    #[serde(flatten)]
    pub instruction: Option<Instruction>,
    pub shopper_result_url: Option<String>,
    #[serde(rename = "customParameters[3DS2_enrolled]")]
    pub three_ds_two_enrolled: Option<bool>,
    pub recurring_type: Option<AciRecurringType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionDetails {
    pub entity_id: Secret<String>,
    pub amount: StringMajorUnit,
    pub currency: String,
    pub payment_type: AciPaymentType,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AciCancelRequest {
    pub entity_id: Secret<String>,
    pub payment_type: AciPaymentType,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AciMandateRequest<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
{
    pub entity_id: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_brand: Option<PaymentBrand>,
    #[serde(flatten)]
    pub payment_details: PaymentDetails<T>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AciMandateResponse {
    pub id: String,
    pub result: ResultCode,
    pub build_number: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum PaymentDetails<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> {
    #[serde(rename = "card")]
    AciCard(Box<CardDetails<T>>),
    BankRedirect(Box<BankRedirectionPMData>),
    Wallet(Box<WalletPMData>),
    Klarna,
    Mandate,
    AciNetworkToken(Box<AciNetworkTokenData>),
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        &WalletData,
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    )> for PaymentDetails<T>
{
    type Error = Error;
    fn try_from(
        value: (
            &WalletData,
            &RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        ),
    ) -> Result<Self, Self::Error> {
        let (wallet_data, item) = value;
        let payment_data = match wallet_data {
            WalletData::MbWayRedirect(_) => {
                let phone_details = item.resource_common_data.get_billing_phone()?;
                Self::Wallet(Box::new(WalletPMData {
                    payment_brand: PaymentBrand::Mbway,
                    account_id: Some(phone_details.get_number_with_hash_country_code()?),
                }))
            }
            WalletData::AliPayRedirect { .. } => Self::Wallet(Box::new(WalletPMData {
                payment_brand: PaymentBrand::AliPay,
                account_id: None,
            })),
            WalletData::AliPayHkRedirect(_)
            | WalletData::AmazonPayRedirect(_)
            | WalletData::MomoRedirect(_)
            | WalletData::KakaoPayRedirect(_)
            | WalletData::GoPayRedirect(_)
            | WalletData::GcashRedirect(_)
            | WalletData::ApplePay(_)
            | WalletData::ApplePayThirdPartySdk(_)
            | WalletData::DanaRedirect { .. }
            | WalletData::GooglePay(_)
            | WalletData::BluecodeRedirect {}
            | WalletData::GooglePayThirdPartySdk(_)
            | WalletData::MobilePayRedirect(_)
            | WalletData::PaypalRedirect(_)
            | WalletData::PaypalSdk(_)
            | WalletData::Paze(_)
            | WalletData::SamsungPay(_)
            | WalletData::TwintRedirect { .. }
            | WalletData::VippsRedirect { .. }
            | WalletData::TouchNGoRedirect(_)
            | WalletData::WeChatPayRedirect(_)
            | WalletData::WeChatPayQr(_)
            | WalletData::CashappQr(_)
            | WalletData::SwishQr(_)
            | WalletData::AliPayQr(_)
            | WalletData::ApplePayRedirect(_)
            | WalletData::GooglePayRedirect(_)
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
            | WalletData::EaseBuzzRedirect(_)
            | WalletData::AmazonPayDirect(_) => {
                Err(ConnectorError::NotImplemented("Payment method".to_string()))?
            }
        };
        Ok(payment_data)
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        &AciRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
        &BankRedirectData,
    )> for PaymentDetails<T>
{
    type Error = Error;
    fn try_from(
        value: (
            &AciRouterData<
                RouterDataV2<
                    Authorize,
                    PaymentFlowData,
                    PaymentsAuthorizeData<T>,
                    PaymentsResponseData,
                >,
                T,
            >,
            &BankRedirectData,
        ),
    ) -> Result<Self, Self::Error> {
        let (item, bank_redirect_data) = value;
        let payment_data = match bank_redirect_data {
            BankRedirectData::Eps { .. } => Self::BankRedirect(Box::new(BankRedirectionPMData {
                payment_brand: PaymentBrand::Eps,
                bank_account_country: Some(
                    item.router_data
                        .resource_common_data
                        .get_billing_country()?,
                ),
                bank_account_bank_name: None,
                bank_account_bic: None,
                bank_account_iban: None,
                billing_country: None,
                merchant_customer_id: None,
                merchant_transaction_id: None,
                customer_email: None,
            })),
            BankRedirectData::Eft { .. } => Self::BankRedirect(Box::new(BankRedirectionPMData {
                payment_brand: PaymentBrand::Eft,
                bank_account_country: Some(
                    item.router_data
                        .resource_common_data
                        .get_billing_country()?,
                ),
                bank_account_bank_name: None,
                bank_account_bic: None,
                bank_account_iban: None,
                billing_country: None,
                merchant_customer_id: None,
                merchant_transaction_id: None,
                customer_email: None,
            })),
            BankRedirectData::Giropay {
                bank_account_bic,
                bank_account_iban,
                ..
            } => Self::BankRedirect(Box::new(BankRedirectionPMData {
                payment_brand: PaymentBrand::Giropay,
                bank_account_country: Some(
                    item.router_data
                        .resource_common_data
                        .get_billing_country()?,
                ),
                bank_account_bank_name: None,
                bank_account_bic: bank_account_bic.clone(),
                bank_account_iban: bank_account_iban.clone(),
                billing_country: None,
                merchant_customer_id: None,
                merchant_transaction_id: None,
                customer_email: None,
            })),
            BankRedirectData::Ideal { bank_name, .. } => {
                Self::BankRedirect(Box::new(BankRedirectionPMData {
                    payment_brand: PaymentBrand::Ideal,
                    bank_account_country: Some(
                        item.router_data
                            .resource_common_data
                            .get_billing_country()?,
                    ),
                    bank_account_bank_name: Some(bank_name.ok_or(
                        ConnectorError::MissingRequiredField {
                            field_name: "ideal.bank_name",
                        },
                    )?),
                    bank_account_bic: None,
                    bank_account_iban: None,
                    billing_country: None,
                    merchant_customer_id: None,
                    merchant_transaction_id: None,
                    customer_email: None,
                }))
            }
            BankRedirectData::Sofort { .. } => {
                Self::BankRedirect(Box::new(BankRedirectionPMData {
                    payment_brand: PaymentBrand::Sofortueberweisung,
                    bank_account_country: Some(
                        item.router_data
                            .resource_common_data
                            .get_billing_country()?,
                    ),
                    bank_account_bank_name: None,
                    bank_account_bic: None,
                    bank_account_iban: None,
                    billing_country: None,
                    merchant_customer_id: None,
                    merchant_transaction_id: None,
                    customer_email: None,
                }))
            }
            BankRedirectData::Przelewy24 { .. } => {
                Self::BankRedirect(Box::new(BankRedirectionPMData {
                    payment_brand: PaymentBrand::Przelewy,
                    bank_account_country: None,
                    bank_account_bank_name: None,
                    bank_account_bic: None,
                    bank_account_iban: None,
                    billing_country: None,
                    merchant_customer_id: None,
                    merchant_transaction_id: None,
                    customer_email: Some(
                        item.router_data.resource_common_data.get_billing_email()?,
                    ),
                }))
            }
            BankRedirectData::Interac { .. } => {
                Self::BankRedirect(Box::new(BankRedirectionPMData {
                    payment_brand: PaymentBrand::InteracOnline,
                    bank_account_country: Some(
                        item.router_data
                            .resource_common_data
                            .get_billing_country()?,
                    ),
                    bank_account_bank_name: None,
                    bank_account_bic: None,
                    bank_account_iban: None,
                    billing_country: None,
                    merchant_customer_id: None,
                    merchant_transaction_id: None,
                    customer_email: Some(
                        item.router_data.resource_common_data.get_billing_email()?,
                    ),
                }))
            }
            BankRedirectData::Trustly { .. } => {
                Self::BankRedirect(Box::new(BankRedirectionPMData {
                    payment_brand: PaymentBrand::Trustly,
                    bank_account_country: None,
                    bank_account_bank_name: None,
                    bank_account_bic: None,
                    bank_account_iban: None,
                    billing_country: Some(
                        item.router_data
                            .resource_common_data
                            .get_billing_country()?,
                    ),
                    merchant_customer_id: Some(Secret::new(
                        item.router_data.resource_common_data.get_customer_id()?,
                    )),
                    merchant_transaction_id: Some(Secret::new(
                        item.router_data
                            .resource_common_data
                            .connector_request_reference_id
                            .clone(),
                    )),
                    customer_email: None,
                }))
            }
            BankRedirectData::Bizum { .. }
            | BankRedirectData::Blik { .. }
            | BankRedirectData::BancontactCard { .. }
            | BankRedirectData::OnlineBankingCzechRepublic { .. }
            | BankRedirectData::OnlineBankingFinland { .. }
            | BankRedirectData::OnlineBankingFpx { .. }
            | BankRedirectData::OnlineBankingPoland { .. }
            | BankRedirectData::OnlineBankingSlovakia { .. }
            | BankRedirectData::OnlineBankingThailand { .. }
            | BankRedirectData::LocalBankRedirect {}
            | BankRedirectData::OpenBankingUk { .. }
            | BankRedirectData::OpenBanking {} => {
                Err(ConnectorError::NotImplemented("Payment method".to_string()))?
            }
        };
        Ok(payment_data)
    }
}

fn get_aci_payment_brand(
    card_network: Option<common_enums::CardNetwork>,
    is_network_token_flow: bool,
) -> Result<PaymentBrand, Error> {
    match card_network {
        Some(common_enums::CardNetwork::Visa) => Ok(PaymentBrand::Visa),
        Some(common_enums::CardNetwork::Mastercard) => Ok(PaymentBrand::Mastercard),
        Some(common_enums::CardNetwork::AmericanExpress) => Ok(PaymentBrand::AmericanExpress),
        Some(common_enums::CardNetwork::JCB) => Ok(PaymentBrand::Jcb),
        Some(common_enums::CardNetwork::DinersClub) => Ok(PaymentBrand::DinersClub),
        Some(common_enums::CardNetwork::Discover) => Ok(PaymentBrand::Discover),
        Some(common_enums::CardNetwork::UnionPay) => Ok(PaymentBrand::UnionPay),
        Some(common_enums::CardNetwork::Maestro) => Ok(PaymentBrand::Maestro),
        Some(unsupported_network) => Err(ConnectorError::NotSupported {
            message: format!("Card network {unsupported_network} is not supported by ACI"),
            connector: "ACI",
        })?,
        None => {
            if is_network_token_flow {
                Ok(PaymentBrand::Visa)
            } else {
                Err(ConnectorError::MissingRequiredField {
                    field_name: "card.card_network",
                }
                .into())
            }
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<(Card<T>, Option<Secret<String>>)> for PaymentDetails<T>
{
    type Error = Error;
    fn try_from(
        (card_data, card_holder_name): (Card<T>, Option<Secret<String>>),
    ) -> Result<Self, Self::Error> {
        let card_expiry_year = card_data.get_expiry_year_4_digit();

        let payment_brand = get_aci_payment_brand(card_data.card_network, false).ok();

        Ok(Self::AciCard(Box::new(CardDetails {
            card_number: card_data.card_number,
            card_holder: card_holder_name.ok_or(ConnectorError::MissingRequiredField {
                field_name: "billing_address.first_name",
            })?,
            card_expiry_month: card_data.card_exp_month.clone(),
            card_expiry_year,
            card_cvv: card_data.card_cvc,
            payment_brand,
        })))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        &AciRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
        &NetworkTokenData,
    )> for PaymentDetails<T>
{
    type Error = Error;
    fn try_from(
        value: (
            &AciRouterData<
                RouterDataV2<
                    Authorize,
                    PaymentFlowData,
                    PaymentsAuthorizeData<T>,
                    PaymentsResponseData,
                >,
                T,
            >,
            &NetworkTokenData,
        ),
    ) -> Result<Self, Self::Error> {
        let (_item, network_token_data) = value;
        let token_number = network_token_data.get_network_token();
        let payment_brand = get_aci_payment_brand(network_token_data.card_network.clone(), true)?;
        let aci_network_token_data = AciNetworkTokenData {
            token_type: AciTokenAccountType::Network,
            token_number,
            token_expiry_month: network_token_data.get_network_token_expiry_month(),
            token_expiry_year: network_token_data.get_expiry_year_4_digit(),
            token_cryptogram: Some(
                network_token_data
                    .get_cryptogram()
                    .clone()
                    .unwrap_or_default(),
            ),
            payment_brand,
        };
        Ok(Self::AciNetworkToken(Box::new(aci_network_token_data)))
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum AciTokenAccountType {
    Network,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AciNetworkTokenData {
    #[serde(rename = "tokenAccount.type")]
    pub token_type: AciTokenAccountType,
    #[serde(rename = "tokenAccount.number")]
    pub token_number: cards::NetworkToken,
    #[serde(rename = "tokenAccount.expiryMonth")]
    pub token_expiry_month: Secret<String>,
    #[serde(rename = "tokenAccount.expiryYear")]
    pub token_expiry_year: Secret<String>,
    #[serde(rename = "tokenAccount.cryptogram")]
    pub token_cryptogram: Option<Secret<String>>,
    #[serde(rename = "paymentBrand")]
    pub payment_brand: PaymentBrand,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BankRedirectionPMData {
    payment_brand: PaymentBrand,
    #[serde(rename = "bankAccount.country")]
    bank_account_country: Option<common_enums::CountryAlpha2>,
    #[serde(rename = "bankAccount.bankName")]
    bank_account_bank_name: Option<common_enums::BankNames>,
    #[serde(rename = "bankAccount.bic")]
    bank_account_bic: Option<Secret<String>>,
    #[serde(rename = "bankAccount.iban")]
    bank_account_iban: Option<Secret<String>>,
    #[serde(rename = "billing.country")]
    billing_country: Option<common_enums::CountryAlpha2>,
    #[serde(rename = "customer.email")]
    customer_email: Option<Email>,
    #[serde(rename = "customer.merchantCustomerId")]
    merchant_customer_id: Option<Secret<CustomerId>>,
    merchant_transaction_id: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletPMData {
    payment_brand: PaymentBrand,
    #[serde(rename = "virtualAccount.accountId")]
    account_id: Option<Secret<String>>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaymentBrand {
    Eps,
    Eft,
    Ideal,
    Giropay,
    Sofortueberweisung,
    InteracOnline,
    Przelewy,
    Trustly,
    Mbway,
    #[serde(rename = "ALIPAY")]
    AliPay,
    // Card network brands
    #[serde(rename = "VISA")]
    Visa,
    #[serde(rename = "MASTER")]
    Mastercard,
    #[serde(rename = "AMEX")]
    AmericanExpress,
    #[serde(rename = "JCB")]
    Jcb,
    #[serde(rename = "DINERS")]
    DinersClub,
    #[serde(rename = "DISCOVER")]
    Discover,
    #[serde(rename = "UNIONPAY")]
    UnionPay,
    #[serde(rename = "MAESTRO")]
    Maestro,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct CardDetails<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> {
    #[serde(rename = "card.number")]
    pub card_number: RawCardNumber<T>,
    #[serde(rename = "card.holder")]
    pub card_holder: Secret<String>,
    #[serde(rename = "card.expiryMonth")]
    pub card_expiry_month: Secret<String>,
    #[serde(rename = "card.expiryYear")]
    pub card_expiry_year: Secret<String>,
    #[serde(rename = "card.cvv")]
    pub card_cvv: Secret<String>,
    #[serde(rename = "paymentBrand")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_brand: Option<PaymentBrand>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum InstructionMode {
    Initial,
    Repeated,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum InstructionType {
    Unscheduled,
}

#[derive(Debug, Clone, Serialize)]
pub enum InstructionSource {
    #[serde(rename = "CIT")]
    CardholderInitiatedTransaction,
    #[serde(rename = "MIT")]
    MerchantInitiatedTransaction,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Instruction {
    #[serde(rename = "standingInstruction.mode")]
    mode: InstructionMode,

    #[serde(rename = "standingInstruction.type")]
    transaction_type: InstructionType,

    #[serde(rename = "standingInstruction.source")]
    source: InstructionSource,

    create_registration: Option<bool>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct BankDetails {
    #[serde(rename = "bankAccount.holder")]
    pub account_holder: Secret<String>,
}

#[allow(dead_code)]
#[derive(Debug, Default, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum AciPaymentType {
    #[serde(rename = "PA")]
    Preauthorization,
    #[default]
    #[serde(rename = "DB")]
    Debit,
    #[serde(rename = "CD")]
    Credit,
    #[serde(rename = "CP")]
    Capture,
    #[serde(rename = "RV")]
    Reversal,
    #[serde(rename = "RF")]
    Refund,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AciRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AciPaymentsRequest<T>
{
    type Error = Error;
    fn try_from(
        item: AciRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        match item.router_data.request.payment_method_data.clone() {
            PaymentMethodData::Card(ref card_data) => Self::try_from((&item, card_data)),
            PaymentMethodData::NetworkToken(ref network_token_data) => {
                Self::try_from((&item, network_token_data))
            }
            PaymentMethodData::Wallet(ref wallet_data) => Self::try_from((&item, wallet_data)),
            PaymentMethodData::PayLater(ref pay_later_data) => {
                Self::try_from((&item, pay_later_data))
            }
            PaymentMethodData::BankRedirect(ref bank_redirect_data) => {
                Self::try_from((&item, bank_redirect_data))
            }
            PaymentMethodData::MandatePayment => {
                let mandate_id = item.router_data.request.mandate_id.clone().ok_or(
                    ConnectorError::MissingRequiredField {
                        field_name: "mandate_id",
                    },
                )?;
                Self::try_from((&item, mandate_id))
            }
            PaymentMethodData::Crypto(_)
            | PaymentMethodData::BankDebit(_)
            | PaymentMethodData::BankTransfer(_)
            | PaymentMethodData::Reward
            | PaymentMethodData::RealTimePayment(_)
            | PaymentMethodData::MobilePayment(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::CardToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
                Err(ConnectorError::NotImplemented(
                    utils::get_unimplemented_payment_method_error_message("Aci"),
                ))?
            }
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        &AciRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
        &WalletData,
    )> for AciPaymentsRequest<T>
{
    type Error = Error;
    fn try_from(
        value: (
            &AciRouterData<
                RouterDataV2<
                    Authorize,
                    PaymentFlowData,
                    PaymentsAuthorizeData<T>,
                    PaymentsResponseData,
                >,
                T,
            >,
            &WalletData,
        ),
    ) -> Result<Self, Self::Error> {
        let (item, wallet_data) = value;
        let txn_details = get_transaction_details(item)?;
        let payment_method = PaymentDetails::try_from((wallet_data, &item.router_data))?;

        Ok(Self {
            txn_details,
            payment_method,
            instruction: None,
            shopper_result_url: item.router_data.request.router_return_url.clone(),
            three_ds_two_enrolled: None,
            recurring_type: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        &AciRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
        &BankRedirectData,
    )> for AciPaymentsRequest<T>
{
    type Error = Error;
    fn try_from(
        value: (
            &AciRouterData<
                RouterDataV2<
                    Authorize,
                    PaymentFlowData,
                    PaymentsAuthorizeData<T>,
                    PaymentsResponseData,
                >,
                T,
            >,
            &BankRedirectData,
        ),
    ) -> Result<Self, Self::Error> {
        let (item, bank_redirect_data) = value;
        let txn_details = get_transaction_details(item)?;
        let payment_method = PaymentDetails::try_from((item, bank_redirect_data))?;

        Ok(Self {
            txn_details,
            payment_method,
            instruction: None,
            shopper_result_url: item.router_data.request.router_return_url.clone(),
            three_ds_two_enrolled: None,
            recurring_type: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        &AciRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
        &PayLaterData,
    )> for AciPaymentsRequest<T>
{
    type Error = Error;
    fn try_from(
        value: (
            &AciRouterData<
                RouterDataV2<
                    Authorize,
                    PaymentFlowData,
                    PaymentsAuthorizeData<T>,
                    PaymentsResponseData,
                >,
                T,
            >,
            &PayLaterData,
        ),
    ) -> Result<Self, Self::Error> {
        let (item, _pay_later_data) = value;
        let txn_details = get_transaction_details(item)?;
        let payment_method = PaymentDetails::Klarna;

        Ok(Self {
            txn_details,
            payment_method,
            instruction: None,
            shopper_result_url: item.router_data.request.router_return_url.clone(),
            three_ds_two_enrolled: None,
            recurring_type: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        &AciRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
        &Card<T>,
    )> for AciPaymentsRequest<T>
{
    type Error = Error;
    fn try_from(
        value: (
            &AciRouterData<
                RouterDataV2<
                    Authorize,
                    PaymentFlowData,
                    PaymentsAuthorizeData<T>,
                    PaymentsResponseData,
                >,
                T,
            >,
            &Card<T>,
        ),
    ) -> Result<Self, Self::Error> {
        let (item, card_data) = value;
        let card_holder_name = item
            .router_data
            .resource_common_data
            .get_optional_billing_full_name();
        let txn_details = get_transaction_details(item)?;
        let payment_method = PaymentDetails::try_from((card_data.clone(), card_holder_name))?;
        let instruction = get_instruction_details(item);
        let recurring_type = get_recurring_type(item);
        let three_ds_two_enrolled = item
            .router_data
            .resource_common_data
            .is_three_ds()
            .then_some(item.router_data.request.enrolled_for_3ds)
            .flatten();

        Ok(Self {
            txn_details,
            payment_method,
            instruction,
            shopper_result_url: item.router_data.request.router_return_url.clone(),
            three_ds_two_enrolled,
            recurring_type,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        &AciRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
        &NetworkTokenData,
    )> for AciPaymentsRequest<T>
{
    type Error = Error;
    fn try_from(
        value: (
            &AciRouterData<
                RouterDataV2<
                    Authorize,
                    PaymentFlowData,
                    PaymentsAuthorizeData<T>,
                    PaymentsResponseData,
                >,
                T,
            >,
            &NetworkTokenData,
        ),
    ) -> Result<Self, Self::Error> {
        let (item, network_token_data) = value;
        let txn_details = get_transaction_details(item)?;
        let payment_method = PaymentDetails::try_from((item, network_token_data))?;
        let instruction = get_instruction_details(item);

        Ok(Self {
            txn_details,
            payment_method,
            instruction,
            shopper_result_url: item.router_data.request.router_return_url.clone(),
            three_ds_two_enrolled: None,
            recurring_type: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        &AciRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
        MandateIds,
    )> for AciPaymentsRequest<T>
{
    type Error = Error;
    fn try_from(
        value: (
            &AciRouterData<
                RouterDataV2<
                    Authorize,
                    PaymentFlowData,
                    PaymentsAuthorizeData<T>,
                    PaymentsResponseData,
                >,
                T,
            >,
            MandateIds,
        ),
    ) -> Result<Self, Self::Error> {
        let (item, _mandate_data) = value;
        let instruction = get_instruction_details(item);
        let txn_details = get_transaction_details(item)?;
        let recurring_type = get_recurring_type(item);

        Ok(Self {
            txn_details,
            payment_method: PaymentDetails::Mandate,
            instruction,
            shopper_result_url: item.router_data.request.router_return_url.clone(),
            three_ds_two_enrolled: None,
            recurring_type,
        })
    }
}

fn get_transaction_details<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
>(
    item: &AciRouterData<
        RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        T,
    >,
) -> Result<TransactionDetails, error_stack::Report<ConnectorError>> {
    let auth = AciAuthType::try_from(&item.router_data.connector_config)?;
    let amount = item
        .connector
        .amount_converter
        .convert(
            item.router_data.request.minor_amount,
            item.router_data.request.currency,
        )
        .change_context(ConnectorError::AmountConversionFailed)?;
    let payment_type = if item.router_data.request.is_auto_capture()? {
        AciPaymentType::Debit
    } else {
        AciPaymentType::Preauthorization
    };
    Ok(TransactionDetails {
        entity_id: auth.entity_id,
        amount,
        currency: item.router_data.request.currency.to_string(),
        payment_type,
    })
}

fn get_instruction_details<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
>(
    item: &AciRouterData<
        RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        T,
    >,
) -> Option<Instruction> {
    if item.router_data.request.customer_acceptance.is_some()
        && item.router_data.request.setup_future_usage
            == Some(common_enums::FutureUsage::OffSession)
    {
        return Some(Instruction {
            mode: InstructionMode::Initial,
            transaction_type: InstructionType::Unscheduled,
            source: InstructionSource::CardholderInitiatedTransaction,
            create_registration: Some(true),
        });
    } else if item.router_data.request.mandate_id.is_some() {
        return Some(Instruction {
            mode: InstructionMode::Repeated,
            transaction_type: InstructionType::Unscheduled,
            source: InstructionSource::MerchantInitiatedTransaction,
            create_registration: None,
        });
    }
    None
}

fn get_recurring_type<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>(
    item: &AciRouterData<
        RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        T,
    >,
) -> Option<AciRecurringType> {
    if item.router_data.request.customer_acceptance.is_some()
        && item.router_data.request.setup_future_usage
            == Some(common_enums::FutureUsage::OffSession)
    {
        Some(AciRecurringType::Initial)
    } else {
        None
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AciRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for AciCancelRequest
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: AciRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = AciAuthType::try_from(&item.router_data.connector_config)?;
        let aci_payment_request = Self {
            entity_id: auth.entity_id,
            payment_type: AciPaymentType::Reversal,
        };
        Ok(aci_payment_request)
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AciRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AciMandateRequest<T>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: AciRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = AciAuthType::try_from(&item.router_data.connector_config)?;

        let (payment_brand, payment_details) = match &item.router_data.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                let brand = get_aci_payment_brand(card_data.card_network.clone(), false).ok();
                match brand.as_ref() {
                    Some(PaymentBrand::Visa)
                    | Some(PaymentBrand::Mastercard)
                    | Some(PaymentBrand::AmericanExpress) => (),
                    Some(_) => {
                        return Err(ConnectorError::NotSupported {
                            message: "Payment method not supported for mandate setup".to_string(),
                            connector: "ACI",
                        }
                        .into());
                    }
                    None => (),
                };

                let details = PaymentDetails::AciCard(Box::new(CardDetails {
                    card_number: card_data.card_number.clone(),
                    card_expiry_month: card_data.card_exp_month.clone(),
                    card_expiry_year: card_data.get_expiry_year_4_digit(),
                    card_cvv: card_data.card_cvc.clone(),
                    card_holder: card_data.card_holder_name.clone().ok_or(
                        ConnectorError::MissingRequiredField {
                            field_name: "payment_method.card.card_holder_name",
                        },
                    )?,
                    payment_brand: brand.clone(),
                }));

                (brand, details)
            }
            _ => {
                return Err(ConnectorError::NotSupported {
                    message: "Payment method not supported for mandate setup".to_string(),
                    connector: "ACI",
                }
                .into());
            }
        };

        Ok(Self {
            entity_id: auth.entity_id,
            payment_brand,
            payment_details,
        })
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AciPaymentStatus {
    Succeeded,
    Failed,
    #[default]
    Pending,
    RedirectShopper,
}

fn map_aci_attempt_status(
    item: AciPaymentStatus,
    auto_capture: bool,
) -> common_enums::AttemptStatus {
    match item {
        AciPaymentStatus::Succeeded => {
            if auto_capture {
                common_enums::AttemptStatus::Charged
            } else {
                common_enums::AttemptStatus::Authorized
            }
        }
        AciPaymentStatus::Failed => common_enums::AttemptStatus::Failure,
        AciPaymentStatus::Pending => common_enums::AttemptStatus::Authorizing,
        AciPaymentStatus::RedirectShopper => common_enums::AttemptStatus::AuthenticationPending,
    }
}

impl FromStr for AciPaymentStatus {
    type Err = error_stack::Report<ConnectorError>;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if FAILURE_CODES.contains(&s) {
            Ok(Self::Failed)
        } else if PENDING_CODES.contains(&s) {
            Ok(Self::Pending)
        } else if SUCCESSFUL_CODES.contains(&s) {
            Ok(Self::Succeeded)
        } else {
            Err(report!(ConnectorError::UnexpectedResponseError(
                bytes::Bytes::from(s.to_owned())
            )))
        }
    }
}

#[derive(Debug, Default, Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AciPaymentsResponse {
    id: String,
    registration_id: Option<Secret<String>>,
    ndc: String,
    timestamp: String,
    build_number: String,
    pub(super) result: ResultCode,
    pub(super) redirect: Option<AciRedirectionData>,
}

#[derive(Debug, Default, Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AciErrorResponse {
    ndc: String,
    timestamp: String,
    build_number: String,
    pub(super) result: ResultCode,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AciRedirectionData {
    pub method: Option<Method>,
    pub parameters: Vec<Parameters>,
    pub url: Url,
    pub preconditions: Option<Vec<PreconditionData>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreconditionData {
    pub method: Option<Method>,
    pub parameters: Vec<Parameters>,
    pub url: Url,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Parameters {
    pub name: String,
    pub value: String,
}

#[derive(Default, Debug, Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultCode {
    pub(super) code: String,
    pub(super) description: String,
    pub(super) parameter_errors: Option<Vec<ErrorParameters>>,
}

#[derive(Default, Debug, Clone, Deserialize, PartialEq, Eq, Serialize)]
pub struct ErrorParameters {
    pub(super) name: String,
    pub(super) value: Option<String>,
    pub(super) message: String,
}

impl<F, Req> TryFrom<ResponseRouterData<AciPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, Req, PaymentsResponseData>
where
    Req: GetCaptureMethod,
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<AciPaymentsResponse, Self>) -> Result<Self, Self::Error> {
        let redirection_data = item.response.redirect.map(|data| {
            let mut form_fields = std::collections::HashMap::<_, _>::from_iter(
                data.parameters
                    .iter()
                    .map(|parameter| (parameter.name.clone(), parameter.value.clone())),
            );

            if let Some(preconditions) = data.preconditions {
                if let Some(first_precondition) = preconditions.first() {
                    for param in &first_precondition.parameters {
                        form_fields.insert(param.name.clone(), param.value.clone());
                    }
                }
            }

            // If method is Get, parameters are appended to URL
            // If method is post, we http Post the method to URL
            RedirectForm::Form {
                endpoint: data.url.to_string(),
                // Handles method for Bank redirects currently.
                // 3DS response have method within preconditions. That would require replacing below line with a function.
                method: data.method.unwrap_or(Method::Post),
                form_fields,
            }
        });

        let mandate_reference = item
            .response
            .registration_id
            .clone()
            .map(|id| MandateReference {
                connector_mandate_id: Some(id.expose()),
                payment_method_id: None,
                connector_mandate_request_reference_id: None,
            });

        let auto_capture = matches!(
            item.router_data.request.get_capture_method(),
            Some(common_enums::CaptureMethod::Automatic) | None
        );

        let status = if redirection_data.is_some() {
            map_aci_attempt_status(AciPaymentStatus::RedirectShopper, auto_capture)
        } else {
            map_aci_attempt_status(
                AciPaymentStatus::from_str(&item.response.result.code)?,
                auto_capture,
            )
        };

        let response = if status == common_enums::AttemptStatus::Failure {
            Err(ErrorResponse {
                code: item.response.result.code.clone(),
                message: item.response.result.description.clone(),
                reason: Some(item.response.result.description),
                status_code: item.http_code,
                attempt_status: Some(status),
                connector_transaction_id: Some(item.response.id.clone()),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            })
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: redirection_data.map(Box::new),
                mandate_reference: mandate_reference.map(Box::new),
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.id),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            })
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AciCaptureRequest {
    #[serde(flatten)]
    pub txn_details: TransactionDetails,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AciRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for AciCaptureRequest
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: AciRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = AciAuthType::try_from(&item.router_data.connector_config)?;
        let amount = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.minor_amount_to_capture,
                item.router_data.request.currency,
            )
            .change_context(ConnectorError::AmountConversionFailed)?;
        Ok(Self {
            txn_details: TransactionDetails {
                entity_id: auth.entity_id,
                amount,
                currency: item.router_data.request.currency.to_string(),
                payment_type: AciPaymentType::Capture,
            },
        })
    }
}

#[derive(Debug, Default, Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AciCaptureResponse {
    id: String,
    referenced_id: String,
    payment_type: AciPaymentType,
    amount: StringMajorUnit,
    currency: String,
    descriptor: String,
    result: AciCaptureResult,
    result_details: Option<AciCaptureResultDetails>,
    build_number: String,
    timestamp: String,
    ndc: Secret<String>,
    source: Option<Secret<String>>,
    payment_method: Option<String>,
    short_id: Option<String>,
}

#[derive(Debug, Default, Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AciCaptureResult {
    code: String,
    description: String,
}

#[derive(Debug, Default, Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct AciCaptureResultDetails {
    extended_description: String,
    #[serde(rename = "clearingInstituteName")]
    clearing_institute_name: Option<String>,
    connector_tx_i_d1: Option<String>,
    connector_tx_i_d3: Option<String>,
    connector_tx_i_d2: Option<String>,
    acquirer_response: Option<String>,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub enum AciStatus {
    Succeeded,
    Failed,
    #[default]
    Pending,
}

impl FromStr for AciStatus {
    type Err = error_stack::Report<ConnectorError>;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if FAILURE_CODES.contains(&s) {
            Ok(Self::Failed)
        } else if PENDING_CODES.contains(&s) {
            Ok(Self::Pending)
        } else if SUCCESSFUL_CODES.contains(&s) {
            Ok(Self::Succeeded)
        } else {
            Err(report!(ConnectorError::UnexpectedResponseError(
                bytes::Bytes::from(s.to_owned())
            )))
        }
    }
}

fn map_aci_capture_status(item: AciStatus) -> common_enums::AttemptStatus {
    match item {
        AciStatus::Succeeded => common_enums::AttemptStatus::Charged,
        AciStatus::Failed => common_enums::AttemptStatus::Failure,
        AciStatus::Pending => common_enums::AttemptStatus::Pending,
    }
}

impl<F, T> TryFrom<ResponseRouterData<AciCaptureResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<AciCaptureResponse, Self>) -> Result<Self, Self::Error> {
        let status = map_aci_capture_status(AciStatus::from_str(&item.response.result.code)?);
        let response = if status == common_enums::AttemptStatus::Failure {
            Err(ErrorResponse {
                code: item.response.result.code.clone(),
                message: item.response.result.description.clone(),
                reason: Some(item.response.result.description),
                status_code: item.http_code,
                attempt_status: Some(status),
                connector_transaction_id: Some(item.response.id.clone()),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            })
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.referenced_id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            })
        };
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}

#[derive(Debug, Default, Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AciVoidResponse {
    id: String,
    referenced_id: String,
    payment_type: AciPaymentType,
    amount: StringMajorUnit,
    currency: String,
    descriptor: String,
    result: AciCaptureResult,
    result_details: Option<AciCaptureResultDetails>,
    build_number: String,
    timestamp: String,
    ndc: Secret<String>,
}

fn map_aci_void_status(item: AciStatus) -> common_enums::AttemptStatus {
    match item {
        AciStatus::Succeeded => common_enums::AttemptStatus::Voided,
        AciStatus::Failed => common_enums::AttemptStatus::VoidFailed,
        AciStatus::Pending => common_enums::AttemptStatus::VoidInitiated,
    }
}

impl<F, T> TryFrom<ResponseRouterData<AciVoidResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<AciVoidResponse, Self>) -> Result<Self, Self::Error> {
        let status = map_aci_void_status(AciStatus::from_str(&item.response.result.code)?);
        let response = if status == common_enums::AttemptStatus::Failure {
            Err(ErrorResponse {
                code: item.response.result.code.clone(),
                message: item.response.result.description.clone(),
                reason: Some(item.response.result.description),
                status_code: item.http_code,
                attempt_status: Some(status),
                connector_transaction_id: Some(item.response.id.clone()),
                ..Default::default()
            })
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.referenced_id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            })
        };
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}

#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AciRefundRequest {
    pub amount: StringMajorUnit,
    pub currency: String,
    pub payment_type: AciPaymentType,
    pub entity_id: Secret<String>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AciRouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for AciRefundRequest
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: AciRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.minor_refund_amount,
                item.router_data.request.currency,
            )
            .change_context(ConnectorError::AmountConversionFailed)?;
        let currency = item.router_data.request.currency;
        let payment_type = AciPaymentType::Refund;
        let auth = AciAuthType::try_from(&item.router_data.connector_config)?;

        Ok(Self {
            amount,
            currency: currency.to_string(),
            payment_type,
            entity_id: auth.entity_id,
        })
    }
}

#[derive(Debug, Default, Deserialize, Clone)]
pub enum AciRefundStatus {
    Succeeded,
    Failed,
    #[default]
    Pending,
}

impl FromStr for AciRefundStatus {
    type Err = error_stack::Report<ConnectorError>;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if FAILURE_CODES.contains(&s) {
            Ok(Self::Failed)
        } else if PENDING_CODES.contains(&s) {
            Ok(Self::Pending)
        } else if SUCCESSFUL_CODES.contains(&s) {
            Ok(Self::Succeeded)
        } else {
            Err(report!(ConnectorError::UnexpectedResponseError(
                bytes::Bytes::from(s.to_owned())
            )))
        }
    }
}

impl From<AciRefundStatus> for common_enums::RefundStatus {
    fn from(item: AciRefundStatus) -> Self {
        match item {
            AciRefundStatus::Succeeded => Self::Success,
            AciRefundStatus::Failed => Self::Failure,
            AciRefundStatus::Pending => Self::Pending,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AciRefundResponse {
    id: String,
    ndc: String,
    timestamp: String,
    build_number: String,
    pub(super) result: ResultCode,
}

impl<F> TryFrom<ResponseRouterData<AciRefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<AciRefundResponse, Self>) -> Result<Self, Self::Error> {
        let refund_status = common_enums::RefundStatus::from(AciRefundStatus::from_str(
            &item.response.result.code,
        )?);
        let response = if refund_status == common_enums::RefundStatus::Failure {
            Err(ErrorResponse {
                code: item.response.result.code.clone(),
                message: item.response.result.description.clone(),
                reason: Some(item.response.result.description),
                status_code: item.http_code,
                attempt_status: None,
                connector_transaction_id: Some(item.response.id.clone()),
                network_decline_code: None,
                network_advice_code: None,
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

impl<F, T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<AciMandateResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<AciMandateResponse, Self>) -> Result<Self, Self::Error> {
        let mandate_reference = Some(MandateReference {
            connector_mandate_id: Some(item.response.id.clone()),
            payment_method_id: None,
            connector_mandate_request_reference_id: None,
        });

        let status = if SUCCESSFUL_CODES.contains(&item.response.result.code.as_str()) {
            common_enums::AttemptStatus::Charged
        } else if FAILURE_CODES.contains(&item.response.result.code.as_str()) {
            common_enums::AttemptStatus::Failure
        } else {
            common_enums::AttemptStatus::Pending
        };

        let response = if status == common_enums::AttemptStatus::Failure {
            Err(ErrorResponse {
                code: item.response.result.code.clone(),
                message: item.response.result.description.clone(),
                reason: Some(item.response.result.description),
                status_code: item.http_code,
                attempt_status: Some(status),
                connector_transaction_id: Some(item.response.id.clone()),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            })
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: mandate_reference.map(Box::new),
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.id),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            })
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum AciWebhookEventType {
    Payment,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum AciWebhookAction {
    Created,
    Updated,
    Deleted,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AciWebhookCardDetails {
    pub bin: Option<String>,
    #[serde(rename = "last4Digits")]
    pub last4_digits: Option<String>,
    pub holder: Option<String>,
    pub expiry_month: Option<Secret<String>>,
    pub expiry_year: Option<Secret<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AciWebhookCustomerDetails {
    #[serde(rename = "givenName")]
    pub given_name: Option<Secret<String>>,
    pub surname: Option<Secret<String>>,
    #[serde(rename = "merchantCustomerId")]
    pub merchant_customer_id: Option<Secret<String>>,
    pub sex: Option<Secret<String>>,
    pub email: Option<Email>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AciWebhookAuthenticationDetails {
    #[serde(rename = "entityId")]
    pub entity_id: Secret<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AciWebhookRiskDetails {
    pub score: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AciPaymentWebhookPayload {
    pub id: String,
    pub payment_type: String,
    pub payment_brand: String,
    pub amount: StringMajorUnit,
    pub currency: String,
    pub presentation_amount: Option<StringMajorUnit>,
    pub presentation_currency: Option<String>,
    pub descriptor: Option<String>,
    pub result: ResultCode,
    pub authentication: Option<AciWebhookAuthenticationDetails>,
    pub card: Option<AciWebhookCardDetails>,
    pub customer: Option<AciWebhookCustomerDetails>,
    #[serde(rename = "customParameters")]
    pub custom_parameters: Option<serde_json::Value>,
    pub risk: Option<AciWebhookRiskDetails>,
    pub build_number: Option<String>,
    pub timestamp: String,
    pub ndc: String,
    #[serde(rename = "channelName")]
    pub channel_name: Option<String>,
    pub source: Option<String>,
    pub payment_method: Option<String>,
    #[serde(rename = "shortId")]
    pub short_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AciWebhookNotification {
    #[serde(rename = "type")]
    pub event_type: AciWebhookEventType,
    pub action: Option<AciWebhookAction>,
    pub payload: serde_json::Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AciRepeatPaymentRequest<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(flatten)]
    pub txn_details: TransactionDetails,
    #[serde(flatten)]
    pub payment_method: PaymentDetails<T>,
    #[serde(flatten)]
    pub instruction: Option<Instruction>,
    pub shopper_result_url: Option<String>,
    #[serde(rename = "customParameters[3DS2_enrolled]")]
    pub three_ds_two_enrolled: Option<bool>,
    pub recurring_type: Option<AciRecurringType>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AciRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AciRepeatPaymentRequest<T>
{
    type Error = Error;
    fn try_from(
        item: AciRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = AciAuthType::try_from(&item.router_data.connector_config)?;
        let amount = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.minor_amount,
                item.router_data.request.currency,
            )
            .change_context(ConnectorError::AmountConversionFailed)?;
        let payment_type = if item.router_data.request.is_auto_capture()? {
            AciPaymentType::Debit
        } else {
            AciPaymentType::Preauthorization
        };

        let instruction = Some(Instruction {
            mode: InstructionMode::Repeated,
            transaction_type: InstructionType::Unscheduled,
            source: InstructionSource::MerchantInitiatedTransaction,
            create_registration: None,
        });
        let txn_details = TransactionDetails {
            entity_id: auth.entity_id,
            amount,
            currency: item.router_data.request.currency.to_string(),
            payment_type,
        };
        let recurring_type = Some(AciRecurringType::Repeated);

        Ok(Self {
            txn_details,
            payment_method: PaymentDetails::Mandate,
            instruction,
            shopper_result_url: item.router_data.resource_common_data.return_url.clone(),
            three_ds_two_enrolled: None,
            recurring_type,
        })
    }
}
