use crate::types::ResponseRouterData;
use common_enums::{AttemptStatus, AuthorizationStatus, Currency, RefundStatus};
use common_utils::{
    pii,
    request::Method,
    types::{AmountConvertor, ConnectorMinorUnit, MinorUnit},
};
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
        ApplePayWalletData, BankRedirectData, GpayTokenizationData, PaymentMethodData,
        PaymentMethodDataTypes, RawCardNumber, WalletData,
    },
    router_data::{
        AdditionalPaymentMethodConnectorResponse, ConnectorResponseData, ConnectorSpecificConfig,
        FlowStatus,
    },
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, ExposeOptionInterface, PeekInterface, Secret};
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
    /// Shift4 customer id (`cust_...`).
    pub id: Secret<String>,
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
                connector_customer_id: item.response.id.expose(),
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
    pub amount: ConnectorMinorUnit,
    pub currency: Currency,
    pub captured: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    /// Shift4 customer (`cust_...`). Sent only on a charge that stores the card
    /// or wallet payment method on file, or that charges a token; see the
    /// Authorize builder.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_id: Option<Secret<String>>,
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
    Wallet(Shift4WalletPayment),
}

/// Token-based payment — the `card` field carries a token ID from Shift4 Components SDK
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4TokenPayment {
    pub card: Secret<String>,
}

/// Apple Pay or Google Pay sent inline as `paymentMethod` on `POST /charges`.
/// Charged together with `customerId`, it creates a payment method (`pm_...`)
/// owned by that customer, which later charges reference by id.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4WalletPayment {
    pub payment_method: Shift4WalletPaymentMethod,
}

/// Shift4 takes a card wallet only as its encrypted token: there is no request
/// field for decrypted wallet data (a DPAN, cryptogram or ECI).
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Shift4WalletPaymentMethod {
    ApplePay {
        #[serde(rename = "applePay")]
        apple_pay: Shift4ApplePayToken,
        /// Billed-party details live on the payment method: Shift4 rejects a
        /// charge-level `billing` on a charge that carries `paymentMethod`.
        #[serde(skip_serializing_if = "Option::is_none")]
        billing: Option<Shift4Billing>,
    },
    GooglePay {
        #[serde(rename = "googlePay")]
        google_pay: Shift4GooglePayToken,
        /// See `ApplePay::billing`.
        #[serde(skip_serializing_if = "Option::is_none")]
        billing: Option<Shift4Billing>,
    },
}

