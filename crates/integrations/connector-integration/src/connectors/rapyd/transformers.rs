use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    ext_traits::OptionExt,
    pii::Email,
    request::Method,
    types::{FloatMajorUnitForConnector, MinorUnit, StringMinorUnitForConnector},
    FloatMajorUnit, StringMajorUnit,
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, CreateOrder, PSync, RSync, Refund,
        RepeatPayment, SetupMandate, Void,
    },
    connector_types::{
        ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData,
        ConnectorSpecificClientAuthenticationResponse, DisputeWebhookDetailsResponse,
        DisputeWebhookReference, EventType, MandateReference, MandateReferenceId,
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData, PaymentVoidData,
        PaymentWebhookReference, PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData,
        PaymentsSyncData,
        RapydClientAuthenticationResponse as RapydClientAuthenticationResponseDomain,
        RefundFlowData, RefundSyncData, RefundWebhookDetailsResponse, RefundWebhookReference,
        RefundsData, RefundsResponseData, RepeatPaymentData, ResponseId, SetupMandateRequestData,
        WebhookDetailsResponse, WebhookResourceReference,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext, WebhookError},
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_address,
    payment_method_data::{
        Card, GpayTokenizationData, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber,
        WalletData,
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
const EXTERNAL_THREE_DS_DOC_URL: &str =
    "https://docs.rapyd.net/en/creating-a-card-payment-with-3ds-authentication---external-3ds.html";
const CAPTURE_PAYMENT_DOC_URL: &str = "https://docs.rapyd.net/en/capture-payment.html";

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
/// (`in_credit_visa_card` / `in_debit_visa_card`, etc.) via `from_wallet_network`;
/// a wallet card network without a derived type uses the same placeholder.
///
/// A raw card payment that carries external 3DS authentication data is the
/// exception: it goes out as `is_visa_card` / `is_mastercard_card`, selected by
/// card network (`try_for_external_authentication`).
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
}

impl RapydPaymentMethodType {
    /// The `payment_method.type` of a raw card payment that carries external
    /// 3DS authentication data.
    ///
    /// Rapyd accepts `payment_method_options` per payment method type: the
    /// external-3DS options (`3d_version`, `cavv`, `eci`, `xid`,
    /// `ds_trans_id`) may be sent only with a type whose Get Payment Method
    /// Required Fields response lists them, and a type that does not list them
    /// refuses the whole request (`in_amex_card` answers
    /// `UNKNOWN_PAYMENT_METHOD_FIELD - [3D_VERSION]`). Such a type is known for
    /// Visa (`is_visa_card`) and Mastercard (`is_mastercard_card`) only, so any
    /// other network, or one that cannot be determined, is refused here rather
    /// than sent under a type known to reject the options.
    ///
    /// The network is the one the caller states (`card.card_network`), else the
    /// issuer derived from the card number.
    ///
    /// References:
    /// https://docs.rapyd.net/en/creating-a-card-payment-with-3ds-authentication---external-3ds.html
    /// https://docs.rapyd.net/en/get-payment-method-required-fields.html
    fn try_for_external_authentication<T: PaymentMethodDataTypes>(
        card: &Card<T>,
    ) -> Result<Self, error_stack::Report<IntegrationError>> {
        let not_supported = |message: String, additional_context: &str| {
            error_stack::Report::new(IntegrationError::NotSupported {
                message,
                connector: "rapyd",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Send external 3DS authentication data only with a Visa or Mastercard card; state card.card_network when the card number is not a raw PAN."
                            .to_owned(),
                    ),
                    doc_url: Some(EXTERNAL_THREE_DS_DOC_URL.to_owned()),
                    additional_context: Some(additional_context.to_owned()),
                },
            })
        };
        const PER_TYPE_CONTEXT: &str = "Rapyd accepts the external 3DS options per payment method type; a type that lists them is known for Visa and Mastercard only.";
        match card.card_network.as_ref() {
            Some(common_enums::CardNetwork::Visa) => Ok(Self::IsVisaCard),
            Some(common_enums::CardNetwork::Mastercard) => Ok(Self::IsMastercardCard),
            Some(other) => Err(not_supported(
                format!("external 3DS authentication data for card network {other}"),
                PER_TYPE_CONTEXT,
            )),
            None => match domain_types::utils::get_card_issuer(card.card_number.peek()) {
                Ok(domain_types::utils::CardIssuer::Visa) => Ok(Self::IsVisaCard),
                Ok(domain_types::utils::CardIssuer::Master) => Ok(Self::IsMastercardCard),
                Ok(other) => Err(not_supported(
                    format!("external 3DS authentication data for card network {other}"),
                    PER_TYPE_CONTEXT,
                )),
                Err(_) => Err(not_supported(
                    "external 3DS authentication data for a card whose network cannot be determined"
                        .to_owned(),
                    "No card.card_network was sent and the network could not be derived from the card number, so no payment method type that accepts the external 3DS options can be selected.",
                )),
            },
        }
    }

    /// Resolve the Rapyd `payment_method.type` for a digital-wallet card from
    /// its network and funding. Wallets carry only the network and credit/debit
    /// funding (the PAN lives in the decrypted payload), so the type is derived
    /// from those.
    ///
    /// No source gives a selection rule for the other card networks, so they
    /// go out under `in_amex_card`, the placeholder type raw cards use: a type
    /// the account does not enable is answered by Rapyd with an error, so the
    /// payment is forwarded rather than refused here.
    fn from_wallet_network(network: &str, card_type: Option<&str>) -> Self {
        let is_debit = matches!(card_type.map(str::to_lowercase).as_deref(), Some("debit"));
        match network.to_lowercase().as_str() {
            "visa" if is_debit => Self::InDebitVisaCard,
            "visa" => Self::InCreditVisaCard,
            "mastercard" | "master" if is_debit => Self::InDebitMastercardCard,
            "mastercard" | "master" => Self::InCreditMastercardCard,
            "amex" | "americanexpress" | "american express" => Self::InAmexCard,
            _ => Self::InAmexCard,
        }
    }
}

/// Rapyd `initiation_type` for `/v1/payments`: "Indicates how the transaction
/// is initiated." Only the merchant-initiated values a Charge can state are
/// modelled; the rest of Rapyd's vocabulary (`customer_present`, `moto`,
/// `no_show`, `delayed_charges`, `reauthorization`) is not sent by any flow.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RapydInitiationType {
    Recurring,
    Installment,
    Unscheduled,
}

impl RapydInitiationType {
    /// The MIT category of the request decides the value. A resubmission is a
    /// retried MIT with no Rapyd value of its own and goes out as
    /// `unscheduled`, which is also what is sent when the caller states no
    /// category.
    fn from_mit_category(mit_category: Option<&common_enums::MitCategory>) -> Self {
        match mit_category {
            Some(common_enums::MitCategory::Recurring) => Self::Recurring,
            Some(common_enums::MitCategory::Installment) => Self::Installment,
            Some(common_enums::MitCategory::Unscheduled)
            | Some(common_enums::MitCategory::Resubmission)
            | None => Self::Unscheduled,
        }
    }
}

/// Per-flow switches of the shared payment-object conversion
/// ([`convert_rapyd_payment_response`]).
#[derive(Debug, Clone, Copy)]
struct RapydPaymentResponseOptions {
    /// A 2xx body without `data` fails the attempt (flows that create a
    /// payment). Sync-style flows report the error and leave the status alone.
    no_data_is_terminal: bool,
    /// Surface `data.payment_method` (the stored `card_…` id) as the mandate.
    emit_mandate_reference: bool,
    /// Surface `payment_method_data.network_reference_id` and
    /// `transaction_link_id`.
    emit_network_txn_id: bool,
}

/// The single conversion of a Rapyd payment object into router data, shared by
/// every flow that receives one (Authorize, PSync, Capture, Void, SetupMandate,
/// RepeatPayment). The status is exactly what [`get_status`] yields — no flow
/// promotes or rewrites it.
fn convert_rapyd_payment_response<F, Req>(
    status: Status,
    data: Option<ResponseData>,
    http_code: u16,
    router_data: RouterDataV2<F, PaymentFlowData, Req, PaymentsResponseData>,
    options: RapydPaymentResponseOptions,
) -> Result<
    RouterDataV2<F, PaymentFlowData, Req, PaymentsResponseData>,
    error_stack::Report<ConnectorError>,
