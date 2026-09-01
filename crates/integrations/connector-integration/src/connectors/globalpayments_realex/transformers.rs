//! Transformers for the Global Payments **Ecommerce XML API** (legacy *Realex Payments*
//! "XML API" / "Remote API").
//!
//! This is **not** the GP-API JSON product handled by the `globalpay` connector. Everything here
//! speaks XML over a single CGI endpoint and is authenticated by a two-stage SHA-1 digest.
//!
//! Reference: `grace/rulesbook/codegen/references/globalpayments_realex/technical_specification.md`

use common_enums::{AttemptStatus, CaptureMethod, CardNetwork};
use common_utils::{pii::SecretSerdeValue, types::MinorUnit};
use domain_types::{
    connector_flow::{Authorize, Capture, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, ResponseId as DomainResponseId,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::{Card, PaymentMethodData, PaymentMethodDataTypes},
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    utils::{get_card_issuer, CardIssuer},
};
use error_stack::{report, ResultExt};
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use super::super::macros::GetSoapXml;
use super::GlobalpaymentsRealexRouterData;
use crate::types::ResponseRouterData;

// =============================================================================
// CONSTANTS
// =============================================================================

/// Root element `type` attribute for the Authorize flow.
pub const REQUEST_TYPE_AUTH: &str = "auth";
/// Root element `type` attribute for the Capture flow. Same endpoint, same envelope — only the
/// `type` attribute and the body elements differ (tech spec §12.1).
pub const REQUEST_TYPE_SETTLE: &str = "settle";
/// Root element `type` attribute for the Void flow. Same endpoint and envelope again — a `void`
/// reverses an authorization that has not been settled yet (tech spec §12.2).
pub const REQUEST_TYPE_VOID: &str = "void";
/// Ecommerce channel. The other documented value is `MOTO`, which is out of scope.
pub const CHANNEL_ECOM: &str = "ECOM";
/// `<cvn><presind>` — `1` means "CVN present on the card and supplied by the cardholder".
pub const CVN_PRESENCE_INDICATOR_PRESENT: u8 = 1;
/// Cardholder name is mandatory on the wire; RealEx rejects an empty `<chname>`.
pub const DEFAULT_CARDHOLDER_NAME: &str = "Cardholder";
/// `<orderid>` maximum length accepted by the gateway.
const ORDER_ID_MAX_LEN: usize = 50;
/// Success result code. Every other value is a decline or an error (tech spec §9).
const RESULT_SUCCESS: &str = "00";

// =============================================================================
// AUTH
// =============================================================================

/// RealEx credentials, sourced from the `MultiAuthKey` connector auth type.
///
/// | UCS field    | RealEx name                | Used by                          |
/// |--------------|----------------------------|----------------------------------|
/// | `api_key`    | Shared Secret              | every request/response digest    |
/// | `key1`       | Merchant ID                | `<merchantid>` + digest          |
/// | `key2`       | Account (sub-account)      | `<account>`                      |
/// | `api_secret` | Refund / Rebate password   | `<refundhash>` on `rebate` only  |
#[derive(Debug, Clone)]
pub struct GlobalpaymentsRealexAuthType {
    pub shared_secret: Secret<String>,
    pub merchant_id: Secret<String>,
    pub account: Secret<String>,
    /// Only consumed by the (not-yet-implemented) Refund flow's `<refundhash>`.
    pub refund_password: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for GlobalpaymentsRealexAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::GlobalpaymentsRealex {
                shared_secret,
                merchant_id,
                account,
                refund_password,
                ..
            } => Ok(Self {
                shared_secret: shared_secret.clone(),
                merchant_id: merchant_id.clone(),
                account: account.clone(),
                refund_password: refund_password.clone(),
            }),
            _ => Err(report!(IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Expected ConnectorSpecificConfig::GlobalpaymentsRealex containing \
                         shared_secret, merchant_id, account and refund_password"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })),
        }
    }
}

// =============================================================================
// CONNECTOR METADATA (carried from Authorize to the follow-up flows)
// =============================================================================

/// The values a follow-up `settle` / `void` / `rebate` / `query` needs that are **not** carried by
/// any standard UCS field.
///
/// `pasref` travels as the `connector_transaction_id`, but RealEx also requires the **original**
/// `<orderid>` — and, for `rebate`, the original `<authcode>` — verbatim. Neither can be
/// regenerated at capture time (a fresh timestamp-derived order id is rejected), so Authorize
/// publishes them as `connector_metadata`, which the gRPC layer surfaces as
/// `connector_feature_data` on the Authorize response and accepts back on the Capture request.
///
/// Tech spec §14.5 ("what to persist").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalpaymentsRealexPaymentMetadata {
    /// The `<orderid>` **we sent** on the original `auth` (not the gateway's echo, which is
    /// prefixed with `_settle_` on some error documents).
    pub orderid: String,
    /// `<authcode>` from the original `auth` response. Optional because a `5xx` auth carries none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authcode: Option<String>,
}

// =============================================================================
// HASHING (tech spec §3)
// =============================================================================

/// Lowercase hex SHA-1 of the UTF-8 bytes of `input`.
///
/// SHA-1 is dictated by the RealEx wire protocol; it is not used here as a security primitive
/// of our own choosing.
fn sha1_hex(input: &str) -> String {
    hex::encode(ring::digest::digest(
        &ring::digest::SHA1_FOR_LEGACY_USE_ONLY,
        input.as_bytes(),
    ))
}

/// The two-stage RealEx digest:
///
/// ```text
/// stage1 = SHA1_hex(field1 "." field2 "." … "." fieldN)
/// digest = SHA1_hex(stage1 "." sharedSecret)
/// ```
///
/// `fields` are joined verbatim; an absent field contributes an empty string but still emits its
/// separator, because the dots are positional.
fn realex_digest(fields: &[&str], shared_secret: &str) -> String {
    let stage1 = sha1_hex(&fields.join("."));
    sha1_hex(&format!("{stage1}.{shared_secret}"))
}

