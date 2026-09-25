use std::collections::HashMap;

use common_enums::{AttemptStatus, AuthenticationType, CaptureMethod, RefundStatus};
use common_utils::{
    types::{MinorUnit, StringMinorUnit},
    Method,
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, PSync, PreAuthenticate, RSync, Refund, RepeatPayment, SetupMandate,
        VerifyWebhookSource, Void,
    },
    connector_types::{
        EventType, MandateReference, MandateReferenceId, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsPreAuthenticateData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, RepeatPaymentData, ResponseId, SetupMandateRequestData,
        VerifyWebhookSourceFlowData,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_request_types::VerifyWebhookSourceRequestData,
    router_response_types::{RedirectForm, VerifyWebhookSourceResponseData, VerifyWebhookStatus},
};
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::connectors::saferpay::{SaferpayAmountConvertor, SaferpayRouterData};
use crate::types::ResponseRouterData;

/// Saferpay JSON API contract version this integration is pinned to. Every
/// `RequestHeader` must carry it; bumping it changes the wire contract.
pub const SAFERPAY_SPEC_VERSION: &str = "1.44";
const SAFERPAY_TRANSACTION_DOC_URL: &str =
    "https://docs.saferpay.com/home/open-api-specification-beta/transaction";

/// `RetryIndicator` for a first attempt. Saferpay allows 0-9, incremented per retry
/// of the *same* `RequestId`. The caller supplies the stable logical request id.
const RETRY_INDICATOR_FIRST_ATTEMPT: u8 = 0;

/// `Payment.Description` is documented as optional but the Saferpay Backoffice needs
/// a value to render the transaction, so a fallback is always sent.
const DEFAULT_PAYMENT_DESCRIPTION: &str = "Payment";

/// Saferpay caps `OrderId` at 80 characters and rejects longer values outright.
const ORDER_ID_MAX_LEN: usize = 80;

/// `Transaction.Type` — which leg of the payment a transaction object describes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SaferpayTransactionType {
    #[serde(rename = "PAYMENT")]
    Payment,
    #[serde(rename = "REFUND")]
    Refund,
    /// Anything Saferpay adds later. Kept so an unknown type is reported rather
    /// than failing the whole response parse.
    #[serde(other)]
    Unknown,
}

/// `Transaction.Status`. Saferpay spells the cancelled state `CANCELED` with a
/// single `L`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SaferpayTransactionStatus {
    #[serde(rename = "AUTHORIZED")]
    Authorized,
    #[serde(rename = "CAPTURED")]
    Captured,
    #[serde(rename = "PENDING")]
    Pending,
    #[serde(rename = "CANCELED")]
    Canceled,
    /// Unrecognised status. Treated as pending rather than rejected, so a status
    /// Saferpay introduces later degrades to "still in flight" instead of
    /// failing the response.
    #[serde(other)]
    Unknown,
}

/// Key under which the Capture flow publishes the `CaptureId` in
/// `connector_metadata`, and under which the Refund flow expects to find it in
/// `RefundsData::refund_connector_metadata`.
pub const CAPTURE_ID_METADATA_KEY: &str = "capture_id";
/// Authorized total used to distinguish a full Capture from a partial Capture.
pub const AUTHORIZED_AMOUNT_METADATA_KEY: &str = "authorized_amount";
/// Marks which leg of the flow last wrote the metadata blob.
pub const SAFERPAY_STAGE_METADATA_KEY: &str = "stage";
/// Stage written by the 3DS `Initialize` leg, while the session token is live.
pub const STAGE_INITIALIZED: &str = "initialized";
/// Stage written after RSync has captured the refund transaction successfully.
pub const STAGE_REFUND_SETTLED: &str = "refund_settled";
/// Key under which the 3DS `Initialize` response publishes the Saferpay session
/// token in `connector_metadata`.
pub const SAFERPAY_TOKEN_METADATA_KEY: &str = "saferpay_token";

fn context() -> IntegrationErrorContext {
    IntegrationErrorContext {
        additional_context: Some("while building a Saferpay Transaction API request".to_string()),
        suggested_action: Some(
            "Check the required Saferpay fields and the connector flow configuration".to_string(),
        ),
        doc_url: Some(SAFERPAY_TRANSACTION_DOC_URL.to_string()),
    }
}

fn missing_field(field_name: &'static str) -> error_stack::Report<IntegrationError> {
    error_stack::report!(IntegrationError::MissingRequiredField {
        field_name,
        context: context(),
    })
}

fn not_supported(message: String) -> error_stack::Report<IntegrationError> {
    error_stack::report!(IntegrationError::NotSupported {
        message,
        connector: "saferpay",
        context: context(),
    })
}

fn capture_method_not_supported(method: CaptureMethod) -> error_stack::Report<IntegrationError> {
    error_stack::report!(IntegrationError::CaptureMethodNotSupported {
        context: IntegrationErrorContext {
            // The framework renders this as "<context> is not implemented", so keep it
            // a noun phrase.
            additional_context: Some(format!(
                "{method} capture on saferpay, whose Transaction interface has no sale \
                 mode — an authorization is always settled by an explicit Capture —"
            )),
            suggested_action: Some(
                "Set capture_method = manual and issue the Capture explicitly".to_string(),
            ),
            doc_url: Some(SAFERPAY_TRANSACTION_DOC_URL.to_string()),
        },
    })
}

/// Saferpay `OrderId` is limited to 80 characters; UCS references can be longer.
fn truncate_order_id(reference: &str) -> Option<String> {
    if reference.is_empty() {
        return None;
    }
    Some(reference.chars().take(ORDER_ID_MAX_LEN).collect())
}

// =============================================================================
// AUTH
// =============================================================================

/// Saferpay credentials.
///
/// `username` / `password` form the HTTP Basic header; `customer_id` and
/// `terminal_id` are **not** headers — they travel inside the JSON body
/// (`RequestHeader.CustomerId` and the top-level `TerminalId` respectively).
#[derive(Debug, Clone)]
pub struct SaferpayAuthType {
    pub username: Secret<String>,
    pub password: Secret<String>,
    pub customer_id: Secret<String>,
    pub terminal_id: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for SaferpayAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Saferpay {
                api_key,
                key1,
                api_secret,
                key2,
                ..
            } => Ok(Self {
                username: api_key.to_owned(),
                password: key1.to_owned(),
                customer_id: api_secret.to_owned(),
                terminal_id: key2.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType { context: context() }
            )),
        }
    }
}

impl SaferpayAuthType {
    /// `Authorization: Basic base64(username:password)`.
    pub fn basic_auth_value(&self) -> String {
        use base64::Engine;
        let raw = format!("{}:{}", self.username.peek(), self.password.peek());
        format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(raw)
        )
    }
}

// =============================================================================
// SHARED ENVELOPE
// =============================================================================

/// Mandatory envelope on every Saferpay request.
///
/// `RequestId` identifies one logical connector request. Retries must reuse it so
/// Saferpay can recognize the retry instead of executing the operation twice.
#[derive(Debug, Clone, Serialize)]
pub struct SaferpayRequestHeader {
    #[serde(rename = "SpecVersion")]
    pub spec_version: String,
    #[serde(rename = "CustomerId")]
    pub customer_id: Secret<String>,
    #[serde(rename = "RequestId")]
    pub request_id: String,
    #[serde(rename = "RetryIndicator")]
    pub retry_indicator: u8,
}

impl SaferpayRequestHeader {
    fn new(auth: &SaferpayAuthType, request_id: String) -> Self {
        Self {
            spec_version: SAFERPAY_SPEC_VERSION.to_string(),
            customer_id: auth.customer_id.clone(),
            request_id,
            retry_indicator: RETRY_INDICATOR_FIRST_ATTEMPT,
        }
    }
}

fn stable_request_id(
    merchant_request_id: Option<&str>,
    connector_request_reference_id: &str,
) -> String {
    let id = merchant_request_id
        .map(str::trim)
        .filter(|request_id| !request_id.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| connector_request_reference_id.trim().to_string());
    if id.is_empty() {
        // Saferpay rejects an empty `RequestId` outright ("The RequestId field is
        // required."), and neither caller-supplied id source carries a value here
        // — mint one rather than ship a request that is guaranteed to fail.
        common_utils::fp_utils::generate_uuid_v4()
    } else {
        id
    }
}

fn payment_request_id(common: &PaymentFlowData) -> String {
    stable_request_id(
        common.merchant_request_id.as_deref(),
        &common.connector_request_reference_id,
    )
}

