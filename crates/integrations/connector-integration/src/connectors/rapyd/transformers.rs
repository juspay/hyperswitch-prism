use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    pii::{Email, IpAddress},
    request::Method,
    types::MinorUnit,
    FloatMajorUnit, StringMajorUnit,
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, CreateOrder, PSync, RSync, Refund,
        RepeatPayment, SetupMandate,
    },
    connector_types::{
        BillingDescriptor, ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData,
        ConnectorSpecificClientAuthenticationResponse, EventType, MandateReference,
        MandateReferenceId, PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        RapydClientAuthenticationResponse as RapydClientAuthenticationResponseDomain,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        ResponseId, SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext, WebhookError},
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::{
        GpayTokenizationData, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber, WalletData,
    },
    router_data::{
        AdditionalPaymentMethodConnectorResponse, ConnectorResponseData, ConnectorSpecificConfig,
        ErrorResponse, FlowStatus,
    },
    router_data_v2::RouterDataV2,
    router_request_types::{AuthenticationData, BrowserInformation},
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
                context: crate::utils::integration_ctx(
                    "rapyd wallet brand_data.type accepts credit, debit or prepaid only",
                    "Send the wallet card funding type as credit, debit or prepaid.",
                ),
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
            context: crate::utils::integration_ctx(
                "rapyd could not map the wallet card network",
                "Send a supported card network (Visa, Mastercard, Amex) in the wallet info.",
            ),
        },
    )
}

/// Rapyd `payment_method.type` identifier. Rapyd's types are country-prefixed
/// (`<country>_<network>_card`) and enumerated per-country by the List Payment
/// Methods endpoint.
///
/// - Raw card payments without external 3DS use `InAmexCard` as a fixed
///   placeholder (HS parity): resolving the correct per-country/per-funding type
///   from a bare PAN is not implemented, and the sandbox merchant accepts
///   `in_amex_card`.
/// - Digital wallets carry their network + funding, so they derive the
///   funding-specific `in_*` type (`in_credit_visa_card` / `in_debit_visa_card`,
///   etc.) via `try_from_wallet_network`.
/// - External 3DS (Mode B, `authentication_data` present) uses the `is_*` types
///   via `external_3ds_card_type`: every `in_*` card type rejects
///   `3d_version`/`cavv`/`eci`/`ds_trans_id` (its required fields list only
///   `3d_required`/`tavv`/`expiration_action`), while `is_visa_card` is the type
///   in Rapyd's own external-3DS example.
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
    IsVisaCard,
    IsMastercardCard,
    IsAmexCard,
}

impl RapydPaymentMethodType {
    /// Mode B (external 3DS) card type: the explicit card network, else the BIN
    /// issuer. Networks other than Visa / Mastercard / Amex are refused before
    /// the call rather than sent on a type Rapyd rejects (G-ThreeDS-09).
    fn external_3ds_card_type<T: PaymentMethodDataTypes>(
        card: &domain_types::payment_method_data::Card<T>,
    ) -> Result<Self, error_stack::Report<IntegrationError>> {
        let issuer = domain_types::utils::get_card_issuer(card.card_number.peek()).ok();
        match (card.card_network.as_ref(), issuer) {
            (Some(common_enums::CardNetwork::Visa), _)
            | (None, Some(domain_types::utils::CardIssuer::Visa)) => Ok(Self::IsVisaCard),
            (Some(common_enums::CardNetwork::Mastercard), _)
            | (None, Some(domain_types::utils::CardIssuer::Master)) => Ok(Self::IsMastercardCard),
            (Some(common_enums::CardNetwork::AmericanExpress), _)
            | (None, Some(domain_types::utils::CardIssuer::AmericanExpress)) => {
                Ok(Self::IsAmexCard)
            }
            _ => Err(IntegrationError::NotSupported {
                message: "rapyd external 3DS card network".to_owned(),
                connector: "rapyd",
                context: crate::utils::integration_ctx(
                    "rapyd external 3DS is sent on is_visa_card / is_mastercard_card / is_amex_card only",
                    "Use a Visa, Mastercard or Amex card for external 3DS on rapyd.",
                ),
            })?,
        }
    }

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
                context: crate::utils::integration_ctx(
                    "rapyd wallet payments support Visa, Mastercard and Amex networks only",
                    "Use a Visa, Mastercard or Amex card in the wallet.",
                ),
            })?,
        }
    }
}

/// Rapyd `initiation_type` for a merchant-initiated `/v1/payments` call,
/// derived from the MIT category (spec "Complete Endpoint Inventory >
/// RepeatPayment"). Rapyd's vocabulary also includes `customer_present` and
/// `moto`, which are never sent on the MIT path.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RapydInitiationType {
    Recurring,
    Installment,
    Unscheduled,
}

impl RapydInitiationType {
    /// `Recurring` and an absent category -> `recurring`; `Installment` ->
    /// `installment`; `Unscheduled` -> `unscheduled`. Rapyd has no value for a
    /// resubmitted MIT, so `Resubmission` is refused rather than guessed (UD-11).
    fn try_from_mit_category(
        mit_category: Option<&common_enums::MitCategory>,
    ) -> Result<Self, error_stack::Report<IntegrationError>> {
        match mit_category {
            None | Some(common_enums::MitCategory::Recurring) => Ok(Self::Recurring),
            Some(common_enums::MitCategory::Installment) => Ok(Self::Installment),
            Some(common_enums::MitCategory::Unscheduled) => Ok(Self::Unscheduled),
            Some(common_enums::MitCategory::Resubmission) => Err(IntegrationError::NotSupported {
                message: "mit_category resubmission".to_owned(),
                connector: "rapyd",
                context: crate::utils::integration_ctx(
                    "Rapyd initiation_type accepts recurring, installment and unscheduled for merchant-initiated payments; it has no resubmission value",
                    "Send mit_category RECURRING_MIT, INSTALLMENT_MIT or UNSCHEDULED_MIT for rapyd.",
                ),
            })?,
        }
    }
}

impl<F, T> TryFrom<ResponseRouterData<RapydPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<RapydPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let (status, response, connector_response) = match &item.response.data {
            Some(data) => {
                let attempt_status =
                    get_status(data.status.to_owned(), data.next_action.to_owned());
                // Diagnostic only (AVS/CVV/ACS checks, 3DS result, auth code):
                // surfaced on the success and the decline path, never an input
                // to `attempt_status`.
                let connector_response = build_rapyd_connector_response(data);
                match attempt_status {
                    common_enums::AttemptStatus::Failure => {
                        // A 2xx body carrying `status: ERR` is a failed payment
                        // (spec 3DS §5.2). Code preference mirrors hyperswitch:
                        // the bare network code first, then the qualified codes.
                        let code = non_empty(data.failure_code.as_deref())
                            .or_else(|| non_empty(Some(item.response.status.error_code.as_str())))
                            .or_else(|| non_empty(data.error_code.as_deref()))
                            .map(str::to_owned)
                            .unwrap_or_else(|| NO_ERROR_CODE.to_string());
                        let network = rapyd_network_error_fields(
                            &item.response.status,
                            RapydErrorDataView {
                                failure_code: data.failure_code.as_deref(),
                                error_code: data.error_code.as_deref(),
                                failure_message: data.failure_message.as_deref(),
                                merchant_advice_code: data.merchant_advice_code.as_deref(),
                            },
                        );
                        (
                            common_enums::AttemptStatus::Failure,
                            Err(ErrorResponse {
                                code,
                                status_code: item.http_code,
                                message: item
                                    .response
                                    .status
                                    .status
                                    .clone()
                                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                                reason: data.failure_message.to_owned(),
                                attempt_status: None,
                                connector_transaction_id: Some(data.id.clone()),
                                network_advice_code: network.advice_code,
                                network_decline_code: network.decline_code,
                                network_error_message: network.error_message,
                                typed_connector_response: None,
                                raw_connector_response: None,
                                raw_connector_request: None,
                                typed_connector_request: None,
                            }),
                            connector_response,
                        )
                    }
                    _ => {
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

                        // spec 3DS §5.3: an outstanding 3DS challenge with no page to
                        // send the cardholder to can never complete; fail loudly
                        // instead of parking the attempt in AuthenticationPending.
                        if matches!(data.next_action, Some(NextAction::ThreedsVerification))
                            && attempt_status == common_enums::AttemptStatus::AuthenticationPending
                            && redirection_url.is_none()
                        {
                            return Err(error_stack::report!(
                                crate::utils::response_handling_fail_for_connector(
                                    item.http_code,
                                    "rapyd",
                                )
                            ))
                            .attach_printable(
                                "rapyd: next_action 3d_verification without a redirect_url",
                            );
                        }

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
                                connector_response_reference_id: data
                                    .merchant_reference_id
                                    .to_owned(),
                                incremental_authorization_allowed: None,
                                status_code: item.http_code,
                                splits: None,
                                payment_account_reference: None,
                            }),
                            connector_response,
                        )
                    }
                }
            }
            None => {
                let network = rapyd_network_error_fields(
                    &item.response.status,
                    RapydErrorDataView::default(),
                );
                (
                    common_enums::AttemptStatus::Failure,
                    Err(ErrorResponse {
                        code: non_empty(Some(item.response.status.error_code.as_str()))
                            .map(str::to_owned)
                            .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                        status_code: item.http_code,
                        message: item
                            .response
                            .status
                            .status
                            .clone()
                            .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                        reason: item.response.status.message.clone(),
                        attempt_status: None,
                        connector_transaction_id: None,
                        network_advice_code: network.advice_code,
                        network_decline_code: network.decline_code,
                        network_error_message: network.error_message,
                        typed_connector_response: None,
                        raw_connector_response: None,
                        raw_connector_request: None,
                        typed_connector_request: None,
                    }),
                    None,
                )
            }
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_response: connector_response.or(item
                    .router_data
                    .resource_common_data
                    .connector_response
                    .clone()),
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