/// Digest for `type="auth"`: `timestamp.merchantid.orderid.amount.currency.cardnumber`.
///
/// `<account>`, `<channel>`, `<autosettle>` and the whole `<mpi>` block are **not** hashed
/// (verified live — tech spec §3.2).
#[allow(clippy::too_many_arguments)]
fn build_auth_request_hash(
    timestamp: &str,
    merchant_id: &str,
    order_id: &str,
    amount: &str,
    currency: &str,
    card_number: &str,
    shared_secret: &str,
) -> Secret<String> {
    Secret::new(realex_digest(
        &[
            timestamp,
            merchant_id,
            order_id,
            amount,
            currency,
            card_number,
        ],
        shared_secret,
    ))
}

/// Digest for the request types that reference an existing transaction rather than a card —
/// `settle` (Capture) and `void` (Void).
///
/// The blueprint is the same six positional slots as `auth`
/// (`timestamp.merchantid.orderid.amount.currency.cardnumber`), but the amount slot must mirror the
/// body **exactly**:
///
/// * `<amount>` omitted -> `timestamp.merchantid.orderid...`
/// * `<amount>` present -> `timestamp.merchantid.orderid.<amount>..`
///
/// Both variants were **verified live** against the sandbox while implementing the Capture flow;
/// the documentation's partial-settle worked example is a placeholder and could not be used
/// (tech spec §16 item 6). Filling the amount slot while omitting `<amount>` — or filling the
/// currency slot at all — is rejected with `505 sha1hash incorrect`.
///
/// Capture always sends `<amount>` and therefore always uses the amount-filled blueprint; `void`
/// carries no `<amount>` at all, so it passes `None` and the slot stays empty (tech spec §12.2).
fn build_reference_request_hash(
    timestamp: &str,
    merchant_id: &str,
    order_id: &str,
    amount: Option<&str>,
    shared_secret: &str,
) -> Secret<String> {
    Secret::new(realex_digest(
        &[
            timestamp,
            merchant_id,
            order_id,
            amount.unwrap_or_default(),
            // Neither `settle` nor `void` carries a currency attribute or a card number; both slots
            // stay empty but their separators are still emitted.
            "",
            "",
        ],
        shared_secret,
    ))
}

/// Response digest blueprint: `timestamp.merchantid.orderid.result.message.pasref.authcode`,
/// computed from the values in the **response** — in particular the response's own `timestamp`,
/// which is server-local and routinely differs from the request's (tech spec §3.4).
fn build_response_hash(
    response: &GlobalpaymentsRealexPaymentsResponse,
    shared_secret: &str,
) -> String {
    realex_digest(
        &[
            response.timestamp.as_deref().unwrap_or_default(),
            response.merchantid.as_deref().unwrap_or_default(),
            response.orderid.as_deref().unwrap_or_default(),
            response.result.as_str(),
            response.message.as_deref().unwrap_or_default(),
            response.pasref.as_deref().unwrap_or_default(),
            response.authcode.as_deref().unwrap_or_default(),
        ],
        shared_secret,
    )
}

/// Outcome of verifying `<sha1hash>` on a response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HashVerification {
    /// The response carried a `<sha1hash>` and it matched.
    Verified,
    /// The response carried no `<sha1hash>` — this is normal for `5xx` error documents, which are
    /// a smaller shape with no digest (tech spec §10.2). Verification is *skipped*, not failed.
    Skipped,
    /// A digest was present and did not match what we computed.
    Mismatch,
}

fn verify_response_hash(
    response: &GlobalpaymentsRealexPaymentsResponse,
    shared_secret: &str,
) -> HashVerification {
    match response.sha1hash.as_deref() {
        None => HashVerification::Skipped,
        Some(returned) => {
            let computed = build_response_hash(response, shared_secret);
            if computed.eq_ignore_ascii_case(returned) {
                HashVerification::Verified
            } else {
                HashVerification::Mismatch
            }
        }
    }
}

// =============================================================================
// FIELD FORMATTING HELPERS (tech spec §5)
// =============================================================================

/// `YYYYMMDDHHMMSS`, UTC, no separators. The gateway rejects anything more than 86400 s away from
/// its own clock. The same string is used for both the `@timestamp` attribute and the digest.
fn current_timestamp() -> Result<String, error_stack::Report<IntegrationError>> {
    let now = time::OffsetDateTime::now_utc();
    let format = time::macros::format_description!("[year][month][day][hour][minute][second]");
    now.format(&format)
        .change_context(IntegrationError::RequestEncodingFailed {
            context: IntegrationErrorContext {
                additional_context: Some(
                    "Failed to format the current UTC time as YYYYMMDDHHMMSS for the \
                     GlobalpaymentsRealex request timestamp"
                        .to_string(),
                ),
                ..Default::default()
            },
        })
}

/// `<orderid>` accepts `[a-zA-Z0-9_-]` only, 1–50 characters, and must be unique per attempt
/// (a reuse returns `501 … already been processed`). Anything outside the charset is replaced
/// with `-`, then the value is truncated.
fn sanitize_order_id(reference: &str) -> Result<String, error_stack::Report<IntegrationError>> {
    let sanitized: String = reference
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .take(ORDER_ID_MAX_LEN)
        .collect();

    if sanitized.is_empty() {
        return Err(report!(IntegrationError::InvalidDataFormat {
            field_name: "connector_request_reference_id",
            context: IntegrationErrorContext {
                additional_context: Some(
                    "GlobalpaymentsRealex <orderid> cannot be empty; it must be 1-50 characters \
                     of [a-zA-Z0-9_-]"
                        .to_string(),
                ),
                ..Default::default()
            },
        }));
    }

    Ok(sanitized)
}

