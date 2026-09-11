use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    ext_traits::OptionExt,
    pii::Email,
    request::Method,
    types::MinorUnit,
    FloatMajorUnit, StringMajorUnit,
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
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::Deserialize;
use serde::Serialize;
use std::fmt::Debug;
use time::PrimitiveDateTime;
use url::Url;

use crate::types::ResponseRouterData;

use super::RapydRouterData;

/// Rapyd's request-signing / authentication reference. Surfaced on
/// `IntegrationErrorContext::doc_url` so an operator hitting an auth failure is
/// pointed at the page that explains the `access_key`/`secret_key` pair and the
/// HMAC preimage, rather than at an opaque `InvalidDataFormat` at the gRPC
/// boundary.
pub(super) const RAPYD_AUTH_DOC_URL: &str = "https://docs.rapyd.net/en/authentication.html";

/// Rapyd's master error-code catalogue, including the transport-class codes
/// (`MISSING_AUTHENTICATION_HEADERS`, `UNAUTHENTICATED_API_CALL`) that this
/// connector must never report as a card decline.
pub(super) const RAPYD_ERROR_CODES_DOC_URL: &str =
    "https://docs.rapyd.net/en/rapyd-error-codes.html";

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

/// Rapyd `initiation_type` for `/v1/payments`. MIT replays go out as
/// `recurring`; the full Rapyd vocabulary also includes `customer_present`,
/// `installment`, `moto`, and `unscheduled`, but only `recurring` is used
/// on this path today.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RapydInitiationType {
    Recurring,
}

impl<F, T> TryFrom<ResponseRouterData<RapydPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<RapydPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let (status, response) = match &item.response.data {
            Some(data) => {
                let attempt_status = get_status_for_payment_response(data, item.http_code)?;
                match attempt_status {
                    common_enums::AttemptStatus::Failure => (
                        common_enums::AttemptStatus::Failure,
                        Err(rapyd_error_response(
                            item.http_code,
                            classify_rapyd_error(item.http_code, &item.response.status, Some(data)),
                            &item.response.status,
                            Some(data),
                            Some(data.id.clone()),
                            // A 200 whose body reports a failed payment: the
                            // attempt is terminal and this is the payment path,
                            // so the computed `AttemptStatus` is carried through
                            // rather than left for the http-2xx fallback to
                            // reconstruct.
                            Some(FlowStatus::Payment(common_enums::AttemptStatus::Failure)),
                        )),
                    ),
                    _ => {
                        let redirection_data = build_redirection_data(data, item.http_code)?;

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
                                connector_metadata: build_connector_metadata(data),
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
                        )
                    }
                }
            }
            None => (
                common_enums::AttemptStatus::Failure,
                Err(rapyd_error_response(
                    item.http_code,
                    classify_rapyd_error(item.http_code, &item.response.status, None),
                    &item.response.status,
                    None,
                    None,
                    Some(FlowStatus::Payment(common_enums::AttemptStatus::Failure)),
                )),
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
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Configure this merchant with Rapyd credentials: the connector config must \
                         be the `Rapyd` variant carrying `access_key` and `secret_key`."
                            .to_owned(),
                    ),
                    doc_url: Some(RAPYD_AUTH_DOC_URL.to_owned()),
                    additional_context: Some(
                        "rapyd: connector_config was not the Rapyd variant".to_owned(),
                    ),
                },
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
    /// Browser/device context for a 3DS challenge. Omitted entirely when the
    /// request carried no usable browser information, and on the external-3DS
    /// pass-through path (there is no challenge for an ACS to risk-assess).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_details: Option<RapydClientDetails>,
    /// Rapyd documents this as required for Visa 3DS. Sent when the request
    /// carries an email; never hard-required, so a non-3DS payment that always
    /// worked without one keeps working.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt_email: Option<Email>,
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

/// Rapyd `payment_method_options` on `POST /v1/payments`.
///
/// Two mutually exclusive 3DS modes ride on this one object:
///
/// * **Mode A — Rapyd-hosted 3DS.** `3d_required: true` asks Rapyd to run the
///   challenge itself and hand back a `redirect_url`.
/// * **Mode B — external 3DS pass-through.** The merchant already authenticated
///   the cardholder with its own 3DS server / MPI, so the cryptogram
///   (`cavv` + `eci` + `3d_version` + `ds_trans_id`) is handed to Rapyd inline and
///   Rapyd authorises without a challenge. `3d_required` is sent as `false` on
///   this path — asking Rapyd to run its own challenge as well would defeat the
///   point of pass-through.
///
/// Every Mode B field is `skip_serializing_if = "Option::is_none"`, so a
/// non-3DS or Mode A request serialises exactly the body this connector shipped
/// before external 3DS existed.
///
/// Deliberately absent:
/// * `xid` — Rapyd documents it (Base64, "required for VISA 1.0"), but our
///   `AuthenticationData` carries no XID. `ds_trans_id` is a *different*
///   identifier and must NOT be substituted into `xid`: Rapyd has both slots
///   (unlike Datatrans, which has only one and therefore does substitute).
/// * `tavv` — a network-token cryptogram, not a 3DS one. Different producer,
///   different flow; never populate it from `cavv`.
///
/// Reference: <https://docs.rapyd.net/en/get-payment-method-required-fields.html>
#[derive(Debug, Serialize)]
pub struct PaymentMethodOptions {
    /// Ask Rapyd to run its own 3DS challenge. Rapyd's machine-readable
    /// required-fields schema types this `boolean` and every documented request
    /// example sends a JSON boolean, so a boolean is what we send. (Two prose
    /// doc pages annotate it `string|boolean`; the defensiveness for that lives
    /// on the response side — see `RapydResponsePaymentMethodOptions`.)
    #[serde(rename = "3d_required")]
    pub three_ds: bool,

    // ---- Mode B: external 3DS pass-through. All omitted on Mode A / no-3DS. ----
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

impl PaymentMethodOptions {
    /// Mode A / no-3DS: only the `3d_required` switch, derived from the request's
    /// `auth_type`. Never a constant.
    pub(super) fn rapyd_hosted(three_ds: bool) -> Self {
        Self {
            three_ds,
            three_ds_version: None,
            cavv: None,
            eci: None,
            ds_trans_id: None,
            sca_exemption: None,
        }
    }
}

/// Request-side `3d_version`. Rapyd validates against the regex
/// `(1.0.2|2.1.0|2.2.0)` — only these three strings are accepted, so the closed
/// set is modelled as an enum rather than a stringified `SemanticVersion`.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum RapydThreeDsVersion {
    #[serde(rename = "1.0.2")]
    V1_0_2,
    #[serde(rename = "2.1.0")]
    V2_1_0,
    #[serde(rename = "2.2.0")]
    V2_2_0,
}

impl RapydThreeDsVersion {
    /// Map our `SemanticVersion` onto Rapyd's three accepted strings. Anything
    /// else (e.g. `2.3.0`) is an error, never a raw string in the body — Rapyd
    /// would reject the whole payment.
    fn from_semantic_version(
        version: &common_utils::types::SemanticVersion,
    ) -> Result<Self, error_stack::Report<IntegrationError>> {
        match version.to_string().as_str() {
            "1.0.2" => Ok(Self::V1_0_2),
            "2.1.0" => Ok(Self::V2_1_0),
            "2.2.0" => Ok(Self::V2_2_0),
            other => Err(IntegrationError::NotSupported {
                message: format!("3DS protocol version {other}"),
                connector: "rapyd",
                context: crate::utils::integration_ctx(
                    format!("Rapyd accepts only 3DS versions 1.0.2, 2.1.0 and 2.2.0 in payment_method_options.3d_version; the authentication supplied {other}"),
                    "Re-run the external 3DS authentication with a protocol version Rapyd supports, or route this payment through Rapyd-hosted 3DS.",
                ),
            })?,
        }
    }

    /// True for the 3DS 2.x protocol versions, which is when Rapyd documents
    /// `ds_trans_id` as required for Mastercard.
    fn is_two_x(self) -> bool {
        matches!(self, Self::V2_1_0 | Self::V2_2_0)
    }
}

/// Request-side ECI. Regex from Rapyd's required-fields schema:
/// `(01|02|05|06|07|08)`. Deliberately a *different* set from the response-side
/// values Rapyd documents (`05/02`, `06/01`, `07/00`) — the two sides of the
/// wire do not agree, so they are not modelled by one enum.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
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