/// `POST /v1/payments` body (Authorize, SetupMandate and RepeatPayment).
///
/// Deliberately absent: a shipping address (Rapyd's payment object has no
/// shipping field — spec "Operator Requirements Coverage" row 17) and L2/L3
/// commercial-card data (not supported by Rapyd — row 21). The new optional
/// keys below are skipped when `None`, so the signed body of a request that
/// carries none of their inputs stays byte-identical to what shipped before.
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
    /// credential; derived from the MIT category (see [`RapydInitiationType`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initiation_type: Option<RapydInitiationType>,
    /// Top-level billing address (never the shipping address). Rapyd uses it
    /// for AVS when the merchant is entitled; `avs_required` is never sent
    /// (sandbox rejects it: UNKNOWN_PAYMENT_METHOD_FIELD - [AVS_REQUIRED]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<Address>,
    /// Dynamic statement descriptor, 5-22 characters — see
    /// [`rapyd_statement_descriptor`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statement_descriptor: Option<String>,
    /// Receipt email; sent on the Rapyd-hosted 3DS path only ("required for
    /// Visa 3DS", spec 3DS §8.3 rule 4).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt_email: Option<Email>,
    /// Browser data for the issuer's 3DS risk assessment; Rapyd-hosted 3DS
    /// path only (spec 3DS §7.5, §8.3 rule 3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_details: Option<RapydClientDetails>,
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

/// `payment_method_options` for a card payment. `3d_required` is always sent
/// (a JSON boolean — spec 3DS §7.2.1); every other key is skipped when `None`.
/// `xid` and `tavv` are deliberately absent (spec 3DS §6.3, §6.7).
#[derive(Debug, Default, Serialize)]
pub struct PaymentMethodOptions {
    #[serde(rename = "3d_required")]
    pub three_ds: bool,
    // ---- External 3DS pass-through (Mode B), spec 3DS §6.1-§6.2 ----
    #[serde(rename = "3d_version", skip_serializing_if = "Option::is_none")]
    pub three_ds_version: Option<RapydThreeDsVersion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cavv: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eci: Option<RapydEciRequest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds_trans_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sca_exemption: Option<RapydScaExemption>,
}

/// Request-side `3d_version`; Rapyd validates `(1.0.2|2.1.0|2.2.0)` (spec 3DS §4.5).
#[derive(Debug, Clone, Copy, Serialize)]
pub enum RapydThreeDsVersion {
    #[serde(rename = "1.0.2")]
    V1_0_2,
    #[serde(rename = "2.1.0")]
    V2_1_0,
    #[serde(rename = "2.2.0")]
    V2_2_0,
}

impl RapydThreeDsVersion {
    fn is_v2(self) -> bool {
        matches!(self, Self::V2_1_0 | Self::V2_2_0)
    }
}

/// Request-side ECI; Rapyd validates `(01|02|05|06|07|08)` (spec 3DS §4.3).
/// Distinct from the response-side value set, which is kept as a string.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum RapydEciRequest {
    #[serde(rename = "01")]
    E01,
    #[serde(rename = "02")]
    E02,
    #[serde(rename = "05")]
    E05,
    #[serde(rename = "06")]
    E06,
    #[serde(rename = "07")]
    E07,
    #[serde(rename = "08")]
    E08,
}

/// Request-side `sca_exemption` (spec 3DS §4.6, §6.2).
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RapydScaExemption {
    LowValue,
    TransactionRiskAnalysis,
    AuthenticationOutage,
    SecureCorporatePayments,
}

/// Rapyd's accepted `screen_color_depth` values (spec 3DS §7.5).
const RAPYD_SCREEN_COLOR_DEPTHS: [u8; 8] = [1, 4, 8, 15, 16, 24, 32, 48];

/// `client_details` for the Rapyd-hosted 3DS challenge (spec 3DS §7.5).
/// `time_zone_offset` is omitted: its sign convention is unverified (UD-09).
#[derive(Debug, Serialize)]
pub struct RapydClientDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_address: Option<Secret<String, IpAddress>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accept_header: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screen_height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screen_width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screen_color_depth: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub java_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub java_script_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

