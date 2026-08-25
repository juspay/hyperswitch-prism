//! Tap Payments request/response transformers for the Authorize and PSync flows.
//!
//! Amounts are decimal **major** units ([`FloatMajorUnit`]); auth is a Bearer secret key with the
//! merchant id (`key1`) placed in the request body's `merchant.id`. Status is mapped off Tap's
//! string response codes (`000` → Charged, `001` → Authorized, `100`/`200` → Pending, `601` →
//! Voided) with a redirect (`transaction.url`) forcing `AuthenticationPending`.

use base64::Engine;
use common_enums::{AttemptStatus, Currency, RefundStatus};
use common_utils::types::{FloatMajorUnit, MinorUnit};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId, SplitSettlement, SplitSettlementRefund, SplitValue,
    },
    errors,
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use super::TapRouterData;

// ---------------------------------------------------------------------------
// Auth
// ---------------------------------------------------------------------------

/// Tap's credentials, resolved from [`ConnectorSpecificConfig::Tap`].
///
/// * `api_key`    — the secret key, sent as `Authorization: Bearer <api_key>`.
/// * `merchant_id`— goes into the request body as `merchant.id`.
/// * `public_key` — the RSA card-encryption key, unused until encrypted-card is implemented.
#[derive(Debug, Clone)]
pub struct TapAuthType {
    pub api_key: Secret<String>,
    pub merchant_id: Secret<String>,
    #[allow(dead_code)]
    pub public_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for TapAuthType {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Tap {
                api_key,
                key1,
                api_secret,
                ..
            } => Ok(Self {
                api_key: api_key.to_owned(),
                merchant_id: key1.to_owned(),
                public_key: api_secret.to_owned(),
            }),
            _ => Err(error_stack::report!(
                errors::IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext {
                        suggested_action: Some(
                            "Configure this merchant account's tap connector with a SignatureKey \
                             auth type: api_key = secret key (sk_…), key1 = merchant_id, \
                             api_secret = public key."
                                .to_string(),
                        ),
                        doc_url: Some("https://developers.tap.company/reference".to_string()),
                        additional_context: Some(
                            "The connector_config passed to TapAuthType::try_from was not the \
                             ConnectorSpecificConfig::Tap variant."
                                .to_string(),
                        ),
                    }
                }
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// Flow helpers used by tap.rs (URL selection)
// ---------------------------------------------------------------------------

/// Whether this Authorize is a pre-authorisation (`capture_method == Manual`), which routes to
/// `POST /authorize` instead of `POST /charges`.
pub fn is_manual_capture<T: PaymentMethodDataTypes>(request: &PaymentsAuthorizeData<T>) -> bool {
    matches!(
        request.capture_method,
        Some(common_enums::CaptureMethod::Manual)
    )
}

/// The connector transaction id a PSync must address (`GET /charges/{id}`).
pub fn sync_transaction_id(
    request: &PaymentsSyncData,
) -> Result<String, error_stack::Report<errors::IntegrationError>> {
    request.get_connector_transaction_id()
}

// ---------------------------------------------------------------------------
// Request bodies
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct TapReference {
    pub transaction: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TapPhone {
    /// Sent as a string; Tap accepts both string and numeric forms.
    pub country_code: Secret<String>,
    pub number: Secret<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TapCustomer {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<common_utils::pii::Email>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<TapPhone>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TapMerchant {
    pub id: Secret<String>,
}

/// The `source` object identifying how the money is taken.
///
/// Tap does not accept a raw PAN here: it must be a token (`id = tok_…`) or an RSA-encrypted card
/// blob built with the merchant public key. Only the token path is wired now (see
/// [`build_tap_source`]).
#[derive(Debug, Clone, Serialize)]
pub struct TapSource {
    /// Token source (`tok_…`) or, for capture, the authorized charge id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// RSA-encrypted card blob (base64) built with the merchant `tapPublicKey`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TapPost {
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TapRedirect {
    pub url: String,
}

/// `POST /charges` (auto-capture) or `POST /authorize` (pre-auth) body.
#[derive(Debug, Clone, Serialize)]
pub struct TapPaymentRequest {
    pub amount: FloatMajorUnit,
    pub currency: Currency,
    pub customer_initiated: bool,
    #[serde(rename = "threeDSecure")]
    pub three_d_secure: bool,
    pub save_card: bool,
    pub reference: TapReference,
    pub customer: TapCustomer,
    pub merchant: TapMerchant,
    pub source: TapSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post: Option<TapPost>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect: Option<TapRedirect>,
    /// Tap's native split-settlement model. Absent when the payment is not a split.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destinations: Option<TapDestinations>,
}

/// Resolves the Tap `source` from the payment method.
///
/// **Card handling.** Tap does not accept a raw PAN on `/charges`; the card is RSA-encrypted
/// (PKCS#1 v1.5) with the merchant `tapPublicKey` and sent as `source.card` (see
/// [`encrypt_tap_card`]). Non-card payment methods are out of scope for this connector.
/// The card JSON Tap RSA-encrypts into `source.card` (mirrors euler's `TapCardForEncryption`).
#[derive(Debug, Serialize)]
struct TapCardForEncryption {
    number: String,
    exp_month: String,
    exp_year: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    cvc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

/// Error for any failure while RSA-encrypting the Tap card.
fn tap_encryption_error() -> errors::IntegrationError {
    errors::IntegrationError::RequestEncodingFailed {
        context: errors::IntegrationErrorContext {
            suggested_action: Some(
                "Verify the merchant tapPublicKey is a valid base64 DER / PEM public key."
                    .to_string(),
            ),
            doc_url: None,
            additional_context: Some("tap card RSA encryption failed".to_string()),
        },
    }
}

/// RSA (PKCS#1 v1.5) encrypt the card JSON with the merchant `tapPublicKey`, base64-encoded.
/// The `tapPublicKey` is a base64 DER SubjectPublicKeyInfo; wrap it in PEM headers before loading.
fn encrypt_tap_card<T: PaymentMethodDataTypes + std::fmt::Debug>(
    card: &domain_types::payment_method_data::Card<T>,
    public_key: &Secret<String>,
) -> Result<String, error_stack::Report<errors::IntegrationError>> {
    let payload = TapCardForEncryption {
        number: card.card_number.peek().to_string(),
        exp_month: card.card_exp_month.peek().to_string(),
        exp_year: card.card_exp_year.peek().to_string(),
        cvc: Some(card.card_cvc.peek().to_string()),
        name: card
            .card_holder_name
            .as_ref()
            .map(|name| name.peek().to_string()),
    };
    let plaintext = serde_json::to_string(&payload).change_context(tap_encryption_error())?;

    let key_text: String = public_key
        .peek()
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    let pem = if key_text.starts_with("-----BEGIN PUBLIC KEY-----") {
        key_text
    } else {
        format!("-----BEGIN PUBLIC KEY-----\n{key_text}\n-----END PUBLIC KEY-----")
    };

    let rsa = openssl::rsa::Rsa::public_key_from_pem(pem.as_bytes())
        .change_context(tap_encryption_error())?;
    let mut buffer = vec![0u8; rsa.size() as usize];
    let len = rsa
        .public_encrypt(
            plaintext.as_bytes(),
            &mut buffer,
            openssl::rsa::Padding::PKCS1,
        )
        .change_context(tap_encryption_error())?;
    buffer.truncate(len);
    Ok(common_utils::consts::BASE64_ENGINE.encode(&buffer))
}

/// A token source (`source.id = <token>`, no encryption) — used for stored connector
/// tokens/mandates and wallet variants that carry a Tap-usable token.
fn tap_token_source(token: String) -> TapSource {
    TapSource {
        id: Some(token),
        card: None,
    }
}

/// `NotImplemented` for a wallet variant UCS cannot turn into a Tap `source.id` token.
///
/// Kept honest: only wallet variants whose token we can extract ([`WalletData::get_wallet_token`]
/// — Google Pay / Apple Pay / PayPal SDK) are wired; every other wallet is deferred here with the
/// variant named, rather than guessing an encoding Tap would reject.
fn unsupported_wallet_error(
    wallet: &domain_types::payment_method_data::WalletData,
) -> error_stack::Report<errors::IntegrationError> {
    error_stack::report!(errors::IntegrationError::NotImplemented(
        "tap wallet source not implemented for this wallet variant; only wallets carrying a \
         Tap-usable token (Google Pay / Apple Pay / PayPal SDK) map to source.id"
            .to_string(),
        errors::IntegrationErrorContext {
            suggested_action: Some(
                "Route this payment with a card, a stored connector mandate token, or a \
                 Google Pay / Apple Pay / PayPal SDK wallet."
                    .to_string(),
            ),
            doc_url: Some("https://developers.tap.company/reference".to_string()),
            additional_context: Some(format!("Unsupported wallet variant for tap: {wallet:?}")),
        },
    ))
}

/// Resolves the Tap `source`, mirroring euler's `useId` selection.
///
/// Precedence:
/// 1. A stored connector token / mandate id (`connector_mandate_id`) — a mandate/repeat payment —
///    goes into `source.id` as a plain token string (no encryption).
/// 2. `PaymentMethodData::MandatePayment` (a bare mandate marker) likewise routes to `source.id`,
///    using the same connector token; without a token it is refused.
/// 3. `PaymentMethodData::Wallet` — the wallet token (Google Pay / Apple Pay / PayPal SDK) goes
///    into `source.id`; other wallet variants are refused with [`unsupported_wallet_error`].
/// 4. `PaymentMethodData::Card` — RSA-encrypted (PKCS#1 v1.5) with the merchant `tapPublicKey`
///    into `source.card` (see [`encrypt_tap_card`]). Unchanged.
///
/// Any other payment method is refused with a `NotImplemented` naming the variant.
fn build_tap_source<T: PaymentMethodDataTypes + std::fmt::Debug>(
    payment_method_data: &PaymentMethodData<T>,
    public_key: &Secret<String>,
    connector_mandate_id: Option<String>,
) -> Result<TapSource, error_stack::Report<errors::IntegrationError>> {
    // 1. A stored connector token / mandate id always wins: it is a plain token → source.id.
    if let Some(token) = connector_mandate_id
        .as_ref()
        .filter(|token| !token.is_empty())
    {
        return Ok(tap_token_source(token.clone()));
    }

    match payment_method_data {
        PaymentMethodData::Card(card) => {
            let encrypted_card = encrypt_tap_card(card, public_key)?;
            Ok(TapSource {
                id: None,
                card: Some(encrypted_card),
            })
        }
        // A bare mandate marker with no accompanying connector token cannot be turned into a
        // Tap source: source.id needs the stored token string.
        PaymentMethodData::MandatePayment => Err(error_stack::report!(
            errors::IntegrationError::MissingRequiredField {
                field_name: "mandate_id.connector_mandate_id",
                context: errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "A tap mandate/stored-token payment must carry the connector mandate id \
                         (the Tap token) in mandate_id; it is sent as source.id."
                            .to_string(),
                    ),
                    doc_url: Some("https://developers.tap.company/reference".to_string()),
                    additional_context: Some(
                        "PaymentMethodData::MandatePayment received for tap without a \
                         connector_mandate_id token."
                            .to_string(),
                    ),
                },
            }
        )),
        PaymentMethodData::Wallet(wallet) => {
            // Only wallets whose token we can extract map cleanly to source.id; others are
            // deferred with the variant named (no guessed encoding).
            let token = wallet
                .get_wallet_token()
                .map_err(|_| unsupported_wallet_error(wallet))?;
            Ok(tap_token_source(token.peek().to_string()))
        }
        other => Err(error_stack::report!(
            errors::IntegrationError::NotImplemented(
                "tap supports card, stored connector token/mandate and token-bearing wallet \
                 payments in this connector; other APMs and bank transfers are out of scope"
                    .to_string(),
                errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Route this payment with a card, a stored connector mandate token, or a \
                         Google Pay / Apple Pay / PayPal SDK wallet."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(format!(
                        "Unsupported payment_method_data variant for tap: {other:?}"
                    )),
                },
            )
        )),
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TapRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for TapPaymentRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: TapRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;
        let common = &router_data.resource_common_data;

        let auth = TapAuthType::try_from(&router_data.connector_config)?;

        let amount = item
            .connector
            .amount_converter
            .convert(request.minor_amount, request.currency)
            .change_context(errors::IntegrationError::AmountConversionFailed {
                context: errors::IntegrationErrorContext {
                    suggested_action: None,
                    doc_url: None,
                    additional_context: Some(format!(
                        "Failed to convert minor_amount {} {} into tap's decimal major-unit \
                         amount.",
                        request.minor_amount.get_amount_as_i64(),
                        request.currency
                    )),
                },
            })?;

        let source = build_tap_source(
            &request.payment_method_data,
            &auth.public_key,
            request.connector_mandate_id(),
        )?;

        // three_ds off for NoThreeDs; PaymentFlowData carries the resolved auth_type.
        let three_d_secure = common.auth_type == common_enums::AuthenticationType::ThreeDs;

        let first_name = request
            .customer_name
            .as_ref()
            .and_then(|name| name.split_whitespace().next().map(String::from))
            .map(Secret::new)
            .or_else(|| common.get_optional_billing_first_name());
        let last_name = request
            .customer_name
            .as_ref()
            .and_then(|name| {
                name.split_once(char::is_whitespace)
                    .map(|(_, last)| last.trim().to_string())
            })
            .filter(|last| !last.is_empty())
            .map(Secret::new)
            .or_else(|| common.get_optional_billing_last_name());

        let email = request
            .email
            .clone()
            .or_else(|| common.get_optional_billing_email());

        let customer = TapCustomer {
            first_name,
            last_name,
            email,
            // Phone is optional on Tap; omitted when we cannot source a full number. Splitting a
            // raw billing phone into country_code/number reliably is deferred.
            phone: None,
        };

        let reference = TapReference {
            transaction: common.connector_request_reference_id.clone(),
            order: request.merchant_order_id.clone(),
        };

        let post = request.webhook_url.clone().map(|url| TapPost { url });
        let redirect = request
            .router_return_url
            .clone()
            .or_else(|| request.complete_authorize_url.clone())
            .map(|url| TapRedirect { url });

        let destinations = build_authorize_destinations(
            request.split_settlement.as_ref(),
            request.currency,
            item.connector.amount_converter,
        )?;

        Ok(Self {
            amount,
            currency: request.currency,
            customer_initiated: true,
            three_d_secure,
            save_card: false,
            reference,
            customer,
            merchant: TapMerchant {
                id: auth.merchant_id,
            },
            source,
            post,
            redirect,
            destinations,
        })
    }
}

// ---------------------------------------------------------------------------
// Error response
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TapError {
    pub code: String,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TapErrorResponse {
    #[serde(default)]
    pub errors: Vec<TapError>,
}

// ---------------------------------------------------------------------------
// Charge response
// ---------------------------------------------------------------------------

/// The `response` object nested in a charge/sync response.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TapResponseDetail {
    pub code: Option<String>,
    pub message: Option<String>,
}

/// The `transaction` object; `url` present ⇒ a 3DS redirect is required.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TapTransaction {
    pub url: Option<String>,
}

/// Shared `chargeResponse` envelope for `/charges`, `/authorize` and `GET /charges/{id}`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TapChargeResponse {
    /// `chg_…` or `auth_…`.
    pub id: String,
    /// String status code (`CAPTURED`, `AUTHORIZED`, `INITIATED`, …) — mapped via the numeric
    /// `response.code`, or the status string as a fallback.
    pub status: Option<String>,
    pub response: Option<TapResponseDetail>,
    pub transaction: Option<TapTransaction>,
}

/// `GET /charges/{id}` returns the same `chargeResponse` envelope as the Authorize legs.
///
/// A distinct alias rather than a direct reuse: `create_all_prerequisites!` mints one
/// `<Ident>Templating` marker struct per named response body, so two flows cannot both name
/// `TapChargeResponse` directly without defining that marker twice.
pub type TapSyncResponse = TapChargeResponse;

/// `POST /charges` capture returns the same `chargeResponse` envelope. A distinct alias for the
/// same macro reason as [`TapSyncResponse`].
pub type TapCaptureResponse = TapChargeResponse;

/// `POST /authorize/{id}/void` returns the same charge-shape envelope. A distinct alias for the
/// same macro reason as [`TapSyncResponse`].
pub type TapVoidResponse = TapChargeResponse;

impl TapChargeResponse {
    /// The `transaction.url` when a 3DS redirect is required.
    fn redirect_url(&self) -> Option<&str> {
        self.transaction
            .as_ref()
            .and_then(|transaction| transaction.url.as_deref())
            .filter(|url| !url.is_empty())
    }