> {
    let Some(data) = data else {
        // A 2xx envelope that carries no payment object is not a success.
        return Ok(if options.no_data_is_terminal {
            RouterDataV2 {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::Failure,
                    ..router_data.resource_common_data
                },
                response: Err(build_rapyd_error_response(
                    &status,
                    None,
                    http_code,
                    Some(FlowStatus::Payment(common_enums::AttemptStatus::Failure)),
                )),
                ..router_data
            }
        } else {
            RouterDataV2 {
                response: Err(build_rapyd_error_response(&status, None, http_code, None)),
                ..router_data
            }
        });
    };

    let attempt_status = get_status(&data.status, &data.next_action);
    if attempt_status == common_enums::AttemptStatus::Failure {
        // `data.status = ERR`: the payment object itself says it failed,
        // whatever the HTTP status of the envelope was.
        return Ok(RouterDataV2 {
            resource_common_data: PaymentFlowData {
                status: common_enums::AttemptStatus::Failure,
                ..router_data.resource_common_data
            },
            response: Err(build_rapyd_error_response(
                &status,
                Some(&data),
                http_code,
                Some(FlowStatus::Payment(common_enums::AttemptStatus::Failure)),
            )),
            ..router_data
        });
    }

    let redirection_data = data
        .redirect_url
        .as_deref()
        .filter(|redirect_url| !redirect_url.is_empty())
        .map(|redirect_url| {
            Url::parse(redirect_url).change_context(
                crate::utils::response_handling_fail_for_connector(http_code, "rapyd"),
            )
        })
        .transpose()?
        .map(|url| Box::new(RedirectForm::from((url, Method::Get))));

    // The stored card id Rapyd returns when the card was saved is the mandate
    // reference for later MIT charges; Rapyd charges it without a customer id.
    let mandate_reference = if options.emit_mandate_reference {
        data.payment_method
            .as_ref()
            .filter(|card_id| !card_id.peek().is_empty())
            .map(|card_id| {
                Box::new(MandateReference {
                    connector_mandate_id: Some(card_id.clone().expose()),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                    mandate_metadata: None,
                })
            })
    } else {
        None
    };

    let (network_txn_id, network_txn_link_id) = if options.emit_network_txn_id {
        (
            data.payment_method_data
                .as_ref()
                .and_then(|payment_method_data| payment_method_data.network_reference_id.clone())
                .map(ExposeInterface::expose)
                .filter(|network_reference_id| !network_reference_id.is_empty()),
            data.transaction_link_id
                .clone()
                .filter(|transaction_link_id| !transaction_link_id.is_empty()),
        )
    } else {
        (None, None)
    };

    // `data.amount` of a closed payment is the amount that was captured (a
    // partial capture closes the payment with the partial amount).
    // Reported only when Rapyd names the currency: it is never assumed.
    let minor_amount_captured = if data.status == RapydPaymentStatus::Closed {
        data.currency_code
            .map(|currency| {
                domain_types::utils::convert_back_amount_to_minor_units(
                    &FloatMajorUnitForConnector,
                    data.amount,
                    currency,
                )
                .change_context(
                    crate::utils::response_handling_fail_for_connector(http_code, "rapyd"),
                )
            })
            .transpose()?
    } else {
        None
    };

    let connector_response = build_rapyd_connector_response(&data);

    let response = Ok(PaymentsResponseData::TransactionResponse {
        // `data.id` (`payment_…`) is the id every later call takes;
        // `data.transaction_id` is not.
        resource_id: ResponseId::ConnectorTransactionId(data.id),
        redirection_data,
        mandate_reference,
        connector_metadata: None,
        network_txn_id,
        network_txn_link_id,
        connector_response_reference_id: data.merchant_reference_id,
        incremental_authorization_allowed: None,
        status_code: http_code,
        splits: None,
        payment_account_reference: None,
    });

    let resource_common_data = router_data.resource_common_data;
    Ok(RouterDataV2 {
        resource_common_data: PaymentFlowData {
            status: attempt_status,
            amount_captured: minor_amount_captured
                .map(|minor_amount| minor_amount.get_amount_as_i64())
                .or(resource_common_data.amount_captured),
            minor_amount_captured: minor_amount_captured
                .or(resource_common_data.minor_amount_captured),
            connector_response: connector_response.or(resource_common_data.connector_response),
            ..resource_common_data
        },
        response,
        ..router_data
    })
}

/// AVS / CVV / ACS check results, the 3DS authentication result and the
/// authorization code of a payment object, when Rapyd returned any of them.
fn build_rapyd_connector_response(data: &ResponseData) -> Option<ConnectorResponseData> {
    let payment_checks = data
        .payment_method_data
        .as_ref()
        .and_then(|payment_method_data| {
            let mut checks = serde_json::Map::new();
            if let Some(cvv_check) = payment_method_data.cvv_check.as_ref() {
                checks.insert("cvv_check".to_owned(), serde_json::json!(cvv_check));
            }
            if let Some(avs_check) = payment_method_data
                .avs_check
                .as_deref()
                .filter(|avs_check| !avs_check.is_empty())
            {
                checks.insert("avs_check".to_owned(), serde_json::json!(avs_check));
            }
            if let Some(acs_check) = payment_method_data.acs_check.as_ref() {
                checks.insert("acs_check".to_owned(), serde_json::json!(acs_check));
            }
            (!checks.is_empty()).then_some(serde_json::Value::Object(checks))
        });

    let authentication_data =
        data.authentication_result
            .as_ref()
            .and_then(|authentication_result| {
                let mut result = serde_json::Map::new();
                for (key, value) in [
                    ("eci", authentication_result.eci.as_deref()),
                    ("result", authentication_result.result.as_deref()),
                    ("version", authentication_result.version.as_deref()),
                ] {
                    if let Some(value) = value.filter(|value| !value.is_empty()) {
                        result.insert(key.to_owned(), serde_json::json!(value));
                    }
                }
                (!result.is_empty()).then_some(serde_json::Value::Object(result))
            });

    let auth_code = data
        .auth_code
        .clone()
        .filter(|auth_code| !auth_code.is_empty());

    (payment_checks.is_some() || authentication_data.is_some() || auth_code.is_some()).then(|| {
        ConnectorResponseData::with_additional_payment_method_data(
            AdditionalPaymentMethodConnectorResponse::Card {
                authentication_data,
                payment_checks,
                card_network: None,
                domestic_network: None,
                auth_code,
            },
        )
    })
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<RapydPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<RapydPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        convert_rapyd_payment_response(
            item.response.status,
            item.response.data,
            item.http_code,
            item.router_data,
            RapydPaymentResponseOptions {
                no_data_is_terminal: true,
                emit_mandate_reference: true,
                emit_network_txn_id: true,
            },
        )
    }
}

impl TryFrom<ResponseRouterData<RapydPaymentsResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<RapydPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        convert_rapyd_payment_response(
            item.response.status,
            item.response.data,
            item.http_code,
            item.router_data,
            RapydPaymentResponseOptions {
                no_data_is_terminal: false,
                emit_mandate_reference: true,
                emit_network_txn_id: true,
            },
        )
    }
}

impl TryFrom<ResponseRouterData<RapydPaymentsResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<RapydPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        convert_rapyd_payment_response(
            item.response.status,
            item.response.data,
            item.http_code,
            item.router_data,
            RapydPaymentResponseOptions {
                no_data_is_terminal: false,
                emit_mandate_reference: false,
                emit_network_txn_id: false,
            },
        )
    }
}

impl TryFrom<ResponseRouterData<RapydPaymentsResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<RapydPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        convert_rapyd_payment_response(
            item.response.status,
            item.response.data,
            item.http_code,
            item.router_data,
            RapydPaymentResponseOptions {
                no_data_is_terminal: false,
                emit_mandate_reference: false,
                emit_network_txn_id: false,
            },
        )
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
    /// Card options (3DS). Left out of the body when the flow has none: a
    /// merchant-initiated payment must not ask for 3DS.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method_options: Option<PaymentMethodOptions>,
    pub merchant_reference_id: Option<String>,
    pub capture: Option<bool>,
    pub description: Option<String>,
    /// Redirect targets. Left out of the body when the caller gave no return
    /// URL (a merchant-initiated payment has no redirect).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub complete_payment_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
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
    /// Sent on merchant-initiated payments only: `recurring`, `installment`
    /// or `unscheduled`, from the request's MIT category.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initiation_type: Option<RapydInitiationType>,
    /// Billing address of the payment (root level, not `payment_method.address`).
    /// "Required when an Address Verification Service (AVS) check is being made."
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<Address>,
    /// Dynamic descriptor: "A text description suitable for a customer's
    /// payment statement. 5-22 characters."
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statement_descriptor: Option<String>,
    /// "Email address that the receipt for this transaction is sent to.
    /// Required for Visa card payments with 3DS authentication."
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt_email: Option<Email>,
    /// Browser of the customer; "Required for 3DS authentication of the
    /// customer for card payments."
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

