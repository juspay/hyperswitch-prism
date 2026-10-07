use base64::Engine;
use common_enums::{AttemptStatus, CaptureMethod, CardNetwork};
use common_utils::consts::NO_ERROR_MESSAGE;
use common_utils::{fp_utils::when, pii::SecretSerdeValue};
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, Refund, RepeatPayment,
        ServerAuthenticationToken, SetupMandate, Void, VoidPC,
    },
    connector_types::{
        ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData,
        ConnectorSpecificClientAuthenticationResponse,
        JpmorganClientAuthenticationResponse as JpmorganClientAuthenticationResponseDomain,
        MandateReference, MandateReferenceId, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCancelPostCaptureData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, RepeatPaymentData, ResponseId, ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData, SetupMandateRequestData,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::{BankDebitData, PaymentMethodData, PaymentMethodDataTypes, WalletData},
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret, SwitchStrategy};
use serde::{Deserialize, Serialize};

use super::{requests, responses, JpmorganAmountConvertor};
use crate::{connectors::jpmorgan::JpmorganRouterData, types::ResponseRouterData, utils};
use domain_types::errors::{ConnectorError, IntegrationError, IntegrationErrorContext};
use domain_types::utils::is_payment_failure;

type Error = error_stack::Report<IntegrationError>;
type ResponseError = error_stack::Report<ConnectorError>;

const JPMORGAN_GETTING_STARTED_DOC: &str =
    "https://developer.payments.jpmorgan.com/docs/commerce-solutions/online-payments/guides/getting-started";
const JPMORGAN_TOKENIZATION_DOC: &str =
    "https://developer.payments.jpmorgan.com/docs/commerce/online-payments/capabilities/online-payments/payment-enhancements/tokenization";
// JPMorgan specifies these token ECI values when the wallet does not supply one.
const VISA_TOKEN_ECI: &str = "7";
const MASTERCARD_TOKEN_ECI: &str = "5";

const JPMORGAN_API_DOC: &str =
    "https://developer.payments.jpmorgan.com/docs/commerce/online-payments/capabilities/online-payments";

impl TryFrom<Option<common_enums::BankType>> for requests::JpmorganAchAccountType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(bank_type: Option<common_enums::BankType>) -> Result<Self, Self::Error> {
        match bank_type {
            Some(common_enums::BankType::Savings) => Ok(Self::Savings),
            Some(common_enums::BankType::Checking) | None => Ok(Self::Checking),
            Some(bank) => Err(error_stack::report!(IntegrationError::NotSupported {
                message: format!("Bank type {bank:?} is not supported by jpmorgan"),
                connector: "jpmorgan",
                context: IntegrationErrorContext {
                    suggested_action: Some("Provide a valid bank account type".to_owned()),
                    additional_context: None,
                    doc_url: None,
                },
            })),
        }
    }
}

/// Build an `IntegrationErrorContext` for a missing JPMorgan connector config field.
fn jpmorgan_missing_field_context(field_name: &str) -> IntegrationErrorContext {
    IntegrationErrorContext {
        suggested_action: Some(format!(
            "Set the '{}' field in the JPMorgan connector configuration. This is required \
             by JPMorgan's Online Payments API for every payment request.",
            field_name
        )),
        doc_url: Some(JPMORGAN_GETTING_STARTED_DOC.to_owned()),
        additional_context: Some(format!(
            "JPMorgan requires '{}' as a mandatory field in the merchant software or \
             connector configuration.",
            field_name
        )),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganAuthType {
    pub client_id: Secret<String>,
    pub client_secret: Secret<String>,
    pub company_name: Option<Secret<String>>,
    pub product_name: Option<Secret<String>>,
    pub merchant_purchase_description: Option<Secret<String>>,
    pub statement_descriptor: Option<Secret<String>>,
}

impl TryFrom<&ConnectorSpecificConfig> for JpmorganAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Jpmorgan {
                client_id,
                client_secret,
                company_name,
                product_name,
                merchant_purchase_description,
                statement_descriptor,
                ..
            } => Ok(Self {
                client_id: client_id.clone(),
                client_secret: client_secret.clone(),
                company_name: company_name.clone(),
                product_name: product_name.clone(),
                merchant_purchase_description: merchant_purchase_description.clone(),
                statement_descriptor: statement_descriptor.clone(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

/// JPMorgan connector metadata containing merchant software information
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct JpmorganConnectorMetadataObject {
    pub company_name: Secret<String>,
    pub product_name: Secret<String>,
    pub merchant_purchase_description: Secret<String>,
    pub statement_descriptor: Secret<String>,
}

impl TryFrom<&Option<SecretSerdeValue>> for JpmorganConnectorMetadataObject {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(meta_data: &Option<SecretSerdeValue>) -> Result<Self, Self::Error> {
        let metadata: Self = utils::to_connector_meta_from_secret::<Self>(meta_data.clone())
            .change_context(IntegrationError::InvalidConnectorConfig {
                config: "merchant_connector_account.metadata",
                context: Default::default(),
            })?;
        Ok(metadata)
    }
}

// OAuth 2.0 transformers
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        JpmorganRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for requests::JpmorganTokenRequest
{
    type Error = Error;
    fn try_from(
        _item: JpmorganRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            grant_type: String::from("client_credentials"),
            scope: String::from("jpm:payments:sandbox"),
        })
    }
}

impl<F> TryFrom<ResponseRouterData<responses::JpmorganAuthUpdateResponse, Self>>
    for RouterDataV2<
        F,
        MerchantAuthenticationFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    >
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganAuthUpdateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(ServerAuthenticationTokenResponseData {
                access_token: item.response.access_token,
                token_type: Some(item.response.token_type.clone()),
                expires_in: Some(item.response.expires_in),
            }),
            ..item.router_data
        })
    }
}

fn map_capture_method(
    capture_method: Option<CaptureMethod>,
) -> Result<requests::CapMethod, error_stack::Report<IntegrationError>> {
    match capture_method {
        Some(CaptureMethod::Automatic) | None => Ok(requests::CapMethod::Now),
        Some(CaptureMethod::Manual) => Ok(requests::CapMethod::Manual),
        Some(CaptureMethod::Scheduled)
        | Some(CaptureMethod::ManualMultiple)
        | Some(CaptureMethod::SequentialAutomatic) => {
            Err(error_stack::report!(IntegrationError::NotSupported {
                message: "Capture Method".to_string(),
                connector: "Jpmorgan",
                context: Default::default(),
            }))
        }
    }
}

fn merchant_order_number(value: Option<&String>) -> Result<Option<String>, Error> {
    value
        .map(|value| {
            if value.is_empty()
                || value.len() > 40
                || !value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b' ' | b'.' | b'-'))
            {
                return Err(IntegrationError::InvalidDataFormat {
                    field_name: "merchant_order_id",
                    context: utils::integration_ctx("JPMorgan order numbers permit at most 40 letters, digits, spaces, periods, and hyphens", "Supply a valid merchant order number"),
                }.into());
            }
            Ok(value.clone())
        })
        .transpose()
}

fn token_authentication(
    cryptogram: &Secret<String>,
    eci: Option<&String>,
    network: &str,
) -> Result<requests::JpmorganAuthentication, Error> {
    let value = cryptogram.peek();
    if value.is_empty()
        || value.len() > 80
        || (value.len() > 4 && common_utils::consts::BASE64_ENGINE.decode(value).is_err())
    {
        return Err(IntegrationError::InvalidDataFormat {
            field_name: "wallet.cryptogram",
            context: IntegrationErrorContext {
                suggested_action: Some("Supply a cryptogram of at most 80 characters. Use Base64 for values longer than four characters.".to_owned()),
                doc_url: Some(JPMORGAN_TOKENIZATION_DOC.to_owned()),
                additional_context: None,
            },
        }.into());
    }
    if eci.is_some_and(|eci| {
        eci.is_empty() || eci.len() > 2 || !eci.bytes().all(|byte| byte.is_ascii_digit())
    }) {
        return Err(IntegrationError::InvalidDataFormat {
            field_name: "wallet.eci_indicator",
            context: utils::integration_ctx(
                "The supplied ECI must contain one or two digits",
                "Supply a numeric ECI or omit it",
            ),
        }
        .into());
    }
    let electronic_commerce_indicator =
        eci.cloned()
            .or_else(|| match network.to_ascii_lowercase().as_str() {
                "visa" => Some(VISA_TOKEN_ECI.to_owned()),
                "mastercard" | "master_card" => Some(MASTERCARD_TOKEN_ECI.to_owned()),
                _ => None,
            });
    Ok(requests::JpmorganAuthentication {
        three_ds: None,
        token_authentication_value: Some(cryptogram.clone()),
        electronic_commerce_indicator,
    })
}