/// RealEx expects the integer amount in the smallest unit of the currency, with no leading zeros,
/// no decimal point and no separators.
///
/// One documented exception: **JPY amounts must be multiplied by 100 before submission**, unlike
/// every other zero-exponent currency (tech spec §5.4). This could not be exercised against the
/// sandbox account, so it is implemented from the documentation and flagged here.
fn format_amount(
    amount: MinorUnit,
    currency: common_enums::Currency,
) -> Result<String, error_stack::Report<IntegrationError>> {
    let raw = amount.get_amount_as_i64();

    let adjusted = if currency == common_enums::Currency::JPY {
        raw.checked_mul(100).ok_or_else(|| {
            report!(IntegrationError::AmountConversionFailed {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Overflow while applying the GlobalpaymentsRealex JPY x100 rule"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })
        })?
    } else {
        raw
    };

    if adjusted <= 0 {
        return Err(report!(IntegrationError::InvalidDataFormat {
            field_name: "amount",
            context: IntegrationErrorContext {
                additional_context: Some(
                    "GlobalpaymentsRealex rejects zero or negative amounts (508 Zero, negative or \
                     insufficient amount specified)"
                        .to_string(),
                ),
                ..Default::default()
            },
        }));
    }

    Ok(adjusted.to_string())
}

/// The `<card><type>` enum. Case-sensitive uppercase; note `MC`, **not** `MASTERCARD` — the 3DS2
/// JSON API uses a different spelling and the two tables must not be shared (tech spec §5.6).
fn map_card_type<T>(card: &Card<T>) -> Result<&'static str, error_stack::Report<IntegrationError>>
where
    T: PaymentMethodDataTypes,
{
    // Prefer the network supplied by the caller; fall back to BIN detection. Never default to a
    // brand — production cross-validates `<type>` against the PAN.
    if let Some(network) = card.card_network.as_ref() {
        return match network {
            CardNetwork::Visa => Ok("VISA"),
            CardNetwork::Mastercard => Ok("MC"),
            CardNetwork::AmericanExpress => Ok("AMEX"),
            CardNetwork::DinersClub => Ok("DINERS"),
            CardNetwork::Discover => Ok("DISCOVER"),
            CardNetwork::JCB => Ok("JCB"),
            unsupported => Err(report!(IntegrationError::NotImplemented(
                format!("Card network {unsupported:?} is not supported by GlobalpaymentsRealex"),
                IntegrationErrorContext::default(),
            ))),
        };
    }

    match get_card_issuer(card.card_number.peek())? {
        CardIssuer::Visa => Ok("VISA"),
        CardIssuer::Master => Ok("MC"),
        CardIssuer::AmericanExpress => Ok("AMEX"),
        CardIssuer::DinersClub => Ok("DINERS"),
        CardIssuer::Discover => Ok("DISCOVER"),
        CardIssuer::JCB => Ok("JCB"),
        unsupported => Err(report!(IntegrationError::NotImplemented(
            format!("Card issuer {unsupported} is not supported by GlobalpaymentsRealex"),
            IntegrationErrorContext::default(),
        ))),
    }
}

// =============================================================================
// AUTOSETTLE (tech spec §5.7)
// =============================================================================

/// `<autosettle flag="…"/>` is **mandatory** for `type="auth"` — omitting it returns
/// `502 Mandatory Fields missing: [/request/autosettle]`.
///
/// The flag also determines how `result=00` maps to an `AttemptStatus`, so it is threaded through
/// to the response transformer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum GlobalpaymentsRealexAutoSettleFlag {
    /// `flag="1"` — authorize and add to the next settlement file (auth + capture).
    #[serde(rename = "1")]
    AutoSettle,
    /// `flag="0"` — authorize only; a later `type="settle"` is required.
    #[serde(rename = "0")]
    DelayedSettle,
}

impl TryFrom<Option<CaptureMethod>> for GlobalpaymentsRealexAutoSettleFlag {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(capture_method: Option<CaptureMethod>) -> Result<Self, Self::Error> {
        match capture_method {
            None | Some(CaptureMethod::Automatic) | Some(CaptureMethod::SequentialAutomatic) => {
                Ok(Self::AutoSettle)
            }
            Some(CaptureMethod::Manual) => Ok(Self::DelayedSettle),
            // `MULTI` is not enabled on the sandbox account (503 Request type [multisettle] not
            // allowed for this merchant), so multiple partial captures are not claimed here.
            Some(capture_method) => Err(report!(IntegrationError::NotImplemented(
                format!(
                    "Capture method {capture_method:?} is not supported by GlobalpaymentsRealex"
                ),
                IntegrationErrorContext::default(),
            ))),
        }
    }
}

// =============================================================================
// REQUEST (tech spec §4, §6, §7)
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename = "request")]
pub struct GlobalpaymentsRealexPaymentsRequest {
    #[serde(rename = "@type")]
    pub request_type: String,
    #[serde(rename = "@timestamp")]
    pub timestamp: String,
    pub merchantid: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<Secret<String>>,
    pub channel: String,
    pub orderid: String,
    pub amount: GlobalpaymentsRealexAmount,
    pub card: GlobalpaymentsRealexCard,
    pub autosettle: GlobalpaymentsRealexAutoSettle,
    /// 3DS2 authentication result, forwarded verbatim. Absent for non-3DS payments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mpi: Option<GlobalpaymentsRealexMpi>,
    pub sha1hash: Secret<String>,
}

/// `<amount currency="EUR">1001</amount>` — text content plus a currency attribute.
#[derive(Debug, Serialize)]
pub struct GlobalpaymentsRealexAmount {
    #[serde(rename = "@currency")]
    pub currency: String,
    #[serde(rename = "$text")]
    pub value: String,
}

/// `<autosettle flag="1"/>` — attribute only, no text content.
#[derive(Debug, Serialize)]
pub struct GlobalpaymentsRealexAutoSettle {
    #[serde(rename = "@flag")]
    pub flag: GlobalpaymentsRealexAutoSettleFlag,
}

#[derive(Debug, Serialize)]
pub struct GlobalpaymentsRealexCard {
    pub number: Secret<String>,
    /// `MMYY` — four digits, no separator.
    pub expdate: Secret<String>,
    pub chname: Secret<String>,
    #[serde(rename = "type")]
    pub card_type: String,
    /// Omitted entirely when no CVC is available; an empty `<cvn>` is rejected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cvn: Option<GlobalpaymentsRealexCvn>,
}

#[derive(Debug, Serialize)]
pub struct GlobalpaymentsRealexCvn {
    pub number: Secret<String>,
    /// Mandatory whenever `<cvn>` is present.
    pub presind: u8,
}

/// The 3DS2 `<mpi>` block. Populated from an authentication that happened **before** this call —
/// this connector never drives the separate 3DS2 JSON API itself.
///
/// `<exempt_status>` is deliberately not modelled: the sandbox account returns
/// `508 Exemption is not configured for this merchant.` whenever it is sent.
#[derive(Debug, Serialize)]
pub struct GlobalpaymentsRealexMpi {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eci: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds_trans_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication_value: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_version: Option<String>,
}