/// Inline customer object embedded in a `/v1/payments` call. Both members
/// are always set, so this struct never produces an empty `{}` body: the
/// SetupMandate transformer enforces their presence at the request level,
/// and Authorize sends the object only when both are present (Rapyd
/// generates the customer of a saved card when `customer` is omitted).
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
    /// Card details with the network reference id of the initial payment in
    /// place of the CVV (merchant-initiated payments only).
    NetworkReference(Box<RapydNetworkReferencePaymentMethod>),
}

/// `payment_method` of a merchant-initiated payment that references the
/// initial cardholder-initiated one by its network reference id: "you can
/// store and reuse a network reference ID" instead of CVVs.
/// Reference: https://docs.rapyd.net/en/creating-a-card-payment-with-a-network-reference-id.html
#[derive(Debug, Serialize)]
pub struct RapydNetworkReferencePaymentMethod {
    #[serde(rename = "type")]
    pub pm_type: RapydPaymentMethodType,
    pub fields: RapydNetworkReferenceCardFields,
}

/// Card fields of the network-reference-id variant. No `cvv` member exists:
/// `network_reference_id` is sent instead of it.
#[derive(Debug, Serialize)]
pub struct RapydNetworkReferenceCardFields {
    pub number: cards::CardNumber,
    pub expiration_month: Secret<String>,
    pub expiration_year: Secret<String>,
    pub name: Secret<String>,
    /// "Use the initial network reference ID for all subsequent payments."
    pub network_reference_id: Secret<String>,
}

/// `payment_method_options` of a card payment. Either Rapyd runs 3DS itself
/// (`3d_required`), or the merchant passes the result of its own 3DS server
/// (`3d_version`, `cavv`, `eci`, `ds_trans_id`, `xid`) and `3d_required` is
/// omitted, as in the documented external-authentication request.
/// Reference: https://docs.rapyd.net/en/creating-a-card-payment-with-3ds-authentication---external-3ds.html
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
pub struct PaymentMethodOptions {
    #[serde(rename = "3d_required")]
    pub three_ds: Option<bool>,
    /// "3DS version": `1.0.2` | `2.1.0` | `2.2.0`.
    #[serde(rename = "3d_version")]
    pub three_ds_version: Option<String>,
    /// "Cardholder authentication verification value (CAVV) ... Base64 encoded".
    pub cavv: Option<Secret<String>>,
    /// "Electronic commerce indicator (ECI) from MPI Plugin (3-D Secure 1.0)
    /// or 3DS Server".
    pub eci: Option<String>,
    /// "The directory server transaction ID. Required for Mastercard 2.0".
    pub ds_trans_id: Option<String>,
    /// "3D Secure XID, Base64 encoded. Required for VISA 1.0".
    pub xid: Option<String>,
}

impl PaymentMethodOptions {
    /// Rapyd-run 3DS: only the `3d_required` flag is sent.
    fn three_ds_required(three_ds: bool) -> Self {
        Self {
            three_ds: Some(three_ds),
            three_ds_version: None,
            cavv: None,
            eci: None,
            ds_trans_id: None,
            xid: None,
        }
    }
}

