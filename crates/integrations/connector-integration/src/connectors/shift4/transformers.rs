use crate::types::ResponseRouterData;
use common_enums::{AttemptStatus, AuthorizationStatus, Currency, RefundStatus};
use common_utils::{pii, request::Method, types::MinorUnit};
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, CreateConnectorCustomer,
        IncrementalAuthorization, PSync, RSync, Refund, RepeatPayment, SetupMandate, Void,
    },
    connector_types::{
        ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData, ConnectorCustomerData,
        ConnectorCustomerResponse, ConnectorSpecificClientAuthenticationResponse, MandateReference,
        MandateReferenceId, PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsIncrementalAuthorizationData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        RepeatPaymentData, ResponseId, SetupMandateRequestData,
        Shift4ClientAuthenticationResponse as Shift4ClientAuthenticationResponseDomain,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_address,
    payment_method_data::{
        BankRedirectData, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber,
    },
    router_data::{
        AdditionalPaymentMethodConnectorResponse, ConnectorResponseData, ConnectorSpecificConfig,
        FlowStatus,
    },
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeOptionInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use url::Url;

// Import the connector's RouterData wrapper type created by the macro
use super::Shift4RouterData;
use domain_types::errors::{ConnectorError, IntegrationError, IntegrationErrorContext};

/// Shift4's refund object has no failure fields, so a declined authorization
/// release surfaces only as `status: "failed"`. This spells out what actually
/// happened instead of letting the caller see a bare "declined by shift4".
const SHIFT4_VOID_DECLINED: &str =
    "Shift4 declined the authorization release (refund of the uncaptured charge reported `failed`)";

#[derive(Debug, Clone)]
pub struct Shift4AuthType {
    pub api_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for Shift4AuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Shift4 { api_key, .. } => Ok(Self {
                api_key: api_key.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shift4ErrorResponse {
    pub error: ApiErrorResponse,
}

/// Class of the Shift4 error envelope (`error.type`).
///
/// Shift4 documents exactly four values. `Unknown` keeps an unrecognised value
/// from failing the whole error-body parse — an unparsable error body would
/// otherwise surface as a deserialization failure instead of the decline the
/// merchant needs to see.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Shift4ErrorType {
    InvalidRequest,
    CardError,
    GatewayError,
    RateLimitError,
    #[serde(other)]
    Unknown,
}

/// Shift4's declined/failed error envelope (`HTTP 402` for card declines, `4xx`
/// otherwise): `{ "error": { type, code, message, issuerDeclineCode, adviceCode,
/// networkAdviceCode, chargeId, ... } }`.
///
/// **The wire encoding is camelCase**, so `rename_all = "camelCase"` is load
/// bearing: the reference Hyperswitch struct carries `#[serde(rename = "camelCase")]`,
/// which is a no-op (`rename`, not `rename_all`) and silently deserializes every
/// network field as `None`. Do not "simplify" this attribute back.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiErrorResponse {
    #[serde(rename = "type")]
    pub error_type: Option<Shift4ErrorType>,
    pub code: Option<String>,
    pub message: String,
    /// Decline code supplied by the card issuer. Declined charges only.
    /// Maps to `ErrorResponse::network_decline_code`.
    pub issuer_decline_code: Option<String>,
    /// Merchant advice code (MAC). Shift4 documents exactly two states:
    /// absent/`null`, or `do_not_try_again` (a hard stop for smart retry).
    /// Maps to `ErrorResponse::network_error_message`.
    pub advice_code: Option<String>,
    /// Raw advice code from the issuer / card network. Shift4 publishes no
    /// value table for it — it is pass-through.
    /// Maps to `ErrorResponse::network_advice_code`.
    pub network_advice_code: Option<String>,
    /// Charge the decline belongs to. Maps to `ErrorResponse::connector_transaction_id`.
    pub charge_id: Option<String>,
}

// ===== CREATE CUSTOMER FLOW STRUCTURES =====

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4CreateCustomerRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<pii::Email>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Shift4CreateCustomerResponse {
    pub id: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        Shift4RouterData<
            RouterDataV2<
                CreateConnectorCustomer,
                PaymentFlowData,
                ConnectorCustomerData,
                ConnectorCustomerResponse,
            >,
            T,
        >,
    > for Shift4CreateCustomerRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: Shift4RouterData<
            RouterDataV2<
                CreateConnectorCustomer,
                PaymentFlowData,
                ConnectorCustomerData,
                ConnectorCustomerResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            email: item.router_data.request.email.clone().expose_option(),
            description: item.router_data.request.description.clone(),
        })
    }
}