impl RapydEciRequest {
    fn from_authentication_eci(eci: &str) -> Result<Self, error_stack::Report<IntegrationError>> {
        match eci.trim() {
            "01" | "1" => Ok(Self::E01),
            "02" | "2" => Ok(Self::E02),
            "05" | "5" => Ok(Self::E05),
            "06" | "6" => Ok(Self::E06),
            "07" | "7" => Ok(Self::E07),
            "08" | "8" => Ok(Self::E08),
            other => Err(IntegrationError::NotSupported {
                message: format!("3DS ECI value {other}"),
                connector: "rapyd",
                context: crate::utils::integration_ctx(
                    format!("Rapyd validates payment_method_options.eci against (01|02|05|06|07|08); the authentication supplied {other}"),
                    "Send the ECI exactly as the card network returned it (two digits), or omit the external authentication data and use Rapyd-hosted 3DS.",
                ),
            })?,
        }
    }
}

/// Request-side `sca_exemption`. Rapyd's regex is
/// `(low_value|transaction_risk_analysis|authentication_outage|secure_corporate_payments)`.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RapydScaExemption {
    LowValue,
    TransactionRiskAnalysis,
    AuthenticationOutage,
    SecureCorporatePayments,
}

impl RapydScaExemption {
    /// Partial map. Our `ExemptionIndicator` carries variants Rapyd has no value
    /// for; those return `None` so the field is omitted entirely rather than
    /// substituted with a near-neighbour or an empty string.
    fn from_exemption_indicator(indicator: &common_enums::ExemptionIndicator) -> Option<Self> {
        match indicator {
            common_enums::ExemptionIndicator::LowValue => Some(Self::LowValue),
            common_enums::ExemptionIndicator::TransactionRiskAssessment => {
                Some(Self::TransactionRiskAnalysis)
            }
            common_enums::ExemptionIndicator::ThreeDsOutage => Some(Self::AuthenticationOutage),
            common_enums::ExemptionIndicator::SecureCorporatePayment => {
                Some(Self::SecureCorporatePayments)
            }
            common_enums::ExemptionIndicator::TrustedListing
            | common_enums::ExemptionIndicator::ScaDelegation
            | common_enums::ExemptionIndicator::OutOfScaScope
            | common_enums::ExemptionIndicator::LowRiskProgram
            | common_enums::ExemptionIndicator::RecurringOperation
            | common_enums::ExemptionIndicator::Other => None,
        }
    }
}

/// Build `payment_method_options` for a card Authorize.
///
/// The three-way selection, all of it derived from the request — never from a
/// constant, a config flag or connector metadata:
///
/// | condition | mode | body |
/// |---|---|---|
/// | `authentication_data.is_some()` | B | `3d_required: false` + the cryptogram |
/// | else `auth_type == ThreeDs`     | A | `3d_required: true` |
/// | else                            | N | `3d_required: false` |
///
/// Mode B wins over `auth_type`: if the merchant already authenticated the
/// cardholder we must not also ask Rapyd to challenge them.
///
/// Note that Rapyd may return a `3d_verification` next-action even on modes B
/// and N (PSD2/SCA and issuer discretion) — the response handler is therefore
/// mode-agnostic and never gates redirect handling on what the request asked for.
pub(super) fn build_card_payment_method_options(
    authentication_data: Option<&domain_types::router_request_types::AuthenticationData>,
    wants_rapyd_hosted_three_ds: bool,
    card_network: Option<&common_enums::CardNetwork>,
) -> Result<PaymentMethodOptions, error_stack::Report<IntegrationError>> {
    let Some(auth) = authentication_data else {
        return Ok(PaymentMethodOptions::rapyd_hosted(
            wants_rapyd_hosted_three_ds,
        ));
    };

    // `trans_status` has no Rapyd target field and is never serialised. It is
    // used only as a local guard: a cryptogram from an authentication that did
    // not succeed (or that is still mid-challenge) carries no liability shift,
    // and sending it would have Rapyd reject the payment at the network.
    match auth.trans_status {
        // `Y` authenticated, `A` attempted — both carry a usable liability shift.
        Some(common_enums::TransactionStatus::Success)
        | Some(common_enums::TransactionStatus::NotVerified)
        // Absent: older 3DS servers do not always populate it; the cryptogram
        // itself is the load-bearing artefact and is required below.
        | None => {}
        Some(ref other) => Err(IntegrationError::NotSupported {
            message: format!("external 3DS pass-through with transaction status {other:?}"),
            connector: "rapyd",
            context: crate::utils::integration_ctx(
                format!("Rapyd's external-3DS pass-through expects an authenticated (Y) or attempted (A) 3DS result; the authentication reported {other:?}"),
                "Do not send the cryptogram for a failed, rejected or still-challenging authentication — re-authenticate the cardholder, or fall back to Rapyd-hosted 3DS.",
            ),
        })?,
    }

    let cavv = auth
        .cavv
        .clone()
        .ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "authentication_data.cavv",
                context: crate::utils::integration_ctx(
                    "Rapyd's external 3DS pass-through is identified by the cavv cryptogram in payment_method_options; without it the request is an ordinary non-authenticated payment",
                    "Send the CAVV returned by your 3DS server, or omit authentication_data entirely and let Rapyd run the challenge.",
                ),
            })
        })?;

    let three_ds_version = auth
        .message_version
        .as_ref()
        .map(RapydThreeDsVersion::from_semantic_version)
        .transpose()?;
    let eci = auth
        .eci
        .as_deref()
        .map(RapydEciRequest::from_authentication_eci)
        .transpose()?;

    // Rapyd: "ds_trans_id — Directory Server Transaction ID. Required for
    // Mastercard 2.0". Fail here rather than send a body Rapyd will decline.
    let is_mastercard = matches!(card_network, Some(common_enums::CardNetwork::Mastercard));
    if is_mastercard
        && three_ds_version.is_some_and(RapydThreeDsVersion::is_two_x)
        && auth.ds_trans_id.is_none()
    {
        Err(IntegrationError::MissingRequiredField {
            field_name: "authentication_data.ds_trans_id",
            context: crate::utils::integration_ctx(
                "Rapyd requires payment_method_options.ds_trans_id for Mastercard 3DS 2.x external authentication",
                "Send the Directory Server Transaction ID from the 3DS authentication response.",
            ),
        })?;
    }

    Ok(PaymentMethodOptions {
        // Mode B suppresses Rapyd's own challenge; the cardholder is already
        // authenticated.
        three_ds: false,
        three_ds_version,
        cavv: Some(cavv),
        eci,
        ds_trans_id: auth.ds_trans_id.clone(),
        sca_exemption: auth
            .exemption_indicator
            .as_ref()
            .and_then(RapydScaExemption::from_exemption_indicator),
    })
}

/// Rapyd `client_details` — the browser/device context an ACS risk-assesses a
/// 3DS challenge with. Sourced entirely from `BrowserInformation`; every field
/// is optional and omitted when absent, and the whole object is omitted when no
/// browser information reached us. Deliberately NOT hard-required: the connector
/// shipped without it, and hard-failing would regress live merchants.
///
/// In production, Visa and Mastercard 3DS are expected to decline without
/// `ip_address`, and Visa additionally without `screen_height` / `screen_width`.
#[derive(Debug, Serialize)]
pub struct RapydClientDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_address: Option<Secret<String, common_utils::pii::IpAddress>>,
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
    /// Minutes from UTC, passed through from `BrowserInformation::time_zone`
    /// unchanged. Rapyd documents "UTC offset in minutes" without stating
    /// whether it follows the JavaScript `getTimezoneOffset()` sign convention,
    /// so negating it speculatively would be a guess; it is a risk-scoring
    /// input, not a correctness gate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_zone_offset: Option<i32>,
}

/// Rapyd's documented `screen_color_depth` values. A browser reporting anything
/// else has the field omitted rather than sent and rejected.
const RAPYD_SCREEN_COLOR_DEPTHS: [u8; 8] = [1, 4, 8, 15, 16, 24, 32, 48];

impl RapydClientDetails {
    /// `None` when the browser payload carries nothing Rapyd can use, so the
    /// whole `client_details` object is dropped rather than serialised as `{}`.
    pub(super) fn from_browser_info(
        browser_info: &domain_types::router_request_types::BrowserInformation,
    ) -> Option<Self> {
        let details = Self {
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
            time_zone_offset: browser_info.time_zone,
        };
        details.is_populated().then_some(details)
    }

    fn is_populated(&self) -> bool {
        self.ip_address.is_some()
            || self.accept_header.is_some()
            || self.screen_height.is_some()
            || self.screen_width.is_some()
            || self.screen_color_depth.is_some()
            || self.java_enabled.is_some()
            || self.java_script_enabled.is_some()
            || self.language.is_some()
            || self.time_zone_offset.is_some()
    }
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