fn refund_request_id(common: &RefundFlowData) -> String {
    stable_request_id(
        common.merchant_request_id.as_deref(),
        &common.connector_request_reference_id,
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaferpayResponseHeader {
    #[serde(rename = "SpecVersion")]
    pub spec_version: Option<String>,
    #[serde(rename = "RequestId")]
    pub request_id: Option<String>,
}

/// `Amount.Value` is a **string** in minor units — Saferpay rejects a numeric value.
#[derive(Debug, Clone, Serialize)]
pub struct SaferpayAmount {
    #[serde(rename = "Value")]
    pub value: StringMinorUnit,
    #[serde(rename = "CurrencyCode")]
    pub currency_code: common_enums::Currency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaferpayAmountResponse {
    #[serde(rename = "Value")]
    pub value: Option<String>,
    #[serde(rename = "CurrencyCode")]
    pub currency_code: Option<String>,
}

/// Names an existing transaction (payment **or** refund leg) by its Saferpay id.
#[derive(Debug, Clone, Serialize)]
pub struct SaferpayTransactionReference {
    #[serde(rename = "TransactionId")]
    pub transaction_id: String,
}

/// Names a **capture**, which is what a referenced refund must point at since
/// SpecVersion 1.10 — passing a `TransactionId` here fails.
#[derive(Debug, Clone, Serialize)]
pub struct SaferpayCaptureReference {
    #[serde(rename = "CaptureId")]
    pub capture_id: String,
}

// =============================================================================
// AUTHORIZE REQUEST (`AuthorizeDirect` and `Initialize`)
// =============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct SaferpayPaymentDetails {
    #[serde(rename = "Amount")]
    pub amount: SaferpayAmount,
    #[serde(rename = "OrderId", skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
    #[serde(rename = "Description")]
    pub description: String,
}

/// `ExpYear` and `ExpMonth` are JSON **numbers**, not strings — the single most
/// common Saferpay integration trap.
#[derive(Debug, Clone, Serialize)]
pub struct SaferpayCardDetails<T: PaymentMethodDataTypes> {
    #[serde(rename = "Number")]
    pub number: RawCardNumber<T>,
    #[serde(rename = "ExpYear")]
    pub exp_year: Secret<u16>,
    #[serde(rename = "ExpMonth")]
    pub exp_month: Secret<u8>,
    #[serde(rename = "VerificationCode", skip_serializing_if = "Option::is_none")]
    pub verification_code: Option<Secret<String>>,
    #[serde(rename = "HolderName", skip_serializing_if = "Option::is_none")]
    pub holder_name: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SaferpayPaymentMeans<T: PaymentMethodDataTypes> {
    #[serde(rename = "Card")]
    pub card: SaferpayCardDetails<T>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SaferpayReturnUrl {
    #[serde(rename = "Url")]
    pub url: String,
}

/// `ThreeDsChallenge: FORCE` makes the challenged flow deterministic instead of
/// leaving frictionless-vs-challenge to the issuer.
#[derive(Debug, Clone, Serialize)]
pub struct SaferpayAuthentication {
    #[serde(rename = "ThreeDsChallenge")]
    pub three_ds_challenge: &'static str,
}

/// Body for `POST /Payment/v1/Transaction/AuthorizeDirect` (non-3DS) and
/// `POST /Payment/v1/Transaction/Initialize` (3DS). The two differ only by the
/// presence of `ReturnUrl` / `Authentication`; `TerminalId` is required on both and
/// on no other endpoint.
#[derive(Debug, Clone, Serialize)]
pub struct SaferpayCardAuthorizationRequest<T: PaymentMethodDataTypes> {
    #[serde(rename = "RequestHeader")]
    pub request_header: SaferpayRequestHeader,
    #[serde(rename = "TerminalId")]
    pub terminal_id: Secret<String>,
    #[serde(rename = "Payment")]
    pub payment: SaferpayPaymentDetails,
    #[serde(rename = "PaymentMeans")]
    pub payment_means: SaferpayPaymentMeans<T>,
    #[serde(rename = "ReturnUrl", skip_serializing_if = "Option::is_none")]
    pub return_url: Option<SaferpayReturnUrl>,
    #[serde(rename = "Authentication", skip_serializing_if = "Option::is_none")]
    pub authentication: Option<SaferpayAuthentication>,
}

/// The Authorize flow speaks to two different endpoints.
///
/// * Non-3DS: `AuthorizeDirect` with the raw card.
/// * 3DS settle: `Authorize {Token}`, finalising the journey `PreAuthenticate` opened.
///
/// Either way this flow is where the money moves. See `is_three_ds_settlement`.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum SaferpayAuthorizeRequest<T: PaymentMethodDataTypes> {
    /// `POST /Payment/v1/Transaction/AuthorizeDirect`
    Direct(Box<SaferpayCardAuthorizationRequest<T>>),
    /// `POST /Payment/v1/Transaction/Authorize`
    Settle {
        #[serde(rename = "RequestHeader")]
        request_header: SaferpayRequestHeader,
        /// The `Initialize` session token. Presenting it is what finalises and
        /// authorizes the payment, so it is a bearer credential.
        #[serde(rename = "Token")]
        token: Secret<String>,
    },
}

type AuthorizeRouterData<T> =
    RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>;

/// `true` when this Authorize is the 3DS settle leg rather than a fresh charge.
///
/// `PreAuthenticate` opens the journey with `Initialize` and returns a redirect; the caller
/// stops there (`should_continue_after_preauthenticate` defaults to false), so a 3DS attempt
/// never reaches Authorize before the shopper has been away. When it comes back, the caller
/// re-enters through complete-authorize with `redirect_response` populated — that is the
/// signal, and it is the same one Ilixium keys on (`ilixium::is_three_ds_completion`).
pub fn is_three_ds_settlement<T: PaymentMethodDataTypes>(
    request: &PaymentsAuthorizeData<T>,
) -> bool {
    request.redirect_response.is_some()
}

/// The Saferpay session `Token` published by `PreAuthenticate`.
///
/// It rides in `connector_metadata`, which the caller persists on the attempt and hands back
/// on the settle Authorize as `connector_feature_data` — the same channel Paysafe uses to
/// carry its payment handle across a redirect.
fn settle_token<T: PaymentMethodDataTypes>(request: &PaymentsAuthorizeData<T>) -> Option<String> {
    request
        .connector_feature_data
        .as_ref()
        .and_then(|metadata| {
            metadata
                .peek()
                .get(SAFERPAY_TOKEN_METADATA_KEY)
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|token| !token.is_empty())
                .map(str::to_string)
        })
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<SaferpayRouterData<AuthorizeRouterData<T>, T>> for SaferpayAuthorizeRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: SaferpayRouterData<AuthorizeRouterData<T>, T>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;
        let common = &router_data.resource_common_data;
        let auth = SaferpayAuthType::try_from(&router_data.connector_config)?;

        // 3DS settle: spend the session token PreAuthenticate obtained. The card is not
        // read here — Saferpay needs only the token — even though the caller rehydrates it
        // from the temp locker to satisfy the framework's payment-method requirement.
        if is_three_ds_settlement(request) {
            let token = settle_token(request)
                .ok_or_else(|| missing_field("connector_feature_data.saferpay_token"))?;
            return Ok(Self::Settle {
                request_header: SaferpayRequestHeader::new(&auth, payment_request_id(common)),
                token: Secret::new(token),
            });
        }

        let card = match &request.payment_method_data {
            PaymentMethodData::Card(card) => card,
            _ => {
                return Err(error_stack::report!(IntegrationError::NotImplemented(
                    "Only card payments are supported by saferpay".to_string(),
                    context(),
                )))
            }
        };

        // Saferpay has no sale mode. There is no capture field on `AuthorizeDirect` or
        // `Initialize`, no combined authorize+capture endpoint anywhere in the Payment
        // API, and no terminal-level capture setting — the only "auto" switch on a
        // terminal is `AutoCloseDailyStatement`, which closes the daily batch of
        // *already captured* transactions. Saferpay also ignores unknown request fields
        // silently, so there is nothing to send speculatively either.
        //
        // Accepting `Automatic` would therefore mean reporting `requires_capture` and
        // leaving the money unmoved with no error, until the authorization expires.
        // Refuse it instead, so the caller finds out at authorize time.
        //
        // Evaluated before every other intent guard: the capture-method refusal is
        // the deterministic first answer — a request carrying both an automatic
        // capture method and external 3DS data is refused for the capture method,
        // which no reordering of the other fields can fix.
        if let Some(method @ (CaptureMethod::Automatic | CaptureMethod::SequentialAutomatic)) =
            &request.capture_method
        {
            return Err(capture_method_not_supported(*method));
        }

        // Saferpay's Transaction interface has no field to carry an externally
        // obtained CAVV/ECI/dsTransId: 3DS is always run by Saferpay itself through
        // the PreAuthenticate redirect, so merchant-supplied authentication data
        // cannot be honoured. (The 3DS settle leg was handled above.)
        if request.authentication_data.is_some() {
            return Err(not_supported(
                "External/merchant-provided 3DS authentication data".to_string(),
            ));
        }

        // Setting up a NEW mandate on Authorize is out of scope: the SetupMandate
        // flow (Alias/InsertDirect) owns registration, and RepeatPayment charges
        // it. Asking this connector to switch flows mid-call (mandate setup on the
        // Authorize RPC, or an MIT charge on the CIT RPC) mixes the SetupMandate /
        // RepeatPayment contracts into Authorize.
        if request.mandate_id.is_some() || request.setup_mandate_details.is_some() {
            return Err(not_supported(
                "Mandate setup or mandate usage inside the Authorize flow — use the \
                 SetupMandate / RepeatPayment flows instead"
                    .to_string(),
            ));
        }

        let exp_year = card
            .get_expiry_year_4_digit()
            .expose()
            .parse::<u16>()
            .map_err(|_| {
                error_stack::report!(IntegrationError::InvalidDataFormat {
                    field_name: "card_exp_year",
                    context: context(),
                })
            })?;
        let exp_month = card.card_exp_month.peek().parse::<u8>().map_err(|_| {
            error_stack::report!(IntegrationError::InvalidDataFormat {
                field_name: "card_exp_month",
                context: context(),
            })
        })?;

        let amount = SaferpayAmountConvertor::convert(request.minor_amount, request.currency)?;

        let description = common
            .description
            .clone()
            .unwrap_or_else(|| DEFAULT_PAYMENT_DESCRIPTION.to_string());

        // A 3DS attempt never reaches this flow for the initial charge — PreAuthenticate
        // owns `Initialize`. Reaching here with ThreeDs means the authentication legs did
        // not run, which on the hyperswitch path means the connector's
        // `is_pre_authentication_flow_required` predicate is not wired up. Failing loudly
        // beats silently charging without authentication.
        if common.auth_type == AuthenticationType::ThreeDs {
            return Err(not_supported(
                "3DS on AuthorizeDirect — the PreAuthenticate leg must run first".to_string(),
            ));
        }

        Ok(Self::Direct(Box::new(SaferpayCardAuthorizationRequest {
            request_header: SaferpayRequestHeader::new(&auth, payment_request_id(common)),
            terminal_id: auth.terminal_id.clone(),
            payment: SaferpayPaymentDetails {
                amount: SaferpayAmount {
                    value: amount,
                    currency_code: request.currency,
                },
                order_id: truncate_order_id(&common.connector_request_reference_id),
                description,
            },
            payment_means: SaferpayPaymentMeans {
                card: SaferpayCardDetails {
                    number: card.card_number.clone(),
                    exp_year: Secret::new(exp_year),
                    exp_month: Secret::new(exp_month),
                    verification_code: Some(card.card_cvc.clone()),
                    holder_name: card.get_optional_cardholder_name(),
                },
            },
            return_url: None,
            authentication: None,
        })))
    }
}

// =============================================================================
// SHARED RESPONSE SHAPES
// =============================================================================

/// The `Transaction` object returned by `AuthorizeDirect`, the token-based
/// `Authorize`, `Refund` and `Inquire`. `Type` discriminates a payment leg from a
/// refund leg; `CaptureId` only appears once the leg has been captured.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaferpayTransaction {
    #[serde(rename = "Type")]
    pub transaction_type: Option<SaferpayTransactionType>,
    #[serde(rename = "Status")]
    pub status: Option<SaferpayTransactionStatus>,
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "CaptureId")]
    pub capture_id: Option<String>,
    #[serde(rename = "Date")]
    pub date: Option<String>,
    #[serde(rename = "Amount")]
    pub amount: Option<SaferpayAmountResponse>,
    #[serde(rename = "OrderId")]
    pub order_id: Option<String>,
    #[serde(rename = "AcquirerName")]
    pub acquirer_name: Option<String>,
    #[serde(rename = "AcquirerReference")]
    pub acquirer_reference: Option<String>,
    #[serde(rename = "SixTransactionReference")]
    pub six_transaction_reference: Option<String>,
    #[serde(rename = "ApprovalCode")]
    pub approval_code: Option<String>,
}

