//! Saferpay transformers.
//!
//! Saferpay (SIX Payment Services) exposes a POST-only, RPC-style JSON API under
//! `/Payment/v1/Transaction/*`. There are no GET endpoints and no path or query
//! parameters: the resource being acted on is always named inside the JSON body
//! (`TransactionReference.TransactionId`, `CaptureReference.CaptureId` or `Token`).
//! Every request carries a `RequestHeader` envelope and every response a
//! `ResponseHeader`.
//!
//! Scope of this module: **Card, one-time payments only** — Authorize (non-3DS via
//! `AuthorizeDirect`, 3DS via `Initialize` + the token-based `Authorize` issued from
//! PSync), PSync / RSync (`Inquire`), Capture, Void (`Cancel`) and Refund. Wallets,
//! bank debits, mandates/MIT, tokenization (Alias / Secure Card Data), `RefundDirect`,
//! DCC and marketplace splits are all out of scope.

use std::collections::HashMap;

use common_enums::{AttemptStatus, AuthenticationType, CaptureMethod, RefundStatus};
use common_utils::{types::StringMinorUnit, Method};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::connectors::saferpay::{SaferpayAmountConvertor, SaferpayRouterData};
use crate::types::ResponseRouterData;

/// Saferpay JSON API contract version this integration is pinned to. Every
/// `RequestHeader` must carry it; bumping it changes the wire contract.
pub const SAFERPAY_SPEC_VERSION: &str = "1.44";

/// `RetryIndicator` for a first attempt. Saferpay allows 0-9, incremented per retry
/// of the *same* `RequestId`; UCS generates a fresh `RequestId` per call instead.
const RETRY_INDICATOR_FIRST_ATTEMPT: u8 = 0;

/// `Payment.Description` is documented as optional but the Saferpay Backoffice needs
/// a value to render the transaction, so a fallback is always sent.
const DEFAULT_PAYMENT_DESCRIPTION: &str = "Payment";

/// Saferpay caps `OrderId` at 80 characters and rejects longer values outright.
const ORDER_ID_MAX_LEN: usize = 80;

/// `Transaction.Type` for a payment leg.
const TRANSACTION_TYPE_PAYMENT: &str = "PAYMENT";

/// `Transaction.Status` values. Saferpay spells the cancelled state `CANCELED`
/// with a single `L`.
const STATUS_AUTHORIZED: &str = "AUTHORIZED";
const STATUS_CAPTURED: &str = "CAPTURED";
const STATUS_PENDING: &str = "PENDING";
const STATUS_CANCELED: &str = "CANCELED";

/// Key under which the Capture flow publishes the `CaptureId` in
/// `connector_metadata`, and under which the Refund flow expects to find it in
/// `RefundsData::refund_connector_metadata`.
pub const CAPTURE_ID_METADATA_KEY: &str = "capture_id";
/// Key under which the 3DS `Initialize` response publishes the Saferpay session
/// token in `connector_metadata`.
pub const SAFERPAY_TOKEN_METADATA_KEY: &str = "saferpay_token";