impl<F, T> TryFrom<ResponseRouterData<Shift4CreateCustomerResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, ConnectorCustomerResponse>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<Shift4CreateCustomerResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(ConnectorCustomerResponse {
                connector_customer_id: item.response.id,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// ===== AUTHORIZE FLOW STRUCTURES =====

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4PaymentsRequest<T: PaymentMethodDataTypes> {
    pub amount: MinorUnit,
    pub currency: Currency,
    pub captured: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    /// Customer ID required when charging a stored card token
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_id: Option<String>,
    /// Optional charge options. When incremental authorization is requested, this
    /// must include `authorizationType = "pre"` so that the charge is created as
    /// a pre-authorization eligible for future `POST /charges/{id}/increment-authorization`
    /// calls. Shift4 requires BOTH `captured=false` AND `options.authorizationType=pre`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Shift4ChargeOptions>,
    /// Transaction initiation class. Shift4 documents no default, so an untyped
    /// charge is classified by the scheme rather than by us. A one-off sale is
    /// `customer_initiated`; a charge that is also storing the card on file for
    /// later merchant-initiated use is `first_recurring`.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub transaction_type: Option<Shift4TransactionType>,
    /// Charge-level billed-party details. Distinct from the flat `card.address*`
    /// fields (which take precedence for AVS): this is what Shift4 echoes on the
    /// charge object and shows in the dashboard. Note `billing.address.country`
    /// is **two-letter** ISO while `card.addressCountry` is three-letter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing: Option<Shift4Billing>,
    /// Recipient and delivery address.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipping: Option<Shift4Shipping>,
    /// Carries the merchant reference. `external.vendorReference` is the only
    /// merchant-controlled reference identifier on `POST /charges` — Shift4 has
    /// no `orderId` / `invoice` / `merchantReference` field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external: Option<Shift4External>,
    #[serde(flatten)]
    pub payment_method: Shift4PaymentMethod<T>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4ChargeOptions {
    /// "pre" to mark the charge as a pre-authorization (required for incremental auth).
    pub authorization_type: Shift4AuthorizationType,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Shift4AuthorizationType {
    Pre,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Shift4PaymentMethod<T: PaymentMethodDataTypes> {
    Card(Shift4CardPayment<T>),
    TokenPayment(Shift4TokenPayment),
    BankRedirect(Shift4BankRedirectPayment),
}

/// Token-based payment — the `card` field carries a token ID from Shift4 Components SDK
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4TokenPayment {
    pub card: Secret<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4CardPayment<T: PaymentMethodDataTypes> {
    pub card: Shift4CardData<T>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4CardData<T: PaymentMethodDataTypes> {
    pub number: RawCardNumber<T>,
    pub exp_month: Secret<String>,
    pub exp_year: Secret<String>,
    /// Card security code. Shift4 only runs the CVV check when this is present —
    /// omitting it forces `cvvCheck.result` to `not_provided` on every sale.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cvc: Option<Secret<String>>,
    /// Optional for a normal charge; **required** for the zero-amount Account
    /// Name Inquiry (ANI) path, which is enforced in the request builder.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cardholder_name: Option<Secret<String>>,
    /// Flat card-level address. This is the highest-precedence AVS source in
    /// Shift4's documented lookup order (card address -> charge `billing`
    /// address -> customer address -> payment-method billing address).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address_line1: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address_line2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address_city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address_state: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address_zip: Option<Secret<String>>,
    /// **Three-letter** ISO country code — deliberately different from
    /// `billing.address.country`, which Shift4 documents as two-letter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address_country: Option<common_enums::CountryAlpha3>,
}

impl<T: PaymentMethodDataTypes> Shift4CardData<T> {
    /// Build the inline `card` object from UCS card data plus the billing
    /// address that drives AVS. `cardholder_name` is passed in because its
    /// requiredness is amount-dependent (see the ANI rules on `amount == 0`).
    fn new(
        card_data: &domain_types::payment_method_data::Card<T>,
        cardholder_name: Option<Secret<String>>,
        billing_address: Option<&payment_address::AddressDetails>,
    ) -> Self {
        Self {
            number: card_data.card_number.clone(),
            exp_month: card_data.card_exp_month.clone(),
            exp_year: card_data.card_exp_year.clone(),
            cvc: Some(card_data.card_cvc.clone()),
            cardholder_name,
            address_line1: billing_address.and_then(|addr| addr.line1.clone()),
            address_line2: billing_address.and_then(|addr| addr.line2.clone()),
            address_city: billing_address.and_then(|addr| addr.city.clone()),
            address_state: billing_address.and_then(|addr| addr.state.clone()),
            address_zip: billing_address.and_then(|addr| addr.zip.clone()),
            address_country: billing_address
                .and_then(|addr| addr.country)
                .map(common_enums::CountryAlpha2::from_alpha2_to_alpha3),
        }
    }
}

// BankRedirect Payment Structures
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4BankRedirectPayment {
    pub payment_method: Shift4BankRedirectMethod,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow: Option<Shift4FlowRequest>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4FlowRequest {
    pub return_url: String,
}

#[derive(Debug, Serialize)]
pub struct Shift4BankRedirectMethod {
    #[serde(rename = "type")]
    pub payment_type: String,
    pub billing: Shift4Billing,
}

#[derive(Debug, Serialize)]
pub struct Shift4Billing {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<pii::Email>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<Secret<String>>,
    /// Tax identification number. Shift4 documents this as the billed party's
    /// VAT id; the Boleto APM repurposes it for the payer's social security
    /// number, which is why it is not restricted to bank-redirect payloads.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vat: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<Shift4Address>,
}

/// Recipient and delivery address on `POST /charges`. Shares `Shift4Address`
/// with `billing` — Shift4 documents the same six sub-fields for both.
#[derive(Debug, Serialize)]
pub struct Shift4Shipping {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<Shift4Address>,
}

#[derive(Debug, Serialize)]
pub struct Shift4Address {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line1: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zip: Option<Secret<String>>,
    /// **Two-letter** ISO country code (contrast `card.addressCountry`, which
    /// Shift4 documents as three-letter).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<common_enums::CountryAlpha2>,
}

impl Shift4Address {
    /// `None` when the address carries no field Shift4 accepts, so an empty
    /// `{}` object is never serialized onto the charge.
    fn from_address_details(address: Option<&payment_address::AddressDetails>) -> Option<Self> {
        let address = address?;
        let built = Self {
            line1: address.line1.clone(),
            line2: address.line2.clone(),
            city: address.city.clone(),
            state: address.state.clone(),
            zip: address.zip.clone(),
            country: address.country,
        };
        (built.line1.is_some()
            || built.line2.is_some()
            || built.city.is_some()
            || built.state.is_some()
            || built.zip.is_some()
            || built.country.is_some())
        .then_some(built)
    }
}

/// `external` object on `POST /charges`.
///
/// `vendorReference` is Shift4's only merchant-controlled reference identifier —
/// there is no `orderId`, `invoice`, `merchantReference` or `referenceId` field
/// anywhere in the api.shift4.com reference, so do not invent one.
/// `schemeTransactionId` is an inbound-from-scheme value used for MIT continuity,
/// not a merchant order id.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4External {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vendor_reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheme_transaction_id: Option<String>,
}

/// Wrapper for Shift4's three verification-check objects on the charge:
/// `avsCheck`, `cvvCheck` and `aniCheck`, each `{ "result": "<value>" }`.
///
/// `result` is deliberately a `String` rather than an enum: it is a
/// pass-through diagnostic that is copied verbatim into `payment_checks` for
/// the merchant and never drives payment status, so a value Shift4 adds later
/// must survive intact rather than collapse onto a catch-all variant.
/// Documented values today — AVS: `full_match`, `partial_match`, `no_match`,
/// `not_provided`, `unavailable`; CVV: `match`, `no_match`, `not_verified`,
/// `not_provided`, `not_supported`; ANI: `full_match`, `partial_match`,
/// `no_match`, `not_verified`, `not_supported`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Shift4CheckResult {
    pub result: Option<String>,
}

/// Shift4 documents `billing.phone` / `shipping.phone` with the pattern
/// `+12 345678901`, i.e. dialling code, a space, then the subscriber number.
/// UCS keeps the two apart, so they are rejoined here; a number with no dialling
/// code is sent as-is rather than guessed at.
fn build_shift4_phone(address: Option<&payment_address::Address>) -> Option<Secret<String>> {
    let number = address.and_then(|a| a.get_optional_phone_number())?;
    match address.and_then(|a| a.get_optional_phone_country_code()) {
        Some(country_code) => Some(Secret::new(format!(
            "{} {}",
            country_code,
            number.peek().trim()
        ))),
        None => Some(number),
    }
}

/// Build the charge-level `billing` object.
///
/// Returns `None` when nothing billable is known, so an empty `billing: {}` is
/// never put on the wire. `email` prefers the request-level email and falls back
/// to the address-level one, which is how the bank-redirect path already behaved.
/// `vat` is left unset: Shift4 documents it as a tax identification number and
/// UCS has no generic carrier for one on the Authorize contract (the Boleto APM,
/// which repurposes it, is not supported by this connector).
fn build_shift4_billing(
    billing: Option<&payment_address::Address>,
    request_email: Option<&pii::Email>,
) -> Option<Shift4Billing> {
    let name = billing.and_then(|b| b.get_optional_full_name());
    let email = request_email
        .cloned()
        .or_else(|| billing.and_then(|b| b.email.clone()));
    let phone = build_shift4_phone(billing);
    let address = Shift4Address::from_address_details(billing.and_then(|b| b.address.as_ref()));

    if name.is_none() && email.is_none() && phone.is_none() && address.is_none() {
        return None;
    }
    Some(Shift4Billing {
        name,
        email,
        phone,
        vat: None,
        address,
    })
}

/// Build the charge-level `shipping` object from the UCS shipping address.
/// `None` when no shipping detail is present, so no empty object is serialized.
fn build_shift4_shipping(shipping: Option<&payment_address::Address>) -> Option<Shift4Shipping> {
    let name = shipping.and_then(|s| s.get_optional_full_name());
    let phone = build_shift4_phone(shipping);
    let address = Shift4Address::from_address_details(shipping.and_then(|s| s.address.as_ref()));

    if name.is_none() && phone.is_none() && address.is_none() {
        return None;
    }
    Some(Shift4Shipping {
        name,
        phone,
        address,
    })
}

// BankRedirect Data Transformation
impl<T: PaymentMethodDataTypes>
    TryFrom<
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    > for Shift4BankRedirectMethod
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        router_data: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        let payment_type = match &router_data.request.payment_method_data {
            PaymentMethodData::BankRedirect(bank_redirect_data) => match bank_redirect_data {
                BankRedirectData::Ideal { .. } => "ideal",
                BankRedirectData::Eps { .. } => "eps",
                _ => {
                    return Err(error_stack::report!(IntegrationError::NotSupported {
                        message: format!(
                            "BankRedirect type {:?} is not supported by Shift4",
                            bank_redirect_data
                        ),
                        connector: "Shift4",
                        context: Default::default()
                    }))
                }
            },
            _ => {
                return Err(error_stack::report!(IntegrationError::NotSupported {
                    message: "Non-bank redirect payment method".to_string(),
                    connector: "Shift4",
                    context: Default::default()
                }))
            }
        };

        let billing_info = build_shift4_billing(
            router_data
                .resource_common_data
                .address
                .get_payment_method_billing(),
            router_data.request.email.as_ref(),
        )
        .unwrap_or(Shift4Billing {
            name: None,
            email: None,
            phone: None,
            vat: None,
            address: None,
        });

        Ok(Self {
            payment_type: payment_type.to_string(),
            billing: billing_info,
        })
    }
}

impl<T: PaymentMethodDataTypes>
    TryFrom<
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    > for Shift4PaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        let is_zero_amount = item.request.minor_amount == MinorUnit::new(0);
        // Shift4 rejects a zero-amount charge that asks to be captured with
        // HTTP 400 `{"type":"invalid_request","message":"Zero amount charge cannot
        // be captured"}` (verified against api.shift4.com). A zero-amount charge is
        // an Account Name Inquiry — no funds move either way, so `captured` carries
        // no merchant intent here, and honouring AUTOMATIC would make the documented
        // verification path fail outright instead of returning `aniCheck`.
        let captured = !is_zero_amount && item.request.is_auto_capture();
        let billing_details = item
            .resource_common_data
            .address
            .get_payment_method_billing();

        let payment_method = match &item.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                // Shift4 documents `cardholderName` as OPTIONAL for a normal charge,
                // so a card without a billing name must not fail locally. The one
                // exception is the zero-amount Account Name Inquiry path, where the
                // name IS the thing being verified.
                let cardholder_name = billing_details
                    .and_then(|billing| billing.get_optional_full_name())
                    .or_else(|| {
                        item.request
                            .customer_name
                            .as_ref()
                            .map(|name| Secret::new(name.clone()))
                    });

                if is_zero_amount && cardholder_name.is_none() {
                    return Err(error_stack::report!(
                        IntegrationError::MissingRequiredField {
                            field_name: "card.cardholderName",
                            context: IntegrationErrorContext {
                                additional_context: Some(
                                    "A zero-amount Shift4 charge is an Account Name Inquiry (ANI): \
                                     Shift4 verifies `card.cardholderName` against the name on the \
                                     account and returns the outcome in `aniCheck.result`. Without \
                                     a name there is nothing to verify and Shift4 rejects the charge."
                                        .to_string(),
                                ),
                                suggested_action: Some(
                                    "Send a billing first/last name (or a customer name) with the \
                                     zero-amount request, or use a non-zero amount."
                                        .to_string(),
                                ),
                                doc_url: Some(
                                    "https://dev.shift4.com/docs/fraud-prevention/verification-checks"
                                        .to_string(),
                                ),
                            },
                        }
                    ));
                }

                Shift4PaymentMethod::Card(Shift4CardPayment {
                    card: Shift4CardData::new(
                        card_data,
                        cardholder_name,
                        billing_details.and_then(|billing| billing.address.as_ref()),
                    ),
                })
            }
            PaymentMethodData::PaymentMethodToken(pmt) => {
                Shift4PaymentMethod::TokenPayment(Shift4TokenPayment {
                    card: pmt.token.clone(),
                })
            }
            PaymentMethodData::BankRedirect(_bank_redirect_data) => {
                let bank_redirect_method = Shift4BankRedirectMethod::try_from(item)?;
                let return_url = item.request.get_router_return_url().change_context(
                    IntegrationError::MissingRequiredField {
                        field_name: "return_url",
                        context: Default::default(),
                    },
                )?;

                Shift4PaymentMethod::BankRedirect(Shift4BankRedirectPayment {
                    payment_method: bank_redirect_method,
                    flow: Some(Shift4FlowRequest { return_url }),
                })
            }
            _ => {
                return Err(IntegrationError::NotImplemented(
                    "Payment method".to_string(),
                    Default::default(),
                )
                .into());
            }
        };

        // Get customer_id from connector_customer if available (needed for token payments)
        let customer_id = item.resource_common_data.connector_customer.clone();

        // When the upstream requests incremental authorization support, Shift4 requires
        // the original charge to be created as a pre-authorization: `captured=false` AND
        // `options.authorizationType=pre`. Fail fast at authorize time if the caller
        // asked for incremental auth under AUTOMATIC capture — otherwise the mismatch
        // would only surface later at increment time with an opaque Shift4 rejection.
        let wants_incremental_auth =
            matches!(item.request.request_incremental_authorization, Some(true));
        if wants_incremental_auth && captured {
            return Err(IntegrationError::InvalidDataFormat {
                field_name: "capture_method",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Shift4 incremental authorization requires the parent charge to be a \
                         pre-authorization (captured=false, options.authorizationType=\"pre\"). \
                         The caller sent request_incremental_authorization=true with \
                         capture_method=AUTOMATIC, which would create a captured sale and cause \
                         POST /charges/{id}/incremental-authorization to fail with HTTP 400."
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Set capture_method=MANUAL when request_incremental_authorization=true, \
                         or drop request_incremental_authorization if a normal auto-capture sale \
                         is intended."
                            .to_string(),
                    ),
                    doc_url: Some(
                        "https://dev.shift4.com/docs/api#increment-authorization".to_string(),
                    ),
                },
            }
            .into());
        }
        let options = if wants_incremental_auth {
            Some(Shift4ChargeOptions {
                authorization_type: Shift4AuthorizationType::Pre,
            })
        } else {
            None
        };

        // Shift4 documents no default for `type`, so classify explicitly: a charge
        // that is also storing the card on file for later merchant-initiated use is
        // `first_recurring`; every other Authorize is a plain `customer_initiated`
        // sale. (`merchant_initiated` / `subsequent_recurring` belong to
        // `Shift4RepeatPaymentRequest` and are set there.)
        let transaction_type = if item.request.is_customer_initiated_mandate_payment() {
            Shift4TransactionType::FirstRecurring
        } else {
            Shift4TransactionType::CustomerInitiated
        };

        // NOT SUPPORTED BY SHIFT4, deliberately dropped rather than approximated:
        // * `PaymentFlowData::l2_l3_data` — api.shift4.com publishes no Level 2 /
        //   Level 3 fields at all (no purchase order number, tax amount, duty,
        //   freight, line items or commodity codes). The Level-2 capability on
        //   docs.shift4.com belongs to a different product with a different wire
        //   format and is not reachable through this API.
        // * `PaymentsAuthorizeData::billing_descriptor` — there is no
        //   `statementDescriptor` / `dynamicDescriptor` / `softDescriptor` field.
        //   `description` is an internal charge description that never reaches the
        //   cardholder's statement, so aliasing the descriptor onto it would
        //   silently change its meaning.
        Ok(Self {
            amount: item.request.minor_amount,
            currency: item.request.currency,
            captured,
            description: item.resource_common_data.description.clone(),
            metadata: item.request.metadata.clone().expose_option(),
            customer_id,
            options,
            transaction_type: Some(transaction_type),
            billing: build_shift4_billing(billing_details, item.request.email.as_ref()),
            shipping: build_shift4_shipping(item.resource_common_data.address.get_shipping()),
            external: Some(Shift4External {
                vendor_reference: Some(
                    item.resource_common_data
                        .connector_request_reference_id
                        .clone(),
                ),
                // Inbound-from-scheme continuity value; only meaningful on a
                // follow-up MIT, which is `Shift4RepeatPaymentRequest`, not here.
                scheme_transaction_id: None,
            }),
            payment_method,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4PaymentsResponse {
    pub id: String,
    pub currency: Currency,
    pub amount: MinorUnit,
    pub status: Shift4PaymentStatus,
    pub captured: bool,
    pub refunded: bool,
    pub flow: Option<FlowResponse>,
    /// Alphanumeric authorization code from the issuing bank.
    pub auth_code: Option<String>,
    /// Unique reference for this transaction within the card network. This is
    /// the value a subsequent MIT / recurring charge must quote, so it is
    /// surfaced as `network_txn_id`.
    pub scheme_transaction_id: Option<String>,
    /// Address Verification Service outcome. Absent unless Shift4 has activated
    /// AVS on the merchant account — absence means "check not performed", never
    /// a failure.
    pub avs_check: Option<Shift4CheckResult>,
    /// Card security code check outcome. Only ever populated when `card.cvc` was
    /// sent on the request. A charge may succeed even when this is `no_match`,
    /// so it must never be used to derive payment status.
    pub cvv_check: Option<Shift4CheckResult>,
    /// Account Name Inquiry outcome. Shift4 documents ANI as working only on a
    /// zero-amount charge and only on an account where it has been activated.
    pub ani_check: Option<Shift4CheckResult>,
    /// Nested stored-card object — Shift4 returns this on any /charges
    /// success. Its `id` (e.g., `card_xxx`) is the token used for
    /// subsequent RepeatPayment / MIT calls.
    pub card: Option<Shift4ResponseCard>,
    /// Nested customer object — present when a customerId was supplied
    /// or created during the charge. Required alongside a stored card
    /// id for MIT charges.
    pub customer: Option<Shift4ResponseCustomer>,
    /// Populated by Shift4 on declined / failed charges (e.g.,
    /// `"card_declined"`). Surfaced as the ErrorResponse `code`.
    pub failure_code: Option<String>,
    /// Populated by Shift4 on declined / failed charges (e.g.,
    /// `"Your card was declined."`). Surfaced as the ErrorResponse
    /// `message` and `reason`.
    pub failure_message: Option<String>,
    /// Charge-object twin of the error envelope's `issuerDeclineCode`. Shift4
    /// exposes the same decline data under two different names depending on
    /// whether the decline arrives as an HTTP 402 envelope or as an HTTP 200
    /// charge with `status: "failed"`. Maps to `network_decline_code`.
    pub failure_issuer_decline_code: Option<String>,
    /// Merchant advice code (MAC) — `null` or `do_not_try_again`. Charge-object
    /// twin of `error.adviceCode`. Maps to `network_error_message`.
    pub advice_code: Option<String>,
    /// Raw scheme advice code, pass-through with no published value table.
    /// Charge-object twin of `error.networkAdviceCode`. Maps to `network_advice_code`.
    pub network_advice_code: Option<String>,
}

/// The single Shift4 charge-status table, shared by every flow that reads a
/// charge object (Authorize, PSync, Capture, RepeatPayment, SetupMandate).
///
/// Shift4 reports only three charge states, so terminality has to come from
/// `captured` and, for `pending`, from `flow.nextAction`:
/// a settled sale is `Charged`, an uncaptured authorization is `Authorized`, an
/// uncaptured-but-refunded charge is a released authorization (`Voided`), and
/// a `pending` charge awaiting a shopper redirect is `AuthenticationPending`
/// rather than plain `Pending`.
fn get_shift4_attempt_status(response: &Shift4PaymentsResponse) -> AttemptStatus {
    match response.status {
        Shift4PaymentStatus::Successful => {
            if response.captured {
                AttemptStatus::Charged
            } else if response.refunded {
                // Shift4 has no cancel endpoint: an authorization is released by
                // refunding the uncaptured charge, so `!captured && refunded` is
                // a completed VOID. Without this arm a voided charge keeps
                // reporting `Authorized` and the caller polls it forever.
                AttemptStatus::Voided
            } else {
                AttemptStatus::Authorized
            }
        }
        Shift4PaymentStatus::Failed => AttemptStatus::Failure,
        Shift4PaymentStatus::Pending => match response
            .flow
            .as_ref()
            .and_then(|flow| flow.next_action.as_ref())
        {
            Some(NextAction::Redirect) => AttemptStatus::AuthenticationPending,
            Some(NextAction::Wait) | Some(NextAction::None) | None => AttemptStatus::Pending,
        },
        Shift4PaymentStatus::Unknown => AttemptStatus::Unresolved,
    }
}

/// Redirect target Shift4 hands back for APM / redirect flows, if any.
fn get_shift4_redirection_data(response: &Shift4PaymentsResponse) -> Option<Box<RedirectForm>> {
    response
        .flow
        .as_ref()
        .and_then(|flow| flow.redirect.as_ref())
        .and_then(|redirect| {
            Url::parse(&redirect.redirect_url)
                .ok()
                .map(|url| Box::new(RedirectForm::from((url, Method::Get))))
        })
}

/// Fold Shift4's three verification checks and the issuer auth code into the
/// only UCS carrier that exists for them: `payment_checks` on
/// `AdditionalPaymentMethodConnectorResponse::Card`, which reaches callers as
/// `CardConnectorResponse.payment_checks`. There is no dedicated `avs_result`
/// field on the UCS response types.
///
/// AVS and ANI both require Shift4 to activate them on the merchant account; on
/// an account without them the objects are simply absent, which is reported as
/// "check not performed" (the key is omitted) rather than as a failure.
fn build_shift4_connector_response(
    response: &Shift4PaymentsResponse,
) -> Option<ConnectorResponseData> {
    let mut payment_checks = serde_json::Map::new();
    if let Some(result) = response.avs_check.as_ref().and_then(|c| c.result.as_ref()) {
        payment_checks.insert("avs_result".to_string(), serde_json::json!(result));
    }
    if let Some(result) = response.cvv_check.as_ref().and_then(|c| c.result.as_ref()) {
        payment_checks.insert(
            "card_validation_result".to_string(),
            serde_json::json!(result),
        );
    }
    if let Some(result) = response.ani_check.as_ref().and_then(|c| c.result.as_ref()) {
        payment_checks.insert("ani_result".to_string(), serde_json::json!(result));
    }

    if payment_checks.is_empty() && response.auth_code.is_none() {
        return None;
    }

    Some(ConnectorResponseData::with_additional_payment_method_data(
        AdditionalPaymentMethodConnectorResponse::Card {
            authentication_data: None,
            payment_checks: (!payment_checks.is_empty())
                .then(|| serde_json::Value::Object(payment_checks)),
            card_network: None,
            domestic_network: None,
            auth_code: response.auth_code.clone(),
        },
    ))
}

/// Build the `ErrorResponse` for an HTTP-200 charge body carrying
/// `status: "failed"` — Shift4's "soft decline" shape.
///
/// The decline data lives under charge-object names (`failureCode`,
/// `failureIssuerDeclineCode`, `adviceCode`, `networkAdviceCode`) rather than
/// the error-envelope names, but maps onto the same UCS fields, so smart retry
/// and the GSM tables see the same values on both decline paths.
///
/// `flow_status` is passed in because terminality is flow-specific: an Authorize
/// decline is `Failure`, a Capture decline is `CaptureFailed`.
fn build_shift4_failure_response(
    response: &Shift4PaymentsResponse,
    http_code: u16,
    flow_status: FlowStatus,
) -> domain_types::router_data::ErrorResponse {
    domain_types::router_data::ErrorResponse {
        status_code: http_code,
        code: response
            .failure_code
            .clone()
            .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
        message: response
            .failure_message
            .clone()
            .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
        reason: response.failure_message.clone(),
        attempt_status: Some(flow_status),
        connector_transaction_id: Some(response.id.clone()),
        network_decline_code: response.failure_issuer_decline_code.clone(),
        network_advice_code: response.network_advice_code.clone(),
        network_error_message: response.advice_code.clone(),
        ..Default::default()
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Shift4ResponseCard {
    pub id: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Shift4ResponseCustomer {
    Id(String),
    Object { id: String },
}

impl Shift4ResponseCustomer {
    pub fn id(&self) -> &str {
        match self {
            Self::Id(s) => s.as_str(),
            Self::Object { id } => id.as_str(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowResponse {
    pub next_action: Option<NextAction>,
    pub redirect: Option<RedirectResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedirectResponse {
    pub redirect_url: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NextAction {
    Redirect,
    Wait,
    /// Also the landing spot for any `nextAction` Shift4 adds later: an
    /// unrecognised next action means "we do not know of an action to take",
    /// which is exactly `none`, and must not fail the response parse.
    #[serde(other)]
    None,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Shift4PaymentStatus {
    Successful,
    Pending,
    Failed,
    /// A charge status Shift4 has added since this integration was written.
    /// Parsing it instead of failing keeps the charge id recoverable; it is
    /// mapped to `Unresolved`, never to `Failure`, because an unknown state is
    /// not evidence that the money did not move.
    #[serde(other)]
    Unknown,
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<Shift4PaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<Shift4PaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = get_shift4_attempt_status(&item.response);
        let connector_response = build_shift4_connector_response(&item.response);

        // A Shift4 decline can arrive as an HTTP 200 whose charge body says
        // `status: "failed"`. Returning `Ok` there would strand the decline —
        // `failureCode`, `failureMessage` and the three network codes would never
        // reach the caller — so route it through the error channel, exactly as the
        // SetupMandate and IncrementalAuthorization mappers already do.
        let response = if matches!(item.response.status, Shift4PaymentStatus::Failed) {
            Err(build_shift4_failure_response(
                &item.response,
                item.http_code,
                FlowStatus::Payment(status),
            ))
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: get_shift4_redirection_data(&item.response),
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: item.response.scheme_transaction_id.clone(),
                network_txn_link_id: None,
                connector_response_reference_id: Some(item.response.id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            })
        };

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                // AVS / CVV / ANI results are reported on both the success and the
                // decline path — a declined charge is precisely when the merchant
                // needs to know the AVS or CVV verdict.
                connector_response,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// PSync response transformation - reuses Shift4PaymentsResponse and status mapping logic
impl TryFrom<ResponseRouterData<Shift4PaymentsResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<Shift4PaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = get_shift4_attempt_status(&item.response);
        let connector_response = build_shift4_connector_response(&item.response);

        let response = if matches!(item.response.status, Shift4PaymentStatus::Failed) {
            Err(build_shift4_failure_response(
                &item.response,
                item.http_code,
                FlowStatus::Payment(status),
            ))
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: item.response.scheme_transaction_id.clone(),
                network_txn_link_id: None,
                connector_response_reference_id: Some(item.response.id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            })
        };

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                connector_response,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// Capture response transformation - reuses Shift4PaymentsResponse
impl TryFrom<ResponseRouterData<Shift4PaymentsResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<Shift4PaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = get_shift4_attempt_status(&item.response);
        let connector_response = build_shift4_connector_response(&item.response);

        // A capture that Shift4 reports as `failed` is terminal for the capture
        // leg specifically, so the flow status is `CaptureFailed`, not the
        // Authorize-level `Failure`.
        let response = if matches!(item.response.status, Shift4PaymentStatus::Failed) {
            Err(build_shift4_failure_response(
                &item.response,
                item.http_code,
                FlowStatus::Payment(AttemptStatus::CaptureFailed),
            ))
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: item.response.scheme_transaction_id.clone(),
                network_txn_link_id: None,
                connector_response_reference_id: Some(item.response.id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            })
        };

        let status = if matches!(item.response.status, Shift4PaymentStatus::Failed) {
            AttemptStatus::CaptureFailed
        } else {
            status
        };

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                connector_response,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== REFUND FLOW STRUCTURES =====

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4RefundRequest {
    pub charge_id: String,
    pub amount: MinorUnit,
}

impl TryFrom<&RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>>
    for Shift4RefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            charge_id: item.request.connector_transaction_id.clone(),
            amount: item.request.minor_refund_amount,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Shift4RefundResponse {
    pub id: String,
    pub amount: MinorUnit,
    pub currency: Currency,
    pub charge: String,
    pub status: Shift4RefundStatus,
}

/// Shift4's refund-object status, shared by Refund, RSync and Void (Shift4
/// releases an authorization by refunding the uncaptured charge, so a void
/// response *is* a refund object).
///
/// The public API reference documents only `successful` and `failed`, but the
/// official Shift4 SDKs also emit `pending` for an in-flight refund, and
/// `processing` is the spelling this connector was originally written against.
/// Both in-flight spellings are accepted so neither can fail deserialization,
/// and anything Shift4 adds later lands on `Unknown` instead of failing the
/// whole response parse.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Shift4RefundStatus {
    Successful,
    Failed,
    /// In flight. `pending` is what the SDKs emit; `processing` is kept as the
    /// variant name for compatibility and `pending` is accepted as an alias.
    #[serde(alias = "pending")]
    Processing,
    #[serde(other)]
    Unknown,
}

impl TryFrom<ResponseRouterData<Shift4RefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<Shift4RefundResponse, Self>) -> Result<Self, Self::Error> {
        // CRITICAL: Explicitly check the status field from the response
        // Do NOT assume success based solely on HTTP 200 response
        let refund_status = match item.response.status {
            Shift4RefundStatus::Successful => RefundStatus::Success,
            Shift4RefundStatus::Failed => RefundStatus::Failure,
            Shift4RefundStatus::Processing => RefundStatus::Pending,
            // Never invent a terminal state for a status Shift4 has not
            // documented. NOT `RefundStatus::Unknown` — that serializes to the
            // proto's Unspecified, on which the caller falls back to the
            // previously stored status, which is silently misleading. `Pending`
            // keeps RSync polling until Shift4 reports something we understand.
            Shift4RefundStatus::Unknown => RefundStatus::Pending,
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            ..item.router_data
        })
    }
}

// RSync (Refund Sync) response transformation - reuses Shift4RefundResponse
impl TryFrom<ResponseRouterData<Shift4RefundResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<Shift4RefundResponse, Self>) -> Result<Self, Self::Error> {
        // CRITICAL: Explicitly check the status field from the response
        // Do NOT assume success based solely on HTTP 200 response
        let refund_status = match item.response.status {
            Shift4RefundStatus::Successful => RefundStatus::Success,
            Shift4RefundStatus::Failed => RefundStatus::Failure,
            Shift4RefundStatus::Processing => RefundStatus::Pending,
            // Never invent a terminal state for a status Shift4 has not
            // documented. NOT `RefundStatus::Unknown` — that serializes to the
            // proto's Unspecified, on which the caller falls back to the
            // previously stored status, which is silently misleading. `Pending`
            // keeps RSync polling until Shift4 reports something we understand.
            Shift4RefundStatus::Unknown => RefundStatus::Pending,
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            ..item.router_data
        })
    }
}

// ===== VOID FLOW STRUCTURES =====

/// Shift4 exposes **no** cancel / void / reverse / release endpoint — every such
/// URL 404s (verified against the live sandbox, 2026-09-10). The documented and
/// only way to release an open authorization is to refund the uncaptured charge:
/// `POST /refunds` with just `chargeId`, and **no** `amount`, so Shift4 releases
/// the full authorized amount (<https://dev.shift4.com/docs/api#refunds>).
///
/// `amount` is deliberately absent from this struct: a void always releases the
/// whole authorization, and sending a partial amount would leave the remainder
/// authorized while the caller believes the payment was cancelled. Shift4's own
/// documentation agrees — "Partial refunds are only possible for captured
/// charge" — so a full release is the only permitted operation here.
///
/// `PaymentVoidData::cancellation_reason` is deliberately NOT forwarded either:
/// `POST /refunds` accepts a `reason`, but only the two literals `fraudulent`
/// and `expired`. UCS cancellation reasons (e.g. `requested_by_customer`) are
/// outside that set and Shift4 rejects them, so the field is dropped rather
/// than mapped onto a wrong value.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4VoidRequest {
    pub charge_id: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        Shift4RouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for Shift4VoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: Shift4RouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            charge_id: item.router_data.request.connector_transaction_id.clone(),
        })
    }
}