impl SaferpayTransaction {
    fn transaction_status(&self) -> SaferpayTransactionStatus {
        self.status.unwrap_or(SaferpayTransactionStatus::Unknown)
    }

    /// Payment-leg status. Saferpay never auto-captures on this interface: an
    /// `AuthorizeDirect` / token `Authorize` always answers `AUTHORIZED`, and the
    /// caller settles with an explicit Capture regardless of `capture_method`.
    fn attempt_status(&self) -> AttemptStatus {
        match self.transaction_status() {
            SaferpayTransactionStatus::Authorized => AttemptStatus::Authorized,
            SaferpayTransactionStatus::Captured => AttemptStatus::Charged,
            SaferpayTransactionStatus::Canceled => AttemptStatus::Voided,
            SaferpayTransactionStatus::Pending | SaferpayTransactionStatus::Unknown => {
                AttemptStatus::Pending
            }
        }
    }

    /// Refund-leg status. A refund that is still `AUTHORIZED` has **not** moved any
    /// money — Saferpay requires the refund transaction itself to be captured — so
    /// it is reported as `Pending`, never `Success`.
    fn refund_status(&self) -> RefundStatus {
        match self.transaction_status() {
            SaferpayTransactionStatus::Captured => RefundStatus::Success,
            SaferpayTransactionStatus::Authorized
            | SaferpayTransactionStatus::Pending
            | SaferpayTransactionStatus::Unknown => RefundStatus::Pending,
            SaferpayTransactionStatus::Canceled => RefundStatus::Failure,
        }
    }

    fn connector_metadata(&self) -> Option<serde_json::Value> {
        let mut metadata = serde_json::Map::new();

        if let Some(capture_id) = self.capture_id.as_ref() {
            metadata.insert(
                CAPTURE_ID_METADATA_KEY.to_string(),
                serde_json::Value::String(capture_id.clone()),
            );
        }

        if let Some(authorized_amount) = self
            .amount
            .as_ref()
            .and_then(|amount| amount.value.as_ref())
        {
            metadata.insert(
                AUTHORIZED_AMOUNT_METADATA_KEY.to_string(),
                serde_json::Value::String(authorized_amount.clone()),
            );
        }

        (!metadata.is_empty()).then_some(serde_json::Value::Object(metadata))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaferpayRedirect {
    // Saferpay hosted-page URLs are .../vt2/Api/<Page>/<customer_id>/<terminal_id>/<token>
    // and the <terminal_id> segment is a credential (auth key2). `Secret` keeps
    // masked_serialize and any Debug `<response>` log paths from writing it;
    // expose() only where the redirect URL is returned to the caller.
    #[serde(rename = "RedirectUrl")]
    pub redirect_url: Option<Secret<String>>,
    #[serde(rename = "PaymentMeansRequired")]
    pub payment_means_required: Option<bool>,
}

/// Response envelope shared by `AuthorizeDirect`, `Initialize`, the token-based
/// `Authorize` and `Inquire`.
///
/// `Initialize` answers with `Token` + `Redirect` and **no** `Transaction` (no
/// transaction id exists yet); the other three answer with `Transaction`. Every
/// field is therefore optional and the two shapes are discriminated at mapping time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaferpayPaymentsResponse {
    #[serde(rename = "ResponseHeader")]
    pub response_header: Option<SaferpayResponseHeader>,
    #[serde(rename = "Transaction")]
    pub transaction: Option<SaferpayTransaction>,
    #[serde(rename = "Token")]
    pub token: Option<String>,
    #[serde(rename = "Expiration")]
    pub expiration: Option<String>,
    #[serde(rename = "RedirectRequired")]
    pub redirect_required: Option<bool>,
    #[serde(rename = "Redirect")]
    pub redirect: Option<SaferpayRedirect>,
    #[serde(rename = "LiabilityShift")]
    pub liability_shift: Option<bool>,
}

impl SaferpayPaymentsResponse {
    fn redirect_form(&self) -> Option<RedirectForm> {
        // Saferpay hands back an absolute URL whose path already carries the
        // session token; it is opened with a plain GET and has no form fields.
        self.redirect
            .as_ref()
            .and_then(|redirect| {
                redirect
                    .redirect_url
                    .as_ref()
                    .map(|url| url.clone().expose())
            })
            .map(|url| url.trim().to_string())
            .filter(|url| !url.is_empty())
            .map(|url| RedirectForm::Form {
                endpoint: url,
                method: Method::Get,
                form_fields: HashMap::new(),
            })
    }

    /// Metadata emitted alongside an `Initialize` response so the token survives to
    /// the settle Authorize that finalises the 3DS transaction.
    fn initialize_metadata(&self, token: &str) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            SAFERPAY_TOKEN_METADATA_KEY: token,
            SAFERPAY_STAGE_METADATA_KEY: STAGE_INITIALIZED,
        }))
    }
}

// =============================================================================
// ERROR RESPONSE
// =============================================================================

/// Saferpay `Behavior` — the retry signal on an error body. `ErrorName` says what
/// failed; `Behavior` says whether trying again (possibly with other means) can
/// succeed, so it is the field terminality is keyed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SaferpayErrorBehavior {
    #[serde(rename = "DO_NOT_RETRY")]
    DoNotRetry,
    #[serde(rename = "OTHER_MEANS")]
    OtherMeans,
    #[serde(rename = "RETRY")]
    Retry,
    #[serde(rename = "RETRY_LATER")]
    RetryLater,
    /// Anything Saferpay adds later. Kept so an unknown value parses instead of
    /// failing the whole error response.
    #[serde(other)]
    Unknown,
}

/// Saferpay error body (HTTP 400/401/402/403/406/415/500).
///
/// `ErrorDetail` is an **array of strings**, not a string.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaferpayErrorResponse {
    #[serde(rename = "ResponseHeader")]
    pub response_header: Option<SaferpayResponseHeader>,
    #[serde(rename = "Behavior")]
    pub behavior: Option<SaferpayErrorBehavior>,
    #[serde(rename = "ErrorName")]
    pub error_name: Option<String>,
    #[serde(rename = "ErrorMessage")]
    pub error_message: Option<String>,
    #[serde(rename = "ErrorDetail")]
    pub error_detail: Option<Vec<String>>,
    #[serde(rename = "TransactionId")]
    pub transaction_id: Option<String>,
    #[serde(rename = "OrderId")]
    pub order_id: Option<String>,
    #[serde(rename = "PayerMessage")]
    pub payer_message: Option<String>,
    #[serde(rename = "ProcessorName")]
    pub processor_name: Option<String>,
    #[serde(rename = "ProcessorResult")]
    pub processor_result: Option<String>,
    #[serde(rename = "ProcessorMessage")]
    pub processor_message: Option<String>,
}

impl SaferpayErrorResponse {
    pub fn to_error_response(&self, status_code: u16) -> ErrorResponse {
        let message = self
            .error_message
            .clone()
            .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string());
        let reason = self
            .error_detail
            .as_ref()
            .filter(|details| !details.is_empty())
            .map(|details| details.join(" "))
            .or_else(|| self.payer_message.clone())
            .or_else(|| self.error_message.clone());

