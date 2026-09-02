use crate::{connectors::globalpay::GlobalpayRouterData, types::ResponseRouterData};
use common_enums::{AttemptStatus, RefundStatus};
use common_utils::consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE};
use common_utils::request::Method;
use common_utils::types::StringMinorUnit;
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, PSync, PaymentMethodToken, RSync, Refund,
        RepeatPayment, ServerAuthenticationToken, SetupMandate, Void,
    },
    connector_types::{
        ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData,
        ConnectorSpecificClientAuthenticationResponse,
        GlobalpayClientAuthenticationResponse as GlobalpayClientAuthenticationResponseDomain,
        MandateReference, MandateReferenceId, PaymentFlowData, PaymentMethodTokenResponse,
        PaymentMethodTokenizationData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, RepeatPaymentData, ResponseId, ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData, SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::{
        BankRedirectData, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber, WalletData,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use rand::distributions::DistString;
use serde::{Deserialize, Serialize};
use url::Url;

// ===== TYPE ALIASES FOR MACRO =====
// These type aliases are needed because the create_all_prerequisites! macro
// creates unique "Templating" structs for each response type, but GlobalPay
// reuses the same response types across multiple flows. To avoid duplication errors,
// we create flow-specific aliases that reference the same underlying types.

/// Response type for Authorize flow - reuses GlobalpayPaymentsResponse
pub type GlobalpayAuthorizeResponse = GlobalpayPaymentsResponse;
/// Response type for PSync flow - reuses GlobalpayPaymentsResponse
pub type GlobalpayPSyncResponse = GlobalpayPaymentsResponse;
/// Response type for Void flow - reuses GlobalpayPaymentsResponse
pub type GlobalpayVoidResponse = GlobalpayPaymentsResponse;
/// Response type for Capture flow - reuses GlobalpayPaymentsResponse
pub type GlobalpayCaptureResponse = GlobalpayPaymentsResponse;
/// Response type for SetupMandate flow. GlobalPay's mandate setup tokenizes the
/// card via the `/payment-methods` endpoint; the `PMT_` id returned here is
/// what we surface as the connector_mandate_id for later MIT charges.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlobalpaySetupMandateResponse {
    pub id: String,
    pub reference: Option<String>,
    pub usage_mode: Option<GlobalpayUsageMode>,
    pub card: Option<GlobalpayTokenizedCard>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlobalpayTokenizedCard {
    pub masked_number_last4: Option<Secret<String>>,
    pub brand: Option<Secret<String>>,
    pub expiry_month: Option<Secret<String>>,
    pub expiry_year: Option<Secret<String>>,
}
/// Response type for RSync flow - reuses GlobalpayRefundResponse
pub type GlobalpayRSyncResponse = GlobalpayRefundResponse;

// ===== CONSTANTS =====

mod constants {

    /// Entry mode for e-commerce transactions
    pub(super) const ENTRY_MODE_ECOM: &str = "ECOM";

    /// Channel for card-not-present transactions
    pub(super) const CHANNEL_CNP: &str = "CNP";
}

#[derive(Debug, Clone)]
pub struct GlobalpayAuthType {
    pub app_id: Secret<String>,
    pub app_key: Secret<String>,
    pub account_name: Option<Secret<String>>,
}

impl TryFrom<&ConnectorSpecificConfig> for GlobalpayAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Globalpay {
                app_id,
                app_key,
                account_name,
                ..
            } => Ok(Self {
                app_id: app_id.to_owned(),
                app_key: app_key.to_owned(),
                account_name: account_name.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Expected ConnectorSpecificConfig::Globalpay with app_id and app_key \
                             fields; received a different connector variant"
                                .to_string(),
                        ),
                        suggested_action: Some(
                            "Ensure the connector is configured as Globalpay with valid \
                             app_id and app_key credentials"
                                .to_string(),
                        ),
                        doc_url: None,
                    },
                }
            )),
        }
    }
}

#[derive(Debug, Clone, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct GlobalpayErrorResponse {
    pub error_code: String,
    pub detailed_error_code: String,
    pub detailed_error_description: String,
}

// Access token endpoint returns a simpler error shape — no detailed_error_code field
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalpayAccessTokenErrorResponse {
    pub error_code: String,
    pub detailed_error_description: String,
}

// ===== STATUS ENUMS =====

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GlobalpayPaymentStatus {
    Captured,
    Preauthorized,
    Declined,
    Failed,
    Rejected,
    Pending,
    Initiated,
    ForReview,
    Funded,
    Reversed,
}