/// Releasing an authorization returns an ordinary Shift4 *refund* object, byte
/// for byte the same shape as an ordinary refund — `{id, amount, currency,
/// charge, status}` — so the void response is that same type. Nothing on the
/// refund object marks it as a released authorization; the discriminator lives
/// on the charge (`captured == false && refunded == true`), which is what
/// `get_shift4_attempt_status` reads on the PSync leg.
pub type Shift4VoidResponse = Shift4RefundResponse;

impl TryFrom<ResponseRouterData<Shift4VoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<Shift4VoidResponse, Self>) -> Result<Self, Self::Error> {
        // Terminality matters more here than anywhere else: a void that Shift4
        // explicitly declined is finished, so it maps to `VoidFailed` and never
        // to `Pending` — a `Pending` void is polled forever by the caller.
        let status = match item.response.status {
            Shift4RefundStatus::Successful => AttemptStatus::Voided,
            Shift4RefundStatus::Failed => AttemptStatus::VoidFailed,
            Shift4RefundStatus::Processing => AttemptStatus::VoidInitiated,
            // An undocumented status is "we could not read the outcome", not a
            // failure — `Unresolved`, never `VoidFailed` and never `Pending`.
            Shift4RefundStatus::Unknown => AttemptStatus::Unresolved,
        };