impl From<&BrowserInformation> for RapydClientDetails {
    fn from(browser_info: &BrowserInformation) -> Self {
        Self {
            // Never hard-required (spec 3DS §8.3 rule 3): absent -> key omitted.
            ip_address: browser_info.get_ip_address().ok(),
            accept_header: browser_info.accept_header.clone(),
            screen_height: browser_info.screen_height,
            screen_width: browser_info.screen_width,
            screen_color_depth: browser_info
                .color_depth
                .filter(|depth| RAPYD_SCREEN_COLOR_DEPTHS.contains(depth)),
            java_enabled: browser_info.java_enabled,
            java_script_enabled: browser_info.java_script_enabled,
            language: browser_info.language.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PaymentMethod<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> {
    #[serde(rename = "type")]
    pub pm_type: RapydPaymentMethodType,
    pub fields: Option<RapydPaymentFields<T>>,
    pub address: Option<Address>,
    pub digital_wallet: Option<RapydWallet>,
}

/// `payment_method.fields`: full card details for a customer-present or
/// card-save payment, or the card + network reference id of a stored-credential
/// MIT (Variant B). Serialized untagged, so the wire keys are the inner struct's.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum RapydPaymentFields<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> {
    Card(PaymentFields<T>),
    NetworkTransaction(RapydNetworkTransactionFields),
}

/// Card fields for a merchant-initiated payment keyed on the network reference
/// id of the original customer-initiated transaction. No `cvv`: the cardholder
/// is not present on a MIT.
/// Reference: https://docs.rapyd.net/en/creating-a-card-payment-with-a-network-reference-id.html
#[derive(Debug, Serialize)]
pub struct RapydNetworkTransactionFields {
    pub number: cards::CardNumber,
    pub expiration_month: Secret<String>,
    pub expiration_year: Secret<String>,
    pub name: Secret<String>,
    pub network_reference_id: Secret<String>,
}

#[derive(Default, Debug, Serialize)]
pub struct PaymentFields<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> {
    pub number: RawCardNumber<T>,
    pub expiration_month: Secret<String>,
    pub expiration_year: Secret<String>,
    pub name: Secret<String>,
    pub cvv: Secret<String>,
}

/// Rapyd address object (`name` and `line_1` are required by Rapyd).
#[derive(Default, Debug, Serialize)]
pub struct Address {
    name: Secret<String>,
    line_1: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line_2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line_3: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    country: Option<common_enums::CountryAlpha2>,
    #[serde(skip_serializing_if = "Option::is_none")]
    zip: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    phone_number: Option<Secret<String>>,
}

/// Builds Rapyd's top-level `address` from the **billing** address only —
/// Rapyd has no shipping field on a payment, so shipping is never read here
/// (spec "Operator Requirements Coverage" rows 16-17). Returns `None` unless
/// both a billing `line1` and a name (billing full name, else the card holder
/// name passed in) exist, because Rapyd requires both.
fn rapyd_billing_address(
    flow_data: &PaymentFlowData,
    card_holder_name: Option<&Secret<String>>,
) -> Option<Address> {
    let line_1 = flow_data.get_optional_billing_line1()?;
    let name = flow_data
        .get_optional_billing_full_name()
        .or_else(|| card_holder_name.cloned())?;
    Some(Address {
        name,
        line_1,
        line_2: flow_data.get_optional_billing_line2(),
        line_3: flow_data.get_optional_billing_line3(),
        city: flow_data.get_optional_billing_city(),
        state: flow_data.get_optional_billing_state(),
        country: flow_data.get_optional_billing_country(),
        zip: flow_data.get_optional_billing_zip(),
        phone_number: flow_data.get_optional_billing_phone_number(),
    })
}

/// Rapyd documents `statement_descriptor` as 5-22 characters
/// (<https://docs.rapyd.net/en/create-payment.html>). Trim, then:
///   - fewer than 5 chars (incl. empty) -> `None` (key omitted; the
///     account-level descriptor applies instead of failing the payment)
///   - more than 22 chars -> truncated to the first 22 characters
///
/// Truncation counts `chars()`, never bytes, so a multi-byte character cannot
/// split. No charset filter: Rapyd's own accepted example contradicts its
/// charset rule, so Rapyd adjudicates (spec enrichment §3.2, UD-06).
fn rapyd_statement_descriptor(descriptor: Option<&BillingDescriptor>) -> Option<String> {
    let raw = descriptor?.statement_descriptor.as_deref()?.trim();
    if raw.chars().count() < 5 {
        return None;
    }
    Some(raw.chars().take(22).collect())
}

#[derive(Debug, Clone, Serialize)]
pub struct RapydWallet {
    #[serde(rename = "type")]
    payment_type: String,
    details: RapydWalletDetails,
}

/// Rapyd's `digital_wallet.details` is either the raw encrypted token (Rapyd
/// decrypts server-side) or a decrypted payload the merchant already decrypted
/// with its own keys. Hyperswitch decrypts Apple Pay / Google Pay upstream, so
/// the decrypted variants are what UCS receives and forwards.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum RapydWalletDetails {
    ApplePayDecrypted(Box<RapydApplePayDecryptedDetails>),
    GooglePayDecrypted(Box<RapydGooglePayDecryptedDetails>),
    Token(Secret<String>),
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

        // Rapyd captures once (capture moves the payment to CLO); only
        // Automatic / Manual / SequentialAutomatic are offered (HS parity).
        if let Some(
            capture_method @ (common_enums::CaptureMethod::ManualMultiple
            | common_enums::CaptureMethod::Scheduled),
        ) = item.router_data.request.capture_method
        {
            return Err(IntegrationError::NotSupported {
                message: format!("capture_method {capture_method:?}"),
                connector: "rapyd",
                context: crate::utils::integration_ctx(
                    "Rapyd captures an authorized payment once; multiple or scheduled captures are not offered",
                    "Use capture_method AUTOMATIC or MANUAL for rapyd.",
                ),
            })?;
        }
        // Capture intent applies to every payment method.
        let capture = Some(item.router_data.request.is_auto_capture());
        let is_card_payment = matches!(
            item.router_data.resource_common_data.payment_method,
            common_enums::PaymentMethod::Card
        );
        let wants_rapyd_3ds = matches!(
            item.router_data.resource_common_data.auth_type,
            common_enums::AuthenticationType::ThreeDs
        );
        // Card-arm outputs: the external-3DS (Mode B) options and the top-level
        // billing address. Wallet / token arms send neither (HS parity).
        let mut external_3ds_options: Option<PaymentMethodOptions> = None;
        let mut address: Option<Address> = None;
        let payment_method = match item.router_data.request.payment_method_data {
            PaymentMethodData::Card(ref ccard) => {
                // Mode B (spec 3DS §8.1): the merchant already authenticated;
                // it takes precedence over `auth_type`. Mode B needs a type whose
                // required fields carry the external-3DS options; `in_*` types
                // reject them (400 UNKNOWN_PAYMENT_METHOD_FIELD - [3D_VERSION]).
                // Every other card payment keeps the `in_amex_card` placeholder
                // (UD-13, HS parity). The network is decided once, here.
                let mode_b_pm_type = item
                    .router_data
                    .request
                    .authentication_data
                    .as_ref()
                    .map(|authentication_data| {
                        let pm_type = RapydPaymentMethodType::external_3ds_card_type(ccard)?;
                        let is_mastercard =
                            matches!(pm_type, RapydPaymentMethodType::IsMastercardCard);
                        external_3ds_options = Some(rapyd_external_3ds_options(
                            authentication_data,
                            is_mastercard,
                        )?);
                        Ok::<_, error_stack::Report<IntegrationError>>(pm_type)
                    })
                    .transpose()?;
                let pm_type = mode_b_pm_type.unwrap_or(RapydPaymentMethodType::InAmexCard);
                // spec "Field Dependency Analysis > ThreeDS": card holder name,
                // else billing full name. The empty-string last resort is HS
                // parity (UD-10): the sandbox accepts it and refusing would
                // regress payments hyperswitch sends today.
                let card_holder_name = ccard
                    .card_holder_name
                    .clone()
                    .or_else(|| {
                        item.router_data
                            .resource_common_data
                            .get_optional_billing_full_name()
                    })
                    .unwrap_or_else(|| Secret::new(String::new()));
                address = rapyd_billing_address(
                    &item.router_data.resource_common_data,
                    ccard.card_holder_name.as_ref(),
                );
                RapydPaymentMethodData::PaymentMethod(Box::new(PaymentMethod {
                    pm_type,
                    fields: Some(RapydPaymentFields::Card(PaymentFields {
                        number: ccard.card_number.to_owned(),
                        expiration_month: ccard.card_exp_month.to_owned(),
                        expiration_year: ccard.card_exp_year.to_owned(),
                        name: card_holder_name,
                        cvv: ccard.card_cvc.to_owned(),
                    })),
                    // The billing address goes top-level (UD-08); the nested
                    // copy stays unset so it is never sent twice.
                    address: None,
                    digital_wallet: None,
                }))
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
                                RapydWalletDetails::Token(Secret::new(
                                    data.tokenization_data
                                        .get_encrypted_google_pay_token()
                                        .change_context(IntegrationError::MissingRequiredField {
                                            field_name: "gpay wallet_token",
                                            context: crate::utils::integration_ctx(
                                                "Encrypted Google Pay payload has no wallet token",
                                                "Ensure the Google Pay tokenization data includes the token.",
                                            ),
                                        })?
                                        .to_owned(),
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
                                RapydWalletDetails::Token(Secret::new(
                                    apple_pay_encrypted_data.to_string(),
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
                        context: crate::utils::integration_ctx(
                            "rapyd Authorize accepts Google Pay and Apple Pay wallets only",
                            "Use a card, Google Pay or Apple Pay for rapyd.",
                        ),
                    })?,
                };
                RapydPaymentMethodData::PaymentMethod(Box::new(PaymentMethod {
                    pm_type,
                    fields: None,
                    address: None,
                    digital_wallet: Some(rapyd_wallet),
                }))
            }
            PaymentMethodData::PaymentMethodToken(ref token_data) => {
                RapydPaymentMethodData::Token(token_data.token.clone())
            }
            // Network-transaction-id card details belong to RepeatPayment (MIT).
            PaymentMethodData::CardDetailsForNetworkTransactionId(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_) => {
                Err(IntegrationError::NotImplemented(
                    "payment_method".to_owned(),
                    crate::utils::integration_ctx(
                        "rapyd Authorize does not accept network-transaction-id payment data; merchant-initiated payments go through RepeatPayment",
                        "Send the NTID MIT through RecurringPaymentService.Charge.",
                    ),
                ))?
            }
            // No `tavv` support: network tokens are refused (spec 3DS §6.7).
            PaymentMethodData::NetworkToken(_) => Err(IntegrationError::NotImplemented(
                "payment_method".to_owned(),
                crate::utils::integration_ctx(
                    "rapyd Authorize does not support network tokens",
                    "Send the card details instead of a network token.",
                ),
            ))?,
            PaymentMethodData::MandatePayment => Err(IntegrationError::NotImplemented(
                "payment_method".to_owned(),
                crate::utils::integration_ctx(
                    "rapyd Authorize does not charge mandates; merchant-initiated payments go through RepeatPayment",
                    "Send the mandate charge through RecurringPaymentService.Charge.",
                ),
            ))?,
            PaymentMethodData::CardWithNoCvc(_) => Err(IntegrationError::NotImplemented(
                "payment_method".to_owned(),
                crate::utils::integration_ctx(
                    "rapyd Authorize requires the card CVV",
                    "Send the card with its CVV.",
                ),
            ))?,
            PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::PayLater(_)
            | PaymentMethodData::BankRedirect(_)
            | PaymentMethodData::BankDebit(_)
            | PaymentMethodData::BankTransfer(_)
            | PaymentMethodData::Crypto(_)
            | PaymentMethodData::Reward
            | PaymentMethodData::RealTimePayment(_)
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::MobilePayment(_) => Err(IntegrationError::NotImplemented(
                "payment_method".to_owned(),
                crate::utils::integration_ctx(
                    "rapyd Authorize implements card, Google Pay, Apple Pay and payment-method tokens only",
                    "Use a card, Google Pay or Apple Pay for rapyd.",
                ),
            ))?,
        };
        // Only a CIT that stores a mandate (setup_future_usage = OFF_SESSION)
        // asks Rapyd to save the card and create an inline customer, so the
        // response carries the reusable `card_*` token for later MIT calls.
        // ON_SESSION is not a mandate request (HS saves on-session cards in its
        // own locker; HS Direct Rapyd ignores setup_future_usage): charge it as
        // a plain payment with neither field. Rapyd cannot save a payment method
        // without a customer, so OFF_SESSION requires name and email (fail closed).
        let (customer, save_payment_method) = if item.router_data.request.setup_future_usage
            == Some(common_enums::FutureUsage::OffSession)
        {
            let customer_name = item
                    .router_data
                    .request
                    .get_optional_customer_name()
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "customer.name",
                        context: crate::utils::integration_ctx(
                            "Rapyd creates an inline customer to save the payment method, which requires a name",
                            "Send the customer name when setup_future_usage is OFF_SESSION.",
                        ),
                    })?;
            let customer_email = item.router_data.request.get_optional_email().ok_or(
                IntegrationError::MissingRequiredField {
                    field_name: "customer.email",
                    context: crate::utils::integration_ctx(
                        "Rapyd's inline customer requires an email",
                        "Send the customer email when setup_future_usage is OFF_SESSION.",
                    ),
                },
            )?;
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

        // 3DS mode selection (spec 3DS §8.1): Mode B (external pass-through)
        // wins over `auth_type`; Mode A asks Rapyd to host the challenge;
        // Mode N sends `3d_required: false`. Non-card payments send no
        // `payment_method_options` (wallets carry their own authentication).
        let is_mode_b = external_3ds_options.is_some();
        let payment_method_options = external_3ds_options.or_else(|| {
            is_card_payment.then(|| PaymentMethodOptions {
                three_ds: wants_rapyd_3ds,
                ..Default::default()
            })
        });
        // Browser data and receipt email feed the Rapyd-hosted challenge only
        // (Mode A, spec 3DS §8.3); Mode B and Mode N send neither.
        let is_mode_a = payment_method_options
            .as_ref()
            .is_some_and(|options| options.three_ds)
            && !is_mode_b;
        let (client_details, receipt_email) = if is_mode_a {
            (
                item.router_data
                    .request
                    .browser_info
                    .as_ref()
                    .map(RapydClientDetails::from),
                item.router_data.request.get_optional_email(),
            )
        } else {
            (None, None)
        };
        let statement_descriptor =
            rapyd_statement_descriptor(item.router_data.request.billing_descriptor.as_ref());

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
            address,
            statement_descriptor,
            receipt_email,
            client_details,
        })
    }
}