impl From<GlobalpayPaymentStatus> for AttemptStatus {
    fn from(status: GlobalpayPaymentStatus) -> Self {
        match status {
            GlobalpayPaymentStatus::Captured => Self::Charged,
            GlobalpayPaymentStatus::Preauthorized => Self::Authorized,
            GlobalpayPaymentStatus::Declined => Self::Failure,
            GlobalpayPaymentStatus::Failed => Self::Failure,
            GlobalpayPaymentStatus::Rejected => Self::Failure,
            GlobalpayPaymentStatus::Pending => Self::Pending,
            GlobalpayPaymentStatus::Initiated => Self::AuthenticationPending,
            GlobalpayPaymentStatus::ForReview => Self::Pending,
            GlobalpayPaymentStatus::Funded => Self::Charged,
            GlobalpayPaymentStatus::Reversed => Self::Voided,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GlobalpayRefundStatus {
    Captured,
    Funded,
    Pending,
    Initiated,
    ForReview,
    Declined,
    Failed,
    Rejected,
    // A refund can be reversed (voided before funding) or, rarely, pre-authorized
    Reversed,
    Preauthorized,
}

impl From<GlobalpayRefundStatus> for RefundStatus {
    fn from(status: GlobalpayRefundStatus) -> Self {
        match status {
            GlobalpayRefundStatus::Captured | GlobalpayRefundStatus::Funded => Self::Success,
            GlobalpayRefundStatus::Pending
            | GlobalpayRefundStatus::Initiated
            | GlobalpayRefundStatus::ForReview
            | GlobalpayRefundStatus::Preauthorized => Self::Pending,
            GlobalpayRefundStatus::Declined
            | GlobalpayRefundStatus::Failed
            | GlobalpayRefundStatus::Rejected
            | GlobalpayRefundStatus::Reversed => Self::Failure,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Sequence {
    First,
    Last,
    Subsequent,
}

/// GlobalPay `usage_mode` on /payment-methods. `MULTIPLE` allows the returned
/// PMT_ id to be reused for subsequent MIT charges; `SINGLE` restricts it to
/// a single subsequent transaction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GlobalpayUsageMode {
    Single,
    Multiple,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GlobalpayCaptureMode {
    Auto,
    Later,
}

// ===== OAUTH / ACCESS TOKEN FLOW STRUCTURES =====

#[derive(Debug, Serialize)]
pub struct GlobalpayAccessTokenRequest {
    pub app_id: Secret<String>,
    pub nonce: Secret<String>,
    pub secret: Secret<String>,
    pub grant_type: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GlobalpayRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for GlobalpayAccessTokenRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        value: GlobalpayRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &value.router_data;
        let auth = GlobalpayAuthType::try_from(&item.connector_config)?;

        use sha2::{Digest, Sha512};
        let nonce = rand::distributions::Alphanumeric.sample_string(&mut rand::thread_rng(), 12);
        let secret_input = format!("{}{}", nonce, auth.app_key.peek());
        let mut hasher = Sha512::new();
        hasher.update(secret_input.as_bytes());
        let secret_hex = hex::encode(hasher.finalize());

        Ok(Self {
            app_id: auth.app_id,
            nonce: Secret::new(nonce),
            secret: Secret::new(secret_hex),
            grant_type: item.request.grant_type.clone(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GlobalpayAccessTokenResponse {
    pub token: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub seconds_to_expire: i64,
}

impl<F, T> TryFrom<ResponseRouterData<GlobalpayAccessTokenResponse, Self>>
    for RouterDataV2<F, MerchantAuthenticationFlowData, T, ServerAuthenticationTokenResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlobalpayAccessTokenResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(ServerAuthenticationTokenResponseData {
                access_token: item.response.token.into(),
                token_type: Some(item.response.type_),
                expires_in: Some(item.response.seconds_to_expire),
            }),
            ..item.router_data
        })
    }
}

// ===== PAYMENT FLOW STRUCTURES =====

#[derive(Debug, Serialize)]
pub struct GlobalpayNotifications {
    pub cancel_url: String,
    pub return_url: String,
    pub status_url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InitiatorType {
    Merchant,
    Payer,
}

#[derive(Debug, Serialize)]
pub struct Initiator {
    #[serde(rename = "type")]
    pub initiator_type: Option<InitiatorType>,
    pub id: Option<String>,
    pub stored_credential: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StoredCredentialType {
    Installment,
    Recurring,
    Unscheduled,
    Subscription,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StoredCredentialSequence {
    First,
    Subsequent,
}

#[derive(Debug, Serialize)]
pub struct StoredCredential {
    #[serde(rename = "type")]
    pub credential_type: Option<StoredCredentialType>,
    pub sequence: Option<StoredCredentialSequence>,
    pub initiator: Option<InitiatorType>,
}

/// Transaction type for GlobalPay. `Sale` moves funds from payer to merchant;
/// `Refund` moves funds from merchant to payer (used on the dedicated /refund endpoint).
/// Authorize and RepeatPayment flows always use `Sale`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GlobalpayTransactionType {
    Sale,
    Refund,
}

// ===== APM / BANK REDIRECT STRUCTURES =====

/// APM (Alternative Payment Method) provider for bank redirect payments
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApmProvider {
    Giropay,
    Ideal,
    Paypal,
    Eps,
    Testpay,
}

/// APM payment method data for bank redirect flows
#[derive(Debug, Serialize)]
pub struct GlobalpayApm {
    /// A string used to identify the payment method provider being used to execute this transaction.
    pub provider: Option<ApmProvider>,
}

/// Digital wallet provider identifier. GlobalPay uses SCREAMING_SNAKE_CASE here.
#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GlobalpayDigitalWalletProvider {
    PayByGoogle,
}

/// Digital wallet payment method data (Google Pay).
/// The `payment_token` is the raw JSON object returned by the Google Pay API.
#[derive(Debug, Serialize)]
pub struct GlobalpayDigitalWallet {
    pub provider: GlobalpayDigitalWalletProvider,
    pub payment_token: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct GlobalpayPaymentsRequest<T: PaymentMethodDataTypes> {
    pub account_name: String,
    #[serde(rename = "type")]
    pub type_: GlobalpayTransactionType,
    pub channel: String,
    pub amount: StringMinorUnit,
    pub currency: common_enums::Currency,
    pub reference: String,
    pub country: common_enums::CountryAlpha2,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_mode: Option<GlobalpayCaptureMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initiator: Option<Initiator>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notifications: Option<GlobalpayNotifications>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stored_credential: Option<StoredCredential>,
    pub payment_method: GlobalpayPaymentMethod<T>,
}

#[derive(Debug, Serialize)]
pub struct GlobalpayPaymentMethod<T: PaymentMethodDataTypes> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Secret<String>>,
    pub entry_mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<GlobalpayCard<T>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apm: Option<GlobalpayApm>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digital_wallet: Option<GlobalpayDigitalWallet>,
    /// Connector-issued token reference (e.g. from GlobalPayments.js hosted fields).
    /// When set, GlobalPay looks up the tokenized card by this ID instead of
    /// requiring raw card data in the request body.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
pub struct GlobalpayCard<T: PaymentMethodDataTypes> {
    pub number: RawCardNumber<T>,
    pub expiry_month: Secret<String>,
    pub expiry_year: Secret<String>,
    pub cvv: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cvv_indicator: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GlobalpayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for GlobalpayPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: GlobalpayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &wrapper.router_data;
        let payment_method = match &item.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                let expiry_year_2digit = card_data.get_card_expiry_year_2_digit().change_context(
                    IntegrationError::RequestEncodingFailed {
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "Failed to convert card expiry year to 2-digit format for \
                                 GlobalPay Authorize request"
                                    .to_string(),
                            ),
                            suggested_action: None,
                            doc_url: None,
                        },
                    },
                )?;

                let cvv_indicator = if card_data.card_cvc.peek().is_empty() {
                    Some("NOT_PRESENT".to_string())
                } else {
                    Some("PRESENT".to_string())
                };

                GlobalpayPaymentMethod {
                    name: item.request.customer_name.clone().map(Secret::new),
                    entry_mode: constants::ENTRY_MODE_ECOM.to_string(),
                    card: Some(GlobalpayCard {
                        number: card_data.card_number.clone(),
                        expiry_month: card_data.card_exp_month.clone(),
                        expiry_year: expiry_year_2digit,
                        cvv: card_data.card_cvc.clone(),
                        cvv_indicator,
                    }),
                    apm: None,
                    digital_wallet: None,
                    id: None,
                }
            }
            PaymentMethodData::BankRedirect(bank_redirect) => {
                let apm_provider = match bank_redirect {
                    BankRedirectData::Eps { .. } => ApmProvider::Eps,
                    BankRedirectData::Giropay { .. } => ApmProvider::Giropay,
                    BankRedirectData::Ideal { .. } => ApmProvider::Ideal,
                    // Sofort was discontinued by Klarna in 2024 and is no longer
                    // supported by GlobalPay.
                    BankRedirectData::Sofort { .. } => {
                        return Err(error_stack::report!(IntegrationError::NotSupported {
                            message: "Sofort".to_string(),
                            connector: "globalpay",
                            context: IntegrationErrorContext {
                                additional_context: Some(
                                    "Sofort was discontinued by Klarna in 2024 and is no \
                                     longer supported by GlobalPay"
                                        .to_string(),
                                ),
                                suggested_action: Some(
                                    "Use iDEAL, EPS, or Giropay for bank redirect payments"
                                        .to_string(),
                                ),
                                doc_url: None,
                            },
                        }))
                    }
                    _ => {
                        return Err(error_stack::report!(IntegrationError::NotImplemented(
                            "Bank redirect payment method not supported".to_string(),
                            IntegrationErrorContext {
                                additional_context: Some(
                                    "GlobalPay Authorize supports EPS, iDEAL, and Giropay \
                                     bank redirects; received an unsupported variant"
                                        .to_string(),
                                ),
                                suggested_action: None,
                                doc_url: None,
                            },
                        )))
                    }
                };

                GlobalpayPaymentMethod {
                    name: item.request.customer_name.clone().map(Secret::new),
                    entry_mode: constants::ENTRY_MODE_ECOM.to_string(),
                    card: None,
                    apm: Some(GlobalpayApm {
                        provider: Some(apm_provider),
                    }),
                    digital_wallet: None,
                    id: None,
                }
            }

            PaymentMethodData::Wallet(wallet_data) => match wallet_data {
                WalletData::PaypalRedirect(_) => GlobalpayPaymentMethod {
                    name: item.request.customer_name.clone().map(Secret::new),
                    entry_mode: constants::ENTRY_MODE_ECOM.to_string(),
                    card: None,
                    apm: Some(GlobalpayApm {
                        provider: Some(ApmProvider::Paypal),
                    }),
                    digital_wallet: None,
                    id: None,
                },
                WalletData::GooglePay(_) => {
                    let payment_token = wallet_data
                        .get_wallet_token_as_json::<serde_json::Value>("Google Pay".to_string())
                        .change_context(IntegrationError::RequestEncodingFailed {
                            context: IntegrationErrorContext {
                                additional_context: Some(
                                    "Failed to parse Google Pay token as JSON for GlobalPay \
                                     POST /transactions digital_wallet.payment_token"
                                        .to_string(),
                                ),
                                suggested_action: None,
                                doc_url: None,
                            },
                        })?;

                    GlobalpayPaymentMethod {
                        name: item.request.customer_name.clone().map(Secret::new),
                        entry_mode: constants::ENTRY_MODE_ECOM.to_string(),
                        card: None,
                        apm: None,
                        digital_wallet: Some(GlobalpayDigitalWallet {
                            provider: GlobalpayDigitalWalletProvider::PayByGoogle,
                            payment_token,
                        }),
                        id: None,
                    }
                }
                _ => {
                    return Err(error_stack::report!(IntegrationError::NotImplemented(
                        "Wallet payment method not supported".to_string(),
                        IntegrationErrorContext {
                            additional_context: Some(
                                "GlobalPay Authorize supports PaypalRedirect and GooglePay \
                                 wallets; received an unsupported wallet variant"
                                    .to_string(),
                            ),
                            suggested_action: None,
                            doc_url: None,
                        },
                    )))
                }
            },

            PaymentMethodData::PaymentMethodToken(t) => {
                let token = t.token.clone();

                GlobalpayPaymentMethod {
                    name: item.request.customer_name.clone().map(Secret::new),
                    entry_mode: constants::ENTRY_MODE_ECOM.to_string(),
                    card: None,
                    apm: None,
                    digital_wallet: None,
                    id: Some(token),
                }
            }
            _ => {
                return Err(error_stack::report!(IntegrationError::NotImplemented(
                    "Payment method not supported".to_string(),
                    IntegrationErrorContext {
                        additional_context: Some(
                            "GlobalPay Authorize supports Card, BankRedirect (EPS/iDEAL/\
                             Giropay/Sofort), Wallet (PaypalRedirect/GooglePay), and \
                             PaymentMethodToken; received an unsupported payment method type"
                                .to_string(),
                        ),
                        suggested_action: None,
                        doc_url: None,
                    },
                )))
            }
        };

        let capture_mode = match item.request.capture_method {
            Some(common_enums::CaptureMethod::Manual) => Some(GlobalpayCaptureMode::Later),
            _ => Some(GlobalpayCaptureMode::Auto),
        };

        let country = item.resource_common_data.get_billing_country()?;

        let notifications = if let (Some(return_url), Some(webhook_url)) = (
            item.request.router_return_url.as_ref(),
            item.request.webhook_url.as_ref(),
        ) {
            Some(GlobalpayNotifications {
                cancel_url: return_url.clone(),
                return_url: return_url.clone(),
                status_url: webhook_url.clone(),
            })
        } else {
            None
        };

        let auth = GlobalpayAuthType::try_from(&item.connector_config)?;
        let account_name = auth
            .account_name
            .ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                    field_name: "account_name",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "GlobalPay requires account_name in connector config to identify the \
                             processing account for POST /transactions"
                                .to_string(),
                        ),
                        suggested_action: Some(
                            "Set account_name in the GlobalPay connector configuration".to_string(),
                        ),
                        doc_url: None,
                    },
                })
            })?
            .peek()
            .to_string();

        let amount = wrapper
            .connector
            .amount_converter
            .convert(item.request.minor_amount, item.request.currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to convert authorize amount to StringMinorUnit for GlobalPay \
                         POST /transactions request"
                            .to_string(),
                    ),
                    suggested_action: None,
                    doc_url: None,
                },
            })?;

        Ok(Self {
            account_name,
            type_: GlobalpayTransactionType::Sale,
            channel: constants::CHANNEL_CNP.to_string(),
            amount,
            currency: item.request.currency,
            reference: item
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            country,
            capture_mode,
            initiator: None,
            notifications,
            stored_credential: None,
            payment_method,
        })
    }
}

