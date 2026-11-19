use crate::{types::ResponseRouterData, utils};
use common_enums::{MitCategory, PaymentChannel, RefundStatus};
use common_utils::{types::Money, Email};
use domain_types::{
    connector_flow::{
        Authorize, Capture, IncrementalAuthorization, PostAuthenticate, PreAuthenticate, RSync,
        Refund, RepeatPayment, ServerAuthenticationToken, Void,
    },
    connector_types::{
        MandateReference, PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsIncrementalAuthorizationData, PaymentsPostAuthenticateData,
        PaymentsPreAuthenticateData, PaymentsResponseData, RefundFlowData, RefundSyncData,
        RefundsData, RefundsResponseData, RepeatPaymentData, ResponseId,
        ServerAuthenticationTokenResponseData,
    },
    errors::{
        ConnectorError, IntegrationError, IntegrationErrorContext,
        ResponseTransformationErrorContext,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::{
        ApplePayPaymentData, GpayTokenizationData, PaymentMethodData, PaymentMethodDataTypes,
        RawCardNumber, WalletData,
    },
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_request_types::AuthenticationData,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::MonerisRouterData;

pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
const CLIENT_CREDENTIALS: &str = "client_credentials";

pub mod auth_headers {
    pub const X_MERCHANT_ID: &str = "X-Merchant-Id";
    pub const API_VERSION: &str = "Api-Version";
    pub const API_VERSION_VALUE: &str = "2026-08-14";
}

pub struct MonerisAuthType {
    pub(super) client_id: Secret<String>,
    pub(super) client_secret: Secret<String>,
    pub(super) merchant_id: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for MonerisAuthType {
    type Error = Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Moneris {
                client_secret,
                merchant_id,
                client_id,
                ..
            } => Ok(Self {
                client_id: client_id.to_owned(),
                client_secret: client_secret.to_owned(),
                merchant_id: merchant_id.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Configure this merchant account's Moneris connector with a \
                             client_id, client_secret and merchant_id (ConnectorSpecificConfig::Moneris)."
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "The connector_config passed to MonerisAuthType::try_from was not \
                             the ConnectorSpecificConfig::Moneris variant — either a different \
                             connector's config was routed to Moneris, or Moneris's client_id/ \
                             client_secret/merchant_id were never configured for this merchant account."
                                .to_string(),
                        ),
                    }
                }
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MonerisAuthRequest {
    client_id: Secret<String>,
    client_secret: Secret<String>,
    grant_type: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MonerisRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                domain_types::connector_types::ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for MonerisAuthRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: MonerisRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                domain_types::connector_types::ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = MonerisAuthType::try_from(&item.router_data.connector_config)?;
        Ok(Self {
            client_id: auth.client_id.clone(),
            client_secret: auth.client_secret.clone(),
            grant_type: CLIENT_CREDENTIALS.to_string(),
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MonerisAuthResponse {
    access_token: Secret<String>,
    token_type: String,
    expires_in: String,
}

impl<F, T> TryFrom<ResponseRouterData<MonerisAuthResponse, Self>>
    for RouterDataV2<F, MerchantAuthenticationFlowData, T, ServerAuthenticationTokenResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<MonerisAuthResponse, Self>) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(ServerAuthenticationTokenResponseData {
                access_token: item.response.access_token,
                expires_in: Some(item.response.expires_in.parse::<i64>().change_context(
                    ConnectorError::ResponseDeserializationFailed {
                        context: ResponseTransformationErrorContext {
                            http_status_code: None,
                            additional_context: Some(format!(
                                "Failed to parse Moneris `expires_in` value as i64: {:?}",
                                item.response.expires_in
                            )),
                        },
                    },
                )?),
                token_type: Some(item.response.token_type),
            }),
            ..item.router_data
        })
    }
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MonerisPaymentsRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    idempotency_key: String,
    amount: Money,
    payment_method: PaymentMethod<T>,
    automatic_capture: bool,
    incremental_authorization: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    ecommerce_indicator: Option<EcommerceIndicator>,
    #[serde(skip_serializing_if = "Option::is_none")]
    three_d_secure_data: Option<MonerisThreeDSecureData>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EcommerceIndicator {
    MailTelephoneOrderSingle,
    MailTelephoneOrderRecurring,
    MailTelephoneOrderInstalment,
    MailTelephoneOrderUnknown,
    AuthenticatedEcommerce,
    NonAuthenticatedEcommerce,
    SslMerchant,
}

fn derive_ecommerce_indicator(
    channel: Option<&PaymentChannel>,
    mit_category: Option<&MitCategory>,
    is_three_ds: bool,
    is_authenticated: bool,
    is_recurring: bool,
) -> EcommerceIndicator {
    if !matches!(
        channel,
        Some(PaymentChannel::MailOrder | PaymentChannel::TelephoneOrder)
    ) {
        return match (is_three_ds, is_authenticated) {
            (true, true) => EcommerceIndicator::AuthenticatedEcommerce,
            (true, false) => EcommerceIndicator::NonAuthenticatedEcommerce,
            (false, _) => EcommerceIndicator::SslMerchant,
        };
    }

    match mit_category {
        Some(MitCategory::Installment) => EcommerceIndicator::MailTelephoneOrderInstalment,
        Some(MitCategory::Recurring | MitCategory::Subscription) => {
            EcommerceIndicator::MailTelephoneOrderRecurring
        }
        None if is_recurring => EcommerceIndicator::MailTelephoneOrderRecurring,
        None => EcommerceIndicator::MailTelephoneOrderSingle,
        Some(MitCategory::Unscheduled | MitCategory::Resubmission) => {
            EcommerceIndicator::MailTelephoneOrderUnknown
        }
    }
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MonerisThreeDSecureAuthenticationId {
    three_d_secure_authentication_id: String,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MonerisThreeDSecureCryptogramData {
    three_d_secure_authentication_value: Option<Secret<String>>,
    three_d_secure_version: Option<String>,
    three_d_secure_server_transaction_id: Option<String>,
    three_d_secure_directory_server_transaction_id: String,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(untagged)]
pub enum MonerisThreeDSecureData {
    AuthenticationId(MonerisThreeDSecureAuthenticationId),
    Cryptogram(MonerisThreeDSecureCryptogramData),
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaymentIndicator {
    UnscheduledCredentialOnFile,
    Recurring,
    MerchantInitiated,
    VariableRecurring,
    CustomerInitiated,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaymentInformation {
    First,
    Subsequent,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CredentialOnFileInformation {
    payment_indicator: PaymentIndicator,
    payment_information: PaymentInformation,
    #[serde(skip_serializing_if = "Option::is_none")]
    issuer_id: Option<String>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum PaymentMethod<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    Card(PaymentMethodCard<T>),
    PaymentMethodId(PaymentMethodId),
    ApplePayDecrypted(PaymentMethodApplePayDecrypted),
    GooglePayDecrypted(PaymentMethodGooglePayDecrypted),
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaymentMethodCard<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    payment_method_source: PaymentMethodSource,
    card: MonerisCard<T>,
    store_payment_method: StorePaymentMethod,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaymentMethodId {
    payment_method_source: PaymentMethodSource,
    payment_method_id: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    credential_on_file_information: Option<CredentialOnFileInformation>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaymentMethodSource {
    Card,
    PaymentMethodId,
    ApplePayEncrypted,
    ApplePayDecrypted,
    GooglePayEncrypted,
    GooglePayDecrypted,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WalletIndicator {
    InApplication,
    InBrowser,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MonerisCardBrand {
    Mastercard,
    Visa,
    AmericanExpress,
    Interac,
    Discover,
}

#[derive(Debug, Serialize, PartialEq)]
pub enum ApplePayVersion {
    #[serde(rename = "EC_V1")]
    EcV1,
    #[serde(rename = "RSA_V1")]
    RsaV1,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ApplePayDataType {
    ThreeDSecure,
    Emv,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GooglePayWalletSource {
    Card,
    TokenizedCard,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaymentMethodApplePayDecrypted {
    payment_method_source: PaymentMethodSource,
    application_primary_account_number: Secret<String>,
    expiry_month: Secret<i64>,
    expiry_year: Secret<i64>,
    data_type: ApplePayDataType,
    cryptogram: Secret<String>,
    card_brand: MonerisCardBrand,
    #[serde(skip_serializing_if = "Option::is_none")]
    wallet_ecommerce_indicator: Option<String>,
    wallet_indicator: WalletIndicator,
    #[serde(skip_serializing_if = "Option::is_none")]
    device_manufacturer_identifier: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    store_payment_method: Option<StorePaymentMethod>,
    #[serde(skip_serializing_if = "Option::is_none")]
    credential_on_file_information: Option<CredentialOnFileInformation>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GooglePayDecryptedCardDetails {
    personal_account_number: Secret<String>,
    expiry_month: Secret<i64>,
    expiry_year: Secret<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    authentication_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cryptogram: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    wallet_ecommerce_indicator: Option<String>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaymentMethodGooglePayDecrypted {
    payment_method_source: PaymentMethodSource,
    wallet_source: GooglePayWalletSource,
    card_brand: MonerisCardBrand,
    wallet_indicator: WalletIndicator,
    card_details: GooglePayDecryptedCardDetails,
    #[serde(skip_serializing_if = "Option::is_none")]
    store_payment_method: Option<StorePaymentMethod>,
    #[serde(skip_serializing_if = "Option::is_none")]
    credential_on_file_information: Option<CredentialOnFileInformation>,
}

#[derive(Default, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MonerisCard<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    card_number: RawCardNumber<T>,
    expiry_month: Secret<i64>,
    expiry_year: Secret<i64>,
    card_security_code: Secret<String>,
}

#[derive(Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StorePaymentMethod {
    DoNotStore,
    CardholderInitiated,
    MerchantInitiated,
}

impl TryFrom<&str> for MonerisCardBrand {
    type Error = Report<IntegrationError>;

    fn try_from(network: &str) -> Result<Self, Self::Error> {
        // Uppercase normalises Apple Pay ("masterCard", "amex") and Google Pay ("MASTERCARD")
        // to match the SCREAMING_SNAKE_CASE serde aliases on common_enums::CardNetwork.
        let card_network: common_enums::CardNetwork =
            serde_json::from_value(serde_json::Value::String(network.to_uppercase()))
                .change_context(IntegrationError::NotSupported {
                    message: format!(
                        "Card network '{}' is not supported by Moneris wallet",
                        network
                    ),
                    connector: "Moneris",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Use a Mastercard, Visa, American Express, Interac, or Discover card \
                             with Apple Pay or Google Pay."
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(format!(
                            "Moneris wallet received unrecognised card network string: '{}'",
                            network
                        )),
                    },
                })?;
        Self::try_from(card_network)
    }
}

impl TryFrom<common_enums::CardNetwork> for MonerisCardBrand {
    type Error = Report<IntegrationError>;

    fn try_from(network: common_enums::CardNetwork) -> Result<Self, Self::Error> {
        match network {
            common_enums::CardNetwork::Mastercard => Ok(Self::Mastercard),
            common_enums::CardNetwork::Visa => Ok(Self::Visa),
            common_enums::CardNetwork::AmericanExpress => Ok(Self::AmericanExpress),
            common_enums::CardNetwork::Interac => Ok(Self::Interac),
            common_enums::CardNetwork::Discover => Ok(Self::Discover),
            other => Err(error_stack::report!(IntegrationError::NotSupported {
                message: format!(
                    "Card network '{:?}' is not supported by Moneris wallet",
                    other
                ),
                connector: "Moneris",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Use a Mastercard, Visa, American Express, Interac, or Discover card \
                         with Apple Pay or Google Pay."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(format!(
                        "Moneris wallet does not support card network: {:?}",
                        other
                    )),
                },
            })),
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MonerisRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for MonerisPaymentsRequest<T>
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: MonerisRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let automatic_capture = item.router_data.request.is_auto_capture();
        let incremental_authorization =
            item.router_data.request.request_incremental_authorization == Some(true);
        if incremental_authorization && automatic_capture {
            return Err(IntegrationError::InvalidDataFormat {
                field_name: "capture_method",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Set capture_method=MANUAL when request_incremental_authorization=true."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "Moneris incremental authorization is enabled on a pre-authorization. The request would use automaticCapture=true, which creates a captured purchase."
                            .to_string(),
                    ),
                },
            }
            .into());
        }

        match item.router_data.request.payment_method_data.clone() {
            PaymentMethodData::Card(ref req_card) => {
                let idempotency_key = format!(
                    "auth_{}",
                    item.router_data
                        .resource_common_data
                        .connector_request_reference_id
                );
                let payment_method = PaymentMethod::Card(PaymentMethodCard {
                    payment_method_source: PaymentMethodSource::Card,
                    card: MonerisCard {
                        card_number: req_card.card_number.clone(),
                        expiry_month: Secret::new(
                            req_card
                                .card_exp_month
                                .peek()
                                .parse::<i64>()
                                .change_context(IntegrationError::InvalidDataFormat {
                                    field_name: "card_exp_month",
                                    context: IntegrationErrorContext {
                                        suggested_action: Some(
                                            "Pass card_exp_month as a 1- or 2-digit numeric \
                                             string (e.g. \"1\" or \"12\")."
                                                .to_string(),
                                        ),
                                        doc_url: None,
                                        additional_context: Some(format!(
                                            "Failed to parse card expiry month as i64; \
                                             received value: {:?}.",
                                            req_card.card_exp_month.peek()
                                        )),
                                    },
                                })?,
                        ),
                        expiry_year: Secret::new(
                            req_card
                                .get_expiry_year_4_digit()
                                .peek()
                                .parse::<i64>()
                                .change_context(IntegrationError::InvalidDataFormat {
                                    field_name: "card_exp_year",
                                    context: IntegrationErrorContext {
                                        suggested_action: Some(
                                            "Pass card_exp_year as a 4-digit numeric string \
                                             (e.g. \"2030\")."
                                                .to_string(),
                                        ),
                                        doc_url: None,
                                        additional_context: Some(format!(
                                            "Failed to parse card expiry year as i64; \
                                             received value: {:?}.",
                                            req_card.get_expiry_year_4_digit().peek()
                                        )),
                                    },
                                })?,
                        ),
                        card_security_code: req_card.card_cvc.clone(),
                    },
                    store_payment_method: if item
                        .router_data
                        .request
                        .is_customer_initiated_mandate_payment()
                    {
                        StorePaymentMethod::CardholderInitiated
                    } else {
                        StorePaymentMethod::DoNotStore
                    },
                });
                let authentication_data = item.router_data.request.authentication_data.as_ref();
                let has_external_authentication = authentication_data
                    .is_some_and(AuthenticationData::has_external_authentication);
                let is_three_ds = item.router_data.resource_common_data.is_three_ds()
                    || has_external_authentication;
                let is_recurring = item.router_data.request.is_mandate_payment();
                let is_authenticated = authentication_data.is_some_and(|auth| {
                    auth.trans_status.as_ref().map_or_else(
                        || auth.cavv.is_some(),
                        |status| *status == common_enums::TransactionStatus::Success,
                    )
                });
                let amount = item.router_data.request.money.clone();
                let ecommerce_indicator = Some(derive_ecommerce_indicator(
                    item.router_data.request.payment_channel.as_ref(),
                    item.router_data.request.mit_category.as_ref(),
                    is_three_ds,
                    is_authenticated,
                    is_recurring,
                ));
                let three_d_secure_data = build_three_d_secure_data(authentication_data)?;

                Ok(Self {
                    idempotency_key,
                    amount,
                    payment_method,
                    automatic_capture,
                    incremental_authorization,
                    ecommerce_indicator,
                    three_d_secure_data,
                })
            }
            PaymentMethodData::Wallet(ref wallet_data) => {
                let idempotency_key = format!(
                    "auth_{}",
                    item.router_data
                        .resource_common_data
                        .connector_request_reference_id
                );
                let wallet_indicator = if item.router_data.request.browser_info.is_some() {
                    WalletIndicator::InBrowser
                } else {
                    WalletIndicator::InApplication
                };
                let is_cit_mandate = item
                    .router_data
                    .request
                    .is_customer_initiated_mandate_payment();
                let is_recurring = item.router_data.request.is_mandate_payment();
                // ecommerce_indicator describes the transaction channel (MOTO, 3DS, SSL) —
                // independent of credential storage, so it is derived for every wallet payment.
                // Wallets carry their own authentication via wallet_ecommerce_indicator (Apple Pay
                // ECI / Google Pay cryptogram), so is_three_ds and has_authentication_cavv are
                // always false here — the top-level indicator reflects channel only.
                let ecommerce_indicator = Some(derive_ecommerce_indicator(
                    item.router_data.request.payment_channel.as_ref(),
                    item.router_data.request.mit_category.as_ref(),
                    false,
                    false,
                    is_recurring,
                ));
                let (store_payment_method, credential_on_file_information) = if is_cit_mandate {
                    (
                        Some(StorePaymentMethod::CardholderInitiated),
                        Some(CredentialOnFileInformation {
                            payment_indicator: PaymentIndicator::CustomerInitiated,
                            payment_information: PaymentInformation::First,
                            issuer_id: None,
                        }),
                    )
                } else {
                    (None, None)
                };

                let payment_method = match wallet_data {
                    WalletData::ApplePay(apple_pay_data) => match &apple_pay_data.payment_data {
                        ApplePayPaymentData::Decrypted(decrypt_data) => {
                            let card_brand = MonerisCardBrand::try_from(
                                apple_pay_data.payment_method.network.as_str(),
                            )?;
                            let expiry_month = decrypt_data
                                .application_expiration_month
                                .peek()
                                .parse::<i64>()
                                .change_context(IntegrationError::InvalidDataFormat {
                                    field_name: "apple_pay_expiry_month",
                                    context: IntegrationErrorContext {
                                        suggested_action: Some(
                                            "Pass expiry month as a 1- or 2-digit numeric \
                                                 string."
                                                .to_string(),
                                        ),
                                        doc_url: None,
                                        additional_context: None,
                                    },
                                })?;
                            let expiry_year = decrypt_data
                                .get_four_digit_expiry_year()
                                .peek()
                                .parse::<i64>()
                                .change_context(IntegrationError::InvalidDataFormat {
                                    field_name: "apple_pay_expiry_year",
                                    context: IntegrationErrorContext {
                                        suggested_action: Some(
                                            "Pass expiry year as a 4-digit numeric string."
                                                .to_string(),
                                        ),
                                        doc_url: None,
                                        additional_context: None,
                                    },
                                })?;
                            PaymentMethod::ApplePayDecrypted(PaymentMethodApplePayDecrypted {
                                payment_method_source: PaymentMethodSource::ApplePayDecrypted,
                                application_primary_account_number: Secret::new(
                                    decrypt_data
                                        .application_primary_account_number
                                        .get_card_no(),
                                ),
                                expiry_month: Secret::new(expiry_month),
                                expiry_year: Secret::new(expiry_year),
                                data_type: match card_brand {
                                    MonerisCardBrand::Interac => ApplePayDataType::Emv,
                                    _ => ApplePayDataType::ThreeDSecure,
                                },
                                cryptogram: decrypt_data
                                    .payment_data
                                    .online_payment_cryptogram
                                    .clone(),
                                card_brand,
                                wallet_ecommerce_indicator: decrypt_data
                                    .payment_data
                                    .eci_indicator
                                    .clone(),
                                wallet_indicator,
                                device_manufacturer_identifier: None,
                                store_payment_method,
                                credential_on_file_information,
                            })
                        }
                        ApplePayPaymentData::Encrypted(_) => {
                            return Err(IntegrationError::NotImplemented(
                                "Apple Pay encrypted payment_data is not yet implemented for \
                                 Moneris"
                                    .to_string(),
                                IntegrationErrorContext {
                                    suggested_action: None,
                                    doc_url: None,
                                    additional_context: Some(
                                        "Moneris supports Apple Pay encrypted tokens, but this \
                                         integration currently only handles \
                                         ApplePayPaymentData::Decrypted. Support for \
                                         ApplePayPaymentData::Encrypted is pending implementation."
                                            .to_string(),
                                    ),
                                },
                            )
                            .into());
                        }
                    },
                    WalletData::GooglePay(gpay_data) => {
                        let card_brand =
                            MonerisCardBrand::try_from(gpay_data.info.card_network.as_str())?;
                        match &gpay_data.tokenization_data {
                            GpayTokenizationData::Decrypted(decrypt_data) => {
                                let expiry_month = decrypt_data
                                    .card_exp_month
                                    .peek()
                                    .parse::<i64>()
                                    .change_context(IntegrationError::InvalidDataFormat {
                                        field_name: "google_pay_expiry_month",
                                        context: IntegrationErrorContext {
                                            suggested_action: Some(
                                                "Pass expiry month as a 1- or 2-digit numeric \
                                                 string."
                                                    .to_string(),
                                            ),
                                            doc_url: None,
                                            additional_context: None,
                                        },
                                    })?;
                                let expiry_year = decrypt_data
                                    .get_four_digit_expiry_year()
                                    .change_context(IntegrationError::InvalidDataFormat {
                                        field_name: "google_pay_expiry_year",
                                        context: IntegrationErrorContext {
                                            suggested_action: Some(
                                                "Pass expiry year as a 4-digit numeric string."
                                                    .to_string(),
                                            ),
                                            doc_url: None,
                                            additional_context: None,
                                        },
                                    })?
                                    .peek()
                                    .parse::<i64>()
                                    .change_context(IntegrationError::InvalidDataFormat {
                                        field_name: "google_pay_expiry_year",
                                        context: IntegrationErrorContext {
                                            suggested_action: Some(
                                                "Pass expiry year as a 4-digit numeric string."
                                                    .to_string(),
                                            ),
                                            doc_url: None,
                                            additional_context: None,
                                        },
                                    })?;
                                PaymentMethod::GooglePayDecrypted(PaymentMethodGooglePayDecrypted {
                                    payment_method_source: PaymentMethodSource::GooglePayDecrypted,
                                    wallet_source: if decrypt_data.cryptogram.is_some() {
                                        GooglePayWalletSource::TokenizedCard
                                    } else {
                                        GooglePayWalletSource::Card
                                    },
                                    card_brand,
                                    wallet_indicator,
                                    card_details: GooglePayDecryptedCardDetails {
                                        personal_account_number: Secret::new(
                                            decrypt_data
                                                .application_primary_account_number
                                                .get_card_no(),
                                        ),
                                        expiry_month: Secret::new(expiry_month),
                                        expiry_year: Secret::new(expiry_year),
                                        authentication_method: None,
                                        cryptogram: decrypt_data.cryptogram.clone(),
                                        wallet_ecommerce_indicator: decrypt_data
                                            .eci_indicator
                                            .clone(),
                                    },
                                    store_payment_method,
                                    credential_on_file_information,
                                })
                            }
                            GpayTokenizationData::Encrypted(_) => {
                                return Err(IntegrationError::NotImplemented(
                                    "Google Pay encrypted tokenization_data is not yet \
                                     implemented for Moneris"
                                        .to_string(),
                                    IntegrationErrorContext {
                                        suggested_action: None,
                                        doc_url: None,
                                        additional_context: Some(
                                            "Moneris supports Google Pay encrypted tokens, but \
                                             this integration currently only handles \
                                             GpayTokenizationData::Decrypted. Support for \
                                             GpayTokenizationData::Encrypted is pending \
                                             implementation."
                                                .to_string(),
                                        ),
                                    },
                                )
                                .into());
                            }
                        }
                    }
                    _ => {
                        return Err(IntegrationError::NotImplemented(
                            utils::get_unimplemented_payment_method_error_message("Moneris"),
                            IntegrationErrorContext {
                                suggested_action: Some(
                                    "Use Apple Pay or Google Pay wallet variants for Moneris \
                                     wallet payments."
                                        .to_string(),
                                ),
                                doc_url: None,
                                additional_context: Some(format!(
                                    "Moneris wallet authorize received an unsupported wallet \
                                     variant: {:?}.",
                                    wallet_data
                                )),
                            },
                        )
                        .into())
                    }
                };

                let amount = item.router_data.request.money.clone();

                Ok(Self {
                    idempotency_key,
                    amount,
                    payment_method,
                    automatic_capture,
                    incremental_authorization,
                    ecommerce_indicator,
                    three_d_secure_data: None,
                })
            }
            _ => Err(IntegrationError::NotImplemented(
                "Only card and wallet payments are currently implemented for Moneris".to_string(),
                IntegrationErrorContext {
                    suggested_action: Some(
                        "Use a card or wallet payment method for Moneris authorize.".to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(format!(
                        "Moneris authorize received a non-card, non-wallet payment method: {:?}.",
                        item.router_data.request.payment_method_data
                    )),
                },
            )
            .into()),
        }
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MonerisAuthorizeResponse {
    payment_status: MonerisPaymentStatus,
    payment_id: String,
    payment_method: MonerisPaymentMethodData,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<MonerisAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<MonerisAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let mandate_reference = if item
            .router_data
            .request
            .is_customer_initiated_mandate_payment()
        {
            Some(Box::new(MandateReference {
                connector_mandate_id: Some(
                    item.response
                        .payment_method
                        .payment_method_id
                        .peek()
                        .to_string(),
                ),
                payment_method_id: None,
                mandate_metadata: None,
                connector_mandate_request_reference_id: None,
            }))
        } else {
            None
        };
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: common_enums::AttemptStatus::from(item.response.payment_status),
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.payment_id.clone()),
                redirection_data: None,
                mandate_reference,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(item.response.payment_id),
                incremental_authorization_allowed: item
                    .router_data
                    .request
                    .request_incremental_authorization,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            }),
            ..item.router_data
        })
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MonerisPaymentsResponse {
    payment_status: MonerisPaymentStatus,
    payment_id: String,
    payment_method: MonerisPaymentMethodData,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MonerisPaymentStatus {
    Succeeded,
    #[default]
    Processing,
    Canceled,
    Declined,
    DeclinedRetry,
    Authorized,
    Incremented,
}

impl From<MonerisPaymentStatus> for common_enums::AttemptStatus {
    fn from(item: MonerisPaymentStatus) -> Self {
        match item {
            MonerisPaymentStatus::Succeeded => Self::Charged,
            MonerisPaymentStatus::Authorized | MonerisPaymentStatus::Incremented => {
                Self::Authorized
            }
            MonerisPaymentStatus::Canceled => Self::Voided,
            MonerisPaymentStatus::Declined | MonerisPaymentStatus::DeclinedRetry => Self::Failure,
            MonerisPaymentStatus::Processing => Self::Pending,
        }
    }
}

impl From<MonerisPaymentStatus> for common_enums::AuthorizationStatus {
    fn from(item: MonerisPaymentStatus) -> Self {
        match item {
            MonerisPaymentStatus::Incremented
            | MonerisPaymentStatus::Authorized
            | MonerisPaymentStatus::Succeeded => Self::Success,
            MonerisPaymentStatus::Processing => Self::Processing,
            MonerisPaymentStatus::Canceled
            | MonerisPaymentStatus::Declined
            | MonerisPaymentStatus::DeclinedRetry => Self::Failure,
        }
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MonerisPaymentMethodData {
    payment_method_id: Secret<String>,
}

impl<F, T> TryFrom<ResponseRouterData<MonerisPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<MonerisPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: common_enums::AttemptStatus::from(item.response.payment_status),
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.payment_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(item.response.payment_id),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            }),
            ..item.router_data
        })
    }
}

// Moneris does not return a distinct status for partial capture — a successful completion
// always comes back as SUCCEEDED regardless of whether the captured amount is less than
// the authorized amount. This dedicated response type carries out the amount comparison
// that the generic MonerisPaymentsResponse impl cannot perform.
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MonerisCaptureResponse {
    payment_status: MonerisPaymentStatus,
    payment_id: String,
    payment_method: MonerisPaymentMethodData,
}

impl TryFrom<ResponseRouterData<MonerisCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<MonerisCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = match item.response.payment_status {
            MonerisPaymentStatus::Succeeded => {
                let captured = common_utils::types::ConnectorMinorUnitForConnector
                    .convert(&item.router_data.request.capture_money);
                match item.router_data.resource_common_data.capturable_money {
                    Some(ref authorized)
                        if captured
                            < common_utils::types::ConnectorMinorUnitForConnector
                                .convert(authorized) =>
                    {
                        common_enums::AttemptStatus::PartialCharged
                    }
                    _ => common_enums::AttemptStatus::Charged,
                }
            }
            other => common_enums::AttemptStatus::from(other),
        };
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.payment_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(item.response.payment_id),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            }),
            ..item.router_data
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonerisIncrementalAuthorizationRequest {
    idempotency_key: String,
    amount: Money,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MonerisRouterData<
            RouterDataV2<
                IncrementalAuthorization,
                PaymentFlowData,
                PaymentsIncrementalAuthorizationData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for MonerisIncrementalAuthorizationRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: MonerisRouterData<
            RouterDataV2<
                IncrementalAuthorization,
                PaymentFlowData,
                PaymentsIncrementalAuthorizationData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request = &item.router_data.request;
        let idempotency_key = format!(
            "increment_{}",
            item.router_data
                .resource_common_data
                .connector_request_reference_id
        );
        let amount = request
            .additional_money
            .as_ref()
            .ok_or_else(|| {
                IntegrationError::MissingRequiredField {
                    field_name: "additional_amount",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Provide the increase over the current authorized amount in additional_amount."
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "Moneris Increment Payment accepts the amount to add; the total authorization amount alone cannot be converted to an increment without the original authorization amount."
                                .to_string(),
                        ),
                    },
                }
            })?
            .clone();

        Ok(Self {
            idempotency_key,
            amount,
        })
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MonerisIncrementalAuthorizationResponse {
    payment_status: MonerisPaymentStatus,
    payment_id: String,
    payment_method: MonerisPaymentMethodData,
}

impl TryFrom<ResponseRouterData<MonerisIncrementalAuthorizationResponse, Self>>
    for RouterDataV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    >
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<MonerisIncrementalAuthorizationResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let authorization_status =
            common_enums::AuthorizationStatus::from(item.response.payment_status);

        Ok(Self {
            response: Ok(PaymentsResponseData::IncrementalAuthorizationResponse {
                status: authorization_status,
                connector_authorization_id: Some(item.response.payment_id),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MonerisRepeatPaymentRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    idempotency_key: String,
    amount: Money,
    payment_method: PaymentMethod<T>,
    automatic_capture: bool,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MonerisRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for MonerisRepeatPaymentRequest<T>
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: MonerisRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        match item.router_data.request.payment_method_data.clone() {
            PaymentMethodData::MandatePayment => {
                let idempotency_key = format!(
                    "repeat_{}",
                    item.router_data
                        .resource_common_data
                        .connector_request_reference_id
                );
                let automatic_capture = item.router_data.request.is_auto_capture();
                let payment_method = PaymentMethod::PaymentMethodId(PaymentMethodId {
                    payment_method_source: PaymentMethodSource::PaymentMethodId,
                    payment_method_id: item
                        .router_data
                        .request
                        .connector_mandate_id()
                        .ok_or(IntegrationError::MissingRequiredField {
                            field_name: "connector_mandate_id",
                            context: IntegrationErrorContext {
                                suggested_action: Some(
                                    "Provide `connector_mandate_id` in the RepeatPayment \
                                     request — this is the mandate reference returned by \
                                     Moneris during the initial customer-initiated mandate \
                                     setup (Authorize with `setup_future_usage`)."
                                        .to_string(),
                                ),
                                doc_url: None,
                                additional_context: Some(
                                    "RepeatPayment calls Moneris's stored-credential \
                                     endpoint which requires a `payment_method_id` derived \
                                     from `connector_mandate_id`, but the field was None."
                                        .to_string(),
                                ),
                            },
                        })?
                        .into(),
                    credential_on_file_information: Some(CredentialOnFileInformation {
                        payment_indicator: if item.router_data.request.off_session == Some(false) {
                            PaymentIndicator::CustomerInitiated
                        } else {
                            PaymentIndicator::MerchantInitiated
                        },
                        payment_information: PaymentInformation::Subsequent,
                        issuer_id: None,
                    }),
                });
                let amount = item.router_data.request.money.clone();
                Ok(Self {
                    idempotency_key,
                    amount,
                    payment_method,
                    automatic_capture,
                })
            }
            _ => Err(IntegrationError::NotImplemented(
                "Only mandate (stored-credential) payments are currently implemented for \
                 Moneris repeat_payment"
                    .to_string(),
                IntegrationErrorContext {
                    suggested_action: Some(
                        "Use `payment_method_data = MandatePayment` together with a valid \
                         `connector_mandate_id`; Moneris repeat_payment is a merchant-initiated \
                         transaction that charges a previously stored credential."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(format!(
                        "Moneris repeat_payment received a non-mandate payment method \
                         variant: {:?}.",
                        item.router_data.request.payment_method_data
                    )),
                },
            )
            .into()),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonerisPaymentsCaptureRequest {
    amount: Money,
    idempotency_key: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MonerisRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for MonerisPaymentsCaptureRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: MonerisRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount = item.router_data.request.capture_money.clone();
        let idempotency_key = format!(
            "capture_{}",
            item.router_data
                .resource_common_data
                .connector_request_reference_id
        );
        Ok(Self {
            amount,
            idempotency_key,
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonerisCancelRequest {
    idempotency_key: String,
    reason: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MonerisRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for MonerisCancelRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: MonerisRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let idempotency_key = format!(
            "void_{}",
            item.router_data
                .resource_common_data
                .connector_request_reference_id
        );
        let reason = item.router_data.request.cancellation_reason.clone();
        Ok(Self {
            idempotency_key,
            reason,
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonerisRefundRequest {
    pub refund_amount: Money,
    pub idempotency_key: String,
    pub reason: Option<String>,
    pub payment_id: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MonerisRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for MonerisRefundRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: MonerisRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let refund_amount = item.router_data.request.refund_money.clone();
        let idempotency_key = format!(
            "refund_{}",
            item.router_data
                .resource_common_data
                .connector_request_reference_id
        );
        let reason = item.router_data.request.reason.clone();
        let payment_id = item.router_data.request.connector_transaction_id.clone();
        Ok(Self {
            refund_amount,
            idempotency_key,
            reason,
            payment_id,
        })
    }
}

#[derive(Debug, Serialize, Default, Deserialize, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MonerisRefundStatus {
    Succeeded,
    #[default]
    Processing,
    Declined,
    DeclinedRetry,
}

impl From<MonerisRefundStatus> for RefundStatus {
    fn from(item: MonerisRefundStatus) -> Self {
        match item {
            MonerisRefundStatus::Succeeded => Self::Success,
            MonerisRefundStatus::Declined | MonerisRefundStatus::DeclinedRetry => Self::Failure,
            MonerisRefundStatus::Processing => Self::Pending,
        }
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonerisRefundResponse {
    refund_id: String,
    refund_status: MonerisRefundStatus,
}

impl TryFrom<ResponseRouterData<MonerisRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<MonerisRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.refund_id.to_string(),
                refund_status: RefundStatus::from(item.response.refund_status),
                acquirer_reference_number: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

impl TryFrom<ResponseRouterData<MonerisRefundResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<MonerisRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.refund_id.to_string(),
                refund_status: RefundStatus::from(item.response.refund_status),
                acquirer_reference_number: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MonerisErrorResponse {
    pub status: u16,
    pub category: String,
    pub title: String,
    pub errors: Option<Vec<MonerisError>>,
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MonerisError {
    pub reason_code: String,
    pub parameter_name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MonerisAuthErrorResponse {
    pub error: String,
    pub error_description: Option<String>,
}

// ===== 3DS FLOW TYPES =====

pub fn build_three_d_secure_data(
    authentication_data: Option<&AuthenticationData>,
) -> Result<Option<MonerisThreeDSecureData>, Report<IntegrationError>> {
    let Some(auth) = authentication_data else {
        return Ok(None);
    };

    // Moneris Create Authentication returns its own resource ID in transaction_id, along with
    // ECI and cryptogram fields. Only pass transaction_id as a Moneris authentication ID when it
    // matches Moneris' resource ID format; otherwise it may be a HyperSwitch transaction ID.
    if let Some(authentication_id) = auth
        .transaction_id
        .as_deref()
        .filter(|id| is_moneris_resource_id(id))
    {
        return Ok(Some(MonerisThreeDSecureData::AuthenticationId(
            MonerisThreeDSecureAuthenticationId {
                three_d_secure_authentication_id: authentication_id.to_string(),
            },
        )));
    }

    if !auth.has_external_authentication() {
        return Ok(None);
    }
    let directory_server_transaction_id = auth.ds_trans_id.clone().ok_or(
        IntegrationError::MissingRequiredField {
            field_name: "authentication_data.ds_trans_id",
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Pass the Directory Server Transaction ID returned by the external 3DS service."
                        .to_string(),
                ),
                doc_url: None,
                additional_context: Some(
                    "Moneris requires threeDSecureDirectoryServerTransactionId when Create Payment receives external 3DS authentication data."
                        .to_string(),
                ),
            },
        },
    )?;
    let server_transaction_id = auth
        .threeds_server_transaction_id
        .as_ref()
        .filter(|server_id| auth.ds_trans_id.as_ref() != Some(*server_id))
        .cloned();

    Ok(Some(MonerisThreeDSecureData::Cryptogram(
        MonerisThreeDSecureCryptogramData {
            three_d_secure_authentication_value: auth.cavv.clone(),
            three_d_secure_version: auth.message_version.as_ref().map(ToString::to_string),
            three_d_secure_server_transaction_id: server_transaction_id,
            three_d_secure_directory_server_transaction_id: directory_server_transaction_id,
        },
    )))
}

fn is_moneris_resource_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 30
        && bytes
            .get(..2)
            .is_some_and(|prefix| prefix.iter().all(u8::is_ascii_alphabetic))
        && bytes
            .get(2..4)
            .is_some_and(|digits| digits.iter().all(u8::is_ascii_digit))
        && bytes
            .get(4..)
            .is_some_and(|suffix| suffix.iter().all(u8::is_ascii_alphanumeric))
}

// ---- 3DS Enums ----

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ThreeDSecureMessageCategory {
    Payment,
    NonPayment,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ThreeDSecureDeviceChannel {
    Browser,
    ThreeDSecureRequestorInitiated,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ThreeDSecureRequestType {
    Cardholder,
    Recurring,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ThreeDSecureCompletionIndicator {
    Success,
    Failure,
    Unavailable,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ThreeDSecureChallengeRequested {
    NoPreference,
    NoChallengeRequested,
    ChallengeRequestedMandate,
    ChallengeRequestedPreference,
    NoChallengeRequestedRiskAnalysisPerformed,
    NoChallengeRequestedDataOnly,
    NoChallengeRequestedAuthenticationPerformed,
    NoChallengeRequestedWhitelistExemption,
    ChallengeRequestedWhitelistPrompt,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ThreeDSecureChallengeWindowSize {
    XSmall,
    Small,
    Medium,
    Large,
    FullScreen,
}

// ---- 3DS Address ----

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MonerisThreeDSAddress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit_number: Option<Secret<String>>,
    pub street_number: Secret<String>,
    pub street_name: Secret<String>,
    pub city: Secret<String>,
    pub province: Secret<String>,
    pub postal_code: Secret<String>,
    pub country: common_enums::CountryAlpha2,
}

// ---- 3DS Transaction Status ----

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MonerisThreeDSecureTransactionStatus {
    Authenticated,
    #[serde(alias = "AUTHENTICATED_ATTEMPTED")]
    AuthenticationAttempted,
    ChallengeAuthenticationRequired,
    #[serde(alias = "DECOUPLED")]
    ChallengeDecoupledAuthenticationConfirmed,
    TechnicalIssue,
    NotAuthenticated,
    Rejected,
    #[serde(alias = "INFORMATION_ONLY")]
    ChallengePreferenceAcknowledged,
}

impl MonerisThreeDSecureTransactionStatus {
    fn to_transaction_status(&self) -> common_enums::TransactionStatus {
        match self {
            Self::Authenticated => common_enums::TransactionStatus::Success,
            Self::AuthenticationAttempted => common_enums::TransactionStatus::NotVerified,
            Self::ChallengeAuthenticationRequired => {
                common_enums::TransactionStatus::ChallengeRequired
            }
            Self::ChallengeDecoupledAuthenticationConfirmed => {
                common_enums::TransactionStatus::ChallengeRequiredDecoupledAuthentication
            }
            Self::TechnicalIssue => common_enums::TransactionStatus::VerificationNotPerformed,
            Self::NotAuthenticated => common_enums::TransactionStatus::Failure,
            Self::Rejected => common_enums::TransactionStatus::Rejected,
            Self::ChallengePreferenceAcknowledged => {
                common_enums::TransactionStatus::InformationOnly
            }
        }
    }

    fn to_attempt_status(&self) -> common_enums::AttemptStatus {
        match self {
            Self::Authenticated | Self::AuthenticationAttempted => {
                common_enums::AttemptStatus::AuthenticationSuccessful
            }
            // EMVCo status I is informational, not a declined authentication. The transaction
            // status remains InformationOnly (no liability shift), while authorization proceeds.
            Self::ChallengePreferenceAcknowledged => {
                common_enums::AttemptStatus::AuthenticationSuccessful
            }
            Self::ChallengeAuthenticationRequired => {
                common_enums::AttemptStatus::AuthenticationPending
            }
            // These outcomes do not provide a browser challenge redirect that Prism can
            // complete and are not eligible to proceed to payment.
            Self::ChallengeDecoupledAuthenticationConfirmed
            | Self::TechnicalIssue
            | Self::NotAuthenticated
            | Self::Rejected => common_enums::AttemptStatus::AuthenticationFailed,
        }
    }
}

struct MonerisAuthenticationDataFields<'a> {
    transaction_status: &'a MonerisThreeDSecureTransactionStatus,
    ecommerce_indicator: &'a str,
    authentication_value: Option<&'a Secret<String>>,
    server_transaction_id: Option<&'a String>,
    version: Option<&'a str>,
    directory_server_transaction_id: Option<&'a String>,
    access_control_server_transaction_id: Option<&'a String>,
    authentication_id: &'a str,
}

fn build_moneris_authentication_data(
    fields: MonerisAuthenticationDataFields<'_>,
) -> AuthenticationData {
    AuthenticationData {
        trans_status: Some(fields.transaction_status.to_transaction_status()),
        eci: Some(fields.ecommerce_indicator.to_string()),
        cavv: fields.authentication_value.cloned(),
        threeds_server_transaction_id: fields.server_transaction_id.cloned(),
        message_version: fields.version.and_then(|version| version.parse().ok()),
        ds_trans_id: fields.directory_server_transaction_id.cloned(),
        acs_transaction_id: fields.access_control_server_transaction_id.cloned(),
        transaction_id: Some(fields.authentication_id.to_string()),
        ucaf_collection_indicator: None,
        network_params: None,
        exemption_indicator: None,
        created_at: None,
        challenge_code: None,
        challenge_cancel: None,
        challenge_code_reason: None,
        message_extension: None,
        authentication_type: None,
    }
}

// ---- 3DS Payment Method (no store_payment_method for auth requests) ----

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Moneris3DSCard<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    card_number: RawCardNumber<T>,
    expiry_month: Secret<i64>,
    expiry_year: Secret<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    card_security_code: Option<Secret<String>>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Moneris3DSPaymentMethodCard<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    payment_method_source: PaymentMethodSource,
    card: Moneris3DSCard<T>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(untagged)]
pub enum Moneris3DSPaymentMethod<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    Card(Moneris3DSPaymentMethodCard<T>),
}

// ---- PreAuthenticate Request ----

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonerisPreAuthenticateRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub idempotency_key: String,
    pub amount: Money,
    pub cardholder_name: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cardholder_email: Option<Email>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cardholder_phone_number: Option<Secret<String>>,
    pub billing_address: MonerisThreeDSAddress,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipping_address: Option<MonerisThreeDSAddress>,
    pub payment_method: Moneris3DSPaymentMethod<T>,
    pub three_d_secure_message_category: ThreeDSecureMessageCategory,
    pub three_d_secure_device_channel: ThreeDSecureDeviceChannel,
    pub three_d_secure_request_type: ThreeDSecureRequestType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_d_secure_notification_url: Option<String>,
    pub three_d_secure_completion_indicator: ThreeDSecureCompletionIndicator,
    pub three_d_secure_challenge_requested: ThreeDSecureChallengeRequested,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_d_secure_challenge_window_size: Option<ThreeDSecureChallengeWindowSize>,
    pub browser_ip_address: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_user_agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_java_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_javascript_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_screen_height: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_screen_width: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_language: Option<String>,
}

fn missing_moneris_three_ds_field(
    field_name: &'static str,
    suggested_action: &'static str,
) -> IntegrationError {
    IntegrationError::MissingRequiredField {
        field_name,
        context: utils::integration_ctx(
            format!("Moneris 3DS authentication requires {field_name}."),
            suggested_action,
        ),
    }
}

fn invalid_moneris_street_address(
    field_name: &'static str,
    address_type: &'static str,
) -> IntegrationError {
    IntegrationError::InvalidDataFormat {
        field_name,
        context: utils::integration_ctx(
            "Moneris Create Authentication requires a street number and street name.",
            format!(
                "Provide {address_type} address line1 with a street number followed by a street name."
            ),
        ),
    }
}

fn split_moneris_street_address(
    street_line: Secret<String>,
    field_name: &'static str,
    address_type: &'static str,
) -> Result<(Secret<String>, Secret<String>), Report<IntegrationError>> {
    let street_line = street_line.peek().trim();
    if street_line.is_empty() {
        return Err(invalid_moneris_street_address(field_name, address_type).into());
    }

    let street_parts = street_line
        .split_once(char::is_whitespace)
        .map(|(street_number, street_name)| (street_number.trim(), street_name.trim()))
        .filter(|(street_number, street_name)| {
            let street_number = street_number.trim_end_matches(',');
            !street_number.is_empty()
                && street_number
                    .chars()
                    .any(|character| character.is_ascii_digit())
                && !street_name.is_empty()
        });

    let (street_number, street_name) =
        street_parts.ok_or_else(|| invalid_moneris_street_address(field_name, address_type))?;

    Ok((
        Secret::new(street_number.trim_end_matches(',').to_string()),
        Secret::new(street_name.to_string()),
    ))
}

fn build_moneris_billing_address(
    resource: &PaymentFlowData,
) -> Result<MonerisThreeDSAddress, Report<IntegrationError>> {
    let street_line = resource.get_optional_billing_line1().ok_or_else(|| {
        missing_moneris_three_ds_field(
            "billing.address.line1",
            "Provide a billing street number and street name for Moneris 3DS authentication.",
        )
    })?;
    let (street_number, street_name) =
        split_moneris_street_address(street_line, "billing.address.line1", "billing")?;
    let city = resource.get_optional_billing_city().ok_or_else(|| {
        missing_moneris_three_ds_field(
            "billing.address.city",
            "Provide the billing city for Moneris 3DS authentication.",
        )
    })?;
    let province = resource
        .get_optional_billing_state_code_iso()
        .ok_or_else(|| {
            missing_moneris_three_ds_field(
                "billing.address.state",
                "Provide an ISO subdivision code for the billing province/state.",
            )
        })?;
    let postal_code = resource.get_optional_billing_zip().ok_or_else(|| {
        missing_moneris_three_ds_field(
            "billing.address.postal_code",
            "Provide the billing postal code for Moneris 3DS authentication.",
        )
    })?;
    let country = resource.get_optional_billing_country().ok_or_else(|| {
        missing_moneris_three_ds_field(
            "billing.address.country",
            "Provide the billing country for Moneris 3DS authentication.",
        )
    })?;

    Ok(MonerisThreeDSAddress {
        unit_number: resource.get_optional_billing_line2(),
        street_number,
        street_name,
        city,
        province,
        postal_code,
        country,
    })
}

fn build_moneris_shipping_address(
    resource: &PaymentFlowData,
) -> Result<Option<MonerisThreeDSAddress>, Report<IntegrationError>> {
    let has_shipping_address = resource
        .get_optional_shipping()
        .is_some_and(|shipping| shipping.address.is_some());
    if !has_shipping_address {
        return Ok(None);
    }

    let street_line = resource.get_optional_shipping_line1().ok_or_else(|| {
        missing_moneris_three_ds_field(
            "shipping.address.line1",
            "Provide a shipping street number and street name for Moneris 3DS authentication.",
        )
    })?;
    let (street_number, street_name) =
        split_moneris_street_address(street_line, "shipping.address.line1", "shipping")?;
    let city = resource.get_optional_shipping_city().ok_or_else(|| {
        missing_moneris_three_ds_field(
            "shipping.address.city",
            "Provide the shipping city for Moneris 3DS authentication.",
        )
    })?;
    let province = resource
        .get_optional_shipping()
        .and_then(|shipping| shipping.address.as_ref())
        .and_then(|address| address.get_optional_state_code_iso())
        .ok_or_else(|| {
            missing_moneris_three_ds_field(
                "shipping.address.state",
                "Provide an ISO subdivision code for the shipping province/state.",
            )
        })?;
    let postal_code = resource.get_optional_shipping_zip().ok_or_else(|| {
        missing_moneris_three_ds_field(
            "shipping.address.postal_code",
            "Provide the shipping postal code for Moneris 3DS authentication.",
        )
    })?;
    let country = resource.get_optional_shipping_country().ok_or_else(|| {
        missing_moneris_three_ds_field(
            "shipping.address.country",
            "Provide the shipping country for Moneris 3DS authentication.",
        )
    })?;

    Ok(Some(MonerisThreeDSAddress {
        unit_number: resource.get_optional_shipping_line2(),
        street_number,
        street_name,
        city,
        province,
        postal_code,
        country,
    }))
}

fn required_browser_dimension(
    value: Option<u32>,
    field_name: &'static str,
) -> Result<i32, Report<IntegrationError>> {
    let value = value.ok_or_else(|| {
        missing_moneris_three_ds_field(
            field_name,
            "Provide browser screen dimensions for Moneris browser-channel 3DS.",
        )
    })?;
    i32::try_from(value).change_context(IntegrationError::InvalidDataFormat {
        field_name,
        context: utils::integration_ctx(
            "Moneris represents browser screen dimensions as signed 32-bit integers.",
            "Provide browser screen dimensions within Moneris's supported integer range.",
        ),
    })
}

fn validate_moneris_phone_number(
    phone_number: Secret<String>,
) -> Result<Secret<String>, Report<IntegrationError>> {
    let is_valid_e164 = phone_number.peek().strip_prefix('+').is_some_and(|digits| {
        (2..=15).contains(&digits.len())
            && !digits.starts_with('0')
            && digits.bytes().all(|digit| digit.is_ascii_digit())
    });

    is_valid_e164.then_some(phone_number).ok_or_else(|| {
        IntegrationError::InvalidDataFormat {
            field_name: "billing.phone",
            context: utils::integration_ctx(
                "Moneris requires cardholderPhoneNumber as a plus sign followed by 2 to 15 digits.",
                "Provide the cardholder phone number in E.164 format with a leading plus sign and country code.",
            ),
        }
        .into()
    })
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MonerisRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for MonerisPreAuthenticateRequest<T>
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: MonerisRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let req = &item.router_data.request;
        let resource = &item.router_data.resource_common_data;

        let payment_method_data =
            req.payment_method_data
                .clone()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "payment_method_data",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Provide card data in the PreAuthenticate request.".to_string(),
                        ),
                        doc_url: None,
                        additional_context: None,
                    },
                })?;

        let payment_method = match payment_method_data {
            PaymentMethodData::Card(ref req_card) => {
                let expiry_month = Secret::new(
                    req_card
                        .card_exp_month
                        .peek()
                        .parse::<i64>()
                        .change_context(IntegrationError::InvalidDataFormat {
                            field_name: "card_exp_month",
                            context: IntegrationErrorContext {
                                suggested_action: Some(
                                    "Pass card_exp_month as a 1- or 2-digit numeric string."
                                        .to_string(),
                                ),
                                doc_url: None,
                                additional_context: Some(format!(
                                    "Failed to parse card expiry month as i64; received: {:?}.",
                                    req_card.card_exp_month.peek()
                                )),
                            },
                        })?,
                );
                let expiry_year = Secret::new(
                    req_card
                        .get_expiry_year_4_digit()
                        .peek()
                        .parse::<i64>()
                        .change_context(IntegrationError::InvalidDataFormat {
                            field_name: "card_exp_year",
                            context: IntegrationErrorContext {
                                suggested_action: Some(
                                    "Pass card_exp_year as a 4-digit numeric string.".to_string(),
                                ),
                                doc_url: None,
                                additional_context: Some(format!(
                                    "Failed to parse card expiry year as i64; received: {:?}.",
                                    req_card.get_expiry_year_4_digit().peek()
                                )),
                            },
                        })?,
                );
                Moneris3DSPaymentMethod::Card(Moneris3DSPaymentMethodCard {
                    payment_method_source: PaymentMethodSource::Card,
                    card: Moneris3DSCard {
                        card_number: req_card.card_number.clone(),
                        expiry_month,
                        expiry_year,
                        card_security_code: Some(req_card.card_cvc.clone()),
                    },
                })
            }
            _ => {
                return Err(IntegrationError::NotImplemented(
                    "Moneris 3DS PreAuthenticate only supports card payments".to_string(),
                    IntegrationErrorContext {
                        suggested_action: Some(
                            "Use a card payment method for 3DS authentication.".to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(format!(
                            "Received non-card payment method: {:?}.",
                            payment_method_data
                        )),
                    },
                )
                .into())
            }
        };

        let cardholder_email = req
            .email
            .clone()
            .or_else(|| resource.get_optional_billing_email());

        let cardholder_name = match &payment_method_data {
            PaymentMethodData::Card(card) => card.card_holder_name.clone(),
            _ => None,
        }
        .or_else(|| resource.get_optional_billing_full_name())
        .ok_or(IntegrationError::MissingRequiredField {
            field_name: "cardholder_name",
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Provide card_holder_name on the card or first_name in the billing \
                     address for Moneris 3DS pre-authentication."
                        .to_string(),
                ),
                doc_url: None,
                additional_context: None,
            },
        })?;

        let billing_address = build_moneris_billing_address(resource)?;
        let shipping_address = build_moneris_shipping_address(resource)?;

        let amount = req.money.clone();

        let idempotency_key = format!("pre_auth_{}", resource.connector_request_reference_id);

        let (
            device_channel,
            browser_ip_address,
            browser_screen_height,
            browser_screen_width,
            browser_user_agent,
            browser_java_enabled,
            browser_javascript_enabled,
            browser_language,
        ) = match req.browser_info.as_ref() {
            Some(browser_info) => {
                let browser_ip_address = browser_info
                    .ip_address
                    .filter(|ip| ip.is_ipv4())
                    .map(|ip| Secret::new(ip.to_string()));
                let browser_screen_height = Some(required_browser_dimension(
                    browser_info.screen_height,
                    "browser_info.screen_height",
                )?);
                let browser_screen_width = Some(required_browser_dimension(
                    browser_info.screen_width,
                    "browser_info.screen_width",
                )?);
                let browser_user_agent = Some(
                    browser_info
                        .user_agent
                        .as_ref()
                        .filter(|user_agent| !user_agent.is_empty())
                        .ok_or_else(|| {
                            missing_moneris_three_ds_field(
                                "browser_info.user_agent",
                                "Provide the shopper browser's user-agent for Moneris 3DS.",
                            )
                        })?
                        .chars()
                        .take(2048)
                        .collect(),
                );
                let browser_java_enabled = Some(browser_info.java_enabled.ok_or_else(|| {
                    missing_moneris_three_ds_field(
                        "browser_info.java_enabled",
                        "Provide the shopper browser's Java-enabled value for Moneris 3DS.",
                    )
                })?);
                let browser_javascript_enabled =
                    Some(browser_info.java_script_enabled.ok_or_else(|| {
                        missing_moneris_three_ds_field(
                            "browser_info.java_script_enabled",
                            "Provide the shopper browser's JavaScript-enabled value for Moneris 3DS.",
                        )
                    })?);
                let browser_language = Some(
                    browser_info
                        .language
                        .as_ref()
                        .filter(|language| language.len() == 2 && language.is_ascii())
                        .cloned()
                        .ok_or_else(|| {
                            missing_moneris_three_ds_field(
                                "browser_info.language",
                                "Provide a two-letter ISO 639-1 browser language for Moneris 3DS.",
                            )
                        })?,
                );

                (
                    ThreeDSecureDeviceChannel::Browser,
                    browser_ip_address,
                    browser_screen_height,
                    browser_screen_width,
                    browser_user_agent,
                    browser_java_enabled,
                    browser_javascript_enabled,
                    browser_language,
                )
            }
            None => (
                ThreeDSecureDeviceChannel::ThreeDSecureRequestorInitiated,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ),
        };

        let is_browser_channel = matches!(&device_channel, ThreeDSecureDeviceChannel::Browser);
        let three_d_secure_notification_url = req
            .continue_redirection_url
            .as_ref()
            .map(|url| urlencoding::encode(url.as_str()).into_owned());
        if is_browser_channel && three_d_secure_notification_url.is_none() {
            return Err(missing_moneris_three_ds_field(
                "continue_redirection_url",
                "Provide the complete-authorize callback URL that can receive the issuer ACS cres POST.",
            )
            .into());
        }

        let cardholder_phone_number = resource
            .get_optional_billing()
            .and_then(|billing| billing.phone.as_ref())
            .and_then(|phone| phone.get_e123_phone_number());
        let cardholder_phone_number = match cardholder_phone_number {
            Some(phone_number) => Some(validate_moneris_phone_number(phone_number)?),
            None if is_browser_channel => {
                return Err(missing_moneris_three_ds_field(
                    "billing.phone",
                    "Provide the cardholder phone number with its country code in E.164 format.",
                )
                .into());
            }
            None => None,
        };
        let (three_d_secure_request_type, three_d_secure_challenge_window_size) =
            match &device_channel {
                ThreeDSecureDeviceChannel::Browser => (
                    ThreeDSecureRequestType::Cardholder,
                    Some(ThreeDSecureChallengeWindowSize::FullScreen),
                ),
                ThreeDSecureDeviceChannel::ThreeDSecureRequestorInitiated => {
                    (ThreeDSecureRequestType::Recurring, None)
                }
            };

        Ok(Self {
            idempotency_key,
            amount,
            cardholder_name,
            cardholder_email,
            cardholder_phone_number,
            billing_address,
            shipping_address,
            payment_method,
            three_d_secure_message_category: ThreeDSecureMessageCategory::Payment,
            three_d_secure_device_channel: device_channel,
            three_d_secure_request_type,
            three_d_secure_notification_url,
            three_d_secure_completion_indicator: ThreeDSecureCompletionIndicator::Unavailable,
            three_d_secure_challenge_requested: ThreeDSecureChallengeRequested::NoPreference,
            three_d_secure_challenge_window_size,
            browser_ip_address,
            browser_user_agent,
            browser_java_enabled,
            browser_javascript_enabled,
            browser_screen_height,
            browser_screen_width,
            browser_language,
        })
    }
}

// ---- PreAuthenticate Response ----

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonerisPreAuthenticateResponse {
    pub three_d_secure_authentication_id: String,
    pub three_d_secure_transaction_status: MonerisThreeDSecureTransactionStatus,
    pub three_d_secure_transaction_status_reason: Option<String>,
    pub three_d_secure_authentication_value: Option<Secret<String>>,
    pub ecommerce_indicator: String,
    pub three_d_secure_server_transaction_id: Option<String>,
    pub three_d_secure_directory_server_transaction_id: Option<String>,
    pub three_d_secure_access_control_server_transaction_id: Option<String>,
    pub three_d_secure_version: Option<String>,
    pub three_d_secure_challenge_url: Option<String>,
    pub three_d_secure_challenge_data: Option<String>,
}

impl<'a> From<&'a MonerisPreAuthenticateResponse> for MonerisAuthenticationDataFields<'a> {
    fn from(response: &'a MonerisPreAuthenticateResponse) -> Self {
        Self {
            transaction_status: &response.three_d_secure_transaction_status,
            ecommerce_indicator: &response.ecommerce_indicator,
            authentication_value: response.three_d_secure_authentication_value.as_ref(),
            server_transaction_id: response.three_d_secure_server_transaction_id.as_ref(),
            version: response.three_d_secure_version.as_deref(),
            directory_server_transaction_id: response
                .three_d_secure_directory_server_transaction_id
                .as_ref(),
            access_control_server_transaction_id: response
                .three_d_secure_access_control_server_transaction_id
                .as_ref(),
            authentication_id: &response.three_d_secure_authentication_id,
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<MonerisPreAuthenticateResponse, Self>>
    for RouterDataV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<MonerisPreAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;

        let (status, response_data) = match response.three_d_secure_transaction_status {
            MonerisThreeDSecureTransactionStatus::Authenticated
            | MonerisThreeDSecureTransactionStatus::AuthenticationAttempted => {
                let authentication_data = Some(build_moneris_authentication_data(response.into()));
                let response_data = PaymentsResponseData::PreAuthenticateResponse {
                    resource_id: Some(ResponseId::ConnectorTransactionId(
                        response.three_d_secure_authentication_id.clone(),
                    )),
                    authentication_data,
                    redirection_data: None,
                    connector_response_reference_id: Some(
                        response.three_d_secure_authentication_id.clone(),
                    ),
                    status_code: item.http_code,
                };
                (
                    response
                        .three_d_secure_transaction_status
                        .to_attempt_status(),
                    response_data,
                )
            }
            MonerisThreeDSecureTransactionStatus::ChallengeAuthenticationRequired => {
                let challenge_url =
                    response
                        .three_d_secure_challenge_url
                        .clone()
                        .ok_or_else(|| {
                            error_stack::report!(ConnectorError::ResponseDeserializationFailed {
                                context: ResponseTransformationErrorContext {
                                    http_status_code: Some(item.http_code),
                                    additional_context: Some(
                                        "Moneris returned CHALLENGE_AUTHENTICATION_REQUIRED but \
                                 challenge_url was missing in the response."
                                            .to_string(),
                                    ),
                                },
                            })
                        })?;
                let challenge_data =
                    response
                        .three_d_secure_challenge_data
                        .clone()
                        .ok_or_else(|| {
                            error_stack::report!(ConnectorError::ResponseDeserializationFailed {
                                context: ResponseTransformationErrorContext {
                                    http_status_code: Some(item.http_code),
                                    additional_context: Some(
                                        "Moneris returned CHALLENGE_AUTHENTICATION_REQUIRED but \
                                 challenge_data (creq) was missing in the response."
                                            .to_string(),
                                    ),
                                },
                            })
                        })?;
                let mut form_fields = HashMap::new();
                form_fields.insert("creq".to_string(), challenge_data);
                let redirection_data = Some(Box::new(
                    domain_types::router_response_types::RedirectForm::Form {
                        endpoint: challenge_url,
                        method: common_utils::request::Method::Post,
                        form_fields,
                    },
                ));
                let response_data = PaymentsResponseData::PreAuthenticateResponse {
                    resource_id: Some(ResponseId::ConnectorTransactionId(
                        response.three_d_secure_authentication_id.clone(),
                    )),
                    authentication_data: None,
                    redirection_data,
                    connector_response_reference_id: Some(
                        response.three_d_secure_authentication_id.clone(),
                    ),
                    status_code: item.http_code,
                };
                (
                    response
                        .three_d_secure_transaction_status
                        .to_attempt_status(),
                    response_data,
                )
            }
            MonerisThreeDSecureTransactionStatus::NotAuthenticated
            | MonerisThreeDSecureTransactionStatus::Rejected
            | MonerisThreeDSecureTransactionStatus::TechnicalIssue
            | MonerisThreeDSecureTransactionStatus::ChallengeDecoupledAuthenticationConfirmed
            | MonerisThreeDSecureTransactionStatus::ChallengePreferenceAcknowledged => {
                let authentication_data = Some(build_moneris_authentication_data(response.into()));
                let response_data = PaymentsResponseData::PreAuthenticateResponse {
                    resource_id: Some(ResponseId::ConnectorTransactionId(
                        response.three_d_secure_authentication_id.clone(),
                    )),
                    authentication_data,
                    redirection_data: None,
                    connector_response_reference_id: Some(
                        response.three_d_secure_authentication_id.clone(),
                    ),
                    status_code: item.http_code,
                };
                (
                    response
                        .three_d_secure_transaction_status
                        .to_attempt_status(),
                    response_data,
                )
            }
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response: Ok(response_data),
            ..item.router_data
        })
    }
}

// ---- PostAuthenticate Request ----

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonerisPostAuthenticateRequest {
    pub idempotency_key: String,
    pub three_d_secure_challenge_response_data: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MonerisRouterData<
            RouterDataV2<
                PostAuthenticate,
                PaymentFlowData,
                PaymentsPostAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for MonerisPostAuthenticateRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: MonerisRouterData<
            RouterDataV2<
                PostAuthenticate,
                PaymentFlowData,
                PaymentsPostAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Map the missing-payload error to "redirect_response" so the field probe's
        // patcher rule for post_authenticate.redirection_response fires and injects
        // a probe value. Without this remap the field name is
        // "request.redirect_response.payload" which matches no patcher alias.
        let cres_payload = item
            .router_data
            .request
            .get_redirect_response_payload()
            .map_err(|_| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                    field_name: "redirect_response",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Provide the cres (ACS challenge response) in \
                             redirect_response.payload for Moneris PostAuthenticate."
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: None,
                    },
                })
            })?;

        // Accept the payload as:
        //   1. A bare JSON string (direct cres)
        //   2. An object with a "cres" key (preferred map shape from the ACS callback)
        //   3. Any other string value in the map (field-probe fallback — the patcher
        //      injects {"TransactionId":"probe_txn_123"} which has no "cres" key)
        let cres_value = cres_payload
            .peek()
            .as_str()
            .map(|s| s.to_owned())
            .or_else(|| {
                let obj = cres_payload.peek().as_object()?;
                obj.get("cres")
                    .or_else(|| obj.values().next())
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_owned())
            })
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "redirect_response.payload.cres",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Ensure the cres value from the ACS callback is present in \
                         redirect_response.payload for Moneris PostAuthenticate."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: None,
                },
            })?;

        let idempotency_key = format!(
            "post_auth_{}",
            item.router_data
                .resource_common_data
                .connector_request_reference_id
        );

        Ok(Self {
            idempotency_key,
            three_d_secure_challenge_response_data: cres_value,
        })
    }
}