        ErrorResponse {
            status_code,
            code: self
                .error_name
                .clone()
                .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
            message,
            reason,
            attempt_status: None,
            connector_transaction_id: self.transaction_id.clone(),
            // Saferpay explicitly documents that ProcessorResult must not drive
            // decisions; it is surfaced verbatim and nothing more.
            network_decline_code: self.processor_result.clone(),
            network_advice_code: None,
            network_error_message: self.processor_message.clone(),
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_response: None,
            typed_connector_request: None,
        }
    }

    pub fn to_refund_error_response(&self, status_code: u16) -> ErrorResponse {
        // Only a terminal decline settles the outcome: `TRANSACTION_DECLINED` means
        // the refund was refused, and Saferpay's `OTHER_MEANS` behaviour means no
        // retry of this operation can succeed. Every other error — validation, 4xx,
        // or a PSync/RSync lookup on a refund that is still in flight — leaves
        // `attempt_status` unset so the sync is not terminally failed while the
        // money may still be moving (review theme TH-07).
        let is_terminal_decline = self.error_name.as_deref() == Some("TRANSACTION_DECLINED")
            || self.behavior == Some(SaferpayErrorBehavior::OtherMeans);

        ErrorResponse {
            attempt_status: is_terminal_decline
                .then_some(FlowStatus::Refund(RefundStatus::Failure)),
            ..self.to_error_response(status_code)
        }
    }
}

// =============================================================================
// AUTHORIZE RESPONSE
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SaferpayAuthorizeResponse(pub SaferpayPaymentsResponse);

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<SaferpayAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<SaferpayAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response.0;

        // Both arms of this flow — `AuthorizeDirect` (non-3DS) and the read-only
        // `Inquire` that tails a completed 3DS journey — answer with a `Transaction`.
        // There is no token-bearing arm any more: `Initialize` lives in PreAuthenticate.
        let transaction = response.transaction.as_ref().ok_or_else(|| {
            error_stack::report!(crate::utils::unexpected_response_fail(
                item.http_code,
                "saferpay: Authorize response carried no Transaction object",
            ))
        })?;

        {
            let status = transaction.attempt_status();
            Ok(Self {
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(transaction.id.clone()),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: transaction.connector_metadata(),
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: transaction
                        .six_transaction_reference
                        .clone()
                        .or_else(|| transaction.order_id.clone()),
                    incremental_authorization_allowed: None,
                    splits: None,
                    status_code: item.http_code,
                    payment_account_reference: None,
                }),
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            })
        }
    }
}

// =============================================================================
// PSYNC — `Inquire`
// =============================================================================

/// PSync body: `POST /Payment/v1/Transaction/Inquire`.
///
/// A sync is strictly read-only. The 3DS second leg is the settle `Authorize`.
#[derive(Debug, Clone, Serialize)]
pub struct SaferpayPSyncRequest {
    #[serde(rename = "RequestHeader")]
    pub request_header: SaferpayRequestHeader,
    #[serde(rename = "TransactionReference")]
    pub transaction_reference: SaferpayTransactionReference,
}

type SyncRouterData = RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<SaferpayRouterData<SyncRouterData, T>> for SaferpayPSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: SaferpayRouterData<SyncRouterData, T>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = SaferpayAuthType::try_from(&router_data.connector_config)?;

        let transaction_id = router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .map_err(|_| missing_field("connector_transaction_id"))?;

        Ok(Self {
            request_header: SaferpayRequestHeader::new(
                &auth,
                payment_request_id(&router_data.resource_common_data),
            ),
            transaction_reference: SaferpayTransactionReference { transaction_id },
        })
    }
}

/// The newtype only gives the macro framework a distinct response type per flow.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SaferpayPSyncResponse(pub SaferpayPaymentsResponse);

impl TryFrom<ResponseRouterData<SaferpayPSyncResponse, Self>> for SyncRouterData {
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<SaferpayPSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response.0;

        let transaction = response.transaction.as_ref().ok_or_else(|| {
            error_stack::report!(crate::utils::unexpected_response_fail(
                item.http_code,
                "saferpay: sync response carried no Transaction object",
            ))
        })?;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(transaction.id.clone()),
                // A sync has no browser to redirect: the redirect instruction, if
                // any, was handed out by Authorize.
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: transaction.connector_metadata(),
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: transaction
                    .six_transaction_reference
                    .clone()
                    .or_else(|| transaction.order_id.clone()),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status: transaction.attempt_status(),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// PRE-AUTHENTICATE — `Initialize` (opens the 3DS journey)
// =============================================================================

/// Body for `POST /Payment/v1/Transaction/Initialize`.
///
/// Same shape as `AuthorizeDirect` plus `ReturnUrl` and `Authentication`; the
/// presence of `ReturnUrl` is what makes Saferpay return a redirect instead of
/// authorizing inline.
#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct SaferpayPreAuthenticateRequest<T: PaymentMethodDataTypes>(
    pub SaferpayCardAuthorizationRequest<T>,
);

type PreAuthenticateRouterData<T> = RouterDataV2<
    PreAuthenticate,
    PaymentFlowData,
    PaymentsPreAuthenticateData<T>,
    PaymentsResponseData,