        // Capture intent applies to every payment method; `payment_method_options`
        // is card-only (wallets carry their own authentication).
        let capture = Some(item.router_data.request.is_auto_capture());
        let is_card = matches!(
            item.router_data.resource_common_data.payment_method,
            common_enums::PaymentMethod::Card
        );
        // The 3DS decision comes from the request and nowhere else: external
        // authentication data if the merchant supplied it, otherwise the
        // caller's `auth_type`.
        let external_three_ds = item.router_data.request.authentication_data.as_ref();
        let wants_rapyd_hosted_three_ds = matches!(
            item.router_data.resource_common_data.auth_type,
            common_enums::AuthenticationType::ThreeDs
        );
        let card_network = match item.router_data.request.payment_method_data {
            PaymentMethodData::Card(ref ccard) => ccard.card_network.as_ref(),
            _ => None,
        };
        let payment_method_options = is_card
            .then(|| {
                build_card_payment_method_options(
                    external_three_ds,
                    wants_rapyd_hosted_three_ds,
                    card_network,
                )
            })
            .transpose()?;
        // Browser data exists so an ACS can risk-assess a challenge. There is no
        // challenge on the external pass-through path, so it is omitted there.
        let client_details = if is_card && external_three_ds.is_none() {
            item.router_data
                .request
                .browser_info
                .as_ref()
                .and_then(RapydClientDetails::from_browser_info)
        } else {
            None
        };
        let payment_method = match item.router_data.request.payment_method_data {
            PaymentMethodData::Card(ref ccard) => {
                Some(RapydPaymentMethodData::PaymentMethod(Box::new(
                    PaymentMethod {
                        // Placeholder India type. Rapyd's valid `payment_method.type`
                        // is country- and merchant-specific (this sandbox enables
                        // `in_amex_card`, not plain `in_visa_card`), so the correct
                        // per-network/funding mapping is deferred; sandbox is lenient
                        // about the card BIN.
                        //
                        // KNOWN LIMITATION, and it gates external 3DS: which
                        // `payment_method_options` a type accepts is decided
                        // per-type by `GET /v1/payment_methods/{type}/required_fields`.
                        // `in_amex_card` advertises none of the external-3DS
                        // options, so a Mode B request against it is rejected with
                        // `UNKNOWN_PAYMENT_METHOD_FIELD - [CAVV]` /
                        // `- [3D_VERSION]` (reproduced against the sandbox).
                        // Rapyd-hosted 3DS (Mode A) is unaffected and works on this
                        // type. Deriving the type — from card network + billing
                        // country, from merchant metadata, or from a runtime
                        // `GET /v1/payment_methods/country` lookup — is tracked as
                        // an open question in the 3DS technical specification and is
                        // deliberately out of scope here rather than silently
                        // guessed.
                        pm_type: RapydPaymentMethodType::InAmexCard,
                        fields: Some(PaymentFields {
                            number: ccard.card_number.to_owned(),
                            expiration_month: ccard.card_exp_month.to_owned(),
                            expiration_year: ccard.card_exp_year.to_owned(),
                            name: item
                                .router_data
                                .resource_common_data
                                .get_optional_billing_full_name()
                                .to_owned()
                                .unwrap_or(Secret::new("".to_string())),
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
        // When the merchant requests future off-session use, ask Rapyd to save
        // the card and create an inline customer, so the response carries the
        // reusable `card_*` / `cus_*` tokens (the mandate) for later MIT calls.
        let (customer, save_payment_method) =
            if item.router_data.request.setup_future_usage.is_some() {
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
            client_details,
            receipt_email: item.router_data.request.email.clone(),
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
    /// Any status Rapyd adds after this integration was written. Present so a
    /// new upstream value parses instead of failing the whole response body.
    #[serde(other)]
    Unknown,
}

pub(super) fn get_status(
    status: RapydPaymentStatus,
    next_action: NextAction,
) -> common_enums::AttemptStatus {
    match (status, next_action) {
        (RapydPaymentStatus::Closed, _) => common_enums::AttemptStatus::Charged,
        (
            RapydPaymentStatus::Active,
            NextAction::ThreedsVerification | NextAction::PendingConfirmation,
        ) => common_enums::AttemptStatus::AuthenticationPending,
        // Authorised, capture outstanding. `pending_offline_capture` has the same
        // semantics as `pending_capture` — it waits on us, not on the connector,
        // so it must not be `Pending`.
        (
            RapydPaymentStatus::Active,
            NextAction::PendingCapture
            | NextAction::NotApplicable
            | NextAction::PendingOfflineCapture,
        ) => common_enums::AttemptStatus::Authorized,
        // An unrecognised next-action on a live payment is not a licence to
        // guess "authorized" — leave it non-terminal for PSync to resolve.
        (RapydPaymentStatus::Active, NextAction::Unknown) => common_enums::AttemptStatus::Pending,
        (
            RapydPaymentStatus::CanceledByClientOrBank
            | RapydPaymentStatus::Expired
            | RapydPaymentStatus::ReversedByRapyd,
            _,
        ) => common_enums::AttemptStatus::Voided,
        (RapydPaymentStatus::Error, _) => common_enums::AttemptStatus::Failure,
        (RapydPaymentStatus::New, _) => common_enums::AttemptStatus::Authorizing,
        // An unrecognised upstream status is not terminal — leave the attempt
        // pending so PSync resolves it rather than guessing an outcome.
        (RapydPaymentStatus::Unknown, _) => common_enums::AttemptStatus::Pending,
    }
}

/// Status for a **payment API** response (Authorize / PSync / Capture / Void /
/// SetupMandate / RepeatPayment).
///
/// `next_action` is `Option` only because Rapyd's reduced `PAYMENT_FAILED`
/// webhook payload omits it. On an API response the field is always present,
/// and on a live (`ACT`) payment it is the half of the status pair that decides
/// between "authorised, awaiting capture" and "3DS challenge outstanding" —
/// defaulting it there would silently report a 3DS-pending attempt as
/// `Authorized` and strand the challenge. So a missing `next_action` on an
/// `ACT` payment is an error.
///
/// Terminal statuses (`CLO`, `ERR`, `CAN`, `EXP`, `REV`, `NEW`) do not consult
/// `next_action` at all and keep the permissive default, so this rule can never
/// fail a payment Rapyd already closed. The webhook path keeps the permissive
/// default throughout — see `get_status_for_webhook`.
pub(super) fn get_status_for_payment_response(
    data: &ResponseData,
    http_code: u16,
) -> Result<common_enums::AttemptStatus, error_stack::Report<ConnectorError>> {
    let next_action = match data.next_action.to_owned() {
        Some(next_action) => next_action,
        None if matches!(data.status, RapydPaymentStatus::Active) => {
            return Err(error_stack::report!(
                crate::utils::unexpected_response_fail(
                    http_code,
                    format!(
                    "rapyd returned status=ACT for payment {} with no next_action; whether the \
                     payment is awaiting a 3DS challenge or awaiting capture is undeterminable",
                    data.id
                ),
                )
            ))
        }
        None => NextAction::NotApplicable,
    };
    Ok(get_status(data.status.to_owned(), next_action))
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

// ---------------------------------------------------------------------------
// Response-side error classification, AVS/CVV checks, and the GSM triple
//
// Rapyd ships THREE distinct failure envelopes and every one of them has to
// land on the same `ErrorResponse`:
//
//   1. status-only  — `{ "status": { error_code, message, ... } }`, no `data`.
//      This is what the sandbox decline cards return
//      (<https://docs.rapyd.net/en/card-numbers-for-testing.html>). The card
//      network code exists ONLY inside the bracket group of `error_code`.
//   2. status + data — a Create Payment error carries a full payment object
//      alongside the envelope, with `failure_code`, `failure_message`,
//      `merchant_advice_code`
//      (<https://docs.rapyd.net/en/create-payment-error-examples.html>).
//   3. webhook — no `status` envelope at all; the qualified code lives at
//      `data.error_code`
//      (<https://docs.rapyd.net/en/payment-failed-webhook.html>).
// ---------------------------------------------------------------------------

/// Rapyd's card-network error prefix. The scheme's own decline code rides in a
/// bracket group after a literal `" - "`, e.g.
/// `"ERROR_PROCESSING_CARD - [51]"`.
/// <https://docs.rapyd.net/en/card-network-errors.html>
const ERROR_PROCESSING_CARD_PREFIX: &str = "ERROR_PROCESSING_CARD";

/// Class (c) — the request never became a payment attempt: bad/absent auth
/// headers, a signature Rapyd could not verify, a replayed idempotency key, a
/// Rapyd-side fault, or a rate limit.
///
/// None of these is a card decline and none of them proves the attempt did not
/// go through, so they must never terminally fail a payment and never populate
/// the GSM fields. `IDEMPOTENCY_ERROR` and `GENERAL_ERROR` in particular mean
/// "outcome unknown, go and PSync".
///
/// <https://docs.rapyd.net/en/rapyd-error-codes.html>,
/// <https://docs.rapyd.net/en/general-errors.html>
const RAPYD_TRANSPORT_ERROR_CODES: [&str; 5] = [
    "MISSING_AUTHENTICATION_HEADERS",
    "UNAUTHENTICATED_API_CALL",
    "IDEMPOTENCY_ERROR",
    "GENERAL_ERROR",
    "ERROR_REPORTS_RATE_LIMIT_EXCEEDED",
];

/// Class (a) — the card network, the issuer or the customer refused. These are
/// genuine declines: they feed GSM and a retry is governed by the Merchant
/// Advice Code.
///
/// Everything Rapyd publishes that is NOT in this list and not in
/// [`RAPYD_TRANSPORT_ERROR_CODES`] is a class (b) merchant-configuration or
/// malformed-request rejection — by far the largest family (95 of the 107 codes
/// on `payment-errors.html`) — which is why class (b) is the residual here
/// rather than a second exhaustive list that would rot on Rapyd's next release.
///
/// <https://docs.rapyd.net/en/payment-errors.html>,
/// <https://docs.rapyd.net/en/card-transaction-errors.html>,
/// <https://docs.rapyd.net/en/general-errors.html>
const RAPYD_ISSUER_DECLINE_ERROR_CODES: [&str; 20] = [
    "ERROR_3DS_AUTHENTICATION_FAILURE",
    "ERROR_AUTHENTICATION_PHONE_UNAVAILABLE",
    "ERROR_CARD_AUTHENTICATION_FAILURE",
    "ERROR_CARD_CVV_NOT_VALID",
    "ERROR_CARD_EXPIRED",
    "ERROR_CARD_INFORMATION_NOT_VALID",
    "ERROR_CARD_NOT_AUTHENTICATED",
    "ERROR_CARD_NOT_SUPPORTED_FOR_ECOMMERCE",
    "ERROR_CREATE_PAYMENT_ADDRESS_VERIFICATION_FAILURE",
    "ERROR_CREATE_PAYMENT_CUSTOMER_CANCEL",
    "ERROR_CREATE_PAYMENT_INSUFFICIENT_FUNDS",
    "ERROR_CREATE_PAYMENT_ODFI_CANCEL",
    "ERROR_CREATE_PAYMENT_PAD_PROBLEM",
    "ERROR_CREATE_PAYMENT_SOURCE_UNAVAILABLE",
    "ERROR_PAYER_UNKNOWN",
    "ERROR_PAYMENT_METHOD_EXPIRED",
    "ERROR_SCA_EXEMPTION_DECLINED",
    "ERROR_TRANSACTION_FAILED",
    "ERROR_TRANSACTION_REJECTED_BY_CARD_PROCESSOR",
    "ERROR_TRANSACTION_TYPE_NOT_SUPPORTED",
];

/// What kind of failure Rapyd is reporting. Drives BOTH whether the GSM fields
/// are populated and whether the attempt may be marked terminally failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RapydErrorClass {
    /// (a) The card network / issuer / customer refused. Feeds GSM.
    IssuerDecline,
    /// (b) Rapyd rejected the request before or independently of the network —
    /// entitlement, capability, malformed field, wrong lifecycle state. The
    /// operation definitively did not happen, but it is **not** a card decline:
    /// the GSM fields stay `None` so no smart-retry rule can key on it.
    MerchantConfiguration,
    /// (c) Transport / auth / rate limit / Rapyd-side fault. Outcome unknown.
    Transport,
}

/// Classifies a Rapyd failure. Ordered cheapest-first; every branch is
/// derivable from the parsed body alone.
pub(super) fn classify_rapyd_error(
    http_status_code: u16,
    status: &Status,
    data: Option<&ResponseData>,
) -> RapydErrorClass {
    let code = status.error_code.trim();

    // 1. The card network answered and Rapyd forwarded its code verbatim.
    if code.starts_with(ERROR_PROCESSING_CARD_PREFIX) {
        return RapydErrorClass::IssuerDecline;
    }

    // 2. Explicit transport codes.
    if RAPYD_TRANSPORT_ERROR_CODES.contains(&code) {
        return RapydErrorClass::Transport;
    }

    // 3. Rapyd does not document which HTTP status accompanies which error code,
    //    so the status is never used to *prove* a decline. It is used only to
    //    WIDEN class (c): an auth/conflict/timeout/rate-limit status is never
    //    evidence that a card was presented, so treating it as transport can
    //    only ever prevent a false terminal failure, never cause one.
    if matches!(http_status_code, 401 | 403 | 408 | 409 | 429) {
        return RapydErrorClass::Transport;
    }

    // 4. Named issuer/customer declines that carry no bracketed network code.
    if RAPYD_ISSUER_DECLINE_ERROR_CODES.contains(&code) {
        return RapydErrorClass::IssuerDecline;
    }

    // 5. The emptiness discriminator. When a Create Payment error reached the
    //    card network every `data.*` error field populates; when Rapyd rejected
    //    it on merchant configuration they are all `""`/null. Proved by the two
    //    examples on
    //    <https://docs.rapyd.net/en/create-payment-error-examples.html>.
    if data
        .and_then(|payment| non_empty(payment.failure_code.clone()))
        .is_some()
    {
        return RapydErrorClass::IssuerDecline;
    }

    RapydErrorClass::MerchantConfiguration
}

/// The three fields Hyperswitch's Global Status Mapping keys smart-retry rules
/// on. Populated only for [`RapydErrorClass::IssuerDecline`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RapydNetworkErrorFields {
    /// `ErrorResponse::network_decline_code` — the BARE scheme code (`"51"`).
    pub decline_code: Option<String>,
    /// `ErrorResponse::network_advice_code` — the Merchant Advice Code
    /// (`"01"`..`"18"`), the only thing Rapyd returns that says whether and
    /// when to retry.
    pub advice_code: Option<String>,
    /// `ErrorResponse::network_error_message` — the network's own short
    /// decline reason (`"Insufficient Funds"`).
    pub error_message: Option<String>,
}

/// Extracts the bracketed card-network code out of a qualified
/// `"ERROR_PROCESSING_CARD - [65]"` style code.
///
/// The token is **alphanumeric, not numeric** — Rapyd's own tables list `OF`,
/// `TJ`, `5C`, `N7`, `W1`, `XA`, `3X`, `1A`, `6P`, `B1`, `Q1`, `Z1` and `CV`
/// among others — so it is never parsed as a number.
/// <https://docs.rapyd.net/en/card-network-errors.html>
pub(super) fn network_code_from_qualified(code: &str) -> Option<String> {
    let start = code.find('[')?;
    let rest = code.get(start + 1..)?;
    let end = rest.find(']')?;
    let token = rest.get(..end)?.trim();
    (!token.is_empty()).then(|| token.to_owned())
}

/// Normalises Rapyd's three observed `message` / `failure_message` shapes down
/// to the card network's short reason.
///
/// Rapyd's prose says a webhook's `failure_message` is bracketed
/// (`"[Insufficient Funds]"`) while both published payloads show it bare, and a
/// REST `status.message` is `"[short] long"`. Taking the bracket group when one
/// is present, and the whole string otherwise, covers all three without relying
/// on which of Rapyd's two accounts is right.
/// <https://docs.rapyd.net/en/card-network-errors.html>
pub(super) fn rapyd_short_message(message: &str) -> &str {
    let trimmed = message.trim();
    if let Some(rest) = trimmed.strip_prefix('[') {
        if let Some(end) = rest.find(']') {
            if let Some(short) = rest.get(..end).map(str::trim) {
                if !short.is_empty() {
                    return short;
                }
            }
        }
    }
    trimmed
}

/// Builds the GSM triple from whichever failure envelope is at hand.
///
/// Returns all-`None` for class (b) and class (c): neither is a card decline,
/// and a GSM rule that fired on a merchant-configuration rejection would
/// schedule retries of a request that is guaranteed to fail forever.
pub(super) fn rapyd_network_error_fields(
    class: RapydErrorClass,
    status: &Status,
    data: Option<&ResponseData>,
) -> RapydNetworkErrorFields {
    if class != RapydErrorClass::IssuerDecline {
        return RapydNetworkErrorFields::default();
    }

    // Preferred: the bare code Rapyd already split out for us. Fallbacks: the
    // bracket group of the qualified code on `data`, then on the envelope —
    // which is all the status-only shape has. All three provably agree.
    let decline_code = data
        .and_then(|payment| non_empty(payment.failure_code.clone()))
        .or_else(|| {
            data.and_then(|payment| payment.error_code.as_deref())
                .and_then(network_code_from_qualified)
        })
        .or_else(|| network_code_from_qualified(&status.error_code));

    // Nothing else Rapyd returns says whether or when to retry, so there is no
    // fallback for this one.
    let advice_code = data.and_then(|payment| non_empty(payment.merchant_advice_code.clone()));

    let error_message = data
        .and_then(|payment| non_empty(payment.failure_message.clone()))
        .or_else(|| non_empty(status.message.clone()))
        .map(|message| rapyd_short_message(&message).to_owned());

    RapydNetworkErrorFields {
        decline_code,
        advice_code,
        error_message,
    }
}

/// The single place a Rapyd `ErrorResponse` is built. Every flow funnels
/// through here so the eight construction sites cannot drift apart.
///
/// `attempt_status` is a **caller** decision, deliberately: this function is
/// flow-agnostic, and the right status is not.
/// `ConnectorCommon::build_error_response` — and therefore every macro-generated
/// `get_error_response_v2` — has no idea whether it is serving a payment, a
/// refund, a capture or a sync, so hardcoding one here would report an HTTP
/// error on a Refund as a terminal `RefundFailure` (`ForeignFrom<FlowStatus> for
/// RefundStatus` maps `Payment(_)` straight to `RefundFailure`) and would report
/// a rejected Capture as a failed *payment*. See the per-flow
/// `get_error_response_v2` overrides in `rapyd.rs`.
pub(super) fn rapyd_error_response(
    http_code: u16,
    class: RapydErrorClass,
    status: &Status,
    data: Option<&ResponseData>,
    connector_transaction_id: Option<String>,
    attempt_status: Option<FlowStatus>,
) -> ErrorResponse {
    let network = rapyd_network_error_fields(class, status, data);

    // The FULLY-QUALIFIED code belongs in `code`; the bare scheme code has its
    // own slot in `network_decline_code`. A webhook has no envelope, so
    // `data.error_code` is the fallback (and `Status::from(ResponseData)`
    // synthesises `NO_ERROR_CODE`, which must not win).
    let code = non_empty(Some(status.error_code.clone()))
        .filter(|value| value != NO_ERROR_CODE)
        .or_else(|| data.and_then(|payment| non_empty(payment.error_code.clone())))
        .or_else(|| data.and_then(|payment| non_empty(payment.failure_code.clone())))
        .unwrap_or_else(|| NO_ERROR_CODE.to_owned());

    // The merchant-facing headline. Rapyd's envelope `status` field is the
    // literal string "ERROR", which is what this used to surface.
    let message = data
        .and_then(|payment| non_empty(payment.failure_message.clone()))
        .or_else(|| non_empty(status.message.clone()))
        .map(|value| rapyd_short_message(&value).to_owned())
        .unwrap_or_else(|| NO_ERROR_MESSAGE.to_owned());

    // `reason` carries Rapyd's own remediation prose: the long form of
    // `status.message`, plus the Merchant Advice Code description. The MAC
    // message is retry ADVICE ("Try again later"), never a decline reason, so it
    // is deliberately kept out of `network_error_message` — routing it there
    // would make every decline report its reason as "Try again later" and
    // destroy the GSM signal.
    let long_message = non_empty(status.message.clone())
        .or_else(|| data.and_then(|payment| non_empty(payment.failure_message.clone())));
    let advice_message =
        data.and_then(|payment| non_empty(payment.merchant_advice_message.clone()));
    let reason = match (long_message, advice_message) {
        (Some(long), Some(advice)) => Some(format!("{long} (Rapyd merchant advice: {advice})")),
        (Some(long), None) => Some(long),
        (None, Some(advice)) => Some(format!("Rapyd merchant advice: {advice}")),
        (None, None) => None,
    };

    ErrorResponse {
        code,
        message,
        reason,
        status_code: http_code,
        attempt_status,
        connector_transaction_id,
        network_decline_code: network.decline_code,
        network_advice_code: network.advice_code,
        network_error_message: network.error_message,
        ..Default::default()
    }
}

/// Result of one of Rapyd's response-side verification checks.
///
/// Closed four-value set, documented identically for `acs_check` and
/// `cvv_check` on every payment page. `#[serde(other)]` for the same reason
/// `NextAction` carries one: these ride on `payment_method_data`, which backs
/// Authorize, PSync, Capture, Void, SetupMandate, RepeatPayment and every
/// payment webhook, so one unparsable value would fail all of them at once.
///
/// **Diagnostic only.** A `Fail` here does not by itself mean the payment
/// failed — the authoritative outcome remains `(status, next_action)`, and the
/// 3DS authority for this connector remains `authentication_result`.
/// <https://docs.rapyd.net/en/retrieve-payment.html>
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

/// `$.data.outcome` — Rapyd's risk-assessment result.
///
/// Documented on `retrieve-payment.html` but `null` in every published example
/// on every Rapyd page, so it is parsed and surfaced for diagnostics and is
/// deliberately **not** an input to the error classification, which uses the
/// empty-`data.*` discriminator instead.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RapydOutcome {
    pub network_status: Option<RapydNetworkStatus>,
    pub risk_level: Option<String>,
    pub seller_message: Option<String>,
    #[serde(rename = "type")]
    pub outcome_type: Option<String>,
    pub reason: Option<String>,
}

/// `$.data.outcome.network_status`. Documented closed set, plus the usual
/// catch-all so a new value cannot fail the whole payment body.
/// <https://docs.rapyd.net/en/retrieve-payment.html>
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

/// Rapyd's `next_action`, the second half of the status pair.
///
/// `next_action` lives on `ResponseData`, which backs Authorize, PSync, Capture,
/// Void, SetupMandate, RepeatPayment **and** every incoming payment webhook — so
/// a value this enum cannot parse fails all of them at once. Hence both the full
/// documented set and a `#[serde(other)]` catch-all.
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
    /// Authorised, awaiting an offline capture. Documented by Rapyd but absent
    /// from this enum until now, which turned a valid 200 into a parse failure.
    #[serde(rename = "pending_offline_capture")]
    PendingOfflineCapture,
    /// Any `next_action` Rapyd adds after this integration was written. Present
    /// so a new upstream value parses instead of failing the whole response
    /// body; it is mapped explicitly in `get_status`, never folded into
    /// `NotApplicable` (which would read as `Authorized`).
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResponseData {
    pub id: String,
    pub amount: FloatMajorUnit,
    pub status: RapydPaymentStatus,
    /// Absent on the reduced `PAYMENT_FAILED` webhook payload, hence optional.
    /// Treated as `NextAction::NotApplicable` when missing.
    pub next_action: Option<NextAction>,
    pub redirect_url: Option<String>,
    pub original_amount: Option<FloatMajorUnit>,
    pub is_partial: Option<bool>,
    pub currency_code: Option<common_enums::Currency>,
    pub country_code: Option<String>,
    pub captured: Option<bool>,
    /// Absent on the reduced `PAYMENT_FAILED` webhook payload, hence optional.
    pub transaction_id: Option<String>,
    pub merchant_reference_id: Option<String>,
    pub paid: Option<bool>,
    pub failure_code: Option<String>,
    pub failure_message: Option<String>,
    /// `$.data.error_code` — the FULLY-QUALIFIED code, e.g.
    /// `"ERROR_PROCESSING_CARD - [65]"`; `""` when there is no failure.
    ///
    /// This is the ONLY place the qualified code appears on a webhook — a
    /// webhook payload has no `status` envelope at all — so without it a
    /// `PAYMENT_FAILED` webhook could surface nothing but the bare `"65"`.
    /// <https://docs.rapyd.net/en/payment-failed-webhook.html>
    pub error_code: Option<String>,
    /// `$.data.merchant_advice_code` — the Merchant Advice Code, `"01"`..`"18"`,
    /// a zero-padded 2-character STRING (`"02"`, never `2`). Feeds
    /// `ErrorResponse::network_advice_code`; it is the only thing Rapyd returns
    /// that says whether and when a declined transaction may be retried.
    ///
    /// **A non-null value is NOT evidence of a decline**: codes `15` and `16`
    /// are documented to ride on *successful* payments to flag a non-reloadable
    /// prepaid or single-use virtual card.
    /// <https://docs.rapyd.net/en/merchant-advice-codes.html>
    pub merchant_advice_code: Option<String>,
    /// `$.data.merchant_advice_message` — free text describing the MAC, e.g.
    /// `"Try again later"`. Retry ADVICE, not a decline reason: it must never
    /// be routed into `network_error_message`.
    pub merchant_advice_message: Option<String>,
    /// `$.data.outcome` — risk-assessment result. Documented by Rapyd but
    /// `null` in every published example, so it is surfaced for diagnostics and
    /// is not an input to any decision. Boxed for the same reason as
    /// `authentication_result`.
    pub outcome: Option<Box<RapydOutcome>>,
    /// Saved-card token (`card_*`) — populated when the payment was
    /// created with `save_payment_method: true`. Used as the MIT token
    /// on subsequent charges.
    pub payment_method: Option<String>,
    /// Nested payment-method data; carries `network_reference_id`, the
    /// network transaction id surfaced as `network_txn_id` for recurring.
    pub payment_method_data: Option<RapydResponsePaymentMethodData>,
    /// 3DS outcome as reported by the issuer/ACS. **Diagnostic only** — it is
    /// deliberately not an input to `get_status`; see the note on
    /// `build_connector_metadata`.
    ///
    /// Boxed because it is absent on every non-3DS payment, and `ResponseData`
    /// is cloned along the whole payment path (and is the largest variant of
    /// `WebhookData`).
    pub authentication_result: Option<Box<RapydAuthenticationResult>>,
    /// Echoed back on the external-3DS pass-through path — the signal that Rapyd
    /// accepted the supplied cryptogram instead of running its own challenge.
    /// Boxed for the same reason as `authentication_result`.
    pub payment_method_options: Option<Box<RapydResponsePaymentMethodOptions>>,
}

/// Rapyd's `authentication_result` object. Every field is optional: `eci` and
/// `cardholder_info` come back `null` while a challenge is outstanding, and the
/// whole object is absent on a non-3DS payment.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RapydAuthenticationResult {
    pub result: Option<RapydAuthResult>,
    /// 3DS protocol version as Rapyd reports it (e.g. `"2.2.0"`). Left a string:
    /// this is the open response side, not the closed request set that
    /// `RapydThreeDsVersion` models.
    pub version: Option<String>,
    /// Response-side ECI. Rapyd documents `05/02`, `06/01`, `07/00` here, which
    /// is a different set from the values it validates on the request, so this
    /// side stays permissive.
    pub eci: Option<String>,
    /// Free-text issuer message (max 128 chars).
    pub cardholder_info: Option<String>,
}

/// `authentication_result.result`. Closed set per Rapyd, plus a catch-all so an
/// unrecognised code parses to `Unknown` rather than being guessed into `A`/`N`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum RapydAuthResult {
    /// Authenticated.
    #[serde(rename = "A")]
    Authenticated,
    /// Not authenticated. Note this does NOT by itself mean the payment failed —
    /// see `build_connector_metadata`.
    #[serde(rename = "N")]
    NotAuthenticated,
    /// Redirection pending — the challenge is still outstanding.
    #[serde(rename = "R")]
    RedirectionPending,
    /// Unsupported.
    #[serde(rename = "U")]
    Unsupported,
    #[serde(other)]
    Unknown,
}

/// The `payment_method_options` Rapyd echoes back on the external-3DS path.
///
/// `cavv` is deliberately **not** deserialized: it is a cardholder
/// authentication cryptogram, and capturing the echo would only carry it into
/// connector metadata and logs for no operational benefit. Presence of the other
/// echoed fields is discriminator enough.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RapydResponsePaymentMethodOptions {
    /// Rapyd's own docs disagree on whether this is a JSON boolean or a string.
    /// We always *send* a boolean (that is what Rapyd's machine-readable schema
    /// says and what every documented example uses), but we accept either on the
    /// way back so a string echo cannot fail the whole response body.
    #[serde(
        rename = "3d_required",
        default,
        deserialize_with = "deserialize_optional_bool_or_string"
    )]
    pub three_ds: Option<bool>,
    #[serde(rename = "3d_version")]
    pub three_ds_version: Option<String>,
    pub eci: Option<String>,
    pub ds_trans_id: Option<String>,
}

/// Accept `true` / `false` as either a JSON boolean or a JSON string.
/// An unrecognised string yields `None` rather than an error — this field is
/// informational, and failing on it would fail the whole payment response.
fn deserialize_optional_bool_or_string<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum BoolOrString {
        Bool(bool),
        Str(String),
    }

    Ok(match Option::<BoolOrString>::deserialize(deserializer)? {
        None => None,
        Some(BoolOrString::Bool(value)) => Some(value),
        Some(BoolOrString::Str(value)) => match value.trim().to_ascii_lowercase().as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
    })
}

/// Build the redirect form from the payment object.
///
/// Rapyd puts `redirect_url` at the **top level** of the payment object and can
/// return it on any card Authorize — including one that asked for no 3DS at all
/// (PSD2/SCA and plain issuer discretion), and one that supplied external
/// authentication data. It is therefore read unconditionally and never gated on
/// what the request asked for.
///
/// A `3d_verification` next-action with no usable `redirect_url` is a hard
/// failure rather than a pending attempt: the cardholder would have nowhere to
/// authenticate, so the attempt would sit in `AuthenticationPending` and poll
/// until Rapyd's 15-minute window expired.
pub(super) fn build_redirection_data(
    data: &ResponseData,
    http_code: u16,
) -> Result<Option<RedirectForm>, error_stack::Report<ConnectorError>> {
    let redirect_url = data
        .redirect_url
        .as_ref()
        .filter(|url| !url.trim().is_empty());

    if redirect_url.is_none() && data.next_action.as_ref() == Some(&NextAction::ThreedsVerification)
    {
        return Err(error_stack::report!(crate::utils::unexpected_response_fail(
            http_code,
            format!(
                "rapyd returned next_action=3d_verification for payment {} without a redirect_url; the cardholder has no way to complete the 3DS challenge",
                data.id
            ),
        )));
    }

    let parsed = redirect_url
        .map(|url| {
            Url::parse(url).change_context(crate::utils::response_handling_fail_for_connector(
                http_code, "rapyd",
            ))
        })
        .transpose()?;

    Ok(parsed.map(|url| RedirectForm::from((url, Method::Get))))
}

/// Rapyd's response-side verification checks, surfaced together under
/// `connector_metadata.verification_checks`.
///
/// These are the merchant's only view of the AVS and CVV outcomes: Rapyd
/// reports them per attempt and nothing else in `PaymentsResponseData` has a
/// slot for them.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RapydVerificationChecks {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acs_check: Option<RapydCheckResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cvv_check: Option<RapydCheckResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avs_check: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avs_result: Option<String>,
}