    /// The Tap numeric result code (`000`, `001`, `100`, …) from `response.code`.
    fn result_code(&self) -> Option<&str> {
        self.response
            .as_ref()
            .and_then(|response| response.code.as_deref())
    }
}

/// Maps a Tap charge/sync response onto a UCS attempt status.
///
/// Order matters: a present redirect URL wins (a 3DS challenge is pending regardless of the numeric
/// code), then the numeric result code decides. Every non-terminal/unknown code stays `Pending` so
/// a PSync can resolve it; recognised failure codes map to `Failure`.
fn map_attempt_status(response: &TapChargeResponse) -> AttemptStatus {
    if response.redirect_url().is_some() {
        return AttemptStatus::AuthenticationPending;
    }
    match response.result_code() {
        Some("000") => AttemptStatus::Charged,
        Some("001") => AttemptStatus::Authorized,
        Some("100") | Some("200") => AttemptStatus::Pending,
        Some("601") => AttemptStatus::Voided,
        Some(code) if is_failure_code(code) => AttemptStatus::Failure,
        // No numeric code yet (freshly INITIATED) or an unrecognised one — let PSync resolve it.
        _ => AttemptStatus::Pending,
    }
}

/// Whether a Tap numeric code is one of the documented failure families (auth/capture/void
/// declines, validation and gateway errors). Anything outside these ranges is treated as
/// non-terminal so it is not prematurely failed.
fn is_failure_code(code: &str) -> bool {
    let Ok(numeric) = code.parse::<u32>() else {
        return false;
    };
    matches!(numeric,
        401..=408
        | 501..=515
        | 701..=704
        | 801
        | 1100..=1202
        | 2100..=2108
        | 9998
    )
}

fn build_error_response(response: &TapChargeResponse, http_code: u16) -> ErrorResponse {
    let code = response
        .result_code()
        .map(String::from)
        .or_else(|| response.status.clone())
        .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string());
    let message = response
        .response
        .as_ref()
        .and_then(|detail| detail.message.clone())
        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string());

    ErrorResponse {
        status_code: http_code,
        code,
        message: message.clone(),
        reason: Some(message),
        attempt_status: Some(domain_types::router_data::FlowStatus::Payment(
            AttemptStatus::Failure,
        )),
        connector_transaction_id: Some(response.id.clone()),
        network_decline_code: None,
        network_advice_code: None,
        network_error_message: None,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
}

