use crate::{types::ResponseRouterData, utils};
use common_enums::RefundStatus;
use common_utils::types::MinorUnit;
use domain_types::{
    connector_flow::{
        Authorize, Capture, RSync, Refund, RepeatPayment, ServerAuthenticationToken, Void,
    },
    connector_types::{
        MandateReference, PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsResponseData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, RepeatPaymentData, ResponseId, ServerAuthenticationTokenResponseData,
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
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

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
    amount: Amount,
    payment_method: PaymentMethod<T>,
    automatic_capture: bool,
}

#[derive(Default, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Amount {
    currency: common_enums::Currency,
    amount: MinorUnit,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum PaymentMethod<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    Card(PaymentMethodCard<T>),
    PaymentMethodId(PaymentMethodId),
    ApplePayEncrypted(PaymentMethodApplePayEncrypted),
    ApplePayDecrypted(PaymentMethodApplePayDecrypted),
    GooglePayEncrypted(PaymentMethodGooglePayEncrypted),
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
}

/// Internal struct for deserializing Apple Pay encrypted token header
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApplePayEncryptedTokenHeader {
    public_key_hash: String,
    ephemeral_public_key: String,
    transaction_id: Option<String>,
}

/// Internal struct for deserializing Apple Pay encrypted token
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApplePayEncryptedToken {
    version: String,
    data: String,
    signature: String,
    header: ApplePayEncryptedTokenHeader,
}