impl RapydVerificationChecks {
    fn from_payment_method_data(pmd: &RapydResponsePaymentMethodData) -> Option<Self> {
        let checks = Self {
            acs_check: pmd.acs_check,
            cvv_check: pmd.cvv_check,
            avs_check: pmd.avs_check.clone(),
            avs_result: pmd.avs_result.clone(),
        };
        // Every one of these is absent on a non-card payment method, and
        // `payment_method_data` is the same struct across all of them.
        (!checks.is_empty()).then_some(checks)
    }

    fn is_empty(&self) -> bool {
        self.acs_check.is_none()
            && self.cvv_check.is_none()
            && self.avs_check.is_none()
            && self.avs_result.is_none()
    }
}

/// Surface the per-attempt diagnostics Rapyd returns — the 3DS result
/// (`authentication_result` and the external-3DS echo) plus the AVS/CVV/ACS
/// verification checks — as `connector_metadata`.
///
/// All of it is **diagnostic only and never an input to `get_status`**. An
/// issuer can return `result: "N"` under an attempts/liability-shift or an SCA
/// exemption while Rapyd still closes the payment with `paid: true`, and a
/// `cvv_check: "fail"` can likewise accompany an approved payment — mapping
/// either to `Failure` would report a false decline on a captured payment. The
/// authoritative outcome stays `(status, next_action)` plus `failure_code`.
pub(super) fn build_connector_metadata(data: &ResponseData) -> Option<serde_json::Value> {
    #[derive(Serialize)]
    struct RapydConnectorMetadata<'a> {
        #[serde(skip_serializing_if = "Option::is_none")]
        authentication_result: Option<&'a RapydAuthenticationResult>,
        #[serde(skip_serializing_if = "Option::is_none")]
        payment_method_options: Option<&'a RapydResponsePaymentMethodOptions>,
        #[serde(skip_serializing_if = "Option::is_none")]
        verification_checks: Option<RapydVerificationChecks>,
    }

    let verification_checks = data
        .payment_method_data
        .as_ref()
        .and_then(RapydVerificationChecks::from_payment_method_data);

    if data.authentication_result.is_none()
        && data.payment_method_options.is_none()
        && verification_checks.is_none()
    {
        return None;
    }

    serde_json::to_value(RapydConnectorMetadata {
        authentication_result: data.authentication_result.as_deref(),
        payment_method_options: data.payment_method_options.as_deref(),
        verification_checks,
    })
    .ok()
}