/// External 3DS: the values of the merchant's own authentication. `3d_version`,
/// `cavv` and `eci` are documented as required, so a partial set is refused by
/// name instead of being sent as an unauthenticated payment.
impl TryFrom<&AuthenticationData> for PaymentMethodOptions {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(authentication_data: &AuthenticationData) -> Result<Self, Self::Error> {
        let missing = |field_name: &'static str| {
            IntegrationError::MissingRequiredField {
            field_name,
            context: crate::utils::integration_ctx(
                "Rapyd requires 3d_version, cavv and eci when external authentication data is passed",
                "Send cavv, eci and message_version in authentication_data, or omit authentication_data to let Rapyd run 3DS.",
            ),
        }
        };
        let cavv = authentication_data
            .cavv
            .clone()
            .ok_or_else(|| missing("authentication_data.cavv"))?;
        let eci = authentication_data
            .eci
            .clone()
            .ok_or_else(|| missing("authentication_data.eci"))?;
        let three_ds_version = authentication_data
            .message_version
            .as_ref()
            .map(ToString::to_string)
            .ok_or_else(|| missing("authentication_data.message_version"))?;
        Ok(Self {
            three_ds: None,
            three_ds_version: Some(three_ds_version),
            cavv: Some(cavv),
            eci: Some(eci),
            ds_trans_id: authentication_data.ds_trans_id.clone(),
            xid: authentication_data.transaction_id.clone(),
        })
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

/// Rapyd address object (Create Address), accepted inline as the root-level
/// `address` of Create Payment.
/// Reference: https://docs.rapyd.net/en/create-address.html
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
pub struct Address {
    name: Secret<String>,
    line_1: Secret<String>,
    line_2: Option<Secret<String>>,
    line_3: Option<Secret<String>>,
    city: Option<Secret<String>>,
    state: Option<Secret<String>>,
    /// Two-letter ISO 3166-1 alpha-2 code.
    country: Option<common_enums::CountryAlpha2>,
    zip: Option<Secret<String>>,
    /// Country code followed by the number.
    phone_number: Option<Secret<String>>,
}

impl Address {
    /// Billing address -> Rapyd `address`. Rapyd rejects an address without
    /// `name` or `line_1`, so the object is produced only when the billing
    /// address has both; every other member is passed when present and never
    /// defaulted.
    fn from_billing(billing: Option<&payment_address::Address>) -> Option<Self> {
        let billing = billing?;
        let details = billing.address.as_ref()?;
        let name = details.get_optional_full_name()?;
        let line_1 = details.get_optional_line1()?;
        Some(Self {
            name,
            line_1,
            line_2: details.get_optional_line2(),
            line_3: details.line3.clone(),
            city: details.get_optional_city(),
            state: details.get_optional_state(),
            country: details.get_optional_country(),
            zip: details.get_optional_zip(),
            phone_number: billing
                .phone
                .as_ref()
                .and_then(|phone| phone.get_number_with_country_code().ok()),
        })
    }
}

/// Rapyd `client_details`: the customer's browser, used for 3DS.
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
pub struct RapydClientDetails {
    /// "Required for Visa and Mastercard payments with 3DS."
    ip_address: Option<Secret<String, common_utils::pii::IpAddress>>,
    accept_header: Option<String>,
    /// IETF BCP 47 language tag.
    language: Option<String>,
    java_enabled: Option<bool>,
    java_script_enabled: Option<bool>,
    screen_color_depth: Option<u8>,
    screen_height: Option<u32>,
    screen_width: Option<u32>,
    /// Minutes from UTC.
    time_zone_offset: Option<i32>,
}

impl From<&BrowserInformation> for RapydClientDetails {
    fn from(browser_info: &BrowserInformation) -> Self {
        Self {
            ip_address: browser_info
                .ip_address
                .map(|ip_address| Secret::new(ip_address.to_string())),
            accept_header: browser_info.accept_header.clone(),
            language: browser_info.language.clone(),
            java_enabled: browser_info.java_enabled,
            java_script_enabled: browser_info.java_script_enabled,
            screen_color_depth: browser_info.color_depth,
            screen_height: browser_info.screen_height,
            screen_width: browser_info.screen_width,
            time_zone_offset: browser_info.time_zone,
        }
    }
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
        // Rapyd has one capture per payment (Capture Payment closes it, a
        // partial capture included) and no scheduled capture, so these two
        // capture methods are refused for every payment method rather than
        // run as a single manual capture.
        if matches!(
            item.router_data.request.capture_method,
            Some(common_enums::CaptureMethod::ManualMultiple)
                | Some(common_enums::CaptureMethod::Scheduled)
        ) {
            return Err(IntegrationError::CaptureMethodNotSupported {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Use capture_method automatic, sequential_automatic or manual."
                            .to_owned(),
                    ),
                    doc_url: Some(CAPTURE_PAYMENT_DOC_URL.to_owned()),
                    additional_context: Some(
                        "Rapyd captures a card payment once: Capture Payment closes the payment, so multiple captures and scheduled capture are not available."
                            .to_owned(),
                    ),
                },
            }
            .into());
        }
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

        // Capture intent applies to every payment method; the `three_ds`
        // option is card-only (wallets carry their own authentication).
        let capture = Some(item.router_data.request.is_auto_capture());
        // External 3DS (raw card data only): the merchant's own authentication
        // values replace `3d_required`. A partial set is refused by name before
        // anything is built (see `TryFrom<&AuthenticationData>`).
        let external_authentication = match (
            &item.router_data.request.payment_method_data,
            item.router_data.request.authentication_data.as_ref(),
        ) {
            (PaymentMethodData::Card(card), Some(authentication_data)) => {
                let options = PaymentMethodOptions::try_from(authentication_data)?;
                // The options are accepted per payment method type: they are
                // sent only with the type selected for the card network, and a
                // network without such a type is refused here.
                let pm_type = RapydPaymentMethodType::try_for_external_authentication(card)?;
                Some((options, pm_type))
            }
            _ => None,
        };
        let (external_authentication, external_authentication_type) =
            external_authentication.unzip();
        let payment_method_options = match external_authentication {
            Some(options) => Some(options),
            None => matches!(
                item.router_data.resource_common_data.payment_method,
                common_enums::PaymentMethod::Card
            )
            .then(|| {
                PaymentMethodOptions::three_ds_required(matches!(
                    item.router_data.resource_common_data.auth_type,
                    common_enums::AuthenticationType::ThreeDs
                ))
            }),
        };
        let is_card = matches!(
            item.router_data.request.payment_method_data,
            PaymentMethodData::Card(_)
        );
        let payment_method = match item.router_data.request.payment_method_data {
            PaymentMethodData::Card(ref ccard) => {
                // `payment_method.fields.name` is the card holder name and Rapyd
                // refuses an empty one (INVALID_HOLDER_NAME). Prefer the name on
                // the card itself, fall back to the billing full name; a value
                // that is empty after trimming counts as absent, and with neither
                // present the payment is refused before any request is built.
                let cardholder_name = ccard
                    .card_holder_name
                    .clone()
                    .filter(|name| !name.peek().trim().is_empty())
                    .or_else(|| {
                        item.router_data
                            .resource_common_data
                            .get_optional_billing_full_name()
                            .filter(|name| !name.peek().trim().is_empty())
                    })
                    .ok_or_else(|| IntegrationError::MissingRequiredField {
                        field_name: "card.card_holder_name / billing.full_name",
                        context: crate::utils::integration_ctx(
                            "Rapyd requires payment_method.fields.name (the card holder name) for card payments",
                            "Send card.card_holder_name, or a billing address with first_name / last_name.",
                        ),
                    })?;
                Some(RapydPaymentMethodData::PaymentMethod(Box::new(
                    PaymentMethod {
                        // With external 3DS authentication data: the type
                        // selected for the card network, the only ones that
                        // accept those options. Otherwise the placeholder India
                        // type: Rapyd's valid `payment_method.type` is country-
                        // and merchant-specific (this sandbox enables
                        // `in_amex_card`, not plain `in_visa_card`), so the correct
                        // per-network/funding mapping is deferred; sandbox is lenient
                        // about the card BIN.
                        pm_type: match external_authentication_type {
                            Some(pm_type) => pm_type,
                            None => RapydPaymentMethodType::InAmexCard,
                        },
                        fields: Some(PaymentFields {
                            number: ccard.card_number.to_owned(),
                            expiration_month: ccard.card_exp_month.to_owned(),
                            expiration_year: ccard.card_exp_year.to_owned(),
                            name: cardholder_name,
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
                        let pm_type = RapydPaymentMethodType::from_wallet_network(
                            &data.info.card_network,
                            None,
                        );
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
                        let pm_type = RapydPaymentMethodType::from_wallet_network(
                            &data.payment_method.network,
                            Some(data.payment_method.pm_type.as_str()),
                        );
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
        // When the merchant requests future use, ask Rapyd to save the card so
        // the response carries the reusable `card_*` token (the mandate) for
        // later MIT calls. Rapyd requires `customer` only when `payment_method`
        // is blank, so the inline customer is sent when both its members are
        // present and omitted otherwise (Rapyd then generates the customer).
        let (customer, save_payment_method) =
            if item.router_data.request.setup_future_usage.is_some() {
                let customer = item
                    .router_data
                    .request
                    .get_optional_customer_name()
                    .zip(item.router_data.request.get_optional_email())
                    .map(|(name, email)| {
                        RapydCustomerRef::Inline(RapydInlineCustomer { name, email })
                    });
                (customer, Some(true))
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
            // Root-level billing address (AVS input); omitted unless complete.
            address: Address::from_billing(
                item.router_data.resource_common_data.get_optional_billing(),
            ),
            // Dynamic descriptor, passed through unmodified.
            statement_descriptor: item
                .router_data
                .request
                .billing_descriptor
                .as_ref()
                .and_then(|billing_descriptor| billing_descriptor.statement_descriptor.clone()),
            // Documented 3DS inputs of a card payment, sent whenever the caller
            // provided them (the issuer or Rapyd can require 3DS even when it
            // was not requested) and never defaulted. Card-only: wallets carry
            // their own authentication.
            receipt_email: is_card
                .then(|| item.router_data.request.email.clone())
                .flatten(),
            client_details: is_card
                .then(|| {
                    item.router_data
                        .request
                        .browser_info
                        .as_ref()
                        .map(RapydClientDetails::from)
                })
                .flatten(),
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
    /// Any value outside the documented set; parsed instead of failing the
    /// whole response.
    #[serde(other)]
    Unknown,
}

/// Payment state = `status` + `next_action`.
/// Reference: https://docs.rapyd.net/en/payment.html (status, next_action)
fn get_status(
    status: &RapydPaymentStatus,
    next_action: &NextAction,
) -> common_enums::AttemptStatus {
    match status {
        // CLO: "Closed." — the payment is completed / captured.
        RapydPaymentStatus::Closed => common_enums::AttemptStatus::Charged,
        // ACT: "Active and awaiting completion of 3DS or capture."
        RapydPaymentStatus::Active => match next_action {
            // 3d_verification: redirect the customer to `redirect_url`.
            NextAction::ThreedsVerification => common_enums::AttemptStatus::AuthenticationPending,
            // pending_confirmation: awaiting payment confirmation.
            NextAction::PendingConfirmation => common_enums::AttemptStatus::AuthenticationPending,
            // pending_capture: authorized, awaiting amount capture.
            NextAction::PendingCapture => common_enums::AttemptStatus::Authorized,
            // not_applicable while ACT: no action needed (Hyperswitch reference).
            NextAction::NotApplicable => common_enums::AttemptStatus::Authorized,
            // pending_offline_capture: settles later via clearing — not final.
            NextAction::PendingOfflineCapture => common_enums::AttemptStatus::Pending,
            // Undocumented next_action: never resolved to a terminal status.
            NextAction::Unknown => common_enums::AttemptStatus::Pending,
        },
        // CAN: "Canceled by the client or the customer's bank."
        RapydPaymentStatus::CanceledByClientOrBank => common_enums::AttemptStatus::Voided,
        // EXP: "The payment has expired."
        RapydPaymentStatus::Expired => common_enums::AttemptStatus::Voided,
        // REV: "Reversed by Rapyd. See cancel_reason."
        RapydPaymentStatus::ReversedByRapyd => common_enums::AttemptStatus::Voided,
        // ERR: "Error. An attempt was made to create or complete a payment,
        // but it failed."
        RapydPaymentStatus::Error => common_enums::AttemptStatus::Failure,
        // NEW: not a documented payment status (Hyperswitch reference).
        RapydPaymentStatus::New => common_enums::AttemptStatus::Authorizing,
        // Undocumented status: never resolved to a terminal status.
        RapydPaymentStatus::Unknown => common_enums::AttemptStatus::Pending,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RapydPaymentsResponse {
    pub status: Status,
    pub data: Option<ResponseData>,
}

/// Envelope of a non-2xx Rapyd response. `status` is its only required
/// member. `data` may be absent, null, or a payment-shaped object that is not
/// a complete payment — a refused Create Payment carries `"id": null` — so
/// none of its members is required and none of those shapes fails the parse.
/// The success path keeps the strict [`RapydPaymentsResponse`].
///
/// Reference: https://docs.rapyd.net/en/rapyd-error-codes.html
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RapydErrorEnvelope {
    pub status: Status,
    #[serde(default, deserialize_with = "deserialize_error_data")]
    pub data: Option<RapydErrorData>,
}

/// The members of a non-2xx envelope's `data` the error builder reads. Each is
/// optional on its own: a present `failure_code` is kept when `id` is null.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RapydErrorData {
    #[serde(default, deserialize_with = "deserialize_error_member")]
    pub id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_error_member")]
    pub failure_code: Option<String>,
    #[serde(default, deserialize_with = "deserialize_error_member")]
    pub failure_message: Option<String>,
    #[serde(default, deserialize_with = "deserialize_error_member")]
    pub error_code: Option<String>,
    #[serde(default, deserialize_with = "deserialize_error_member")]
    pub merchant_advice_code: Option<String>,
}

/// `data` of a non-2xx envelope: kept when it is an object, absent otherwise
/// (missing, null, or any other JSON value).
fn deserialize_error_data<'de, D>(deserializer: D) -> Result<Option<RapydErrorData>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value
        .filter(serde_json::Value::is_object)
        .and_then(|value| serde_json::from_value(value).ok()))
}

/// A member of a non-2xx envelope's `data`: a string, or a number rendered as
/// one, is kept; null and every other JSON value count as absent.
fn deserialize_error_member<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(
        match Option::<serde_json::Value>::deserialize(deserializer)? {
            Some(serde_json::Value::String(value)) => Some(value),
            Some(serde_json::Value::Number(value)) => Some(value.to_string()),
            Some(serde_json::Value::Null)
            | Some(serde_json::Value::Bool(_))
            | Some(serde_json::Value::Array(_))
            | Some(serde_json::Value::Object(_))
            | None => None,
        },
    )
}

/// The members of a payment object the error builder reads, borrowed from
/// either the strict success-path [`ResponseData`] or the tolerant
/// [`RapydErrorData`] of a non-2xx envelope.
#[derive(Debug, Clone, Copy)]
struct RapydErrorDetails<'a> {
    id: Option<&'a String>,
    failure_code: Option<&'a String>,
    failure_message: Option<&'a String>,
    error_code: Option<&'a String>,
    merchant_advice_code: Option<&'a String>,
}

impl<'a> From<&'a ResponseData> for RapydErrorDetails<'a> {
    fn from(data: &'a ResponseData) -> Self {
        Self {
            id: Some(&data.id),
            failure_code: data.failure_code.as_ref(),
            failure_message: data.failure_message.as_ref(),
            error_code: data.error_code.as_ref(),
            merchant_advice_code: data.merchant_advice_code.as_ref(),
        }
    }
}

impl<'a> From<&'a RapydErrorData> for RapydErrorDetails<'a> {
    fn from(data: &'a RapydErrorData) -> Self {
        Self {
            id: data.id.as_ref(),
            failure_code: data.failure_code.as_ref(),
            failure_message: data.failure_message.as_ref(),
            error_code: data.error_code.as_ref(),
            merchant_advice_code: data.merchant_advice_code.as_ref(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Status {
    pub error_code: String,
    pub status: Option<String>,
    pub message: Option<String>,
    pub response_code: Option<String>,
    pub operation_id: Option<String>,
}

const CARD_NETWORK_ERROR_PREFIX: &str = "ERROR_PROCESSING_CARD";

/// The card network (issuer) response code carried inside a Rapyd error code.
///
/// Documented format: "ERROR_PROCESSING_CARD — indicates card network
/// rejection, followed by a network-specific error code in square brackets
/// (e.g., `ERROR_PROCESSING_CARD - [51]`)". Any other error code — including
/// bracketed ones such as `UNKNOWN_PAYMENT_METHOD_FIELD - [RECURRENCE_TYPE]` —
/// yields `None`.
/// Reference: https://docs.rapyd.net/en/card-network-errors.html
fn extract_network_decline_code(error_code: &str) -> Option<String> {
    let code = error_code
        .trim()
        .strip_prefix(CARD_NETWORK_ERROR_PREFIX)?
        .trim_start()
        .strip_prefix('-')?
        .trim()
        .strip_prefix('[')?
        .strip_suffix(']')?
        .trim();
    (!code.is_empty()).then(|| code.to_owned())
}

/// The one error builder of the connector: every flow turns a Rapyd envelope
/// (`status`, plus the payment object when there is one) into an
/// `ErrorResponse` here. Rapyd sends empty strings for absent values, so an
/// empty member is treated as absent.
pub(super) fn build_rapyd_error_response(
    status: &Status,
    data: Option<&ResponseData>,
    http_code: u16,
    attempt_status: Option<FlowStatus>,
) -> ErrorResponse {
    build_rapyd_error_response_from_details(
        status,
        data.map(RapydErrorDetails::from),
        http_code,
        attempt_status,
    )
}

/// [`build_rapyd_error_response`] for a non-2xx envelope, whose `data` may be
/// absent or an incomplete payment object (`id` null): there is then no
/// `connector_transaction_id`, and every other member is still read.
pub(super) fn build_rapyd_error_response_from_envelope(
    envelope: &RapydErrorEnvelope,
    http_code: u16,
    attempt_status: Option<FlowStatus>,
) -> ErrorResponse {
    build_rapyd_error_response_from_details(
        &envelope.status,
        envelope.data.as_ref().map(RapydErrorDetails::from),
        http_code,
        attempt_status,
    )
}

fn build_rapyd_error_response_from_details(
    status: &Status,
    data: Option<RapydErrorDetails<'_>>,
    http_code: u16,
    attempt_status: Option<FlowStatus>,
) -> ErrorResponse {
    let present = |value: Option<&String>| -> Option<String> {
        value.filter(|value| !value.is_empty()).cloned()
    };
    let failure_code = data.and_then(|data| present(data.failure_code));
    let failure_message = data.and_then(|data| present(data.failure_message));
    let status_message = present(status.message.as_ref());

    // The bracketed issuer code of `ERROR_PROCESSING_CARD - [NN]`; the same
    // code appears bare in `data.failure_code`.
    let bracketed_decline_code = data
        .and_then(|data| data.error_code)
        .and_then(|error_code| extract_network_decline_code(error_code))
        .or_else(|| extract_network_decline_code(&status.error_code));
    let network_error_message = failure_message.clone().or_else(|| {
        bracketed_decline_code
            .as_ref()
            .and_then(|_| status_message.clone())
    });

    ErrorResponse {
        code: failure_code
            .clone()
            .or_else(|| present(Some(&status.error_code)))
            .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
        message: present(status.status.as_ref()).unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
        reason: failure_message.or(status_message),
        status_code: http_code,
        attempt_status,
        connector_transaction_id: data.and_then(|data| present(data.id)),
        network_decline_code: bracketed_decline_code.or(failure_code),
        network_advice_code: data.and_then(|data| present(data.merchant_advice_code)),
        network_error_message,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
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
    /// Any value outside the documented set.
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
    pub payment_method: Option<Secret<String>>,
    /// Nested payment-method data; carries `network_reference_id`, the
    /// network transaction id surfaced as `network_txn_id` for recurring.
    pub payment_method_data: Option<RapydResponsePaymentMethodData>,
    /// `ERROR_PROCESSING_CARD - [NN]` on a failed card payment; empty otherwise.
    pub error_code: Option<String>,
    /// "Optional code with additional information about the payment for the
    /// merchant." (MAC)
    pub merchant_advice_code: Option<String>,
    /// "Description of the merchant advice code."
    pub merchant_advice_message: Option<String>,
    /// "Indicates that the card payment was authorized by the card network."
    pub auth_code: Option<String>,
    /// "A unique transaction identifier. Save the ID to link future lifecycle
    /// and economically-related transactions to the current transaction."
    pub transaction_link_id: Option<String>,
    /// 3DS details of the payment.
    pub authentication_result: Option<RapydAuthenticationResult>,
}

/// Rapyd `authentication_result`: `eci` (05/02 authenticated, 06/01 attempted,
/// 07/00 failed or not attempted), `result` (`A` completed, `N` not
/// authenticated, `R` redirection awaiting action, `U` unsupported), `version`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RapydAuthenticationResult {
    pub eci: Option<String>,
    pub result: Option<String>,
    pub version: Option<String>,
}

/// Result of a card check (`cvv_check`, `acs_check`).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RapydCheckResult {
    Pass,
    Fail,
    Unavailable,
    Unchecked,
    /// Any value outside the documented set.
    #[serde(other)]
    Unknown,
}

/// Subset of Rapyd's response `payment_method_data` object.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RapydResponsePaymentMethodData {
    /// Payment method id (`card_…`) when the card was saved.
    pub id: Option<Secret<String>>,
    pub network_reference_id: Option<Secret<String>>,
    pub cvv_check: Option<RapydCheckResult>,
    pub acs_check: Option<RapydCheckResult>,
    /// "Results of the Address Verification Service (AVS) check." The
    /// documentation enumerates no values, so it stays a string.
    pub avs_check: Option<String>,
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
    #[default]
    Pending,
    Canceled,
    /// Any value outside the documented set.
    #[serde(other)]
    Unknown,
}

/// Reference: https://docs.rapyd.net/en/refund.html (status)
impl From<RefundStatus> for common_enums::RefundStatus {
    fn from(item: RefundStatus) -> Self {
        match item {
            // Completed: "The refund was complete."
            RefundStatus::Completed => Self::Success,
            // Pending: "the refund is not yet complete."
            RefundStatus::Pending => Self::Pending,
            // Rejected: "rejected by the specific payment processor".
            RefundStatus::Rejected => Self::Failure,
            // Error: "The refund failed since the payment object on which the
            // refund is based is not in closed status."
            RefundStatus::Error => Self::Failure,
            // Canceled: "The merchant canceled the refund."
            RefundStatus::Canceled => Self::Failure,
            // Undocumented status: never resolved to a terminal status.
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
    /// Refund failure code; an empty string when the refund did not fail.
    pub failure_code: Option<String>,
}

/// The single conversion of a Rapyd refund object into router data, shared by
/// Refund and RSync. A refund object always keeps its own id and the status
/// the status map yields (`Rejected` / `Error` / `Canceled` are a failed
/// refund, not an error response).
///
/// `no_data_is_terminal`: a 2xx envelope without `data` fails the refund
/// (Refund: nothing was created). RSync reports the error and leaves the
/// stored refund status alone.
fn convert_rapyd_refund_response<F, Req>(
    response: RefundResponse,
    http_code: u16,
    router_data: RouterDataV2<F, RefundFlowData, Req, RefundsResponseData>,
    no_data_is_terminal: bool,
) -> RouterDataV2<F, RefundFlowData, Req, RefundsResponseData> {
    let Some(data) = response.data else {
        // A 2xx envelope that carries no refund object is not a success, and
        // its error code is never a refund id.
        return if no_data_is_terminal {
            RouterDataV2 {
                resource_common_data: RefundFlowData {
                    status: common_enums::RefundStatus::Failure,
                    ..router_data.resource_common_data
                },
                response: Err(build_rapyd_error_response(
                    &response.status,
                    None,
                    http_code,
                    Some(FlowStatus::Refund(common_enums::RefundStatus::Failure)),
                )),
                ..router_data
            }
        } else {
            RouterDataV2 {
                response: Err(build_rapyd_error_response(
                    &response.status,
                    None,
                    http_code,
                    None,
                )),
                ..router_data
            }
        };
    };

    RouterDataV2 {
        response: Ok(RefundsResponseData {
            connector_refund_id: data.id,
            refund_status: common_enums::RefundStatus::from(data.status),
            status_code: http_code,
            acquirer_reference_number: None,
        }),
        ..router_data
    }
}

impl TryFrom<ResponseRouterData<RefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<RefundResponse, Self>) -> Result<Self, Self::Error> {
        Ok(convert_rapyd_refund_response(
            item.response,
            item.http_code,
            item.router_data,
            true,
        ))
    }
}

impl TryFrom<ResponseRouterData<RefundResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<RefundResponse, Self>) -> Result<Self, Self::Error> {
        Ok(convert_rapyd_refund_response(
            item.response,
            item.http_code,
            item.router_data,
            false,
        ))
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
            .ok_or_else(|| IntegrationError::MissingRequiredField {
                field_name: "minor_amount",
                context: crate::utils::integration_ctx(
                    "Rapyd card verification needs an explicit amount; none is assumed",
                    "Send the amount on the setup-mandate request (0 for a zero-amount verification).",
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
                // (inline `{name, email}`) — not the cardholder. A value that is
                // empty after trimming counts as absent: Rapyd refuses a blank
                // name (INVALID_HOLDER_NAME), so it is refused here instead.
                let cardholder_name = ccard
                    .card_holder_name
                    .clone()
                    .filter(|name| !name.peek().trim().is_empty())
                    .or_else(|| {
                        router_data
                            .resource_common_data
                            .get_optional_billing_full_name()
                            .filter(|name| !name.peek().trim().is_empty())
                    })
                    .ok_or_else(|| IntegrationError::MissingRequiredField {
                        field_name: "card.card_holder_name / billing.full_name",
                        context: crate::utils::integration_ctx(
                            "Rapyd requires payment_method.fields.name (the card holder name) for card payments",
                            "Send card.card_holder_name, or a billing address with first_name / last_name.",
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
                    crate::utils::integration_ctx(
                        "Rapyd mandate setup is implemented for raw card data only",
                        "Send a card as the payment method of the setup-mandate request.",
                    ),
                ))?;
            }
        };

        let three_ds_enabled = matches!(
            router_data.resource_common_data.auth_type,
            common_enums::AuthenticationType::ThreeDs
        );
        let payment_method_options =
            Some(PaymentMethodOptions::three_ds_required(three_ds_enabled));

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

        let return_url = router_data
            .resource_common_data
            .return_url
            .clone()
            .ok_or_else(|| IntegrationError::MissingRequiredField {
                field_name: "return_url",
                context: crate::utils::integration_ctx(
                    "Rapyd runs 3DS on card verification and needs complete_payment_url / error_payment_url",
                    "Send return_url on the setup-mandate request.",
                ),
            })?;

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
            // Rapyd runs 3DS on a card verification even with
            // `3d_required: false`; its documented 3DS inputs are the billing
            // address, the receipt email and the customer's browser. Each is
            // sent when the caller provided it and never defaulted.
            address: Address::from_billing(router_data.resource_common_data.get_optional_billing()),
            statement_descriptor: None,
            receipt_email: request.email.clone(),
            client_details: request.browser_info.as_ref().map(RapydClientDetails::from),
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
        convert_rapyd_payment_response(
            item.response.status,
            item.response.data,
            item.http_code,
            item.router_data,
            RapydPaymentResponseOptions {
                no_data_is_terminal: true,
                emit_mandate_reference: true,
                emit_network_txn_id: true,
            },
        )
    }
}

// ---------------------------------------------------------------------------
// RepeatPayment (MIT) — Rapyd reuses /v1/payments with either the stored
// card id (`card_…`, returned when the card was saved) as `payment_method`,
// or card details carrying the network reference id of the initial payment.
// The request body is structurally identical to `RapydPaymentsRequest`, but
// we use a distinct response newtype so the TryFrom impls don't collide with
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

        // Same rule as the Authorize flow: Rapyd has one capture per payment
        // and no scheduled capture, so these two capture methods are refused
        // for every mandate reference rather than run as a single manual
        // capture.
        if matches!(
            request.capture_method,
            Some(common_enums::CaptureMethod::ManualMultiple)
                | Some(common_enums::CaptureMethod::Scheduled)
        ) {
            return Err(IntegrationError::CaptureMethodNotSupported {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Use capture_method automatic, sequential_automatic or manual."
                            .to_owned(),
                    ),
                    doc_url: Some(CAPTURE_PAYMENT_DOC_URL.to_owned()),
                    additional_context: Some(
                        "Rapyd captures a card payment once: Capture Payment closes the payment, so multiple captures and scheduled capture are not available."
                            .to_owned(),
                    ),
                },
            }
            .into());
        }

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

        let payment_method = match &request.mandate_reference {
            // (a) Stored card: the `card_…` id returned when the card was saved
            // (SetupMandate, or an Authorize that saved it) is the whole
            // `payment_method`.
            MandateReferenceId::ConnectorMandateId(connector_mandate) => {
                let card_id = connector_mandate.get_connector_mandate_id().ok_or_else(|| {
                    IntegrationError::MissingRequiredField {
                        field_name: "mandate_reference.connector_mandate_id",
                        context: crate::utils::integration_ctx(
                            "Rapyd charges a stored card by its card_… id, which the connector mandate reference did not carry",
                            "Send the connector_mandate_id returned when the card was saved.",
                        ),
                    }
                })?;
                RapydPaymentMethodData::Token(Secret::new(card_id))
            }
            // (b) Network reference id: full card details with the network
            // transaction id of the initial payment in place of the CVV.
            MandateReferenceId::NetworkMandateId(network_mandate) => {
                let PaymentMethodData::CardDetailsForNetworkTransactionId(card_details) =
                    &request.payment_method_data
                else {
                    return Err(IntegrationError::MissingRequiredField {
                        field_name: "payment_method.card_details_for_network_transaction_id",
                        context: crate::utils::integration_ctx(
                            "Rapyd takes a network reference id only together with the card number, expiry and card holder name",
                            "Send card_details_for_network_transaction_id as the payment method when charging with a network_transaction_id.",
                        ),
                    })?;
                };
                // `payment_method.fields.name` is a required card field; a
                // value that is empty after trimming counts as absent.
                let cardholder_name = card_details
                    .card_holder_name
                    .clone()
                    .filter(|name| !name.peek().trim().is_empty())
                    .ok_or_else(|| IntegrationError::MissingRequiredField {
                        field_name:
                            "payment_method.card_details_for_network_transaction_id.card_holder_name",
                        context: crate::utils::integration_ctx(
                            "Rapyd requires payment_method.fields.name (the card holder name) on a network-reference-id payment",
                            "Send card_holder_name in card_details_for_network_transaction_id.",
                        ),
                    })?;
                RapydPaymentMethodData::NetworkReference(Box::new(
                    RapydNetworkReferencePaymentMethod {
                        // Same placeholder type as every raw-card request of
                        // this connector (see the Authorize flow).
                        pm_type: RapydPaymentMethodType::InAmexCard,
                        fields: RapydNetworkReferenceCardFields {
                            number: card_details.card_number.clone(),
                            expiration_month: card_details.card_exp_month.clone(),
                            expiration_year: card_details.card_exp_year.clone(),
                            name: cardholder_name,
                            network_reference_id: Secret::new(
                                network_mandate.network_transaction_id.clone(),
                            ),
                        },
                    },
                ))
            }
            // Rapyd documents no merchant-initiated payment on a network token.
            MandateReferenceId::NetworkTokenWithNTI(_) => {
                return Err(IntegrationError::NotImplemented(
                    "network token mandate for rapyd RepeatPayment".to_owned(),
                    crate::utils::integration_ctx(
                        "Rapyd merchant-initiated payments are implemented for a stored card id and for card details with a network transaction id",
                        "Charge with a connector_mandate_id, or with card_details_for_network_transaction_id and a network_transaction_id.",
                    ),
                ))?;
            }
        };

        // Optional on a merchant-initiated payment (no redirect happens): sent
        // when the caller gave one, left out otherwise. On Charge the URL
        // arrives on `request.router_return_url`.
        let return_url = request
            .router_return_url
            .clone()
            .or_else(|| router_data.resource_common_data.return_url.clone());

        Ok(Self {
            amount,
            currency: request.currency,
            payment_method,
            // Honor the caller's capture intent; SequentialAutomatic and
            // unspecified default to auto-capture, matching the Authorize flow.
            capture: Some(request.is_auto_capture()),
            // Not sent: asking for 3DS on a recurring / installment payment is
            // a documented error (ERROR_CREATE_PAYMENT_CUSTOMER_NOT_PRESENT).
            payment_method_options: None,
            merchant_reference_id: Some(
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            description: None,
            error_payment_url: return_url.clone(),
            complete_payment_url: return_url,
            // The stored card id alone identifies the payment. A Rapyd
            // customer id (`cus_…`) is passed through only when the caller
            // supplies one; none is derived or defaulted.
            customer: router_data
                .resource_common_data
                .connector_customer
                .clone()
                .map(RapydCustomerRef::Id),
            save_payment_method: None,
            initiation_type: Some(RapydInitiationType::from_mit_category(
                request.mit_category.as_ref(),
            )),
            address: None,
            statement_descriptor: None,
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
        convert_rapyd_payment_response(
            item.response.status,
            item.response.data,
            item.http_code,
            item.router_data,
            RapydPaymentResponseOptions {
                no_data_is_terminal: true,
                emit_mandate_reference: false,
                emit_network_txn_id: false,
            },
        )
    }
}

// ---------------------------------------------------------------------------
// Incoming webhooks
// ---------------------------------------------------------------------------

/// The JSON webhook secret `{"access_key": "...", "secret_key": "..."}`.
///
/// Rapyd has no dedicated webhook secret: a notification is signed with the
/// merchant's own access key and secret key, so both are supplied as the
/// webhook secret. Deserialize only — it is never sent anywhere.
#[derive(Debug, Clone, Deserialize)]
pub struct RapydWebhookSecret {
    pub access_key: Secret<String>,
    pub secret_key: Secret<String>,
}

/// Envelope of a Rapyd notification. `data` stays raw JSON until the event
/// family of `type` says which object it is.
/// Reference: https://docs.rapyd.net/en/webhook-format.html
#[derive(Debug, Clone, Deserialize)]
pub struct RapydIncomingWebhook {
    pub id: String,
    #[serde(rename = "type")]
    pub webhook_type: RapydWebhookEventType,
    pub data: Secret<serde_json::Value>,
    pub trigger_operation_id: Option<String>,
    /// Lenient: the REFUND_COMPLETED example carries `""`.
    pub status: Option<String>,
    /// Lenient: the REFUND_COMPLETED example carries `0`.
    pub created_at: Option<i64>,
}

/// Rapyd webhook `type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    CustomerPaymentMethodCreated,
    /// Any value outside the documented set.
    #[serde(other)]
    Unknown,
}

/// Which object the `data` of a notification is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RapydWebhookEventFamily {
    Payment,
    Refund,
    Dispute,
    /// Neither a payment, a refund nor a dispute object.
    None,
}

impl RapydWebhookEventType {
    pub fn family(self) -> RapydWebhookEventFamily {
        match self {
            Self::PaymentSucceeded
            | Self::PaymentCompleted
            | Self::PaymentCaptured
            | Self::PaymentFailed
            | Self::PaymentCanceled
            | Self::PaymentExpired
            | Self::PaymentReversed
            | Self::PaymentUpdated => RapydWebhookEventFamily::Payment,
            Self::RefundCompleted
            | Self::PaymentRefundFailed
            | Self::PaymentRefundRejected
            | Self::PaymentRefundUpdated => RapydWebhookEventFamily::Refund,
            Self::PaymentDisputeCreated | Self::PaymentDisputeUpdated => {
                RapydWebhookEventFamily::Dispute
            }
            Self::CardAddedSuccessfully | Self::CustomerPaymentMethodCreated | Self::Unknown => {
                RapydWebhookEventFamily::None
            }
        }
    }
}

/// Rapyd dispute `status`.
/// Reference: https://docs.rapyd.net/en/dispute-created-webhook.html
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RapydDisputeStatus {
    /// Active — awaiting merchant action.
    #[serde(rename = "ACT")]
    Active,
    /// Review — Rapyd is reviewing the merchant's evidence.
    #[serde(rename = "RVW")]
    Review,
    /// Pre-arbitration — the card issuer challenged the dispute.
    #[serde(rename = "PRA")]
    PreArbitration,
    /// Arbitration — awaiting the scheme / arbitration committee ruling.
    #[serde(rename = "ARB")]
    Arbitration,
    /// Loss — the merchant lost; funds deducted.
    #[serde(rename = "LOS")]
    Loss,
    /// Win — the merchant won; funds credited.
    #[serde(rename = "WIN")]
    Win,
    /// Reverse — the issuer reversed the dispute.
    #[serde(rename = "REV")]
    Reverse,
    /// Any value outside the documented set.
    #[serde(other)]
    Unknown,
}

/// The dispute object of `PAYMENT_DISPUTE_CREATED` / `PAYMENT_DISPUTE_UPDATED`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RapydDisputeData {
    /// Dispute id (`dispute_…`).
    pub token: String,
    /// The disputed payment (`payment_…`).
    pub original_transaction_id: String,
    /// Decimal major units (`10` for a USD 10 dispute).
    pub amount: FloatMajorUnit,
    pub currency: common_enums::Currency,
    pub status: RapydDisputeStatus,
    pub dispute_reason_description: Option<String>,
    pub dispute_category: Option<String>,
    pub due_date: Option<i64>,
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
}

/// The parsed `data` of a notification, chosen by the event family of `type`
/// (never by trying one shape after another).
#[derive(Debug, Clone)]
pub enum RapydWebhookData {
    Payment(Box<ResponseData>),
    Refund(RefundResponseData),
    Dispute(RapydDisputeData),
}

/// Rapyd sends `""` for an absent value.
fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.is_empty())
}

