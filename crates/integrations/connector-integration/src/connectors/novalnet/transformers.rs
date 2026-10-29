use std::collections::HashMap;

use cards::CardNumber;
use common_utils::{
    ext_traits::OptionExt,
    pii,
    request::Method,
    types::{MinorUnit, StringMinorUnit},
};
use domain_types::{
    connector_flow::{
        self, Authorize, IncrementalAuthorization, PSync, RSync, RepeatPayment, SetupMandate, Void,
    },
    connector_types::{
        EventType, MandateReference, MandateReferenceId, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsIncrementalAuthorizationData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData,
        RefundWebhookDetailsResponse, RefundsData, RefundsResponseData, RepeatPaymentData,
        ResponseId, SetupMandateRequestData, WebhookDetailsResponse,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext, WebhookError},
    payment_method_data::{
        ApplePayPaymentData, BankDebitData, GpayTokenizationData, PaymentMethodData,
        PaymentMethodDataTypes, RawCardNumber, WalletData as WalletDataPaymentMethod,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
    utils::{self, ForeignTryFrom},
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};
use strum::Display;

use crate::{connectors::novalnet::NovalnetRouterData, types::ResponseRouterData};

/// Default locale
const DEFAULT_LOCALE: &str = "en";

/// Novalnet API parameter reference, cited in request-validation errors
const NOVALNET_API_DOC_URL: &str =
    "https://developer.novalnet.de/onlinepayments/apiparameterreference";

const MINIMAL_CUSTOMER_DATA_PASSED: i64 = 1;
const CREATE_TOKEN_REQUIRED: i8 = 1;

const TEST_MODE_ENABLED: i8 = 1;
const TEST_MODE_DISABLED: i8 = 0;

fn get_test_mode(item: Option<bool>) -> i8 {
    match item {
        Some(true) => TEST_MODE_ENABLED,
        Some(false) | None => TEST_MODE_DISABLED,
    }
}

#[derive(Debug, Copy, Serialize, Deserialize, Clone)]
pub enum NovalNetPaymentTypes {
    CREDITCARD,
    PAYPAL,
    GOOGLEPAY,
    APPLEPAY,
    #[serde(rename = "DIRECT_DEBIT_SEPA")]
    DirectDebitSepa,
    #[serde(rename = "DIRECT_DEBIT_ACH")]
    DirectDebitAch,
}

#[derive(Default, Debug, Serialize, Clone)]
pub struct NovalnetPaymentsRequestMerchant {
    signature: Secret<String>,
    tariff: Secret<String>,
}

#[derive(Default, Debug, Serialize, Clone)]
pub struct NovalnetPaymentsRequestBilling {
    house_no: Option<Secret<String>>,
    street: Option<Secret<String>>,
    city: Option<Secret<String>>,
    zip: Option<Secret<String>>,
    country_code: Option<common_enums::CountryAlpha2>,
}

#[derive(Default, Debug, Serialize, Clone)]
pub struct NovalnetPaymentsRequestCustomer {
    first_name: Option<Secret<String>>,
    last_name: Option<Secret<String>>,
    email: pii::Email,
    mobile: Option<Secret<String>>,
    billing: Option<NovalnetPaymentsRequestBilling>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping: Option<NovalnetPaymentsRequestShipping>,
    no_nc: i64,
    birth_date: Option<String>, // Mandatory for SEPA Guarantee Payment
}

#[derive(Default, Debug, Serialize, Clone)]
pub struct NovalnetPaymentsRequestShipping {
    first_name: Option<Secret<String>>,
    last_name: Option<Secret<String>>,
    email: Option<pii::Email>,
    tel: Option<Secret<String>>,
    street: Option<Secret<String>>,
    city: Option<Secret<String>>,
    zip: Option<Secret<String>>,
    state: Option<Secret<String>>,
    country_code: Option<common_enums::CountryAlpha2>,
}

/// Builds `customer.shipping` from the shipping address; `None` when no shipping address exists
/// or when none of its fields is populated (an all-null object is never sent).
fn get_novalnet_shipping(
    resource_common_data: &PaymentFlowData,
) -> Option<NovalnetPaymentsRequestShipping> {
    resource_common_data
        .get_optional_shipping()
        .map(|_| NovalnetPaymentsRequestShipping {
            first_name: resource_common_data.get_optional_shipping_first_name(),
            last_name: resource_common_data.get_optional_shipping_last_name(),
            email: resource_common_data.get_optional_shipping_email(),
            tel: resource_common_data.get_optional_shipping_phone_number(),
            street: resource_common_data.get_optional_shipping_line1(),
            city: resource_common_data.get_optional_shipping_city(),
            zip: resource_common_data.get_optional_shipping_zip(),
            state: resource_common_data.get_optional_shipping_state(),
            country_code: resource_common_data.get_optional_shipping_country(),
        })
        .filter(|shipping| {
            shipping.first_name.is_some()
                || shipping.last_name.is_some()
                || shipping.email.is_some()
                || shipping.tel.is_some()
                || shipping.street.is_some()
                || shipping.city.is_some()
                || shipping.zip.is_some()
                || shipping.state.is_some()
                || shipping.country_code.is_some()
        })
}

/// Novalnet offers automatic (/v2/payment) and a single manual (/v2/authorize) capture only, so
/// multiple partial captures and scheduled captures are refused. `context_suffix` is appended to
/// the error's additional context (e.g. `" for recurring payments"`).
fn validate_novalnet_capture_method(
    capture_method: Option<common_enums::CaptureMethod>,
    context_suffix: &str,
) -> Result<(), error_stack::Report<IntegrationError>> {
    match capture_method {
        Some(common_enums::CaptureMethod::ManualMultiple)
        | Some(common_enums::CaptureMethod::Scheduled) => {
            Err(IntegrationError::CaptureMethodNotSupported {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Use capture_method AUTOMATIC, SEQUENTIAL_AUTOMATIC or MANUAL; Novalnet \
                         supports a single capture of an authorization only"
                            .to_string(),
                    ),
                    doc_url: Some(NOVALNET_API_DOC_URL.to_string()),
                    additional_context: Some(format!(
                        "capture_method {capture_method:?} is not supported by Novalnet{context_suffix}"
                    )),
                },
            }
            .into())
        }
        Some(common_enums::CaptureMethod::Automatic)
        | Some(common_enums::CaptureMethod::Manual)
        | Some(common_enums::CaptureMethod::SequentialAutomatic)
        | None => Ok(()),
    }
}

/// Novalnet documents no transaction.payment_data field for a decrypted Google Pay
/// DPAN/cryptogram, so decrypted data is refused before any call; returns the encrypted token.
fn get_novalnet_gpay_encrypted_token(
    tokenization_data: &GpayTokenizationData,
    flow: &str,
) -> Result<String, error_stack::Report<IntegrationError>> {
    match tokenization_data {
        GpayTokenizationData::Decrypted(_) => Err(IntegrationError::NotSupported {
            message: "Decrypted Google Pay payment data".to_string(),
            connector: "novalnet",
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Send the encrypted Google Pay token (PSP decryption); \
                     Novalnet does not accept decrypted Google Pay data"
                        .to_string(),
                ),
                doc_url: Some(NOVALNET_API_DOC_URL.to_string()),
                additional_context: Some(format!(
                    "{flow}: Google Pay tokenization_data is Decrypted"
                )),
            },
        }
        .into()),
        GpayTokenizationData::Encrypted(encrypted) => Ok(encrypted.token.clone()),
    }
}