impl GetSoapXml for GlobalpaymentsRealexPaymentsRequest {
    fn to_soap_xml(&self) -> String {
        // On a serialization failure we emit a minimal, well-formed document rather than panicking;
        // the gateway answers it with `502 Mandatory Fields missing`, which surfaces cleanly.
        quick_xml::se::to_string_with_root("request", self).unwrap_or_else(|error| {
            tracing::error!(
                connector = "globalpayments_realex",
                ?error,
                "Failed to serialize the GlobalpaymentsRealex auth request to XML"
            );
            "<request/>".to_string()
        })
    }
}

impl<T>
    TryFrom<
        GlobalpaymentsRealexRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for GlobalpaymentsRealexPaymentsRequest
where
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: GlobalpaymentsRealexRouterData<
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

        let card = match &request.payment_method_data {
            PaymentMethodData::Card(card) => card,
            _ => {
                return Err(report!(IntegrationError::NotImplemented(
                    "Only card payments are supported by GlobalpaymentsRealex".to_string(),
                    IntegrationErrorContext::default(),
                )))
            }
        };

        let auth = GlobalpaymentsRealexAuthType::try_from(&router_data.connector_config)?;

        // All six digest inputs must be byte-identical to what ends up on the wire, so build the
        // strings once and reuse them for both the body and the hash.
        let timestamp = current_timestamp()?;
        let order_id = sanitize_order_id(
            &router_data
                .resource_common_data
                .connector_request_reference_id,
        )?;
        let amount = format_amount(request.minor_amount, request.currency)?;
        let currency = request.currency.to_string();
        let card_number = card.card_number.peek().to_string();
        let merchant_id = auth.merchant_id.clone().expose();

        let sha1hash = build_auth_request_hash(
            &timestamp,
            &merchant_id,
            &order_id,
            &amount,
            &currency,
            &card_number,
            auth.shared_secret.peek(),
        );

        // A CVC of "" means the caller had none; sending an empty `<cvn>` is rejected, so the whole
        // element is dropped in that case (verified live: accepted, result 00).
        let cvn = Some(card.card_cvc.clone())
            .filter(|cvc| !cvc.peek().is_empty())
            .map(|cvc| GlobalpaymentsRealexCvn {
                number: cvc,
                presind: CVN_PRESENCE_INDICATOR_PRESENT,
            });

        let card_element = GlobalpaymentsRealexCard {
            number: Secret::new(card_number),
            expdate: card.get_expiry_date_as_mmyy()?,
            chname: card
                .get_optional_cardholder_name()
                .filter(|name| !name.peek().trim().is_empty())
                .unwrap_or_else(|| Secret::new(DEFAULT_CARDHOLDER_NAME.to_string())),
            card_type: map_card_type(card)?.to_string(),
            cvn,
        };

        Ok(Self {
            request_type: REQUEST_TYPE_AUTH.to_string(),
            timestamp,
            merchantid: auth.merchant_id,
            account: Some(auth.account),
            channel: CHANNEL_ECOM.to_string(),
            orderid: order_id,
            amount: GlobalpaymentsRealexAmount {
                currency,
                value: amount,
            },
            card: card_element,
            autosettle: GlobalpaymentsRealexAutoSettle {
                flag: GlobalpaymentsRealexAutoSettleFlag::try_from(request.capture_method)?,
            },
            mpi: build_mpi(request),
            sha1hash,
        })
    }
}

/// Builds the `<mpi>` block from an authentication that already completed elsewhere.
///
/// Returns `None` when there is nothing to forward — sending an empty `<mpi>` would be pointless
/// and the digest is unaffected either way.
fn build_mpi<T: PaymentMethodDataTypes>(
    request: &PaymentsAuthorizeData<T>,
) -> Option<GlobalpaymentsRealexMpi> {
    let authentication_data = request.authentication_data.as_ref()?;

    let mpi = GlobalpaymentsRealexMpi {
        eci: authentication_data.eci.clone(),
        // 3DS2 uses `ds_trans_id`; the 3DS1 shape (`xid`) is deliberately not emitted.
        ds_trans_id: authentication_data.ds_trans_id.clone(),
        authentication_value: authentication_data.cavv.clone(),
        message_version: authentication_data
            .message_version
            .as_ref()
            .map(|version| version.to_string()),
    };

    let is_empty = mpi.eci.is_none()
        && mpi.ds_trans_id.is_none()
        && mpi.authentication_value.is_none()
        && mpi.message_version.is_none();

    (!is_empty).then_some(mpi)
}

// =============================================================================
// RESPONSE (tech spec §8, §10)
// =============================================================================

/// The single response shape for **every** outcome.
///
/// The gateway always answers HTTP 200 — success, decline, bad digest and malformed XML alike —
/// and `5xx` result codes come back as a *smaller* document carrying only `timestamp`, `result`,
/// `message` and `orderid`. Every field except `result` is therefore optional.
///
/// `deny_unknown_fields` is deliberately **not** used: the live sandbox returns an undocumented
/// `<fraudresponse>` element and a `<cardissuer>` whose children differ from the documentation.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlobalpaymentsRealexPaymentsResponse {
    #[serde(rename = "@timestamp")]
    pub timestamp: Option<String>,
    /// The outcome code — `00` is success. Kept as a string because the leading zero is
    /// significant to the response digest and to `error_code`.
    pub result: String,
    pub message: Option<String>,
    pub orderid: Option<String>,
    pub merchantid: Option<String>,
    pub account: Option<String>,
    /// The gateway transaction reference — this is the `connector_transaction_id`, and every
    /// follow-up operation (`settle` / `void` / `rebate` / `query`) needs it.
    pub pasref: Option<String>,
    /// Issuer authorization code. Required by a later `rebate`.
    pub authcode: Option<String>,
    /// Settlement batch id. Can be negative (`-1` from `query` when not yet batched), is `0` on a
    /// successful `void`, and comes back as an **empty element** on declines, hence the tolerant
    /// deserializer.
    #[serde(default, deserialize_with = "deserialize_optional_i64")]
    pub batchid: Option<i64>,
    pub cvnresult: Option<String>,
    pub avspostcoderesponse: Option<String>,
    pub avsaddressresponse: Option<String>,
    /// Scheme Reference Data — the scheme transaction id, needed by any future MIT / COF work.
    pub srd: Option<String>,
    pub cardissuer: Option<GlobalpaymentsRealexCardIssuer>,
    /// Absent on `5xx` error documents; verification is skipped in that case.
    pub sha1hash: Option<String>,
}

