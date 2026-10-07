use common_utils::{
    ext_traits::OptionExt,
    pii::{Email, IpAddress, SecretSerdeValue},
    request::Method,
    types::{AmountConvertor, FloatMajorUnitForConnector, MinorUnit, StringMinorUnitForConnector},
    FloatMajorUnit, StringMajorUnit,
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, CreateOrder, PSync, RSync, Refund,
        RepeatPayment, SetupMandate, Void,
    },
    connector_types::{
        ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData,
        ConnectorSpecificClientAuthenticationResponse, ConnectorWebhookSecrets,
        DisputeWebhookDetailsResponse, DisputeWebhookReference, EventType, MandateReference,
        MandateReferenceId, PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData,
        PaymentWebhookReference, PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData,
        RapydClientAuthenticationResponse as RapydClientAuthenticationResponseDomain,
        RefundFlowData, RefundSyncData, RefundWebhookDetailsResponse, RefundWebhookReference,
        RefundsData, RefundsResponseData, RepeatPaymentData, ResponseId, SetupMandateRequestData,
        WebhookDetailsResponse, WebhookResourceReference,
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
    router_request_types::BrowserInformation,
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
/// Methods endpoint; this connector covers the India (`in_`) set and three
/// United Kingdom (`gb_`) card types.
///
/// Raw card payments use `InAmexCard` by default: the sandbox merchant accepts
/// `in_amex_card` whatever the PAN. Rapyd validates the members of
/// `payment_method` and `payment_method_options` against the type and rejects
/// one the type does not list (`UNKNOWN_PAYMENT_METHOD_FIELD`), so a raw-card
/// request that carries a member `in_amex_card` does not list is sent on the
/// `gb_` type of its card network instead (see `try_for_raw_card` and
/// `capabilities`). Digital wallets carry their network + funding, so they
/// derive the funding-specific type (`in_credit_visa_card` /
/// `in_debit_visa_card`, etc.) via `try_from_wallet_network`.
///
/// References: https://docs.rapyd.net/en/list-payment-methods-by-country.html
/// and https://docs.rapyd.net/en/get-payment-method-required-fields.html
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RapydPaymentMethodType {
    InAmexCard,
    InCreditVisaCard,
    InDebitVisaCard,
    InCreditMastercardCard,
    InDebitMastercardCard,
    GbVisaCard,
    GbMastercardCard,
    GbAmexCard,
}

/// Optional request members a Rapyd payment-method type accepts, as its
/// required-fields listing reports them. A member the type does not accept
/// must be left out of the request.
/// Reference: https://docs.rapyd.net/en/get-payment-method-required-fields.html
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RapydPaymentMethodTypeCapabilities {
    /// `payment_method_options.avs_required`
    pub avs_required: bool,
    /// External 3DS `payment_method_options`: `3d_version`, `cavv`, `eci`,
    /// `xid`, `ds_trans_id`.
    pub external_three_ds: bool,
    /// `payment_method.fields.network_reference_id`
    pub network_reference_id: bool,
    /// `payment_method.fields.recurrence_type`
    pub recurrence_type: bool,
}

impl RapydPaymentMethodType {
    /// What this payment-method type accepts. Every request builder asks here
    /// before it serializes one of these members.
    pub const fn capabilities(self) -> RapydPaymentMethodTypeCapabilities {
        match self {
            Self::GbVisaCard | Self::GbMastercardCard => RapydPaymentMethodTypeCapabilities {
                avs_required: true,
                external_three_ds: true,
                network_reference_id: true,
                recurrence_type: true,
            },
            Self::GbAmexCard => RapydPaymentMethodTypeCapabilities {
                avs_required: false,
                external_three_ds: true,
                network_reference_id: true,
                recurrence_type: true,
            },
            Self::InAmexCard
            | Self::InCreditVisaCard
            | Self::InDebitVisaCard
            | Self::InCreditMastercardCard
            | Self::InDebitMastercardCard => RapydPaymentMethodTypeCapabilities {
                avs_required: false,
                external_three_ds: false,
                network_reference_id: false,
                recurrence_type: false,
            },
        }
    }

    /// Resolve the Rapyd `payment_method.type` of a raw card.
    ///
    /// `InAmexCard` unless the request carries data that type does not list
    /// (`needs_extended_members`: external 3DS authentication data or a
    /// network reference id). Such a request is sent on the `gb_` type of the
    /// card's network, taken from `card_network` when present and otherwise
    /// derived from the card number. A network without such a type, or one
    /// that cannot be determined, is refused: dropping the data would send a
    /// different payment from the one asked for.
    pub fn try_for_raw_card(
        card_network: Option<&common_enums::CardNetwork>,
        card_number: &str,
        needs_extended_members: bool,
    ) -> Result<Self, error_stack::Report<IntegrationError>> {
        if !needs_extended_members {
            return Ok(Self::InAmexCard);
        }
        let from_network = card_network.and_then(|network| match network {
            common_enums::CardNetwork::Visa => Some(Self::GbVisaCard),
            common_enums::CardNetwork::Mastercard => Some(Self::GbMastercardCard),
            common_enums::CardNetwork::AmericanExpress => Some(Self::GbAmexCard),
            _ => None,
        });
        let resolved = match (from_network, card_network) {
            (Some(pm_type), _) => Some(pm_type),
            // A network was supplied and has no type here: do not second-guess it.
            (None, Some(_)) => None,
            (None, None) => match domain_types::utils::get_card_issuer(card_number) {
                Ok(domain_types::utils::CardIssuer::Visa) => Some(Self::GbVisaCard),
                Ok(domain_types::utils::CardIssuer::Master) => Some(Self::GbMastercardCard),
                Ok(domain_types::utils::CardIssuer::AmericanExpress) => Some(Self::GbAmexCard),
                Ok(_) | Err(_) => None,
            },
        };
        resolved.ok_or_else(|| {
            IntegrationError::NotSupported {
                message: "external 3DS authentication data or a network transaction id for this card network".to_string(),
                connector: "rapyd",
                context: crate::utils::integration_ctx(
                    "Rapyd accepts external 3DS data and a network reference id only on the gb_visa_card, gb_mastercard_card and gb_amex_card payment-method types; the card network is not Visa, Mastercard or American Express, or could not be determined",
                    "Send a Visa, Mastercard or American Express card, or set card_network on the card",
                ),
            }
            .into()
        })
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
                context: Default::default(),
            })?,
        }
    }
}

/// Rapyd `initiation_type` of a merchant-initiated payment on
/// `/v1/payments`. A customer-initiated payment sends none: Rapyd's default is
/// `customer_present`. The vocabulary also has `moto` and three
/// industry-specific values that need `original_payment`; none is used here.
/// Reference: https://docs.rapyd.net/en/card-payments-and-merchant-initiated-transactions.html
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RapydInitiationType {
    /// A subscription charged at regular intervals with no end date.
    Recurring,
    /// A subscription with a fixed number of installments.
    Installment,
    /// A charge the cardholder authorised earlier, made off schedule.
    Unscheduled,
}

impl From<&common_enums::MitCategory> for RapydInitiationType {
    fn from(category: &common_enums::MitCategory) -> Self {
        match category {
            common_enums::MitCategory::Recurring => Self::Recurring,
            common_enums::MitCategory::Installment => Self::Installment,
            // Rapyd has no value for a retried charge; like any other
            // off-schedule charge on a stored credential it is `unscheduled`.
            common_enums::MitCategory::Unscheduled | common_enums::MitCategory::Resubmission => {
                Self::Unscheduled
            }
        }
    }
}

/// Rapyd `payment_method.fields.recurrence_type`: what a card saved by this
/// payment will be used for. Rapyd's default is `unscheduled`. It is sent only
/// on a payment-method type that lists the member; the `initiation_type` of a
/// later merchant-initiated payment is derived from the same caller value.
/// Reference: https://docs.rapyd.net/en/saving-a-european-card-while-creating-a-payment.html
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RapydRecurrenceType {
    Recurring,
    Installment,
    Unscheduled,
}

impl From<&common_enums::MitCategory> for RapydRecurrenceType {
    fn from(category: &common_enums::MitCategory) -> Self {
        match category {
            common_enums::MitCategory::Recurring => Self::Recurring,
            common_enums::MitCategory::Installment => Self::Installment,
            common_enums::MitCategory::Unscheduled | common_enums::MitCategory::Resubmission => {
                Self::Unscheduled
            }
        }
    }
}