// Capture Request Structure
#[derive(Debug, Serialize)]
pub struct GlobalpayCaptureRequest {
    pub amount: StringMinorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_sequence: Option<Sequence>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GlobalpayRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for GlobalpayCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: GlobalpayRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &wrapper.router_data;
        let amount = wrapper
            .connector
            .amount_converter
            .convert(item.request.minor_amount_to_capture, item.request.currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to convert capture amount to StringMinorUnit for GlobalPay \
                         POST /transactions/{id}/capture request"
                            .to_string(),
                    ),
                    suggested_action: None,
                    doc_url: None,
                },
            })?;

        Ok(Self {
            amount,
            capture_sequence: item.request.multiple_capture_data.as_ref().map(|mcd| {
                if mcd.capture_sequence == 1 {
                    Sequence::First
                } else {
                    Sequence::Subsequent
                }
            }),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GlobalpayPaymentsResponse {
    pub id: String,
    pub status: GlobalpayPaymentStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<StringMinorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<common_enums::Currency>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method: Option<GlobalpayPaymentMethodResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GlobalpayPaymentMethodResponse {
    pub card: Option<GlobalpayCardResponse>,
    pub apm: Option<GlobalpayApmResponse>,
    pub id: Option<Secret<String>>,
    pub message: Option<String>,
    pub result: Option<String>,
}

/// Data associated with the response of an APM transaction
#[derive(Debug, Deserialize, Serialize)]
pub struct GlobalpayApmResponse {
    #[serde(alias = "provider_redirect_url")]
    pub redirect_url: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GlobalpayCardResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brand_reference: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub masked_number_last4: Option<String>,
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<GlobalpayPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlobalpayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Extract redirect URL from APM response for bank redirect flows
        let redirect_url = item
            .response
            .payment_method
            .as_ref()
            .and_then(|payment_method| {
                payment_method
                    .apm
                    .as_ref()
                    .and_then(|apm| apm.redirect_url.as_ref())
            })
            .filter(|redirect_str| !redirect_str.is_empty())
            .map(|url| {
                Url::parse(url).change_context(crate::utils::response_handling_fail_for_connector(
                    item.http_code,
                    "globalpay",
                ))
            })
            .transpose()?;

        let redirection_data = redirect_url
            .as_ref()
            .map(|url| Box::new(RedirectForm::from((url.clone(), Method::Get))));

        // Determine status based on connector status and presence of redirect
        let status = AttemptStatus::from(item.response.status.clone());

        // Extract network transaction ID from card response
        let network_txn_id = item
            .response
            .payment_method
            .as_ref()
            .and_then(|pm| pm.card.as_ref())
            .and_then(|card| card.brand_reference.as_ref())
            .map(|s| s.peek().to_string());

        // Handle failure responses separately
        let response = match status {
            AttemptStatus::Failure => Err(ErrorResponse {
                status_code: item.http_code,
                code: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.result.clone())
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.message.clone())
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.message.clone()),
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: Some(item.response.id.clone()),
                network_decline_code: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.result.clone()),
                network_advice_code: None,
                network_error_message: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.message.clone()),
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            }),
            _ => Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id,
                network_txn_link_id: None,
                connector_response_reference_id: item.response.reference.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            }),
        };

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