/// Subset of Rapyd's response `payment_method_data` object.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RapydResponsePaymentMethodData {
    pub network_reference_id: Option<Secret<String>>,
    /// Access Control Server (3DS) check. Closed set, diagnostic only —
    /// `authentication_result` stays the 3DS authority for this connector and
    /// this is a coarser second view of the same event.
    /// <https://docs.rapyd.net/en/retrieve-payment.html>
    pub acs_check: Option<RapydCheckResult>,
    /// Verification of the card's CVV/CVC against the issuer's record. Closed
    /// set. Diagnostic only: a `Fail` does not by itself mean the payment
    /// failed — `(status, next_action)` remains authoritative. Rapyd's own
    /// `PAYMENT_FAILED` example carries `"acs_check": "unavailable"` and
    /// `"cvv_check": "fail"` in the same payload, so the two are independent.
    /// <https://docs.rapyd.net/en/retrieve-payment.html>
    pub cvv_check: Option<RapydCheckResult>,
    /// Address Verification Service result. **Deliberately an opaque `String`,
    /// not an enum**: unlike its two table neighbours above, Rapyd documents no
    /// value set for it on any page, and it appears in zero published JSON
    /// examples. Inventing the `pass|fail|unavailable|unchecked` set here would
    /// be fabricating a contract Rapyd has never published.
    ///
    /// AVS is off unless the request sets `payment_method_options.avs_required`
    /// **and** includes an `address` object. The actionable AVS failure signal
    /// is the top-level `ERROR_CREATE_PAYMENT_ADDRESS_VERIFICATION_FAILURE`
    /// code, not this field, so nothing gates status on it.
    /// <https://docs.rapyd.net/en/retrieve-payment.html>,
    /// <https://docs.rapyd.net/en/card-payments.html>
    pub avs_check: Option<String>,
    /// The field Rapyd's one published `avs_required: true` example actually
    /// returns (value `"X"`), which appears in NO field table on any page.
    /// Both names are modelled because Rapyd's prose and Rapyd's examples
    /// disagree about which one is emitted; both are opaque.
    /// <https://docs.rapyd.net/en/create-payment.html>
    pub avs_result: Option<String>,
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
            payment: item
                .router_data
                .request
                .connector_transaction_id
                .to_string(),
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
    /// Documented on `refund-completed-webhook.html`; terminal.
    Canceled,
    #[default]
    Pending,
    /// Any refund status Rapyd adds after this integration was written.
    #[serde(other)]
    Unknown,
}