impl RapydIncomingWebhook {
    fn parse_data<D: serde::de::DeserializeOwned>(
        self,
    ) -> Result<D, error_stack::Report<WebhookError>> {
        serde_json::from_value(self.data.expose())
            .change_context(WebhookError::WebhookBodyDecodingFailed)
    }

    /// `data` as the object its event family names; `None` for a `type` that
    /// carries neither a payment, a refund nor a dispute.
    pub fn into_data(self) -> Result<Option<RapydWebhookData>, error_stack::Report<WebhookError>> {
        Ok(match self.webhook_type.family() {
            RapydWebhookEventFamily::Payment => {
                Some(RapydWebhookData::Payment(Box::new(self.parse_data()?)))
            }
            RapydWebhookEventFamily::Refund => Some(RapydWebhookData::Refund(self.parse_data()?)),
            RapydWebhookEventFamily::Dispute => Some(RapydWebhookData::Dispute(self.parse_data()?)),
            RapydWebhookEventFamily::None => None,
        })
    }

    /// Event type of the notification.
    /// Reference: https://docs.rapyd.net/en/webhook-format.html
    pub fn event_type(self) -> Result<EventType, error_stack::Report<WebhookError>> {
        Ok(match self.webhook_type {
            // https://docs.rapyd.net/en/payment-completed-webhook.html
            // https://docs.rapyd.net/en/payment-captured-webhook.html
            RapydWebhookEventType::PaymentCompleted | RapydWebhookEventType::PaymentCaptured => {
                EventType::PaymentIntentSuccess
            }
            // https://docs.rapyd.net/en/payment-failed-webhook.html
            RapydWebhookEventType::PaymentFailed => EventType::PaymentIntentFailure,
            // https://docs.rapyd.net/en/refund-completed-webhook.html
            RapydWebhookEventType::RefundCompleted => EventType::RefundSuccess,
            // https://docs.rapyd.net/en/refund-failed-webhook.html
            // https://docs.rapyd.net/en/refund-rejected-webhook.html
            RapydWebhookEventType::PaymentRefundFailed
            | RapydWebhookEventType::PaymentRefundRejected => EventType::RefundFailure,
            // https://docs.rapyd.net/en/dispute-created-webhook.html
            RapydWebhookEventType::PaymentDisputeCreated => EventType::DisputeOpened,
            // The update carries the dispute's current status.
            RapydWebhookEventType::PaymentDisputeUpdated => {
                let dispute: RapydDisputeData = self.parse_data()?;
                match dispute.status {
                    RapydDisputeStatus::Active => EventType::DisputeOpened,
                    RapydDisputeStatus::Review => EventType::DisputeChallenged,
                    RapydDisputeStatus::Loss => EventType::DisputeLost,
                    RapydDisputeStatus::Win => EventType::DisputeWon,
                    RapydDisputeStatus::PreArbitration
                    | RapydDisputeStatus::Arbitration
                    | RapydDisputeStatus::Reverse
                    | RapydDisputeStatus::Unknown => EventType::IncomingWebhookEventUnspecified,
                }
            }
            // Documented, but not a state change this connector reports.
            RapydWebhookEventType::PaymentSucceeded
            | RapydWebhookEventType::PaymentCanceled
            | RapydWebhookEventType::PaymentExpired
            | RapydWebhookEventType::PaymentReversed
            | RapydWebhookEventType::PaymentUpdated
            | RapydWebhookEventType::PaymentRefundUpdated
            | RapydWebhookEventType::CardAddedSuccessfully
            | RapydWebhookEventType::CustomerPaymentMethodCreated
            | RapydWebhookEventType::Unknown => EventType::IncomingWebhookEventUnspecified,
        })
    }