/// The `settle` response is the **same document shape** as the `auth` response — same root, same
/// optional fields, same `<batchid>`-may-be-empty trap — so it is modelled by the same struct.
///
/// The alias exists because the macro layer derives per-flow helper types from the response type's
/// name and therefore needs a distinct identifier per flow.
pub type GlobalpaymentsRealexCaptureResponse = GlobalpaymentsRealexPaymentsResponse;

/// Deserializes an optional integer that the gateway may send as an **empty element**.
///
/// A declined `auth` answers with `<batchid></batchid>` while a successful one carries a number,
/// and serde cannot turn `""` into an `i64`. Blank text becomes `None` rather than an error.
fn deserialize_optional_i64<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw: Option<String> = Option::deserialize(deserializer)?;
    match raw.as_deref().map(str::trim) {
        None | Some("") => Ok(None),
        Some(value) => value
            .parse::<i64>()
            .map(Some)
            .map_err(serde::de::Error::custom),
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlobalpaymentsRealexCardIssuer {
    pub bank: Option<String>,
    pub country: Option<String>,
    pub countrycode: Option<String>,
    /// Documented but not returned by the live sandbox.
    pub region: Option<String>,
    /// Returned live but undocumented.
    pub commercial: Option<String>,
    /// Returned live but undocumented.
    pub cardtype: Option<String>,
}

impl GlobalpaymentsRealexPaymentsResponse {
    fn is_success(&self) -> bool {
        self.result == RESULT_SUCCESS
    }
}

/// `result` + the `autosettle` flag we sent → `AttemptStatus` (tech spec §9.3).
///
/// There is no pending/async state in this API: every `auth` resolves synchronously, so
/// `AttemptStatus::Pending` is never correct here and every non-`00` code is a `Failure`.
/// `111` (Strong Customer Authentication Required) is a soft decline — still a failure, but its
/// `error_code` is surfaced verbatim so callers can choose to retry through 3DS.
fn map_attempt_status(
    result: &str,
    auto_settle: GlobalpaymentsRealexAutoSettleFlag,
) -> AttemptStatus {
    match (result, auto_settle) {
        (RESULT_SUCCESS, GlobalpaymentsRealexAutoSettleFlag::AutoSettle) => AttemptStatus::Charged,
        (RESULT_SUCCESS, GlobalpaymentsRealexAutoSettleFlag::DelayedSettle) => {
            AttemptStatus::Authorized
        }
        _ => AttemptStatus::Failure,
    }
}

impl<T> TryFrom<ResponseRouterData<GlobalpaymentsRealexPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
where
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        value: ResponseRouterData<GlobalpaymentsRealexPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        // The flag we sent is what decides whether `00` means Charged or Authorized, so recover it
        // from the request rather than guessing from the response.
        let auto_settle =
            GlobalpaymentsRealexAutoSettleFlag::try_from(router_data.request.capture_method)
                .change_context(ConnectorError::ResponseHandlingFailed {
                    context: Default::default(),
                })?;

        let auth = GlobalpaymentsRealexAuthType::try_from(&router_data.connector_config)
            .change_context(ConnectorError::ResponseHandlingFailed {
                context: Default::default(),
            })?;

        let hash_verification = verify_response_hash(&response, auth.shared_secret.peek());

        let status = match hash_verification {
            // A tampered response must not be reported as a successful payment.
            HashVerification::Mismatch => AttemptStatus::IntegrityFailure,
            HashVerification::Verified | HashVerification::Skipped => {
                map_attempt_status(&response.result, auto_settle)
            }
        };

        if hash_verification == HashVerification::Mismatch {
            tracing::warn!(
                connector = "globalpayments_realex",
                order_id = ?response.orderid,
                "GlobalpaymentsRealex response sha1hash did not match the computed digest"
            );
        }

        let message = response
            .message
            .clone()
            .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string());

        let payments_response = if response.is_success()
            && hash_verification != HashVerification::Mismatch
        {
            // `pasref` is the gateway transaction reference every follow-up flow needs; a success
            // without one would leave the payment unreferenceable.
            let pasref = response.pasref.clone().ok_or_else(|| {
                report!(ConnectorError::ResponseHandlingFailed {
                    context: Default::default(),
                })
                .attach_printable(
                    "GlobalpaymentsRealex returned result 00 without a <pasref> transaction \
                     reference",
                )
            })?;

            // The order id we actually put on the wire — recomputed rather than read back from
            // the response so that a follow-up `settle` reuses a byte-identical value.
            let sent_order_id = sanitize_order_id(
                &router_data
                    .resource_common_data
                    .connector_request_reference_id,
            )
            .change_context(ConnectorError::ResponseHandlingFailed {
                context: Default::default(),
            })?;

            let connector_metadata = serde_json::to_value(GlobalpaymentsRealexPaymentMetadata {
                orderid: sent_order_id,
                authcode: response.authcode.clone(),
            })
            .change_context(ConnectorError::ResponseHandlingFailed {
                context: Default::default(),
            })
            .attach_printable(
                "Failed to serialize the GlobalpaymentsRealex follow-up metadata (orderid / \
                     authcode)",
            )?;

            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: DomainResponseId::ConnectorTransactionId(pasref),
                redirection_data: None,
                connector_metadata: Some(connector_metadata),
                mandate_reference: None,
                // Scheme Reference Data is the scheme-level transaction id for this payment.
                network_txn_id: response.srd.clone(),
                network_txn_link_id: None,
                connector_response_reference_id: response.orderid.clone(),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: http_code,
                payment_account_reference: None,
            })
        } else {
            Err(ErrorResponse {
                status_code: http_code,
                // The `<result>` code verbatim — `"101"`, `"505"`, `"111"`, …
                code: response.result.clone(),
                message: message.clone(),
                reason: response.message.clone(),
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: response.pasref.clone(),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            })
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            response: payments_response,
            ..router_data
        })
    }
}