        // The caller's transaction id must stay the CHARGE, not the refund that
        // released it: a later PSync is `GET /charges/{id}`, and the refund id
        // (`item.response.id`) addresses a different resource entirely.
        let charge_id = item.response.charge.clone();

        let response = if matches!(item.response.status, Shift4RefundStatus::Failed) {
            // A Shift4 refund object carries no failureCode / failureMessage —
            // `status: "failed"` is the whole signal — so the message is built
            // here rather than read off the response.
            Err(domain_types::router_data::ErrorResponse {
                status_code: item.http_code,
                code: common_utils::consts::NO_ERROR_CODE.to_string(),
                message: SHIFT4_VOID_DECLINED.to_string(),
                reason: Some(format!(
                    "{SHIFT4_VOID_DECLINED} (charge {charge_id}, refund {})",
                    item.response.id
                )),
                attempt_status: Some(FlowStatus::Payment(AttemptStatus::VoidFailed)),
                connector_transaction_id: Some(charge_id.clone()),
                ..Default::default()
            })
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(charge_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(charge_id),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            })
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

// ===== SYNC REQUEST STRUCTURES =====
// Sync operations (GET requests) typically don't send a body, but we need these for the macro

#[derive(Debug, Serialize, Default)]
pub struct Shift4PSyncRequest {}

#[derive(Debug, Serialize, Default)]
pub struct Shift4RSyncRequest {}

// ===== MACRO-COMPATIBLE TRYFROM IMPLEMENTATIONS =====
// The macro creates a Shift4RouterData wrapper type. We need TryFrom implementations
// that work with this wrapper.

// PSync Request - converts from Shift4RouterData to empty request struct
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        Shift4RouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for Shift4PSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: Shift4RouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self::default())
    }
}