/// Builds the `ErrorResponse` for a 2xx Rapyd payment response that did not
/// succeed: a payment object in status `ERR`, or a body without `data`.
///
/// `message` is the failure text (`data.failure_message`, else the envelope
/// `status.message`), never the envelope `status.status`: that member is the
/// outcome of the HTTP request and reads `SUCCESS` on a 200 whose payment is
/// `ERR`.
///
/// On a card-network decline `failure_code` is the card network response
/// code alone (`51`) and `failure_message` its short text; a payment Rapyd
/// failed itself (a failed 3DS challenge) carries a Rapyd error code there
/// (`ERROR_CREATE_PAYMENT`), which is not reported as a network decline code.
/// `merchant_advice_code` is the network's retry advice:
/// <https://docs.rapyd.net/en/card-network-errors.html>,
/// <https://docs.rapyd.net/en/merchant-advice-codes.html>
///
/// `missing_payment_status` is what a body without the payment object means
/// for the calling flow (`RapydMissingPaymentStatus`): `None` reports no
/// status.
fn build_payment_failure_response(
    status: &Status,
    data: Option<&ResponseData>,
    http_code: u16,
    missing_payment_status: Option<common_enums::AttemptStatus>,
) -> ErrorResponse {
    let non_empty = |value: &Option<String>| value.clone().filter(|text| !text.is_empty());
    let failure_code = data.and_then(|payment| non_empty(&payment.failure_code));
    let failure_message = data.and_then(|payment| non_empty(&payment.failure_message));
    let code = match (&failure_code, status.error_code.is_empty()) {
        (Some(failure_code), _) => failure_code.clone(),
        (None, false) => status.error_code.clone(),
        (None, true) => common_utils::consts::NO_ERROR_CODE.to_string(),
    };
    let reason = failure_message
        .clone()
        .or_else(|| non_empty(&status.message));
    let network_decline_code = failure_code
        .clone()
        .filter(|failure_code| is_card_network_response_code(failure_code));
    let network_error_message = network_decline_code
        .as_ref()
        .and_then(|_| failure_message.clone());
    ErrorResponse {
        code,
        status_code: http_code,
        message: reason
            .clone()
            .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
        reason,
        // A payment object in status `ERR` is a failed payment on every
        // flow; a body without one is judged by the calling flow.
        attempt_status: match data {
            Some(_) => Some(FlowStatus::Payment(common_enums::AttemptStatus::Failure)),
            None => missing_payment_status.map(FlowStatus::Payment),
        },
        connector_transaction_id: data.map(|payment| payment.id.clone()),
        network_advice_code: data.and_then(|payment| non_empty(&payment.merchant_advice_code)),
        network_decline_code,
        network_error_message,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
}

/// Whether a payment object's `failure_code` is a card-network response code
/// ("the card network error code alone", for example `05`, `51`, `N7`)
/// rather than a Rapyd error code (`ERROR_...`).
/// <https://docs.rapyd.net/en/card-network-errors.html>
fn is_card_network_response_code(failure_code: &str) -> bool {
    (2..=3).contains(&failure_code.len())
        && failure_code
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
}

/// Builds the success payload shared by every payment flow from a Rapyd
/// payment object: the transaction response and, when the payment carries
/// card check results, the connector response.
///
/// `include_mandate_details` is `false` for a merchant-initiated replay, where
/// neither the saved-card token nor the network reference id may replace the
/// ones stored from the initial payment.
///
/// Check fields (`cvv_check`, `avs_check` / `avs_result`, `acs_check`),
/// `auth_code` and `authentication_result` are documented at
/// <https://docs.rapyd.net/en/create-payment.html>
fn build_payment_success_response(
    data: &ResponseData,
    http_code: u16,
    include_mandate_details: bool,
) -> Result<
    (PaymentsResponseData, Option<ConnectorResponseData>),
    error_stack::Report<ConnectorError>,
> {
    let redirection_data = data
        .redirect_url
        .as_ref()
        .filter(|redirect_str| !redirect_str.is_empty())
        .map(|url| {
            Url::parse(url).change_context(crate::utils::response_handling_fail_for_connector(
                http_code, "rapyd",
            ))
        })
        .transpose()?
        .map(|url| Box::new(RedirectForm::from((url, Method::Get))));

    // The saved-card token Rapyd returns on a CIT save is the mandate
    // reference for later MIT replays. Rapyd charges the saved card without a
    // customer id, so nothing else needs to round-trip.
    let mandate_reference = data
        .payment_method
        .as_ref()
        .filter(|_| include_mandate_details)
        .map(|card| {
            Box::new(MandateReference {
                connector_mandate_id: Some(card.clone().expose()),
                payment_method_id: None,
                connector_mandate_request_reference_id: None,
                mandate_metadata: None,
            })
        });
    let network_txn_id = data
        .payment_method_data
        .as_ref()
        .filter(|_| include_mandate_details)
        .and_then(|pmd| pmd.network_reference_id.clone())
        .map(|nti| nti.expose());

    let transaction_response = PaymentsResponseData::TransactionResponse {
        // `transaction_id` also exists, but `id` is what a refund refers to.
        resource_id: ResponseId::ConnectorTransactionId(data.id.to_owned()),
        redirection_data,
        mandate_reference,
        connector_metadata: None,
        network_txn_id,
        network_txn_link_id: None,
        connector_response_reference_id: data.merchant_reference_id.to_owned(),
        incremental_authorization_allowed: None,
        status_code: http_code,
        splits: None,
        payment_account_reference: None,
    };

    Ok((
        transaction_response,
        build_card_connector_response(data, http_code)?,
    ))
}

/// Card check results, authorisation code and 3DS outcome of a payment, or
/// `None` when the payment object reports none of them.
fn build_card_connector_response(
    data: &ResponseData,
    http_code: u16,
) -> Result<Option<ConnectorResponseData>, error_stack::Report<ConnectorError>> {
    let to_json = |value: &RapydCheckResult| {
        serde_json::to_value(value).change_context(
            crate::utils::response_handling_fail_for_connector(http_code, "rapyd"),
        )
    };
    let non_empty = |value: &Option<String>| value.clone().filter(|text| !text.is_empty());

    let mut payment_checks = serde_json::Map::new();
    if let Some(checks) = data.payment_method_data.as_ref() {
        if let Some(cvv_check) = checks.cvv_check.as_ref() {
            payment_checks.insert("cvv_check".to_string(), to_json(cvv_check)?);
        }
        // Rapyd documents the AVS result as `avs_check` and returns it as
        // `avs_result` in its AVS sample; it is surfaced under the key the
        // response used, `avs_check` first.
        match (non_empty(&checks.avs_check), non_empty(&checks.avs_result)) {
            (Some(avs_check), _) => {
                payment_checks.insert(
                    "avs_check".to_string(),
                    serde_json::Value::String(avs_check),
                );
            }
            (None, Some(avs_result)) => {
                payment_checks.insert(
                    "avs_result".to_string(),
                    serde_json::Value::String(avs_result),
                );
            }
            (None, None) => {}
        }
        if let Some(acs_check) = checks.acs_check.as_ref() {
            payment_checks.insert("acs_check".to_string(), to_json(acs_check)?);
        }
    }

    let mut authentication_result = serde_json::Map::new();
    if let Some(authentication) = data.authentication_result.as_ref() {
        for (key, value) in [
            ("result", &authentication.result),
            ("eci", &authentication.eci),
            ("version", &authentication.version),
        ] {
            if let Some(text) = non_empty(value) {
                authentication_result.insert(key.to_string(), serde_json::Value::String(text));
            }
        }
    }

    let payment_checks =
        (!payment_checks.is_empty()).then(|| serde_json::Value::Object(payment_checks));
    let authentication_data = (!authentication_result.is_empty())
        .then(|| serde_json::Value::Object(authentication_result));
    let auth_code = non_empty(&data.auth_code);

    Ok(
        (payment_checks.is_some() || authentication_data.is_some() || auth_code.is_some()).then(
            || {
                ConnectorResponseData::with_additional_payment_method_data(
                    AdditionalPaymentMethodConnectorResponse::Card {
                        authentication_data,
                        payment_checks,
                        card_network: None,
                        domestic_network: None,
                        auth_code,
                    },
                )
            },
        ),
    )
}

/// What a 2xx Rapyd body without the payment object means for the flow that
/// received it. Create Payment made no payment, so the attempt failed. A
/// lookup, capture or cancel of an existing payment that returns no payment
/// object says nothing about that payment: no status is reported and the
/// stored one stays.
pub trait RapydMissingPaymentStatus {
    const MISSING_PAYMENT_STATUS: Option<common_enums::AttemptStatus>;
}

impl RapydMissingPaymentStatus for Authorize {
    const MISSING_PAYMENT_STATUS: Option<common_enums::AttemptStatus> =
        Some(common_enums::AttemptStatus::Failure);
}

impl RapydMissingPaymentStatus for PSync {
    const MISSING_PAYMENT_STATUS: Option<common_enums::AttemptStatus> = None;
}

impl RapydMissingPaymentStatus for Capture {
    const MISSING_PAYMENT_STATUS: Option<common_enums::AttemptStatus> = None;
}

impl RapydMissingPaymentStatus for Void {
    const MISSING_PAYMENT_STATUS: Option<common_enums::AttemptStatus> = None;
}

impl<F: RapydMissingPaymentStatus, T> TryFrom<ResponseRouterData<RapydPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<RapydPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let (status, response, connector_response) = match &item.response.data {
            Some(data) => {
                let attempt_status = get_status(
                    data.status.to_owned(),
                    data.next_action.to_owned(),
                    data.is_zero_amount(),
                );
                // A payment object in status `ERR` arrives with HTTP 200: it
                // is an error response, never a transaction response.
                if attempt_status == common_enums::AttemptStatus::Failure {
                    (
                        attempt_status,
                        Err(build_payment_failure_response(
                            &item.response.status,
                            Some(data),
                            item.http_code,
                            F::MISSING_PAYMENT_STATUS,
                        )),
                        None,
                    )
                } else {
                    let (transaction_response, connector_response) =
                        build_payment_success_response(data, item.http_code, true)?;
                    (attempt_status, Ok(transaction_response), connector_response)
                }
            }
            None => (
                F::MISSING_PAYMENT_STATUS.unwrap_or(common_enums::AttemptStatus::Unspecified),
                Err(build_payment_failure_response(
                    &item.response.status,
                    None,
                    item.http_code,
                    F::MISSING_PAYMENT_STATUS,
                )),
                None,
            ),
        };

        // Keep what the router data already carries when this response has
        // no card checks to report.
        let connector_response = connector_response.or_else(|| {
            item.router_data
                .resource_common_data
                .connector_response
                .clone()
        });

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_response,
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
    /// Rapyd customer of a payment that saves the card: an inline object
    /// `{ name, email? }`, sent when the customer name is known. Without it
    /// Rapyd creates the customer itself (see `build_inline_customer`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer: Option<RapydCustomerRef>,
    /// When true and `payment_method` carries card fields, Rapyd saves
    /// the card to the customer and returns a reusable `card_*` id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save_payment_method: Option<bool>,
    /// How a merchant-initiated payment was initiated. Left out on a
    /// customer-initiated payment (Rapyd's default is `customer_present`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initiation_type: Option<RapydInitiationType>,
    /// Billing address of the payer, sent at the top level of the request.
    /// Rapyd requires `name` and `line_1` inside it, so it is only built
    /// when both are known (see `build_rapyd_address`). It is also the
    /// input of the AVS check requested by
    /// `payment_method_options.avs_required`.
    /// Reference: https://docs.rapyd.net/en/create-payment.html
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<Address>,
    /// Text for the customer's statement (Rapyd: 5-22 characters). When
    /// absent Rapyd applies the account default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statement_descriptor: Option<String>,
    /// Email the receipt is sent to; Rapyd asks for it on Visa 3DS payments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt_email: Option<Email>,
    /// Merchant-defined JSON object, echoed back by Rapyd.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<SecretSerdeValue>,
    /// Browser details of the payer, used by Rapyd for 3DS authentication.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_details: Option<RapydClientDetails>,
}

/// `client_details` of a `/v1/payments` request: the payer's browser as the
/// client collected it. Rapyd does not echo it back. Every member is
/// optional and omitted when unknown.
/// Reference: https://docs.rapyd.net/en/creating-a-card-payment-with-3ds-authentication---rapyd-3ds.html
#[derive(Debug, Default, Serialize)]
pub struct RapydClientDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_address: Option<Secret<String, IpAddress>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accept_header: Option<String>,
    /// IETF BCP 47 language tag of the browser.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub java_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub java_script_enabled: Option<bool>,
    /// Colour depth in bits.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screen_color_depth: Option<u8>,
    /// Screen height in pixels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screen_height: Option<u32>,
    /// Screen width in pixels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screen_width: Option<u32>,
    /// Difference between UTC and the payer's time zone, in minutes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_zone_offset: Option<i32>,
}

/// Rapyd customer reference: either a raw id string (`cus_*`), or an inline
/// `{name, email?}` object when Rapyd is to create the customer alongside the
/// payment. Only the inline form is sent today.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum RapydCustomerRef {
    Id(String),
    Inline(RapydInlineCustomer),
}