>;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<SaferpayRouterData<PreAuthenticateRouterData<T>, T>>
    for SaferpayPreAuthenticateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: SaferpayRouterData<PreAuthenticateRouterData<T>, T>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;
        let common = &router_data.resource_common_data;

        let card = match &request.payment_method_data {
            Some(PaymentMethodData::Card(card)) => card,
            _ => {
                return Err(error_stack::report!(IntegrationError::NotImplemented(
                    "Only card payments are supported by saferpay".to_string(),
                    context(),
                )))
            }
        };

        if request.mandate_reference.is_some() {
            return Err(not_supported("Mandates / stored credentials".to_string()));
        }

        // Same reasoning as the Authorize leg: Saferpay has no sale mode, so an
        // authorization opened here still has to be settled by an explicit Capture.
        // Refuse at the opening leg rather than after the shopper has completed 3DS.
        if let Some(method @ (CaptureMethod::Automatic | CaptureMethod::SequentialAutomatic)) =
            &request.capture_method
        {
            return Err(capture_method_not_supported(*method));
        }

        let auth = SaferpayAuthType::try_from(&router_data.connector_config)?;

        let exp_year = card
            .get_expiry_year_4_digit()
            .expose()
            .parse::<u16>()
            .map_err(|_| {
                error_stack::report!(IntegrationError::InvalidDataFormat {
                    field_name: "card_exp_year",
                    context: context(),
                })
            })?;
        let exp_month = card.card_exp_month.peek().parse::<u8>().map_err(|_| {
            error_stack::report!(IntegrationError::InvalidDataFormat {
                field_name: "card_exp_month",
                context: context(),
            })
        })?;

        let amount = SaferpayAmountConvertor::convert(
            request.amount,
            request.currency.ok_or_else(|| missing_field("currency"))?,
        )?;

        // `continue_redirection_url` first, deliberately. It is the URL that routes the
        // returning shopper into the caller's complete-authorize path, which is what
        // dispatches the settle Authorize. Landing on the plain return URL instead would
        // leave the payment stuck at `AuthenticationPending`, because the second leg
        // would never be issued.
        let url = request
            .continue_redirection_url
            .clone()
            .map(|url| url.to_string())
            .or_else(|| request.router_return_url.clone().map(|url| url.to_string()))
            .or_else(|| common.return_url.clone())
            .ok_or_else(|| missing_field("continue_redirection_url"))?;

        Ok(Self(SaferpayCardAuthorizationRequest {
            request_header: SaferpayRequestHeader::new(&auth, payment_request_id(common)),
            terminal_id: auth.terminal_id.clone(),
            payment: SaferpayPaymentDetails {
                amount: SaferpayAmount {
                    value: amount,
                    currency_code: request.currency.ok_or_else(|| missing_field("currency"))?,
                },
                order_id: truncate_order_id(&common.connector_request_reference_id),
                description: common
                    .description
                    .clone()
                    .unwrap_or_else(|| DEFAULT_PAYMENT_DESCRIPTION.to_string()),
            },
            payment_means: SaferpayPaymentMeans {
                card: SaferpayCardDetails {
                    number: card.card_number.clone(),
                    exp_year: Secret::new(exp_year),
                    exp_month: Secret::new(exp_month),
                    verification_code: Some(card.card_cvc.clone()),
                    holder_name: card.get_optional_cardholder_name(),
                },
            },
            return_url: Some(SaferpayReturnUrl { url }),
            authentication: Some(SaferpayAuthentication {
                three_ds_challenge: "FORCE",
            }),
        }))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SaferpayPreAuthenticateResponse(pub SaferpayPaymentsResponse);

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<SaferpayPreAuthenticateResponse, Self>>
    for PreAuthenticateRouterData<T>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<SaferpayPreAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response.0;

        let token = response.token.clone().ok_or_else(|| {
            error_stack::report!(crate::utils::unexpected_response_fail(
                item.http_code,
                "saferpay: Initialize response carried no Token",
            ))
        })?;

        // `RedirectRequired: false` means Saferpay resolved authentication inline and
        // no browser step is needed. The token still has to be spent by
        // the settle Authorize, so the only difference is that no redirect is emitted.
        let redirection_data = if response.redirect_required.unwrap_or(false) {
            response.redirect_form().map(Box::new)
        } else {
            None
        };

        // The token travels in `connector_feature_data` on the common data — the channel
        // the framework reads for this leg (it does not read it off the response variant).
        // The caller persists it as `connector_metadata` on the attempt and hands it back
        // on the settle Authorize.
        Ok(Self {
            response: Ok(PaymentsResponseData::PreAuthenticateResponse {
                resource_id: Some(ResponseId::ConnectorTransactionId(token.clone())),
                redirection_data,
                // The token's carrier is `connector_feature_data` below; the caller
                // persists that as `connector_metadata` and hands it back on the settle
                // Authorize. No 3DS protocol data exists to report here — Saferpay runs
                // the authentication itself and never returns a CAVV/ECI.
                authentication_data: None,
                connector_response_reference_id: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::AuthenticationPending,
                connector_feature_data: response.initialize_metadata(&token).map(Secret::new),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// CAPTURE
// =============================================================================

/// `POST /Payment/v1/Transaction/Capture`. Carries no `TerminalId`; the optional
/// `Amount` block makes it a partial capture.
#[derive(Debug, Clone, Serialize)]
pub struct SaferpayCaptureRequest {
    #[serde(rename = "RequestHeader")]
    pub request_header: SaferpayRequestHeader,
    #[serde(rename = "TransactionReference")]
    pub transaction_reference: SaferpayTransactionReference,
    #[serde(rename = "Amount", skip_serializing_if = "Option::is_none")]
    pub amount: Option<SaferpayAmount>,
}

type CaptureRouterData =
    RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<SaferpayRouterData<CaptureRouterData, T>> for SaferpayCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: SaferpayRouterData<CaptureRouterData, T>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;

        // Saferpay settles an authorization with a single Capture. Splitting one
        // authorization across several captures needs MultipartCapture /
        // MultipartFinalize, which are out of scope.
        if request.is_multiple_capture() {
            return Err(not_supported("Multiple partial captures".to_string()));
        }
        if let Some(method @ (CaptureMethod::ManualMultiple | CaptureMethod::Scheduled)) =
            &request.capture_method
        {
            return Err(not_supported(format!("{method} capture")));
        }

        let auth = SaferpayAuthType::try_from(&router_data.connector_config)?;
        let transaction_id = request
            .connector_transaction_id
            .get_connector_transaction_id()
            .map_err(|_| missing_field("connector_transaction_id"))?;

        let amount =
            SaferpayAmountConvertor::convert(request.minor_amount_to_capture, request.currency)?;

        Ok(Self {
            request_header: SaferpayRequestHeader::new(
                &auth,
                payment_request_id(&router_data.resource_common_data),
            ),
            transaction_reference: SaferpayTransactionReference { transaction_id },
            amount: Some(SaferpayAmount {
                value: amount,
                currency_code: request.currency,
            }),
        })
    }
}

/// The Capture response carries **no `Transaction` object and no transaction id** —
/// only the `CaptureId` that a later Refund must reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaferpayCaptureResponse {
    #[serde(rename = "ResponseHeader")]
    pub response_header: Option<SaferpayResponseHeader>,
    #[serde(rename = "CaptureId")]
    pub capture_id: String,
    #[serde(rename = "Status")]
    pub status: Option<SaferpayTransactionStatus>,
    #[serde(rename = "Date")]
    pub date: Option<String>,
}

impl SaferpayCaptureResponse {
    fn attempt_status(&self, is_partial: bool) -> AttemptStatus {
        match self.status.unwrap_or(SaferpayTransactionStatus::Unknown) {
            SaferpayTransactionStatus::Captured if is_partial => AttemptStatus::PartialCharged,
            SaferpayTransactionStatus::Captured => AttemptStatus::Charged,
            SaferpayTransactionStatus::Canceled => AttemptStatus::Failure,
            SaferpayTransactionStatus::Authorized
            | SaferpayTransactionStatus::Pending
            | SaferpayTransactionStatus::Unknown => AttemptStatus::Pending,
        }
    }
}

fn authorized_amount_from_metadata(request: &PaymentsCaptureData) -> Option<MinorUnit> {
    let value = request
        .connector_feature_data
        .as_ref()?
        .peek()
        .get(AUTHORIZED_AMOUNT_METADATA_KEY)?;

    value
        .as_str()
        .and_then(|amount| amount.parse::<i64>().ok())
        .or_else(|| value.as_i64())
        .map(MinorUnit::new)
}

impl TryFrom<ResponseRouterData<SaferpayCaptureResponse, Self>> for CaptureRouterData {
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<SaferpayCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let request = &item.router_data.request;

        // Capture carries only the amount being captured, not the authorization
        // total. Prefer the common-data total when a caller supplies it, otherwise
        // use the total persisted from Authorize/PSync connector metadata. If an old
        // caller supplies neither, report PartialCharged conservatively rather than
        // claiming that the entire authorization was settled.
        let authorized_amount = item
            .router_data
            .resource_common_data
            .amount
            .as_ref()
            .map(|money| money.amount)
            .or_else(|| authorized_amount_from_metadata(request));
        let is_partial = authorized_amount
            .map(|amount| amount > request.minor_amount_to_capture)
            .unwrap_or(true);

        let status = response.attempt_status(is_partial);

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                // Echoed from the request: the response has no transaction id, and
                // every later operation still keys off the authorization's id.
                resource_id: request.connector_transaction_id.clone(),
                redirection_data: None,
                mandate_reference: None,
                // The Refund flow references `CaptureId`, never the transaction id,
                // so it has to survive this response.
                connector_metadata: Some(
                    serde_json::json!({ CAPTURE_ID_METADATA_KEY: response.capture_id }),
                ),
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// VOID / CANCEL
// =============================================================================

/// `POST /Payment/v1/Transaction/Cancel`. Full void only — the wire format has
/// neither an `Amount` nor a cancellation-reason field.
#[derive(Debug, Clone, Serialize)]
pub struct SaferpayVoidRequest {
    #[serde(rename = "RequestHeader")]
    pub request_header: SaferpayRequestHeader,
    #[serde(rename = "TransactionReference")]
    pub transaction_reference: SaferpayTransactionReference,
}

type VoidRouterData = RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<SaferpayRouterData<VoidRouterData, T>> for SaferpayVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: SaferpayRouterData<VoidRouterData, T>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = SaferpayAuthType::try_from(&router_data.connector_config)?;

        Ok(Self {
            request_header: SaferpayRequestHeader::new(
                &auth,
                payment_request_id(&router_data.resource_common_data),
            ),
            transaction_reference: SaferpayTransactionReference {
                transaction_id: router_data.request.connector_transaction_id.clone(),
            },
        })
    }
}

/// The Cancel response has **no `Status` field**: HTTP 200 *is* the success signal.
/// A later `Inquire` reports `CANCELED`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaferpayVoidResponse {
    #[serde(rename = "ResponseHeader")]
    pub response_header: Option<SaferpayResponseHeader>,
    #[serde(rename = "TransactionId")]
    pub transaction_id: Option<String>,
    #[serde(rename = "OrderId")]
    pub order_id: Option<String>,
    #[serde(rename = "Date")]
    pub date: Option<String>,
}

impl TryFrom<ResponseRouterData<SaferpayVoidResponse, Self>> for VoidRouterData {
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<SaferpayVoidResponse, Self>) -> Result<Self, Self::Error> {
        let response = item.response;
        let transaction_id = response
            .transaction_id
            .clone()
            .unwrap_or_else(|| item.router_data.request.connector_transaction_id.clone());

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(transaction_id),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: response.order_id.clone(),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::Voided,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// REFUND
// =============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct SaferpayRefundDetails {
    #[serde(rename = "Amount")]
    pub amount: SaferpayAmount,
    #[serde(rename = "OrderId", skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
}

/// `POST /Payment/v1/Transaction/Refund`.
///
/// Since SpecVersion 1.10 a referenced refund **must** name the capture
/// (`CaptureReference.CaptureId`); sending `TransactionReference.TransactionId`
/// fails. The `CaptureId` is produced by the Capture flow and reaches this flow
/// through `RefundsData::refund_connector_metadata`.
#[derive(Debug, Clone, Serialize)]
pub struct SaferpayRefundRequest {
    #[serde(rename = "RequestHeader")]
    pub request_header: SaferpayRequestHeader,
    #[serde(rename = "Refund")]
    pub refund: SaferpayRefundDetails,
    #[serde(rename = "CaptureReference")]
    pub capture_reference: SaferpayCaptureReference,
}

type RefundRouterData = RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>;

/// Pull the Saferpay `CaptureId` out of the metadata carriers the Capture flow
/// published it into.
fn extract_capture_id(request: &RefundsData) -> Option<String> {
    let read = |value: &common_utils::pii::SecretSerdeValue| -> Option<String> {
        value
            .clone()
            .expose()
            .get(CAPTURE_ID_METADATA_KEY)
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
    };

    request
        .refund_connector_metadata
        .as_ref()
        .and_then(read)
        .or_else(|| request.connector_feature_data.as_ref().and_then(read))
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<SaferpayRouterData<RefundRouterData, T>> for SaferpayRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: SaferpayRouterData<RefundRouterData, T>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;
        let auth = SaferpayAuthType::try_from(&router_data.connector_config)?;

        // There is no way to derive the CaptureId from the transaction id, and the
        // one-HTTP-call-per-flow contract leaves no room for a preliminary Inquire,
        // so the caller must round-trip the Capture flow's `connector_metadata`.
        let capture_id = extract_capture_id(request)
            .ok_or_else(|| missing_field("refund_metadata.capture_id"))?;

        let amount =
            SaferpayAmountConvertor::convert(request.minor_refund_amount, request.currency)?;

        Ok(Self {
            request_header: SaferpayRequestHeader::new(
                &auth,
                refund_request_id(&router_data.resource_common_data),
            ),
            refund: SaferpayRefundDetails {
                amount: SaferpayAmount {
                    value: amount,
                    currency_code: request.currency,
                },
                order_id: truncate_order_id(&request.refund_id),
            },
            capture_reference: SaferpayCaptureReference { capture_id },
        })
    }
}