// RSync Request - converts from Shift4RouterData to empty request struct
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        Shift4RouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for Shift4RSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: Shift4RouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self::default())
    }
}

// Authorize Request - delegates to existing implementation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        Shift4RouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for Shift4PaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: Shift4RouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Delegate to the existing TryFrom<&RouterDataV2> implementation
        Self::try_from(&item.router_data)
    }
}

// Capture Request - we need a separate request type
#[derive(Debug, Serialize)]
pub struct Shift4CaptureRequest {
    // Shift4 capture is done via POST to /charges/{id}/capture with no body
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        Shift4RouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for Shift4CaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: Shift4RouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;

        // `POST /charges/{id}/capture` documents no request body, so Shift4 always
        // settles the full authorisation — a partial-capture intent cannot be
        // expressed on the wire. Reject it here instead of silently settling more
        // money than the caller asked for.
        let reject_partial = |detail: String| {
            error_stack::report!(IntegrationError::NotSupported {
                message: "Partial capture".to_string(),
                connector: "Shift4",
                context: IntegrationErrorContext {
                    additional_context: Some(detail),
                    suggested_action: Some(
                        "Capture the full authorised amount, then issue a refund for the \
                         difference via POST /refunds."
                            .to_string(),
                    ),
                    doc_url: Some("https://dev.shift4.com/docs/api#capture-a-charge".to_string()),
                },
            })
        };