// PSync flow - reuses the same GlobalpayPaymentsResponse structure
impl TryFrom<ResponseRouterData<GlobalpayPaymentsResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlobalpayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::from(item.response.status.clone());

        let network_txn_id = item
            .response
            .payment_method
            .as_ref()
            .and_then(|pm| pm.card.as_ref())
            .and_then(|card| card.brand_reference.as_ref())
            .map(|s| s.peek().to_string());

        // For pending APM payments, the GET /transactions/{id} response still includes
        // the redirect URL so the caller can re-redirect the user if needed.
        let redirection_data = item
            .response
            .payment_method
            .as_ref()
            .and_then(|pm| pm.apm.as_ref())
            .and_then(|apm| apm.redirect_url.as_ref())
            .filter(|url| !url.is_empty())
            .map(|url| {
                Url::parse(url).change_context(crate::utils::response_handling_fail_for_connector(
                    item.http_code,
                    "globalpay",
                ))
            })
            .transpose()?
            .map(|url| Box::new(RedirectForm::from((url, Method::Get))));

        let response = match status {
            AttemptStatus::Failure => Err(ErrorResponse {
                status_code: item.http_code,
                code: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.result.clone())
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.message.clone())
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.message.clone()),
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: Some(item.response.id.clone()),
                network_decline_code: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.result.clone()),
                network_advice_code: None,
                network_error_message: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.message.clone()),
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            }),
            _ => Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id,
                network_txn_link_id: None,
                connector_response_reference_id: item.response.reference.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            }),
        };

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