fn context() -> IntegrationErrorContext {
    IntegrationErrorContext::default()
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
/// `RequestId` must be unique per **HTTP request** (not per payment), so a fresh
/// UUID is generated for each outbound call.
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
    fn new(auth: &SaferpayAuthType) -> Self {
        Self {
            spec_version: SAFERPAY_SPEC_VERSION.to_string(),
            customer_id: auth.customer_id.clone(),
            request_id: uuid::Uuid::new_v4().to_string(),
            retry_indicator: RETRY_INDICATOR_FIRST_ATTEMPT,
        }
    }
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
    pub exp_year: u16,
    #[serde(rename = "ExpMonth")]
    pub exp_month: u8,
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
pub struct SaferpayAuthorizeRequest<T: PaymentMethodDataTypes> {
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

type AuthorizeRouterData<T> =
    RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>;

/// `true` when this attempt must go through the redirect (`Initialize`) leg.
pub fn is_three_ds_authorize<T: PaymentMethodDataTypes>(
    router_data: &AuthorizeRouterData<T>,
) -> bool {
    router_data.resource_common_data.auth_type == AuthenticationType::ThreeDs
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<SaferpayRouterData<AuthorizeRouterData<T>, T>> for SaferpayAuthorizeRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: SaferpayRouterData<AuthorizeRouterData<T>, T>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;
        let common = &router_data.resource_common_data;

        let card = match &request.payment_method_data {
            PaymentMethodData::Card(card) => card,
            _ => {
                return Err(error_stack::report!(IntegrationError::NotImplemented(
                    "Only card payments are supported by saferpay".to_string(),
                    context(),
                )))
            }
        };

        // Saferpay's Transaction interface has no field to carry an externally
        // obtained CAVV/ECI/dsTransId: 3DS is always run by Saferpay itself through
        // the Initialize redirect, so merchant-supplied authentication data cannot
        // be honoured.
        if request.authentication_data.is_some() {
            return Err(not_supported(
                "External/merchant-provided 3DS authentication data".to_string(),
            ));
        }

        // Mandates, MIT and tokenization are out of scope: Saferpay expresses them
        // through Secure Card Data / `Alias`, which this integration does not
        // implement. Reject rather than silently dropping the intent.
        if request.mandate_id.is_some() || request.setup_mandate_details.is_some() {
            return Err(not_supported("Mandates / stored credentials".to_string()));
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

        let amount = SaferpayAmountConvertor::convert(request.minor_amount, request.currency)?;

        let description = common
            .description
            .clone()
            .unwrap_or_else(|| DEFAULT_PAYMENT_DESCRIPTION.to_string());

        let (return_url, authentication) = if is_three_ds_authorize(router_data) {
            let url = request
                .router_return_url
                .clone()
                .or_else(|| request.complete_authorize_url.clone())
                .or_else(|| common.return_url.clone())
                .ok_or_else(|| missing_field("router_return_url"))?;
            (
                Some(SaferpayReturnUrl { url }),
                Some(SaferpayAuthentication {
                    three_ds_challenge: "FORCE",
                }),
            )
        } else {
            (None, None)
        };

        Ok(Self {
            request_header: SaferpayRequestHeader::new(&auth),
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
                    exp_year,
                    exp_month,
                    verification_code: Some(card.card_cvc.clone()),
                    holder_name: card.get_optional_cardholder_name(),
                },
            },
            return_url,
            authentication,
        })
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
    pub transaction_type: Option<String>,
    #[serde(rename = "Status")]
    pub status: Option<String>,
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
    fn status_str(&self) -> &str {
        self.status.as_deref().unwrap_or_default()
    }

    /// Payment-leg status. Saferpay never auto-captures on this interface: an
    /// `AuthorizeDirect` / token `Authorize` always answers `AUTHORIZED`, and the
    /// caller settles with an explicit Capture regardless of `capture_method`.
    fn attempt_status(&self) -> AttemptStatus {
        match self.status_str() {
            STATUS_AUTHORIZED => AttemptStatus::Authorized,
            STATUS_CAPTURED => AttemptStatus::Charged,
            STATUS_CANCELED => AttemptStatus::Voided,
            STATUS_PENDING => AttemptStatus::Pending,
            _ => AttemptStatus::Pending,
        }
    }

    /// Refund-leg status. A refund that is still `AUTHORIZED` has **not** moved any
    /// money — Saferpay requires the refund transaction itself to be captured — so
    /// it is reported as `Pending`, never `Success`.
    fn refund_status(&self) -> RefundStatus {
        match self.status_str() {
            STATUS_CAPTURED => RefundStatus::Success,
            STATUS_AUTHORIZED | STATUS_PENDING => RefundStatus::Pending,
            STATUS_CANCELED => RefundStatus::Failure,
            _ => RefundStatus::Pending,
        }
    }

    fn connector_metadata(&self) -> Option<serde_json::Value> {
        self.capture_id
            .as_ref()
            .map(|capture_id| serde_json::json!({ CAPTURE_ID_METADATA_KEY: capture_id }))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaferpayRedirect {
    #[serde(rename = "RedirectUrl")]
    pub redirect_url: Option<String>,
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
            .and_then(|redirect| redirect.redirect_url.as_deref())
            .map(str::trim)
            .filter(|url| !url.is_empty())
            .map(|url| RedirectForm::Form {
                endpoint: url.to_string(),
                method: Method::Get,
                form_fields: HashMap::new(),
            })
    }

    /// Metadata emitted alongside an `Initialize` response so the token survives to
    /// the PSync that finalises the 3DS transaction.
    fn initialize_metadata(&self, token: &str) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            SAFERPAY_TOKEN_METADATA_KEY: token,
            "stage": "initialized",
        }))
    }
}