/// The Refund response is a `Transaction` with `Type: REFUND` and
/// `Status: AUTHORIZED` — the refund exists but no money has moved yet.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SaferpayRefundResponse(pub SaferpayPaymentsResponse);

impl TryFrom<ResponseRouterData<SaferpayRefundResponse, Self>> for RefundRouterData {
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<SaferpayRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response.0;

        let transaction = response.transaction.as_ref().ok_or_else(|| {
            error_stack::report!(crate::utils::unexpected_response_fail(
                item.http_code,
                "saferpay: Refund response carried no Transaction object",
            ))
        })?;

        // `AUTHORIZED` here maps to `Pending`, never `Success`: Saferpay only moves
        // the money once the refund transaction is itself captured.
        let refund_status = transaction.refund_status();

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: transaction.id.clone(),
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: transaction.acquirer_reference.clone(),
            }),
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// RSYNC — `Inquire` on the refund transaction
// =============================================================================

/// Byte-identical on the wire to the `Inquire` variant of [`SaferpayPSyncRequest`];
/// the only difference is that the id sent is the **refund** transaction id.
#[derive(Debug, Clone, Serialize)]
pub struct SaferpayRefundSyncRequest {
    #[serde(rename = "RequestHeader")]
    pub request_header: SaferpayRequestHeader,
    #[serde(rename = "TransactionReference")]
    pub transaction_reference: SaferpayTransactionReference,
}

type RefundSyncRouterData =
    RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>;

/// True when this sync must first *settle* the refund rather than just read it.
///
/// A Saferpay refund is a two-call operation: `POST /Transaction/Refund` only
/// creates the refund transaction in state `AUTHORIZED`, and it never settles on
/// its own — it stays `AUTHORIZED` indefinitely until an explicit
/// `POST /Transaction/Capture` is issued against the refund's own transaction id.
///
/// UCS has no dedicated "capture a refund" flow and the Refund flow itself is a
/// single request, so the settling Capture is issued from RSync. A persisted
/// metadata stage records that Capture succeeded; later syncs then use `Inquire`.
///
/// Without this, a Saferpay refund can never reach `Success` through the normal
/// refund lifecycle.
pub fn refund_needs_settlement(request: &RefundSyncData) -> bool {
    request
        .refund_connector_metadata
        .as_ref()
        .and_then(|metadata| {
            metadata
                .peek()
                .get(SAFERPAY_STAGE_METADATA_KEY)
                .and_then(serde_json::Value::as_str)
        })
        .is_none_or(|stage| stage != STAGE_REFUND_SETTLED)
}

fn settled_refund_metadata(request: &RefundSyncData) -> common_utils::pii::SecretSerdeValue {
    let mut metadata = request
        .refund_connector_metadata
        .clone()
        .map(ExposeInterface::expose)
        .and_then(|metadata| metadata.as_object().cloned())
        .unwrap_or_default();
    metadata.insert(
        SAFERPAY_STAGE_METADATA_KEY.to_string(),
        serde_json::Value::String(STAGE_REFUND_SETTLED.to_string()),
    );
    Secret::new(serde_json::Value::Object(metadata))
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<SaferpayRouterData<RefundSyncRouterData, T>> for SaferpayRefundSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: SaferpayRouterData<RefundSyncRouterData, T>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = SaferpayAuthType::try_from(&router_data.connector_config)?;

        // Both `Capture` and `Inquire` take the same body shape, keyed on the
        // refund's own transaction id; only the endpoint differs (see `get_url`).
        Ok(Self {
            request_header: SaferpayRequestHeader::new(
                &auth,
                refund_request_id(&router_data.resource_common_data),
            ),
            transaction_reference: SaferpayTransactionReference {
                transaction_id: router_data.request.connector_refund_id.clone(),
            },
        })
    }
}

/// RSync answers with one of two different bodies depending on which endpoint it
/// hit (see `refund_needs_settlement`):
///
/// * `Capture`  -> `{"CaptureId": "...", "Status": "CAPTURED", ...}` — no
///   `Transaction` object at all.
/// * `Inquire`  -> `{"Transaction": {"Type": "REFUND", "Status": "...", ...}}`
///
/// `Settled` is listed first so serde tries the narrower shape before the
/// transaction-bearing one.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SaferpayRefundSyncResponse {
    /// Response to the settling `Capture`.
    Settled(SaferpayCaptureResponse),
    /// Response to a plain `Inquire`.
    Inquired(Box<SaferpayPaymentsResponse>),
}

impl TryFrom<ResponseRouterData<SaferpayRefundSyncResponse, Self>> for RefundSyncRouterData {
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<SaferpayRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = match item.response {
            // The settling Capture succeeded: Saferpay returns only a CaptureId and
            // a status, so the refund id is the one we already hold.
            SaferpayRefundSyncResponse::Settled(capture) => {
                let refund_status =
                    match capture.status.unwrap_or(SaferpayTransactionStatus::Unknown) {
                        SaferpayTransactionStatus::Captured => RefundStatus::Success,
                        SaferpayTransactionStatus::Canceled => RefundStatus::Failure,
                        SaferpayTransactionStatus::Authorized
                        | SaferpayTransactionStatus::Pending
                        | SaferpayTransactionStatus::Unknown => RefundStatus::Pending,
                    };

                let mut router_data = item.router_data;
                if refund_status == RefundStatus::Success {
                    let refund_metadata = settled_refund_metadata(&router_data.request);
                    router_data.request.refund_connector_metadata = Some(refund_metadata);
                }

                return Ok(Self {
                    response: Ok(RefundsResponseData {
                        connector_refund_id: router_data.request.connector_refund_id.clone(),
                        refund_status,
                        status_code: item.http_code,
                        acquirer_reference_number: None,
                    }),
                    resource_common_data: RefundFlowData {
                        status: refund_status,
                        ..router_data.resource_common_data
                    },
                    ..router_data
                });
            }
            SaferpayRefundSyncResponse::Inquired(inquired) => inquired,
        };

        let transaction = response.transaction.as_ref().ok_or_else(|| {
            error_stack::report!(crate::utils::unexpected_response_fail(
                item.http_code,
                "saferpay: RSync response carried no Transaction object",
            ))
        })?;

        // Guard against being pointed at the payment leg: only a `Type: REFUND`
        // transaction may be reported as a refund state.
        if transaction.transaction_type == Some(SaferpayTransactionType::Payment) {
            return Err(error_stack::report!(
                crate::utils::unexpected_response_fail(
                    item.http_code,
                    "saferpay: RSync resolved a PAYMENT transaction, not a REFUND",
                )
            ));
        }

        let refund_status = transaction.refund_status();

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: transaction.id.clone(),
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: transaction.acquirer_reference.clone(),
            }),
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// REPEAT PAYMENT — `AuthorizeDirect` with a stored alias (unscheduled MIT)
// =============================================================================

/// Previously registered card, referenced by `Alias.Id` — the SetupMandate flow's
/// response surfaces this id as the `connector_mandate_id`.
#[derive(Debug, Clone, Serialize)]
pub struct SaferpayAliasReference {
    #[serde(rename = "Id")]
    pub id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SaferpayAliasPaymentMeans {
    #[serde(rename = "Alias")]
    pub alias: SaferpayAliasReference,
}

/// `Authentication.Initiator` / `Authentication.Exemption` on `AuthorizeDirect`.
/// `MERCHANT` marks the request as merchant-initiated (MIT) rather than a fresh
/// checkout; `RECURRING` claims the PSD2 SCA exemption for a payment on a mandate
/// the payer already consented to.
#[derive(Debug, Clone, Serialize)]
pub struct SaferpayMitAuthentication {
    #[serde(rename = "Initiator")]
    pub initiator: &'static str,
    #[serde(rename = "Exemption")]
    pub exemption: &'static str,
}

/// Body for `POST /Payment/v1/Transaction/AuthorizeDirect` on the MIT rail
/// (plan UD-07/UD-11): an unscheduled charge against a stored alias. The scheduled
/// rail (`AuthorizeReferenced` + `Payment.Recurring`) is not implemented, and the
/// `Authentication.IssuerReference` block is only available on the scheduled/NTID
/// rail, so it is omitted here.
#[derive(Debug, Clone, Serialize)]
pub struct SaferpayRepeatPaymentRequest {
    #[serde(rename = "RequestHeader")]
    pub request_header: SaferpayRequestHeader,
    #[serde(rename = "TerminalId")]
    pub terminal_id: Secret<String>,
    #[serde(rename = "Payment")]
    pub payment: SaferpayPaymentDetails,
    #[serde(rename = "PaymentMeans")]
    pub payment_means: SaferpayAliasPaymentMeans,
    #[serde(rename = "Authentication")]
    pub authentication: SaferpayMitAuthentication,
}

type RepeatPaymentRouterData<T> =
    RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<SaferpayRouterData<RepeatPaymentRouterData<T>, T>> for SaferpayRepeatPaymentRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: SaferpayRouterData<RepeatPaymentRouterData<T>, T>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;
        let common = &router_data.resource_common_data;
        let auth = SaferpayAuthType::try_from(&router_data.connector_config)?;