/// Builds the Mode B (external 3DS pass-through) `payment_method_options`
/// from our `AuthenticationData` (spec 3DS §6.1-§6.2, §8.2). Only `cavv`,
/// `eci`, `message_version`, `ds_trans_id` and `exemption_indicator` are
/// mapped; `trans_status` is a local guard and is never serialised.
fn rapyd_external_3ds_options(
    authentication_data: &AuthenticationData,
    is_mastercard: bool,
) -> Result<PaymentMethodOptions, error_stack::Report<IntegrationError>> {
    const DOC_URL: &str = "https://docs.rapyd.net/en/creating-a-card-payment-with-3ds-authentication---rapyd-3ds.html";
    let ctx = |additional_context: &str, suggested_action: &str| IntegrationErrorContext {
        additional_context: Some(additional_context.to_owned()),
        suggested_action: Some(suggested_action.to_owned()),
        doc_url: Some(DOC_URL.to_owned()),
    };
    // spec 3DS §8.2 rule 1: a pass-through without a cryptogram is not external 3DS.
    let cavv = authentication_data
        .cavv
        .clone()
        .ok_or(IntegrationError::MissingRequiredField {
            field_name: "authentication_data.cavv",
            context: ctx(
                "rapyd external 3DS pass-through requires the CAVV from the authentication",
                "Send authentication_data.cavv, or omit authentication_data to let Rapyd run 3DS.",
            ),
        })?;
    // spec 3DS §8.2 rule 2: only Y (authenticated) or A (attempted) carry a
    // liability shift worth passing through.
    match authentication_data.trans_status {
        Some(common_enums::TransactionStatus::Success)
        | Some(common_enums::TransactionStatus::NotVerified) => {}
        ref other => Err(IntegrationError::NotSupported {
            message: format!("authentication_data.trans_status {other:?}"),
            connector: "rapyd",
            context: ctx(
                "rapyd external 3DS pass-through accepts trans_status Y or A only",
                "Pass through only successful or attempted authentications.",
            ),
        })?,
    }
    // spec 3DS §4.5, §8.2 rule 3: Rapyd validates 3d_version against a closed set.
    let message_version = authentication_data
        .message_version
        .as_ref()
        .map(ToString::to_string);
    let three_ds_version = match message_version.as_deref() {
        Some("1.0.2") => RapydThreeDsVersion::V1_0_2,
        Some("2.1.0") => RapydThreeDsVersion::V2_1_0,
        Some("2.2.0") => RapydThreeDsVersion::V2_2_0,
        other => Err(IntegrationError::NotSupported {
            message: format!("authentication_data.message_version {other:?}"),
            connector: "rapyd",
            context: ctx(
                "rapyd accepts 3d_version 1.0.2, 2.1.0 or 2.2.0 only",
                "Send a supported authentication_data.message_version.",
            ),
        })?,
    };
    // spec 3DS §4.3, §8.2 rule 3: request-side ECI regex (01|02|05|06|07|08).
    let eci = match authentication_data.eci.as_deref() {
        Some("01") => RapydEciRequest::E01,
        Some("02") => RapydEciRequest::E02,
        Some("05") => RapydEciRequest::E05,
        Some("06") => RapydEciRequest::E06,
        Some("07") => RapydEciRequest::E07,
        Some("08") => RapydEciRequest::E08,
        other => Err(IntegrationError::NotSupported {
            message: format!("authentication_data.eci {other:?}"),
            connector: "rapyd",
            context: ctx(
                "rapyd accepts eci 01, 02, 05, 06, 07 or 08 only",
                "Send a two-digit authentication_data.eci from the supported set.",
            ),
        })?,
    };
    // spec 3DS §6.1, §8.2 rule 4: ds_trans_id is required for Mastercard 2.x.
    if three_ds_version.is_v2() && authentication_data.ds_trans_id.is_none() && is_mastercard {
        Err(IntegrationError::MissingRequiredField {
            field_name: "authentication_data.ds_trans_id",
            context: ctx(
                "rapyd requires ds_trans_id for Mastercard 3DS 2.x",
                "Send authentication_data.ds_trans_id for Mastercard.",
            ),
        })?;
    }
    // spec 3DS §6.2: only four exemptions have a Rapyd value; others are omitted.
    let sca_exemption = match authentication_data.exemption_indicator {
        Some(common_enums::ExemptionIndicator::LowValue) => Some(RapydScaExemption::LowValue),
        Some(common_enums::ExemptionIndicator::TransactionRiskAssessment) => {
            Some(RapydScaExemption::TransactionRiskAnalysis)
        }
        Some(common_enums::ExemptionIndicator::ThreeDsOutage) => {
            Some(RapydScaExemption::AuthenticationOutage)
        }
        Some(common_enums::ExemptionIndicator::SecureCorporatePayment) => {
            Some(RapydScaExemption::SecureCorporatePayments)
        }
        Some(
            common_enums::ExemptionIndicator::TrustedListing
            | common_enums::ExemptionIndicator::ScaDelegation
            | common_enums::ExemptionIndicator::OutOfScaScope
            | common_enums::ExemptionIndicator::Other
            | common_enums::ExemptionIndicator::LowRiskProgram
            | common_enums::ExemptionIndicator::RecurringOperation,
        )
        | None => None,
    };
    Ok(PaymentMethodOptions {
        // UD-07: Mode B never asks Rapyd to run its own challenge.
        three_ds: false,
        three_ds_version: Some(three_ds_version),
        cavv: Some(cavv),
        eci: Some(eci),
        ds_trans_id: authentication_data.ds_trans_id.clone(),
        sca_exemption,
    })
}

/// Rapyd payment `data.status` (spec 3DS §4.1; https://docs.rapyd.net/en/retrieve-payment.html).
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
    /// Any status Rapyd adds later: parses instead of failing the whole
    /// payment / webhook body, and maps to a non-terminal status.
    #[serde(other)]
    Unknown,
}

/// The one Rapyd payment status map, shared by every payment flow and the
/// payment webhook. A pure function of `(status, next_action)`:
/// `authentication_result` and the AVS/CVV checks are never inputs
/// (spec 3DS §5.4, errors §8.2 rule 5).
pub(super) fn get_status(
    status: RapydPaymentStatus,
    next_action: Option<NextAction>,
) -> common_enums::AttemptStatus {
    match (status, next_action) {
        // spec: Status Mappings > Payment status — CLO is closed/completed,
        // whatever next_action still says (3DS §5.1).
        (RapydPaymentStatus::Closed, _) => common_enums::AttemptStatus::Charged,
        // spec 3DS §4.2 / §5.1: challenge outstanding or awaiting confirmation.
        (
            RapydPaymentStatus::Active,
            Some(NextAction::ThreedsVerification | NextAction::PendingConfirmation),
        ) => common_enums::AttemptStatus::AuthenticationPending,
        // spec 3DS §4.2 / §5.1: authorised, capture outstanding (online or
        // offline) or nothing further required.
        (
            RapydPaymentStatus::Active,
            Some(
                NextAction::PendingCapture
                | NextAction::PendingOfflineCapture
                | NextAction::NotApplicable,
            ),
        ) => common_enums::AttemptStatus::Authorized,
        // spec 3DS §7.3 / UD-14: an unknown or absent next_action on ACT is not
        // guessed into Authorized; stay non-terminal so PSync resolves it.
        (RapydPaymentStatus::Active, Some(NextAction::Unknown) | None) => {
            common_enums::AttemptStatus::Pending
        }
        // spec: Status Mappings > Payment status — canceled, expired, reversed.
        (
            RapydPaymentStatus::CanceledByClientOrBank
            | RapydPaymentStatus::Expired
            | RapydPaymentStatus::ReversedByRapyd,
            _,
        ) => common_enums::AttemptStatus::Voided,
        // spec 3DS §5.2: ERR is a failed payment.
        (RapydPaymentStatus::Error, _) => common_enums::AttemptStatus::Failure,
        // hs rapyd/transformers.rs get_status: NEW is a transient create state.
        (RapydPaymentStatus::New, _) => common_enums::AttemptStatus::Authorizing,
        // spec: Rust Type Design > Required changes (2) — unknown status is
        // non-terminal.
        (RapydPaymentStatus::Unknown, _) => common_enums::AttemptStatus::Pending,
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

/// Rapyd payment `data.next_action` (spec 3DS §4.2, §7.3).
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
    /// Any value Rapyd adds later; must not fail the whole body.
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResponseData {
    pub id: String,
    pub amount: FloatMajorUnit,
    pub status: RapydPaymentStatus,
    /// Absent on a `PAYMENT_FAILED` webhook body (UD-02).
    pub next_action: Option<NextAction>,
    pub redirect_url: Option<String>,
    pub original_amount: Option<FloatMajorUnit>,
    pub is_partial: Option<bool>,
    pub currency_code: Option<common_enums::Currency>,
    pub country_code: Option<String>,
    pub captured: Option<bool>,
    /// Absent on a `PAYMENT_FAILED` webhook body (UD-02).
    pub transaction_id: Option<String>,
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
    /// Issuer authorization code.
    pub auth_code: Option<String>,
    /// `$.data.error_code` — the fully-qualified code, e.g.
    /// `"ERROR_PROCESSING_CARD - [65]"`; `""` when there is no failure.
    pub error_code: Option<String>,
    /// Merchant Advice Code (`"01"`..`"18"`, zero-padded string). May ride on
    /// a successful payment too (15/16), so it is read on the error path only.
    pub merchant_advice_code: Option<String>,
    /// Retry advice text for the MAC; never a decline reason (errors §6.3).
    pub merchant_advice_message: Option<String>,
    /// Risk-assessment outcome; parsed only, never a decision input (errors §4.1).
    pub outcome: Option<RapydOutcome>,
    /// 3DS result; diagnostic only, never a status input (spec 3DS §5.4).
    pub authentication_result: Option<RapydAuthenticationResult>,
}

/// Subset of Rapyd's response `payment_method_data` object.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RapydResponsePaymentMethodData {
    pub network_reference_id: Option<Secret<String>>,
    /// Access Control Server (3DS) check. Diagnostic only.
    pub acs_check: Option<RapydCheckResult>,
    /// CVV verification. Diagnostic only: a `Fail` does not by itself mean
    /// the payment failed.
    pub cvv_check: Option<RapydCheckResult>,
    /// AVS check. Its value set is not documented by Rapyd, so it stays an
    /// opaque string (errors §2.5).
    pub avs_check: Option<String>,
    /// The AVS field Rapyd's one `avs_required: true` example returns;
    /// value set not documented (errors §2.4-§2.5).
    pub avs_result: Option<String>,
}

/// `acs_check` / `cvv_check` closed set (errors §3.2; https://docs.rapyd.net/en/retrieve-payment.html).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RapydCheckResult {
    Pass,
    Fail,
    Unavailable,
    Unchecked,
    #[serde(other)]
    Unknown,
}