// =============================================================================
// CAPTURE — `type="settle"` (tech spec §12.1)
// =============================================================================

/// The `settle` request.
///
/// It reuses the shared envelope (`<merchantid>`, `<account>`, `<sha1hash>`) but references an
/// existing authorization instead of carrying card data: `<orderid>`, `<pasref>` and `<authcode>`
/// must all be the **original** values from the `auth` that is being settled.
///
/// `<amount>` deliberately has **no** `currency` attribute — unlike `auth` and `rebate`, `settle`
/// takes a bare integer and inherits the currency from the original authorization (tech spec §12.1).
#[derive(Debug, Serialize)]
#[serde(rename = "request")]
pub struct GlobalpaymentsRealexCaptureRequest {
    #[serde(rename = "@type")]
    pub request_type: String,
    #[serde(rename = "@timestamp")]
    pub timestamp: String,
    pub merchantid: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<Secret<String>>,
    /// The `<orderid>` of the transaction being settled, not a new one.
    pub orderid: String,
    /// The gateway reference returned by the original `auth`.
    pub pasref: String,
    /// The issuer authorization code returned by the original `auth`. Omitted when Authorize did
    /// not surface one; the sandbox accepts a `settle` without it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authcode: Option<String>,
    /// Bare integer in the currency's minor unit — **no** `currency` attribute.
    pub amount: String,
    pub sha1hash: Secret<String>,
}

impl GetSoapXml for GlobalpaymentsRealexCaptureRequest {
    fn to_soap_xml(&self) -> String {
        // Mirrors the Authorize flow: a serialization failure emits a minimal well-formed document
        // that the gateway answers with `502 Mandatory Fields missing`, rather than panicking.
        quick_xml::se::to_string_with_root("request", self).unwrap_or_else(|error| {
            tracing::error!(
                connector = "globalpayments_realex",
                ?error,
                "Failed to serialize the GlobalpaymentsRealex settle request to XML"
            );
            "<request/>".to_string()
        })
    }
}

/// Recovers the `<orderid>` / `<authcode>` that Authorize published as `connector_metadata`.
///
/// The gRPC layer hands the same JSON back on either `connector_feature_data` (where the Authorize
/// response emits it) or `metadata`, so both are accepted, in that order. This is the **single**
/// metadata channel for every follow-up flow — Capture and Void both go through here.
fn extract_followup_metadata(
    connector_feature_data: Option<&SecretSerdeValue>,
    metadata: Option<&SecretSerdeValue>,
) -> Option<GlobalpaymentsRealexPaymentMetadata> {
    connector_feature_data.or(metadata).and_then(|value| {
        match serde_json::from_value::<GlobalpaymentsRealexPaymentMetadata>(value.peek().clone()) {
            Ok(metadata) => Some(metadata),
            Err(error) => {
                // Not fatal: the caller may be passing unrelated metadata, in which case the
                // order id falls back to the flow's own merchant reference below.
                tracing::warn!(
                    connector = "globalpayments_realex",
                    ?error,
                    "GlobalpaymentsRealex follow-up metadata did not contain an <orderid>; \
                     falling back to the request reference"
                );
                None
            }
        }
    })
}

impl<T>
    TryFrom<
        GlobalpaymentsRealexRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for GlobalpaymentsRealexCaptureRequest
where
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: GlobalpaymentsRealexRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;

        // RealEx expresses multiple partial captures through `autosettle flag="MULTI"` + repeated
        // `multisettle` requests, which this sandbox account is not provisioned for
        // (`503 Request type [multisettle] not allowed for this merchant` — tech spec §16 item 7).
        // Reject it explicitly rather than silently degrading to a single settle.
        if matches!(request.capture_method, Some(CaptureMethod::ManualMultiple))
            || request.is_multiple_capture()
        {
            return Err(report!(IntegrationError::NotImplemented(
                "Multiple partial captures (ManualMultiple) are not supported by \
                 GlobalpaymentsRealex: the account must be provisioned for the `multisettle` \
                 request type, which returns 503 otherwise"
                    .to_string(),
                IntegrationErrorContext::default(),
            )));
        }

        let auth = GlobalpaymentsRealexAuthType::try_from(&router_data.connector_config)?;
        let metadata = extract_followup_metadata(
            request.connector_feature_data.as_ref(),
            request.metadata.as_ref(),
        );

        // `<orderid>` must be the original one. Preferred source is the metadata Authorize
        // published; `merchant_capture_id` is accepted as a fallback for callers that echo the
        // original reference there.
        let order_id = match metadata.as_ref().map(|metadata| metadata.orderid.clone()) {
            Some(order_id) => sanitize_order_id(&order_id)?,
            None => sanitize_order_id(
                &router_data
                    .resource_common_data
                    .connector_request_reference_id,
            )
            .attach_printable(
                "GlobalpaymentsRealex settle needs the original <orderid>: echo the Authorize \
                 response's connector_feature_data back on the capture request, or set \
                 merchant_capture_id to the original order id",
            )?,
        };

        // `<pasref>` is the gateway reference, i.e. the connector_transaction_id.
        let pasref = request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(IntegrationError::MissingConnectorTransactionID {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "GlobalpaymentsRealex settle needs the original <pasref> as the \
                         connector_transaction_id"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })?;

        // Always sent explicitly, so full and partial settles share one code path and one digest
        // blueprint (both verified live — see `build_reference_request_hash`).
        let amount = format_amount(request.minor_amount_to_capture, request.currency)?;

        let timestamp = current_timestamp()?;
        let merchant_id = auth.merchant_id.clone().expose();
        let sha1hash = build_reference_request_hash(
            &timestamp,
            &merchant_id,
            &order_id,
            Some(&amount),
            auth.shared_secret.peek(),
        );

        Ok(Self {
            request_type: REQUEST_TYPE_SETTLE.to_string(),
            timestamp,
            merchantid: auth.merchant_id,
            account: Some(auth.account),
            orderid: order_id,
            pasref,
            authcode: metadata.and_then(|metadata| metadata.authcode),
            amount,
            sha1hash,
        })
    }
}