        // The alias rail stores a card credential; a wallet or bank transfer has no
        // alias to charge.
        match &request.payment_method_data {
            PaymentMethodData::Card(_) => {}
            _ => {
                return Err(error_stack::report!(IntegrationError::NotImplemented(
                    "Only card payments are supported by saferpay".to_string(),
                    context(),
                )))
            }
        };

        // Same reasoning as the Authorize leg: Saferpay has no sale mode, so an MIT
        // authorization still has to be settled by an explicit Capture. Refuse
        // `Automatic`/`SequentialAutomatic` rather than report `requires_capture` and
        // leave the money unmoved with no error.
        if let Some(method @ (CaptureMethod::Automatic | CaptureMethod::SequentialAutomatic)) =
            &request.capture_method
        {
            return Err(capture_method_not_supported(*method));
        }

        // Only the connector-stored alias rail is chargeable here. A network-transaction-id
        // / network-token mandate belongs to the NTID rail (`AuthorizeReferenced` +
        // `Authentication.IssuerReference`), which this integration does not implement,
        // so it is a hard refusal rather than a silent wrong-route.
        let alias_id = match request.get_mandate_reference() {
            MandateReferenceId::ConnectorMandateId(connector_mandate_id) => connector_mandate_id
                .get_connector_mandate_id()
                .ok_or_else(|| missing_field("mandate_reference.connector_mandate_id"))?,
            _ => {
                return Err(not_supported(
                    "Network-mandate-id / network-token MIT — saferpay charges the \
                     stored alias (ConnectorMandateId) issued by Alias/InsertDirect only"
                        .to_string(),
                ))
            }
        };

        let amount = SaferpayAmountConvertor::convert(request.minor_amount, request.currency)?;

        let description = common
            .description
            .clone()
            .unwrap_or_else(|| DEFAULT_PAYMENT_DESCRIPTION.to_string());

        Ok(Self {
            request_header: SaferpayRequestHeader::new(&auth, payment_request_id(common)),
            terminal_id: auth.terminal_id.clone(),
            payment: SaferpayPaymentDetails {
                amount: SaferpayAmount {
                    value: amount,
                    currency_code: request.currency,
                },
                order_id: truncate_order_id(&common.connector_request_reference_id),
                description,
            },
            payment_means: SaferpayAliasPaymentMeans {
                alias: SaferpayAliasReference { id: alias_id },
            },
            authentication: SaferpayMitAuthentication {
                initiator: "MERCHANT",
                exemption: "RECURRING",
            },
        })
    }
}

/// `AuthorizeDirect` answers with the same `Transaction` body as the card
/// authorization (spec §5).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SaferpayRepeatPaymentResponse(pub SaferpayPaymentsResponse);

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<SaferpayRepeatPaymentResponse, Self>>
    for RepeatPaymentRouterData<T>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<SaferpayRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response.0;

        let transaction = response.transaction.as_ref().ok_or_else(|| {
            error_stack::report!(crate::utils::unexpected_response_fail(
                item.http_code,
                "saferpay: RepeatPayment response carried no Transaction object",
            ))
        })?;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(transaction.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: transaction.connector_metadata(),
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: transaction
                    .six_transaction_reference
                    .clone()
                    .or_else(|| transaction.order_id.clone()),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status: transaction.attempt_status(),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// SETUP MANDATE — `Alias/InsertDirect` (Secure Card Data alias registration)
// =============================================================================

/// Body for `POST /Payment/v1/Alias/InsertDirect` — the spec's SetupMandate
/// **Option B**: one-shot, server-to-server alias registration of a raw card.
/// It moves no money (a $0 registration) and synchronously hands back the
/// `Alias.Id` that the RepeatPayment flow later charges, so the caller never
/// has to drive a redirect before it can repeat against the mandate.
///
/// Option A (`Alias/Insert` + `Alias/AssertInsert`) is deliberately not used:
/// it opens a hosted-form redirect before the alias id is known, which cannot
/// satisfy Charge's strict synchronous `connector_mandate_id` dependency.
#[derive(Debug, Clone, Serialize)]
pub struct SaferpaySetupMandateRequest<T: PaymentMethodDataTypes> {
    #[serde(rename = "RequestHeader")]
    pub request_header: SaferpayRequestHeader,
    /// Raw card to register. Same shape as the authorization's PaymentMeans;
    /// `HolderName` is required by InsertDirect.
    #[serde(rename = "PaymentMeans")]
    pub payment_means: SaferpayPaymentMeans<T>,
    /// `IdGenerator: RANDOM` — Saferpay mints the alias id; the merchant never
    /// chooses it, so it can never collide with a merchant namespace.
    #[serde(rename = "RegisterAlias")]
    pub register_alias: SaferpayRegisterAlias,
}

#[derive(Debug, Clone, Serialize)]
pub struct SaferpayRegisterAlias {
    #[serde(rename = "IdGenerator")]
    pub id_generator: &'static str,
}

type SetupMandateRouterData<T> =
    RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<SaferpayRouterData<SetupMandateRouterData<T>, T>> for SaferpaySetupMandateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: SaferpayRouterData<SetupMandateRouterData<T>, T>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;
        let common = &router_data.resource_common_data;
        let auth = SaferpayAuthType::try_from(&router_data.connector_config)?;

        // InsertDirect registers a raw card server-to-server; a wallet or bank
        // transfer has no card container to register.
        let card = match &request.payment_method_data {
            PaymentMethodData::Card(card) => card,
            _ => {
                return Err(error_stack::report!(IntegrationError::NotImplemented(
                    "Only card payments are supported by saferpay".to_string(),
                    context(),
                )))
            }
        };

        // InsertDirect takes the card on the call itself and answers
        // synchronously — there is no redirect and no `ReturnUrl` field on the
        // endpoint. A caller asking for a payer redirect is describing the
        // hosted-form registration (Option A, `Alias/Insert` + `Alias/AssertInsert`),
        // which this integration does not implement; refuse loudly rather than
        // silently registering the card behind the shopper's back.
        if request.return_url.is_some() || request.router_return_url.is_some() {
            return Err(not_supported(
                "Redirect-based alias registration (Alias/Insert) — saferpay mandates \
                 are registered headlessly via Alias/InsertDirect"
                    .to_string(),
            ));
        }

        let exp_year = card
            .get_expiry_year_4_digit()
            .expose()
            .parse::<u16>()
            .map_err(|_| {
                error_stack::report!(IntegrationError::InvalidDataFormat {
                    field_name: "card_exp_year",
                    context: context(),
                })
            })?;
        let exp_month = card.card_exp_month.peek().parse::<u8>().map_err(|_| {
            error_stack::report!(IntegrationError::InvalidDataFormat {
                field_name: "card_exp_month",
                context: context(),
            })
        })?;

        // `HolderName` is a required field on `Alias/InsertDirect` (unlike the
        // authorization endpoints, where it is optional). Never invent one from
        // billing data: a name that is not the cardholder's mislabels the stored
        // credential (review theme TH-ST-3 — no silent holder-name fallback).
        let holder_name = card
            .get_optional_cardholder_name()
            .ok_or_else(|| missing_field("card_holder_name"))?;

        Ok(Self {
            request_header: SaferpayRequestHeader::new(&auth, payment_request_id(common)),
            payment_means: SaferpayPaymentMeans {
                card: SaferpayCardDetails {
                    number: card.card_number.clone(),
                    exp_year: Secret::new(exp_year),
                    exp_month: Secret::new(exp_month),
                    verification_code: Some(card.card_cvc.clone()),
                    holder_name: Some(holder_name),
                },
            },
            register_alias: SaferpayRegisterAlias {
                id_generator: "RANDOM",
            },
        })
    }
}

/// InsertDirect answers synchronously with the freshly minted alias. There is no
/// `Transaction` object — nothing was charged — so `Alias.Id` is both the success
/// signal (2xx **and** an alias id) and the mandate reference RepeatPayment
/// consumes. A 2xx *without* an `Alias.Id` is a connector contract violation and
/// is failed closed, never reported as success.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaferpaySetupMandateResponse {
    #[serde(rename = "ResponseHeader")]
    pub response_header: Option<SaferpayResponseHeader>,
    #[serde(rename = "Alias")]
    pub alias: Option<SaferpayAlias>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaferpayAlias {
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "Lifetime")]
    pub lifetime: Option<u32>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static>
    TryFrom<ResponseRouterData<SaferpaySetupMandateResponse, Self>> for SetupMandateRouterData<T>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<SaferpaySetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        let alias_id = response
            .alias
            .as_ref()
            .map(|alias| alias.id.trim().to_string())
            .filter(|id| !id.is_empty())
            .ok_or_else(|| {
                error_stack::report!(crate::utils::unexpected_response_fail(
                    item.http_code,
                    "saferpay: InsertDirect response carried no Alias.Id",
                ))
            })?;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(alias_id.clone()),
                redirection_data: None,
                // `Alias.Id` is the single mandate reference: RepeatPayment sends it
                // back as `PaymentMeans.Alias.Id` on `AuthorizeDirect`.
                mandate_reference: Some(Box::new(MandateReference {
                    connector_mandate_id: Some(alias_id),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                    mandate_metadata: None,
                })),
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: response
                    .response_header
                    .and_then(|header| header.request_id),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
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