    /// Reference of the object the notification is about.
    pub fn event_reference(
        self,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        Ok(self.into_data()?.map(|data| match data {
            RapydWebhookData::Payment(payment) => {
                WebhookResourceReference::Payment(PaymentWebhookReference {
                    // `data.id` (`payment_…`): the id Authorize returns.
                    connector_transaction_id: Some(payment.id),
                    merchant_transaction_id: non_empty(payment.merchant_reference_id),
                })
            }
            RapydWebhookData::Refund(refund) => {
                WebhookResourceReference::Refund(RefundWebhookReference {
                    connector_refund_id: Some(refund.id),
                    merchant_refund_id: None,
                    connector_transaction_id: Some(refund.payment),
                    merchant_transaction_id: None,
                })
            }
            RapydWebhookData::Dispute(dispute) => {
                // Names the parent payment only: the caller resolves a dispute reference as a
                // payment connector transaction id and prefers `connector_dispute_id` when present.
                WebhookResourceReference::Dispute(DisputeWebhookReference {
                    connector_dispute_id: None,
                    connector_transaction_id: Some(dispute.original_transaction_id),
                })
            }
        }))
    }
}

/// A payment notification as webhook details: the same status, mandate
/// reference, network transaction id and captured amount the payment-object
/// conversion ([`convert_rapyd_payment_response`]) yields.
pub fn build_rapyd_payment_webhook_details(
    data: ResponseData,
    raw_body: &[u8],
) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
    let status = get_status(&data.status, &data.next_action);

    // The stored card id (`card_…`) is the mandate reference for MIT charges.
    let mandate_reference = data
        .payment_method
        .filter(|card_id| !card_id.peek().is_empty())
        .map(|card_id| {
            Box::new(MandateReference {
                connector_mandate_id: Some(card_id.expose()),
                payment_method_id: None,
                connector_mandate_request_reference_id: None,
                mandate_metadata: None,
            })
        });

    let network_txn_id = non_empty(
        data.payment_method_data
            .and_then(|payment_method_data| payment_method_data.network_reference_id)
            .map(ExposeInterface::expose),
    );

    // `data.amount` of a closed payment is the captured amount; reported only
    // when Rapyd names the currency.
    let minor_amount_captured = if data.status == RapydPaymentStatus::Closed {
        data.currency_code
            .map(|currency| {
                domain_types::utils::convert_back_amount_to_minor_units_for_webhook(
                    &FloatMajorUnitForConnector,
                    data.amount,
                    currency,
                )
            })
            .transpose()?
    } else {
        None
    };

    Ok(WebhookDetailsResponse {
        resource_id: Some(ResponseId::ConnectorTransactionId(data.id)),
        status,
        connector_response_reference_id: non_empty(data.merchant_reference_id),
        connector_request_reference_id: None,
        mandate_reference,
        // The issuer code when there is one, else Rapyd's own error code.
        error_code: non_empty(data.failure_code).or_else(|| non_empty(data.error_code)),
        error_message: non_empty(data.failure_message),
        error_reason: None,
        raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
        status_code: 200,
        response_headers: None,
        amount_captured: minor_amount_captured.map(|amount| amount.get_amount_as_i64()),
        minor_amount_captured,
        network_txn_id,
        payment_method_update: None,
        sender_payment_instrument_id: None,
        connector_returned_payment_method_details: None,
    })
}