/// `result` → `AttemptStatus` for `settle` (tech spec §9.4).
///
/// As with `auth` there is no asynchronous state: a `settle` either succeeds (`00 Settled
/// Successfully`) or fails outright, so `Pending` is never correct here.
fn map_capture_attempt_status(result: &str) -> AttemptStatus {
    if result == RESULT_SUCCESS {
        AttemptStatus::Charged
    } else {
        AttemptStatus::CaptureFailed
    }
}

impl TryFrom<ResponseRouterData<GlobalpaymentsRealexCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        value: ResponseRouterData<GlobalpaymentsRealexCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        let auth = GlobalpaymentsRealexAuthType::try_from(&router_data.connector_config)
            .change_context(ConnectorError::ResponseHandlingFailed {
                context: Default::default(),
            })?;

        // The `settle` response uses the same check-hash blueprint as `auth` (verified live), and
        // the same "no <sha1hash> on 5xx error documents" rule applies.
        let hash_verification = verify_response_hash(&response, auth.shared_secret.peek());

        let status = match hash_verification {
            HashVerification::Mismatch => AttemptStatus::IntegrityFailure,
            HashVerification::Verified | HashVerification::Skipped => {
                map_capture_attempt_status(&response.result)
            }
        };

        if hash_verification == HashVerification::Mismatch {
            tracing::warn!(
                connector = "globalpayments_realex",
                order_id = ?response.orderid,
                "GlobalpaymentsRealex settle response sha1hash did not match the computed digest"
            );
        }

        let message = response
            .message
            .clone()
            .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string());

        let capture_response =
            if response.is_success() && hash_verification != HashVerification::Mismatch {
                // A settle mints its own `<pasref>`; keep the original one when the gateway does
                // not return a new one so the payment stays referenceable either way.
                let resource_id = response
                    .pasref
                    .clone()
                    .map(DomainResponseId::ConnectorTransactionId)
                    .unwrap_or_else(|| router_data.request.connector_transaction_id.clone());

                Ok(PaymentsResponseData::TransactionResponse {
                    resource_id,
                    redirection_data: None,
                    connector_metadata: None,
                    mandate_reference: None,
                    network_txn_id: response.srd.clone(),
                    network_txn_link_id: None,
                    connector_response_reference_id: response.orderid.clone(),
                    incremental_authorization_allowed: None,
                    splits: None,
                    status_code: http_code,
                    payment_account_reference: None,
                })
            } else {
                Err(ErrorResponse {
                    status_code: http_code,
                    code: response.result.clone(),
                    message: message.clone(),
                    reason: response.message.clone(),
                    attempt_status: Some(FlowStatus::Payment(status)),
                    connector_transaction_id: response.pasref.clone(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                })
            };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            response: capture_response,
            ..router_data
        })
    }
}

// =============================================================================
// VOID — `type="void"` (tech spec §12.2)
// =============================================================================

/// The documented `<reasoncode>` enum. RealEx defaults to `NOTGIVEN` when the element is absent, so
/// an unrecognised free-text cancellation reason is dropped rather than guessed at — sending an
/// unknown value is rejected by the schema.
const VOID_REASON_CODES: [&str; 6] = [
    "FRAUD",
    "OUTOFSTOCK",
    "DUPLICATE",
    "MISTAKE",
    "OTHER",
    "NOTGIVEN",
];

/// Maps `cancellation_reason` onto `<reasoncode>` when — and only when — it is one of the six
/// documented values (case-insensitively). Anything else yields `None` and the element is omitted.
///
/// Verified live: sending `<reasoncode>FRAUD</reasoncode>` does not change the digest — the
/// blueprint stays `timestamp.merchantid.orderid...` and the void still returns `00`.
fn map_void_reason_code(cancellation_reason: Option<&str>) -> Option<&'static str> {
    let reason = cancellation_reason?.trim();
    VOID_REASON_CODES
        .into_iter()
        .find(|code| code.eq_ignore_ascii_case(reason))
}

/// The `void` request.
///
/// Reuses the shared envelope and, like `settle`, references an existing authorization by its
/// original `<orderid>` plus the gateway's `<pasref>`. It carries **no `<amount>`** — a void always
/// reverses the full authorization — and no card data.
///
/// `<authcode>` is documented as optional for `void` and is deliberately not sent; the sandbox
/// accepts the request without it (verified live).
///
/// **`<pasref>` must be the pasref minted by the original `auth`.** A `settle` mints a *new*
/// pasref, and voiding with that one returns `508 Original transaction not found.` — verified live.
/// Since UCS surfaces the settle pasref as the Capture response's `connector_transaction_id`, a
/// caller that wants to reverse a captured payment must pass the **Authorize** transaction id here.
#[derive(Debug, Serialize)]
#[serde(rename = "request")]
pub struct GlobalpaymentsRealexVoidRequest {
    #[serde(rename = "@type")]
    pub request_type: String,
    #[serde(rename = "@timestamp")]
    pub timestamp: String,
    pub merchantid: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<Secret<String>>,
    /// The `<orderid>` of the transaction being voided, not a new one.
    pub orderid: String,
    /// The gateway reference returned by the original `auth`.
    pub pasref: String,
    /// One of the six documented reason codes; omitted entirely when the caller supplied none, in
    /// which case the gateway applies its `NOTGIVEN` default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoncode: Option<String>,
    pub sha1hash: Secret<String>,
}

impl GetSoapXml for GlobalpaymentsRealexVoidRequest {
    fn to_soap_xml(&self) -> String {
        // Mirrors the other flows: a serialization failure emits a minimal well-formed document
        // that the gateway answers with `502 Mandatory Fields missing`, rather than panicking.
        quick_xml::se::to_string_with_root("request", self).unwrap_or_else(|error| {
            tracing::error!(
                connector = "globalpayments_realex",
                ?error,
                "Failed to serialize the GlobalpaymentsRealex void request to XML"
            );
            "<request/>".to_string()
        })
    }
}

/// The `void` response is the **same document shape** as every other response on this API, so it
/// reuses the one response struct — including the tolerant `<batchid>` deserializer, which matters
/// here because a successful void returns `<batchid>0</batchid>` while a failure returns the small
/// error document with no `<batchid>` at all.
pub type GlobalpaymentsRealexVoidResponse = GlobalpaymentsRealexPaymentsResponse;

impl<T>
    TryFrom<
        GlobalpaymentsRealexRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for GlobalpaymentsRealexVoidRequest