// Capture flow - reuses the same GlobalpayPaymentsResponse structure
impl TryFrom<ResponseRouterData<GlobalpayPaymentsResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlobalpayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::from(item.response.status.clone());

        // Extract network transaction ID from card response
        let network_txn_id = item
            .response
            .payment_method
            .as_ref()
            .and_then(|pm| pm.card.as_ref())
            .and_then(|card| card.brand_reference.as_ref())
            .map(|s| s.peek().to_string());

        // Handle failure responses separately
        let response = match status {
            AttemptStatus::Failure => Err(ErrorResponse {
                status_code: item.http_code,
                code: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.result.clone())
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.message.clone())
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.message.clone()),
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: Some(item.response.id.clone()),
                network_decline_code: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.result.clone()),
                network_advice_code: None,
                network_error_message: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.message.clone()),
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            }),
            _ => Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id,
                network_txn_link_id: None,
                connector_response_reference_id: item.response.reference.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            }),
        };

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

// ===== REFUND FLOW STRUCTURES =====

// Refund Request - Based on tech spec, refunds can be with amount or empty body
// Following Pattern 2 from pattern_refund.md - Amount-Required Refunds
#[derive(Debug, Clone, Serialize)]
pub struct GlobalpayRefundRequest {
    pub amount: StringMinorUnit,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GlobalpayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for GlobalpayRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: GlobalpayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &wrapper.router_data;
        let amount = wrapper
            .connector
            .amount_converter
            .convert(item.request.minor_refund_amount, item.request.currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to convert refund amount to StringMinorUnit for GlobalPay \
                         POST /transactions/{id}/refund request"
                            .to_string(),
                    ),
                    suggested_action: None,
                    doc_url: None,
                },
            })?;
        Ok(Self { amount })
    }
}

// Refund Response - Based on tech spec, refund response is similar to transaction response
// The refund endpoint returns a transaction object with status
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlobalpayRefundResponse {
    pub id: String,
    pub status: GlobalpayRefundStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<StringMinorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<common_enums::Currency>,
}

impl TryFrom<ResponseRouterData<GlobalpayRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlobalpayRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let refund_status = RefundStatus::from(item.response.status.clone());

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id.clone(),
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            ..item.router_data
        })
    }
}

// RSync Response - Reuses the same GlobalpayRefundResponse structure
impl TryFrom<ResponseRouterData<GlobalpayRefundResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlobalpayRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let refund_status = RefundStatus::from(item.response.status.clone());

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id.clone(),
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            ..item.router_data
        })
    }
}

// ===== VOID FLOW STRUCTURES =====

// Void Request - Based on tech spec, /transactions/{transaction_id}/reverse endpoint
#[derive(Debug, Clone, Serialize)]
pub struct GlobalpayVoidRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<StringMinorUnit>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GlobalpayRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for GlobalpayVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: GlobalpayRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &wrapper.router_data;
        // Validate that we have a connector transaction ID (required for URL construction)
        if item.request.connector_transaction_id.is_empty() {
            return Err(error_stack::report!(
                IntegrationError::MissingConnectorTransactionID {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "connector_transaction_id is required to construct the \
                             POST /transactions/{id}/reversal URL for GlobalPay Void"
                                .to_string(),
                        ),
                        suggested_action: Some(
                            "Ensure the payment was authorized and a connector_transaction_id \
                             was captured before attempting a void"
                                .to_string(),
                        ),
                        doc_url: None,
                    },
                }
            ));
        }

        // Convert amount from MinorUnit to StringMinorUnit if present
        let amount = item
            .request
            .amount
            .zip(item.request.currency)
            .map(|(amount_value, currency)| {
                wrapper
                    .connector
                    .amount_converter
                    .convert(amount_value, currency)
                    .change_context(IntegrationError::AmountConversionFailed {
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "Failed to convert partial void amount to StringMinorUnit for \
                                 GlobalPay POST /transactions/{id}/reversal request"
                                    .to_string(),
                            ),
                            suggested_action: None,
                            doc_url: None,
                        },
                    })
            })
            .transpose()?;

        Ok(Self { amount })
    }
}