/// `$.data.outcome` (errors §4.1, §7.3). Every field optional; parsed only.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RapydOutcome {
    pub network_status: Option<RapydNetworkStatus>,
    pub risk_level: Option<String>,
    pub seller_message: Option<String>,
    #[serde(rename = "type")]
    pub outcome_type: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RapydNetworkStatus {
    ApprovedByNetwork,
    DeclinedByNetwork,
    NotSentToNetwork,
    ReversedAfterApproval,
    #[serde(other)]
    Unknown,
}

/// `$.data.authentication_result` (spec 3DS §4.3, §7.4). Every field is
/// optional: `eci` and `cardholder_info` are null on the pending leg.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RapydAuthenticationResult {
    pub result: Option<RapydAuthResult>,
    /// Protocol version as returned (e.g. "2.2.0"); not the request-side set.
    pub version: Option<String>,
    /// Response-side ECI (05/02, 06/01, 07/00); permissive string.
    pub eci: Option<String>,
    /// Issuer message, max 128 chars.
    pub cardholder_info: Option<String>,
}

/// `authentication_result.result` (spec 3DS §4.3).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum RapydAuthResult {
    #[serde(rename = "A")]
    Authenticated,
    #[serde(rename = "N")]
    NotAuthenticated,
    #[serde(rename = "R")]
    RedirectionPending,
    #[serde(rename = "U")]
    Unsupported,
    #[serde(other)]
    Unknown,
}

/// Error body parsed by `build_error_response` (errors §1.B / §1.C). The
/// Create-Payment error `data` is a reduced payment object without `id`,
/// `amount` or `next_action`, so it cannot be parsed as [`ResponseData`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RapydErrorResponse {
    pub status: Status,
    pub data: Option<RapydErrorData>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RapydErrorData {
    pub id: Option<String>,
    pub status: Option<RapydPaymentStatus>,
    pub error_code: Option<String>,
    pub failure_code: Option<String>,
    pub failure_message: Option<String>,
    pub merchant_advice_code: Option<String>,
    pub merchant_advice_message: Option<String>,
}

/// Error classes of errors §4.2: only an issuer decline feeds the GSM fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RapydErrorClass {
    /// (a) the card network or the customer refused.
    IssuerDecline,
    /// (b) Rapyd rejected the request itself; not a card decline.
    MerchantRequest,
    /// (c) auth / signature / idempotency / general / rate-limit; the request
    /// never became a payment attempt.
    Transport,
}

/// Rapyd `status.error_code` values that are transport-level (errors §4.5).
const RAPYD_TRANSPORT_ERROR_CODES: [&str; 5] = [
    "MISSING_AUTHENTICATION_HEADERS",
    "UNAUTHENTICATED_API_CALL",
    "IDEMPOTENCY_ERROR",
    "GENERAL_ERROR",
    "ERROR_REPORTS_RATE_LIMIT_EXCEEDED",
];

/// Prefix of Rapyd's card-network decline codes (errors §5.1).
const RAPYD_CARD_DECLINE_PREFIX: &str = "ERROR_PROCESSING_CARD";

impl RapydErrorClass {
    /// errors §4.6 discriminator, cheapest first.
    pub fn classify(status_error_code: &str, failure_code: Option<&str>) -> Self {
        if status_error_code.starts_with(RAPYD_CARD_DECLINE_PREFIX) {
            Self::IssuerDecline
        } else if RAPYD_TRANSPORT_ERROR_CODES.contains(&status_error_code) {
            Self::Transport
        } else if non_empty(failure_code).is_some() {
            Self::IssuerDecline
        } else {
            Self::MerchantRequest
        }
    }
}

/// The error-bearing `data.*` fields, borrowed from whichever Rapyd body
/// shape is at hand ([`ResponseData`] or [`RapydErrorData`]).
#[derive(Debug, Clone, Copy, Default)]
pub struct RapydErrorDataView<'a> {
    pub failure_code: Option<&'a str>,
    pub error_code: Option<&'a str>,
    pub failure_message: Option<&'a str>,
    pub merchant_advice_code: Option<&'a str>,
}

/// The GSM triple for `ErrorResponse`, plus the class it was derived from.
#[derive(Debug, Clone)]
pub struct RapydNetworkErrorFields {
    pub class: RapydErrorClass,
    pub decline_code: Option<String>,
    pub advice_code: Option<String>,
    pub error_message: Option<String>,
}

/// `Some(s)` when `s` is present and not blank.
fn non_empty(value: Option<&str>) -> Option<&str> {
    value.filter(|s| !s.trim().is_empty())
}

/// Extracts the bracketed card-network code from a qualified Rapyd code such
/// as `"ERROR_PROCESSING_CARD - [65]"`. The token is an opaque alphanumeric
/// string (`OF`, `5C`, `N7` …) — never parsed as a number (errors §5.1).
fn network_code_from_qualified(code: &str) -> Option<String> {
    let start = code.find('[')?;
    let end = code.get(start..)?.find(']')? + start;
    let token = code.get(start + 1..end)?.trim();
    (!token.is_empty()).then(|| token.to_string())
}

/// Strips one surrounding `[` `]` pair: Rapyd's prose and its examples
/// disagree on whether the short message is bracketed (errors §5.2, H-4).
fn strip_brackets(message: &str) -> String {
    let trimmed = message.trim();
    trimmed
        .strip_prefix('[')
        .and_then(|inner| inner.strip_suffix(']'))
        .unwrap_or(trimmed)
        .trim()
        .to_string()
}

/// GSM smart-retry inputs (errors §4.6, §6.1-§6.4, §8.2). Populated only for
/// an issuer decline; merchant-request and transport errors are never
/// reported as card declines.
///   - decline code: `data.failure_code`, else the bracket group of
///     `data.error_code`, else of `status.error_code`
///   - advice code: `data.merchant_advice_code`, verbatim
///   - message: `data.failure_message`, else `status.message`, brackets
///     stripped. `merchant_advice_message` never goes here.
pub fn rapyd_network_error_fields(
    status: &Status,
    data: RapydErrorDataView<'_>,
) -> RapydNetworkErrorFields {
    let class = RapydErrorClass::classify(status.error_code.as_str(), data.failure_code);
    if class != RapydErrorClass::IssuerDecline {
        return RapydNetworkErrorFields {
            class,
            decline_code: None,
            advice_code: None,
            error_message: None,
        };
    }
    let decline_code = non_empty(data.failure_code)
        .map(|code| code.trim().to_string())
        .or_else(|| non_empty(data.error_code).and_then(network_code_from_qualified))
        .or_else(|| network_code_from_qualified(status.error_code.as_str()));
    let advice_code = non_empty(data.merchant_advice_code).map(str::to_string);
    let error_message = non_empty(data.failure_message)
        .or_else(|| non_empty(status.message.as_deref()))
        .map(strip_brackets)
        .filter(|message| !message.is_empty());
    RapydNetworkErrorFields {
        class,
        decline_code,
        advice_code,
        error_message,
    }
}

/// Folds Rapyd's diagnostic card results into the UCS connector response:
/// `payment_checks` carries `cvv_check`, `acs_check`, `avs_check` and
/// `avs_result` (each key only when Rapyd sent it), `authentication_data` the
/// 3DS `authentication_result`, plus the issuer auth code. Diagnostic only —
/// never feeds the attempt status (errors §8.2 rule 5, spec 3DS §5.4).
fn build_rapyd_connector_response(data: &ResponseData) -> Option<ConnectorResponseData> {
    let mut payment_checks = serde_json::Map::new();
    if let Some(pmd) = data.payment_method_data.as_ref() {
        if let Some(cvv_check) = pmd.cvv_check {
            payment_checks.insert("cvv_check".to_string(), serde_json::json!(cvv_check));
        }
        if let Some(acs_check) = pmd.acs_check {
            payment_checks.insert("acs_check".to_string(), serde_json::json!(acs_check));
        }
        if let Some(avs_check) = pmd.avs_check.as_ref() {
            payment_checks.insert("avs_check".to_string(), serde_json::json!(avs_check));
        }
        if let Some(avs_result) = pmd.avs_result.as_ref() {
            payment_checks.insert("avs_result".to_string(), serde_json::json!(avs_result));
        }
    }
    let authentication_data = data
        .authentication_result
        .as_ref()
        .and_then(|result| serde_json::to_value(result).ok());
    let auth_code = non_empty(data.auth_code.as_deref()).map(str::to_string);
    if payment_checks.is_empty() && authentication_data.is_none() && auth_code.is_none() {
        return None;
    }
    Some(ConnectorResponseData::with_additional_payment_method_data(
        AdditionalPaymentMethodConnectorResponse::Card {
            authentication_data,
            payment_checks: (!payment_checks.is_empty())
                .then_some(serde_json::Value::Object(payment_checks)),
            card_network: None,
            domestic_network: None,
            auth_code,
        },
    ))
}