fn decrypted_wallet_card<T: PaymentMethodDataTypes>(
    wallet: &WalletData,
) -> Result<requests::JpmorganCard<T>, Error> {
    match wallet {
        WalletData::GooglePay(data) => {
            let decrypted = match &data.tokenization_data {
                domain_types::payment_method_data::GpayTokenizationData::Decrypted(data) => data,
                domain_types::payment_method_data::GpayTokenizationData::Encrypted(_) => {
                    return Err(IntegrationError::MissingRequiredField {
                        field_name: "wallet.google_pay.decrypted_data",
                        context: utils::integration_ctx(
                            "This flow requires supplied decrypted Google Pay data",
                            "Supply decrypted wallet data",
                        ),
                    }
                    .into());
                }
            };
            let auth_method =
                decrypted
                    .auth_method
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "wallet.google_pay.auth_method",
                        context: utils::integration_ctx(
                            "Decrypted Google Pay data requires its credential type",
                            "Supply PAN_ONLY or CRYPTOGRAM_3DS in auth_method",
                        ),
                    })?;
            let (account_number_type, authentication) = match auth_method {
                common_enums::GooglePayAuthMethod::PanOnly => {
                    if decrypted.cryptogram.is_some() {
                        return Err(IntegrationError::InvalidDataFormat {
                            field_name: "wallet.google_pay.auth_method",
                            context: utils::integration_ctx(
                                "PAN_ONLY data must not contain a token cryptogram",
                                "Use CRYPTOGRAM_3DS for token cryptogram data",
                            ),
                        }
                        .into());
                    }
                    (requests::JpmorganAccountNumberType::Pan, None)
                }
                common_enums::GooglePayAuthMethod::Cryptogram => {
                    let cryptogram =
                        decrypted
                            .cryptogram
                            .as_ref()
                            .ok_or(IntegrationError::MissingRequiredField {
                            field_name: "wallet.cryptogram",
                            context: utils::integration_ctx(
                                "A token payment initiated by the cardholder requires a cryptogram",
                                "Supply the cryptogram from the decrypted wallet data",
                            ),
                        })?;
                    (
                        requests::JpmorganAccountNumberType::DeviceToken,
                        Some(token_authentication(
                            cryptogram,
                            decrypted.eci_indicator.as_ref(),
                            &data.info.card_network,
                        )?),
                    )
                }
            };
            let year = decrypted.get_four_digit_expiry_year().change_context(
                IntegrationError::InvalidDataFormat {
                    field_name: "wallet.expiry_year",
                    context: utils::integration_ctx(
                        "The wallet expiry year has an invalid length",
                        "Supply a two-digit or four-digit expiry year",
                    ),
                },
            )?;
            Ok(requests::JpmorganCard {
                account_number: requests::JpmorganAccountNumber::Decrypted(
                    decrypted.application_primary_account_number.clone(),
                ),
                expiry: build_jpmorgan_expiry(&decrypted.card_exp_month, &year)?,
                account_number_type: Some(account_number_type),
                wallet_provider: Some(requests::JpmorganWalletProvider::GooglePay),
                authentication,
                original_network_transaction_id: None,
                original_transaction_link_id: None,
                payment_authentication_request: None,
                verification_authentication_request: None,
            })
        }
        WalletData::ApplePay(data) => {
            let decrypted = data
                .payment_data
                .get_decrypted_apple_pay_payment_data_optional()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "wallet.apple_pay.decrypted_data",
                    context: utils::integration_ctx(
                        "Apple Pay requires supplied decrypted data in this connector",
                        "Supply decrypted Apple Pay data",
                    ),
                })?;
            if decrypted
                .merchant_token_identifier
                .as_ref()
                .is_some_and(|id| id.peek().is_empty())
            {
                return Err(IntegrationError::InvalidDataFormat {
                    field_name: "wallet.apple_pay.merchant_token_identifier",
                    context: utils::integration_ctx(
                        "The supplied Apple merchant token identifier is empty",
                        "Supply the original merchant token identifier or omit it",
                    ),
                }
                .into());
            }
            Ok(requests::JpmorganCard {
                account_number: requests::JpmorganAccountNumber::Decrypted(
                    decrypted.application_primary_account_number.clone(),
                ),
                expiry: build_jpmorgan_expiry(
                    &decrypted.application_expiration_month,
                    &decrypted.get_four_digit_expiry_year(),
                )?,
                account_number_type: Some(if decrypted.merchant_token_identifier.is_some() {
                    requests::JpmorganAccountNumberType::NetworkToken
                } else {
                    requests::JpmorganAccountNumberType::DeviceToken
                }),
                wallet_provider: Some(requests::JpmorganWalletProvider::ApplePay),
                authentication: Some(token_authentication(
                    &decrypted.payment_data.online_payment_cryptogram,
                    decrypted.payment_data.eci_indicator.as_ref(),
                    &data.payment_method.network,
                )?),
                original_network_transaction_id: None,
                original_transaction_link_id: None,
                payment_authentication_request: None,
                verification_authentication_request: None,
            })
        }
        WalletData::AliPayQr(_)
        | WalletData::AliPayRedirect(_)
        | WalletData::AliPayHkRedirect(_)
        | WalletData::BluecodeRedirect { .. }
        | WalletData::AmazonPayRedirect(_)
        | WalletData::MomoRedirect(_)
        | WalletData::KakaoPayRedirect(_)
        | WalletData::GoPayRedirect(_)
        | WalletData::GcashRedirect(_)
        | WalletData::ApplePayRedirect(_)
        | WalletData::ApplePayThirdPartySdk(_)
        | WalletData::DanaRedirect { .. }
        | WalletData::GrabpayRedirect { .. }
        | WalletData::GooglePayRedirect(_)
        | WalletData::GooglePayThirdPartySdk(_)
        | WalletData::MbWayRedirect(_)
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
        | WalletData::PaymayaRedirect(_)
        | WalletData::PayhereRedirect { .. }
        | WalletData::QwikcilverWalletDirect(_)
        | WalletData::Skrill(_)
        | WalletData::Neteller(_) => Err(IntegrationError::NotSupported {
            message: "Wallet payment method".to_owned(),
            connector: "jpmorgan",
            context: utils::integration_ctx(
                "JPMorgan wallet support covers Apple Pay and Google Pay",
                "Supply Apple Pay or Google Pay data",
            ),
        }
        .into()),
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    requests::JpmorganPaymentsRequest<T>
{
    fn from_decrypted_wallet(
        data: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
        wallet: &WalletData,
    ) -> Result<Self, Error> {
        let auth = JpmorganAuthType::try_from(&data.connector_config)?;
        Ok(Self {
            capture_method: map_capture_method(data.request.capture_method)?,
            amount: JpmorganAmountConvertor::convert(
                data.request.minor_amount,
                data.request.currency,
            )?,
            currency: data.request.currency,
            merchant: requests::JpmorganMerchant::try_from(&auth)?,
            payment_method_type: requests::JpmorganPaymentMethodType {
                card: Some(decrypted_wallet_card(wallet)?),
                ach: None,
                googlepay: None,
                token: None,
            },
            account_holder: None,
            statement_descriptor: None,
            stored_credential: Default::default(),
        })
    }
}

/// Extract first name and last name from account holder name or billing info
fn extract_account_holder_names<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
    _bank_account_holder_name: &Option<Secret<String>>,
) -> Result<(Secret<String>, Secret<String>), error_stack::Report<IntegrationError>> {
    // Use billing address first_name and last_name directly (like Forte connector)
    let first_name = router_data
        .resource_common_data
        .get_billing_first_name()
        .ok()
        .unwrap_or_else(|| Secret::new("".to_string()));

    let last_name = router_data
        .resource_common_data
        .get_optional_billing_last_name()
        .unwrap_or_else(|| first_name.clone());

    Ok((first_name, last_name))
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        JpmorganRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for requests::JpmorganPaymentsRequest<T>
{
    type Error = Error;
    fn try_from(
        item: JpmorganRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        let mut request = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                let capture_method = map_capture_method(router_data.request.capture_method)?;

                let auth = JpmorganAuthType::try_from(&router_data.connector_config)?;

                let merchant = requests::JpmorganMerchant::try_from(&auth)?;

                let exp_month_str = card_data.card_exp_month.peek().to_string();
                let exp_year_str = card_data.get_expiry_year_4_digit().peek().to_string();

                // Vault token placeholders (e.g. "{{$card_exp_month}}") cannot be parsed as i32.
                // JPMorgan requires numeric expiry values, so proxy flows are not supported.
                when(
                    exp_month_str.contains("{{") || exp_year_str.contains("{{"),
                    || {
                        Err(error_stack::report!(IntegrationError::NotSupported {
                            message: "JPMorgan requires numeric expiry values; vault token placeholders are not supported for proxy flows".to_string(),
                            connector: "Jpmorgan",
                            context: Default::default(),
                        }))
                    },
                )?;

                let expiry =
                    build_jpmorgan_expiry(&card_data.card_exp_month, &card_data.card_exp_year)?;

                let card = requests::JpmorganCard {
                    account_number: requests::JpmorganAccountNumber::Card(
                        card_data.card_number.clone(),
                    ),
                    expiry,
                    account_number_type: None,
                    wallet_provider: None,
                    authentication: None,
                    original_network_transaction_id: None,
                    original_transaction_link_id: None,
                    payment_authentication_request: None,
                    verification_authentication_request: None,
                };

                let payment_method_type = requests::JpmorganPaymentMethodType {
                    card: Some(card),
                    ach: None,
                    googlepay: None,
                    token: None,
                };

                let amount = JpmorganAmountConvertor::convert(
                    router_data.request.minor_amount,
                    router_data.request.currency,
                )?;

                Ok(Self {
                    capture_method,
                    currency: router_data.request.currency,
                    amount,
                    merchant,
                    payment_method_type,
                    account_holder: None,
                    statement_descriptor: None,
                    stored_credential: Default::default(),
                })
            }
            PaymentMethodData::BankDebit(BankDebitData::AchBankDebit {
                account_number,
                routing_number,
                bank_account_holder_name,
                bank_type,
                ..
            }) => {
                let capture_method = map_capture_method(router_data.request.capture_method)?;

                let auth = JpmorganAuthType::try_from(&router_data.connector_config)?;

                let merchant = requests::JpmorganMerchant::try_from(&auth)?;

                // Extract first name and last name from account holder name or billing info
                let (first_name, last_name) =
                    extract_account_holder_names(router_data, bank_account_holder_name)?;

                let billing = router_data
                    .resource_common_data
                    .address
                    .get_payment_method_billing()
                    .or_else(|| {
                        router_data
                            .resource_common_data
                            .address
                            .get_payment_billing()
                    });

                let account_holder = requests::JpmorganAccountHolder {
                    first_name,
                    last_name,
                    email: router_data
                        .request
                        .email
                        .as_ref()
                        .or_else(|| billing.and_then(|billing| billing.email.as_ref()))
                        .cloned(),
                    billing_address: billing.and_then(|billing| billing.address.as_ref()).map(
                        |address| requests::JpmorganBillingAddress {
                            line1: address.line1.clone(),
                            city: address.city.clone(),
                            postal_code: address.zip.clone(),
                            country_code: address
                                .country
                                .map(common_enums::CountryAlpha2::from_alpha2_to_alpha3),
                        },
                    ),
                    phone: billing
                        .and_then(|billing| billing.phone.as_ref())
                        .map(requests::JpmorganPhone::try_from)
                        .transpose()?,
                };

                // Determine account type based on bank_type field, default to Checking
                let account_type = requests::JpmorganAchAccountType::try_from(*bank_type)?;

                let ach = requests::JpmorganAch {
                    account_number: account_number.clone(),
                    financial_institution_routing_number: routing_number.clone(),
                    account_type,
                };

                let payment_method_type = requests::JpmorganPaymentMethodType {
                    card: None,
                    ach: Some(ach),
                    googlepay: None,
                    token: None,
                };

                let amount = JpmorganAmountConvertor::convert(
                    router_data.request.minor_amount,
                    router_data.request.currency,
                )?;

                // Get statement_descriptor from connector config
                let statement_descriptor = auth.statement_descriptor.clone().ok_or(
                    IntegrationError::MissingRequiredField {
                        field_name: "statement_descriptor",
                        context: Default::default(),
                    },
                )?;

                Ok(Self {
                    capture_method,
                    currency: router_data.request.currency,
                    amount,
                    merchant,
                    payment_method_type,
                    account_holder: Some(account_holder),
                    statement_descriptor: Some(statement_descriptor),
                    stored_credential: Default::default(),
                })
            }
            PaymentMethodData::PaymentMethodToken(token_data) => {
                let token = token_data.token.clone();

                let capture_method = map_capture_method(router_data.request.capture_method)?;

                let auth = JpmorganAuthType::try_from(&router_data.connector_config)?;

                let merchant = requests::JpmorganMerchant::try_from(&auth)?;

                // For CardToken, the token is passed in the payment_method_type
                // instead of raw card details
                let payment_method_type = requests::JpmorganPaymentMethodType {
                    card: None,
                    ach: None,
                    googlepay: None,
                    token: Some(token),
                };

                let amount = JpmorganAmountConvertor::convert(
                    router_data.request.minor_amount,
                    router_data.request.currency,
                )?;

                let account_holder = requests::JpmorganAccountHolder {
                    first_name: Secret::new("NA".to_string()),
                    last_name: Secret::new("NA".to_string()),
                    email: None,
                    billing_address: None,
                    phone: None,
                };
                let statement_descriptor = Secret::new("Statement Descriptor".to_string());

                Ok(Self {
                    capture_method,
                    currency: router_data.request.currency,
                    amount,
                    merchant,
                    payment_method_type,
                    account_holder: Some(account_holder),
                    statement_descriptor: Some(statement_descriptor),
                    stored_credential: Default::default(),
                })
            }
            PaymentMethodData::BankDebit(_) => {
                Err(error_stack::report!(IntegrationError::NotSupported {
                    message: "Only ACH Bank Debit is supported".to_string(),
                    connector: "Jpmorgan",
                    context: Default::default(),
                }))
            }
            PaymentMethodData::Wallet(wallet_data @ WalletData::GooglePay(google_pay_data)) => {
                match &google_pay_data.tokenization_data {
                    domain_types::payment_method_data::GpayTokenizationData::Encrypted(
                        encrypted_data,
                    ) => {
                        let capture_method =
                            map_capture_method(router_data.request.capture_method)?;

                        let auth = JpmorganAuthType::try_from(&router_data.connector_config)?;

                        let merchant = requests::JpmorganMerchant::try_from(&auth)?;

                        let amount = JpmorganAmountConvertor::convert(
                            router_data.request.minor_amount,
                            router_data.request.currency,
                        )?;

                        // Parse the Google Pay token string into its component fields.
                        // The token is a JSON string containing protocolVersion, signature,
                        // optionally intermediateSigningKey, and signedMessage.
                        let gpay_token: requests::GooglePayToken = serde_json::from_str(
                            &encrypted_data.token,
                        )
                        .change_context(IntegrationError::RequestEncodingFailed {
                            context: Default::default(),
                        })?;

                        // Parse signedMessage to extract ephemeralPublicKey.
                        // signedMessage is itself a JSON string.
                        let signed_message: requests::GooglePaySignedMessage =
                            serde_json::from_str(gpay_token.signed_message.peek()).change_context(
                                IntegrationError::RequestEncodingFailed {
                                    context: Default::default(),
                                },
                            )?;

                        // For ECv2, signature comes from intermediateSigningKey.signatures[0].
                        // For ECv1, signature comes from the top-level signature field.
                        let signature = if let Some(isk) = &gpay_token.intermediate_signing_key {
                            isk.signatures.first().cloned().ok_or(
                                IntegrationError::MissingRequiredField {
                                    field_name: "intermediateSigningKey.signatures[0]",
                                    context: Default::default(),
                                },
                            )?
                        } else {
                            gpay_token.signature.clone()
                        };

                        let googlepay = requests::JpmorganGooglePay {
                            // latLong is required by JPMorgan; use "0,0" when not available
                            lat_long: "0,0".to_string(),
                            encrypted_payment_bundle: requests::JpmorganEncryptedPaymentBundle {
                                // encryptedPayload is the raw signedMessage JSON string
                                encrypted_payload: gpay_token.signed_message.clone(),
                                encrypted_payment_header:
                                    requests::JpmorganEncryptedPaymentHeader {
                                        ephemeral_public_key: signed_message.ephemeral_public_key,
                                    },
                                signature,
                                protocol_version: gpay_token.protocol_version,
                            },
                        };

                        let payment_method_type = requests::JpmorganPaymentMethodType {
                            card: None,
                            ach: None,
                            googlepay: Some(googlepay),
                            token: None,
                        };

                        Ok(Self {
                            capture_method,
                            currency: router_data.request.currency,
                            amount,
                            merchant,
                            payment_method_type,
                            // account_holder and statement_descriptor are not required
                            // for Google Pay encrypted flow
                            account_holder: None,
                            statement_descriptor: None,
                            stored_credential: Default::default(),
                        })
                    }
                    domain_types::payment_method_data::GpayTokenizationData::Decrypted(_) => {
                        Self::from_decrypted_wallet(router_data, wallet_data)
                    }
                }
            }
            PaymentMethodData::Wallet(wallet_data) => {
                Self::from_decrypted_wallet(router_data, wallet_data)
            }
            _ => Err(error_stack::report!(IntegrationError::NotSupported {
                message: "Payment method not supported".to_string(),
                connector: "Jpmorgan",
                context: Default::default(),
            })),
        }?;
        let data = &router_data.request;
        let flow = &router_data.resource_common_data;
        let network = payment_network(&data.payment_method_data);
        let stores_credentials = data.setup_future_usage.is_some()
            || data.setup_mandate_details.is_some()
            || data.mit_category.is_some();

        if stores_credentials {
            let context = cit_context(
                data.connector_feature_data
                    .as_ref()
                    .or(flow.connector_feature_data.as_ref()),
                data.metadata.as_ref(),
                data.mit_category.as_ref(),
                data.merchant_order_id.as_deref(),
                &flow.connector_request_reference_id,
            )?;
            validate_recurring_context(&context, network.clone())?;
            request.stored_credential = cit_stored_credential(&context)?;
        }

        if let Some(authentication_data) = &data.authentication_data {
            let card = request.payment_method_type.card.as_mut().ok_or_else(|| {
                IntegrationError::NotSupported {
                    message: "JPMorgan pass-through 3DS requires card credentials".to_owned(),
                    connector: "jpmorgan",
                    context: utils::integration_ctx(
                        "The payment method cannot carry card authentication data",
                        "Supply card or decrypted wallet credentials for pass-through 3DS",
                    ),
                }
            })?;
            apply_three_ds_authentication(card, authentication_data, network)?;
        } else if flow.auth_type == common_enums::AuthenticationType::ThreeDs {
            let purpose = if request.stored_credential.recurring.is_some() {
                requests::JpmorganAuthenticationPurpose::RecurringTransaction
            } else {
                requests::JpmorganAuthenticationPurpose::PaymentTransaction
            };
            let (authentication, browser, holder) = native_three_ds(
                flow,
                data.browser_info.as_ref(),
                data.complete_authorize_url.as_deref(),
                data.email.as_ref(),
                network,
                purpose,
                requests::JpmorganThreeDsTransactionType::GoodsServices,
            )?;
            let card = request.payment_method_type.card.as_mut().ok_or_else(
                domain_types::utils::missing_field_err("payment_method_data.card"),
            )?;
            card.payment_authentication_request = Some(authentication);
            request.stored_credential.browser_info = Some(browser);
            request.account_holder = Some(holder);
        }
        let has_card_enhancements = request
            .payment_method_type
            .card
            .as_ref()
            .is_some_and(|card| {
                card.account_number_type.is_some()
                    || card.payment_authentication_request.is_some()
                    || card
                        .authentication
                        .as_ref()
                        .is_some_and(|authentication| authentication.three_ds.is_some())
            });
        if request.stored_credential.initiator_type.is_some() || has_card_enhancements {
            request.stored_credential.merchant_order_number =
                merchant_order_number(data.merchant_order_id.as_ref())?;
        }
        Ok(request)
    }
}