/// Builds the `PaymentsResponseData::TransactionResponse` (or the error) shared by Authorize and
/// PSync — the two flows deserialise the same envelope and map status identically.
fn build_payments_response(
    response: &TapChargeResponse,
    status: AttemptStatus,
    http_code: u16,
) -> Result<PaymentsResponseData, ErrorResponse> {
    if status == AttemptStatus::Failure {
        return Err(build_error_response(response, http_code));
    }

    let redirection_data = response.redirect_url().map(|url| {
        Box::new(RedirectForm::Form {
            endpoint: url.to_string(),
            method: common_utils::request::Method::Get,
            form_fields: std::collections::HashMap::new(),
        })
    });

    Ok(PaymentsResponseData::TransactionResponse {
        resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
        redirection_data,
        mandate_reference: None,
        connector_metadata: None,
        network_txn_id: None,
        network_txn_link_id: None,
        connector_response_reference_id: Some(response.id.clone()),
        incremental_authorization_allowed: None,
        splits: None,
        status_code: http_code,
    })
}

impl<T: PaymentMethodDataTypes> TryFrom<crate::types::ResponseRouterData<TapChargeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: crate::types::ResponseRouterData<TapChargeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let status = map_attempt_status(&response);
        let payments_response = build_payments_response(&response, status, item.http_code);

        Ok(Self {
            response: payments_response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl TryFrom<crate::types::ResponseRouterData<TapSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: crate::types::ResponseRouterData<TapSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let status = map_attempt_status(&response);
        let payments_response = build_payments_response(&response, status, item.http_code);

        Ok(Self {
            response: payments_response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ---------------------------------------------------------------------------
// Split settlement (Tap `destinations`)
// ---------------------------------------------------------------------------

/// Error code surfaced when a split line omits the required `connector_sub_account_id`.
const SPLIT_SETTLEMENT_MISSING_GW_SUB_MERCHANT_ID: &str =
    "SPLIT_SETTLEMENT_MISSING_GW_SUB_MERCHANT_ID";
/// Error code surfaced when a split-settlement refund carries no vendor detail lines.
const SPLIT_DETAILS_MISSING: &str = "SPLIT_DETAILS_MISSING";

/// A single Tap settlement destination — the sub-account to credit, the amount (decimal major
/// units) and the currency.
#[derive(Debug, Clone, Serialize)]
pub struct TapDestination {
    pub id: String,
    pub amount: f64,
    pub currency: Currency,
}

/// Tap's `destinations` wrapper: `{ "destination": [ … ] }`.
#[derive(Debug, Clone, Serialize)]
pub struct TapDestinations {
    pub destination: Vec<TapDestination>,
}

/// The `AmountConvertor` handle carried on the connector, specialised to Tap's decimal major
/// unit. Both split builders take it so the destination amounts are converted exactly as the
/// top-level `amount` is.
type TapAmountConverter =
    &'static (dyn common_utils::types::AmountConvertor<Output = FloatMajorUnit> + Sync);

/// Converts a domain `SplitValue::Amount` into Tap's decimal major-unit `f64`. `Percentage` is
/// rejected: Tap accepts absolute amounts only.
fn split_value_to_major(
    split_value: &SplitValue,
    currency: Currency,
    amount_converter: TapAmountConverter,
) -> Result<f64, error_stack::Report<errors::IntegrationError>> {
    let minor = match split_value {
        SplitValue::Amount(amount) => *amount,
        SplitValue::Percentage(percent) => {
            return Err(error_stack::report!(
                errors::IntegrationError::NotImplemented(
                    "tap split settlement accepts absolute destination amounts only; a \
                     percentage split cannot be forwarded to Tap's destinations model"
                        .to_string(),
                    errors::IntegrationErrorContext {
                        suggested_action: Some(
                            "Express each vendor split as an absolute amount (SplitValue::Amount) \
                             rather than a percentage."
                                .to_string(),
                        ),
                        doc_url: Some(
                            "https://developers.tap.company/docs/split-settlement".to_string(),
                        ),
                        additional_context: Some(format!(
                            "Received SplitValue::Percentage({percent}) for a tap destination."
                        )),
                    },
                )
            ));
        }
    };

    convert_minor_to_major(minor, currency, amount_converter)
}

/// Shared `MinorUnit` → decimal-major `f64` conversion for split destination amounts.
fn convert_minor_to_major(
    minor: MinorUnit,
    currency: Currency,
    amount_converter: TapAmountConverter,
) -> Result<f64, error_stack::Report<errors::IntegrationError>> {
    amount_converter
        .convert(minor, currency)
        .map(|major| major.0)
        .change_context(errors::IntegrationError::AmountConversionFailed {
            context: errors::IntegrationErrorContext {
                suggested_action: None,
                doc_url: None,
                additional_context: Some(format!(
                    "Failed to convert split amount {} {} into tap's decimal major-unit \
                     destination amount.",
                    minor.get_amount_as_i64(),
                    currency
                )),
            },
        })
}

/// Missing-sub-account error, shared by the authorize and refund split builders.
fn missing_sub_account_error() -> error_stack::Report<errors::IntegrationError> {
    error_stack::report!(errors::IntegrationError::MissingRequiredField {
        field_name: "split_settlement.vendor_split_details.connector_sub_account_id",
        context: errors::IntegrationErrorContext {
            suggested_action: Some(format!(
                "{SPLIT_SETTLEMENT_MISSING_GW_SUB_MERCHANT_ID}: every tap split destination \
                 requires connector_sub_account_id (the Tap sub-account/destination id to \
                 credit). Populate it on each vendor split line."
            )),
            doc_url: Some("https://developers.tap.company/docs/split-settlement".to_string()),
            additional_context: None,
        },
    })
}

/// Builds the Authorize/Capture `destinations` from the domain `split_settlement`.
///
/// Returns `None` when there is no split settlement, so the field is simply omitted for
/// non-split payments. Each `vendor_split_details[i]` maps to one destination;
/// `connector_sub_account_id` is required and a percentage split is rejected.
fn build_authorize_destinations(
    split_settlement: Option<&SplitSettlement>,
    currency: Currency,
    amount_converter: TapAmountConverter,
) -> Result<Option<TapDestinations>, error_stack::Report<errors::IntegrationError>> {
    let Some(split) = split_settlement else {
        return Ok(None);
    };

    let mut destination = Vec::with_capacity(split.vendor_split_details.len());
    for vendor in &split.vendor_split_details {
        let id = vendor
            .connector_sub_account_id
            .clone()
            .filter(|value| !value.is_empty())
            .ok_or_else(missing_sub_account_error)?;
        let amount = split_value_to_major(&vendor.split_value, currency, amount_converter)?;
        destination.push(TapDestination {
            id,
            amount,
            currency,
        });
    }

    Ok(Some(TapDestinations { destination }))
}

/// Builds the Refund `destinations` from the domain `split_settlement_refund`.
///
/// Mirrors [`build_authorize_destinations`] but keyed off `SplitSettlementRefund`: when the field
/// is present but carries no vendor lines the request is refused with `SPLIT_DETAILS_MISSING`, and
/// each line still requires `connector_sub_account_id`.
fn build_refund_destinations(
    split_settlement_refund: Option<&SplitSettlementRefund>,
    currency: Currency,
    amount_converter: TapAmountConverter,
) -> Result<Option<TapDestinations>, error_stack::Report<errors::IntegrationError>> {
    let Some(split) = split_settlement_refund else {
        return Ok(None);
    };

    if split.vendor_split_details.is_empty() {
        return Err(error_stack::report!(
            errors::IntegrationError::MissingRequiredField {
                field_name: "split_settlement_refund.vendor_split_details",
                context: errors::IntegrationErrorContext {
                    suggested_action: Some(format!(
                        "{SPLIT_DETAILS_MISSING}: this refund is flagged as a split-settlement \
                         refund but carries no vendor_split_details. Populate the per-vendor \
                         refund split lines, or omit split_settlement_refund for a plain refund."
                    )),
                    doc_url: Some(
                        "https://developers.tap.company/docs/split-settlement".to_string()
                    ),
                    additional_context: None,
                },
            }
        ));
    }

    let mut destination = Vec::with_capacity(split.vendor_split_details.len());
    for vendor in &split.vendor_split_details {
        let id = vendor
            .connector_sub_account_id
            .clone()
            .filter(|value| !value.is_empty())
            .ok_or_else(missing_sub_account_error)?;
        let amount = split_value_to_major(&vendor.split_value, currency, amount_converter)?;
        destination.push(TapDestination {
            id,
            amount,
            currency,
        });
    }

    Ok(Some(TapDestinations { destination }))
}

// ---------------------------------------------------------------------------
// Capture — re-post to POST /charges referencing the authorised charge.
// ---------------------------------------------------------------------------

/// `POST /charges` capture body. Tap captures a pre-authorisation by re-posting a charge that
/// references the authorised source (the `auth_…` id) with `save_card:false`. The response is the
/// shared `chargeResponse` envelope.
#[derive(Debug, Clone, Serialize)]
pub struct TapCaptureRequest {
    pub amount: FloatMajorUnit,
    pub currency: Currency,
    pub customer_initiated: bool,
    pub save_card: bool,
    /// The authorised charge/authorisation id being captured (`source.id = auth_…`).
    pub source: TapSource,
    pub merchant: TapMerchant,
    pub reference: TapReference,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destinations: Option<TapDestinations>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TapRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for TapCaptureRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: TapRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;
        let common = &router_data.resource_common_data;

        let auth = TapAuthType::try_from(&router_data.connector_config)?;

        let amount = item
            .connector
            .amount_converter
            .convert(request.minor_amount_to_capture, request.currency)
            .change_context(errors::IntegrationError::AmountConversionFailed {
                context: errors::IntegrationErrorContext {
                    suggested_action: None,
                    doc_url: None,
                    additional_context: Some(format!(
                        "Failed to convert minor_amount_to_capture {} {} into tap's decimal \
                         major-unit amount.",
                        request.minor_amount_to_capture.get_amount_as_i64(),
                        request.currency
                    )),
                },
            })?;

        // The authorised charge id (`auth_…`) is the capture source.
        let authorized_id = request.get_connector_transaction_id().change_context(
            errors::IntegrationError::MissingRequiredField {
                field_name: "connector_transaction_id",
                context: errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "A tap capture references the authorised charge id (auth_…) as its \
                         source; the pre-auth's connector_transaction_id must be supplied."
                            .to_string(),
                    ),
                    doc_url: Some(
                        "https://developers.tap.company/reference/create-a-charge".to_string(),
                    ),
                    additional_context: None,
                },
            },
        )?;

        let destinations = build_authorize_destinations(
            request.split_settlement.as_ref(),
            request.currency,
            item.connector.amount_converter,
        )?;

        Ok(Self {
            amount,
            currency: request.currency,
            customer_initiated: true,
            save_card: false,
            source: TapSource {
                id: Some(authorized_id),
                card: None,
            },
            merchant: TapMerchant {
                id: auth.merchant_id,
            },
            reference: TapReference {
                transaction: common.connector_request_reference_id.clone(),
                order: request.merchant_order_id.clone(),
            },
            destinations,
        })
    }
}

/// Maps a Tap capture response onto a UCS attempt status. `000` ⇒ Charged; `100`/`200` ⇒
/// CaptureInitiated (pending); recognised failure codes ⇒ CaptureFailed; anything else stays
/// pending so a PSync can resolve it.
fn map_capture_status(response: &TapChargeResponse) -> AttemptStatus {
    match response.result_code() {
        Some("000") => AttemptStatus::Charged,
        Some("100") | Some("200") => AttemptStatus::CaptureInitiated,
        Some(code) if is_failure_code(code) => AttemptStatus::CaptureFailed,
        _ => AttemptStatus::CaptureInitiated,
    }
}

impl TryFrom<crate::types::ResponseRouterData<TapCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: crate::types::ResponseRouterData<TapCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let status = map_capture_status(&response);

        let payments_response = if status == AttemptStatus::CaptureFailed {
            Err(build_flow_error_response(&response, status, item.http_code))
        } else {
            Ok(build_transaction_response(&response, item.http_code))
        };

        Ok(Self {
            response: payments_response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ---------------------------------------------------------------------------
// Void — POST /authorize/{id}/void, empty body.
// ---------------------------------------------------------------------------

/// Maps a Tap void response onto a UCS attempt status. `601` ⇒ Voided; `100`/`200` ⇒
/// VoidInitiated (pending); recognised failure codes ⇒ VoidFailed; anything else stays
/// VoidInitiated so a PSync can resolve it.
fn map_void_status(response: &TapChargeResponse) -> AttemptStatus {
    match response.result_code() {
        Some("601") => AttemptStatus::Voided,
        Some("000") => AttemptStatus::Voided,
        Some("100") | Some("200") => AttemptStatus::VoidInitiated,
        Some(code) if is_failure_code(code) => AttemptStatus::VoidFailed,
        _ => AttemptStatus::VoidInitiated,
    }
}

impl TryFrom<crate::types::ResponseRouterData<TapSyncResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: crate::types::ResponseRouterData<TapSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let status = map_void_status(&response);

        let payments_response = if status == AttemptStatus::VoidFailed {
            Err(build_flow_error_response(&response, status, item.http_code))
        } else {
            Ok(build_transaction_response(&response, item.http_code))
        };

        Ok(Self {
            response: payments_response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ---------------------------------------------------------------------------
// Shared payment-flow response builders (Capture/Void)
// ---------------------------------------------------------------------------

/// The `TransactionResponse` shared by the Capture and Void success paths. Identical field set to
/// the Authorize/PSync builder, minus the redirect (capture/void never challenge).
fn build_transaction_response(
    response: &TapChargeResponse,
    http_code: u16,
) -> PaymentsResponseData {
    PaymentsResponseData::TransactionResponse {
        resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
        redirection_data: None,
        mandate_reference: None,
        connector_metadata: None,
        network_txn_id: None,
        network_txn_link_id: None,
        connector_response_reference_id: Some(response.id.clone()),
        incremental_authorization_allowed: None,
        splits: None,
        status_code: http_code,
    }
}

/// [`build_error_response`] with the caller's flow-specific status. `build_error_response` always
/// stamps `AttemptStatus::Failure`; a capture/void failure must report its own
/// `CaptureFailed`/`VoidFailed` so the payment attempt state is not overwritten with a bare
/// `Failure`.
fn build_flow_error_response(
    response: &TapChargeResponse,
    status: AttemptStatus,
    http_code: u16,
) -> ErrorResponse {
    let mut error = build_error_response(response, http_code);
    error.attempt_status = Some(domain_types::router_data::FlowStatus::Payment(status));
    error
}

// ---------------------------------------------------------------------------
// Refund — POST /refunds, and RSync — GET /refunds/{id}.
// ---------------------------------------------------------------------------

/// The `metadata` object echoed back on a Tap refund, carrying the UCS refund/txn references.
#[derive(Debug, Clone, Serialize)]
pub struct TapRefundMetadata {
    pub refund_id: String,
    pub txn_id: String,
}

/// `POST /refunds` body.
#[derive(Debug, Clone, Serialize)]
pub struct TapRefundRequest {
    /// The charge to refund (`chg_…`) — the original payment's connector transaction id.
    pub charge_id: String,
    pub amount: FloatMajorUnit,
    pub currency: Currency,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post: Option<TapPost>,
    pub metadata: TapRefundMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destinations: Option<TapDestinations>,
}

/// Tap's fixed reason string for a merchant-initiated refund.
const TAP_REFUND_REASON: &str = "MERCHANT_INITIATED_REFUND";

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TapRouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for TapRefundRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: TapRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;

        let amount = item
            .connector
            .amount_converter
            .convert(request.minor_refund_amount, request.currency)
            .change_context(errors::IntegrationError::AmountConversionFailed {
                context: errors::IntegrationErrorContext {
                    suggested_action: None,
                    doc_url: None,
                    additional_context: Some(format!(
                        "Failed to convert minor_refund_amount {} {} into tap's decimal \
                         major-unit refund amount.",
                        request.minor_refund_amount.get_amount_as_i64(),
                        request.currency
                    )),
                },
            })?;

        let destinations = build_refund_destinations(
            request.split_settlement_refund.as_ref(),
            request.currency,
            item.connector.amount_converter,
        )?;

        let post = request.webhook_url.clone().map(|url| TapPost { url });

        Ok(Self {
            charge_id: request.connector_transaction_id.clone(),
            amount,
            currency: request.currency,
            reason: TAP_REFUND_REASON.to_string(),
            post,
            metadata: TapRefundMetadata {
                refund_id: request.refund_id.clone(),
                txn_id: request.connector_transaction_id.clone(),
            },
            destinations,
        })
    }
}

/// The nested `response` object on a Tap refund envelope.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TapRefundResponseDetail {
    pub code: Option<String>,
    pub message: Option<String>,
}

/// Tap refund envelope for `POST /refunds` and `GET /refunds/{id}`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TapRefundResponse {
    /// `ref_…`.
    pub id: String,
    /// The original charge id this refund is against (`chg_…`).
    pub charge_id: Option<String>,
    /// String status code — the numeric outcome lives in `response.code`.
    pub status: Option<String>,
    pub response: Option<TapRefundResponseDetail>,
}

/// `GET /refunds/{id}` returns the same envelope as `POST /refunds`. A distinct alias because
/// `create_all_prerequisites!` mints one `…Templating` marker per named response body, so two
/// flows cannot both name `TapRefundResponse` directly.
pub type TapRefundSyncResponse = TapRefundResponse;

impl TapRefundResponse {
    /// The numeric result code (`000`, `100`, …) from `response.code`.
    fn result_code(&self) -> Option<&str> {
        self.response
            .as_ref()
            .and_then(|detail| detail.code.as_deref())
    }
}

/// Maps a Tap refund/refund-sync response onto a UCS **refund** status. `000` ⇒ Success;
/// `100`/`200` ⇒ Pending; recognised failure codes ⇒ Failure; anything else stays Pending so an
/// RSync can resolve it. Always [`RefundStatus`], never `AttemptStatus`.
fn map_refund_status(response: &TapRefundResponse) -> RefundStatus {
    match response.result_code() {
        Some("000") => RefundStatus::Success,
        Some("100") | Some("200") => RefundStatus::Pending,
        Some(code) if is_failure_code(code) => RefundStatus::Failure,
        _ => RefundStatus::Pending,
    }
}

/// Builds the refund `ErrorResponse`. Reports `FlowStatus::Refund(Failure)` so a failed refund
/// leaves the payment attempt untouched.
fn build_refund_error_response(response: &TapRefundResponse, http_code: u16) -> ErrorResponse {
    let code = response
        .result_code()
        .map(String::from)
        .or_else(|| response.status.clone())
        .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string());
    let message = response
        .response
        .as_ref()
        .and_then(|detail| detail.message.clone())
        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string());

    ErrorResponse {
        status_code: http_code,
        code,
        message: message.clone(),
        reason: Some(message),
        attempt_status: Some(domain_types::router_data::FlowStatus::Refund(
            RefundStatus::Failure,
        )),
        connector_transaction_id: Some(response.id.clone()),
        network_decline_code: None,
        network_advice_code: None,
        network_error_message: None,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
}