// =============================================================================
// ERROR RESPONSE
// =============================================================================

/// Saferpay error body (HTTP 400/401/402/403/406/415/500).
///
/// `ErrorDetail` is an **array of strings**, not a string.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaferpayErrorResponse {
    #[serde(rename = "ResponseHeader")]
    pub response_header: Option<SaferpayResponseHeader>,
    #[serde(rename = "Behavior")]
    pub behavior: Option<String>,
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
        ErrorResponse {
            attempt_status: Some(FlowStatus::Refund(RefundStatus::Failure)),
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

        // Non-3DS (`AuthorizeDirect`): a real transaction already exists.
        if let Some(transaction) = response.transaction.as_ref() {
            let status = transaction.attempt_status();
            return Ok(Self {
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
                }),
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        // 3DS (`Initialize`): only a `Token` exists. It is published as the
        // resource id, in `connector_metadata` and — via the caller round-tripping
        // it as `connector_feature_data` — is what PSync uses for the second leg
        // (`POST /Transaction/Authorize` with `{ "Token": ... }`).
        let token = response.token.clone().ok_or_else(|| {
            error_stack::report!(crate::utils::unexpected_response_fail(
                item.http_code,
                "saferpay: Authorize response carried neither a Transaction nor a Token",
            ))
        })?;

        // `RedirectRequired: false` means Saferpay resolved authentication inline
        // and no browser step is needed; the transaction is still finalised by the
        // token-based Authorize, which PSync issues.
        let redirection_data = if response.redirect_required.unwrap_or(false) {
            response.redirect_form().map(Box::new)
        } else {
            None
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(token.clone()),
                redirection_data,
                mandate_reference: None,
                connector_metadata: response.initialize_metadata(&token),
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::AuthenticationPending,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// PSYNC — `Inquire`, or the 3DS second leg (`Authorize` with a token)
// =============================================================================

/// PSync body.
///
/// Saferpay has two different "read the current state" calls depending on what the
/// stored handle is, and they are different endpoints with different bodies:
///
/// * a Saferpay session `Token` (the attempt is `AuthenticationPending` after
///   `Initialize`) ⇒ `POST /Transaction/Authorize` with `{ "Token": ... }`, which
///   finalises the 3DS transaction and returns the real `Transaction`. `Inquire`
///   would answer `TRANSACTION_NOT_FOUND` here, because no transaction exists yet.
/// * a real `Transaction.Id` ⇒ `POST /Transaction/Inquire`.
///
/// The token is carried by `PaymentsSyncData::connector_feature_data` — the
/// `connector_metadata` published by `Initialize` and round-tripped by the
/// caller. See `pending_three_ds_token` for why `encoded_data` is not used.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum SaferpayPSyncRequest {
    /// `POST /Payment/v1/Transaction/Authorize`
    TokenAuthorize {
        #[serde(rename = "RequestHeader")]
        request_header: SaferpayRequestHeader,
        #[serde(rename = "Token")]
        token: String,
    },
    /// `POST /Payment/v1/Transaction/Inquire`
    Inquire {
        #[serde(rename = "RequestHeader")]
        request_header: SaferpayRequestHeader,
        #[serde(rename = "TransactionReference")]
        transaction_reference: SaferpayTransactionReference,
    },
}

type SyncRouterData = RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>;

/// The Saferpay session token to finalise, if this sync is the 3DS second leg.
///
/// The token is carried by `connector_feature_data` — the `connector_metadata`
/// this connector published from `Initialize`, which the caller persists on the
/// attempt and hands back on every subsequent sync. Once the second leg
/// succeeds the sync response replaces that metadata with the capture-id blob
/// (see `SaferpayTransaction::connector_metadata`), so the token disappears and
/// later syncs correctly fall through to `Inquire`.
///
/// `encoded_data` is deliberately NOT used: hyperswitch fills it with the raw
/// query string of the redirect return URL, and Saferpay calls the return URL
/// with no query parameters at all, so it is empty on the one sync that matters
/// and is never a valid token when it is not.
pub fn pending_three_ds_token(request: &PaymentsSyncData) -> Option<String> {
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
    TryFrom<SaferpayRouterData<SyncRouterData, T>> for SaferpayPSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: SaferpayRouterData<SyncRouterData, T>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = SaferpayAuthType::try_from(&router_data.connector_config)?;
        let request_header = SaferpayRequestHeader::new(&auth);

        if let Some(token) = pending_three_ds_token(&router_data.request) {
            return Ok(Self::TokenAuthorize {
                request_header,
                token,
            });
        }

        let transaction_id = router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .map_err(|_| missing_field("connector_transaction_id"))?;

        Ok(Self::Inquire {
            request_header,
            transaction_reference: SaferpayTransactionReference { transaction_id },
        })
    }
}

/// Both PSync modes answer with the shared payments envelope; the newtype only
/// gives the macro framework a distinct response type per flow.
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
            request_header: SaferpayRequestHeader::new(&auth),
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
    pub status: Option<String>,
    #[serde(rename = "Date")]
    pub date: Option<String>,
}