pub fn connector_context(
    feature_data: Option<&SecretSerdeValue>,
    metadata: Option<&SecretSerdeValue>,
) -> Result<requests::JpmorganContext, Error> {
    let feature = feature_data.and_then(|data| data.peek().get("jpmorgan"));
    let value = feature.or_else(|| metadata.and_then(|data| data.peek().get("jpmorgan")));
    let context: requests::JpmorganContext = value.map_or_else(
        || Ok(requests::JpmorganContext::default()),
        |value| {
            serde_json::from_value(value.clone()).change_context(
                IntegrationError::InvalidDataFormat {
                    field_name: "metadata.jpmorgan",
                    context: utils::integration_ctx(
                        "JPMorgan metadata has invalid field types",
                        "Supply JPMorgan metadata with the documented field types",
                    ),
                },
            )
        },
    )?;
    if feature.is_none()
        && (context.continue_three_ds
            || context.three_ds_resource.is_some()
            || context.native_capture_method.is_some())
    {
        return Err(IntegrationError::InvalidDataFormat {
            field_name: "metadata.jpmorgan",
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Use persisted connector_feature_data for 3DS continuation".to_owned(),
                ),
                additional_context: Some(
                    "Caller metadata cannot contain 3DS operation state".to_owned(),
                ),
                doc_url: Some(JPMORGAN_API_DOC.to_owned()),
            },
        }
        .into());
    }
    Ok(context)
}

pub fn resolved_connector_context(
    request_feature: Option<&SecretSerdeValue>,
    common_feature: Option<&SecretSerdeValue>,
    metadata: Option<&SecretSerdeValue>,
) -> Result<requests::JpmorganContext, Error> {
    let request = request_feature.and_then(|data| data.peek().get("jpmorgan"));
    let common = common_feature.and_then(|data| data.peek().get("jpmorgan"));
    if matches!((request, common), (Some(request), Some(common)) if request != common) {
        return Err(IntegrationError::InvalidDataFormat {
            field_name: "connector_feature_data.jpmorgan",
            context: utils::integration_ctx(
                "Request and common connector state conflict",
                "Return the same saved connector state in both locations",
            ),
        }
        .into());
    }
    connector_context(
        request_feature
            .filter(|_| request.is_some())
            .or(common_feature),
        metadata,
    )
}

fn cit_context(
    feature_data: Option<&SecretSerdeValue>,
    metadata: Option<&SecretSerdeValue>,
    category: Option<&common_enums::MitCategory>,
    merchant_order_id: Option<&str>,
    request_reference: &str,
) -> Result<requests::JpmorganContext, Error> {
    let mut context = connector_context(feature_data, metadata)?;
    match category {
        Some(common_enums::MitCategory::Recurring) => context.scheduled_recurring = true,
        Some(common_enums::MitCategory::Unscheduled) => context.scheduled_recurring = false,
        None => {}
        Some(common_enums::MitCategory::Installment | common_enums::MitCategory::Resubmission) => {
            return Err(IntegrationError::NotImplemented(
                "JPMorgan installment and resubmission mandates".to_owned(),
                utils::integration_ctx(
                    "This connector does not implement installment or resubmission payments",
                    "Use recurring or unscheduled payments",
                ),
            )
            .into());
        }
    }
    if context.scheduled_recurring {
        context
            .agreement_id
            .get_or_insert_with(|| merchant_order_id.unwrap_or(request_reference).to_owned());
        if context
            .agreement_id
            .as_ref()
            .is_some_and(|id| id.is_empty() || id.len() > 100)
        {
            return Err(IntegrationError::InvalidDataFormat {
                field_name: "metadata.jpmorgan.agreementId",
                context: utils::integration_ctx(
                    "A recurring agreement identifier must contain 1 to 100 characters",
                    "Supply the original recurring agreement identifier",
                ),
            }
            .into());
        }
    }
    Ok(context)
}

fn cit_stored_credential(
    context: &requests::JpmorganContext,
) -> Result<requests::JpmorganStoredCredential, Error> {
    Ok(requests::JpmorganStoredCredential {
        merchant_order_number: None,
        initiator_type: Some(requests::JpmorganInitiatorType::Cardholder),
        account_on_file: Some(if context.original_network_transaction_id.is_some() {
            requests::JpmorganAccountOnFile::Stored
        } else {
            requests::JpmorganAccountOnFile::ToBeStored
        }),
        recurring: if context.scheduled_recurring {
            Some(requests::JpmorganRecurring {
                recurring_sequence: requests::JpmorganRecurringSequence::First,
                agreement_id: context.agreement_id.clone().ok_or(
                    IntegrationError::MissingRequiredField {
                        field_name: "metadata.jpmorgan.agreementId",
                        context: utils::integration_ctx(
                            "Scheduled payments require the original recurring agreement",
                            "Supply the recurring agreement identifier",
                        ),
                    },
                )?,
                is_variable_amount: context.is_variable_amount,
                recurring_number: context.recurring_number,
            })
        } else {
            None
        },
        is_amount_final: Some(true),
        browser_info: None,
    })
}

fn native_three_ds(
    flow: &PaymentFlowData,
    browser: Option<&domain_types::router_request_types::BrowserInformation>,
    return_url: Option<&str>,
    email: Option<&common_utils::pii::Email>,
    network: Option<CardNetwork>,
    purpose: requests::JpmorganAuthenticationPurpose,
    transaction_type: requests::JpmorganThreeDsTransactionType,
) -> Result<
    (
        requests::JpmorganNativeAuthentication,
        requests::JpmorganBrowserInfo,
        requests::JpmorganAccountHolder,
    ),
    Error,