        if request.is_multiple_capture() {
            return Err(reject_partial(
                "Shift4's capture endpoint takes no body and settles the whole charge, so a \
                 multiple-capture request cannot be honoured."
                    .to_string(),
            ));
        }
        if let Some(
            method @ (common_enums::CaptureMethod::ManualMultiple
            | common_enums::CaptureMethod::Scheduled),
        ) = &request.capture_method
        {
            return Err(reject_partial(format!(
                "Shift4 supports only AUTOMATIC and MANUAL capture; {method} capture would settle \
                 the whole charge on the first call."
            )));
        }
        // `minor_amount_authorized` is a response-reporting field that request-path
        // constructors leave as `None`, so this fires only when the caller actually
        // knows the authorised amount. A lone partial capture with no authorised
        // amount on the request is still indistinguishable from a full one at this
        // layer — closing that gap needs the authorised amount added to the capture
        // contract, mirroring the same known gap documented in `citigate`.
        if let Some(authorized) = router_data.resource_common_data.minor_amount_authorized {
            if request.minor_amount_to_capture != authorized {
                return Err(reject_partial(format!(
                    "amount_to_capture ({}) differs from the authorised amount ({}), but Shift4's \
                     capture endpoint takes no amount and would settle the full authorisation.",
                    request.minor_amount_to_capture.get_amount_as_i64(),
                    authorized.get_amount_as_i64(),
                )));
            }
        }

        Ok(Self {})
    }
}

// Refund Request - delegates to existing implementation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        Shift4RouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for Shift4RefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: Shift4RouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Delegate to the existing TryFrom<&RouterDataV2> implementation
        Self::try_from(&item.router_data)
    }
}

// ===== REPEAT PAYMENT (MIT) FLOW STRUCTURES =====

/// Shift4 MIT request - supports both stored card token and raw card details
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4RepeatPaymentRequest<T: PaymentMethodDataTypes> {
    pub amount: MinorUnit,
    pub currency: Currency,
    pub captured: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    /// Card: either a token string ("card_xxx") or raw card details object
    pub card: Shift4RepeatPaymentCard<T>,
    /// Transaction type: "merchant_initiated", "subsequent_recurring", etc.
    #[serde(rename = "type")]
    pub transaction_type: Shift4TransactionType,
    /// Customer ID required when charging a stored card (not needed for raw card)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_id: Option<String>,
}

/// Card field for MIT: either a stored card token or raw card details
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Shift4RepeatPaymentCard<T: PaymentMethodDataTypes> {
    /// Stored card identifier (e.g., "card_xxx")
    Token(String),
    /// Raw card details for approach 3 MIT
    RawCard(Shift4CardData<T>),
}

/// Shift4's `type` field on `POST /charges` — how the charge was initiated.
/// The enum is exactly these four values; Shift4 documents no default.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Shift4TransactionType {
    /// A one-off sale the cardholder is present for.
    CustomerInitiated,
    /// The cardholder-present charge that establishes a card on file.
    FirstRecurring,
    MerchantInitiated,
    SubsequentRecurring,
}

/// MIT response reuses the standard payments response
pub type Shift4RepeatPaymentResponse = Shift4PaymentsResponse;

// ===== REPEAT PAYMENT (MIT) REQUEST TRANSFORMATION =====