/// Novalnet documents no transaction.payment_data field for a decrypted Apple Pay
/// DPAN/cryptogram, so decrypted data is refused before any call.
fn reject_novalnet_decrypted_apple_pay(
    payment_data: &ApplePayPaymentData,
    flow: &str,
) -> Result<(), error_stack::Report<IntegrationError>> {
    match payment_data {
        ApplePayPaymentData::Decrypted(_) => Err(IntegrationError::NotSupported {
            message: "Decrypted Apple Pay payment data".to_string(),
            connector: "novalnet",
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Send the encrypted Apple Pay payment data (PSP decryption); \
                     Novalnet does not accept decrypted Apple Pay data"
                        .to_string(),
                ),
                doc_url: Some(NOVALNET_API_DOC_URL.to_string()),
                additional_context: Some(format!("{flow}: Apple Pay payment_data is Decrypted")),
            },
        }
        .into()),
        ApplePayPaymentData::Encrypted(_) => Ok(()),
    }
}
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct NovalnetCard<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    card_number: RawCardNumber<T>,
    card_expiry_month: Secret<String>,
    card_expiry_year: Secret<String>,
    card_cvc: Secret<String>,
    card_holder: Secret<String>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct NovalnetRawCardDetails {
    card_number: CardNumber,
    card_expiry_month: Secret<String>,
    card_expiry_year: Secret<String>,
    scheme_tid: Secret<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NovalnetMandate {
    token: Secret<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NovalnetSepaDebit {
    account_holder: Secret<String>,
    iban: Secret<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NovalnetAchDebit {
    account_holder: Secret<String>,
    account_number: Secret<String>,
    routing_number: Secret<String>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct NovalnetGooglePay {
    wallet_data: Secret<String>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct NovalnetApplePay {
    wallet_data: Secret<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum NovalNetPaymentData<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    Card(NovalnetCard<T>),
    RawCardForNTI(NovalnetRawCardDetails),
    GooglePay(NovalnetGooglePay),
    ApplePay(NovalnetApplePay),
    MandatePayment(NovalnetMandate),
    Sepa(NovalnetSepaDebit),
    Ach(NovalnetAchDebit),
}

#[derive(Default, Debug, Serialize, Clone)]
pub struct NovalnetCustom {
    lang: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum NovalNetAmount {
    StringMinor(StringMinorUnit),
    Int(i64),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NovalnetPaymentsRequestTransaction<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    test_mode: i8,
    payment_type: NovalNetPaymentTypes,
    amount: NovalNetAmount,
    currency: common_enums::Currency,
    order_no: String,
    payment_data: Option<NovalNetPaymentData<T>>,
    hook_url: Option<String>,
    return_url: Option<String>,
    error_return_url: Option<String>,
    enforce_3d: Option<i8>, //NOTE: Needed for CREDITCARD, GOOGLEPAY
    create_token: Option<i8>,
}

#[derive(Debug, Serialize, Clone)]
pub struct NovalnetPaymentsRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    merchant: NovalnetPaymentsRequestMerchant,
    customer: NovalnetPaymentsRequestCustomer,
    transaction: NovalnetPaymentsRequestTransaction<T>,
    custom: NovalnetCustom,
}

impl TryFrom<&common_enums::PaymentMethodType> for NovalNetPaymentTypes {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(item: &common_enums::PaymentMethodType) -> Result<Self, Self::Error> {
        match item {
            common_enums::PaymentMethodType::ApplePay => Ok(Self::APPLEPAY),
            common_enums::PaymentMethodType::Card => Ok(Self::CREDITCARD),
            common_enums::PaymentMethodType::GooglePay => Ok(Self::GOOGLEPAY),
            common_enums::PaymentMethodType::Paypal => Ok(Self::PAYPAL),
            common_enums::PaymentMethodType::Sepa => Ok(Self::DirectDebitSepa),
            common_enums::PaymentMethodType::Ach => Ok(Self::DirectDebitAch),
            _ => Err(error_stack::report!(IntegrationError::NotSupported {
                message: utils::get_unimplemented_payment_method_error_message("Novalnet"),
                connector: "Novalnet",
                context: Default::default(),
            }))?,
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NovalnetRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NovalnetPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: NovalnetRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Novalnet runs 3DS itself (transaction.enforce_3d -> result.redirect_url) and documents
        // no field for merchant-performed authentication results (CAVV/ECI/DS transaction id).
        // Silently dropping them would re-run 3DS or charge without liability shift, so external
        // 3DS data is refused for every payment-method arm before the request is built.
        if item.router_data.request.authentication_data.is_some() {
            return Err(IntegrationError::NotSupported {
                message: "external 3DS authentication data".to_string(),
                connector: "Novalnet",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Omit authentication_data; Novalnet authenticates the shopper itself \
                         when auth_type is THREE_DS (transaction.enforce_3d)"
                            .to_string(),
                    ),
                    doc_url: Some(NOVALNET_API_DOC_URL.to_string()),
                    additional_context: Some(
                        "Novalnet has no request field for externally performed 3DS results \
                         (cavv, eci, ds_trans_id)"
                            .to_string(),
                    ),
                },
            }
            .into());
        }

        let auth = NovalnetAuthType::try_from(&item.router_data.connector_config)?;

        let merchant = NovalnetPaymentsRequestMerchant {
            signature: auth.product_activation_key,
            tariff: auth.tariff_id,
        };

        let enforce_3d = match item.router_data.resource_common_data.auth_type {
            common_enums::AuthenticationType::ThreeDs => Some(1),
            common_enums::AuthenticationType::NoThreeDs => None,
        };
        let test_mode = get_test_mode(item.router_data.resource_common_data.test_mode);

        let billing = NovalnetPaymentsRequestBilling {
            house_no: item
                .router_data
                .resource_common_data
                .get_optional_billing_line1(),
            street: item
                .router_data
                .resource_common_data
                .get_optional_billing_line2(),
            city: item
                .router_data
                .resource_common_data
                .get_optional_billing_city(),
            zip: item
                .router_data
                .resource_common_data
                .get_optional_billing_zip(),
            country_code: item
                .router_data
                .resource_common_data
                .get_optional_billing_country(),
        };

        let customer = NovalnetPaymentsRequestCustomer {
            first_name: item
                .router_data
                .resource_common_data
                .get_optional_billing_first_name(),
            last_name: item
                .router_data
                .resource_common_data
                .get_optional_billing_last_name(),
            email: item
                .router_data
                .resource_common_data
                .get_billing_email()
                .or(item.router_data.request.get_email())?,
            mobile: item
                .router_data
                .resource_common_data
                .get_optional_billing_phone_number(),
            billing: Some(billing),
            shipping: get_novalnet_shipping(&item.router_data.resource_common_data),
            // no_nc is used to indicate if minimal customer data is passed or not
            no_nc: MINIMAL_CUSTOMER_DATA_PASSED,
            // DOB should be populated in case of SepaGuaranteedBankDebit payments
            birth_date: None,
        };

        let lang = item
            .router_data
            .request
            .get_optional_language_from_browser_info()
            .unwrap_or(DEFAULT_LOCALE.to_string().to_string());
        let custom = NovalnetCustom { lang };
        let hook_url = item.router_data.request.get_webhook_url()?;
        let return_url = item.router_data.request.get_router_return_url()?;
        let create_token = if item.router_data.request.is_mandate_payment() {
            Some(CREATE_TOKEN_REQUIRED)
        } else {
            None
        };

        let amount = item
            .connector
            .amount_converter
            .convert(&item.router_data.request.amount)
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        // Novalnet offers automatic (/v2/payment) and a single manual (/v2/authorize) capture
        // only, so multiple partial captures and scheduled captures are refused for every arm.
        validate_novalnet_capture_method(item.router_data.request.capture_method, "")?;

        match item.router_data.request.payment_method_data {
            PaymentMethodData::Card(ref req_card) => {
                let novalnet_card = NovalNetPaymentData::Card(NovalnetCard {
                    card_number: req_card.card_number.clone(),
                    card_expiry_month: req_card.card_exp_month.clone(),
                    card_expiry_year: req_card.card_exp_year.clone(),
                    card_cvc: req_card.card_cvc.clone(),
                    card_holder: item
                        .router_data
                        .resource_common_data
                        .get_billing_full_name()?,
                });

                let transaction = NovalnetPaymentsRequestTransaction {
                    test_mode,
                    payment_type: NovalNetPaymentTypes::CREDITCARD,
                    amount: NovalNetAmount::StringMinor(amount.clone()),
                    currency: item.router_data.request.currency,
                    order_no: item
                        .router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                    hook_url: Some(hook_url),
                    return_url: Some(return_url.clone()),
                    error_return_url: Some(return_url.clone()),
                    payment_data: Some(novalnet_card),
                    enforce_3d,
                    create_token,
                };

                Ok(Self {
                    merchant,
                    transaction,
                    customer,
                    custom,
                })
            }
            PaymentMethodData::Wallet(ref wallet_data) => match wallet_data {
                WalletDataPaymentMethod::GooglePay(ref req_wallet) => {
                    // Novalnet documents no transaction.payment_data field for a decrypted
                    // Google Pay DPAN/cryptogram, so decrypted data is refused before any call.
                    let encrypted_token = get_novalnet_gpay_encrypted_token(
                        &req_wallet.tokenization_data,
                        "Authorize",
                    )?;
                    let novalnet_google_pay: NovalNetPaymentData<T> =
                        NovalNetPaymentData::GooglePay(NovalnetGooglePay {
                            wallet_data: Secret::new(encrypted_token),
                        });

                    let transaction = NovalnetPaymentsRequestTransaction {
                        test_mode,
                        payment_type: NovalNetPaymentTypes::GOOGLEPAY,
                        amount: NovalNetAmount::StringMinor(amount.clone()),
                        currency: item.router_data.request.currency,
                        order_no: item
                            .router_data
                            .resource_common_data
                            .connector_request_reference_id
                            .clone(),
                        hook_url: Some(hook_url),
                        return_url: None,
                        error_return_url: None,
                        payment_data: Some(novalnet_google_pay),
                        enforce_3d,
                        create_token,
                    };

                    Ok(Self {
                        merchant,
                        transaction,
                        customer,
                        custom,
                    })
                }
                WalletDataPaymentMethod::ApplePay(payment_method_data) => {
                    // Novalnet documents no transaction.payment_data field for a decrypted
                    // Apple Pay DPAN/cryptogram, so decrypted data is refused before any call.
                    reject_novalnet_decrypted_apple_pay(
                        &payment_method_data.payment_data,
                        "Authorize",
                    )?;
                    let transaction = NovalnetPaymentsRequestTransaction {
                        test_mode,
                        payment_type: NovalNetPaymentTypes::APPLEPAY,
                        amount: NovalNetAmount::StringMinor(amount.clone()),
                        currency: item.router_data.request.currency,
                        order_no: item
                            .router_data
                            .resource_common_data
                            .connector_request_reference_id
                            .clone(),
                        hook_url: Some(hook_url),
                        return_url: None,
                        error_return_url: None,
                        payment_data: Some(NovalNetPaymentData::ApplePay(NovalnetApplePay {
                            wallet_data: payment_method_data.get_applepay_decoded_payment_data()?,
                        })),
                        enforce_3d: None,
                        create_token,
                    };

                    Ok(Self {
                        merchant,
                        transaction,
                        customer,
                        custom,
                    })
                }
                WalletDataPaymentMethod::PaypalRedirect(_) => {
                    let transaction = NovalnetPaymentsRequestTransaction {
                        test_mode,
                        payment_type: NovalNetPaymentTypes::PAYPAL,
                        amount: NovalNetAmount::StringMinor(amount.clone()),
                        currency: item.router_data.request.currency,
                        order_no: item
                            .router_data
                            .resource_common_data
                            .connector_request_reference_id
                            .clone(),
                        hook_url: Some(hook_url),
                        return_url: Some(return_url.clone()),
                        error_return_url: Some(return_url.clone()),
                        payment_data: None,
                        enforce_3d: None,
                        create_token,
                    };
                    Ok(Self {
                        merchant,
                        transaction,
                        customer,
                        custom,
                    })
                }
                WalletDataPaymentMethod::AliPayQr(_)
                | WalletDataPaymentMethod::BluecodeRedirect {}
                | WalletDataPaymentMethod::AliPayRedirect(_)
                | WalletDataPaymentMethod::AliPayHkRedirect(_)
                | WalletDataPaymentMethod::AmazonPayRedirect(_)
                | WalletDataPaymentMethod::MomoRedirect(_)
                | WalletDataPaymentMethod::KakaoPayRedirect(_)
                | WalletDataPaymentMethod::GoPayRedirect(_)
                | WalletDataPaymentMethod::GcashRedirect(_)
                | WalletDataPaymentMethod::ApplePayRedirect(_)
                | WalletDataPaymentMethod::ApplePayThirdPartySdk(_)
                | WalletDataPaymentMethod::DanaRedirect {}
                | WalletDataPaymentMethod::GrabpayRedirect {}
                | WalletDataPaymentMethod::GooglePayRedirect(_)
                | WalletDataPaymentMethod::GooglePayThirdPartySdk(_)
                | WalletDataPaymentMethod::MbWayRedirect(_)
                | WalletDataPaymentMethod::MobilePayRedirect(_)
                | WalletDataPaymentMethod::RevolutPay(_)
                | WalletDataPaymentMethod::PaypalSdk(_)
                | WalletDataPaymentMethod::Paze(_)
                | WalletDataPaymentMethod::SamsungPay(_)
                | WalletDataPaymentMethod::TwintRedirect {}
                | WalletDataPaymentMethod::VippsRedirect {}
                | WalletDataPaymentMethod::TouchNGoRedirect(_)
                | WalletDataPaymentMethod::WeChatPayRedirect(_)
                | WalletDataPaymentMethod::CashappQr(_)
                | WalletDataPaymentMethod::SwishQr(_)
                | WalletDataPaymentMethod::WeChatPayQr(_)
                | WalletDataPaymentMethod::Mifinity(_)
                | WalletDataPaymentMethod::MbWay(_)
                | WalletDataPaymentMethod::Satispay(_)
                | WalletDataPaymentMethod::SatispayIntent(_)
                | WalletDataPaymentMethod::SatispayQr(_)
                | WalletDataPaymentMethod::Wero(_)
                | WalletDataPaymentMethod::LazyPayRedirect(_)
                | WalletDataPaymentMethod::PhonePeRedirect(_)
                | WalletDataPaymentMethod::BillDeskRedirect(_)
                | WalletDataPaymentMethod::CashfreeRedirect(_)
                | WalletDataPaymentMethod::PayURedirect(_)
                | WalletDataPaymentMethod::EaseBuzzRedirect(_)
                | WalletDataPaymentMethod::PaymayaRedirect(_)
                | WalletDataPaymentMethod::PayhereRedirect {}
                | WalletDataPaymentMethod::QwikcilverWalletDirect(_)
                | WalletDataPaymentMethod::Skrill(_)
                | WalletDataPaymentMethod::Neteller(_) => Err(IntegrationError::NotImplemented(
                    utils::get_unimplemented_payment_method_error_message("novalnet"),
                    Default::default(),
                )
                .into()),
            },
            PaymentMethodData::BankDebit(ref bank_debit_data) => {
                let payment_type = NovalNetPaymentTypes::try_from(
                    &item.router_data.request.payment_method_type.ok_or(
                        IntegrationError::MissingPaymentMethodType {
                            context: Default::default(),
                        },
                    )?,
                )?;

                let transaction = match bank_debit_data {
                    BankDebitData::SepaBankDebit {
                        iban,
                        bank_account_holder_name,
                    } => {
                        let account_holder = match bank_account_holder_name {
                            Some(name) => name.clone(),
                            None => item
                                .router_data
                                .resource_common_data
                                .get_billing_full_name()?,
                        };
                        NovalnetPaymentsRequestTransaction {
                            test_mode,
                            payment_type,
                            amount: NovalNetAmount::StringMinor(amount.clone()),
                            currency: item.router_data.request.currency,
                            order_no: item
                                .router_data
                                .resource_common_data
                                .connector_request_reference_id
                                .clone(),
                            hook_url: Some(hook_url),
                            return_url: Some(return_url.clone()),
                            error_return_url: Some(return_url.clone()),
                            payment_data: Some(NovalNetPaymentData::Sepa(NovalnetSepaDebit {
                                account_holder: account_holder.clone(),
                                iban: iban.clone(),
                            })),
                            enforce_3d,
                            create_token,
                        }
                    }
                    BankDebitData::AchBankDebit {
                        account_number,
                        routing_number,
                        bank_account_holder_name,
                        ..
                    } => {
                        let account_holder = match bank_account_holder_name {
                            Some(name) => name.clone(),
                            None => item
                                .router_data
                                .resource_common_data
                                .get_billing_full_name()?,
                        };
                        NovalnetPaymentsRequestTransaction {
                            test_mode,
                            payment_type,
                            amount: NovalNetAmount::StringMinor(amount.clone()),
                            currency: item.router_data.request.currency,
                            order_no: item
                                .router_data
                                .resource_common_data
                                .connector_request_reference_id
                                .clone(),
                            hook_url: Some(hook_url),
                            return_url: Some(return_url.clone()),
                            error_return_url: Some(return_url.clone()),
                            payment_data: Some(NovalNetPaymentData::Ach(NovalnetAchDebit {
                                account_holder: account_holder.clone(),
                                account_number: account_number.clone(),
                                routing_number: routing_number.clone(),
                            })),
                            enforce_3d,
                            create_token,
                        }
                    }
                    BankDebitData::SepaGuaranteedBankDebit { .. }
                    | BankDebitData::BecsBankDebit { .. }
                    | BankDebitData::EftBankDebit { .. }
                    | BankDebitData::BacsBankDebit { .. } => {
                        return Err(IntegrationError::NotImplemented(
                            utils::get_unimplemented_payment_method_error_message("novalnet"),
                            Default::default(),
                        )
                        .into());
                    }
                };

                Ok(Self {
                    merchant,
                    transaction,
                    customer,
                    custom,
                })
            }
            _ => Err(IntegrationError::NotImplemented(
                utils::get_unimplemented_payment_method_error_message("novalnet"),
                Default::default(),
            )
            .into()),
        }
    }
}

// Auth Struct
pub struct NovalnetAuthType {
    pub(super) product_activation_key: Secret<String>,
    pub(super) payment_access_key: Secret<String>,
    pub(super) tariff_id: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for NovalnetAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Novalnet {
                product_activation_key,
                payment_access_key,
                tariff_id,
                ..
            } => Ok(Self {
                product_activation_key: product_activation_key.to_owned(),
                payment_access_key: payment_access_key.to_owned(),
                tariff_id: tariff_id.to_owned(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

// PaymentsResponse
#[derive(Debug, Display, Copy, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NovalnetTransactionStatus {
    Success,
    Failure,
    Confirmed,
    OnHold,
    Pending,
    Deactivated,
    Progress,
    Error,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Copy, Display, Clone, Serialize, Deserialize, PartialEq)]
#[strum(serialize_all = "UPPERCASE")]
#[serde(rename_all = "UPPERCASE")]
pub enum NovalnetAPIStatus {
    Success,
    Failure,
    #[serde(other)]
    Unknown,
}

/// Maps a Novalnet `transaction.status` to the payment attempt status.
/// An undocumented status retains `current` (non-terminal), as Hyperswitch does.
pub fn get_novalnet_attempt_status(
    status: NovalnetTransactionStatus,
    current: common_enums::AttemptStatus,
) -> common_enums::AttemptStatus {
    match status {
        // spec: ## Status Mappings (CONFIRMED / SUCCESS -> Charged)
        NovalnetTransactionStatus::Confirmed | NovalnetTransactionStatus::Success => {
            common_enums::AttemptStatus::Charged
        }
        // spec: ## Status Mappings (ON_HOLD -> Authorized)
        NovalnetTransactionStatus::OnHold => common_enums::AttemptStatus::Authorized,
        // spec: ## Status Mappings (PENDING -> Pending)
        NovalnetTransactionStatus::Pending => common_enums::AttemptStatus::Pending,
        // spec: ## Status Mappings (PROGRESS -> AuthenticationPending)
        NovalnetTransactionStatus::Progress => common_enums::AttemptStatus::AuthenticationPending,
        // spec: ## Status Mappings (DEACTIVATED -> Voided)
        NovalnetTransactionStatus::Deactivated => common_enums::AttemptStatus::Voided,
        // spec: ## Status Mappings (FAILURE / ERROR -> Failure)
        NovalnetTransactionStatus::Failure | NovalnetTransactionStatus::Error => {
            common_enums::AttemptStatus::Failure
        }
        // spec: ## Status Mappings ((unknown) -> previous status retained)
        NovalnetTransactionStatus::Unknown => current,
    }
}

/// Maps a Novalnet `transaction.status` to the refund status.
/// An undocumented status retains `current` (non-terminal), as Hyperswitch does.
pub fn get_novalnet_refund_status(
    status: NovalnetTransactionStatus,
    current: common_enums::RefundStatus,
) -> common_enums::RefundStatus {
    match status {
        // spec: ## Status Mappings (CONFIRMED / SUCCESS -> Success)
        NovalnetTransactionStatus::Confirmed | NovalnetTransactionStatus::Success => {
            common_enums::RefundStatus::Success
        }
        // spec: ## Status Mappings (PENDING -> Pending)
        NovalnetTransactionStatus::Pending => common_enums::RefundStatus::Pending,
        // spec: ## Status Mappings (ON_HOLD / PROGRESS / DEACTIVATED / FAILURE / ERROR -> Failure)
        NovalnetTransactionStatus::OnHold
        | NovalnetTransactionStatus::Progress
        | NovalnetTransactionStatus::Deactivated
        | NovalnetTransactionStatus::Failure
        | NovalnetTransactionStatus::Error => common_enums::RefundStatus::Failure,
        // spec: ## Status Mappings ((unknown) -> previous status retained)
        NovalnetTransactionStatus::Unknown => current,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultData {
    pub redirect_url: Option<Secret<url::Url>>,
    pub status: NovalnetAPIStatus,
    pub status_code: Option<u64>,
    pub status_text: String,
    pub additional_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NovalnetPaymentsResponseTransactionData {
    pub amount: Option<MinorUnit>,
    pub currency: Option<common_enums::Currency>,
    pub date: Option<String>,
    pub order_no: Option<String>,
    pub payment_data: Option<NovalnetResponsePaymentData>,
    pub payment_type: Option<String>,
    pub status_code: Option<u64>,
    pub txn_secret: Option<Secret<String>>,
    pub tid: Option<i64>,
    pub test_mode: Option<i8>,
    pub status: Option<NovalnetTransactionStatus>,
    pub authorization: Option<NovalnetAuthorizationResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NovalnetPaymentsResponse {
    result: ResultData,
    transaction: Option<NovalnetPaymentsResponseTransactionData>,
}

pub fn get_error_response(
    result: ResultData,
    status_code: u16,
    transaction_id: Option<String>,
    attempt_status: Option<FlowStatus>,
) -> ErrorResponse {
    let error_code = result.status;
    let error_reason = result.status_text.clone();
    // Novalnet's only issuer signal is `result.additional_message`, e.g. "33 Expired Card":
    // the leading token is the issuer decline code only when it is purely numeric.
    let network_decline_code = result.additional_message.as_deref().and_then(|message| {
        message
            .split_whitespace()
            .next()
            .filter(|token| token.chars().all(|c| c.is_ascii_digit()))
            .map(str::to_string)
    });

    ErrorResponse {
        code: error_code.to_string(),
        message: error_reason.clone(),
        reason: Some(error_reason),
        status_code,
        attempt_status,
        connector_transaction_id: transaction_id,
        network_advice_code: None,
        network_decline_code,
        network_error_message: result.additional_message,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
}

/// Error response for a SUCCESS envelope whose transaction failed (an in-band decline). The code
/// comes from the transaction (`transaction.status_code`, else its wire status), never from the
/// envelope's `result.status`, which would read "SUCCESS".
fn get_transaction_decline_error_response(
    result: ResultData,
    transaction_status_code: Option<u64>,
    transaction_status: NovalnetTransactionStatus,
    status_code: u16,
    transaction_id: Option<String>,
    attempt_status: Option<FlowStatus>,
) -> ErrorResponse {
    let code = transaction_status_code
        .map(|code| code.to_string())
        .unwrap_or_else(|| {
            match transaction_status {
                NovalnetTransactionStatus::Error => "ERROR",
                NovalnetTransactionStatus::Failure
                | NovalnetTransactionStatus::Success
                | NovalnetTransactionStatus::Confirmed
                | NovalnetTransactionStatus::OnHold
                | NovalnetTransactionStatus::Pending
                | NovalnetTransactionStatus::Deactivated
                | NovalnetTransactionStatus::Progress
                | NovalnetTransactionStatus::Unknown => "FAILURE",
            }
            .to_string()
        });
    ErrorResponse {
        code,
        ..get_error_response(result, status_code, transaction_id, attempt_status)
    }
}

impl NovalnetPaymentsResponseTransactionData {
    pub fn get_token(transaction_data: Option<&Self>) -> Option<String> {
        if let Some(data) = transaction_data {
            match &data.payment_data {
                Some(NovalnetResponsePaymentData::Card(card_data)) => {
                    card_data.token.clone().map(|token| token.expose())
                }
                Some(NovalnetResponsePaymentData::Paypal(paypal_data)) => {
                    paypal_data.token.clone().map(|token| token.expose())
                }
                None => None,
            }
        } else {
            None
        }
    }
}

/// Shared response mapping for the `/v2/payment` and `/v2/authorize` bodies returned to
/// Authorize, SetupMandate and RepeatPayment. Returns the attempt status to store and the
/// flow response.
fn build_novalnet_payments_response(
    response: NovalnetPaymentsResponse,
    http_code: u16,
    current_status: common_enums::AttemptStatus,
) -> (
    common_enums::AttemptStatus,
    Result<PaymentsResponseData, ErrorResponse>,
) {
    let transaction_id = response
        .transaction
        .as_ref()
        .and_then(|data| data.tid.map(|tid| tid.to_string()));

    match response.result.status {
        NovalnetAPIStatus::Success => {
            let redirection_data: Option<RedirectForm> =
                response
                    .result
                    .redirect_url
                    .clone()
                    .map(|url| RedirectForm::Form {
                        endpoint: url.expose().to_string(),
                        method: Method::Get,
                        form_fields: HashMap::new(),
                    });

            let transaction_status = response
                .transaction
                .as_ref()
                .and_then(|transaction_data| transaction_data.status)
                .unwrap_or(if redirection_data.is_some() {
                    // NOTE: Novalnet does not send the transaction.status for the redirection
                    // flow, so the status is mapped to Progress if the flow has redirection data
                    NovalnetTransactionStatus::Progress
                } else {
                    NovalnetTransactionStatus::Pending
                });

            match transaction_status {
                // A SUCCESS envelope can still carry a failed transaction: that is a decline.
                // Its error code comes from the transaction (status_code, else its wire status),
                // never from the envelope's literal "SUCCESS".
                NovalnetTransactionStatus::Failure | NovalnetTransactionStatus::Error => {
                    let transaction_status_code = response
                        .transaction
                        .as_ref()
                        .and_then(|transaction_data| transaction_data.status_code);
                    (
                        common_enums::AttemptStatus::Failure,
                        Err(get_transaction_decline_error_response(
                            response.result,
                            transaction_status_code,
                            transaction_status,
                            http_code,
                            transaction_id,
                            Some(FlowStatus::Payment(common_enums::AttemptStatus::Failure)),
                        )),
                    )
                }
                NovalnetTransactionStatus::Success
                | NovalnetTransactionStatus::Confirmed
                | NovalnetTransactionStatus::OnHold
                | NovalnetTransactionStatus::Pending
                | NovalnetTransactionStatus::Deactivated
                | NovalnetTransactionStatus::Progress
                | NovalnetTransactionStatus::Unknown => {
                    let mandate_reference_id = NovalnetPaymentsResponseTransactionData::get_token(
                        response.transaction.as_ref(),
                    );
                    let network_txn_id = response.transaction.and_then(|data| {
                        data.payment_data
                            .and_then(|payment_data| match payment_data {
                                NovalnetResponsePaymentData::Card(card) => {
                                    card.scheme_tid.map(|tid| tid.expose())
                                }
                                NovalnetResponsePaymentData::Paypal(_) => None,
                            })
                    });
                    (
                        get_novalnet_attempt_status(transaction_status, current_status),
                        Ok(PaymentsResponseData::TransactionResponse {
                            resource_id: transaction_id
                                .clone()
                                .map(ResponseId::ConnectorTransactionId)
                                .unwrap_or(ResponseId::NoResponseId),
                            redirection_data: redirection_data.map(Box::new),
                            mandate_reference: mandate_reference_id
                                .map(|id| MandateReference {
                                    connector_mandate_id: Some(id),
                                    payment_method_id: None,
                                    connector_mandate_request_reference_id: None,
                                    mandate_metadata: None,
                                })
                                .map(Box::new),
                            connector_metadata: None,
                            network_txn_id,
                            network_txn_link_id: None,
                            connector_response_reference_id: transaction_id,
                            incremental_authorization_allowed: None,
                            status_code: http_code,
                            splits: None,
                            payment_account_reference: None,
                        }),
                    )
                }
            }
        }
        NovalnetAPIStatus::Failure => (
            common_enums::AttemptStatus::Failure,
            Err(get_error_response(
                response.result,
                http_code,
                transaction_id,
                Some(FlowStatus::Payment(common_enums::AttemptStatus::Failure)),
            )),
        ),
        // An undocumented API status is not a decline: surface the error, keep the status.
        NovalnetAPIStatus::Unknown => (
            current_status,
            Err(get_error_response(
                response.result,
                http_code,
                transaction_id,
                None,
            )),
        ),
    }
}

// Specific implementations for Authorize flow
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<NovalnetPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<NovalnetPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let (status, response) = build_novalnet_payments_response(
            item.response,
            item.http_code,
            item.router_data.resource_common_data.status,
        );
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

// Specific implementations for SetupMandate flow
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<NovalnetPaymentsResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<NovalnetPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let (status, response) = build_novalnet_payments_response(
            item.response,
            item.http_code,
            item.router_data.resource_common_data.status,
        );
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

// Specific implementations for RepeatPayment flow
impl<
        T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize + Serialize,
    > TryFrom<ResponseRouterData<NovalnetPaymentsResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<NovalnetPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let (status, response) = build_novalnet_payments_response(
            item.response,
            item.http_code,
            item.router_data.resource_common_data.status,
        );
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NovalnetResponseCustomer {
    pub billing: Option<NovalnetResponseBilling>,
    pub customer_ip: Option<Secret<String>>,
    pub email: Option<pii::Email>,
    pub first_name: Option<Secret<String>>,
    pub gender: Option<Secret<String>>,
    pub last_name: Option<Secret<String>>,
    pub mobile: Option<Secret<String>>,
    pub tel: Option<Secret<String>>,
    pub fax: Option<Secret<String>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NovalnetResponseBilling {
    pub city: Option<Secret<String>>,
    pub country_code: Option<Secret<String>>,
    pub house_no: Option<Secret<String>>,
    pub street: Option<Secret<String>>,
    pub zip: Option<Secret<String>>,
    pub state: Option<Secret<String>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NovalnetResponseMerchant {
    pub project: Option<Secret<i64>>,
    pub project_name: Option<Secret<String>>,
    pub project_url: Option<url::Url>,
    pub vendor: Option<Secret<i64>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NovalnetAuthorizationResponse {
    expiry_date: Option<String>,
    auto_action: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NovalnetSyncResponseTransactionData {
    pub amount: Option<MinorUnit>,
    pub currency: Option<common_enums::Currency>,
    pub date: Option<String>,
    pub order_no: Option<String>,
    pub payment_data: Option<NovalnetResponsePaymentData>,
    pub payment_type: Option<String>,
    pub status: NovalnetTransactionStatus,
    pub status_code: Option<u64>,
    pub test_mode: Option<u8>,
    pub tid: Option<i64>,
    pub txn_secret: Option<Secret<String>>,
    pub authorization: Option<NovalnetAuthorizationResponse>,
    pub reason: Option<String>,
    pub reason_code: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum NovalnetResponsePaymentData {
    Card(NovalnetResponseCard),
    Paypal(NovalnetResponsePaypal),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NovalnetResponseCard {
    pub card_brand: Option<Secret<String>>,
    // Wallet (apple_pay / google_pay) responses carry no full card number, so these required
    // fields made the untagged `Card` variant fail to deserialize and silently fall through to
    // `Paypal`, whose arm drops `scheme_tid` — losing `network_txn_id` on every wallet payment.
    // Mirrors hyperswitch's novalnet deserialization resilience (#13214).
    pub card_expiry_month: Option<Secret<u8>>,
    pub card_expiry_year: Option<Secret<u16>>,
    pub card_holder: Option<Secret<String>>,
    pub card_number: Option<Secret<String>>,
    pub cc_3d: Option<Secret<u8>>,
    pub last_four: Option<Secret<String>>,
    pub token: Option<Secret<String>>,
    pub scheme_tid: Option<Secret<String>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NovalnetResponsePaypal {
    pub paypal_account: Option<pii::Email>,
    pub paypal_transaction_id: Option<Secret<String>>,
    pub token: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NovalnetPSyncResponse {
    pub customer: Option<NovalnetResponseCustomer>,
    pub merchant: Option<NovalnetResponseMerchant>,
    pub result: ResultData,
    pub transaction: Option<NovalnetSyncResponseTransactionData>,
}

#[derive(Debug, Copy, Serialize, Default, Deserialize, Clone)]
pub enum CaptureType {
    #[default]
    Partial,
    Final,
}

#[derive(Default, Debug, Serialize)]
pub struct Capture {
    #[serde(rename = "type")]
    cap_type: CaptureType,
    reference: String,
}
#[derive(Default, Debug, Serialize)]
pub struct NovalnetTransaction {
    tid: String,
    amount: Option<StringMinorUnit>,
    capture: Capture,
}

#[derive(Default, Debug, Serialize)]
pub struct NovalnetCaptureRequest {
    pub transaction: NovalnetTransaction,
    pub custom: NovalnetCustom,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NovalnetRouterData<
            RouterDataV2<
                connector_flow::Capture,
                PaymentFlowData,
                PaymentsCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NovalnetCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: NovalnetRouterData<
            RouterDataV2<
                connector_flow::Capture,
                PaymentFlowData,
                PaymentsCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let capture_type = CaptureType::Final;
        let reference = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let capture = Capture {
            cap_type: capture_type,
            reference,
        };

        let amount = item
            .connector
            .amount_converter
            .convert(&item.router_data.request.amount_to_capture)
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        let transaction = NovalnetTransaction {
            tid: item
                .router_data
                .request
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID {
                    context: Default::default(),
                })?,
            capture,
            amount: Some(amount.to_owned()),
        };

        let custom = NovalnetCustom {
            lang: item
                .router_data
                .request
                .get_optional_language_from_browser_info()
                .unwrap_or(DEFAULT_LOCALE.to_string()),
        };
        Ok(Self {
            transaction,
            custom,
        })
    }
}

// Type definition for RefundRequest
#[derive(Default, Debug, Serialize)]
pub struct NovalnetRefundTransaction {
    tid: String,
    amount: Option<StringMinorUnit>,
}

#[derive(Default, Debug, Serialize)]
pub struct NovalnetRefundRequest {
    pub transaction: NovalnetRefundTransaction,
    pub custom: NovalnetCustom,
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NovalnetRouterData<RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for NovalnetRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: NovalnetRouterData<
            RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount = item
            .connector
            .amount_converter
            .convert(&item.router_data.request.refund_amount)
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        let transaction = NovalnetRefundTransaction {
            tid: item.router_data.request.connector_transaction_id.clone(),
            amount: Some(amount.to_owned()),
        };

        let custom = NovalnetCustom {
            lang: item
                .router_data
                .request
                .get_optional_language_from_browser_info()
                .unwrap_or(DEFAULT_LOCALE.to_string().to_string()),
        };
        Ok(Self {
            transaction,
            custom,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NovalnetRefundSyncResponse {
    result: ResultData,
    transaction: Option<NovalnetSyncResponseTransactionData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NovalnetRefundsTransactionData {
    pub amount: Option<MinorUnit>,
    pub date: Option<String>,
    pub currency: Option<common_enums::Currency>,
    pub order_no: Option<String>,
    pub payment_type: String,
    // Absent on a FAILURE body: keeping it optional lets a declined refund reach the error path.
    pub refund: Option<RefundData>,
    pub refunded_amount: Option<u64>,
    pub status: NovalnetTransactionStatus,
    pub status_code: u64,
    pub test_mode: u8,
    pub tid: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefundData {
    amount: Option<u64>,
    currency: Option<common_enums::Currency>,
    payment_type: Option<String>,
    tid: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NovalnetRefundResponse {
    pub customer: Option<NovalnetResponseCustomer>,
    pub merchant: Option<NovalnetResponseMerchant>,
    pub result: ResultData,
    pub transaction: Option<NovalnetRefundsTransactionData>,
}

impl<F> TryFrom<ResponseRouterData<NovalnetRefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<NovalnetRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Refund id is the child refund tid, falling back to the transaction tid (Hyperswitch parity).
        let refund_id = item.response.transaction.as_ref().and_then(|data| {
            data.refund
                .as_ref()
                .and_then(|refund| refund.tid)
                .or(data.tid)
                .map(|tid| tid.to_string())
        });

        match item.response.result.status {
            NovalnetAPIStatus::Success => {
                let refund_id = refund_id.ok_or(
                    crate::utils::response_handling_fail_for_connector(item.http_code, "novalnet"),
                )?;
                let transaction_status = item
                    .response
                    .transaction
                    .map(|transaction| transaction.status)
                    .unwrap_or(NovalnetTransactionStatus::Pending);

                Ok(Self {
                    response: Ok(RefundsResponseData {
                        connector_refund_id: refund_id,
                        refund_status: get_novalnet_refund_status(
                            transaction_status,
                            common_enums::RefundStatus::Pending,
                        ),
                        status_code: item.http_code,
                        acquirer_reference_number: None,
                    }),
                    ..item.router_data
                })
            }
            // spec: ## Status Mappings (result.status FAILURE -> refund failed)
            NovalnetAPIStatus::Failure => {
                let response = Err(get_error_response(
                    item.response.result,
                    item.http_code,
                    refund_id,
                    Some(FlowStatus::Refund(common_enums::RefundStatus::Failure)),
                ));
                Ok(Self {
                    response,
                    ..item.router_data
                })
            }
            // Undocumented result.status: error without a terminal refund status.
            NovalnetAPIStatus::Unknown => {
                let response = Err(get_error_response(
                    item.response.result,
                    item.http_code,
                    refund_id,
                    None,
                ));
                Ok(Self {
                    response,
                    ..item.router_data
                })
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NovalnetRedirectionResponse {
    status: NovalnetTransactionStatus,
    tid: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NovalnetRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for NovalnetSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: NovalnetRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let transaction = if item
            .router_data
            .request
            .encoded_data
            .clone()
            .get_required_value("encoded_data")
            .is_ok()
        {
            let encoded_data = item
                .router_data
                .request
                .encoded_data
                .clone()
                .get_required_value("encoded_data")
                .change_context(IntegrationError::RequestEncodingFailed {
                    context: Default::default(),
                })?;
            let novalnet_redirection_response =
                serde_urlencoded::from_str::<NovalnetRedirectionResponse>(encoded_data.as_str())
                    .change_context(IntegrationError::InvalidDataFormat {
                        field_name: "encoded_data",
                        context: Default::default(),
                    })?;
            let tid = novalnet_redirection_response.tid.ok_or(
                IntegrationError::MissingRequiredField {
                    field_name: "tid",
                    context: Default::default(),
                },
            )?;
            NovalnetSyncTransaction { tid }
        } else {
            NovalnetSyncTransaction {
                tid: item
                    .router_data
                    .request
                    .get_connector_transaction_id()
                    .change_context(IntegrationError::MissingConnectorTransactionID {
                        context: Default::default(),
                    })?,
            }
        };

        let custom = NovalnetCustom {
            lang: DEFAULT_LOCALE.to_string().to_string(),
        };
        Ok(Self {
            transaction,
            custom,
        })
    }
}

impl NovalnetSyncResponseTransactionData {
    pub fn get_token(transaction_data: Option<&Self>) -> Option<String> {
        if let Some(data) = transaction_data {
            match &data.payment_data {
                Some(NovalnetResponsePaymentData::Card(card_data)) => {
                    card_data.token.clone().map(|token| token.expose())
                }
                Some(NovalnetResponsePaymentData::Paypal(paypal_data)) => {
                    paypal_data.token.clone().map(|token| token.expose())
                }
                None => None,
            }
        } else {
            None
        }
    }
}

impl<F> TryFrom<ResponseRouterData<NovalnetPSyncResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<NovalnetPSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let transaction_id = item
            .response
            .transaction
            .clone()
            .and_then(|data| data.tid)
            .map(|tid| tid.to_string());

        match item.response.result.status {
            NovalnetAPIStatus::Success => {
                let transaction_status = item
                    .response
                    .transaction
                    .clone()
                    .map(|transaction_data| transaction_data.status)
                    // spec: ## Status Mappings (transaction absent -> Pending)
                    .unwrap_or(NovalnetTransactionStatus::Pending);

                // A SUCCESS envelope can still carry a failed transaction: report it as an
                // error with a terminal Failure, as the payment-creating flows do.
                if matches!(
                    transaction_status,
                    NovalnetTransactionStatus::Failure | NovalnetTransactionStatus::Error
                ) {
                    let transaction_status_code = item
                        .response
                        .transaction
                        .as_ref()
                        .and_then(|transaction_data| transaction_data.status_code);
                    return Ok(Self {
                        resource_common_data: PaymentFlowData {
                            status: common_enums::AttemptStatus::Failure,
                            ..item.router_data.resource_common_data
                        },
                        response: Err(get_transaction_decline_error_response(
                            item.response.result,
                            transaction_status_code,
                            transaction_status,
                            item.http_code,
                            transaction_id,
                            Some(FlowStatus::Payment(common_enums::AttemptStatus::Failure)),
                        )),
                        ..item.router_data
                    });
                }

                let mandate_reference_id = NovalnetSyncResponseTransactionData::get_token(
                    item.response.transaction.as_ref(),
                );

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status: get_novalnet_attempt_status(
                            transaction_status,
                            item.router_data.resource_common_data.status,
                        ),
                        ..item.router_data.resource_common_data
                    },
                    response: Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: transaction_id
                            .clone()
                            .map(ResponseId::ConnectorTransactionId)
                            .unwrap_or(ResponseId::NoResponseId),
                        redirection_data: None,
                        mandate_reference: mandate_reference_id
                            .as_ref()
                            .map(|id| MandateReference {
                                connector_mandate_id: Some(id.clone()),
                                payment_method_id: None,
                                connector_mandate_request_reference_id: None,
                                mandate_metadata: None,
                            })
                            .map(Box::new),
                        connector_metadata: None,
                        network_txn_id: item.response.transaction.and_then(|data| {
                            data.payment_data
                                .and_then(|payment_data| match payment_data {
                                    NovalnetResponsePaymentData::Card(card) => {
                                        card.scheme_tid.map(|tid| tid.expose())
                                    }
                                    NovalnetResponsePaymentData::Paypal(_) => None,
                                })
                        }),
                        network_txn_link_id: None,
                        connector_response_reference_id: transaction_id.clone(),
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                        splits: None,
                        payment_account_reference: None,
                    }),
                    ..item.router_data
                })
            }
            NovalnetAPIStatus::Failure | NovalnetAPIStatus::Unknown => {
                let response = Err(get_error_response(
                    item.response.result,
                    item.http_code,
                    transaction_id,
                    None,
                ));
                Ok(Self {
                    response,
                    ..item.router_data
                })
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NovalnetCaptureTransactionData {
    pub amount: Option<MinorUnit>,
    // spec: ## Complete Endpoint Inventory — `transaction.capture` is returned for card and
    // PayPal only, so a capture of any other payment type omits it.
    pub capture: Option<CaptureData>,
    pub currency: Option<common_enums::Currency>,
    pub order_no: Option<String>,
    pub payment_type: Option<String>,
    // spec/HS: `status` is returned for CreditCard/ApplePay/GooglePay/PayPal captures only.
    pub status: Option<NovalnetTransactionStatus>,
    pub status_code: Option<u64>,
    pub test_mode: Option<u8>,
    // HS: documented as mandatory but absent from the SEPA capture response.
    pub tid: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureData {
    amount: Option<u64>,
    payment_type: Option<String>,
    status: Option<String>,
    status_code: Option<u64>,
    tid: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NovalnetCaptureResponse {
    pub result: ResultData,
    pub transaction: Option<NovalnetCaptureTransactionData>,
}

impl<F> TryFrom<ResponseRouterData<NovalnetCaptureResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<NovalnetCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let transaction_id = item
            .response
            .transaction
            .as_ref()
            .and_then(|data| data.tid)
            .map(|tid| tid.to_string());

        match item.response.result.status {
            NovalnetAPIStatus::Success => {
                let transaction_status_code = item
                    .response
                    .transaction
                    .as_ref()
                    .and_then(|transaction_data| transaction_data.status_code);
                let transaction_status = item
                    .response
                    .transaction
                    .and_then(|transaction_data| transaction_data.status)
                    // spec: ## Status Mappings (transaction or its status absent -> Pending)
                    .unwrap_or(NovalnetTransactionStatus::Pending);

                let status = match transaction_status {
                    // spec: ## Status Mappings — a SUCCESS envelope can still carry a failed
                    // capture. A rejected capture leaves the authorization intact, so it is
                    // CaptureFailed, not a failed payment.
                    NovalnetTransactionStatus::Failure | NovalnetTransactionStatus::Error => {
                        return Ok(Self {
                            resource_common_data: PaymentFlowData {
                                status: common_enums::AttemptStatus::CaptureFailed,
                                ..item.router_data.resource_common_data
                            },
                            response: Err(get_transaction_decline_error_response(
                                item.response.result,
                                transaction_status_code,
                                transaction_status,
                                item.http_code,
                                transaction_id,
                                Some(FlowStatus::Payment(
                                    common_enums::AttemptStatus::CaptureFailed,
                                )),
                            )),
                            ..item.router_data
                        });
                    }
                    // spec: ## Status Mappings (CONFIRMED / SUCCESS -> Charged, ON_HOLD ->
                    // Authorized, PENDING -> Pending, PROGRESS -> AuthenticationPending,
                    // DEACTIVATED -> Voided, unknown -> current status retained)
                    NovalnetTransactionStatus::Confirmed
                    | NovalnetTransactionStatus::Success
                    | NovalnetTransactionStatus::OnHold
                    | NovalnetTransactionStatus::Pending
                    | NovalnetTransactionStatus::Progress
                    | NovalnetTransactionStatus::Deactivated
                    | NovalnetTransactionStatus::Unknown => get_novalnet_attempt_status(
                        transaction_status,
                        item.router_data.resource_common_data.status,
                    ),
                };

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        ..item.router_data.resource_common_data
                    },
                    response: Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: transaction_id
                            .clone()
                            .map(ResponseId::ConnectorTransactionId)
                            .unwrap_or(ResponseId::NoResponseId),
                        redirection_data: None,
                        mandate_reference: None,
                        connector_metadata: None,
                        network_txn_id: None,
                        network_txn_link_id: None,
                        connector_response_reference_id: transaction_id.clone(),
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                        splits: None,
                        payment_account_reference: None,
                    }),
                    ..item.router_data
                })
            }
            // spec: ## HTTP Codes and Errors — result.status FAILURE rejects the capture; the
            // authorization survives, so the attempt is CaptureFailed.
            NovalnetAPIStatus::Failure => Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::CaptureFailed,
                    ..item.router_data.resource_common_data
                },
                response: Err(get_error_response(
                    item.response.result,
                    item.http_code,
                    transaction_id,
                    Some(FlowStatus::Payment(
                        common_enums::AttemptStatus::CaptureFailed,
                    )),
                )),
                ..item.router_data
            }),
            // An undocumented API status is not a decline: surface the error, keep the status.
            NovalnetAPIStatus::Unknown => Ok(Self {
                response: Err(get_error_response(
                    item.response.result,
                    item.http_code,
                    transaction_id,
                    None,
                )),
                ..item.router_data
            }),
        }
    }
}

#[derive(Default, Debug, Serialize)]
pub struct NovalnetSyncTransaction {
    tid: String,
}

#[derive(Default, Debug, Serialize)]
pub struct NovalnetSyncRequest {
    pub transaction: NovalnetSyncTransaction,
    pub custom: NovalnetCustom,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NovalnetRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for NovalnetSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: NovalnetRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let transaction = NovalnetSyncTransaction {
            tid: item.router_data.request.connector_transaction_id.clone(),
        };

        let custom = NovalnetCustom {
            lang: item
                .router_data
                .request
                .get_optional_language_from_browser_info()
                .unwrap_or(DEFAULT_LOCALE.to_string().to_string()),
        };
        Ok(Self {
            transaction,
            custom,
        })
    }
}

impl<F> TryFrom<ResponseRouterData<NovalnetRefundSyncResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<NovalnetRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // The lookup is keyed by the parent payment tid, so `transaction.tid` in the response is
        // the payment's tid, not the refund's: echo the refund id the caller already holds.
        let refund_id = item.router_data.request.connector_refund_id.clone();
        // An undocumented transaction status retains the refund's current status (HS parity).
        let prev_refund_status = item.router_data.request.refund_status;

        match item.response.result.status {
            NovalnetAPIStatus::Success => {
                let transaction_status = item
                    .response
                    .transaction
                    .map(|transaction_data| transaction_data.status)
                    .unwrap_or(NovalnetTransactionStatus::Pending);

                Ok(Self {
                    response: Ok(RefundsResponseData {
                        connector_refund_id: refund_id,
                        refund_status: get_novalnet_refund_status(
                            transaction_status,
                            prev_refund_status,
                        ),
                        status_code: item.http_code,
                        acquirer_reference_number: None,
                    }),
                    ..item.router_data
                })
            }
            // spec: ## HTTP Codes and Errors - a failed lookup says nothing about the refund itself,
            // so the error carries no attempt status.
            NovalnetAPIStatus::Failure | NovalnetAPIStatus::Unknown => {
                let response = Err(get_error_response(
                    item.response.result,
                    item.http_code,
                    Some(refund_id),
                    None,
                ));
                Ok(Self {
                    response,
                    ..item.router_data
                })
            }
        }
    }
}

#[derive(Default, Debug, Serialize)]
pub struct NovalnetCancelTransaction {
    tid: String,
}

#[derive(Default, Debug, Serialize)]
pub struct NovalnetCancelRequest {
    pub transaction: NovalnetCancelTransaction,
    pub custom: NovalnetCustom,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NovalnetRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for NovalnetCancelRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: NovalnetRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let transaction = NovalnetCancelTransaction {
            tid: item.router_data.request.connector_transaction_id.clone(),
        };

        let custom = NovalnetCustom {
            lang: item
                .router_data
                .request
                .get_optional_language_from_browser_info()
                .unwrap_or(DEFAULT_LOCALE.to_string().to_string()),
        };
        Ok(Self {
            transaction,
            custom,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NovalnetCancelResponse {
    result: ResultData,
    transaction: Option<NovalnetPaymentsResponseTransactionData>,
}

impl<F> TryFrom<ResponseRouterData<NovalnetCancelResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NovalnetCancelResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let transaction_id = item
            .response
            .transaction
            .clone()
            .and_then(|data| data.tid.map(|tid| tid.to_string()));

        match item.response.result.status {
            NovalnetAPIStatus::Success => {
                let status = match item
                    .response
                    .transaction
                    .and_then(|transaction_data| transaction_data.status)
                {
                    // spec: ## Status Mappings (DEACTIVATED -> Voided: the hold was released)
                    Some(NovalnetTransactionStatus::Deactivated) => {
                        common_enums::AttemptStatus::Voided
                    }
                    // spec: ## Status Mappings (any other documented status after a cancel
                    // call means the transaction was not deactivated -> VoidFailed)
                    Some(
                        NovalnetTransactionStatus::Confirmed
                        | NovalnetTransactionStatus::Success
                        | NovalnetTransactionStatus::OnHold
                        | NovalnetTransactionStatus::Pending
                        | NovalnetTransactionStatus::Progress
                        | NovalnetTransactionStatus::Failure
                        | NovalnetTransactionStatus::Error,
                    ) => common_enums::AttemptStatus::VoidFailed,
                    // spec: ## Status Mappings ((unknown) -> previous status retained)
                    Some(NovalnetTransactionStatus::Unknown) => {
                        item.router_data.resource_common_data.status
                    }
                    // No transaction status in a SUCCESS envelope: the cancel is unconfirmed,
                    // so the void is not reported as done (Hyperswitch parity -> VoidFailed).
                    None => common_enums::AttemptStatus::VoidFailed,
                };
                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        ..item.router_data.resource_common_data
                    },

                    response: Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: transaction_id
                            .clone()
                            .map(ResponseId::ConnectorTransactionId)
                            .unwrap_or(ResponseId::NoResponseId),
                        redirection_data: None,
                        mandate_reference: None,
                        connector_metadata: None,
                        network_txn_id: None,
                        network_txn_link_id: None,
                        connector_response_reference_id: transaction_id.clone(),
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                        splits: None,
                        payment_account_reference: None,
                    }),
                    ..item.router_data
                })
            }
            // spec: ## HTTP Codes and Errors — result.status FAILURE rejects the cancel; the
            // authorization survives, so the attempt is VoidFailed, not Failure.
            NovalnetAPIStatus::Failure => Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::VoidFailed,
                    ..item.router_data.resource_common_data
                },
                response: Err(get_error_response(
                    item.response.result,
                    item.http_code,
                    transaction_id,
                    Some(FlowStatus::Payment(common_enums::AttemptStatus::VoidFailed)),
                )),
                ..item.router_data
            }),
            // An undocumented API status is not a decline: surface the error, keep the status.
            NovalnetAPIStatus::Unknown => Ok(Self {
                response: Err(get_error_response(
                    item.response.result,
                    item.http_code,
                    transaction_id,
                    None,
                )),
                ..item.router_data
            }),
        }
    }
}

//TODO: Fill the struct with respective fields
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct NovalnetErrorResponse {
    pub status_code: u64,
    pub code: String,
    pub message: String,
    pub reason: Option<String>,
}

#[derive(Display, Debug, Serialize, Deserialize)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WebhookEventType {
    Payment,
    TransactionCapture,
    TransactionCancel,
    TransactionRefund,
    Chargeback,
    // spec: ## Webhook Events (RETURN_DEBIT: SEPA return debit, handled with chargebacks)
    ReturnDebit,
    // spec: ## Webhook Events (REVERSAL: chargeback-like)
    Reversal,
    Credit,
    // spec: ## Webhook Events (TRANSACTION_UPDATE, INSTALMENT, PAYMENT_REMINDER_*, ... ->
    // not supported). Catch-all so an undocumented `event.type` still decodes; it is never
    // part of a valid checksum preimage, so verification of such a body fails closed.
    #[serde(other)]
    Unknown,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NovalnetWebhookEvent {
    pub checksum: String,
    pub tid: i64,
    pub parent_tid: Option<i64>,
    #[serde(rename = "type")]
    pub event_type: WebhookEventType,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum NovalnetWebhookTransactionData {
    SyncTransactionData(NovalnetSyncResponseTransactionData),
    CaptureTransactionData(NovalnetCaptureTransactionData),
    CancelTransactionData(NovalnetPaymentsResponseTransactionData),
    RefundsTransactionData(NovalnetRefundsTransactionData),
}
impl NovalnetWebhookTransactionData {
    /// The unsigned `transaction.status`, whichever variant the body decoded as. It is outside
    /// the checksum preimage, so it may only lower an outcome, never raise one.
    pub fn transaction_status(&self) -> Option<NovalnetTransactionStatus> {
        match self {
            Self::SyncTransactionData(data) => Some(data.status),
            Self::CaptureTransactionData(data) => data.status,
            Self::CancelTransactionData(data) => data.status,
            Self::RefundsTransactionData(_) => None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NovalnetWebhookNotificationResponse {
    pub event: NovalnetWebhookEvent,
    pub result: ResultData,
    pub transaction: NovalnetWebhookTransactionData,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NovalnetWebhookNotificationResponseRefunds {
    pub event: NovalnetWebhookEvent,
    pub result: ResultData,
    pub transaction: NovalnetRefundsTransactionData,
}

pub fn is_refund_event(event_code: &WebhookEventType) -> bool {
    matches!(event_code, WebhookEventType::TransactionRefund)
}

pub fn reverse_string(s: &str) -> String {
    s.chars().rev().collect()
}

#[derive(Display, Debug, Serialize, Deserialize)]
pub enum WebhookDisputeStatus {
    DisputeOpened,
    DisputeWon,
    Unknown,
}

pub fn get_novalnet_dispute_status(status: WebhookEventType) -> WebhookDisputeStatus {
    match status {
        // spec: ## Webhook Events (CHARGEBACK / RETURN_DEBIT / REVERSAL -> dispute opened)
        WebhookEventType::Chargeback
        | WebhookEventType::ReturnDebit
        | WebhookEventType::Reversal => WebhookDisputeStatus::DisputeOpened,
        // spec: ## Webhook Events (CREDIT -> DisputeWon)
        WebhookEventType::Credit => WebhookDisputeStatus::DisputeWon,
        // spec: ## Webhook Events (non-dispute events carry no dispute status)
        WebhookEventType::Payment
        | WebhookEventType::TransactionCapture
        | WebhookEventType::TransactionCancel
        | WebhookEventType::TransactionRefund
        | WebhookEventType::Unknown => WebhookDisputeStatus::Unknown,
    }
}

impl ForeignTryFrom<WebhookDisputeStatus> for common_enums::DisputeStatus {
    type Error = IntegrationError;

    fn foreign_try_from(value: WebhookDisputeStatus) -> error_stack::Result<Self, Self::Error> {
        match value {
            WebhookDisputeStatus::DisputeOpened => Ok(Self::DisputeOpened),
            WebhookDisputeStatus::DisputeWon => Ok(Self::DisputeWon),
            WebhookDisputeStatus::Unknown => Err(IntegrationError::NotImplemented(
                "webhook body decoding failed".to_string(),
                Default::default(),
            ))?,
        }
    }
}

pub fn option_to_result<T>(opt: Option<T>) -> Result<T, IntegrationError> {
    opt.ok_or(IntegrationError::NotImplemented(
        "webhook body decoding failed".to_string(),
        Default::default(),
    ))
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NovalnetRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NovalnetPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: NovalnetRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, error_stack::Report<IntegrationError>> {
        let auth = NovalnetAuthType::try_from(&item.router_data.connector_config)?;

        let merchant = NovalnetPaymentsRequestMerchant {
            signature: auth.product_activation_key,
            tariff: auth.tariff_id,
        };

        let enforce_3d = match item.router_data.resource_common_data.auth_type {
            common_enums::AuthenticationType::ThreeDs => Some(1),
            common_enums::AuthenticationType::NoThreeDs => None,
        };
        let test_mode = get_test_mode(item.router_data.resource_common_data.test_mode);
        let req_address = item.router_data.resource_common_data.get_optional_billing();

        let billing = NovalnetPaymentsRequestBilling {
            house_no: item
                .router_data
                .resource_common_data
                .get_optional_billing_line1(),
            street: item
                .router_data
                .resource_common_data
                .get_optional_billing_line2(),
            city: item
                .router_data
                .resource_common_data
                .get_optional_billing_city(),
            zip: item
                .router_data
                .resource_common_data
                .get_optional_billing_zip(),
            country_code: item
                .router_data
                .resource_common_data
                .get_optional_billing_country(),
        };

        let email = item
            .router_data
            .resource_common_data
            .get_billing_email()
            .or(item.router_data.request.get_email())?;

        let customer = NovalnetPaymentsRequestCustomer {
            first_name: req_address.and_then(|addr| addr.get_optional_first_name()),
            last_name: req_address.and_then(|addr| addr.get_optional_last_name()),
            email,
            mobile: item
                .router_data
                .resource_common_data
                .get_optional_billing_phone_number(),
            billing: Some(billing),
            shipping: get_novalnet_shipping(&item.router_data.resource_common_data),
            // no_nc is used to indicate if minimal customer data is passed or not
            no_nc: MINIMAL_CUSTOMER_DATA_PASSED,
            // DOB should be populated in case of SepaGuaranteedBankDebit payments
            birth_date: None,
        };

        let lang = item
            .router_data
            .request
            .get_optional_language_from_browser_info()
            .unwrap_or(DEFAULT_LOCALE.to_string().to_string());

        let custom = NovalnetCustom { lang };
        let hook_url = item.router_data.request.get_webhook_url()?;
        let return_url = item.router_data.request.get_router_return_url()?;
        let create_token = Some(CREATE_TOKEN_REQUIRED);

        match item.router_data.request.payment_method_data {
            PaymentMethodData::Card(ref req_card) => {
                let novalnet_card = NovalNetPaymentData::Card(NovalnetCard {
                    card_number: req_card.card_number.clone(),
                    card_expiry_month: req_card.card_exp_month.clone(),
                    card_expiry_year: req_card.card_exp_year.clone(),
                    card_cvc: req_card.card_cvc.clone(),
                    card_holder: item
                        .router_data
                        .resource_common_data
                        .get_billing_address()?
                        .get_full_name()?,
                });

                let transaction = NovalnetPaymentsRequestTransaction {
                    test_mode,
                    payment_type: NovalNetPaymentTypes::CREDITCARD,
                    amount: NovalNetAmount::Int(0),
                    currency: item.router_data.request.currency,
                    order_no: item
                        .router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                    hook_url: Some(hook_url),
                    return_url: Some(return_url.clone()),
                    error_return_url: Some(return_url.clone()),
                    payment_data: Some(novalnet_card),
                    enforce_3d,
                    create_token,
                };

                Ok(Self {
                    merchant,
                    transaction,
                    customer,
                    custom,
                })
            }

            PaymentMethodData::Wallet(ref wallet_data) => match wallet_data {
                WalletDataPaymentMethod::GooglePay(ref req_wallet) => {
                    // Novalnet documents no transaction.payment_data field for a decrypted
                    // Google Pay DPAN/cryptogram, so decrypted data is refused before any call.
                    let encrypted_token = get_novalnet_gpay_encrypted_token(
                        &req_wallet.tokenization_data,
                        "SetupMandate",
                    )?;
                    let novalnet_google_pay: NovalNetPaymentData<T> =
                        NovalNetPaymentData::GooglePay(NovalnetGooglePay {
                            wallet_data: Secret::new(encrypted_token),
                        });

                    let transaction = NovalnetPaymentsRequestTransaction {
                        test_mode,
                        payment_type: NovalNetPaymentTypes::GOOGLEPAY,
                        amount: NovalNetAmount::Int(0),
                        currency: item.router_data.request.currency,
                        order_no: item
                            .router_data
                            .resource_common_data
                            .connector_request_reference_id
                            .clone(),
                        hook_url: Some(hook_url),
                        return_url: None,
                        error_return_url: None,
                        payment_data: Some(novalnet_google_pay),
                        enforce_3d,
                        create_token,
                    };

                    Ok(Self {
                        merchant,
                        transaction,
                        customer,
                        custom,
                    })
                }
                WalletDataPaymentMethod::ApplePay(payment_method_data) => {
                    // Novalnet documents no transaction.payment_data field for a decrypted
                    // Apple Pay DPAN/cryptogram, so decrypted data is refused before any call.
                    reject_novalnet_decrypted_apple_pay(
                        &payment_method_data.payment_data,
                        "SetupMandate",
                    )?;
                    let transaction = NovalnetPaymentsRequestTransaction {
                        test_mode,
                        payment_type: NovalNetPaymentTypes::APPLEPAY,
                        amount: NovalNetAmount::Int(0),
                        currency: item.router_data.request.currency,
                        order_no: item
                            .router_data
                            .resource_common_data
                            .connector_request_reference_id
                            .clone(),
                        hook_url: Some(hook_url),
                        return_url: None,
                        error_return_url: None,
                        payment_data: Some(NovalNetPaymentData::ApplePay(NovalnetApplePay {
                            wallet_data: payment_method_data.get_applepay_decoded_payment_data()?,
                        })),
                        enforce_3d: None,
                        create_token,
                    };

                    Ok(Self {
                        merchant,
                        transaction,
                        customer,
                        custom,
                    })
                }
                WalletDataPaymentMethod::AliPayQr(_)
                | WalletDataPaymentMethod::BluecodeRedirect { .. }
                | WalletDataPaymentMethod::AliPayRedirect(_)
                | WalletDataPaymentMethod::AliPayHkRedirect(_)
                | WalletDataPaymentMethod::AmazonPayRedirect(_)
                | WalletDataPaymentMethod::MomoRedirect(_)
                | WalletDataPaymentMethod::KakaoPayRedirect(_)
                | WalletDataPaymentMethod::GoPayRedirect(_)
                | WalletDataPaymentMethod::GcashRedirect(_)
                | WalletDataPaymentMethod::ApplePayRedirect(_)
                | WalletDataPaymentMethod::ApplePayThirdPartySdk(_)
                | WalletDataPaymentMethod::DanaRedirect {}
                | WalletDataPaymentMethod::GrabpayRedirect {}
                | WalletDataPaymentMethod::GooglePayRedirect(_)
                | WalletDataPaymentMethod::GooglePayThirdPartySdk(_)
                | WalletDataPaymentMethod::MbWayRedirect(_)
                | WalletDataPaymentMethod::MobilePayRedirect(_)
                | WalletDataPaymentMethod::RevolutPay(_) => Err(IntegrationError::NotImplemented(
                    utils::get_unimplemented_payment_method_error_message("novalnet"),
                    Default::default(),
                ))?,
                WalletDataPaymentMethod::PaypalRedirect(_) => {
                    let transaction = NovalnetPaymentsRequestTransaction {
                        test_mode,
                        payment_type: NovalNetPaymentTypes::PAYPAL,
                        amount: NovalNetAmount::Int(0),
                        currency: item.router_data.request.currency,
                        order_no: item
                            .router_data
                            .resource_common_data
                            .connector_request_reference_id
                            .clone(),
                        hook_url: Some(hook_url),
                        return_url: Some(return_url.clone()),
                        error_return_url: Some(return_url.clone()),
                        payment_data: None,
                        enforce_3d: None,
                        create_token,
                    };

                    Ok(Self {
                        merchant,
                        transaction,
                        customer,
                        custom,
                    })
                }
                WalletDataPaymentMethod::PaypalSdk(_)
                | WalletDataPaymentMethod::Paze(_)
                | WalletDataPaymentMethod::SamsungPay(_)
                | WalletDataPaymentMethod::TwintRedirect {}
                | WalletDataPaymentMethod::VippsRedirect {}
                | WalletDataPaymentMethod::TouchNGoRedirect(_)
                | WalletDataPaymentMethod::WeChatPayRedirect(_)
                | WalletDataPaymentMethod::CashappQr(_)
                | WalletDataPaymentMethod::SwishQr(_)
                | WalletDataPaymentMethod::WeChatPayQr(_)
                | WalletDataPaymentMethod::Mifinity(_)
                | WalletDataPaymentMethod::MbWay(_)
                | WalletDataPaymentMethod::Satispay(_)
                | WalletDataPaymentMethod::SatispayIntent(_)
                | WalletDataPaymentMethod::SatispayQr(_)
                | WalletDataPaymentMethod::Wero(_)
                | WalletDataPaymentMethod::LazyPayRedirect(_)
                | WalletDataPaymentMethod::PhonePeRedirect(_)
                | WalletDataPaymentMethod::BillDeskRedirect(_)
                | WalletDataPaymentMethod::CashfreeRedirect(_)
                | WalletDataPaymentMethod::PayURedirect(_)
                | WalletDataPaymentMethod::EaseBuzzRedirect(_)
                | WalletDataPaymentMethod::PaymayaRedirect(_)
                | WalletDataPaymentMethod::PayhereRedirect {}
                | WalletDataPaymentMethod::QwikcilverWalletDirect(_)
                | WalletDataPaymentMethod::Skrill(_)
                | WalletDataPaymentMethod::Neteller(_) => Err(IntegrationError::NotImplemented(
                    utils::get_unimplemented_payment_method_error_message("novalnet"),
                    Default::default(),
                ))?,
            },
            _ => Err(IntegrationError::NotImplemented(
                utils::get_unimplemented_payment_method_error_message("novalnet"),
                Default::default(),
            ))?,
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NovalnetRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NovalnetPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: NovalnetRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = NovalnetAuthType::try_from(&item.router_data.connector_config)?;

        let merchant = NovalnetPaymentsRequestMerchant {
            signature: auth.product_activation_key,
            tariff: auth.tariff_id,
        };

        let enforce_3d = match item.router_data.resource_common_data.auth_type {
            common_enums::AuthenticationType::ThreeDs => Some(1),
            common_enums::AuthenticationType::NoThreeDs => None,
        };
        let test_mode = get_test_mode(item.router_data.resource_common_data.test_mode);

        let billing = NovalnetPaymentsRequestBilling {
            house_no: item
                .router_data
                .resource_common_data
                .get_optional_billing_line1(),
            street: item
                .router_data
                .resource_common_data
                .get_optional_billing_line2(),
            city: item
                .router_data
                .resource_common_data
                .get_optional_billing_city(),
            zip: item
                .router_data
                .resource_common_data
                .get_optional_billing_zip(),
            country_code: item
                .router_data
                .resource_common_data
                .get_optional_billing_country(),
        };

        let customer = NovalnetPaymentsRequestCustomer {
            first_name: item
                .router_data
                .resource_common_data
                .get_optional_billing_first_name(),
            last_name: item
                .router_data
                .resource_common_data
                .get_optional_billing_last_name(),
            email: item
                .router_data
                .resource_common_data
                .get_billing_email()
                .or(item.router_data.request.get_email())?,
            mobile: item
                .router_data
                .resource_common_data
                .get_optional_billing_phone_number(),
            billing: Some(billing),
            shipping: get_novalnet_shipping(&item.router_data.resource_common_data),
            // no_nc is used to indicate if minimal customer data is passed or not
            no_nc: MINIMAL_CUSTOMER_DATA_PASSED,
            // DOB should be populated in case of SepaGuaranteedBankDebit payments
            birth_date: None,
        };

        let lang = item
            .router_data
            .request
            .get_optional_language_from_browser_info()
            .unwrap_or(DEFAULT_LOCALE.to_string().to_string());
        let custom = NovalnetCustom { lang };
        let hook_url = item.router_data.request.get_webhook_url()?;
        // A recurring token MIT never sends return_url (techspec Field Dependency Analysis,
        // RepeatPayment), so it is optional here: only the NTI arm forwards it when present.
        // A blank or whitespace-only router_return_url is treated as absent, never forwarded.
        let return_url = item
            .router_data
            .request
            .router_return_url
            .clone()
            .filter(|url| !url.trim().is_empty());

        let amount = item
            .connector
            .amount_converter
            .convert(&item.router_data.request.amount)
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        // Novalnet offers automatic (/v2/payment) and a single manual (/v2/authorize) capture
        // only, so multiple partial captures and scheduled captures are refused for every arm.
        validate_novalnet_capture_method(
            item.router_data.request.capture_method,
            " for recurring payments",
        )?;

        match item.router_data.request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(mandate_data) => {
                let connector_mandate_id = mandate_data.get_connector_mandate_id().ok_or(
                    IntegrationError::MissingRequiredField {
                        field_name: "connector_mandate_id",
                        context: Default::default(),
                    },
                )?;

                let novalnet_mandate_data = NovalNetPaymentData::MandatePayment(NovalnetMandate {
                    token: Secret::new(connector_mandate_id),
                });

                let payment_type = match item.router_data.request.payment_method_type {
                    Some(pm_type) => NovalNetPaymentTypes::try_from(&pm_type)?,
                    None => NovalNetPaymentTypes::CREDITCARD,
                };

                let transaction = NovalnetPaymentsRequestTransaction {
                    test_mode,
                    payment_type,
                    amount: NovalNetAmount::StringMinor(amount.clone()),
                    currency: item.router_data.request.currency,
                    order_no: item
                        .router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                    hook_url: Some(hook_url),
                    return_url: None,
                    error_return_url: None,
                    payment_data: Some(novalnet_mandate_data),
                    enforce_3d,
                    create_token: None,
                };

                Ok(Self {
                    merchant,
                    transaction,
                    customer,
                    custom,
                })
            }
            MandateReferenceId::NetworkMandateId(network_transaction_id) => {
                match item.router_data.request.payment_method_data {
                    PaymentMethodData::CardDetailsForNetworkTransactionId(ref raw_card_details) => {
                        let novalnet_card =
                            NovalNetPaymentData::RawCardForNTI(NovalnetRawCardDetails {
                                card_number: raw_card_details
                                    .card_number
                                    .try_card_number("Novalnet")?,
                                card_expiry_month: raw_card_details.card_exp_month.clone(),
                                card_expiry_year: raw_card_details.card_exp_year.clone(),
                                scheme_tid: network_transaction_id.network_transaction_id.into(),
                            });

                        let transaction = NovalnetPaymentsRequestTransaction {
                            test_mode,
                            payment_type: NovalNetPaymentTypes::CREDITCARD,
                            amount: NovalNetAmount::StringMinor(amount.clone()),
                            currency: item.router_data.request.currency,
                            order_no: item
                                .router_data
                                .resource_common_data
                                .connector_request_reference_id
                                .clone(),
                            hook_url: Some(hook_url),
                            return_url: return_url.clone(),
                            error_return_url: return_url,
                            payment_data: Some(novalnet_card),
                            enforce_3d,
                            create_token: Some(CREATE_TOKEN_REQUIRED),
                        };

                        Ok(Self {
                            merchant,
                            transaction,
                            customer,
                            custom,
                        })
                    }
                    _ => Err(IntegrationError::NotImplemented(
                        utils::get_unimplemented_payment_method_error_message("novalnet"),
                        Default::default(),
                    )
                    .into()),
                }
            }
            _ => Err(IntegrationError::NotImplemented(
                utils::get_unimplemented_payment_method_error_message("novalnet"),
                Default::default(),
            )
            .into()),
        }
    }
}

/// The event class comes from the signed checksum preimage (`event.type` + `result.status`).
///
/// `transaction_status` is unsigned, so it is used under the same lower-only rule as the
/// attempt status: it may narrow a signed `PAYMENT`/`SUCCESS` to the authorization-success or
/// processing variant (which is what an ON_HOLD or PENDING transaction actually is, and what
/// this returned before the flow was reworked), never widen anything. Editing it on a validly
/// signed body can therefore only under-report an event.
pub fn get_incoming_webhook_event(
    event_type: WebhookEventType,
    result_status: NovalnetAPIStatus,
    transaction_status: Option<NovalnetTransactionStatus>,
) -> EventType {
    // Narrow PAYMENT/SUCCESS to match the status HandleEvent reports for the same body.
    if let (WebhookEventType::Payment, NovalnetAPIStatus::Success) = (&event_type, &result_status) {
        match transaction_status {
            Some(NovalnetTransactionStatus::OnHold) => {
                return EventType::PaymentIntentAuthorizationSuccess
            }
            Some(NovalnetTransactionStatus::Pending | NovalnetTransactionStatus::Progress) => {
                return EventType::PaymentIntentProcessing
            }
            _ => {}
        }
    }
    match (event_type, result_status) {
        // spec: ### Webhook Authentication & Signature Verification (undocumented result.status
        // stays non-terminal)
        (_, NovalnetAPIStatus::Unknown) => EventType::IncomingWebhookEventUnspecified,
        // spec: ## Webhook Events (PAYMENT + SUCCESS)
        (WebhookEventType::Payment, NovalnetAPIStatus::Success) => EventType::PaymentIntentSuccess,
        // spec: ## Webhook Events (PAYMENT + FAILURE)
        (WebhookEventType::Payment, NovalnetAPIStatus::Failure) => EventType::PaymentIntentFailure,
        // spec: ## Webhook Events (TRANSACTION_CAPTURE + SUCCESS)
        (WebhookEventType::TransactionCapture, NovalnetAPIStatus::Success) => {
            EventType::PaymentIntentCaptureSuccess
        }
        // spec: ## Webhook Events (TRANSACTION_CAPTURE + FAILURE)
        (WebhookEventType::TransactionCapture, NovalnetAPIStatus::Failure) => {
            EventType::PaymentIntentCaptureFailure
        }
        // spec: ## Webhook Events (TRANSACTION_CANCEL + SUCCESS)
        (WebhookEventType::TransactionCancel, NovalnetAPIStatus::Success) => {
            EventType::PaymentIntentCancelled
        }
        // spec: ## Webhook Events (TRANSACTION_CANCEL + FAILURE)
        (WebhookEventType::TransactionCancel, NovalnetAPIStatus::Failure) => {
            EventType::PaymentIntentCancelFailure
        }
        // spec: ## Webhook Events (TRANSACTION_REFUND + SUCCESS)
        (WebhookEventType::TransactionRefund, NovalnetAPIStatus::Success) => {
            EventType::RefundSuccess
        }
        // spec: ## Webhook Events (TRANSACTION_REFUND + FAILURE)
        (WebhookEventType::TransactionRefund, NovalnetAPIStatus::Failure) => {
            EventType::RefundFailure
        }
        // spec: ## Webhook Events (CHARGEBACK / RETURN_DEBIT / REVERSAL -> dispute opened)
        (
            WebhookEventType::Chargeback
            | WebhookEventType::ReturnDebit
            | WebhookEventType::Reversal,
            NovalnetAPIStatus::Success | NovalnetAPIStatus::Failure,
        ) => EventType::DisputeOpened,
        // spec: ## Webhook Events (CREDIT -> DisputeWon)
        (WebhookEventType::Credit, NovalnetAPIStatus::Success | NovalnetAPIStatus::Failure) => {
            EventType::DisputeWon
        }
        // spec: ## Webhook Events (TRANSACTION_UPDATE, INSTALMENT, ... -> EventNotSupported)
        (WebhookEventType::Unknown, NovalnetAPIStatus::Success | NovalnetAPIStatus::Failure) => {
            EventType::IncomingWebhookEventUnspecified
        }
    }
}

/// TH-17: reached only on a signed `result.status` = SUCCESS. `transaction.status` is unsigned:
/// it may only lower the outcome to non-terminal, never raise it. Authorized vs Charged for a
/// PAYMENT comes from the original authorize's `capture_method` (EventContext).
pub fn get_novalnet_webhook_attempt_status(
    event_type: &WebhookEventType,
    transaction_status: NovalnetTransactionStatus,
    capture_method: Option<common_enums::CaptureMethod>,
) -> common_enums::AttemptStatus {
    use common_enums::{AttemptStatus, CaptureMethod};
    match event_type {
        // spec: ## Webhook Events (TRANSACTION_CAPTURE + SUCCESS -> captured)
        WebhookEventType::TransactionCapture => AttemptStatus::Charged,
        // spec: ## Webhook Events (TRANSACTION_CANCEL + SUCCESS -> cancelled)
        WebhookEventType::TransactionCancel => AttemptStatus::Voided,
        // spec: ### Webhook Authentication & Signature Verification (PAYMENT + SUCCESS)
        WebhookEventType::Payment => {
            match transaction_status {
                NovalnetTransactionStatus::Confirmed | NovalnetTransactionStatus::Success => {
                    match capture_method {
                        Some(CaptureMethod::Automatic | CaptureMethod::SequentialAutomatic) => {
                            AttemptStatus::Charged
                        }
                        Some(
                            CaptureMethod::Manual
                            | CaptureMethod::ManualMultiple
                            | CaptureMethod::Scheduled,
                        ) => AttemptStatus::Authorized,
                        // PAYMENT/SUCCESS is ambiguous without capture_method (the checksum
                        // does not cover transaction.status, so AUTOMATIC -> Charged vs
                        // MANUAL -> Authorized cannot be told apart here). Hyperswitch leaves
                        // it unset whenever the webhook reference is not a payment id, the
                        // attempt lookup fails, or the attempt carries no capture method, so
                        // erroring would drop those webhooks entirely. The same lower-only
                        // rule applies instead: report the lesser of the two and let PSync
                        // raise it to Charged.
                        None => AttemptStatus::Authorized,
                    }
                }
                // spec: ## Status Mappings (ON_HOLD -> Authorized; lower-only, never raised)
                NovalnetTransactionStatus::OnHold => AttemptStatus::Authorized,
                // spec: ## Status Mappings (PENDING -> Pending)
                NovalnetTransactionStatus::Pending => AttemptStatus::Pending,
                // spec: ## Status Mappings (PROGRESS -> AuthenticationPending)
                NovalnetTransactionStatus::Progress => AttemptStatus::AuthenticationPending,
                // Contradicts the signed SUCCESS (or undocumented): never terminal on an unsigned value.
                NovalnetTransactionStatus::Failure
                | NovalnetTransactionStatus::Error
                | NovalnetTransactionStatus::Deactivated
                | NovalnetTransactionStatus::Unknown => AttemptStatus::Pending,
            }
        }
        // Not a payment-attempt event on this path: non-terminal.
        WebhookEventType::TransactionRefund
        | WebhookEventType::Chargeback
        | WebhookEventType::ReturnDebit
        | WebhookEventType::Reversal
        | WebhookEventType::Credit
        | WebhookEventType::Unknown => AttemptStatus::Pending,
    }
}

/// Payment webhook details. The attempt status is derived from the signed `result.status`
/// plus `capture_method` (TH-17); see `get_novalnet_webhook_attempt_status`.
pub fn webhook_payment_details(
    notif: NovalnetWebhookNotificationResponse,
    capture_method: Option<common_enums::CaptureMethod>,
) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
    match notif.transaction {
        NovalnetWebhookTransactionData::SyncTransactionData(response) => {
            let transaction_id = response.tid.map(|tid| tid.to_string());

            match notif.result.status {
                NovalnetAPIStatus::Success => {
                    let mandate_reference_id =
                        NovalnetSyncResponseTransactionData::get_token(Some(&response));
                    let status = get_novalnet_webhook_attempt_status(
                        &notif.event.event_type,
                        response.status,
                        capture_method,
                    );

                    Ok(WebhookDetailsResponse {
                        connector_returned_payment_method_details: None,
                        status,
                        resource_id: Some(
                            transaction_id
                                .clone()
                                .map(ResponseId::ConnectorTransactionId)
                                .unwrap_or(ResponseId::NoResponseId),
                        ),
                        mandate_reference: mandate_reference_id
                            .as_ref()
                            .map(|id| MandateReference {
                                connector_mandate_id: Some(id.clone()),
                                payment_method_id: None,
                                connector_mandate_request_reference_id: None,
                                mandate_metadata: None,
                            })
                            .map(Box::new),
                        status_code: 200,
                        connector_response_reference_id: transaction_id.clone(),
                        connector_request_reference_id: transaction_id.clone(),
                        error_code: None,
                        error_message: None,
                        raw_connector_response: None,
                        response_headers: None,
                        amount_captured: None,
                        error_reason: None,
                        network_txn_id: response.payment_data.and_then(|payment_data| {
                            match payment_data {
                                NovalnetResponsePaymentData::Card(card) => {
                                    card.scheme_tid.map(|tid| tid.expose())
                                }
                                NovalnetResponsePaymentData::Paypal(_) => None,
                            }
                        }),
                        payment_method_update: None,
                        sender_payment_instrument_id: None,
                    })
                }
                // spec: ## Webhook Events (result FAILURE -> payment failed)
                //
                // Which operation failed matters: a rejected capture or cancel leaves the
                // authorization intact, so reporting a flat `Failure` would write off a
                // payment that is still authorized -- and would disagree with the Capture and
                // Void response mappers, which return CaptureFailed / VoidFailed. The event
                // type is inside the signed checksum preimage, so it is safe to branch on.
                NovalnetAPIStatus::Failure => Ok(WebhookDetailsResponse {
                    connector_returned_payment_method_details: None,
                    status: match notif.event.event_type {
                        WebhookEventType::TransactionCapture => {
                            common_enums::AttemptStatus::CaptureFailed
                        }
                        WebhookEventType::TransactionCancel => {
                            common_enums::AttemptStatus::VoidFailed
                        }
                        _ => common_enums::AttemptStatus::Failure,
                    },
                    resource_id: Some(
                        transaction_id
                            .clone()
                            .map(ResponseId::ConnectorTransactionId)
                            .unwrap_or(ResponseId::NoResponseId),
                    ),
                    status_code: 200,
                    mandate_reference: None,
                    connector_response_reference_id: None,
                    connector_request_reference_id: None,
                    error_code: Some(notif.result.status.to_string()),
                    error_message: Some(notif.result.status_text),
                    raw_connector_response: None,
                    response_headers: None,
                    amount_captured: None,
                    error_reason: None,
                    network_txn_id: None,
                    payment_method_update: None,
                    sender_payment_instrument_id: None,
                }),
                // Undocumented result.status: never terminal (o-themes-r3); flow mappers keep
                // the current status for an unknown value.
                NovalnetAPIStatus::Unknown => Ok(WebhookDetailsResponse {
                    connector_returned_payment_method_details: None,
                    status: common_enums::AttemptStatus::Pending,
                    resource_id: Some(
                        transaction_id
                            .clone()
                            .map(ResponseId::ConnectorTransactionId)
                            .unwrap_or(ResponseId::NoResponseId),
                    ),
                    status_code: 200,
                    mandate_reference: None,
                    connector_response_reference_id: transaction_id.clone(),
                    connector_request_reference_id: transaction_id,
                    error_code: None,
                    error_message: None,
                    raw_connector_response: None,
                    response_headers: None,
                    amount_captured: None,
                    error_reason: None,
                    network_txn_id: None,
                    payment_method_update: None,
                    sender_payment_instrument_id: None,
                }),
            }
        }
        // `SyncTransactionData` requires `transaction.status`, but Novalnet omits it on a
        // capture or cancel of anything other than card/wallet/PayPal (SEPA, for one). Those
        // bodies fall through to the all-optional capture/cancel variants, and rejecting them
        // dropped the webhook. They do not need `transaction.status`: the signed event type
        // alone decides the outcome, exactly as `get_novalnet_webhook_attempt_status` does for
        // TRANSACTION_CAPTURE and TRANSACTION_CANCEL.
        NovalnetWebhookTransactionData::CaptureTransactionData(_)
        | NovalnetWebhookTransactionData::CancelTransactionData(_) => {
            // A capture / cancel event's own tid is the operation; the payment it belongs to
            // is parent_tid. Both are inside the signed checksum preimage.
            let payment_tid = notif
                .event
                .parent_tid
                .unwrap_or(notif.event.tid)
                .to_string();
            let status = match (&notif.event.event_type, &notif.result.status) {
                (WebhookEventType::TransactionCapture, NovalnetAPIStatus::Success) => {
                    common_enums::AttemptStatus::Charged
                }
                (WebhookEventType::TransactionCapture, NovalnetAPIStatus::Failure) => {
                    common_enums::AttemptStatus::CaptureFailed
                }
                (WebhookEventType::TransactionCancel, NovalnetAPIStatus::Success) => {
                    common_enums::AttemptStatus::Voided
                }
                (WebhookEventType::TransactionCancel, NovalnetAPIStatus::Failure) => {
                    common_enums::AttemptStatus::VoidFailed
                }
                // Any other pairing on these variants is not a capture/cancel outcome, and an
                // undocumented result.status is never terminal.
                _ => common_enums::AttemptStatus::Pending,
            };
            let error = matches!(notif.result.status, NovalnetAPIStatus::Failure);
            Ok(WebhookDetailsResponse {
                connector_returned_payment_method_details: None,
                status,
                resource_id: Some(ResponseId::ConnectorTransactionId(payment_tid.clone())),
                status_code: 200,
                mandate_reference: None,
                connector_response_reference_id: Some(payment_tid.clone()),
                connector_request_reference_id: Some(payment_tid),
                error_code: error.then(|| notif.result.status.to_string()),
                error_message: error.then_some(notif.result.status_text),
                raw_connector_response: None,
                response_headers: None,
                amount_captured: None,
                error_reason: None,
                network_txn_id: None,
                payment_method_update: None,
                sender_payment_instrument_id: None,
            })
        }
        NovalnetWebhookTransactionData::RefundsTransactionData(_) => Err(error_stack::report!(
            WebhookError::WebhookBodyDecodingFailed
        )
        .attach_printable(
            "Novalnet payment webhook carried a refund transaction, not a payment transaction",
        )),
    }
}

impl TryFrom<NovalnetWebhookNotificationResponseRefunds> for RefundWebhookDetailsResponse {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(notif: NovalnetWebhookNotificationResponseRefunds) -> Result<Self, Self::Error> {
        // spec: ### Webhook Payload Structure (TRANSACTION_REFUND -> refund id = event.tid).
        // `transaction.refund` is absent on a FAILURE body, so fall back to the signed
        // `event.tid`, which is part of the checksum preimage.
        let refund_id = notif
            .transaction
            .refund
            .as_ref()
            .and_then(|refund| refund.tid)
            .unwrap_or(notif.event.tid)
            .to_string();

        match notif.result.status {
            NovalnetAPIStatus::Success => {
                // TH-17: the signed TRANSACTION_REFUND + SUCCESS decides; the unsigned
                // transaction.status may only lower the outcome to non-terminal.
                let status = match notif.transaction.status {
                    // spec: ## Status Mappings (CONFIRMED / SUCCESS -> Success)
                    NovalnetTransactionStatus::Confirmed | NovalnetTransactionStatus::Success => {
                        common_enums::RefundStatus::Success
                    }
                    // Lower-only: never terminal on an unsigned value.
                    NovalnetTransactionStatus::Pending
                    | NovalnetTransactionStatus::Progress
                    | NovalnetTransactionStatus::OnHold
                    | NovalnetTransactionStatus::Failure
                    | NovalnetTransactionStatus::Error
                    | NovalnetTransactionStatus::Deactivated
                    | NovalnetTransactionStatus::Unknown => common_enums::RefundStatus::Pending,
                };

                Ok(Self {
                    connector_refund_id: Some(refund_id),
                    merchant_transaction_id: None,
                    status,
                    status_code: 200,
                    connector_response_reference_id: None,
                    error_code: None,
                    error_message: None,
                    raw_connector_response: None,
                    response_headers: None,
                })
            }
            // Undocumented result.status: never terminal (o-themes-r3).
            NovalnetAPIStatus::Unknown => Ok(Self {
                status: common_enums::RefundStatus::Pending,
                connector_refund_id: Some(refund_id),
                merchant_transaction_id: None,
                status_code: 200,
                connector_response_reference_id: None,
                error_code: None,
                error_message: None,
                raw_connector_response: None,
                response_headers: None,
            }),
            // spec: ## Webhook Events (result FAILURE -> refund failed)
            NovalnetAPIStatus::Failure => Ok(Self {
                status: common_enums::RefundStatus::Failure,
                connector_refund_id: Some(refund_id),
                merchant_transaction_id: None,
                status_code: 200,
                connector_response_reference_id: None,
                error_code: Some(notif.result.status.to_string()),
                error_message: Some(notif.result.status_text),
                raw_connector_response: None,
                response_headers: None,
            }),
        }
    }
}

// =============================================================================
// Incremental Authorization
//
// Novalnet exposes incremental authorization through its transaction amount
// update endpoint: POST https://payport.novalnet.de/v2/transaction/update
// The endpoint allows a merchant to update the authorized amount of an
// existing on-hold / pending transaction (primarily supported for Direct
// Debit SEPA, Direct Debit ACH, Invoice, Prepayment & Barzahlen/viacash).
// The response envelope is the same `{ result, transaction }` shape used by
// capture/void — we reuse the shared `ResultData` type here.
// =============================================================================

#[derive(Debug, Serialize)]
pub struct NovalnetIncrementalAuthTransaction {
    pub tid: String,
    pub amount: StringMinorUnit,
}

#[derive(Debug, Serialize)]
pub struct NovalnetIncrementalAuthRequest {
    pub transaction: NovalnetIncrementalAuthTransaction,
    pub custom: NovalnetCustom,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NovalnetRouterData<
            RouterDataV2<
                IncrementalAuthorization,
                PaymentFlowData,
                PaymentsIncrementalAuthorizationData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NovalnetIncrementalAuthRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: NovalnetRouterData<
            RouterDataV2<
                IncrementalAuthorization,
                PaymentFlowData,
                PaymentsIncrementalAuthorizationData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let tid = item
            .router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(IntegrationError::MissingConnectorTransactionID {
                context: Default::default(),
            })?;

        let amount = item
            .connector
            .amount_converter
            .convert(&item.router_data.request.amount)
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        let transaction = NovalnetIncrementalAuthTransaction { tid, amount };

        let custom = NovalnetCustom {
            lang: DEFAULT_LOCALE.to_string(),
        };

        Ok(Self {
            transaction,
            custom,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NovalnetIncrementalAuthTransactionData {
    pub amount: Option<MinorUnit>,
    pub currency: Option<common_enums::Currency>,
    pub order_no: Option<String>,
    pub payment_type: Option<String>,
    pub status: Option<NovalnetTransactionStatus>,
    pub status_code: Option<u64>,
    pub test_mode: Option<u8>,
    pub tid: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NovalnetIncrementalAuthResponse {
    pub result: ResultData,
    pub transaction: Option<NovalnetIncrementalAuthTransactionData>,
}

impl TryFrom<ResponseRouterData<NovalnetIncrementalAuthResponse, Self>>
    for RouterDataV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NovalnetIncrementalAuthResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let transaction_id = item
            .response
            .transaction
            .as_ref()
            .and_then(|data| data.tid.map(|tid| tid.to_string()));

        let transaction_status = item
            .response
            .transaction
            .as_ref()
            .and_then(|data| data.status);

        match item.response.result.status {
            NovalnetAPIStatus::Success => {
                // Novalnet can return a SUCCESS envelope with a transaction-level
                // status that still indicates the increment was not honoured
                // (Failure/Deactivated). Route those back through the error path
                // rather than falsely reporting AuthorizationStatus::Success.
                let authorization_status = match transaction_status {
                    Some(NovalnetTransactionStatus::Success)
                    | Some(NovalnetTransactionStatus::Confirmed)
                    | Some(NovalnetTransactionStatus::OnHold) => {
                        // OnHold = authorization still held after a successful
                        // amount update (Novalnet keeps the auth in ON_HOLD
                        // until a subsequent capture/void).
                        common_enums::AuthorizationStatus::Success
                    }
                    Some(NovalnetTransactionStatus::Pending)
                    | Some(NovalnetTransactionStatus::Progress)
                    | Some(NovalnetTransactionStatus::Unknown)
                    | None => {
                        // Missing transaction / no definitive signal — let the
                        // caller re-sync rather than optimistically marking
                        // success.
                        common_enums::AuthorizationStatus::Processing
                    }
                    Some(NovalnetTransactionStatus::Failure)
                    | Some(NovalnetTransactionStatus::Error)
                    | Some(NovalnetTransactionStatus::Deactivated) => {
                        common_enums::AuthorizationStatus::Failure
                    }
                };

                if matches!(
                    authorization_status,
                    common_enums::AuthorizationStatus::Failure
                ) {
                    // Incremental-auth failure does not invalidate the
                    // original authorization — leave the attempt status
                    // untouched so the initial auth remains capturable at
                    // its existing amount.
                    return Ok(Self {
                        response: Err(get_error_response(
                            item.response.result,
                            item.http_code,
                            transaction_id,
                            None,
                        )),
                        ..item.router_data
                    });
                }

                Ok(Self {
                    response: Ok(PaymentsResponseData::IncrementalAuthorizationResponse {
                        status: authorization_status,
                        connector_authorization_id: transaction_id,
                        status_code: item.http_code,
                    }),
                    ..item.router_data
                })
            }
            NovalnetAPIStatus::Failure | NovalnetAPIStatus::Unknown => {
                // Incremental-auth failure does not invalidate the original
                // authorization — surface the error but keep the attempt
                // status so the initial auth remains valid.
                Ok(Self {
                    response: Err(get_error_response(
                        item.response.result,
                        item.http_code,
                        transaction_id,
                        None,
                    )),
                    ..item.router_data
                })
            }
        }
    }
}