/// Inline customer object embedded in a `/v1/payments` call. Rapyd requires
/// `name`; `email` is optional.
/// Reference: https://docs.rapyd.net/en/create-customer.html
#[derive(Debug, Serialize)]
pub struct RapydInlineCustomer {
    pub name: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<Email>,
}

/// Builds the inline `customer` of a payment that saves the card. Shared by
/// the two customer-initiated entry points (Authorize with
/// `setup_future_usage = off_session`, SetupMandate).
///
/// Rapyd does not make `customer` mandatory for saving a card: its samples
/// send an inline object, a `cus_*` id, or nothing, and return a
/// `customer_token` each time. So the object is sent when a customer name is
/// known and left out otherwise; a missing name is never a refusal.
/// Reference: https://docs.rapyd.net/en/create-payment.html
fn build_inline_customer(
    customer_name: Option<Secret<String>>,
    customer_email: Option<Email>,
) -> Option<RapydCustomerRef> {
    customer_name
        .filter(|name| !name.peek().trim().is_empty())
        .map(|name| {
            RapydCustomerRef::Inline(RapydInlineCustomer {
                name,
                email: customer_email,
            })
        })
}

/// Builds `client_details` from the browser information the caller
/// collected. Only the members that are known are sent; `None` when there
/// are none.
/// Reference: https://docs.rapyd.net/en/creating-a-card-payment-with-3ds-authentication---rapyd-3ds.html
fn build_rapyd_client_details(
    browser_info: Option<&BrowserInformation>,
) -> Option<RapydClientDetails> {
    let browser = browser_info?;
    let has_any_member = browser.ip_address.is_some()
        || browser.accept_header.is_some()
        || browser.language.is_some()
        || browser.java_enabled.is_some()
        || browser.java_script_enabled.is_some()
        || browser.color_depth.is_some()
        || browser.screen_height.is_some()
        || browser.screen_width.is_some()
        || browser.time_zone.is_some();
    has_any_member.then(|| RapydClientDetails {
        ip_address: browser
            .ip_address
            .map(|ip_address| Secret::new(ip_address.to_string())),
        accept_header: browser.accept_header.clone(),
        language: browser.language.clone(),
        java_enabled: browser.java_enabled,
        java_script_enabled: browser.java_script_enabled,
        screen_color_depth: browser.color_depth,
        screen_height: browser.screen_height,
        screen_width: browser.screen_width,
        time_zone_offset: browser.time_zone,
    })
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

/// `payment_method_options` of a card payment. Every member is optional and
/// omitted when unset, so the object carries only what the flow decided.
/// Reference: https://docs.rapyd.net/en/create-payment.html
#[derive(Debug, Default, Serialize)]
pub struct PaymentMethodOptions {
    /// Asks Rapyd to run (or skip) its own redirect-based 3DS. Left out when
    /// the payment carries externally obtained authentication data.
    #[serde(rename = "3d_required", skip_serializing_if = "Option::is_none")]
    pub three_ds: Option<bool>,
    /// Asks Rapyd to run the address verification (AVS) check against the
    /// top-level `address` object. Sent only on a payment-method type that
    /// accepts it (`RapydPaymentMethodType::capabilities`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avs_required: Option<bool>,
    /// External 3DS pass-through: the 3DS protocol version used.
    #[serde(rename = "3d_version", skip_serializing_if = "Option::is_none")]
    pub three_ds_version: Option<String>,
    /// External 3DS pass-through: the cardholder authentication value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cavv: Option<Secret<String>>,
    /// External 3DS pass-through: the electronic commerce indicator.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eci: Option<String>,
    /// External 3DS pass-through: directory-server transaction id (3DS 2.x).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds_trans_id: Option<Secret<String>>,
    /// External 3DS pass-through: transaction id of a 3DS 1.x authentication.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xid: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
pub struct PaymentMethod<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> {
    #[serde(rename = "type")]
    pub pm_type: RapydPaymentMethodType,
    pub fields: Option<RapydPaymentMethodFields<T>>,
    pub address: Option<Address>,
    pub digital_wallet: Option<RapydWallet>,
}

/// `payment_method.fields` of a card: the card with its CVV, or, on a
/// merchant-initiated payment made with a network reference id, the card
/// with that id in place of the CVV.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum RapydPaymentMethodFields<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
> {
    Card(PaymentFields<T>),
    NetworkReference(RapydNetworkReferenceFields<T>),
}

#[derive(Default, Debug, Serialize)]
pub struct PaymentFields<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> {
    pub number: RawCardNumber<T>,
    pub expiration_month: Secret<String>,
    pub expiration_year: Secret<String>,
    pub name: Secret<String>,
    pub cvv: Secret<String>,
    /// Intended use of the card when this payment saves it. Sent only on a
    /// payment-method type that accepts it
    /// (`RapydPaymentMethodType::capabilities`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurrence_type: Option<RapydRecurrenceType>,
}

/// Card number of a network-reference-id payment. The caller sends either a
/// card payment method or the stored card details kept for network
/// transaction id payments; the two carry different card-number types.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum RapydCardNumber<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> {
    Card(RawCardNumber<T>),
    StoredCard(cards::CardNumber),
}

/// `payment_method.fields` of a payment made with a network reference id:
/// "you can use a network reference ID instead of the CVV inside
/// payment_method.fields". There is no `cvv` member.
/// Reference: https://docs.rapyd.net/en/creating-a-card-payment-with-a-network-reference-id.html
#[derive(Debug, Serialize)]
pub struct RapydNetworkReferenceFields<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
> {
    pub number: RapydCardNumber<T>,
    pub expiration_month: Secret<String>,
    pub expiration_year: Secret<String>,
    /// Cardholder name; always set by the request builder, which refuses a
    /// request that carries none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Secret<String>>,
    /// The network reference id of the initial payment. Every later payment
    /// uses that same id, never the one a later response returns.
    pub network_reference_id: Secret<String>,
}

/// Rapyd `address` object. `name` and `line_1` are required by Rapyd; the
/// rest is omitted when unknown.
/// Reference: https://docs.rapyd.net/en/create-payment.html
#[derive(Debug, Serialize)]
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
    /// ISO 3166-1 alpha-2.
    #[serde(skip_serializing_if = "Option::is_none")]
    country: Option<common_enums::CountryAlpha2>,
    #[serde(skip_serializing_if = "Option::is_none")]
    zip: Option<Secret<String>>,
    /// E.164, so only sent when the country code is known.
    #[serde(skip_serializing_if = "Option::is_none")]
    phone_number: Option<Secret<String>>,
}

/// Builds the top-level `address` object of a `/v1/payments` request from
/// the billing address. Shared by every flow that posts to `/v1/payments`
/// (Authorize, SetupMandate, RepeatPayment).
///
/// Rapyd rejects an `address` without `name` and `line_1`, so `None` is
/// returned unless the billing address carries both; nothing is invented
/// for a missing one. The shipping address is not read: Rapyd's Create
/// Payment has no field for it.
/// Reference: https://docs.rapyd.net/en/create-payment.html
pub fn build_rapyd_address(flow_data: &PaymentFlowData) -> Option<Address> {
    let is_present = |value: &Secret<String>| !value.peek().trim().is_empty();
    let name = flow_data
        .get_optional_billing_full_name()
        .filter(is_present)?;
    let line_1 = flow_data.get_optional_billing_line1().filter(is_present)?;
    // E.164 needs the country code; a bare national number is not sent.
    let phone_number = flow_data
        .get_optional_billing()
        .and_then(|billing| billing.phone.as_ref())
        .and_then(|phone| phone.get_number_with_country_code().ok());
    Some(Address {
        name,
        line_1,
        line_2: flow_data.get_optional_billing_line2(),
        line_3: flow_data.get_optional_billing_line3(),
        city: flow_data.get_optional_billing_city(),
        state: flow_data.get_optional_billing_state(),
        country: flow_data.get_optional_billing_country(),
        zip: flow_data.get_optional_billing_zip(),
        phone_number,
    })
}

/// `payment_method.fields.name`, which Rapyd requires on a card payment
/// method: a request without it is rejected with `INVALID_HOLDER_NAME`.
/// The cardholder name on the card is used, else the billing full name; a
/// blank value is not a name. The customer name is not used: it describes
/// the Rapyd customer, not the cardholder.
/// <https://docs.rapyd.net/en/create-card-payment-method.html>
fn rapyd_cardholder_name(
    card_holder_name: Option<&Secret<String>>,
    flow_data: &PaymentFlowData,
) -> Result<Secret<String>, error_stack::Report<IntegrationError>> {
    let is_present = |name: &Secret<String>| !name.peek().trim().is_empty();
    card_holder_name
        .cloned()
        .filter(is_present)
        .or_else(|| {
            flow_data
                .get_optional_billing_full_name()
                .filter(is_present)
        })
        .ok_or_else(|| {
            IntegrationError::MissingRequiredField {
                field_name: "card.card_holder_name / billing.full_name",
                context: crate::utils::integration_ctx(
                    "Rapyd requires payment_method.fields.name on a card payment method",
                    "Send the cardholder name on the card, or the first and last name in the billing address",
                ),
            }
            .into()
        })
}

/// Rapyd's `capture` member is a plain boolean, so only "capture now" and
/// "capture later, once" can be expressed. A capture method that needs more
/// than that is refused rather than silently sent as a manual capture.
fn rapyd_capture_flag(
    capture_method: Option<common_enums::CaptureMethod>,
) -> Result<bool, error_stack::Report<IntegrationError>> {
    match capture_method {
        Some(common_enums::CaptureMethod::Automatic)
        | Some(common_enums::CaptureMethod::SequentialAutomatic)
        | None => Ok(true),
        Some(common_enums::CaptureMethod::Manual) => Ok(false),
        Some(common_enums::CaptureMethod::ManualMultiple)
        | Some(common_enums::CaptureMethod::Scheduled) => {
            Err(IntegrationError::CaptureMethodNotSupported {
                context: crate::utils::integration_ctx(
                    "Rapyd payments accept only capture true or false; multiple partial captures and scheduled capture are not available",
                    "Use capture_method automatic, sequential_automatic or manual",
                ),
            }
            .into())
        }
    }
}