impl<T: PaymentMethodDataTypes>
    TryFrom<
        &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
    > for Shift4RepeatPaymentRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<
            RepeatPayment,
            PaymentFlowData,
            RepeatPaymentData<T>,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        // Determine card: use raw card data if available, otherwise use stored card token
        let (card, customer_id) = if let PaymentMethodData::Card(card_data) =
            &item.request.payment_method_data
        {
            // Approach 3: Raw card details for MIT (no customer needed).
            // `cardholderName` is optional on a normal Shift4 charge, so an
            // absent billing name is simply omitted rather than sent as `""`.
            let billing = item
                .resource_common_data
                .address
                .get_payment_method_billing();
            (
                Shift4RepeatPaymentCard::RawCard(Shift4CardData::new(
                    card_data,
                    billing.and_then(|b| b.get_optional_full_name()),
                    billing.and_then(|b| b.address.as_ref()),
                )),
                None, // No customer needed for raw card
            )
        } else {
            // Stored card token approach: extract from mandate_reference
            let token = match &item.request.mandate_reference {
                MandateReferenceId::ConnectorMandateId(connector_mandate_ref) => {
                    connector_mandate_ref
                        .get_connector_mandate_id()
                        .ok_or_else(|| {
                            error_stack::report!(IntegrationError::MissingRequiredField {
                                field_name: "connector_mandate_id (card token)",
                                context: Default::default(),
                            })
                        })?
                }
                MandateReferenceId::NetworkMandateId(_) => {
                    return Err(error_stack::report!(IntegrationError::NotSupported {
                        message: "NetworkMandateId is not supported for Shift4 MIT".to_string(),
                        connector: "Shift4",
                        context: IntegrationErrorContext {
                            suggested_action: Some(
                                "Use ConnectorMandateId with the stored Shift4 card token for this RepeatPayment path, or add a separate raw-card MIT mapper before sending NetworkMandateId."
                                    .to_string(),
                            ),
                            doc_url: None,
                            additional_context: Some(
                                "Shift4 RepeatPayment received a NetworkMandateId mandate reference. The current transformer only builds a token payment from connector_mandate_id; NetworkMandateId carries an NTI for raw-card MIT handling, which cannot be represented in the stored-token payload built here".to_string(),
                            ),
                        },
                    }));
                }
                MandateReferenceId::NetworkTokenWithNTI(_) => {
                    return Err(error_stack::report!(IntegrationError::NotSupported {
                        message: "NetworkTokenWithNTI is not supported for Shift4 MIT".to_string(),
                        connector: "Shift4",
                        context: IntegrationErrorContext {
                            suggested_action: Some(
                                "Use ConnectorMandateId with the stored Shift4 card token for this RepeatPayment path, or implement a dedicated Shift4 network-token MIT mapper before sending NetworkTokenWithNTI."
                                    .to_string(),
                            ),
                            doc_url: None,
                            additional_context: Some(
                                "Shift4 RepeatPayment received a NetworkTokenWithNTI mandate reference. The current transformer only builds a token payment from connector_mandate_id; it does not extract or map network token credentials, cryptogram data, or the NTI into a Shift4 MIT request".to_string(),
                            ),
                        },
                    }));
                }
            };
            (
                Shift4RepeatPaymentCard::Token(token),
                item.resource_common_data.connector_customer.clone(),
            )
        };

        // Determine Shift4 transaction type based on MIT category
        let transaction_type = match item.request.mit_category {
            Some(common_enums::MitCategory::Recurring) => {
                Shift4TransactionType::SubsequentRecurring
            }
            _ => Shift4TransactionType::MerchantInitiated,
        };

        let captured = item.request.is_auto_capture();

        Ok(Self {
            amount: item.request.minor_amount,
            currency: item.request.currency,
            captured,
            description: item.resource_common_data.description.clone(),
            metadata: item.request.metadata.clone().expose_option(),
            card,
            transaction_type,
            customer_id,
        })
    }
}

// RepeatPayment Request - converts from Shift4RouterData wrapper
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        Shift4RouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for Shift4RepeatPaymentRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: Shift4RouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Delegate to the existing TryFrom<&RouterDataV2> implementation
        Self::try_from(&item.router_data)
    }
}

// RepeatPayment Response transformation - reuses standard payments response mapping
impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<Shift4RepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<Shift4RepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = get_shift4_attempt_status(&item.response);
        let connector_response = build_shift4_connector_response(&item.response);

        let response = if matches!(item.response.status, Shift4PaymentStatus::Failed) {
            Err(build_shift4_failure_response(
                &item.response,
                item.http_code,
                FlowStatus::Payment(status),
            ))
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: get_shift4_redirection_data(&item.response),
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: item.response.scheme_transaction_id.clone(),
                network_txn_link_id: None,
                connector_response_reference_id: Some(item.response.id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            })
        };

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                connector_response,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== CLIENT AUTHENTICATION TOKEN FLOW STRUCTURES =====

/// Shift4 Checkout Session Request — creates a checkout session for client-side SDK initialization.
/// The response contains a `clientSecret` used by the Shift4 Checkout Session SDK.
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4ClientAuthRequest {
    pub line_items: Vec<Shift4LineItem>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4LineItem {
    pub product: Shift4InlineProduct,
    pub quantity: i64,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4InlineProduct {
    pub name: String,
    pub amount: MinorUnit,
    pub currency: Currency,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        Shift4RouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                MerchantAuthenticationFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for Shift4ClientAuthRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: Shift4RouterData<
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

        Ok(Self {
            line_items: vec![Shift4LineItem {
                product: Shift4InlineProduct {
                    name: "Payment".to_string(),
                    amount: router_data.request.amount,
                    currency: router_data.request.currency,
                },
                quantity: 1,
            }],
        })
    }
}

// ===== INCREMENTAL AUTHORIZATION FLOW =====
//
// Shift4 exposes `POST /charges/{chargeId}/incremental-authorization` to raise the
// authorized amount on an existing pre-authorization. The charge must have been
// created with `captured=false` AND `options.authorizationType="pre"`.
// Reference: https://dev.shift4.com/docs/api/#increment-charge-authorization
// Note: the published doc example URL uses the singular "/increment-authorization",
// but the deployed API only routes the plural "/incremental-authorization". The
// plural form is what the SDK actually calls.
//
// The `amount` field in the request is the INCREMENT amount (additional amount to
// add to the existing authorization), not the new total. This matches CyberSource's
// `additionalAmount` semantics and Prism's `PaymentsIncrementalAuthorizationData.minor_amount`.
// The response is the updated charge object, which mirrors `Shift4PaymentsResponse`.

#[derive(Debug, Serialize)]
pub struct Shift4IncrementalAuthRequest {
    /// Increment amount (additional funds to authorize) in minor units.
    /// Example: initial charge $10.00 (amount=1000) + increment $5.00 (amount=500)
    /// results in a total authorization of $15.00 (amount=1500).
    pub amount: MinorUnit,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        Shift4RouterData<
            RouterDataV2<
                IncrementalAuthorization,
                PaymentFlowData,
                PaymentsIncrementalAuthorizationData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for Shift4IncrementalAuthRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: Shift4RouterData<
            RouterDataV2<
                IncrementalAuthorization,
                PaymentFlowData,
                PaymentsIncrementalAuthorizationData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            amount: item.router_data.request.minor_amount,
        })
    }
}

// Map the Shift4 charge-object response returned by /increment-authorization
// into a PaymentsResponseData::IncrementalAuthorizationResponse.
//
// A 200 OK from Shift4 with `status: "failed"` is mapped to `Err(ErrorResponse)`
// so downstream error handling uses the conventional error channel rather than
// the caller having to inspect `AuthorizationStatus::Failure` inside an `Ok`.
// This mirrors the worldpayvantiv IncrementalAuthorization transformer.
impl TryFrom<ResponseRouterData<Shift4PaymentsResponse, Self>>
    for RouterDataV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<Shift4PaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = match item.response.status {
            // Same decline shape as every other charge-object failure, so it goes
            // through the shared builder: an incremental-authorization decline
            // carries `issuerDeclineCode` / `adviceCode` / `networkAdviceCode` too,
            // and smart retry needs them here just as much as on Authorize.
            Shift4PaymentStatus::Failed => Err(build_shift4_failure_response(
                &item.response,
                item.http_code,
                FlowStatus::Payment(AttemptStatus::AuthorizationFailed),
            )),
            Shift4PaymentStatus::Successful => {
                Ok(PaymentsResponseData::IncrementalAuthorizationResponse {
                    status: AuthorizationStatus::Success,
                    connector_authorization_id: Some(item.response.id.clone()),
                    status_code: item.http_code,
                })
            }
            Shift4PaymentStatus::Pending => {
                Ok(PaymentsResponseData::IncrementalAuthorizationResponse {
                    status: AuthorizationStatus::Processing,
                    connector_authorization_id: Some(item.response.id.clone()),
                    status_code: item.http_code,
                })
            }
            // A charge status Shift4 has added since this was written. It is not
            // evidence the increment was declined, so it must not become
            // `Failure` — surface it as requiring merchant action instead.
            Shift4PaymentStatus::Unknown => {
                Ok(PaymentsResponseData::IncrementalAuthorizationResponse {
                    status: AuthorizationStatus::Unresolved,
                    connector_authorization_id: Some(item.response.id.clone()),
                    status_code: item.http_code,
                })
            }
        };

        // Keep the parent payment in Authorized state on success; on failure, mark
        // the attempt as AuthorizationFailed so downstream sees a coherent terminal
        // state rather than a stale Authorized with an Err response.
        let status = if response.is_ok() {
            AttemptStatus::Authorized
        } else {
            AttemptStatus::AuthorizationFailed
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

/// Shift4 Checkout Session Response — contains the clientSecret for SDK initialization.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4ClientAuthResponse {
    pub client_secret: Secret<String>,
}