// Void Response - Reuses GlobalpayPaymentsResponse structure
// The response is similar to transaction response with REVERSED status
impl TryFrom<ResponseRouterData<GlobalpayPaymentsResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlobalpayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = match item.response.status.clone() {
            GlobalpayPaymentStatus::Reversed => AttemptStatus::Voided,
            GlobalpayPaymentStatus::Pending
            | GlobalpayPaymentStatus::Initiated
            | GlobalpayPaymentStatus::ForReview => AttemptStatus::Pending,
            GlobalpayPaymentStatus::Declined
            | GlobalpayPaymentStatus::Failed
            | GlobalpayPaymentStatus::Rejected
            | GlobalpayPaymentStatus::Captured
            | GlobalpayPaymentStatus::Preauthorized
            | GlobalpayPaymentStatus::Funded => AttemptStatus::VoidFailed,
        };

        let response = match status {
            AttemptStatus::VoidFailed => Err(ErrorResponse {
                status_code: item.http_code,
                code: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.result.clone())
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.message.clone())
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.message.clone()),
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: Some(item.response.id.clone()),
                network_decline_code: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.result.clone()),
                network_advice_code: None,
                network_error_message: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.message.clone()),
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            }),
            _ => Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: item.response.reference.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            }),
        };

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

// ===== CLIENT AUTHENTICATION TOKEN FLOW STRUCTURES =====

/// Request to obtain an access token for client-side SDK initialization.
/// Uses the /accesstoken endpoint with merchant credentials and permissions
/// for hosted fields integration.
#[derive(Debug, Serialize)]
pub struct GlobalpayClientAuthRequest {
    pub app_id: Secret<String>,
    pub nonce: Secret<String>,
    pub secret: Secret<String>,
    pub grant_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval_to_expire: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restricted_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Vec<String>>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GlobalpayRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                MerchantAuthenticationFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for GlobalpayClientAuthRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: GlobalpayRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                MerchantAuthenticationFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &wrapper.router_data;
        let auth = GlobalpayAuthType::try_from(&item.connector_config)?;

        use sha2::{Digest, Sha512};
        let nonce = rand::distributions::Alphanumeric.sample_string(&mut rand::thread_rng(), 12);
        let secret_input = format!("{}{}", nonce, auth.app_key.peek());
        let mut hasher = Sha512::new();
        hasher.update(secret_input.as_bytes());
        let secret_hex = hex::encode(hasher.finalize());

        let permissions = item.request.permissions.clone();

        Ok(Self {
            app_id: auth.app_id,
            nonce: Secret::new(nonce),
            secret: Secret::new(secret_hex),
            grant_type: "client_credentials".to_string(),
            interval_to_expire: Some("1_HOUR".to_string()),
            restricted_token: Some("YES".to_string()),
            permissions,
        })
    }
}

/// Response from the /accesstoken endpoint for client-side SDK use.
#[derive(Debug, Deserialize, Serialize)]
pub struct GlobalpayClientAuthResponse {
    pub token: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub seconds_to_expire: i64,
}

impl TryFrom<ResponseRouterData<GlobalpayClientAuthResponse, Self>>
    for RouterDataV2<
        ClientAuthenticationToken,
        MerchantAuthenticationFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlobalpayClientAuthResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        let session_data = ClientAuthenticationTokenData::ConnectorSpecific(Box::new(
            ConnectorSpecificClientAuthenticationResponse::Globalpay(
                GlobalpayClientAuthenticationResponseDomain {
                    access_token: Secret::new(response.token),
                    token_type: Some(response.type_),
                    expires_in: Some(response.seconds_to_expire),
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

// ===== SETUP MANDATE FLOW STRUCTURES =====
//
// GlobalPay requires a tokenized payment method (PMT_...) to drive MIT charges
// against the /transactions endpoint (the `payment_method.id` field only
// accepts tokens, never transaction ids). SetupRecurring therefore calls the
// /payment-methods endpoint to tokenize the card with `usage_mode: MULTIPLE`
// and surfaces the returned `PMT_...` id as the connector_mandate_id that
// RepeatPayment later plugs into `payment_method.id`.

/// Initiator enum for SetupMandate - GlobalPay expects a simple string
/// value here, not a nested object (which is what the existing `Initiator`
/// struct serializes to). See HyperSwitch's globalpay reference impl.
#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GlobalpayMandateInitiator {
    Merchant,
    Payer,
}

/// Stored credential model for setup mandate. GlobalPay expects `model`
/// (not `type`) as the field name and only the `model`/`sequence` pair.
#[derive(Debug, Serialize)]
pub struct GlobalpayMandateStoredCredential {
    pub model: GlobalpayStoredCredentialModel,
    pub sequence: StoredCredentialSequence,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GlobalpayStoredCredentialModel {
    Recurring,
    Subscription,
    Unscheduled,
    Installment,
}

#[derive(Debug, Serialize)]
pub struct GlobalpaySetupMandateCard<T: PaymentMethodDataTypes> {
    pub number: RawCardNumber<T>,
    pub expiry_month: Secret<String>,
    pub expiry_year: Secret<String>,
    pub cvv: Secret<String>,
}

/// Tokenization request sent to `/payment-methods`. `usage_mode: MULTIPLE`
/// allows the returned PMT_ id to be reused by subsequent MIT charges.
#[derive(Debug, Serialize)]
pub struct GlobalpaySetupMandateRequest<T: PaymentMethodDataTypes> {
    pub reference: String,
    pub usage_mode: GlobalpayUsageMode,
    pub card: GlobalpaySetupMandateCard<T>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GlobalpayRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for GlobalpaySetupMandateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: GlobalpayRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &wrapper.router_data;

        let card = match &item.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                let expiry_year_2digit = card_data.get_card_expiry_year_2_digit().change_context(
                    IntegrationError::RequestEncodingFailed {
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "Failed to convert card expiry year to 2-digit format for \
                                 GlobalPay SetupMandate POST /payment-methods request"
                                    .to_string(),
                            ),
                            suggested_action: None,
                            doc_url: None,
                        },
                    },
                )?;
                // GlobalPay's /payment-methods endpoint requires CVV for card
                // tokenization; unlike the Authorize flow there is no
                // `cvv_indicator` fallback, so an empty CVV would surface as an
                // opaque connector-side rejection. Fail fast instead.
                if card_data.card_cvc.peek().is_empty() {
                    return Err(error_stack::report!(
                        IntegrationError::MissingRequiredField {
                            field_name: "card_cvc",
                            context: IntegrationErrorContext {
                                additional_context: Some(
                                    "GlobalPay POST /payment-methods requires CVV for card \
                                     tokenization; an empty CVV causes a connector-side rejection"
                                        .to_string(),
                                ),
                                suggested_action: Some(
                                    "Provide a valid CVV value in the card payment method data"
                                        .to_string(),
                                ),
                                doc_url: None,
                            },
                        }
                    ));
                }
                GlobalpaySetupMandateCard {
                    number: card_data.card_number.clone(),
                    expiry_month: card_data.card_exp_month.clone(),
                    expiry_year: expiry_year_2digit,
                    cvv: card_data.card_cvc.clone(),
                }
            }
            _ => {
                return Err(error_stack::report!(IntegrationError::NotImplemented(
                    "Payment method not supported for SetupMandate".to_string(),
                    IntegrationErrorContext {
                        additional_context: Some(
                            "GlobalPay SetupMandate POST /payment-methods only supports card \
                             payment methods; received an unsupported payment method type"
                                .to_string(),
                        ),
                        suggested_action: None,
                        doc_url: None,
                    },
                )))
            }
        };

        Ok(Self {
            reference: item
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            usage_mode: GlobalpayUsageMode::Multiple,
            card,
        })
    }
}