/// The `RefundsResponseData` (or error) shared by Refund and RSync — same envelope, same status
/// map, `connector_refund_id = id`.
fn build_refunds_response(
    response: &TapRefundResponse,
    status: RefundStatus,
    http_code: u16,
) -> Result<RefundsResponseData, ErrorResponse> {
    if status == RefundStatus::Failure {
        return Err(build_refund_error_response(response, http_code));
    }

    Ok(RefundsResponseData {
        connector_refund_id: response.id.clone(),
        refund_status: status,
        status_code: http_code,
        acquirer_reference_number: None,
    })
}

impl TryFrom<crate::types::ResponseRouterData<TapRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: crate::types::ResponseRouterData<TapRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let status = map_refund_status(&response);
        let refunds_response = build_refunds_response(&response, status, item.http_code);

        Ok(Self {
            response: refunds_response,
            resource_common_data: RefundFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl TryFrom<crate::types::ResponseRouterData<TapRefundSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: crate::types::ResponseRouterData<TapRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let status = map_refund_status(&response);
        let refunds_response = build_refunds_response(&response, status, item.http_code);

        Ok(Self {
            response: refunds_response,
            resource_common_data: RefundFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

/// The connector refund id an RSync must address (`GET /refunds/{id}`).
pub fn refund_sync_id(request: &RefundSyncData) -> String {
    request.connector_refund_id.clone()
}

// ---------------------------------------------------------------------------
// Incoming webhooks (POST from Tap → us)
// ---------------------------------------------------------------------------
//
// Mirrors euler `Gateway/Tap/Flows/Webhook.hs`. A Tap webhook body is either a **charge** webhook
// or a **refund** webhook; both share the `id` / `status` / `response{code,message}` shape, so one
// permissive struct deserialises both and [`TapWebhookBody::classify`] decides which it is:
//
// * **Charge** — carries `object` (`AUTHORIZE`/`CHARGE`) and a `reference.transaction` (the merchant
//   txn id). `(object,status)` of `(AUTHORIZE,AUTHORIZED)` or `(CHARGE,CAPTURED)` is a payment
//   event; anything else is treated as pending/unspecified.
// * **Refund** — its `id` is a `ref_…` and it carries `metadata{txn_id,refund_id}`. The merchant
//   txn id is `metadata.txn_id`, the merchant refund ref is `metadata.refund_id`.
//
// **No signature verification.** Tap does not sign its webhooks; euler trusts the body and confirms
// via a follow-up sync. `verify_webhook_source` therefore returns `Ok(true)` (documented in tap.rs).

/// The `reference` object echoed on a charge webhook (`reference.transaction` = merchant txn id).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TapWebhookReference {
    pub transaction: Option<String>,
    pub order: Option<String>,
}

/// The `metadata` object echoed on a refund webhook, carrying the UCS refund/txn references.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TapWebhookMetadata {
    pub txn_id: Option<String>,
    pub refund_id: Option<String>,
}

/// A single incoming Tap webhook body — permissive over both the charge and refund shapes.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TapWebhookBody {
    /// `chg_…`/`auth_…` for a charge webhook, `ref_…` for a refund webhook.
    pub id: String,
    /// The object type on a charge webhook (`AUTHORIZE`, `CHARGE`, …). Absent on refund webhooks.
    pub object: Option<String>,
    /// String status (`AUTHORIZED`, `CAPTURED`, …).
    pub status: Option<String>,
    pub response: Option<TapResponseDetail>,
    pub transaction: Option<TapTransaction>,
    pub reference: Option<TapWebhookReference>,
    pub metadata: Option<TapWebhookMetadata>,
}

/// Which kind of webhook a [`TapWebhookBody`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TapWebhookKind {
    /// A charge/pre-auth webhook (payment event).
    Charge,
    /// A refund webhook (refund event).
    Refund,
}