// ---- PostAuthenticate Response ----

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonerisPostAuthenticateResponse {
    pub three_d_secure_authentication_id: String,
    pub three_d_secure_authentication_value: Option<Secret<String>>,
    pub ecommerce_indicator: String,
    pub three_d_secure_server_transaction_id: Option<String>,
    pub three_d_secure_directory_server_transaction_id: Option<String>,
    pub three_d_secure_access_control_server_transaction_id: Option<String>,
    pub three_d_secure_version: Option<String>,
    pub three_d_secure_transaction_status: MonerisThreeDSecureTransactionStatus,
}

impl<'a> From<&'a MonerisPostAuthenticateResponse> for MonerisAuthenticationDataFields<'a> {
    fn from(response: &'a MonerisPostAuthenticateResponse) -> Self {
        Self {
            transaction_status: &response.three_d_secure_transaction_status,
            ecommerce_indicator: &response.ecommerce_indicator,
            authentication_value: response.three_d_secure_authentication_value.as_ref(),
            server_transaction_id: response.three_d_secure_server_transaction_id.as_ref(),
            version: response.three_d_secure_version.as_deref(),
            directory_server_transaction_id: response
                .three_d_secure_directory_server_transaction_id
                .as_ref(),
            access_control_server_transaction_id: response
                .three_d_secure_access_control_server_transaction_id
                .as_ref(),
            authentication_id: &response.three_d_secure_authentication_id,
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<MonerisPostAuthenticateResponse, Self>>
    for RouterDataV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<MonerisPostAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;

        let status = response
            .three_d_secure_transaction_status
            .to_attempt_status();

        let authentication_data = Some(build_moneris_authentication_data(response.into()));

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::PostAuthenticateResponse {
                authentication_data,
                connector_response_reference_id: Some(
                    response.three_d_secure_authentication_id.clone(),
                ),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}
