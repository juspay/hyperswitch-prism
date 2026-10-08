use base64::Engine;
use common_utils::{
    ext_traits::OptionExt, pii::Email, request::Method, types::MinorUnit, FloatMajorUnit,
    StringMajorUnit,
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, CreateOrder, RepeatPayment, SetupMandate,
    },
    connector_types::{
        ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData,
        ConnectorSpecificClientAuthenticationResponse, MandateReference, MandateReferenceId,
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsResponseData,
        RapydClientAuthenticationResponse as RapydClientAuthenticationResponseDomain,
        RefundFlowData, RefundsData, RefundsResponseData, RepeatPaymentData, ResponseId,
        SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::{
        GpayTokenizationData, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber, WalletData,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::Deserialize;
use serde::Serialize;
use std::fmt::Debug;
use url::Url;

use crate::types::ResponseRouterData;

use super::RapydRouterData;

/// Rapyd digital-wallet `payment_type` values.
const WALLET_TYPE_GOOGLE_PAY: &str = "google_pay";
const WALLET_TYPE_APPLE_PAY: &str = "apple_pay";

/// Apple Pay `paymentDataType` — Apple's PKPaymentToken spec defines exactly
/// `3DSecure` and `EMV`. The decrypted network-token path is always `3DSecure`.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum RapydApplePayPaymentDataType {
    #[serde(rename = "3DSecure")]
    ThreeDSecure,
    #[serde(rename = "EMV")]
    Emv,
}

/// Google Pay decrypted `payment_method` discriminator.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum RapydGooglePayPaymentMethod {
    #[serde(rename = "CARD")]
    Card,
}

/// Google Pay decrypted `auth_method`: cryptogram present vs PAN-only.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum RapydGooglePayAuthMethod {
    #[serde(rename = "CRYPTOGRAM_3DS")]
    Cryptogram3ds,
    #[serde(rename = "PAN_ONLY")]
    PanOnly,
}

/// Card funding for the wallet `brand_data.type`.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RapydCardFunding {
    Credit,
    Debit,
    Prepaid,
}

impl TryFrom<&str> for RapydCardFunding {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "credit" => Ok(Self::Credit),
            "debit" => Ok(Self::Debit),
            "prepaid" => Ok(Self::Prepaid),
            other => Err(IntegrationError::NotSupported {
                message: format!("rapyd wallet card funding: {other}"),
                connector: "rapyd",
                context: Default::default(),
            })?,
        }
    }
}

/// Serialize a currency as its ISO-4217 numeric code (e.g. "978" for EUR),
/// which is the form Rapyd's decrypted Apple Pay `currencyCode` expects.
fn serialize_currency_as_numeric<S>(
    currency: &common_enums::Currency,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(currency.iso_4217())
}

/// Parse a wallet's card-network string (Apple sends `"Visa"`, Google `"VISA"`)
/// into `CardNetwork`. Serde aliases cover the casing differences.
fn parse_card_network(
    network: &str,
) -> Result<common_enums::CardNetwork, error_stack::Report<IntegrationError>> {
    serde_json::from_value(serde_json::Value::String(network.to_string())).change_context(
        IntegrationError::NotSupported {
            message: format!("rapyd wallet card network: {network}"),
            connector: "rapyd",
            context: Default::default(),
        },
    )
}

/// Rapyd `payment_method.type` identifier. Rapyd's types are country-prefixed
/// (`<country>_<network>_card`) and enumerated per-country by the List Payment
/// Methods endpoint; this connector covers the India (`in_`) set.
///
/// Raw card payments use `InAmexCard` as a fixed placeholder: resolving the
/// correct per-country/per-funding type from a bare PAN is not implemented yet,
/// and the sandbox merchant accepts `in_amex_card`. Digital wallets carry their
/// network + funding, so they derive the funding-specific type
/// (`in_credit_visa_card` / `in_debit_visa_card`, etc.) via `try_from_wallet_network`.
///
/// Reference: https://docs.rapyd.net/en/list-payment-methods-by-country.html
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RapydPaymentMethodType {
    InAmexCard,
    InCreditVisaCard,
    InDebitVisaCard,
    InCreditMastercardCard,
    InDebitMastercardCard,
}

impl RapydPaymentMethodType {
    /// Resolve the Rapyd `payment_method.type` for a digital-wallet card from
    /// its network and funding. Wallets carry only the network and credit/debit
    /// funding (the PAN lives in the decrypted payload), so the type is derived
    /// from those.
    fn try_from_wallet_network(
        network: &str,
        card_type: Option<&str>,
    ) -> Result<Self, error_stack::Report<IntegrationError>> {
        let is_debit = matches!(card_type.map(str::to_lowercase).as_deref(), Some("debit"));
        match network.to_lowercase().as_str() {
            "visa" if is_debit => Ok(Self::InDebitVisaCard),
            "visa" => Ok(Self::InCreditVisaCard),
            "mastercard" | "master" if is_debit => Ok(Self::InDebitMastercardCard),
            "mastercard" | "master" => Ok(Self::InCreditMastercardCard),
            "amex" | "americanexpress" | "american express" => Ok(Self::InAmexCard),
            other => Err(IntegrationError::NotSupported {
                message: format!("rapyd wallet card network: {other}"),
                connector: "rapyd",
                context: Default::default(),
            })?,
        }
    }
}

/// Rapyd `initiation_type` for `/v1/payments`. It follows the
/// `recurrence_type` the card was saved with: SetupMandate sends none, so the
/// card is saved with the default `unscheduled`, and a merchant-initiated
/// charge of it goes out as `unscheduled`. The other documented values
/// (`customer_present`, `recurring`, `installment`, `moto`, ...) are not
/// sent on any path.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RapydInitiationType {
    Unscheduled,
}

/// The `capture` flag of a mandate setup or a saved-card charge. Rapyd's
/// Create Payment has one boolean `capture`, so a capture method it cannot
/// express is refused instead of being folded into one of the two values.
fn mandate_capture_flag(
    capture_method: Option<common_enums::CaptureMethod>,
) -> Result<bool, error_stack::Report<IntegrationError>> {
    match capture_method {
        Some(common_enums::CaptureMethod::Automatic)
        | Some(common_enums::CaptureMethod::SequentialAutomatic)
        | None => Ok(true),
        Some(common_enums::CaptureMethod::Manual) => Ok(false),
        Some(
            method @ (common_enums::CaptureMethod::ManualMultiple
            | common_enums::CaptureMethod::Scheduled),
        ) => Err(IntegrationError::NotSupported {
            message: format!("capture method {method:?}"),
            connector: "rapyd",
            context: crate::utils::integration_ctx(
                "Rapyd Create Payment takes a single boolean `capture`; scheduled and multiple manual captures have no value",
                "Use capture_method automatic, sequential_automatic or manual.",
            ),
        })?,
    }
}