impl TapWebhookBody {
    /// Classify the body as a charge or refund webhook.
    ///
    /// Refund webhooks carry a `metadata` block and/or a `ref_…` id and no `object`; charge
    /// webhooks carry an `object`. Mirrors euler's two-way `TapWebhookResponse` split.
    pub fn classify(&self) -> TapWebhookKind {
        let looks_like_refund =
            self.metadata.is_some() || self.id.starts_with("ref_") || self.object.is_none();
        if looks_like_refund && self.object.is_none() {
            TapWebhookKind::Refund
        } else {
            TapWebhookKind::Charge
        }
    }

    /// Reconstruct the shared charge envelope so the existing [`map_attempt_status`] status map is
    /// reused rather than duplicating status logic in the webhook path.
    fn as_charge_response(&self) -> TapChargeResponse {
        TapChargeResponse {
            id: self.id.clone(),
            status: self.status.clone(),
            response: self.response.clone(),
            transaction: self.transaction.clone(),
        }
    }

    /// Reconstruct the shared refund envelope so the existing [`map_refund_status`] status map is
    /// reused for the refund webhook path.
    fn as_refund_response(&self) -> TapRefundResponse {
        TapRefundResponse {
            id: self.id.clone(),
            charge_id: self.metadata.as_ref().and_then(|meta| meta.txn_id.clone()),
            status: self.status.clone(),
            response: self.response.as_ref().map(|detail| TapRefundResponseDetail {
                code: detail.code.clone(),
                message: detail.message.clone(),
            }),
        }
    }