> {
    if network == Some(CardNetwork::CartesBancaires) {
        return Err(IntegrationError::NotImplemented(
            "JPMorgan threeDSRequestorAuthenticationInfo.authenticationUseCase".to_owned(),
            utils::integration_ctx("Cartes Bancaires native 3DS requires an explicit authentication use case and merchant risk score; these inputs are not mapped", "Supply these fields through a supported integration; do not substitute the issuer risk score"),
        )
        .into());
    }
    let return_url = return_url.ok_or_else(domain_types::utils::missing_field_err(
        "complete_authorize_url",
    ))?;
    let parsed =
        url::Url::parse(return_url).change_context(IntegrationError::InvalidDataFormat {
            field_name: "complete_authorize_url",
            context: utils::integration_ctx(
                "3DS continuation requires an absolute HTTPS URL",
                "Supply an absolute HTTPS completion URL",
            ),
        })?;
    if parsed.scheme() != "https" || parsed.host_str().is_none() {
        return Err(IntegrationError::InvalidDataFormat {
            field_name: "complete_authorize_url",
            context: utils::integration_ctx(
                "3DS continuation requires an absolute HTTPS URL",
                "Supply an absolute HTTPS completion URL",
            ),
        }
        .into());
    }
    let browser = browser.ok_or_else(domain_types::utils::missing_field_err("browser_info"))?;
    let browser_info = requests::JpmorganBrowserInfo {
        browser_accept_header: browser.get_accept_header()?,
        device_ip_address: browser.get_ip_address()?.switch_strategy(),
        browser_language: browser.get_language()?,
        browser_color_depth: browser.get_color_depth()?.to_string(),
        browser_screen_height: browser.get_screen_height()?.to_string(),
        browser_screen_width: browser.get_screen_width()?.to_string(),
        device_local_time_zone: browser.get_time_zone()?.to_string(),
        browser_user_agent: browser.get_user_agent()?,
        challenge_window_size: requests::JpmorganChallengeWindowSize::FullScreen,
        java_enabled: browser.get_java_enabled()?,
        java_script_enabled: browser.get_java_script_enabled()?,
    };
    let billing = flow
        .address
        .get_payment_method_billing()
        .or_else(|| flow.address.get_payment_billing())
        .ok_or_else(domain_types::utils::missing_field_err("billing"))?;
    let address = billing
        .address
        .as_ref()
        .ok_or_else(domain_types::utils::missing_field_err("billing.address"))?;
    let phone = billing
        .phone
        .as_ref()
        .ok_or_else(domain_types::utils::missing_field_err("billing.phone"))?;
    let holder = requests::JpmorganAccountHolder {
        first_name: address.get_first_name()?.clone(),
        last_name: address.get_last_name()?.clone(),
        email: Some(
            email
                .or(billing.email.as_ref())
                .cloned()
                .ok_or_else(domain_types::utils::missing_field_err("email"))?,
        ),
        billing_address: Some(requests::JpmorganBillingAddress {
            line1: Some(address.get_line1()?.clone()),
            city: Some(address.get_city()?.clone()),
            postal_code: Some(address.get_zip()?.clone()),
            country_code: Some(common_enums::CountryAlpha2::from_alpha2_to_alpha3(
                *address.get_country()?,
            )),
        }),
        phone: Some(requests::JpmorganPhone::try_from(phone)?),
    };
    let authentication = requests::JpmorganNativeAuthentication {
        authentication_return_url: return_url.to_owned(),
        requestor_info: requests::JpmorganRequestorInfo {
            authentication_purpose: purpose,
        },
        purchase_info: requests::JpmorganPurchaseInfo {
            purchase_date: common_utils::date_time::date_as_yyyymmddthhmmssmmmz().change_context(
                IntegrationError::RequestEncodingFailed {
                    context: utils::integration_ctx(
                        "The purchase timestamp cannot be formatted",
                        "Retry the request or inspect the server clock",
                    ),
                },
            )?,
            three_domain_secure_transaction_type: transaction_type,
        },
    };
    Ok((authentication, browser_info, holder))
}

impl TryFrom<&domain_types::payment_address::PhoneDetails> for requests::JpmorganPhone {
    type Error = Error;

    fn try_from(phone: &domain_types::payment_address::PhoneDetails) -> Result<Self, Self::Error> {
        let country_code = phone
            .country_code
            .as_deref()
            .map(|value| {
                value.trim_start_matches('+').parse::<u16>().change_context(
                    IntegrationError::InvalidDataFormat {
                        field_name: "billing.phone.country_code",
                        context: utils::integration_ctx(
                            "The phone country code must be a number from 1 to 999",
                            "Supply a numeric phone country code",
                        ),
                    },
                )
            })
            .transpose()?;
        if country_code.is_some_and(|code| code == 0 || code > 999) {
            return Err(IntegrationError::InvalidDataFormat {
                field_name: "billing.phone.country_code",
                context: utils::integration_ctx(
                    "The phone country code must be a number from 1 to 999",
                    "Supply a numeric phone country code",
                ),
            }
            .into());
        }
        Ok(Self {
            phone_number: phone.get_number()?.clone(),
            country_code,
        })
    }
}

fn apply_three_ds_authentication<T: PaymentMethodDataTypes>(
    card: &mut requests::JpmorganCard<T>,
    proof: &domain_types::router_request_types::AuthenticationData,
    network: Option<CardNetwork>,
) -> Result<(), Error> {
    if network == Some(CardNetwork::CartesBancaires) {
        return Err(IntegrationError::NotImplemented(
            "JPMorgan paymentMethodType.card.authentication.threeDS.threeDSChallengeType".to_owned(),
            utils::integration_ctx("Cartes Bancaires requires the requested challenge preference; authentication_data.challenge_code identifies an ACS challenge instead", "Use an integration that supplies the requested challenge preference without inferring it from the result"),
        )
        .into());
    }
    let cavv = proof
        .cavv
        .clone()
        .ok_or_else(domain_types::utils::missing_field_err(
            "authentication_data.cavv",
        ))?;
    if cavv.peek().is_empty()
        || cavv.peek().len() > 56
        || common_utils::consts::BASE64_ENGINE
            .decode(cavv.peek())
            .is_err()
    {
        return Err(IntegrationError::InvalidDataFormat {
            field_name: "authentication_data.cavv",
            context: utils::integration_ctx(
                "JPMorgan requires a Base64 authentication value of at most 56 characters",
                "Supply the authentication value returned by the authentication provider",
            ),
        }
        .into());
    }
    let eci = proof
        .eci
        .clone()
        .ok_or_else(domain_types::utils::missing_field_err(
            "authentication_data.eci",
        ))?;
    let allowed = match network {
        Some(CardNetwork::Mastercard | CardNetwork::Maestro) => Some(["00", "01", "02"]),
        Some(
            CardNetwork::Visa
            | CardNetwork::AmericanExpress
            | CardNetwork::Discover
            | CardNetwork::DinersClub
            | CardNetwork::JCB
            | CardNetwork::UnionPay,
        ) => Some(["05", "06", "07"]),
        _ => None,
    };
    if eci.is_empty()
        || eci.len() > 2
        || !eci.bytes().all(|byte| byte.is_ascii_digit())
        || allowed.is_some_and(|values| !values.contains(&eci.as_str()))
    {
        return Err(IntegrationError::InvalidDataFormat {
            field_name: "authentication_data.eci",
            context: utils::integration_ctx(
                "The supplied ECI is invalid for the card network",
                "Supply the ECI returned by the authentication provider",
            ),
        }
        .into());
    }
    // JPMorgan requires Visa XID or the Mastercard directory server ID.
    let transaction_id = proof.ds_trans_id.clone().filter(|id| !id.trim().is_empty());
    if matches!(
        network,
        Some(CardNetwork::Visa | CardNetwork::Mastercard | CardNetwork::Maestro)
    ) && transaction_id.is_none()
    {
        return Err(domain_types::utils::missing_field_err(
            "authentication_data.ds_trans_id",
        )());
    }
    let version = proof.message_version.as_ref().map(ToString::to_string);
    if matches!(
        network,
        Some(CardNetwork::Mastercard | CardNetwork::Maestro)
    ) && version.is_none()
    {
        return Err(domain_types::utils::missing_field_err(
            "authentication_data.message_version",
        )());
    }
    let authentication = card
        .authentication
        .get_or_insert(requests::JpmorganAuthentication {
            three_ds: None,
            token_authentication_value: None,
            electronic_commerce_indicator: None,
        });
    authentication.three_ds = Some(requests::JpmorganThreeDs {
        authentication_value: cavv,
        authentication_transaction_id: transaction_id,
        three_ds_program_protocol: version,
    });
    authentication.electronic_commerce_indicator = Some(eci);
    Ok(())
}

fn payment_network<T: PaymentMethodDataTypes>(
    payment_method: &PaymentMethodData<T>,
) -> Option<CardNetwork> {
    let (network, number): (Option<&CardNetwork>, &str) = match payment_method {
        PaymentMethodData::Card(card) => (card.card_network.as_ref(), card.card_number.peek()),
        PaymentMethodData::CardDetailsForNetworkTransactionId(card) => {
            (card.card_network.as_ref(), card.card_number.peek())
        }
        PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(wallet) => {
            (wallet.card_network.as_ref(), wallet.decrypted_token.peek())
        }
        PaymentMethodData::NetworkToken(token) => {
            (token.card_network.as_ref(), token.token_number.peek())
        }
        PaymentMethodData::Wallet(wallet) => {
            let network = match wallet {
                WalletData::GooglePay(data) => data.info.card_network.as_str(),
                WalletData::ApplePay(data) => data.payment_method.network.as_str(),
                _ => return None,
            };
            return serde_json::from_value(serde_json::Value::String(
                network.replace('_', "").to_ascii_uppercase(),
            ))
            .ok();
        }
        _ => return None,
    };
    network.cloned().or_else(|| {
        use domain_types::utils::CardIssuer;

        match domain_types::utils::get_card_issuer(number).ok()? {
            CardIssuer::AmericanExpress => Some(CardNetwork::AmericanExpress),
            CardIssuer::Master => Some(CardNetwork::Mastercard),
            CardIssuer::Maestro => Some(CardNetwork::Maestro),
            CardIssuer::Visa => Some(CardNetwork::Visa),
            CardIssuer::Discover => Some(CardNetwork::Discover),
            CardIssuer::DinersClub => Some(CardNetwork::DinersClub),
            CardIssuer::JCB => Some(CardNetwork::JCB),
            CardIssuer::CartesBancaires => Some(CardNetwork::CartesBancaires),
            CardIssuer::UnionPay => Some(CardNetwork::UnionPay),
            CardIssuer::CarteBlanche => None,
        }
    })
}