#[derive(Debug, Serialize)]
pub struct Shift4ApplePayToken {
    /// The base64-decoded Apple Pay `paymentData`. A JSON token is sent as an
    /// object, as Shift4's WooCommerce gateway does; Shift4's sandbox test
    /// tokens (`TEST_TOKEN:<amount><CURRENCY>`) are not JSON and are sent as a
    /// string. Shift4 accepts `applePay.token` in either form.
    pub token: Secret<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct Shift4GooglePayToken {
    /// Google Pay `tokenizationData.token`, as issued.
    pub token: Secret<String>,
}

/// `flow` names the flow the wallet was sent on (`SetupMandate`, `Authorize`),
/// so the message says where it was refused.
fn shift4_wallet_not_supported(
    wallet: &str,
    flow: &str,
    reason: &str,
    suggested_action: &str,
) -> error_stack::Report<IntegrationError> {
    error_stack::report!(IntegrationError::NotSupported {
        message: format!("{wallet} for {flow}"),
        connector: "Shift4",
        context: IntegrationErrorContext {
            additional_context: Some(reason.to_string()),
            suggested_action: Some(suggested_action.to_string()),
            doc_url: Some(
                "https://dev.shift4.com/docs/api#create-a-new-payment-method".to_string(),
            ),
        },
    })
}

impl Shift4ApplePayToken {
    fn try_new(
        apple_pay: &ApplePayWalletData,
        flow: &str,
    ) -> Result<Self, error_stack::Report<IntegrationError>> {
        if apple_pay
            .payment_data
            .get_decrypted_apple_pay_payment_data_optional()
            .is_some()
        {
            return Err(shift4_wallet_not_supported(
                "Pre-decrypted Apple Pay",
                flow,
                "Shift4 accepts Apple Pay only as the encrypted `applePay.token`, which it \
                 decrypts itself. It has no request field for a decrypted DPAN, online \
                 cryptogram or ECI and rejects them as unrecognized fields, so decrypted \
                 Apple Pay data can be neither charged nor stored as a payment method.",
                "Send the encrypted Apple Pay payment data (payment_data.encrypted_data).",
            ));
        }
        let decoded = apple_pay.get_applepay_decoded_payment_data()?;
        let token = match serde_json::from_str::<serde_json::Value>(decoded.peek()) {
            Ok(json @ serde_json::Value::Object(_)) => json,
            _ => serde_json::Value::String(decoded.peek().clone()),
        };
        Ok(Self {
            token: Secret::new(token),
        })
    }
}

impl Shift4WalletPaymentMethod {
    /// Google Pay `PAN_ONLY` credentials need a 3DS step that this server-side
    /// request cannot perform; Shift4 declines them with HTTP 402
    /// `authentication_required`. The auth method is inside the encrypted
    /// token, so it cannot be rejected before the request is sent.
    fn try_new(
        wallet_data: &WalletData,
        payment_method_type: Option<common_enums::PaymentMethodType>,
        billing: Option<Shift4Billing>,
        flow: &str,
    ) -> Result<Self, error_stack::Report<IntegrationError>> {
        match wallet_data {
            WalletData::ApplePay(apple_pay) => Ok(Self::ApplePay {
                apple_pay: Shift4ApplePayToken::try_new(apple_pay, flow)?,
                billing,
            }),
            WalletData::GooglePay(google_pay) => match &google_pay.tokenization_data {
                GpayTokenizationData::Encrypted(encrypted) => Ok(Self::GooglePay {
                    google_pay: Shift4GooglePayToken {
                        token: Secret::new(encrypted.token.clone()),
                    },
                    billing,
                }),
                GpayTokenizationData::Decrypted(_) => Err(shift4_wallet_not_supported(
                    "Pre-decrypted Google Pay",
                    flow,
                    "Shift4 accepts Google Pay only as the encrypted `googlePay.token`, which it \
                     decrypts itself. It has no request field for a decrypted PAN / DPAN, \
                     cryptogram or ECI and rejects them as unrecognized fields, so decrypted \
                     Google Pay data can be neither charged nor stored as a payment method.",
                    "Send the encrypted Google Pay token (tokenization_data.encrypted_data).",
                )),
            },
            WalletData::ApplePayThirdPartySdk(_) | WalletData::GooglePayThirdPartySdk(_) => {
                Err(shift4_wallet_not_supported(
                    "Apple Pay / Google Pay third-party SDK token",
                    flow,
                    "The token was issued by a third-party wallet SDK. Shift4 accepts a card \
                     wallet only as the Apple Pay or Google Pay encrypted payment token.",
                    "Send the encrypted Apple Pay payment data or Google Pay token instead.",
                ))
            }
            WalletData::ApplePayRedirect(_) | WalletData::GooglePayRedirect(_) => {
                Err(shift4_wallet_not_supported(
                    "Apple Pay / Google Pay redirect",
                    flow,
                    "A redirect wallet carries no encrypted payment token, which is the only \
                     form in which Shift4 accepts a card wallet.",
                    "Send the encrypted Apple Pay payment data or Google Pay token instead.",
                ))
            }
            _ => Err(shift4_wallet_not_supported(
                &payment_method_type.map_or_else(
                    || "This wallet".to_string(),
                    |wallet| format!("Wallet {wallet:?}"),
                ),
                flow,
                "Shift4 stores only Apple Pay and Google Pay wallet payment methods \
                 (`paymentMethod.type` `apple_pay` / `google_pay`) for later \
                 merchant-initiated charges.",
                "Use Apple Pay or Google Pay with an encrypted token, or a card.",
            )),
        }
    }
}

/// `true` when `wallet_data` is an Apple Pay or Google Pay variant that
/// `Shift4WalletPaymentMethod::try_new` maps or refuses with a specific reason.
/// Other wallets fall to try_new's catch-all, whose message is about storing a
/// payment method, so Authorize checks this first and keeps them `NotImplemented`.
fn is_shift4_card_wallet(wallet_data: &WalletData) -> bool {
    matches!(
        wallet_data,
        WalletData::ApplePay(_)
            | WalletData::GooglePay(_)
            | WalletData::ApplePayThirdPartySdk(_)
            | WalletData::GooglePayThirdPartySdk(_)
            | WalletData::ApplePayRedirect(_)
            | WalletData::GooglePayRedirect(_)
    )
}

/// Why a charge that stores an Apple Pay / Google Pay payment method must name
/// the Shift4 customer. Shared by SetupMandate and the off-session Authorize CIT.
const SHIFT4_WALLET_MANDATE_NEEDS_CUSTOMER: &str =
    "A wallet mandate is the payment method (`pm_...`) the storing charge creates, and \
     Shift4 charges a stored payment method only together with a `customerId` \
     (\"Charge using customer's payment method requires customerId to be provided\"). \
     Shift4 would store a payment method no customer owns, but the first customer to \
     charge it would then take ownership of it. The owner is therefore named on the \
     storing charge, so the mandate can only ever be charged under that customer.";

/// Why a charge that stores a card on file must name the Shift4 customer. Shared by
/// SetupMandate and the off-session Authorize CIT.
const SHIFT4_CARD_MANDATE_NEEDS_CUSTOMER: &str =
    "Shift4 stores a card on file only under a Shift4 customer and rejects a later \
     merchant-initiated charge of that card without the owning `customerId`, so a \
     mandate created without a customer could not be charged.";

const SHIFT4_CREATE_CUSTOMER_DOC: &str = "https://dev.shift4.com/docs/api#create-a-customer";

/// Prefix of a Shift4 payment method id. Shift4 ids carry their object type as a
/// prefix: an Apple Pay / Google Pay mandate is `pm_` + 24 alphanumerics, and a
/// card mandate is `card_...` (both confirmed in the sandbox).
///
/// Hyperswitch's own payment method ids also start with `pm_`. The prefix is
/// therefore only ever tested on `connector_mandate_id`, the id Shift4 issued, and
/// never on `payment_method_id` or any other caller-side id.
const SHIFT4_PAYMENT_METHOD_ID_PREFIX: &str = "pm_";

/// The stored Apple Pay / Google Pay payment method id (`pm_...`) a RepeatPayment
/// mandate names, which Shift4 charges as `paymentMethod`, not `card`. `None` for
/// every other mandate, including a stored card (`card_...`).
///
/// The Shift4 mandate id's own prefix decides: it is the value that goes on the
/// wire and it is present on every MIT, whereas `payment_method_data` /
/// `payment_method_type` are optional on RecurringPaymentService/Charge and no
/// Shift4 mandate carries `mandate_metadata`.
fn shift4_payment_method_mandate_id(mandate_reference: &MandateReferenceId) -> Option<String> {
    match mandate_reference {
        MandateReferenceId::ConnectorMandateId(connector_mandate_ref) => connector_mandate_ref
            .get_connector_mandate_id()
            .filter(|id| id.starts_with(SHIFT4_PAYMENT_METHOD_ID_PREFIX)),
        MandateReferenceId::NetworkMandateId(_) | MandateReferenceId::NetworkTokenWithNTI(_) => {
            None
        }
    }
}

/// `external` carrying the merchant reference and, on a network-mandate MIT, the
/// original charge's scheme transaction id. `None` when both are empty, so neither
/// an empty `vendorReference` nor an empty `external` object is sent.
fn build_shift4_external(
    reference: &str,
    scheme_transaction_id: Option<String>,
) -> Option<Shift4External> {
    let vendor_reference = Some(reference.to_string()).filter(|r| !r.trim().is_empty());
    let scheme_transaction_id = scheme_transaction_id.filter(|id| !id.trim().is_empty());
    (vendor_reference.is_some() || scheme_transaction_id.is_some()).then_some(Shift4External {
        vendor_reference,
        scheme_transaction_id,
    })
}

/// The Shift4 customer (`cust_...`) on the request, taken only from
/// `connector_customer`, never from the merchant-side customer id. An empty id is
/// treated as absent.
fn shift4_connector_customer(flow_data: &PaymentFlowData) -> Option<Secret<String>> {
    flow_data
        .connector_customer
        .clone()
        .filter(|id| !id.trim().is_empty())
        .map(Secret::new)
}

fn shift4_mandate_reference(credential_id: &Secret<String>) -> Box<MandateReference> {
    Box::new(MandateReference {
        connector_mandate_id: Some(credential_id.peek().clone()),
        payment_method_id: Some(credential_id.peek().clone()),
        connector_mandate_request_reference_id: None,
        mandate_metadata: None,
    })
}

/// Mandate reference a successful charge hands back: the stored credential a later
/// merchant-initiated charge references together with its owning customer. Shared
/// by Authorize (CIT), SetupMandate and RepeatPayment.
///
/// * Apple Pay / Google Pay: Shift4 returns no `card`. The credential is the payment
///   method (`pm_...`) the charge used, and only while a customer owns it and its
///   `status` is `chargeable`. Shift4 documents `pending`, `failed` and `used`
///   ("already charged and cannot be reused") too, none of which can be charged.
/// * Card: the stored card (`card_...`), and only when `card.customerId` names the
///   customer that owns it. The charge's flat `customerId` is deliberately *not*
///   accepted as a substitute: it says which customer the charge was assigned to,
///   not who owns the card, so it cannot stand in for card ownership. Shift4 sets
///   `card.customerId` itself when it files the card under the charge's customer,
///   so on a charge that really did store a card the two agree; reading the card's
///   own field just means the mandate is never handed back on Shift4's word about a
///   different object.
///
/// The card branch looks asymmetric next to the wallet one — no `status` check —
/// because the Shift4 card object has no `status` field at all. Its attributes are
/// `id`, `created`, `objectType`, `first6`, `last4`, `fingerprint`, `expMonth`,
/// `expYear`, `cardholderName`, `customerId`, `brand`, `type`, `country`, `issuer`,
/// the `address*` fields, `fraudCheckData`, `merchantAccountId` and `fastCredit`
/// (<https://dev.shift4.com/docs/api#card-object>). `status` is a payment-method
/// concept only (<https://dev.shift4.com/docs/api#payment-methods>), so on a card
/// there is nothing to check and ownership is the whole test.
///
/// The charge id (`char_...`) cannot charge the credential again, so there is no
/// fallback to it.
fn build_shift4_mandate_reference(
    response: &Shift4PaymentsResponse,
) -> Option<Box<MandateReference>> {
    if let Some(payment_method) = response.payment_method.as_ref() {
        return payment_method
            .id
            .as_ref()
            .filter(|_| {
                payment_method.customer_id.is_some()
                    && payment_method.status == Some(Shift4PaymentMethodStatus::Chargeable)
            })
            .map(shift4_mandate_reference);
    }
    response
        .card
        .as_ref()
        .filter(|card| card.customer_id.is_some())
        .map(|card| shift4_mandate_reference(&card.id))
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
    /// Card security code. Shift4 only runs the CVV check when this is present;
    /// omitting it reports `cvvCheck.result` as `not_provided`. It is omitted,
    /// never sent empty, when the card carries no CVC.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cvc: Option<Secret<String>>,
    /// The name on the card. Optional on every Shift4 charge, a zero-amount one
    /// included (sandbox: a zero-amount charge without it succeeds). When present
    /// on a zero-amount charge, Shift4 runs its Account Name Inquiry on it
    /// (`aniCheck`, where activated on the account).
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
    /// address that drives AVS. Used by Authorize, SetupMandate and the
    /// network-mandate RepeatPayment.
    ///
    /// `cardholderName` is the card's own holder name and nothing else. It is
    /// never filled from the billing name or the customer name: the cardholder,
    /// the billed party and the customer can be different people, and a
    /// substituted name would skew Shift4's name verification.
    fn new(
        card_data: &domain_types::payment_method_data::Card<T>,
        billing_address: Option<&payment_address::AddressDetails>,
    ) -> Self {
        Self {
            number: card_data.card_number.clone(),
            exp_month: card_data.card_exp_month.clone(),
            exp_year: card_data.card_exp_year.clone(),
            cvc: Some(card_data.card_cvc.clone()).filter(|cvc| !cvc.peek().trim().is_empty()),
            cardholder_name: card_data
                .card_holder_name
                .clone()
                .filter(|name| !name.peek().trim().is_empty()),
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

/// `MissingRequiredField` for a request that would put a credential on file
/// without the Shift4 customer that has to own it. Callers differ only in why
/// the customer is needed and which documentation page they cite.
fn missing_shift4_connector_customer(
    additional_context: &str,
    doc_url: &str,
) -> error_stack::Report<IntegrationError> {
    error_stack::report!(IntegrationError::MissingRequiredField {
        field_name: "customer.connector_customer_id",
        context: IntegrationErrorContext {
            additional_context: Some(additional_context.to_string()),
            suggested_action: Some(
                "Create the customer on Shift4 first (CustomerService/Create, \
                 POST /customers) and pass the returned `cust_...` id as \
                 customer.connector_customer_id."
                    .to_string(),
            ),
            doc_url: Some(doc_url.to_string()),
        },
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
        let is_zero_amount = item.request.minor_amount == MinorUnit::default();
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
            PaymentMethodData::Card(card_data) => Shift4PaymentMethod::Card(Shift4CardPayment {
                card: Shift4CardData::new(
                    card_data,
                    billing_details.and_then(|billing| billing.address.as_ref()),
                ),
            }),
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
            // Apple Pay / Google Pay encrypted token, sent inline as `paymentMethod`
            // exactly as SetupMandate sends it. Billing goes on the payment method,
            // because Shift4 rejects a charge-level `billing` next to `paymentMethod`.
            PaymentMethodData::Wallet(wallet_data) if is_shift4_card_wallet(wallet_data) => {
                Shift4PaymentMethod::Wallet(Shift4WalletPayment {
                    payment_method: Shift4WalletPaymentMethod::try_new(
                        wallet_data,
                        item.request.payment_method_type,
                        build_shift4_billing(billing_details, item.request.email.as_ref()),
                        "Authorize",
                    )?,
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
        let is_wallet = matches!(payment_method, Shift4PaymentMethod::Wallet(_));
        let is_token = matches!(payment_method, Shift4PaymentMethod::TokenPayment(_));

        // A CIT that stores the card or the Apple Pay / Google Pay payment method
        // for later merchant-initiated use (off-session + customer acceptance) must
        // name the Shift4 customer that will own the credential, by the same rule
        // SetupMandate enforces. It is refused before the shopper is charged rather
        // than handing back a mandate that could not be charged.
        let stores_on_file = item.request.is_customer_initiated_mandate_payment()
            && matches!(
                payment_method,
                Shift4PaymentMethod::Card(_)
                    | Shift4PaymentMethod::TokenPayment(_)
                    | Shift4PaymentMethod::Wallet(_)
            );
        let connector_customer = shift4_connector_customer(&item.resource_common_data);
        if stores_on_file && connector_customer.is_none() {
            return Err(missing_shift4_connector_customer(
                if is_wallet {
                    SHIFT4_WALLET_MANDATE_NEEDS_CUSTOMER
                } else {
                    SHIFT4_CARD_MANDATE_NEEDS_CUSTOMER
                },
                SHIFT4_CREATE_CUSTOMER_DOC,
            ));
        }
        // `customerId` is sent only when the charge stores the credential, or when
        // it charges a token (a token of a customer's stored card is charged only
        // together with that customer). A one-off card, wallet or bank-redirect
        // sale omits it even when the customer is known: Shift4 would otherwise
        // attach the card (as the customer's default card) or the payment method
        // to that customer, keeping a credential nobody asked to store.
        let customer_id = if stores_on_file || is_token {
            connector_customer
        } else {
            None
        };

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
        // that is also storing the card or wallet payment method on file for later
        // merchant-initiated use is `first_recurring`; every other Authorize is a
        // plain `customer_initiated` sale. (`merchant_initiated` / `subsequent_recurring` belong to
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
            amount: common_utils::types::MinorUnitForConnector
                .convert(&common_utils::types::Money::from_minor_unit(
                    item.request.minor_amount,
                    item.request.currency,
                ))
                .change_context(IntegrationError::AmountConversionFailed {
                    context: Default::default(),
                })?,
            currency: item.request.currency,
            captured,
            description: item.resource_common_data.description.clone(),
            metadata: item.request.metadata.clone().expose_option(),
            customer_id,
            options,
            transaction_type: Some(transaction_type),
            // A wallet charge carries billing on `paymentMethod.billing` instead.
            billing: if is_wallet {
                None
            } else {
                build_shift4_billing(billing_details, item.request.email.as_ref())
            },
            shipping: build_shift4_shipping(item.resource_common_data.address.get_shipping()),
            // The merchant reference. A scheme transaction id is only ever sent on a
            // network-mandate MIT (`Shift4RepeatPaymentRequest`), never here.
            external: build_shift4_external(
                &item.resource_common_data.connector_request_reference_id,
                None,
            ),
            payment_method,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4PaymentsResponse {
    pub id: String,
    pub currency: Currency,
    pub amount: ConnectorMinorUnit,
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
    /// Stored-card object, present on a card charge. Its `id` (`card_...`) is the
    /// credential a later RepeatPayment / MIT references.
    pub card: Option<Shift4ResponseCard>,
    /// Payment method the charge used. Present on an Apple Pay / Google Pay
    /// charge, which has no `card` object, and on other payment-method charges;
    /// `null` on a card charge.
    pub payment_method: Option<Shift4ResponsePaymentMethod>,
    /// Shift4 customer (`cust_...`) the charge is assigned to. Shift4 returns it
    /// flat on the charge (and again as `card.customerId` or
    /// `paymentMethod.customerId`); a charge object has no nested `customer`.
    pub customer_id: Option<Secret<String>>,
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
    /// Network-assigned Payment Account Reference (PAR) for the underlying card.
    /// `null` on the wallet charges seen in the sandbox.
    pub payment_account_reference: Option<Secret<String>>,
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

/// The captured amount of a Shift4 charge, reported as `amount_captured` /
/// `minor_amount_captured` by every flow that reads a charge object (Authorize,
/// PSync, Capture, RepeatPayment, SetupMandate).
///
/// Funds are captured only on a successful charge with `captured: true`, and the
/// captured amount is then the charge `amount`: Shift4 rewrites `amount` to the
/// captured amount on a capture (a 1000 authorization captured with 400 reads
/// back `amount: 400, captured: true`). An uncaptured authorization, a
/// zero-amount verification (always sent uncaptured), a released authorization,
/// a pending charge and a decline report no captured amount.
fn get_shift4_captured_amount(response: &Shift4PaymentsResponse) -> Option<ConnectorMinorUnit> {
    (matches!(response.status, Shift4PaymentStatus::Successful) && response.captured)
        .then_some(response.amount)
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

/// Fold Shift4's verification checks and the issuer auth code into the UCS
/// connector response.
///
/// * Apple Pay / Google Pay charge (`paymentMethod.type`): only the auth code, on
///   the `ApplePay` / `GooglePay` variant. The AVS / CVV results Shift4 reports
///   for a wallet charge have no wallet carrier, so they are not reported.
/// * Card charge: `payment_checks` on `AdditionalPaymentMethodConnectorResponse::Card`,
///   which reaches callers as `CardConnectorResponse.payment_checks` (there is no
///   dedicated `avs_result` field on the UCS response types), plus the auth code.
///
/// AVS and ANI both require Shift4 to activate them on the merchant account; on
/// an account without them the objects are simply absent, which is reported as
/// "check not performed" (the key is omitted) rather than as a failure.
fn build_shift4_connector_response(
    response: &Shift4PaymentsResponse,
) -> Option<ConnectorResponseData> {
    let wallet_type = response.payment_method.as_ref().and_then(|payment_method| {
        match payment_method.payment_method_type {
            Some(Shift4ResponsePaymentMethodType::ApplePay) => {
                Some(common_enums::PaymentMethodType::ApplePay)
            }
            Some(Shift4ResponsePaymentMethodType::GooglePay) => {
                Some(common_enums::PaymentMethodType::GooglePay)
            }
            Some(Shift4ResponsePaymentMethodType::Other) | None => None,
        }
    });
    if let Some(wallet_type) = wallet_type {
        return response
            .auth_code
            .clone()
            .map(|auth_code| ConnectorResponseData::with_auth_code(auth_code, wallet_type));
    }

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
#[serde(rename_all = "camelCase")]
pub struct Shift4ResponseCard {
    /// Stored card id (`card_...`).
    pub id: Secret<String>,
    /// Customer the stored card belongs to. `None` when the charge carried no
    /// `customerId`; such a card cannot be charged again.
    pub customer_id: Option<Secret<String>>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4ResponsePaymentMethod {
    /// Payment method id (`pm_...`). A later charge sends it as
    /// `paymentMethod`, together with the owning `customerId`. Optional so that a
    /// payment method shape without an id cannot fail the parse of the whole
    /// charge, which every charge flow shares.
    pub id: Option<Secret<String>>,
    /// Customer that owns the payment method; `None` when no customer does.
    pub customer_id: Option<Secret<String>>,
    #[serde(rename = "type")]
    pub payment_method_type: Option<Shift4ResponsePaymentMethodType>,
    pub status: Option<Shift4PaymentMethodStatus>,
}

/// `paymentMethod.type` on a charge. Only the card wallets are told apart; bank
/// redirects and every other payment method type land on `Other`.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Shift4ResponsePaymentMethodType {
    ApplePay,
    GooglePay,
    #[serde(other)]
    Other,
}

/// `paymentMethod.status`, as documented on Shift4's payment method object. Only
/// `chargeable` can be charged again.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Shift4PaymentMethodStatus {
    /// Ready to be used to create a charge.
    Chargeable,
    /// Setup in progress.
    Pending,
    /// Setup failed.
    Failed,
    /// Already charged and cannot be reused.
    Used,
    /// A status Shift4 has added since; never treated as chargeable.
    #[serde(other)]
    Unknown,
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
            // A CIT that stores the credential (off-session + customer acceptance)
            // is the Authorize twin of SetupMandate, so it surfaces the same
            // mandate: the stored card (`card_...`) or Apple Pay / Google Pay
            // payment method (`pm_...`), which RepeatPayment references together
            // with the owning customer. A plain one-off sale returns no mandate.
            let mandate_reference = item
                .router_data
                .request
                .is_customer_initiated_mandate_payment()
                .then(|| build_shift4_mandate_reference(&item.response))
                .flatten();

            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: get_shift4_redirection_data(&item.response),
                mandate_reference,
                connector_metadata: None,
                network_txn_id: item.response.scheme_transaction_id.clone(),
                network_txn_link_id: None,
                connector_response_reference_id: Some(item.response.id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: item
                    .response
                    .payment_account_reference
                    .clone()
                    .expose_option(),
            })
        };

        let minor_amount_captured = get_shift4_captured_amount(&item.response).and_then(|amount| {
            common_utils::types::MinorUnitForConnector
                .convert_back(amount, item.response.currency)
                .ok()
        });

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                amount_captured: minor_amount_captured
                    .map(domain_types::utils::legacy_amount_as_i64),
                minor_amount_captured,
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
                payment_account_reference: item
                    .response
                    .payment_account_reference
                    .clone()
                    .expose_option(),
            })
        };

        let minor_amount_captured = get_shift4_captured_amount(&item.response).and_then(|amount| {
            common_utils::types::MinorUnitForConnector
                .convert_back(amount, item.response.currency)
                .ok()
        });

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                amount_captured: minor_amount_captured
                    .map(domain_types::utils::legacy_amount_as_i64),
                minor_amount_captured,
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

        // A successful capture reports `Charged`, also after a partial capture.
        // `PartialCharged` cannot be derived here: Shift4 rewrites the charge
        // `amount` to the captured amount and returns no authorized amount (a
        // 1000 authorization captured with 400 reads back `amount: 400`,
        // `amountRefunded: 0`), and the capture request carries no authorized
        // amount either (`PaymentsCaptureData` has none, and the gRPC capture
        // leaves `minor_amount_authorized` / `minor_amount_capturable` unset). A
        // caller that knows the authorized amount can compare it with
        // `amount_to_capture` itself.
        //
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
                payment_account_reference: item
                    .response
                    .payment_account_reference
                    .clone()
                    .expose_option(),
            })
        };

        let status = if matches!(item.response.status, Shift4PaymentStatus::Failed) {
            AttemptStatus::CaptureFailed
        } else {
            status
        };

        // The charge `amount` is the captured amount after a capture, so a
        // partial capture of 400 reports 400 captured.
        let minor_amount_captured = get_shift4_captured_amount(&item.response).and_then(|amount| {
            common_utils::types::MinorUnitForConnector
                .convert_back(amount, item.response.currency)
                .ok()
        });

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                amount_captured: minor_amount_captured
                    .map(domain_types::utils::legacy_amount_as_i64),
                minor_amount_captured,
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
    pub amount: ConnectorMinorUnit,
}

impl TryFrom<&RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>>
    for Shift4RefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
    ) -> Result<Self, Self::Error> {
        use error_stack::ResultExt;
        Ok(Self {
            charge_id: item.request.connector_transaction_id.clone(),
            amount: common_utils::types::MinorUnitForConnector
                .convert(&common_utils::types::Money::from_minor_unit(
                    item.request.minor_refund_amount,
                    item.request.currency,
                ))
                .change_context(IntegrationError::AmountConversionFailed {
                    context: Default::default(),
                })?,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Shift4RefundResponse {
    pub id: String,
    pub amount: ConnectorMinorUnit,
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

/// `POST /charges/{id}/capture` body.
///
/// The API reference lists no capture body, but Shift4 accepts `amount` and settles
/// exactly that amount, releasing the rest of the authorization. Verified in the
/// sandbox (2026-09-15): a 1000 authorization captured with `amount: 400` reads
/// back `amount: 400, captured: true`; an amount above the authorization is refused
/// with HTTP 400 "Invalid Capture data", and a second capture with "Requested Charge
/// is already captured". The requested amount is always sent, so a partial capture
/// can never settle the full authorization.
#[derive(Debug, Serialize)]
pub struct Shift4CaptureRequest {
    pub amount: ConnectorMinorUnit,
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
        let request = &item.router_data.request;

        // Shift4 captures a charge once: the first capture settles the requested
        // amount and releases the rest, and a later capture is refused. A
        // multiple-capture intent can therefore not be honoured and is rejected
        // before any money moves.
        let reject_multiple = |detail: String| {
            error_stack::report!(IntegrationError::NotSupported {
                message: "Multiple partial captures".to_string(),
                connector: "Shift4",
                context: IntegrationErrorContext {
                    additional_context: Some(detail),
                    suggested_action: Some(
                        "Capture once, with the amount to settle; Shift4 releases the rest of \
                         the authorization."
                            .to_string(),
                    ),
                    doc_url: Some("https://dev.shift4.com/docs/api#capture-a-charge".to_string()),
                },
            })
        };

        if request.is_multiple_capture() {
            return Err(reject_multiple(
                "Shift4 captures a charge only once, so a multiple-capture request cannot be \
                 honoured."
                    .to_string(),
            ));
        }
        if let Some(
            method @ (common_enums::CaptureMethod::ManualMultiple
            | common_enums::CaptureMethod::Scheduled),
        ) = &request.capture_method
        {
            return Err(reject_multiple(format!(
                "Shift4 supports only AUTOMATIC and MANUAL capture; a {method} capture needs \
                 more than one capture of the same charge."
            )));
        }

        Ok(Self {
            amount: common_utils::types::MinorUnitForConnector
                .convert(&common_utils::types::Money::from_minor_unit(
                    request.minor_amount_to_capture,
                    request.currency,
                ))
                .change_context(IntegrationError::AmountConversionFailed {
                    context: Default::default(),
                })?,
        })
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
//
// Shift4 has no dedicated MIT / recurring-charge endpoint. A repeat payment is
// an ordinary `POST /charges` (tech-spec "10a. Create a Charge on a Stored
// Card") that references the card SetupMandate stored on a Shift4 customer:
//
// * `card`       — the stored card id (`card_...`), which SetupMandate surfaces
//                  as `connector_mandate_id`;
// * `customerId` — the customer that owns that card (`cust_...`), which
//                  SetupMandate required and surfaces as `connector_customer`.
//
// Both are mandatory. Shift4 documents `customerId` as "required if the charge
// is being created with the customer's existing card" and `card` as "must be an
// existing card that is associated with the customer specified in
// `customerId`"; a stored card sent without its customer is rejected with HTTP
// 400 "Charge using customer's card requires customerId to be provided".
// Sending `customerId` without `card` is not a safe fallback either: it charges
// whatever the customer's *default* card is, not the card the mandate names.
//
// A wallet mandate (tech-spec "10e. Create a Charge on a Stored Wallet Payment
// Method") is the Apple Pay / Google Pay payment method (`pm_...`) that
// SetupMandate or an off-session Authorize stored on the customer. It is sent as
// `paymentMethod` with the same mandatory `customerId`. Shift4 answers HTTP 400
// "Charge using customer's payment method requires customerId to be provided"
// without it, and "Cannot define billing in Payment Method charge" when a
// charge-level `billing` is added.
//
// A network-mandate MIT has no Shift4 credential: it sends the raw card, no
// customer, and the original charge's network transaction id as
// `external.schemeTransactionId` ("external, unique reference of the transaction
// within a payment card network"; accepted in the sandbox on a
// `merchant_initiated` raw-card charge).

/// `POST /charges` body for a merchant-initiated charge.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4RepeatPaymentRequest<T: PaymentMethodDataTypes> {
    pub amount: ConnectorMinorUnit,
    pub currency: Currency,
    /// `true` for automatic capture, `false` for an authorization-only MIT that
    /// is settled later through `POST /charges/{id}/capture`.
    pub captured: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    /// The stored credential charged: `card` or `paymentMethod`.
    #[serde(flatten)]
    pub source: Shift4RepeatPaymentSource<T>,
    /// `merchant_initiated` or `subsequent_recurring`.
    #[serde(rename = "type")]
    pub transaction_type: Shift4TransactionType,
    /// Owner of the stored card or payment method. Always set on the stored-card
    /// and payment-method paths; absent only when raw card details are sent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_id: Option<Secret<String>>,
    /// Charge-level billed-party details, built as on Authorize. Never set on a
    /// `paymentMethod` charge, which Shift4 rejects with a charge-level billing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing: Option<Shift4Billing>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipping: Option<Shift4Shipping>,
    /// Merchant reference (`external.vendorReference`) and, on a network-mandate
    /// MIT, the original charge's network transaction id
    /// (`external.schemeTransactionId`). Omitted when both are empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external: Option<Shift4External>,
}

/// Card field for MIT: either a stored card id or raw card details
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Shift4RepeatPaymentCard<T: PaymentMethodDataTypes> {
    /// Stored card identifier (`card_...`) owned by `customerId`.
    Token(Secret<String>),
    /// Raw card details, used only with a network mandate reference.
    RawCard(Shift4CardData<T>),
}

/// The stored credential a merchant-initiated charge references. Shift4 takes
/// a card and a wallet payment method in different fields.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Shift4RepeatPaymentSource<T: PaymentMethodDataTypes> {
    /// `card`: a stored card id (`card_...`), or raw card details.
    Card { card: Shift4RepeatPaymentCard<T> },
    /// `paymentMethod`: a stored Apple Pay / Google Pay payment method id
    /// (`pm_...`) owned by `customerId`.
    PaymentMethod {
        #[serde(rename = "paymentMethod")]
        payment_method: Secret<String>,
    },
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
    /// An unscheduled charge of a stored credential (top-up, delayed charge, retry).
    MerchantInitiated,
    /// A scheduled charge in the series the `first_recurring` charge started.
    SubsequentRecurring,
}

/// MIT response reuses the standard payments response
pub type Shift4RepeatPaymentResponse = Shift4PaymentsResponse;

fn missing_shift4_mit_field(
    field_name: &'static str,
    additional_context: &str,
    suggested_action: &str,
) -> error_stack::Report<IntegrationError> {
    error_stack::report!(IntegrationError::MissingRequiredField {
        field_name,
        context: IntegrationErrorContext {
            additional_context: Some(additional_context.to_string()),
            suggested_action: Some(suggested_action.to_string()),
            doc_url: Some("https://dev.shift4.com/docs/api#create-a-new-charge".to_string()),
        },
    })
}

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
        let billing_details = item
            .resource_common_data
            .address
            .get_payment_method_billing();

        let (source, customer_id, scheme_transaction_id) = match (
            &item.request.mandate_reference,
            shift4_payment_method_mandate_id(&item.request.mandate_reference),
        ) {
            // A stored Apple Pay / Google Pay payment method (`pm_...`); see
            // `shift4_payment_method_mandate_id` for why the id prefix decides. The
            // mandate is authoritative, so any wallet data on the request is not
            // sent: the MIT carries no token and no inline payment method.
            (MandateReferenceId::ConnectorMandateId(_), Some(payment_method_id)) => {
                let customer_id = shift4_connector_customer(&item.resource_common_data)
                    .ok_or_else(|| {
                        missing_shift4_mit_field(
                            "connector_customer_id",
                            "Shift4 charges a stored payment method only together with a \
                             `customerId` (\"Charge using customer's payment method requires \
                             customerId to be provided\") and refuses any customer other than \
                             its owner, so the charge cannot be sent without it.",
                            "Pass the Shift4 customer id (`cust_...`) the payment method was \
                             stored under as connector_customer_id.",
                        )
                    })?;
                (
                    Shift4RepeatPaymentSource::PaymentMethod {
                        payment_method: Secret::new(payment_method_id),
                    },
                    Some(customer_id),
                    None,
                )
            }
            // A stored card (`card_...`), from SetupMandate or an off-session card
            // Authorize. It is authoritative: a stored card is referenced by id
            // only, so any card details on the request are not sent (the
            // cardholder is not present on an MIT).
            (MandateReferenceId::ConnectorMandateId(connector_mandate_ref), None) => {
                let card_id = connector_mandate_ref
                    .get_connector_mandate_id()
                    .filter(|id| !id.trim().is_empty())
                    .ok_or_else(|| {
                        missing_shift4_mit_field(
                            "connector_recurring_payment_id.connector_mandate_id.connector_mandate_id",
                            "A Shift4 repeat payment charges the stored card (`card_...`) or \
                             payment method (`pm_...`) the mandate names.",
                            "Pass the `connector_mandate_id` returned by SetupRecurring or by \
                             the off-session Authorize.",
                        )
                    })?;

                // Refusing without the customer is deliberate: without it Shift4
                // either rejects the charge or, if `card` were dropped instead,
                // charges the customer's default card rather than the one the
                // mandate names.
                let customer_id = shift4_connector_customer(&item.resource_common_data)
                    .ok_or_else(|| {
                        missing_shift4_mit_field(
                            "connector_customer_id",
                            "Shift4 charges a stored card only together with the customer that \
                             owns it (`customerId` is required when `card` is an existing card \
                             id), so the stored-card charge cannot be sent without it.",
                            "Pass the Shift4 customer id (`cust_...`) the card was stored under \
                             (the `connector_customer_id` returned by SetupRecurring) as \
                             connector_customer_id.",
                        )
                    })?;

                (
                    Shift4RepeatPaymentSource::Card {
                        card: Shift4RepeatPaymentCard::Token(Secret::new(card_id)),
                    },
                    Some(customer_id),
                    None,
                )
            }
            // Network-mandate MIT: the raw card is charged inline with no customer,
            // linked to the original cardholder-initiated charge by its network
            // transaction id in `external.schemeTransactionId`.
            (MandateReferenceId::NetworkMandateId(network_mandate), _) => {
                let network_transaction_id = Some(network_mandate.network_transaction_id.clone())
                    .filter(|id| !id.trim().is_empty())
                    .ok_or_else(|| {
                        missing_shift4_mit_field(
                            "connector_recurring_payment_id.network_mandate_id.network_transaction_id",
                            "A Shift4 network-mandate MIT is linked to the original \
                             cardholder-initiated charge only through its network transaction \
                             id (`external.schemeTransactionId`).",
                            "Pass the network transaction id of the original charge.",
                        )
                    })?;
                match &item.request.payment_method_data {
                    PaymentMethodData::Card(card_data) => (
                        Shift4RepeatPaymentSource::Card {
                            card: Shift4RepeatPaymentCard::RawCard(Shift4CardData::new(
                                card_data,
                                billing_details.and_then(|b| b.address.as_ref()),
                            )),
                        },
                        None,
                        Some(network_transaction_id),
                    ),
                    _ => {
                        return Err(error_stack::report!(IntegrationError::NotSupported {
                            message: "NetworkMandateId without raw card details".to_string(),
                            connector: "Shift4",
                            context: IntegrationErrorContext {
                                additional_context: Some(
                                    "Shift4 charges a network-mandate MIT on the raw card details, \
                                     with the network transaction id in \
                                     `external.schemeTransactionId`. This request carries no raw \
                                     card (for example a wallet, a token or card details for a \
                                     network transaction id), so there is nothing Shift4 can charge."
                                        .to_string(),
                                ),
                                suggested_action: Some(
                                    "Send the raw card details with the NetworkMandateId, or charge \
                                     a stored card or Apple Pay / Google Pay payment method by its \
                                     connector mandate id (`card_...` / `pm_...`) with \
                                     connector_customer_id."
                                        .to_string(),
                                ),
                                doc_url: Some(
                                    "https://dev.shift4.com/docs/api#create-a-new-charge".to_string(),
                                ),
                            },
                        }));
                    }
                }
            }
            (MandateReferenceId::NetworkTokenWithNTI(_), _) => {
                return Err(error_stack::report!(IntegrationError::NotSupported {
                    message: "NetworkTokenWithNTI".to_string(),
                    connector: "Shift4",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Shift4 has no request field for a network token or its cryptogram, \
                             so a network-token MIT cannot be expressed."
                                .to_string(),
                        ),
                        suggested_action: Some(
                            "Charge the stored card or Apple Pay / Google Pay payment method by \
                             its connector mandate id (`card_...` / `pm_...`) with \
                             connector_customer_id, or send the raw card with a NetworkMandateId."
                                .to_string(),
                        ),
                        doc_url: Some(
                            "https://dev.shift4.com/docs/api#create-a-new-charge".to_string(),
                        ),
                    },
                }));
            }
        };

        // Shift4 offers only two merchant-initiated types. A charge in a
        // scheduled series (the setup charge was `first_recurring`) is
        // `subsequent_recurring`; every unscheduled use of the stored credential
        // is `merchant_initiated`.
        let transaction_type = match item.request.mit_category {
            Some(common_enums::MitCategory::Recurring)
            | Some(common_enums::MitCategory::Installment) => {
                Shift4TransactionType::SubsequentRecurring
            }
            Some(common_enums::MitCategory::Unscheduled)
            | Some(common_enums::MitCategory::Resubmission)
            | None => Shift4TransactionType::MerchantInitiated,
        };

        // Same rule as Authorize: Shift4 rejects a captured zero-amount charge
        // ("Zero amount charge cannot be captured"), and no funds move on one.
        let captured =
            item.request.minor_amount != MinorUnit::default() && item.request.is_auto_capture();

        let is_payment_method_charge =
            matches!(source, Shift4RepeatPaymentSource::PaymentMethod { .. });

        // NOT SUPPORTED BY SHIFT4, deliberately dropped rather than approximated
        // (same reasoning as the Authorize builder): `billing_descriptor` and
        // Level 2 / Level 3 data have no field on `POST /charges`.
        Ok(Self {
            amount: common_utils::types::MinorUnitForConnector
                .convert(&common_utils::types::Money::from_minor_unit(
                    item.request.minor_amount,
                    item.request.currency,
                ))
                .change_context(IntegrationError::AmountConversionFailed {
                    context: Default::default(),
                })?,
            currency: item.request.currency,
            captured,
            description: item.resource_common_data.description.clone(),
            metadata: item.request.metadata.clone().expose_option(),
            source,
            transaction_type,
            customer_id,
            // A payment method keeps the billing it was stored with; Shift4 rejects
            // a charge-level `billing` on a `paymentMethod` charge.
            billing: if is_payment_method_charge {
                None
            } else {
                build_shift4_billing(billing_details, item.request.email.as_ref())
            },
            shipping: build_shift4_shipping(item.resource_common_data.address.get_shipping()),
            // A stored card or payment method is linked to its credential through
            // `customerId` + `card` / `paymentMethod`; only the network-mandate MIT
            // carries a scheme transaction id.
            external: build_shift4_external(
                &item.resource_common_data.connector_request_reference_id,
                scheme_transaction_id,
            ),
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

// RepeatPayment Response transformation — the charge object shared with
// Authorize, mapped through the same status table and decline builder.
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
            // The charged credential stays on file, so the same mandate is handed
            // back for the next MIT, but only while a customer owns it: a raw-card
            // network-mandate charge returns a card no customer owns, which can
            // never be charged again and is not offered as a mandate.
            let mandate_reference = build_shift4_mandate_reference(&item.response);

            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: get_shift4_redirection_data(&item.response),
                mandate_reference,
                connector_metadata: None,
                network_txn_id: item.response.scheme_transaction_id.clone(),
                network_txn_link_id: None,
                connector_response_reference_id: Some(item.response.id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: item
                    .response
                    .payment_account_reference
                    .clone()
                    .expose_option(),
            })
        };

        let minor_amount_captured = get_shift4_captured_amount(&item.response).and_then(|amount| {
            common_utils::types::MinorUnitForConnector
                .convert_back(amount, item.response.currency)
                .ok()
        });

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                amount_captured: minor_amount_captured
                    .map(domain_types::utils::legacy_amount_as_i64),
                minor_amount_captured,
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
    pub amount: ConnectorMinorUnit,
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
        use error_stack::ResultExt;
        let router_data = item.router_data;

        Ok(Self {
            line_items: vec![Shift4LineItem {
                product: Shift4InlineProduct {
                    name: "Payment".to_string(),
                    amount: common_utils::types::MinorUnitForConnector
                        .convert(&common_utils::types::Money::from_minor_unit(
                            router_data.request.amount,
                            router_data.request.currency,
                        ))
                        .change_context(IntegrationError::AmountConversionFailed {
                            context: Default::default(),
                        })?,
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
    pub amount: ConnectorMinorUnit,
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
        use error_stack::ResultExt;
        Ok(Self {
            amount: common_utils::types::MinorUnitForConnector
                .convert(&common_utils::types::Money::from_minor_unit(
                    item.router_data.request.minor_amount,
                    item.router_data.request.currency,
                ))
                .change_context(IntegrationError::AmountConversionFailed {
                    context: Default::default(),
                })?,
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
// Shift4 has no dedicated mandate / setup-intent resource. A card or an Apple Pay
// / Google Pay payment method is put on file with a `POST /charges` typed
// `first_recurring` and assigned to a Shift4 customer through `customerId`
// (tech-spec "SetupMandate (Card)", Sequence A). The stored card's `card.id`
// (`card_...`) or the payment method's `paymentMethod.id` (`pm_...`) is surfaced
// as the `connector_mandate_id` and the owning customer as `connector_customer`:
// a later RepeatPayment (MIT) has to send both.
//
// Customer-Initiated Transaction (CIT): the cardholder is present and consents
// to storing the credential. The caller's amount is sent as-is. A `0` amount is
// a verification: it is always sent with `captured: false`, and its successful
// charge is reported `Charged`. Any other amount is always sent with
// `captured: true`, because a SetupRecurring request carries no capture method
// (the gRPC request has none, so the domain `capture_method` is always unset).
// When Shift4 reports the charge as captured, the response reports the captured
// amount (`get_shift4_captured_amount`).
//
// There is deliberately no embedded `customer` object: Shift4 rejects one with
// HTTP 400 "Unable to parse request - unrecognized field: customer" (verified
// against api.shift4.com). A customer is created with `POST /customers`
// (CreateConnectorCustomer) and referenced by id.

/// `POST /charges` body for a card or wallet setup.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4SetupMandateRequest<T: PaymentMethodDataTypes> {
    pub amount: ConnectorMinorUnit,
    pub currency: Currency,
    /// `false` for a zero-amount verification, `true` for any other amount.
    pub captured: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    /// Shift4 customer (`cust_...`) the charge and its stored card or payment
    /// method are assigned to. Mandatory for a setup: a credential no customer
    /// owns cannot be charged again.
    pub customer_id: Secret<String>,
    /// Always `first_recurring`: the cardholder-present charge that establishes
    /// the credential on file for later merchant-initiated use.
    #[serde(rename = "type")]
    pub transaction_type: Shift4TransactionType,
    /// Charge-level billed-party details (two-letter country), built as on
    /// Authorize. Not set on a wallet setup, whose billing goes on
    /// `paymentMethod.billing`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing: Option<Shift4Billing>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipping: Option<Shift4Shipping>,
    /// Merchant reference in `external.vendorReference`, Shift4's only
    /// merchant-controlled reference field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external: Option<Shift4External>,
    #[serde(flatten)]
    pub payment_method: Shift4PaymentMethod<T>,
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
        let amount = common_utils::types::MinorUnitForConnector
            .convert(&common_utils::types::Money::from_minor_unit(item.request.minor_amount.ok_or_else(|| {
                    error_stack::report!(IntegrationError::MissingRequiredField {
                        field_name: "amount",
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "Shift4 has no default charge amount: `POST /charges` requires an \
                                 explicit `amount`, and `0` is not a stand-in for \"unset\" but a \
                                 distinct instruction to run a card-on-file verification that stores \
                                 the credential without moving funds. Defaulting a missing amount to \
                                 `0` would silently turn a payment into a verification, so the setup \
                                 is refused before anything reaches Shift4."
                                    .to_string(),
                            ),
                            suggested_action: Some(
                                "Set the amount on the SetupRecurring request: the minor-unit amount \
                                 to charge while storing the credential, or `0` to store it with a \
                                 zero-amount card-on-file verification."
                                    .to_string(),
                            ),
                            doc_url: Some(
                                "https://dev.shift4.com/docs/api#create-a-new-charge".to_string(),
                            ),
                        },
                    })
                })?, item.request.currency))
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;
        let billing_details = item
            .resource_common_data
            .address
            .get_payment_method_billing();

        let payment_method = match &item.request.payment_method_data {
            PaymentMethodData::Card(card_data) => Shift4PaymentMethod::Card(Shift4CardPayment {
                card: Shift4CardData::new(
                    card_data,
                    billing_details.and_then(|billing| billing.address.as_ref()),
                ),
            }),
            PaymentMethodData::PaymentMethodToken(pmt) => {
                Shift4PaymentMethod::TokenPayment(Shift4TokenPayment {
                    card: pmt.token.clone(),
                })
            }
            PaymentMethodData::Wallet(wallet_data) => {
                Shift4PaymentMethod::Wallet(Shift4WalletPayment {
                    payment_method: Shift4WalletPaymentMethod::try_new(
                        wallet_data,
                        item.request.payment_method_type,
                        build_shift4_billing(billing_details, item.request.email.as_ref()),
                        "SetupMandate",
                    )?,
                })
            }
            _ => {
                return Err(error_stack::report!(IntegrationError::NotSupported {
                    message: "Payment method not supported for SetupMandate".to_string(),
                    connector: "Shift4",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "A Shift4 SetupRecurring stores a credential the later \
                             merchant-initiated charge can reference by id, so it only accepts \
                             the payment methods Shift4 files under a customer: a raw card, a \
                             Shift4 card token (`tok_...`), and Apple Pay / Google Pay, which \
                             is stored as a payment method (`pm_...`). Shift4's other payment \
                             methods — the bank redirects (iDEAL, EPS) among them — are \
                             single-use and leave nothing chargeable behind, so there would be \
                             no mandate to hand back."
                                .to_string(),
                        ),
                        suggested_action: Some(
                            "Send a card, a Shift4 card token, or Apple Pay / Google Pay on \
                             SetupRecurring. To take a one-off payment with another payment \
                             method, use Authorize instead."
                                .to_string(),
                        ),
                        doc_url: Some(
                            "https://dev.shift4.com/docs/api#create-a-new-charge".to_string(),
                        ),
                    },
                }))
            }
        };

        // `customerId` is the Shift4 customer identifier, taken only from
        // `connector_customer` (populated after a CreateConnectorCustomer call),
        // never from the merchant-side `request.customer_id`.
        //
        // It is mandatory here. Shift4 charges a stored card or payment method
        // only together with its owning customer: `POST /charges` with
        // `card: "card_..."` and no `customerId` fails with HTTP 400 "Charge
        // using customer's card requires customerId to be provided" (verified
        // against api.shift4.com). A setup without a customer would store a
        // credential every later MIT is rejected on, so it is refused before
        // anything reaches Shift4.
        let is_wallet_setup = matches!(payment_method, Shift4PaymentMethod::Wallet(_));
        let customer_id =
            shift4_connector_customer(&item.resource_common_data).ok_or_else(|| {
                missing_shift4_connector_customer(
                    if is_wallet_setup {
                        SHIFT4_WALLET_MANDATE_NEEDS_CUSTOMER
                    } else {
                        SHIFT4_CARD_MANDATE_NEEDS_CUSTOMER
                    },
                    SHIFT4_CREATE_CUSTOMER_DOC,
                )
            })?;

        // A zero-amount setup is a verification. Shift4 refuses to capture a zero
        // amount ("Zero amount charge cannot be captured"), so it is always sent
        // uncaptured. A non-zero setup is a real payment that also stores the
        // credential, and it is always captured: the SetupRecurring contract has no
        // capture method (the gRPC request carries none, so it is always unset),
        // and reporting a setup as completed while Shift4 holds an uncaptured
        // authorization would mean the money never settles. Shift4 accepts
        // `captured: true` on a `first_recurring` charge and still stores the card
        // or payment method under the customer (sandbox, card and Apple Pay,
        // followed by a successful MIT on the stored credential).
        let captured = amount != ConnectorMinorUnit::default();

        // NOT SUPPORTED BY SHIFT4, deliberately dropped rather than approximated
        // (same reasoning as the Authorize builder):
        // * `billing_descriptor` — there is no statement-descriptor field on
        //   `POST /charges`, and `description` never reaches the cardholder.
        Ok(Self {
            amount,
            currency: item.request.currency,
            captured,
            description: item.resource_common_data.description.clone(),
            metadata: item.request.metadata.clone().expose_option(),
            customer_id,
            transaction_type: Shift4TransactionType::FirstRecurring,
            // A wallet setup sends billing on `paymentMethod.billing` instead.
            billing: if is_wallet_setup {
                None
            } else {
                build_shift4_billing(billing_details, item.request.email.as_ref())
            },
            shipping: build_shift4_shipping(item.resource_common_data.address.get_shipping()),
            external: build_shift4_external(
                &item.resource_common_data.connector_request_reference_id,
                None,
            ),
            payment_method,
        })
    }
}

// SetupMandate Response transformation - reuses Shift4PaymentsResponse and the
// shared status table, and extracts the stored credential (`card.id` or
// `paymentMethod.id`) as the mandate plus its owning customer.
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

        // A zero-amount setup is a verification that holds no funds and can never
        // be captured, so its successful uncaptured charge is the completed setup:
        // `Authorized` is reported as `Charged`, for a card and a wallet alike. A
        // non-zero setup is sent captured, so it keeps the status Shift4 reports
        // for that charge (`Charged` on success).
        if status == AttemptStatus::Authorized
            && item.router_data.request.minor_amount == Some(MinorUnit::default())
        {
            status = AttemptStatus::Charged;
        }

        // Redirect target, if Shift4 asks for one.
        let redirection_data = get_shift4_redirection_data(&item.response);
        let connector_response = build_shift4_connector_response(&item.response);

        let response = if matches!(item.response.status, Shift4PaymentStatus::Failed) {
            // Shift4 sets `failureCode` / `failureMessage` (and the three
            // network codes) on declined charges, so the merchant sees the
            // actual decline reason rather than a static sentinel.
            Err(build_shift4_failure_response(
                &item.response,
                item.http_code,
                FlowStatus::Payment(status),
            ))
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data,
                mandate_reference: build_shift4_mandate_reference(&item.response),
                connector_metadata: None,
                // The initial CIT's scheme transaction id — what a later
                // MIT quotes for credential-on-file continuity.
                network_txn_id: item.response.scheme_transaction_id.clone(),
                network_txn_link_id: None,
                // Shift4 PSync hits `GET /charges/{id}` with the
                // charge id, so surfacing it here lets sync flows
                // look up this attempt.
                connector_response_reference_id: Some(item.response.id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: item
                    .response
                    .payment_account_reference
                    .clone()
                    .expose_option(),
            })
        };

        // Propagate the customer that owns the stored credential so the subsequent
        // RepeatPayment (MIT) can send `customerId` alongside the card or payment
        // method id; Shift4 rejects a stored-credential charge without it. Shift4
        // reports it flat as `customerId` and again on the card or payment method;
        // the request's own value is the last resort.
        let connector_customer = item
            .response
            .customer_id
            .clone()
            .or_else(|| {
                item.response
                    .card
                    .as_ref()
                    .and_then(|card| card.customer_id.clone())
            })
            .or_else(|| {
                item.response
                    .payment_method
                    .as_ref()
                    .and_then(|payment_method| payment_method.customer_id.clone())
            })
            .expose_option()
            .or_else(|| {
                item.router_data
                    .resource_common_data
                    .connector_customer
                    .clone()
            });

        // A non-zero setup is sent captured, so its settled amount is reported
        // like any other captured charge's. A zero-amount verification is sent
        // uncaptured and reports none, although its status is `Charged`.
        let minor_amount_captured = get_shift4_captured_amount(&item.response).and_then(|amount| {
            common_utils::types::MinorUnitForConnector
                .convert_back(amount, item.response.currency)
                .ok()
        });

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                amount_captured: minor_amount_captured
                    .map(domain_types::utils::legacy_amount_as_i64),
                minor_amount_captured,
                connector_customer,
                connector_response,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}