    /// The merchant transaction id echoed by the webhook (`reference.transaction` on a charge,
    /// `metadata.txn_id` on a refund).
    pub fn merchant_transaction_id(&self) -> Option<String> {
        match self.classify() {
            TapWebhookKind::Charge => self
                .reference
                .as_ref()
                .and_then(|reference| reference.transaction.clone()),
            TapWebhookKind::Refund => {
                self.metadata.as_ref().and_then(|meta| meta.txn_id.clone())
            }
        }
    }

    /// The merchant refund reference echoed by a refund webhook (`metadata.refund_id`).
    pub fn merchant_refund_id(&self) -> Option<String> {
        self.metadata.as_ref().and_then(|meta| meta.refund_id.clone())
    }

    /// Map a charge webhook to a payment [`AttemptStatus`] using the shared charge status map.
    pub fn payment_attempt_status(&self) -> AttemptStatus {
        map_attempt_status(&self.as_charge_response())
    }

    /// Map a refund webhook to a [`RefundStatus`] using the shared refund status map.
    pub fn refund_status(&self) -> RefundStatus {
        map_refund_status(&self.as_refund_response())
    }

    /// The Tap numeric result code (`response.code`), if any.
    pub fn result_code(&self) -> Option<&str> {
        self.response
            .as_ref()
            .and_then(|detail| detail.code.as_deref())
    }

    /// The human-readable message (`response.message`), if any.
    pub fn result_message(&self) -> Option<String> {
        self.response
            .as_ref()
            .and_then(|detail| detail.message.clone())
    }
}