// =============================================================================
// INCOMING WEBHOOK — unsigned `*NotifyUrl` ping + authenticated `Inquire` pull
// (spec:## Webhook Events; plan P-webhook-01, G-webhook-01, UD-12)
// =============================================================================

/// Which Saferpay `Notification` URL family a ping arrived on.
///
/// Success and failure are two different URLs, so the URL itself is the only
/// signal that classifies an outcome; the ping carries no body. Saferpay
/// unconditionally appends the session `Token` as the first query parameter
/// ("success_notify_url?token=.."), so it is never a classification aid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaferpayNotifyPath {
    /// `SuccessNotifyUrl` / `RedirectNotifyUrls.Success` — a good outcome.
    Success,
    /// `FailNotifyUrl` / `RedirectNotifyUrls.Fail` — a declined/aborted outcome.
    Fail,
    /// The URL gave no family hint. Any event derived from it fails closed
    /// (G-webhook-01).
    Ambiguous,
}

/// Tolerant grab of anything Saferpay (or an embedding page) posts to the
/// notify URL. Real Saferpay notifications are bodyless GETs, so this is
/// optional everywhere; the id here is only a *hint* — authenticity is
/// established by the authenticated `Inquire` pull, never by the body.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SaferpayWebhookBody {
    #[serde(default)]
    pub id: Option<String>,
    /// Event family hint for embedders that post a classified ping back. Real
    /// Saferpay GETs carry no such key; the URL family wins over this hint.
    #[serde(default)]
    pub event: Option<String>,
}

/// The notification token Saferpay may append verbatim to a notify URL, the
/// session handle opened by `PaymentPage/Initialize` (or
/// `Transaction/Initialize`). Parsing is tolerant: a bare `?<token>` is as
/// valid as `?token=<token>`.
pub fn notify_query_token(query_params: Option<&str>) -> Option<String> {
    let query = query_params?.trim_start_matches('?');
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or(("", pair));
        if key.eq_ignore_ascii_case("token") && !value.is_empty() {
            return Some(value.to_string());
        }
        if key.is_empty() && !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

/// The outcome an ambiguous-CANCELED ping reports. A CANCELED payment is a
/// decline/failure, a CANCELED refund is also a failure — the refund_status()
/// mapping shares the arm, so a webhook details status can reuse
/// refund_status() when the ping's family is unknown.
pub fn webhook_attempt_status(
    path: SaferpayNotifyPath,
) -> Result<AttemptStatus, error_stack::Report<domain_types::errors::WebhookError>> {
    use domain_types::errors::WebhookError;
    match path {
        SaferpayNotifyPath::Success => Ok(AttemptStatus::Authorized),
        SaferpayNotifyPath::Fail => Ok(AttemptStatus::Failure),
        SaferpayNotifyPath::Ambiguous => Err(WebhookError::WebhookEventTypeNotFound.into()),
    }
}

/// Classify `RequestDetails.uri` into its notify-URL family. Matching is
/// case-insensitive; `fail`/`error`/`cancel` win over `success` when the URL
/// somehow carries both — failing closed is cheaper than a false success
/// (G-webhook-01).
pub fn classify_notify_url(uri: Option<&str>) -> SaferpayNotifyPath {
    let uri = match uri {
        Some(uri) => uri.to_lowercase(),
        None => return SaferpayNotifyPath::Ambiguous,
    };
    let path = uri.split('?').next().unwrap_or(uri.as_str());
    let has_fail = ["fail", "error", "cancel", "abort", "declin"]
        .iter()
        .any(|needle| path.contains(needle));
    let has_success = path.contains("success");
    match (has_success, has_fail) {
        (true, false) => SaferpayNotifyPath::Success,
        (false, true) => SaferpayNotifyPath::Fail,
        _ => SaferpayNotifyPath::Ambiguous,
    }
}

/// Whether `uri` names a refund-leg notification (a `RefundNotifyUrl` /
/// refund-scoped `PendingNotification.NotifyUrl`). Payment pings never carry
/// the word, so its presence is definitive; its absence says nothing by
/// itself.
fn is_refund_notify_uri(uri: Option<&str>) -> bool {
    uri.map(|uri| uri.to_lowercase().contains("refund"))
        .unwrap_or(false)
}

/// The event type a ping on `uri` maps to. The URL family decides the
/// outcome, and a refund-marked URL (or, as a fallback, the ping body's
/// `event` hint) shifts it to the refund arm of the same outcome. An
/// ambiguous URL is the missing/ambiguous-key case and errors in
/// `get_event_type` (G-webhook-01); it never produces a synthetic event.
pub fn classify_notify_event(
    uri: Option<&str>,
    body: Option<&SaferpayWebhookBody>,
) -> Result<EventType, error_stack::Report<domain_types::errors::WebhookError>> {
    use domain_types::errors::WebhookError;
    let is_refund = is_refund_notify_uri(uri)
        || body
            .and_then(|body| body.event.as_deref())
            .map(|event| event.to_lowercase().contains("refund"))
            .unwrap_or(false);
    match classify_notify_url(uri) {
        SaferpayNotifyPath::Success if is_refund => Ok(EventType::RefundSuccess),
        SaferpayNotifyPath::Success => Ok(EventType::PaymentIntentSuccess),
        SaferpayNotifyPath::Fail if is_refund => Ok(EventType::RefundFailure),
        SaferpayNotifyPath::Fail => Ok(EventType::PaymentIntentFailure),
        SaferpayNotifyPath::Ambiguous => Err(WebhookError::WebhookEventTypeNotFound.into()),
    }
}

/// `Inquire` body for the external source-verification pull
/// (`Transaction/Inquire`, UD-12 option b): confirm the transaction reference
/// the notification claims with an authenticated call, keyed on the event id.
#[derive(Debug, Clone, Serialize)]
pub struct SaferpayWebhookVerifyRequest {
    #[serde(rename = "RequestHeader")]
    pub request_header: SaferpayRequestHeader,
    #[serde(rename = "TransactionReference")]
    pub transaction_reference: SaferpayTransactionReference,
}

/// The newtype only gives the macro framework a distinct response type for
/// the flow; the payload is the standard `Inquire` envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SaferpayWebhookVerifyResponse(pub SaferpayPaymentsResponse);

type VerifyWebhookSourceRouterData = RouterDataV2<
    VerifyWebhookSource,
    VerifyWebhookSourceFlowData,
    VerifyWebhookSourceRequestData,
    VerifyWebhookSourceResponseData,
>;

/// Pull the event id (a Saferpay ``Transaction.Id``) out of the raw webhook
/// request. The verify step is reached before `get_event_type` has ran, so it
/// accepts the same tolerant shapes: a `{"id": ..}` JSON body first (the
/// harness fixture and any embedding-page POST), then a `TransactionId` /
/// `id` query key on bare GETs, and finally the lone first-value of the query
/// string — Saferpay appends exactly one parameter, the session token.
///
/// Nothing found is a fail-closed error, so the server reports the source as
/// unverified instead of querying the wrong reference (G-webhook-01).
fn webhook_event_id(
    req: &VerifyWebhookSourceRequestData,
) -> Result<String, error_stack::Report<IntegrationError>> {
    if !req.webhook_body.is_empty() {
        if let Ok(body) = serde_json::from_slice::<SaferpayWebhookBody>(&req.webhook_body) {
            if let Some(id) = body.id.filter(|id| !id.trim().is_empty()) {
                return Ok(id);
            }
        }
    }
    if let Some(uri) = req.webhook_uri.as_deref() {
        if let Some((_, query)) = uri.split_once('?') {
            for pair in query.split('&').filter(|pair| !pair.is_empty()) {
                let (key, value) = pair.split_once('=').unwrap_or(("", pair));
                if !key.is_empty() {
                    if matches!(key.to_lowercase().as_str(), "transactionid" | "id")
                        && !value.is_empty()
                    {
                        return Ok(value.to_string());
                    }
                } else {
                    // Saferpay unilaterally appends the reference as the first
                    // (and only) query parameter, without a key of its own.
                    return Ok(value.to_string());
                }
            }
        }
    }
    Err(missing_field(
        "webhook transaction id (body id or URL parameter)",
    ))
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<SaferpayRouterData<VerifyWebhookSourceRouterData, T>> for SaferpayWebhookVerifyRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: SaferpayRouterData<VerifyWebhookSourceRouterData, T>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = SaferpayAuthType::try_from(&router_data.connector_config)?;
        let transaction_id = webhook_event_id(&router_data.request)?;

        Ok(Self {
            request_header: SaferpayRequestHeader::new(
                &auth,
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            transaction_reference: SaferpayTransactionReference { transaction_id },
        })
    }
}

impl TryFrom<ResponseRouterData<SaferpayWebhookVerifyResponse, Self>>
    for VerifyWebhookSourceRouterData
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<SaferpayWebhookVerifyResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response.0;

        // Source verification asks one question: did the authenticated call
        // return the same transaction the notification named? A 2xx whose
        // Transaction block is missing, or whose ``Id`` disagrees with the
        // claimed id, is an in-band failure — the source is not verified; it
        // is never silently promoted to verified (TH-07).
        let claimed_id = webhook_event_id(&item.router_data.request).ok();
        let verify_webhook_status = match (claimed_id, response.transaction) {
            (Some(claimed), Some(transaction)) if transaction.id == claimed => {
                VerifyWebhookStatus::SourceVerified
            }
            _ => VerifyWebhookStatus::SourceNotVerified,
        };

        Ok(Self {
            response: Ok(VerifyWebhookSourceResponseData {
                verify_webhook_status,
            }),
            ..item.router_data
        })
    }
}