impl SaferpayCaptureResponse {
    fn attempt_status(&self, is_partial: bool) -> AttemptStatus {
        match self.status.as_deref().unwrap_or_default() {
            STATUS_CAPTURED if is_partial => AttemptStatus::PartialCharged,
            STATUS_CAPTURED => AttemptStatus::Charged,
            STATUS_PENDING => AttemptStatus::Pending,
            _ => AttemptStatus::Pending,
        }
    }
}

impl TryFrom<ResponseRouterData<SaferpayCaptureResponse, Self>> for CaptureRouterData {
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<SaferpayCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let request = &item.router_data.request;

        // `minor_amount_authorized` is a response-reporting field and is `None` on
        // every request path, so a capture can only be classified as partial when
        // the caller told us the payment total.
        let is_partial = item
            .router_data
            .resource_common_data
            .amount
            .as_ref()
            .is_some_and(|money| money.amount > request.minor_amount_to_capture);

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
            request_header: SaferpayRequestHeader::new(&auth),
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
            request_header: SaferpayRequestHeader::new(&auth),
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

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<SaferpayRouterData<RefundSyncRouterData, T>> for SaferpayRefundSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: SaferpayRouterData<RefundSyncRouterData, T>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = SaferpayAuthType::try_from(&router_data.connector_config)?;

        Ok(Self {
            request_header: SaferpayRequestHeader::new(&auth),
            transaction_reference: SaferpayTransactionReference {
                transaction_id: router_data.request.connector_refund_id.clone(),
            },
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SaferpayRefundSyncResponse(pub SaferpayPaymentsResponse);

impl TryFrom<ResponseRouterData<SaferpayRefundSyncResponse, Self>> for RefundSyncRouterData {
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<SaferpayRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response.0;

        let transaction = response.transaction.as_ref().ok_or_else(|| {
            error_stack::report!(crate::utils::unexpected_response_fail(
                item.http_code,
                "saferpay: RSync response carried no Transaction object",
            ))
        })?;

        // Guard against being pointed at the payment leg: only a `Type: REFUND`
        // transaction may be reported as a refund state.
        if transaction.transaction_type.as_deref() == Some(TRANSACTION_TYPE_PAYMENT) {
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