impl From<RefundStatus> for common_enums::RefundStatus {
    fn from(item: RefundStatus) -> Self {
        match item {
            RefundStatus::Completed => Self::Success,
            RefundStatus::Error | RefundStatus::Rejected | RefundStatus::Canceled => Self::Failure,
            // An unrecognised status is not terminal — keep it pending so RSync
            // resolves it rather than guessing success or failure.
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
    /// Card-network error code, present on refund webhooks.
    pub failure_code: Option<String>,
    pub failure_reason: Option<String>,
}

impl<F, T> TryFrom<ResponseRouterData<RefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, T, RefundsResponseData>
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
            // A 2xx carrying Rapyd's status-only error envelope. This used to be
            // reported as `Ok` with the ERROR CODE stuffed into
            // `connector_refund_id`, which both fabricated a refund id and threw
            // the reason away — the merchant saw a failed refund with no code
            // and no message. It is an `ErrorResponse`.
            //
            // `FlowStatus::Refund(..)`, never `FlowStatus::Payment(..)`: the
            // refund error builder in `domain_types` reads `attempt_status`
            // alone, and `ForeignFrom<FlowStatus> for RefundStatus` maps
            // `Payment(_)` to `RefundFailure` — so a payment status here would
            // be silently laundered into a terminal refund failure. The status
            // carried is the one this arm has always produced.
            None => Err(rapyd_error_response(
                item.http_code,
                classify_rapyd_error(item.http_code, &item.response.status, None),
                &item.response.status,
                None,
                None,
                Some(FlowStatus::Refund(common_enums::RefundStatus::Failure)),
            )),
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
                response: Err(rapyd_error_response(
                    item.http_code,
                    classify_rapyd_error(item.http_code, &response.status, None),
                    &response.status,
                    None,
                    None,
                    Some(FlowStatus::Payment(common_enums::AttemptStatus::Failure)),
                )),
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
                context: Default::default(),
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
                        context: Default::default(),
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
        let payment_method_options = Some(PaymentMethodOptions::rapyd_hosted(three_ds_enabled));

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

        let return_url = router_data.resource_common_data.return_url.clone().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "return_url",
                context: Default::default(),
            },
        )?;

        Ok(Self {
            amount,
            currency: request.currency,
            payment_method,
            // Zero-auth: authorize the card so Rapyd can mint the
            // `card_*` / `cus_*` tokens, but do not capture funds.
            capture: Some(false),
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
            client_details: router_data
                .request
                .browser_info
                .as_ref()
                .and_then(RapydClientDetails::from_browser_info),
            receipt_email: request.email.clone(),
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
        let (status, response) = match &item.response.data {
            Some(data) => {
                let attempt_status = get_status_for_payment_response(data, item.http_code)?;
                match attempt_status {
                    common_enums::AttemptStatus::Failure => (
                        common_enums::AttemptStatus::Failure,
                        Err(rapyd_error_response(
                            item.http_code,
                            classify_rapyd_error(item.http_code, &item.response.status, Some(data)),
                            &item.response.status,
                            Some(data),
                            Some(data.id.clone()),
                            Some(FlowStatus::Payment(common_enums::AttemptStatus::Failure)),
                        )),
                    ),
                    _ => {
                        // Surface the 3DS redirect so verification can be completed.
                        let redirection_data = build_redirection_data(data, item.http_code)?;
                        // The saved card token is the mandate reference used on
                        // MIT replays; Rapyd charges it without a customer id.
                        let mandate_reference = data.payment_method.as_ref().map(|card| {
                            Box::new(MandateReference {
                                connector_mandate_id: Some(card.clone()),
                                payment_method_id: None,
                                connector_mandate_request_reference_id: None,
                                mandate_metadata: None,
                            })
                        });
                        // Promote Authorized → Charged so a zero-amount
                        // verification reaches a terminal state.
                        let terminal_status = match attempt_status {
                            common_enums::AttemptStatus::Authorized => {
                                common_enums::AttemptStatus::Charged
                            }
                            other => other,
                        };
                        (
                            terminal_status,
                            Ok(PaymentsResponseData::TransactionResponse {
                                resource_id: ResponseId::ConnectorTransactionId(data.id.clone()),
                                redirection_data: redirection_data.map(Box::new),
                                mandate_reference,
                                connector_metadata: build_connector_metadata(data),
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
            }
            None => (
                common_enums::AttemptStatus::Failure,
                Err(rapyd_error_response(
                    item.http_code,
                    classify_rapyd_error(item.http_code, &item.response.status, None),
                    &item.response.status,
                    None,
                    None,
                    Some(FlowStatus::Payment(common_enums::AttemptStatus::Failure)),
                )),
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
        let card_id = connector_mandate.get_connector_mandate_id().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "mandate_reference.connector_mandate_id",
                context: Default::default(),
            },
        )?;

        let three_ds_enabled = matches!(
            router_data.resource_common_data.auth_type,
            common_enums::AuthenticationType::ThreeDs
        );
        let payment_method_options = Some(PaymentMethodOptions::rapyd_hosted(three_ds_enabled));

        // On Charge, return_url arrives on `request.router_return_url`;
        // `PaymentFlowData.return_url` is hardcoded to None for this flow.
        let return_url = request
            .router_return_url
            .clone()
            .or_else(|| router_data.resource_common_data.return_url.clone())
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "return_url",
                context: Default::default(),
            })?;

        Ok(Self {
            amount,
            currency: request.currency,
            payment_method: RapydPaymentMethodData::Token(Secret::new(card_id)),
            // Honor the caller's capture intent; SequentialAutomatic and
            // unspecified default to auto-capture, matching the Authorize flow.
            capture: Some(request.is_auto_capture()),
            payment_method_options,
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
            initiation_type: Some(RapydInitiationType::Recurring),
            // MIT replay: no cardholder browser is present and Rapyd bypasses
            // 3DS on the stored credential.
            client_details: None,
            receipt_email: None,
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
        let (status, response) = match &item.response.data {
            Some(data) => {
                let attempt_status = get_status_for_payment_response(data, item.http_code)?;
                match attempt_status {
                    common_enums::AttemptStatus::Failure => (
                        common_enums::AttemptStatus::Failure,
                        Err(rapyd_error_response(
                            item.http_code,
                            classify_rapyd_error(item.http_code, &item.response.status, Some(data)),
                            &item.response.status,
                            Some(data),
                            // Preserve the connector's transaction id on
                            // failure so reconciliation / support lookups
                            // can locate the attempt in Rapyd's dashboard.
                            Some(data.id.clone()),
                            Some(FlowStatus::Payment(common_enums::AttemptStatus::Failure)),
                        )),
                    ),
                    _ => (
                        attempt_status,
                        Ok(PaymentsResponseData::TransactionResponse {
                            resource_id: ResponseId::ConnectorTransactionId(data.id.clone()),
                            redirection_data: None,
                            // MIT replay does not mint a new mandate — the
                            // `connector_customer` + `card_*` from SetupMandate
                            // stay valid. `data.id` is a one-shot payment id
                            // and must never be stored as a mandate.
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
                    ),
                }
            }
            None => (
                common_enums::AttemptStatus::Failure,
                Err(rapyd_error_response(
                    item.http_code,
                    classify_rapyd_error(item.http_code, &item.response.status, None),
                    &item.response.status,
                    None,
                    None,
                    Some(FlowStatus::Payment(common_enums::AttemptStatus::Failure)),
                )),
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
// Incoming webhooks
// ---------------------------------------------------------------------------

/// Rapyd webhook envelope.
///
/// Rapyd ships two envelope shapes: payment/dispute events (`id` is `wh_*`,
/// `status` is `NEW`/`CLO`/`ERR`/`RET`, `created_at` is a unix timestamp) and
/// refund events (`id` is a bare UUID, the `wh_*` id moves to `token`, `status`
/// is `""` and `created_at` is `0`). Both are covered by this struct — the
/// refund-only extra fields are simply not modelled.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RapydIncomingWebhook {
    pub id: String,
    #[serde(rename = "type")]
    pub webhook_type: RapydWebhookObjectEventType,
    pub data: WebhookData,
    pub trigger_operation_id: Option<String>,
    /// Delivery status of the webhook itself (`NEW` | `CLO` | `ERR` | `RET`),
    /// empty string on the refund envelope. Not the resource status.
    pub status: Option<String>,
    pub created_at: Option<i64>,
}

/// The `type` discriminator on the webhook envelope.
///
/// Only the eight events UCS acts on are modelled; Rapyd's live catalogue is
/// wider, so `#[serde(other)] Unknown` is mandatory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
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

/// Dispute lifecycle status, sent as a three-letter code on the dispute object.
///
/// The four codes below are the ones with a faithful `common_enums::DisputeStatus`
/// counterpart. `PRA` (pre-arbitration), `ARB` (arbitration) and `REV` (reversed)
/// deserialise to `Unknown` rather than being mapped to a guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, strum::Display)]
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

impl From<RapydWebhookDisputeStatus> for domain_types::connector_types::EventType {
    fn from(value: RapydWebhookDisputeStatus) -> Self {
        match value {
            RapydWebhookDisputeStatus::Active => Self::DisputeOpened,
            RapydWebhookDisputeStatus::Review => Self::DisputeChallenged,
            RapydWebhookDisputeStatus::Lose => Self::DisputeLost,
            RapydWebhookDisputeStatus::Win => Self::DisputeWon,
            RapydWebhookDisputeStatus::Unknown => Self::IncomingWebhookEventUnspecified,
        }
    }
}

impl RapydWebhookDisputeStatus {
    /// `common_enums::DisputeStatus` has no "unspecified" variant, so an
    /// unmapped Rapyd code is an error rather than a guessed status.
    pub fn to_dispute_status(
        self,
    ) -> Result<common_enums::DisputeStatus, error_stack::Report<domain_types::errors::WebhookError>>
    {
        match self {
            Self::Active => Ok(common_enums::DisputeStatus::DisputeOpened),
            Self::Review => Ok(common_enums::DisputeStatus::DisputeChallenged),
            Self::Lose => Ok(common_enums::DisputeStatus::DisputeLost),
            Self::Win => Ok(common_enums::DisputeStatus::DisputeWon),
            Self::Unknown => Err(error_stack::report!(
                domain_types::errors::WebhookError::WebhookProcessingFailed
            )
            .attach_printable("Rapyd dispute status has no UCS DisputeStatus counterpart")),
        }
    }
}

/// The dispute object carried on `PAYMENT_DISPUTE_CREATED` / `PAYMENT_DISPUTE_UPDATED`.
///
/// `amount` is in **major** units, consistent with every other amount Rapyd
/// sends (`ResponseData::amount` and `RefundResponseData::amount` are both
/// `FloatMajorUnit`). Hyperswitch declared this `MinorUnit`; that would report a
/// $10 dispute as $0.10.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DisputeResponseData {
    pub id: String,
    pub amount: FloatMajorUnit,
    pub currency: common_enums::Currency,
    /// The `dispute_*` identifier. This — not `id` — is the dispute id UCS reports.
    pub token: String,
    pub dispute_reason_description: String,
    #[serde(default, with = "common_utils::custom_serde::timestamp::option")]
    pub due_date: Option<PrimitiveDateTime>,
    pub status: RapydWebhookDisputeStatus,
    #[serde(default, with = "common_utils::custom_serde::timestamp::option")]
    pub created_at: Option<PrimitiveDateTime>,
    #[serde(default, with = "common_utils::custom_serde::timestamp::option")]
    pub updated_at: Option<PrimitiveDateTime>,
    /// Parent payment id (`payment_*`) — the payment lookup key.
    pub original_transaction_id: String,
    #[serde(default)]
    pub pre_dispute: Option<bool>,
}

/// The `data` object of a Rapyd webhook.
///
/// `#[serde(untagged)]` tries the variants in declaration order and takes the
/// first that deserialises cleanly, so the order below is load-bearing and is
/// pinned by the tests at the bottom of this file:
///
/// * `Dispute` first — it is the only payload carrying `token`,
///   `original_transaction_id` and `dispute_reason_description`, all required.
/// * `Refund` second — it is the only remaining payload carrying `payment`
///   (the parent payment id), which is required.
/// * `Payment` last — `ResponseData` is the most permissive of the three
///   (`next_action`/`transaction_id` are optional so the reduced
///   `PAYMENT_FAILED` body parses, and `RapydPaymentStatus` accepts unknown
///   values), so it must not be tried before the other two or it would swallow
///   refund and dispute bodies.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum WebhookData {
    Dispute(DisputeResponseData),
    Refund(RefundResponseData),
    /// Boxed: `ResponseData` is by far the largest of the three payloads (it is
    /// the full payment object), and an unboxed variant would size every
    /// `WebhookData` — including the two small ones — to match it.
    Payment(Box<ResponseData>),
}

impl From<ResponseData> for RapydPaymentsResponse {
    fn from(value: ResponseData) -> Self {
        Self {
            status: Status {
                error_code: NO_ERROR_CODE.to_owned(),
                status: None,
                message: None,
                response_code: None,
                operation_id: None,
            },
            data: Some(value),
        }
    }
}

impl From<RefundResponseData> for RefundResponse {
    fn from(value: RefundResponseData) -> Self {
        Self {
            status: Status {
                error_code: NO_ERROR_CODE.to_owned(),
                status: None,
                message: None,
                response_code: None,
                operation_id: None,
            },
            data: Some(value),
        }
    }
}

/// Maps the webhook envelope onto the UCS event taxonomy.
///
/// `PAYMENT_DISPUTE_UPDATED` fires on every dispute transition, so the real
/// event has to be read from `data.status`, not from the `type` field.
pub fn get_webhook_event_type(
    webhook: &RapydIncomingWebhook,
) -> domain_types::connector_types::EventType {
    use domain_types::connector_types::EventType;

    match webhook.webhook_type {
        RapydWebhookObjectEventType::PaymentCompleted
        | RapydWebhookObjectEventType::PaymentCaptured => EventType::PaymentIntentSuccess,
        RapydWebhookObjectEventType::PaymentFailed => EventType::PaymentIntentFailure,
        RapydWebhookObjectEventType::RefundCompleted => EventType::RefundSuccess,
        RapydWebhookObjectEventType::PaymentRefundFailed
        | RapydWebhookObjectEventType::PaymentRefundRejected => EventType::RefundFailure,
        RapydWebhookObjectEventType::PaymentDisputeCreated => EventType::DisputeOpened,
        RapydWebhookObjectEventType::PaymentDisputeUpdated => match &webhook.data {
            WebhookData::Dispute(dispute_data) => EventType::from(dispute_data.status),
            WebhookData::Payment(_) | WebhookData::Refund(_) => {
                EventType::IncomingWebhookEventUnspecified
            }
        },
        RapydWebhookObjectEventType::Unknown => EventType::IncomingWebhookEventUnspecified,
    }
}

/// Maps a webhook payment object onto an `AttemptStatus` using the same
/// `(status, next_action)` table the Authorize / PSync / Capture paths use.
/// `PAYMENT_FAILED` omits `next_action`; `NotApplicable` pairs with
/// `RapydPaymentStatus::Error` to yield `AttemptStatus::Failure`.
pub fn get_status_for_webhook(data: &ResponseData) -> common_enums::AttemptStatus {
    get_status(
        data.status.to_owned(),
        data.next_action
            .to_owned()
            .unwrap_or(NextAction::NotApplicable),
    )
}

/// Rapyd echoes `""` rather than `null` for unset string fields.
pub(super) fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|inner| !inner.is_empty())
}