where
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: GlobalpaymentsRealexRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;

        let auth = GlobalpaymentsRealexAuthType::try_from(&router_data.connector_config)?;

        // Exactly the same metadata channel Capture uses: Authorize publishes the original
        // `<orderid>` as `connector_metadata`, which comes back on `connector_feature_data`.
        let metadata = extract_followup_metadata(
            request.connector_feature_data.as_ref(),
            request.metadata.as_ref(),
        );

        // `<orderid>` must be the reference we originally sent — never the gateway echo, which is
        // prefixed (`_void_…` / `_settle_…`) on error documents. `merchant_void_id` (surfaced as
        // `connector_request_reference_id`) is accepted as a fallback for callers that echo the
        // original reference there.
        let order_id = match metadata.as_ref().map(|metadata| metadata.orderid.clone()) {
            Some(order_id) => sanitize_order_id(&order_id)?,
            None => sanitize_order_id(
                &router_data
                    .resource_common_data
                    .connector_request_reference_id,
            )
            .attach_printable(
                "GlobalpaymentsRealex void needs the original <orderid>: echo the Authorize \
                 response's connector_feature_data back on the void request, or set \
                 merchant_void_id to the original order id",
            )?,
        };

        // `<pasref>` is the gateway reference, i.e. the connector_transaction_id.
        let pasref = request.connector_transaction_id.trim().to_string();
        if pasref.is_empty() {
            return Err(report!(IntegrationError::MissingConnectorTransactionID {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "GlobalpaymentsRealex void needs the original <pasref> as the \
                         connector_transaction_id"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            }));
        }

        let timestamp = current_timestamp()?;
        let merchant_id = auth.merchant_id.clone().expose();
        // No `<amount>` on the wire, so the amount slot of the digest stays empty:
        // `timestamp.merchantid.orderid...` (tech spec §12.2).
        let sha1hash = build_reference_request_hash(
            &timestamp,
            &merchant_id,
            &order_id,
            None,
            auth.shared_secret.peek(),
        );

        Ok(Self {
            request_type: REQUEST_TYPE_VOID.to_string(),
            timestamp,
            merchantid: auth.merchant_id,
            account: Some(auth.account),
            orderid: order_id,
            pasref,
            reasoncode: map_void_reason_code(request.cancellation_reason.as_deref())
                .map(str::to_string),
            sha1hash,
        })
    }
}

/// `result` → `AttemptStatus` for `void` (tech spec §9.4).
///
/// As with `auth` and `settle` there is no asynchronous state: a void either succeeds
/// (`00 Voided Successfully`) or fails outright, so `Pending` is never correct here. Every non-`00`
/// code is a plain `VoidFailed` carrying the gateway's own code and message — never a
/// transport-level error and never silently mapped to success.
///
/// Failure codes observed live, all as the small error document (HTTP 200, no `<sha1hash>`,
/// no `<batchid>`):
///
/// | Scenario | `result` | `message` |
/// |---|---|---|
/// | Voiding an already-voided transaction | `508` | `That transaction has already been voided.` |
/// | Voiding with the pasref a `settle` minted | `508` | `Original transaction not found.` |
/// | Unparseable `<pasref>` | `506` | `… does not conform to the schema` |
///
/// The documented `513 Can't void a settled transaction` could **not** be reproduced: an
/// `auth` + `settle` pair is still voidable through its *original* pasref (`00 Voided
/// Successfully`) because `settle` only queues the transaction into the open batch. `513` presumably
/// only appears once that batch has actually been closed, which a merchant cannot trigger on
/// demand. Either way it lands in the same `VoidFailed` branch as the codes above.
fn map_void_attempt_status(result: &str) -> AttemptStatus {
    if result == RESULT_SUCCESS {
        AttemptStatus::Voided
    } else {
        AttemptStatus::VoidFailed
    }
}

impl TryFrom<ResponseRouterData<GlobalpaymentsRealexVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        value: ResponseRouterData<GlobalpaymentsRealexVoidResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        let auth = GlobalpaymentsRealexAuthType::try_from(&router_data.connector_config)
            .change_context(ConnectorError::ResponseHandlingFailed {
                context: Default::default(),
            })?;

        // Same check-hash blueprint as `auth` and `settle`, and the same "no <sha1hash> on the
        // small 5xx error document" rule.
        let hash_verification = verify_response_hash(&response, auth.shared_secret.peek());

        let status = match hash_verification {
            HashVerification::Mismatch => AttemptStatus::IntegrityFailure,
            HashVerification::Verified | HashVerification::Skipped => {
                map_void_attempt_status(&response.result)
            }
        };

        if hash_verification == HashVerification::Mismatch {
            tracing::warn!(
                connector = "globalpayments_realex",
                order_id = ?response.orderid,
                "GlobalpaymentsRealex void response sha1hash did not match the computed digest"
            );
        }

        let message = response
            .message
            .clone()
            .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string());

        let void_response =
            if response.is_success() && hash_verification != HashVerification::Mismatch {
                // Keep the payment referenceable: a void response echoes a `<pasref>`, but fall back to
                // the one we sent if the gateway omits it.
                let resource_id = DomainResponseId::ConnectorTransactionId(
                    response
                        .pasref
                        .clone()
                        .unwrap_or_else(|| router_data.request.connector_transaction_id.clone()),
                );

                Ok(PaymentsResponseData::TransactionResponse {
                    resource_id,
                    redirection_data: None,
                    connector_metadata: None,
                    mandate_reference: None,
                    network_txn_id: response.srd.clone(),
                    network_txn_link_id: None,
                    connector_response_reference_id: response.orderid.clone(),
                    incremental_authorization_allowed: None,
                    splits: None,
                    status_code: http_code,
                    payment_account_reference: None,
                })
            } else {
                Err(ErrorResponse {
                    status_code: http_code,
                    // The `<result>` code verbatim — e.g. `"513"` for an already-settled transaction.
                    code: response.result.clone(),
                    message: message.clone(),
                    reason: response.message.clone(),
                    attempt_status: Some(FlowStatus::Payment(status)),
                    connector_transaction_id: response.pasref.clone(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                })
            };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            response: void_response,
            ..router_data
        })
    }
}