// Capture Request
#[derive(Debug, Serialize, Clone)]
pub struct CaptureRequest {
    amount: Option<StringMajorUnit>,
    receipt_email: Option<Secret<String>>,
    statement_descriptor: Option<String>,
    // The merchant's reference for this capture operation
    // (https://docs.rapyd.net/en/capture-payment.html). Omitted when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    merchant_reference_id: Option<String>,
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
            // Not sent on capture (HS parity); Rapyd has no L2/L3 fields on
            // this endpoint either.
            receipt_email: None,
            // Rapyd accepts `statement_descriptor` on capture
            // (https://docs.rapyd.net/en/capture-payment.html), but PaymentsCaptureData
            // carries no `billing_descriptor` — only PaymentsAuthorizeData,
            // SetupMandateRequestData and RepeatPaymentData do. Wiring this needs a
            // proto + domain-type change; on the auto-capture path the Authorize
            // request already carries the descriptor.
            statement_descriptor: None,
            // Same reference the Authorize request sent, so the capture is
            // correlated with the original payment.
            merchant_reference_id: Some(
                item.router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
        })
    }
}

// Refund Request (spec "Complete Endpoint Inventory > Refund";
// https://docs.rapyd.net/en/create-refund.html). A partial refund is a
// refund with an `amount` below the payment amount.
#[serde_with::skip_serializing_none]
#[derive(Default, Debug, Serialize)]
pub struct RapydRefundRequest {
    pub payment: String,
    pub amount: Option<StringMajorUnit>,
    pub currency: Option<common_enums::Currency>,
    /// The merchant's refund id, echoed back by Rapyd (req 20).
    pub merchant_reference_id: Option<String>,
    pub reason: Option<String>,
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
            payment: item
                .router_data
                .request
                .connector_transaction_id
                .to_string(),
            amount: Some(amount),
            currency: Some(item.router_data.request.currency),
            merchant_reference_id: Some(item.router_data.request.refund_id.clone()),
            reason: item.router_data.request.reason.clone(),
        })
    }
}

// Refund Response
/// Rapyd refund `data.status` (spec "Status Mappings > Refund status";
/// https://docs.rapyd.net/en/refund-object.html).
#[allow(dead_code)]
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub enum RefundStatus {
    Completed,
    Error,
    Rejected,
    Canceled,
    #[default]
    Pending,
    /// Any status Rapyd adds later; parses and stays non-terminal.
    #[serde(other)]
    Unknown,
}