/// A refund notification as webhook details.
pub fn build_rapyd_refund_webhook_details(
    data: RefundResponseData,
    raw_body: &[u8],
) -> RefundWebhookDetailsResponse {
    RefundWebhookDetailsResponse {
        connector_refund_id: Some(data.id),
        merchant_transaction_id: None,
        status: common_enums::RefundStatus::from(data.status),
        connector_response_reference_id: None,
        error_code: non_empty(data.failure_code),
        error_message: non_empty(data.failure_reason),
        raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
        status_code: 200,
        response_headers: None,
    }
}

/// A dispute notification as webhook details. A dispute status with no
/// counterpart (pre-arbitration, arbitration, reverse, undocumented) is not
/// reported as some other status.
pub fn build_rapyd_dispute_webhook_details(
    data: RapydDisputeData,
    raw_body: &[u8],
) -> Result<DisputeWebhookDetailsResponse, error_stack::Report<WebhookError>> {
    let status = match data.status {
        RapydDisputeStatus::Active => common_enums::DisputeStatus::DisputeOpened,
        RapydDisputeStatus::Review => common_enums::DisputeStatus::DisputeChallenged,
        RapydDisputeStatus::Loss => common_enums::DisputeStatus::DisputeLost,
        RapydDisputeStatus::Win => common_enums::DisputeStatus::DisputeWon,
        RapydDisputeStatus::PreArbitration
        | RapydDisputeStatus::Arbitration
        | RapydDisputeStatus::Reverse
        | RapydDisputeStatus::Unknown => {
            return Err(error_stack::report!(WebhookError::WebhooksNotImplemented {
                operation: "process_dispute_webhook",
            }));
        }
    };

    // The dispute amount is in decimal major units.
    let minor_amount = domain_types::utils::convert_back_amount_to_minor_units_for_webhook(
        &FloatMajorUnitForConnector,
        data.amount,
        data.currency,
    )?;
    let amount = domain_types::utils::convert_amount_for_webhook(
        &StringMinorUnitForConnector,
        minor_amount,
        data.currency,
    )?;

    Ok(DisputeWebhookDetailsResponse {
        amount,
        currency: data.currency,
        dispute_id: data.token,
        status,
        stage: common_enums::DisputeStage::Dispute,
        connector_response_reference_id: Some(data.original_transaction_id),
        dispute_message: data.dispute_reason_description,
        raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
        status_code: 200,
        response_headers: None,
        connector_reason_code: None,
        additional_details: None,
    })
}