impl<F, T> TryFrom<ResponseRouterData<RapydPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<RapydPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Rapyd sends empty strings where a value is absent (`failure_code: ""`);
        // an empty value is never reported as the error code, message or reason.
        let non_empty = |value: Option<&String>| value.filter(|v| !v.is_empty()).cloned();
        let envelope_code = non_empty(Some(&item.response.status.error_code));
        let envelope_message = non_empty(item.response.status.status.as_ref())
            .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string());

        let (status, response) = match &item.response.data {
            Some(data) => {
                let attempt_status = get_payment_status(data);
                if attempt_status == common_enums::AttemptStatus::Failure {
                    (
                        common_enums::AttemptStatus::Failure,
                        Err(ErrorResponse {
                            code: non_empty(data.failure_code.as_ref())
                                .or(envelope_code)
                                .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                            status_code: item.http_code,
                            message: envelope_message,
                            reason: non_empty(data.failure_message.as_ref()),
                            attempt_status: None,
                            connector_transaction_id: Some(data.id.clone()),
                            network_advice_code: None,
                            network_decline_code: None,
                            network_error_message: None,
                            typed_connector_response: None,
                            raw_connector_response: None,
                            raw_connector_request: None,
                            typed_connector_request: None,
                        }),
                    )
                } else {
                    let redirection_url = data
                        .redirect_url
                        .as_ref()
                        .filter(|redirect_str| !redirect_str.is_empty())
                        .map(|url| {
                            Url::parse(url).change_context(
                                crate::utils::response_handling_fail_for_connector(
                                    item.http_code,
                                    "rapyd",
                                ),
                            )
                        })
                        .transpose()?;

                    let redirection_data =
                        redirection_url.map(|url| RedirectForm::from((url, Method::Get)));

                    // The saved-card token Rapyd returns on a CIT save is the
                    // mandate reference for later MIT replays. Rapyd charges
                    // the saved card without a customer id, so nothing else
                    // needs to round-trip.
                    let mandate_reference = data.payment_method.as_ref().map(|card| {
                        Box::new(MandateReference {
                            connector_mandate_id: Some(card.clone()),
                            payment_method_id: None,
                            connector_mandate_request_reference_id: None,
                            mandate_metadata: None,
                        })
                    });
                    let network_txn_id = data
                        .payment_method_data
                        .as_ref()
                        .and_then(|pmd| pmd.network_reference_id.clone())
                        .map(|nti| nti.expose());

                    (
                        attempt_status,
                        Ok(PaymentsResponseData::TransactionResponse {
                            resource_id: ResponseId::ConnectorTransactionId(data.id.to_owned()), //transaction_id is also the field but this id is used to initiate a refund
                            redirection_data: redirection_data.map(Box::new),
                            mandate_reference,
                            connector_metadata: None,
                            network_txn_id,
                            network_txn_link_id: None,
                            connector_response_reference_id: data.merchant_reference_id.to_owned(),
                            incremental_authorization_allowed: None,
                            status_code: item.http_code,
                            splits: None,
                            payment_account_reference: None,
                        }),
                    )
                }
            }
            // A 2xx without `data` says nothing about the payment: the error is
            // reported, the status the request carried is left as it was (this
            // mapper also serves PSync, Capture and Void).
            None => (
                item.router_data.resource_common_data.status,
                Err(ErrorResponse {
                    code: envelope_code
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                    status_code: item.http_code,
                    message: envelope_message,
                    reason: non_empty(item.response.status.message.as_ref()),
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
            ),
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

// RapydRouterData is now generated by the macro in rapyd.rs

#[derive(Debug, Serialize)]
pub struct RapydAuthType {
    pub(super) access_key: Secret<String>,
    pub(super) secret_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for RapydAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Rapyd {
                access_key,
                secret_key,
                ..
            } => Ok(Self {
                access_key: access_key.to_owned(),
                secret_key: secret_key.to_owned(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            })?,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct RapydPaymentsRequest<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
> {
    // Major-unit string amount. A zero-amount card verification is sent as
    // `"0"` (via `StringMajorUnit::zero`); Rapyd rejects `"0.00"`.
    pub amount: StringMajorUnit,
    pub currency: common_enums::Currency,
    pub payment_method: RapydPaymentMethodData<T>,
    pub payment_method_options: Option<PaymentMethodOptions>,
    pub merchant_reference_id: Option<String>,
    pub capture: Option<bool>,
    pub description: Option<String>,
    pub complete_payment_url: Option<String>,
    pub error_payment_url: Option<String>,
    /// Rapyd customer — may be either a string id (`cus_*`, for MIT)
    /// or an inline object `{ name, email }` (for SetupMandate, so that
    /// Rapyd creates the customer alongside the payment and issues a
    /// customer-scoped `card_*` token in the response).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer: Option<RapydCustomerRef>,
    /// When true and `payment_method` carries card fields, Rapyd saves
    /// the card under the customer and returns a reusable `card_*` id.
    /// Must be paired with `customer`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save_payment_method: Option<bool>,
    /// Required on MIT replays so Rapyd bypasses 3DS using the stored
    /// credential. Rapyd's vocabulary also includes `customer_present`,
    /// `installment`, `moto`, and `unscheduled` — only `recurring` is
    /// emitted on the current MIT path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initiation_type: Option<RapydInitiationType>,
}

/// Rapyd customer reference: either a raw id string (`cus_*`) for MIT
/// replay, or an inline `{name, email}` object when we want Rapyd to
/// create the customer alongside the payment.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum RapydCustomerRef {
    Id(String),
    Inline(RapydInlineCustomer),
}

/// Inline customer object embedded in a `/v1/payments` call. Both fields
/// are required by Rapyd when `save_payment_method: true` — without them
/// Rapyd cannot mint a `cus_*` to attach the saved `card_*` to. The
/// SetupMandate transformer enforces presence at the request level, so
/// this struct never produces an empty `{}` body.
/// Reference: https://docs.rapyd.net/en/create-customer.html
#[derive(Debug, Serialize)]
pub struct RapydInlineCustomer {
    pub name: Secret<String>,
    pub email: Email,
}

/// Rapyd payment_method field can be either a token string (for saved/tokenized
/// payment methods) or a full payment method object (for new card / wallet).
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum RapydPaymentMethodData<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
> {
    Token(Secret<String>),
    PaymentMethod(Box<PaymentMethod<T>>),
}

#[derive(Debug, Serialize)]
pub struct PaymentMethodOptions {
    #[serde(rename = "3d_required")]
    pub three_ds: bool,
}

#[derive(Debug, Serialize)]
pub struct PaymentMethod<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> {
    #[serde(rename = "type")]
    pub pm_type: RapydPaymentMethodType,
    pub fields: Option<PaymentFields<T>>,
    pub address: Option<Address>,
    pub digital_wallet: Option<RapydWallet>,
}

#[derive(Default, Debug, Serialize)]
pub struct PaymentFields<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> {
    pub number: RawCardNumber<T>,
    pub expiration_month: Secret<String>,
    pub expiration_year: Secret<String>,
    pub name: Secret<String>,
    pub cvv: Secret<String>,
}

#[derive(Default, Debug, Serialize)]
pub struct Address {
    name: Secret<String>,
    line_1: Secret<String>,
    line_2: Option<Secret<String>>,
    line_3: Option<Secret<String>>,
    city: Option<String>,
    state: Option<Secret<String>>,
    country: Option<String>,
    zip: Option<Secret<String>>,
    phone_number: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RapydWallet {
    #[serde(rename = "type")]
    payment_type: String,
    details: RapydWalletDetails,
}

/// Rapyd's `digital_wallet.details` is an object in every form: either the
/// wallet's encrypted token object under `token` (Rapyd decrypts server-side)
/// or a decrypted payload the merchant already decrypted with its own keys
/// (`decrypted_data` + `brand_data`). The request decides which one is sent.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum RapydWalletDetails {
    ApplePayDecrypted(Box<RapydApplePayDecryptedDetails>),
    GooglePayDecrypted(Box<RapydGooglePayDecryptedDetails>),
    ApplePayEncrypted(Box<RapydApplePayEncryptedDetails>),
    GooglePayEncrypted(Box<RapydGooglePayEncryptedDetails>),
}

/// Google Pay API version of the token object Rapyd documents (`apiVersion` 2,
/// `apiVersionMinor` 0).
const GOOGLE_PAY_API_VERSION: u8 = 2;
const GOOGLE_PAY_API_VERSION_MINOR: u8 = 0;

/// Google Pay `tokenizationData.type`; Rapyd documents `PAYMENT_GATEWAY` only.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum RapydGooglePayTokenizationType {
    #[serde(rename = "PAYMENT_GATEWAY")]
    PaymentGateway,
}

/// Encrypted Google Pay: `{ "token": <Google Pay token object>, "type": "CARD" }`.
#[derive(Debug, Clone, Serialize)]
pub struct RapydGooglePayEncryptedDetails {
    token: RapydGooglePayToken,
    #[serde(rename = "type")]
    payment_method: RapydGooglePayPaymentMethod,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RapydGooglePayToken {
    api_version: u8,
    api_version_minor: u8,
    payment_method_data: RapydGooglePayTokenPaymentMethodData,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RapydGooglePayTokenPaymentMethodData {
    description: String,
    tokenization_data: RapydGooglePayTokenizationData,
    info: RapydGooglePayTokenInfo,
}

#[derive(Debug, Clone, Serialize)]
pub struct RapydGooglePayTokenizationData {
    #[serde(rename = "type")]
    token_type: RapydGooglePayTokenizationType,
    /// The encrypted token string Google Pay issued, unmodified.
    token: Secret<String>,
}

/// `info` of the Google Pay token, forwarded as the wallet sent it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RapydGooglePayTokenInfo {
    card_network: String,
    card_details: String,
}

/// Encrypted Apple Pay: `{ "token": <Apple Pay token object> }`.
#[derive(Debug, Clone, Serialize)]
pub struct RapydApplePayEncryptedDetails {
    token: RapydApplePayToken,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RapydApplePayToken {
    /// Apple's encrypted `paymentData` object (`data`, `signature`, `header`,
    /// `version`), forwarded unmodified.
    payment_data: Secret<serde_json::Value>,
    payment_method: RapydApplePayTokenPaymentMethod,
    transaction_identifier: String,
}

/// `paymentMethod` of the Apple Pay token, forwarded as the wallet sent it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RapydApplePayTokenPaymentMethod {
    display_name: String,
    network: String,
    #[serde(rename = "type")]
    pm_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RapydApplePayDecryptedDetails {
    decrypted_data: RapydApplePayDecryptedData,
    brand_data: RapydApplePayBrandData,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RapydApplePayDecryptedData {
    application_primary_account_number: cards::CardNumber,
    application_expiration_date: Secret<String>,
    /// ISO-4217 numeric currency code (e.g. "978" for EUR), per Apple/Rapyd.
    #[serde(serialize_with = "serialize_currency_as_numeric")]
    currency_code: common_enums::Currency,
    transaction_amount: MinorUnit,
    payment_data_type: RapydApplePayPaymentDataType,
    payment_data: RapydApplePayCryptogram,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RapydApplePayCryptogram {
    online_payment_cryptogram: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    eci_indicator: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RapydApplePayBrandData {
    display_name: String,
    network: common_enums::CardNetwork,
    #[serde(rename = "type")]
    card_type: RapydCardFunding,
}

#[derive(Debug, Clone, Serialize)]
pub struct RapydGooglePayDecryptedDetails {
    decrypted_data: RapydGooglePayDecryptedData,
    brand_data: RapydGooglePayBrandData,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RapydGooglePayDecryptedData {
    gateway_merchant_id: Secret<String>,
    payment_method: RapydGooglePayPaymentMethod,
    payment_method_details: RapydGooglePayMethodDetails,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RapydGooglePayMethodDetails {
    expiration_year: Secret<String>,
    expiration_month: Secret<String>,
    pan: cards::CardNumber,
    auth_method: RapydGooglePayAuthMethod,
    #[serde(skip_serializing_if = "Option::is_none")]
    eci_indicator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cryptogram: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RapydGooglePayBrandData {
    card_details: String,
    card_network: common_enums::CardNetwork,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        RapydRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for RapydPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: RapydRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let return_url = item.router_data.request.get_router_return_url()?;
        // Rapyd's Create Payment has one boolean `capture`. A capture method it
        // cannot express is refused here, before any payment-method data is
        // read, so the refusal covers every arm (card, wallets, token).
        let capture = match item.router_data.request.capture_method {
            Some(common_enums::CaptureMethod::Automatic)
            | Some(common_enums::CaptureMethod::SequentialAutomatic)
            | None => Some(true),
            Some(common_enums::CaptureMethod::Manual) => Some(false),
            Some(
                method @ (common_enums::CaptureMethod::ManualMultiple
                | common_enums::CaptureMethod::Scheduled),
            ) => Err(IntegrationError::NotSupported {
                message: format!("capture method {method:?}"),
                connector: "rapyd",
                context: crate::utils::integration_ctx(
                    "Rapyd Create Payment takes a single boolean `capture`; scheduled and multiple manual captures have no value",
                    "Use capture_method automatic, sequential_automatic or manual.",
                ),
            })?,
        };
        // Authorize always sends the real transaction amount. Zero-amount card
        // verification is the SetupMandate flow's responsibility (hyperswitch
        // routes `amount == 0 && setup_future_usage` there), not Authorize.
        let amount = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.minor_amount,
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: crate::utils::amount_conversion_ctx(
                    "rapyd authorize",
                    &item.router_data.request.minor_amount,
                    &item.router_data.request.currency,
                ),
            })?;

        // The `three_ds` option is card-only (wallets carry their own
        // authentication).
        let payment_method_options = matches!(
            item.router_data.resource_common_data.payment_method,
            common_enums::PaymentMethod::Card
        )
        .then(|| PaymentMethodOptions {
            three_ds: matches!(
                item.router_data.resource_common_data.auth_type,
                common_enums::AuthenticationType::ThreeDs
            ),
        });
        let payment_method = match item.router_data.request.payment_method_data {
            PaymentMethodData::Card(ref ccard) => {
                // Rapyd rejects an empty `fields.name` (INVALID_HOLDER_NAME):
                // the cardholder name, else the billing name, else refuse.
                let is_not_blank = |name: &Secret<String>| !name.peek().trim().is_empty();
                let card_holder_name = ccard
                    .card_holder_name
                    .clone()
                    .filter(is_not_blank)
                    .or_else(|| {
                        item.router_data
                            .resource_common_data
                            .get_optional_billing_full_name()
                            .filter(is_not_blank)
                    })
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "payment_method.card.card_holder_name",
                        context: crate::utils::integration_ctx(
                            "Rapyd requires a non-empty cardholder name in `payment_method.fields.name`; the request carries neither a cardholder name nor a billing name",
                            "Provide `payment_method.card.card_holder_name` or the billing first and last name.",
                        ),
                    })?;
                Some(RapydPaymentMethodData::PaymentMethod(Box::new(
                    PaymentMethod {
                        // Placeholder India type. Rapyd's valid `payment_method.type`
                        // is country- and merchant-specific (this sandbox enables
                        // `in_amex_card`, not plain `in_visa_card`), so the correct
                        // per-network/funding mapping is deferred; sandbox is lenient
                        // about the card BIN.
                        pm_type: RapydPaymentMethodType::InAmexCard,
                        fields: Some(PaymentFields {
                            number: ccard.card_number.to_owned(),
                            expiration_month: ccard.card_exp_month.to_owned(),
                            expiration_year: ccard.card_exp_year.to_owned(),
                            name: card_holder_name,
                            cvv: ccard.card_cvc.to_owned(),
                        }),
                        address: None,
                        digital_wallet: None,
                    },
                )))
            }
            PaymentMethodData::Wallet(ref wallet_data) => {
                let (rapyd_wallet, pm_type) = match wallet_data {
                    WalletData::GooglePay(data) => {
                        let details = match &data.tokenization_data {
                            GpayTokenizationData::Decrypted(decrypt_data) => {
                                // Hyperswitch decrypted the Google Pay payload; forward the
                                // decrypted card + cryptogram as Rapyd's `decrypted_data`.
                                let auth =
                                    RapydAuthType::try_from(&item.router_data.connector_config)?;
                                let auth_method = if decrypt_data.cryptogram.is_some() {
                                    RapydGooglePayAuthMethod::Cryptogram3ds
                                } else {
                                    RapydGooglePayAuthMethod::PanOnly
                                };
                                RapydWalletDetails::GooglePayDecrypted(Box::new(
                                    RapydGooglePayDecryptedDetails {
                                        decrypted_data: RapydGooglePayDecryptedData {
                                            gateway_merchant_id: auth.access_key,
                                            payment_method: RapydGooglePayPaymentMethod::Card,
                                            payment_method_details: RapydGooglePayMethodDetails {
                                                expiration_year: decrypt_data
                                                    .get_four_digit_expiry_year()
                                                    .change_context(
                                                        IntegrationError::MissingRequiredField {
                                                            field_name: "gpay expiration_year",
                                                            context: crate::utils::integration_ctx(
                                                                "Google Pay decrypted token has no 4-digit expiry year",
                                                                "Ensure the Google Pay payload was decrypted with the card expiry.",
                                                            ),
                                                        },
                                                    )?,
                                                expiration_month: decrypt_data
                                                    .get_expiry_month()
                                                    .change_context(
                                                    IntegrationError::MissingRequiredField {
                                                        field_name: "gpay expiration_month",
                                                        context: crate::utils::integration_ctx(
                                                            "Google Pay decrypted token has no expiry month",
                                                            "Ensure the Google Pay payload was decrypted with the card expiry.",
                                                        ),
                                                    },
                                                )?,
                                                pan: decrypt_data
                                                    .application_primary_account_number
                                                    .clone(),
                                                auth_method,
                                                eci_indicator: decrypt_data.eci_indicator.clone(),
                                                cryptogram: decrypt_data.cryptogram.clone(),
                                            },
                                        },
                                        brand_data: RapydGooglePayBrandData {
                                            card_details: decrypt_data
                                                .application_primary_account_number
                                                .get_last4(),
                                            card_network: parse_card_network(
                                                &data.info.card_network,
                                            )?,
                                        },
                                    },
                                ))
                            }
                            GpayTokenizationData::Encrypted(_) => {
                                // Rapyd decrypts: forward the Google Pay token
                                // object under `details.token`, with `type: CARD`.
                                let token = data
                                    .tokenization_data
                                    .get_encrypted_google_pay_token()
                                    .change_context(IntegrationError::MissingRequiredField {
                                        field_name: "gpay wallet_token",
                                        context: crate::utils::integration_ctx(
                                            "Encrypted Google Pay payload has no wallet token",
                                            "Ensure the Google Pay tokenization data includes the token.",
                                        ),
                                    })?;
                                RapydWalletDetails::GooglePayEncrypted(Box::new(
                                    RapydGooglePayEncryptedDetails {
                                        token: RapydGooglePayToken {
                                            api_version: GOOGLE_PAY_API_VERSION,
                                            api_version_minor: GOOGLE_PAY_API_VERSION_MINOR,
                                            payment_method_data:
                                                RapydGooglePayTokenPaymentMethodData {
                                                    description: data.description.clone(),
                                                    tokenization_data:
                                                        RapydGooglePayTokenizationData {
                                                            token_type: RapydGooglePayTokenizationType::PaymentGateway,
                                                            token: Secret::new(token),
                                                        },
                                                    info: RapydGooglePayTokenInfo {
                                                        card_network: data
                                                            .info
                                                            .card_network
                                                            .clone(),
                                                        card_details: data
                                                            .info
                                                            .card_details
                                                            .clone(),
                                                    },
                                                },
                                        },
                                        payment_method: RapydGooglePayPaymentMethod::Card,
                                    },
                                ))
                            }
                        };
                        let pm_type = RapydPaymentMethodType::try_from_wallet_network(
                            &data.info.card_network,
                            None,
                        )?;
                        (
                            RapydWallet {
                                payment_type: WALLET_TYPE_GOOGLE_PAY.to_string(),
                                details,
                            },
                            pm_type,
                        )
                    }
                    WalletData::ApplePay(data) => {
                        let details = match data
                            .payment_data
                            .get_decrypted_apple_pay_payment_data_optional()
                        {
                            Some(decrypt_data) => {
                                // Hyperswitch decrypted the Apple Pay payload; forward the
                                // decrypted card + cryptogram as Rapyd's `decrypted_data`.
                                // Rapyd wants YYMMDD; the decrypted token exposes only
                                // month + year (via the shared expiry helpers), so append
                                // the last day of the expiry month (leap-aware).
                                let ctx = || {
                                    crate::utils::integration_ctx(
                                        "Apple Pay decrypted token has an invalid card expiry",
                                        "Ensure the Apple Pay payload was decrypted with the card expiry.",
                                    )
                                };
                                let expiry_year_yy = decrypt_data
                                    .get_two_digit_expiry_year()
                                    .change_context(IntegrationError::MissingRequiredField {
                                        field_name: "apple expiration_year",
                                        context: ctx(),
                                    })?;
                                let month_u8 = decrypt_data
                                    .get_expiry_month()
                                    .peek()
                                    .parse::<u8>()
                                    .change_context(IntegrationError::MissingRequiredField {
                                        field_name: "apple expiration_month",
                                        context: ctx(),
                                    })?;
                                let year_i32 = decrypt_data
                                    .get_four_digit_expiry_year()
                                    .peek()
                                    .parse::<i32>()
                                    .change_context(IntegrationError::MissingRequiredField {
                                        field_name: "apple expiration_year",
                                        context: ctx(),
                                    })?;
                                let last_day = time::Month::try_from(month_u8)
                                    .change_context(IntegrationError::MissingRequiredField {
                                        field_name: "apple expiration_month",
                                        context: ctx(),
                                    })?
                                    .length(year_i32);
                                let application_expiration_date = Secret::new(format!(
                                    "{}{month_u8:02}{last_day:02}",
                                    expiry_year_yy.peek(),
                                ));
                                RapydWalletDetails::ApplePayDecrypted(Box::new(
                                    RapydApplePayDecryptedDetails {
                                        decrypted_data: RapydApplePayDecryptedData {
                                            application_primary_account_number: decrypt_data
                                                .application_primary_account_number
                                                .clone(),
                                            application_expiration_date,
                                            currency_code: item.router_data.request.currency,
                                            transaction_amount: item
                                                .router_data
                                                .request
                                                .minor_amount,
                                            payment_data_type:
                                                RapydApplePayPaymentDataType::ThreeDSecure,
                                            payment_data: RapydApplePayCryptogram {
                                                online_payment_cryptogram: decrypt_data
                                                    .payment_data
                                                    .online_payment_cryptogram
                                                    .clone(),
                                                eci_indicator: decrypt_data
                                                    .payment_data
                                                    .eci_indicator
                                                    .clone(),
                                            },
                                        },
                                        brand_data: RapydApplePayBrandData {
                                            display_name: data.payment_method.display_name.clone(),
                                            network: parse_card_network(
                                                &data.payment_method.network,
                                            )?,
                                            card_type: RapydCardFunding::try_from(
                                                data.payment_method.pm_type.as_str(),
                                            )?,
                                        },
                                    },
                                ))
                            }
                            None => {
                                let apple_pay_encrypted_data = data
                                    .payment_data
                                    .get_encrypted_apple_pay_payment_data_mandatory()
                                    .change_context(IntegrationError::MissingRequiredField {
                                        field_name: "Apple pay encrypted data",
                                        context: crate::utils::integration_ctx(
                                            "Apple Pay payload is neither decrypted nor a usable encrypted token",
                                            "Provide a decryptable Apple Pay token, or configure connector-side decryption.",
                                        ),
                                    })?;
                                // Rapyd decrypts: the encrypted payment data is
                                // Apple's `paymentData` JSON object, base64-encoded.
                                // Anything that does not decode to a JSON object
                                // is not a usable token.
                                let invalid_payment_data = || IntegrationError::MissingRequiredField {
                                    field_name: "Apple pay encrypted data",
                                    context: crate::utils::integration_ctx(
                                        "Apple Pay encrypted payment data is not a base64-encoded JSON object",
                                        "Send the `paymentData` object of the Apple Pay token, base64-encoded and otherwise unmodified.",
                                    ),
                                };
                                let decoded = common_utils::consts::BASE64_ENGINE
                                    .decode(apple_pay_encrypted_data)
                                    .change_context(invalid_payment_data())?;
                                let payment_data =
                                    serde_json::from_slice::<serde_json::Value>(&decoded)
                                        .change_context(invalid_payment_data())?;
                                if !payment_data.is_object() {
                                    Err(invalid_payment_data())?
                                }
                                RapydWalletDetails::ApplePayEncrypted(Box::new(
                                    RapydApplePayEncryptedDetails {
                                        token: RapydApplePayToken {
                                            payment_data: Secret::new(payment_data),
                                            payment_method: RapydApplePayTokenPaymentMethod {
                                                display_name: data
                                                    .payment_method
                                                    .display_name
                                                    .clone(),
                                                network: data.payment_method.network.clone(),
                                                pm_type: data.payment_method.pm_type.clone(),
                                            },
                                            transaction_identifier: data
                                                .transaction_identifier
                                                .clone(),
                                        },
                                    },
                                ))
                            }
                        };
                        let pm_type = RapydPaymentMethodType::try_from_wallet_network(
                            &data.payment_method.network,
                            Some(data.payment_method.pm_type.as_str()),
                        )?;
                        (
                            RapydWallet {
                                payment_type: WALLET_TYPE_APPLE_PAY.to_string(),
                                details,
                            },
                            pm_type,
                        )
                    }
                    _ => Err(IntegrationError::NotSupported {
                        message: "Selected wallet is not supported by rapyd".to_string(),
                        connector: "rapyd",
                        context: Default::default(),
                    })?,
                };
                Some(RapydPaymentMethodData::PaymentMethod(Box::new(
                    PaymentMethod {
                        pm_type,
                        fields: None,
                        address: None,
                        digital_wallet: Some(rapyd_wallet),
                    },
                )))
            }
            PaymentMethodData::PaymentMethodToken(ref token_data) => {
                Some(RapydPaymentMethodData::Token(token_data.token.clone()))
            }
            _ => None,
        }
        .get_required_value("payment_method not implemented")
        .change_context(IntegrationError::NotImplemented(
            "payment_method".to_owned(),
            Default::default(),
        ))?;
        // Only when the merchant requests future off-session use, ask Rapyd to
        // save the card and create an inline customer, so the response carries
        // the reusable `card_*` / `cus_*` tokens (the mandate) for later MIT
        // calls. An on-session payment saves nothing and reads no customer field.
        let (customer, save_payment_method) = if item.router_data.request.setup_future_usage
            == Some(common_enums::FutureUsage::OffSession)
        {
            let customer_name = item.router_data.request.get_customer_name()?;
            let customer_email = item.router_data.request.get_email()?;
            (
                Some(RapydCustomerRef::Inline(RapydInlineCustomer {
                    name: customer_name,
                    email: customer_email,
                })),
                Some(true),
            )
        } else {
            (None, None)
        };
        Ok(Self {
            amount,
            currency: item.router_data.request.currency,
            payment_method,
            capture,
            payment_method_options,
            merchant_reference_id: Some(
                item.router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            description: None,
            error_payment_url: Some(return_url.clone()),
            complete_payment_url: Some(return_url),
            customer,
            save_payment_method,
            initiation_type: None,
        })
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[allow(clippy::upper_case_acronyms)]
pub enum RapydPaymentStatus {
    #[serde(rename = "ACT")]
    Active,
    #[serde(rename = "CAN")]
    CanceledByClientOrBank,
    #[serde(rename = "CLO")]
    Closed,
    #[serde(rename = "ERR")]
    Error,
    #[serde(rename = "EXP")]
    Expired,
    #[serde(rename = "REV")]
    ReversedByRapyd,
    #[default]
    #[serde(rename = "NEW")]
    New,
    /// Any value Rapyd adds later; never terminal.
    #[serde(other)]
    Unknown,
}

fn get_status(status: RapydPaymentStatus, next_action: NextAction) -> common_enums::AttemptStatus {
    match (status, next_action) {
        // `CLO` | any -> Charged
        (RapydPaymentStatus::Closed, _) => common_enums::AttemptStatus::Charged,
        // `ACT` | `3d_verification` or `pending_confirmation` -> AuthenticationPending
        (
            RapydPaymentStatus::Active,
            NextAction::ThreedsVerification | NextAction::PendingConfirmation,
        ) => common_enums::AttemptStatus::AuthenticationPending,
        // `ACT` | `pending_capture` or `not_applicable` -> Authorized
        (RapydPaymentStatus::Active, NextAction::PendingCapture | NextAction::NotApplicable) => {
            common_enums::AttemptStatus::Authorized
        }
        // `ACT` | `pending_offline_capture` -> Authorized (treated like `pending_capture`)
        (RapydPaymentStatus::Active, NextAction::PendingOfflineCapture) => {
            common_enums::AttemptStatus::Authorized
        }
        // `ACT` | unrecognised `next_action` -> Pending (non-terminal)
        (RapydPaymentStatus::Active, NextAction::Unknown) => common_enums::AttemptStatus::Pending,
        // `CAN`, `EXP`, `REV` | any -> Voided
        (
            RapydPaymentStatus::CanceledByClientOrBank
            | RapydPaymentStatus::Expired
            | RapydPaymentStatus::ReversedByRapyd,
            _,
        ) => common_enums::AttemptStatus::Voided,
        // `ERR` | any -> Failure
        (RapydPaymentStatus::Error, _) => common_enums::AttemptStatus::Failure,
        // `NEW` | any -> Authorizing
        (RapydPaymentStatus::New, _) => common_enums::AttemptStatus::Authorizing,
        // unrecognised `data.status` | any -> Pending (non-terminal)
        (RapydPaymentStatus::Unknown, _) => common_enums::AttemptStatus::Pending,
    }
}

/// Status of a payment object. A zero-amount payment is Rapyd's card
/// validation: once the customer has passed the challenge it rests at `ACT`
/// with `pending_capture`, `original_amount` 0 and `paid: false`. The
/// validation is complete, the card is saved and no funds are held, so it is
/// reported as terminal success, not as an authorization waiting for a
/// capture the automatic-capture caller never sends. The discriminator is
/// `original_amount`: an uncaptured non-zero authorization also reads
/// `amount: 0`, and an absent `original_amount` is not zero. Every other
/// state, `pending_offline_capture` included, is `get_status` unchanged.
fn get_payment_status(data: &ResponseData) -> common_enums::AttemptStatus {
    let is_card_validation = data.status == RapydPaymentStatus::Active
        && (data.next_action == NextAction::PendingCapture
            || data.next_action == NextAction::NotApplicable)
        && data.original_amount == Some(FloatMajorUnit::zero());
    if is_card_validation {
        common_enums::AttemptStatus::Charged
    } else {
        get_status(data.status.to_owned(), data.next_action.to_owned())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RapydPaymentsResponse {
    pub status: Status,
    pub data: Option<ResponseData>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Status {
    pub error_code: String,
    pub status: Option<String>,
    pub message: Option<String>,
    pub response_code: Option<String>,
    pub operation_id: Option<String>,
}

/// Body of a non-2xx response. Rapyd returns the `status` object alone, or
/// `status` plus a `data` object for a failed Create Payment, in which
/// `data.id` can be `null` and the failure fields can be empty strings. Every
/// field is optional so that any of those shapes parses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RapydErrorResponse {
    pub status: RapydErrorStatus,
    pub data: Option<RapydErrorData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RapydErrorStatus {
    pub error_code: Option<String>,
    pub status: Option<String>,
    pub message: Option<String>,
    pub response_code: Option<String>,
    pub operation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RapydErrorData {
    pub id: Option<String>,
    pub status: Option<String>,
    pub failure_code: Option<String>,
    pub failure_message: Option<String>,
    pub merchant_reference_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NextAction {
    #[serde(rename = "3d_verification")]
    ThreedsVerification,
    #[serde(rename = "pending_capture")]
    PendingCapture,
    #[serde(rename = "not_applicable")]
    NotApplicable,
    #[serde(rename = "pending_confirmation")]
    PendingConfirmation,
    #[serde(rename = "pending_offline_capture")]
    PendingOfflineCapture,
    /// Any value Rapyd adds later; never terminal.
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResponseData {
    pub id: String,
    pub amount: FloatMajorUnit,
    pub status: RapydPaymentStatus,
    pub next_action: NextAction,
    pub redirect_url: Option<String>,
    pub original_amount: Option<FloatMajorUnit>,
    pub is_partial: Option<bool>,
    pub currency_code: Option<common_enums::Currency>,
    pub country_code: Option<String>,
    pub captured: Option<bool>,
    pub transaction_id: String,
    pub merchant_reference_id: Option<String>,
    pub paid: Option<bool>,
    pub failure_code: Option<String>,
    pub failure_message: Option<String>,
    /// Saved-card token (`card_*`) — populated when the payment was
    /// created with `save_payment_method: true`. Used as the MIT token
    /// on subsequent charges.
    pub payment_method: Option<String>,
    /// Nested payment-method data; carries `network_reference_id`, the
    /// network transaction id surfaced as `network_txn_id` for recurring.
    pub payment_method_data: Option<RapydResponsePaymentMethodData>,
}

/// Subset of Rapyd's response `payment_method_data` object.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RapydResponsePaymentMethodData {
    pub network_reference_id: Option<Secret<String>>,
}

// Capture Request
#[derive(Debug, Serialize, Clone)]
pub struct CaptureRequest {
    amount: Option<StringMajorUnit>,
    receipt_email: Option<Secret<String>>,
    statement_descriptor: Option<String>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        RapydRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for CaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: RapydRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.minor_amount_to_capture,
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: crate::utils::amount_conversion_ctx(
                    "rapyd capture",
                    &item.router_data.request.minor_amount_to_capture,
                    &item.router_data.request.currency,
                ),
            })?;
        Ok(Self {
            amount: Some(amount),
            receipt_email: None,
            statement_descriptor: None,
        })
    }
}

// Refund Request
#[derive(Default, Debug, Serialize)]
pub struct RapydRefundRequest {
    pub payment: String,
    pub amount: Option<StringMajorUnit>,
    pub currency: Option<common_enums::Currency>,
}

impl<F, T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<RapydRouterData<RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>, T>>
    for RapydRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: RapydRouterData<RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    ) -> Result<Self, Self::Error> {
        let amount = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.minor_refund_amount,
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: crate::utils::amount_conversion_ctx(
                    "rapyd refund",
                    &item.router_data.request.minor_refund_amount,
                    &item.router_data.request.currency,
                ),
            })?;
        Ok(Self {
            // The id is the body field `payment`: a blank one is refused here
            // instead of being sent to Rapyd as `"payment": ""`.
            payment: item.connector.require_non_blank_id(
                &item.router_data.request.connector_transaction_id,
                "connector_transaction_id",
            )?,
            amount: Some(amount),
            currency: Some(item.router_data.request.currency),
        })
    }
}

// Refund Response
#[allow(dead_code)]
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub enum RefundStatus {
    Completed,
    Error,
    Rejected,
    #[default]
    Pending,
    // Cancel Refund documents the value as `Cancelled`; every other page
    // spells it `Canceled`. Both are accepted.
    #[serde(alias = "Cancelled")]
    Canceled,
    #[serde(other)]
    Unknown,
}

// Techspec "Status Mappings > Refund status (`data.status`)", one arm per row.
impl From<RefundStatus> for common_enums::RefundStatus {
    fn from(item: RefundStatus) -> Self {
        match item {
            // `Completed` | Success
            RefundStatus::Completed => Self::Success,
            // `Pending` | Pending
            RefundStatus::Pending => Self::Pending,
            // `Error` | Failure; `Rejected` | Failure;
            // `Canceled` (also seen as `Cancelled`) | refund was cancelled
            RefundStatus::Error | RefundStatus::Rejected | RefundStatus::Canceled => Self::Failure,
            // A value outside the documented set says nothing final about the
            // refund, so it stays pending rather than failing the response.
            RefundStatus::Unknown => Self::Pending,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefundResponse {
    pub status: Status,
    pub data: Option<RefundResponseData>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RefundResponseData {
    pub id: String,
    pub payment: String,
    pub amount: FloatMajorUnit,
    pub currency: common_enums::Currency,
    pub status: RefundStatus,
    pub created_at: Option<i64>,
    pub failure_reason: Option<String>,
}

impl<F, T> TryFrom<ResponseRouterData<RefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, T, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<RefundResponse, Self>) -> Result<Self, Self::Error> {
        // Rapyd sends empty strings where a value is absent; an empty value is
        // never reported as the error code, message or reason.
        let non_empty = |value: Option<&String>| value.filter(|v| !v.is_empty()).cloned();
        let response = match item.response.data {
            Some(data) => Ok(RefundsResponseData {
                connector_refund_id: data.id,
                refund_status: common_enums::RefundStatus::from(data.status),
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            // A 2xx without `data` carries no refund object: the error is
            // reported and no refund id or terminal status is invented (this
            // mapper also serves RSync).
            None => Err(ErrorResponse {
                code: non_empty(Some(&item.response.status.error_code))
                    .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                status_code: item.http_code,
                message: non_empty(item.response.status.status.as_ref())
                    .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                reason: non_empty(item.response.status.message.as_ref()),
                attempt_status: None,
                connector_transaction_id: None,
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            }),
        };
        Ok(Self {
            response,
            ..item.router_data
        })
    }
}

// ---- ClientAuthenticationToken flow types ----

/// Creates a Rapyd checkout page/session. The checkout id and redirect_url
/// are returned to the frontend for client-side payment completion.
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
pub struct RapydClientAuthRequest {
    pub amount: StringMajorUnit,
    pub currency: common_enums::Currency,
    pub country: Option<String>,
    pub merchant_reference_id: Option<String>,
    pub complete_checkout_url: Option<String>,
    pub cancel_checkout_url: Option<String>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        RapydRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                MerchantAuthenticationFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for RapydClientAuthRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: RapydRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                MerchantAuthenticationFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;

        let amount = item
            .connector
            .amount_converter
            .convert(router_data.request.amount, router_data.request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Verify that the checkout amount and currency are valid.".to_owned(),
                    ),
                    doc_url: Some("https://docs.rapyd.net/en/create-checkout-page.html".to_owned()),
                    additional_context: Some(
                        "Rapyd checkout requires the amount in major-unit string format."
                            .to_owned(),
                    ),
                },
            })?;

        let country = router_data.request.country.map(|c| c.to_string());
        let return_url = router_data.resource_common_data.return_url.clone();

        Ok(Self {
            amount,
            currency: router_data.request.currency,
            country,
            merchant_reference_id: Some(
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            complete_checkout_url: return_url.clone(),
            cancel_checkout_url: return_url,
        })
    }
}

/// Rapyd checkout response containing checkout id and redirect_url for SDK initialization.
#[derive(Debug, Deserialize, Serialize)]
pub struct RapydClientAuthResponse {
    pub status: Status,
    pub data: Option<RapydCheckoutResponseData>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RapydCheckoutResponseData {
    pub id: String,
    pub redirect_url: String,
}

impl TryFrom<ResponseRouterData<RapydClientAuthResponse, Self>>
    for RouterDataV2<
        ClientAuthenticationToken,
        MerchantAuthenticationFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<RapydClientAuthResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        let data = response.data.ok_or(
            ConnectorError::response_deserialization_failed_with_context(
                item.http_code,
                Some(
                    "Rapyd checkout response is missing the 'data' field containing \
                     checkout_id and redirect_url."
                        .to_owned(),
                ),
            ),
        )?;

        let session_data = ClientAuthenticationTokenData::ConnectorSpecific(Box::new(
            ConnectorSpecificClientAuthenticationResponse::Rapyd(
                RapydClientAuthenticationResponseDomain {
                    checkout_id: data.id,
                    redirect_url: data.redirect_url,
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

// ============================================================================
// CreateOrder Flow - Request/Response Types
// ============================================================================

#[derive(Debug, Serialize)]
pub struct RapydCreateOrderRequest {
    pub amount: StringMajorUnit,
    pub currency: common_enums::Currency,
    pub country: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_reference_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub complete_payment_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_payment_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RapydCreateOrderResponse {
    pub status: Status,
    pub data: Option<RapydCheckoutData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RapydCheckoutData {
    pub id: String,
    pub status: String,
    pub redirect_url: Option<String>,
    pub amount: Option<FloatMajorUnit>,
    pub currency: Option<String>,
    pub country: Option<String>,
    pub language: Option<String>,
    pub merchant_reference_id: Option<String>,
    pub page_expiration: Option<i64>,
    pub timestamp: Option<i64>,
}

/// Metadata for CreateOrder flow, passed via connector_feature_data
#[derive(Debug, Clone, Deserialize)]
pub struct RapydCreateOrderMetadata {
    /// Country code for the checkout page (ISO 3166-1 alpha-2)
    pub country: Option<String>,
}

// ============================================================================
// CreateOrder Flow - Request Transformation
// ============================================================================

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        RapydRouterData<
            RouterDataV2<
                CreateOrder,
                PaymentFlowData,
                PaymentCreateOrderData,
                PaymentCreateOrderResponse,
            >,
            T,
        >,
    > for RapydCreateOrderRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: RapydRouterData<
            RouterDataV2<
                CreateOrder,
                PaymentFlowData,
                PaymentCreateOrderData,
                PaymentCreateOrderResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        let amount = item
            .connector
            .amount_converter
            .convert(router_data.request.amount, router_data.request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        // Try to get country from billing address first, then fallback to connector_feature_data
        let country = router_data
            .resource_common_data
            .get_optional_billing_country()
            .map(|c| c.to_string())
            .or_else(|| {
                // Fallback: try to get country from connector_feature_data
                router_data
                    .resource_common_data
                    .connector_feature_data
                    .as_ref()
                    .and_then(|meta| {
                        serde_json::from_value::<RapydCreateOrderMetadata>(meta.clone().expose())
                            .ok()
                    })
                    .and_then(|m| m.country)
            })
            .ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                    field_name: "billing_country or connector_feature_data.country",
                    context: Default::default(),
                })
            })?;

        Ok(Self {
            amount,
            currency: router_data.request.currency,
            country,
            merchant_reference_id: Some(
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            complete_payment_url: router_data.resource_common_data.return_url.clone(),
            error_payment_url: router_data.resource_common_data.return_url.clone(),
            language: Some("en".to_string()),
        })
    }
}

// ============================================================================
// CreateOrder Flow - Response Transformation
// ============================================================================

impl TryFrom<ResponseRouterData<RapydCreateOrderResponse, Self>>
    for RouterDataV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<RapydCreateOrderResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        match response.data {
            Some(data) => {
                let status = match data.status.as_str() {
                    "NEW" | "INP" => common_enums::AttemptStatus::Pending,
                    "DON" => common_enums::AttemptStatus::Charged,
                    "EXP" | "DEC" => common_enums::AttemptStatus::Failure,
                    _ => common_enums::AttemptStatus::Pending,
                };

                // Extract checkout_id for use in resource_common_data
                let checkout_id = data.id.clone();

                Ok(Self {
                    response: Ok(PaymentCreateOrderResponse {
                        connector_order_id: checkout_id.clone(),
                        session_data: None,
                    }),
                    resource_common_data: PaymentFlowData {
                        status,
                        reference_id: Some(checkout_id.clone()),
                        // Store order ID so Authorize flow can use it via connector_order_id
                        connector_order_id: Some(checkout_id),
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                })
            }
            None => Ok(Self {
                response: Err(ErrorResponse {
                    code: response.status.error_code,
                    status_code: item.http_code,
                    message: response.status.status.unwrap_or_default(),
                    reason: response.status.message,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::Failure,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            }),
        }
    }
}

// ============================================================================
// SetupMandate (zero/low-amount COF verification) — Rapyd
// ============================================================================
// Rapyd has no dedicated mandate endpoint. To capture a reusable card token
// we call POST /v1/payments with `save_payment_method: true` plus an inline
// `customer: { name, email }` object so Rapyd creates `cus_*` in the same
// call and attaches a reusable `card_*` to it.
//
// Using `/v1/payments` (rather than `/v1/customers`) avoids the
// `complete_payment_url` whitelist check that the customer-create
// endpoint enforces on sandbox accounts.

/// SetupMandate request – reuses the `/v1/payments` shape but asks Rapyd
/// to save the card under a newly-created customer.
pub type RapydSetupMandateRequest<T> = RapydPaymentsRequest<T>;

/// SetupMandate response – structurally identical to `RapydPaymentsResponse`
/// but defined as a distinct newtype so the SetupMandate `TryFrom` does
/// not collide with the blanket Authorize-style conversion (E0119).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RapydSetupMandateResponse {
    pub status: Status,
    pub data: Option<ResponseData>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        RapydRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for RapydSetupMandateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: RapydRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let request = &router_data.request;

        // Capture intent, refused before any payment-method data is read:
        // manual capture leaves the setup payment authorized, not charged.
        // The zero-amount case is settled below, once the amount is known; the
        // G-Mandates-01 refusal still fires here for every amount.
        let capture_intent = mandate_capture_flag(request.capture_method)?;

        // Rapyd rejects mandate-setup calls with no amount and silently
        // defaulting here would charge an arbitrary value in the caller's
        // currency (e.g. ¥100 vs $1.00). Require the caller to pass an
        // explicit verification amount — zero-amount is allowed if the
        // Rapyd account supports zero-auth.
        let minor_amount = request
            .minor_amount
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "minor_amount",
                context: crate::utils::integration_ctx(
                    "Rapyd Create Payment requires an amount; a setup without one would be charged an arbitrary value",
                    "Send the setup amount on the setup-mandate request (0 for a verification-only setup).",
                ),
            })?;
        // A zero-amount setup is Rapyd's card validation. Rapyd accepts it only
        // with `capture: false` (HTTP 400 ERROR_CARD_VALIDATION_CAPTURE_TRUE
        // otherwise), so the capture method does not decide the flag for it;
        // the completed validation is reported as terminal success by
        // `get_payment_status`.
        let is_zero_amount = minor_amount.get_amount_as_i64() == 0;
        let capture = capture_intent && !is_zero_amount;
        // Zero-amount verification goes as "0"; Rapyd rejects "0.00".
        let amount = if minor_amount.get_amount_as_i64() == 0 {
            StringMajorUnit::zero()
        } else {
            item.connector
                .amount_converter
                .convert(minor_amount, request.currency)
                .change_context(IntegrationError::AmountConversionFailed {
                    context: crate::utils::amount_conversion_ctx(
                        "rapyd setup mandate",
                        &minor_amount,
                        &request.currency,
                    ),
                })?
        };

        let payment_method = match &request.payment_method_data {
            PaymentMethodData::Card(ccard) => {
                // Placeholder India type — see the Authorize flow: the sandbox
                // merchant enables `in_amex_card`, not the per-network types.
                let pm_type = RapydPaymentMethodType::InAmexCard;
                // Rapyd documents `payment_method.fields.name` as required
                // (https://docs.rapyd.net/en/create-card-payment-method.html).
                // Prefer the cardholder name on the card itself; fall back to
                // the billing full name. We deliberately do not fall back to
                // `customer_name`, which describes the Rapyd customer object
                // (inline `{name, email}`) — not the cardholder. A blank
                // candidate counts as absent: Rapyd rejects an empty name.
                let is_not_blank = |name: &Secret<String>| !name.peek().trim().is_empty();
                let cardholder_name = ccard
                    .card_holder_name
                    .clone()
                    .filter(is_not_blank)
                    .or_else(|| {
                        router_data
                            .resource_common_data
                            .get_optional_billing_full_name()
                            .filter(is_not_blank)
                    })
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "payment_method.card.card_holder_name",
                        context: crate::utils::integration_ctx(
                            "Rapyd requires a non-empty cardholder name in `payment_method.fields.name`; the request carries neither a cardholder name nor a billing name",
                            "Provide `payment_method.card.card_holder_name` or the billing first and last name.",
                        ),
                    })?;
                RapydPaymentMethodData::PaymentMethod(Box::new(PaymentMethod {
                    pm_type,
                    fields: Some(PaymentFields {
                        number: ccard.card_number.to_owned(),
                        expiration_month: ccard.card_exp_month.to_owned(),
                        expiration_year: ccard.card_exp_year.to_owned(),
                        name: cardholder_name,
                        cvv: ccard.card_cvc.to_owned(),
                    }),
                    address: None,
                    digital_wallet: None,
                }))
            }
            _ => {
                return Err(IntegrationError::NotImplemented(
                    "payment_method for rapyd SetupMandate".to_owned(),
                    Default::default(),
                ))?;
            }
        };

        let three_ds_enabled = matches!(
            router_data.resource_common_data.auth_type,
            common_enums::AuthenticationType::ThreeDs
        );
        let payment_method_options = Some(PaymentMethodOptions {
            three_ds: three_ds_enabled,
        });

        // Rapyd REQUIRES a customer to save a payment method. We pass an
        // inline `{name, email}` object so Rapyd creates `cus_*` in the
        // same call and attaches the saved `card_*` to it. Both fields
        // must come from the customer payload — billing address describes
        // the cardholder, not the customer-of-record, and mixing them
        // would attach the card to the wrong customer on repeat use.
        let customer_name =
            request
                .customer_name
                .clone()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "customer.name",
                    context: crate::utils::integration_ctx(
                        "Rapyd creates an inline customer to save the card, which requires a name",
                        "Send the customer name on the setup-mandate request.",
                    ),
                })?;
        let customer_email =
            request
                .email
                .clone()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "customer.email",
                    context: crate::utils::integration_ctx(
                        "Rapyd's inline customer requires an email",
                        "Send the customer email on the setup-mandate request.",
                    ),
                })?;
        let inline_customer = RapydInlineCustomer {
            name: Secret::new(customer_name),
            email: customer_email,
        };

        // The setup's return URL arrives on `request.router_return_url` on
        // every request path; `PaymentFlowData.return_url` is filled only by
        // the gRPC server's conversion and is `None` on the SDK/FFI path.
        let return_url = request
            .router_return_url
            .clone()
            .or_else(|| router_data.resource_common_data.return_url.clone())
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "return_url",
                context: crate::utils::integration_ctx(
                    "A Rapyd setup can answer with a 3DS challenge, which needs `complete_payment_url` and `error_payment_url`",
                    "Send a return URL on the setup-mandate request.",
                ),
            })?;

        Ok(Self {
            amount,
            currency: request.currency,
            payment_method,
            capture: Some(capture),
            payment_method_options,
            merchant_reference_id: Some(
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            description: router_data.resource_common_data.description.clone(),
            complete_payment_url: Some(return_url.clone()),
            error_payment_url: Some(return_url),
            customer: Some(RapydCustomerRef::Inline(inline_customer)),
            save_payment_method: Some(true),
            initiation_type: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<RapydSetupMandateResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<RapydSetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Rapyd sends empty strings where a value is absent (`failure_code: ""`);
        // an empty value is never reported as the error code, message or reason.
        let non_empty = |value: Option<&String>| value.filter(|v| !v.is_empty()).cloned();
        let envelope_code = non_empty(Some(&item.response.status.error_code));
        let envelope_message = non_empty(item.response.status.status.as_ref())
            .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string());

        let (status, response) = match &item.response.data {
            Some(data) => {
                let attempt_status = get_payment_status(data);
                if attempt_status == common_enums::AttemptStatus::Failure {
                    (
                        common_enums::AttemptStatus::Failure,
                        Err(ErrorResponse {
                            code: non_empty(data.failure_code.as_ref())
                                .or(envelope_code)
                                .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                            status_code: item.http_code,
                            message: envelope_message,
                            reason: non_empty(data.failure_message.as_ref()),
                            attempt_status: None,
                            // Keep the connector's payment id on a failure so the
                            // attempt can be located in Rapyd's dashboard.
                            connector_transaction_id: Some(data.id.clone()),
                            network_advice_code: None,
                            network_decline_code: None,
                            network_error_message: None,
                            typed_connector_response: None,
                            raw_connector_response: None,
                            raw_connector_request: None,
                            typed_connector_request: None,
                        }),
                    )
                } else {
                    // Surface the 3DS redirect so verification can be completed.
                    let redirection_data = data
                        .redirect_url
                        .as_ref()
                        .filter(|url| !url.is_empty())
                        .map(|url| {
                            Url::parse(url).change_context(
                                crate::utils::response_handling_fail_for_connector(
                                    item.http_code,
                                    "rapyd",
                                ),
                            )
                        })
                        .transpose()?
                        .map(|url| RedirectForm::from((url, Method::Get)));
                    // The saved card token is the mandate reference used on
                    // MIT replays; Rapyd charges it without a customer id. It
                    // is absent while `next_action` is `3d_verification`.
                    let mandate_reference = data.payment_method.as_ref().map(|card| {
                        Box::new(MandateReference {
                            connector_mandate_id: Some(card.clone()),
                            payment_method_id: None,
                            connector_mandate_request_reference_id: None,
                            mandate_metadata: None,
                        })
                    });
                    // `ACT` with `pending_capture` is an authorized, uncaptured
                    // payment for a non-zero setup; a completed zero-amount card
                    // validation is terminal success (see `get_payment_status`).
                    (
                        attempt_status,
                        Ok(PaymentsResponseData::TransactionResponse {
                            resource_id: ResponseId::ConnectorTransactionId(data.id.clone()),
                            redirection_data: redirection_data.map(Box::new),
                            mandate_reference,
                            connector_metadata: None,
                            network_txn_id: None,
                            network_txn_link_id: None,
                            connector_response_reference_id: data.merchant_reference_id.clone(),
                            incremental_authorization_allowed: None,
                            status_code: item.http_code,
                            splits: None,
                            payment_account_reference: None,
                        }),
                    )
                }
            }
            // A 2xx without `data` says nothing about the payment: the error is
            // reported and the status the request carried is left as it was.
            None => (
                item.router_data.resource_common_data.status,
                Err(ErrorResponse {
                    code: envelope_code
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                    status_code: item.http_code,
                    message: envelope_message,
                    reason: non_empty(item.response.status.message.as_ref()),
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
            ),
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

// ---------------------------------------------------------------------------
// RepeatPayment (MIT) — Rapyd reuses /v1/payments with a stored
// `payment_method` token (the payment id returned by SetupMandate). The
// request body is structurally identical to `RapydPaymentsRequest`, but we
// use a distinct response newtype so the TryFrom impls don't collide with
// the blanket Authorize conversion.
// ---------------------------------------------------------------------------

pub type RapydRepeatPaymentRequest<T> = RapydPaymentsRequest<T>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RapydRepeatPaymentResponse {
    pub status: Status,
    pub data: Option<ResponseData>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        RapydRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for RapydRepeatPaymentRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: RapydRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let request = &router_data.request;

        let amount = if request.minor_amount.get_amount_as_i64() == 0 {
            StringMajorUnit::zero()
        } else {
            item.connector
                .amount_converter
                .convert(request.minor_amount, request.currency)
                .change_context(IntegrationError::AmountConversionFailed {
                    context: crate::utils::amount_conversion_ctx(
                        "rapyd repeat payment",
                        &request.minor_amount,
                        &request.currency,
                    ),
                })?
        };

        // The saved card token stored at CIT/SetupMandate time is the mandate
        // reference. Rapyd charges it directly — no customer id needed.
        let connector_mandate = match &request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(connector_mandate) => connector_mandate,
            _ => {
                return Err(IntegrationError::NotImplemented(
                    "non-connector mandate for rapyd RepeatPayment".to_owned(),
                    Default::default(),
                ))?;
            }
        };
        // The `card_...` id is the whole `payment_method` value: a blank one
        // is refused, never sent.
        let card_id = connector_mandate
            .get_connector_mandate_id()
            .filter(|id| !id.trim().is_empty())
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "mandate_reference.connector_mandate_id",
                context: crate::utils::integration_ctx(
                    "Rapyd charges a saved card by its `card_...` id, sent as `payment_method`; the request carries none",
                    "Send the connector mandate id returned by the mandate setup.",
                ),
            })?;
        let capture = mandate_capture_flag(request.capture_method)?;

        let three_ds_enabled = matches!(
            router_data.resource_common_data.auth_type,
            common_enums::AuthenticationType::ThreeDs
        );
        let payment_method_options = Some(PaymentMethodOptions {
            three_ds: three_ds_enabled,
        });

        // On Charge, return_url arrives on `request.router_return_url`;
        // `PaymentFlowData.return_url` is hardcoded to None for this flow.
        // A merchant-initiated charge has no browser step, so the two redirect
        // URLs are sent when a return URL is present and omitted otherwise.
        let return_url = request
            .router_return_url
            .clone()
            .or_else(|| router_data.resource_common_data.return_url.clone());

        Ok(Self {
            amount,
            currency: request.currency,
            payment_method: RapydPaymentMethodData::Token(Secret::new(card_id)),
            capture: Some(capture),
            payment_method_options,
            merchant_reference_id: Some(
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            description: None,
            error_payment_url: return_url.clone(),
            complete_payment_url: return_url,
            customer: None,
            save_payment_method: None,
            initiation_type: Some(RapydInitiationType::Unscheduled),
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<RapydRepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<RapydRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Rapyd sends empty strings where a value is absent (`failure_code: ""`);
        // an empty value is never reported as the error code, message or reason.
        let non_empty = |value: Option<&String>| value.filter(|v| !v.is_empty()).cloned();
        let envelope_code = non_empty(Some(&item.response.status.error_code));
        let envelope_message = non_empty(item.response.status.status.as_ref())
            .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string());

        let (status, response) = match &item.response.data {
            Some(data) => {
                let attempt_status =
                    get_status(data.status.to_owned(), data.next_action.to_owned());
                if attempt_status == common_enums::AttemptStatus::Failure {
                    (
                        common_enums::AttemptStatus::Failure,
                        Err(ErrorResponse {
                            code: non_empty(data.failure_code.as_ref())
                                .or(envelope_code)
                                .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                            status_code: item.http_code,
                            message: envelope_message,
                            reason: non_empty(data.failure_message.as_ref()),
                            attempt_status: None,
                            // Keep the connector's payment id on a failure so the
                            // attempt can be located in Rapyd's dashboard.
                            connector_transaction_id: Some(data.id.clone()),
                            network_advice_code: None,
                            network_decline_code: None,
                            network_error_message: None,
                            typed_connector_response: None,
                            raw_connector_response: None,
                            raw_connector_request: None,
                            typed_connector_request: None,
                        }),
                    )
                } else {
                    (
                        attempt_status,
                        Ok(PaymentsResponseData::TransactionResponse {
                            resource_id: ResponseId::ConnectorTransactionId(data.id.clone()),
                            redirection_data: None,
                            // MIT replay does not mint a new mandate — the
                            // `card_*` from SetupMandate stays valid. `data.id`
                            // is a one-shot payment id and must never be stored
                            // as a mandate.
                            mandate_reference: None,
                            connector_metadata: None,
                            network_txn_id: None,
                            network_txn_link_id: None,
                            connector_response_reference_id: data.merchant_reference_id.to_owned(),
                            incremental_authorization_allowed: None,
                            status_code: item.http_code,
                            splits: None,
                            payment_account_reference: None,
                        }),
                    )
                }
            }
            // A 2xx without `data` says nothing about the payment: the error is
            // reported and the status the request carried is left as it was.
            None => (
                item.router_data.resource_common_data.status,
                Err(ErrorResponse {
                    code: envelope_code
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                    status_code: item.http_code,
                    message: envelope_message,
                    reason: non_empty(item.response.status.message.as_ref()),
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
            ),
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

// ===== Incoming webhooks =====
// Techspec "Complete Endpoint Inventory > IncomingWebhook > 1. Receive Webhook
// (inbound)" and "Webhook Events".

/// The webhook secret for Rapyd: a JSON object holding the two keys that
/// enter the signature. It is configured separately from the connector
/// account credentials, which are never used to verify a webhook.
#[derive(Debug, Deserialize)]
pub struct RapydWebhookSecret {
    pub access_key: Secret<String>,
    pub secret_key: Secret<String>,
}

/// Body of an inbound Rapyd webhook. Only `type` and `data` are read: the
/// documented samples disagree on the root `id` / `token` / `status` /
/// `created_at` fields (refund events carry the `wh_` id in `token`, and one
/// sample has none of them), so none of those is required.
#[derive(Debug, Deserialize)]
pub struct RapydIncomingWebhook {
    #[serde(rename = "type")]
    pub webhook_type: RapydWebhookEventType,
    pub data: serde_json::Value,
}

/// Documented webhook `type` values.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RapydWebhookEventType {
    PaymentCompleted,
    PaymentCaptured,
    PaymentFailed,
    PaymentSucceeded,
    PaymentCanceled,
    PaymentExpired,
    PaymentUpdated,
    RefundCompleted,
    PaymentRefundFailed,
    PaymentRefundRejected,
    PaymentRefundUpdated,
    PaymentDisputeCreated,
    PaymentDisputeUpdated,
    CardAddedSuccessfully,
    /// Any `type` Rapyd adds later; reported as an unsupported event.
    #[serde(other)]
    Unknown,
}

/// `data` of a payment event. It is the payment object the sync endpoints
/// return, but the event samples omit fields those structs require
/// (`next_action`, `transaction_id`), so only what the webhook reads is typed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RapydWebhookPaymentData {
    pub id: String,
    pub status: RapydPaymentStatus,
    pub next_action: Option<NextAction>,
    pub merchant_reference_id: Option<String>,
    pub failure_code: Option<String>,
    pub failure_message: Option<String>,
    pub payment_method: Option<String>,
}

impl RapydWebhookPaymentData {
    /// Same mapping as the sync responses, so a webhook and a sync can never
    /// disagree. A payment event without `next_action` has nothing left to
    /// do, which is what `not_applicable` means.
    pub fn attempt_status(&self) -> common_enums::AttemptStatus {
        get_status(
            self.status.clone(),
            self.next_action
                .clone()
                .unwrap_or(NextAction::NotApplicable),
        )
    }
}

/// `data` of a refund event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RapydWebhookRefundData {
    pub id: String,
    pub status: RefundStatus,
    pub payment: Option<String>,
    pub failure_code: Option<String>,
    pub failure_reason: Option<String>,
}

/// `data` of a dispute event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RapydWebhookDisputeData {
    pub token: String,
    pub original_transaction_id: String,
    pub status: RapydWebhookDisputeStatus,
    /// Read in minor units, as the reference implementation does.
    pub amount: MinorUnit,
    pub currency: common_enums::Currency,
    pub dispute_reason_description: Option<String>,
    pub due_date: Option<i64>,
}

/// Documented dispute `data.status` values.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
pub enum RapydWebhookDisputeStatus {
    #[serde(rename = "ACT")]
    Active,
    #[serde(rename = "RVW")]
    Review,
    #[serde(rename = "PRA")]
    PreArbitration,
    #[serde(rename = "ARB")]
    Arbitration,
    #[serde(rename = "LOS")]
    Lose,
    #[serde(rename = "WIN")]
    Win,
    #[serde(rename = "REV")]
    Reversed,
    /// Any value Rapyd adds later; reported as an unsupported event.
    #[serde(other)]
    Unknown,
}

/// `data` typed by the event `type`. The payload is selected by `type`, never
/// by which shape happens to deserialize first: a refund body also satisfies
/// most of a payment's fields.
#[derive(Debug, Clone)]
pub enum RapydWebhookPayload {
    Payment(RapydWebhookPaymentData),
    Refund(RapydWebhookRefundData),
    Dispute(RapydWebhookDisputeData),
    /// `CARD_ADDED_SUCCESSFULLY` and unknown types carry no object that is read.
    Unsupported,
}

impl RapydIncomingWebhook {
    /// Types `data` according to the "`data` is" column of the event table.
    pub fn payload(&self) -> Result<RapydWebhookPayload, serde_json::Error> {
        match self.webhook_type {
            RapydWebhookEventType::PaymentCompleted
            | RapydWebhookEventType::PaymentCaptured
            | RapydWebhookEventType::PaymentFailed
            | RapydWebhookEventType::PaymentSucceeded
            | RapydWebhookEventType::PaymentCanceled
            | RapydWebhookEventType::PaymentExpired
            | RapydWebhookEventType::PaymentUpdated => {
                RapydWebhookPaymentData::deserialize(&self.data).map(RapydWebhookPayload::Payment)
            }
            RapydWebhookEventType::RefundCompleted
            | RapydWebhookEventType::PaymentRefundFailed
            | RapydWebhookEventType::PaymentRefundRejected
            | RapydWebhookEventType::PaymentRefundUpdated => {
                RapydWebhookRefundData::deserialize(&self.data).map(RapydWebhookPayload::Refund)
            }
            RapydWebhookEventType::PaymentDisputeCreated
            | RapydWebhookEventType::PaymentDisputeUpdated => {
                RapydWebhookDisputeData::deserialize(&self.data).map(RapydWebhookPayload::Dispute)
            }
            RapydWebhookEventType::CardAddedSuccessfully | RapydWebhookEventType::Unknown => {
                Ok(RapydWebhookPayload::Unsupported)
            }
        }
    }
}