// SetupMandate response: a 2xx from /payment-methods returns a PMT_ id which
// becomes the connector_mandate_id used later by RepeatPayment (as
// `payment_method.id` on /transactions). GlobalPay's /payment-methods response
// has no status field - a successful parse implies tokenization succeeded.
impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<GlobalpaySetupMandateResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlobalpaySetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // The PMT_ id is a payment-method token, not a transaction id, so PSync
        // (which hits /transactions/{id}) cannot be performed against it. We
        // surface the PMT_ id through MandateReference.connector_mandate_id for
        // later RepeatPayment use and leave resource_id as NoResponseId.
        let mandate_reference = Some(Box::new(MandateReference {
            connector_mandate_id: Some(item.response.id.clone()),
            payment_method_id: None,
            connector_mandate_request_reference_id: None,
            mandate_metadata: None,
        }));

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::NoResponseId,
                redirection_data: None,
                mandate_reference,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: item.response.reference.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::Charged,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== REPEAT PAYMENT (MIT) FLOW STRUCTURES =====
//
// GlobalPay MIT charges reuse the `/transactions` endpoint. The stored credential
// is referenced by putting the connector_mandate_id (transaction id from the prior
// SetupMandate) into `payment_method.id`, with initiator=MERCHANT and
// stored_credential={ model: RECURRING, sequence: SUBSEQUENT }.

/// Response type for RepeatPayment flow - reuses GlobalpayPaymentsResponse
pub type GlobalpayRepeatPaymentResponse = GlobalpayPaymentsResponse;

/// Payment method body for MIT - references the stored transaction by id.
#[derive(Debug, Serialize)]
pub struct GlobalpayRepeatPaymentMethod {
    pub entry_mode: String,
    pub id: String,
}

#[derive(Debug, Serialize)]
pub struct GlobalpayRepeatPaymentRequest {
    pub account_name: String,
    #[serde(rename = "type")]
    pub type_: GlobalpayTransactionType,
    pub channel: String,
    pub amount: StringMinorUnit,
    pub currency: common_enums::Currency,
    pub reference: String,
    pub country: common_enums::CountryAlpha2,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_mode: Option<GlobalpayCaptureMode>,
    pub initiator: GlobalpayMandateInitiator,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notifications: Option<GlobalpayNotifications>,
    pub stored_credential: GlobalpayMandateStoredCredential,
    pub payment_method: GlobalpayRepeatPaymentMethod,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GlobalpayRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for GlobalpayRepeatPaymentRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: GlobalpayRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &wrapper.router_data;