/// The one Rapyd refund status map, shared by Refund, RSync and the refund
/// webhook.
impl From<RefundStatus> for common_enums::RefundStatus {
    fn from(item: RefundStatus) -> Self {
        match item {
            // spec: Status Mappings > Refund status — completed.
            RefundStatus::Completed => Self::Success,
            // spec: Status Mappings > Refund status — error / rejected / canceled.
            RefundStatus::Error | RefundStatus::Rejected | RefundStatus::Canceled => Self::Failure,
            // spec: Status Mappings > Refund status — pending; unknown stays
            // non-terminal so RSync resolves it.
            RefundStatus::Pending | RefundStatus::Unknown => Self::Pending,
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
    /// Refund failure code (source of the refund webhook error_code, UD-04).
    pub failure_code: Option<String>,
    pub merchant_reference_id: Option<String>,
}

/// Refund response. A 2xx envelope without `data` is a refused refund: it
/// fails the refund (HS rapyd/transformers.rs:383-405) and the Rapyd error
/// code stays an error code, never a connector_refund_id.
impl TryFrom<ResponseRouterData<RefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<RefundResponse, Self>) -> Result<Self, Self::Error> {
        let response = match item.response.data {
            Some(data) => Ok(RefundsResponseData {
                connector_refund_id: data.id,
                refund_status: common_enums::RefundStatus::from(data.status),
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            None => Err(ErrorResponse {
                code: non_empty(Some(item.response.status.error_code.as_str()))
                    .map(str::to_owned)
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                status_code: item.http_code,
                message: item
                    .response
                    .status
                    .status
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: item.response.status.message.clone(),
                attempt_status: Some(FlowStatus::Refund(common_enums::RefundStatus::Failure)),
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

/// Refund sync response. `data` present maps through the same `RefundStatus`
/// conversion as Refund and the refund webhook. A 2xx envelope without `data`
/// is an inconclusive lookup, not a verdict on the refund: it surfaces as an
/// error with `attempt_status: None` so a sync never terminally fails a refund
/// (HS rapyd/transformers.rs:407-427 diverges here; TH-07), and the Rapyd
/// error code is never used as a connector_refund_id.
impl TryFrom<ResponseRouterData<RefundResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<RefundResponse, Self>) -> Result<Self, Self::Error> {
        let response = match item.response.data {
            Some(data) => Ok(RefundsResponseData {
                connector_refund_id: data.id,
                refund_status: common_enums::RefundStatus::from(data.status),
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            None => Err(ErrorResponse {
                code: non_empty(Some(item.response.status.error_code.as_str()))
                    .map(str::to_owned)
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                status_code: item.http_code,
                message: item
                    .response
                    .status
                    .status
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: item.response.status.message.clone(),
                // Non-terminal: the refund's state is unknown, not failed.
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
                    "rapyd SetupMandate saves the card through POST /v1/payments, which needs an explicit amount",
                    "Send the verification amount in minor units (0 for a zero-amount card verification).",
                ),
            })?;
        // Decided on the minor unit, never on the converted string.
        let is_zero_amount = minor_amount.get_amount_as_i64() == 0;
        // Zero-amount verification goes as "0"; Rapyd rejects "0.00".
        let amount = if is_zero_amount {
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

        // A $0 verification is never captured (Rapyd: "Enter false as the
        // value since amount is 0"). A non-zero CIT honours the caller's
        // capture intent; Rapyd captures an authorization once, so multiple or
        // scheduled captures are refused (same rule as Authorize).
        let capture = if is_zero_amount {
            Some(false)
        } else {
            if let Some(
                capture_method @ (common_enums::CaptureMethod::ManualMultiple
                | common_enums::CaptureMethod::Scheduled),
            ) = request.capture_method
            {
                return Err(IntegrationError::NotSupported {
                    message: format!("capture_method {capture_method:?}"),
                    connector: "rapyd",
                    context: crate::utils::integration_ctx(
                        "Rapyd captures an authorized payment once; multiple or scheduled captures are not offered",
                        "Use capture_method AUTOMATIC or MANUAL for rapyd.",
                    ),
                })?;
            }
            Some(request.is_auto_capture())
        };

        let (payment_method, address) = match &request.payment_method_data {
            PaymentMethodData::Card(ccard) => {
                // Placeholder India type — see the Authorize flow: the sandbox
                // merchant enables `in_amex_card`, not the per-network types.
                let pm_type = RapydPaymentMethodType::InAmexCard;
                // Rapyd documents `payment_method.fields.name` as required
                // (https://docs.rapyd.net/en/create-card-payment-method.html).
                // Prefer the cardholder name on the card itself; fall back to
                // the billing full name. We deliberately do not fall back to
                // `customer_name`, which describes the Rapyd customer object
                // (inline `{name, email}`) — not the cardholder.
                let cardholder_name = ccard
                    .card_holder_name
                    .clone()
                    .or_else(|| {
                        router_data
                            .resource_common_data
                            .get_optional_billing_full_name()
                    })
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "card.card_holder_name / billing.full_name",
                        context: crate::utils::integration_ctx(
                            "Rapyd requires payment_method.fields.name to save a card",
                            "Send the card holder name, or a billing address with first and last name.",
                        ),
                    })?;
                let address = rapyd_billing_address(
                    &router_data.resource_common_data,
                    ccard.card_holder_name.as_ref(),
                );
                let payment_method =
                    RapydPaymentMethodData::PaymentMethod(Box::new(PaymentMethod {
                        pm_type,
                        fields: Some(RapydPaymentFields::Card(PaymentFields {
                            number: ccard.card_number.to_owned(),
                            expiration_month: ccard.card_exp_month.to_owned(),
                            expiration_year: ccard.card_exp_year.to_owned(),
                            name: cardholder_name,
                            cvv: ccard.card_cvc.to_owned(),
                        })),
                        address: None,
                        digital_wallet: None,
                    }));
                (payment_method, address)
            }
            _ => {
                return Err(IntegrationError::NotImplemented(
                    "payment_method for rapyd SetupMandate".to_owned(),
                    crate::utils::integration_ctx(
                        "rapyd SetupMandate saves cards only (save_payment_method on a card payment)",
                        "Set up the mandate with a card for rapyd.",
                    ),
                ))?;
            }
        };

        let three_ds_enabled = matches!(
            router_data.resource_common_data.auth_type,
            common_enums::AuthenticationType::ThreeDs
        );
        let payment_method_options = Some(PaymentMethodOptions {
            three_ds: three_ds_enabled,
            ..Default::default()
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

        // On SetupRecurring the proto return_url arrives on
        // `request.router_return_url`; `PaymentFlowData.return_url` is None for
        // this flow. Rapyd requires complete/error_payment_url for card types.
        let return_url = request
            .router_return_url
            .clone()
            .or_else(|| router_data.resource_common_data.return_url.clone())
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "return_url",
                context: crate::utils::integration_ctx(
                    "Rapyd requires complete_payment_url and error_payment_url on card payments, including the card-save payment",
                    "Send return_url on the setup-mandate request.",
                ),
            })?;
        let statement_descriptor = rapyd_statement_descriptor(request.billing_descriptor.as_ref());

        Ok(Self {
            amount,
            currency: request.currency,
            payment_method,
            capture,
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
            address,
            statement_descriptor,
            receipt_email: None,
            client_details: None,
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
        // $0 verification: Rapyd leaves the card-save payment ACT/pending_capture
        // (capture: false), which is the terminal outcome of a verification.
        // A non-zero CIT keeps the shared status map: a manual-capture CIT is
        // an uncaptured authorization, not a charge (UD-15). One rule with the
        // redirect-return PSync (B013): request amount 0 and original_amount 0.
        let is_zero_amount = item
            .router_data
            .request
            .minor_amount
            .is_some_and(|amount| is_zero_amount_save(amount, item.response.data.as_ref()));
        // Same envelope as every other /v1/payments call, so the shared mapper
        // decides status, redirect, mandate reference, NTID, decline fields
        // and connector_response (one status map for every payment flow).
        let mut router_data = Self::try_from(ResponseRouterData {
            response: RapydPaymentsResponse {
                status: item.response.status,
                data: item.response.data,
            },
            router_data: item.router_data,
            http_code: item.http_code,
        })?;
        if is_zero_amount
            && router_data.resource_common_data.status == common_enums::AttemptStatus::Authorized
            && router_data.response.is_ok()
        {
            router_data.resource_common_data.status = common_enums::AttemptStatus::Charged;
        }
        Ok(router_data)
    }
}

/// A completed $0 card save: the request amount is 0 and Rapyd reports
/// `data.original_amount` = 0. `original_amount`, not `amount`, is checked
/// because a 3DS-pending non-zero payment also reports `amount` 0. An absent
/// `original_amount` is not a $0 save (fail closed to the unpromoted status).
pub(crate) fn is_zero_amount_save(request_amount: MinorUnit, data: Option<&ResponseData>) -> bool {
    request_amount.get_amount_as_i64() == 0
        && data.is_some_and(|data| data.original_amount == Some(FloatMajorUnit::zero()))
}

/// PSync response – structurally identical to `RapydPaymentsResponse`
/// but defined as a distinct newtype so the $0-save promotion does not
/// collide with the blanket `RapydPaymentsResponse` conversion (E0119).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RapydPSyncResponse {
    pub status: Status,
    pub data: Option<ResponseData>,
}

impl TryFrom<ResponseRouterData<RapydPSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<RapydPSyncResponse, Self>) -> Result<Self, Self::Error> {
        // A $0 card save is finished by the redirect-return PSync, and Rapyd
        // leaves it ACT/pending_capture with paid false and original_amount 0
        // (live observation, B013) instead of the documented CLO. Nothing is
        // capturable, so that is the terminal outcome of a verification.
        let is_zero_amount =
            is_zero_amount_save(item.router_data.request.amount, item.response.data.as_ref());
        // Same envelope as every other /v1/payments call: the shared mapper
        // decides status, redirect, mandate reference, NTID, decline fields
        // and connector_response.
        let mut router_data = Self::try_from(ResponseRouterData {
            response: RapydPaymentsResponse {
                status: item.response.status,
                data: item.response.data,
            },
            router_data: item.router_data,
            http_code: item.http_code,
        })?;
        if is_zero_amount
            && router_data.resource_common_data.status == common_enums::AttemptStatus::Authorized
            && router_data.response.is_ok()
        {
            router_data.resource_common_data.status = common_enums::AttemptStatus::Charged;
        }
        Ok(router_data)
    }
}

// ---------------------------------------------------------------------------
// RepeatPayment (MIT) — Rapyd reuses /v1/payments with either the stored
// `payment_method` token (the card_* returned by SetupMandate) or card details
// plus the original network reference id. The request body is structurally identical to `RapydPaymentsRequest`, but we
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

        // Refusals that apply to every mandate arm come first, before any
        // request field is built.
        let initiation_type =
            RapydInitiationType::try_from_mit_category(request.mit_category.as_ref())?;
        // Rapyd captures an authorization once; multiple or scheduled captures
        // are refused (same rule as Authorize and SetupMandate, TH-15).
        if let Some(
            capture_method @ (common_enums::CaptureMethod::ManualMultiple
            | common_enums::CaptureMethod::Scheduled),
        ) = request.capture_method
        {
            return Err(IntegrationError::NotSupported {
                message: format!("capture_method {capture_method:?}"),
                connector: "rapyd",
                context: crate::utils::integration_ctx(
                    "Rapyd captures an authorized payment once; multiple or scheduled captures are not offered",
                    "Use capture_method AUTOMATIC or MANUAL for rapyd.",
                ),
            })?;
        }
        // On Charge, return_url arrives on `request.router_return_url`;
        // `PaymentFlowData.return_url` is hardcoded to None for this flow.
        let return_url = request
            .router_return_url
            .clone()
            .or_else(|| router_data.resource_common_data.return_url.clone())
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "return_url",
                context: crate::utils::integration_ctx(
                    "Rapyd requires complete_payment_url and error_payment_url on card payments, including merchant-initiated ones",
                    "Send return_url on the recurring charge request.",
                ),
            })?;

        let (payment_method, payment_method_options) = match &request.mandate_reference {
            // Variant A: the saved card token (`card_*`) from SetupMandate is
            // the mandate reference. Rapyd charges it directly — no customer
            // id needed.
            MandateReferenceId::ConnectorMandateId(connector_mandate) => {
                let card_id = connector_mandate.get_connector_mandate_id().ok_or(
                    IntegrationError::MissingRequiredField {
                        field_name: "mandate_reference.connector_mandate_id",
                        context: crate::utils::integration_ctx(
                            "Rapyd charges the saved card token (card_*) returned by SetupMandate",
                            "Send the connector_mandate_id returned by the setup-recurring call.",
                        ),
                    },
                )?;
                let three_ds_enabled = matches!(
                    router_data.resource_common_data.auth_type,
                    common_enums::AuthenticationType::ThreeDs
                );
                (
                    RapydPaymentMethodData::Token(Secret::new(card_id)),
                    PaymentMethodOptions {
                        three_ds: three_ds_enabled,
                        ..Default::default()
                    },
                )
            }
            // Variant B: card details plus the network reference id of the
            // original customer-initiated transaction; no cvv, no 3DS.
            // doc: https://docs.rapyd.net/en/creating-a-card-payment-with-a-network-reference-id.html
            MandateReferenceId::NetworkMandateId(network_mandate) => {
                let card = match &request.payment_method_data {
                    PaymentMethodData::CardDetailsForNetworkTransactionId(card) => card,
                    _ => Err(IntegrationError::MissingRequiredField {
                        field_name: "payment_method.card_details_for_network_transaction_id",
                        context: crate::utils::integration_ctx(
                            "A rapyd network-reference-id MIT sends the card number and expiry alongside network_reference_id",
                            "Send card_details_for_network_transaction_id with the network transaction id.",
                        ),
                    })?,
                };
                // Rapyd lists `name` among the Variant B card fields; there is
                // no hyperswitch precedent for an empty name here, so refuse
                // (UD-12).
                let name = card
                    .card_holder_name
                    .clone()
                    .or_else(|| {
                        router_data
                            .resource_common_data
                            .get_optional_billing_full_name()
                    })
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "card_holder_name",
                        context: crate::utils::integration_ctx(
                            "Rapyd requires payment_method.fields.name on a network-reference-id MIT",
                            "Send the card holder name, or a billing address with first and last name.",
                        ),
                    })?;
                (
                    RapydPaymentMethodData::PaymentMethod(Box::new(PaymentMethod {
                        // Same placeholder type as the raw-card Authorize path (UD-13).
                        pm_type: RapydPaymentMethodType::InAmexCard,
                        fields: Some(RapydPaymentFields::NetworkTransaction(
                            RapydNetworkTransactionFields {
                                number: card.card_number.clone(),
                                expiration_month: card.card_exp_month.clone(),
                                expiration_year: card.card_exp_year.clone(),
                                name,
                                network_reference_id: Secret::new(
                                    network_mandate.network_transaction_id.clone(),
                                ),
                            },
                        )),
                        address: None,
                        digital_wallet: None,
                    })),
                    PaymentMethodOptions {
                        three_ds: false,
                        ..Default::default()
                    },
                )
            }
            // A network-token MIT needs the token cryptogram (`tavv`), which
            // this connector does not send (spec 3DS §6.7).
            MandateReferenceId::NetworkTokenWithNTI(_) => Err(IntegrationError::NotSupported {
                message: "network token with network transaction id".to_owned(),
                connector: "rapyd",
                context: crate::utils::integration_ctx(
                    "rapyd merchant-initiated payments accept a saved card token or card details with a network reference id; network-token MITs are not supported",
                    "Charge the connector_mandate_id, or send card details with the network transaction id.",
                ),
            })?,
        };