fn validate_recurring_context(
    context: &requests::JpmorganContext,
    network: Option<CardNetwork>,
) -> Result<(), Error> {
    if !context.scheduled_recurring {
        return Ok(());
    }
    if network == Some(CardNetwork::Mastercard) && context.is_variable_amount.is_none() {
        return Err(IntegrationError::MissingRequiredField {
            field_name: "metadata.jpmorgan.isVariableAmount",
            context: utils::integration_ctx(
                "Mastercard scheduled payments require the agreement amount type",
                "Supply isVariableAmount for the recurring agreement",
            ),
        }
        .into());
    }
    if network == Some(CardNetwork::CartesBancaires) && context.recurring_number.is_none() {
        return Err(IntegrationError::MissingRequiredField {
            field_name: "metadata.jpmorgan.recurringNumber",
            context: utils::integration_ctx(
                "Cartes Bancaires scheduled payments require a recurring payment number",
                "Supply recurringNumber for the recurring agreement",
            ),
        }
        .into());
    }
    if context.recurring_number == Some(0) {
        return Err(IntegrationError::InvalidDataFormat {
            field_name: "metadata.jpmorgan.recurringNumber",
            context: utils::integration_ctx(
                "The recurring payment number must be greater than zero",
                "Supply a positive recurring payment number",
            ),
        }
        .into());
    }
    Ok(())
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        JpmorganRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for requests::JpmorganCaptureRequest
{
    type Error = Error;
    fn try_from(
        item: JpmorganRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let capture_method = requests::CapMethod::Now;
        let amount_to_capture = item.router_data.request.minor_amount_to_capture;

        let amount =
            JpmorganAmountConvertor::convert(amount_to_capture, item.router_data.request.currency)?;

        // When AuthenticationType is `Manual`, Documentation suggests us to pass `isAmountFinal` field being `true`
        // isAmountFinal is by default `true`. Since Manual Multiple support is not added here, the field is not used.
        Ok(Self {
            capture_method,
            amount,
            currency: item.router_data.request.currency,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        JpmorganRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for requests::JpmorganVoidRequest
{
    type Error = Error;
    fn try_from(
        _item: JpmorganRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self { is_void: true })
    }
}

/// VoidPC (post-capture void/reversal) request transformer.
///
/// JPMorgan uses the same `PATCH /payments/{id}` endpoint with `{"isVoid": true}`
/// for both pre-capture void and post-capture reversal. The transaction ID is used
/// to build the URL in the connector implementation.
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        JpmorganRouterData<
            RouterDataV2<
                VoidPC,
                PaymentFlowData,
                PaymentsCancelPostCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for requests::JpmorganVoidPcRequest
{
    type Error = Error;
    fn try_from(
        _item: JpmorganRouterData<
            RouterDataV2<
                VoidPC,
                PaymentFlowData,
                PaymentsCancelPostCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self { is_void: true })
    }
}

impl<F> TryFrom<ResponseRouterData<responses::JpmorganPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Map JPMorgan's transaction state directly to `PostCaptureVoidStatus` —
        // the Reverse flow has its own status enum, so we don't go through
        // `AttemptStatus`. `Closed` / `Authorized` mean the PATCH was accepted
        // but the transaction did not move to `Voided`, so the reversal did
        // not apply. `Declined` / `Error` are matched explicitly to keep the
        // match exhaustive.
        let post_capture_void_status = match item.response.transaction_state {
            responses::JpmorganTransactionState::Voided => {
                common_enums::PostCaptureVoidStatus::Succeeded
            }
            responses::JpmorganTransactionState::Pending => {
                common_enums::PostCaptureVoidStatus::Pending
            }
            responses::JpmorganTransactionState::Closed
            | responses::JpmorganTransactionState::Authorized
            | responses::JpmorganTransactionState::Declined
            | responses::JpmorganTransactionState::Error => {
                common_enums::PostCaptureVoidStatus::Failed
            }
        };

        let response = if post_capture_void_status.is_post_capture_void_failure() {
            Err(ErrorResponse {
                attempt_status: None,
                code: item.response.response_code.clone(),
                message: item
                    .response
                    .response_message
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: item.response.response_message.clone(),
                status_code: item.http_code,
                connector_transaction_id: Some(item.response.transaction_id.clone()),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            })
        } else {
            Ok(PaymentsResponseData::PostCaptureVoidResponse {
                post_capture_void_status,
                connector_reference_id: Some(item.response.transaction_id.clone()),
                description: None,
                status_code: item.http_code,
            })
        };

        Ok(Self {
            response,
            ..item.router_data
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        JpmorganRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for requests::JpmorganRefundRequest
{
    type Error = Error;
    fn try_from(
        item: JpmorganRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = JpmorganAuthType::try_from(&item.router_data.connector_config)?;

        let merchant = requests::JpmorganMerchantRefund {
            merchant_software: requests::JpmorganMerchantSoftware {
                company_name: auth
                    .company_name
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "company_name",
                        context: Default::default(),
                    })?,
                product_name: auth
                    .product_name
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "product_name",
                        context: Default::default(),
                    })?,
            },
        };

        let amount = JpmorganAmountConvertor::convert(
            item.router_data.request.minor_refund_amount,
            item.router_data.request.currency,
        )?;

        Ok(Self {
            merchant,
            amount,
            currency: item.router_data.request.currency,
        })
    }
}

fn map_transaction_state_to_attempt_status(
    transaction_state: &responses::JpmorganTransactionState,
    capture_method: &Option<requests::CapMethod>,
) -> AttemptStatus {
    match transaction_state {
        responses::JpmorganTransactionState::Closed => match capture_method {
            Some(requests::CapMethod::Now) => AttemptStatus::Charged,
            _ => AttemptStatus::Authorized,
        },
        responses::JpmorganTransactionState::Authorized => AttemptStatus::Authorized,
        responses::JpmorganTransactionState::Declined
        | responses::JpmorganTransactionState::Error => AttemptStatus::Failure,
        responses::JpmorganTransactionState::Pending => AttemptStatus::Pending,
        responses::JpmorganTransactionState::Voided => AttemptStatus::Voided,
    }
}

impl TryFrom<&responses::JpmorganPaymentsResponse> for PaymentsResponseData {
    type Error = ResponseError;
    fn try_from(item: &responses::JpmorganPaymentsResponse) -> Result<Self, Self::Error> {
        // Extract networkTransactionId from card.networkResponse for MIT flows
        let network_txn_id = item
            .payment_method_type
            .as_ref()
            .and_then(|pmt| pmt.card.as_ref())
            .and_then(|card| card.network_response.as_ref())
            .and_then(|nr| nr.network_transaction_id.clone());

        Ok(Self::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(item.transaction_id.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id,
            network_txn_link_id: item
                .payment_method_type
                .as_ref()
                .and_then(|pmt| pmt.card.as_ref())
                .and_then(|card| card.network_response.as_ref())
                .and_then(|network| network.transaction_link_id.clone()),
            connector_response_reference_id: Some(item.request_id.clone()),
            incremental_authorization_allowed: None,
            status_code: item.response_code.parse::<u16>().unwrap_or(0),
            splits: None,
            payment_account_reference: None,
        })
    }
}

impl TryFrom<&responses::JpmorganPaymentsResponse> for AttemptStatus {
    type Error = ResponseError;
    fn try_from(item: &responses::JpmorganPaymentsResponse) -> Result<Self, Self::Error> {
        Ok(payment_status_with_capture_intent(item, None))
    }
}

fn payment_status_with_capture_intent(
    response: &responses::JpmorganPaymentsResponse,
    capture_intent: Option<requests::CapMethod>,
) -> AttemptStatus {
    if response.response_status != responses::JpmorganTransactionStatus::Success {
        return AttemptStatus::Failure;
    }
    // Native GET can omit captureMethod. Keep the original intent, not a callback default.
    let capture_method = response.capture_method.or(capture_intent);
    authentication_status(
        response.payment_authentication_result.as_ref(),
        map_transaction_state_to_attempt_status(&response.transaction_state, &capture_method),
    )
}

fn authentication_status(
    result: Option<&responses::JpmorganAuthenticationResult>,
    status: AttemptStatus,
) -> AttemptStatus {
    if is_payment_failure(status) {
        return status;
    }
    match result.and_then(|result| result.three_domain_secure_completion.as_ref()) {
        Some(completion) => match completion.three_ds_transaction_status {
            responses::JpmorganThreeDsStatus::Authenticated
            | responses::JpmorganThreeDsStatus::Attempted => status,
            responses::JpmorganThreeDsStatus::ChallengeRequired
            | responses::JpmorganThreeDsStatus::DecoupledAuthentication
            | responses::JpmorganThreeDsStatus::InformationalOnly => {
                AttemptStatus::AuthenticationPending
            }
            responses::JpmorganThreeDsStatus::NotAuthenticated
            | responses::JpmorganThreeDsStatus::Unavailable
            | responses::JpmorganThreeDsStatus::Rejected
            | responses::JpmorganThreeDsStatus::Unknown => AttemptStatus::Failure,
        },
        None if result.is_some_and(|result| result.authentication_orchestration_url.is_some()) => {
            AttemptStatus::AuthenticationPending
        }
        None => status,
    }
}

pub fn validate_three_ds_resource(
    resource: &requests::JpmorganThreeDsResource,
    flow: &PaymentFlowData,
) -> Result<(), Error> {
    if resource.id.is_empty()
        || !resource
            .id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        || resource.merchant_id != flow.merchant_id.get_string_repr()
    {
        return Err(IntegrationError::InvalidDataFormat {
            field_name: "connector_feature_data.jpmorgan.threeDsResource",
            context: utils::integration_ctx(
                "The saved resource is invalid or belongs to another merchant",
                "Keep the returned resource and merchant identifiers unchanged",
            ),
        }
        .into());
    }
    Ok(())
}

fn finish_authentication(
    response: &mut Result<PaymentsResponseData, ErrorResponse>,
    mut context: requests::JpmorganContext,
    authentication: Option<&responses::JpmorganAuthenticationResult>,
    kind: requests::JpmorganResourceKind,
    flow: &PaymentFlowData,
    status: AttemptStatus,
    http_code: u16,
) -> Result<(), ResponseError> {
    context.continue_three_ds = false;
    if status != AttemptStatus::AuthenticationPending {
        if matches!(status, AttemptStatus::Authorized | AttemptStatus::Charged) {
            return attach_original_context(response, context, http_code);
        }
        if let Ok(PaymentsResponseData::TransactionResponse {
            mandate_reference,
            network_txn_id,
            network_txn_link_id,
            connector_metadata,
            ..
        }) = response
        {
            *mandate_reference = None;
            *network_txn_id = None;
            *network_txn_link_id = None;
            *connector_metadata = Some(serde_json::json!({"jpmorgan": context}));
        }
        return Ok(());
    }
    if let Ok(PaymentsResponseData::TransactionResponse {
        resource_id,
        redirection_data,
        connector_metadata,
        mandate_reference,
        network_txn_id,
        network_txn_link_id,
        ..
    }) = response
    {
        let id = resource_id
            .get_connector_transaction_id()
            .change_context(ConnectorError::response_handling_failed(http_code))?;
        let resource = requests::JpmorganThreeDsResource {
            kind,
            id,
            merchant_id: flow.merchant_id.get_string_repr().to_owned(),
        };
        validate_three_ds_resource(&resource, flow)
            .change_context(ConnectorError::response_handling_failed(http_code))?;
        context.three_ds_resource = Some(resource);
        *mandate_reference = None;
        *network_txn_id = None;
        *network_txn_link_id = None;
        *redirection_data = authentication
            .and_then(|result| result.authentication_orchestration_url.as_ref())
            .map(|endpoint| -> Result<_, ResponseError> {
                let url = url::Url::parse(endpoint)
                    .change_context(ConnectorError::response_handling_failed(http_code))?;
                if url.scheme() != "https" || url.host_str().is_none() {
                    return Err(ConnectorError::response_handling_failed(http_code).into());
                }
                Ok(Box::new(
                    domain_types::router_response_types::RedirectForm::Form {
                        endpoint: endpoint.clone(),
                        method: common_utils::request::Method::Get,
                        form_fields: std::collections::HashMap::new(),
                    },
                ))
            })
            .transpose()?;
        *connector_metadata = Some(serde_json::json!({"jpmorgan": context}));
    }
    Ok(())
}

fn verification_response(
    response: &responses::JpmorganSetupMandateResponse,
    http_code: u16,
) -> (AttemptStatus, Result<PaymentsResponseData, ErrorResponse>) {
    // Charged represents terminal nonfinancial setup, not a funds movement.
    let status = authentication_status(
        response.verification_authentication_result.as_ref(),
        match response.response_status {
            responses::JpmorganVerificationStatus::Success => AttemptStatus::Charged,
            responses::JpmorganVerificationStatus::Denied
            | responses::JpmorganVerificationStatus::Error
            | responses::JpmorganVerificationStatus::Unknown => AttemptStatus::Failure,
        },
    );
    let result = if is_payment_failure(status) {
        Err(ErrorResponse {
            attempt_status: Some(FlowStatus::Payment(status)),
            code: response.response_code.clone(),
            message: response
                .response_message
                .clone()
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_owned()),
            reason: response.response_message.clone(),
            status_code: http_code,
            connector_transaction_id: Some(response.transaction_id.clone()),
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
            typed_connector_response: None,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        })
    } else {
        let network = response
            .payment_method_type
            .as_ref()
            .and_then(|method| method.card.as_ref())
            .and_then(|card| card.network_response.as_ref());
        Ok(PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(response.transaction_id.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: network.and_then(|network| network.network_transaction_id.clone()),
            network_txn_link_id: network.and_then(|network| network.transaction_link_id.clone()),
            connector_response_reference_id: Some(response.request_id.clone()),
            incremental_authorization_allowed: None,
            status_code: http_code,
            splits: None,
            payment_account_reference: None,
        })
    };
    (status, result)
}