impl TryFrom<ResponseRouterData<Shift4ClientAuthResponse, Self>>
    for RouterDataV2<
        ClientAuthenticationToken,
        MerchantAuthenticationFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<Shift4ClientAuthResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        let session_data = ClientAuthenticationTokenData::ConnectorSpecific(Box::new(
            ConnectorSpecificClientAuthenticationResponse::Shift4(
                Shift4ClientAuthenticationResponseDomain {
                    client_secret: response.client_secret,
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
// Shift4 does not expose a dedicated mandate-setup endpoint. The idiomatic
// approach for setting up a card-on-file / mandate with Shift4 is to issue
// an authorization-only (uncaptured) charge via the standard `/charges`
// endpoint. On success, the resulting `charge.id` is surfaced as the
// `connector_mandate_id` used for subsequent RepeatPayment (MIT) calls —
// this mirrors the pattern used by Shift4's existing Authorize flow and
// plays well with downstream `Shift4RepeatPaymentRequest` which accepts
// either a token or raw card for MIT.
//
// Customer-Initiated Transaction (CIT): the customer is present consenting
// to store the card on file. We use the request's minor_amount if provided
// (some callers pass a small verification amount) and fall back to 0 for a
// zero-dollar verification.

/// SetupMandate request - a slim, reusable shape matching the Shift4
/// `/charges` contract used for zero/low-amount verification.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4SetupMandateRequest<T: PaymentMethodDataTypes> {
    pub amount: MinorUnit,
    pub currency: Currency,
    pub captured: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    /// Existing Shift4 customer id (format `cust_xxx`). Only set when the
    /// caller has already provisioned the customer on Shift4.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_id: Option<String>,
    /// Embedded customer payload — when no pre-existing `customer_id` is
    /// known, Shift4 will auto-create a customer from this object and
    /// return its id + the stored-card id, which are both required for
    /// subsequent MIT / RepeatPayment calls.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer: Option<Shift4EmbeddedCustomer>,
    #[serde(flatten)]
    pub payment_method: Shift4PaymentMethod<T>,
}

/// Minimal embedded customer object accepted by Shift4 `/charges`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4EmbeddedCustomer {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<pii::Email>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// SetupMandate response — reuses Shift4's standard charge response.
pub type Shift4SetupMandateResponse = Shift4PaymentsResponse;

// SetupMandate Request - converts from Shift4RouterData wrapper
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        Shift4RouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for Shift4SetupMandateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: Shift4RouterData<
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

        // Require the caller to specify an amount. Shift4 accepts 0 for
        // card-on-file verification, but we don't silently default to it —
        // the caller must pass 0 explicitly if that's what they mean, so a
        // missing amount is always a client error rather than an implicit
        // zero-dollar auth.
        let amount = item.request.minor_amount.ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "amount",
                context: Default::default(),
            })
        })?;
        let billing_details = item
            .resource_common_data
            .address
            .get_payment_method_billing();

        let payment_method = match &item.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                // Cardholder name comes from the billing address — the
                // cardholder and customer may be different entities, so
                // never fall back to the customer-level name. Shift4 documents
                // it as optional on a normal charge, so it is only enforced on
                // the zero-amount Account Name Inquiry path, where the name is
                // the thing being verified.
                let cardholder_name =
                    billing_details.and_then(|billing| billing.get_optional_full_name());

                if amount == MinorUnit::new(0) && cardholder_name.is_none() {
                    return Err(error_stack::report!(
                        IntegrationError::MissingRequiredField {
                            field_name: "card.cardholderName",
                            context: IntegrationErrorContext {
                                additional_context: Some(
                                    "A zero-amount Shift4 charge is an Account Name Inquiry (ANI): \
                                     Shift4 verifies `card.cardholderName` against the name on the \
                                     account and returns the outcome in `aniCheck.result`. Without \
                                     a name there is nothing to verify and Shift4 rejects the charge."
                                        .to_string(),
                                ),
                                suggested_action: Some(
                                    "Send a billing first/last name with the zero-amount setup \
                                     request, or set a non-zero verification amount."
                                        .to_string(),
                                ),
                                doc_url: Some(
                                    "https://dev.shift4.com/docs/fraud-prevention/verification-checks"
                                        .to_string(),
                                ),
                            },
                        }
                    ));
                }

                Shift4PaymentMethod::Card(Shift4CardPayment {
                    card: Shift4CardData::new(
                        card_data,
                        cardholder_name,
                        billing_details.and_then(|billing| billing.address.as_ref()),
                    ),
                })
            }
            PaymentMethodData::PaymentMethodToken(pmt) => {
                Shift4PaymentMethod::TokenPayment(Shift4TokenPayment {
                    card: pmt.token.clone(),
                })
            }
            _ => {
                return Err(error_stack::report!(IntegrationError::NotSupported {
                    message: "Payment method not supported for SetupMandate".to_string(),
                    connector: "Shift4",
                    context: Default::default(),
                }))
            }
        };

        // captured=false for SetupMandate; we only authorize (or
        // verify) to store the card-on-file. `customer_id` is the
        // Shift4 customer identifier, sourced exclusively from
        // `connector_customer` (populated by the orchestrator after a
        // CreateConnectorCustomer call). We do not infer it from the
        // merchant-side `request.customer_id`, which is an opaque
        // Hyperswitch identifier and may coincidentally share any
        // prefix.
        let customer_id = item.resource_common_data.connector_customer.clone();

        Ok(Self {
            amount,
            currency: item.request.currency,
            captured: false,
            description: item.resource_common_data.description.clone(),
            metadata: item.request.metadata.clone().expose_option(),
            customer_id,
            customer: None,
            payment_method,
        })
    }
}

// SetupMandate Response transformation - reuses Shift4PaymentsResponse and
// extracts connector_mandate_id = charge.id. For zero-amount setup, map
// Authorized -> Charged so the flow reaches a terminal state.
impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<Shift4SetupMandateResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<Shift4SetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let mut status = get_shift4_attempt_status(&item.response);

        // For zero-amount mandate setup, treat Authorized as Charged so
        // the attempt reaches a terminal state for downstream consumers.
        if status == AttemptStatus::Authorized {
            status = AttemptStatus::Charged;
        }

        // Extract redirect URL if present (BankRedirect setups).
        let redirection_data = get_shift4_redirection_data(&item.response);
        let connector_response = build_shift4_connector_response(&item.response);

        let response = match status {
            // Shift4 sets `failureCode` / `failureMessage` (and the three
            // network codes) on declined charges, so the merchant sees the
            // actual decline reason rather than a static sentinel.
            AttemptStatus::Failure => Err(build_shift4_failure_response(
                &item.response,
                item.http_code,
                FlowStatus::Payment(status),
            )),
            _ => {
                // For MIT/RepeatPayment, Shift4 requires the stored-card
                // token (`card_xxx`) returned inside `response.card`. The
                // top-level `response.id` is the charge id (`char_xxx`)
                // and cannot be used to charge the card again, so we do
                // not fall back to it — returning `None` instead lets
                // downstream detect an unusable mandate.
                let mandate_reference = item.response.card.as_ref().map(|card| {
                    Box::new(MandateReference {
                        connector_mandate_id: Some(card.id.clone()),
                        payment_method_id: Some(card.id.clone()),
                        connector_mandate_request_reference_id: None,
                        mandate_metadata: None,
                    })
                });

                Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                    redirection_data,
                    mandate_reference,
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    // Shift4 PSync hits `GET /charges/{id}` with the
                    // charge id, so surfacing it here lets sync flows
                    // look up this attempt.
                    connector_response_reference_id: Some(item.response.id),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                    splits: None,
                    payment_account_reference: None,
                })
            }
        };

        // Propagate the customer id returned by Shift4 so that the
        // subsequent RepeatPayment (MIT) call can pass `customerId`
        // alongside the stored card token — required by Shift4 when
        // charging a stored card.
        let connector_customer = item
            .response
            .customer
            .as_ref()
            .map(|c| c.id().to_string())
            .or(item
                .router_data
                .resource_common_data
                .connector_customer
                .clone());

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                connector_customer,
                connector_response,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}