/// Whether the billing address carries the two inputs an AVS check compares,
/// `line1` and `zip`.
fn billing_has_avs_inputs(flow_data: &PaymentFlowData) -> bool {
    let is_present = |value: &Secret<String>| !value.peek().trim().is_empty();
    flow_data
        .get_optional_billing_line1()
        .is_some_and(|line1| is_present(&line1))
        && flow_data
            .get_optional_billing_zip()
            .is_some_and(|zip| is_present(&zip))
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
        // Refused here, before any field is built, when the capture method
        // cannot be expressed.
        let capture = Some(rapyd_capture_flag(item.router_data.request.capture_method)?);
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

        // Capture intent (resolved above) applies to every payment method; the
        // `three_ds` option is card-only (wallets carry their own
        // authentication).
        // No request field asks for an AVS check, so it is derived: Rapyd is
        // asked to run it when the billing address carries the two inputs AVS
        // compares, `line1` and `zip`, and the payment-method type accepts the
        // member (see the card arm below). Otherwise the member is omitted
        // and Rapyd applies the account default.
        let has_avs_inputs = billing_has_avs_inputs(&item.router_data.resource_common_data);
        // A customer-initiated payment stores the card only when the caller
        // asks for later off-session use. An on-session payment must not
        // create a card-on-file object at Rapyd.
        let saves_card = matches!(
            item.router_data.request.setup_future_usage,
            Some(common_enums::FutureUsage::OffSession)
        );
        let mut payment_method_options = matches!(
            item.router_data.resource_common_data.payment_method,
            common_enums::PaymentMethod::Card
        )
        .then(|| PaymentMethodOptions {
            three_ds: Some(matches!(
                item.router_data.resource_common_data.auth_type,
                common_enums::AuthenticationType::ThreeDs
            )),
            ..Default::default()
        });
        // Browser details of the payer, which Rapyd uses for 3DS. Sent on a
        // card payment with 3DS requested, including one that carries
        // external authentication data ("The card issuer might require 3DS
        // even when it is not specified in the payment request"); every other
        // payment leaves them out.
        // Reference: https://docs.rapyd.net/en/creating-a-card-payment-with-3ds-authentication---rapyd-3ds.html
        let mut client_details = None;
        let payment_method = match item.router_data.request.payment_method_data {
            PaymentMethodData::Card(ref ccard) => {
                // The result of an authentication the merchant ran with its
                // own 3DS provider: present when the request carries a CAVV.
                let external_authentication = item
                    .router_data
                    .request
                    .authentication_data
                    .as_ref()
                    .filter(|authentication_data| authentication_data.cavv.is_some());
                // `in_amex_card` unless the request carries externally obtained
                // 3DS authentication data, which that type does not list.
                let pm_type = RapydPaymentMethodType::try_for_raw_card(
                    ccard.card_network.as_ref(),
                    ccard.card_number.peek(),
                    external_authentication.is_some(),
                )?;
                if let Some(options) = payment_method_options.as_mut() {
                    options.avs_required =
                        (has_avs_inputs && pm_type.capabilities().avs_required).then_some(true);
                }
                if matches!(
                    item.router_data.resource_common_data.auth_type,
                    common_enums::AuthenticationType::ThreeDs
                ) {
                    client_details =
                        build_rapyd_client_details(item.router_data.request.browser_info.as_ref());
                }
                // External 3DS pass-through: the authentication values go in
                // `payment_method_options` and `3d_required` is left out, so
                // Rapyd is not asked for a redirect of its own.
                // Reference: https://docs.rapyd.net/en/creating-a-card-payment-with-3ds-authentication---external-3ds.html
                if let Some(authentication_data) = external_authentication {
                    // A CAVV without its ECI is half of the authentication
                    // result; sending it would claim an authentication the
                    // request cannot prove.
                    let eci = authentication_data.eci.clone().ok_or_else(|| {
                        IntegrationError::MissingRequiredField {
                            field_name: "authentication_data.eci",
                            context: crate::utils::integration_ctx(
                                "Rapyd external 3DS needs payment_method_options.eci together with payment_method_options.cavv",
                                "Send authentication_data.eci from the external 3DS provider along with authentication_data.cavv",
                            ),
                        }
                    })?;
                    // `xid` identifies a 3DS 1.x authentication; a 2.x one is
                    // identified by `ds_trans_id`.
                    let is_three_ds_v1 = authentication_data
                        .message_version
                        .as_ref()
                        .is_some_and(|message_version| message_version.get_major() == 1);
                    let options =
                        payment_method_options.get_or_insert_with(PaymentMethodOptions::default);
                    options.three_ds = None;
                    options.three_ds_version = authentication_data
                        .message_version
                        .as_ref()
                        .map(ToString::to_string);
                    options.cavv = authentication_data.cavv.clone();
                    options.eci = Some(eci);
                    options.ds_trans_id =
                        authentication_data.ds_trans_id.clone().map(Secret::new);
                    options.xid = authentication_data
                        .transaction_id
                        .clone()
                        .filter(|_| is_three_ds_v1)
                        .map(Secret::new);
                }
                // The intended use of a card this payment saves, when the
                // caller names one and the payment-method type accepts the
                // member; otherwise Rapyd applies its default, `unscheduled`.
                let recurrence_type = item
                    .router_data
                    .request
                    .mit_category
                    .as_ref()
                    .filter(|_| saves_card && pm_type.capabilities().recurrence_type)
                    .map(RapydRecurrenceType::from);
                Some(RapydPaymentMethodData::PaymentMethod(Box::new(
                    PaymentMethod {
                        pm_type,
                        fields: Some(RapydPaymentMethodFields::Card(PaymentFields {
                            number: ccard.card_number.to_owned(),
                            expiration_month: ccard.card_exp_month.to_owned(),
                            expiration_year: ccard.card_exp_year.to_owned(),
                            name: rapyd_cardholder_name(
                                ccard.card_holder_name.as_ref(),
                                &item.router_data.resource_common_data,
                            )?,
                            cvv: ccard.card_cvc.to_owned(),
                            recurrence_type,
                        })),
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
        // Ask Rapyd to save the card, so the response carries the reusable
        // `card_*` id (the mandate) for later merchant-initiated payments.
        let (customer, save_payment_method) = if saves_card {
            (
                build_inline_customer(
                    item.router_data.request.get_optional_customer_name(),
                    item.router_data.request.get_optional_email(),
                ),
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
            description: item.router_data.resource_common_data.description.clone(),
            error_payment_url: Some(return_url.clone()),
            complete_payment_url: Some(return_url),
            customer,
            save_payment_method,
            initiation_type: None,
            // Shipping address and level 2/3 data are not read: Rapyd's
            // Create Payment has no field for either.
            address: build_rapyd_address(&item.router_data.resource_common_data),
            statement_descriptor: item
                .router_data
                .request
                .billing_descriptor
                .as_ref()
                .and_then(|descriptor| descriptor.statement_descriptor.clone()),
            receipt_email: item.router_data.request.get_optional_email().or_else(|| {
                item.router_data
                    .resource_common_data
                    .get_optional_billing_email()
            }),
            metadata: item.router_data.request.metadata.clone(),
            client_details,
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
    /// A status this connector does not know; never terminal.
    #[serde(other)]
    Unknown,
}

/// Maps Rapyd's payment `status` + `next_action` pair to an attempt status.
///
/// `next_action` values are documented in the Create Payment response
/// parameters: <https://docs.rapyd.net/en/create-payment.html>
///
/// Zero-amount rule: a card verification authorises nothing, so it never
/// reaches `CLO` and stays `ACT` for good, with `next_action` `pending_capture`
/// (what the sandbox reports once the 3DS challenge is completed) or
/// `not_applicable`. There is nothing left to capture, so both pairs are
/// reported as `Charged` when the payment was created for a zero amount
/// (`is_zero_amount`), and as `Authorized` otherwise.
/// Every reader of a payment object (create, sync, webhook) passes the same
/// fact, so they agree on the outcome.
fn get_status(
    status: RapydPaymentStatus,
    next_action: Option<NextAction>,
    is_zero_amount: bool,
) -> common_enums::AttemptStatus {
    match (status, next_action) {
        (RapydPaymentStatus::Closed, _) => common_enums::AttemptStatus::Charged,
        (
            RapydPaymentStatus::Active,
            Some(NextAction::ThreedsVerification | NextAction::PendingConfirmation),
        ) => common_enums::AttemptStatus::AuthenticationPending,
        // A verified zero-amount payment stays `ACT` + `pending_capture`
        // (sandbox, after 3DS) or `not_applicable`: nothing is left to capture.
        (
            RapydPaymentStatus::Active,
            Some(NextAction::PendingCapture | NextAction::NotApplicable),
        ) => {
            if is_zero_amount {
                common_enums::AttemptStatus::Charged
            } else {
                common_enums::AttemptStatus::Authorized
            }
        }
        // Settles later through clearing: not decided yet.
        (RapydPaymentStatus::Active, Some(NextAction::PendingOfflineCapture)) => {
            common_enums::AttemptStatus::Pending
        }
        // A next action that is absent or not known here is never terminal.
        (RapydPaymentStatus::Active, Some(NextAction::Unknown) | None) => {
            common_enums::AttemptStatus::Pending
        }
        (
            RapydPaymentStatus::CanceledByClientOrBank
            | RapydPaymentStatus::Expired
            | RapydPaymentStatus::ReversedByRapyd,
            _,
        ) => common_enums::AttemptStatus::Voided,
        (RapydPaymentStatus::Error, _) => common_enums::AttemptStatus::Failure,
        (RapydPaymentStatus::New, _) => common_enums::AttemptStatus::Authorizing,
        // A status this connector does not know says nothing about the
        // payment: no status is reported, so a stored `Charged` or
        // `Authorized` is not downgraded by a later sync or webhook.
        (RapydPaymentStatus::Unknown, _) => common_enums::AttemptStatus::Unspecified,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RapydPaymentsResponse {
    pub status: Status,
    pub data: Option<ResponseData>,
}

/// Envelope of a non-2xx Rapyd body. Only `status` is read: the error mapping
/// needs nothing else, and the `data` object of a rejected call carries null
/// members (`id: null` on a rejected Create Payment), so it is not
/// deserialized at all.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RapydErrorEnvelope {
    pub status: RapydErrorStatus,
}

/// `status` object of a non-2xx Rapyd body. Every member tolerates null and
/// absence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RapydErrorStatus {
    pub error_code: Option<String>,
    pub status: Option<String>,
    pub message: Option<String>,
    pub response_code: Option<String>,
    pub operation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Status {
    pub error_code: String,
    pub status: Option<String>,
    pub message: Option<String>,
    pub response_code: Option<String>,
    pub operation_id: Option<String>,
}

/// `status.error_code` prefix of a card-network decline. Rapyd documents the
/// form `ERROR_PROCESSING_CARD - [NN]`, where `NN` is the code received from
/// the card network: <https://docs.rapyd.net/en/card-network-errors.html>
const CARD_NETWORK_ERROR_PREFIX: &str = "ERROR_PROCESSING_CARD - [";
const CARD_NETWORK_ERROR_SUFFIX: &str = "]";

/// Create Payment `status.error_code` values after which the outcome at the
/// processor is not known, so an HTTP 400 carrying one of them is not a
/// terminal failure.
const NON_TERMINAL_CREATE_PAYMENT_ERROR_CODES: [&str; 3] = [
    "ERROR_CREATE_PAYMENT_GATEWAY_NOT_RESPONDING",
    "ERROR_PROCESS_PAYMENT_SETTLEMENT_PENDING_ON_GATEWAY",
    "ERROR_PROCESS_PAYMENT_PROCESSOR_UNAVAILABLE",
];

/// The class of call a non-2xx Rapyd response answers. It decides whether the
/// error is terminal for the attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RapydErrorFlow {
    /// `POST /v1/payments`
    PaymentCreate,
    /// `GET /v1/payments/{id}`
    PaymentSync,
    /// A call on an existing payment (capture): a rejection leaves the payment untouched.
    PaymentModify,
    /// `POST /v1/refunds`
    RefundCreate,
    /// `GET /v1/refunds/{id}`
    RefundSync,
}

impl RapydErrorFlow {
    /// Attempt status carried by a non-2xx response; `None` leaves the attempt
    /// non-terminal.
    pub fn error_attempt_status(
        self,
        http_status_code: u16,
        error_code: &str,
    ) -> Option<FlowStatus> {
        match self {
            Self::PaymentCreate => (http_status_code == 400
                && !NON_TERMINAL_CREATE_PAYMENT_ERROR_CODES.contains(&error_code))
            .then_some(FlowStatus::Payment(common_enums::AttemptStatus::Failure)),
            // "The payment must be in closed status": Rapyd rejects a refund it
            // will not make with HTTP 400, and no refund object is created.
            // <https://docs.rapyd.net/en/create-refund.html>
            Self::RefundCreate => (http_status_code == 400)
                .then_some(FlowStatus::Refund(common_enums::RefundStatus::Failure)),
            Self::PaymentSync | Self::PaymentModify | Self::RefundSync => None,
        }
    }
}

/// Card-network code `NN` of an `ERROR_PROCESSING_CARD - [NN]` error code;
/// `None` for every other code.
/// <https://docs.rapyd.net/en/card-network-errors.html>
pub fn card_network_error_code(error_code: &str) -> Option<&str> {
    error_code
        .strip_prefix(CARD_NETWORK_ERROR_PREFIX)
        .and_then(|rest| rest.strip_suffix(CARD_NETWORK_ERROR_SUFFIX))
        .map(str::trim)
        .filter(|network_code| !network_code.is_empty())
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
    /// Pending an offline capture: the payment settles to `CLO` through the
    /// clearing process.
    #[serde(rename = "pending_offline_capture")]
    PendingOfflineCapture,
    /// A next action this connector does not know.
    #[serde(other)]
    Unknown,
}

/// Result of a card check (`cvv_check`, `acs_check`) in the response
/// `payment_method_data` object.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RapydCheckResult {
    Pass,
    Fail,
    Unavailable,
    Unchecked,
    #[serde(other)]
    Unknown,
}

/// 3DS outcome Rapyd reports in the payment object's `authentication_result`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RapydAuthenticationResult {
    pub result: Option<String>,
    pub eci: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResponseData {
    pub id: String,
    pub amount: Option<FloatMajorUnit>,
    pub status: RapydPaymentStatus,
    pub next_action: Option<NextAction>,
    pub redirect_url: Option<String>,
    pub original_amount: Option<FloatMajorUnit>,
    pub is_partial: Option<bool>,
    pub currency_code: Option<common_enums::Currency>,
    pub country_code: Option<String>,
    pub captured: Option<bool>,
    pub transaction_id: Option<String>,
    pub merchant_reference_id: Option<String>,
    pub paid: Option<bool>,
    pub failure_code: Option<String>,
    pub failure_message: Option<String>,
    pub error_code: Option<String>,
    /// Merchant advice code (MAC) the card network returned with a decline.
    pub merchant_advice_code: Option<String>,
    pub merchant_advice_message: Option<String>,
    pub customer_token: Option<Secret<String>>,
    pub auth_code: Option<String>,
    pub authentication_result: Option<RapydAuthenticationResult>,
    /// Saved-card token (`card_*`) — populated when the payment was
    /// created with `save_payment_method: true`. Used as the MIT token
    /// on subsequent charges.
    pub payment_method: Option<Secret<String>>,
    /// Nested payment-method data; carries `network_reference_id`, the
    /// network transaction id surfaced as `network_txn_id` for recurring.
    pub payment_method_data: Option<RapydResponsePaymentMethodData>,
}

impl ResponseData {
    /// Whether the payment was created for a zero amount (a card
    /// verification). Only `original_amount` says so: `amount` is the
    /// captured amount and is 0 on every uncaptured manual-capture payment,
    /// so a payment object without `original_amount` is not read as a
    /// verification.
    fn is_zero_amount(&self) -> bool {
        self.original_amount == Some(FloatMajorUnit::zero())
    }
}

/// Subset of Rapyd's response `payment_method_data` object.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RapydResponsePaymentMethodData {
    pub network_reference_id: Option<Secret<String>>,
    pub cvv_check: Option<RapydCheckResult>,
    /// Not enumerated by Rapyd's documentation.
    pub avs_check: Option<String>,
    /// AVS result code; appears on payments created with an AVS check.
    pub avs_result: Option<String>,
    pub acs_check: Option<RapydCheckResult>,
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
    /// "Identifier defined by the client for reference purposes."
    /// <https://docs.rapyd.net/en/create-refund.html>
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_reference_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
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
#[allow(dead_code)]
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub enum RefundStatus {
    Completed,
    Error,
    Rejected,
    Canceled,
    #[default]
    Pending,
    /// A status this connector does not know; never terminal.
    #[serde(other)]
    Unknown,
}

impl From<RefundStatus> for common_enums::RefundStatus {
    fn from(item: RefundStatus) -> Self {
        match item {
            RefundStatus::Completed => Self::Success,
            RefundStatus::Error | RefundStatus::Rejected | RefundStatus::Canceled => Self::Failure,
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
    pub payment: Option<String>,
    pub amount: Option<FloatMajorUnit>,
    pub currency: Option<common_enums::Currency>,
    pub status: RefundStatus,
    pub created_at: Option<i64>,
    pub failure_reason: Option<String>,
    pub failure_code: Option<String>,
    pub merchant_reference_id: Option<String>,
}

/// Maps a 2xx Rapyd refund body. A body without the refund object carries no
/// refund id, so it is an error, with the attempt status the calling flow gives
/// it. A refund object in status `Error`, `Rejected` or `Canceled` is an error
/// response too, carrying Rapyd's `failure_code` and `failure_reason`.
/// Refund `status` values: <https://docs.rapyd.net/en/create-refund.html>
fn build_refund_response(
    response: RefundResponse,
    http_code: u16,
    missing_data_status: Option<FlowStatus>,
) -> Result<RefundsResponseData, ErrorResponse> {
    let non_empty = |value: Option<String>| value.filter(|text| !text.is_empty());
    let envelope_code = non_empty(Some(response.status.error_code));
    let envelope_message = non_empty(response.status.message);
    match response.data {
        Some(data) => {
            let refund_status = common_enums::RefundStatus::from(data.status);
            if matches!(refund_status, common_enums::RefundStatus::Failure) {
                let reason = non_empty(data.failure_reason).or(envelope_message);
                return Err(ErrorResponse {
                    code: non_empty(data.failure_code)
                        .or(envelope_code)
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                    status_code: http_code,
                    message: reason
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                    reason,
                    attempt_status: Some(FlowStatus::Refund(common_enums::RefundStatus::Failure)),
                    // The refunded payment; the refund id stays in the raw response.
                    connector_transaction_id: non_empty(data.payment),
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                });
            }
            Ok(RefundsResponseData {
                connector_refund_id: data.id,
                refund_status,
                status_code: http_code,
                acquirer_reference_number: None,
            })
        }
        None => Err(ErrorResponse {
            code: envelope_code.unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
            status_code: http_code,
            message: envelope_message
                .clone()
                .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
            reason: envelope_message,
            attempt_status: missing_data_status,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
            typed_connector_response: None,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        }),
    }
}

impl TryFrom<ResponseRouterData<RefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<RefundResponse, Self>) -> Result<Self, Self::Error> {
        // Create Refund answered without a refund object: no refund was made.
        Ok(Self {
            response: build_refund_response(
                item.response,
                item.http_code,
                Some(FlowStatus::Refund(common_enums::RefundStatus::Failure)),
            ),
            ..item.router_data
        })
    }
}

impl TryFrom<ResponseRouterData<RefundResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<RefundResponse, Self>) -> Result<Self, Self::Error> {
        // A lookup that returns no refund object says nothing about the
        // refund: it is still pending, to be read again.
        Ok(Self {
            response: build_refund_response(
                item.response,
                item.http_code,
                Some(FlowStatus::Refund(common_enums::RefundStatus::Pending)),
            ),
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
// SetupMandate (card verification that stores the card) — Rapyd
// ============================================================================
// Rapyd has no mandate object. A card is stored by a Create Payment call with
// `save_payment_method: true`; the `card_*` id it returns is the mandate
// reference, and `payment_method_data.network_reference_id` the network
// transaction id. A verification sends amount 0 with `capture: false`.
// Reference: https://docs.rapyd.net/en/card-on-file-with-3ds-verification.html

/// SetupMandate request – the `/v1/payments` shape with
/// `save_payment_method: true` and `capture: false`.
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
        let flow_data = &router_data.resource_common_data;

        // Rapyd sends the customer to `complete_payment_url` /
        // `error_payment_url` after the 3DS verification it may apply to a
        // card save, so the call is refused without a return URL.
        let return_url = flow_data
            .return_url
            .clone()
            .or_else(|| request.router_return_url.clone())
            .ok_or_else(|| IntegrationError::MissingRequiredField {
                field_name: "return_url",
                context: crate::utils::integration_ctx(
                    "Rapyd needs complete_payment_url and error_payment_url on a payment that saves a card, because it may route the card verification through 3DS",
                    "Send return_url on the setup-recurring request",
                ),
            })?;

        // No amount is invented: a default would authorise an arbitrary value
        // in the caller's currency. Zero is the card verification.
        let minor_amount =
            request
                .minor_amount
                .ok_or_else(|| IntegrationError::MissingRequiredField {
                    field_name: "minor_amount",
                    context: crate::utils::integration_ctx(
                        "Rapyd Create Payment requires an amount; a card verification sends 0",
                        "Send the amount on the setup-recurring request; 0 verifies the card",
                    ),
                })?;
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

        let (payment_method, payment_method_options) = match &request.payment_method_data {
            PaymentMethodData::Card(ccard) => {
                let pm_type = RapydPaymentMethodType::try_for_raw_card(
                    ccard.card_network.as_ref(),
                    ccard.card_number.peek(),
                    false,
                )?;
                let capabilities = pm_type.capabilities();
                let cardholder_name =
                    rapyd_cardholder_name(ccard.card_holder_name.as_ref(), flow_data)?;
                // The intended use of the saved card, when the caller names
                // one and the payment-method type accepts the member;
                // otherwise Rapyd applies its default, `unscheduled`.
                let recurrence_type = request
                    .mit_category
                    .as_ref()
                    .filter(|_| capabilities.recurrence_type)
                    .map(RapydRecurrenceType::from);
                let payment_method_options = PaymentMethodOptions {
                    three_ds: Some(matches!(
                        flow_data.auth_type,
                        common_enums::AuthenticationType::ThreeDs
                    )),
                    avs_required: (billing_has_avs_inputs(flow_data) && capabilities.avs_required)
                        .then_some(true),
                    ..Default::default()
                };
                (
                    RapydPaymentMethodData::PaymentMethod(Box::new(PaymentMethod {
                        pm_type,
                        fields: Some(RapydPaymentMethodFields::Card(PaymentFields {
                            number: ccard.card_number.to_owned(),
                            expiration_month: ccard.card_exp_month.to_owned(),
                            expiration_year: ccard.card_exp_year.to_owned(),
                            name: cardholder_name,
                            cvv: ccard.card_cvc.to_owned(),
                            recurrence_type,
                        })),
                        address: None,
                        digital_wallet: None,
                    })),
                    payment_method_options,
                )
            }
            _ => {
                return Err(IntegrationError::NotImplemented(
                    "payment_method for rapyd SetupMandate".to_owned(),
                    crate::utils::integration_ctx(
                        "Rapyd saves a payment method for later use only from a card payment",
                        "Send a card as the payment method of the setup-recurring request",
                    ),
                )
                .into());
            }
        };

        Ok(Self {
            amount,
            currency: request.currency,
            payment_method,
            // Rapyd rejects a zero-amount payment with `capture: true`
            // (ERROR_CARD_VALIDATION_CAPTURE_TRUE). Nothing is captured by a
            // mandate setup.
            capture: Some(false),
            payment_method_options: Some(payment_method_options),
            merchant_reference_id: Some(flow_data.connector_request_reference_id.clone()),
            description: flow_data.description.clone(),
            complete_payment_url: Some(return_url.clone()),
            error_payment_url: Some(return_url),
            customer: build_inline_customer(
                request.customer_name.clone().map(Secret::new),
                request.email.clone(),
            ),
            save_payment_method: Some(true),
            // Customer-initiated: Rapyd's default, `customer_present`.
            initiation_type: None,
            address: build_rapyd_address(flow_data),
            statement_descriptor: request
                .billing_descriptor
                .as_ref()
                .and_then(|descriptor| descriptor.statement_descriptor.clone()),
            receipt_email: request
                .email
                .clone()
                .or_else(|| flow_data.get_optional_billing_email()),
            metadata: request.metadata.clone(),
            client_details: build_rapyd_client_details(request.browser_info.as_ref()),
        })
    }
}

/// What a 2xx Rapyd payment body means for a mandate flow.
struct RapydMandateFlowResponse {
    attempt_status: common_enums::AttemptStatus,
    response: Result<PaymentsResponseData, ErrorResponse>,
    /// Card check results, when the payment object reports any.
    connector_response: Option<ConnectorResponseData>,
}

/// Maps a 2xx Rapyd payment body for the two mandate flows. A payment object
/// in status `ERR`, or a body without `data`, is an error response.
fn build_mandate_flow_response(
    status: &Status,
    data: Option<&ResponseData>,
    http_code: u16,
    include_mandate_details: bool,
) -> Result<RapydMandateFlowResponse, error_stack::Report<ConnectorError>> {
    let Some(data) = data else {
        return Ok(RapydMandateFlowResponse {
            attempt_status: common_enums::AttemptStatus::Failure,
            response: Err(build_payment_failure_response(
                status,
                None,
                http_code,
                Some(common_enums::AttemptStatus::Failure),
            )),
            connector_response: None,
        });
    };
    let attempt_status = get_status(
        data.status.to_owned(),
        data.next_action.to_owned(),
        data.is_zero_amount(),
    );
    if attempt_status == common_enums::AttemptStatus::Failure {
        return Ok(RapydMandateFlowResponse {
            attempt_status,
            response: Err(build_payment_failure_response(
                status,
                Some(data),
                http_code,
                Some(common_enums::AttemptStatus::Failure),
            )),
            connector_response: None,
        });
    }
    let (transaction_response, connector_response) =
        build_payment_success_response(data, http_code, include_mandate_details)?;
    Ok(RapydMandateFlowResponse {
        attempt_status,
        response: Ok(transaction_response),
        connector_response,
    })
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
        // The saved-card id (`data.payment_method`) is the mandate reference
        // and `payment_method_data.network_reference_id` the network
        // transaction id. "Card payment method ID is not returned when the
        // next_action is 3d_verification": the id then arrives on the payment
        // sync that follows the challenge.
        // https://docs.rapyd.net/en/card-on-file-with-3ds-verification.html
        let RapydMandateFlowResponse {
            attempt_status,
            response,
            connector_response,
        } = build_mandate_flow_response(
            &item.response.status,
            item.response.data.as_ref(),
            item.http_code,
            true,
        )?;
        // A zero-amount setup is a card verification: nothing was authorised,
        // so the pair Rapyd reports for it (`ACT` + `pending_capture` or
        // `not_applicable`) is its finished state and is reported as `Charged`.
        // A non-zero setup is sent with `capture: false` and leaves a real,
        // uncaptured authorisation: it stays `Authorized`, which is also what
        // payment sync and the payment webhook report for the same payment.
        let is_card_verification = item
            .router_data
            .request
            .minor_amount
            .as_ref()
            .is_some_and(|minor_amount| minor_amount.get_amount_as_i64() == 0);
        let status =
            if is_card_verification && attempt_status == common_enums::AttemptStatus::Authorized {
                common_enums::AttemptStatus::Charged
            } else {
                attempt_status
            };
        let connector_response = connector_response.or_else(|| {
            item.router_data
                .resource_common_data
                .connector_response
                .clone()
        });

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_response,
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}

// ---------------------------------------------------------------------------
// RepeatPayment (merchant-initiated) — Rapyd reuses /v1/payments, either with
// the saved-card id (`card_*`) as `payment_method`, or with the card and the
// network reference id of the initial payment in place of the CVV. The
// request body is `RapydPaymentsRequest`; the response is a distinct newtype
// so the TryFrom impls don't collide with the blanket Authorize conversion.
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
        let flow_data = &router_data.resource_common_data;

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

        let caller_initiation_type = request.mit_category.as_ref().map(RapydInitiationType::from);
        let (payment_method, initiation_type) = match &request.mandate_reference {
            // Card on file: `payment_method` is the saved-card id; no card
            // number or CVV is sent.
            MandateReferenceId::ConnectorMandateId(connector_mandate) => {
                let card_id = connector_mandate
                    .get_connector_mandate_id()
                    .filter(|card_id| !card_id.trim().is_empty())
                    .ok_or_else(|| IntegrationError::MissingRequiredField {
                        field_name: "mandate_reference.connector_mandate_id",
                        context: crate::utils::integration_ctx(
                            "A Rapyd merchant-initiated payment charges the saved card id (card_...) returned when the card was stored",
                            "Send the connector_mandate_id returned by the setup-recurring or customer-initiated payment",
                        ),
                    })?;
                // The saved card's `recurrence_type` decides which
                // `initiation_type` Rapyd accepts. Without a caller value the
                // card was saved with Rapyd's default, `unscheduled`.
                (
                    RapydPaymentMethodData::Token(Secret::new(card_id)),
                    Some(caller_initiation_type.unwrap_or(RapydInitiationType::Unscheduled)),
                )
            }
            // Network reference id: the card held by the merchant, with the
            // id of the initial payment in place of the CVV.
            // https://docs.rapyd.net/en/creating-a-card-payment-with-a-network-reference-id.html
            MandateReferenceId::NetworkMandateId(network_mandate) => {
                let network_reference_id =
                    Secret::new(network_mandate.network_transaction_id.clone());
                let (card_network, card_number, fields) = match &request.payment_method_data {
                    PaymentMethodData::Card(card) => (
                        card.card_network.as_ref(),
                        card.card_number.peek(),
                        RapydNetworkReferenceFields {
                            number: RapydCardNumber::Card(card.card_number.clone()),
                            expiration_month: card.card_exp_month.clone(),
                            expiration_year: card.card_exp_year.clone(),
                            name: Some(rapyd_cardholder_name(
                                card.card_holder_name.as_ref(),
                                flow_data,
                            )?),
                            network_reference_id,
                        },
                    ),
                    PaymentMethodData::CardDetailsForNetworkTransactionId(card) => (
                        card.card_network.as_ref(),
                        card.card_number.peek().as_str(),
                        RapydNetworkReferenceFields {
                            number: RapydCardNumber::StoredCard(card.card_number.clone()),
                            expiration_month: card.card_exp_month.clone(),
                            expiration_year: card.card_exp_year.clone(),
                            name: Some(rapyd_cardholder_name(
                                card.card_holder_name.as_ref(),
                                flow_data,
                            )?),
                            network_reference_id,
                        },
                    ),
                    _ => {
                        return Err(IntegrationError::MissingRequiredField {
                            field_name: "payment_method_data",
                            context: crate::utils::integration_ctx(
                                "A Rapyd payment made with a network reference id carries the card number and expiry held by the merchant",
                                "Send the card as the payment method together with network_mandate_id",
                            ),
                        }
                        .into());
                    }
                };
                // Only the `gb_` card types list `fields.network_reference_id`;
                // a card network without one is refused by the resolver.
                let pm_type =
                    RapydPaymentMethodType::try_for_raw_card(card_network, card_number, true)?;
                (
                    RapydPaymentMethodData::PaymentMethod(Box::new(PaymentMethod {
                        pm_type,
                        fields: Some(RapydPaymentMethodFields::NetworkReference(fields)),
                        address: None,
                        digital_wallet: None,
                    })),
                    // The documented network-reference-id payment sends no
                    // `initiation_type`; one is sent only when the caller
                    // names the category.
                    caller_initiation_type,
                )
            }
            MandateReferenceId::NetworkTokenWithNTI(_) => {
                return Err(IntegrationError::NotSupported {
                    message: "network token with network transaction id".to_string(),
                    connector: "rapyd",
                    context: crate::utils::integration_ctx(
                        "Rapyd documents merchant-initiated payments on a saved card id or on a card with a network reference id, not on a network token",
                        "Use connector_mandate_id, or network_mandate_id with the card",
                    ),
                }
                .into());
            }
        };

        // On Charge the return URL arrives on `request.router_return_url`.
        // Rapyd takes `complete_payment_url` on every Create Payment.
        let return_url = request
            .router_return_url
            .clone()
            .or_else(|| flow_data.return_url.clone())
            .ok_or_else(|| IntegrationError::MissingRequiredField {
                field_name: "return_url",
                context: crate::utils::integration_ctx(
                    "Rapyd Create Payment takes complete_payment_url and error_payment_url on a card payment",
                    "Send return_url on the recurring charge request",
                ),
            })?;

        Ok(Self {
            amount,
            currency: request.currency,
            payment_method,
            // The caller's capture intent, resolved and refused exactly as on
            // Authorize.
            capture: Some(rapyd_capture_flag(request.capture_method)?),
            payment_method_options: Some(PaymentMethodOptions {
                three_ds: Some(matches!(
                    flow_data.auth_type,
                    common_enums::AuthenticationType::ThreeDs
                )),
                ..Default::default()
            }),
            merchant_reference_id: Some(flow_data.connector_request_reference_id.clone()),
            description: flow_data.description.clone(),
            error_payment_url: Some(return_url.clone()),
            complete_payment_url: Some(return_url),
            customer: None,
            save_payment_method: None,
            initiation_type,
            address: build_rapyd_address(flow_data),
            statement_descriptor: request
                .billing_descriptor
                .as_ref()
                .and_then(|descriptor| descriptor.statement_descriptor.clone()),
            receipt_email: None,
            metadata: request.metadata.clone(),
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
        // A merchant-initiated payment creates no mandate: `data.id` is a
        // one-off payment id, and "the response in each subsequent payment
        // contains a different network reference ID, which you do not use",
        // so neither a mandate reference nor a network transaction id is
        // returned and the ones stored from the initial payment stay.
        // https://docs.rapyd.net/en/creating-a-card-payment-with-a-network-reference-id.html
        let RapydMandateFlowResponse {
            attempt_status: status,
            response,
            connector_response,
        } = build_mandate_flow_response(
            &item.response.status,
            item.response.data.as_ref(),
            item.http_code,
            false,
        )?;
        let connector_response = connector_response.or_else(|| {
            item.router_data
                .resource_common_data
                .connector_response
                .clone()
        });

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_response,
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}

// ---- IncomingWebhook ----

/// HTTP status reported for a webhook Rapyd delivered: the notification itself
/// is the response being described.
const WEBHOOK_STATUS_CODE: u16 = 200;

/// `type` of a Rapyd webhook. It names the event and, with it, the object the
/// `data` member carries (a payment, a refund or a dispute).
/// <https://docs.rapyd.net/en/webhook-format.html>
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RapydWebhookEventType {
    PaymentSucceeded,
    PaymentCompleted,
    PaymentCaptured,
    PaymentFailed,
    PaymentCanceled,
    PaymentExpired,
    PaymentReversed,
    PaymentUpdated,
    RefundCompleted,
    PaymentRefundFailed,
    PaymentRefundRejected,
    PaymentRefundUpdated,
    PaymentDisputeCreated,
    PaymentDisputeUpdated,
    CardAddedSuccessfully,
    /// An event type this connector does not know.
    #[serde(other)]
    Unknown,
}

/// Body of a Rapyd webhook. Only `type` and `data` are relied on: the
/// documented samples send the other envelope members empty, zero or absent.
/// <https://docs.rapyd.net/en/webhook-format.html>
#[derive(Debug, Clone, Deserialize)]
pub struct RapydIncomingWebhook {
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub webhook_type: RapydWebhookEventType,
    /// Read as the object `type` names, never guessed from its shape: a refund
    /// object would otherwise satisfy the payment object's required members.
    pub data: SecretSerdeValue,
    pub trigger_operation_id: Option<String>,
    /// Delivery status of the webhook itself (`NEW`, `CLO`, `ERR`, `RET`); the
    /// status of the payment, refund or dispute is inside `data`.
    pub status: Option<String>,
    /// Unix time; documented as a string and sent as a number.
    pub created_at: Option<serde_json::Value>,
}

/// The typed `data` object of a webhook.
#[derive(Debug, Clone)]
pub enum RapydWebhookResource {
    Payment(Box<ResponseData>),
    Refund(RefundResponseData),
    Dispute(RapydDisputeData),
    /// The event carries no payment, refund or dispute object.
    NotSupported,
}

/// Dispute object of the `PAYMENT_DISPUTE_CREATED` and
/// `PAYMENT_DISPUTE_UPDATED` webhooks.
/// <https://docs.rapyd.net/en/dispute-created-webhook.html>
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct RapydDisputeData {
    /// Dispute id (`dispute_...`).
    pub token: String,
    /// The disputed payment (`payment_...`).
    pub original_transaction_id: String,
    /// Disputed amount, in decimal major units of `currency`.
    pub amount: FloatMajorUnit,
    pub currency: common_enums::Currency,
    pub status: RapydDisputeStatus,
    pub dispute_reason_description: Option<String>,
    pub dispute_category: Option<String>,
    /// Unix time by which the merchant has to respond.
    pub due_date: Option<i64>,
}

/// Dispute `status` values.
/// <https://docs.rapyd.net/en/dispute-updated-webhook.html>
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub enum RapydDisputeStatus {
    /// Awaiting action by the merchant.
    #[serde(rename = "ACT")]
    Active,
    /// Rapyd is reviewing the merchant's evidence.
    #[serde(rename = "RVW")]
    Review,
    /// The issuer challenged a contested dispute.
    #[serde(rename = "PRA")]
    PreArbitration,
    /// Awaiting a ruling by the card scheme.
    #[serde(rename = "ARB")]
    Arbitration,
    #[serde(rename = "LOS")]
    Lose,
    #[serde(rename = "WIN")]
    Win,
    /// The issuer reversed the dispute; the funds went back to the merchant.
    #[serde(rename = "REV")]
    Reverse,
    /// A status this connector does not know.
    #[serde(other)]
    Unknown,
}

impl RapydDisputeStatus {
    /// `None` for a status this connector does not know.
    fn dispute_status(self) -> Option<common_enums::DisputeStatus> {
        match self {
            Self::Active => Some(common_enums::DisputeStatus::DisputeOpened),
            Self::Review | Self::PreArbitration | Self::Arbitration => {
                Some(common_enums::DisputeStatus::DisputeChallenged)
            }
            Self::Lose => Some(common_enums::DisputeStatus::DisputeLost),
            Self::Win => Some(common_enums::DisputeStatus::DisputeWon),
            Self::Reverse => Some(common_enums::DisputeStatus::DisputeCancelled),
            Self::Unknown => None,
        }
    }

    fn dispute_stage(self) -> common_enums::DisputeStage {
        match self {
            Self::PreArbitration | Self::Arbitration => common_enums::DisputeStage::PreArbitration,
            Self::Active
            | Self::Review
            | Self::Lose
            | Self::Win
            | Self::Reverse
            | Self::Unknown => common_enums::DisputeStage::Dispute,
        }
    }
}

fn dispute_event_type(status: common_enums::DisputeStatus) -> EventType {
    match status {
        common_enums::DisputeStatus::DisputeOpened => EventType::DisputeOpened,
        common_enums::DisputeStatus::DisputeExpired => EventType::DisputeExpired,
        common_enums::DisputeStatus::DisputeAccepted => EventType::DisputeAccepted,
        common_enums::DisputeStatus::DisputeCancelled => EventType::DisputeCancelled,
        common_enums::DisputeStatus::DisputeChallenged => EventType::DisputeChallenged,
        common_enums::DisputeStatus::DisputeWon => EventType::DisputeWon,
        common_enums::DisputeStatus::DisputeLost => EventType::DisputeLost,
    }
}

/// The account's API key pair, which is what a Rapyd webhook is signed with:
/// Rapyd issues no separate webhook secret. It is supplied as the webhook
/// secret, a JSON object `{"access_key": ..., "secret_key": ...}`.
/// <https://docs.rapyd.net/en/webhook-authentication.html>
#[derive(Debug, Deserialize)]
pub struct RapydWebhookSecret {
    pub(super) access_key: Secret<String>,
    pub(super) secret_key: Secret<String>,
}

impl TryFrom<&ConnectorWebhookSecrets> for RapydWebhookSecret {
    type Error = error_stack::Report<WebhookError>;
    fn try_from(secrets: &ConnectorWebhookSecrets) -> Result<Self, Self::Error> {
        if secrets.secret.is_empty() {
            return Err(error_stack::report!(
                WebhookError::WebhookVerificationSecretNotFound
            ));
        }
        // The parser's error is dropped on purpose: it can quote the input,
        // and the input here is the key pair.
        let keys: Self = serde_json::from_slice(&secrets.secret)
            .map_err(|_| error_stack::report!(WebhookError::WebhookVerificationSecretInvalid))?;
        if keys.access_key.peek().is_empty() || keys.secret_key.peek().is_empty() {
            return Err(error_stack::report!(
                WebhookError::WebhookVerificationSecretInvalid
            ));
        }
        Ok(keys)
    }
}

fn non_empty_text(value: Option<&String>) -> Option<String> {
    value.filter(|text| !text.is_empty()).cloned()
}

fn raw_webhook_body(body: &[u8]) -> Option<String> {
    Some(String::from_utf8_lossy(body).into_owned())
}

/// Reads `data` as `T`. The parser's error is not attached: it can quote
/// values of the payment, refund or dispute object.
fn parse_webhook_data<T: serde::de::DeserializeOwned>(
    data: &serde_json::Value,
) -> Result<T, error_stack::Report<WebhookError>> {
    T::deserialize(data).map_err(|_| {
        error_stack::report!(WebhookError::WebhookBodyDecodingFailed)
            .attach_printable("rapyd webhook: `data` is not the object its `type` names")
    })
}

fn webhook_object_mismatch(expected: &'static str) -> error_stack::Report<WebhookError> {
    error_stack::report!(WebhookError::WebhookBodyDecodingFailed).attach_printable(format!(
        "rapyd webhook: the event `type` does not carry a {expected} object"
    ))
}

impl RapydIncomingWebhook {
    pub fn from_body(body: &[u8]) -> Result<Self, error_stack::Report<WebhookError>> {
        // The parser's error is not attached: it can quote the body.
        serde_json::from_slice(body).map_err(|_| {
            error_stack::report!(WebhookError::WebhookBodyDecodingFailed)
                .attach_printable("rapyd webhook: body is not a JSON object with `type` and `data`")
        })
    }

    /// The `data` object, read as the object the event `type` documents.
    pub fn resource(&self) -> Result<RapydWebhookResource, error_stack::Report<WebhookError>> {
        let data = self.data.peek();
        match self.webhook_type {
            RapydWebhookEventType::PaymentSucceeded
            | RapydWebhookEventType::PaymentCompleted
            | RapydWebhookEventType::PaymentCaptured
            | RapydWebhookEventType::PaymentFailed
            | RapydWebhookEventType::PaymentCanceled
            | RapydWebhookEventType::PaymentExpired
            | RapydWebhookEventType::PaymentReversed
            | RapydWebhookEventType::PaymentUpdated => parse_webhook_data::<ResponseData>(data)
                .map(|payment| RapydWebhookResource::Payment(Box::new(payment))),
            RapydWebhookEventType::RefundCompleted
            | RapydWebhookEventType::PaymentRefundFailed
            | RapydWebhookEventType::PaymentRefundRejected
            | RapydWebhookEventType::PaymentRefundUpdated => {
                parse_webhook_data::<RefundResponseData>(data).map(RapydWebhookResource::Refund)
            }
            RapydWebhookEventType::PaymentDisputeCreated
            | RapydWebhookEventType::PaymentDisputeUpdated => {
                parse_webhook_data::<RapydDisputeData>(data).map(RapydWebhookResource::Dispute)
            }
            RapydWebhookEventType::CardAddedSuccessfully | RapydWebhookEventType::Unknown => {
                Ok(RapydWebhookResource::NotSupported)
            }
        }
    }

    /// Dispute status a dispute event reports: a created dispute is open, an
    /// updated one is whatever its `status` says. `None` when that status is
    /// not known here, or when the event is not a dispute event.
    fn dispute_status(&self, dispute: &RapydDisputeData) -> Option<common_enums::DisputeStatus> {
        match self.webhook_type {
            RapydWebhookEventType::PaymentDisputeCreated => {
                Some(common_enums::DisputeStatus::DisputeOpened)
            }
            RapydWebhookEventType::PaymentDisputeUpdated => dispute.status.dispute_status(),
            RapydWebhookEventType::PaymentSucceeded
            | RapydWebhookEventType::PaymentCompleted
            | RapydWebhookEventType::PaymentCaptured
            | RapydWebhookEventType::PaymentFailed
            | RapydWebhookEventType::PaymentCanceled
            | RapydWebhookEventType::PaymentExpired
            | RapydWebhookEventType::PaymentReversed
            | RapydWebhookEventType::PaymentUpdated
            | RapydWebhookEventType::RefundCompleted
            | RapydWebhookEventType::PaymentRefundFailed
            | RapydWebhookEventType::PaymentRefundRejected
            | RapydWebhookEventType::PaymentRefundUpdated
            | RapydWebhookEventType::CardAddedSuccessfully
            | RapydWebhookEventType::Unknown => None,
        }
    }

    /// Event the webhook reports. Events documented at
    /// <https://docs.rapyd.net/en/webhooks-542504.html>.
    ///
    /// `PAYMENT_SUCCEEDED` only says the Create Payment request was received
    /// (a 3DS payment is still `ACT`), and the `*_UPDATED` / `PAYMENT_REVERSED`
    /// events do not name an outcome, so none of them is mapped to one.
    pub fn event_type(&self) -> Result<EventType, error_stack::Report<WebhookError>> {
        match self.webhook_type {
            RapydWebhookEventType::PaymentCompleted | RapydWebhookEventType::PaymentCaptured => {
                Ok(EventType::PaymentIntentSuccess)
            }
            RapydWebhookEventType::PaymentFailed => Ok(EventType::PaymentIntentFailure),
            RapydWebhookEventType::PaymentCanceled => Ok(EventType::PaymentIntentCancelled),
            RapydWebhookEventType::PaymentExpired => Ok(EventType::PaymentIntentExpired),
            RapydWebhookEventType::RefundCompleted => Ok(EventType::RefundSuccess),
            RapydWebhookEventType::PaymentRefundFailed
            | RapydWebhookEventType::PaymentRefundRejected => Ok(EventType::RefundFailure),
            RapydWebhookEventType::PaymentDisputeCreated
            | RapydWebhookEventType::PaymentDisputeUpdated => {
                let dispute = parse_webhook_data::<RapydDisputeData>(self.data.peek())?;
                Ok(self.dispute_status(&dispute).map_or(
                    EventType::IncomingWebhookEventUnspecified,
                    dispute_event_type,
                ))
            }
            RapydWebhookEventType::PaymentSucceeded
            | RapydWebhookEventType::PaymentUpdated
            | RapydWebhookEventType::PaymentReversed
            | RapydWebhookEventType::PaymentRefundUpdated
            | RapydWebhookEventType::CardAddedSuccessfully
            | RapydWebhookEventType::Unknown => Ok(EventType::IncomingWebhookEventUnspecified),
        }
    }

    /// Ids of the record the event is about: the ones the payment and refund
    /// flows return, so the event resolves to the record those flows created.
    pub fn reference(
        &self,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        Ok(match self.resource()? {
            RapydWebhookResource::Payment(payment) => {
                Some(WebhookResourceReference::Payment(PaymentWebhookReference {
                    merchant_transaction_id: non_empty_text(payment.merchant_reference_id.as_ref()),
                    connector_transaction_id: Some(payment.id),
                }))
            }
            RapydWebhookResource::Refund(refund) => {
                Some(WebhookResourceReference::Refund(RefundWebhookReference {
                    merchant_refund_id: non_empty_text(refund.merchant_reference_id.as_ref()),
                    connector_transaction_id: non_empty_text(refund.payment.as_ref()),
                    merchant_transaction_id: None,
                    connector_refund_id: Some(refund.id),
                }))
            }
            RapydWebhookResource::Dispute(dispute) => {
                Some(WebhookResourceReference::Dispute(DisputeWebhookReference {
                    connector_dispute_id: Some(dispute.token),
                    connector_transaction_id: Some(dispute.original_transaction_id),
                }))
            }
            RapydWebhookResource::NotSupported => None,
        })
    }

    /// Payment outcome of a payment event, read from the payment object with
    /// the same status table the payment flows use.
    pub fn payment_details(
        &self,
        raw_body: &[u8],
    ) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let RapydWebhookResource::Payment(payment) = self.resource()? else {
            return Err(webhook_object_mismatch("payment"));
        };
        let status = get_status(
            payment.status.clone(),
            payment.next_action.clone(),
            payment.is_zero_amount(),
        );
        let is_failure = status == common_enums::AttemptStatus::Failure;

        // On a closed payment `amount` is what was collected.
        let minor_amount_captured = if payment.status == RapydPaymentStatus::Closed {
            payment
                .amount
                .zip(payment.currency_code)
                .map(|(amount, currency)| {
                    domain_types::utils::convert_back_amount_to_minor_units_for_webhook(
                        &FloatMajorUnitForConnector,
                        amount,
                        currency,
                    )
                })
                .transpose()?
        } else {
            None
        };

        // Decline details: the card network code, its text, and the network's
        // retry advice. <https://docs.rapyd.net/en/merchant-advice-codes.html>
        let (error_code, error_message, error_reason) = if is_failure {
            let advice = match (
                non_empty_text(payment.merchant_advice_code.as_ref()),
                non_empty_text(payment.merchant_advice_message.as_ref()),
            ) {
                (Some(code), Some(message)) => Some(format!("{code}: {message}")),
                (Some(code), None) => Some(code),
                (None, Some(message)) => Some(message),
                (None, None) => None,
            };
            (
                non_empty_text(payment.failure_code.as_ref())
                    .or_else(|| non_empty_text(payment.error_code.as_ref())),
                non_empty_text(payment.failure_message.as_ref()),
                advice,
            )
        } else {
            (None, None, None)
        };

        let mandate_reference =
            payment
                .payment_method
                .as_ref()
                .filter(|_| !is_failure)
                .map(|card| {
                    Box::new(MandateReference {
                        connector_mandate_id: Some(card.clone().expose()),
                        payment_method_id: None,
                        connector_mandate_request_reference_id: None,
                        mandate_metadata: None,
                    })
                });
        let network_txn_id = payment
            .payment_method_data
            .as_ref()
            .filter(|_| !is_failure)
            .and_then(|method_data| method_data.network_reference_id.clone())
            .map(|network_reference_id| network_reference_id.expose());

        Ok(WebhookDetailsResponse {
            connector_response_reference_id: non_empty_text(payment.merchant_reference_id.as_ref()),
            resource_id: Some(ResponseId::ConnectorTransactionId(payment.id)),
            status,
            connector_request_reference_id: None,
            mandate_reference,
            error_code,
            error_message,
            error_reason,
            raw_connector_response: raw_webhook_body(raw_body),
            status_code: WEBHOOK_STATUS_CODE,
            response_headers: None,
            amount_captured: minor_amount_captured.map(|amount| amount.get_amount_as_i64()),
            minor_amount_captured,
            network_txn_id,
            payment_method_update: None,
            sender_payment_instrument_id: None,
            connector_returned_payment_method_details: None,
        })
    }

    /// Refund outcome of a refund event. Refund `status` values:
    /// <https://docs.rapyd.net/en/create-refund.html>
    pub fn refund_details(
        &self,
        raw_body: &[u8],
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let RapydWebhookResource::Refund(refund) = self.resource()? else {
            return Err(webhook_object_mismatch("refund"));
        };
        let merchant_reference_id = non_empty_text(refund.merchant_reference_id.as_ref());
        Ok(RefundWebhookDetailsResponse {
            error_code: non_empty_text(refund.failure_code.as_ref()),
            error_message: non_empty_text(refund.failure_reason.as_ref()),
            connector_refund_id: Some(refund.id),
            merchant_transaction_id: merchant_reference_id.clone(),
            status: common_enums::RefundStatus::from(refund.status),
            connector_response_reference_id: merchant_reference_id,
            raw_connector_response: raw_webhook_body(raw_body),
            status_code: WEBHOOK_STATUS_CODE,
            response_headers: None,
        })
    }

    /// Dispute a dispute event reports. The amount is in decimal major units
    /// (the documented samples send `10` for a 10.00 USD dispute).
    pub fn dispute_details(
        &self,
        raw_body: &[u8],
    ) -> Result<DisputeWebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let RapydWebhookResource::Dispute(dispute) = self.resource()? else {
            return Err(webhook_object_mismatch("dispute"));
        };
        let status = self.dispute_status(&dispute).ok_or_else(|| {
            error_stack::report!(WebhookError::WebhookProcessingFailed)
                .attach_printable("rapyd webhook: dispute status is not one this connector knows")
        })?;
        let minor_amount = domain_types::utils::convert_back_amount_to_minor_units_for_webhook(
            &FloatMajorUnitForConnector,
            dispute.amount,
            dispute.currency,
        )?;
        let amount = StringMinorUnitForConnector
            .convert(minor_amount, dispute.currency)
            .map_err(|_| {
                error_stack::report!(WebhookError::WebhookAmountConversionFailed {
                    reason: format!(
                        "Failed to express the dispute amount in minor units: currency={}",
                        dispute.currency
                    ),
                })
            })?;
        Ok(DisputeWebhookDetailsResponse {
            amount,
            currency: dispute.currency,
            status,
            stage: dispute.status.dispute_stage(),
            connector_response_reference_id: Some(dispute.original_transaction_id),
            dispute_message: non_empty_text(dispute.dispute_reason_description.as_ref()),
            connector_reason_code: non_empty_text(dispute.dispute_category.as_ref()),
            dispute_id: dispute.token,
            raw_connector_response: raw_webhook_body(raw_body),
            status_code: WEBHOOK_STATUS_CODE,
            response_headers: None,
            additional_details: None,
        })
    }
}