        let auth = GlobalpayAuthType::try_from(&item.connector_config)?;
        let account_name = auth
            .account_name
            .ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                    field_name: "account_name",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "GlobalPay requires account_name in connector config to identify the \
                             processing account for MIT POST /transactions"
                                .to_string(),
                        ),
                        suggested_action: Some(
                            "Set account_name in the GlobalPay connector configuration".to_string(),
                        ),
                        doc_url: None,
                    },
                })
            })?
            .peek()
            .to_string();

        let mandate_id = match &item.request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(connector_mandate_ref) => connector_mandate_ref
                .get_connector_mandate_id()
                .ok_or_else(|| {
                    error_stack::report!(IntegrationError::MissingRequiredField {
                        field_name: "connector_mandate_id",
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "connector_mandate_id (PMT_ token from a prior SetupMandate) is \
                                 required as payment_method.id for GlobalPay MIT charges via \
                                 POST /transactions"
                                    .to_string(),
                            ),
                            suggested_action: Some(
                                "Ensure a SetupMandate was completed and the resulting PMT_ token \
                                 is stored as connector_mandate_id before initiating RepeatPayment"
                                    .to_string(),
                            ),
                            doc_url: None,
                        },
                    })
                })?,
            MandateReferenceId::NetworkMandateId(_)
            | MandateReferenceId::NetworkTokenWithNTI(_) => {
                return Err(error_stack::report!(IntegrationError::NotImplemented(
                    "Network mandate id not supported for GlobalPay RepeatPayment".to_string(),
                    IntegrationErrorContext {
                        additional_context: Some(
                            "GlobalPay RepeatPayment requires a PMT_ connector mandate id; \
                             network mandate ids and network token NTIs are not supported"
                                .to_string(),
                        ),
                        suggested_action: None,
                        doc_url: None,
                    },
                )));
            }
        };

        let country = item.resource_common_data.get_billing_country()?;

        let notifications = if let Some(webhook_url) = item.request.webhook_url.as_ref() {
            let return_url = item
                .request
                .router_return_url
                .clone()
                .unwrap_or_else(|| webhook_url.clone());
            Some(GlobalpayNotifications {
                cancel_url: return_url.clone(),
                return_url,
                status_url: webhook_url.clone(),
            })
        } else {
            None
        };

        let capture_mode = match item.request.capture_method {
            Some(common_enums::CaptureMethod::Manual) => Some(GlobalpayCaptureMode::Later),
            _ => Some(GlobalpayCaptureMode::Auto),
        };

        let amount = wrapper
            .connector
            .amount_converter
            .convert(item.request.minor_amount, item.request.currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to convert RepeatPayment amount to StringMinorUnit for GlobalPay \
                         MIT POST /transactions request"
                            .to_string(),
                    ),
                    suggested_action: None,
                    doc_url: None,
                },
            })?;

        Ok(Self {
            account_name,
            type_: GlobalpayTransactionType::Sale,
            channel: constants::CHANNEL_CNP.to_string(),
            amount,
            currency: item.request.currency,
            reference: item
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            country,
            capture_mode,
            initiator: GlobalpayMandateInitiator::Merchant,
            notifications,
            stored_credential: GlobalpayMandateStoredCredential {
                model: GlobalpayStoredCredentialModel::Recurring,
                sequence: StoredCredentialSequence::Subsequent,
            },
            payment_method: GlobalpayRepeatPaymentMethod {
                entry_mode: constants::ENTRY_MODE_ECOM.to_string(),
                id: mandate_id,
            },
        })
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<GlobalpayRepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlobalpayRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::from(item.response.status.clone());

        let network_txn_id = item
            .response
            .payment_method
            .as_ref()
            .and_then(|pm| pm.card.as_ref())
            .and_then(|card| card.brand_reference.as_ref())
            .map(|s| s.peek().to_string());

        let response = match status {
            AttemptStatus::Failure => Err(ErrorResponse {
                status_code: item.http_code,
                code: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.result.clone())
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.message.clone())
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.message.clone()),
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: Some(item.response.id.clone()),
                network_decline_code: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.result.clone()),
                network_advice_code: None,
                network_error_message: item
                    .response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.message.clone()),
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            }),
            _ => Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id,
                network_txn_link_id: None,
                connector_response_reference_id: item.response.reference.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            }),
        };

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

// ===== PAYMENT METHOD TOKEN FLOW STRUCTURES =====
//
// GlobalPay's /payment-methods endpoint stores a card for future off-session
// (MIT) use and returns a PMT_ token. This flow is the standalone tokenization
// equivalent of SetupMandate — same endpoint, same payload shape, but triggered
// as a discrete pre-authorize step rather than bundled with a CIT charge.

#[derive(Debug, Serialize)]
pub struct GlobalpayPaymentMethodTokenRequest<T: PaymentMethodDataTypes> {
    pub reference: String,
    pub usage_mode: GlobalpayUsageMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<GlobalpaySetupMandateCard<T>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GlobalpayPaymentMethodTokenResponse {
    pub id: Option<String>,
    pub card: Option<GlobalpayPaymentMethodTokenCard>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GlobalpayPaymentMethodTokenCard {
    pub brand_reference: Option<Secret<String>>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GlobalpayRouterData<
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
            T,
        >,
    > for GlobalpayPaymentMethodTokenRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: GlobalpayRouterData<
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &wrapper.router_data;
        let request = &item.request;

        if !request.is_customer_initiated_mandate_payment() {
            return Err(error_stack::report!(IntegrationError::NotImplemented(
                "GlobalPay PaymentMethodToken only supports mandate/recurring tokenization"
                    .to_string(),
                IntegrationErrorContext {
                    additional_context: Some(
                        "Both a mandate signal (customer_acceptance or setup_mandate_details) \
                         and setup_future_usage=OffSession are required to tokenize a card via \
                         GlobalPay POST /payment-methods"
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Set setup_future_usage to OffSession and provide either \
                         customer_acceptance or setup_mandate_details"
                            .to_string(),
                    ),
                    doc_url: None,
                },
            )));
        }

        let card = match &request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                let expiry_year_2digit = card_data.get_card_expiry_year_2_digit().change_context(
                    IntegrationError::RequestEncodingFailed {
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "Failed to convert card expiry year to 2-digit format for \
                                 GlobalPay PaymentMethodToken POST /payment-methods request"
                                    .to_string(),
                            ),
                            suggested_action: None,
                            doc_url: None,
                        },
                    },
                )?;
                if card_data.card_cvc.peek().is_empty() {
                    return Err(error_stack::report!(
                        IntegrationError::MissingRequiredField {
                            field_name: "card_cvc",
                            context: IntegrationErrorContext {
                                additional_context: Some(
                                    "GlobalPay POST /payment-methods requires CVV for card \
                                     tokenization; an empty CVV causes a connector-side rejection"
                                        .to_string(),
                                ),
                                suggested_action: Some(
                                    "Provide a valid CVV value in the card payment method data"
                                        .to_string(),
                                ),
                                doc_url: None,
                            },
                        }
                    ));
                }
                Some(GlobalpaySetupMandateCard {
                    number: card_data.card_number.clone(),
                    expiry_month: card_data.card_exp_month.clone(),
                    expiry_year: expiry_year_2digit,
                    cvv: card_data.card_cvc.clone(),
                })
            }
            _ => None,
        };

        Ok(Self {
            reference: item
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            usage_mode: GlobalpayUsageMode::Multiple,
            card,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<GlobalpayPaymentMethodTokenResponse, Self>>
    for RouterDataV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlobalpayPaymentMethodTokenResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let token = item.response.id.clone().unwrap_or_default();
        Ok(Self {
            response: Ok(PaymentMethodTokenResponse {
                token,
                connector_payment_method_id: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}