        Ok(Self {
            amount,
            currency: request.currency,
            payment_method,
            // Honor the caller's capture intent; SequentialAutomatic and
            // unspecified default to auto-capture, matching the Authorize flow.
            capture: Some(request.is_auto_capture()),
            payment_method_options: Some(payment_method_options),
            merchant_reference_id: Some(
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            description: None,
            error_payment_url: Some(return_url.clone()),
            complete_payment_url: Some(return_url),
            customer: None,
            save_payment_method: None,
            initiation_type: Some(initiation_type),
            address: None,
            statement_descriptor: rapyd_statement_descriptor(request.billing_descriptor.as_ref()),
            receipt_email: None,
            client_details: None,
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
        // Same envelope as every other /v1/payments call, so the shared mapper
        // decides status, redirect (fail-loud), the decline code/message and
        // GSM triple (incl. merchant advice code) and connector_response.
        let mut router_data = Self::try_from(ResponseRouterData {
            response: RapydPaymentsResponse {
                status: item.response.status,
                data: item.response.data,
            },
            router_data: item.router_data,
            http_code: item.http_code,
        })?;
        if let Ok(PaymentsResponseData::TransactionResponse {
            mandate_reference,
            network_txn_id,
            ..
        }) = &mut router_data.response
        {
            // A MIT does not mint a new mandate: the `card_*` from
            // SetupMandate stays valid, and a returned payment_method must not
            // replace it. Rapyd returns a different network reference id on
            // each subsequent payment "which you do not use", so it must not
            // overwrite the original CIT NTID (UD-18).
            // doc: https://docs.rapyd.net/en/creating-a-card-payment-with-a-network-reference-id.html
            *mandate_reference = None;
            *network_txn_id = None;
        }
        Ok(router_data)
    }
}

// ---- IncomingWebhook types ----

/// Inbound webhook envelope. Payment and dispute events use Envelope A
/// (`id` = `wh_*`, `status` = `NEW`); refund events use Envelope B (`id` =
/// bare UUID, `status` = `""`, `created_at` = `0`). Envelope-B-only keys are
/// not modelled (spec: Complete Webhook Inventory > Envelope A / Envelope B).
/// `data` stays raw JSON and is decoded by event class in
/// [`parse_webhook_data`], never by trying variants in order (UD-17).
#[derive(Debug, Deserialize)]
pub struct RapydIncomingWebhook {
    pub id: String,
    #[serde(rename = "type")]
    pub webhook_type: RapydWebhookObjectEventType,
    pub data: serde_json::Value,
    pub trigger_operation_id: Option<String>,
    pub status: String,
    pub created_at: i64,
}

/// Webhook `type` (spec: Complete Webhook Inventory > Event type inventory).
/// The eight handled types are the set Hyperswitch maps; every other Rapyd
/// event (`PAYMENT_SUCCEEDED`, `PAYMENT_REVERSED`, `REFUND_UPDATED`, …)
/// parses as `Unknown` (UD-05).
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RapydWebhookObjectEventType {
    PaymentCompleted,
    PaymentCaptured,
    PaymentFailed,
    RefundCompleted,
    PaymentRefundRejected,
    PaymentRefundFailed,
    PaymentDisputeCreated,
    PaymentDisputeUpdated,
    #[serde(other)]
    Unknown,
}

/// Dispute `data.status` (spec: Status Mappings > Dispute status). `PRA`,
/// `ARB` and `REV` have no faithful `DisputeStatus` counterpart and parse as
/// `Unknown`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, strum::Display)]
pub enum RapydWebhookDisputeStatus {
    #[serde(rename = "ACT")]
    Active,
    #[serde(rename = "RVW")]
    Review,
    #[serde(rename = "LOS")]
    Lose,
    #[serde(rename = "WIN")]
    Win,
    #[serde(other)]
    Unknown,
}

/// Dispute webhook `data` (spec: Rust Type Design > New types to add).
/// `amount` is in MAJOR units of `currency`, like every other Rapyd amount
/// (UD-03; Hyperswitch declares MinorUnit here, HP-45).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DisputeResponseData {
    pub id: String,
    pub amount: FloatMajorUnit,
    pub currency: common_enums::Currency,
    /// The dispute identifier (`dispute_*`) — reported as `dispute_id`.
    pub token: String,
    pub dispute_reason_description: String,
    #[serde(default, with = "common_utils::custom_serde::timestamp::option")]
    pub due_date: Option<time::PrimitiveDateTime>,
    pub status: RapydWebhookDisputeStatus,
    #[serde(default, with = "common_utils::custom_serde::timestamp::option")]
    pub created_at: Option<time::PrimitiveDateTime>,
    #[serde(default, with = "common_utils::custom_serde::timestamp::option")]
    pub updated_at: Option<time::PrimitiveDateTime>,
    /// The parent payment (`payment_*`) — the payment lookup key.
    pub original_transaction_id: String,
    #[serde(default)]
    pub pre_dispute: Option<bool>,
}

/// `data` decoded by the class of the webhook `type` (UD-17).
#[derive(Debug, Clone)]
pub enum RapydWebhookData {
    /// Boxed: the payment object is several times larger than the others.
    Payment(Box<ResponseData>),
    Refund(RefundResponseData),
    Dispute(DisputeResponseData),
}

fn decode_webhook_data<D: serde::de::DeserializeOwned>(
    data: &serde_json::Value,
) -> Result<D, error_stack::Report<WebhookError>> {
    D::deserialize(data).change_context(WebhookError::WebhookBodyDecodingFailed)
}

/// Selects the `data` shape from the webhook `type`: payment events decode
/// [`ResponseData`], refund events [`RefundResponseData`], dispute events
/// [`DisputeResponseData`]. An unhandled `type` yields `None` (UD-17).
pub fn parse_webhook_data(
    webhook: &RapydIncomingWebhook,
) -> Result<Option<RapydWebhookData>, error_stack::Report<WebhookError>> {
    match webhook.webhook_type {
        RapydWebhookObjectEventType::PaymentCompleted
        | RapydWebhookObjectEventType::PaymentCaptured
        | RapydWebhookObjectEventType::PaymentFailed => decode_webhook_data(&webhook.data)
            .map(|data| Some(RapydWebhookData::Payment(Box::new(data)))),
        RapydWebhookObjectEventType::RefundCompleted
        | RapydWebhookObjectEventType::PaymentRefundRejected
        | RapydWebhookObjectEventType::PaymentRefundFailed => {
            decode_webhook_data(&webhook.data).map(|data| Some(RapydWebhookData::Refund(data)))
        }
        RapydWebhookObjectEventType::PaymentDisputeCreated
        | RapydWebhookObjectEventType::PaymentDisputeUpdated => {
            decode_webhook_data(&webhook.data).map(|data| Some(RapydWebhookData::Dispute(data)))
        }
        RapydWebhookObjectEventType::Unknown => Ok(None),
    }
}

/// `PAYMENT_DISPUTE_UPDATED` fires for every transition, so its event comes
/// from `data.status` (spec: Status Mappings > Dispute status, column 4).
impl From<RapydWebhookDisputeStatus> for EventType {
    fn from(status: RapydWebhookDisputeStatus) -> Self {
        match status {
            // spec: Dispute status — ACT, initiated and awaiting merchant action.
            RapydWebhookDisputeStatus::Active => Self::DisputeOpened,
            // spec: Dispute status — RVW, Rapyd reviewing merchant evidence.
            RapydWebhookDisputeStatus::Review => Self::DisputeChallenged,
            // spec: Dispute status — LOS, merchant lost.
            RapydWebhookDisputeStatus::Lose => Self::DisputeLost,
            // spec: Dispute status — WIN, merchant won.
            RapydWebhookDisputeStatus::Win => Self::DisputeWon,
            // spec: Dispute status — PRA / ARB / REV are not modelled.
            RapydWebhookDisputeStatus::Unknown => Self::IncomingWebhookEventUnspecified,
        }
    }
}

/// Dispute status for `process_dispute_webhook`. `DisputeStatus` has no
/// unspecified variant, so an unmodelled Rapyd code is an error, never a
/// guess (spec: Status Mappings > Dispute status).
pub fn dispute_status_to_common(
    status: &RapydWebhookDisputeStatus,
) -> Result<common_enums::DisputeStatus, error_stack::Report<WebhookError>> {
    match status {
        // spec: Dispute status — ACT.
        RapydWebhookDisputeStatus::Active => Ok(common_enums::DisputeStatus::DisputeOpened),
        // spec: Dispute status — RVW.
        RapydWebhookDisputeStatus::Review => Ok(common_enums::DisputeStatus::DisputeChallenged),
        // spec: Dispute status — LOS.
        RapydWebhookDisputeStatus::Lose => Ok(common_enums::DisputeStatus::DisputeLost),
        // spec: Dispute status — WIN.
        RapydWebhookDisputeStatus::Win => Ok(common_enums::DisputeStatus::DisputeWon),
        // spec: Dispute status — PRA / ARB / REV.
        RapydWebhookDisputeStatus::Unknown => {
            Err(error_stack::report!(WebhookError::WebhookProcessingFailed)
                .attach_printable("rapyd dispute webhook: unmodelled dispute status (PRA/ARB/REV)"))
        }
    }
}

/// Webhook `type` → UCS event (spec: Implementation Contract > Phase 1 —
/// get_event_type). Stateless: runs in both ParseEvent and HandleEvent.
pub fn get_webhook_event_type(
    webhook: &RapydIncomingWebhook,
) -> Result<EventType, error_stack::Report<WebhookError>> {
    match webhook.webhook_type {
        // spec: Event type inventory — payment fully collected / captured.
        RapydWebhookObjectEventType::PaymentCompleted
        | RapydWebhookObjectEventType::PaymentCaptured => Ok(EventType::PaymentIntentSuccess),
        // spec: Event type inventory — collection attempt failed.
        RapydWebhookObjectEventType::PaymentFailed => Ok(EventType::PaymentIntentFailure),
        // spec: Event type inventory — refund completed.
        RapydWebhookObjectEventType::RefundCompleted => Ok(EventType::RefundSuccess),
        // spec: Event type inventory — refund failed / rejected.
        RapydWebhookObjectEventType::PaymentRefundFailed
        | RapydWebhookObjectEventType::PaymentRefundRejected => Ok(EventType::RefundFailure),
        // spec: Event type inventory — dispute created.
        RapydWebhookObjectEventType::PaymentDisputeCreated => Ok(EventType::DisputeOpened),
        // spec: Event type inventory — dispute updated, event from data.status.
        RapydWebhookObjectEventType::PaymentDisputeUpdated => {
            let data: DisputeResponseData = decode_webhook_data(&webhook.data)?;
            Ok(EventType::from(data.status))
        }
        // spec: Event type inventory / UD-05 — every other Rapyd event.
        RapydWebhookObjectEventType::Unknown => Ok(EventType::IncomingWebhookEventUnspecified),
    }
}

/// Lifts a webhook payment object into the flow response envelope so the
/// resource object has the shape the PSync handler reads.
impl From<ResponseData> for RapydPaymentsResponse {
    fn from(data: ResponseData) -> Self {
        Self {
            status: Status {
                error_code: NO_ERROR_CODE.to_owned(),
                status: None,
                message: None,
                response_code: None,
                operation_id: None,
            },
            data: Some(data),
        }
    }
}

/// `Some(s)` when the webhook string is present and not blank: Rapyd sends
/// `""` rather than `null` for an unset value.
pub fn non_empty_owned(value: Option<&str>) -> Option<String> {
    non_empty(value).map(str::to_owned)
}