/// Build the `response` field for a JPMorgan payments flow: `Err(ErrorResponse)`
/// when the transaction was declined/errored, otherwise `Ok(PaymentsResponseData)`.
fn build_payments_response_result(
    response: &responses::JpmorganPaymentsResponse,
    http_code: u16,
    status: AttemptStatus,
) -> Result<Result<PaymentsResponseData, ErrorResponse>, ResponseError> {
    if is_payment_failure(status) {
        Ok(Err(ErrorResponse {
            attempt_status: Some(FlowStatus::Payment(status)),
            code: response.response_code.clone(),
            message: response
                .response_message
                .clone()
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            reason: response.response_message.clone(),
            status_code: http_code,
            connector_transaction_id: Some(response.transaction_id.clone()),
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
            typed_connector_response: None,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        }))
    } else {
        Ok(Ok(PaymentsResponseData::try_from(response)?))
    }
}

fn attach_original_context(
    response: &mut Result<PaymentsResponseData, ErrorResponse>,
    mut context: requests::JpmorganContext,
    http_code: u16,
) -> Result<(), ResponseError> {
    if let Ok(PaymentsResponseData::TransactionResponse {
        connector_metadata,
        mandate_reference,
        network_txn_id,
        network_txn_link_id,
        ..
    }) = response
    {
        if context.original_network_transaction_id.is_none() {
            context
                .original_network_transaction_id
                .clone_from(network_txn_id);
            context
                .original_transaction_link_id
                .clone_from(network_txn_link_id);
        }
        let metadata = serde_json::to_value(&context)
            .change_context(ConnectorError::response_handling_failed(http_code))?;
        let metadata = serde_json::json!({ "jpmorgan": metadata });
        *connector_metadata = Some(metadata.clone());
        if network_txn_id.is_some() {
            // Reusable credential state must not carry an attempt's redirect resource.
            context.three_ds_resource = None;
            context.native_capture_method = None;
            context.continue_three_ds = false;
            *mandate_reference = Some(Box::new(MandateReference {
                connector_mandate_id: None,
                payment_method_id: None,
                connector_mandate_request_reference_id: None,
                mandate_metadata: Some(Secret::new(serde_json::json!({"jpmorgan": context}))),
            }));
        }
    }
    Ok(())
}

impl TryFrom<&responses::JpmorganRefundResponse> for RefundsResponseData {
    type Error = ResponseError;
    fn try_from(item: &responses::JpmorganRefundResponse) -> Result<Self, Self::Error> {
        let refund_status = responses::RefundStatus::from((
            item.response_status.clone(),
            item.transaction_state.clone(),
        ))
        .into();

        Ok(Self {
            connector_refund_id: item.transaction_id.clone(),
            refund_status,
            status_code: item.response_code.parse::<u16>().unwrap_or(0),
            acquirer_reference_number: None,
        })
    }
}

// Bridge pattern implementations for RouterDataV2

impl<T: PaymentMethodDataTypes, F>
    TryFrom<ResponseRouterData<responses::JpmorganPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let request = &item.router_data.request;
        let operation_context = resolved_connector_context(
            request.connector_feature_data.as_ref(),
            item.router_data
                .resource_common_data
                .connector_feature_data
                .as_ref(),
            request.metadata.as_ref(),
        )
        .change_context(ConnectorError::response_handling_failed(item.http_code))?;
        let capture_intent = if (request.redirect_response.is_some()
            || operation_context.continue_three_ds)
            && item.response.capture_method.is_none()
            && payment_status_with_capture_intent(&item.response, None) == AttemptStatus::Authorized
            && matches!(
                item.response.transaction_state,
                responses::JpmorganTransactionState::Closed
            )
            && operation_context
                .three_ds_resource
                .as_ref()
                .is_some_and(|resource| resource.kind == requests::JpmorganResourceKind::Payment)
        {
            Some(
                operation_context
                    .native_capture_method
                    .map_or_else(|| map_capture_method(request.capture_method), Ok)
                    .change_context(ConnectorError::response_handling_failed(item.http_code))?,
            )
        } else {
            None
        };
        let status = payment_status_with_capture_intent(&item.response, capture_intent);
        let mut response = build_payments_response_result(&item.response, item.http_code, status)?;
        if request.setup_future_usage.is_some()
            || request.setup_mandate_details.is_some()
            || request.mit_category.is_some()
            || item.router_data.resource_common_data.auth_type
                == common_enums::AuthenticationType::ThreeDs
            || request.redirect_response.is_some()
            || request.connector_feature_data.is_some()
            || item
                .router_data
                .resource_common_data
                .connector_feature_data
                .is_some()
        {
            let mut context = cit_context(
                request.connector_feature_data.as_ref().or(item
                    .router_data
                    .resource_common_data
                    .connector_feature_data
                    .as_ref()),
                request.metadata.as_ref(),
                request.mit_category.as_ref(),
                request.merchant_order_id.as_deref(),
                &item
                    .router_data
                    .resource_common_data
                    .connector_request_reference_id,
            )
            .change_context(ConnectorError::response_handling_failed(item.http_code))?;
            if request.redirect_response.is_none() && !context.continue_three_ds {
                context.three_ds_resource = None;
                context.native_capture_method = None;
                if item.router_data.resource_common_data.auth_type
                    == common_enums::AuthenticationType::ThreeDs
                    && request.authentication_data.is_none()
                {
                    context.native_capture_method =
                        Some(map_capture_method(request.capture_method).change_context(
                            ConnectorError::response_handling_failed(item.http_code),
                        )?);
                }
            }
            if context.three_ds_resource.is_none() {
                if let PaymentMethodData::Wallet(wallet) = &request.payment_method_data {
                    let is_decrypted = match wallet {
                        WalletData::GooglePay(data) => matches!(
                            data.tokenization_data,
                            domain_types::payment_method_data::GpayTokenizationData::Decrypted(_)
                        ),
                        WalletData::ApplePay(data) => data
                            .payment_data
                            .get_decrypted_apple_pay_payment_data_optional()
                            .is_some(),
                        _ => false,
                    };
                    if is_decrypted {
                        let card = decrypted_wallet_card::<T>(wallet).change_context(
                            ConnectorError::response_handling_failed(item.http_code),
                        )?;
                        context.account_number_type = card.account_number_type;
                        context.wallet_provider = card.wallet_provider;
                    }
                }
            }
            finish_authentication(
                &mut response,
                context,
                item.response.payment_authentication_result.as_ref(),
                requests::JpmorganResourceKind::Payment,
                &item.router_data.resource_common_data,
                status,
                item.http_code,
            )?;
        }

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl<T: PaymentMethodDataTypes, F>
    TryFrom<ResponseRouterData<responses::JpmorganAuthorizeResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let mut context = resolved_connector_context(
            item.router_data.request.connector_feature_data.as_ref(),
            item.router_data
                .resource_common_data
                .connector_feature_data
                .as_ref(),
            item.router_data.request.metadata.as_ref(),
        )
        .change_context(ConnectorError::response_handling_failed(item.http_code))?;
        let continuing =
            item.router_data.request.redirect_response.is_some() || context.continue_three_ds;
        let expected = if continuing {
            let resource = context
                .three_ds_resource
                .as_ref()
                .ok_or_else(|| ConnectorError::response_handling_failed(item.http_code))?;
            validate_three_ds_resource(resource, &item.router_data.resource_common_data)
                .change_context(ConnectorError::response_handling_failed(item.http_code))?;
            let returned_id = match &item.response {
                responses::JpmorganAuthorizeResponse::Payment(response) => &response.transaction_id,
                responses::JpmorganAuthorizeResponse::Verification(response) => {
                    &response.transaction_id
                }
            };
            if returned_id != &resource.id {
                return Err(ConnectorError::response_handling_failed(item.http_code).into());
            }
            resource.kind
        } else {
            context.three_ds_resource = None;
            requests::JpmorganResourceKind::Payment
        };
        match item.response {
            responses::JpmorganAuthorizeResponse::Payment(response)
                if expected == requests::JpmorganResourceKind::Payment =>
            {
                Self::try_from(ResponseRouterData {
                    response,
                    router_data: item.router_data,
                    http_code: item.http_code,
                })
            }
            responses::JpmorganAuthorizeResponse::Verification(response)
                if expected == requests::JpmorganResourceKind::Verification =>
            {
                let (status, mut result) = verification_response(&response, item.http_code);
                finish_authentication(
                    &mut result,
                    context,
                    response.verification_authentication_result.as_ref(),
                    expected,
                    &item.router_data.resource_common_data,
                    status,
                    item.http_code,
                )?;
                Ok(Self {
                    response: result,
                    resource_common_data: PaymentFlowData {
                        status,
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                })
            }
            responses::JpmorganAuthorizeResponse::Payment(_)
            | responses::JpmorganAuthorizeResponse::Verification(_) => {
                Err(ConnectorError::response_handling_failed(item.http_code).into())
            }
        }
    }
}