/// Internal struct for deserializing Google Pay encrypted token
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GooglePayEncryptedToken {
    protocol_version: String,
    signature: String,
    signed_message: String,
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
pub struct PaymentMethodApplePayEncrypted {
    payment_method_source: PaymentMethodSource,
    display_name: String,
    card_brand: MonerisCardBrand,
    apple_pay_version: ApplePayVersion,
    data: String,
    signature: String,
    public_key_hash: String,
    ephemeral_public_key: String,
    apple_pay_transaction_id: String,
    wallet_indicator: WalletIndicator,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaymentMethodApplePayDecrypted {
    payment_method_source: PaymentMethodSource,
    application_primary_account_number: Secret<String>,
    expiry_month: i64,
    expiry_year: i64,
    data_type: ApplePayDataType,
    cryptogram: Secret<String>,
    card_brand: MonerisCardBrand,
    #[serde(skip_serializing_if = "Option::is_none")]
    wallet_ecommerce_indicator: Option<String>,
    wallet_indicator: WalletIndicator,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaymentMethodGooglePayEncrypted {
    payment_method_source: PaymentMethodSource,
    card_brand: MonerisCardBrand,
    signature: String,
    google_pay_protocol_version: String,
    signed_message: String,
    wallet_indicator: WalletIndicator,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GooglePayDecryptedCardDetails {
    personal_account_number: Secret<String>,
    expiry_month: i64,
    expiry_year: i64,
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
        match item.router_data.request.payment_method_data.clone() {
            PaymentMethodData::Card(ref req_card) => {
                if item.router_data.resource_common_data.is_three_ds() {
                    Err(IntegrationError::NotSupported {
                        message: "Card 3DS".to_string(),
                        connector: "Moneris",
                        context: IntegrationErrorContext {
                            suggested_action: Some(
                                "Disable 3DS for this payment or use a Moneris integration \
                                 that supports 3DS authentication."
                                    .to_string(),
                            ),
                            doc_url: None,
                            additional_context: Some(
                                "Moneris Authorize was called with three_ds enabled on \
                                 `resource_common_data`, but this connector implementation \
                                 does not support 3DS-authenticated card transactions."
                                    .to_string(),
                            ),
                        },
                    })?
                };
                let idempotency_key = format!(
                    "auth_{}",
                    item.router_data
                        .resource_common_data
                        .connector_request_reference_id
                );
                let amount = Amount {
                    currency: item.router_data.request.currency,
                    amount: item.router_data.request.amount,
                };
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
                let automatic_capture = item.router_data.request.is_auto_capture();

                Ok(Self {
                    idempotency_key,
                    amount,
                    payment_method,
                    automatic_capture,
                })
            }
            PaymentMethodData::Wallet(ref wallet_data) => {
                let idempotency_key = format!(
                    "auth_{}",
                    item.router_data
                        .resource_common_data
                        .connector_request_reference_id
                );
                let amount = Amount {
                    currency: item.router_data.request.currency,
                    amount: item.router_data.request.amount,
                };
                let automatic_capture = item.router_data.request.is_auto_capture();

                let payment_method = match wallet_data {
                    WalletData::ApplePay(apple_pay_data) => match &apple_pay_data.payment_data {
                        ApplePayPaymentData::Encrypted(encrypted_str) => {
                            let token: ApplePayEncryptedToken = serde_json::from_str(encrypted_str)
                                .change_context(IntegrationError::RequestEncodingFailed {
                                    context: IntegrationErrorContext {
                                        suggested_action: Some(
                                            "Ensure the Apple Pay payment data is a \
                                                     valid JSON token."
                                                .to_string(),
                                        ),
                                        doc_url: None,
                                        additional_context: Some(
                                            "Failed to parse Apple Pay encrypted token \
                                                     JSON"
                                                .to_string(),
                                        ),
                                    },
                                })?;
                            let apple_pay_version = match token.version.as_str() {
                                "RSA_V1" => ApplePayVersion::RsaV1,
                                _ => ApplePayVersion::EcV1,
                            };
                            let card_brand = MonerisCardBrand::try_from(
                                apple_pay_data.payment_method.network.as_str(),
                            )?;
                            let apple_pay_transaction_id = token
                                .header
                                .transaction_id
                                .unwrap_or_else(|| apple_pay_data.transaction_identifier.clone());
                            PaymentMethod::ApplePayEncrypted(PaymentMethodApplePayEncrypted {
                                payment_method_source: PaymentMethodSource::ApplePayEncrypted,
                                display_name: apple_pay_data.payment_method.display_name.clone(),
                                card_brand,
                                apple_pay_version,
                                data: token.data,
                                signature: token.signature,
                                public_key_hash: token.header.public_key_hash,
                                ephemeral_public_key: token.header.ephemeral_public_key,
                                apple_pay_transaction_id,
                                wallet_indicator: WalletIndicator::InBrowser,
                            })
                        }
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
                                .application_expiration_year
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
                                expiry_month,
                                expiry_year,
                                data_type: ApplePayDataType::ThreeDSecure,
                                cryptogram: decrypt_data
                                    .payment_data
                                    .online_payment_cryptogram
                                    .clone(),
                                card_brand,
                                wallet_ecommerce_indicator: decrypt_data
                                    .payment_data
                                    .eci_indicator
                                    .clone(),
                                wallet_indicator: WalletIndicator::InApplication,
                            })
                        }
                    },
                    WalletData::GooglePay(gpay_data) => {
                        let card_brand =
                            MonerisCardBrand::try_from(gpay_data.info.card_network.as_str())?;
                        match &gpay_data.tokenization_data {
                            GpayTokenizationData::Encrypted(encrypted_data) => {
                                let token: GooglePayEncryptedToken = serde_json::from_str(
                                    &encrypted_data.token,
                                )
                                .change_context(IntegrationError::RequestEncodingFailed {
                                    context: IntegrationErrorContext {
                                        suggested_action: Some(
                                            "Ensure the Google Pay token is a valid JSON \
                                                     string with protocolVersion, signature, and \
                                                     signedMessage fields."
                                                .to_string(),
                                        ),
                                        doc_url: None,
                                        additional_context: Some(
                                            "Failed to parse Google Pay encrypted token \
                                                     JSON"
                                                .to_string(),
                                        ),
                                    },
                                })?;
                                PaymentMethod::GooglePayEncrypted(PaymentMethodGooglePayEncrypted {
                                    payment_method_source: PaymentMethodSource::GooglePayEncrypted,
                                    card_brand,
                                    signature: token.signature,
                                    google_pay_protocol_version: token.protocol_version,
                                    signed_message: token.signed_message,
                                    wallet_indicator: WalletIndicator::InBrowser,
                                })
                            }
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
                                    .card_exp_year
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
                                    wallet_source: GooglePayWalletSource::TokenizedCard,
                                    card_brand,
                                    wallet_indicator: WalletIndicator::InBrowser,
                                    card_details: GooglePayDecryptedCardDetails {
                                        personal_account_number: Secret::new(
                                            decrypt_data
                                                .application_primary_account_number
                                                .get_card_no(),
                                        ),
                                        expiry_month,
                                        expiry_year,
                                        authentication_method: None,
                                        cryptogram: decrypt_data.cryptogram.clone(),
                                        wallet_ecommerce_indicator: decrypt_data
                                            .eci_indicator
                                            .clone(),
                                    },
                                })
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

                Ok(Self {
                    idempotency_key,
                    amount,
                    payment_method,
                    automatic_capture,
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
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
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
}

impl From<MonerisPaymentStatus> for common_enums::AttemptStatus {
    fn from(item: MonerisPaymentStatus) -> Self {
        match item {
            MonerisPaymentStatus::Succeeded => Self::Charged,
            MonerisPaymentStatus::Authorized => Self::Authorized,
            MonerisPaymentStatus::Canceled => Self::Voided,
            MonerisPaymentStatus::Declined | MonerisPaymentStatus::DeclinedRetry => Self::Failure,
            MonerisPaymentStatus::Processing => Self::Pending,
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
                let captured = item.router_data.request.minor_amount_to_capture;
                match item
                    .router_data
                    .resource_common_data
                    .minor_amount_capturable
                {
                    Some(authorized) if captured < authorized => {
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
    amount: Amount,
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
                let amount = Amount {
                    currency: item.router_data.request.currency,
                    amount: item.router_data.request.minor_amount,
                };
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
                });
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
    amount: Amount,
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
        let amount = Amount {
            currency: item.router_data.request.currency,
            amount: item.router_data.request.minor_amount_to_capture,
        };
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

#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonerisRefundRequest {
    pub refund_amount: Amount,
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
        let refund_amount = Amount {
            currency: item.router_data.request.currency,
            amount: item.router_data.request.minor_refund_amount,
        };
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