impl<F> TryFrom<ResponseRouterData<responses::JpmorganAuthorizeResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let context = resolved_connector_context(
            item.router_data.request.connector_feature_data.as_ref(),
            item.router_data
                .resource_common_data
                .connector_feature_data
                .as_ref(),
            None,
        )
        .change_context(ConnectorError::response_handling_failed(item.http_code))?;
        let requested_id = item
            .router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(ConnectorError::response_handling_failed(item.http_code))?;
        let returned_id = match &item.response {
            responses::JpmorganAuthorizeResponse::Payment(response) => &response.transaction_id,
            responses::JpmorganAuthorizeResponse::Verification(response) => {
                &response.transaction_id
            }
        };
        if returned_id != &requested_id {
            return Err(ConnectorError::response_handling_failed(item.http_code).into());
        }
        let expected = context
            .three_ds_resource
            .as_ref()
            .map_or(requests::JpmorganResourceKind::Payment, |resource| {
                resource.kind
            });
        let (status, mut response) = match &item.response {
            responses::JpmorganAuthorizeResponse::Payment(response)
                if expected == requests::JpmorganResourceKind::Payment =>
            {
                let capture_intent = if context.three_ds_resource.is_some()
                    && response.capture_method.is_none()
                    && payment_status_with_capture_intent(response, None)
                        == AttemptStatus::Authorized
                    && matches!(
                        response.transaction_state,
                        responses::JpmorganTransactionState::Closed
                    ) {
                    Some(
                        context
                            .native_capture_method
                            .map_or_else(
                                || map_capture_method(item.router_data.request.capture_method),
                                Ok,
                            )
                            .change_context(ConnectorError::response_handling_failed(
                                item.http_code,
                            ))?,
                    )
                } else {
                    None
                };
                let status = payment_status_with_capture_intent(response, capture_intent);
                (
                    status,
                    build_payments_response_result(response, item.http_code, status)?,
                )
            }
            responses::JpmorganAuthorizeResponse::Verification(response)
                if expected == requests::JpmorganResourceKind::Verification =>
            {
                verification_response(response, item.http_code)
            }
            responses::JpmorganAuthorizeResponse::Payment(_)
            | responses::JpmorganAuthorizeResponse::Verification(_) => {
                return Err(ConnectorError::response_handling_failed(item.http_code).into())
            }
        };
        let authentication = match &item.response {
            responses::JpmorganAuthorizeResponse::Payment(response) => {
                response.payment_authentication_result.as_ref()
            }
            responses::JpmorganAuthorizeResponse::Verification(response) => {
                response.verification_authentication_result.as_ref()
            }
        };
        if context.three_ds_resource.is_some() || status == AttemptStatus::AuthenticationPending {
            finish_authentication(
                &mut response,
                context,
                authentication,
                expected,
                &item.router_data.resource_common_data,
                status,
                item.http_code,
            )?;
        }

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<responses::JpmorganPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::try_from(&item.response)?;
        let response = build_payments_response_result(&item.response, item.http_code, status)?;

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<responses::JpmorganPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::try_from(&item.response)?;
        let response = build_payments_response_result(&item.response, item.http_code, status)?;

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<responses::JpmorganRefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = responses::RefundStatus::from((
            item.response.response_status.clone(),
            item.response.transaction_state.clone(),
        ))
        .into();
        let response_data = RefundsResponseData::try_from(&item.response)?;

        Ok(Self {
            response: Ok(response_data),
            resource_common_data: RefundFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<responses::JpmorganRefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = responses::RefundStatus::from((
            item.response.response_status.clone(),
            item.response.transaction_state.clone(),
        ))
        .into();
        let response_data = RefundsResponseData::try_from(&item.response)?;

        Ok(Self {
            response: Ok(response_data),
            resource_common_data: RefundFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ---- ClientAuthenticationToken flow types ----

/// Obtains an OAuth2 access token from JPMorgan for client-side SDK initialization.
/// The access_token serves as the client authentication token.
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        JpmorganRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                MerchantAuthenticationFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for requests::JpmorganClientAuthRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: JpmorganRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                MerchantAuthenticationFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let scope = if item
            .router_data
            .resource_common_data
            .test_mode
            .unwrap_or(true)
        {
            String::from("jpm:payments:sandbox")
        } else {
            String::from("jpm:payments")
        };
        Ok(Self {
            grant_type: String::from("client_credentials"),
            scope,
        })
    }
}

impl TryFrom<ResponseRouterData<responses::JpmorganClientAuthResponse, Self>>
    for RouterDataV2<
        ClientAuthenticationToken,
        MerchantAuthenticationFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganClientAuthResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        let session_data = ClientAuthenticationTokenData::ConnectorSpecific(Box::new(
            ConnectorSpecificClientAuthenticationResponse::Jpmorgan(
                JpmorganClientAuthenticationResponseDomain {
                    transaction_id: response.access_token.peek().to_string(),
                    request_id: response.token_type,
                },
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

impl TryFrom<&JpmorganAuthType> for requests::JpmorganMerchant {
    type Error = Error;
    fn try_from(auth: &JpmorganAuthType) -> Result<Self, Self::Error> {
        Ok(Self {
            merchant_software: requests::JpmorganMerchantSoftware {
                company_name: auth.company_name.clone().ok_or(
                    IntegrationError::MissingRequiredField {
                        field_name: "company_name",
                        context: jpmorgan_missing_field_context("company_name"),
                    },
                )?,
                product_name: auth.product_name.clone().ok_or(
                    IntegrationError::MissingRequiredField {
                        field_name: "product_name",
                        context: jpmorgan_missing_field_context("product_name"),
                    },
                )?,
            },
            soft_merchant: auth.merchant_purchase_description.clone().map(|d| {
                requests::JpmorganSoftMerchant {
                    merchant_purchase_description: d,
                }
            }),
        })
    }
}

fn build_jpmorgan_expiry(
    month: &Secret<String>,
    year: &Secret<String>,
) -> Result<requests::Expiry, Error> {
    let month =
        month
            .peek()
            .parse::<i32>()
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "payment_method_data.expiry_month",
                context: utils::integration_ctx(
                    "JPMorgan requires a numeric expiry month",
                    "Supply an expiry month from 1 to 12",
                ),
            })?;
    let year = utils::pad_expiry_year_to_four_digits(year)
        .peek()
        .parse::<i32>()
        .change_context(IntegrationError::InvalidDataFormat {
            field_name: "payment_method_data.expiry_year",
            context: utils::integration_ctx(
                "JPMorgan requires a numeric expiry year",
                "Supply a two-digit or four-digit expiry year",
            ),
        })?;
    if !(1..=12).contains(&month) || !(2018..=2999).contains(&year) {
        return Err(IntegrationError::InvalidDataFormat {
            field_name: "payment_method_data.expiry",
            context: utils::integration_ctx(
                "The expiry is outside the JPMorgan schema limits",
                "Supply a month from 1 to 12 and a year from 2018 to 2999",
            ),
        }
        .into());
    }
    Ok(requests::Expiry {
        month: Secret::new(month),
        year: Secret::new(year),
    })
}

// SetupMandate (initial CIT with credential storage) request transformer
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        JpmorganRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for requests::JpmorganSetupMandateRequest<T>
{
    type Error = Error;
    fn try_from(
        item: JpmorganRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        let request = &router_data.request;
        if request.amount.is_some_and(|amount| amount != 0)
            || request
                .minor_amount
                .is_some_and(|amount| amount.get_amount_as_i64() != 0)
        {
            return Err(IntegrationError::NotSupported {
                message: "SetupMandate requires a zero amount".to_owned(),
                connector: "jpmorgan",
                context: utils::integration_ctx(
                    "SetupMandate performs a non-charging verification",
                    "Use a zero amount for SetupMandate or Authorize for a payment",
                ),
            }
            .into());
        }
        let mut card = match &request.payment_method_data {
            PaymentMethodData::Card(card) => requests::JpmorganCard {
                account_number: requests::JpmorganAccountNumber::Card(card.card_number.clone()),
                expiry: build_jpmorgan_expiry(&card.card_exp_month, &card.card_exp_year)?,
                account_number_type: None,
                wallet_provider: None,
                authentication: None,
                original_network_transaction_id: None,
                original_transaction_link_id: None,
                payment_authentication_request: None,
                verification_authentication_request: None,
            },
            PaymentMethodData::Wallet(wallet) => decrypted_wallet_card(wallet)?,
            _ => {
                return Err(IntegrationError::NotImplemented(
                    "JPMorgan SetupMandate requires a card or decrypted wallet".to_owned(),
                    utils::integration_ctx(
                        "The payment method cannot be used for account verification",
                        "Supply card or decrypted wallet credentials",
                    ),
                )
                .into())
            }
        };
        if let Some(proof) = &request.authentication_data {
            apply_three_ds_authentication(
                &mut card,
                proof,
                payment_network(&request.payment_method_data),
            )?;
        }
        let context = cit_context(
            None,
            request.metadata.as_ref(),
            request.mit_category.as_ref(),
            request.merchant_order_id.as_deref(),
            &router_data
                .resource_common_data
                .connector_request_reference_id,
        )?;
        validate_recurring_context(&context, payment_network(&request.payment_method_data))?;
        let (browser_info, account_holder) = if router_data.resource_common_data.auth_type
            == common_enums::AuthenticationType::ThreeDs
            && request.authentication_data.is_none()
        {
            let (authentication, browser, holder) = native_three_ds(
                &router_data.resource_common_data,
                request.browser_info.as_ref(),
                request.complete_authorize_url.as_deref(),
                request.email.as_ref(),
                payment_network(&request.payment_method_data),
                requests::JpmorganAuthenticationPurpose::AddCard,
                requests::JpmorganThreeDsTransactionType::Check,
            )?;
            card.verification_authentication_request = Some(authentication);
            (Some(browser), Some(holder))
        } else {
            (None, None)
        };
        let auth = JpmorganAuthType::try_from(&router_data.connector_config)?;
        Ok(Self {
            currency: request.currency,
            merchant: requests::JpmorganMerchant::try_from(&auth)?,
            payment_method_type: requests::JpmorganPaymentMethodType {
                card: Some(card),
                ach: None,
                googlepay: None,
                token: None,
            },
            recurring_sequence: context
                .scheduled_recurring
                .then_some(requests::JpmorganRecurringSequence::First),
            initiator_type: requests::JpmorganInitiatorType::Cardholder,
            account_on_file: requests::JpmorganAccountOnFile::ToBeStored,
            browser_info,
            account_holder,
        })
    }
}

// SetupMandate response transformer
impl<T: PaymentMethodDataTypes>
    TryFrom<ResponseRouterData<responses::JpmorganSetupMandateResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganSetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let (status, mut response) = verification_response(&item.response, item.http_code);
        if !is_payment_failure(status) {
            let request = &item.router_data.request;
            let mut context = cit_context(
                None,
                request.metadata.as_ref(),
                request.mit_category.as_ref(),
                request.merchant_order_id.as_deref(),
                &item
                    .router_data
                    .resource_common_data
                    .connector_request_reference_id,
            )
            .change_context(ConnectorError::response_handling_failed(item.http_code))?;
            if let PaymentMethodData::Wallet(wallet) = &request.payment_method_data {
                let card = decrypted_wallet_card::<T>(wallet)
                    .change_context(ConnectorError::response_handling_failed(item.http_code))?;
                context.account_number_type = card.account_number_type;
                context.wallet_provider = card.wallet_provider;
            }
            finish_authentication(
                &mut response,
                context,
                item.response.verification_authentication_result.as_ref(),
                requests::JpmorganResourceKind::Verification,
                &item.router_data.resource_common_data,
                status,
                item.http_code,
            )?;
        }

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

fn repeat_context<T: PaymentMethodDataTypes>(
    request: &RepeatPaymentData<T>,
) -> Result<requests::JpmorganContext, Error> {
    let mandate_metadata = match &request.mandate_reference {
        MandateReferenceId::ConnectorMandateId(reference) => reference.get_mandate_metadata(),
        MandateReferenceId::NetworkMandateId(_) | MandateReferenceId::NetworkTokenWithNTI(_) => {
            None
        }
    };
    let mut context = connector_context(
        request
            .connector_feature_data
            .as_ref()
            .or(mandate_metadata.as_ref()),
        request.metadata.as_ref(),
    )?;
    context.three_ds_resource = None;
    context.native_capture_method = None;
    context.continue_three_ds = false;
    let (network_id, link_id) = match &request.mandate_reference {
        MandateReferenceId::NetworkMandateId(reference) => (
            Some(&reference.network_transaction_id),
            reference.transaction_link_id.as_ref(),
        ),
        MandateReferenceId::NetworkTokenWithNTI(reference) => (
            Some(&reference.network_transaction_id),
            reference.transaction_link_id.as_ref(),
        ),
        // A payment transaction ID is not a reusable credential.
        MandateReferenceId::ConnectorMandateId(_) => (None, None),
    };
    if let Some(network_id) = network_id {
        if network_id.trim().is_empty()
            || context
                .original_network_transaction_id
                .as_ref()
                .is_some_and(|original| original != network_id)
        {
            return Err(IntegrationError::InvalidDataFormat {
                field_name: "original_network_transaction_id",
                context: utils::integration_ctx("The supplied network transaction identifier conflicts with the original transaction", "Use the network transaction identifier from the original transaction"),
            }.into());
        }
        context.original_network_transaction_id = Some(network_id.clone());
    }
    if context
        .original_network_transaction_id
        .as_ref()
        .is_none_or(|id| id.trim().is_empty())
    {
        return Err(IntegrationError::MissingRequiredField {
            field_name: "original_network_transaction_id",
            context: utils::integration_ctx(
                "A merchant-initiated payment requires the original network transaction reference",
                "Supply the network transaction identifier from the original transaction",
            ),
        }
        .into());
    }
    if let Some(link_id) = link_id {
        if context
            .original_transaction_link_id
            .as_ref()
            .is_some_and(|original| original != link_id)
        {
            return Err(IntegrationError::InvalidDataFormat {
                field_name: "original_transaction_link_id",
                context: utils::integration_ctx("The supplied transaction link identifier conflicts with the original transaction", "Supply the original transaction link identifier"),
            }.into());
        }
        context.original_transaction_link_id = Some(link_id.clone());
    }
    if context
        .original_transaction_link_id
        .as_ref()
        .is_some_and(|id| {
            id.len() != 22
                || !id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        })
    {
        return Err(IntegrationError::InvalidDataFormat {
            field_name: "original_transaction_link_id",
            context: utils::integration_ctx("The transaction link identifier must contain 22 letters, digits, hyphens, or underscores", "Supply the original 22-character transaction link identifier"),
        }.into());
    }
    match request.mit_category.as_ref() {
        Some(common_enums::MitCategory::Recurring) => context.scheduled_recurring = true,
        Some(common_enums::MitCategory::Unscheduled) => context.scheduled_recurring = false,
        None => {}
        Some(common_enums::MitCategory::Installment | common_enums::MitCategory::Resubmission) => {
            return Err(IntegrationError::NotImplemented(
                "JPMorgan installment and resubmission mandates".to_owned(),
                utils::integration_ctx(
                    "This connector does not implement installment or resubmission payments",
                    "Use recurring or unscheduled payments",
                ),
            )
            .into());
        }
    }
    if context.scheduled_recurring
        && context
            .agreement_id
            .as_ref()
            .is_none_or(|id| id.is_empty() || id.len() > 100)
    {
        return Err(IntegrationError::MissingRequiredField {
            field_name: "metadata.jpmorgan.agreementId",
            context: utils::integration_ctx(
                "Scheduled payments require the original recurring agreement",
                "Supply the original recurring agreement identifier",
            ),
        }
        .into());
    }
    let network = payment_network(&request.payment_method_data);
    if network == Some(CardNetwork::Mastercard) && context.original_transaction_link_id.is_none() {
        return Err(IntegrationError::MissingRequiredField {
            field_name: "original_transaction_link_id",
            context: utils::integration_ctx("Mastercard merchant-initiated payments require the original transaction link identifier", "Supply the transaction link identifier returned for the original transaction"),
        }
        .into());
    }
    validate_recurring_context(&context, network)?;
    Ok(context)
}

fn mit_wallet_type(
    context: &requests::JpmorganContext,
    provider: requests::JpmorganWalletProvider,
    observed: Option<requests::JpmorganAccountNumberType>,
) -> Result<requests::JpmorganAccountNumberType, Error> {
    if context
        .wallet_provider
        .is_some_and(|original| original != provider)
        || observed
            .zip(context.account_number_type)
            .is_some_and(|(current, original)| current != original)
    {
        return Err(IntegrationError::InvalidDataFormat {
            field_name: "metadata.jpmorgan.accountNumberType",
            context: utils::integration_ctx(
                "The supplied credential type conflicts with the original transaction",
                "Use the original credential type and wallet provider",
            ),
        }
        .into());
    }
    context.account_number_type.or(observed).ok_or_else(|| {
        IntegrationError::MissingRequiredField {
            field_name: "metadata.jpmorgan.accountNumberType",
            context: utils::integration_ctx(
                "The token credential type cannot be determined from the original transaction",
                "Return the original accountNumberType in connector_feature_data",
            ),
        }
        .into()
    })
}

fn mit_card<T: PaymentMethodDataTypes>(
    payment_method: &PaymentMethodData<T>,
    context: &requests::JpmorganContext,
) -> Result<requests::JpmorganCard<T>, Error> {
    use requests::{
        JpmorganAccountNumber as Number, JpmorganAccountNumberType as Kind,
        JpmorganWalletProvider as Provider,
    };
    let (account_number, expiry, kind, provider) = match payment_method {
        PaymentMethodData::Card(card) => (
            Number::Card(card.card_number.clone()),
            build_jpmorgan_expiry(&card.card_exp_month, &card.card_exp_year)?,
            Kind::Pan,
            context.wallet_provider,
        ),
        PaymentMethodData::CardDetailsForNetworkTransactionId(card) => (
            Number::Decrypted(card.card_number.clone()),
            build_jpmorgan_expiry(&card.card_exp_month, &card.card_exp_year)?,
            Kind::Pan,
            context.wallet_provider,
        ),
        PaymentMethodData::NetworkToken(token) => (
            Number::Token(token.token_number.clone()),
            build_jpmorgan_expiry(&token.token_exp_month, &token.token_exp_year)?,
            Kind::NetworkToken,
            context.wallet_provider,
        ),
        PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(wallet) => {
            let provider = match wallet.token_source.as_ref() {
                Some(domain_types::payment_method_data::TokenSource::ApplePay) => {
                    Provider::ApplePay
                }
                Some(domain_types::payment_method_data::TokenSource::GooglePay) => {
                    Provider::GooglePay
                }
                None => {
                    return Err(IntegrationError::MissingRequiredField {
                        field_name: "payment_method_data.decrypted_wallet_token.token_source",
                        context: utils::integration_ctx(
                            "Decrypted wallet credentials require their wallet provider",
                            "Supply Apple Pay or Google Pay as token_source",
                        ),
                    }
                    .into())
                }
            };
            (
                Number::Token(wallet.decrypted_token.clone()),
                build_jpmorgan_expiry(&wallet.token_exp_month, &wallet.token_exp_year)?,
                mit_wallet_type(context, provider, None)?,
                Some(provider),
            )
        }
        PaymentMethodData::Wallet(WalletData::GooglePay(wallet)) => {
            let domain_types::payment_method_data::GpayTokenizationData::Decrypted(data) =
                &wallet.tokenization_data
            else {
                return Err(IntegrationError::MissingRequiredField {
                    field_name: "wallet.google_pay.decrypted_data",
                    context: utils::integration_ctx(
                        "This flow requires supplied decrypted Google Pay data",
                        "Supply decrypted Google Pay data",
                    ),
                }
                .into());
            };
            let observed = data.auth_method.map(|method| match method {
                common_enums::GooglePayAuthMethod::PanOnly => Kind::Pan,
                common_enums::GooglePayAuthMethod::Cryptogram => Kind::DeviceToken,
            });
            let year = data.get_four_digit_expiry_year().change_context(
                IntegrationError::InvalidDataFormat {
                    field_name: "wallet.expiry_year",
                    context: utils::integration_ctx(
                        "The wallet expiry year has an invalid length",
                        "Supply a two-digit or four-digit expiry year",
                    ),
                },
            )?;
            (
                Number::Decrypted(data.application_primary_account_number.clone()),
                build_jpmorgan_expiry(&data.card_exp_month, &year)?,
                mit_wallet_type(context, Provider::GooglePay, observed)?,
                Some(Provider::GooglePay),
            )
        }
        PaymentMethodData::Wallet(WalletData::ApplePay(wallet)) => {
            let data = wallet
                .payment_data
                .get_decrypted_apple_pay_payment_data_optional()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "wallet.apple_pay.decrypted_data",
                    context: utils::integration_ctx(
                        "Apple Pay requires supplied decrypted data in this connector",
                        "Supply decrypted Apple Pay data",
                    ),
                })?;
            if data
                .merchant_token_identifier
                .as_ref()
                .is_some_and(|id| id.peek().is_empty())
            {
                return Err(IntegrationError::InvalidDataFormat {
                    field_name: "wallet.apple_pay.merchant_token_identifier",
                    context: utils::integration_ctx(
                        "The supplied Apple merchant token identifier is empty",
                        "Supply the original merchant token identifier or omit it",
                    ),
                }
                .into());
            }
            // Vault rehydration can omit the MPAN marker. Retain original classification.
            let observed = data
                .merchant_token_identifier
                .as_ref()
                .map(|_| Kind::NetworkToken);
            (
                Number::Decrypted(data.application_primary_account_number.clone()),
                build_jpmorgan_expiry(
                    &data.application_expiration_month,
                    &data.get_four_digit_expiry_year(),
                )?,
                mit_wallet_type(context, Provider::ApplePay, observed)?,
                Some(Provider::ApplePay),
            )
        }
        _ => {
            return Err(IntegrationError::NotImplemented(
                "JPMorgan MIT requires card, network-token or decrypted-wallet credentials"
                    .to_owned(),
                utils::integration_ctx(
                    "The payment method cannot be used for merchant-initiated payments",
                    "Supply the credentials used for the original transaction",
                ),
            )
            .into())
        }
    };
    if context
        .account_number_type
        .is_some_and(|original| original != kind)
    {
        return Err(IntegrationError::InvalidDataFormat {
            field_name: "metadata.jpmorgan.accountNumberType",
            context: utils::integration_ctx(
                "The supplied credential type conflicts with the original transaction",
                "Use the original credential type and wallet provider",
            ),
        }
        .into());
    }
    Ok(requests::JpmorganCard {
        account_number,
        expiry,
        account_number_type: Some(kind),
        wallet_provider: provider,
        // Do not replay a CIT cryptogram or pass-through proof on an MIT.
        authentication: None,
        original_network_transaction_id: context.original_network_transaction_id.clone(),
        original_transaction_link_id: context.original_transaction_link_id.clone(),
        payment_authentication_request: None,
        verification_authentication_request: None,
    })
}

// RepeatPayment (subsequent MIT) request transformer
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        JpmorganRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for requests::JpmorganRepeatPaymentRequest<T>
{
    type Error = Error;
    fn try_from(
        item: JpmorganRouterData<
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

        let auth = JpmorganAuthType::try_from(&router_data.connector_config)?;
        let merchant = requests::JpmorganMerchant::try_from(&auth)?;

        let context = repeat_context(&router_data.request)?;
        if router_data.resource_common_data.auth_type == common_enums::AuthenticationType::ThreeDs
            || router_data.request.authentication_data.is_some()
        {
            return Err(IntegrationError::NotSupported {
                message: "JPMorgan MIT uses the original network reference, not 3DS authentication"
                    .to_owned(),
                connector: "jpmorgan",
                context: utils::integration_ctx("Merchant-initiated payments use the original network reference instead of fresh 3DS authentication", "Remove authentication_data and use the original network transaction reference"),
            }
            .into());
        }
        let capture_method = map_capture_method(router_data.request.capture_method)?;
        let amount = item
            .connector
            .amount_converter
            .convert(
                router_data.request.minor_amount,
                router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        let payment_method_type = requests::JpmorganPaymentMethodType {
            card: Some(mit_card(
                &router_data.request.payment_method_data,
                &context,
            )?),
            ach: None,
            googlepay: None,
            token: None,
        };
        if matches!(
            router_data.request.mandate_reference,
            MandateReferenceId::NetworkTokenWithNTI(_)
        ) && payment_method_type.card.as_ref().is_some_and(|card| {
            card.account_number_type == Some(requests::JpmorganAccountNumberType::Pan)
        }) {
            return Err(IntegrationError::InvalidDataFormat {
                field_name: "payment_method_data.network_token",
                context: utils::integration_ctx(
                    "A network token reference cannot be used with PAN credentials",
                    "Supply the matching network token credentials",
                ),
            }
            .into());
        }
        let mut stored_credential = cit_stored_credential(&context)?;
        stored_credential.merchant_order_number =
            merchant_order_number(router_data.request.merchant_order_id.as_ref())?;
        stored_credential.initiator_type = Some(requests::JpmorganInitiatorType::Merchant);
        stored_credential.account_on_file = Some(requests::JpmorganAccountOnFile::Stored);
        if let Some(recurring) = &mut stored_credential.recurring {
            recurring.recurring_sequence = requests::JpmorganRecurringSequence::Subsequent;
        }

        Ok(Self {
            capture_method,
            amount,
            currency: router_data.request.currency,
            merchant,
            payment_method_type,
            account_holder: None,
            statement_descriptor: None,
            stored_credential,
        })
    }
}

// RepeatPayment response transformer
impl<T: PaymentMethodDataTypes>
    TryFrom<ResponseRouterData<responses::JpmorganPaymentsResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::try_from(&item.response)?;
        let mut response = build_payments_response_result(&item.response, item.http_code, status)?;
        let mut context = repeat_context(&item.router_data.request)
            .change_context(ConnectorError::response_handling_failed(item.http_code))?;
        let card = mit_card(&item.router_data.request.payment_method_data, &context)
            .change_context(ConnectorError::response_handling_failed(item.http_code))?;
        context.account_number_type = card.account_number_type;
        context.wallet_provider = card.wallet_provider;
        finish_authentication(
            &mut response,
            context,
            item.response.payment_authentication_result.as_ref(),
            requests::JpmorganResourceKind::Payment,
            &item.router_data.resource_common_data,
            status,
            item.http_code,
        )?;

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}
